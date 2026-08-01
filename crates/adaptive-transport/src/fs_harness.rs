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

/// FEC-shard sealed-wire FRAMING (source VE repair İÇİN AYNI — INV-ENGINE-SOC: transport ikisini
/// BOYUTLA ayırt-EDEMEZ; tek fark sealed-payload İÇİNDEKİ 1-byte inner_kind codepoint'i, wire-boyu değil).
/// GROUNDED (uydurma-YOK; §8 ölçülmemiş-byte-yasağı): EXTENDED_OUTER_OVERHEAD(26: h_ext4+wfmt1+wflags1+
/// pn4+tag16) + FEC_INNER_HDR_LEN(11: iv1+kind1+flags1+pid1+block4+esi2+k1) = 37. Otorite: kama-noise-wire
/// `protector.rs` sealed_len(FecShard) = 26 + 11 + E ("N7: 101B @ E=64"). Sabit-const → ADR-17 deterministik.
pub const FS_SHARD_FRAMING_BYTES: u64 = 37;

/// Bir FEC-shard'ın (source|retransmit|duplicate|repair — HEPSİ) sealed-wire datagram boyu = symbol-E +
/// sabit-framing. TEK-DOMAIN: A1-dup, F0-parity, source, retransmit AYNI modelden serialize+sayılır
/// (C-P4 realized-wire adilliği + PIN-E egress + completion-timing hepsi tek sealed-byte-domain'inde).
#[must_use]
pub fn fs_sealed_len(symbol_e: u64) -> u64 {
    symbol_e.saturating_add(FS_SHARD_FRAMING_BYTES)
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

    /// **B1/B3 — reverse-path (ACK) kararı:** `true` = ACK DÜŞER (Sender feedback-almaz). Forward'dan AYRI.
    pub fn decide_reverse(&mut self, emit_time_ns: u64, ack_ordinal: u64) -> bool {
        self.trace.decide_reverse(emit_time_ns, ack_ordinal)
    }
    #[must_use]
    pub fn reverse_counts(&self) -> (u64, u64) {
        self.trace.reverse_counts()
    }
    #[must_use]
    pub fn reverse_decision_log_hash(&self) -> u64 {
        self.trace.reverse_decision_log_hash()
    }
    #[must_use]
    pub fn forward_schedule_id(&self) -> u64 {
        self.trace.forward_schedule_id()
    }
    #[must_use]
    pub fn reverse_schedule_id(&self) -> u64 {
        self.trace.reverse_schedule_id()
    }
    #[must_use]
    pub fn configured_schedule_hash(&self) -> u64 {
        self.trace.configured_schedule_hash()
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

use crate::fs_trace::{RedundancyPolicy, ReverseSpec, TraceSpec};

/// **B2 — deficit source-position sınıfı** (tail-erasure karakterizasyonu; analiz tail'i universal-FEC-
/// avantajından ayırabilsin diye). k=source_count; TAIL = son-unit(k−1) düşük; HEAD = ilk-unit(0); MIXED = birden-çok-strata.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DeficitPosition {
    /// Deficit-yok.
    #[default]
    None,
    /// İlk source-unit (esi 0) bölgesi (largest_acked hızlı-ilerler → recovery-fair).
    Head,
    /// Orta source-unit'ler (ne ilk ne son).
    Middle,
    /// SON source-unit (esi k−1) — seq>largest_acked yapısal tail-erasure sınıfı.
    Tail,
    /// Birden-çok strata (head+tail vb.).
    Mixed,
}

/// **B2 — tail-erasure'ın hangi yolla (varsa) kurtarıldığı** (yalnız-gözlem; Sender-fix YOK).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TailRecoveryPath {
    /// Kurtarılamadı (yapısal tail-erasure stall; B_REAL_TAIL_ERASURE).
    #[default]
    NoneStalled,
    /// ARQ-retransmit ile kurtarıldı (tail-DEĞİL ya da later-ACK mevcuttu).
    Arq,
    /// A1 duplicate ile kurtarıldı.
    Duplicate,
}
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
    /// **B1/B3 — reverse-path (ACK-yönü) impairment şeması** (blackout/loss). Clear = Harness-A davranışı.
    pub reverse: ReverseSpec,
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
    /// **B1/B3 — reverse-path provenance (forward'dan AYRI-görünür).**
    pub forward_schedule_id: u64,
    pub reverse_schedule_id: u64,
    /// B3 pairing-kimliği: forward+reverse CONFIGURED-şema (kollar-arası ÖZDEŞ olmalı; realizasyon-DEĞİL).
    pub configured_schedule_hash: u64,
    pub reverse_dropped: u64,
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
    /// **B2 — tail-erasure karakterizasyonu (yalnız-gözlem; Sender-fix YOK).**
    pub deficit_position: DeficitPosition,
    pub deficit_is_tail: bool,
    /// source_count = num_units (K).
    pub source_count: u64,
    /// Loss-detection anında gözlemlenen largest-acked (harness-observable; tail-erasure kanıtı: k−2'de takılır).
    pub largest_acked_observed: Option<u64>,
    pub tail_recovery_path: TailRecoveryPath,
    /// **§8 TOTAL-OUTCOME-COST (Pareto/oracle; INTERVENTION-BUDGET'ten AYRI):** ARQ-retransmit wire-byte'ları.
    pub arq_retransmit_bytes: u64,
    /// FEC/dup erken-tamamladıysa emitilen retransmit'ler SPURIOUS (hybrid-coordination-cost; BUDGET_MISMATCH-DEĞİL).
    pub spurious_retransmits: u64,
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
                res.arq_retransmit_bytes += d.size; // §8 total-outcome-cost (sealed wire)
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
            // TEK-DOMAIN: d.size = sealed datagram (fs_sealed_len; F0-parity ile AYNI). payload + framing
            // == realized == d.size (A1_WIRE_ACCOUNTING_REAL_DATAGRAM; framing artık 0-DEĞİL).
            res.cost.realized_sealed_wire_bytes += d.size;
            res.cost.redundancy_payload_bytes += unit_size;
            res.cost.redundancy_framing_bytes += d.size.saturating_sub(unit_size);
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
    let trace = TraceChannel::with_reverse(
        TraceSpec::ExactDeficit {
            drop_initial: drop_set,
            redundancy: RedundancyPolicy::PassAll,
        },
        cfg.reverse.clone(), // B1/B3: reverse-path (ACK) impairment
    );
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
    let mut ack_ordinal = 0u64; // B1 reverse-loss iid-anahtarı (monoton ACK-sayacı)
    let mut largest_acked_observed: Option<u64> = None; // B2 tail-erasure kanıtı

    let mut d0 = Vec::new();
    for u in 0..cfg.num_units as u64 {
        d0.extend(s.on_event(
            Event::AppData(OpaqueUnit {
                id: u,
                // OpaqueUnit.size = güvenlik-katmanının sealed ct||tag blob wire-boyu (sender.rs:74-82;
                // TransmitDecision.size ondan). E-symbol DEĞİL → fs_sealed_len (Sender wire-reality'de pace-eder).
                size: fs_sealed_len(cfg.unit_size),
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
                // B1/B3 REVERSE-PATH: ACK receiver→sender'da `arr` anında emit. Reverse-blackout/loss
                // içindeyse DÜŞER → Sender feedback-almaz (largest_acked ilerlemez; ARQ-clock durur — C-P3).
                let ack_dropped = ch.decide_reverse(arr, ack_ordinal);
                ack_ordinal += 1;
                if !ack_dropped {
                    acks.push((arr + cfg.owd_ns, seq));
                    largest_acked_observed =
                        Some(largest_acked_observed.map_or(unit, |m| m.max(unit)));
                }
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
    // B2 — tail-erasure karakterizasyonu (yalnız-gözlem; Sender-fix YOK).
    res.source_count = cfg.num_units as u64;
    res.deficit_position = classify_deficit_position(&cfg.drop_initial_units, cfg.num_units as u64);
    res.deficit_is_tail = matches!(res.deficit_position, DeficitPosition::Tail);
    res.largest_acked_observed = largest_acked_observed;
    res.tail_recovery_path = if res.a1_dup_hits > 0 {
        TailRecoveryPath::Duplicate
    } else if res.retransmits_emitted > 0 && res.delivered_units == cfg.num_units {
        TailRecoveryPath::Arq
    } else {
        TailRecoveryPath::NoneStalled // tamamlanmadıysa (tail-erasure) veya recovery-yoksa
    };
    // §8 spurious: dup erken-tamamladıysa (a1-hit) emitilen ARQ-retransmit'ler SPURIOUS (hybrid-cost).
    res.spurious_retransmits = if res.a1_dup_hits > 0 {
        res.retransmits_emitted
    } else {
        0
    };
    // Per-run instrument-chain (+ B1/B3 reverse provenance ayrı-görünür).
    let (_, realized_drops) = ch.trace_counts();
    let (_, reverse_dropped) = ch.reverse_counts();
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
        forward_schedule_id: ch.forward_schedule_id(),
        reverse_schedule_id: ch.reverse_schedule_id(),
        configured_schedule_hash: ch.configured_schedule_hash(),
        reverse_dropped,
    };
    res
}

/// **B2 — deficit source-position sınıflandırması** (HEAD/MIDDLE/TAIL; tail = son-unit k−1 ∈ D).
#[must_use]
pub fn classify_deficit_position(drop: &BTreeSet<u64>, k: u64) -> DeficitPosition {
    if drop.is_empty() || k == 0 {
        return DeficitPosition::None;
    }
    let last = k - 1;
    let has_tail = drop.contains(&last);
    let has_head = drop.contains(&0);
    let has_middle = drop.iter().any(|&u| u != 0 && u != last);
    match (has_head, has_middle, has_tail) {
        (_, _, true) if drop.len() == 1 => DeficitPosition::Tail,
        (true, false, false) if drop.len() == 1 => DeficitPosition::Head,
        (false, true, false) if drop.len() == 1 => DeficitPosition::Middle,
        // Çoklu-strata: tail dahilse yine TAIL-riski taşır ama Mixed etiketlenir (analiz stratify-eder).
        _ => DeficitPosition::Mixed,
    }
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
    // B1/B3: reverse-şema config-kimliğine dahil (aynı-forward + farklı-reverse → farklı config-hash).
    let rc = TraceChannel::with_reverse(
        TraceSpec::NaturalIid {
            loss_permille: 0,
            seed: 0,
        },
        cfg.reverse.clone(),
    );
    k.extend_from_slice(&rc.reverse_schedule_id().to_le_bytes());
    fnv1a(&k)
}

// ── PIN-C MECHANISM FIXTURE (F1/F3 parity wire-time) ─────────────────────────────────────────────

/// **PIN-C mechanism modeli:** real-Sender'ın DATA-first + pacer parity-emisyonu (schedule_transmit'te
/// plan_fec_parity, DATA-drain SONRASI — sender.rs:2290 mühürlü; pacer-token-gated). Parity kendi-block'unun
/// önündeki DATA'yı wire'da GEÇEMEZ → `t_parity_wire = data_before_parity × serialize_ns`. `t_parity_eligible`
/// (logical-close anı) AYRIDIR — F1 onu ilerletir ama wire'ı ilerletemez (STRUCTURALLY_NON_EFFECTIVE).
#[derive(Debug, Clone, Copy)]
pub struct ParityWireModel {
    pub link_bps: u64,
    pub sealed_len: u64,
}
impl ParityWireModel {
    #[must_use]
    pub fn serialize_ns(&self) -> u64 {
        self.sealed_len
            .saturating_mul(8)
            .saturating_mul(1_000_000_000)
            / self.link_bps.max(1)
    }
    /// t_parity_wire: parity, block'un `data_before_parity` DATA'sı serialize edildikten SONRA wire'a çıkar.
    #[must_use]
    pub fn t_parity_wire(&self, data_before_parity: u64) -> u64 {
        data_before_parity.saturating_mul(self.serialize_ns())
    }
}

/// **PIN-C sonucu** (F1/F3 candidate mekanizması).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinCResult {
    /// `t_parity_wire(candidate) < t_parity_wire(F0)` → parity WIRE-time ilerledi (ordering-mührü korunarak): EFEKTİF.
    WireTimeAdvanced,
    /// Eligibility ilerledi AMA t_parity_wire ilerlemedi (DATA-first/pacer-gated) → STRUCTURALLY NON-EFFECTIVE
    /// UNDER CURRENT ORDERING. (Sıradan performance-failure DEĞİL; ordering-mührü GEVŞETİLMEZ.)
    StructurallyNonEffectiveUnderCurrentOrdering,
}

/// PIN-C bağlayıcı-değerlendirme: candidate parity WIRE-time'ını F0'a göre ilerletti mi? (eligibility TEK-BAŞINA
/// yetmez — wire ŞART.)
#[must_use]
pub fn pin_c_classify(t_parity_wire_f0: u64, t_parity_wire_candidate: u64) -> PinCResult {
    if t_parity_wire_candidate < t_parity_wire_f0 {
        PinCResult::WireTimeAdvanced
    } else {
        PinCResult::StructurallyNonEffectiveUnderCurrentOrdering
    }
}

// ── §17/§18 DEADLINE GEOMETRY (arm-independent; ratified formula; v1.3-DONDURULMADI) ─────────────

/// **§17 — effective source-egress rate** = min(configured source-pacer, bottleneck capacity) (bits/s;
/// BottleneckChannel.link_bps ile AYNI birim). util=0.80C → pacer = 0.80×C; capacity = C → effective = 0.80C.
/// (NOT: harness'ın source'u GERÇEKTEN 0.80C'de pace-ettiği kanıtı 6-arm-spine-source-emission'ında ölçülür.)
#[must_use]
pub fn fs_effective_source_bps(source_pacer_bps: u64, bottleneck_capacity_bps: u64) -> u64 {
    source_pacer_bps.min(bottleneck_capacity_bps)
}

/// Ortak source-serialization envelope süresi (ns): total_base_source_sealed_bytes × 8 bit / effective_bps.
/// TÜM-arm için AYNI (48×101=4848B); proaktif-intervention HARİÇ (arm-independence).
#[must_use]
pub fn fs_t_serialize_source_ns(total_base_source_sealed_bytes: u64, effective_bps: u64) -> u64 {
    total_base_source_sealed_bytes
        .saturating_mul(8)
        .saturating_mul(1_000_000_000)
        / effective_bps.max(1)
}

/// **§18 — arm-independent deadline (ms; ceil):** T_serialize_source + N_recovery×RTT + M_fixed.
/// N_recovery=2, M_fixed=50ms (OUTCOME-BLIND inherited design-constants; §19). Proaktif-intervention deadline'a
/// GİRMEZ (completion-time'a/hit'e/cost'a GİRER). ceil(exact) = ceil(t_ser_ms) + N×RTT + M (N×RTT+M integer).
#[must_use]
pub fn fs_deadline_ms(total_base_source_sealed_bytes: u64, effective_bps: u64, rtt_ms: u64) -> u64 {
    const N_RECOVERY: u64 = 2;
    const M_FIXED_MS: u64 = 50;
    let t_ser_ms_ceil =
        fs_t_serialize_source_ns(total_base_source_sealed_bytes, effective_bps).div_ceil(1_000_000);
    t_ser_ms_ceil + N_RECOVERY * rtt_ms + M_FIXED_MS
}

// ── F4 ADAPTIVE-K MECHANISM (MECHANISTIC/EXPLORATORY) ────────────────────────────────────────────

/// **F4 seçilen block-K (OPTION-D; v1.3-aday).** YALNIZ codec-realize-edilebilir değerler — K=1/K=3 YOK, ordinal YOK.
/// E=64 profiller: K2→0x01 · K4→0x02 · K8→0x03 (kama-fec-codec katalog). `k()` gerçek-block-K; `profile_id()` codec-profil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecK {
    K2,
    K4,
    K8,
}
impl CodecK {
    /// Gerçek block-K değeri (num source-symbols/blok).
    #[must_use]
    pub fn k(self) -> u16 {
        match self {
            CodecK::K2 => 2,
            CodecK::K4 => 4,
            CodecK::K8 => 8,
        }
    }
    /// E=64 codec-profil id (source==repair; profile.rs 0x01/0x02/0x03). Katalog-varlığı kama-integration'da pinlenir.
    #[must_use]
    pub fn profile_id(self) -> u8 {
        match self {
            CodecK::K2 => 0x01,
            CodecK::K4 => 0x02,
            CodecK::K8 => 0x03,
        }
    }
}

/// F4 confidence-öncesi FROZEN muhafazakâr-K (region-2 default → K=4; codec-realize-edilebilir).
pub const F4_K_COLD: CodecK = CodecK::K4;

/// **F4 — OPTION-D DIRECT-K seçim-kuralı (v1.3-aday; pre-data düzeltme; sonuç-görülmeden).** DOĞRUDAN
/// codec-realize-edilebilir block-K döner (ordinal-{1,2,3} YOK; K=1/K=3 YOK — v1.2-literal-K-realizability-gap
/// kapatıldı). Base-RTT bölge-sınırları Harness-B/v1.2 ile AYNI: region1(<50ms)→K2 · region2(50-150ms)→K4 ·
/// region3(≥150ms)→K8. Confidence-kapısı: sample+rtprop-fresh değilse K_COLD(K4). MECHANISTIC/EXPLORATORY —
/// discriminative-validity/online-selectability/controller-readiness KURMAZ (FS-5B). ADR-17 saf-deterministik
/// (integer-only; RNG-YOK; GELECEK/outcome-bilgi-YOK). YALNIZ F4 PathTimingState tüketir.
#[must_use]
pub fn f4_select_k(pt: &crate::sender::PathTimingState) -> CodecK {
    if !(pt.sample_present && pt.rtprop_fresh) {
        return F4_K_COLD;
    }
    let Some(base) = pt.min_rtt_ns else {
        return F4_K_COLD;
    };
    match base {
        0..=49_999_999 => CodecK::K2, // region1 <50ms → K=2 (profil 0x01)
        50_000_000..=149_999_999 => CodecK::K4, // region2 50-150ms → K=4 (profil 0x02)
        _ => CodecK::K8,              // region3 ≥150ms → K=8 (profil 0x03)
    }
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
        let sealed = [0xABu8; 101]; // GROUNDED sealed FEC-shard @ E=64 (26+11+64; protector.rs N7)
        let targets: Vec<u16> = emitter.target_set().iter().copied().collect();
        for esi in &targets {
            emitter.maybe_emit_duplicate((0, *esi), &sealed, 64, &mut sink);
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
        let sealed = [0u8; 101]; // GROUNDED sealed FEC-shard @ E=64 (fs_sealed_len(64)=101)
        let targets: Vec<u16> = emitter.target_set().iter().copied().collect();
        for esi in &targets {
            emitter.maybe_emit_duplicate((0, *esi), &sealed, 64, &mut sink);
        }
        let cost = emitter.cost();
        assert_eq!(cost.redundancy_packet_count, 2);
        assert_eq!(cost.realized_sealed_wire_bytes, 2 * fs_sealed_len(64)); // 2×101
        assert_eq!(cost.redundancy_payload_bytes, 128); // 2×64 (E)
        assert_eq!(cost.redundancy_framing_bytes, 2 * FS_SHARD_FRAMING_BYTES); // 2×37
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
            reverse: ReverseSpec::Clear,
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

    /// **B2 — B_REAL_TAIL_ERASURE deterministik pin:** A0 tail-deficit (son-unit) → PTO sadık-servis →
    /// horizon-uzatma KURTARMAZ (yapısal, timeout-DEĞİL) → stall (delivered=k−1, completion=None,
    /// retransmit=0, largest_acked=k−2). Harness-defekt DEĞİL (admitted). MIDDLE-deficit KURTARILIR (kontrast).
    #[test]
    fn b2_tail_erasure_is_real_not_harness_defect() {
        let k = 8u64;
        let tail = run_arm(Arm::A0, &cfg(&[k - 1]));
        assert_eq!(tail.deficit_position, DeficitPosition::Tail);
        assert!(tail.deficit_is_tail);
        assert!(
            tail.completion_ns.is_none(),
            "tail-erasure yapısal-olarak tamamlanmamalı"
        );
        assert_eq!(tail.delivered_units as u64, k - 1, "delivered = k−1");
        assert_eq!(
            tail.retransmits_emitted, 0,
            "loss-classify seq<largest_acked ister → tail retransmit-YOK"
        );
        assert_eq!(
            tail.largest_acked_observed,
            Some(k - 2),
            "largest_acked k−2'de takılır (son-unit ACK'lenmez)"
        );
        assert_eq!(tail.tail_recovery_path, TailRecoveryPath::NoneStalled);
        assert!(
            tail.instrument.admitted(),
            "harness-geçerli (PTO sadık-servis-edildi; INVALID/defekt DEĞİL)"
        );
        // HORIZON-UZATMA ÇÖZMEZ (yapısal-önkoşul, timeout değil).
        let mut long = cfg(&[k - 1]);
        long.horizon_ns *= 3;
        let tail_long = run_arm(Arm::A0, &long);
        assert!(
            tail_long.completion_ns.is_none(),
            "horizon-uzatma tail-erasure'ı çözdü → yapısal-değilmiş (B sınıflandırması hatalı)"
        );
        // KONTRAST — MIDDLE-deficit ARQ ile KURTARILIR (later-ACK mevcut → loss-classify çalışır).
        let mid = run_arm(Arm::A0, &cfg(&[3]));
        assert_eq!(mid.deficit_position, DeficitPosition::Middle);
        assert_eq!(
            mid.delivered_units as u64, k,
            "middle-deficit ARQ ile tamamlanır"
        );
        assert!(mid.retransmits_emitted > 0);
        assert_eq!(mid.tail_recovery_path, TailRecoveryPath::Arq);
    }

    /// **B3 — PAIRED reverse-blackout:** A0/A1 AYNI (forward+reverse) şema → configured_schedule_hash ÖZDEŞ
    /// (channel-intervention identity; kol-çıktısı/realizasyon farklı olabilir). Reverse-blackout GERÇEKTEN
    /// ACK düşürür (feedback-starvation). Aynı-seed TEK-BAŞINA değil — şema-kimliği pairing kanıtı.
    #[test]
    fn b3_paired_reverse_blackout_channel_intervention_identical() {
        let mut c = cfg(&[2]);
        c.reverse = ReverseSpec::Scheduled {
            blackout_intervals_ns: vec![(30 * MS, 400 * MS)],
            loss_permille: 0,
            seed: 0,
        };
        let a0 = run_arm(Arm::A0, &c);
        let a1 = run_arm(Arm::A1 { r: 2 }, &c);
        assert_eq!(
            a0.instrument.configured_schedule_hash, a1.instrument.configured_schedule_hash,
            "paired-trace configured-şema kollar-arası farklı (B3 ihlali)"
        );
        assert_eq!(
            a0.instrument.reverse_schedule_id,
            a1.instrument.reverse_schedule_id
        );
        assert_eq!(
            a0.instrument.forward_schedule_id,
            a1.instrument.forward_schedule_id
        );
        assert!(
            a0.instrument.reverse_dropped > 0,
            "reverse-blackout hiç-ACK-düşürmedi (window ıskaladı)"
        );
        // Clear-reverse ≠ blackout-reverse: reverse_schedule_id FARKLI (ayrı-görünürlük).
        let clear = run_arm(Arm::A0, &cfg(&[2]));
        assert_ne!(
            clear.instrument.reverse_schedule_id, a0.instrument.reverse_schedule_id,
            "Clear ve Scheduled reverse-şema aynı-id (ayrımsız)"
        );
    }

    /// **F1 PIN-C fixture — STRUCTURALLY_NON_EFFECTIVE:** early-logical-close eligibility'yi ilerletir AMA
    /// DATA-first parity'yi block-DATA'nın ARDINDA tutar (sender.rs:2290 mühür) → t_parity_wire ilerlemez.
    /// Ordering-mührü GEVŞETİLMEZ; sahte-başarı YOK; sıradan-fail olarak GİZLENMEZ.
    #[test]
    fn f1_pin_c_structurally_non_effective_under_ordering() {
        let m = ParityWireModel {
            link_bps: 1_000_000,
            sealed_len: fs_sealed_len(64),
        };
        let k = 8u64;
        // F0: later-trigger close → eligible = wire = K-DATA sonrası.
        let (elig_f0, wire_f0) = (m.t_parity_wire(k), m.t_parity_wire(k));
        // F1: early logical-close → eligible ilerledi (≈0) AMA wire hâlâ K-DATA-first (DEĞİŞMEDİ).
        let elig_f1 = 0u64;
        let wire_f1 = m.t_parity_wire(k); // parity block'un K DATA'sını GEÇEMEZ (DATA-first).
        assert!(
            elig_f1 < elig_f0,
            "F1 eligibility ilerlemedi (fixture-kurgusu hatalı)"
        );
        assert_eq!(
            wire_f1, wire_f0,
            "F1 wire F0'dan sapmamalı (ordering-mührü)"
        );
        assert_eq!(
            pin_c_classify(wire_f0, wire_f1),
            PinCResult::StructurallyNonEffectiveUnderCurrentOrdering,
            "F1 wire-time ilerletmediği hâlde EFEKTİF raporlandı (sahte-başarı)"
        );
    }

    /// **F3 PIN-C fixture — WireTimeAdvanced:** deadline partial-block (K'<K) close → parity'nin ÖNÜNDE
    /// daha-az DATA → t_parity_wire(F3) < t_parity_wire(F0). Ordering-mührü korunur (interleaved-YOK).
    #[test]
    fn f3_pin_c_partial_block_advances_wire_time() {
        let m = ParityWireModel {
            link_bps: 1_000_000,
            sealed_len: fs_sealed_len(64),
        };
        let (k, k_partial) = (8u64, 4u64);
        let wire_f0 = m.t_parity_wire(k); // parity after 8 DATA
        let wire_f3 = m.t_parity_wire(k_partial); // partial parity after 4 DATA
        assert!(
            wire_f3 < wire_f0,
            "partial-block parity daha-erken wire'a çıkmalı"
        );
        assert_eq!(
            pin_c_classify(wire_f0, wire_f3),
            PinCResult::WireTimeAdvanced
        );
    }

    /// **§17/§18 DEADLINE-GEOMETRY pin (v1.3-aday; frozen-DEĞİL):** ortak 48×101=4848B source-envelope @ effective
    /// 0.80C (C=1Mbps bits/s) → arm-independent deadline 199/599/2099ms (ratified §18). effective=min(pacer,capacity).
    #[test]
    fn deadline_geometry_arm_independent_ratified_values() {
        let capacity_bps = 1_000_000u64; // C = 1 Mbps (bits/s; BottleneckChannel birimi)
        let pacer_bps = capacity_bps * 80 / 100; // util=0.80C → 800_000 bits/s
        let effective = fs_effective_source_bps(pacer_bps, capacity_bps);
        assert_eq!(effective, 800_000, "effective = min(0.80C, C) = 0.80C");
        let base_source = 48 * fs_sealed_len(64); // 4848B (arm-independent; intervention HARİÇ)
        assert_eq!(base_source, 4848);
        // T_serialize_source = 4848×8 / 800_000 s = 48.48ms.
        let t_ser_ns = fs_t_serialize_source_ns(base_source, effective);
        assert_eq!(t_ser_ns, 48_480_000, "48.48ms source-serialization");
        // Arm-independent deadlines (ceil): RTT{50,250,1000} → {199,599,2099}.
        assert_eq!(fs_deadline_ms(base_source, effective, 50), 199);
        assert_eq!(fs_deadline_ms(base_source, effective, 250), 599);
        assert_eq!(fs_deadline_ms(base_source, effective, 1000), 2099);
        // C-P2 future RTT=2500 → 5099.
        assert_eq!(fs_deadline_ms(base_source, effective, 2500), 5099);
    }

    /// **F4 mechanism pin (PIN-D):** iki-FARKLI prereg path-state → FARKLI K (frozen-kural). No-future-leakage
    /// (yalnız-snapshot; deterministik: aynı-state → aynı-K). MECHANISTIC/EXPLORATORY (validity-iddiası YOK).
    #[test]
    fn f4_adaptive_k_two_states_differ_no_leakage() {
        use crate::sender::PathTimingState;
        let short = PathTimingState {
            srtt_ns: Some(30_000_000),
            min_rtt_ns: Some(30_000_000),
            rttvar_ns: 1_000_000,
            queue_delay_ns: 0,
            sample_present: true,
            rtprop_fresh: true,
        };
        let long = PathTimingState {
            srtt_ns: Some(220_000_000),
            min_rtt_ns: Some(220_000_000),
            ..short
        };
        let (k_short, k_long) = (f4_select_k(&short), f4_select_k(&long));
        assert_eq!(
            k_short,
            CodecK::K2,
            "region1 base<50ms → K2 (OPTION-D direct-K)"
        );
        assert_eq!(
            k_long,
            CodecK::K8,
            "region3 base≥150ms → K8 (OPTION-D direct-K)"
        );
        assert_ne!(
            k_short, k_long,
            "iki-path-state aynı-K (mechanism ayrışmıyor)"
        );
        // No-leakage / determinism: aynı-snapshot iki-kez → AYNI K (RNG/future-YOK).
        assert_eq!(f4_select_k(&short), k_short);
        // Confidence-kapısı: fresh-değil → K_COLD=K4 (base yüksek olsa bile).
        let cold = PathTimingState {
            sample_present: false,
            ..long
        };
        assert_eq!(f4_select_k(&cold), F4_K_COLD, "confidence-kapısı açılmadı");
        // OPTION-D REALIZABILITY: her seçim codec-realize-edilebilir block-K (∈{2,4,8}); K=1/K=3 ÜRETİLEMEZ.
        for ck in [CodecK::K2, CodecK::K4, CodecK::K8] {
            assert!(matches!(ck.k(), 2 | 4 | 8), "illegal block-K");
            assert!(matches!(ck.profile_id(), 0x01..=0x03), "illegal profile");
        }
    }

    /// A1_WIRE_ACCOUNTING_REAL_DATAGRAM (review-blocking-pin): bir duplicate için payload + framing ==
    /// total-datagram == BottleneckChannel-serialization — HEPSİ AYNI sealed-domain (fs_sealed_len;
    /// source==repair). Eski-defekt: A1 raw-payload (framing=0) sayıyordu, F0-parity framed → domain-uyuşmazlığı.
    #[test]
    fn a1_wire_accounting_real_datagram() {
        let c = cfg(&[]); // deficit-yok → loss/retransmit-yok (unit_size=300 E-symbol).
        let r = run_arm(Arm::A1 { r: 1 }, &c);
        assert_eq!(r.duplicates_emitted, 1, "r=1 → blok-total 1 dup");
        let sealed = fs_sealed_len(c.unit_size); // 337 = 300 (E) + 37 (grounded framing)
                                                 // (1) accounting: payload + framing == realized == sealed datagram (framing artık 0-DEĞİL).
        assert_eq!(
            r.cost.redundancy_payload_bytes, c.unit_size,
            "payload = E-symbol"
        );
        assert_eq!(
            r.cost.redundancy_framing_bytes, FS_SHARD_FRAMING_BYTES,
            "framing = grounded 37B (0-DEĞİL)"
        );
        assert_eq!(
            r.cost.redundancy_payload_bytes + r.cost.redundancy_framing_bytes,
            r.cost.realized_sealed_wire_bytes,
            "payload+framing != realized (domain-tutarsız)"
        );
        assert_eq!(
            r.cost.realized_sealed_wire_bytes, sealed,
            "realized != sealed datagram"
        );
        // (2) channel-serialization AYNI domain: loss-yok → 8 source + 1 dup = 9 datagram × sealed.
        assert_eq!(r.data_emitted, 8);
        assert_eq!(r.retransmits_emitted, 0);
        assert_eq!(
            r.channel_serialized_bytes,
            9 * sealed,
            "channel dup'u sealed-domain'de saymadı (F0-parity ile aynı-birim değil)"
        );
    }
}
