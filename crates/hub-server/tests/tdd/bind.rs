//! Bind — T-B1 … T-B11 (S4 §2.4(a)(b), H2, H3, H11).

use std::time::Duration;

use ed25519_dalek::SigningKey;
use hub_server::config::Limits;
use hub_server::rate::Rate;
use hub_server::wire::{code, envelope, flags, method};

use crate::harness::{data, error_code, only, reply, Phone, TestRng, World};
use hub_server::wire::Payload;

/// T-B1 — iki tur doğru → `bind_ok=1`, hub imzası doğrulanır, `endpoints` dolu, `bind_epoch = 1`.
#[test]
fn t_b1_two_rounds_bind_with_a_signed_answer() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.bind(&mut alice); // yanıt + hub imzası `World::bind` içinde doğrulanır
    let endpoint = world.hub.table().endpoint(&alice.id).expect("uç kaydı");
    assert_eq!(endpoint.addr, Some(alice.addr));
    assert_eq!(endpoint.bind_epoch, 1);
    assert_eq!(endpoint.cert_ref, alice.cert_ref);
    assert_eq!(world.hub.stats().bind_ok, 1);
    assert_eq!(world.net.logs, ["bind ok dh=a1b2c3d4 epoch=1"]);
}

/// T-B2 — dizinde olmayan `cert_ref` → iki turda da cevap yok.
#[test]
fn t_b2_unknown_cert_ref_gets_no_answer_in_either_round() {
    let mut world = World::new();
    let mut mallory = Phone::mallory();
    let rid = mallory.rid();
    assert!(world
        .deliver(mallory.addr, &mallory.bind1(rid, [1; 16]))
        .is_empty());
    let rid = mallory.rid();
    let proof = mallory.bind2(rid, &world.hub_pub, [1; 16], [2; 16]);
    assert!(world.deliver(mallory.addr, &proof).is_empty());
    assert_eq!(world.hub.stats().bind_unknown, 2);
    assert_eq!(world.hub.table().endpoint_count(), 0);
}

/// T-B3 — yanlış anahtarla / başka hub pub'ıyla imza → `POSSESSION_PROOF_FAIL` (ErrorPayload,
/// imzasız), kayıt yok, `bind_fail[src]` = 1.
#[test]
fn t_b3_wrong_key_or_wrong_hub_is_possession_proof_fail() {
    let mut world = World::new();
    let mut alice = Phone::alice();

    let (_, cn, hn) = world.challenge(&mut alice);
    let rid = alice.rid();
    let proof = alice.bind2_signed_by(
        &SigningKey::from_bytes(&[0x99; 32]),
        rid,
        &world.hub_pub,
        cn,
        hn,
    );
    let out = world.deliver(alice.addr, &proof);
    let r = reply(&only(&out).1);
    assert_eq!(
        (r.method_id, r.flags, r.request_id),
        (method::BIND_PROOF, flags::RESPONSE_ERROR, rid)
    );
    assert_eq!(error_code(&only(&out).1), code::POSSESSION_PROOF_FAIL);
    assert_eq!(world.hub.bind_fail_entries(), 1, "bind_fail[src] yazıldı");

    let (_, cn, hn) = world.challenge(&mut alice);
    let rid = alice.rid();
    let other_hub = SigningKey::from_bytes(&[0x77; 32])
        .verifying_key()
        .to_bytes();
    let out = world.deliver(alice.addr, &alice.bind2(rid, &other_hub, cn, hn));
    assert_eq!(error_code(&only(&out).1), code::POSSESSION_PROOF_FAIL);

    assert_eq!(world.hub.table().endpoint_count(), 0, "kayıt yok");
    assert_eq!(world.hub.stats().bind_fail, 2);
}

/// T-B4 — 2 dakika eski `hub_nonce` → `BAD_COOKIE`; imza doğrulaması hiç çağrılmaz; `bind_fail` boş.
#[test]
fn t_b4_two_minute_old_cookie_is_bad_cookie_and_costs_no_verification() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let (_, cn, hn) = world.challenge(&mut alice);
    world.advance(Duration::from_secs(120));
    let rid = alice.rid();
    let out = world.deliver(alice.addr, &alice.bind2(rid, &world.hub_pub, cn, hn));
    assert_eq!(error_code(&only(&out).1), code::BAD_COOKIE);
    assert_eq!(
        world.hub.stats().verify_calls,
        0,
        "imza doğrulaması çağrılmadı"
    );
    assert_eq!(world.hub.bind_fail_entries(), 0);
    assert_eq!(world.hub.table().endpoint_count(), 0);
}

