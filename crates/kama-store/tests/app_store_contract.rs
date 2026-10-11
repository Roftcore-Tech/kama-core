//! `AppStore` ortak sözleşmesi: AYNI vakalar bellek içi sahteye (`MemoryAppStore`; K4G-07/08
//! testleri onu kullanır) ve ürün deposuna (`RedbStore`) koşar — sahtenin doğruluğu ancak aynı
//! testin ikisine de koşturulmasıyla gösterilir.

mod common;

use common::{outbox_unit, peer, TempDir};
use kama_app::messaging::MAX_OUTBOX_UNACKED;
use kama_app::store::{
    AppStore, AppStoreError, CounterKind, DedupWindow, MemoryAppStore, OutboxUnit,
    DEDUP_WINDOW_WORDS, FIRST_COUNTER_VALUE, MAX_OUTBOX_RECORD_BYTES,
};
use kama_contracts::s1::CompressionPolicy;
use kama_noise_wire::CounterStore;
use kama_store::RedbStore;

/// Vakayı iki depoya da koşar; kırılan deponun adı her mesajda yazılır.
fn each_store(tag: &str, case: impl Fn(&str, &dyn AppStore)) {
    case("MemoryAppStore", &MemoryAppStore::new());
    let dir = TempDir::new(tag);
    let store = RedbStore::open(&dir.db()).expect("depo açılamadı");
    case("RedbStore", &store);
}

#[test]
fn counters_start_at_the_first_value_and_are_independent_per_peer_and_kind() {
    each_store("counters", |name, store| {
        let (a, b) = (peer(1), peer(2));
        for step in 0..3 {
            assert_eq!(
                store.reserve_next(a, CounterKind::MsgId),
                Ok(FIRST_COUNTER_VALUE + step),
                "{name}"
            );
        }
        for kind in [CounterKind::StreamId, CounterKind::TransferId] {
            assert_eq!(
                store.reserve_next(a, kind),
                Ok(FIRST_COUNTER_VALUE),
                "{name}: {kind:?}"
            );
        }
        for kind in CounterKind::ALL {
            assert_eq!(
                store.reserve_next(b, kind),
                Ok(FIRST_COUNTER_VALUE),
                "{name}: başka eş, {kind:?}"
            );
        }
        assert_eq!(
            store.reserve_next(a, CounterKind::MsgId),
            Ok(FIRST_COUNTER_VALUE + 3),
            "{name}"
        );
    });
}

/// `RedbStore`'a özgü: iki arayüz tek dosyada durur ama sayaçları birbirine dokunmaz (D8:
/// arayüzler birleştirilmez).
#[test]
fn init_counter_is_independent_of_app_counters() {
    let dir = TempDir::new("init-independent");
    let store = RedbStore::open(&dir.db()).expect("depo açılamadı");
    assert_eq!(CounterStore::reserve_next(&store), Ok(0));
    for kind in CounterKind::ALL {
        assert_eq!(
            AppStore::reserve_next(&store, peer(1), kind),
            Ok(FIRST_COUNTER_VALUE)
        );
    }
    assert_eq!(CounterStore::reserve_next(&store), Ok(1));
    assert_eq!(
        AppStore::reserve_next(&store, peer(1), CounterKind::MsgId),
        Ok(FIRST_COUNTER_VALUE + 1)
    );
}

