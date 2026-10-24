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
//! `t` = açılıştan beri saniye. Sayaçlar birikimli; `relay_pps` / `relay_bps` son aralığın ortalaması.

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

/// Birikimli sayaçlar (hepsi doyar).
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

/// Sayacı bir artırır (doyar).
pub fn bump(counter: &mut u64) {
    *counter = counter.saturating_add(1);
}

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
        let pps = per_second(s.relay_packets.saturating_sub(self.relay_packets));
        let bps = per_second(
            s.relay_bytes
                .saturating_sub(self.relay_bytes)
                .saturating_mul(8),
        );
        sink.log(format_args!(
            "hub stats t={} endpoints={} links={} tombstones={} bind_fail_entries={} relay_pps={pps} \
             relay_bps={bps} bind_ok={} bind_ack_repeat={} bind_bad_cookie={} bind_fail={} bind_unknown={} \
             bind_held={} store_ok={} store_repeat={} store_key_retired={} peer_unbound={} hub_capacity={} \
             route_miss={} drop_size={} drop_malformed={} drop_unknown_frame={} drop_unknown_link={} \
             src_mismatch={} src_unbound={} rate_store={} rate_alloc={} hb_unbound={} bind_moved={} bind_displaced={} rate_verify={}",
            now.as_secs(),
            sizes.endpoints,
            sizes.links,
            sizes.tombstones,
            sizes.bind_fail_entries,
            s.bind_ok,
            s.bind_ack_repeat,
            s.bind_bad_cookie,
            s.bind_fail,
            s.bind_unknown,
            s.bind_held,
            s.store_ok,
            s.store_repeat,
            s.store_key_retired,
            s.peer_unbound,
            s.hub_capacity,
            s.route_miss,
            s.drop_size,
            s.drop_malformed,
            s.drop_unknown_frame,
            s.drop_unknown_link,
            s.src_mismatch,
            s.src_unbound,
            s.rate_store,
            s.rate_alloc,
            s.hb_unbound,
            s.bind_moved,
            s.bind_displaced,
            s.rate_verify,
        ));
        *self = Self {
            at: now,
            relay_packets: s.relay_packets,
            relay_bytes: s.relay_bytes,
        };
    }
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
