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

use crate::fs_trace::{TracePacketId, TransmissionRole};

/// Bir kolun redundancy sealed-wire maliyeti (C-P4 realized-byte fairness ekseni).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RedundancyCost {
    pub configured_r: u16,
    /// Redundancy payload (header-hariç) — configured muhasebe.
    pub redundancy_payload_bytes: u64,
    /// GERÇEK sealed-wire (header/framing dahil) — **C-P4 fairness bunun üstünden**.
    pub realized_sealed_wire_bytes: u64,
    pub packet_count: u64,
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
            self.cost.realized_sealed_wire_bytes += sealed.len() as u64;
            self.cost.redundancy_payload_bytes += payload_len;
            self.cost.packet_count += 1;
        }
    }

    #[must_use]
    pub fn cost(&self) -> RedundancyCost {
        self.cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sender::{AckInfo, Event, OpaqueUnit, Sender};
    use std::time::Duration;

    const MS: u64 = 1_000_000;

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
        assert_eq!(cost.packet_count, 2, "r=2 → 2 kopya");
        // REALIZED sealed-wire = 2 × 108 = 216 (header/framing DAHİL; C-P4 fairness ekseni).
        assert_eq!(cost.realized_sealed_wire_bytes, 216);
        assert_eq!(cost.redundancy_payload_bytes, 130); // 2 × 65 (payload)
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
        assert_eq!(emitter.cost().packet_count, 3);
        let dup_ids: Vec<(u32, u16)> = sink.packets.iter().map(|(id, _)| id.source_id).collect();
        assert_eq!(dup_ids, vec![(0, 0), (0, 1), (0, 2)]);
    }
}
