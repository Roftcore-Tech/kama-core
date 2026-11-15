//! [ADAY · eski motor canlılığı] Teslim kanalı dolu alıcı, üç PTO turundan uzun durduğunda eski
//! motor oturumu kalıcı durdurmamalı — deterministik benzetim (gerçek saat, soket, tokio YOK).
//!
//! Ne benzetiliyor: `ack_after_admit.rs` / `legacy_stalled_consumer_loses_no_acked_unit`
//! düzeneği. Gönderici `ClientSession::new()` (eski motor, ürün yapılandırması); alıcı eski
//! `Receiver` (`quota_r` 4, sunucu yuvasıyla aynı kurulum). Çalışma-zamanı yapıştırıcısının iki
//! kuralı birebir: (1) teslim kanalı (4 yer) doluyken datagram alıcı motoruna VERİLMEZ, ACK
//! üretilmez (`runtime.rs` sunucu görevi, `data_not_admitted_full`); (2) zamanlayıcılar çalışma
//! zamanının aralıklarında: gönderici PTO 1 ms, RtpropProbe 200 ms, alıcı ACK zamanlayıcısı 20 ms.
//! Bağlantı kayıpsız, tek yön gecikme bir tik (RTT ≈ 1 ms, geri döngü gibi).
//!
//! Sıralama (CI dökümündeki yarış sonucu, sabitlenmiş): ilk 4 birim PreConverge'de gider; geri
//! kalan 60 birim ilk ACK işlendikten SONRA sunulur. Böylece hiçbir reddedilen birimin §3.3-B
//! PreConverge kopyası olmaz (dökümde `tx_dup_bytes: 0`, `left_preconverge_ns` ≈ 0,5 ms). Bu
//! kopyalar (ilk gecikme ≈ 5 s) varsa oturum onlarla kurtulur; testin geçmesi o rastlantıya
//! bağlıydı.
//!
//! Kusur (tavanlı `validated_v1`, prob turu tavanı 3): üç prob duraklamanın ilk ≈ 200 ms'sinde
//! ateşler ve reddedilir; kuyruk birimleri `largest_acked`'in üstünde olduğundan paket/zaman
//! eşiği kayıp ilan edemez; alıcının eski durumu yineleyen ACK'leri `ack_confidence`'ı tam tutar
//! (RecoverFwd'e girilmez) → hiçbir yol ateşlemez, 60 s'de 60 birim eksik. Düzeltme adayı
//! (`validated_v1_uncapped`): prob üstel geri çekilmeyle sürer, duraklama bitince ilk prob kabul
//! edilir, ACK'i paket eşiğiyle kalanları ARQ'ya sokar.

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, SenderState, Timer, TransmitKind};
use client_core::{ClientSession, TransmitDecision};

const MS: u64 = 1_000_000;
const UNITS: u64 = 64;
/// İlk sunulan (PreConverge'de giden) birim sayısı = teslim kanalının yeri.
const FIRST: u64 = 4;
const QUEUE: usize = 4;
/// Birimin tel boyu (≈ "admit-unit-N" + DATA başlığı + mühür).
const UNIT_WIRE: u64 = 40;
const STALL_END_NS: u64 = 1_500 * MS;
const HORIZON_NS: u64 = 60_000 * MS;
const ACK_TIMER_NS: u64 = 20 * MS;
const RTPROP_TICK_NS: u64 = 200 * MS;

/// Benzetimin sonucu ve duraklama sonundaki ön koşul görüntüsü.
#[derive(Debug)]
struct Outcome {
    /// Uygulamaya teslim edilen (ilk-teslim) birimler.
    delivered: BTreeSet<u64>,
    /// Göndericinin teyit ettiği birimler (`take_confirmed_units`).
    confirmed: BTreeSet<u64>,
    /// Son birimin hem teslim hem teyit edildiği an (ns); bitmediyse `None`.
    done_ns: Option<u64>,
    /// Duraklama sonunda: tele çıkan kuyruk probu, kopya (Duplicate) ve reddedilen datagram.
    tail_probes_at_stall_end: u64,
    dups_at_stall_end: u64,
    refused_at_stall_end: u64,
    /// Benzetim boyunca gönderici RecoverFwd'e girdi mi.
    entered_recover_fwd: bool,
}

