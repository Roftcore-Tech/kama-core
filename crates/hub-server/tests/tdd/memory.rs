//! Bellek ve kapasite (sahte saat) — T-M1 … T-M9 (S4 §2.6, H9; KR-9, KR-19; kapanış vakaları K4–K6).
//! T-M5 (aday tükenmesi) adayları önceden kurgulamayı gerektirdiği için `src/table.rs` birim
//! testidir: `t_m5_candidate_exhaustion_never_overwrites`.

use std::time::Duration;

use hub_server::config::Limits;
use hub_server::rate::Rate;
use hub_server::wire::{code, envelope};

use crate::harness::{data, error_code, only, sent_to, store_response, Phone, TestRng, World};

/// `n` hat açar (tahsis hızına takılmamak için her açılıştan önce 1 s); handle'ları döner.
fn open_many(world: &mut World, from: &mut Phone, to: &Phone, kind: u8, n: usize) -> Vec<u32> {
    (0..n)
        .map(|i| {
            world.advance(Duration::from_secs(1));
            let env = if kind == envelope::RELAY_OPEN {
                vec![u8::try_from(i % 256).expect("u8"); 16]
            } else {
                format!("INIT {i}").into_bytes()
            };
            world.open(from, to, kind, &env)
        })
        .collect()
}

fn bound(world: &mut World) -> Vec<Phone> {
    let mut phones = Phone::directory();
    for phone in &mut phones {
        world.bind(phone);
    }
    phones
}

/// T-M1 — gönderen kotası: alice'in 65. hattı → alice'in en-uzun-boşta hattı emekli, diğer uçların
/// hatları etkilenmez; iki ucun sayacı da −1.
#[test]
fn t_m1_sender_quota_retires_the_senders_oldest_link() {
    let mut world = World::new();
    let mut phones = bound(&mut world);
    let (bob, carol) = (phones[1].clone(), phones[2].clone());
    let alice = &mut phones[0];
    let first = open_many(&mut world, alice, &bob, envelope::SESSION_INIT, 1)[0];
    let rest = open_many(&mut world, alice, &carol, envelope::SESSION_INIT, 63);
    assert_eq!(world.link_count_of(alice), 64);
    let bob_before = world.link_count_of(&bob);

    let newest = open_many(&mut world, alice, &carol, envelope::SESSION_INIT, 1)[0];
    assert!(
        world.hub.table().link(first).is_none() && world.hub.table().tombstone(first).is_some()
    );
    assert_eq!(world.link_count_of(alice), 64);
    assert_eq!(
        world.link_count_of(&bob),
        bob_before - 1,
        "emekli hattın karşı ucu da −1"
    );
    assert!(rest
        .iter()
        .chain([&newest])
        .all(|h| world.hub.table().link(*h).is_some()));
    assert!(world
        .net
        .logs
        .iter()
        .any(|l| *l == format!("link retire h={first:08x}")));
}

/// T-M1b (K4) — alice–bob: 1 kurulu oturum + 63 kapalı medya hattı (ikisi de 64'te); alice yeni
/// RELAY_OPEN → yalnız alice–bob ortak en-uzun-boşta medya hattı emekli; iki uç ≤ 64; oturum dokunulmamış.
#[test]
fn t_m1b_recipient_quota_with_a_shared_link_replaces_the_shared_idle_link() {
    let mut world = World::new();
    let mut phones = bound(&mut world);
    let bob = phones[1].clone();
    let alice = &mut phones[0];
    let session = open_many(&mut world, alice, &bob, envelope::SESSION_INIT, 1)[0];
    let media = open_many(&mut world, alice, &bob, envelope::RELAY_OPEN, 63);
    // Oturum kullanımda: en uzun boşta olan değil.
    let datagram = data(session, b"canli");
    world.deliver(alice.addr, &datagram);
    assert_eq!(
        (world.link_count_of(alice), world.link_count_of(&bob)),
        (64, 64)
    );

    let new_media = open_many(&mut world, alice, &bob, envelope::RELAY_OPEN, 1)[0];
    assert!(
        world.hub.table().tombstone(media[0]).is_some(),
        "en eski medya hattı emekli"
    );
    assert!(
        world.hub.table().link(session).is_some(),
        "kurulu oturum dokunulmamış"
    );
    assert!(world.hub.table().link(new_media).is_some());
    assert_eq!(
        (world.link_count_of(alice), world.link_count_of(&bob)),
        (64, 64)
    );
    assert_eq!(only(&world.deliver(alice.addr, &datagram)).0, bob.addr);
}

