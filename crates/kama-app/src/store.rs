//! K4G-06 (plan D8 v1.1) — asgari kalıcılık v0: Application'ın kalıcı durumu ([`AppStore`]).
//!
//! # Kapsam (D8: üç ayrı soyutlama, tek somut depo)
//!
//! [`AppStore`] YALNIZ uygulama durumunu taşır: eş başına `msg_id` / `stream_id` / `transfer_id`
//! sayaçları ([`CounterKind`]), alıcı dedup penceresi ([`DedupWindow`]) ve gönderen outbox'ı
//! ([`OutboxUnit`]) — başka hiçbir şey. `init_counter` (D2) ayrı arayüzdedir
//! (`kama_noise_wire::CounterStore`; Application kripto katmanını görmez, INV-NEIGHBOR-1). Kimlik,
//! anahtarlar ve eş dizini depoya GİRMEZ: bileşim kökü onları provisioning dosyasından okur ve
//! bellekte tutar (K4G-11). Mesaj geçmişi ve ek (blob) dizini bu sprintte YOKTUR (KATMAN 6).
//!
//! Ürün gerçeklemesi `kama-store::RedbStore`'dur; iki arayüzü (bu trait ve `CounterStore`) ayrı
//! ayrı gerçekler, birleştirmez. Bu modülde sözleşme ve bellek içi test deposu [`MemoryAppStore`]
//! vardır.
//!
//! # Motorlara bağlama (K4G-07/08'in işi)
//!
//! Messenger / Ptt / FileTransfer bu kartta depoya BAĞLANMAZ; sayaçlarını K4G-07/08'de yalnız
//! [`AppStore::reserve_next`] üzerinden alırlar. Tipler bugünkü yapıları doğrudan eşler:
//!
//! * [`DedupWindow`] — Messenger'ın eş başına alıcı penceresi, alan alan: görülen en büyük
//!   `msg_id` + `MSG_DEDUP_WINDOW` bitlik harita (bit konumu `msg_id % MSG_DEDUP_WINDOW`).
//! * [`OutboxUnit`] — outbox kaydı: `msg_id` + kodlanmış kayıt (`PreparedUnit`; yeniden gönderim
//!   AYNI baytları taşır). Kaydın "en son hangi tutamaca kabul ettirildi" bilgisi KALICI DEĞİLDİR:
//!   oturum tutamacı süreç ömrüyle sınırlıdır; yeniden başlamada her kayıt güncel oturuma
//!   gönderilmemiş sayılır.
//!
//! # Sınırlar (SR-AUDIT-001: bounds-before-alloc)
//!
//! [`DedupWindow`] sabit boyludur ([`DEDUP_WINDOW_WORDS`] × `u64`). [`OutboxUnit`]'in kaydı en çok
//! [`MAX_OUTBOX_RECORD_BYTES`]'tır (yapıcıda denetlenir). Eş başına outbox en çok
//! [`MAX_OUTBOX_UNACKED`] kayıt tutar ([`AppStoreError::OutboxFull`]). Kalıcı bayttan okuyan
//! gerçekleme boyu, ayırmadan ÖNCE denetler.
//!
//! # Gizlilik
//!
//! [`OutboxUnit`]'in `Debug` çıktısı kaydın baytlarını değil yalnız uzunluğunu yazar (outbox
//! kayıtları hiçbir `Debug` çıktısına girmez — `messaging` modül belgesi).

use core::fmt;
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use kama_contracts::handles::PeerRef;
use kama_contracts::s1::PreparedUnit;

use crate::codec::limits::APP_RECORD_HEADER_BYTES;
use crate::file_transfer::{TransferId, FIRST_TRANSFER_ID};
use crate::messaging::{MsgId, FIRST_MSG_ID, MAX_OUTBOX_UNACKED, MSG_DEDUP_WINDOW};
use crate::ptt::{StreamId, FIRST_STREAM_ID};

/// Taze depoda (satırı olmayan) her sayacın ilk değeri — motorların bugünkü ilk değerleriyle
/// (`FIRST_MSG_ID`, `FIRST_STREAM_ID`, `FIRST_TRANSFER_ID`) AYNI; derleme zamanında denetlenir.
pub const FIRST_COUNTER_VALUE: u64 = 0;

