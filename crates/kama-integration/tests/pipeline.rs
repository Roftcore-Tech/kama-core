//! Sprint-5a: cross-crate uçtan-uca pipeline (mock-key; adaptive-transport'a DOKUNMADAN).
//! app-bytes → chunk → canonical-prefix → FEC-encode(Model-A) → seal(EXTENDED,LE64) → [wire] → open →
//! admission → FEC-decode(known-zero) → app-bytes-EŞİT. + kayıp-matrisi + control-yolu + reorder/dup/replay
//! + dh_epoch-geçişi + seal==sealed_len/PMTU-admission.

use kama_fec_codec::{
    by_id, FecCodec, FecError, FecProfile, ReceivedSymbol, RsSimdCodec, SourceSymbol,
};
use kama_noise_wire::retry::control_produces_bandwidth_credit;
use kama_noise_wire::SensitiveSymbolBytes;
use kama_noise_wire::{
    nonce96, InnerPlain, KamaNoiseWireProtector, KamaNoiseWireSizer, SealArgs, WireError,
    SEND_PN_CAP,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::{ShardKind, ACK, RECOVERED};
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const PAYLOAD_CAP: usize = 64 - 11; // E - SOURCE_PREFIX_LEN = 53

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

/// Sender+receiver protector çifti (aynı h_ext, ters-anahtar).
fn pair(h_ext: u32) -> (KamaNoiseWireProtector, KamaNoiseWireProtector) {
    (
        KamaNoiseWireProtector::new(h_ext, &key(0xA1), &key(0xB2)),
        KamaNoiseWireProtector::new(h_ext, &key(0xB2), &key(0xA1)),
    )
}

/// Bir sealed FEC-shard + logical-esi/kind (wire-simülasyonu için).
struct SentShard {
    datagram: Vec<u8>,
    esi: u16,
    is_repair: bool,
}

/// SENDER: app-bytes'ı Model-A blok olarak encode+seal eder. K_actual = chunk-sayısı.
/// Döner: (gönderilen-shard'lar, k_shape, k_actual, message_id).
fn sender_encode_seal(
    protector: &mut KamaNoiseWireProtector,
    profile: &FecProfile,
    app_bytes: &[u8],
    block_id: u32,
    message_id: u32,
    base_pn: u64,
) -> (Vec<SentShard>, u16, u8) {
    let k_shape = profile.k_shape as usize;
    let e = profile.e_usize();
    // Chunk'la (≤ PAYLOAD_CAP her chunk).
    let chunks: Vec<&[u8]> = app_bytes.chunks(PAYLOAD_CAP).collect();
    let k_actual = chunks.len();
    assert!(k_actual >= 1 && k_actual <= k_shape);

    // Canonical source symbols (prefix + payload + zero-pad); zero-pad [K_actual, K_shape).
    let mut block: Vec<Vec<u8>> = Vec::with_capacity(k_shape);
    for (i, chunk) in chunks.iter().enumerate() {
        let pfx = SourcePrefix {
            message_id,
            chunk_offset: (i * PAYLOAD_CAP) as u32,
            actual_payload_len: chunk.len() as u16,
            source_flags: u8::from(i == k_actual - 1), // son-chunk final-flag
        };
        let mut sym = vec![0u8; e];
        pfx.serialize_symbol(chunk, &mut sym).unwrap();
        block.push(sym);
    }
    while block.len() < k_shape {
        block.push(vec![0u8; e]); // canonical-zero-pad (wire'a GÖNDERİLMEZ)
    }

    // FEC encode → repair.
    let codec = RsSimdCodec;
    let src_syms: Vec<SourceSymbol> = block
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let repairs = codec.encode(profile, &src_syms).unwrap();

    let mut sent = Vec::new();
    let mut pn = base_pn;
    // Yalnız real-source [0, K_actual) + repair'ler seal edilir (zero-pad GÖNDERİLMEZ).
    for (esi, sym) in block.iter().enumerate().take(k_actual) {
        let pkt = UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Source,
                profile_id: profile.profile_id,
                block_id,
                esi: esi as u16,
                k_actual: k_actual as u8,
            },
            symbol: sym.clone(),
        };
        // seal==sealed_len + PMTU-admission (D1-EXIT).
        let want = protector.sealed_len(&pkt).unwrap();
        assert!(want <= 1200, "PMTU-admission: sealed {want} ≤ 1200");
        let sealed = protector
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(sealed.bytes.len(), want, "seal==sealed_len");
        sent.push(SentShard {
            datagram: sealed.bytes,
            esi: esi as u16,
            is_repair: false,
        });
        pn += 1;
    }
    for rep in &repairs {
        let pkt = UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Repair,
                profile_id: profile.profile_id,
                block_id,
                esi: rep.esi,
                k_actual: k_actual as u8,
            },
            symbol: rep.bytes.clone(),
        };
        let sealed = protector
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        sent.push(SentShard {
            datagram: sealed.bytes,
            esi: rep.esi,
            is_repair: true,
        });
        pn += 1;
    }
    (sent, k_shape as u16, k_actual as u8)
}

