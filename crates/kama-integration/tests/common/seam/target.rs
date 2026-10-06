//! Hedef katmanı — sözleşme süitinin "hangi implementasyona karşı koşuyorum" sorusu.
//!
//! Google SWE ch.13'ün contract-test kuralı ("the same tests run against both the real
//! implementation and the fake") derleme seviyesinde ancak iki hedef AYNI trait'i sağlarsa
//! tutar. Bu dosya o hedef soyutlamasını ve bugünkü tek hedefi (sahte) verir; süitin kendisi
//! [`super::contract`] içinde.
//!
//! # Sahte: TEK KOPYA
//!
//! `mock` hedefi `crates/kama-app/tests/common/mock_transport.rs` dosyasındaki `MockTransport`'tur
//! (ported from feature/app — Hayri). Dosya buraya `#[path]` ile alınır; Application'ın kendi
//! testlerinde kullandığı sahte ile bu süitin sınadığı sahte AYNI koddur. İkinci bir kopya olsaydı
//! sözleşme süiti birini sınarken Application diğerine karşı geliştirilirdi.
//!
//! # Bugünkü durum: tek hedef
//!
//! `mock` kayıtlı, `real` DEĞİL — ve bu **beyan edilmiş bir boşluktur** ([`GAP_REAL_ADAPTER`]),
//! sessiz bir atlama değil. `scripts/ci/seam-conformance.sh --target real` bugün çıkış kodu 3
//! döner; `seam_real_adapter` süiti boşluğun BEYAN EDİLDİĞİNİ sınar. Gerçek adapter kaydedildiği
//! an o sınama kırmızıya döner — kapsam sessizce değişemez.

#![allow(dead_code)]

use std::time::Duration;

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{KamaTransport, TransportCapabilities};

/// Sahtenin TEK kopyası Application crate'inin test ağacındadır.
#[path = "../../../../kama-app/tests/common/mock_transport.rs"]
pub mod app_mock;

pub use app_mock::MockTransport;

/// Gerçek adapter'ın yokluğunun kimliği. Runner ve `seam_real_adapter` bu sabiti paylaşır.
pub const GAP_REAL_ADAPTER: &str = "GAP-SEAM-REAL-1";

/// Hedef ailesi. `Mock` = sahte modül (geliştirme sırasında), `Real` = gerçek implementasyon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Mock,
    Real,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Mock => "mock",
            TargetKind::Real => "real",
        }
    }
}

/// Hedef kurulamadı — **typed**, panik değil, `todo!()` değil (CLAUDE.md §6).
///
/// Bu tip bilerek `TransportError` DEĞİLDİR: "implementasyon henüz yok" bir harness durumudur,
/// sözleşmenin §3 hata kümesine ait değildir. İkisini karıştırmak, uygulanmamış bir yolu
/// sözleşmenin bir davranışıymış gibi gösterirdi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TargetUnavailable {
    #[error("hedef uygulanmadı ({gap})")]
    NotImplemented { gap: &'static str },
}

/// Sözleşme süitinin parametresi: hedefi kuran fabrika + senaryo kancaları.
///
/// Kancalar sözleşmenin parçası DEĞİLDİR: satırın ihtiyaç duyduğu DIŞ koşulu (karşı ucun yanıtı,
/// link durumu, saat, kayıp) hedefe özgü biçimde üretirler. Sahte onları doğrudan enjekte eder;
/// gerçek hedef aynı kancaları gerçek bir karşı uç fikstürüyle doldurur. Satır gövdeleri değişmez.
pub trait SeamTarget {
    /// Süitin karşı koştuğu somut tip.
    type Transport: KamaTransport;
    /// Runner ve çıktı satırlarında görünen kimlik (`mock` / `real`).
    const ID: &'static str;
    /// Hedef ailesi — kategori sızıntısı sınamalarında kullanılır.
    const KIND: TargetKind;
    /// Hedefin istemci pending INIT tavanı — ADLANDIRILMIŞ sabitinden (ccr §35.15 KS-DOS-1).
    const PENDING_INIT_CAP: usize;
    /// Hedefin beyan ettiği `TransportCapabilities::reliable` — ADLANDIRILMIŞ sabitinden
    /// (S1 §1 F0.2(iv); H-CAPS `capabilities().reliable`'ı bununla karşılaştırır).
    const RELIABLE: bool;

    /// # Errors
    /// Hedef bu derlemede mevcut değilse [`TargetUnavailable`] (typed; beyan edilmiş boşluk).
    fn build() -> Result<Self::Transport, TargetUnavailable>;

    /// Hedefin iç ilerlemesini bir adım sürer: bekleyen handshake'ler tamamlanır, Transport'un
    /// kurtarabildiği kayıplar teslim edilir.
    ///
    /// **Bu kanca ilk sözleşme satırını (H-ROUNDTRIP) yazarken doğdu:** gerçek adapter socket
    /// okumadan ilerleyemez. Kanca olmasaydı süit sahte-biçimli olurdu ve bunu ancak gerçek hedefi
    /// yazarken, bütün satırları yeniden şekillendirerek öğrenirdik. Bir niyetin BEKLEMEDE kalması
    /// gereken evrede satır bu kancayı ÇAĞIRMAZ (H-CANCEL: iptale kadar · H-SILENT-REJECT: sahte
    /// yanıtlar boyunca · H-PENDING-CAP: tavan ve ret iddiaları boyunca); niyetin çözülmesini
    /// istediği evrede çağırır. Hedef, çağrılmadığı sürece bekleyen niyeti çözmemelidir.
    fn pump(transport: &mut Self::Transport);

