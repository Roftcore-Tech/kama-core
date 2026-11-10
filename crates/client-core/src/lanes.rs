//! [YOL3-T1] İnce şeritler — S1 `send(.., class)`'in sunum katmanı (gösterim; EK-1 D-09,
//! sahip kararı v4 III.2; tasarım notu `docs/design/T1-SERITLER.md`).
//!
//! Gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.
//!
//! # Ne yapar
//!
//! Oturum başına dört sunum kanalı ([`Lane`]): PTT · SIG · TEXT · FILE. Sınıf → şerit eşlemesi
//! YALNIZ S1 parametresinden okunur ([`Lane::of`]); birim baytı ve kayıt türü OKUNMAZ (v4 K3;
//! INV-ENGINE-SOC, EK-1 D-04 notu). Şerit içi FIFO'dur. Pompa ([`SessionLanes::pop_next`])
//! şeritleri kabul sırasıyla (PTT > SIG > TEXT > FILE) boşaltır; FILE şeridi ayrıca pencerelidir:
//! uca verilmiş ve teyidi gelmemiş en çok `W` dosya birimi ([`LaneConfig::file_window_units`],
//! varsayılan [`DEFAULT_FILE_WINDOW_UNITS`]). Teyit, veri yolunun birim başına ilk-ACK olayıdır
//! (`ClientHandle::try_acked`); kimlik veri yolunun sıralı birim kimliğidir.
//!
//! Motora (`adaptive-transport`) hiçbir şey eklenmez; tel, altın vektör ve ürün profili
//! değişmez. Kayıp gönderim hızını DÜŞÜRMEZ (INV-RATE-1 motor kuralıdır; burada hız yoktur):
//! pencere yalnız teyitsiz dosya birimi SAYISINI sınırlar.
//!
//! # Sınırlar (SR-AUDIT-001)
//!
//! Her şerit tavanlıdır ([`LaneConfig::caps`]); tavan doluyken yeni birim reddedilir
//! ([`LaneFull`] → S1 `Backpressure`), tahsis tavan denetiminden SONRA yapılır. Uçuştaki dosya
//! kimlikleri kümesi `W` ile sınırlıdır.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

use kama_contracts::handles::SessionHandle;
use kama_contracts::s1::{CompressionPolicy, DeliveryPolicy, TrafficClass};

/// Gösterim varsayılanı W (v4 K6): teyitsiz dosya birimi sayısı. Türetme ve alt sınır:
/// [`file_window_covers_bdp`] (KK-T1-09).
pub const DEFAULT_FILE_WINDOW_UNITS: u32 = 5;

/// PTT şeridinin varsayılan tavanı (birim) — `[KALİBRASYON]`, normatif DEĞİL.
pub const DEFAULT_PTT_CAP: usize = 64;
/// SIG şeridinin varsayılan tavanı (birim) — `[KALİBRASYON]`.
pub const DEFAULT_SIG_CAP: usize = 64;
/// TEXT şeridinin varsayılan tavanı (birim) — `[KALİBRASYON]`; kama-app TEXT outbox tavanı
/// (`MAX_OUTBOX_UNACKED` = 256) ile aynı mertebe.
pub const DEFAULT_TEXT_CAP: usize = 256;

/// Gösterim profili (v4 III.1): 12,5 kB/s darboğaz.
pub const GOSTER_BOTTLENECK_BYTES_PER_S: u64 = 12_500;
/// Gösterim profili (v4 III.1): RTT 300 ms.
pub const GOSTER_RTT_MS: u64 = 300;

/// Sunum kanalı. Bildirim sırası KABUL sırasıdır (PTT > SIG > TEXT > FILE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lane {
    /// `Vo`, `VoRel` (PTT; D-10 B: Reliable-sıralı).
    Ptt,
    /// `Sig`.
    Sig,
    /// `Be`, `Ctrl` (TEXT, alındılar).
    Text,
    /// `Bk` (dosya); pencereli.
    File,
}

impl Lane {
    /// Kabul sırası.
    pub const ALL: [Self; 4] = [Self::Ptt, Self::Sig, Self::Text, Self::File];

    /// KK-T1-04: sınıf → şerit. Tek eşleme noktası.
    #[must_use]
    pub const fn of(class: TrafficClass) -> Self {
        match class {
            TrafficClass::Vo | TrafficClass::VoRel => Self::Ptt,
            TrafficClass::Sig => Self::Sig,
            TrafficClass::Be | TrafficClass::Ctrl => Self::Text,
            TrafficClass::Bk => Self::File,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Ptt => 0,
            Self::Sig => 1,
            Self::Text => 2,
            Self::File => 3,
        }
    }
}

