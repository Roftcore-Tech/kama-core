//! DR-FEC-FS-1 FS-HARNESS-A — F0 CLOSED-LOOP sanity pins (gerçek Sender FEC-authority + GERÇEK RsSimdCodec).
//! Harness çekirdeği paylaşılan modülde (common/fs_f0.rs; fs_runner CLI de #[path]-include eder).

#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_f0.rs"]
mod fs_f0;
use adaptive_transport::fs_harness::{fs_sealed_len, FS_SHARD_FRAMING_BYTES};
use fs_f0::{run_f0, MS};

/// F0_LOOPBACK: loss-yok → block-tamamlanır; parity substrate'i BOZMAZ; cold-start-sızmaz.
#[test]
fn f0_loopback() {
    let r = run_f0(2, &[], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8);
    assert!(r.completion_ns.is_some());
    assert!(r.reconstruct_byte_equal);
    assert!(r.post_timeout_notice_none, "cold-start sızmamalı");
    assert!(
        r.estimator_pure,
        "loopback'te RECOVERED-yok → estimator-saf"
    );
    assert!(r.instrument.overlap_ok && r.instrument.faithful);
}

/// F0_EXACT_D1_R1 + F0_ENCODER_REAL: 1-erased + 1-real-repair → GERÇEK reconstruct == original.
#[test]
fn f0_exact_d1_r1_real_decode_byte_equal() {
    let r = run_f0(1, &[3], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8);
    assert_eq!(r.fec_decoded_sources, 1);
    assert!(r.reconstruct_byte_equal, "reconstructed != original");
    assert!(r.post_timeout_notice_none);
    assert!(
        r.estimator_pure,
        "RECOVERED estimator'a-dokunmamalı (fake-ACK-YOK)"
    );
    // t_repair_usable (reconstruct-complete) parity-arrival'dan SONRA (codec-latency causal-path'te).
    let (usable, parr) = (r.timing.t_repair_usable, r.timing.t_parity_arrival);
    assert!(usable.is_some() && parr.is_some());
    assert!(
        usable.unwrap() >= parr.unwrap(),
        "t_repair_usable parity-arrival'dan-önce olamaz (codec-latency)"
    );
}

/// F0_EXACT_D2_R2: 2-erasure + 2-repair → GERÇEK 2-source reconstruct == original.
#[test]
fn f0_exact_d2_r2_two_erasure_real_decode() {
    let r = run_f0(2, &[2, 5], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8);
    assert_eq!(r.fec_decoded_sources, 2);
    assert!(r.reconstruct_byte_equal);
    assert!(r.estimator_pure);
}

/// F0_EXACT_D2_R1 (HYBRID): FEC-yetersiz → residual-ARQ tamamlar (ARQ-KAPALI-DEĞİL).
#[test]
fn f0_exact_d2_r1_insufficient_fec_residual_arq() {
    let r = run_f0(1, &[2, 5], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8, "hybrid: FEC-yetersiz olsa da ARQ tamamlar");
    assert!(r.retransmits > 0, "residual-ARQ çalışmalı");
    assert!(r.reconstruct_byte_equal);
    assert!(
        r.timing.t_arq_wire.is_some(),
        "ARQ-lifecycle timestamp'i dolmalı"
    );
}

/// F0_PURITY: manual-R sabit; controller/PathTiming etkisi YOK (parity-count == manual_r).
#[test]
fn f0_purity_fixed_r_no_controller() {
    for &rr in &[1u16, 2, 4] {
        let r = run_f0(rr, &[], 1_000_000, 20 * MS);
        assert_eq!(
            r.parity_emitted,
            u64::from(rr),
            "manual-R sabit değil (controller-sızdı?)"
        );
    }
}

/// F0_RECOVERED_SCOPE + ESTIMATOR-PURITY (6. pin): decode→RECOVERED estimator'a-dokunmaz; notice-yok.
#[test]
fn f0_recovered_scope_estimator_purity() {
    let r = run_f0(2, &[2, 5], 1_000_000, 20 * MS);
    assert!(r.fec_decoded_sources > 0, "RECOVERED-yolu tetiklenmeli");
    assert!(
        r.estimator_pure,
        "RECOVERED RTT/BtlBw'yi değiştirdi (fake-ACK ihlali)"
    );
    assert!(
        r.post_timeout_notice_none,
        "steady-F0 → PostTimeoutNotice YOK"
    );
}

/// F0 WIRE-ACCOUNTING: redundancy payload/framing/sealed tutarlı (A1-dup ile AYNI-domain fs_sealed_len;
/// GROUNDED 26+11+E=101@E=64 — eski uydurma 108 DEĞİL; C-P4 realized-wire source==repair).
#[test]
fn f0_wire_accounting_consistent() {
    let r = run_f0(2, &[], 1_000_000, 20 * MS);
    let sealed = fs_sealed_len(64); // 101 grounded (source==repair; protector.rs N7)
    assert_eq!(r.realized_sealed_wire_bytes, 2 * sealed, "2 parity × 101");
    assert_eq!(r.redundancy_payload_bytes, 2 * 64);
    assert_eq!(r.redundancy_framing_bytes, 2 * FS_SHARD_FRAMING_BYTES);
    assert_eq!(
        r.redundancy_payload_bytes + r.redundancy_framing_bytes,
        r.realized_sealed_wire_bytes
    );
}
