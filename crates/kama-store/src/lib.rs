//! K4G-06 (plan D8 v1.1) — asgari kalıcılık v0: TEK somut depo [`RedbStore`].
//!
//! # Üç soyutlama, tek depo (D8)
//!
//! [`RedbStore`] iki arayüzü AYRI AYRI gerçekler, birleştirmez:
//!
//! * [`AppStore`] (`kama-app`) — uygulama durumu: eş başına `msg_id` / `stream_id` /
//!   `transfer_id` sayaçları, alıcı dedup penceresi, gönderen outbox'ı (D4, D5);
//! * [`CounterStore`] (`kama-noise-wire`) — cihaz başına TEK `init_counter` (D2).
//!
//! Kimlik, anahtarlar ve eş dizini depoya GİRMEZ: bileşim kökü onları provisioning dosyasından
//! okur ve bellekte tutar (K4G-11). At-rest şifreleme YOKTUR (PoC-KAPSAM-001 §2, EK-2: kabul
//! edilmiş PoC riski). Şema göçü, sorgu, mesaj geçmişi YOKTUR.
//!
//! İki trait de `reserve_next` adlı yöntem taşır; ikisi birden kapsamdayken çağrı tam nitelikli
//! yazılır: `AppStore::reserve_next(&store, peer, kind)` / `CounterStore::reserve_next(&store)`.
//!
//! # Katman yeri (`scripts/ci/layer_check.py` C7)
//!
//! `kama-store` ALTYAPI crate'idir: trait sahipleri (`kama-app`, `kama-noise-wire`) ona bağımlı
//! DEĞİLDİR, o onlara bağımlıdır. Workspace'te ona yalnız bileşim kökü (K4G-11 `kama-node`)
//! bağımlı olabilir.
//!
//! # Şema v0 (tek redb dosyası, dört tablo; göç YOK)
//!
//! Bu crate'in kodladığı anahtar ve değerler sabit genişlikli big-endian'dır (ADR-05 geleneği);
//! sayaç değerleri redb'nin kendi `u64` kodlamasıyla durur (dosya içi, telde değil).
//!
//! | Tablo | Anahtar | Değer |
//! |---|---|---|
//! | `app_counters` | `dest_hash [32] ‖ tür u8` (`0x01` msg_id · `0x02` stream_id · `0x03` transfer_id) | `u64`: sıradaki dağıtılacak değer |
//! | `init_counter` | `()` (tek satır) | `u64`: sıradaki `init_counter` |
//! | `dedup` | `dest_hash [32]` | `has_highest u8 ‖ highest u32_be ‖ seen [64 × u64_be]` (517 B) |
//! | `outbox` | `dest_hash [32] ‖ msg_id u32_be` | `compression_policy u8 ‖ kayıt [≤ 65540]` |
//!
//! Satırı olmayan sayaç 0'dan başlar (`kama_app::store::FIRST_COUNTER_VALUE`; `init_counter` için
//! `MemoryCounterStore::new()` ile aynı). Okunan bayt v0 düzenine uymazsa
//! [`AppStoreError::Corrupt`]; boy, ayırmadan ÖNCE denetlenir (SR-AUDIT-001). redb'nin kendi
//! hataları (bozulma dahil) [`AppStoreError::Persist`] / [`CounterStoreError::Persist`]'tir.
//!
//! # İşlem sınırları
//!
//! Her `reserve_next` TEK redb yazma işlemidir: mevcut değeri okur, ardılı yazar, `commit` eder ve
//! değeri ANCAK `commit` `Ok` döndükten sonra döndürür. redb yazma işleminin varsayılan
//! dayanıklılığı `Durability::Immediate`'tir (bu crate onu değiştirmez): `commit` `Ok`
//! döndüğünde değişiklik dosyaya işlenmiş ve eşitlenmiştir. `RedbStore` bellekte sayaç TUTMAZ;
//! tek durum redb dosyasıdır, bu yüzden hata yolunda "bellekte artırılmış" bir değer kalamaz.
//!
//! * Taşma → `Exhausted`: işlem commit edilmeden düşer (redb düşen yazma işlemini geri alır),
//!   değer dönmez, sayaç ilerlemez.
//! * G/Ç, işlem ya da commit hatası → `Persist`: değer dönmez. redb'ye göre başarısız bir commit
//!   değişikliği "ya tamamen ya hiç" uygular ama kalıcı olmuş OLABİLİR; o durumda değer yanar
//!   (sonraki değer daha büyüktür) — hiçbir değer iki kez dönmez. Başarısız commit'ten sonra redb
//!   o açılışta yeni yazma işlemi kabul etmez: depo kapatılıp yeniden açılana dek her yazma
//!   `Persist` döner (fail-closed).
//! * Eşzamanlılık: redb aynı anda TEK yazma işlemine izin verir (`begin_write` öncekinin bitmesini
//!   bekler) ve oku-artır-yaz tek işlemdedir; iki iş parçacığı aynı değeri okuyup yazamaz. Aynı
//!   dosyanın ikinci kez açılması, dosya kilidini destekleyen platformlarda redb'nin kilidiyle
//!   reddedilir ([`StoreOpenError::AlreadyOpen`]); tek açılış bileşim kökünün sorumluluğudur.
//!
//! Diğer yazmalar (`dedup_save`, `outbox_put`, `outbox_remove`) da tek işlem + commit'tir ve
//! idempotenttir (`outbox_remove` silecek kayıt yoksa commit etmez). `outbox_put`'ta tavan
//! denetimi ile ekleme AYNI işlemdedir.

