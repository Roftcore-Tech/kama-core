//! Dayanıklılık — T-R1 … T-R4, T-L1 (S4 H10, H12). Bellek ölçümü `tests/dos_memory.rs`.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

use hub_server::wire::{classify, envelope, flags, frame, method, Datagram, RpcFrame};

use crate::harness::{
    data, heartbeat, hex, hub_public, rpc, Phone, Sent, TestRng, World, HUB_SEED,
};

/// H10: bir girdinin çıkışı en çok bir cevap (kaynağa) ve en çok bir iletim (o an bağlı bir uca);
/// kayıtsız kaynağa yalnız CLIENT_BIND / BIND_PROOF yanıtı gider.
fn assert_h10(
    world: &World,
    src: SocketAddr,
    src_was_bound: bool,
    request_len: usize,
    out: &[Sent],
) {
    let replies = out.iter().filter(|(to, _)| *to == src).count();
    let forwards: Vec<_> = out.iter().filter(|(to, _)| *to != src).collect();
    assert!(replies <= 1 && forwards.len() <= 1, "{} çıkış", out.len());
    for (to, _) in &forwards {
        assert!(
            world.hub.is_registered_addr(*to),
            "iletim yalnız bağlı uca: {to}"
        );
    }
    if !src_was_bound {
        assert!(forwards.is_empty(), "bağsız kaynak iletim tetikleyemez");
        for (_, bytes) in out {
            let Ok(Datagram::Control {
                frame_type: frame::PLANE1_RPC,
                body,
            }) = classify(bytes)
            else {
                panic!("bağsız kaynağa RPC dışı cevap");
            };
            // S4 v1.3.1 H10: doğrulanmamış kaynağa giden her cevap ≤ istek.
            assert!(
                bytes.len() <= request_len,
                "yansıtma: {} B cevap, {request_len} B istek",
                bytes.len()
            );
            let f = RpcFrame::decode(body).expect("çerçeve");
            assert!(
                f.method_id == method::CLIENT_BIND || f.method_id == method::BIND_PROOF,
                "bağsız kaynağa yalnız bind yanıtı: 0x{:02x}",
                f.method_id
            );
        }
    }
}

/// T-R1 — rastgele adreslerden 10 000 anlamsız paket → 0 cevap.
#[test]
fn t_r1_garbage_from_random_addresses_gets_no_answer() {
    let mut world = World::new();
    let mut rng = TestRng(0x701);
    for _ in 0..10_000 {
        let from = rng.addr();
        let len = rng.below(1400);
        let garbage = rng.bytes(len);
        assert!(world.deliver(from, &garbage).is_empty());
    }
    assert!(world.net.sent.is_empty());
    assert!(world.net.logs.is_empty());
    assert_eq!(world.hub.table().endpoint_count(), 0);
}

