//! A4.4 satırlarının ortak fikstürü: kurulu bir uç çifti ve küçük yardımcılar.

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s3::{ProtectionContext, SessionCommand, SessionControl, SessionResult};

use crate::mock_transport::MockTransport;
use crate::target::S3Target;

/// Test eşi: `dest_hash`'in ilk sekiz baytı `n` (big-endian), kalanı sıfır. Sahte Crypto oturum
/// tutamacını bu sekiz bayttan türetir; farklı `n` farklı eş ve farklı oturum demektir.
pub fn peer(n: u64) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[..8].copy_from_slice(&n.to_be_bytes());
    PeerRef::from_dest_hash(dest_hash)
}

/// Sahte Transport'tan sıradaki zarfı Crypto'ya enjekte edilecek komut olarak alır.
pub fn next_command(net: &mut MockTransport) -> SessionCommand {
    net.deliver()
        .expect("zarf ulaşır")
        .expect("tanımlı zarf türü")
}

/// `Initiate` çıktısı: zarf türü + byte'lar.
pub fn initiate<T: S3Target>(a: &mut T::Crypto, peer_id: u64) -> (u8, Vec<u8>) {
    match a.command(SessionCommand::Initiate {
        peer: peer(peer_id),
        early_unit: None,
    }) {
        SessionResult::OutboundEnvelope {
            envelope_type,
            bytes,
        } => (envelope_type, bytes),
        other => panic!("Initiate bir INIT zarfı üretmeli, gelen {other:?}"),
    }
}

/// Kurulu çift: Alice `Initiate` → (Transport) → Bob `AcceptInit` → (Transport) → Alice
/// `AcceptResponse`. Dönen tutamaç iki uçta da aynı oturumu adlandırır.
pub struct Pair<T: S3Target> {
    pub a: T::Crypto,
    pub b: T::Crypto,
    pub h: SessionHandle,
}

pub const PEER: u64 = 0x00A1_1CE0;

pub fn established<T: S3Target>() -> Pair<T> {
    let mut a = T::build().expect("hedef kayıtlı olmalı");
    let mut b = T::build().expect("hedef kayıtlı olmalı");
    let mut net = MockTransport::new();

    let (ty, init) = initiate::<T>(&mut a, PEER);
    net.submit(ty, &init).expect("INIT yola çıkar");
    let cmd = next_command(&mut net);
    let (h, rty, resp) = match b.command(cmd) {
        SessionResult::EstablishedAndReply {
            session,
            envelope_type,
            bytes,
        } => (session, envelope_type, bytes),
        other => panic!("AcceptInit oturum + RESPONSE üretmeli, gelen {other:?}"),
    };
    net.submit(rty, &resp).expect("RESPONSE yola çıkar");
    let cmd = next_command(&mut net);
    assert_eq!(a.command(cmd), SessionResult::Established { session: h });
    // Kurulum olaylarını boşalt: satırlar yalnız kendi ürettikleri olaylara bakar.
    while a.poll_event().is_some() {}
    while b.poll_event().is_some() {}
    Pair { a, b, h }
}

pub fn session(h: SessionHandle) -> ProtectionContext {
    ProtectionContext::Session(h)
}

/// Son byte'ın en düşük biti çevrilmiş kopya (tag bozulur).
pub fn tamper(wire: &[u8]) -> Vec<u8> {
    let mut w = wire.to_vec();
    let last = w.len() - 1;
    w[last] ^= 1;
    w
}
