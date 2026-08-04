//! DR-FEC-FS-1 FS-HARNESS-A — TRACE-DRIVEN PAIRED REPLAY CHANNEL (Abkarada v5 pin-2 + attempt-aware düzeltme).
//!
//! **Frozen pairing semantics.** Wire-ordinal drop replay YASAK: kollar (A0/A1-EARLY/F0/…) farklı sayıda
//! wire-packet emit eder → "drop packet #N" kollar-arası AYNI channel-realization'ı YAŞATMAZ.
//!
//! **EXACT-DEFICIT = INITIAL-SOURCE-TRANSMISSION ERASURE (source-identity erasure DEĞİL).** Kritik
//! düzeltme (Abkarada): deficit-set `D`, bir source-id'nin BÜTÜN transmission-attempt'lerini öldürmez;
//! yalnız **INITIAL source-transmission'ı** (role=InitialSource, attempt=0) siler. Sonraki attempt'ler
//! — **ARQ retransmit** (A0'ın recovery-kanalı), **A1 duplicate**, **FEC parity** — BAĞIMSIZ scripted
//! kanal-kararı alır (default PassAll → hepsi admissible). Böylece:
//!   - A0 kayıp-source'u retransmit ile kurtarabilir,
//!   - A1 duplicate ile kurtarabilir,
//!   - F0 parity ile kurtarabilir,
//! HEPSİ AYNI initial-deficit'te → gerçek coding-vs-duplication karşılaştırması, oracle-leakage-YOK,
//! future-loss-knowledge-YOK. (Aksi: source-identity-erasure A0-retransmit'i + A1-dup'ı da öldürür →
//! FEC/redundancy lehine yapısal-bias — C-P1/C-P4'ü geçersiz kılardı.)
//!
//! **Trace-key = (semantic_source_id, transmission_role, attempt_index).** Determinism-pin: aynı trace ×
//! N replay → identical decision-log hash (RNG-YOK; saf-fonksiyon).

use std::collections::BTreeSet;

/// Bir wire-transmission'ın rolü (initial-source vs sonraki-attempt'ler ayrımı = deficit-tanımının çekirdeği).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransmissionRole {
    /// İLK source-transmission (attempt=0) — deficit-set `D` YALNIZ bunu siler.
    InitialSource,
    /// ARQ retransmit (attempt≥1) — A0'ın recovery-kanalı; bağımsız-admissible.
    RetransmitSource,
    /// A1-EARLY proaktif source-kopyası; bağımsız-admissible (D'den bağımsız seçilir).
    Duplicate,
    /// FEC parity/repair; bağımsız-admissible.
    Repair,
    /// Kontrol (ACK/CS/…): kanal-loss'undan muaf (reverse-trace ayrı).
    Control,
}

/// Kanal-kararı için paketin SEMANTİK kimliği (wire-ordinal DEĞİL): source-id + role + attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TracePacketId {
    /// Semantik source-birimi (block_id, esi). Repair için parity'nin (block,esi)'si; Control'de yok-sayılır.
    pub source_id: (u32, u16),
    pub role: TransmissionRole,
    /// 0 = initial; ≥1 = sonraki-attempt (retransmit/duplicate-index).
    pub attempt: u16,
}

impl TracePacketId {
    #[must_use]
    pub fn initial_source(block_id: u32, esi: u16) -> Self {
        Self {
            source_id: (block_id, esi),
            role: TransmissionRole::InitialSource,
            attempt: 0,
        }
    }
}

/// Redundancy-attempt'lerin (retransmit/duplicate/parity) BAĞIMSIZ kanal-politikası (D'den bağımsız).
#[derive(Debug, Clone)]
pub enum RedundancyPolicy {
    /// Confirmatory-default: redundancy her-zaman admissible (yalnız initial-source-deficit ölçülür).
    PassAll,
    /// Bağımsız-scripted deterministic loss (redundancy'ye kendi kanalı; D'den bağımsız).
    Iid { loss_permille: u32, seed: u64 },
}

