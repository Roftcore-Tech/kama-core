//! [KUYRUK İZİ · KANITSIZ ALARM · #175 7089430406] Daralma alarmının iki kusuru, belirlenimli
//! girişle (yol=bitmap aday, ürün varsayılanı V2).
//!
//! 1. **Sahte alarm.** Kopyalı PTT'de gönderici boşken yalnız kopyalarla gelen akış düzenlilik
//!    denetiminden geçiyor (`arrival_rate.rs` `max_gap·2 ≤ mean_gap·5`). Dürüst hatta rastgele
//!    kayıp pencere hızını düşürüyor (alınmayan datagramın aralığı sayılıyor, baytı sayılmıyor).
//!    Oturum başında başlangıç penceresi bitince gönderici duruyor; dedektörün referansı o
//!    pencerenin varışlarından kurulu. Benzetimde (`starved_line_sim.rs`, ürün alıcısı, anahtarlı
//!    kayıp, aday `86c5602c`) 30 koşunun 30'unda alarm 0,99–1,08 s'de çıktı. Dürüst sabit 6 250
//!    hatta 4 / 4 / 4 alarm oldu (tohum 1–3).
//! 2. **Susan alarm ACK'i.** Alarm anında daralma başlangıcı (`RateDropDetector::onset`)
//!    bulunamazsa kanıt kurulamıyor ve ACK planlı kadansa kalıyor. Alınmayan seq `DEFAULT_MTU`
//!    üst sınırıyla sayılıyor; kopyalı yükte en yeni çift bu yüzden eşiğin üstünde okunuyor.
//!
//! Düzenek: gönderici açık döngüdür. Dosya 1 000 B, profil hattında (12 500 tel B/s) pace edilir.
//! PTT ya kopyalı (86 B, 200 ms'de 6 kopya, 200/7 ms arayla) ya da yalın (78 B, 200 ms'de bir).
//! Darboğaz tek FIFO'dur (64 datagram, hız çizelgesi, 300 ms yayılım). Kayıp serileşmeden sonradır
//! (tohumlu LCG, ‰200).
//!
//! Adayın kuralı bu dosyada kopya olarak durur ([`CandidateRule`], `86c5602c` `on_arrival_seq`'in
//! karar gövdesi). Her kolda önce kopyanın tetiklendiği sınanır (ön koşul: girdi adayda sahte alarm
//! üretir). Gerçek düşüş kollarında ürün dedektörü kopyayla aynı varışta tetiklenmelidir
//! (algılama gecikmez).
//!
//! Karşı-olgu (adayın `arrival_rate.rs` ve `receiver.rs` dosyalarıyla aynı test gövdesi): (a1)
//! (a2) (a3) (b) KIRMIZI; gerçek düşüş kollarında ürün dedektörü kopyayla her varışta aynı karar.

use std::collections::VecDeque;

use adaptive_transport::arrival_rate::{
    RateDropDetector, DROP_MIN_ARRIVALS, DROP_PERMILLE, DROP_REF_HOLD_NS, DROP_WINDOW_NS,
};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const OVH: u64 = 28;
const PROFILE: u64 = 12_500;
const FILE_UDP: u64 = 1_000;
const COPY_UDP: u64 = 86;
const LEAN_UDP: u64 = 78;
const ONE_WAY: u64 = 300 * MS;
const FIFO: usize = 64;

/// Adayın (`86c5602c`) `RateDropDetector::on_arrival_seq` karar gövdesinin kopyası.
#[derive(Default)]
struct CandidateRule {
    ring: VecDeque<(u64, u64)>,
    reference_bps: u64,
    reference_ns: u64,
    fired: bool,
}

impl CandidateRule {
    fn on_ack(&mut self, now: u64) {
        if self.fired {
            self.fired = false;
            self.reference_bps = 0;
            self.reference_ns = now;
        }
    }

