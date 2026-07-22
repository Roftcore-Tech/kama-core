//! Sender State Machine (engine spec §3) — 18 bileşeni tek olay-güdümlü döngüde birleştiren ŞEF.
//!
//! **Deterministik çekirdek (ADR-17):** olaylar girer → transmit KARARLARI çıkar. Gerçek I/O ve
//! kripto DIŞINDA (INV-ENGINE-SOC): Sender yalnız opak birim KİMLİĞİ + BOYUTU tutar (`OpaqueUnit`),
//! bytes/AEAD/anahtar TUTMAZ. `on_event` saf: aynı girdi → aynı çıktı → golden-vector'le test edilir.
//!
//! **INV-RATE-1:** loss `send_rate`'i DÜŞÜRMEZ. Recovery durumları (RECOVER_ARQ/FWD) pacing_gain'i
//! `Steady` tutar; loss yalnız [`loss`](crate::loss) + fuzzy `loss_ppm` → FEC/ARQ recovery kolunu besler.
//! Retransmit/duplication DATA pacing bütçesini PAYLAŞIR (§9/§13) → recovery fırtınası rate'i patlatamaz.
//!
//! **Recovery İCRA SoC (INV-FEC-1/INV-ENGINE-SOC):** ARQ retransmit (§9) + duplication (§11)
//! transport-owned'dır — transport kayıp birimin METADATA'sını (id/size) tutar, yeniden pace eder.
//! FEC parity (§10) yalnız POLİTİKA'dır (parity_ratio + `RecoveryAction::MinimalParity`); RS-encode +
//! per-shard AEAD GÜVENLİK katmanında (code-then-encrypt-authenticate-shards). Transport parity BYTE'ı
//! ÜRETMEZ — oranı verir, güvenlik katmanı encode eder, transport opak parity birimlerini pace eder.
//!
//! **SPEC-UYUMLU (§3 AMENDMENT-1 + §5.1/AMENDMENT-2 — impl-driven, spec'e patch'lendi):** RECOVER_FWD
//! *ve* RECOVER_ARQ her aktif fazdan erişilir (§3 AMENDMENT-1: ACK-yolu çökerse STARTUP platoya ulaşamaz
//! → recovery kilitlenmesin; forward-recovery ACK-bağımsız). Öncelik: ackconf LOW → RECOVER_FWD (§11) >
//! pending loss → RECOVER_ARQ (§9). ack_confidence = ACK SESSİZLİĞİ (§5.1/AMENDMENT-2: naif sent/recv
//! oranı değil). Bu kararlar önce implementasyonda bulundu, gerekçeleriyle spec'e cerrahi eklendi (çelişki yok).
//!
//! **Kapsam (Artım 1+3):** state machine + ölçüm + karar pipeline + DATA pacing + recovery İCRA
//! (ARQ retransmit / forward duplication). Receiver (§4) = sonraki artım.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::arq::{
    ArqScheduler, CapExceedPolicy, DeadlineArqScheduler, LostSeq, ReorderTracker, RetxConfig,
    RetxDecision,
};
use crate::budget::{self, BudgetConfig};
use crate::bw::{BtlBwFilter, DeliveryRateSampler, SentPacket};
use crate::emergency::{
    self, EmergencyConfig, EmergencyLevel, Framing, HysteresisConfig, HysteresisController,
    RecoveryAction, RecoveryModeController,
};
use crate::fusion::{
    DeterministicFuzzyController, FusionController, FusionDecision, FusionInputSnapshot,
    FusionInputs, FuzzyConfig, FuzzyConfigError, Mode, ValidatedFuzzyConfig,
};
use crate::loss::{LossDetector, ThresholdLossDetector};
use crate::pacer::{TokenBucket, DEFAULT_BURST_CAP};
use crate::rate::{effective_send_rate, PacingPhase, SrcConfig, DEFAULT_MTU};
use crate::rtt::{apply_ack_delay, RttEstimator};

/// Sender durumları (§3). Geçişler event-driven + deterministik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderState {
    /// Gönderilecek yok; timer'lar pasif.
    Idle,
    /// §3.3 cold-start: motor HİÇ ölçüm yokken. En-kötü-köşe önsel pacing (floor) + geometrik kopya;
    /// loss-ilanı/PTO/emergency/ack_confidence DONDURULUR. İlk geçerli SACK'te → Startup. [K-KEYSTONE]
    PreConverge,
    /// BtlBw hızlı keşif (pacing_gain yüksek), path doldur.
    Startup,
    /// [TUR-3b] Startup-exit drain (BBR-analog): 1/high_gain ile kuyruk → BDP; sonra Steady.
    Drain,
    /// [TUR-3c] Bant-probe (1.25×): app-limited kilidin tek yukarı-yolu; sözleşme D-2/4.
    ProbeUp,
    /// pace = BtlBw; periyodik BW/RTprop probe.
    Steady,
    /// residual loss + ack_confidence OK → selective retransmit (§9).
    RecoverArq,
    /// ack_confidence LOW → forward redundancy (§11), ACK-bağımsız.
    RecoverFwd,
    /// RTprop probe: inflight düşür, min-RTT ölç (§8.3).
    DrainRtprop,
}

/// Opak güvenli birim — güvenlik katmanının `ct||tag` blob'u. Transport yalnız KİMLİK+BOYUT
/// görür (INV-ENGINE-SOC: bytes YOK). `id` çağıranın gerçek payload'una geri-eşlemesi içindir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueUnit {
    /// Çağıran-tanımlı birim kimliği (bytes'a geri-eşleme).
    pub id: u64,
    /// Wire boyutu (pacing/inflight muhasebesi).
    pub size: u64,
}

/// Sender'a giren olaylar (§3 ana döngü). `PacketSent` DIŞ olay DEĞİL — `ScheduleTransmit`
/// içinde OnPacketSent (§3.1) muhasebesi olarak yapılır.
#[derive(Debug, Clone)]
pub enum Event {
    /// App gönderilecek opak birim verdi → enqueue; IDLE ise STARTUP.
    AppData(OpaqueUnit),
    /// ACK geldi (§2 rekonstrüksiyondan SONRA: yeni acked seq'ler + ack_delay).
    AckReceived(AckInfo),
    /// Timer ateşlendi (§12).
    TimerFired(Timer),
    /// DR-S5-4: RECOVERED-control (engine block_id/esi→unit_id-çevrimi SONRASI). SAF-LOGICAL olay:
    /// retransmit-cancel + retention-release + logical-defter; **rate/pacing/schedule yoluna GİRMEZ**
    /// (emit-üretmez; N8: recovered≠ACK — BtlBw/delivered-kredi-yolu AÇILMAZ).
    RecoveredControl(RecoveredInfo),
}

/// DR-S5-4: RECOVERED-control'ün sender-girdisi. Engine, Control::Recovered'ın logical (block_id,
/// esi_ranges) referanslarını kendi send-record'undan native `unit_id`'ye ÇEVİRİP verir (SoC:
/// çevrim engine'de; sender ACK-cancel/retention-release ile AYNI unit_id-mekanizmasını kullanır).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredInfo {
    /// Kurtarılan (receiver-FEC-decode) birimler.
    pub units: Vec<RecoveredUnit>,
}

/// Tek kurtarılan-birim: native unit_id + logical-kimlik (defter-kaydı için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveredUnit {
    /// Transport-native birim kimliği (cancel-anahtarı; ACK-cancel ile aynı).
    pub unit_id: u64,
    /// Logical-kimlik (confirmed_logical defter-kaydı; SrLogicalRef::Fec).
    pub block_id: u32,
    /// Logical ESI.
    pub esi: u16,
}

/// Commit-G (DR-1a): FEC-aktüasyon TEK-OTORİTESİ. Ayrı `fec_enabled: bool` KALDIRILDI
/// (gizli-ikinci-state-yok); Manual-enabled enum-İÇİNDE. Default: `Manual { enabled: false }`
/// (mevcut-davranış-birebir). Geçişler block-boundary-etkin (çağıran-kontratı: engine mode-değişimini
/// blok-sınırında-çağırır; authority yalnız blok-granüler-noktalarda-okunur — enqueue-anı/latch-anı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FecActuationMode {
    /// Manuel-otorite: `enabled` emisyon-kapısı (DR-S5-1-davranışı).
    Manual { enabled: bool },
    /// Controller-otorite: FEC-açıklığı latch'lenen-ratio'dan türer; manuel-toggle typed-hata.
    Controller,
}

/// Commit-I (review-hardening §1, Model-D-dar-varyantı): BLOK-OTORİTE-WITNESS'ı — blok-AÇILIŞINDA
/// aktif-mode'u YAKALAR; o-bloğun-tüm-otorite-kararları (latch/Manual-dal-seçimi) BU-mode'la-yapılır.
/// Private-alan → yalnız [`Sender::open_fec_block_authority`] üretir (forge/elle-kurulum-derlenmez).
/// YAPISAL-GARANTİ: "blok hangi-mode'da-açıldıysa o-mode'da-kapanır" — sonraki `set_fec_actuation_mode`
/// in-flight-witness'ı ETKİLEYEMEZ (Copy-değer). Pending-state-YOK → idle-transition-hemen; no-future-
/// block-leak-yok; repeated-transition = last-write-wins (aktif-mode'da; açık-witness'lar-değişmez).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecBlockAuthority {
    mode: FecActuationMode,
}

impl FecBlockAuthority {
    /// Bu-bloğun-otorite-mode'u (blok-açılış-anı-yakalaması).
    #[must_use]
    pub fn mode(&self) -> FecActuationMode {
        self.mode
    }
}

/// Commit-G: otorite-API hataları (sessiz-no-op-YASAK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FecAuthorityError {
    /// Controller-mode'da manuel-FEC-toggle çağrıldı (state-DEĞİŞMEDİ; typed-red).
    #[error("manual FEC toggle rejected: authority is Controller (NotInManualMode)")]
    NotInManualMode,
}

/// Commit-G: controller-karar-anlık-görüntüsü (same-event-causality-guard girdisi).
/// `event_index` = üretildiği `fusion_event_index` (Commit-D-sayacı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerDecisionSnapshot {
    pub ratio_permille: u32,
    pub event_index: u64,
    pub no_rule: bool,
    pub emergency: bool,
}

/// Commit-G: latch-baskılama nedenleri (named; kalibrasyon-dışı-hücre-işaretleri).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FecSuppressionReason {
    NoRule,
    NoEligibleDecision,
    DecisionEventMismatch,
    /// "Temporary safety interlock pending 1b/1d arbitration" — ürün-politikası/kalibrasyon-sonucu-DEĞİL.
    EmergencyPendingArbitration,
    ManualDisabled,
}

/// Commit-G: latch-sonucu (saf-karar-zinciri çıktısı; her-adım-görünür).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecLatchResult {
    pub requested_ratio_permille: u32,
    pub computed_r: u16,
    pub clamped_r: u16,
    /// Enqueue-hedefi (baskılamada 0; requested/computed telemetride-görünür-kalır).
    pub planned_r: u16,
    pub same_event_eligible: bool,
    pub suppression: Option<FecSuppressionReason>,
}

/// Commit-G/H: FEC-aktüasyon teşhisi (named-struct; tuple-YASAK). Sender yalnız SON-latch +
/// bounded-sayaç tutar (per-block-tam-kayıt harness/test-yolunda; alloc/unbounded-log-yok).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecActuationDiag {
    pub authority_mode: FecActuationMode,
    pub last_latch: Option<FecLatchResult>,
    pub latch_event_index: u64,
    pub no_eligible_decision_count: u64,
    pub decision_event_mismatch_count: u64,
    pub no_rule_zero_count: u64,
    pub manual_toggle_rejected_count: u64,
    /// Kümülatif enqueue_fec_parity-çağrısı (requested→enqueued-zinciri).
    pub enqueued_r_total: u64,
    /// Kümülatif wire'a-çıkan FecParity-decision (enqueued→emitted-zinciri).
    pub emitted_r_total: u64,
    /// RECOVERED-iptalleri (DR-S5-4-sayacı; canceled-zinciri).
    pub canceled_r_total: u64,
    /// §8.1-EXACT-TANIM: SON-plan_fec_parity-token-REDDİ-anında-kuyrukta-bekleyen-parity-adedi
    /// (GAUGE — kümülatif-DEĞİL; requested−emitted-DEĞİL; clamp/suppression/mode-disabled-farkları
    /// token-denial-SAYILMAZ; yalnız-pacer-token-reddi). 1b-tasarım-girdisi.
    pub token_denied_r_gauge: u64,
    /// Commit-I: son-mode-transition-isteğinin event-index'i.
    pub last_transition_event_index: u64,
    /// Commit-I: uygulanan-transition-sayısı (saturating; wrap-yok).
    pub transition_applied_count: u64,
}

/// Commit-G: ratio→r SAF-hesabı (§8). Half-up; float/RNG/platform-rounding-YOK; overflow-kanıtı:
/// ratio≤1000 ∧ k_actual≤255 → çarpım ≤255_000 « u64. `R_ENQUEUE_CAP`-YOK (tek-clamp profile-bound).
#[must_use]
pub fn compute_requested_r(ratio_permille: u32, k_actual: u8, r_code_max: u16) -> (u16, u16) {
    let unclamped = (u64::from(ratio_permille) * u64::from(k_actual) + 500) / 1000;
    // k_actual≤255 → unclamped≤255 → u16-güvenli (checked-dönüşüm-gereksiz-kanıtlı; yine-de-min-yolu):
    let computed = u16::try_from(unclamped).unwrap_or(u16::MAX);
    (computed, computed.min(r_code_max))
}

/// DR-S5-4: confirmed_logical defter-girişinin teyit-KAYNAĞI. M1 "FEC-kurtardığını-retransmit-etmiyor"
/// ayrımı: Recovered-teslim (FEC) vs WireAck-teslim. Kredi-anlamı YOK (defter salt-logical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmSource {
    /// Gerçek wire-ACK teyidi (DR-S5-3 yolu).
    WireAck,
    /// RECOVERED-control teyidi (FEC-decode; kredi-ÜRETMEZ — N8).
    Recovered,
}

/// §2 ACK rekonstrüksiyonunun ÇIKTISI (transport-control codec ayrı; burada çözülmüş girdi).
#[derive(Debug, Clone)]
pub struct AckInfo {
    /// Bu ACK ile YENİ acknowledge edilen seq'ler (full-counter).
    pub acked_seqs: Vec<u64>,
    /// largest_seq alımı → ACK gönderimi arası (§6 RTT düzeltmesi).
    pub ack_delay: Duration,
}

/// Zamanlayıcı türü (§12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {
    /// Probe timeout — zaman-eşiği loss tespiti + probe.
    Pto,
    /// RTprop probe vadesi → DRAIN_RTPROP.
    RtpropProbe,
}

/// Transmit kararı — çağıran `unit_id`'yi gerçekten yollar (güvenlik katmanı AEAD uygular, socket gönderir).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransmitDecision {
    /// Ne tür gönderim.
    pub kind: TransmitKind,
    /// Atanan wire seq (full-counter; wire'da u16, §2).
    pub seq: u64,
    /// Gönderilecek opak birim kimliği.
    pub unit_id: u64,
    /// Wire boyutu.
    pub size: u64,
    /// Send-record hook'u (§0.5 P0-4; DR-S5-1). YALNIZ FEC/control-emisyonunda `Some`; DATA/retx/dup için
    /// `None` (mevcut-davranış korunur). Engine bunu noise-wire `SealedPacketMeta`'ya tamamlar (seal-sonrası
    /// full_wire_pn/dh_epoch/sealed_len eklenir). SoC: transport-core NoiseState-TİPİ-TUTMAZ → kendi-hint-tipi.
    pub send_record: Option<SendRecordHint>,
}

/// Send-record hint'i (transport-tarafı; §0.5). Engine noise-wire-SealedPacketMeta'ya eşler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendRecordHint {
    /// Logical referans (FEC: block_id+esi).
    pub logical_ref: SrLogicalRef,
    /// Paket-sınıfı.
    pub packet_class: SrPacketClass,
    /// Retransmit-generation (§0.5 send_generation).
    pub send_generation: u32,
}

/// Transport-tarafı logical-ref (noise-wire LogicalPacketRef'e engine'de eşlenir; SoC-bağımsız).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrLogicalRef {
    /// FEC shard: block_id + esi.
    Fec { block_id: u32, esi: u16 },
}

/// Transport-tarafı paket-sınıfı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrPacketClass {
    Source,
    Repair,
    Control,
}

/// Bekleyen FEC-parity (engine FEC-encode-sonrası enqueue eder; sender pace-eder — DR-S5-1).
#[derive(Debug, Clone, Copy)]
struct PendingFecParity {
    block_id: u32,
    esi: u16,
    size: u64,
}

/// confirmed_logical üst-sınırı (DR-S5-3; SR-AUDIT-001 unbounded-alloc-yok). Engine
/// `take_confirmed_logical` ile drenaj yapmasa-da defter bounded: cap-dolunca en-eski-teyit düşer
/// (memory-hijyen; mühürlü-DEĞİL — logical-ledger'ın hiçbir mühürlü-muhasebeye etkisi yok).
const CONFIRMED_LOGICAL_CAP: usize = 4096;

/// sent_logical üst-sınırı (DR-S5-6; SR-AUDIT-001). GEREKÇE (M1-doğruluk-şartı, Abkarada şart-2):
/// eviction ancak "kayıt ACK-dönmeden düşerse" M1-delivery'yi yapay-düşürür. Kalıcı-büyüme kaynağı
/// in-flight DEĞİL (SACK-range'ler toplu-remove eder), KAYIP-ve-kurtarılmamış sızıntısıdır:
/// ~fwd_loss × rate × süre. 16384 ≈ 120s-M1-hücresi × %15-loss × ~910 pkt/s'e kadar SIFIR-eviction
/// (M1-hostile-profilleri bunun-altında). M1-HÜCRE-İŞARETLEME-KURALI: **YALNIZ evicted>0**
/// (sessiz-eğilme-yok). `ack_miss` işaretleme-kuralına GİRMEZ — miss>0 ∧ evicted==0 beklenen
/// Recovered→geç-ACK yarışıdır (RECOVERED kaydı-teyit-etti, geciken-wire-ACK sonra-geldi; artefakt
/// DEĞİL — adversarial-ÇÜRÜT-1 bulgusu). Gerekirse özgüllük-seti (recovered_then_acked) M1-sonrası.
const SENT_LOGICAL_CAP: usize = 16384;

/// Transmit türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmitKind {
    /// tx_queue'dan yeni DATA.
    Data,
    /// ARQ selective retransmit (§9) — kayıp birim, yeni seq.
    Retransmit,
    /// forward-recovery duplication (§11) — ACK-bağımsız redundant kopya.
    Duplicate,
    /// FEC parity shard (§10; DR-S5-1) — KOŞULLU-emit (fec_enabled). Shard-byte'ları güvenlik-katmanı
    /// encode-eder; sender YALNIZ pace-eder + send-record-hint üretir. fec_enabled=OFF → emit-EDİLMEZ.
    FecParity,
}

/// Bir gönderilmiş paketin muhasebe kaydı (RTT/BW/loss + retransmit).
struct InflightPacket {
    unit: OpaqueUnit,
    send_ns: u64,
    bw_sample: SentPacket,
    retx_count: u8,
    origin: TransmitKind,
}

/// Kayıp ilan edilmiş, recovery bekleyen kayıt (birim + öncelik metadata).
#[derive(Debug, Clone, Copy)]
struct LostRec {
    seq: u64,
    unit: OpaqueUnit,
    deadline_ns: u64,
    retx_count: u8,
    /// [TUR-2a D-1/3 kol-1] orijinal gönderim anı — un-loss'ta (retransmit HENÜZ yoksa,
    /// retx_count==0) orijinal RTT örneği GEÇERLİDİR; bu damgayla alınır.
    send_ns: u64,
}

/// [TUR-2b D-1/4] TOMBSTONE — kayıp-İLANI sonrası SAKLANAN gönderim kaydı (geç-ACK eşleştirme).
/// Active-flight muhasebesinden DÜŞER (gönderim durmaz) ama kayıt YAŞAR: geç-ACK gelirse
/// kol-1 (retx_count==0) BW-örneği NORMAL + Data'ysa RTT; kol-2 (retx'li) örneksiz sayaç
/// (tam kurallar TUR-2c). Süresi/tavanı dolan kaydın geç-ACK'i → Expired/Unknown (örneksiz).
#[derive(Debug, Clone, Copy)]
struct TombstoneRec {
    unit: OpaqueUnit,
    send_ns: u64,
    bw_sample: SentPacket,
    retx_count: u8,
    origin: TransmitKind,
    expiry_ns: u64, // ilan + K_TOMB×T_loss(ilan-anı)
}

/// Zamanlanmış forward-recovery duplication kopyası (§11 spacing — burst-loss dayanıklılığı).
#[derive(Debug, Clone, Copy)]
struct DupRec {
    due_ns: u64,
    unit: OpaqueUnit,
    /// §3.3-B geometrik kopya reschedule aralığı (ns). 0 = emergency (fire-once, reschedule YOK);
    /// >0 = PRE_CONVERGE geometrik kopya → ateşlenince due += interval, interval ×= COPY_MULTIPLIER.
    interval_ns: u64,
}

/// [TUR-3c] ProbeUp başarısız-çıkış türleri (AYRI raporlanır — sözleşme).
#[derive(Debug, Clone, Copy)]
enum ProbeExit {
    Abort,
    Timeout,
}

