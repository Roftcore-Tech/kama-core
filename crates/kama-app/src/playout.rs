//! [YOL3-T1 KK-T1-11] PTT oynatma tamponu — sabit gecikme `T`, boşluk atlamaz (gösterim; sahip
//! kararı v4 III.2; Rust ↔ Kotlin sözleşmesi `docs/design/T1-SERITLER.md` Ek A).
//!
//! Gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.
//!
//! [`crate::ptt::Ptt`]'nin alıcı yolunun ([`Ptt::poll`](crate::ptt::Ptt::poll)) çıktısına oturur;
//! [`Ptt::set_playout`](crate::ptt::Ptt::set_playout) `None` (varsayılan) = bugünkü davranış
//! aynen (demet varışta verilir). Açıkken, akış (eş, `stream_id`) başına:
//!
//! * `PttStarted` HEMEN verilir.
//! * İlk demet `a0` anında vardıysa ilk `PttAudio` `a0 + T`'de; sonrakiler `bundle_ms` aralıkla
//!   (takvim ızgarası: düğüm turu geç kalsa da kadans kaymaz; aynı turda birden çok demet çıkabilir).
//! * Geç demet BEKLENİR, ATLANMAZ: gelince hemen verilir ve takvim ona yeniden çapalanır
//!   (`playout_stalls`, `playout_stall_ms`). Gecikme o akışta durma kadar artar.
//! * `PttEnded` yalnız tampon boşalınca: en erken son demetin veriliş anı + `bundle_ms` (KK-T1-17).
//! * Akış başına en çok [`PLAYOUT_MAX_BUNDLES_PER_STREAM`] demet tutulur; aşılırsa en eski demet
//!   BEKLEMEDEN verilir (düşürme yok; `playout_overflow`).
//!
//! Saat `Ptt`'nin enjekte saatidir (`MonotonicClock`): testler sahte saatle. Zaman çözünürlüğü
//! çağıranın `poll` sıklığıdır (kama-node: `NODE_TICK` = 20 ms).

use core::fmt;
use core::time::Duration;
use std::collections::{BTreeMap, VecDeque};

use kama_contracts::handles::PeerRef;

use crate::ptt::{PttEvent, StreamId};

/// T alt sınırı (v4 III.1: T ∈ [1,5; 2,5] s).
pub const PLAYOUT_DELAY_MIN: Duration = Duration::from_millis(1500);
/// T üst sınırı.
pub const PLAYOUT_DELAY_MAX: Duration = Duration::from_millis(2500);
/// Varsayılan T (v4 III.1: "ölçüm yoksa 2,0 s").
pub const PLAYOUT_DELAY_DEFAULT: Duration = Duration::from_millis(2000);
/// Akış başına tampon tavanı (demet) — `[KALİBRASYON]`, normatif DEĞİL.
pub const PLAYOUT_MAX_BUNDLES_PER_STREAM: usize = 256;

/// Tampon yapılandırması.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayoutConfig {
    delay: Duration,
}

impl PlayoutConfig {
    /// `delay` = T.
    ///
    /// # Errors
    /// [`PlayoutConfigError::DelayOutOfRange`] — T ∉ [[`PLAYOUT_DELAY_MIN`], [`PLAYOUT_DELAY_MAX`]].
    pub fn new(delay: Duration) -> Result<Self, PlayoutConfigError> {
        if delay < PLAYOUT_DELAY_MIN || delay > PLAYOUT_DELAY_MAX {
            return Err(PlayoutConfigError::DelayOutOfRange { delay });
        }
        Ok(Self { delay })
    }

    /// T.
    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }
}

impl Default for PlayoutConfig {
    fn default() -> Self {
        Self {
            delay: PLAYOUT_DELAY_DEFAULT,
        }
    }
}

/// Geçersiz tampon yapılandırması.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayoutConfigError {
    /// T izinli aralığın dışında.
    DelayOutOfRange {
        /// Verilen T.
        delay: Duration,
    },
}

impl fmt::Display for PlayoutConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DelayOutOfRange { delay } => write!(
                f,
                "playout delay {delay:?} outside [{PLAYOUT_DELAY_MIN:?}, {PLAYOUT_DELAY_MAX:?}]"
            ),
        }
    }
}

impl std::error::Error for PlayoutConfigError {}

/// Tampon sayaçları — salt gözlem.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayoutStats {
    /// Takvim anında demeti henüz gelmemiş (beklenen) duraklamalar.
    pub playout_stalls: u64,
    /// Duraklamaların toplam süresi (ms; takvim anından demetin varışına).
    pub playout_stall_ms: u64,
    /// Tavan aşımında beklemeden verilen demetler (düşürme yok).
    pub playout_overflow: u64,
    /// Verilen demetler.
    pub bundles_released: u64,
}

/// Tampondaki demet: varış anı + yük.
struct Held {
    arrived: Duration,
    payload: Vec<u8>,
}

/// Bir akışın tamponu.
struct Stream {
    bundle_ms: Duration,
    queue: VecDeque<Held>,
    /// Sıradaki demetin takvim anı; ilk demet gelmeden `None`.
    next_due: Option<Duration>,
    /// Son verilen demetin veriliş anı.
    last_release: Option<Duration>,
    /// `PttEnded` alındı; tampon boşalınca verilir.
    ended: bool,
}

impl Stream {
    /// Sıradaki olayın anı (demet ya da biten akışın `PttEnded`'i); olay yoksa `None`.
    fn due(&self) -> Option<Duration> {
        if let Some(front) = self.queue.front() {
            let due = self.next_due?;
            return Some(due.max(front.arrived));
        }
        if self.ended {
            return Some(
                self.last_release
                    .map_or(Duration::ZERO, |t| t.saturating_add(self.bundle_ms)),
            );
        }
        None
    }
}