    fn on_arrival(&mut self, now: u64, udp: u64) -> bool {
        self.ring.push_back((now, udp));
        while self
            .ring
            .front()
            .is_some_and(|&(t, _)| t.saturating_add(DROP_WINDOW_NS) < now)
        {
            self.ring.pop_front();
        }
        if self.ring.len() < DROP_MIN_ARRIVALS {
            return false;
        }
        let first = self.ring.front().map_or(now, |&(t, _)| t);
        let span = now.saturating_sub(first);
        if span < DROP_WINDOW_NS / 2 {
            return false;
        }
        let n = self.ring.len() as u64;
        let mean_gap = span / (n - 1);
        let max_gap = self
            .ring
            .iter()
            .zip(self.ring.iter().skip(1))
            .map(|(a, b)| b.0.saturating_sub(a.0))
            .max()
            .unwrap_or(0);
        if max_gap.saturating_mul(2) > mean_gap.saturating_mul(5) {
            return false;
        }
        let bytes: u64 = self.ring.iter().skip(1).map(|&(_, b)| b).sum();
        let rate = u64::try_from(u128::from(bytes) * u128::from(SEC) / u128::from(span.max(1)))
            .unwrap_or(u64::MAX);
        if rate >= self.reference_bps || now.saturating_sub(self.reference_ns) > DROP_REF_HOLD_NS {
            if rate >= self.reference_bps || !self.fired {
                self.reference_bps = rate;
                self.reference_ns = now;
            }
            return false;
        }
        if !self.fired
            && rate.saturating_mul(1000) < self.reference_bps.saturating_mul(DROP_PERMILLE)
        {
            self.fired = true;
            return true;
        }
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ptt {
    Copied,
    Lean,
}

/// Akış: dosya `[file_from, file_until)` profil hızında pace edilir; PTT 0,5 s'den `until`'e.
struct Flow {
    file_from: u64,
    file_until: u64,
    ptt: Ptt,
    until: u64,
    /// İleri hat: (bu andan itibaren, tel B/s).
    line: Vec<(u64, u64)>,
    /// Serileşmeden sonra kayıp (‰; LCG tohumu).
    loss: Option<(u64, u64)>,
}

/// Darboğazdan çıkan varışlar `(an, gövde, seq)`, varış sırasıyla.
fn arrivals(f: &Flow) -> Vec<(u64, u64, u64)> {
    let mut sends: Vec<(u64, u64)> = Vec::new();
    let gap = (FILE_UDP + OVH) * SEC / PROFILE;
    let mut t = f.file_from;
    while t < f.file_until {
        sends.push((t, FILE_UDP));
        t += gap;
    }
    let mut b = 500 * MS;
    while b < f.until {
        match f.ptt {
            Ptt::Copied => {
                for k in 0..6u64 {
                    sends.push((b + k * 200 * MS / 7, COPY_UDP));
                }
            }
            Ptt::Lean => sends.push((b, LEAN_UDP)),
        }
        b += 200 * MS;
    }
    sends.sort_unstable();
    let rate_at = |t: u64| {
        f.line
            .iter()
            .rev()
            .find(|&&(from, _)| from <= t)
            .map_or(f.line[0].1, |&(_, r)| r)
    };
    let mut lcg = f.loss.map_or(0, |(seed, _)| seed);
    let mut queue: VecDeque<u64> = VecDeque::new();
    let mut free = 0u64;
    let mut out = Vec::new();
    for (seq, &(at, udp)) in sends.iter().enumerate() {
        while queue.front().is_some_and(|&d| d <= at) {
            queue.pop_front();
        }
        if queue.len() >= FIFO {
            continue;
        }
        let start = free.max(at);
        let done = start + (udp + OVH) * SEC / rate_at(start);
        free = done;
        queue.push_back(done);
        if let Some((_, permille)) = f.loss {
            lcg = lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            if (lcg >> 33) % 1000 < permille {
                continue;
            }
        }
        out.push((done + ONE_WAY, udp, seq as u64));
    }
    out.sort_unstable();
    out
}

/// Dedektörün ACK kadansı (test girdisi): her [`ACK_EVERY`]'de bir `on_ack` (alarmdan sonra
/// sonraki alarm yeniden mümkün; adayın kuralında da aynı).
const ACK_EVERY: u64 = SEC;

/// Dedektör ve adayın kuralı aynı varışlarla: her biri için tetiklenme anları.
fn fires(arr: &[(u64, u64, u64)]) -> (Vec<u64>, Vec<u64>) {
    let (mut det, mut cand) = (RateDropDetector::default(), CandidateRule::default());
    let (mut d, mut c) = (Vec::new(), Vec::new());
    let mut next_ack = ACK_EVERY;
    for &(t, udp, seq) in arr {
        while next_ack <= t {
            det.on_ack(next_ack);
            cand.on_ack(next_ack);
            next_ack += ACK_EVERY;
        }
        if det.on_arrival_seq(t, udp, Some(seq)) {
            d.push(t);
        }
        if cand.on_arrival(t, udp) {
            c.push(t);
        }
    }
    (d, c)
}

fn secs(v: &[u64]) -> Vec<String> {
    v.iter()
        .map(|&t| format!("{:.3}", t as f64 / SEC as f64))
        .collect()
}

/// (a1) Kopyalı PTT, gönderici dosyayı bitirip boşta: dürüst 12 500 hatta alarm yok. Benzetimde
/// oturum başı deseni (1,048 s halkası): iki 1 000 B varıştan sonra yalnız 86 B kopyalar, aralıklar
/// 16–41 ms. Burada dosya 0,2–1,0 s'de, kopyalar 3 s'ye dek. İkiz: aynı akış, dosya 3 s'ye dek
/// sürer ve hat 0,8 s'de 6 250'ye iner. Ürün dedektörü adayın kuralıyla aynı varışta tetiklenir.
#[test]
fn a1_copied_ptt_after_the_sender_stops_is_not_a_drop() {
    let idle = Flow {
        file_from: 200 * MS,
        file_until: SEC,
        ptt: Ptt::Copied,
        until: 3 * SEC,
        line: vec![(0, PROFILE)],
        loss: None,
    };
    let (d, c) = fires(&arrivals(&idle));
    println!("(a1) boşta: dal {:?}, aday {:?}", secs(&d), secs(&c));
    assert!(!c.is_empty(), "ön koşul: adayın kuralı sahte alarm vermedi");
    assert!(
        d.is_empty(),
        "(a1) gönderici boştayken alarm: {:?}",
        secs(&d)
    );
    let drop = Flow {
        file_until: 3 * SEC,
        line: vec![(0, PROFILE), (800 * MS, PROFILE / 2)],
        ..idle
    };
    let (d, c) = fires(&arrivals(&drop));
    println!("(a1) gerçek düşüş: dal {:?}, aday {:?}", secs(&d), secs(&c));
    assert!(
        !c.is_empty() && d.first() == c.first(),
        "(a1) gerçek düşüş: dal {:?} aday {:?}",
        secs(&d),
        secs(&c)
    );
}

/// (a2) Dürüst sabit 6 250 hat, ‰200 rastgele kayıp oturum başından, gönderici dosyayla tavanda
/// (kuyruk dolu): alarm yok. Ardışık seq'li çiftler hattın hızını gösterir. Üç tohum. İkiz: hat
/// 12 500'den 30 s'de 6 250'ye iner, aynı kayıp; ürün dedektörü adayınkiyle aynı varışta
/// tetiklenir.
#[test]
fn a2_random_loss_on_an_honest_busy_line_is_not_a_drop() {
    let mut premise = 0;
    for seed in 1..=3u64 {
        let honest = Flow {
            file_from: 200 * MS,
            file_until: 60 * SEC,
            ptt: Ptt::Copied,
            until: 60 * SEC,
            line: vec![(0, PROFILE / 2)],
            loss: Some((seed, 200)),
        };
        let (d, c) = fires(&arrivals(&honest));
        println!(
            "(a2) tohum {seed} dürüst: dal {:?}, aday {:?}",
            secs(&d),
            secs(&c)
        );
        premise += c.len();
        assert!(
            d.is_empty(),
            "(a2) tohum {seed}: dürüst hatta alarm {:?}",
            secs(&d)
        );
        let drop = Flow {
            line: vec![(0, PROFILE), (30 * SEC, PROFILE / 2)],
            ..honest
        };
        let (d, c) = fires(&arrivals(&drop));
        let after = |v: &[u64]| v.iter().copied().find(|&t| t >= 30 * SEC);
        println!(
            "(a2) tohum {seed} gerçek düşüş: dal {:?}, aday {:?}",
            secs(&d),
            secs(&c)
        );
        let (da, ca) = (after(&d), after(&c));
        assert!(
            da.is_some() && (ca.is_none() || da <= ca),
            "(a2) tohum {seed}: düşüş algılaması dal {da:?} aday {ca:?}"
        );
    }
    assert!(premise > 0, "ön koşul: adayın kuralı sahte alarm vermedi");
}

/// Ürün alıcısının kurulumu (sunucu yuvası, 62 B/s dönüş, bitmap açık, beyan hat profil).
fn receiver() -> Receiver {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(Some(PROFILE));
    r
}

/// Alıcıyı varışlarla sürer (zamanlayıcı her 1 ms, dönüş hattı açık). Döner: (ACK anları,
/// her ACK'te alarm sayacı, alarm sayacının arttığı varışların anı).
fn drive(arr: &[(u64, u64, u64)], until: u64) -> (Vec<(u64, u64)>, Vec<u64>) {
    let mut r = receiver();
    let mut acks = Vec::new();
    let mut released = Vec::new();
    let mut i = 0;
    let mut now = 0;
    while now < until {
        while i < arr.len() && arr[i].0 <= now {
            let (_, udp, seq) = arr[i];
            r.set_next_datagram_bytes(udp);
            let before = r.diag_drop_alarms();
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: seq,
                    kind: RxKind::Data,
                },
                now,
            );
            if r.diag_drop_alarms() > before {
                released.push(now);
            }
            if out.ack.is_some() {
                acks.push((now, r.diag_drop_alarms()));
            }
            i += 1;
        }
        if r.on_ack_timer(now).is_some() {
            acks.push((now, r.diag_drop_alarms()));
        }
        now += MS;
    }
    (acks, released)
}

