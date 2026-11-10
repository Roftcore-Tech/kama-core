//! S4 §2 tel biçiminin hub istemcisinin gerektirdiği alt kümesi — **yerel tip** (KK-H34).
//!
//! `kama_contracts::s4` (HA) `main`'e girene dek burada durur; girince geçiş ayrı commit'te yapılır
//! ve bu modül kaldırılır. Bayt farkı çıkarsa S4 kazanır, `vectors/hub` hakemdir.
//!
//! Tüm sayılar big-endian, tüm alanlar sabit genişlikte (ADR-05; varint yok). Çözme KATIDIR.
//!
//! Çerçeve sınırı: bu modül **açılmış** kontrol çerçevesini kodlar/çözer:
//! `[0;4] ‖ frame_type ‖ gövde` (16 B no-op kuyruk kriptonun `seal_hub`/`open_hub`'ındadır).
//!
//! HB1-M0: yalnız sabitler (gövdeler HB1-M1'de).

/// `MAX_DATAGRAM` (S4 §6).
pub const MAX_DATAGRAM: usize = 1280;
/// HubChannel no-op mühür kuyruğu (S4 §2.1; kriptoda).
pub const HUB_TRAILER_LEN: usize = 16;
/// Açılmış kontrol çerçevesinin azami boyu (`MAX_DATAGRAM − 16`).
pub const MAX_OPENED_FRAME: usize = MAX_DATAGRAM - HUB_TRAILER_LEN;

/// HubChannel `frame_type` (engine-spec §1.1; `0x1D` S4 [YAMA]).
pub mod frame {
    pub const HEARTBEAT: u8 = 0x1B;
    pub const HEARTBEAT_ACK: u8 = 0x1C;
    pub const PLANE1_RPC: u8 = 0x1D;
}

/// `AppRpcFrame.method_id` (S4 §2.3).
pub mod method {
    pub const CLIENT_BIND: u8 = 0x01;
    pub const STORE_MESSAGE: u8 = 0x05;
    pub const BIND_PROOF: u8 = 0x0B;
    pub const DELIVER: u8 = 0x81;
    pub const ROUTE_MISS: u8 = 0x84;
}

/// `envelope_type` (S4 §2.4(c)).
pub mod envelope {
    pub const SESSION_INIT: u8 = 0x01;
    pub const SESSION_INIT_RESPONSE: u8 = 0x02;
    pub const RELAY_OPEN: u8 = 0x05;
}
