//! DR-FEC-FS-1 FS-HARNESS-A — A1-EARLY runner-level source-duplication emitter + redundancy cost-accounting.
//!
//! **A1-EARLY = same-byte early redundancy baseline (coding-DEĞİL).** Runner-seviyesi: Sender bir source-DATA
//! emisyonu verince, sealed-source-datagram'ı r-kez early-timing'de egress-path'e (sink) verir. Receiver
//! zaten dedup-eder.
//!
//! **PIN-A — DIRECT EMISSION-SIDE STATE NEUTRALITY (Abkarada netleştirmesi):** emisyon-EYLEMİ Sender-state'ini
//! DOĞRUDAN değiştirmez. [`A1EarlyEmitter`] Sender'a **referans TUTMAZ** → mutasyon TİP-DÜZEYİNDE imkânsız.
//! Test: emisyon-öncesi snapshot → dup-emit → **feedback'ten ÖNCE** snapshot → özdeş. Duplicate'in daha-sonra
//! gerçek-receiver-feedback'iyle (erken-ACK/decode) yarattığı state-değişimi müdahalenin MEŞRU-nedensel-sonucudur;
//! yasaklanmaz (A1-vs-A0 post-feedback state-eşitliği İSTENMEZ).
//!
//! **PIN-E — EGRESS-FAIRNESS:** her kopya sink'e (gerçek-egress; channel-loss + serialization + accounting)
//! yazılır; bedava-unmetered-lane YOK. Realized-sealed-wire-bytes ayrı kaydedilir (C-P4 fairness = realized-
//! wire, configured_r DEĞİL — A1 r=2 ≠ FEC r=2 sealed-byte; header/framing farkı).
//!
//! **D-INDEPENDENT selection:** emitter hangi source'u kopyalayacağını PREDECLARED bilir; trace-drop-set `D`'ye
//! REFERANS TUTMAZ (future-loss-knowledge-YOK) — yapısal (emitter'da D-alanı yok).

use crate::fs_trace::{TraceChannel, TracePacketId, TransmissionRole};

/// Redundancy'nin türü (arm-neutral timing-ledger + accounting eşlemesi için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedundancyKind {
    /// A0: proaktif-redundancy yok.
    None,
    /// A1-EARLY: same-byte source-duplication.
    Duplicate,
    /// F0/F1/F3/F4: coded parity/repair.
    FecRepair,
}

/// Bir kolun redundancy sealed-wire maliyeti — A1/F0 AYNI physical-accounting-domain'de (Abkarada
/// integration-rule-2). **C-P4 fairness `realized_sealed_wire_bytes` üstünden** (configured_r DEĞİL;
/// payload ≠ sealed; A1-duplicate framing'i FEC-parity framing'inden farklı olabilir).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RedundancyCost {
    pub configured_r: u16,
    /// Redundancy payload (header/framing-hariç).
    pub redundancy_payload_bytes: u64,
    /// Framing/header byte'ları (sealed − payload).
    pub redundancy_framing_bytes: u64,
    /// GERÇEK sealed-wire (payload + framing) — **C-P4 nihai fairness metriği**.
    pub realized_sealed_wire_bytes: u64,
    pub redundancy_packet_count: u64,
}

/// Paket-yutağı (channel/socket egress soyutlaması). Emitter YALNIZ buna yazar — Sender'a dokunmaz.
pub trait PacketSink {
    /// Bir paketi (semantic-id + sealed-bytes) egress-path'e verir (channel-loss + serialization + accounting).
    fn send(&mut self, id: TracePacketId, sealed: &[u8]);
}

/// A1-EARLY runner-level proactive source-duplication emitter.
///
/// PIN-A: Sender-REFERANSI YOK → emisyon-eylemi Sender-state'ini yapısal-olarak değiştiremez.
/// PIN-E: her kopya sink'e yazılır + exact-accounting. D-INDEPENDENT: D-alanı yok (predeclared target).
pub struct A1EarlyEmitter {
    configured_r: u16,
    cost: RedundancyCost,
}