/// Dondurulmuş trace-spesifikasyonu.
#[derive(Debug, Clone)]
pub enum TraceSpec {
    /// C-P1/C-P2: yalnız INITIAL-source-transmission'ı (role=InitialSource) düşür; redundancy bağımsız.
    ExactDeficit {
        drop_initial: BTreeSet<(u32, u16)>,
        redundancy: RedundancyPolicy,
    },
    /// Doğal IID: per-(id,role,attempt) deterministic hash-keyed drop (arm-independent; her-attempt bağımsız).
    NaturalIid { loss_permille: u32, seed: u64 },
    /// Doğal burst: arm-bağımsız time-domain drop-aralıkları [lo,hi) ns.
    NaturalBurst { drop_intervals_ns: Vec<(u64, u64)> },
}

/// **B1 — REVERSE-PATH (ACK/control yönü) deterministik impairment ŞEMASI.** GROUND-TRUTH: reverse-blackout
/// EKSİK-ACK'ten ÇIKARILMAZ — şemanın kendisi gerçektir (Abkarada B1: "impairment schedule is the ground
/// truth; do not infer reverse blackout from missing ACKs"). Üç-boyut: reverse-loss (ordinal-keyed
/// deterministic iid) · reverse-blackout (time-interval [lo,hi) ns) · recovery (aralık-DIŞI → PASS, otomatik).
/// Arm-BAĞIMSIZ ŞEMA (spec kollar-arası özdeş; realizasyon kola göre değişebilir — C-P3 pairing B3).
#[derive(Debug, Clone, Default)]
pub enum ReverseSpec {
    /// Reverse-path temiz — ACK/control her-zaman geçer (Harness-A davranışı; forward-only).
    #[default]
    Clear,
    /// Zamanlanmış reverse-impairment (blackout + opsiyonel reverse-loss).
    Scheduled {
        /// Reverse-blackout aralıkları [lo,hi) ns; içindeki ACK DÜŞER, DIŞINDA geçer (recovery-after-blackout).
        blackout_intervals_ns: Vec<(u64, u64)>,
        /// Reverse-loss (ordinal-keyed deterministic iid; 0 = reverse-loss yok, yalnız-blackout).
        loss_permille: u32,
        seed: u64,
    },
}

/// FNV-1a 64-bit (deterministik; RNG-YOK — golden/replay-güvenli).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// **B3 pairing identity** — CONFIGURED forward-schedule kimliği (realizasyon DEĞİL). Arm-BAĞIMSIZ:
/// spec kollar-arası özdeşse hash özdeş (aynı-PRNG-seed TEK-BAŞINA yetmez; şema-kimliği ŞART).
fn hash_forward_spec(spec: &TraceSpec) -> u64 {
    let mut b: Vec<u8> = Vec::new();
    match spec {
        TraceSpec::ExactDeficit {
            drop_initial,
            redundancy,
        } => {
            b.push(1);
            for (blk, esi) in drop_initial {
                b.extend_from_slice(&blk.to_le_bytes());
                b.extend_from_slice(&esi.to_le_bytes());
            }
            match redundancy {
                RedundancyPolicy::PassAll => b.push(10),
                RedundancyPolicy::Iid {
                    loss_permille,
                    seed,
                } => {
                    b.push(11);
                    b.extend_from_slice(&loss_permille.to_le_bytes());
                    b.extend_from_slice(&seed.to_le_bytes());
                }
            }
        }
        TraceSpec::NaturalIid {
            loss_permille,
            seed,
        } => {
            b.push(2);
            b.extend_from_slice(&loss_permille.to_le_bytes());
            b.extend_from_slice(&seed.to_le_bytes());
        }
        TraceSpec::NaturalBurst { drop_intervals_ns } => {
            b.push(3);
            for (lo, hi) in drop_intervals_ns {
                b.extend_from_slice(&lo.to_le_bytes());
                b.extend_from_slice(&hi.to_le_bytes());
            }
        }
    }
    fnv1a(&b)
}

