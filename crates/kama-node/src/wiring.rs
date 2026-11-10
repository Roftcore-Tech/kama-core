//! Kurma modülü — bileşim kökünün Crypto katmanına (`kama-noise-wire`) dokunduğu TEK yer.
//!
//! `scripts/ci/layer_check.py` C8: `kama-node`'un ürün kaynağında (`src/**`) `kama_noise_wire`
//! adı YALNIZ bu dosyada geçebilir. Burada yalnız YAPICI düzeyi vardır: ürün kurulumu
//! (`NoiseWireProtection::with_control`), onun girdileri ve tip adları. Çalışma zamanında düğüm
//! Crypto'nun hiçbir KOMUTUNU çağırmaz (`SessionControl` / `PacketProtection` Transport'undur;
//! INV-NEIGHBOR-1), yalnız Application motorlarını sürer. Tek temas salt-okunurdur: her turun ölçü
//! anlık görüntüsü eş başına oturum epoch'unu Transport üzerinden okur
//! (`ClientTransport::session_epoch` → Crypto kilidi kısa süre alınır; durum değişmez).
//!
//! # Ürün kurulumu (plan K4G-11 NE 1; sahip kararı CEVAP PAKETİ 2 madde 3)
//!
//! * kimlik + eş dizini: provisioning dosyasından, BELLEKTE (D8; ccr §35.4 KAPALI-SİSTEM KİPİ —
//!   dizin bant dışında doğrulanmış güven çapasıdır);
//! * saat: `kama_noise_wire::clock::SystemClock` — Crypto'nun saati. Application motorlarına
//!   (`Ptt`, `FileTransfer`) AYRI bir saat verilir (`kama_app::clock::SystemClock`, `lib.rs`): iki
//!   katmana tek saat tipi sokulmaz (INV-NEIGHBOR-1);
//! * entropi: `kama_noise_wire::entropy::OsEntropy` (işletim sisteminin CSPRNG'i);
//! * `init_counter` deposu: `kama_store::RedbStore` — `CounterStore` olarak; aynı `Arc` nesnesi
//!   Application'a `AppStore` olarak da verilir (D8: tek somut depo, iki ayrı arayüz).

use std::fmt;
use std::sync::Arc;

use client_core::hub::shared_crypto::{HubCrypto, HubCryptoAccess, StashConfig};
use client_core::hub::signer::{BindSigner, BIND_SIGNATURE_LEN, BIND_TBS_LEN};
use client_core::transport::ClientTransport;
use ed25519_dalek::{Signer, SigningKey};
use kama_noise_wire::clock::SystemClock;
use kama_noise_wire::entropy::{EntropySource, OsEntropy};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::session_control::{ControlConfig, EnvelopeKind};
use kama_noise_wire::CounterStore;
use kama_provision::format::DeviceConfig;
use kama_store::RedbStore;

// Eş dizini kurulamadı: `NodeError::PeerDirectory`'nin yükü (tip adı yalnız bu modülde geçer).
pub(crate) use kama_noise_wire::peer_dir::DirectoryError;

/// Ürün kriptosunun çekirdeği.
pub(crate) type Inner = NoiseWireProtection;

/// Ürün kriptosu: [`HubCrypto`] sarmalayıcısı (S2-A3-2, KK-H52). Doğrudan kipte
/// `HubCrypto::Direct` saf geçiştir (aynı çağrılar aynı sırayla, ek kilit yok); hub kipinde
/// `HubCrypto::Hub` tek-açma kanalıdır (`docs/design/HB2-ALIM-YOLU.md`).
pub(crate) type Crypto = HubCrypto<Inner>;

/// Ürün Transport'u: gerçek S1 adaptörü + ürün kriptosu + gerçek UDP veri yolu.
pub(crate) type Transport = ClientTransport<Crypto>;

/// Oturum zarfı: 22 B legacy SESSION_DATA (ccr §35.6). Gerekçe: doğrulanmış her gerçek hedef —
/// S1 conformance `real` (`kama-integration/tests/common/seam/real.rs`), K4G-01 / K4G-01b
/// regresyonları (`client-core/tests/{ctx_first_dispatch,session_keyed_slots}.rs`) — bu zarfla
/// koşar; 26 B extended zarfın bağımsız Python vektörü yoktur (KATMAN 6, S10). Uç başına tek kip:
/// iki uç aynı kurulumla açılır.
const ENVELOPE: EnvelopeKind = EnvelopeKind::Legacy;

/// Ürün kriptosunu kurar: gerçek anahtarlar (provisioning), gerçek saat, OS entropisi, kalıcı
/// `init_counter`. Anahtarlar kurulan nesnenin içine taşınır; `config` çağırandadır ve onunla
/// birlikte düşer (gizli alanlar Drop'ta sıfırlanır — kama-provision `Secret32`).
///
/// # Errors
/// [`DirectoryError`] — doğrulanmış dosyada bile eş dizini kurulamadıysa.
pub(crate) fn protection(
    config: &DeviceConfig,
    store: &Arc<RedbStore>,
) -> Result<Inner, DirectoryError> {
    let directory = config.peer_directory()?;
    let store: Arc<RedbStore> = Arc::clone(store);
    let counter_store: Arc<dyn CounterStore> = store;
    Ok(NoiseWireProtection::with_control(ControlConfig {
        identity: config.local_identity(),
        directory,
        clock: Box::new(SystemClock::new()),
        entropy: Box::new(OsEntropy::new()),
        counter_store,
        envelope: ENVELOPE,
    }))
}

