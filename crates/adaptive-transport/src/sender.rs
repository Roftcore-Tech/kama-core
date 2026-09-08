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

#[cfg(feature = "experimental-cold-start")]
use std::collections::BTreeSet;
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
use crate::rate::{bdp_bytes, effective_send_rate, Gain, PacingPhase, SrcConfig, DEFAULT_MTU};
use crate::rtt::{apply_ack_delay, RttEstimator};
use crate::tail_recovery::{ProbeDecision, TailRecovery, TailRecoveryConfig};

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
    /// DR-CS-1 M1 (EK5-i): `Tick` ≡ `on_event_batch{now_ns, inbound=[]}` — tek-giriş-noktası,
    /// çift-semantik-yok. Adapter CS-wakeup'larında üretir. E2: due-exposed-timer VE bekleyen-
    /// scheduling-işi yoksa EXACT-NO-OP (schedule_transmit/drain çağrılmaz).
    #[cfg(feature = "experimental-cold-start")]
    Tick,
    /// DR-CS-1 M3 (F2B): authenticated CS-probe-echo (CTRL_CS_ECHO 0x1A; güvenlik-katmanı-açtı,
    /// engine/runner çevirdi). YALNIZ bootstrap-RTT: `(probe_seq)`-lookup → tek-tüketim →
    /// rtt-estimator (matematik-değişmez). cwnd/BtlBw/delivery/loss/ack-conf'a DOKUNMAZ (N8-spot).
    /// seen_count/arrival_gap SENDER'A-TAŞINMAZ (ledger-only alıcı/harness-defterinde; karar-
    /// girdisi-yapılamaz — amendman-§3).
    /// DR-CS-3: UNGATED — the `0x1A` codepoint already ships in `kama-transport-schema`.
    CsProbeEcho {
        /// Yansıtılan probe_seq (INV-CS-7 namespace).
        probe_seq: u16,
    },
}

/// DR-CS-1 M1: exposed-timer türleri (`next_wakeup_ns` sözleşmesine bağlı küme). Scope: CS-probe/
/// CS-giveup/(d)-PTO (F2A/F2B'de eklenir) + TEST-ONLY hook (F1 atomiklik-kanıtı; üretim-yolu-değil).
#[cfg(feature = "experimental-cold-start")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CsTimerKind {
    /// TEST-ONLY (DI-12E/F/G F1-sürümleri): fire-kaydı üretir, başka-etkisi-yok.
    TestHook(u8),
    /// M2 CS-probe merdiveni fire'ı (idx 0..N_PROBE; fire-anları 3/9/21/45s arm'dan-itibaren).
    CsProbe(u8),
    /// (d)-kapsamı: bootstrap-sonrası MEVCUT time-threshold makinesinin bir-sonraki deadline'ı
    /// (EK4-A2 yaşam-döngüsü: her-batch faz-2'de canlı-loss-state'ten yeniden-türetilir; fire =
    /// mevcut `Timer::Pto` core-yolu — YENİ-retransmit-yolu-YOK, INV-CS-4).
    /// SUPERSESSION (DR-ARQ-PRODUCT-TAIL-1, versioned TDD): `Timer::Pto` MAY now emit a bounded tail-loss probe
    /// (`emit_tail_probe`) — but ONLY when `TailRecovery` is explicitly enabled (product-config, default OFF). With
    /// the default OFF config this INV-CS-4 "no-Pto-retransmit" invariant holds byte-for-byte; the supersession is
    /// gated + additive, not silent. See docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD.md §4.
    BootstrapPto,
    /// M5 (F3): cold-start giveup deadline (T_COLD_GIVEUP; son-probe+RTT_MAX sonrası). Fire →
    /// resolved-değilse TimedOut-terminal + epoch-kapanışı.
    CsGiveup,
}

/// DR-CS-1 M5 (F3): cold-start bootstrap TERMINAL sonucu (INV-CS-3; output-API — input-Event'ten
/// AYRI). Epoch-başına EN-FAZLA-bir terminal (ResolvedBy*/TimedOut); TimedOut-sonrası DATA-ACK-
/// kurtarması `PostTimeoutRecovery` (İKİNCİ-terminal-DEĞİL). `take_cs_terminal` ile drain.
#[cfg(feature = "experimental-cold-start")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsTerminal {
    /// Hayatta-kalan DATA-ACK bootstrap-srtt kurdu.
    ResolvedByDataAck,
    /// CS-probe-echo bootstrap-srtt kurdu.
    ResolvedByProbeEcho,
    /// Giveup-deadline'a kadar çözülmedi (terminal-bildirim; oturum-zorunlu-kapanmaz).
    TimedOut,
    /// TimedOut-SONRASI DATA-ACK-kurtarması (ikinci-terminal-değil; heal-bildirimi).
    PostTimeoutRecovery,
}

/// K-8/F4: post-timeout recovery-kanıt türü (PostTimeoutNotice.cause).
#[cfg(feature = "experimental-cold-start")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostTimeoutCause {
    /// TimedOut-sonrası DATA-ACK-clocked kurtarma (300/1200B rejimi).
    DataAckClocked,
    /// TimedOut-sonrası §11 forward-recovery RECOVERED-control (65B-sınıfı ACK-bağımsız heal; B1-amendman).
    RecoveredControl,
}

/// K-8/F4 SENDER-tarafı POST-TIMEOUT NOTIFICATION CONTRACT: ürün-katmanına TimedOut-terminali-ALDIKTAN
/// SONRA bloğun kurtarıldığını bildirir (ikinci-terminal DEĞİL; CsTerminal DEĞİŞMEZ). one-slot (NOTICE_BOUND=1;
/// first-valid-proof-wins; duplicate/stale no-op). N8: BtlBw/credit'e SIFIR etki (SALT-LOGICAL bildirim).
#[cfg(feature = "experimental-cold-start")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostTimeoutNotice {
    /// Aktif bootstrap cold-epoch (MAX_ACTIVE_COLD_EPOCHS_PER_SESSION_DIRECTION=1).
    pub cold_epoch: u32,
    /// Kurtarılan BootstrapColdBlock (MAX_COLD_BLOCKS_PER_EPOCH=1).
    pub block_id: u32,
    /// İlk-geçerli-kanıt türü (first-valid-proof-wins).
    pub cause: PostTimeoutCause,
    /// Kanıt-zamanı (receiver-monotonic ns; teşhis).
    pub proof_time_ns: u64,
}

/// DR-CS-1 (F2B): bootstrap-srtt'nin kaynağı (INV-CS-3 terminal-sınıflandırma + diag).
#[cfg(feature = "experimental-cold-start")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsSrttSource {
    /// Hayatta-kalan DATA-ACK srtt'yi kurdu (ResolvedByDataAck).
    DataAck,
    /// CS-probe-echo srtt'yi kurdu (ResolvedByProbeEcho).
    ProbeEcho,
}

// ── DR-CS-1 M2 [CAL] sabitleri (F2A) ──
#[cfg(feature = "experimental-cold-start")]
const CS_PROBE_FIRES_NS: [u64; 4] = [3_000_000_000, 9_000_000_000, 21_000_000_000, 45_000_000_000];
#[cfg(feature = "experimental-cold-start")]
const CS_PROBE_WIRE_MAX: u64 = 44; // amp≤1 pad dahil (ECHO_WIRE_MAX=43B → probe≥43; F0-tespit-(i))
#[cfg(feature = "experimental-cold-start")]
const CS_PROBE_TX_HARD_BUDGET: u64 = 200; // nominal 4×44=176B (INV-CS-2; YALNIZ-forward-probe)
#[cfg(feature = "experimental-cold-start")]
const CS_GIVEUP_NS: u64 = 60_000_000_000; // T_COLD_GIVEUP=60s (son-probe-45s + RTT_MAX-5s + marj)

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

/// FS-5A (DR-FEC-FS-1) — PATH-TIMING SALT-GÖZLEM snapshot. Engine'in ZATEN ölçtüğü RTT/path-timing
/// state'inin ([`crate::rtt::RttEstimator`]) MUTLAK-DEĞER görünümü. `FusionInputs` yalnız ORAN taşıdığından
/// (jitter=rttvar/srtt, qd_ratio=queue_delay/rtprop) mutlak-magnitude FEC-selector'a ulaşmıyordu — bu
/// snapshot o PLUMBING-GAP'i kapatır (**yeni-ölçüm YOK; salt-forward**). Case-A(base≈220,qd düşük) vs
/// Case-B(base≈30,qd yüksek) AYNI-srtt'de `min_rtt_ns`+`queue_delay_ns` ile AYRIŞIR. **Production-policy
/// DEĞİŞMEZ:** fusion/`FusionInputs`/wire dokunulmadı; salt-okuma — experimental FEC-selector + FS-harness
/// girdisi. Üretim-çağıranı YOK (flag-OFF byte-özdeş; INV-CS-5-ruhu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathTimingState {
    /// Smoothed RTT (ns); henüz RTT-sample yoksa `None`.
    pub srtt_ns: Option<u64>,
    /// min/base RTT = RTprop (pencereli-min; ns); henüz sample yoksa `None`.
    pub min_rtt_ns: Option<u64>,
    /// RTT varyansı (ns).
    pub rttvar_ns: u64,
    /// Ayakta-kuyruk gecikmesi = min_W_qd(rtt) − rtprop (ns; ≥0; congestion sinyali, loss DEĞİL).
    pub queue_delay_ns: u64,
    /// Confidence: en az bir RTT-sample işlendi mi (srtt mevcut).
    pub sample_present: bool,
    /// Freshness: rtprop-min-örneği taze mi (yaş < pencere/2 → taze; kanal-saatli).
    pub rtprop_fresh: bool,
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

/// [TEŞHİS] Yapı-gate'in karar yolu. `None`/`Some` ikilisi "açmadı"yı tek bir kovaya
/// yığıyordu; NEDEN'i ayırt etmek için her çıkış noktası adlandırıldı. Sayımı çağıran yapar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome {
    /// Fuzzy zaten parity planlamış — gate devreye girmez (kapsam dışı).
    NotApplicable,
    /// Ledger'ın pencereli kaybı Q18 bandının altında (`< STRUCTURE_GATE_LOSS_MIN_PPM`).
    BlockedLossBelowBand,
    /// [KURULUŞ-GUARD] Oturum henüz kurulmadı (PreConverge) → ölçüm yok → yapısal karar YOK.
    BlockedPreConverge,
    /// Fate penceresi çok küçük (`n < STRUCTURE_GATE_MIN_N`).
    BlockedWindowTooSmall,
    /// Kayıp-geçişi kanıtı yetersiz (`n10+n11 < MIN_LOSS_TRANSITIONS`) — yalnız `BurstFusion`.
    BlockedNoTransitionEvidence,
    /// Burst yapı okundu ve sevk politikası işi ARQ'ya bıraktı.
    BlockedBurstLeftToArq,
    /// Gate açtı; taşınan değer parity oranı (permille).
    Opened(u32),
}

impl GateOutcome {
    /// Histogram indeksi (0..[`GATE_OUTCOME_KINDS`]).
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::NotApplicable => 0,
            Self::BlockedLossBelowBand => 1,
            Self::BlockedWindowTooSmall => 2,
            Self::BlockedNoTransitionEvidence => 3,
            Self::BlockedBurstLeftToArq => 4,
            Self::Opened(_) => 5,
            Self::BlockedPreConverge => 6,
        }
    }
}

/// [`GateOutcome`] histogramının kova sayısı.
pub const GATE_OUTCOME_KINDS: usize = 7;

/// [IW] PreConverge **ilk-uçuş penceresi** (initial window).
///
/// SORUN: PreConverge'de pacing hızı `PRE_CONVERGE_FLOOR_BW` (31 B/s) olduğundan
/// `window_bytes = rate × BURST_WINDOW_NS` ≈ 1 B çıkar ve burst-cap `front_size`'a clamp'lenir
/// → **ilk uçuş TAM BİR PAKET**. Oysa `TokenBucket::new` kovayı `DEFAULT_BURST_CAP` (2400 B)
/// kredisiyle DOLU başlatır; `set_burst_cap` o krediyi tek birime kırpıp ATAR.
///
/// Bu, handshake'sizliğin bedeli DEĞİLDİR — bir pacer clamp'idir. QUIC'in IW=10 (RFC 9002 §7.2,
/// ≤14720 B) seçimi de congestion-control tercihidir, handshake ürünü değil. Kritik sonuç:
/// PreConverge'den çıkış YALNIZ bir RTT örneğiyle olur ve örnek YALNIZ `TransmitKind::Data`
/// emisyonlarından gelir (Karn, `on_ack` `p.origin == Data` kapısı) → ilk uçuştaki ORİJİNAL
/// sayısı, kuruluş olasılığını doğrudan belirler: N orijinalle örnek alamama olasılığı
/// `p_loss^N`. Tek pakette %50 kayıpta bu 0.5; on pakette ~0.001.
///
/// `bytes == 0` (DEFAULT) = mevcut davranış BİREBİR (clamp'e dokunulmaz). `>0` = PreConverge'de
/// burst-cap tabanı bu değere yükseltilir; kova zaten dolu başladığından ilk uçuş bir-kerelik bu
/// bütçeyi kullanır, SONRASINDA 31 B/s tabanı aynen geçerlidir (floor'un gerekçesi —
/// "1 kbps kanalı floodlamaz" — SÜRDÜRÜLEN hız hakkındadır, ilk uçuş hakkında değil).
///
/// KALİBRE EDİLMEMİŞ: default flip ölçüm ister.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InitialWindowConfig {
    /// İlk-uçuş bütçesi (byte). 0 = kapalı (sevk davranışı).
    pub bytes: u64,
}

/// [BURST-FUSION] Yapı-gate'inin **burst** dalındaki politikası.
///
/// Sorun: `Q18Iid` (mevcut sevk davranışı) burst-yapı görünce parity'yi SIFIR bırakıp işi tamamen
/// ARQ'ya devreder. Ürün runtime'ında ölçüldü (`client-core/examples/fec_arm_compare.rs`,
/// GE p_gb=3% / p_bg=20% / loss_bad=60%): adaptif kol o kanalda `fec_bytes=0, blocks=0,
/// repairs=0` üretir — yani kodlama HİÇ devreye girmez ve kol, mekanizma olarak FEC-kapalı kolun
/// aynısı olur. Hostile ağ tam olarak o rejim olduğundan, ürünün ayırt edici mekanizması tam
/// ihtiyaç duyulan yerde kapalı kalıyor.
///
/// Gerekçenin diğer yarısı da gerçek: uzun bir burst tek bloktaki `r` sınırını aşarsa parity
/// israf olur. Ama doğru cevap "hiç kodlama" değil, **füzyon**dur (§8.4): parity ölçülen silme
/// oranını paylı biçimde takip eder, artakalanı ARQ taşır.
///
/// **KALİBRE EDİLMEMİŞ — default `Q18Iid`.** R1 disiplini: kampanya görmemiş bir eşik sevk
/// edilmez. `BurstFusion`'a geçmek netem matrisinde ölçülmüş bir kanıt ister; flip edilene kadar
/// davranış ve wire byte-özdeştir.
/// [FEC-RISK] **Deneysel** FEC kayıp-riski tahmincisi. Default **KAPALI** → davranış bugünküyle
/// semantik olarak özdeş (kanonik `loss_windowed_ppm` tek girdi kalır).
///
/// NEDEN AYRI BİR SİNYAL: ARQ ile FEC farklı soru sorar.
/// * ARQ: *"kaybolduğundan yeterince emin miyim?"* — gereksiz retransmit pahalı → `Confirmed` bekle
/// * FEC: *"parity ekleyecek vaktim varken şu an ne kadar risk var?"* — blok tele çıkarken karar
///
/// Kanonik kayıp YENİDEN TANIMLANMAZ. `Suspected + Confirmed` toplamı kanonik metrik OLMAZ;
/// bu ayrı bir olasılıksal okumadır ve TEK tüketicisi FEC karar yoludur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecLossRiskConfig {
    /// `false` (default) = kapalı; FEC girdisi kanonik `Confirmed` oranıdır.
    pub enabled: bool,
    /// `Suspected` olayların yaş→güven eğrisi. **KALİBRE EDİLMEMİŞ** — ölçümle belirlenecek.
    pub weights: crate::loss::SuspectConfidence,
    /// [CAL] L1 (bootstrap) parity tavanı, per-mille. Default 125‰ = K=8'de **1 repair**.
    /// Belirsiz kanıta ucuz eylem: yanlış reorder sinyali en fazla bunu öder.
    pub bootstrap_cap_permille: u32,
    /// [CAL] L2 (persistent) tavanı, per-mille. Default 250‰ = K=8'de 2 repair — FEC-MARGINAL-R
    /// v1/v2'nin ölçtüğü verimli nokta (r=4 iki katı byte'a AYNI kurtarmayı verdi).
    pub persistent_cap_permille: u32,
    /// [CAL] Persistence TABANI (ve `Fixed` modda eşiğin kendisi): `age ≥ srtt × bu / 1000`.
    /// Default 500 = 0.5×srtt. ÖLÇÜLDÜ: test hücresinde reorder ömrü OWD'ye bağlı (0.5×RTT) —
    /// çakışma tesadüf değil inşa gereği; sabit srtt-oranı reorder'ı modellemez.
    pub persist_srtt_permille: u32,
    /// [PERSIST] Ufuk modu: `FixedSrtt` (sevk, `persist_srtt_permille`) ya da
    /// `LearnedSpuriousAge` = max(taban, ρ × Q_q(ölçülen spurious yaşı)); örnek yoksa taban.
    pub persist_mode: PersistMode,
    /// [CAL] Öğrenilen ufukta pay: ρ (per-mille, 1250 = ×1.25).
    pub persist_rho_permille: u32,
    /// [CAL] Öğrenilen ufukta quantile (per-mille, 950 = Q95). Ortalama DEĞİL: reorder kuyruklu.
    pub persist_q_permille: u32,
    /// [CAL] Ufuk tavanı (srtt per-mille; `u32::MAX` = tavan yok). Kuyruklanma geç ACK'leri
    /// "spurious" gösterip ufku şişirebilir (tıkanıklıkta L2 gecikir) — bu sweep'te ÖLÇÜLMEDİ.
    pub persist_cap_srtt_permille: u32,
    /// [CAL] L1'in ateşlemesi için asgari instant risk (ppm). Gürültü tabanı; default 10 000.
    pub l1_min_instant_ppm: u64,
    /// [CAL] L2'nin ateşlemesi için asgari persistent risk (ppm). Default 10 000.
    pub l2_min_persistent_ppm: u64,
    /// [CAL] reorder_pressure'ın uygulanması için pencerede asgari olay birimi (küçük-n koruması).
    pub reorder_min_evidence_units: u64,
    /// [CAL] Tam reorder baskısında (1000‰) tavana uygulanan AZAMİ indirim, per-mille.
    /// Default 500 → tavan en fazla yarıya iner. **Asla sıfıra inmez: discount, veto değil.**
    pub reorder_discount_max_permille: u32,
    /// [CAL] `deadline/srtt ≤ bu` ise bütçe SIKI → L1 bile persistent tavanı kullanır
    /// (DEADLINE_ASSISTED). Default 2500 = `calibration_v2_deadline`'ın ölçülen kutusu.
    pub tight_budget_permille: u32,
    /// [YAPISAL] Spekülatif önerinin alt sınırı (per-mille) — indirimden SONRA uygulanır.
    /// Denetim bulgusu: K=8'de `125‰ × 0.5 = 62‰` → `compute_requested_r` = **0** repair, yani
    /// "indirim" yuvarlamayla VETO'ya dönüyordu. 63‰ = `ceil(1000/8)`: K≤8 profillerde ≥1 repair
    /// garantisi. Motor K bilmez (INV-ENGINE-SOC) — bu yüzden sabit, profil-bağımsız bir taban;
    /// K>8 profilde 1 repair için daha küçük ‰ yeterdi, taban yalnız ihtiyatlıdır.
    pub spec_min_permille: u32,
    /// [C4 G1 `[CAL]`] Reorder-belirsiz rejim guard'ı: `declared_units ≥ reorder_min_evidence_units ∧
    /// reorder_pressure_permille ≥ bu eşik` ⇒ spekülatif öneri YOK. `u32::MAX` = kapalı (sevk).
    /// Yalnız MEVCUT sinyal (spurious/olay oranı); risk hesabı ve `t_first_risk` değişmez.
    pub reorder_suppress_permille: u32,
}

impl Default for FecLossRiskConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            weights: crate::loss::SuspectConfidence::default(),
            bootstrap_cap_permille: 125,
            persistent_cap_permille: 250,
            persist_srtt_permille: 500,
            persist_mode: PersistMode::FixedSrtt,
            persist_rho_permille: 1250,
            persist_q_permille: 950,
            persist_cap_srtt_permille: u32::MAX,
            l1_min_instant_ppm: 10_000,
            l2_min_persistent_ppm: 10_000,
            reorder_min_evidence_units: 8,
            reorder_discount_max_permille: 500,
            tight_budget_permille: 2_500,
            spec_min_permille: 63,
            reorder_suppress_permille: u32::MAX,
        }
    }
}

impl FecLossRiskConfig {
    /// Deneysel kol AÇIK: arbiter + sınırlı spekülatif bootstrap (v2). `[CAL]` default'larla.
    #[must_use]
    pub fn experimental_v1() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// **FEC v1 sınırlı politikası (kapanış kararı):** aday A = 250-250-250 (L1 = L2 = 250‰; yapı-gate
    /// seviyesi client-core `RuntimeConfig`'te `Some(250)`), persistence P2 (öğrenilen Q95 spurious-yaş
    /// ufku × 1.25, taban 0.5×srtt) ve C4 guard'ı G1 (`reorder_pressure ≥ 300‰ ∧ declared ≥ 8` ⇒ öneri
    /// yok). Ölçüldü (reports/fec-loss-risk-v1/closure): loss-only 20 % +23 pp teslim; karışık
    /// kayıp+reorder (C4) n=20 OFF'a eşit/üstü (+0.04), tel +6 %; temiz yolda parity 0. Transport
    /// `default()` NÖTR (kapalı) kalır; ürün default'u client-core'dadır.
    #[must_use]
    pub fn bounded_v1() -> Self {
        Self {
            enabled: true,
            bootstrap_cap_permille: 250,
            persistent_cap_permille: 250,
            persist_mode: PersistMode::LearnedSpuriousAge,
            persist_srtt_permille: 500,
            persist_rho_permille: 1250,
            persist_q_permille: 950,
            reorder_suppress_permille: 300,
            ..Self::default()
        }
    }
}

/// [PERSIST] Persistence ufkunun kaynağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PersistMode {
    /// Sevk: sabit `persist_srtt_permille × srtt`.
    #[default]
    FixedSrtt,
    /// Kanal-türevli: `max(taban, ρ × Q_q(spurious yaşı))`; örnek yoksa taban. Yalnız L2
    /// tırmanışını frenler — L1 anında kalır (cold-start yapısal olarak korunur).
    LearnedSpuriousAge,
}

/// [PERSIST] Öğrenilen ufuk (salt fonksiyon, deterministik): `max(floor, q×ρ/1000)`, tavanlı.
#[must_use]
pub fn learned_persist_min_ns(
    floor_ns: u64,
    quantile_ns: Option<u64>,
    rho_permille: u32,
    cap_ns: Option<u64>,
) -> u64 {
    let learned = quantile_ns.map_or(floor_ns, |q| {
        floor_ns.max(q.saturating_mul(u64::from(rho_permille)) / 1000)
    });
    cap_ns.map_or(learned, |c| learned.min(c))
}

/// [FEC-RISK v2] Spekülatif kolun güven seviyesi. Belirsizlik arttıkça EYLEM küçülür.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeLevel {
    /// Kanıt yok → parity yok.
    L0None,
    /// Suspected var, kalıcılık yok → küçük bootstrap tavanı.
    L1Bootstrap,
    /// Gap'ler ≥ persist_min yaşamış → daha yüksek tavan.
    L2Persistent,
}

/// [FEC-RISK v2] Spekülatif kolun ÖNERİSİ (arbiter girdisi; tek başına otorite DEĞİL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeculativeProposal {
    pub level: SpeculativeLevel,
    /// Önerilen parity oranı (per-mille), tavan ve indirimler uygulanmış.
    pub ratio_permille: u32,
    /// Bütçe sıkı olduğu için tavan yükseltildi mi.
    pub deadline_assisted: bool,
}

impl SpeculativeProposal {
    const NONE: Self = Self {
        level: SpeculativeLevel::L0None,
        ratio_permille: 0,
        deadline_assisted: false,
    };
}

/// [FEC-RISK v2] TEK FEC ARBİTER'inin hüküm sebebi — "parity NEDEN bu miktarda" sorusu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FecArbiterReason {
    /// Hiçbir kol parity önermedi.
    None,
    /// Kanonik fuzzy/Q18 (Confirmed-taban) kazandı.
    ConfirmedFuzzy,
    /// Kalibre füzyon politikası (Confirmed-taban) kazandı.
    ConfirmedPolicy,
    /// Yapı-gate (Confirmed-taban, loss-yapı sinyali) kazandı.
    StructureGate,
    /// Spekülatif L1 kazandı.
    SpeculativeBootstrap,
    /// Spekülatif L2 kazandı.
    SpeculativePersistent,
    /// Spekülatif kol kazandı ve tavanı sıkı deadline yükseltmişti.
    DeadlineAssisted,
}

/// [HANDOFF ENSTRÜMANI] Bir `fuzzy_evaluate` kararının anlık görüntüsü — zaman serisi için.
/// Salt gözlem; sabit boyutlu `Copy`. Alanların hepsi karar anında zaten hesaplanmış değerlerdir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TsSample {
    /// Spekülatif girdi: instant / persistent risk (ppm), reorder baskısı (‰). Kol kapalıysa 0.
    pub instant_ppm: u64,
    pub persistent_ppm: u64,
    pub reorder_pressure_permille: u32,
    /// Kanonik `loss_windowed_ppm` (Confirmed).
    pub confirmed_ppm: u64,
    /// Üç kolun ÖNERİSİ (‰) ve arbiter'ın HÜKMÜ (‰).
    pub confirmed_prop: u32,
    pub structure_prop: u32,
    pub spec_prop: u32,
    pub final_ratio: u32,
    /// `FecArbiterReason::index` · spekülatif seviye (0/1/2).
    pub reason_idx: u8,
    pub level_idx: u8,
    /// [EVIDENCE] `sent_window_units` (ham) ve `declared_units` karar anında.
    pub evidence_mass: u64,
    pub declared_units: u64,
    /// [C4-ATIF] Karar anındaki bütçe/kuyruk durumu: pacer token bakiyesi, bekleyen parity, bekleyen
    /// kayıp (retx adayı) sayısı; kümülatif sınıf sayaçları (telemetri feature'ı kapalıyken 0).
    pub pacer_tokens: u64,
    pub pending_parity: u64,
    pub pending_lost: u64,
    pub sched_data_denied: u64,
    pub sched_retx_denied: u64,
    pub sched_parity_denied: u64,
    pub sched_retx_emitted: u64,
    pub sched_parity_emitted: u64,
    pub sched_data_emitted: u64,
}

/// [TEŞHİS] Arbiter sebep histogramı kova sayısı.
pub const ARBITER_REASON_KINDS: usize = 7;

impl FecArbiterReason {
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::None => 0,
            Self::ConfirmedFuzzy => 1,
            Self::ConfirmedPolicy => 2,
            Self::StructureGate => 3,
            Self::SpeculativeBootstrap => 4,
            Self::SpeculativePersistent => 5,
            Self::DeadlineAssisted => 6,
        }
    }
}

/// [FEC-RISK v2] TEK FEC ARBİTER'i: kollar yalnız ÖNERİR, parity miktarına burası karar verir.
///
/// `final = max(confirmed, structure, speculative)` — **TOPLAMA YOK**: iki kol aynı kaybı görüp
/// parity'yi iki kez sayamaz. Eşitlikte öncelik confirmed > structure > speculative (sebep
/// atfı yerleşik yolu tercih eder). Salt fonksiyon; deterministik (ADR-17).
#[must_use]
pub fn fec_arbiter(
    confirmed: (u32, FecArbiterReason),
    structure: u32,
    spec: SpeculativeProposal,
) -> (u32, FecArbiterReason) {
    let mut best = (confirmed.0, confirmed.1);
    if structure > best.0 {
        best = (structure, FecArbiterReason::StructureGate);
    }
    if spec.ratio_permille > best.0 {
        let reason = if spec.deadline_assisted {
            FecArbiterReason::DeadlineAssisted
        } else if spec.level == SpeculativeLevel::L2Persistent {
            FecArbiterReason::SpeculativePersistent
        } else {
            FecArbiterReason::SpeculativeBootstrap
        };
        best = (spec.ratio_permille, reason);
    }
    if best.0 == 0 {
        (0, FecArbiterReason::None)
    } else {
        best
    }
}

/// [KAYIP-DEFTERİ ONAY UFKU] `Suspected → Confirmed` geçişinin bekleme ufku.
///
/// **NEDEN AYRI BİR KOL VAR:** ölçüldü ki sevk edilen ufuk ürün yolunda HİÇ ateşlemiyor
/// (3 koşu, ~13k tick, `confirmed_total = 0`). `loss_windowed_ppm` yalnız `Confirmed` saydığı
/// için kayıp sinyali yapısal olarak sıfır kalıyor ve kayıp tüketen HER mekanizma (fuzzy
/// controller, Q18 yapı-gate, kalibre füzyon politikası) kör kalıyor.
/// Bkz. `reports/loss-ledger-confirm-defect/FINDING.md`.
///
/// Bu enum bir DÜZELTME DEĞİL, bir ÖLÇÜM KOLUDUR. Default sevk davranışıdır ve byte-özdeştir;
/// alternatif kol yalnız ölçüm için açılır. Hangi ufkun doğru olduğu ölçülmeden sevk edilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LossConfirmHorizon {
    /// Sevk davranışı: `cadence_loss_floor()` = 1.5 × EAI. EAI'nin içinde
    /// `EAI_SACK_WIRE_BYTES / EAI_RETURN_BW_PRIOR` = 65 B / 31 B·s⁻¹ ≈ **2.097 s** vardır ve bu
    /// terim ÖLÇÜLEN KANALDAN BAĞIMSIZDIR — 200 ms RTT'li yolda ufku ~3.2 s = 16×RTT'ye çiviler.
    #[default]
    CadenceLegacy,
    /// Kanal-türevli: `k_permille × srtt / 1000`. srtt yoksa (kuruluş öncesi) legacy'ye düşer —
    /// ölçüm yokken sabit uydurmak, düzeltmeye çalıştığımız kusurun ta kendisidir.
    ChannelDerived {
        /// Ufuk = k × srtt, per-mille (`2000` = 2×srtt).
        k_permille: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FecStructurePolicy {
    /// Sevk davranışı: yalnız IID-yapıda (`burst_delta ≤ 50k ppm`) FEC açılır; burst → ARQ.
    #[default]
    Q18Iid,
    /// Burst dalında da kodlama açılır ve seviye ölçülen silme oranından (emniyet paylı)
    /// seçilir: pay'lı oran `fec_med`'e sığıyorsa `fec_med`, aşıyorsa `fec_high`. Kodlama
    /// tek başına yetmediğinde bile parity SIFIRLANMAZ — kalanı ARQ taşır (füzyon).
    BurstFusion,
}

/// Commit-K (authority-continuity): BLOK-OTORİTE-WITNESS'ı — blok-AÇILIŞINDA aktif-mode'u + sender-
/// kimliğini + monoton-generation'ı YAKALAR. Bloğun-otorite-tüketimi (Controller-latch VEYA manuel-
/// close) witness'ı BY-VALUE-tüketir → **tek-kullanım COMPILE-TIME** (Clone/Copy/Default/serde YOK;
/// ikinci-tüketim E0382). Private-alan + private-ctor → forge/elle-kurulum-derlenmez. Yabancı-Sender'da
/// tüketim TYPED-red (instance-eşleşmesi runtime; test-kanıtlı). Pending-state-YOK → idle-transition-
/// hemen; drop-without-consume = blok-terk (sender-tarafı-state-yok, sızıntı-imkansız; Drop-gözlemi
/// bilinçli-eklenmedi — normal-tüketimde-de-ateşlenirdi, gürültü).
///
/// Tek-kullanım kanıtı (E0382 — Copy-OLMADIĞININ-da-negatif-kanıtı; Copy-olsaydı-derlenirdi):
/// ```compile_fail,E0382
/// use adaptive_transport::sender::Sender;
/// let mut s = Sender::new();
/// let a = s.open_fec_block_authority();
/// let _ = s.fec_controller_latch(a, None, 8, 4, 1, 64);
/// let _ = s.fec_controller_latch(a, None, 8, 4, 2, 64); // ikinci-tüketim: use-after-move
/// ```
/// Clone-yok kanıtı (E0599):
/// ```compile_fail,E0599
/// use adaptive_transport::sender::Sender;
/// let mut s = Sender::new();
/// let a = s.open_fec_block_authority();
/// let _b = a.clone();
/// ```
/// Pozitif-kontrol (DERLENİR — stable-rustdoc compile_fail-hata-kodunu-doğrulamaz; bu-blok API-rename/
/// arity-değişiminde-kırılarak yukarıdaki-negatif-kanıtların-vakumlaşmasını-CI'da-yakalar):
/// ```
/// use adaptive_transport::sender::Sender;
/// let mut s = Sender::new();
/// let a = s.open_fec_block_authority();
/// let _ = s.fec_controller_latch(a, None, 8, 4, 1, 64);
/// ```
#[derive(Debug)]
pub struct FecBlockAuthority {
    /// Üreten-Sender'ın process-yerel-kimliği (yabancı-Sender-reddi anahtarı).
    instance_id: u64,
    /// Sender-başına monoton açılış-generation'ı (saturating; teşhis + gelecek-engine-binding-anahtarı).
    generation: u64,
    mode: FecActuationMode,
}

impl FecBlockAuthority {
    /// Bu-bloğun-otorite-mode'u (blok-açılış-anı-yakalaması).
    #[must_use]
    pub fn mode(&self) -> FecActuationMode {
        self.mode
    }
    /// Açılış-generation'ı (monoton; teşhis/uniqueness-testi).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Commit-G: otorite-API hataları (sessiz-no-op-YASAK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FecAuthorityError {
    /// Controller-mode'da manuel-FEC-toggle/manuel-close çağrıldı (state-DEĞİŞMEDİ; typed-red).
    #[error("manual FEC toggle rejected: authority is Controller (NotInManualMode)")]
    NotInManualMode,
    /// Commit-K: witness başka-Sender'a ait (instance-eşleşmedi; typed-red + sayaç). Manuel-close-
    /// yolunda kuyruk/plan-state-değişmez; latch-yolunda red BİLİNÇLİ-görünür-kayıt-bırakır
    /// (last_latch + latch_event_index — testli) ama kuyruk/plan-DEĞİŞMEZ.
    #[error("authority rejected: witness belongs to a different Sender instance")]
    ForeignAuthority,
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
    /// Commit-K: witness başka-Sender'a ait — kimlik-reddi (guard-önceliği: mode-semantiğinden-ÖNCE).
    ForeignAuthority,
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
    /// Kümülatif kuyruğa-KABUL-edilen parity (requested→enqueued-zinciri; reddedilen-SAYILMAZ).
    pub enqueued_r_total: u64,
    /// Kümülatif wire'a-çıkan FecParity-decision (enqueued→emitted-zinciri).
    pub emitted_r_total: u64,
    /// Commit-K CORRIGENDUM: RECOVERED'la-iptal-edilen KUYRUKTA-BEKLEYEN-PARITY adedi (önce bu-alan
    /// diag_recovered_cancels'a-bağlıydı = DATA-retransmit-iptalleri — "_r_"-adıyla-YANILTICI-etiket;
    /// retransmit-iptalleri `diag_recovered_cancels()`-API'sinde-kalır, M1-metriği-değişmedi).
    pub canceled_r_total: u64,
    /// Commit-K: emisyon-izinsiz (Manual{false}-aktif) legacy-enqueue-reddi (kuyruk-üyeliği=emisyon-izni
    /// invariantının-kapısı; sessiz-DEĞİL — sayaç+doc-görünür).
    pub parity_enqueue_rejected_count: u64,
    /// Commit-K: yabancı-Sender-witness tüketim-reddi (latch-suppression + manuel-close-Err toplamı).
    pub foreign_authority_rejected_count: u64,
    /// §8.1-EXACT-TANIM: SON-plan_fec_parity-token-REDDİ-anında-kuyrukta-bekleyen-parity-adedi
    /// (GAUGE — kümülatif-DEĞİL; requested−emitted-DEĞİL; clamp/suppression/mode-disabled-farkları
    /// token-denial-SAYILMAZ; yalnız-pacer-token-reddi). 1b-tasarım-girdisi.
    pub token_denied_r_gauge: u64,
    /// Commit-I: son-mode-transition-isteğinin event-index'i.
    pub last_transition_event_index: u64,
    /// Commit-I: uygulanan-transition-sayısı (saturating; wrap-yok).
    pub transition_applied_count: u64,
}

/// Commit-N (1b-evidence): SON-token-denial anlık-görüntüsü — SALT-TEŞHİS (politika-değişikliği-YOK).
/// TOKEN-DENIAL-KESİN-TANIMI: "emisyon-İZNİ-olan (kuyruklu) parity'nin mevcut-token-şartını
/// (`pacer.try_send`) sağlayamadığı PLANLAMA-anı". ŞUNLAR-DENIAL-DEĞİL (bu-yapıya-YAZILMAZ):
/// ratio-clamp · no-rule/emergency/ManualDisabled/ForeignAuthority-suppression · legacy-enqueue-reddi ·
/// RECOVERED-cancellation · mode-transition · blok-terk. Her-denial-TAM-overwrite (carryover-yok);
/// unbounded-history/alloc/IO/format/RNG-yok (Copy-alanlar). `token_balance_before` mevcut-READ-ONLY
/// `TokenBucket::tokens()`-erişimcisinden (mühürlü-TokenBucket'a-DOKUNULMADI); token-maliyeti =
/// denied-item-boyutu (try_send-argümanı). Per-block-emitted-ledger'ı SENDER-TUTMAZ (bounded-model);
/// `emitted_r_total_at_denial` global-sayaç-anıdır — blok-düzeyi-ledger harness/test-collector'da.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecTokenDenialSnapshot {
    /// Denial-anındaki fusion-event-index (planlama-anı-attribution'u).
    pub denial_event_index: u64,
    /// Reddedilen (kuyruk-önü) item'ın bloğu.
    pub block_id: u32,
    /// Item'ın staging-otoritesi-generation'ı (legacy-enqueue: None).
    pub authority_generation: Option<u64>,
    /// Item'ın staging-anı-kaynak-otoritesi.
    pub authority_mode: FecActuationMode,
    /// Denial-anı-kuyruk-uzunluğu (toplam-bekleyen-parity).
    pub pending_parity_total: u64,
    /// Aynı-bloğun-kuyruktaki-item-adedi (interleaved-blok-ayrıştırması).
    pub pending_for_block: u64,
    /// Reddedilen-item-boyutu = token-maliyeti (try_send-argümanı).
    pub parity_size_bytes: u64,
    /// Denial-anı-pacer-token-bakiyesi (READ-ONLY-erişimci).
    pub token_balance_before: u64,
    /// Item'ın-bloğunun-planned/manual-r'ı (otorite-staging-tag'i; legacy: None).
    pub planned_r: Option<u16>,
    /// Denial-anı-global-emitted-sayacı (per-block-DEĞİL — bounded-model; ledger-harness'ta).
    pub emitted_r_total_at_denial: u64,
}

