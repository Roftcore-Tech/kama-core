//! Eş adres defteri — provisioning dosyasından, BELLEKTE (plan D8 v1.1: kimlik ve eş dizini
//! depoya girmez). API eşleri provisioning adıyla (`device-<ad>.json`'daki `peers[].name`) anar;
//! düğüm onları `PeerRef`'e (`dest_hash`) çevirir. Dizin çalışma zamanında BÜYÜMEZ: boyu dosyadaki
//! eş sayısıdır (kama-provision doğrulaması: en çok 63).
//!
//! Gizli değer taşımaz: adlar, `dest_hash` sarmalayıcıları (`PeerRef`; `Debug`'ı bayt basmaz) ve
//! direct-mode adresleri.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;

use kama_contracts::handles::PeerRef;
use kama_provision::format::DeviceConfig;

use crate::error::NodeError;

/// Provisioning dosyasının açık kısmı (bkz. modül belgesi).
#[derive(Debug)]
pub(crate) struct PeerBook {
    /// Bu cihaz.
    me: PeerRef,
    /// Bu cihazın direct-mode veri adresi (`direct_addrs[device_id]`).
    own_addr: Option<SocketAddr>,
    by_name: BTreeMap<String, PeerRef>,
    by_ref: BTreeMap<PeerRef, String>,
    /// Direct-mode veri adresi olan eşler.
    addrs: HashMap<PeerRef, SocketAddr>,
}

impl PeerBook {
    /// Doğrulanmış dosyadan (adlar ve `dest_hash`'ler tekildir — kama-provision §5).
    pub(crate) fn from_config(config: &DeviceConfig) -> Self {
        let me = PeerRef::from_dest_hash(*config.dest_hash.bytes());
        let own_addr = config.direct_addrs.get(&config.device_id).copied();
        let mut by_name = BTreeMap::new();
        let mut by_ref = BTreeMap::new();
        let mut addrs = HashMap::new();
        for peer in &config.peers {
            let peer_ref = PeerRef::from_dest_hash(*peer.dest_hash.bytes());
            let name = peer.name.as_str().to_owned();
            by_name.insert(name.clone(), peer_ref);
            by_ref.insert(peer_ref, name);
            if let Some(addr) = config.direct_addrs.get(&peer.name) {
                addrs.insert(peer_ref, *addr);
            }
        }
        Self {
            me,
            own_addr,
            by_name,
            by_ref,
            addrs,
        }
    }

    pub(crate) fn me(&self) -> PeerRef {
        self.me
    }

    pub(crate) fn own_addr(&self) -> Option<SocketAddr> {
        self.own_addr
    }

    /// Eşlerin direct-mode veri adresleri (`ClientTransportConfig::peer_addrs`).
    pub(crate) fn peer_addrs(&self) -> HashMap<PeerRef, SocketAddr> {
        self.addrs.clone()
    }

    /// API adı → eş.
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`].
    pub(crate) fn resolve(&self, peer_id: &str) -> Result<PeerRef, NodeError> {
        self.by_name
            .get(peer_id)
            .copied()
            .ok_or(NodeError::UnknownPeer)
    }

    /// Eş → API adı. Crypto yalnız dizindeki eşle oturum açtığı için Transport'un bildirdiği her
    /// eş burada bulunur; bulunamayan (sözleşme dışı) eşin olayı düşer.
    pub(crate) fn name(&self, peer: PeerRef) -> Option<&str> {
        self.by_ref.get(&peer).map(String::as_str)
    }

    /// (ad, eş) çiftleri, ad sırasıyla.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, PeerRef)> + '_ {
        self.by_name
            .iter()
            .map(|(name, peer)| (name.as_str(), *peer))
    }
}