fn simulate(mut s: ClientSession, mut rx: Receiver) -> Outcome {
    rx.set_quota_r(4);
    // Gönderici → alıcı: (seq, birim); alıcı → gönderici: (onaylanan seq'ler, ack_delay).
    let mut to_rx: VecDeque<(u64, u64)> = VecDeque::new();
    let mut to_tx: VecDeque<(Vec<u64>, Duration)> = VecDeque::new();
    let mut channel: VecDeque<u64> = VecDeque::new();
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let mut delivered: BTreeSet<u64> = BTreeSet::new();
    let mut confirmed: BTreeSet<u64> = BTreeSet::new();
    let mut next_seq: u64 = 0;
    let mut refused: u64 = 0;
    let mut rest_offered = false;
    let mut entered_recover_fwd = false;
    let mut at_stall_end: Option<(u64, u64, u64)> = None;
    let mut done_ns = None;

    let emit = |out: Vec<TransmitDecision>, to_rx: &mut VecDeque<(u64, u64)>, next: &mut u64| {
        for d in out {
            if matches!(
                d.kind,
                TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
            ) {
                to_rx.push_back((d.seq, d.unit_id));
                *next = (*next).max(d.seq + 1);
            }
        }
    };

    for id in 0..FIRST {
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id,
                size: UNIT_WIRE,
            }),
            0,
        );
        emit(out, &mut to_rx, &mut next_seq);
    }
    let mut t = 0u64;
    while t <= HORIZON_NS {
        // Alıcı: teslim kanalında yer yoksa datagram motora verilmez (ACK yok).
        while let Some((seq, unit)) = to_rx.pop_front() {
            if channel.len() >= QUEUE {
                refused += 1;
                continue;
            }
            let out = rx.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: unit,
                    kind: RxKind::Data,
                },
                t,
            );
            if out.delivered.contains(&unit) && seen.insert(unit) {
                channel.push_back(unit);
            }
            if let Some(a) = out.ack {
                to_tx.push_back((a.acked_full(next_seq), a.ack_delay_duration()));
            }
        }
        if t % ACK_TIMER_NS == 0 {
            if let Some(a) = rx.on_ack_timer(t) {
                to_tx.push_back((a.acked_full(next_seq), a.ack_delay_duration()));
            }
        }
        // Tüketici duraklama bitince her tikte kanalı boşaltır.
        if t >= STALL_END_NS {
            delivered.extend(channel.drain(..));
        }
        if t == STALL_END_NS {
            let a = s.sender().diag_activation();
            at_stall_end = Some((a.tail_probes, a.dups, refused));
        }
        t += MS;
        // Gönderici: ACK'ler bir tik sonra varır.
        for (acked_seqs, ack_delay) in std::mem::take(&mut to_tx) {
            let out = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs,
                    ack_delay,
                }),
                t,
            );
            emit(out, &mut to_rx, &mut next_seq);
            confirmed.extend(s.sender_mut().take_confirmed_units());
            if !rest_offered {
                // Kalan birimler ilk ACK işlendikten sonra (PreConverge'den çıkılmış).
                rest_offered = true;
                assert_ne!(s.sender().state(), SenderState::PreConverge);
                for id in FIRST..UNITS {
                    let out = s.on_event(
                        Event::AppData(OpaqueUnit {
                            id,
                            size: UNIT_WIRE,
                        }),
                        t,
                    );
                    emit(out, &mut to_rx, &mut next_seq);
                }
            }
        }
        let out = s.on_event(Event::TimerFired(Timer::Pto), t);
        emit(out, &mut to_rx, &mut next_seq);
        if t % RTPROP_TICK_NS == 0 {
            let out = s.on_event(Event::TimerFired(Timer::RtpropProbe), t);
            emit(out, &mut to_rx, &mut next_seq);
        }
        confirmed.extend(s.sender_mut().take_confirmed_units());
        entered_recover_fwd |= s.sender().state() == SenderState::RecoverFwd;
        if delivered.len() as u64 == UNITS && confirmed.len() as u64 == UNITS {
            done_ns = Some(t);
            break;
        }
    }
    let (tail_probes_at_stall_end, dups_at_stall_end, refused_at_stall_end) =
        at_stall_end.expect("benzetim duraklama sonuna ulaştı");
    Outcome {
        delivered,
        confirmed,
        done_ns,
        tail_probes_at_stall_end,
        dups_at_stall_end,
        refused_at_stall_end,
        entered_recover_fwd,
    }
}

