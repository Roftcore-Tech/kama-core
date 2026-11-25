//! [K2 · "çift kanıtı gelmedi" · kart 7175576090] Alıcı tarafında çift ölçüsünün bugünkü davranışı:
//! ürünün sunucu yuvası gibi kurulmuş `Receiver` (`starved_line_sim.rs` `RxModel::Product`: kota 4,
//! dönüş 62 B/s, yeniden rapor kapalı, bitmap, `set_pair_squeeze_line(12 500)`) beş varış dizisi
//! alır; her dizide çıkan ACK'lerin `pair_rate` alanı (UDP gövde B/s; göndericinin tel çevirisi
//! × 1 228 / 1 200) yazılır. Rapor testidir: assert'ler BUGÜNKÜ davranışı sabitler, eşik önermez
//! (sıkışma eşiği 0,9; sahip 7175498704). Benzetim/birim testi, kabul değil.
//!
//! Boylar (IP): tam datagram 1 074 B (UDP 1 046), küçük 136 B (UDP 108). Dizilerden önce aynı hatta
//! ısınma: duran kuyrukla 12 tam datagram ve ilk (muaf) ACK.

use adaptive_transport::ack::{AckFrame, PAIR_RATE_NONE};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const FULL: u64 = 1_046;
const SMALL: u64 = 108;
const OVH: u64 = 28;
const DECLARED: u64 = 12_500;

fn product_rx() -> Receiver {
    let mut r = Receiver::with_cc_port();
    r.set_quota_r(4);
    r.set_return_bandwidth(Some(62));
    r.set_ack_rereport(false);
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(Some(DECLARED));
    r.set_ack_bitmap_ptt_lean(false);
    r
}

/// Tel serileşmesi (ns): `udp` gövdeli datagram `line` B/s hatta (IP sayımı).
fn ser(udp: u64, line: u64) -> u64 {
    (udp + OVH) * SEC / line
}

#[derive(Debug, Default)]
struct Outcome {
    /// Dizi sırasında ve sonunda çıkan ACK'ler: (an ms, alan).
    fields: Vec<(u64, u16)>,
    drop_alarms: u64,
    rise_alarms: u64,
    /// `PairRate::on_arrival` kurallarının aynası: (sayılan, sıkışmış) çift.
    counted: u32,
    squeezed: u32,
}

/// `warm_line`: ısınma hattı (duran kuyruk). `seq`: (önceki varıştan aralık ns, UDP gövdesi).
fn run(warm_line: u64, seq: &[(u64, u64)]) -> Outcome {
    let mut r = product_rx();
    let mut o = Outcome::default();
    let mut t = SEC;
    let mut s = 0u64;
    let mut prev: Option<(u64, u64)> = None;
    let mut after = false;
    let mut feed = |r: &mut Receiver, t: u64, b: u64, o: &mut Outcome, count: bool| {
        if count {
            if let Some((pt, _)) = prev {
                let g = t - pt;
                let sq = u128::from(g) * 1_000 * u128::from(DECLARED)
                    < 900 * u128::from(b + OVH) * u128::from(SEC);
                if !(sq || after) && b >= 800 && g >= 2 * MS {
                    o.counted += 1;
                }
                if sq {
                    o.squeezed += 1;
                }
                after = sq;
            }
        }
        prev = Some((t, b));
        r.set_next_datagram_bytes(b);
        let out = r.on_datagram(
            RxDatagram {
                seq: s,
                unit_id: s,
                kind: RxKind::Data,
            },
            t,
        );
        s += 1;
        out.ack
    };
    let push = |o: &mut Outcome, t: u64, f: Option<AckFrame>| {
        if let Some(f) = f {
            o.fields.push((
                t / MS,
                f.bitmap.as_ref().map_or(PAIR_RATE_NONE, |b| b.pair_rate),
            ));
        }
    };
    // Isınma + ilk ACK (dizinin alanına girmez).
    for _ in 0..12 {
        let _ = feed(&mut r, t, FULL, &mut o, false);
        t += ser(FULL, warm_line);
    }
    for k in 0..200 {
        if r.on_ack_timer(t + k * 20 * MS).is_some() {
            break;
        }
    }
    let (d0, r0) = (r.diag_drop_alarms(), r.diag_rise_alarms());
    for &(gap, b) in seq {
        t += gap;
        let a = feed(&mut r, t, b, &mut o, true);
        push(&mut o, t, a);
    }
    // Dizi bitti: alan bir sonraki ACK'te (bütçe dolana dek zamanlayıcı).
    for k in 1..=1_000 {
        let at = t + k * 20 * MS;
        if let Some(f) = r.on_ack_timer(at) {
            push(&mut o, at, Some(f));
            break;
        }
    }
    o.drop_alarms = r.diag_drop_alarms() - d0;
    o.rise_alarms = r.diag_rise_alarms() - r0;
    o
}

fn report(name: &str, o: &Outcome) {
    let tel: Vec<String> = o
        .fields
        .iter()
        .map(|&(at, f)| {
            if f == PAIR_RATE_NONE {
                format!("{at} ms: YOK")
            } else {
                format!("{at} ms: {f} (tel {})", u64::from(f) * 1_228 / 1_200)
            }
        })
        .collect();
    println!(
        "{name}: sayılan {} sıkışmış {} | daralma alarmı {} yükseliş alarmı {} | ACK alanları [{}]",
        o.counted,
        o.squeezed,
        o.drop_alarms,
        o.rise_alarms,
        tel.join("; ")
    );
}

