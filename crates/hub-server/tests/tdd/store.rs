//! Zarf rölesi — T-S1 … T-S15 (S4 §2.4(c)(d), §2.6, H5, H6, H11; KR-19).

use std::time::Duration;

use hub_server::config::Limits;
use hub_server::wire::{code, envelope};

use crate::harness::{data, deliver, error_code, only, sent_to, store_response, Phone, World};

const INIT: &[u8] = b"SSI1 oturum kurma zarfi (hub icin opak)";

fn bound_pair(world: &mut World) -> (Phone, Phone) {
    let (mut alice, mut bob) = (Phone::alice(), Phone::bob());
    world.bind(&mut alice);
    world.bind(&mut bob);
    (alice, bob)
}

fn assert_indexes(world: &World) {
    let table = world.hub.table();
    assert_eq!(
        table.index_sizes(),
        (table.link_count(), table.tombstone_count()),
        "T-S15"
    );
}

/// T-S1 — `STORE{0, INIT, bob}` → `{OK, h}` ve bob'a `DELIVER{h, alice, INIT, aynı zarf}`.
#[test]
fn t_s1_store_init_allocates_and_delivers() {
    let mut world = World::new();
    let (mut alice, bob) = bound_pair(&mut world);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, INIT); // yanıt + DELIVER doğrulanır
    assert_ne!(h, 0);
    let link = world.hub.table().link(h).expect("hat");
    assert_eq!((link.a, link.b), (alice.id, bob.id));
    assert_eq!(world.link_count_of(&alice), 1);
    assert_eq!(world.link_count_of(&bob), 1, "hat iki uçta sayılır");
    assert!(world
        .net
        .logs
        .iter()
        .any(|l| *l == format!("link open h={h:08x} a=a1b2c3d4 b=c3d4e5f6 type=01")));
    assert_indexes(&world);
}

/// T-S2 — aynı istek tekrar (aynı request_id + alıcı + tip + zarf) → aynı h, bob'a yine DELIVER,
/// `alloc_rate` harcanmaz.
#[test]
fn t_s2_repeated_store_returns_the_same_handle_and_redelivers() {
    let mut world = World::with_limits(Limits {
        link_alloc_rate: hub_server::rate::Rate::new(1, 1),
        ..Limits::S4
    });
    let (mut alice, bob) = bound_pair(&mut world);
    let (rid, request) = alice.init(&bob, INIT);
    let first = world.deliver(alice.addr, &request);
    let h = store_response(sent_to(&first, alice.addr)).assigned_handle;
    for _ in 0..5 {
        let again = world.deliver(alice.addr, &request);
        assert_eq!(
            store_response(sent_to(&again, alice.addr)).assigned_handle,
            h
        );
        assert_eq!(
            deliver(sent_to(&again, bob.addr)),
            (h, alice.id, envelope::SESSION_INIT, INIT.to_vec())
        );
    }
    assert_eq!(world.hub.table().link_count(), 1);
    assert_eq!(world.hub.stats().store_repeat, 5);
    assert_eq!(
        world.hub.stats().rate_alloc,
        0,
        "tekrar tahsis bütçesi harcamaz"
    );
    let _ = rid;
}

/// T-S3 — aynı request_id, farklı alıcı → yeni h, doğru alıcıya DELIVER.
#[test]
fn t_s3_same_request_id_to_another_recipient_is_a_new_link() {
    let mut world = World::new();
    let (mut alice, bob) = bound_pair(&mut world);
    let mut carol = Phone::carol();
    world.bind(&mut carol);
    let rid = alice.rid();
    let to_bob = world.deliver(
        alice.addr,
        &alice.store(rid, 0, envelope::SESSION_INIT, &bob, INIT),
    );
    let to_carol = world.deliver(
        alice.addr,
        &alice.store(rid, 0, envelope::SESSION_INIT, &carol, INIT),
    );
    let h_bob = store_response(sent_to(&to_bob, alice.addr)).assigned_handle;
    let h_carol = store_response(sent_to(&to_carol, alice.addr)).assigned_handle;
    assert_ne!(h_bob, h_carol);
    assert_eq!(deliver(sent_to(&to_carol, carol.addr)).0, h_carol);
}

