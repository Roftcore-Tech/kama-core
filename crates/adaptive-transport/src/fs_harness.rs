//! DR-FEC-FS-1 FS-HARNESS-A — A1-EARLY runner-level source-duplication + closed-loop runner + accounting.
//!
//! **A1-EARLY = same-byte early redundancy baseline (coding-DEĞİL).** Runner-seviyesi source-duplication.
//!
//! **BLOCK-TOTAL BUDGET (Abkarada kardinalite-düzeltmesi):** A1 `configured_r` = FEC-R-sınıfına-EŞDEĞER
//! **blok-toplam** redundancy-bütçesi. **source-başına-r DEĞİL, source-başına-1 de DEĞİL** → yalnız
//! `configured_r` adet SEÇİLİ source birer-kez kopyalanır (K=8,R=2 → 2 duplicate, 16 veya 8 DEĞİL).
//! Target-set deterministik AMA **deficit-BAĞIMSIZ**: `deterministic_selection(block_id, dup_seed, R)`.
//! YASAK-girdiler: exact-deficit-D · future-loss · future-ACK · receiver-outcome. Domain-separated seed:
//! `loss_seed = H(master,"LOSS")`, `dup_seed = H(master,"A1_DUP_TARGET")`. Sonuç: A1 kayıp-source'u
//! duplicate ile YALNIZ o-source target-set'teyse kurtarır (blind-duplication'ın dürüst-davranışı).
//!
//! **PIN-A — DIRECT EMISSION-SIDE STATE NEUTRALITY:** emisyon-EYLEMİ Sender-state'ini DOĞRUDAN değiştirmez
//! ([`A1EarlyEmitter`] Sender'a referans-TUTMAZ). Post-feedback state-eşitliği İSTENMEZ (erken-ACK meşru-sonuç).
//! **PIN-E — EGRESS-FAIRNESS:** her kopya gerçek-egress'ten serialize; bedava-lane-YOK; realized-sealed-wire
//! ayrı kaydedilir. **C-P4 fairness = realized_sealed_wire_bytes** (configured_r-EŞİTLİĞİ TEK-BAŞINA-YETMEZ;
//! F0-sonrası A1-realized-wire vs F0-realized-wire karşılaştırılır; frozen-tolerance-dışı → BUDGET_MISMATCH).

use crate::fs_trace::{TraceChannel, TracePacketId, TransmissionRole};
use std::collections::BTreeSet;

/// Redundancy'nin türü (arm-neutral timing-ledger + accounting eşlemesi için).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RedundancyKind {
    /// A0: proaktif-redundancy yok.
    #[default]
    None,
    /// A1-EARLY: same-byte source-duplication.
    Duplicate,
    /// F0/F1/F3/F4: coded parity/repair.
    FecRepair,
}

/// Redundancy sealed-wire maliyeti — A1/F0 AYNI physical-accounting-domain'de. **C-P4 fairness
/// `realized_sealed_wire_bytes` üstünden** (configured_r-eşitliği TEK-BAŞINA-yetmez).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RedundancyCost {
    pub configured_r: u16,
    pub redundancy_payload_bytes: u64,
    pub redundancy_framing_bytes: u64,
    /// GERÇEK sealed-wire (payload + framing) — C-P4 nihai fairness metriği.
    pub realized_sealed_wire_bytes: u64,
    pub redundancy_packet_count: u64,
}

/// Paket-yutağı (channel/socket egress soyutlaması). Emitter YALNIZ buna yazar — Sender'a dokunmaz.
pub trait PacketSink {
    fn send(&mut self, id: TracePacketId, sealed: &[u8]);
}

/// FNV-1a 64-bit (deterministik; RNG-YOK).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Domain-separated deterministic seed: `H(master_seed ‖ tag)`. Örn `loss_seed = domain_seed(m,"LOSS")`,
/// `dup_seed = domain_seed(m,"A1_DUP_TARGET")` → loss ve dup-target KARŞILIKLI-BAĞIMSIZ.
#[must_use]
pub fn domain_seed(master_seed: u64, tag: &str) -> u64 {
    let mut k = master_seed.to_le_bytes().to_vec();
    k.extend_from_slice(tag.as_bytes());
    fnv1a(&k)
}

/// A1 duplicate-target-set: `configured_r` adet source-ESI'yi DETERMINISTIK + **deficit-BAĞIMSIZ** seçer
/// (en-küçük-`configured_r` hash-rank). Girdiler YALNIZ (block_id, dup_seed, configured_r, k_source) —
/// D/future-loss/ACK/outcome GİRMEZ. |target| = min(configured_r, k_source).
#[must_use]
pub fn a1_duplicate_target_set(
    block_id: u32,
    dup_seed: u64,
    configured_r: u16,
    k_source: u16,
) -> BTreeSet<u16> {
    let r = configured_r.min(k_source) as usize;
    let mut ranked: Vec<(u64, u16)> = (0..k_source)
        .map(|esi| {
            let mut k = [0u8; 14];
            k[0..4].copy_from_slice(&block_id.to_le_bytes());
            k[4..12].copy_from_slice(&dup_seed.to_le_bytes());
            k[12..14].copy_from_slice(&esi.to_le_bytes());
            (fnv1a(&k), esi)
        })
        .collect();
    ranked.sort_unstable();
    ranked.into_iter().take(r).map(|(_, esi)| esi).collect()
}

/// A1-EARLY blok-toplam duplication emitter. `configured_r` SEÇİLİ source birer-kez kopyalanır (blok-total;
/// source-başına-r DEĞİL). Target-set deficit-BAĞIMSIZ. PIN-A: Sender'a referans-TUTMAZ.
pub struct A1EarlyEmitter {
    target: BTreeSet<u16>,
    cost: RedundancyCost,
}