/// T-R2 — 1 000 000 rastgele/kesik datagram → panik yok (std ile üretilir).
#[test]
fn t_r2_a_million_random_and_truncated_datagrams_never_panic() {
    let mut world = World::new();
    let mut rng = TestRng(0x702);
    let (mut alice, mut bob, mut carol) = (Phone::alice(), Phone::bob(), Phone::carol());
    world.bind(&mut alice);
    world.bind(&mut bob);
    world.bind(&mut carol);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, b"INIT");
    let rid = alice.rid();
    let seeds: Vec<Vec<u8>> = vec![
        alice.bind1(rid, [1; 16]),
        alice.bind2(rid, &world.hub_pub, [1; 16], [2; 16]),
        alice.store(rid, 0, envelope::SESSION_INIT, &bob, b"INIT INIT"),
        bob.store(rid, h, envelope::SESSION_INIT_RESPONSE, &alice, b"SSR1"),
        alice.store(rid, 0, envelope::RELAY_OPEN, &carol, &[9; 16]),
        heartbeat(&[1, 2, 3, 4]),
        heartbeat(&[]),
        data(h, &[0x77; 100]),
        rpc(method::DELIVER, 0, 0, &[0; 41]),
        rpc(method::CLIENT_BIND, flags::RESPONSE, 1, &[0; 35]),
    ];
    let sources = [alice.addr, bob.addr, carol.addr];
    let mut datagram = Vec::with_capacity(2048);
    for round in 0..1_000_000u32 {
        datagram.clear();
        let seed = &seeds[rng.below(seeds.len())];
        match rng.below(6) {
            0 => {
                datagram.resize(rng.below(1400), 0);
                rng.fill_bytes(&mut datagram);
            }
            1 => {
                datagram.extend_from_slice(&[0, 0, 0, 0, frame::PLANE1_RPC]);
                let n = rng.below(200);
                let start = datagram.len();
                datagram.resize(start + n, 0);
                rng.fill_bytes(&mut datagram[start..]);
                datagram.extend_from_slice(&[0; 16]);
            }
            2 => datagram.extend_from_slice(&seed[..rng.below(seed.len() + 1)]),
            3 => {
                datagram.extend_from_slice(seed);
                for _ in 0..=rng.below(3) {
                    let at = rng.below(datagram.len());
                    datagram[at] ^= 1 << rng.below(8);
                }
            }
            4 => {
                // Gövde bozuk, kuyruk sağlam: çözücünün içine kadar girer.
                datagram.extend_from_slice(seed);
                let tail = datagram.len().saturating_sub(16);
                if tail > 5 {
                    let at = 5 + rng.below(tail - 5);
                    datagram[at] = rng.below(256) as u8;
                }
            }
            _ => {
                datagram.extend_from_slice(seed);
                let extra = rng.below(32);
                let start = datagram.len();
                datagram.resize(start + extra, 0);
                rng.fill_bytes(&mut datagram[start..]);
            }
        }
        let from = if rng.below(4) == 0 {
            rng.addr()
        } else {
            sources[rng.below(sources.len())]
        };
        world.hub.on_datagram(from, &datagram, &mut world.net);
        world.net.sent.clear();
        world.net.logs.clear();
        if round % 1_000 == 0 {
            world.advance(Duration::from_millis(700));
            world.hub.on_tick(&mut world.net);
        }
    }
    world.hub.table().check_invariants().expect("değişmezler");
    assert!(world.hub.stats().drop_malformed > 100_000);
}

/// T-R3 — hub'ın her çıkışı bir girişe cevap ya da bağlı uca iletim (sayaçla); zamanlayıcı datagram
/// yollamaz.
#[test]
fn t_r3_every_output_is_a_reply_or_a_forward_to_a_bound_endpoint() {
    let mut world = World::new();
    let mut rng = TestRng(0x703);
    let mut phones = [
        Phone::alice(),
        Phone::bob(),
        Phone::carol(),
        Phone::mallory(),
    ];
    for phone in phones.iter_mut().take(3) {
        world.bind(phone);
    }
    let h = {
        let bob = phones[1].clone();
        world.open(&mut phones[0], &bob, envelope::SESSION_INIT, b"INIT")
    };
    for step in 0..20_000 {
        let i = rng.below(phones.len());
        let peer = phones[rng.below(phones.len())].clone();
        let from = if rng.below(10) == 0 {
            rng.addr()
        } else {
            phones[i].addr
        };
        let rid = phones[i].rid();
        let p = &phones[i];
        let datagram = match rng.below(8) {
            0 => p.bind1(rid, [rng.below(256) as u8; 16]),
            1 => p.bind2(rid, &world.hub_pub, [1; 16], [2; 16]),
            2 => heartbeat(&[1, 2, 3, 4]),
            3 => p.store(rid, 0, envelope::SESSION_INIT, &peer, &rng.bytes(24)),
            4 => p.store(rid, h, envelope::SESSION_INIT_RESPONSE, &peer, b"SSR1"),
            5 => data(h, &rng.bytes(64)),
            6 => data(rng.below(1 << 20) as u32 + 1, b"veri"),
            _ => rng.bytes(40),
        };
        let was_bound = world.hub.is_registered_addr(from);
        let out = world.deliver(from, &datagram);
        assert_h10(&world, from, was_bound, datagram.len(), &out);
        if step % 50 == 0 {
            world.advance(Duration::from_secs(1));
            world.tick();
        }
    }
    let stats = *world.hub.stats();
    assert_eq!(
        stats.tx_reply + stats.tx_forward,
        world.net.sent.len() as u64,
        "her gönderim sayıldı"
    );
    assert!(
        stats.tx_forward > 0 && stats.relay_packets > 0 && stats.store_ok > 0,
        "{stats:?}"
    );
}

