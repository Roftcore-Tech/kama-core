//! [ACK-BITMAP · ÖLÇÜM ARALIĞI] 0x15 `rx_span` sözleşmesi (#175 7067964700): M = `[base, largest]`
//! içinde alınan seq'ler, `m_first` = M'nin en küçüğü; alan = alıcıda
//! `ilk_varış(largest) − ilk_varış(m_first)` (256 µs, aşağı yuvarlı). Kayıp, çift datagram ve
//! tavan koşulları; gönderici aynı kümeyi bitmap'ten kurar.

use adaptive_transport::ack::{AckFrame, ACK_DELAY_UNIT_US, RX_SPAN_NONE};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;

fn rx() -> Receiver {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r
}

fn arrive(r: &mut Receiver, seq: u64, t: u64) -> Vec<AckFrame> {
    let mut out: Vec<AckFrame> = r
        .on_datagram(
            RxDatagram {
                seq,
                unit_id: seq,
                kind: RxKind::Data,
            },
            t,
        )
        .ack
        .into_iter()
        .collect();
    out.extend(r.on_ack_timer(t));
    out
}

#[test]
fn rx_span_is_the_data_arrival_time_of_the_reported_interval() {
    let mut r = rx();
    // 300 seq, her biri 80 ms arayla; her 5'te biri düşüyor; seq 40 iki kez geliyor.
    let mut arrival = std::collections::BTreeMap::new();
    let mut frames = Vec::new();
    for seq in 0..300u64 {
        let t = 1_000 * MS + seq * 80 * MS;
        if seq % 5 == 3 {
            continue;
        }
        arrival.insert(seq, t);
        frames.extend(arrive(&mut r, seq, t));
        if seq == 40 {
            frames.extend(arrive(&mut r, 40, t + 30 * MS)); // çift: ilk varış sayılır
        }
    }
    let bitmap_frames: Vec<_> = frames.iter().filter(|f| f.bitmap.is_some()).collect();
    assert!(bitmap_frames.len() >= 2, "bitmap ACK çıkmadı");
    let mut measured = 0;
    for f in bitmap_frames {
        let bytes = f.encode();
        let g = AckFrame::decode_with_bitmap(&bytes).expect("çözülür");
        let largest = u64::from(g.largest_seq);
        let (lo, hi) = g.evidence_span_full(largest + 1).expect("kanıt aralığı");
        let acked = g.acked_full(largest + 1);
        let m: Vec<u64> = acked.into_iter().filter(|&s| s >= lo && s <= hi).collect();
        let m_first = *m.iter().min().expect("M boş değil");
        let span = g.bitmap.as_ref().expect("bitmap").rx_span;
        if m_first == hi {
            assert_eq!(span, RX_SPAN_NONE);
            continue;
        }
        let want = (arrival[&hi] - arrival[&m_first]) / (ACK_DELAY_UNIT_US * 1000);
        assert_eq!(u64::from(span), want, "M = [{m_first}, {hi}]");
        measured += 1;
    }
    assert!(measured >= 1, "hiç ölçüm yok");
}

#[test]
fn v2_receiver_has_no_rx_span() {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    let mut frames = Vec::new();
    for seq in 0..50u64 {
        frames.extend(arrive(&mut r, seq, seq * 80 * MS));
    }
    assert!(!frames.is_empty());
    assert!(frames.iter().all(|f| f.bitmap.is_none()));
}

/// Bitmap oturum ortasında (CAPS gelince) açılır: ilk bitmap ACK'lerinin M'si oturum başına uzanır.
/// Varış anları bitmap'ten önce de tutulduğu için bu ACK'ler ölçüm taşır (karşı-olgu: tutulmasaydı
/// `RX_SPAN_NONE` dönerdi ve gönderici eski cephe tahmincisinde kalırdı).
#[test]
fn rx_span_covers_arrivals_from_before_bitmap_was_enabled() {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    let mut t = 0u64;
    for seq in 0..20u64 {
        let _ = arrive(&mut r, seq, t);
        t += 80 * MS;
    }
    r.set_ack_bitmap(true);
    let mut first_bitmap = None;
    for seq in 20..200u64 {
        for f in arrive(&mut r, seq, t) {
            if first_bitmap.is_none() && f.bitmap.is_some() {
                first_bitmap = Some(f);
            }
        }
        t += 80 * MS;
    }
    let f = first_bitmap.expect("bitmap ACK çıkmadı");
    let largest = u64::from(f.largest_seq);
    let (lo, _) = f.evidence_span_full(largest + 1).expect("kanıt aralığı");
    assert_eq!(lo, 0, "ilk bitmap ACK'inin tabanı oturum başı");
    let span = f.bitmap.as_ref().expect("bitmap").rx_span;
    assert_ne!(span, RX_SPAN_NONE, "bitmap'ten önceki varışlar ölçümde yok");
    assert_eq!(
        u64::from(span),
        largest * 80 * MS / (ACK_DELAY_UNIT_US * 1000),
        "M = [0, {largest}]"
    );
}