/// Eski motor (geri alma yolu, `ClientSession::new()`): 64 birim, 4'lük teslim kanalı, 1,5 s
/// duran tüketici. Tavanlı yapılandırmada 60 s sonunda 4 teslim / 4 teyit kalıyordu.
#[test]
fn legacy_full_receiver_longer_than_three_ptos_does_not_stall_the_session() {
    let o = simulate(ClientSession::new(), Receiver::new());
    // Ön koşul — düzenek kusurun şeklini kuruyor: duraklama içinde en az üç prob tele çıktı ve
    // reddedildi, hiçbir §3.3-B kopyası yok (kurtarma rastlantıya kalmıyor), RecoverFwd yok.
    assert!(
        o.tail_probes_at_stall_end >= 3,
        "duraklama içinde en az üç kuyruk probu (eski tavan) beklenir: {o:?}"
    );
    assert_eq!(
        o.dups_at_stall_end, 0,
        "PreConverge kopyası olmamalı: {o:?}"
    );
    assert!(
        o.refused_at_stall_end >= UNITS - FIRST + 3,
        "60 birim + 3 prob reddedilmiş olmalı: {o:?}"
    );
    assert!(
        !o.entered_recover_fwd,
        "kurtarma RecoverFwd'den gelmemeli: {o:?}"
    );
    // İddia: oturum durmaz — her birim tam bir kez teslim edilir ve teyit edilir.
    let all: BTreeSet<u64> = (0..UNITS).collect();
    assert_eq!(
        o.delivered,
        all,
        "60 s içinde hepsi teslim edilmeli — eksik {} birim: {o:?}",
        UNITS - o.delivered.len() as u64
    );
    assert_eq!(o.confirmed, all, "her birim teyit edilmeli: {o:?}");
    // Canlılık sınırı: duraklama bittikten sonra ilk prob en çok bir geri çekilmiş PTO sonra gelir
    // (≤ duraklama süresi kadar); 3 s = 2 × duraklama.
    let done = o.done_ns.expect("bitti");
    assert!(
        done <= 2 * STALL_END_NS,
        "duraklama bitince oturum bir geri çekilmiş PTO içinde sürmeli: {} ms",
        done / MS
    );
}

/// Port motoru (ürün varsayılanı) aynı benzetimde: prob turu tavanı yok (`rfc9002_cc_port`),
/// bu yüzden aynı yarışta durmuyordu. Eşlik kanıtı.
#[test]
fn port_full_receiver_longer_than_three_ptos_does_not_stall_the_session() {
    let o = simulate(ClientSession::with_cc_port(), Receiver::with_cc_port());
    let all: BTreeSet<u64> = (0..UNITS).collect();
    assert_eq!(o.delivered, all, "{o:?}");
    assert_eq!(o.confirmed, all, "{o:?}");
    assert!(o.tail_probes_at_stall_end >= 3, "{o:?}");
    let done = o.done_ns.expect("bitti");
    assert!(done <= 2 * STALL_END_NS, "{} ms", done / MS);
}
