//! **noise conformance-core** — S3 sözleşmesinin gözlenebilir davranışı (KM-SEAM-TC-001 §6).
//!
//! Düzen S1'deki `seam_conformance_core` kalıbıyla aynıdır (R-7(c)): satır gövdeleri hedef-agnostik
//! generic `fn`'lerdir (`demux.rs`), makro onları hedef başına ayrı `#[test]` olarak sarar. Sahte
//! ile gerçek için "ayrı ama benzer" test yoktur — aynı koddur.
//!
//! # Hedefler [2026-10-08, L-A5b]
//!
//! `mock` iki aileyi de (PacketProtection + SessionControl) koşar. `real` = GERÇEK
//! `NoiseWireProtection`, iki zarf profiliyle (`real_s3_legacy` 22 B, `real_s3_extended` 26 B) ve
//! SSD1 vektörleri (`real_ssd1`); bugün YALNIZ PacketProtection ailesi. SessionControl ailesi
//! (H-RETRY, H-HANDLE, H-DEMUX, H-NO-MUTATE'in kontrol yarısı) gerçek hedefte BEYAN EDİLMİŞ
//! boşluktur ([`target::GAP_REAL_SESSION_CONTROL`]) — A5.5 (L-A5c) indiğinde kaydedilir.
//!
//! Koşucu: `scripts/ci/noise-conformance.sh --target mock|real` (`real_` önekli testler).

#[macro_use]
mod demux;
#[macro_use]
mod epoch;
#[path = "../common/mock_transport.rs"]
mod mock_transport;
mod s3_ctx;
mod s3_fec_order;
mod s3_handle;
mod s3_no_mutate;
mod s3_pn_own;
mod s3_replay_poison;
mod s3_retry;
mod s3_support;
mod ssd1_vectors;
mod target;

noise_demux_suite!(mock_demux, crate::target::MockTarget);

// A3.2 — hedef MEVCUT `epoch_machine.rs` (+ test tutkalı); mock/real ayrımının dışındadır çünkü
// `MockCrypto` epoch geçişini modellemez. A5.4'te aynı hücreler gerçek oturuma (`real_epoch`) koşar.
noise_epoch_suite!(machine_epoch;
    cell_1_response_normal,
    cell_2_response_lost,
    cell_3_response_reordered,
    cell_4_invalid_epoch1_is_inert,
    cell_5_count_closes_grace,
    cell_5b_n_g_is_fixed,
    cell_6_time_closes_grace,
    cell_7_whichever_first,
    cell_8_commit_goes_to_the_single_window,
    cell_9_at_most_two_key_trials,
);

/// `real` hedefinin SessionControl ailesi BEYAN EDİLMİŞ bir boşluktur (L-A5c), sessiz atlama değil:
/// gerçek hedef bugün yalnız PacketProtection ailesiyle kayıtlıdır. Kontrol ailesi gerçek hedefe
/// kaydedildiği an bu sınama kırmızıya döner — kapsam sessizce değişemez. (Ad `real_` içerir:
/// `--target real` koşusu boşluğu da listeler.)
#[test]
fn session_control_on_real_is_a_declared_gap() {
    use target::{Family, TargetKind, GAP_REAL_SESSION_CONTROL};
    let real: Vec<_> = target::REGISTERED_TARGETS
        .iter()
        .filter(|r| r.kind == TargetKind::Real)
        .collect();
    assert!(!real.is_empty(), "gerçek hedef kayıtlı olmalı (L-A5b)");
    assert!(
        real.iter().all(|r| r.family == Family::PacketProtection),
        "gerçek hedefe SessionControl ailesi kaydedildi: {GAP_REAL_SESSION_CONTROL} kapandı — bu \
         sınamayı kaldır, real_* kontrol süitlerini ve demux'u ekle"
    );
}

// ── A4.4 — yedi S3 sözleşme satırı (KM-SEAM-TC-001 §6) ──────────────────────────────────────

/// PacketProtection satırları — her hedefte (`ProtectionTarget`).
macro_rules! noise_protection_rows {
    ($target:ty) => {
        #[test]
        fn h_ctx_reaches_seam_unchanged() {
            crate::s3_ctx::ctx_reaches_seam_unchanged::<$target>();
        }
        #[test]
        fn h_pn_own_survives_transport_local_events() {
            crate::s3_pn_own::pn_survives_transport_local_events::<$target>();
        }
        #[test]
        fn h_pn_own_continues_across_epoch_transition() {
            crate::s3_pn_own::pn_continues_across_epoch_transition::<$target>();
        }
        #[test]
        fn h_fec_order_forged_shard_never_reaches_decoder() {
            crate::s3_fec_order::forged_shard_never_reaches_decoder::<$target>();
        }
        #[test]
        fn h_replay_poison_forged_packet_does_not_poison_the_window() {
            crate::s3_replay_poison::forged_packet_does_not_poison_the_window::<$target>();
        }
    };
}

