//! KATMAN-4 full-wire-golden (K-WIRE-NONCE96-1): sabit-anahtar + sabit-PN + sabit-inner → PINNED
//! tam-datagram. ChaCha20-Poly1305 deterministik; nonce96=LE64 (§4.5, DR-4-CLOSED). Drift-gate.
//! Değişirse = kasıtlı-onaylı-güncelleme (wire-format veya nonce96-mapping değişikliği).
//! NOT: bu self-golden TEK-ORACLE-DEĞİL — primitive-doğruluğu KATMAN-1 (rfc8439_vectors), mapping
//! KATMAN-2 (nonce96_vectors), impl-bağımsızlık KATMAN-3 (aead_differential) ile ayrıca-kanıtlanır.

use kama_noise_wire::inner::InnerPlain;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

// PINNED (nonce96=LE64, §4.5): h_ext=0x01020304, send_key=0x11×32, recv_key=0x22×32, pn=5, epoch=0,
// inner = FEC_SOURCE profil=0x03 block=7 esi=3 k_actual=8 symbol=0..64.
// Outer (ilk 10B) nonce-bağımsız: 01020304 01 00 00000005; ct+tag LE64-nonce'a bağlı.
const GOLDEN_DATAGRAM: &str = "01020304010000000005d1b4032624c6951f19bd7a6c92b5f2fbeb702d75e01ad27b14ab190ff7dbc78d95b54a16e4284d121d5a783a8125861f55a2ed35a39aa7f999f3e6ca99eb30cfa404d94daa2f7c09bea2a897be0f6b566d3be7c14711b09bea2662";

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
            assert_eq!(&symbol[..], &(0..64u8).collect::<Vec<_>>()[..]);
        }
        InnerPlain::Control { .. } => panic!("FEC beklendi"),
    }
}
