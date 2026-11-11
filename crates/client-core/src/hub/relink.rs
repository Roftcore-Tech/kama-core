//! [HLR] Hat kaybı yenilemesinin eş başına sınırı ve geri çekilmesi — S4 C5 ("`ROUTE_MISS{h}` → o
//! handle bırakılır, oturum için yeni INIT; hub başına eş başına sınırlı deneme + geri çekilme
//! (app-rpc §6.11.1 ruhu)").
//!
//! [`RelinkGovernor`] sans-IO'dur (KK-H33): `now` dışarıdan gelir, transport'a dokunmaz. Bileşim
//! kökü (`kama-node`) her hub bildirimini [`RelinkGovernor::on_notice`]'e verir ve her turda
//! [`RelinkGovernor::drive`]'ı transport çağrısıyla (`ClientTransport::relink` /
//! `ClientTransport::restart_renewal`) sürer. Bir yönetici bir hub'ındır (bugün düğüm başına bir
//! hub); tablo eş başınadır.
//!
//! | kural | burada |
//! |---|---|
//! | sınırlı deneme | eş başına art arda ilk [`RELINK_FREE_STARTS`] yenileme beklemesiz (app-rpc §6.11.1 `max_rebind_attempts` 2..3) |
//! | geri çekilme | sonrakiler arasında `RELINK_BACKOFF_MIN · 2^k`, tavan [`RELINK_BACKOFF_MAX`], + jitter `[0, aralık/4)` (KK-H48 `heartbeat::jitter`) |
//! | sayaç sıfırlama | eş için [`RELINK_QUIET_RESET`] boyunca yeni istek yoksa |
//! | yerel erteleme | tavana takılan (`Deferred`) istek kaybolmaz: [`RELINK_RETRY`]'den başlayıp ikiye katlanan aralıkla yeniden denenir (tele bayt çıkmaz) |
//! | bellek | eş tablosu [`MAX_RELINK_PEERS`] (SR-AUDIT-001) |
//! | eski hat | [HLR-STALE] `HandleLost{peer, h}` yalnız eşin ETKİN oturumunun hattı `h` ise yenilemeye gider: bildirim anında (`on_notice`, [`ActiveLink`]) ve uygulama anında (`RelinkLine(h)` → `ClientTransport::relink_line`) aynı kural; eski hattınki düşer ve sayılır (`stale_handle_lost`). Hat bilinmiyorsa düşmez (koşulsuz yenileme, `handle_lost_unverified`) |
//!
//! Tek hub'lı PoC'de app-rpc §6.11.1'in "K başarısızlıktan sonra sıradaki hub'a FAILOVER" kolu
//! yoktur: K'den sonra aynı hub'a yalnız geri çekilmeyle dönülür (§6.11.1 "degraded hub için
//! exponential backoff + jitter"). Kurulu oturum hiçbir zaman bırakılmaz (INV-NO-IDLE-TEARDOWN);
//! sahte `ROUTE_MISS` (S4 beyanlı sınır 1) eş başına en çok tavan aralığında bir INIT üretebilir.

use core::time::Duration;
use std::time::Instant;

use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;
use kama_contracts::s4::NONCE_LEN;

use super::heartbeat::jitter;
use super::t3::BACKOFF_MIN;
use super::HubNotice;
use crate::transport::{ActiveLink, RelinkOutcome};

/// Jitter tohumunun boyu (KK-H48 `heartbeat::jitter` girdisi).
pub const RELINK_SEED_LEN: usize = NONCE_LEN;

/// Eş başına geri çekilmesiz art arda yenileme sayısı (app-rpc §6.11.1 `max_rebind_attempts`).
pub const RELINK_FREE_STARTS: u32 = 3;
/// Geri çekilmenin ilk aralığı (S4 "≥ 5 s geri çekilme"; T3 `BACKOFF_MIN`).
pub const RELINK_BACKOFF_MIN: Duration = BACKOFF_MIN;
/// Geri çekilme aralığının tavanı (jitter hariç) `[KALİBRASYON]`.
pub const RELINK_BACKOFF_MAX: Duration = Duration::from_secs(300);
/// Eş için bu kadar süre yeni istek gelmezse sayaç sıfırlanır (tavanın iki katı: süren bir
/// `ROUTE_MISS` akışında sayaç sıfırlanmaz) `[KALİBRASYON]`.
pub const RELINK_QUIET_RESET: Duration = Duration::from_secs(600);
/// Yerel tavana takılan isteğin ilk yeniden deneme aralığı `[KALİBRASYON]`.
pub const RELINK_RETRY: Duration = Duration::from_secs(1);
/// Eş tablosu tavanı (SR-AUDIT-001). Doluyken isteği olmayan en eski girdi düşer; hepsinin isteği
/// varsa yeni istek düşer ve sayılır (`dropped`).
pub const MAX_RELINK_PEERS: usize = 256;