/// Şerit yapılandırması (KK-T1-05). `ClientTransportConfig::lanes = None` şeritleri kapatır
/// (bugünkü yol).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneConfig {
    /// W: FILE şeridinin uca verdiği ve teyidi gelmemiş en çok birim sayısı (runtime parametresi).
    pub file_window_units: u32,
    /// Şerit tavanları (birim), [`Lane::ALL`] sırasıyla: PTT, SIG, TEXT, FILE.
    pub caps: [usize; 4],
}

impl LaneConfig {
    /// W'li yapılandırma; tavanlar varsayılan, FILE tavanı `2·W`.
    #[must_use]
    pub fn with_file_window(file_window_units: u32) -> Self {
        let file_cap = usize::try_from(file_window_units)
            .unwrap_or(usize::MAX)
            .saturating_mul(2);
        Self {
            file_window_units,
            caps: [DEFAULT_PTT_CAP, DEFAULT_SIG_CAP, DEFAULT_TEXT_CAP, file_cap],
        }
    }

    /// Tipli doğrulama: W ≥ 1, her tavan ≥ 1, tavanlar toplamı ≤ veri yolunun teklif kuyruğu
    /// (`RuntimeConfig::offer_queue`).
    ///
    /// # Errors
    /// [`LaneConfigError`].
    pub fn validate(&self, offer_queue: usize) -> Result<(), LaneConfigError> {
        if self.file_window_units == 0 {
            return Err(LaneConfigError::ZeroWindow);
        }
        for lane in Lane::ALL {
            if self.caps[lane.index()] == 0 {
                return Err(LaneConfigError::ZeroCap { lane });
            }
        }
        let total = self
            .caps
            .iter()
            .fold(0usize, |acc, &c| acc.saturating_add(c));
        if total > offer_queue {
            return Err(LaneConfigError::CapsExceedOfferQueue { total, offer_queue });
        }
        Ok(())
    }
}

impl Default for LaneConfig {
    /// Gösterim varsayılanı: W = 5; tavanlar PTT 64 · SIG 64 · TEXT 256 · FILE 10.
    fn default() -> Self {
        Self::with_file_window(DEFAULT_FILE_WINDOW_UNITS)
    }
}

/// Geçersiz şerit yapılandırması.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LaneConfigError {
    /// W = 0: FILE şeridi hiç birim veremezdi.
    #[error("şerit: dosya penceresi W = 0")]
    ZeroWindow,
    /// Bir şeridin tavanı 0.
    #[error("şerit: {lane:?} tavanı 0")]
    ZeroCap {
        /// Tavanı 0 olan şerit.
        lane: Lane,
    },
    /// Tavanlar toplamı veri yolunun teklif kuyruğunu aşıyor.
    #[error("şerit: tavanlar toplamı {total} > teklif kuyruğu {offer_queue}")]
    CapsExceedOfferQueue {
        /// Tavanlar toplamı.
        total: usize,
        /// `RuntimeConfig::offer_queue`.
        offer_queue: usize,
    },
}

/// KK-T1-09: `W` dosya birimi (her biri en çok `unit_bytes`) motor kuyruğunda en az bir BDP
/// (`bottleneck_bytes_per_s × rtt_ms / 1000`) tutar mı.
#[must_use]
pub const fn file_window_covers_bdp(
    file_window_units: u32,
    unit_bytes: u64,
    bottleneck_bytes_per_s: u64,
    rtt_ms: u64,
) -> bool {
    let window = (file_window_units as u64).saturating_mul(unit_bytes);
    let bdp = bottleneck_bytes_per_s.saturating_mul(rtt_ms) / 1000;
    window >= bdp
}

/// Şerit tavanı dolu (S1 `Backpressure`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneFull;

/// Şeritte bekleyen birim: S1 `send`'in argümanları, aynen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedUnit {
    /// Birim baytları (opak; okunmaz).
    pub bytes: Vec<u8>,
    /// `PreparedUnit::compression_policy`, aynen.
    pub compression_policy: CompressionPolicy,
    /// Teslim politikası, aynen.
    pub policy: DeliveryPolicy,
    /// Sınıf, aynen (uca verilirken S1'e geri verilir).
    pub class: TrafficClass,
}