impl A1EarlyEmitter {
    #[must_use]
    pub fn new(configured_r: u16, dup_seed: u64, block_id: u32, k_source: u16) -> Self {
        Self {
            target: a1_duplicate_target_set(block_id, dup_seed, configured_r, k_source),
            cost: RedundancyCost {
                configured_r,
                ..RedundancyCost::default()
            },
        }
    }

    #[must_use]
    pub fn target_set(&self) -> &BTreeSet<u16> {
        &self.target
    }

    /// `source_id` target-set'teyse **ONE** duplicate emit eder (blok-total bütçe). Değilse no-op.
    /// Döner: duplicate-emit-edildi-mi. `payload_len` = source payload (framing-accounting).
    pub fn maybe_emit_duplicate<S: PacketSink>(
        &mut self,
        source_id: (u32, u16),
        sealed: &[u8],
        payload_len: u64,
        sink: &mut S,
    ) -> bool {
        if !self.target.contains(&source_id.1) {
            return false;
        }
        let id = TracePacketId {
            source_id,
            role: TransmissionRole::Duplicate,
            attempt: 1,
        };
        sink.send(id, sealed); // PIN-E: gerçek egress (bedava-lane-yok)
        let sealed_len = sealed.len() as u64;
        self.cost.realized_sealed_wire_bytes += sealed_len;
        self.cost.redundancy_payload_bytes += payload_len;
        self.cost.redundancy_framing_bytes += sealed_len.saturating_sub(payload_len);
        self.cost.redundancy_packet_count += 1;
        true
    }

    #[must_use]
    pub fn cost(&self) -> RedundancyCost {
        self.cost
    }
}

/// A1-EARLY emisyon-sırası politikası (deterministik; async-scheduling'e BIRAKILMAZ).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedundancyEmissionPolicy {
    /// `source_i → (i∈target ? duplicate_i) → source_{i+1}` (early per-source adjacent; SEÇİLİ-source-dup).
    PerSourceAdjacentEarly,
}

/// PER_SOURCE_ADJACENT_EARLY emisyon-dizisi: her source'un InitialSource'u; SEÇİLİ source'un (target) BİR
/// duplicate'i hemen-ardından (blok-total). Deterministik.
#[must_use]
pub fn a1_emission_order(
    sources: &[(u32, u16)],
    target: &BTreeSet<u16>,
    policy: RedundancyEmissionPolicy,
) -> Vec<TracePacketId> {
    let RedundancyEmissionPolicy::PerSourceAdjacentEarly = policy;
    let mut out = Vec::with_capacity(sources.len() + target.len());
    for &sid in sources {
        out.push(TracePacketId {
            source_id: sid,
            role: TransmissionRole::InitialSource,
            attempt: 0,
        });
        if target.contains(&sid.1) {
            out.push(TracePacketId {
                source_id: sid,
                role: TransmissionRole::Duplicate,
                attempt: 1,
            });
        }
    }
    out
}

/// Arm-neutral redundancy timing-ledger. Legacy FEC-specific `t_parity_*` KORUNUR (F*); generic
/// `t_redundancy_*` HER-arm (A1→Duplicate · F*→FecRepair). SALT-GÖZLEM. `t_source_loss` = deterministic
/// TraceChannel INITIAL-source DROP-anı (ACK-tahmini DEĞİL). C-P4 "coding mı timing mi" bundan okunur.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BlockTiming {
    pub block_id: u32,
    pub redundancy_kind: Option<RedundancyKind>,
    pub t_source_loss: Option<u64>,
    pub t_parity_eligible: Option<u64>,
    pub t_parity_enqueued: Option<u64>,
    pub t_parity_wire: Option<u64>,
    pub t_parity_arrival: Option<u64>,
    pub t_arq_eligible: Option<u64>,
    pub t_arq_wire: Option<u64>,
    pub t_arq_arrival: Option<u64>,
    pub t_block_complete: Option<u64>,
    pub t_redundancy_eligible: Option<u64>,
    pub t_redundancy_enqueued: Option<u64>,
    pub t_redundancy_wire: Option<u64>,
    pub t_redundancy_arrival: Option<u64>,
    /// **t_repair_usable (Abkarada):** redundancy'nin GERÇEKTEN-kullanılabilir olduğu an. FEC için =
    /// reconstruct-TAMAMLANDI + recovered-byte-hazır (codec-decode-latency dahil; parity-arrival'DAN
    /// FARKLI). A1 için = duplicate-arrival (decode-latency yok). **USABLE_BEFORE_ARQ bundan türetilir.**
    pub t_repair_usable: Option<u64>,
}

/// Redundancy gecikme-sınıfı (§8.1; t_repair_usable-tabanlı — parity-arrival DEĞİL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityDelayClass {
    NotEligible,
    EligibleNotEnqueued,
    EnqueuedNotWire,
    WireButLate,
    /// t_repair_usable < t_arq_arrival (redundancy ARQ-onarımından ÖNCE kullanılabilir).
    UsableBeforeArq,
}

impl BlockTiming {
    /// USABLE_BEFORE_ARQ **t_repair_usable**'dan türetilir (codec-latency causal-path'te; parity-arrival DEĞİL).
    #[must_use]
    pub fn parity_delay_class(&self) -> ParityDelayClass {
        match (
            self.t_redundancy_eligible,
            self.t_redundancy_enqueued,
            self.t_redundancy_wire,
        ) {
            (None, _, _) => ParityDelayClass::NotEligible,
            (Some(_), None, _) => ParityDelayClass::EligibleNotEnqueued,
            (Some(_), Some(_), None) => ParityDelayClass::EnqueuedNotWire,
            (Some(_), Some(_), Some(_)) => match (self.t_repair_usable, self.t_arq_arrival) {
                (Some(u), Some(a)) if u < a => ParityDelayClass::UsableBeforeArq,
                (Some(u), None) if self.t_block_complete.is_some_and(|c| u <= c) => {
                    ParityDelayClass::UsableBeforeArq
                }
                _ => ParityDelayClass::WireButLate,
            },
        }
    }
}

