//! P1 — transport çalışma-zamanı kabuğu (PRODUCT-RUNTIME item 2/3/7 + 8/9 kapıları).
//!
//! Tasarım otoritesi `docs/design/P1-RUNTIME-SHELL.md` (D1-D10). Döngü iskeleti kampanya-kanıtlı
//! bench şeklidir (tek görev `select!`, PACE 1 ms → `Timer::Pto` pompası, RTPROP 200 ms, alıcıda
//! ACK 20 ms; taraf başına TEK `Instant`, motor saf u64-ns olay-güdümlü — time-model kuralı).
//! `next_wakeup_ns()`'e tek-kaynak-uyku YOK (D3, EK4-A2 tarihi): pompa idempotent + kanıtlı.
//!
//! SoC: motor yalnız `OpaqueUnit{id,size}` / `RxDatagram` metadata görür; datagram'lar
//! `kama_contracts::s3::PacketProtection`'dan mühürlenir/açılır (kripto AYRI düzlem). INV-RATE-1: gönderim
//! hatası = kanal kaybı; motor durumu emisyonu ZATEN işledi, geri itilmez, rate'e dokunulmaz.
//!
//! SINIRLAR (SR-AUDIT-001, bounds-before-alloc; motor `tx_queue`'yu sınırlamaz — kabuk sınırlar):
//! offer-kanalı bounded (typed `QueueFull`) · unit-store bounded (doluyken offer alınmaz — doğal
//! backpressure) · server peer tablosu cap'li fail-closed (yeni-peer typed-drop + sayaç; aktif peer
//! ASLA sessiz-evict edilmez, boşalma idle-emeklilikle) · teslim kanalı bounded (`try_send`,
//! taşan teslim sayaçla raporlanır — app drenaj sözleşmesi).
//! [K4G-01b] Server eş yuvası `(adres, SessionHandle)` ile anahtarlıdır; tavan YUVA sayısıdır.
//! Boşalma idle-emeklilikle YA DA bağlı oturum oturum tablosundan çıkınca (kapanınca) olur —
//! böyle bir yuva zaten erişilemezdir (bkz. "BAĞLAM-ÖNCE DAĞITIM"), ikisi de sayılır.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, FecActuationMode, InitialWindowConfig, OpaqueUnit,
    RecoveredInfo, RecoveredUnit, SrLogicalRef, Timer, TransmitKind,
};
/// [BURST-FUSION] Motorun yapı-gate politikası — `RuntimeConfig` alanının tipi. Çağıran ayrıca
/// adaptive-transport'a bağımlı olmasın diye buradan yeniden dışa verilir.
pub use adaptive_transport::sender::{
    FecArbiterReason, FecLossRiskConfig, FecStructurePolicy, LossConfirmHorizon, PersistMode,
    TsSample, ARBITER_REASON_KINDS,
};
use kama_engine::{plan_param_bootstrap, ParamState};
use kama_fec_codec::{
    by_id, FecCodec, FecProfile, ReceivedSymbol, RepairSymbol, RsSimdCodec, SourceSymbol,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::param::ParamFields;
use kama_transport_schema::prefix::{SourcePrefix, SOURCE_FLAG_FINAL};
use kama_transport_schema::{inner_kind, SOURCE_PREFIX_LEN};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::wire::{
    decode_cs_echo, decode_cs_probe, decode_data, decode_data_frag, decode_fec, decode_param,
    decode_param_ack, decode_recovered, encode_cs_echo, encode_cs_probe, encode_data,
    encode_data_frag, encode_fec, encode_param, encode_param_ack, encode_recovered, FecFrameHeader,
    CS_PROBE_SUBTYPE, FRAME_CS_ECHO, FRAME_PARAM_ACK, FRAME_RECOVERED, HDR_DATA, HDR_DATA_FRAG,
    HDR_FEC, LEN_CS_PROBE,
};
use crate::ClientSession;
use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{Opened, PacketProtection, ProtectionContext};

/// Pacing pompası (bench PACE_TICK): her tick §13 token drain + §5 zaman-eşiği loss; idempotent,
/// yüksek frekans güvenli. EK4-A2 dersi gereği ESAS uyandırma kaynağı budur (D3).
const PACE_TICK: Duration = Duration::from_millis(1);
/// RTprop probe periyodu (§8.3) — `next_wakeup_ns` kaynağı DEĞİL, kaba tick şart.
const RTPROP_TICK: Duration = Duration::from_millis(200);
/// Alıcı ACK timer'ı (§2.1 ACK_TIMER_T) — ACK_FREQ_N'e ulaşmayan son DATA'yı da teyit eder.
const ACK_TICK: Duration = Duration::from_millis(20);
/// Server bakım tick'i: idle-peer emekliliği (item 3 yaşam-döngüsü).
const PATIENCE_TICK: Duration = Duration::from_secs(1);
/// Client recv tamponu (yalnız ACK yönü) / server recv tamponu (max UDP datagram).
const CLIENT_RECV_BUF: usize = 2048;
const SERVER_RECV_BUF: usize = 65536;
/// [D1a] PN → motor-seq çeviri tablosunun PN ufku. ACK_V2 `largest_seq` u16'dır: `acked_full` PN'i
/// bir u16 sarması içinde çözer, daha gerideki PN telden doğru geri kurulamaz — tutmak anlamsız
/// olurdu ve tablo sınırsız büyürdü (SR-AUDIT-001). Kayıp bir paritenin PN'i de burada budanır.
const PN_SEQ_WINDOW: u64 = 1 << 16;

/// Çalışma-zamanı konfigürasyonu. Tüm sınırlar açık; `Default` üretim-makul değerler.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Aynı anda outstanding (teslim-bekleyen) birim tavanı — unit-store sınırı.
    pub max_inflight_units: usize,
    /// App→transport offer kuyruğu (bounded mpsc).
    pub offer_queue: usize,
    /// Transport→app teslim kuyruğu (bounded mpsc; taşan teslim sayılır).
    pub delivery_queue: usize,
    /// **Birim (wire chunk)** payload tavanı — tek bir DATA çerçevesinin taşıdığı en çok byte.
    /// FEC açıkken `max_unit_bytes + SOURCE_PREFIX_LEN ≤ profile.e` olmak ZORUNDA (symbol-hizası).
    pub max_unit_bytes: usize,
    /// [S1B] Fragmenter — **AÇIK OPT-IN**. `None` (default) = fragmentasyon YOK: mesaj = birim,
    /// app tavanı `max_unit_bytes`, wire 0x00 DATA ile birebir aynı (mevcut tüm kurulumlar ve
    /// bankadaki ölçümler etkilenmez). `Some(n)` = fragmentasyon AÇIK, app mesaj tavanı `n`
    /// (bounds-before-alloc; `n ≥ max_unit_bytes` olmak zorunda): mesaj `max_unit_bytes`lik
    /// ardışık chunk'lara bölünür, her chunk kendi birim-id'sini/ARQ izlemesini/FEC symbol
    /// yuvasını alır, alıcı yeniden birleştirip TAM mesajı teslim eder.
    ///
    /// Eşitsizlikten TÜREMEZ: `max_unit_bytes`i küçülten bir config (ör. FEC profilinin symbol
    /// tavanına inmek) fragmentasyonu KAZAYLA açamaz — wire formatı değişikliği ancak açık
    /// niyetle olur.
    pub max_message_bytes: Option<usize>,
    /// Server eş-tablosu tavanı (fail-closed; SESSION_COUNT_CAP=64 hizası).
    /// [K4G-01b] Sayılan birim eş YUVASIDIR, anahtarı `(adres, SessionHandle)`: aynı adresten
    /// iki oturum iki yuva tutar (yenileme / supersession geçişinde geçici).
    pub max_peers: usize,
    /// Alıcı R-kotası (sessizlik re-ACK bütçesi).
    pub quota_r: u32,
    /// Shutdown drain süre-tavanı (motor birim terk etmez → drain CAP'siz sonlanmayabilir, D8).
    pub drain_cap: Duration,
    /// Server idle-peer emeklilik eşiği (item 3; SESSION_IDLE_HOLD hizası 105 s).
    /// [K4G-01b] Bağlı oturumu kapanan yuva bu eşiği BEKLEMEZ
    /// (bkz. [`ServerStats::retired_idle_peers`]).
    pub idle_hold: Duration,
    /// [IW] PreConverge ilk-uçuş penceresi (byte). **DEFAULT AÇIK** (`DEFAULT_BURST_CAP` = 2×MTU).
    ///
    /// Neden default: bu bir EŞİK SEÇİMİ değil, bir CLAMP HATASININ düzeltilmesidir.
    /// `TokenBucket::new` kovayı zaten `DEFAULT_BURST_CAP` kredisiyle DOLU başlatır; PreConverge'de
    /// pacing 31 B/s olduğundan `set_burst_cap` o krediyi `front_size`'a kırpıp ATIYORDU → ilk uçuş
    /// TEK paket. Buradaki değer yeni bir sabit değil, pacer'ın kendi başlangıç kredisidir.
    ///
    /// Neden önemli: PreConverge'den çıkış YALNIZ bir RTT örneğiyle olur ve örnek YALNIZ ORİJİNAL
    /// DATA emisyonlarından gelir (Karn) → ilk uçuştaki orijinal sayısı N, kuruluş olasılığını
    /// doğrudan belirler (`p_loss^N`). N=1 + %50 kayıp = yazı-tura.
    ///
    /// Ölçüm (netem 200 ms RTT / %50 kayıp, n=6/hücre, 2 hücre): sevk davranışı 6/12 çöküş
    /// (teslim < 50); ilk pencere açıkken 0/12. Sevk default'u ÖLÇÜLÜ BOZUKTU; düzeltmenin kanıtı
    /// n=6 diye bozuk default'u korumak yanlış risk takasıdır. Ön-kayıtlı doğrulama borç olarak
    /// kayıtlı. `0` = eski davranış (clamp geri gelir).
    pub initial_window_bytes: u64,
    /// [KURULUŞ] `bootstrap_rtt` açıkken merdiven seçimi. `false` (default) = sevk varsayılanı
    /// (3/9/21/45 s — BLACKOUT için kalibre edilmiştir, kuruluş için değil). `true` =
    /// kuruluş-hızlı (0.2/0.6/1.8/5.4 s; ilk basamak 2×`INITIAL_RTT`). Probe BÜTÇESİ değişmez
    /// (4 × 44 B ≤ 200 B) — merdiven yalnız aynı bütçeyi erken harcar, trafik ŞİŞİRMEZ.
    pub bootstrap_fast_ladder: bool,
    /// [DR-CS-3] Cold-start bootstrap-RTT merdiveni. `false` (default) = sevk davranışı: motor
    /// probe PLANLAMAZ ve oturum, ilk ACK gelene dek PreConverge'de kalır. `true` = motor
    /// `BS_FIRES_NS` merdiveninde (3/9/21/45 s, toplam ≤200 B) 44 B'lik CS-probe planlar; alıcı
    /// 43 B echo döner; gönderen RTT örneğini KENDİ yerel damgalarından türetip PreConverge'den
    /// çıkar. KALİBRE EDİLMEMİŞ — default flip ölçüm ister (DR-CS-3 §9 kilitli panel).
    pub bootstrap_rtt: bool,
    /// [BURST-FUSION] Yapı-gate'inin burst-dalı politikası (motor `FecStructurePolicy`).
    /// Default = sevk davranışı (`Q18Iid`: burst → parity yok). `BurstFusion` KALİBRE
    /// EDİLMEMİŞTİR — flip ölçüm ister; bkz. `adaptive_transport::sender::FecStructurePolicy`.
    pub fec_structure_policy: FecStructurePolicy,
    /// [S1] FEC-runtime. `None` = KAPALI (sevk default'u; flip S1-GATE kanıtına bağlı — R1
    /// disiplininin FEC karşılığı). `Some` = Controller-mode + yapı-gate (Q18-fix) + blok
    /// yaşam-döngüsü + repair emisyonu/kurtarımı. İKİ UÇTA AYNI kurulmalı (blok/esi ataması
    /// deterministik `unit_id`'den; PARAM-bootstrap bağlaması S2/B-hattı `kama_engine::param_state`).
    pub fec: Option<FecRuntimeConfig>,
    /// [DEADLINE-EKSENİ] Kalibre füzyon politikası. `None` = KAPALI (sevk default'u; yapı-gate
    /// koşar). `Some` = politika karar otoritesi (yapı-gate SUSAR — çift-otorite gözlemlenemez
    /// davranış üretir).
    ///
    /// `FusionPolicy::calibration_v2_deadline()` FEC-MARGINAL-R v1+v2'den TÜREDİ ve fail-safe'tir:
    /// app bütçe bildirmezse ArqOnly → wire byte-özdeş. Yani AÇMAK, bütçe bildirmeyen uygulamalar
    /// için davranışı değiştirmez; yalnız ölçülen kutuda parity açar.
    pub fusion_policy: Option<adaptive_transport::fusion_policy::FusionPolicy>,
    /// [ONAY UFKU] Kayıp-defteri `Suspected → Confirmed` ufku. Default = sevk davranışı
    /// (byte-özdeş). `ChannelDerived` bir ÖLÇÜM KOLUDUR, düzeltme değil — bkz.
    /// `reports/loss-ledger-confirm-defect/FINDING.md`.
    pub loss_confirm_horizon: LossConfirmHorizon,
    /// [FEC v1] Spekülatif FEC kayıp-riski lane'i. **Ürün default'u `FecLossRiskConfig::bounded_v1()`**
    /// (aday A 250-250-250 + persistence P2 + C4 guard'ı G1; bkz. `reports/fec-loss-risk-v1/closure`).
    /// `fec: None` iken (default) İNERT — oran hesaplanır, hiçbir aktüasyon yolu çalışmaz (tümü
    /// `fec.as_ref()`'e kapılı) → FEC'siz yol byte-özdeş. Tek tüketici FEC oranı kararıdır;
    /// CC/pacing/ARQ/kanonik kayıp dokunulmaz. Kapatmak için `FecLossRiskConfig::default()`.
    pub fec_loss_risk: FecLossRiskConfig,
    /// [HANDOFF ENSTRÜMANI] Zaman-serisi örnekleme periyodu (ns). `None` (default) = kapalı.
    /// Açıkken pace tick'lerinde en çok [`TS_CAP`] örnek toplanır (bounded; SR-AUDIT-001).
    /// Salt gözlem — motor durumuna dokunmaz.
    pub ts_sample_period_ns: Option<u64>,
    /// [FEC v1] Yapı-gate parity seviyesi (‰). **Ürün default'u `Some(250)`** (aday A'nın gate
    /// bileşeni; r-sweep: 500‰/r4 15–20 % kayıpta israflı). `None` = motor `fec_med` (500). Yalnız
    /// `fec: Some(..)` iken etkili.
    pub structure_gate_level_permille: Option<u32>,
    /// [K4G-05 / DUR-19] Client ucunun CANLI teşhis aynası ve yazıcısı ([`ClientLiveDiag`],
    /// [`ClientHandle::live_diag`]). **Ürün varsayılanı KAPALI** (`false`). Çalışma-zamanı
    /// bayrağıdır, Cargo feature'ı DEĞİL: ürün ve INT-02 düzeneği AYNI ikiliyi koşar, düzenek
    /// bayrağı açar. Kapalıyken görev ayna kurmaz ve yazmaz — ana döngü turu başındaki motor / tel
    /// okuması, durum değişikliği kaydı, oranlı kurtarma görüntüsü ([`RECOVERY_DIAG_PERIOD`]), ACK
    /// çerçevesi kaydı ve bunların kilidi ÇALIŞMAZ; erişimciler kilit almadan
    /// [`ClientLiveDiag::default`] döner. Açıkken görev aynayı yazar; hiçbir karar aynayı OKUMAZ
    /// (salt gözlem: motor kararları bayraktan bağımsızdır). Açık kolun maliyeti ölçülmedi. Yalnız
    /// client ucu okur; server ucu bu alanı yok sayar.
    pub live_diag: bool,
}

/// [HANDOFF ENSTRÜMANI] Zaman-serisi örnek tavanı (50 ms periyotta ≈ 205 s).
pub const TS_CAP: usize = 4096;

/// [S1] FEC-runtime parametreleri.
#[derive(Debug, Clone, Copy)]
pub struct FecRuntimeConfig {
    /// `kama_fec_codec::by_id` profil kimliği (k_shape/r_code_max/e buradan; S1: tam-blok
    /// k_actual = k_shape, known_zero = 0).
    pub profile_id: u8,
    /// `Some(r)` = STATİK-r modu (`Manual{enabled}` otorite + `close_fec_block_manual`):
    /// her blokta sabit r repair — kampanyaların STATIC_R kolu / operatör-forced FEC.
    /// `None` = Controller modu: yapı-gate (Q18-fix) kararı latch'e akar (adaptif).
    pub manual_r: Option<u16>,
}

/// [KAMA TRANSPORT v1 ÜRÜN PROFİLİ] — sevk yapılandırmasının **TEK doğruluk kaynağı**.
///
/// "Kama v1" adıyla ölçülebilecek tek konfigürasyon budur. `RuntimeConfig::default()` bu profile
/// devreder; benchmark kolları da elle `RuntimeConfig { .. }` kurmak yerine [`Self::runtime_config`]
/// ile başlayıp YALNIZ açık override uygular. Denetim bulgusu (2026-09-08,
/// `reports/transport-v1-audit/CONFIRMED-FINDINGS.md` B-5): bench profili elle kurduğu için ürün
/// default'ları (IW 2400, `bounded_v1`, gate 250) benchmark'a HİÇ ulaşmamış; "sevk" etiketli kol
/// ürünün sevk etmediği bir yapılandırmayı ölçmüştü.
///
/// **Açık mekanizmalar:** ARQ + selective retransmit · tail-recovery (`validated_v1`, [`ClientSession`]
/// kurar) · kuruluş probu (DR-EST-1) · ilk-uçuş penceresi 2400 B · adaptif FEC (profil 0x05, K=8,
/// E=1024) · yapı-gate (seviye 250‰) · spekülatif FEC kolu `bounded_v1` (A 250-250-250 + P2 + C4
/// guard'ı G1) · kayıp defteri (Suspected/Confirmed/Spurious).
///
/// **Kapalı (bilinçli):** DR-CS-3 bootstrap-RTT merdiveni (`bootstrap_rtt`, kalibre edilmemiş) ·
/// kalibre füzyon politikası (`fusion_policy`, opt-in ölçüm kapısı) · `BurstFusion` yapı-gate dalı
/// (kalibre edilmemiş) · fragmenter (`max_message_bytes`, açık opt-in — wire formatı görünür değişir) ·
/// kanal-türevli confirm ufku (ölçüm kolu) · canlı teşhis aynası (`live_diag`, salt gözlem; INT-02
/// düzeneği açar — DUR-19).
pub struct KamaV1Profile;

/// [FAZ-6 DONDURMA] Transport v1 ÜRÜN-PROFİL kimliği — benchmark manifestlerindeki `arm` değeri.
/// Yalnız `profile_delta` BOŞ olan koşu bu adı taşıyabilir; herhangi bir sapma `kama-custom`tur.
/// Bu sabit tek kaynaktır: `crates/bench` etiketi buradan okur, elle yazmaz.
/// [v1.1 · K4G-05b — transport-layer-tdd §12 AMENDMENT-12; sahip kararı CEVAP PAKETİ 5, 2026-10-16]
/// Yeniden-gönderim sahipliği motor davranışını değiştirdiği için kimlik `kama-v1.1-full`'dür
/// (`RuntimeConfig` alanları değişmez; tel, codec ve `tv_*` değişmez).
pub const TRANSPORT_V1_PROFILE_ID: &str = "kama-v1.1-full";
/// [FAZ-6 DONDURMA] Mimari/spec sürüm kimliği. Davranış değiştiren her değişiklik bu kimliği
/// `transport-v1.x` ya da `transport-v2` yapmak ZORUNDADIR (bkz. docs/transport-v1/EVOLUTION.md §9.5).
/// [v1.1 · K4G-05b — AMENDMENT-12] `transport-v1.1` (AMENDMENT-11'in beyan ettiği `transport-v1.x`).
pub const TRANSPORT_V1_ARCHITECTURE: &str = "transport-v1.1";

impl KamaV1Profile {
    /// Sevk FEC profili: `kama_fec_codec` 0x05 — K_shape 8, R_code_max 4, **E = 1024**.
    ///
    /// Neden 0x05 (0x03 değil): symbol boyu E, birim tavanını `E − SOURCE_PREFIX_LEN` ile bağlar
    /// (`validate_fec`). E=64 profilleri birimi 53 B'ye indirir (sabit-overhead %36.6); E=1024
    /// birimi 1013 B'de tutar, mühürlü FEC-shard 26+11+1024 = 1061 B < 1280 PMTU tabanı,
    /// sabit-overhead %3.5 — düşük-bant hedef rejimi için doğru takas (profile.rs:82-84).
    pub const FEC_PROFILE_ID: u8 = 0x05;

    /// Birim (chunk) tavanı = `fec_chunk_capacity(FEC_PROFILE_ID)` = E − `SOURCE_PREFIX_LEN`.
    /// Bir birim tam olarak bir FEC source-symbol'üne oturur (§2.6 symbol_len kilidi).
    /// `fec_v1_profile_is_symbol_aligned` testi bu sabiti katalogla eşler.
    pub const MAX_UNIT_BYTES: usize = 1024 - SOURCE_PREFIX_LEN;

    /// PreConverge ilk-uçuş kredisi (pacer burst cap). Eski `0` değeri bir CLAMP HATASIYDI.
    pub const INITIAL_WINDOW_BYTES: u64 = adaptive_transport::pacer::DEFAULT_BURST_CAP;

    /// Yapı-gate açıldığında istediği parity seviyesi (‰): r=2 @ K=8. r-sweep: 500‰/r4 15–20 %
    /// kayıpta israflı (`reports/fec-loss-risk-v1/rsweep`).
    pub const STRUCTURE_GATE_LEVEL_PERMILLE: u32 = 250;

    /// v1 ürün profilinin `RuntimeConfig`'i. Değiştirmek = sevk davranışını değiştirmek;
    /// `product_v1_profile_is_pinned` testi her alanı pinler.
    #[must_use]
    pub fn runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            max_inflight_units: 4096,
            offer_queue: 1024,
            delivery_queue: 1024,
            max_unit_bytes: Self::MAX_UNIT_BYTES,
            // Fragmenter AÇIK OPT-IN kalır: açmak her DATA çerçevesine 11 B prefix ekler (wire
            // görünür değişiklik) — düşük-bant hedefinde yanlış takas. >1013 B mesaj gönderen
            // uygulama `max_message_bytes`i AÇIKÇA kurar.
            max_message_bytes: None,
            max_peers: 64,
            quota_r: 4,
            drain_cap: Duration::from_secs(3),
            idle_hold: Duration::from_secs(105),
            initial_window_bytes: Self::INITIAL_WINDOW_BYTES,
            bootstrap_rtt: false,
            bootstrap_fast_ladder: false,
            fec_structure_policy: FecStructurePolicy::default(),
            // [DENETİM B-1 DÜZELTMESİ] FEC ürün profiline BAĞLANDI. Önceki `None` "gerektiğinde
            // otomatik açılır" DEĞİL, "bu runtime'da FEC yok" demekti: `fec_loss_risk` kolu açık
            // olmasına rağmen yönettiği alt-sistem ürün yolunda bulunmuyordu (configuration
            // zombie). Sevk kapısı S1-GATE 2026-08-26'da geçilmişti (NO-HARM PASS; burst h3b
            // FEC %100 vs ARQ-only %99) — eksik olan yeni kanıt değil, kararın default'a
            // uygulanmasıydı. `manual_r: None` = Controller (adaptif): parity oranını arbiter
            // belirler, temiz yolda 0'dır.
            fec: Some(FecRuntimeConfig {
                profile_id: Self::FEC_PROFILE_ID,
                manual_r: None,
            }),
            fusion_policy: None,
            loss_confirm_horizon: LossConfirmHorizon::CadenceLegacy,
            fec_loss_risk: FecLossRiskConfig::bounded_v1(),
            ts_sample_period_ns: None,
            structure_gate_level_permille: Some(Self::STRUCTURE_GATE_LEVEL_PERMILLE),
            // [K4G-05 / DUR-19] Canlı teşhis aynası ürün yolunda KAPALI: yazıcı çalışmaz, ayna
            // kurulmaz. INT-02 düzeneği açar (`kama-node` `test-util` kurucuları).
            live_diag: false,
        }
    }
}

impl Default for RuntimeConfig {
    /// v1 ürün profili ([`KamaV1Profile`]).
    fn default() -> Self {
        KamaV1Profile::runtime_config()
    }
}

/// [S1] FEC-config doğrulaması (connect/bind, koşu-öncesi typed): profil bilinmeli ve kabul
/// edilen en büyük payload canonical symbol'e sığmalı (prefix dahil) — aksi hâlde blok/esi
/// hizası sessizce bozulurdu; bounds-before-run.
fn validate_fec(cfg: &RuntimeConfig) -> Result<Option<FecProfile>, RuntimeError> {
    // [S1B] Fragmenter sınırı: mesaj tavanı birim tavanının ALTINDA olamaz (chunk'lama tanımsız
    // kalırdı) ve birim tavanı sıfır olamaz (bölme/sonsuz-döngü). Koşu-öncesi typed.
    if cfg.max_unit_bytes == 0 {
        return Err(RuntimeError::FecConfig("max_unit_bytes 0 olamaz"));
    }
    if let Some(m) = cfg.max_message_bytes {
        if m < cfg.max_unit_bytes {
            return Err(RuntimeError::FecConfig(
                "max_message_bytes < max_unit_bytes (mesaj birimden küçük olamaz)",
            ));
        }
    }
    let Some(f) = cfg.fec else { return Ok(None) };
    let p = by_id(f.profile_id).ok_or(RuntimeError::FecConfig("unknown profile_id"))?;
    if cfg.max_unit_bytes + SOURCE_PREFIX_LEN > p.e_usize() {
        return Err(RuntimeError::FecConfig(
            "max_unit_bytes + SOURCE_PREFIX_LEN > profile.e (payload symbol'e sığmıyor)",
        ));
    }
    if let Some(r) = f.manual_r {
        // [KAMPANYA-KRİTİK] r == 0 GEÇERLİDİR ve anlamlıdır: profil kurulu kalır (fragmentasyon,
        // 0x05 wire çerçevesi ve `SourcePrefix` FEC kollarıyla BYTE-ÖZDEŞ) ama parity ÜRETİLMEZ.
        // Tek-eksenli kontrol kolu budur. `close_fec_block_manual`'ın `for esi in 0..r` döngüsü
        // r=0'da no-op'tur ve `Ok(0)` döner. Eskiden burada reddediliyordu, dolayısıyla kontrol
        // kolu için `profile_id = 0` kullanmak zorunlu kalıyor, o da fragmentasyonu kapatıp
        // birim granülünü (256 B vs 53 B) İKİNCİ BİR EKSEN hâline getiriyordu.
        if r > p.r_code_max {
            return Err(RuntimeError::FecConfig("manual_r > r_code_max"));
        }
    }
    Ok(Some(p))
}

/// [S1B] Bir FEC profilinin **chunk kapasitesi**: `E − SOURCE_PREFIX_LEN`, yani tek bir
/// canonical source-symbol'ün taşıyabildiği en çok app byte'ı. `max_unit_bytes` bunu AŞAMAZ;
/// daha büyük mesajlar fragmenter ile bu boyda chunk'lara bölünür. Bilinmeyen profil → `None`.
///
/// Çağıran (harness/app) config'i bu değere göre kurar; böylece profil katalogunun `E`'sini
/// kendi kopyalamak zorunda kalmaz (tek-kaynak).
#[must_use]
pub fn fec_chunk_capacity(profile_id: u8) -> Option<usize> {
    by_id(profile_id).map(|p| p.e_usize().saturating_sub(SOURCE_PREFIX_LEN))
}

/// [v1/PARAM] Profilden canonical `ParamFields` (§8; iki uç AYNI türetir — hash buna bağlanır).
fn param_fields_for(p: &FecProfile) -> ParamFields {
    ParamFields {
        protocol_version: 1,
        fec_version: 1,
        code_suite_id: p.code_suite_id,
        profile_id: p.profile_id,
        k_shape: p.k_shape,
        r_code_max: p.r_code_max,
        e: p.e,
        partial_policy: 0,
        packet_scheduling_policy: 0,
    }
}

