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

/// FNV-1a 64-bit (deterministik; RNG-YOK — golden/replay-güvenli).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
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

/// Deterministik paired-replay kanalı. `decide()` saf-fonksiyon; karar-log determinism-pin için tutulur.
pub struct TraceChannel {
    spec: TraceSpec,
    decision_log_hash: u64,
    decided: u64,
    dropped: u64,
}

impl TraceChannel {
    #[must_use]
    pub fn new(spec: TraceSpec) -> Self {
        Self {
            spec,
            decision_log_hash: 0xcbf2_9ce4_8422_2325,
            decided: 0,
            dropped: 0,
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
        drop
    }

    /// Determinism-pin: karar-dizisinin hash'i (aynı trace × N replay → identical).
    #[must_use]
    pub fn decision_log_hash(&self) -> u64 {
        self.decision_log_hash
    }

    /// (karar-sayısı, düşürülen-sayısı) — realized-loss faithfulness raporu için.
    #[must_use]
    pub fn counts(&self) -> (u64, u64) {
        (self.decided, self.dropped)
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
