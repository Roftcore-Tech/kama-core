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

// Re-export yüzeyi (path-pin: bu yollar kaybolursa import derlenmez).
use client_core::runtime::{
    ClientEndpoint, ClientHandle, ClientStats, Delivery, FecRuntimeConfig, RuntimeConfig,
    RuntimeError, ServerEndpoint, ServerHandle, ServerStats,
};
use client_core::seam::{
    CloseReason, EntropyError, EntropySource, IdentitySeam, Opened, PacketProtection, PeerRef,
    ProtectionClass, ProtectionContext, ProtectionLimits, SealError, SealedPacketMeta, SealedUnit,
    SessionCommand, SessionControl, SessionEvent, SessionHandle, SessionResult, IDENTITY_TAG,
};
use client_core::wire::{
    decode_cs_echo, decode_cs_probe, decode_data, decode_data_frag, decode_fec, decode_param,
    decode_param_ack, decode_recovered, encode_cs_echo, encode_cs_probe, encode_data,
    encode_data_frag, encode_fec, encode_param, encode_param_ack, encode_recovered, FecFrameHeader,
    CS_PROBE_SUBTYPE, FRAME_CS_ECHO, FRAME_CS_PROBE, FRAME_DATA, FRAME_DATA_FRAG, FRAME_FEC,
    FRAME_PARAM, FRAME_PARAM_ACK, FRAME_RECOVERED, HDR_DATA, HDR_DATA_FRAG, HDR_FEC, HDR_RECOVERED,
    LEN_CS_ECHO, LEN_CS_PROBE, LEN_PARAM, LEN_PARAM_ACK, RECOVERED_MAX_ESI,
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
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    // ── RuntimeConfig: TAM alan kümesi (eksiksiz destructure → alan ekle/sil = RED) ──
    let RuntimeConfig {
        max_inflight_units: _,
        offer_queue: _,
        delivery_queue: _,
        max_unit_bytes: _,
        max_message_bytes: _, // v0.2 (S1B fragmenter): minor-EKLEME, kasıtlı pin-güncellemesi
        max_peers: _,
        quota_r: _,
        drain_cap: _,
        idle_hold: _,
        initial_window_bytes: _, // v0.5 (IW): minor-EKLEME, kasıtlı pin-güncellemesi
        bootstrap_rtt: _,
        bootstrap_fast_ladder: _, // v0.6 (kuruluş merdiveni): kasıtlı pin-güncellemesi // v0.4 (DR-CS-3 kablolama): minor-EKLEME, kasıtlı pin-güncellemesi
        fec_structure_policy: _,  // v0.3 (BURST-FUSION): minor-EKLEME, kasıtlı pin-güncellemesi
        fec: _, // v0.1 (S1): minor-EKLEME — kasıtlı pin-güncellemesi, P4 belgesi §3
        // v0.7 (DEADLINE-EKSENİ): minor-EKLEME, kasıtlı pin-güncellemesi. `None` = sevk
        // davranışı (yapı-gate koşar); `Some` = kalibre politika otoritedir.
        fusion_policy: _,
        // v0.8 (ONAY UFKU): minor-EKLEME, kasıtlı pin-güncellemesi. Default = sevk davranışı;
        // `ChannelDerived` bir ÖLÇÜM KOLUDUR (reports/loss-ledger-confirm-defect/).
        loss_confirm_horizon: _,
        // v0.9 (FEC-RISK): minor-EKLEME, kasıtlı pin-güncellemesi. Default KAPALI; AÇIKKEN
        // tek tüketici FEC oranı kararıdır (CC/pacing/ARQ dokunulmaz).
        fec_loss_risk: _,
        // v0.11 (HANDOFF ENSTRÜMANI): minor-EKLEME, kasıtlı pin-güncellemesi. Default None; salt gözlem.
        ts_sample_period_ns: _,
        // v0.12 (R-SWEEP): gate seviyesi override'ı; default None = sevk.
        structure_gate_level_permille: _,
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
    let ch: ClientHandle =
        ClientEndpoint::connect(addr, cfg.clone(), Box::new(IdentitySeam::new()))
            .await
            .unwrap();
    let mut ch = ch;
    let _: Result<(), RuntimeError> = ch.send(Vec::new());
    let _: Option<u64> = ch.acked().await;
    let _: ClientStats = ch.shutdown().await;

    // ── ServerEndpoint / ServerHandle imzaları ──
    let sh: ServerHandle = ServerEndpoint::bind(addr, cfg.clone(), Box::new(IdentitySeam::new()))
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
        seal_failures: _,
        param_sends: _,
        param_confirmed: _,
        fec_blocks_latched: _,
        fec_repairs_tx: _,
        fec_repair_missing: _,
        fec_recovered_ctrl_units: _,
        // [egress-seam] wire byte muhasebesi + EgressPolicy reddi (DENEME düzlemi: ret dahil).
        tx_bytes: _,
        rx_bytes: _,
        tx_data_bytes: _,
        tx_retx_bytes: _,
        tx_dup_bytes: _,
        tx_fec_bytes: _,
        tx_ctrl_bytes: _,
        egress_refused_pkts: _,
        egress_refused_bytes: _,
        // [S1B] fragmenter: mesaj-granülü ACK + canonical-serialize hata sayacı.
        messages_acked: _,
        frag_encode_failures: _,
        fec_partial_blocks_closed: _,
        // [TEŞHİS] yapı-gate karar tabanı + karar-yolu histogramı + kuruluş görünürlüğü.
        fate: _,
        // [DR-CS-3] cold-start probe/echo muhasebesi.
        cs_probes_tx: _,
        cs_echoes_rx: _,
        // v0.10 (KISA-AKIŞ ENSTRÜMANI): minor-EKLEME, kasıtlı pin-güncellemesi. Salt gözlem —
        // ana döngü çıkış anı, sender damgalarıyla aynı saat tabanında.
        flow_end_ns: _,
        // v0.11: bounded zaman serisi (yalnız örnekleme açıkken dolar).
        ts: _,
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
        fec_repairs_wasted_complete: _,
        fec_repairs_wasted_evicted: _,
        fec_repairs_useful: _,
        fec_blocks_complete_with_repairs: _,
        fec_blocks_decoded: _,
        fec_blocks_evicted_with_repairs: _,
        fec_recovered_ctrl_tx: _,
        seal_failures: _,
        param_bound_peers: _,
        param_rejects: _,
        fec_pre_param_dropped: _,
        // [egress-seam] dönüş-yönü wire byte muhasebesi + EgressPolicy reddi.
        tx_datagrams: _,
        tx_bytes: _,
        rx_bytes: _,
        egress_refused_pkts: _,
        egress_refused_bytes: _,
        // [S1B] fragmenter: chunk birleştirme muhasebesi.
        frag_chunks_rx: _,
        msgs_reassembled: _,
        frag_chunk_dropped: _,
        msg_asm_evicted: _,
        // [DR-CS-3] cold-start probe/echo muhasebesi.
        cs_probes_rx: _,
        cs_echoes_tx: _,
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

    // ── S3 sözleşmesi (KM-SEAM-TC-001 §1.1 PacketProtection + §1.2 SessionControl), A4.1 ──
    // v1'in `SecuritySeam`/`SealArgs`/`OpenReject`'i KALKTI: PN sahipliği Crypto'da (K2), `open`
    // sessiz (K5), bağlam çağırandan (K1). Bu blok o yüzeyi pinler; biri kaybolursa derlenmez.
    let mut sm = IdentitySeam::new();
    let ctx: ProtectionContext = ProtectionContext::Session(IdentitySeam::SESSION);
    match ctx {
        ProtectionContext::Session(_h) => {}
        ProtectionContext::HubChannel => {}
    }
    match ProtectionClass::Session {
        ProtectionClass::Session => {}
        ProtectionClass::HubChannel => {}
    }
    let su: SealedUnit = sm.seal(ctx, &[]).unwrap();
    let SealedUnit {
        bytes: _,
        meta:
            SealedPacketMeta {
                full_wire_pn: _,
                dh_epoch: _,
                sealed_len: _,
            },
    } = su;
    let od: Option<Opened> = sm.open(&[0u8; 32]);
    if let Some(Opened {
        ctx: _,
        frame: _,
        meta: _,
    }) = od
    {}
    // SealError KAPALI ve yalnız yapısal (S3 §3).
    match SealError::CapReached {
        SealError::FrameTooLarge => {}
        SealError::UnknownSession => {}
        SealError::CapReached => {}
    }
    let _: Result<usize, SealError> = sm.sealed_len(ctx, 0);
    let ProtectionLimits {
        wire_overhead_bytes: _,
        max_plaintext_bytes: _,
    } = sm.conservative_limits(ProtectionClass::Session);
    let _: ProtectionLimits = sm.limits(ctx);
    let _: usize = IDENTITY_TAG;
    // dyn-uyumluluk (endpoint sahipliği): Box<dyn PacketProtection>.
    let _dyn: Box<dyn PacketProtection> = Box::new(IdentitySeam::new());

    // SessionControl tipleri (K6 typed komut · K10/DR-CONNECT-1 K5 provisional + Confirmed).
    let h = SessionHandle::from_raw(1);
    let peer = PeerRef::from_dest_hash([1; 32]);
    for cmd in [
        SessionCommand::Initiate {
            peer,
            early_unit: None,
        },
        SessionCommand::AcceptInit { wire: Vec::new() },
        SessionCommand::AcceptResponse { wire: Vec::new() },
        SessionCommand::Renew { session: h },
        SessionCommand::Close { session: h },
    ] {
        match cmd {
            SessionCommand::Initiate { .. }
            | SessionCommand::AcceptInit { .. }
            | SessionCommand::AcceptResponse { .. }
            | SessionCommand::Renew { .. }
            | SessionCommand::Close { .. } => {}
        }
    }
    match SessionResult::NoOutput {
        SessionResult::OutboundEnvelope {
            envelope_type: _,
            bytes: _,
        } => {}
        SessionResult::Established { session: _ } => {}
        SessionResult::EstablishedAndReply {
            session: _,
            envelope_type: _,
            bytes: _,
        } => {}
        SessionResult::ProvisionalAndReply {
            new: _,
            supersedes: _,
            envelope_type: _,
            bytes: _,
        } => {}
        SessionResult::NoOutput => {}
        SessionResult::Failed => {}
    }
    match SessionEvent::Ready(h) {
        SessionEvent::Ready(_) => {}
        SessionEvent::RenewRequired(_) => {}
        SessionEvent::Closed(
            _,
            CloseReason::Superseded
            | CloseReason::UserClosed
            | CloseReason::PeerClosed
            | CloseReason::KeyHygiene,
        ) => {}
        SessionEvent::Confirmed(_) => {}
    }
    fn _dyn_session_control(_: &mut dyn SessionControl) {}
    fn _dyn_entropy(e: &mut dyn EntropySource) -> Result<(), EntropyError> {
        e.fill(&mut [0u8; 32])
    }

    // ── wire-v0 codec imzaları (tag YOK — seam'in işi) ──
    let _: Vec<u8> = encode_data(0u16, 0u64, &[]);
    let _: Option<(u16, u64, &[u8])> = decode_data(&[]);
    let _: u8 = FRAME_DATA;
    let _: usize = HDR_DATA;
    // [S1B] fragmentli DATA (0x05): gövde = canonical SourcePrefix ‖ payload.
    let _: Vec<u8> = encode_data_frag(0u16, 0u64, &[]);
    let _: Option<(u16, u64, client_core::SourcePrefix, &[u8])> = decode_data_frag(&[]);
    let _: u8 = FRAME_DATA_FRAG;
    let _: usize = HDR_DATA_FRAG;
    // [DR-CS-3] cold-start probe/echo (0x06/0x07; şema 0x19/0x1A alan-hizası, amp≤1).
    let _: Vec<u8> = encode_cs_probe(CS_PROBE_SUBTYPE, 0u16);
    let _: Option<(u8, u16)> = decode_cs_probe(&[]);
    let _: Vec<u8> = encode_cs_echo(CS_PROBE_SUBTYPE, 0u16, 0u32, 0u32);
    let _: Option<(u8, u16, u32, u32)> = decode_cs_echo(&[]);
    let _: u8 = FRAME_CS_PROBE;
    let _: u8 = FRAME_CS_ECHO;
    // ANTİ-AMPLİFİKASYON, DERLEME-ZAMANI: echo probe'dan küçük olmak ZORUNDA (amp ≤ 1).
    // `const _` formu boyutlar kayarsa CI'ı derlemede kırar — runtime assert'i bu dosyada
    // (koşulsuz `return` gövdesi) hiç çalışmazdı.
    const _: () = assert!(LEN_CS_ECHO < LEN_CS_PROBE);

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
    let _: Vec<u8> = encode_param(0, 0, &[0u8; 32]);
    let _: Option<(u8, u8, [u8; 32])> = decode_param(&[]);
    let _: Vec<u8> = encode_param_ack(&[0u8; 32]);
    let _: Option<[u8; 32]> = decode_param_ack(&[]);
    let _: u8 = FRAME_PARAM;
    let _: u8 = FRAME_PARAM_ACK;
    let _: usize = LEN_PARAM;
    let _: usize = LEN_PARAM_ACK;

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