/// Şerit sayaçları — salt gözlem; karar okumaz.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LaneStats {
    /// Şeritlere kabul edilen birimler, [`Lane::ALL`] sırasıyla.
    pub accepted: [u64; 4],
    /// Şeritten uca verilen birimler, [`Lane::ALL`] sırasıyla.
    pub given: [u64; 4],
    /// Şerit tavanı dolu olduğu için reddedilen birimler (`Backpressure`), [`Lane::ALL`] sırasıyla.
    pub refused: [u64; 4],
    /// Şeritte beklerken süresi dolan `DeadlineBounded` birimler (uca verilmedi).
    pub lane_expired: u64,
    /// Oturumu etkin olmaktan çıktığı (supersede/kapanış) için şeritte düşen birimler — S1 K7:
    /// teslim edilmemiş birimler yeni oturuma TAŞINMAZ.
    pub lane_dropped: u64,
    /// Uç, şeritten verilen birimi `Backpressure` dışı bir hatayla reddetti (savunma; beklenmez).
    pub give_failed: u64,
    /// FILE penceresinden teyitle düşen birimler.
    pub file_acked: u64,
    /// Teyit kanalından okunan bütün olaylar (her sınıf).
    pub acks_drained: u64,
}

/// Bir oturumun şeritleri.
#[derive(Debug)]
pub struct SessionLanes {
    queues: [VecDeque<QueuedUnit>; 4],
    caps: [usize; 4],
    window: usize,
    /// FILE şeridinin uca verdiği, teyidi gelmemiş birim kimlikleri (≤ W).
    file_inflight: BTreeSet<u64>,
}

impl SessionLanes {
    /// Boş şeritler.
    #[must_use]
    pub fn new(cfg: &LaneConfig) -> Self {
        Self {
            queues: Default::default(),
            caps: cfg.caps,
            window: usize::try_from(cfg.file_window_units).unwrap_or(usize::MAX),
            file_inflight: BTreeSet::new(),
        }
    }

    /// Birimi sınıfının şeridine koyar (şerit içi FIFO). Tavan denetimi eklemeden ÖNCE.
    ///
    /// # Errors
    /// [`LaneFull`] — şerit tavanda; birim alınmadı.
    pub fn push(&mut self, unit: QueuedUnit) -> Result<Lane, LaneFull> {
        let lane = Lane::of(unit.class);
        let q = &mut self.queues[lane.index()];
        if q.len() >= self.caps[lane.index()] {
            return Err(LaneFull);
        }
        q.push_back(unit);
        Ok(lane)
    }

    /// Uç geri bastı: birim şeridinin BAŞINA döner (sıra korunur). Tavan denetimi yoktur: birim
    /// az önce bu şeritten çıktı.
    pub fn push_front(&mut self, lane: Lane, unit: QueuedUnit) {
        self.queues[lane.index()].push_front(unit);
    }

    /// Kabul sırasıyla uca verilecek sıradaki birim. FILE şeridi pencere doluysa atlanır.
    /// Süresi dolmuş `DeadlineBounded` birim çıkarılır ve `expired`'da sayılır (uca verilmez).
    pub fn pop_next(&mut self, now: Instant, expired: &mut u64) -> Option<(Lane, QueuedUnit)> {
        for lane in Lane::ALL {
            if lane == Lane::File && self.file_inflight.len() >= self.window {
                continue;
            }
            let q = &mut self.queues[lane.index()];
            while let Some(unit) = q.pop_front() {
                if let DeliveryPolicy::DeadlineBounded { deadline } = unit.policy {
                    if deadline <= now {
                        *expired = expired.saturating_add(1);
                        continue;
                    }
                }
                return Some((lane, unit));
            }
        }
        None
    }

    /// Birim uca verildi; FILE ise kimliği pencereye girer.
    pub fn on_given(&mut self, lane: Lane, unit_id: u64) {
        if lane == Lane::File {
            self.file_inflight.insert(unit_id);
        }
    }

    /// Birim teyidi (veri yolunun ilk-ACK olayı). Pencereden düştüyse `true`.
    pub fn on_acked(&mut self, unit_id: u64) -> bool {
        self.file_inflight.remove(&unit_id)
    }