/// Satır gövdelerini hedef başına ayrı `#[test]` olarak sarar.
macro_rules! noise_s3_suite {
    // Tam süit: PacketProtection + SessionControl satırları (`S3Target`).
    ($suite:ident, $target:ty) => {
        mod $suite {
            noise_protection_rows!($target);

            #[test]
            fn h_ctx_session_and_hub_seal_differently() {
                crate::s3_ctx::session_and_hub_seal_differently::<$target>();
            }
            #[test]
            fn h_retry_alice() {
                crate::s3_retry::alice_retransmits_cached_init_and_new_initiate_differs::<$target>(
                );
            }
            #[test]
            fn h_retry_bob() {
                crate::s3_retry::bob_maps_duplicate_init_to_same_session_and_cached_response::<
                    $target,
                >();
            }
            #[test]
            fn h_no_mutate_failures_leave_state_byte_identical() {
                crate::s3_no_mutate::failures_leave_state_byte_identical::<$target>();
            }
            #[test]
            fn h_handle_closed_handle_cannot_seal() {
                crate::s3_handle::closed_handle_cannot_seal::<$target>();
            }
            #[test]
            fn h_handle_pending_close_and_assign_handle() {
                crate::s3_handle::pending_close_and_assign_handle::<$target>();
            }
        }
    };
    // Yalnız PacketProtection ailesi; HubChannel no-op (gerçek hedef bugün — L-A5b).
    ($suite:ident, $target:ty, packet_only) => {
        mod $suite {
            noise_protection_rows!($target);

            #[test]
            fn h_ctx_hub_channel_is_identity_noop() {
                crate::s3_ctx::hub_channel_is_identity_noop::<$target>();
            }
            #[test]
            fn h_no_mutate_packet_failures_leave_state_byte_identical() {
                crate::s3_no_mutate::packet_failures_leave_state_byte_identical::<$target>();
            }
        }
    };
}

noise_s3_suite!(mock_s3, crate::target::MockTarget);
noise_s3_suite!(real_s3_legacy, crate::target::RealLegacyTarget, packet_only);
noise_s3_suite!(
    real_s3_extended,
    crate::target::RealExtendedTarget,
    packet_only
);

// PoC `IdentitySeam` hedefi (H-CTX'in bağlam-kaydı ve no-op yarısı) `client-core`'dadır:
// `crates/client-core/tests/noise_identity_target.rs`. Bu crate `client-core`'u göremez (katman
// kuralı C3). Koşucu: `scripts/ci/noise-conformance.sh --target identity`.

// ── SSD1 vektörleri (Python referansı) — aynı vaka gövdeleri, iki mühür yolu ─────────────────
// `vectors_ssd1`: bugünkü `LegacySessionProtector` (#90, G-S3 kanıtı; `--target vectors`).
// `real_ssd1`: GERÇEK S3 yolu `NoiseWireProtection` + legacy profil (L-A5b; `--target real`).
mod vectors_ssd1 {
    use crate::ssd1_vectors::LegacyPath as Path;

    #[test]
    fn seal_is_byte_identical_to_python_reference() {
        crate::ssd1_vectors::seal_is_byte_identical::<Path>();
    }
    #[test]
    fn open_matches_expected_decisions() {
        crate::ssd1_vectors::open_matches_expected_decisions::<Path>();
    }
    #[test]
    fn every_case_is_consumed() {
        crate::ssd1_vectors::every_case_is_consumed::<Path>();
    }
}

mod real_ssd1 {
    use crate::ssd1_vectors::RealPath as Path;

    #[test]
    fn seal_is_byte_identical_to_python_reference() {
        crate::ssd1_vectors::seal_is_byte_identical::<Path>();
    }
    #[test]
    fn open_matches_expected_decisions() {
        crate::ssd1_vectors::open_matches_expected_decisions::<Path>();
    }
    #[test]
    fn every_case_is_consumed() {
        crate::ssd1_vectors::every_case_is_consumed::<Path>();
    }
}