impl A1EarlyEmitter {
    #[must_use]
    pub fn new(configured_r: u16) -> Self {
        Self {
            configured_r,
            cost: RedundancyCost {
                configured_r,
                ..RedundancyCost::default()
            },
        }
    }

    /// Bir source'un `configured_r` proaktif-kopyasını sink'e verir (early per-source timing; attempt≥1).
    /// `payload_len` = source payload (header-hariç accounting). **Sender-parametresi YOK (PIN-A)** —
    /// yalnız sink + accounting. Kaç-kopya `configured_r`'den; hangi-source predeclared (D-bağımsız).
    pub fn emit_duplicates<S: PacketSink>(
        &mut self,
        source_id: (u32, u16),
        sealed: &[u8],
        payload_len: u64,
        sink: &mut S,
    ) {
        for i in 0..self.configured_r {
            let id = TracePacketId {
                source_id,
                role: TransmissionRole::Duplicate,
                attempt: i + 1,
            };
            sink.send(id, sealed); // PIN-E: gerçek egress-path (bedava-lane-yok)
            let sealed_len = sealed.len() as u64;
            self.cost.realized_sealed_wire_bytes += sealed_len;
            self.cost.redundancy_payload_bytes += payload_len;
            self.cost.redundancy_framing_bytes += sealed_len.saturating_sub(payload_len);
            self.cost.redundancy_packet_count += 1;
        }
    }

    #[must_use]
    pub fn cost(&self) -> RedundancyCost {
        self.cost
    }
}

/// A1-EARLY emisyon-sırası politikası (deterministik; async-scheduling'e BIRAKILMAZ — Abkarada).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedundancyEmissionPolicy {
    /// `source_i → configured_r duplicate_i → source_{i+1}` (early per-source adjacent).
    PerSourceAdjacentEarly,
}

/// PER_SOURCE_ADJACENT_EARLY emisyon-dizisini üretir (deterministik; aynı-girdi → aynı-çıktı).
/// "EARLY"nin tam-tanımı: her source'un kopyaları source'un HEMEN-ARDINDAN, sonraki-source'tan-ÖNCE.
#[must_use]
pub fn a1_emission_order(
    sources: &[(u32, u16)],
    configured_r: u16,
    policy: RedundancyEmissionPolicy,
) -> Vec<TracePacketId> {
    let RedundancyEmissionPolicy::PerSourceAdjacentEarly = policy;
    let mut out = Vec::with_capacity(sources.len() * (1 + configured_r as usize));
    for &sid in sources {
        out.push(TracePacketId {
            source_id: sid,
            role: TransmissionRole::InitialSource,
            attempt: 0,
        });
        for i in 0..configured_r {
            out.push(TracePacketId {
                source_id: sid,
                role: TransmissionRole::Duplicate,
                attempt: i + 1,
            });
        }
    }
    out
}

/// Arm-neutral redundancy timing-ledger (Abkarada integration-rule-1). Legacy FEC-specific `t_parity_*`
/// KORUNUR (F* için); generic `t_redundancy_*` HER-arm için (A1→duplicate-lifecycle · F*→parity-lifecycle).
/// **SALT-GÖZLEM: arm-davranışını ETKİLEMEZ.** `t_source_loss` (ExactDeficit) = deterministic TraceChannel
/// INITIAL-source DROP-anı (ACK-tahmini DEĞİL). C-P4: "coding mı, timing mi?" bu ledger'dan okunur.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BlockTiming {
    pub block_id: u32,
    pub redundancy_kind: Option<RedundancyKind>,
    /// ExactDeficit: deterministic INITIAL-source DROP-anı (ACK-tahmini değil).
    pub t_source_loss: Option<u64>,
    // ── legacy FEC-specific (yalnız FecRepair'de anlamlı; geriye-uyumluluk) ──
    pub t_parity_eligible: Option<u64>,
    pub t_parity_enqueued: Option<u64>,
    pub t_parity_wire: Option<u64>,
    pub t_parity_arrival: Option<u64>,
    pub t_arq_eligible: Option<u64>,
    pub t_arq_wire: Option<u64>,
    pub t_arq_arrival: Option<u64>,
    pub t_block_complete: Option<u64>,
    // ── generic arm-neutral redundancy lifecycle (A1 + F* karşılaştırılabilir) ──
    pub t_redundancy_eligible: Option<u64>,
    pub t_redundancy_enqueued: Option<u64>,
    pub t_redundancy_wire: Option<u64>,
    pub t_redundancy_arrival: Option<u64>,
}

