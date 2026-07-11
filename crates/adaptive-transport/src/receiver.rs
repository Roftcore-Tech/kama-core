//! Receiver State Machine (engine spec §4) — datagram al → dedup / reorder / FEC → SACK üret.
//!
//! **Deterministik çekirdek (ADR-17):** datagram metadata'sı girer → teslim + ACK çıkar. Kripto
//! DIŞINDA (INV-ENGINE-SOC): güvenlik katmanı (Noise+kompakt, §0.2) datagram'ı çözer/verify eder;
//! Receiver VERIFIED opak birim + metadata (seq/block_id/shard_index) ALIR. verify FAIL → güvenlik
//! katmanı silent drop (transport hiç görmez). Receiver seq metadata OKUR, rx-set tutar, SACK üretir
//! ([`crate::ack`]). per-shard AEAD + RS-decode GÜVENLİK katmanında (authenticate-before-decode, INV-FEC-1).
//!
//! **SoC (reorder/loss):** Receiver TÜM gap'leri SACK'te bildirir (gerçekleri raporlar); reorder
//! toleransı + loss ilanı SENDER-tarafındadır (§5 packet/time-threshold, §9.3) — spurious retransmit'i
//! sender'ın loss detector'ı önler. RX_REORDER durumu burada bilgilendiricidir.
//!
//! Wire u16 seq → full-counter çözümü ([`crate::seq::resolve_seq`]) codec sınırındadır; `on_datagram`
//! çözülmüş full-counter seq alır (sender ile simetrik).

use std::collections::BTreeSet;

use crate::ack::AckFrame;

/// Receiver durumları (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxState {
    /// in-order / near-order teslim; windowed ACK.
    Normal,
    /// gap var (reordering olası) — rx-set'te boşluk mevcut.
    Reorder,
    /// block/shard framing; eksik data shard'ları parity'den reconstruct (§10, güvenlik katmanı).
    FecDecode,
    /// duplicate/retransmit geldi, zaten teslim → drop + ACK teyit (§9.4).
    DupSuppress,
}

/// Alınan datagram'ın sınıfı + metadata'sı (transport-görünür kısım; §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxKind {
    /// SESSION_DATA — E2E opak birim teslim edilir.
    Data,
    /// FEC_SHARD — block/shard; reconstruction güvenlik katmanında (transport buffer'lar + ACK'ler).
    FecShard {
        /// FEC block kimliği.
        block_id: u64,
        /// Shard indeksi (0..k+r).
        shard_index: u16,
        /// Data shard sayısı.
        k: u16,
        /// Parity shard sayısı.
        r: u16,
    },
}

/// VERIFIED gelen datagram (güvenlik katmanı çözdü/verify etti; §0.2).
#[derive(Debug, Clone, Copy)]
pub struct RxDatagram {
    /// full-counter transport seq (codec [`crate::seq::resolve_seq`] ile çözdü).
    pub seq: u64,
    /// Opak birim kimliği (teslim için; SoC: bytes değil, kimlik).
    pub unit_id: u64,
    /// Sınıf + metadata.
    pub kind: RxKind,
}

/// `on_datagram` çıktısı: teslim edilen birimler + (tetiklendiyse) gönderilecek ACK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RxOutput {
    /// App'e teslim edilen opak birim kimlikleri (in-order garanti YOK — E2E envelope sıralar, §4).
    pub delivered: Vec<u64>,
    /// Gönderilecek SACK ACK (§2.1 frekans tetiklendiyse; yoksa `None`).
    pub ack: Option<AckFrame>,
    /// Bu datagram duplicate mıydı (RX_DUP_SUPPRESS).
    pub was_duplicate: bool,
}

/// §2.1 ACK_FREQ_N — bu kadar DATA alınınca ACK üret [CAL] (netem'den kalibre).
const ACK_FREQ_N: u32 = 2;
/// ack_delay u16 µs tavanı.
const ACK_DELAY_MAX_US: u64 = u16::MAX as u64;