// ═════════════════════════════════════════════════════════════════════════════════════════════
// Commit-V: EXPERIMENTAL-FEC-BUDGET (cargo-feature `experimental-fec-budget`; DEFAULT-OFF).
// OFF: aşağıdaki-HİÇBİR-öğe-derlenmez → üretim-yolu-baseline-BYTE-ÖZDEŞ. ON+LeftoverOnly:
// rezervasyon=0/ticket-yok/kernel-çağrılmaz → karar-dizisi-baseline-özdeş (Commit-W-testli).
// Product/admin-activation-YOK: politika-yalnız-test/harness `set_budget_policy`-ile-seçer.
// ═════════════════════════════════════════════════════════════════════════════════════════════

/// Deneysel-bütçe-politikası (yalnız-test/harness-seçer; default-HER-KOŞULDA-LeftoverOnly).
#[cfg(feature = "experimental-fec-budget")]
#[derive(Debug, Clone, Copy)]
pub enum BudgetPolicy {
    /// Baseline: mevcut-leftover-davranışı-birebir (rezervasyon/ticket/kernel-yok).
    LeftoverOnly,
    /// R-sınıfı-pay (ticket'sız).
    ProtectedShareExperimental(crate::budget_kernel::PolicyParams),
    /// R + F2-ticket + C1-cap.
    FloorCapExperimental(crate::budget_kernel::PolicyParams),
}

#[cfg(feature = "experimental-fec-budget")]
impl Default for BudgetPolicy {
    // derive(Default)-yerine-açık-impl: default=LeftoverOnly-sözleşmesi-görünür-kalsın.
    #[allow(clippy::derivable_impls)]
    fn default() -> Self {
        BudgetPolicy::LeftoverOnly
    }
}

/// U6: policy-geçişi yalnız-boş-kuyrukta (Busy-guard).
#[cfg(feature = "experimental-fec-budget")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BudgetPolicyTransitionError {
    #[error("budget policy busy: pending tickets/bulk parity/reservation outstanding")]
    Busy,
}

/// U4: floor-ticket (kimlik=(block,generation); missed-tam-1-kez).
#[cfg(feature = "experimental-fec-budget")]
#[derive(Debug, Clone, Copy)]
struct FloorTicket {
    block_id: u32,
    generation: u64,
    deadline_ns: u64,
    missed: bool,
}

/// U4-miss-teşhisi (tam-overwrite).
#[cfg(feature = "experimental-fec-budget")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorMissSnapshot {
    pub block_id: u32,
    pub generation: u64,
    pub event_index: u64,
    pub pending_items: u64,
    pub pending_tickets: u64,
    pub cap_suppressed: bool,
}

#[cfg(feature = "experimental-fec-budget")]
#[derive(Debug, Default)]
struct BudgetExt {
    policy: BudgetPolicy,
    state: crate::budget_kernel::BudgetPolicyState,
    tickets: VecDeque<FloorTicket>,
    reserved_now: u64,
    parity_budget_left: Option<u64>, // C1-canlı-enforcement: None=Leftover-sınırsız; Some=kernel-limit-kalanı
    pending_decision: Option<crate::budget_kernel::BudgetDecision>,
    spent_this_opportunity: u64,
    last_opportunity_ns: u64,
    profile_parity_size: u64, // U11: F2-size-sabitliği (0=henüz-görülmedi)
    // U4-lifecycle-sayaçları (saturating):
    tk_rejected: u64,
    tk_admitted: u64,
    tk_on_time: u64,
    tk_late: u64,
    tk_missed_unique: u64,
    tk_cancel_before: u64,
    tk_cancel_after: u64,
    bulk_admission_rejected: u64, // staging-cap-reddi (admitted_r < planned_r kaynağı)
    last_floor_miss: Option<FloorMissSnapshot>,
    reservation_waste_total: u64,
    new_data_displaced: u64,
    retx_displaced: u64,
    cap_suppressed_events: u64,
    rate_zero_events: u64,
}

/// Canlı-telemetri (U-EK-§17-alt-kümesi; salt-gözlem).
#[cfg(feature = "experimental-fec-budget")]
#[derive(Debug, Clone, Copy)]
pub struct BudgetDiag {
    pub entitlement_credit: u64,
    pub hardcap_credit: u64,
    pub reserved_now: u64,
    pub tickets_pending: u64,
    pub tk_rejected: u64,
    pub tk_admitted: u64,
    pub tk_on_time: u64,
    pub tk_late: u64,
    pub tk_missed_unique: u64,
    pub tk_cancel_before: u64,
    pub tk_cancel_after: u64,
    pub bulk_admission_rejected: u64,
    pub last_floor_miss: Option<FloorMissSnapshot>,
    pub reservation_waste_total: u64,
    pub new_data_displaced: u64,
    pub retx_displaced: u64,
    pub cap_suppressed_events: u64,
    pub rate_zero_events: u64,
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

/// DR-CS-3 (§FAZ-2) product config: shipping bootstrap RTT probe. DEFAULT OFF ⇒ inert and
/// byte-identical to the FAZ-1A baseline.
///
/// Contract: reports/dr-cs-3/DR-CS-3-BOOTSTRAP-RTT-CONTRACT.md (frozen before implementation).
/// Mechanism: while `PreConverge && srtt.is_none()`, fire at most one `0x19` probe per authoritative
/// ladder step, on an ADDITIVE allowance (no DATA token, no pacer debt), and turn the matching
/// authenticated `0x1A` echo into a real RTT sample via the existing `RttEstimator::observe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapRttConfig {
    /// Master gate. Default false → inert.
    pub enabled: bool,
    /// [KURULUŞ-KALİBRASYONU] Probe merdiveni (ns, arm anına göre). `None` = `BS_FIRES_NS`
    /// varsayılanı (3/9/21/45 s).
    ///
    /// O varsayılan BLACKOUT için kalibre edildi (uzun sessizlikten sonra hattı yoklamak), KURULUŞ
    /// için değil. Kuruluşta beklenen gecikme `INITIAL_RTT` (100 ms) mertebesindedir; ilk basamağın
    /// 3 s olması, kilidin çözülmesini üç saniye geciktirir ve dar deadline'lı hücrelerde koşu
    /// biter. `Some(..)` ile kuruluş-hızlı bir merdiven verilebilir (bkz.
    /// [`BootstrapRttConfig::establishment_fast`]).
    ///
    /// Bütçe DEĞİŞMEZ: `BS_TX_HARD_BUDGET` (200 B) her hâlde geçerlidir, yani merdiven ne kadar
    /// sıkı olursa olsun toplam 4 probe × 44 B tavanı aşılamaz — hızlandırma trafik ŞİŞİRMEZ,
    /// yalnız aynı bütçeyi erken harcar.
    pub fires_ns: Option<[u64; 4]>,
}

impl BootstrapRttConfig {
    /// KURULUŞ için merdiven: ilk basamak 2×`INITIAL_RTT` (200 ms), sonra ×3 büyüme
    /// (0.2 / 0.6 / 1.8 / 5.4 s). Gerekçe: kuruluş kilidi bir RTT örneğinin YOKLUĞUDUR ve
    /// beklenen RTT `INITIAL_RTT` mertebesindedir; ilk yoklamayı bir RTT-turu ötesine koymak
    /// gereksiz gecikmedir. KALİBRE EDİLMEMİŞ — ölçümle doğrulanmalı.
    #[must_use]
    pub fn establishment_fast() -> Self {
        Self {
            enabled: true,
            fires_ns: Some([200_000_000, 600_000_000, 1_800_000_000, 5_400_000_000]),
        }
    }

    /// Etkin merdiven (config verilmemişse sevk varsayılanı).
    #[must_use]
    pub fn ladder(&self) -> [u64; 4] {
        self.fires_ns.unwrap_or(BS_FIRES_NS)
    }
}

#[allow(clippy::derivable_impls)]
impl Default for BootstrapRttConfig {
    fn default() -> Self {
        // DEFAULT OFF — pending locked/fresh panel validation (DR-CS-3 §9).
        Self {
            enabled: false,
            fires_ns: None,
        }
    }
}

/// DR-CS-3 authoritative ladder (absolute offsets from the bootstrap arm instant).
/// Same values as the legacy `CS_PROBE_FIRES_NS`; no new constant.
const BS_FIRES_NS: [u64; 4] = [3_000_000_000, 9_000_000_000, 21_000_000_000, 45_000_000_000];
/// Exact `CsProbe` wire size (schema `control.rs:60`: PROBE_WIRE_EXACT = 44 B).
const BS_PROBE_WIRE_EXACT: u64 = 44;
/// Total sender probe budget (legacy `CS_PROBE_TX_HARD_BUDGET`): 4 × 44 = 176 B ≤ 200 B.
const BS_TX_HARD_BUDGET: u64 = 200;

/// DR-CS-3 observation-only bootstrap lineage (always compiled; counters only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BootstrapRttDiag {
    /// Ladder armed (once per bootstrap epoch).
    pub arm: u64,
    /// `not-due → due` step transitions (unique demand; NEVER a poll count).
    pub step_edge: u64,
    /// A step that had already fired was refused.
    pub duplicate_step_denied: u64,
    /// Probes actually emitted to the wire.
    pub probe_emitted: u64,
    /// Probe wire bytes emitted.
    pub probe_wire_bytes: u64,
    /// Echoes handed to the sender.
    pub echo_ingested: u64,
    /// Echoes that matched an outstanding generation.
    pub echo_match_valid: u64,
    /// Echoes rejected (unsolicited / wrong generation / duplicate / already resolved).
    pub echo_match_rejected: u64,
    /// The accepted RTT sample (ns); 0 if none.
    pub rtt_sample_ns: u64,
    /// Ladder cancelled after the first valid sample.
    pub cancelled: u64,
    /// MUST stay 0: a probe was emitted after the ladder resolved.
    pub probe_after_success_violation: u64,
    /// MUST stay 0: budget exceeded.
    pub budget_violation: u64,
}

/// `bytes / srtt` → bytes/sec. u128-widened, saturating (ADR-17: integer only, no float).
fn rate_per_srtt(bytes: u64, srtt_ns: u64) -> u64 {
    if srtt_ns == 0 {
        return 0;
    }
    u64::try_from(u128::from(bytes) * 1_000_000_000u128 / u128::from(srtt_ns)).unwrap_or(u64::MAX)
}

/// DR-RATE-RAMP-2 (§FAZ-1) product config: startup pacing INITIALIZATION at the PreConverge→Startup
/// transition. DEFAULT OFF ⇒ mechanism inert and the pacing path is byte-identical to the baseline.
///
/// Rationale. Leaving `PreConverge` requires a valid SACK, so an `srtt` measurement exists — but `BtlBw`
/// at that instant is seeded from a sample taken under the 31 B/s floor (one sealed unit per RTT). The
/// pacing rate therefore starts at `gain × BtlBw ≈ 288 B/s`, which reproduces one-unit-per-RTT samples and
/// re-derives the same low `BtlBw`: a self-clocked fixed point (DR-RATE-RAMP-1).
///
/// This does NOT write synthetic samples into the BtlBw max-filter and does NOT change `app_limited`
/// semantics or add an in-flight gate. It only initializes the PACER to the minimum-pipe rate the
/// product's OWN cwnd policy already permits, and only for the bootstrap window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupPacingConfig {
    /// Master gate. Default false → inert.
    pub enabled: bool,
}

// The manual impl is deliberate: it carries the load-bearing rationale for why the default is OFF
// (the sealed INV-RATE-1 lock-golden). A bare `#[derive(Default)]` would strip that record.
#[allow(clippy::derivable_impls)]
impl Default for StartupPacingConfig {
    fn default() -> Self {
        // DR-RATE-RAMP-2 §FAZ-1: the treatment PASSED its paired single-axis validation (260 runs:
        // locked-replay / GE-L10-p20 / IID-p5 / no-loss{50,250,1000}; locked fixed points 3 -> 0;
        // delivery worse in 0/115 pairs; no-loss wire byte-identical; no synthetic BtlBw samples).
        //
        // DR-RATE-RAMP-2 §FAZ-1A: **DEFAULT ON**. The treatment passed its six frozen PASS criteria
        // (260 paired single-axis runs) and the INV-RATE-1 lock-golden was formally superseded, not
        // silently rewritten: see reports/fs-cal-2/v3.5/INV-RATE-1-STARTUP-FLOOR-SUPERSESSION.md
        // (recorded and hashed BEFORE any test edit). The old golden is preserved verbatim and re-scoped
        // to the floor-INACTIVE legacy path; the active-floor contract is sealed by six new tests
        // (arming only on the PreConverge exit, loss can neither arm nor raise it, cwnd/srtt has the last
        // word at every application, no synthetic BtlBw samples, deterministic permanent release).
        //
        // DR-RATE-RAMP-2B §A3.2/§A4 reverted this to OFF. EK4-A2 (the liveness blocker) is now FIXED
        // and regression-sealed (tests/ek4_a2_liveness.rs; reports/ek4-a2/), and restoring the default
        // to ON was attempted here — but it exposed a THIRD, separate blocker:
        //
        //   kama-engine/tests/fuzzy_e2e.rs::{e2e_real_fuzzy_nonzero_r2_to_emission,
        //   e2e_real_fuzzy_partial_block} -- the controller plans r=2 parity (ratio=800, planned=4)
        //   and with the floor ACTIVE **zero** parity units reach the wire in 900 ticks (9 s virtual),
        //   while with the floor INACTIVE all 4 emit. Isolated by toggling ONLY this axis.
        //
        // Explicit FEC_R2 must stay runtime-executable, so shipping this default would have traded a
        // fixed liveness defect for a broken FEC actuator.
        //
        // RESOLVED 2026-08-15 — the FEC-actuator blocker above no longer reproduces, so the default is
        // ON (restored by 925dea66). Re-measured with the floor ACTIVE, i.e. through this very default,
        // on the exact scenario the paragraph above names:
        //
        //   kama-engine/tests/fuzzy_e2e.rs  ->  5/5 pass
        //     e2e_real_fuzzy_nonzero_r2_to_emission : ratio=800 planned=4 emitted=4
        //     e2e_real_fuzzy_partial_block         : ratio=800 planned=4 emitted=4
        //
        // Same ratio and same planned count the blocker cites; emitted is 4, not the 0 it reported.
        // The assertion is `emitted == planned` with planned=4, so it is non-vacuous — a zero-parity
        // regression would fail it rather than pass trivially.
        //
        // Until this line the comment said "Default stays OFF" while the code returned true: 925dea66
        // flipped the value and left the rationale from 68165aca in place. The value was right and the
        // record was stale; the record is now corrected rather than the behaviour.
        //
        // The mechanism remains fully available via `set_startup_pacing_init`.
        Self { enabled: true }
    }
}