/// Bileşim kökünün transport'a yapacağı çağrı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelinkOp {
    /// `ClientTransport::restart_renewal` (yenilemenin hub denemesi bitti).
    Restart,
    /// `ClientTransport::relink` (kurulu oturum için koşulsuz yeni INIT: `Restart`'ın yerel tavana
    /// takılan yeniden başlatması ya da hattı bilinmeyen oturumun kaybı).
    Relink,
    /// [HLR-STALE] `ClientTransport::relink_line` (kurulu oturumun `h` hattı kayboldu): `h` artık
    /// eşin etkin oturumunun hattı değilse transport `Stale` döner, iş yapılmaz.
    RelinkLine(HubHandle),
}

/// Bir hub bildiriminin yönü ([`RelinkGovernor::on_notice`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeRoute {
    /// Yönetici üstlendi (istek kaydedildi ya da sınırlı tablo doluydu).
    Governed,
    /// Kurulu oturumu olmayan eşin açılış denemesi bitti: bekleyen niyet yeniden `connect` edilir
    /// (KK-H57 (a); bu yöneticinin işi değil).
    RestartIntent(PeerRef),
    /// [HLR-STALE] `HandleLost{peer, h}`'nin `h`'si eşin etkin oturumunun hattı değil (yerini yenisi
    /// almış oturumun eski hattı): düştü, `RelinkStats::stale_handle_lost` sayıldı.
    Stale,
    /// İş yok (`Bound`, `Disconnected`).
    Ignore,
}

/// Salt-okunur sayaçlar (doyarak artar).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelinkStats {
    /// Kaydedilen istek (`HandleLost` ya da oturumlu eşin `AttemptEnded`'i).
    pub requests: u64,
    /// Yola çıkan yenileme INIT'i (`RelinkOutcome::Started`).
    pub started: u64,
    /// Yerel tavana takılan deneme (`RelinkOutcome::Deferred`; yeniden denenir).
    pub deferred: u64,
    /// Yapılacak iş çıkmayan deneme (`RelinkOutcome::Nothing`).
    pub nothing: u64,
    /// Tablo dolu, istek düştü.
    pub dropped: u64,
    /// [HLR-STALE] Eski hattın kaybı düştü: bildirim anında (`NoticeRoute::Stale`) ya da uygulama
    /// anında (`RelinkOutcome::Stale`) `h` eşin etkin oturumunun hattı değildi.
    pub stale_handle_lost: u64,
    /// [HLR-STALE] Etkin oturumun hattı bilinmeden kabul edilen kayıp (koşulsuz yenilemeye gitti).
    pub handle_lost_unverified: u64,
}

/// Bir eşin durumu.
#[derive(Debug, Clone, Copy)]
struct Entry {
    peer: PeerRef,
    /// `HandleLost` bekliyor.
    relink: bool,
    /// [HLR-STALE] Bekleyen `relink`'in uygulama anında denetlenecek hattı; `None` = koşulsuz.
    line: Option<HubHandle>,
    /// Oturumlu eşin `AttemptEnded`'i bekliyor.
    restart: bool,
    /// Sayaç penceresinde yola çıkan yenileme.
    starts: u32,
    /// Art arda yerel erteleme.
    deferrals: u32,
    /// Son istek anı (sayaç sıfırlama).
    last_request: Instant,
    /// Bu andan önce transport çağrılmaz (geri çekilme / erteleme).
    not_before: Instant,
}

impl Entry {
    fn wants(&self) -> bool {
        self.relink || self.restart
    }
}