/// Varışları 1 ms'ye yuvarlar (alıcı 1 ms adımla sürülür).
fn on_ms(arr: Vec<(u64, u64, u64)>) -> Vec<(u64, u64, u64)> {
    arr.into_iter()
        .map(|(t, u, s)| (t.div_ceil(MS) * MS, u, s))
        .collect()
}

/// (a3) Yalın PTT, oturum başı: başlangıç penceresi bitince gönderici duruyor, yalnız 200 ms'de
/// bir 78 B PTT geliyor. Benzetimde (yalın kol) alarm 1,009 s'de, bırakma 1,21–1,41 s'de, alarm
/// ACK'i 1,562 s'de. Koordinatörün ürün izinde alarm 1,0–1,8 s'de ve ACK PARAM_ACK'in önüne
/// geçiyordu. Gönderici ilk ACK'i almadan durur; bu hattın değil göndericinin sınırıdır. İlk ACK'in
/// göndericiye en erken varışından önce dedektör çalışmaz, alarm ACK'i yok. İkiz: dosya 6 s'ye dek
/// sürer, hat 3 s'de 6 250'ye iner. Gerçek düşüşün alarm ACK'i çıkar.
#[test]
fn a3_lean_session_start_sender_stop_is_not_a_drop() {
    let idle = Flow {
        file_from: 300 * MS,
        file_until: 800 * MS,
        ptt: Ptt::Lean,
        until: 3 * SEC,
        line: vec![(0, PROFILE)],
        loss: None,
    };
    let arr = on_ms(arrivals(&idle));
    let (_, c) = fires(&arr);
    let (acks, released) = drive(&arr, 3 * SEC);
    println!(
        "(a3) boşta: aday kuralı {:?}; alarm bırakma {:?}; ACK {:?}",
        secs(&c),
        secs(&released),
        acks.iter().map(|a| a.0 / MS).collect::<Vec<_>>()
    );
    assert!(!c.is_empty(), "ön koşul: adayın kuralı sahte alarm vermedi");
    assert!(
        released.is_empty(),
        "(a3) oturum başında alarm ACK'i {:?}",
        secs(&released)
    );
    let drop = Flow {
        file_until: 6 * SEC,
        until: 6 * SEC,
        line: vec![(0, PROFILE), (3 * SEC, PROFILE / 2)],
        ..idle
    };
    let arr = on_ms(arrivals(&drop));
    let (acks, released) = drive(&arr, 6 * SEC);
    println!(
        "(a3) gerçek düşüş: alarm bırakma {:?}; ACK {:?}",
        secs(&released),
        acks.iter().map(|a| a.0 / MS).collect::<Vec<_>>()
    );
    assert!(
        released.iter().any(|&t| t >= 3 * SEC),
        "(a3) gerçek düşüşün alarm ACK'i yok"
    );
}