/// T-B5 — aynı kimlik yeni nonce'la yeni adresten → adres taşınır, hatlar korunur, eski adresten veri
/// düşer.
#[test]
fn t_b5_rebinding_from_a_new_address_moves_it_and_keeps_links() {
    let mut world = World::new();
    let (mut alice, mut bob) = (Phone::alice(), Phone::bob());
    world.bind(&mut alice);
    world.bind(&mut bob);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, b"INIT");

    let mut moved = alice.moved_to([100, 64, 9, 9], 31_337);
    world.bind(&mut moved);
    let endpoint = world.hub.table().endpoint(&alice.id).expect("uç");
    assert_eq!((endpoint.addr, endpoint.bind_epoch), (Some(moved.addr), 2));
    assert!(world.hub.table().link(h).is_some(), "hat korundu");

    let datagram = data(h, b"yeni adresten");
    assert_eq!(
        only(&world.deliver(moved.addr, &datagram)),
        &(bob.addr, datagram.clone())
    );
    let reverse = data(h, b"bob'dan");
    assert_eq!(
        only(&world.deliver(bob.addr, &reverse)),
        &(moved.addr, reverse.clone())
    );
    // Eski adres artık bağlı değil: veri yüke bakılmadan düşer (H4).
    assert!(world.deliver(alice.addr, &datagram).is_empty());
    assert_eq!(world.hub.stats().src_unbound, 1);
}

/// T-B6 — epoch tekrarı (halka yok, N bağımsız): A'da bind (kanıt₀), B'den 20 bind, kanıt₀ A'dan
/// tekrar (cookie penceresi içinde) → `BAD_COOKIE`, adres B'de, `bind_epoch = 21`.
#[test]
fn t_b6_an_old_proof_never_moves_the_address_back() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let (_, cn, hn) = world.challenge(&mut alice);
    let rid = alice.rid();
    let proof0 = alice.bind2(rid, &world.hub_pub, cn, hn);
    let out = world.deliver(alice.addr, &proof0);
    world.expect_bound(&only(&out).1, &alice, rid, cn, hn);

    let mut at_b = alice.moved_to([203, 0, 113, 200], 5_000);
    for _ in 0..20 {
        world.bind(&mut at_b);
    }
    world.advance(Duration::from_secs(30));
    let out = world.deliver(alice.addr, &proof0);
    assert_eq!(error_code(&only(&out).1), code::BAD_COOKIE);
    let endpoint = world.hub.table().endpoint(&alice.id).expect("uç");
    assert_eq!((endpoint.addr, endpoint.bind_epoch), (Some(at_b.addr), 21));
}

/// T-B6b — gecikmiş kanıt: A ve B için iki 1. tur (aynı epoch), B'nin kanıtı önce kabul, sonra A'nınki →
/// `BAD_COOKIE`, adres B'de.
#[test]
fn t_b6b_the_later_acceptance_invalidates_the_other_pending_proof() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let mut at_b = alice.moved_to([203, 0, 113, 200], 5_000);
    let (_, cn_a, hn_a) = world.challenge(&mut alice);
    let (_, cn_b, hn_b) = world.challenge(&mut at_b);
    let rid = at_b.rid();
    let out = world.deliver(at_b.addr, &at_b.bind2(rid, &world.hub_pub, cn_b, hn_b));
    world.expect_bound(&only(&out).1, &at_b, rid, cn_b, hn_b);
    let rid = alice.rid();
    let out = world.deliver(alice.addr, &alice.bind2(rid, &world.hub_pub, cn_a, hn_a));
    assert_eq!(error_code(&only(&out).1), code::BAD_COOKIE);
    assert_eq!(world.hub.table().bound_addr(&alice.id), Some(at_b.addr));
}