/// [§16.1] SACK wire boyutu (28 UDP/IP + handle4+seq2+tag16 + ACK_FRAME_V2 gövde, 0-range) — cadence türetimi.
const SACK_WIRE_BYTES: u64 = 65;
/// [§16.1] RETURN_BUDGET_FRACTION (Y) permil — SACK+echo dönüşü ≤ Y·return_bw. 0.25 → 250.
const RETURN_BUDGET_PERMIL: u64 = 250;
/// [K2/§3.3-E] cold-start return_bw önseli (B/s); worst-corner 0.25 kbps = 31.25 B/s → 31 (int floor).
const RETURN_BW_PRIOR: u64 = 31;
/// [K2/R1/R15/§3.3-E/§12] ACK_CADENCE cold-start floor = sack_wire/(Y·return_bw_önsel) ≈ 8.4s.
/// TIMER-SACK'ları bununla seyrekleşir: kbps'te sabit 20ms tick → SACK-seli/self-congestion (19s-lag);
/// throttle bunu keser. Frequency-yolu (§2.1, on_datagram) DATA-oranıyla eşleşir → hızlı kanalda responsive
/// kalır (timer yalnız tail backstop). İlk timer-SACK cadence-exempt ([R1]); return_bw öğrenilince FULL
/// adaptif hızlanma D2 sonrası (§3.3-E). Sabit const → ADR-17 determinizm (RNG yok).
const ACK_CADENCE_FLOOR_NS: u64 =
    SACK_WIRE_BYTES * 1_000_000_000 * 1000 / (RETURN_BUDGET_PERMIL * RETURN_BW_PRIOR);

/// Receiver state machine (§4). rx-set + SACK üretimi; `on_datagram` / `on_ack_timer` ile sürülür.
pub struct Receiver {
    state: RxState,
    received: BTreeSet<u64>, // alınan full-counter seq'ler (SACK kaynağı)
    smallest_seq: u64,
    largest_seq: u64,
    have_recv: bool,
    largest_recv_ns: u64, // ack_delay için (largest_seq alım anı)
    ack_epoch: u8,
    since_last_ack: u32,
    ack_freq: u32,
    last_ack_ns: u64,     // [K2/§12] son SACK emit anı (timer cadence gate kaynağı)
    first_ack_sent: bool, // [R1] ilk SACK cadence-exempt: henüz SACK yollanmadıysa timer hemen emit eder
}