#![forbid(unsafe_code)]

use std::path::Path;

use kama_app::messaging::{MsgId, MAX_OUTBOX_UNACKED};
use kama_app::store::{
    AppStore, AppStoreError, CounterKind, DedupWindow, OutboxUnit, DEDUP_WINDOW_WORDS,
    FIRST_COUNTER_VALUE, MAX_OUTBOX_RECORD_BYTES,
};
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{CompressionPolicy, PreparedUnit};
use kama_noise_wire::{CounterStore, CounterStoreError};
use redb::{Database, ReadableDatabase, ReadableTable, Table, TableDefinition};

/// `dest_hash` boyu; [`peer_key`] bunu `PeerRef`'in gerçek genişliğine karşı derleme zamanında
/// denetler.
const PEER_LEN: usize = 32;
const KIND_LEN: usize = core::mem::size_of::<u8>();
const MSG_ID_LEN: usize = core::mem::size_of::<MsgId>();
const WORD_LEN: usize = core::mem::size_of::<u64>();

const APP_COUNTER_KEY_LEN: usize = PEER_LEN + KIND_LEN;
const OUTBOX_KEY_LEN: usize = PEER_LEN + MSG_ID_LEN;

/// `dedup` değeri: `has_highest u8 ‖ highest u32_be ‖ seen [DEDUP_WINDOW_WORDS × u64_be]`.
const DEDUP_FLAG_AT: usize = 0;
const DEDUP_HIGHEST_AT: usize = DEDUP_FLAG_AT + core::mem::size_of::<u8>();
const DEDUP_SEEN_AT: usize = DEDUP_HIGHEST_AT + MSG_ID_LEN;
const DEDUP_VALUE_LEN: usize = DEDUP_SEEN_AT + DEDUP_WINDOW_WORDS * WORD_LEN;
const HIGHEST_ABSENT: u8 = 0x00;
const HIGHEST_PRESENT: u8 = 0x01;

/// `outbox` değeri: `compression_policy u8 ‖ kayıt`.
const POLICY_NEVER: u8 = 0x00;
const POLICY_APP_COMPRESSED: u8 = 0x01;

/// `app_counters` anahtarının tür baytı.
const KIND_MSG_ID: u8 = 0x01;
const KIND_STREAM_ID: u8 = 0x02;
const KIND_TRANSFER_ID: u8 = 0x03;

