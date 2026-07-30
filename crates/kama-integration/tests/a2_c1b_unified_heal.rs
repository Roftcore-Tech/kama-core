//! CLOSURE-1B — BİRLEŞİK 65B HEAL (DR-CS-1b/A2 FINAL VALIDATION; Abkarada CLOSURE-1B-onayı, B-yolu).
//!
//! KAPSAM: sender→TimedOut → receiver decode/delivery → RECOVERED → sender PostTimeoutNotice
//! zincirini TEK birleşik akışta, AYRI directional TX/RX anahtarla doğrular (hücre-(i) kapanışı).
//!
//! ANAHTAR-SÖZLEŞMESİ (Abkarada bağlayıcı):
//! - AYRI directional key: sender-TX = receiver-RX (KA); receiver-TX = sender-RX (KB); **KA ≠ KB**
//!   (aynı-symmetric-key iki-yönde paylaşılmaz).
//! - ANAHTAR ENJEKSİYONU **TEST-ONLY**'dir (açıkça adlandırılır). Gerçek Noise-handshake + Split
//!   Sprint-6 kripto işidir; bu test **"real handshake" veya "handshake-derived key" İDDİA ETMEZ**.
//!   Doğrulanan: directional-split DAVRANIŞI + birleşik-ürün-akışı — handshake-DERIVATION DEĞİL
//!   (hücre-(iii) AÇIK kalır; final-hüküm PARTIAL).
//! - BRIDGE (RECOVERED-wire → RecoveredInfo) GERÇEK-kodeklerle kurulur: `KamaNoiseWireProtector::open`
//!   (gerçek-AEAD) + `decode_control` + `RecoveredMap::resolve`. Mekanizma fake'lenmez; test yalnız
//!   kripto/engine glue-katmanının rolünü gerçek-API'lerle oynar (gerçek-üründe bu katman krypto-layer).
//! - `ReceiverHost` product-path bypass EDİLMEZ; harness-ledger product-acceptance yerine GEÇMEZ.

#![cfg(feature = "experimental-cold-start")]

use adaptive_transport::sender::{
    AckInfo, CsTerminal, Event, OpaqueUnit, PostTimeoutCause, RecoveredInfo, RecoveredUnit, Sender,
    Timer,
};
use kama_engine::RecoveredMap;
use kama_integration::{IngestOutcome, ReceiverHost};
use kama_noise_wire::control::decode_control;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use std::time::Duration;
use zeroize::Zeroizing;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const H: u32 = 0x00C1_B065; // CLOSURE-1B handle
const HEAL_BLOCK: u32 = 7; // sıfır-dışı: identity-propagation gözlemlenebilir olsun
const K_ACTUAL: u8 = 8;
const PROFILE: u8 = 0x03; // MANDATORY-BOOTSTRAP (K=8/R=4/E=64)

// ── AYRI directional anahtar-materyali (TEST-ONLY key injection; gerçek-handshake İDDİA EDİLMEZ) ──
/// KA: sender→receiver kanal-anahtarı (sender.send == receiver.recv).
fn ka() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0xA1u8; 32])
}
/// KB: receiver→sender kanal-anahtarı (receiver.send == sender.recv). KA ≠ KB.
fn kb() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0xB2u8; 32])
}

/// Sender-endpoint protector: send=KA (kaynak-shard seal) + recv=KB (RECOVERED open) — TEK gerçek
/// endpoint, iki-yön bağımsız cipher/PN. Directional (paired crossed-key; degenerate-symmetric değil).
fn sender_endpoint() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H, &ka(), &kb())
}

/// Bir source-shard'ı verilen protector'un SEND-cipher'ıyla seal → wire (receiver product-path açar).
fn seal_source(ep: &mut KamaNoiseWireProtector, block: u32, esi: u16, pn: u64) -> Vec<u8> {
    let pfx = SourcePrefix {
        message_id: 1,
        chunk_offset: 0,
        actual_payload_len: 10,
        source_flags: 0,
    };
    let mut sym = vec![0u8; 64];
    pfx.serialize_symbol(&[0xCD; 10], &mut sym).unwrap();
    ep.seal(
        &UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Source,
                profile_id: PROFILE,
                block_id: block,
                esi,
                k_actual: K_ACTUAL,
            },
            symbol: sym,
        },
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