const _: () = assert!(
    FIRST_MSG_ID as u64 == FIRST_COUNTER_VALUE
        && FIRST_STREAM_ID as u64 == FIRST_COUNTER_VALUE
        && FIRST_TRANSFER_ID == FIRST_COUNTER_VALUE
);

/// [`DedupWindow::seen`]'in `u64` kelime sayısı: `MSG_DEDUP_WINDOW / 64`.
pub const DEDUP_WINDOW_WORDS: usize = (MSG_DEDUP_WINDOW / u64::BITS) as usize;

// Pencere tam kelimelerle kaplanır (Messenger'daki denetimle aynı).
const _: () = assert!(MSG_DEDUP_WINDOW.is_multiple_of(u64::BITS));

/// [`OutboxUnit`]'in taşıyabileceği en uzun kayıt: AMP kayıt başlığı + `u16` `body_len` tavanı
/// (AMP §2: `5 + 65535`). Codec'in ürettiği her kayıt buna sığar (`MAX_BODY_BYTES ≤ 65535`, A2).
pub const MAX_OUTBOX_RECORD_BYTES: usize = APP_RECORD_HEADER_BYTES + u16::MAX as usize;

/// [`AppStore::reserve_next`]'in sayaç türü. Her `(eş, tür)` çifti AYRI bir sayaçtır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CounterKind {
    /// `TEXT.msg_id` (AMP §3; telde `u32_be`) — Messenger.
    MsgId,
    /// `PTT_*` / `MEDIA_BUNDLE` `stream_id` (`u32_be`) — Ptt.
    StreamId,
    /// `FILE_*.transfer_id` (`u64_be`) — FileTransfer.
    TransferId,
}

impl CounterKind {
    /// Bütün türler (kapalı küme).
    pub const ALL: [Self; 3] = [Self::MsgId, Self::StreamId, Self::TransferId];

    /// Türün tel alanına sığan en büyük değer: depo bundan büyük bir değer DÖNDÜRMEZ.
    #[must_use]
    pub const fn max_value(self) -> u64 {
        match self {
            Self::MsgId => MsgId::MAX as u64,
            Self::StreamId => StreamId::MAX as u64,
            Self::TransferId => TransferId::MAX,
        }
    }

    /// Depoda kalıcı "sıradaki değer" `next` ayrılırsa yerine yazılacak ardıl (`next + 1`).
    ///
    /// Bütün depolar (bellek içi ve redb) tükenme kuralını buradan alır: `next` türün tavanını
    /// ([`Self::max_value`]) aşıyorsa ya da ardılı `u64`'e sığmıyorsa değer ayrılamaz. `u64::MAX`
    /// bu yüzden hiç dağıtılmaz (ardılı kalıcılaştırılamaz) — `kama_noise_wire`'ın
    /// `MemoryCounterStore`'uyla aynı kural.
    ///
    /// # Errors
    ///
    /// [`AppStoreError::Exhausted`] — sarma YOK; durum kalıcıdır.
    pub const fn advance(self, next: u64) -> Result<u64, AppStoreError> {
        if next > self.max_value() {
            return Err(AppStoreError::Exhausted);
        }
        match next.checked_add(1) {
            Some(after) => Ok(after),
            None => Err(AppStoreError::Exhausted),
        }
    }
}

/// [`AppStore`] hatası.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStoreError {
    /// Sayaç uzayı tükendi (tür tavanı ya da `u64`): değer AYRILMADI; sarma YOK ve durum
    /// kalıcıdır (sonraki çağrılar da `Exhausted`).
    Exhausted,
    /// Depo okunamadı, yazılamadı ya da işlem kalıcılaştırılamadı (commit). `reserve_next`'te
    /// değer AYRILMAMIŞTIR.
    Persist,
    /// Kalıcı kayıt depo düzenine (şema v0) uymuyor — bozuk ya da yabancı bayt. Yeniden denemek
    /// düzeltmez.
    Corrupt,
    /// Eşin outbox'ında zaten [`MAX_OUTBOX_UNACKED`] kayıt var; yeni `msg_id` alınmadı.
    OutboxFull,
    /// Kayıt [`MAX_OUTBOX_RECORD_BYTES`]'ı aşıyor.
    RecordTooLarge {
        /// Reddedilen kaydın boyu (bayt).
        len: usize,
    },
}

