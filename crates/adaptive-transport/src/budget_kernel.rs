//! DR-1b-v2 SAF-POLİTİKA-KERNELİ (Commit-R) — **DENEYSEL; ÜRETİM-YOLUNA-BAĞLI-DEĞİL.**
//!
//! Bu-modül `plan_fec_parity`/pacer/scheduler'a KABLOLANMAMIŞTIR; hiçbir-üretim-yolu-çağırmaz
//! (yalnız-testler + Commit-S-adversarial-sim). Default-ürün-davranışı-DEĞİŞMEZ. Amaç: aday-token-
//! tahsis-politikalarını (imkânsızlık-üçgeni-sınıfları L/R/B — DR-1b-problem-formal.md) production-
//! scheduling'den-bağımsız, deterministik, integer-only, alloc'suz-çekirdekte-tanımlayıp-sınamak.
//!
//! SÖZLEŞMELER:
//! - **Ordering-first-invariant:** kernel wire-SIRAYA-karışmaz; yalnız-token-LİMİTLERİ üretir
//!   (DATA→ARQ→parity-emisyon-sırası çağıranın-mühürlü-düzeni).
//! - **Rezervasyon-adlandırması:** `ProtectedShare`/`FloorCap` R-sınıfı = BOUNDED-REZERVASYON
//!   (DATA korunan-payı-tüketemez; talep-yoksa-pay-DATA'ya-döner — work-conserving).
//!   `BoundedDebtFloor` B-sınıfı = KESİN-cap'li-borçlanma (gelecek-parity-bütçesinden).
//! - **Hard-cap-accounting = BOUNDED-CREDIT state-machine** (connection-lifetime-kümülatif-DEĞİL;
//!   idle-birikimi `credit_cap`-ile-sınırlı → uzun-duraklama-sonrası-burst-bounded):
//!   `credit = min(credit + share×rate×dt, cap); emit → credit -= size`.
//! - **Floor = KOŞULLU-servis-sözleşmesi:** yalnız ADMITTED-ticket, scheduler-OPPORTUNITY-sayısı +
//!   token-feasibility-koşullarıyla; wall-clock-SLA-İDDİASI-YOK (wakeup'sız-garanti-yazılmaz).
//!   Miss = sessiz-değil (çağıran `FloorMiss`-teşhisi-üretir; sim-kanıtlı).
//! - Deterministik · integer-only (saturating) · RNG/alloc/IO-YOK · state-geçişleri-kontrollü.

/// Politika-türü (aday-mimariler; SEÇİLMİŞ-politika-DEĞİL — DR-1b-v2-karşılaştırma-girdisi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind {
    /// Sınıf-L (baseline): parity-yalnız-gerçek-leftover; floor=best-effort-değil-YOK.
    LeftoverOnly,
    /// Sınıf-R: bounded-rezervasyon (entitlement-credit); floor-ticket-YOK (yalnız-pay).
    ProtectedShare,
    /// Sınıf-R + F2-floor-ticket-kuyruğu + C1-hard-cap-absolute (cap-asla-aşılmaz → miss-mümkün+teşhis).
    FloorCap,
    /// Sınıf-B + C2-dar-bounded-overdraft: floor-due-ticket max_debt'e-kadar-borçlanabilir;
    /// sonraki-entitlement-accrual-önce-borcu-kapatır (birikim-yok).
    BoundedDebtFloor,
}

/// Kernel-parametreleri (tümü [CAL]; sim-taramasında-sabitlenen-varsayılanlar new()-içinde).
#[derive(Debug, Clone, Copy)]
pub struct PolicyParams {
    /// Parity-entitlement-payı (‰ of accrual) — R/B-sınıfları.
    pub share_permille: u32,
    /// Entitlement-credit-tavanı (byte) — idle-birikim-sınırı.
    pub entitlement_cap_bytes: u64,
    /// Hard-cap-payı (‰ of accrual) — toplam-parity-üst-akış-sınırı (C1).
    pub hardcap_share_permille: u32,
    /// Hard-cap-credit-tavanı (byte).
    pub hardcap_cap_bytes: u64,
    /// Floor-borç-tavanı (byte) — yalnız-BoundedDebtFloor.
    pub max_debt_bytes: u64,
    /// T_floor = clamp(mult×srtt, min, max).max(serialization+granularity).
    pub floor_mult_permille: u32,
    pub t_floor_min_ns: u64,
    pub t_floor_max_ns: u64,
    /// Scheduler-granularity-önseli (T_floor-alt-sınır-bileşeni).
    pub sched_granularity_ns: u64,
    /// SRTT-yokken-önsel (cold-start).
    pub srtt_prior_ns: u64,
}