impl BottleneckChannel {
    /// Trace decision-log hash (pairing-kanıtı: aynı-trace kollar-arası identical-hash).
    #[must_use]
    pub fn trace_hash(&self) -> u64 {
        self.trace.decision_log_hash()
    }
    /// (kanal-karar-sayısı, düşürülen) — faithfulness: realized-drop vs configured-|D|.
    #[must_use]
    pub fn trace_counts(&self) -> (u64, u64) {
        self.trace.counts()
    }
}

impl BlockTiming {
    /// Generic redundancy lifecycle'ı kaydet; FecRepair'de AYRICA legacy `t_parity_*`.
    pub fn record_redundancy(
        &mut self,
        kind: RedundancyKind,
        eligible: u64,
        enqueued: u64,
        wire: u64,
        arrival: Option<u64>,
    ) {
        self.redundancy_kind = Some(kind);
        self.t_redundancy_eligible = Some(eligible);
        self.t_redundancy_enqueued = Some(enqueued);
        self.t_redundancy_wire = Some(wire);
        self.t_redundancy_arrival = arrival;
        if matches!(kind, RedundancyKind::FecRepair) {
            self.t_parity_eligible = Some(eligible);
            self.t_parity_enqueued = Some(enqueued);
            self.t_parity_wire = Some(wire);
            self.t_parity_arrival = arrival;
        }
    }
}

/// Deterministik bottleneck egress-model (PIN-E physical-cost). Her paket AYNI bottleneck'ten serialize →
/// A1-duplicate BEDAVA-LANE-ALMAZ; dropped-paket de egress-serialization-tüketir.
pub struct BottleneckChannel {
    trace: TraceChannel,
    link_bps: u64,
    prop_delay_ns: u64,
    link_free_ns: u64,
    serialized_bytes: u64,
}

impl BottleneckChannel {
    #[must_use]
    pub fn new(trace: TraceChannel, link_bps: u64, prop_delay_ns: u64) -> Self {
        Self {
            trace,
            link_bps: link_bps.max(1),
            prop_delay_ns,
            link_free_ns: 0,
            serialized_bytes: 0,
        }
    }

    /// Paketi bottleneck'e sun. `None` = dropped; `Some(arrival_ns)` = teslim. Egress-serialization HER
    /// paket için (dropped dahil) → redundancy egress-budget tüketir (PIN-E).
    pub fn offer(&mut self, id: TracePacketId, sealed_len: u64, emit_ns: u64) -> Option<u64> {
        let serialize_ns =
            sealed_len.saturating_mul(8).saturating_mul(1_000_000_000) / self.link_bps;
        let start = emit_ns.max(self.link_free_ns);
        self.link_free_ns = start.saturating_add(serialize_ns);
        self.serialized_bytes = self.serialized_bytes.saturating_add(sealed_len);
        if self.trace.decide(id, emit_ns) {
            return None;
        }
        Some(self.link_free_ns.saturating_add(self.prop_delay_ns))
    }

    #[must_use]
    pub fn serialized_bytes(&self) -> u64 {
        self.serialized_bytes
    }
}

// ── CLOSED-LOOP RUNNER (gerçek Sender + BottleneckChannel; A0/A1) ─────────────────────────────────

use crate::fs_trace::{RedundancyPolicy, TraceSpec};
use crate::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind};
use std::time::Duration;

/// Ölçülen kol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// ARQ + 0 proaktif-redundancy.
    A0,
    /// ARQ + same-byte early source-duplication (blok-total R selected-source, deficit-independent).
    A1 { r: u16 },
}

/// Kapalı-döngü koşu konfigürasyonu.
#[derive(Debug, Clone)]
pub struct RunCfg {
    pub num_units: usize,
    pub unit_size: u64,
    pub link_bps: u64,
    pub owd_ns: u64,
    pub horizon_ns: u64,
    pub dt_ns: u64,
    /// ExactDeficit: INITIAL-transmission'ı düşürülen unit_id'ler (retransmit/duplicate BAĞIMSIZ-geçer).
    pub drop_initial_units: BTreeSet<u64>,
    /// A1 duplicate-target seed (domain-separated; deficit-BAĞIMSIZ). Loss-trace'ten AYRI.
    pub dup_seed: u64,
}

/// Path-timing snapshot ledger-only (A0/A1/F0 KARAR-İÇİN-TÜKETMEZ; yalnız gözlem).
#[derive(Debug, Default, Clone, Copy)]
pub struct PathTimingLedger {
    pub path_srtt_ns: Option<u64>,
    pub path_min_rtt_ns: Option<u64>,
    pub path_rttvar_ns: u64,
    pub path_queue_delay_ns: u64,
    pub path_sample_present: bool,
    pub path_rtprop_fresh: bool,
}