#[test]
fn outbox_is_bounded_per_peer_and_replacing_does_not_grow_it() {
    each_store("outbox-cap", |name, store| {
        let (a, b) = (peer(1), peer(2));
        let cap = u32::try_from(MAX_OUTBOX_UNACKED).expect("tavan u32'ye sığar");
        for msg_id in 0..cap {
            if let Err(error) =
                store.outbox_put(a, &outbox_unit(msg_id, 8, CompressionPolicy::Never))
            {
                panic!("{name}: msg_id {msg_id}: {error}");
            }
        }
        assert_eq!(
            store.outbox_put(a, &outbox_unit(cap, 8, CompressionPolicy::Never)),
            Err(AppStoreError::OutboxFull),
            "{name}: yeni msg_id tavanı aşamaz"
        );
        let replacement = outbox_unit(0, 16, CompressionPolicy::AppCompressed);
        assert_eq!(
            store.outbox_put(a, &replacement),
            Ok(()),
            "{name}: aynı msg_id yerine geçer"
        );
        assert_eq!(
            store.outbox_put(b, &outbox_unit(cap, 8, CompressionPolicy::Never)),
            Ok(()),
            "{name}: başka eşin tavanı ayrıdır"
        );
        assert_eq!(store.outbox_remove(a, 1), Ok(true), "{name}");
        assert_eq!(
            store.outbox_put(a, &outbox_unit(cap, 8, CompressionPolicy::Never)),
            Ok(()),
            "{name}: silme yer açar"
        );
        let listed = store.outbox_list(a).expect("outbox okunamadı");
        assert_eq!(listed.len(), MAX_OUTBOX_UNACKED, "{name}");
        assert_eq!(listed.first(), Some(&replacement), "{name}");
    });
}

#[test]
fn outbox_lists_in_msg_id_order_and_remove_reports_presence() {
    each_store("outbox-order", |name, store| {
        let (a, b) = (peer(1), peer(2));
        for msg_id in [9, 2, u32::MAX, 0, 5] {
            store
                .outbox_put(a, &outbox_unit(msg_id, 4, CompressionPolicy::Never))
                .expect("outbox yazılamadı");
        }
        store
            .outbox_put(b, &outbox_unit(3, 4, CompressionPolicy::Never))
            .expect("outbox yazılamadı");
        let ids: Vec<u32> = store
            .outbox_list(a)
            .expect("outbox okunamadı")
            .iter()
            .map(OutboxUnit::msg_id)
            .collect();
        assert_eq!(ids, [0, 2, 5, 9, u32::MAX], "{name}");
        assert_eq!(store.outbox_remove(a, 5), Ok(true), "{name}");
        assert_eq!(store.outbox_remove(a, 5), Ok(false), "{name}: ikinci silme");
        assert_eq!(
            store.outbox_remove(a, 3),
            Ok(false),
            "{name}: başka eşin kaydı"
        );
        assert_eq!(
            store.outbox_list(b),
            Ok(vec![outbox_unit(3, 4, CompressionPolicy::Never)]),
            "{name}"
        );
        assert_eq!(store.outbox_list(peer(3)), Ok(Vec::new()), "{name}");
    });
}

#[test]
fn records_and_windows_round_trip_bit_exact() {
    each_store("round-trip", |name, store| {
        let a = peer(1);
        let units = [
            outbox_unit(1, 0, CompressionPolicy::Never),
            outbox_unit(2, 1, CompressionPolicy::AppCompressed),
            outbox_unit(3, MAX_OUTBOX_RECORD_BYTES, CompressionPolicy::Never),
        ];
        for unit in &units {
            store.outbox_put(a, unit).expect("outbox yazılamadı");
        }
        assert_eq!(store.outbox_list(a), Ok(units.to_vec()), "{name}");

        assert_eq!(
            store.dedup_load(a),
            Ok(DedupWindow::EMPTY),
            "{name}: kaydı olmayan eş"
        );
        let mut window = DedupWindow {
            highest: Some(u32::MAX),
            seen: [0; DEDUP_WINDOW_WORDS],
        };
        for (i, word) in (0u64..).zip(window.seen.iter_mut()) {
            *word = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
        store.dedup_save(a, &window).expect("dedup yazılamadı");
        assert_eq!(store.dedup_load(a), Ok(window.clone()), "{name}");
        // Depo pencereyi YORUMLAMAZ: çapasız ama bitli bir pencere de aynen döner.
        let unanchored = DedupWindow {
            highest: None,
            seen: window.seen,
        };
        store.dedup_save(a, &unanchored).expect("dedup yazılamadı");
        assert_eq!(store.dedup_load(a), Ok(unanchored), "{name}");
    });
}