/// **B3 pairing identity** — CONFIGURED reverse-schedule kimliği (forward'dan AYRI-görünür).
fn hash_reverse_spec(spec: &ReverseSpec) -> u64 {
    let mut b: Vec<u8> = Vec::new();
    match spec {
        ReverseSpec::Clear => b.push(0),
        ReverseSpec::Scheduled {
            blackout_intervals_ns,
            loss_permille,
            seed,
        } => {
            b.push(1);
            for (lo, hi) in blackout_intervals_ns {
                b.extend_from_slice(&lo.to_le_bytes());
                b.extend_from_slice(&hi.to_le_bytes());
            }
            b.extend_from_slice(&loss_permille.to_le_bytes());
            b.extend_from_slice(&seed.to_le_bytes());
        }
    }
    fnv1a(&b)
}

fn role_disc(r: TransmissionRole) -> u8 {
    match r {
        TransmissionRole::InitialSource => 1,
        TransmissionRole::RetransmitSource => 2,
        TransmissionRole::Duplicate => 3,
        TransmissionRole::Repair => 4,
        TransmissionRole::Control => 5,
    }
}

/// (source_id, role, attempt, seed) → deterministic key (her-attempt ayrık).
fn keyed(id: TracePacketId, seed: u64) -> [u8; 17] {
    let mut k = [0u8; 17];
    k[0] = role_disc(id.role);
    k[1..5].copy_from_slice(&id.source_id.0.to_le_bytes());
    k[5..7].copy_from_slice(&id.source_id.1.to_le_bytes());
    k[7..9].copy_from_slice(&id.attempt.to_le_bytes());
    k[9..17].copy_from_slice(&seed.to_le_bytes());
    k
}

/// **OPT-X2 observation-only instrumentation.** Terminal per-event forward decision record (append-only; lossless).
/// Captured ADDITIVELY at the existing decision point — reconstructs the exact `rec` fed to `decision_log_hash`,
/// so `recomputed_decision_log_hash()` MUST equal `decision_log_hash()` (consistency pin). Does NOT alter any
/// existing computation, RNG, counter, or the returned drop decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionEvent {
    pub role: TransmissionRole,
    pub block_id: u32,
    pub esi: u16,
    pub attempt: u16,
    pub emit_time_ns: u64,
    /// true = DROP, false = PASS (exact `drop` returned by `decide`).
    pub dropped: bool,
}

/// Deterministik paired-replay kanalı. `decide()` saf-fonksiyon; karar-log determinism-pin için tutulur.
/// **B1: forward + reverse impairment AYRI-görünür** (ayrı realizasyon-hash + ayrı counts + ayrı şema-kimliği).
pub struct TraceChannel {
    spec: TraceSpec,
    reverse: ReverseSpec,
    decision_log_hash: u64,
    decided: u64,
    dropped: u64,
    reverse_decision_log_hash: u64,
    reverse_decided: u64,
    reverse_dropped: u64,
    /// OPT-X2 additive observation: append-only forward decision events (lossless per-event trace).
    forward_decisions: Vec<DecisionEvent>,
}

impl TraceChannel {
    #[must_use]
    pub fn new(spec: TraceSpec) -> Self {
        Self::with_reverse(spec, ReverseSpec::Clear)
    }

    /// B1: forward + reverse şemayı birlikte kur (C-P3 paired reverse-blackout için).
    #[must_use]
    pub fn with_reverse(spec: TraceSpec, reverse: ReverseSpec) -> Self {
        Self {
            spec,
            reverse,
            decision_log_hash: 0xcbf2_9ce4_8422_2325,
            decided: 0,
            dropped: 0,
            reverse_decision_log_hash: 0xcbf2_9ce4_8422_2325,
            reverse_decided: 0,
            reverse_dropped: 0,
            forward_decisions: Vec::new(),
        }
    }