/// Per-run instrument-chain (admission-gate'ler; ihlal → INVALID — NO EVIDENCE, protokol-FAIL-DEĞİL).
#[derive(Debug, Default, Clone, Copy)]
pub struct PerRunInstrument {
    /// faithfulness: realized-source-drop == configured (deterministic-channel → tam-eşit).
    pub faithful: bool,
    /// overlap-gate: receiver_events > 0.
    pub overlap_ok: bool,
    /// binary/arg provenance (config-kimliği; arg-parity kanıtı).
    pub config_hash: u64,
    /// trace-kimliği (pairing: aynı-trace kollar-arası identical).
    pub trace_hash: u64,
    /// intervention-kimliği.
    pub arm_intervention: RedundancyKind,
    pub configured_source_drops: u64,
    pub realized_source_drops: u64,
    pub receiver_events: u64,
}
impl PerRunInstrument {
    /// Admission: faithful ∧ overlap. İhlal → INVALID.
    #[must_use]
    pub fn admitted(&self) -> bool {
        self.faithful && self.overlap_ok
    }
}

/// Kapalı-döngü koşu sonucu (observational).
#[derive(Debug, Default, Clone, Copy)]
pub struct RunResult {
    pub num_units: usize,
    pub delivered_units: usize,
    pub completion_ns: Option<u64>,
    pub data_emitted: u64,
    pub retransmits_emitted: u64,
    pub duplicates_emitted: u64,
    pub cost: RedundancyCost,
    pub channel_serialized_bytes: u64,
    /// 9-timestamp + generic ledger (t_repair_usable dahil).
    pub timing: BlockTiming,
    /// A1 blind-dup causal: hit = dropped-initial ∈ target; miss = dropped-initial ∉ target.
    pub a1_dup_hits: u64,
    pub a1_dup_misses: u64,
    pub path_timing: PathTimingLedger,
    pub instrument: PerRunInstrument,
}

fn tag_for(kind: TransmitKind, unit: u64) -> Option<(TracePacketId, bool)> {
    let (role, is_data) = match kind {
        TransmitKind::Data => (TransmissionRole::InitialSource, true),
        TransmitKind::Retransmit => (TransmissionRole::RetransmitSource, false),
        _ => return None,
    };
    Some((
        TracePacketId {
            source_id: (0, unit as u16),
            role,
            attempt: u16::from(!is_data),
        },
        is_data,
    ))
}

/// Emisyonları işle: semantic-tag → BottleneckChannel → delivery-schedule; A1'de SEÇİLİ-source dup (blok-total).
/// deliveries: (arr, unit, seq, kind) — kind: 0=source · 1=retransmit · 2=duplicate. Timing capture.
fn process_emissions(
    decisions: &[TransmitDecision],
    now: u64,
    dup_target: &BTreeSet<u16>,
    unit_size: u64,
    ch: &mut BottleneckChannel,
    res: &mut RunResult,
    deliveries: &mut Vec<(u64, u64, u64, u8)>,
) {
    for d in decisions {
        let Some((id, is_data)) = tag_for(d.kind, d.unit_id) else {
            continue;
        };
        let kind_tag = match d.kind {
            TransmitKind::Data => {
                res.data_emitted += 1;
                0
            }
            TransmitKind::Retransmit => {
                res.retransmits_emitted += 1;
                if res.timing.t_arq_eligible.is_none() {
                    res.timing.t_arq_eligible = Some(now);
                    res.timing.t_arq_wire = Some(now);
                }
                1
            }
            _ => continue,
        };
        match ch.offer(id, d.size, now) {
            Some(arr) => deliveries.push((arr, d.unit_id, d.seq, kind_tag)),
            None if is_data && res.timing.t_source_loss.is_none() => {
                res.timing.t_source_loss = Some(now); // deterministic INITIAL-source DROP-anı
            }
            None => {}
        }
        // A1 blok-total: yalnız SEÇİLİ (target) source'un BİR duplicate'i (K×R DEĞİL).
        if is_data && dup_target.contains(&(d.unit_id as u16)) {
            let did = TracePacketId {
                source_id: (0, d.unit_id as u16),
                role: TransmissionRole::Duplicate,
                attempt: 1,
            };
            res.duplicates_emitted += 1;
            res.cost.redundancy_packet_count += 1;
            res.cost.realized_sealed_wire_bytes += d.size;
            res.cost.redundancy_payload_bytes += unit_size;
            if res.timing.t_redundancy_wire.is_none() {
                res.timing.redundancy_kind = Some(RedundancyKind::Duplicate);
                res.timing.t_redundancy_eligible = Some(now);
                res.timing.t_redundancy_enqueued = Some(now);
                res.timing.t_redundancy_wire = Some(now); // dup: eligible=enqueued=wire (early, synchronous)
            }
            if let Some(arr) = ch.offer(did, d.size, now) {
                deliveries.push((arr, d.unit_id, d.seq, 2));
            }
        }
    }
}

/// A1 target-set'i cfg'den hesapla (blok=0; k=num_units; deficit-BAĞIMSIZ). Test/smoke için de kullanılır.
#[must_use]
pub fn run_dup_target(arm: Arm, cfg: &RunCfg) -> BTreeSet<u16> {
    match arm {
        Arm::A1 { r } => a1_duplicate_target_set(0, cfg.dup_seed, r, cfg.num_units as u16),
        Arm::A0 => BTreeSet::new(),
    }
}