/// Sender'ı bootstrap → giveup@60s → TimedOut sür (F4-şablonu). `size` = sınıf-göstergesi.
fn drive_to_timed_out(s: &mut Sender, size: u64) {
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size })]);
    let mut t = 100 * MS;
    while t <= 60 * SEC + 200 * MS {
        let _ = s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]);
        t += 100 * MS;
    }
    assert_eq!(
        s.take_cs_terminal(),
        Some(CsTerminal::TimedOut),
        "bootstrap TimedOut terminaline ulaşmadı"
    );
}

/// RECEIVER: HEAL_BLOCK'un K source-shard'ını GERÇEK-ingest yolundan geçir → decode+delivery.
/// Döner: son `IngestOutcome` (`BlockDelivered` beklenir).
fn receiver_ingest_block(
    host: &mut ReceiverHost,
    tx: &mut KamaNoiseWireProtector,
    base_pn: u64,
) -> IngestOutcome {
    let mut last = IngestOutcome::Dropped("init");
    for esi in 0..u16::from(K_ACTUAL) {
        let dg = seal_source(tx, HEAL_BLOCK, esi, base_pn + u64::from(esi));
        last = host.on_datagram(SEC, &dg).unwrap();
    }
    last
}

/// Sender'ın emit-anı `(block,esi)→unit_id` defteri (gerçek-sender emit'te register eder; burada
/// test glue-rolünde — HEAL_BLOCK'un tüm source-ESI'leri kayıtlı).
fn sent_map() -> RecoveredMap {
    let mut map = RecoveredMap::new();
    for esi in 0..u16::from(K_ACTUAL) {
        map.register(
            HEAL_BLOCK,
            esi,
            u64::from(HEAL_BLOCK) * 100 + u64::from(esi),
        );
    }
    map
}

/// BRIDGE (gerçek-kodekler): RECOVERED-wire → sender-endpoint.open (gerçek-AEAD) → decode_control →
/// RecoveredMap.resolve → RecoveredInfo. AEAD-open BAŞARISI = directional-key doğruluğunun kanıtı.
fn recovered_wire_to_info(
    sender_ep: &mut KamaNoiseWireProtector,
    map: &RecoveredMap,
    wire: &[u8],
) -> RecoveredInfo {
    let opened = sender_ep
        .open(wire)
        .expect("RECOVERED-wire directional-key (recv=KB) ile AÇILMALI");
    let ctrl = decode_control(opened.inner())
        .expect("opened inner Control-olmalı")
        .expect("Control::Recovered decode-etmeli");
    let (resolved, unresolved) = map.resolve(&ctrl).expect("Recovered resolve");
    assert_eq!(
        unresolved, 0,
        "tüm source-ESI'ler emit-defterinde kayıtlı olmalı"
    );
    RecoveredInfo {
        units: resolved
            .iter()
            .map(|r| RecoveredUnit {
                unit_id: r.unit_id,
                block_id: r.block_id,
                esi: r.esi,
            })
            .collect(),
    }
}