    /// `true` = DROP, `false` = PASS. Saf: aynı (spec,id,time) → aynı karar (arm-invariant/replay-det.).
    pub fn decide(&mut self, id: TracePacketId, emit_time_ns: u64) -> bool {
        let drop = match &self.spec {
            TraceSpec::ExactDeficit {
                drop_initial,
                redundancy,
            } => match id.role {
                // Deficit YALNIZ initial source-transmission'ı siler.
                TransmissionRole::InitialSource => drop_initial.contains(&id.source_id),
                TransmissionRole::Control => false,
                // Retransmit / Duplicate / Repair: BAĞIMSIZ scripted (D'den bağımsız).
                _ => match redundancy {
                    RedundancyPolicy::PassAll => false,
                    RedundancyPolicy::Iid {
                        loss_permille,
                        seed,
                    } => (fnv1a(&keyed(id, *seed)) % 1000) < u64::from(*loss_permille),
                },
            },
            TraceSpec::NaturalIid {
                loss_permille,
                seed,
            } => match id.role {
                TransmissionRole::Control => false,
                _ => (fnv1a(&keyed(id, *seed)) % 1000) < u64::from(*loss_permille),
            },
            TraceSpec::NaturalBurst { drop_intervals_ns } => match id.role {
                TransmissionRole::Control => false,
                _ => drop_intervals_ns
                    .iter()
                    .any(|&(lo, hi)| emit_time_ns >= lo && emit_time_ns < hi),
            },
        };
        let mut rec = keyed(id, 0).to_vec();
        rec.extend_from_slice(&emit_time_ns.to_le_bytes());
        rec.push(u8::from(drop));
        self.decision_log_hash ^= fnv1a(&rec);
        self.decision_log_hash = self.decision_log_hash.wrapping_mul(0x0000_0100_0000_01b3);
        self.decided += 1;
        if drop {
            self.dropped += 1;
        }
        // OPT-X2 additive observation: record the (already-computed) decision. No effect on hash/counters/drop.
        self.forward_decisions.push(DecisionEvent {
            role: id.role,
            block_id: id.source_id.0,
            esi: id.source_id.1,
            attempt: id.attempt,
            emit_time_ns,
            dropped: drop,
        });
        drop
    }

    /// Determinism-pin: karar-dizisinin hash'i (aynı trace × N replay → identical).
    #[must_use]
    pub fn decision_log_hash(&self) -> u64 {
        self.decision_log_hash
    }

    /// OPT-X2: append-only forward decision events (lossless per-event trace; observation-only).
    #[must_use]
    pub fn forward_decisions(&self) -> &[DecisionEvent] {
        &self.forward_decisions
    }

