//! INT-04'ün kurulum yardımcıları.
//!
//! Gövde `kama_node::testing`'dedir (`test-util`; `kama-node`'un kendi `tests/common`'ı ile TEK
//! kaynak). Burada yalnız provisioning'in entropi adımı kalır: bileşim kökünün ürün kaynağında
//! Crypto katmanının adı yalnız kurma modülünde geçebilir (layer_check C8) — testler o kapsamın
//! dışındadır, bu çağrı buraya düşer.

#![allow(dead_code, unused_imports)]

use std::net::SocketAddr;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use kama_noise_wire::entropy::OsEntropy;
use kama_provision::provision::init;

pub use kama_node::testing::{
    devices_from, free_ports, init_request, listener, peer_ref_of, serial, ChannelListener, Device,
    Events, Provisioned, TempDir, WAIT,
};

/// `names` cihazlarını kama-provision kütüphanesiyle üretir; `addrs[i]` `names[i]`'nin direct-mode
/// veri adresidir. INT-04'te bu adresler netem'in uygulandığı veth uçlarıdır (ya da netem yokken
/// 127.0.0.1'de boş portlar).
pub fn provision_at(root: &Path, names: &[&str], addrs: &[SocketAddr]) -> Provisioned {
    let request = init_request(root, names, addrs);
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("sistem saati")
        .as_secs();
    init(&request, &mut OsEntropy::new(), created_at).expect("provisioning");
    devices_from(root, &request, addrs)
}