/// BİRLEŞİK 65B HEAL — sender+receiver TEK akış, AYRI directional-key, ≥3 deterministik tekrar.
///
/// Zincir (Abkarada §tek-akış-kabul): (1-2) sender bootstrap→TimedOut · (3) receiver directional-key
/// session · (4-5) gerçek-ingest→decode→delivery · (6) RECOVERED emit (ControlIdIssueGate) · (7-9)
/// gerçek open+decode+resolve bridge · (10) sender RECOVERED işleme · (11) notice.block_id =
/// recovered-block (identity) · (12) resource baseline'a döner. N8: heal BtlBw-BİT-ÖZDEŞ.
#[test]
fn c1b_unified_65b_heal_directional_three_reps() {
    for rep in 0..3u32 {
        // ── (1-2) SENDER bootstrap → TimedOut (65B-sınıfı; ACK-bağımsız heal beklenir).
        let mut sender = Sender::new();
        drive_to_timed_out(&mut sender, 65);
        assert_eq!(
            sender.take_post_timeout_notice(),
            None,
            "rep {rep}: TimedOut tek-başına notice üretmemeli"
        );

        // ── (3) RECEIVER directional-key session: send=KB, recv=KA (sender-TX(KA)=receiver-RX(KA)).
        let mut host = ReceiverHost::new();
        host.install_session_with_directional_keys(H, &kb(), &ka())
            .unwrap();
        // Sender-endpoint (send=KA, recv=KB): kaynak-shard seal + RECOVERED open — TEK gerçek endpoint.
        let mut sender_ep = sender_endpoint();

        // ── (4-5) GERÇEK ingest → decode → delivery (product path).
        let out = receiver_ingest_block(&mut host, &mut sender_ep, 0);
        assert!(
            matches!(out, IngestOutcome::BlockDelivered { block_id, .. } if block_id == HEAL_BLOCK),
            "rep {rep}: receiver decode/delivery zinciri kırıldı: {out:?}"
        );
        assert_eq!(
            host.delivered_count(),
            1,
            "rep {rep}: tam-bir-blok teslim edilmeli"
        );
        assert_eq!(
            host.global_cold().res.retained_bytes,
            0,
            "rep {rep}: teslim-sonrası resource baseline'a (0) dönmeli"
        );

        // ── (6) RECEIVER RECOVERED emit (bütçeli; F-CID-0 ControlIdIssueGate) → gerçek-wire.
        let recovered_wire = host
            .emit_recovered(H, HEAL_BLOCK, vec![(0, u16::from(K_ACTUAL) - 1)], 0)
            .unwrap();

        // ── (7-9) BRIDGE: gerçek open (recv=KB directional) + decode + resolve.
        let map = sent_map();
        let info = recovered_wire_to_info(&mut sender_ep, &map, &recovered_wire);
        assert_eq!(
            info.units.len(),
            usize::from(K_ACTUAL),
            "rep {rep}: 8 source-ESI çözülmeli"
        );

        // ── (10-12) SENDER RECOVERED işleme → PostTimeoutNotice{RecoveredControl}; N8 BtlBw-özdeş.
        let (lfb_b, w_b) = sender.diag_wbtlbw();
        let _ = sender.on_event(Event::RecoveredControl(info), 61 * SEC);
        let notice = sender
            .take_post_timeout_notice()
            .expect("rep: RecoveredControl PostTimeoutNotice üretmeli");
        assert_eq!(
            notice.cause,
            PostTimeoutCause::RecoveredControl,
            "rep {rep}: 65B-sınıfı ACK-bağımsız heal cause = RecoveredControl"
        );
        assert_eq!(
            notice.block_id, HEAL_BLOCK,
            "rep {rep}: notice.block_id receiver-recovered-block ile eşleşmeli (identity)"
        );
        assert_eq!(
            notice.cold_epoch, 0,
            "rep {rep}: tek-epoch (MAX_ACTIVE_COLD_EPOCHS_PER_SESSION_DIRECTION=1)"
        );
        let (lfb_a, w_a) = sender.diag_wbtlbw();
        assert_eq!(
            (lfb_b, w_b),
            (lfb_a, w_a),
            "rep {rep}: birleşik-heal BtlBw'yi değiştirdi (N8 ihlali — heal SALT-LOGICAL olmalı)"
        );
    }
}

// ── NEGATİF PİNLER (Abkarada 7-liste; her biri GERÇEK-davranışa karşı pinlenir) ────────────────

