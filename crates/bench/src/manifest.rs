//! [FAZ-4] KOŞU MANİFESTİ — yapılandırılmış (JSON), ölçümden ÖNCE ve SONRA.
//!
//! Denetim bulgusu: bir kıyas koşusunun NE ölçtüğü serbest-metin yorumlardan çıkarılıyordu ve
//! "sevk profili" etiketli kol ürünün sevk etmediği bir yapılandırmayı ölçmüştü. Manifest bunu
//! yapısal olarak imkânsız kılar: kimlik, yapılandırma, anlam ve fiilen ateşleyen mekanizmalar
//! makine-okunur biçimde koşunun yanında durur.
//!
//! İki bölüm:
//! * `config` — ölçümden ÖNCE (kimlik/profil/yük/anlam/bozulma/kurulum sınıfı)
//! * `activation` — ölçümden SONRA (hangi mekanizma fiilen ateşledi; Faz-3 tanılarından)

use serde::{Deserialize, Serialize};

/// Manifest şema kimliği — analizörler bunu kontrol eder.
pub const MANIFEST_SCHEMA: &str = "kama-bench-manifest/v1";

/// Uygulama-düzeyi anlam sözleşmesi. Kollar arası kıyas ancak bu alanlar eşleşiyorsa "adil"dir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Semantics {
    /// `reliable-messages` · `reliable-ordered-stream` · `unreliable-datagram` · `datagram+app-retry`
    pub reliability: String,
    /// `none` (mesaj granülü) · `total` (byte akışı)
    pub ordering: String,
    /// Teslim olayının TANIMI — kollar arası AYNI katman olmalı.
    pub delivery_definition: String,
    /// Mükerrer teslim politikası.
    pub duplicate_handling: String,
    /// Yeniden deneme sahibi: `transport` · `application` · `none`
    pub retry_owner: String,
}

/// Yol bozulması — ileri ve dönüş AYRI kaydedilir (asimetri birincil hedef).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Impairment {
    pub forward: String,
    pub reverse: String,
}

/// Ateşleme özeti (Faz-3 tanılarından; yalnız Kama kollarında dolu).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationSummary {
    pub tail_configured: bool,
    pub fec_configured: bool,
    pub tail_probes: u64,
    pub tail_cancelled: u64,
    pub parity_emitted: u64,
    pub parity_token_denied: u64,
    pub retransmits: u64,
    pub dups: u64,
    pub state_transitions: u64,
    pub recover_arq_entries: u64,
    pub left_preconverge_ns: u64,
    pub btlbw_bps: u64,
    pub srtt_us: u64,
    pub rtprop_us: u64,
    pub return_ack_loss_permille: u32,
    pub return_forward_loss_permille: u32,
    pub bytes_data: u64,
    pub bytes_retx: u64,
    pub bytes_parity: u64,
}

/// Bir koşunun tam kimliği ve sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema: String,
    pub commit: String,
    /// `kama-v1-full` · `kama-custom` · `quinn-reliable-stream` · `quinn-datagram-app-retry` · `tcp-default`
    pub arm: String,
    pub transport: String,
    /// Ürün profilinden sapmalar (Kama kolları; boş = kanonik profil).
    pub profile_delta: Vec<String>,
    pub semantics: Semantics,
    /// TÜRETİLMİŞ (operatör iddiası değil): `cold-flow` (kurulum ölçüm penceresi İÇİNDE) ·
    /// `setup-excluded` (kurulum ayrı ölçülür, `setup_ns`; pencere DIŞINDA). Kıyas ancak aynı
    /// sınıftaki kollar arasında geçerlidir — `fairness_diff` farkı yakalar.
    pub setup_class: String,
    pub setup_ns: u64,
    pub impairment: Impairment,
    pub payload_bytes: u64,
    pub msg_count: u64,
    pub deadline_ns: u64,
    pub submit_interval_ns: u64,
    pub seed: u64,
    pub repeat: u32,
    /// Tel muhasebesinin bilinen sınırı (ör. TCP çekirdek ek yükü görünmez).
    pub wire_accounting_note: String,
    #[serde(default)]
    pub activation: ActivationSummary,
}

/// [FAZ-4 §16/§17] Kıyas denetim sonucu — ÜÇ DÜZEY, karıştırılmaz.
///
/// | düzey | anlamı | çıkış kodu |
/// |---|---|---|
/// | `blocking` boş değil | deney koşulu farklı → kıyas sayısı ÜRETİLEMEZ | 2 |
/// | `non_normalized` boş değil | anlam farklı → sayı üretilebilir ama etiket `non-normalized` | 1 |
/// | ikisi de boş | tam normalize kıyas | 0 |
///
/// §17'nin "Kama reliable ↔ Quinn unreliable DATAGRAM" örneği `non_normalized` düzeyine düşer:
/// reddedilmez ama "adil kıyas" etiketini ALAMAZ.
#[derive(Debug, Clone, Default)]
pub struct FairnessReport {
    /// Deney koşulu farkları — kıyası GEÇERSİZ kılar.
    pub blocking: Vec<String>,
    /// Anlam farkları — kıyas yapılabilir ama `non-normalized` işaretlenir (§16).
    pub non_normalized: Vec<String>,
}

