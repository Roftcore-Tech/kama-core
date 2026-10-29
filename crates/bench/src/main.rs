//! Kama transport benchmark harness CLI (§4.4). SoC: `serve`/`client` HAM event üretir;
//! `metrics` HESAP yapar (transport'a dokunmaz). quinn baseline (§4.4.1) + kama stub.

mod budget;
mod error;
mod event;
mod kama;
mod kama_product;
mod manifest;
mod metrics;
mod mixed;
mod outcome;
mod qdgram;
mod quic;
mod rival;
mod rxlog;
mod tcp;
mod transport;

use clap::{Parser, Subcommand, ValueEnum};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use client_core::runtime::KamaV1Profile;

use crate::error::BenchError;
use crate::transport::{BenchTransport, Cc, ClientCfg, FecArm, FecModeArg, KamaArm, ServerCfg};

#[derive(Parser)]
#[command(name = "bench", about = "Kama transport benchmark harness (§4.4)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

// [FAZ-4] CLI komut enum'u süreç başına BİR kez parse edilir; kutulamak ölçüme hiçbir şey
// katmaz, yalnız arg tanımlarını dolaylı hale getirir. Boyut farkı kabul edilir.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Cmd {
    /// Transport server'ı çalıştır.
    Serve {
        #[arg(long, default_value = "[::]:4433")]
        bind: SocketAddr,
        #[arg(long, value_enum, default_value_t = CcArg::Cubic)]
        cc: CcArg,
        #[arg(long, value_enum, default_value_t = TransportArg::Quinn)]
        transport: TransportArg,
        /// [TUR-5 r3] Epizot-kotası R [CAL {4,8}] (kama receiver).
        #[arg(long, default_value_t = 4)]
        quota_r: u32,
        /// [KAPI-1 W2] Reverse-yön burst hard-cap (IP byte; 0 = yok). Paced ile birlikte VERİLMEZ.
        #[arg(long, default_value_t = 0)]
        budget_burst_bytes: u64,
        /// [KAPI-1 W2] Reverse-yön paced refill (IP byte/s; 0 = paced yok).
        #[arg(long, default_value_t = 0)]
        budget_refill_bytes_per_s: u64,
        /// [KAPI-1 W2] Paced kova (IP byte; refill>0 iken ZORUNLU).
        #[arg(long, default_value_t = 0)]
        budget_bucket_bytes: u64,
        /// [KAPI-1 W2] Paced kümülatif tavan (IP byte; 0 = tavansız).
        #[arg(long, default_value_t = 0)]
        budget_ceiling_bytes: u64,
        /// [KAPI-1 W6] Reverse-bütçe arm senkronu: `<path>.go` belirene dek arm etme.
        #[arg(long)]
        gate_watch: Option<PathBuf>,
        /// [SPRINT-A] FEC profil kimliği (0 = FEC kapalı). `kama-product` dışında yok sayılır.
        /// İSTEMCİYLE AYNI verilmelidir. Örn. 3 = K8/R4/E64 (payload ≤ 53 B).
        // [FAZ-4 §1.1] Bayraksız değer ÜRÜN PROFİLİNE BAĞLI. Literal `5` yazmak, profil
        // sabiti değiştiğinde resmî kıyas yolunu sessizce ürün-dışı bırakırdı.
        #[arg(long, default_value_t = KamaV1Profile::FEC_PROFILE_ID)]
        fec_profile: u8,
        /// [SPRINT-A] Blok başına sabit repair sayısı. `--fec-mode static` ile birlikte
        /// **r=0 GEÇERLİDİR** ve tek-eksenli kontrol kolunu verir (fragmentasyon açık, parity yok).
        #[arg(long, default_value_t = 0)]
        fec_r: u16,
        /// [KAMPANYA-KRİTİK] FEC modu: `static` (r kadar repair, r=0 = kontrol) | `adaptive`
        /// (Controller). `r`'den ORTOGONAL — eski `r==0 ⇒ adaptif` kısayolu statik-r0 kolunu
        /// temsil edilemez kılıyordu.
        #[arg(long, value_enum, default_value_t = FecModeCli::Adaptive)]
        fec_mode: FecModeCli,
        /// [DEADLINE-EKSENİ] `--fec-mode policy` kolunda her offer'la bildirilen teslim bütçesi
        /// (ms). Kalibre politika kararını bu bütçenin srtt'ye ORANINDAN verir; bildirilmezse
        /// (0) `NoDeadlineDeclared` ile ArqOnly'ye düşer — sessiz varsayılan YOK.
        #[arg(long, default_value_t = 0)]
        fec_deadline_ms: u64,
        /// [ONAY UFKU] Kayıp-defteri `Suspected → Confirmed` ufku: `0` = sevk davranışı
        /// (cadence-legacy, ölçüldü ki HİÇ ateşlemiyor), `>0` = kanal-türevli `k × srtt`
        /// per-mille (`2000` = 2×srtt). ÖLÇÜM KOLU.
        #[arg(long, default_value_t = 0)]
        loss_confirm_k: u32,
        /// [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi (Suspected olaylara yaş-ağırlıklı
        /// güven). KAPALI = sevk davranışı. TEK tüketici FEC oranı kararıdır.
        #[arg(long, default_value_t = false)]
        fec_risk: bool,
        /// [HANDOFF ENSTRÜMANI] Zaman-serisi CSV çıktısı (client): t_ms,instant,persistent,rp,
        /// confirmed,cprop,sprop,specprop,final,reason,level. 50 ms örnekleme, ≤4096 satır.
        #[arg(long)]
        ts_out: Option<std::path::PathBuf>,
        /// [R-SWEEP] Spekülatif L1 tavanı (‰; K=8'de 125=r1, 250=r2, 375=r3). None = default 125.
        #[arg(long)]
        spec_l1_permille: Option<u32>,
        /// [R-SWEEP] Spekülatif L2 tavanı (‰). None = default 250.
        #[arg(long)]
        spec_l2_permille: Option<u32>,
        /// [R-SWEEP] Yapı-gate seviyesi (‰; 500 = sevk r4). None = motor default (fec_med).
        #[arg(long)]
        gate_permille: Option<u32>,
        /// [PERSIST] Ufuk modu: fixed (sevk 0.5×srtt) | learned (max(α·srtt, ρ·Q_q(spurious yaşı))).
        #[arg(long)]
        persist_mode: Option<String>,
        /// [PERSIST] Taban α (‰ srtt; fixed modda eşiğin kendisi). None = 500.
        #[arg(long)]
        persist_alpha: Option<u32>,
        /// [PERSIST] ρ (‰). None = 1250.
        #[arg(long)]
        persist_rho: Option<u32>,
        /// [PERSIST] quantile (‰). None = 950.
        #[arg(long)]
        persist_q: Option<u32>,
        /// [C4 G1] reorder-baskısı eşiği (‰): rp ≥ N ⇒ spekülatif öneri yok. None = kapalı.
        #[arg(long)]
        rp_suppress: Option<u32>,
        /// [SPRINT-A] Alıcı birim-payload tavanı — client `--payload` ile AYNI olmalı.
        #[arg(long, default_value_t = 256)]
        payload: u64,
        /// [BURST-FUSION] Yapı-gate'in burst dalında da kodlama açmasına izin ver (opt-in;
        /// KALİBRE EDİLMEMİŞ). Verilmezse sevk davranışı (burst → parity yok). İKİ UÇTA AYNI.
        #[arg(long, default_value_t = false)]
        fec_burst_fusion: bool,
        /// [DR-CS-3] Cold-start bootstrap-RTT merdivenini aç (opt-in). İKİ UÇTA AYNI.
        #[arg(long, default_value_t = false)]
        bootstrap_rtt: bool,
        /// [IW] PreConverge ilk-uçuş penceresi (byte; 0 = sevk davranışı, tek paket).
        #[arg(long, default_value_t = 0)]
        initial_window_bytes: u64,
        /// [KURULUŞ] Bootstrap merdivenini kuruluş-hızlı yap (0.2/0.6/1.8/5.4 s). Bütçe değişmez.
        #[arg(long, default_value_t = false)]
        bootstrap_fast: bool,
        /// [KTP2-F0 / D] Rakip kolların (quinn, tcp) teslim tanımı: `app-reply` (eski; uygulama
        /// yanıtı) | `transport-ack` (eşin taşıma ACK'i; uygulama yanıtı YOK). İSTEMCİYLE AYNI.
        #[arg(long, value_enum, default_value_t = rival::Delivery::AppReply)]
        rival_delivery: rival::Delivery,
        /// [KTP2-F0 / D] quinn ilk RTT (ms; 0 = quinn varsayılanı 333 ms). İki uçta aynı.
        #[arg(long, default_value_t = 0)]
        quinn_initial_rtt_ms: u64,
        /// [KTP2-F0 / D] quinn CC ilk penceresi (bayt; 0 = CC varsayılanı: cubic 12000,
        /// bbr 240000).
        #[arg(long, default_value_t = 0)]
        quinn_iw_bytes: u64,
        /// [KTP2-F0 / D] quinn keep-alive aralığı (ms; 0 = yok).
        #[arg(long, default_value_t = 0)]
        quinn_keepalive_ms: u64,
        /// [KTP2-F0 / C1·D2] Alıcı-tarafı teslim damgası (JSONL, şema `ktp2-c-rx-v1`;
        /// `kama-product`, `tcp`, `quinn`). Verilmezse kayıt yok, dosya yok. Sunucu çıkınca
        /// (SIGTERM dahil) atomik yazılır. quinn'de kimlik için istemci `--payload-id` verir.
        #[arg(long)]
        rx_log: Option<PathBuf>,
    },
    /// Client: N mesaj gönder, ham event JSONL yaz.
    Client {
        #[arg(long)]
        server: SocketAddr,
        #[arg(long, default_value_t = 100)]
        msgs: u64,
        #[arg(long, default_value_t = 256)]
        payload: u64,
        #[arg(long, default_value_t = 120)]
        max_deadline_s: u64,
        #[arg(long, value_enum, default_value_t = CcArg::Cubic)]
        cc: CcArg,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[arg(long, default_value_t = 0)]
        repeat: u32,
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, value_enum, default_value_t = TransportArg::Quinn)]
        transport: TransportArg,
        /// Kama kolu: arq (defaults, tail OFF) | tail-v1 (KAMA-TAIL-V1). quinn'de yok sayılır. [KAPI-1 W4]
        #[arg(long, value_enum, default_value_t = KamaArmArg::Arq)]
        kama_arm: KamaArmArg,
        /// Mesaj sunum aralığı ms (0 = burst; >0 = paced, mesaj i t=i×interval'de). [KAPI-1 W3]
        #[arg(long, default_value_t = 0)]
        submit_interval_ms: u64,
        /// [KAPI-1 W2] Forward-yön burst hard-cap (IP byte; 0 = yok). Paced ile birlikte VERİLMEZ.
        #[arg(long, default_value_t = 0)]
        budget_burst_bytes: u64,
        /// [KAPI-1 W2] Forward-yön paced refill (IP byte/s; 0 = paced yok).
        #[arg(long, default_value_t = 0)]
        budget_refill_bytes_per_s: u64,
        /// [KAPI-1 W2] Paced kova (IP byte; refill>0 iken ZORUNLU).
        #[arg(long, default_value_t = 0)]
        budget_bucket_bytes: u64,
        /// [KAPI-1 W2] Paced kümülatif tavan (IP byte; 0 = tavansız).
        #[arg(long, default_value_t = 0)]
        budget_ceiling_bytes: u64,
        /// [KAPI-1 W6] Ölçüm-öncesi warmup mesajı (0 = yok; ölçüme/bütçeye girmez).
        #[arg(long, default_value_t = 0)]
        warmup_msgs: u64,
        /// [KAPI-1 W6] Barrier gate yolu (`<gate>.ready` yazılır, `<gate>.go` beklenir).
        #[arg(long)]
        gate: Option<PathBuf>,
        /// [KAPI-1 W6] Kuruluş-gate'i (bağlantı kurulunca ready; netem uygulanınca go → warmup).
        #[arg(long)]
        establish_gate: Option<PathBuf>,
        /// [SPRINT-A] FEC profil kimliği (0 = FEC kapalı). `kama-product` dışında yok sayılır.
        /// SUNUCUYLA AYNI verilmelidir. Örn. 3 = K8/R4/E64 (payload ≤ 53 B).
        // [FAZ-4 §1.1] Bayraksız değer ÜRÜN PROFİLİNE BAĞLI. Literal `5` yazmak, profil
        // sabiti değiştiğinde resmî kıyas yolunu sessizce ürün-dışı bırakırdı.
        #[arg(long, default_value_t = KamaV1Profile::FEC_PROFILE_ID)]
        fec_profile: u8,
        /// [SPRINT-A] Blok başına sabit repair sayısı. `--fec-mode static` ile birlikte
        /// **r=0 GEÇERLİDİR** ve tek-eksenli kontrol kolunu verir (fragmentasyon açık, parity yok).
        #[arg(long, default_value_t = 0)]
        fec_r: u16,
        /// [KAMPANYA-KRİTİK] FEC modu: `static` (r kadar repair, r=0 = kontrol) | `adaptive`
        /// (Controller). `r`'den ORTOGONAL — eski `r==0 ⇒ adaptif` kısayolu statik-r0 kolunu
        /// temsil edilemez kılıyordu.
        #[arg(long, value_enum, default_value_t = FecModeCli::Adaptive)]
        fec_mode: FecModeCli,
        /// [DEADLINE-EKSENİ] `--fec-mode policy` kolunda her offer'la bildirilen teslim bütçesi
        /// (ms). Kalibre politika kararını bu bütçenin srtt'ye ORANINDAN verir; bildirilmezse
        /// (0) `NoDeadlineDeclared` ile ArqOnly'ye düşer — sessiz varsayılan YOK.
        #[arg(long, default_value_t = 0)]
        fec_deadline_ms: u64,
        /// [ONAY UFKU] Kayıp-defteri `Suspected → Confirmed` ufku: `0` = sevk davranışı
        /// (cadence-legacy, ölçüldü ki HİÇ ateşlemiyor), `>0` = kanal-türevli `k × srtt`
        /// per-mille (`2000` = 2×srtt). ÖLÇÜM KOLU.
        #[arg(long, default_value_t = 0)]
        loss_confirm_k: u32,
        /// [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi (Suspected olaylara yaş-ağırlıklı
        /// güven). KAPALI = sevk davranışı. TEK tüketici FEC oranı kararıdır.
        #[arg(long, default_value_t = false)]
        fec_risk: bool,
        /// [HANDOFF ENSTRÜMANI] Zaman-serisi CSV çıktısı (client): t_ms,instant,persistent,rp,
        /// confirmed,cprop,sprop,specprop,final,reason,level. 50 ms örnekleme, ≤4096 satır.
        #[arg(long)]
        ts_out: Option<std::path::PathBuf>,
        /// [R-SWEEP] Spekülatif L1 tavanı (‰; K=8'de 125=r1, 250=r2, 375=r3). None = default 125.
        #[arg(long)]
        spec_l1_permille: Option<u32>,
        /// [R-SWEEP] Spekülatif L2 tavanı (‰). None = default 250.
        #[arg(long)]
        spec_l2_permille: Option<u32>,
        /// [R-SWEEP] Yapı-gate seviyesi (‰; 500 = sevk r4). None = motor default (fec_med).
        #[arg(long)]
        gate_permille: Option<u32>,
        /// [PERSIST] Ufuk modu: fixed (sevk 0.5×srtt) | learned (max(α·srtt, ρ·Q_q(spurious yaşı))).
        #[arg(long)]
        persist_mode: Option<String>,
        /// [PERSIST] Taban α (‰ srtt; fixed modda eşiğin kendisi). None = 500.
        #[arg(long)]
        persist_alpha: Option<u32>,
        /// [PERSIST] ρ (‰). None = 1250.
        #[arg(long)]
        persist_rho: Option<u32>,
        /// [PERSIST] quantile (‰). None = 950.
        #[arg(long)]
        persist_q: Option<u32>,
        /// [FAZ-4] Koşu manifestini (JSON) bu yola yaz: kimlik + profil sapması + anlam +
        /// bozulma + kurulum sınıfı + ateşleme özeti. Kıyas raporları BUNU okur.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// [FAZ-4] İleri yol bozulması (netem dizesi) — manifeste AYNEN kaydedilir.
        #[arg(long, default_value = "")]
        fwd_impair: String,
        /// [FAZ-4] Dönüş yolu bozulması (netem dizesi) — AYRI kaydedilir (asimetri hedefi).
        #[arg(long, default_value = "")]
        rev_impair: String,
        /// [FAZ-4 §M4] Kurulum-sınıfı İDDİASI (opsiyonel): `cold-flow` | `setup-excluded`.
        /// Bu bayrak ölçümü DEĞİŞTİRMEZ; harness'ın kendi yapısal gerçeğiyle KARŞILAŞTIRILIR.
        /// Çelişirse koşu manifest yazmadan hata verir → sessiz kurulum-sınıfı karışımı imkânsız.
        #[arg(long)]
        setup_class: Option<String>,
        /// [C4 G1] reorder-baskısı eşiği (‰): rp ≥ N ⇒ spekülatif öneri yok. None = kapalı.
        #[arg(long)]
        rp_suppress: Option<u32>,
        /// [BURST-FUSION] Bkz. `serve --fec-burst-fusion`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, default_value_t = false)]
        fec_burst_fusion: bool,
        /// [DR-CS-3] Bkz. `serve --bootstrap-rtt`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, default_value_t = false)]
        bootstrap_rtt: bool,
        /// [IW] PreConverge ilk-uçuş penceresi (byte; 0 = sevk davranışı, tek paket).
        #[arg(long, default_value_t = 0)]
        initial_window_bytes: u64,
        /// [KURULUŞ] Bkz. `serve --bootstrap-fast`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, default_value_t = false)]
        bootstrap_fast: bool,
        /// [KTP2-F0 / D] Bkz. `serve --rival-delivery`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, value_enum, default_value_t = rival::Delivery::AppReply)]
        rival_delivery: rival::Delivery,
        /// [KTP2-F0 / D] El sıkışma / bağlanma tavanı (s; 0 = eski: quinn `--max-deadline-s`,
        /// tcp 20 s). Kurulum teslim penceresinin DIŞINDADIR, `setup_ns` olarak ayrı raporlanır.
        #[arg(long, default_value_t = 0)]
        handshake_timeout_s: u64,
        /// [KTP2-F0 / D] Bkz. `serve --quinn-initial-rtt-ms`.
        #[arg(long, default_value_t = 0)]
        quinn_initial_rtt_ms: u64,
        /// [KTP2-F0 / D] Bkz. `serve --quinn-iw-bytes`.
        #[arg(long, default_value_t = 0)]
        quinn_iw_bytes: u64,
        /// [KTP2-F0 / D] Bkz. `serve --quinn-keepalive-ms`.
        #[arg(long, default_value_t = 0)]
        quinn_keepalive_ms: u64,
        /// [KTP2-F0 / D] Koşu sonunda `QUINN-STATS` satırı (c.err; salt-gözlem).
        #[arg(long, default_value_t = false)]
        rival_stats: bool,
        /// [KTP2-F0 / D1] tcp (`app-reply`): bütün ACK'ler gelince ya da pencere dolunca bitir
        /// (eski: sabit `min(max_deadline, 60 s)` bekleme). Teslim olayları değişmez.
        #[arg(long, default_value_t = false)]
        early_exit: bool,
        /// [KTP2-F0 / D2] quinn: mesaj kimliği yükün ilk 8 baytına (BE) yazılır — sunucunun
        /// `--rx-log` kaydı mesajı tanır. Yük boyu ve zamanlama aynı.
        #[arg(long, default_value_t = false)]
        payload_id: bool,
        /// [KTP2-F0 / C2] Yalnız `kama-product`: her ölçüm mesajı N ayrı birim olarak sunulur
        /// (gerçek tekrar kolu); teslim = ilk kopyanın teyidi. 1 = eski davranış.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=64))]
        dup_copies: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// [FAZ-4 §17] İki veya daha fazla koşu manifestini KIYAS-ADALETİ açısından denetle.
    /// Fark bulunursa exit=2 → kıyas raporu `non-normalized` işaretlenmeden yayımlanamaz.
    /// Bu komut PERFORMANS SAYISI ÜRETMEZ; yalnız kıyasın geçerli olup olmadığını söyler.
    Fairness {
        #[arg(long, num_args = 2.., required = true)]
        manifests: Vec<PathBuf>,
    },
    /// Ham JSONL koşularından metrik hesapla (A: varyans, B: deadline eğrisi).
    Metrics {
        #[arg(long, num_args = 1.., required = true)]
        runs: Vec<PathBuf>,
        #[arg(long, value_delimiter = ',', default_value = "10,30,120")]
        deadlines_s: Vec<u64>,
        /// [FAZ-4 §7] Kurulum maliyeti görünümü. `steady-state` = harness'ın ölçtüğü
        /// (kurulum pencere dışında); `cold-flow` = uçtan-uca (deadline'dan `setup_ns` düşülür).
        /// İKİ GÖRÜNÜM KARIŞTIRILMAZ — her çıktı hangisinde olduğunu yazar.
        #[arg(long, value_enum, default_value_t = SetupViewArg::SteadyState)]
        setup_view: SetupViewArg,
        /// [FAZ-4 §8] Blackout tarama penceresi (ms).
        #[arg(long, default_value_t = 1000)]
        blackout_window_ms: u64,
        /// [FAZ-4 §8] Pencerenin kullanılabilir sayılması için asgari teslim.
        #[arg(long, default_value_t = 1)]
        blackout_min_deliveries: u64,
        /// [FAZ-4 §12.3] Güven düzeyi (‰).
        #[arg(long, default_value_t = outcome::DEFAULT_CI_PERMILLE)]
        ci_permille: u32,
    },
    /// [KTP2-F0 / D] Karışık akış (dosya + TEXT + PTT) sunucusu: alım günlüğü yazar.
    MixedServe(mixed::ServeArgs),
    /// [KTP2-F0 / D] Karışık akış istemcisi: sunum planı + taşıma sayaçları (taşıma düzeyi).
    MixedClient(mixed::ClientArgs),
}

