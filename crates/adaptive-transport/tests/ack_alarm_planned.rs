//! [ALARM ARALIĞI · TUR 3–4 · sahip kuralı 2026-11-23] Alarm ACK'i, yeterli kanıt geldiğinde ya da
//! planlı (bütçe) ACK'in gönderim zamanı geldiğinde, hangisi önceyse çıkar. Yeni datagram gelmezse
//! planlı ACK kendi zamanında ve adayın kuralıyla (aralık bir ACK geriye) çıkar; bekleme düşer.
//!
//! Düzenek `ack_alarm_interval.rs` ile aynı: 1 000 B gövde, 80 ms arayla (12 500 B/s), 30 s'de hat
//! yarıya (160 ms), her beşte biri hatta serileşip düşer (o kolda daralma alarmı, başlangıçtan sonra
//! yalnız bir datagram serileşmişken verilir: alarm 365'te, başlangıç 364). Dönüş 62 B/s, bitmap
//! açık, beyan hat profil.
//!
//! "Planlı ACK zamanı" ölçülür, varsayılmaz: ikiz alıcı aynı akışı alır, yalnız alarmı tetikleyen
//! datagramın boyu verilmez (`set_next_datagram_bytes` çağrılmaz; varış hızı dedektörü o varışı
//! görmez, alarm verilmez). Jeton, yedek ve sessizlik muhasebesi iki alıcıda aynıdır; ikizin ilk
//! ACK'i, alarm olmasaydı çıkacak planlı ACK'tir. Zamanlayıcı her 1 ms'de çağrılır.

use adaptive_transport::ack::{AckFrame, PAIR_RATE_NONE};
use adaptive_transport::arrival_rate::RateDropDetector;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const BODY: u64 = 1_000;
const OLD_GAP: u64 = 80 * MS;
const NEW_GAP: u64 = 160 * MS;
const DROP_AT: u64 = 30 * SEC;
/// Profilin ileri hattı (B/s): düşüş öncesi hat.
const PROFILE_LINE: u64 = BODY * SEC / OLD_GAP;
/// Zamanlayıcı çağrı aralığı.
const POLL: u64 = MS;
/// Alarmdan sonra gözlem süresi: 62 B/s dönüşte bütçe aralığının (≈ 3–5 s) birkaç katı.
const WATCH: u64 = 30 * SEC;

fn every_fifth(seq: u64) -> bool {
    seq % 5 == 3
}

fn receiver() -> Receiver {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(Some(PROFILE_LINE));
    r
}

fn feed(r: &mut Receiver, seq: u64, t: u64, sized: bool) -> Vec<(u64, AckFrame)> {
    if sized {
        r.set_next_datagram_bytes(BODY);
    }
    let mut out: Vec<(u64, AckFrame)> = r
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
        .map(|f| (t, f))
        .collect();
    out.extend(r.on_ack_timer(t).map(|f| (t, f)));
    out
}

/// Akış alarmı tetikleyen datagrama dek (dahil). Dönüş: alıcı, çıkan ACK'ler, alarm datagramı
/// `(seq, an)` ve kanıtın ilk seq'i (daralma başlangıcı). Alarm datagramı bağımsız dedektörle
/// bulunur (alıcının dedektörüyle aynı girdi); alarm anında kanıt yetersiz olmalı (kısa alarm:
/// `AlarmEvidence::sufficient`, tur 4). `twin`: alarm datagramının boyu verilmez.
/// [`until_alarm`] dönüşü: alıcı, çıkan ACK'ler, alarm datagramı `(seq, an)`, kanıtın ilk seq'i.
type UntilAlarm = (Receiver, Vec<(u64, AckFrame)>, (u64, u64), u64);