impl PolicyParams {
    /// Sim-varsayılanları [CAL]: share %20, entitlement-cap 4×1200, hardcap %30/8×1200,
    /// debt 2×1200, floor 2×srtt clamp [50ms, 10s], granularity 10ms, prior 300ms.
    #[must_use]
    pub fn defaults() -> Self {
        PolicyParams {
            share_permille: 200,
            entitlement_cap_bytes: 4 * 1200,
            hardcap_share_permille: 300,
            hardcap_cap_bytes: 8 * 1200,
            max_debt_bytes: 2 * 1200,
            floor_mult_permille: 2000,
            t_floor_min_ns: 50_000_000,
            t_floor_max_ns: 10_000_000_000,
            sched_granularity_ns: 10_000_000,
            srtt_prior_ns: 300_000_000,
        }
    }
}

/// Kernel-durumu (bounded; her-alan-üst-sınırlı — SR-AUDIT-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetPolicyState {
    /// R/B: parity-entitlement-credit (≤ entitlement_cap_bytes).
    pub entitlement_credit: u64,
    /// C1-hard-cap-credit (≤ hardcap_cap_bytes).
    pub hardcap_credit: u64,
    /// B: açık-borç (≤ max_debt_bytes; entitlement-accrual-önce-bunu-kapatır).
    pub debt_bytes: u64,
}

impl BudgetPolicyState {
    /// Mode-transition/rollback-semantiği: politika-değişiminde state-SIFIRLANIR
    /// (LeftoverOnly'ye-dönüş temiz; kalıntı-credit/debt-taşınmaz).
    pub fn reset(&mut self) {
        *self = BudgetPolicyState::default();
    }
}

/// Bir-scheduling-OPPORTUNITY bağlamı (kernel-girdisi; kaynak-saat=çağıranın-event-saati).
#[derive(Debug, Clone, Copy)]
pub struct BudgetContext {
    /// Bir-önceki-opportunity'den-geçen-süre (ns; event-clock — wall-clock-değil).
    pub elapsed_ns: u64,
    /// Pacing-rate (B/s) — accrual-kaynağı (BtlBw/delivered-DEĞİL; mühürlü-pacer'ın-rate'i).
    pub pacing_rate_bps: u64,
    /// Bu-opportunity'de-harcanabilir-token (pacer-bucket-durumu; burst-cap-dahil).
    pub available_tokens: u64,
    /// DATA-talebi (byte; bu-turda-emit-etmek-istediği).
    pub data_demand_bytes: u64,
    /// ARQ/retransmit-talebi (byte) — ordering'de-DATA'dan-sonra-parity'den-önce.
    pub arq_demand_bytes: u64,
    /// Bulk-parity-kuyruk-talebi (byte; floor-ticket'lar-HARİÇ).
    pub bulk_parity_demand_bytes: u64,
    /// Floor-ticket-kuyruğu-uzunluğu (admitted; FIFO — F2).
    pub floor_tickets: u32,
    /// En-eski-ticket-yaşı (ns; 0=ticket-yok).
    pub oldest_ticket_age_ns: u64,
    /// Parity-birim-boyutu (byte; E-türevi).
    pub parity_size: u64,
    /// SRTT (ns; 0 = henüz-yok → prior).
    pub srtt_ns: u64,
}

/// Kernel-kararı (token-LİMİTLERİ; sıra-değil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetDecision {
    /// DATA+ARQ-tarafının-bu-turda-görebileceği-token (ordering: DATA→ARQ çağıranda).
    pub data_token_limit: u64,
    /// Parity-tarafı-toplam-limit (floor+bulk; floor-önce — F2).
    pub parity_token_limit: u64,
    /// Bu-turda-servis-edilecek-floor-ticket-adedi (ticket-FIFO-önünden).
    pub floor_grants: u32,
    /// Hard-cap-bastırması-oldu-mu (C1: cap-aşılamaz → floor-miss-adayı; teşhis-zorunlu).
    pub cap_suppressed: bool,
    /// Bu-turda-İZİN-verilen-borç-tavanı (yalnız-B-sınıfı-due-floor; GERÇEK-borç settle'da-doğar).
    pub borrow_allowance_bytes: u64,
    /// Bu-tur-rezerve-edilen (entitlement'tan-korunan) byte — settle-muhasebesi-girdisi.
    pub reserved_bytes: u64,
    /// Sonraki-uyanma-önerisi (ns; en-yakın-ticket-deadline'ına; 0=öneri-yok).
    pub next_wakeup_hint_ns: u64,
}

/// Admission-feasibility-sınırları (çağıran-kuyruk-sahibi; kernel-saf-karar).
#[derive(Debug, Clone, Copy)]
pub struct AdmissionCaps {
    pub pending_items_cap: u32,
    pub pending_bytes_cap: u64,
    pub floor_ticket_cap: u32,
    pub per_block_admitted_cap: u16,
    pub parity_size_max: u64,
}

