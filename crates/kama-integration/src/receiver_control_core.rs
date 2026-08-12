//! DR-CS-3 — **DEFAULT-BUILD receiver control core** (no `experimental-cold-start`).
//!
//! This is a reusable **library component**, not a product runtime. It owns:
//! session-bound authenticated receive, AEAD open/seal, authenticated control decode, bounded
//! responder state, typed outbound control actions, and sealed outbound datagram drain.
//!
//! It deliberately does **NOT** own: UDP sockets, a receive/send loop, process lifecycle, connection
//! lifecycle, or the Noise handshake. Those belong to a future product runtime consumer
//! (`PRODUCT-RUNTIME-INTEGRATION-PENDING.md`). Nothing here can be run by a user on its own.
//!
//! Dependencies are all **ungated**: `Session`/`KamaNoiseWireProtector`/`SealArgs` (kama-noise-wire),
//! `decode_control` (kama-noise-wire), `Control` (kama-transport-schema). It pulls in **none** of the
//! `experimental-cold-start` cold-RX subsystem (`ColdRxState`, `GlobalColdLedger`, `TombstoneRegistry`,
//! `ControlDedupWindow`, `ControlIdIssueGate`).

use kama_noise_wire::control::decode_control;
use kama_noise_wire::inner::InnerPlain;
use kama_noise_wire::{
    Direction, KamaNoiseWireProtector, SealArgs, Session, SessionId,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::UnsealedTransportPacket;
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::Zeroizing;

/// DR-CS-3 contract §16: at most four unique echo generations per session (the authoritative
/// four-step ladder). Fixed-size — never a growing set.
pub const MAX_ECHOES: usize = 4;
/// `ECHO_WIRE_MAX` from the shipping schema (`control.rs:67`): 43 B. The probe is 44 B, so the
/// amplification ratio is < 1 by construction.
pub const ECHO_WIRE_MAX: u64 = 43;
/// Contract §17 receiver budget: 4 × 43 = 172 B per session.
pub const ECHO_BUDGET_BYTES: u64 = MAX_ECHOES as u64 * ECHO_WIRE_MAX;
/// Bound on concurrently installed sessions in this component (fail-closed, no silent eviction).
pub const MAX_CORE_SESSIONS: usize = 1024;

/// Typed core errors (reachable `todo!()`/panic forbidden).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    /// No such session (or it was closed).
    #[error("unknown session: {h_ext:#010x}")]
    UnknownSession {
        /// External handle.
        h_ext: u32,
    },
    /// Session capacity reached; fail-closed.
    #[error("session capacity reached: cap={cap}")]
    SessionCapacity {
        /// Cap.
        cap: usize,
    },
    /// Bounded responder refusal (duplicate generation, generation cap, or budget).
    #[error("bootstrap responder refused: {0}")]
    ResponderRefused(&'static str),
    /// Seal failure.
    #[error("seal failed: {0}")]
    SealFailed(&'static str),
}

/// Result of ingesting one authenticated datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreIngest {
    /// AEAD verification failed — dropped with no side effect (verify-before-admission).
    Rejected(&'static str),
    /// An authenticated `0x19` bootstrap probe was accepted and a typed echo action is pending.
    BootstrapProbe {
        /// Correlation carried by the request.
        probe_seq: u16,
        /// The request's control id (echoed back; probe/echo are stateless and never enter
        /// control-id dedup, per `control_id_gate.rs` A-2).
        control_id: u32,
    },
    /// An authenticated control frame this component does not act on.
    OtherControl {
        /// Inner kind codepoint.
        inner_kind: u8,
    },
    /// An authenticated non-control frame (e.g. a FEC shard) — not this component's concern.
    NonControl,
}

/// **Single-source** bootstrap `0x19 → 0x1A` responder state and decision logic.
///
/// Both the default-build [`ReceiverControlCore`] and the feature-gated `ReceiverHost` embed **this
/// one type** — there is no second implementation and no copy. Fixed-size state only.
#[derive(Debug, Clone, Copy, Default)]
pub struct BootstrapEchoResponder {
    /// Typed pending echo action (single slot).
    pending: Option<u16>,
    /// Generations already echoed — FIXED-SIZE array, never a growing set (contract §18).
    echoed: [Option<u16>; MAX_ECHOES],
    /// Echo wire bytes emitted for this session (contract §17).
    bytes: u64,
}

impl BootstrapEchoResponder {
    /// Record a typed pending echo action for an **already authenticated** probe, if the bounded
    /// policy allows it: fresh generation, generation slot free, budget available.
    pub fn on_authenticated_probe(&mut self, probe_seq: u16) {
        if self.echoed.contains(&Some(probe_seq)) {
            return; // duplicate/replay ⇒ no additional response
        }
        if !self.echoed.iter().any(Option::is_none) {
            return; // generation cap
        }
        if self.bytes + ECHO_WIRE_MAX > ECHO_BUDGET_BYTES {
            return; // budget
        }
        self.pending = Some(probe_seq);
    }

    /// Drain the pending typed action (at most once per arrival).
    pub fn take_pending(&mut self) -> Option<u16> {
        self.pending.take()
    }

    /// Commit one echo emission against the bounded policy. Returns the wire size to account for.
    ///
    /// # Errors
    /// [`CoreError::ResponderRefused`] on duplicate generation, generation cap, or budget.
    pub fn commit(&mut self, probe_seq: u16) -> Result<u64, CoreError> {
        if self.echoed.contains(&Some(probe_seq)) {
            return Err(CoreError::ResponderRefused("duplicate generation"));
        }
        let Some(free) = self.echoed.iter().position(Option::is_none) else {
            return Err(CoreError::ResponderRefused("generation cap"));
        };
        if self.bytes + ECHO_WIRE_MAX > ECHO_BUDGET_BYTES {
            return Err(CoreError::ResponderRefused("budget"));
        }
        self.echoed[free] = Some(probe_seq);
        self.bytes += ECHO_WIRE_MAX;
        Ok(ECHO_WIRE_MAX)
    }

    /// Build the typed echo control frame. `seen_count`/`arrival_gap` are LEDGER-ONLY per the schema
    /// (`control.rs:68`) and are emitted as zero: a peer must never supply a sender clock input.
    #[must_use]
    pub fn echo_control(probe_seq: u16, control_id: u32) -> Control {
        Control::CsEcho {
            id: control_id,
            subtype: 0x01,
            probe_seq,
            seen_count: 0,
            arrival_gap: 0,
        }
    }

    /// Observation-only: `(unique_echoes, wire_bytes, budget)`.
    #[must_use]
    pub fn budget(&self) -> (usize, u64, u64) {
        (
            self.echoed.iter().filter(|g| g.is_some()).count(),
            self.bytes,
            ECHO_BUDGET_BYTES,
        )
    }

    /// Reset on session close / give-up (contract §26).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

struct CoreSession {
    session: Session,
    responder: BootstrapEchoResponder,
}

/// Default-build authenticated control component. See the module doc for what it is NOT.
pub struct ReceiverControlCore {
    sessions: BTreeMap<u32, CoreSession>,
}

impl Default for ReceiverControlCore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceiverControlCore {
    /// Empty component.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
        }
    }

    /// Install a session with **directional** keys. In production these come from the Noise
    /// handshake `Split()`; there is no handshake in this repository yet, so callers inject them.
    ///
    /// # Errors
    /// [`CoreError::SessionCapacity`] when the bound is reached (fail-closed).
    pub fn install_session_with_directional_keys(
        &mut self,
        h_ext: u32,
        send_key: &Zeroizing<[u8; 32]>,
        recv_key: &Zeroizing<[u8; 32]>,
    ) -> Result<(), CoreError> {
        if !self.sessions.contains_key(&h_ext) && self.sessions.len() >= MAX_CORE_SESSIONS {
            return Err(CoreError::SessionCapacity {
                cap: MAX_CORE_SESSIONS,
            });
        }
        let protector = KamaNoiseWireProtector::new(h_ext, send_key, recv_key);
        let session = Session::new(SessionId(u64::from(h_ext)), Direction::Responder, protector);
        self.sessions.insert(
            h_ext,
            CoreSession {
                session,
                responder: BootstrapEchoResponder::default(),
            },
        );
        Ok(())
    }

    /// Ingest one sealed datagram for `h_ext`. **Verify-before-admission**: AEAD open runs first and
    /// a failure produces no state change at all.
    ///
    /// # Errors
    /// [`CoreError::UnknownSession`] when the handle is not installed (no echo is ever produced).
    pub fn ingest(&mut self, h_ext: u32, datagram: &[u8]) -> Result<CoreIngest, CoreError> {
        let slot = self
            .sessions
            .get_mut(&h_ext)
            .ok_or(CoreError::UnknownSession { h_ext })?;
        let Ok(opened) = slot.session.protector_mut().open(datagram) else {
            return Ok(CoreIngest::Rejected("aead verify failed"));
        };
        let InnerPlain::Control {
            inner_kind,
            control_id,
            ..
        } = opened.inner()
        else {
            return Ok(CoreIngest::NonControl);
        };
        let (inner_kind, control_id) = (*inner_kind, *control_id);
        if inner_kind == 0x19 {
            if let Some(Ok(Control::CsProbe { probe_seq, .. })) = decode_control(opened.inner()) {
                slot.responder.on_authenticated_probe(probe_seq);
                return Ok(CoreIngest::BootstrapProbe {
                    probe_seq,
                    control_id,
                });
            }
            return Ok(CoreIngest::Rejected("malformed cs probe"));
        }
        Ok(CoreIngest::OtherControl { inner_kind })
    }

    /// Drain the typed pending echo action for `h_ext`.
    pub fn take_pending_cs_echo(&mut self, h_ext: u32) -> Option<u16> {
        self.sessions.get_mut(&h_ext)?.responder.take_pending()
    }

    /// Emit the bounded authenticated `0x1A` echo as a sealed outbound datagram.
    ///
    /// `control_id` is the request's id echoed back: probe/echo are stateless and explicitly do NOT
    /// enter control-id issuance/dedup (`control_id_gate.rs` A-2), so no issuance gate is consumed.
    ///
    /// # Errors
    /// [`CoreError::UnknownSession`] · [`CoreError::ResponderRefused`] · [`CoreError::SealFailed`].
    pub fn emit_cs_echo(
        &mut self,
        h_ext: u32,
        probe_seq: u16,
        control_id: u32,
        wire_pn: u64,
    ) -> Result<Vec<u8>, CoreError> {
        let slot = self
            .sessions
            .get_mut(&h_ext)
            .ok_or(CoreError::UnknownSession { h_ext })?;
        slot.responder.commit(probe_seq)?;
        let ctrl = BootstrapEchoResponder::echo_control(probe_seq, control_id);
        slot.session
            .seal(
                &UnsealedTransportPacket::Control(ctrl),
                SealArgs {
                    wire_pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .map(|sealed| sealed.bytes)
            .map_err(|_| CoreError::SealFailed("cs-echo seal"))
    }

    /// Observation-only responder budget for `h_ext`.
    #[must_use]
    pub fn echo_budget(&self, h_ext: u32) -> Option<(usize, u64, u64)> {
        Some(self.sessions.get(&h_ext)?.responder.budget())
    }

    /// Close/reset a session (contract §26: bootstrap state is cleared).
    pub fn close_session(&mut self, h_ext: u32) {
        self.sessions.remove(&h_ext);
    }

    /// Installed session count (bounded by [`MAX_CORE_SESSIONS`]).
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