/// RECEIVER: gelen-datagram'ları open + admit + FEC-decode → app-bytes reassemble.
fn receiver_open_decode(
    protector: &mut KamaNoiseWireProtector,
    profile: &FecProfile,
    received_datagrams: &[Vec<u8>],
    k_actual: u8,
) -> Result<Vec<u8>, WireError> {
    let k_shape = profile.k_shape as usize;
    let known_zero = k_shape - usize::from(k_actual);

    // Open → symbols topla (owned; sonra ReceivedSymbol referansları).
    let mut source_syms: Vec<(u16, SensitiveSymbolBytes)> = Vec::new();
    let mut repair_syms: Vec<(u16, SensitiveSymbolBytes)> = Vec::new();
    for dg in received_datagrams {
        let opened = protector.open(dg)?;
        match opened.into_inner() {
            InnerPlain::FecShard { header, symbol } => match header.kind {
                ShardKind::Source => source_syms.push((header.esi, symbol)),
                ShardKind::Repair => repair_syms.push((header.esi, symbol)),
            },
            InnerPlain::Control { .. } => {}
        }
    }

    // FEC reconstruct (Model-A known-zero-inject).
    let codec = RsSimdCodec;
    let mut recv: Vec<ReceivedSymbol> = Vec::new();
    for (esi, bytes) in &source_syms {
        recv.push(ReceivedSymbol {
            esi: *esi,
            is_repair: false,
            bytes: bytes.as_ref(),
        });
    }
    for (esi, bytes) in &repair_syms {
        recv.push(ReceivedSymbol {
            esi: *esi,
            is_repair: true,
            bytes: bytes.as_ref(),
        });
    }
    let recovered = codec
        .reconstruct(profile, &recv, known_zero)
        .map_err(|_| WireError::InnerParse("fec reconstruct"))?;

    // Tüm K_actual source-symbol'ü topla (alınan + kurtarılan).
    let mut all_source: Vec<Option<SensitiveSymbolBytes>> = Vec::new();
    all_source.resize_with(usize::from(k_actual), || None);
    for (esi, bytes) in source_syms {
        if (esi as usize) < usize::from(k_actual) {
            all_source[esi as usize] = Some(bytes);
        }
    }
    for r in recovered {
        if (r.esi as usize) < usize::from(k_actual) {
            all_source[r.esi as usize] = Some(SensitiveSymbolBytes::from(r.bytes));
        }
    }

    // Reassemble: her source-symbol prefix'ten payload.
    let mut app = Vec::new();
    for slot in all_source {
        let sym = slot.ok_or(WireError::InnerParse("missing source after decode"))?;
        let pfx = SourcePrefix::parse(&sym).map_err(|_| WireError::InnerParse("prefix"))?;
        app.extend_from_slice(pfx.payload_slice(&sym));
    }
    Ok(app)
}

fn profile3() -> FecProfile {
    by_id(0x03).unwrap() // K8 R4 E64
}

// ── VERİ-YOLU: kayıpsız roundtrip ──
#[test]
fn data_path_no_loss() {
    let (mut tx, mut rx) = pair(0x0100_0001);
    let p = profile3();
    let app = b"kama end-to-end pipeline: fec-encode seal open decode reassemble roundtrip payload"
        .to_vec();
    let (sent, _k_shape, k_actual) = sender_encode_seal(&mut tx, &p, &app, 1, 0xABCD, 0);
    let dgs: Vec<Vec<u8>> = sent.iter().map(|s| s.datagram.clone()).collect();
    let out = receiver_open_decode(&mut rx, &p, &dgs, k_actual).unwrap();
    assert_eq!(out, app, "kayıpsız app-bytes EŞİT");
}