/// Eş başına sınırlı deneme + geri çekilme (bkz. modül belgesi).
pub struct RelinkGovernor {
    entries: Vec<Entry>,
    seed: [u8; RELINK_SEED_LEN],
    draws: u32,
    stats: RelinkStats,
}

impl RelinkGovernor {
    /// `seed`: jitter tohumu (üründe OS entropisi; testte sabit).
    #[must_use]
    pub fn new(seed: [u8; RELINK_SEED_LEN]) -> Self {
        Self {
            entries: Vec::new(),
            seed,
            draws: 0,
            stats: RelinkStats::default(),
        }
    }

    /// Hub bildirimi. `has_session`: eşin kurulu (Application'da hazır) oturumu var mı. `link`:
    /// eşin etkin oturumunun hattı (`ClientTransport::active_link`; yalnız `HandleLost` okur).
    ///
    /// [HLR-STALE] `HandleLost{peer, h}`: `link = On(a)`, `a ≠ h` → eski hattın geç kaybı, düşer
    /// (`Stale`, sayaç); istek kaydedilmez — kaydedilseydi aynı eşin bekleyen geçerli isteğinin
    /// hattını ezebilirdi. `On(h)` / `NoSession` → `RelinkLine(h)` (uygulama anında yeniden
    /// denetlenir). `Unknown` → koşulsuz `Relink` (yanlış negatif yok: doğrulanamayan kayıp
    /// düşürülmez; `handle_lost_unverified`).
    pub fn on_notice(
        &mut self,
        now: Instant,
        notice: HubNotice,
        has_session: bool,
        link: ActiveLink,
    ) -> NoticeRoute {
        match notice {
            HubNotice::HandleLost { peer, h } => match link {
                ActiveLink::On(active) if active != h => {
                    bump(&mut self.stats.stale_handle_lost);
                    NoticeRoute::Stale
                }
                ActiveLink::Unknown => {
                    bump(&mut self.stats.handle_lost_unverified);
                    self.request(now, peer, RelinkOp::Relink);
                    NoticeRoute::Governed
                }
                ActiveLink::On(_) | ActiveLink::NoSession => {
                    self.request(now, peer, RelinkOp::RelinkLine(h));
                    NoticeRoute::Governed
                }
            },
            HubNotice::AttemptEnded { peer } if has_session => {
                self.request(now, peer, RelinkOp::Restart);
                NoticeRoute::Governed
            }
            HubNotice::AttemptEnded { peer } => NoticeRoute::RestartIntent(peer),
            HubNotice::Bound | HubNotice::Disconnected => NoticeRoute::Ignore,
        }
    }

    /// Vadesi gelen istekleri `act` ile uygular (bileşim kökü: transport çağrısı). Her turda
    /// çağrılır; vadesi gelmemiş ya da isteği olmayan eş için `act` çağrılmaz.
    ///
    /// Sıra: `Restart` önce (bitmiş denemenin yenilemesi yeniden başlar; yeni INIT `Relink`'i de
    /// karşılar). `Restart` `Nothing` dönerse ve `Relink` bekliyorsa `Relink`. `Restart`
    /// `Deferred` dönerse (eski bekleyen kapandı, yenisi tavana takıldı) istek koşulsuz `Relink`
    /// olarak sürer. [HLR-STALE] `RelinkLine(h)`'nin `Stale`'i isteği bitirir (sayaç).
    pub fn drive(&mut self, now: Instant, mut act: impl FnMut(PeerRef, RelinkOp) -> RelinkOutcome) {
        for i in 0..self.entries.len() {
            let e = &mut self.entries[i];
            if !e.wants() || now < e.not_before {
                continue;
            }
            let mut out = RelinkOutcome::Nothing;
            if e.restart {
                e.restart = false;
                out = act(e.peer, RelinkOp::Restart);
                if out == RelinkOutcome::Deferred {
                    e.relink = true;
                    e.line = None;
                }
            }
            if out == RelinkOutcome::Nothing && e.relink {
                out = act(
                    e.peer,
                    e.line.map_or(RelinkOp::Relink, RelinkOp::RelinkLine),
                );
            }
            match out {
                RelinkOutcome::Started => {
                    e.relink = false;
                    e.line = None;
                    e.restart = false;
                    e.deferrals = 0;
                    e.starts = e.starts.saturating_add(1);
                    bump(&mut self.stats.started);
                    let gap = backoff(e.starts, &self.seed, self.draws);
                    self.draws = self.draws.wrapping_add(1);
                    e.not_before = now + gap;
                }
                RelinkOutcome::Nothing => {
                    e.relink = false;
                    e.line = None;
                    bump(&mut self.stats.nothing);
                }
                RelinkOutcome::Stale => {
                    e.relink = false;
                    e.line = None;
                    bump(&mut self.stats.stale_handle_lost);
                }
                RelinkOutcome::Deferred => {
                    e.relink = true;
                    e.deferrals = e.deferrals.saturating_add(1);
                    bump(&mut self.stats.deferred);
                    e.not_before = now + retry_gap(e.deferrals);
                }
            }
        }
        self.entries.retain(|e| {
            e.wants()
                || (e.starts > 0
                    && now.saturating_duration_since(e.last_request) < RELINK_QUIET_RESET)
        });
    }