impl fmt::Display for AppStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => f.write_str("counter space is exhausted (no wrap)"),
            Self::Persist => f.write_str("app store could not read, write or commit"),
            Self::Corrupt => f.write_str("persisted app store record does not match schema v0"),
            Self::OutboxFull => write!(
                f,
                "{MAX_OUTBOX_UNACKED} outbox records are already stored for this peer"
            ),
            Self::RecordTooLarge { len } => write!(
                f,
                "outbox record of {len} bytes exceeds {MAX_OUTBOX_RECORD_BYTES} bytes"
            ),
        }
    }
}

impl std::error::Error for AppStoreError {}

/// Eş başına alıcı dedup penceresinin kalıcı hâli — Messenger'ın penceresiyle alan alan aynı.
///
/// Pencere `highest − (MSG_DEDUP_WINDOW − 1) ..= highest`'tir; `msg_id`'nin bit konumu
/// `msg_id % MSG_DEDUP_WINDOW`, yani `seen[konum / 64]`'ün `konum % 64`. bitidir. Depo değeri
/// YORUMLAMAZ: verileni bit bit aynen saklar ve aynen döndürür.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupWindow {
    /// Görülen en büyük `msg_id` (pencerenin çapası); `None` = eşten henüz TEXT gelmedi.
    pub highest: Option<MsgId>,
    /// Pencere içindeki görülmüş `msg_id`'lerin bit haritası.
    pub seen: [u64; DEDUP_WINDOW_WORDS],
}

impl DedupWindow {
    /// Boş pencere: eşten henüz TEXT gelmedi. Kaydı olmayan eş için [`AppStore::dedup_load`] bunu
    /// döndürür.
    pub const EMPTY: Self = Self {
        highest: None,
        seen: [0; DEDUP_WINDOW_WORDS],
    };
}

impl Default for DedupWindow {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Outbox kaydı: `RECEIPT(delivered)`'ı beklenen TEXT'in `msg_id`'si ve kodlanmış kaydı.
///
/// SINIRLI tiptir: kayıt en çok [`MAX_OUTBOX_RECORD_BYTES`] bayttır ve bu yalnız
/// [`OutboxUnit::new`]'de denetlenir (alanlar açık değildir). Depo kaydı yorumlamaz.
#[derive(Clone, PartialEq, Eq)]
pub struct OutboxUnit {
    msg_id: MsgId,
    unit: PreparedUnit,
}

impl OutboxUnit {
    /// Kaydı sınırına karşı denetleyip kurar.
    ///
    /// # Errors
    ///
    /// [`AppStoreError::RecordTooLarge`] — `unit.bytes` [`MAX_OUTBOX_RECORD_BYTES`]'ı aşıyor.
    pub fn new(msg_id: MsgId, unit: PreparedUnit) -> Result<Self, AppStoreError> {
        let len = unit.bytes.len();
        if len > MAX_OUTBOX_RECORD_BYTES {
            return Err(AppStoreError::RecordTooLarge { len });
        }
        Ok(Self { msg_id, unit })
    }

    /// Kaydın `msg_id`'si.
    #[must_use]
    pub const fn msg_id(&self) -> MsgId {
        self.msg_id
    }

    /// Kodlanmış kayıt (yeniden gönderimde AYNEN verilir).
    #[must_use]
    pub const fn unit(&self) -> &PreparedUnit {
        &self.unit
    }

    /// Kaydı sahiplenerek çıkarır.
    #[must_use]
    pub fn into_unit(self) -> PreparedUnit {
        self.unit
    }
}

/// Kaydın baytları YAZILMAZ; yalnız uzunluğu ve sıkıştırma politikası.
impl fmt::Debug for OutboxUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboxUnit")
            .field("msg_id", &self.msg_id)
            .field("record_len", &self.unit.bytes.len())
            .field("compression_policy", &self.unit.compression_policy)
            .finish_non_exhaustive()
    }
}

