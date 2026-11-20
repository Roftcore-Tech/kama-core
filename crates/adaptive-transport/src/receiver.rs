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

use std::collections::{BTreeMap, BTreeSet};

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
/// [DÖNÜŞ-BÜTÇESİ] ACK tel boyu aralık başına artışı (B): `AckFrame::encode` her aralık için gap + len
/// (2 × u16) yazar. Tahmin = `SACK_WIRE_BYTES` (0 aralık) + 4 × aralık.
const ACK_WIRE_PER_RANGE: u64 = 4;
/// [DÖNÜŞ-BÜTÇESİ] Tipik ACK tel boyu (B): 4 aralık (ölçüldü: %20 kayıpta ortalama ≈ 80–86 B).
const ACK_WIRE_TYPICAL_BYTES: u64 = SACK_WIRE_BYTES + 4 * ACK_WIRE_PER_RANGE;

/// [DÖNÜŞ-BÜTÇESİ] Dönüş yolu `return_bw_bps` (B/s) iken bütçeli alıcının tipik ACK aralığı (ns):
/// tipik ACK tel boyu / (Y · return_bw). Gönderici ilk penceresini bununla kurar (ACK'ler bu aralıkla
/// geleceği için bir aralıkta gönderebileceği veri); alıcı ile gönderici AYNI formülü kullanır
/// (deterministik, ADR-17). `0` → `u64::MAX`.
#[must_use]
pub fn budgeted_ack_interval_ns(return_bw_bps: u64) -> u64 {
    let budget = return_bw_bps.saturating_mul(RETURN_BUDGET_PERMIL) / 1000;
    if budget == 0 {
        return u64::MAX;
    }
    u64::try_from(u128::from(ACK_WIRE_TYPICAL_BYTES) * 1_000_000_000 / u128::from(budget))
        .unwrap_or(u64::MAX)
}

/// [GERİ-BİLDİRİM-UFKU] Dönüş yolu `return_bw_bps` (B/s) iken bütçeli alıcının en büyük ACK'inin
/// (MAX_ACK_RANGES aralıklı) göndericiye ulaşma ufku (ns): bütçe aralığı (en büyük ACK / Y·dönüş)
/// ile o ACK'in dönüş hattında serileşmesinin toplamı. Gönderici, geri bildirimi aç profilde uçuş
/// tabanını bu ufukla kurar: bir birimin teyidi en geç bu ufuk ve yol RTT'si kadar sonra gelir.
/// `0` → `u64::MAX`.
#[must_use]
pub fn feedback_horizon_ns(return_bw_bps: u64) -> u64 {
    let budget = return_bw_bps.saturating_mul(RETURN_BUDGET_PERMIL) / 1000;
    if budget == 0 {
        return u64::MAX;
    }
    let largest = u128::from(ACK_BUDGET_RESERVE_BYTES) * 1_000_000_000;
    let ns = largest / u128::from(budget) + largest / u128::from(return_bw_bps);
    u64::try_from(ns).unwrap_or(u64::MAX)
}

/// [DÖNÜŞ-BÜTÇESİ] Jeton kovası tavanı (B): en büyük iki ACK (MAX_ACK_RANGES aralıklı). Kova dolu
/// başlar → oturumun İLK ACK'i bütçeden muaf (§12 [R1]).
const ACK_BUDGET_CAP_BYTES: u64 =
    2 * (SACK_WIRE_BYTES + ACK_WIRE_PER_RANGE * crate::ack::MAX_ACK_RANGES as u64);
/// [AKIŞ-SONU YEDEĞİ] Varışlar sürerken kovada bırakılan yedek (B): en büyük bir ACK (kovanın yarısı).
const ACK_BUDGET_RESERVE_BYTES: u64 = ACK_BUDGET_CAP_BYTES / 2;
/// [YENİDEN-RAPOR] Bu kadar çıkan ACK'te bildirilmiş blok seçimde önceliksizdir.
const REREPORT_SAFE: u8 = 2;
/// [K2/R1/R15/§3.3-E/§12] ACK_CADENCE cold-start floor = sack_wire/(Y·return_bw_önsel) ≈ 8.4s.
/// TIMER-SACK'ları bununla seyrekleşir: kbps'te sabit 20ms tick → SACK-seli/self-congestion (19s-lag);
/// throttle bunu keser. Frequency-yolu (§2.1, on_datagram) DATA-oranıyla eşleşir → hızlı kanalda responsive
/// kalır (timer yalnız tail backstop). İlk timer-SACK cadence-exempt ([R1]); return_bw öğrenilince FULL
/// adaptif hızlanma D2 sonrası (§3.3-E). Sabit const → ADR-17 determinizm (RNG yok).
const ACK_CADENCE_FLOOR_NS: u64 =
    SACK_WIRE_BYTES * 1_000_000_000 * 1000 / (RETURN_BUDGET_PERMIL * RETURN_BW_PRIOR);
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
const T_SPACING_NS: u64 = SACK_WIRE_BYTES * 1_000_000_000 / RETURN_BW_PRIOR;
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

/// Receiver state machine (§4). rx-set + SACK üretimi; `on_datagram` / `on_ack_timer` ile sürülür.
pub struct Receiver {
    /// [CC-PORT] quinn ACK kuralları (bkz. [`Receiver::with_cc_port`]). `false` = bugünkü alıcı.
    cc_port: bool,
    state: RxState,
    received: BTreeSet<u64>, // alınan full-counter seq'ler (SACK kaynağı)
    /// [YENİDEN-RAPOR] Geri bildirimi aç alıcıda alınan seq'in kaç çıkan ACK'te bildirildiği
    /// (`received` ile aynı kayan pencere; başka yolda boş).
    reported: BTreeMap<u64, u8>,
    smallest_seq: u64,
    largest_seq: u64,
    have_recv: bool,
    largest_recv_ns: u64, // ack_delay için (largest_seq alım anı)
    ack_epoch: u8,
    since_last_ack: u32,
    ack_freq: u32,
    /// [CC-PORT] zamanlayıcı ACK gecikme tavanı (ns); kurucu `MAX_ACK_DELAY_NS`.
    max_ack_delay_ns: u64,
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
    // [3b-C] HERHANGİ datagram'ın son varışı (dup dahil — kanal-tempo kanıtı). [AKIŞ-SONU YEDEĞİ]
    // Bütçeli port alıcısında akış sessizken gelen teslim edilmiş birim kopyası hariç
    // (`redundant_copy_after_flight`).
    last_arrival_ns: u64,
    last_gap_ns: u64, // [3b-C] son varış-aralığı (serialization-vekili; 0 = henüz tek varış)
    last_tail_ack_ns: u64, // [3b-C rev2] tail-SACK'in KENDİ ritim-sayacı (cadence'ten bağımsız)
    /// [DÖNÜŞ-BÜTÇESİ] ACK bayt bütçesi (B/s) = Y × dönüş bant genişliği (§16.1 RETURN_BUDGET_FRACTION).
    /// `None` = bütçe yok (quinn kuralı aynen). Yalnız `cc_port` alıcısında okunur.
    ack_budget_bps: Option<u64>,
    /// [DÖNÜŞ-BÜTÇESİ] Kovadaki jeton (milibayt) ve son dolum anı (ns).
    ack_tokens_mb: u64,
    ack_tokens_at_ns: u64,
    /// [AKIŞ-SONU YEDEĞİ] Bütçeli yolda ilk ACK çıktı mı (ilk ACK yedekten muaf).
    ack_budget_first_done: bool,
    /// [YEDEK-SÜRESİ] Son bütçeli ACK'in anı (ns).
    ack_budget_last_ns: u64,
    /// [DÖNÜŞ-BÜTÇESİ] Bütçe yetmediği için ertelenen ACK sayısı (SALT-GÖZLEM).
    diag_ack_budget_deferred: u64,
    /// [DÖNÜŞ-BÜTÇESİ] `note_non_eliciting` ile kaydedilen en büyük PN (sıra denetimi; `None` =
    /// hiç çağrılmadı).
    non_eliciting_largest: Option<u64>,
    /// [AKIŞ-SONU YEDEĞİ] DATA akışı sürerken gelen son ACK istemeyen varışın anı
    /// (`note_non_eliciting`; 0 = hiç). Sessizlik denetimi onu eşin etkinliği sayar.
    non_eliciting_at_ns: u64,
    /// [AKIŞ-SONU YEDEĞİ] Bütçeli port alıcısında teslim edilen DATA birim kimlikleri (kayan pencere
    /// `RX_DEDUP_WINDOW`; başka yolda boş kalır). Yalnız `redundant_copy_after_flight` okur; teslim
    /// tekilleştirmesi bu katmanda değildir.
    delivered_units: BTreeSet<u64>,
    /// [BİLGİSİZ-ACK] Dönüş yolunun bant genişliği (B/s; `set_return_bandwidth`; `None` = bütçe yok).
    return_bw_bps: Option<u64>,
    /// [BİLGİSİZ-ACK] Son ACK'in göndericiye en erken varabileceği an (ns): emisyon + tel boyu /
    /// dönüş hızı (yayılım gecikmesi bilinmez, eklenmez; alt sınır). 0 = henüz ACK yok.
    ack_horizon_ns: u64,
    /// [DÖNÜŞ-HATTI] Dönüş hattında şu an bu alıcının ACK'inin önünde serileşmeyi bekleyen süre
    /// (ns): aynı cihazın oturum uçlarının verisi, PARAM'ları ve bu alıcının önceki çerçeveleri
    /// (`set_return_line_ahead_ns`). 0 = bilinmiyor / boş hat (bugünkü varsayım).
    return_line_ahead_ns: u64,
    /// [DÖNÜŞ-HATTI · ÖNCELİK] Cihazın dar yukarı yönünde aynı cihazın verisi (ör. alındı) hattın
    /// boşalmasını bekliyor (`set_uplink_yield`). Bütçeli ACK bu sürece çıkmaz: verinin önüne en
    /// çok hattaki o anki datagram girer. `false` = bugünkü yol.
    uplink_yield: bool,
    /// [DÖNÜŞ-HATTI · ÖNCELİK] Veriye yol verdiği için o an çıkmayan ACK denemesi (SALT-GÖZLEM).
    diag_ack_yield_deferred: u64,
    /// [BİLGİSİZ-ACK] Son ACK'ten beri bilgi taşıyan bir varış oldu mu (`no_info_copy` olmayan her
    /// varış). Yalnız bütçeli port alıcısında okunur.
    info_since_ack: bool,
    /// [BİLGİSİZ-ACK] Bilgi taşımadığı için çıkarılmayan ACK denemesi (SALT-GÖZLEM).
    diag_ack_no_info_deferred: u64,
}