/// NEG-1 — yanlış-key kaynak-shard → receiver AEAD open reject (side-effect-yok drop).
#[test]
fn c1b_neg_wrong_key_aead_reject() {
    let mut host = ReceiverHost::new();
    host.install_session_with_directional_keys(H, &kb(), &ka())
        .unwrap();
    // Sender-endpoint YANLIŞ send-key (receiver.recv=KA beklerken KX).
    let kx = Zeroizing::new([0xEEu8; 32]);
    let mut bad = KamaNoiseWireProtector::new(H, &kx, &kb());
    let dg = seal_source(&mut bad, HEAL_BLOCK, 0, 0);
    assert_eq!(
        host.on_datagram(SEC, &dg).unwrap(),
        IngestOutcome::Dropped("aead verify failed"),
        "yanlış-key AEAD-reject üretmedi (verify-before-admission ihlali)"
    );
    // Cold-defter DOKUNULMAMIŞ (reject side-effect-yok).
    assert_eq!(host.global_cold().res.retained_bytes, 0);
}

/// NEG-2 — ters-direction key (send/recv swap) → receiver open reject.
#[test]
fn c1b_neg_reversed_direction_reject() {
    let mut host = ReceiverHost::new();
    host.install_session_with_directional_keys(H, &kb(), &ka())
        .unwrap();
    // Sender-endpoint directional-key TERS: send=KB (receiver.recv=KA bekler) → uyuşmaz.
    let mut reversed = KamaNoiseWireProtector::new(H, &kb(), &ka());
    let dg = seal_source(&mut reversed, HEAL_BLOCK, 0, 0);
    assert_eq!(
        host.on_datagram(SEC, &dg).unwrap(),
        IngestOutcome::Dropped("aead verify failed"),
        "ters-direction key reject üretmedi (sender-TX≠receiver-RX kabul edildi)"
    );
}

/// NEG-3 — stale-epoch (TimedOut-ÖNCESİ) RecoveredControl → notice YOK (post-timeout-gate).
#[test]
fn c1b_neg_stale_epoch_no_notice() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 65 })]);
    let info = RecoveredInfo {
        units: vec![RecoveredUnit {
            unit_id: 700,
            block_id: HEAL_BLOCK,
            esi: 0,
        }],
    };
    // Epoch henüz TimedOut-DEĞİL (stale) → notice-gate kapalı.
    let _ = s.on_event(Event::RecoveredControl(info), 5 * SEC);
    assert_eq!(
        s.take_post_timeout_notice(),
        None,
        "TimedOut-öncesi (stale-epoch) RecoveredControl notice üretti"
    );
}

/// NEG-4 — duplicate RECOVERED → ikinci notice YOK (first-valid-proof-wins; NOTICE_BOUND=1 one-slot).
#[test]
fn c1b_neg_duplicate_recovered_single_notice() {
    let mut s = Sender::new();
    drive_to_timed_out(&mut s, 65);
    let info = RecoveredInfo {
        units: vec![RecoveredUnit {
            unit_id: 700,
            block_id: HEAL_BLOCK,
            esi: 0,
        }],
    };
    let _ = s.on_event(Event::RecoveredControl(info.clone()), 61 * SEC);
    let _ = s.on_event(Event::RecoveredControl(info), 62 * SEC); // duplicate (take-etmeden)
    assert!(
        s.take_post_timeout_notice().is_some(),
        "ilk notice üretilmeli"
    );
    assert_eq!(
        s.take_post_timeout_notice(),
        None,
        "duplicate ikinci-notice üretti (one-slot ihlali)"
    );
}

/// NEG-5 — aynı-epoch FARKLI-block → ikinci-notice YOK (MAX_COLD_BLOCKS_PER_EPOCH=1 invariant).
///
/// GERÇEK-DAVRANIŞ: sealed-uygulama bu invariant'ı **first-valid-proof-wins SUPPRESSION** olarak
/// gerçekler (sender.rs record_post_timeout_notice: slot-doluysa no-op) — direktif "InvariantViolation"
/// der ama üretimde hard-error DEĞİL. İkisi de invariant-korur (bir-notice/epoch); üretim-davranışı
/// DEĞİŞTİRİLMEZ (bu bir DUR-B-tetiği olurdu). İfade-sapması final-raporda not-edilir.
#[test]
fn c1b_neg_same_epoch_different_block_suppressed() {
    let mut s = Sender::new();
    drive_to_timed_out(&mut s, 65);
    let first = RecoveredInfo {
        units: vec![RecoveredUnit {
            unit_id: 700,
            block_id: HEAL_BLOCK,
            esi: 0,
        }],
    };
    let other = RecoveredInfo {
        units: vec![RecoveredUnit {
            unit_id: 900,
            block_id: HEAL_BLOCK + 2, // aynı-epoch, FARKLI-block
            esi: 0,
        }],
    };
    let _ = s.on_event(Event::RecoveredControl(first), 61 * SEC);
    let _ = s.on_event(Event::RecoveredControl(other), 62 * SEC);
    let n = s
        .take_post_timeout_notice()
        .expect("ilk-block notice üretilmeli");
    assert_eq!(
        n.block_id, HEAL_BLOCK,
        "farklı-block ilk-notice'i ezdi (MAX_COLD_BLOCKS_PER_EPOCH=1 ihlali)"
    );
    assert_eq!(
        s.take_post_timeout_notice(),
        None,
        "tek-notice/epoch (suppression bozuldu)"
    );
}