/// Typed çalışma-zamanı hataları — reachable `todo!()` YASAK; her sınır typed reddeder.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Offer kuyruğu dolu (backpressure): app yeniden dener veya düşürür — transport BLOKLAMAZ.
    #[error("offer kuyruğu dolu (bounded backpressure)")]
    QueueFull,
    /// Payload birim tavanını aşıyor (bounds-before-alloc).
    #[error("payload {got} B > max_unit_bytes {cap} B")]
    UnitTooLarge { got: usize, cap: usize },
    /// Endpoint kapatılmış (görev bitti / shutdown sonrası).
    #[error("endpoint kapalı")]
    Closed,
    /// Soket kuruluş hatası (bind/connect) — koşu-öncesi; koşu-içi send hataları kayıp sayılır.
    #[error("soket: {0}")]
    Io(#[from] std::io::Error),
    /// [S1] FEC konfigürasyonu geçersiz (bilinmeyen profil / payload-tavanı symbol'e sığmıyor).
    #[error("fec config: {0}")]
    FecConfig(&'static str),
}

/// Egress kapı-dikişi (opsiyonel; üretim default'u `None` — hiçbir kapı yok).
///
/// **SoC:** politika datagram'ın NE taşıdığını bilmez — yalnız mühürlü uzunluğu ve endpoint
/// koşu-saatini görür. Transport↔security körlüğünün egress tarafı: kapı içeriğe BAKMAZ.
///
/// `admit` `false` dönerse datagram sokete YAZILMAZ ve bu **kanal-kaybı eşdeğeridir**: motor
/// durumu emisyonu ZATEN işledi, karar geri itilmez, send_rate'e dokunulmaz (INV-RATE-1).
/// Politika ASLA geciktirmez/kuyruk tutmaz — ret ya da geçir (paket-atomik).
///
/// Kullanım: benchmark harness'ının yön-başına bütçe governor'ü; ileride operatör kotası.
/// `Arc<Mutex<_>>` ile sarılırsa çağıran koşu sonunda politikanın kendi sayaçlarını okur.
pub trait EgressPolicy: Send {
    /// Bu datagram kanala enjekte edilebilir mi? `len` = mühürlü UDP payload byte'ı;
    /// `now_ns` = endpoint koşu-saati (monoton, `connect`/`bind` anından itibaren).
    fn admit(&mut self, len: usize, now_ns: u64) -> bool;

    /// Arm işareti ([`ClientHandle::mark_t0`] / [`ServerHandle::mark_t0`]): bu andan ÖNCESİ
    /// politikadan muaftır (kuruluş/warmup). Default: no-op.
    fn arm(&mut self, now_ns: u64) {
        let _ = now_ns;
    }
}

/// tx byte'ının sınıf etiketi — YALNIZ sayaç ayrımı için; wire'a hiçbir şey yazmaz.
#[derive(Clone, Copy)]
enum TxClass {
    /// Yeni DATA (ilk emisyon).
    Data,
    /// ARQ retransmit (§9).
    Retx,
    /// §11 forward duplication.
    Dup,
    /// FEC repair-shard.
    Fec,
    /// Kontrol düzlemi (PARAM_SET / PARAM_ACK / RECOVERED / ACK).
    Ctrl,
}

/// [TEŞHİS] Yapı-gate'in KARAR TABANI — koşu sonunda fate-penceresinin fotoğrafı.
///
/// Neden ürün yüzeyinde: `structure_gate_fec` kararını `burst_delta`ya dayandırır, ama o değer
/// Laplace-yumuşatılmıştır ve geçiş sayaçları boşken tamamen PRIOR'dan gelir (sıfır geçiş →
/// p11=p01=500_000 → delta=0 → "IID"). Sayaçlar görünmeden "gate neden açmadı/açtı" sorusu
/// cevaplanamaz; bu kusur sınıfı repoda daha önce de vakum-sayaç olarak yaşandı. Alanlar
/// `adaptive_transport::fate_estimator::WindowFeatures`'ın karar-taşıyan alt kümesidir.
#[derive(Debug, Default, Clone, Copy)]
pub struct FateWindowDiag {
    /// Penceredeki fate-known birim sayısı.
    pub n: u32,
    /// Bunların kaçı KAYIP.
    pub lost: u32,
    /// Ardışık-id geçiş sayaçları (S→S, S→L, L→S, L→L). Toplamı 0 ise yapı sinyali PRIOR'dır.
    pub n00: u32,
    pub n01: u32,
    pub n10: u32,
    pub n11: u32,
    /// `p11 − p01` (ppm, signed) — gate'in okuduğu yapı sinyali.
    pub burst_delta_ppm: i64,
    /// Yapı-gate'in fuzzy `fec=0`'ı açtığı event sayısı (0 = hiç açmadı).
    pub structure_gate_opened: u64,
    /// Ledger'ın pencereli kayıp oranı (ppm) — gate'in KAYIP önkoşulu bu değeri okur
    /// (`STRUCTURE_GATE_LOSS_MIN_PPM = 100_000`), fate-penceresinin `lost/n`'ini DEĞİL.
    ///
    /// **UYARI — bu bir ANLIK ÖLÇER, kapanış anında örneklenir.** Ledger trafik durunca DECAY
    /// eder (gate'in "bayat-FEC yok" tasarımı bunu ister), dolayısıyla buradaki 0 değeri
    /// *"koşu boyunca hiç 100_000'i geçmedi"* ile *"geçti ama kapanışa dek söndü"* arasında
    /// AYRIM YAPMAZ. Koşu boyunca geçerli tek sonuç `structure_gate_opened`'dır: o monoton bir
    /// sayaçtır, 0 ise gate koşunun HİÇBİR anında açmamıştır.
    pub ledger_loss_ppm: u64,
    /// Ledger pencereli kaybının koşu-boyu **TEPESİ** (ppm). `ledger_loss_ppm`'in aksine
    /// decay'den etkilenmez → gate'in kayıp önkoşulunu (`≥ 100_000`) koşu boyunca HİÇ görüp
    /// görmediğini bu değer söyler.
    pub ledger_loss_ppm_peak: u64,
    /// [GÖRÜNÜRLÜK] Kapanış anındaki motor fazı (`SenderState`), metin olarak. Cold-start
    /// koşusunda EN KARAR-VERİCİ olgu buydu ve ürün yolunda hiç okunmuyordu: bir koşunun
    /// PreConverge'de takılı kalıp kalmadığı, teslim sayısına bakarak AYIRT EDİLEMİYORDU.
    pub sender_state: &'static str,
    /// [GÖRÜNÜRLÜK] Oturum KURULDU mu = bir RTT örneği alındı mı. PreConverge'den çıkışın motor
    /// içindeki TEK koşulu budur, dolayısıyla bu bir vekil değil TANIMDIR. `false` = oturum hiç
    /// kurulamadı → o koşunun teslim sayısı bir FEC/ARQ sonucu DEĞİLDİR, kuruluş başarısızlığıdır.
    pub left_preconverge: bool,
    /// [GÖRÜNÜRLÜK] Alınan RTT örneği sayısı (monoton).
    pub rtt_samples: u32,
    /// [GÖRÜNÜRLÜK] Kuruluş-denemesi (establish probe) emisyon sayısı.
    pub establish_probes_tx: u64,
    /// [DEADLINE-EKSENİ] Uygulamanın bildirdiği EN SIKI teslim bütçesi (ns), kapanış anında.
    /// `None` = hiçbir offer bütçe bildirmedi → kalibre politika `NoDeadlineDeclared` ile
    /// ArqOnly'ye düşer. Bu alan olmadan "app bütçe bildirdi mi, motora ULAŞTI mı" sorusu
    /// ürün yolundan cevaplanamıyordu (kablolamanın kendisi gözlemlenemezdi).
    pub declared_deadline_ns: Option<u64>,
    /// [DEADLINE-EKSENİ] Füzyon politikası aksiyon histogramı
    /// (`adaptive_transport::fusion_policy::FusionAction::index` kovaları). Politika kapalıysa
    /// tamamı sıfır. Ret sebebi görünmeden "politika neden FEC açmadı" cevaplanamaz.
    pub fusion_actions: [u64; adaptive_transport::fusion_policy::FUSION_ACTION_KINDS],
    /// [TEŞHİS] Politikaya giden SON gözlemin ham değerleri:
    /// `(loss_ppm, bandwidth_bps, telemetry_age_ns, rtt_sample_count, deadline_over_srtt_permille)`.
    pub last_fusion_obs: Option<(u64, u64, u64, u32, Option<u32>)>,
    /// [TEŞHİS] Politikanın gördüğü `bandwidth_bps` aralığı (min, max).
    pub fusion_bw_range: Option<(u64, u64)>,
    /// [TEŞHİS] Politikanın gördüğü `loss_ppm` aralığı (min, max).
    pub fusion_loss_range: Option<(u64, u64)>,
    /// [TEŞHİS] Kayıp-defteri iç durumu:
    /// `(lost_units, events_len, suspected_units, confirmed_units, sent_window_units)`.
    pub ledger_internals: (u64, u64, u64, u64, u64),
    /// [TEŞHİS] Confirm-yolu:
    /// `(confirmed_total, max_suspected_age_ns, confirm_threshold_ns, expire_threshold_ns)`.
    pub ledger_confirm_path: (u64, u64, u64, u64),
    /// [TEŞHİS] Ledger tick kapsamı: `(tick_count, tick_span_ns)`.
    pub ledger_tick_span: (u64, u64),
    /// [TEŞHİS] Geri-alınan (Spurious) toplam birim — kısa confirm ufkunun YANLIŞ-confirm
    /// bedeli bu sayaç olmadan ölçülemez. Un-loss `Confirmed`'ı da çürütür (kanıt kanıttır).
    pub spurious_units: u64,
    /// [TEŞHİS] Controller-mode FEC latch bastırma sayaçları:
    /// `(no_eligible_decision, decision_event_mismatch, no_rule_zero, enqueued_r, emitted_r)`.
    /// Politika `FecAssist` isteyip de wire'da parity görünmediğinde kopuk halka buradadır.
    pub fec_latch_diag: (u64, u64, u64, u64, u64),
    /// [TEŞHİS] Son latch sonucu, metinsel bastırma sebebi (`None` = latch hiç koşmadı).
    pub fec_last_suppression: Option<&'static str>,
    /// [FEC-RISK ENSTRÜMAN] `(t_first_suspected, t_first_confirmed, t_first_risk, t_first_parity)`
    /// ns; 0 = hiç. Kolun ölçtüğü büyüklük bu damgaların FARKIDIR.
    pub fec_risk_times: (u64, u64, u64, u64),
    /// [FEC-RISK ENSTRÜMAN] `(son_risk_ppm, tepe_risk_ppm)`.
    pub fec_risk_ppm: (u64, u64),
    /// [FEC-RISK v2 ENSTRÜMAN] Arbiter sebep histogramı (`FecArbiterReason::index`):
    /// None · ConfirmedFuzzy · ConfirmedPolicy · StructureGate · SpecBootstrap · SpecPersistent ·
    /// DeadlineAssisted. "Parity NEDEN bu miktarda" sorusunun tek kaynağı.
    pub fec_arbiter_reasons: [u64; ARBITER_REASON_KINDS],
    /// [FEC-RISK v2 ENSTRÜMAN] Spekülatif seviye histogramı `(L0, L1, L2)`.
    pub fec_spec_levels: [u64; 3],
    /// [FEC-RISK v2 ENSTRÜMAN] `(persistent_ppm_peak, reorder_pressure_last‰, reorder_pressure_peak‰,
    /// t_first_l1_ns, t_first_l2_ns)`.
    pub fec_spec_signals: (u64, u32, u32, u64, u64),
    /// [PERSIST ENSTRÜMANI] `(ufuk_son_ns, ufuk_tepe_ns, spurious_örnek)`.
    pub fec_persist: (u64, u64, usize),
    /// [EVIDENCE ENSTRÜMANI] İlk L1 / ilk L2 anında `(sent_window_units, instant_ppm, persistent_ppm)`.
    pub fec_spec_transitions: ((u64, u64, u64), (u64, u64, u64)),
    /// [C4-ATIF] Zamanlayıcı sınıf sayaçları (yalnız `diag-sched-telemetry` feature'ında dolu, aksi 0):
    /// `[data_sel, data_denied, data_emit, retx_sel, retx_denied, retx_emit, dup_sel, dup_denied,
    /// dup_emit, parity_denied, parity_emit, tail_probe_emit]`.
    pub sched: [u64; 12],
    /// [KURULUŞ-GUARD] PreConverge'de arbiter çıkışının sıfırlandığı karar sayısı.
    pub preconverge_parity_blocked: u64,
    /// [FAZ-1 DÖNÜŞ-YOLU] Ölçülen geri-bildirim durumu: `(measured_return_bw_bps, ack_interval_ns,
    /// silence_ns, continuity‰, forward_loss‰, return_impairment‰, direction_evidence,
    /// confidence‰, health, ack_samples, v2_samples)`.
    pub return_path: adaptive_transport::return_path::ReturnPathState,
    /// [FAZ-2] `(repair_loss_not_arqed, parity_budget_clamped, parity_budget_would_zero)`.
    pub fec_completeness: (u64, u64, u64),
    /// [FAZ-3] Aktivasyon + tahminci yüzeyi: yapılandırıldı / ulaşılabilir / ateşledi ayrımı.
    pub activation: adaptive_transport::sender::ActivationDiag,
    /// Yapı-gate karar-yolu histogramı (`adaptive_transport::sender::GateOutcome::index`
    /// sırasıyla): 0=kapsam-dışı · 1=kayıp-bandın-altında · 2=pencere-küçük ·
    /// 3=geçiş-kanıtı-yetersiz · 4=burst→ARQ · 5=AÇILDI · 6=oturum-kurulmadı(PreConverge).
    /// "Açmadı" gözleminin NEDENİ buradadır.
    pub gate_outcomes: [u64; 7],
    /// Ledger'ın GERİ ALDIĞI (un-loss / Spurious) birim toplamı. Yüksekse: kayıplar ilan edildi
    /// ama geç-ACK ufkunda çürütüldü — yani ARQ onları ufuk içinde kurtardı ve `loss_windowed_ppm`
    /// TASARIM GEREĞİ sıfıra yakın kalır. Bu, ledger kusuru ile "hızlı yolda ARQ zaten yetiyor"
    /// durumunu ayırt eden tek ucuz sinyaldir (loopback'te RTT≈0 → ikincisi beklenir).
    pub ledger_spurious_units: u64,
    /// `diag_unloss` — geç-ACK ile geri alınan kayıp ilanı sayısı.
    pub ledger_unloss: u64,
    /// Paket-eşiği ile ilan edilen kayıp sayısı (`diag_loss_pkt`).
    pub loss_declared_pkt: u64,
    /// Zaman-eşiği ile ilan edilen kayıp sayısı (`diag_loss_time`).
    pub loss_declared_time: u64,
}

/// Client koşu istatistikleri (item 9 doğrulama yüzeyi; gözlem-amaçlı).
#[derive(Debug, Default, Clone)]
pub struct ClientStats {
    /// [KISA-AKIŞ ENSTRÜMANI] Ana döngünün bittiği an (ns), sender damgalarıyla AYNI saat
    /// tabanında. `t_flow_start` bu tabanda tanım gereği 0'dır → `flow_end_ns == flow_duration`.
    pub flow_end_ns: u64,
    /// [HANDOFF ENSTRÜMANI] `(t_ns, örnek)` zaman serisi; yalnız `ts_sample_period_ns` açıkken dolar,
    /// en çok [`TS_CAP`] örnek. Salt gözlem.
    pub ts: Vec<(u64, TsSample)>,
    /// App'ten kabul edilen birim.
    pub offered: u64,
    /// İlk-ACK'i görülen birim (ilk-ack-kazanır dedup). [K4G-05b · CEVAP 13 §3] "İlk ACK" =
    /// motorun ACK ile teyit ettiği birim (`Sender::take_confirmed_units`; birim oturumda en çok bir
    /// kez). Motorun tanımadığı geç ACK (kaydı unutulmuş) sayılmaz; birim yeniden gönderimin ACK'iyle
    /// sayılır.
    pub acked: u64,
    /// Drain sonunda hâlâ teslim-teyitsiz birim (kabul edilen residue).
    pub undelivered: u64,
    /// Wire'a yazılan / wire'dan okunan datagram.
    pub tx_datagrams: u64,
    pub rx_datagrams: u64,
    /// `seam.open` reddi (doğrulanamayan gelen datagram — sessiz düşürüldü).
    pub open_failures: u64,
    /// Açıldı ama tanınmayan çerçeve tipi (ne DATA ne ACK) — kontrol-düzlemi dal-noktası
    /// (item 6, workstream B'de kama-noise-wire formatıyla bağlanır); sessiz yutulmaz, sayılır.
    pub unknown_frames: u64,
    /// Payload'ı store'da bulunamayan (çoktan ACK'lenmiş birime geç karar) emisyon — kayıp sayıldı.
    /// [K4G-05b · CEVAP 13 §3] Store yalnız motor teyidiyle boşaldığından bu sayaç motorun TEYİTLİ
    /// birime karar üretmesidir (sözleşme ihlali): 0 beklenir (store-miss bekçisi).
    pub stale_decisions: u64,
    /// Soket send hatası (kanal kaybı eşdeğeri, INV-RATE-1).
    pub send_errors: u64,
    /// Shutdown anında offer-kuyruğunda kalmış, HİÇ işlenmemiş birim (app `Ok` almıştı ama
    /// gönderim denenmedi — sessiz kayıp yerine sayaç; app kapanışta bunu okur).
    pub unaccepted_offers: u64,
    /// [S1] Latch'in parity planladığı (ve RS-encode edilen) blok sayısı.
    pub fec_blocks_latched: u64,
    /// [S1] Wire'a yazılan gerçek repair-shard.
    pub fec_repairs_tx: u64,
    /// [S1] Repair-store'da bulunamayan FecParity kararı (evict/latch-0) — başlık-only gitti.
    pub fec_repair_missing: u64,
    /// [S1] RECOVERED-control ile teslim-teyidi alan birim (alıcı FEC-kurtardı → retransmit iptal,
    /// store'dan düşer, app `acked()` akışına girer). [K4G-05b · CEVAP 13 §3] Teyidi motor verir
    /// (`RecoveredControl` sonrası teyit çıktısı); motorun daha önce ACK'le teyit ettiği birim
    /// sayılmaz (eskisi gibi: o birim store'da yoktu).
    pub fec_recovered_ctrl_units: u64,
    /// [v1] Typed seal-reddi (seam hazır-değil/PN) — emisyon kayıp sayıldı (INV-RATE-1).
    pub seal_failures: u64,
    /// [v1/PARAM] Gönderilen PARAM_SET çerçevesi (bounded retry).
    pub param_sends: u64,
    /// [v1/PARAM] PARAM_ACK hash-echo doğrulandı (0/1; B2: emisyon zaten akıyordu).
    pub param_confirmed: u64,
    /// Wire'a yazılmak üzere ÜRETİLEN mühürlü UDP payload byte'ı — DENEME düzlemi
    /// ([`EgressPolicy`] reddettikleri DAHİL; gerçek-enjekte düzlem politikanın kendi
    /// sayacındadır). Alt-sınıf ayrımı `tx_*_bytes` alanlarında; toplamları bu değere eşittir.
    pub tx_bytes: u64,
    /// Wire'dan okunan UDP payload byte'ı.
    pub rx_bytes: u64,
    /// Yeni DATA byte'ı.
    pub tx_data_bytes: u64,
    /// ARQ retransmit byte'ı (§9).
    pub tx_retx_bytes: u64,
    /// §11 forward duplication byte'ı.
    pub tx_dup_bytes: u64,
    /// FEC repair-shard byte'ı.
    pub tx_fec_bytes: u64,
    /// Kontrol-düzlemi byte'ı (PARAM_SET).
    pub tx_ctrl_bytes: u64,
    /// [`EgressPolicy`] reddi: hiç enjekte edilmeyen datagram/byte (kanal-kaybı eşdeğeri).
    pub egress_refused_pkts: u64,
    /// Reddedilen byte toplamı.
    pub egress_refused_bytes: u64,
    /// [S1B] TÜM chunk'ları teyitlenip app'e ACK'i verilen mesaj (yalnız fragmentasyon açıkken;
    /// kapalıyken birim=mesaj olduğundan `acked` ile aynıdır ve bu sayaç 0 kalır).
    pub messages_acked: u64,
    /// [S1B] Canonical symbol serialize hatası — ULAŞILAMAZ olmalı (bounds koşu-öncesi typed).
    /// Sıfırdan farklıysa blok/esi hizası bozulmuştur; sessiz bozulma yerine görünür sayaç.
    pub frag_encode_failures: u64,
    /// [FAZ-2 KALEM-1] Akış sonunda zero-dolgu ile kapatılan KISMİ FEC bloğu sayısı (Model-A).
    pub fec_partial_blocks_closed: u64,
    /// [TEŞHİS] Yapı-gate'in karar tabanı (kapanış anındaki fotoğraf) — bkz. [`FateWindowDiag`].
    pub fate: FateWindowDiag,
    /// [DR-CS-3] Wire'a yazılan CS-probe (0 = bootstrap kapalı VEYA PreConverge hiç yaşanmadı).
    pub cs_probes_tx: u64,
    /// [DR-CS-3] Alınan ve motora beslenen CS-echo — RTT örneğinin tek kaynağı.
    pub cs_echoes_rx: u64,
    /// [L-T1] Kurtarma kapalıyken ([`ClientHandle::set_recovery`]) tele ÇIKARILMAYAN ARQ
    /// retransmit / §11 kopya / FEC repair kararı (yalnız test yapılandırması).
    pub recovery_suppressed: u64,
}

/// Server koşu istatistikleri.
#[derive(Debug, Default, Clone, Copy)]
pub struct ServerStats {
    /// App'e teslim edilen birim (ilk-teslim).
    pub delivered: u64,
    /// Teslim kanalı doluyken düşen teslim (app drenaj sözleşmesi ihlali görünür olsun).
    pub delivery_dropped: u64,
    /// Peer-cap fail-closed reddi (yeni peer, tablo dolu).
    /// [K4G-01b] "Yeni peer" = yeni `(adres, SessionHandle)` yuvası.
    pub dropped_new_peers: u64,
    /// Idle-emekliliğe ayrılan peer (item 3).
    /// [K4G-01b] Bağlı oturumu oturum tablosundan çıkmış (kapanmış) yuvanın emekliliği de burada
    /// sayılır — `idle_hold`'u beklemez; yapı alanları `api_surface_v0` ile pinli olduğundan ayrı
    /// sayaç açılmadı.
    pub retired_idle_peers: u64,
    pub rx_datagrams: u64,
    pub open_failures: u64,
    /// Açıldı ama tanınmayan çerçeve (kontrol-düzlemi dal-noktası; bkz. ClientStats.unknown_frames).
    pub unknown_frames: u64,
    /// [S1] Alınan repair-shard (blok-birleştiriciye girdi).
    pub fec_repairs_rx: u64,
    /// [S1] FEC-reconstruct ile kurtarılıp app'e teslim edilen birim (DATA'sı hiç gelmemişti).
    pub fec_recovered_units: u64,
    /// [S1] Hazır-görünen blokta reconstruct hatası (blok KORUNUR — shard'lar atılmaz; readiness
    /// sayım-bug'ının düzeltilmiş davranışı).
    pub fec_decode_failures: u64,
    /// [S1] Kapasite gereği düşürülen en-eski blok (bounded birleştirici).
    pub fec_blocks_evicted: u64,
    /// [C4-ATIF] Alınan repair'lerin kesişimsiz akıbeti: blok KAYNAKLARLA tamamlandığında elde tutulan
    /// repair'ler (ARQ/geç-orijinal parity'yi yendi → boşa), evict anında tutulanlar (boşa), başarılı
    /// decode'da tutulanlar (faydalı). Kalan = kapanışta hâlâ açık bloklarda.
    pub fec_repairs_wasted_complete: u64,
    pub fec_repairs_wasted_evicted: u64,
    pub fec_repairs_useful: u64,
    /// [C4-ATIF] Blok akıbetleri: kaynaklarla tamamlanan (repair tutan / tutmayan), decode edilen,
    /// evict edilen (repair tutan / hayalet = repair'siz ve kaynak < K).
    pub fec_blocks_complete_with_repairs: u64,
    pub fec_blocks_decoded: u64,
    pub fec_blocks_evicted_with_repairs: u64,
    /// [S1] Wire'a yazılan RECOVERED-control çerçevesi (blok başına bir).
    pub fec_recovered_ctrl_tx: u64,
    /// [S1B] Birleştiriciye giren chunk (DATA + FEC-kurtarma; yalnız fragmentasyon açıkken).
    pub frag_chunks_rx: u64,
    /// [S1B] Tam olarak yeniden kurulup app'e teslim edilen mesaj.
    pub msgs_reassembled: u64,
    /// [S1B] Mesaj tavanını aşacağı için reddedilen chunk (bounds-before-alloc; bozuk/kötücül
    /// FINAL'in belleği şişirmesi engellendi).
    pub frag_chunk_dropped: u64,
    /// [S1B] Kapasite gereği düşürülen en-eski yarım mesaj (bounded birleştirici).
    pub msg_asm_evicted: u64,
    /// [DR-CS-3] Alınan CS-probe.
    pub cs_probes_rx: u64,
    /// [DR-CS-3] Dönülen CS-echo (probe'dan KÜÇÜK — amp≤1).
    pub cs_echoes_tx: u64,
    /// [v1] Typed seal-reddi (ACK/RECOVERED/PARAM_ACK dönüş yolu).
    pub seal_failures: u64,
    /// [v1/PARAM] PARAM_SET doğrulanıp bağlanan peer (ilk-bind başına 1).
    pub param_bound_peers: u64,
    /// [v1/PARAM] PARAM_SET doğrulama reddi (hash/profil-mismatch; fail-closed, bind YOK).
    pub param_rejects: u64,
    /// [v1/PARAM] PARAM-bağlanmadan gelen FEC-shard (fail-closed sayaçlı düşürme).
    pub fec_pre_param_dropped: u64,
    /// Dönüş yönünde üretilen datagram (ACK/RECOVERED/PARAM_ACK) — DENEME düzlemi.
    pub tx_datagrams: u64,
    /// Dönüş yönünde üretilen mühürlü UDP payload byte'ı — DENEME düzlemi ([`EgressPolicy`]
    /// reddettikleri DAHİL; client tarafındaki `tx_bytes` ile aynı sözleşme).
    pub tx_bytes: u64,
    /// Wire'dan okunan UDP payload byte'ı.
    pub rx_bytes: u64,
    /// [`EgressPolicy`] reddi: hiç enjekte edilmeyen datagram/byte.
    pub egress_refused_pkts: u64,
    /// Reddedilen byte toplamı.
    pub egress_refused_bytes: u64,
}

/// [K4G-05] Client ucunun CANLI salt-gözlem aynası ([`ClientHandle::live_diag`]): motorun ARQ /
/// kurtarma sayaçları ve RTT kestiricisi, kapanışı beklemeden okunur. Görev her ana döngü turunun
/// başında değerleri buraya yazar; hiçbir karar bu aynayı OKUMAZ, sayaç ve davranış değişmez
/// ([`ServerHandle::delivery_dropped`] emsali). Ana döngüden sonraki kapanış boşaltmasının
/// (drain) değerleri aynaya girmez; kesin kapanış değeri [`ClientHandle::shutdown`]'ın
/// istatistiğidir (`ClientStats::fate.activation`). İstisna: ACK çerçevesi kaydı
/// ([`Self::acks`], [`Self::ack`]) çerçeve işlendiği anda yazılır, boşaltmadakiler dahil.
///
/// [DUR-19] Ayna ve yazıcısı YALNIZ [`RuntimeConfig::live_diag`] açıkken vardır (ürün varsayılanı
/// KAPALI). Kapalıyken görev hiçbir alanı yazmaz ve okuyucular [`ClientLiveDiag::default`] görür:
/// bütün sayaçlar ve RTT alanları 0, `recovery` / `clock_origin` / `control.state` `None`, halkalar
/// boş (`total() == 0`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ClientLiveDiag {
    /// Motorun `Retransmit` sınıflı emisyon kararları (ARQ + tail-probe) —
    /// `ActivationDiag::retransmits`. Kurtarma kapalıyken karar sayılır ama tele çıkmaz
    /// (bkz. [`Self::recovery_suppressed`]).
    pub retransmits: u64,
    /// Bunlardan tail-probe olanlar — `ActivationDiag::tail_probes`.
    pub tail_probes: u64,
    /// §11 ileri kopya kararları — `ActivationDiag::dups`.
    pub dups: u64,
    /// Motorun tele çıkardığı FEC parity kararları — `ActivationDiag::parity_emitted`.
    pub parity_emitted: u64,
    /// Kurtarma kapalıyken tele ÇIKMAYAN retransmit / kopya / FEC repair kararı —
    /// [`ClientStats::recovery_suppressed`].
    pub recovery_suppressed: u64,
    /// Tele çıkan ARQ retransmit baytı — [`ClientStats::tx_retx_bytes`] (`mark_t0` sıfırlar).
    pub tx_retx_bytes: u64,
    /// Motorun aldığı RTT örneği sayısı (Karn: yalnız orijinal emisyonlardan).
    pub rtt_samples: u32,
    /// sRTT (µs); örnek yoksa 0 — `ActivationDiag::srtt_us`.
    pub srtt_us: u64,
    /// RTprop / en küçük RTT (µs); örnek yoksa 0 — `ActivationDiag::rtprop_us`.
    pub rtprop_us: u64,
    /// PreConverge'den çıkış anı (ns, ucun başlangıcından; 0 = çıkılmadı) —
    /// `ActivationDiag::left_preconverge_ns`.
    pub left_preconverge_ns: u64,
    /// [K4G-05 / INT-02 TEŞHİS] Gönderen kurtarma anlık görüntüsü (takılı birim zaman çizelgesi);
    /// görev ana döngüde en çok [`RECOVERY_DIAG_PERIOD`]'da bir yeniler. İlk yenilemeden önce,
    /// kapanmış oturumlar toplamında (`ClientTransport::retired_live_diag`) ve canlı ayna
    /// kapalıyken ([`RuntimeConfig::live_diag`]) `None`.
    pub recovery: Option<SenderRecoveryDiag>,
    // ── [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] M1 (bayat zamanlayıcı SACK'leri) / M2 (hız tabanı) ──
    /// Ucun saat tabanının sıfırı: `*_ns` anları bu ana göredir (motorun `on_event`'e verilen
    /// saat). Görev başlamadan ve kapanmış oturumlar toplamında `None`.
    pub clock_origin: Option<Instant>,
    /// Aynanın son yazıldığı an (ns; ucun saat tabanında).
    pub at_ns: u64,
    /// Gönderen motorun denetim durumu: faz, hız kestiricileri, acil durum, dönüş yolu.
    pub control: SenderControlDiag,
    /// Tele yazılan / telden okunan datagram ve tele yazılan bayt — [`ClientStats::tx_datagrams`],
    /// [`ClientStats::rx_datagrams`], [`ClientStats::tx_bytes`] (`mark_t0` sıfırlar).
    pub tx_datagrams: u64,
    pub rx_datagrams: u64,
    pub tx_bytes: u64,
    /// ACK çerçevesi sayaçları (çerçeve işlendiği anda yazılır).
    pub ack: AckFrameCounts,
    /// Son [`ACK_LOG_LEN`] ACK çerçevesinin kaydı (çerçeve işlendiği anda yazılır).
    pub acks: DiagRing<AckFrameDiag, ACK_LOG_LEN>,
    /// Son [`STATE_LOG_LEN`] motor durum değişikliği (ana döngü turu çözünürlüğünde).
    pub states: DiagRing<StateChangeDiag, STATE_LOG_LEN>,
}