/// T-R4 — `flags ≠ 0` ya da bit7'li method_id ile gelen istek → `drop_malformed`; bilinmeyen method ve
/// bilinmeyen çerçeve tipi ayrı sayılır.
#[test]
fn t_r4_responses_and_hub_methods_from_phones_are_malformed() {
    let mut world = World::new();
    let mut alice = Phone::alice();
    world.bind(&mut alice);
    let payload = Phone::alice().bind1(1, [0; 16]);
    let body = &payload[10..payload.len() - 16];
    for (method_id, flag) in [
        (method::CLIENT_BIND, flags::RESPONSE),
        (method::CLIENT_BIND, flags::ERROR),
        (method::DELIVER, 0),
        (method::ROUTE_MISS, 0),
        (0x85, 0),
    ] {
        assert!(
            world
                .deliver(alice.addr, &rpc(method_id, flag, 1, body))
                .is_empty(),
            "{method_id:02x}/{flag}"
        );
    }
    assert_eq!(world.hub.stats().drop_malformed, 5);
    assert!(world.deliver(alice.addr, &rpc(0x42, 0, 1, b"")).is_empty());
    assert_eq!(world.hub.stats().drop_unknown_method, 1);
    let mut unknown_frame = heartbeat(&[]);
    unknown_frame[4] = frame::HEARTBEAT_ACK;
    assert!(world.deliver(alice.addr, &unknown_frame).is_empty());
    assert_eq!(world.hub.stats().drop_unknown_frame, 1);
}

/// T-L1 — loglarda yük, zarf, anahtar ve tam kimlik yok.
#[test]
fn t_l1_logs_never_contain_payload_envelopes_keys_or_full_identities() {
    let mut world = World::with_limits(hub_server::config::Limits {
        max_links_per_endpoint: 1,
        ..hub_server::config::Limits::S4
    });
    let (mut alice, mut bob) = (Phone::alice(), Phone::bob());
    let envelope_bytes: Vec<u8> = (0..120u8)
        .map(|n| n.wrapping_mul(91).wrapping_add(5))
        .collect();
    let payload: Vec<u8> = (0..200u8)
        .map(|n| n.wrapping_mul(37).wrapping_add(11))
        .collect();
    world.bind(&mut alice);
    world.bind(&mut bob);
    let h = world.open(&mut alice, &bob, envelope::SESSION_INIT, &envelope_bytes);
    world.deliver(alice.addr, &data(h, &payload));
    world.deliver(bob.addr, &data(h, b"GIZLI-MESAJ-METNI"));
    world.advance(Duration::from_secs(2));
    world.open(
        &mut alice,
        &bob,
        envelope::RELAY_OPEN,
        &envelope_bytes[..16],
    ); // emeklilik → link retire
    for _ in 0..400 {
        world.advance(Duration::from_secs(10));
        world.tick();
    }
    let logs = world.net.logs.join("\n");
    let kinds: HashSet<String> = world
        .net
        .logs
        .iter()
        .map(|l| l.split(' ').take(2).collect::<Vec<_>>().join(" "))
        .collect();
    for kind in &kinds {
        assert!(
            [
                "bind ok",
                "link open",
                "link retire",
                "cookie rotate",
                "hub stats"
            ]
            .contains(&kind.as_str()),
            "belgelenmemiş satır: {kind}"
        );
    }
    assert_eq!(kinds.len(), 5, "beş tür de üretildi: {kinds:?}");
    for secret in [&payload[..], &envelope_bytes[..], b"GIZLI-MESAJ-METNI"] {
        for window in secret.windows(4) {
            assert!(!logs.contains(&hex(window)), "yük/zarf baytı logda");
        }
    }
    assert!(!logs.contains("GIZLI"));
    for key in [HUB_SEED, hub_public(), alice.key.verifying_key().to_bytes()] {
        for window in key.windows(8) {
            assert!(!logs.contains(&hex(window)), "anahtar logda");
        }
    }
    for id in [alice.id, bob.id] {
        assert!(logs.contains(&hex(&id[..4])));
        assert!(
            !logs.contains(&hex(&id[..5])) && !logs.contains(&hex(&id[1..6])),
            "kimliğin 4 bayttan fazlası"
        );
    }
    for cert_ref in [alice.cert_ref, bob.cert_ref] {
        assert!(!logs.contains(&hex(&cert_ref[..4])), "cert_ref logda");
    }
}