/// Typed-admission-sonucu (sessiz-red-yok).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionResult {
    AdmittedWithFloorTicket,
    AdmittedBulkOnly,
    RejectedItemsCap,
    RejectedBytesCap,
    RejectedTicketCap,
    RejectedPerBlockCap,
    RejectedSizeUnsupported,
    RejectedBlockMoot,
    RejectedDuplicateTicket,
}

/// Saf-admission-feasibility (talimat-§4.1-koşulları; blok-canlılığı/moot çağıran-sinyali).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn admit(
    caps: &AdmissionCaps,
    pending_items: u32,
    pending_bytes: u64,
    floor_tickets: u32,
    block_admitted: u16,
    parity_size: u64,
    block_alive: bool,
    block_has_ticket: bool,
    wants_floor_ticket: bool,
) -> AdmissionResult {
    if !block_alive {
        return AdmissionResult::RejectedBlockMoot;
    }
    if parity_size == 0 || parity_size > caps.parity_size_max {
        return AdmissionResult::RejectedSizeUnsupported;
    }
    if pending_items >= caps.pending_items_cap {
        return AdmissionResult::RejectedItemsCap;
    }
    if pending_bytes.saturating_add(parity_size) > caps.pending_bytes_cap {
        return AdmissionResult::RejectedBytesCap;
    }
    if block_admitted >= caps.per_block_admitted_cap {
        return AdmissionResult::RejectedPerBlockCap;
    }
    if wants_floor_ticket {
        if block_has_ticket {
            return AdmissionResult::RejectedDuplicateTicket;
        }
        if floor_tickets >= caps.floor_ticket_cap {
            return AdmissionResult::RejectedTicketCap;
        }
        return AdmissionResult::AdmittedWithFloorTicket;
    }
    AdmissionResult::AdmittedBulkOnly
}

/// U7: floor-zamanlama-sınıflandırması — rate=0'da devasa-saturating-deadline-ÜRETİLMEZ; sözleşme-
/// ASKIDA (deadline İLK-pozitif-rate-eligible-opportunity'de-BAŞLAR — kalan-süre-devamı-değil).
/// İstismar-önlemi: çağıran `floor_suspended_time`/`rate_zero_events`-ölçer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorTiming {
    /// Pozitif-rate: deadline = admit-anı + t_floor_ns (event-saati).
    Eligible { deadline_delta_ns: u64 },
    /// rate=0 → sözleşme-askıda (motor-notu: pacer-min-rate>0 → üretim-yolunda-pratik-erişilmez).
    SuspendedNoRate,
    /// Scheduling-opportunity-akışı-yok (idle; çağıran-wakeup-hint'iyle-çözer).
    SuspendedNoOpportunity,
}

/// U7-sınıflandırıcı: rate=0 → SuspendedNoRate; aksi Eligible{t_floor}.
#[must_use]
pub fn floor_timing(
    p: &PolicyParams,
    srtt_ns: u64,
    parity_size: u64,
    rate_bps: u64,
) -> FloorTiming {
    if rate_bps == 0 {
        FloorTiming::SuspendedNoRate
    } else {
        FloorTiming::Eligible {
            deadline_delta_ns: t_floor_ns(p, srtt_ns, parity_size, rate_bps),
        }
    }
}

/// T_floor (talimat-§5): `clamp(mult×srtt, min, max).max(serialization + granularity)` —
/// srtt-yok/0 → prior; serialization = parity_size/rate (rate=0 → rate-1-tabanı, saturating).
#[must_use]
pub fn t_floor_ns(p: &PolicyParams, srtt_ns: u64, parity_size: u64, rate_bps: u64) -> u64 {
    let srtt = if srtt_ns == 0 {
        p.srtt_prior_ns
    } else {
        srtt_ns
    };
    let base = srtt
        .saturating_mul(u64::from(p.floor_mult_permille))
        .saturating_div(1000)
        .clamp(p.t_floor_min_ns, p.t_floor_max_ns);
    let ser = parity_size
        .saturating_mul(1_000_000_000)
        .saturating_div(rate_bps.max(1));
    base.max(ser.saturating_add(p.sched_granularity_ns))
}

fn share(amount: u64, permille: u32) -> u64 {
    amount
        .saturating_mul(u64::from(permille.min(1000)))
        .saturating_div(1000)
}