/// KAPALI-DÖNGÜ: gerçek `Sender` (ARQ+ACK+pacing) × deterministic semantic `BottleneckChannel`.
/// A0 kayıp-source'u ARQ-retransmit ile; A1 duplicate ile (YALNIZ o-source target-set'teyse, aksi ARQ).
#[must_use]
pub fn run_arm(arm: Arm, cfg: &RunCfg) -> RunResult {
    let r = match arm {
        Arm::A1 { r } => r,
        Arm::A0 => 0,
    };
    let dup_target = run_dup_target(arm, cfg);
    let drop_set: BTreeSet<(u32, u16)> = cfg
        .drop_initial_units
        .iter()
        .map(|&u| (0u32, u as u16))
        .collect();
    let trace = TraceChannel::new(TraceSpec::ExactDeficit {
        drop_initial: drop_set,
        redundancy: RedundancyPolicy::PassAll,
    });
    let mut ch = BottleneckChannel::new(trace, cfg.link_bps, cfg.owd_ns);
    let mut s = Sender::new();
    let mut res = RunResult {
        num_units: cfg.num_units,
        cost: RedundancyCost {
            configured_r: r,
            ..RedundancyCost::default()
        },
        ..RunResult::default()
    };
    let mut delivered: BTreeSet<u64> = BTreeSet::new();
    let mut deliveries: Vec<(u64, u64, u64, u8)> = Vec::new();
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut receiver_events = 0u64;

    let mut d0 = Vec::new();
    for u in 0..cfg.num_units as u64 {
        d0.extend(s.on_event(
            Event::AppData(OpaqueUnit {
                id: u,
                size: cfg.unit_size,
            }),
            0,
        ));
    }
    process_emissions(
        &d0,
        0,
        &dup_target,
        cfg.unit_size,
        &mut ch,
        &mut res,
        &mut deliveries,
    );

    let mut now = 0u64;
    while now < cfg.horizon_ns && delivered.len() < cfg.num_units {
        now += cfg.dt_ns;
        deliveries.retain(|&(arr, unit, seq, kind)| {
            if arr <= now {
                receiver_events += 1;
                let first = delivered.insert(unit);
                if first {
                    // Arrival-timestamp by kind (t_repair_usable = dup-arrival; A1 decode-latency-yok).
                    match kind {
                        1 if res.timing.t_arq_arrival.is_none() => {
                            res.timing.t_arq_arrival = Some(arr);
                        }
                        2 => {
                            if res.timing.t_redundancy_arrival.is_none() {
                                res.timing.t_redundancy_arrival = Some(arr);
                            }
                            if res.timing.t_repair_usable.is_none() {
                                res.timing.t_repair_usable = Some(arr);
                            }
                        }
                        _ => {}
                    }
                    if delivered.len() == cfg.num_units {
                        res.completion_ns = Some(arr.max(now));
                        res.timing.t_block_complete = res.completion_ns;
                    }
                }
                acks.push((arr + cfg.owd_ns, seq));
                false
            } else {
                true
            }
        });
        let due: Vec<u64> = {
            let mut v = Vec::new();
            acks.retain(|&(t, seq)| {
                if t <= now {
                    v.push(seq);
                    false
                } else {
                    true
                }
            });
            v
        };
        if !due.is_empty() {
            let d = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: due,
                    ack_delay: Duration::from_nanos(0),
                }),
                now,
            );
            process_emissions(
                &d,
                now,
                &dup_target,
                cfg.unit_size,
                &mut ch,
                &mut res,
                &mut deliveries,
            );
        }
        let d = s.on_event(Event::TimerFired(Timer::Pto), now);
        process_emissions(
            &d,
            now,
            &dup_target,
            cfg.unit_size,
            &mut ch,
            &mut res,
            &mut deliveries,
        );
    }
    res.delivered_units = delivered.len();
    res.channel_serialized_bytes = ch.serialized_bytes();
    // A0 redundancy_kind=None (dup-yoksa timing'de set edilmedi).
    if res.timing.redundancy_kind.is_none() {
        res.timing.redundancy_kind = Some(RedundancyKind::None);
    }
    // A1 blind-dup hit/miss: dropped-initial ∈ target → hit (dup-kurtarır); ∉ → miss (ARQ-fallback).
    for &u in &cfg.drop_initial_units {
        if dup_target.contains(&(u as u16)) {
            res.a1_dup_hits += 1;
        } else if matches!(arm, Arm::A1 { .. }) {
            res.a1_dup_misses += 1;
        }
    }
    // Path-timing ledger-only (KARAR-İÇİN-TÜKETİLMEDİ; yalnız gözlem).
    let pt = s.path_timing_snapshot();
    res.path_timing = PathTimingLedger {
        path_srtt_ns: pt.srtt_ns,
        path_min_rtt_ns: pt.min_rtt_ns,
        path_rttvar_ns: pt.rttvar_ns,
        path_queue_delay_ns: pt.queue_delay_ns,
        path_sample_present: pt.sample_present,
        path_rtprop_fresh: pt.rtprop_fresh,
    };
    // Per-run instrument-chain.
    let (_, realized_drops) = ch.trace_counts();
    let configured = cfg.drop_initial_units.len() as u64;
    res.instrument = PerRunInstrument {
        faithful: realized_drops == configured, // deterministic-channel → tam-eşit
        overlap_ok: receiver_events > 0,
        config_hash: run_config_hash(arm, cfg),
        trace_hash: ch.trace_hash(),
        arm_intervention: res.timing.redundancy_kind.unwrap_or(RedundancyKind::None),
        configured_source_drops: configured,
        realized_source_drops: realized_drops,
        receiver_events,
    };
    res
}