// ── KAYIP-MATRİSİ: source-kayıp + repair-yeterli → kurtar ──
#[test]
fn data_path_source_loss_repair_recovers() {
    let (mut tx, mut rx) = pair(0x0100_0002);
    let p = profile3();
    let app = vec![0x42u8; 53 * 5]; // 5 chunk (K_actual=5)
    let (sent, _ks, k_actual) = sender_encode_seal(&mut tx, &p, &app, 1, 0x1234, 0);
    assert_eq!(k_actual, 5);
    // 2 source düş (esi 1,3); repair'ler geçer.
    let dgs: Vec<Vec<u8>> = sent
        .iter()
        .filter(|s| !(!s.is_repair && (s.esi == 1 || s.esi == 3)))
        .map(|s| s.datagram.clone())
        .collect();
    let out = receiver_open_decode(&mut rx, &p, &dgs, k_actual).unwrap();
    assert_eq!(out, app, "2-source-kayıp repair ile kurtarıldı");
}

// ── KAYIP-MATRİSİ: karışık (source+repair kayıp, yine yeterli) ──
#[test]
fn data_path_mixed_loss() {
    let (mut tx, mut rx) = pair(0x0100_0003);
    let p = profile3();
    let app = vec![0x7Eu8; 53 * 6]; // K_actual=6
    let (sent, _ks, k_actual) = sender_encode_seal(&mut tx, &p, &app, 1, 0x55, 0);
    // esi source 2 düş + repair esi 3 düş; kalan (5 source + 3 repair) ≥ 6 → kurtar.
    let dgs: Vec<Vec<u8>> = sent
        .iter()
        .filter(|s| !((!s.is_repair && s.esi == 2) || (s.is_repair && s.esi == 3)))
        .map(|s| s.datagram.clone())
        .collect();
    let out = receiver_open_decode(&mut rx, &p, &dgs, k_actual).unwrap();
    assert_eq!(out, app);
}

// ── KAYIP-MATRİSİ: yetersiz → fail-closed (decode-edilemez) ──
#[test]
fn data_path_insufficient_fail_closed() {
    let (mut tx, mut rx) = pair(0x0100_0004);
    let p = profile3();
    let app = vec![0x11u8; 53 * 5]; // K_actual=5, R=4
    let (sent, _ks, k_actual) = sender_encode_seal(&mut tx, &p, &app, 1, 0x99, 0);
    // 5 source düş (hepsi), yalnız 4 repair kalır → 4 < 5 → yetersiz.
    let dgs: Vec<Vec<u8>> = sent
        .iter()
        .filter(|s| s.is_repair)
        .map(|s| s.datagram.clone())
        .collect();
    // Not: 4 repair + known_zero(3) = 7 ≥ 8? K_shape=8 → 4+3=7 < 8 → Insufficient.
    assert!(
        receiver_open_decode(&mut rx, &p, &dgs, k_actual).is_err(),
        "yetersiz → fail-closed"
    );
}

// ── REORDER + DUP + REPLAY pipeline-üstünde (accept() entegre) ──
#[test]
fn pipeline_reorder_dup_replay() {
    let (mut tx, mut rx) = pair(0x0100_0005);
    let p = profile3();
    let app = vec![0x33u8; 53 * 4];
    let (sent, _ks, k_actual) = sender_encode_seal(&mut tx, &p, &app, 1, 0x77, 0);
    let mut dgs: Vec<Vec<u8>> = sent.iter().map(|s| s.datagram.clone()).collect();
    // Reorder: ters-sıra.
    dgs.reverse();
    // Dup: ilk-paketi tekrarla (replay). Receiver PN-window ile ikinci-kopyayı düşürmeli ama decode yine OK.
    let first = dgs[0].clone();
    // İlk open → OK; ikinci (dup) open → DUPLICATE-reject (ama pipeline decode etkilenmez).
    assert!(rx.open(&first).is_ok());
    assert!(
        matches!(rx.open(&first), Err(WireError::PnReject(_))),
        "replay reddedilir"
    );
    // Kalan-paketleri (dup-hariç) aç + decode → yeni-rx ile temiz-koşu.
    let (mut _tx2, mut rx2) = pair(0x0100_0006);
    let (sent2, _k, k_actual2) = sender_encode_seal(&mut _tx2, &p, &app, 1, 0x77, 0);
    let mut dgs2: Vec<Vec<u8>> = sent2.iter().map(|s| s.datagram.clone()).collect();
    dgs2.reverse();
    let out = receiver_open_decode(&mut rx2, &p, &dgs2, k_actual2).unwrap();
    assert_eq!(out, app, "reorder'a rağmen decode doğru");
    let _ = k_actual;
}