/// Tek-scheduling-opportunity kararı. SAF (state-geçişi-kontrollü; determinizm-testli).
#[must_use]
pub fn step(
    kind: PolicyKind,
    p: &PolicyParams,
    st: &mut BudgetPolicyState,
    ctx: &BudgetContext,
) -> BudgetDecision {
    let accrual = ctx
        .pacing_rate_bps
        .saturating_mul(ctx.elapsed_ns)
        .saturating_div(1_000_000_000);
    let parity_demand = ctx
        .bulk_parity_demand_bytes
        .saturating_add(u64::from(ctx.floor_tickets).saturating_mul(ctx.parity_size));
    let data_side = ctx.data_demand_bytes.saturating_add(ctx.arq_demand_bytes);

    // Hard-cap bounded-credit (FloorCap/BoundedDebt; diğerlerinde-etkisiz-tutulur).
    if matches!(kind, PolicyKind::FloorCap | PolicyKind::BoundedDebtFloor) {
        st.hardcap_credit = (st
            .hardcap_credit
            .saturating_add(share(accrual, p.hardcap_share_permille)))
        .min(p.hardcap_cap_bytes);
    }
    // Entitlement-accrual (R/B): önce-borç-kapatılır (B), sonra-credit (cap'li).
    if matches!(
        kind,
        PolicyKind::ProtectedShare | PolicyKind::FloorCap | PolicyKind::BoundedDebtFloor
    ) {
        let mut acc = share(accrual, p.share_permille);
        if st.debt_bytes > 0 {
            let pay = st.debt_bytes.min(acc);
            st.debt_bytes -= pay;
            acc -= pay;
        }
        st.entitlement_credit =
            (st.entitlement_credit.saturating_add(acc)).min(p.entitlement_cap_bytes);
    }

    let t_floor = t_floor_ns(p, ctx.srtt_ns, ctx.parity_size, ctx.pacing_rate_bps);
    let mut borrow_allowance = 0u64;
    let mut cap_suppressed = false;

    // Rezervasyon (R/B): korunan-pay = min(entitlement, talep) — work-conserving (talep-yoksa-0).
    // Rezervasyon (R/B): korunan-pay = min(entitlement, talep, avail). KUANTUM-NOTU (sim-bulgusu-2):
    // psize-altı-rezerv İSRAF-DEĞİL — rezerve-token bucket'ta-BİRİKİR (DATA-tüketemez) ve havuz
    // psize'a-ulaşınca-emisyon-olur; displacement = yalnız-entitlement'tan-HARCANAN (settle-ile-
    // ölçülür), tick-başı-rezerv-tekrarı-sayılmaz (metrik-artefaktı-düzeltildi).
    let reserved = match kind {
        PolicyKind::LeftoverOnly => 0,
        _ => st
            .entitlement_credit
            .min(parity_demand)
            .min(ctx.available_tokens),
    };
    let data_limit = ctx.available_tokens.saturating_sub(reserved);
    // Leftover: DATA-tarafı-talebi-düşüldükten-sonra-kalan (rezervasyon-üstü).
    let leftover = data_limit.saturating_sub(data_side.min(data_limit));
    let mut parity_budget = reserved.saturating_add(leftover).min(parity_demand);

    // Hard-cap-C1 (FloorCap/B): parity-akışı hardcap_credit'i-aşamaz (cap-asla-aşılmaz).
    if matches!(kind, PolicyKind::FloorCap | PolicyKind::BoundedDebtFloor)
        && parity_budget > st.hardcap_credit
    {
        // B-sınıfı-C2-dar-istisnası: floor-ticket-DUE ise max_debt'e-kadar-borç-İZNİ
        // (GERÇEK-borç settle'da — harcanmamış-izin-debt-doğurmaz).
        if kind == PolicyKind::BoundedDebtFloor
            && ctx.floor_tickets > 0
            && ctx.oldest_ticket_age_ns >= t_floor
        {
            let need = ctx.parity_size.min(parity_budget);
            let extra = need.saturating_sub(st.hardcap_credit);
            let can_borrow = p.max_debt_bytes.saturating_sub(st.debt_bytes);
            if extra <= can_borrow {
                borrow_allowance = extra;
                parity_budget = need; // yalnız-due-floor-servis-kadarı (borç-izinli)
            } else {
                cap_suppressed = true;
                parity_budget = st.hardcap_credit;
            }
        } else {
            cap_suppressed = true;
            parity_budget = st.hardcap_credit;
        }
    }

    // F2: floor-ticket'lar-önce (ticket-FIFO); kalan-bütçe-bulk'a.
    let floor_grants = if matches!(kind, PolicyKind::FloorCap | PolicyKind::BoundedDebtFloor)
        && ctx.parity_size > 0
    {
        u32::try_from((parity_budget / ctx.parity_size.max(1)).min(u64::from(ctx.floor_tickets)))
            .unwrap_or(u32::MAX)
    } else {
        0
    };

    // İKİ-FAZ-MUHASEBE (adversarial-sim-bulgusu: akışkan-varsayım × kuantalı-emisyon):
    // step() HARCAMA-DÜŞMEZ — yalnız-limit-üretir; GERÇEK-harcama `settle()`-ile-düşülür
    // (harcanmamış-grant credit-eritmez → kuantum-altı-turlarda-birikim-korunur).
    let next_wakeup_hint_ns = if ctx.floor_tickets > 0 {
        t_floor.saturating_sub(ctx.oldest_ticket_age_ns.min(t_floor))
    } else {
        0
    };
    BudgetDecision {
        data_token_limit: data_limit,
        parity_token_limit: parity_budget,
        floor_grants,
        cap_suppressed,
        borrow_allowance_bytes: borrow_allowance,
        reserved_bytes: reserved,
        next_wakeup_hint_ns,
    }
}