/// T-S4 — bob bağlı değil → yanıt `PEER_UNBOUND`, hat yok.
#[test]
fn t_s4_unbound_recipient_is_peer_unbound() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.bind(&mut alice);
    let (_, request) = alice.init(&Phone::bob(), INIT);
    let out = world.deliver(alice.addr, &request);
    let response = store_response(&only(&out).1);
    assert_eq!(
        (response.status, response.assigned_handle),
        (code::PEER_UNBOUND, 0)
    );
    assert_eq!(world.hub.table().link_count(), 0);
}

/// T-S5 — `STORE{h, RESPONSE, alice}` bob'dan → alice'e `DELIVER{h, bob, RESPONSE, zarf}`.
#[test]
fn t_s5_response_on_the_link_is_delivered_to_the_initiator() {
    let mut world = World::new();
    let (mut alice, mut bob) = bound_pair(&mut world);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, INIT);
    let rid = bob.rid();
    let out = world.deliver(
        bob.addr,
        &bob.store(rid, h, envelope::SESSION_INIT_RESPONSE, &alice, b"SSR1"),
    );
    let response = store_response(sent_to(&out, bob.addr));
    assert_eq!((response.status, response.assigned_handle), (code::OK, h));
    assert_eq!(
        deliver(sent_to(&out, alice.addr)),
        (h, bob.id, envelope::SESSION_INIT_RESPONSE, b"SSR1".to_vec())
    );
}

/// T-S6 — `STORE{h, RESPONSE, carol}` (alıcı karşı uç değil) → `BAD_REQUEST`.
#[test]
fn t_s6_response_to_someone_other_than_the_peer_is_bad_request() {
    let mut world = World::new();
    let (mut alice, mut bob) = bound_pair(&mut world);
    let mut carol = Phone::carol();
    world.bind(&mut carol);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, INIT);
    let rid = bob.rid();
    let out = world.deliver(
        bob.addr,
        &bob.store(rid, h, envelope::SESSION_INIT_RESPONSE, &carol, b"SSR1"),
    );
    assert_eq!(error_code(&only(&out).1), code::BAD_REQUEST);
}

/// T-S7 — `STORE{h,…}` hattın ucu olmayan carol'dan → `BAD_REQUEST`.
#[test]
fn t_s7_store_on_a_link_by_a_stranger_is_bad_request() {
    let mut world = World::new();
    let (mut alice, bob) = bound_pair(&mut world);
    let mut carol = Phone::carol();
    world.bind(&mut carol);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, INIT);
    let rid = carol.rid();
    let out = world.deliver(
        carol.addr,
        &carol.store(rid, h, envelope::SESSION_INIT_RESPONSE, &bob, b"x"),
    );
    assert_eq!(error_code(&only(&out).1), code::BAD_REQUEST);
}

/// T-S8 — `{0, RESPONSE}`, `{h, INIT}`, tip 0x03/0x04 → `BAD_REQUEST`.
#[test]
fn t_s8_invalid_handle_type_combinations_are_bad_request() {
    let mut world = World::new();
    let (mut alice, bob) = bound_pair(&mut world);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, INIT);
    for (handle, kind) in [
        (0, envelope::SESSION_INIT_RESPONSE),
        (h, envelope::SESSION_INIT),
        (0, 0x03),
        (0, 0x04),
        (h, 0x03),
        (h, 0x04),
        (0, 0x00),
        (0, 0xff),
    ] {
        let rid = alice.rid();
        let out = world.deliver(alice.addr, &alice.store(rid, handle, kind, &bob, b"x"));
        assert_eq!(
            error_code(&only(&out).1),
            code::BAD_REQUEST,
            "{handle} {kind}"
        );
    }
    // Alıcı dizinde değil ya da gönderenin kendisi.
    for to in [Phone::mallory(), alice.clone()] {
        let rid = alice.rid();
        let out = world.deliver(
            alice.addr,
            &alice.store(rid, 0, envelope::SESSION_INIT, &to, b"x"),
        );
        assert_eq!(error_code(&only(&out).1), code::BAD_REQUEST);
    }
    assert_eq!(world.hub.table().link_count(), 1);
}