/// CLI karşılığı — `transport::FecModeArg`'a birebir eşlenir.
#[derive(Copy, Clone, ValueEnum, PartialEq)]
enum FecModeCli {
    Static,
    Adaptive,
    /// [DEADLINE-EKSENİ] Kalibre füzyon politikası karar verir; `--fec-deadline-ms` zorunlu.
    Policy,
}

impl From<FecModeCli> for FecModeArg {
    fn from(m: FecModeCli) -> Self {
        match m {
            FecModeCli::Static => FecModeArg::Static,
            FecModeCli::Adaptive => FecModeArg::Adaptive,
            FecModeCli::Policy => FecModeArg::Policy,
        }
    }
}

/// [FAZ-4 §7] CLI karşılığı — `outcome::SetupView`'a birebir eşlenir.
#[derive(Copy, Clone, ValueEnum, PartialEq)]
enum SetupViewArg {
    SteadyState,
    ColdFlow,
}

impl From<SetupViewArg> for outcome::SetupView {
    fn from(v: SetupViewArg) -> Self {
        match v {
            SetupViewArg::SteadyState => Self::SteadyState,
            SetupViewArg::ColdFlow => Self::ColdFlow,
        }
    }
}

#[derive(Copy, Clone, ValueEnum)]
enum CcArg {
    Cubic,
    Bbr,
}