/// T-B6c — tek bind: 1. turdan 50 s sonra gelen kanıt (pencere içinde, arada kabul yok) → kabul.
#[test]
fn t_b6c_a_delayed_proof_with_no_newer_bind_is_accepted() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let (_, cn, hn) = world.challenge(&mut alice);
    world.advance(Duration::from_secs(50));
    let rid = alice.rid();
    let out = world.deliver(alice.addr, &alice.bind2(rid, &world.hub_pub, cn, hn));
    world.expect_bound(&only(&out).1, &alice, rid, cn, hn);
}

/// T-B7 — kayıp ACK: aynı kanıt aynı adresten tekrar → bayt-özdeş yanıt, `bind_epoch` değişmez,
/// `bind_ack_repeat` +1.
#[test]
fn t_b7_a_repeated_proof_gets_the_identical_answer_and_changes_nothing() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let (_, cn, hn) = world.challenge(&mut alice);
    let rid = alice.rid();
    let proof = alice.bind2(rid, &world.hub_pub, cn, hn);
    let first = world.deliver(alice.addr, &proof);
    world.advance(Duration::from_secs(5));
    let again = world.deliver(alice.addr, &proof);
    assert_eq!(again, first, "bayt-özdeş");
    assert_eq!(world.hub.table().bind_epoch(&alice.id), 1);
    assert_eq!(world.hub.stats().bind_ack_repeat, 1);
    assert_eq!(world.hub.stats().bind_ok, 1);
    // Cookie penceresinin dışında da aynı yanıt (son kabul edilen kanıt hatırlanır).
    world.advance(Duration::from_secs(600));
    assert_eq!(world.deliver(alice.addr, &proof), first);
}

/// T-B7b — kayıp ACK sonrası taşınma: A'da kabul (ACK kayıp), B'de kabul, A-kanıtı A'dan tekrar →
/// `BAD_COOKIE`.
#[test]
fn t_b7b_after_moving_the_lost_ack_proof_is_stale() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let (_, cn, hn) = world.challenge(&mut alice);
    let rid = alice.rid();
    let proof_a = alice.bind2(rid, &world.hub_pub, cn, hn);
    world.deliver(alice.addr, &proof_a); // ACK kaybolur
    let mut at_b = alice.moved_to([203, 0, 113, 200], 5_000);
    world.bind(&mut at_b);
    let out = world.deliver(alice.addr, &proof_a);
    assert_eq!(error_code(&only(&out).1), code::BAD_COOKIE);
    assert_eq!(world.hub.table().bound_addr(&alice.id), Some(at_b.addr));
}

/// T-B8 — kimlik kilitlenemez: C adresinden Alice'in `cert_ref`'iyle 50 geçersiz imza (gerçek
/// cookie'lerle) → C adresi `BIND_FAIL_HOLD` boyunca sessiz; gerçek Alice kendi adresinden bağlanır.
#[test]
fn t_b8_failed_signatures_lock_the_source_address_not_the_identity() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let mut attacker = alice.moved_to([203, 0, 113, 66], 6_000);
    let forged = SigningKey::from_bytes(&[0x66; 32]);
    let mut answered = 0;
    for _ in 0..50 {
        let (_, cn, hn) = world.challenge(&mut attacker);
        let rid = attacker.rid();
        let out = world.deliver(
            attacker.addr,
            &attacker.bind2_signed_by(&forged, rid, &world.hub_pub, cn, hn),
        );
        answered += out.len();
    }
    assert_eq!(answered, 10, "BIND_FAIL_MAX'tan sonra adres sessiz");
    assert_eq!(world.hub.stats().bind_held, 40);
    // Kilitli adres geçerli imzayla da geçemez (süre dolana dek).
    let (_, cn, hn) = world.challenge(&mut attacker);
    let rid = attacker.rid();
    assert!(world
        .deliver(attacker.addr, &attacker.bind2(rid, &world.hub_pub, cn, hn))
        .is_empty());
    // Gerçek Alice kendi adresinden bağlanır.
    world.bind(&mut alice);
    assert_eq!(world.hub.table().bound_addr(&alice.id), Some(alice.addr));
    // Süre dolunca C de yeniden deneyebilir.
    world.advance(Duration::from_secs(60));
    let (_, cn, hn) = world.challenge(&mut attacker);
    let rid = attacker.rid();
    assert_eq!(
        world
            .deliver(attacker.addr, &attacker.bind2(rid, &world.hub_pub, cn, hn))
            .len(),
        1
    );
}

