//! Heartbeat — T-H1 … T-H3 (S4 §2.2, H8).

use hub_server::wire::{classify, frame, Datagram};

use crate::harness::{heartbeat, only, Phone, World};

/// T-H1 — bağlı alice, token'lı → ACK aynı uzunluk, token aynı (boş gövde de).
#[test]
fn t_h1_bound_heartbeat_is_echoed_with_the_same_length() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.bind(&mut alice);
    for token in [&[0xde, 0xad, 0xbe, 0xef][..], &[][..]] {
        let request = heartbeat(token);
        let out = world.deliver(alice.addr, &request);
        let (to, bytes) = only(&out);
        assert_eq!(*to, alice.addr);
        assert_eq!(bytes.len(), request.len());
        assert_eq!(
            classify(bytes),
            Ok(Datagram::Control {
                frame_type: frame::HEARTBEAT_ACK,
                body: token
            })
        );
    }
}

/// T-H2 — bağsız adres → cevap yok; slot yaratmaz, adres değiştirmez.
#[test]
fn t_h2_unbound_heartbeat_gets_no_answer() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.bind(&mut alice);
    let stranger = alice.moved_to([203, 0, 113, 77], 7_777);
    assert!(world
        .deliver(stranger.addr, &heartbeat(&[1, 2, 3, 4]))
        .is_empty());
    assert_eq!(world.hub.stats().hb_unbound, 1);
    assert!(!world.hub.is_registered_addr(stranger.addr));
    assert_eq!(world.hub.table().bound_addr(&alice.id), Some(alice.addr));
}

/// T-H3 — gövde 2 B → `drop_malformed`.
#[test]
fn t_h3_heartbeat_body_must_be_empty_or_four_bytes() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.bind(&mut alice);
    for size in [1usize, 2, 3, 5, 8] {
        assert!(
            world
                .deliver(alice.addr, &heartbeat(&vec![0; size]))
                .is_empty(),
            "{size}"
        );
    }
    assert_eq!(world.hub.stats().drop_malformed, 5);
}