/// T-M1c (K5) — bob 64 hatta, carol'ın bob'la ortak hattı yok → carol→bob `HUB_CAPACITY`; diğer çiftlerin
/// hatları ve sayaçlar aynen.
#[test]
fn t_m1c_recipient_quota_without_a_shared_link_is_hub_capacity() {
    let mut world = World::new();
    let mut phones = bound(&mut world);
    let bob = phones[1].clone();
    open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 64);
    let before = world.hub.table().capacity_snapshot();
    world.advance(Duration::from_secs(1));
    let carol = &mut phones[2];
    let (_, request) = carol.init(&bob, b"INIT");
    let out = world.deliver(carol.addr, &request);
    assert_eq!(error_code(&only(&out).1), code::HUB_CAPACITY);
    assert_eq!(world.hub.table().capacity_snapshot(), before);
}

/// T-M1d — üçüncü taraf seçilemez: bob 64 hatta, carol'ın bob'la 1 ortak hattı var → yalnız o hat
/// emekli; alice–bob hatlarına dokunulmaz.
#[test]
fn t_m1d_only_the_shared_link_of_the_requesting_pair_is_eligible() {
    let mut world = World::new();
    let mut phones = bound(&mut world);
    let bob = phones[1].clone();
    let alice_links = open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 63);
    let shared = open_many(&mut world, &mut phones[2], &bob, envelope::SESSION_INIT, 1)[0];
    // Carol–bob hattı en YENİ hat; yine de seçilebilecek tek hat odur.
    let replacement = open_many(&mut world, &mut phones[2], &bob, envelope::SESSION_INIT, 1)[0];
    assert!(world.hub.table().tombstone(shared).is_some());
    assert!(alice_links
        .iter()
        .all(|h| world.hub.table().link(*h).is_some()));
    assert!(world.hub.table().link(replacement).is_some());
    assert_eq!(world.link_count_of(&bob), 64);
}

/// T-M2 — `MAX_LINKS` dolu → küresel en-uzun-boşta emekli.
#[test]
fn t_m2_global_ceiling_retires_the_globally_oldest_idle_link() {
    let mut world = World::with_limits(Limits {
        max_links: 6,
        ..Limits::S4
    });
    let mut phones = bound(&mut world);
    let (bob, dave, frank) = (phones[1].clone(), phones[3].clone(), phones[5].clone());
    let oldest = open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 1)[0];
    open_many(&mut world, &mut phones[2], &dave, envelope::SESSION_INIT, 5);
    assert_eq!(world.hub.table().link_count(), 6);
    open_many(
        &mut world,
        &mut phones[4],
        &frank,
        envelope::SESSION_INIT,
        1,
    );
    assert_eq!(world.hub.table().link_count(), 6);
    assert!(world.hub.table().tombstone(oldest).is_some());
}

/// T-M3 — tombstone'daki h 7 gün boyunca durur (yeniden verilmez: aday üretici onu atlar — birim
/// testi `table::tests`), 7 gün dolunca kalkar.
#[test]
fn t_m3_tombstones_last_seven_days() {
    let mut world = World::with_limits(Limits {
        max_links_per_endpoint: 1,
        ..Limits::S4
    });
    let mut phones = bound(&mut world);
    let bob = phones[1].clone();
    let old = open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 1)[0];
    open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 1);
    assert!(world.hub.table().tombstone(old).is_some());
    world.advance(Duration::from_secs(7 * 86_400 - 2));
    world.tick();
    assert!(
        world.hub.table().tombstone(old).is_some(),
        "7 gün dolmadan silinmez"
    );
    world.advance(Duration::from_secs(60));
    world.tick();
    assert!(world.hub.table().tombstone(old).is_none());
}

/// T-M4 (K6) — alice 64 hatta ve tombstone tablosu dolu → yeni hat `HUB_CAPACITY`; alice'in 64 hattı,
/// uç sayaçları ve tombstone içeriği aynen; yalnız tahsis bütçesi ve `alloc_counter` ilerleyebilir.
#[test]
fn t_m4_capacity_plan_failure_changes_nothing() {
    let mut world = World::with_limits(Limits {
        max_tombstones: 0,
        ..Limits::S4
    });
    let mut phones = bound(&mut world);
    let carol = phones[2].clone();
    open_many(
        &mut world,
        &mut phones[0],
        &carol,
        envelope::SESSION_INIT,
        64,
    );
    let before = world.hub.table().capacity_snapshot();
    let counter = world.hub.table().alloc_counter();
    world.advance(Duration::from_secs(1));
    let bob = phones[1].clone();
    let alice = &mut phones[0];
    let (_, request) = alice.init(&bob, b"65.");
    assert_eq!(
        error_code(&only(&world.deliver(alice.addr, &request)).1),
        code::HUB_CAPACITY
    );
    assert_eq!(world.hub.table().capacity_snapshot(), before);
    assert!(world.hub.table().alloc_counter() >= counter);
}

