//! Sprint-4c: dh_epoch 0→1 confirmation-safe machine (§4-B3) + protector-entegrasyonu (PN epoch'lar boyunca sürekli).

use core::time::Duration;

use kama_noise_wire::epoch_machine::{GRACE_MIN_PACKETS, GRACE_T_G};
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
    let mut m = DhEpochMachine::new(Role::Initiator);
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
    let m = DhEpochMachine::new(Role::Initiator);
    // on_response_received hiç çağrılmadı → send_epoch=0, epoch-0-recv-Accepting.
    assert_eq!(m.send_epoch(), 0);
    assert!(m.recv_accepts(0));
    assert!(!m.recv_accepts(1), "epoch-1 kurulmadı");
}

// ── Adım-3/4 (Bob): ilk-geçerli-epoch-1 → send=1 + grace-başlar ──
#[test]
fn responder_epoch1_recv_starts_grace() {
    let mut m = DhEpochMachine::new(Role::Responder);
    assert_eq!(m.send_epoch(), 0);
    // Bob RESPONSE-göndermez → on_response NO-OP.
    assert_eq!(m.on_response_received(), None);
    // RESPONSE gönderilirken epoch-1 ALIM anahtarları kurulur; gönderim epoch-0'da kalır (adım-1).
    m.on_response_sent();
    assert_eq!(m.send_epoch(), 0);
    assert!(m.recv_accepts(1));
    let actions = m.on_valid_epoch1_received(Duration::ZERO);
    assert_eq!(
        actions,
        vec![EpochAction::InstallEpoch1AndSend, EpochAction::StartGrace]
    );
    assert_eq!(m.send_epoch(), 1);
    assert!(matches!(m.recv0(), RecvEpoch0::Grace { .. }));
    // Grace boyunca epoch-0 HÂLÂ kabul (adım-4: grace-aktif ama discard-değil).
    assert!(m.recv_accepts(0));
    assert!(m.recv_accepts(1));
    // Sonraki geçerli epoch-1 paketi yalnız sayımdır (eylem yok).
    assert!(m.on_valid_epoch1_received(Duration::ZERO).is_empty());
}

// ── Adım-5: N_g geçerli epoch-1 paketi (onaylayan #1) epoch-0-recv'i discard eder ──
#[test]
fn grace_expiry_discards_epoch0() {
    let mut m = DhEpochMachine::new(Role::Responder);
    m.on_response_sent();
    let _ = m.on_valid_epoch1_received(Duration::ZERO); // onaylayan paket = #1
                                                        // #2 .. #(N_g − 1): hâlâ grace.
    for _ in 2..GRACE_MIN_PACKETS {
        assert!(m.on_valid_epoch1_received(Duration::ZERO).is_empty());
        assert!(m.recv_accepts(0));
    }
    // N_g'inci paket: discard.
    assert_eq!(
        m.on_valid_epoch1_received(Duration::ZERO),
        vec![EpochAction::DiscardEpoch0Recv]
    );
    assert_eq!(m.recv0(), RecvEpoch0::Discarded);
    assert!(!m.recv_accepts(0), "grace-bitti → epoch-0-recv-discard");
    assert!(m.recv_accepts(1));
}

// ── KS-GRACE-1: N_g SABİT (kurucu bütçesi yok) ve T_g süresi de kapatır; hangisi önce ──
#[test]
fn n_g_is_exact_and_t_g_closes_grace() {
    let mut m = DhEpochMachine::new(Role::Responder);
    m.on_response_sent();
    let mut counted = 0;
    while m.recv_accepts(0) {
        let _ = m.on_valid_epoch1_received(Duration::ZERO);
        counted += 1;
        assert!(counted <= GRACE_MIN_PACKETS, "N_g'de kapanmadı");
    }
    assert_eq!(counted, GRACE_MIN_PACKETS, "onaylayan dahil tam N_g");

    let mut t = DhEpochMachine::new(Role::Responder);
    t.on_response_sent();
    let start = Duration::from_secs(5);
    let _ = t.on_valid_epoch1_received(start);
    assert_eq!(
        t.on_time(start + GRACE_T_G - Duration::from_millis(1)),
        None
    );
    assert!(t.recv_accepts(0), "T_g dolmadan açık");
    assert_eq!(
        t.on_time(start + GRACE_T_G),
        Some(EpochAction::DiscardEpoch0Recv)
    );
    assert!(!t.recv_accepts(0));
    // Yeniden açılmaz.
    let _ = t.on_valid_epoch1_received(start + GRACE_T_G * 2);
    assert_eq!(t.on_time(start + GRACE_T_G * 3), None);
    assert!(!t.recv_accepts(0));
}

// ── Entegrasyon: transition sonrası PN sayacı DEVAM eder (§4 EPOCH-PN-INIT, 2026-10-06) ──
#[test]
fn epoch1_pn_continues_after_transition() {
    let mut a = KamaNoiseWireProtector::new(0x9, &key(0xAA), &key(0xBB));
    let mut b = KamaNoiseWireProtector::new(0x9, &key(0xBB), &key(0xAA));
    // Machine adım-2/3 → epoch-1 kur (engine InstallEpoch1AndSend'i uygular).
    a.install_epoch1_keys(&key(0xCC), &key(0xDD));
    b.install_epoch1_keys(&key(0xDD), &key(0xCC));
    let args = |wire_pn, dh_epoch| SealArgs {
        wire_pn,
        dh_epoch,
        send_generation: 0,
    };
    // epoch-0'da pn=0 ve pn=5 gönderilir, Bob ikisini de açar.
    for pn in [0u64, 5] {
        let s = a.seal(&fec(), args(pn, 0)).unwrap();
        assert!(b.open(&s.bytes).is_ok());
    }
    assert_eq!(b.recv_expected_pn(), 6);

    // Gönderen: epoch-1'de PN 0 mühürlenemez (tek sayaç, monotonluk epoch'tan bağımsız).
    assert!(
        a.seal(&fec(), args(0, 1)).is_err(),
        "sayaç epoch'ta sıfırlandı"
    );
    // epoch-1 PN 6 ile devam eder ve kabul edilir.
    let s = a.seal(&fec(), args(6, 1)).unwrap();
    assert!(b.open(&s.bytes).is_ok(), "epoch-1 pn=6 kabul");
    assert_eq!(b.recv_expected_pn(), 7, "tek pencere");

    // Alıcı: epoch-1 anahtarıyla mühürlenmiş PN 0 (aynı anahtarlı, state'i taze ikinci bir gönderen
    // üretir) reddedilir — PN 0 tek pencerede zaten görülmüştür. Pencere ilerlemez.
    let mut stale = KamaNoiseWireProtector::new(0x9, &key(0xAA), &key(0xBB));
    stale.install_epoch1_keys(&key(0xCC), &key(0xDD));
    let old = stale.seal(&fec(), args(0, 1)).unwrap();
    assert!(b.open(&old.bytes).is_err(), "epoch-1 pn=0 kabul edildi");
    assert_eq!(b.recv_expected_pn(), 7);
}