/// T-S9 — bağsız adresten STORE → sessiz.
#[test]
fn t_s9_store_from_an_unbound_address_is_silent() {
    let mut world = World::new();
    let (alice, bob) = bound_pair(&mut world);
    let mut imposter = alice.moved_to([203, 0, 113, 77], 7_777);
    let (_, request) = imposter.init(&bob, INIT);
    assert!(world.deliver(imposter.addr, &request).is_empty());
    assert_eq!(world.hub.stats().src_unbound, 1);
    assert_eq!(
        world.hub.table().bound_addr(&alice.id),
        Some(alice.addr),
        "H4"
    );
}

/// T-S10 — `RELAY_OPEN` 16 B → INIT ile aynı yol; 65 B → `BAD_REQUEST`; `{h, RELAY_OPEN}` → `BAD_REQUEST`.
#[test]
fn t_s10_relay_open_is_an_ordinary_second_link() {
    let mut world = World::new();
    let (mut alice, bob) = bound_pair(&mut world);
    let session = world.open(&mut alice, &bob, envelope::SESSION_INIT, INIT);
    let media = world.open(&mut alice, &bob, envelope::RELAY_OPEN, &[0x6e; 16]);
    assert_ne!(media, session);
    assert_eq!(world.link_count_of(&alice), 2);
    for (handle, size) in [(0u32, 65usize), (media, 16)] {
        let rid = alice.rid();
        let out = world.deliver(
            alice.addr,
            &alice.store(rid, handle, envelope::RELAY_OPEN, &bob, &vec![1; size]),
        );
        assert_eq!(
            error_code(&only(&out).1),
            code::BAD_REQUEST,
            "{handle} {size}"
        );
    }
    let rid = alice.rid();
    let out = world.deliver(
        alice.addr,
        &alice.store(rid, 0, envelope::RELAY_OPEN, &bob, &[1; 64]),
    );
    assert_eq!(
        store_response(sent_to(&out, alice.addr)).status,
        code::OK,
        "64 B sınırda"
    );
    // İki hat ayrı ayrı iletir.
    for h in [session, media] {
        let datagram = data(h, b"veri");
        assert_eq!(
            only(&world.deliver(bob.addr, &datagram)),
            &(alice.addr, datagram.clone())
        );
    }
}

