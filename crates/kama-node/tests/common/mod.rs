//! `kama-node` testlerinin ortak yardımcıları.
//!
//! Gövde `kama_node::testing`'dedir (`test-util`; INT-04 ile TEK kaynak). Burada yalnız
//! provisioning'in entropi adımı kalır: bileşim kökünün ürün kaynağında Crypto katmanının adı
//! yalnız kurma modülünde geçebilir (layer_check C8), testler o kapsamın dışındadır.
//!
//! * Provisioning: cihaz dosyaları `kama-provision` KÜTÜPHANESİYLE (`provision::init`, OS
//!   entropisi) geçici dizinde üretilir — elle yazılmış fikstür DEĞİL. Otorite çıktısı (`root.json`
//!   dahil) ayrı bir dizinde kalır; her "telefon"un dizinine YALNIZ kendi `device-<ad>.json`'u
//!   kopyalanır (D9: `root.json` cihaz dizinine girmez — denetlenir).
//! * Olaylar: dinleyici olayları kanala yazar; [`Events::wait`] beklenen olayı ÜST SÜRE SINIRIYLA
//!   bekler (gerçek uyku yok; koşul olaydır). Eşleşmeyen olaylar sırasıyla saklanır, sonraki
//!   beklemeler önce onlara bakar (olay sırası iddiası gerekmez). Periyodik `Metrics` olayları
//!   saklanmaz; kanalda ise bekleyebilirler: bir aşamanın SONRASINI gösteren ölçü için önce
//!   [`Events::drain_now`] çağrılır (`two_nodes_loopback::expect_clean_metrics`).
//! * Zarf taşıyıcısı: ÜRÜN yolu (`KamaNode::start` → süreç-içi röle). Röle süreç-geneldir ve
//!   testler paralel koşabilir: her test kendi cihazlarını [`provision`] ile (yeni anahtarlar,
//!   yeni kimlikler) üretir; sabit kimlik yoktur.

// Bu ortak modül birden fazla test ikilisinde ayrı derlenir; her ikili yardımcıların hepsini
// kullanmaz (yeniden dışa açılanlar dahil).
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

/// `names` cihazlarını kama-provision kütüphanesiyle üretir. `direct` doğruysa her cihaza
/// 127.0.0.1'de boş bir UDP portu verilir (direct-mode).
pub fn provision(root: &Path, names: &[&str], direct: bool) -> Provisioned {
    let addrs: Vec<SocketAddr> = if direct {
        free_ports(names.len())
            .into_iter()
            .map(|port| SocketAddr::from(([127, 0, 0, 1], port)))
            .collect()
    } else {
        Vec::new()
    };
    provision_at(root, names, &addrs)
}

/// [`provision`], direct-mode adresleri çağırandan: `addrs[i]` `names[i]`'nin adresidir (eksikse
/// o cihazın adresi yok). Adresi bir soketle TUTAN test, port yarışı olmadan kurulur.
pub fn provision_at(root: &Path, names: &[&str], addrs: &[SocketAddr]) -> Provisioned {
    let request = init_request(root, names, addrs);
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("sistem saati")
        .as_secs();
    init(&request, &mut OsEntropy::new(), created_at).expect("provisioning");
    devices_from(root, &request, addrs)
}