    /// Bekleyen birim yok (pencere sayılmaz: uca verilmiş birim ucundur).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(VecDeque::is_empty)
    }

    /// Bekleyen birim sayısı.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queues.iter().map(VecDeque::len).sum()
    }

    /// Şeridin bekleyen birim sayısı.
    #[must_use]
    pub fn lane_len(&self, lane: Lane) -> usize {
        self.queues[lane.index()].len()
    }

    /// FILE penceresindeki (uca verilmiş, teyitsiz) birim sayısı.
    #[must_use]
    pub fn file_inflight(&self) -> usize {
        self.file_inflight.len()
    }
}

/// Adaptörün şerit durumu: yapılandırma, oturum başına şeritler, sayaçlar.
#[derive(Debug)]
pub struct LaneState {
    cfg: LaneConfig,
    sessions: BTreeMap<SessionHandle, SessionLanes>,
    stats: LaneStats,
}

impl LaneState {
    /// Boş durum.
    #[must_use]
    pub fn new(cfg: LaneConfig) -> Self {
        Self {
            cfg,
            sessions: BTreeMap::new(),
            stats: LaneStats::default(),
        }
    }

    /// Yapılandırma.
    #[must_use]
    pub fn config(&self) -> &LaneConfig {
        &self.cfg
    }

    /// Sayaçlar.
    #[must_use]
    pub fn stats(&self) -> LaneStats {
        self.stats
    }

    /// Birimi oturumun şeridine koyar (oturumun şeritleri ilk birimde açılır).
    ///
    /// # Errors
    /// [`LaneFull`] — şerit tavanda.
    pub fn enqueue(&mut self, session: SessionHandle, unit: QueuedUnit) -> Result<(), LaneFull> {
        let lane = Lane::of(unit.class);
        let cfg = self.cfg;
        let lanes = self
            .sessions
            .entry(session)
            .or_insert_with(|| SessionLanes::new(&cfg));
        match lanes.push(unit) {
            Ok(_) => {
                self.stats.accepted[lane.index()] += 1;
                Ok(())
            }
            Err(LaneFull) => {
                self.stats.refused[lane.index()] += 1;
                Err(LaneFull)
            }
        }
    }

    /// Oturumun sıradaki birimi ([`SessionLanes::pop_next`]); süresi dolanlar sayılır.
    pub fn pop_next(&mut self, session: SessionHandle, now: Instant) -> Option<(Lane, QueuedUnit)> {
        let lanes = self.sessions.get_mut(&session)?;
        lanes.pop_next(now, &mut self.stats.lane_expired)
    }

    /// Birim uca verildi.
    pub fn on_given(&mut self, session: SessionHandle, lane: Lane, unit_id: u64) {
        self.stats.given[lane.index()] += 1;
        if let Some(lanes) = self.sessions.get_mut(&session) {
            lanes.on_given(lane, unit_id);
        }
    }

    /// Uç geri bastı: birim şeridinin başına döner.
    pub fn push_front(&mut self, session: SessionHandle, lane: Lane, unit: QueuedUnit) {
        if let Some(lanes) = self.sessions.get_mut(&session) {
            lanes.push_front(lane, unit);
        }
    }

    /// Birimin deadline'ı şeritten uca verilirken doldu (uç `DeadlineExpired`); `lane_expired`.
    pub fn on_expired(&mut self) {
        self.stats.lane_expired += 1;
    }

    /// Uç, şeritten verilen birimi geri basma dışı bir hatayla reddetti.
    pub fn on_give_failed(&mut self) {
        self.stats.give_failed += 1;
    }

    /// Teyit olayı.
    pub fn on_acked(&mut self, session: SessionHandle, unit_id: u64) {
        self.stats.acks_drained += 1;
        if let Some(lanes) = self.sessions.get_mut(&session) {
            if lanes.on_acked(unit_id) {
                self.stats.file_acked += 1;
            }
        }
    }

    /// Etkin olmayan oturumların şeritlerini bırakır; bekleyen birimleri `lane_dropped`'ta sayılır.
    pub fn retain_active(&mut self, mut active: impl FnMut(SessionHandle) -> bool) {
        let dropped = &mut self.stats.lane_dropped;
        self.sessions.retain(|&h, lanes| {
            let keep = active(h);
            if !keep {
                *dropped = dropped.saturating_add(lanes.len() as u64);
            }
            keep
        });
    }

    /// Şeritlerinde bekleyen birim olan oturumlar.
    #[must_use]
    pub fn sessions_with_work(&self) -> Vec<SessionHandle> {
        self.sessions
            .iter()
            .filter(|(_, l)| !l.is_empty())
            .map(|(&h, _)| h)
            .collect()
    }

