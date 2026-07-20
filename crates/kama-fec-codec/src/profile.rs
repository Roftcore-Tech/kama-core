//! Enumerated FEC profilleri (fec-core §6). Immutable-tuple: (code_suite_id, K_shape, R_code_max,
//! E, partial_policy, packet_schedule_policy). K_shape∈{2,4,8,16,..}, K_shape≤255, E≥11 even
//! (mümkünse 64-multiple — reed-solomon-simd). İhlal → profile-reject fail-closed (B4-invariant §8).

use crate::error::FecError;

/// Alpha tek code_suite_id (fec-core §5; RS/GF(2^8)/reed-solomon-simd).
pub const CODE_SUITE_RS_SIMD_V1: u8 = 0x01;

/// Immutable FEC profili — wire-code-shape (§4 R-üçlü: K_shape/R_code_max/E immutable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecProfile {
    pub profile_id: u8,
    pub code_suite_id: u8,
    /// Sabit source-shard sayısı (Model-A: K_actual ≤ K_shape; zero-slot pad ile K_shape'e tamamlanır).
    pub k_shape: u16,
    /// Wire-code-shape repair üst-sınırı (immutable; on-demand-repair bunun içindeki gönderilmemiş ESI).
    pub r_code_max: u16,
    /// Symbol-byte uzunluğu (her shard TAM E; §2.6 symbol_len-KİLİDİ).
    pub e: u16,
}

impl FecProfile {
    /// B4-invariant + reed-solomon-simd shard-alignment kontrolü.
    ///
    /// # Errors
    /// [`FecError::ProfileBound`] — K_shape/E/R sınır ihlali.
    pub fn validate(&self) -> Result<(), FecError> {
        if self.k_shape < 1 || self.k_shape > 255 {
            return Err(FecError::ProfileBound("1 <= K_shape <= 255"));
        }
        if self.e < 11 {
            return Err(FecError::ProfileBound("E >= 11 (prefix sığar)"));
        }
        if !self.e.is_multiple_of(2) {
            return Err(FecError::ProfileBound("E even-byte (reed-solomon-simd)"));
        }
        if self.r_code_max < 1 {
            return Err(FecError::ProfileBound("R_code_max >= 1"));
        }
        Ok(())
    }
    #[must_use]
    pub const fn k_shape_usize(&self) -> usize {
        self.k_shape as usize
    }
    #[must_use]
    pub const fn e_usize(&self) -> usize {
        self.e as usize
    }
}

/// Alpha enumerated-profile katalog (E=64 → 64-multiple, reed-solomon-simd-ideal).
pub const PROFILES: &[FecProfile] = &[
    FecProfile {
        profile_id: 0x01,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 2,
        r_code_max: 2,
        e: 64,
    },
    FecProfile {
        profile_id: 0x02,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 4,
        r_code_max: 4,
        e: 64,
    },
    FecProfile {
        profile_id: 0x03,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 8,
        r_code_max: 4,
        e: 64,
    },
    FecProfile {
        profile_id: 0x04,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 16,
        r_code_max: 8,
        e: 64,
    },
    // Büyük-E profilleri (Sprint-3; sub-kbps/M3 — sabit-overhead 37B → %3.5@E=1024 vs %36.6@E=64).
    // E=1024=16×64 (64-multiple; sealed FEC-shard = 26+11+1024 = 1061B < 1280 PMTU-tabanı).
    FecProfile {
        profile_id: 0x05,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 8,
        r_code_max: 4,
        e: 1024,
    },
    FecProfile {
        profile_id: 0x06,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 16,
        r_code_max: 8,
        e: 1024,
    },
];

/// profile_id → katalog profili (allowlist; bilinmeyen → None).
#[must_use]
pub fn by_id(profile_id: u8) -> Option<FecProfile> {
    PROFILES
        .iter()
        .copied()
        .find(|p| p.profile_id == profile_id)
}
