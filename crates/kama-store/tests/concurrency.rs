//! K4G-06 DONE — iki iş parçacığından eşzamanlı `reserve_next` çakışmaz: değerler benzersizdir.
//!
//! İki iş parçacığı AYNI sayacı (aynı eş + aynı tür; ayrıca `init_counter`) tek `RedbStore`
//! üzerinden yarışarak ilerletir. redb aynı anda tek yazma işlemine izin verir ve oku-artır-yaz
//! tek işlemdedir; bu yüzden dönen her değer benzersizdir, her iş parçacığının dizisi kesin
//! artandır ve hata enjekte edilmediği için birleşim boşluksuz `0..N`'dir.

mod common;

use std::collections::BTreeSet;

use common::{peer, TempDir};
use kama_app::store::{AppStore, CounterKind};
use kama_noise_wire::CounterStore;
use kama_store::RedbStore;

const THREADS: usize = 2;
const PER_THREAD: usize = 64;

fn race(store: &RedbStore, reserve: impl Fn(&RedbStore) -> u64 + Sync) {
    let per_thread: Vec<Vec<u64>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                scope.spawn(|| {
                    (0..PER_THREAD)
                        .map(|_| reserve(store))
                        .collect::<Vec<u64>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("iş parçacığı panikledi"))
            .collect()
    });
    for values in &per_thread {
        assert!(
            values.windows(2).all(|pair| pair[0] < pair[1]),
            "iş parçacığı içinde değerler kesin artan değil: {values:?}"
        );
    }
    let all: Vec<u64> = per_thread.into_iter().flatten().collect();
    let unique: BTreeSet<u64> = all.iter().copied().collect();
    assert_eq!(unique.len(), all.len(), "aynı değer iki kez döndü");
    let expected: BTreeSet<u64> = (0..(THREADS * PER_THREAD) as u64).collect();
    assert_eq!(unique, expected, "değer kaybı ya da boşluk");
}

#[test]
fn concurrent_app_counter_reservations_never_collide() {
    for kind in CounterKind::ALL {
        let dir = TempDir::new("race-app");
        let store = RedbStore::open(&dir.db()).expect("depo açılamadı");
        race(&store, |store| {
            AppStore::reserve_next(store, peer(9), kind).expect("ayrılamadı")
        });
    }
}

#[test]
fn concurrent_init_counter_reservations_never_collide() {
    let dir = TempDir::new("race-init");
    let store = RedbStore::open(&dir.db()).expect("depo açılamadı");
    race(&store, |store| {
        CounterStore::reserve_next(store).expect("ayrılamadı")
    });
}