/// T-M6 — 6 saat sessiz uç ve hat → hiçbir şey silinmez.
#[test]
fn t_m6_silence_deletes_nothing() {
    let mut world = World::new();
    let mut phones = bound(&mut world);
    let bob = phones[1].clone();
    let h = open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 1)[0];
    let before = world.hub.table().capacity_snapshot();
    for _ in 0..(6 * 60) {
        world.advance(Duration::from_secs(60));
        world.tick();
    }
    assert_eq!(world.hub.table().capacity_snapshot(), before);
    assert_eq!(world.hub.table().endpoint_count(), 6);
    let datagram = data(h, b"6 saat sonra");
    assert_eq!(only(&world.deliver(phones[0].addr, &datagram)).0, bob.addr);
}

/// T-M7 — cookie rotasyonu sınırında önceki anahtarla verilmiş nonce kabul.
#[test]
fn t_m7_a_cookie_issued_before_rotation_is_accepted_after_it() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.advance(Duration::from_secs(3590));
    let (_, cn, hn) = world.challenge(&mut alice);
    world.advance(Duration::from_secs(10));
    let logs = world.tick();
    assert!(logs.iter().any(|l| l == "cookie rotate"), "{logs:?}");
    let rid = alice.rid();
    let out = world.deliver(alice.addr, &alice.bind2(rid, &world.hub_pub, cn, hn));
    world.expect_bound(&only(&out).1, &alice, rid, cn, hn);
}

/// T-M8 — tavanlar hiçbir anda aşılmaz: rastgele 100 000 STORE dizisi (emeklilik planlayıcısıyla) boyunca
/// değişmezler her adımda tutar ve her `HUB_CAPACITY` yanıtında tablo istek öncesiyle özdeş.
#[test]
fn t_m8_ceilings_hold_and_capacity_refusals_change_nothing() {
    let mut world = World::with_limits(Limits {
        max_links: 8,
        max_links_per_endpoint: 3,
        max_tombstones: 40,
        handle_tombstone: Duration::from_secs(120),
        sweep_period: Duration::from_secs(1),
        link_alloc_rate: Rate::new(1_000_000, 1_000_000),
        store_rate: Rate::new(1_000_000, 1_000_000),
        ..Limits::S4
    });
    let mut phones = bound(&mut world);
    let mut rng = TestRng(0x0e8);
    let mut capacity = 0;
    let mut opened = 0;
    for step in 0..100_000 {
        let (i, j) = (rng.below(6), rng.below(6));
        if i == j {
            continue;
        }
        let to = phones[j].clone();
        let kind = if rng.below(4) == 0 {
            envelope::RELAY_OPEN
        } else {
            envelope::SESSION_INIT
        };
        let rid = phones[i].rid();
        let request = phones[i].store(rid, 0, kind, &to, &rng.bytes(16));
        let before = world.hub.table().capacity_snapshot();
        let out = world.deliver(phones[i].addr, &request);
        let answer = sent_to(&out, phones[i].addr);
        if hub_server::wire::flags::RESPONSE_ERROR == crate::harness::reply(answer).flags {
            assert_eq!(error_code(answer), code::HUB_CAPACITY);
            assert_eq!(world.hub.table().capacity_snapshot(), before, "adım {step}");
            capacity += 1;
        } else {
            assert_eq!(store_response(answer).status, code::OK);
            opened += 1;
        }
        if step % 10 == 0 {
            world.advance(Duration::from_secs(1));
            world.tick();
        }
    }
    assert!(
        capacity > 1_000 && opened > 1_000,
        "iki dal da koştu: capacity={capacity} opened={opened}"
    );
}

/// T-M9 — plan kümedir: iki kota aynı ikilide doluyken tek hat emekli edilir, tombstone +1.
#[test]
fn t_m9_one_retirement_when_both_quotas_point_at_the_same_pair() {
    let mut world = World::with_limits(Limits {
        max_links_per_endpoint: 2,
        ..Limits::S4
    });
    let mut phones = bound(&mut world);
    let bob = phones[1].clone();
    open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 2);
    let tombstones = world.hub.table().tombstone_count();
    open_many(&mut world, &mut phones[0], &bob, envelope::SESSION_INIT, 1);
    assert_eq!(world.hub.table().tombstone_count(), tombstones + 1);
    assert_eq!(
        (world.link_count_of(&phones[0]), world.link_count_of(&bob)),
        (2, 2)
    );
}
