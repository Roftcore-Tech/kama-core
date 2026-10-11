//! K4G-06 DONE — crash-safety: `reserve_next` döndükten sonra süreç "ölür" (depo kapatılır) ve
//! AYNI dosya yeniden açılır; sonraki değer öncekinden büyüktür — üç `CounterKind` ve
//! `init_counter` için ayrı ayrı. Dedup penceresi ve outbox yeniden açılışta aynıdır.
//!
//! Ek kanıt: alt süreç destructor çalışmadan ölür (`std::process::exit`: redb kapanışı, tampon
//! boşaltma ve kilit bırakma YAPILMAZ); ayırdığı değerler yine kalıcıdır — değer `reserve_next`
//! dönmeden önce commit edilmiştir, kapanışta değil.

mod common;

use std::path::Path;
use std::process::Command;

use common::{outbox_unit, peer, TempDir, DB_FILE};
use kama_app::store::{
    AppStore, CounterKind, DedupWindow, OutboxUnit, DEDUP_WINDOW_WORDS, MAX_OUTBOX_RECORD_BYTES,
};
use kama_contracts::s1::CompressionPolicy;
use kama_noise_wire::CounterStore;
use kama_store::{RedbStore, StoreOpenError};

/// Yeniden açılış sayısı ve her açılışta ayrılan değer sayısı.
const RESTARTS: usize = 3;
const PER_RUN: usize = 4;

/// Alt süreç: depo dizini (yalnız alt süreçte tanımlı) ve rapor dosyası.
const CHILD_DIR_ENV: &str = "KAMA_STORE_EXIT_CHILD_DIR";
const CHILD_TEST: &str = "reserved_values_survive_process_exit_without_drop";
const REPORT_FILE: &str = "reported";

fn open(dir: &TempDir) -> RedbStore {
    RedbStore::open(&dir.db()).expect("depo açılamadı")
}

/// Bir sayacın "süreç ölür → aynı dosya yeniden açılır" döngüsü. Her açılışta birkaç değer
/// ayrılır ve depo kapatılır; her değer bir öncekinden (önceki açılışınkiler dahil) büyüktür.
/// Hata enjekte edilmediği için dizi boşluksuzdur: son değer `RESTARTS × PER_RUN − 1`.
fn survives_restarts(tag: &str, mut reserve: impl FnMut(&RedbStore) -> u64) {
    let dir = TempDir::new(tag);
    let mut last: Option<u64> = None;
    for _ in 0..RESTARTS {
        let store = open(&dir);
        for _ in 0..PER_RUN {
            let value = reserve(&store);
            if let Some(previous) = last {
                assert!(value > previous, "{tag}: {value} ≤ önceki {previous}");
            }
            last = Some(value);
        }
        // Süreç "ölür": depo kapanır; sonraki tur aynı dosyayı açar.
        drop(store);
    }
    assert_eq!(last, Some((RESTARTS * PER_RUN - 1) as u64), "{tag}");
}

#[test]
fn msg_id_survives_reopen() {
    survives_restarts("msg-id", |store| {
        AppStore::reserve_next(store, peer(1), CounterKind::MsgId).expect("msg_id ayrılamadı")
    });
}

#[test]
fn stream_id_survives_reopen() {
    survives_restarts("stream-id", |store| {
        AppStore::reserve_next(store, peer(1), CounterKind::StreamId).expect("stream_id ayrılamadı")
    });
}

#[test]
fn transfer_id_survives_reopen() {
    survives_restarts("transfer-id", |store| {
        AppStore::reserve_next(store, peer(1), CounterKind::TransferId)
            .expect("transfer_id ayrılamadı")
    });
}

#[test]
fn init_counter_survives_reopen() {
    survives_restarts("init-counter", |store| {
        CounterStore::reserve_next(store).expect("init_counter ayrılamadı")
    });
}

#[test]
fn dedup_window_survives_reopen() {
    let dir = TempDir::new("dedup");
    let (a, b) = (peer(1), peer(2));
    let mut window = DedupWindow {
        highest: Some(5000),
        seen: [0; DEDUP_WINDOW_WORDS],
    };
    window.seen[0] = 0x8000_0000_0000_0001;
    window.seen[DEDUP_WINDOW_WORDS - 1] = u64::MAX;
    let mut other = DedupWindow::EMPTY;
    other.highest = Some(0);
    other.seen[0] = 1;
    {
        let store = open(&dir);
        store.dedup_save(a, &window).expect("dedup yazılamadı");
        store.dedup_save(b, &other).expect("dedup yazılamadı");
    }
    {
        let store = open(&dir);
        assert_eq!(store.dedup_load(a), Ok(window.clone()));
        assert_eq!(store.dedup_load(b), Ok(other.clone()));
        assert_eq!(store.dedup_load(peer(3)), Ok(DedupWindow::EMPTY));
        // Pencere ilerler ve yeniden yazılır; yeni hâl de bir sonraki açılışa taşınır.
        window.highest = Some(5001);
        window.seen[1] |= 1 << 5;
        store.dedup_save(a, &window).expect("dedup yazılamadı");
    }
    let store = open(&dir);
    assert_eq!(store.dedup_load(a), Ok(window));
    assert_eq!(store.dedup_load(b), Ok(other));
}