impl Receiver {
    /// Yeni receiver (epoch 0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RxState::Normal,
            received: BTreeSet::new(),
            smallest_seq: 0,
            largest_seq: 0,
            have_recv: false,
            largest_recv_ns: 0,
            ack_epoch: 0,
            since_last_ack: 0,
            ack_freq: ACK_FREQ_N,
            last_ack_ns: 0,
            first_ack_sent: false,
        }
    }

    /// §4 on_datagram: VERIFIED datagram'ı işle → teslim + belki ACK.
    pub fn on_datagram(&mut self, d: RxDatagram, now_ns: u64) -> RxOutput {
        // Duplicate (zaten alınmış seq) → RX_DUP_SUPPRESS: teslim etme, yalnız ACK teyidi (§9.4).
        if self.received.contains(&d.seq) {
            self.state = RxState::DupSuppress;
            let ack = self.tick_ack(now_ns, true); // dup ACK teyidi tetikler (sender loss'u çözsün).
            return RxOutput {
                delivered: Vec::new(),
                ack,
                was_duplicate: true,
            };
        }

        self.record_seq(d.seq, now_ns);

        let (delivered, is_shard) = match d.kind {
            RxKind::Data => (vec![d.unit_id], false),
            RxKind::FecShard { .. } => {
                // Shard buffer'lanır + ACK'lenir; RS-decode + reconstruction GÜVENLİK katmanında
                // (authenticate-before-decode, INV-FEC-1). Transport birimi burada teslim ETMEZ.
                (Vec::new(), true)
            }
        };

        self.state = if is_shard {
            RxState::FecDecode
        } else if self.has_gap() {
            RxState::Reorder
        } else {
            RxState::Normal
        };

        let ack = self.tick_ack(now_ns, false);
        RxOutput {
            delivered,
            ack,
            was_duplicate: false,
        }
    }

    /// §12 ACK_TIMER: zamanlayıcı ile ACK üret (frekans dolmasa da). [K2/R1/§3.3-E/§12] cadence-limitli:
    /// İLK SACK henüz yollanmadıysa cadence-exempt (hemen — client PRE_CONVERGE'den hızlı çıksın); yollandıysa
    /// son SACK'ten `ACK_CADENCE_FLOOR_NS` geçmeden timer-SACK BASTIRILIR (kbps 20ms-sabit SACK-seli/19s-lag
    /// önlenir). Frequency-yolu (§2.1) DATA-oranıyla eşleştiğinden hızlı kanalda responsive kalır.
    pub fn on_ack_timer(&mut self, now_ns: u64) -> Option<AckFrame> {
        if !self.have_recv {
            return None;
        }
        if self.first_ack_sent && now_ns.saturating_sub(self.last_ack_ns) < ACK_CADENCE_FLOOR_NS {
            return None;
        }
        self.since_last_ack = 0;
        self.record_ack_emit(now_ns);
        self.build_ack(now_ns)
    }

    /// rx-set + largest/smallest muhasebesi.
    fn record_seq(&mut self, seq: u64, now_ns: u64) {
        self.received.insert(seq);
        if !self.have_recv {
            self.smallest_seq = seq;
            self.largest_seq = seq;
            self.largest_recv_ns = now_ns;
            self.have_recv = true;
        } else {
            self.smallest_seq = self.smallest_seq.min(seq);
            if seq > self.largest_seq {
                self.largest_seq = seq;
                self.largest_recv_ns = now_ns; // ack_delay largest'ın alımından ölçülür.
            }
        }
    }

    /// rx-set [smallest..largest] aralığında boşluk (gap) var mı → reorder/loss göstergesi.
    fn has_gap(&self) -> bool {
        if !self.have_recv {
            return false;
        }
        let span = self.largest_seq - self.smallest_seq + 1;
        u64::try_from(self.received.len()).unwrap_or(u64::MAX) < span
    }

    /// §2.1 ACK-tetik: DATA sayacı ACK_FREQ'e ulaştıysa (ya da `force`) ACK üret. Frequency-yolu cadence
    /// ile GATE'lenmez (DATA-oranıyla eşleşir → self-limiting) ama `last_ack_ns`'i günceller → timer bunu
    /// backstop olarak referans alır (freq-SACK cadence'i sıfırlar → timer hemen tekrarlamaz).
    fn tick_ack(&mut self, now_ns: u64, force: bool) -> Option<AckFrame> {
        self.since_last_ack += 1;
        if force || self.since_last_ack >= self.ack_freq {
            self.since_last_ack = 0;
            self.record_ack_emit(now_ns);
            self.build_ack(now_ns)
        } else {
            None
        }
    }

    /// [K2/R1/§12] SACK emit muhasebesi — timer cadence gate'i beslemek için son emit anını + ilk-SACK
    /// bayrağını günceller. Her GERÇEK SACK emisyonu (freq/force/timer) buradan geçer.
    fn record_ack_emit(&mut self, now_ns: u64) {
        self.last_ack_ns = now_ns;
        self.first_ack_sent = true;
    }

    /// SACK ACK'i rx-set'ten kur (§2) — [`AckFrame::from_received`] gruplamayı yapar.
    fn build_ack(&self, now_ns: u64) -> Option<AckFrame> {
        let ack_delay_us = now_ns
            .saturating_sub(self.largest_recv_ns)
            .checked_div(1000)
            .unwrap_or(0)
            .min(ACK_DELAY_MAX_US) as u16;
        let received: Vec<u64> = self.received.iter().copied().collect();
        AckFrame::from_received(&received, self.ack_epoch, ack_delay_us)
    }

    // ── gözlemciler ──

    /// Güncel state.
    #[must_use]
    pub fn state(&self) -> RxState {
        self.state
    }
    /// Alınan farklı seq sayısı.
    #[must_use]
    pub fn received_count(&self) -> usize {
        self.received.len()
    }
    /// Alınan en yüksek seq (varsa).
    #[must_use]
    pub fn largest_seq(&self) -> Option<u64> {
        self.have_recv.then_some(self.largest_seq)
    }
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn data(seq: u64, unit_id: u64) -> RxDatagram {
        RxDatagram {
            seq,
            unit_id,
            kind: RxKind::Data,
        }
    }

    #[test]
    fn in_order_delivers_and_acks_on_frequency() {
        let mut r = Receiver::new();
        // 1. DATA: teslim, ama ACK_FREQ(2) dolmadı → ACK yok.
        let o0 = r.on_datagram(data(0, 100), 0);
        assert_eq!(o0.delivered, vec![100]);
        assert!(o0.ack.is_none(), "1 DATA < ACK_FREQ → ACK yok");
        assert_eq!(r.state(), RxState::Normal);
        // 2. DATA: teslim + ACK tetiklenir.
        let o1 = r.on_datagram(data(1, 101), 10 * MS);
        assert_eq!(o1.delivered, vec![101]);
        let ack = o1.ack.expect("ACK_FREQ dolunca ACK");
        assert_eq!(ack.largest_seq, 1);
        assert_eq!(ack.block0_len, 2, "0,1 ardışık → block0_len=2");
        assert!(ack.ranges.is_empty(), "gap yok → ek range yok");
    }

    #[test]
    fn duplicate_suppressed_not_delivered() {
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0);
        // Aynı seq tekrar → RX_DUP_SUPPRESS: teslim YOK, ACK teyidi VAR.
        let dup = r.on_datagram(data(0, 100), 5 * MS);
        assert!(dup.was_duplicate);
        assert!(dup.delivered.is_empty(), "duplicate teslim edilmez (§9.4)");
        assert_eq!(r.state(), RxState::DupSuppress);
        assert!(dup.ack.is_some(), "duplicate ACK teyidi tetikler");
        assert_eq!(r.received_count(), 1, "duplicate rx-set'i büyütmez");
    }

    #[test]
    fn gap_produces_sack_range_and_reorder_state() {
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0); // seq0
        let o = r.on_datagram(data(2, 102), 10 * MS); // seq2 (gap: seq1 eksik)
        assert_eq!(r.state(), RxState::Reorder, "gap → RX_REORDER");
        let ack = o.ack.expect("2. DATA → ACK");
        assert_eq!(ack.largest_seq, 2);
        assert_eq!(ack.block0_len, 1, "block0 = {{2}} (seq1 gap)");
        // seq0 ayrı blok: gap=2 (cursor 2 − high 0), len=1.
        assert_eq!(ack.ranges.len(), 1, "gap → 1 SACK range");
        assert_eq!(ack.ranges[0], crate::ack::AckRange { gap: 2, len: 1 });
    }

    #[test]
    fn ack_delay_reflects_elapsed_since_largest() {
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0);
        // largest (seq0) 0'da alındı; ACK timer 5ms sonra → ack_delay ≈ 5000µs.
        let ack = r.on_ack_timer(5 * MS).expect("ACK");
        assert_eq!(ack.ack_delay, 5000, "ack_delay = 5ms = 5000µs");
    }

    #[test]
    fn ack_timer_first_exempt_then_cadence_throttled() {
        // [R1/K2/§3.3-E] İlk timer-SACK cadence-exempt (hemen); sonrakiler ACK_CADENCE_FLOOR ile seyrek.
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0); // 1 DATA (freq'e ulaşmaz → on_datagram SACK üretmez)
        let a0 = r
            .on_ack_timer(20 * MS)
            .expect("[R1] ilk timer-SACK cadence-exempt → hemen emit");
        assert_eq!(a0.largest_seq, 0);
        // Cadence dolmadan gelen timer'lar → BASTIRILIR (kbps 20ms SACK-seli önlenir; §12).
        assert!(
            r.on_ack_timer(40 * MS).is_none(),
            "cadence dolmadı (20ms<8.4s) → timer-SACK bastırıldı"
        );
        assert!(
            r.on_ack_timer(500 * MS).is_none(),
            "hâlâ cadence içinde → timer-SACK bastırıldı"
        );
        // ACK_CADENCE_FLOOR (~8.4s) sonrası → tekrar emit (tail backstop).
        let a1 = r
            .on_ack_timer(9_000 * MS)
            .expect("cadence sonrası timer-SACK yine emit");
        assert_eq!(a1.largest_seq, 0);
    }

    #[test]
    fn freq_ack_resets_timer_cadence() {
        // Frequency-yolu SACK (on_datagram) da last_ack'i günceller → timer backstop hemen tekrarlamaz.
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0);
        let f = r.on_datagram(data(1, 101), 10 * MS); // freq=2 → SACK, last_ack=10ms, first_ack_sent=true
        assert!(f.ack.is_some(), "freq dolunca SACK");
        assert!(
            r.on_ack_timer(30 * MS).is_none(),
            "freq-SACK cadence'i sıfırladı (20ms<8.4s) → timer-SACK bastırıldı"
        );
    }

    #[test]
    fn fec_shard_buffered_not_delivered() {
        let mut r = Receiver::new();
        let shard = RxDatagram {
            seq: 0,
            unit_id: 100,
            kind: RxKind::FecShard {
                block_id: 7,
                shard_index: 0,
                k: 4,
                r: 2,
            },
        };
        let o = r.on_datagram(shard, 0);
        assert!(
            o.delivered.is_empty(),
            "shard reconstruction güvenlik katmanında — transport teslim etmez (INV-FEC-1)"
        );
        assert_eq!(r.state(), RxState::FecDecode);
        assert_eq!(r.received_count(), 1, "shard ACK için rx-set'e girer");
    }
}