    /// OPT-X2 CONSISTENCY PIN: recompute the rolling forward decision-log hash from the captured event vector.
    /// MUST equal `decision_log_hash()` — proves the additive capture faithfully mirrors the (untouched) rolling hash.
    #[must_use]
    pub fn recomputed_decision_log_hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for ev in &self.forward_decisions {
            let id = TracePacketId {
                source_id: (ev.block_id, ev.esi),
                role: ev.role,
                attempt: ev.attempt,
            };
            let mut rec = keyed(id, 0).to_vec();
            rec.extend_from_slice(&ev.emit_time_ns.to_le_bytes());
            rec.push(u8::from(ev.dropped));
            h ^= fnv1a(&rec);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// (karar-sayısı, düşürülen-sayısı) — realized-loss faithfulness raporu için.
    #[must_use]
    pub fn counts(&self) -> (u64, u64) {
        (self.decided, self.dropped)
    }

    /// **B1 — REVERSE karar:** `true` = ACK/control DÜŞER (Sender feedback-almaz → ARQ-clock durur; C-P3).
    /// GROUND-TRUTH şema (eksik-ACK'ten ÇIKARIM DEĞİL). `ack_ordinal` = monoton ACK-sayacı (reverse-loss
    /// iid-anahtarı). Forward'dan TAMAMEN AYRI hash/counts (ayrı-görünürlük). Saf: aynı (rev-spec,time,ordinal)
    /// → aynı karar.
    pub fn decide_reverse(&mut self, emit_time_ns: u64, ack_ordinal: u64) -> bool {
        let drop = match &self.reverse {
            ReverseSpec::Clear => false,
            ReverseSpec::Scheduled {
                blackout_intervals_ns,
                loss_permille,
                seed,
            } => {
                let blackout = blackout_intervals_ns
                    .iter()
                    .any(|&(lo, hi)| emit_time_ns >= lo && emit_time_ns < hi);
                let loss = if *loss_permille == 0 {
                    false
                } else {
                    let mut k = ack_ordinal.to_le_bytes().to_vec();
                    k.extend_from_slice(&seed.to_le_bytes());
                    (fnv1a(&k) % 1000) < u64::from(*loss_permille)
                };
                blackout || loss
            }
        };
        let mut rec = emit_time_ns.to_le_bytes().to_vec();
        rec.extend_from_slice(&ack_ordinal.to_le_bytes());
        rec.push(u8::from(drop));
        self.reverse_decision_log_hash ^= fnv1a(&rec);
        self.reverse_decision_log_hash = self
            .reverse_decision_log_hash
            .wrapping_mul(0x0000_0100_0000_01b3);
        self.reverse_decided += 1;
        if drop {
            self.reverse_dropped += 1;
        }
        drop
    }

    /// B1: reverse realizasyon-hash (forward'dan AYRI). Aynı reverse-trace × N → identical.
    #[must_use]
    pub fn reverse_decision_log_hash(&self) -> u64 {
        self.reverse_decision_log_hash
    }

    /// B1: (reverse-karar-sayısı, reverse-düşürülen).
    #[must_use]
    pub fn reverse_counts(&self) -> (u64, u64) {
        (self.reverse_decided, self.reverse_dropped)
    }

    /// **B3 pairing:** CONFIGURED forward-şema kimliği (realizasyon DEĞİL; arm-BAĞIMSIZ). Kollar-arası
    /// özdeş-spec → özdeş-id (aynı-seed TEK-BAŞINA yetmez — şema-kimliği ŞART).
    #[must_use]
    pub fn forward_schedule_id(&self) -> u64 {
        hash_forward_spec(&self.spec)
    }

    /// **B3 pairing:** CONFIGURED reverse-şema kimliği (forward'dan AYRI-görünür).
    #[must_use]
    pub fn reverse_schedule_id(&self) -> u64 {
        hash_reverse_spec(&self.reverse)
    }

    /// **B3 pairing:** forward+reverse birleşik CONFIGURED-şema kimliği (channel-intervention identity;
    /// kollar-arası ÖZDEŞ olmalı — "identical configured impairment schedule across arms").
    #[must_use]
    pub fn configured_schedule_hash(&self) -> u64 {
        let mut b = self.forward_schedule_id().to_le_bytes().to_vec();
        b.extend_from_slice(&self.reverse_schedule_id().to_le_bytes());
        fnv1a(&b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[(u32, u16)]) -> BTreeSet<(u32, u16)> {
        items.iter().copied().collect()
    }
    fn exact(d: &[(u32, u16)]) -> TraceSpec {
        TraceSpec::ExactDeficit {
            drop_initial: set(d),
            redundancy: RedundancyPolicy::PassAll,
        }
    }
    fn pid(role: TransmissionRole, block: u32, esi: u16, attempt: u16) -> TracePacketId {
        TracePacketId {
            source_id: (block, esi),
            role,
            attempt,
        }
    }

    /// EXACT-DEFICIT yalnız INITIAL-source siler; retransmit/duplicate/parity BAĞIMSIZ-admissible.
    #[test]
    fn exact_deficit_erases_only_initial_source_transmission() {
        use TransmissionRole::*;
        let mut ch = TraceChannel::new(exact(&[(0, 2), (0, 5)]));
        // initial source 2,5 → DROP; 3 → PASS.
        assert!(ch.decide(pid(InitialSource, 0, 2, 0), 0));
        assert!(ch.decide(pid(InitialSource, 0, 5, 0), 0));
        assert!(!ch.decide(pid(InitialSource, 0, 3, 0), 0));
        // ANTI-BIAS: aynı kayıp-source'un recovery-attempt'leri PASS (hepsi kurtarabilir).
        assert!(
            !ch.decide(pid(RetransmitSource, 0, 2, 1), 0),
            "A0 ARQ-retransmit öldürüldü (bias)"
        );
        assert!(
            !ch.decide(pid(Duplicate, 0, 2, 1), 0),
            "A1 duplicate öldürüldü (bias)"
        );
        assert!(
            !ch.decide(pid(Repair, 0, 0, 0), 0),
            "F0 parity öldürüldü (bias)"
        );
        assert!(!ch.decide(pid(Control, 0, 0, 0), 0));
    }

    /// D (initial-source-deficit) A0/A1/F0/F1/F3/F4 kollarında ÖZDEŞ; recovery-attempt'ler hepsinde geçer.
    #[test]
    fn initial_source_deficit_is_arm_invariant_and_recovery_fair() {
        use TransmissionRole::*;
        let d = &[(0u32, 2u16), (0, 5)];
        // Her kolun paket-akışı: initial 0..7 + kola-özel recovery-attempt'ler.
        let a0: Vec<TracePacketId> = (0..8)
            .map(|e| pid(InitialSource, 0, e, 0))
            .chain([
                pid(RetransmitSource, 0, 2, 1),
                pid(RetransmitSource, 0, 5, 1),
            ])
            .collect();
        let a1: Vec<TracePacketId> = (0..8)
            .flat_map(|e| [pid(InitialSource, 0, e, 0), pid(Duplicate, 0, e, 1)])
            .collect();
        let f0: Vec<TracePacketId> = (0..8)
            .map(|e| pid(InitialSource, 0, e, 0))
            .chain((0..2).map(|e| pid(Repair, 0, e, 0)))
            .collect();
        for stream in [&a0, &a1, &f0] {
            let mut ch = TraceChannel::new(exact(d));
            let mut dropped_initial: BTreeSet<(u32, u16)> = BTreeSet::new();
            let mut recovery_dropped = 0;
            for &id in stream {
                let drop = ch.decide(id, 0);
                if drop {
                    match id.role {
                        InitialSource => {
                            dropped_initial.insert(id.source_id);
                        }
                        _ => recovery_dropped += 1,
                    }
                }
            }
            assert_eq!(dropped_initial, set(d), "initial-deficit kol-bağımlı");
            assert_eq!(
                recovery_dropped, 0,
                "recovery-attempt öldürüldü (fairness ihlali)"
            );
        }
    }

    /// Redundancy-policy BAĞIMSIZ scriptable (D'den bağımsız kendi kanalı).
    #[test]
    fn redundancy_policy_independently_scriptable() {
        use TransmissionRole::*;
        let spec = TraceSpec::ExactDeficit {
            drop_initial: set(&[(0, 2)]),
            redundancy: RedundancyPolicy::Iid {
                loss_permille: 1000,
                seed: 1,
            }, // %100 redundancy-loss (bağımsız)
        };
        let mut ch = TraceChannel::new(spec);
        assert!(ch.decide(pid(InitialSource, 0, 2, 0), 0)); // initial-deficit
        assert!(!ch.decide(pid(InitialSource, 0, 3, 0), 0)); // D-dışı initial PASS
        assert!(
            ch.decide(pid(Duplicate, 0, 2, 1), 0),
            "redundancy-policy uygulanmadı"
        );
        assert!(
            ch.decide(pid(Repair, 0, 0, 0), 0),
            "redundancy-policy parity'ye uygulanmadı"
        );
    }

    /// NATURAL-IID: deterministic + arm-independent + her-attempt bağımsız (initial vs retransmit ayrı karar-uzayı).
    #[test]
    fn natural_iid_deterministic_arm_independent_attempt_aware() {
        use TransmissionRole::*;
        let spec = TraceSpec::NaturalIid {
            loss_permille: 300,
            seed: 7,
        };
        let mut a = TraceChannel::new(spec.clone());
        let mut b = TraceChannel::new(spec);
        for e in 0..64u16 {
            let id = pid(InitialSource, 1, e, 0);
            assert_eq!(a.decide(id, 0), b.decide(id, 0), "esi {e}: arm-bağımlı");
        }
        let (n, dd) = a.counts();
        assert!(dd > 0 && dd < n, "IID dejenere (d={dd}/n={n})");
    }

    /// NATURAL-BURST: time-domain; identity/count'tan bağımsız.
    #[test]
    fn natural_burst_time_domain() {
        use TransmissionRole::*;
        let mut ch = TraceChannel::new(TraceSpec::NaturalBurst {
            drop_intervals_ns: vec![(1_000, 2_000)],
        });
        assert!(ch.decide(pid(InitialSource, 0, 0, 0), 1_500));
        assert!(!ch.decide(pid(InitialSource, 0, 1, 0), 2_500));
        assert!(ch.decide(pid(Repair, 0, 0, 0), 1_200));
        assert!(!ch.decide(pid(Control, 0, 0, 0), 1_500));
    }

    /// B1 — REVERSE-LOSS ŞEMA determinizmi: aynı reverse-loss trace × 3 → identical reverse-hash + kayıp>0.
    #[test]
    fn reverse_loss_schedule_replays_deterministically() {
        let rev = ReverseSpec::Scheduled {
            blackout_intervals_ns: vec![],
            loss_permille: 300,
            seed: 9,
        };
        let run = || {
            let mut ch = TraceChannel::with_reverse(exact(&[]), rev.clone());
            for ord in 0..200u64 {
                let _ = ch.decide_reverse(ord * 1000, ord);
            }
            (ch.reverse_decision_log_hash(), ch.reverse_counts())
        };
        let (h1, c1) = run();
        let (h2, _) = run();
        let (h3, _) = run();
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);
        assert!(
            c1.1 > 0 && c1.1 < c1.0,
            "reverse-loss dejenere (d={}/n={})",
            c1.1,
            c1.0
        );
    }

    /// B1 — REVERSE-BLACKOUT ŞEMA: [lo,hi) içindeki ACK düşer, DIŞINDA geçer (recovery-after); replay-det.
    #[test]
    fn reverse_blackout_schedule_replays_deterministically() {
        let rev = ReverseSpec::Scheduled {
            blackout_intervals_ns: vec![(100_000, 300_000)],
            loss_permille: 0,
            seed: 0,
        };
        let run = || {
            let mut ch = TraceChannel::with_reverse(exact(&[]), rev.clone());
            let mut inside = 0u64;
            let mut outside_passed = 0u64;
            for t in [50_000u64, 150_000, 250_000, 350_000] {
                let dropped = ch.decide_reverse(t, 0);
                if (100_000..300_000).contains(&t) {
                    assert!(dropped, "blackout-içi ACK geçti (t={t})");
                    inside += 1;
                } else {
                    assert!(
                        !dropped,
                        "blackout-DIŞI ACK düştü (recovery-after ihlali; t={t})"
                    );
                    outside_passed += 1;
                }
            }
            assert_eq!((inside, outside_passed), (2, 2));
            ch.reverse_decision_log_hash()
        };
        let (h1, h2, h3) = (run(), run(), run());
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);
    }

