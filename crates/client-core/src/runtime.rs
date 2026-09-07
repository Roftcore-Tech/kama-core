//! P1 — transport çalışma-zamanı kabuğu (PRODUCT-RUNTIME item 2/3/7 + 8/9 kapıları).
//!
//! Tasarım otoritesi `docs/design/P1-RUNTIME-SHELL.md` (D1-D10). Döngü iskeleti kampanya-kanıtlı
//! bench şeklidir (tek görev `select!`, PACE 1 ms → `Timer::Pto` pompası, RTPROP 200 ms, alıcıda
//! ACK 20 ms; taraf başına TEK `Instant`, motor saf u64-ns olay-güdümlü — time-model kuralı).
//! `next_wakeup_ns()`'e tek-kaynak-uyku YOK (D3, EK4-A2 tarihi): pompa idempotent + kanıtlı.
//!
//! SoC: motor yalnız `OpaqueUnit{id,size}` / `RxDatagram` metadata görür; datagram'lar
//! [`crate::seam::SecuritySeam`]'den mühürlenir/açılır (kripto AYRI düzlem). INV-RATE-1: gönderim
//! hatası = kanal kaybı; motor durumu emisyonu ZATEN işledi, geri itilmez, rate'e dokunulmaz.
//!
//! SINIRLAR (SR-AUDIT-001, bounds-before-alloc; motor `tx_queue`'yu sınırlamaz — kabuk sınırlar):
//! offer-kanalı bounded (typed `QueueFull`) · unit-store bounded (doluyken offer alınmaz — doğal
//! backpressure) · server peer tablosu cap'li fail-closed (yeni-peer typed-drop + sayaç; aktif peer
//! ASLA sessiz-evict edilmez, boşalma idle-emeklilikle) · teslim kanalı bounded (`try_send`,
//! taşan teslim sayaçla raporlanır — app drenaj sözleşmesi).

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
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
use adaptive_transport::seq::resolve_seq;
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

use crate::seam::{SealArgs, SeamPeer, SecuritySeam};
use crate::wire::{
    decode_cs_echo, decode_cs_probe, decode_data, decode_data_frag, decode_fec, decode_param,
    decode_param_ack, decode_recovered, encode_cs_echo, encode_cs_probe, encode_data,
    encode_data_frag, encode_fec, encode_param, encode_param_ack, encode_recovered, FecFrameHeader,
    CS_PROBE_SUBTYPE, FRAME_CS_ECHO, FRAME_PARAM_ACK, FRAME_RECOVERED, HDR_DATA, HDR_DATA_FRAG,
    HDR_FEC, LEN_CS_PROBE,
};
use crate::ClientSession;

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
    pub max_peers: usize,
    /// Alıcı R-kotası (sessizlik re-ACK bütçesi).
    pub quota_r: u32,
    /// Shutdown drain süre-tavanı (motor birim terk etmez → drain CAP'siz sonlanmayabilir, D8).
    pub drain_cap: Duration,
    /// Server idle-peer emeklilik eşiği (item 3; SESSION_IDLE_HOLD hizası 105 s).
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

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_inflight_units: 4096,
            offer_queue: 1024,
            delivery_queue: 1024,
            max_unit_bytes: 1200,
            // Fragmenter KAPALI (mesaj = birim; eski davranış birebir).
            max_message_bytes: None,
            max_peers: 64,
            quota_r: 4,
            drain_cap: Duration::from_secs(3),
            idle_hold: Duration::from_secs(105),
            // [IW] Pacer'ın KENDİ başlangıç kredisi — yeni bir sabit değil (bkz. alan dokümanı).
            initial_window_bytes: adaptive_transport::pacer::DEFAULT_BURST_CAP,
            bootstrap_rtt: false,
            bootstrap_fast_ladder: false,
            fec_structure_policy: FecStructurePolicy::default(),
            fec: None,
            // Sevk default'u KAPALI: kalibrasyon opt-in kalır (ürün default kararı ayrı bir
            // ölçüm kapısıdır; bkz. reports/fec-policy-arm-v1/).
            fusion_policy: None,
            loss_confirm_horizon: LossConfirmHorizon::CadenceLegacy,
            fec_loss_risk: FecLossRiskConfig::bounded_v1(),
            ts_sample_period_ns: None,
            structure_gate_level_permille: Some(250),
        }
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
    /// İlk-ACK'i görülen birim (ilk-ack-kazanır dedup).
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
    /// store'dan düşer, app `acked()` akışına girer).
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
    /// [TEŞHİS] Yapı-gate'in karar tabanı (kapanış anındaki fotoğraf) — bkz. [`FateWindowDiag`].
    pub fate: FateWindowDiag,
    /// [DR-CS-3] Wire'a yazılan CS-probe (0 = bootstrap kapalı VEYA PreConverge hiç yaşanmadı).
    pub cs_probes_tx: u64,
    /// [DR-CS-3] Alınan ve motora beslenen CS-echo — RTT örneğinin tek kaynağı.
    pub cs_echoes_rx: u64,
}