/// [K4G-05 / INT-02 TEŞHİS] [`SenderRecoveryDiag`]'ın yenilenme aralığı — gözlem çözünürlüğü,
/// normatif DEĞİL `[KALİBRASYON]`. Görüntü sınırlı yapıları (teyitsiz birim deposu, motorun uçuş /
/// kayıp / kopya kuyrukları, PN çeviri penceresi) tarar; her döngü turunda değil, bu aralıkla alınır.
/// [DUR-19] Yalnız [`RuntimeConfig::live_diag`] açıkken alınır.
pub const RECOVERY_DIAG_PERIOD: Duration = Duration::from_millis(200);

/// [K4G-05 / INT-02 TEŞHİS] Gönderen ucun kurtarma anlık görüntüsü ([`ClientLiveDiag::recovery`]):
/// teyitsiz birim sayısı, en eski teyitsiz birimin emisyon geçmişi ve motorun kurtarma
/// zamanlayıcıları. SALT OKUMA: görev yalnız mevcut durumu okur (`Sender::diag_recovery_timers`,
/// `Sender::diag_unit_recovery`); hiçbir karar bu yapıyı okumaz, motor olayı sürülmez. Süreler
/// görüntünün alındığı ana (`taken_at`) göredir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderRecoveryDiag {
    /// Görüntünün alındığı an (süreç monotonik saati).
    pub taken_at: Instant,
    /// Kabul edilmiş ama teyidi (ACK / FEC-RECOVERED) gelmemiş birim sayısı.
    pub unacked_units: u32,
    /// En eski (en küçük kimlikli) teyitsiz birim; teyitsiz birim yoksa `None`.
    pub oldest_unacked: Option<StuckUnitDiag>,
    /// Son ACK'in işlenmesinden beri (ns); hiç ACK yoksa `None`.
    pub since_last_ack_ns: Option<u64>,
    /// Tail-recovery idle penceresinin başından beri (ns); pencere kurulu değilse `None`.
    pub tail_armed_for_ns: Option<u64>,
    /// Motorun kurtarma kuyrukları ve zamanlayıcıları.
    pub timers: adaptive_transport::sender::RecoveryTimersDiag,
    /// [K4G-05b · CEVAP 13 §3 — CEVAP 10 DUR-23 (b) runtime tablo sayacı] Runtime'ın motor seq →
    /// birim haritasındaki ve birim → seq listesindeki girdi sayısı. Girdiler yalnız teyitsiz
    /// (store'daki) birim için açılır ve birim motorca teyit edilince düşer; teyitsiz birim yokken
    /// ikisi de 0 olmalıdır (sızıntı bekçisi). SALT GÖZLEM.
    pub seq_to_unit_len: u32,
    pub unit_seqs_len: u32,
}

/// [K4G-05 / INT-02 TEŞHİS] En eski teyitsiz birim ([`SenderRecoveryDiag::oldest_unacked`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StuckUnitDiag {
    /// Birim kimliği (alıcının sıralı teslimindeki `unit_id`).
    pub unit_id: u64,
    /// Motorun bu birim için verdiği emisyon kararı sayısı: orijinal + ARQ retransmit + kopya +
    /// tail-probe (kurtarma kapalıyken bastırılanlar dahil).
    pub emission_decisions: u32,
    /// Son emisyon kararının motor seq'i ve — tele çıktıysa ve PN çeviri penceresindeyse — tel PN'i.
    pub last_seq: Option<u64>,
    pub last_pn: Option<u64>,
    /// Motorun bu birim için o an tuttuğu kayıtlar.
    pub engine: adaptive_transport::sender::UnitRecoveryDiag,
    /// Motor kayıtlarının en yenisinin / en eskisinin gönderiminden beri (ns). Kayıp ilanında düşen
    /// kopya ve süresi dolan tombstone kayıtları görünmez.
    pub since_latest_send_ns: Option<u64>,
    pub since_earliest_record_ns: Option<u64>,
    /// Kopya kuyruğundaki vadeye kalan süre (ns; negatif = vade geçti, jeton bekliyor).
    pub dup_due_in_ns: Option<i64>,
}

/// [K4G-05 / INT-02 TEŞHİS] Gönderen kurtarma görüntüsü (bkz. [`SenderRecoveryDiag`]). SALT OKUMA:
/// yalnız `&` referanslar; `now_ns` motorun saat tabanındadır (`elapsed_ns(start)`).
fn capture_recovery(
    sender: &adaptive_transport::sender::Sender,
    store: &HashMap<u64, Vec<u8>>,
    seq_to_unit: &HashMap<u64, u64>,
    unit_seqs: &HashMap<u64, Vec<u64>>,
    pn_seq: &VecDeque<(u64, u64)>,
    now_ns: u64,
) -> SenderRecoveryDiag {
    let since = |t: u64| now_ns.saturating_sub(t);
    let timers = sender.diag_recovery_timers();
    let oldest_unacked = store.keys().min().map(|&unit_id| {
        let seqs = unit_seqs.get(&unit_id);
        let last_seq = seqs.and_then(|s| s.last().copied());
        let last_pn = last_seq.and_then(|seq| {
            pn_seq
                .iter()
                .rev()
                .find(|&&(_, s)| s == seq)
                .map(|&(pn, _)| pn)
        });
        let engine = sender.diag_unit_recovery(unit_id);
        StuckUnitDiag {
            unit_id,
            emission_decisions: seqs.map_or(0, |s| u32::try_from(s.len()).unwrap_or(u32::MAX)),
            last_seq,
            last_pn,
            engine,
            since_latest_send_ns: engine.latest_send_ns.map(since),
            since_earliest_record_ns: engine.earliest_record_send_ns.map(since),
            dup_due_in_ns: engine.dup_due_ns.map(|due| {
                let delta = i128::from(due) - i128::from(now_ns);
                i64::try_from(delta).unwrap_or(if delta < 0 { i64::MIN } else { i64::MAX })
            }),
        }
    });
    SenderRecoveryDiag {
        taken_at: Instant::now(),
        unacked_units: u32::try_from(store.len()).unwrap_or(u32::MAX),
        oldest_unacked,
        since_last_ack_ns: timers.last_ack_ns.map(since),
        tail_armed_for_ns: timers.tail_armed_at_ns.map(since),
        timers,
        seq_to_unit_len: u32::try_from(seq_to_unit.len()).unwrap_or(u32::MAX),
        unit_seqs_len: u32::try_from(unit_seqs.len()).unwrap_or(u32::MAX),
    }
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Canlı aynada tutulan son ACK çerçevesi kaydı sayısı
/// ([`ClientLiveDiag::acks`]) — gözlem çözünürlüğü, normatif DEĞİL `[KALİBRASYON]`. Halka sabit
/// boyutludur (SR-AUDIT-001); iki okuma arasında bundan fazla çerçeve işlenirse eskileri halkadan
/// düşer ve okuyucu bunu kayıt sıra numarasındaki ([`AckFrameDiag::index`]) boşluktan görür.
pub const ACK_LOG_LEN: usize = 32;

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Canlı aynada tutulan son durum değişikliği kaydı sayısı
/// ([`ClientLiveDiag::states`]); bkz. [`ACK_LOG_LEN`].
pub const STATE_LOG_LEN: usize = 32;

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Sabit boyutlu son-kayıt halkası: en yeni `N` kayıt ve şimdiye
/// dek eklenen kayıt sayısı. Ekleme ayırma yapmaz; salt gözlem yapısıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagRing<T, const N: usize> {
    total: u64,
    next: usize,
    held: usize,
    slots: [T; N],
}

impl<T: Copy + Default, const N: usize> Default for DiagRing<T, N> {
    fn default() -> Self {
        Self {
            total: 0,
            next: 0,
            held: 0,
            slots: [T::default(); N],
        }
    }
}

impl<T: Copy, const N: usize> DiagRing<T, N> {
    /// Şimdiye dek eklenen kayıt sayısı (halkadan düşenler dahil).
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Halkadaki kayıtlar, eskiden yeniye (en çok `N`).
    pub fn recent(&self) -> impl Iterator<Item = &T> + '_ {
        let start = (self.next + N - self.held) % N.max(1);
        (0..self.held).map(move |k| &self.slots[(start + k) % N])
    }

    fn push(&mut self, item: T) {
        self.total = self.total.saturating_add(1);
        if N == 0 {
            return;
        }
        self.slots[self.next] = item;
        self.next = (self.next + 1) % N;
        self.held = (self.held + 1).min(N);
    }
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Motorun bir ACK çerçevesini işlemeden hemen önceki ya da olayı
/// işledikten hemen sonraki görünümü ([`AckFrameDiag::before`] / [`AckFrameDiag::after`]):
/// `Sender::diag_recovery_timers` ve `Sender::diag_activation` okuması. Motor olayı TEK çağrıda
/// işler ve planlar: "sonra" görünümü aynı olayda planlanan emisyonları da içerir.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MotorAckView {
    pub state: Option<adaptive_transport::sender::SenderState>,
    /// Motorun uçuştaki, retransmit bekleyen, saklanan (tombstone) ve vadeli kopya kayıtları.
    pub inflight_len: u32,
    pub pending_lost_len: u32,
    pub tombstones_len: u32,
    pub dup_queue_len: u32,
    /// Motorun tele çıkardığı tail-probe sayısı (`ActivationDiag::tail_probes`).
    pub tail_probes: u64,
    /// Tail-recovery idle penceresinin başlangıcı ve bu penceredeki probe turu.
    pub tail_armed_at_ns: Option<u64>,
    pub tail_probe_index: u32,
    /// Motorun son ACK'i işlediği an; hiç ACK yoksa `None`.
    pub last_ack_ns: Option<u64>,
}

impl MotorAckView {
    fn of(sender: &adaptive_transport::sender::Sender) -> Self {
        let t = sender.diag_recovery_timers();
        Self {
            state: Some(t.state),
            inflight_len: t.inflight_len,
            pending_lost_len: t.pending_lost_len,
            tombstones_len: t.tombstones_len,
            dup_queue_len: t.dup_queue_len,
            tail_probes: sender.diag_activation().tail_probes,
            tail_armed_at_ns: t.tail_armed_at_ns,
            tail_probe_index: t.tail_probe_index,
            last_ack_ns: t.last_ack_ns,
        }
    }
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Gönderen ucun çözdüğü bir ACK çerçevesi (M1: bayat zamanlayıcı
/// SACK'leri). SALT GÖZLEM: alanlar çerçeve motora verilirken okunur; hiçbir karar bu kaydı okumaz.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AckFrameDiag {
    /// Ucun çözülen ACK çerçevesi sırası (1'den; [`AckFrameCounts::frames`] ile aynı sayaç).
    pub index: u64,
    /// İşlendiği an (ns; ucun saat tabanında — bkz. [`ClientLiveDiag::clock_origin`]).
    pub at_ns: u64,
    /// V2 (0x14) çerçeve mi; değilse [`Self::seen_count`] 0'dır.
    pub v2: bool,
    /// Alıcının o ana dek gördüğü datagram sayısı (V2 `seen_count`; kümülatif, tekrarlar dahil).
    pub seen_count: u32,
    /// Çerçevenin teyit ettiği en büyük tel PN'i (aralıklar açıldıktan sonra); yoksa `None`.
    pub largest_pn: Option<u64>,
    /// `ack_delay` alanının tel değeri (V2'de 256 µs birim; `u16::MAX` alıcıdaki tavandır) ve
    /// çözülmüş değeri (µs): alıcıda en büyük PN'in varışından bu ACK'in üretimine geçen süre.
    pub ack_delay_raw: u16,
    pub ack_delay_us: u64,
    /// Çerçevenin teyit ettiği PN sayısı.
    pub acked_pns: u32,
    /// Bunlardan motor seq'ine çevrilenler (motora giden liste). 0 iken motor çerçeveyi işlemez:
    /// boş listede erken döner, son-ACK anını tazelemez.
    pub mapped_seqs: u32,
    /// Bu çerçeveyle İLK kez teyit edilen birim (teyitsiz birim deposundan düşen). 0 = çerçeve yeni
    /// hiçbir birimi onaylamadı. [K4G-05b] Teyit motorun teyit çıktısıdır
    /// (`Sender::take_confirmed_units`): motorun tanımadığı geç ACK (kaydı unutulmuş) sayılmaz.
    pub new_units: u32,
    /// Motorun bu olayda planlayıp mühürlettiği datagram (`ClientStats::tx_datagrams` farkı).
    pub tx_datagrams_during: u32,
    /// Motor görünümü: çerçeve motora verilmeden hemen önce ve olay işlendikten hemen sonra.
    pub before: MotorAckView,
    pub after: MotorAckView,
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Bir ACK çerçevesinin motora verilmeden ÖNCE okunan teşhis
/// alanları ([`AckFrameDiag`]'ın motor öncesi yarısı). [DUR-19] Yalnız canlı ayna açıkken
/// ([`RuntimeConfig::live_diag`]) kurulur; görev okumaz, yalnız kayda aktarır.
#[derive(Debug, Clone, Copy)]
struct AckFramePre {
    acked_pns: u32,
    largest_pn: Option<u64>,
    mapped_seqs: u32,
    before: MotorAckView,
    tx_before: u64,
}

/// Süre → µs, `u64`'e doymalı (teşhis kaydı alanları için).
fn saturating_us(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] ACK çerçevesi sayaçları ([`ClientLiveDiag::ack`]).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AckFrameCounts {
    /// Çözülen ACK çerçevesi (V1 + V2).
    pub frames: u64,
    /// En az bir PN'i motor seq'ine çevrilen çerçeve (motor çerçeveyi işledi).
    pub mapped: u64,
    /// En az bir birimi İLK kez teyit eden çerçeve (birim düzeyinde ilerleme).
    pub new_units: u64,
    /// Olaydan sonra motorun uçuştaki kayıt sayısı olaydan öncekinden küçük olan çerçeve. Kayıp
    /// ilanıyla ve tail-probe'un yerini aldığı orijinalle uçuştan çıkan kayıt da küçültür; aynı
    /// olayda planlanan emisyonlar küçülmeyi örtebilir ([`AckFrameDiag::tx_datagrams_during`]).
    pub inflight_shrunk: u64,
}

impl AckFrameCounts {
    /// Çözülen bir çerçeveyi sayar (salt gözlem; yalnız canlı ayna açıkken çağrılır).
    fn count(&mut self, mapped: bool, new_units: bool, inflight_shrunk: bool) {
        self.frames = self.frames.saturating_add(1);
        self.mapped = self.mapped.saturating_add(u64::from(mapped));
        self.new_units = self.new_units.saturating_add(u64::from(new_units));
        self.inflight_shrunk = self
            .inflight_shrunk
            .saturating_add(u64::from(inflight_shrunk));
    }

    fn add(&mut self, other: &Self) {
        self.frames = self.frames.saturating_add(other.frames);
        self.mapped = self.mapped.saturating_add(other.mapped);
        self.new_units = self.new_units.saturating_add(other.new_units);
        self.inflight_shrunk = self.inflight_shrunk.saturating_add(other.inflight_shrunk);
    }
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Gönderen motorun durum değişikliği. Görev her ana döngü turunun
/// başında motor durumunu okur ve öncekinden farklıysa kaydeder (çözünürlük: bir tur, yani bir
/// olay). Bir tur içinde birden çok geçiş yaşandıysa ara durumlar kayıtta yoktur;
/// [`Self::motor_transitions`] farkı bunu gösterir.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateChangeDiag {
    /// Kayıt sırası (1'den).
    pub index: u64,
    /// Görüldüğü an (ns; ucun saat tabanında).
    pub at_ns: u64,
    /// Önceki ve yeni durum (`from` ilk kayıtta `None`).
    pub from: Option<adaptive_transport::sender::SenderState>,
    pub to: Option<adaptive_transport::sender::SenderState>,
    /// Motorun o ana dek saydığı durum geçişi (`ActivationDiag::state_transitions`).
    pub motor_transitions: u64,
    /// O andaki acil durum seviyesi, son füzyon kararının acil ileri-kurtarma kipi ve füzyonun son
    /// gördüğü ack_confidence (‰). Motor RecoverFwd'e iki yoldan girer: seviye `Arq`'ın üstünde ya
    /// da füzyon kipi acil.
    pub emergency: Option<adaptive_transport::emergency::EmergencyLevel>,
    pub fusion_emergency_mode: bool,
    pub ack_confidence_permille: Option<u64>,
    /// Motorun son ACK'i işlediği an; hiç ACK yoksa `None`.
    pub last_ack_ns: Option<u64>,
}

/// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Gönderen motorun denetim durumu (M2: hız tabanı): faz, hız
/// kestiricileri, acil durum ve dönüş yolu. Kaynaklar mevcut salt-okunur erişimcilerdir
/// (`Sender::state`, `diag_activation`, `diag_pacer`, `emergency_level`, `last_decision`,
/// `diag_fusion_input_snapshot`, `diag_return_path`); motor olayı sürülmez.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SenderControlDiag {
    pub state: Option<adaptive_transport::sender::SenderState>,
    /// Motorun saydığı durum geçişi ve RecoverArq / RecoverFwd / ProbeUp girişleri.
    pub state_transitions: u64,
    pub recover_arq_entries: u64,
    pub recover_fwd_entries: u64,
    pub probe_up_entries: u64,
    /// BtlBw kestirimi (B/s).
    pub btlbw_bps: u64,
    /// Pacer hızı (B/s), jetonu ve patlama tavanı (B).
    pub pacer_rate_bps: u64,
    pub pacer_tokens: u64,
    pub pacer_burst_cap: u64,
    /// Uçuştaki bayt.
    pub inflight_bytes: u64,
    /// Kuyruk gecikmesi kestirimi (µs).
    pub queue_delay_us: u64,
    /// Acil durum seviyesi (`Arq < Dup2 < Dup3`).
    pub emergency: Option<adaptive_transport::emergency::EmergencyLevel>,
    /// Son füzyon kararı acil ileri-kurtarma kipinde mi; kararın hız kazancı (‰).
    pub fusion_emergency_mode: bool,
    pub send_rate_gain_permille: u32,
    /// Füzyonun son gördüğü girdiler (ack_confidence dahil); değerlendirme olmadıysa `None`.
    pub fusion: Option<adaptive_transport::fusion::FusionInputSnapshot>,
    /// Dönüş yolu gözlemi (aynanın yazıldığı anda).
    pub return_path: adaptive_transport::return_path::ReturnPathState,
}

impl SenderControlDiag {
    fn capture(
        sender: &adaptive_transport::sender::Sender,
        activation: &adaptive_transport::sender::ActivationDiag,
        now_ns: u64,
    ) -> Self {
        let (_, _, pacer_burst_cap) = sender.diag_pacer();
        let decision = sender.last_decision();
        Self {
            state: Some(sender.state()),
            state_transitions: activation.state_transitions,
            recover_arq_entries: activation.recover_arq_entries,
            recover_fwd_entries: activation.recover_fwd_entries,
            probe_up_entries: activation.probe_up_entries,
            btlbw_bps: activation.btlbw_bps,
            pacer_rate_bps: activation.pacer_rate_bps,
            pacer_tokens: activation.pacer_tokens,
            pacer_burst_cap,
            inflight_bytes: activation.inflight_bytes,
            queue_delay_us: activation.queue_delay_us,
            emergency: Some(sender.emergency_level()),
            fusion_emergency_mode: decision.mode
                == adaptive_transport::fusion::Mode::EmergencyForwardRedundancy,
            send_rate_gain_permille: decision.send_rate_gain_permille,
            fusion: sender.diag_fusion_input_snapshot(),
            return_path: sender.diag_return_path(now_ns),
        }
    }
}

impl ClientLiveDiag {
    /// Motorun ve tel sayaçlarının o anki değerlerini aynaya yazar (salt okuma). Kurtarma görüntüsü
    /// ([`Self::recovery`]) oranlı, halkalar olay anında ayrı yazılır.
    fn refresh(
        &mut self,
        sender: &adaptive_transport::sender::Sender,
        activation: &adaptive_transport::sender::ActivationDiag,
        stats: &ClientStats,
        control: SenderControlDiag,
        origin: Instant,
        now_ns: u64,
    ) {
        self.retransmits = activation.retransmits;
        self.tail_probes = activation.tail_probes;
        self.dups = activation.dups;
        self.parity_emitted = activation.parity_emitted;
        self.recovery_suppressed = stats.recovery_suppressed;
        self.tx_retx_bytes = stats.tx_retx_bytes;
        self.rtt_samples = sender.diag_rtt_sample_count();
        self.srtt_us = activation.srtt_us;
        self.rtprop_us = activation.rtprop_us;
        self.left_preconverge_ns = activation.left_preconverge_ns;
        self.clock_origin = Some(origin);
        self.at_ns = now_ns;
        self.control = control;
        self.tx_datagrams = stats.tx_datagrams;
        self.rx_datagrams = stats.rx_datagrams;
        self.tx_bytes = stats.tx_bytes;
    }

    /// [K4G-05] Kapanan bir ucun SAYAÇLARINI bu toplama ekler (S1 adaptörünün kapanmış oturumlar
    /// toplamı). RTT alanları toplanmaz, değişmez: uç başına anlamlıdır. [CEVAP PAKETİ 6] ACK
    /// çerçevesi sayaçları ve tel datagram / bayt sayaçları toplanır; denetim durumu, saat tabanı
    /// ve halkalar uç başınadır, toplanmaz.
    pub(crate) fn add_counters(&mut self, other: &Self) {
        self.retransmits = self.retransmits.saturating_add(other.retransmits);
        self.tail_probes = self.tail_probes.saturating_add(other.tail_probes);
        self.dups = self.dups.saturating_add(other.dups);
        self.parity_emitted = self.parity_emitted.saturating_add(other.parity_emitted);
        self.recovery_suppressed = self
            .recovery_suppressed
            .saturating_add(other.recovery_suppressed);
        self.tx_retx_bytes = self.tx_retx_bytes.saturating_add(other.tx_retx_bytes);
        self.tx_datagrams = self.tx_datagrams.saturating_add(other.tx_datagrams);
        self.rx_datagrams = self.rx_datagrams.saturating_add(other.rx_datagrams);
        self.tx_bytes = self.tx_bytes.saturating_add(other.tx_bytes);
        self.ack.add(&other.ack);
    }
}

/// Teslim olayı: hangi eşten hangi birim, payload'ıyla.
#[derive(Debug, Clone)]
pub struct Delivery {
    pub peer: SocketAddr,
    pub unit_id: u64,
    pub payload: Vec<u8>,
    /// [L-T1] Datagramı açan koruma bağlamı (eş yuvasının `open` ile kimliklendirilmiş bağlamı) —
    /// S1 adaptörü teslimi bununla oturuma atfeder.
    pub ctx: ProtectionContext,
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

// ───────────────────── BAĞLAM-ÖNCE DAĞITIM (K4G-01 / D1 v1.1) ─────────────────────
//
// İLKE: Bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer. Yük, bağlam doğrulanmadan
// çözülmez.
//
// `seam.open`'ın döndürdüğü `Opened::ctx`, `Opened::frame`'in (yükün) tek baytı okunmadan ÖNCE üç
// kola ayrılır (`CtxRoute`):
//   (a) `Session(h)` ve `h` bu ucun BEKLEDİĞİ tutamaçla birebir eşit → Session dağıtıcısı (istemci:
//       ACK / RECOVERED / CS_ECHO / PARAM_ACK; sunucu: DATA / FEC / PARAM / CS_PROBE, eş yuvası,
//       teslim, `last_seen_ns`). Yalnız "Session mı?" sorusu YETMEZ.
//   (b) `HubChannel` → `HubDispatch::on_hub_frame`: PoC'de yalnız `hub_frames_dropped` sayar;
//       hiçbir Session durumuna (ARQ deposu, PN çevirisi, eş/oturum tablosu) erişimi YOKTUR —
//       imzası bu durumlara referans almaz.
//   (c) başka her şey (başka oturumun ya da beklenmeyen tutamacın `Session(_)`'ı) → düşür,
//       `ctx_mismatch_drops`.
// "Çerçeve tipi → izinli bağlam" tablosu BİLİNÇLİ OLARAK YOKTUR ve yapılmaz: HubChannel yükü
// AEAD'sizdir (no-op hop kanalı), tipini de saldırgan seçer; tabloya satır eklemeyi unutan deliği
// yeniden açar. Bu yüzden yeni bir çerçeve tipi eklemek güvenlik kararı gerektirmez — bağlam önce
// seçilmiştir.
//
// "Beklenen" tutamaç BEKLENENDİR, ilk gelen datagramdan öğrenilmez (TOFU yok): istemci ucunda
// `connect_session`'a verilen bağlam; sunucu ucunda oturum tablosunun yayımladığı
// `ExpectedSessions` ve eş yuvasının bağlandığı bağlam.
//
// [K4G-01b] Eş yuvası `(adres, SessionHandle)` ile anahtarlıdır (`SlotKey`), adresle DEĞİL:
//   · `Session(h)` datagramı için yalnız `(kaynak adres, h)` yuvası okunur; aynı adresteki başka
//     bir oturumun yuvası bu datagramın dağıtımına, ARQ / teslim durumuna KATILMAZ.
//   · Yuva YALNIZ kol (a)'da — beklenen kümede `Ready` tutamaçla, AEAD-geçerli datagramla —
//     yaratılır (K4G-01 kapısı korunur). Aynı adresten yeni bir beklenen oturum → YENİ yuva
//     (yenileme, supersession, cancel+connect sonrası efemeral port yeniden kullanımı 105 s
//     düşmez).
//   · Yuvanın oturum bağı (`PeerSlot::ctx`) yaratılırken kurulur ve veri datagramıyla ASLA değişmez
//     (PoC-KAPSAM-001 K5 analoğu: bağ yalnız el sıkışmayla — yeni oturum = yeni yuva).
//   · Emeklilik: `idle_hold` dolunca YA DA bağlı oturum beklenen kümeden çıkınca (kapandı). İkinci
//     durumdaki yuva zaten ERİŞİLEMEZ — beklenmeyen oturumun datagramı yuvaya bakılmadan kol (c)'de
//     düşer —; emeklilik yalnız kapasiteyi boşaltır, hiçbir teslim / ACK davranışını değiştirmez.
//   · `max_peers` yuva sayısını sınırlar; tablo doluyken önce kapanmış oturumların yuvaları
//     emekliye ayrılır, yer yoksa yeni yuva fail-closed reddedilir (`dropped_new_peers`).
//   · Beyanlı açık (INT-02; PoC-KAPSAM-001 §5, kapanış M4 / adres doğrulama): hattaki saldırgan
//     gözlediği geçerli datagramı sahte adresten ÖNCE ileterek `max_peers` içinde ek yuva açabilir.
//     Ek yuvanın teslimi S1 adaptöründe oturum anahtarlı `Reorder`'da tekilleşir (çift teslim yok).
//
// INV-ACK-DELIVER-1 (plan Ek 2026-10-11 (3)): teslim yoluna kabul edilmeyen birim ACK'lenmez.
// Sunucu ucunda beklenen tutamacın HAZIRLIĞI da yayımlanır (`Readiness`): kol (a)'ya yalnız
// `Ready` (S1'e açılmış — teslim yolunun kabul ettiği) oturumun datagramı girer ve ACK'lenir.
// Beklenen ama hazır olmayan (`NotReady`: bekleyen INIT / yenileme, onaysız provisional, `Ready`
// olayı henüz işlenmemiş yanıtlayan) oturumun datagramı motora girmeden, ACK'siz düşer ve
// `not_ready_drops` artar; gönderen ARQ ile yeniden yollar, `Ready` sonrası kabul edilir.
// Tamponlama YOKTUR.

/// Bağlam belirtmeyen kurucuların (`connect`, `bind`) tek örtük PoC oturumu (`IdentitySeam`'in
/// oturumuyla aynı tutamaç; eski çağıranların davranışı birebir).
const IMPLICIT_SESSION: SessionHandle = SessionHandle::from_raw(0);

/// [K4G-01] Bağlam-önce dağıtımın iki telemetri sayacı. Klonlar AYNI sayaçları paylaşır: S1
/// adaptörü düğüm başına tek bir küme kurar ve veri alıcısına + her oturum ucuna verir (uç kapansa
/// da sayım kaybolmaz). Değerler canlı okunur ([`CtxDispatchCounters::snapshot`]).
#[derive(Debug, Clone, Default)]
pub struct CtxDispatchCounters(Arc<CtxDispatchCells>);

#[derive(Debug, Default)]
struct CtxDispatchCells {
    ctx_mismatch_drops: AtomicU64,
    hub_frames_dropped: AtomicU64,
    not_ready_drops: AtomicU64,
    full_refusals: AtomicU64,
    reorder_skip: AtomicU64,
    reorder_window_violations: AtomicU64,
    distance_holds: AtomicU64,
}

impl CtxDispatchCounters {
    /// Sıfırdan başlayan yeni küme.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Anlık değerler.
    #[must_use]
    pub fn snapshot(&self) -> CtxDispatchStats {
        CtxDispatchStats {
            ctx_mismatch_drops: self.0.ctx_mismatch_drops.load(Ordering::Relaxed),
            hub_frames_dropped: self.0.hub_frames_dropped.load(Ordering::Relaxed),
            not_ready_drops: self.0.not_ready_drops.load(Ordering::Relaxed),
            full_refusals: self.0.full_refusals.load(Ordering::Relaxed),
            reorder_skip: self.0.reorder_skip.load(Ordering::Relaxed),
            reorder_window_violations: self.0.reorder_window_violations.load(Ordering::Relaxed),
            distance_holds: self.0.distance_holds.load(Ordering::Relaxed),
        }
    }

    fn add_ctx_mismatch(&self) {
        self.0.ctx_mismatch_drops.fetch_add(1, Ordering::Relaxed);
    }

    fn add_hub_frame_dropped(&self) {
        self.0.hub_frames_dropped.fetch_add(1, Ordering::Relaxed);
    }

    fn add_not_ready(&self) {
        self.0.not_ready_drops.fetch_add(1, Ordering::Relaxed);
    }

    /// [K4G-05c · R2] Teslim kanalında yer yok: birim ACK'siz reddedildi.
    fn add_full_refusal(&self) {
        self.0.full_refusals.fetch_add(1, Ordering::Relaxed);
    }

