//! Wire-golden (K-WIRE-1 emsali): sabit-anahtar + sabit-PN + sabit-inner → PINNED tam-datagram.
//! ChaCha20-Poly1305 RFC-8439-deterministik → byte-dizisi crate/impl-bağımsız; drift-gate.
//! Değişirse = kasıtlı-onaylı-güncelleme (wire-format veya nonce-kurgusu-DR-4 değişikliği).

use kama_noise_wire::inner::InnerPlain;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

// PINNED: h_ext=0x01020304, send_key=0x11×32, recv_key=0x22×32, pn=5, epoch=0,
// inner = FEC_SOURCE profil=0x03 block=7 esi=3 k_actual=8 symbol=0..64.
const GOLDEN_DATAGRAM: &str = "01020304010000000005aa1bf3a9995f1b730e3686964b50b26b6de0b981a1afd433755d02aad3860d784d40f8ba9e970397ceab2d2642414e8ad3e1a845e277bbac99b08db758bcef92aaa55f610c393fd86cbcb7b9b0b77b7d549e127335b5d2d39ed2cd";

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn golden_packet() -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: (0..64u8).collect(),
    }
}

#[test]
fn wire_golden_seal_bytes_pinned() {
    let mut a = KamaNoiseWireProtector::new(
        0x0102_0304,
        &Zeroizing::new([0x11u8; 32]),
        &Zeroizing::new([0x22u8; 32]),
    );
    let s = a
        .seal(
            &golden_packet(),
            SealArgs {
                wire_pn: 5,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert_eq!(s.bytes, hex(GOLDEN_DATAGRAM), "wire-golden drift-gate");
    assert_eq!(s.bytes.len(), 101, "sealed FEC-shard @E=64 = 26+11+64");
}

#[test]
fn wire_golden_opens_back() {
    // Golden-datagram'ı eşleşen-recv-anahtarlı protector açar (send=0x11 → recv=0x11).
    let mut b = KamaNoiseWireProtector::new(
        0x0102_0304,
        &Zeroizing::new([0x22u8; 32]),
        &Zeroizing::new([0x11u8; 32]),
    );
    let opened = b.open(&hex(GOLDEN_DATAGRAM)).unwrap();
    assert_eq!(opened.full_pn(), 5);
    assert_eq!(opened.dh_epoch(), 0);
    match opened.inner() {
        InnerPlain::FecShard { header, symbol } => {
            assert_eq!(header.block_id, 7);
            assert_eq!(header.esi, 3);
            assert_eq!(header.k_actual, 8);
            assert_eq!(header.profile_id, 0x03);
            assert_eq!(symbol, &(0..64u8).collect::<Vec<_>>());
        }
        InnerPlain::Control { .. } => panic!("FEC beklendi"),
    }
}