/// İKİNCİ-FAZ: GERÇEK-parity-harcamasının-muhasebesi (çağıran drain-sonrası-çağırır).
/// `spent ≤ d.parity_token_limit` şart (assert). Döner: gerçekleşen-borç (teşhis).
/// entitlement: spent'in-reserved-kadarı-düşer; hard-cap: spent-düşer, açık borç-doğurur
/// (≤ allowance; debt'e-yazılır — sonraki-accrual-önce-kapatır).
#[must_use]
pub fn settle(
    kind: PolicyKind,
    p: &PolicyParams,
    st: &mut BudgetPolicyState,
    d: &BudgetDecision,
    spent_bytes: u64,
) -> u64 {
    assert!(
        spent_bytes <= d.parity_token_limit,
        "spent≤limit-sözleşmesi"
    );
    let from_entitlement = spent_bytes.min(d.reserved_bytes);
    st.entitlement_credit = st.entitlement_credit.saturating_sub(from_entitlement);
    let mut borrowed = 0u64;
    if matches!(kind, PolicyKind::FloorCap | PolicyKind::BoundedDebtFloor) {
        let from_cap = spent_bytes.min(st.hardcap_credit);
        st.hardcap_credit -= from_cap;
        let overrun = spent_bytes - from_cap;
        borrowed = overrun.min(d.borrow_allowance_bytes);
        // C1-mutlaklık: allowance-üstü-harcama-sözleşme-ihlali (assert; sim-kanıtlı).
        assert!(overrun <= d.borrow_allowance_bytes, "cap-aşımı-yasak (C1)");
        st.debt_bytes = (st.debt_bytes.saturating_add(borrowed)).min(p.max_debt_bytes);
    }
    borrowed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(rate: u64, avail: u64, data: u64, bulk: u64, tickets: u32, age: u64) -> BudgetContext {
        BudgetContext {
            elapsed_ns: 10_000_000,
            pacing_rate_bps: rate,
            available_tokens: avail,
            data_demand_bytes: data,
            arq_demand_bytes: 0,
            bulk_parity_demand_bytes: bulk,
            floor_tickets: tickets,
            oldest_ticket_age_ns: age,
            parity_size: 1200,
            srtt_ns: 100_000_000,
        }
    }

    /// EXHAUSTIVE-küçük-uzay: 4-politika × rate × avail × data × parity × ticket-grid'inde
    /// invariant'lar: (I1) token-icadı-yok; (I2) hard-cap-uyum (borç-hariç); (I3) state-bound'ları;
    /// (I4) work-conserving; (I5) determinizm (klon-state-çift-koşu-bit-özdeş); (I6) leftover-formülü.
    #[test]
    fn exhaustive_small_space_invariants() {
        let p = PolicyParams::defaults();
        let kinds = [
            PolicyKind::LeftoverOnly,
            PolicyKind::ProtectedShare,
            PolicyKind::FloorCap,
            PolicyKind::BoundedDebtFloor,
        ];
        let mut cases = 0u64;
        for kind in kinds {
            for rate in [0u64, 31, 1_000, 12_000, 60_000, 1_000_000] {
                for avail in [0u64, 500, 2_400, 10_000] {
                    for data in [0u64, 1_000, 5_000, 100_000] {
                        for bulk in [0u64, 1_200, 4_800, 60_000] {
                            for tickets in [0u32, 1, 3] {
                                for age in [0u64, 60_000_000, 20_000_000_000] {
                                    let mut st = BudgetPolicyState::default();
                                    // Ön-yükleme (idle-birikim-simülasyonu):
                                    let _ = step(kind, &p, &mut st, &ctx(rate, avail, 0, 0, 0, 0));
                                    let mut st2 = st;
                                    let c = ctx(rate, avail, data, bulk, tickets, age);
                                    let d1 = step(kind, &p, &mut st, &c);
                                    let d2 = step(kind, &p, &mut st2, &c);
                                    assert_eq!(d1, d2, "I5-determinizm");
                                    assert_eq!(st, st2, "I5-state-determinizm");
                                    // I1: toplam-limit ≤ available + borç (borç-yalnız-B-due-floor).
                                    assert!(
                                        d1.parity_token_limit
                                            <= c.available_tokens
                                                .saturating_add(d1.borrow_allowance_bytes),
                                        "I1 {kind:?}"
                                    );
                                    assert!(d1.data_token_limit <= c.available_tokens);
                                    // I3: state-bound'ları.
                                    assert!(
                                        st.entitlement_credit <= p.entitlement_cap_bytes,
                                        "I3-ent"
                                    );
                                    assert!(st.hardcap_credit <= p.hardcap_cap_bytes, "I3-cap");
                                    assert!(st.debt_bytes <= p.max_debt_bytes, "I3-debt");
                                    // I4: work-conserving — parity-talebi-yoksa DATA-tümünü-görür.
                                    if bulk == 0 && tickets == 0 {
                                        assert_eq!(
                                            d1.data_token_limit, c.available_tokens,
                                            "I4 {kind:?}"
                                        );
                                        assert_eq!(d1.parity_token_limit, 0);
                                    }
                                    // I6: LeftoverOnly-formülü.
                                    if kind == PolicyKind::LeftoverOnly {
                                        let want = c
                                            .available_tokens
                                            .saturating_sub(data.min(c.available_tokens))
                                            .min(bulk + u64::from(tickets) * 1200);
                                        assert_eq!(d1.parity_token_limit, want, "I6");
                                        assert_eq!(d1.floor_grants, 0);
                                        assert_eq!(d1.borrow_allowance_bytes, 0);
                                    }
                                    // I2: borç-İZNİ-yalnız-B'de-ve-due-ticket-varken.
                                    if d1.borrow_allowance_bytes > 0 {
                                        assert_eq!(kind, PolicyKind::BoundedDebtFloor);
                                        assert!(tickets > 0);
                                    }
                                    // I7-settle: tam-limit-harcaması-sonrası state-bound'ları.
                                    let _ = settle(kind, &p, &mut st, &d1, d1.parity_token_limit);
                                    assert!(st.entitlement_credit <= p.entitlement_cap_bytes);
                                    assert!(st.debt_bytes <= p.max_debt_bytes);
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(cases >= 13_000, "exhaustive-hacim: {cases}");
    }

    /// T_floor-mekanik-kapanışı (§5-durumları): prior/clamp/serialization-tabanı/overflow.
    #[test]
    fn t_floor_semantics() {
        let p = PolicyParams::defaults();
        // SRTT-yok → prior (300ms×2=600ms).
        assert_eq!(t_floor_ns(&p, 0, 1200, 60_000), 600_000_000);
        // Çok-düşük-srtt → min-clamp (50ms) AMA serialization-tabanı-üstün: 30kbps≈3750B/s →
        // 1200B-serialization=320ms + 10ms-gran = 330ms > clamp-min.
        assert_eq!(t_floor_ns(&p, 1_000_000, 1200, 3_750), 330_000_000);
        // RTT=5s → 10s (2×5s = max-clamp-sınırında).
        assert_eq!(t_floor_ns(&p, 5_000_000_000, 1200, 60_000), 10_000_000_000);
        // rate=0 → saturating (taban-1); overflow-yok.
        let big = t_floor_ns(&p, u64::MAX, u64::MAX, 0);
        assert!(big >= p.t_floor_max_ns, "saturating-üst");
        // E1024-vs-E64-parity-boyutu serialization-tabanını-büyütür (1kB/s-linkte:
        // ser(1061B)=1.061s+gran > 600ms-base; ser(101B)=111ms < 600ms-base).
        assert!(t_floor_ns(&p, 0, 1061, 1_000) > t_floor_ns(&p, 0, 101, 1_000));
    }

    /// U3-ENVELOPE-PROPERTY (arbitrary-başlangıç-credit): her-[a,b]-aralığında
    /// `protected_displacement ≤ credit(a) + floor(share×accrual)` ve
    /// `total_parity_spend ≤ hardcap_credit(a) + floor(hardcap_share×accrual)` + 0≤credit≤cap.
    #[test]
    fn resource_envelope_properties_arbitrary_initial_credit() {
        let p = PolicyParams::defaults();
        for kind in [PolicyKind::ProtectedShare, PolicyKind::FloorCap] {
            for init_ent in [0u64, 1_200, 4_800] {
                for init_cap in [0u64, 2_400, 9_600] {
                    let mut st = BudgetPolicyState {
                        entitlement_credit: init_ent.min(p.entitlement_cap_bytes),
                        hardcap_credit: init_cap.min(p.hardcap_cap_bytes),
                        debt_bytes: 0,
                    };
                    let ent_a = st.entitlement_credit;
                    let cap_a = st.hardcap_credit;
                    let mut accrual_total = 0u64;
                    let mut displaced = 0u64;
                    let mut parity_spend = 0u64;
                    // Deterministik-karışık-sürüş: değişken-rate/talep 200-opportunity.
                    for i in 0..200u64 {
                        let rate = [0u64, 31, 12_000, 60_000, 240_000][(i % 5) as usize];
                        let c = BudgetContext {
                            elapsed_ns: 10_000_000,
                            pacing_rate_bps: rate,
                            available_tokens: 2_400 + (i % 3) * 1_200,
                            data_demand_bytes: (i % 4) * 1_200,
                            arq_demand_bytes: (i % 2) * 600,
                            bulk_parity_demand_bytes: ((i + 1) % 5) * 1_200,
                            floor_tickets: (i % 3) as u32,
                            oldest_ticket_age_ns: i * 10_000_000,
                            parity_size: 1_200,
                            srtt_ns: 100_000_000,
                        };
                        accrual_total += rate * 10_000_000 / 1_000_000_000;
                        let d = step(kind, &p, &mut st, &c);
                        // Kuantize-tam-harcama (gerçekçi-drain): limit'in-psize-katı.
                        let spend = (d.parity_token_limit / 1_200) * 1_200;
                        displaced += spend.min(d.reserved_bytes);
                        parity_spend += spend;
                        let _ = settle(kind, &p, &mut st, &d, spend);
                        assert!(st.entitlement_credit <= p.entitlement_cap_bytes);
                        assert!(st.hardcap_credit <= p.hardcap_cap_bytes);
                    }
                    let ent_env = ent_a + share(accrual_total, p.share_permille);
                    assert!(
                        displaced <= ent_env,
                        "U3-entitlement-envelope: {displaced} ≤ {ent_env} ({kind:?})"
                    );
                    if kind == PolicyKind::FloorCap {
                        let cap_env = cap_a + share(accrual_total, p.hardcap_share_permille);
                        assert!(
                            parity_spend <= cap_env,
                            "U3-hardcap-envelope: {parity_spend} ≤ {cap_env}"
                        );
                    }
                }
            }
        }
    }

    /// U7: FloorTiming-sınıflandırması.
    #[test]
    fn floor_timing_rate_zero_suspended() {
        let p = PolicyParams::defaults();
        assert_eq!(
            floor_timing(&p, 100_000_000, 1200, 0),
            FloorTiming::SuspendedNoRate
        );
        match floor_timing(&p, 100_000_000, 1200, 60_000) {
            FloorTiming::Eligible { deadline_delta_ns } => {
                assert_eq!(deadline_delta_ns, t_floor_ns(&p, 100_000_000, 1200, 60_000));
            }
            other => panic!("Eligible-bekleniyordu: {other:?}"),
        }
    }

    /// Admission-feasibility typed-sonuçları (§4.1) — her-red-nedeni-ayrık.
    #[test]
    fn admission_typed_results() {
        let caps = AdmissionCaps {
            pending_items_cap: 4,
            pending_bytes_cap: 10_000,
            floor_ticket_cap: 2,
            per_block_admitted_cap: 4,
            parity_size_max: 1200,
        };
        use AdmissionResult::*;
        assert_eq!(
            admit(&caps, 0, 0, 0, 0, 1200, true, false, true),
            AdmittedWithFloorTicket
        );
        assert_eq!(
            admit(&caps, 0, 0, 0, 0, 1200, true, false, false),
            AdmittedBulkOnly
        );
        assert_eq!(
            admit(&caps, 4, 0, 0, 0, 1200, true, false, true),
            RejectedItemsCap
        );
        assert_eq!(
            admit(&caps, 0, 9_500, 0, 0, 1200, true, false, true),
            RejectedBytesCap
        );
        assert_eq!(
            admit(&caps, 0, 0, 2, 0, 1200, true, false, true),
            RejectedTicketCap
        );
        assert_eq!(
            admit(&caps, 0, 0, 0, 4, 1200, true, false, true),
            RejectedPerBlockCap
        );
        assert_eq!(
            admit(&caps, 0, 0, 0, 0, 5_000, true, false, true),
            RejectedSizeUnsupported
        );
        assert_eq!(
            admit(&caps, 0, 0, 0, 0, 0, true, false, true),
            RejectedSizeUnsupported
        );
        assert_eq!(
            admit(&caps, 0, 0, 0, 0, 1200, false, false, true),
            RejectedBlockMoot
        );
        assert_eq!(
            admit(&caps, 0, 0, 1, 0, 1200, true, true, true),
            RejectedDuplicateTicket
        );
    }

    /// R-sınıfı-rezervasyon: saturated-DATA'da-bile entitlement-kadarı-parity'ye (üçgen-T1-değişimi);
    /// C1: hard-cap-asla-aşılmaz (cap_suppressed-teşhisli); B-C2: due-floor-borçlanır (≤max_debt).
    #[test]
    fn class_semantics_saturated() {
        let p = PolicyParams::defaults();
        // L: saturated → parity-0.
        let mut st = BudgetPolicyState::default();
        let d = step(
            PolicyKind::LeftoverOnly,
            &p,
            &mut st,
            &ctx(60_000, 2_400, 100_000, 4_800, 0, 0),
        );
        assert_eq!(
            d.parity_token_limit, 0,
            "L-saturated-starvation (üçgen-kanıtı)"
        );
        // R: entitlement-birikimi-sonrası saturated'da-parity>0, DATA-limit=avail−reserved.
        let mut st = BudgetPolicyState::default();
        // 60×(600B-accrual×%20=120B)=7200B > cap-4800 → CAP'te-durur (idle-birikim-bound'u).
        for _ in 0..60 {
            let _ = step(
                PolicyKind::ProtectedShare,
                &p,
                &mut st,
                &ctx(60_000, 2_400, 0, 0, 0, 0),
            );
        }
        assert_eq!(
            st.entitlement_credit, p.entitlement_cap_bytes,
            "idle-birikim-CAP'li"
        );
        let d = step(
            PolicyKind::ProtectedShare,
            &p,
            &mut st,
            &ctx(60_000, 2_400, 100_000, 4_800, 0, 0),
        );
        assert!(
            d.parity_token_limit > 0,
            "R-saturated-parity-akar (bounded-rezervasyon)"
        );
        assert_eq!(d.data_token_limit, 2_400 - d.parity_token_limit.min(2_400));
        // FloorCap-C1: hardcap_credit=0'ken parity-bastırılır (cap-aşılMAZ) + teşhis.
        let mut st = BudgetPolicyState {
            entitlement_credit: 2_400,
            hardcap_credit: 0,
            debt_bytes: 0,
        };
        let d = step(
            PolicyKind::FloorCap,
            &p,
            &mut st,
            &ctx(0, 2_400, 0, 2_400, 1, 0),
        );
        assert_eq!(d.parity_token_limit, 0, "C1-mutlak");
        assert!(d.cap_suppressed, "miss-adayı-teşhisli (sessiz-değil)");
        // B-C2: due-ticket → borç (≤max_debt); erken-ticket-borçlanAMAZ.
        let mut st = BudgetPolicyState {
            entitlement_credit: 1_200,
            hardcap_credit: 0,
            debt_bytes: 0,
        };
        // rate=60k: t_floor=max(2×100ms-srtt, ser-20ms+gran)=200ms; age-999s → DUE.
        // hardcap-accrual bu-turda 180B (600×%30) → extra=1200−180=1020 borçlanılır.
        let d = step(
            PolicyKind::BoundedDebtFloor,
            &p,
            &mut st,
            &ctx(60_000, 2_400, 0, 0, 1, 999_000_000_000),
        );
        assert_eq!(
            d.borrow_allowance_bytes, 1_020,
            "due-floor-borç-İZNİ (extra=1200−180)"
        );
        assert_eq!(st.debt_bytes, 0, "izin≠borç: settle-öncesi-debt-yok");
        assert_eq!(d.floor_grants, 1, "due-ticket-servis");
        let got = settle(PolicyKind::BoundedDebtFloor, &p, &mut st, &d, 1_200);
        assert_eq!(got, 1_020, "GERÇEK-borç-settle'da");
        assert_eq!(st.debt_bytes, 1_020);
        let mut st2 = BudgetPolicyState::default();
        let d2 = step(
            PolicyKind::BoundedDebtFloor,
            &p,
            &mut st2,
            &ctx(60_000, 2_400, 0, 0, 1, 0),
        );
        assert_eq!(d2.borrow_allowance_bytes, 0, "erken-ticket-borç-izni-yok");
        assert!(d2.cap_suppressed, "erken-ticket-cap-bastırması-teşhisli");
        // Kuantum-altı-birikim (iki-faz-kanıtı): spent=0-settle → entitlement-ERİMEZ.
        let mut st4 = BudgetPolicyState::default();
        let d4 = step(
            PolicyKind::ProtectedShare,
            &p,
            &mut st4,
            &ctx(60_000, 600, 600, 4_800, 0, 0),
        );
        let ent_after_step = st4.entitlement_credit;
        let _ = settle(PolicyKind::ProtectedShare, &p, &mut st4, &d4, 0);
        assert_eq!(
            st4.entitlement_credit, ent_after_step,
            "harcanmamış-grant-eritmez"
        );
        // Borç-geri-ödeme: sonraki-accrual-önce-borcu-kapatır.
        let mut st3 = BudgetPolicyState {
            entitlement_credit: 0,
            hardcap_credit: 0,
            debt_bytes: 1_200,
        };
        let _ = step(
            PolicyKind::BoundedDebtFloor,
            &p,
            &mut st3,
            &ctx(1_000_000, 0, 0, 0, 0, 0),
        );
        assert!(st3.debt_bytes < 1_200, "accrual-borcu-kapatıyor");
        // reset(): rollback/mode-transition-temizliği.
        st3.reset();
        assert_eq!(st3, BudgetPolicyState::default());
    }
}
