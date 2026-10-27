//! Sayaçlar ve log satırları (KM-HUB-TDD-001 §5).
//!
//! Hub'ın yazdığı HER log satırı bu modüldeki fonksiyonlardan çıkar. Argümanlar yalnız sayılar ve
//! [`ShortId`] (kimliğin ilk dört baytı); yük, zarf, tam kimlik ya da anahtar alan bir imza yoktur
//! (S4 H12, T-L1).
//!
//! ```text
//! hub stats t= endpoints= links= tombstones= bind_fail_entries= relay_pps= relay_bps= bind_ok=
//!           bind_ack_repeat= bind_bad_cookie= bind_fail= bind_unknown= bind_held= store_ok= store_repeat=
//!           store_key_retired= peer_unbound= hub_capacity= route_miss= drop_size= drop_malformed=
//!           drop_unknown_frame= drop_unknown_link= src_mismatch= src_unbound= rate_store= rate_alloc=
//!           hb_unbound= bind_moved= bind_displaced= rate_verify=  (tek satır, STATS_PERIOD'da bir)
//! ```
//! Son üç alan TDD §5 listesine eklentidir (YAAZ-EK-1: "yer değiştirme" sayaçları okunabilir olmalı;
//! `rate_verify` doğrulama bütçesinin tükenmesini görünür kılar). Önceki alanların sırası değişmez.
//! ```text
//! bind ok dh=<8 hex> epoch=<n>
//! link open h=<8 hex> a=<8 hex> b=<8 hex> type=<2 hex>
//! link retire h=<8 hex>
//! cookie rotate
//! ```
//! `t` = açılıştan beri saniye. Sayaçlar birikimli ve `u64`'te SARAR (Counter64 gibi); okuyan farkı
//! `wrapping_sub` ile alır. `relay_pps` / `relay_bps` son aralığın ortalaması, sarma anında da doğru.
//!
//! Satırın alan sırası tek yerde: [`COUNTERS`] tablosu (sayaçlar) + [`StatsLine`]'ın baştaki yedi alanı.

use core::fmt;
use core::time::Duration;

use crate::wire::DestHash;
use crate::Sink;

/// Kimliğin loglanabilen kısmı: ilk dört bayt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortId([u8; 4]);

impl ShortId {
    #[must_use]
    pub fn of(dest_hash: &DestHash) -> Self {
        let mut short = [0u8; 4];
        for (slot, byte) in short.iter_mut().zip(dest_hash) {
            *slot = *byte;
        }
        Self(short)
    }
}

impl fmt::Display for ShortId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Birikimli sayaçlar (hepsi sarar; bkz. modül belgesi).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Alınan datagram.
    pub rx: u64,
    /// Gelen datagramın KAYNAĞINA giden cevap (H10).
    pub tx_reply: u64,
    /// BAĞLI bir uca giden iletim: veri datagramı ya da DELIVER.
    pub tx_forward: u64,
    pub relay_packets: u64,
    pub relay_bytes: u64,
    pub bind_ok: u64,
    pub bind_ack_repeat: u64,
    pub bind_bad_cookie: u64,
    /// Geçerli cookie sonrası imza başarısızlığı.
    pub bind_fail: u64,
    pub bind_unknown: u64,
    pub bind_held: u64,
    /// `ed25519` doğrulama çağrısı (T-B4: yanlış cookie'de artmaz).
    pub verify_calls: u64,
    /// `VERIFY_BUDGET` aşımı.
    pub rate_verify: u64,
    pub store_ok: u64,
    pub store_repeat: u64,
    pub store_key_retired: u64,
    pub peer_unbound: u64,
    pub hub_capacity: u64,
    pub bad_request: u64,
    pub route_miss: u64,
    pub drop_size: u64,
    pub drop_malformed: u64,
    pub drop_unknown_frame: u64,
    pub drop_unknown_method: u64,
    pub drop_unknown_link: u64,
    pub drop_peer_unbound: u64,
    pub src_mismatch: u64,
    pub src_unbound: u64,
    pub rate_store: u64,
    pub rate_alloc: u64,
    pub hb_unbound: u64,
    /// Kabul edilen bind ucun adresini taşıdı (H3).
    pub bind_moved: u64,
    /// Kabul edilen bind başka bir kimliğin adresini aldı; o uç adressiz (H3, TDD v0.6.1).
    pub bind_displaced: u64,
}

/// Sayacı bir artırır (sarar: `u64::MAX` → 0; doyan sayaç orada takılır ve fark hesabını öldürür).
pub fn bump(counter: &mut u64) {
    *counter = counter.wrapping_add(1);
}

/// Sayaca `n` ekler (sarar).
pub fn add(counter: &mut u64, n: u64) {
    *counter = counter.wrapping_add(n);
}

