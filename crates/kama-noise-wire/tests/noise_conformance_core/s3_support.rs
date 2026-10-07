//! A4.4 satırlarının ortak fikstürü: kurulu bir uç çifti ve küçük yardımcılar.

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s3::{ProtectionContext, SessionCommand, SessionControl, SessionResult};

use crate::mock_transport::MockTransport;
use crate::target::{ProtectionTarget, S3Target};

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

/// `Initiate` çıktısı [s3 v1.1]: bekleyen oturumun tutamacı + zarf türü + byte'lar.
pub fn initiate_session<T: S3Target>(
    a: &mut T::Crypto,
    peer_id: u64,
) -> (SessionHandle, u8, Vec<u8>) {
    match a.command(SessionCommand::Initiate {
        peer: peer(peer_id),
        early_unit: None,
    }) {
        SessionResult::Initiated {
            session,
            envelope_type,
            bytes,
        } => (session, envelope_type, bytes),
        other => panic!("Initiate bekleyen oturum + INIT zarfı üretmeli, gelen {other:?}"),
    }
}

/// `Initiate` çıktısı: zarf türü + byte'lar.
pub fn initiate<T: S3Target>(a: &mut T::Crypto, peer_id: u64) -> (u8, Vec<u8>) {
    let (_, envelope_type, bytes) = initiate_session::<T>(a, peer_id);
    (envelope_type, bytes)
}

/// Kurulu çift — hedefin kendi fikstürüyle ([`ProtectionTarget::established`]). Tutamaç iki uçta
/// da aynı oturumu adlandırır.
pub struct Pair<T: ProtectionTarget> {
    pub a: T::Crypto,
    pub b: T::Crypto,
    pub h: SessionHandle,
}

pub const PEER: u64 = 0x00A1_1CE0;
/// Başlatan ucun (Alice) kimliği — `PEER`'den (Bob) farklı [s3 v1.1].
pub const INITIATOR: u64 = 0x0011_7A70;

pub fn established<T: ProtectionTarget>() -> Pair<T> {
    let (a, b, h) = T::established();
    Pair { a, b, h }
}

/// `SessionControl` el sıkışmasıyla kurulu çift: Alice `Initiate` → (Transport) → Bob
/// `AcceptInit` → (Transport) → Alice `AcceptResponse` (kontrol ailesi olan hedefin fikstürü).
///
/// [s3 v1.1] İki iddia daha: Bob'un `EstablishedAndReply.peer`'i Alice'in kimliğidir ve Alice'in
/// `Established` tutamacı `Initiated`'da aldığı bekleyen tutamaçtır.
pub fn handshake<T: S3Target>() -> (T::Crypto, T::Crypto, SessionHandle) {
    let mut a = T::build_as(peer(INITIATOR)).expect("hedef kayıtlı olmalı");
    let mut b = T::build_as(peer(PEER)).expect("hedef kayıtlı olmalı");
    let mut net = MockTransport::new();

    let (pending, ty, init) = initiate_session::<T>(&mut a, PEER);
    net.submit(ty, &init).expect("INIT yola çıkar");
    let cmd = next_command(&mut net);
    let (h, rty, resp) = match b.command(cmd) {
        SessionResult::EstablishedAndReply {
            session,
            peer: initiator,
            envelope_type,
            bytes,
        } => {
            assert_eq!(initiator, peer(INITIATOR), "yanıtlayan eşi yanlış öğrendi");
            (session, envelope_type, bytes)
        }
        other => panic!("AcceptInit oturum + RESPONSE üretmeli, gelen {other:?}"),
    };
    net.submit(rty, &resp).expect("RESPONSE yola çıkar");
    let cmd = next_command(&mut net);
    assert_eq!(a.command(cmd), SessionResult::Established { session: h });
    assert_eq!(
        pending, h,
        "Established, Initiated'daki bekleyen tutamaçla gelmeli"
    );
    // Kurulum olaylarını boşalt: satırlar yalnız kendi ürettikleri olaylara bakar.
    while a.poll_event().is_some() {}
    while b.poll_event().is_some() {}
    (a, b, h)
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
