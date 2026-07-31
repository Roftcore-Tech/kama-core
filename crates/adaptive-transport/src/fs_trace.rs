//! DR-FEC-FS-1 FS-HARNESS-A — TRACE-DRIVEN PAIRED REPLAY CHANNEL (Abkarada v5 pin-2).
//!
//! **Frozen pairing semantics.** Wire-ordinal drop replay YASAK: kollar (A0/A1-EARLY/F0/…) farklı
//! sayıda wire-packet emit eder → "drop packet #N" kollar-arası AYNI channel-realization'ı YAŞATMAZ.
//! Bu yüzden iki trace-sınıfı, ikisi de arm-INVARIANT keyed:
//!
//! - [`TraceSpec::ExactDeficit`] (C-P1/C-P2 mechanism): **semantic SOURCE-id keyed** loss — drop
//!   `Source{block,esi}` ∈ drop-set; repair/duplicate/control PASS (trace explicit demezse). Deficit
//!   böylece kollar-arası GERÇEKTEN eşlenir (A0 kaybeder→ARQ; A1 dup-recovers; F0 parity-decodes; hepsi
//!   AYNI source-deficit'te).
//! - [`TraceSpec::NaturalIid`] / [`TraceSpec::NaturalBurst`]: arm-INDEPENDENT deterministic channel —
//!   IID per-semantic-id hash-keyed (aynı-id → aynı-karar), burst time-domain interval (arm packet-
//!   count'undan bağımsız). Wire-ordinal DEĞİL.
//!
//! **Determinism-pin:** aynı trace × N replay → identical decision-log hash (RNG-YOK; saf-fonksiyon).

use std::collections::BTreeSet;

/// Kanal-kararı için paketin SEMANTİK kimliği (wire-ordinal DEĞİL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TracePacketId {
    /// Kaynak-shard (block_id, esi).
    Source { block_id: u32, esi: u16 },
    /// Parity/repair-shard (block_id, esi).
    Repair { block_id: u32, esi: u16 },
    /// A1-EARLY kaynak-kopyası (aynı source'un duplicate'ı).
    Duplicate { block_id: u32, esi: u16 },
    /// Kontrol (ACK/CS/…): kanal-loss'undan muaf (feedback-yolu; ayrı reverse-trace ile modellenir).
    Control,
}