/// Server koşu istatistikleri.
#[derive(Debug, Default, Clone, Copy)]
pub struct ServerStats {
    /// App'e teslim edilen birim (ilk-teslim).
    pub delivered: u64,
    /// Teslim kanalı doluyken düşen teslim (app drenaj sözleşmesi ihlali görünür olsun).
    pub delivery_dropped: u64,
    /// Peer-cap fail-closed reddi (yeni peer, tablo dolu).
    pub dropped_new_peers: u64,
    /// Idle-emekliliğe ayrılan peer (item 3).
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

/// Teslim olayı: hangi eşten hangi birim, payload'ıyla.
#[derive(Debug, Clone)]
pub struct Delivery {
    pub peer: SocketAddr,
    pub unit_id: u64,
    pub payload: Vec<u8>,
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
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
    task: JoinHandle<ClientStats>,
}

/// Çok-üreticili offer tutamağı ([`ClientHandle::sender`]). ACK akışını `&mut` ile tutan
/// döngüden BAĞIMSIZ ödünç alınabilir (aynı `select!` içinde hem sunmak hem ACK drenajı).
#[derive(Clone)]
pub struct ClientSender {
    offer_tx: mpsc::Sender<(Vec<u8>, Option<u64>)>,
    /// [S1B] App-yüzü tavan MESAJ tavanıdır; birim (chunk) tavanı fragmenter'ın içinde kalır.
    max_message_bytes: usize,
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
            })
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
        seam: Box<dyn SecuritySeam>,
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
        seam: Box<dyn SecuritySeam>,
        egress: Option<Box<dyn EgressPolicy>>,
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
        let task = tokio::spawn(client_task(
            sock,
            cfg,
            seam,
            egress,
            offer_rx,
            acked_tx,
            shutdown_rx,
            t0_rx,
        ));
        Ok(ClientHandle {
            offer: ClientSender {
                offer_tx,
                max_message_bytes,
            },
            acked_rx,
            shutdown_tx,
            t0_tx,
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
    pub async fn acked(&mut self) -> Option<u64> {
        self.acked_rx.recv().await
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
async fn client_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    mut seam: Box<dyn SecuritySeam>,
    mut egress: Option<Box<dyn EgressPolicy>>,
    mut offer_rx: mpsc::Receiver<(Vec<u8>, Option<u64>)>,
    acked_tx: mpsc::Sender<u64>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut t0_rx: mpsc::Receiver<oneshot::Sender<()>>,
) -> ClientStats {
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
    // [v1] Seam-PN sayacı (per-endpoint; noise-wire caller-supplied sözleşmesi) + PARAM planı.
    let mut tx_pn: u64 = 0;
    const CLIENT_SEAM_PEER: SeamPeer = 0;
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
    let mut repair_store: BTreeMap<u32, Vec<RepairSymbol>> = BTreeMap::new();
    const REPAIR_STORE_BLOCKS: usize = 64;
    // unit-store: unit_id → payload; varlık = outstanding. İlk-ACK'te düşer (sınır: cfg).
    let mut store: HashMap<u64, Vec<u8>> = HashMap::new();
    // seq → unit haritası + birim → seq'leri (ACK'te toplu budama — harita sınırlı kalır).
    let mut seq_to_unit: HashMap<u64, u64> = HashMap::new();
    let mut unit_seqs: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut max_emitted_seq: u64 = 0;
    let mut next_unit: u64 = 0;
    let mut pace = interval(PACE_TICK);
    let mut rtprop = interval(RTPROP_TICK);
    let mut buf = vec![0u8; CLIENT_RECV_BUF];

    // Motoru bir olayla sürer + kararları mühürleyip wire'a yazar. seq→unit kaydı GÖNDERİMDEN
    // BAĞIMSIZ önce yapılır (acked_full(max_emitted+1) doğruluğu buna bağlı; refused/failed send
    // de seq'ini işgal eder). Send hatası = kanal kaybı (INV-RATE-1; abort YOK).
    // [v1] Tek-nokta seal+send: typed seal-reddi sayılır, PN yalnız başarılı seal'de ilerler.
    macro_rules! seal_send {
        ($frame:expr, $class:expr, $now:expr) => {
            match seam.seal(
                CLIENT_SEAM_PEER,
                $frame,
                SealArgs {
                    wire_pn: tx_pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            ) {
                Ok(sd) => {
                    tx_pn = tx_pn.wrapping_add(1);
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
                }
                Err(_) => {
                    stats.seal_failures += 1; // typed-red; emisyon kayıp sayıldı.
                }
            }
        };
    }
    macro_rules! drive {
        ($ev:expr, $now:expr) => {
            for d in session.on_event($ev, $now) {
                // Seq-uzayı takibi HER kararda (acked_full(max+1) doğruluğu — repair'ler de seq
                // işgal eder); ama seq→unit haritası YALNIZ birim-taşıyan kararlarda: FecParity'nin
                // unit_id'si ESI'dir (AYRI ad-uzayı) — haritaya girseydi repair-ACK'i gerçek bir
                // birimin store'unu silerdi (S1'de yakalanan tuzak).
                max_emitted_seq = max_emitted_seq.max(d.seq);
                let carries_unit = matches!(
                    d.kind,
                    TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
                );
                if carries_unit {
                    seq_to_unit.insert(d.seq, d.unit_id);
                    unit_seqs.entry(d.unit_id).or_default().push(d.seq);
                } else {
                    // [S1] FecParity (FEC-açıkken): repair-store'daki gerçek RS-repair →
                    // FRAME_FEC → seam → wire. Bulunamazsa (evict/latch-0) başlık-only emisyon
                    // (DATA store-miss gerekçesiyle aynı: motor her emisyonu inflight izler; hiç
                    // göndermemek yapay kayıp sinyali üretirdi).
                    if d.kind == TransmitKind::FecParity {
                        if let (Some(p), Some(hint)) = (fec.as_ref(), d.send_record) {
                            let SrLogicalRef::Fec { block_id, esi } = hint.logical_ref;
                            let sym: Option<&[u8]> = repair_store
                                .get(&block_id)
                                .and_then(|rs| rs.iter().find(|r| r.esi == esi))
                                .map(|r| r.bytes.as_slice());
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
                                    k_actual: u8::try_from(p.k_shape).unwrap_or(255),
                                    seq: (d.seq & 0xFFFF) as u16,
                                },
                                sym.unwrap_or(&[]),
                            );
                            seal_send!(frame, TxClass::Fec, $now);
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
                        seal_send!(f, TxClass::Ctrl, $now);
                        continue;
                    }
                    // FEC-kapalıyken FecParity: v0 kabuk taşımaz, sayar (bootstrap OFF;
                    // sevk default'larında üretilmezler — typed genişleme noktası).
                    stats.stale_decisions += 1;
                    continue;
                }
                // Store-miss = birim BAŞKA bir kopyasıyla çoktan teslim edilmiş, motor eski seq'in
                // retransmit'ini istiyor. Emisyon YİNE wire'a çıkar (başlık-only): motor her
                // emisyonu inflight izler — hiç göndermemek yapay kayıp sinyali üretirdi (bench her
                // kararı gönderirdi). Alıcı birimi dedup eder, seq'i ACK'ler → motor temiz kapatır.
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
                // canonical sıfır-prefix konur: alıcı çözer, seq ACK'lenir, birleştiriciye
                // len=0 katkısıyla girer (aynı esi'de gerçek symbol first-wins ile korunur).
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
                seal_send!(frame, class, $now);
            }
        };
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
    // [S1B] Bir BİRİMİN teslim-teyidi app'e nasıl yansır.
    //
    // Fragmentasyon KAPALIYKEN birim = mesaj → id doğrudan akar (eski davranış birebir).
    // AÇIKKEN app mesaj granülünde konuşur: birim ACK'i mesajın kalan-chunk sayacını düşürür ve
    // ACK yalnız SON chunk teyitlendiğinde verilir — yarım mesaj "teslim edildi" diye
    // raporlanamaz. ACK ve FEC-RECOVERED yolları BU tek noktayı paylaşır.
    macro_rules! unit_acked {
        ($unit:expr) => {
            if frag_on {
                if let Some(m) = unit_msg.remove(&$unit) {
                    if let Some(rem) = msg_pending.get_mut(&m) {
                        *rem = rem.saturating_sub(1);
                        if *rem == 0 {
                            msg_pending.remove(&m);
                            stats.messages_acked += 1;
                            let _ = acked_tx.try_send(u64::from(m));
                        }
                    }
                }
            } else {
                // app drenajı; taşarsa olay düşer, teslim durumu store'da zaten kesin.
                let _ = acked_tx.try_send($unit);
            }
        };
    }
    // Gelen datagram: seam.open → ACK (V1 0x10 / V2 0x14) → ilk-ACK damgası + motor beslemesi.
    macro_rules! handle_rx {
        ($raw:expr, $now:expr) => {
            let raw: &[u8] = $raw;
            stats.rx_datagrams += 1;
            stats.rx_bytes += raw.len() as u64;
            if let Ok(od) = seam.open(raw) {
                let plain = od.plain;
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
                    // [S1] RECOVERED-control: alıcı bu ESI'leri FEC ile kurtardı → birim teslim-
                    // teyitli: store'dan düşer, app acked() akışına girer, motor saf-logical iptal
                    // eder (retransmit israfı biter). unit_id = block*K + esi (deterministik atama).
                    match (fec.as_ref(), decode_recovered(&plain)) {
                        (Some(p), Some((block_id, esis))) => {
                            let mut units = Vec::with_capacity(esis.len());
                            for esi in esis {
                                let unit =
                                    u64::from(block_id) * u64::from(p.k_shape) + u64::from(esi);
                                if store.remove(&unit).is_some() {
                                    stats.fec_recovered_ctrl_units += 1;
                                    unit_acked!(unit);
                                    for old in unit_seqs.remove(&unit).unwrap_or_default() {
                                        seq_to_unit.remove(&old);
                                    }
                                }
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
                        let acked = frame.acked_full(max_emitted_seq + 1);
                        for s in &acked {
                            let Some(&unit) = seq_to_unit.get(s) else { continue };
                            if store.remove(&unit).is_some() {
                                stats.acked += 1;
                                unit_acked!(unit);
                                // Birimin TÜM seq kayıtları budanır — haritalar store disipliniyle sınırlı kalır.
                                for old in unit_seqs.remove(&unit).unwrap_or_default() {
                                    seq_to_unit.remove(&old);
                                }
                            }
                        }
                        // Kampanya-kanıtlı sözleşme (bench birebir): acked_full çıktısı diff'siz
                        // beslenir; motor inflight.remove ile idempotent işler.
                        drive!(
                            Event::AckReceived(AckInfo {
                                acked_seqs: acked,
                                ack_delay: frame.ack_delay_duration(),
                            }),
                            $now
                        );
                    }
                }
            } else {
                stats.open_failures += 1; // verify-fail: sessiz düşür, durum değişmez.
            }
        };
    }

    // ── Ana döngü ──
    loop {
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
                    let size = seam.sealed_len(hdr + chunk.len()) as u64;
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
                        let parity_wire = seam.sealed_len(HDR_FEC + e) as u64;
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
                                repair_store.insert(cur_block_id, reps);
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
                            seal_send!(f, TxClass::Ctrl, now);
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
}

impl ServerEndpoint {
    /// Bağla ve görevi başlat.
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind hatası.
    pub async fn bind(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn SecuritySeam>,
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
        seam: Box<dyn SecuritySeam>,
        egress: Option<Box<dyn EgressPolicy>>,
    ) -> Result<ServerHandle, RuntimeError> {
        validate_fec(&cfg)?; // [S1] koşu-öncesi typed FEC-config doğrulaması
        let sock = UdpSocket::bind(bind).await?;
        let local_addr = sock.local_addr()?;
        let (delivery_tx, delivery_rx) = mpsc::channel(cfg.delivery_queue);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Kapasite 1 + onay-oneshot: mark_t0 görev işleyene dek bekler (ölçüm sözleşmesi).
        let (t0_tx, t0_rx) = mpsc::channel::<oneshot::Sender<()>>(1);
        let task = tokio::spawn(server_task(
            sock,
            cfg,
            seam,
            egress,
            delivery_tx,
            shutdown_rx,
            t0_rx,
        ));
        Ok(ServerHandle {
            delivery_rx,
            shutdown_tx,
            t0_tx,
            task,
            local_addr,
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

    /// Ölçüm-t0 işareti (dönüş yönü): wire sayaçları sıfırlanır, ALICI DURUMU KORUNUR ve varsa
    /// [`EgressPolicy`] arm edilir. Görev işleyene dek bekler — bkz. [`ClientHandle::mark_t0`].
    pub async fn mark_t0(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.t0_tx.send(ack_tx).await.is_ok() {
            let _ = ack_rx.await;
        }
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
    unit_id: u64,
    payload: Vec<u8>,
    from_fec: bool,
) {
    let d = Delivery {
        peer,
        unit_id,
        payload,
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
        push_delivery(stats, delivery_tx, peer, mid, full, from_fec);
    } else if slot.msgs.len() > MSG_ASM_CAP {
        slot.msgs.pop_first();
        stats.msg_asm_evicted += 1;
    }
}

/// Eş yuvası: alıcı motoru + idle-emeklilik çapası (item 3) + teslim-dedup penceresi.
struct PeerSlot {
    rx: Receiver,
    last_seen_ns: u64,
    delivered_seen: std::collections::BTreeSet<u64>,
    /// [S1] FEC blok-birleştirici (bounded; yalnız FEC-açıkken dolar).
    blocks: BTreeMap<u32, BlockAsm>,
    /// [S1B] Mesaj yeniden-birleştirici (bounded `MSG_ASM_CAP`; yalnız fragmentasyon açıkken dolar).
    msgs: BTreeMap<u32, MsgAsm>,
    /// [v1] Seam-peer (open'ın kimliklendirdiği; dönüş-seal bu kimlikle).
    seam_peer: SeamPeer,
    /// [v1] Dönüş-yönü seam-PN sayacı (per-peer monoton).
    tx_pn: u64,
    /// [v1/PARAM] Per-peer bound-param durumu (fail-closed admission-gate).
    param: ParamState,
    /// [v1/PARAM] Bind-öncesi gelen FEC-shard'ların bounded bekletmesi (PreParamHold deseni;
    /// DÜŞÜRME yarış-duyarlıydı: repair'ler yeniden-üretilmez — CI yavaş-runner'da yakalandı).
    pre_param_fec: Vec<(FecFrameHeader, Vec<u8>)>,
}

/// [v1/PARAM] Per-peer pre-param hold tavanı (MAX_PREPARAM_SHARDS hizası, küçük: E≤1KB sınıfı).
const PRE_PARAM_FEC_CAP: usize = 64;

impl PeerSlot {
    fn next_pn(&mut self) -> u64 {
        let v = self.tx_pn;
        self.tx_pn = self.tx_pn.wrapping_add(1);
        v
    }
}

/// [v1] Server dönüş-yönü emisyon bağlamı: güvenlik dikişi + opsiyonel egress kapısı + koşu
/// saati. Tek yerde toplanır — böylece `srv_seal_send`'in dokuz çağrı-yeri saati/kapıyı elden
/// ele taşımaz (ve yeni bir çağrı-yeri kapıyı ATLAYAMAZ: tek geçit).
struct SrvTx {
    seam: Box<dyn SecuritySeam>,
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
    sp: SeamPeer,
    pn: u64,
    frame: Vec<u8>,
) {
    match tx.seam.seal(
        sp,
        frame,
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    ) {
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
            RsSimdCodec.reconstruct(p, &recv, 0)
        };
        match result {
            Ok(recovered) => {
                let k = slot.blocks.get(&block_id).map_or(k_actual, |a| a.k);
                for r in recovered {
                    recovered_esis.push(r.esi);
                    let unit_id = u64::from(block_id) * u64::from(k) + u64::from(r.esi);
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
async fn server_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    seam: Box<dyn SecuritySeam>,
    egress: Option<Box<dyn EgressPolicy>>,
    delivery_tx: mpsc::Sender<Delivery>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut t0_rx: mpsc::Receiver<oneshot::Sender<()>>,
) -> ServerStats {
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
    let mut peers: HashMap<SocketAddr, PeerSlot> = HashMap::new();
    let mut ack_timer = interval(ACK_TICK);
    let mut patience = interval(PATIENCE_TICK);
    let mut buf = vec![0u8; SERVER_RECV_BUF];
    let idle_hold_ns = u64::try_from(cfg.idle_hold.as_nanos()).unwrap_or(u64::MAX);

    loop {
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
                let acks: Vec<(SocketAddr, SeamPeer, u64, Vec<u8>)> = peers
                    .iter_mut()
                    .filter_map(|(p, s)| {
                        s.rx.on_ack_timer(now)
                            .map(|a| (*p, s.seam_peer, s.next_pn(), a.encode()))
                    })
                    .collect();
                for (peer, sp, pn, plain) in acks {
                    srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, plain).await;
                }
            }
            _ = patience.tick() => {
                // Item 3: idle-peer emekliliği — tablo boşalması YALNIZ burada (sessiz-eviction yok;
                // emeklilik gözlemlenir). ACK-durumu peer'la gider; geç DATA yeni peer-slot açar.
                let now = elapsed_ns(start);
                let before = peers.len();
                peers.retain(|_, s| now.saturating_sub(s.last_seen_ns) < idle_hold_ns);
                stats.retired_idle_peers += (before - peers.len()) as u64;
            }
            r = sock.recv_from(&mut buf) => {
                let Ok((n, peer)) = r else { continue };
                stats.rx_datagrams += 1;
                stats.rx_bytes += n as u64;
                let Ok(od) = tx.seam.open(&buf[..n]) else {
                    stats.open_failures += 1;
                    continue;
                };
                let (seam_peer, plain) = (od.peer, od.plain);
                // [S1] Çerçeve ayrımı: DATA / FEC-shard. İkisi de peer-slot + motor-ACK yolundan
                // geçer (repair de seq işgal eder — RxKind::FecShard motor tarafından ACK'lenir).
                enum RxFrame<'a> {
                    Data {
                        wire_seq: u16,
                        unit_id: u64,
                        payload: &'a [u8],
                    },
                    /// [S1B] Fragmentli DATA: chunk metadata'sı wire'dan AÇIKÇA gelir.
                    /// `body` = prefix ‖ payload (canonical symbol'ün başı).
                    DataFrag {
                        wire_seq: u16,
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
                let parsed = if let Some((wire_seq, unit_id, payload)) = decode_data(&plain) {
                    RxFrame::Data {
                        wire_seq,
                        unit_id,
                        payload,
                    }
                } else if let Some((wire_seq, unit_id, pfx, body)) = decode_data_frag(&plain) {
                    RxFrame::DataFrag {
                        wire_seq,
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
                if !peers.contains_key(&peer) {
                    if peers.len() >= cfg.max_peers {
                        stats.dropped_new_peers += 1;
                        continue;
                    }
                    let mut rx = Receiver::new();
                    rx.set_quota_r(cfg.quota_r);
                    peers.insert(
                        peer,
                        PeerSlot {
                            rx,
                            last_seen_ns: now,
                            delivered_seen: std::collections::BTreeSet::new(),
                            blocks: BTreeMap::new(),
                            msgs: BTreeMap::new(),
                            seam_peer,
                            tx_pn: 0,
                            param: ParamState::new(),
                            pre_param_fec: Vec::new(),
                        },
                    );
                }
                let Some(slot) = peers.get_mut(&peer) else { continue };
                slot.last_seen_ns = now;
                match parsed {
                    // [S1B] İki DATA çerçevesi TEK kolda işlenir. Fark yalnızca chunk
                    // metadata'sının NEREDEN geldiğidir:
                    //   0x00 DATA      → prefix `unit_id`'den TÜRETİLİR (mesaj = birim),
                    //   0x05 DATA-FRAG → prefix WIRE'DAN gelir (mesaj = ardışık chunk'lar).
                    // Sonrasındaki her şey — motor beslemesi, ACK, symbol ingest, teslim —
                    // ortaktır; iki ayrı kopya bakım-zamanlı ayrışma riskiydi.
                    RxFrame::Data { .. } | RxFrame::DataFrag { .. } => {
                        let (wire_seq, unit_id, pfx, payload, body) = match parsed {
                            RxFrame::Data {
                                wire_seq,
                                unit_id,
                                payload,
                            } => (
                                wire_seq,
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
                                wire_seq,
                                unit_id,
                                pfx,
                                body,
                            } => (
                                wire_seq,
                                unit_id,
                                pfx,
                                &body[SOURCE_PREFIX_LEN..],
                                Some(body),
                            ),
                            _ => unreachable!("dış match yalnız iki DATA varyantını seçti"),
                        };
                        let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                        let full = resolve_seq(expected, wire_seq);
                        let out = slot.rx.on_datagram(
                            RxDatagram {
                                seq: full,
                                unit_id,
                                kind: RxKind::Data,
                            },
                            now,
                        );
                        // İlk-teslim: seq-yeni ise chunk teslim kapısına girer (dedup orada:
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
                            let (sp, pn) = (slot.seam_peer, slot.next_pn());
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, ack.encode())
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
                                let (sp, pn) = (slot.seam_peer, slot.next_pn());
                                srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
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
                            // Motor-ACK'i yine verilir (seq işgal edildi; kayıp-sinyali üretme).
                            let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                            let full = resolve_seq(expected, h.seq);
                            let is_repair = h.kind == inner_kind::FEC_REPAIR;
                            let out = slot.rx.on_datagram(
                                RxDatagram {
                                    seq: full,
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
                                let (sp, pn) = (slot.seam_peer, slot.next_pn());
                                srv_seal_send(
                                    &mut tx, &sock, &mut stats, peer, sp, pn,
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
                        let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                        let full = resolve_seq(expected, h.seq);
                        let is_repair = h.kind == inner_kind::FEC_REPAIR;
                        let out = slot.rx.on_datagram(
                            RxDatagram {
                                seq: full,
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
                            let (sp, pn) = (slot.seam_peer, slot.next_pn());
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, ack.encode())
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
                            let (sp, pn) = (slot.seam_peer, slot.next_pn());
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
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
                        let (sp, pn) = (slot.seam_peer, slot.next_pn());
                        stats.cs_echoes_tx += 1;
                        srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
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
                                let (sp, pn) = (slot.seam_peer, slot.next_pn());
                                srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
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
                                            let (sp2, pn2) = (slot.seam_peer, slot.next_pn());
                                            srv_seal_send(
                                                &mut tx, &sock, &mut stats, peer, sp2, pn2, f2,
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
