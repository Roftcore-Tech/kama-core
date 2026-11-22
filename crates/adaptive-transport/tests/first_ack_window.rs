//! [İŞ-33 C4 alıcı] Alıcının ilk varış penceresi ve ilk ACK bekletmesi (gerçek `Receiver`, açık
//! API): yalnız geri bildirimi aç alıcı ve yalnız datagram boyu verildiğinde ölçer; ölçüm yoksa
//! ilk ACK bugünkü gibi hemen çıkar.
//!
//! Spec etkisi (§12 [R1]; sahip kararı 2026-11-21 madde 1, koşullu GO; spec YAMA KAYDI
//! 2026-11-21): bekletme sırasında ilk ACK ilk varışlarda hemen çıkmaz; pencere kapanınca ya da en
//! çok `FIRST_ACK_HOLD_MAX_NS` sonra çıkar.

use adaptive_transport::ack::{AckFrame, ARRIVAL_RATE_SENTINEL};
use adaptive_transport::arrival_rate::FIRST_ACK_HOLD_MAX_NS;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;

fn starved_receiver() -> Receiver {
    let mut r = Receiver::with_cc_port();
    // Ürün profili: dönüş 62 B/s → bütçe 15 B/s, tipik ACK'ten küçük (geri bildirimi aç alıcı).
    r.set_return_bandwidth(Some(62));
    r
}

/// Bir datagram (boyu `size` ise verilir); dönen ACK varsa o.
fn arrive(r: &mut Receiver, seq: u64, size: Option<u64>, now: u64) -> Option<AckFrame> {
    if let Some(b) = size {
        r.set_next_datagram_bytes(b);
    }
    r.on_datagram(
        RxDatagram {
            seq,
            unit_id: seq,
            kind: RxKind::Data,
        },
        now,
    )
    .ack
}

/// `[from, to)` boyunca 20 ms'de bir ACK zamanlayıcısı; çıkan ilk ACK ve anı.
fn timer_until(r: &mut Receiver, from: u64, to: u64) -> Option<(u64, AckFrame)> {
    let mut t = from;
    while t < to {
        if let Some(a) = r.on_ack_timer(t) {
            return Some((t, a));
        }
        t += 20 * MS;
    }
    None
}

/// Geri bildirimi aç alıcı, boy verilir: küçük ilk datagram pencereyi açmaz ama ilk ACK'i
/// bekletir; ilk parça pencereyi açar, ikinci parça kapatır ve ilk ACK pencerenin hızını taşır.
/// Sonraki ACK'ler sentinel taşır.
#[test]
fn starved_receiver_holds_the_first_ack_for_the_window_and_reports_its_rate() {
    let mut r = starved_receiver();
    assert!(arrive(&mut r, 0, Some(128), 300 * MS).is_none());
    assert!(
        timer_until(&mut r, 300 * MS, 400 * MS).is_none(),
        "ilk ACK bekletilir"
    );
    assert!(arrive(&mut r, 1, Some(1046), 400 * MS).is_none());
    assert!(
        timer_until(&mut r, 400 * MS, 570 * MS).is_none(),
        "pencere açık"
    );
    // 1 046 B, 172,8 ms sonra (6 250 B/s tel hızı): pencere kapanır.
    let first = arrive(&mut r, 2, Some(1046), 572_800_000)
        .or_else(|| timer_until(&mut r, 572_800_000, 700 * MS).map(|(_, a)| a))
        .expect("pencere kapanınca ilk ACK çıkar");
    assert_eq!(first.arrival_rate, 6053, "1 046 B / 0,1728 s");
    // Sonraki ACK'ler (bütçe izin verdikçe) sentinel.
    let mut later = Vec::new();
    for k in 3..200u64 {
        let t = 700 * MS + k * 100 * MS;
        later.extend(arrive(&mut r, k, Some(1046), t));
        later.extend(r.on_ack_timer(t));
    }
    assert!(!later.is_empty(), "sonraki ACK çıkmadı");
    assert!(later
        .iter()
        .all(|a| a.arrival_rate == ARRIVAL_RATE_SENTINEL));
}

/// Boy verilmezse (eski çalışma zamanı, bench): ölçüm yok, ilk ACK bekletilmez, alan sentinel.
#[test]
fn without_sizes_the_first_ack_is_not_held() {
    let mut r = starved_receiver();
    let _ = arrive(&mut r, 0, None, 300 * MS);
    let (at, a) = timer_until(&mut r, 300 * MS, 400 * MS).expect("ilk ACK hemen");
    assert!(at < 400 * MS);
    assert_eq!(a.arrival_rate, ARRIVAL_RATE_SENTINEL);
}

/// Geri bildirimi aç olmayan alıcı (bütçe tipik ACK'ten büyük): boy verilse de ölçmez.
#[test]
fn an_unstarved_receiver_does_not_measure() {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(1_000_000));
    let _ = arrive(&mut r, 0, Some(1046), 300 * MS);
    let (_, a) = timer_until(&mut r, 300 * MS, 400 * MS).expect("ilk ACK hemen");
    assert_eq!(a.arrival_rate, ARRIVAL_RATE_SENTINEL);
}

/// Yalnız küçük datagramlar (ör. yalnız PTT): pencere açılmaz, ilk ACK bekletme tavanında
/// sentinel'le çıkar.
#[test]
fn small_datagrams_only_release_the_first_ack_at_the_hold_cap() {
    let mut r = starved_receiver();
    let t0 = 300 * MS;
    let mut first = None;
    let mut t = t0;
    let mut seq = 0u64;
    while t < t0 + 2 * SEC && first.is_none() {
        if (t - t0).is_multiple_of(40 * MS) {
            first = arrive(&mut r, seq, Some(86), t).map(|a| (t, a));
            seq += 1;
        }
        if first.is_none() {
            first = r.on_ack_timer(t).map(|a| (t, a));
        }
        t += 20 * MS;
    }
    let (at, a) = first.expect("ilk ACK çıktı");
    assert!(
        at >= t0 + FIRST_ACK_HOLD_MAX_NS,
        "tavandan önce çıktı: {at}"
    );
    assert!(at < t0 + FIRST_ACK_HOLD_MAX_NS + 100 * MS);
    assert_eq!(a.arrival_rate, ARRIVAL_RATE_SENTINEL);
}
