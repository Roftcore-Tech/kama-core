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
use crate::bw::{bw_bps, BtlBwFilter, DeliveryRateSampler, RateSample, SentPacket};
use crate::emergency::{
    self, EmergencyConfig, EmergencyLevel, Framing, HysteresisConfig, HysteresisController,
    RecoveryAction, RecoveryModeController,
};
use crate::fusion::{
    DeterministicFuzzyController, FusionController, FusionDecision, FusionInputs, Mode,
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
}

/// Transmit türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmitKind {
    /// tx_queue'dan yeni DATA.
    Data,
    /// ARQ selective retransmit (§9) — kayıp birim, yeni seq.
    Retransmit,
    /// forward-recovery duplication (§11) — ACK-bağımsız redundant kopya.
    Duplicate,
    /// FEC parity shard (§10) — POLİTİKA; transport üretmez (güvenlik katmanı encode eder). Emit EDİLMEZ.
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
/// [TUR-D1] Shrink-ateşleme: K_SHRINK ardışık-eligible-evidence-OLAYI (LOW_EVIDENCE) [CAL {3,5}].
/// [Y3-sim] delivered-EAI cadence-hesabı tipik birim-boyu.
const UNIT_SIZE_HINT: u64 = 267;
const K_SHRINK: u32 = 3;
/// [TUR-D1] Underdelivery-oranı: eligible-örnek < B_ref/K_RATIO → LOW-kanıt [CAL {2,4}].
const K_RATIO: u64 = 2;
/// [TUR-D1 P1] Taze-snapshot kapısı: ack_elapsed ≤ K_FRESH × olay-başı-ack_epoch [CAL {2,4}].
/// FAIL-OPEN (srtt-şişkin epoch kapıyı gevşetir) — ana-koruma verdict+olay-başına-max-1.
const K_FRESH: u32 = 2;
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

    // ── [TUR-D1] stale-btlbw reconciliation durumu ──
    wbtlbw_mode: u8,        // [Y3-sim] 0=legacy(srtt) 1=delivered-EAI 2=rtprop-swap
    shrink_count: u32,      // ardışık-LOW_EVIDENCE-olay sayacı (generation-izole)
    btlbw_generation: u64,  // HIGH-2: her max-yükseliş VE her reset yeni-nesil
    last_raise_ns: u64,     // F2 yaş-kapısı çıpası (olay-başı-snapshot'la okunur)
    last_low_event_ns: u64, // P6 olay-tanımı: LOW-sayım-aralığı ≥ olay-başı-ack_epoch
    shrink_low_max: u64,    // S2: sayılan-LOW-olayların eligible-düşük-örnek MAX'ı (reseed-tohumu)
    next_probe_ok_ns: u64,  // S4: shrink-reset-sonrası period-istisnası (0=pasif)
    diag_shrink_resets: u64,
    diag_stale_snapshot: u64, // P1: TUR-S-teşhis-sayacı (kopma-yayılımlı örnekler)
    diag_braise_elig: u64,    // P3: BASELINE_RAISED kaynak-ayrıştırması
    diag_braise_inelig: u64,

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
    last_recovery: RecoveryAction,
}

