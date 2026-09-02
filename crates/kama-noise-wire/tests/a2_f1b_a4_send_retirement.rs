//! A2-F1-B — A-4 `drop_send_state()` KABUL-PİNLERİ (Abkarada 5-pin listesi aynen):
//! (1) drop-sonrası seal → SendStateRetired · (2) recv-open → devam-eder (ReplayWindow-sürekliliğiyle) ·
//! (3) ikinci drop → idempotent · (4) send-secret-material → zeroized (yapısal-proxy: her-iki-epoch
//! seal-yolu kapalı; cipher-drop = crate anahtar-yaşamı-güvencesi) · (5) renewal-üzerinden sessiz
//! reactivation → YOK (install_epoch1_keys-sonrası seal HÂLÂ SendStateRetired — bayrak tek-yön).
//!
//! Yapısal-lifecycle-pini: `drop_send_state` pub(crate) → dış-yol yalnız `Session::begin_close`
//! (postcondition-1 "yalnız Closing altında" derleme-düzeyinde).

#![cfg(feature = "experimental-cold-start")]

use kama_noise_wire::{
    Direction, KamaNoiseWireProtector, SealArgs, Session, SessionId, SessionLifecycle, WireError,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(b: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([b; 32])
}

fn paired(h_ext: u32) -> (KamaNoiseWireProtector, KamaNoiseWireProtector) {
    let a = KamaNoiseWireProtector::new(h_ext, &key(0xAA), &key(0xBB));
    let b = KamaNoiseWireProtector::new(h_ext, &key(0xBB), &key(0xAA));
    (a, b)
}

fn fec_packet(block_id: u32, esi: u16) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id,
            esi,
            k_actual: 8,
        },
        symbol: vec![0x5A; 64],
    }
}

fn args(pn: u64, epoch: u8) -> SealArgs {
    SealArgs {
        wire_pn: pn,
        dh_epoch: epoch,
        send_generation: 0,
    }
}

/// Pin-1 + Pin-2 + recv-ReplayWindow-sürekliliği: retire-ÖNCESİ pn=0 açılır; retire; seal→typed;
/// retire-SONRASI peer'den pn=1 AÇILMAYA-DEVAM (pencere-state'i byte-for-byte korunmuş → süreklilik).
#[test]
fn a4_pin1_seal_retired_and_pin2_recv_continues() {
    let (alice, mut bob) = paired(0x1111_2222);
    let mut sess = Session::new(SessionId(1), Direction::Responder, alice);

    // Retire-öncesi: bob→alice pn=0 açılır (pencere ilerler).
    let s0 = bob.seal(&fec_packet(1, 0), args(0, 0)).unwrap();
    sess.protector_mut().open(&s0.bytes).unwrap();

    // İade-zinciri adım-(1)+(2b): Closing + send-retirement.
    sess.begin_close();
    assert_eq!(sess.lifecycle(), SessionLifecycle::Closing);

    // Pin-1: drop-sonrası seal → typed SendStateRetired (epoch-0).
    assert_eq!(
        sess.seal(&fec_packet(1, 1), args(1, 0)).unwrap_err(),
        WireError::SendStateRetired(0x1111_2222)
    );

    // Pin-2: recv-open DEVAM + pencere-sürekliliği (pn=1; retire recv-state'e dokunmadı).
    let s1 = bob.seal(&fec_packet(1, 1), args(1, 0)).unwrap();
    let opened = sess.protector_mut().open(&s1.bytes).unwrap();
    assert_eq!(opened.full_pn(), 1);
    // Replay hâlâ reddedilir (pencere korunmuş — sıfırlanmış olsaydı replay geçerdi):
    assert!(sess.protector_mut().open(&s0.bytes).is_err());
}

/// Pin-3: ikinci begin_close/drop → İDEMPOTENT (state aynı; seal aynı-typed; recv aynı-devam).
#[test]
fn a4_pin3_second_drop_idempotent() {
    let (alice, mut bob) = paired(0x3333_4444);
    let mut sess = Session::new(SessionId(2), Direction::Responder, alice);
    sess.begin_close();
    sess.begin_close(); // idempotent
    assert_eq!(sess.lifecycle(), SessionLifecycle::Closing);
    assert_eq!(
        sess.seal(&fec_packet(1, 0), args(0, 0)).unwrap_err(),
        WireError::SendStateRetired(0x3333_4444)
    );
    let s = bob.seal(&fec_packet(1, 0), args(0, 0)).unwrap();
    assert!(sess.protector_mut().open(&s.bytes).is_ok());
}

/// Pin-4 (yapısal-proxy) + Pin-5: install_epoch1_keys SONRASI bile seal her-iki-epoch'ta
/// SendStateRetired — bayrak tek-yön; renewal-anahtar-kurulumu send'i SESSİZCE-reaktive-EDEMEZ.
/// (Send-secret'ların fiziksel-zeroize'ı cipher-drop'un crate-güvencesi; yapısal-kanıt = hiçbir
/// seal-yolu kalmadı + bayrağı temizleyen API YOK.)
#[test]
fn a4_pin4_pin5_no_silent_reactivation_via_renewal() {
    let (alice, _bob) = paired(0x5555_6666);
    let mut sess = Session::new(SessionId(3), Direction::Responder, alice);
    sess.begin_close();

    // Renewal-benzeri anahtar-kurulumu (epoch-1): send-yarısını REAKTİVE EDEMEMELİ.
    sess.protector_mut()
        .install_epoch1_keys(&key(0xC1), &key(0xC2));

    assert_eq!(
        sess.seal(&fec_packet(2, 0), args(0, 1)).unwrap_err(),
        WireError::SendStateRetired(0x5555_6666),
        "epoch-1 seal reaktive-oldu — A-4 tek-yön-bayrak ihlali"
    );
    assert_eq!(
        sess.seal(&fec_packet(2, 0), args(0, 0)).unwrap_err(),
        WireError::SendStateRetired(0x5555_6666),
        "epoch-0 seal reaktive-oldu"
    );
}

/// Closing ≠ Closed sınırı: Closing'de open-yolu Session-katmanında da açık kalmalı
/// (SessionManager.open yalnız Closed'u reddeder — grace-recv sözleşmesi).
#[test]
fn a4_closing_is_not_closed() {
    let (alice, _bob) = paired(0x7777_8888);
    let mut sess = Session::new(SessionId(4), Direction::Responder, alice);
    sess.begin_close();
    assert_ne!(sess.lifecycle(), SessionLifecycle::Closed);
    sess.close(); // grace-sonu: Closed'a geçiş hâlâ mümkün
    assert_eq!(sess.lifecycle(), SessionLifecycle::Closed);
}
