//! [S2-A3-2 / HB2] Hub kipi — bileşim kökünün hub parçası (S4 `docs/seams/KM-SEAM-TH-001.md`).
//!
//! **Seçim (KK-H50):** hub kipi = (dışarıdan zarf taşıyıcısı verilmemiş) ∧ (cihaz dosyasında `hub`
//! var; v1'de `hub.ed25519_pub` zorunlu). Başka her durumda bugünkü doğrudan yol (`carrier::
//! product_envelope_link` + `direct_addrs`) bit-eşit sürer; `KamaNode::start` imzası değişmez.
//!
//! **Kurulum (KK-H51, KK-H63):** `hub.addr` açılışta bir kez çözülür, ilk IPv4 kullanılır. Hub soketi
//! `0.0.0.0:0`, yerel röle `127.0.0.1:0`; veri alıcısı `127.0.0.1:0`'a bağlanır ve her eşin veri
//! adresi röle adresidir. `HubLink` (HB1) + `HubEnvelopeLink` transport'un zarf taşıyıcısıdır;
//! `RecordingLink` onu davranış değiştirmeden sarar (KK-H66). `BindSigner` cihaz anahtarıyla
//! `wiring.rs`'te kurulur (KK-H56).
//!
//! **Çalışma (`node.rs`):** `Ready` → [`HubSide::peer_ready`] (KK-H58); her tur
//! [`HubSide::take_notices`] (KK-H57). C9 `last_heartbeat_age_s` A5-min'dedir.
//!
//! Bu modül protokol mantığı taşımaz: yalnız kurar ve HB1/HB2 tiplerinin yöntemlerini çağırır.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

#[cfg(feature = "test-util")]
use client_core::hub::endpoint::HubEndpointCounters;
use client_core::hub::endpoint::{
    HubControl, HubEndpoint, HubEndpointConfig, KnownHandles, RecordingLink,
};
use client_core::hub::{HubConfig, HubEnvelopeLink, HubLink, HubNotice};
use client_core::transport::{EnvelopeLink, TransportBuildError};
use kama_contracts::handles::PeerRef;
use kama_provision::format::{DeviceConfig, FormatError};

use crate::error::NodeError;
use crate::wiring::{self, Crypto, Inner};

fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Cihaz dosyasından hub parametreleri (gizli alan yok).
#[derive(Debug, Clone, Copy)]
pub(crate) struct HubParams {
    addr: SocketAddr,
    ed25519_pub: [u8; 32],
}

/// KK-H50 / KK-H63: `hub` yoksa `None`; varsa adres bir kez çözülür, ilk IPv4 alınır.
///
/// # Errors
/// [`NodeError::Config`] (`hub.addr unresolvable`).
pub(crate) fn params(config: &DeviceConfig) -> Result<Option<HubParams>, NodeError> {
    let Some(hub) = config.hub.as_ref() else {
        return Ok(None);
    };
    let addr = hub
        .addr
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.find(SocketAddr::is_ipv4))
        .ok_or(NodeError::Config(FormatError::Invalid(
            "hub.addr unresolvable",
        )))?;
    Ok(Some(HubParams {
        addr,
        ed25519_pub: *hub.ed25519_pub.bytes(),
    }))
}

/// `drop(config)`'tan ÖNCE kurulan, cihaz dosyasından türeyen hub girdileri (imzacı dahil).
pub(crate) struct HubInputs {
    params: HubParams,
    cfg: HubConfig,
    signer: Box<dyn client_core::hub::BindSigner>,
}

/// Gizli anahtarı taşıyan dosya düşmeden önce çağrılır (KK-H56).
pub(crate) fn inputs(config: &DeviceConfig, params: HubParams) -> HubInputs {
    HubInputs {
        params,
        cfg: HubConfig {
            hub_addr: params.addr,
            hub_ed25519_pub: params.ed25519_pub,
            device_cert_ref: wiring::device_cert_ref(config),
            self_dest_hash: *config.dest_hash.bytes(),
        },
        signer: wiring::bind_signer(config),
    }
}

/// Kurulmuş hub kipi: transport'a verilecek kripto + taşıyıcı, ve düğümün tuttuğu taraf.
pub(crate) struct HubStart {
    pub(crate) crypto: Crypto,
    pub(crate) link: Box<dyn EnvelopeLink>,
    pub(crate) side: HubSide,
}

fn transport_io(error: io::Error) -> NodeError {
    NodeError::Transport(TransportBuildError::Io(error))
}

