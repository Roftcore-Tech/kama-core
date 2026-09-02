//! Session-state + cross-session-admission (§3 dört-kimlik + §2.55 handle-scoping + §5 renewal).
//!
//! SECURE-WIRE-IDENTITY = (session_id, direction, dh_epoch, full_pn) (§3). Wire'da session_id TAŞINMAZ —
//! h_ext (routing-handle, §2.55) session-scoped'tur; datagram h_ext'iyle doğru-session'a demux edilir.
//! **CROSS-SESSION REDDİ:** eski-session-datagramı (eski-h_ext) yeni-session'a admission-ALAMAZ — yeni-session
//! yeni-h_ext alır (§2.55 RESTART), eski-h_ext unknown-handle→cheap-drop (ccr-§35.7). Ayrıca anahtar-farkı
//! (per-session Split()) → AEAD-fail ikinci-savunma.

use std::collections::HashMap;

use crate::error::WireError;
use crate::opened::OpenedTransportShard;
use crate::outer::ExtendedOuter;
use crate::protector::{KamaNoiseWireProtector, SealArgs, SealedPacket};
use kama_transport_schema::UnsealedTransportPacket;

/// İç session kimliği (wire'da TAŞINMAZ; state-scoping için). Handshake'ten türetilir (Sprint-4 sonrası).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Yön (direction-local state; §3). Endpoint'in gönderdiği vs aldığı yön.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Bu-endpoint initiator (SESSION_INIT gönderen).
    Initiator,
    /// Bu-endpoint responder.
    Responder,
}

/// Session yaşam-durumu (§5 renewal + §2.57 SESSION_CLOSE-tombstone).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLifecycle {
    /// Aktif — seal/open yapar.
    Active,
    /// Kapatıldı (SESSION_CLOSE / cap→renewal) — data-admit-ETMEZ; bounded-grace'te yalnız-tombstone.
    Closed,
    /// A2 iade-zinciri: Closing = yeni-cold-alloc-yasak + send-retired; recv/grace YAŞAR (Closed-değil).
    #[cfg(feature = "experimental-cold-start")]
    Closing,
}

/// Bir E2E session'ın durumu: kimlik + yön + protector (h_ext + anahtarlar + PN-state).
pub struct Session {
    session_id: SessionId,
    direction: Direction,
    protector: KamaNoiseWireProtector,
    lifecycle: SessionLifecycle,
}

impl Session {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        direction: Direction,
        protector: KamaNoiseWireProtector,
    ) -> Self {
        Self {
            session_id,
            direction,
            protector,
            lifecycle: SessionLifecycle::Active,
        }
    }

    #[must_use]
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
    #[must_use]
    pub fn direction(&self) -> Direction {
        self.direction
    }
    #[must_use]
    pub fn h_ext(&self) -> u32 {
        self.protector.h_ext()
    }
    #[must_use]
    pub fn lifecycle(&self) -> SessionLifecycle {
        self.lifecycle
    }

    /// SESSION_CLOSE / cap→renewal: session'ı kapatır (data-admit-etmez; §5).
    pub fn close(&mut self) {
        self.lifecycle = SessionLifecycle::Closed;
    }

    /// A2 iade-zinciri adım-(1)+(2b): lifecycle=Closing + A-4 send-state-retirement (İDEMPOTENT).
    /// Yapısal-pin: `drop_send_state` pub(crate) — Closing-dışı çağrı-yolu YOK (A-4 postcondition-1).
    /// Recv-yolu grace boyunca YAŞAR (open Closing'de çalışır; yalnız Closed reddeder).
    #[cfg(feature = "experimental-cold-start")]
    pub fn begin_close(&mut self) {
        self.lifecycle = SessionLifecycle::Closing;
        self.protector.drop_send_state();
    }

    #[must_use]
    pub fn protector_mut(&mut self) -> &mut KamaNoiseWireProtector {
        &mut self.protector
    }

    /// Seal (yalnız-Active). Closed-session'da SessionClosed.
    ///
    /// # Errors
    /// [`WireError::SessionClosed`] veya protector-seal-hataları.
    pub fn seal(
        &mut self,
        p: &UnsealedTransportPacket,
        args: SealArgs,
    ) -> Result<SealedPacket, WireError> {
        if self.lifecycle == SessionLifecycle::Closed {
            return Err(WireError::SessionClosed(self.h_ext()));
        }
        self.protector.seal(p, args)
    }
}

/// Session-demux (§2.55 handle-routing): h_ext → Session. Gelen-datagram doğru-session'a yönlenir;
/// bilinmeyen/eski h_ext → UnknownHandle (cross-session/stale reddi). Aynı-endpoint çok-session tutabilir
/// (session-migration/renewal sırasında eski+yeni bir-arada).
#[derive(Default)]
pub struct SessionManager {
    by_handle: HashMap<u32, Session>,
}

impl SessionManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Yeni-session kurar (h_ext-scoped). Aynı-h_ext varsa ÜZERİNE-YAZAR (renewal); farklı-h_ext = ayrı-session.
    pub fn install(&mut self, session: Session) {
        self.by_handle.insert(session.h_ext(), session);
    }

    /// Session'ı kaldırır (renewal/expiry sonrası; sonraki eski-h_ext-datagramları UnknownHandle).
    pub fn remove(&mut self, h_ext: u32) -> Option<Session> {
        self.by_handle.remove(&h_ext)
    }

    #[must_use]
    pub fn get_mut(&mut self, h_ext: u32) -> Option<&mut Session> {
        self.by_handle.get_mut(&h_ext)
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.by_handle.len()
    }

    /// Gelen-datagram'ı h_ext-demux ile aç (§4-ORDER: outer-parse → handle-lookup → session.open).
    /// **CROSS-SESSION REDDİ:** h_ext hiçbir-session'da yoksa → UnknownHandle; session Closed ise → SessionClosed.
    ///
    /// # Errors
    /// [`WireError::UnknownHandle`] / [`WireError::SessionClosed`] / protector-open-hataları (AEAD/PN/inner).
    pub fn open(
        &mut self,
        datagram: &[u8],
    ) -> Result<(SessionId, OpenedTransportShard), WireError> {
        // Fixed-cost outer-parse (decrypt-öncesi h_ext'i öğren; §2.5 handle→layout).
        let (outer, _ct, _tag) = ExtendedOuter::parse(datagram)?;
        let session = self
            .by_handle
            .get_mut(&outer.h_ext)
            .ok_or(WireError::UnknownHandle(outer.h_ext))?;
        if session.lifecycle == SessionLifecycle::Closed {
            return Err(WireError::SessionClosed(outer.h_ext));
        }
        let sid = session.session_id;
        let opened = session.protector.open(datagram)?;
        Ok((sid, opened))
    }
}