/// DR-CS-2 / DR-RATE-RAMP-1 observation-only scheduler telemetry (feature `diag-sched-telemetry`,
/// DEFAULT OFF). Every field is a saturating counter incremented AT an existing decision point; nothing
/// here participates in control flow, ordering, token accounting or pacing.
///
/// Vocabulary (deliberately disaggregated — an aggregate `probe_due` counter is a POLL counter and MUST
/// NOT be read as emission demand):
/// * `*_due_poll`      — the due-predicate was evaluated and held (level-triggered; grows with tick rate).
/// * `*_due_edge`      — a `not-due → due` TRANSITION (this, not `due_poll`, is "a probe was wanted").
/// * `*_enqueued`      — accepted into the class's pending queue/plan.
/// * `*_selected`      — the scheduler reached this class and attempted an emission this pass.
/// * `*_token_denied`  — the attempt was refused by the pacer/token check.
/// * `*_wire_emitted`  — a `TransmitDecision` was actually produced.
#[cfg(feature = "diag-sched-telemetry")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchedDiag {
    /// DATA (fresh original source) — scheduler reached the tx_queue front.
    pub data_selected: u64,
    /// DATA refused by the token check.
    pub data_token_denied: u64,
    /// DATA `TransmitDecision` emitted.
    pub data_wire_emitted: u64,
    /// `Event::AppData` accepted into `tx_queue`.
    pub data_enqueued: u64,
    /// ARQ selective retransmit — scheduler reached a `pending_lost` record.
    pub arq_retransmit_selected: u64,
    /// ARQ retransmit refused by the token check.
    pub arq_retransmit_token_denied: u64,
    /// ARQ retransmit emitted.
    pub arq_retransmit_wire_emitted: u64,
    /// §3.3-B geometric copy — scheduler reached a due `dup_queue` entry.
    pub dup_selected: u64,
    /// Geometric copy refused by the token check.
    pub dup_token_denied: u64,
    /// Geometric copy emitted.
    pub dup_wire_emitted: u64,
    /// Bounded tail probe — `emit_tail_probe` had a pending target and reached the token check.
    pub tail_probe_selected: u64,
    /// Tail probe refused by the token check (stays pending; cap NOT consumed).
    pub tail_probe_token_denied: u64,
    /// Tail probe emitted to the wire.
    pub tail_probe_wire_emitted: u64,
    /// Tail probe pending-target set by the PTO handler (`not-pending → pending` transition).
    pub tail_probe_enqueued: u64,
    /// Cold-start ladder probe — reached the token check in `drain_cs_probes`.
    pub cs_probe_selected: u64,
    /// Cold-start probe refused by the token check.
    pub cs_probe_token_denied: u64,
    /// Cold-start probe emitted to the wire.
    pub cs_probe_wire_emitted: u64,
    /// Cold-start probe pushed onto `cs_probe_pending` by a due ladder timer.
    pub cs_probe_enqueued: u64,
    /// FEC parity refused by the token check.
    pub fec_parity_token_denied: u64,
    /// FEC parity emitted.
    pub fec_parity_wire_emitted: u64,
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
    // ── Commit-N (1b-evidence; SALT-TEŞHİS — davranışa-etkisiz attribution-tag'leri) ──
    authority_generation: Option<u64>, // otorite-bağlı-staging: witness-gen; legacy-enqueue: None
    authority_mode: FecActuationMode, // staging-anı-kaynak-otorite (witness / current-mode-snapshot)
    planned_r: Option<u16>,           // otorite-bağlı: bloğun-planned/manual-r; legacy: None
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
    /// DR-CS-1 M2 (F2A): cold-start CS-probe (CTRL_PATH_PROBE cold-start-varyantı; ≤44B; amp≤1).
    /// `seq`-alanı = probe_seq (AYRI-namespace; INV-CS-7); data-seq/loss-ledger'a GİRMEZ (units=0-
    /// muadili: track_sent-ÇAĞRILMAZ). Encode güvenlik/harness-katmanında (§11.1-layout).
    /// DR-CS-3: UNGATED — the `0x19` codepoint already ships in `kama-transport-schema`; this is the
    /// internal signal that the shipping bootstrap emitter produced one.
    CsProbe,
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
/// [Q18-FIX] Yapı-gate alt kayıp-eşiği (ppm). Altında fuzzy'nin R1 `fec=0`'ı doğru (kayıp düşük,
/// ARQ yeter); üstünde kayıp anlamlı ve yapı belirleyici.
///
/// **[DAYANAK BORCU — 2026-09-04] BU SAYININ ARKASINDA GEÇERLİ BİR KAMPANYA YOK.**
/// Eski gerekçe "100k = Q17 holdout'un IID/burst'ün ayrıştığı bandın tabanı (FS-FUSION-CAL-1 §5:
/// 20_833..83_333 ARQ-tail Pareto; 200k FEC Pareto)" idi. İki ayrı sebeple çöktü:
///
/// 1. **Q17 holdout'u FORMEL OLARAK GEÇERSİZ.** `reports/fs-fusion-cal-1/FRAMING-CORRIGENDUM.md`
///    :73 `holdout-*` dosyalarını "**VOID** — evaluated the withdrawn rival-framing policy" ilan
///    eder; :29-31 kritik cümle: holdout'un "burst ARQ ister" tarafını üreten zararlı
///    false-positive hücresi `PRODUCT_NETNS`'ten gelir ve o veri seti **HİÇ FEC KOLU
///    İÇERMEZ** (iki kol da r0) → "A conclusion about FEC could not legitimately be drawn from it."
/// 2. **Bandın ppm değerleri türetilemez.** R2A grid'i loss ORANINI loss YAPISIYLA karıştırır
///    (S1/S2 = pozisyonel silme, 48 symbol'ün %2-8'i; S3 = IID %20) ve nominal ppm'ler jeneratör
///    artefaktıdır (20_833 = 1/48, 41_666 = 2/48, 83_333 = 4/48). Raporun kendi hükmü: "a
///    production FEC-activation threshold cannot be derived from loss_ppm alone from R2A."
///
/// `SPEC-GAP-REGISTRY-2026-08-27.md:302-305` bu sabiti zaten "unsanctioned magic constant …
/// exactly the uncalibrated-knee class" olarak kaydetmişti.
///
/// SABİT DEĞİŞTİRİLMEDİ: kampanya görmemiş bir eşiği kampanyasız BAŞKA bir sayıyla değiştirmek
/// aynı hatadır. Kayıt açık; doğru kapanış, ürün-katmanında FEC kollu ön-kayıtlı bir grid.
const STRUCTURE_GATE_LOSS_MIN_PPM: u64 = 100_000;
/// [Q18-FIX] Gate'in karar vermesi için pencerede gereken fate-known birim (kanıt-eşiği; azsa
/// dokunmaz). 32 = Laplace p11/p01'in ±~30k ppm gürültü bandına inmesi için yeterli geçiş sayısı.
const STRUCTURE_GATE_MIN_N: u32 = 32;
/// [Q18-FIX] IID kabul-tavanı: `burst_delta = p11−p01 ≤ bu` → korelasyonsuz kayıp → FEC açılır.
/// IID'de beklenen 0 (±gürültü); burst'te (GE-L3+) FATE2ARM ölçümü +567k ppm — boşluk ~433k.
/// 50k = gürültü-üstü, burst-altı güvenli çizgi (fate_estimator'daki 25_000 pin'i geçer).
///
/// **[DAYANAK BORCU — 2026-09-04]** Ayrım eşiğinin KONUMU (50k) gürültü-payı argümanıyla
/// savunulabilir, ama bu eşiğin arkasındaki POLİTİKA — "burst → ARQ'ya bırak" — geçersiz
/// Q17 holdout'una dayanıyordu (bkz. `STRUCTURE_GATE_LOSS_MIN_PPM`). Burst rejiminde FEC'in
/// zararlı olduğuna dair GEÇERLİ bir ölçüm yoktur: o iddiayı üreten hücrede hiç FEC kolu yoktu.
/// Politikanın kendisi `FecStructurePolicy` ile opt-in hâle getirildi; ölçüm gelene dek
/// default davranış korunuyor.
const STRUCTURE_GATE_IID_MAX_DELTA_PPM: i64 = 50_000;
/// [BURST-FUSION] Ölçülen silme oranının üzerine konan emniyet payı (pay/payda). Bir RS bloğu
/// ancak parity oranı blok-içi silme oranını AŞARSA kurtarır; ölçülen oran ortalamadır, burst
/// varyansı blok-içi oranı ortalamanın üstüne çıkarır → paysız seçim sistematik olarak yetersiz
/// kodlar. 3/2 = %50 pay (kalibre-EDİLMEMİŞ başlangıç; bkz. `FecStructurePolicy::BurstFusion`).
const BURST_FUSION_MARGIN_NUM: u64 = 3;
const BURST_FUSION_MARGIN_DEN: u64 = 2;
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
/// [DR-EST-1] PreConverge KURULUŞ-DENEMESİ backoff'u: ilk vade + tavan [CAL]. Floor (31 B/s)
/// worst-corner gerekçesini KORUYARAK ilk-ölçüm-öncesi sürekliliği kurtarır (S2-REGION rt60d300
/// teşhisi: sıkı-deadline + iki-yön-%60 + 600 ms RTT'de teyit-zinciri hiç kurulamıyordu → 0/40).
/// Global TEK-slot + exponential-backoff ⇒ 1 kbps kanalda ~10 s'de ≤4 datagram (~%4) — logaritmik-güvenli.
const ESTABLISH_PROBE_INITIAL_NS: u64 = 500_000_000;
const ESTABLISH_PROBE_CAP_NS: u64 = 8_000_000_000;
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
/// Commit-K: process-yerel Sender-instance-sayacı (witness yabancı-Sender-reddi anahtarı).
/// Determinizm-notu: kimlik hiçbir-emisyon/wire/golden/M1-çıktısına-girmez; yalnız-reddetme-karşılaştırması.
static NEXT_SENDER_INSTANCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
    /// FS-CAL-2 v3 §3 loss-STRUCTURE estimator. OBSERVATION-ONLY: it is fed by the same events the
    /// engine already processes and is read by nothing except `diag_loss_structure`. No decision,
    /// no ordering, no byte on the wire depends on it. It exists because Q17 and FS-FUSION-CELL-1
    /// both showed that loss RATE cannot gate FEC and loss STRUCTURE is the missing input.
    fate_estimator: crate::fate_estimator::FateEstimator,
    tx_queue: VecDeque<OpaqueUnit>,
    pending_lost: VecDeque<LostRec>, // §5 kayıp seq'ler → RECOVER_ARQ retransmit / RECOVER_FWD dup
    // DR-ARQ-PRODUCT-TAIL-1: bounded tail-loss recovery (default OFF; INV-CS-4 supersession). Decision logic lives
    // in crate::tail_recovery; the sender only bridges live ACK/RTT/inflight state to it and emits via the ARQ path.
    tail_recovery: TailRecovery,
    /// [FAZ-1] Dönüş-yolu gözlemcisi — SALT GÖZLEM (hiçbir kontrol yolu okumaz).
    return_path: crate::return_path::ReturnPathObserver,
    // DR-RATE-RAMP-2 (§FAZ-1): startup pacing initialization (default OFF).
    startup_pacing: StartupPacingConfig,
    // Active bootstrap pacing floor (B/s). 0 = inactive. Bootstrap-only: released on startup plateau or
    // as soon as the naturally-computed rate catches up. NEVER a permanent minimum rate.
    startup_pacing_floor_bps: u64,
    // Latch: the bootstrap floor may arm AT MOST ONCE per sender. Once released it can never re-arm
    // (no Plateau/ProbeBW re-entry). §1A.2 binding contract.
    startup_floor_armed_once: bool,
    // Latch: the Startup ramp base has been entered at least once since the floor armed. The floor may
    // only be released by the ramp-END condition AFTER the ramp has actually begun.
    startup_base_seen: bool,
    // ── DR-CS-3 shipping bootstrap RTT probe ───────────────────────────────────────────────
    bootstrap_rtt: BootstrapRttConfig,
    /// Arm instant; ladder offsets are absolute from here. `None` ⇒ not armed.
    bs_armed_ns: Option<u64>,
    /// Next ladder step index (0..4). Edge-only: a step can fire at most once.
    bs_next_step: u8,
    /// Session-monotonic probe generation (`probe_seq`, INV-CS-7).
    bs_seq_next: u16,
    /// The single outstanding generation: `(probe_seq, local monotonic send instant)`.
    /// Fixed-size (one slot) — never a growing map.
    bs_outstanding: Option<(u16, u64)>,
    /// Probe wire bytes spent this bootstrap epoch (budget `BS_TX_HARD_BUDGET`).
    bs_bytes: u64,
    /// The ladder has resolved (first valid sample) or been cleared; no further probes.
    bs_done: bool,
    bs_diag: BootstrapRttDiag,
    // DR-CS-2 / DR-RATE-RAMP-1 (feature-gated, default OFF): observation-only scheduler telemetry.
    #[cfg(feature = "diag-sched-telemetry")]
    sched_diag: SchedDiag,
    tail_probe_pending: Option<u64>, // oldest-inflight seq a PTO probe is due for (un-emitted); None ⇒ none.
    tail_probe_cancelled: u64, // observation-only (§7): due probes cancelled at emission (ACK-race: target no
    // longer outstanding). Distinct from token-starved (kept pending). Only mutated on the ON path.
    dup_queue: Vec<DupRec>, // §11 zamanlanmış forward-recovery kopyaları
    /// [DR-EST-1] Kuruluş-kolu anahtarı (DEFAULT-ON — ürün-davranışı). Yalnız donmuş-enstrüman
    /// testleri (Gate-5 structural-oracle: probe-vs-DATA çekişme-izolasyonu, DR-EST-1-öncesi
    /// tasarım) kapatır; kolun KENDİ pinleri tests/establish_probe.rs'te default-ON koşar.
    establish_probe_enabled: bool,
    /// [DR-EST-1] Kuruluş-denemesi vadesi (0 = disarm; yalnız PreConverge'de kurulur).
    establish_due_ns: u64,
    establish_interval_ns: u64,
    /// [DR-EST-1] gözlem: token-muaf kuruluş-kopyası emisyon sayısı.
    establish_probe_emitted: u64,
    last_now_ns: u64,

    // ── FEC-emisyon (DR-S5-1; koşullu — guard-OFF'ta hot-path byte-değişmez) ──
    fec_mode: FecActuationMode, // Commit-G TEK-otorite (ayrı-bool-YOK); Manual{false}=DR-S5-1-default
    instance_id: u64,           // Commit-K: process-yerel Sender-kimliği (yabancı-witness-reddi)
    fec_authority_generation: u64, // Commit-K: monoton açılış-generation (saturating)
    last_latch: Option<FecLatchResult>,
    latch_event_index: u64,
    no_eligible_decision_count: u64,
    decision_event_mismatch_count: u64,
    no_rule_zero_count: u64,
    manual_toggle_rejected_count: u64,
    parity_enqueued_total: u64,
    parity_emitted_total: u64,
    parity_canceled_total: u64, // Commit-K: RECOVERED-parity-iptalleri (canceled_r_total-corrigendum)
    parity_enqueue_rejected_count: u64, // Commit-K: izinsiz-legacy-enqueue-reddi
    foreign_authority_rejected_count: u64, // Commit-K: yabancı-witness-reddi
    token_denied_r_gauge: u64,  // Commit-J-rename (eski: budget_denied_r_gauge) — §8.1-exact-tanım
    #[cfg(feature = "experimental-fec-budget")]
    budget_ext: BudgetExt, // Commit-V (OFF'ta-derlenmez; ON+LeftoverOnly-inert)
    last_token_denial: Option<FecTokenDenialSnapshot>, // Commit-N: son-denial TAM-overwrite (salt-teşhis)
    token_denial_count_total: u64, // Commit-N: kümülatif-denial-OLAYI-sayısı (saturating)
    last_transition_event_index: u64, // Commit-I
    transition_applied_count: u64, // Commit-I: saturating
    pending_fec: VecDeque<PendingFecParity>, // engine FEC-encode-sonrası enqueue; sender pace-eder
    // ── DR-CS-1 M1 (cfg-gated; flag-OFF'ta derlenmez → struct byte-özdeş) ──
    #[cfg(feature = "experimental-cold-start")]
    cs_wakeup: Vec<(u64, CsTimerKind)>, // exposed-timer kümesi (deadline_ns, kind); retire-on-fire
    #[cfg(feature = "experimental-cold-start")]
    cs_hook_fired: Vec<(u64, u8)>, // TEST-ONLY gözlem: hook-timer fire-kayıtları (now_ns, id)
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_armed_once: bool, // M2: merdiven epoch-başına-BİR-kez arm (yeniden-arm-YOK)
    /// [OFF = OFF] F2A lane'inin ürün anahtarı. Feature-default `true` (setter'ı çağırmayan DR-CS-1 ve
    /// kama-integration panel testleri değişmez); ürün runtime'ı `set_cs_probe_lane(false)` ile KAPATIR.
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_lane_enabled: bool,
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_disarmed: bool, // srtt/state-çıkışı-disarm (kalan-fire'lar + pending iptal)
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_seq_next: u16, // INV-CS-7: session-monoton; yeniden-kullanım-YOK
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_seq_wrapped: u64, // wrap → probe-YOK + typed-diag (saturating-sayaç)
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_pending: VecDeque<u16>, // fire-edilmiş, pacer-bekleyen probe'lar (probe_seq)
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_emitted: u64, // wire-emission-sayısı (INV-CS-1 kanıtı)
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_tx_bytes: u64, // cold_probe_tx_bytes (INV-CS-2; C4: YALNIZ-forward-probe)
    #[cfg(feature = "experimental-cold-start")]
    cs_probe_outstanding: Vec<(u16, u64)>, // emit-edilmiş, echo-bekleyen (probe_seq, emit_ns); ≤N_PROBE
    #[cfg(feature = "experimental-cold-start")]
    cs_resolved: Option<CsSrttSource>, // ilk-bootstrap-sonucu (INV-CS-3; tam-bir-kez)
    #[cfg(feature = "experimental-cold-start")]
    cs_epoch_timed_out: bool, // M5 (F3): TimedOut-terminal → post-timeout-echo-yok-sayma (koşul-5)
    #[cfg(feature = "experimental-cold-start")]
    cs_induced_units: BTreeSet<u64>, // bootstrap-anı-outstanding unit_id'ler (induced-retx-atfı; bounded)
    #[cfg(feature = "experimental-cold-start")]
    cs_induced_retx_bytes: u64, // bootstrap_induced_retransmit_bytes (C4; §2.4-a)
    #[cfg(feature = "experimental-cold-start")]
    cs_srtt_reported: bool, // M5: bootstrap-srtt-terminal bir-kez-raporlandı (PostTimeout ayrı)
    #[cfg(feature = "experimental-cold-start")]
    cs_terminal: Option<CsTerminal>, // M5 output-API (drain: take_cs_terminal)
    // ── K-8/F4: PostTimeoutNotice (SENDER-tarafı; ColdRxState/Host TUTMAZ). one-slot NOTICE_BOUND=1;
    //    MAX_ACTIVE_COLD_EPOCHS=1 (tek cs_cold_epoch); SALT-LOGICAL (BtlBw'ye SIFIR satır — N8). ──
    #[cfg(feature = "experimental-cold-start")]
    cs_cold_epoch: u32, // aktif bootstrap cold-epoch (F4-slice: tek-epoch; MAX_ACTIVE_COLD_EPOCHS=1)
    #[cfg(feature = "experimental-cold-start")]
    post_timeout_notice: Option<PostTimeoutNotice>, // one-slot; first-valid-proof-wins

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
    diag_structure_gate_opened: u64, // [Q18-FIX] yapı-gate'in fuzzy fec=0'ı açtığı event sayısı
    /// [BURST-FUSION] Yapı-gate'in burst dalındaki politikası (default = sevk davranışı).
    fec_structure_policy: FecStructurePolicy,
    /// [R-SWEEP] Yapı-gate'in açtığı parity seviyesi (‰) override'ı. `None` (default) = fuzzy
    /// config'in `fec_med`'i (sevk davranışı, byte-özdeş). Kalibrasyon kolu — gate'in 500‰=r4
    /// seviyesi kampanyada r=2'ye eşit kurtarma/2× byte ölçülmüştü; ekonomik seviye süpürülür.
    structure_gate_level_permille: Option<u32>,
    /// [ONAY UFKU] Ledger confirm ufku kolu — default sevk davranışı.
    loss_confirm_horizon: LossConfirmHorizon,
    /// [FEC-RISK] Deneysel FEC kayıp-riski kolu — default KAPALI.
    fec_loss_risk: FecLossRiskConfig,
    /// [FEC-RISK ENSTRÜMAN] `fec_loss_risk_ppm` ilk kez > 0 olduğu an (ns, 0 = hiç).
    diag_t_first_risk_ns: u64,
    /// [FEC-RISK ENSTRÜMAN] İlk parity emisyonu anı (ns, 0 = hiç).
    diag_t_first_parity_ns: u64,
    /// [FEC-RISK ENSTRÜMAN] Son hesaplanan risk (ppm) ve koşu-boyu TEPESİ.
    diag_risk_ppm_last: u64,
    diag_risk_ppm_peak: u64,
    /// [FEC-RISK v2 ENSTRÜMAN] Arbiter sebep histogramı · spekülatif seviye histogramı (L0/L1/L2).
    diag_arbiter_reasons: [u64; ARBITER_REASON_KINDS],
    diag_spec_levels: [u64; 3],
    /// [FEC-RISK v2 ENSTRÜMAN] persistent risk tepesi · reorder baskısı (son, tepe).
    diag_persistent_ppm_peak: u64,
    diag_reorder_pressure_last: u32,
    diag_reorder_pressure_peak: u32,
    /// [FEC-RISK v2 ENSTRÜMAN] İlk L1 / L2 önerisinin anı (ns, 0 = hiç).
    diag_t_first_l1_ns: u64,
    diag_t_first_l2_ns: u64,
    diag_preconverge_parity_blocked: u64, // [KURULUŞ-GUARD] arbiter çıkışı PreConverge'de sıfırlandı
    repair_loss_not_arqed: u64, // [FAZ-2/3] ARQ'ya SOKULMAYAN kayıp repair sayısı (salt gözlem)
    diag_parity_budget_clamped: u64, // [FAZ-2/2] §8.5 bütçe clamp'inin bağladığı karar sayısı
    diag_parity_budget_would_zero: u64, // [FAZ-2/2] clamp parity'yi sıfırlayacaktı (§11 çelişkisi)
    diag_first_l1_ctx: (u64, u64, u64), // [EVIDENCE] (em, instant, persistent) ilk L1'de
    diag_first_l2_ctx: (u64, u64, u64), // [EVIDENCE] ilk L2'de
    /// [HANDOFF ENSTRÜMANI] Son kararın anlık görüntüsü (zaman serisi örnekleyici okur).
    diag_last_ts: TsSample,
    /// [PERSIST ENSTRÜMANI] Yürürlükteki persistence ufku (son, tepe) ns.
    diag_persist_min_last_ns: u64,
    diag_persist_min_peak_ns: u64,
    /// [IW] PreConverge ilk-uçuş penceresi (0 = kapalı, sevk davranışı).
    initial_window: InitialWindowConfig,
    /// [FUSION-POLICY] Kalibre edilmiş füzyon karar katmanı (default `disabled()` → ArqOnly).
    fusion_policy: crate::fusion_policy::FusionPolicy,
    /// [FUSION-POLICY] Alınan RTT örneği sayısı. `FusionObservation.rtt_sample_count` kaynağı;
    /// güvenlik doğrulayıcısı `min_rtt_samples`(2) altında `LowConfidence`'a düşer — rtt.rs ilk
    /// örnekte `rttvar = srtt/2` bootstrap'lar, jitter yapay olarak HIGH okunur.
    rtt_sample_count: u32,
    /// [FUSION-POLICY] Telemetriyi besleyen son örnek anı (`telemetry_age_ns` tabanı).
    last_telemetry_ns: u64,
    /// [DEADLINE-EKSENİ] Politikaya giden SON gözlemin ham fotoğrafı (teşhis). Politika
    /// ateşlemediğinde "hangi değer kapıyı kapattı" sorusu bu olmadan cevaplanamaz — histogram
    /// KOVAYI söyler, DEĞERİ söylemez, ve kalibrasyon sınırının mı yoksa motorun mu yanlış
    /// olduğu ancak değere bakılarak ayrılır.
    diag_last_obs: Option<(u64, u64, u64, u32, Option<u32>)>,
    /// [TEŞHİS] Politikanın GÖRDÜĞÜ `bandwidth_bps` aralığı (min, max). Bölge sınırı ancak
    /// motorun gerçekten ürettiği büyüklükle ifade edilirse fence olur; link kapasitesiyle
    /// yazılmış bir sınır sessizce HER ZAMAN reddeder (v1 kalibrasyonunda tam bu oldu).
    diag_obs_bw_range: Option<(u64, u64)>,
    /// [TEŞHİS] Politikanın GÖRDÜĞÜ `loss_ppm` aralığı (min, max). Kalibrasyonun kayıp ekseni
    /// ancak motorun ürettiği ölçekte ifade edilirse kapı olur.
    diag_obs_loss_range: Option<(u64, u64)>,
    /// [DEADLINE-EKSENİ] Uygulamanın bildirdiği EN SIKI teslim bütçesi (ns). `None` = bildirim yok.
    /// Bu bir POLİTİKA girdisidir; ARQ/pacing/loss/RTT durumuna DOKUNMAZ (INV-RATE-1 korunur).
    tightest_deadline_ns: Option<u64>,
    /// [FUSION-POLICY/TEŞHİS] Füzyon aksiyon histogramı — "neden ateşlemedi" bu sayaçsız
    /// cevapsız kalırdı (yapı-gate dersi).
    diag_fusion_actions: [u64; crate::fusion_policy::FUSION_ACTION_KINDS],
    /// [TEŞHİS] Yapı-gate karar-yolu histogramı (bkz. `GateOutcome`).
    diag_gate_outcomes: [u64; GATE_OUTCOME_KINDS],
    /// [TEŞHİS] Ledger pencereli kaybının koşu-boyu TEPESİ (ppm) — anlık ölçer decay eder.
    diag_loss_ppm_peak: u64,

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
            fate_estimator: crate::fate_estimator::FateEstimator::new(),
            tx_queue: VecDeque::new(),
            pending_lost: VecDeque::new(),
            tail_recovery: TailRecovery::new(TailRecoveryConfig::default()), // default OFF
            return_path: crate::return_path::ReturnPathObserver::default(),
            startup_pacing: StartupPacingConfig::default(), // default OFF
            startup_pacing_floor_bps: 0,
            startup_floor_armed_once: false,
            startup_base_seen: false,
            bootstrap_rtt: BootstrapRttConfig::default(), // default OFF
            bs_armed_ns: None,
            bs_next_step: 0,
            bs_seq_next: 0,
            bs_outstanding: None,
            bs_bytes: 0,
            bs_done: false,
            bs_diag: BootstrapRttDiag::default(),
            #[cfg(feature = "diag-sched-telemetry")]
            sched_diag: SchedDiag::default(),
            tail_probe_pending: None,
            tail_probe_cancelled: 0,
            dup_queue: Vec::new(),
            establish_probe_enabled: true,
            establish_due_ns: 0,
            establish_interval_ns: 0,
            establish_probe_emitted: 0,
            last_now_ns: 0,
            fec_mode: FecActuationMode::Manual { enabled: false }, // default: mevcut-davranış-birebir
            // Commit-K: process-yerel-kimlik. Relaxed-yeterli (yalnız-uniqueness; ordering-anlamı-yok).
            // Wrap 2^64-kuruluşta (ulaşılamaz-inşa-gereği; belgeli). Wire/golden/M1-çıktılarına-GİRMEZ.
            instance_id: NEXT_SENDER_INSTANCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fec_authority_generation: 0,
            last_latch: None,
            latch_event_index: 0,
            no_eligible_decision_count: 0,
            decision_event_mismatch_count: 0,
            no_rule_zero_count: 0,
            manual_toggle_rejected_count: 0,
            parity_enqueued_total: 0,
            parity_emitted_total: 0,
            parity_canceled_total: 0,
            parity_enqueue_rejected_count: 0,
            foreign_authority_rejected_count: 0,
            token_denied_r_gauge: 0,
            #[cfg(feature = "experimental-fec-budget")]
            budget_ext: BudgetExt::default(),
            last_token_denial: None,
            token_denial_count_total: 0,
            last_transition_event_index: 0,
            transition_applied_count: 0,
            pending_fec: VecDeque::new(),
            #[cfg(feature = "experimental-cold-start")]
            cs_wakeup: Vec::new(),
            #[cfg(feature = "experimental-cold-start")]
            cs_hook_fired: Vec::new(),
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_armed_once: false,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_lane_enabled: true,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_disarmed: false,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_seq_next: 0,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_seq_wrapped: 0,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_pending: VecDeque::new(),
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_emitted: 0,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_tx_bytes: 0,
            #[cfg(feature = "experimental-cold-start")]
            cs_probe_outstanding: Vec::new(),
            #[cfg(feature = "experimental-cold-start")]
            cs_resolved: None,
            #[cfg(feature = "experimental-cold-start")]
            cs_epoch_timed_out: false,
            #[cfg(feature = "experimental-cold-start")]
            cs_induced_units: BTreeSet::new(),
            #[cfg(feature = "experimental-cold-start")]
            cs_induced_retx_bytes: 0,
            #[cfg(feature = "experimental-cold-start")]
            cs_srtt_reported: false,
            #[cfg(feature = "experimental-cold-start")]
            cs_terminal: None,
            #[cfg(feature = "experimental-cold-start")]
            cs_cold_epoch: 0,
            #[cfg(feature = "experimental-cold-start")]
            post_timeout_notice: None,
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
            diag_structure_gate_opened: 0,
            fec_structure_policy: FecStructurePolicy::Q18Iid,
            structure_gate_level_permille: None,
            loss_confirm_horizon: LossConfirmHorizon::CadenceLegacy,
            fec_loss_risk: FecLossRiskConfig::default(),
            diag_t_first_risk_ns: 0,
            diag_t_first_parity_ns: 0,
            diag_risk_ppm_last: 0,
            diag_risk_ppm_peak: 0,
            diag_arbiter_reasons: [0; ARBITER_REASON_KINDS],
            diag_spec_levels: [0; 3],
            diag_persistent_ppm_peak: 0,
            diag_reorder_pressure_last: 0,
            diag_reorder_pressure_peak: 0,
            diag_t_first_l1_ns: 0,
            diag_t_first_l2_ns: 0,
            diag_preconverge_parity_blocked: 0,
            repair_loss_not_arqed: 0,
            diag_parity_budget_clamped: 0,
            diag_parity_budget_would_zero: 0,
            diag_first_l1_ctx: (0, 0, 0),
            diag_first_l2_ctx: (0, 0, 0),
            diag_last_ts: TsSample::default(),
            diag_persist_min_last_ns: 0,
            diag_persist_min_peak_ns: 0,
            initial_window: InitialWindowConfig::default(),
            fusion_policy: crate::fusion_policy::FusionPolicy::disabled(),
            rtt_sample_count: 0,
            last_telemetry_ns: 0,
            tightest_deadline_ns: None,
            diag_last_obs: None,
            diag_obs_bw_range: None,
            diag_obs_loss_range: None,
            diag_fusion_actions: [0; crate::fusion_policy::FUSION_ACTION_KINDS],
            diag_gate_outcomes: [0; GATE_OUTCOME_KINDS],
            diag_loss_ppm_peak: 0,
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

    /// DR-CS-2 / DR-RATE-RAMP-1 (feature-gated): observation-only scheduler telemetry snapshot.
    #[cfg(feature = "diag-sched-telemetry")]
    #[must_use]
    pub fn sched_diag(&self) -> SchedDiag {
        self.sched_diag
    }

    /// DR-CS-3: product config — shipping bootstrap RTT probe (default OFF). YALNIZ DR-CS-3
    /// merdivenini anahtarlar (kama-integration DR-CS-3 paneli legacy F2A ladder'ı kontrol kolu olarak
    /// bundan bağımsız kullanır) — ürün OFF'u için bkz. [`Self::set_cs_probe_lane`].
    pub fn set_bootstrap_rtt(&mut self, cfg: BootstrapRttConfig) {
        self.bootstrap_rtt = cfg;
    }

    /// [OFF = OFF] DR-CS-1 M2 (F2A) CS-probe lane'inin ÜRÜN anahtarı; feature derlenmemişse no-op.
    /// client-core runtime `RuntimeConfig.bootstrap_rtt`'yi buraya da yansıtır: belgelenmiş
    /// "`bootstrap_rtt: false` ⇒ motor probe PLANLAMAZ" HER cold-start lane'i kapsar. Aksi hâlde
    /// `--all-features` derlemesinde F2A merdiveni ilk DATA emisyonunda koşulsuz arm oluyordu
    /// (client-core `bootstrap_rtt_probe_and_echo_traverse_the_product_wire` OFF kolu).
    pub fn set_cs_probe_lane(&mut self, on: bool) {
        #[cfg(feature = "experimental-cold-start")]
        {
            self.cs_probe_lane_enabled = on;
        }
        #[cfg(not(feature = "experimental-cold-start"))]
        {
            let _ = on;
        }
    }

    /// DR-CS-3 observation-only bootstrap lineage.
    #[must_use]
    pub fn bootstrap_rtt_diag(&self) -> BootstrapRttDiag {
        self.bs_diag
    }

    /// DR-CS-3 observation-only: remaining sender probe budget (bytes).
    #[must_use]
    pub fn bootstrap_budget_remaining(&self) -> u64 {
        BS_TX_HARD_BUDGET.saturating_sub(self.bs_bytes)
    }

    /// DR-RATE-RAMP-2 (§FAZ-1): product config — startup pacing initialization (default OFF).
    pub fn set_startup_pacing_init(&mut self, cfg: StartupPacingConfig) {
        self.startup_pacing = cfg;
    }

    /// DR-RATE-RAMP-2 observation-only: the active bootstrap pacing floor (B/s); 0 = inactive.
    #[must_use]
    pub fn startup_pacing_floor_bps(&self) -> u64 {
        self.startup_pacing_floor_bps
    }

    /// DR-RATE-RAMP-1 (observation-only): `(pacer_rate_bps, tokens, burst_cap)`.
    #[must_use]
    pub fn diag_pacer(&self) -> (u64, u64, u64) {
        (
            self.pacer.rate(),
            self.pacer.tokens(),
            self.pacer.burst_cap(),
        )
    }

    /// DR-RATE-RAMP-1 (observation-only): BtlBw filter acceptance lineage
    /// `(offered, accepted, rejected_invalid, rejected_interval, rejected_app_limited)`.
    #[cfg(feature = "diag-sched-telemetry")]
    #[must_use]
    pub fn diag_bw_accept(&self) -> (u64, u64, u64, u64, u64) {
        self.btlbw.diag_accept()
    }

    /// DR-CS-2 (observation-only): tail-probe `not-due → due` TRANSITION count. `tail_probe_diag*`
    /// exposes a LEVEL poll counter which must never be read as emission demand; this is the demand signal.
    #[must_use]
    pub fn tail_probe_due_edge_count(&self) -> u64 {
        self.tail_recovery.probe_due_edge_count()
    }

    /// DR-ARQ-PRODUCT-TAIL-1: product config — enable/tune the bounded tail-recovery mechanism (default OFF).
    /// Additive; does not touch any other sender state. INV-CS-4 supersession (docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD).
    pub fn set_tail_recovery_config(&mut self, cfg: TailRecoveryConfig) {
        self.tail_recovery = TailRecovery::new(cfg);
        self.tail_probe_pending = None;
        self.tail_probe_cancelled = 0;
    }

    /// FS-CAL-2 v3 §3 loss-STRUCTURE snapshot (observation-only).
    ///
    /// Returns the estimator's current window features. `burst_delta_ppm = p11 - p01` is the
    /// quantity Q17 proved is missing from the FEC gate: at one loss RATE, IID structure wants
    /// coding and burst structure wants ARQ, so a rate-only threshold cannot separate them.
    ///
    /// Nothing in the engine reads this. It is exposed so the input exists and can be measured
    /// before any rule is allowed to consume it — see reports/fs-fusion-cell-1/Q18-CALIBRATION-DEFECT.md.
    #[must_use]
    pub fn diag_loss_structure(&self) -> crate::fate_estimator::WindowFeatures {
        self.fate_estimator.snapshot()
    }

    /// S-0.7 correction-leg diagnostics (observation-only):
    /// `(late_ack_corrections, spurious_loss_corrections, loss_to_success_corrections)`.
    #[must_use]
    pub fn diag_fate_corrections(&self) -> (u64, u64, u64) {
        let (late, spurious) = self.fate_estimator.corrections();
        (
            late,
            spurious,
            self.fate_estimator.loss_to_success_corrections(),
        )
    }

    /// Observation-only: whether the bounded tail-recovery mechanism is enabled. Exists so an
    /// arm selector (bench KAPI-1 W4) can PIN that its config call took effect; reads no timers.
    #[must_use]
    pub fn tail_recovery_enabled(&self) -> bool {
        self.tail_recovery.is_enabled()
    }

    /// DR-ARQ-PRODUCT-TAIL-1 diagnostics (observation-only): (probes, probe_bytes, spurious, spurious_bytes).
    #[must_use]
    pub fn tail_probe_diag(&self) -> (u64, u64, u64, u64) {
        (
            self.tail_recovery.probe_count(),
            self.tail_recovery.probe_bytes(),
            self.tail_recovery.spurious_probe_count(),
            self.tail_recovery.spurious_probe_bytes(),
        )
    }

    /// DR-ARQ-PRODUCT-TAIL-1 extended diagnostics (observation-only, §7 separable probe accounting):
    /// (probe_due, probe_sent, probe_cancelled, probe_bytes, spurious_count, timer_fire_count).
    /// `probe_due` = eligible; `probe_sent` = emitted (cap-consuming); `probe_cancelled` = ACK-race cancels;
    /// `probe_due - probe_sent - probe_cancelled` = token-starved (kept pending). Never affects wire/decisions.
    #[must_use]
    pub fn tail_probe_diag_ext(&self) -> (u64, u64, u64, u64, u64, u64) {
        (
            self.tail_recovery.probe_due_count(),
            self.tail_recovery.probe_count(),
            self.tail_probe_cancelled,
            self.tail_recovery.probe_bytes(),
            self.tail_recovery.spurious_probe_count(),
            self.tail_recovery.timer_fire_count(),
        )
    }

    /// Ana döngü (§3): olay işle → FuzzyEvaluate → state → send_rate → UpdatePacing → ScheduleTransmit.
    ///
    /// DR-CS-1 (EK5-v): flag-ON'da tekil `on_event` = adapter'ın batch-of-1 sarmalaması (TÜM
    /// olaylar dört-faz-pipeline'dan geçer; Tick ≡ boş-batch). Flag-OFF = legacy-yol BİREBİR.
    pub fn on_event(&mut self, event: Event, now_ns: u64) -> Vec<TransmitDecision> {
        #[cfg(feature = "experimental-cold-start")]
        {
            let inbound = if matches!(event, Event::Tick) {
                Vec::new()
            } else {
                vec![event]
            };
            self.on_event_batch(now_ns, inbound)
        }
        #[cfg(not(feature = "experimental-cold-start"))]
        {
            // DR-S5-4 SAF-LOGICAL erken-dönüş: RECOVERED-control pacer-saatine/fuzzy/rate/schedule'a
            // HİÇ girmez (emit-yok, kredi-yok, saat-ilerletme-yok — N8-bit-özdeşliğin yapısal-tabanı).
            if let Event::RecoveredControl(info) = &event {
                self.on_recovered_control(info);
                return Vec::new();
            }
            self.on_event_core(event, now_ns);
            self.schedule_transmit(now_ns)
        }
    }

    /// on_event çekirdeği: dispatch + fuzzy + state + rate/burst — `schedule_transmit`-HARİÇ.
    /// DR-CS-1 M1 batch-girişi bu çekirdeği paylaşır; flag-OFF davranışı BİREBİR (salt-dikiş-
    /// refaktörü; tek-output-seam schedule_transmit'te kalır).
    fn on_event_core(&mut self, event: Event, now_ns: u64) {
        let dt = Duration::from_nanos(now_ns.saturating_sub(self.last_now_ns));
        self.pacer.refill(dt);
        self.last_now_ns = now_ns;
        // FS-CAL-2 v3 §3 lifecycle (design: docs/design/ESTIMATOR-LIFECYCLE.md). Finalize
        // guard-elapsed provisional losses at the engine clock, BEFORE this event's evidence is
        // fed: an ACK arriving at now >= expiry is late by the module contract (the fate sealed at
        // expiry), and advance-first realises exactly that — feed-first would silently extend the
        // guard to "the next event". Evidence fed later in THIS event gets expiry = now + guard
        // > now, so it cannot be finalized prematurely by this call. TimerFired flows through this
        // core, so a flow that goes silent after losses still finalizes on PTO ticks — the exact
        // degenerate case S-ANSWERS recorded (advance had zero product callers; the product window
        // held successes only and n10+n11 was structurally zero). RecoveredControl never reaches
        // this core, preserving its documented clock isolation. Observation-only: nothing reads
        // the estimator except diag_loss_structure (pinned by loss_structure_observation.rs).
        self.fate_estimator.advance(now_ns);

        match event {
            Event::AppData(unit) => {
                self.tx_queue.push_back(unit);
                #[cfg(feature = "diag-sched-telemetry")]
                {
                    self.sched_diag.data_enqueued += 1;
                }
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
            // on_event/on_event_batch girişinde ele-alınır (buraya ulaşmaz; exhaustiveness).
            #[cfg(feature = "experimental-cold-start")]
            Event::Tick => {}
            // DR-CS-1 M3: echo-ingest (yalnız-RTT; N8-spot-izolasyon) → pipeline-devamı state'i
            // doğal-PreConverge→Startup geçirir (mevcut srtt.is_some kapısı).
            // DR-CS-3: UNGATED — authenticated `0x1A` echo ingest on the shipping path.
            Event::CsProbeEcho { probe_seq } => {
                #[cfg(feature = "experimental-cold-start")]
                self.cs_on_probe_echo(probe_seq, now_ns);
                self.bs_on_echo(probe_seq, now_ns);
            }
        }

        let decision = self.fuzzy_evaluate(now_ns);
        self.last_decision = decision;
        let prev_state = self.state;
        self.update_state(&decision, now_ns);
        // DR-RATE-RAMP-2 (§FAZ-1, default OFF): arm the bootstrap pacing floor at the PreConverge exit.
        if self.startup_pacing.enabled
            && !self.startup_floor_armed_once
            && prev_state == SenderState::PreConverge
            && self.state != SenderState::PreConverge
        {
            self.startup_pacing_floor_bps = self.grounded_startup_rate();
            self.startup_floor_armed_once = true; // may never arm again (§1A.2)
        }
        let natural_rate = self.compute_send_rate(&decision);
        // §1A.2: the structural HARD ceiling `cwnd/srtt` has the LAST WORD at every application, not just
        // at arming — so if the congestion/safety cap SHRINKS (e.g. after loss), the floor shrinks with it
        // and can never push the sender past the current cap.
        let effective_floor = self.startup_pacing_floor_bps.min(self.cwnd_over_srtt());
        // Bootstrap floor: a low self-clocked sample may not immediately drag the freshly initialized
        // pacing rate back down. `natural_rate` is left untouched so the release test below is honest.
        let send_rate = natural_rate.max(effective_floor);
        self.pacer.set_rate(send_rate);
        // [R3/ADIM-6] KANAL-TÜREVLİ burst cap: max(1 birim, rate·pencere), üstten ~2×MTU. Bant-kör 2400B
        // sabiti yerine → PreConverge floor'da 1-birim (self-congestion/backlog-spike yok), Startup+'ta
        // ölçülen bw'ye orantılı micro-burst. front yoksa MTU (güvenli 1-paket tabanı).
        let front_size = self.tx_queue.front().map_or(DEFAULT_MTU, |u| u.size);
        let window_bytes = send_rate.saturating_mul(BURST_WINDOW_NS) / 1_000_000_000;
        // [IW] PreConverge'de ilk-uçuş tabanı: açıkken clamp'in ALT sınırı `front_size` yerine
        // ilk-uçuş bütçesidir (üst sınır da onu kapsayacak şekilde yükselir), böylece
        // `TokenBucket::new`'in DOLU başlattığı kredi tek birime kırpılıp ATILMAZ. Kapalıyken
        // (bytes == 0) ifade eskisiyle BİREBİR aynıdır. Yalnız PreConverge'i etkiler: ölçüm
        // geldikten sonra pencere zaten kanal-türevlidir.
        let iw = if self.state == SenderState::PreConverge {
            self.initial_window.bytes
        } else {
            0
        };
        let burst_lo = front_size.max(iw);
        let burst_hi = front_size.max(2 * DEFAULT_MTU).max(iw);
        self.pacer
            .set_burst_cap(window_bytes.clamp(burst_lo, burst_hi));
        if prev_state == SenderState::PreConverge && self.state != SenderState::PreConverge {
            // §3.3-D cold-start çıkışı: kanal-türevli micro-burst grant (STARTUP probing; kuyruk kanal-uygun
            // küçük burst'le boşalır — bu birimler STARTUP'ta emit → §3.3-B geometrik kopya ALMAZ).
            self.pacer.fill_to_cap();
        }
        // DR-RATE-RAMP-2: BOOTSTRAP-ONLY. Release the floor permanently once EITHER the existing Startup
        // exit condition has fired (the ramp ended: `base_is_startup` cleared at the plateau, `sender.rs`
        // update_state) OR the naturally derived rate has caught up on its own. Never a permanent minimum.
        //
        // NOTE: `plateau_reached` must NOT be used here — `update_state` CONSUMES it in the same call
        // (Startup→Drain sets it back to false), so it is not observable at this point. `base_is_startup`
        // is the durable "ramp finished" signal. Pinned by
        // `startup_floor_release_is_deterministic_and_permanent`.
        // The bootstrap window is over once the sender has left the Startup ramp (and its
        // recovery overlays) for a post-ramp phase — Drain / Steady / ProbeUp / DrainRtprop.
        // NOTE: neither `plateau_reached` nor `base_is_startup` works as the release signal:
        // `update_state` CONSUMES `plateau_reached` in the same call (Startup→Drain resets it), and it
        // RE-ARMS `base_is_startup` on every event while `state == Startup`. The phase itself is the
        // only durable signal. Pinned by `startup_floor_release_is_deterministic_and_permanent`.
        let in_startup_ramp = matches!(
            self.state,
            SenderState::PreConverge
                | SenderState::Startup
                | SenderState::RecoverArq
                | SenderState::RecoverFwd
        );
        // The latch keys on the ACTUAL Startup ramp (not PreConverge, and not a recovery overlay that
        // may be entered before the ramp ever starts): only once the sender is genuinely ramping can
        // "the ramp has ended" become a meaningful release signal.
        if self.state == SenderState::Startup {
            self.startup_base_seen = true;
        }
        if self.startup_pacing_floor_bps > 0
            && ((self.startup_base_seen && !in_startup_ramp)
                || natural_rate >= self.startup_pacing_floor_bps)
        {
            self.startup_pacing_floor_bps = 0; // permanent: `startup_floor_armed_once` blocks re-arming
        }
    }

    /// DR-RATE-RAMP-2 (§FAZ-1) grounded startup pacing rate, from EXISTING product constants only:
    ///
    /// ```text
    /// grounded = min( STARTUP_gain × (min_cwnd / srtt),   // instructed initialization
    ///                 cwnd / srtt )                        // rate.rs structural HARD ceiling
    /// ```
    ///
    /// `min_cwnd = SrcConfig::min_cwnd` (4×DEFAULT_MTU) and `STARTUP_gain = Gain::STARTUP` (2885/1000)
    /// are the shipping values — no new constant is introduced. The second term is mandatory: `cwnd/srtt`
    /// is documented in `rate.rs` as the structural hard ceiling ("son söz"), so the initialization must
    /// not exceed the rate the product's own cwnd policy already permits. In the observed lock the
    /// ceiling BINDS (it is the smaller term). All arithmetic is u128-widened and saturating.
    fn grounded_startup_rate(&self) -> u64 {
        let Some(srtt) = self.rtt.srtt() else {
            return 0; // no measurement ⇒ no grounded rate ⇒ mechanism inert
        };
        let srtt_ns = dur_ns(srtt);
        if srtt_ns == 0 {
            return 0;
        }
        let initialized = Gain::STARTUP.apply(rate_per_srtt(self.src_cfg.min_cwnd, srtt_ns));
        initialized.min(self.cwnd_over_srtt())
    }

    /// §1A.2 structural HARD ceiling `cwnd/srtt` (`rate.rs`: "son söz"), evaluated at the CURRENT
    /// congestion state. `cwnd = max(cwnd_gain × BDP, min_cwnd)`. 0 when no RTT sample exists.
    fn cwnd_over_srtt(&self) -> u64 {
        let Some(srtt) = self.rtt.srtt() else {
            return 0;
        };
        let srtt_ns = dur_ns(srtt);
        if srtt_ns == 0 {
            return 0;
        }
        let bdp = bdp_bytes(self.btlbw.btlbw(), self.rtt.rtprop().unwrap_or(INITIAL_RTT));
        let cwnd = self
            .src_cfg
            .cwnd_gain
            .saturating_mul(bdp)
            .max(self.src_cfg.min_cwnd);
        rate_per_srtt(cwnd, srtt_ns)
    }

    // ── DR-CS-1 M1 (EK5): ATOMİK BATCH GİRİŞİ + next_wakeup_ns + faz-makinesi ────────────────
    // Flag-OFF'ta bu blok DERLENMEZ (INV-CS-5: legacy tekil-yol byte-for-byte). Scope: bağlanan
    // timer'lar = CS-probe/CS-giveup/(d)-PTO (F2A/F2B'de eklenir) + TEST-ONLY hook (F1-atomiklik-
    // kanıtları). Genel-tam-wakeup-API iddiası YOK.

    /// DR-CS-1 M1 — TEK atomik giriş noktası (EK5-E1). Dört faz sırayla, tek çağrıda:
    /// 1) authenticated-inbound işle (tip-önceliği: DATA-ACK → [probe-echo F2B] → varış-sırası)
    /// 2) state-resolution / cancellation / disarm
    /// 3) due exposed-timer'lar BİR KEZ (retire-on-fire)
    /// 4) output-scheduling (tek schedule_transmit)
    ///
    /// Post-condition (C2, NORMATİF): dönüşte due exposed-timer kalmaz; `next_wakeup_ns()` ∈
    /// {None, gelecek} (busy-loop yapısal-imkansız). `Event::Tick` ≡ boş-batch; E2: boş-batch +
    /// due-yok + bekleyen-iş-yok → EXACT-NO-OP. RecoveredControl-only batch = legacy-özdeş
    /// saf-logical (saat/schedule-ilerletmez; N8-tabanı) — due-timer-yoksa boş döner.
    #[cfg(feature = "experimental-cold-start")]
    pub fn on_event_batch(
        &mut self,
        now_ns: u64,
        mut inbound: Vec<Event>,
    ) -> Vec<TransmitDecision> {
        let any_due = self.cs_any_due(now_ns);
        // E2 exact-no-op: yalnız boş-batch (Tick) + due-yok + bekleyen-scheduling-işi-yok.
        if inbound.is_empty() && !any_due && !self.cs_has_pending_work() {
            return Vec::new();
        }
        let was_empty = inbound.is_empty();
        // Faz-1: tip-önceliği (EK5-iv; stable-sort → aynı-tip varış-sırası korunur).
        inbound.sort_by_key(Self::cs_type_priority);
        let mut had_pipeline_inbound = false; // RecoveredControl/Tick pipeline'a girmez
        for ev in inbound {
            if let Event::RecoveredControl(info) = &ev {
                self.on_recovered_control(info); // saf-logical (DR-S5-4; N8-tabanı)
                continue;
            }
            if matches!(ev, Event::Tick) {
                continue; // batch-içi Tick anlamsız (boş-batch-eşdeğeri); yut
            }
            had_pipeline_inbound = true;
            self.on_event_core(ev, now_ns);
        }
        if !had_pipeline_inbound {
            // İki-durum: boş-batch (Tick) veya RecoveredControl-only.
            if !any_due && !was_empty {
                // RecoveredControl-only + due-yok → legacy-özdeşlik: RecoveredControl tek-başına
                // saat-ilerletmez/emisyon-yaratamaz (N8) — iş-olsa-bile boş döner.
                return Vec::new();
            }
            if !any_due && !self.cs_has_pending_work() {
                return Vec::new(); // savunmacı (E2'de-yakalanmıştı)
            }
            // Tick+iş VEYA due-timer-var → saat/refill ilerlet (core-refill muadili) → faz-2..4.
            let dt = Duration::from_nanos(now_ns.saturating_sub(self.last_now_ns));
            self.pacer.refill(dt);
            self.last_now_ns = now_ns;
        }
        // Faz-2: state-resolution / cancellation / disarm (F1: CS-state-yok; F2A/F2B genişletir).
        self.cs_phase2_resolve(now_ns);
        // Faz-3: due exposed-timer'lar BİR KEZ (retire-on-fire; deadline+ekleme-sırası).
        self.cs_phase3_fire_due(now_ns);
        // Faz-4: output-scheduling (tek-seam).
        let out = self.schedule_transmit(now_ns);
        // §2.4-a/C4: bootstrap-induced-retransmit atfı — bootstrap-anı-outstanding birimin
        // Retransmit-emisyonu (unit_id-eşleşme; birim-başına-BİR-kez; bounded).
        for d in &out {
            if d.kind == TransmitKind::Retransmit && self.cs_induced_units.remove(&d.unit_id) {
                self.cs_induced_retx_bytes += d.size;
            }
        }
        // C2 post-condition: batch-sınırında due-kalmadı.
        // C2 (NORMATİF, ORİJİNAL KÜMESİYLE KORUNDU): dönüşte due **CS exposed-timer** kalmaz —
        // `cs_wakeup` retire-on-fire disiplininin özelliğidir. EK4-A2 sonrası `next_wakeup_ns()`
        // daha GENİŞ bir şeydir (ARQ/dup/bootstrap/tail deadline'ları + pacer-token'a bağlı due-now
        // iş); bunlar batch dönüşünde meşru olarak due olabilir (ör. token-denied dup kaydı), bu
        // yüzden C2'nin kümesi GENİŞLETİLMEZ.
        debug_assert!(
            self.cs_wakeup
                .iter()
                .map(|&(d, _)| d)
                .min()
                .is_none_or(|w| w > now_ns),
            "C2-ihlali: batch-dönüşünde due-exposed-timer kaldı"
        );
        out
    }

    /// EK4-A2 — **EVENT-LOOP WAKEUP DEADLINE** (ns). Artık DEFAULT BUILD'de.
    ///
    /// Sözleşme (`EK4-A2-NEXT-WAKEUP-CONTRACT`, md `d59212b2…`): belgelenmiş immediate-output drain
    /// SONRASINDA timer-güdümlü bekleyen iş veya internal future deadline varsa **`None` DÖNMEZ**.
    /// `None` YALNIZ: terminal/closed · gerçekten idle · immediate output yok · internal deadline yok
    /// · ilerleme yalnız dış network input'una bağlı.
    ///
    /// Due-now iş varsa `Some(now)` döner — gizli "hemen tekrar poll et" `None` ASLA.
    /// Busy-loop kurulamaz: `Some(now)` yalnız gerçekten due iş varken döner ve iş tüketilince
    /// kendiliğinden düşer (kaynaklar tükendiğinde aday üretmezler).
    ///
    /// **YENİ TIMER İCAT ETMEZ; İKİNCİ TRUTH-SOURCE DEĞİL.** Yalnız MEVCUT authoritative
    /// kaynakların en erken deadline'ını toplar (§1.2 envanteri):
    ///   1. `cs_wakeup` (CS-probe/giveup/BootstrapPto) — feature-gated; kapalıyken aday üretmez
    ///   2. ARQ time-threshold loss deadline (`loss.next_time_loss_deadline_ns`)
    ///   3. §3.3-B geometrik kopyalar (`dup_queue[*].due_ns`)
    ///   4. DR-CS-3 bootstrap merdiveni (`bs_armed_ns + BS_FIRES_NS[bs_next_step]`)
    ///   5. Tail-recovery PTO (`armed_at_ns + current_pto_ns`)
    ///   6. Pacer token yenilenmesi — bekleyen iş VARSA (`tx_queue|pending_lost|dup_queue|pending_fec`)
    ///      en küçük bekleyen birimin gönderilebilir olacağı an
    #[must_use]
    pub fn next_wakeup_ns(&self) -> Option<u64> {
        // Kaynak 1-5 (timer'lar) + kaynak 6 (pacer-token'a bağlı due-now iş).
        Self::earlier(self.next_timer_deadline_ns(), self.pending_work_ready_ns())
    }

    fn earlier(a: Option<u64>, b: Option<u64>) -> Option<u64> {
        match (a, b) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (x, None) => x,
            (None, y) => y,
        }
    }

    /// EK4-A2 kaynak 1-5: yalnız **TIMER** deadline'ları (immediate-readiness HARİÇ).
    ///
    /// C2 post-condition (`on_event_batch` dönüşünde due exposed-timer kalmaz) BU kümeye bağlıdır ve
    /// anlamı DEĞİŞMEZ. Immediate-readiness (kaynak 6) bir timer değildir — pacer token'ı bekleyen
    /// due-now iştir — bu yüzden C2'yi ihlal etmez, ama `next_wakeup_ns()`'e dahil olmak ZORUNDADIR
    /// (yoksa event-loop uyanmaz: EK4-A2'nin kök nedeni tam olarak buydu).
    #[must_use]
    fn next_timer_deadline_ns(&self) -> Option<u64> {
        let earlier = Self::earlier;
        let mut best: Option<u64> = None;

        // 1. CS exposed-timer kümesi (flag-OFF'ta bu blok derlenmez → aday yok).
        #[cfg(feature = "experimental-cold-start")]
        {
            best = earlier(best, self.cs_wakeup.iter().map(|&(d, _)| d).min());
        }

        // 2. ARQ time-threshold loss deadline (aynı çağrı (d)-kapsamının kullandığı).
        if let Some(srtt) = self.rtt.srtt() {
            best = earlier(
                best,
                self.loss
                    .next_time_loss_deadline_ns(srtt, self.cadence_loss_floor()),
            );
        }

        // 3. §3.3-B zamanlanmış geometrik kopyalar — ELIGIBILITY + EMISSION birlikte.
        //
        // `due_ns` yalnız NE ZAMAN uygun olduğunu söyler; emisyon ayrıca pacer token'ına bağlıdır
        // (`drain_due_duplicates` → `try_send`). due geçmiş ama token yetmiyorsa kayıt kuyrukta
        // kalır ve `due_ns` DEĞİŞMEZ → stale değeri tekrar tekrar raporlamak ilerlemesiz uyanış
        // (busy-loop) üretir. Bu yüzden raporlanan an, scheduler'ın GERÇEKTEN işleyebileceği andır:
        //   due gelecekte  → `due_ns`
        //   due geçmiş     → token'ın yeteceği an
        // (Yeni timer YOK: her iki girdi de mevcut authoritative durumdan okunur.)
        best = earlier(
            best,
            self.dup_queue
                .iter()
                .filter_map(|r| {
                    if r.due_ns > self.last_now_ns {
                        Some(r.due_ns)
                    } else {
                        self.pacer
                            .ns_until_sendable(r.unit.size)
                            .map(|dt| self.last_now_ns.saturating_add(dt))
                    }
                })
                .min(),
        );

        // 3b. [DR-EST-1] Kuruluş-denemesi vadesi (yalnız PreConverge'de armed; token'sız kol
        // olduğundan stale-busy-loop riski yok — vadesi gelince EMİT EDER ve yeniden kurar).
        if self.establish_probe_enabled
            && self.state == SenderState::PreConverge
            && self.establish_due_ns > 0
        {
            best = earlier(best, Some(self.establish_due_ns));
        }

        // 4. DR-CS-3 bootstrap merdiveni (config default OFF ⇒ aday yok).
        if self.bootstrap_rtt.enabled && !self.bs_done {
            if let (Some(armed), Some(&off)) = (
                self.bs_armed_ns,
                self.bootstrap_rtt
                    .ladder()
                    .get(self.bs_next_step as usize)
                    .copied()
                    .as_ref(),
            ) {
                best = earlier(best, Some(armed.saturating_add(off)));
            }
        }

        // 5. Tail-recovery PTO (config default OFF ⇒ aday yok).
        best = earlier(best, self.tail_probe_deadline_ns());

        // NOT: `LostRec.deadline_ns` BİLEREK kaynak DEĞİL. `arq.rs:110-111` normatif: "deadline YALNIZ
        // SIRA — expired seq DROP EDİLMEZ, planda kalır" ⇒ o bir TESLİM-önceliği anahtarıdır, zamanlayıcı
        // değil. Wakeup kaynağı yapmak ilerlemesiz uyanış üretirdi. `pending_lost` zaten `update_state`
        // ile RecoverArq'a geçer ve her turda planlanır; onu bekleten tek şey token'dır (kaynak 6).

        best
    }

    /// EK4-A2 kaynak-5: tail-recovery PTO deadline'ı (SALT-OKUMA; mevcut state'ten türetilir).
    fn tail_probe_deadline_ns(&self) -> Option<u64> {
        if !self.tail_recovery.is_enabled() {
            return None;
        }
        let armed = self.tail_recovery.armed_at_ns()?;
        let pt = self.path_timing_snapshot();
        Some(armed.saturating_add(self.tail_recovery.current_pto_ns(pt.srtt_ns, pt.rttvar_ns)))
    }

    /// EK4-A2 kaynak-6: pacer-token'a bağlı bekleyen iş için ilerleme anı (SALT-OKUMA).
    ///
    /// Bekleyen küme = her turda KOŞULSUZ drain edilen sınıflar (`tx_queue`, `pending_fec`). Bu iş
    /// "due"dur — onu bekleten tek şey pacer token'ıdır, bir timer değil. Bu
    /// yüzden ilerleme anı `pacer.ns_until_sendable(en_küçük_bekleyen_birim)`'dir; token zaten
    /// yeterliyse `Some(now)` (immediate readiness).
    ///
    /// `None`: bekleyen iş yok, YA DA hiçbir bekleyen birim token'la gönderilemez (rate 0 /
    /// birim > burst_cap) — ikincisinde ilerleme zamana değil dış olaya bağlıdır, bu yüzden
    /// deadline uydurmak yanlış olur.
    fn pending_work_ready_ns(&self) -> Option<u64> {
        // Her turda planlanabilen sınıflar: `drain_new_data`, `plan_fec_parity` ve `pending_lost`
        // (`update_state` `pending_lost` boş değilken RecoverArq'a geçer ⇒ `plan_arq_retransmits`
        // aynı turda çalışır; onu bekleten tek şey token'dır). `dup_queue` BİLEREK dışarıda: kendi
        // `due_ns` deadline'ı var (kaynak 3).
        let smallest = self
            .tx_queue
            .iter()
            .map(|u| u.size)
            .chain(self.pending_fec.iter().map(|p| p.size))
            .chain(self.pending_lost.iter().map(|r| r.unit.size))
            .min()?;
        let dt = self.pacer.ns_until_sendable(smallest)?;
        Some(self.last_now_ns.saturating_add(dt))
    }

    #[cfg(feature = "experimental-cold-start")]
    fn cs_type_priority(e: &Event) -> u8 {
        match e {
            Event::AckReceived(_) => 0,     // DATA-ACK önce (EK5-iv)
            Event::CsProbeEcho { .. } => 1, // probe-echo ikinci
            _ => 2,                         // aynı-tip-varış-sırası (stable-sort)
        }
    }

    #[cfg(feature = "experimental-cold-start")]
    fn cs_any_due(&self, now_ns: u64) -> bool {
        self.cs_wakeup.iter().any(|&(d, _)| d <= now_ns)
    }

    /// "Önceden-oluşmuş bekleyen scheduling-işi" (E2): kuyruklarda drain-bekleyen-iş var mı.
    #[cfg(feature = "experimental-cold-start")]
    fn cs_has_pending_work(&self) -> bool {
        !self.tx_queue.is_empty()
            || !self.pending_lost.is_empty()
            || !self.dup_queue.is_empty()
            || !self.pending_fec.is_empty()
    }

    /// DR-CS-1 M3 (F2B) — echo-ingest: `(probe_seq)`-lookup → TEK-TÜKETİM → YALNIZ rtt-estimator
    /// (matematik-değişmez; `source=ProbeEcho` etiketi diag'ta). LATE-ECHO-YOK-SAYMA (5-koşul):
    /// (1) data-ACK-srtt-oluşturduysa (2) seq-outstanding-değilse (3) session_generation/epoch-
    /// değiştiyse (in-instance-yeniden-giriş-İMKANSIZ — F0-tespit-(v); yeni-instance=yeni-namespace)
    /// (4) echo-zaten-işlendiyse (tek-tüketim=aynı-koşul-2-yolu) (5) epoch-TimedOut-kapandıysa.
    #[cfg(feature = "experimental-cold-start")]
    fn cs_on_probe_echo(&mut self, probe_seq: u16, now_ns: u64) {
        if self.cs_epoch_timed_out {
            return; // koşul-5: post-timeout-echo YOK-SAYILIR (Seçenek-A; probe_seq'ler-retire'lı)
        }
        if self.rtt.srtt().is_some() {
            return; // koşul-1 (data-ACK-önce-kurdu) — koşul-4'ü-de-kapsar (ilk-echo-sonrası)
        }
        let Some(pos) = self
            .cs_probe_outstanding
            .iter()
            .position(|&(s, _)| s == probe_seq)
        else {
            return; // koşul-2/4: outstanding-değil (hiç-emit-edilmedi / zaten-tüketildi / retire'lı)
        };
        let (_, emit_ns) = self.cs_probe_outstanding.swap_remove(pos); // TEK-TÜKETİM
        let sample = Duration::from_nanos(now_ns.saturating_sub(emit_ns));
        // Bootstrap-anı-outstanding DATA kümesi (induced-retx-atfı §2.4-a; bounded=inflight).
        self.cs_induced_units = self.inflight.values().map(|p| p.unit.id).collect();
        // YALNIZ RTT-estimator (mevcut observe; matematik-değişmez). cwnd/BtlBw/delivery/loss/
        // ack-conf'a DOKUNULMAZ (N8-spot-testli).
        self.rtt.observe(now_ns, sample);
        // [FUSION-POLICY] Örnek sayacı + telemetri damgası: güvenlik doğrulayıcısının güven ve
        // tazelik kapıları bunlara bakar (min_rtt_samples / max_telemetry_age_ns).
        self.rtt_sample_count = self.rtt_sample_count.saturating_add(1);
        self.last_telemetry_ns = now_ns;
        self.cs_resolved = Some(CsSrttSource::ProbeEcho);
    }

    /// Faz-2: state-resolution/cancellation/disarm. (a) srtt-oluştuysa CS-probe-merdiveni DISARM
    /// (kalan-fire'lar + pending iptal; C3: ACK/echo-önce-işlendiği-için aynı-batch'te fire-
    /// edilmeden-disarm) + INV-CS-3 terminal-kaynağı (DataAck-yolu burada-yakalanır) + outstanding-
    /// probe-retire. (b) (d)/EK4-A2: BootstrapPto-exposure canlı-loss-state'ten yeniden-türetilir
    /// (retire-all + re-register; ACK/recovery-temizliği-otomatik).
    #[cfg(feature = "experimental-cold-start")]
    fn cs_phase2_resolve(&mut self, _now_ns: u64) {
        // (a) ilk-srtt → ladder/pending/outstanding-disarm + kaynak-belirle (echo-yolu cs_resolved'ı
        //     zaten-ProbeEcho-yaptı; aksi DataAck).
        if self.cs_probe_armed_once && !self.cs_probe_disarmed && self.rtt.srtt().is_some() {
            self.cs_probe_disarmed = true;
            self.cs_wakeup
                .retain(|&(_, k)| !matches!(k, CsTimerKind::CsProbe(_)));
            self.cs_probe_pending.clear();
            self.cs_probe_outstanding.clear(); // epoch-kapanışı: outstanding-probe KESİN-retire
            if self.cs_resolved.is_none() {
                self.cs_resolved = Some(CsSrttSource::DataAck); // ResolvedByDataAck
            }
        }
        // (b) M5 terminal-raporlama (bir-kez; C3: ACK/echo faz-2 → giveup faz-3'te fire-etmez çünkü
        //     giveup burada-retire). TimedOut-sonrası-srtt → PostTimeoutRecovery (ikinci-terminal-değil).
        if self.cs_probe_armed_once && !self.cs_srtt_reported && self.rtt.srtt().is_some() {
            self.cs_srtt_reported = true;
            self.cs_wakeup
                .retain(|&(_, k)| !matches!(k, CsTimerKind::CsGiveup));
            self.cs_terminal = Some(if self.cs_epoch_timed_out {
                // K-8/F4 TETİK-2a: TimedOut-sonrası DATA-ACK-clocked heal → PostTimeoutNotice
                // (first-valid-proof; SALT-LOGICAL N8). block_id=cs_cold_epoch-bootstrap (F4-slice tek-block).
                self.record_post_timeout_notice(
                    0, // F4-slice bootstrap-block (tek-block; MAX_COLD_BLOCKS_PER_EPOCH=1)
                    PostTimeoutCause::DataAckClocked,
                    _now_ns,
                );
                CsTerminal::PostTimeoutRecovery
            } else {
                match self.cs_resolved {
                    Some(CsSrttSource::ProbeEcho) => CsTerminal::ResolvedByProbeEcho,
                    _ => CsTerminal::ResolvedByDataAck,
                }
            });
        }
        // (d)/EK4-A2: bootstrap-sonrası MEVCUT time-threshold deadline'ı expose (yaşam-döngüsü:
        // her-batch yeniden-türet → re-arm/ACK-retire otomatik; C2: overdue-ise-aynı-batch-faz-3).
        if self.cs_resolved.is_some() {
            self.cs_wakeup
                .retain(|&(_, k)| !matches!(k, CsTimerKind::BootstrapPto));
            if let Some(srtt) = self.rtt.srtt() {
                if let Some(d) = self
                    .loss
                    .next_time_loss_deadline_ns(srtt, self.cadence_loss_floor())
                {
                    self.cs_wakeup.push((d, CsTimerKind::BootstrapPto));
                }
            }
        }
    }

    /// Faz-3: due exposed-timer'ları ateşle (retire-on-fire; deadline-sırası). Dış-döngü:
    /// fire-yan-etkisi (BootstrapPto-re-arm floor-kayması-edge'i) yeni-due-üretirse AYNI-batch'te
    /// tüketilir (C2: geleceğe-erteleme-YASAK; ilerleme-garantili — her-tur-birim-declare-eder;
    /// güvenlik-tavanı debug_assert'li).
    #[cfg(feature = "experimental-cold-start")]
    fn cs_phase3_fire_due(&mut self, now_ns: u64) {
        let mut guard = 0u32;
        while self.cs_any_due(now_ns) {
            guard += 1;
            debug_assert!(guard <= 64, "faz-3 due-döngüsü ilerleme-kaybı");
            if guard > 64 {
                break; // release-güvenlik: kalan-due bir-sonraki-batch'te (anomali-korunması)
            }
            self.cs_phase3_fire_due_once(now_ns);
        }
    }

    #[cfg(feature = "experimental-cold-start")]
    fn cs_phase3_fire_due_once(&mut self, now_ns: u64) {
        // deadline-sırası deterministik (stable: eşit-deadline'da ekleme-sırası).
        let mut due: Vec<(u64, CsTimerKind)> = Vec::new();
        self.cs_wakeup.retain(|&(d, k)| {
            if d <= now_ns {
                due.push((d, k));
                false // retire-on-fire
            } else {
                true
            }
        });
        due.sort_by_key(|&(d, _)| d);
        for (_, kind) in due {
            match kind {
                CsTimerKind::TestHook(id) => self.cs_hook_fired.push((now_ns, id)),
                CsTimerKind::CsGiveup => {
                    // M5: resolved-değilse (faz-2 srtt-raporlamadıysa) TimedOut-terminal + epoch-
                    // kapanışı (INV-CS-3 tek-terminal; C3: aynı-batch'te-çözüm-olsaydı-giveup-faz-2'de-
                    // retire-edilirdi → buraya-ulaşmaz). outstanding-probe KESİN-retire (post-timeout-
                    // echo cs_epoch_timed_out-guard'ıyla-yok-sayılır).
                    if !self.cs_srtt_reported && !self.cs_epoch_timed_out {
                        self.cs_epoch_timed_out = true;
                        self.cs_terminal = Some(CsTerminal::TimedOut);
                        self.cs_probe_disarmed = true;
                        self.cs_wakeup
                            .retain(|&(_, k)| !matches!(k, CsTimerKind::CsProbe(_)));
                        self.cs_probe_pending.clear();
                        self.cs_probe_outstanding.clear();
                    }
                }
                CsTimerKind::BootstrapPto => {
                    // (d)-fire: MEVCUT Pto-core-yolu (dispatch+fuzzy+state — pending_lost-oluşursa
                    // update_state RecoverArq'a-geçer → faz-4 plan_arq AYNI-batch'te emit-eder;
                    // DI-1A "ek-RTT/ek-network-event-beklemez"). INV-CS-4: yeni-retransmit-yolu-YOK.
                    self.on_event_core(Event::TimerFired(Timer::Pto), now_ns);
                    // EK4-A2 re-arm: kalan-eligible-birim-varsa YENİ-deadline (zorunlu-gelecek:
                    // az-önce-fire-edilen-eşik-altı-birimler declare-edilip-inflight'tan-düştü).
                    if let Some(srtt) = self.rtt.srtt() {
                        if let Some(d) = self
                            .loss
                            .next_time_loss_deadline_ns(srtt, self.cadence_loss_floor())
                        {
                            self.cs_wakeup.push((d, CsTimerKind::BootstrapPto));
                        }
                    }
                }
                CsTimerKind::CsProbe(_idx) => {
                    // M2 fire: probe_seq ata (INV-CS-7 monoton; wrap→probe-YOK+typed-diag) +
                    // bütçe-kontrol (INV-CS-2 hard) → pacer-bekleyen-pending'e.
                    if self.cs_probe_disarmed {
                        continue;
                    }
                    if self.cs_probe_tx_bytes.saturating_add(CS_PROBE_WIRE_MAX)
                        > CS_PROBE_TX_HARD_BUDGET
                    {
                        continue; // hard-bütçe: emisyon-yok (nominal-176B zaten-altında)
                    }
                    if self.cs_probe_seq_next == u16::MAX {
                        self.cs_probe_seq_wrapped = self.cs_probe_seq_wrapped.saturating_add(1);
                        continue; // wrap → yeni-probe-YOK (typed-diag-sayacı)
                    }
                    let pseq = self.cs_probe_seq_next;
                    self.cs_probe_seq_next += 1;
                    #[cfg(feature = "diag-sched-telemetry")]
                    {
                        self.sched_diag.cs_probe_enqueued += 1;
                    }
                    self.cs_probe_pending.push_back(pseq);
                }
            }
        }
    }

    /// M2 emisyon-pass'i (F2A): pacer-bekleyen probe'ları paced-emit et. `schedule_transmit`'in
    /// SONUNA-eklenir (budget_settle-sonrası) → mühürlü-DATA-önce-sıralama + FEC-budget-penceresi
    /// DOKUNULMAZ (F0-tespit-(vii) ekleme-noktası). Plain `pacer.try_send` (44B) — TokenBucket-
    /// değişikliği-YOK; probe kendi-bütçesiyle-sınırlı (≤200B-toplam).
    #[cfg(feature = "experimental-cold-start")]
    fn drain_cs_probes(&mut self, out: &mut Vec<TransmitDecision>) {
        while let Some(&pseq) = self.cs_probe_pending.front() {
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.cs_probe_selected += 1;
            }
            if !self.pacer.try_send(CS_PROBE_WIRE_MAX) {
                #[cfg(feature = "diag-sched-telemetry")]
                {
                    self.sched_diag.cs_probe_token_denied += 1;
                }
                break; // token-yok → sonraki-event'te (pending-korunur)
            }
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.cs_probe_wire_emitted += 1;
            }
            self.cs_probe_pending.pop_front();
            self.cs_probe_emitted += 1;
            self.cs_probe_tx_bytes += CS_PROBE_WIRE_MAX;
            // F2B: echo-korelasyonu-için outstanding-kayıt (probe_seq, emit_ns=RTT-t0).
            self.cs_probe_outstanding.push((pseq, self.last_now_ns));
            out.push(TransmitDecision {
                kind: TransmitKind::CsProbe,
                seq: u64::from(pseq), // probe_seq (AYRI-namespace; data-seq-DEĞİL)
                unit_id: 0,
                size: CS_PROBE_WIRE_MAX,
                send_record: None,
            });
        }
    }

    /// DR-CS-1 diag (INV-CS-1/2 kanıt-sayaçları): (emitted, cold_probe_tx_bytes, seq_wrapped,
    /// pending_len, armed, disarmed).
    #[cfg(feature = "experimental-cold-start")]
    #[must_use]
    pub fn diag_cold_start(&self) -> (u64, u64, u64, usize, bool, bool) {
        (
            self.cs_probe_emitted,
            self.cs_probe_tx_bytes,
            self.cs_probe_seq_wrapped,
            self.cs_probe_pending.len(),
            self.cs_probe_armed_once,
            self.cs_probe_disarmed,
        )
    }

    /// DR-CS-1 M5 (F3) output-API: cold-start terminal-sonucunu DRAIN et (input-Event'ten-AYRI;
    /// epoch-başına-en-fazla-bir ResolvedBy*/TimedOut + ayrı PostTimeoutRecovery). Tek-tüketim.
    #[cfg(feature = "experimental-cold-start")]
    pub fn take_cs_terminal(&mut self) -> Option<CsTerminal> {
        self.cs_terminal.take()
    }

    /// K-8/F4: PostTimeoutNotice one-slot kaydı (first-valid-proof-wins). NOTICE_BOUND=1: slot-doluysa
    /// idempotent no-op (duplicate/sonraki-kanıt). SALT-LOGICAL — BtlBw/pacer/rate'e SIFIR-satır (N8).
    /// same-epoch/farklı-block: MAX_COLD_BLOCKS_PER_EPOCH=1 varsayımı (F4-slice tek-block); farklı-block
    /// gelirse slot-doluluğuyla no-op (invariant F5-pre-reg'de test-pinlenir).
    #[cfg(feature = "experimental-cold-start")]
    fn record_post_timeout_notice(&mut self, block_id: u32, cause: PostTimeoutCause, now_ns: u64) {
        if self.post_timeout_notice.is_none() {
            self.post_timeout_notice = Some(PostTimeoutNotice {
                cold_epoch: self.cs_cold_epoch,
                block_id,
                cause,
                proof_time_ns: now_ns,
            });
        }
    }

    /// K-8/F4: PostTimeoutNotice drain (ürün-katmanı pull-API; take = tek-tüketim).
    #[cfg(feature = "experimental-cold-start")]
    pub fn take_post_timeout_notice(&mut self) -> Option<PostTimeoutNotice> {
        self.post_timeout_notice.take()
    }

    /// DR-CS-1 F2B diag: (bootstrap-sonucu-kaynağı, outstanding-probe-sayısı,
    /// bootstrap_induced_retransmit_bytes, epoch_timed_out).
    #[cfg(feature = "experimental-cold-start")]
    #[must_use]
    pub fn diag_cs_bootstrap(&self) -> (Option<CsSrttSource>, usize, u64, bool) {
        (
            self.cs_resolved,
            self.cs_probe_outstanding.len(),
            self.cs_induced_retx_bytes,
            self.cs_epoch_timed_out,
        )
    }

    /// TEST-ONLY (F1 atomiklik-kanıtları; DI-12E/F/G F1-sürümleri): exposed-timer-hook arm.
    /// Üretim-yolu-değil (feature default-OFF + doc-hidden).
    #[cfg(feature = "experimental-cold-start")]
    #[doc(hidden)]
    pub fn cs_test_arm_hook_timer(&mut self, deadline_ns: u64, id: u8) {
        self.cs_wakeup
            .push((deadline_ns, CsTimerKind::TestHook(id)));
    }

    /// TEST-ONLY: hook-fire kayıtları (now_ns, id).
    #[cfg(feature = "experimental-cold-start")]
    #[doc(hidden)]
    #[must_use]
    pub fn cs_test_hook_fired(&self) -> &[(u64, u8)] {
        &self.cs_hook_fired
    }

    /// §3.2 OnAckReceived: inflight boşalt + RTT/BW/loss güncelle.
    fn on_ack_received(&mut self, ack: &AckInfo, now_ns: u64) {
        if ack.acked_seqs.is_empty() {
            return;
        }
        self.last_ack_ns = Some(now_ns);
        // DR-ARQ-PRODUCT-TAIL-1: snapshot inflight to detect REAL progress (new ack-eliciting data acked) vs a
        // duplicate/stale ACK that removes nothing. Only real progress resets the tail timer (§4).
        let inflight_before = self.inflight.len();

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
                // Observation-only success evidence, ORIGINALS ONLY — the same Karn discipline the
                // RTT sampler applies two lines below. A retransmit or duplicate proves nothing
                // about the original's fate, and feeding one would corrupt the structure counters.
                if p.origin == TransmitKind::Data {
                    self.fate_estimator.on_original_ack(p.unit.id);
                }
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
                                             // S-0.7 correction leg (ESTIMATOR-LIFECYCLE §6.1): the ORIGINAL's own seq was
                                             // late-acked — the product just disproved the loss it declared, so the estimator's
                                             // fate is corrected in place. ORIGINALS ONLY: a superseded transmission's late ACK
                                             // (kol-2, origin Retransmit) proves nothing about the original's fate.
                if t.origin == TransmitKind::Data {
                    self.fate_estimator.correct_original_delivered(t.unit.id);
                }
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
                                               // S-0.7 correction leg: un-loss = the declared-lost seq itself was acked, the loss
                                               // hypothesis is withdrawn. retx_count == 0 ⟺ this record is the ORIGINAL emission
                                               // (a lost retransmit re-enters pending_lost with retx_count ≥ 1 and its ACK proves
                                               // only the retransmit's delivery). Exactly-once is carried by the estimator method:
                                               // the same late ACK can hit both the tombstone branch and this drain in one event.
                if rec.retx_count == 0 {
                    self.fate_estimator.correct_original_delivered(rec.unit.id);
                }
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
            // [FUSION-POLICY] Örnek sayacı + telemetri damgası: güvenlik doğrulayıcısının güven ve
            // tazelik kapıları bunlara bakar (min_rtt_samples / max_telemetry_age_ns).
            self.rtt_sample_count = self.rtt_sample_count.saturating_add(1);
            self.last_telemetry_ns = now_ns;
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
        // DR-ARQ-PRODUCT-TAIL-1 (gated): real ACK progress (inflight shrank) resets the tail timer + clears backoff;
        // a drained inflight disarms. Duplicate/stale ACKs that remove nothing do NOT reset (§4).
        if self.tail_recovery.is_enabled() {
            if self.inflight.len() < inflight_before {
                self.tail_recovery.on_ack_progress(now_ns);
            }
            if self.inflight.is_empty() {
                self.tail_recovery.disarm();
            }
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
                // DR-ARQ-PRODUCT-TAIL-1 (gated; default OFF → skipped, OFF-mode byte-identical): a PTO with
                // outstanding data and no ACK progress for a PTO interval makes the OLDEST inflight probe-eligible.
                // Emission (token + re-verify) happens in plan_arq_retransmits; the attempt is spent only on a real
                // emission (§3 probe-due ≠ probe-sent).
                if self.tail_recovery.is_enabled() {
                    let oldest = self.inflight.keys().next().copied();
                    let srtt = self.rtt.srtt().map(dur_ns);
                    let rttvar = dur_ns(self.rtt.rttvar());
                    if let ProbeDecision::ProbeDue { target_seq, .. } =
                        self.tail_recovery.on_timer(now_ns, srtt, rttvar, oldest)
                    {
                        #[cfg(feature = "diag-sched-telemetry")]
                        if self.tail_probe_pending.is_none() {
                            self.sched_diag.tail_probe_enqueued += 1;
                        }
                        self.tail_probe_pending = Some(target_seq);
                    }
                }
            }
        }
    }

    /// DR-CS-3: emit at most one bootstrap `0x19` probe per authoritative ladder step.
    ///
    /// **ADDITIVE, NOT PACED**: this deliberately does NOT call `try_send_pre_parity` / `pacer.try_send`
    /// and creates NO pacer debt — the DR-CS-2 failure proved a debit only reallocates the DATA budget.
    /// It is bounded instead by three independent limits: one generation per step, four steps, and a
    /// total wire budget of `BS_TX_HARD_BUDGET` (200 B; 4 × 44 = 176 B worst case).
    ///
    /// **NO DISPLACEMENT**: called at the very END of `schedule_transmit`, after DATA, recovery and
    /// parity have already been planned in the same turn, so it can only ever *append* — it can never
    /// take a slot a normal DATA emission would have used.
    fn drain_bootstrap_probes(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        if !self.bootstrap_rtt.enabled || self.bs_done {
            return;
        }
        // Gate: PreConverge with no RTT measurement yet (contract §1).
        if self.state != SenderState::PreConverge || self.rtt.srtt().is_some() {
            return;
        }
        let Some(armed) = self.bs_armed_ns else {
            return;
        };
        let step = self.bs_next_step as usize;
        let ladder = self.bootstrap_rtt.ladder();
        if step >= ladder.len() {
            return; // ladder exhausted
        }
        if now_ns.saturating_sub(armed) < ladder[step] {
            return; // not yet due — NOT an edge, and NOT counted
        }
        // EDGE: this step transitions not-due → due exactly once (bs_next_step advances below), so a
        // repeated poll of the same due state can never mint a second generation.
        self.bs_diag.step_edge += 1;
        self.bs_next_step += 1;
        if self.bs_bytes + BS_PROBE_WIRE_EXACT > BS_TX_HARD_BUDGET {
            self.bs_diag.budget_violation += 0; // budget respected by refusing, not by exceeding
            return;
        }
        let probe_seq = self.bs_seq_next;
        self.bs_seq_next = self.bs_seq_next.saturating_add(1);
        // Single outstanding slot: a new probe supersedes the previous unanswered one.
        self.bs_outstanding = Some((probe_seq, now_ns));
        self.bs_bytes += BS_PROBE_WIRE_EXACT;
        self.bs_diag.probe_emitted += 1;
        self.bs_diag.probe_wire_bytes += BS_PROBE_WIRE_EXACT;
        out.push(TransmitDecision {
            kind: TransmitKind::CsProbe,
            seq: u64::from(probe_seq), // separate namespace: NOT a DATA seq, NOT inflight-tracked
            unit_id: 0,
            size: BS_PROBE_WIRE_EXACT,
            send_record: None,
        });
    }

    /// DR-CS-3: ingest an authenticated `0x1A` echo. The ONLY path that may produce a bootstrap RTT
    /// sample, and only from LOCAL monotonic timestamps: `now_ns − local_probe_send_ns`.
    /// Peer-carried `seen_count`/`arrival_gap` are ledger-only and are never used here.
    fn bs_on_echo(&mut self, probe_seq: u16, now_ns: u64) {
        if !self.bootstrap_rtt.enabled {
            return;
        }
        self.bs_diag.echo_ingested += 1;
        if self.bs_done || self.rtt.srtt().is_some() {
            self.bs_diag.echo_match_rejected += 1; // already resolved (covers duplicate-after-success)
            return;
        }
        let Some((seq, send_ns)) = self.bs_outstanding else {
            self.bs_diag.echo_match_rejected += 1; // unsolicited / already consumed
            return;
        };
        if seq != probe_seq {
            self.bs_diag.echo_match_rejected += 1; // wrong generation
            return;
        }
        self.bs_outstanding = None; // SINGLE consumption
        let sample = Duration::from_nanos(now_ns.saturating_sub(send_ns));
        self.bs_diag.echo_match_valid += 1;
        self.bs_diag.rtt_sample_ns = dur_ns(sample);
        // Existing estimator API — no synthetic write, no estimator rewrite.
        self.rtt.observe(now_ns, sample);
        // [FUSION-POLICY] Örnek sayacı + telemetri damgası: güvenlik doğrulayıcısının güven ve
        // tazelik kapıları bunlara bakar (min_rtt_samples / max_telemetry_age_ns).
        self.rtt_sample_count = self.rtt_sample_count.saturating_add(1);
        self.last_telemetry_ns = now_ns;
        // First valid sample cancels the ladder permanently (contract §25/§26).
        self.bs_done = true;
        self.bs_armed_ns = None;
        self.bs_diag.cancelled += 1;
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
                    // [P0 burst-recovery] Forward-recovery kopyası kaybolduysa birim HÂLÂ
                    // teslim edilmemiştir (ACK olsaydı on_ack inflight'tan siler + dup_queue.retain
                    // kopyayı iptal ederdi; buraya ancak gerçekten-outstanding birim düşer). Recovery-
                    // obsesif taban (§9.1 "veri ulaşsın"): SESSİZCE VAZGEÇME — birimin zaten planlı bir
                    // kopyası yoksa GEOMETRİK (self-reschedule, ×2 backoff, ACK-ile-iptal) forward kopya
                    // yeniden-kur. Mevcut saf-sessizlik/PreConverge makinesinin aynısı; INV-RATE-1'e
                    // dokunmaz (dup_queue token-paced), loss METRİĞİNE dokunmaz. Kanıt: kapi-eval-v1
                    // exploratory h3 (GE-burst); regresyon kapısı tests/burst_loss_recovery.rs.
                    if !self.dup_queue.iter().any(|d| d.unit.id == p.unit.id) {
                        let srtt_ns = dur_ns(self.rtt.srtt().unwrap_or(INITIAL_RTT));
                        self.dup_queue.push(DupRec {
                            due_ns: now_ns.saturating_add(srtt_ns),
                            unit: p.unit,
                            interval_ns: srtt_ns.max(1), // >0 ⇒ drain_due_duplicates geometrik reschedule
                        });
                    }
                }
                if p.origin != TransmitKind::Duplicate {
                    // Observation-only loss evidence, ORIGINALS ONLY — symmetric with the success
                    // side in on_ack_received. The enclosing branch admits everything that is not
                    // a Duplicate, which also lets Retransmit and FecParity through, and parity
                    // carries `unit_id: u64::from(p.esi)` (sender.rs, plan_fec_parity): an ESI is a
                    // small integer in a DIFFERENT namespace from source logical ids. Feeding it
                    // would inject foreign ids into a window defined over logical source order and
                    // could fabricate `id`/`id+1` adjacency or collide with a real source id.
                    //
                    // The estimator's contract is the fate of an ORIGINAL transmission, so only
                    // TransmitKind::Data may feed either side. Found by SOURCE-GROUNDING S-0.7,
                    // which asks the question this wiring had not been asked: ingest existing is
                    // not the same as ingest being faithful.
                    if p.origin == TransmitKind::Data {
                        let guard = self
                            .tail_recovery
                            .base_pto_ns(self.rtt.srtt().map(dur_ns), dur_ns(self.rtt.rttvar()));
                        self.fate_estimator
                            .on_loss_evidence(p.unit.id, now_ns, guard);
                    }
                    // [FAZ-2 / KALEM-3 — REPAIR FATE AYRIMI] Kaybolan bir REPAIR shard'ı ARQ
                    // kuyruğuna GİREMEZ. Parity `unit.id = esi` taşır (plan_fec_parity) ve ESI,
                    // kaynak birim-id'lerinden FARKLI bir namespace'tir: `pending_lost`'a girerse
                    // `plan_arq_retransmits` onu birim-id'si `esi` olan bir DATA gibi yeniden
                    // gönderir → ürün runtime'ı `store.get(&esi)` ile YANLIŞ (ilk) kaynak birimi
                    // retransmit eder. Aynı namespace tehlikesi fate-estimator tarafında zaten
                    // tanınmıştı (yukarıdaki `origin == Data` koşulu); ARQ tarafında tanınmamıştı.
                    //
                    // Spec: repair'ler proaktif ve ACK-BAĞIMSIZDIR (fec-core §4/§6; engine §9 ARQ
                    // yalnız orijinal DATA'yı kapsar) — kaybolan repair "eksik kalan redundans"tır,
                    // yeniden iletilecek bir yük değil. Inflight'tan düşürme ve tombstone (geç-ACK
                    // anlaşılsın) KORUNUR; yalnız ARQ kuyruğu atlanır.
                    if p.origin == TransmitKind::FecParity {
                        self.repair_loss_not_arqed = self.repair_loss_not_arqed.saturating_add(1);
                    } else {
                        self.pending_lost.push_back(LostRec {
                            seq,
                            unit: p.unit,
                            deadline_ns: now_ns,
                            retx_count: p.retx_count,
                            send_ns: p.send_ns, // [TUR-2a] un-loss kol-1 RTT için orijinal damga
                        });
                    }
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
                                                                // [ONAY UFKU] Kol seçimi. Legacy = sevk davranışı. Kanal-türevli kolda srtt ÖLÇÜLMÜŞ
                                                                // olmalı: `self.rtt.srtt()` None iken `INITIAL_RTT` bir prior'dır ve prior'dan ufuk
                                                                // türetmek tam da düzeltilmek istenen kusurdur → o durumda legacy'ye düşer.
        let confirm_horizon = match self.loss_confirm_horizon {
            LossConfirmHorizon::CadenceLegacy => self.cadence_loss_floor(),
            LossConfirmHorizon::ChannelDerived { k_permille } => match self.rtt.srtt() {
                Some(measured) => Duration::from_nanos(
                    dur_ns(measured).saturating_mul(u64::from(k_permille)) / 1000,
                ),
                None => self.cadence_loss_floor(),
            },
        };
        self.loss
            .ledger_tick(now_ns, ack_epoch * K_WL, confirm_horizon);
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
        let mut d = self.fuzzy.decide(&inputs);
        // [TUR-4] sıfır-ağırlık fallback kanıt-sayacı (fallback çıktısı R1-baskın çıktıyla
        // bayt-özdeş — sayaçsız gözlemlenemez, workflow-bulgusu).
        if d.no_rule {
            self.diag_fuzzy_no_rule += 1;
        }
        // ── [FEC-RISK v2] SPEKÜLATİF KOL — yalnız ÖNERİ üretir, karar arbiter'ındır ────────────
        //
        // KONTRAT: kanonik `d` YUKARIDA `inputs` (= Confirmed) ile hesaplandı ve `mode` ·
        // `arq_aggressiveness_permille` ile state-machine, pacing, cwnd, retransmit yollarını
        // beslemeye DEVAM EDER. Spekülatif kol fuzzy'den GEÇMEZ (fuzzy'nin ≈200k dizi tam olarak
        // erken sinyali öldüren şeydir); kendi eşlemesi var ve TAVANLIDIR. Risk sinyalinin ürün
        // yolundaki tek etkisi arbiter üzerinden parity MİKTARIDIR.
        //
        // SIRALAMA: split burada hesaplanır (enstrümanlar dahil), ÖNERİ ise politika kararından
        // SONRA üretilir — çünkü kalibre politika açıkken spekülatif kolun TAVANI politikadır
        // (denetim BLOCKER'ı: lane'ler arası max() politikanın "asla daha agresif değil"
        // sözleşmesini bozardı).
        let spec_split = if self.fec_loss_risk.enabled {
            // srtt ölçülmemişse Suspected'a güven verilmez → split kanonikle özdeş.
            let srtt_ns = self.rtt.srtt().map_or(0, dur_ns);
            // [PERSIST] Ufuk: taban α·srtt; Learned modda ölçülen spurious yaşının Q_q'su × ρ.
            let c = self.fec_loss_risk;
            let floor_ns = srtt_ns.saturating_mul(u64::from(c.persist_srtt_permille)) / 1000;
            let persist_min_ns = match c.persist_mode {
                PersistMode::FixedSrtt => floor_ns,
                PersistMode::LearnedSpuriousAge => learned_persist_min_ns(
                    floor_ns,
                    self.loss.spurious_age_quantile(c.persist_q_permille),
                    c.persist_rho_permille,
                    (c.persist_cap_srtt_permille != u32::MAX).then(|| {
                        srtt_ns.saturating_mul(u64::from(c.persist_cap_srtt_permille)) / 1000
                    }),
                ),
            };
            self.diag_persist_min_last_ns = persist_min_ns;
            self.diag_persist_min_peak_ns = self.diag_persist_min_peak_ns.max(persist_min_ns);
            let split = self.loss.fec_loss_risk_split(
                now_ns,
                srtt_ns,
                self.fec_loss_risk.weights,
                persist_min_ns,
            );
            self.diag_risk_ppm_last = split.instant_ppm;
            self.diag_risk_ppm_peak = self.diag_risk_ppm_peak.max(split.instant_ppm);
            self.diag_persistent_ppm_peak = self.diag_persistent_ppm_peak.max(split.persistent_ppm);
            self.diag_reorder_pressure_last = split.reorder_pressure_permille;
            self.diag_reorder_pressure_peak = self
                .diag_reorder_pressure_peak
                .max(split.reorder_pressure_permille);
            if split.instant_ppm > 0 && self.diag_t_first_risk_ns == 0 {
                self.diag_t_first_risk_ns = now_ns;
            }
            Some(split)
        } else {
            None
        };
        // Kanonik (Confirmed-taban) öneri: fuzzy'nin oranı. Politika açıksa aşağıda onunla değişir.
        let mut confirmed: (u32, FecArbiterReason) = (
            d.fec_parity_ratio_permille,
            FecArbiterReason::ConfirmedFuzzy,
        );
        let mut structure: u32 = 0;
        // Politika AÇIKKEN spekülatif kolun üst sınırı (None = politika kapalı).
        let mut policy_ceiling: Option<u32> = None;
        if self.fusion_policy.is_enabled() {
            let obs = crate::fusion_policy::FusionObservation {
                feature_schema: self.fusion_policy.version().feature_schema,
                // Kalibre politika Confirmed-taban bir karar katmanıdır → KANONİK girdiyi okur.
                inputs,
                rtt_sample_count: self.rtt_sample_count,
                telemetry_age_ns: now_ns.saturating_sub(self.last_telemetry_ns),
                converged: self.state != SenderState::PreConverge,
                fec_actuation_available: !matches!(
                    self.fec_mode,
                    FecActuationMode::Manual { enabled: false }
                ),
                // [DEADLINE-EKSENİ] Uçuştaki EN SIKI teslim bütçesi / srtt. App bildirmediyse None.
                deadline_over_srtt_permille: self.tightest_deadline_over_srtt(),
            };
            // Karar-başına payload: kuyruğun önündeki birim (yoksa MTU) — bütçe tavanı buna göre.
            let payload = self.tx_queue.front().map_or(DEFAULT_MTU, |u| u.size);
            self.diag_last_obs = Some((
                obs.inputs.loss_ppm,
                obs.inputs.bandwidth_bps,
                obs.telemetry_age_ns,
                obs.rtt_sample_count,
                obs.deadline_over_srtt_permille,
            ));
            let lp = obs.inputs.loss_ppm;
            self.diag_obs_loss_range = Some(match self.diag_obs_loss_range {
                Some((lo, hi)) => (lo.min(lp), hi.max(lp)),
                None => (lp, lp),
            });
            let bw = obs.inputs.bandwidth_bps;
            self.diag_obs_bw_range = Some(match self.diag_obs_bw_range {
                Some((lo, hi)) => (lo.min(bw), hi.max(bw)),
                None => (bw, bw),
            });
            let action = self.fusion_policy.decide(&obs, payload);
            self.diag_fusion_actions[action.index()] += 1;
            // Politika açıkken Confirmed-taban öneri ONUNDUR (fuzzy'nin değil): yapı-gate susar,
            // çift-otorite yok. Öneri arbiter'a gider; parity'yi tek yer yazar.
            confirmed = match action {
                crate::fusion_policy::FusionAction::FecAssist {
                    redundancy_permille,
                    ..
                } => (redundancy_permille, FecArbiterReason::ConfirmedPolicy),
                _ => (0, FecArbiterReason::ConfirmedPolicy),
            };
            // Spekülatif kolun tavanı: KANIT-YOKLUĞU geri dönüşleri (ColdStart · LowConfidence ·
            // NoDeadlineDeclared) bootstrap'ın tam hedefidir → sert tavana kadar serbest; FecAssist
            // → sert tavan; diğer her geri dönüş GÜVENLİK sebebidir (bölge dışı, bayat telemetri,
            // gevşek deadline = ölçülü israf, …) → spekülatif kol SUSAR.
            use crate::fusion_policy::{FallbackReason as FR, FusionAction as FA};
            policy_ceiling = Some(match action {
                FA::FecAssist { .. }
                | FA::FallbackArq(FR::ColdStart | FR::LowConfidence | FR::NoDeadlineDeclared) => {
                    self.fusion_policy.hard_max_redundancy_permille()
                }
                _ => 0,
            });
        } else {
            // Yapı-gate Confirmed-taban bir kapıdır → KANONİK kaybı okur (loss-yapı sinyali).
            // Yalnız politika KAPALIYKEN koşar (çift-otorite yok). Artık doğrudan parity YAZMAZ:
            // önerir, arbiter karar verir. Kol kapalıyken `max(fuzzy, gate)` eski davranışla
            // özdeştir çünkü gate yalnız fuzzy==0 iken `Opened` döner.
            let outcome = self.structure_gate_fec(d.fec_parity_ratio_permille, inputs.loss_ppm);
            // [TEŞHİS] Karar-yolu histogramı: gate'in HANGİ önkoşulda durduğu sayılır.
            self.diag_gate_outcomes[outcome.index()] += 1;
            if let GateOutcome::Opened(fec) = outcome {
                structure = fec;
                self.diag_structure_gate_opened += 1;
            }
        }
        // Spekülatif ÖNERİ — tavan artık bilinir.
        let spec = match spec_split {
            Some(split) => {
                let p = self.speculative_propose(split, policy_ceiling, confirmed.0.max(structure));
                match p.level {
                    SpeculativeLevel::L0None => self.diag_spec_levels[0] += 1,
                    SpeculativeLevel::L1Bootstrap => {
                        self.diag_spec_levels[1] += 1;
                        if self.diag_t_first_l1_ns == 0 {
                            self.diag_t_first_l1_ns = now_ns;
                            self.diag_first_l1_ctx = (
                                split.sent_window_units,
                                split.instant_ppm,
                                split.persistent_ppm,
                            );
                        }
                    }
                    SpeculativeLevel::L2Persistent => {
                        self.diag_spec_levels[2] += 1;
                        if self.diag_t_first_l2_ns == 0 {
                            self.diag_t_first_l2_ns = now_ns;
                            self.diag_first_l2_ctx = (
                                split.sent_window_units,
                                split.instant_ppm,
                                split.persistent_ppm,
                            );
                        }
                    }
                }
                p
            }
            None => SpeculativeProposal::NONE,
        };
        // Ledger kaybının koşu-boyu TEPESİ — artık HER iki dalda yazılır (eskiden politika açıkken
        // yapısal sıfırdı → vakum sayaç; CORRIGENDUM 6fa81b4e).
        self.diag_loss_ppm_peak = self.diag_loss_ppm_peak.max(inputs.loss_ppm);
        // ── TEK FEC ARBİTER ───────────────────────────────────────────────────────────────
        // final = max(confirmed, structure, speculative). Toplama YOK. Parity'yi yazan tek yer.
        let (final_ratio, reason) = fec_arbiter(confirmed, structure, spec);
        // [KURULUŞ-GUARD — ARBİTER KAPSAMI] (transport-v1 denetimi, 2026-09-08)
        // Guard `structure_gate_fec` içinde vardı (`BlockedPreConverge`, :3664) ama YALNIZ yapı-gate
        // kolunu kapsıyordu; spekülatif kol (`speculative_propose`) hiçbir `SenderState` kontrolü
        // içermiyor. FEC ürün profilinde default AÇIK olduğundan (client-core `KamaV1Profile`) bu
        // artık canlı bir yol: Karn-dışı bir ACK (retransmit belirsizliği) RTT örneği vermeden
        // SACK boşluklarıyla kayıp ilan edebilir → risk > l1_min → PreConverge'de parity önerilir.
        // Guard'ın belgelenmiş ilkesi kola değil TELE ÇIKAN PARITY'ye aittir (:3655-3659: "FEC ölçüm
        // olmadan aksiyon almaz"; PreConverge'de parity kıt token bütçesinde ORİJİNALLERLE yarışır ve
        // kuruluşu bozar). Bu yüzden guard arbiter ÇIKIŞINA uygulanır — üç kolu birden kapsar.
        // [FAZ-2 / KALEM-2 — §8.5 BÜTÇE CLAMP'İ] `parity_final = min(fuzzy_parity, budget_cap,
        // profile_cap)` (engine-spec §658-661, NORMATİF; TDD §5.5). `budget_cap = f(BtlBw)` ZATEN
        // uygulanmıştı (`budget::budget_cap_permille`) ama HİÇBİR ÜRÜN ÇAĞIRANI YOKTU — yalnız
        // tv_emergency testinden çağrılıyordu (denetim: WIRE). Dar bantta yüksek kayıp parity'yi
        // PATLATAMAZ: 1 mbit'te cap 900‰ (bağlamaz), 64 kbit'te 64‰ (sert bağlar).
        //
        // TEK-ARBİTER İNVARYANTI KORUNUR: bu ikinci bir FEC otoritesi DEĞİL, arbiter ÇIKIŞINA
        // uygulanan bir üst-sınırdır (yalnız küçültür, asla büyütmez). Kalibrasyon YAPILMADI —
        // `BudgetConfig::fixture()` mevcut [CAL] değerleriyle bağlandı.
        let final_ratio = {
            let cfg = crate::budget::BudgetConfig::fixture();
            let btlbw = self.btlbw.btlbw();
            // KANIT YOKKEN CLAMP YOK: `btlbw()` örnek yokken 0 döner; 0'ı "bütçe yok" diye okumak
            // FEC'i kuruluş fazında SESSİZCE kapatırdı. Ölçüm yokken üst-sınır profil tavanıdır
            // (spec §8.5 cap'i BtlBw'nin FONKSİYONUDUR; BtlBw yoksa fonksiyon tanımsızdır).
            let cap = if btlbw == 0 {
                cfg.profile_cap_permille
            } else {
                crate::budget::budget_cap_permille(btlbw, &cfg)
            };
            let clamped = crate::budget::parity_final(final_ratio, cap, cfg.profile_cap_permille);
            // ── SPEC-İÇİ ÇELİŞKİ (kayıtlı, sessizce çözülmedi) ──
            // §8.5 formülü `cap = BtlBw×1000/budget_ref` dar bantta 0'a iner: `budget_ref =
            // 125 kB/s` fixture'ıyla BtlBw < 125 B/s ⇒ cap 0‰. Ama §11 (NORMATİF) TAM O KÖŞEDE
            // (sub-kbps, emergency) "minimal parity shard" ister ve ürünün MIN_RATE tabanı
            // 31 B/s'tir. Yani formülün literal okuması, transport'un birincil hedef rejiminde
            // FEC'i tamamen kapatır — §11 ile çelişir.
            //
            // KARAR (kalibrasyon DEĞİL): clamp yalnız KÜÇÜLTTÜĞÜNDE uygulanır; parity'yi SIFIRA
            // indirecekse UYGULANMAZ ve ayrı sayaçla görünür kılınır. Böylece §8.5'in koruyucu
            // etkisi (dar bantta parity patlamasın) kazanılır, §11'in tabanı yok edilmez.
            // Sıfırlama vakasının politikası owner kararıdır (kapanış raporu, açık kalem).
            if clamped == 0 && final_ratio > 0 {
                self.diag_parity_budget_would_zero =
                    self.diag_parity_budget_would_zero.saturating_add(1);
                final_ratio
            } else {
                if clamped < final_ratio {
                    self.diag_parity_budget_clamped =
                        self.diag_parity_budget_clamped.saturating_add(1);
                }
                clamped
            }
        };
        let (final_ratio, reason) = if self.state == SenderState::PreConverge && final_ratio > 0 {
            self.diag_preconverge_parity_blocked =
                self.diag_preconverge_parity_blocked.saturating_add(1);
            (0, FecArbiterReason::None)
        } else {
            (final_ratio, reason)
        };
        self.diag_arbiter_reasons[reason.index()] += 1;
        // [HANDOFF ENSTRÜMANI] Kararın anlık görüntüsü — "canonical geldiğinde parity düştü mü"
        // sorusu bu seri olmadan sorulamaz. Salt gözlem, sabit boyut.
        self.diag_last_ts = TsSample {
            instant_ppm: spec_split.map_or(0, |x| x.instant_ppm),
            persistent_ppm: spec_split.map_or(0, |x| x.persistent_ppm),
            reorder_pressure_permille: spec_split.map_or(0, |x| x.reorder_pressure_permille),
            confirmed_ppm: inputs.loss_ppm,
            confirmed_prop: confirmed.0,
            structure_prop: structure,
            spec_prop: spec.ratio_permille,
            final_ratio,
            reason_idx: u8::try_from(reason.index()).unwrap_or(u8::MAX),
            level_idx: match spec.level {
                SpeculativeLevel::L0None => 0,
                SpeculativeLevel::L1Bootstrap => 1,
                SpeculativeLevel::L2Persistent => 2,
            },
            evidence_mass: spec_split.map_or(0, |x| x.sent_window_units),
            declared_units: spec_split.map_or(0, |x| x.declared_units),
            pacer_tokens: self.pacer.tokens(),
            pending_parity: self.pending_fec.len() as u64,
            pending_lost: self.pending_lost.len() as u64,
            sched_data_denied: self.diag_sched_summary()[1],
            sched_retx_denied: self.diag_sched_summary()[4],
            sched_parity_denied: self.diag_sched_summary()[9],
            sched_retx_emitted: self.diag_sched_summary()[5],
            sched_parity_emitted: self.diag_sched_summary()[10],
            sched_data_emitted: self.diag_sched_summary()[2],
        };
        d.fec_parity_ratio_permille = final_ratio;
        if final_ratio > 0 {
            // Bir kol parity önerdiyse bir KURAL ateşlemiştir → latch'in no_rule bastırması kalkar.
            // Eski davranışla özdeş: fuzzy>0 ⟹ no_rule zaten false (fec_zero=0); gate/politika
            // açtığında eskiden de false yapılıyordu.
            d.no_rule = false;
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

    /// [Q18-FIX] Yapı-farkında FEC gate. Yalnız fuzzy'nin `fec=0` bıraktığı ama kaybın anlamlı
    /// olduğu Q18 bandında devreye girer; estimator IID-yapı gösteriyorsa (`burst_delta ≤ 0`:
    /// kayıplar korelasyonsuz → retransmit de aynı oranla kaybolur → ileri-parity kazanır)
    /// `fec_med` açar. Burst-yapı (`burst_delta` pozitif) → ARQ'ya bırakır (`None`, dokunmaz).
    /// Kanıt-eşiği: pencerede `≥ STRUCTURE_GATE_MIN_N` fate-known birim; azsa `None`.
    fn structure_gate_fec(&self, fuzzy_fec_permille: u32, loss_ppm: u64) -> GateOutcome {
        if fuzzy_fec_permille != 0 {
            return GateOutcome::NotApplicable;
        }
        // [KURULUŞ-GUARD] PreConverge'de HİÇBİR yapısal FEC kararı verilmez.
        //
        // İlke: FEC ölçüm olmadan aksiyon almaz. PreConverge oturumun kurulmadığı, dolayısıyla
        // RTT/BtlBw örneği OLMADIĞI ve kayıp-ledger'ının güvenilmez (bkz. loopback bulgusu)
        // olduğu fazdır — orada parity açmak ölçüme değil önyargıya dayanır ("sürekli açık FEC
        // bant israfıdır" ilkesinin ihlali). Somut zarar ölçüldü: burst-fusion politikası
        // PreConverge'de parity açıyordu, parity kıt token bütçesinde ORİJİNALLERLE yarışıyor ve
        // ilk-uçuş orijinal sayısını düşürerek kuruluş olasılığını kırıyordu (kuruluş yalnız
        // orijinal DATA'nın RTT örneğiyle olur — cold-start-wiring bulgusu). Sonuç: burst-fusion
        // kolu netem'de EN KÖTÜ koldu (v2 5/12 çöküş). Kuruluş bittikten sonra (Startup+) gate
        // gerçek ölçümle çalışır; guard yalnız kurulmamış oturumu korur.
        if self.state == SenderState::PreConverge {
            return GateOutcome::BlockedPreConverge;
        }
        if loss_ppm < STRUCTURE_GATE_LOSS_MIN_PPM {
            return GateOutcome::BlockedLossBelowBand;
        }
        let f = self.fate_estimator.snapshot();
        if f.n < STRUCTURE_GATE_MIN_N {
            return GateOutcome::BlockedWindowTooSmall;
        }
        // [BULGU — prior-as-evidence; AÇIK KAYIT, default DAVRANIŞ DEĞİŞMEDİ]
        //
        // `burst_delta` Laplace-yumuşatılmıştır ve kayıp-GEÇİŞİ sayaçları boşken tamamen
        // PRIOR'dan gelir:
        //   • hiç geçiş yok (n00=n01=n10=n11=0) → p11 = p01 = (0+1)/(0+2) = 500_000 → delta = 0
        //     → "IID" okunur ve FEC AÇILIR;
        //   • yalnız kayıpsız geçişler → p11 = 500_000, p01 ≈ 0 → delta ≈ +475_000
        //     → "burst" okunur ve FEC KAPANIR.
        // Yani her iki dal da kanıt yokken prior tarafından belirlenir. Estimator'ın kendi §3.4
        // sözleşmesi bunu zaten yasaklıyor (`WindowFeatures::eligible` → `n10+n11 ≥ 8`;
        // `fate_estimator.rs::laplace_no_loss_window_ineligible` testinin yorumu birebir bunu
        // söyler) — gate o sözleşmenin DIŞINDA okuyor.
        //
        // Ölçülen kanıt: `tests/structure_gate_fec.rs` içindeki sevk pinlerinin İKİSİ de
        // (`iid_loss_opens_fec_*`, `burst_loss_leaves_fec_to_arq`) sıfır-geçişli pencerelerde
        // koşuyor — sonuçları doğru ama gerekçeleri prior. Kanıt eşiği default yola eklendiğinde
        // `iid_loss_opens_fec_*` RED veriyor, yani Q18-FIX'in IID dalı bugün prior'la ateşliyor.
        //
        // KARAR: default (`Q18Iid`) yol DEĞİŞTİRİLMEDİ — kampanya görmüş bir davranışı tek
        // taraflı kaydırmak R1 disiplinine aykırı ve sonuçları ölçülmemiş olurdu. Kanıt eşiği
        // opt-in `BurstFusion` politikasının parçasıdır; o politika zaten kalibrasyon bekliyor.
        // Bu bir AÇIK BORÇTUR (owner kararı gerekir), gizlenmiş bir kusur değil.
        let evidenced = f.n10 + f.n11 >= crate::fate_estimator::MIN_LOSS_TRANSITIONS;
        let iid_structure = f.burst_delta_ppm <= STRUCTURE_GATE_IID_MAX_DELTA_PPM;
        match self.fec_structure_policy {
            FecStructurePolicy::Q18Iid => {
                // SEVK DAVRANIŞI, byte-özdeş: IID → fec_med; burst → ARQ'ya bırak.
                if iid_structure {
                    GateOutcome::Opened(self.gate_level_permille())
                } else {
                    GateOutcome::BlockedBurstLeftToArq
                }
            }
            FecStructurePolicy::BurstFusion => {
                // Kanıtsız pencerede yapısal karar YOK (prior veri sayılmaz).
                if !evidenced {
                    return GateOutcome::BlockedNoTransitionEvidence;
                }
                let c = self.fuzzy.config();
                if iid_structure {
                    return GateOutcome::Opened(self.gate_level_permille());
                }
                // BURST dalı: kodlama açılır, seviye ölçülen silme oranını emniyet paylı takip
                // eder. Profil-bağımsız (INV-ENGINE-SOC: motor K/E bilmez) — oran permille
                // emit edilir, r'ye çevrimi latch profille birlikte yapar. Kodlama tek başına
                // yetmese bile parity SIFIRLANMAZ: kalanı ARQ taşır (§8.4 füzyon).
                let needed_permille = loss_ppm.saturating_mul(BURST_FUSION_MARGIN_NUM)
                    / BURST_FUSION_MARGIN_DEN
                    / 1_000;
                let lvl = self.gate_level_permille();
                GateOutcome::Opened(if needed_permille <= u64::from(lvl) {
                    lvl
                } else {
                    c.fec_high
                })
            }
        }
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
        self.last_recovery =
            emergency::action(level, self.emergency_framing(), &self.emergency_cfg);

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
        // Commit-V (U12-atomik-plan-adım-1..5): pre-parity-drain'lerden-ÖNCE exact-emittable-kuantum-
        // rezervasyonu (OFF'ta-derlenmez; LeftoverOnly'de-no-op → karar-dizisi-baseline-özdeş).
        #[cfg(feature = "experimental-fec-budget")]
        self.budget_reserve(now_ns);
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
        // DR-ARQ-PRODUCT-TAIL-1 (gated; default OFF → no-op): forced PTO tail probe, state-independent, after the
        // normal ARQ planner (so it never double-plans a seq the planner already superseded).
        self.emit_tail_probe(&mut out, now_ns);
        // [DR-EST-1] PreConverge kuruluş-denemesi: §3.3-B kopya-makinesi ilk-ölçüm-öncesi
        // PRE_CONVERGE_FLOOR_BW pacer'ına takılıp boğuluyordu (token-yok → break). Bu kol
        // vadesi gelen EN-ESKİ kopyadan TEK datagram'ı token-MUAF geçirir; INV-RATE-1'e
        // dokunmaz (send_rate/pacer-rate değişmez — ayrı bounded yan-kol), teyitle söner
        // (PreConverge-çıkışı). EK4-A2: vadesi next_timer_deadline_ns kümesine DAHİL.
        self.drain_establish_probe(&mut out, now_ns);
        self.drain_due_duplicates(&mut out, now_ns);
        // DR-S5-1/Commit-K: FEC-parity DATA/recovery SONRASI (DATA-ÖNCE-sıralaması mühürlü). Drain'de
        // mode-kapısı-YOK (üyelik=izin; girişte-kapılı) — buraya-drain-guard-EKLEME: Commit-K'nın
        // kaldırdığı suppression-bug'ını-geri-getirir.
        self.plan_fec_parity(&mut out, now_ns);
        // Commit-V (U12-adım-8): gerçek-harcama-settle + waste-denetimi (normal-yolda-waste=0).
        #[cfg(feature = "experimental-fec-budget")]
        self.budget_settle();
        // DR-CS-1 M2 (F2A): CS-probe pass'i EN-SONDA (F0-tespit-(vii) ekleme-noktası) — mühürlü
        // DATA-önce-sıralama + FEC-budget-penceresi dokunulmaz; flag-OFF'ta derlenmez.
        // DR-CS-3 SUPERSESSION: the legacy experimental ladder and the shipping bootstrap mechanism
        // are two implementations of the SAME 0x19/0x1A protocol. When the shipping mechanism is
        // enabled it is authoritative and the legacy pass is skipped, so a feature-enabled build can
        // never run both ladders and emit duplicate generations.
        #[cfg(feature = "experimental-cold-start")]
        if !self.bootstrap_rtt.enabled {
            self.drain_cs_probes(&mut out);
        }
        // DR-CS-3: bootstrap probe LAST — DATA, recovery and parity for this turn are already planned,
        // so this can only append. It never occupies a slot a DATA emission would have used.
        self.drain_bootstrap_probes(&mut out, now_ns);
        out
    }

    /// FEC-emisyonu aç/kapa (DR-S5-1 guard). Engine FEC-etkinken açar; kapalı=mühürlü-hot-path.
    /// Commit-G/K: otorite-mode-ayarı. Geçiş BLOCK-BOUNDARY-etkin ve otorite-bağlı-yolda-YAPISAL:
    /// planlama witness'tan (Commit-I) + kuyruklu-parity üyelik=izin (Commit-K) → aktif-mode-değişimi
    /// ne-açık-bloğun-planını ne-kuyruklu-parity'sinin-emisyonunu-etkiler; yalnız-SONRAKİ-open'ı-etkiler.
    /// Immediate-global-emission-kill BU-API-DEĞİL (bilinçli; §7 safety-disable AYRI-DR-gerektirir).
    /// Şimdilik-hatasız (Result-imza ileri-uyum).
    pub fn set_fec_actuation_mode(
        &mut self,
        mode: FecActuationMode,
    ) -> Result<(), FecAuthorityError> {
        self.fec_mode = mode;
        self.last_transition_event_index = self.fusion_event_index;
        self.transition_applied_count = self.transition_applied_count.saturating_add(1);
        Ok(())
    }

    /// [DEADLINE-EKSENİ] Uygulamanın teslim bütçesini bildirir (mesaj-başı; en sıkı olan tutulur).
    ///
    /// Ölçüm (FEC-MARGINAL-R v1+v2, 480 rep) proaktif redundancy'nin değerinin KAYIP ORANINA
    /// DEĞİL bu bütçenin RTT'ye oranına bağlı olduğunu gösterdi. Transport bu bilgiyi kendi
    /// başına TÜRETEMEZ — aynı kanalda aynı kayıpta, sıkı bütçeli trafik için parity kazanç,
    /// gevşek bütçeli trafik için israftır. Bildirilmezse politika `NoDeadlineDeclared` ile
    /// ArqOnly'ye düşer; sessizce kayıp oranından karar VERMEZ.
    pub fn declare_delivery_deadline(&mut self, deadline_ns: u64) {
        self.tightest_deadline_ns = Some(match self.tightest_deadline_ns {
            Some(cur) => cur.min(deadline_ns),
            None => deadline_ns,
        });
    }

    /// [DEADLINE-EKSENİ] Bildirilen en sıkı bütçenin srtt'ye oranı (per-mille). srtt yoksa
    /// (kuruluş öncesi) `None` — zaten `ColdStart` kapısı orada devreye girer.
    fn tightest_deadline_over_srtt(&self) -> Option<u32> {
        let d = self.tightest_deadline_ns?;
        let srtt = dur_ns(self.rtt.srtt()?);
        if srtt == 0 {
            return None;
        }
        Some(u32::try_from(d.saturating_mul(1000) / srtt).unwrap_or(u32::MAX))
    }

    /// [TEŞHİS] Politikaya giden son gözlem: (loss_ppm, bandwidth_bps, telemetry_age_ns,
    /// rtt_sample_count, deadline_over_srtt_permille).
    #[must_use]
    pub fn diag_last_fusion_obs(&self) -> Option<(u64, u64, u64, u32, Option<u32>)> {
        self.diag_last_obs
    }

    /// [TEŞHİS] Politikanın gördüğü `bandwidth_bps` aralığı (min, max).
    #[must_use]
    pub fn diag_fusion_bw_range(&self) -> Option<(u64, u64)> {
        self.diag_obs_bw_range
    }

    /// [TEŞHİS] Politikanın gördüğü `loss_ppm` aralığı (min, max).
    #[must_use]
    pub fn diag_fusion_loss_range(&self) -> Option<(u64, u64)> {
        self.diag_obs_loss_range
    }

    /// [TEŞHİS] Kayıp-defteri iç durumu — bkz. `ThresholdLossDetector::diag_ledger_internals`.
    #[must_use]
    pub fn diag_ledger_internals(&self) -> (u64, u64, u64, u64, u64) {
        self.loss.diag_ledger_internals()
    }

    /// [TEŞHİS] Confirm-yolu — bkz. `ThresholdLossDetector::diag_confirm_path`.
    #[must_use]
    pub fn diag_confirm_path(&self) -> (u64, u64, u64, u64) {
        self.loss.diag_confirm_path()
    }

    /// [TEŞHİS] Ledger tick kapsamı — bkz. `ThresholdLossDetector::diag_tick_span`.
    #[must_use]
    pub fn diag_ledger_tick_span(&self) -> (u64, u64) {
        self.loss.diag_tick_span()
    }

    /// Bildirilen en sıkı teslim bütçesi (gözlem/test).
    #[must_use]
    pub fn diag_tightest_deadline_ns(&self) -> Option<u64> {
        self.tightest_deadline_ns
    }

    /// [FUSION-POLICY] Kalibre edilmiş füzyon politikasını kurar. Default `disabled()` = her
    /// girdide `ArqOnly` → ürün yolu byte-özdeş. `calibration_v1()` OPT-IN ve VERSİYONLU;
    /// güvenlik sınırları kalibrasyondan BAĞIMSIZDIR — bir kalibrasyon yalnız DAHA AZ
    /// isteyebilir, daha fazlasını asla.
    pub fn set_fusion_policy(&mut self, policy: crate::fusion_policy::FusionPolicy) {
        self.fusion_policy = policy;
    }

    /// [FUSION-POLICY/TEŞHİS] Füzyon aksiyon histogramı.
    #[must_use]
    pub fn diag_fusion_actions(&self) -> [u64; crate::fusion_policy::FUSION_ACTION_KINDS] {
        self.diag_fusion_actions
    }

    /// [FUSION-POLICY] Alınan RTT örneği sayısı (gözlem/test).
    #[must_use]
    pub fn diag_rtt_sample_count(&self) -> u32 {
        self.rtt_sample_count
    }

    /// [IW] PreConverge ilk-uçuş penceresini kurar. `bytes == 0` = kapalı (sevk davranışı).
    /// Bkz. [`InitialWindowConfig`]; KALİBRE EDİLMEMİŞ.
    pub fn set_initial_window(&mut self, cfg: InitialWindowConfig) {
        self.initial_window = cfg;
    }

    /// Kurulu ilk-uçuş penceresi (gözlem/test).
    #[must_use]
    pub fn initial_window(&self) -> InitialWindowConfig {
        self.initial_window
    }

    /// [BURST-FUSION] Yapı-gate'inin burst-dalı politikasını seçer. Default [`FecStructurePolicy::Q18Iid`]
    /// = sevk davranışı (burst → parity yok). Bkz. [`FecStructurePolicy`]; flip KALİBRASYON ister.
    /// [FEC-RISK] Deneysel FEC kayıp-riski kolunu kurar. Default KAPALI.
    ///
    /// AÇIKKEN sinyalin TEK tüketicisi FEC oranı kararıdır: `mode`, `arq_aggressiveness`,
    /// pacing, cwnd, state-machine geçişleri ve retransmit tetikleme KANONİK `Confirmed`
    /// oranını okumaya devam eder (bkz. `fuzzy_evaluate`'teki çift-karar ayrımı).
    pub fn set_fec_loss_risk(&mut self, cfg: FecLossRiskConfig) {
        self.fec_loss_risk = cfg;
    }

    /// [FEC-RISK] Yürürlükteki kol (gözlem/test).
    #[must_use]
    pub fn fec_loss_risk_config(&self) -> FecLossRiskConfig {
        self.fec_loss_risk
    }

    /// [FEC-RISK ENSTRÜMAN] `(t_first_suspected, t_first_confirmed, t_first_risk, t_first_parity)`
    /// ns cinsinden; 0 = hiç olmadı. Kolun tüm amacı bu dört damganın arasındaki farktır.
    #[must_use]
    pub fn diag_fec_risk_times(&self) -> (u64, u64, u64, u64) {
        let (susp, conf) = self.loss.diag_first_loss_times();
        (
            susp,
            conf,
            self.diag_t_first_risk_ns,
            self.diag_t_first_parity_ns,
        )
    }

    /// [FEC-RISK ENSTRÜMAN] `(son_risk_ppm, tepe_risk_ppm)`.
    #[must_use]
    pub fn diag_risk_ppm(&self) -> (u64, u64) {
        (self.diag_risk_ppm_last, self.diag_risk_ppm_peak)
    }

    /// [FEC-RISK v2 ENSTRÜMAN] Arbiter sebep histogramı (`FecArbiterReason::index`).
    #[must_use]
    pub fn diag_arbiter_reasons(&self) -> [u64; ARBITER_REASON_KINDS] {
        self.diag_arbiter_reasons
    }

    /// [FEC-RISK v2 ENSTRÜMAN] `(L0, L1, L2)` öneri sayıları.
    #[must_use]
    pub fn diag_spec_levels(&self) -> [u64; 3] {
        self.diag_spec_levels
    }

    /// [PERSIST ENSTRÜMANI] `(ufuk_son_ns, ufuk_tepe_ns, spurious_örnek_sayısı)`.
    #[must_use]
    pub fn diag_persist(&self) -> (u64, u64, usize) {
        (
            self.diag_persist_min_last_ns,
            self.diag_persist_min_peak_ns,
            self.loss.spurious_age_samples(),
        )
    }

    /// [HANDOFF ENSTRÜMANI] Son `fuzzy_evaluate` kararının anlık görüntüsü.
    #[must_use]
    pub fn diag_ts_sample(&self) -> TsSample {
        self.diag_last_ts
    }

    /// [FEC-RISK v2 ENSTRÜMAN] `(persistent_ppm_peak, reorder_pressure_last, reorder_pressure_peak,
    /// t_first_l1_ns, t_first_l2_ns)`.
    #[must_use]
    pub fn diag_spec_signals(&self) -> (u64, u32, u32, u64, u64) {
        (
            self.diag_persistent_ppm_peak,
            self.diag_reorder_pressure_last,
            self.diag_reorder_pressure_peak,
            self.diag_t_first_l1_ns,
            self.diag_t_first_l2_ns,
        )
    }

    /// [FEC-RISK v2] Spekülatif kolun ÖNERİSİ. Belirsizliği bekleyerek çözmez; belirsizlik
    /// arttıkça eylemin BÜYÜKLÜĞÜNÜ sınırlar.
    ///
    /// ```text
    /// L0  instant < l1_min                      → 0
    /// L1  instant ≥ l1_min, persistent < l2_min → bootstrap_cap   (t_risk == t_suspected KORUNUR)
    /// L2  persistent ≥ l2_min                   → persistent_cap
    /// deadline/srtt ≤ tight_budget              → L1 de persistent_cap (DEADLINE_ASSISTED)
    /// kanonik pencerede görünür (windowed>0)   → tavan bootstrap_cap'e iner (steady-state devri)
    /// reorder_pressure (yeterli kanıtla)        → tavan × (1 − rp × discount_max)   [asla 0'a değil]
    /// ```
    /// `policy_ceiling`: kalibre politika AÇIKKEN spekülatif kola izin verilen ÜST SINIR —
    /// `Some(0)` = politika güvenlik sebebiyle ArqOnly dedi, spekülatif kol SUSAR;
    /// `Some(hard_max)` = politika kanıt-yokluğu (ColdStart/LowConfidence/NoDeadline) bildirdi
    /// ya da FecAssist verdi → spekülatif kol en çok sert tavana kadar; `None` = politika kapalı.
    /// Bu, denetimin bulduğu "max() kalibre politikanın 'asla daha agresif değil' sözleşmesini
    /// bozar" kusurunun kapanışıdır: lane'ler arası kompozisyon max değil, politika = tavan.
    /// `canonical_prop`: kanonik lane'in (fuzzy/politika + yapı-gate) fiili ÖNERİSİ (‰). Devir
    /// teslim ancak devralan biri VARSA olur: kanonik lane bir şey öneriyorsa spekülatif kol
    /// bootstrap tavanına iner; önermiyorsa kanıt neyi destekliyorsa o kalır.
    fn speculative_propose(
        &self,
        split: crate::loss::RiskSplit,
        policy_ceiling: Option<u32>,
        canonical_prop: u32,
    ) -> SpeculativeProposal {
        let c = self.fec_loss_risk;
        if policy_ceiling == Some(0) || split.instant_ppm < c.l1_min_instant_ppm {
            return SpeculativeProposal::NONE;
        }
        // [C4 G1] Reorder-belirsiz rejim: son olayların ≥R‰'i Spurious çıktıysa spekülatif parity'nin
        // ölçülü faydası ≈0 (alıcıda blok zaten kapanmış oluyor) → öneri yok. Kapalıyken (u32::MAX) inert.
        if split.declared_units >= c.reorder_min_evidence_units
            && split.reorder_pressure_permille >= c.reorder_suppress_permille
        {
            return SpeculativeProposal::NONE;
        }
        let level = if split.persistent_ppm >= c.l2_min_persistent_ppm {
            SpeculativeLevel::L2Persistent
        } else {
            SpeculativeLevel::L1Bootstrap
        };
        // Bütçe sıkıysa ARQ'nun tekrar-deneme fırsatı yok → belirsiz kanıta daha büyük tepki
        // meşru. Bildirilmemiş bütçe = sıkı DEĞİL (varsayılan uydurulmaz).
        let tight = self
            .tightest_deadline_over_srtt()
            .is_some_and(|b| b <= c.tight_budget_permille);
        let mut cap = match level {
            SpeculativeLevel::L2Persistent => c.persistent_cap_permille,
            SpeculativeLevel::L1Bootstrap if tight => c.persistent_cap_permille,
            _ => c.bootstrap_cap_permille,
        };
        // Steady-state devri — HANDOFF AİLESİ ÖLÇÜMÜYLE DÜZELTİLDİ (reports/…/handoff/):
        // eski kural "pencerede bir Confirmed var → L1'e in" kanıtın değişmesine değil bir onayın
        // VAR OLMASINA tepki veriyordu; kanonik lane bu bant rejiminde hiçbir şey önermezken
        // (fuzzy 0, gate ≥100k'a ulaşmıyor) spekülatif kol yine de düşüyor → t_conf'ta r=2→r=1
        // uçurumu, teslim ≤2×RTT −10…−21 pp (n=3, 400/1000 msg, %10-20). Doğru kural: otorite
        // ancak DEVRALAN biri varsa devredilir — kanonik lane fiilen öneriyorsa bootstrap tavanı,
        // aksi hâlde persistent kanıt neyi destekliyorsa o. `max()` kanonik öneri büyükse zaten
        // onu seçer; klamp yalnız "kanonik küçük ama var" durumunda frenler.
        if canonical_prop > 0 {
            cap = cap.min(c.bootstrap_cap_permille);
        }
        // Reorder indirimi — yalnız yeterli kanıtla, ve ASLA veto değil.
        if split.declared_units >= c.reorder_min_evidence_units {
            let disc = u64::from(split.reorder_pressure_permille)
                * u64::from(c.reorder_discount_max_permille)
                / 1000; // 0..=discount_max
            cap = u32::try_from(u64::from(cap) * (1000 - disc.min(999)) / 1000).unwrap_or(cap);
        }
        // İndirim VETO olamaz: yuvarlama tabanının altına inmez (K=8'de ≥1 repair).
        cap = cap.max(c.spec_min_permille);
        if let Some(ceil) = policy_ceiling {
            cap = cap.min(ceil);
        }
        SpeculativeProposal {
            level,
            ratio_permille: cap,
            deadline_assisted: tight && level == SpeculativeLevel::L1Bootstrap,
        }
    }

    /// [C4-ATIF] Zamanlayıcı sınıf sayaçlarının sabit-boyutlu özeti (feature kapalıyken sıfır):
    /// `[data_sel, data_denied, data_emit, retx_sel, retx_denied, retx_emit, dup_sel, dup_denied,
    /// dup_emit, parity_denied, parity_emit, tail_probe_emit]`. Salt gözlem.
    #[must_use]
    pub fn diag_sched_summary(&self) -> [u64; 12] {
        #[cfg(feature = "diag-sched-telemetry")]
        {
            let d = self.sched_diag;
            [
                d.data_selected,
                d.data_token_denied,
                d.data_wire_emitted,
                d.arq_retransmit_selected,
                d.arq_retransmit_token_denied,
                d.arq_retransmit_wire_emitted,
                d.dup_selected,
                d.dup_token_denied,
                d.dup_wire_emitted,
                d.fec_parity_token_denied,
                d.fec_parity_wire_emitted,
                d.tail_probe_wire_emitted,
            ]
        }
        #[cfg(not(feature = "diag-sched-telemetry"))]
        {
            [0; 12]
        }
    }

    /// [FAZ-2 KALEM-4] Acil-durum kurtarma eyleminin framing'i. Spec §11 / TDD §5.6 iki kol tanımlar:
    /// `Tiny` (tek datagram) → zaman-aralıklı D× KOPYA; `SegmentBlock` (çok-shard'lı blok framing)
    /// → **minimal parity shard** (düşük r/k, ACK-bağımsız). Çağıran `Framing::Tiny`'yi SABİT
    /// veriyordu; FEC ürün profilinde açık olduğundan blok framing MEVCUTTUR ve spec'in
    /// segment-block kolu ulaşılamaz kalıyordu (denetim: `Framing::SegmentBlock` ve
    /// `RecoveryAction::MinimalParity` "hiçbir yerden çağrılmıyor").
    fn emergency_framing(&self) -> Framing {
        match self.fec_mode {
            FecActuationMode::Controller => Framing::SegmentBlock,
            FecActuationMode::Manual { enabled } => {
                if enabled {
                    Framing::SegmentBlock
                } else {
                    Framing::Tiny
                }
            }
        }
    }

    /// [FAZ-2 KALEM-2] §8.5 bant-türevli bütçe clamp'inin parity oranını KÜÇÜLTTÜĞÜ karar sayısı.
    #[must_use]
    pub fn diag_parity_budget_clamped(&self) -> u64 {
        self.diag_parity_budget_clamped
    }

    /// [FAZ-2 KALEM-2] §8.5 clamp'inin parity'yi SIFIRLAYACAĞI (ve bu yüzden uygulanmadığı) karar
    /// sayısı — §11 ile çelişen köşenin ölçüsü. `> 0` = owner kararı bekleyen rejimdeyiz.
    #[must_use]
    pub fn diag_parity_budget_would_zero(&self) -> u64 {
        self.diag_parity_budget_would_zero
    }

    /// [FAZ-2 KALEM-3] Kaybolduğu ilan edilip ARQ kuyruğuna SOKULMAYAN repair shard sayısı.
    /// `> 0` = repair/DATA fate ayrımı fiilen çalıştı (salt gözlem).
    #[must_use]
    pub fn diag_repair_loss_not_arqed(&self) -> u64 {
        self.repair_loss_not_arqed
    }

    /// [FAZ-1 DÖNÜŞ-YOLU] Bir ACK datagram'ı gözlendi. **Motor davranışını DEĞİŞTİRMEZ** —
    /// yalnız `diag_return_path()` çıktısını besler. Ürün runtime'ı `Event::AckReceived`'dan
    /// hemen ÖNCE çağırır; çağrılmaması motoru etkilemez (gözlem boş kalır).
    pub fn observe_ack_feedback(&mut self, obs: crate::return_path::AckObservation, now_ns: u64) {
        self.return_path.on_ack(obs, now_ns);
    }

    /// [FAZ-1 DÖNÜŞ-YOLU] Ölçülen dönüş-yolu durumu (ileri-yön kaybı ile dönüş bozulmasının
    /// AYRIMI dâhil). `now_ns` sessizliği türetmek için gerekir.
    #[must_use]
    pub fn diag_return_path(&self, now_ns: u64) -> crate::return_path::ReturnPathState {
        self.return_path.state(now_ns)
    }

    /// [KURULUŞ-GUARD] PreConverge'de arbiter çıkışının sıfırlandığı karar sayısı (salt gözlem).
    /// `> 0` = kuruluş fazında parity önerildi ve guard tarafından tele çıkmadan kesildi.
    #[must_use]
    pub fn diag_preconverge_parity_blocked(&self) -> u64 {
        self.diag_preconverge_parity_blocked
    }

    /// [EVIDENCE ENSTRÜMAN] İlk L1 ve ilk L2 anındaki `(sent_window_units, instant_ppm,
    /// persistent_ppm)`; `(0,0,0)` = hiç olmadı.
    #[must_use]
    pub fn diag_spec_transitions(&self) -> ((u64, u64, u64), (u64, u64, u64)) {
        (self.diag_first_l1_ctx, self.diag_first_l2_ctx)
    }

    /// [ONAY UFKU] Ledger confirm ufku kolunu kurar. Default = sevk davranışı (byte-özdeş).
    ///
    /// TEK DEĞİŞKEN: yalnız `ledger_tick`'in confirm argümanını etkiler. Kayıp TESPİTİ
    /// (`detect_time_losses` zaman-eşiği, packet-threshold) ve expire penceresi (`w_loss`)
    /// DEĞİŞMEZ — aksi hâlde kol iki eksende birden kayardı ve ölçüm yorumlanamazdı.
    pub fn set_loss_confirm_horizon(&mut self, h: LossConfirmHorizon) {
        self.loss_confirm_horizon = h;
    }

    /// [ONAY UFKU] Yürürlükteki confirm ufku (gözlem/test).
    #[must_use]
    pub fn loss_confirm_horizon(&self) -> LossConfirmHorizon {
        self.loss_confirm_horizon
    }

    /// [R-SWEEP] Yapı-gate seviyesi override'ı; `None` = `fec_med` (sevk, byte-özdeş).
    pub fn set_structure_gate_level(&mut self, level_permille: Option<u32>) {
        self.structure_gate_level_permille = level_permille;
    }

    /// [R-SWEEP/TEST] Fuzzy config'in `fec_med`'i (gate default'unun kaynağı).
    #[must_use]
    pub fn fuzzy_config_fec_med_for_test(&self) -> u32 {
        self.fuzzy.config().fec_med
    }

    /// [R-SWEEP] Yürürlükteki gate seviyesi (‰).
    #[must_use]
    pub fn gate_level_permille(&self) -> u32 {
        self.structure_gate_level_permille
            .unwrap_or(self.fuzzy.config().fec_med)
    }

    pub fn set_fec_structure_policy(&mut self, policy: FecStructurePolicy) {
        self.fec_structure_policy = policy;
    }

    /// Kurulu yapı-gate politikası (gözlem/test).
    #[must_use]
    pub fn fec_structure_policy(&self) -> FecStructurePolicy {
        self.fec_structure_policy
    }

    /// Commit-I/K: blok-AÇILIŞINDA çağrılır — o-anki-aktif-mode + sender-kimliği + monoton-generation
    /// witness'a-yakalanır. Bloğun-otorite-tüketimi (latch/manuel-close) bu-witness'la (yapısal
    /// open-mode==close-mode; tek-kullanım-by-value). Generation `saturating_add` — u64::MAX'ta
    /// (2^64-açılış; ulaşılamaz) uniqueness-durur, tek-kullanım-ownership'i-ETKİLENMEZ (belgeli).
    #[must_use]
    pub fn open_fec_block_authority(&mut self) -> FecBlockAuthority {
        self.fec_authority_generation = self.fec_authority_generation.saturating_add(1);
        FecBlockAuthority {
            instance_id: self.instance_id,
            generation: self.fec_authority_generation,
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

    /// Commit-K: emisyon-İZİN-kapısı KUYRUK-GİRİŞİNE-taşındı (drain'den-kaldırıldı) — kuyruk-üyeliği=
    /// kalıcı-emisyon-izni invariantı ("membership-as-permission"): izinsiz-item-hiç-girmez → current-
    /// global-mode eski-bloğun-kuyruklu-parity'sini-BASTIRAMAZ (block-stable, Seçenek-1) + head-of-line-
    /// contamination-imkansız. Yalnız-legacy-raw-enqueue bu-kapıyı-kullanır; otorite-bağlı-close'lar
    /// izni-witness'tan-türetir.
    fn fec_emission_enabled(&self) -> bool {
        match self.fec_mode {
            FecActuationMode::Manual { enabled } => enabled,
            FecActuationMode::Controller => true,
        }
    }

    /// Commit-K: OTORİTE-DOĞRULANMIŞ kuyruk-girişi (iç-yol; izin-kapısı-atlanır çünkü izin witness/
    /// latch-kararından-türedi). Tek push-noktası: enqueued-sayacı-burada. Commit-N: attribution-
    /// tag'leri (salt-teşhis; davranışa-etkisiz).
    fn push_parity_authorized(
        &mut self,
        block_id: u32,
        esi: u16,
        size: u64,
        authority_generation: Option<u64>,
        authority_mode: FecActuationMode,
        planned_r: Option<u16>,
    ) {
        self.parity_enqueued_total = self.parity_enqueued_total.saturating_add(1);
        self.pending_fec.push_back(PendingFecParity {
            block_id,
            esi,
            size,
            authority_generation,
            authority_mode,
            planned_r,
        });
    }

    /// Commit-K: MANUEL blok-KAPANIŞI — witness'ı BY-VALUE-TÜKETİR (tek-kullanım). Manual{true}-witness
    /// → `manual_r`-adet parity kuyruklanır (esi 0..r; izin-witness'tan → sonraki-mode-değişimi-
    /// bastıramaz); Manual{false}-witness → Ok(0) parity-yok (meşru-kapanış, hata-değil). Manuel-r-
    /// politikası (değer-seçimi/clamp) çağıranın — production-block-owner-entegrasyonu DEFERRED.
    ///
    /// # Errors
    /// [`FecAuthorityError::ForeignAuthority`] — witness-başka-Sender'ın (kimlik-reddi, mode'dan-ÖNCE).
    /// [`FecAuthorityError::NotInManualMode`] — Controller-witness manuel-close'a-verildi.
    pub fn close_fec_block_manual(
        &mut self,
        authority: FecBlockAuthority,
        block_id: u32,
        manual_r: u16,
        parity_size: u64,
    ) -> Result<u16, FecAuthorityError> {
        if authority.instance_id != self.instance_id {
            self.foreign_authority_rejected_count =
                self.foreign_authority_rejected_count.saturating_add(1);
            return Err(FecAuthorityError::ForeignAuthority);
        }
        match authority.mode {
            FecActuationMode::Manual { enabled: true } => {
                for esi in 0..manual_r {
                    self.push_parity_authorized(
                        block_id,
                        esi,
                        parity_size,
                        Some(authority.generation),
                        authority.mode,
                        Some(manual_r),
                    );
                }
                #[cfg(feature = "experimental-fec-budget")]
                if manual_r > 0 {
                    self.budget_admit_block(block_id, authority.generation, parity_size);
                }
                Ok(manual_r)
            }
            FecActuationMode::Manual { enabled: false } => Ok(0),
            FecActuationMode::Controller => Err(FecAuthorityError::NotInManualMode),
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

    /// Commit-G/K: block-close-LATCH — otorite-TÜKETİMİ + saf-karar-zinciri (§9) + planned-parity-
    /// STAGING tek-atomik-operasyonda: fusion→same-event-eligibility→no-rule/initial-guard→ratio→
    /// k_actual-r→profile-clamp→emergency-suppression→planned→(planned>0 ise esi 0..planned kuyruklanır,
    /// izin-witness'tan → block-stable). Witness by-value-TÜKETİLİR (tek-kullanım E0382); yabancı-
    /// Sender-witness'ı kimlik-guard'ıyla-reddedilir (mode-semantiğinden-ÖNCE). Her-adım
    /// FecLatchResult'ta-görünür. Baskılamalar LITERAL-0 (config-bağımsız).
    pub fn fec_controller_latch(
        &mut self,
        authority: FecBlockAuthority,
        snapshot: Option<ControllerDecisionSnapshot>,
        k_actual: u8,
        profile_r_max: u16,
        block_id: u32,
        parity_size: u64,
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
        // Commit-K GUARD-ÖNCELİĞİ-1: kimlik (yabancı-witness) — otorite-semantiğinden-ÖNCE.
        if authority.instance_id != self.instance_id {
            self.foreign_authority_rejected_count =
                self.foreign_authority_rejected_count.saturating_add(1);
            let r = zero(0, FecSuppressionReason::ForeignAuthority, false);
            self.last_latch = Some(r);
            return r;
        }
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
                        // [FAZ-2 / KALEM-4 — SPEC UYUMU] Acil durumda davranış FRAMING'e bağlıdır
                        // (engine §11 L771-772 / TDD §5.6 L584-585, NORMATİF):
                        //   Tiny        → parity YOK, kurtarma zaman-aralıklı D× kopyayla yapılır
                        //   SegmentBlock→ **minimal parity shard** (düşük r/k, ACK-bağımsız)
                        // Kod her iki kolda da parity'yi 0'a düşürüyordu ("temporary safety
                        // interlock pending 1b/1d arbitration") — yani segment-block kolunda
                        // spec'in TERSİNİ yapıyordu (denetim tema B). `emergency::action` zaten
                        // `MinimalParity { permille }` üretiyor; burada TÜKETİLİYOR.
                        // Kalibrasyon YAPILMADI: oran `EmergencyConfig::fixture()` değeridir.
                        let minimal = match self.last_recovery {
                            RecoveryAction::MinimalParity { permille } => permille,
                            _ => 0,
                        };
                        if minimal > 0 {
                            let (mc, mclamped) =
                                compute_requested_r(minimal, k_actual, profile_r_max);
                            FecLatchResult {
                                requested_ratio_permille: minimal,
                                computed_r: mc,
                                clamped_r: mclamped,
                                planned_r: mclamped,
                                same_event_eligible: true,
                                suppression: None,
                            }
                        } else {
                            FecLatchResult {
                                requested_ratio_permille: s.ratio_permille,
                                computed_r: computed,
                                clamped_r: clamped,
                                planned_r: 0,
                                same_event_eligible: true,
                                suppression: Some(
                                    FecSuppressionReason::EmergencyPendingArbitration,
                                ),
                            }
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
        // Commit-K: STAGING — planned-parity witness-otoritesiyle-kuyruklanır (izin-kaynağı=bu-latch;
        // sonraki-global-mode-değişimi-bastıramaz). planned_r=0 → kuyruk-dokunulmaz.
        for esi in 0..r.planned_r {
            self.push_parity_authorized(
                block_id,
                esi,
                parity_size,
                Some(authority.generation),
                authority.mode,
                Some(r.planned_r),
            );
        }
        #[cfg(feature = "experimental-fec-budget")]
        if r.planned_r > 0 {
            self.budget_admit_block(block_id, authority.generation, parity_size);
        }
        self.last_latch = Some(r);
        r
    }

    /// Commit-N: token-denial-teşhisi — `(son-denial-snapshot, kümülatif-denial-olay-sayısı)`.
    /// SALT-GÖZLEM; §8.1-gauge'dan-ayrı (gauge=pending-adet; bu=attribution).
    #[must_use]
    pub fn diag_token_denial(&self) -> (Option<FecTokenDenialSnapshot>, u64) {
        (self.last_token_denial, self.token_denial_count_total)
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
            // Commit-K-corrigendum: parity-iptalleri (retransmit-iptalleri diag_recovered_cancels'ta).
            canceled_r_total: self.parity_canceled_total,
            parity_enqueue_rejected_count: self.parity_enqueue_rejected_count,
            foreign_authority_rejected_count: self.foreign_authority_rejected_count,
            token_denied_r_gauge: self.token_denied_r_gauge,
            last_transition_event_index: self.last_transition_event_index,
            transition_applied_count: self.transition_applied_count,
        }
    }

    /// LEGACY/test-sınıfı raw-enqueue (DR-S5-1-kökenli; TEST-HARNESS-yolu — üretim-block-owner otorite-
    /// bağlı-close-API'lerini KULLANMALI: `fec_controller_latch`/`close_fec_block_manual`).
    /// Commit-K izin-kapısı: emisyon-izni ENQUEUE-ANINDA-karara-bağlanır (aktif-mode-snapshot'ı) —
    /// izinsiz (Manual{false}-aktif) çağrı KUYRUĞA-GİRMEZ (sayaç+diag-görünür; kuyruk-üyeliği=kalıcı-
    /// emisyon-izni invariantı korunur). DAVRANIŞ-DEĞİŞİMİ (bilinçli, Seçenek-1): eski "OFF'ta-kuyrukta-
    /// beklet, re-enable'da-devam" pause/resume-semantiği KALDIRILDI — izin artık-kaynak-anında-kalıcı.
    /// SINIF-UYARISI (adversarial-bulgu m3): Controller-aktifken bu-yol KARARSIZ-battaniye-kabuldür
    /// (`fec_emission_enabled()` Controller-kolu=true; latch-emergency-interlock'unu-atlar) —
    /// deprecation product-reachability-sınıflandırmasının KOD-DÜZEYİ-enforcementıdır.
    #[deprecated(
        note = "legacy test-harness yolu — üretim-block-owner otorite-bağlı-close kullanmalı: \
                fec_controller_latch / close_fec_block_manual (Commit-K)"
    )]
    pub fn enqueue_fec_parity(&mut self, block_id: u32, esi: u16, size: u64) {
        if !self.fec_emission_enabled() {
            self.parity_enqueue_rejected_count =
                self.parity_enqueue_rejected_count.saturating_add(1);
            return;
        }
        let mode = self.fec_mode;
        self.push_parity_authorized(block_id, esi, size, None, mode, None);
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
        // K-8/F4 TETİK-2b: TimedOut-SONRASI §11 forward-recovery RECOVERED → PostTimeoutNotice
        // (65B-sınıfı ACK-bağımsız heal; B1-amendman). SALT-LOGICAL (N8: BtlBw'ye dokunmaz). first-valid-
        // proof-wins (slot-doluysa no-op). Yalnız TimedOut-terminali-alınmışsa (post-timeout).
        #[cfg(feature = "experimental-cold-start")]
        if self.cs_epoch_timed_out {
            if let Some(ru) = info.units.first() {
                let now = self.last_now_ns;
                self.record_post_timeout_notice(
                    ru.block_id,
                    PostTimeoutCause::RecoveredControl,
                    now,
                );
            }
        }
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
            // (iv) Commit-K parity-CANCEL: blok-decode-edildi (RS all-or-nothing) → o-bloğun-kuyruklu-
            // parity'si-anlamsız; block_id-bazlı-retain (sıra-korunur; authority-binding-cancel'i-
            // BOZMAZ — üyelik-izni-iptalle-çelişmez, iptal=iş-yok-oldu). bw/pacer'a-SIFIR-satır (N8).
            let before_fec = self.pending_fec.len();
            self.pending_fec.retain(|p| p.block_id != ru.block_id);
            self.parity_canceled_total = self
                .parity_canceled_total
                .saturating_add((before_fec - self.pending_fec.len()) as u64);
            // Commit-V/U4: ticket-CanceledMoot (deadline-öncesi≠miss; sonrası-miss-zaten-1-kez-sayıldı).
            #[cfg(feature = "experimental-fec-budget")]
            {
                let now = self.last_now_ns;
                let mut before_cnt = 0u64;
                let mut after_cnt = 0u64;
                self.budget_ext.tickets.retain(|t| {
                    if t.block_id == ru.block_id {
                        if t.missed || now > t.deadline_ns {
                            after_cnt += 1;
                        } else {
                            before_cnt += 1;
                        }
                        false
                    } else {
                        true
                    }
                });
                self.budget_ext.tk_cancel_before =
                    self.budget_ext.tk_cancel_before.saturating_add(before_cnt);
                self.budget_ext.tk_cancel_after =
                    self.budget_ext.tk_cancel_after.saturating_add(after_cnt);
            }
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

    /// Bekleyen FEC-parity'yi pace-ederek emit et (DR-S5-1/Commit-K). Kuyruk-üyeliği=emisyon-izni
    /// (izin-kapısı-girişte) → drain'de-mode-kontrolü-YOK: current-global-mode eski-bloğun-kuyruklu-
    /// parity'sini-BASTIRAMAZ (block-stable). FEC-off-hot-path: kuyruk-boş → while-hiç-girmez,
    /// allocation-yok (C-kuralı byte-değişmez). Pacing: mevcut `pacer.try_send` (TokenBucket-internals'a
    /// DOKUNMAZ; data/retx ile aynı-API). send-record-hint üretir.
    fn plan_fec_parity(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        // Commit-V/F2: FloorCap'te ticket'lı-blokların-İLK-parity'si-önce (parity-İÇİ-seçim; onaylı-scope).
        #[cfg(feature = "experimental-fec-budget")]
        self.budget_service_tickets(out, now_ns);
        while let Some(&p) = self.pending_fec.front() {
            #[cfg(feature = "experimental-fec-budget")]
            if !self.budget_parity_allows(p.size) {
                break; // C1-canlı: kernel-limit (Leftover'da-None → hiç-girmez; baseline-özdeş)
            }
            let balance_before = self.pacer.tokens(); // READ-ONLY (mühürlü-TokenBucket'a-dokunulmaz)
            if !self.pacer.try_send(p.size) {
                // Commit-G: budget-denied-GÖRÜNÜRLÜĞÜ (gauge; 1b-tasarım-girdisi — politika-DEĞİŞMEDİ).
                self.token_denied_r_gauge = self.pending_fec.len() as u64;
                // Commit-N: attribution-snapshot (TAM-overwrite; yalnız-GERÇEK-token-denial-yolu).
                self.token_denial_count_total = self.token_denial_count_total.saturating_add(1);
                self.last_token_denial = Some(FecTokenDenialSnapshot {
                    denial_event_index: self.fusion_event_index,
                    block_id: p.block_id,
                    authority_generation: p.authority_generation,
                    authority_mode: p.authority_mode,
                    pending_parity_total: self.pending_fec.len() as u64,
                    pending_for_block: self
                        .pending_fec
                        .iter()
                        .filter(|q| q.block_id == p.block_id)
                        .count() as u64,
                    parity_size_bytes: p.size,
                    token_balance_before: balance_before,
                    planned_r: p.planned_r,
                    emitted_r_total_at_denial: self.parity_emitted_total,
                });
                break; // token yok → sonraki tick (data/retx ile aynı pacing-davranışı).
            }
            let _sz = self.emit_parity_at(0, out, now_ns);
            #[cfg(feature = "experimental-fec-budget")]
            self.budget_parity_spend(_sz);
        }
    }

    /// Parity-emisyon-gövdesi (Commit-V-refaktörü; davranış-birebir — try_send-BAŞARISI-sonrası-çağrılır).
    /// idx: kuyruk-konumu (bulk=0; F2-ticket-servisi-hedef-blok-item'ı). Döner: emit-edilen-boyut.
    fn emit_parity_at(&mut self, idx: usize, out: &mut Vec<TransmitDecision>, now_ns: u64) -> u64 {
        let p = self
            .pending_fec
            .remove(idx)
            .expect("emit_parity_at: idx-geçerli");
        self.parity_emitted_total = self.parity_emitted_total.saturating_add(1);
        // [FEC-RISK ENSTRÜMAN] İlk parity'nin TELE ÇIKTIĞI an — kolun asıl ölçtüğü büyüklük
        // `t_first_parity − t_first_suspected` (FEC'in tepki gecikmesi).
        if self.diag_t_first_parity_ns == 0 {
            self.diag_t_first_parity_ns = now_ns;
        }
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
        p.size
    }

    // ── Commit-V: EXPERIMENTAL-BUDGET metodları (cfg; OFF'ta-derlenmez) ──

    /// U6: policy-geçişi — Busy-guard (ticket/bulk/rezerv-boşken); başarıda credit-reset,
    /// kümülatif-sayaçlar-KORUNUR, gauge'lar-sıfır (last_floor_miss-KORUNUR — dokümante).
    #[cfg(feature = "experimental-fec-budget")]
    pub fn set_budget_policy(
        &mut self,
        policy: BudgetPolicy,
    ) -> Result<(), BudgetPolicyTransitionError> {
        if !self.budget_ext.tickets.is_empty()
            || !self.pending_fec.is_empty()
            || self.budget_ext.reserved_now > 0
        {
            return Err(BudgetPolicyTransitionError::Busy);
        }
        self.budget_ext.policy = policy;
        self.budget_ext.state.reset();
        self.budget_ext.reserved_now = 0;
        self.budget_ext.pending_decision = None;
        self.budget_ext.spent_this_opportunity = 0;
        self.budget_ext.profile_parity_size = 0;
        Ok(())
    }

    /// Canlı-telemetri (salt-gözlem).
    #[cfg(feature = "experimental-fec-budget")]
    #[must_use]
    pub fn diag_budget(&self) -> BudgetDiag {
        let b = &self.budget_ext;
        BudgetDiag {
            entitlement_credit: b.state.entitlement_credit,
            hardcap_credit: b.state.hardcap_credit,
            reserved_now: b.reserved_now,
            tickets_pending: b.tickets.len() as u64,
            tk_rejected: b.tk_rejected,
            tk_admitted: b.tk_admitted,
            tk_on_time: b.tk_on_time,
            tk_late: b.tk_late,
            tk_missed_unique: b.tk_missed_unique,
            tk_cancel_before: b.tk_cancel_before,
            tk_cancel_after: b.tk_cancel_after,
            bulk_admission_rejected: b.bulk_admission_rejected,
            last_floor_miss: b.last_floor_miss,
            reservation_waste_total: b.reservation_waste_total,
            new_data_displaced: b.new_data_displaced,
            retx_displaced: b.retx_displaced,
            cap_suppressed_events: b.cap_suppressed_events,
            rate_zero_events: b.rate_zero_events,
        }
    }

    #[cfg(feature = "experimental-fec-budget")]
    fn budget_kind_params(
        &self,
    ) -> Option<(
        crate::budget_kernel::PolicyKind,
        crate::budget_kernel::PolicyParams,
    )> {
        match self.budget_ext.policy {
            BudgetPolicy::LeftoverOnly => None,
            BudgetPolicy::ProtectedShareExperimental(p) => {
                Some((crate::budget_kernel::PolicyKind::ProtectedShare, p))
            }
            BudgetPolicy::FloorCapExperimental(p) => {
                Some((crate::budget_kernel::PolicyKind::FloorCap, p))
            }
        }
    }

    /// U12-atomik-rezervasyon: yalnız-AYNI-opportunity'de-gerçekten-emit-edilebilir-TAM-kuantum.
    /// (RECOVERED-cancel'ler kendi-event'lerinde-önce-işlenmiştir — moot-item-rezervi-yapısal-yok.)
    #[cfg(feature = "experimental-fec-budget")]
    #[allow(clippy::cast_possible_truncation)]
    fn budget_reserve(&mut self, now_ns: u64) {
        self.budget_ext.spent_this_opportunity = 0;
        let Some((kind, params)) = self.budget_kind_params() else {
            self.budget_ext.reserved_now = 0; // LeftoverOnly: inert (baseline-özdeş)
            self.budget_ext.parity_budget_left = None;
            return;
        };
        let elapsed = now_ns.saturating_sub(self.budget_ext.last_opportunity_ns);
        self.budget_ext.last_opportunity_ns = now_ns;
        let rate = self.pacer.rate();
        if rate == 0 {
            // U7: SuspendedNoRate (üretim-yolu-pratik-erişilmez; ölçülür).
            self.budget_ext.rate_zero_events = self.budget_ext.rate_zero_events.saturating_add(1);
            self.budget_ext.reserved_now = 0;
            return;
        }
        // U4-miss-taraması: deadline-İLK-aşımı → missed-tam-1-kez + snapshot-overwrite.
        let ev = self.fusion_event_index;
        let pend_items = self.pending_fec.len() as u64;
        let pend_tickets = self.budget_ext.tickets.len() as u64;
        for t in &mut self.budget_ext.tickets {
            if !t.missed && now_ns > t.deadline_ns {
                t.missed = true;
                self.budget_ext.tk_missed_unique =
                    self.budget_ext.tk_missed_unique.saturating_add(1);
                self.budget_ext.last_floor_miss = Some(FloorMissSnapshot {
                    block_id: t.block_id,
                    generation: t.generation,
                    event_index: ev,
                    pending_items: pend_items,
                    pending_tickets: pend_tickets,
                    cap_suppressed: false,
                });
            }
        }
        // Talepler (bounded-hesap; teşhis-sınıfı — erken-kesme available+psize-üstünde).
        let avail = self.pacer.tokens();
        let cutoff = avail.saturating_add(4 * DEFAULT_MTU);
        let mut data_demand = 0u64;
        for u in &self.tx_queue {
            data_demand += u.size;
            if data_demand > cutoff {
                break;
            }
        }
        let mut arq_demand = 0u64;
        for r in &self.pending_lost {
            arq_demand += r.unit.size;
            if arq_demand > cutoff {
                break;
            }
        }
        let psize = self
            .pending_fec
            .front()
            .map_or(self.budget_ext.profile_parity_size, |x| x.size);
        let bulk_bytes: u64 = self.pending_fec.iter().map(|x| x.size).sum();
        let n_tickets = self.budget_ext.tickets.len() as u32;
        let bulk_minus_tickets =
            bulk_bytes.saturating_sub(u64::from(n_tickets).saturating_mul(psize));
        let oldest_age = self.budget_ext.tickets.front().map_or(0, |t| {
            now_ns
                .saturating_sub(t.deadline_ns.saturating_sub(0))
                .min(now_ns)
        });
        let ctx = crate::budget_kernel::BudgetContext {
            elapsed_ns: elapsed,
            pacing_rate_bps: rate,
            available_tokens: avail,
            data_demand_bytes: data_demand,
            arq_demand_bytes: arq_demand,
            bulk_parity_demand_bytes: bulk_minus_tickets,
            floor_tickets: n_tickets,
            oldest_ticket_age_ns: oldest_age,
            parity_size: psize,
            srtt_ns: self.rtt.srtt().map_or(0, dur_ns),
        };
        let mut st = self.budget_ext.state;
        let d = crate::budget_kernel::step(kind, &params, &mut st, &ctx);
        self.budget_ext.state = st;
        if d.cap_suppressed {
            self.budget_ext.cap_suppressed_events =
                self.budget_ext.cap_suppressed_events.saturating_add(1);
        }
        // U12-EXACT-kuantum: rezerv = tam-emit-edilebilir-birimler (limit, pacer-toplamı, pending).
        let reserved = d.parity_token_limit.checked_div(psize).map_or(0, |units| {
            units
                .min(avail / psize)
                .min(self.pending_fec.len() as u64)
                .saturating_mul(psize)
        });
        self.budget_ext.pending_decision = Some(d);
        self.budget_ext.reserved_now = reserved;
        self.budget_ext.parity_budget_left = Some(d.parity_token_limit);
    }

    /// C1-canlı-enforcement: parity-harcaması kernel-limitini-aşamaz (Leftover: None=sınırsız-baseline).
    #[cfg(feature = "experimental-fec-budget")]
    fn budget_parity_allows(&self, size: u64) -> bool {
        self.budget_ext
            .parity_budget_left
            .is_none_or(|left| left >= size)
    }

    #[cfg(feature = "experimental-fec-budget")]
    fn budget_parity_spend(&mut self, size: u64) {
        if let Some(left) = self.budget_ext.parity_budget_left.as_mut() {
            *left = left.saturating_sub(size);
        }
        self.budget_ext.spent_this_opportunity =
            self.budget_ext.spent_this_opportunity.saturating_add(size);
    }

    /// F2-ticket-servisi (FloorCap): ticket-FIFO-önünden; hedef-bloğun-İLK-pending-item'ı (bounded-scan).
    #[cfg(feature = "experimental-fec-budget")]
    fn budget_service_tickets(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        if !matches!(
            self.budget_ext.policy,
            BudgetPolicy::FloorCapExperimental(_)
        ) {
            return;
        }
        while let Some(t) = self.budget_ext.tickets.front().copied() {
            let Some(idx) = self
                .pending_fec
                .iter()
                .position(|q| q.block_id == t.block_id)
            else {
                // Item-yok (yarış-artığı): ticket-düş (cancel-muhasebesi RECOVERED-yolunda yapılır).
                self.budget_ext.tickets.pop_front();
                continue;
            };
            let size = self.pending_fec[idx].size;
            if !self.budget_parity_allows(size) {
                break; // C1: kernel-parity-limiti (hardcap-dahil) aşılamaz → miss-adayı (teşhisli)
            }
            if !self.pacer.try_send(size) {
                break; // token-yok → sonraki-opportunity (miss-taraması-rezervde)
            }
            let sz = self.emit_parity_at(idx, out, now_ns);
            self.budget_parity_spend(sz);
            self.budget_ext.tickets.pop_front();
            if t.missed || now_ns > t.deadline_ns {
                self.budget_ext.tk_late = self.budget_ext.tk_late.saturating_add(1);
            } else {
                self.budget_ext.tk_on_time = self.budget_ext.tk_on_time.saturating_add(1);
            }
        }
    }

    /// U12-adım-8: settle + waste-denetimi (normal-deterministik-yolda waste=0 — kabul-kriteri).
    #[cfg(feature = "experimental-fec-budget")]
    fn budget_settle(&mut self) {
        let Some(d) = self.budget_ext.pending_decision.take() else {
            self.budget_ext.reserved_now = 0;
            return;
        };
        let Some((kind, params)) = self.budget_kind_params() else {
            self.budget_ext.reserved_now = 0;
            return;
        };
        // C1-canlı-enforcement-sonrası spent ≤ limit YAPISAL (clamp-maskesi-yok; kernel-assert-gerçek).
        let spent = self.budget_ext.spent_this_opportunity;
        if self.budget_ext.reserved_now > spent {
            self.budget_ext.reservation_waste_total = self
                .budget_ext
                .reservation_waste_total
                .saturating_add(self.budget_ext.reserved_now - spent);
        }
        let mut st = self.budget_ext.state;
        let _borrowed = crate::budget_kernel::settle(kind, &params, &mut st, &d, spent);
        self.budget_ext.state = st;
        self.budget_ext.reserved_now = 0;
        self.budget_ext.parity_budget_left = None;
        self.budget_ext.spent_this_opportunity = 0;
    }

    /// U-staging-hook: otorite-bağlı-staging-sonrası ticket-admission + bulk-cap (yalnız-experimental-
    /// policy'de; LeftoverOnly'de-çağrı-inert). U11: size-sabitliği-enforce (≠profil → ticket-yok).
    #[cfg(feature = "experimental-fec-budget")]
    fn budget_admit_block(&mut self, block_id: u32, generation: u64, parity_size: u64) {
        let Some((kind, params)) = self.budget_kind_params() else {
            return;
        };
        if kind != crate::budget_kernel::PolicyKind::FloorCap {
            return;
        }
        if self.budget_ext.profile_parity_size == 0 {
            self.budget_ext.profile_parity_size = parity_size;
        }
        let caps = crate::budget_kernel::AdmissionCaps {
            pending_items_cap: 64,
            pending_bytes_cap: 64 * self.budget_ext.profile_parity_size.max(1),
            floor_ticket_cap: 8,
            per_block_admitted_cap: 8,
            parity_size_max: 2 * DEFAULT_MTU,
        };
        let has_ticket = self
            .budget_ext
            .tickets
            .iter()
            .any(|t| t.block_id == block_id);
        let size_ok = parity_size == self.budget_ext.profile_parity_size;
        let res = crate::budget_kernel::admit(
            &caps,
            self.pending_fec.len() as u32,
            self.pending_fec.iter().map(|x| x.size).sum(),
            self.budget_ext.tickets.len() as u32,
            0,
            parity_size,
            true,
            has_ticket,
            size_ok, // size≠profil → bulk-only (U11 strict-FIFO-güvencesi)
        );
        match res {
            crate::budget_kernel::AdmissionResult::AdmittedWithFloorTicket => {
                let rate = self.pacer.rate();
                let srtt = self.rtt.srtt().map_or(0, dur_ns);
                let delta =
                    match crate::budget_kernel::floor_timing(&params, srtt, parity_size, rate) {
                        crate::budget_kernel::FloorTiming::Eligible { deadline_delta_ns } => {
                            deadline_delta_ns
                        }
                        _ => {
                            // U7: rate=0 → askıda; deadline-ilk-pozitif-rate'te-başlar (burada: uzak-işaret).
                            self.budget_ext.rate_zero_events =
                                self.budget_ext.rate_zero_events.saturating_add(1);
                            u64::MAX / 2
                        }
                    };
                self.budget_ext.tickets.push_back(FloorTicket {
                    block_id,
                    generation,
                    deadline_ns: self.last_now_ns.saturating_add(delta),
                    missed: false,
                });
                self.budget_ext.tk_admitted = self.budget_ext.tk_admitted.saturating_add(1);
            }
            crate::budget_kernel::AdmissionResult::AdmittedBulkOnly => {}
            _ => {
                self.budget_ext.tk_rejected = self.budget_ext.tk_rejected.saturating_add(1);
            }
        }
    }

    /// RECOVER_ARQ (§9): ACK-güdümlü selective retransmit. arq_aggressiveness=0 → baskıla (fec-dominant).
    /// Commit-V: pre-parity-token-kapısı. OFF: saf-try_send (inline; byte-özdeş). ON: rezerv-korunur —
    /// `tokens < size + reserved` → red (yalnız-tokens≥size-iken-displacement-sayılır: "would-have-sent").
    #[inline]
    fn try_send_pre_parity(&mut self, size: u64, is_retx: bool) -> bool {
        #[cfg(feature = "experimental-fec-budget")]
        {
            let r = self.budget_ext.reserved_now;
            if r > 0 && self.pacer.tokens() < size.saturating_add(r) {
                if self.pacer.tokens() >= size {
                    if is_retx {
                        self.budget_ext.retx_displaced =
                            self.budget_ext.retx_displaced.saturating_add(size);
                    } else {
                        self.budget_ext.new_data_displaced =
                            self.budget_ext.new_data_displaced.saturating_add(size);
                    }
                }
                return false;
            }
        }
        #[cfg(not(feature = "experimental-fec-budget"))]
        let _ = is_retx;
        self.pacer.try_send(size)
    }

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
                    #[cfg(feature = "diag-sched-telemetry")]
                    {
                        self.sched_diag.arq_retransmit_selected += 1;
                    }
                    if !self.try_send_pre_parity(rec.unit.size, true) {
                        #[cfg(feature = "diag-sched-telemetry")]
                        {
                            self.sched_diag.arq_retransmit_token_denied += 1;
                        }
                        break; // token yok → sonraki tick'te (deadline plandan düşmez, §9).
                    }
                    #[cfg(feature = "diag-sched-telemetry")]
                    {
                        self.sched_diag.arq_retransmit_wire_emitted += 1;
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

    /// DR-ARQ-PRODUCT-TAIL-1 (gated; ON-only — never reached when disabled): retire a still-outstanding `seq` from
    /// inflight into a bounded tombstone WITHOUT declaring loss (no pending_lost push, no loss-counter mutation —
    /// INV-RATE-1). Used by the PTO tail probe to supersede the original before retransmitting a probe copy, so a
    /// late ACK of the ORIGINAL seq is disambiguated (Karn) and its bytes are not double-counted in inflight.
    /// Mirrors the Data/Duplicate tombstone bookkeeping of `queue_losses` (same expiry clock, bounded eviction,
    /// bw in-flight accounting) minus the loss semantics. OFF-mode is unaffected (this is unreachable when disabled).
    fn retire_inflight_to_tombstone(&mut self, seq: u64, now_ns: u64) {
        let Some(p) = self.inflight.remove(&seq) else {
            return;
        };
        self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
        if p.origin == TransmitKind::Duplicate {
            // Dup-origin: no tombstone (matches queue_losses) — drop the in-flight count here.
            self.bw_sampler.forget_in_flight();
            return;
        }
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let t_loss_ns = (dur_ns(srtt) * 9 / 8).max(dur_ns(self.cadence_loss_floor()));
        let expiry_ns = now_ns.saturating_add(t_loss_ns.saturating_mul(u64::from(K_TOMB)));
        while self.tombstones.len() >= TOMBSTONE_MAX {
            match self.tombstone_order.pop_front() {
                Some(old) => {
                    if self.tombstones.remove(&old).is_some() {
                        self.tombstone_evicted += 1;
                        self.bw_sampler.forget_in_flight();
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

    /// DR-ARQ-PRODUCT-TAIL-1 (gated; default OFF → no-op): emit at most one forced PTO tail probe. Called from
    /// schedule_transmit AFTER the normal ARQ planner and REGARDLESS of sender state (a PTO probe is independent of
    /// the RecoverArq state machine). ARQ-eligibility override ONLY — emission still uses the safe token
    /// (try_send_pre_parity) + track_sent path (no pacing/wire-format bypass). Re-verifies the target is still
    /// outstanding at emission time (ACK race / already-retransmitted ⇒ absent from inflight ⇒ cancelled, attempt NOT
    /// consumed). Declares NO "confirmed loss" (pending_lost/loss counters untouched — INV-RATE-1). Karn: the probe
    /// copy is emitted as a Retransmit (origin ≠ Data ⇒ excluded from RTT sampling) and the superseded original's
    /// tombstone retx_count is bumped so its late ACK is Karn-excluded too. Cap spent only on a real emission (§3).
    fn emit_tail_probe(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        if !self.tail_recovery.is_enabled() {
            return;
        }
        let Some(target) = self.tail_probe_pending.take() else {
            return;
        };
        let Some((unit, retx_count)) = self.inflight.get(&target).map(|r| (r.unit, r.retx_count))
        else {
            // target no longer outstanding (ACK race / already retransmitted) → cancelled; attempt NOT consumed.
            self.tail_probe_cancelled = self.tail_probe_cancelled.saturating_add(1);
            return;
        };
        #[cfg(feature = "diag-sched-telemetry")]
        {
            self.sched_diag.tail_probe_selected += 1;
        }
        if !self.try_send_pre_parity(unit.size, true) {
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.tail_probe_token_denied += 1;
            }
            self.tail_probe_pending = Some(target); // no token → keep pending; cap NOT consumed.
            return;
        }
        #[cfg(feature = "diag-sched-telemetry")]
        {
            self.sched_diag.tail_probe_wire_emitted += 1;
        }
        // Supersede the still-outstanding original: retire it inflight→tombstone (no loss declaration), then bump
        // the tombstone so the original's late ACK is Karn-excluded, then emit the probe copy under a fresh seq.
        self.retire_inflight_to_tombstone(target, now_ns);
        self.mark_tombstone_superseded(target);
        let new_seq = self.track_sent(unit, now_ns, retx_count + 1, TransmitKind::Retransmit);
        out.push(TransmitDecision {
            kind: TransmitKind::Retransmit,
            seq: new_seq,
            unit_id: unit.id,
            size: unit.size,
            send_record: None,
        });
        self.tail_recovery.on_probe_sent(now_ns); // spend the attempt only on a real emission.
        self.tail_recovery.record_probe(unit.size, false);
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
    /// [DR-EST-1] Bkz. schedule_transmit'teki çağrı-yorumu. Tek-slot, backoff'lu, token-muaf.
    fn drain_establish_probe(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        if !self.establish_probe_enabled {
            return;
        }
        if self.state != SenderState::PreConverge {
            self.establish_due_ns = 0; // teyit geldi / faz değişti → söner.
            return;
        }
        if self.dup_queue.is_empty() {
            return; // kaynak yok (ilk DATA henüz kopya planlamadı).
        }
        if self.establish_due_ns == 0 {
            self.establish_interval_ns = ESTABLISH_PROBE_INITIAL_NS;
            self.establish_due_ns = now_ns.saturating_add(self.establish_interval_ns);
            return;
        }
        if now_ns < self.establish_due_ns {
            return;
        }
        // En-eski vadeli §3.3-B kopyası; drain_due_duplicates gövdesinin try_send'siz tek-elemanlı
        // ikizi — dup'ın KENDİ geometrik reschedule'ı aynen sürer (kopya-sayısı şişmez).
        let Some(i) = self
            .dup_queue
            .iter()
            .enumerate()
            .min_by_key(|(_, d)| d.due_ns)
            .map(|(i, _)| i)
        else {
            return;
        };
        let dup = self.dup_queue.remove(i);
        let seq = self.track_sent(dup.unit, now_ns, 0, TransmitKind::Duplicate);
        out.push(TransmitDecision {
            kind: TransmitKind::Duplicate,
            seq,
            unit_id: dup.unit.id,
            size: dup.unit.size,
            send_record: None,
        });
        if dup.interval_ns > 0 {
            let next_interval = dup.interval_ns.saturating_mul(PC_COPY_MULTIPLIER);
            self.dup_queue.push(DupRec {
                due_ns: now_ns.saturating_add(next_interval),
                unit: dup.unit,
                interval_ns: next_interval,
            });
        }
        self.establish_probe_emitted = self.establish_probe_emitted.saturating_add(1);
        self.establish_interval_ns = self
            .establish_interval_ns
            .saturating_mul(2)
            .min(ESTABLISH_PROBE_CAP_NS);
        self.establish_due_ns = now_ns.saturating_add(self.establish_interval_ns);
    }

    /// [DR-EST-1] Kuruluş-kolu anahtarı (test-görünür; default-ON). Bkz. alan-doc'u.
    pub fn set_establish_probe(&mut self, on: bool) {
        self.establish_probe_enabled = on;
        if !on {
            self.establish_due_ns = 0;
        }
    }

    /// [DR-EST-1] gözlem-sayacı.
    #[must_use]
    pub fn diag_establish_probe_emitted(&self) -> u64 {
        self.establish_probe_emitted
    }

    fn drain_due_duplicates(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        let mut i = 0;
        while i < self.dup_queue.len() {
            if self.dup_queue[i].due_ns > now_ns {
                i += 1;
                continue; // henüz vadesi gelmedi.
            }
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.dup_selected += 1;
            }
            if !self.try_send_pre_parity(self.dup_queue[i].unit.size, true) {
                #[cfg(feature = "diag-sched-telemetry")]
                {
                    self.sched_diag.dup_token_denied += 1;
                }
                break; // token yok.
            }
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.dup_wire_emitted += 1;
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
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.data_selected += 1;
            }
            if !self.try_send_pre_parity(front.size, false) {
                #[cfg(feature = "diag-sched-telemetry")]
                {
                    self.sched_diag.data_token_denied += 1;
                }
                break;
            }
            #[cfg(feature = "diag-sched-telemetry")]
            {
                self.sched_diag.data_wire_emitted += 1;
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
                // DR-CS-3 ARM: PreConverge + srtt=None + FIRST DATA wire emission (t₀ = this instant,
                // i.e. the unit passed try_send and is on the wire). Once per bootstrap epoch.
                if self.bootstrap_rtt.enabled
                    && !self.bs_done
                    && self.bs_armed_ns.is_none()
                    && self.rtt.srtt().is_none()
                {
                    self.bs_armed_ns = Some(now_ns);
                    self.bs_diag.arm += 1;
                }
                // DR-CS-1 M2 ARM (F2A): PreConverge + srtt=None + İLK-DATA-wire-emisyonu (try_send-
                // geçti = t₀) → CS-probe merdiveni epoch-başına-BİR-kez arm (fire 3/9/21/45s).
                #[cfg(feature = "experimental-cold-start")]
                if self.cs_probe_lane_enabled
                    && !self.cs_probe_armed_once
                    && self.rtt.srtt().is_none()
                {
                    self.cs_probe_armed_once = true;
                    for (i, off) in CS_PROBE_FIRES_NS.iter().enumerate() {
                        self.cs_wakeup
                            .push((now_ns.saturating_add(*off), CsTimerKind::CsProbe(i as u8)));
                    }
                    // M5: giveup-deadline (epoch-başına-bir; T_COLD_GIVEUP).
                    self.cs_wakeup
                        .push((now_ns.saturating_add(CS_GIVEUP_NS), CsTimerKind::CsGiveup));
                }
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

    /// FS-5A (DR-FEC-FS-1) — PATH-TIMING SALT-GÖZLEM snapshot: engine'in ZATEN-ölçtüğü mutlak RTT/path-
    /// timing state'i (srtt · min_rtt/rtprop · rttvar · queue_delay + confidence/freshness). `FusionInputs`
    /// yalnız-oran taşıdığından FEC-selector'a ulaşmayan MUTLAK-magnitude'ı salt-forward eder (yeni-ölçüm
    /// YOK). **Production-policy/`FusionInputs`/wire DEĞİŞMEZ** — experimental FEC-selector + FS-harness
    /// (Case-A/B rejim-ayrımı) girdisi. Freshness kanal-saatli (`last_now_ns`).
    #[must_use]
    pub fn path_timing_snapshot(&self) -> PathTimingState {
        let now = self.last_now_ns;
        let rtprop_fresh = self
            .rtt
            .rtprop_age_ns(now)
            .is_some_and(|age| age < self.rtt.rtprop_window_ns() / 2);
        PathTimingState {
            srtt_ns: self.rtt.srtt().map(dur_ns),
            min_rtt_ns: self.rtt.rtprop().map(dur_ns),
            rttvar_ns: dur_ns(self.rtt.rttvar()),
            queue_delay_ns: dur_ns(self.rtt.queue_delay()),
            sample_present: self.rtt.srtt().is_some(),
            rtprop_fresh,
        }
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

    /// [Q18-FIX] Yapı-gate'in fuzzy `fec=0`'ı `fec_med`'e açtığı event sayısı (gözlem).
    #[must_use]
    pub fn diag_structure_gate_opened(&self) -> u64 {
        self.diag_structure_gate_opened
    }

    /// [TEŞHİS] Yapı-gate karar-yolu histogramı — indeksler [`GateOutcome::index`].
    #[must_use]
    pub fn diag_gate_outcomes(&self) -> [u64; GATE_OUTCOME_KINDS] {
        self.diag_gate_outcomes
    }

    /// [TEŞHİS] Ledger pencereli kaybının koşu-boyu tepesi (ppm). Kapanış anındaki anlık değer
    /// decay etmiş olabileceğinden önkoşul teşhisi için TEK geçerli okuma budur.
    #[must_use]
    pub fn diag_loss_ppm_peak(&self) -> u64 {
        self.diag_loss_ppm_peak
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
        let a1 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a1, None, 8, 4, 1, 64);
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
        let a2 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a2, Some(stale), 8, 4, 2, 64);
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
        let a3 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a3, Some(nr), 8, 4, 3, 64);
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
        let a4 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a4, Some(em), 8, 4, 4, 64);
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
        let a5 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a5, Some(ok), 8, 4, 5, 64);
        assert!(r.same_event_eligible);
        assert_eq!((r.computed_r, r.planned_r, r.suppression), (4, 4, None));
        // ManualDisabled: Manual-mode'da-latch → 0.
        s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
            .unwrap();
        let a6 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a6, Some(ok), 8, 4, 6, 64);
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
        let a7 = s.open_fec_block_authority();
        let r = s.fec_controller_latch(a7, Some(snap), 8, 4, 7, 64);
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
        let a8 = s.open_fec_block_authority();
        let _ = s.fec_controller_latch(a8, Some(snap), 8, 4, 8, 64);
        // Manual-default'ta ManualDisabled-dalı no_rule-sayacına-girmez; Controller'a-alıp-tekrar:
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        let a9 = s.open_fec_block_authority();
        let _ = s.fec_controller_latch(a9, Some(snap), 8, 4, 9, 64);
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
        let a10 = s.open_fec_block_authority();
        let r1 = s.fec_controller_latch(a10, Some(nr), 8, 4, 10, 64);
        assert_eq!(r1.suppression, Some(FecSuppressionReason::NoRule));
        assert_eq!(s.diag_fec_actuation().last_latch, Some(r1));
        // 2. latch: TEMİZ — last_latch TAM-yeni-değer; suppression-carryover-YOK.
        let ok = ControllerDecisionSnapshot {
            ratio_permille: 500,
            ..snap
        };
        let a11 = s.open_fec_block_authority();
        let r2 = s.fec_controller_latch(a11, Some(ok), 8, 4, 11, 64);
        assert_eq!(r2.suppression, None, "temiz-latch");
        assert_eq!(
            s.diag_fec_actuation().last_latch,
            Some(r2),
            "TAM-overwrite: alan-alan-yeni-latch (önceki-suppression/ratio-taşınmadı)"
        );
        assert_ne!(r1, r2, "iki-latch-farklı → overwrite-gözlemlenebilir");
        // 3. latch: tekrar-SUPPRESSED — yine-tam-yeni (çift-yönlü-kanıt).
        let a12 = s.open_fec_block_authority();
        let r3 = s.fec_controller_latch(a12, Some(nr), 8, 4, 12, 64);
        assert_eq!(s.diag_fec_actuation().last_latch, Some(r3));
        assert_eq!(r3.suppression, Some(FecSuppressionReason::NoRule));
    }

    // ── Commit-K: yabancı-witness-reddi (latch+manuel-close) + manuel-close-dalları ──
    #[test]
    fn commit_k_foreign_authority_and_manual_close() {
        let mut a = Sender::new();
        let mut b = Sender::new();
        b.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        // Yabancı-witness latch: kimlik-guard mode-semantiğinden-ÖNCE → suppression + sayaç + kuyruk-boş.
        let wa = a.open_fec_block_authority();
        let r = b.fec_controller_latch(wa, None, 8, 4, 1, 64);
        assert_eq!(
            (r.planned_r, r.suppression),
            (0, Some(FecSuppressionReason::ForeignAuthority))
        );
        let d = b.diag_fec_actuation();
        assert_eq!(
            (d.foreign_authority_rejected_count, d.enqueued_r_total),
            (1, 0)
        );
        assert_eq!(d.last_latch, Some(r), "red-de-latch-kaydı-görünür");
        // Yabancı-witness manuel-close: typed-Err + state-değişmez.
        let wa2 = a.open_fec_block_authority();
        assert_eq!(
            b.close_fec_block_manual(wa2, 1, 2, 64),
            Err(FecAuthorityError::ForeignAuthority)
        );
        let d2 = b.diag_fec_actuation();
        assert_eq!(d2.foreign_authority_rejected_count, 2);
        assert_eq!(
            d2.enqueued_r_total, 0,
            "foreign-manuel-close: kuyruk-boş-kalır"
        );
        // Controller-witness manuel-close'a: NotInManualMode (typed).
        a.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        let wc = a.open_fec_block_authority();
        assert_eq!(
            a.close_fec_block_manual(wc, 1, 2, 64),
            Err(FecAuthorityError::NotInManualMode)
        );
        // Manual{true}-witness: r-kuyruklanır; Manual{false}-witness: Ok(0) (meşru, hata-değil).
        let mut m = Sender::new();
        m.set_manual_fec_enabled(true).unwrap();
        let w1 = m.open_fec_block_authority();
        assert_eq!(m.close_fec_block_manual(w1, 5, 3, 64), Ok(3));
        assert_eq!(m.diag_fec_actuation().enqueued_r_total, 3);
        m.set_manual_fec_enabled(false).unwrap();
        let w0 = m.open_fec_block_authority();
        assert_eq!(m.close_fec_block_manual(w0, 6, 3, 64), Ok(0));
        assert_eq!(
            m.diag_fec_actuation().enqueued_r_total,
            3,
            "false-witness: parity-yok"
        );
    }

    // ── Commit-K: legacy-enqueue izin-kapısı (üyelik=izin) + generation-monotonluğu ──
    #[test]
    #[allow(deprecated)] // deprecated-legacy-yolun-kapısını-test-eder
    fn commit_k_enqueue_permission_gate_and_generation() {
        let mut s = Sender::new(); // default Manual{false}
        s.enqueue_fec_parity(1, 0, 64); // izinsiz → kuyruğa-GİRMEZ (sayaçla-görünür-red)
        let d = s.diag_fec_actuation();
        assert_eq!(
            (d.enqueued_r_total, d.parity_enqueue_rejected_count),
            (0, 1)
        );
        s.set_manual_fec_enabled(true).unwrap();
        s.enqueue_fec_parity(1, 0, 64);
        assert_eq!(s.diag_fec_actuation().enqueued_r_total, 1);
        // Generation monoton (witness-uniqueness; saturating-belgeli).
        let g1 = s.open_fec_block_authority().generation();
        let g2 = s.open_fec_block_authority().generation();
        assert!(g2 > g1, "monoton-generation");
    }

    // ── Commit-N: token-denial ATTRIBUTION (blok/event/generation) + kesin-semantik-ayrım ──
    #[test]
    #[allow(deprecated)] // legacy-enqueue-reddi≠denial-dalını-da-test-eder
    fn commit_n_token_denial_attribution_and_semantics() {
        let mut s = Sender::new();
        s.set_manual_fec_enabled(true).unwrap();
        // (a) DENIAL-DEĞİL-yolları snapshot-YAZMAZ: legacy-enqueue-reddi + suppression'lar + clamp.
        s.set_manual_fec_enabled(false).unwrap();
        s.enqueue_fec_parity(9, 0, 64); // izinsiz-red
        assert_eq!(s.diag_token_denial(), (None, 0), "enqueue-reddi≠denial");
        s.set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap();
        s.on_event(Event::AppData(unit(0)), 0);
        let snap = s.controller_decision_snapshot().unwrap();
        let nr = ControllerDecisionSnapshot {
            no_rule: true,
            ..snap
        };
        let aa = s.open_fec_block_authority();
        let _ = s.fec_controller_latch(aa, Some(nr), 8, 4, 30, 64);
        let em = ControllerDecisionSnapshot {
            emergency: true,
            ratio_permille: 500,
            ..snap
        };
        let ab = s.open_fec_block_authority();
        let _ = s.fec_controller_latch(ab, Some(em), 8, 4, 31, 64);
        let clamp = ControllerDecisionSnapshot {
            ratio_permille: 1000,
            ..snap
        }; // computed-8→clamp-4
        let ac = s.open_fec_block_authority();
        let l = s.fec_controller_latch(ac, Some(clamp), 8, 4, 32, 100_000);
        assert_eq!((l.computed_r, l.planned_r), (8, 4), "clamp-oldu");
        assert_eq!(
            s.diag_token_denial().0,
            None,
            "no-rule/emergency/clamp≠denial (emisyon-denenmedi)"
        );
        // (b) GERÇEK-denial: 100kB-item try_send-red → snapshot TAM-attribution.
        let out = s.on_event(Event::TimerFired(Timer::Pto), MS);
        let _ = out;
        let (d, n) = s.diag_token_denial();
        let d = d.expect("gerçek-token-denial-kaydedildi");
        assert!(n >= 1);
        assert_eq!(d.block_id, 32, "denied-item-bloğu");
        assert_eq!(
            d.authority_mode,
            FecActuationMode::Controller,
            "staging-otoritesi"
        );
        assert!(
            d.authority_generation.is_some(),
            "otorite-bağlı-staging: gen-attribution"
        );
        assert_eq!(d.planned_r, Some(4), "bloğun-planned_r-tag'i");
        assert_eq!(d.parity_size_bytes, 100_000);
        assert!(
            d.token_balance_before < 100_000,
            "bakiye<maliyet (denial-tutarlı)"
        );
        assert_eq!(d.pending_for_block, 4, "aynı-blok-4-item-kuyrukta");
        assert_eq!(d.pending_parity_total, 4);
        assert_eq!(
            d.denial_event_index,
            s.diag_fusion_input_snapshot().unwrap().event_index,
            "planlama-anı-event-attribution"
        );
        // (c) RECOVERED-cancellation DENIAL-SAYILMAZ: blok-32-cancel → sayaç/snapshot-DEĞİŞMEZ.
        let n_before = s.diag_token_denial().1;
        s.on_event(
            Event::RecoveredControl(RecoveredInfo {
                units: vec![RecoveredUnit {
                    unit_id: 3200,
                    block_id: 32,
                    esi: 0,
                }],
            }),
            2 * MS,
        );
        assert_eq!(
            s.diag_token_denial().1,
            n_before,
            "cancel≠denial (sayaç-sabit)"
        );
        assert_eq!(
            s.diag_fec_actuation().canceled_r_total,
            4,
            "4-item-cancel-edildi"
        );
    }

    // ── Commit-N: interleaved-blok-denial-ayrıştırması + TAM-overwrite (carryover-yok) + saturation ──
    #[test]
    #[allow(deprecated)] // legacy-enqueue None-attribution-dalını-da-test-eder
    fn commit_n_interleaved_denial_overwrite_and_saturation() {
        let mut s = Sender::new();
        s.set_manual_fec_enabled(true).unwrap();
        // Blok-40 (otorite-bağlı; küçük-64B → emit-edilebilir) + blok-41 (legacy; 100kB → hep-denied).
        let w = s.open_fec_block_authority();
        let g40 = w.generation();
        assert_eq!(s.close_fec_block_manual(w, 40, 1, 64), Ok(1));
        s.enqueue_fec_parity(41, 0, 100_000); // legacy: gen=None, planned=None
                                              // İlk-tick'ler: 64B-emit-olana-kadar (önde) — sonra-41-öne-geçer-ve-denial-41'e-atfedilir.
        let mut denial_41_seen = false;
        for t in 1..400u64 {
            s.on_event(Event::TimerFired(Timer::Pto), t * 10 * MS);
            if let (Some(d), _) = s.diag_token_denial() {
                if d.block_id == 41 {
                    denial_41_seen = true;
                    assert_eq!(
                        d.authority_generation, None,
                        "legacy-item: gen-attribution-None"
                    );
                    assert_eq!(d.planned_r, None, "legacy-item: planned-None");
                    assert_eq!(d.authority_mode, FecActuationMode::Manual { enabled: true });
                    break;
                }
                // 41'den-önce-görülen-denial-varsa-40'a-ait-olmalı (interleaved-ayrışma):
                assert_eq!(d.block_id, 40);
                assert_eq!(
                    d.authority_generation,
                    Some(g40),
                    "blok-40-denial: witness-gen"
                );
            }
        }
        assert!(denial_41_seen, "blok-41-denial-atfedildi");
        assert_eq!(
            s.diag_fec_actuation().emitted_r_total,
            1,
            "blok-40-emit-oldu (ayrışma-kanıtı)"
        );
        // TAM-overwrite/carryover-yok: son-snapshot-41'in (Some-gen-carryover-YOK — None-üzerine-yazıldı).
        let (d, _) = s.diag_token_denial();
        assert_eq!(
            d.unwrap().authority_generation,
            None,
            "overwrite: önceki-Some(gen)-taşınmadı"
        );
        // Saturation: denial-sayacı-MAX'ta-kalır.
        s.token_denial_count_total = u64::MAX - 1;
        s.on_event(Event::TimerFired(Timer::Pto), 5000 * MS);
        assert_eq!(s.diag_token_denial().1, u64::MAX);
        s.on_event(Event::TimerFired(Timer::Pto), 5001 * MS);
        assert_eq!(s.diag_token_denial().1, u64::MAX, "saturating: wrap-yok");
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

    // ── DR-ARQ-PRODUCT-TAIL-1 integration (default-OFF exactness + ON path) ──
    fn tail_cfg_on() -> TailRecoveryConfig {
        TailRecoveryConfig {
            enabled: true,
            min_pto_ns: MS,
            max_pto_ns: 1000 * MS,
            initial_pto_ns: MS,
            max_backoff_pto_ns: 100_000 * MS,
            max_ack_delay_ns: 0,
            max_tail_probes: 2,
        }
    }

    #[test]
    fn tail_probe_off_never_probes() {
        // default OFF: outstanding tail + repeated PTO ⇒ 0 tail probes (INV-CS-4 baseline preserved).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(99)), 20 * MS); // unacked tail
        for t in 1..=20u64 {
            s.on_event(Event::TimerFired(Timer::Pto), (20 + t * 100) * MS);
        }
        assert_eq!(s.tail_probe_diag().0, 0, "OFF must never probe");
    }

    #[test]
    fn tail_probe_on_no_loss_zero_probe() {
        // ON but everything acked (inflight drains) ⇒ disarmed ⇒ 0 probes, 0 added bytes.
        let mut s = Sender::new();
        s.set_tail_recovery_config(tail_cfg_on());
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(1)), 20 * MS);
        s.on_event(ack(&[1]), 30 * MS); // acked → inflight drains
        for t in 1..=20u64 {
            s.on_event(Event::TimerFired(Timer::Pto), (30 + t * 100) * MS);
        }
        let (probes, bytes, _, _) = s.tail_probe_diag();
        assert_eq!(probes, 0, "no-loss ON ⇒ 0 probe");
        assert_eq!(bytes, 0, "no-loss ON ⇒ 0 added probe bytes");
    }

    #[test]
    fn tail_probe_mid_stream_loss_no_same_tick_double_plan() {
        // Mid-stream loss (later packets exist) IS detectable by the existing packet-threshold ARQ: the lost seq
        // moves to pending_lost (out of inflight) and RECOVER_ARQ retransmits it. The probe targets the oldest
        // INFLIGHT seq, which by construction EXCLUDES anything just moved to pending_lost (detect_time_losses runs
        // before the probe arm in the same Pto tick). §1b/§3 invariant: within ONE tick, the normal ARQ and the
        // probe never BOTH retransmit the same unit. (The probe MAY later recover the retransmission if it in turn
        // becomes an unacked terminal tail — that is correct bounded recovery, not a duplicate.)
        let mut s = Sender::new();
        s.set_tail_recovery_config(tail_cfg_on());
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS); // srtt + BtlBw/tokens.
        for i in 1..=5u64 {
            s.on_event(Event::AppData(small(i)), (10 + i) * MS);
        }
        let mut retx = 0usize;
        let mut ticks: Vec<Vec<TransmitDecision>> = Vec::new();
        ticks.push(s.on_event(ack(&[2, 3, 4, 5]), 30 * MS)); // seq 1 lost → normal ARQ recovers.
        for t in 1..=20u64 {
            ticks.push(s.on_event(Event::TimerFired(Timer::Pto), (30 + t * 100) * MS));
        }
        for out in &ticks {
            let mut retx_unit_ids: Vec<u64> = out
                .iter()
                .filter(|d| d.kind == TransmitKind::Retransmit)
                .map(|d| d.unit_id)
                .collect();
            retx += retx_unit_ids.len();
            let before = retx_unit_ids.len();
            retx_unit_ids.sort_unstable();
            retx_unit_ids.dedup();
            assert_eq!(
                retx_unit_ids.len(),
                before,
                "no unit retransmitted twice in a single tick (no ARQ+probe double-plan)"
            );
        }
        assert!(retx >= 1, "mid-stream loss recovered (retx={retx})");
        assert!(
            s.tail_probe_diag().0 <= 2,
            "any follow-on tail probe stays bounded by max_tail_probes"
        );
    }

    #[test]
    fn tail_probe_terminal_tail_bounded_recovery() {
        // Terminal tail = the LAST outstanding source unit, nothing sent after it. The existing ARQ is BLIND to it
        // (no later packet ⇒ no packet-threshold; time-threshold has no later-acked reference either — this is the
        // classic tail-loss hole INV-CS-4 left open). PROVEN: with tail_probe OFF this seq stays stuck in inflight,
        // 0 retransmits. With the bounded probe ON, at most max_tail_probes probes recover it — no more.
        // Warm-up first so srtt + BtlBw/tokens exist (a probe respects pacing; a cold PreConverge floor is
        // deliberately too token-starved to probe — that is correct pacing behaviour, not a mechanism failure).
        let recover = |on: bool| {
            let mut s = Sender::new();
            if on {
                s.set_tail_recovery_config(tail_cfg_on());
            }
            s.on_event(Event::AppData(unit(200)), 0);
            s.on_event(ack(&[0]), 10 * MS); // srtt + BtlBw + leaves PreConverge.
            s.on_event(Event::AppData(small(99)), 20 * MS); // the terminal tail (never acked).
            let mut probe_retx = 0usize;
            for t in 1..=40u64 {
                probe_retx += s
                    .on_event(Event::TimerFired(Timer::Pto), (20 + t * 100) * MS)
                    .iter()
                    .filter(|d| d.kind == TransmitKind::Retransmit)
                    .count();
            }
            (probe_retx, s.tail_probe_diag(), s.pending_lost.len())
        };
        // OFF: normal ARQ is blind to the pure tail → 0 retransmits (the hole).
        let (off_retx, off_diag, _) = recover(false);
        assert_eq!(
            off_retx, 0,
            "normal ARQ is blind to a pure tail (OFF ⇒ 0 retx)"
        );
        assert_eq!(off_diag.0, 0, "OFF ⇒ 0 probes");
        // ON: bounded probe recovers it (1..=max_tail_probes), each surfaced as a Retransmit decision.
        let (on_retx, on_diag, _) = recover(true);
        let (probes, bytes, spurious, _) = on_diag;
        assert!(probes >= 1, "tail probe ON ⇒ ≥1 probe (got {probes})");
        assert!(probes <= 2, "bounded by max_tail_probes (got {probes})");
        assert!(bytes >= 1, "probe emitted real bytes");
        assert_eq!(
            on_retx as u64, probes,
            "every probe surfaced as a Retransmit decision"
        );
        // no spurious accounting on a genuinely-lost tail (unit was never acked).
        assert_eq!(spurious, 0, "genuinely lost tail ⇒ not spurious");
    }

    #[test]
    fn tail_probe_token_unavailable_cap_not_consumed() {
        // §3 probe-due ≠ probe-sent: a cold PreConverge floor is deliberately token-starved (~1.5 B/tick), so a
        // 100 B probe is DUE every tick but can never be emitted. The attempt/cap must NOT be consumed on a due-but-
        // un-emitted probe — the target stays pending so a later token can still send it (bounded once, not spent).
        let mut s = Sender::new();
        s.set_tail_recovery_config(tail_cfg_on());
        s.on_event(Event::AppData(small(99)), 0); // cold start: no srtt, floor pacing, no ACK.
        for t in 1..=20u64 {
            s.on_event(Event::TimerFired(Timer::Pto), t * 50 * MS);
        }
        assert_eq!(
            s.tail_probe_diag().0,
            0,
            "token-starved ⇒ 0 emissions ⇒ cap not consumed"
        );
        assert!(
            s.tail_probe_pending.is_some(),
            "probe stays pending (due) while token-starved — not dropped, not spent"
        );
        assert_eq!(
            s.inflight.keys().next().copied(),
            Some(0),
            "the outstanding tail is still un-recovered (held for a future token)"
        );
    }

    #[test]
    fn tail_probe_ack_race_cancels_without_consuming_cap() {
        // §3 re-verify-at-emission: a probe is DUE and pending (token-starved) when a real ACK drains the target
        // first. At emission the target is no longer inflight ⇒ the probe CANCELS (attempt NOT consumed, no wire
        // bytes), and the pending slot clears. No spurious retransmission of already-delivered data.
        let mut s = Sender::new();
        s.set_tail_recovery_config(tail_cfg_on());
        s.on_event(Event::AppData(small(99)), 0);
        for t in 1..=6u64 {
            s.on_event(Event::TimerFired(Timer::Pto), t * 50 * MS);
        }
        assert_eq!(s.tail_probe_pending, Some(0), "probe due+pending pre-ACK");
        // the ACK for seq 0 arrives before the probe could ever get a token → target drains.
        let out = s.on_event(ack(&[0]), 400 * MS);
        assert_eq!(
            out.iter()
                .filter(|d| d.kind == TransmitKind::Retransmit)
                .count(),
            0,
            "ACK-race ⇒ no probe retransmit emitted"
        );
        assert_eq!(s.tail_probe_diag().0, 0, "ACK-race ⇒ cap not consumed");
        assert_eq!(s.tail_probe_pending, None, "cancelled probe slot cleared");
        assert!(
            s.inflight.is_empty(),
            "target delivered, nothing outstanding"
        );
    }

    #[test]
    fn tail_probe_ack_is_karn_excluded_from_rtt() {
        // Karn: an ACK of a PROBE (origin = Retransmit) must NEVER feed the RTT estimator (send-time ambiguity).
        // The sender samples RTT only for origin == Data (sender.rs); probes emit as Retransmit, so acking them
        // leaves srtt untouched. Proven by capturing srtt after the probe fires, then acking the probe seqs.
        let mut s = Sender::new();
        s.set_tail_recovery_config(tail_cfg_on());
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS); // the ONLY Data RTT sample → srtt fixed here.
        s.on_event(Event::AppData(small(99)), 20 * MS);
        for t in 1..=40u64 {
            s.on_event(Event::TimerFired(Timer::Pto), (20 + t * 100) * MS);
        }
        assert!(s.tail_probe_diag().0 >= 1, "probe fired (precondition)");
        let srtt_before = s.rtt.srtt();
        // every currently-outstanding seq is a probe retransmit (origin = Retransmit).
        let probe_seqs: Vec<u64> = s.inflight.keys().copied().collect();
        assert!(!probe_seqs.is_empty(), "probe retransmits outstanding");
        s.on_event(
            Event::AckReceived(AckInfo {
                acked_seqs: probe_seqs,
                ack_delay: Duration::ZERO,
            }),
            5_000 * MS,
        );
        assert_eq!(
            s.rtt.srtt(),
            srtt_before,
            "probe ACK is Karn-excluded — srtt must not move"
        );
    }

    #[test]
    fn tail_probe_determinism_two_senders() {
        // Deterministic (RNG-free): two independent senders driven by the identical event sequence produce
        // byte-identical outputs AND identical probe accounting.
        let drive = || {
            let mut s = Sender::new();
            s.set_tail_recovery_config(tail_cfg_on());
            let mut trace: Vec<(TransmitKind, u64, u64)> = Vec::new();
            let push = |out: Vec<TransmitDecision>, trace: &mut Vec<(TransmitKind, u64, u64)>| {
                for d in out {
                    trace.push((d.kind, d.seq, d.size));
                }
            };
            push(s.on_event(Event::AppData(unit(200)), 0), &mut trace);
            push(s.on_event(ack(&[0]), 10 * MS), &mut trace);
            push(s.on_event(Event::AppData(small(99)), 20 * MS), &mut trace);
            for t in 1..=40u64 {
                push(
                    s.on_event(Event::TimerFired(Timer::Pto), (20 + t * 100) * MS),
                    &mut trace,
                );
            }
            (trace, s.tail_probe_diag())
        };
        let a = drive();
        let b = drive();
        assert_eq!(a.0, b.0, "identical event stream ⇒ identical output trace");
        assert_eq!(a.1, b.1, "identical probe accounting");
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
        //
        // [DR-RATE-RAMP-2 SUPERSESSION — INV-RATE-1-STARTUP-FLOOR-SUPERSESSION.md] This golden is
        // PRESERVED VERBATIM (name, fixture, both exact equalities) and RE-SCOPED to the authoritative
        // LEGACY FLOOR-INACTIVE path: with the bootstrap startup floor disabled, the old rate law must
        // still hold EXACTLY. The active-floor contract is pinned separately by
        // `inv_rate_1_active_floor_loss_does_not_reduce_rate` (no assertion was weakened).
        let mut s = Sender::new();
        s.set_startup_pacing_init(StartupPacingConfig { enabled: false });
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

    // ── DR-RATE-RAMP-2 §1A.2: binding contract for the bootstrap startup floor ──────────────
    // Supersession record: reports/fs-cal-2/v3.5/INV-RATE-1-STARTUP-FLOOR-SUPERSESSION.md

    fn floor_on() -> Sender {
        let mut s = Sender::new();
        s.set_startup_pacing_init(StartupPacingConfig { enabled: true });
        s
    }

    /// Drive the golden fixture up to "Startup ramp in progress, emissions done".
    fn ramping(s: &mut Sender) {
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 500 * MS); // btlbw≈2400; PreConverge→Startup
        for i in 1..=8u64 {
            s.on_event(Event::AppData(unit(i)), (500 + i) * MS);
        }
        for t in [700u64, 900, 1_100] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        }
    }

    #[test]
    fn inv_rate_1_active_floor_loss_does_not_reduce_rate() {
        // SUPERSEDING LOCK: with the floor ACTIVE the safety purpose is re-asserted with EXACT equality
        // against the safe composition max(phase_rate, floor) — not a loose `>=`.
        let mut s = floor_on();
        ramping(&mut s);
        assert!(s.base_is_startup, "kurulum: ramp sürüyor");
        let floor = s.startup_pacing_floor_bps();
        assert!(floor > 0, "floor must be armed on the PreConverge exit");
        let btlbw = s.btlbw.btlbw();
        let expect_before = crate::rate::Gain::STARTUP.apply(btlbw).max(floor);
        let rate_before = s.pacer.rate();
        assert_eq!(
            rate_before, expect_before,
            "kurulum: max(phase_rate, floor) exact"
        );
        // loss declaration
        s.on_event(ack(&[2, 3, 4]), 1_200 * MS);
        assert_eq!(
            s.state(),
            SenderState::RecoverArq,
            "kayıp-ilanı → recovery-overlay"
        );
        let btlbw_after = s.btlbw.btlbw();
        let floor_after = s.startup_pacing_floor_bps();
        let expect_after = crate::rate::Gain::STARTUP
            .apply(btlbw_after)
            .max(floor_after.min(s.cwnd_over_srtt()));
        assert_eq!(
            s.pacer.rate(),
            expect_after,
            "KİLİT: kayıp-ilanı faz-gain'ini DEĞİŞTİRMEZ (rate = max(2.885×btlbw, floor) KALIR)"
        );
        assert!(
            s.pacer.rate() >= rate_before.min(expect_after),
            "loss must never CUT the rate"
        );
        // recovery exit still returns to the Startup BASE (ramp not killed)
        s.on_event(ack(&[1]), 1_300 * MS);
        assert_eq!(
            s.state(),
            SenderState::Startup,
            "recovery-çıkışı base'e döner"
        );
    }

    #[test]
    fn startup_floor_arms_only_on_preconverge_exit() {
        let mut s = floor_on();
        s.on_event(Event::AppData(unit(200)), 0);
        assert_eq!(s.state(), SenderState::PreConverge);
        assert_eq!(
            s.startup_pacing_floor_bps(),
            0,
            "not armed while in PreConverge"
        );
        s.on_event(ack(&[0]), 500 * MS); // the ONLY arming transition
        assert!(
            s.startup_pacing_floor_bps() > 0,
            "armed exactly on PreConverge→Startup"
        );
    }

    #[test]
    fn startup_floor_not_armed_or_raised_by_loss() {
        // A loss declaration alone must neither arm nor raise the floor.
        let mut s = floor_on();
        ramping(&mut s);
        let before = s.startup_pacing_floor_bps();
        s.on_event(ack(&[2, 3, 4]), 1_200 * MS); // packet-threshold loss declaration
        assert_eq!(s.state(), SenderState::RecoverArq);
        assert!(
            s.startup_pacing_floor_bps() <= before,
            "loss must never RAISE the floor ({before} -> {})",
            s.startup_pacing_floor_bps()
        );
    }

    #[test]
    fn startup_floor_never_exceeds_cwnd_over_srtt() {
        // The structural HARD ceiling has the last word at EVERY application, including after loss.
        let mut s = floor_on();
        ramping(&mut s);
        for t in [1_200u64, 1_300, 1_400] {
            s.on_event(ack(&[2, 3, 4]), t * MS);
            let ceiling = s.cwnd_over_srtt();
            assert!(
                s.pacer.rate() <= ceiling.max(crate::rate::Gain::STARTUP.apply(s.btlbw.btlbw())),
                "rate must never exceed the composition of the hard ceiling and the phase rate"
            );
            assert!(
                s.startup_pacing_floor_bps().min(ceiling) <= ceiling,
                "effective floor is clamped to the CURRENT cwnd/srtt"
            );
        }
    }

    #[test]
    fn startup_floor_writes_no_synthetic_btlbw_samples() {
        // The floor must not inject anything into the BtlBw max-filter: the sample set is identical
        // to the floor-inactive run.
        let mut a = Sender::new();
        a.set_startup_pacing_init(StartupPacingConfig { enabled: false });
        let mut b = floor_on();
        a.on_event(Event::AppData(unit(200)), 0);
        b.on_event(Event::AppData(unit(200)), 0);
        a.on_event(ack(&[0]), 500 * MS);
        b.on_event(ack(&[0]), 500 * MS);
        assert_eq!(
            a.diag_btlbw_samples().len(),
            b.diag_btlbw_samples().len(),
            "floor must not add samples to the max-filter"
        );
        assert_eq!(
            a.btlbw.btlbw(),
            b.btlbw.btlbw(),
            "floor must not alter the BtlBw estimate itself"
        );
    }

    #[test]
    fn startup_floor_release_is_deterministic_and_permanent() {
        let mut s = floor_on();
        ramping(&mut s);
        assert!(s.startup_pacing_floor_bps() > 0, "armed");
        // RELEASE BRANCH 1 (deterministic): the Startup/plateau exit condition fires.
        // (RELEASE BRANCH 2 — "the natural rate catches up" — is pinned end-to-end on a real flow by
        //  kama-integration `startup_pacing_floor_is_bootstrap_only_and_released`.)
        let t = 1_400u64;
        assert!(
            s.base_is_startup,
            "ramp is in progress while the floor is armed"
        );
        // Fire the EXISTING Startup exit condition: plateau ⇒ update_state moves Startup → Drain.
        s.plateau_reached = true;
        s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        assert_eq!(
            s.state(),
            SenderState::Drain,
            "startup ramp must have ended (post-ramp phase)"
        );
        assert_eq!(
            s.startup_pacing_floor_bps(),
            0,
            "floor must be released deterministically"
        );
        // and it may NEVER re-arm (no Plateau/ProbeBW re-entry)
        for i in 0..50u64 {
            s.on_event(Event::TimerFired(Timer::Pto), (t + 100 + i * 10) * MS);
            assert_eq!(s.startup_pacing_floor_bps(), 0, "floor must never re-arm");
        }
    }

    // ── DR-CS-3 §6: shipping bootstrap RTT probe — sender-side security/budget contract ──────
    // Contract: reports/dr-cs-3/DR-CS-3-BOOTSTRAP-RTT-CONTRACT.md

    fn bs_on() -> Sender {
        let mut s = Sender::new();
        s.set_bootstrap_rtt(BootstrapRttConfig {
            enabled: true,
            fires_ns: None,
        });
        s
    }

    /// PreConverge flow whose DATA is never ACKed: the ladder arms and steps come due.
    fn bs_stalled(s: &mut Sender, until_s: u64) -> Vec<TransmitDecision> {
        let mut all = Vec::new();
        for i in 1..=384u64 {
            all.extend(s.on_event(
                Event::AppData(OpaqueUnit {
                    id: 1000 + i,
                    size: 101,
                }),
                0,
            ));
        }
        for t in 1..=until_s {
            all.extend(s.on_event(Event::TimerFired(Timer::Pto), t * 1_000 * MS));
        }
        all
    }

    fn probes(v: &[TransmitDecision]) -> Vec<&TransmitDecision> {
        v.iter()
            .filter(|d| d.kind == TransmitKind::CsProbe)
            .collect()
    }

    #[test]
    fn bootstrap_off_is_inert() {
        let mut s = Sender::new(); // default OFF
        let out = bs_stalled(&mut s, 60);
        // Assert on the DR-CS-3 counters, not on raw probe presence: in an
        // `experimental-cold-start` build the LEGACY experimental ladder is a different
        // implementation of the same protocol and still runs when DR-CS-3 is OFF.
        assert_eq!(s.bootstrap_rtt_diag().arm, 0, "OFF must never arm");
        assert_eq!(
            s.bootstrap_rtt_diag().probe_emitted,
            0,
            "OFF must never emit"
        );
        assert_eq!(s.bootstrap_budget_remaining(), BS_TX_HARD_BUDGET);
        #[cfg(not(feature = "experimental-cold-start"))]
        assert!(
            probes(&out).is_empty(),
            "shipping build: OFF emits no probe at all"
        );
        let _ = out;
    }

    #[test]
    fn bootstrap_ladder_is_edge_only_and_bounded() {
        let mut s = bs_on();
        let out = bs_stalled(&mut s, 60);
        let d = s.bootstrap_rtt_diag();
        let p = probes(&out);
        assert_eq!(d.arm, 1, "armed exactly once");
        assert!(
            p.len() <= BS_FIRES_NS.len(),
            "at most one probe per ladder step"
        );
        assert_eq!(d.probe_emitted as usize, p.len());
        // poll/mint separation: the PTO tick fired 60 times, the ladder minted at most 4
        assert!(
            d.step_edge <= BS_FIRES_NS.len() as u64,
            "edge count is bounded by the ladder"
        );
        // every probe carries a DISTINCT monotonic generation
        let mut seqs: Vec<u64> = p.iter().map(|d| d.seq).collect();
        let n = seqs.len();
        seqs.sort_unstable();
        seqs.dedup();
        assert_eq!(seqs.len(), n, "probe_seq generations must be unique");
    }

    #[test]
    fn bootstrap_sender_budget_never_exceeds_200_bytes() {
        let mut s = bs_on();
        let out = bs_stalled(&mut s, 120); // far past the last ladder step
        let bytes: u64 = probes(&out).iter().map(|d| d.size).sum();
        assert!(
            bytes <= BS_TX_HARD_BUDGET,
            "sender probe wire {bytes} B exceeds the 200 B budget"
        );
        assert_eq!(s.bootstrap_rtt_diag().probe_wire_bytes, bytes);
        assert_eq!(s.bootstrap_rtt_diag().budget_violation, 0);
        assert!(bytes <= 4 * 44, "4 x PROBE_WIRE_EXACT worst case");
    }

    /// Scoped to the SHIPPING build: under `experimental-cold-start` the OFF arm runs the legacy
    /// ladder, which DOES consume DATA tokens (`pacer.try_send`), so its DATA prefix is not a valid
    /// baseline for an additive comparison. DR-CS-3 targets the default build.
    #[cfg(not(feature = "experimental-cold-start"))]
    #[test]
    fn bootstrap_probe_is_additive_not_paced() {
        // The probe must NOT consume a DATA token: with the ladder ON, the DATA emissions in the
        // same window are IDENTICAL to the OFF baseline (no displacement), and only probes are added.
        let mut off = Sender::new();
        let mut on = bs_on();
        let a = bs_stalled(&mut off, 60);
        let b = bs_stalled(&mut on, 60);
        let data_a: Vec<u64> = a
            .iter()
            .filter(|d| d.kind == TransmitKind::Data)
            .map(|d| d.unit_id)
            .collect();
        let data_b: Vec<u64> = b
            .iter()
            .filter(|d| d.kind == TransmitKind::Data)
            .map(|d| d.unit_id)
            .collect();
        assert_eq!(
            data_a, data_b,
            "no DATA displacement: source prefix must be identical"
        );
        assert!(
            !probes(&b).is_empty(),
            "the treatment must actually add probes"
        );
    }

    #[test]
    fn bootstrap_unsolicited_echo_yields_no_rtt() {
        let mut s = bs_on();
        bs_stalled(&mut s, 2); // armed, but no probe is due yet at t=2s
        s.on_event(Event::CsProbeEcho { probe_seq: 999 }, 2_500 * MS);
        assert!(
            s.rtt.srtt().is_none(),
            "unsolicited echo must not establish srtt"
        );
        assert_eq!(s.bootstrap_rtt_diag().echo_match_valid, 0);
        assert!(s.bootstrap_rtt_diag().echo_match_rejected >= 1);
    }

    #[test]
    fn bootstrap_wrong_generation_echo_yields_no_rtt() {
        let mut s = bs_on();
        let out = bs_stalled(&mut s, 5); // step 0 (3 s) has fired
        let p = probes(&out);
        assert!(!p.is_empty(), "a probe must have been emitted");
        let good = p[0].seq as u16;
        s.on_event(
            Event::CsProbeEcho {
                probe_seq: good.wrapping_add(7),
            },
            6_000 * MS,
        );
        assert!(
            s.rtt.srtt().is_none(),
            "wrong generation must not establish srtt"
        );
        assert_eq!(s.bootstrap_rtt_diag().echo_match_valid, 0);
    }

    #[test]
    fn bootstrap_valid_echo_yields_rtt_from_local_timestamps_only() {
        let mut s = bs_on();
        let out = bs_stalled(&mut s, 5);
        let p = probes(&out);
        let seq = p[0].seq as u16;
        assert!(s.rtt.srtt().is_none(), "no srtt before the echo");
        // echo arrives 1 s after the probe was emitted at t = 3 s
        s.on_event(Event::CsProbeEcho { probe_seq: seq }, 4_000 * MS);
        let d = s.bootstrap_rtt_diag();
        assert_eq!(d.echo_match_valid, 1);
        assert!(s.rtt.srtt().is_some(), "valid echo must establish srtt");
        assert_eq!(
            d.rtt_sample_ns,
            1_000 * MS,
            "RTT = local arrival - local send"
        );
        assert_eq!(d.cancelled, 1, "the first valid sample cancels the ladder");
    }

    #[test]
    fn bootstrap_duplicate_echo_yields_at_most_one_sample() {
        let mut s = bs_on();
        let out = bs_stalled(&mut s, 5);
        let seq = probes(&out)[0].seq as u16;
        s.on_event(Event::CsProbeEcho { probe_seq: seq }, 4_000 * MS);
        s.on_event(Event::CsProbeEcho { probe_seq: seq }, 4_500 * MS);
        s.on_event(Event::CsProbeEcho { probe_seq: seq }, 5_000 * MS);
        assert_eq!(
            s.bootstrap_rtt_diag().echo_match_valid,
            1,
            "single consumption"
        );
    }

    #[test]
    fn bootstrap_no_probe_after_success() {
        let mut s = bs_on();
        let out1 = bs_stalled(&mut s, 5);
        let seq = probes(&out1)[0].seq as u16;
        s.on_event(Event::CsProbeEcho { probe_seq: seq }, 4_000 * MS);
        let before = s.bootstrap_rtt_diag().probe_emitted;
        // drive well past every remaining ladder step
        let mut later = Vec::new();
        for t in 6..=60u64 {
            later.extend(s.on_event(Event::TimerFired(Timer::Pto), t * 1_000 * MS));
        }
        assert!(
            probes(&later).is_empty(),
            "no probe may be emitted after success"
        );
        assert_eq!(s.bootstrap_rtt_diag().probe_emitted, before);
        assert_eq!(s.bootstrap_rtt_diag().probe_after_success_violation, 0);
    }

    #[test]
    fn bootstrap_does_not_relax_karn_or_touch_arq_timing() {
        // A duplicate/retransmit ACK must still yield no RTT sample (Karn), with the ladder ON.
        let mut on = bs_on();
        let mut off = Sender::new();
        for s in [&mut on, &mut off] {
            s.on_event(Event::AppData(unit(200)), 0);
        }
        // identical ARQ-relevant driving; compare loss/PTO diagnostics
        for t in [700u64, 900, 1_100] {
            on.on_event(Event::TimerFired(Timer::Pto), t * MS);
            off.on_event(Event::TimerFired(Timer::Pto), t * MS);
        }
        assert_eq!(
            on.diag_loss(),
            off.diag_loss(),
            "ARQ loss-declaration accounting must be unchanged before any echo"
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