/// Dondurulmuş trace-spesifikasyonu (iki-sınıf).
#[derive(Debug, Clone)]
pub enum TraceSpec {
    /// C-P1/C-P2: yalnız adlandırılmış source-shard'ları düşür; gerisi PASS.
    ExactDeficit { drop_sources: BTreeSet<(u32, u16)> },
    /// Doğal IID: per-semantic-id deterministic hash-keyed drop (arm-invariant; aynı-id → aynı-karar).
    NaturalIid { loss_permille: u32, seed: u64 },
    /// Doğal burst: arm-bağımsız time-domain drop-aralıkları [lo,hi) ns (data-paketlerine uygulanır).
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

fn id_key(id: TracePacketId) -> [u8; 15] {
    // discriminant + block + esi (Control/Source/Repair/Duplicate ayrık namespace).
    let (disc, block, esi): (u8, u32, u16) = match id {
        TracePacketId::Source { block_id, esi } => (1, block_id, esi),
        TracePacketId::Repair { block_id, esi } => (2, block_id, esi),
        TracePacketId::Duplicate { block_id, esi } => (3, block_id, esi),
        TracePacketId::Control => (4, 0, 0),
    };
    let mut k = [0u8; 15];
    k[0] = disc;
    k[1..5].copy_from_slice(&block.to_le_bytes());
    k[5..7].copy_from_slice(&esi.to_le_bytes());
    // 7..15 seed için decide()'da ayrı karılır.
    k
}

/// Deterministik paired-replay kanalı. `decide()` saf-fonksiyon (spec+id+time); karar-log'u
/// determinism-pin için tutulur.
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
            TraceSpec::ExactDeficit { drop_sources } => match id {
                TracePacketId::Source { block_id, esi } => drop_sources.contains(&(block_id, esi)),
                _ => false, // repair/duplicate/control PASS (deficit yalnız adlandırılmış source'ta)
            },
            TraceSpec::NaturalIid {
                loss_permille,
                seed,
            } => match id {
                TracePacketId::Control => false, // kontrol muaf
                _ => {
                    let mut k = id_key(id).to_vec();
                    k.extend_from_slice(&seed.to_le_bytes());
                    (fnv1a(&k) % 1000) < u64::from(*loss_permille)
                }
            },
            TraceSpec::NaturalBurst { drop_intervals_ns } => match id {
                TracePacketId::Control => false,
                _ => drop_intervals_ns
                    .iter()
                    .any(|&(lo, hi)| emit_time_ns >= lo && emit_time_ns < hi),
            },
        };
        // Karar-log hash'i (determinism-pin): id-key + time + drop.
        let mut rec = id_key(id).to_vec();
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

    /// EXACT-DEFICIT: yalnız adlandırılmış source düşer; repair/duplicate/control PASS.
    #[test]
    fn exact_deficit_drops_only_named_sources() {
        let mut ch = TraceChannel::new(TraceSpec::ExactDeficit {
            drop_sources: set(&[(0, 2), (0, 5)]),
        });
        assert!(ch.decide(
            TracePacketId::Source {
                block_id: 0,
                esi: 2
            },
            0
        ));
        assert!(ch.decide(
            TracePacketId::Source {
                block_id: 0,
                esi: 5
            },
            0
        ));
        assert!(!ch.decide(
            TracePacketId::Source {
                block_id: 0,
                esi: 3
            },
            0
        ));
        // dup of a DROPPED source PASSES → A1 kurtarabilir (redundancy-benefit).
        assert!(!ch.decide(
            TracePacketId::Duplicate {
                block_id: 0,
                esi: 2
            },
            0
        ));
        // parity PASSES → F0 decode edebilir.
        assert!(!ch.decide(
            TracePacketId::Repair {
                block_id: 0,
                esi: 0
            },
            0
        ));
        assert!(!ch.decide(TracePacketId::Control, 0));
    }

    /// ARM-INVARIANT: A0(source-only) · A1(source+dup) · F0(source+repair) AYNI source-deficit yaşar.
    #[test]
    fn exact_deficit_source_deficit_is_arm_invariant() {
        let spec = TraceSpec::ExactDeficit {
            drop_sources: set(&[(0, 2), (0, 5)]),
        };
        // Her kol için kendi paket-akışını sür; DÜŞEN source-id kümesini topla.
        let arms: [Vec<TracePacketId>; 3] = [
            // A0: yalnız source 0..7
            (0..8)
                .map(|e| TracePacketId::Source {
                    block_id: 0,
                    esi: e,
                })
                .collect(),
            // A1: source 0..7 + her source'un duplicate'ı
            (0..8)
                .flat_map(|e| {
                    [
                        TracePacketId::Source {
                            block_id: 0,
                            esi: e,
                        },
                        TracePacketId::Duplicate {
                            block_id: 0,
                            esi: e,
                        },
                    ]
                })
                .collect(),
            // F0: source 0..7 + 2 parity
            (0..8)
                .map(|e| TracePacketId::Source {
                    block_id: 0,
                    esi: e,
                })
                .chain((0..2).map(|e| TracePacketId::Repair {
                    block_id: 0,
                    esi: e,
                }))
                .collect(),
        ];
        let mut dropped_sources_per_arm = Vec::new();
        for stream in &arms {
            let mut ch = TraceChannel::new(spec.clone());
            let mut dropped: BTreeSet<(u32, u16)> = BTreeSet::new();
            for &id in stream {
                if ch.decide(id, 0) {
                    if let TracePacketId::Source { block_id, esi } = id {
                        dropped.insert((block_id, esi));
                    }
                }
            }
            dropped_sources_per_arm.push(dropped);
        }
        // Üç kolun DÜŞEN-source-kümesi ÖZDEŞ (deficit gerçekten eşlendi).
        assert_eq!(dropped_sources_per_arm[0], set(&[(0, 2), (0, 5)]));
        assert_eq!(dropped_sources_per_arm[0], dropped_sources_per_arm[1]);
        assert_eq!(dropped_sources_per_arm[0], dropped_sources_per_arm[2]);
    }

    /// NATURAL-IID: deterministic + arm-independent (aynı-id iki-kanal aynı-karar).
    #[test]
    fn natural_iid_deterministic_and_arm_independent() {
        let spec = TraceSpec::NaturalIid {
            loss_permille: 300,
            seed: 7,
        };
        let mut a = TraceChannel::new(spec.clone());
        let mut b = TraceChannel::new(spec);
        for e in 0..64u16 {
            let id = TracePacketId::Source {
                block_id: 1,
                esi: e,
            };
            assert_eq!(
                a.decide(id, 0),
                b.decide(id, 0),
                "esi {e}: arm-bağımlı-karar"
            );
        }
        // ~%30 civarı (deterministik; ama non-trivial: hiç-drop/hep-drop değil).
        let (n, d) = a.counts();
        assert!(d > 0 && d < n, "IID dejenere (d={d}/n={n})");
    }

    /// NATURAL-BURST: time-domain; identity/count'tan bağımsız (aynı-t aynı-karar).
    #[test]
    fn natural_burst_time_domain() {
        let mut ch = TraceChannel::new(TraceSpec::NaturalBurst {
            drop_intervals_ns: vec![(1_000, 2_000)],
        });
        assert!(ch.decide(
            TracePacketId::Source {
                block_id: 0,
                esi: 0
            },
            1_500
        ));
        assert!(!ch.decide(
            TracePacketId::Source {
                block_id: 0,
                esi: 1
            },
            2_500
        ));
        // Parity de aynı-aralıkta düşer (data-sınıfı); kontrol muaf.
        assert!(ch.decide(
            TracePacketId::Repair {
                block_id: 0,
                esi: 0
            },
            1_200
        ));
        assert!(!ch.decide(TracePacketId::Control, 1_500));
    }

    /// DETERMINISM-PIN: aynı trace × 3 replay → identical decision-log hash.
    #[test]
    fn trace_determinism_thrice_identical_hash() {
        let spec = TraceSpec::NaturalIid {
            loss_permille: 200,
            seed: 42,
        };
        let stream: Vec<(TracePacketId, u64)> = (0..100u32)
            .map(|i| {
                (
                    TracePacketId::Source {
                        block_id: i / 8,
                        esi: (i % 8) as u16,
                    },
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
        assert_eq!(h1, h2, "replay-1≠2 (determinism ihlali)");
        assert_eq!(h2, h3, "replay-2≠3 (determinism ihlali)");
    }
}
