//! [YOL3-T1 KK-T1-11, KK-T1-19, KK-T1-20] PTT oynatma tamponu — sabit gecikme `T`, boşluk atlamaz
//! (gösterim; sahip kararı v4 III.2 ve EK-A §3, §10; Rust ↔ Kotlin sözleşmesi
//! `docs/design/T1-SERITLER.md` Ek A v2).
//!
//! Gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.
//!
//! [`crate::ptt::Ptt`]'nin alıcı yolunun ([`Ptt::poll`](crate::ptt::Ptt::poll)) çıktısına oturur;
//! [`Ptt::set_playout`](crate::ptt::Ptt::set_playout) `None` (varsayılan) = bugünkü davranış
//! aynen (demet varışta verilir). Açıkken, yaşam döngüsü başına (anahtar (eş, `stream_id`);
//! `PttStarted` … `PttEnded`):
//!
//! * `PttStarted` HEMEN verilir (aynı anahtarın önceki yaşam döngüsü tamponda sürüyorsa önce o
//!   kapanır — aşağıda "yerel sonlandırma"; iki yaşam döngüsü hiçbir zaman iç içe geçmez).
//! * Plan: ilk demet `a0`'da vardıysa `i`'inci demet en erken `a0 + T + i·bundle_ms`'te verilir
//!   (takvim ızgarası: düğüm turu geç kalsa da kadans kaymaz; aynı turda birden çok demet çıkabilir).
//!   **Hiçbir demet planından ERKEN verilmez** (EK-A §10); geç verilebilir.
//! * Geç demet BEKLENİR, ATLANMAZ: gelince hemen verilir ve takvim ona yeniden çapalanır
//!   (`playout_stalls`, `playout_stall_ms`). Gecikme o akışta durma kadar artar (açık gösterim
//!   tercihi, EK-A §3.5).
//! * `PttEnded` yalnız tampon boşalınca: en erken son demetin veriliş anı + `bundle_ms` (KK-T1-17).
//! * **Yerel sonlandırma** (EK-A §10 politikası; Kotlin tarafındakiyle aynı kural): (a) akışın
//!   tamponu [`PLAYOUT_MAX_BUNDLES_PER_STREAM`] demete ulaşmışken yeni demet gelirse (KK-T1-19);
//!   (b) oturum değişiminde — eşin güncel oturumu yaşam döngüsünün başladığı oturumdan farklıysa ya
//!   da aynı anahtara yeni yaşam döngüsü gelirse (KK-T1-20, EK-A §3.6); (c) akış tablosu
//!   [`PLAYOUT_MAX_STREAMS`]'e ulaşınca Ptt'de bitmiş, tamponu çalan en yakın bitişli akış. Akışın
//!   çalınmamış demetleri atılır (`playout_discarded`), `PttEnded` HEMEN verilir; akış Ptt'de hâlâ
//!   etkinse sonraki demetleri elenir (`playout_late_eliminated`) ve Ptt'nin kendi `PttEnded`'i
//!   yutulur (yaşam döngüsü başına tam bir `PttEnded`). Yaşam döngüsü olayı DÜŞMEZ; atılan ve elenen
//!   her demet sayılır.
//! * Tampon akış ortasında açılırsa ya da kapatılırken ([`Ptt::set_playout`](crate::ptt::Ptt::set_playout)
//!   `None`) yeni başlayan akış tamponlanmaz: plan yalnız tampon açıkken başlayan yaşam döngüsünündür.
//!   Kapatma isteğinde tampondaki akışlar planlarıyla biter; tablo boşalınca tampon kalkar.
//!
//! Saat `Ptt`'nin enjekte saatidir (`MonotonicClock`): testler sahte saatle. Zaman çözünürlüğü
//! çağıranın `poll` sıklığıdır (kama-node: `NODE_TICK` = 20 ms).
//!
//! Bellek tavanı (SR-AUDIT-001): akış başına [`PLAYOUT_MAX_BUNDLES_PER_STREAM`] demet, akış tablosu
//! [`PLAYOUT_MAX_STREAMS`]; yerel sonlandırılmış etkin akış kümesi Ptt'nin etkin gelen akışlarının
//! alt kümesidir (`MAX_PEERS × PTT_MAX_ACTIVE_STREAMS`); hazır olay kuyruğu her adımda en çok tablo
//! boyu + 1 olay alır ve bir sonraki `poll`'larda ilk önce boşalır.