    /// Sayaçlar.
    #[must_use]
    pub fn stats(&self) -> RelinkStats {
        self.stats
    }

    /// Tablodaki eş sayısı (≤ [`MAX_RELINK_PEERS`]).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Tablo boş mu.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn request(&mut self, now: Instant, peer: PeerRef, op: RelinkOp) {
        let i = match self.entries.iter().position(|e| e.peer == peer) {
            Some(i) => i,
            None => {
                if self.entries.len() >= MAX_RELINK_PEERS {
                    let idle = self
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| !e.wants())
                        .min_by_key(|(_, e)| e.last_request)
                        .map(|(i, _)| i);
                    let Some(idle) = idle else {
                        bump(&mut self.stats.dropped);
                        return;
                    };
                    self.entries.swap_remove(idle);
                }
                self.entries.push(Entry {
                    peer,
                    relink: false,
                    line: None,
                    restart: false,
                    starts: 0,
                    deferrals: 0,
                    last_request: now,
                    not_before: now,
                });
                self.entries.len() - 1
            }
        };
        bump(&mut self.stats.requests);
        let e = &mut self.entries[i];
        e.last_request = now;
        match op {
            RelinkOp::Restart => e.restart = true,
            RelinkOp::Relink => {
                e.relink = true;
                e.line = None;
            }
            // Bekleyen koşulsuz istek koşulsuz kalır; bekleyen hatlı isteğin hattı en yenisidir
            // (bildirim anında etkin oturumun hattıydı).
            RelinkOp::RelinkLine(h) => {
                if !e.relink || e.line.is_some() {
                    e.line = Some(h);
                }
                e.relink = true;
            }
        }
    }
}

/// `starts`'ıncı yenilemeden sonra bir sonrakine dek beklenecek süre: ilk
/// [`RELINK_FREE_STARTS`] için sıfır; sonra `RELINK_BACKOFF_MIN · 2^k` (tavan
/// [`RELINK_BACKOFF_MAX`]) + jitter `[0, aralık/4)`.
fn backoff(starts: u32, seed: &[u8; RELINK_SEED_LEN], draw: u32) -> Duration {
    let Some(k) = starts.checked_sub(RELINK_FREE_STARTS) else {
        return Duration::ZERO;
    };
    let base = 1u32
        .checked_shl(k)
        .and_then(|m| RELINK_BACKOFF_MIN.checked_mul(m))
        .map_or(RELINK_BACKOFF_MAX, |d| d.min(RELINK_BACKOFF_MAX));
    base + jitter(seed, draw, base)
}

/// `deferrals`'ıncı art arda yerel ertelemeden sonraki bekleme: `RELINK_RETRY · 2^(d−1)`, tavan
/// [`RELINK_BACKOFF_MAX`].
fn retry_gap(deferrals: u32) -> Duration {
    1u32.checked_shl(deferrals.saturating_sub(1))
        .and_then(|m| RELINK_RETRY.checked_mul(m))
        .map_or(RELINK_BACKOFF_MAX, |d| d.min(RELINK_BACKOFF_MAX))
}