/// `HubLink` + `HubEndpoint` + taşıyıcı (KK-H51). Hata yolunda açılan soketler kapanır.
///
/// # Errors
/// [`NodeError::Transport`] (`HubLink` kurulamadı ya da soket açılamadı; KK-H55).
pub(crate) fn start(inner: Inner, inputs: HubInputs) -> Result<HubStart, NodeError> {
    let HubInputs {
        params,
        cfg,
        signer,
    } = inputs;
    let link = HubLink::new(cfg, signer, wiring::hub_entropy(), Instant::now())
        .map_err(|e| transport_io(io::Error::other(e)))?;
    let link = Arc::new(Mutex::new(link));
    let (crypto, tap) = wiring::hub_crypto(inner);
    let known = KnownHandles::new();
    let control: Arc<Mutex<dyn HubControl>> = link.clone();
    let endpoint = HubEndpoint::start(
        HubEndpointConfig::product(params.addr),
        tap,
        control,
        known.clone(),
    )
    .map_err(transport_io)?;
    let envelope = RecordingLink::new(HubEnvelopeLink::new(Arc::clone(&link)), known.clone());
    Ok(HubStart {
        crypto,
        link: Box::new(envelope),
        side: HubSide {
            endpoint,
            link,
            known,
        },
    })
}

/// Veri alıcısının hub kipindeki bağlanma adresi (KK-H51).
pub(crate) const DATA_BIND: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::LOCALHOST,
    0,
));

/// Düğümün tuttuğu hub tarafı (düşerken `HubEndpoint` döngüleri durur).
pub(crate) struct HubSide {
    endpoint: HubEndpoint,
    link: Arc<Mutex<HubLink>>,
    known: KnownHandles,
}

impl HubSide {
    /// Her eşin veri adresi (röle).
    pub(crate) fn relay_addr(&self) -> SocketAddr {
        self.endpoint.relay_addr()
    }

    /// Veri alıcısının adresini bildirir (transport kurulduktan sonra).
    pub(crate) fn set_server_addr(&self, addr: SocketAddr) {
        self.endpoint.set_server_addr(addr);
    }

    /// KK-H58: S1 `Ready` → `HubLink::peer_ready(peer, h)` (T3 durur). `h` taşıyıcının o eşe
    /// bağladığı son handle'dır; bilinmiyorsa çağrı yapılmaz (T3 yedeği `on_session_traffic`).
    pub(crate) fn peer_ready(&self, peer: PeerRef) {
        if let Some(h) = self.known.handle_of(peer) {
            lock(&self.link).peer_ready(peer, h);
        }
    }

    /// Bekleyen hub bildirimleri (sınırlı: bir turda en çok `cap`).
    pub(crate) fn take_notices(&self, cap: usize) -> Vec<HubNotice> {
        let mut link = lock(&self.link);
        std::iter::from_fn(|| link.take_notice())
            .take(cap)
            .collect()
    }

    /// C9 (A5-min): hub'dan gelen son datagramın (her tür) yaşı, saniye.
    pub(crate) fn last_rx_age_s(&self) -> Option<u64> {
        lock(&self.link)
            .last_rx_age(Instant::now())
            .map(|d| d.as_secs())
    }