use core::fmt;
use core::time::Duration;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use kama_contracts::handles::{PeerRef, SessionHandle};

use crate::messaging::MAX_PEERS;
use crate::ptt::{PttEvent, StreamId, PTT_MAX_ACTIVE_STREAMS};

/// T alt sınırı (v4 III.1: T ∈ [1,5; 2,5] s).
pub const PLAYOUT_DELAY_MIN: Duration = Duration::from_millis(1500);
/// T üst sınırı.
pub const PLAYOUT_DELAY_MAX: Duration = Duration::from_millis(2500);
/// Varsayılan T (v4 III.1: "ölçüm yoksa 2,0 s").
pub const PLAYOUT_DELAY_DEFAULT: Duration = Duration::from_millis(2000);
/// Akış başına tampon tavanı (demet) — `[KALİBRASYON]`, normatif DEĞİL. Kotlin tarafının zaman
/// tabanlı tavanı (256 × `bundleMs`, EK-A §10) ile aynı büyüklük.
pub const PLAYOUT_MAX_BUNDLES_PER_STREAM: usize = 256;
/// Akış tablosu tavanı: Ptt'nin gelen akış tavanının (`MAX_PEERS × PTT_MAX_ACTIVE_STREAMS`) iki
/// katı — etkin akış yuvası başına bir de Ptt'de bitmiş, tamponu henüz çalan akış (türetilmiş; yeni
/// sayı yok). Etkin akışlar tablonun en çok yarısıdır; tablo dolunca biten akışlardan biri yerel
/// sonlandırılır.
pub const PLAYOUT_MAX_STREAMS: usize = 2 * MAX_PEERS * PTT_MAX_ACTIVE_STREAMS;

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
    /// Akış tavanı ([`PLAYOUT_MAX_BUNDLES_PER_STREAM`]) aşımıyla yerel sonlandırılan yaşam
    /// döngüleri (KK-T1-19).
    pub playout_overflow: u64,
    /// Oturum değişimiyle yerel sonlandırılan yaşam döngüleri (KK-T1-20).
    pub playout_session_closes: u64,
    /// Akış tablosu tavanı ([`PLAYOUT_MAX_STREAMS`]) yüzünden yerel sonlandırılan, Ptt'de bitmiş
    /// yaşam döngüleri.
    pub playout_table_closes: u64,
    /// Yerel sonlandırmada çalınmadan atılan demetler (tampondakiler + sonlandırmayı tetikleyen).
    pub playout_discarded: u64,
    /// Yerel sonlandırılmış (ya da Ptt'de bitmiş) yaşam döngüsüne sonradan gelip elenen demetler.
    pub playout_late_eliminated: u64,
    /// Verilen demetler.
    pub bundles_released: u64,
    /// Bir akış tamponunun ulaştığı en yüksek doluluk (demet; EK-A §6.5).
    pub playout_peak_bundles: u64,
}

/// Yaşam döngüsü anahtarı.
type Key = (PeerRef, StreamId);

/// Yerel sonlandırma nedeni (yalnız sayaç seçer).
#[derive(Clone, Copy)]
enum Termination {
    Overflow,
    Session,
    Table,
}

/// Tampondaki demet: varış anı + yük.
struct Held {
    arrived: Duration,
    payload: Vec<u8>,
}

/// Bir yaşam döngüsünün tamponu.
struct Stream {
    bundle_ms: Duration,
    /// Yaşam döngüsü başladığında eşin güncel oturumu (KK-T1-20).
    session: Option<SessionHandle>,
    queue: VecDeque<Held>,
    /// Sıradaki demetin takvim anı; ilk demet gelmeden `None`.
    next_due: Option<Duration>,
    /// Son verilen demetin veriliş anı.
    last_release: Option<Duration>,
    /// Ptt `PttEnded` verdi; tampon boşalınca verilir.
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
    streams: BTreeMap<Key, Stream>,
    /// Yerel sonlandırılmış ama Ptt'de hâlâ etkin yaşam döngüleri: demetleri elenir, Ptt'nin
    /// `PttEnded`'i yutulur.
    terminated: BTreeSet<Key>,
    /// Takvim beklemeyen yaşam döngüsü olayları (yerel `PttEnded`, ardından gelen `PttStarted`),
    /// sırayla; her `poll`'da tampon olaylarından önce verilir.
    ready: VecDeque<PttEvent>,
    /// `set_playout(None)` sonrası: yeni yaşam döngüsü tamponlanmaz; tablo boşalınca tampon kalkar.
    draining: bool,
    stats: PlayoutStats,
}