/// Application'ın kalıcı durumu — YALNIZ sayaçlar, dedup penceresi ve outbox (D8 v1.1). Kimlik,
/// anahtar, eş dizini, mesaj geçmişi ve blob dizini bu trait'e GİRMEZ.
///
/// # `reserve_next` sözleşmesi (D2 ile AYNI)
///
/// Döndürülen değer, çağrı dönmeden **önce** kalıcılaştırılmıştır; aynı depo aynı `(peer, kind)`
/// için aynı değeri iki kez döndürmez; depo hata verirse değer **ayrılmamıştır** (çağırana değer
/// verilmedi, kullanılamaz). İşlem tektir: depo mevcut değeri okur, artırır, kalıcılaştırır
/// (commit), ancak sonra döndürür. `load()`/`store()` çifti YOKTUR.
///
/// Ek: değerler `(peer, kind)` başına kesin artandır, taze depoda [`FIRST_COUNTER_VALUE`]'dan
/// başlar ve `kind.max_value()`'yu aşmaz; tükenme ([`AppStoreError::Exhausted`]) kalıcıdır ve
/// hiçbir şeyi ilerletmez ([`CounterKind::advance`]). Sayaçlar birbirinden ve `init_counter`'dan
/// bağımsızdır. Hata sonrası durum belirsiz kalabilir (commit kalıcı olmuş olabilir): o değer
/// yanmış olur, sonraki değer daha büyüktür — hiçbir değer iki kez dönmez.
///
/// # Diğer işlemler
///
/// Her yazma (`dedup_save`, `outbox_put`, `outbox_remove`) `Ok` dönmeden önce kalıcıdır ve
/// idempotenttir: `Err` sonrası durum eski ya da yeni olabilir, aynı çağrının tekrarı güvenlidir.
/// Okumalar (`dedup_load`, `outbox_list`) en son kalıcılaştırılmış durumu verir.
pub trait AppStore: Send + Sync {
    /// `(peer, kind)` sayacının sıradaki değerini ayırır ve kalıcılaştırır; değer ancak bundan
    /// sonra döner.
    ///
    /// # Errors
    ///
    /// [`AppStoreError::Exhausted`] · [`AppStoreError::Persist`] · [`AppStoreError::Corrupt`] —
    /// değer ayrılmamıştır; çağıran onu KULLANMAZ.
    fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError>;

    /// Eşin dedup penceresi; kaydı yoksa [`DedupWindow::EMPTY`].
    ///
    /// # Errors
    ///
    /// [`AppStoreError::Persist`] · [`AppStoreError::Corrupt`].
    fn dedup_load(&self, peer: PeerRef) -> Result<DedupWindow, AppStoreError>;

    /// Eşin dedup penceresini (varsa öncekinin yerine) kalıcılaştırır.
    ///
    /// # Errors
    ///
    /// [`AppStoreError::Persist`].
    fn dedup_save(&self, peer: PeerRef, window: &DedupWindow) -> Result<(), AppStoreError>;

    /// Kaydı eşin outbox'ına yazar; aynı `msg_id`'li kayıt varsa yerine geçer (sayı büyümez).
    ///
    /// # Errors
    ///
    /// [`AppStoreError::OutboxFull`] — yeni `msg_id` ve eşin outbox'ı dolu (hiçbir şey
    /// yazılmadı) · [`AppStoreError::Persist`].
    fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError>;

    /// `msg_id`'li kaydı eşin outbox'ından siler; `true` = kayıt vardı.
    ///
    /// # Errors
    ///
    /// [`AppStoreError::Persist`].
    fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError>;

    /// Eşin outbox'ı, `msg_id` artan sırasıyla (≤ [`MAX_OUTBOX_UNACKED`] kayıt).
    ///
    /// # Errors
    ///
    /// [`AppStoreError::Persist`] · [`AppStoreError::Corrupt`].
    fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError>;
}