fn bump(c: &mut u64) {
    *c = c.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; RELINK_SEED_LEN] = [7; RELINK_SEED_LEN];

    fn peer(b: u8) -> PeerRef {
        PeerRef::from_dest_hash([b; 32])
    }

    fn handle(p: u8) -> HubHandle {
        HubHandle::from_raw(u32::from(p))
    }

    fn lost(p: u8) -> HubNotice {
        HubNotice::HandleLost {
            peer: peer(p),
            h: handle(p),
        }
    }

    /// Eşin etkin oturumu `lost(p)`'nin hattında.
    fn on(p: u8) -> ActiveLink {
        ActiveLink::On(handle(p))
    }

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// `drive` turu; `act` her çağrıyı kaydeder ve `out` döner.
    fn turn(g: &mut RelinkGovernor, now: Instant, out: RelinkOutcome) -> Vec<(PeerRef, RelinkOp)> {
        let mut calls = Vec::new();
        g.drive(now, |p, op| {
            calls.push((p, op));
            out
        });
        calls
    }

    /// Sürekli `ROUTE_MISS` (her yenilemeden hemen sonra yeni `HandleLost`; sahte ya da kırık hub):
    /// ilk üç yenileme beklemesiz, sonra aralık 5 s'den ikiye katlanarak 300 s tavanına çıkar
    /// (+ jitter < %25). Sınırsız yenileme fırtınası yok.
    #[test]
    fn route_miss_storm_is_bounded_with_exponential_backoff_and_jitter() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        let mut now = t0;
        let mut starts = Vec::new();
        while now < t0 + s(3600) {
            assert_eq!(
                g.on_notice(now, lost(1), true, on(1)),
                NoticeRoute::Governed
            );
            if !turn(&mut g, now, RelinkOutcome::Started).is_empty() {
                starts.push(now);
            }
            now += Duration::from_millis(250);
        }
        let gaps: Vec<Duration> = starts.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            gaps[0] < s(1) && gaps[1] < s(1),
            "ilk üçü beklemesiz: {gaps:?}"
        );
        let mut base = RELINK_BACKOFF_MIN;
        for (k, gap) in gaps.iter().enumerate().skip(2) {
            assert!(
                *gap >= base && *gap < base + base / 4 + Duration::from_millis(250),
                "aralık {k}: {gap:?}, taban {base:?}"
            );
            base = (base * 2).min(RELINK_BACKOFF_MAX);
        }
        // 1 saatte: 3 serbest + 5,10,…,160 (6) + 300 s tavanında kalan süre (≤ 11).
        assert!(starts.len() <= 20, "bir saatte {} yenileme", starts.len());
        assert_eq!(g.stats().started, starts.len() as u64);
        let distinct: std::collections::BTreeSet<_> = gaps.iter().skip(4).collect();
        assert!(distinct.len() > 1, "jitter: aralıklar eş değil");
    }

    /// `RELINK_QUIET_RESET` boyunca istek gelmezse sayaç sıfırlanır: hattı arada bir emekli olan
    /// eşin yenilemesi yine beklemesizdir.
    #[test]
    fn counter_resets_after_quiet_period() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        for k in 0..RELINK_FREE_STARTS {
            let now = t0 + Duration::from_millis(u64::from(k));
            g.on_notice(now, lost(1), true, on(1));
            assert_eq!(turn(&mut g, now, RelinkOutcome::Started).len(), 1);
        }
        let later = t0 + RELINK_QUIET_RESET + s(1);
        turn(&mut g, later, RelinkOutcome::Started);
        assert!(g.is_empty(), "sessizlik → girdi silindi");
        g.on_notice(later, lost(1), true, on(1));
        assert_eq!(
            turn(&mut g, later, RelinkOutcome::Started),
            vec![(peer(1), RelinkOp::RelinkLine(handle(1)))],
            "sıfırlanan sayaç: beklemesiz"
        );
    }

    /// Yerel tavana takılan istek kaybolmaz: 1 s, 2 s, 4 s … aralıkla yeniden denenir, başarınca
    /// biter. ESKİ DAVRANIŞ: `relink`'in sonucu atılıyordu; `HandleLost` h başına bir kez geldiği
    /// için oturum yeni INIT'siz kalırdı.
    #[test]
    fn deferred_request_is_retried_until_started() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        g.on_notice(t0, lost(1), true, on(1));
        assert_eq!(turn(&mut g, t0, RelinkOutcome::Deferred).len(), 1);
        assert!(turn(
            &mut g,
            t0 + Duration::from_millis(999),
            RelinkOutcome::Deferred
        )
        .is_empty());
        assert_eq!(turn(&mut g, t0 + s(1), RelinkOutcome::Deferred).len(), 1);
        assert!(turn(&mut g, t0 + s(2), RelinkOutcome::Deferred).is_empty());
        assert_eq!(turn(&mut g, t0 + s(3), RelinkOutcome::Deferred).len(), 1);
        assert_eq!(
            turn(&mut g, t0 + s(7), RelinkOutcome::Started),
            vec![(peer(1), RelinkOp::RelinkLine(handle(1)))]
        );
        assert!(turn(&mut g, t0 + s(8), RelinkOutcome::Started).is_empty());
        let st = g.stats();
        assert_eq!((st.requests, st.deferred, st.started), (1, 3, 1));
        assert_eq!(
            retry_gap(64),
            RELINK_BACKOFF_MAX,
            "erteleme aralığı tavanlı"
        );
    }

    /// `AttemptEnded` oturumlu eşte `Restart`; oturumsuz eşte niyetin işidir. `Restart` `Nothing`
    /// dönerse bekleyen `Relink` uygulanır; `Deferred` dönerse istek `Relink` olarak sürer.
    #[test]
    fn notices_route_and_restart_falls_back_to_relink() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        let ended = HubNotice::AttemptEnded { peer: peer(2) };
        assert_eq!(
            g.on_notice(t0, ended, false, ActiveLink::NoSession),
            NoticeRoute::RestartIntent(peer(2))
        );
        assert_eq!(
            g.on_notice(t0, HubNotice::Bound, false, ActiveLink::NoSession),
            NoticeRoute::Ignore
        );
        assert!(g.is_empty());

        g.on_notice(t0, ended, true, ActiveLink::NoSession);
        g.on_notice(t0, lost(2), true, on(2));
        assert_eq!(
            turn(&mut g, t0, RelinkOutcome::Nothing),
            vec![
                (peer(2), RelinkOp::Restart),
                (peer(2), RelinkOp::RelinkLine(handle(2)))
            ]
        );
        assert!(g.is_empty(), "iş çıkmadı, sayaç yok → girdi silinir");

        g.on_notice(t0, ended, true, ActiveLink::NoSession);
        let mut calls = Vec::new();
        g.drive(t0, |p, op| {
            calls.push((p, op));
            RelinkOutcome::Deferred
        });
        assert_eq!(calls, vec![(peer(2), RelinkOp::Restart)]);
        assert_eq!(
            turn(&mut g, t0 + RELINK_RETRY, RelinkOutcome::Started),
            vec![(peer(2), RelinkOp::Relink)]
        );
    }

    /// Tablo sınırlı: isteği olmayan en eski girdi düşer; hepsinin isteği varsa yeni istek düşer.
    #[test]
    fn table_is_bounded() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        for i in 0..MAX_RELINK_PEERS {
            let mut d = [0u8; 32];
            d[..8].copy_from_slice(&u64::try_from(i).unwrap_or(u64::MAX).to_be_bytes());
            let peer = PeerRef::from_dest_hash(d);
            g.on_notice(
                t0,
                HubNotice::AttemptEnded { peer },
                true,
                ActiveLink::NoSession,
            );
        }
        assert_eq!(g.len(), MAX_RELINK_PEERS);
        g.on_notice(t0, lost(0xEE), true, on(0xEE));
        assert_eq!((g.len(), g.stats().dropped), (MAX_RELINK_PEERS, 1));
        // Hepsi başlar (istek biter, sayaç kalır) → yer açılır.
        turn(&mut g, t0, RelinkOutcome::Started);
        g.on_notice(t0 + s(1), lost(0xEE), true, on(0xEE));
        assert_eq!((g.len(), g.stats().dropped), (MAX_RELINK_PEERS, 1));
        assert_eq!(
            turn(&mut g, t0 + s(1), RelinkOutcome::Started),
            vec![(peer(0xEE), RelinkOp::RelinkLine(handle(0xEE)))]
        );
    }

    /// [HLR-STALE] Bildirim anı: etkin oturum h2'deyken eski hattın (h1) kaybı düşer ve sayılır;
    /// istek kaydedilmez, `act` çağrılmaz. Aynı turda önce gelen GEÇERLİ kaybın (h2) bekleyen
    /// isteğini ezmez (yanlış negatif yok): `RelinkLine(h2)` uygulanır. Sonra h2'nin kaybı yine
    /// yeniler.
    #[test]
    fn stale_handle_lost_is_dropped_without_touching_a_valid_request() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        let (h1, h2) = (handle(1), handle(2));
        let stale = HubNotice::HandleLost {
            peer: peer(1),
            h: h1,
        };
        let valid = HubNotice::HandleLost {
            peer: peer(1),
            h: h2,
        };
        assert_eq!(
            g.on_notice(t0, stale, true, ActiveLink::On(h2)),
            NoticeRoute::Stale
        );
        assert!(
            turn(&mut g, t0, RelinkOutcome::Started).is_empty(),
            "iş yok"
        );
        assert!(g.is_empty(), "istek kaydedilmedi");
        let st = g.stats();
        assert_eq!((st.requests, st.stale_handle_lost, st.started), (0, 1, 0));

        assert_eq!(
            g.on_notice(t0, valid, true, ActiveLink::On(h2)),
            NoticeRoute::Governed
        );
        assert_eq!(
            g.on_notice(t0, stale, true, ActiveLink::On(h2)),
            NoticeRoute::Stale
        );
        assert_eq!(
            turn(&mut g, t0, RelinkOutcome::Started),
            vec![(peer(1), RelinkOp::RelinkLine(h2))],
            "geçerli istek ezilmedi"
        );
        let st = g.stats();
        assert_eq!((st.requests, st.stale_handle_lost, st.started), (1, 2, 1));
    }

    /// [HLR-STALE] Uygulama anı: yöneticide bekleyen (geri çekilme / erteleme) `RelinkLine(h)`
    /// arada etkin oturum değiştiyse transport'tan `Stale` alır: istek biter (yeniden denenmez),
    /// `stale_handle_lost` sayılır, `nothing` sayılmaz.
    #[test]
    fn stale_at_drive_time_ends_the_request_and_is_counted() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        g.on_notice(t0, lost(1), true, on(1));
        assert_eq!(
            turn(&mut g, t0, RelinkOutcome::Stale),
            vec![(peer(1), RelinkOp::RelinkLine(handle(1)))]
        );
        assert!(turn(&mut g, t0 + s(1), RelinkOutcome::Started).is_empty());
        let st = g.stats();
        assert_eq!((st.stale_handle_lost, st.nothing, st.started), (1, 0, 0));
    }

    /// [HLR-STALE] Etkin oturumun hattı bilinmiyorsa kayıp DÜŞMEZ: koşulsuz `Relink`
    /// (`handle_lost_unverified`). Bekleyen koşulsuz istek sonradan gelen hatlı istekle
    /// koşullanmaz; `Restart`'ın ertelenen yeniden başlatması da koşulsuzdur.
    #[test]
    fn unverified_handle_lost_is_relinked_unconditionally() {
        let mut g = RelinkGovernor::new(SEED);
        let t0 = Instant::now();
        assert_eq!(
            g.on_notice(t0, lost(1), true, ActiveLink::Unknown),
            NoticeRoute::Governed
        );
        g.on_notice(t0, lost(1), true, on(1));
        assert_eq!(
            turn(&mut g, t0, RelinkOutcome::Started),
            vec![(peer(1), RelinkOp::Relink)]
        );
        let st = g.stats();
        assert_eq!((st.handle_lost_unverified, st.stale_handle_lost), (1, 0));

        g.on_notice(t0, HubNotice::AttemptEnded { peer: peer(2) }, true, on(2));
        g.on_notice(t0, lost(2), true, on(2));
        let mut calls = Vec::new();
        g.drive(t0, |p, op| {
            calls.push((p, op));
            if op == RelinkOp::Restart {
                RelinkOutcome::Deferred
            } else {
                RelinkOutcome::Started
            }
        });
        assert_eq!(calls, vec![(peer(2), RelinkOp::Restart)]);
        assert_eq!(
            turn(&mut g, t0 + RELINK_RETRY, RelinkOutcome::Started),
            vec![(peer(2), RelinkOp::Relink)],
            "ertelenen yeniden başlatma koşulsuz"
        );
    }
}