/// Taze depoda ilk `init_counter` (`kama_noise_wire`'ın `MemoryCounterStore::new()`'uyla aynı).
const INIT_COUNTER_FIRST: u64 = 0;

const APP_COUNTERS: TableDefinition<&[u8; APP_COUNTER_KEY_LEN], u64> =
    TableDefinition::new("app_counters");
const INIT_COUNTER: TableDefinition<(), u64> = TableDefinition::new("init_counter");
const DEDUP: TableDefinition<&[u8; PEER_LEN], &[u8]> = TableDefinition::new("dedup");
const OUTBOX: TableDefinition<&[u8; OUTBOX_KEY_LEN], &[u8]> = TableDefinition::new("outbox");

/// [`RedbStore::open`] hatası.
#[derive(Debug, thiserror::Error)]
pub enum StoreOpenError {
    /// Dosya başka bir açılışça tutuluyor (redb dosya kilidi): aynı dosyaya iki depo yazamaz.
    #[error("the store file is already open (redb file lock)")]
    AlreadyOpen,
    /// redb dosyayı açamadı (redb dosyası değil, G/Ç) ya da şema v0 tablolarını hazırlayamadı
    /// (tablo tipi uyuşmazlığı dahil).
    #[error("redb could not open or initialise the store: {0}")]
    Redb(Box<redb::Error>),
}

fn open_error(error: impl Into<redb::Error>) -> StoreOpenError {
    match error.into() {
        redb::Error::DatabaseAlreadyOpen => StoreOpenError::AlreadyOpen,
        other => StoreOpenError::Redb(Box::new(other)),
    }
}

/// redb hatası → uygulama deposu hatası. Ayrıntı trait sınırını geçmez (`AppStoreError` `Copy`'dir
/// ve `kama-app` redb'yi görmez).
fn persist<E>(_: E) -> AppStoreError {
    AppStoreError::Persist
}

/// redb hatası → `init_counter` deposu hatası.
fn counter_persist<E>(_: E) -> CounterStoreError {
    CounterStoreError::Persist
}

/// Tek somut kalıcı depo: [`AppStore`] ve [`CounterStore`]'u ayrı ayrı gerçekler. Ayrıntı: crate
/// belgesi.
#[derive(Debug)]
pub struct RedbStore {
    db: Database,
}

impl RedbStore {
    /// `path`'teki redb dosyasını açar (yoksa yaratır) ve şema v0'ın dört tablosunu hazırlar.
    ///
    /// # Errors
    ///
    /// [`StoreOpenError::AlreadyOpen`] — dosya başka bir açılışça tutuluyor ·
    /// [`StoreOpenError::Redb`] — dosya açılamadı, redb dosyası değil ya da tablolar
    /// hazırlanamadı.
    pub fn open(path: &Path) -> Result<Self, StoreOpenError> {
        Self::with_database(Database::create(path).map_err(open_error)?)
    }

    /// Açılmış veritabanında şema v0 tablolarını (yoksa) yaratır; tek yazma işlemi.
    fn with_database(db: Database) -> Result<Self, StoreOpenError> {
        let txn = db.begin_write().map_err(open_error)?;
        txn.open_table(APP_COUNTERS).map_err(open_error)?;
        txn.open_table(INIT_COUNTER).map_err(open_error)?;
        txn.open_table(DEDUP).map_err(open_error)?;
        txn.open_table(OUTBOX).map_err(open_error)?;
        txn.commit().map_err(open_error)?;
        Ok(Self { db })
    }
}