/// Bellek içi depo — YALNIZ TEST ve geliştirme: "kalıcılık" örneğin ömrüyle sınırlıdır. Yeniden
/// başlatma, aynı örneği yeni bir motora vererek benzetilir; yeni bir örnek, kalıcı durumu
/// kaybolmuş bir cihazdır. Ürün yolu `kama-store::RedbStore`'u kullanır (K4G-11).
///
/// Kurallar ürün deposuyla aynıdır (tükenme [`CounterKind::advance`], outbox tavanı ve sırası);
/// iki depo `kama-store`'un ortak sözleşme testinden geçer.
#[derive(Debug, Default)]
pub struct MemoryAppStore {
    state: Mutex<MemoryState>,
}

#[derive(Debug, Default)]
struct MemoryState {
    /// `(eş, tür)` → sıradaki değer; satır yoksa [`FIRST_COUNTER_VALUE`].
    counters: BTreeMap<(PeerRef, CounterKind), u64>,
    dedup: BTreeMap<PeerRef, DedupWindow>,
    /// Eş → `msg_id` → kayıt (eş başına ≤ [`MAX_OUTBOX_UNACKED`]).
    outbox: BTreeMap<PeerRef, BTreeMap<MsgId, OutboxUnit>>,
}

impl MemoryAppStore {
    /// Boş depo.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, MemoryState> {
        // Zehirlenme yalnız başka bir iş parçacığının kilit altında paniklemesiyle olur; kilit
        // altındaki her yazma tek bir harita işlemidir, durum yarım kalamaz.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AppStore for MemoryAppStore {
    fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
        let mut state = self.state();
        let next = state
            .counters
            .get(&(peer, kind))
            .copied()
            .unwrap_or(FIRST_COUNTER_VALUE);
        let after = kind.advance(next)?;
        state.counters.insert((peer, kind), after);
        Ok(next)
    }

    fn dedup_load(&self, peer: PeerRef) -> Result<DedupWindow, AppStoreError> {
        Ok(self.state().dedup.get(&peer).cloned().unwrap_or_default())
    }

    fn dedup_save(&self, peer: PeerRef, window: &DedupWindow) -> Result<(), AppStoreError> {
        self.state().dedup.insert(peer, window.clone());
        Ok(())
    }

    fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError> {
        let mut state = self.state();
        let units = state.outbox.entry(peer).or_default();
        if !units.contains_key(&unit.msg_id) && units.len() >= MAX_OUTBOX_UNACKED {
            return Err(AppStoreError::OutboxFull);
        }
        units.insert(unit.msg_id, unit.clone());
        Ok(())
    }

    fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError> {
        Ok(self
            .state()
            .outbox
            .get_mut(&peer)
            .is_some_and(|units| units.remove(&msg_id).is_some()))
    }

    fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
        Ok(self
            .state()
            .outbox
            .get(&peer)
            .map(|units| units.values().cloned().collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    //! Tip sınırları, tükenme ve eşzamanlılık. Bellek içi ve redb deposunun ORTAK davranışı
    //! (sayaç bağımsızlığı, outbox tavanı/sırası, dedup gidiş-dönüşü) `kama-store`'un sözleşme
    //! testindedir (`crates/kama-store/tests/app_store_contract.rs`).

    use super::*;
    use kama_contracts::s1::CompressionPolicy;

    fn peer(tag: u8) -> PeerRef {
        PeerRef::from_dest_hash([tag; 32])
    }

    fn record(len: usize) -> PreparedUnit {
        PreparedUnit {
            bytes: vec![0xA5; len],
            compression_policy: CompressionPolicy::Never,
        }
    }

    #[test]
    fn advance_stops_at_the_wire_width_of_each_kind() {
        for kind in [CounterKind::MsgId, CounterKind::StreamId] {
            let max = u64::from(u32::MAX);
            assert_eq!(kind.max_value(), max);
            assert_eq!(
                kind.advance(max),
                Ok(max + 1),
                "{kind:?}: u32::MAX dağıtılır"
            );
            assert_eq!(kind.advance(max + 1), Err(AppStoreError::Exhausted));
        }
        let kind = CounterKind::TransferId;
        assert_eq!(kind.max_value(), u64::MAX);
        assert_eq!(kind.advance(u64::MAX - 1), Ok(u64::MAX));
        assert_eq!(
            kind.advance(u64::MAX),
            Err(AppStoreError::Exhausted),
            "u64::MAX'ın ardılı kalıcılaştırılamaz"
        );
    }

    #[test]
    fn memory_store_counts_from_the_first_value_per_peer_and_kind() {
        let store = MemoryAppStore::new();
        let (a, b) = (peer(1), peer(2));
        for expected in 0..3 {
            assert_eq!(store.reserve_next(a, CounterKind::MsgId), Ok(expected));
        }
        assert_eq!(store.reserve_next(a, CounterKind::StreamId), Ok(0));
        assert_eq!(store.reserve_next(b, CounterKind::MsgId), Ok(0));
        assert_eq!(store.reserve_next(a, CounterKind::MsgId), Ok(3));
    }

    #[test]
    fn memory_store_exhaustion_is_permanent_and_reserves_nothing() {
        for kind in CounterKind::ALL {
            let store = MemoryAppStore::new();
            let last = kind.max_value().min(u64::MAX - 1);
            store.state().counters.insert((peer(1), kind), last);
            assert_eq!(store.reserve_next(peer(1), kind), Ok(last), "{kind:?}");
            for _ in 0..2 {
                assert_eq!(
                    store.reserve_next(peer(1), kind),
                    Err(AppStoreError::Exhausted),
                    "{kind:?}: hata durumu ilerletmez"
                );
            }
            assert_eq!(
                store.reserve_next(peer(2), kind),
                Ok(0),
                "başka eş etkilenmez"
            );
        }
    }

    #[test]
    fn memory_store_never_returns_a_value_twice_across_threads() {
        const THREADS: usize = 4;
        const PER_THREAD: usize = 500;
        let store = MemoryAppStore::new();
        let mut all: Vec<u64> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|_| {
                    s.spawn(|| {
                        (0..PER_THREAD)
                            .map(|_| store.reserve_next(peer(7), CounterKind::MsgId).unwrap())
                            .collect::<Vec<u64>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), THREADS * PER_THREAD, "aynı değer iki kez döndü");
    }

    #[test]
    fn outbox_unit_is_bounded_by_the_amp_record_ceiling() {
        let at_limit = OutboxUnit::new(9, record(MAX_OUTBOX_RECORD_BYTES)).unwrap();
        assert_eq!(at_limit.msg_id(), 9);
        assert_eq!(at_limit.unit().bytes.len(), MAX_OUTBOX_RECORD_BYTES);
        assert_eq!(
            OutboxUnit::new(9, record(MAX_OUTBOX_RECORD_BYTES + 1)),
            Err(AppStoreError::RecordTooLarge {
                len: MAX_OUTBOX_RECORD_BYTES + 1
            })
        );
        assert_eq!(
            MAX_OUTBOX_RECORD_BYTES,
            APP_RECORD_HEADER_BYTES + usize::from(u16::MAX)
        );
    }

    #[test]
    fn outbox_unit_debug_does_not_print_the_record() {
        let unit = OutboxUnit::new(3, record(40)).unwrap();
        let shown = format!("{unit:?}");
        assert!(shown.contains("record_len: 40"), "{shown}");
        assert!(
            !shown.contains("165"),
            "kayıt baytı (0xA5 = 165) basıldı: {shown}"
        );
        assert!(!shown.to_lowercase().contains("a5"), "{shown}");
    }

    #[test]
    fn empty_window_is_the_default_and_the_unknown_peer_answer() {
        assert_eq!(DedupWindow::default(), DedupWindow::EMPTY);
        assert_eq!(DEDUP_WINDOW_WORDS * 64, MSG_DEDUP_WINDOW as usize);
        let store = MemoryAppStore::new();
        assert_eq!(store.dedup_load(peer(4)), Ok(DedupWindow::EMPTY));
        assert_eq!(store.outbox_list(peer(4)), Ok(Vec::new()));
        assert_eq!(store.outbox_remove(peer(4), 0), Ok(false));
    }
}