impl Sender {
    /// Fixture config ile (KALİBRE DEĞİL — golden/motor testi; gerçek değerler netem'den).
    #[must_use]
    pub fn new() -> Self {
        let budget_cfg = BudgetConfig::fixture();
        Self {
            state: SenderState::Idle,
            loss: ThresholdLossDetector::new(),
            rtt: RttEstimator::new(),
            bw_sampler: DeliveryRateSampler::new(),
            btlbw: BtlBwFilter::new(),
            fuzzy: DeterministicFuzzyController::fixture(),
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
            wbtlbw_mode: 0,
            shrink_count: 0,
            btlbw_generation: 0,
            last_raise_ns: 0,
            last_low_event_ns: 0,
            shrink_low_max: 0,
            next_probe_ok_ns: 0,
            diag_shrink_resets: 0,
            diag_stale_snapshot: 0,
            diag_braise_elig: 0,
            diag_braise_inelig: 0,
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
        let ack_epoch_ref = {
            let srtt_now = self.rtt.srtt().unwrap_or(INITIAL_RTT);
            let ack_epoch = srtt_now.max(self.expected_ack_interval());
            // [TUR-D1 EK-3 AÇIK-BORÇ] W=K_BW×ack_epoch'un srtt-dalı daralma-rejiminde ANTİ-FİX:
            // srtt-eğimi > 1/K_BW iken cutoff GERİLER → bayat-max ölümsüz (DV-3 ölçülü 0.92 s/s).
            // Shrink-reset SEMPTOMU keser, KÖK DURUR — W-türetim/M3-decoupling turunda kapanır;
            // "dedektör-var-diye-W-fix-gereksiz" SANILMAZ (m45 §4-S1).
            let w_epoch = match self.wbtlbw_mode {
                1 => {
                    let dbw = self.btlbw.last_sample_bw().max(1);
                    let term1 = Duration::from_nanos(2 * UNIT_SIZE_HINT * 1_000_000_000 / dbw);
                    (term1 + Duration::from_nanos(2_097_000_000)).max(INITIAL_RTT)
                }
                2 => self
                    .rtt
                    .rtprop()
                    .unwrap_or(INITIAL_RTT)
                    .max(self.expected_ack_interval()),
                _ => ack_epoch,
            };
            self.btlbw.set_window(w_epoch * K_BW);
            self.rtt.set_rtprop_window(srtt_now * K_RTPROP);
            self.rtt.set_qd_window(ack_epoch * K_QD); // [3a.5-α] rtprop-penceresinden sonra (≤ invariant)
                                                      // [3a.5-β] kuantum-döngü T'si — yalnız rtprop varken (PreConverge'de None → n=1).
            if let Some(rtp) = self.rtt.rtprop() {
                self.rtt.set_conv_interval(rtp / K_CONV);
            }
            ack_epoch
        };
        // [TUR-D1 HIGH-1] Atomik-event-verdict için OLAY-BAŞI snapshot'ları (loop-order-bağımsız).
        let shrink_b_ref = self.btlbw.btlbw();
        let shrink_g_ref = self.btlbw_generation;
        let shrink_raise_ref = self.last_raise_ns;
        let mut shrink_samples: Vec<RateSample> = Vec::new();

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
                let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample);
                self.btlbw.update(sample, now_ns, sample.delivered > 0);
                shrink_samples.push(sample); // [TUR-D1] olay-sonu verdict için
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
                    shrink_samples.push(sample); // [TUR-D1] olay-sonu verdict için
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
        // [TUR-D1] Olay-sonu atomik-verdict (tüm btlbw.update'ler bitti; HIGH-1/HIGH-2).
        let shrink_raised = self.btlbw.btlbw() > shrink_b_ref;
        self.shrink_apply_event(
            shrink_raised,
            shrink_b_ref,
            shrink_g_ref,
            ack_epoch_ref,
            shrink_raise_ref,
            &shrink_samples,
            now_ns,
        );

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
        d
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
                        >= epoch.saturating_mul(u64::from(K_PROBE_PERIOD_EPOCH))
                        // [TUR-D1 S4] shrink-reset-sonrası period-istisnası (YALNIZ period-
                        // bileşeni; qd_ok ∧ backlog BİT-AYNI — m45 P2/P4).
                        || (self.next_probe_ok_ns != 0 && now_ns >= self.next_probe_ok_ns);
                    // Giriş-histerezisi (workflow N=0-bulgusu): qd zaten abort-eşiği üstündeyse GİRME.
                    let qd_ok = self.qd_ratio_permille_now() <= PROBE_QD_ABORT_PERMILLE;
                    // DrainRtprop-çakışması çözümü: tazelik-KAPISI YOK (min-örnek yaşı sürekli-akışta
                    // bile büyür — kapı ProbeUp'ı yapısal bloklar, test+m1'de ölçüldü). DrainRtprop
                    // yalnız Steady'den girer → probe-dwell'i kadar bekler (sınırlı ≤4×epoch_entry).
                    let backlog = !self.tx_queue.is_empty();
                    if period_ok && qd_ok && backlog {
                        // GİRİŞ-SNAPSHOT'ları (donuk — öz-referans önlenir; ACK'ler işlendikten
                        // sonra alınır: on_event sırası bunu doğal sağlar).
                        self.next_probe_ok_ns = 0; // [TUR-D1 S4] istisna tek-kullanımlık
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
        out
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

    /// [TUR-D1, m45 §3] Stale-btlbw reconciliation — OLAY-SONU atomik-verdict (HIGH-1) +
    /// generation-izolasyonu (HIGH-2) + olay-tanımı (P6). Rol: "kesin-kanal-daralması-tespiti"
    /// DEĞİL, persistent-underdelivery karşısında konservatif btlbw-demotion. Örnek-kapıları:
    /// !app_limited ∧ gönderim-anı-gain≥1 (F3) ∧ yaş (F2: sent≥raise_ref) ∧ taze-snapshot
    /// (P1: ack_elapsed ≤ K_FRESH×epoch — aşan → TUR-S-teşhis-sayacı). Verdict-önceliği:
    /// BASELINE_RAISED > COUNTER_EVIDENCE > LOW_EVIDENCE > SKIP (yüksek-örnek-dominance).
    #[allow(clippy::too_many_arguments)]
    fn shrink_apply_event(
        &mut self,
        raised: bool,
        b_ref: u64,
        g_ref: u64,
        epoch_ref: Duration,
        raise_ref: u64,
        samples: &[RateSample],
        now_ns: u64,
    ) {
        let fresh_lim = epoch_ref.saturating_mul(K_FRESH);
        let mut any_elig_high = false;
        let mut any_elig_low = false;
        let mut any_elig_raiser = false;
        let mut low_max = 0u64;
        for sm in samples {
            if sm.ack_elapsed > fresh_lim {
                self.diag_stale_snapshot += 1; // kopma-yayılımlı → daralma-kanıtı DEĞİL (TUR-S girdisi)
                continue;
            }
            if sm.is_app_limited || !sm.gain_ge1 || sm.sent_ns < raise_ref {
                continue; // INELIGIBLE → ATLA (F4: sayaç-korunur)
            }
            let bw = bw_bps(sm.delivered, sm.interval);
            if bw > b_ref {
                any_elig_raiser = true;
            }
            if bw >= b_ref / K_RATIO {
                any_elig_high = true;
            } else {
                any_elig_low = true;
                low_max = low_max.max(bw);
            }
        }
        if raised {
            // BASELINE_RAISED — FİLTER-DÜZEYİ (P3: app-limited-yüksek-yükseliş DAHİL; kural-i
            // BU-verdict'le gerçeklenir, ayrı orta-döngü-hook YOK; generation++ yalnız burada).
            self.shrink_count = 0;
            self.shrink_low_max = 0;
            self.btlbw_generation += 1;
            self.last_raise_ns = now_ns;
            if any_elig_raiser {
                self.diag_braise_elig += 1;
            } else {
                self.diag_braise_inelig += 1;
            }
        } else if any_elig_high {
            // COUNTER_EVIDENCE — pozitif-kapasite-kanıtı dominance.
            self.shrink_count = 0;
            self.shrink_low_max = 0;
        } else if any_elig_low {
            // LOW_EVIDENCE — yalnız aynı-generation (P5/kural-iii) ∧ olay-aralığı ≥ epoch (P6).
            if g_ref == self.btlbw_generation
                && now_ns.saturating_sub(self.last_low_event_ns) >= dur_ns(epoch_ref)
            {
                self.shrink_count += 1;
                self.last_low_event_ns = now_ns;
                self.shrink_low_max = self.shrink_low_max.max(low_max);
            }
        } // else: SKIP — sayaç-korunur (ardışık-ELIGIBLE-EVIDENCE-olayı semantiği)

        if self.shrink_count >= K_SHRINK {
            // AKTÜATÖR (yalnız-düşürür): reseed = sayılan-LOW-olayların eligible-düşük MAX'ı (S2).
            self.btlbw.shrink_reset(self.shrink_low_max, now_ns);
            self.btlbw_generation += 1; // kural-ii: reset = yeni-nesil
            self.shrink_count = 0;
            self.shrink_low_max = 0;
            self.diag_shrink_resets += 1;
            // [S4] period-istisnası: ~1-epoch-sonra probe-eligible (qd_ok ∧ backlog bit-aynı).
            self.next_probe_ok_ns = now_ns + dur_ns(epoch_ref);
        }
    }

    /// §3.1 OnPacketSent muhasebe: seq ata, inflight+RTT/BW/loss kayıt. `origin` recovery türünü ayırır
    /// (DATA = orijinal birim → loss sayacı +1; retransmit/dup = recovery → +0, RTT'de Karn dışı).
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
        // DrainRtprop 0.75× / PreConverge floor) gain<1 → örnekleri shrink-kanıtı OLAMAZ.
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
    /// [Y3-sim] W_BTLBW saat-modu (deneysel; 0=legacy/1=delivered-EAI/2=rtprop-swap).
    pub fn set_wbtlbw_mode(&mut self, mode: u8) {
        self.wbtlbw_mode = mode;
    }

    /// [TUR-D1] Shrink-teşhis: (resets, count, stale_snapshot, braise_elig, braise_inelig).
    #[must_use]
    pub fn diag_shrink(&self) -> (u64, u32, u64, u64, u64) {
        (
            self.diag_shrink_resets,
            self.shrink_count,
            self.diag_stale_snapshot,
            self.diag_braise_elig,
            self.diag_braise_inelig,
        )
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

    // ═══ [TUR-D1] Stale-btlbw reconciliation vektörleri (m45 §6: 8-ek-vektör + r2-seti) ═══
    // Mekanizma-düzeyi: shrink_apply_event doğrudan sürülür (olay-sonu atomik-verdict).

    const SEC: u64 = 1_000_000_000;
    /// Eligible örnek: bw B/s (interval=1s → delivered=bw), taze, gain≥1, app_limited=false.
    fn rs_ok(bw: u64, sent_ns: u64) -> RateSample {
        RateSample {
            delivered: bw,
            interval: Duration::from_secs(1),
            is_app_limited: false,
            gain_ge1: true,
            ack_elapsed: Duration::from_millis(100),
            sent_ns,
        }
    }
    fn epoch1() -> Duration {
        Duration::from_secs(1)
    }
    /// n adet LOW-olayı sür (aralık 2×epoch — P6-uyumlu), b_ref sabit.
    fn drive_low(s: &mut Sender, n: u32, b_ref: u64, start_ns: u64) -> u64 {
        let mut now = start_ns;
        for _ in 0..n {
            s.shrink_apply_event(
                false,
                b_ref,
                s.btlbw_generation,
                epoch1(),
                0,
                &[rs_ok(b_ref / K_RATIO / 2, now)],
                now,
            );
            now += 2 * SEC;
        }
        now
    }

    #[test]
    fn d1_f3_stamp_is_send_time_not_ack_time() {
        // [MED-2 kapanışı] F3-PLUMBING: damga GÖNDERİM-anı fazından (PreConverge→false),
        // ack-anı-fazından DEĞİL. Regresyon-kalkanı: damga ack-anına kayarsa bu test kırılır.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0); // Idle→PreConverge: gain<1 → damga false
        let p0 = s.inflight.get(&0).unwrap().bw_sample;
        assert!(!p0.gain_ge1_stamp(), "PreConverge-gönderimi gain<1 damgalı");
        s.on_event(Event::AppData(unit(1)), MS);
        s.on_event(ack(&[0]), 50 * MS); // → Startup (faz-sınırı geçildi)
        assert_eq!(s.state(), SenderState::Startup);
        s.on_event(Event::AppData(unit(2)), 60 * MS); // Startup-gönderimi: gain≥1
        let p2 = s
            .inflight
            .values()
            .find(|p| p.unit.id == 2)
            .unwrap()
            .bw_sample;
        assert!(p2.gain_ge1_stamp(), "Startup-gönderimi gain≥1 damgalı");
        // seq1 (PreConverge-damgalı) Startup'ta ack'lenir → damga GÖNDERİM-anını korur:
        // örnek INELIGIBLE-sınıf (mekanizma-testleri bunu shrink_apply_event'te doğrular).
    }

    #[test]
    fn d1_v1_mixed_batch_dominance() {
        // Vektör-1: aynı-olayda düşük+yüksek eligible → COUNTER_EVIDENCE kazanır, count=0.
        let mut s = Sender::new();
        s.shrink_apply_event(
            false,
            4000,
            0,
            epoch1(),
            0,
            &[rs_ok(300, SEC), rs_ok(3000, SEC)],
            2 * SEC,
        );
        assert_eq!(s.shrink_count, 0, "yüksek-örnek dominance");
    }

    #[test]
    fn d1_v2_same_event_expansion_wins() {
        // Vektör-2: olay-içi btlbw-yükselişi (raised) → BASELINE_RAISED; count=0, generation++.
        let mut s = Sender::new();
        drive_low(&mut s, 2, 4000, SEC);
        assert_eq!(s.shrink_count, 2);
        let g0 = s.btlbw_generation;
        s.shrink_apply_event(
            true,
            4000,
            g0,
            epoch1(),
            0,
            &[rs_ok(5000, 9 * SEC), rs_ok(300, 9 * SEC)],
            10 * SEC,
        );
        assert_eq!((s.shrink_count, s.btlbw_generation), (0, g0 + 1));
        assert_eq!(s.diag_braise_elig, 1);
    }

    #[test]
    fn d1_v3_generation_isolation() {
        // Vektör-3: LOW,LOW → raise → LOW → trigger-YOK (sayaç eski-nesilden taşınmaz).
        let mut s = Sender::new();
        let now = drive_low(&mut s, 2, 4000, SEC);
        s.shrink_apply_event(
            true,
            4000,
            s.btlbw_generation,
            epoch1(),
            0,
            &[rs_ok(8000, now)],
            now,
        );
        assert_eq!(s.shrink_count, 0);
        // yeni-neslin İLK LOW'u — K_SHRINK'e ulaşmaz, reset YOK.
        s.shrink_apply_event(
            false,
            8000,
            s.btlbw_generation,
            epoch1(),
            s.last_raise_ns,
            &[rs_ok(500, now + 3 * SEC)],
            now + 4 * SEC,
        );
        assert_eq!((s.shrink_count, s.diag_shrink_resets), (1, 0));
    }

    #[test]
    fn d1_v4_skip_persistence_then_fire() {
        // Vektör-4 [beklenti-AÇIK-pinli]: LOW,LOW → uzun-SKIP (örneksiz olaylar) → LOW →
        // K_SHRINK=3 dolu → ATEŞLER (reconciliation-rolünde SKIP-boyunca-koruma KABUL — m45).
        let mut s = Sender::new();
        // filter'a yüksek-örnek koy (reset-sonrası btlbw düşecek — aktüatör-doğrulaması)
        s.btlbw.update(rs_ok(4000, 0), SEC, true);
        let mut now = drive_low(&mut s, 2, 4000, 2 * SEC);
        for _ in 0..5 {
            s.shrink_apply_event(false, 4000, s.btlbw_generation, epoch1(), 0, &[], now); // SKIP
            now += 2 * SEC;
        }
        assert_eq!(s.shrink_count, 2, "SKIP sayacı korur");
        s.shrink_apply_event(
            false,
            4000,
            s.btlbw_generation,
            epoch1(),
            0,
            &[rs_ok(900, now)],
            now,
        );
        assert_eq!(s.diag_shrink_resets, 1, "3. LOW ateşler");
        assert_eq!(
            s.btlbw.btlbw(),
            1000,
            "reseed = sayılan-LOW'ların eligible-düşük MAX'ı (S2)"
        );
        assert!(s.next_probe_ok_ns > 0, "S4 period-istisnası kuruldu");
    }

    #[test]
    fn d1_v5_b2_artifact_no_fire() {
        // Vektör-5: kapasite-sabit, yalnız-srtt-şişer → örnekler ≈ btlbw → COUNTER → tetik-YOK.
        let mut s = Sender::new();
        let mut now = SEC;
        for _ in 0..6 {
            s.shrink_apply_event(false, 4000, 0, epoch1(), 0, &[rs_ok(3900, now)], now);
            now += 2 * SEC;
        }
        assert_eq!((s.shrink_count, s.diag_shrink_resets), (0, 0));
    }

    #[test]
    fn d1_v6_ack_split_invariance() {
        // Vektör-6 [P6 olay-tanımı]: tek-teslim-patlaması 3-parça-ACK'e bölünse de (< epoch-aralıklı)
        // yalnız 1 LOW sayılır — cumulative-tek-ACK'le eşdeğer.
        let mut s = Sender::new();
        let base = 10 * SEC;
        for i in 0..3u64 {
            s.shrink_apply_event(
                false,
                4000,
                0,
                epoch1(),
                0,
                &[rs_ok(500, base)],
                base + i * (SEC / 10),
            );
        }
        assert_eq!(s.shrink_count, 1, "epoch-içi bölünmüş-ACK'ler tek-kanıt");
    }

    #[test]
    fn d1_v8_ineligible_spike_deferral() {
        // Vektör-8 [P3]: LOW,LOW → app-limited-şişkin-örnek filter'ı yükseltir (BASELINE_RAISED,
        // FİLTER-DÜZEYİ) → count=0 + diag_braise_inelig (eligible-raiser yok).
        let mut s = Sender::new();
        let now = drive_low(&mut s, 2, 4000, SEC);
        let spike = RateSample {
            is_app_limited: true,
            ..rs_ok(9000, now)
        };
        s.shrink_apply_event(true, 4000, s.btlbw_generation, epoch1(), 0, &[spike], now);
        assert_eq!((s.shrink_count, s.diag_braise_inelig), (0, 1));
    }

    #[test]
    fn d1_r2_gates_skip_not_reset() {
        // r2-seti [F4 geçersiz=ATLA]: app-limited / gain<1 (Drain'de-gönder-Steady'de-ack'le —
        // gönderim-anı-damgası) / bayat-yaş / bayat-snapshot örnekleri SAYACI SIFIRLAMAZ.
        let mut s = Sender::new();
        let now = drive_low(&mut s, 2, 4000, SEC);
        let applim = RateSample {
            is_app_limited: true,
            ..rs_ok(300, now)
        };
        let drained = RateSample {
            gain_ge1: false,
            ..rs_ok(300, now)
        }; // faz-sınırı-geçen-ack (anti-vacuous)
        let old_age = rs_ok(300, 0); // sent < raise_ref
        let stale = RateSample {
            ack_elapsed: Duration::from_secs(30),
            ..rs_ok(300, now)
        }; // > K_FRESH×epoch
        s.shrink_apply_event(
            false,
            4000,
            s.btlbw_generation,
            epoch1(),
            SEC / 2,
            &[applim, drained, stale],
            now + 2 * SEC,
        );
        s.shrink_apply_event(
            false,
            4000,
            s.btlbw_generation,
            epoch1(),
            SEC,
            &[old_age],
            now + 4 * SEC,
        );
        assert_eq!(s.shrink_count, 2, "INELIGIBLE=ATLA — sayaç korunur");
        assert_eq!(s.diag_stale_snapshot, 1, "bayat-snapshot TUR-S-sayacına");
    }

    #[test]
    fn d1_r2_after_fire_honest_bref_counters() {
        // Vektör-7-çekirdeği [P4 geçme-kriteri]: reset-sonrası B_ref dürüst → örnekler ≥ B_ref/K
        // → COUNTER → limit-cycle-YOK (yeniden-tetiklenme ancak GERÇEK-ikinci-daralmada).
        let mut s = Sender::new();
        s.btlbw.update(rs_ok(4000, 0), SEC, true);
        let now = drive_low(&mut s, 3, 4000, 2 * SEC); // ateşler (K_SHRINK=3)
        assert_eq!(s.diag_shrink_resets, 1);
        let b_new = s.btlbw.btlbw(); // dürüst-reseed (1000-sınıfı)
        let g = s.btlbw_generation;
        for i in 0..4u64 {
            s.shrink_apply_event(
                false,
                b_new,
                g,
                epoch1(),
                0,
                &[rs_ok(b_new, now + i * 2 * SEC)],
                now + i * 2 * SEC,
            );
        }
        assert_eq!(
            (s.shrink_count, s.diag_shrink_resets),
            (0, 1),
            "dürüst-B_ref COUNTER üretir"
        );
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