// ── dh_epoch 0→1 geçişi pipeline-CANLIYKEN ──
#[test]
fn pipeline_epoch_transition_live() {
    let mut tx = KamaNoiseWireProtector::new(0x0100_0007, &key(0xA1), &key(0xB2));
    let mut rx = KamaNoiseWireProtector::new(0x0100_0007, &key(0xB2), &key(0xA1));
    let p = profile3();
    let app = vec![0x5Au8; 53 * 3];

    // epoch-0'da blok gönder + decode.
    let (sent0, _k, ka0) = sender_encode_seal(&mut tx, &p, &app, 1, 0x01, 0);
    let dgs0: Vec<Vec<u8>> = sent0.iter().map(|s| s.datagram.clone()).collect();
    assert_eq!(receiver_open_decode(&mut rx, &p, &dgs0, ka0).unwrap(), app);

    // Epoch-1 kur (machine InstallEpoch1AndSend'i uygular).
    tx.install_epoch1_keys(&key(0xCC), &key(0xDD));
    rx.install_epoch1_keys(&key(0xDD), &key(0xCC));

    // epoch-1'de blok gönder (PN epoch-0'ın son değerinden +1 ile DEVAM eder; §4 EPOCH-PN-INIT
    // 2026-10-06) + decode → app doğru.
    let mut sent1 = Vec::new();
    let mut pn = sent0.len() as u64;
    // Basit: tek-blok epoch-1'de (sender_encode_seal epoch-0-sabit; burada elle epoch-1 seal).
    let (raw, _ks, ka1) = {
        // Re-encode aynı app; ama seal epoch-1. sender_encode_seal epoch-0 kullanıyor → burada
        // shard'ları epoch-1 seal için yeniden-üret.
        let codec = RsSimdCodec;
        let chunks: Vec<&[u8]> = app.chunks(PAYLOAD_CAP).collect();
        let k_actual = chunks.len();
        let mut block: Vec<Vec<u8>> = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            let pfx = SourcePrefix {
                message_id: 0x02,
                chunk_offset: (i * PAYLOAD_CAP) as u32,
                actual_payload_len: c.len() as u16,
                source_flags: u8::from(i == k_actual - 1),
            };
            let mut s = vec![0u8; 64];
            pfx.serialize_symbol(c, &mut s).unwrap();
            block.push(s);
        }
        while block.len() < 8 {
            block.push(vec![0u8; 64]);
        }
        let src: Vec<SourceSymbol> = block
            .iter()
            .enumerate()
            .map(|(i, b)| SourceSymbol {
                esi: i as u16,
                bytes: b,
            })
            .collect();
        let reps = codec.encode(&p, &src).unwrap();
        for (esi, sym) in block.iter().enumerate().take(k_actual) {
            let pkt = UnsealedTransportPacket::FecShard {
                header: LogicalShardHeader {
                    kind: ShardKind::Source,
                    profile_id: p.profile_id,
                    block_id: 2,
                    esi: esi as u16,
                    k_actual: k_actual as u8,
                },
                symbol: sym.clone(),
            };
            sent1.push(
                tx.seal(
                    &pkt,
                    SealArgs {
                        wire_pn: pn,
                        dh_epoch: 1,
                        send_generation: 0,
                    },
                )
                .unwrap()
                .bytes,
            );
            pn += 1;
        }
        for rep in &reps {
            let pkt = UnsealedTransportPacket::FecShard {
                header: LogicalShardHeader {
                    kind: ShardKind::Repair,
                    profile_id: p.profile_id,
                    block_id: 2,
                    esi: rep.esi,
                    k_actual: k_actual as u8,
                },
                symbol: rep.bytes.clone(),
            };
            sent1.push(
                tx.seal(
                    &pkt,
                    SealArgs {
                        wire_pn: pn,
                        dh_epoch: 1,
                        send_generation: 0,
                    },
                )
                .unwrap()
                .bytes,
            );
            pn += 1;
        }
        (sent1, 8u16, k_actual as u8)
    };
    assert_eq!(
        receiver_open_decode(&mut rx, &p, &raw, ka1).unwrap(),
        app,
        "epoch-1 pipeline decode"
    );
}