/// NEG-6 — TimedOut-sonrası delivery-VAR ama sender'a valid-proof-YOK → notice YOK.
/// (Receiver gerçekten teslim eder ama RECOVERED sender'a ULAŞMAZSA sender heal-bilmez.)
#[test]
fn c1b_neg_delivery_without_proof_no_notice() {
    let mut sender = Sender::new();
    drive_to_timed_out(&mut sender, 65);

    let mut host = ReceiverHost::new();
    host.install_session_with_directional_keys(H, &kb(), &ka())
        .unwrap();
    let mut sender_ep = sender_endpoint();
    let out = receiver_ingest_block(&mut host, &mut sender_ep, 0);
    assert!(
        matches!(out, IngestOutcome::BlockDelivered { .. }),
        "receiver gerçekten teslim etmeli"
    );

    // RECOVERED sender'a HİÇ beslenmez → proof-yok → notice-yok.
    assert_eq!(
        sender.take_post_timeout_notice(),
        None,
        "proof-yokken (RECOVERED sender'a ulaşmadı) notice üretildi"
    );
}

// NEG-7 (flag-OFF → eski-davranış): bu dosya `#![cfg(feature = \"experimental-cold-start\")]` ile
// flag-OFF'ta TÜMDEN derlenmez; cold-start-yüzeyi (PostTimeoutNotice/RecoveredControl-notice/
// ReceiverHost) cfg-gated. Flag-OFF byte-özdeşliği crate-düzeyi INV-CS-5 kapısıyla (workspace flag-OFF
// test-sayısı DEĞİŞMEZ) doğrulanır — bu dosya o sayıya HİÇ katkı yapmaz.

// ── 300B-SINIFI SMOKE/REGRESSION (DataAckClocked) — 65B-acceptance'a KARIŞTIRILMAZ ──────────────

/// 300B-SINIFI DataAckClocked SMOKE: aynı-directional-fixture-sınıfı; post-timeout DATA-ACK heal →
/// PostTimeoutNotice{DataAckClocked} (block=0). Abkarada: yalnız smoke/regression — 65B-acceptance
/// (RecoveredControl, ACK-bağımsız) ile AYRI kanıt.
#[test]
fn c1b_300b_data_ack_clocked_smoke() {
    let mut s = Sender::new();
    drive_to_timed_out(&mut s, 300);
    assert_eq!(
        s.take_post_timeout_notice(),
        None,
        "TimedOut tek-başına notice üretmemeli"
    );
    // Post-timeout DATA-ACK (bootstrap-unit seq=0) → bootstrap-srtt → PostTimeoutRecovery +
    // DataAckClocked notice (cs_phase2_resolve TETİK-2a).
    let ack = Event::AckReceived(AckInfo {
        acked_seqs: vec![0],
        ack_delay: Duration::from_millis(1),
    });
    let _ = s.on_event(ack, 61 * SEC);
    let n = s
        .take_post_timeout_notice()
        .expect("300B post-timeout DATA-ACK heal notice üretmeli");
    assert_eq!(
        n.cause,
        PostTimeoutCause::DataAckClocked,
        "300B-sınıfı cause = DataAckClocked (RecoveredControl DEĞİL)"
    );
}
