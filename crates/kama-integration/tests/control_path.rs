//! Sprint-5a: control-yolu uçtan-uca (mock-key). HANDLE_ASSIGN(hub_sim) → PARAM_SET → FEC-emisyon →
//! ~~ACK/SACK(epoch'lu)~~ PARAM_ACK(hash-echo) → RECOVERED(credit-ÜRETMEZ-assert) →
//! BLOCK_COMPLETE → SESSION_RENEW_REQUIRED. [D2 2026-10-08] noise-wire Ack/Sack ailesi kodda yok
//! (K3: ürün ACK'i engine `ACK_V2`); 0x10/0x11 RESERVED → reject (codec/schema testlerinde pinli).

use kama_noise_wire::retry::control_produces_bandwidth_credit;
use kama_noise_wire::{
    control::decode_control, HubSim, InnerPlain, KamaNoiseWireProtector, ResponderDelivery,
    SealArgs, WireError,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}
fn pair(h_ext: u32) -> (KamaNoiseWireProtector, KamaNoiseWireProtector) {
    (
        KamaNoiseWireProtector::new(h_ext, &key(0xA1), &key(0xB2)),
        KamaNoiseWireProtector::new(h_ext, &key(0xB2), &key(0xA1)),
    )
}

/// Bir control'ü seal→open→decode roundtrip et.
fn roundtrip_control(
    tx: &mut KamaNoiseWireProtector,
    rx: &mut KamaNoiseWireProtector,
    ctl: Control,
    pn: u64,
    epoch: u8,
) -> Result<Control, WireError> {
    let sealed = tx.seal(
        &UnsealedTransportPacket::Control(ctl),
        SealArgs {
            wire_pn: pn,
            dh_epoch: epoch,
            send_generation: 0,
        },
    )?;
    let opened = rx.open(&sealed.bytes)?;
    decode_control(opened.inner()).expect("control")
}

