//! transport-API-v0 YÜZEY PIN'i (P4 — kama-contract'ın transport-tarafı dondurulmuş alt-kümesi).
//!
//! Bu bir ÇALIŞMA doğrulaması DEĞİL, bir DERLEME kapısıdır. Dondurulmuş public imzalardan biri
//! (isim/alan/varyant/imza) sessizce değişir veya silinirse bu dosya DERLENMEZ → CI RED.
//! Gerekçe: KALICI-KURAL-3 — hash-kimliği ≠ sözleşme-uygunluğu; sözleşme yürütülebilir olmalı.
//!
//! KAPSAM (spec-uyumlu): yalnız KRİPTODAN BAĞIMSIZ transport-katmanı ürün yüzeyi. Signaling
//! `send_signaling/on_signaling` KESİN imzası ve `kama-contract` crate'i BURADA DEĞİL — onlar
//! `specs/signaling-seam-v0.md` §5 + CLAUDE.md §10 gereği §4.4 (quinn/custom) + signaling-API
//! stabilizasyonuna kadar ERTELENDİ. Bkz. `docs/design/P4-TRANSPORT-API-FREEZE.md`.

use std::net::SocketAddr;
use std::sync::Arc;

// Re-export yüzeyi (path-pin: bu yollar kaybolursa import derlenmez).
use client_core::runtime::{
    ClientEndpoint, ClientHandle, ClientStats, Delivery, FecRuntimeConfig, RuntimeConfig,
    RuntimeError, ServerEndpoint, ServerHandle, ServerStats,
};
use client_core::seam::{IdentitySeam, SecuritySeam, IDENTITY_TAG};
use client_core::wire::{
    decode_data, decode_fec, decode_recovered, encode_data, encode_fec, encode_recovered,
    FecFrameHeader, FRAME_DATA, FRAME_FEC, FRAME_RECOVERED, HDR_DATA, HDR_FEC, HDR_RECOVERED,
    RECOVERED_MAX_ESI,
};
use client_core::ClientSession;
// Motor re-export'ları (yalnız yol/isim-pin; derinlemesine imza motorun kendi testlerinde).
use client_core::{
    admit_and_decode, EngineError, Event, OpaqueUnit, OpenedTransportShard, Sender,
    TailRecoveryConfig, TransmitDecision, TransmitKind,
};

/// Değeri okunmayan tip-taşıyıcı: re-export'ların NAMEABLE olduğunu pinler (isim silinirse RED).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn _engine_reexport_names_are_frozen(
    _e: EngineError,
    _ev: Event,
    _u: OpaqueUnit,
    _o: OpenedTransportShard,
    _s: Sender,
    _t: TailRecoveryConfig,
    _d: TransmitDecision,
    _k: TransmitKind,
) {
    let _f: fn(Vec<OpenedTransportShard>, u8, u8) -> Result<Vec<u8>, EngineError> =
        admit_and_decode;
    let _ = _f;
}