/// `hub stats` satırındaki bir sayaç alanı: ad ve okuyucu.
pub type Counter = (&'static str, fn(&Stats) -> u64);

/// `hub stats` satırının sayaç alanları, satırdaki SIRAYLA (TDD §5; son üçü eklenti).
pub const COUNTERS: [Counter; 24] = [
    ("bind_ok", |s| s.bind_ok),
    ("bind_ack_repeat", |s| s.bind_ack_repeat),
    ("bind_bad_cookie", |s| s.bind_bad_cookie),
    ("bind_fail", |s| s.bind_fail),
    ("bind_unknown", |s| s.bind_unknown),
    ("bind_held", |s| s.bind_held),
    ("store_ok", |s| s.store_ok),
    ("store_repeat", |s| s.store_repeat),
    ("store_key_retired", |s| s.store_key_retired),
    ("peer_unbound", |s| s.peer_unbound),
    ("hub_capacity", |s| s.hub_capacity),
    ("route_miss", |s| s.route_miss),
    ("drop_size", |s| s.drop_size),
    ("drop_malformed", |s| s.drop_malformed),
    ("drop_unknown_frame", |s| s.drop_unknown_frame),
    ("drop_unknown_link", |s| s.drop_unknown_link),
    ("src_mismatch", |s| s.src_mismatch),
    ("src_unbound", |s| s.src_unbound),
    ("rate_store", |s| s.rate_store),
    ("rate_alloc", |s| s.rate_alloc),
    ("hb_unbound", |s| s.hb_unbound),
    ("bind_moved", |s| s.bind_moved),
    ("bind_displaced", |s| s.bind_displaced),
    ("rate_verify", |s| s.rate_verify),
];

/// `hub stats` satırının tablo boyutları.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sizes {
    pub endpoints: usize,
    pub links: usize,
    pub tombstones: usize,
    pub bind_fail_entries: usize,
}

/// `hub stats` aralığı.
#[derive(Debug, Clone, Copy, Default)]
pub struct RateWindow {
    at: Duration,
    relay_packets: u64,
    relay_bytes: u64,
}

impl RateWindow {
    #[must_use]
    pub fn starting_at(now: Duration) -> Self {
        Self {
            at: now,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn due(&self, now: Duration, period: Duration) -> bool {
        now.saturating_sub(self.at) >= period
    }

    /// Satırı yazar, pencereyi `now`'a taşır.
    pub fn emit(&mut self, sink: &mut dyn Sink, now: Duration, s: &Stats, sizes: Sizes) {
        let elapsed_ms = u64::try_from(now.saturating_sub(self.at).as_millis())
            .unwrap_or(u64::MAX)
            .max(1);
        let per_second = |delta: u64| {
            delta
                .saturating_mul(1000)
                .checked_div(elapsed_ms)
                .unwrap_or(0)
        };
        // Sayaçlar sarar → fark `wrapping_sub` ile (sarma anında da doğru).
        let line = StatsLine {
            t: now.as_secs(),
            sizes,
            relay_pps: per_second(s.relay_packets.wrapping_sub(self.relay_packets)),
            relay_bps: per_second(
                s.relay_bytes
                    .wrapping_sub(self.relay_bytes)
                    .saturating_mul(8),
            ),
            stats: s,
        };
        sink.log(format_args!("{line}"));
        *self = Self {
            at: now,
            relay_packets: s.relay_packets,
            relay_bytes: s.relay_bytes,
        };
    }
}

/// Tek `hub stats` satırı; `Display` doğrudan yazar (ayırma yok).
pub struct StatsLine<'a> {
    pub t: u64,
    pub sizes: Sizes,
    pub relay_pps: u64,
    pub relay_bps: u64,
    pub stats: &'a Stats,
}

impl fmt::Display for StatsLine<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Sizes {
            endpoints,
            links,
            tombstones,
            bind_fail_entries,
        } = self.sizes;
        write!(
            f,
            "hub stats t={} endpoints={endpoints} links={links} tombstones={tombstones} bind_fail_entries={bind_fail_entries} relay_pps={} relay_bps={}",
            self.t, self.relay_pps, self.relay_bps
        )?;
        for (name, read) in COUNTERS {
            write!(f, " {name}={}", read(self.stats))?;
        }
        Ok(())
    }
}

/// `send_to` hataları (`main.rs` `UdpSink`). Hub gönderimi yeniden DENEMEZ ve beklemez (röle: paket
/// düşer); hata sayılır ve en çok `period`'da bir satırla yöneticiye görünür olur — kalıcı bir ağ
/// hatası her pakette satır yazıp log'u doldurmasın (C6) ve log yazımı döngüyü yavaşlatmasın.
///
/// ```text
/// send error kind=<io::ErrorKind> to=<ip:port> total=<n> suppressed=<son satırdan beri yazılmayan>: <hata>
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SendErrors {
    /// Açılıştan beri bütün gönderim hataları (sarar).
    pub total: u64,
    /// Son satırdan beri yazılmadan sayılanlar.
    pub suppressed: u64,
    last_line: Option<Duration>,
}