    /// B1 — FORWARD/REVERSE AYRI-GÖRÜNÜR: forward realizasyon reverse-kararlardan BAĞIMSIZ; şema-kimlikleri
    /// stabil + forward≠reverse; configured_schedule_hash stabil (B3 pairing tabanı).
    #[test]
    fn forward_and_reverse_trace_hashes_are_stable() {
        use TransmissionRole::*;
        let rev = ReverseSpec::Scheduled {
            blackout_intervals_ns: vec![(0, 1_000)],
            loss_permille: 0,
            seed: 0,
        };
        // İki kanal: biri reverse-karar-alır, biri ALMAZ → forward-hash AYNI (ayrı-görünürlük).
        let mut a = TraceChannel::with_reverse(exact(&[(0, 2)]), rev.clone());
        let mut b = TraceChannel::with_reverse(exact(&[(0, 2)]), rev.clone());
        for e in 0..8u16 {
            a.decide(pid(InitialSource, 0, e, 0), 0);
            b.decide(pid(InitialSource, 0, e, 0), 0);
        }
        // Yalnız a reverse-karar-alır.
        a.decide_reverse(500, 0);
        a.decide_reverse(1500, 1);
        assert_eq!(
            a.decision_log_hash(),
            b.decision_log_hash(),
            "forward-hash reverse-kararlardan etkilendi (ayrı-görünürlük ihlali)"
        );
        // Şema-kimlikleri stabil + forward≠reverse-şema.
        assert_eq!(a.forward_schedule_id(), b.forward_schedule_id());
        assert_eq!(a.reverse_schedule_id(), b.reverse_schedule_id());
        assert_ne!(
            a.forward_schedule_id(),
            a.reverse_schedule_id(),
            "forward ve reverse şema-kimliği çakıştı"
        );
        assert_eq!(a.configured_schedule_hash(), b.configured_schedule_hash());
    }