/// Asla ÇALIŞMAZ (baştaki koşulsuz `return`) — gövde yalnız TİP-DENETİMİ için var. Her `.await`
/// ve her imza burada dondurulmuş tiplere bağlanır; imza kayarsa derleme kırılır.
#[tokio::test]
#[allow(
    unreachable_code,
    unused_variables,
    unused_mut,
    clippy::diverging_sub_expression,
    clippy::type_complexity
)]
async fn transport_api_v0_surface_is_frozen() {
    return;

    let cfg: RuntimeConfig = RuntimeConfig::default();
    let seam: Arc<dyn SecuritySeam> = Arc::new(IdentitySeam);
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    // ── RuntimeConfig: TAM alan kümesi (eksiksiz destructure → alan ekle/sil = RED) ──
    let RuntimeConfig {
        max_inflight_units: _,
        offer_queue: _,
        delivery_queue: _,
        max_unit_bytes: _,
        max_peers: _,
        quota_r: _,
        drain_cap: _,
        idle_hold: _,
        fec: _, // v0.1 (S1): minor-EKLEME — kasıtlı pin-güncellemesi, P4 belgesi §3
    } = cfg.clone();
    let FecRuntimeConfig {
        profile_id: _,
        manual_r: _,
    } = FecRuntimeConfig {
        profile_id: 0,
        manual_r: None,
    };

    // ── RuntimeError: TAM varyant kümesi (eksiksiz match, `_` yok → varyant ekle/sil = RED) ──
    match RuntimeError::Closed {
        RuntimeError::QueueFull => {}
        RuntimeError::UnitTooLarge { got: _, cap: _ } => {}
        RuntimeError::Closed => {}
        RuntimeError::Io(_) => {}
        RuntimeError::FecConfig(m) => {
            let _: &'static str = m; // payload-tipi pinli (v0.1)
        }
    }

    // ── ClientEndpoint / ClientHandle imzaları ──
    let ch: ClientHandle = ClientEndpoint::connect(addr, cfg.clone(), seam.clone())
        .await
        .unwrap();
    let mut ch = ch;
    let _: Result<(), RuntimeError> = ch.send(Vec::new());
    let _: Option<u64> = ch.acked().await;
    let _: ClientStats = ch.shutdown().await;

    // ── ServerEndpoint / ServerHandle imzaları ──
    let sh: ServerHandle = ServerEndpoint::bind(addr, cfg.clone(), seam.clone())
        .await
        .unwrap();
    let mut sh = sh;
    let _: SocketAddr = sh.local_addr();
    let _: Option<Delivery> = sh.next_delivery().await;
    let _: ServerStats = sh.shutdown().await;

    // ── ClientStats / ServerStats / Delivery: TAM alan kümesi ──
    let ClientStats {
        offered: _,
        acked: _,
        undelivered: _,
        tx_datagrams: _,
        rx_datagrams: _,
        open_failures: _,
        unknown_frames: _,
        stale_decisions: _,
        send_errors: _,
        unaccepted_offers: _,
        fec_blocks_latched: _,
        fec_repairs_tx: _,
        fec_repair_missing: _,
        fec_recovered_ctrl_units: _,
    } = ClientStats::default();
    let ServerStats {
        delivered: _,
        delivery_dropped: _,
        dropped_new_peers: _,
        retired_idle_peers: _,
        rx_datagrams: _,
        open_failures: _,
        unknown_frames: _,
        fec_repairs_rx: _,
        fec_recovered_units: _,
        fec_decode_failures: _,
        fec_blocks_evicted: _,
        fec_recovered_ctrl_tx: _,
    } = ServerStats::default();
    let Delivery {
        peer: _,
        unit_id: _,
        payload: _,
    } = Delivery {
        peer: addr,
        unit_id: 0,
        payload: Vec::new(),
    };

    // ── SecuritySeam sözleşmesi (güvenlik-düzlemi bu 3 metoda takılır) ──
    let s = IdentitySeam;
    let _: Vec<u8> = s.seal(Vec::new());
    let _: Option<Vec<u8>> = s.open(&[]);
    let _: usize = s.overhead();
    let _: usize = IDENTITY_TAG;
    // dyn-uyumluluk (güvenlik-düzlemi Arc<dyn SecuritySeam> olarak takar):
    let _dyn: Arc<dyn SecuritySeam> = Arc::new(IdentitySeam);

    // ── wire-v0 codec imzaları (tag YOK — seam'in işi) ──
    let _: Vec<u8> = encode_data(0u16, 0u64, &[]);
    let _: Option<(u16, u64, &[u8])> = decode_data(&[]);
    let _: u8 = FRAME_DATA;
    let _: usize = HDR_DATA;

    // ── wire-v0.1 FEC + RECOVERED codec imzaları (freeze-audit kapatması: TAM alan kümesi) ──
    let fh = FecFrameHeader {
        kind: 0,
        profile_id: 0,
        block_id: 0,
        esi: 0,
        k_actual: 1,
        seq: 0,
    };
    let FecFrameHeader {
        kind: _,
        profile_id: _,
        block_id: _,
        esi: _,
        k_actual: _,
        seq: _,
    } = fh;
    let _: Vec<u8> = encode_fec(&fh, &[]);
    let _: Option<(FecFrameHeader, &[u8])> = decode_fec(&[]);
    let _: u8 = FRAME_FEC;
    let _: usize = HDR_FEC;
    let _: Vec<u8> = encode_recovered(0u32, &[]);
    let _: Option<(u32, Vec<u16>)> = decode_recovered(&[]);
    let _: u8 = FRAME_RECOVERED;
    let _: usize = HDR_RECOVERED;
    let _: usize = RECOVERED_MAX_ESI;

    // ── ClientSession (G2 sevk yüzeyi — tail-ON validated_v1): 5 metot + Default imza-pinli ──
    let _cs: ClientSession = ClientSession::new();
    let _cs2: ClientSession = ClientSession::default();
    let _f: fn(&mut ClientSession, Event, u64) -> Vec<TransmitDecision> = ClientSession::on_event;
    let _f2: fn(
        &mut ClientSession,
        Vec<OpenedTransportShard>,
        u8,
        u8,
    ) -> Result<Vec<u8>, EngineError> = ClientSession::admit;
    let _f3: fn(&ClientSession) -> &Sender = ClientSession::sender;
    let _f4: fn(&mut ClientSession) -> &mut Sender = ClientSession::sender_mut;
}