impl Receiver {
    /// Yeni receiver (epoch 0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            cc_port: false,
            state: RxState::Normal,
            received: BTreeSet::new(),
            reported: BTreeMap::new(),
            smallest_seq: 0,
            largest_seq: 0,
            have_recv: false,
            largest_recv_ns: 0,
            ack_epoch: 0,
            since_last_ack: 0,
            ack_freq: ACK_FREQ_N,
            max_ack_delay_ns: Self::MAX_ACK_DELAY_NS,
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
            ack_budget_bps: None,
            ack_tokens_mb: 0,
            ack_tokens_at_ns: 0,
            ack_budget_first_done: false,
            ack_budget_last_ns: 0,
            diag_ack_budget_deferred: 0,
            non_eliciting_largest: None,
            non_eliciting_at_ns: 0,
            delivered_units: BTreeSet::new(),
            return_bw_bps: None,
            ack_horizon_ns: 0,
            return_line_ahead_ns: 0,
            uplink_yield: false,
            diag_ack_yield_deferred: 0,
            info_since_ack: false,
            diag_ack_no_info_deferred: 0,
        }
    }

    /// §4 on_datagram: VERIFIED datagram'ı işle → teslim + belki ACK.
    pub fn on_datagram(&mut self, d: RxDatagram, now_ns: u64) -> RxOutput {
        // [§2.2 V2] seen_count: HER görülen datagram sayılır (dup/shard DAHİL) — sender'ın
        // "seen vs sent" yön-ayrımlı kayıp kıyası için (K3 cold-start).
        self.seen_total = self.seen_total.saturating_add(1);
        // [AKIŞ-SONU YEDEĞİ] Akış sessizken gelen teslim edilmiş birim kopyası varış ritmine ve
        // sessizlik saatine girmez (`redundant_copy_after_flight`); kayıt ve ACK'i aşağıda aynen.
        let redundant = self.redundant_copy_after_flight(&d, now_ns);
        // [BİLGİSİZ-ACK] Son ACK'in göndericiye varamayacağı kadar erken gelen kopya ACK'i kendi başına
        // gerektirmez (`no_info_copy`); diğer her varış bilgi taşır.
        if !(redundant && self.no_info_copy(now_ns)) {
            self.info_since_ack = true;
        }
        // [AKIŞ-SONU YEDEĞİ] Bütçeli port alıcısında eş sessizken gelen varışın boşluğu sessizliği
        // kapsar, ritmi değil: varış aralığı penceresine yazılmaz (`gap_spans_silence`).
        let spans_silence = self.gap_spans_silence(now_ns);
        // [TUR-5 r3 §1a] EPİZOT-DEĞERLENDİRME — pencere/last_gap GÜNCELLENMEDEN ÖNCE (kopma-gap'i
        // ritim-referansını zehirleyemez). Oturum-ilk = açık özel-durum. Dup dahil her datagram.
        if self.seen_total == 1 {
            self.open_episode();
        } else if !redundant {
            let gap = now_ns.saturating_sub(self.last_arrival_ns);
            let baseline = self.episode_baseline();
            if baseline > 0 && gap > K_EPISODE.saturating_mul(baseline) {
                self.open_episode();
            }
            // SİMETRİK CLAMP'Lİ-YAZIM (r3-doğrulama): zehirlenme-koruması + livelock-ölümü +
            // mikro-gap tabanı tek kuralda; count=0'da (ilk gap) ham — sınıflandırılamaz (beyanlı).
            if !spans_silence {
                let written = if baseline > 0 {
                    gap.clamp(baseline / K_EPISODE, baseline.saturating_mul(K_EPISODE))
                } else {
                    gap
                };
                self.push_gap(written);
            }
        }
        // [3b-C] kanal-tempo ölçümü: varış-aralığı HAM kalır (tail/bacak-2 girdisi — anlık ritim;
        // epizot-penceresi AYRI iş, r3 §3).
        if !redundant {
            if self.seen_total > 1 {
                self.last_gap_ns = now_ns.saturating_sub(self.last_arrival_ns);
            }
            self.last_arrival_ns = now_ns;
        }
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

        // [CC-PORT] Sırasız varış tespiti kayıttan ÖNCE: beklenen = largest+1 değilse (boşluk açıldı
        // ya da boşluk doldu) anında ACK (RFC 9000 §13.2.1; quinn spaces.rs:718-725 `immediate_ack_pending`).
        // [DÖNÜŞ-BÜTÇESİ] Kaydedilmiş ACK istemeyen PN'ler de sıraya sayılır (yalnız
        // `note_non_eliciting` çağrıldıysa; yoksa ifade bugünküyle aynı).
        let in_order_after = self
            .non_eliciting_largest
            .map_or(self.largest_seq, |n| n.max(self.largest_seq));
        let out_of_order =
            self.cc_port && self.have_recv && d.seq != in_order_after.wrapping_add(1);

        self.record_seq(d.seq, now_ns);

        let (delivered, is_shard) = match d.kind {
            RxKind::Data => {
                self.note_delivered_unit(d.unit_id);
                (vec![d.unit_id], false)
            }
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

        let ack = self.tick_ack(now_ns, out_of_order);
        RxOutput {
            delivered,
            ack,
            was_duplicate: false,
        }
    }

    /// [CC-PORT] quinn `max_ack_delay` (RFC 9000 §13.2.1 varsayılanı 25 ms): bekleyen yeni bilgi
    /// varken zamanlayıcı SACK'i en geç bu kadar sonra çıkar; cadence tabanı (8,39 s) ve dönüş-kotası
    /// uygulanmaz. Yalnız `cc_port` alıcısında.
    const MAX_ACK_DELAY_NS: u64 = 25_000_000;

    /// [CC-PORT] quinn ACK kurallarıyla alıcı: sırasız varışta anında ACK, zamanlayıcı
    /// `max_ack_delay` 25 ms; cadence tabanı (8,39 s), tail muafiyeti ve dönüş kotası uygulanmaz.
    /// `Receiver::new()` bugünkü alıcıdır (bit-eşit).
    #[must_use]
    pub fn with_cc_port() -> Self {
        let mut r = Self::new();
        r.cc_port = true;
        r
    }

    /// [DÖNÜŞ-BÜTÇESİ] Dönüş yolunun (bu alıcının ACK'lerini taşıyan yön) bant genişliği (B/s).
    /// Spec §2.1/§12/§16.1: "SACK + yankı dönüşü ≤ Y · return_bw"; dönüş kıtken ACK seyrekleşir.
    /// Port alıcısı quinn kuralıyla ACK'ler (her `ACK_FREQ_N` DATA'da bir + her sırasız varışta
    /// anında); bütçe verilince her ACK emisyonu jeton kovasından (`Y · return_bw` B/s dolum, tavan
    /// iki ACK) geçer, jeton yetmezse ACK ERTELENİR — SACK kümülatif olduğundan bilgi kaybolmaz,
    /// zamanlayıcı (`on_ack_timer`) jeton dolunca birleşik ACK'i çıkarır. `None` = bütçe yok
    /// (bugünkü davranış). Yalnız `cc_port` alıcısında etkilidir.
    pub fn set_return_bandwidth(&mut self, return_bw_bps: Option<u64>) {
        self.return_bw_bps = return_bw_bps;
        self.ack_budget_bps =
            return_bw_bps.map(|bw| (bw.saturating_mul(RETURN_BUDGET_PERMIL) / 1000).max(1));
        self.ack_tokens_mb = ACK_BUDGET_CAP_BYTES * 1000;
        self.ack_tokens_at_ns = 0;
    }

    /// [DÖNÜŞ-BÜTÇESİ] Bütçe yetmediği için ertelenen ACK emisyonu (SALT-GÖZLEM).
    #[must_use]
    pub fn diag_ack_budget_deferred(&self) -> u64 {
        self.diag_ack_budget_deferred
    }

    /// [DÖNÜŞ-BÜTÇESİ] ACK istemeyen (non-ack-eliciting) bir datagramın PN'ini kaydeder: alıcı
    /// motoruna birim taşımayan kontrol çerçeveleri (PARAM, cold-start probu). RFC 9000 §13.2.1:
    /// alınan her paket ACK aralıklarına girer, yalnız ack-eliciting olanlar ACK tetikler.
    /// Kaydedilmeyen PN kümede kalıcı DELİK olur: sonraki her ACK bir aralık fazla taşır (+4 B;
    /// bütçeli yolda her ACK'in bekleme aralığı uzar) ve ardından gelen DATA sırasız sayılıp anında
    /// ACK'e zorlanır (bütçe jetonunu akış ortasında harcar). Bu çağrı ACK üretmez, teslim etmez,
    /// ACK sayacına girmez; `largest_seq` / `ack_delay` tabanı (en büyük ack-eliciting PN'in
    /// varışı) değişmez — göndericinin RTT örneği o PN'den alınır. Sıra denetimi (anında ACK
    /// kuralı) için ayrı en büyük (`non_eliciting_largest`) tutulur. Gönderici bu PN'leri birim
    /// kaydına eşlemediği için teyitleri yok sayılır. Runtime bunu yalnız dönüş bütçesi kurulu
    /// alıcıda çağırır (bütçesiz yol bayt-aynı).
    ///
    /// [AKIŞ-SONU YEDEĞİ] Varış anı (`now_ns`), DATA akışı sürerken (son DATA varışının sessizlik
    /// penceresi içinde) geldiyse sessizlik denetimine (`arrivals_quiet`) eşin etkinliği olarak
    /// girer; akış sessizleştikten sonra gelen kontrol trafiği akış sonu ACK'ini bekletmez. Epizot
    /// ve varış aralığı ölçümüne girmez.
    pub fn note_non_eliciting(&mut self, seq: u64, now_ns: u64) {
        if !self.data_quiet(now_ns) {
            self.non_eliciting_at_ns = self.non_eliciting_at_ns.max(now_ns);
        }
        if self.received.contains(&seq) {
            return;
        }
        self.received.insert(seq);
        if self.have_recv {
            self.smallest_seq = self.smallest_seq.min(seq);
        }
        self.non_eliciting_largest = Some(self.non_eliciting_largest.map_or(seq, |n| n.max(seq)));
        // [SR-AUDIT-001] `record_seq` ile aynı kayan pencere.
        while self.received.len() > RX_DEDUP_WINDOW {
            self.received.pop_first();
            if let Some(&first) = self.received.first() {
                self.smallest_seq = first;
                // [YENİDEN-RAPOR] Pencereden düşen seq'in sayacı da unutulur.
                while self
                    .reported
                    .first_key_value()
                    .is_some_and(|(&k, _)| k < first)
                {
                    self.reported.pop_first();
                }
            }
        }
    }

    /// [DÖNÜŞ-BÜTÇESİ] Kovayı `now`'a dek doldurur; `frame`'in tel maliyeti kadar jeton varsa düşer.
    ///
    /// [AKIŞ-SONU YEDEĞİ] Bütçe bağlayıcıyken (bir varış aralığında dolan jeton bir tam ACK'e
    /// yetmiyor) ve varışlar sürerken ACK, ardından kovada bir tam ACK'lik yedek
    /// (`ACK_BUDGET_RESERVE_BYTES`) kalacaksa çıkar. Yedek, varışlar durunca (`arrivals_quiet`)
    /// çıkan ACK'e aittir: o ACK akışın bütün kayıplarını bildirir. Yedeksiz kural jetonu akış ortasında tek birimlik haber taşıyan ACK'lere harcıyor,
    /// akışın kayıp raporunu bir bütçe aralığı (62 B/s dönüşte 5–7 s) bekletiyordu
    /// (tests/shaped_relay.rs asimetrik hücre, `CC_SHAPED_TL`). Oturumun ilk ACK'i yedekten muaf
    /// (§12 [R1]). Bütçe bağlayıcı değilken (simetrik hızlı yol) kural bugünküyle aynı.
    fn ack_budget_take(&mut self, now_ns: u64, frame: &AckFrame) -> bool {
        let Some(rate) = self.ack_budget_bps else {
            return true;
        };
        let cap = ACK_BUDGET_CAP_BYTES * 1000;
        let dt = now_ns.saturating_sub(self.ack_tokens_at_ns);
        self.ack_tokens_at_ns = now_ns;
        // B/s × ns → milibayt: rate · dt / 1e6.
        let add = u64::try_from(u128::from(rate) * u128::from(dt) / 1_000_000).unwrap_or(u64::MAX);
        self.ack_tokens_mb = self.ack_tokens_mb.saturating_add(add).min(cap);
        if self.uplink_yield {
            // Jeton harcanmaz; ACK, hat veriye geçince zamanlayıcıyla çıkar.
            self.diag_ack_yield_deferred += 1;
            return false;
        }
        let cost = (SACK_WIRE_BYTES + ACK_WIRE_PER_RANGE * frame.ranges.len() as u64) * 1000;
        let mut need = cost;
        if self.ack_budget_first_done && !self.arrivals_quiet(now_ns) {
            let gap = self.episode_baseline();
            let refill_mb =
                u64::try_from(u128::from(rate) * u128::from(gap) / 1_000_000).unwrap_or(u64::MAX);
            // [YEDEK-SÜRESİ · 2026-11-20] Yedek, son ACK'ten bu yana bu çerçevenin bütçe süresi
            // (tel maliyeti / bütçe hızı) geçmediyse istenir. Kova tavanı iki tam ACK'tir: tam boy
            // (16 aralık, 129 B) bir akış ortası ACK yedekle birlikte BÜTÜN kovayı ister ve boş
            // kovadan 62 B/s dönüşte 2 × 129 / 15,5 ≈ 16,6 s bekler. Ölçüldü (ürün testi, tohum
            // 1): iki erken ACK kovayı boşalttıktan sonra 16,5 s ACK boşluğu, dosya kayıp
            // raporsuz 21 s'de bitti; gerçek 6 250 B/s hücrede 8–10 s boşluklar. Bütçe süresi
            // dolduktan sonra ACK jetonla çıkar (Y bütçesi aşılmaz: çerçeve kendi jetonunu öder);
            // daha sık ACK'lerde yedek kuralı bugünkü gibi.
            let budget_ns = u64::try_from(u128::from(cost) * 1_000_000 / u128::from(rate.max(1)))
                .unwrap_or(u64::MAX);
            if refill_mb < ACK_BUDGET_RESERVE_BYTES * 1000
                && now_ns.saturating_sub(self.ack_budget_last_ns) < budget_ns
            {
                need += ACK_BUDGET_RESERVE_BYTES * 1000;
            }
        }
        if self.ack_tokens_mb >= need {
            self.ack_tokens_mb -= cost;
            self.ack_budget_last_ns = now_ns;
            self.ack_budget_first_done = true;
            true
        } else {
            self.diag_ack_budget_deferred += 1;
            false
        }
    }

    /// [AKIŞ-SONU YEDEĞİ] Varışlar durdu mu: son varıştan beri > `K_EPISODE` × varış aralığı tabanı
    /// (`episode_baseline`, son aralıkların kırpılmış medyanı — uzun bir boşluk tabanı şişirmez).
    /// Eşik alıcının epizot kopma tanımıdır (`on_datagram`). `K_TAIL` (2) kullanılamaz: tek kayıp
    /// paketin boşluğu zaten 2 × aralık, ardışık iki kayıp 3 × — %20 kayıpta akış ortası "sessizlik"
    /// yedeği harcatıyordu (ölçüm: 2,18 s ve 3,26 s'de akış ortası ACK). Taban yokken (≤ 1 varış)
    /// sessiz sayılır.
    ///
    /// Sessizlik EŞİN sessizliğidir: ACK istemeyen varış (PARAM, cold-start probu;
    /// `note_non_eliciting`) da saati tazeler. Yalnız DATA sayılınca, DATA kayıpları arasına düşen
    /// bir PARAM yeniden denemesi eş gönderirken boşluğu eşiğin üstüne taşıyordu: üç DATA kaybı
    /// 4 aralık, PARAM'ın serileşmesi de ≈ 9 ms ekliyor; 20 ms'lik ACK tikine göre bazı koşularda
    /// akış ortası ACK yedeği harcıyor, akışın kayıp raporu bir bütçe aralığı (≈ 4,4 s) gecikiyordu
    /// (tests/shaped_relay.rs asimetrik hücre, tohum 2: koşuların bir kısmında 10,7 → 15,1 s).
    ///
    /// Akış sessizleştikten sonra gelen ACK istemeyen paket saati tazelemez (`note_non_eliciting`):
    /// akış bittikten sonra sessizlik penceresinden sık süren kontrol trafiği (PARAM yenilemesi,
    /// prob) alıcıyı hiç "sessiz" bırakmaz, akış sonu ACK'i kovanın maliyet + yedeğe dolmasını
    /// bekler (62 B/s dönüşte saniyeler). Böylece kontrol trafiği akış sonu ACK'ini en çok bir
    /// sessizlik penceresi geciktirir.
    fn arrivals_quiet(&self, now_ns: u64) -> bool {
        let base = self.episode_baseline();
        let last = self.last_arrival_ns.max(self.non_eliciting_at_ns);
        base == 0 || now_ns.saturating_sub(last) > base.saturating_mul(K_EPISODE)
    }

    /// [AKIŞ-SONU YEDEĞİ] Yalnız DATA'ya göre sessizlik: son DATA varışından beri > `K_EPISODE` ×
    /// varış aralığı tabanı (taban yokken sessiz).
    fn data_quiet(&self, now_ns: u64) -> bool {
        let base = self.episode_baseline();
        base == 0 || now_ns.saturating_sub(self.last_arrival_ns) > base.saturating_mul(K_EPISODE)
    }

    /// [AKIŞ-SONU YEDEĞİ] Bütçeli port alıcısında, eş sessizken (`arrivals_quiet`) gelen ve bu
    /// alıcının çoktan teslim ettiği birimi yeni PN'le taşıyan DATA mı (göndericinin PTO probu ya
    /// da gereksiz yeniden iletimi). Böyle bir kopya akışı yeniden başlatmaz: son varış anına ve
    /// varış aralığı penceresine yazılmaz. PN'i kümeye girer ve ACK sayacını artırır (gönderici
    /// teyidini alır); akış sonu ACK'i sessiz akışın kuralıyla, yedek istemeden çıkar.
    ///
    /// Kopya akış sayılınca (`dc4365dd`) akış sonu ACK'i saniyelerce bekliyordu: (1) ACK yeniden
    /// yedek istiyor, jeton yetecekken sessizliği bekliyordu; (2) kopyanın uzun boşluğu 4 × taban
    /// olarak pencereye yazılıyor, önceki turun boşluğuyla birlikte tabanı 4 katına, sessizlik
    /// penceresini 84 ms'lik ritimde 336 ms'den 1,34 s'ye çıkarıyordu; (3) bu pencere boyunca gelen
    /// kontrol paketleri (`note_non_eliciting`) saati tazeliyordu (tests/shaped_relay.rs asimetrik
    /// hücre, tohum 2, akış sonrası 250 ms kontrol trafiği: son ACK 17,16 s; kopyasız koşu 14,76 s).
    ///
    /// Akış sürerken gelen kopya (eş gönderiyor) ve yeni birim taşıyan varış bugünkü gibi akıştır.
    /// Teslim edilmiş birim bu alıcının `delivered` çıktısıdır; FEC ile kurtarılan birimi alıcı
    /// görmez, onun kopyası akış sayılır (bugünkü davranış).
    fn redundant_copy_after_flight(&self, d: &RxDatagram, now_ns: u64) -> bool {
        self.reserve_path()
            && matches!(d.kind, RxKind::Data)
            && !self.received.contains(&d.seq)
            && self.delivered_units.contains(&d.unit_id)
            && self.arrivals_quiet(now_ns)
    }

    /// [AKIŞ-SONU YEDEĞİ] Bütçeli port alıcısında, eş sessizken (`arrivals_quiet`, taban varken)
    /// gelen varışın boşluğu sessizliği kapsar: varış aralığı penceresine yazılmaz. Varış akıştır
    /// (son varış anı tazelenir, yedek kuralı ve epizot sayımı aynen); yalnız taban akışın ritminde
    /// kalır. Akış ortasındaki boşluk (eş sessiz değil; araya PARAM düşen DATA kayıpları dahil) ve
    /// ritimde gelen varış eskisi gibi yazılır.
    ///
    /// Yazılınca (`9b2ccee7`) akış sonrası tek PTO probu (yeni birim taşıyan) boşluğunu 4 × taban
    /// olarak pencereye koyuyordu; önceki turun başındaki boşlukla pencerenin medyanı 4 katına,
    /// sessizlik penceresi 84 ms'lik ritimde 336 ms'den 1,34 s'ye çıkıyordu. ACK probdan 1,35–1,46 s,
    /// akış sonrası kontrol trafiğiyle 2,7 s sonra çıkıyordu. Gönderici kuyruktaki yeni birimi PTO
    /// probuyla taşıdığında (şerit kipi) bu her durakta tekrarlar ve her prob tabanı bir kez daha 4
    /// katına çıkarırdı.
    ///
    /// Bedel: ritim bir adımda 4 kattan fazla yavaşlarsa taban ona uyarlanmaz; her varış sessizlikten
    /// sonra gelmiş sayılır ve ACK'i bir pencere sonra yedek istemeden çıkar (seyrek varışta ACK
    /// beklemez). Eski motor ve bütçesiz yol eski uyarlamayı korur.
    fn gap_spans_silence(&self, now_ns: u64) -> bool {
        self.reserve_path() && self.episode_baseline() > 0 && self.arrivals_quiet(now_ns)
    }

    /// [AKIŞ-SONU YEDEĞİ] Akış sonu yedeği kurallarının geçerli olduğu yol: dönüş bütçesi kurulu port
    /// alıcısı. Başka yolda bu kurallar çalışmaz (eski motor ve bütçesiz yol bayt-aynı).
    fn reserve_path(&self) -> bool {
        self.cc_port && self.ack_budget_bps.is_some()
    }

    /// [YENİDEN-RAPOR] Geri bildirimi aç alıcı: bütçeli port alıcısı ve tipik ACK'in bütçe aralığı
    /// en az 1 s (bütçe ≤ tipik ACK tel boyu B/s; `runtime::link_feedback_starved` ile aynı eşik).
    fn feedback_starved_rx(&self) -> bool {
        self.cc_port
            && self
                .ack_budget_bps
                .is_some_and(|b| b <= ACK_WIRE_TYPICAL_BYTES)
    }

    /// [YENİDEN-RAPOR · 2026-11-21] Kırpılacak çerçevede bildirilecek seq'ler. Tel düzeni aynıdır
    /// (en büyük blok + en çok [`crate::ack::MAX_ACK_RANGES`] azalan aralık); değişen, alt blokların
    /// SEÇİMİDİR. Bugünkü kural en yeni 16 alt bloğu yazar: 62 B/s dönüşte ACK ≈ 8,6 s'de bir çıkar
    /// ve ileri yönde aynı sürede 35–75 boşluk oluşur, eski bloklar HİÇ bildirilmez. Gönderici
    /// bildirilmeyen iletimi paket eşiğiyle kayıp sayar ve yeniden yollar; yeniden iletim ACK'in
    /// hemen ardından çıktığından bir sonraki ACK'in kapsamının da altında kalır (ürün testi,
    /// düşüş hücresi tohum 1: PTT demeti 85'in dört iletim turu 33 s teyitsiz; PTT şeridinin teyit
    /// sınırı ilerlemedi, şerit açıklığı (256) dolunca yeni demet motora verilmedi).
    ///
    /// Kural: en büyük blok (largest, `ack_delay` ve göndericinin cephesi onundur) her zaman; kalan
    /// yerler önce hiç bildirilmemiş bloklara — yarısı en yeniden (yeni kayıpların haberi), yarısı
    /// en eskiden (bekleyenin yaşı sınırlı kalsın) —, sonra en az bildirilmiş bloklara, en yeniden.
    /// Bir blok en az [`REREPORT_SAFE`] çıkan ACK'te bildirildiyse önceliksizdir (güvenle unutulur:
    /// dönüş kaybına karşı iki kopya; pencere dışına düşen seq'in sayacı da düşer). Göndericinin
    /// muhasebesi teyitli KÜME üzerindendir (`acked_full`); bildirilmeyen seq "teyitsiz"dir, aradaki
    /// aralık ona kayıp demez. Seçim yalnız blok sayısı tavanı aşınca yapılır; aşmıyorsa çerçeve
    /// bugünküyle aynıdır.
    fn rereport_selection(&self) -> Vec<u64> {
        let all: Vec<u64> = self.received.iter().copied().collect();
        let mut blocks: Vec<(u64, u64)> = Vec::new();
        for &x in &all {
            match blocks.last_mut() {
                Some(b) if b.1 + 1 == x => b.1 = x,
                _ => blocks.push((x, x)),
            }
        }
        let slots = crate::ack::MAX_ACK_RANGES;
        if blocks.len() <= slots + 1 {
            return all;
        }
        let top = blocks.pop().expect("blok var");
        // Bloğun en az bildirilmiş seq'inin sayacı (sayacı olmayan seq hiç bildirilmemiştir).
        let count = |b: &(u64, u64)| -> u8 {
            let (k, m) = self
                .reported
                .range(b.0..=b.1)
                .fold((0u64, u8::MAX), |(k, m), (_, &c)| (k + 1, m.min(c)));
            if k < b.1 - b.0 + 1 {
                0
            } else {
                m
            }
        };
        // (öncelik sınıfı, blok indisi): sınıf 0 = hiç bildirilmemiş seq içerir.
        let fresh: Vec<usize> = (0..blocks.len())
            .filter(|&i| count(&blocks[i]) == 0)
            .collect();
        let mut pick: Vec<usize> = Vec::with_capacity(slots);
        let newest = slots / 2;
        for &i in fresh.iter().rev().take(newest) {
            pick.push(i);
        }
        for &i in &fresh {
            if pick.len() >= slots {
                break;
            }
            if !pick.contains(&i) {
                pick.push(i);
            }
        }
        if pick.len() < slots {
            let mut rest: Vec<usize> = (0..blocks.len()).filter(|i| !pick.contains(i)).collect();
            rest.sort_by_key(|&i| (count(&blocks[i]).min(REREPORT_SAFE), std::cmp::Reverse(i)));
            for i in rest {
                if pick.len() >= slots {
                    break;
                }
                pick.push(i);
            }
        }
        let mut out: Vec<u64> = Vec::new();
        for i in pick {
            let (lo, hi) = blocks[i];
            out.extend(lo..=hi);
        }
        out.extend(top.0..=top.1);
        out
    }

    /// [YENİDEN-RAPOR] Çıkan ACK'in bildirdiği seq'lerin sayacını artırır (yalnız geri bildirimi
    /// aç alıcıda).
    fn note_reported(&mut self, frame: &AckFrame) {
        if !self.feedback_starved_rx() {
            return;
        }
        for seq in frame.acked_full(self.largest_seq) {
            if self.received.contains(&seq) {
                let c = self.reported.entry(seq).or_insert(0);
                *c = c.saturating_add(1);
            }
        }
    }

    /// [BİLGİSİZ-ACK] Bütçeli port alıcısında, akış sessizken gelen teslim edilmiş birim kopyası
    /// (`redundant_copy_after_flight`) son ACK'in göndericiye en erken varış anından
    /// (`ack_horizon_ns`) ÖNCE geldiyse ACK'i kendi başına gerektirmez. Böyle bir kopya o ACK'e
    /// verilmiş tepki olamaz: gönderici onu ACK'i görmeden yolladı (ölçülen tohum 3 koşusu: birim 1'in
    /// eski bir ACK'le kayıp ilan edilen yeniden iletimi, ACK çıkmadan 0,14 s önce yollanmış). Yeni
    /// birim bilgisi taşımaz; ACK'i 62 B/s dönüşte ≈ 6 s'lik jetonu, bilgi taşıyan sonraki ACK'in
    /// (sonraki turun akış sonu raporu) önünde harcıyordu.
    ///
    /// Canlılık: gönderici kopyanın birimini önceki ACK'le teyit eder; teyitli birimin uçuştaki
    /// kopyası prob hedefi olmaz ve ACK'siz de uçuştan bırakılır (sender `oldest_probe_target`,
    /// `release_confirmed_copies`). Önceki ACK yolda kaybolduysa birim teyitsiz kalır; göndericinin
    /// bir sonraki probu ufuktan sonra gelir ve ACK'i bugünkü kuralla çıkar (en çok bir PTO turu).
    fn no_info_copy(&self, now_ns: u64) -> bool {
        self.reserve_path() && now_ns < self.ack_horizon_ns
    }

    /// [BİLGİSİZ-ACK] ACK'in göndericiye en erken varış anı: emisyon + hatta önündeki baytların
    /// serileşmesi ([`Self::set_return_line_ahead_ns`]) + tel boyu / dönüş hızı. Tel boyu kovanın
    /// kullandığı tahmin (`SACK_WIRE_BYTES` + aralık başına `ACK_WIRE_PER_RANGE`).
    ///
    /// [DÖNÜŞ-HATTI] Ufuk eskiden hattın ACK anında boş olduğunu varsayıyordu. Dar dönüşte aynı
    /// cihazın ters oturumu (PARAM, alındı) ve bu alıcının PARAM_ACK'i ACK'in önünde saniyelerce hat
    /// tutar (62 B/s: 85 B PARAM 1,37 s); o sürede gelen kopya ACK'e tepki olamaz.
    fn ack_arrival_horizon(&self, now_ns: u64, frame: &AckFrame) -> u64 {
        let Some(bw) = self.return_bw_bps.filter(|&b| b > 0) else {
            return 0;
        };
        let wire = SACK_WIRE_BYTES + ACK_WIRE_PER_RANGE * frame.ranges.len() as u64;
        let ser =
            u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(bw)).unwrap_or(u64::MAX);
        now_ns
            .saturating_add(self.return_line_ahead_ns)
            .saturating_add(ser)
    }

    /// [DÖNÜŞ-HATTI] Dönüş hattında şu an bu alıcının bir sonraki ACK'inin önünde serileşmeyi
    /// bekleyen süre (ns). Çalışma-zamanı cihazın yukarı yön hattını tutar ve ACK üretebilecek her
    /// çağrıdan önce verir; değer yalnız ACK'in varış ufkuna girer (bütçesiz yolda ufuk okunmaz).
    pub fn set_return_line_ahead_ns(&mut self, ahead_ns: u64) {
        self.return_line_ahead_ns = ahead_ns;
    }

    /// [DÖNÜŞ-HATTI · ÖNCELİK] Cihazın dar yukarı yönünde aynı cihazın verisi hattın boşalmasını
    /// bekliyor mu. Çalışma-zamanı ACK üretebilecek her çağrıdan önce verir; yalnız bütçeli yolda
    /// okunur (bütçesiz yolda ACK kararı değişmez).
    pub fn set_uplink_yield(&mut self, yield_to_data: bool) {
        self.uplink_yield = yield_to_data;
    }

    /// [DÖNÜŞ-HATTI · ÖNCELİK] Veriye yol verdiği için o an çıkmayan ACK denemesi (SALT-GÖZLEM).
    #[must_use]
    pub fn diag_ack_yield_deferred(&self) -> u64 {
        self.diag_ack_yield_deferred
    }

    /// [BİLGİSİZ-ACK] Bilgi taşımadığı için çıkarılmayan ACK denemesi (SALT-GÖZLEM).
    #[must_use]
    pub fn diag_ack_no_info_deferred(&self) -> u64 {
        self.diag_ack_no_info_deferred
    }

    /// [AKIŞ-SONU YEDEĞİ] Teslim edilen DATA biriminin kimliği (yalnız bütçeli port alıcısında;
    /// `RX_DEDUP_WINDOW` kayan pencere, en küçük kimlik düşer).
    fn note_delivered_unit(&mut self, unit_id: u64) {
        if !self.reserve_path() {
            return;
        }
        self.delivered_units.insert(unit_id);
        while self.delivered_units.len() > RX_DEDUP_WINDOW {
            self.delivered_units.pop_first();
        }
    }

    /// ACK emisyonu (frekans/zorunlu/zamanlayıcı yollarının ortak gövdesi). [DÖNÜŞ-BÜTÇESİ] Port
    /// alıcısında bütçe varken jeton yetmezse `None`: `since_last_ack` sıfırlanmaz, bekleyen bilgi
    /// zamanlayıcıyla yeniden denenir. Bütçe yokken bugünkü sıra aynen (sayaç → kota → saat → kur).
    fn emit_ack(&mut self, now_ns: u64) -> Option<AckFrame> {
        if self.cc_port && self.ack_budget_bps.is_some() {
            // [BİLGİSİZ-ACK] Son ACK'ten beri yalnız `no_info_copy` kopyalar geldiyse ACK çıkmaz, jeton
            // harcanmaz: kopyaların PN'leri bilgi taşıyan sonraki ACK'e katılır.
            if !self.info_since_ack {
                self.diag_ack_no_info_deferred += 1;
                return None;
            }
            let (frame, saturated) = self.ack_frame(now_ns);
            let frame = frame?;
            if !self.ack_budget_take(now_ns, &frame) {
                return None;
            }
            if saturated {
                self.ack_delay_saturated += 1;
            }
            self.since_last_ack = 0;
            self.info_since_ack = false;
            self.note_reported(&frame);
            self.ack_horizon_ns = self.ack_arrival_horizon(now_ns, &frame);
            self.burn_quota();
            self.record_ack_emit(now_ns);
            return Some(frame);
        }
        self.since_last_ack = 0;
        self.burn_quota();
        self.record_ack_emit(now_ns);
        self.build_ack(now_ns)
    }

    /// §12 ACK_TIMER: zamanlayıcı ile ACK üret (frekans dolmasa da). [K2/R1/§3.3-E/§12] cadence-limitli:
    /// İLK SACK henüz yollanmadıysa cadence-exempt (hemen — client PRE_CONVERGE'den hızlı çıksın); yollandıysa
    /// son SACK'ten `ACK_CADENCE_FLOOR_NS` geçmeden timer-SACK BASTIRILIR (kbps 20ms-sabit SACK-seli/19s-lag
    /// önlenir). Frequency-yolu (§2.1) DATA-oranıyla eşleştiğinden hızlı kanalda responsive kalır.
    pub fn on_ack_timer(&mut self, now_ns: u64) -> Option<AckFrame> {
        if !self.have_recv {
            return None;
        }
        if self.cc_port {
            // [CC-PORT] RFC 9000 §13.2.1: ack-eliciting paket bekliyorsa max_ack_delay içinde ACK.
            if self.since_last_ack > 0
                && now_ns.saturating_sub(self.last_emit_any()) >= self.max_ack_delay_ns
            {
                return self.emit_ack(now_ns);
            }
            return None;
        }
        self.on_ack_timer_legacy(now_ns)
    }

    /// Eski zamanlayıcı yolu — `cc_port` kapalıyken bugünkü davranış BİREBİR.
    fn on_ack_timer_legacy(&mut self, now_ns: u64) -> Option<AckFrame> {
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
            if self.episode_first_pending || elapsed >= T_SPACING_NS {
                self.since_last_ack = 0;
                self.burn_quota();
                self.record_ack_emit(now_ns);
                return self.build_ack(now_ns);
            }
            return None;
        }
        // [r3 §3] Bacak-2 §3.3-E MİNİMAL-TABAN (kota-dışı): beklemede-yeni-bilgi varken cadence
        // varış-ritmine bağlanır; yoksa statik-önsel FLOOR (sessiz-dönem tekrarları korunur).
        let cadence_eff = if self.since_last_ack > 0 && self.last_gap_ns > 0 {
            (K_ARR.saturating_mul(self.last_gap_ns)).clamp(T_SPACING_NS, ACK_CADENCE_FLOOR_NS)
        } else {
            ACK_CADENCE_FLOOR_NS
        };
        if elapsed < cadence_eff {
            return None;
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
                // [YENİDEN-RAPOR] Pencereden düşen seq'in sayacı da unutulur.
                while self
                    .reported
                    .first_key_value()
                    .is_some_and(|(&k, _)| k < first)
                {
                    self.reported.pop_first();
                }
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
            // [r3 §2] freq/force emisyonları da kredi-muhasebesine girer (`emit_ack`).
            self.emit_ack(now_ns)
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
        let (frame, saturated) = self.ack_frame(now_ns);
        if saturated {
            self.ack_delay_saturated += 1;
        }
        frame
    }

    /// ACK çerçevesini kurar (durum DEĞİŞMEZ); ikinci değer `ack_delay` doygunluğu (audit sayacı
    /// yalnız ACK gerçekten çıkınca sayılır — bütçeyle ertelenen çerçeve saymaz).
    fn ack_frame(&self, now_ns: u64) -> (Option<AckFrame>, bool) {
        let delay_us = now_ns
            .saturating_sub(self.largest_recv_ns)
            .checked_div(1000)
            .unwrap_or(0);
        let encoded = (delay_us / crate::ack::ACK_DELAY_UNIT_US).min(ACK_DELAY_ENC_MAX);
        let received: Vec<u64> = if self.feedback_starved_rx() {
            self.rereport_selection()
        } else {
            self.received.iter().copied().collect()
        };
        // arrival_rate: receiver bu artımda BOYUT görmüyor (RxDatagram size taşımaz) → ölçüm YOK →
        // sentinel (§2.2 zeyli: 0xFFFF = "ölçüm yok/üst-bantta, ipucu olarak KULLANMA"). K3
        // tüketicisi bağlandığında gerçek pencere-oranı ölçümüyle değiştirilir.
        let frame = AckFrame::from_received(
            &received,
            self.ack_epoch,
            encoded as u16,
            self.seen_total,
            crate::ack::ARRIVAL_RATE_SENTINEL,
        );
        (frame, encoded == ACK_DELAY_ENC_MAX)
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

    /// [GERİ-BİLDİRİM-UFKU] Ufuk = en büyük bütçeli ACK'in aralığı + dönüş hattında serileşmesi.
    #[test]
    fn feedback_horizon_counts_the_largest_budgeted_ack_and_its_serialization() {
        // 62 B/s: bütçe 15 B/s; en büyük ACK 129 B → 8,6 s aralık + 2,08 s serileşme.
        let h = feedback_horizon_ns(62);
        assert!(
            (10_600 * MS..=10_700 * MS).contains(&h),
            "ufuk {} ms",
            h / MS
        );
        assert!(
            feedback_horizon_ns(12_500) < 60 * MS,
            "hızlı dönüşte ufuk kısa"
        );
        assert_eq!(feedback_horizon_ns(0), u64::MAX);
    }

    /// [DÖNÜŞ-HATTI · ÖNCELİK] Yol verme açıkken bütçeli alıcı ACK çıkarmaz (jeton harcanmaz);
    /// kapanınca zamanlayıcı ertelenen bilginin tamamını tek ACK'te çıkarır. Yol vermesiz aynı
    /// akışta ilk ACK hemen çıkar.
    /// [YENİDEN-RAPOR] Geri bildirimi aç alıcıda blok sayısı tavanı aşınca her alınan blok, art arda
    /// çıkan ACK'lerde en az bir kez bildirilir; ilk çerçeve en büyük bloğu ve hiç bildirilmemiş eski
    /// blokları taşır. Tel düzeni aynıdır (en çok 16 alt aralık).
    #[test]
    fn starved_receiver_reports_every_block_across_acks() {
        const SEC: u64 = 1_000_000_000;
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        // 60 blok: her 3 seq'ten 2'si alınır (seq 3k+2 kayıp).
        let mut t = 0u64;
        for k in 0..60u64 {
            for j in 0..2u64 {
                let seq = 3 * k + j;
                let _ = r.on_datagram(data(seq, seq), t);
                t += 10 * MS;
            }
        }
        let mut covered: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
        let mut frames = 0;
        let mut now = t;
        while frames < 8 {
            now += 20 * SEC;
            if let Some(a) = r.on_ack_timer(now) {
                assert!(a.ranges.len() <= crate::ack::MAX_ACK_RANGES);
                let largest = r.largest_seq().expect("varış var");
                assert_eq!(u64::from(a.largest_seq), largest, "en büyük her zaman");
                covered.extend(a.acked_full(largest));
                frames += 1;
                // Bilgi yoksa ACK çıkmaz: her çerçeve öncesi yeni bir varış.
                let s = 3 * 60 + frames;
                let _ = r.on_datagram(data(s, s), now + MS);
            }
        }
        for k in 0..60u64 {
            assert!(covered.contains(&(3 * k)), "blok {k} hiç bildirilmedi");
        }
    }

    #[test]
    fn uplink_yield_holds_budgeted_acks_until_the_line_is_free() {
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        r.set_uplink_yield(true);
        for i in 0..10u64 {
            assert!(
                r.on_datagram(data(i, i), i * 100 * MS).ack.is_none(),
                "hat veriye borçluyken ACK yok ({i})"
            );
        }
        assert!(r.on_ack_timer(2_000 * MS).is_none());
        assert!(r.diag_ack_yield_deferred() > 0);
        r.set_uplink_yield(false);
        let a = r.on_ack_timer(2_020 * MS).expect("hat boşalınca ACK");
        assert_eq!(a.largest_seq, 9);
        assert_eq!(a.block0_len, 10, "bekleyen varışların tamamı");
        let mut q = Receiver::with_cc_port();
        q.set_return_bandwidth(Some(62));
        let mut first = None;
        for i in 0..10u64 {
            if q.on_datagram(data(i, i), i * 100 * MS).ack.is_some() {
                first = Some(i);
                break;
            }
        }
        assert!(first.is_some(), "yol vermesiz bütçeli alıcı ACK'ler");
    }

    /// [DÖNÜŞ-BÜTÇESİ] Bütçeli port alıcısı: ilk ACK muaf (kova dolu başlar), sonrakiler jeton
    /// yetmeyince ertelenir; zamanlayıcı jeton dolunca TEK birleşik ACK çıkarır ve o ACK ertelenen
    /// bütün varışları kapsar. Bütçe yokken aynı akış her iki DATA'da bir ACK'ler.
    #[test]
    fn return_budget_defers_and_coalesces_acks_on_port_receiver() {
        const SEC: u64 = 1_000_000_000;
        // Dönüş 62 B/s → bütçe Y · 62 = 15 B/s; 0 aralıklı ACK 65 B ≈ 4,3 s'de bir.
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        let mut acks = 0u32;
        // 40 DATA, 100 ms arayla (4 s): kova iki büyük ACK'lik doluyla başlar.
        for i in 0..40u64 {
            if r.on_datagram(data(i, i), i * 100 * MS).ack.is_some() {
                acks += 1;
            }
        }
        assert!(acks >= 1, "ilk ACK bütçeden muaf");
        assert!(
            acks <= 4,
            "4 s'de ≤ 4 ACK (kova 2 büyük ACK + 4 s × 15 B/s): {acks}"
        );
        assert!(r.diag_ack_budget_deferred() > 0, "bütçe ACK'leri erteledi");
        // Bütçesiz aynı akış: her 2 DATA'da bir → 20 ACK.
        let mut q = Receiver::with_cc_port();
        let mut q_acks = 0u32;
        for i in 0..40u64 {
            if q.on_datagram(data(i, i), i * 100 * MS).ack.is_some() {
                q_acks += 1;
            }
        }
        assert_eq!(q_acks, 20, "bütçesiz port alıcısı quinn kuralıyla ACK'ler");
        assert_eq!(q.diag_ack_budget_deferred(), 0);
        // Varışlar biter; zamanlayıcı jeton dolunca birleşik ACK'i çıkarır: en büyük seq 39'u ve
        // 0..=39 bloğunun tamamını kapsar.
        let mut t = 4 * SEC;
        let ack = loop {
            if let Some(a) = r.on_ack_timer(t) {
                break a;
            }
            t += 20 * MS;
            assert!(t < 20 * SEC, "zamanlayıcı bütçe dolunca ACK çıkarmalı");
        };
        assert_eq!(ack.largest_seq, 39);
        assert_eq!(ack.block0_len, 40, "ertelenen bütün varışlar tek ACK'te");
        assert!(
            r.on_ack_timer(t + 20 * MS).is_none(),
            "bekleyen bilgi yok → ACK yok"
        );
    }

    /// [AKIŞ-SONU YEDEĞİ] Bütçe bağlayıcıyken (62 B/s dönüş) akış ortası ACK'leri kovanın yedeğine
    /// dokunmaz — ardışık iki kaybın boşluğu (3 × aralık) da akış sonu sayılmaz; varışlar durunca
    /// çıkan ACK beklemeden gider ve akışın en büyük seq'ini kapsar. Yedeksiz kural jetonu akış
    /// ortasında bitiriyor, akış sonu ACK'i ~6 s bekliyordu; `K_TAIL` sessizlik eşiği çift kayıpta
    /// yedeği harcatıyordu.
    #[test]
    fn return_budget_keeps_reserve_for_the_end_of_flight_ack() {
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        // 40 DATA, 84 ms arayla (12,5 kB/s'de 1 kB); her onlukta ardışık iki kayıp (4–5, 14–15 …).
        let gap = 84 * MS;
        let lost = |i: u64| i % 10 == 4 || i % 10 == 5;
        let mut last_t = 0;
        let mut mid = 0u32;
        for i in 0..40u64 {
            let t = i * gap;
            // Zamanlayıcı da 20 ms'de bir koşar (runtime ACK_TICK).
            let mut k = last_t + 20 * MS;
            while k < t {
                if r.on_ack_timer(k).is_some() {
                    mid += 1;
                }
                k += 20 * MS;
            }
            if !lost(i) && r.on_datagram(data(i, i), t).ack.is_some() {
                mid += 1;
            }
            last_t = t;
        }
        assert!(mid >= 1, "oturumun ilk ACK'i muaf");
        assert!(mid <= 2, "akış ortasında yedeğe dokunulmaz: {mid} ACK");
        // Akış bitti (son varış 39 · 84 ms): sessizlik > 4 × 84 ms → ilk uygun zamanlayıcı turunda.
        let end = last_t;
        let mut t = last_t + 20 * MS;
        let ack = loop {
            if let Some(a) = r.on_ack_timer(t) {
                break a;
            }
            t += 20 * MS;
            assert!(t < end + 500 * MS, "akış sonu ACK'i yedekten hemen çıkmalı");
        };
        assert_eq!(ack.largest_seq, 39, "akışın son varışını kapsar");
        assert_eq!(ack.ranges.len(), 4, "akışın dört kayıp çiftini bildirir");
    }

    /// [AKIŞ-SONU YEDEĞİ] Bütçe bağlayıcı değilken (simetrik 12,5 kB/s dönüş, bütçe 3 125 B/s) yedek
    /// kuralı devreye girmez: ACK'ler bütçesiz port alıcısıyla aynı anlarda çıkar.
    #[test]
    fn return_budget_reserve_is_inert_when_the_budget_does_not_bind() {
        let mut a = Receiver::with_cc_port();
        let mut b = Receiver::with_cc_port();
        b.set_return_bandwidth(Some(12_500));
        for i in 0..200u64 {
            let t = i * 84 * MS;
            let seq = i + i / 7; // aralıklı delik: sırasız varışta anında ACK
            let x = a.on_datagram(data(seq, i), t).ack;
            let y = b.on_datagram(data(seq, i), t).ack;
            assert_eq!(x, y, "seq {seq}");
        }
        assert_eq!(b.diag_ack_budget_deferred(), 0);
    }

    /// [DÖNÜŞ-BÜTÇESİ] ACK istemeyen PN (PARAM) kaydedilince delik kapanır: ACK aralık taşımaz,
    /// ardından gelen DATA sırasız sayılmaz (anında ACK yok), `largest_seq` / `ack_delay` tabanı
    /// ack-eliciting PN'de kalır. Kaydedilmeyince bugünkü davranış: bir aralık + anında ACK.
    #[test]
    fn non_eliciting_pn_closes_the_hole_without_eliciting_an_ack() {
        let mut with = Receiver::with_cc_port();
        let mut without = Receiver::with_cc_port();
        for r in [&mut with, &mut without] {
            assert!(r.on_datagram(data(0, 0), 0).ack.is_none());
            assert!(
                r.on_datagram(data(1, 1), MS).ack.is_some(),
                "ACK_FREQ_N = 2"
            );
        }
        with.note_non_eliciting(2, 2 * MS); // PARAM
        assert!(
            with.on_ack_timer(2 * MS).is_none(),
            "ACK istemeyen PN ACK tetiklemez"
        );
        assert!(
            with.on_datagram(data(3, 2), 3 * MS).ack.is_none(),
            "PARAM'dan sonraki DATA sırada"
        );
        assert!(
            without.on_datagram(data(3, 2), 3 * MS).ack.is_some(),
            "kaydedilmeyen PN delik: anında ACK"
        );
        assert_eq!(with.largest_seq(), Some(3));
        let a = with
            .on_datagram(data(4, 3), 4 * MS)
            .ack
            .expect("frekans ACK'i");
        assert!(a.ranges.is_empty(), "delik yok: {:?}", a.ranges);
        assert_eq!(a.block0_len, 5);
        // En büyük PN ACK istemeyense: tel `largest_seq`'i odur, `largest_seq()` / ack_delay tabanı
        // son DATA'da kalır (göndericinin RTT örneği DATA'dan).
        with.note_non_eliciting(5, 5 * MS);
        assert_eq!(with.largest_seq(), Some(4));
        let t = with.on_datagram(data(6, 4), 10 * MS);
        assert!(t.ack.is_none(), "5 kayıtlı → 6 sırada");
        assert_eq!(with.largest_seq(), Some(6));
    }

    /// Akış ortası boşluk (tests/shaped_relay.rs asimetrik hücre, tohum 2'nin yavaş koşusu): DATA
    /// 84 ms arayla; son DATA'dan sonra iki DATA kaybı, PARAM yeniden denemesi (≈ 9 ms serileşir,
    /// 2 aralık + 9 ms'de varır), bir DATA kaybı; sonraki DATA 4 aralık + 9 ms'de. ACK tiki 20 ms'de
    /// bir, `phase` evresinde. Dönen: boşluk içinde çıkan ACK sayısı.
    fn mid_flight_gap_acks(note_param: bool, phase: u64) -> u32 {
        let slot = 84 * MS;
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        let t_last = 20 * slot;
        let t_next = t_last + 4 * slot + 9 * MS;
        // (an, PARAM mı, seq)
        let mut events: Vec<(u64, bool, u64)> = (0..=20u64).map(|i| (i * slot, false, i)).collect();
        if note_param {
            events.push((t_last + 2 * slot + 9 * MS, true, 23));
        }
        events.push((t_next, false, 25));
        events.sort_unstable();
        let mut tick = phase;
        let mut in_gap = 0;
        for (t, param, seq) in events {
            while tick < t {
                if r.on_ack_timer(tick).is_some() && tick > t_last {
                    in_gap += 1;
                }
                tick += 20 * MS;
            }
            if param {
                r.note_non_eliciting(seq, t);
            } else {
                let _ = r.on_datagram(data(seq, seq), t);
            }
        }
        in_gap
    }

    /// [AKIŞ-SONU YEDEĞİ] Eşin ACK istemeyen paketi sessizliği bozar: DATA kayıpları arasına düşen
    /// PARAM varken yedek akış ortasında harcanmaz (her tik evresinde). PARAM kaydedilmeyince aynı
    /// boşluk eşin gerçek sessizliğidir (345 ms > 4 × 84 ms): tikin eşikle sonraki DATA arasına düştüğü
    /// evrelerde ACK çıkar — eski kuralda yavaş koşunun 2,18 s'deki fazladan ACK'i buydu.
    #[test]
    fn non_eliciting_arrival_keeps_the_reserve_inside_the_flight() {
        for p in 0..20u64 {
            assert_eq!(
                mid_flight_gap_acks(true, p * MS),
                0,
                "PARAM varışı eşin etkinliği (evre {p} ms)"
            );
        }
        let fired = (0..20u64)
            .filter(|&p| mid_flight_gap_acks(false, p * MS) > 0)
            .count();
        assert!(
            (1..20).contains(&fired),
            "PARAM'sız boşlukta yalnız eşik sonrası evreler ACK çıkarır: {fired}/20"
        );
    }

    /// Akış (DATA 0..=20, 84 ms arayla, kayıpsız) bittikten sonra eşin ACK istemeyen kontrol
    /// paketleri `ctrl` aralıkla gelir (ilki son DATA'dan `ctrl` sonra; PARAM yeniden denemesi,
    /// prob). ACK tiki 20 ms. Dönen: akış sonu ACK'inin son DATA'dan gecikmesi.
    fn end_of_flight_ack_delay(ctrl: Option<u64>) -> u64 {
        let slot = 84 * MS;
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        let t_last = 20 * slot;
        let mut tick = 0;
        for i in 0..=20u64 {
            let t = i * slot;
            while tick < t {
                let _ = r.on_ack_timer(tick);
                tick += 20 * MS;
            }
            let _ = r.on_datagram(data(i, i), t);
        }
        let mut next_ctrl = ctrl.map(|c| t_last + c);
        let mut seq = 100;
        loop {
            while let Some(c) = next_ctrl.filter(|&c| c <= tick) {
                r.note_non_eliciting(seq, c);
                seq += 1;
                next_ctrl = ctrl.map(|p| c + p);
            }
            if r.on_ack_timer(tick).is_some() {
                return tick - t_last;
            }
            tick += 20 * MS;
            assert!(
                tick < t_last + 30_000 * MS,
                "akış sonu ACK'i 30 s içinde çıkmalı"
            );
        }
    }

    /// [AKIŞ-SONU YEDEĞİ] Akış bittikten sonra süren kontrol trafiği son ACK'i bekletmez: eş
    /// etkinliği yalnız DATA akışı sürerken (son DATA'nın sessizlik penceresi içinde) sayılır.
    /// Kontrol trafiği varken akış sonu ACK'i, trafiksiz kola göre en çok bir sessizlik penceresi
    /// (4 × 84 ms) geç çıkar — her aralıkta, sessizlik penceresinden sık trafikte de.
    #[test]
    fn end_of_flight_ack_is_not_held_by_control_traffic_after_the_flight() {
        let quiet = end_of_flight_ack_delay(None);
        assert!(
            quiet <= 4 * 84 * MS + 20 * MS,
            "trafiksiz: akış sonu ACK'i sessizlik penceresi + bir tik içinde: {quiet}"
        );
        for c in [50u64, 100, 250, 500, 1_000, 1_500] {
            let d = end_of_flight_ack_delay(Some(c * MS));
            assert!(
                d <= quiet + 4 * 84 * MS,
                "kontrol {c} ms aralıkla: akış sonu ACK'i {} ms (trafiksiz {} ms)",
                d / MS,
                quiet / MS
            );
        }
    }

    /// Akış sonu kuyruğu kurgusundaki alıcı olayı.
    #[derive(Clone, Copy)]
    enum TailEv {
        /// DATA (PN, birim kimliği).
        Data(u64, u64),
        /// ACK istemeyen kontrol paketi (PARAM, prob; PN).
        Ctrl(u64),
    }

    /// Olayları sanal zamanda bütçeli (62 B/s dönüş) port alıcısına verir; ACK tiki 20 ms'de bir
    /// (runtime `ACK_TICK`), bir anda önce o ana dek varan olaylar işlenir. Dönen: `after`'dan sonra
    /// çıkan ilk ACK'in anı.
    fn first_ack_after(events: &[(u64, TailEv)], after: u64) -> u64 {
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        let mut ev = events.to_vec();
        ev.sort_by_key(|e| e.0);
        let mut next = ev.iter().peekable();
        let mut tick = 0;
        loop {
            while let Some(&&(t, e)) = next.peek().filter(|e| e.0 <= tick) {
                next.next();
                match e {
                    TailEv::Data(seq, unit) => {
                        if r.on_datagram(data(seq, unit), t).ack.is_some() && t > after {
                            return t;
                        }
                    }
                    TailEv::Ctrl(seq) => r.note_non_eliciting(seq, t),
                }
            }
            if r.on_ack_timer(tick).is_some() && tick > after {
                return tick;
            }
            tick += 20 * MS;
            assert!(tick < after + 30_000 * MS, "ACK 30 s içinde çıkmalı");
        }
    }

    /// Akış sonu kuyruğu (tests/shaped_relay.rs asimetrik hücre, tohum 2, `CC_SHAPED_CTRL_AFTER_MS`
    /// kolu). DATA 84 ms arayla; PN 1 bir PARAM. A turu: PN 0..=39, PN 2, 5, …, 35 yolda düşer →
    /// ACK'te 12 aralık (113 B); son dört PN kayıpsız (varış aralığı tabanı 84 ms). 2 s duraklamadan
    /// sonra B turu iki yeniden iletim: PN 40 (birim 2) ve PN 41 (birim 5); jeton akış sonu ACK'ine
    /// yetmez, ACK kovanın dolmasını bekler.
    /// `copy`: (an, birim) tek DATA PN 42. `ctrl`: B turundan sonra o aralıkla PARAM'ın kopyası
    /// (aynı PN 1; alıcıda küme değişmez, ACK boyu aynı kalır). Dönen: (olaylar, B turunun sonu).
    fn tail_events(copy: Option<(u64, u64)>, ctrl: Option<u64>) -> (Vec<(u64, TailEv)>, u64) {
        let slot = 84 * MS;
        let mut ev: Vec<(u64, TailEv)> = (0..40u64)
            .filter(|&i| i % 3 != 2 || i > 35)
            .map(|i| {
                let e = if i == 1 {
                    TailEv::Ctrl(1)
                } else {
                    TailEv::Data(i, i)
                };
                (i * slot, e)
            })
            .collect();
        let b0 = 39 * slot + 2_000 * MS;
        ev.push((b0, TailEv::Data(40, 2)));
        ev.push((b0 + slot, TailEv::Data(41, 5)));
        let end = b0 + slot;
        if let Some((at, unit)) = copy {
            ev.push((at, TailEv::Data(42, unit)));
        }
        if let Some(p) = ctrl {
            let mut t = end + p;
            while t < end + 12_000 * MS {
                ev.push((t, TailEv::Ctrl(1)));
                t += p;
            }
        }
        (ev, end)
    }

    /// [AKIŞ-SONU YEDEĞİ] Akış sessizken gelen, çoktan teslim edilmiş birimin kopyası (göndericinin
    /// PTO probu) akışı yeniden başlatmaz: akış sonu ACK'i kopyasız koldaki anda çıkar, sonra gelen
    /// kontrol trafiği onu uzatmaz. Eski kuralda kopya (1) yedek kuralını yeniden kuruyordu, (2)
    /// uzun boşluğu 4 × taban olarak varış penceresine yazıp B turunun boşluğuyla birlikte tabanı
    /// 84 → 336 ms'ye, sessizlik penceresini 1,34 s'ye şişiriyordu, (3) kontrol paketleri o pencere
    /// boyunca saati tazeliyordu. Ölçülen koşu: kopyasız 14,76 s, kopyayla 17,16 s.
    #[test]
    fn redundant_copy_after_the_flight_does_not_hold_the_end_of_flight_ack() {
        let window = K_EPISODE * 84 * MS;
        let tick = 20 * MS;
        let (ev, end) = tail_events(None, None);
        let t0 = first_ack_after(&ev, end);
        assert!(
            t0 > end + window + 1_000 * MS,
            "kurgu: akış sonu ACK'i jeton bekliyor ({} ms)",
            (t0 - end) / MS
        );
        // Kopya, jeton yetmeden 150 ms önce varır (ölçülen koşu: 14,58 s ↔ 14,72 s).
        let at = t0 - 150 * MS;
        // Kontrol aralığı (ms; 0 = kontrol trafiği yok).
        for ctrl in [0u64, 50, 100, 250, 500, 1_000, 1_500] {
            let (ev, _) = tail_events(Some((at, 2)), (ctrl > 0).then_some(ctrl * MS));
            let d = first_ack_after(&ev, end);
            assert!(
                d <= t0.max(at + window + tick),
                "kopya + kontrol {ctrl} ms: akış sonu ACK'i {} ms (kopyasız {} ms)",
                (d - end) / MS,
                (t0 - end) / MS
            );
        }
        // Sınır: aynı varış yeni birim taşırsa (birim 8, hiç ulaşmamıştı) akış yeniden başlar ve
        // yedek kuralı ona uygulanır — muafiyet yalnız teslim edilmiş birimin kopyasınadır.
        let (ev, _) = tail_events(Some((at, 8)), None);
        assert!(
            first_ack_after(&ev, end) > t0,
            "yeni birim akışı yeniden başlatır"
        );
    }

    /// Ölçülen koşunun alıcı tarafı (`CC_SHAPED_TL_DUMP`, `dc4365dd`, KT250 tohum 2, 1. koşu; 10 ms
    /// çözünürlük): (PN, birim, varış ms). PN 2 ve 5 PARAM; 11,30 s'den sonra PN 22'li PARAM kopyaları
    /// (kontrol trafiği), 14,58 s'de birim 6'nın PTO kopyası PN 60. Eski kuralda bu kayıt 17,26 s
    /// veriyordu (ölçülen 17,16 s); kopyasız 14,72 s (ölçülen ikinci koşu 14,76 s).
    #[test]
    fn measured_tail_replay_copy_and_control_do_not_hold_the_last_ack() {
        #[rustfmt::skip]
        let d: &[(u64, u64, u64)] = &[
            (0, 0, 380), (1, 1, 470), (3, 2, 560), (4, 3, 640), (6, 4, 740), (7, 5, 820),
            (9, 7, 990), (11, 8, 1160), (13, 9, 1250), (14, 10, 1330), (18, 14, 1670),
            (19, 15, 1750), (20, 16, 1840), (23, 18, 2010), (25, 1, 2180), (27, 21, 2350),
            (29, 23, 2520), (30, 24, 2600), (31, 25, 2690), (32, 26, 2770), (33, 27, 2860),
            (35, 29, 3030), (36, 30, 3110), (37, 31, 3200), (38, 32, 3280), (39, 33, 3360),
            (40, 34, 3450), (42, 36, 3620), (45, 39, 3870), (47, 11, 6990), (48, 12, 7080),
            (49, 13, 7160), (50, 17, 7250), (51, 19, 7330), (52, 20, 7410), (53, 22, 7500),
            (54, 28, 7580), (56, 37, 7750), (57, 38, 7840), (58, 6, 10660), (59, 35, 10740),
        ];
        #[rustfmt::skip]
        let ctrl_ms: &[u64] = &[
            11300, 11550, 11800, 12050, 12300, 12550, 13050, 13300, 13550, 13800, 14050, 14300,
            14800, 15050, 15300, 15550, 15800, 16050, 16300, 17050, 17300, 18050, 18300,
        ];
        // `probe`: 14,58 s'de PN 60'ın taşıdığı birim; `lost59`: PN 59 da yolda düşer (birim 35 hiç
        // ulaşmaz). Dönen: `after`'dan sonraki ilk ACK.
        let run = |probe: Option<u64>, ctrl: bool, lost59: bool, after: u64| {
            let mut ev: Vec<(u64, TailEv)> = d
                .iter()
                .filter(|&&(s, _, _)| !(lost59 && s == 59))
                .map(|&(s, u, t)| (t * MS, TailEv::Data(s, u)))
                .collect();
            ev.push((480 * MS, TailEv::Ctrl(2)));
            ev.push((650 * MS, TailEv::Ctrl(5)));
            if let Some(unit) = probe {
                ev.push((14_580 * MS, TailEv::Data(60, unit)));
            }
            if ctrl {
                ev.extend(ctrl_ms.iter().map(|&t| (t * MS, TailEv::Ctrl(22))));
            }
            first_ack_after(&ev, after)
        };
        let end = 10_740 * MS;
        let base = run(None, false, false, end);
        assert_eq!(
            run(None, true, false, end),
            base,
            "kopyasız: kontrol trafiği etkisiz"
        );
        assert_eq!(
            run(Some(6), false, false, end),
            base,
            "kopya son ACK'i bekletmez"
        );
        assert_eq!(
            run(Some(6), true, false, end),
            base,
            "kopya + kontrol son ACK'i bekletmez"
        );
        // PN 59 da düşseydi PTO probu YENİ birim 35'i taşırdı: akış yeniden başlar, ACK bir ritim
        // penceresi (4 × 90 ms) ve jetonu bekler (400 ms); kontrol trafiği en çok bir pencere ekler
        // (600 ms). Eski kural probun boşluğunu pencereye 4 × taban yazıyordu: probdan 1 460 ms,
        // kontrol trafiğiyle 2 680 ms.
        let probe = 14_580 * MS;
        let quiet = run(Some(35), false, true, probe) - probe;
        let ctrl = run(Some(35), true, true, probe) - probe;
        assert!(
            quiet <= 500 * MS,
            "yeni birimli prob: ACK probdan {} ms sonra",
            quiet / MS
        );
        assert!(
            ctrl <= 800 * MS,
            "yeni birimli prob + kontrol: ACK probdan {} ms sonra",
            ctrl / MS
        );
    }

    /// [AKIŞ-SONU YEDEĞİ] Akış sessizken gelen tek PTO probu YENİ birim taşırsa (birim 8, hiç
    /// ulaşmamıştı) akış yeniden başlar ve ACK bir sessizlik penceresi (4 × 84 ms) bekler; ama
    /// pencere probun uzun boşluğuyla şişmez, kontrol trafiği onu en çok bir pencere uzatır. Eski
    /// kuralda probun boşluğu 4 × taban olarak varış penceresine yazılıyor, B turunun boşluğuyla
    /// birlikte tabanı 84 → 336 ms'ye, pencereyi 1 344 ms'ye çıkarıyordu. Gönderici kuyruktaki yeni
    /// birimi PTO probuyla taşıdığında (şerit kipinde) bu her durakta tekrarlar. Karşı olgu: probdan
    /// 84 ms sonra ikinci yeni birim gelirse akış ritimde sürer, ACK ikinciden sonra bir pencere bekler.
    #[test]
    fn new_unit_probe_after_the_flight_restarts_it_for_one_rhythm_window() {
        let window = K_EPISODE * 84 * MS;
        let tick = 20 * MS;
        let (ev, end) = tail_events(None, None);
        let t0 = first_ack_after(&ev, end);
        let at = t0 - 150 * MS;
        // Kontrol aralığı (ms; 0 = kontrol trafiği yok).
        for ctrl in [0u64, 50, 100, 250, 500, 1_000, 1_500] {
            let (ev, _) = tail_events(Some((at, 8)), (ctrl > 0).then_some(ctrl * MS));
            let d = first_ack_after(&ev, end) - at;
            assert!(
                d > window,
                "yeni birim akışı yeniden başlatır (kontrol {ctrl} ms): ACK probdan {} ms sonra",
                d / MS
            );
            let bound = if ctrl == 0 { window } else { 2 * window };
            assert!(
                d <= bound + tick,
                "pencere probun boşluğuyla şişmez (kontrol {ctrl} ms): ACK probdan {} ms sonra",
                d / MS
            );
        }
        let (mut ev, _) = tail_events(Some((at, 8)), None);
        ev.push((at + 84 * MS, TailEv::Data(43, 11)));
        assert!(
            first_ack_after(&ev, end) > at + 84 * MS + window,
            "ritimde gelen ikinci yeni birim akıştır"
        );
    }

    /// Olayları `first_ack_after` ile aynı düzende verir; `(from, to]` aralığında çıkan ACK anları.
    fn acks_between(events: &[(u64, TailEv)], from: u64, to: u64) -> Vec<u64> {
        acks_between_with_line(events, from, to, 0)
    }

    /// [`acks_between`]; her çağrıdan önce dönüş hattında ACK'in önünde `ahead_ns` serileşme var.
    fn acks_between_with_line(
        events: &[(u64, TailEv)],
        from: u64,
        to: u64,
        ahead_ns: u64,
    ) -> Vec<u64> {
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(Some(62));
        r.set_return_line_ahead_ns(ahead_ns);
        let mut ev = events.to_vec();
        ev.sort_by_key(|e| e.0);
        let mut next = ev.iter().peekable();
        let mut out = Vec::new();
        let mut tick = 0;
        while tick <= to {
            while let Some(&&(t, e)) = next.peek().filter(|e| e.0 <= tick) {
                next.next();
                match e {
                    TailEv::Data(seq, unit) => {
                        if r.on_datagram(data(seq, unit), t).ack.is_some() && t > from && t <= to {
                            out.push(t);
                        }
                    }
                    TailEv::Ctrl(seq) => r.note_non_eliciting(seq, t),
                }
            }
            if r.on_ack_timer(tick).is_some() && tick > from {
                out.push(tick);
            }
            tick += 20 * MS;
        }
        out
    }

    /// Akış sonu ACK'inden sonra gelen kopya kurgusu. A turu: PN 0..=39, 84 ms arayla; PN 1 PARAM;
    /// PN 2, 5, …, 35 yolda düşer (12 aralık). `copy`: (an, birim) tek DATA PN 40. C turu: A'dan
    /// `c_after` sonra dört yeniden iletim (PN 41..=44, birim 2, 5, 8, 11), 84 ms arayla. `ctrl`:
    /// A'dan sonra o aralıkla PARAM kopyası (PN 1). Dönen: (olaylar, A'nın sonu, C'nin sonu).
    fn copy_after_eof_events(
        copy: Option<(u64, u64)>,
        ctrl: Option<u64>,
        c_after: u64,
    ) -> (Vec<(u64, TailEv)>, u64, u64) {
        let slot = 84 * MS;
        let mut ev: Vec<(u64, TailEv)> = (0..40u64)
            .filter(|&i| i % 3 != 2 || i > 35)
            .map(|i| {
                let e = if i == 1 {
                    TailEv::Ctrl(1)
                } else {
                    TailEv::Data(i, i)
                };
                (i * slot, e)
            })
            .collect();
        let end = 39 * slot;
        if let Some((at, unit)) = copy {
            ev.push((at, TailEv::Data(40, unit)));
        }
        let c0 = end + c_after;
        for (k, unit) in [2u64, 5, 8, 11].into_iter().enumerate() {
            ev.push((c0 + k as u64 * slot, TailEv::Data(41 + k as u64, unit)));
        }
        let c_end = c0 + 3 * slot;
        if let Some(p) = ctrl {
            let mut t = end + p;
            while t < c_end + 20_000 * MS {
                ev.push((t, TailEv::Ctrl(1)));
                t += p;
            }
        }
        (ev, end, c_end)
    }

    /// [BİLGİSİZ-ACK] Akış sonu ACK'inden sonra gelen teslim edilmiş birim kopyası (göndericinin eski
    /// bir ACK'le kayıp ilan edip yeniden ilettiği ya da PTO'yla yolladığı) bilgi taşımaz: ACK'i,
    /// sonraki turun akış sonu ACK'inin jetonunu harcamaz; kontrol trafiği bunu değiştirmez. Eski
    /// kuralda kopyanın ACK'i jeton yetince çıkıyor, sonraki turun raporu bir bütçe aralığı bekliyordu
    /// (ölçülen tohum 3 koşusu: 4,82 s'de yeni birimsiz ACK, sonraki rapor 11,56 s; gecikme 4,52 s).
    /// Kopyasız kolda kopyanın PN'i aynı anda ACK istemeyen paketle gelir (ACK aralığı aynı kalır).
    /// Karşı olgu: kopya son ACK'in göndericiye en erken varış anından sonra gelirse (ACK kaybına
    /// tepki olabilir) ACK'i bugünkü kuralla çıkar.
    #[test]
    fn redundant_copy_after_the_end_of_flight_ack_does_not_spend_the_next_reports_token() {
        let c_after = 4_500 * MS;
        let window = K_EPISODE * 84 * MS;
        let tick = 20 * MS;
        // Kopyasız kol: PN 40 akış sonu ACK'inden 250 ms sonra ACK istemeyen paket.
        let (probe, end, _) = copy_after_eof_events(None, None, c_after);
        let t_e = first_ack_after(&probe, end);
        let at = t_e + 250 * MS;
        let mut base_ev = probe.clone();
        base_ev.push((at, TailEv::Ctrl(40)));
        let c0 = end + c_after;
        let (_, _, c_end) = copy_after_eof_events(None, None, c_after);
        let base = acks_between(&base_ev, end, c_end + 20_000 * MS);
        assert_eq!(base[0], t_e, "kurgu: A'nın akış sonu ACK'i");
        let base_c = *base.iter().find(|&&t| t > c0).expect("C turunun ACK'i");
        assert!(
            base_c <= c_end + window + tick,
            "kurgu: kopyasız kolda C'nin raporu sessizlikle çıkar ({} ms)",
            (base_c - c_end) / MS
        );
        // Kontrol aralığı (ms; 0 = kontrol trafiği yok). Taban her aralıkta aynı kontrol trafiğiyle
        // kopyasız koldur (kontrol trafiği C turu sürerken saati bir pencere uzatabilir — 640f909b).
        for ctrl in [0u64, 250, 1_000] {
            let ctrl_ns = (ctrl > 0).then_some(ctrl * MS);
            let (mut base_ev, _, _) = copy_after_eof_events(None, ctrl_ns, c_after);
            base_ev.push((at, TailEv::Ctrl(40)));
            let base_c = *acks_between(&base_ev, end, c_end + 20_000 * MS)
                .iter()
                .find(|&&t| t > c0)
                .expect("kopyasız kolda C turunun ACK'i");
            let (ev, _, _) = copy_after_eof_events(Some((at, 0)), ctrl_ns, c_after);
            let acks = acks_between(&ev, end, c_end + 20_000 * MS);
            assert!(
                !acks.iter().any(|&t| t > at && t < c0),
                "kontrol {ctrl} ms: kopya kendi ACK'ini çıkarmaz: {:?}",
                acks.iter().map(|t| (t - end) / MS).collect::<Vec<_>>()
            );
            let c_ack = *acks.iter().find(|&&t| t > c0).expect("C turunun ACK'i");
            assert_eq!(
                c_ack,
                base_c,
                "kontrol {ctrl} ms: C'nin raporu kopyasız koldaki anda ({} ↔ {} ms)",
                (c_ack - end) / MS,
                (base_c - end) / MS
            );
        }
        // Karşı olgu: kopya A'nın ACK'inin en erken varış anından (113 B / 62 B/s ≈ 1,82 s) sonra.
        let late = t_e + 2_500 * MS;
        let (ev, _, _) = copy_after_eof_events(Some((late, 0)), None, c_after);
        let acks = acks_between(&ev, end, c_end + 20_000 * MS);
        assert!(
            acks.iter().any(|&t| t > late && t < c0),
            "ufuktan sonra gelen kopya ACK'lenir: {:?}",
            acks.iter().map(|t| (t - end) / MS).collect::<Vec<_>>()
        );
    }

    /// [DÖNÜŞ-HATTI] ACK'in varış ufku dönüş hattında önündeki baytları sayar. Aynı cihazın ters
    /// oturumu (PARAM, alındı) ACK'in önünde hat tutarken gelen teslim edilmiş birim kopyası o ACK'e
    /// tepki olamaz: gönderici onu ACK'i görmeden yolladı. Kurgu `redundant_copy_after_…`'in karşı
    /// olgusudur: kopya akış sonu ACK'inden 2,5 s sonra gelir; boş hatta ACK'in en erken varışı
    /// 1,82 s'dir (113 B / 62 B/s), kopya ACK'lenir. Hatta ACK'in önünde 1,37 s (85 B PARAM)
    /// varken en erken varış 3,19 s'dir: kopya bilgi taşımaz, kendi ACK'ini çıkarmaz ve sonraki turun
    /// raporu kopyasız koldaki anda çıkar. Düzeltmeden önce (ufuk hattı boş sayar) kopya ACK'lenir.
    #[test]
    fn copy_before_an_ack_queued_behind_the_line_can_arrive_carries_no_information() {
        let c_after = 4_500 * MS;
        let ahead = 1_370 * MS;
        let (probe, end, c_end) = copy_after_eof_events(None, None, c_after);
        let t_e = first_ack_after(&probe, end);
        let late = t_e + 2_500 * MS;
        let c0 = end + c_after;
        let to = c_end + 20_000 * MS;
        let (ev, _, _) = copy_after_eof_events(Some((late, 0)), None, c_after);
        let free = acks_between_with_line(&ev, end, to, 0);
        assert!(
            free.iter().any(|&t| t > late && t < c0),
            "kurgu: boş hatta kopya ACK'lenir: {:?}",
            free.iter().map(|t| (t - end) / MS).collect::<Vec<_>>()
        );
        let busy = acks_between_with_line(&ev, end, to, ahead);
        assert!(
            !busy.iter().any(|&t| t > late && t < c0),
            "hatta 1,37 s önünde bayt varken kopya kendi ACK'ini çıkarmaz: {:?}",
            busy.iter().map(|t| (t - end) / MS).collect::<Vec<_>>()
        );
        let mut base_ev = probe.clone();
        base_ev.push((late, TailEv::Ctrl(40)));
        let base_c = *acks_between_with_line(&base_ev, end, to, ahead)
            .iter()
            .find(|&&t| t > c0)
            .expect("kopyasız kolda C turunun ACK'i");
        let c_ack = *busy.iter().find(|&&t| t > c0).expect("C turunun ACK'i");
        assert_eq!(c_ack, base_c, "C'nin raporu kopyasız koldaki anda");
    }

    /// [DÖNÜŞ-BÜTÇESİ] Eski alıcı bütçeden etkilenmez (kendi cadence tabanı var).
    #[test]
    fn return_budget_does_not_touch_the_legacy_receiver() {
        let mut a = Receiver::new();
        let mut b = Receiver::new();
        b.set_return_bandwidth(Some(62));
        for i in 0..40u64 {
            let x = a.on_datagram(data(i, i), i * 100 * MS).ack;
            let y = b.on_datagram(data(i, i), i * 100 * MS).ack;
            assert_eq!(x, y, "eski alıcıda bütçe etkisiz (seq {i})");
        }
        assert_eq!(b.diag_ack_budget_deferred(), 0);
    }

    /// [DÖNÜŞ-BÜTÇESİ] Tipik ACK aralığı: dönüş hızıyla ters orantılı; simetrik hızlı yolda ≈ 26 ms
    /// (göndericinin ilk penceresi ve PTO payı varsayılanda kalır).
    #[test]
    fn budgeted_ack_interval_scales_inversely_with_return_bandwidth() {
        // Tipik ACK 81 B; Y = 0,25.
        assert_eq!(budgeted_ack_interval_ns(62), 81 * 1_000_000_000 / 15);
        assert_eq!(budgeted_ack_interval_ns(12_500), 81 * 1_000_000_000 / 3_125);
        assert!(budgeted_ack_interval_ns(12_500) < 30 * MS);
        assert_eq!(budgeted_ack_interval_ns(0), u64::MAX);
        assert_eq!(
            budgeted_ack_interval_ns(3),
            u64::MAX,
            "bütçe 0 B/s → ACK yok"
        );
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
