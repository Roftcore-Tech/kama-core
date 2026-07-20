//! Handle-uzayı demux (§2.5 PN-WIDTH-DEMUX YOL-1): decrypt-ÖNCESİ ilk-4-byte-handle'dan doğru-parser seçilir.
//! h_legacy → legacy-SESSION_DATA-parser · h_ext → extended-parser. Cross-handle-karışma-YOK (handle-value
//! session+HANDLE_ASSIGN-ile-atanır; farklı-değer → farklı-parser).

use std::collections::HashMap;

/// Bir handle'ın parser-sınıfı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleClass {
    /// Legacy SESSION_DATA (22B) parser'ı.
    Legacy,
    /// Extended (FEC/control, 26B) parser'ı.
    Extended,
}

/// Handle → parser-sınıfı yönlendirici (§2.5). Datagram'ın ilk-4-byte'ı handle; sınıfa göre-parser.
#[derive(Debug, Default)]
pub struct HandleRouter {
    classes: HashMap<u32, HandleClass>,
}

impl HandleRouter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// h_legacy handle'ını legacy-parser'a kaydet.
    pub fn register_legacy(&mut self, h_legacy: u32) {
        self.classes.insert(h_legacy, HandleClass::Legacy);
    }
    /// h_ext handle'ını extended-parser'a kaydet.
    pub fn register_extended(&mut self, h_ext: u32) {
        self.classes.insert(h_ext, HandleClass::Extended);
    }

    /// Datagram'ın ilk-4-byte-handle'ından parser-sınıfını seçer (decrypt-öncesi). Bilinmeyen → None (drop).
    #[must_use]
    pub fn classify(&self, datagram: &[u8]) -> Option<HandleClass> {
        if datagram.len() < 4 {
            return None;
        }
        let handle = u32::from_be_bytes([datagram[0], datagram[1], datagram[2], datagram[3]]);
        self.classes.get(&handle).copied()
    }
}
