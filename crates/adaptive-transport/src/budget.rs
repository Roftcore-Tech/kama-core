//! TV-EMERGENCY (§8.5): redundancy / rate budget clamp.
//!
//! `parity_final = min(fuzzy_parity, budget_cap, profile_cap)` — dar bantta yüksek loss
//! parity'yi PATLATAMAZ (budget_cap bağlar). `send_rate_final = clamp(fuzzy_rate,
//! [MIN_RATE, cap])` — MIN_RATE floor rate-düşüklüğünden tetiklenir, loss'tan DEĞİL
//! (INV-RATE-1: clamp yalnız bw+queue_delay'den gelen rate'e uygulanır). Sabitler [CAL]
//! fixture (netem'e).

/// §8.5 budget clamp yapılandırması. **[CAL] fixture** — gerçek değerler netem'den.
#[derive(Debug, Clone, Copy)]
pub struct BudgetConfig {
    /// Profil üst-sınırı (per-mille) — mutlak parity tavanı.
    pub profile_cap_permille: u32,
    /// MIN_RATE (bytes/sec) — send_rate alt-sınırı (path'i boğmadan minimal-reliable).
    pub min_rate_bps: u64,
    /// 1000‰ (tam) parity afford edilen referans BtlBw (budget_cap ölçeği).
    pub budget_ref_bps: u64,
}

impl BudgetConfig {
    /// **KALİBRE DEĞİL** fixture.
    #[must_use]
    pub fn fixture() -> Self {
        Self {
            profile_cap_permille: 900,
            min_rate_bps: 10_000,
            budget_ref_bps: 125_000,
        }
    }
}

/// available_redundancy_budget cap'i (per-mille) = min(profile_cap, BtlBw·1000/budget_ref)
/// — §8.5 `f(BtlBw)`. Dar bantta düşük → parity israfını engeller.
#[must_use]
pub fn budget_cap_permille(btlbw: u64, cfg: &BudgetConfig) -> u32 {
    let scaled = (u128::from(btlbw) * 1000 / u128::from(cfg.budget_ref_bps)) as u32;
    scaled.min(cfg.profile_cap_permille)
}

/// `parity_final = min(fuzzy_parity, budget_cap, profile_cap)` (§8.5/§5.5).
#[must_use]
pub fn parity_final(fuzzy_parity: u32, budget_cap: u32, profile_cap: u32) -> u32 {
    fuzzy_parity.min(budget_cap).min(profile_cap)
}

/// `send_rate_final = clamp(fuzzy_rate, [min_rate, cap])` (§8.5). `fuzzy_rate` bw+queue_delay'den
/// (loss-kör); clamp bunu değiştirmez → MIN_RATE floor loss'tan tetiklenmez. `min_rate ≤ cap`.
#[must_use]
pub fn send_rate_final(fuzzy_rate: u64, min_rate: u64, cap: u64) -> u64 {
    fuzzy_rate.clamp(min_rate, cap)
}
