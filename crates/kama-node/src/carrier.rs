//! Zarf taşıyıcısı — INIT/RESPONSE zarflarını (SSI1/SSR1) eşe götüren
//! `client_core::transport::EnvelopeLink` ("Plane 1'in bu sprintteki dar yerine geçeni").
//!
//! # DUR-K4G-11 (SAHİPTE, cevap bekleniyor)
//!
//! Ürün `EnvelopeLink`'i YOKTUR: tek gerçekleme `client_core::memory_link` (bellek içi röle) ve
//! YALNIZ `test-util`'dir; direct-mode SSI1/SSR1 UDP çerçevesi spec'te tanımlı değildir. Sahibe
//! sunulan seçenekler: (A) ürün `DirectUdpLink` (yeni tel çerçevesi → spec + vektör + sahip
//! kararı), (B) bu sprint için süreç-içi zarf rölesi ürün koduna (iki düğüm aynı süreçte), (C) veri
//! soketinde yeni çerçeve (bağlam-önce dağıtımı deler; önerilmedi).
//!
//! Karar gelene kadar ürün taşıyıcısının TEK kurucusu [`product_envelope_link`] tipli hata döner
//! ([`crate::ENVELOPE_LINK_PENDING`]; `todo!` / `unimplemented!` YOK). Bileşim kökünün geri kalanı
//! taşıyıcıdan bağımsızdır: taşıyıcı `KamaNode`'un kurulumuna tek bir kurucu olarak girer;
//! `test-util` kurucusu (`KamaNode::start_with_envelope_link`) aynı yere test rölesini verir.

use std::collections::HashMap;
use std::net::SocketAddr;

use client_core::transport::EnvelopeLink;
use kama_contracts::handles::PeerRef;

use crate::error::NodeError;
use crate::ENVELOPE_LINK_PENDING;

/// Ürün zarf taşıyıcısının TEK kurucusu. Girdiler kararın üç seçeneğinin de isteyeceği kurulum
/// bilgisidir: bu cihazın kimliği (`me`), veri alıcısının adresi ve eşlerin direct-mode
/// adresleri.
///
/// # Errors
/// Bugün daima [`NodeError::FeatureNotImplemented`] ([`ENVELOPE_LINK_PENDING`]) — DUR-K4G-11.
pub(crate) fn product_envelope_link(
    _me: PeerRef,
    _data_bind: SocketAddr,
    _peer_addrs: &HashMap<PeerRef, SocketAddr>,
) -> Result<Box<dyn EnvelopeLink>, NodeError> {
    Err(NodeError::FeatureNotImplemented(ENVELOPE_LINK_PENDING))
}