    /// [K4G-05c · R2] Birim oturumun kabul penceresinin dışında: ACK'siz reddedildi.
    fn add_reorder_skip(&self) {
        self.0.reorder_skip.fetch_add(1, Ordering::Relaxed);
    }

    /// [K4G-05c · R1] S1 adaptörünün sıralama penceresi dışındaki birim (yapısal 0).
    pub(crate) fn add_reorder_window_violation(&self) {
        self.0
            .reorder_window_violations
            .fetch_add(1, Ordering::Relaxed);
    }

    /// [K4G-05c · R1] Gönderende birim-kimliği mesafe kapısının sunumu beklettiği bölüm sayısı.
    fn add_distance_hold(&self) {
        self.0.distance_holds.fetch_add(1, Ordering::Relaxed);
    }
}

/// [K4G-01] Bağlam-önce dağıtım sayaçlarının anlık görüntüsü (telemetri; u64, monoton).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CtxDispatchStats {
    /// Kol (c): `Session(_)` bağlamı beklenen tutamaçla birebir eşleşmeyen (başka oturumun ya da
    /// beklenmeyen tutamacın) datagram — yükü çözülmeden düşürüldü.
    pub ctx_mismatch_drops: u64,
    /// Kol (b): `HubChannel` bağlamında açılan datagram — `on_hub_frame` saydı ve düşürdü (PoC:
    /// HubChannel no-op, AEAD'siz; gerçek hub handler'ları hub sprintinde).
    pub hub_frames_dropped: u64,
    /// INV-ACK-DELIVER-1: tutamacı beklenen ama oturumu henüz `Ready` olmayan datagram — motora
    /// girmeden, ACK'siz düşürüldü; gönderen ARQ ile yeniden yollar. Güvenlik sayacı DEĞİLDİR:
    /// `Ready` / onay geçişleri çevresinde olağan akışta da artar (`ctx_mismatch_drops`'tan ayrı).
    pub not_ready_drops: u64,
    /// [K4G-05c · R2] Teslim kanalında yer olmadığı için ACK'siz reddedilen DATA / FEC kurtarımı
    /// (`Refuse(Full)`): birim alıcı motoruna, blok birleştiriciye ve kanala girmez; gönderen
    /// yeniden yollar. Tüketici durduğunda olağan akışta artar.
    pub full_refusals: u64,
    /// [K4G-05c · R2] Oturumun kabul penceresinin (`max_inflight_units`) dışında kaldığı için
    /// ACK'siz reddedilen birim (`Refuse(Window)`). Gönderenin mesafe kapısı bunu meşru trafikte
    /// önler; > 0 gönderen ihlalidir.
    pub reorder_skip: u64,
    /// [K4G-05c · R1] S1 adaptörünün sıralama penceresi dışında kalıp düşen birim. Veri alıcısının
    /// kabul penceresi bunu yapısal olarak 0 tutar; > 0 kabul kararının atlandığını gösterir.
    pub reorder_window_violations: u64,
    /// [K4G-05c · R1] Gönderende en yeni ile en eski teyitsiz birim arasındaki kimlik mesafesi
    /// `max_inflight_units`'e ulaştığı için bekleyen sunumun alınmadığı bölüm sayısı.
    pub distance_holds: u64,
}

/// [K4G-01 / INV-ACK-DELIVER-1] Beklenen bir tutamacın teslim hazırlığı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    /// Oturum S1'e açıldı ve teslim yolu birimini KABUL EDER (`Ready` sonrası, yalnız-alım drain
    /// dahil): datagram Session dağıtıcısına girer, ACK'lenir ve teslim edilir.
    Ready,
    /// Oturum bekleniyor ama teslim yolu henüz kabul ETMEZ (bekleyen INIT / yenileme, onaysız
    /// provisional, `Ready` olayı işlenmemiş yanıtlayan): datagram ACK'siz düşer.
    NotReady,
}

/// [K4G-01 / D1 (v1.1)] Sunucu ucunun BEKLEDİĞİ oturum tutamaçları ve teslim hazırlıkları.
///
/// Kaynağı oturum tablosudur, gelen datagram DEĞİLDİR (TOFU yok): S1 adaptörü
/// ([`crate::transport::ClientTransport`]) kümeyi kendi tablosundan yayımlar — bekleyen niyet,
/// yenileme, onay bekleyen provisional ve yanıtlayan rolde kurulan tutamaçlar `NotReady`, S1'e
/// açılmış (kapanmamış) oturumlar `Ready`; yeni bir tutamacı zarfı (INIT/RESPONSE) yola çıkmadan
/// ÖNCE ekler, yenileme/supersession/kapanışta günceller. Bağımsız `bind` / `bind_with_policy`
/// yalnız örtük PoC oturumunu (`Ready`) bekler. Klonlar AYNI kümeyi paylaşır.
///
/// [K4G-01b] Eş yuvalarının ömrü de bu kümeye bağlıdır: bağlı oturumu kümeden çıkan (kapanan)
/// `(adres, oturum)` yuvası `idle_hold`'u beklemeden emekliye ayrılır.
#[derive(Debug, Clone, Default)]
pub struct ExpectedSessions(Arc<Mutex<BTreeMap<SessionHandle, Readiness>>>);

impl ExpectedSessions {
    /// Boş küme: hiçbir `Session(_)` beklenmiyor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Yalnız `session`'ı (`Ready`) bekleyen küme.
    #[must_use]
    pub fn only(session: SessionHandle) -> Self {
        let expected = Self::new();
        expected.insert_ready(session);
        expected
    }

    /// `session` beklenir ve teslim yolu onu kabul eder.
    pub fn insert_ready(&self, session: SessionHandle) {
        self.lock().insert(session, Readiness::Ready);
    }

    /// `session` beklenir ama henüz hazır değildir. Zaten `Ready` ise DEĞİŞMEZ (hazırlık geri
    /// alınmaz; kaldırma yalnız tablonun yeniden yayınıyla olur).
    pub fn insert_pending(&self, session: SessionHandle) {
        self.lock().entry(session).or_insert(Readiness::NotReady);
    }

    /// Kümeyi TEK adımda yeniden yayımlar: okuyucu ara hâli görmez. Aynı tutamaç birden çok kez
    /// gelirse SONUNCUSU geçerlidir.
    pub fn replace(&self, sessions: impl IntoIterator<Item = (SessionHandle, Readiness)>) {
        let next: BTreeMap<SessionHandle, Readiness> = sessions.into_iter().collect();
        *self.lock() = next;
    }

    /// `session` bekleniyorsa hazırlığı; beklenmiyorsa `None`.
    #[must_use]
    pub fn readiness(&self, session: SessionHandle) -> Option<Readiness> {
        self.lock().get(&session).copied()
    }

    /// Zehirlenmeye karşı panik yapmaz: küme her işlemin sonunda tutarlıdır.
    fn lock(&self) -> MutexGuard<'_, BTreeMap<SessionHandle, Readiness>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// [K4G-01 / D1 (v1.1) kol (b)] HubChannel dağıtıcısı — Session dağıtıcısından AYRI bir tip.
///
/// Yalnız kendi (hub tarafı) durumunu taşır; bugün bu yalnız telemetri sayacıdır. ARQ deposu, PN
/// çevirisi, eş/oturum tablosu bu tipin alanı DEĞİLDİR ve [`HubDispatch::on_hub_frame`] onlara
/// referans ALMAZ. Kısıt bilinçlidir (D1 v1.1 TRADE-OFF): hub sprintinde gerçek handler'lar
/// (HANDLE_ASSIGN, hop ACK, heartbeat) eklenirken bu imza korunur.
struct HubDispatch {
    counters: CtxDispatchCounters,
}

impl HubDispatch {
    /// PoC (direct-mode; HubChannel no-op ve AEAD'siz, hop anahtarı M4'te): yük OKUNMAZ — tip
    /// baytı dahil —, yalnız `hub_frames_dropped` artar.
    fn on_hub_frame(&self, opened: &Opened) {
        let _ = opened;
        self.counters.add_hub_frame_dropped();
    }
}

/// [K4G-01 / D1 (v1.1)] Açılmış datagramın dağıtıcısı. YALNIZ `Opened::ctx` ile beklenen bağlamdan
/// türetilir; `Opened::frame`'e (yüke) bakılmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CtxRoute {
    /// (a) `Session(h)`, `h` beklenenle birebir eşit (sunucuda: `Ready`) → o oturumun Session
    /// dağıtıcısı.
    Session,
    /// (a') Sunucu: `Session(h)`, `h` beklenen ama `NotReady` → motora girmeden, ACK'siz düşür +
    /// `not_ready_drops` (INV-ACK-DELIVER-1).
    NotReady,
    /// (b) `HubChannel` → [`HubDispatch::on_hub_frame`].
    Hub,
    /// (c) başka her şey → düşür + `ctx_mismatch_drops`.
    Mismatch,
}

/// İstemci ucu: beklenen bağlam = `connect_session`'a verilen bağlam. `HubChannel`, beklenen bağlam
/// olsa bile Session dağıtıcısına GİRMEZ (kol (b) önce gelir).
fn route_client(opened: ProtectionContext, expected: ProtectionContext) -> CtxRoute {
    match opened {
        ProtectionContext::HubChannel => CtxRoute::Hub,
        ProtectionContext::Session(_) if opened == expected => CtxRoute::Session,
        ProtectionContext::Session(_) => CtxRoute::Mismatch,
    }
}

/// Sunucu ucu: `Session(h)` için eş yuvası varsa yuvanın bağlandığı bağlamla birebir eşleşmeli —
/// bir yuva tek oturuma aittir; başka bir oturumun geçerli datagramı o yuvanın ARQ / teslim
/// durumuna giremez — VE `h` oturum tablosunda beklenmeli ([`ExpectedSessions`]): `Ready` → kol
/// (a); `NotReady` → ACK'siz düşüş (INV-ACK-DELIVER-1); beklenmiyor → kol (c).
///
/// [K4G-01b] Çağıran `slot`'a `(kaynak adres, h)` anahtarlı yuvanın bağını verir; yuva yalnız kendi
/// anahtarının oturumuna bağlı yaratıldığından bağ ya yoktur ya `opened`'a eşittir. Eşitsizlik kolu
/// derinlemesine savunmadır (anahtar ileride değişirse yine kol (c)); aynı adresteki BAŞKA oturumun
/// yuvası buraya hiç verilmez.
fn route_server(
    opened: ProtectionContext,
    slot: Option<ProtectionContext>,
    expected: &ExpectedSessions,
) -> CtxRoute {
    match opened {
        ProtectionContext::HubChannel => CtxRoute::Hub,
        ProtectionContext::Session(_) if slot.is_some_and(|bound| bound != opened) => {
            CtxRoute::Mismatch
        }
        ProtectionContext::Session(h) => match expected.readiness(h) {
            Some(Readiness::Ready) => CtxRoute::Session,
            Some(Readiness::NotReady) => CtxRoute::NotReady,
            None => CtxRoute::Mismatch,
        },
    }
}

// ─────────────────────────────── CLIENT ───────────────────────────────

/// Client ucu: [`ClientSession`] (G2 sevk default'ları — tail-ON validated_v1) + UDP döngüsü.
pub struct ClientEndpoint;

/// Client tutamağı: bounded offer + ACK olay akışı + kontrollü kapanış.
pub struct ClientHandle {
    offer: ClientSender,
    acked_rx: mpsc::Receiver<u64>,
    shutdown_tx: watch::Sender<bool>,
    t0_tx: mpsc::Sender<oneshot::Sender<()>>,
    /// [L-T1] Kurtarma anahtarı (bkz. [`ClientHandle::set_recovery`]).
    recovery_tx: watch::Sender<bool>,
    /// [K4G-01] Bağlam-önce dağıtım sayaçları (bkz. [`ClientHandle::ctx_dispatch_stats`]).
    ctx_counters: CtxDispatchCounters,
    /// [K4G-05] Canlı salt-gözlem aynası (bkz. [`ClientHandle::live_diag`]). [DUR-19] Yalnız
    /// [`RuntimeConfig::live_diag`] açıkken kurulur; kapalıyken `None` (Mutex yok).
    live: Option<Arc<Mutex<ClientLiveDiag>>>,
    task: JoinHandle<ClientStats>,
}

/// Çok-üreticili offer tutamağı ([`ClientHandle::sender`]). ACK akışını `&mut` ile tutan
/// döngüden BAĞIMSIZ ödünç alınabilir (aynı `select!` içinde hem sunmak hem ACK drenajı).
#[derive(Clone)]
pub struct ClientSender {
    offer_tx: mpsc::Sender<(Vec<u8>, Option<u64>)>,
    /// [S1B] App-yüzü tavan MESAJ tavanıdır; birim (chunk) tavanı fragmenter'ın içinde kalır.
    max_message_bytes: usize,
    /// [L-T1] Kabul edilip henüz app-düzeyi teyidi gelmemiş mesaj sayısı: kabulde artar,
    /// teyitte (ACK ya da FEC-RECOVERED) azalır (bkz. [`ClientHandle::outstanding`]). [K4G-05b]
    /// Teyit motorun teyit çıktısıdır (`Sender::take_confirmed_units`).
    outstanding: Arc<AtomicUsize>,
}

impl ClientSender {
    /// Bir MESAJ sun (non-blocking; bounded). Dolu kuyruk → typed `QueueFull` (transport
    /// bloklamaz). Mesaj `max_unit_bytes`ten büyükse fragmenter chunk'lara böler; app bunu
    /// GÖRMEZ — teslim ve ACK yine mesaj granülünde olur.
    ///
    /// # Errors
    /// `UnitTooLarge` (bounds-before-alloc; cap = `max_message_bytes`) · `QueueFull` · `Closed`.
    pub fn send(&self, payload: Vec<u8>) -> Result<(), RuntimeError> {
        self.send_with_deadline(payload, None)
    }

    /// [DEADLINE-EKSENİ] Bir mesajı **teslim bütçesiyle** sunar.
    ///
    /// `deadline_ns` = bu mesajın app için hâlâ değerli olduğu süre (sunum anından itibaren).
    /// `None` = bütçe bildirilmedi (en iyi çaba).
    ///
    /// NEDEN VAR: FEC-MARGINAL-R v1+v2 (480 rep, ön-kayıtlı, replikasyonlu) ölçtü ki proaktif
    /// redundancy'nin değeri **kayıp oranına değil bu bütçenin RTT'ye oranına** bağlıdır. AYNI
    /// kanalda, AYNI %20 kayıpta: bütçe 2×RTT iken +10..+19 pp completion, 16×RTT iken ≈0 veya
    /// negatif — yani transport bu bilgiyi kendi başına TÜREMEZ, ve hiçbir kayıp eşiği kararı
    /// veremez. Bildirilmezse politika `NoDeadlineDeclared` ile ArqOnly'ye düşer.
    ///
    /// Bu bir POLİTİKA girdisidir: ARQ'yu, pacing'i, loss/RTT durumunu DEĞİŞTİRMEZ.
    ///
    /// # Errors
    /// `UnitTooLarge` · `QueueFull` · `Closed`.
    pub fn send_with_deadline(
        &self,
        payload: Vec<u8>,
        deadline_ns: Option<u64>,
    ) -> Result<(), RuntimeError> {
        if payload.len() > self.max_message_bytes {
            return Err(RuntimeError::UnitTooLarge {
                got: payload.len(),
                cap: self.max_message_bytes,
            });
        }
        self.offer_tx
            .try_send((payload, deadline_ns))
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => RuntimeError::QueueFull,
                mpsc::error::TrySendError::Closed(_) => RuntimeError::Closed,
            })?;
        self.outstanding.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

impl ClientEndpoint {
    /// Bağlan ve görevi başlat. `seam` = güvenlik dikişi (stub: [`crate::seam::IdentitySeam`]).
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind/connect hatası.
    pub async fn connect(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
    ) -> Result<ClientHandle, RuntimeError> {
        Self::connect_with_policy(server, cfg, seam, None).await
    }

    /// [`connect`](Self::connect) + opsiyonel egress kapısı ([`EgressPolicy`]). Politika `None`
    /// iken davranış `connect` ile BİREBİR aynıdır (kapı kodu hiç çalışmaz).
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind/connect hatası.
    pub async fn connect_with_policy(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
        egress: Option<Box<dyn EgressPolicy>>,
    ) -> Result<ClientHandle, RuntimeError> {
        // PoC: tek örtük oturum (`IdentitySeam::SESSION`) — eski çağıranların davranışı birebir.
        Self::connect_session(
            server,
            cfg,
            seam,
            egress,
            ProtectionContext::Session(IMPLICIT_SESSION),
        )
        .await
    }

    /// [L-T1] [`connect_with_policy`](Self::connect_with_policy) + koruma bağlamı: bu ucun HER
    /// çerçevesi `ctx` ile mühürlenir (S1 adaptörü oturum başına bir uç açar; `ctx` =
    /// `ProtectionContext::Session(h)`).
    ///
    /// [K4G-01 / D1 (v1.1)] `ctx` aynı zamanda bu ucun BEKLEDİĞİ bağlamdır: açılan datagramın
    /// yükü YALNIZ bağlamı birebir `ctx` ise Session dağıtıcısına girer (bkz. modüldeki bağlam-önce
    /// dağıtım bölümü). Sayaçlar bu uca özeldir; paylaşımlı küme için
    /// [`connect_session_counted`](Self::connect_session_counted).
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind/connect hatası.
    pub async fn connect_session(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
        egress: Option<Box<dyn EgressPolicy>>,
        ctx: ProtectionContext,
    ) -> Result<ClientHandle, RuntimeError> {
        Self::connect_session_counted(server, cfg, seam, egress, ctx, CtxDispatchCounters::new())
            .await
    }

    /// [K4G-01] [`connect_session`](Self::connect_session) + paylaşımlı bağlam-önce dağıtım
    /// sayaçları: S1 adaptörü düğüm başına TEK bir [`CtxDispatchCounters`] kurar ve veri alıcısına
    /// ve her oturum ucuna verir (uç kapansa da sayım kaybolmaz).
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind/connect hatası.
    pub async fn connect_session_counted(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
        egress: Option<Box<dyn EgressPolicy>>,
        ctx: ProtectionContext,
        ctx_counters: CtxDispatchCounters,
    ) -> Result<ClientHandle, RuntimeError> {
        validate_fec(&cfg)?; // [S1] koşu-öncesi typed FEC-config doğrulaması
        let sock = UdpSocket::bind(match server {
            SocketAddr::V4(_) => "0.0.0.0:0",
            SocketAddr::V6(_) => "[::]:0",
        })
        .await?;
        sock.connect(server).await?;
        let (offer_tx, offer_rx) = mpsc::channel::<(Vec<u8>, Option<u64>)>(cfg.offer_queue);
        let (acked_tx, acked_rx) = mpsc::channel(cfg.offer_queue.max(cfg.max_inflight_units));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Kapasite 1 + onay-oneshot: mark_t0 görev işleyene dek bekler (ölçüm sözleşmesi).
        let (t0_tx, t0_rx) = mpsc::channel::<oneshot::Sender<()>>(1);
        let max_message_bytes = cfg.max_message_bytes.unwrap_or(cfg.max_unit_bytes);
        let (recovery_tx, recovery_rx) = watch::channel(true);
        let outstanding = Arc::new(AtomicUsize::new(0));
        // [K4G-05 / DUR-19] Canlı ayna yalnız bayrak açıkken kurulur (kapalıyken Mutex yok).
        let live = cfg
            .live_diag
            .then(|| Arc::new(Mutex::new(ClientLiveDiag::default())));
        let task = tokio::spawn(client_task(
            sock,
            cfg,
            seam,
            egress,
            ClientTaskLinks {
                offer_rx,
                acked_tx,
                shutdown_rx,
                t0_rx,
                ctx,
                recovery_rx,
                outstanding: Arc::clone(&outstanding),
                ctx_counters: ctx_counters.clone(),
                live: live.clone(),
            },
        ));
        Ok(ClientHandle {
            offer: ClientSender {
                offer_tx,
                max_message_bytes,
                outstanding,
            },
            acked_rx,
            shutdown_tx,
            t0_tx,
            recovery_tx,
            ctx_counters,
            live,
            task,
        })
    }
}

impl ClientHandle {
    /// Bir birim sun (non-blocking; bounded). Dolu kuyruk → typed `QueueFull` (transport bloklamaz).
    ///
    /// # Errors
    /// `UnitTooLarge` · `QueueFull` · `Closed` — bkz. [`ClientSender::send`].
    pub fn send(&self, payload: Vec<u8>) -> Result<(), RuntimeError> {
        self.offer.send(payload)
    }

    /// [DEADLINE-EKSENİ] Bkz. [`ClientSender::send_with_deadline`].
    ///
    /// # Errors
    /// `UnitTooLarge` · `QueueFull` · `Closed`.
    pub fn send_with_deadline(
        &self,
        payload: Vec<u8>,
        deadline_ns: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.offer.send_with_deadline(payload, deadline_ns)
    }

    /// Klonlanabilir offer tutamağı (bkz. [`ClientSender`]).
    #[must_use]
    pub fn sender(&self) -> ClientSender {
        self.offer.clone()
    }

    /// Ölçüm-t0 işareti: **wire sayaçları sıfırlanır, TRANSPORT DURUMU KORUNUR**
    /// (counter-reset-without-state-reset) ve varsa [`EgressPolicy`] arm edilir — bu andan
    /// öncesi (kuruluş/warmup) kapıdan muaftır. App-düzeyi sayaçlara (`offered`/`acked`/FEC)
    /// DOKUNMAZ.
    ///
    /// **Görev İŞLEYENE KADAR bekler.** Bu bir kolaylık değil, ölçüm sözleşmesidir: dönüşten
    /// sonra sunulan HİÇBİR birim arm-öncesi muafiyetten yararlanamaz. (Ateşle-unut bir işaret
    /// bunu garanti edemezdi — `select!` t0 ile bekleyen bir offer arasında rastgele seçer ve
    /// ilk datagram bütçesiz kaçardı; sıfır-bütçe kolunda bir teslim olarak yakalandı.)
    /// Görev ölmüşse sessizce döner.
    pub async fn mark_t0(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.t0_tx.send(ack_tx).await.is_ok() {
            let _ = ack_rx.await;
        }
    }

    /// İlk-ACK olay akışı (birim id'leri; ilk-ack-kazanır).
    ///
    /// [K4G-05b · CEVAP 13 §3 / 11-EK §4] Olay MOTOR TEYİDİNDE gelir (`Sender::take_confirmed_units`):
    /// motorun kaydını unuttuğu bir iletimin geç ACK'i teyit etmez — olay, birimin yeniden
    /// gönderiminin ACK'iyle gelir. Tanınmayan geç ACK ek yeniden gönderimlere yol açabilir;
    /// payload sahipliği korunur.
    pub async fn acked(&mut self) -> Option<u64> {
        self.acked_rx.recv().await
    }

    /// [L-T1] Kabul edilmiş ama app-düzeyi teyidi (ACK ya da FEC-RECOVERED) henüz gelmemiş mesaj
    /// sayısı. S1 adaptörü atomik cutover'dan önce eski oturumun boşaldığını buradan okur.
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.offer.outstanding.load(Ordering::Acquire)
    }

    /// [L-T1] Kurtarma anahtarı — **yalnız test yapılandırması** (ürün builder'ı kapalı kurtarmayı
    /// reddeder; S1 `TransportCapabilities::reliable`). Kapalıyken motorun ARQ retransmit, §11
    /// kopya ve FEC repair kararları tele ÇIKMAZ (`ClientStats::recovery_suppressed`); ilk
    /// emisyonlar ve kontrol çerçeveleri etkilenmez. Motor emisyonu zaten işlediğinden çıkmayan karar kayıp
    /// sayılır (INV-RATE-1: hız düşmez). Varsayılan: açık.
    pub fn set_recovery(&self, on: bool) {
        let _ = self.recovery_tx.send(on);
    }

    /// [K4G-01 / D1 (v1.1)] Bağlam-önce dağıtımın canlı sayaçları (telemetri): `ctx_mismatch_drops`
    /// ve `hub_frames_dropped`. Uç paylaşımlı bir küme aldıysa
    /// ([`ClientEndpoint::connect_session_counted`]; S1 adaptörü) değerler o kümenindir — düğüm
    /// geneli toplam.
    #[must_use]
    pub fn ctx_dispatch_stats(&self) -> CtxDispatchStats {
        self.ctx_counters.snapshot()
    }

    /// [K4G-05] Salt-okunur: motorun ARQ / kurtarma sayaçları ve RTT kestiricisinin CANLI aynası
    /// (bkz. [`ClientLiveDiag`]), kapanışı beklemeden okunur. Yayın: görev her ana döngü turunun
    /// BAŞINDA yazar — okuyucu en çok bir tur (pacing tick'i 1 ms ya da tek bir `select!` kolunun
    /// işi) geride kalır. Kapanış boşaltmasının değerleri aynaya girmez; kesin değer
    /// [`ClientHandle::shutdown`]'ın istatistiğidir. Görev başlamadan önce sıfırdır.
    ///
    /// [DUR-19] Ayna YALNIZ [`RuntimeConfig::live_diag`] açıkken vardır. Kapalıyken (ürün
    /// varsayılanı) görev yazmaz ve bu çağrı kilit almadan HER ZAMAN [`ClientLiveDiag::default`]
    /// döner — bütün sayaçlar ve RTT alanları 0, `recovery` / `clock_origin` / `control.state`
    /// `None`, halkalar boş; değer "trafik yok" ANLAMINA GELMEZ. Ayırt etmek için
    /// [`Self::live_diag_enabled`].
    #[must_use]
    pub fn live_diag(&self) -> ClientLiveDiag {
        match &self.live {
            Some(live) => *live.lock().unwrap_or_else(PoisonError::into_inner),
            None => ClientLiveDiag::default(),
        }
    }

    /// [K4G-05 / DUR-19] Bu ucun canlı aynası var mı: uç kurulurken [`RuntimeConfig::live_diag`]
    /// açıktı. `false` iken [`Self::live_diag`] daima [`ClientLiveDiag::default`] döner.
    #[must_use]
    pub fn live_diag_enabled(&self) -> bool {
        self.live.is_some()
    }

    /// Kontrollü kapanış: drain (pif→0 VEYA `drain_cap`) → istatistik. Görev panik ederse
    /// varsayılan istatistik döner (panik yutulmaz: ayrı gözlemlenebilir, unwrap edilmez).
    pub async fn shutdown(self) -> ClientStats {
        let _ = self.shutdown_tx.send(true);
        self.task.await.unwrap_or_default()
    }
}

/// Client görevi: bench `client_loop` şekli, ölçüm makinesi çıkarılmış, sınırlar eklenmiş hâli.
#[allow(clippy::too_many_arguments)]
/// Client görevinin kanal uçları + oturum bağlamı (argüman listesi sınırlı kalsın diye bir arada).
struct ClientTaskLinks {
    offer_rx: mpsc::Receiver<(Vec<u8>, Option<u64>)>,
    acked_tx: mpsc::Sender<u64>,
    shutdown_rx: watch::Receiver<bool>,
    t0_rx: mpsc::Receiver<oneshot::Sender<()>>,
    /// [L-T1] Bu ucun koruma bağlamı (her çerçeve bununla mühürlenir).
    ctx: ProtectionContext,
    /// [L-T1] Kurtarma anahtarı (bkz. [`ClientHandle::set_recovery`]).
    recovery_rx: watch::Receiver<bool>,
    /// [L-T1] Teyitsiz mesaj sayacı (bkz. [`ClientHandle::outstanding`]).
    outstanding: Arc<AtomicUsize>,
    /// [K4G-01] Bağlam-önce dağıtım sayaçları (bkz. [`ClientHandle::ctx_dispatch_stats`]).
    ctx_counters: CtxDispatchCounters,
    /// [K4G-05] Canlı salt-gözlem aynası (bkz. [`ClientHandle::live_diag`]); görev yalnız YAZAR.
    /// [DUR-19] `None` = [`RuntimeConfig::live_diag`] kapalı: görev hiçbir teşhis yazımı yapmaz.
    live: Option<Arc<Mutex<ClientLiveDiag>>>,
}

/// [K4G-05b · CEVAP 13 §3] Motor teyidinin geldiği olay sınıfı — yalnız sayaç ayrımı
/// ([`ClientStats::acked`] / [`ClientStats::fec_recovered_ctrl_units`]). Motor sözleşmesi gereği
/// teyit yalnız ACK ve FEC RECOVERED olaylarından çıkar (`Sender::take_confirmed_units`); başka
/// bir olaydan sonraki drenaj boştur, boş değilse ACK sınıfına sayılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmVia {
    Ack,
    Recovered,
}

impl ConfirmVia {
    fn of(event: &Event) -> Self {
        if matches!(event, Event::RecoveredControl(_)) {
            Self::Recovered
        } else {
            Self::Ack
        }
    }
}