/// Doğrudan kip: saf geçiş.
pub(crate) fn direct_crypto(inner: Inner) -> Crypto {
    HubCrypto::direct(inner)
}

/// Hub kipi: sarmalayıcı (transport'a) + erişim ucu (`HubEndpoint`'e).
pub(crate) fn hub_crypto(inner: Inner) -> (Crypto, Box<dyn HubCryptoAccess>) {
    let (crypto, tap) = HubCrypto::hub(inner, StashConfig::default());
    (crypto, Box::new(tap))
}

// ── [S2-A3-2] hub kipi kurulumu (KK-H56) ─────────────────────────────────────────────────────

/// PoC-BIND imzacısı (S4 §2.4(b), KR-5): **cihaz** Ed25519 anahtarı (`DeviceConfig.ed25519`;
/// sertifikadaki cihaz anahtarı, kimlik anahtarı DEĞİL). Anahtar `zeroize`'lı `SigningKey`
/// içindedir, `Debug` basmaz; imzacı yalnız `HubLink`'in tipli alanlardan kurduğu 112 baytlık TBS'yi
/// görür (imza kâhini yok, D-14).
pub(crate) struct DeviceBindSigner {
    key: SigningKey,
}

impl fmt::Debug for DeviceBindSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceBindSigner(<redacted>)")
    }
}

impl BindSigner for DeviceBindSigner {
    fn sign_bind(&self, tbs: &[u8; BIND_TBS_LEN]) -> [u8; BIND_SIGNATURE_LEN] {
        self.key.sign(tbs).to_bytes()
    }
}

/// Cihaz dosyasından imzacı; `drop(config)`'tan ÖNCE çağrılır (`lib.rs` kurulum sırası).
pub(crate) fn bind_signer(config: &DeviceConfig) -> Box<dyn BindSigner> {
    Box::new(DeviceBindSigner {
        key: SigningKey::from_bytes(config.ed25519.secret.expose()),
    })
}

/// `device_cert_ref = SHA-256(compact_device_cert)` (`kama_noise_wire::handshake::cert_ref_of`;
/// S4 H2).
pub(crate) fn device_cert_ref(config: &DeviceConfig) -> [u8; 32] {
    kama_noise_wire::handshake::cert_ref_of(config.cert.bytes())
}

/// `HubLink`'in entropisi (`request_id` başlangıcı, `client_nonce`): OS CSPRNG'i.
pub(crate) fn hub_entropy() -> Box<dyn EntropySource> {
    Box::new(OsEntropy::new())
}

/// YALNIZ birim testleri: cihaz dosyalarını gerçek provisioning yolundan (`kama-provision`
/// kütüphanesi, OS entropisi) üretir. Entropi kaynağının adı yalnız bu modülde geçer (C8).
#[cfg(test)]
pub(crate) fn provision_for_test(request: &kama_provision::provision::InitRequest) {
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("sistem saati")
        .as_secs();
    kama_provision::provision::init(request, &mut OsEntropy::new(), created_at)
        .expect("provisioning");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use kama_noise_wire::ed25519_strict::verify_strict;
    use kama_provision::format::{load_device_config, DeviceName};
    use kama_provision::provision::InitRequest;
    use std::collections::BTreeMap;

    /// KK-H56: imza CİHAZ anahtarıyla (`ed25519`), katı kipte doğrulanır; kimlik anahtarıyla
    /// doğrulanmaz. `Debug` anahtar basmaz. `device_cert_ref` = SHA-256(cert).
    #[test]
    fn bind_signer_uses_the_device_key_and_verifies_strictly() {
        let root = TempDir::new("bind-signer");
        let name = DeviceName::new("alice").expect("ad");
        let out_dir = root.path().join("authority");
        provision_for_test(&InitRequest {
            out_dir: out_dir.clone(),
            devices: vec![name.clone(), DeviceName::new("bob").expect("ad")],
            hub: None,
            direct: BTreeMap::new(),
        });
        let config = load_device_config(&out_dir.join(name.file_name())).expect("cihaz dosyası");
        let signer = bind_signer(&config);
        let tbs: [u8; BIND_TBS_LEN] = core::array::from_fn(|i| i as u8);
        let sig = signer.sign_bind(&tbs);
        verify_strict(config.ed25519.public.bytes(), &tbs, &sig).expect("cihaz anahtarı");
        assert!(
            verify_strict(config.identity_ed25519.public.bytes(), &tbs, &sig).is_err(),
            "kimlik anahtarı değil (S4 KR-5)"
        );
        assert_eq!(signer.sign_bind(&tbs), sig, "Ed25519 deterministik");
        let mut other = tbs;
        other[0] ^= 1;
        assert!(verify_strict(config.ed25519.public.bytes(), &other, &sig).is_err());
        let shown = format!(
            "{:?}",
            DeviceBindSigner {
                key: SigningKey::from_bytes(config.ed25519.secret.expose()),
            }
        );
        assert_eq!(shown, "DeviceBindSigner(<redacted>)");
        assert_eq!(device_cert_ref(&config), config.cert.cert_ref());
    }
}