fn until_alarm(twin: bool) -> UntilAlarm {
    let mut r = receiver();
    let mut det = RateDropDetector::default();
    let mut acks: Vec<(u64, AckFrame)> = Vec::new();
    let mut t = SEC;
    let mut seq = 0u64;
    loop {
        if !every_fifth(seq) {
            let fired = !acks.is_empty() && det.on_arrival_seq(t, BODY, Some(seq));
            let mut first = 0;
            if fired {
                let prev = &acks.last().expect("ACK").1;
                let ev = det
                    .alarm_evidence(u64::from(prev.largest_seq) + 1)
                    .expect("başlangıç");
                assert!(
                    !ev.sufficient(),
                    "alarm kısa değil (seq {seq}, kanıt {:?})",
                    ev.margin_ns()
                );
                first = ev.first_seq();
            }
            let out = feed(&mut r, seq, t, !(twin && fired));
            for &(at, _) in &out {
                det.on_ack(at);
            }
            assert!(
                !fired || out.is_empty(),
                "alarm datagramında ACK çıktı (düzenek planlı ACK'i ayıramaz)"
            );
            acks.extend(out);
            if fired {
                return (r, acks, (seq, t), first);
            }
        }
        seq += 1;
        t += if t < DROP_AT { OLD_GAP } else { NEW_GAP };
        assert!(t < DROP_AT + 10 * SEC, "alarm yok");
    }
}

/// Zamanlayıcıyı `from`'dan (hariç) `to`'ya (dahil) `POLL` adımla çağırır; ilk ACK.
fn poll_first(r: &mut Receiver, from: u64, to: u64) -> Option<(u64, AckFrame)> {
    let mut t = from + POLL;
    while t <= to {
        if let Some(f) = r.on_ack_timer(t) {
            return Some((t, f));
        }
        t += POLL;
    }
    None
}

/// Çiftin alanı dışında bayt bayt aynı mı (alarmda çift ölçümü yeniden başlar; değer ikizde farklı).
fn same_but_pair(a: &AckFrame, b: &AckFrame) -> bool {
    let strip = |f: &AckFrame| {
        let mut f = f.clone();
        if let Some(bm) = f.bitmap.as_mut() {
            bm.pair_rate = PAIR_RATE_NONE;
        }
        f.encode()
    };
    strip(a) == strip(b)
}

/// Bir ACK geriye aralığın başı (adayın kuralı): son ACK'in en büyüğünden sonraki ilk alınan seq.
fn first_received_after(largest: u64) -> u64 {
    (largest + 1..).find(|&s| !every_fifth(s)).expect("seq")
}

/// (a) Kısa alarmdan sonra hiç datagram gelmez: ACK planlı zamanında (ikizle aynı an, geç değil) ve
/// adayın kuralıyla (alarm ACK'i değil; aralık bir ACK geriye, çerçeve ikizinkiyle aynı) çıkar.
#[test]
fn a_no_more_datagrams_the_planned_ack_leaves_on_time_with_the_candidate_rule() {
    let (mut r, acks, (seq, t_alarm), _) = until_alarm(false);
    let (mut twin, twin_acks, _, _) = until_alarm(true);
    assert_eq!(acks.len(), twin_acks.len(), "alarmdan önceki ACK'ler");
    let prev = &acks.last().expect("alarmdan önce ACK").1;
    let planned = poll_first(&mut twin, t_alarm, t_alarm + WATCH).expect("ikizde planlı ACK yok");
    let got = poll_first(&mut r, t_alarm, t_alarm + WATCH);
    println!(
        "alarm {seq} @ {} ms; planlı ACK (ikiz) {} ms (+{} ms); dal {:?}",
        t_alarm / MS,
        planned.0 / MS,
        (planned.0 - t_alarm) / MS,
        got.as_ref()
            .map(|(t, f)| (t / MS, f.span_first_full(u64::from(f.largest_seq) + 1)))
    );
    let (t, f) = got.expect("yeni datagram gelmeyince ACK hiç çıkmadı");
    assert_eq!(t, planned.0, "ACK planlı zamanında değil");
    assert!(
        same_but_pair(&f, &planned.1),
        "çerçeve planlı ACK'ten farklı"
    );
    assert_eq!(
        f.span_first_full(u64::from(f.largest_seq) + 1),
        Some(first_received_after(u64::from(prev.largest_seq))),
        "aralık bir ACK geriye değil"
    );
    assert_eq!(r.diag_drop_alarms(), 0, "planlı ACK alarm kuralıyla çıktı");
}