async fn client_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    mut seam: Box<dyn PacketProtection>,
    mut egress: Option<Box<dyn EgressPolicy>>,
    links: ClientTaskLinks,
) -> ClientStats {
    let ClientTaskLinks {
        mut offer_rx,
        acked_tx,
        mut shutdown_rx,
        mut t0_rx,
        ctx: client_ctx,
        recovery_rx,
        outstanding,
        ctx_counters,
        live,
    } = links;
    // [K4G-01 / D1 (v1.1)] HubChannel dağıtıcısı: Session durumunun hiçbirini TUTMAZ (yalnız
    // sayaç).
    let hub = HubDispatch {
        counters: ctx_counters.clone(),
    };
    let start = Instant::now();
    // G2 sevk default'ları TEK kaynaktan: ClientSession (tail-ON validated_v1, startup-pacing motor
    // default'u ON, bootstrap OFF, FEC Manual-off). Burada default'a dokunulmaz.
    let mut session = ClientSession::new();
    // [BURST-FUSION] Yapı-gate politikası config'ten; default sevk davranışıdır (byte-özdeş).
    session
        .sender_mut()
        .set_fec_structure_policy(cfg.fec_structure_policy);
    // [DR-CS-3] Bootstrap-RTT merdiveni: emitter motorun sevk yolunda ZATEN var, yalnız bu
    // bayrağa bağlıydı ve hiçbir üretim çağıranı yoktu (7 çağrı yerinin hepsi testti).
    session
        .sender_mut()
        .set_bootstrap_rtt(if cfg.bootstrap_rtt {
            let mut c = if cfg.bootstrap_fast_ladder {
                BootstrapRttConfig::establishment_fast()
            } else {
                BootstrapRttConfig::default()
            };
            c.enabled = true;
            c
        } else {
            BootstrapRttConfig::default()
        });
    // [OFF = OFF] Ürün anahtarı HER cold-start probe lane'ini kapsar: DR-CS-1 M2 (F2A) merdiveni de
    // (feature derlenmişse) `bootstrap_rtt` ile açılır/kapanır. Belge: "false ⇒ motor probe PLANLAMAZ".
    session.sender_mut().set_cs_probe_lane(cfg.bootstrap_rtt);
    // [ONAY UFKU] Ledger confirm ufku kolu; default sevk davranışı (byte-özdeş).
    session
        .sender_mut()
        .set_loss_confirm_horizon(cfg.loss_confirm_horizon);
    // [FEC-RISK] Deneysel risk tahmincisi; default KAPALI (semantik özdeş).
    session.sender_mut().set_fec_loss_risk(cfg.fec_loss_risk);
    // [R-SWEEP] gate seviyesi override'ı (None = sevk).
    session
        .sender_mut()
        .set_structure_gate_level(cfg.structure_gate_level_permille);
    // [DEADLINE-EKSENİ] Kalibre politika (opt-in). Kablolanmamış bir karar katmanı bu reponun
    // kronik kusuruydu: `fusion_policy` üretimde SIFIR çağıranla duruyordu. Burası o kablodur.
    if let Some(p) = cfg.fusion_policy {
        session.sender_mut().set_fusion_policy(p);
    }
    // [IW] İlk-uçuş penceresi (0 = sevk davranışı, clamp'e dokunulmaz).
    session
        .sender_mut()
        .set_initial_window(InitialWindowConfig {
            bytes: cfg.initial_window_bytes,
        });
    let mut stats = ClientStats::default();
    let mut ts_last_ns: u64 = 0;
    // [S1] FEC-runtime kuruluşu (cfg.fec varsa): Controller-mode — yapı-gate'in (Q18) kararı
    // latch'e ancak bu modda akar. validate_fec connect'te koştu; profil burada çözülür.
    let fec: Option<FecProfile> = cfg.fec.and_then(|f| by_id(f.profile_id));
    let fec_manual_r: Option<u16> = cfg.fec.and_then(|f| f.manual_r);
    if fec.is_some() {
        let mode = if fec_manual_r.is_some() {
            FecActuationMode::Manual { enabled: true } // statik-r (STATIC_R kolu)
        } else {
            FecActuationMode::Controller // adaptif: yapı-gate (Q18) kararı latch'e akar
        };
        session
            .sender_mut()
            .set_fec_actuation_mode(mode)
            .expect("taze Sender mode-set kabul eder");
    }
    // [v2 / S3 K1] Koruma bağlamı. PN'i artık SEAM sayar (S3 K2) — burada sayaç YOK.
    // PoC: tek örtük oturum. Gerçek tutamaç `SessionControl`'ün `Ready`/`Established`'inden gelir
    // (A5.5); engine çerçevelerinin `HubChannel`'a ayrılması WS-D1'dir. Bugün hepsi tek bağlamda
    // mühürlenir — v1 ile aynı davranış.
    let param_plan = fec
        .as_ref()
        .and_then(|p| plan_param_bootstrap(1, &param_fields_for(p)).ok());
    let mut param_confirmed = param_plan.is_none(); // FEC-yoksa PARAM-akışı yok.
    let mut param_last_tick_ms: u64 = 0;
    let mut param_tries: u32 = 0;
    // [S1B] Fragmenter durumu. `frag_on` = mesaj tavanı birim tavanından BÜYÜK. Kapalıyken bu
    // haritalar hiç dokunulmaz ve gönderim yolu 0x00 DATA ile birebir aynıdır (bankadaki tüm
    // ölçümler geçerli kalır); açıkken mesaj ardışık chunk-birimlere bölünür ve app'e ACK
    // MESAJ tamamlanınca verilir.
    let frag_on = cfg.max_message_bytes.is_some();
    let mut next_message: u32 = 0;
    let mut unit_msg: HashMap<u64, u32> = HashMap::new();
    let mut msg_pending: HashMap<u32, u32> = HashMap::new();
    // [S1] Ardışık-blok birikimi: unit_id → (block = id/K, esi = id%K) deterministik (iki uç da
    // türetir); yalnız SON blok birikir. K dolunca same-event latch + gerçek RS-encode →
    // bounded repair_store (en-eski düşer → o bloğun FecParity kararı başlık-only'e düşer).
    let mut cur_block: Vec<Vec<u8>> = Vec::new();
    let mut cur_block_id: u32 = 0;
    // [FAZ-2 / KALEM-1] blok → (K_actual, repair'ler). K_actual kısmi/son blokta k_shape'ten
    // küçüktür ve TEL BAŞLIĞINDA taşınmak zorundadır (alıcı Model-A zero-enjeksiyonunu ondan türetir).
    let mut repair_store: BTreeMap<u32, (u8, Vec<RepairSymbol>)> = BTreeMap::new();
    const REPAIR_STORE_BLOCKS: usize = 64;
    // unit-store: unit_id → payload; varlık = outstanding (sınır: cfg). [K4G-05b · CEVAP 13 §3]
    // YALNIZ motor teyidinde düşer (`drain_confirmed!`): ACK'teki seq'in runtime haritasındaki
    // birimi silme gerekçesi DEĞİLDİR — motor kaydı unuttuysa (tombstone TTL / tahliye / kopya
    // kökenli kayıp / K_MAP) o geç ACK teyit etmez, birim motorda sahipli kalır ve yeniden gider.
    let mut store: HashMap<u64, Vec<u8>> = HashMap::new();
    // seq → unit haritası + birim → seq'leri (teyitte toplu budama — harita store disipliniyle
    // sınırlı kalır; teyitli birime kayıt eklenmez).
    let mut seq_to_unit: HashMap<u64, u64> = HashMap::new();
    let mut unit_seqs: HashMap<u64, Vec<u64>> = HashMap::new();
    // [D1a] ARQ kimliği = `meta.full_wire_pn` (BL D1; S3 K2/K4): her (yeniden) gönderim yeni bir
    // seal → yeni PN; alıcı ACK_V2 aralıklarını PN uzayında kurar. Motor kendi seq uzayında KALIR
    // (motor değişmez): bu tablo PN → motor seq çevirir. PN'ler artan sırada eklenir (ikili arama);
    // `PN_SEQ_WINDOW` gerisi budanır. Motor kararı taşımayan çerçeveler (PARAM, CS-probe) PN
    // tüketir ama tabloya girmez — alıcı onları ACK'lemez, ACK'lense de çevrilmez.
    let mut pn_seq: VecDeque<(u64, u64)> = VecDeque::new();
    let mut max_sealed_pn: Option<u64> = None;
    // [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] ACK çerçevesi sayaçları (yalnız aynaya yazılır; görev
    // okumaz).
    let mut ack_counts = AckFrameCounts::default();
    let mut next_unit: u64 = 0;
    let mut pace = interval(PACE_TICK);
    let mut rtprop = interval(RTPROP_TICK);
    let mut buf = vec![0u8; CLIENT_RECV_BUF];

    // Motoru bir olayla sürer + kararları mühürleyip wire'a yazar. seq→unit kaydı GÖNDERİMDEN
    // BAĞIMSIZ önce yapılır (refused/failed send de seq'ini işgal eder). Send hatası = kanal kaybı
    // (INV-RATE-1; abort YOK).
    // [v2] Tek-nokta seal+send: yapısal seal-reddi sayılır; PN'i seam atar (yalnız başarılı seal'de).
    // [D1a] Değeri: başarılı seal'in PN'i (egress reddi / send hatası da PN'i tüketmiş sayılır —
    // kanal kaybı), seal reddinde `None`.
    macro_rules! seal_send {
        ($frame:expr, $class:expr, $now:expr) => {
            match seam.seal(client_ctx, &$frame) {
                Ok(sd) => {
                    let pn = sd.meta.full_wire_pn;
                    max_sealed_pn = Some(max_sealed_pn.map_or(pn, |m: u64| m.max(pn)));
                    let n = sd.bytes.len();
                    let n64 = n as u64;
                    // DENEME düzlemi: kapı reddetse de üretilen byte sayılır (sınıf ayrımıyla).
                    stats.tx_datagrams += 1;
                    stats.tx_bytes += n64;
                    match $class {
                        TxClass::Data => stats.tx_data_bytes += n64,
                        TxClass::Retx => stats.tx_retx_bytes += n64,
                        TxClass::Dup => stats.tx_dup_bytes += n64,
                        TxClass::Fec => stats.tx_fec_bytes += n64,
                        TxClass::Ctrl => stats.tx_ctrl_bytes += n64,
                    }
                    // Egress kapısı: ret = kanal-kaybı eşdeğeri. Motor durumu emisyonu ZATEN
                    // işledi; geri itilmez, rate'e dokunulmaz (INV-RATE-1).
                    let permitted = match egress.as_mut() {
                        Some(p) => p.admit(n, $now),
                        None => true,
                    };
                    if permitted {
                        if sock.send(&sd.bytes).await.is_err() {
                            stats.send_errors += 1; // kayıp; asla geri itme (INV-RATE-1).
                        }
                    } else {
                        stats.egress_refused_pkts += 1;
                        stats.egress_refused_bytes += n64;
                    }
                    Some(pn)
                }
                Err(_) => {
                    stats.seal_failures += 1; // typed-red; emisyon kayıp sayıldı.
                    None
                }
            }
        };
    }
    // [D1a] Motor kararının PN'ini çeviri tablosuna yazar + PN ufkunun gerisini budar.
    macro_rules! map_pn {
        ($pn:expr, $seq:expr) => {
            if let Some(pn) = $pn {
                pn_seq.push_back((pn, $seq));
                while pn_seq
                    .front()
                    .is_some_and(|&(old, _)| pn.saturating_sub(old) >= PN_SEQ_WINDOW)
                {
                    pn_seq.pop_front();
                }
            }
        };
    }
    // [S1B] Bir BİRİMİN teslim-teyidi app'e nasıl yansır.
    //
    // Fragmentasyon KAPALIYKEN birim = mesaj → id doğrudan akar (eski davranış birebir).
    // AÇIKKEN app mesaj granülünde konuşur: birim teyidi mesajın kalan-chunk sayacını düşürür ve
    // ACK yalnız SON chunk teyitlendiğinde verilir — yarım mesaj "teslim edildi" diye
    // raporlanamaz. [K4G-05b] Tek çağıranı teyit drenajıdır (`drain_confirmed!`): ACK ve
    // FEC-RECOVERED teyitleri motordan aynı noktadan gelir.
    macro_rules! unit_acked {
        ($unit:expr) => {
            if frag_on {
                if let Some(m) = unit_msg.remove(&$unit) {
                    if let Some(rem) = msg_pending.get_mut(&m) {
                        *rem = rem.saturating_sub(1);
                        if *rem == 0 {
                            msg_pending.remove(&m);
                            stats.messages_acked += 1;
                            let _ = outstanding.fetch_update(
                                Ordering::AcqRel,
                                Ordering::Acquire,
                                |v| Some(v.saturating_sub(1)),
                            );
                            let _ = acked_tx.try_send(u64::from(m));
                        }
                    }
                }
            } else {
                let _ = outstanding.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                    Some(v.saturating_sub(1))
                });
                // app drenajı; taşarsa olay düşer, teslim durumu store'da zaten kesin.
                let _ = acked_tx.try_send($unit);
            }
        };
    }
    // [K4G-05b · CEVAP 13 §3 / 11-EK §3] TEYİT DRENAJI — payload'ı silen ve app-düzeyi teyidi
    // (`acked()` akışı, `outstanding` düşümü, `stats.acked` / `stats.fec_recovered_ctrl_units`)
    // üreten TEK yol: motorun teyit çıktısı (`Sender::take_confirmed_units`; birim oturumda en çok
    // bir kez). Motorun tanımadığı geç ACK (kaydı tombstone TTL'i, tahliye, kopya kökenli kayıp ya
    // da K_MAP ile düşmüş) teyit üretmez: payload yerinde kalır, sonraki yeniden gönderim gövdeli
    // gider ve onun ACK'i teyit eder (`docs/design/K4G-05b-RUNTIME-ENVANTER.md`). Birimin
    // haritaları aynı yerde budanır. Değer: bu drenajda store'dan düşen birim sayısı.
    macro_rules! drain_confirmed {
        ($via:expr) => {{
            let mut newly: u32 = 0;
            for unit in session.sender_mut().take_confirmed_units() {
                for old in unit_seqs.remove(&unit).unwrap_or_default() {
                    seq_to_unit.remove(&old);
                }
                // Store'da olmayan birimin teyidi motor sözleşmesince erişilemez (birim yalnız
                // teyitle düşer, teyit birim başına bir kez); app olayı üretilmez.
                if store.remove(&unit).is_none() {
                    continue;
                }
                match $via {
                    ConfirmVia::Recovered => stats.fec_recovered_ctrl_units += 1,
                    ConfirmVia::Ack => stats.acked += 1,
                }
                newly = newly.saturating_add(1);
                unit_acked!(unit);
            }
            newly
        }};
    }
    // Değeri: olayın teyit drenajında store'dan düşen birim sayısı (`drain_confirmed!`).
    macro_rules! drive {
        ($ev:expr, $now:expr) => {{
            let ev: Event = $ev;
            let via = ConfirmVia::of(&ev);
            let decisions = session.on_event(ev, $now);
            // [K4G-05b] Teyit drenajı kararlar işlenmeden ÖNCE: motor aynı olayda teyit ettiği
            // birime karar üretmemelidir; üretirse karar aşağıda store-miss olarak sayılır
            // (`stale_decisions`, panik yok) ve birime yeni harita kaydı açılmaz.
            let confirmed_now: u32 = drain_confirmed!(via);
            for d in decisions {
                // PN → seq çevirisi motor seq'i taşıyan HER kararda (repair'ler de seq işgal eder;
                // `map_pn!`); ama seq→unit haritası YALNIZ birim-taşıyan kararlarda: FecParity'nin
                // unit_id'si ESI'dir (AYRI ad-uzayı) — haritaya girseydi repair-ACK'i gerçek bir
                // birimin store'unu silerdi (S1'de yakalanan tuzak).
                let carries_unit = matches!(
                    d.kind,
                    TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
                );
                if carries_unit {
                    // [K4G-05b] Teyitli (store'dan düşmüş) birim için kayıt EKLENMEZ.
                    if store.contains_key(&d.unit_id) {
                        seq_to_unit.insert(d.seq, d.unit_id);
                        unit_seqs.entry(d.unit_id).or_default().push(d.seq);
                    }
                } else {
                    // [S1] FecParity (FEC-açıkken): repair-store'daki gerçek RS-repair →
                    // FRAME_FEC → seam → wire. Bulunamazsa (evict/latch-0) başlık-only emisyon
                    // (DATA store-miss gerekçesiyle aynı: motor her emisyonu inflight izler; hiç
                    // göndermemek yapay kayıp sinyali üretirdi).
                    if d.kind == TransmitKind::FecParity {
                        // [L-T1] Kurtarma kapalıyken FEC repair da tele ÇIKMAZ: parity bir kurtarma
                        // mekanizmasıdır (ARQ ile aynı anahtar; yalnız test yapılandırması).
                        if !*recovery_rx.borrow() {
                            stats.recovery_suppressed += 1;
                            continue;
                        }
                        if let (Some(p), Some(hint)) = (fec.as_ref(), d.send_record) {
                            let SrLogicalRef::Fec { block_id, esi } = hint.logical_ref;
                            let entry = repair_store.get(&block_id);
                            let sym: Option<&[u8]> = entry
                                .and_then(|(_, rs)| rs.iter().find(|r| r.esi == esi))
                                .map(|r| r.bytes.as_slice());
                            // [FAZ-2 / KALEM-1] K_actual tel başlığında: tam blokta k_shape,
                            // kısmi/son blokta gerçek kaynak sayısı.
                            let k_actual_wire = entry
                                .map_or_else(|| u8::try_from(p.k_shape).unwrap_or(255), |(k, _)| *k);
                            if sym.is_some() {
                                stats.fec_repairs_tx += 1;
                            } else {
                                stats.fec_repair_missing += 1;
                            }
                            let frame = encode_fec(
                                &FecFrameHeader {
                                    kind: inner_kind::FEC_REPAIR,
                                    profile_id: p.profile_id,
                                    block_id,
                                    esi,
                                    k_actual: k_actual_wire,
                                    // [D1a] ara çerçeve alanı; kimlik DEĞİL (alıcı PN'i kullanır).
                                    seq: (d.seq & 0xFFFF) as u16,
                                },
                                sym.unwrap_or(&[]),
                            );
                            let pn = seal_send!(frame, TxClass::Fec, $now);
                            map_pn!(pn, d.seq);
                            continue;
                        }
                    }
                    // [DR-CS-3] CS-probe: motor PreConverge'de RTT örneği üretmek için planlar.
                    // probe_seq AYRI ad-uzayındadır (`bs_maybe_emit_probe`: "NOT a DATA seq, NOT
                    // inflight-tracked") — seq→unit haritasına GİRMEZ, yukarıdaki carries_unit
                    // dalı bunu zaten dışlıyor.
                    if d.kind == TransmitKind::CsProbe {
                        let f = encode_cs_probe(CS_PROBE_SUBTYPE, (d.seq & 0xFFFF) as u16);
                        stats.cs_probes_tx += 1;
                        // probe_seq ayrı ad-uzayı: PN tabloya GİRMEZ (alıcı probe'u ACK'lemez).
                        let _ = seal_send!(f, TxClass::Ctrl, $now);
                        continue;
                    }
                    // FEC-kapalıyken FecParity: v0 kabuk taşımaz, sayar (bootstrap OFF;
                    // sevk default'larında üretilmezler — typed genişleme noktası).
                    stats.stale_decisions += 1;
                    continue;
                }
                // [L-T1] Kurtarma kapalı (yalnız test yapılandırması): ARQ retransmit ve §11 kopya
                // tele ÇIKMAZ. Motor emisyonu zaten işledi; çıkmayan karar kayıp sayılır (INV-RATE-1).
                if !*recovery_rx.borrow()
                    && matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate)
                {
                    stats.recovery_suppressed += 1;
                    continue;
                }
                // Store-miss: [K4G-05b] store yalnız motor teyidiyle boşaldığından bu, motorun
                // TEYİTLİ birime karar üretmesidir (sözleşme ihlali; `stale_decisions`, 0 beklenir).
                // Emisyon YİNE wire'a çıkar (başlık-only): motor her emisyonu inflight izler — hiç
                // göndermemek yapay kayıp sinyali üretirdi (bench her kararı gönderirdi). Alıcı
                // birimi dedup eder, seq'i ACK'ler → motor temiz kapatır.
                let (payload, stale): (&[u8], bool) = match store.get(&d.unit_id) {
                    Some(p) => (p.as_slice(), false),
                    None => (&[], true),
                };
                if stale {
                    stats.stale_decisions += 1;
                }
                // [S1B] store WIRE GÖVDESİNİ tutar: frag açıkken `prefix ‖ payload`, kapalıyken
                // düz payload — emisyon tek şekilli kalır, prefix burada YENİDEN kurulmaz.
                // Stale (store-miss) emisyonda gövde boştur; frag çerçevesi ≥11 B ister, bu yüzden
                // canonical sıfır-prefix konur: alıcı çözer, PN ACK'lenir, birleştiriciye
                // len=0 katkısıyla girer (aynı esi'de gerçek symbol first-wins ile korunur).
                // [D1a] Çerçevedeki u16 `seq` ara çerçeve alanıdır (S3: interim çerçeve bu sprint
                // beyanlı KARŞILANMAZ); ARQ kimliği DEĞİL — alıcı kimliği `meta.full_wire_pn`'den alır.
                let stale_body: [u8; SOURCE_PREFIX_LEN] = [0; SOURCE_PREFIX_LEN];
                let frame = if frag_on {
                    let body = if stale { &stale_body[..] } else { payload };
                    encode_data_frag((d.seq & 0xFFFF) as u16, d.unit_id, body)
                } else {
                    encode_data((d.seq & 0xFFFF) as u16, d.unit_id, payload)
                };
                // Sınıf ayrımı motorun karar tipinden: ilk emisyon / ARQ retransmit / §11 dup.
                let class = match d.kind {
                    TransmitKind::Retransmit => TxClass::Retx,
                    TransmitKind::Duplicate => TxClass::Dup,
                    _ => TxClass::Data,
                };
                let pn = seal_send!(frame, class, $now);
                map_pn!(pn, d.seq);
            }
            confirmed_now
        }};
    }
    // [HANDOFF ENSTRÜMANI] bounded zaman-serisi örnekleme — HER pace tick'inde (ana döngü + drenaj).
    // Period-kapılı: aynı tick'te iki çağrı ikinci örnek üretmez. Salt gözlem.
    macro_rules! ts_sample {
        ($now:expr) => {
            if let Some(p) = cfg.ts_sample_period_ns {
                if $now.saturating_sub(ts_last_ns) >= p && stats.ts.len() < TS_CAP {
                    stats.ts.push(($now, session.sender().diag_ts_sample()));
                    ts_last_ns = $now;
                }
            }
        };
    }
    // Gelen datagram: seam.open → BAĞLAM-ÖNCE kapı (K4G-01) → yalnız kol (a)'da Session dağıtıcısı:
    // ACK (V1 0x10 / V2 0x14) → ilk-ACK damgası + motor beslemesi; RECOVERED / CS_ECHO / PARAM_ACK.
    macro_rules! handle_rx {
        ($raw:expr, $now:expr) => {
            let raw: &[u8] = $raw;
            stats.rx_datagrams += 1;
            stats.rx_bytes += raw.len() as u64;
            // [K4G-01 / D1 (v1.1)] Bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer. Yük,
            // bağlam doğrulanmadan çözülmez: `od.frame` aşağıdaki Session dağıtıcısına YALNIZ
            // bağlamı birebir `client_ctx` olan datagramdan girer. HubChannel (AEAD'siz no-op;
            // tipini de saldırgan seçer) ve başka oturumun bağlamı; yüke, ARQ deposuna, PN
            // çevirisine, Session sayaçlarına, RTT / PARAM / motor durumuna DOKUNMADAN düşer
            // (yukarıdaki tel sayaçları açmadan önce ve yükten bağımsız sayılır).
            let session_od = match seam.open(raw) {
                None => {
                    stats.open_failures += 1; // verify-fail: sessiz düşür, durum değişmez.
                    None
                }
                Some(od) => match route_client(od.ctx, client_ctx) {
                    CtxRoute::Session => Some(od),
                    CtxRoute::Hub => {
                        hub.on_hub_frame(&od);
                        None
                    }
                    // İstemci ucu yalnız `Ready` oturum için açılır; kol yine de kendi sayacıyla.
                    CtxRoute::NotReady => {
                        ctx_counters.add_not_ready();
                        None
                    }
                    CtxRoute::Mismatch => {
                        ctx_counters.add_ctx_mismatch();
                        None
                    }
                },
            };
            if let Some(od) = session_od {
                let plain = od.frame;
                if plain.first() == Some(&FRAME_CS_ECHO) {
                    // [DR-CS-3] Echo → RTT örneği. Peer'in taşıdığı seen_count/arrival_gap
                    // YALNIZ-LEDGER'dır (şema amendman-§3): motor örneği KENDİ yerel
                    // damgalarından türetir, bu alanlar karar girdisi YAPILMAZ.
                    if let Some((_sub, probe_seq, _seen, _gap)) = decode_cs_echo(&plain) {
                        stats.cs_echoes_rx += 1;
                        drive!(Event::CsProbeEcho { probe_seq }, $now);
                    } else {
                        stats.unknown_frames += 1;
                    }
                } else if plain.first() == Some(&FRAME_PARAM_ACK) {
                    // [v1/PARAM] hash-echo confirmation (B2: yalnız retry-durdurur; emisyon zaten akıyor).
                    if let (Some(plan), Some(h)) = (param_plan.as_ref(), decode_param_ack(&plain)) {
                        if h == plan.param_hash && !param_confirmed {
                            param_confirmed = true;
                            stats.param_confirmed = 1;
                        }
                    } else {
                        stats.unknown_frames += 1;
                    }
                } else if plain.first() == Some(&FRAME_RECOVERED) {
                    // [S1] RECOVERED-control: alıcı bu ESI'leri FEC ile kurtardı → motor
                    // saf-logical iptal eder (retransmit israfı biter). unit_id = block*K + esi
                    // (deterministik atama). [K4G-05b · CEVAP 13 §3] Birim teyidini MOTOR verir
                    // (`RecoveredControl` → teyit kümesi): payload ve app acked() akışı `drive!`'ın
                    // teyit drenajında düşer — burada doğrudan silme YOK.
                    match (fec.as_ref(), decode_recovered(&plain)) {
                        (Some(p), Some((block_id, esis))) => {
                            let mut units = Vec::with_capacity(esis.len());
                            for esi in esis {
                                let unit =
                                    u64::from(block_id) * u64::from(p.k_shape) + u64::from(esi);
                                units.push(RecoveredUnit {
                                    unit_id: unit,
                                    block_id,
                                    esi,
                                });
                            }
                            drive!(Event::RecoveredControl(RecoveredInfo { units }), $now);
                        }
                        _ => stats.unknown_frames += 1,
                    }
                } else if plain.is_empty()
                    || (plain[0] != FRAME_TYPE_ACK && plain[0] != FRAME_TYPE_ACK_V2)
                {
                    stats.unknown_frames += 1;
                } else {
                    if let Ok(frame) = AckFrame::decode(&plain) {
                        // [FAZ-1 DÖNÜŞ-YOLU] Gözlem girdisi: teldeki ACK boyu + alıcının GÖRDÜĞÜ
                        // datagram sayısı (V2 `seen_count`) + göndericinin yolladığı sayı.
                        // Motor davranışını DEĞİŞTİRMEZ; yön-ayrımını (ileri kayıp ↔ dönüş
                        // bozulması) mümkün kılan tek kanıt budur.
                        session.sender_mut().observe_ack_feedback(
                            adaptive_transport::return_path::AckObservation {
                                wire_bytes: raw.len() as u64,
                                seen_count: frame.seen_count,
                                v2: frame.v2,
                                sent_datagrams: stats.tx_datagrams,
                            },
                            $now,
                        );
                        // [D1a] ACK aralıkları PN uzayında → motor seq'ine çevrilir. Tabloda olmayan
                        // PN (ufuk dışı / motor kararı olmayan çerçeve) motora GİTMEZ.
                        let pns = frame.acked_full(max_sealed_pn.map_or(0, |m| m + 1));
                        // [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Salt gözlem: çerçevenin teyit ettiği PN
                        // sayısı ve en büyüğü (motora giden liste değişmez). [DUR-19] Yalnız canlı
                        // ayna açıkken hesaplanır.
                        let pn_view = if live.is_some() {
                            Some((
                                u32::try_from(pns.len()).unwrap_or(u32::MAX),
                                pns.iter().copied().max(),
                            ))
                        } else {
                            None
                        };
                        let acked: Vec<u64> = pns
                            .into_iter()
                            .filter_map(|pn| {
                                pn_seq
                                    .binary_search_by_key(&pn, |&(p, _)| p)
                                    .ok()
                                    .map(|i| pn_seq[i].1)
                            })
                            .collect();
                        // [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Çerçeve kaydının motordan ÖNCEKİ yarısı.
                        // [DUR-19] Yalnız canlı ayna açıkken okunur.
                        let pre = pn_view.map(|(acked_pns, largest_pn)| AckFramePre {
                            acked_pns,
                            largest_pn,
                            mapped_seqs: u32::try_from(acked.len()).unwrap_or(u32::MAX),
                            before: MotorAckView::of(session.sender()),
                            tx_before: stats.tx_datagrams,
                        });
                        // Kampanya-kanıtlı sözleşme (bench birebir): acked_full çıktısı diff'siz
                        // beslenir; motor inflight.remove ile idempotent işler.
                        // [K4G-05b · CEVAP 13 §3] Birim teyidini MOTOR verir: runtime ACK'teki seq'i
                        // kendi haritasıyla birime çevirip silmez. Payload, app acked() akışı ve
                        // `stats.acked` `drive!`'ın teyit drenajındadır; `new_units` = bu çerçevenin
                        // motorca ilk kez teyit ettirdiği birim sayısı.
                        let new_units: u32 = drive!(
                            Event::AckReceived(AckInfo {
                                acked_seqs: acked,
                                ack_delay: frame.ack_delay_duration(),
                            }),
                            $now
                        );
                        // [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Çerçeve kaydı ve sayaçları: yalnız aynaya
                        // yazılır; hiçbir karar okumaz. [DUR-19] Canlı ayna kapalıyken bu blok hiç
                        // çalışmaz: motor görünümü okunmaz, kayıt kurulmaz, kilit alınmaz.
                        if let (Some(live_mirror), Some(pre)) = (live.as_ref(), pre) {
                            let after = MotorAckView::of(session.sender());
                            ack_counts.count(
                                pre.mapped_seqs > 0,
                                new_units > 0,
                                after.inflight_len < pre.before.inflight_len,
                            );
                            let record = AckFrameDiag {
                                index: ack_counts.frames,
                                at_ns: $now,
                                v2: frame.v2,
                                seen_count: frame.seen_count,
                                largest_pn: pre.largest_pn,
                                ack_delay_raw: frame.ack_delay,
                                ack_delay_us: saturating_us(frame.ack_delay_duration()),
                                acked_pns: pre.acked_pns,
                                mapped_seqs: pre.mapped_seqs,
                                new_units,
                                tx_datagrams_during: u32::try_from(
                                    stats.tx_datagrams.saturating_sub(pre.tx_before),
                                )
                                .unwrap_or(u32::MAX),
                                before: pre.before,
                                after,
                            };
                            let mut mirror =
                                live_mirror.lock().unwrap_or_else(PoisonError::into_inner);
                            mirror.ack = ack_counts;
                            mirror.acks.push(record);
                        }
                    }
                }
            }
        };
    }

    // [K4G-05 / INT-02 TEŞHİS] Gönderen kurtarma görüntüsü ve sonraki yenileme anı (oranlı; bkz.
    // `RECOVERY_DIAG_PERIOD`). Yalnız aynaya yazılır, görev okumaz.
    let recovery_period_ns = u64::try_from(RECOVERY_DIAG_PERIOD.as_nanos()).unwrap_or(u64::MAX);
    let mut recovery_next_ns: u64 = 0;
    let mut recovery: Option<SenderRecoveryDiag> = None;
    // [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Son görülen motor durumu ve durum değişikliği kayıt sırası
    // (yalnız aynaya yazılır, görev okumaz).
    let mut last_state: Option<adaptive_transport::sender::SenderState> = None;
    let mut state_changes: u64 = 0;

    // ── Ana döngü ──
    loop {
        // [K4G-05] Salt gözlem: motor ve tel sayaçlarının o anki değeri yayımlanır (önceki turun
        // bütün kolları dahil). Burada okunur, değişmez; görev aynayı OKUMAZ (bkz.
        // `ClientHandle::live_diag`). [DUR-19] YALNIZ canlı ayna açıkken
        // (`RuntimeConfig::live_diag`); kapalıyken bu blok hiç çalışmaz — saat, motor okuması,
        // durum karşılaştırması, kurtarma görüntüsü ve kilit YOK; tur, aynasız ana döngüdür.
        if let Some(live_mirror) = live.as_ref() {
            let mirror_now_ns = elapsed_ns(start);
            let activation = session.sender().diag_activation();
            let control = SenderControlDiag::capture(session.sender(), &activation, mirror_now_ns);
            if mirror_now_ns >= recovery_next_ns {
                recovery_next_ns = mirror_now_ns.saturating_add(recovery_period_ns);
                recovery = Some(capture_recovery(
                    session.sender(),
                    &store,
                    &seq_to_unit,
                    &unit_seqs,
                    &pn_seq,
                    mirror_now_ns,
                ));
            }
            // [CEVAP PAKETİ 6] Durum değişikliği: bir önceki turda görülenden farklıysa kaydedilir.
            let state_change = if control.state == last_state {
                None
            } else {
                state_changes = state_changes.saturating_add(1);
                Some(StateChangeDiag {
                    index: state_changes,
                    at_ns: mirror_now_ns,
                    from: last_state,
                    to: control.state,
                    motor_transitions: control.state_transitions,
                    emergency: control.emergency,
                    fusion_emergency_mode: control.fusion_emergency_mode,
                    ack_confidence_permille: control.fusion.map(|f| f.ack_confidence_permille),
                    last_ack_ns: session.sender().diag_recovery_timers().last_ack_ns,
                })
            };
            last_state = control.state;
            let mut mirror = live_mirror.lock().unwrap_or_else(PoisonError::into_inner);
            mirror.refresh(
                session.sender(),
                &activation,
                &stats,
                control,
                start,
                mirror_now_ns,
            );
            mirror.recovery = recovery;
            if let Some(change) = state_change {
                mirror.states.push(change);
            }
        }
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            // Ölçüm-t0 işareti (ClientHandle::mark_t0): wire sayaçları sıfırlanır, MOTOR DURUMU
            // KORUNUR (counter-reset-without-state-reset) ve egress kapısı arm edilir. Onay
            // gönderilene dek çağıran bekler → t0'dan SONRAKİ hiçbir sunum arm-öncesi kalmaz.
            Some(ack) = t0_rx.recv() => {
                let now = elapsed_ns(start);
                if let Some(p) = egress.as_mut() { p.arm(now); }
                stats.tx_datagrams = 0;
                stats.rx_datagrams = 0;
                stats.tx_bytes = 0;
                stats.rx_bytes = 0;
                stats.tx_data_bytes = 0;
                stats.tx_retx_bytes = 0;
                stats.tx_dup_bytes = 0;
                stats.tx_fec_bytes = 0;
                stats.tx_ctrl_bytes = 0;
                stats.egress_refused_pkts = 0;
                stats.egress_refused_bytes = 0;
                let _ = ack.send(()); // çağıran serbest: bundan sonrası ölçüm penceresi
            }
            // Offer YALNIZ store'da yer varken alınır (bounds-before-alloc: kanal + store çift sınır).
            maybe = offer_rx.recv(), if store.len() < cfg.max_inflight_units => {
                let Some((message, deadline_ns)) = maybe else { break }; // handle'lar düştü → drain
                stats.offered += 1;
                // [DEADLINE-EKSENİ] Politika girdisi; ARQ/pacing/loss durumuna DOKUNMAZ.
                if let Some(d) = deadline_ns {
                    session.sender_mut().declare_delivery_deadline(d);
                }
                let now = elapsed_ns(start);
                // [S1B] FRAGMENTER. Mesaj `max_unit_bytes`lik ardışık chunk'lara bölünür; her
                // chunk KENDİ birim-id'sini alır → ARQ izlemesi, FEC blok/esi yuvası ve pacing
                // muhasebesi chunk granülünde kalır (mesaj katmanı motora GÖRÜNMEZ — SoC).
                // Fragmentasyon kapalıyken n_chunks daima 1'dir ve bu döngü eski kodun aynısıdır.
                let msg_id = next_message;
                next_message = next_message.wrapping_add(1);
                let chunk_cap = cfg.max_unit_bytes; // validate_fec: > 0 garantili
                let n_chunks = message.len().div_ceil(chunk_cap).max(1); // boş mesaj = 1 chunk
                if frag_on {
                    msg_pending.insert(msg_id, u32::try_from(n_chunks).unwrap_or(u32::MAX));
                }
                for ci in 0..n_chunks {
                    let off = ci * chunk_cap;
                    let chunk = &message[off..(off + chunk_cap).min(message.len())];
                    let id = next_unit;
                    next_unit += 1;
                    // Canonical source-symbol prefix'i (§2.6). Fragmentasyon KAPALIYKEN alanlar
                    // eski değerlerini korur (message_id = unit_id, offset 0, flags 0) — alıcı
                    // onları türetir ve wire'da prefix TAŞINMAZ. AÇIKKEN prefix wire'a yazılır.
                    let pfx = SourcePrefix {
                        message_id: if frag_on { msg_id } else { (id & 0xFFFF_FFFF) as u32 },
                        chunk_offset: if frag_on { u32::try_from(off).unwrap_or(u32::MAX) } else { 0 },
                        actual_payload_len: u16::try_from(chunk.len()).unwrap_or(u16::MAX),
                        source_flags: if frag_on && ci + 1 == n_chunks { SOURCE_FLAG_FINAL } else { 0 },
                    };
                    // Gövde = prefix ‖ payload, TEK KAYNAK canonical serializer'dan (pad'siz boy).
                    // FEC symbol'ü de wire gövdesi de bundan türer → iki uçtaki symbol byte-eşit.
                    let mut body = vec![0u8; SOURCE_PREFIX_LEN + chunk.len()];
                    if pfx.serialize_symbol(chunk, &mut body).is_err() {
                        stats.frag_encode_failures += 1; // bounds koşu-öncesi doğrulandı; sessiz kalmaz
                        continue;
                    }
                    // FEC symbol'ü gövdeden: `body ‖ canonical_zero_pad` (pad zaten sıfır).
                    let sym: Option<Vec<u8>> = fec.as_ref().and_then(|p| {
                        let mut s = vec![0u8; p.e_usize()];
                        if body.len() > s.len() {
                            return None; // validate_fec engelledi; savunmacı
                        }
                        s[..body.len()].copy_from_slice(&body);
                        Some(s)
                    });
                    let hdr = if frag_on { HDR_DATA_FRAG } else { HDR_DATA };
                    let size = wire_len(seam.as_ref(), client_ctx, hdr + chunk.len());
                    // store WIRE GÖVDESİNİ tutar (frag: prefix‖payload, düz: payload).
                    store.insert(id, if frag_on { body } else { chunk.to_vec() });
                    if frag_on {
                        unit_msg.insert(id, msg_id);
                    }
                    drive!(Event::AppData(OpaqueUnit { id, size }), now);
                // [S1] Blok birikimi + K'de SAME-EVENT latch: drive! az önce on_event koşturdu →
                // snapshot.event_index taze; arada başka event YOK (tek-task sıralılık) → latch'in
                // same-event-causality guard'ı geçer.
                if let Some(p) = fec.as_ref() {
                    let e = p.e_usize();
                    let Some(sym) = sym else {
                        // Ulaşılamaz (validate_fec body ≤ E garantiler); sessiz blok-desenkronu
                        // yerine sayaç — hizanın bozulduğu ANDA görünür.
                        stats.frag_encode_failures += 1;
                        continue;
                    };
                    debug_assert_eq!(cur_block.len() as u64, id % u64::from(p.k_shape));
                    cur_block.push(sym);
                    if cur_block.len() == p.k_shape_usize() {
                        let parity_wire = wire_len(seam.as_ref(), client_ctx, HDR_FEC + e);
                        // Statik-r (Manual{true} + manuel-close) YA DA adaptif (Controller + latch).
                        let planned_r: u16 = if let Some(r) = fec_manual_r {
                            let auth = session.sender_mut().open_fec_block_authority();
                            session
                                .sender_mut()
                                .close_fec_block_manual(auth, cur_block_id, r, parity_wire)
                                .unwrap_or(0)
                        } else {
                            let snap = session.sender().controller_decision_snapshot();
                            let auth = session.sender_mut().open_fec_block_authority();
                            session
                                .sender_mut()
                                .fec_controller_latch(
                                    auth,
                                    snap,
                                    u8::try_from(p.k_shape).unwrap_or(255),
                                    p.r_code_max,
                                    cur_block_id,
                                    parity_wire,
                                )
                                .planned_r
                        };
                        if planned_r > 0 {
                            let srcs: Vec<SourceSymbol> = cur_block
                                .iter()
                                .enumerate()
                                .map(|(i, b)| SourceSymbol {
                                    esi: i as u16,
                                    bytes: b,
                                })
                                .collect();
                            if let Ok(reps) = RsSimdCodec.encode(p, &srcs) {
                                stats.fec_blocks_latched += 1;
                                repair_store
                                    .insert(cur_block_id, (u8::try_from(p.k_shape).unwrap_or(255), reps));
                                while repair_store.len() > REPAIR_STORE_BLOCKS {
                                    repair_store.pop_first();
                                }
                            }
                        }
                        cur_block.clear();
                        cur_block_id = cur_block_id.wrapping_add(1);
                    }
                }
                } // [S1B] chunk döngüsü sonu
            }
            _ = pace.tick() => {
                let now = elapsed_ns(start);
                drive!(Event::TimerFired(Timer::Pto), now); ts_sample!(now);
            }
            _ = rtprop.tick() => {
                let now = elapsed_ns(start);
                // [v1/PARAM] bounded retry (B2): confirmed olana dek her ≥200ms'de bir, ≤25 deneme.
                if let Some(plan) = param_plan.as_ref() {
                    let now_ms = now / 1_000_000;
                    let due = param_tries == 0 || now_ms >= param_last_tick_ms + 200;
                    if !param_confirmed && param_tries < 25 && due {
                        if let (Some(p), Control::ParamSet { param_hash, .. }) = (fec.as_ref(), &plan.control) {
                            let f = encode_param(p.profile_id, p.code_suite_id, param_hash);
                            // Motor kararı değil: PN tabloya GİRMEZ (alıcı PARAM'ı ACK'lemez).
                            let _ = seal_send!(f, TxClass::Ctrl, now);
                            stats.param_sends += 1;
                            param_tries += 1;
                            param_last_tick_ms = now_ms;
                        }
                    }
                }
                drive!(Event::TimerFired(Timer::RtpropProbe), now);
            }
            r = sock.recv(&mut buf) => {
                let Ok(n) = r else { continue };
                let now = elapsed_ns(start);
                handle_rx!(&buf[..n], now);
            }
        }
    }

    // ── [FAZ-2 / KALEM-1] KISMİ/SON BLOK KAPANIŞI (Model-A known-zero) ──────────────────────
    // fec-core §6 NORMATİF: `known_zero_count = K_shape − K_actual`; **gönderici zero-slot'ları
    // ENCODE'a katar (repair üretimi için), WIRE'a GÖNDERMEZ**. Eskiden blok YALNIZ tam
    // dolduğunda kapanıyordu (`cur_block.len() == k_shape`) → akışın SONUNDAKİ eksik blok ve
    // K'ya ulaşmayan kısa akışlar FEC korumasından TAMAMEN yoksundu (denetim B-2).
    if let Some(p) = fec.as_ref() {
        if !cur_block.is_empty() {
            let k_actual = u8::try_from(cur_block.len()).unwrap_or(255);
            let parity_wire = wire_len(seam.as_ref(), client_ctx, HDR_FEC + p.e_usize());
            let planned_r: u16 = if let Some(r) = fec_manual_r {
                let auth = session.sender_mut().open_fec_block_authority();
                session
                    .sender_mut()
                    .close_fec_block_manual(auth, cur_block_id, r, parity_wire)
                    .unwrap_or(0)
            } else {
                let snap = session.sender().controller_decision_snapshot();
                let auth = session.sender_mut().open_fec_block_authority();
                session
                    .sender_mut()
                    .fec_controller_latch(
                        auth,
                        snap,
                        k_actual,
                        p.r_code_max,
                        cur_block_id,
                        parity_wire,
                    )
                    .planned_r
            };
            if planned_r > 0 {
                // Zero-slot dolgusu: ESI [K_actual, K_shape) canonical-zero (E byte). ENCODE'a
                // girer, TELE ÇIKMAZ — alıcı bunları k_actual'dan türetip enjekte eder.
                let zero = vec![0u8; p.e_usize()];
                let mut srcs: Vec<SourceSymbol> = cur_block
                    .iter()
                    .enumerate()
                    .map(|(i, b)| SourceSymbol {
                        esi: i as u16,
                        bytes: b,
                    })
                    .collect();
                for esi in cur_block.len()..p.k_shape_usize() {
                    srcs.push(SourceSymbol {
                        esi: esi as u16,
                        bytes: &zero,
                    });
                }
                if let Ok(reps) = RsSimdCodec.encode(p, &srcs) {
                    stats.fec_blocks_latched += 1;
                    stats.fec_partial_blocks_closed += 1;
                    repair_store.insert(cur_block_id, (k_actual, reps));
                }
            }
            cur_block.clear();
        }
    }

    // Kuyrukta kalan (hiç işlenmemiş) offer'lar sessizce kaybolmaz — sayılır.
    while offer_rx.try_recv().is_ok() {
        stats.unaccepted_offers += 1;
    }
    // ── Drain (D8): motor birim terk etmez → pif→0 hedefi MUTLAKA süre-tavanlı. ──
    // [KISA-AKIŞ ENSTRÜMANI] AKIŞ SONU = ana döngünün çıkışı (drenajdan ÖNCE), sender'ın
    // damgalarıyla AYNI saat tabanında (`elapsed_ns(start)`). Kısa-akış hipotezinin tüm sorusu
    // "kayıp bilgisi akış HÂLÂ CANLIYKEN geldi mi" olduğundan, bu damga olmadan
    // `t_first_confirmed` gibi değerler bir eşikle KIYASLANAMAZ. Salt gözlem.
    stats.flow_end_ns = elapsed_ns(start);
    let deadline = tokio::time::Instant::now() + cfg.drain_cap;
    loop {
        if store.is_empty() && session.sender().diag_pif() == 0 {
            break;
        }
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => break,
            _ = pace.tick() => {
                let now = elapsed_ns(start);
                drive!(Event::TimerFired(Timer::Pto), now); ts_sample!(now);
            }
            r = sock.recv(&mut buf) => {
                let Ok(n) = r else { continue };
                let now = elapsed_ns(start);
                handle_rx!(&buf[..n], now);
            }
        }
    }
    stats.undelivered = store.len() as u64;
    // [TEŞHİS] Yapı-gate'in karar tabanını kapanışta fotoğrafla (gözlem-amaçlı; motor durumunu
    // DEĞİŞTİRMEZ). Bunlar görünmeden "gate neden açmadı" sorusu cevaplanamıyordu.
    let f = session.sender().diag_loss_structure();
    stats.fate = FateWindowDiag {
        n: f.n,
        lost: f.lost,
        n00: f.n00,
        n01: f.n01,
        n10: f.n10,
        n11: f.n11,
        burst_delta_ppm: f.burst_delta_ppm,
        sender_state: match session.sender().state() {
            adaptive_transport::sender::SenderState::Idle => "Idle",
            adaptive_transport::sender::SenderState::PreConverge => "PreConverge",
            adaptive_transport::sender::SenderState::Startup => "Startup",
            adaptive_transport::sender::SenderState::Drain => "Drain",
            adaptive_transport::sender::SenderState::DrainRtprop => "DrainRtprop",
            adaptive_transport::sender::SenderState::ProbeUp => "ProbeUp",
            adaptive_transport::sender::SenderState::Steady => "Steady",
            adaptive_transport::sender::SenderState::RecoverArq => "RecoverArq",
            adaptive_transport::sender::SenderState::RecoverFwd => "RecoverFwd",
        },
        // [GÖRÜNÜRLÜK] Kuruluşun TANIMI: bir RTT örneği alınmış olması. PreConverge'den çıkışın
        // motor içindeki tek koşulu budur (`rtt.srtt().is_some()`). Faz ADINA bakmak yanlış bir
        // vekildi: tek-mesajlık bir koşuda motor PreConverge → Idle geçiyor, Startup'a hiç
        // uğramıyor, ve faz-tabanlı bayrak kuruluşu KAÇIRIYOR (test bunu yakaladı). Sayaç
        // monotondur → kapanış-anı örneklemesinden bağımsız.
        left_preconverge: session.sender().diag_rtt_sample_count() > 0,
        rtt_samples: session.sender().diag_rtt_sample_count(),
        establish_probes_tx: session.sender().diag_establish_probe_emitted(),
        declared_deadline_ns: session.sender().diag_tightest_deadline_ns(),
        fusion_actions: session.sender().diag_fusion_actions(),
        last_fusion_obs: session.sender().diag_last_fusion_obs(),
        fusion_bw_range: session.sender().diag_fusion_bw_range(),
        fusion_loss_range: session.sender().diag_fusion_loss_range(),
        ledger_internals: session.sender().diag_ledger_internals(),
        ledger_confirm_path: session.sender().diag_confirm_path(),
        ledger_tick_span: session.sender().diag_ledger_tick_span(),
        spurious_units: session.sender().diag_ledger().2,
        fec_risk_times: session.sender().diag_fec_risk_times(),
        fec_risk_ppm: session.sender().diag_risk_ppm(),
        fec_arbiter_reasons: session.sender().diag_arbiter_reasons(),
        fec_spec_levels: session.sender().diag_spec_levels(),
        fec_spec_signals: session.sender().diag_spec_signals(),
        fec_persist: session.sender().diag_persist(),
        fec_spec_transitions: session.sender().diag_spec_transitions(),
        sched: session.sender().diag_sched_summary(),
        preconverge_parity_blocked: session.sender().diag_preconverge_parity_blocked(),
        return_path: session.sender().diag_return_path(elapsed_ns(start)),
        activation: session.sender().diag_activation(),
        fec_completeness: (
            session.sender().diag_repair_loss_not_arqed(),
            session.sender().diag_parity_budget_clamped(),
            session.sender().diag_parity_budget_would_zero(),
        ),
        fec_latch_diag: {
            let d = session.sender().diag_fec_actuation();
            (
                d.no_eligible_decision_count,
                d.decision_event_mismatch_count,
                d.no_rule_zero_count,
                d.enqueued_r_total,
                d.emitted_r_total,
            )
        },
        fec_last_suppression: session
            .sender()
            .diag_fec_actuation()
            .last_latch
            .and_then(|l| l.suppression)
            .map(|r| match r {
                adaptive_transport::sender::FecSuppressionReason::ManualDisabled => {
                    "ManualDisabled"
                }
                adaptive_transport::sender::FecSuppressionReason::NoEligibleDecision => {
                    "NoEligibleDecision"
                }
                adaptive_transport::sender::FecSuppressionReason::DecisionEventMismatch => {
                    "DecisionEventMismatch"
                }
                adaptive_transport::sender::FecSuppressionReason::NoRule => "NoRule",
                other => {
                    let _ = other;
                    "Diger"
                }
            }),
        structure_gate_opened: session.sender().diag_structure_gate_opened(),
        ledger_loss_ppm: session.sender().diag_ledger().1,
        ledger_loss_ppm_peak: session.sender().diag_loss_ppm_peak(),
        gate_outcomes: session.sender().diag_gate_outcomes(),
        ledger_spurious_units: session.sender().diag_ledger().2,
        ledger_unloss: session.sender().diag_ledger().0,
        loss_declared_pkt: session.sender().diag_loss().0,
        loss_declared_time: session.sender().diag_loss().1,
    };
    stats
}

