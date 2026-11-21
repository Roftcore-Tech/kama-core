//! [ONARIM-SIRASI] Geri bildirimi aç profilde kapanmış FEC bloğunun onarım sembolleri sonraki
//! blokların kaynaklarından önce tele çıkar.
//!
//! Düzenek: ürünün geri bildirimi aç profil kurulumu (12 500 / 62 B/s, `FeedbackStarvedRule`),
//! statik r = 4 FEC. Üç blok × 8 dosya birimi sunulur; her blok son biriminin hemen ardından
//! kapanır (üst katmanın blok kapanışıyla aynı sıra). ACK gelmez. Profil kapalıyken sıra eskisi
//! gibidir: onarımlar gönderim kuyruğu boşalınca çıkar.

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::feedback_horizon_ns;
use adaptive_transport::sender::{
    Event, FecActuationMode, FeedbackStarvedRule, OpaqueUnit, Sender, SrLogicalRef, Timer,
    TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const UP: u64 = 12_500;
const DOWN: u64 = 62;
const OVH: u64 = 28;
const FILE_UNIT: u64 = 1046;
const PARITY: u64 = 1086;
const K: u64 = 8;
const R: u16 = 4;
const BLOCKS: u64 = 3;

/// Tele çıkış sırası: (`Data`, birim) ya da (`FecParity`, blok).
fn emission_order(starved: bool) -> Vec<(TransmitKind, u64)> {
    let mut s = Sender::with_cc_port();
    let t_ack = adaptive_transport::receiver::budgeted_ack_interval_ns(DOWN);
    s.set_cc_initial_window(UP * t_ack / SEC);
    let cap = UP * MTU / (MTU + OVH);
    s.set_cc_profile_pacing_cap(Some(cap));
    if starved {
        s.set_cc_feedback_starved(Some(FeedbackStarvedRule {
            pacing_cap_bps: cap,
            feedback_ns: feedback_horizon_ns(DOWN),
            line: LineModel {
                rate_bps: UP,
                datagram_overhead: OVH,
            },
            return_line: LineModel {
                rate_bps: DOWN,
                datagram_overhead: OVH,
            },
        }));
    }
    s.set_establish_probe(false);
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .expect("mode");
    let mut out = Vec::new();
    for b in 0..BLOCKS {
        for i in 0..K {
            let id = b * K + i;
            out.extend(s.on_event(
                Event::AppData(OpaqueUnit {
                    id,
                    size: FILE_UNIT,
                }),
                0,
            ));
        }
        let auth = s.open_fec_block_authority();
        let r = s
            .close_fec_block_manual(auth, b as u32, R, PARITY)
            .expect("close");
        assert_eq!(r, R);
    }
    let mut now = 0;
    while now < 5 * SEC {
        now += MS;
        out.extend(s.on_event(Event::TimerFired(Timer::Pto), now));
    }
    out.iter()
        .filter_map(|d| match d.kind {
            TransmitKind::Data => Some((TransmitKind::Data, d.unit_id)),
            TransmitKind::FecParity => d.send_record.map(|h| {
                let SrLogicalRef::Fec { block_id, .. } = h.logical_ref;
                (TransmitKind::FecParity, u64::from(block_id))
            }),
            _ => None,
        })
        .collect()
}

fn pos(order: &[(TransmitKind, u64)], f: impl Fn(&(TransmitKind, u64)) -> bool) -> Vec<usize> {
    order
        .iter()
        .enumerate()
        .filter(|(_, e)| f(e))
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn closed_block_repairs_precede_later_block_sources() {
    let order = emission_order(true);
    let rep0 = pos(&order, |e| *e == (TransmitKind::FecParity, 0));
    let src0_last = *pos(&order, |e| e.0 == TransmitKind::Data && e.1 < K)
        .last()
        .expect("blok 0 kaynakları");
    let src2_first = pos(&order, |e| e.0 == TransmitKind::Data && e.1 >= 2 * K)[0];
    assert_eq!(rep0.len(), usize::from(R), "{order:?}");
    assert!(
        rep0.iter().all(|&p| p > src0_last && p < src2_first),
        "blok 0 onarımları kaynaklarından sonra, blok 2 kaynaklarından önce: {order:?}"
    );
}

/// Profil kapalı yol değişmez: onarımlar gönderim kuyruğu boşaldıktan sonra.
#[test]
fn profile_off_keeps_repairs_after_queued_data() {
    let order = emission_order(false);
    let last_data = *pos(&order, |e| e.0 == TransmitKind::Data)
        .last()
        .expect("veri");
    let first_rep = pos(&order, |e| e.0 == TransmitKind::FecParity)[0];
    assert!(first_rep > last_data, "{order:?}");
}