    /// OPT-X2 INSTRUMENTATION-INVARIANCE PIN: additive decision capture mirrors the rolling hash exactly, and the
    /// captured event vector is lossless (count == decided; drop flags match). Recompute == rolling for every spec.
    #[test]
    fn opt_x2_decision_capture_consistency() {
        use TransmissionRole::*;
        for spec in [
            exact(&[(0, 2), (0, 5)]),
            TraceSpec::NaturalIid {
                loss_permille: 200,
                seed: 42,
            },
            TraceSpec::NaturalBurst {
                drop_intervals_ns: vec![(1_000, 2_000)],
            },
        ] {
            let mut ch = TraceChannel::new(spec);
            let stream: Vec<(TracePacketId, u64)> = (0..64u32)
                .flat_map(|i| {
                    [
                        (
                            pid(InitialSource, i / 8, (i % 8) as u16, 0),
                            u64::from(i) * 500,
                        ),
                        (
                            pid(Repair, i / 8, (i % 8) as u16, 0),
                            u64::from(i) * 500 + 10,
                        ),
                        (
                            pid(RetransmitSource, i / 8, (i % 8) as u16, 1),
                            u64::from(i) * 500 + 20,
                        ),
                    ]
                })
                .collect();
            for &(id, t) in &stream {
                let _ = ch.decide(id, t);
            }
            // rolling hash (untouched) == recomputed-from-vector (additive capture faithful).
            assert_eq!(
                ch.decision_log_hash(),
                ch.recomputed_decision_log_hash(),
                "additive capture diverged from rolling hash"
            );
            // lossless: one event per decision; drop flags reconstruct counts.
            let (decided, dropped) = ch.counts();
            assert_eq!(
                ch.forward_decisions().len() as u64,
                decided,
                "capture count != decided"
            );
            assert_eq!(
                ch.forward_decisions().iter().filter(|e| e.dropped).count() as u64,
                dropped,
                "captured drops != dropped counter"
            );
        }
    }

    /// DETERMINISM-PIN: aynı trace × 3 replay → identical decision-log hash.
    #[test]
    fn trace_determinism_thrice_identical_hash() {
        use TransmissionRole::*;
        let spec = TraceSpec::NaturalIid {
            loss_permille: 200,
            seed: 42,
        };
        let stream: Vec<(TracePacketId, u64)> = (0..100u32)
            .map(|i| {
                (
                    pid(InitialSource, i / 8, (i % 8) as u16, 0),
                    u64::from(i) * 1000,
                )
            })
            .collect();
        let run = || {
            let mut ch = TraceChannel::new(spec.clone());
            for &(id, t) in &stream {
                let _ = ch.decide(id, t);
            }
            ch.decision_log_hash()
        };
        let (h1, h2, h3) = (run(), run(), run());
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);
    }
}