impl AppStore for RedbStore {
    fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
        let key = app_counter_key(&peer, kind);
        // TEK yazma işlemi: oku → artır → yaz → commit. Erken dönüşte (`?`) işlem düşer ve redb
        // onu geri alır.
        let txn = self.db.begin_write().map_err(persist)?;
        let value = {
            let mut table = txn.open_table(APP_COUNTERS).map_err(persist)?;
            let next = table
                .get(&key)
                .map_err(persist)?
                .map_or(FIRST_COUNTER_VALUE, |stored| stored.value());
            let after = kind.advance(next)?;
            table.insert(&key, after).map_err(persist)?;
            next
        };
        // Değer ANCAK commit kalıcılaştırdıktan sonra döner.
        txn.commit().map_err(persist)?;
        Ok(value)
    }

    fn dedup_load(&self, peer: PeerRef) -> Result<DedupWindow, AppStoreError> {
        let txn = self.db.begin_read().map_err(persist)?;
        let table = txn.open_table(DEDUP).map_err(persist)?;
        let stored = table.get(peer_key(&peer)).map_err(persist)?;
        stored.map_or(Ok(DedupWindow::EMPTY), |stored| {
            decode_dedup(stored.value())
        })
    }

    fn dedup_save(&self, peer: PeerRef, window: &DedupWindow) -> Result<(), AppStoreError> {
        let value = encode_dedup(window);
        let txn = self.db.begin_write().map_err(persist)?;
        {
            let mut table = txn.open_table(DEDUP).map_err(persist)?;
            table
                .insert(peer_key(&peer), value.as_slice())
                .map_err(persist)?;
        }
        txn.commit().map_err(persist)
    }

    fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError> {
        let key = outbox_key(&peer, unit.msg_id());
        let value = encode_outbox(unit.unit());
        let txn = self.db.begin_write().map_err(persist)?;
        {
            let mut table = txn.open_table(OUTBOX).map_err(persist)?;
            // Tavan denetimi ve ekleme AYNI işlemde: aradaki başka bir yazma sayıyı değiştiremez.
            let replacing = table.get(&key).map_err(persist)?.is_some();
            if !replacing && held_units(&table, &peer)? >= MAX_OUTBOX_UNACKED {
                // İşlem commit edilmeden düşer: hiçbir şey yazılmadı.
                return Err(AppStoreError::OutboxFull);
            }
            table.insert(&key, value.as_slice()).map_err(persist)?;
        }
        txn.commit().map_err(persist)
    }

    fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError> {
        let key = outbox_key(&peer, msg_id);
        let txn = self.db.begin_write().map_err(persist)?;
        let removed = {
            let mut table = txn.open_table(OUTBOX).map_err(persist)?;
            // Eski değerin koruyucusu tablodan ÖNCE düşsün diye yerel değişkende tutulur.
            let old = table.remove(&key).map_err(persist)?;
            old.is_some()
        };
        if !removed {
            // Değişiklik yok: işlem commit edilmeden düşer (gereksiz eşitleme yok).
            return Ok(false);
        }
        txn.commit().map_err(persist)?;
        Ok(true)
    }

    fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
        let txn = self.db.begin_read().map_err(persist)?;
        let table = txn.open_table(OUTBOX).map_err(persist)?;
        let (first, last) = outbox_bounds(&peer);
        let mut units = Vec::new();
        for entry in table.range(&first..=&last).map_err(persist)? {
            let (key, value) = entry.map_err(persist)?;
            // SR-AUDIT-001: eş başına en çok MAX_OUTBOX_UNACKED kayıt yazılabilir; fazlası bozulmadır
            // ve ayırmadan ÖNCE reddedilir.
            if units.len() >= MAX_OUTBOX_UNACKED {
                return Err(AppStoreError::Corrupt);
            }
            units.push(decode_outbox(&peer, key.value(), value.value())?);
        }
        Ok(units)
    }
}

impl CounterStore for RedbStore {
    fn reserve_next(&self) -> Result<u64, CounterStoreError> {
        // TEK yazma işlemi: oku → artır → yaz → commit (AppStore sayaçlarıyla aynı akış).
        let txn = self.db.begin_write().map_err(counter_persist)?;
        let value = {
            let mut table = txn.open_table(INIT_COUNTER).map_err(counter_persist)?;
            let next = table
                .get(())
                .map_err(counter_persist)?
                .map_or(INIT_COUNTER_FIRST, |stored| stored.value());
            // `u64::MAX` dağıtılmaz: ardılı kalıcılaştırılamaz (MemoryCounterStore ile aynı).
            let after = next.checked_add(1).ok_or(CounterStoreError::Exhausted)?;
            table.insert((), after).map_err(counter_persist)?;
            next
        };
        // Değer ANCAK commit kalıcılaştırdıktan sonra döner.
        txn.commit().map_err(counter_persist)?;
        Ok(value)
    }
}

