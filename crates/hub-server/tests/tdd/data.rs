//! Veri — T-D1 … T-D5 (S4 §2.1, H4, H7, KR-7).

use std::time::Duration;

use hub_server::wire::{envelope, ROUTE_MISS_UNKNOWN_HANDLE};

use crate::harness::{data, only, route_miss, Phone, TestRng, World};

fn linked(world: &mut World) -> (Phone, Phone, u32) {
    let (mut alice, mut bob) = (Phone::alice(), Phone::bob());
    world.bind(&mut alice);
    world.bind(&mut bob);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, b"INIT");
    (alice, bob, h)
}

/// T-D1 — `[h][1276 rastgele bayt]` alice'ten → bob'a bayt bayt aynı.
#[test]
fn t_d1_data_is_forwarded_byte_for_byte() {
    let mut world = World::new();
    let (alice, bob, h) = linked(&mut world);
    let mut rng = TestRng(0xd1);
    for round in 0..50 {
        let datagram = data(h, &rng.bytes(1276));
        assert_eq!(datagram.len(), 1280);
        let (from, to) = if round % 2 == 0 {
            (&alice, &bob)
        } else {
            (&bob, &alice)
        };
        assert_eq!(
            only(&world.deliver(from.addr, &datagram)),
            &(to.addr, datagram.clone())
        );
    }
    // Yük bir kontrol çerçevesine benzese de hat datagramıdır: içine bakılmaz.
    let mut lookalike = h.to_be_bytes().to_vec();
    lookalike.extend_from_slice(&Phone::alice().bind1(1, [0; 16]));
    assert_eq!(
        only(&world.deliver(alice.addr, &lookalike)),
        &(bob.addr, lookalike.clone())
    );
    // En kısa veri: handle + 1 bayt.
    let short = data(h, &[7]);
    assert_eq!(
        only(&world.deliver(alice.addr, &short)),
        &(bob.addr, short.clone())
    );
    assert_eq!(world.hub.stats().relay_packets, 52);
}

/// T-D2 — 1281 B → `drop_size` (4 baytlık, yüksüz datagram da).
#[test]
fn t_d2_oversized_or_too_short_datagram_is_dropped() {
    let mut world = World::new();
    let (alice, _bob, h) = linked(&mut world);
    assert!(world.deliver(alice.addr, &data(h, &[0; 1277])).is_empty());
    assert!(world.deliver(alice.addr, &data(h, &[])).is_empty());
    assert_eq!(world.hub.stats().drop_size, 2);
    assert_eq!(world.hub.stats().relay_packets, 0);
}

/// T-D3 — doğru h, alice'in bağlı adresinden değil → `src_mismatch` (bağlı ama uç değil) / `src_unbound`
/// (bağlı değil); alice'in adresi değişmez.
#[test]
fn t_d3_right_handle_from_the_wrong_address_is_dropped() {
    let mut world = World::new();
    let (alice, _bob, h) = linked(&mut world);
    let mut carol = Phone::carol();
    world.bind(&mut carol);
    let datagram = data(h, b"ustume cek");
    assert!(world.deliver(carol.addr, &datagram).is_empty());
    assert_eq!(world.hub.stats().src_mismatch, 1);
    let spoof = alice.moved_to([198, 51, 100, 7], 40_002);
    assert!(world.deliver(spoof.addr, &datagram).is_empty());
    assert_eq!(world.hub.stats().src_unbound, 1);
    assert_eq!(
        world.hub.table().bound_addr(&alice.id),
        Some(alice.addr),
        "H4"
    );
}

/// T-D4 — bilinmeyen h bağlı alice'ten → `ROUTE_MISS{h, UNKNOWN_HANDLE}`, en çok `ROUTE_MISS_RATE`;
/// bağsız adresten → sessiz.
#[test]
fn t_d4_unknown_handle_route_miss_is_rate_limited_and_only_for_bound_sources() {
    let mut world = World::new();
    let (alice, _bob, h) = linked(&mut world);
    let unknown = h ^ 0x0101_0101;
    let out = world.deliver(alice.addr, &data(unknown, b"veri"));
    let miss = route_miss(&only(&out).1);
    assert_eq!(
        (miss.hub_handle, miss.reason),
        (unknown, ROUTE_MISS_UNKNOWN_HANDLE)
    );
    for _ in 0..10 {
        assert!(
            world
                .deliver(alice.addr, &data(unknown, b"veri"))
                .is_empty(),
            "1/s"
        );
    }
    world.advance(Duration::from_secs(1));
    assert_eq!(world.deliver(alice.addr, &data(unknown, b"veri")).len(), 1);
    assert!(world
        .deliver(Phone::mallory().addr, &data(unknown, b"veri"))
        .is_empty());
    assert_eq!(world.hub.stats().route_miss, 2);
    assert_eq!(world.hub.stats().drop_unknown_link, 11);
}

/// T-D5 — karşı uç bağlı değil → `drop_peer_unbound`. (Uç kaydı silinmez; bob'un adresini başka bir
/// kimlik kanıtlayınca bob'un dönüş adresi düşer.)
#[test]
fn t_d5_peer_without_a_return_address_is_dropped() {
    let mut world = World::new();
    let (alice, bob, h) = linked(&mut world);
    let mut carol_at_bobs = Phone::carol().moved_to([192, 0, 2, 33], 50_022);
    world.bind(&mut carol_at_bobs);
    assert_eq!(world.hub.table().bound_addr(&bob.id), None);
    assert!(world.deliver(alice.addr, &data(h, b"veri")).is_empty());
    assert_eq!(world.hub.stats().drop_peer_unbound, 1);
}