impl Playout {
    pub(crate) fn new(cfg: PlayoutConfig) -> Self {
        Self {
            cfg,
            streams: BTreeMap::new(),
            terminated: BTreeSet::new(),
            ready: VecDeque::new(),
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

    /// Kapatma isteği: yeni yaşam döngüsü tamponlanmaz; tampondakiler planlarıyla biter.
    pub(crate) fn start_draining(&mut self) {
        self.draining = true;
    }

    /// Kapatma isteğinden sonra tampon boşaldı: akış, yerel sonlandırılmış etkin akış ve bekleyen
    /// olay yok.
    pub(crate) fn drained(&self) -> bool {
        self.draining
            && self.streams.is_empty()
            && self.terminated.is_empty()
            && self.ready.is_empty()
    }

    /// Savunma: yerel sonlandırılmış anahtarlar Ptt'nin etkin gelen akışlarının alt kümesidir (Ptt
    /// her etkin akışını bir `PttEnded` ile bitirir ve o olay [`Self::on_ended`]'den geçer).
    pub(crate) fn retain_terminated(&mut self, active_in_ptt: impl Fn(&Key) -> bool) {
        self.terminated.retain(|key| active_in_ptt(key));
    }

    /// KK-T1-20 (EK-A §3.6): eşin güncel oturumu yaşam döngüsünün başladığı oturumdan FARKLI bir
    /// oturumsa (supersession ya da kapanış + yeniden bağlanma) yaşam döngüsü yerel sonlandırılır.
    /// Eşin güncel oturumu yoksa (oturum sonu, yenisi henüz yok) bir şey yapılmaz: Ptt akışı kendi
    /// `PttEnded`'iyle bitirir ve tamponda kalan demetler planlarıyla çalınır.
    pub(crate) fn close_superseded(
        &mut self,
        session_of: impl Fn(PeerRef) -> Option<SessionHandle>,
    ) {
        let changed: Vec<Key> = self
            .streams
            .iter()
            .filter(|((peer, _), stream)| {
                session_of(*peer).is_some_and(|now| stream.session.is_some_and(|s| s != now))
            })
            .map(|(&key, _)| key)
            .collect();
        for key in changed {
            self.terminate(key, Termination::Session);
        }
    }

    /// Takvim beklemeyen sıradaki yaşam döngüsü olayı.
    pub(crate) fn next_ready(&mut self) -> Option<PttEvent> {
        self.ready.pop_front()
    }

    /// `PttStarted`: yaşam döngüsünün tamponu açılır; olay [`Self::next_ready`]'den verilir. Aynı
    /// anahtarın önceki yaşam döngüsü tablodaysa önce o yerel sonlandırılır (KK-T1-20): onun
    /// `PttEnded`'i bu `PttStarted`'dan ÖNCE verilir.
    pub(crate) fn on_started(
        &mut self,
        event: PttEvent,
        key: Key,
        bundle_ms: u8,
        session: Option<SessionHandle>,
    ) {
        if self.streams.contains_key(&key) {
            self.terminate(key, Termination::Session);
        }
        // Ptt etkin bir anahtarı yeniden başlatmaz (PTT-R1); yine de eski işaret yeni yaşam
        // döngüsünün demetlerini elemesin.
        self.terminated.remove(&key);
        if !self.draining {
            if self.streams.len() >= PLAYOUT_MAX_STREAMS {
                self.evict_ended();
            }
            // Tablo yalnız etkin akışlarla dolu olamaz (tavan türetmesi); olursa akış tamponsuz geçer.
            if self.streams.len() < PLAYOUT_MAX_STREAMS {
                self.streams.insert(
                    key,
                    Stream {
                        bundle_ms: Duration::from_millis(u64::from(bundle_ms)),
                        session,
                        queue: VecDeque::new(),
                        next_due: None,
                        last_release: None,
                        ended: false,
                    },
                );
            }
        }
        self.ready.push_back(event);
    }

    /// `PttAudio`: tampona girer. Yerel sonlandırılmış yaşam döngüsünün demeti elenir; oturum
    /// değişiminde ya da akış tavanında yaşam döngüsü yerel sonlandırılır (olay
    /// [`Self::next_ready`]'den). Tamponu olmayan akışın demeti (plan yok: tampon akış ortasında
    /// açıldı ya da kapatılıyor) beklemeden döner.
    pub(crate) fn on_audio(
        &mut self,
        now: Duration,
        key: Key,
        payload: Vec<u8>,
        session: Option<SessionHandle>,
    ) -> Option<PttEvent> {
        if self.terminated.contains(&key) {
            self.stats.playout_late_eliminated += 1;
            return None;
        }
        let delay = self.cfg.delay;
        let Some(stream) = self.streams.get_mut(&key) else {
            let (peer, stream_id) = key;
            return Some(PttEvent::PttAudio {
                peer,
                stream_id,
                payload,
            });
        };
        if stream.ended {
            // Savunma: Ptt biten akışa demet vermez (PTT-R3).
            self.stats.playout_late_eliminated += 1;
            return None;
        }
        if session.is_some_and(|now| stream.session.is_some_and(|s| s != now)) {
            self.stats.playout_discarded += 1;
            self.terminate(key, Termination::Session);
            return None;
        }
        if stream.queue.len() >= PLAYOUT_MAX_BUNDLES_PER_STREAM {
            self.stats.playout_discarded += 1;
            self.terminate(key, Termination::Overflow);
            return None;
        }
        if stream.next_due.is_none() && stream.last_release.is_none() {
            stream.next_due = Some(now.saturating_add(delay));
        }
        stream.queue.push_back(Held {
            arrived: now,
            payload,
        });
        let depth = u64::try_from(stream.queue.len()).unwrap_or(u64::MAX);
        self.stats.playout_peak_bundles = self.stats.playout_peak_bundles.max(depth);
        None
    }

    /// `PttEnded`: tampon boşalınca verilir. Yerel sonlandırılmış yaşam döngüsünün `PttEnded`'i
    /// zaten verildi (yutulur); tamponu olmayan akışın sonu hemen döner.
    pub(crate) fn on_ended(&mut self, key: Key) -> Option<PttEvent> {
        if self.terminated.remove(&key) {
            return None;
        }
        if let Some(stream) = self.streams.get_mut(&key) {
            stream.ended = true;
            return None;
        }
        let (peer, stream_id) = key;
        Some(PttEvent::PttEnded { peer, stream_id })
    }

    /// Zamanı gelmiş ilk olay (en erken takvim anı; eşitlikte akış anahtarı sırası). Takvim
    /// anından önce hiçbir şey verilmez.
    pub(crate) fn release_due(&mut self, now: Duration) -> Option<PttEvent> {
        let (&key, _) = self
            .streams
            .iter()
            .filter_map(|(k, s)| s.due().map(|d| (k, d)))
            .filter(|&(_, d)| d <= now)
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

    /// Yerel sonlandırma: çalınmamış demetler atılır, `PttEnded` hazır kuyruğa girer; Ptt'de hâlâ
    /// etkinse anahtar işaretlenir (sonraki demetler elenir, Ptt'nin `PttEnded`'i yutulur).
    fn terminate(&mut self, key: Key, why: Termination) {
        let Some(stream) = self.streams.remove(&key) else {
            return;
        };
        let unplayed = u64::try_from(stream.queue.len()).unwrap_or(u64::MAX);
        self.stats.playout_discarded = self.stats.playout_discarded.saturating_add(unplayed);
        if !stream.ended {
            self.terminated.insert(key);
        }
        match why {
            Termination::Overflow => self.stats.playout_overflow += 1,
            Termination::Session => self.stats.playout_session_closes += 1,
            Termination::Table => self.stats.playout_table_closes += 1,
        }
        let (peer, stream_id) = key;
        self.ready.push_back(PttEvent::PttEnded { peer, stream_id });
    }

    /// Tablo tavanı: Ptt'de bitmiş akışlardan bitişi en yakın olanı (eşitlikte anahtar sırası)
    /// yerel sonlandırılır.
    fn evict_ended(&mut self) {
        let victim = self
            .streams
            .iter()
            .filter(|(_, s)| s.ended)
            .min_by_key(|&(&k, s)| (s.due(), k))
            .map(|(&k, _)| k);
        if let Some(key) = victim {
            self.terminate(key, Termination::Table);
        }
    }
}
