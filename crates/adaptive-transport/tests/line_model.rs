//! [HAT-MODELİ] Bilinen yukarı yön hattında kendi datagramlarımızın serileşmesi.
//!
//! Kök vaka (ters oturum, tests/shaped_relay.rs taklit dönüş mesajı ölçümü): alıcı düğümün ters
//! oturumu 62 B/s yukarı yönde 37 B'lik kısa mesajlar gönderir. Port pacer'ı ilk RTT örneğinden önce
//! 1,25·pencere/100 ms hızla ve en az 10·MTU burst'le çalıştığından bütün mesaj akışı emit anında
//! darboğaz kuyruğuna iner; PreConverge kopya saatleri (§3.3-B merdiveni, DR-EST-1 kuruluş denemesi)
//! emit anından başladığı için kopyalar orijinaller hattan çıkmadan ateşlenir ve aynı kuyruğa girer.
//! Bu pinler: (1) hat modeli datagramı (gövde + zarf) hattın hızında serileştirir, model yokken kova
//! aynendir; (2) ters oturum simülasyonunda dönüş paketleri mesaj sayısını en çok kuruluş denemesi
//! payı kadar aşar ve her mesaj FIFO hattın serileşme sınırında teslim edilir.

use adaptive_transport::pacer::{LineModel, TokenBucket};
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use std::collections::VecDeque;
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;

/// Saha hattı: 62 B/s yukarı yön, UDP/IP zarfı 28 B.
const NARROW: LineModel = LineModel {
    rate_bps: 62,
    datagram_overhead: 28,
};

/// `bytes` baytın `rate` B/s hatta serileşme süresi (ns, yukarı yuvarlanır).
fn ser_ns(bytes: u64, rate: u64) -> u64 {
    (bytes * SEC).div_ceil(rate)
}

#[test]
fn line_holds_the_next_datagram_until_the_previous_one_serialized() {
    let mut b = TokenBucket::new(1_000_000, 12_000);
    b.set_line(Some(NARROW));
    assert!(b.try_send(37), "boş hat ilk datagramı alır");
    // Kovada 11 963 B jeton var; hat 37 + 28 = 65 B'yi 62 B/s'de serileştirirken ikinci datagram çıkmaz.
    assert!(!b.try_send(37), "hat meşgulken kova datagram vermez");
    let t = ser_ns(65, 62);
    assert_eq!(
        b.ns_until_sendable(37),
        Some(t),
        "bekleme = zarf dahil serileşme"
    );
    assert_eq!(b.line_ns_until_drained(), t);
    b.refill(Duration::from_nanos(t - 1));
    assert!(!b.line_is_open(), "serileşme bitmeden hat kapalı");
    b.refill(Duration::from_nanos(1));
    assert!(b.line_is_open(), "serileşme bitince hat açık");
    assert_eq!(b.line_ns_until_drained(), 0);
    assert!(b.try_send(37));
}

#[test]
fn line_counts_the_datagram_envelope_and_idle_line_keeps_no_credit() {
    let mut b = TokenBucket::new(1_000_000, 12_000);
    b.set_line(Some(NARROW));
    for _ in 0..10 {
        b.line_charge(37);
    }
    // 10 × (37 + 28) B; zarfsız sayım 10 × 37 B'yi 6 s'de bitmiş sanardı.
    assert_eq!(b.line_ns_until_drained(), ser_ns(650, 62));
    // Uzun boşluk hatta kredi biriktirmez: boşalan hat bir sonraki datagramı yine tam serileştirir.
    b.refill(Duration::from_secs(60));
    assert!(b.try_send(37));
    assert!(!b.try_send(37));
    assert_eq!(b.line_ns_until_drained(), ser_ns(65, 62));
}

#[test]
fn fast_line_admits_a_burst_interval_of_datagrams() {
    // 12,5 MB/s hatta bir burst aralığı (2 ms) 25 000 B: zamanlayıcı çözünürlüğü hattı boşta bırakmaz.
    let mut b = TokenBucket::new(100_000_000, 256 * 1200);
    b.set_line(Some(LineModel {
        rate_bps: 12_500_000,
        datagram_overhead: 28,
    }));
    let mut n = 0;
    while b.try_send(1200) {
        n += 1;
    }
    assert_eq!(n, 21, "kuyruk 25 000 B'yi aşana dek (21 × 1 228 B)");
}

#[test]
fn line_none_is_the_plain_bucket() {
    let mut a = TokenBucket::new(31, 2400);
    let mut b = TokenBucket::new(31, 2400);
    b.set_line(None);
    b.set_line(Some(LineModel {
        rate_bps: 0,
        datagram_overhead: 28,
    })); // hız 0 = model yok
    for i in 0..400u64 {
        let size = 37 + (i % 7) * 100;
        assert_eq!(a.try_send(size), b.try_send(size), "adım {i}");
        assert_eq!(
            a.ns_until_sendable(size),
            b.ns_until_sendable(size),
            "adım {i}"
        );
        let dt = Duration::from_millis(1 + i % 13);
        a.refill(dt);
        b.refill(dt);
        assert_eq!(a.tokens(), b.tokens(), "adım {i}");
    }
    assert!(b.line_is_open());
    assert_eq!(b.line_ns_until_open(), 0);
    assert_eq!(b.line_ns_until_drained(), 0);
}