impl SendErrors {
    /// Hatayı sayar; satır yazılmalıysa `true` (ilk hata ya da son satırdan `period` geçtiyse).
    /// `true` dönünce çağıran [`log_send_error`]'u çağırır, sonra [`SendErrors::logged`].
    pub fn record(&mut self, now: Duration, period: Duration) -> bool {
        bump(&mut self.total);
        let due = self
            .last_line
            .is_none_or(|at| now.saturating_sub(at) >= period);
        if !due {
            bump(&mut self.suppressed);
        }
        due
    }

    /// Satır yazıldı: bastırma sayacı sıfırlanır.
    pub fn logged(&mut self, now: Duration) {
        self.last_line = Some(now);
        self.suppressed = 0;
    }
}

pub fn log_send_error(
    sink: &mut dyn Sink,
    to: std::net::SocketAddr,
    error: &std::io::Error,
    errors: &SendErrors,
) {
    sink.log(format_args!(
        "send error kind={:?} to={to} total={} suppressed={}: {error}",
        error.kind(),
        errors.total,
        errors.suppressed
    ));
}

pub fn log_bind_ok(sink: &mut dyn Sink, id: ShortId, epoch: u32) {
    sink.log(format_args!("bind ok dh={id} epoch={epoch}"));
}

pub fn log_link_open(sink: &mut dyn Sink, handle: u32, a: ShortId, b: ShortId, envelope_type: u8) {
    sink.log(format_args!(
        "link open h={handle:08x} a={a} b={b} type={envelope_type:02x}"
    ));
}

pub fn log_link_retire(sink: &mut dyn Sink, handle: u32) {
    sink.log(format_args!("link retire h={handle:08x}"));
}

