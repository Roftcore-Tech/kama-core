//! **noise conformance-core** — S3 sözleşmesinin gözlenebilir davranışı (KM-SEAM-TC-001 §6).
//!
//! Düzen S1'deki `seam_conformance_core` kalıbıyla aynıdır (R-7(c)): satır gövdeleri hedef-agnostik
//! generic `fn`'lerdir (`demux.rs`), makro onları hedef başına ayrı `#[test]` olarak sarar. Sahte
//! ile gerçek için "ayrı ama benzer" test yoktur — aynı koddur.
//!
//! # Bugün tek hedef koşuyor
//!
//! `mock` kayıtlı; `real` DEĞİL ([`target::GAP_REAL_CRYPTO`] — gerçek kripto A5'te). `RealTarget`
//! derlenir ama `build()` typed "uygulanmadı" döner ve süite kaydedilmez. A5.5 indiğinde eklenecek
//! tek satır:
//!
//! ```ignore
//! noise_demux_suite!(real_demux, crate::target::RealTarget);
//! ```
//!
//! Koşucu: `scripts/ci/noise-conformance.sh --target mock|real`.

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

/// `real` hedefinin yokluğu BEYAN EDİLMİŞ bir boşluktur; sessiz atlama değil. Gerçek hedef
/// kaydedildiği an bu sınama kırmızıya döner — kapsam sessizce değişemez.
#[test]
fn missing_target_is_a_declared_gap() {
    use target::{S3Target, TargetKind, TargetUnavailable};
    assert!(
        !target::REGISTERED_TARGETS
            .iter()
            .any(|(_, k)| *k == TargetKind::Real),
        "gerçek hedef kaydedildi: bu sınamayı kaldır ve real_* süitlerini ekle"
    );
    assert!(matches!(
        target::RealTarget::build(),
        Err(TargetUnavailable::NotImplemented { gap }) if gap == target::GAP_REAL_CRYPTO
    ));
}

// ── A4.4 — yedi S3 sözleşme satırı (KM-SEAM-TC-001 §6) ──────────────────────────────────────

/// Satır gövdelerini hedef başına ayrı `#[test]` olarak sarar.
macro_rules! noise_s3_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            type Target = $target;

            #[test]
            fn h_ctx_reaches_seam_unchanged() {
                crate::s3_ctx::ctx_reaches_seam_unchanged::<Target>();
            }
            #[test]
            fn h_ctx_session_and_hub_seal_differently() {
                crate::s3_ctx::session_and_hub_seal_differently::<Target>();
            }
            #[test]
            fn h_pn_own_survives_transport_local_events() {
                crate::s3_pn_own::pn_survives_transport_local_events::<Target>();
            }
            #[test]
            fn h_pn_own_continues_across_epoch_transition() {
                crate::s3_pn_own::pn_continues_across_epoch_transition::<Target>();
            }
            #[test]
            fn h_fec_order_forged_shard_never_reaches_decoder() {
                crate::s3_fec_order::forged_shard_never_reaches_decoder::<Target>();
            }
            #[test]
            fn h_retry_alice() {
                crate::s3_retry::alice_retransmits_cached_init_and_new_initiate_differs::<Target>();
            }
            #[test]
            fn h_retry_bob() {
                crate::s3_retry::bob_maps_duplicate_init_to_same_session_and_cached_response::<
                    Target,
                >();
            }
            #[test]
            fn h_no_mutate_failures_leave_state_byte_identical() {
                crate::s3_no_mutate::failures_leave_state_byte_identical::<Target>();
            }
            #[test]
            fn h_handle_closed_handle_cannot_seal() {
                crate::s3_handle::closed_handle_cannot_seal::<Target>();
            }
            #[test]
            fn h_replay_poison_forged_packet_does_not_poison_the_window() {
                crate::s3_replay_poison::forged_packet_does_not_poison_the_window::<Target>();
            }
        }
    };
}

noise_s3_suite!(mock_s3, crate::target::MockTarget);

/// PoC `IdentitySeam` hedefi: H-CTX'in yalnız bağlam-kaydı ve no-op yarısı uygulanır
/// ("Session ≠ HubChannel" gerçek anahtarla, M4).
mod identity_s3 {
    #[test]
    fn h_ctx_identity_seam_is_noop_in_both_contexts() {
        crate::s3_ctx::identity_seam_is_noop_in_both_contexts();
    }
}