// ─────────────────────────────── SERVER ───────────────────────────────

/// Server ucu: eş-başına [`Receiver`] (bounded tablo) + ACK/patience tick'leri.
pub struct ServerEndpoint;

/// Server tutamağı: teslim akışı + kontrollü kapanış.
pub struct ServerHandle {
    delivery_rx: mpsc::Receiver<Delivery>,
    shutdown_tx: watch::Sender<bool>,
    t0_tx: mpsc::Sender<oneshot::Sender<()>>,
    task: JoinHandle<ServerStats>,
    local_addr: SocketAddr,
    /// [K4G-01] Bağlam-önce dağıtım sayaçları (bkz. [`ServerHandle::ctx_dispatch_stats`]).
    ctx_counters: CtxDispatchCounters,
    /// [K4G-11] [`ServerStats::delivery_dropped`]'ın canlı aynası (salt gözlem; bkz.
    /// [`ServerHandle::delivery_dropped`]). Görev her döngü turunun başında sayacın o anki değerini
    /// buraya yazar; sayaç ve davranış değişmez.
    delivery_dropped: Arc<AtomicU64>,
}

impl ServerEndpoint {
    /// Bağla ve görevi başlat.
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind hatası.
    pub async fn bind(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
    ) -> Result<ServerHandle, RuntimeError> {
        Self::bind_with_policy(bind, cfg, seam, None).await
    }

    /// [`bind`](Self::bind) + opsiyonel dönüş-yönü egress kapısı ([`EgressPolicy`]). Politika
    /// `None` iken davranış `bind` ile BİREBİR aynıdır.
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind hatası.
    pub async fn bind_with_policy(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
        egress: Option<Box<dyn EgressPolicy>>,
    ) -> Result<ServerHandle, RuntimeError> {
        // PoC: tek örtük oturum — `connect` / `connect_with_policy` ile simetrik (eski çağıranların
        // davranışı birebir). Çok-oturumlu uç beklenen kümeyi oturum tablosundan alır.
        Self::bind_expecting(
            bind,
            cfg,
            seam,
            egress,
            ExpectedSessions::only(IMPLICIT_SESSION),
            CtxDispatchCounters::new(),
        )
        .await
    }

    /// [K4G-01 / D1 (v1.1)] [`bind_with_policy`](Self::bind_with_policy) + bağlam-önce dağıtımın
    /// girdileri. `expected` = bu ucun BEKLEDİĞİ oturumlar (oturum tablosu yayımlar; bkz.
    /// [`ExpectedSessions`]): açılan datagramın yükü YALNIZ bağlamı beklenen bir `Session(h)` ise
    /// ve — eş yuvası varsa — yuvanın bağlandığı bağlamla birebir aynıysa Session dağıtıcısına
    /// girer. `ctx_counters` = paylaşımlı telemetri sayaçları.
    ///
    /// [K4G-01b] Eş yuvası `(kaynak adres, h)` ile anahtarlıdır: aynı adresten yeni bir beklenen
    /// oturum yeni yuva alır; bağlı oturumu `expected`'dan çıkan yuva emekliye ayrılır.
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind hatası.
    pub async fn bind_expecting(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn PacketProtection>,
        egress: Option<Box<dyn EgressPolicy>>,
        expected: ExpectedSessions,
        ctx_counters: CtxDispatchCounters,
    ) -> Result<ServerHandle, RuntimeError> {
        validate_fec(&cfg)?; // [S1] koşu-öncesi typed FEC-config doğrulaması
        let sock = UdpSocket::bind(bind).await?;
        let local_addr = sock.local_addr()?;
        let (delivery_tx, delivery_rx) = mpsc::channel(cfg.delivery_queue);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Kapasite 1 + onay-oneshot: mark_t0 görev işleyene dek bekler (ölçüm sözleşmesi).
        let (t0_tx, t0_rx) = mpsc::channel::<oneshot::Sender<()>>(1);
        let delivery_dropped = Arc::new(AtomicU64::new(0));
        let task = tokio::spawn(server_task(
            sock,
            cfg,
            seam,
            egress,
            delivery_tx,
            shutdown_rx,
            t0_rx,
            expected,
            ctx_counters.clone(),
            Arc::clone(&delivery_dropped),
        ));
        Ok(ServerHandle {
            delivery_rx,
            shutdown_tx,
            t0_tx,
            task,
            local_addr,
            ctx_counters,
            delivery_dropped,
        })
    }
}

impl ServerHandle {
    /// Gerçek bağlanılan adres (test/portsuz bind için).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Sıradaki teslim (ilk-teslim dedup'lu; kanal bounded).
    pub async fn next_delivery(&mut self) -> Option<Delivery> {
        self.delivery_rx.recv().await
    }

    /// [L-T1] Bekleyen teslim varsa hemen döner, yoksa `None` (senkron S1 adaptörü için).
    pub fn try_next_delivery(&mut self) -> Option<Delivery> {
        self.delivery_rx.try_recv().ok()
    }

    /// Ölçüm-t0 işareti (dönüş yönü): wire sayaçları sıfırlanır, ALICI DURUMU KORUNUR ve varsa
    /// [`EgressPolicy`] arm edilir. Görev işleyene dek bekler — bkz. [`ClientHandle::mark_t0`].
    pub async fn mark_t0(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.t0_tx.send(ack_tx).await.is_ok() {
            let _ = ack_rx.await;
        }
    }

    /// [K4G-01 / D1 (v1.1)] Bağlam-önce dağıtımın canlı sayaçları (telemetri): `ctx_mismatch_drops`
    /// ve `hub_frames_dropped` (bkz. [`ClientHandle::ctx_dispatch_stats`]).
    #[must_use]
    pub fn ctx_dispatch_stats(&self) -> CtxDispatchStats {
        self.ctx_counters.snapshot()
    }

    /// [K4G-11] Salt-okunur: teslim kanalı doluyken düşen teslim sayısı —
    /// [`ServerStats::delivery_dropped`] ile AYNI sayaç, kapanışı beklemeden okunur (bileşim kökünün
    /// `delivery_channel_drops`'u). Yayın: sayaç görevin bir `select!` kolunda (`push_delivery`)
    /// artar; döngü o kol biter bitmez başa döner ve değeri yayımlar — okuyucu en çok artışın olduğu
    /// kolun kalan işi kadar geride kalır, bir zamanlayıcıyı beklemez. Kapanış kolunda (`break`)
    /// yayın yoktur; kapanıştaki kesin değer [`ServerHandle::shutdown`]'ın döndürdüğü
    /// istatistiktir. Monoton.
    #[must_use]
    pub fn delivery_dropped(&self) -> u64 {
        self.delivery_dropped.load(Ordering::Relaxed)
    }

    /// Kapanış → istatistik.
    pub async fn shutdown(self) -> ServerStats {
        let _ = self.shutdown_tx.send(true);
        self.task.await.unwrap_or_default()
    }
}

/// Birim-düzeyi teslim-dedup penceresi (eş başına; SR-AUDIT-001 sınırı). Retransmit YENİ seq
/// taşıdığından [`Receiver`]'ın seq-dedup'u birim tekrarını YAKALAMAZ (ilk netem kapı-koşusu
/// bunu 26 duplicate-teslimle gösterdi); ilk-teslim sözleşmesi bu pencereyle sağlanır. Pencere
/// dolunca en-eski id düşer (bellek sınırı; pencere-dışı çok-geç duplicate teorik olarak yeniden
/// teslim edilebilir — E2E envelope katmanı §4 nihai dedup otoritesidir).
const DELIVERED_DEDUP_WINDOW: usize = 8192;

/// [S1] Peer-başına FEC blok-birleştirme tavanı (bounded, SR-AUDIT-001; en-eski düşer + sayaç).
const RX_BLOCKS_CAP: usize = 32;

/// [S1] Blok-birleştirme durumu: esi→symbol. Source-symbol DATA payload'ından deterministik
/// türetilir (TX ile aynı `SourcePrefix`); repair FRAME_FEC'ten gelir. BTreeMap = dup-ESI doğal
/// dedup (codec DuplicateEsi reddine hiç düşülmez).
struct BlockAsm {
    k: u8,
    sources: BTreeMap<u16, Vec<u8>>,
    repairs: BTreeMap<u16, Vec<u8>>,
}

/// [S1B] Peer-başına aynı anda birleştirilen mesaj tavanı (bounded, SR-AUDIT-001; en-eski
/// düşer + sayaç — yarım kalan mesajlar belleği süresiz tutamaz).
const MSG_ASM_CAP: usize = 64;

/// [S1B] Bir mesajın yeniden-birleştirme durumu.
///
/// Chunk'lar DATA yolundan da FEC-kurtarmadan da gelebilir; ikisi de aynı `SourcePrefix`'i
/// taşıdığından burada ayrım YOKTUR (kurtarılmış chunk ile alınmış chunk eşdeğerdir — §11
/// forward-recovery'nin mesaj katmanındaki karşılığı).
struct MsgAsm {
    /// `chunk_offset` → payload. BTreeMap: sıra doğal, aynı offset'in tekrarı doğal dedup
    /// (retransmit + FEC-kurtarma aynı chunk'ı iki kez getirebilir; ilk-kazanır).
    chunks: BTreeMap<u32, Vec<u8>>,
    /// FINAL chunk görüldüyse mesajın toplam boyu (`offset + len`).
    total_len: Option<usize>,
    /// Tamponlanan byte (per-mesaj sınır kontrolü).
    buffered: usize,
}