/// `PeerRef`'in ham `dest_hash`'i. Dönüş tipi `PEER_LEN`'i `PeerRef`'in gerçek genişliğine karşı
/// derleme zamanında denetler.
fn peer_key(peer: &PeerRef) -> &[u8; PEER_LEN] {
    peer.as_bytes()
}

fn kind_code(kind: CounterKind) -> u8 {
    match kind {
        CounterKind::MsgId => KIND_MSG_ID,
        CounterKind::StreamId => KIND_STREAM_ID,
        CounterKind::TransferId => KIND_TRANSFER_ID,
    }
}

fn app_counter_key(peer: &PeerRef, kind: CounterKind) -> [u8; APP_COUNTER_KEY_LEN] {
    let mut key = [0u8; APP_COUNTER_KEY_LEN];
    key[..PEER_LEN].copy_from_slice(peer_key(peer));
    key[PEER_LEN] = kind_code(kind);
    key
}

fn outbox_key(peer: &PeerRef, msg_id: MsgId) -> [u8; OUTBOX_KEY_LEN] {
    let mut key = [0u8; OUTBOX_KEY_LEN];
    key[..PEER_LEN].copy_from_slice(peer_key(peer));
    key[PEER_LEN..].copy_from_slice(&msg_id.to_be_bytes());
    key
}

/// Eşin outbox anahtar aralığı (kapalı): `msg_id` big-endian olduğu için bayt sırası = `msg_id`
/// sırası.
fn outbox_bounds(peer: &PeerRef) -> ([u8; OUTBOX_KEY_LEN], [u8; OUTBOX_KEY_LEN]) {
    (outbox_key(peer, MsgId::MIN), outbox_key(peer, MsgId::MAX))
}

/// Eşin outbox'ındaki kayıt sayısı, [`MAX_OUTBOX_UNACKED`]'te kesilerek (sayım sınırlıdır).
fn held_units(
    table: &Table<'_, &'static [u8; OUTBOX_KEY_LEN], &'static [u8]>,
    peer: &PeerRef,
) -> Result<usize, AppStoreError> {
    let (first, last) = outbox_bounds(peer);
    let mut held = 0;
    for entry in table.range(&first..=&last).map_err(persist)? {
        entry.map_err(persist)?;
        held += 1;
        if held >= MAX_OUTBOX_UNACKED {
            break;
        }
    }
    Ok(held)
}