/// Ters oturum simülasyonunun sonucu.
struct Outcome {
    /// Tele çıkan veri taşıyan datagram (her tür).
    datagrams: usize,
    /// Bunların kopya / yeniden iletim olanları.
    extra: usize,
    /// Birim başına ilk teslim anı (ns).
    delivered: Vec<Option<u64>>,
}

/// 40 × 37 B mesaj 100 ms arayla sunulur (sunum 62 B/s hattın taşıyabileceğinin ≈ 10 katı). Yukarı yön
/// FIFO, `rate` B/s, datagram başına 28 B zarf, tek yön 300 ms, kayıpsız; alıcı her varışı 25 ms
/// sonra ACK'ler, ACK geniş ileri yoldan (12,5 kB/s, 300 ms) döner. Motor runtime gibi 1 ms'de bir
/// pompalanır.
fn reverse_session(line: Option<LineModel>) -> Outcome {
    const N: u64 = 40;
    const SIZE: u64 = 37;
    const RATE: u64 = 62;
    const ONE_WAY: u64 = 300 * MS;
    const ACK_DELAY: u64 = 25 * MS;
    let mut s = Sender::with_cc_port();
    s.set_cc_initial_window(4 * 1200);
    s.set_cc_line(line);
    let mut line_free = 0u64;
    // (varış, seq, birim) — FIFO hat sırayla teslim eder.
    let mut wire: VecDeque<(u64, u64, u64)> = VecDeque::new();
    // Alıcı: henüz ACK'lenmemiş seq'ler ve bekleyen ACK'in gönderim anı.
    let mut unacked: Vec<u64> = Vec::new();
    let mut ack_at: Option<u64> = None;
    // Göndericiye varacak ACK'ler (varış, seq'ler).
    let mut acks: VecDeque<(u64, Vec<u64>)> = VecDeque::new();
    let mut out = Outcome {
        datagrams: 0,
        extra: 0,
        delivered: vec![None; N as usize],
    };
    let mut next_unit = 0u64;
    let mut done_at: Option<u64> = None;
    let mut t = 0u64;
    while t <= 150 * SEC {
        let mut emitted = Vec::new();
        while next_unit < N && next_unit * 100 * MS <= t {
            emitted.extend(s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_unit,
                    size: SIZE,
                }),
                t,
            ));
            next_unit += 1;
        }
        while acks.front().is_some_and(|(at, _)| *at <= t) {
            let (_, seqs) = acks.pop_front().unwrap();
            emitted.extend(s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: Duration::from_nanos(ACK_DELAY),
                }),
                t,
            ));
        }
        emitted.extend(s.on_event(Event::TimerFired(Timer::Pto), t));
        for d in &emitted {
            out.datagrams += 1;
            if d.kind != TransmitKind::Data {
                out.extra += 1;
            }
            let start = line_free.max(t);
            line_free = start + ser_ns(d.size + 28, RATE);
            wire.push_back((line_free + ONE_WAY, d.seq, d.unit_id));
        }
        while wire.front().is_some_and(|(at, _, _)| *at <= t) {
            let (at, seq, unit) = wire.pop_front().unwrap();
            let slot = &mut out.delivered[unit as usize];
            if slot.is_none() {
                *slot = Some(at);
            }
            unacked.push(seq);
            ack_at.get_or_insert(at + ACK_DELAY);
        }
        if ack_at.is_some_and(|a| a <= t) {
            let sent = ack_at.take().unwrap();
            // 59 B ACK datagramı 12,5 kB/s ileri yolda + tek yön gecikme.
            acks.push_back((
                sent + ser_ns(59, 12_500) + ONE_WAY,
                std::mem::take(&mut unacked),
            ));
        }
        if done_at.is_none() && out.delivered.iter().all(Option::is_some) {
            done_at = Some(t);
        }
        if done_at.is_some_and(|d| t > d + 20 * SEC) {
            break;
        }
        t += MS;
    }
    out
}

#[test]
fn reverse_session_short_messages_do_not_flood_a_narrow_uplink() {
    let o = reverse_session(Some(NARROW));
    let free = reverse_session(None);
    eprintln!(
        "[hat-modeli] ters oturum 40 × 37 B, 62 B/s: modelle {} datagram ({} fazla), modelsiz {} ({} fazla)",
        o.datagrams, o.extra, free.datagrams, free.extra
    );
    assert!(
        o.delivered.iter().all(Option::is_some),
        "40/40 teslim: {:?}",
        o.delivered
    );
    // Fazla datagram yalnız DR-EST-1 kuruluş denemesi payı: deneme orijinal hattı terk ettikten
    // 0,5 s sonra çıkar, 600 ms gidiş-dönüşlü yolda ilk ACK'ten önce bir kez ateşlenebilir.
    assert!(
        o.extra <= 2,
        "dönüş {} datagram / 40 mesaj ({} kopya) > 2 kopya payı",
        o.datagrams,
        o.extra
    );
    // Teslim gecikmesi FIFO hattın serileşme sınırında: k. mesajın önünde en çok k orijinal ve iki
    // kopya serileşir (her biri 65 B / 62 B/s), ardından tek yön gecikme ve pompa adımı.
    let slot = ser_ns(65, 62);
    for (k, at) in o.delivered.iter().enumerate() {
        let bound = (k as u64 + 3) * slot + 300 * MS + 2 * MS;
        let at = at.unwrap();
        assert!(
            at <= bound,
            "mesaj {k}: teslim {:.2} s > FIFO sınırı {:.2} s",
            at as f64 / SEC as f64,
            bound as f64 / SEC as f64
        );
    }
}