/// T-B8b — adres de kilitlenemez: Alice'in adresini taklit eden 50 yanlış cookie → `bind_fail` girdisi
/// yok, Alice'in sonraki geçerli bind'i kabul.
#[test]
fn t_b8b_wrong_cookies_from_a_spoofed_address_lock_nothing() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    for n in 0..50u8 {
        let rid = alice.rid();
        let out = world.deliver(
            alice.addr,
            &alice.bind2(rid, &world.hub_pub, [n; 16], [n; 16]),
        );
        assert_eq!(error_code(&only(&out).1), code::BAD_COOKIE);
    }
    assert_eq!(world.hub.bind_fail_entries(), 0);
    world.bind(&mut alice);
}

/// T-B9 — geçersiz cookie seli `verify_budget`'ı tüketmez.
#[test]
fn t_b9_bad_cookie_flood_does_not_spend_the_verify_budget() {
    let mut world = World::with_limits(Limits {
        verify_budget: Rate::new(1, 1),
        ..Limits::S4
    });
    let mut alice = Phone::alice();
    for n in 0..1_000u32 {
        let rid = alice.rid();
        let b = n.to_be_bytes();
        let nonce = [b[0], b[1], b[2], b[3], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        world.deliver(alice.addr, &alice.bind2(rid, &world.hub_pub, nonce, nonce));
    }
    assert_eq!(world.hub.stats().verify_calls, 0);
    world.bind(&mut alice); // tek jetonlu bütçe hâlâ dolu
}

/// T-B10 — 1. tur 10 000 kez (dizindeki kimlikle, rastgele adreslerden) → tablo boş, cevap boyu ≤
/// istek. (Bellek ölçümü `tests/dos_memory.rs`.)
#[test]
fn t_b10_first_round_flood_writes_nothing_and_never_amplifies() {
    let mut world = World::new();
    let mut rng = TestRng(0xb10);
    let mut alice = Phone::alice();
    for _ in 0..10_000 {
        let from = rng.addr();
        let rid = alice.rid();
        let mut cn = [0u8; 16];
        rng.fill_bytes(&mut cn);
        let request = alice.bind1(rid, cn);
        let out = world.deliver(from, &request);
        let (to, bytes) = only(&out);
        assert_eq!(*to, from);
        assert!(bytes.len() <= request.len(), "cevap ≤ istek");
    }
    assert_eq!(world.hub.table().endpoint_count(), 0);
    assert_eq!(world.hub.bind_fail_entries(), 0);
}

/// T-B11 — kilit tablosu sınırı: tablodan çok daha fazla farklı erişilebilir adresten geçersiz imza →
/// `bind_fail` ≤ `BIND_FAIL_ENTRIES`, en eskiler atılmış; 60 s sonra boş. (Tablo 4096 yerine 100 girdiyle
/// ve 1 000 adresle sınanır: her adım bir Ed25519 doğrulaması ister, optimizasyonsuz test derlemesinde
/// 10 000 doğrulama dakikalar sürer; sınır mantığı boydan bağımsızdır.)
#[test]
fn t_b11_lock_table_is_bounded_and_expires() {
    let limits = Limits {
        verify_budget: Rate::new(1_000_000, 1_000_000),
        bind_fail_entries: 100,
        ..Limits::S4
    };
    let mut world = World::with_limits(limits);
    let mut rng = TestRng(0xb11);
    let forged = SigningKey::from_bytes(&[0x66; 32]);
    let mut alice = Phone::alice();
    let first = rng.addr();
    let mut addrs = vec![first];
    addrs.extend((1..1_000).map(|_| rng.addr()));
    for addr in &addrs {
        let mut at = alice.moved_to([0, 0, 0, 0], 1);
        at.addr = *addr;
        let (_, cn, hn) = world.challenge(&mut at);
        let rid = at.rid();
        world.deliver(
            at.addr,
            &at.bind2_signed_by(&forged, rid, &world.hub_pub, cn, hn),
        );
        alice.rid();
    }
    assert_eq!(world.hub.bind_fail_entries(), limits.bind_fail_entries);
    world.advance(Duration::from_secs(60));
    world.tick();
    assert_eq!(world.hub.bind_fail_entries(), 0);
}

/// T-B12 — aynı adresten ikinci kimlik: alice X'ten bağlı; bob X'ten geçerli kanıt → `by_addr[X] = bob`,
/// alice adressiz, alice'in hatları ve anahtarları yerinde; carol → alice STORE → `PEER_UNBOUND`; alice'e
/// veri → `drop_peer_unbound`; alice Y'den yeniden bind → her şey devam (S4 v1.3.1 H3).
#[test]
fn t_b12_a_second_identity_takes_the_address_and_the_first_keeps_its_links() {
    let mut world = World::new();
    let (mut alice, mut carol) = (Phone::alice(), Phone::carol());
    world.bind(&mut alice);
    world.bind(&mut carol);
    let (rid, init) = carol.init(&alice, b"INIT");
    let out = world.deliver(carol.addr, &init);
    let h =
        crate::harness::store_response(crate::harness::sent_to(&out, carol.addr)).assigned_handle;
    let _ = rid;

    let mut bob_at_x = Phone::bob().moved_to([198, 51, 100, 7], 40_001);
    world.bind(&mut bob_at_x);
    assert_eq!(world.hub.table().key_at(alice.addr), Some(bob_at_x.id));
    assert_eq!(
        world.hub.table().bound_addr(&alice.id),
        None,
        "alice adressiz"
    );
    assert!(
        world.hub.table().link(h).is_some(),
        "alice'in hattı yerinde"
    );
    assert_eq!(world.hub.stats().bind_displaced, 1);

    // Anahtar da yerinde: aynı INIT tekrarı aynı hattı bulur ama alıcı adressiz → PEER_UNBOUND.
    let out = world.deliver(carol.addr, &init);
    let response = crate::harness::store_response(&only(&out).1);
    assert_eq!(
        (response.status, response.assigned_handle),
        (code::PEER_UNBOUND, 0)
    );
    assert!(world.deliver(carol.addr, &data(h, b"veri")).is_empty());
    assert_eq!(world.hub.stats().drop_peer_unbound, 1);

    let mut alice_at_y = alice.moved_to([100, 64, 9, 9], 31_337);
    world.bind(&mut alice_at_y);
    let datagram = data(h, b"yeniden");
    assert_eq!(
        only(&world.deliver(carol.addr, &datagram)),
        &(alice_at_y.addr, datagram.clone())
    );
    assert_eq!(
        world.hub.table().key_at(bob_at_x.addr),
        Some(bob_at_x.id),
        "bob X'te kalır"
    );
}

/// T-B13 (hub düzeyi) — `::ffff:a.b.c.d` kaynağından gelen 1. tur `a.b.c.d` ile aynı cookie'yi alır.
#[test]
fn t_b13_mapped_and_plain_ipv4_sources_get_the_same_cookie() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    let (_, cn, hn_v4) = {
        let rid = alice.rid();
        let out = world.deliver(alice.addr, &alice.bind1(rid, [9; 16]));
        let r = reply(&only(&out).1);
        (
            rid,
            [9u8; 16],
            hub_server::wire::ClientBindResponse::decode(&r.payload)
                .expect("CHALLENGE")
                .hub_nonce,
        )
    };
    let std::net::IpAddr::V4(v4) = alice.addr.ip() else {
        panic!("v4")
    };
    let mapped =
        std::net::SocketAddr::new(std::net::IpAddr::V6(v4.to_ipv6_mapped()), alice.addr.port());
    let rid = alice.rid();
    let out = world.deliver(mapped, &alice.bind1(rid, cn));
    let r = reply(&only(&out).1);
    let hn_mapped = hub_server::wire::ClientBindResponse::decode(&r.payload)
        .expect("CHALLENGE")
        .hub_nonce;
    assert_eq!(hn_mapped, hn_v4);
}
