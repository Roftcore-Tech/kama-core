//! [ÖNCELİK · ALARM TEK YOL VERİR] Dar dönüşte (62 B/s) aynı cihazın verisi hattı tutarken daralma
//! alarmının ACK'i hattı veriye en çok bir kez bırakır; alarm dışı ACK bugünkü gibi bekler.
//!
//! Ürün gözlemi (`kama-integration` `asym_product_outcome`, düşüş hücresi): Bob'un dönüş hattı
//! alındılar ve bekletilen RECOVERED'larla saniyelerce kesintisiz doluydu; her ACK denemesi "hat
//! dolu" görüp yol verdi ve göndericiye 8–18 s ACK varmadı. Bu testler hattı aynı biçimde sürekli
//! dolu tutar (her denemede önünde 1 s serileşme var). Karşı-olgu: eski kural (hat doluyken her
//! zaman yol ver) alarm testinde hiç ACK çıkarmaz.

use adaptive_transport::ack::AckFrame;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
/// Hattın her denemede önündeki serileşme: başka bir datagram sürekli hattı alıyor.
const AHEAD: u64 = 1_000 * MS;

fn arrive_busy(r: &mut Receiver, seq: u64, t: u64) -> Vec<AckFrame> {
    r.set_return_line_ahead_ns(AHEAD);
    r.set_uplink_yield(true);
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
    r.set_return_line_ahead_ns(AHEAD);
    r.set_uplink_yield(true);
    out.extend(r.on_ack_timer(t + 30 * MS));
    out
}

/// Alarm dışı bütçeli ACK sürekli dolu hatta bugünkü gibi bekler (verinin önceliği; alındı
/// sınırı). Bu test kuralın alarm ACK'iyle sınırlı kaldığını sabitler.
#[test]
fn regular_budgeted_ack_still_waits_for_a_busy_line() {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    for seq in 0..40u64 {
        assert!(
            arrive_busy(&mut r, seq, seq * 100 * MS).is_empty(),
            "alarmsız ACK dolu hatta çıktı ({seq})"
        );
    }
    assert!(r.diag_ack_yield_deferred() > 0);
}

/// Daralma alarmının ACK'i sürekli dolu hatta hattı bir kez bırakır (alarm anındaki boşalma anı
/// = alarm + 1 s), sonra boşalma anını alan datagramın arkasına girer. Düzenek: 1 000 B gövde,
/// 80 ms arayla (12 500 B/s); 30 s'de aralık 160 ms (hat yarıya iner); hat 29 s'den sonra dolu.
#[test]
fn drop_alarm_ack_gives_way_to_a_busy_line_only_once() {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    let drop_at = 30_000 * MS;
    let busy_from = 29_000 * MS;
    let mut t = 1_000 * MS;
    let mut seq = 0u64;
    let mut alarm_at = None;
    let mut ack_after_alarm = None;
    while t < 40_000 * MS && ack_after_alarm.is_none() {
        let before = r.diag_drop_alarms();
        r.set_next_datagram_bytes(1_000);
        let frames = if t >= busy_from {
            arrive_busy(&mut r, seq, t)
        } else {
            r.set_uplink_yield(false);
            r.set_return_line_ahead_ns(0);
            let mut v: Vec<AckFrame> = r
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
            v.extend(r.on_ack_timer(t));
            v
        };
        if alarm_at.is_none() && t >= drop_at && r.diag_drop_alarms() > before {
            alarm_at = Some(t);
        }
        if alarm_at.is_some() && !frames.is_empty() {
            ack_after_alarm = Some(t);
        }
        seq += 1;
        t += if t < drop_at { 80 * MS } else { 160 * MS };
    }
    let alarm = alarm_at.expect("daralma alarmı yok");
    let ack = ack_after_alarm.expect("alarm ACK'i dolu hatta hiç çıkmadı");
    assert!(
        (alarm + AHEAD..=alarm + AHEAD + 200 * MS).contains(&ack),
        "alarm ACK'i bir yol verişten sonra çıkmalı: alarm {} ms, ACK {} ms",
        alarm / MS,
        ack / MS
    );
}