/// T-S11 — `STORE_RATE` aşımı → `RATE_LIMITED`; `LINK_ALLOC_RATE` aşımı → `RATE_LIMITED` (tekrarlar
/// tahsis sayılmaz).
#[test]
fn t_s11_store_and_allocation_rates() {
    let mut world = World::new();
    let (mut alice, mut bob) = bound_pair(&mut world);
    // LINK_ALLOC_RATE: patlama 10, sonra 1/s.
    for n in 0..10 {
        let (_, request) = alice.init(&bob, format!("INIT {n}").as_bytes());
        assert_eq!(
            store_response(sent_to(&world.deliver(alice.addr, &request), alice.addr)).status,
            code::OK
        );
    }
    let (_, request) = alice.init(&bob, b"INIT 10");
    assert_eq!(
        error_code(&only(&world.deliver(alice.addr, &request)).1),
        code::RATE_LIMITED
    );
    assert_eq!(world.hub.stats().rate_alloc, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(
        store_response(sent_to(&world.deliver(alice.addr, &request), alice.addr)).status,
        code::OK
    );

    // STORE_RATE: patlama 20 (11 kullanıldı), tekrarlar da STORE sayılır ama tahsis sayılmaz.
    let (_, repeat) = bob.init(&alice, b"bob INIT");
    let mut limited = 0;
    for _ in 0..30 {
        let out = world.deliver(bob.addr, &repeat);
        if out.len() == 1 && error_code(&out[0].1) == code::RATE_LIMITED {
            limited += 1;
        }
    }
    assert_eq!(limited, 10, "bob: 20 jetonluk patlama, 30 istek");
    assert_eq!(world.hub.stats().rate_store, 10);
}

/// T-S12 — aktif anahtar unutulmaz (K7): arada başka çiftler arasında 200 hat açılıp kapanır; K
/// yinelenir → aynı h, yeniden DELIVER; h üzerinden veri iki yönde geçer.
#[test]
fn t_s12_an_active_key_is_never_forgotten() {
    let mut world = World::with_limits(Limits {
        max_links_per_endpoint: 4,
        ..Limits::S4
    });
    let (mut alice, bob) = bound_pair(&mut world);
    let (mut carol, mut dave) = (Phone::carol(), Phone::dave());
    world.bind(&mut carol);
    world.bind(&mut dave);
    let (_, k) = alice.init(&bob, INIT);
    let h = store_response(sent_to(&world.deliver(alice.addr, &k), alice.addr)).assigned_handle;

    for n in 0..200 {
        world.advance(Duration::from_secs(1));
        world.open(
            &mut carol,
            &dave,
            envelope::SESSION_INIT,
            format!("c-d {n}").as_bytes(),
        );
    }
    assert!(world.hub.table().tombstone_count() >= 190, "çoğu kapandı");

    let again = world.deliver(alice.addr, &k);
    assert_eq!(
        store_response(sent_to(&again, alice.addr)).assigned_handle,
        h
    );
    assert_eq!(deliver(sent_to(&again, bob.addr)).0, h);
    for (from, to) in [(&alice, &bob), (&bob, &alice)] {
        let datagram = data(h, b"hala aktif");
        assert_eq!(
            only(&world.deliver(from.addr, &datagram)),
            &(to.addr, datagram.clone())
        );
    }
    assert_indexes(&world);
}

/// T-S13 — emekli anahtar: h emekli edildikten sonra K yinelenir → `{UNKNOWN_HANDLE, 0}`, tahsis yok,
/// DELIVER yok.
#[test]
fn t_s13_a_retired_key_is_unknown_handle_without_allocation() {
    let mut world = World::with_limits(Limits {
        max_links_per_endpoint: 1,
        ..Limits::S4
    });
    let (mut alice, bob) = bound_pair(&mut world);
    let (_, k) = alice.init(&bob, INIT);
    world.deliver(alice.addr, &k);
    world.advance(Duration::from_secs(1));
    world.open(&mut alice, &bob, envelope::SESSION_INIT, b"yeni INIT"); // eski hat emekli (kota 1)
    assert_eq!(world.hub.table().tombstone_count(), 1);
    let links = world.hub.table().link_count();

    let out = world.deliver(alice.addr, &k);
    let response = store_response(&only(&out).1);
    assert_eq!(
        (response.status, response.assigned_handle),
        (code::UNKNOWN_HANDLE, 0)
    );
    assert_eq!(world.hub.table().link_count(), links);
    assert_eq!(world.hub.stats().store_key_retired, 1);
    assert_indexes(&world);
}

/// T-S14 — tombstone süresi dolunca (7 gün) K yinelenir → normal yeni tahsis (yeni h).
#[test]
fn t_s14_after_seven_days_the_key_allocates_again() {
    let mut world = World::with_limits(Limits {
        max_links_per_endpoint: 1,
        ..Limits::S4
    });
    let (mut alice, bob) = bound_pair(&mut world);
    let (_, k) = alice.init(&bob, INIT);
    let old = store_response(sent_to(&world.deliver(alice.addr, &k), alice.addr)).assigned_handle;
    world.advance(Duration::from_secs(1));
    world.open(&mut alice, &bob, envelope::SESSION_INIT, b"yeni INIT");
    world.advance(Duration::from_secs(7 * 86_400));
    world.tick();
    assert_eq!(world.hub.table().tombstone_count(), 0);
    let out = world.deliver(alice.addr, &k);
    let response = store_response(sent_to(&out, alice.addr));
    assert_eq!(response.status, code::OK);
    assert_ne!(response.assigned_handle, old);
    assert_indexes(&world);
}

/// T-S15 — indeks boyutu: her adımda `len(by_key) == len(links)` ve `len(tomb_by_key) ==
/// len(tombstones)`. (`World::deliver` her datagramdan sonra bütün değişmezleri denetler; burada
/// emeklilik ağırlıklı bir dizi koşulur.)
#[test]
fn t_s15_index_sizes_track_links_and_tombstones() {
    let mut world = World::with_limits(Limits {
        max_links_per_endpoint: 3,
        max_links: 5,
        ..Limits::S4
    });
    let mut phones = Phone::directory();
    for phone in &mut phones {
        world.bind(phone);
    }
    for n in 0..300usize {
        world.advance(Duration::from_millis(1_100));
        let (i, j) = (n % 6, (n * 7 + 1) % 6);
        if i == j {
            continue;
        }
        let to = phones[j].clone();
        let (_, request) = phones[i].init(&to, format!("{n}").as_bytes());
        world.deliver(phones[i].addr, &request);
        assert_indexes(&world);
    }
    assert!(world.hub.table().tombstone_count() > 100);
}
