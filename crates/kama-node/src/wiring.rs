//! Kurma modülü — bileşim kökünün Crypto katmanına (`kama-noise-wire`) dokunduğu TEK yer.
//!
//! `scripts/ci/layer_check.py` C8: `kama-node`'un ürün kaynağında (`src/**`) `kama_noise_wire`
//! adı YALNIZ bu dosyada geçebilir. Burada yalnız YAPICI düzeyi vardır: ürün kurulumu
//! (`NoiseWireProtection::with_control`), onun girdileri ve tip adları. Çalışma zamanında düğüm
//! Crypto'yu çağırmaz: Crypto'yu Transport sürer (INV-NEIGHBOR-1), düğüm yalnız Application
//! motorlarını sürer.
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

use std::sync::Arc;

use client_core::transport::ClientTransport;
use kama_noise_wire::clock::SystemClock;
use kama_noise_wire::entropy::OsEntropy;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::session_control::{ControlConfig, EnvelopeKind};
use kama_noise_wire::CounterStore;
use kama_provision::format::DeviceConfig;
use kama_store::RedbStore;

// Eş dizini kurulamadı: `NodeError::PeerDirectory`'nin yükü (tip adı yalnız bu modülde geçer).
pub(crate) use kama_noise_wire::peer_dir::DirectoryError;

/// Ürün kriptosu.
pub(crate) type Crypto = NoiseWireProtection;

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
) -> Result<Crypto, DirectoryError> {
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