    /// Anahtar yenileme gereksinimi doğar (→ `RenewRequired(h)`).
    fn require_renew(transport: &mut Self::Transport, session: SessionHandle);

    /// Yenileme tamamlanır: atomik cutover (→ `Ready(h')`, ardından `Closed(h, Superseded)`).
    fn complete_renew(transport: &mut Self::Transport, old: SessionHandle);

    /// Bilinen eşten ONAYLANMIŞ yeni INIT (peer supersession, S1 K7 / DR-CONNECT-1 H6):
    /// `RenewRequired` OLMADAN `Ready(h')`, ardından `Closed(h, Superseded)`. Onay-öncesi
    /// (provisional) dönem, bayat/duplicate INIT ve glare bu kancanın kapsamı DEĞİLDİR (S3 real
    /// hedefte, L-A5c). [F0.4]
    fn inject_peer_supersession(transport: &mut Self::Transport, old: SessionHandle);

    /// Bekleyen niyete sahte/geçersiz bir RESPONSE ulaşır.
    fn inject_invalid_response(transport: &mut Self::Transport, peer: PeerRef);

    /// Link durumu değişir.
    fn set_link(transport: &mut Self::Transport, up: bool);

    /// Saat ilerler (sanal ya da gerçek).
    fn advance_clock(transport: &mut Self::Transport, by: Duration);

    /// Bugüne kadarki hub yolu prob emisyonu sayısı.
    fn probe_emissions(transport: &Self::Transport) -> u64;

    /// Transport'un kayıp kurtarması açılır/kapatılır.
    fn set_recovery(transport: &mut Self::Transport, on: bool);

    /// Sonraki `n` gönderimin ilk iletimi kaybettirilir.
    fn drop_next_units(transport: &mut Self::Transport, n: usize);
}

/// Kayıtlı hedefler — `seam_real_adapter` bu listeye karşı "gerçek hedef yok" iddiasını sınar.
pub const REGISTERED_TARGETS: &[(&str, TargetKind)] = &[(MockTarget::ID, TargetKind::Mock)];

// ── Sahte hedef ──────────────────────────────────────────────────────────────────────────────

/// Sahtenin taşıdığı kapasite değerleri — **normatif değil**.
///
/// Gerçek değerler K5 zincirinden türeyecek (`Crypto conservative_limits → Transport bütçesi →
/// TransportCapabilities`). Süit bu sayılara BAĞLANMAZ: hedefin kendi `capabilities()`'ini okur.
pub const STUB_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Bkz. [`STUB_MAX_PLAINTEXT_UNIT_BYTES`] — normatif değil.
pub const STUB_PREFERRED_PLAINTEXT_UNIT_BYTES: u32 = 512;
/// `early_unit` tavanı (K6, INIT zarfı) — normatif değil; `preferred`'dan küçük bir yer tutucu,
/// böylece H-CAPS'in `max_early` ve `max_plaintext` sınırları ayrı ayrı ölçülür. [F0.2(iv)]
pub const STUB_MAX_EARLY_UNIT_BYTES: u32 = 256;
/// Sahtenin `reliable` beyanı — normatif değil. Sahtenin "recovery" anahtarı bu beyandan
/// bağımsızdır (sahte alanı yalnız taşır). [F0.2(iv)]
pub const STUB_RELIABLE: bool = true;

/// Sahte hedef kaydı.
pub struct MockTarget;

impl SeamTarget for MockTarget {
    type Transport = MockTransport;
    const ID: &'static str = "mock";
    const KIND: TargetKind = TargetKind::Mock;
    const PENDING_INIT_CAP: usize = app_mock::PENDING_INIT_CAP;
    const RELIABLE: bool = STUB_RELIABLE;

    fn build() -> Result<Self::Transport, TargetUnavailable> {
        Ok(MockTransport::new(TransportCapabilities {
            max_plaintext_unit_bytes: STUB_MAX_PLAINTEXT_UNIT_BYTES,
            preferred_plaintext_unit_bytes: STUB_PREFERRED_PLAINTEXT_UNIT_BYTES,
            max_early_unit_bytes: STUB_MAX_EARLY_UNIT_BYTES,
            reliable: STUB_RELIABLE,
        }))
    }

    fn pump(transport: &mut Self::Transport) {
        transport.pump();
    }

    fn require_renew(transport: &mut Self::Transport, session: SessionHandle) {
        transport.request_renew(session);
    }

    fn complete_renew(transport: &mut Self::Transport, old: SessionHandle) {
        transport.complete_renew(old);
    }

    fn inject_peer_supersession(transport: &mut Self::Transport, old: SessionHandle) {
        transport.inject_peer_supersession(old);
    }

    fn inject_invalid_response(transport: &mut Self::Transport, peer: PeerRef) {
        transport.inject_invalid_response(&peer);
    }

    fn set_link(transport: &mut Self::Transport, up: bool) {
        transport.set_link(up);
    }

    fn advance_clock(transport: &mut Self::Transport, by: Duration) {
        transport.advance_clock(by);
    }

    fn probe_emissions(transport: &Self::Transport) -> u64 {
        transport.stats().probes
    }

    fn set_recovery(transport: &mut Self::Transport, on: bool) {
        transport.set_recovery(on);
    }

    fn drop_next_units(transport: &mut Self::Transport, n: usize) {
        transport.drop_next_units(n);
    }
}