/// (b) [SAHİP KARARI GEREKİR] Kopyalı PTT, gerçek düşüş (12 500 → 6 250, 8 s), ‰200 kayıp: alarm
/// anında daralma başlangıcı bulunamıyorsa (`onset` `None`) alarm ACK'i alarmı veren varışta
/// adayın kuralıyla bırakılır (`drop_alarm`; ACK alarm yolundan, bütçe yedeğiyle). Başlangıç
/// bulunursa #237 kuralı aynen kalır. Sapma sahibin kuralındandır: "kanıt ya da planlı ACK, hangisi önceyse".
#[test]
fn b_alarm_without_an_onset_leaves_at_the_alarm() {
    let mut checked = 0;
    for seed in 1..=8u64 {
        let flow = Flow {
            file_from: 200 * MS,
            file_until: 20 * SEC,
            ptt: Ptt::Copied,
            until: 20 * SEC,
            line: vec![(0, PROFILE), (8 * SEC, PROFILE / 2)],
            loss: Some((seed, 200)),
        };
        let arr = on_ms(arrivals(&flow));
        // Bağımsız dedektör 3 s'den başlar (alıcınınki ilk ACK'in göndericiye en erken varışından,
        // ≈ 1–2 s; düşüşten önce ikisi aynı varışları görür). Düşüşten sonraki ilk tetiklenmesi
        // alarm anıdır; başlangıç (`onset`) o anda okunur. Alıcının alarm sayacı aynı varışta
        // artmalıdır (bırakma) ve ACK o varışta çıkmalıdır.
        let mut r = receiver();
        let mut det = RateDropDetector::default();
        let mut i = 0;
        let mut now = 0;
        let mut found = None;
        while now < 20 * SEC && found.is_none() {
            while i < arr.len() && arr[i].0 <= now {
                let (_, udp, seq) = arr[i];
                let before = r.diag_drop_alarms();
                let fired = now >= 3 * SEC && det.on_arrival_seq(now, udp, Some(seq));
                let onset = det.onset().is_some();
                r.set_next_datagram_bytes(udp);
                let out = r.on_datagram(
                    RxDatagram {
                        seq,
                        unit_id: seq,
                        kind: RxKind::Data,
                    },
                    now,
                );
                if fired && now >= 8 * SEC {
                    found = Some((now, onset, r.diag_drop_alarms() > before, out.ack.is_some()));
                    break;
                }
                if out.ack.is_some() {
                    det.on_ack(now);
                }
                i += 1;
            }
            if found.is_none() && r.on_ack_timer(now).is_some() {
                det.on_ack(now);
            }
            now += MS;
        }
        let Some((t, onset, released, ack)) = found else {
            println!("(b) tohum {seed}: düşüşten sonra alarm yok");
            continue;
        };
        println!(
            "(b) tohum {seed}: alarm {:.3} s, başlangıç {onset}, bırakıldı {released}, ACK aynı \
             varışta {ack}",
            t as f64 / SEC as f64
        );
        // Başlangıç bulunduysa #237 kuralı: kanıt aynı varışta yeterliyse bırakılır, değilse
        // bekler (bu kolda sınanmaz; `ack_alarm_interval.rs`, `ack_alarm_planned.rs`).
        if !onset {
            assert!(
                released,
                "(b) tohum {seed}: başlangıçsız alarm alarm anında bırakılmadı (ACK aynı varışta {ack})"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "(b) başlangıçsız alarm ölçülmedi");
}
