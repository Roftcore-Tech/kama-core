//! A4.4 satırlarının ortak fikstürü: kurulu bir uç çifti ve küçük yardımcılar.

use client_core::seam::{
    PeerRef, ProtectionContext, SessionCommand, SessionControl, SessionHandle, SessionResult,
};

use crate::mock_transport::{InjectedCommand, MockTransport};
use crate::target::S3Target;

/// Sahte Transport'un verdiği komutu S3 komutuna çevirir (yerel ayna → gerçek tip).
pub fn to_command(c: InjectedCommand) -> SessionCommand {
    match c {
        InjectedCommand::AcceptInit { wire } => SessionCommand::AcceptInit { wire },
        InjectedCommand::AcceptResponse { wire } => SessionCommand::AcceptResponse { wire },
        other => panic!("zarf taşıyan komut bekleniyordu: {other:?}"),
    }
}

/// `Initiate` çıktısı: zarf türü + byte'lar.
pub fn initiate<T: S3Target>(a: &mut T::Crypto, peer: u64) -> (u8, Vec<u8>) {
    match a.command(SessionCommand::Initiate {
        peer: PeerRef::from_raw(peer),
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
    let cmd = to_command(
        net.deliver()
            .expect("INIT ulaşır")
            .expect("tanımlı zarf türü"),
    );
    let (h, rty, resp) = match b.command(cmd) {
        SessionResult::EstablishedAndReply {
            session,
            envelope_type,
            bytes,
        } => (session, envelope_type, bytes),
        other => panic!("AcceptInit oturum + RESPONSE üretmeli, gelen {other:?}"),
    };
    net.submit(rty, &resp).expect("RESPONSE yola çıkar");
    let cmd = to_command(
        net.deliver()
            .expect("RESPONSE ulaşır")
            .expect("tanımlı zarf türü"),
    );
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
