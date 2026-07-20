//! Sprint-4c: dh_epoch 0→1 confirmation-safe machine (§4-B3) + protector-entegrasyonu (epoch1-PN=0).

use kama_noise_wire::epoch_machine::GRACE_MIN_PACKETS;
use kama_noise_wire::{
    DhEpochMachine, EpochAction, KamaNoiseWireProtector, RecvEpoch0, Role, SealArgs,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}
fn fec() -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 3,
            block_id: 1,
            esi: 0,
            k_actual: 1,
        },
        symbol: vec![0u8; 64],
    }
}

// ── Adım-2 (Alice): RESPONSE alınca send_epoch 0→1 + InstallEpoch1AndSend ──
#[test]
fn initiator_response_switches_send_epoch() {
    let mut m = DhEpochMachine::new(Role::Initiator, 100);
    assert_eq!(m.send_epoch(), 0);
    assert_eq!(
        m.on_response_received(),
        Some(EpochAction::InstallEpoch1AndSend)
    );
    assert_eq!(m.send_epoch(), 1);
    assert!(m.recv_accepts(1));
    // İdempotent: ikinci-kez NO-OP.
    assert_eq!(m.on_response_received(), None);
}

// ── RESPONSE-KAYIP: Alice epoch-0'da-kalır ("epoch-0-usable" garantisi) ──
#[test]
fn response_loss_keeps_epoch0() {
    let m = DhEpochMachine::new(Role::Initiator, 100);
    // on_response_received hiç çağrılmadı → send_epoch=0, epoch-0-recv-Accepting.
    assert_eq!(m.send_epoch(), 0);
    assert!(m.recv_accepts(0));
    assert!(!m.recv_accepts(1), "epoch-1 kurulmadı");
}

// ── Adım-3/4 (Bob): ilk-geçerli-epoch-1 → send=1 + grace-başlar ──
#[test]
fn responder_epoch1_recv_starts_grace() {
    let mut m = DhEpochMachine::new(Role::Responder, 100);
    assert_eq!(m.send_epoch(), 0);
    // Bob RESPONSE-göndermez → on_response NO-OP.
    assert_eq!(m.on_response_received(), None);
    let actions = m.on_valid_epoch1_received();
    assert_eq!(
        actions,
        vec![EpochAction::InstallEpoch1AndSend, EpochAction::StartGrace]
    );
    assert_eq!(m.send_epoch(), 1);
    assert!(matches!(m.recv0(), RecvEpoch0::Grace { .. }));
    // Grace boyunca epoch-0 HÂLÂ kabul (adım-4: grace-aktif ama discard-değil).
    assert!(m.recv_accepts(0));
    assert!(m.recv_accepts(1));
    // İdempotent.
    assert!(m.on_valid_epoch1_received().is_empty());
}

// ── Adım-5: grace-tick tükenince epoch-0-recv discard ──
#[test]
fn grace_expiry_discards_epoch0() {
    let mut m = DhEpochMachine::new(Role::Responder, GRACE_MIN_PACKETS);
    m.on_valid_epoch1_received();
    // GRACE_MIN_PACKETS-1 tick: hâlâ grace.
    for _ in 0..(GRACE_MIN_PACKETS - 1) {
        assert_eq!(m.on_grace_tick(), None);
        assert!(m.recv_accepts(0));
    }
    // Son tick: discard.
    assert_eq!(m.on_grace_tick(), Some(EpochAction::DiscardEpoch0Recv));
    assert_eq!(m.recv0(), RecvEpoch0::Discarded);
    assert!(!m.recv_accepts(0), "grace-bitti → epoch-0-recv-discard");
    assert!(m.recv_accepts(1));
}

// ── Grace-budget GRACE_MIN_PACKETS'in altına düşürülemez (max clamp) ──
#[test]
fn grace_min_clamp() {
    let mut m = DhEpochMachine::new(Role::Responder, 1); // < GRACE_MIN
    m.on_valid_epoch1_received();
    let mut ticks = 0;
    while m.recv_accepts(0) {
        m.on_grace_tick();
        ticks += 1;
        assert!(ticks <= GRACE_MIN_PACKETS + 1);
    }
    assert_eq!(ticks, GRACE_MIN_PACKETS, "grace en-az GRACE_MIN_PACKETS");
}

// ── Entegrasyon: transition sonrası epoch-1 seal/open PN=0'dan başlar (§4 EPOCH-PN-INIT) ──
#[test]
fn epoch1_pn_starts_zero() {
    let mut a = KamaNoiseWireProtector::new(0x9, &key(0xAA), &key(0xBB));
    let mut b = KamaNoiseWireProtector::new(0x9, &key(0xBB), &key(0xAA));
    // Machine adım-2/3 → epoch-1 kur (engine InstallEpoch1AndSend'i uygular).
    a.install_epoch1_keys(&key(0xCC), &key(0xDD));
    b.install_epoch1_keys(&key(0xDD), &key(0xCC));
    // epoch-0'da bir paket gönderilmiş olsun (pn=0..2).
    let _ = a
        .seal(
            &fec(),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    let _ = a
        .seal(
            &fec(),
            SealArgs {
                wire_pn: 5,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    // epoch-1 PN=0'dan başlar (ayrı-cipher-state; epoch-0'ın pn=5'inden devam-ETMEZ).
    assert_eq!(b.recv_expected_pn(1), 0);
    let s = a
        .seal(
            &fec(),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 1,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&s.bytes).is_ok(), "epoch-1 pn=0 kabul");
    assert_eq!(b.recv_expected_pn(1), 1);
    assert_eq!(b.recv_expected_pn(0), 0, "epoch-0/1 PN-state AYRI");
}