/// Gönderici tavanı ≈ 6 223 tel B/s'de tek datagram aralığı.
fn single_gap() -> u64 {
    ser(FULL, 6_223)
}

/// Göndericinin çift düzeni: tek datagramlar `single_gap` arayla, ≈ 500 ms'de bir çift (ikincisi
/// `pair_gap` sonra).
fn paired(pair_gap: u64, n_pairs: usize) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    for _ in 0..n_pairs {
        v.push((single_gap(), FULL));
        v.push((single_gap(), FULL));
        v.push((pair_gap, FULL));
    }
    v
}

fn last_field(o: &Outcome) -> u16 {
    o.fields.last().map_or(PAIR_RATE_NONE, |&(_, f)| f)
}

/// 1 · Toparlanmış 12 500 hat, dolu kova: çiftler 75,84 ms (beyan hatta eşik 77,3 ms → sıkışma).
#[test]
fn d1_recovered_12500_full_bucket_pairs() {
    let o = run(12_500, &paired(75_840_000, 8));
    report("1 · 12 500, dolu kova, tam→tam 75,84 ms", &o);
    // Bugünkü davranış: çiftler sıkışma sayılır; alan tek datagram aralığını (≈ gönderici hızı) taşır.
    let f = u64::from(last_field(&o));
    assert!(o.squeezed >= 8, "çiftler sıkışma sayılmadı");
    assert!(f != u64::from(PAIR_RATE_NONE) && f < 7_000, "alan {f}");
}

/// 2 · Aynı hat, dönüş anında 3 tam datagramlık duran kuyruk: 85,92 ms aralıklı art arda çıkış.
#[test]
fn d2_recovered_12500_standing_queue() {
    let q = ser(FULL, 12_500);
    let mut v = vec![(q, FULL), (q, FULL), (q, FULL)];
    v.extend(paired(75_840_000, 2));
    let o = run(6_250, &v);
    report("2 · 12 500, 3 datagramlık kuyruk, 85,92 ms", &o);
    let f = u64::from(last_field(&o));
    assert!((12_100..=12_499).contains(&f), "alan {f}");
}

/// 3 · Hat 6 250, dolu kova: tam → tam 151,7 ms.
#[test]
fn d3_6250_full_bucket_pairs() {
    let mut v = Vec::new();
    for _ in 0..8 {
        v.push((ser(FULL, 6_250), FULL));
        v.push((ser(FULL, 6_250), FULL));
        v.push((151_700_000, FULL));
    }
    let o = run(6_250, &v);
    report("3 · 6 250, dolu kova, tam→tam 151,7 ms", &o);
    let f = u64::from(last_field(&o));
    assert!((6_800..=7_000).contains(&f), "alan {f}");
}

/// 4 · Hat 6 250: küçük (136 B IP) → tam, 85,5 ms (kova kredisi; sahte yükseliş kanıtı).
#[test]
fn d4_6250_small_then_full() {
    let mut v = Vec::new();
    for _ in 0..6 {
        v.push((ser(FULL, 6_250), FULL));
        v.push((ser(FULL, 6_250), FULL));
        v.push((ser(FULL, 6_250), SMALL));
        v.push((85_500_000, FULL));
    }
    let o = run(6_250, &v);
    report("4 · 6 250, küçük→tam 85,5 ms", &o);
    let f = u64::from(last_field(&o));
    assert!((12_200..=12_499).contains(&f), "alan {f}");
}

/// 5 · Dizi 1'in ortasına metin kümesi (136 B × 4, 22 ms aralıklı): daralma dedektörü algılar mı,
/// saklanan çift değeri silinir mi (Bulut-A 7175343023).
#[test]
fn d5_text_cluster_inside_recovered_pairs() {
    let q = ser(FULL, 12_500);
    let mut v = vec![(q, FULL), (q, FULL), (q, FULL)];
    v.extend(paired(75_840_000, 2));
    v.extend([
        (22 * MS, SMALL),
        (22 * MS, SMALL),
        (22 * MS, SMALL),
        (22 * MS, SMALL),
    ]);
    v.extend(paired(75_840_000, 4));
    let o = run(6_250, &v);
    report("5 · dizi 2 + metin kümesi 4 × 136 B / 22 ms + çiftler", &o);
    // Denetim kolu: aynı dizi, metin kümesi yok (küme yerine aynı süre boşluk).
    let mut c = vec![(q, FULL), (q, FULL), (q, FULL)];
    c.extend(paired(75_840_000, 2));
    let mut rest = paired(75_840_000, 4);
    rest[0].0 += 88 * MS;
    c.extend(rest);
    let oc = run(6_250, &c);
    report("5d · denetim: aynı dizi, metin kümesi yok", &oc);
    // Bugünkü davranış sabitlenir (sayı raporda); yalnız dizinin işlendiğini doğrular.
    assert!(!o.fields.is_empty(), "ACK yok");
}