#[test]
fn control_flow_end_to_end() {
    // 1) HANDLE_ASSIGN akışı (hub_sim) → h_ext ata.
    let mut hub = HubSim::new(0x0200_0000);
    let assign = hub.admit_init(0xAAA, 1);
    let h_ext = assign.h_ext;
    // Responder-stream'de HANDLE_ASSIGN ilk (sıralama-garantisi).
    assert!(matches!(
        hub.responder_stream()[0],
        ResponderDelivery::HandleAssign(_)
    ));

    let (mut client, mut server) = pair(h_ext);
    let mut pn = 0u64;

    // 2) PARAM_SET (client→server; ACK-eliciting).
    let param = Control::ParamSet {
        id: 1,
        param_hash: [0x5A; 32],
        profile_id: 0x03,
        code_suite_id: 1,
    };
    let got = roundtrip_control(&mut client, &mut server, param.clone(), pn, 0).unwrap();
    assert_eq!(got, param);
    pn += 1;

    // 3) FEC-emisyon (client→server; Bob-offline-olsa-da akar — B2 proactive).
    let fec = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 0,
            k_actual: 4,
        },
        symbol: vec![0x11u8; 64],
    };
    let sealed_fec = client
        .seal(
            &fec,
            SealArgs {
                wire_pn: pn,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(matches!(
        server.open(&sealed_fec.bytes).unwrap().inner(),
        InnerPlain::FecShard { .. }
    ));
    let _ = pn; // client-yön PN'i burada biter (server-yön AYRI spn kullanır)

    // Sunucu-tarafı yanıtları (server→client): AYRI protector-yönü.
    let (mut s2c_tx, mut s2c_rx) = pair(h_ext);
    let mut spn = 0u64;

    // 4) ~~ACK (epoch-scoped) / SACK~~ — [D2 2026-10-08] Ack/Sack ailesi kaldırıldı (K3). Yerine
    //    sunucunun PARAM_SET'e yanıtı PARAM_ACK (hash-echo; NoRetry saf geri-bildirim — eski ACK
    //    ile aynı retry sınıfı).
    let param_ack = Control::ParamAck {
        id: 10,
        param_hash: [0x5A; 32],
    };
    assert_eq!(
        roundtrip_control(&mut s2c_tx, &mut s2c_rx, param_ack.clone(), spn, 0).unwrap(),
        param_ack
    );
    spn += 1;

    // 5) RECOVERED — credit ÜRETMEZ (N8-invariant).
    let recovered = Control::Recovered {
        id: 12,
        block_id: 7,
        esi_ranges: vec![(0, 0)],
    };
    let got_rec = roundtrip_control(&mut s2c_tx, &mut s2c_rx, recovered.clone(), spn, 0).unwrap();
    assert_eq!(got_rec, recovered);
    assert!(
        !control_produces_bandwidth_credit(&got_rec),
        "RECOVERED bandwidth-credit ÜRETMEZ"
    );
    spn += 1;

    // 6) BLOCK_COMPLETE.
    let complete = Control::BlockComplete {
        id: 13,
        block_id: 7,
        final_source_count: 4,
    };
    assert_eq!(
        roundtrip_control(&mut s2c_tx, &mut s2c_rx, complete.clone(), spn, 0).unwrap(),
        complete
    );
    spn += 1;

    // 7) SESSION_RENEW_REQUIRED (cap/wrap-yaklaşımı tetiği; §5 → yeni-handshake).
    let renew = Control::SessionRenewRequired {
        id: 14,
        reason_code: 1,
        renew_hint: 0,
    };
    assert_eq!(
        roundtrip_control(&mut s2c_tx, &mut s2c_rx, renew.clone(), spn, 0).unwrap(),
        renew
    );
}

// ── PARAM_ACK = confirmation (hash-echo) — data-admission-kapısı DEĞİL ──
#[test]
fn param_ack_hash_echo() {
    let (mut tx, mut rx) = pair(0x0200_0100);
    let hash = [0xABu8; 32];
    let pset = Control::ParamSet {
        id: 1,
        param_hash: hash,
        profile_id: 3,
        code_suite_id: 1,
    };
    let got = roundtrip_control(&mut tx, &mut rx, pset, 0, 0).unwrap();
    if let Control::ParamSet { param_hash, .. } = got {
        // PARAM_ACK aynı-hash'i echo eder.
        let ack = Control::ParamAck { id: 2, param_hash };
        let echoed = roundtrip_control(&mut tx, &mut rx, ack, 1, 0).unwrap();
        assert_eq!(
            echoed,
            Control::ParamAck {
                id: 2,
                param_hash: hash
            }
        );
    } else {
        panic!("ParamSet beklendi");
    }
}

// ── RECOVERED hiçbir control-tipi credit-üretmez (tam-set) ──
#[test]
fn no_control_produces_credit() {
    // [D2 2026-10-08] ~~Ack/Sack~~ (aile kodda yok, K3) → kalan geri-bildirim türleri: CS_ECHO
    // (eski ACK gibi seen_count taşır; alan YALNIZ-LEDGER) ve PARAM_ACK (hash-echo).
    let all = [
        Control::CsEcho {
            id: 1,
            subtype: kama_noise_wire::control::CS_SUBTYPE_COLD_START,
            probe_seq: 0,
            seen_count: 0,
            arrival_gap: 0,
        },
        Control::ParamAck {
            id: 2,
            param_hash: [0; 32],
        },
        Control::Recovered {
            id: 3,
            block_id: 1,
            esi_ranges: vec![(0, 0)],
        },
        Control::BlockComplete {
            id: 4,
            block_id: 1,
            final_source_count: 1,
        },
    ];
    for c in all {
        assert!(
            !control_produces_bandwidth_credit(&c),
            "credit-YOK: {:#04x}",
            c.codepoint()
        );
    }
}