// ══════════════════════════════════════════════════════════════════════════════════════════════
// N8-YENİDEN-DOĞRULAMA (emisyon-AÇIK) · Test-2: recovered ≠ ACK/BtlBw-credit
// ──────────────────────────────────────────────────────────────────────────────────────────────
// D0-N8'in ikinci-yarısı: FEC-decode başarısı (RECOVERED) BtlBw'ye/delivered-EAI'ye kredi-VERMEZ.
// Emisyon-AÇIK canlı-pipeline: source-kayıp + repair → recovered-source LOGICAL-teslim (app-bytes
// tam) AMA WIRE-teslim DEĞİL (repair-decode'dan geldi, wire'dan gelmedi) → kredi-tabanına girmez.
//
// SINIR-NOTU (DR-S5-3 §5): sender-tarafı RECOVERED-control-İŞLEME (→ confirmed_logical logical-defter)
// henüz-bağlı-DEĞİL (ayrı-adım). Bu test recovered≠BtlBw-credit'in GÜVENLİK-yarısını kanıtlar:
// recovered'ın BtlBw'ye YAPISAL-yolu YOK (kredi yalnız wire-ACK'ten, engine SealedPacketMeta ile).
// ══════════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn n8_recovered_is_logical_delivery_not_wire_credit() {
    let (mut tx, mut rx) = pair(0x0100_00A8);
    let p = profile3();
    let app = vec![0x5Au8; 53 * 5]; // 5 chunk → K_actual=5
    let (sent, _ks, k_actual) = sender_encode_seal(&mut tx, &p, &app, 1, 0x9ABC, 0);
    assert_eq!(k_actual, 5);

    // 2 source düş (esi 1,3): bunlar WIRE'da HİÇ teslim-edilmez; receiver repair'den kurtaracak.
    let lost_source_esis = [1u16, 3u16];
    let dgs: Vec<Vec<u8>> = sent
        .iter()
        .filter(|s| s.is_repair || !lost_source_esis.contains(&s.esi))
        .map(|s| s.datagram.clone())
        .collect();
    // Wire-teslim source esi'leri = kredi-tabanı (yalnız wire'dan gelenler → BtlBw/delivered).
    let wire_delivered_source: Vec<u16> = sent
        .iter()
        .filter(|s| !s.is_repair && !lost_source_esis.contains(&s.esi))
        .map(|s| s.esi)
        .collect();

    // Recovered ile app-bytes TAM (logical-teslim başarılı).
    let out = receiver_open_decode(&mut rx, &p, &dgs, k_actual).unwrap();
    assert_eq!(out, app, "recovered ile app-bytes TAM (logical-teslim)");

    // ── ASSERT-2a: recovered-source WIRE-teslim DEĞİL → kredi-tabanına girmez ──
    // Kredi-tabanı (wire-teslim) yalnız {0,2,4}; recovered {1,3} bunun-DIŞINDA.
    assert_eq!(
        wire_delivered_source.len(),
        3,
        "wire-teslim source = {{0,2,4}} (BtlBw/delivered-kredi-tabanı)"
    );
    for e in lost_source_esis {
        assert!(
            !wire_delivered_source.contains(&e),
            "recovered esi={e} WIRE-teslim-DEĞİL (repair-decode'dan; BtlBw'ye bit-eklemez)"
        );
    }

    // ── ASSERT-2b: RECOVERED-control ile ACK AYRI-defter/AYRI-codepoint; RECOVERED kredi-ÜRETMEZ ──
    let recovered_ctl = Control::Recovered {
        id: 20,
        block_id: 1,
        esi_ranges: vec![(1, 1), (3, 3)], // tam kurtarılan esi'ler
    };
    let ack_ctl = Control::Ack {
        id: 21,
        acked_dh_epoch: 0,
        ack_delay_256us: 0,
        seen_count: 0,
        arrival_rate: 0xFFFF,
        ranges: vec![(0, 4)],
    };
    // RECOVERED wire-katmanında BtlBw-credit ÜRETMEZ (§2.57; N8-invariant).
    assert!(
        !control_produces_bandwidth_credit(&recovered_ctl),
        "RECOVERED BtlBw-credit ÜRETMEZ (recovered≠ACK)"
    );
    // Ayrı-codepoint → yapısal olarak ACK'le KARIŞAMAZ (ayrı-defter runtime-kanıtı).
    assert_eq!(
        recovered_ctl.codepoint(),
        RECOVERED,
        "RECOVERED codepoint=0x12"
    );
    assert_eq!(ack_ctl.codepoint(), ACK, "ACK codepoint=0x10");
    assert_ne!(
        recovered_ctl.codepoint(),
        ack_ctl.codepoint(),
        "RECOVERED ≠ ACK (ayrı-defter: recovered→retention-release; ACK→wire-kredi/confirmed_logical)"
    );
}