    /// `HubEndpoint` sayaçları (salt-okunur; ölçüm sondası).
    #[cfg(feature = "test-util")]
    pub(crate) fn counters(&self) -> HubEndpointCounters {
        self.endpoint.counters()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{event_channel, TempDir};
    use crate::{KamaNode, NodeEvent, NodeListener};
    use client_core::transport::{LinkAttempt, LinkError, LinkEvent};
    use kama_contracts::s3::HubHandle;
    use kama_provision::format::{load_device_config, DeviceName};
    use kama_provision::provision::InitRequest;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::mpsc::Sender;

    /// `alice` + `bob`; `hub` verilirse cihaz dosyalarında `hub` alanı (v1: `ed25519_pub` dahil).
    fn devices(root: &TempDir, hub: Option<&str>, direct: bool) -> (PathBuf, PathBuf) {
        let names = [
            DeviceName::new("alice").expect("ad"),
            DeviceName::new("bob").expect("ad"),
        ];
        let out_dir = root.path().join("authority");
        let direct = if direct {
            names
                .iter()
                .cloned()
                .zip([47_101u16, 47_102].map(|p| SocketAddr::from(([127, 0, 0, 1], p))))
                .collect()
        } else {
            BTreeMap::new()
        };
        wiring::provision_for_test(&InitRequest {
            out_dir: out_dir.clone(),
            devices: names.to_vec(),
            hub: hub.map(str::to_owned),
            direct,
        });
        (out_dir.join(names[0].file_name()), root.path().join("data"))
    }

    /// KK-H50: `hub` yoksa doğrudan kip (`None`); varsa adres çözülür (KK-H63: IPv4).
    #[test]
    fn params_follow_the_device_file() {
        let root = TempDir::new("hub-params-none");
        let (path, _) = devices(&root, None, true);
        let config = load_device_config(&path).expect("cihaz");
        assert!(params(&config).expect("hub yok").is_none());

        let root = TempDir::new("hub-params-some");
        let (path, _) = devices(&root, Some("127.0.0.1:47000"), false);
        let config = load_device_config(&path).expect("cihaz");
        let p = params(&config).expect("çözülür").expect("hub var");
        assert_eq!(p.addr, SocketAddr::from(([127, 0, 0, 1], 47_000)));
        assert_eq!(
            p.ed25519_pub,
            *config.hub.as_ref().expect("hub").ed25519_pub.bytes()
        );
        let inputs = inputs(&config, p);
        assert_eq!(inputs.cfg.device_cert_ref, config.cert.cert_ref());
        assert_eq!(inputs.cfg.self_dest_hash, *config.dest_hash.bytes());
        assert!(!format!("{:?}", inputs.cfg).contains(&format!("{:?}", config.dest_hash.bytes())));
    }

    /// KK-H55 / KK-H63: çözülemeyen `hub.addr` yeni varyant değil, `NodeError::Config`.
    #[test]
    fn unresolvable_hub_addr_is_a_config_error() {
        let root = TempDir::new("hub-params-bad");
        let (path, _) = devices(&root, Some("hub.invalid:47000"), false);
        let config = load_device_config(&path).expect("cihaz");
        assert!(matches!(
            params(&config),
            Err(NodeError::Config(FormatError::Invalid(
                "hub.addr unresolvable"
            )))
        ));
    }

    struct NullLink;
    impl EnvelopeLink for NullLink {
        fn send_init(&mut self, _: PeerRef, _: u8, _: Vec<u8>) -> Result<LinkAttempt, LinkError> {
            Err(LinkError)
        }
        fn send_response(
            &mut self,
            _: PeerRef,
            _: HubHandle,
            _: u8,
            _: Vec<u8>,
        ) -> Result<(), LinkError> {
            Err(LinkError)
        }
        fn poll(&mut self) -> Option<LinkEvent> {
            None
        }
    }

    struct Quiet(Sender<NodeEvent>);
    impl NodeListener for Quiet {
        fn on_event(&self, event: NodeEvent) {
            let _ = self.0.send(event);
        }
    }

    /// KK-H50: dışarıdan taşıyıcı verilen kurucu (`start_with_link_for_test`; L yolu, F0) cihaz
    /// dosyasında hub olsa da hub kipine GİRMEZ: doğrudan adres kullanılır, hub tarafı yoktur.
    #[test]
    fn external_link_never_enters_hub_mode() {
        let _serial = crate::test_support::serial();
        let root = TempDir::new("hub-external-link");
        let (path, data) = devices(&root, Some("127.0.0.1:47000"), true);
        let (tx, _events) = event_channel();
        let node = KamaNode::start_with_link_for_test(
            &path,
            &data,
            Box::new(Quiet(tx)),
            Box::new(NullLink),
            false,
            false,
        )
        .expect("doğrudan kip kurulur");
        let probe = node.probe_for_test().expect("sonda");
        assert!(probe.hub.is_none(), "hub tarafı kurulmadı");
        node.shutdown().expect("kapanış");
    }

    /// KK-H63: hub kipinde `direct_addrs` gerekmez — `NoDirectAddress` yolu atlanır. (HB1'in
    /// `HubLink::new` gövdesi gelene dek kurulum `NodeError::Transport` ile tipli biter; gövde
    /// geldiğinde düğüm kurulur. İki durumda da `NoDirectAddress` DEĞİL ve panik yok.)
    #[test]
    fn hub_mode_does_not_need_a_direct_address() {
        let _serial = crate::test_support::serial();
        let root = TempDir::new("hub-no-direct");
        let (path, data) = devices(&root, Some("127.0.0.1:47000"), false);
        let (tx, _events) = event_channel();
        match KamaNode::start(&path, &data, Box::new(Quiet(tx))) {
            Ok(node) => {
                let probe = node.probe_for_test().expect("sonda");
                assert!(probe.hub.is_some(), "hub tarafı kuruldu");
                assert_eq!(probe.envelopes, crate::probe::EnvelopeCounts::default());
                node.shutdown().expect("kapanış");
            }
            Err(NodeError::Transport(_)) => {}
            Err(other) => panic!("beklenmeyen hata: {other}"),
        }
    }
}