impl FairnessReport {
    /// Deney koşulları eşleşiyor mu (sayı üretilebilir mi)?
    #[must_use]
    pub fn is_comparable(&self) -> bool {
        self.blocking.is_empty()
    }

    /// TAM normalize mi (anlam farkı da yok)? Yalnız bu durumda "adil kıyas" denebilir.
    #[must_use]
    pub fn is_normalized(&self) -> bool {
        self.blocking.is_empty() && self.non_normalized.is_empty()
    }
}

impl RunManifest {
    /// Kanonik Kama profili mi (sapma yok)?
    #[must_use]
    pub fn is_canonical_kama(&self) -> bool {
        self.arm == client_core::runtime::TRANSPORT_V1_PROFILE_ID && self.profile_delta.is_empty()
    }

    /// [FAZ-4 §17] KIYAS DENETİMİ — İKİ AYRI SINIF, karıştırılmaz.
    ///
    /// * `blocking` — **DENEY KOŞULU** farkları (yük, mesaj sayısı, deadline, sunum aralığı,
    ///   bozulma, kurulum sınıfı). Bunlar eşit olmadan hiçbir kıyas sayısı anlamlı değildir.
    /// * `non_normalized` — **ANLAM** farkları (güvenilirlik, sıra, teslim tanımı, retry sahibi).
    ///   Bunlar kıyası GEÇERSİZ KILMAZ ama "adil kıyas" etiketini KALDIRIR: sonuç yalnız
    ///   `comparison=non-normalized` damgasıyla yayımlanabilir (§16). Anlamları eşitlemek
    ///   (TCP'ye fazladan iş yüklemek / Kama'ya sıra eklemek) YASAKTIR — fark kıyasın konusudur.
    ///
    /// İlk yazımda ikisi tek listede toplanmıştı → §4.4'ün ana kıyası (Kama ↔ quinn) her zaman
    /// "GEÇERSİZ" çıkıyordu. Bu, aracı gürültüye çevirir; ayrım DÜZELTİLDİ.
    #[must_use]
    pub fn fairness_diff(&self, other: &Self) -> FairnessReport {
        let mut blocking = Vec::new();
        let mut non_normalized = Vec::new();

        // ── ANLAM (beyan edilir, engellemez) ────────────────────────────────────────────
        if self.semantics.reliability != other.semantics.reliability {
            non_normalized.push(format!(
                "reliability {} != {}",
                self.semantics.reliability, other.semantics.reliability
            ));
        }
        if self.semantics.ordering != other.semantics.ordering {
            non_normalized.push(format!(
                "ordering {} != {}",
                self.semantics.ordering, other.semantics.ordering
            ));
        }
        if self.semantics.delivery_definition != other.semantics.delivery_definition {
            non_normalized.push(format!(
                "delivery_definition \"{}\" != \"{}\"",
                self.semantics.delivery_definition, other.semantics.delivery_definition
            ));
        }
        if self.semantics.retry_owner != other.semantics.retry_owner {
            non_normalized.push(format!(
                "retry_owner {} != {}",
                self.semantics.retry_owner, other.semantics.retry_owner
            ));
        }
        if self.wire_accounting_note != other.wire_accounting_note {
            non_normalized
                .push("wire_accounting sınırı farkı — byte kıyası DOĞRUDAN yapılamaz".into());
        }

        // ── DENEY KOŞULU (engeller) ─────────────────────────────────────────────────────
        if self.setup_class != other.setup_class {
            blocking.push(format!(
                "setup_class {} != {}",
                self.setup_class, other.setup_class
            ));
        }
        if self.payload_bytes != other.payload_bytes {
            blocking.push(format!(
                "payload {} != {}",
                self.payload_bytes, other.payload_bytes
            ));
        }
        if self.msg_count != other.msg_count {
            blocking.push(format!(
                "msg_count {} != {}",
                self.msg_count, other.msg_count
            ));
        }
        if self.deadline_ns != other.deadline_ns {
            blocking.push(format!(
                "deadline {} != {} ns",
                self.deadline_ns, other.deadline_ns
            ));
        }
        if self.submit_interval_ns != other.submit_interval_ns {
            blocking.push(format!(
                "submit_interval {} != {} ns",
                self.submit_interval_ns, other.submit_interval_ns
            ));
        }
        if self.impairment.forward != other.impairment.forward {
            blocking.push(format!(
                "forward impairment \"{}\" != \"{}\"",
                self.impairment.forward, other.impairment.forward
            ));
        }
        if self.impairment.reverse != other.impairment.reverse {
            blocking.push(format!(
                "reverse impairment \"{}\" != \"{}\"",
                self.impairment.reverse, other.impairment.reverse
            ));
        }
        FairnessReport {
            blocking,
            non_normalized,
        }
    }
}