pub fn log_cookie_rotate(sink: &mut dyn Sink) {
    sink.log(format_args!("cookie rotate"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[derive(Default)]
    struct Lines(Vec<String>);

    impl Sink for Lines {
        fn send(&mut self, _to: SocketAddr, _datagram: &[u8]) {}
        fn log(&mut self, line: fmt::Arguments<'_>) {
            self.0.push(line.to_string());
        }
    }

    #[test]
    fn stats_line_has_the_documented_fields_in_order() {
        let mut lines = Lines::default();
        let mut window = RateWindow::starting_at(Duration::ZERO);
        let period = Duration::from_secs(10);
        assert!(!window.due(Duration::from_millis(9_999), period));
        assert!(window.due(period, period));
        let stats = Stats {
            relay_packets: 500,
            relay_bytes: 125_000,
            bind_ok: 2,
            hb_unbound: 9,
            ..Stats::default()
        };
        window.emit(
            &mut lines,
            period,
            &stats,
            Sizes {
                endpoints: 2,
                links: 1,
                tombstones: 3,
                bind_fail_entries: 4,
            },
        );
        let line = &lines.0[0];
        let keys: Vec<&str> = line
            .strip_prefix("hub stats ")
            .expect("önek")
            .split(' ')
            .map(|field| field.split('=').next().expect("anahtar"))
            .collect();
        assert_eq!(
            keys,
            [
                "t",
                "endpoints",
                "links",
                "tombstones",
                "bind_fail_entries",
                "relay_pps",
                "relay_bps",
                "bind_ok",
                "bind_ack_repeat",
                "bind_bad_cookie",
                "bind_fail",
                "bind_unknown",
                "bind_held",
                "store_ok",
                "store_repeat",
                "store_key_retired",
                "peer_unbound",
                "hub_capacity",
                "route_miss",
                "drop_size",
                "drop_malformed",
                "drop_unknown_frame",
                "drop_unknown_link",
                "src_mismatch",
                "src_unbound",
                "rate_store",
                "rate_alloc",
                "hb_unbound",
                "bind_moved",
                "bind_displaced",
                "rate_verify",
            ]
        );
        assert!(line.contains(" relay_pps=50 relay_bps=100000 "), "{line}");
        assert!(line
            .starts_with("hub stats t=10 endpoints=2 links=1 tombstones=3 bind_fail_entries=4 "));
        assert!(line.ends_with(" hb_unbound=9 bind_moved=0 bind_displaced=0 rate_verify=0"));
    }

    #[test]
    fn line_fields_come_from_the_counter_table() {
        // Tablodaki her sayaç satırda kendi değeriyle görünür: alan ile okuyucu karışamaz.
        let mut stats = Stats::default();
        let fields: [&mut u64; 24] = [
            &mut stats.bind_ok,
            &mut stats.bind_ack_repeat,
            &mut stats.bind_bad_cookie,
            &mut stats.bind_fail,
            &mut stats.bind_unknown,
            &mut stats.bind_held,
            &mut stats.store_ok,
            &mut stats.store_repeat,
            &mut stats.store_key_retired,
            &mut stats.peer_unbound,
            &mut stats.hub_capacity,
            &mut stats.route_miss,
            &mut stats.drop_size,
            &mut stats.drop_malformed,
            &mut stats.drop_unknown_frame,
            &mut stats.drop_unknown_link,
            &mut stats.src_mismatch,
            &mut stats.src_unbound,
            &mut stats.rate_store,
            &mut stats.rate_alloc,
            &mut stats.hb_unbound,
            &mut stats.bind_moved,
            &mut stats.bind_displaced,
            &mut stats.rate_verify,
        ];
        for (value, field) in (100u64..).zip(fields) {
            *field = value;
        }
        let mut lines = Lines::default();
        RateWindow::starting_at(Duration::ZERO).emit(
            &mut lines,
            Duration::from_secs(10),
            &stats,
            Sizes::default(),
        );
        for ((name, read), value) in COUNTERS.iter().zip(100u64..) {
            assert_eq!(read(&stats), value, "{name}");
            assert!(lines.0[0].contains(&format!(" {name}={value}")), "{name}");
        }
    }

    #[test]
    fn counters_wrap_and_rates_survive_the_wrap() {
        let mut counter = u64::MAX;
        bump(&mut counter);
        assert_eq!(counter, 0);
        let mut bytes = u64::MAX - 99;
        add(&mut bytes, 1_100);
        assert_eq!(bytes, 1_000);

        // Pencere sarmadan hemen önce açılır; 10 s'de 500 paket / 125 000 bayt geçer.
        let before = Stats {
            relay_packets: u64::MAX - 199,
            relay_bytes: u64::MAX - 24_999,
            ..Stats::default()
        };
        let mut window = RateWindow::starting_at(Duration::ZERO);
        let mut lines = Lines::default();
        window.emit(&mut lines, Duration::ZERO, &before, Sizes::default());
        let mut after = before;
        add(&mut after.relay_packets, 500);
        add(&mut after.relay_bytes, 125_000);
        assert!(after.relay_packets < before.relay_packets, "sarmadı");
        window.emit(
            &mut lines,
            Duration::from_secs(10),
            &after,
            Sizes::default(),
        );
        assert!(
            lines.0[1].contains(" relay_pps=50 relay_bps=100000 "),
            "{}",
            lines.0[1]
        );
    }

    #[test]
    fn send_errors_are_counted_and_logged_at_most_once_per_period() {
        let period = Duration::from_secs(10);
        let mut errors = SendErrors::default();
        let mut lines = Lines::default();
        let to: SocketAddr = "192.0.2.7:47000".parse().expect("adres");
        let error = std::io::Error::from(std::io::ErrorKind::NetworkUnreachable);
        let at = |errors: &mut SendErrors, lines: &mut Lines, ms: u64| {
            let now = Duration::from_millis(ms);
            if errors.record(now, period) {
                log_send_error(lines, to, &error, errors);
                errors.logged(now);
            }
        };
        at(&mut errors, &mut lines, 0); // ilk hata → satır
        for ms in 1..=1_000 {
            at(&mut errors, &mut lines, ms); // aynı aralık → sayılır, yazılmaz
        }
        assert_eq!(lines.0.len(), 1);
        assert_eq!((errors.total, errors.suppressed), (1_001, 1_000));
        at(&mut errors, &mut lines, 10_000); // aralık doldu → satır, bastırılanlarla
        assert_eq!(lines.0.len(), 2);
        assert!(
            lines.0[1].starts_with(
                "send error kind=NetworkUnreachable to=192.0.2.7:47000 total=1002 suppressed=1000: "
            ),
            "{}",
            lines.0[1]
        );
        assert_eq!(errors.suppressed, 0);
    }

    #[test]
    fn event_lines() {
        let mut lines = Lines::default();
        let a = ShortId([0xa1, 0xb2, 0xc3, 0xd4]);
        log_bind_ok(&mut lines, a, 3);
        log_link_open(&mut lines, 0x10, a, ShortId([0xc3, 0xd4, 0xe5, 0xf6]), 5);
        log_link_retire(&mut lines, 0x10);
        log_cookie_rotate(&mut lines);
        assert_eq!(
            lines.0,
            [
                "bind ok dh=a1b2c3d4 epoch=3",
                "link open h=00000010 a=a1b2c3d4 b=c3d4e5f6 type=05",
                "link retire h=00000010",
                "cookie rotate",
            ]
        );
    }
}