/// (b) Planlı zaman kanıtlı varıştan önce gelir: planlı ACK adayın kuralıyla zamanında çıkar,
/// bekleme düşer; sonra gelen (yeterli kanıtlı) varış alarm ACK'i doğurmaz.
#[test]
fn b_planned_time_before_the_evidence_drops_the_wait() {
    let (mut r, _, (seq, t_alarm), _) = until_alarm(false);
    let (mut twin, _, _, _) = until_alarm(true);
    let planned = poll_first(&mut twin, t_alarm, t_alarm + WATCH).expect("ikizde planlı ACK yok");
    // Sonraki datagram planlı ACK'ten sonra, yeni hattın aralığıyla.
    let t_next = planned.0 + NEW_GAP;
    let next = seq + 1;
    assert!(!every_fifth(next));
    let got = poll_first(&mut r, t_alarm, t_next - POLL);
    println!(
        "alarm {seq} @ {} ms; planlı ACK {} ms; dal {:?} ms; sonraki varış {next} @ {} ms",
        t_alarm / MS,
        planned.0 / MS,
        got.as_ref().map(|(t, _)| t / MS),
        t_next / MS
    );
    let (t, f) = got.expect("planlı ACK kanıtlı varışı bekledi");
    assert_eq!(t, planned.0, "planlı ACK zamanında değil");
    assert!(
        same_but_pair(&f, &planned.1),
        "çerçeve planlı ACK'ten farklı"
    );
    let after = feed(&mut r, next, t_next, true);
    assert_eq!(
        r.diag_drop_alarms(),
        0,
        "bekleme düşmedi: kanıtlı varış alarm ACK'i doğurdu ({} ACK)",
        after.len()
    );
}

/// (c) Kanıtlı varış planlı zamandan önce gelir (akış sürer): alarm ACK'i o varışta çıkar, planlı
/// zamandan önce; aralığı daralma başlangıcından (kanıtın ilk seq'i).
#[test]
fn c_evidence_before_the_planned_time_sends_the_alarm_ack_at_the_evidence() {
    let (mut r, _, (seq, t_alarm), onset) = until_alarm(false);
    let (mut twin, _, _, _) = until_alarm(true);
    let planned = poll_first(&mut twin, t_alarm, t_alarm + WATCH).expect("ikizde planlı ACK yok");
    let next = seq + 1;
    let t_next = t_alarm + NEW_GAP;
    assert!(!every_fifth(next) && t_next < planned.0);
    let got = poll_first(&mut r, t_alarm, t_next - POLL);
    assert!(
        got.is_none(),
        "kanıttan önce ACK çıktı: {:?}",
        got.map(|g| g.0)
    );
    let out = feed(&mut r, next, t_next, true);
    println!(
        "alarm {seq} @ {} ms; kanıt {next} @ {} ms (+{} ms); planlı {} ms; ACK {:?}",
        t_alarm / MS,
        t_next / MS,
        (t_next - t_alarm) / MS,
        planned.0 / MS,
        out.iter()
            .map(|(t, f)| (t / MS, f.span_first_full(u64::from(f.largest_seq) + 1)))
            .collect::<Vec<_>>()
    );
    let (t, f) = out.first().expect("kanıtlı varışta alarm ACK'i çıkmadı");
    assert_eq!(*t, t_next);
    assert_eq!(r.diag_drop_alarms(), 1, "alarm ACK'i değil");
    let sf = f
        .span_first_full(u64::from(f.largest_seq) + 1)
        .expect("aralık");
    assert_eq!(sf, onset, "aralık başlangıçtan değil");
    assert_eq!(next - sf, 2, "aralık başlangıçtan değil");
}
