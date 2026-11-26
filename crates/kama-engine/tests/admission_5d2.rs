//! Sprint-5d-2 — decoder-admission köprüsü (INV-FEC-1 production-kapanışı) uçtan-uca.
//! authenticated → decode · forge open()-atlayamaz (type-gate) · mixed → yalnız-authenticated · profil-gate fail-closed.

mod common;

use kama_engine::admit_and_decode;
use kama_engine::EngineError;
use kama_fec_codec::{by_id, FecError};
use kama_noise_wire::{KamaNoiseWireProtector, OpenedTransportShard, SealArgs, WireError};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};

fn profile3() -> kama_fec_codec::FecProfile {
    by_id(0x03).unwrap()
}

/// `encode_seal_block`'un tam-yüklü, FINAL'siz source'unu (aynı canonical sembol) yeni PN ile
/// yeniden mühürler: aynı mantıksal source'un yeniden iletimi (K-RETX-1: yeni PN, aynı kimlik).
fn reseal_source(
    tx: &mut KamaNoiseWireProtector,
    block_id: u32,
    esi: u16,
    k_actual: u8,
    message_id: u32,
    wire_pn: u64,
) -> Vec<u8> {
    let pkt = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id,
            esi,
            k_actual,
        },
        symbol: common::source_symbol(esi, message_id),
    };
    tx.seal(
        &pkt,
        SealArgs {
            wire_pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

// ── authenticated shard'lar → decode → app-bytes EŞİT (production köprü) ──
#[test]
fn authenticated_shards_decode_to_app_bytes() {
    let (mut tx, mut rx) = common::pair(0x0600_0001);
    let p = profile3();
    let app = vec![0x33u8; common::PAYLOAD_CAP * 5]; // k_actual=5
    let (sealed, k_actual) = common::encode_seal_block(&mut tx, &p, &app, 1, 0xABCD, 0);
    assert_eq!(k_actual, 5);

    // TYPE-GATE: yalnız open() OpenedTransportShard üretir → admit_and_decode'a YALNIZ bunlar girer.
    let opened: Vec<OpenedTransportShard> = sealed
        .iter()
        .map(|s| rx.open(&s.datagram).expect("authenticated"))
        .collect();
    let out = admit_and_decode(opened, 3, k_actual).unwrap();
    assert_eq!(out, app, "authenticated shard'lar app-bytes'a decode oldu");
}

// ── source-kayıp + repair → decode RECOVERED (production köprüde de kurtarır) ──
#[test]
fn source_loss_recovers_via_bridge() {
    let (mut tx, mut rx) = common::pair(0x0600_0002);
    let p = profile3();
    let app = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let (sealed, k_actual) = common::encode_seal_block(&mut tx, &p, &app, 1, 0x1234, 0);

    // 2 source düş (esi 1,3); repair'ler geçer.
    let kept: Vec<OpenedTransportShard> = sealed
        .iter()
        .filter(|s| s.is_repair || !(s.esi == 1 || s.esi == 3))
        .map(|s| rx.open(&s.datagram).expect("authenticated"))
        .collect();
    let out = admit_and_decode(kept, 3, k_actual).unwrap();
    assert_eq!(
        out, app,
        "2-source-kayıp repair'den kurtarıldı (production köprü)"
    );
}

// ── INV-FEC-1 TYPE-GATE: forge open()-atlayamaz → OpenedTransportShard üretemez → köprüye ULAŞAMAZ ──
#[test]
fn forged_datagram_cannot_open_never_reaches_decode() {
    let (mut tx, mut rx) = common::pair(0x0600_0003);
    // Meşru bir datagram al, sonra ct-byte'ını boz (tag-verify düşecek).
    let mut dg = common::seal_source(&mut tx, 1, 0, 3, 0x7777, 0);
    let ct_pos = 12; // outer-header sonrası bir ct-byte'ı boz
    dg[ct_pos] ^= 0xFF;
    // open() AEAD-fail → OpenedTransportShard ÜRETİLMEZ (forge decode-yoluna giremez).
    assert!(
        matches!(rx.open(&dg), Err(WireError::AeadFail)),
        "forge → AeadFail (OpenedShard üretilmez)"
    );
    // Tip-seviyesi: admit_and_decode'un girdisi Vec<OpenedTransportShard>; forge böyle bir değer
    // ÜRETEMEZ (open()-dışı constructor yok — opened.rs mühürlü + trybuild compile-fail). Yani forge
    // bu fonksiyona derleme/çalışma-zamanı hiçbir yolla ULAŞAMAZ (INV-FEC-1).
}

// ── mixed authenticated+forged → yalnız authenticated'lar open olur → onlarla decode başarılı ──
#[test]
fn mixed_authenticated_and_forged_only_authenticated_decode() {
    let (mut tx, mut rx) = common::pair(0x0600_0004);
    let p = profile3();
    let app = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let (sealed, k_actual) = common::encode_seal_block(&mut tx, &p, &app, 1, 0x9999, 0);

    // Datagram-akışına bir forge enjekte et (bozuk); receiver her birini open-dener.
    let mut forged = sealed[0].datagram.clone();
    forged[13] ^= 0xAA;

    let mut opened: Vec<OpenedTransportShard> = Vec::new();
    let mut forge_rejected = 0;
    // Forge'u araya koy; sonra tüm meşru-shard'lar.
    for dg in std::iter::once(&forged).chain(sealed.iter().map(|s| &s.datagram)) {
        match rx.open(dg) {
            Ok(o) => opened.push(o),
            Err(_) => forge_rejected += 1, // forge (ve varsa çakışan-PN) reddedilir
        }
    }
    assert!(
        forge_rejected >= 1,
        "forge reddedildi (decode-yoluna girmedi)"
    );
    // Yalnız authenticated'larla decode → app-bytes EŞİT (forge muhasebeye girmedi).
    let out = admit_and_decode(opened, 3, k_actual).unwrap();
    assert_eq!(out, app, "yalnız-authenticated shard'lar app'e decode oldu");
}

// ── PROFİL-GATE: shard.profile_id ≠ bound → fail-closed (PARAM-bağı) ──
#[test]
fn profile_gate_fail_closed() {
    let (mut tx, mut rx) = common::pair(0x0600_0005);
    // profile_id=5 ile seal edilmiş authenticated-shard, ama bound-profil=3.
    let dg = common::seal_source(&mut tx, 1, 0, 5, 0x5555, 0);
    let opened = rx
        .open(&dg)
        .expect("authenticated (AEAD profile_id'yi denetlemez)");
    let r = admit_and_decode(vec![opened], 3, 1);
    assert_eq!(
        r,
        Err(EngineError::ProfileMismatch { shard: 5, bound: 3 }),
        "yanlış-profil-shard fail-closed (bound=3 ≠ shard=5)"
    );
}

// ── unknown bound-profil → fail-closed (by_id miss) ──
#[test]
fn unknown_bound_profile_fail_closed() {
    let (mut tx, mut rx) = common::pair(0x0600_0006);
    let dg = common::seal_source(&mut tx, 1, 0, 200, 0x2222, 0);
    let opened = rx.open(&dg).expect("authenticated");
    let r = admit_and_decode(vec![opened], 200, 1);
    assert_eq!(
        r,
        Err(EngineError::UnknownProfile(200)),
        "registry-dışı profil → fail-closed"
    );
}

// ── BLOK-GATE (S-53/M2-07; fec-core §6.5 MADDE-9 block_id decoder-context'ten): ESI'leri çakışmayan
//    iki bloğun shard'ları tek çağrıda → fail-closed (eskiden tek blok gibi çözülüp birleşiyordu) ──
#[test]
fn second_block_in_one_call_fail_closed() {
    let (mut tx, mut rx) = common::pair(0x0600_0007);
    let p = profile3();
    let app_a = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let app_b = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let (sealed_a, k_actual) = common::encode_seal_block(&mut tx, &p, &app_a, 1, 0xA001, 0);
    let (sealed_b, _) = common::encode_seal_block(&mut tx, &p, &app_b, 2, 0xB002, 100);

    // Blok 1'in source 0..2'si + blok 2'nin source 3..4'ü: ESI'ler çakışmaz, K_actual tutar.
    let mut opened: Vec<OpenedTransportShard> = Vec::new();
    for s in sealed_a.iter().filter(|s| !s.is_repair && s.esi < 3) {
        opened.push(rx.open(&s.datagram).expect("authenticated"));
    }
    for s in sealed_b.iter().filter(|s| !s.is_repair && s.esi >= 3) {
        opened.push(rx.open(&s.datagram).expect("authenticated"));
    }
    assert_eq!(
        admit_and_decode(opened, 3, k_actual),
        Err(EngineError::BlockIdMismatch { shard: 2, block: 1 }),
        "ikinci bloğun shard'ı bu bloğun decode'una giremez"
    );
}

// ── K_ACTUAL-GATE (S-53/M2-07; fec-core §6 PATCH-4): inner-header K_actual ≠ blok durumu → fail-closed ──
#[test]
fn inner_k_actual_mismatch_fail_closed() {
    let (mut tx, mut rx) = common::pair(0x0600_0008);
    // seal_source inner-header k_actual=1 taşır; blok durumu (çağıran) K_actual=2 diyor.
    let dg = common::seal_source(&mut tx, 1, 0, 3, 0x4444, 0);
    let opened = rx.open(&dg).expect("authenticated");
    assert_eq!(
        admit_and_decode(vec![opened], 3, 2),
        Err(EngineError::KActualMismatch { shard: 1, block: 2 }),
        "inner K_actual blok durumuyla uyuşmazsa packet-REJECT (rank artmaz)"
    );
}

// ── YİNELENEN ESI (S-53/M2-09; fec-core §13 duplicate-ESI-rejection, §6 decode iff benzersiz
//    admitted ≥ K_actual): aynı source'un yeniden iletimi çağrıyı düşürmez (eskiden DuplicateEsi) ──
#[test]
fn retransmitted_source_is_not_admitted_twice() {
    let (mut tx, mut rx) = common::pair(0x0600_0009);
    let p = profile3();
    let app = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let (sealed, k_actual) = common::encode_seal_block(&mut tx, &p, &app, 1, 0x6161, 0);
    let retx = reseal_source(&mut tx, 1, 0, k_actual, 0x6161, 50);

    let mut opened: Vec<OpenedTransportShard> = sealed
        .iter()
        .filter(|s| !s.is_repair)
        .map(|s| rx.open(&s.datagram).expect("authenticated"))
        .collect();
    opened.push(
        rx.open(&retx)
            .expect("yeni PN: replay penceresi kabul eder"),
    );
    assert_eq!(
        admit_and_decode(opened, 3, k_actual),
        Ok(app),
        "yinelenen source rank'a girmeden bırakılır; benzersiz 5 source bloğu çözer"
    );
}

// ── Yinelenen shard rank'ı ARTIRMAZ: 4 benzersiz source + 1 kopya, K_actual=5 → yetersiz ──
#[test]
fn duplicate_does_not_raise_rank() {
    let (mut tx, mut rx) = common::pair(0x0600_000A);
    let p = profile3();
    let app = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let (sealed, k_actual) = common::encode_seal_block(&mut tx, &p, &app, 1, 0x6262, 0);
    let retx = reseal_source(&mut tx, 1, 0, k_actual, 0x6262, 50);

    let mut opened: Vec<OpenedTransportShard> = sealed
        .iter()
        .filter(|s| !s.is_repair && s.esi != 4)
        .map(|s| rx.open(&s.datagram).expect("authenticated"))
        .collect();
    opened.push(rx.open(&retx).expect("authenticated"));
    assert_eq!(
        admit_and_decode(opened, 3, k_actual),
        Err(EngineError::FecDecode(FecError::Insufficient {
            admitted: 4,
            known_zero: 3,
            k_shape: 8,
        })),
        "kopya benzersiz sayıyı (4) artırmaz"
    );
}