/// İlk RTT ölçülene kadar pacing/eşik için başlangıç RTT [CAL initial].
const INITIAL_RTT: Duration = Duration::from_millis(100);
/// STARTUP→STEADY plato: BtlBw N ardışık round Δ<ε ise plato [CAL].
const STARTUP_PLATEAU_ROUNDS: u32 = 3;
/// plato ε (per-mille): btlbw artışı < %5 → stabil.
const STARTUP_PLATEAU_EPS_PERMILLE: u64 = 50;
/// ack_confidence: bu kadar srtt sessizlik (ACK yok) → tam güvensizlik ölçeği [CAL].
const ACK_SILENCE_SRTT_MULT: u32 = 3;
/// last_recovery Duplication değilse forward-recovery için varsayılan faktör/aralık [CAL].
const DEFAULT_DUP_FACTOR: u8 = 2;
const DEFAULT_DUP_SPACING: Duration = Duration::from_millis(20);
/// §3.3-A en-kötü-köşe önsel pacing floor (B/s) — ürün floor'u ~0.25 kbps. min_rate fixture (Ek-B)
/// PRE_CONVERGE'de UYGULANMAZ; bu taban kullanılır → 1 kbps kanalı floodlamaz. [CAL PLACEHOLDER]
const PRE_CONVERGE_FLOOR_BW: u64 = 31;
/// [TUR-1b D-1/2] Cadence-taban bileşenleri — receiver §2.1/§16.1 AYNA sabitleri (sender-yerel,
/// deterministik). Beklenen ACK-frekansı (receiver ACK_FREQ_N) [CAL].
const EXPECTED_ACK_FREQ_N: u64 = 2;
/// SACK wire boyutu (§16.1 `sack_wire` = 65B; V2 15B gövde dahil).
const EAI_SACK_WIRE_BYTES: u64 = 65;
/// Return-bw worst-corner önseli (§16.1; §3.3-E-FULL ölçülen return_bw ile değiştirecek) [CAL].
const EAI_RETURN_BW_PRIOR: u64 = 31;
/// K_c cadence çarpanı = 3/2 (1.5) [CAL kalibrasyon-matrisi {1.25, 1.5, 2.0}].
const CADENCE_KC_NUM: u64 = 3;
const CADENCE_KC_DEN: u64 = 2;
/// [TUR-2a D-1/5] W_loss ledger-penceresi çarpanı: W_loss = K_wl × ack_epoch [CAL]
/// (K_bw/K_rtprop merkezi 8 ile hizalı başlangıç; saat-sınıfı: kanal-türevi, ZM-2).
const K_WL: u32 = 8;
/// [TUR-2b D-1/4] Tombstone TTL çarpanı: TTL = K_tomb × T_loss(ilan-anı) [CAL]. Onaylı formülün
/// `min(mesaj-deadline, ...)` kolu motorda uygulanamaz (per-unit deadline engine'e taşınmıyor —
/// bench/app katmanında) → TTL tek-kollu; spec §5 zeyli. Saat-sınıfı: T_loss-türevi (kanal, ZM-2).
const K_TOMB: u32 = 8;
/// [TUR-3a D-2/1] Estimator pencere çarpanları [CAL, matris {6,8,10}]:
/// W_BTLBW = max(10s, K_BW×ack_epoch); W_RTPROP = max(10s, K_RTPROP×corrected_srtt);
/// ack_epoch = max(corrected_srtt, EAI). Saat-sınıfı: kanal-türevi (ZM-2 — 10s wall-clock
/// sub-kbps'te 0.8-3.3 RTT'ye düşüyordu: btlbw 88→4 çöküş sınıfı, §16.2).
const K_BW: u32 = 8;
const K_RTPROP: u32 = 8;
/// [TUR-3a.5 commit-α] qd kısa-pencere çarpanı: W_qd = K_qd × ack_epoch [CAL matris {1,2,4}].
const K_QD: u32 = 2;
/// [TUR-3b D-2/3] DrainRtprop dwell sınırları (ack_epoch biriminde — kanal-saatli, ZM-2) [CAL].
const DRAIN_RTPROP_MIN_DWELL_EPOCH: u32 = 1;
const DRAIN_RTPROP_MAX_DWELL_EPOCH: u32 = 3;
/// [TUR-3b] Startup-Drain max-dwell (ack_epoch biriminde; liveness-yedeği) [CAL {2,3,4}].
const DRAIN_MAX_DWELL_EPOCH: u32 = 3;
/// [TUR-3c D-2/4] ProbeUp sözleşme-sabitleri [CAL, golden'a girmez]. SAAT-SINIFI (ZM-3):
/// periyot canlı-ack_epoch (Steady, gain=1 → kirlenmez); dwell/volume GİRİŞ-DONUK epoch/btlbw
/// (öz-referans tuzağı workflow'la bulundu: canlı-epoch probe'un kendi gain'iyle kısalır,
/// kuyruk-srtt'siyle UZAR — pozitif geri-besleme; srtt-clock-v1 §2.1 tuzağının kardeşi).
/// [TUR-7 β2, cift-kilit r3 §4] ARQ token-payı [CAL {4,8}]: pending>0 ∧ RecoverArq iken her
/// K_ARQ_SHARE'inci schedule-grant'i ARQ-ÖNCE kullanır. Gerekçe (ölçülü, §0.24-r3): DATA-önce
/// ve tek-birim-burst-cap altında ARQ-payı yapısal 0'dı (4 düşük-koşuda 120s retx=0 — BOŞ-NÖBET);
/// cap-tabanı seçeneği doğrulamada yapısal-ölü (token 2-birime hiç birikmiyor). DATA-önce
/// ölçülü-kararı KORUNUR (pay sınırlı, sayaç-tabanlı — saat YOK).
const K_ARQ_SHARE: u32 = 4;

const K_PROBE_PERIOD_EPOCH: u32 = 4; // [CAL {2,4,8}]
const PROBE_MAX_DWELL_EPOCH: u32 = 4; // ×epoch_entry [onaylı placeholder]
const PROBE_VOLUME_EPOCH: u32 = 2; // volume = btlbw_entry × 2×epoch_entry [onaylı]
const PROBE_SUCCESS_EPS_PERMILLE: u64 = 25; // btlbw_entry×1.025 [CAL]
const PROBE_QD_ABORT_PERMILLE: u64 = 500; // onaylı (giriş-histerezisi de bu eşik)
/// [TUR-3b commit-C, 3a.5-devri] ack_confidence sessizlik-eşiği EAI-tabanı [CAL {1,2,3}]:
/// eşik = max(3×srtt, K_CONF_EAI×EAI) — srtt_t hızlı-düşüşünün sahte-LOW-conf riski kapanır (ZM-4).
/// Karar-a eşleşmesi: receiver tail-muafiyeti (K_TAIL) ile ÇİFT — liveness ağır-yol olmaktan çıkar.
const K_CONF_EAI: u32 = 2;
/// [TUR-3a.5 commit-β] Kuantum-döngü yakınsama-böleni: T = rtprop / K_CONV [CAL matris {2,4,8}].
/// T=rtprop = 3a-sonrası tek DÜRÜST saat (ack_epoch öz-referans tuzağı reddedildi — srtt-clock-v1 §2.1).
const K_CONV: u32 = 4;
/// [Y3-sim] delivered-EAI cadence-hesabı tipik birim-boyu.
const UNIT_SIZE_HINT: u64 = 267;
/// [TUR-2b D-1/4] Tombstone kayıt tavanı [CAL] — SR-AUDIT-001 bounded; en-eski-önce eviction +
/// audit. 4-decade doluş-doğrulaması not D-1/4 (0.25kbps ≈ 11 kayıt; ≲Gbps'te ~450 ≪ 4096).
const TOMBSTONE_MAX: usize = 4096;
/// §3.3-B geometrik kopya: ilk kopya = k_base × RTT_floor; RTT_floor ≈ (unit+sack)/floor_bw.
/// k_base = 3/2 (>1 → temiz kanalda ilk SACK kopyayı ateşlemeden iptal eder). [CAL PLACEHOLDER]
const PC_SACK_WIRE: u64 = 65;
const PC_KBASE_NUM: u64 = 3;
const PC_KBASE_DEN: u64 = 2;
const PC_COPY_MULTIPLIER: u64 = 2;
/// [R3/ADIM-6] Kanal-türevli burst penceresi [CAL]: burst_cap = clamp(rate·BURST_WINDOW, 1 birim, ~2×MTU).
/// Bant-kör 2×MTU=2400B sabitini (kbit-domain'i floodlayan) kanal-zamanı penceresine çevirir → sub-kbps'te
/// 1-birim (backlog-spike yok), hızlı kanalda micro-burst. ~500kbps'te 40ms ≈ 2×MTU (§13 "1-2×MTU").
const BURST_WINDOW_NS: u64 = 40_000_000;

/// Sender state machine (§3). Tüm bileşen state'ini tutar; `on_event` ile sürülür.
pub struct Sender {
    state: SenderState,

    // ── ölçüm (§5/§6/§7) ──
    loss: ThresholdLossDetector,
    rtt: RttEstimator,
    bw_sampler: DeliveryRateSampler,
    btlbw: BtlBwFilter,

    // ── karar (§8.2/§8.4/§8.5/§11) ──
    fuzzy: DeterministicFuzzyController,
    src_cfg: SrcConfig,
    budget_cfg: BudgetConfig,
    emergency: HysteresisController,
    emergency_cfg: EmergencyConfig,

    // ── icra (§9/§11/§13) ──
    pacer: TokenBucket,
    arq: DeadlineArqScheduler,

    // ── durum sayaçları ──
    next_seq: u64,
    inflight_bytes: u64,
    inflight: BTreeMap<u64, InflightPacket>,
    tx_queue: VecDeque<OpaqueUnit>,
    pending_lost: VecDeque<LostRec>, // §5 kayıp seq'ler → RECOVER_ARQ retransmit / RECOVER_FWD dup
    dup_queue: Vec<DupRec>,          // §11 zamanlanmış forward-recovery kopyaları
    last_now_ns: u64,

    // ── FEC-emisyon (DR-S5-1; koşullu — guard-OFF'ta hot-path byte-değişmez) ──
    fec_mode: FecActuationMode, // Commit-G TEK-otorite (ayrı-bool-YOK); Manual{false}=DR-S5-1-default
    last_latch: Option<FecLatchResult>,
    latch_event_index: u64,
    no_eligible_decision_count: u64,
    decision_event_mismatch_count: u64,
    no_rule_zero_count: u64,
    manual_toggle_rejected_count: u64,
    parity_enqueued_total: u64,
    parity_emitted_total: u64,
    token_denied_r_gauge: u64, // Commit-J-rename (eski: budget_denied_r_gauge) — §8.1-exact-tanım
    last_transition_event_index: u64, // Commit-I
    transition_applied_count: u64, // Commit-I: saturating
    pending_fec: VecDeque<PendingFecParity>, // engine FEC-encode-sonrası enqueue; sender pace-eder

    // ── send-record ↔ ACK-eşleme (DR-S5-3; SALT-LOGICAL — BtlBw/delivered'e BESLENMEZ) ──
    sent_logical: BTreeMap<u64, SrLogicalRef>, // wire-seq → logical (FEC-parity plan_fec_parity-içi; DATA engine-post-emit register_sent_logical — DR-S5-6)
    confirmed_logical: VecDeque<(u64, SrLogicalRef, ConfirmSource)>, // teyit-logical (WireAck|Recovered — DR-S5-4); BOUNDED
    diag_recovered_cancels: u64, // DR-S5-4: RECOVERED ile iptal-edilen bekleyen-retransmit sayısı (K2/M1-sinyali)
    diag_sent_logical_evicted: u64, // DR-S5-6: cap-eviction sayısı (M1-guard: hücrede 0-olmalı)
    diag_confirmed_logical_miss: u64, // DR-S5-6: ACK-teyidi kayıtsız-seq'e geldi (M1'de tüm-emisyon-register'lıyken = eviction-artefaktı)

    // ── ack_confidence (RECOVER_FWD tetiği): sessizlik-tabanlı ──
    last_ack_ns: Option<u64>,
    first_send_ns: Option<u64>,

    // ── [TUR-3b] DrainRtprop sözleşme-durumu (D-2/3: giriş=pencere-doluşu; exit-invariant) ──
    drain_rtprop_entry_ns: Option<u64>,
    drain_rtprop_qualifying: bool, // girişten beri ≥1 rtt-örneği (qualifying_min_rtt)
    drain_entry_ns: Option<u64>,   // [TUR-3b] Startup-Drain giriş anı (max-dwell yedeği)
    // ── [TUR-3c] ProbeUp sözleşme-durumu (giriş-DONUK snapshot'lar — öz-referans önlenir) ──
    probe_entry_ns: Option<u64>,
    probe_epoch_entry_ns: u64,
    probe_btlbw_entry: u64,
    probe_delivered_entry: u64,
    probe_loss_attributed: bool, // probe-atıflı kayıp-ilanı (send_ns ≥ entry; queue_losses işaretler)
    last_probe_end_ns: u64,
    diag_probe_success: u64,
    diag_probe_abort: u64,
    diag_probe_timeout: u64,
    diag_probe_preempt: u64, // 4. çıkış-türü (workflow bulgusu: preemption muhasebesiz kalamaz)

    // ── STARTUP plato tespiti ──
    startup_last_btlbw: u64,
    startup_stable_rounds: u32,
    // [TUR-6 plateau-fix, §16.2 yarım-fix kapanışı] DELIVERED-ANCHOR round (BBR-standart):
    // round-başında işaretlenen next_seq'ten SONRA gönderilen bir birimin İLK ACK'i = 1 round.
    // Round-tanımı SACK-arz-yoğunluğundan BAĞIMSIZ (TUR-5 zengin-arzı ack-OLAY-sayacını
    // kaydırıp Startup'ı 24.5→37.5s'e uzatmıştı → temiz-1k 44-regresyonu; ölçüldü §0.24).
    startup_round_marker_seq: u64,
    startup_round_complete: bool,
    // [TUR-7 β1, cift-kilit-v1 r3 + S1-onayı] base_phase ∈ {Startup(true), Steady(false)}:
    // recovery-overlay'in pacing-gain KAYNAĞI. INV-RATE-1-tamamlama: faz yalnız KANAL-ÖLÇÜMÜ
    // olaylarıyla değişir (plato); kayıp-ilanı faz-gain'ine DOKUNAMAZ (recovery = NE-gönderilir
    // overlay'i, NASIL-hızlı değil). Kilit-golden: inv_rate_1_loss_declaration_preserves_phase_gain.
    base_is_startup: bool,
    plateau_reached: bool, // [TUR-7 β1] plato-sinyali (base'ten AYRI: taze-giriş de base=false)
    arq_share_tick: u32,   // [TUR-7 β2] grant-sayacı (K_ARQ_SHARE payı)
    /// [TUR-A ölçüm-#1, TEK DEĞİŞKEN] Son plato-değerlendirmesinden bu yana YENİ bw örneği geldi mi.
    /// Plato round'u yalnız yeni örnekte sayılır (pace-tick başına DEĞİL) — saat-sınıfı: event-count →
    /// bw-sample. (Eski: her on_event çağrısı "round" sayılıyordu → 1ms tick'te 3 round = 3ms'de STEADY.)
    bw_sampled_since_round: bool,

    // ── [TUR-A teşhis] loss-atıflama sayaçları (SALT-GÖZLEM) ──
    diag_loss_pkt: u64,
    diag_loss_time: u64,
    // ── [TUR-1x teşhis] ack_delay/RTT-guard sayaçları (SALT-GÖZLEM) ──
    diag_rtt_rejected: u64, // guard'a takılan sample sayısı (underflow / < MIN_RTT_FLOOR)
    diag_ack_delay_max_us: u64, // görülen en büyük (decoded) ack_delay — V2 sadakati kanıtı
    // ── [TUR-2a] adaptif reorder-toleransı + un-loss ──
    reorder: ReorderTracker, // §9.3 spurious→note_spurious kablosu; packet-thresh tüketimi
    retx_cfg: RetxConfig,    // reorder_thresh(ρ, K) parametreleri (arq ile aynı fixture)
    diag_unloss: u64,        // teşhis: geri-alınan (Spurious) pending-iptal sayısı

    // ── [TUR-2b] tombstone (geç-ACK eşleştirme; bounded, SR-AUDIT-001) ──
    tombstones: BTreeMap<u64, TombstoneRec>,
    tombstone_order: VecDeque<u64>, // ekleme sırası ≈ en-eski-TTL-önce (eviction kaynağı)
    tombstone_evicted: u64,         // audit: kapasite-eviction
    diag_tomb_hits: u64,            // geç-ACK tombstone'a çarptı (SALT-GÖZLEM)
    diag_bw_recovered: u64,         // kol-1'den kurtarılan BW-örneği sayısı
    diag_ambiguous_skipped: u64, // [TUR-2c resmî] kol-2 örnek-atlaması (sample_skipped_ambiguous)
    diag_delivered_credited: u64, // [TUR-2c] kol-2 delivered-kredisi (byte)
    diag_qd_no_evidence: u64,    // [TUR-3a D-3/1] rtprop-yokken qd_ratio=0 sayısı
    diag_fuzzy_no_rule: u64,     // [TUR-4] sıfır-ağırlık fallback ateşleme sayısı

    // ── gözlemlenebilir son kararlar ──
    last_decision: FusionDecision,
    // Commit-D: canonical-girdi-anlık-görüntüsü (fuzzy_evaluate-kopyası; salt-teşhis, davranışsız).
    last_fusion_snapshot: Option<FusionInputSnapshot>,
    fusion_event_index: u64,
    last_recovery: RecoveryAction,
}

impl Sender {
    /// Fixture config ile (KALİBRE DEĞİL — golden/motor testi; gerçek değerler netem'den).
    /// DR-1e-v2: ortak-internal-ctor'a delege; fixture VALIDATE-EDİLMEDEN geçirilir (mevcut-
    /// davranış-birebir; panik-yolu-YOK — fixture-geçerliliği ayrı-test-time-invariant:
    /// `tv_fusion::fixture_validates`).
    #[must_use]
    pub fn new() -> Self {
        Self::new_internal(ValidatedFuzzyConfig::trusted_fixture())
    }

    /// DR-1e-v2: dışarıdan-verilen fuzzy-config ile (validate-İÇERİDE; typed-hata — panik-yok).
    /// Kalibrasyon = config-değişikliği (kod-değil). Yalnız fuzzy-config (god-config-yok).
    ///
    /// # Errors
    /// [`FuzzyConfigError`] — yapısal/spec-invariant ihlali (fusion.rs §13.4-tablosu).
    pub fn try_with_fuzzy_config(config: FuzzyConfig) -> Result<Self, FuzzyConfigError> {
        let validated = config.validate()?;
        Ok(Self::new_internal(validated))
    }

    /// Ortak-internal-ctor (DR-1e-v2): her-iki-public-ctor buraya-delege. Motor-matematiği/
    /// diğer-fixture'lar DEĞİŞMEZ; yalnız fuzzy-config-parametrik.
    /// Commit-F-invariant: TEK-gerçek-ctor YALNIZ `ValidatedFuzzyConfig` kabul-eder (raw-`FuzzyConfig`-
    /// overload-YOK) — "Sender-yalnız-validated/trusted-config'le-kurulur" tip-düzeyi-sınırı.
    fn new_internal(fuzzy_cfg: ValidatedFuzzyConfig) -> Self {
        let budget_cfg = BudgetConfig::fixture();
        Self {
            state: SenderState::Idle,
            loss: ThresholdLossDetector::new(),
            rtt: RttEstimator::new(),
            bw_sampler: DeliveryRateSampler::new(),
            btlbw: BtlBwFilter::new(),
            fuzzy: DeterministicFuzzyController::new(fuzzy_cfg.into_inner()),
            src_cfg: SrcConfig::default(),
            budget_cfg,
            emergency: HysteresisController::new(HysteresisConfig::fixture(), EmergencyLevel::Arq),
            emergency_cfg: EmergencyConfig::fixture(),
            pacer: TokenBucket::new(budget_cfg.min_rate_bps, DEFAULT_BURST_CAP),
            arq: DeadlineArqScheduler::new(RetxConfig::fixture()),
            next_seq: 0,
            inflight_bytes: 0,
            inflight: BTreeMap::new(),
            tx_queue: VecDeque::new(),
            pending_lost: VecDeque::new(),
            dup_queue: Vec::new(),
            last_now_ns: 0,
            fec_mode: FecActuationMode::Manual { enabled: false }, // default: mevcut-davranış-birebir
            last_latch: None,
            latch_event_index: 0,
            no_eligible_decision_count: 0,
            decision_event_mismatch_count: 0,
            no_rule_zero_count: 0,
            manual_toggle_rejected_count: 0,
            parity_enqueued_total: 0,
            parity_emitted_total: 0,
            token_denied_r_gauge: 0,
            last_transition_event_index: 0,
            transition_applied_count: 0,
            pending_fec: VecDeque::new(),
            sent_logical: BTreeMap::new(), // DR-S5-3
            confirmed_logical: VecDeque::new(),
            diag_recovered_cancels: 0,      // DR-S5-4
            diag_sent_logical_evicted: 0,   // DR-S5-6
            diag_confirmed_logical_miss: 0, // DR-S5-6
            last_ack_ns: None,
            first_send_ns: None,
            drain_rtprop_entry_ns: None,
            drain_rtprop_qualifying: false,
            drain_entry_ns: None,
            probe_entry_ns: None,
            probe_epoch_entry_ns: 0,
            probe_btlbw_entry: 0,
            probe_delivered_entry: 0,
            probe_loss_attributed: false,
            last_probe_end_ns: 0,
            diag_probe_success: 0,
            diag_probe_abort: 0,
            diag_probe_timeout: 0,
            diag_probe_preempt: 0,
            startup_last_btlbw: 0,
            startup_stable_rounds: 0,
            startup_round_marker_seq: 0,
            startup_round_complete: false,
            base_is_startup: false,
            plateau_reached: false,
            arq_share_tick: 0,
            bw_sampled_since_round: false,
            diag_loss_pkt: 0,
            diag_loss_time: 0,
            diag_rtt_rejected: 0,
            diag_ack_delay_max_us: 0,
            reorder: ReorderTracker::new(),
            retx_cfg: RetxConfig::fixture(),
            diag_unloss: 0,
            tombstones: BTreeMap::new(),
            tombstone_order: VecDeque::new(),
            tombstone_evicted: 0,
            diag_tomb_hits: 0,
            diag_bw_recovered: 0,
            diag_ambiguous_skipped: 0,
            diag_delivered_credited: 0,
            diag_qd_no_evidence: 0,
            diag_fuzzy_no_rule: 0,
            last_fusion_snapshot: None,
            fusion_event_index: 0,
            last_decision: FusionDecision {
                fec_parity_ratio_permille: 0,
                arq_aggressiveness_permille: 0,
                send_rate_gain_permille: 1000,
                compress: false,
                mode: Mode::Normal,
                no_rule: false,
            },
            last_recovery: RecoveryAction::Arq,
        }
    }

    /// Ana döngü (§3): olay işle → FuzzyEvaluate → state → send_rate → UpdatePacing → ScheduleTransmit.
    pub fn on_event(&mut self, event: Event, now_ns: u64) -> Vec<TransmitDecision> {
        // DR-S5-4 SAF-LOGICAL erken-dönüş: RECOVERED-control pacer-saatine/fuzzy/rate/schedule'a
        // HİÇ girmez (emit-yok, kredi-yok, saat-ilerletme-yok — N8-bit-özdeşliğin yapısal-tabanı).
        if let Event::RecoveredControl(info) = &event {
            self.on_recovered_control(info);
            return Vec::new();
        }

        let dt = Duration::from_nanos(now_ns.saturating_sub(self.last_now_ns));
        self.pacer.refill(dt);
        self.last_now_ns = now_ns;

        match event {
            Event::AppData(unit) => {
                self.tx_queue.push_back(unit);
                if self.state == SenderState::Idle {
                    // §3.3 cold-start YALNIZ ilk sefer (hiç ölçüm yok). Ölçüm varsa (rtt.srtt Some) önsel
                    // zaten tahliye edilmiş → Idle→Startup (normal resume), PreConverge'e geri DÖNÜLMEZ.
                    self.state = if self.rtt.srtt().is_none() {
                        SenderState::PreConverge
                    } else {
                        SenderState::Startup
                    };
                }
            }
            Event::AckReceived(ack) => self.on_ack_received(&ack, now_ns),
            Event::TimerFired(timer) => self.on_timer(timer, now_ns),
            // Yukarıda erken-dönüşle işlendi (defensive no-op; exhaustiveness için).
            Event::RecoveredControl(_) => {}
        }

        let decision = self.fuzzy_evaluate(now_ns);
        self.last_decision = decision;
        let prev_state = self.state;
        self.update_state(&decision, now_ns);
        let send_rate = self.compute_send_rate(&decision);
        self.pacer.set_rate(send_rate);
        // [R3/ADIM-6] KANAL-TÜREVLİ burst cap: max(1 birim, rate·pencere), üstten ~2×MTU. Bant-kör 2400B
        // sabiti yerine → PreConverge floor'da 1-birim (self-congestion/backlog-spike yok), Startup+'ta
        // ölçülen bw'ye orantılı micro-burst. front yoksa MTU (güvenli 1-paket tabanı).
        let front_size = self.tx_queue.front().map_or(DEFAULT_MTU, |u| u.size);
        let window_bytes = send_rate.saturating_mul(BURST_WINDOW_NS) / 1_000_000_000;
        let burst_hi = front_size.max(2 * DEFAULT_MTU);
        self.pacer
            .set_burst_cap(window_bytes.clamp(front_size, burst_hi));
        if prev_state == SenderState::PreConverge && self.state != SenderState::PreConverge {
            // §3.3-D cold-start çıkışı: kanal-türevli micro-burst grant (STARTUP probing; kuyruk kanal-uygun
            // küçük burst'le boşalır — bu birimler STARTUP'ta emit → §3.3-B geometrik kopya ALMAZ).
            self.pacer.fill_to_cap();
        }

        self.schedule_transmit(now_ns)
    }