impl From<CcArg> for Cc {
    fn from(c: CcArg) -> Cc {
        match c {
            CcArg::Cubic => Cc::Cubic,
            CcArg::Bbr => Cc::Bbr,
        }
    }
}

#[derive(Copy, Clone, ValueEnum, PartialEq)]
enum TransportArg {
    Quinn,
    /// Security-stub sürücüsü (FEC parity YAPISAL olarak emit edilemez — bkz. src/kama.rs).
    Kama,
    /// [KAPI-1 W1] quinn DATAGRAM + app-ARQ (Q-DGRAM-ARQ, FAZ-1 primary comparator).
    Qdgram,
    /// [FAZ-4] TCP taban kolu (`tcp-default`): platformun normal TCP yığını, uygulama-düzeyi
    /// ACK ile teslim ölçümü, minimum simetrik çerçeveleme. Denetim: TCP kolu YOKTU.
    Tcp,
    /// [SPRINT-A] GERÇEK ÜRÜN runtime'ı (client-core): FEC+ARQ füzyonu, tail-recovery,
    /// fuzzy kontrolcü, PARAM bootstrap, handshake-siz kuruluş.
    KamaProduct,
}

/// [KAPI-1 W4] CLI karşılığı — `transport::KamaArm`'a birebir eşlenir.
#[derive(Copy, Clone, ValueEnum)]
enum KamaArmArg {
    Arq,
    TailV1,
}