/// Config-kimliği (binary/arg provenance + arg-parity: aynı-config → aynı-hash; yalnız arm-farkı ölçülür).
#[must_use]
pub fn run_config_hash(arm: Arm, cfg: &RunCfg) -> u64 {
    let arm_disc: u8 = match arm {
        Arm::A0 => 0,
        Arm::A1 { .. } => 1,
    };
    let mut k = Vec::new();
    k.push(arm_disc);
    k.extend_from_slice(&(cfg.num_units as u64).to_le_bytes());
    k.extend_from_slice(&cfg.unit_size.to_le_bytes());
    k.extend_from_slice(&cfg.link_bps.to_le_bytes());
    k.extend_from_slice(&cfg.owd_ns.to_le_bytes());
    k.extend_from_slice(&cfg.dt_ns.to_le_bytes());
    k.extend_from_slice(&cfg.dup_seed.to_le_bytes());
    for u in &cfg.drop_initial_units {
        k.extend_from_slice(&u.to_le_bytes());
    }
    fnv1a(&k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_trace::TraceChannel;

    const MS: u64 = 1_000_000;

    fn pid(role: TransmissionRole, block: u32, esi: u16, attempt: u16) -> TracePacketId {
        TracePacketId {
            source_id: (block, esi),
            role,
            attempt,
        }
    }

    #[derive(Default)]
    struct CountingSink {
        packets: Vec<(TracePacketId, usize)>,
    }
    impl PacketSink for CountingSink {
        fn send(&mut self, id: TracePacketId, sealed: &[u8]) {
            self.packets.push((id, sealed.len()));
        }
    }

    #[allow(clippy::type_complexity)]
    fn snapshot(
        s: &Sender,
    ) -> (
        crate::sender::SenderState,
        (u64, u64),
        (u64, u64),
        (u64, u64),
        (u64, u64, u64),
        u64,
        Option<u64>,
        Option<u64>,
        u64,
    ) {
        let pt = s.path_timing_snapshot();
        (
            s.state(),
            s.diag_wbtlbw(),
            s.diag_qd(),
            s.diag_rtt(),
            s.diag_ledger(),
            s.diag_pif(),
            pt.srtt_ns,
            pt.min_rtt_ns,
            pt.queue_delay_ns,
        )
    }

    /// PIN-A: A1 duplicate-emisyonu Sender-state'ini DOĞRUDAN değiştirmez (feedback'ten önce özdeş).
    #[test]
    fn a1_early_duplicate_is_direct_emission_state_neutral() {
        let mut s = Sender::new();
        let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 1200 }), 0);
        let _ = s.on_event(
            Event::AckReceived(AckInfo {
                acked_seqs: vec![0],
                ack_delay: Duration::from_millis(1),
            }),
            50 * MS,
        );
        let before = snapshot(&s);

        // configured_r=2, k=8 → target 2 source; onları emit et.
        let mut emitter = A1EarlyEmitter::new(2, domain_seed(11, "A1_DUP_TARGET"), 0, 8);
        let mut sink = CountingSink::default();
        let sealed = [0xABu8; 108];
        let targets: Vec<u16> = emitter.target_set().iter().copied().collect();
        for esi in &targets {
            emitter.maybe_emit_duplicate((0, *esi), &sealed, 65, &mut sink);
        }

        let after = snapshot(&s);
        assert_eq!(
            before, after,
            "A1 duplicate-emisyonu Sender-state'ini değiştirdi (PIN-A ihlali)"
        );
        assert_eq!(sink.packets.len(), 2, "blok-total: 2 target → 2 dup");
    }

    /// PIN-E: duplicate egress-path tüketir + exact realized-sealed-wire (framing-split).
    #[test]
    fn a1_early_duplicate_consumes_wire_budget() {
        let mut emitter = A1EarlyEmitter::new(2, domain_seed(11, "A1_DUP_TARGET"), 0, 8);
        let mut sink = CountingSink::default();
        let sealed = [0u8; 108];
        let targets: Vec<u16> = emitter.target_set().iter().copied().collect();
        for esi in &targets {
            emitter.maybe_emit_duplicate((0, *esi), &sealed, 65, &mut sink);
        }
        let cost = emitter.cost();
        assert_eq!(cost.redundancy_packet_count, 2);
        assert_eq!(cost.realized_sealed_wire_bytes, 216); // 2×108
        assert_eq!(cost.redundancy_payload_bytes, 130); // 2×65
        assert_eq!(cost.redundancy_framing_bytes, 86); // 2×43
        assert_eq!(
            cost.redundancy_payload_bytes + cost.redundancy_framing_bytes,
            cost.realized_sealed_wire_bytes
        );
        assert_eq!(sink.packets.len(), 2);
    }

    /// BLOCK-BOUNDED: K=8,R=2 → toplam-dup 2 (16 veya 8 DEĞİL). Tüm-source denense de yalnız-target emit.
    #[test]
    fn a1_total_redundancy_is_block_bounded() {
        let mut emitter = A1EarlyEmitter::new(2, domain_seed(7, "A1_DUP_TARGET"), 0, 8);
        let mut sink = CountingSink::default();
        let sealed = [0u8; 100];
        for esi in 0..8u16 {
            emitter.maybe_emit_duplicate((0, esi), &sealed, 60, &mut sink);
        }
        assert_eq!(
            emitter.cost().redundancy_packet_count,
            2,
            "blok-total = R = 2 (K×R=16 DEĞİL)"
        );
        assert_eq!(sink.packets.len(), 2);
    }

    /// TARGET-COUNT = configured budget (|target| = min(R,k)).
    #[test]
    fn a1_duplicate_target_count_matches_configured_budget() {
        assert_eq!(a1_duplicate_target_set(0, 123, 2, 8).len(), 2);
        assert_eq!(a1_duplicate_target_set(0, 123, 4, 8).len(), 4);
        assert_eq!(a1_duplicate_target_set(0, 123, 10, 8).len(), 8); // clamp min(R,k)
        assert_eq!(a1_duplicate_target_set(0, 123, 0, 8).len(), 0);
    }

    /// DEFICIT-INDEPENDENT: target-set D/future-loss almaz (yapısal) + dup_seed'e-bağlı, loss_seed'e-DEĞİL.
    #[test]
    fn a1_duplicate_target_selection_is_deficit_independent() {
        let dup = domain_seed(42, "A1_DUP_TARGET");
        // Aynı (block,dup_seed,r,k) → aynı target (D ne-olursa-olsun; D girdi-DEĞİL).
        let t1 = a1_duplicate_target_set(0, dup, 3, 8);
        let t2 = a1_duplicate_target_set(0, dup, 3, 8);
        assert_eq!(t1, t2);
        // Domain-ayrımı: loss_seed ile dup_seed FARKLI → target loss-trace'e kaymaz.
        let loss = domain_seed(42, "LOSS");
        assert_ne!(dup, loss, "domain-separation yok (loss_seed==dup_seed)");
        assert_ne!(
            a1_duplicate_target_set(0, dup, 3, 8),
            a1_duplicate_target_set(0, loss, 3, 8),
            "target loss-seed'e bağlı (contamination)"
        );
    }

    /// Emission-order: SEÇİLİ source'un (target) BİR dup'i hemen-ardından; deterministik.
    #[test]
    fn a1_emission_order_per_source_adjacent_early_deterministic() {
        use TransmissionRole::*;
        let sources = [(0u32, 0u16), (0, 1), (0, 2)];
        let target: BTreeSet<u16> = [1u16].into_iter().collect(); // yalnız esi=1 dup'lanır
        let o1 = a1_emission_order(
            &sources,
            &target,
            RedundancyEmissionPolicy::PerSourceAdjacentEarly,
        );
        let o2 = a1_emission_order(
            &sources,
            &target,
            RedundancyEmissionPolicy::PerSourceAdjacentEarly,
        );
        assert_eq!(o1, o2);
        // S0, S1, D1, S2 (yalnız esi=1'in dup'i, hemen-ardından).
        assert_eq!(o1.len(), 4);
        assert_eq!((o1[0].role, o1[0].source_id), (InitialSource, (0, 0)));
        assert_eq!((o1[1].role, o1[1].source_id), (InitialSource, (0, 1)));
        assert_eq!(
            (o1[2].role, o1[2].source_id, o1[2].attempt),
            (Duplicate, (0, 1), 1)
        );
        assert_eq!((o1[3].role, o1[3].source_id), (InitialSource, (0, 2)));
    }

    #[test]
    fn bottleneck_serializes_all_classes_no_free_lane() {
        use TransmissionRole::*;
        let ch = TraceChannel::new(TraceSpec::NaturalIid {
            loss_permille: 0,
            seed: 0,
        });
        let mut bn = BottleneckChannel::new(ch, 1_000_000, 10_000_000);
        let a_s = bn.offer(pid(InitialSource, 0, 0, 0), 100, 0).unwrap();
        let a_d1 = bn.offer(pid(Duplicate, 0, 0, 1), 100, 0).unwrap();
        let a_d2 = bn.offer(pid(Duplicate, 0, 1, 1), 100, 0).unwrap();
        assert!(a_d1 > a_s && a_d2 > a_d1, "dup bedava-lane aldı");
        assert_eq!(a_d1 - a_s, 800_000);
        assert_eq!(bn.serialized_bytes(), 300);
    }

    #[test]
    fn bottleneck_drops_still_consume_egress_budget() {
        use TransmissionRole::*;
        let ch = TraceChannel::new(TraceSpec::ExactDeficit {
            drop_initial: [(0u32, 2u16)].into_iter().collect(),
            redundancy: RedundancyPolicy::PassAll,
        });
        let mut bn = BottleneckChannel::new(ch, 1_000_000, 0);
        assert!(bn.offer(pid(InitialSource, 0, 2, 0), 100, 0).is_none());
        assert_eq!(
            bn.serialized_bytes(),
            100,
            "dropped-paket egress-budget tüketmeli (PIN-E)"
        );
    }

    #[test]
    fn block_timing_generic_and_legacy_mapping() {
        let mut a1 = BlockTiming::default();
        a1.record_redundancy(RedundancyKind::Duplicate, 10, 20, 30, Some(40));
        assert_eq!(a1.t_redundancy_wire, Some(30));
        assert_eq!(
            a1.t_parity_wire, None,
            "A1(Duplicate) legacy-parity DOLMAMALI"
        );
        let mut f0 = BlockTiming::default();
        f0.record_redundancy(RedundancyKind::FecRepair, 10, 20, 30, Some(40));
        assert_eq!(
            f0.t_parity_wire,
            Some(30),
            "F*(FecRepair) legacy+generic AYNI"
        );
    }

    fn cfg(drop: &[u64]) -> RunCfg {
        RunCfg {
            num_units: 8,
            unit_size: 300,
            link_bps: 1_000_000,
            owd_ns: 20 * MS,
            horizon_ns: 30_000 * MS,
            dt_ns: MS,
            drop_initial_units: drop.iter().copied().collect(),
            dup_seed: domain_seed(2026, "A1_DUP_TARGET"),
        }
    }

    /// CLOSED-LOOP SMOKE (loopback): kayıp-yok → tüm-unit teslim, retransmit-YOK.
    #[test]
    fn run_arm_loopback_completes_no_retransmit() {
        let r = run_arm(Arm::A0, &cfg(&[]));
        assert_eq!(r.delivered_units, 8);
        assert!(r.completion_ns.is_some());
        assert_eq!(r.retransmits_emitted, 0);
        assert_eq!(r.data_emitted, 8);
    }

    /// FAITHFULNESS: A0 kayıp-source'u ARQ-retransmit ile kurtarır.
    #[test]
    fn run_arm_a0_deficit_recovers_via_retransmit() {
        let r = run_arm(Arm::A0, &cfg(&[2, 5]));
        assert_eq!(r.delivered_units, 8);
        assert!(
            r.retransmits_emitted > 0,
            "A0 retransmit-etmeden kurtardı (UNFAITHFUL)"
        );
    }

    /// FAITHFULNESS: A1 kayıp-source TARGET-SET'TEYSE duplicate ile kurtarır (0-retransmit) + A0'dan ERKEN.
    #[test]
    fn run_arm_a1_deficit_in_target_recovers_via_duplicate() {
        let arm = Arm::A1 { r: 3 };
        let target = run_dup_target(arm, &cfg(&[]));
        // Deficit = target-üyesi source (deficit-independent target önce hesaplandı; D onu-seçti).
        let victim = *target.iter().next().unwrap() as u64;
        let a0 = run_arm(Arm::A0, &cfg(&[victim]));
        let a1 = run_arm(arm, &cfg(&[victim]));
        assert_eq!(a1.delivered_units, 8);
        assert!(a1.duplicates_emitted > 0);
        assert_eq!(
            a1.retransmits_emitted, 0,
            "target-içi kayıp → dup-kurtarır, retransmit-YOK"
        );
        assert!(
            a1.completion_ns.unwrap() < a0.completion_ns.unwrap(),
            "A1 target-içi-kayıpta A0'dan erken bitmeli"
        );
    }

    /// HONEST BLIND-DUP: A1 kayıp TARGET-DIŞI ise duplicate KURTARMAZ → ARQ'ya düşer (A0-gibi retransmit).
    #[test]
    fn run_arm_a1_deficit_outside_target_falls_back_to_arq() {
        let arm = Arm::A1 { r: 2 };
        let target = run_dup_target(arm, &cfg(&[]));
        // Target-DIŞI bir source seç.
        let victim = (0..8u64).find(|u| !target.contains(&(*u as u16))).unwrap();
        let a1 = run_arm(arm, &cfg(&[victim]));
        assert_eq!(a1.delivered_units, 8, "yine de kurtarır (ARQ)");
        assert!(
            a1.retransmits_emitted > 0,
            "target-dışı kayıp → duplicate kurtarmaz, ARQ-retransmit-gerekir (blind-dup dürüst-davranışı)"
        );
    }

    /// TIMING LEDGER + INSTRUMENT: A0-deficit → t_source_loss/t_arq_arrival/t_block_complete dolu; admitted.
    #[test]
    fn run_arm_timing_ledger_and_instrument_chain() {
        let r = run_arm(Arm::A0, &cfg(&[2, 5]));
        assert!(
            r.timing.t_source_loss.is_some(),
            "t_source_loss (deterministic-drop) dolmalı"
        );
        assert!(
            r.timing.t_arq_wire.is_some() && r.timing.t_arq_arrival.is_some(),
            "ARQ-lifecycle dolmalı"
        );
        assert!(r.timing.t_block_complete.is_some());
        assert_eq!(
            r.timing.redundancy_kind,
            Some(RedundancyKind::None),
            "A0 = None"
        );
        // instrument-chain admitted.
        assert!(
            r.instrument.faithful,
            "realized-drop({}) != configured({})",
            r.instrument.realized_source_drops, r.instrument.configured_source_drops
        );
        assert!(r.instrument.overlap_ok, "overlap-gate (receiver_events>0)");
        assert!(r.instrument.admitted(), "instrument admitted olmalı");
        assert_eq!(r.instrument.configured_source_drops, 2);
    }

    /// A1 HIT: target-içi kayıp → hit=1/miss=0; t_repair_usable(dup-arrival) dolu; class=UsableBeforeArq.
    #[test]
    fn run_arm_a1_hit_ledger_and_usable_before_arq() {
        let arm = Arm::A1 { r: 3 };
        let target = run_dup_target(arm, &cfg(&[]));
        let victim = *target.iter().next().unwrap() as u64;
        let r = run_arm(arm, &cfg(&[victim]));
        assert_eq!(r.a1_dup_hits, 1, "target-içi-kayıp → hit");
        assert_eq!(r.a1_dup_misses, 0);
        assert!(
            r.timing.t_repair_usable.is_some(),
            "t_repair_usable (dup-arrival) dolmalı"
        );
        assert_eq!(r.timing.redundancy_kind, Some(RedundancyKind::Duplicate));
        // USABLE_BEFORE_ARQ t_repair_usable'dan (retransmit-gerekmedi → t_arq_arrival None; usable≤complete).
        assert_eq!(
            r.timing.parity_delay_class(),
            ParityDelayClass::UsableBeforeArq
        );
    }

    /// A1 MISS: target-dışı kayıp → hit=0/miss=1.
    #[test]
    fn run_arm_a1_miss_ledger() {
        let arm = Arm::A1 { r: 2 };
        let target = run_dup_target(arm, &cfg(&[]));
        let victim = (0..8u64).find(|u| !target.contains(&(*u as u16))).unwrap();
        let r = run_arm(arm, &cfg(&[victim]));
        assert_eq!(r.a1_dup_hits, 0);
        assert_eq!(r.a1_dup_misses, 1, "target-dışı-kayıp → miss");
    }

    /// TRACE-HASH DETERMINISM: aynı arm+cfg → identical trace-hash (replay-stable pairing-kanıtı).
    #[test]
    fn run_arm_trace_hash_deterministic() {
        let a = run_arm(Arm::A1 { r: 2 }, &cfg(&[2, 5]));
        let b = run_arm(Arm::A1 { r: 2 }, &cfg(&[2, 5]));
        assert_eq!(
            a.instrument.trace_hash, b.instrument.trace_hash,
            "trace-hash replay-stabil değil"
        );
        assert_eq!(a.instrument.config_hash, b.instrument.config_hash);
        // PATH-TIMING ledger-only mevcut (karar-tüketimi-yok; yalnız gözlem).
        let _ = a.path_timing.path_srtt_ns;
    }
}