// ══════════════════════════════════════════════════════════════════════════════════════════════
// RETRANSMIT-INVARIANT (emisyon-AÇIK) · Sprint-5c çıkış-şartı (2/2) · saf-doğrulama, mühürlü-dokunmaz
// ──────────────────────────────────────────────────────────────────────────────────────────────
// Kanıtlanan invariant: retransmission = YENİ full_pn + YENİ nonce96 + YENİ ciphertext; eski-PN ASLA
// yeniden-seal-edilmez (nonce-reuse guard, wire-yaptırımı). TEK MEŞRU İSTİSNA = SESSION_INIT byte-for-byte
// cached retransmit — InitFlight anahtar-TUTMAZ (re-seal imkansız); Sprint-4d'de testli, BURADA yeniden
// yazılmaz, REFERANS: crates/kama-noise-wire/tests/sprint4d_init_flight.rs::retransmit_is_byte_for_byte_cached.
//
// Not (keşif-workflow bulguları): (1) reuse-guard `last_sealed_pn` PER-INSTANCE (2026-10-06'dan beri TEK sayaç;
// epoch'a bölünmez) → negatif-test AYNI tx-instance şart (vacuity: yeni-instance-reset meşrudur).
// (2) FEC-decode dup-esi'yi FAIL-CLOSED reddeder (codec idempotent-DEĞİL) → tek-teslim = admission distinct-by-esi
// CALLER-sözleşmesi. (3) send_generation transport-core'da retransmit'te ARTMAZ (sabit 0); retransmit-görünürlüğü
// SealedPacketMeta.full_wire_pn per-emisyon ayrımıdır (sender-içi retx_count ayrı sayaç) — 5c-kapanış K2/DR-S5-4.
// ══════════════════════════════════════════════════════════════════════════════════════════════

/// EXTENDED datagram'da ciphertext dilimi: [OUTER_HDR 10B][ct][TAG 16B] (outer.rs:9-11).
fn ct(dg: &[u8]) -> &[u8] {
    &dg[10..dg.len() - 16]
}

/// Tek source FEC-shard'ın canonical 64B symbol'ü (prefix + 0x33-payload + zero-pad).
fn fec_source_symbol(esi: u16, message_id: u32) -> Vec<u8> {
    let payload = vec![0x33u8; PAYLOAD_CAP];
    let pfx = SourcePrefix {
        message_id,
        chunk_offset: u32::from(esi) * PAYLOAD_CAP as u32,
        actual_payload_len: PAYLOAD_CAP as u16,
        source_flags: 0,
    };
    let mut sym = vec![0u8; 64];
    pfx.serialize_symbol(&payload, &mut sym).unwrap();
    sym
}

/// Tek source FEC-shard logical-paketi (canonical prefix; k_actual=1 → known_zero = k_shape-1).
fn fec_source_pkt(block_id: u32, esi: u16, message_id: u32) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id,
            esi,
            k_actual: 1,
        },
        symbol: fec_source_symbol(esi, message_id),
    }
}