/// Manifesti diske yaz (JSON). Hata sessizce yutulmaz.
pub fn write(path: &std::path::Path, m: &RunManifest) {
    match serde_json::to_string_pretty(m) {
        Ok(js) => {
            if let Err(e) = std::fs::write(path, js) {
                eprintln!("KP-MANIFEST-WRITE-FAILED {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("KP-MANIFEST-SERIALIZE-FAILED: {e}"),
    }
}

/// [FAZ-4 §15] KOL ADI POLİTİKASI — belirsiz `Quinn` / `Kama` etiketi YASAK. Rakip kolları
/// hangi QUIC primitifini test ettiklerini adlarında TAŞIR; Kama kolunun adı profil kimliğinden
/// (`kama-v1-full` / `kama-custom`) gelir, bu yüzden burada yer almaz.
#[must_use]
pub fn arm_name(transport: &str) -> &str {
    match transport {
        "quinn" => "quinn-reliable-stream",
        "qdgram" => "quinn-datagram-app-retry",
        "tcp-default" => "tcp-default",
        "kama" => "kama-stub-legacy",
        other => other,
    }
}

/// Kolun anlam sözleşmesi — TEK kaynak (rapor ve doğrulama buradan okur).
#[must_use]
pub fn semantics_for(transport: &str) -> Semantics {
    match transport {
        "tcp-default" => Semantics {
            reliability: "reliable-ordered-stream".into(),
            ordering: "total".into(),
            delivery_definition: "app-level ACK of framed message reaches sender".into(),
            duplicate_handling: "kernel-dedup (stream)".into(),
            retry_owner: "transport".into(),
        },
        "quinn" => Semantics {
            reliability: "reliable-ordered-stream".into(),
            ordering: "total".into(),
            delivery_definition: "stream write acknowledged by peer".into(),
            duplicate_handling: "transport-dedup".into(),
            retry_owner: "transport".into(),
        },
        "qdgram" => Semantics {
            reliability: "datagram+app-retry".into(),
            ordering: "none".into(),
            delivery_definition: "first app-level ACK covering the message".into(),
            duplicate_handling: "app first-ack-wins".into(),
            retry_owner: "application".into(),
        },
        _ => Semantics {
            reliability: "reliable-messages".into(),
            ordering: "none".into(),
            delivery_definition: "first delivery confirmation (ACK or FEC-RECOVERED)".into(),
            duplicate_handling: "transport first-delivery-wins".into(),
            retry_owner: "transport".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(arm: &str, transport: &str) -> RunManifest {
        RunManifest {
            schema: MANIFEST_SCHEMA.to_string(),
            commit: "test".into(),
            arm: arm.into(),
            transport: transport.into(),
            profile_delta: Vec::new(),
            semantics: semantics_for(transport),
            setup_class: "setup-excluded".into(),
            setup_ns: 0,
            impairment: Impairment {
                forward: "delay 100ms loss 20%".into(),
                reverse: "delay 100ms".into(),
            },
            payload_bytes: 256,
            msg_count: 300,
            deadline_ns: 20_000_000_000,
            submit_interval_ns: 20_000_000,
            seed: 7,
            repeat: 1,
            wire_accounting_note: "full".into(),
            activation: ActivationSummary::default(),
        }
    }

    /// [M0] Kanonik profil `kama-v1-full`; TEK alan sapması `kama-custom` yapar.
    #[test]
    fn m0_profil_kimligi_deterministik() {
        let canon = base("kama-v1-full", "kama-product");
        assert!(canon.is_canonical_kama());
        let mut custom = canon.clone();
        custom.arm = "kama-custom".into();
        custom.profile_delta = vec!["initial_window_bytes 0 != 2400".into()];
        assert!(
            !custom.is_canonical_kama(),
            "sapma varken kanonik SAYILAMAZ"
        );
        // Etiket doğru ama sapma listesi doluysa da kanonik DEĞİLDİR (çift koruma).
        let mut lying = canon.clone();
        lying.profile_delta = vec!["fec None != Some((5, None))".into()];
        assert!(
            !lying.is_canonical_kama(),
            "sapma listesi boş değilse yalan söyleyemez"
        );
    }

    /// [FAZ-4 §17] İKİ SINIF AYRIMI: anlam farkı BEYAN edilir (engellemez), deney-koşulu farkı
    /// ENGELLER. Bu ayrım olmadan §4.4'ün ana kıyası (Kama ↔ quinn) her zaman "geçersiz" çıkardı.
    #[test]
    fn anlam_farki_beyan_kosul_farki_engel() {
        let kama = base("kama-v1-full", "kama-product");

        // (a) Kama güvenilir-mesaj ↔ qdgram: ANLAM farkı → beyan, ama kıyas YAPILABİLİR.
        let dgram = base("qdgram", "qdgram");
        let r = kama.fairness_diff(&dgram);
        assert!(
            r.non_normalized.iter().any(|x| x.contains("reliability")),
            "güvenilirlik farkı BEYAN edilmeli: {r:?}"
        );
        assert!(
            r.is_comparable(),
            "anlam farkı kıyası ENGELLEMEZ (deney koşulu aynı): {r:?}"
        );
        assert!(
            !r.is_normalized(),
            "ama TAM NORMALİZE de DEĞİLDİR — `adil kıyas` etiketi alamaz: {r:?}"
        );

        // (b) Kurulum sınıfı farkı: DENEY KOŞULU → engeller.
        let mut cold = base("tcp-default", "tcp-default");
        cold.setup_class = "cold-flow".into();
        let rc = kama.fairness_diff(&cold);
        assert!(
            rc.blocking.iter().any(|x| x.contains("setup_class")),
            "kurulum sınıfı farkı ENGELLEYİCİ olmalı: {rc:?}"
        );
        assert!(!rc.is_comparable());

        // (c) Dönüş yolu bozulması farkı: asimetri birincil hedef → ileri/dönüş AYRI denetlenir.
        let mut other_net = base("tcp-default", "tcp-default");
        other_net.impairment.reverse = "delay 100ms loss 40%".into();
        let rn = kama.fairness_diff(&other_net);
        assert!(
            rn.blocking.iter().any(|x| x.contains("reverse impairment")),
            "dönüş yolu farkı ENGELLEYİCİ olmalı: {rn:?}"
        );
        assert!(
            !rn.blocking.iter().any(|x| x.contains("forward impairment")),
            "ileri yol AYNI iken forward farkı raporlanmamalı: {rn:?}"
        );

        // (d) Yük farkı: engeller.
        let mut big = base("kama-v1-full", "kama-product");
        big.payload_bytes = 1024;
        assert!(!kama.fairness_diff(&big).is_comparable());
    }

    /// Aynı kolun iki koşusu: ne engelleyici ne beyan farkı (yanlış alarm üretmez).
    #[test]
    fn ayni_kol_iki_kosu_farksiz() {
        let a = base("kama-v1-full", "kama-product");
        let b = base("kama-v1-full", "kama-product");
        let r = a.fairness_diff(&b);
        assert!(r.is_normalized(), "{r:?}");
    }

    /// [FAZ-4 §15] Belirsiz kol adı YASAK: `quinn` iki farklı primitife karşılık gelir.
    #[test]
    fn kol_adlari_belirsiz_degil() {
        assert_eq!(arm_name("quinn"), "quinn-reliable-stream");
        assert_eq!(arm_name("qdgram"), "quinn-datagram-app-retry");
        assert_ne!(
            arm_name("quinn"),
            arm_name("qdgram"),
            "iki QUIC primitifi AYNI ada sahip olamaz"
        );
        assert_eq!(arm_name("tcp-default"), "tcp-default");
        // Kama kolunun adı profil kimliğinden gelir; taşıyıcı adı kullanılmaz.
        assert_eq!(arm_name("kama-product"), "kama-product");
    }

    /// Tel-muhasebesi sınırı farklıysa BEYAN edilir — byte kıyası doğrudan yapılamaz.
    #[test]
    fn tel_muhasebesi_farki_beyan_edilir() {
        let kama = base("kama-v1-full", "kama-product");
        let mut tcp = base("tcp-default", "tcp-default");
        tcp.wire_accounting_note = "app-layer bytes only; kernel TCP overhead invisible".into();
        let r = kama.fairness_diff(&tcp);
        assert!(
            r.non_normalized
                .iter()
                .any(|x| x.contains("wire_accounting")),
            "byte muhasebesi sınırı beyan edilmeli: {r:?}"
        );
    }

    /// TCP tel muhasebesinin sınırı manifeste YAZILIR (gizli varsayım bırakılmaz).
    #[test]
    fn tcp_tel_muhasebesi_siniri_beyan_edilir() {
        let s = semantics_for("tcp-default");
        assert_eq!(s.ordering, "total", "TCP byte akışı = tam sıralı");
        assert_eq!(s.reliability, "reliable-ordered-stream");
        assert!(s.delivery_definition.contains("app-level ACK"));
    }
}