/// Tampon durumu ([`crate::ptt::Ptt`] tutar).
pub(crate) struct Playout {
    cfg: PlayoutConfig,
    streams: BTreeMap<(PeerRef, StreamId), Stream>,
    /// `set_playout(None)` sonrası: bekleyen her şey hemen verilir, boşalınca tampon kalkar.
    draining: bool,
    stats: PlayoutStats,
}

impl Playout {
    pub(crate) fn new(cfg: PlayoutConfig) -> Self {
        Self {
            cfg,
            streams: BTreeMap::new(),
            draining: false,
            stats: PlayoutStats::default(),
        }
    }

    pub(crate) fn stats(&self) -> PlayoutStats {
        self.stats
    }

    pub(crate) fn set_config(&mut self, cfg: PlayoutConfig) {
        self.cfg = cfg;
        self.draining = false;
    }

    /// Kapatma isteği: bekleyen demetler ve `PttEnded`'ler sırayla, beklemeden verilir.
    pub(crate) fn start_draining(&mut self) {
        self.draining = true;
    }

    /// Kapatma isteğinden sonra tampon boşaldı: bekleyen demet ve verilmemiş `PttEnded` yok
    /// (kalan akış kayıtları yalnız takvimdir; bırakılınca sonraki olaylar varışta verilir).
    pub(crate) fn drained(&self) -> bool {
        self.draining
            && self
                .streams
                .values()
                .all(|s| s.queue.is_empty() && !s.ended)
    }

    /// `PttStarted`: akışın tamponu açılır (olay çağırana hemen verilir).
    pub(crate) fn on_started(&mut self, peer: PeerRef, stream_id: StreamId, bundle_ms: u8) {
        if self.draining {
            return;
        }
        self.streams.entry((peer, stream_id)).or_insert(Stream {
            bundle_ms: Duration::from_millis(u64::from(bundle_ms)),
            queue: VecDeque::new(),
            next_due: None,
            last_release: None,
            ended: false,
        });
    }

    /// `PttAudio`: tampona girer. Tamponu olmayan akışın demeti (tampon akış ortasında açıldı ya
    /// da kapatılıyor) ve tavan aşımında en eski demet beklemeden döner.
    pub(crate) fn on_audio(
        &mut self,
        now: Duration,
        peer: PeerRef,
        stream_id: StreamId,
        payload: Vec<u8>,
    ) -> Option<PttEvent> {
        let delay = self.cfg.delay;
        let Some(stream) = self.streams.get_mut(&(peer, stream_id)) else {
            return Some(PttEvent::PttAudio {
                peer,
                stream_id,
                payload,
            });
        };
        if stream.next_due.is_none() && stream.last_release.is_none() {
            stream.next_due = Some(now.saturating_add(delay));
        }
        let mut overflow = None;
        if stream.queue.len() >= PLAYOUT_MAX_BUNDLES_PER_STREAM {
            if let Some(oldest) = stream.queue.pop_front() {
                self.stats.playout_overflow += 1;
                self.stats.bundles_released += 1;
                let slot = stream.next_due.unwrap_or(now);
                stream.next_due = Some(slot.max(now).saturating_add(stream.bundle_ms));
                stream.last_release = Some(now);
                overflow = Some(PttEvent::PttAudio {
                    peer,
                    stream_id,
                    payload: oldest.payload,
                });
            }
        }
        stream.queue.push_back(Held {
            arrived: now,
            payload,
        });
        overflow
    }

    /// `PttEnded`: tampon boşalınca verilir. Tamponu olmayan akışın sonu hemen döner.
    pub(crate) fn on_ended(&mut self, peer: PeerRef, stream_id: StreamId) -> Option<PttEvent> {
        match self.streams.get_mut(&(peer, stream_id)) {
            Some(stream) => {
                stream.ended = true;
                None
            }
            None => Some(PttEvent::PttEnded { peer, stream_id }),
        }
    }

    /// Zamanı gelmiş ilk olay (en erken takvim anı; eşitlikte akış anahtarı sırası).
    pub(crate) fn release_due(&mut self, now: Duration) -> Option<PttEvent> {
        let draining = self.draining;
        let (&key, _) = self
            .streams
            .iter()
            .filter_map(|(k, s)| s.due().map(|d| (k, d)))
            .filter(|&(_, d)| draining || d <= now)
            .min_by_key(|&(_, d)| d)?;
        let (peer, stream_id) = key;
        let stream = self.streams.get_mut(&key)?;
        if let Some(held) = stream.queue.pop_front() {
            let scheduled = stream.next_due.unwrap_or(now);
            if held.arrived > scheduled {
                // Geç demet: beklendi, atlanmadı; takvim ona yeniden çapalanır.
                self.stats.playout_stalls += 1;
                let late = held.arrived.saturating_sub(scheduled);
                self.stats.playout_stall_ms = self
                    .stats
                    .playout_stall_ms
                    .saturating_add(u64::try_from(late.as_millis()).unwrap_or(u64::MAX));
                stream.next_due = Some(now.saturating_add(stream.bundle_ms));
            } else {
                stream.next_due = Some(scheduled.saturating_add(stream.bundle_ms));
            }
            stream.last_release = Some(now);
            self.stats.bundles_released += 1;
            return Some(PttEvent::PttAudio {
                peer,
                stream_id,
                payload: held.payload,
            });
        }
        // Kuyruk boş ve akış bitti.
        self.streams.remove(&key);
        Some(PttEvent::PttEnded { peer, stream_id })
    }
}