    /// Oturumun şeritleri boş mu (şeridi hiç açılmadıysa da boş).
    #[must_use]
    pub fn is_empty(&self, session: SessionHandle) -> bool {
        self.sessions
            .get(&session)
            .is_none_or(SessionLanes::is_empty)
    }

    /// Oturumun şeritleri (gözlem; yalnız okuma).
    #[must_use]
    pub fn session(&self, session: SessionHandle) -> Option<&SessionLanes> {
        self.sessions.get(&session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const UNIT_BYTES: u64 = 1013;

    fn unit(class: TrafficClass, tag: u8) -> QueuedUnit {
        QueuedUnit {
            bytes: vec![tag],
            compression_policy: CompressionPolicy::Never,
            policy: DeliveryPolicy::Reliable,
            class,
        }
    }

    /// Pompa modeli: uç her birimi kabul eder ve sırayla kimlik verir (veri yolu `next_unit`).
    fn drain(l: &mut SessionLanes, now: Instant, next_id: &mut u64) -> Vec<(Lane, u8, u64)> {
        let mut out = Vec::new();
        let mut expired = 0;
        while let Some((lane, u)) = l.pop_next(now, &mut expired) {
            let id = *next_id;
            *next_id += 1;
            l.on_given(lane, id);
            out.push((lane, u.bytes[0], id));
        }
        out
    }

    #[test]
    fn t_l1_acceptance_order_is_ptt_sig_text_file() {
        let mut l = SessionLanes::new(&LaneConfig::default());
        for (i, class) in [
            TrafficClass::Bk,
            TrafficClass::Be,
            TrafficClass::Sig,
            TrafficClass::VoRel,
            TrafficClass::Ctrl,
            TrafficClass::Vo,
        ]
        .into_iter()
        .enumerate()
        {
            l.push(unit(class, i as u8)).expect("tavan altı");
        }
        let mut id = 0;
        let got: Vec<(Lane, u8)> = drain(&mut l, Instant::now(), &mut id)
            .into_iter()
            .map(|(lane, tag, _)| (lane, tag))
            .collect();
        assert_eq!(
            got,
            [
                (Lane::Ptt, 3),
                (Lane::Ptt, 5),
                (Lane::Sig, 2),
                (Lane::Text, 1),
                (Lane::Text, 4),
                (Lane::File, 0),
            ],
            "kabul sırası PTT > SIG > TEXT > FILE; şerit içi FIFO"
        );
    }

    #[test]
    fn t_l2_file_window_limits_unacked_units_and_ack_releases_one() {
        let cfg = LaneConfig {
            file_window_units: 5,
            caps: [64, 64, 256, 32],
        };
        let mut l = SessionLanes::new(&cfg);
        for i in 0..20u8 {
            l.push(unit(TrafficClass::Bk, i)).expect("tavan altı");
        }
        let mut id = 100;
        let first = drain(&mut l, Instant::now(), &mut id);
        assert_eq!(first.len(), 5, "teyit yokken uca en çok W = 5 dosya birimi");
        assert_eq!(l.file_inflight(), 5);
        assert!(
            drain(&mut l, Instant::now(), &mut id).is_empty(),
            "pencere dolu"
        );
        assert!(
            l.on_acked(first[0].2),
            "ilk birimin teyidi pencereden düşer"
        );
        assert!(!l.on_acked(first[0].2), "aynı teyit ikinci kez düşürmez");
        let next = drain(&mut l, Instant::now(), &mut id);
        assert_eq!(next.len(), 1, "bir teyit → bir birim daha");
        assert_eq!(next[0].1, 5, "sıra korunur");
        // TEXT pencereden etkilenmez.
        l.push(unit(TrafficClass::Be, 0xEE)).expect("tavan altı");
        let text = drain(&mut l, Instant::now(), &mut id);
        assert_eq!(text.len(), 1);
        assert_eq!(text[0].0, Lane::Text);
    }

    /// T-L3: teyit kanalı (veri yolu: `mpsc::channel(4096)` + `try_send`, dolunca olay düşer)
    /// boşaltılmazsa 4096 teyitten sonra FILE teyitleri kaybolur ve pencere KİLİTLENİR;
    /// her pompa turunda boşaltılırsa ≥ 5000 birimlik akış takılmadan biter.
    #[test]
    fn t_l3_ack_channel_drain_prevents_window_lock_past_4096() {
        fn run(drain_acks: bool) -> (usize, bool) {
            const ACK_CHANNEL: usize = 4096;
            const TOTAL: usize = 5000;
            let (tx, mut rx) = tokio::sync::mpsc::channel::<u64>(ACK_CHANNEL);
            let mut l = SessionLanes::new(&LaneConfig::default());
            let mut next_id = 0u64;
            let mut text_left = TOTAL;
            let mut file_left = TOTAL;
            let mut file_done = 0usize;
            let mut turns = 0usize;
            let mut expired = 0;
            // Her turda: 1 TEXT + 1 FILE sunulur; uca verilen her birim aynı turda teyit alır
            // (veri yolu `try_send`). Boşaltma açıksa tur başında kanal okunur.
            while file_done < TOTAL && turns < 4 * TOTAL {
                turns += 1;
                if drain_acks {
                    while let Ok(id) = rx.try_recv() {
                        if l.on_acked(id) {
                            file_done += 1;
                        }
                    }
                }
                if text_left > 0 && l.push(unit(TrafficClass::Be, 0)).is_ok() {
                    text_left -= 1;
                }
                if file_left > 0 && l.push(unit(TrafficClass::Bk, 1)).is_ok() {
                    file_left -= 1;
                }
                while let Some((lane, _)) = l.pop_next(Instant::now(), &mut expired) {
                    let id = next_id;
                    next_id += 1;
                    l.on_given(lane, id);
                    let _ = tx.try_send(id); // dolunca SESSİZCE düşer (runtime.rs:2136/2145)
                }
                if !drain_acks && l.file_inflight() >= DEFAULT_FILE_WINDOW_UNITS as usize {
                    // Boşaltma yok: pencere teyitsiz kalır.
                    return (file_done, true);
                }
            }
            (file_done, file_done < TOTAL)
        }
        let (done, stuck) = run(true);
        assert!(
            !stuck && done == 5000,
            "boşaltma açık: takılma yok ({done})"
        );
        let (_, stuck_off) = run(false);
        assert!(
            stuck_off,
            "boşaltma kapalı: pencere kilitlenir (eski davranış)"
        );
    }

    /// T-L3 ek: teyitler GEÇ okunursa (kanal 4096'yı aşacak kadar biriktikten sonra) FILE
    /// teyitleri düşmüştür ve pencere kalıcı kilitlenir — boşaltmanın her turda olması şart.
    #[test]
    fn t_l3_late_drain_after_overflow_locks_the_window() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<u64>(4096);
        let mut l = SessionLanes::new(&LaneConfig::default());
        let mut expired = 0;
        let mut id = 0u64;
        // 4096 TEXT teyidi kanalı doldurur …
        for _ in 0..4096 {
            l.push(unit(TrafficClass::Be, 0)).ok();
            let (lane, _) = l.pop_next(Instant::now(), &mut expired).expect("TEXT");
            l.on_given(lane, id);
            assert!(tx.try_send(id).is_ok());
            id += 1;
        }
        // … sonra W dosya birimi verilir; teyitleri dolu kanalda düşer.
        for _ in 0..5 {
            l.push(unit(TrafficClass::Bk, 1)).expect("tavan altı");
        }
        while let Some((lane, _)) = l.pop_next(Instant::now(), &mut expired) {
            l.on_given(lane, id);
            assert!(tx.try_send(id).is_err(), "kanal dolu: olay düşer");
            id += 1;
        }
        while let Ok(a) = rx.try_recv() {
            l.on_acked(a);
        }
        assert_eq!(l.file_inflight(), 5, "düşen teyit pencereyi asla bırakmaz");
    }

    #[test]
    fn t_l4_lane_is_read_from_class_only_not_bytes() {
        let mut l = SessionLanes::new(&LaneConfig::default());
        let same = |class| QueuedUnit {
            bytes: vec![0x01, 0x02, 0x03],
            compression_policy: CompressionPolicy::Never,
            policy: DeliveryPolicy::Reliable,
            class,
        };
        let lanes: Vec<Lane> = TrafficClass::ALL
            .into_iter()
            .map(|c| l.push(same(c)).expect("tavan altı"))
            .collect();
        assert_eq!(
            lanes,
            [
                Lane::Text,
                Lane::Text,
                Lane::Sig,
                Lane::File,
                Lane::Ptt,
                Lane::Ptt
            ],
            "aynı bayt, farklı sınıf → farklı şerit"
        );
    }

    #[test]
    fn t_l5_full_lane_refuses_and_memory_is_bounded_by_cap() {
        let cfg = LaneConfig {
            file_window_units: 1,
            caps: [2, 1, 3, 2],
        };
        let mut st = LaneState::new(cfg);
        let h = SessionHandle::from_raw(9);
        for _ in 0..3 {
            assert_eq!(st.enqueue(h, unit(TrafficClass::Be, 0)), Ok(()));
        }
        assert_eq!(st.enqueue(h, unit(TrafficClass::Be, 0)), Err(LaneFull));
        assert_eq!(st.enqueue(h, unit(TrafficClass::Ctrl, 0)), Err(LaneFull));
        assert_eq!(st.enqueue(h, unit(TrafficClass::VoRel, 0)), Ok(()));
        assert_eq!(st.session(h).map(SessionLanes::len), Some(4));
        assert_eq!(st.session(h).map(|l| l.lane_len(Lane::Text)), Some(3));
        let s = st.stats();
        assert_eq!(s.accepted, [1, 0, 3, 0]);
        assert_eq!(s.refused, [0, 0, 2, 0]);
    }

    #[test]
    fn t_l9_default_window_covers_one_bdp_and_three_does_not() {
        let cfg = LaneConfig::default();
        assert_eq!(cfg.file_window_units, 5);
        assert!(file_window_covers_bdp(
            cfg.file_window_units,
            UNIT_BYTES,
            GOSTER_BOTTLENECK_BYTES_PER_S,
            GOSTER_RTT_MS
        ));
        assert!(
            file_window_covers_bdp(4, UNIT_BYTES, 12_500, 300),
            "W = 4 alt sınır (4052 B ≥ 3750 B)"
        );
        assert!(
            !file_window_covers_bdp(3, UNIT_BYTES, 12_500, 300),
            "W = 3 (3039 B) BDP'yi tutmaz"
        );
        assert_eq!(cfg.caps, [64, 64, 256, 10], "FILE tavanı 2·W");
        assert_eq!(cfg.validate(1024), Ok(()));
    }

    #[test]
    fn lane_config_validation_is_typed() {
        assert_eq!(
            LaneConfig::with_file_window(0).validate(1024),
            Err(LaneConfigError::ZeroWindow)
        );
        let mut c = LaneConfig::default();
        c.caps[1] = 0;
        assert_eq!(
            c.validate(1024),
            Err(LaneConfigError::ZeroCap { lane: Lane::Sig })
        );
        let c = LaneConfig::default();
        assert_eq!(
            c.validate(100),
            Err(LaneConfigError::CapsExceedOfferQueue {
                total: 394,
                offer_queue: 100
            })
        );
    }

    #[test]
    fn t_l10_expired_deadline_unit_is_dropped_and_counted_before_give() {
        let mut l = SessionLanes::new(&LaneConfig::default());
        let t0 = Instant::now();
        let mut u = unit(TrafficClass::Be, 7);
        u.policy = DeliveryPolicy::DeadlineBounded {
            deadline: t0 + Duration::from_millis(10),
        };
        l.push(u).expect("tavan altı");
        l.push(unit(TrafficClass::Be, 8)).expect("tavan altı");
        let mut expired = 0;
        let got = l.pop_next(t0 + Duration::from_millis(10), &mut expired);
        assert_eq!(expired, 1, "süresi dolan birim sayıldı");
        assert_eq!(
            got.map(|(_, u)| u.bytes[0]),
            Some(8),
            "uca yalnız canlı birim"
        );
    }

    #[test]
    fn retain_active_drops_and_counts_pending_units() {
        let mut st = LaneState::new(LaneConfig::default());
        let (a, b) = (SessionHandle::from_raw(1), SessionHandle::from_raw(2));
        st.enqueue(a, unit(TrafficClass::Bk, 0))
            .expect("tavan altı");
        st.enqueue(a, unit(TrafficClass::Be, 0))
            .expect("tavan altı");
        st.enqueue(b, unit(TrafficClass::Be, 0))
            .expect("tavan altı");
        st.retain_active(|h| h == b);
        assert!(st.is_empty(a));
        assert!(!st.is_empty(b));
        assert_eq!(st.stats().lane_dropped, 2);
        assert_eq!(st.sessions_with_work(), [b]);
    }
}