impl BlockTiming {
    /// Redundancy lifecycle'ı kaydet: generic alanlar HER-arm; FecRepair'de AYRICA legacy `t_parity_*`.
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

/// Deterministik bottleneck egress-model (PIN-E physical-cost). Her paket (source/dup/parity/retransmit)
/// AYNI bottleneck-link'ten serialize olur → A1-duplicate **BEDAVA-LANE ALMAZ**. Kayıp (TraceChannel)
/// delivery-anında; egress-serialization kayıp-olsa-da tüketilir (paket link'e kondu). `arrival =
/// max(emit, link_free) + serialize + prop`; `link_free` ilerler (bottleneck-kuyruğu).
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

    /// Paketi bottleneck'e sun. `None` = dropped (channel); `Some(arrival_ns)` = teslim-zamanı.
    /// Egress-serialization HER paket için (dropped dahil) → redundancy egress-budget tüketir (PIN-E).
    pub fn offer(&mut self, id: TracePacketId, sealed_len: u64, emit_ns: u64) -> Option<u64> {
        // Egress link-serialization (kayıp-olsa-da; paket egress-slot'unu tüketti).
        let serialize_ns =
            sealed_len.saturating_mul(8).saturating_mul(1_000_000_000) / self.link_bps;
        let start = emit_ns.max(self.link_free_ns);
        self.link_free_ns = start.saturating_add(serialize_ns);
        self.serialized_bytes = self.serialized_bytes.saturating_add(sealed_len);
        // Delivery-loss (serialize-sonrası): dropped → teslim yok.
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
use crate::sender::{AckInfo, Event, OpaqueUnit, Sender, TransmitDecision, TransmitKind};
use std::collections::BTreeSet;
use std::time::Duration;

/// Ölçülen kol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// ARQ + 0 proaktif-redundancy.
    A0,
    /// ARQ + same-byte early source-duplication (r kopya).
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
}

fn tag_for(kind: TransmitKind, unit: u64) -> Option<(TracePacketId, bool)> {
    let (role, is_data) = match kind {
        TransmitKind::Data => (TransmissionRole::InitialSource, true),
        TransmitKind::Retransmit => (TransmissionRole::RetransmitSource, false),
        _ => return None, // Sender'ın-kendi-Duplicate(§11)/FecParity/CsProbe → A0/A1'de yok-say
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

/// Bir emisyon-listesini işle: semantic-tag → BottleneckChannel → delivery-schedule; A1'de dup-inject.
fn process_emissions(
    decisions: &[TransmitDecision],
    now: u64,
    r: u16,
    ch: &mut BottleneckChannel,
    res: &mut RunResult,
    deliveries: &mut Vec<(u64, u64, u64)>,
) {
    for d in decisions {
        let Some((id, is_data)) = tag_for(d.kind, d.unit_id) else {
            continue;
        };
        match d.kind {
            TransmitKind::Data => res.data_emitted += 1,
            TransmitKind::Retransmit => res.retransmits_emitted += 1,
            _ => {}
        }
        if let Some(arr) = ch.offer(id, d.size, now) {
            deliveries.push((arr, d.unit_id, d.seq));
        }
        // A1: bir Data emisyonundan HEMEN-SONRA r duplicate (PER_SOURCE_ADJACENT_EARLY).
        if is_data && r > 0 {
            for i in 0..r {
                let did = TracePacketId {
                    source_id: (0, d.unit_id as u16),
                    role: TransmissionRole::Duplicate,
                    attempt: i + 1,
                };
                res.duplicates_emitted += 1;
                res.cost.redundancy_packet_count += 1;
                res.cost.realized_sealed_wire_bytes += d.size;
                res.cost.redundancy_payload_bytes += d.size;
                if let Some(arr) = ch.offer(did, d.size, now) {
                    deliveries.push((arr, d.unit_id, d.seq));
                }
            }
        }
    }
}

/// KAPALI-DÖNGÜ: gerçek `Sender` (ARQ+ACK+pacing) × deterministic `BottleneckChannel` (semantic-trace).
/// A0 kayıp-source'u ARQ-retransmit ile, A1 duplicate ile kurtarır. Observational-ledger; production DEĞİL.
#[must_use]
pub fn run_arm(arm: Arm, cfg: &RunCfg) -> RunResult {
    let r = match arm {
        Arm::A1 { r } => r,
        Arm::A0 => 0,
    };
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
    let mut deliveries: Vec<(u64, u64, u64)> = Vec::new(); // (arrival, unit, seq)
    let mut acks: Vec<(u64, u64)> = Vec::new(); // (ack_arrival, seq)

    // t=0: tüm DATA'yı sun.
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
    process_emissions(&d0, 0, r, &mut ch, &mut res, &mut deliveries);

    let mut now = 0u64;
    while now < cfg.horizon_ns && delivered.len() < cfg.num_units {
        now += cfg.dt_ns;
        // (1) Forward-delivery due: unit teslim + reverse-ACK planla.
        let mut newly_acked: Vec<u64> = Vec::new();
        deliveries.retain(|&(arr, unit, seq)| {
            if arr <= now {
                if delivered.insert(unit) && delivered.len() == cfg.num_units {
                    res.completion_ns = Some(arr.max(now));
                }
                acks.push((arr + cfg.owd_ns, seq)); // reverse-path
                let _ = &mut newly_acked;
                false
            } else {
                true
            }
        });
        // (2) Reverse ACK due → Sender'a besle.
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
            process_emissions(&d, now, r, &mut ch, &mut res, &mut deliveries);
        }
        // (3) PTO tick → ARQ-retransmit (kayıp-source için).
        let d = s.on_event(Event::TimerFired(crate::sender::Timer::Pto), now);
        process_emissions(&d, now, r, &mut ch, &mut res, &mut deliveries);
    }
    res.delivered_units = delivered.len();
    res.channel_serialized_bytes = ch.serialized_bytes();
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_trace::TraceChannel;

    const MS: u64 = 1_000_000;

    fn set(items: &[(u32, u16)]) -> BTreeSet<(u32, u16)> {
        items.iter().copied().collect()
    }
    fn pid(role: TransmissionRole, block: u32, esi: u16, attempt: u16) -> TracePacketId {
        TracePacketId {
            source_id: (block, esi),
            role,
            attempt,
        }
    }

    /// Test-sink: alınan paketleri sayar (egress'e-gerçekten-ulaştı kanıtı; bedava-lane-yok).
    #[derive(Default)]
    struct CountingSink {
        packets: Vec<(TracePacketId, usize)>,
    }
    impl PacketSink for CountingSink {
        fn send(&mut self, id: TracePacketId, sealed: &[u8]) {
            self.packets.push((id, sealed.len()));
        }
    }

    /// Sender'ın geniş diag-state snapshot'ı (PIN-A: emisyon-öncesi/sonrası özdeş-olmalı).
    #[allow(clippy::type_complexity)]
    fn snapshot(
        s: &Sender,
    ) -> (
        crate::sender::SenderState, // recovery-mode/state
        (u64, u64),                 // BtlBw (diag_wbtlbw)
        (u64, u64),                 // rtprop/qd (diag_qd)
        (u64, u64),                 // rtt-guard (diag_rtt)
        (u64, u64, u64),            // loss-ledger (diag_ledger)
        u64,                        // packets-in-flight (diag_pif)
        Option<u64>,                // srtt (path_timing)
        Option<u64>,                // min_rtt (path_timing)
        u64,                        // queue_delay (path_timing)
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
    fn a1_early_duplicate_is_transport_state_neutral() {
        // Sender'ı non-trivial state'e sür (DATA + ACK → srtt/btlbw/loss oluşur).
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

        // A1 duplicate-emisyonu — Sender'a DOKUNMADAN (emitter &Sender-almaz).
        let mut emitter = A1EarlyEmitter::new(2);
        let mut sink = CountingSink::default();
        let sealed = [0xABu8; 108]; // örnek sealed-source
        emitter.emit_duplicates((0, 0), &sealed, 65, &mut sink);
        emitter.emit_duplicates((0, 1), &sealed, 65, &mut sink);

        // Feedback'ten ÖNCE: Sender-state DEĞİŞMEDİ (direct emission-side neutrality).
        let after = snapshot(&s);
        assert_eq!(
            before, after,
            "A1 duplicate-emisyonu Sender-state'ini doğrudan değiştirdi (PIN-A ihlali)"
        );
        // Duplicate'ler gerçekten egress'e ulaştı (bedava-lane-yok): 2 source × r=2 = 4.
        assert_eq!(sink.packets.len(), 4);
    }

    /// PIN-E: her duplicate egress-path'i tüketir + exact sealed-wire accounting (configured_r-değil realized).
    #[test]
    fn a1_early_duplicate_consumes_wire_budget() {
        let mut emitter = A1EarlyEmitter::new(2);
        let mut sink = CountingSink::default();
        let sealed = [0u8; 108]; // sealed-source 108B (payload 65B + header/framing)
        emitter.emit_duplicates((0, 3), &sealed, 65, &mut sink);

        let cost = emitter.cost();
        assert_eq!(cost.configured_r, 2);
        assert_eq!(cost.redundancy_packet_count, 2, "r=2 → 2 kopya");
        // REALIZED sealed-wire = 2 × 108 = 216 (header/framing DAHİL; C-P4 fairness ekseni).
        assert_eq!(cost.realized_sealed_wire_bytes, 216);
        assert_eq!(cost.redundancy_payload_bytes, 130); // 2 × 65 (payload)
        assert_eq!(cost.redundancy_framing_bytes, 86); // 2 × (108−65) framing/header
        assert_eq!(
            cost.redundancy_payload_bytes + cost.redundancy_framing_bytes,
            cost.realized_sealed_wire_bytes
        );
        // Egress-path'i gerçekten tüketti (sink 2 paket + 2×108 byte aldı; bedava-lane-yok).
        assert_eq!(sink.packets.len(), 2);
        assert_eq!(sink.packets.iter().map(|(_, n)| *n).sum::<usize>(), 216);
        // configured_r ≠ realized-sealed-wire: r=2 duplicate(216B) FEC-r=2-parity'den farklı sealed-byte
        // olabilir (header/framing) → C-P4 realized-wire üstünden eşlenir, configured_r üstünden DEĞİL.
    }

    /// D-INDEPENDENT: emitter hangi-source'u kopyalayacağını trace-drop-set'ten bağımsız bilir (yapısal:
    /// D-alanı yok). Aynı emitter herhangi-D altında aynı-kopyaları üretir.
    #[test]
    fn a1_duplicate_target_selection_is_deficit_independent() {
        // Predeclared: source {0,1,2} kopyalanır — D ne-olursa-olsun.
        let targets = [(0u32, 0u16), (0, 1), (0, 2)];
        let mut emitter = A1EarlyEmitter::new(1);
        let mut sink = CountingSink::default();
        let sealed = [0u8; 100];
        for &t in &targets {
            emitter.emit_duplicates(t, &sealed, 60, &mut sink);
        }
        // 3 target × r=1 = 3 duplicate; hangi-source-düşecek bilgisi KULLANILMADI (emitter'da D yok).
        assert_eq!(emitter.cost().redundancy_packet_count, 3);
        let dup_ids: Vec<(u32, u16)> = sink.packets.iter().map(|(id, _)| id.source_id).collect();
        assert_eq!(dup_ids, vec![(0, 0), (0, 1), (0, 2)]);
    }

    /// A1 emission-order PER_SOURCE_ADJACENT_EARLY: deterministik + kopyalar source'un HEMEN-ardından.
    #[test]
    fn a1_emission_order_per_source_adjacent_early_deterministic() {
        use TransmissionRole::*;
        let sources = [(0u32, 0u16), (0, 1)];
        let o1 = a1_emission_order(
            &sources,
            2,
            RedundancyEmissionPolicy::PerSourceAdjacentEarly,
        );
        let o2 = a1_emission_order(
            &sources,
            2,
            RedundancyEmissionPolicy::PerSourceAdjacentEarly,
        );
        assert_eq!(
            o1, o2,
            "emisyon-sırası deterministik değil (scheduling-jitter)"
        );
        // Beklenen: S0, D0#1, D0#2, S1, D1#1, D1#2.
        assert_eq!(o1.len(), 6);
        assert_eq!(
            (o1[0].role, o1[0].source_id, o1[0].attempt),
            (InitialSource, (0, 0), 0)
        );
        assert_eq!(
            (o1[1].role, o1[1].source_id, o1[1].attempt),
            (Duplicate, (0, 0), 1)
        );
        assert_eq!(
            (o1[2].role, o1[2].source_id, o1[2].attempt),
            (Duplicate, (0, 0), 2)
        );
        assert_eq!(
            (o1[3].role, o1[3].source_id, o1[3].attempt),
            (InitialSource, (0, 1), 0)
        );
    }

    /// BOTTLENECK: source+dup'lar AYNI link'ten SIRALI serialize (dup BEDAVA-LANE ALMAZ; PIN-E physical).
    #[test]
    fn bottleneck_serializes_all_classes_no_free_lane() {
        use TransmissionRole::*;
        // loss=0 → hepsi geçer; serialize-sırası ölçülür. 1 Mbps, 10ms prop.
        let ch = TraceChannel::new(TraceSpec::NaturalIid {
            loss_permille: 0,
            seed: 0,
        });
        let mut bn = BottleneckChannel::new(ch, 1_000_000, 10_000_000);
        let a_s = bn.offer(pid(InitialSource, 0, 0, 0), 100, 0).unwrap();
        let a_d1 = bn.offer(pid(Duplicate, 0, 0, 1), 100, 0).unwrap();
        let a_d2 = bn.offer(pid(Duplicate, 0, 0, 2), 100, 0).unwrap();
        // 100B @1Mbps = 800us serialize; dup'lar source-sonrası SIRALI (bedava-lane-yok).
        assert!(
            a_d1 > a_s && a_d2 > a_d1,
            "dup bedava-lane aldı (serialize atlandı)"
        );
        assert_eq!(a_d1 - a_s, 800_000);
        assert_eq!(a_d2 - a_d1, 800_000);
        assert_eq!(
            bn.serialized_bytes(),
            300,
            "3×100B egress-budget tüketilmeli"
        );
    }

    /// BOTTLENECK: dropped-paket de egress-serialization tüketir (paket link'e kondu; PIN-E).
    #[test]
    fn bottleneck_drops_still_consume_egress_budget() {
        use TransmissionRole::*;
        let ch = TraceChannel::new(TraceSpec::ExactDeficit {
            drop_initial: set(&[(0, 2)]),
            redundancy: RedundancyPolicy::PassAll,
        });
        let mut bn = BottleneckChannel::new(ch, 1_000_000, 0);
        assert!(
            bn.offer(pid(InitialSource, 0, 2, 0), 100, 0).is_none(),
            "initial-source düşmeli"
        );
        assert_eq!(
            bn.serialized_bytes(),
            100,
            "dropped-paket de egress-budget tüketmeli (PIN-E)"
        );
    }

    /// TIMING-LEDGER mapping: generic HER-arm; legacy t_parity_* yalnız FecRepair.
    #[test]
    fn block_timing_generic_and_legacy_mapping() {
        let mut a1 = BlockTiming::default();
        a1.record_redundancy(RedundancyKind::Duplicate, 10, 20, 30, Some(40));
        assert_eq!(a1.t_redundancy_wire, Some(30));
        assert_eq!(a1.t_redundancy_arrival, Some(40));
        assert_eq!(
            a1.t_parity_wire, None,
            "A1(Duplicate) legacy-parity-alanı DOLMAMALI"
        );

        let mut f0 = BlockTiming::default();
        f0.record_redundancy(RedundancyKind::FecRepair, 10, 20, 30, Some(40));
        assert_eq!(f0.t_redundancy_wire, Some(30));
        assert_eq!(
            f0.t_parity_wire,
            Some(30),
            "F*(FecRepair) legacy-parity + generic AYNI"
        );
    }

    fn cfg(drop: &[u64]) -> RunCfg {
        RunCfg {
            num_units: 8,
            unit_size: 300,
            link_bps: 1_000_000,
            owd_ns: 20 * MS, // 20ms one-way → 40ms RTT
            horizon_ns: 30_000 * MS,
            dt_ns: MS,
            drop_initial_units: drop.iter().copied().collect(),
        }
    }

    /// CLOSED-LOOP SMOKE (loopback): kayıp-yok → tüm-unit teslim, retransmit-YOK.
    #[test]
    fn run_arm_loopback_completes_no_retransmit() {
        let r = run_arm(Arm::A0, &cfg(&[]));
        assert_eq!(r.delivered_units, 8, "loopback tüm-unit teslim etmeli");
        assert!(r.completion_ns.is_some(), "tamamlanmalı");
        assert_eq!(r.retransmits_emitted, 0, "kayıp-yok → retransmit-yok");
        assert_eq!(r.data_emitted, 8);
    }

    /// FAITHFULNESS: A0 kayıp-source'u ancak ARQ-RETRANSMIT ile kurtarır (retransmit > 0).
    #[test]
    fn run_arm_a0_deficit_recovers_via_retransmit() {
        let r = run_arm(Arm::A0, &cfg(&[2, 5]));
        assert_eq!(r.delivered_units, 8, "A0 nihayetinde tüm-unit'i kurtarmalı");
        assert!(
            r.retransmits_emitted > 0,
            "A0 kayıp-source'u retransmit-ETMEDEN kurtardı (harness UNFAITHFUL)"
        );
    }

    /// FAITHFULNESS: A1 kayıp-source'u DUPLICATE ile kurtarır (retransmit gerekmez) VE A0'dan ERKEN biter.
    #[test]
    fn run_arm_a1_deficit_recovers_via_duplicate_earlier_than_a0() {
        let a0 = run_arm(Arm::A0, &cfg(&[2, 5]));
        let a1 = run_arm(Arm::A1 { r: 1 }, &cfg(&[2, 5]));
        assert_eq!(a1.delivered_units, 8, "A1 tüm-unit teslim");
        assert!(a1.duplicates_emitted > 0, "A1 duplicate üretmeli");
        assert_eq!(
            a1.retransmits_emitted, 0,
            "A1 duplicate-kurtardı → retransmit-GEREKMEZ"
        );
        assert!(
            a1.completion_ns.unwrap() < a0.completion_ns.unwrap(),
            "A1({:?}) A0({:?})'dan erken bitmedi (mekanizma-kanıtı yok)",
            a1.completion_ns,
            a0.completion_ns
        );
        assert!(a1.channel_serialized_bytes > a0.channel_serialized_bytes);
    }
}