    /// §3.2 OnAckReceived: inflight boşalt + RTT/BW/loss güncelle.
    fn on_ack_received(&mut self, ack: &AckInfo, now_ns: u64) {
        if ack.acked_seqs.is_empty() {
            return;
        }
        self.last_ack_ns = Some(now_ns);

        // [TUR-3a D-2/1] Kanal-türevi estimator pencereleri — örnekler işlenmeden ÖNCE, bir
        // önceki srtt'yle yazılır (deterministik; 10s [STD] tabanı setter'larda).
        {
            let srtt_now = self.rtt.srtt().unwrap_or(INITIAL_RTT);
            let ack_epoch = srtt_now.max(self.expected_ack_interval());
            // [TUR-D1 EK-3 AÇIK-BORÇ] W=K_BW×ack_epoch'un srtt-dalı daralma-rejiminde ANTİ-FİX:
            // srtt-eğimi > 1/K_BW iken cutoff GERİLER → bayat-max ölümsüz (DV-3 ölçülü 0.92 s/s).
            // Shrink-reset SEMPTOMU keser, KÖK DURUR — W-türetim/M3-decoupling turunda kapanır;
            // "dedektör-var-diye-W-fix-gereksiz" SANILMAZ (m45 §4-S1).
            // [Y3 KALICI — K6R-mühürlü] W_BTLBW delivered-EAI-türetimi: cadence = 2×unit/
            // SON-TAM-örnek-delivered (last_full_bw; K3-pinleri) + return-prior(2.097s [CAL,
            // E-FULL-borcu]). srtt-runaway'den DECOUPLED (kuyruk-değişmez) → bayat-max çok-adımlı-
            // expire (adım≈W; uzun-daralmada kanal-bandına yakınsar — netem-mühür: 4114→308).
            // Kopma-frozen-pencere sınır-vakası BEYANLI (TUR-S kopma-tespiti tamamlar; mini-D0 Ş3).
            let w_epoch = {
                let dbw = self.btlbw.last_sample_bw().max(1);
                let term1 = Duration::from_nanos(2 * UNIT_SIZE_HINT * 1_000_000_000 / dbw);
                (term1 + Duration::from_nanos(2_097_000_000)).max(INITIAL_RTT)
            };
            let _ = ack_epoch; // W_loss-kolu (fuzzy_evaluate) kendi ack_epoch'unu hesaplar (S2: zararsız)
            self.btlbw.set_window(w_epoch * K_BW);
            self.rtt.set_rtprop_window(srtt_now * K_RTPROP);
            // [S-FAZ-0, K8-fix] W_qd srtt-dalından delivered-EAI-saatine (TEK-TOKEN: ack_epoch→
            // w_epoch). Kök: W_qd=2×srtt flood'da flood-öncesine-uzanır → qd≡0 → fuzzy-soft-drain
            // + ProbeUp-abort ÖLÜ (K8 secondary-safety-path-deadness). w_epoch kuyruk-değişmez →
            // DESEN-1'de qd onset+~8s canlanır; sağlıklı-rejimde DEĞER-ÖZDEŞ (rate≈dbw → EAI≡w_epoch).
            // Little bit-değişmez (rate.rs girdisi srtt; qd-tüketicileri yalnız fusion-sr1 + ProbeUp).
            self.rtt.set_qd_window(w_epoch * K_QD); // [3a.5-α] rtprop-penceresinden sonra (≤ invariant)
                                                    // [3a.5-β] kuantum-döngü T'si — yalnız rtprop varken (PreConverge'de None → n=1).
            if let Some(rtp) = self.rtt.rtprop() {
                self.rtt.set_conv_interval(rtp / K_CONV);
            }
        };

        // RTT: en yüksek yeni-acked ORİJİNAL (DATA) paketten örnekle (Karn: retransmit/dup'tan DEĞİL).
        let mut rtt_send_ns: Option<u64> = None;
        let mut largest = 0u64;
        let mut acked_units: Vec<u64> = Vec::new();
        for &seq in &ack.acked_seqs {
            // [TUR-6] delivered-anchor round: marker-sonrası gönderilen seq'in ilk ACK'i.
            if seq >= self.startup_round_marker_seq {
                self.startup_round_complete = true;
            }
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                acked_units.push(p.unit.id); // §3.3-B stop koşulu: teyit edilen birimin kopyaları iptal
                                             // DR-S5-3 [SALT-LOGICAL — BtlBw'ye DEĞMEZ]: wire-ACK hangi-logical-paketi teyit-etti?
                                             // Ayrı-defter (bounded); bw_sampler/btlbw'yi OKUMAZ/YAZMAZ (N8: recovered≠ACK tabanı).
                self.record_confirmed_logical(seq);
                let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample);
                self.btlbw.update(sample, now_ns, sample.delivered > 0);
                self.bw_sampled_since_round = true; // [TUR-A ölçüm-#1] plato round'u bw-örneğiyle sayılır
                if p.origin == TransmitKind::Data && (rtt_send_ns.is_none() || seq >= largest) {
                    largest = seq;
                    rtt_send_ns = Some(p.send_ns);
                }
            } else if let Some(t) = self.tombstones.remove(&seq) {
                // [TUR-2b D-1/4] GEÇ-ACK tombstone'a çarptı: kayıt korunmuştu → örnekler
                // KURTARILIR (eski davranış: kayıt düşmüştü → sansür). Süresi geçmiş kayıt
                // prune edilmişse buraya düşmez (Expired/Unknown — örneksiz).
                self.diag_tomb_hits += 1;
                acked_units.push(t.unit.id); // teslim teyidi → bekleyen kopyaları iptal et
                if t.retx_count == 0 {
                    // kol-1 (tek gönderim, belirsizlik yok): BW-örneği NORMAL üretilir.
                    let sample = self.bw_sampler.on_ack(now_ns, t.bw_sample);
                    self.btlbw.update(sample, now_ns, sample.delivered > 0);
                    self.bw_sampled_since_round = true;
                    self.diag_bw_recovered += 1;
                    if t.origin == TransmitKind::Data && (rtt_send_ns.is_none() || seq >= largest) {
                        largest = seq;
                        rtt_send_ns = Some(t.send_ns);
                    }
                } else {
                    // [TUR-2c D-1/3 kol-2] Superseded iletimin geç-ACK'i: RTT YOK + rate-örneği
                    // YOK (muhafazakâr Karn); ama DELIVERED-KREDİSİ VAR — byte'lar wire'ı
                    // gerçekten geçti, sampler kümülatifine girer (gelecek örnek-delta'larına
                    // dürüst yansır). Not: per-transmission seq tasarımında klasik belirsizlik
                    // zayıftır (seq iletimi tekil tanımlar); kural yine muhafazakâr tutuldu.
                    self.bw_sampler.credit_delivered(now_ns, t.unit.size);
                    self.diag_delivered_credited += t.unit.size;
                    self.diag_ambiguous_skipped += 1;
                    // [TUR-2c review-fix, §9.3] KANONİK spurious-retransmit teyidi tam bu vaka
                    // ("retransmit'ten SONRA orijinal ACK geldi") → reorder-toleransı yükselir.
                    // Un-loss yolu (2a) yalnız retx-ÖNCESİ geç-ACK'i yakalıyordu; kol-2 kabloyu
                    // tamamlar. Ledger zaten Spurious'a çevirir (lppm dürüst).
                    let la = self.loss.largest_acked().unwrap_or(seq);
                    self.reorder
                        .note_spurious(u32::try_from(la.saturating_sub(seq)).unwrap_or(u32::MAX));
                }
            }
        }

        // loss tespiti (§5). loss = RECOVERY sinyali (INV-RATE-1: rate'e DEĞMEZ).
        // §3.3-C/(1) yasak-1: PRE_CONVERGE'de loss İLANI YOK. §3.3-D: loss kilidi İLK GEÇERLİ SACK'le
        // (rtt.srtt Some) AÇILIR → o SACK'ten itibaren loss işlenir. Detector her zaman sync kalır.
        // [TUR-2a §9.3] adaptif packet-threshold: spurious gördükçe reorder-toleransı yükselir.
        self.loss
            .set_packet_threshold(u64::from(self.reorder.reorder_thresh(&self.retx_cfg)));
        let lost = self.loss.on_ack(&ack.acked_seqs, now_ns);

        // [TUR-2a D-1/3] UN-LOSS: kayıp-ilan-edilmiş seq'e ACK geldi → hipotez çürüdü.
        // pending-retransmit İPTAL + kopyalar iptal + note_spurious kablosu + kol-1 RTT adayı.
        let unlossed = self.loss.take_unlossed();
        let mut unloss_rtt: Option<(u64, u64)> = None; // (seq, send_ns)
        if !unlossed.is_empty() {
            // Gözlenen-reorder mesafesi görülen EN YÜKSEK acked'e göredir (bu geç-ACK'in
            // kendi max'ı değil — kayıp-şüphesini yaratan önceki yüksek ACK'ler dahil).
            let la = self.loss.largest_acked().unwrap_or(0);
            let mut kept = VecDeque::with_capacity(self.pending_lost.len());
            for rec in self.pending_lost.drain(..) {
                if !unlossed.contains(&rec.seq) {
                    kept.push_back(rec);
                    continue;
                }
                self.diag_unloss += 1;
                acked_units.push(rec.unit.id); // kopyaları aşağıdaki ortak iptale kat
                                               // §9.3: gözlenen reorder mesafesiyle toleransı yükselt (küçülmez).
                self.reorder
                    .note_spurious(u32::try_from(la.saturating_sub(rec.seq)).unwrap_or(u32::MAX));
                // kol-1: retransmit HENÜZ gönderilmedi → orijinal RTT örneği GEÇERLİ.
                if rec.retx_count == 0 && unloss_rtt.is_none_or(|(s, _)| rec.seq >= s) {
                    unloss_rtt = Some((rec.seq, rec.send_ns));
                }
            }
            self.pending_lost = kept;
        }
        // RTT adayı birleşimi: en yüksek yeni-acked orijinal gönderim (inflight ∪ un-loss kol-1).
        if let Some((useq, usend)) = unloss_rtt {
            if rtt_send_ns.is_none() || useq >= largest {
                rtt_send_ns = Some(usend);
            }
        }

        if let Some(send_ns) = rtt_send_ns {
            let raw = Duration::from_nanos(now_ns.saturating_sub(send_ns));
            self.diag_ack_delay_max_us = self
                .diag_ack_delay_max_us
                .max(ack.ack_delay.as_micros() as u64);
            // [TUR-1x] min-rtt guard (RFC 9002 §5.3): düzeltme güvensizse RAW kullanılır —
            // örnek ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı).
            let (sample, corrected) = apply_ack_delay(raw, ack.ack_delay, self.rtt.rtprop());
            if !corrected {
                self.diag_rtt_rejected += 1; // §7.3 ack_delay_untrusted (düzeltme reddi; raw işlendi)
            }
            self.rtt.observe(now_ns, sample);
            // [TUR-3b D-2/3] DrainRtprop dwell'i içinde gelen rtt-örneği = qualifying.
            if self.state == SenderState::DrainRtprop {
                self.drain_rtprop_qualifying = true;
            }
        }

        // §3.3-B stop koşulu: teyit edilen birimlerin bekleyen geometrik kopyalarını İPTAL et.
        if !acked_units.is_empty() {
            self.dup_queue.retain(|d| !acked_units.contains(&d.unit.id));
        }

        if self.state != SenderState::PreConverge || self.rtt.srtt().is_some() {
            let before = self.pending_lost.len();
            self.queue_losses(&lost, now_ns);
            // [TUR-A teşhis] loss-atıflama: packet-threshold (§5a) kaynaklı kuyruklanan kayıp sayısı.
            self.diag_loss_pkt += (self.pending_lost.len() - before) as u64;
        }
    }

    /// §12 timer handler (RTprop probe → DRAIN_RTPROP; PTO → zaman-eşiği loss).
    fn on_timer(&mut self, timer: Timer, now_ns: u64) {
        match timer {
            Timer::RtpropProbe => {
                // [TUR-3b D-2/3] GİRİŞ = rtprop-penceresi-DOLUŞU (min-örnek yaşı ≥ W/2 [CAL]);
                // 200ms-sabit-tick tetikçiliği CANCELLED — tazelik kanal-saatli (ZM-2).
                if self.state == SenderState::Steady {
                    let stale = self
                        .rtt
                        .rtprop_age_ns(now_ns)
                        .is_some_and(|age| age >= self.rtt.rtprop_window_ns() / 2);
                    if stale {
                        self.state = SenderState::DrainRtprop;
                        self.drain_rtprop_entry_ns = Some(now_ns);
                        self.drain_rtprop_qualifying = false;
                    }
                }
            }
            Timer::Pto => {
                // §3.3-C/(1,5) yasak: PRE_CONVERGE'de time-threshold loss-ilanı + PTO DONDURULUR
                // (srtt BOŞ → tanımsız; geometrik kopya §3.3-B yerini alır). schedule_transmit yine çalışır.
                if self.state != SenderState::PreConverge {
                    let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
                    let lost =
                        self.loss
                            .detect_time_losses(now_ns, srtt, self.cadence_loss_floor());
                    let before = self.pending_lost.len();
                    self.queue_losses(&lost, now_ns);
                    // [TUR-A teşhis] loss-atıflama: time-threshold (§5b) kaynaklı kuyruklanan kayıp.
                    self.diag_loss_time += (self.pending_lost.len() - before) as u64;
                }
                // [TUR-2b] tombstone TTL budaması: süresi dolan kayıt düşer — sonraki geç-ACK'i
                // Expired/Unknown (örneksiz). Ekleme-sırası ≈ TTL-sırası (T_loss yavaş değişir);
                // front'tan tarama deterministik + bounded.
                while let Some(&front) = self.tombstone_order.front() {
                    match self.tombstones.get(&front) {
                        None => {
                            self.tombstone_order.pop_front(); // hit/evict ile zaten düşmüş
                        }
                        Some(t) if t.expiry_ns <= now_ns => {
                            self.tombstones.remove(&front);
                            self.tombstone_order.pop_front();
                            // [commit-B §3.7] artık ACK'lenmeyecek → uçuş-sayısından düş.
                            self.bw_sampler.forget_in_flight();
                        }
                        Some(_) => break,
                    }
                }
            }
        }
    }

    /// [TUR-1b D-1/2] Cadence-farkında loss-eşiği TABANI = K_c × beklenen-ACK-aralığı (EAI).
    /// `EAI = ACK_FREQ_N × ort_birim / send_rate_fiili + sack_dönüş_serialization(önsel)`.
    /// Sender-yerel + deterministik (ADR-17). Saat-sınıfı (ZM-3): kanal-türevi (send_rate,
    /// inflight-ortalama birim) + worst-corner return-önseli [CAL — §3.3-E-FULL'de ölçülen
    /// return_bw ile değişir). Amaç (ZM-4): elapsed'in içindeki "SACK'i henüz gelmedi" süresi
    /// eşiğin saatinde TEMSİL edilsin — kanıtlı-ama-cadence yanlış-sınıflandırması yapısal biter.
    fn cadence_loss_floor(&self) -> Duration {
        Duration::from_nanos(dur_ns(self.expected_ack_interval()) * CADENCE_KC_NUM / CADENCE_KC_DEN)
    }

    /// [TUR-1b/2a] Beklenen-ACK-aralığı EAI (K_c'siz ham değer) — cadence-taban ve
    /// `ack_epoch = max(srtt, EAI)` (ledger W_loss penceresi, D-2/1 tanımı) bundan türer.
    fn expected_ack_interval(&self) -> Duration {
        let unit = if self.inflight.is_empty() {
            DEFAULT_MTU
        } else {
            (self.inflight_bytes / self.inflight.len() as u64).max(1)
        };
        let rate = self.pacer.rate().max(1);
        let eai_ns = EXPECTED_ACK_FREQ_N * unit * 1_000_000_000 / rate
            + EAI_SACK_WIRE_BYTES * 1_000_000_000 / EAI_RETURN_BW_PRIOR;
        Duration::from_nanos(eai_ns)
    }

    /// Kayıp seq'leri recovery kuyruğuna al. Duplicate-kökenli kayıp RE-QUEUE EDİLMEZ (redundancy'di).
    /// [TUR-2b D-1/4] ACTIVE-FLIGHT / TOMBSTONE ayrımı: inflight-muhasebesinden DÜŞER (gönderim
    /// durmaz) ama gönderim-kaydı SİLİNMEZ — geç-ACK eşleştirme için tombstone'a taşınır
    /// (bw_sample DAHİL → kol-1'de BW-örneği kurtarılabilir; eski davranış kaydı düşürüp
    /// geç-ACK örneklerini SANSÜRLÜYORDU — K-1 eviction-sansürü sınıfı).
    fn queue_losses(&mut self, lost: &[u64], now_ns: u64) {
        if lost.is_empty() {
            return;
        }
        // TTL = K_TOMB × T_loss(ilan-anı); T_loss = max(9/8·srtt, cadence-taban) — eşikle AYNI saat.
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let t_loss_ns = (dur_ns(srtt) * 9 / 8).max(dur_ns(self.cadence_loss_floor()));
        let expiry_ns = now_ns.saturating_add(t_loss_ns.saturating_mul(u64::from(K_TOMB)));
        for &seq in lost {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                // [TUR-3c workflow-bulgusu (a)] loss-abort YALNIZ probe-ATIFLI kayıpla:
                // send_ns ≥ probe-girişi (probe-öncesi trafiğin geç-Confirmed'i probe'u öldürmesin;
                // rastgele-kayıp vetosu da böyle daralır — INV-RATE-1 gerilimi düşer).
                if let Some(entry) = self.probe_entry_ns {
                    if p.send_ns >= entry {
                        self.probe_loss_attributed = true;
                    }
                }
                if p.origin == TransmitKind::Duplicate {
                    // [commit-B §3.7] Dup-kökenli kayıp tombstonesuz düşer → uçuş-sayısı burada.
                    self.bw_sampler.forget_in_flight();
                }
                if p.origin != TransmitKind::Duplicate {
                    self.pending_lost.push_back(LostRec {
                        seq,
                        unit: p.unit,
                        deadline_ns: now_ns,
                        retx_count: p.retx_count,
                        send_ns: p.send_ns, // [TUR-2a] un-loss kol-1 RTT için orijinal damga
                    });
                    // [TUR-2b] tombstone: bounded, en-eski-önce eviction + audit.
                    while self.tombstones.len() >= TOMBSTONE_MAX {
                        match self.tombstone_order.pop_front() {
                            Some(old) => {
                                if self.tombstones.remove(&old).is_some() {
                                    self.tombstone_evicted += 1; // geç-ACK'i artık Expired/Unknown
                                    self.bw_sampler.forget_in_flight(); // [commit-B §3.7]
                                }
                            }
                            None => break,
                        }
                    }
                    self.tombstone_order.push_back(seq);
                    self.tombstones.insert(
                        seq,
                        TombstoneRec {
                            unit: p.unit,
                            send_ns: p.send_ns,
                            bw_sample: p.bw_sample,
                            retx_count: p.retx_count,
                            origin: p.origin,
                            expiry_ns,
                        },
                    );
                }
            }
        }
    }

    /// FuzzyEvaluate (§8.4/§15): ölçümlerden FusionInputs kur → controller kararı.
    /// [TUR-2a D-1/5] loss girdisi LEDGER-penceresinden türer (lifetime-ratchet ÖLDÜ):
    /// önce ledger_tick (W_loss = K_wl×ack_epoch; confirm = cadence-taban = geç-SACK ufku),
    /// sonra `loss_windowed_ppm` — Spurious sayılmaz, pencere-dışı decay eder.
    fn fuzzy_evaluate(&mut self, now_ns: u64) -> FusionDecision {
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let ack_epoch = srtt.max(self.expected_ack_interval()); // D-2/1 tanımı
        self.loss
            .ledger_tick(now_ns, ack_epoch * K_WL, self.cadence_loss_floor());
        let jitter_ppm = dur_ns(self.rtt.rttvar())
            .saturating_mul(1_000_000)
            .checked_div(dur_ns(srtt))
            .unwrap_or(0);
        // [TUR-3a önalım, D-3/1] qd_ratio = qd×1000/rtprop; rtprop yoksa 0 (kanıt yok →
        // boğma yok) + sayaç. Dürüst-rtprop'un zorunlu ikizi: mutlak-ns eşikler sub-kbps'te
        // fiziksel-taban qd'yi (serialization çift-yönü, saniyeler) kalıcı-HIGH okuyordu.
        let qd_ratio_permille = if self.rtt.rtprop().is_none_or(|r| r.is_zero()) {
            self.diag_qd_no_evidence += 1;
            0
        } else {
            self.qd_ratio_permille_now()
        };
        let inputs = FusionInputs {
            loss_ppm: self.loss.loss_windowed_ppm(),
            bandwidth_bps: self.btlbw.btlbw(),
            jitter_ppm,
            qd_ratio_permille,
            ack_confidence_permille: self.ack_confidence_permille(now_ns),
        };
        let d = self.fuzzy.decide(&inputs);
        // [TUR-4] sıfır-ağırlık fallback kanıt-sayacı (fallback çıktısı R1-baskın çıktıyla
        // bayt-özdeş — sayaçsız gözlemlenemez, workflow-bulgusu).
        if d.no_rule {
            self.diag_fuzzy_no_rule += 1;
        }
        // Commit-D: CANONICAL-snapshot — aynı-`inputs`-DEĞERİ kopyalanır (ikinci-hesap-yok).
        // HÜKÜM (Commit-F-daraltması): "Semantic non-regression demonstrated; the diagnostic path
        // adds one fixed-size Copy assignment with no allocation, I/O or formatting. Performance
        // neutrality has not been independently benchmarked." (benchmark 1a-blocker-DEĞİL.)
        self.fusion_event_index += 1;
        self.last_fusion_snapshot = Some(FusionInputSnapshot {
            loss_windowed_ppm: inputs.loss_ppm,
            bandwidth_bps: inputs.bandwidth_bps,
            jitter_ppm: inputs.jitter_ppm,
            qd_ratio_permille: inputs.qd_ratio_permille,
            ack_confidence_permille: inputs.ack_confidence_permille,
            no_rule: d.no_rule,
            event_index: self.fusion_event_index,
        });
        d
    }

    /// Commit-D: controller'ın-son-gördüğü girdiler (canonical; harness-örneklemesi dışarıdan).
    /// `None` = henüz-hiç-fuzzy_evaluate-koşmadı.
    #[must_use]
    pub fn diag_fusion_input_snapshot(&self) -> Option<FusionInputSnapshot> {
        self.last_fusion_snapshot
    }

    /// §3 state geçişleri. Öncelik: IDLE (iş yok) → RECOVER_FWD (ack_confidence LOW, her fazdan) →
    /// faz-içi (STARTUP plato / RECOVER_ARQ / STEADY).
    fn update_state(&mut self, decision: &FusionDecision, now_ns: u64) {
        let prev_state = self.state; // [TUR-3c] preemption-muhasebesi için
        let health = u32::try_from(self.ack_confidence_permille(now_ns)).unwrap_or(u32::MAX);
        let level = self.emergency.update(health, now_ns);
        self.last_recovery = emergency::action(level, Framing::Tiny, &self.emergency_cfg);

        let idle = self.tx_queue.is_empty()
            && self.inflight.is_empty()
            && self.pending_lost.is_empty()
            && self.dup_queue.is_empty();
        let emergency_active =
            level > EmergencyLevel::Arq || decision.mode == Mode::EmergencyForwardRedundancy;

        // [TUR-7 β1] PLATO BASE-ÜSTÜNDE: base==Startup iken round-değerlendirmesi HER durumda
        // (recovery-altında da) ilerler — ramp'in sonlanması kanal-ölçümüne bağlı kalır
        // (delivered-anchor round'lar recovery'de de tamamlanır; eternal-2.885 sınırlı).
        // Beyanlı-sınır (r3): plato recovery-altında gelirse çıkış Steady'ye (Drain o pencerede
        // atlanır — recovery gain'i base'te tuttuğundan kuyruk S4-B-ölçüsüyle sınırlı).
        if self.base_is_startup && self.startup_plateau() {
            self.base_is_startup = false;
            self.plateau_reached = true;
        }
        self.state = if idle {
            SenderState::Idle
        } else if self.state == SenderState::PreConverge {
            // §3.3-D kademeli tahliye: ilk geçerli SACK (RTT kilidi açıldı) → Startup; yoksa PreConverge.
            // emergency/ARQ/loss burada BYPASS edilir — recovery cold-start'ta DONDURULMUŞ (§3.3-C).
            if self.rtt.srtt().is_some() {
                SenderState::Startup
            } else {
                SenderState::PreConverge
            }
        } else if emergency_active {
            // ack_confidence LOW → forward-recovery (ACK-bağımsız, §11), her aktif fazdan (§3 AMENDMENT-1).
            SenderState::RecoverFwd
        } else if !self.pending_lost.is_empty() {
            // residual loss + ack_confidence OK → selective retransmit (§9), her aktif fazdan
            // (§3 AMENDMENT-1; RECOVER_FWD ile simetrik: loss STARTUP'ta olsa da recovery tıkanmamalı).
            SenderState::RecoverArq
        } else {
            match self.state {
                SenderState::Idle => SenderState::Startup,
                SenderState::Startup => {
                    if self.plateau_reached {
                        // [TUR-3b/TUR-7] plato (base-üstünde değerlendirildi) → DRAIN.
                        self.plateau_reached = false;
                        self.drain_entry_ns = Some(now_ns);
                        SenderState::Drain
                    } else {
                        SenderState::Startup
                    }
                }
                // [TUR-3b] DRAIN çıkışı: inflight ≤ BDP (byte-eşiği — saat-sınıfsız) ∨
                // elapsed ≥ K_drain_max×ack_epoch (liveness-yedeği; ACK-açlığında sonsuz-Drain yok).
                SenderState::Drain => {
                    // Hedef = cwnd_gain×BDP (Steady'nin yaşanabilir üst-sınırı; yalın-BDP
                    // sub-kbps'te TEK-pakete iner → 55s aşırı-drain ÖLÇÜLDÜ, teslim −2).
                    let bdp = crate::rate::CWND_GAIN
                        * crate::rate::bdp_bytes(
                            self.btlbw.btlbw(),
                            self.rtt.rtprop().unwrap_or(INITIAL_RTT),
                        );
                    let epoch = dur_ns(
                        self.rtt
                            .srtt()
                            .unwrap_or(INITIAL_RTT)
                            .max(self.expected_ack_interval()),
                    );
                    let elapsed = now_ns.saturating_sub(self.drain_entry_ns.unwrap_or(now_ns));
                    if self.inflight_bytes <= bdp
                        || elapsed >= epoch.saturating_mul(u64::from(DRAIN_MAX_DWELL_EPOCH))
                    {
                        self.drain_entry_ns = None;
                        SenderState::Steady
                    } else {
                        SenderState::Drain
                    }
                }
                // [TUR-3b D-2/3] ÇIKIŞ-İNVARİANTI (onaylı):
                // (elapsed ≥ min_dwell ∧ qualifying) ∨ elapsed ≥ max_dwell.
                // VERIFIED N=0 bulgusu (aynı-event çıkış; ProbeDown-gain hiç uygulanmıyordu) kapanır.
                SenderState::DrainRtprop => {
                    let epoch = dur_ns(
                        self.rtt
                            .srtt()
                            .unwrap_or(INITIAL_RTT)
                            .max(self.expected_ack_interval()),
                    );
                    let elapsed =
                        now_ns.saturating_sub(self.drain_rtprop_entry_ns.unwrap_or(now_ns));
                    let min_dwell = epoch.saturating_mul(u64::from(DRAIN_RTPROP_MIN_DWELL_EPOCH));
                    let max_dwell = epoch.saturating_mul(u64::from(DRAIN_RTPROP_MAX_DWELL_EPOCH));
                    if (elapsed >= min_dwell && self.drain_rtprop_qualifying)
                        || elapsed >= max_dwell
                    {
                        self.drain_rtprop_entry_ns = None;
                        SenderState::Steady
                    } else {
                        SenderState::DrainRtprop
                    }
                }
                // [TUR-3c D-2/4] AÇIK ProbeUp kolu (`_`e BIRAKILMAZ — sözleşme).
                SenderState::ProbeUp => {
                    let elapsed = now_ns.saturating_sub(self.probe_entry_ns.unwrap_or(now_ns));
                    let epoch_e = self.probe_epoch_entry_ns.max(1);
                    let volume = (u128::from(self.probe_btlbw_entry)
                        * u128::from(epoch_e)
                        * u128::from(PROBE_VOLUME_EPOCH)
                        / 1_000_000_000) as u64;
                    let delivered = self
                        .bw_sampler
                        .delivered()
                        .saturating_sub(self.probe_delivered_entry);
                    let qd_now = self.qd_ratio_permille_now();
                    let success = delivered >= volume
                        && self.btlbw.btlbw()
                            > self.probe_btlbw_entry
                                + self.probe_btlbw_entry * PROBE_SUCCESS_EPS_PERMILLE / 1000;
                    let abort = qd_now > PROBE_QD_ABORT_PERMILLE || self.probe_loss_attributed;
                    let timeout =
                        elapsed >= epoch_e.saturating_mul(u64::from(PROBE_MAX_DWELL_EPOCH));
                    if success {
                        self.probe_close_success(now_ns)
                    } else if abort {
                        self.probe_close(now_ns, ProbeExit::Abort)
                    } else if timeout {
                        self.probe_close(now_ns, ProbeExit::Timeout)
                    } else {
                        SenderState::ProbeUp
                    }
                }
                // Steady: ProbeUp-giriş kontrolü (periyot CANLI-epoch'la — gain=1'de kirlenmez).
                SenderState::Steady => {
                    let epoch = dur_ns(
                        self.rtt
                            .srtt()
                            .unwrap_or(INITIAL_RTT)
                            .max(self.expected_ack_interval()),
                    );
                    let period_ok = now_ns.saturating_sub(self.last_probe_end_ns)
                        >= epoch.saturating_mul(u64::from(K_PROBE_PERIOD_EPOCH));
                    // Giriş-histerezisi (workflow N=0-bulgusu): qd zaten abort-eşiği üstündeyse GİRME.
                    let qd_ok = self.qd_ratio_permille_now() <= PROBE_QD_ABORT_PERMILLE;
                    // DrainRtprop-çakışması çözümü: tazelik-KAPISI YOK (min-örnek yaşı sürekli-akışta
                    // bile büyür — kapı ProbeUp'ı yapısal bloklar, test+m1'de ölçüldü). DrainRtprop
                    // yalnız Steady'den girer → probe-dwell'i kadar bekler (sınırlı ≤4×epoch_entry).
                    let backlog = !self.tx_queue.is_empty();
                    if period_ok && qd_ok && backlog {
                        // GİRİŞ-SNAPSHOT'ları (donuk — öz-referans önlenir; ACK'ler işlendikten
                        // sonra alınır: on_event sırası bunu doğal sağlar).
                        self.probe_entry_ns = Some(now_ns);
                        self.probe_epoch_entry_ns = epoch;
                        self.probe_btlbw_entry = self.btlbw.btlbw();
                        self.probe_delivered_entry = self.bw_sampler.delivered();
                        self.probe_loss_attributed = false;
                        SenderState::ProbeUp
                    } else {
                        SenderState::Steady
                    }
                }
                // [TUR-7 β1] recovery-çıkışı BASE'e döner (Startup-rampı tamamlanmadıysa
                // Startup'a — kayıp-ilanı rampı KESEMEZ; S1-onaylı INV-RATE-1-tamamlama).
                SenderState::RecoverArq | SenderState::RecoverFwd => {
                    if self.base_is_startup {
                        SenderState::Startup
                    } else {
                        SenderState::Steady
                    }
                }
                _ => SenderState::Steady,
            }
        };
        // [TUR-3c workflow-bulgusu] PREEMPTION-muhasebesi: emergency/pending dalları ProbeUp'ı
        // yukarıda keserse çıkış-muhasebesi çalışmaz — burada yakala (4. ayrı-raporlu çıkış).
        if prev_state == SenderState::ProbeUp
            && self.state != SenderState::ProbeUp
            && self.probe_entry_ns.is_some()
        {
            self.probe_entry_ns = None;
            self.last_probe_end_ns = now_ns;
            self.diag_probe_preempt += 1;
        }
        // [TUR-6] Startup (re-)entry: plato-sayaç/marker sıfırlama (§16.2 satırının ikinci
        // yarısı "re-entry'de sayaç sıfırla" — recovery-dönüşü bayat-round/bayat-btlbw taşımasın).
        // [TUR-7 β1] GERÇEK-giriş = state==Startup ∧ base-henüz-değil (Idle→Startup AppData-
        // handler'da update_state-ÖNCESİ mutasyonlandığından prev-koşulu KULLANILMAZ — ampirik).
        // Recovery-DÖNÜŞÜ re-entry SAYILMAZ (base korunuyordu) → sayaç/marker sıfırlanmaz.
        if self.state == SenderState::Startup && !self.base_is_startup {
            self.startup_stable_rounds = 0;
            self.startup_last_btlbw = self.btlbw.btlbw();
            self.startup_round_marker_seq = self.next_seq;
            self.startup_round_complete = false;
            self.base_is_startup = true;
        }
    }

    /// [TUR-3c] qd_ratio (per-mille) — giriş-histerezisi + abort için tek-kaynak.
    fn qd_ratio_permille_now(&self) -> u64 {
        match self.rtt.rtprop() {
            Some(rtp) if !rtp.is_zero() => {
                dur_ns(self.rtt.queue_delay()).saturating_mul(1000) / dur_ns(rtp)
            }
            _ => 0,
        }
    }

    /// ProbeUp çıkış-muhasebesi (success): sayaç + periyot-sıfırlama; yeni btlbw zaten max-filter'da.
    fn probe_close_success(&mut self, now_ns: u64) -> SenderState {
        self.probe_entry_ns = None;
        self.last_probe_end_ns = now_ns;
        self.diag_probe_success += 1;
        SenderState::Steady
    }

    /// ProbeUp çıkış-muhasebesi (abort/timeout — AYRI raporlanır: "başarısız ≠ kapasite-yok").
    fn probe_close(&mut self, now_ns: u64, exit: ProbeExit) -> SenderState {
        self.probe_entry_ns = None;
        self.last_probe_end_ns = now_ns;
        match exit {
            ProbeExit::Abort => self.diag_probe_abort += 1,
            ProbeExit::Timeout => self.diag_probe_timeout += 1,
        }
        SenderState::Steady
    }

    /// STARTUP plato: BtlBw N round Δ<ε ise stabil (§3 STARTUP→STEADY).
    ///
    /// [TUR-A ölçüm-#1, TEK DEĞİŞKEN] Round = YENİ bw-örneği (ACK'ten). Eski kod her on_event
    /// çağrısını round sayıyordu → 1ms pace-tick'te 3 "round" = STARTUP 3ms'de biter, bant hiç
    /// prob edilmez → motor kendi app-limited hızına kilitlenir (TUR-A trajektorisi, kök).
    fn startup_plateau(&mut self) -> bool {
        // [TUR-6] DELIVERED-ANCHOR round-kapısı: marker-sonrası gönderim ACK'lenmeden round
        // İLERLEMEZ (ack-OLAY-sayacı DEĞİL — ZM-1; arz-yoğunluğu-bağımsız). bw-örneği şartı
        // korunur (ölçümsüz round anlamsız).
        if !self.startup_round_complete || !self.bw_sampled_since_round {
            return false;
        }
        self.startup_round_complete = false;
        self.startup_round_marker_seq = self.next_seq;
        self.bw_sampled_since_round = false;
        let bw = self.btlbw.btlbw();
        if bw == 0 {
            return false;
        }
        let grew = bw
            .saturating_sub(self.startup_last_btlbw)
            .saturating_mul(1000)
            > self
                .startup_last_btlbw
                .saturating_mul(STARTUP_PLATEAU_EPS_PERMILLE);
        if grew {
            self.startup_stable_rounds = 0;
        } else {
            self.startup_stable_rounds += 1;
        }
        self.startup_last_btlbw = bw;
        self.startup_stable_rounds >= STARTUP_PLATEAU_ROUNDS
    }

    /// send_rate (§8.2 + §8.5): SRC pacing × fuzzy gain, sonra budget clamp. INV-RATE-1: recovery
    /// durumları base rate'i (Steady) DÜŞÜRMEZ; fuzzy send_rate_gain yalnız bw+queue_delay'den.
    fn compute_send_rate(&self, decision: &FusionDecision) -> u64 {
        // §3.3-A/[R8]: PRE_CONVERGE en-kötü-köşe önsel pacing = floor. min_rate fixture (Ek-B)
        // UYGULANMAZ → 1 kbps kanalı floodlamaz (self-congestion). İlk ölçümle Startup'ta ramp başlar.
        if self.state == SenderState::PreConverge {
            return PRE_CONVERGE_FLOOR_BW;
        }
        let phase = match self.state {
            SenderState::Startup => PacingPhase::Startup,
            SenderState::Drain => PacingPhase::Drain, // [TUR-3b] 1/high_gain
            SenderState::ProbeUp => PacingPhase::ProbeUp, // [TUR-3c] 1.25× — AÇIK kol (sözleşme)
            SenderState::DrainRtprop => PacingPhase::ProbeDown,
            // [TUR-7 β1] recovery-overlay: gain BASE'ten (kayıp-ilanı faz-gain'ine dokunamaz —
            // INV-RATE-1-tamamlama, S1-onaylı; kilit-golden aşağıda test-modülünde).
            SenderState::RecoverArq | SenderState::RecoverFwd if self.base_is_startup => {
                PacingPhase::Startup
            }
            _ => PacingPhase::Steady,
        };
        let btlbw = self.btlbw.btlbw();
        let rtprop = self.rtt.rtprop().unwrap_or(INITIAL_RTT);
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let fuzzy_rate = effective_send_rate(
            &self.src_cfg,
            btlbw,
            rtprop,
            srtt,
            phase.gain(),
            decision.send_rate_gain_permille,
        );
        budget::send_rate_final(fuzzy_rate, self.budget_cfg.min_rate_bps, u64::MAX)
    }

    /// ScheduleTransmit (§3): pacing bütçesinde emit — sıra §3'e göre **DATA ÖNCE**, sonra recovery.
    /// Recovery-obsesif "veri ULAŞSIN" AGGREGATE'te en çok teslim demektir: her birim en az bir kez
    /// denensin (completion-optimal) → sonra kaybolanlar artakalan bütçeyle kurtarılsın. Recovery-first
    /// birkaç eski birimi ısrarla retransmit ederken YENİ birimleri aç bırakırdı (loopback bottleneck-yok
    /// flood'unda birimlerin yarısı hiç gönderilmez; rate-limited path'te de daha az distinct teslim).
    /// Hepsi tek pacer'ı (token bucket) paylaşır (§9/§13): recovery fırtınası rate'i patlatamaz.
    fn schedule_transmit(&mut self, now_ns: u64) -> Vec<TransmitDecision> {
        let mut out = Vec::new();
        // [TUR-7 β2] ARQ token-payı: her K_ARQ_SHARE'inci grant'te retx ÖNCE dener (BOŞ-NÖBET
        // biter — retx fiilen wire'a çıkar); diğer grant'ler DATA-önce (ölçülü-karar korunur).
        self.arq_share_tick = self.arq_share_tick.wrapping_add(1);
        if self.state == SenderState::RecoverArq
            && !self.pending_lost.is_empty()
            && self.arq_share_tick.is_multiple_of(K_ARQ_SHARE)
        {
            self.plan_arq_retransmits(&mut out, now_ns);
        }
        self.drain_new_data(&mut out, now_ns);
        match self.state {
            SenderState::RecoverArq => self.plan_arq_retransmits(&mut out, now_ns),
            SenderState::RecoverFwd => self.plan_forward_duplication(now_ns),
            _ => {}
        }
        self.drain_due_duplicates(&mut out, now_ns);
        // DR-S5-1: FEC-parity DATA/recovery SONRASI (DATA-ÖNCE-sıralaması mühürlü); yalnız fec_enabled.
        self.plan_fec_parity(&mut out, now_ns);
        out
    }

    /// FEC-emisyonu aç/kapa (DR-S5-1 guard). Engine FEC-etkinken açar; kapalı=mühürlü-hot-path.
    /// Commit-G: otorite-mode-ayarı. Geçiş block-boundary-etkin (çağıran-kontratı; sender-blok-bilmez —
    /// authority-yalnız-blok-granüler-noktalarda-okunduğundan mid-block-karışım-yapısal-önlenir).
    /// Şimdilik-hatasız (Result-imza ileri-uyum; §7 immediate-safety-disable AYRI-DR).
    pub fn set_fec_actuation_mode(
        &mut self,
        mode: FecActuationMode,
    ) -> Result<(), FecAuthorityError> {
        self.fec_mode = mode;
        self.last_transition_event_index = self.fusion_event_index;
        self.transition_applied_count = self.transition_applied_count.saturating_add(1);
        Ok(())
    }

    /// Commit-I: blok-AÇILIŞINDA çağrılır — o-anki-aktif-mode witness'a-yakalanır. Bloğun-otorite-
    /// kararları bu-witness'la (yapısal open-mode==close-mode).
    #[must_use]
    pub fn open_fec_block_authority(&self) -> FecBlockAuthority {
        FecBlockAuthority {
            mode: self.fec_mode,
        }
    }

    /// Commit-G: manuel-FEC-toggle — YALNIZ Manual-mode'da. Controller'da TYPED-red (sessiz-no-op-YASAK;
    /// state-DEĞİŞMEZ; sayaç-artar).
    ///
    /// # Errors
    /// [`FecAuthorityError::NotInManualMode`] — otorite-Controller'dayken.
    pub fn set_manual_fec_enabled(&mut self, enabled: bool) -> Result<(), FecAuthorityError> {
        match self.fec_mode {
            FecActuationMode::Manual { .. } => {
                self.fec_mode = FecActuationMode::Manual { enabled };
                Ok(())
            }
            FecActuationMode::Controller => {
                self.manual_toggle_rejected_count =
                    self.manual_toggle_rejected_count.saturating_add(1);
                Err(FecAuthorityError::NotInManualMode)
            }
        }
    }

    /// Emisyon-kapısı (plan_fec_parity-guard'ı): Manual→enabled; Controller→true (workload latch'ten).
    fn fec_emission_enabled(&self) -> bool {
        match self.fec_mode {
            FecActuationMode::Manual { enabled } => enabled,
            FecActuationMode::Controller => true,
        }
    }

    /// Commit-G: controller-karar-anlık-görüntüsü (same-event-guard-girdisi; engine latch-öncesi-okur).
    #[must_use]
    pub fn controller_decision_snapshot(&self) -> Option<ControllerDecisionSnapshot> {
        self.last_fusion_snapshot
            .map(|s| ControllerDecisionSnapshot {
                ratio_permille: self.last_decision.fec_parity_ratio_permille,
                event_index: s.event_index,
                no_rule: s.no_rule,
                emergency: self.last_decision.mode == Mode::EmergencyForwardRedundancy,
            })
    }

    /// Commit-G: block-close-LATCH — saf-karar-zinciri (§9):
    /// fusion→same-event-eligibility→no-rule/initial-guard→ratio→k_actual-r→profile-clamp→emergency-
    /// suppression→planned. Her-adım FecLatchResult'ta-görünür. Baskılamalar LITERAL-0 (config-bağımsız).
    pub fn fec_controller_latch(
        &mut self,
        authority: FecBlockAuthority,
        snapshot: Option<ControllerDecisionSnapshot>,
        k_actual: u8,
        profile_r_max: u16,
    ) -> FecLatchResult {
        self.latch_event_index = self.fusion_event_index;
        let zero = |ratio, why, same_ev| FecLatchResult {
            requested_ratio_permille: ratio,
            computed_r: 0,
            clamped_r: 0,
            planned_r: 0,
            same_event_eligible: same_ev,
            suppression: Some(why),
        };
        // Commit-I: otorite = WITNESS-mode (blok-açılış-yakalaması) — aktif-mode-DEĞİL.
        let r = match authority.mode {
            FecActuationMode::Manual { .. } => zero(0, FecSuppressionReason::ManualDisabled, false),
            FecActuationMode::Controller => match snapshot {
                None => {
                    self.no_eligible_decision_count =
                        self.no_eligible_decision_count.saturating_add(1);
                    zero(0, FecSuppressionReason::NoEligibleDecision, false)
                }
                Some(s) if s.event_index != self.fusion_event_index => {
                    // SAME-EVENT-CAUSALITY-GUARD (addendum-onaylı-ad): önceki-event-kararı yeni-bloğa
                    // UYGULANMAZ; fail-closed LITERAL-0. (Ağ-staleness-politikası-DEĞİL — ayrı-faz.)
                    self.decision_event_mismatch_count =
                        self.decision_event_mismatch_count.saturating_add(1);
                    zero(
                        s.ratio_permille,
                        FecSuppressionReason::DecisionEventMismatch,
                        false,
                    )
                }
                Some(s) if s.no_rule => {
                    self.no_rule_zero_count = self.no_rule_zero_count.saturating_add(1);
                    zero(0, FecSuppressionReason::NoRule, true) // LITERAL-0 (fec_zero-config'e-BAĞLI-DEĞİL)
                }
                Some(s) => {
                    let (computed, clamped) =
                        compute_requested_r(s.ratio_permille, k_actual, profile_r_max);
                    if s.emergency {
                        // Temporary safety interlock pending 1b/1d arbitration: requested-GÖRÜNÜR, planned=0.
                        FecLatchResult {
                            requested_ratio_permille: s.ratio_permille,
                            computed_r: computed,
                            clamped_r: clamped,
                            planned_r: 0,
                            same_event_eligible: true,
                            suppression: Some(FecSuppressionReason::EmergencyPendingArbitration),
                        }
                    } else {
                        FecLatchResult {
                            requested_ratio_permille: s.ratio_permille,
                            computed_r: computed,
                            clamped_r: clamped,
                            planned_r: clamped,
                            same_event_eligible: true,
                            suppression: None,
                        }
                    }
                }
            },
        };
        self.last_latch = Some(r);
        r
    }

    /// Commit-G/H: aktüasyon-teşhisi (named; son-latch + bounded-sayaçlar).
    #[must_use]
    pub fn diag_fec_actuation(&self) -> FecActuationDiag {
        FecActuationDiag {
            authority_mode: self.fec_mode,
            last_latch: self.last_latch,
            latch_event_index: self.latch_event_index,
            no_eligible_decision_count: self.no_eligible_decision_count,
            decision_event_mismatch_count: self.decision_event_mismatch_count,
            no_rule_zero_count: self.no_rule_zero_count,
            manual_toggle_rejected_count: self.manual_toggle_rejected_count,
            enqueued_r_total: self.parity_enqueued_total,
            emitted_r_total: self.parity_emitted_total,
            canceled_r_total: self.diag_recovered_cancels,
            token_denied_r_gauge: self.token_denied_r_gauge,
            last_transition_event_index: self.last_transition_event_index,
            transition_applied_count: self.transition_applied_count,
        }
    }

    /// Engine FEC-encode-sonrası bir repair-shard'ı emisyon-kuyruğuna alır (DR-S5-1). fec_enabled-OFF'ta
    /// enqueue-edilse-de plan_fec_parity çalışmaz (guard). Shard-byte'ları güvenlik-katmanında; burada boyut+ref.
    pub fn enqueue_fec_parity(&mut self, block_id: u32, esi: u16, size: u64) {
        self.parity_enqueued_total = self.parity_enqueued_total.saturating_add(1);
        self.pending_fec.push_back(PendingFecParity {
            block_id,
            esi,
            size,
        });
    }

    /// DR-S5-3: wire-ACK'lenen seq'in logical-teyidini ayrı-deftere yazar (BOUNDED). **SALT-LOGICAL —
    /// bw_sampler/btlbw/delivered'e DOKUNMAZ.** sent_logical'da-yoksa (DATA/bilinmeyen) NO-OP.
    fn record_confirmed_logical(&mut self, seq: u64) {
        if let Some(lref) = self.sent_logical.remove(&seq) {
            if self.confirmed_logical.len() >= CONFIRMED_LOGICAL_CAP {
                self.confirmed_logical.pop_front(); // bounded: drain-yoksa en-eski düşer (SR-AUDIT-001)
            }
            self.confirmed_logical
                .push_back((seq, lref, ConfirmSource::WireAck));
        } else {
            // DR-S5-6: kayıtsız-seq'e ACK-teyidi (salt-teşhis). Normal-akışta zararsız (register-
            // edilmemiş-DATA). M1'de-bile TEK-BAŞINA artefakt-göstergesi DEĞİL: miss>0 ∧ evicted==0
            // beklenen Recovered→geç-ACK yarışıdır (RECOVERED kaydı-önce-teyit-etti). M1-hücre-
            // işaretleme YALNIZ evicted>0 ile (SENT_LOGICAL_CAP yorumu; adversarial-ÇÜRÜT-1).
            self.diag_confirmed_logical_miss += 1;
        }
    }

    /// DR-S5-6: sent_logical'a BOUNDED-insert (SR-AUDIT-001). Seq'ler monoton → en-eski = en-küçük-seq
    /// (`pop_first`). Eviction M1-artefakt-riski taşır → audit-sayaç (M1-guard: hücrede 0-olmalı).
    fn insert_sent_logical(&mut self, seq: u64, lref: SrLogicalRef) {
        if !self.sent_logical.contains_key(&seq) && self.sent_logical.len() >= SENT_LOGICAL_CAP {
            self.sent_logical.pop_first(); // en-eski-seq düşer
            self.diag_sent_logical_evicted += 1;
        }
        self.sent_logical.insert(seq, lref);
    }

    /// DR-S5-6 (M1-adalet; N8-v2 Not-2 kapanışı): engine, emit-edilmiş bir seq'in logical-kimliğini
    /// deftere kaydeder — DATA-source için (FEC-parity `plan_fec_parity`'de zaten-kayıtlı). Transport
    /// DATA'nın block/esi'sini BİLMEZ (SoC); çevrim-bilgisi engine'de, kayıt post-emit (TransmitDecision
    /// pub seq üzerinden). **SALT-DEFTER — bw_sampler/btlbw/pacer/schedule'a DOKUNMAZ.**
    pub fn register_sent_logical(&mut self, seq: u64, lref: SrLogicalRef) {
        self.insert_sent_logical(seq, lref);
    }

    /// DR-S5-6 teşhis: (sent_logical-len, cap-evicted, ack-teyit-kayıtsız-miss). M1-guard:
    /// hücre-koşusunda evicted==0 olmalı (aksi eviction-artefaktı → hücre-işaretlenir).
    #[must_use]
    pub fn diag_sent_logical(&self) -> (usize, u64, u64) {
        (
            self.sent_logical.len(),
            self.diag_sent_logical_evicted,
            self.diag_confirmed_logical_miss,
        )
    }

    /// DR-S5-4 [SALT-LOGICAL — N8-firewall]: RECOVERED-control işle. YALNIZ pending_lost/dup_queue/
    /// confirmed_logical'a dokunur; **bw_sampler/btlbw/credit_delivered/pacer'a SIFIR-satır**
    /// (recovered≠ACK — kredi-yolu AÇILMAZ; §0 yasak-6-blok). Emit-üretmez (saf-state; on_event
    /// erken-dönüşle schedule_transmit'i ATLAR).
    fn on_recovered_control(&mut self, info: &RecoveredInfo) {
        for ru in &info.units {
            // (i) retransmit-CANCEL: kurtarılan-unit'in bekleyen ARQ-retransmit'i iptal — FEC
            // ana-avantajı (K2/M1: kurtardığını-retransmit-etmeme). Saf VecDeque-filtre;
            // pacer yalnız retransmit-ATTEMPT'te okunur → cancel attempt'i hiç-başlatmaz.
            let before = self.pending_lost.len();
            self.pending_lost.retain(|r| r.unit.id != ru.unit_id);
            self.diag_recovered_cancels += (before - self.pending_lost.len()) as u64;
            // (ii) retention-RELEASE: bekleyen forward-dup kopyaları iptal (mevcut acked-cancel
            // `dup_queue.retain(!acked_units.contains)` mekanizmasının birebir-paraleli).
            self.dup_queue.retain(|d| d.unit.id != ru.unit_id);
            // (iii) logical-defter: recovered-teyit (ConfirmSource::Recovered; SALT-LOGICAL).
            self.record_recovered_logical(ru.block_id, ru.esi);
        }
    }

    /// DR-S5-4: recovered-logical'ı defter'e yaz (ConfirmSource::Recovered). sent_logical'da kayıtlı
    /// bir emisyonu (block_id, esi) varsa onu teyit-eder (kayıtsız-recovered → NO-OP; bilinmeyen-logical
    /// teyit-edilmez — record_confirmed_logical'ın unknown-seq-NO-OP'uyla simetrik). **bw'ye DOKUNMAZ.**
    fn record_recovered_logical(&mut self, block_id: u32, esi: u16) {
        let found = self
            .sent_logical
            .iter()
            .find(|(_, lref)| {
                matches!(lref, SrLogicalRef::Fec { block_id: b, esi: e } if *b == block_id && *e == esi)
            })
            .map(|(&seq, _)| seq);
        if let Some(seq) = found {
            if let Some(lref) = self.sent_logical.remove(&seq) {
                if self.confirmed_logical.len() >= CONFIRMED_LOGICAL_CAP {
                    self.confirmed_logical.pop_front(); // bounded (SR-AUDIT-001)
                }
                self.confirmed_logical
                    .push_back((seq, lref, ConfirmSource::Recovered));
            }
        }
    }

    /// Engine teyit-edilen logical-paketleri drenajlar (ARQ-cancel/retention; DR-S5-3 + DR-S5-4
    /// ConfirmSource-marker). Salt-logical defter; BtlBw/rate ile ilgisi YOK.
    pub fn take_confirmed_logical(&mut self) -> Vec<(u64, SrLogicalRef, ConfirmSource)> {
        self.confirmed_logical.drain(..).collect()
    }

    /// DR-S5-4 (K2/M1-sinyali): RECOVERED ile iptal-edilen bekleyen-retransmit sayısı. M1'de
    /// FEC-arm-metriği olarak okunur ("FEC kurtardığını-retransmit-etmiyor"un ölçülebilir-kanıtı).
    #[must_use]
    pub fn diag_recovered_cancels(&self) -> u64 {
        self.diag_recovered_cancels
    }

    /// Teyit-edilen-logical defter uzunluğu (teşhis/test; DR-S5-3).
    #[must_use]
    pub fn diag_confirmed_logical_count(&self) -> usize {
        self.confirmed_logical.len()
    }

    /// Bekleyen FEC-parity'yi pace-ederek emit et (DR-S5-1). fec_enabled-OFF → NO-OP (hot-path byte-değişmez).
    /// Pacing: mevcut `pacer.try_send` (TokenBucket-internals'a DOKUNMAZ; data/retx ile aynı-API). send-record-hint üretir.
    fn plan_fec_parity(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        if !self.fec_emission_enabled() {
            return; // guard-OFF: dal-hiç-çalışmaz, allocation-yok (C-kuralı).
        }
        while let Some(&p) = self.pending_fec.front() {
            if !self.pacer.try_send(p.size) {
                // Commit-G: budget-denied-GÖRÜNÜRLÜĞÜ (gauge; 1b-tasarım-girdisi — politika-DEĞİŞMEDİ).
                self.token_denied_r_gauge = self.pending_fec.len() as u64;
                break; // token yok → sonraki tick (data/retx ile aynı pacing-davranışı).
            }
            self.parity_emitted_total = self.parity_emitted_total.saturating_add(1);
            self.pending_fec.pop_front();
            let seq = self.track_sent(
                OpaqueUnit {
                    id: u64::from(p.esi),
                    size: p.size,
                },
                now_ns,
                0,
                TransmitKind::FecParity,
            );
            // DR-S5-3: emit-anında seq→logical kaydı (ACK-teyidinde eşlenecek; SALT-LOGICAL).
            // DR-S5-6: bounded-helper'dan (cap-altında davranış-özdeş).
            self.insert_sent_logical(
                seq,
                SrLogicalRef::Fec {
                    block_id: p.block_id,
                    esi: p.esi,
                },
            );
            out.push(TransmitDecision {
                kind: TransmitKind::FecParity,
                seq,
                unit_id: u64::from(p.esi),
                size: p.size,
                send_record: Some(SendRecordHint {
                    logical_ref: SrLogicalRef::Fec {
                        block_id: p.block_id,
                        esi: p.esi,
                    },
                    packet_class: SrPacketClass::Repair,
                    send_generation: 0,
                }),
            });
        }
    }

    /// RECOVER_ARQ (§9): ACK-güdümlü selective retransmit. arq_aggressiveness=0 → baskıla (fec-dominant).
    fn plan_arq_retransmits(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        let lost: Vec<LostSeq> = self
            .pending_lost
            .iter()
            .map(|r| LostSeq {
                seq: r.seq,
                deadline_ns: r.deadline_ns,
                retx_count: r.retx_count,
            })
            .collect();
        let plan = self
            .arq
            .plan(&lost, self.last_decision.arq_aggressiveness_permille);
        for dec in plan {
            match dec {
                RetxDecision::Retransmit { seq } => {
                    let Some(pos) = self.pending_lost.iter().position(|r| r.seq == seq) else {
                        continue;
                    };
                    let rec = self.pending_lost[pos];
                    if !self.pacer.try_send(rec.unit.size) {
                        break; // token yok → sonraki tick'te (deadline plandan düşmez, §9).
                    }
                    self.pending_lost.remove(pos);
                    // [TUR-2c D-1/3 kol-2] Orijinalin tombstone'u SUPERSEDED: retx-sonrası-
                    // geç-orijinal-ACK kol-1'e düşemez (2b'de kayıt ilan-anında donuyordu).
                    self.mark_tombstone_superseded(seq);
                    let new_seq = self.track_sent(
                        rec.unit,
                        now_ns,
                        rec.retx_count + 1,
                        TransmitKind::Retransmit,
                    );
                    out.push(TransmitDecision {
                        kind: TransmitKind::Retransmit,
                        seq: new_seq,
                        unit_id: rec.unit.id,
                        size: rec.unit.size,
                        send_record: None,
                    });
                }
                RetxDecision::CapExceeded { seq, policy } => {
                    if let Some(pos) = self.pending_lost.iter().position(|r| r.seq == seq) {
                        let rec = self.pending_lost.remove(pos).expect("pos geçerli");
                        if policy == CapExceedPolicy::HandToForward {
                            // ARQ cap doldu → forward-recovery'ye devret (§9.1 "veri ulaşsın").
                            // [TUR-2c review-fix] devir de YENİDEN-EMİSYONDUR → superseded-işaret
                            // (örnekleme politikası recovery-yoluna bağımlı olamaz — tekdüze kural).
                            self.mark_tombstone_superseded(rec.seq);
                            self.dup_queue.push(DupRec {
                                due_ns: now_ns,
                                unit: rec.unit,
                                interval_ns: 0, // emergency: fire-once
                            });
                        }
                        // ConnectionFail: spine'da düşür; gerçek impl bağlantı-fail sinyali verir.
                    }
                }
            }
        }
    }

    /// RECOVER_FWD (§11): ACK-BAĞIMSIZ forward duplication. Kayıp birimleri `factor` kez zaman-aralıklı
    /// kopya olarak planla (ACK beklemeden — reverse-path çöktüğü için ARQ futile).
    ///
    /// [TUR-1a / D-1/1 liveness kolu] pending_lost BOŞKEN (saf-sessizlik: later-ACK preconditionı loss
    /// SINIFLANDIRMASINI blokladı ama tail cevapsız): unacked orijinal inflight birimlerine spekülatif
    /// geometrik kopya (taban=srtt, ×2 backoff; ACK'le iptal §3.3-B-stop). Loss METRİĞİNE dokunmaz —
    /// "saf sessizlik loss'u artırmaz; fakat liveness amacıyla probe/retransmit tetikleyebilir" (§5.1).
    fn plan_forward_duplication(&mut self, now_ns: u64) {
        if self.pending_lost.is_empty() {
            // Saf-sessizlik tail-liveness: kopyası zaten planlı olmayan orijinal-DATA inflight birimleri.
            let srtt_ns = dur_ns(self.rtt.srtt().unwrap_or(INITIAL_RTT));
            let tail: Vec<OpaqueUnit> = self
                .inflight
                .values()
                .filter(|p| p.origin == TransmitKind::Data)
                .map(|p| p.unit)
                .filter(|u| !self.dup_queue.iter().any(|d| d.unit.id == u.id))
                .collect();
            for unit in tail {
                self.dup_queue.push(DupRec {
                    due_ns: now_ns,
                    unit,
                    interval_ns: srtt_ns.max(1), // geometrik reschedule (drain'de ×COPY_MULTIPLIER)
                });
            }
            return;
        }
        let (factor, spacing) = self.dup_params();
        let recs: Vec<LostRec> = self.pending_lost.drain(..).collect();
        for rec in recs {
            // [TUR-2c review-fix] FWD-kopya da YENİDEN-EMİSYONDUR: orijinalin tombstone'u
            // superseded — kol-1/kol-2 seçimi recovery-state'e göre DEĞİŞEMEZ (tekdüze
            // muhafazakâr kural; ARQ-koluyla simetri).
            self.mark_tombstone_superseded(rec.seq);
            for i in 0..u64::from(factor) {
                self.dup_queue.push(DupRec {
                    due_ns: now_ns + i.saturating_mul(dur_ns(spacing)),
                    unit: rec.unit,
                    interval_ns: 0, // emergency: fire-once, reschedule YOK
                });
            }
        }
    }

    /// [TUR-2c] Orijinal iletimin tombstone'unu SUPERSEDED işaretle (retx_count++): birimin
    /// HERHANGİ bir yeniden-emisyonundan (ARQ retransmit / FWD dup / HandToForward) sonra
    /// orijinalin geç-ACK'i kol-1'e (RTT + rate-örneği) düşemez — tekdüze muhafazakâr kural.
    fn mark_tombstone_superseded(&mut self, seq: u64) {
        if let Some(t) = self.tombstones.get_mut(&seq) {
            t.retx_count = t.retx_count.saturating_add(1);
        }
    }

    /// Vadesi gelen duplicate kopyalarını pace ederek emit et (§11 spacing).
    fn drain_due_duplicates(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        let mut i = 0;
        while i < self.dup_queue.len() {
            if self.dup_queue[i].due_ns > now_ns {
                i += 1;
                continue; // henüz vadesi gelmedi.
            }
            if !self.pacer.try_send(self.dup_queue[i].unit.size) {
                break; // token yok.
            }
            let dup = self.dup_queue.remove(i);
            let seq = self.track_sent(dup.unit, now_ns, 0, TransmitKind::Duplicate);
            out.push(TransmitDecision {
                kind: TransmitKind::Duplicate,
                seq,
                unit_id: dup.unit.id,
                size: dup.unit.size,
                send_record: None,
            });
            // §3.3-B geometrik reschedule: interval_ns>0 (PRE_CONVERGE kopyası) → sonraki kopya
            // ×COPY_MULTIPLIER (sayı-tavanı YOK, deadline'a dek; teyit gelince §3.3-B stop ile iptal).
            if dup.interval_ns > 0 {
                let next_interval = dup.interval_ns.saturating_mul(PC_COPY_MULTIPLIER);
                self.dup_queue.push(DupRec {
                    due_ns: now_ns.saturating_add(next_interval),
                    unit: dup.unit,
                    interval_ns: next_interval,
                });
            }
        }
    }

    /// Yeni DATA'yı pace ederek emit et. RECOVER_FWD'de her DATA ayrıca (factor−1) kopya planlar.
    fn drain_new_data(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        while let Some(front) = self.tx_queue.front().copied() {
            // [R3/ADIM-6] Burst clamp artık pipeline'da kanal-türevli set_burst_cap ile (PreConverge floor
            // rate → burst_cap = 1 birim). Ayrı PreConverge özel-durumu GEREKMEZ (tek kapı, tüm state'ler).
            if !self.pacer.try_send(front.size) {
                break;
            }
            self.tx_queue.pop_front();
            let seq = self.track_sent(front, now_ns, 0, TransmitKind::Data);
            out.push(TransmitDecision {
                kind: TransmitKind::Data,
                seq,
                unit_id: front.id,
                size: front.size,
                send_record: None,
            });
            if self.state == SenderState::RecoverFwd {
                let (factor, spacing) = self.dup_params();
                for i in 1..u64::from(factor) {
                    self.dup_queue.push(DupRec {
                        due_ns: now_ns + i.saturating_mul(dur_ns(spacing)),
                        unit: front,
                        interval_ns: 0, // emergency: fire-once
                    });
                }
            } else if self.state == SenderState::PreConverge {
                // §3.3-B geometrik kopya: ilk kopya k_base×RTT_floor sonra; ateşlenince ×COPY_MULTIPLIER.
                // RTT_floor ≈ (unit+sack)/floor_bw. Temiz kanalda ilk SACK bunu ateşlemeden iptal eder.
                // [K-D2] t₀ = wire-emit anı = buradaki `now_ns` (birim pacer.try_send'i geçti = WIRE'a çıktı),
                // sunum/enqueue anı DEĞİL → burst kuyruğunun serialization gecikmesi otomatik hesaba girer.
                let rtt_floor = (front.size + PC_SACK_WIRE)
                    .saturating_mul(1_000_000_000)
                    .checked_div(PRE_CONVERGE_FLOOR_BW)
                    .unwrap_or(0);
                let first_delay = rtt_floor.saturating_mul(PC_KBASE_NUM) / PC_KBASE_DEN;
                self.dup_queue.push(DupRec {
                    due_ns: now_ns.saturating_add(first_delay),
                    unit: front,
                    interval_ns: first_delay, // reschedule aralığı (drain'de ×COPY_MULTIPLIER)
                });
            }
        }
    }

    /// last_recovery'den (factor, spacing); Duplication değilse varsayılan (§11 Tiny framing).
    fn dup_params(&self) -> (u8, Duration) {
        match self.last_recovery {
            RecoveryAction::Duplication { factor, spacing } => (factor, spacing),
            _ => (DEFAULT_DUP_FACTOR, DEFAULT_DUP_SPACING),
        }
    }

    fn track_sent(
        &mut self,
        unit: OpaqueUnit,
        now_ns: u64,
        retx_count: u8,
        origin: TransmitKind,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        if self.first_send_ns.is_none() {
            self.first_send_ns = Some(now_ns);
        }
        let more_pending = !self.tx_queue.is_empty()
            || !self.pending_lost.is_empty()
            || !self.dup_queue.is_empty();
        // [TUR-D1 F3] Gönderim-ANI gain-damgası: öz-kısma fazları (Drain 0.347× /
        // DrainRtprop 0.75× / PreConverge floor) gain<1 damgası (örnek-sınıflandırma-bilgisi; tüketicisi-arşivlendi, alan-korunur).
        let gain_ge1 = !matches!(
            self.state,
            SenderState::Drain | SenderState::DrainRtprop | SenderState::PreConverge
        );
        let bw_sample = self
            .bw_sampler
            .on_sent(now_ns, unit.size, !more_pending, gain_ge1);
        let units = u64::from(origin == TransmitKind::Data); // yalnız orijinal DATA loss sayacına.
        self.loss.on_sent(seq, now_ns, units);
        self.inflight_bytes += unit.size;
        self.inflight.insert(
            seq,
            InflightPacket {
                unit,
                send_ns: now_ns,
                bw_sample,
                retx_count,
                origin,
            },
        );
        seq
    }

    /// ack_confidence (per-mille, §5.1): reverse-path canlılığı = ACK SESSİZLİĞİ. Aktif iş varken son
    /// ACK'ten (ya da ilk gönderimden) geçen süre srtt katını aşarsa güven düşer. Yeni paket "düşük
    /// güven" DEĞİL — yalnız SESSİZLİK reverse-path çöküşüdür.
    fn ack_confidence_permille(&self, now_ns: u64) -> u64 {
        // §3.3-C/(4) yasak-4: PRE_CONVERGE'de ack_confidence FULL sabit — sessizlik cold-start'ta
        // reverse-path çöküşü DEĞİL taban-bw serialization gecikmesi (threshold≈300ms ≪ RTT_floor).
        if self.state == SenderState::PreConverge {
            return 1000;
        }
        let active = !self.inflight.is_empty()
            || !self.pending_lost.is_empty()
            || !self.tx_queue.is_empty()
            || !self.dup_queue.is_empty();
        if !active {
            return 1000;
        }
        let anchor = match self.last_ack_ns.or(self.first_send_ns) {
            Some(a) => a,
            None => return 1000,
        };
        let silence = now_ns.saturating_sub(anchor);
        let srtt = dur_ns(self.rtt.srtt().unwrap_or(INITIAL_RTT));
        // [TUR-3b commit-C] eşik = max(3×srtt, K_CONF_EAI×EAI) — tüketici-8 (srtt-clock-v1 §3).
        let threshold = srtt
            .saturating_mul(u64::from(ACK_SILENCE_SRTT_MULT))
            .max(dur_ns(self.expected_ack_interval()).saturating_mul(u64::from(K_CONF_EAI)));
        if silence <= threshold {
            return 1000;
        }
        threshold
            .saturating_mul(1000)
            .checked_div(silence)
            .unwrap_or(0)
            .min(1000)
    }

    // ── gözlemciler (test + entegrasyon) ──

    /// Güncel state.
    #[must_use]
    pub fn state(&self) -> SenderState {
        self.state
    }
    /// inflight bytes.
    #[must_use]
    pub fn inflight_bytes(&self) -> u64 {
        self.inflight_bytes
    }
    /// Son füzyon kararı.
    #[must_use]
    pub fn last_decision(&self) -> FusionDecision {
        self.last_decision
    }
    /// TUR-A teşhis rate-trajektori snapshot'ı (SALT-GÖZLEM; davranış DEĞİŞMEZ):
    /// `(state, btlbw_bps, srtt_us, send_rate_bps, inflight_bytes)`. floor→btlbw ramp kilidini ölçmek için.
    #[must_use]
    pub fn diag_rate(&self) -> (SenderState, u64, u64, u64, u64) {
        let srtt_us = self
            .rtt
            .srtt()
            .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
        (
            self.state,
            self.btlbw.btlbw(),
            srtt_us,
            self.pacer.rate(),
            self.inflight_bytes,
        )
    }
    /// [TUR-A teşhis, ölçüm-#2] loss-atıflama (SALT-GÖZLEM): `(packet_threshold_queued,
    /// time_threshold_queued, pending_lost_now)`. Temiz kanalda hangi tetik spurious RecoverArq açıyor —
    /// gerçek gap (packet) mi, cadence-gap (time) mi.
    #[must_use]
    pub fn diag_loss(&self) -> (u64, u64, u64) {
        (
            self.diag_loss_pkt,
            self.diag_loss_time,
            self.pending_lost.len() as u64,
        )
    }
    /// [TUR-1x teşhis] ack_delay/RTT-guard (SALT-GÖZLEM): `(rejected_samples, max_decoded_ack_delay_us)`.
    /// V2 kodlama-sadakati kanıtı: max_us cadence-hold mertebesine (saniyeler) çıkabilmeli (eski µs-birim
    /// tavanı 65535'te kelepçeliydi); rejected temiz kanalda ≈0 beklenir.
    #[must_use]
    pub fn diag_rtt(&self) -> (u64, u64) {
        (self.diag_rtt_rejected, self.diag_ack_delay_max_us)
    }
    /// [TUR-2a teşhis] ledger/un-loss (SALT-GÖZLEM): `(unloss_iptal, loss_windowed_ppm,
    /// spurious_units_total)` — ratchet-ölümü + geri-alma hacmi kanıtı.
    #[must_use]
    pub fn diag_ledger(&self) -> (u64, u64, u64) {
        (
            self.diag_unloss,
            self.loss.loss_windowed_ppm(),
            self.loss.spurious_units_total(),
        )
    }
    /// [TUR-3a teşhis] sampler uçuş-sayısı (SALT-GÖZLEM — §3.7 sızıntı-borcu ölçümü).
    #[must_use]
    pub fn diag_pif(&self) -> u64 {
        self.bw_sampler.packets_in_flight()
    }
    /// [TUR-3a teşhis] rtprop + queue_delay (SALT-GÖZLEM — qd-atıflaması, µs).
    #[must_use]
    pub fn diag_qd(&self) -> (u64, u64) {
        (
            self.rtt.rtprop().map_or(0, |d| d.as_micros() as u64),
            self.rtt.queue_delay().as_micros() as u64,
        )
    }
    /// [Y3-sim SALT-GÖZLEM] (last_full_bw, window_ns) — (b)-bug-arayışı.
    #[must_use]
    pub fn diag_wbtlbw(&self) -> (u64, u64) {
        (self.btlbw.last_sample_bw(), self.btlbw.window_ns())
    }

    /// [Y3-sim SALT-GÖZLEM] btlbw-pencere-örnek-izi (mekanizma-izi-kalibrasyonu).
    #[must_use]
    pub fn diag_btlbw_samples(&self) -> Vec<(u64, u64)> {
        self.btlbw.samples_snapshot()
    }

    /// [TUR-4] Sıfır-ağırlık fuzzy-fallback sayacı (gözlemlenebilirlik).
    #[must_use]
    pub fn diag_fuzzy_no_rule(&self) -> u64 {
        self.diag_fuzzy_no_rule
    }

    /// [TUR-3c teşhis] ProbeUp çıkış-sayaçları (SALT-GÖZLEM): (success, abort, timeout, preempt).
    #[must_use]
    pub fn diag_probe(&self) -> (u64, u64, u64, u64) {
        (
            self.diag_probe_success,
            self.diag_probe_abort,
            self.diag_probe_timeout,
            self.diag_probe_preempt,
        )
    }
    /// [TUR-2b/2c teşhis] tombstone (SALT-GÖZLEM):
    /// `(canlı, hit, bw_kurtarılan, evicted, ambiguous, credited_bytes)`.
    #[must_use]
    pub fn diag_tombstone(&self) -> (u64, u64, u64, u64, u64, u64) {
        (
            self.tombstones.len() as u64,
            self.diag_tomb_hits,
            self.diag_bw_recovered,
            self.tombstone_evicted,
            self.diag_ambiguous_skipped,
            self.diag_delivered_credited,
        )
    }
    /// Son hesaplanan forward-recovery eylemi (§11).
    #[must_use]
    pub fn last_recovery(&self) -> RecoveryAction {
        self.last_recovery
    }
    /// Güncel emergency seviyesi.
    #[must_use]
    pub fn emergency_level(&self) -> EmergencyLevel {
        self.emergency.level()
    }
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