/// [KAPI-1 W2] CLI bütçe argları → `BudgetMode` (doğrulamalı: burst XOR paced; sessiz yanlış
/// kombinasyon YOK — kampanya-config hatası koşudan önce patlar).
fn budget_mode(
    burst: u64,
    refill: u64,
    bucket: u64,
    ceiling: u64,
) -> Result<budget::BudgetMode, BenchError> {
    match (burst, refill) {
        (0, 0) if bucket == 0 && ceiling == 0 => Ok(budget::BudgetMode::Unlimited),
        (0, 0) => Err(BenchError::Other(
            "bucket/ceiling yalnız --budget-refill-bytes-per-s ile anlamlı".into(),
        )),
        (b, 0) => {
            if bucket != 0 || ceiling != 0 {
                return Err(BenchError::Other(
                    "burst ve paced bütçe birlikte verilemez (r5 D.1 workload-ayrık)".into(),
                ));
            }
            Ok(budget::BudgetMode::Burst { cap_bytes: b })
        }
        (0, r) => {
            if bucket == 0 {
                return Err(BenchError::Other(
                    "paced bütçe --budget-bucket-bytes ister (D6: 2 s × refill)".into(),
                ));
            }
            Ok(budget::BudgetMode::Paced {
                refill_bytes_per_s: r,
                bucket_bytes: bucket,
                ceiling_bytes: if ceiling == 0 { u64::MAX } else { ceiling },
            })
        }
        _ => Err(BenchError::Other(
            "burst ve paced bütçe birlikte verilemez (r5 D.1 workload-ayrık)".into(),
        )),
    }
}

