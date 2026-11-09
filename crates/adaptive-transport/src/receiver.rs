//! Receiver State Machine (engine spec §4) — datagram al → dedup / reorder / FEC → SACK üret.
//!
//! **Deterministik çekirdek (ADR-17):** datagram metadata'sı girer → teslim + ACK çıkar. Kripto
//! DIŞINDA (INV-ENGINE-SOC): güvenlik katmanı (Noise+kompakt, §0.2) datagram'ı çözer/verify eder;
//! Receiver VERIFIED opak birim + metadata (seq/block_id/esi/k_actual — schema §2.6) ALIR. verify FAIL → güvenlik
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
    /// **Genişlik schema §2.6 ile hizalı (DR-S5-2, N8/N3-düzeltme):** block_id u32 · esi u16 · k_actual u8 ·
    /// is_repair (source/repair-ayrımı). r-alanı KALKTI (R_code_max profile/code-space'ten; §4 R-üçlü).
    FecShard {
        /// FEC block kimliği (schema §2.6: session+direction-scoped u32).
        block_id: u32,
        /// Encoding Symbol ID (schema §2.6: esi u16).
        esi: u16,
        /// Bloktaki gerçek source-shard sayısı (schema §2.6: k_actual u8; B4: 1≤k_actual≤255).
        k_actual: u8,
        /// source/repair ayrımı (schema ShardKind).
        is_repair: bool,
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
/// [TUR-1x] ack_delay V2 encoded tavanı (raw u16). 65535×256µs ≈ 16.78s > cadence-floor 8.39s →
/// timer-SACK hold'u artık satüre OLMAZ (eski µs-birim tavanı 65.5ms'ti — hold'da anında satüre,
/// RTT düzeltmesi etkisizdi; §16.2 saat-sınıfı haritası "ack_delay" satırı).
const ACK_DELAY_ENC_MAX: u64 = u16::MAX as u64;

/// [§16.1] SACK wire boyutu (28 UDP/IP + handle4+seq2+tag16 + ACK_FRAME_V2 gövde, 0-range) — cadence türetimi.
const SACK_WIRE_BYTES: u64 = 65;
/// [§16.1] RETURN_BUDGET_FRACTION (Y) permil — SACK+echo dönüşü ≤ Y·return_bw. 0.25 → 250.
const RETURN_BUDGET_PERMIL: u64 = 250;
/// [K2/§3.3-E] cold-start return_bw önseli (B/s); worst-corner 0.25 kbps = 31.25 B/s → 31 (int floor).
const RETURN_BW_PRIOR: u64 = 31;
/// [Q1 A-2] Cadence tabanı, dönüş-debisi girdisi parametreli: `sack_wire / (Y · rb)` (ns).
/// `rb = RETURN_BW_PRIOR` iken [`ACK_CADENCE_FLOOR_NS`] ile AYNI tamsayı ifadesi (bit-aynı).
const fn cadence_floor_for(return_bw_bps: u64) -> u64 {
    let rb = if return_bw_bps == 0 { 1 } else { return_bw_bps };
    SACK_WIRE_BYTES * 1_000_000_000 * 1000 / (RETURN_BUDGET_PERMIL * rb)
}
/// [Q1 A-2] Kota-modu aralığı, dönüş-debisi girdisi parametreli: `sack_wire / rb` (ns).
const fn t_spacing_for(return_bw_bps: u64) -> u64 {
    let rb = if return_bw_bps == 0 { 1 } else { return_bw_bps };
    SACK_WIRE_BYTES * 1_000_000_000 / rb
}
/// [K2/R1/R15/§3.3-E/§12] ACK_CADENCE cold-start floor = sack_wire/(Y·return_bw_önsel) ≈ 8.4s.
/// TIMER-SACK'ları bununla seyrekleşir: kbps'te sabit 20ms tick → SACK-seli/self-congestion (19s-lag);
/// throttle bunu keser. Frequency-yolu (§2.1, on_datagram) DATA-oranıyla eşleşir → hızlı kanalda responsive
/// kalır (timer yalnız tail backstop). İlk timer-SACK cadence-exempt ([R1]); return_bw öğrenilince FULL
/// adaptif hızlanma D2 sonrası (§3.3-E). Sabit const → ADR-17 determinizm (RNG yok).
const ACK_CADENCE_FLOOR_NS: u64 = cadence_floor_for(RETURN_BW_PRIOR);
/// [TUR-3b commit-C, karar-a DAR-PARÇA] Tail-muafiyeti çarpanı [CAL {1,2,4}]: beklemede-veri varken
/// sessizlik ≥ K_TAIL×son-varış-aralığı → timer-SACK cadence-MUAF (R1 ilk-SACK muafiyetinin
/// tail-simetriği). Formül KANAL-ÖLÇÜLÜ (per-arrival aralık; yeni ms-sabiti YOK — ZM-2): fast'ta
/// ms'ler-içinde tail-SACK (QUIC max_ack_delay-sınıfı davranış formülden doğar); sub-kbps'te
/// aralık ≥ cadence → muafiyet fiilen devreye girmez (cadence-borcu §3.3-E-FULL'de, AYRI).
const K_TAIL: u64 = 2;

/// [TUR-5 return-supply r3 §1a] Epizot-dedektörü çarpanı — BOYUTSUZ kanal-türevli oran
/// [CAL {3,4}] (ZM-yasağı mutlak-ZAMAN sabitleridir; boyutsuz oran izinli). K_TAIL'den AYRI:
/// tail-emisyon-ritmi ≠ epizot-tespiti (r2'nin yeniden-kullanımı semantik-hataydı — talimat).
/// K=4 default: 25ms-sınıfı ritimlerde tamsayı-tam-bölünme tam-sınırı korur (V-5).
const K_EPISODE: u64 = 4;
/// [r3 §1a] Ritim-penceresi boyu [CAL {3,5}] — robust-baseline (max→median) örnek sayısı.
const EPISODE_WINDOW: usize = 3;
/// [r3 §2] Epizot-başı SACK-tekrar kredisi (muaf-ilk DAHİL toplam) [CAL {4,8}]; placeholder
/// golden'a girmez — ölçüm-matrisi bench `--quota-r` ile koşar.
const QUOTA_R_DEFAULT: u32 = 4;
/// [r3 §2] Kota-modu emisyon-aralığı = SACK worst-corner serialization'ı ≈ 2.1s.
/// STATİK-ÖNSEL-TÜREVİ (dürüst-etiket; E-FULL'de ölçülen return_bw'ye bağlanır —
/// §3.7 statik-prior borcunun yeni-tüketicisi). Dekorelasyon + dar-dönüş doğal-seyrekleşme.
const T_SPACING_NS: u64 = t_spacing_for(RETURN_BW_PRIOR);
/// [r3 §3] §3.3-E MİNİMAL-TABAN çarpanı [CAL {2,4}] ({1} elendi: yapısal paket-başı-ACK):
/// beklemede-yeni-bilgi varken timer-cadence = clamp(K_ARR×last_gap, T_SPACING, FLOOR).
const K_ARR: u64 = 2;

/// [SR-AUDIT-001 / P3] rx-set kayan-pencere tavanı: `received` en çok bu kadar ayrık seq tutar.
/// 4096: gerçek reorder/inflight ufku (yüzlerce) × geniş güvenlik payı; `from_received` zaten yalnız
/// [`crate::ack::MAX_ACK_RANGES`]=16 range üretir → daha büyük pencere DEDUP dışında işe yaramaz ama
/// her SACK'in collect+sort maliyetini şişirirdi. 4096 → rx-state ~160KB'ta sabit + SACK ucuz. Düşman
/// (ya da çok-uzun-ömürlü) ayrık-seq akışında en-eski seq O(log n) `pop_first` ile düşer; kümülatif
/// SACK kaybı telafi eder (pencere-dışı çok-geç dup teorik — E2E envelope §4 + client-core dedup nihai).
const RX_DEDUP_WINDOW: usize = 4096;

/// [Q1 A-2] Varış-debisi halkası boyu (son N varış; sabit bellek, SR-AUDIT-001). Debi = halkadaki
/// ilk varış hariç baytlar / (en yeni − en eski varış anı) — tek tek aralık oranlarının EWMA'sı
/// DEĞİL (yığılmış varışta yukarı sapmaz).
const ARRIVAL_RATE_RING: usize = 8;

/// [Q1] Alıcının E2a kollarının SALT GÖZLEM sayaçları (karar girdisi değil).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RxE2aDiag {
    /// A-1 (`gap_ack`): sıra dışı / boşluk yaratan datagram yüzünden ANINDA üretilen ACK sayısı
    /// (frekans yolunun zaten üreteceği ACK'ler dahil değil).
    pub gap_ack_emits: u64,
    /// A-2: rejim içinde (önsel dışı) hesaplanmış timer-cadence ile üretilen ACK sayısı.
    pub regime_timer_emits: u64,
}

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
    last_ack_ns: u64, // [K2/§12] son SACK emit anı (timer cadence gate kaynağı)
    // [TUR-5 r3] Epizot-dedektörü + kota (R1 ilk-SACK muafiyetinin genellemesi — first_ack_sent
    // bayrağının yerini oturum-ilk-epizot aldı):
    gap_window: [u64; 5], // ritim-penceresi (EPISODE_WINDOW aktif; clamp'li-yazım §1a)
    gap_len: usize,
    gap_idx: usize,
    quota: u32,   // kalan epizot-kredisi (emisyonla yanar, T_SPACING-takvimli)
    quota_r: u32, // R [CAL {4,8}] — bench ölçüm-matrisi set_quota_r ile
    episode_first_pending: bool, // epizodun İLK emisyonu gecikmesiz (muafiyet)
    diag_episode_count: u64,
    diag_quota_burn: u64, // kredi-düşümü toplamı — K-4 SERT-INVARIANT: ≤ R×episode_count
    diag_accel_emits: u64, // bacak-2 hızlanmış-cadence emisyonları
    diag_emit_total: u64, // tüm SACK emisyonları (RTRAJ)
    ack_delay_saturated: u64, // [TUR-1x] encoded==65535 sayısı (hold > 16.78s — audit; beklenen 0)
    seen_total: u32, // [§2.2 V2 ölçüm eki] görülen TÜM datagram (birim+kopya+shard; saturating)
    last_arrival_ns: u64, // [3b-C] HERHANGİ datagram'ın son varışı (dup dahil — kanal-tempo kanıtı)
    last_gap_ns: u64, // [3b-C] son varış-aralığı (serialization-vekili; 0 = henüz tek varış)
    last_tail_ack_ns: u64, // [3b-C rev2] tail-SACK'in KENDİ ritim-sayacı (cadence'ten bağımsız)
    // ── [Q1] E2a alıcı kolları (varsayılan kapalı = bit-aynı) ──
    e2a: crate::loss::E2aFlags,
    /// A-2: bu alıcının ACK (çıkış) yönü için çağıranın verdiği ölçülen debi (B/s); 0 = yok.
    return_bw_hint_bps: u64,
    /// A-2: ileri yön varış halkası `(an_ns, bayt)` — göndericinin pace'inin alıcıdaki vekili.
    /// Yalnız [`Receiver::observe_arrival_bytes`] yazar; bayrak kapalıyken hiçbir karar okumaz.
    arrivals: [(u64, u64); ARRIVAL_RATE_RING],
    arrivals_len: usize,
    arrivals_idx: usize,
    e2a_diag: RxE2aDiag,
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
            gap_window: [0; 5],
            gap_len: 0,
            gap_idx: 0,
            quota: 0,
            quota_r: QUOTA_R_DEFAULT,
            episode_first_pending: false,
            diag_episode_count: 0,
            diag_quota_burn: 0,
            diag_accel_emits: 0,
            diag_emit_total: 0,
            ack_delay_saturated: 0,
            seen_total: 0,
            last_arrival_ns: 0,
            last_gap_ns: 0,
            last_tail_ack_ns: 0,
            e2a: crate::loss::E2aFlags::default(),
            return_bw_hint_bps: 0,
            arrivals: [(0, 0); ARRIVAL_RATE_RING],
            arrivals_len: 0,
            arrivals_idx: 0,
            e2a_diag: RxE2aDiag::default(),
        }
    }

    /// [Q1] Alıcı kural bayraklarını kurar: A-1 `gap_ack`, A-2 `return_prior_gated`. Diğer
    /// alanlar göndericiye aittir, burada okunmaz. Varsayılan kapalı.
    pub fn set_e2a_flags(&mut self, flags: crate::loss::E2aFlags) {
        self.e2a = flags;
    }

    /// [Q1] Kurulu bayraklar (gözlem/test).
    #[must_use]
    pub fn e2a_flags(&self) -> crate::loss::E2aFlags {
        self.e2a
    }

    /// [Q1 A-2] Bu alıcının ACK yönünün ölçülen debisi (B/s; 0 = ölçüm yok). Kaynak çağıranındır
    /// (alıcı kendi çıkış yolunu ölçemez: ACK'lerin ACK'i yok). Rejim içinde varış debisinin
    /// (simetri vekili) yerine geçer; rejim kapısı yine varış debisiyle.
    pub fn set_return_bw_hint(&mut self, bps: u64) {
        self.return_bw_hint_bps = bps;
    }

    /// [Q1 A-2] İleri yön datagramının TELDEKİ boyu (çağıran, `on_datagram`'dan önce). Salt
    /// ölçüm: sabit halka; bayrak kapalıyken hiçbir karar okumaz (bit-aynı).
    pub fn observe_arrival_bytes(&mut self, wire_bytes: u64, now_ns: u64) {
        self.arrivals[self.arrivals_idx] = (now_ns, wire_bytes);
        self.arrivals_idx = (self.arrivals_idx + 1) % ARRIVAL_RATE_RING;
        self.arrivals_len = (self.arrivals_len + 1).min(ARRIVAL_RATE_RING);
    }

    /// [Q1 A-2] İleri yön varış debisi (B/s): halkadaki en eski varış hariç baytlar / zaman
    /// açıklığı. < 2 örnek ya da sıfır açıklık → 0 (ölçüm yok).
    #[must_use]
    pub fn arrival_rate_bps(&self) -> u64 {
        let n = self.arrivals_len;
        if n < 2 {
            return 0;
        }
        // En eski = yazma imlecinin n adım gerisi (halka doluysa imlecin kendisi).
        let oldest = (self.arrivals_idx + ARRIVAL_RATE_RING - n) % ARRIVAL_RATE_RING;
        let newest = (self.arrivals_idx + ARRIVAL_RATE_RING - 1) % ARRIVAL_RATE_RING;
        let span = self.arrivals[newest]
            .0
            .saturating_sub(self.arrivals[oldest].0);
        if span == 0 {
            return 0;
        }
        let bytes: u64 = (1..n)
            .map(|k| self.arrivals[(oldest + k) % ARRIVAL_RATE_RING].1)
            .fold(0u64, u64::saturating_add);
        u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(span)).unwrap_or(u64::MAX)
    }

    /// [Q1] E2a alıcı kollarının salt gözlem sayaçları.
    #[must_use]
    pub fn diag_e2a(&self) -> RxE2aDiag {
        self.e2a_diag
    }

    /// [Q1 A-2] Rejim içi dönüş debisi (B/s); `None` = önsel rejimi. Bayrak kapalı ya da varış
    /// debisi (göndericinin pace'inin vekili) < [`crate::loss::E2A_RETURN_REGIME_MIN_BPS`] → `None`.
    /// Rejimde: ölçülen ipucu (varsa) yoksa varış debisi (simetri varsayımı); en az önsel.
    fn regime_return_bw(&self) -> Option<u64> {
        if !self.e2a.return_prior_gated() {
            return None;
        }
        let pace_proxy = self.arrival_rate_bps();
        if pace_proxy < crate::loss::E2A_RETURN_REGIME_MIN_BPS {
            return None;
        }
        let rb = if self.return_bw_hint_bps > 0 {
            self.return_bw_hint_bps
        } else {
            pace_proxy
        };
        Some(rb.max(RETURN_BW_PRIOR))
    }

    /// [Q1 A-2] Bekleyen-bilgi cadence üst sınırı: önselde [`ACK_CADENCE_FLOOR_NS`]; rejimde
    /// `max(25 ms, sack_wire / (Y·rb))`.
    fn cadence_floor_ns(&self) -> u64 {
        match self.regime_return_bw() {
            None => ACK_CADENCE_FLOOR_NS,
            Some(rb) => cadence_floor_for(rb).max(crate::loss::E2A_ACK_CADENCE_MIN_NS),
        }
    }

    /// [Q1 A-2] Kota aralığı + bekleyen-bilgi cadence alt sınırı: önselde [`T_SPACING_NS`]; rejimde
    /// `max(25 ms, sack_wire / rb)`.
    fn t_spacing_ns(&self) -> u64 {
        match self.regime_return_bw() {
            None => T_SPACING_NS,
            Some(rb) => t_spacing_for(rb).max(crate::loss::E2A_ACK_CADENCE_MIN_NS),
        }
    }

    /// §4 on_datagram: VERIFIED datagram'ı işle → teslim + belki ACK.
    pub fn on_datagram(&mut self, d: RxDatagram, now_ns: u64) -> RxOutput {
        // [§2.2 V2] seen_count: HER görülen datagram sayılır (dup/shard DAHİL) — sender'ın
        // "seen vs sent" yön-ayrımlı kayıp kıyası için (K3 cold-start).
        self.seen_total = self.seen_total.saturating_add(1);
        // [TUR-5 r3 §1a] EPİZOT-DEĞERLENDİRME — pencere/last_gap GÜNCELLENMEDEN ÖNCE (kopma-gap'i
        // ritim-referansını zehirleyemez). Oturum-ilk = açık özel-durum. Dup dahil her datagram.
        if self.seen_total == 1 {
            self.open_episode();
        } else {
            let gap = now_ns.saturating_sub(self.last_arrival_ns);
            let baseline = self.episode_baseline();
            if baseline > 0 && gap > K_EPISODE.saturating_mul(baseline) {
                self.open_episode();
            }
            // SİMETRİK CLAMP'Lİ-YAZIM (r3-doğrulama): zehirlenme-koruması + livelock-ölümü +
            // mikro-gap tabanı tek kuralda; count=0'da (ilk gap) ham — sınıflandırılamaz (beyanlı).
            let written = if baseline > 0 {
                gap.clamp(baseline / K_EPISODE, baseline.saturating_mul(K_EPISODE))
            } else {
                gap
            };
            self.push_gap(written);
        }
        // [3b-C] kanal-tempo ölçümü: varış-aralığı HAM kalır (tail/bacak-2 girdisi — anlık ritim;
        // epizot-penceresi AYRI iş, r3 §3).
        if self.seen_total > 1 {
            self.last_gap_ns = now_ns.saturating_sub(self.last_arrival_ns);
        }
        self.last_arrival_ns = now_ns;
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

        // [Q1 A-1 / m5] RFC 9000 §13.2.1 (quinn-proto 0.11.18 `connection/spaces.rs`
        // `PendingAcks::is_out_of_order`, `reordering_threshold = 1`; MIT/Apache-2.0): yeni datagram
        // görülen en büyükten KÜÇÜKSE (geç varış) ya da arada eksik bırakarak BÜYÜKSE (yeni boşluk)
        // ACK gecikmesiz. En büyüğün üstünde hiçbir seq alınmamış olduğundan "arada eksik" ⟺
        // `seq > largest + 1`. İlk datagram sınıflandırılmaz. Kayıt GÜNCELLENMEDEN önce okunur.
        let out_of_order = self.e2a.gap_ack()
            && self.have_recv
            && (d.seq < self.largest_seq || d.seq > self.largest_seq.saturating_add(1));

        self.record_seq(d.seq, now_ns);

        let (delivered, is_shard) = match d.kind {
            RxKind::Data => (vec![d.unit_id], false),
            RxKind::FecShard { .. } => {
                // ADMISSION-KÖPRÜSÜ (DR-S5-2): RxDatagram ZATEN open-edilmiş/authenticated (güvenlik-katmanı
                // AEAD-verify etti — INV-FEC-1 authenticate-before-decode). Buraya YALNIZ sealed-type'tan-gelen
                // shard ulaşır (forged-shard bu-noktaya ERİŞEMEZ). Shard buffer'lanır + ACK'lenir; RS-decode +
                // reconstruction GÜVENLİK/engine-katmanında (opened-shard → fec-codec; kama-integration-pipeline).
                // Transport birimi burada teslim ETMEZ. **N8: shard-admission BtlBw/credit-YOLU-AÇMAZ** (kredi
                // yalnız gerçek-wire-ACK'ten, sender.bw.rs; RECOVERED logical-ACK≠credit).
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

        let freq_due = self.since_last_ack.saturating_add(1) >= self.ack_freq;
        let ack = self.tick_ack(now_ns, out_of_order);
        if out_of_order && !freq_due {
            self.e2a_diag.gap_ack_emits = self.e2a_diag.gap_ack_emits.saturating_add(1);
        }
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
        // [TUR-3b commit-C, karar-a] TAIL-MUAFİYETİ (R1 ilk-SACK muafiyetinin simetriği):
        // beklemede-veri (freq'e ulaşmamış varış) VE sessizlik ≥ K_TAIL×son-varış-aralığı →
        // bu bir TAIL'dir; cadence beklenmez (eski davranışta tek-kalan-paketin ACK'i 8.39s
        // cadence'e takılıyor, teslim liveness-emergency gizli-yoluna dayanıyordu — MASKE #5).
        // [rev2 — teşhis-kollarıyla ölçüldü] Tail-SACK cadence-saatini SIFIRLAMAZ: b3-matrisi
        // p5'te {0,10,10,56,100-yavaş,100-yavaş} verdi; kol-atıflaması tail-emisyonların
        // record_ack_emit'le garantili-ritim (cadence) SACK'lerini ERTELEDİĞİNİ gösterdi
        // (%80-kayıplı dönüşte hayatta-kalan SACK fiilen azaldı; r5=0 bileşik-zincir).
        // Tail artık KENDİ ritim-sayacını tutar (spam-siz) ve cadence bağımsız akar —
        // tail-SACK saf-EK emisyondur.
        let tail_due = self.since_last_ack > 0
            && self.last_gap_ns > 0
            && now_ns.saturating_sub(self.last_arrival_ns)
                >= self.last_gap_ns.saturating_mul(K_TAIL)
            && now_ns.saturating_sub(self.last_tail_ack_ns)
                >= self.last_gap_ns.saturating_mul(K_TAIL);
        if tail_due {
            self.last_tail_ack_ns = now_ns; // yalnız tail-ritmi; cadence/freq sayaçlarına DOKUNMA
            self.burn_quota(); // [r3 §2] her emisyon kredi düşürür (birleşik muhasebe)
            return self.build_ack(now_ns);
        }
        let elapsed = now_ns.saturating_sub(self.last_emit_any());
        // [r3 §2] KOTA-MODU: epizot-kredisi varken T_SPACING-takvimli yeniden-deneme (aynı
        // kümülatif-SACK'in dekorele kopyaları — kader-penceresi); epizodun İLK emisyonu
        // gecikmesiz (muafiyet — eski first_ack_sent'in genellemesi). Kopyalar yeni-varış
        // BEKLEMEZ (retry-semantiği; Ekleme-2 geri-çekildi, not §2 revizyonu).
        if self.quota > 0 {
            if self.episode_first_pending || elapsed >= self.t_spacing_ns() {
                self.since_last_ack = 0;
                self.burn_quota();
                self.record_ack_emit(now_ns);
                return self.build_ack(now_ns);
            }
            return None;
        }
        // [r3 §3] Bacak-2 §3.3-E MİNİMAL-TABAN (kota-dışı): beklemede-yeni-bilgi varken cadence
        // varış-ritmine bağlanır; yoksa statik-önsel FLOOR (sessiz-dönem tekrarları korunur).
        // [Q1 A-2] Bekleyen-bilgi kolunun sınırları rejim kapılı (bayrak kapalı → sabitler).
        // BOŞTA tekrar (yeni bilgi yok) tabanı bilinçli olarak önselde KALIR: rejimde o kol 25 ms'de
        // bir sınırsız tekrar SACK'i olurdu (quinn yeni veri yokken hiç ACK üretmez) [kod].
        let cadence_eff = if self.since_last_ack > 0 && self.last_gap_ns > 0 {
            (K_ARR.saturating_mul(self.last_gap_ns))
                .clamp(self.t_spacing_ns(), self.cadence_floor_ns())
        } else {
            ACK_CADENCE_FLOOR_NS
        };
        if elapsed < cadence_eff {
            return None;
        }
        if cadence_eff < T_SPACING_NS {
            // Önsel rejiminde clamp alt sınırı T_SPACING'dir; altı yalnız A-2 rejiminde mümkün.
            self.e2a_diag.regime_timer_emits = self.e2a_diag.regime_timer_emits.saturating_add(1);
        }
        if cadence_eff < ACK_CADENCE_FLOOR_NS {
            self.diag_accel_emits += 1;
        }
        self.since_last_ack = 0;
        self.burn_quota(); // kota=0 burada — yalnız emit_total muhasebesi (birleşik-sayaç)
        self.record_ack_emit(now_ns);
        self.build_ack(now_ns)
    }

    /// [r3 §2] Birleşik emisyon-saati: son HERHANGİ SACK-emisyonu (freq/timer + tail).
    fn last_emit_any(&self) -> u64 {
        self.last_ack_ns.max(self.last_tail_ack_ns)
    }

    /// [r3 §1a] Yeni-epizot: kredi RESET (birikim yok) + muafiyet + sayaç.
    fn open_episode(&mut self) {
        self.quota = self.quota_r;
        self.episode_first_pending = true;
        self.diag_episode_count += 1;
    }

    /// [r3 §2] Kredi-düşümü (her emisyon-yolu) — K-4 SERT-INVARIANT kaynağı.
    fn burn_quota(&mut self) {
        self.episode_first_pending = false;
        self.diag_emit_total += 1;
        if self.quota > 0 {
            self.quota -= 1;
            self.diag_quota_burn += 1;
        }
    }

    /// [r3 §1a] Robust-baseline: 0→yok; <k→max (muhafazakâr); ≥k→median(son k).
    fn episode_baseline(&self) -> u64 {
        match self.gap_len {
            0 => 0,
            n if n < EPISODE_WINDOW => self.gap_window[..n].iter().copied().max().unwrap_or(0),
            _ => {
                let mut v = [0u64; 5];
                v[..EPISODE_WINDOW].copy_from_slice(&self.gap_window[..EPISODE_WINDOW]);
                let m = &mut v[..EPISODE_WINDOW];
                m.sort_unstable();
                m[EPISODE_WINDOW / 2]
            }
        }
    }

    /// [r3 §1a] Ring-yazım (son EPISODE_WINDOW gap; clamp'lenmiş değer).
    fn push_gap(&mut self, g: u64) {
        self.gap_window[self.gap_idx] = g;
        self.gap_idx = (self.gap_idx + 1) % EPISODE_WINDOW;
        if self.gap_len < EPISODE_WINDOW {
            self.gap_len += 1;
        }
    }

    /// [r3] Bench ölçüm-matrisi: R [CAL {4,8}] koşu-başı ayarı.
    pub fn set_quota_r(&mut self, r: u32) {
        self.quota_r = r;
        // Oturum-ilk epizot new()+ilk-varışta açılır; set çağrısı varıştan ÖNCE yapılır
        // (serve_loop peer-insert) — açılmış kotayı da hizala (deterministik).
        if self.quota > 0 {
            self.quota = r;
        }
    }

    /// [r3 §6] RTRAJ diag: (episode_count, quota_burn, accel_emits, emit_total).
    #[must_use]
    pub fn diag_return_supply(&self) -> (u64, u64, u64, u64) {
        (
            self.diag_episode_count,
            self.diag_quota_burn,
            self.diag_accel_emits,
            self.diag_emit_total,
        )
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
        // [SR-AUDIT-001 / P3] rx-set SINIRLI kayan pencere. Aksi hâlde ayrık-seq akıtan bir eş
        // `received`'i ömür-boyu şişirir (BTreeSet + her SACK'te O(n log n) collect+sort). SACK
        // KÜMÜLATİF olduğundan `largest`'ın çok altındaki seq'ler dedup için gereksiz: en-eski seq
        // O(log n) `pop_first` ile düşer (kap sabit). Pencere-dışı çok-geç bir dup "yeni" görünüp
        // yeniden teslim edilebilir (E2E envelope §4 nihai dedup; client-core delivered-window da
        // kapsar) — bir u16-wrap'lık pencere her gerçek reorder/inflight ufkunu fazlasıyla aşar.
        while self.received.len() > RX_DEDUP_WINDOW {
            self.received.pop_first();
            if let Some(&first) = self.received.first() {
                self.smallest_seq = first;
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
            self.burn_quota(); // [r3 §2] freq/force emisyonları da kredi-muhasebesine girer
            self.record_ack_emit(now_ns);
            self.build_ack(now_ns)
        } else {
            None
        }
    }

    /// [K2/R1/§12] SACK emit muhasebesi — timer cadence gate'i beslemek için son emit anını
    /// günceller. Her freq/force/timer emisyonu buradan geçer (tail kendi-saatinde — r3 §2
    /// birleşik-saat `last_emit_any` ikisinin max'ı). Eski `first_ack_sent` R1-muafiyeti
    /// oturum-ilk-EPİZOT kotasıyla genellendi [TUR-5 r3].
    fn record_ack_emit(&mut self, now_ns: u64) {
        self.last_ack_ns = now_ns;
    }

    /// SACK ACK'i rx-set'ten kur (§2) — [`AckFrame::from_received`] gruplamayı yapar.
    ///
    /// [TUR-1x, Seçenek-A] ack_delay FLOOR-encoding: `encoded = min(floor(delay_us/256), 65535)`.
    /// Floor → receiver kendi delay'ini ŞİŞİRMEZ (decoded ≤ gerçek) → sender'ın path-RTT'si fazla
    /// KÜÇÜLMEZ. Satürasyon (hold > 16.78s) audit-sayacına yazılır — beklenen 0 (cadence 8.39s).
    fn build_ack(&mut self, now_ns: u64) -> Option<AckFrame> {
        let delay_us = now_ns
            .saturating_sub(self.largest_recv_ns)
            .checked_div(1000)
            .unwrap_or(0);
        let encoded = (delay_us / crate::ack::ACK_DELAY_UNIT_US).min(ACK_DELAY_ENC_MAX);
        if encoded == ACK_DELAY_ENC_MAX {
            self.ack_delay_saturated += 1;
        }
        let received: Vec<u64> = self.received.iter().copied().collect();
        // arrival_rate: receiver bu artımda BOYUT görmüyor (RxDatagram size taşımaz) → ölçüm YOK →
        // sentinel (§2.2 zeyli: 0xFFFF = "ölçüm yok/üst-bantta, ipucu olarak KULLANMA"). K3
        // tüketicisi bağlandığında gerçek pencere-oranı ölçümüyle değiştirilir.
        AckFrame::from_received(
            &received,
            self.ack_epoch,
            encoded as u16,
            self.seen_total,
            crate::ack::ARRIVAL_RATE_SENTINEL,
        )
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
    /// [TUR-1x] ack_delay satürasyon sayısı (encoded==65535; hold > 16.78s) — SALT-GÖZLEM audit.
    #[must_use]
    pub fn ack_delay_saturated(&self) -> u64 {
        self.ack_delay_saturated
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

    /// [P3 SR-AUDIT-001] Düşman ayrık-seq akışı rx-set'i sınırsız büyütemez: kap sabit.
    #[test]
    fn received_set_is_bounded_under_distinct_seq_flood() {
        let mut rx = Receiver::new();
        // Pencereyi rahatça aşan ayrık, artan seq besle (eviction devreye girsin).
        let n = RX_DEDUP_WINDOW as u64 + 2000;
        for i in 0..n {
            let _ = rx.on_datagram(
                RxDatagram {
                    seq: i,
                    unit_id: i,
                    kind: RxKind::Data,
                },
                i * 1000,
            );
        }
        assert!(
            rx.received_count() <= RX_DEDUP_WINDOW,
            "rx-set tavanı aşmamalı: {} > {RX_DEDUP_WINDOW}",
            rx.received_count()
        );
        // En yüksek seq hâlâ izleniyor (pencere en yeniyi tutar).
        assert_eq!(rx.largest_seq(), Some(n - 1));
    }

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
        // largest (seq0) 0'da alındı; ACK timer 5ms sonra → [TUR-1x] V2 floor-encoding:
        // floor(5000µs/256)=19 → decoded 19×256µs=4.864ms (floor: şişirme YOK, decoded ≤ gerçek).
        let ack = r.on_ack_timer(5 * MS).expect("ACK");
        assert_eq!(ack.ack_delay, 19, "encoded = floor(5000/256)");
        assert_eq!(
            ack.ack_delay_duration(),
            std::time::Duration::from_micros(19 * 256)
        );
        assert_eq!(r.ack_delay_saturated(), 0);
    }

    #[test]
    fn ack_delay_large_hold_not_saturated_and_saturation_counted() {
        // [TUR-1x] Cadence-hold 8.4s ESKİ µs-biriminde satüreydi (65.5ms tavan); V2 256µs-biriminde
        // gerçek değeriyle taşınır. Tavan 16.78s ÜSTÜ hold → satüre + audit-sayacı.
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0);
        let a0 = r.on_ack_timer(8_400 * MS).expect("ilk timer-SACK exempt");
        assert_eq!(a0.ack_delay, 32812, "floor(8.4e6µs/256) — satüre DEĞİL");
        assert_eq!(r.ack_delay_saturated(), 0);
        // 28.4s hold (> 16.78s tavan) → encoded 65535 + sayaç. (Cadence: 20s > floor → emit olur.)
        let a1 = r.on_ack_timer(28_400 * MS).expect("cadence doldu");
        assert_eq!(a1.ack_delay, u16::MAX, "tavan üstü → satüre");
        assert_eq!(r.ack_delay_saturated(), 1, "satürasyon audit'e yazılır");
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
    fn tail_pending_data_gets_cadence_exempt_sack() {
        // [TUR-3b commit-C, karar-a] TAIL-muafiyeti: freq'e ulaşmamış beklemede-veri +
        // sessizlik ≥ K_TAIL×son-aralık → timer-SACK cadence-MUAF (formüllü; ms-sabiti YOK).
        let mut r = Receiver::new();
        r.on_datagram(data(0, 100), 0);
        r.on_datagram(data(1, 101), 10 * MS); // freq=2 → SACK (cadence sıfırlanır); aralık=10ms
                                              // Tek-kalan (tail) datagram: freq'e ulaşmaz.
        r.on_datagram(data(2, 102), 20 * MS);
        // Sessizlik < 2×aralık(20ms): henüz tail-değil → cadence bastırır.
        assert!(
            r.on_ack_timer(30 * MS).is_none(),
            "sessizlik < K_TAIL×aralık → muafiyet yok (cadence bastırır)"
        );
        // Sessizlik ≥ 2×10ms: TAIL → cadence-muaf SACK (eski davranış: 8.39s bekletirdi).
        let a = r
            .on_ack_timer(41 * MS)
            .expect("tail-muafiyeti: beklemedeki son-paket SACK'i ms'ler-içinde");
        assert_eq!(a.largest_seq, 2);
        // [rev2] Tail-tekrarı KENDİ ritmiyle sınırlı (spam yok): hemen ardından tick → None.
        assert!(
            r.on_ack_timer(45 * MS).is_none(),
            "tail-ritmi dolmadan tekrar-tail yok (spam-koruması)"
        );
        // [rev2] Ritim dolunca beklemede-veri hâlâ varsa tekrar tail-SACK (cadence'e dokunulmadı).
        assert!(
            r.on_ack_timer(65 * MS).is_some(),
            "beklemede-veri sürerken tail kendi ritmiyle tekrarlar"
        );
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
                esi: 0,
                k_actual: 4,
                is_repair: false,
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

    // DR-S5-2: repair-shard da buffer+ACK, teslim-YOK (is_repair ayrımı + INV-FEC-1 korunur).
    #[test]
    fn repair_shard_buffered_not_delivered() {
        let mut r = Receiver::new();
        let shard = RxDatagram {
            seq: 1,
            unit_id: 200,
            kind: RxKind::FecShard {
                block_id: 7,
                esi: 0,
                k_actual: 4,
                is_repair: true, // repair
            },
        };
        let o = r.on_datagram(shard, 0);
        assert!(
            o.delivered.is_empty(),
            "repair-shard teslim-YOK (INV-FEC-1)"
        );
        assert_eq!(r.state(), RxState::FecDecode);
        // N8: shard-admission ACK-üretir (SACK) ama BtlBw-credit-YOLU-AÇMAZ (kredi sender.bw.rs'te, gerçek-wire-ACK).
        assert_eq!(r.received_count(), 1);
    }
}

// ── [TUR-5 r3] return-supply sözleşme-vektörleri (V-1..V-5 + count=0 + yakınsama + invariant) ──
#[cfg(test)]
mod return_supply_tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn data(seq: u64, unit_id: u64) -> RxDatagram {
        RxDatagram {
            seq,
            unit_id,
            kind: RxKind::Data,
        }
    }

    /// Gap-dizisini varışlara çevirip epizot-sayısını ölçer (oturum-ilk epizot DAHİL değil —
    /// vektörler "kota YENİLEMEMELİ" der: oturum-ilk-sonrası ek-epizot = 0 assert'lenir).
    fn run_gaps(r: &mut Receiver, gaps_ns: &[u64]) -> u64 {
        let mut t = 0u64;
        r.on_datagram(data(0, 0), t);
        let (ep0, _, _, _) = r.diag_return_supply();
        for (i, g) in gaps_ns.iter().enumerate() {
            t += g;
            r.on_datagram(data(1 + i as u64, 1 + i as u64), t);
        }
        let (ep, _, _, _) = r.diag_return_supply();
        ep - ep0
    }

    #[test]
    fn v1_burst_then_normal_gap_no_episode() {
        // V-1 `20,20,60ms` — K=4: 60>80 F; (K=3 tam-sınır kolu ölçüm-CAL'ında).
        let mut r = Receiver::new();
        assert_eq!(
            run_gaps(&mut r, &[20 * MS, 20 * MS, 60 * MS]),
            0,
            "V-1 kota yenilememeli"
        );
    }

    #[test]
    fn v2_micro_gap_batch_no_episode() {
        // V-2 `25,1,25,25ms` — clamp'li-yazım mikro-gap'i tabana çeker; robust-baseline F.
        let mut r = Receiver::new();
        assert_eq!(
            run_gaps(&mut r, &[25 * MS, MS, 25 * MS, 25 * MS]),
            0,
            "V-2 sahte-epizot yok"
        );
    }

    #[test]
    fn v3_bimodal_jitter_no_episode() {
        // V-3 `25,50,25,50,25ms` — bimodal jitter; medyan iki modu tolere eder.
        let mut r = Receiver::new();
        assert_eq!(
            run_gaps(&mut r, &[25 * MS, 50 * MS, 25 * MS, 50 * MS, 25 * MS]),
            0,
            "V-3 sahte-epizot yok"
        );
    }

    #[test]
    fn v4_slow_channel_variance_no_episode() {
        // V-4 `800ms,1.7s,800ms` — K_TAIL=2 oranını kıran vektör; K_EPISODE=4: 1.7s < 3.2s F.
        let mut r = Receiver::new();
        assert_eq!(
            run_gaps(&mut r, &[800 * MS, 1700 * MS, 800 * MS]),
            0,
            "V-4 sahte-epizot yok"
        );
    }

    #[test]
    fn v5_double_micro_gap_integer_boundary() {
        // V-5 `25,25,1,1,25,25,25ms` — K=4 tamsayı-tam-bölünme: 25/4=6.25ms EXACT →
        // eşik 4×6.25=25.0 → 25>25 F → sahte-epizot SIFIR (r3 §1a.1 pinli-semantik).
        let mut r = Receiver::new();
        assert_eq!(
            run_gaps(
                &mut r,
                &[25 * MS, 25 * MS, MS, MS, 25 * MS, 25 * MS, 25 * MS]
            ),
            0,
            "V-5 K=4: tam-sınır F, sıfır sahte-epizot"
        );
    }

    #[test]
    fn dv2_blackout_opens_episode_and_quota() {
        // DV-2 sınıfı: kararlı 0.8s ritim → 8s kopma → dönüş: epizot + kota-muafiyeti.
        let mut r = Receiver::new();
        let mut t = 0u64;
        r.on_datagram(data(0, 0), t);
        for i in 1..=3u64 {
            t += 800 * MS;
            r.on_datagram(data(i, i), t);
        }
        let (ep_before, _, _, _) = r.diag_return_supply();
        t += 8_000 * MS; // 8s kopma > K_EPISODE×0.8s=3.2s
        r.on_datagram(data(4, 4), t);
        let (ep_after, _, _, _) = r.diag_return_supply();
        assert_eq!(ep_after - ep_before, 1, "kopma-dönüşü epizot açar");
        // Muafiyet: ilk timer-SACK gecikmesiz (cadence beklemez).
        let ack = r.on_ack_timer(t + MS);
        assert!(ack.is_some(), "epizot-ilk emisyon cadence-muaf");
    }

    #[test]
    fn quota_spacing_and_invariant() {
        // Oturum-ilk epizot: muaf-ilk + T_SPACING-takvimli kopyalar; kredi-invariantı.
        let mut r = Receiver::new();
        r.on_datagram(data(0, 0), 0);
        let mut emits = 0;
        let mut t = 20 * MS;
        while t < 20_000 * MS {
            if r.on_ack_timer(t).is_some() {
                emits += 1;
            }
            t += 20 * MS;
        }
        // R=4: muaf@20ms + 3 kopya @ ~T_SPACING(2.1s)-aralıklı = 4 kota-emisyonu;
        // sonrası FLOOR-cadence (8.39s) → 20s içinde ~1-2 ek.
        let (ep, burn, _, total) = r.diag_return_supply();
        assert_eq!(ep, 1, "yalnız oturum-ilk epizot");
        assert_eq!(burn, 4, "kredi tam R kez yandı");
        assert!(
            burn <= 4 * ep,
            "K-4 SERT-INVARIANT: quota_burn ≤ R×episode_count"
        );
        assert!(
            (5..=6).contains(&emits),
            "4 kota + 1-2 kafes-emisyonu, ölçülen: {emits}"
        );
        assert_eq!(total as usize, emits, "emit_total tutarlı");
    }

    #[test]
    fn count0_hole_first_packet_blackout_return() {
        // count=0-deliği (beyanlı, DV-2 açık-riski): ilk-paket→kopma→dönüş — dönüşe epizot YOK
        // (baseline yok, sınıflandırılamaz) ve oturum-kotası kopmada yanmış.
        let mut r = Receiver::new();
        r.on_datagram(data(0, 0), 0);
        let mut t = 0;
        while t < 8_000 * MS {
            t += 20 * MS;
            let _ = r.on_ack_timer(t); // kota kopma-sessizliğine yanar (Ekleme-2 geri-çekildi)
        }
        let (ep0, burn0, _, _) = r.diag_return_supply();
        assert_eq!(ep0, 1);
        assert_eq!(burn0, 4, "kota kopmaya yandı (beyanlı)");
        r.on_datagram(data(1, 1), t); // dönüş-varışı: count=0 → epizot-kontrol atlanır
        let (ep1, _, _, _) = r.diag_return_supply();
        assert_eq!(
            ep1, 1,
            "count=0-deliği: dönüşe epizot verilmez (beyanlı-sınır)"
        );
    }

    #[test]
    fn regime_shift_geometric_convergence() {
        // Rejim-geçişi (25ms → 800ms kalıcı): clamp'li-yazım geometrik yakınsar;
        // epizot-sayısı ≤ ⌈log_K(oran)⌉+1 = ⌈log4(32)⌉+1 = 4 (livelock YOK).
        let mut r = Receiver::new();
        let mut t = 0u64;
        r.on_datagram(data(0, 0), t);
        for i in 1..=3u64 {
            t += 25 * MS;
            r.on_datagram(data(i, i), t);
        }
        let (ep_before, _, _, _) = r.diag_return_supply();
        for i in 4..=20u64 {
            t += 800 * MS;
            r.on_datagram(data(i, i), t);
        }
        let (ep_after, _, _, _) = r.diag_return_supply();
        let transitions = ep_after - ep_before;
        assert!(
            (1..=4).contains(&transitions),
            "geçiş-epizotları sınırlı-geometrik (livelock yok), ölçülen: {transitions}"
        );
    }

    #[test]
    fn arrival_rate_acceleration_mid_regime() {
        // Bacak-2 ayrık-niş vektörü: K_ARR=2'de accel-ritmi freq(2-varış)/tail(K_TAIL×gap)
        // ritimleriyle örtüşür (yapısal-gölgeleme — beklenen); accel YALNIZ
        // "sessizlik < K_TAIL×gap (tail-F) ∧ emit-elapsed ≥ K_ARR×gap" penceresinde ayrışır.
        // Kota izolasyonu: set_quota_r(0) (kota-kolu devre-dışı; ölçüm-matrisi dışı test-modu).
        let mut r = Receiver::new();
        r.set_quota_r(0);
        r.on_datagram(data(0, 0), 0);
        r.on_datagram(data(1, 1), 2_000 * MS); // freq-emit @2s (paylaşık emisyon-saati)
        r.on_datagram(data(2, 2), 4_000 * MS); // since=1
                                               // t=7.9s: sessizlik=3.9s < 4s (tail F); emit-elapsed=5.9s ≥ cadence_eff=4s < FLOOR.
        let ack = r.on_ack_timer(7_900 * MS);
        assert!(ack.is_some(), "accel-penceresinde emisyon");
        assert_eq!(r.diag_return_supply().2, 1, "hızlanmış-cadence sayacı");
    }
}

// ── [Q1] QUINN-FARKI §3-A alıcı kolları: A-1 gap_ack · A-2 return_prior_gated ──
// Her kol iki kollu (kapalı = bugünkü çıktı, açık = yeni çıktı), sahte saatli, deterministik.
#[cfg(test)]
mod q1_tests {
    use super::*;
    use crate::loss::{E2aFlags, E2A_ACK_CADENCE_MIN_NS};

    const MS: u64 = 1_000_000;

    fn data(seq: u64) -> RxDatagram {
        RxDatagram {
            seq,
            unit_id: 1000 + seq,
            kind: RxKind::Data,
        }
    }

    fn rx(f: E2aFlags) -> Receiver {
        let mut r = Receiver::new();
        r.set_e2a_flags(f);
        r
    }

    fn gap_on() -> E2aFlags {
        E2aFlags::new(false, MS, false, false, 1, true).expect("geçerli")
    }

    fn gated() -> E2aFlags {
        E2aFlags::default().with_return_prior_gated(true)
    }

    /// Varış: bayt ölçümü + motor (runtime sırası).
    fn arrive(r: &mut Receiver, seq: u64, bytes: u64, t: u64) -> RxOutput {
        r.observe_arrival_bytes(bytes, t);
        r.on_datagram(data(seq), t)
    }

    // ── A-1 (m5): boşlukta / geç varışta anında ACK ──

    #[test]
    fn a1_gap_ack_immediate_on_new_gap_and_late_arrival() {
        // 0,1 sıralı (frekans ACK'i) · 3 (yeni boşluk: 2 eksik) · 2 (geç varış) · 4 (sıralı).
        let seqs = [
            (0, 0),
            (1, 10 * MS),
            (3, 20 * MS),
            (2, 30 * MS),
            (4, 40 * MS),
        ];
        let run = |r: &mut Receiver| -> Vec<bool> {
            seqs.iter()
                .map(|&(s, t)| r.on_datagram(data(s), t).ack.is_some())
                .collect()
        };
        let mut off = Receiver::new();
        let mut on = rx(gap_on());
        // Kapalı: yalnız frekans (her 2 datagram).
        assert_eq!(run(&mut off), vec![false, true, false, true, false]);
        // Açık: 3 (yeni boşluk) ve 2 (geç varış) gecikmesiz; 4 sıralı → frekansı bekler.
        assert_eq!(run(&mut on), vec![false, true, true, true, false]);
        assert_eq!(off.diag_e2a(), RxE2aDiag::default());
        // 3'teki ACK frekans sayacını sıfırladığından 2'deki de frekansa ulaşmaz → ikisi de ek.
        assert_eq!(on.diag_e2a().gap_ack_emits, 2);
    }

    #[test]
    fn a1_gap_ack_reports_the_gap() {
        let mut on = rx(gap_on());
        on.on_datagram(data(0), 0);
        on.on_datagram(data(1), 10 * MS);
        let a = on
            .on_datagram(data(3), 20 * MS)
            .ack
            .expect("boşlukta anında ACK");
        assert_eq!((a.largest_seq, a.block0_len), (3, 1));
        assert_eq!(a.ranges, vec![crate::ack::AckRange { gap: 2, len: 2 }]);
    }

    #[test]
    fn a1_in_order_stream_is_identical_in_both_arms() {
        // Sıra dışı yokken kol yol üstünde değil: bütün ACK ve timer çıktıları birebir aynı.
        let run = |f: E2aFlags| {
            let mut r = rx(f);
            let mut out = Vec::new();
            for i in 0..40u64 {
                out.push(r.on_datagram(data(i), i * 37 * MS).ack.map(|a| a.encode()));
                out.push(r.on_ack_timer(i * 37 * MS + 20 * MS).map(|a| a.encode()));
            }
            for k in 0..600u64 {
                out.push(
                    r.on_ack_timer(40 * 37 * MS + k * 20 * MS)
                        .map(|a| a.encode()),
                );
            }
            (out, r.diag_return_supply())
        };
        assert_eq!(run(E2aFlags::default()), run(gap_on()));
    }

    #[test]
    fn a1_first_datagram_not_classified_and_duplicate_unchanged() {
        let mut on = rx(gap_on());
        assert!(
            on.on_datagram(data(5), 0).ack.is_none(),
            "ilk datagram sınıflandırılmaz (taban yok)"
        );
        let d = on.on_datagram(data(5), MS);
        assert!(
            d.was_duplicate && d.ack.is_some(),
            "kopya teyidi bugünkü gibi"
        );
        assert_eq!(on.diag_e2a().gap_ack_emits, 0);
    }

    // ── A-2: 31 B/s önseli rejim kapılı ──

    #[test]
    fn a2_prior_formulas_reproduce_shipped_constants() {
        assert_eq!(cadence_floor_for(RETURN_BW_PRIOR), ACK_CADENCE_FLOOR_NS);
        assert_eq!(t_spacing_for(RETURN_BW_PRIOR), T_SPACING_NS);
        assert_eq!(ACK_CADENCE_FLOOR_NS, 8_387_096_774);
        assert_eq!(T_SPACING_NS, 2_096_774_193);
        let r = Receiver::new();
        assert_eq!(r.e2a_flags(), E2aFlags::default());
        assert_eq!(
            (r.cadence_floor_ns(), r.t_spacing_ns()),
            (ACK_CADENCE_FLOOR_NS, T_SPACING_NS)
        );
    }

    #[test]
    fn a2_arrival_rate_ring() {
        let mut r = Receiver::new();
        assert_eq!(r.arrival_rate_bps(), 0, "örnek yok");
        r.observe_arrival_bytes(1200, 0);
        assert_eq!(r.arrival_rate_bps(), 0, "tek örnek");
        for k in 1..=20u64 {
            r.observe_arrival_bytes(1200, k * 100 * MS);
        }
        // Son 8 varış: 7 × 1200 B / 0,7 s = 12 000 B/s (yığılmaya duyarsız oran, EWMA değil).
        assert_eq!(r.arrival_rate_bps(), 12_000);
        // Aynı anda ikinci varış (yığılma). Halka 1,4…2,0 s + 2,0 s: 7 × 1200 B / 0,6 s =
        // 14 000 B/s — sınırlı artış (aralık-oranı EWMA'sında sıfır aralık örneği sonsuza giderdi).
        r.observe_arrival_bytes(1200, 2_000 * MS);
        assert_eq!(r.arrival_rate_bps(), 14_000);
    }

    /// 100 ms aralıklı 1200 B varış (12 kB/s), 0..=7 ve 800 ms'de bekleyen 8; 20 ms tik.
    /// Dönüş: 8'den sonraki ilk ACK anı.
    fn first_ack_after_pending(f: E2aFlags, bytes: u64, gap: u64) -> u64 {
        let mut r = rx(f);
        for i in 0..8u64 {
            arrive(&mut r, i, bytes, i * gap);
        }
        assert!(arrive(&mut r, 8, bytes, 8 * gap).ack.is_none());
        let mut t = 8 * gap;
        loop {
            t += 20 * MS;
            if r.on_ack_timer(t).is_some() {
                return t;
            }
            assert!(t < 60_000 * MS, "ACK hiç gelmedi");
        }
    }

    #[test]
    fn a2_in_regime_pending_cadence_is_25ms_class() {
        // Kapalı: bekleyen tek datagramın ACK'i tail muafiyetiyle K_TAIL × 100 ms = 200 ms sonra.
        assert_eq!(
            first_ack_after_pending(E2aFlags::default(), 1200, 100 * MS),
            1_000 * MS
        );
        // Açık (12 kB/s ≥ 1 kB/s): cadence = clamp(2×100 ms, 25 ms, 25 ms) = 25 ms; son emisyon
        // 700 ms'de → 800 ms'deki varıştan sonraki ilk tik.
        assert_eq!(first_ack_after_pending(gated(), 1200, 100 * MS), 820 * MS);
        let mut r = rx(gated());
        for i in 0..8u64 {
            arrive(&mut r, i, 1200, i * 100 * MS);
        }
        assert_eq!(
            r.t_spacing_ns(),
            E2A_ACK_CADENCE_MIN_NS,
            "max(25 ms, 65 B/12 kB/s)"
        );
        assert_eq!(r.cadence_floor_ns(), E2A_ACK_CADENCE_MIN_NS);
    }

    #[test]
    fn a2_below_regime_is_identical_to_off() {
        // PTT benzeri 80 B / 200 ms ≈ 400 B/s < 1 kB/s → önsel; çıktı dizisi kapalı kolla aynı.
        let run = |f: E2aFlags| {
            let mut r = rx(f);
            let mut out = Vec::new();
            for i in 0..30u64 {
                out.push(arrive(&mut r, i, 80, i * 200 * MS).ack.map(|a| a.encode()));
                for k in 1..10u64 {
                    out.push(
                        r.on_ack_timer(i * 200 * MS + k * 20 * MS)
                            .map(|a| a.encode()),
                    );
                }
            }
            for k in 0..1000u64 {
                out.push(r.on_ack_timer(6_000 * MS + k * 20 * MS).map(|a| a.encode()));
            }
            (out, r.diag_return_supply(), r.diag_e2a())
        };
        let off = run(E2aFlags::default());
        assert_eq!(run(gated()), off);
        assert_eq!(off.2, RxE2aDiag::default());
    }

    #[test]
    fn a2_off_ignores_measurements_entirely() {
        // Kapalı kol: yüksek varış debisi ve ipucu verilse bile sabitler (bit-aynı).
        let mut r = Receiver::new();
        r.set_return_bw_hint(12_500);
        for i in 0..8u64 {
            arrive(&mut r, i, 1200, i * 100 * MS);
        }
        assert_eq!(
            (r.cadence_floor_ns(), r.t_spacing_ns()),
            (ACK_CADENCE_FLOOR_NS, T_SPACING_NS)
        );
    }

    #[test]
    fn a2_hint_replaces_symmetric_proxy_in_regime() {
        let mut r = rx(gated());
        for i in 0..8u64 {
            arrive(&mut r, i, 1200, i * 100 * MS);
        }
        r.set_return_bw_hint(350);
        // Ölçülen dönüş debisi 350 B/s: t_spacing = 65 B / 350 B/s ≈ 185,7 ms > 25 ms.
        assert_eq!(r.t_spacing_ns(), t_spacing_for(350));
        assert_eq!(t_spacing_for(350), 185_714_285);
        assert_eq!(r.cadence_floor_ns(), cadence_floor_for(350));
    }

    #[test]
    fn a2_idle_repeat_floor_stays_on_prior_in_regime() {
        // Rejimde bile yeni bilgi yokken tekrar SACK aralığı ≥ 8,39 s (25 ms'lik tekrar seli yok).
        let mut r = rx(gated());
        for i in 0..8u64 {
            arrive(&mut r, i, 1200, i * 100 * MS);
        }
        let mut emits = Vec::new();
        let mut t = 700 * MS;
        while t < 60_000 * MS {
            t += 20 * MS;
            if r.on_ack_timer(t).is_some() {
                emits.push(t);
            }
        }
        // Frekans ACK'leri kotayı (R = 4) zaten yaktı; geriye yalnız boşta tekrar kalır.
        assert!(!emits.is_empty());
        for w in emits.windows(2) {
            assert!(
                w[1] - w[0] >= ACK_CADENCE_FLOOR_NS,
                "boşta tekrar önselde: {w:?}"
            );
        }
        assert_eq!(
            r.diag_e2a().regime_timer_emits,
            0,
            "boşta kol rejim cadence'i değil"
        );
    }
}