fn dur_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: u64) -> OpaqueUnit {
        OpaqueUnit { id, size: 1200 }
    }
    const MS: u64 = 1_000_000;

    fn ack(seqs: &[u64]) -> Event {
        Event::AckReceived(AckInfo {
            acked_seqs: seqs.to_vec(),
            ack_delay: Duration::ZERO,
        })
    }

    #[test]
    fn idle_to_preconverge_then_startup_on_first_ack() {
        // §3.3: ilk data → PRE_CONVERGE (cold-start, ölçüm yok) — Startup DEĞİL. İlk geçerli SACK → Startup.
        let mut s = Sender::new();
        assert_eq!(s.state(), SenderState::Idle);
        let out = s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.state(), SenderState::PreConverge);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, TransmitKind::Data);
        assert_eq!(out[0].seq, 0);
        assert_eq!(s.inflight_bytes(), 1200);
        s.on_event(Event::AppData(unit(1)), MS); // ikinci birim inflight kalsın (ack sonrası iş var)
                                                 // §3.3-D kademeli tahliye: ilk geçerli SACK (RTT kilidi) + iş var → Startup.
        s.on_event(ack(&[0]), 50 * MS);
        assert_eq!(s.state(), SenderState::Startup);
    }

    #[test]
    fn preconverge_clean_no_spurious_recovery() {
        // §3.3 yasaklar: temiz cold-start → dup YOK + retx YOK (BEFORE'daki dup=1820B'nin kök çözümü).
        let mut s = Sender::new();
        let o0 = s.on_event(Event::AppData(small(0)), 0);
        assert_eq!(s.state(), SenderState::PreConverge);
        assert!(
            o0.iter().all(|d| d.kind == TransmitKind::Data),
            "PRE_CONVERGE'de yalnız DATA emit"
        );
        // ilk geçerli SACK → cold-start biter (tek birim acked → Idle); hiç Duplicate/Retransmit YOK (yasak-1/2).
        let o1 = s.on_event(ack(&[0]), 5 * MS);
        assert_ne!(
            s.state(),
            SenderState::PreConverge,
            "ilk SACK cold-start'ı bitirir"
        );
        let recov = o1
            .iter()
            .filter(|d| matches!(d.kind, TransmitKind::Duplicate | TransmitKind::Retransmit))
            .count();
        assert_eq!(
            recov, 0,
            "temiz cold-start'ta spurious recovery YOK (dup+retx={recov})"
        );
    }

    // ── DR-S5-3 N8-SPOT: send-record↔ACK-eşleme BtlBw'ye SIZMAZ (sayısal bit-özdeşlik) ──
    // Aynı DATA-senaryosu iki-kez: (ref) sent_logical-boş → mapping NO-OP · (mapped) acked-seq'ler
    // sent_logical'a konur → record_confirmed_logical FIRES. BtlBw BİT-ÖZDEŞ olmalı (eşleme BtlBw-yolunu
    // değiştirmez; yeni-satır yalnız ayrı-defter). Sender ACK-yolu deterministik (BTreeMap; HashSet-YOK).
    fn warmup_btlbw(map_acked: bool) -> (u64, usize) {
        let mut s = Sender::new();
        for i in 0..8u64 {
            let out = s.on_event(Event::AppData(unit(i)), i * 10 * MS);
            let seqs: Vec<u64> = out.iter().map(|d| d.seq).collect();
            if map_acked {
                // mapping'i ateşle: bu seq'leri logical-deftere kaydet (ACK'te record_confirmed_logical firar).
                for &sq in &seqs {
                    s.sent_logical.insert(
                        sq,
                        SrLogicalRef::Fec {
                            block_id: 1,
                            esi: 0,
                        },
                    );
                }
            }
            if !seqs.is_empty() {
                s.on_event(ack(&seqs), i * 10 * MS + 5 * MS);
            }
        }
        (s.diag_rate().1, s.diag_confirmed_logical_count())
    }

    #[test]
    fn dr_s5_3_mapping_does_not_change_btlbw() {
        let (btlbw_ref, cl_ref) = warmup_btlbw(false); // mapping NO-OP
        let (btlbw_mapped, cl_mapped) = warmup_btlbw(true); // mapping FIRES
        assert_eq!(
            btlbw_ref, btlbw_mapped,
            "N8: send-record↔ACK-eşleme BtlBw'yi DEĞİŞTİRMEZ (bit-özdeş: {btlbw_ref} == {btlbw_mapped})"
        );
        assert_eq!(
            cl_ref, 0,
            "ref: mapping-fire-etmedi (confirmed_logical boş)"
        );
        assert!(
            cl_mapped > 0,
            "mapped: mapping-fire-etti (confirmed_logical dolu) → yine BtlBw-özdeş"
        );
    }

    // ── DR-S5-3: confirmed_logical yalnız FEC/control-teyidi (DATA-değil) + drenaj ──
    #[test]
    fn dr_s5_3_confirmed_logical_take() {
        let mut s = Sender::new();
        // Data emit et, GERÇEK emitted-seq'leri yakala, onları logical-deftere koy + ACK'le.
        let mut emitted: Vec<u64> = Vec::new();
        for i in 0..4u64 {
            let out = s.on_event(Event::AppData(unit(i)), i * 10 * MS);
            emitted.extend(out.iter().map(|d| d.seq));
        }
        assert!(!emitted.is_empty(), "en-az-bir seq emit");
        for &sq in &emitted {
            s.sent_logical.insert(
                sq,
                SrLogicalRef::Fec {
                    block_id: 7,
                    esi: sq as u16,
                },
            );
        }
        s.on_event(ack(&emitted), 100 * MS);
        let taken = s.take_confirmed_logical();
        assert_eq!(
            taken.len(),
            emitted.len(),
            "her acked-emitted logical-teyit"
        );
        assert!(taken.iter().all(|(_, r, src)| {
            matches!(r, SrLogicalRef::Fec { block_id: 7, .. }) && *src == ConfirmSource::WireAck
        }));
        assert_eq!(s.diag_confirmed_logical_count(), 0, "take drenajladı");
    }

    // ── DR-S5-3 BOUNDED: drain-yoksa cap'te en-eski düşer (SR-AUDIT-001 unbounded-alloc-yok) ──
    #[test]
    fn dr_s5_3_confirmed_logical_bounded() {
        let mut s = Sender::new();
        // CAP+50 kez ateşle (drenaj YOK) → len tavanı-aşmaz.
        for seq in 0..(CONFIRMED_LOGICAL_CAP as u64 + 50) {
            s.sent_logical.insert(
                seq,
                SrLogicalRef::Fec {
                    block_id: 0,
                    esi: 0,
                },
            );
            s.record_confirmed_logical(seq);
        }
        assert_eq!(
            s.diag_confirmed_logical_count(),
            CONFIRMED_LOGICAL_CAP,
            "bounded: cap-aşılmaz (drain-yoksa en-eski düşer)"
        );
    }

    // ── DR-S5-4: RECOVERED → retransmit-cancel + retention-release + Recovered-marker'lı defter ──
    #[test]
    fn dr_s5_4_recovered_cancels_and_ledger() {
        let mut s = Sender::new();
        // pending-retransmit + bekleyen-dup + sent_logical kaydı olan bir unit kur (direct-state).
        s.pending_lost.push_back(LostRec {
            seq: 5,
            unit: OpaqueUnit { id: 42, size: 1200 },
            deadline_ns: 0,
            retx_count: 0,
            send_ns: 0,
        });
        s.dup_queue.push(DupRec {
            due_ns: 0,
            unit: OpaqueUnit { id: 42, size: 1200 },
            interval_ns: 0,
        });
        s.sent_logical.insert(
            5,
            SrLogicalRef::Fec {
                block_id: 9,
                esi: 2,
            },
        );

        let out = s.on_event(
            Event::RecoveredControl(RecoveredInfo {
                units: vec![RecoveredUnit {
                    unit_id: 42,
                    block_id: 9,
                    esi: 2,
                }],
            }),
            77 * MS,
        );
        assert!(out.is_empty(), "saf-state: RECOVERED emit-üretmez");
        assert!(s.pending_lost.is_empty(), "(i) retransmit-CANCEL");
        assert!(s.dup_queue.is_empty(), "(ii) retention-RELEASE");
        assert_eq!(s.diag_recovered_cancels(), 1, "K2/M1-sinyali saydı");
        let taken = s.take_confirmed_logical();
        assert_eq!(
            taken,
            vec![(
                5,
                SrLogicalRef::Fec {
                    block_id: 9,
                    esi: 2
                },
                ConfirmSource::Recovered
            )],
            "(iii) defter Recovered-marker'lı (WireAck'ten ayrık)"
        );
        // Kayıtsız-recovered (sent_logical'da olmayan) → defter-NO-OP, cancel-sayacı-artmaz.
        s.on_event(
            Event::RecoveredControl(RecoveredInfo {
                units: vec![RecoveredUnit {
                    unit_id: 999,
                    block_id: 1,
                    esi: 0,
                }],
            }),
            78 * MS,
        );
        assert_eq!(
            s.diag_recovered_cancels(),
            1,
            "bilinmeyen-unit cancel-saymaz"
        );
        assert_eq!(
            s.diag_confirmed_logical_count(),
            0,
            "bilinmeyen-logical defter-NO-OP"
        );
    }

    // ── DR-S5-4 §3 N8-BİT-ÖZDEŞLİK (ZORUNLU): recovered fires-vs-noop → BtlBw BİT-ÖZDEŞ ──
    /// Aynı DATA+loss senaryosu; `fires` → sondaki RECOVERED-olayı ateşlenir. Dönen:
    /// (btlbw, recovered_cancels, confirmed_logical_len, pending_lost_önce).
    fn warmup_recovered(fires: bool) -> (u64, u64, usize, usize) {
        let mut s = Sender::new();
        // Faz-1: temiz warm-up (BtlBw kurulsun; tüm-emitted ACK'lenir).
        for i in 0..30u64 {
            let out = s.on_event(Event::AppData(unit(i)), i * 10 * MS);
            let acks: Vec<u64> = out.iter().map(|d| d.seq).collect();
            if !acks.is_empty() {
                s.on_event(ack(&acks), i * 10 * MS + 5 * MS);
            }
        }
        // Faz-2: kuyruk-sonu-kayıp — 10 unit emit, YALNIZ-en-yüksek-seq ACK'lenir →
        // packet-threshold öncekileri kayıp-ilan eder; pacer-burst yalnız-birkaçını hemen-retransmit
        // eder → kalanlar pending_lost'ta (deterministik; iki-koşuda özdeş).
        let mut tail: Vec<u64> = Vec::new();
        for j in 0..10u64 {
            let out = s.on_event(Event::AppData(unit(100 + j)), (300 + j) * MS);
            tail.extend(out.iter().map(|d| d.seq));
        }
        if let Some(&top) = tail.iter().max() {
            s.on_event(ack(&[top]), 315 * MS);
        }
        // İki-koşuda da AYNI ön-durum: pending_lost'taki unit'ler + sent_logical kayıtları.
        let lost: Vec<(u64, u64)> = s.pending_lost.iter().map(|r| (r.seq, r.unit.id)).collect();
        let units: Vec<RecoveredUnit> = lost
            .iter()
            .enumerate()
            .map(|(k, &(sq, uid))| {
                s.sent_logical.insert(
                    sq,
                    SrLogicalRef::Fec {
                        block_id: 1,
                        esi: k as u16,
                    },
                );
                RecoveredUnit {
                    unit_id: uid,
                    block_id: 1,
                    esi: k as u16,
                }
            })
            .collect();
        let pending_before = s.pending_lost.len();
        if fires {
            s.on_event(Event::RecoveredControl(RecoveredInfo { units }), 305 * MS);
        }
        // Post-fire saf-tick'ler (ACK-yok → btlbw iki-kolda-da değişmemeli).
        for t in 0..5u64 {
            s.on_event(Event::TimerFired(Timer::Pto), (310 + t * 10) * MS);
        }
        (
            s.diag_rate().1,
            s.diag_recovered_cancels(),
            s.diag_confirmed_logical_count(),
            pending_before,
        )
    }

    // ── DR-S5-6: register_sent_logical (DATA-defter) — bounded/evict/miss + WireAck-teyit ──
    #[test]
    fn dr_s5_6_register_bounded_evict_miss() {
        let mut s = Sender::new();
        // CAP+50 register (drenajsız) → len==CAP, evicted==50; en-eski (en-küçük-seq) düştü.
        for seq in 0..(SENT_LOGICAL_CAP as u64 + 50) {
            s.register_sent_logical(
                seq,
                SrLogicalRef::Fec {
                    block_id: 1,
                    esi: 0,
                },
            );
        }
        let (len, evicted, miss0) = s.diag_sent_logical();
        assert_eq!(len, SENT_LOGICAL_CAP, "bounded: cap-aşılmaz");
        assert_eq!(evicted, 50, "aşan-50 en-eski-drop (audit)");
        assert_eq!(miss0, 0);
        // Evict-edilmiş seq'e (0..50) teyit-denemesi → defter-girişi YOK + miss-artefakt-sayacı artar
        // (şart-2: M1 bu-artefaktı GÖRÜR, sessizce-eğilmez).
        s.record_confirmed_logical(10); // evicted-bölge
        assert_eq!(
            s.diag_confirmed_logical_count(),
            0,
            "evicted-seq teyit-alamaz"
        );
        assert_eq!(s.diag_sent_logical().2, 1, "miss-artefakt sayıldı");
        // Yaşayan-seq teyit-alır (WireAck).
        s.record_confirmed_logical(SENT_LOGICAL_CAP as u64 + 10);
        assert_eq!(s.diag_confirmed_logical_count(), 1);
    }

    // ── DR-S5-6 ŞART-1: N8-bit-özdeşlik DATA-versiyonu — register-fires-vs-noop → BtlBw BİT-ÖZDEŞ ──
    /// Aynı DATA+ACK senaryosu; `fires` → her-DATA-emisyonu PUBLIC register_sent_logical ile deftere
    /// kaydedilir (engine-post-emit örüntüsü). Dönen: (btlbw, WireAck-girişi-sayısı).
    fn warmup_data_register(fires: bool) -> (u64, usize) {
        let mut s = Sender::new();
        for i in 0..8u64 {
            let out = s.on_event(Event::AppData(unit(i)), i * 10 * MS);
            let seqs: Vec<u64> = out.iter().map(|d| d.seq).collect();
            if fires {
                for &sq in &seqs {
                    // engine-post-emit: DATA-source logical-kimliği (block=2, esi=seq).
                    s.register_sent_logical(
                        sq,
                        SrLogicalRef::Fec {
                            block_id: 2,
                            esi: sq as u16,
                        },
                    );
                }
            }
            if !seqs.is_empty() {
                s.on_event(ack(&seqs), i * 10 * MS + 5 * MS);
            }
        }
        let wireack = s
            .take_confirmed_logical()
            .iter()
            .filter(|(_, _, src)| *src == ConfirmSource::WireAck)
            .count();
        (s.diag_rate().1, wireack)
    }

    #[test]
    fn dr_s5_6_n8_data_register_does_not_change_btlbw() {
        let (btlbw_noop, wa_noop) = warmup_data_register(false);
        let (btlbw_fires, wa_fires) = warmup_data_register(true);
        assert_eq!(wa_noop, 0, "noop: defter-girişi yok");
        assert!(
            wa_fires > 0,
            "fires: DATA-teyitleri deftere girdi (WireAck)"
        );
        assert!(
            btlbw_noop > 0,
            "dejenerasyon-guard: 0==0 trivial-eşitlik değil (BtlBw kuruldu)"
        );
        // ŞART-1 HÜKMÜ: DATA-send-record da (FEC-parity gibi) BtlBw'ye SIZMAZ — bit-özdeş.
        // (S5-3/DR-S5-4 N8-spot ailesinin DATA-üyesi.)
        assert_eq!(
            btlbw_fires, btlbw_noop,
            "N8-DATA: register_sent_logical BtlBw'yi DEĞİŞTİRMEZ ({btlbw_fires} == {btlbw_noop})"
        );
    }

    #[test]
    fn dr_s5_4_n8_recovered_does_not_change_btlbw() {
        let (btlbw_noop, canc_noop, cl_noop, pend_noop) = warmup_recovered(false);
        let (btlbw_fires, canc_fires, cl_fires, pend_fires) = warmup_recovered(true);
        // Vacuity-korumaları: senaryo gerçekten kayıp-üretti (iki-koşuda aynı ön-durum).
        assert!(
            pend_noop > 0 && pend_fires > 0,
            "pending_lost dolu (kayıp gerçek)"
        );
        assert_eq!(
            pend_noop, pend_fires,
            "iki-koşu aynı ön-durum (deterministik)"
        );
        // fires: cancel gerçekleşti + defter doldu; noop: hiçbiri.
        assert!(canc_fires > 0, "fires: retransmit-cancel gerçekleşti");
        assert_eq!(canc_noop, 0, "noop: cancel-yok");
        assert!(
            cl_fires > cl_noop,
            "fires: recovered→confirmed_logical DOLDU (muhasebe)"
        );
        // §3 HÜKÜM: recovered-fires BtlBw'yi DEĞİŞTİRMEZ (kredi-yolu AÇILMADI — bit-özdeş).
        assert_eq!(
            btlbw_fires, btlbw_noop,
            "N8: RECOVERED BtlBw'ye bit-eklemez (bit-özdeş: {btlbw_fires} == {btlbw_noop})"
        );
    }

    // ══ Commit-G (DR-1a): ratio→r golden/property + same-event-guard + otorite-API + ordering ══

    #[test]
    fn dr_1a_ratio_to_r_golden_k8() {
        // §8/§16: K=8 boundary-golden (transition-öncesi / exact-half / sonrası + uçlar + clamp).
        // r_unclamped=(ratio*8+500)/1000; half-up: ratio=62→(496+500)/1000=0; 63→(504+500)=1.004→1.
        let cases: [(u32, u16); 12] = [
            (0, 0),
            (62, 0),
            (63, 1),
            (187, 1),
            (188, 2),
            (437, 3),
            (438, 4),
            (562, 4),
            (563, 5),
            (937, 7),
            (938, 8),
            (1000, 8),
        ];
        for (ratio, want_computed) in cases {
            let (computed, clamped) = compute_requested_r(ratio, 8, 4);
            assert_eq!(computed, want_computed, "ratio={ratio}");
            assert_eq!(clamped, want_computed.min(4), "clamp@Rmax=4 ratio={ratio}");
        }
        // Profil-clamp-başlangıcı: computed>4 ilk-nokta ratio=563 (r=5→clamp-4).
        assert_eq!(compute_requested_r(563, 8, 4), (5, 4));
        // ratio=1000 → min(k_actual, r_max).
        assert_eq!(compute_requested_r(1000, 8, 4), (8, 4));
        assert_eq!(compute_requested_r(1000, 3, 4), (3, 3));
    }

    #[test]
    fn dr_1a_ratio_to_r_property_exhaustive() {
        // Property (deterministik-tam-tarama; RNG-yok): ∀ratio∈[0,1000], k∈{1..=8,255}:
        // formül-eş + monoton + clamp + ratio0→0 + ratio1000→min(k,rmax) + overflow-yok.
        for k in (1u8..=8).chain([255]) {
            let mut prev = 0u16;
            for ratio in 0u32..=1000 {
                let (computed, clamped) = compute_requested_r(ratio, k, 4);
                let want = ((u64::from(ratio) * u64::from(k) + 500) / 1000) as u16;
                assert_eq!(computed, want);
                assert!(computed >= prev, "monoton k={k} ratio={ratio}");
                assert!(clamped <= 4);
                prev = computed;
            }
            assert_eq!(compute_requested_r(0, k, 4).1, 0);
            assert_eq!(compute_requested_r(1000, k, 4).1, u16::from(k).min(4));
        }
        // k_actual=1 / K-1 / K kenarları (partial-block-formülü nominal-K-KULLANMAZ):
        assert_eq!(compute_requested_r(500, 1, 4), (1, 1)); // (500+500)/1000=1
        assert_eq!(compute_requested_r(500, 7, 4), (4, 4)); // (3500+500)/1000=4
        assert_eq!(compute_requested_r(500, 8, 4), (4, 4)); // 4500/1000=4 (exact; clamp-örneği 563→(5,4)-üstte)
    }

    #[test]
    fn dr_1a_authority_api_and_manual_reject() {
        let mut s = Sender::new();
        // Default: Manual{enabled:false} (mevcut-davranış).
        assert_eq!(
            s.diag_fec_actuation().authority_mode,
            FecActuationMode::Manual { enabled: false }
        );
        // Manual'da toggle Ok.
        s.set_manual_fec_enabled(true).unwrap();
        assert_eq!(
            s.diag_fec_actuation().authority_mode,
            FecActuationMode::Manual { enabled: true }
        );
        // Controller'a geçiş; manuel-toggle TYPED-red + state-DEĞİŞMEZ + sayaç.
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        assert_eq!(
            s.set_manual_fec_enabled(false),
            Err(FecAuthorityError::NotInManualMode)
        );
        assert_eq!(
            s.diag_fec_actuation().authority_mode,
            FecActuationMode::Controller
        );
        assert_eq!(s.diag_fec_actuation().manual_toggle_rejected_count, 1);
        // Controller→Manual{false}: açık-belirtim (gizli-eski-state-yok).
        s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
            .unwrap();
        s.set_manual_fec_enabled(true).unwrap();
    }

    #[test]
    fn dr_1a_latch_guards_initial_mismatch_norule_emergency() {
        let mut s = Sender::new();
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        // 4.3 INITIAL: hiç-decision-yok → LITERAL-0 + sayaç.
        let r = s.fec_controller_latch(s.open_fec_block_authority(), None, 8, 4);
        assert_eq!(
            (r.planned_r, r.suppression),
            (0, Some(FecSuppressionReason::NoEligibleDecision))
        );
        assert_eq!(s.diag_fec_actuation().no_eligible_decision_count, 1);
        // Canlı-decision üret (event-sür).
        s.on_event(Event::AppData(unit(0)), 0);
        let snap = s.controller_decision_snapshot().unwrap();
        // 4.2 EVENT-MISMATCH: bilerek-eski-index → LITERAL-0 + sayaç + panik-yok + eski-karar-kullanılmaz.
        let stale = ControllerDecisionSnapshot {
            event_index: snap.event_index.wrapping_sub(1),
            ..snap
        };
        let r = s.fec_controller_latch(s.open_fec_block_authority(), Some(stale), 8, 4);
        assert_eq!(
            (r.planned_r, r.suppression),
            (0, Some(FecSuppressionReason::DecisionEventMismatch))
        );
        assert!(!r.same_event_eligible);
        assert_eq!(s.diag_fec_actuation().decision_event_mismatch_count, 1);
        // 4.4 NO-RULE: literal-0 (fec_zero-config'e-BAĞLI-DEĞİL) + NoRule-reason.
        let nr = ControllerDecisionSnapshot {
            no_rule: true,
            ..snap
        };
        let r = s.fec_controller_latch(s.open_fec_block_authority(), Some(nr), 8, 4);
        assert_eq!(
            (r.planned_r, r.suppression),
            (0, Some(FecSuppressionReason::NoRule))
        );
        // EMERGENCY-interlock: requested-GÖRÜNÜR, planned=0.
        let em = ControllerDecisionSnapshot {
            emergency: true,
            ratio_permille: 500,
            ..snap
        };
        let r = s.fec_controller_latch(s.open_fec_block_authority(), Some(em), 8, 4);
        assert_eq!(r.computed_r, 4);
        assert_eq!(r.planned_r, 0);
        assert_eq!(
            r.suppression,
            Some(FecSuppressionReason::EmergencyPendingArbitration)
        );
        // 4.1 NORMAL: same-event → planned=clamped.
        let ok = ControllerDecisionSnapshot {
            ratio_permille: 500,
            ..snap
        };
        let r = s.fec_controller_latch(s.open_fec_block_authority(), Some(ok), 8, 4);
        assert!(r.same_event_eligible);
        assert_eq!((r.computed_r, r.planned_r, r.suppression), (4, 4, None));
        // ManualDisabled: Manual-mode'da-latch → 0.
        s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
            .unwrap();
        let r = s.fec_controller_latch(s.open_fec_block_authority(), Some(ok), 8, 4);
        assert_eq!(
            (r.planned_r, r.suppression),
            (0, Some(FecSuppressionReason::ManualDisabled))
        );
    }

    #[test]
    fn dr_1a_ordering_fuzzy_before_emission_and_recovered_cannot_close() {
        // §4 ORDERING: on_event → fuzzy(index++/snapshot) → ... → schedule(emisyon). Kanıt:
        // event-sonrası snapshot.event_index == fusion-index VE aynı-index'le-latch eligible.
        let mut s = Sender::new();
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        let out = s.on_event(Event::AppData(unit(0)), 0);
        let _ = out; // emisyonlar-bu-event'te; block-close engine-tarafı-AYNI-bağlamda.
        let snap = s.controller_decision_snapshot().unwrap();
        let r = s.fec_controller_latch(s.open_fec_block_authority(), Some(snap), 8, 4);
        assert!(
            r.same_event_eligible,
            "fuzzy latch'ten-ÖNCE koştu (aynı-event-index)"
        );
        // §4.5 RecoveredControl: fuzzy-KOŞMAZ (index-değişmez) VE emisyon-ÜRETMEZ → blok-KAPATAMAZ
        // (block-close yalnız-emisyon-decision'larından türer; boş-dönüş → latch-tetiklenemez).
        let idx_before = s.controller_decision_snapshot().unwrap().event_index;
        let out = s.on_event(
            Event::RecoveredControl(RecoveredInfo { units: vec![] }),
            10 * MS,
        );
        assert!(
            out.is_empty(),
            "RecoveredControl emisyon-üretmez → blok-kapatamaz"
        );
        assert_eq!(
            s.controller_decision_snapshot().unwrap().event_index,
            idx_before,
            "RecoveredControl fuzzy-koşturmaz (index-sabit)"
        );
        // Ve-o-bağlamda-latch-yapılsaydı-bile: snapshot-hala-önceki-event'in → guard-değerlendirmesi
        // index-eşitliğiyle: eşit (yeni-fuzzy-yok) → normal-akış-zararsız; guard anormal-yolları-hedefler.
    }

    // ── Commit-I §8.2: counter-saturation (wrap-YOK; debug/release-özdeş) ──
    #[test]
    fn dr_1a_counter_saturation_boundary() {
        let mut s = Sender::new();
        s.transition_applied_count = u64::MAX - 1;
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        assert_eq!(s.diag_fec_actuation().transition_applied_count, u64::MAX);
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        assert_eq!(
            s.diag_fec_actuation().transition_applied_count,
            u64::MAX,
            "saturating: MAX'ta-kalır"
        );
        s.no_rule_zero_count = u64::MAX;
        let snap = ControllerDecisionSnapshot {
            ratio_permille: 0,
            event_index: s.fusion_event_index,
            no_rule: true,
            emergency: false,
        };
        let _ = s.fec_controller_latch(s.open_fec_block_authority(), Some(snap), 8, 4);
        // Manual-default'ta ManualDisabled-dalı no_rule-sayacına-girmez; Controller'a-alıp-tekrar:
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        let _ = s.fec_controller_latch(s.open_fec_block_authority(), Some(snap), 8, 4);
        assert_eq!(
            s.diag_fec_actuation().no_rule_zero_count,
            u64::MAX,
            "no-wrap"
        );
    }

    // ── Commit-J §8.3: last_latch TAM-ÖZDEŞ-overwrite (carryover-YOK) ──
    #[test]
    fn dr_1a_last_latch_full_overwrite_no_carryover() {
        let mut s = Sender::new();
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        s.on_event(Event::AppData(unit(0)), 0);
        let snap = s.controller_decision_snapshot().unwrap();
        // 1. latch: SUPPRESSED (NoRule; requested-görünür).
        let nr = ControllerDecisionSnapshot {
            no_rule: true,
            ratio_permille: 700,
            ..snap
        };
        let r1 = s.fec_controller_latch(s.open_fec_block_authority(), Some(nr), 8, 4);
        assert_eq!(r1.suppression, Some(FecSuppressionReason::NoRule));
        assert_eq!(s.diag_fec_actuation().last_latch, Some(r1));
        // 2. latch: TEMİZ — last_latch TAM-yeni-değer; suppression-carryover-YOK.
        let ok = ControllerDecisionSnapshot {
            ratio_permille: 500,
            ..snap
        };
        let r2 = s.fec_controller_latch(s.open_fec_block_authority(), Some(ok), 8, 4);
        assert_eq!(r2.suppression, None, "temiz-latch");
        assert_eq!(
            s.diag_fec_actuation().last_latch,
            Some(r2),
            "TAM-overwrite: alan-alan-yeni-latch (önceki-suppression/ratio-taşınmadı)"
        );
        assert_ne!(r1, r2, "iki-latch-farklı → overwrite-gözlemlenebilir");
        // 3. latch: tekrar-SUPPRESSED — yine-tam-yeni (çift-yönlü-kanıt).
        let r3 = s.fec_controller_latch(s.open_fec_block_authority(), Some(nr), 8, 4);
        assert_eq!(s.diag_fec_actuation().last_latch, Some(r3));
        assert_eq!(r3.suppression, Some(FecSuppressionReason::NoRule));
    }

    // ── Commit-D §4.3: snapshot = controller-input CANONICAL-eşitliği ──
    #[test]
    fn fusion_snapshot_equals_controller_input() {
        let mut s = Sender::new();
        assert!(s.diag_fusion_input_snapshot().is_none(), "başlangıç: None");
        // Birkaç-event sür (loss/qd/ackconf-yolları-dolaşsın).
        for i in 0..6u64 {
            let out = s.on_event(Event::AppData(unit(i)), i * 10 * MS);
            let seqs: Vec<u64> = out.iter().map(|d| d.seq).collect();
            if i % 2 == 0 && !seqs.is_empty() {
                s.on_event(ack(&seqs), i * 10 * MS + 5 * MS);
            }
        }
        let snap = s.diag_fusion_input_snapshot().expect("evaluate-koştu");
        // CANONICAL-kanıt: snapshot-inputs'uyla decide → last_decision BİT-ÖZDEŞ
        // (iki-bağımsız-hesap-değil; aynı-değerin-kopyası olduğunun-fonksiyonel-kanıtı).
        let replay = DeterministicFuzzyController::fixture().decide(&snap.to_inputs());
        assert_eq!(
            replay,
            s.last_decision(),
            "snapshot→decide == last_decision"
        );
        assert_eq!(snap.no_rule, s.last_decision().no_rule);
        assert!(snap.event_index > 0);
        // Default-nötrlük: snapshot-okuma state-değiştirmez (ikinci-okuma-özdeş).
        assert_eq!(s.diag_fusion_input_snapshot(), Some(snap));
    }

    // ── DR-1e-v2 §3.1: default-equivalence — new() ≡ try_with_fuzzy_config(fixture) ──
    #[test]
    fn dr_1e_default_constructor_equivalence() {
        let mut a = Sender::new();
        let mut b = Sender::try_with_fuzzy_config(FuzzyConfig::fixture()).unwrap();
        let mut all_a: Vec<TransmitDecision> = Vec::new();
        let mut all_b: Vec<TransmitDecision> = Vec::new();
        // Aynı-event-dizisi: DATA + kuyruk-sonu-kayıp + ACK + Pto (fusion/rate/mode-yollarını gezer).
        for i in 0..12u64 {
            let (oa, ob) = (
                a.on_event(Event::AppData(unit(i)), i * 10 * MS),
                b.on_event(Event::AppData(unit(i)), i * 10 * MS),
            );
            let seqs: Vec<u64> = oa.iter().map(|d| d.seq).collect();
            all_a.extend(oa);
            all_b.extend(ob);
            if i % 3 != 2 && !seqs.is_empty() {
                all_a.extend(a.on_event(ack(&seqs), i * 10 * MS + 5 * MS));
                all_b.extend(b.on_event(ack(&seqs), i * 10 * MS + 5 * MS));
            }
        }
        for t in 0..20u64 {
            all_a.extend(a.on_event(Event::TimerFired(Timer::Pto), (200 + t * 50) * MS));
            all_b.extend(b.on_event(Event::TimerFired(Timer::Pto), (200 + t * 50) * MS));
        }
        // Kararlar + emisyon-dizisi + sayaçlar BİT-ÖZDEŞ (davranışsal-özdeşlik-kümesi).
        assert_eq!(
            all_a, all_b,
            "TransmitDecision-dizisi (scheduling/pacing) özdeş"
        );
        assert_eq!(
            a.last_decision(),
            b.last_decision(),
            "fusion-decision özdeş"
        );
        assert_eq!(a.diag_rate(), b.diag_rate(), "rate/btlbw/pif özdeş");
        assert_eq!(a.diag_loss(), b.diag_loss(), "loss-sayaçları özdeş");
        assert_eq!(
            a.diag_fuzzy_no_rule(),
            b.diag_fuzzy_no_rule(),
            "no_rule-sayacı özdeş"
        );
    }

    #[test]
    fn ack_frees_inflight_and_updates_rtt() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.inflight_bytes(), 1200);
        s.on_event(ack(&[0]), 50 * MS);
        assert_eq!(s.inflight_bytes(), 0);
        assert!(s.rtt.srtt().is_some());
    }

    #[test]
    fn ack_confidence_low_enters_recover_fwd_and_duplicates() {
        // ACK-yolu çöküşü (p5_revpath): DATA gönder, HİÇ ACK gelme → RECOVER_FWD + ACK-bağımsız
        // duplication. Baseline'da QUIC c(10s)=0.00 olduğu nokta; engine forward-recovery yapıyor.
        let mut s = Sender::new();
        // warm-up: PRE_CONVERGE'den çık (§3.3-D) — RECOVER_FWD/emergency POST-cold-start davranışı.
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        assert_ne!(
            s.state(),
            SenderState::PreConverge,
            "warm-up cold-start'ı bitirmeli"
        );
        // artık ACK gelmiyor → ack_confidence düşer → RECOVER_FWD (§11)
        s.on_event(Event::AppData(unit(1)), 20 * MS);
        s.on_event(Event::AppData(unit(2)), 21 * MS);
        let mut saw_fwd = false;
        let mut saw_escalate = false;
        let mut dup_count = 0usize;
        // [commit-C uyumu] eşik artık max(3×srtt, 2×EAI≈4.2s) → LOW için sessizlik ≈14s+ gerek.
        for t in 1..=25 {
            let out = s.on_event(Event::TimerFired(Timer::Pto), (t * 1_000 + 21) * MS);
            if s.state() == SenderState::RecoverFwd {
                saw_fwd = true;
            }
            if s.emergency_level() > EmergencyLevel::Arq {
                saw_escalate = true;
            }
            dup_count += out
                .iter()
                .filter(|d| d.kind == TransmitKind::Duplicate)
                .count();
        }
        assert!(saw_fwd, "ACK yok → RECOVER_FWD'e girmeli (§11)");
        assert!(
            dup_count > 0,
            "RECOVER_FWD ACK-bağımsız DUPLICATE emit etmeli (dup={dup_count})"
        );
        assert!(saw_escalate, "emergency bir noktada escalate etmeli");
    }

    // Küçük birim → burst'e çok paket sığar; düşük loss oranı (fuzzy R1: loss LOW → arq_light>0).
    fn small(id: u64) -> OpaqueUnit {
        OpaqueUnit { id, size: 100 }
    }

    #[test]
    fn low_loss_with_healthy_acks_retransmits() {
        // ACK'ler akıyor (confidence OK) + DÜŞÜK loss (%10) → RECOVER_ARQ → selective retransmit (§9).
        // (Yüksek loss + düşük bw'de fuzzy ARQ'yu baskılar = FEC-dominant, doğru — o ayrı senaryo.)
        let mut s = Sender::new();
        // warm-up: PRE_CONVERGE'den çık + BtlBw kur (MTU birim → burst kanal-türevli ≥10 küçük birime yeter).
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        // 10 küçük paket (burst): seq1..10 (warm-up seq0 tüketti).
        for i in 0..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS);
        }
        // seq2..10 ACK'li, seq1 DEĞİL → packet-threshold (≥3 yüksek acked) → seq1 kayıp. loss %10.
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS);
        let retx = out
            .iter()
            .filter(|d| d.kind == TransmitKind::Retransmit)
            .count();
        assert!(
            retx > 0,
            "düşük loss + ACK-canlı → RECOVER_ARQ retransmit (retx={retx}, arq_aggr={})",
            s.last_decision().arq_aggressiveness_permille
        );
        assert_eq!(s.state(), SenderState::RecoverArq);
    }

    #[test]
    fn retransmit_references_same_unit_id() {
        // Retransmit AYNI opak birimi (unit_id) yeni seq ile taşımalı (SoC: transport birimi bilir).
        let mut s = Sender::new();
        // warm-up: PRE_CONVERGE'den çık + BtlBw kur (MTU birim → burst 10 küçük birime yeter).
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(42)), 20 * MS); // seq1 = unit 42
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS); // seq2..10
        }
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS); // seq1 kayıp
        let retx = out.iter().find(|d| d.kind == TransmitKind::Retransmit);
        assert_eq!(
            retx.map(|d| d.unit_id),
            Some(42),
            "retransmit kayıp birimin unit_id'sini (42) yeni seq ile taşımalı"
        );
        assert_ne!(
            retx.map(|d| d.seq),
            Some(1),
            "retransmit YENİ seq almalı (kayıp seq1 değil)"
        );
    }

    #[test]
    fn drains_to_idle_when_quiescent() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(ack(&[0]), 30 * MS);
        assert_eq!(s.state(), SenderState::Idle);
    }

    #[test]
    fn tail_loss_liveness_without_loss_classification() {
        // [TUR-1a / D-1/1] Tail kaybı: daha yüksek seq ACK'i YOK → time-threshold loss SINIFLANDIRAMAZ
        // (saf sessizlik loss metriğini ARTIRMAZ); recovery LIVENESS'tan gelir (silence → ack_confidence
        // → RECOVER_FWD → spekülatif geometrik Duplicate; ACK'le iptal edilir, loss sayılmaz).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0); // warm-up seq0
        s.on_event(ack(&[0]), 10 * MS); // srtt=10ms; PreConverge'den çık
        s.on_event(Event::AppData(unit(42)), 20 * MS); // tail: seq1 = unit 42, ACK GELMEYECEK
        let mut dup_seen = false;
        // [commit-C uyumu] EAI-tabanı conf-düşüşünü geciktirir → sessizlik-penceresi 30s'e uzatıldı.
        for t in 1..=30 {
            let out = s.on_event(Event::TimerFired(Timer::Pto), (20 + t * 1_000) * MS);
            dup_seen |= out
                .iter()
                .any(|d| d.kind == TransmitKind::Duplicate && d.unit_id == 42);
        }
        let (lpkt, ltime, pend) = s.diag_loss();
        assert_eq!(
            (lpkt, ltime, pend),
            (0, 0, 0),
            "saf sessizlik loss metriğini artırmamalı (later-ACK preconditionı)"
        );
        assert_eq!(s.loss.loss_pre_fec_ppm(), 0, "loss_ppm da 0 kalmalı");
        assert!(
            dup_seen,
            "tail liveness: RECOVER_FWD spekülatif kopyayı ateşlemeli (unit 42)"
        );
    }

    #[test]
    fn clean_pair_flow_no_time_threshold_loss() {
        // [TUR-1a] Temiz akış, SACK'ler cadence gecikmesiyle: precondition sayesinde time-threshold
        // ASLA ateşlemez (K-1 false-loss sınıfı yapısal ölür). ACK'ler her birimi kapsıyor.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        // 4 birim; SACK'ler GEÇ (cadence) ama tam — her ack'ten önce uzun PTO sessizliği.
        for (i, send_ms) in [(1u64, 20u64), (2, 30), (3, 40), (4, 50)] {
            s.on_event(Event::AppData(unit(i)), send_ms * MS);
        }
        // 3s boyunca PTO tick'leri (eski kod burada hepsini "kayıp" ilan ederdi: 9/8×10ms ≪ 3s).
        for t in 1..=30 {
            s.on_event(Event::TimerFired(Timer::Pto), (50 + t * 100) * MS);
        }
        // Geç ama TAM SACK.
        s.on_event(ack(&[1, 2, 3, 4]), 3200 * MS);
        let (lpkt, ltime, _) = s.diag_loss();
        assert_eq!(
            (lpkt, ltime),
            (0, 0),
            "temiz-ama-geç SACK akışında hiçbir loss sınıflandırılmamalı"
        );
    }

    #[test]
    fn unloss_cancels_pending_retransmit_and_raises_tolerance() {
        // [TUR-2a D-1/3] UN-LOSS: packet-threshold'la kayıp ilan edilen seq'e GEÇ ACK gelirse
        // pending-retransmit İPTAL edilir (hipotez çürüdü) + reorder-toleransı yükselir
        // (note_spurious kablosu) + windowed loss_ppm'e SAYILMAZ.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS); // PreConverge'den çık
        for i in 1..=5u64 {
            s.on_event(Event::AppData(unit(i)), (10 + i) * MS);
        }
        // seq1 atlanır, 2..5 ack'lenir → gap≥3 → seq1 packet-threshold Suspected + pending'e girer.
        // [TUR-7 β2 hizalama] backlog token-tavanından BÜYÜK + ilan-tick'i %4-dışı
        // (token'ı DATA yer; amaç: un-loss iptali, retx değil).
        for i in 6..=23u64 {
            s.on_event(Event::AppData(unit(100 + i)), (16 + i) * MS);
        }
        s.on_event(ack(&[2, 3, 4, 5]), 100 * MS);
        {
            let (lpkt, _, pend) = s.diag_loss();
            assert_eq!(
                (lpkt, pend),
                (1, 1),
                "seq1 kayıp ilan edilip kuyruğa girmeli"
            );
        }
        // GEÇ ACK seq1 (reorder'mış meğer) → un-loss: pending boşalır, retransmit YOK.
        s.on_event(ack(&[1]), 200 * MS);
        let (unloss, lppm, spur) = s.diag_ledger();
        let (_, _, pend) = s.diag_loss();
        assert_eq!(pend, 0, "un-loss pending-retransmit'i iptal etmeli");
        assert_eq!(unloss, 1);
        assert_eq!(spur, 1);
        assert_eq!(lppm, 0, "Spurious windowed-ppm'e sayılmaz");
        assert!(
            s.reorder.recent_max_reorder() >= 4,
            "note_spurious gözlenen reorder mesafesini kaydetmeli (la=5 − seq=1)"
        );
    }

    #[test]
    fn probe_up_contract_entry_success_and_gates() {
        // [TUR-3c D-2/4] Giriş-kapıları (periyot+qd-histerezis+backlog) gerçek-akışla; success-dalı
        // donuk-snapshot mekaniğiyle (fizik-entegrasyonu m1-netem ölçümünün işi).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS);
        for i in 1..=6u64 {
            s.on_event(Event::AppData(unit(i)), (500 + i) * MS);
        }
        s.state = SenderState::Steady;
        let epoch = dur_ns(
            s.rtt
                .srtt()
                .unwrap_or(INITIAL_RTT)
                .max(s.expected_ack_interval()),
        );
        // Periyot dolmadan: giriş YOK (bu event token-birikimiyle kuyruğu da boşaltabilir).
        s.on_event(Event::TimerFired(Timer::Pto), 4 * epoch - 100 * MS);
        assert_ne!(
            s.state(),
            SenderState::ProbeUp,
            "periyot dolmadan giriş yok"
        );
        // Backlog tazele (erken-Pto emisyonu kuyruğu boşaltmış olabilir); periyot hâlâ dolmadı.
        s.state = SenderState::Steady;
        for i in 7..=10u64 {
            s.on_event(Event::AppData(unit(i)), 4 * epoch - 50 * MS + i);
        }
        assert_ne!(
            s.state(),
            SenderState::ProbeUp,
            "backlog var ama periyot dolmadı"
        );
        // Periyot dolunca: GİRER ve aynı-event'te ÇIKMAZ (N=0-ölümü; qd=0 histerezis-içi).
        s.state = SenderState::Steady;
        // Periyot CANLI-epoch'la ölçülür; erken-Pto pertürbasyonu send_rate'i düşürüp EAI'yi
        // büyütür (ölçüldü: 2.44s→3.10s) → t_e pertürbasyon-SONRASI canlı-epoch'tan türetilir.
        let live_epoch = dur_ns(
            s.rtt
                .srtt()
                .unwrap_or(INITIAL_RTT)
                .max(s.expected_ack_interval()),
        );
        let t_e = 4 * live_epoch + 100 * MS;
        s.on_event(Event::TimerFired(Timer::Pto), t_e);
        assert_eq!(
            s.state(),
            SenderState::ProbeUp,
            "kapılar açık → giriş; aynı-event çıkış YOK"
        );
        assert!(
            s.probe_btlbw_entry > 0 && s.probe_epoch_entry_ns > 0,
            "giriş-snapshot'ları donuk"
        );
        // SUCCESS-dal-mekaniği: snapshot'ları teste-uygun küçült → sıradaki ack hem volume'ü
        // hem +ε-eşiğini sağlar. (Arm-sırası: success qd-abort'tan ÖNCE değerlendirilir —
        // bayat-seq ack'inin dürüst-qd sıçraması success'i gölgeleyemez.)
        s.probe_btlbw_entry = 10;
        s.probe_epoch_entry_ns = 1_000_000;
        s.probe_delivered_entry = 0;
        s.on_event(ack(&[1]), t_e + 50 * MS);
        assert_eq!(s.state(), SenderState::Steady, "success → Steady");
        assert_eq!(s.diag_probe(), (1, 0, 0, 0), "SUCCESS ayrı-raporlanır");
    }

    #[test]
    fn probe_up_timeout_and_preempt_bookkeeping() {
        // [TUR-3c] timeout + preemption (4. tür) muhasebesi. Dwell-sürüyor kontrolleri
        // rtt-örneksiz Pto'yla (bayat-seq ack'i test-zaman-çizgisinde sahte-qd üretir → abort).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS);
        for i in 1..=6u64 {
            s.on_event(Event::AppData(unit(i)), (500 + i) * MS);
        }
        s.state = SenderState::Steady;
        let epoch = dur_ns(
            s.rtt
                .srtt()
                .unwrap_or(INITIAL_RTT)
                .max(s.expected_ack_interval()),
        );
        let t_e = 4 * epoch + 100 * MS;
        s.on_event(Event::TimerFired(Timer::Pto), t_e);
        assert_eq!(s.state(), SenderState::ProbeUp, "ilk giriş");
        // TIMEOUT: volume erişilmez (btlbw_entry↑↑ → success imkânsız) + dwell kısa (0.8s).
        s.probe_btlbw_entry = 1_000_000_000;
        s.probe_epoch_entry_ns = 200 * MS;
        s.on_event(Event::TimerFired(Timer::Pto), t_e + 100 * MS);
        assert_eq!(s.state(), SenderState::ProbeUp, "dwell içinde sürer");
        s.on_event(Event::TimerFired(Timer::Pto), t_e + 900 * MS);
        assert_eq!(s.state(), SenderState::Steady, "timeout → Steady");
        assert_eq!(s.diag_probe(), (0, 0, 1, 0), "timeout AYRI raporlanır");
        // PREEMPT: pending doluşu (packet-threshold) ProbeUp-kolunu update_state-önceliğiyle
        // KESER → muhasebe post-match yakalayıcıda (4. tür). Bu noktada 21s+ sessizlik var →
        // kapı-girişi meşru-emergency'ye takılır (giriş zaten yukarıda test edildi) → probe
        // İMAL edilir (donuk-snapshot yaklaşımıyla tutarlı; kesilme-mekaniği ölçülür).
        let t2 = t_e + 900 * MS + 5 * epoch;
        s.state = SenderState::ProbeUp;
        s.probe_entry_ns = Some(t2);
        s.probe_epoch_entry_ns = epoch;
        s.probe_btlbw_entry = 1_000_000_000; // success erişilmez → tek çıkış preemption
        s.probe_delivered_entry = s.bw_sampler.delivered();
        s.probe_loss_attributed = false;
        // seq2 (giriş-ÖNCESİ gönderim) atlanır; 3,4,5 ack → gap≥3 → pending → preemption.
        // Ack conf'u tazeler (emergency değil); probe_loss_attributed KALIR-false
        // (send_ns < entry_ns — Confirmed-lag koruması).
        s.on_event(ack(&[3, 4, 5]), t2 + 50 * MS);
        assert_eq!(s.state(), SenderState::RecoverArq, "pending → preemption");
        assert!(
            !s.probe_loss_attributed,
            "giriş-öncesi kayıp probe'a YAZILMAZ"
        );
        assert_eq!(
            s.diag_probe().3,
            1,
            "preemption 4. tür olarak muhasebeleşir"
        );
    }

    #[test]
    fn inv_rate_1_loss_declaration_preserves_phase_gain() {
        // [TUR-7 β1 KİLİT-GOLDEN — tez-koruması, Abkarada karar-2] "loss bizde rate düşürmez"
        // recovery-DAHİL: Startup-rampı sırasında kayıp-ilanı (pending>0 → RecoverArq-overlay)
        // faz-gain'ini DEĞİŞTİRMEZ — rate 2.885×btlbw KALIR (eski-dünya: 1.0×'a düşerdi;
        // fiili ×2.885 rate-kesimi = INV-RATE-1 kör-noktası, cift-kilit-v1 §0-ÖN/S1).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS); // btlbw≈2400; PreConverge→Startup (base=Startup)
        for i in 1..=8u64 {
            s.on_event(Event::AppData(unit(i)), (500 + i) * MS);
        }
        for t in [700u64, 900, 1_100] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS); // emisyonlar (seq 1..)
        }
        assert!(s.base_is_startup, "kurulum: ramp sürüyor");
        let btlbw = s.btlbw.btlbw();
        let rate_before = s.pacer.rate();
        assert_eq!(
            rate_before,
            crate::rate::Gain::STARTUP.apply(btlbw),
            "kurulum: Startup-gain aktif"
        );
        // KAYIP-İLANI: seq1 atlanır, 3 üst-ack → packet-threshold → pending>0 → RecoverArq.
        s.on_event(ack(&[2, 3, 4]), 1_200 * MS);
        assert_eq!(
            s.state(),
            SenderState::RecoverArq,
            "kayıp-ilanı → recovery-overlay"
        );
        let btlbw_after = s.btlbw.btlbw();
        assert_eq!(
            s.pacer.rate(),
            crate::rate::Gain::STARTUP.apply(btlbw_after),
            "KİLİT: kayıp-ilanı faz-gain'ini DEĞİŞTİRMEZ (rate = 2.885×btlbw KALIR)"
        );
        // Pending boşalınca çıkış BASE'e (Startup'a — Steady'ye değil; ramp ölmez).
        s.on_event(ack(&[1]), 1_300 * MS);
        assert_eq!(
            s.state(),
            SenderState::Startup,
            "recovery-çıkışı base'e döner"
        );
    }

    #[test]
    fn arq_share_emits_retx_under_token_starvation() {
        // [TUR-7 β2] BOŞ-NÖBET biter: DATA-backlog doluyken bile her K_ARQ_SHARE'inci grant
        // retx'i öne alır → retx wire'a çıkar (eski-dünya: DATA-önce tüm token'ı yer, retx=0).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS);
        for i in 1..=30u64 {
            s.on_event(Event::AppData(unit(i)), (500 + i) * MS); // kalıcı DATA-backlog
        }
        for t in [700u64, 900, 1_100] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        }
        s.on_event(ack(&[2, 3, 4]), 1_200 * MS); // seq1 → pending (packet-threshold)
        assert_eq!(s.state(), SenderState::RecoverArq);
        assert!(!s.pending_lost.is_empty(), "kurulum: pending var");
        let mut retx_seen = false;
        let mut t = 1_220 * MS;
        for _ in 0..40 {
            let out = s.on_event(Event::TimerFired(Timer::Pto), t);
            if out.iter().any(|d| d.kind == TransmitKind::Retransmit) {
                retx_seen = true;
                break;
            }
            t += 20 * MS;
        }
        assert!(
            retx_seen,
            "β2: token-darlığında bile retx wire'a çıkar (BOŞ-NÖBET biter)"
        );
    }

    #[test]
    fn plateau_round_is_supply_independent() {
        // [TUR-6] AYNI round içinde N-fazla ack-OLAYI plato-sayacını İLERLETEMEZ:
        // round yalnız marker-sonrası gönderimin İLK ACK'iyle döner (delivered-anchor).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS);
        for i in 1..=6u64 {
            s.on_event(Event::AppData(unit(i)), (500 + i) * MS);
        }
        for t in [700u64, 900, 1_100, 1_300] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS); // emisyonlar
        }
        s.state = SenderState::Startup;
        s.startup_stable_rounds = 0;
        s.startup_last_btlbw = s.btlbw.btlbw();
        // Marker'ı İLERİYE koy (bu seq henüz gönderilMEDİ) → eldeki tüm ack'ler round-öncesi.
        s.startup_round_marker_seq = s.next_seq + 100;
        s.startup_round_complete = false;
        // Zengin-arz simülasyonu: 3 AYRI ack-olayı (eski sayaç 3 round sayardı → plato).
        s.on_event(ack(&[1]), 1_400 * MS);
        s.on_event(ack(&[2]), 1_450 * MS);
        s.on_event(ack(&[3]), 1_500 * MS);
        assert_eq!(
            s.startup_stable_rounds, 0,
            "marker-öncesi ack'ler round İLERLETMEZ"
        );
        assert_eq!(s.state(), SenderState::Startup, "plato tetiklenmedi");
    }

    #[test]
    fn startup_reentry_resets_plateau_counter() {
        // [TUR-6, §16.2 satır-yarısı] Idle→Startup yeniden-girişi bayat sayaç/marker taşımaz.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS);
        s.startup_stable_rounds = STARTUP_PLATEAU_ROUNDS; // bayat-plato durumu
        s.startup_round_complete = true;
        s.state = SenderState::Idle;
        s.on_event(Event::AppData(unit(9)), 10_000 * MS); // Idle→Startup (srtt var)
        assert_eq!(s.state(), SenderState::Startup);
        assert_eq!(s.startup_stable_rounds, 0, "re-entry sayaç-sıfırlar");
        assert!(!s.startup_round_complete, "re-entry round-bayrağı sıfırlar");
        // Marker giriş-ANI next_seq'ine konur (aynı-event emisyonu SONRA seq alır → o birim
        // marker-sonrası sayılır = BBR-doğru; kesin-eşitlik post-emisyon next_seq'le değil).
        assert_eq!(
            s.startup_round_marker_seq, 1,
            "marker giriş-anı next_seq'i (0-değil)"
        );
    }

    #[test]
    fn ack_confidence_eai_floor_prevents_false_low() {
        // [TUR-3b commit-C] 3×srtt ≪ EAI iken sessizlik < K_CONF_EAI×EAI conf'u DÜŞÜRMEZ.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS); // srtt=10ms → 3×srtt=30ms ≪ EAI
        s.on_event(Event::AppData(small(1)), 20 * MS); // aktif (seq1 uçuşta)
        let eai = dur_ns(s.expected_ack_interval());
        assert_eq!(
            s.ack_confidence_permille(20 * MS + eai),
            1000,
            "EAI-tabanı içinde sessizlik conf düşürmez (eski: 30ms eşikte çoktan LOW)"
        );
        assert!(
            s.ack_confidence_permille(20 * MS + 4 * eai) < 1000,
            "taban aşılınca sessizlik yine sinyaldir"
        );
    }

    #[test]
    fn startup_exit_enters_drain_then_steady_on_bdp() {
        // [TUR-3b] Plato → DRAIN (Steady değil); çıkış inflight≤BDP (byte-eşiği, saat-sınıfsız).
        // (max-dwell yedeği DrainRtprop-testinde aynı formülle sınanıyor — tekrar edilmez.)
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS); // btlbw≈2400B/s, rtprop=0.5s → BDP≈1200B (tek-paket matematiği)
        for i in 1..=3u64 {
            s.on_event(Event::AppData(unit(i)), (600 + i) * MS); // 3×MTU kuyruğa
        }
        // Pacer-refill tikleriyle emisyonları tamamla (rate≈6.9kB/s → ~173ms/birim).
        for t in [800u64, 1_000, 1_200] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        }
        assert!(
            s.inflight_bytes() > 1_300,
            "kurulum: uçuş BDP'nin üstünde olmalı (emisyonlar tamam)"
        );
        // Plato-koşullarını kur ve tetikle: → DRAIN.
        s.startup_last_btlbw = s.btlbw.btlbw();
        s.startup_stable_rounds = STARTUP_PLATEAU_ROUNDS - 1;
        s.bw_sampled_since_round = true;
        s.startup_round_complete = true; // [TUR-6] delivered-anchor round-kapısı
        s.state = SenderState::Startup;
        s.on_event(Event::TimerFired(Timer::Pto), 1_300 * MS);
        assert_eq!(
            s.state(),
            SenderState::Drain,
            "plato → Drain (BBR-analog; overfill Steady'ye taşınmaz)"
        );
        // Drain'de KALIR (inflight > BDP; dwell dolmadı).
        s.on_event(Event::TimerFired(Timer::Pto), 1_400 * MS);
        assert_eq!(s.state(), SenderState::Drain, "inflight>BDP → Drain sürer");
        // Boşalt (seq3 uçuşta kalır: 1200B ≤ BDP sınır-eşitliği + aktiflik) → Steady.
        s.on_event(ack(&[1, 2]), 1_500 * MS);
        assert_eq!(
            s.state(),
            SenderState::Steady,
            "inflight≤BDP → Drain-çıkışı (byte-eşiği)"
        );
    }

    #[test]
    fn drain_rtprop_contract_dwell_and_exit_invariant() {
        // [TUR-3b D-2/3] VERIFIED N=0 bulgusu ölür: giriş-event'inde ÇIKMAZ; exit-invariant =
        // (elapsed≥min_dwell ∧ qualifying) ∨ elapsed≥max_dwell. Giriş = pencere-doluşu.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 3_000 * MS); // srtt=3s (conf-eşiği 9s — dwell sırasında sahte-emergency yok)
        s.on_event(Event::AppData(small(1)), 3_100 * MS); // seq1 (qualifying-ack adayı)
        s.on_event(Event::AppData(small(9)), 3_200 * MS); // seq2 — HİÇ ack'lenmez: aktiflik-tutucu
        s.state = SenderState::Steady; // plato-kurulumunu atla (sözleşme-odaklı birim-test)

        // Pencere-doluşu ÖNCESİ: giriş REDDedilir (yaş < W/2).
        s.on_event(Event::TimerFired(Timer::RtpropProbe), 5_000 * MS);
        assert_ne!(
            s.state(),
            SenderState::DrainRtprop,
            "taze rtprop → giriş yok"
        );

        // Doluş SONRASI (yaş ≥ W/2 = max(10s,8×3s)/2 = 12s; min-örnek t=3s → t≥15s): GİRER ve
        // AYNI event'te ÇIKMAZ (N=0 ölümü).
        s.state = SenderState::Steady;
        s.on_event(Event::TimerFired(Timer::RtpropProbe), 15_100 * MS);
        assert_eq!(
            s.state(),
            SenderState::DrainRtprop,
            "pencere-doluşu → giriş"
        );

        // min_dwell dolmadan qualifying-örnek: KALIR (invariantın sol-kolu iki-şartlı).
        let epoch = dur_ns(
            s.rtt
                .srtt()
                .unwrap_or(INITIAL_RTT)
                .max(s.expected_ack_interval()),
        );
        s.on_event(ack(&[1]), 15_100 * MS + epoch / 4); // qualifying=true; seq2 uçuşta (Idle yok)
        assert_eq!(
            s.state(),
            SenderState::DrainRtprop,
            "min_dwell dolmadan çıkış yok"
        );

        // min_dwell geçti + qualifying → herhangi bir event çıkışı işler (Pto tick).
        // NOT: qualifying-ack büyük örnekle (raw≈12.7s) srtt'yi kuantum-döngüde YUKARI taşıdı
        // (dürüst davranış) → min_dwell BÜYÜDÜ; epoch ack-SONRASI yeniden okunur.
        let epoch_post = dur_ns(
            s.rtt
                .srtt()
                .unwrap_or(INITIAL_RTT)
                .max(s.expected_ack_interval()),
        );
        s.on_event(
            Event::TimerFired(Timer::Pto),
            15_100 * MS + epoch_post + 1_000_000,
        );
        assert_eq!(
            s.state(),
            SenderState::Steady,
            "min_dwell ∧ qualifying → çıkış"
        );

        // max_dwell yedeği: qualifying'siz 3×epoch → çıkış (silence-conf ~680‰ > escalate 300‰).
        s.state = SenderState::DrainRtprop;
        s.drain_rtprop_entry_ns = Some(16_000 * MS);
        s.drain_rtprop_qualifying = false;
        s.on_event(
            Event::TimerFired(Timer::Pto),
            16_000 * MS + 3 * epoch_post + 1_000_000,
        );
        assert_eq!(
            s.state(),
            SenderState::Steady,
            "max_dwell yedeği (qualifying'siz)"
        );
    }

    #[test]
    fn tombstone_recovers_bw_and_rtt_from_late_ack() {
        // [TUR-2b D-1/4] Kayıp-ilanı kaydı DÜŞÜRMEZ (tombstone) → geç-ACK'te kol-1
        // (retx_count==0) BW-örneği + RTT KURTARILIR (eski davranış: sansür).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        for i in 1..=5u64 {
            s.on_event(Event::AppData(unit(i)), (10 + i) * MS);
        }
        // [TUR-7 β2 hizalama] backlog token-tavanından BÜYÜK tutulur (DATA tüm token'ı yer →
        // retx doğmaz; fixture'ın şişkin-btlbw'si token'ı deviriyor) + ilan-tick'i %4-dışı.
        for i in 6..=23u64 {
            s.on_event(Event::AppData(unit(100 + i)), (16 + i) * MS);
        }
        s.on_event(ack(&[2, 3, 4, 5]), 100 * MS); // seq1 → Suspected + tombstone
        {
            let (live, hits, ..) = s.diag_tombstone();
            assert_eq!((live, hits), (1, 0), "ilan tombstone'a yazılmalı");
        }
        let srtt_before = s.rtt.srtt().expect("srtt var");
        s.on_event(ack(&[1]), 500 * MS); // GEÇ-ACK → tombstone hit
        let (live, hits, bw_rec, evicted, ambig, credited) = s.diag_tombstone();
        assert_eq!(
            (live, hits, bw_rec, evicted, ambig, credited),
            (0, 1, 1, 0, 0, 0),
            "geç-ACK: hit + kol-1 BW-örneği kurtarılır (kredi kol-2'nin işi)"
        );
        assert!(
            s.bw_sampled_since_round,
            "kurtarılan BW-örneği plato-round'unu beslemeli"
        );
        // RTT: seq1 en-yüksek-yeni-acked Data → 500ms−11ms ≈ 489ms örneği srtt'yi YUKARI çeker.
        assert!(
            s.rtt.srtt().expect("srtt") > srtt_before,
            "geç-ACK RTT örneği (kol-1) işlenmeli"
        );
    }

    #[test]
    fn karn_superseded_late_ack_credits_without_samples() {
        // [TUR-2c D-1/3 kol-2] Retransmit EMİT edildikten SONRA gelen orijinal-ACK:
        // RTT ALINMAZ + rate-örneği ÜRETİLMEZ + delivered-KREDİSİ girer + ambiguous sayacı.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0); // warm-up (MTU) → BtlBw kur
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(42)), 20 * MS); // seq1 = unit 42
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS); // seq2..10
        }
        // seq1 packet-threshold kayıp + AYNI event'te retransmit emit (mevcut test-kalıbı).
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS);
        assert!(
            out.iter().any(|d| d.kind == TransmitKind::Retransmit),
            "kalıp gereği retransmit emit edilmeli"
        );
        // Superseded işareti: tombstone(seq1).retx_count arttı → kol-1'e düşemez.
        let srtt_before = s.rtt.srtt().expect("srtt");
        let btlbw_before = s.btlbw.btlbw();
        // GEÇ orijinal-ACK (seq1): kol-2.
        s.on_event(ack(&[1]), 400 * MS);
        let (_, hits, bw_rec, _, ambig, credited) = s.diag_tombstone();
        assert_eq!((hits, ambig, bw_rec), (1, 1, 0), "kol-2: örneksiz hit");
        assert!(
            credited > 0,
            "delivered-kredisi girmeli (byte'lar wire'ı geçti)"
        );
        assert_eq!(
            s.rtt.srtt().expect("srtt"),
            srtt_before,
            "kol-2: RTT örneği ALINMAZ (400ms'lik sahte örnek srtt'yi bozardı)"
        );
        assert_eq!(
            s.btlbw.btlbw(),
            btlbw_before,
            "kol-2: bu ACK'ten rate-örneği ÜRETİLMEZ"
        );
        // [review-fix §9.3] kanonik spurious (retx-SONRASI orijinal-ACK) reorder-toleransını yükseltir.
        assert!(
            s.reorder.recent_max_reorder() >= 9,
            "kol-2 note_spurious kablosu: gözlenen mesafe (la=10 − seq=1) kaydedilmeli"
        );
    }

    #[test]
    fn karn_both_transmissions_acked_wire_byte_semantics() {
        // [TUR-2c review-vektörü] Retransmit'in KENDİ ACK'i geldikten SONRA orijinal-geç-ACK:
        // retransmit inflight-yolundan TAM örnek verir (seq iletimi tekil tanımlar); orijinal
        // kol-2'den yalnız delivered-KREDİSİ alır. İKİ kopyanın da sayılması WIRE-BYTE
        // muhasebesidir (kapasite ölçümü, BBR semantiği) — "teslimat BİR KEZ" BİRİM-düzlemidir
        // (kopya-iptal + bench idempotent damga), iki düzlem karışmaz.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(42)), 20 * MS); // seq1
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS);
        }
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS);
        let retx_seq = out
            .iter()
            .find(|d| d.kind == TransmitKind::Retransmit)
            .expect("retransmit emit")
            .seq;
        // 1) Retransmit'in kendi ACK'i: normal inflight-yolu (örnekli).
        s.on_event(ack(&[retx_seq]), 60 * MS);
        let srtt_after_retx = s.rtt.srtt().expect("srtt");
        let btlbw_after_retx = s.btlbw.btlbw();
        // 2) Orijinal geç-ACK: kol-2 — örneksiz, KREDİLİ.
        s.on_event(ack(&[1]), 400 * MS);
        let (_, hits, bw_rec, _, ambig, credited) = s.diag_tombstone();
        assert_eq!((hits, ambig, bw_rec), (1, 1, 0), "kol-2 örneksiz hit");
        assert!(credited > 0, "wire-byte kredisi (kopya gerçekten geçti)");
        assert_eq!(s.rtt.srtt().expect("srtt"), srtt_after_retx, "RTT değişmez");
        assert_eq!(s.btlbw.btlbw(), btlbw_after_retx, "rate-örneği üretilmez");
    }

    #[test]
    fn tombstone_expiry_yields_no_sample() {
        // [TUR-2b] TTL geçti → prune (PTO) → çok-geç ACK örneksiz (Expired/Unknown).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        for i in 1..=5u64 {
            s.on_event(Event::AppData(unit(i)), (10 + i) * MS);
        }
        // Emisyon-flush: tüm birimler ACK'ten ÖNCE wire'a çıksın (pacer-token beklemesi
        // yüzünden geç-emit → phantom-ack + Pto'da MEŞRU yeni sınıflandırma test-amacını
        // bulandırıyordu; motor doğruydu, testin zaman-çizgisi özensizdi).
        for t in [20u64, 30, 40, 60, 80] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        }
        s.on_event(ack(&[2, 3, 4, 5]), 100 * MS); // seq1 tombstone'da (TTL = 8×T_loss)
        {
            let (live, ..) = s.diag_tombstone();
            assert_eq!(live, 1, "yalnız seq1 tombstone'da olmalı (flush sonrası)");
        }
        // TTL geçsin: T_loss ilan-anı ≈ cadence-taban ~3.2s → TTL ~25s; 400s fazlasıyla aşar.
        s.on_event(Event::TimerFired(Timer::Pto), 400_000 * MS);
        let (live, ..) = s.diag_tombstone();
        assert_eq!(live, 0, "TTL dolan kayıt prune edilmeli");
        s.on_event(ack(&[1]), 401_000 * MS);
        let (_, hits, bw_rec, ..) = s.diag_tombstone();
        assert_eq!((hits, bw_rec), (0, 0), "Expired → geç-ACK örneksiz");
        // [commit-B §3.7] prune uçuş-sayısını düşürmeli: pif == GERÇEK engine-uçuşu (burada
        // liveness-dup seq'i hâlâ uçuşta → eşitlik). Fix'siz pif = uçuş+1 kalırdı (sızıntı).
        assert_eq!(
            s.diag_pif(),
            s.inflight.len() as u64,
            "TTL-prune sonrası pif gerçek-uçuşa eşit olmalı (sızıntı-fix)"
        );
    }
}