#[test]
fn outbox_survives_reopen() {
    let dir = TempDir::new("outbox");
    let (a, b) = (peer(1), peer(2));
    let kept = [
        outbox_unit(7, 40, CompressionPolicy::Never),
        outbox_unit(3, 1, CompressionPolicy::AppCompressed),
        outbox_unit(u32::MAX, MAX_OUTBOX_RECORD_BYTES, CompressionPolicy::Never),
    ];
    {
        let store = open(&dir);
        for unit in &kept {
            store.outbox_put(a, unit).expect("outbox yazılamadı");
        }
        store
            .outbox_put(a, &outbox_unit(5, 10, CompressionPolicy::Never))
            .expect("outbox yazılamadı");
        store
            .outbox_put(b, &outbox_unit(5, 20, CompressionPolicy::Never))
            .expect("outbox yazılamadı");
        assert_eq!(store.outbox_remove(a, 5), Ok(true));
    }
    let store = open(&dir);
    let mut expected = kept.to_vec();
    expected.sort_by_key(OutboxUnit::msg_id);
    assert_eq!(store.outbox_list(a), Ok(expected));
    assert_eq!(
        store.outbox_list(b),
        Ok(vec![outbox_unit(5, 20, CompressionPolicy::Never)])
    );
}

/// Ana süreç alt süreci başlatır; alt süreç ayırdığı son değerleri dosyaya yazar ve destructor
/// çalıştırmadan ölür. Ana süreç aynı dosyayı açar: her sayacın sonraki değeri alt sürecin
/// raporladığından büyüktür.
#[test]
fn reserved_values_survive_process_exit_without_drop() {
    if let Some(dir) = std::env::var_os(CHILD_DIR_ENV) {
        child_reserve_then_exit(Path::new(&dir));
    }
    let dir = TempDir::new("exit");
    let output = Command::new(std::env::current_exe().expect("test ikilisinin yolu"))
        .args([CHILD_TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_DIR_ENV, dir.path())
        .output()
        .expect("alt süreç başlatılamadı");
    assert!(
        output.status.success(),
        "alt süreç başarısız ({:?}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let report =
        std::fs::read_to_string(dir.path().join(REPORT_FILE)).expect("alt süreç raporu yok");
    let reported: Vec<u64> = report
        .lines()
        .map(|line| line.parse().expect("rapor satırı sayı değil"))
        .collect();
    assert_eq!(reported.len(), CounterKind::ALL.len() + 1, "{report}");

    let store = open(&dir);
    for (kind, &before) in CounterKind::ALL.iter().zip(&reported) {
        let after = AppStore::reserve_next(&store, peer(1), *kind).expect("ayrılamadı");
        assert!(after > before, "{kind:?}: {after} ≤ alt sürecin {before}");
    }
    let before = reported[CounterKind::ALL.len()];
    let after = CounterStore::reserve_next(&store).expect("ayrılamadı");
    assert!(
        after > before,
        "init_counter: {after} ≤ alt sürecin {before}"
    );
}

/// Alt süreç gövdesi: değer ayırır, raporlar ve `store` DÜŞÜRÜLMEDEN süreci bitirir.
fn child_reserve_then_exit(dir: &Path) -> ! {
    let store = RedbStore::open(&dir.join(DB_FILE)).expect("alt süreç depoyu açamadı");
    let mut reported = Vec::new();
    for kind in CounterKind::ALL {
        let mut last = 0;
        for _ in 0..PER_RUN {
            last = AppStore::reserve_next(&store, peer(1), kind).expect("alt süreç ayıramadı");
        }
        reported.push(last);
    }
    let mut last = 0;
    for _ in 0..PER_RUN {
        last = CounterStore::reserve_next(&store).expect("alt süreç ayıramadı");
    }
    reported.push(last);
    let report: Vec<String> = reported.iter().map(u64::to_string).collect();
    std::fs::write(dir.join(REPORT_FILE), report.join("\n")).expect("rapor yazılamadı");
    // Destructor'lar ÇALIŞMAZ: `store` kapatılmadan süreç biter.
    std::process::exit(0)
}

/// Aynı dosya, ilk depo yaşarken ikinci kez açılamaz (redb dosya kilidi): iki depo aynı sayacı
/// yarıştıramaz. İlk depo kapanınca dosya yeniden açılır ve sayaç kaldığı yerden sürer.
#[test]
fn second_open_of_the_same_file_is_refused_while_the_first_lives() {
    let dir = TempDir::new("lock");
    let first = open(&dir);
    assert!(matches!(
        RedbStore::open(&dir.db()),
        Err(StoreOpenError::AlreadyOpen)
    ));
    assert_eq!(
        AppStore::reserve_next(&first, peer(1), CounterKind::MsgId),
        Ok(0)
    );
    drop(first);
    let second = open(&dir);
    assert_eq!(
        AppStore::reserve_next(&second, peer(1), CounterKind::MsgId),
        Ok(1)
    );
}