fn encode_dedup(window: &DedupWindow) -> [u8; DEDUP_VALUE_LEN] {
    let mut out = [0u8; DEDUP_VALUE_LEN];
    let (flag, highest) = match window.highest {
        Some(highest) => (HIGHEST_PRESENT, highest),
        // Kanonik: çapa yoksa alanı sıfırdır.
        None => (HIGHEST_ABSENT, 0),
    };
    out[DEDUP_FLAG_AT] = flag;
    out[DEDUP_HIGHEST_AT..DEDUP_SEEN_AT].copy_from_slice(&highest.to_be_bytes());
    for (chunk, word) in out[DEDUP_SEEN_AT..]
        .chunks_exact_mut(WORD_LEN)
        .zip(window.seen.iter())
    {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn decode_dedup(bytes: &[u8]) -> Result<DedupWindow, AppStoreError> {
    // Sabit boy: ayrıştırmadan önce tam eşitlik.
    let bytes: &[u8; DEDUP_VALUE_LEN] = bytes.try_into().map_err(|_| AppStoreError::Corrupt)?;
    let raw_highest = MsgId::from_be_bytes(
        bytes[DEDUP_HIGHEST_AT..DEDUP_SEEN_AT]
            .try_into()
            .map_err(|_| AppStoreError::Corrupt)?,
    );
    let highest = match bytes[DEDUP_FLAG_AT] {
        HIGHEST_PRESENT => Some(raw_highest),
        // Kanonik olmayan "yok" (alan sıfır değil) bozulmadır.
        HIGHEST_ABSENT if raw_highest == 0 => None,
        _ => return Err(AppStoreError::Corrupt),
    };
    let mut seen = [0u64; DEDUP_WINDOW_WORDS];
    for (word, chunk) in seen
        .iter_mut()
        .zip(bytes[DEDUP_SEEN_AT..].chunks_exact(WORD_LEN))
    {
        *word = u64::from_be_bytes(chunk.try_into().map_err(|_| AppStoreError::Corrupt)?);
    }
    Ok(DedupWindow { highest, seen })
}

/// `compression_policy u8 ‖ kayıt`; boy `OutboxUnit` tarafından sınırlıdır
/// (`≤ 1 + MAX_OUTBOX_RECORD_BYTES`).
fn encode_outbox(unit: &PreparedUnit) -> Vec<u8> {
    let policy = match unit.compression_policy {
        CompressionPolicy::Never => POLICY_NEVER,
        CompressionPolicy::AppCompressed => POLICY_APP_COMPRESSED,
    };
    let mut out = Vec::with_capacity(core::mem::size_of::<u8>() + unit.bytes.len());
    out.push(policy);
    out.extend_from_slice(&unit.bytes);
    out
}

fn decode_outbox(
    peer: &PeerRef,
    key: &[u8; OUTBOX_KEY_LEN],
    value: &[u8],
) -> Result<OutboxUnit, AppStoreError> {
    let (key_peer, key_msg_id) = key.split_at(PEER_LEN);
    if key_peer != peer_key(peer).as_slice() {
        return Err(AppStoreError::Corrupt);
    }
    let msg_id = MsgId::from_be_bytes(key_msg_id.try_into().map_err(|_| AppStoreError::Corrupt)?);
    let (&policy, record) = value.split_first().ok_or(AppStoreError::Corrupt)?;
    // SR-AUDIT-001: boy, kopyalamadan (ayırmadan) ÖNCE.
    if record.len() > MAX_OUTBOX_RECORD_BYTES {
        return Err(AppStoreError::Corrupt);
    }
    let compression_policy = match policy {
        POLICY_NEVER => CompressionPolicy::Never,
        POLICY_APP_COMPRESSED => CompressionPolicy::AppCompressed,
        _ => return Err(AppStoreError::Corrupt),
    };
    OutboxUnit::new(
        msg_id,
        PreparedUnit {
            bytes: record.to_vec(),
            compression_policy,
        },
    )
    .map_err(|_| AppStoreError::Corrupt)
}

#[cfg(test)]
mod tests {
    //! Beyaz kutu: tükenme (tohumlanmış sayaç), commit hatası (hata enjekte eden arka uç) ve v0
    //! düzenine uymayan satırlar. Kara kutu testleri `tests/` altında.

    use std::io;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use redb::backends::InMemoryBackend;
    use redb::{Builder, StorageBackend};

    use super::*;

    fn peer(tag: u8) -> PeerRef {
        PeerRef::from_dest_hash([tag; 32])
    }

    /// Bellek içi arka uç; bayrak kalkıkken her yazma ve eşitleme G/Ç hatası verir. Depolama `Arc`
    /// ile paylaşılır: aynı içerik "yeniden açılabilir".
    #[derive(Debug, Clone)]
    struct FlakyBackend {
        inner: Arc<InMemoryBackend>,
        fail_writes: Arc<AtomicBool>,
    }

    impl FlakyBackend {
        fn new() -> Self {
            Self {
                inner: Arc::new(InMemoryBackend::new()),
                fail_writes: Arc::new(AtomicBool::new(false)),
            }
        }

        fn check(&self) -> Result<(), io::Error> {
            if self.fail_writes.load(Ordering::SeqCst) {
                Err(io::Error::other("enjekte edilen yazma hatası"))
            } else {
                Ok(())
            }
        }
    }

    // Çağrılar tam nitelikli: `InMemoryBackend`'in aynı adlı (özel) kendi yöntemleri vardır.
    impl StorageBackend for FlakyBackend {
        fn len(&self) -> Result<u64, io::Error> {
            StorageBackend::len(&*self.inner)
        }

        fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), io::Error> {
            StorageBackend::read(&*self.inner, offset, out)
        }

        fn set_len(&self, len: u64) -> Result<(), io::Error> {
            self.check()?;
            StorageBackend::set_len(&*self.inner, len)
        }

        fn sync_data(&self) -> Result<(), io::Error> {
            self.check()?;
            StorageBackend::sync_data(&*self.inner)
        }

        fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
            self.check()?;
            StorageBackend::write(&*self.inner, offset, data)
        }

        fn close(&self) -> Result<(), io::Error> {
            StorageBackend::close(&*self.inner)
        }
    }

    fn open_on(backend: &FlakyBackend) -> RedbStore {
        let db = Builder::new()
            .create_with_backend(backend.clone())
            .expect("bellek içi redb açılmadı");
        RedbStore::with_database(db).expect("şema v0 hazırlanamadı")
    }

    fn seed_app_counter(store: &RedbStore, peer: &PeerRef, kind: CounterKind, next: u64) {
        let txn = store.db.begin_write().unwrap();
        txn.open_table(APP_COUNTERS)
            .unwrap()
            .insert(&app_counter_key(peer, kind), next)
            .unwrap();
        txn.commit().unwrap();
    }

    fn seed_raw<K: redb::Key + 'static>(
        store: &RedbStore,
        table: TableDefinition<'_, K, &'static [u8]>,
        key: K::SelfType<'_>,
        value: &[u8],
    ) {
        let txn = store.db.begin_write().unwrap();
        txn.open_table(table).unwrap().insert(key, value).unwrap();
        txn.commit().unwrap();
    }

    #[test]
    fn app_counter_exhaustion_is_permanent_and_reserves_nothing() {
        for kind in CounterKind::ALL {
            let store = open_on(&FlakyBackend::new());
            let last = kind.max_value().min(u64::MAX - 1);
            seed_app_counter(&store, &peer(1), kind, last);
            assert_eq!(AppStore::reserve_next(&store, peer(1), kind), Ok(last));
            for _ in 0..2 {
                assert_eq!(
                    AppStore::reserve_next(&store, peer(1), kind),
                    Err(AppStoreError::Exhausted),
                    "{kind:?}: hata durumu ilerletmez"
                );
            }
            assert_eq!(AppStore::reserve_next(&store, peer(2), kind), Ok(0));
        }
    }

    #[test]
    fn init_counter_exhaustion_is_permanent_and_reserves_nothing() {
        let store = open_on(&FlakyBackend::new());
        let txn = store.db.begin_write().unwrap();
        txn.open_table(INIT_COUNTER)
            .unwrap()
            .insert((), u64::MAX - 1)
            .unwrap();
        txn.commit().unwrap();
        assert_eq!(CounterStore::reserve_next(&store), Ok(u64::MAX - 1));
        for _ in 0..2 {
            assert_eq!(
                CounterStore::reserve_next(&store),
                Err(CounterStoreError::Exhausted)
            );
        }
    }

    /// Commit G/Ç hatasıyla biterse değer DÖNMEZ (`Persist`), aynı açılışta sonraki yazmalar da
    /// reddedilir (fail-closed) ve yeniden açılışta sayaç ilerlememiştir: başarısız çağrının
    /// değeri sıradaki başarılı çağrıda verilir — yani ayrılmamıştı.
    #[test]
    fn failed_commit_returns_persist_and_reserves_nothing() {
        let backend = FlakyBackend::new();
        let store = open_on(&backend);
        assert_eq!(
            AppStore::reserve_next(&store, peer(1), CounterKind::MsgId),
            Ok(0)
        );
        assert_eq!(CounterStore::reserve_next(&store), Ok(0));

        backend.fail_writes.store(true, Ordering::SeqCst);
        assert_eq!(
            AppStore::reserve_next(&store, peer(1), CounterKind::MsgId),
            Err(AppStoreError::Persist)
        );
        backend.fail_writes.store(false, Ordering::SeqCst);
        assert_eq!(
            AppStore::reserve_next(&store, peer(1), CounterKind::MsgId),
            Err(AppStoreError::Persist),
            "başarısız commit'ten sonra aynı açılış yazma kabul etmez"
        );
        assert_eq!(
            CounterStore::reserve_next(&store),
            Err(CounterStoreError::Persist)
        );
        drop(store);

        let store = open_on(&backend);
        assert_eq!(
            AppStore::reserve_next(&store, peer(1), CounterKind::MsgId),
            Ok(1),
            "başarısız çağrı değeri ayırmamıştı"
        );
        assert_eq!(CounterStore::reserve_next(&store), Ok(1));
    }

    #[test]
    fn init_counter_commit_failure_reserves_nothing() {
        let backend = FlakyBackend::new();
        let store = open_on(&backend);
        assert_eq!(CounterStore::reserve_next(&store), Ok(0));
        backend.fail_writes.store(true, Ordering::SeqCst);
        assert_eq!(
            CounterStore::reserve_next(&store),
            Err(CounterStoreError::Persist)
        );
        drop(store);
        backend.fail_writes.store(false, Ordering::SeqCst);
        let store = open_on(&backend);
        assert_eq!(CounterStore::reserve_next(&store), Ok(1));
    }

    #[test]
    fn rows_outside_schema_v0_are_corrupt_not_panics() {
        let store = open_on(&FlakyBackend::new());
        let p = peer(3);
        let valid = encode_dedup(&DedupWindow::EMPTY);

        // dedup: kısa / uzun değer, bilinmeyen bayrak, kanonik olmayan "yok".
        let mut unknown_flag = valid;
        unknown_flag[DEDUP_FLAG_AT] = 0x02;
        let mut non_canonical = valid;
        non_canonical[DEDUP_SEEN_AT - 1] = 0x01;
        let mut long = valid.to_vec();
        long.push(0);
        for bad in [
            &valid[..DEDUP_VALUE_LEN - 1],
            long.as_slice(),
            unknown_flag.as_slice(),
            non_canonical.as_slice(),
        ] {
            seed_raw(&store, DEDUP, peer_key(&p), bad);
            assert_eq!(store.dedup_load(p), Err(AppStoreError::Corrupt));
        }

        // outbox: boş değer, bilinmeyen politika, tavanı aşan kayıt.
        let key = outbox_key(&p, 5);
        let too_long = vec![POLICY_NEVER; 1 + MAX_OUTBOX_RECORD_BYTES + 1];
        for bad in [&[][..], &[0x02, 0x01][..], too_long.as_slice()] {
            seed_raw(&store, OUTBOX, &key, bad);
            assert_eq!(store.outbox_list(p), Err(AppStoreError::Corrupt));
        }
    }

    #[test]
    fn dedup_encoding_is_canonical_fixed_width_big_endian() {
        let mut window = DedupWindow {
            highest: Some(0x0102_0304),
            seen: [0; DEDUP_WINDOW_WORDS],
        };
        window.seen[0] = 0x1122_3344_5566_7788;
        let bytes = encode_dedup(&window);
        assert_eq!(bytes.len(), 1 + 4 + 64 * 8);
        assert_eq!(bytes[..5], [HIGHEST_PRESENT, 0x01, 0x02, 0x03, 0x04]);
        assert_eq!(
            bytes[5..13],
            [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
        );
        assert_eq!(decode_dedup(&bytes), Ok(window));
        assert_eq!(encode_dedup(&DedupWindow::EMPTY), [0u8; DEDUP_VALUE_LEN]);
    }
}
