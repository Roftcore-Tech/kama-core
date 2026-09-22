//! Hedef katmanı — sözleşme süitinin "hangi implementasyona karşı koşuyorum" sorusu.
//!
//! Google SWE ch.13'ün contract-test kuralı ("the same tests run against both the real
//! implementation and the fake") derleme seviyesinde ancak iki hedef AYNI trait'i sağlarsa
//! tutar. Bu dosya o hedef soyutlamasını verir; süitin kendisi [`super::contract`] içinde.
//!
//! # Bugünkü durum: tek hedef
//!
//! `mock` kayıtlı, `real` DEĞİL — ve bu **beyan edilmiş bir boşluktur** ([`GAP_REAL_ADAPTER`]),
//! sessiz bir atlama değil. `scripts/ci/seam-conformance.sh --target real` bugün çıkış kodu 3
//! ("beyan edilmiş boşluk") döner; `seam_real_adapter` süiti boşluğun BEYAN EDİLDİĞİNİ sınar.
//! Gerçek adapter kaydedildiği an o sınama kırmızıya döner — kapsam sessizce değişemez.

#![allow(dead_code)]

use super::api::{
    ConnectResult, Delivered, DeliveryPolicy, KamaTransport, PeerRef, PreparedUnit, SessionEvent,
    SessionHandle, TransportCapabilities, TransportError,
};

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
/// Bu tip bilerek [`TransportError`] DEĞİLDİR: "implementasyon henüz yok" bir harness durumudur,
/// sözleşmenin §3 hata kümesine ait değildir. İkisini karıştırmak, uygulanmamış bir yolu
/// sözleşmenin bir davranışıymış gibi gösterirdi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TargetUnavailable {
    #[error("hedef uygulanmadı ({gap})")]
    NotImplemented { gap: &'static str },
}

/// Sözleşme süitinin parametresi: hedefi kuran fabrika.
pub trait SeamTarget {
    /// Süitin karşı koştuğu somut tip.
    type Transport: KamaTransport;
    /// Runner ve çıktı satırlarında görünen kimlik (`mock` / `real`).
    const ID: &'static str;
    /// Hedef ailesi — kategori sızıntısı sınamalarında kullanılır.
    const KIND: TargetKind;

    /// # Errors
    /// Hedef bu derlemede mevcut değilse [`TargetUnavailable`] (typed; beyan edilmiş boşluk).
    fn build() -> Result<Self::Transport, TargetUnavailable>;
}

/// Kayıtlı hedefler — `seam_real_adapter` bu listeye karşı "gerçek hedef yok" iddiasını sınar.
pub const REGISTERED_TARGETS: &[(&str, TargetKind)] = &[(MockTarget::ID, TargetKind::Mock)];

// ── Sahte hedef (iskelet) ────────────────────────────────────────────────────────────────────

/// KM-SEAM-AT-001 §7'nin `MockTransport`'u DEĞİLDİR.
///
/// §7'deki sahte modül deterministik kayıp/sırasız/duplicate enjeksiyonu, `Ready/Renew/Closed`
/// senaryoları ve `ConnectFailed` enjeksiyonu ister; o ayrı bir teslimattır ve sahibi §7'de
/// yazılıdır. Buradaki tip yalnız **iskeletin kendisini** koşturulabilir kılar: her metot
/// TYPED bir cevap döndürür, hiçbiri panik etmez, hiçbiri oturum icat etmez.
///
/// `connect`/`send` → [`TransportError::NotConnected`]: bu bir kaçamak değil, doğru okuma —
/// null hedefte kurulu oturum yoktur. Sözleşme davranışı İDDİA EDİLMEZ; hangi §6 satırının
/// uygulandığı `scripts/ci/seam-coverage.json` içinde beyan edilir (bugün: hiçbiri).
pub struct NullMockTransport {
    caps: TransportCapabilities,
}

/// Null hedefin taşıdığı kapasite değerleri — **normatif değil**.
///
/// Gerçek değerler K5 zincirinden türeyecek (`Crypto conservative_limits → Transport bütçesi →
/// TransportCapabilities`). Buradaki sayılar yalnız `capabilities()`'in bir şey döndürmesi için
/// vardır; H-CAPS satırı uygulanana kadar hiçbir testin bu sayılara bağlanmaması gerekir.
pub const STUB_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Bkz. [`STUB_MAX_PLAINTEXT_UNIT_BYTES`] — normatif değil.
pub const STUB_PREFERRED_PLAINTEXT_UNIT_BYTES: u32 = 512;

impl NullMockTransport {
    pub fn new(caps: TransportCapabilities) -> Self {
        Self { caps }
    }
}

impl KamaTransport for NullMockTransport {
    fn capabilities(&self) -> TransportCapabilities {
        self.caps
    }

    fn connect(
        &mut self,
        _peer: PeerRef,
        _early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError> {
        Err(TransportError::NotConnected)
    }

    fn send(
        &mut self,
        _session: SessionHandle,
        _unit: PreparedUnit,
        _policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        Err(TransportError::NotConnected)
    }

    fn recv(&mut self) -> Option<Delivered> {
        None
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        None
    }

    fn close(&mut self, _session: SessionHandle) {}
}

/// Sahte hedef kaydı.
pub struct MockTarget;

impl SeamTarget for MockTarget {
    type Transport = NullMockTransport;
    const ID: &'static str = "mock";
    const KIND: TargetKind = TargetKind::Mock;

    fn build() -> Result<Self::Transport, TargetUnavailable> {
        Ok(NullMockTransport::new(TransportCapabilities {
            max_plaintext_unit_bytes: STUB_MAX_PLAINTEXT_UNIT_BYTES,
            preferred_plaintext_unit_bytes: STUB_PREFERRED_PLAINTEXT_UNIT_BYTES,
        }))
    }
}