impl MsgAsm {
    fn new() -> Self {
        Self {
            chunks: BTreeMap::new(),
            total_len: None,
            buffered: 0,
        }
    }

    /// Chunk'ı ekler. `false` = REDDEDİLDİ (mesaj tavanı aşılıyor — bounds-before-alloc;
    /// bozuk/kötücül bir FINAL'in belleği şişirmesi engellenir).
    fn insert(&mut self, offset: u32, payload: &[u8], is_final: bool, cap: usize) -> bool {
        let off = offset as usize;
        let end = off.saturating_add(payload.len());
        if end > cap {
            return false;
        }
        if is_final {
            self.total_len = Some(end);
        }
        if self.chunks.contains_key(&offset) {
            return true; // ilk-kazanır; tekrar sessizce yutulmaz ama bellek de büyümez
        }
        self.buffered = self.buffered.saturating_add(payload.len());
        if self.buffered > cap {
            return false;
        }
        self.chunks.insert(offset, payload.to_vec());
        true
    }

    /// Mesaj TAMAMLANDIYSA birleştirilmiş payload. Tamamlanma = FINAL görüldü **ve** `0..total_len`
    /// BOŞLUKSUZ kaplandı. Boşluk/örtüşme varsa `None` (fail-closed: yarım mesaj asla teslim
    /// edilmez — app'e eksik payload vermek sessiz veri bozulmasıdır).
    fn take_if_complete(&self) -> Option<Vec<u8>> {
        let total = self.total_len?;
        let mut out = Vec::with_capacity(total);
        for (&off, data) in &self.chunks {
            if off as usize != out.len() {
                return None; // boşluk veya örtüşme
            }
            out.extend_from_slice(data);
        }
        (out.len() == total).then_some(out)
    }
}

/// Teslim kanalına bir mesaj koyar (bounded; taşan teslim sayılır — app drenaj sözleşmesi).
fn push_delivery(
    stats: &mut ServerStats,
    delivery_tx: &mpsc::Sender<Delivery>,
    peer: SocketAddr,
    ctx: ProtectionContext,
    unit_id: u64,
    payload: Vec<u8>,
    from_fec: bool,
) {
    let d = Delivery {
        peer,
        unit_id,
        payload,
        ctx,
    };
    if delivery_tx.try_send(d).is_ok() {
        stats.delivered += 1;
        if from_fec {
            stats.fec_recovered_units += 1;
        }
    } else {
        stats.delivery_dropped += 1;
    }
}

/// [S1B] Alınan bir chunk'ı app'e teslim yoluna koyar — DATA ve FEC-kurtarma ORTAK kapısı.
///
/// Fragmentasyon KAPALIYKEN: chunk = mesaj → doğrudan teslim, ilk-teslim dedup'ı `unit_id`
/// üzerinde (eski davranış birebir).
/// AÇIKKEN: prefix'teki `(message_id, chunk_offset, FINAL)` ile birleştiriciye girer; mesaj
/// tamamlanınca TAM payload TEK `Delivery` olarak akar ve dedup `message_id` üzerinde olur.
#[allow(clippy::too_many_arguments)]
fn admit_chunk(
    stats: &mut ServerStats,
    delivery_tx: &mpsc::Sender<Delivery>,
    peer: SocketAddr,
    slot: &mut PeerSlot,
    frag_on: bool,
    max_message_bytes: usize,
    unit_id: u64,
    pfx: &SourcePrefix,
    payload: &[u8],
    from_fec: bool,
) {
    if !frag_on {
        if !slot.delivered_seen.insert(unit_id) {
            return; // birim çoktan teslim edildi (DATA/FEC ortak pencere)
        }
        if slot.delivered_seen.len() > DELIVERED_DEDUP_WINDOW {
            slot.delivered_seen.pop_first();
        }
        push_delivery(
            stats,
            delivery_tx,
            peer,
            slot.ctx,
            unit_id,
            payload.to_vec(),
            from_fec,
        );
        return;
    }
    let mid = u64::from(pfx.message_id);
    if slot.delivered_seen.contains(&mid) {
        return; // mesaj çoktan teslim edildi; geç chunk'lar (retransmit/kurtarma) yutulur
    }
    stats.frag_chunks_rx += 1;
    let complete = {
        let asm = slot.msgs.entry(pfx.message_id).or_insert_with(MsgAsm::new);
        if !asm.insert(
            pfx.chunk_offset,
            payload,
            pfx.source_flags & SOURCE_FLAG_FINAL != 0,
            max_message_bytes,
        ) {
            stats.frag_chunk_dropped += 1;
        }
        asm.take_if_complete()
    };
    if let Some(full) = complete {
        slot.msgs.remove(&pfx.message_id);
        slot.delivered_seen.insert(mid);
        if slot.delivered_seen.len() > DELIVERED_DEDUP_WINDOW {
            slot.delivered_seen.pop_first();
        }
        stats.msgs_reassembled += 1;
        push_delivery(stats, delivery_tx, peer, slot.ctx, mid, full, from_fec);
    } else if slot.msgs.len() > MSG_ASM_CAP {
        slot.msgs.pop_first();
        stats.msg_asm_evicted += 1;
    }
}

/// [K4G-01b] Eş yuvası anahtarı: `(kaynak adres, SessionHandle)` — adres tek başına DEĞİL. Aynı
/// adresten gelen iki oturum (yenileme / supersession / efemeral port yeniden kullanımı) iki ayrı
/// yuvadır; bir yuva tek oturuma aittir.
type SlotKey = (SocketAddr, SessionHandle);

/// Eş yuvası: alıcı motoru + idle-emeklilik çapası (item 3) + teslim-dedup penceresi.
struct PeerSlot {
    rx: Receiver,
    last_seen_ns: u64,
    delivered_seen: std::collections::BTreeSet<u64>,
    /// [S1] FEC blok-birleştirici (bounded; yalnız FEC-açıkken dolar).
    blocks: BTreeMap<u32, BlockAsm>,
    /// [S1B] Mesaj yeniden-birleştirici (bounded `MSG_ASM_CAP`; yalnız fragmentasyon açıkken dolar).
    msgs: BTreeMap<u32, MsgAsm>,
    /// [v2] Koruma bağlamı (open'ın kimliklendirdiği; dönüş-seal bu bağlamla). Dönüş-yönü PN sayacı
    /// artık BURADA DEĞİL: seam sayar (S3 K2) — slot emekliliği/yeniden açılması PN'i sıfırlayamaz.
    /// [K4G-01b] = `Session(h)`, `h` yuva anahtarının oturumu; yaratılırken kurulur, ASLA
    /// değişmez (K5 analoğu: bağ veri datagramıyla değişmez).
    ctx: ProtectionContext,
    /// [v1/PARAM] Per-peer bound-param durumu (fail-closed admission-gate).
    param: ParamState,
    /// [v1/PARAM] Bind-öncesi gelen FEC-shard'ların bounded bekletmesi (PreParamHold deseni;
    /// DÜŞÜRME yarış-duyarlıydı: repair'ler yeniden-üretilmez — CI yavaş-runner'da yakalandı).
    pre_param_fec: Vec<(FecFrameHeader, Vec<u8>)>,
}

impl PeerSlot {
    /// Taze yuva; oturum bağı (`ctx`) burada kurulur ve yuvanın ömrü boyunca değişmez.
    fn new(ctx: ProtectionContext, quota_r: u32, now: u64) -> Self {
        let mut rx = Receiver::new();
        rx.set_quota_r(quota_r);
        Self {
            rx,
            last_seen_ns: now,
            delivered_seen: std::collections::BTreeSet::new(),
            blocks: BTreeMap::new(),
            msgs: BTreeMap::new(),
            ctx,
            param: ParamState::new(),
            pre_param_fec: Vec::new(),
        }
    }
}

/// [K4G-01b] Bağlı oturumu beklenen kümeden çıkmış (kapanmış) yuvaları emekliye ayırır; ayrılan
/// yuva sayısını döner (çağıran `retired_idle_peers`'a ekler — sessiz eviction yok).
///
/// Böyle bir yuva zaten ERİŞİLEMEZDİR: oturumu beklenmeyen datagram `route_server`'da yuvaya hiç
/// bakılmadan kol (c)'de düşer. Emeklilik bu yüzden hiçbir teslim / ACK davranışını değiştirmez,
/// yalnız `max_peers` kapasitesini `idle_hold` beklemeden boşaltır. `NotReady` oturum beklenmeye
/// devam ettiğinden yuvası emekliye AYRILMAZ.
fn retire_closed_slots(peers: &mut HashMap<SlotKey, PeerSlot>, expected: &ExpectedSessions) -> u64 {
    let before = peers.len();
    peers.retain(|&(_, session), _| expected.readiness(session).is_some());
    (before - peers.len()) as u64
}

/// [v1/PARAM] Per-peer pre-param hold tavanı (MAX_PREPARAM_SHARDS hizası, küçük: E≤1KB sınıfı).
const PRE_PARAM_FEC_CAP: usize = 64;

/// Mühürlü wire boyu (pacing/FEC bütçe muhasebesi). `sealed_len` yapısal olarak reddederse
/// (çerçeve bağlamın sınırını aşıyor / oturum yok) düz boy döner: o çerçevenin `seal()`'i de AYNI
/// nedenle reddedilecek ve emisyon kayıp sayılacaktır (INV-RATE-1) — muhasebe burada uydurulmaz.
fn wire_len(seam: &dyn PacketProtection, ctx: ProtectionContext, plaintext_len: usize) -> u64 {
    seam.sealed_len(ctx, plaintext_len).unwrap_or(plaintext_len) as u64
}

/// [v1] Server dönüş-yönü emisyon bağlamı: güvenlik dikişi + opsiyonel egress kapısı + koşu
/// saati. Tek yerde toplanır — böylece `srv_seal_send`'in dokuz çağrı-yeri saati/kapıyı elden
/// ele taşımaz (ve yeni bir çağrı-yeri kapıyı ATLAYAMAZ: tek geçit).
struct SrvTx {
    seam: Box<dyn PacketProtection>,
    egress: Option<Box<dyn EgressPolicy>>,
    start: Instant,
}

/// [v1] Server-yönü tek-nokta seal+send (typed seal-reddi sayılır). Egress kapısı reddederse
/// datagram enjekte EDİLMEZ (kanal-kaybı eşdeğeri; alıcı durumu geri itilmez).
async fn srv_seal_send(
    tx: &mut SrvTx,
    sock: &UdpSocket,
    stats: &mut ServerStats,
    peer: SocketAddr,
    ctx: ProtectionContext,
    frame: Vec<u8>,
) {
    match tx.seam.seal(ctx, &frame) {
        Ok(sd) => {
            let n = sd.bytes.len();
            // DENEME düzlemi (client `tx_bytes` ile aynı sözleşme: ret dahil).
            stats.tx_datagrams += 1;
            stats.tx_bytes += n as u64;
            let permitted = match tx.egress.as_mut() {
                Some(p) => p.admit(n, elapsed_ns(tx.start)),
                None => true,
            };
            if permitted {
                let _ = sock.send_to(&sd.bytes, peer).await; // hata = kayıp.
            } else {
                stats.egress_refused_pkts += 1;
                stats.egress_refused_bytes += n as u64;
            }
        }
        Err(_) => stats.seal_failures += 1,
    }
}

/// [S1] Shard'ı peer'ın blok-birleştiricisine koyar; hazırsa reconstruct dener, kurtarılanları
/// teslim eder. READINESS (receiver_host'taki sayım-bug'ının DÜZELTİLMİŞ hâli): yalnız
/// eksik-source > 0 VE distinct(source)+distinct(repair) ≥ K iken denenir; reconstruct HATASI
/// bloğu SİLMEZ (shard'lar korunur — sonraki shard tamamlayabilir). Tam blok (sources ≥ K) işi
/// bitmiştir, düşülür. Kapasite aşımında en-eski blok düşer (sayaçla görünür).
#[allow(clippy::too_many_arguments)]
fn fec_ingest(
    stats: &mut ServerStats,
    delivery_tx: &mpsc::Sender<Delivery>,
    peer: SocketAddr,
    slot: &mut PeerSlot,
    p: &FecProfile,
    block_id: u32,
    k_actual: u8,
    esi: u16,
    symbol: Vec<u8>,
    is_repair: bool,
    // [S1B] Kurtarılan chunk'ların teslim kapısı için gereken fragmenter bağlamı.
    frag_on: bool,
    max_message_bytes: usize,
) -> Vec<u16> {
    let mut recovered_esis: Vec<u16> = Vec::new();
    let asm = slot.blocks.entry(block_id).or_insert_with(|| BlockAsm {
        k: k_actual,
        sources: BTreeMap::new(),
        repairs: BTreeMap::new(),
    });
    if is_repair {
        // [FAZ-2 / KALEM-1 — MODEL-A] `K_actual` OTORİTESİ REPAIR shard'ıdır: v0 telinde source
        // sembolleri DATA çerçevesiyle gider ve k_actual TAŞIMAZ (alıcı orada k_shape varsayar).
        // Repair başlığı gerçek k_actual'ı taşır (§6 NORMATİF) → blok girdisi burada düzeltilir.
        if k_actual > 0 && usize::from(k_actual) <= p.k_shape_usize() {
            asm.k = k_actual;
        }
        asm.repairs.entry(esi).or_insert(symbol);
    } else {
        asm.sources.entry(esi).or_insert(symbol);
    }
    let (complete, ready) = {
        let k = usize::from(asm.k);
        (
            asm.sources.len() >= k,
            asm.sources.len() + asm.repairs.len() >= k,
        )
    };
    if complete {
        // [C4-ATIF] Kaynaklar parity'yi yendi: tutulan repair'ler boşa gitti.
        if let Some(a) = slot.blocks.remove(&block_id) {
            let held = a.repairs.len() as u64;
            if held > 0 {
                stats.fec_repairs_wasted_complete += held;
                stats.fec_blocks_complete_with_repairs += 1;
            }
        }
    } else if ready {
        let result = {
            let asm = slot.blocks.get(&block_id).expect("entry az önce kuruldu");
            let recv: Vec<ReceivedSymbol> = asm
                .sources
                .iter()
                .map(|(e, b)| ReceivedSymbol {
                    esi: *e,
                    is_repair: false,
                    bytes: b,
                })
                .chain(asm.repairs.iter().map(|(e, b)| ReceivedSymbol {
                    esi: *e,
                    is_repair: true,
                    bytes: b,
                }))
                .collect();
            // [FAZ-2 / KALEM-1] Model-A: `known_zero_count = K_shape − K_actual` canonical-zero
            // shard decoder'a BİLİNEN SOURCE olarak enjekte edilir (rank bedava; §6). Eskiden
            // sabit 0 veriliyordu → kısmi/son blok HİÇ decode edilemezdi.
            let known_zero = p.k_shape_usize().saturating_sub(usize::from(asm.k));
            RsSimdCodec.reconstruct(p, &recv, known_zero)
        };
        match result {
            Ok(recovered) => {
                // [FAZ-2 / KALEM-1] birim-id eşlemesi K_SHAPE'e dayanır (gönderici blok_id'yi
                // `id / k_shape` ile atar), K_actual'a DEĞİL — kısmi blokta `asm.k` kullanmak
                // kurtarılan chunk'ı YANLIŞ birime bağlardı.
                let k = u64::from(p.k_shape);
                for r in recovered {
                    recovered_esis.push(r.esi);
                    let unit_id = u64::from(block_id) * k + u64::from(r.esi);
                    let Ok(pfx) = SourcePrefix::parse(&r.bytes) else {
                        continue;
                    };
                    let len = usize::from(pfx.actual_payload_len);
                    let Some(payload) = r.bytes.get(SOURCE_PREFIX_LEN..SOURCE_PREFIX_LEN + len)
                    else {
                        continue;
                    };
                    // [S1B] Kurtarılan chunk DATA ile AYNI kapıdan geçer: fragmentasyon kapalıysa
                    // doğrudan teslim, açıksa birleştiriciye girer — yani FEC kurtarımı ÇOK
                    // PARÇALI mesajlarda da işe yarar (kurtarılan symbol'ün prefix'i mesaj
                    // kimliğini ve offset'ini zaten taşır). İlk-teslim dedup penceresi ORTAK.
                    admit_chunk(
                        stats,
                        delivery_tx,
                        peer,
                        slot,
                        frag_on,
                        max_message_bytes,
                        unit_id,
                        &pfx,
                        payload,
                        true,
                    );
                }
                if let Some(a) = slot.blocks.remove(&block_id) {
                    stats.fec_repairs_useful += a.repairs.len() as u64;
                    stats.fec_blocks_decoded += 1;
                }
            }
            Err(_) => {
                stats.fec_decode_failures += 1; // blok KORUNUR (bug-fix): shard atılmaz.
            }
        }
    }
    while slot.blocks.len() > RX_BLOCKS_CAP {
        if let Some((_, a)) = slot.blocks.pop_first() {
            let held = a.repairs.len() as u64;
            if held > 0 {
                stats.fec_repairs_wasted_evicted += held;
                stats.fec_blocks_evicted_with_repairs += 1;
            }
        }
        stats.fec_blocks_evicted += 1;
    }
    recovered_esis
}

/// Server görevi: bench `serve_loop` şekli + bounded peers + idle-emeklilik + teslim akışı.
/// [K4G-01] `expected` + `ctx_counters`: bağlam-önce dağıtımın girdileri (bkz. `bind_expecting`).
/// [K4G-11] `delivery_dropped_seen`: `stats.delivery_dropped`'ın salt-gözlem aynası
/// ([`ServerHandle::delivery_dropped`]); yalnız yazılır, görev onu OKUMAZ.
#[allow(clippy::too_many_arguments)]
async fn server_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    seam: Box<dyn PacketProtection>,
    egress: Option<Box<dyn EgressPolicy>>,
    delivery_tx: mpsc::Sender<Delivery>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut t0_rx: mpsc::Receiver<oneshot::Sender<()>>,
    expected: ExpectedSessions,
    ctx_counters: CtxDispatchCounters,
    delivery_dropped_seen: Arc<AtomicU64>,
) -> ServerStats {
    // [K4G-01 / D1 (v1.1)] HubChannel dağıtıcısı: Session durumunun hiçbirini TUTMAZ (yalnız
    // sayaç).
    let hub = HubDispatch {
        counters: ctx_counters.clone(),
    };
    let start = Instant::now();
    let mut tx = SrvTx {
        seam,
        egress,
        start,
    };
    let mut stats = ServerStats::default();
    // [S1B] Fragmentasyon İKİ UÇTA AYNI config'ten türetilir (wire'da bayrak taşınmaz): gönderici
    // 0x05 çerçevesi basıyorsa alıcı da fragmentasyon modundadır. Uyumsuz kurulumda 0x05
    // çerçeveleri decode edilir ama teslim birim granülünde kalır — sessiz bozulma değil,
    // gözlemlenebilir davranış farkı.
    let frag_on = cfg.max_message_bytes.is_some();
    let msg_cap = cfg.max_message_bytes.unwrap_or(cfg.max_unit_bytes);
    // [S1] FEC-runtime (cfg.fec varsa): repair-ingest + blok-birleştirme + kurtarım-teslimi.
    let fec: Option<FecProfile> = cfg.fec.and_then(|f| by_id(f.profile_id));
    // [v1/PARAM] yerel-beklenen canonical alanlar (fail-closed doğrulama tabanı).
    let param_expected: Option<ParamFields> = fec.as_ref().map(param_fields_for);
    // [K4G-01b] Yuva anahtarı `(adres, oturum)` (bkz. `SlotKey`); tavan `cfg.max_peers` yuva.
    let mut peers: HashMap<SlotKey, PeerSlot> = HashMap::new();
    let mut ack_timer = interval(ACK_TICK);
    let mut patience = interval(PATIENCE_TICK);
    let mut buf = vec![0u8; SERVER_RECV_BUF];
    let idle_hold_ns = u64::try_from(cfg.idle_hold.as_nanos()).unwrap_or(u64::MAX);

    loop {
        // [K4G-11] Salt gözlem: sayacın o anki değeri yayımlanır (önceki turun bütün `continue`
        // yolları dahil). Sayaç yalnız `push_delivery`'de artar; burada okunur, değişmez.
        delivery_dropped_seen.store(stats.delivery_dropped, Ordering::Relaxed);
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            // Ölçüm-t0 (ServerHandle::mark_t0): wire sayaçları sıfır, ALICI DURUMU KORUNUR.
            Some(ack) = t0_rx.recv() => {
                let now = elapsed_ns(start);
                if let Some(p) = tx.egress.as_mut() { p.arm(now); }
                stats.tx_datagrams = 0;
                stats.rx_datagrams = 0;
                stats.tx_bytes = 0;
                stats.rx_bytes = 0;
                stats.egress_refused_pkts = 0;
                stats.egress_refused_bytes = 0;
                let _ = ack.send(());
            }
            _ = ack_timer.tick() => {
                let now = elapsed_ns(start);
                // ACK_FREQ_N'e ulaşamayan kuyruğu timer teyit eder (bench birebir).
                let acks: Vec<(SocketAddr, ProtectionContext, Vec<u8>)> = peers
                    .iter_mut()
                    .filter_map(|(&(p, _), s)| {
                        s.rx.on_ack_timer(now).map(|a| (p, s.ctx, a.encode()))
                    })
                    .collect();
                for (peer, sp, plain) in acks {
                    srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, plain).await;
                }
            }
            _ = patience.tick() => {
                // Item 3: idle-peer emekliliği — tablo boşalması YALNIZ burada (sessiz-eviction yok;
                // emeklilik gözlemlenir). ACK-durumu peer'la gider; geç DATA yeni peer-slot açar.
                // [K4G-01b] İkinci neden: bağlı oturum beklenen kümeden çıktı (kapandı) — yuva
                // erişilemez, `idle_hold` beklenmez (`retire_closed_slots`; tablo doluyken yeni
                // yuva yolunda da çağrılır). Kapanmış oturumun geç DATA'sı yuva AÇAMAZ (kol c).
                let now = elapsed_ns(start);
                let mut retired = retire_closed_slots(&mut peers, &expected);
                let before = peers.len();
                peers.retain(|_, s| now.saturating_sub(s.last_seen_ns) < idle_hold_ns);
                retired += (before - peers.len()) as u64;
                stats.retired_idle_peers += retired;
            }
            r = sock.recv_from(&mut buf) => {
                let Ok((n, peer)) = r else { continue };
                stats.rx_datagrams += 1;
                stats.rx_bytes += n as u64;
                let Some(od) = tx.seam.open(&buf[..n]) else {
                    stats.open_failures += 1;
                    continue;
                };
                // [K4G-01 / D1 (v1.1)] Bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer.
                // Yük (`od.frame`) bu noktada OKUNMADI; eş tablosu yalnız OKUNUR (yuvanın
                // bağlandığı bağlam), değişmez. Kol (a) dışındaki her datagram yüke, eş yuvasına
                // (yaratma dahil), `last_seen_ns`'e, alıcı motoruna ve teslim kanalına DOKUNMADAN
                // düşer.
                // ESKİ DAVRANIŞ (kapatılan delik): bağlama bakılmadığı için mevcut bir Session
                // yuvasının adresinden gelen AEAD'siz HubChannel DATA'sı yuvanın bağlamıyla
                // (`Session(h)`) teslim ediliyordu.
                // INV-ACK-DELIVER-1: oturumu henüz `Ready` olmayan beklenen tutamacın datagramı
                // alıcı motoruna GİRMEZ → ACK üretilmez; teslim yolu onu kabul etmeyecekti.
                // [K4G-01b] Yuva `(adres, h)` ile anahtarlıdır: yalnız bu datagramın kendi
                // oturumunun yuvası okunur (aynı adresteki başka oturumun yuvası değil). Kol (a)
                // tek çıktısı yuva anahtarıdır; `Session` kolu yalnız `Session(h)`'den doğar —
                // başka her birleşim kol (c) sayılır (fail-closed).
                let slot_ctx = match od.ctx {
                    ProtectionContext::Session(h) => peers.get(&(peer, h)).map(|s| s.ctx),
                    ProtectionContext::HubChannel => None,
                };
                let key: SlotKey = match (route_server(od.ctx, slot_ctx, &expected), od.ctx) {
                    (CtxRoute::Session, ProtectionContext::Session(h)) => (peer, h),
                    (CtxRoute::Hub, _) => {
                        hub.on_hub_frame(&od);
                        continue;
                    }
                    (CtxRoute::NotReady, _) => {
                        ctx_counters.add_not_ready();
                        continue;
                    }
                    _ => {
                        ctx_counters.add_ctx_mismatch();
                        continue;
                    }
                };
                // [D1a] Alıcının ARQ kimliği = Crypto'nun açtığı PN (`meta.full_wire_pn`; BL D1).
                // Çerçevedeki u16 `seq` (ara çerçeve) artık OKUNMAZ.
                let (seam_ctx, plain, pn) = (od.ctx, od.frame, od.meta.full_wire_pn);
                // [S1] Çerçeve ayrımı: DATA / FEC-shard. İkisi de peer-slot + motor-ACK yolundan
                // geçer (repair de PN işgal eder — RxKind::FecShard motor tarafından ACK'lenir).
                enum RxFrame<'a> {
                    Data {
                        unit_id: u64,
                        payload: &'a [u8],
                    },
                    /// [S1B] Fragmentli DATA: chunk metadata'sı wire'dan AÇIKÇA gelir.
                    /// `body` = prefix ‖ payload (canonical symbol'ün başı).
                    DataFrag {
                        unit_id: u64,
                        pfx: SourcePrefix,
                        body: &'a [u8],
                    },
                    Fec {
                        h: FecFrameHeader,
                        symbol: &'a [u8],
                    },
                    Param {
                        profile_id: u8,
                        code_suite_id: u8,
                        param_hash: [u8; 32],
                    },
                    /// [DR-CS-3] Cold-start probe — oturum durumu GEREKTİRMEZ, idempotent
                    /// echo ile cevaplanır (gönderenin RTT örneği buradan doğar).
                    CsProbe { subtype: u8, probe_seq: u16 },
                }
                let parsed = if let Some((_seq, unit_id, payload)) = decode_data(&plain) {
                    RxFrame::Data { unit_id, payload }
                } else if let Some((_seq, unit_id, pfx, body)) = decode_data_frag(&plain) {
                    RxFrame::DataFrag {
                        unit_id,
                        pfx,
                        body,
                    }
                } else if let Some((h, symbol)) = decode_fec(&plain) {
                    RxFrame::Fec { h, symbol }
                } else if let Some((subtype, probe_seq)) = decode_cs_probe(&plain) {
                    RxFrame::CsProbe { subtype, probe_seq }
                } else if let Some((profile_id, code_suite_id, param_hash)) = decode_param(&plain) {
                    RxFrame::Param {
                        profile_id,
                        code_suite_id,
                        param_hash,
                    }
                } else {
                    stats.unknown_frames += 1; // kontrol-düzlemi dal-noktası (item 6 / B-hattı).
                    continue;
                };
                let now = elapsed_ns(start);
                // Peer-cap FAIL-CLOSED (bench'in sınırsız tablosu SR-AUDIT ihlaliydi — D6):
                // tablo doluyken YENİ peer typed-drop; mevcut peer'lar etkilenmez.
                // [K4G-01b] Yeni yuva = yeni `(adres, oturum)`; buraya yalnız kol (a) (beklenen,
                // `Ready`, AEAD-geçerli) ulaşır. Tablo doluysa önce bağlı oturumu kapanmış
                // (erişilemez) yuvalar emekliye ayrılır; aktif bir yuva ASLA çıkarılmaz.
                if !peers.contains_key(&key) {
                    if peers.len() >= cfg.max_peers {
                        stats.retired_idle_peers += retire_closed_slots(&mut peers, &expected);
                    }
                    if peers.len() >= cfg.max_peers {
                        stats.dropped_new_peers += 1;
                        continue;
                    }
                    peers.insert(key, PeerSlot::new(seam_ctx, cfg.quota_r, now));
                }
                let Some(slot) = peers.get_mut(&key) else { continue };
                slot.last_seen_ns = now;
                match parsed {
                    // [S1B] İki DATA çerçevesi TEK kolda işlenir. Fark yalnızca chunk
                    // metadata'sının NEREDEN geldiğidir:
                    //   0x00 DATA      → prefix `unit_id`'den TÜRETİLİR (mesaj = birim),
                    //   0x05 DATA-FRAG → prefix WIRE'DAN gelir (mesaj = ardışık chunk'lar).
                    // Sonrasındaki her şey — motor beslemesi, ACK, symbol ingest, teslim —
                    // ortaktır; iki ayrı kopya bakım-zamanlı ayrışma riskiydi.
                    RxFrame::Data { .. } | RxFrame::DataFrag { .. } => {
                        let (unit_id, pfx, payload, body) = match parsed {
                            RxFrame::Data { unit_id, payload } => (
                                unit_id,
                                SourcePrefix {
                                    message_id: (unit_id & 0xFFFF_FFFF) as u32,
                                    chunk_offset: 0,
                                    actual_payload_len: u16::try_from(payload.len())
                                        .unwrap_or(u16::MAX),
                                    source_flags: 0,
                                },
                                payload,
                                None,
                            ),
                            RxFrame::DataFrag {
                                unit_id,
                                pfx,
                                body,
                            } => (
                                unit_id,
                                pfx,
                                &body[SOURCE_PREFIX_LEN..],
                                Some(body),
                            ),
                            _ => unreachable!("dış match yalnız iki DATA varyantını seçti"),
                        };
                        let out = slot.rx.on_datagram(
                            RxDatagram {
                                seq: pn,
                                unit_id,
                                kind: RxKind::Data,
                            },
                            now,
                        );
                        // İlk-teslim: PN-yeni ise chunk teslim kapısına girer (dedup orada:
                        // fragmentasyon kapalı→birim-id, açık→mesaj-id).
                        if out.delivered.contains(&unit_id) {
                            admit_chunk(
                                &mut stats,
                                &delivery_tx,
                                peer,
                                slot,
                                frag_on,
                                msg_cap,
                                unit_id,
                                &pfx,
                                payload,
                                false,
                            );
                        }
                        if let Some(ack) = out.ack {
                            let sp = slot.ctx;
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, ack.encode())
                                .await;
                        }
                        // [S1] Source-symbol → blok-birleştirici. 0x00'da symbol TÜRETİLİR (TX ile
                        // aynı deterministik prefix); 0x05'te wire gövdesi ZATEN canonical symbol'ün
                        // başıdır → yalnız pad eklenir, yeniden türetme YOK (byte-eşitlik yapısal).
                        // block/esi her iki durumda da unit_id'den türer — iki uç hizalı.
                        // [v1/PARAM] Source-türetimi param-GATE'SİZ: profil YEREL cfg'den (validate_fec
                        // garantili), admission-semantiği yok, bounded (RX_BLOCKS_CAP) — bind'den önce
                        // gelen DATA'ların symbol'leri kaybolursa blok repair-only kalır ve ASLA
                        // tamamlanamazdı (ilk koşuda yakalanan yarış). Fail-closed gate YALNIZ
                        // FEC-shard ingest'inde (mismatch/saldırı yüzeyi orası).
                        if let Some(p) = fec.as_ref() {
                            let block_id = (unit_id / u64::from(p.k_shape)) as u32;
                            let esi = (unit_id % u64::from(p.k_shape)) as u16;
                            let mut sym = vec![0u8; p.e_usize()];
                            match body {
                                Some(b) if b.len() <= sym.len() => {
                                    sym[..b.len()].copy_from_slice(b);
                                }
                                Some(_) => {
                                    stats.unknown_frames += 1; // gövde E'yi aşıyor — fail-closed
                                    continue;
                                }
                                None => {
                                    let _ = pfx.serialize_symbol(payload, &mut sym);
                                }
                            }
                            let k = u8::try_from(p.k_shape).unwrap_or(255);
                            let rec = fec_ingest(
                                &mut stats,
                                &delivery_tx,
                                peer,
                                slot,
                                p,
                                block_id,
                                k,
                                esi,
                                sym,
                                false,
                                frag_on,
                                msg_cap,
                            );
                            if !rec.is_empty() {
                                stats.fec_recovered_ctrl_tx += 1;
                                let f = encode_recovered(block_id, &rec);
                                let sp = slot.ctx;
                                srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, f).await;
                            }
                        }
                    }
                    RxFrame::Fec { h, symbol } => {
                        let Some(p) = fec.as_ref() else {
                            stats.unknown_frames += 1; // FEC kapalı — çerçeve işlenemez, sayılır.
                            continue;
                        };
                        // [v1/PARAM] fail-closed AMA bounded-HOLD: bind-öncesi shard decode/teslim
                        // ALMAZ, bekletilir; bind'de drain edilir. (Düşürme yarış-duyarlıydı:
                        // repair yeniden-üretilmediğinden kurtarım kalıcı kaybolurdu — CI'da yakalandı.)
                        if !slot.param.is_confirmed() {
                            if slot.pre_param_fec.len() < PRE_PARAM_FEC_CAP {
                                slot.pre_param_fec.push((h, symbol.to_vec()));
                            } else {
                                stats.fec_pre_param_dropped += 1; // taşan düşer (bounded).
                            }
                            // Motor-ACK'i yine verilir (PN işgal edildi; kayıp-sinyali üretme).
                            let is_repair = h.kind == inner_kind::FEC_REPAIR;
                            let out = slot.rx.on_datagram(
                                RxDatagram {
                                    seq: pn,
                                    unit_id: u64::from(h.esi),
                                    kind: RxKind::FecShard {
                                        block_id: h.block_id,
                                        esi: h.esi,
                                        k_actual: h.k_actual,
                                        is_repair,
                                    },
                                },
                                now,
                            );
                            if let Some(ack) = out.ack {
                                let sp = slot.ctx;
                                srv_seal_send(
                                    &mut tx, &sock, &mut stats, peer, sp,
                                    ack.encode(),
                                )
                                .await;
                            }
                            continue;
                        }
                        if h.profile_id != p.profile_id || symbol.len() != p.e_usize() {
                            stats.unknown_frames += 1; // profil/symbol uyumsuz — fail-closed.
                            continue;
                        }
                        stats.fec_repairs_rx += 1;
                        let is_repair = h.kind == inner_kind::FEC_REPAIR;
                        let out = slot.rx.on_datagram(
                            RxDatagram {
                                seq: pn,
                                unit_id: u64::from(h.esi),
                                kind: RxKind::FecShard {
                                    block_id: h.block_id,
                                    esi: h.esi,
                                    k_actual: h.k_actual,
                                    is_repair,
                                },
                            },
                            now,
                        );
                        if let Some(ack) = out.ack {
                            let sp = slot.ctx;
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, ack.encode())
                                .await;
                        }
                        let rec = fec_ingest(
                            &mut stats,
                            &delivery_tx,
                            peer,
                            slot,
                            p,
                            h.block_id,
                            h.k_actual,
                            h.esi,
                            symbol.to_vec(),
                            is_repair,
                            frag_on,
                            msg_cap,
                        );
                        if !rec.is_empty() {
                            stats.fec_recovered_ctrl_tx += 1;
                            let f = encode_recovered(h.block_id, &rec);
                            let sp = slot.ctx;
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, f).await;
                        }
                    }
                    RxFrame::CsProbe { subtype, probe_seq } => {
                        // [DR-CS-3] IDEMPOTENT-STATELESS echo. Alıcı motoruna BESLENMEZ: probe
                        // ayrı ad-uzayındadır, DATA seq'i işgal etmez ve ACK üretmez.
                        // seen_count/arrival_gap 0 gönderilir — şema onları yalnız-ledger ilan
                        // eder (karar girdisi yapılamaz) ve durum tutmak stateless sözleşmesini
                        // bozardı. ANTİ-AMPLİFİKASYON: echo 43 B < probe 44 B (amp ≤ 1).
                        stats.cs_probes_rx += 1;
                        let f = encode_cs_echo(subtype, probe_seq, 0, 0);
                        debug_assert!(f.len() < LEN_CS_PROBE, "echo probe'dan küçük olmalı (amp≤1)");
                        let sp = slot.ctx;
                        stats.cs_echoes_tx += 1;
                        srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, f).await;
                    }
                    RxFrame::Param {
                        profile_id,
                        code_suite_id,
                        param_hash,
                    } => {
                        // [v1/PARAM] §8 fail-closed doğrulama: yerel-beklenen alanlarla hash-verify
                        // → bind (admission açılır); mismatch → red-sayaç, ACK YOK.
                        let Some(exp) = param_expected.as_ref() else {
                            stats.unknown_frames += 1; // FEC kapalı uçta PARAM işlenmez.
                            continue;
                        };
                        let ctl = Control::ParamSet {
                            id: 0,
                            param_hash,
                            profile_id,
                            code_suite_id,
                        };
                        let was = slot.param.is_confirmed();
                        match slot.param.on_param_set(&ctl, exp) {
                            Ok(()) => {
                                if !was {
                                    stats.param_bound_peers += 1;
                                }
                                let f = encode_param_ack(&param_hash);
                                let sp = slot.ctx;
                                srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, f).await;
                                // Bind: bekletilen shard'lar drain → normal ingest (kurtarımlar
                                // RECOVERED ile bildirilir).
                                if let Some(p) = fec.as_ref() {
                                    for (fh, sym) in std::mem::take(&mut slot.pre_param_fec) {
                                        let is_rep = fh.kind == inner_kind::FEC_REPAIR;
                                        let rec = fec_ingest(
                                            &mut stats,
                                            &delivery_tx,
                                            peer,
                                            slot,
                                            p,
                                            fh.block_id,
                                            fh.k_actual,
                                            fh.esi,
                                            sym,
                                            is_rep,
                                            frag_on,
                                            msg_cap,
                                        );
                                        if !rec.is_empty() {
                                            stats.fec_recovered_ctrl_tx += 1;
                                            let f2 = encode_recovered(fh.block_id, &rec);
                                            let sp2 = slot.ctx;
                                            srv_seal_send(
                                                &mut tx, &sock, &mut stats, peer, sp2, f2,
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                            Err(_) => stats.param_rejects += 1,
                        }
                    }
                }
            }
        }
    }
    stats
}