impl From<KamaArmArg> for KamaArm {
    fn from(a: KamaArmArg) -> KamaArm {
        match a {
            KamaArmArg::Arq => KamaArm::Arq,
            KamaArmArg::TailV1 => KamaArm::TailV1,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), BenchError> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve {
            bind,
            cc,
            transport,
            quota_r,
            budget_burst_bytes,
            budget_refill_bytes_per_s,
            budget_bucket_bytes,
            budget_ceiling_bytes,
            gate_watch,
            fec_profile,
            fec_r,
            fec_mode,
            fec_deadline_ms,
            loss_confirm_k,
            fec_risk,
            ts_out: _,
            spec_l1_permille,
            spec_l2_permille,
            gate_permille,
            persist_mode,
            persist_alpha,
            persist_rho,
            persist_q,
            rp_suppress,
            payload,
            fec_burst_fusion,
            bootstrap_rtt,
            initial_window_bytes,
            bootstrap_fast,
            rival_delivery,
            quinn_initial_rtt_ms,
            quinn_iw_bytes,
            quinn_keepalive_ms,
            rx_log,
        } => {
            // [KTP2-F0 / C1] Desteklenmeyen kolda sessizce kayıtsız koşmak yerine reddet.
            let rx_supported = matches!(
                transport,
                TransportArg::KamaProduct | TransportArg::Tcp | TransportArg::Quinn
            );
            if rx_log.is_some() && !rx_supported {
                return Err(BenchError::Other(
                    "--rx-log yalnız --transport kama-product | tcp | quinn ile".into(),
                ));
            }
            // [KTP2-F0 / D] Rakip-kol ayarları (bayraksız = eski davranış; Kama kollarına etkisiz).
            rival::set(rival::RivalTuning::from_cli(
                rival_delivery,
                0,
                quinn_initial_rtt_ms,
                quinn_iw_bytes,
                quinn_keepalive_ms,
                false,
            ));
            run_server(
                transport,
                ServerCfg {
                    bind,
                    cc: cc.into(),
                    quota_r,
                    budget: budget_mode(
                        budget_burst_bytes,
                        budget_refill_bytes_per_s,
                        budget_bucket_bytes,
                        budget_ceiling_bytes,
                    )?,
                    gate_watch,
                    fec: FecArm::from_args(fec_profile, fec_r, fec_mode.into(), fec_deadline_ms),
                    loss_confirm_k,
                    fec_risk,
                    ts_out: None,
                    gate_permille,
                    persist_mode,
                    persist_alpha,
                    persist_rho,
                    persist_q,
                    rp_suppress,
                    spec_l2_permille,
                    spec_l1_permille,
                    payload_bytes: payload,
                    burst_fusion: fec_burst_fusion,
                    bootstrap_rtt,
                    initial_window_bytes,
                    bootstrap_fast,
                    rx_log,
                },
            )
            .await
        }
        Cmd::Client {
            manifest,
            fwd_impair,
            rev_impair,
            setup_class,
            server,
            msgs,
            payload,
            max_deadline_s,
            cc,
            seed,
            repeat,
            profile,
            transport,
            kama_arm,
            submit_interval_ms,
            budget_burst_bytes,
            budget_refill_bytes_per_s,
            budget_bucket_bytes,
            budget_ceiling_bytes,
            warmup_msgs,
            gate,
            establish_gate,
            fec_profile,
            fec_r,
            fec_mode,
            fec_deadline_ms,
            loss_confirm_k,
            fec_risk,
            ts_out,
            spec_l1_permille,
            spec_l2_permille,
            gate_permille,
            persist_mode,
            persist_alpha,
            persist_rho,
            persist_q,
            rp_suppress,
            fec_burst_fusion,
            bootstrap_rtt,
            initial_window_bytes,
            bootstrap_fast,
            rival_delivery,
            handshake_timeout_s,
            quinn_initial_rtt_ms,
            quinn_iw_bytes,
            quinn_keepalive_ms,
            rival_stats,
            early_exit,
            payload_id,
            dup_copies,
            out,
        } => {
            // [KTP2-F0 / C2] Yalnız ürün kolunda anlamlı; diğer kollarda sessizce yok sayılmaz.
            if dup_copies > 1 && transport != TransportArg::KamaProduct {
                return Err(BenchError::Other(
                    "--dup-copies yalnız --transport kama-product ile".into(),
                ));
            }
            // [KTP2-F0 / D] Rakip-kol ayarları (bayraksız = eski davranış; Kama kollarına etkisiz).
            let mut tuning = rival::RivalTuning::from_cli(
                rival_delivery,
                handshake_timeout_s,
                quinn_initial_rtt_ms,
                quinn_iw_bytes,
                quinn_keepalive_ms,
                rival_stats,
            );
            tuning.early_exit = early_exit;
            tuning.payload_id = payload_id;
            rival::set(tuning);
            let cfg = ClientCfg {
                server,
                msg_count: msgs,
                payload_bytes: payload,
                max_deadline: Duration::from_secs(max_deadline_s),
                cc: cc.into(),
                seed,
                repeat,
                profile,
                kama_arm: kama_arm.into(),
                submit_interval: Duration::from_millis(submit_interval_ms),
                budget: budget_mode(
                    budget_burst_bytes,
                    budget_refill_bytes_per_s,
                    budget_bucket_bytes,
                    budget_ceiling_bytes,
                )?,
                warmup_msgs,
                gate,
                establish_gate,
                fec: FecArm::from_args(fec_profile, fec_r, fec_mode.into(), fec_deadline_ms),
                loss_confirm_k,
                fec_risk,
                ts_out,
                gate_permille,
                persist_mode,
                persist_alpha,
                persist_rho,
                persist_q,
                rp_suppress,
                spec_l2_permille,
                spec_l1_permille,
                burst_fusion: fec_burst_fusion,
                bootstrap_rtt,
                initial_window_bytes,
                bootstrap_fast,
                dup_copies,
            };
            let output = run_client(transport, cfg).await?;
            // [FAZ-4] KOŞU MANİFESTİ — ölçümle BİRLİKTE, yapılandırılmış. Kıyas raporları
            // profili/anlamı/bozulmayı buradan okur; serbest-metin yorumlara GÜVENİLMEZ.
            // [FAZ-4 §M4] KURULUM SINIFI KOLUN KODUNDAN TÜRETİLİR. Operatörün `--setup-class`
            // iddiası yalnız DENETLENİR; ölçümü etkilemez. Dört kolun tamamında kurulum maliyeti
            // completion penceresinin DIŞINDA ölçülür ve `setup_ns` olarak ayrıca raporlanır.
            let derived_setup_class = outcome::derived_setup_class(output.meta.setup_in_window);
            outcome::check_setup_claim(setup_class.as_deref(), derived_setup_class).map_err(
                |e| {
                    BenchError::Other(format!(
                        "{e} (setup_ns={}). Manifest YAZILMADI.",
                        output.meta.setup_ns
                    ))
                },
            )?;
            if let Some(mp) = manifest.as_ref() {
                let tr = output.meta.transport.clone();
                // [FAZ-4 §1.3] Kimlik kolun KENDİ türetiminden gelir (RunMeta.profile_id).
                // Boşsa (rakip kolları) taşıyıcı adı kullanılır.
                // [§15] Boşsa (rakip kolları) BELİRSİZ taşıyıcı adı DEĞİL, hangi primitifin
                // test edildiğini söyleyen açık kol adı kullanılır.
                let arm = if output.meta.profile_id.is_empty() {
                    manifest::arm_name(&tr).to_string()
                } else {
                    output.meta.profile_id.clone()
                };
                let m = manifest::RunManifest {
                    schema: manifest::MANIFEST_SCHEMA.to_string(),
                    commit: option_env!("KAMA_GIT_SHA").unwrap_or("unknown").to_string(),
                    arm,
                    transport: tr.clone(),
                    profile_delta: output.meta.profile_delta.clone(),
                    semantics: manifest::semantics_for(&tr),
                    setup_class: derived_setup_class.to_string(),
                    setup_ns: output.meta.setup_ns,
                    impairment: manifest::Impairment {
                        forward: fwd_impair.clone(),
                        reverse: rev_impair.clone(),
                    },
                    payload_bytes: output.meta.payload_bytes,
                    msg_count: output.meta.msg_count,
                    deadline_ns: output.meta.max_deadline_ns,
                    submit_interval_ns: output.meta.submit_interval_ns,
                    seed: output.meta.seed,
                    repeat: output.meta.repeat,
                    // [KTP2-F0 / D] `tcp-tack` da çekirdek ek yükünü göremez (aynı sınır).
                    wire_accounting_note: if tr.starts_with("tcp") {
                        "app-layer bytes only; kernel TCP overhead invisible".to_string()
                    } else {
                        "full harness-observed wire bytes".to_string()
                    },
                    activation: output.meta.activation.clone().unwrap_or_default(),
                };
                manifest::write(mp, &m);
            }
            output.write_jsonl(&out)?;
            eprintln!("wrote {} events → {}", output.events.len(), out.display());
            Ok(())
        }
        Cmd::Fairness { manifests } => {
            let mut loaded: Vec<(PathBuf, manifest::RunManifest)> = Vec::new();
            for p in &manifests {
                let txt = std::fs::read_to_string(p)
                    .map_err(|e| BenchError::Other(format!("{}: {e}", p.display())))?;
                let m: manifest::RunManifest = serde_json::from_str(&txt)
                    .map_err(|e| BenchError::Other(format!("{}: {e}", p.display())))?;
                if m.schema != manifest::MANIFEST_SCHEMA {
                    return Err(BenchError::Other(format!(
                        "{}: bilinmeyen şema {}",
                        p.display(),
                        m.schema
                    )));
                }
                loaded.push((p.clone(), m));
            }
            let mut invalid = 0usize;
            let mut non_normalized = 0usize;
            println!("# kıyas-adaleti denetimi — {} manifest", loaded.len());
            // Aynı adlı manifestler farklı dizinlerde durur (netem rejim başına bir dizin).
            // Yalnız dosya adı yazmak üç satırı ayırt edilemez kılardı.
            let short = |p: &PathBuf| -> String {
                let f = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                p.parent()
                    .and_then(|d| d.file_name())
                    .map_or(f.clone(), |d| format!("{}/{f}", d.to_string_lossy()))
            };
            for (p, m) in &loaded {
                // [FAZ-4 §1.3] Kanonik iddiası ile sapma listesi ÇELİŞEMEZ.
                let canon = m.is_canonical_kama();
                // [K4G-05b] Kanonik etiket tek kaynaktan (v1.1: `kama-v1.1-full`; elle yazılmaz).
                let canon_id = client_core::runtime::TRANSPORT_V1_PROFILE_ID;
                if m.arm == canon_id && !canon {
                    println!("INVALID {}: {canon_id} etiketi ama sapma var", short(p));
                    invalid += 1;
                }
                println!(
                    "{:<28} arm={} canonical_kama={} setup={} delta={}",
                    short(p),
                    m.arm,
                    canon,
                    m.setup_class,
                    m.profile_delta.len()
                );
            }
            for i in 0..loaded.len() {
                for j in (i + 1)..loaded.len() {
                    let (pa, a) = &loaded[i];
                    let (pb, b) = &loaded[j];
                    let (na, nb) = (short(pa), short(pb));
                    let r = a.fairness_diff(b);
                    if !r.is_comparable() {
                        invalid += 1;
                        println!("BLOCKING       {na} vs {nb}: {}", r.blocking.join(" | "));
                    } else if r.is_normalized() {
                        println!("NORMALIZED     {na} ~ {nb}");
                    } else {
                        non_normalized += 1;
                        println!("NON-NORMALIZED {na} ~ {nb} — `adil kıyas` etiketi ALAMAZ");
                    }
                    // Anlam farkı RAPORDA aynen beyan edilmek ZORUNDADIR (§16).
                    for note in &r.non_normalized {
                        println!("  semantics    {na} vs {nb}: {note}");
                    }
                }
            }
            if invalid > 0 {
                eprintln!(
                    "DENEY KOŞULU EŞLEŞMİYOR ({invalid} bulgu) — bu manifestlerden kıyas sayısı \
                     ÜRETİLEMEZ"
                );
                std::process::exit(2);
            }
            if non_normalized > 0 {
                // §16: sayı üretilebilir, ama etiket `non-normalized` olmak ZORUNDA.
                println!(
                    "comparison=non-normalized ({non_normalized} çift) — deney koşulları \
                     eşleşiyor, ANLAMLAR eşleşmiyor; `semantics` satırları rapora AYNEN girer"
                );
                std::process::exit(1);
            }
            println!("comparison=normalized — koşul ve anlam eşleşiyor");
            Ok(())
        }
        Cmd::Metrics {
            runs,
            deadlines_s,
            setup_view,
            blackout_window_ms,
            blackout_min_deliveries,
            ci_permille,
        } => {
            let deadlines: Vec<Duration> = deadlines_s
                .iter()
                .map(|&s| Duration::from_secs(s))
                .collect();
            // Eksik/bozuk koşu dosyasında ABORT etme (workflow bulgusu #4: harsh profilde bir
            // handshake fail'i dosya yazmaz → tüm agregasyon ölmemeli). Atla + uyar.
            // [FAZ-4 §12.4] Okunamayan koşu SESSİZCE SİLİNMEZ. Dosya hiç yoksa altyapı
            // (runner/netns) arızası; dosya var ama çözülemiyorsa harness arızası. İkisi de
            // transport ölçümü DEĞİLDİR — agregadan dışlanır ama insidansı raporlanır.
            let mut lost: Vec<outcome::Outcome> = Vec::new();
            let outputs: Vec<_> = runs
                .iter()
                .filter_map(|p| match event::RunOutput::read_jsonl(p) {
                    Ok(o) => Some(o),
                    Err(e) => {
                        let cls = if p.exists() {
                            outcome::Outcome::HarnessFailure
                        } else {
                            outcome::Outcome::InfraFailure
                        };
                        eprintln!("skip {} [{}]: {e}", p.display(), cls.label());
                        lost.push(cls);
                        None
                    }
                })
                .collect();
            // [FAZ-4 §7] Görünüm ÖNCE seçilir; completion eğrisi de bu görünümde hesaplanır.
            // Eğriyi kararlı-durumda bırakıp yalnız alt bloğu uçtan-uca hesaplamak, tam da
            // yasaklanan "iki görünümü karıştırma" olurdu.
            let view: outcome::SetupView = setup_view.into();
            let per_run: Vec<_> = outputs
                .iter()
                .map(|o| {
                    let eff: Vec<Duration> = deadlines
                        .iter()
                        .map(|&d| view.effective_deadline(d, o.meta.setup_ns))
                        .collect();
                    // Etiketler nominal deadline'da kalır; ÖLÇÜM efektif deadline'dadır.
                    let mut m = metrics::run_metrics(o, &eff);
                    for (slot, &d) in m.per_deadline.iter_mut().zip(deadlines.iter()) {
                        slot.deadline = d;
                    }
                    m
                })
                .collect();
            let agg = metrics::aggregate(&per_run, &deadlines);
            println!(
                "# completion vs deadline — {} koşu (A: varyans, B: eğri) — görünüm: {}",
                per_run.len(),
                view.label()
            );
            for a in &agg {
                println!(
                    "deadline={:>4}s  median={:.3}  IQR=[{:.3},{:.3}]  min={:.3}  max={:.3}  (n={})",
                    a.deadline.as_secs(),
                    a.completion_median,
                    a.completion_q1,
                    a.completion_q3,
                    a.completion_min,
                    a.completion_max,
                    a.runs,
                );
            }
            let sec = metrics::aggregate_secondary(&per_run);
            println!(
                "# secondary (§4.4.3): bytes/delivered-msg median={:.1}  ACK-overhead median={:.3}",
                sec.bytes_per_delivered_msg_median, sec.ack_overhead_ratio_median,
            );

            // ── [FAZ-4 §9] TEL MALİYETİ DÖKÜMÜ — kalem kalem, kolun BEYAN EDİLEN sınırıyla ──
            // Kama'nın parity/kontrol trafiğini sayıp rakibin transport ek yükünü dışlayarak
            // "toplam maliyet" demek YASAK. TCP'de çekirdek trafiği görünmez; satır bunu YAZAR.
            for o in &outputs {
                let m = &o.meta;
                let payload = m.msg_count.saturating_mul(m.payload_bytes);
                let visibility = if m.transport.starts_with("tcp") {
                    "UYGULAMA-KATMANI (çekirdek TCP ek yükü GÖRÜNMEZ)"
                } else {
                    "tam harness-gözlemli tel"
                };
                println!(
                    "# wire (§9) {}: payload={payload}B tx={}B rx={}B \
                     [data={}B retx={}B dup={}B fec={}B ctrl={}B] setup={}ns({}) — {visibility}",
                    if m.profile_id.is_empty() {
                        manifest::arm_name(&m.transport).to_string()
                    } else {
                        m.profile_id.clone()
                    },
                    m.udp_tx_bytes,
                    m.udp_rx_bytes,
                    m.tx.data,
                    m.tx.retx,
                    m.tx.dup,
                    m.tx.fec,
                    m.tx.ctrl,
                    m.setup_ns,
                    if m.setup_in_window {
                        "pencere İÇİNDE"
                    } else {
                        "pencere DIŞINDA"
                    },
                );
            }

            // ── [FAZ-4 §8/§12/§13] SONUÇ SINIFI · BLACKOUT · KATMANLI İSTATİSTİK ──────────
            // Bu blok PERFORMANS YORUMU YAPMAZ. Koşunun ne olduğunu sınıflandırır ve hangi
            // iddia sınıfının mümkün olduğunu söyler.
            let bspec = outcome::BlackoutSpec {
                window: Duration::from_millis(blackout_window_ms.max(1)),
                min_deliveries: blackout_min_deliveries,
            };
            let claim_deadline = deadlines
                .last()
                .copied()
                .unwrap_or(Duration::from_secs(120));
            println!(
                "# setup view: {} | blackout: window={}ms min_deliveries={} | CI={}‰",
                view.label(),
                blackout_window_ms,
                blackout_min_deliveries,
                ci_permille
            );
            // Kayıp koşular sınıflandırılmış olarak agregaya GİRER (ölçüm sayılmazlar ama
            // paydadan sessizce düşmezler).
            let mut classified: Vec<(f64, outcome::Outcome)> =
                lost.iter().map(|&c| (0.0, c)).collect();
            let mut blackout_runs = 0usize;
            let mut worst_blackout: Option<outcome::BlackoutReport> = None;
            for o in &outputs {
                let b = outcome::blackout_scan(o, &bspec);
                // [§7] Uçtan-uca görünümde deadline kolun KENDİ ölçülen kurulum maliyeti kadar
                // kısalır — her kol kendi maliyetini öder, simetrik.
                let eff = view.effective_deadline(claim_deadline, o.meta.setup_ns);
                let cls = outcome::classify(o, eff, &b);
                if b.any {
                    blackout_runs += 1;
                    if worst_blackout.is_none_or(|w| b.longest_ns > w.longest_ns) {
                        worst_blackout = Some(b);
                    }
                }
                classified.push((metrics::completion(&o.events, eff), cls));
            }
            let mut by_class: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for (_, c) in &classified {
                *by_class.entry(c.label()).or_default() += 1;
            }
            let classes = by_class
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("# outcomes (§13): {classes}");
            // [§8] İkili cevap + en kötü koşunun süresi ve kurtarma süresi. Kurtarma hiç
            // gelmediyse `n/a` yazılır — 0 DEĞİL (0, "anında kurtardı" demek olurdu).
            match worst_blackout {
                None => println!("# blackout (§8): false — {}/{} koşu", 0, outputs.len()),
                Some(w) => println!(
                    "# blackout (§8): true — {blackout_runs}/{} koşu | en uzun={:.3}s \
                     toplam={:.3}s kurtarma={}",
                    outputs.len(),
                    w.longest_ns as f64 / 1e9,
                    w.total_ns as f64 / 1e9,
                    w.time_to_recovery_ns.map_or_else(
                        || "n/a (dönmedi)".to_string(),
                        |r| format!("{:.3}s", r as f64 / 1e9)
                    )
                ),
            }
            let st = outcome::stratify(&classified, ci_permille);
            let fmt = |s: &outcome::Stats| {
                let ci = s.median_ci.map_or_else(
                    || "CI=n/a(n<6)".to_string(),
                    |(l, h)| format!("CI=[{l:.3},{h:.3}]"),
                );
                format!(
                    "n={} median={:.3} IQR=[{:.3},{:.3}] {ci}",
                    s.n, s.median, s.q1, s.q3
                )
            };
            println!("# all-runs    (§12.4): {}", fmt(&st.all_runs));
            println!("# stall-free  (§12.4): {}", fmt(&st.stall_free));
            println!(
                "# stall incidence: {}/{} CI=[{:.3},{:.3}] | ölçüm-dışı (harness/infra): {}",
                st.stall_incidence.0,
                st.stall_incidence.1,
                st.stall_ci.0,
                st.stall_ci.1,
                st.non_measurement
            );
            // [§12.2] İddia sınıfı n'den TÜRETİLİR; operatör yükseltemez.
            println!("# claim class (§12.2): {}", st.all_runs.claim.label());
            Ok(())
        }
        Cmd::MixedServe(a) => mixed::serve(a).await,
        Cmd::MixedClient(a) => mixed::client(a).await,
    }
}

async fn run_server(t: TransportArg, cfg: ServerCfg) -> Result<(), BenchError> {
    match t {
        TransportArg::Quinn => quic::QuinnTransport.run_server(cfg).await,
        TransportArg::Kama => transport::KamaTransport.run_server(cfg).await,
        TransportArg::Qdgram => qdgram::QdgramTransport.run_server(cfg).await,
        TransportArg::Tcp => tcp::TcpTransport.run_server(cfg).await,
        TransportArg::KamaProduct => kama_product::KamaProductTransport.run_server(cfg).await,
    }
}

async fn run_client(t: TransportArg, cfg: ClientCfg) -> Result<event::RunOutput, BenchError> {
    match t {
        TransportArg::Quinn => quic::QuinnTransport.run_client(cfg).await,
        TransportArg::Kama => transport::KamaTransport.run_client(cfg).await,
        TransportArg::Qdgram => qdgram::QdgramTransport.run_client(cfg).await,
        TransportArg::Tcp => tcp::TcpTransport.run_client(cfg).await,
        TransportArg::KamaProduct => kama_product::KamaProductTransport.run_client(cfg).await,
    }
}