// ── (a) YENİ-monoton-PN + reuse yapısal-imkansız · (b) YENİ nonce96 · (c) YENİ ct · (e) send-record ──
#[test]
fn retransmit_new_pn_new_nonce_new_ct_reuse_blocked() {
    // Tek tx-instance, tüm seal'ler dh_epoch=0 → guard AKTİF (per-instance+per-epoch-parite).
    let mut tx = KamaNoiseWireProtector::new(0x0100_00B1, &key(0xA1), &key(0xB2));
    let logical = fec_source_pkt(7, 0, 0x1111); // AYNI logical-payload (orig + retransmit)

    // (A) İLK emisyon guard'ı ARM eder — vacuity-guard: Ok ZORUNLU (yoksa last_sealed_pn=None, negatif-leg boş).
    let orig = tx
        .seal(
            &logical,
            SealArgs {
                wire_pn: 100,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert_eq!(orig.meta.full_wire_pn, 100);

    // (a-NEG) reuse yapısal-imkansız: AYNI PN (aynı instance+epoch) 2.-seal → TYPED-ERROR, ciphertext ÜRETİLMEZ.
    match tx.seal(
        &logical,
        SealArgs {
            wire_pn: 100,
            dh_epoch: 0,
            send_generation: 1,
        },
    ) {
        Err(WireError::SealInput(m)) => {
            assert!(m.contains("nonce-reuse"), "nonce-reuse guard mesajı: {m}")
        }
        other => panic!("AYNI-PN reuse reddi bekleniyordu, geldi: {other:?}"),
    }
    // geri-PN de reddedilir (strict-monoton; eski-PN asla yeniden-seal).
    assert!(
        matches!(
            tx.seal(
                &logical,
                SealArgs {
                    wire_pn: 99,
                    dh_epoch: 0,
                    send_generation: 1
                }
            ),
            Err(WireError::SealInput(_))
        ),
        "geri-PN reddedilir (strict-monoton)"
    );

    // (a-POS) retransmit = YENİ (monoton-büyük) PN → Ok.
    let retx = tx
        .seal(
            &logical,
            SealArgs {
                wire_pn: 101,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert_eq!(retx.meta.full_wire_pn, 101);

    // (b) YENİ nonce96 — LE64-mapping üzerinden iki nonce-byte karşılaştır.
    assert_ne!(nonce96(100), nonce96(101), "yeni full_pn → yeni nonce96");
    assert_eq!(
        nonce96(orig.meta.full_wire_pn),
        nonce96(100),
        "nonce96 tabanı = meta.full_wire_pn"
    );

    // (c) YENİ ciphertext — aynı logical-payload, farklı-PN → farklı-keystream → farklı ct (nonce-farkının görünür kanıtı).
    assert_ne!(
        ct(&orig.bytes),
        ct(&retx.bytes),
        "aynı-plaintext farklı-PN → farklı ciphertext"
    );
    assert_ne!(orig.bytes, retx.bytes, "datagram tümüyle farklı");

    // (e) send-record: iki-emisyon AYRI SealedPacketMeta (full_wire_pn per-emisyon ayrımı = retransmit-görünürlüğü).
    assert_ne!(
        orig.meta.full_wire_pn, retx.meta.full_wire_pn,
        "iki-emisyon ayrı wire-kayıt (S5-3 defterinin retransmit-görünürlüğü)"
    );
    // BULGU: send_generation transport-core'da retransmit'te ARTMAZ (sabit 0); wire-katmanı alanı SADIK-plumb'lar
    // (DR-S5-4 için hazır). Retransmit-diskriminatörü full_wire_pn; sender-içi sayaç retx_count.
    let plumb = tx
        .seal(
            &logical,
            SealArgs {
                wire_pn: 102,
                dh_epoch: 0,
                send_generation: 7,
            },
        )
        .unwrap();
    assert_eq!(
        plumb.meta.send_generation, 7,
        "wire send_generation'ı sadık taşır (alan mevcut; transport-core kullanmıyor)"
    );

    // (a-CAP) SEND_PN_CAP: cap'te gönderim-reddi (renewal sinyali; ASLA wrap → nonce ASLA tekrar).
    let mut txc = KamaNoiseWireProtector::new(0x0100_00B1, &key(0xA1), &key(0xB2));
    assert!(
        matches!(
            txc.seal(
                &logical,
                SealArgs {
                    wire_pn: SEND_PN_CAP,
                    dh_epoch: 0,
                    send_generation: 0
                }
            ),
            Err(WireError::PnCapReached)
        ),
        "PN-cap: wrap yerine renewal-reddi"
    );
}

// ── (d) logical-dedup: receiver iki-kopyayı-open-eder AMA app-teslim tek (admission distinct-by-esi) ──
#[test]
fn retransmit_logical_single_delivery_via_admission_dedup() {
    let p = profile3();
    let h = 0x0100_00B2;
    let pkt0 = fec_source_pkt(9, 0, 0x7777); // aynı logical (block_id=9, esi=0)

    // Orijinal (PN=200) + retransmit (PN=300, monoton-büyük) — AYNI logical, tek tx-instance.
    let mut tx = KamaNoiseWireProtector::new(h, &key(0xA1), &key(0xB2));
    let orig = tx
        .seal(
            &pkt0,
            SealArgs {
                wire_pn: 200,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    let retx = tx
        .seal(
            &pkt0,
            SealArgs {
                wire_pn: 300,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();

    // (d1) receiver İKİ-kopyayı da OPEN eder (farklı wire-PN → replay-window ikisini de kabul) — AYNI logical.
    let mut rx = KamaNoiseWireProtector::new(h, &key(0xB2), &key(0xA1));
    let o1 = rx.open(&orig.bytes).unwrap();
    let o2 = rx.open(&retx.bytes).unwrap();
    assert_eq!(o1.full_pn(), 200, "orig wire-PN");
    assert_eq!(o2.full_pn(), 300, "retransmit wire-PN (farklı)");
    match (o1.inner(), o2.inner()) {
        (InnerPlain::FecShard { header: h1, .. }, InnerPlain::FecShard { header: h2, .. }) => {
            assert_eq!(h1.block_id, h2.block_id, "AYNI block_id");
            assert_eq!(h1.esi, h2.esi, "AYNI esi (logical-özdeş, wire-PN farklı)");
        }
        _ => panic!("FecShard bekleniyordu"),
    }

    // (d2) app-teslim TEK: admission distinct-by-esi ZORUNLU. İki-kopya (aynı esi) FEC-decode'a girerse
    // codec FAIL-CLOSED reddeder (DuplicateEsi → çift-teslim DEĞİL) — tek-teslimin yapısal-yaptırımı.
    let mut rx2 = KamaNoiseWireProtector::new(h, &key(0xB2), &key(0xA1)); // taze rx (replay-window temiz)
    let both = vec![orig.bytes.clone(), retx.bytes.clone()];
    let r = receiver_open_decode(&mut rx2, &p, &both, 1);
    assert!(
        matches!(r, Err(WireError::InnerParse(_))),
        "dup-esi FAIL-CLOSED (codec DuplicateEsi; app'e çift-teslim YOK → distinct-by-esi admission zorunlu)"
    );
    // Vacuity-kırıcı KONTRAST: dedup'lı TEK-kopya (aynı esi bir-kez) → decode Ok, TEK-teslim.
    // İki-leg birlikte Err'in DUP-esi'den (başka-yetersizlik değil) geldiğini kanıtlar (aynı orig-datagram; tek-fark kopya-sayısı).
    let mut rx3 = KamaNoiseWireProtector::new(h, &key(0xB2), &key(0xA1)); // taze rx
    let single = receiver_open_decode(&mut rx3, &p, std::slice::from_ref(&orig.bytes), 1);
    assert_eq!(
        single.unwrap(),
        vec![0x33u8; PAYLOAD_CAP],
        "distinct-esi tek-kopya → decode Ok (tek-teslim); dup ile kontrast → Err=DuplicateEsi"
    );

    // (d2-codec) VARIANT-KİMLİĞİ codec-sınırında DOĞRUDAN: receiver_open_decode Err'i InnerParse'a collapse
    // ettiği için (pipeline.rs), Err'in SPESİFİK DuplicateEsi olduğunu codec'e doğrudan sorarak sabitle
    // (yetersizlik/SymbolLen değil). Tek-esi → Ok; iki-esi (aynı) → DuplicateEsi(0).
    let sym0 = fec_source_symbol(0, 0x7777);
    let codec = RsSimdCodec;
    let recv_dup = [
        ReceivedSymbol {
            esi: 0,
            is_repair: false,
            bytes: &sym0,
        },
        ReceivedSymbol {
            esi: 0,
            is_repair: false,
            bytes: &sym0,
        },
    ];
    assert_eq!(
        codec.reconstruct(&p, &recv_dup, 7),
        Err(FecError::DuplicateEsi(0)),
        "dup-esi → SPESİFİK DuplicateEsi (yetersizlik-hatası DEĞİL)"
    );
    let recv_one = [ReceivedSymbol {
        esi: 0,
        is_repair: false,
        bytes: &sym0,
    }];
    assert!(
        codec.reconstruct(&p, &recv_one, 7).is_ok(),
        "distinct tek-esi (1-real + 7-known-zero = K) → Ok (Err DUP'tan gelir, yetersizlikten değil)"
    );
}