#[cfg(test)]
mod ctx_first_tests {
    //! [K4G-01 / D1 (v1.1)] Yönlendirme tablosunun saf sınaması: kol YALNIZ açılan bağlamdan ve
    //! beklenen tutamaçtan türer — fonksiyonların yük girdisi bile yoktur. Gerçek kripto + gerçek
    //! veri yoluyla uçtan uca regresyon: `tests/ctx_first_dispatch.rs`.
    //! [K4G-01b] + `(adres, oturum)` yuva emeklilik kuralının saf sınaması; uçtan uca:
    //! `tests/session_keyed_slots.rs`.

    use super::*;
    use kama_contracts::s3::SealedPacketMeta;

    const A: ProtectionContext = ProtectionContext::Session(SessionHandle::from_raw(1));
    const B: ProtectionContext = ProtectionContext::Session(SessionHandle::from_raw(2));
    const HUB: ProtectionContext = ProtectionContext::HubChannel;

    #[test]
    fn client_route_requires_the_exact_expected_handle() {
        assert_eq!(route_client(A, A), CtxRoute::Session);
        // "Session mı?" yetmez: başka oturumun tutamacı düşer.
        assert_eq!(route_client(B, A), CtxRoute::Mismatch);
        assert_eq!(route_client(HUB, A), CtxRoute::Hub);
        // Beklenen bağlam HubChannel olsa bile HubChannel Session dağıtıcısına girmez.
        assert_eq!(route_client(HUB, HUB), CtxRoute::Hub);
        assert_eq!(route_client(A, HUB), CtxRoute::Mismatch);
    }

    #[test]
    fn server_route_requires_the_table_and_the_slot_binding() {
        let expected = ExpectedSessions::only(SessionHandle::from_raw(1));
        assert_eq!(route_server(A, None, &expected), CtxRoute::Session);
        assert_eq!(route_server(A, Some(A), &expected), CtxRoute::Session);
        // Tabloda olmayan tutamaç: yuva yoksa da düşer (ilk datagramdan öğrenme yok).
        assert_eq!(route_server(B, None, &expected), CtxRoute::Mismatch);
        assert_eq!(route_server(HUB, None, &expected), CtxRoute::Hub);
        // Mevcut Session yuvasının adresinden gelen HubChannel da Session'a girmez.
        assert_eq!(route_server(HUB, Some(A), &expected), CtxRoute::Hub);
        // Tablo günceldir: eklenen tutamaç beklenir ...
        expected.insert_ready(SessionHandle::from_raw(2));
        assert_eq!(route_server(B, None, &expected), CtxRoute::Session);
        // ... ama bir yuva tek oturuma aittir. [K4G-01b] Sunucu yuvayı `(adres, h)` ile aradığından
        // bu kola gerçek akışta ulaşılmaz (aynı adresteki başka oturum ayrı yuvadır); derinlemesine
        // savunma olarak korunur.
        assert_eq!(route_server(B, Some(A), &expected), CtxRoute::Mismatch);
        // Tablodan çıkan oturumun mevcut yuvası da artık beklenmez.
        expected.replace([(SessionHandle::from_raw(2), Readiness::Ready)]);
        assert_eq!(route_server(A, Some(A), &expected), CtxRoute::Mismatch);
    }

    /// INV-ACK-DELIVER-1: beklenen ama `Ready` olmayan tutamaç Session koluna (ACK üreten alıcı
    /// motoruna) GİRMEZ.
    #[test]
    fn server_route_keeps_not_ready_out_of_the_session_path() {
        let expected = ExpectedSessions::new();
        expected.insert_pending(SessionHandle::from_raw(1));
        assert_eq!(route_server(A, None, &expected), CtxRoute::NotReady);
        // Yuva bağı hazırlıktan önce denetlenir.
        assert_eq!(route_server(A, Some(B), &expected), CtxRoute::Mismatch);
        // Hazırlık `Ready`'ye yükselir ...
        expected.insert_ready(SessionHandle::from_raw(1));
        assert_eq!(route_server(A, None, &expected), CtxRoute::Session);
        // ... ve `insert_pending` onu geri ALMAZ.
        expected.insert_pending(SessionHandle::from_raw(1));
        assert_eq!(route_server(A, None, &expected), CtxRoute::Session);
        // Yeniden yayında aynı tutamaç iki kez gelirse sonuncusu geçerlidir.
        expected.replace([
            (SessionHandle::from_raw(1), Readiness::NotReady),
            (SessionHandle::from_raw(1), Readiness::Ready),
        ]);
        assert_eq!(route_server(A, None, &expected), CtxRoute::Session);
    }

    #[test]
    fn hub_dispatch_only_counts() {
        let counters = CtxDispatchCounters::new();
        let hub = HubDispatch {
            counters: counters.clone(),
        };
        let opened = Opened {
            ctx: HUB,
            frame: vec![0x14; 15],
            meta: SealedPacketMeta {
                full_wire_pn: 0,
                dh_epoch: 0,
                sealed_len: 31,
            },
        };
        hub.on_hub_frame(&opened);
        let want = CtxDispatchStats {
            ctx_mismatch_drops: 0,
            hub_frames_dropped: 1,
            not_ready_drops: 0,
            full_refusals: 0,
            reorder_skip: 0,
            reorder_window_violations: 0,
            distance_holds: 0,
        };
        assert_eq!(counters.snapshot(), want);
    }

    /// [K4G-01b] Yuva `(adres, oturum)` anahtarlıdır: aynı adreste üç oturum üç yuvadır. Bağlı
    /// oturumu beklenen kümeden çıkan (kapanan) yuva emekliye ayrılır; aynı adresteki diğer
    /// oturumların yuvaları — `NotReady` olan dahil, o da hâlâ beklenir — kalır. Yuvanın bağının
    /// (`PeerSlot::ctx`) anahtarının oturumu olduğu burada değil UÇTAN UCA kanıtlanır: aynı
    /// adresten gelen oturumların teslimleri kendi oturumlarıyla etiketlenir —
    /// `tests/session_keyed_slots.rs` (a), (b) (`Delivered::session`, veri alıcısının
    /// `Delivery::ctx`'inden), (d) ve `tests/ctx_first_dispatch.rs`'in uyarlanan adımı (`d.ctx`).
    #[test]
    fn closed_session_slot_retires_and_same_address_slots_stay() {
        let addr = SocketAddr::from(([127, 0, 0, 1], 40_000));
        let [h1, h2, h3] = [1, 2, 3].map(SessionHandle::from_raw);
        let mut peers: HashMap<SlotKey, PeerSlot> = HashMap::new();
        for h in [h1, h2, h3] {
            peers.insert(
                (addr, h),
                PeerSlot::new(ProtectionContext::Session(h), 4, 0),
            );
        }
        assert_eq!(peers.len(), 3, "aynı adres, üç oturum → üç yuva");
        let expected = ExpectedSessions::new();
        expected.insert_ready(h2);
        expected.insert_pending(h3);

        assert_eq!(retire_closed_slots(&mut peers, &expected), 1);
        let mut left: Vec<SessionHandle> = peers.keys().map(|&(_, h)| h).collect();
        left.sort();
        assert_eq!(left, [h2, h3]);
        // Kapanış dışında hiçbir şey yuva düşürmez (idempotent).
        assert_eq!(retire_closed_slots(&mut peers, &expected), 0);
        assert_eq!(peers.len(), 2);
    }
}

#[cfg(test)]
mod delivery_dropped_mirror_tests {
    //! [K4G-11] [`ServerHandle::delivery_dropped`] salt-okunur erişimcisinin sınaması: değer
    //! [`ServerStats::delivery_dropped`] ile AYNI sayaçtır ve kapanışı beklemeden okunur. Bekleme
    //! süreleri yalnız beklenen olayın ÜST SINIRIDIR (koşul sayaç değeridir, zaman değil).

    use super::*;
    use crate::seam::IdentitySeam;

    /// Beklenen sayaç değerinin üst süre sınırı.
    const WAIT: Duration = Duration::from_secs(20);
    /// Yoklama adımı.
    const TICK: Duration = Duration::from_millis(5);
    /// Taşırılan teslim kanalının kapasitesi (en küçük geçerli değer).
    const TINY_QUEUE: usize = 1;
    /// Gönderilen birim: kanal kapasitesinden büyük olmalı ki teslimler taşsın.
    const UNITS: u64 = 16;

    async fn wait_until(mut done: impl FnMut() -> bool) -> bool {
        let deadline = tokio::time::Instant::now() + WAIT;
        while tokio::time::Instant::now() < deadline {
            if done() {
                return true;
            }
            tokio::time::sleep(TICK).await;
        }
        done()
    }

    /// Teslimler BİLEREK drenaj edilmez: kanala ilk birim girer, kalan `UNITS − 1` birim sayılarak
    /// düşer (her birim ilk teslimde tam bir kez `push_delivery`'ye gelir). Ayna bu değere
    /// kapanıştan ÖNCE ulaşır ve kapanış istatistiğiyle aynıdır.
    #[tokio::test(flavor = "multi_thread")]
    async fn live_value_equals_the_overflow_counter_before_shutdown() {
        let server = ServerEndpoint::bind(
            "127.0.0.1:0".parse().expect("adres"),
            RuntimeConfig {
                delivery_queue: TINY_QUEUE,
                ..RuntimeConfig::default()
            },
            Box::new(IdentitySeam::new()),
        )
        .await
        .expect("server bind");
        assert_eq!(server.delivery_dropped(), 0, "taşma yokken sıfır");
        let client = ClientEndpoint::connect(
            server.local_addr(),
            RuntimeConfig::default(),
            Box::new(IdentitySeam::new()),
        )
        .await
        .expect("client connect");
        for i in 0..UNITS {
            client
                .send(format!("taşma-{i}").into_bytes())
                .expect("offer");
        }
        let expected = UNITS - TINY_QUEUE as u64;
        assert!(
            wait_until(|| server.delivery_dropped() == expected).await,
            "canlı değer kapanıştan önce {expected} olmalı, gelen {}",
            server.delivery_dropped()
        );
        client.shutdown().await;
        let stats = server.shutdown().await;
        assert_eq!(
            stats.delivery_dropped, expected,
            "erişimci ve kapanış istatistiği aynı sayaçtır: {stats:?}"
        );
        assert_eq!(stats.delivered, TINY_QUEUE as u64, "{stats:?}");
    }
}

#[cfg(test)]
mod live_diag_mirror_tests {
    //! [K4G-05] [`ClientHandle::live_diag`] salt-okunur aynasının sınaması: değerler motorun ve
    //! tel sayaçlarının değerleridir, kapanışı beklemeden okunur ve kapanış istatistiğini AŞMAZ
    //! (sayaçlar monoton; ayna en çok bir tur geridedir). Kayıp, client'ın egress kapısıyla
    //! belirlenimli verilir: İLK datagram kanala girmez. FEC kapalı kurulur ki kayıp ARQ ile
    //! (parity ile değil) kurtarılsın. Bekleme süreleri yalnız beklenen olayın ÜST SINIRIDIR.
    //!
    //! [DUR-19] Ayna yalnız `RuntimeConfig::live_diag` açıkken vardır: buradaki kollar bayrağı
    //! açar (`no_fec`); `mirror_stays_default_when_the_flag_is_off` aynı trafikte bayrak kapalıyken
    //! aynanın varsayılan kaldığını ve açıkken güncellendiğini sınar.

    use super::*;
    use crate::seam::IdentitySeam;

    /// Beklenen değerin üst süre sınırı.
    const WAIT: Duration = Duration::from_secs(20);
    /// Yoklama adımı.
    const TICK: Duration = Duration::from_millis(5);
    /// Sunulan birim: ilki kaybolur, sonrakilerin ACK'i kaybı ve RTT örneğini getirir.
    const UNITS: u64 = 5;
    /// Kurtarma kapalı kolda kapanış boşaltmasının tavanı (kayıp birim hiç teyit edilmez).
    const SHORT_DRAIN: Duration = Duration::from_millis(200);

    /// İlk `n` datagramı kanala sokmaz, sonrakileri geçirir (belirlenimli ilk-emisyon kaybı).
    struct DropFirst(usize);

    impl EgressPolicy for DropFirst {
        fn admit(&mut self, _len: usize, _now_ns: u64) -> bool {
            if self.0 > 0 {
                self.0 -= 1;
                false
            } else {
                true
            }
        }
    }

    async fn wait_until(mut done: impl FnMut() -> bool) -> bool {
        let deadline = tokio::time::Instant::now() + WAIT;
        while tokio::time::Instant::now() < deadline {
            if done() {
                return true;
            }
            tokio::time::sleep(TICK).await;
        }
        done()
    }

    /// FEC kapalı, canlı ayna AÇIK (DUR-19: ürün varsayılanı kapalıdır; aynayı sınayan kollar
    /// açar).
    fn no_fec(drain_cap: Duration) -> RuntimeConfig {
        RuntimeConfig {
            fec: None,
            drain_cap,
            live_diag: true,
            ..RuntimeConfig::default()
        }
    }

    async fn pair(cfg: &RuntimeConfig) -> (ServerHandle, ClientHandle) {
        let server = ServerEndpoint::bind(
            "127.0.0.1:0".parse().expect("adres"),
            cfg.clone(),
            Box::new(IdentitySeam::new()),
        )
        .await
        .expect("server bind");
        let client = ClientEndpoint::connect_with_policy(
            server.local_addr(),
            cfg.clone(),
            Box::new(IdentitySeam::new()),
            Some(Box::new(DropFirst(1))),
        )
        .await
        .expect("client connect");
        (server, client)
    }

    /// Kurtarma açık: kaybolan ilk birim ARQ ile yeniden gönderilir. Ayna, kapanıştan ÖNCE
    /// retransmit kararını, tele çıkan retransmit baytını ve RTT örneğini gösterir; kapanış
    /// istatistiği aynanın değerlerinden küçük değildir.
    #[tokio::test(flavor = "multi_thread")]
    async fn live_values_show_the_retransmit_before_shutdown() {
        let cfg = no_fec(RuntimeConfig::default().drain_cap);
        let (server, client) = pair(&cfg).await;
        for i in 0..UNITS {
            client
                .send(format!("ayna-{i}").into_bytes())
                .expect("offer");
        }
        assert!(
            wait_until(|| {
                let live = client.live_diag();
                client.outstanding() == 0 && live.retransmits >= 1 && live.rtt_samples >= 1
            })
            .await,
            "kayıp kurtarılmalı ve ayna kapanıştan önce göstermeli: {:?}",
            client.live_diag()
        );
        let live = client.live_diag();
        assert!(live.tx_retx_bytes > 0, "retransmit tele çıktı: {live:?}");
        assert_eq!(live.recovery_suppressed, 0, "kurtarma açık: {live:?}");
        // [K4G-05 TEŞHİS] Kurtarma görüntüsü oranlı yenilenir: teyitsiz birim kalmadığını gösterir.
        assert!(
            wait_until(|| client
                .live_diag()
                .recovery
                .is_some_and(|r| r.unacked_units == 0 && r.oldest_unacked.is_none()))
            .await,
            "kurtarma görüntüsü teyitsiz birim kalmadığını göstermeli: {:?}",
            client.live_diag()
        );
        // [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] ACK çerçevesi sayaçları ve halkası: ilerleme getiren
        // çerçeve, motorun işlediği çerçeveyi; o da çözülen çerçeveyi aşmaz. Halka toplamı çerçeve
        // sayısıdır, kayıtlar ardışık sıralıdır; FEC kapalıyken her birim bir ACK çerçevesiyle İLK
        // kez teyit edilir.
        let acks = client.live_diag();
        assert!(
            acks.ack.frames >= 1
                && acks.ack.new_units >= 1
                && acks.ack.new_units <= acks.ack.mapped
                && acks.ack.mapped <= acks.ack.frames,
            "{:?}",
            acks.ack
        );
        assert_eq!(acks.acks.total(), acks.ack.frames);
        let records: Vec<AckFrameDiag> = acks.acks.recent().copied().collect();
        assert!(
            records.windows(2).all(|w| w[1].index == w[0].index + 1),
            "{records:?}"
        );
        assert_eq!(records.last().map(|r| r.index), Some(acks.ack.frames));
        assert!(records.iter().all(|r| r.mapped_seqs <= r.acked_pns));
        if acks.acks.total() <= ACK_LOG_LEN as u64 {
            let confirmed: u64 = records.iter().map(|r| u64::from(r.new_units)).sum();
            assert_eq!(confirmed, UNITS, "{records:?}");
        }
        // Denetim durumu ve durum değişikliği kaydı: hız ve durum yazıldı; ilk kayıt öncesizdir.
        assert!(
            acks.clock_origin.is_some() && acks.control.state.is_some(),
            "{:?}",
            acks.control
        );
        assert!(acks.control.pacer_rate_bps > 0, "{:?}", acks.control);
        assert!(acks.states.total() >= 1);
        if acks.states.total() <= STATE_LOG_LEN as u64 {
            assert_eq!(acks.states.recent().next().map(|s| s.from), Some(None));
        }
        let stats = client.shutdown().await;
        let _ = server.shutdown().await;
        assert!(
            stats.fate.activation.retransmits >= live.retransmits,
            "kapanış {} < ayna {}",
            stats.fate.activation.retransmits,
            live.retransmits
        );
        assert!(stats.tx_retx_bytes >= live.tx_retx_bytes, "{stats:?}");
        assert!(stats.fate.rtt_samples >= live.rtt_samples, "{stats:?}");
    }

    /// [K4G-05 / CEVAP PAKETİ 6 TEŞHİS] Halka en yeni `N` kaydı eskiden yeniye verir; toplam,
    /// halkadan düşenleri de sayar.
    #[test]
    fn diag_ring_keeps_the_newest_records_in_order() {
        let mut ring: DiagRing<u64, 3> = DiagRing::default();
        assert_eq!(ring.recent().count(), 0);
        for v in 1..=2 {
            ring.push(v);
        }
        assert_eq!(ring.recent().copied().collect::<Vec<_>>(), [1, 2]);
        for v in 3..=7 {
            ring.push(v);
        }
        assert_eq!(ring.total(), 7);
        assert_eq!(ring.recent().copied().collect::<Vec<_>>(), [5, 6, 7]);
    }

    /// Kurtarma kapalı (yalnız test yapılandırması): retransmit kararı tele çıkmaz. Ayna bastırılan
    /// kararı kapanıştan önce gösterir, tele çıkan retransmit baytı sıfır kalır ve kaybolan birim
    /// teyitsiz kalır.
    #[tokio::test(flavor = "multi_thread")]
    async fn live_values_show_suppressed_recovery_before_shutdown() {
        let cfg = no_fec(SHORT_DRAIN);
        let (server, client) = pair(&cfg).await;
        client.set_recovery(false);
        for i in 0..UNITS {
            client
                .send(format!("ayna-{i}").into_bytes())
                .expect("offer");
        }
        assert!(
            wait_until(|| client.live_diag().recovery_suppressed >= 1).await,
            "bastırılan kurtarma kararı aynada görünmeli: {:?}",
            client.live_diag()
        );
        let live = client.live_diag();
        assert_eq!(live.tx_retx_bytes, 0, "tele retransmit çıkmadı: {live:?}");
        assert!(client.outstanding() >= 1, "kaybolan birim teyitsiz kalır");
        // [K4G-05 TEŞHİS] Takılı birim görüntüsü: en eski teyitsiz birim kaybolan ilk birimdir (0) ve
        // orijinalinden sonra en az bir (bastırılan) kurtarma kararı almıştır.
        assert!(
            wait_until(|| client
                .live_diag()
                .recovery
                .and_then(|r| r.oldest_unacked)
                .is_some_and(|u| u.unit_id == 0 && u.emission_decisions >= 2))
            .await,
            "takılı birim görüntüsü: {:?}",
            client.live_diag()
        );
        let stats = client.shutdown().await;
        let _ = server.shutdown().await;
        assert!(
            stats.recovery_suppressed >= live.recovery_suppressed,
            "{stats:?}"
        );
        assert_eq!(stats.tx_retx_bytes, 0, "{stats:?}");
    }

    /// [DUR-19] Canlı ayna bayrağı (`RuntimeConfig::live_diag`), ürün varsayılanı KAPALI. Aynı
    /// kayıplı trafik (ilk emisyon düşer, ARQ kurtarır) iki kolda da koşar ve tamamlanır: motorun
    /// kurtarma kararı bayraktan bağımsızdır (kapanış istatistiği). Kapalı kolda ayna kurulmaz ve
    /// yazılmaz — erişimci trafik BOYUNCA ve sonunda varsayılan görüntüyü döner; açık kolda ayna
    /// güncellenir.
    #[tokio::test(flavor = "multi_thread")]
    async fn mirror_stays_default_when_the_flag_is_off() {
        assert!(!RuntimeConfig::default().live_diag, "ürün kapalı");
        for on in [false, true] {
            let cfg = RuntimeConfig {
                live_diag: on,
                ..no_fec(RuntimeConfig::default().drain_cap)
            };
            let (server, client) = pair(&cfg).await;
            assert_eq!(client.live_diag_enabled(), on);
            for i in 0..UNITS {
                client
                    .send(format!("ayna-{i}").into_bytes())
                    .expect("offer");
            }
            // Her yoklamada aynaya bakılır: kapalı kolda hiçbir an yazılmış olmamalı.
            let mut written = false;
            let settled = wait_until(|| {
                let live = client.live_diag();
                written |= live != ClientLiveDiag::default();
                client.outstanding() == 0 && (!on || live.ack.frames >= 1)
            })
            .await;
            assert!(
                settled,
                "bayrak {on}: kayıp kurtarılmalı: {:?}",
                client.live_diag()
            );
            let live = client.live_diag();
            let stats = client.shutdown().await;
            let _ = server.shutdown().await;
            assert_eq!(stats.acked, UNITS, "bayrak {on}: {stats:?}");
            assert!(
                stats.fate.activation.retransmits >= 1 && stats.tx_retx_bytes > 0,
                "bayrak {on}: ARQ kurtarması bayraktan bağımsız: {stats:?}"
            );
            if on {
                assert!(written, "açık: ayna yazıldı");
                assert!(
                    live.clock_origin.is_some() && live.ack.frames >= 1,
                    "{live:?}"
                );
            } else {
                assert!(!written, "kapalı: ayna trafik boyunca varsayılan kaldı");
                assert_eq!(live, ClientLiveDiag::default());
            }
        }
    }
}
