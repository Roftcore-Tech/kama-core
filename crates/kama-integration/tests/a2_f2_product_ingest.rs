//! A2-F2 — GERÇEK PRODUCT RECEIVER VERTICAL SLICE (Abkarada §7.6 kabul):
//! gerçek seal → ReceiverHost.on_datagram (AEAD-open) → iki-seviye cold-admission → block-assembly →
//! K-decode → app-delivery → resource-release; ayrıca non-renewable patience-deadline timeout+release;
//! stateless probe/echo; duplicate/cap drop. Test-harness-mantığı DEĞİL — gerçek ReceiverHost.

#![cfg(feature = "experimental-cold-start")]

use kama_integration::{IngestOutcome, ReceiverHost};
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0000_0042;
const E: usize = 64;
const K: u8 = 8;

fn key() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0x11u8; 32])
}

/// Sender-protector (paired symmetric-mock; product'ta handshake-Split key'leri).
fn sender() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H_EXT, &key(), &key())
}

/// Bir source-shard'ı seal eder (app-chunk taşır) → wire datagram.
fn seal_source(
    s: &mut KamaNoiseWireProtector,
    block_id: u32,
    esi: u16,
    pn: u64,
    chunk: &[u8],
) -> Vec<u8> {
    let pfx = SourcePrefix {
        message_id: 1,
        chunk_offset: u32::from(esi) * 53,
        actual_payload_len: chunk.len() as u16,
        source_flags: u8::from(esi == u16::from(K) - 1),
    };
    let mut sym = vec![0u8; E];
    pfx.serialize_symbol(chunk, &mut sym).unwrap();
    let pkt = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id,
            esi,
            k_actual: K,
        },
        symbol: sym,
    };
    s.seal(
        &pkt,
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

/// §7.6 KABUL: shard ingest → ledger admission → block progress → completion → delivery → release.
#[test]
fn f2_end_to_end_block_delivery_and_release() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let now = 1_000_000_000u64;

    // K=8 source-shard: ilk 7 → ShardAdmitted; 8. → BlockDelivered.
    let mut last = IngestOutcome::Dropped("init");
    for esi in 0..u16::from(K) {
        let chunk = vec![0xA0u8 + esi as u8; 10];
        let dg = seal_source(&mut snd, 5, esi, u64::from(esi), &chunk);
        last = host.on_datagram(now, &dg).unwrap();
        if esi < u16::from(K) - 1 {
            assert_eq!(
                last,
                IngestOutcome::ShardAdmitted {
                    block_id: 5,
                    progress: (esi + 1) as u8
                }
            );
        }
    }
    // Son shard: decode + delivery.
    match last {
        IngestOutcome::BlockDelivered { block_id, app_len } => {
            assert_eq!(block_id, 5);
            assert_eq!(app_len, 8 * 10, "8 chunk × 10B app-payload");
        }
        other => panic!("BlockDelivered beklendi, {other:?}"),
    }
    assert_eq!(host.delivered_count(), 1);

    // RESOURCE-RELEASE: blok teslim → cold-defter iade (global+session sıfıra döndü).
    assert_eq!(
        host.global_cold().res.retained_bytes,
        0,
        "teslim sonrası global-cold sızdı"
    );
}

/// Patience: non-renewable per-block deadline; K'dan-az shard + tick(deadline-sonrası) → EVICT+release.
#[test]
fn f2_patience_deadline_evicts_incomplete_block() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let t0 = 1_000_000_000u64;

    // 3 shard (K=8'e ulaşmaz) → blok açık kalır, deadline = t0 + 75s.
    for esi in 0..3u16 {
        let dg = seal_source(&mut snd, 9, esi, u64::from(esi), &[esi as u8; 10]);
        assert!(matches!(
            host.on_datagram(t0, &dg).unwrap(),
            IngestOutcome::ShardAdmitted { .. }
        ));
    }
    assert!(
        host.global_cold().res.retained_bytes > 0,
        "3 shard rezerve edilmeli"
    );

    // Deadline-ÖNCESİ tick: evict YOK.
    let (ev0, _) = host.on_patience_tick(t0 + 70 * 1_000_000_000);
    assert_eq!(ev0, 0, "deadline öncesi evict oldu (non-renewable ihlali)");

    // Deadline-SONRASI tick: blok EVICT + release.
    let (ev1, _) = host.on_patience_tick(t0 + 76 * 1_000_000_000);
    assert_eq!(ev1, 1, "deadline sonrası blok evict edilmedi");
    assert_eq!(
        host.global_cold().res.retained_bytes,
        0,
        "evict sonrası cold-defter sızdı"
    );
    assert_eq!(
        host.delivered_count(),
        0,
        "incomplete blok teslim edilmemeli"
    );
}

/// Non-renewable: sonraki shard deadline'ı UZATMAZ (ilk-shard-anchor sabit).
#[test]
fn f2_deadline_non_renewable() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let t0 = 1_000_000_000u64;

    // İlk shard t0'da (deadline = t0+75s). İkinci shard ÇOK sonra (t0+70s) — deadline UZAMAMALI.
    let d0 = seal_source(&mut snd, 3, 0, 0, &[0u8; 10]);
    host.on_datagram(t0, &d0).unwrap();
    let d1 = seal_source(&mut snd, 3, 1, 1, &[1u8; 10]);
    host.on_datagram(t0 + 70 * 1_000_000_000, &d1).unwrap();

    // t0+76s: ilk-anchor deadline geçti → evict (ikinci-shard uzatmadıysa).
    let (ev, _) = host.on_patience_tick(t0 + 76 * 1_000_000_000);
    assert_eq!(
        ev, 1,
        "sonraki shard deadline'ı uzattı (non-renewable ihlali)"
    );
}

/// Duplicate shard → drop (zero-delta; retransmit).
#[test]
fn f2_duplicate_shard_dropped() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let now = 1_000_000_000u64;
    let dg0 = seal_source(&mut snd, 1, 0, 0, &[7u8; 10]);
    assert!(matches!(
        host.on_datagram(now, &dg0).unwrap(),
        IngestOutcome::ShardAdmitted { .. }
    ));
    // Aynı (block,esi) yeniden seal (yeni-PN; wire-farklı ama logical-aynı) → Duplicate drop.
    let dg1 = seal_source(&mut snd, 1, 0, 99, &[7u8; 10]);
    assert_eq!(
        host.on_datagram(now, &dg1).unwrap(),
        IngestOutcome::Dropped("duplicate shard")
    );
}

/// Bilinmeyen handle → drop (fixed-cost; unknown session).
#[test]
fn f2_unknown_handle_dropped() {
    let mut host = ReceiverHost::new();
    // Session kurulmadı → herhangi-datagram unknown-handle.
    let mut snd = sender();
    let dg = seal_source(&mut snd, 0, 0, 0, &[0u8; 10]);
    assert_eq!(
        host.on_datagram(0, &dg).unwrap(),
        IngestOutcome::Dropped("unknown handle")
    );
}

/// AEAD verify fail → drop (bozuk datagram; side-effect yok).
#[test]
fn f2_aead_fail_dropped() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let mut dg = seal_source(&mut snd, 0, 0, 0, &[0u8; 10]);
    let n = dg.len();
    dg[n - 1] ^= 0xFF; // tag boz
    assert_eq!(
        host.on_datagram(0, &dg).unwrap(),
        IngestOutcome::Dropped("aead verify failed")
    );
    assert_eq!(
        host.global_cold().res.retained_bytes,
        0,
        "aead-fail state değiştirdi"
    );
}
