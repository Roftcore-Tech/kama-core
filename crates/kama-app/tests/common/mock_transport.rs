//! `MockTransport` — KM-SEAM-AT-001 §7'deki "Hayri için" karşı-taraf stub'u.
//!
//! Application'ı gerçek Transport/Crypto olmadan geliştirmeyi/test etmeyi sağlar. Plane 1 ve
//! Crypto **içermez** — yalnız `KamaTransport` sözleşmesinin gözlenebilir davranışını taklit
//! eder. Zamanlama (async handshake) olayları gerçek zamanlı değil, testin çağırdığı
//! `complete_connect`/`fail_connect`/`request_renew`/`complete_renew` enjeksiyon metodlarıyla
//! **deterministik** tetiklenir.

use std::collections::{HashMap, HashSet, VecDeque};

use kama_app::{
    CloseReason, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport, PeerRef,
    PreparedUnit, SessionEvent, SessionHandle, TransportCapabilities, TransportError,
};

pub struct MockTransport {
    capabilities: TransportCapabilities,
    next_id: u64,
    /// peer → aktif session (K1 idempotent connect'in kaynağı).
    active_by_peer: HashMap<PeerRef, SessionHandle>,
    /// peer → bekleyen ticket (henüz `complete_connect`/`fail_connect` çağrılmadı).
    pending_by_peer: HashMap<PeerRef, ConnectTicket>,
    /// pending ticket'a bağlı `early_unit`; `complete_connect`'te teslim edilir (K6).
    pending_early: HashMap<ConnectTicket, Vec<u8>>,
    ticket_peer: HashMap<ConnectTicket, PeerRef>,
    session_peer: HashMap<SessionHandle, PeerRef>,
    /// cutover ile supersede edilmiş session'lar: `send` → `SessionSuperseded` (H-CUTOVER).
    superseded: HashSet<SessionHandle>,
    events: VecDeque<SessionEvent>,
    inbox: VecDeque<Delivered>,
}

// Bu ortak modül birden fazla test binary'si tarafından ayrı ayrı derlenir (her
// `tests/*.rs` kendi crate'idir); her binary enjeksiyon metodlarının hepsini kullanmayabilir,
// bu yüzden dead_code kontrolü burada gevşetilir.
#[allow(dead_code)]
impl MockTransport {
    pub fn new(capabilities: TransportCapabilities) -> Self {
        Self {
            capabilities,
            next_id: 1,
            active_by_peer: HashMap::new(),
            pending_by_peer: HashMap::new(),
            pending_early: HashMap::new(),
            ticket_peer: HashMap::new(),
            session_peer: HashMap::new(),
            superseded: HashSet::new(),
            events: VecDeque::new(),
            inbox: VecDeque::new(),
        }
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Bekleyen bir `connect()`'i tamamlar: yeni `SessionHandle` üretir, `Ready` olayını
    /// kuyruğa koyar, varsa `early_unit`'i (H-ROUNDTRIP) teslim eder. Testin "handshake bitti"
    /// enjeksiyonu.
    pub fn complete_connect(&mut self, peer: &PeerRef) -> SessionHandle {
        let ticket = self
            .pending_by_peer
            .remove(peer)
            .expect("complete_connect: peer için bekleyen connect yok");
        self.ticket_peer.remove(&ticket);

        let handle = SessionHandle::from_raw(self.fresh_id());
        self.active_by_peer.insert(peer.clone(), handle);
        self.session_peer.insert(handle, peer.clone());
        self.events
            .push_back(SessionEvent::Ready(handle, peer.clone()));

        if let Some(early_bytes) = self.pending_early.remove(&ticket) {
            self.inbox.push_back(Delivered {
                session: handle,
                unit: early_bytes,
            });
        }
        handle
    }

    /// Bekleyen bir `connect()`'i başarısız kılar. Sebep kasıtlı olarak alınmaz — K9/H-OPAQUE-FAIL:
    /// yanlış peer material, bozuk RESPONSE, Plane 1 timeout dışarıdan ayırt edilemez.
    pub fn fail_connect(&mut self, peer: &PeerRef) {
        let ticket = self
            .pending_by_peer
            .remove(peer)
            .expect("fail_connect: peer için bekleyen connect yok");
        self.ticket_peer.remove(&ticket);
        self.pending_early.remove(&ticket);
        self.events.push_back(SessionEvent::ConnectFailed(ticket));
    }

    /// Aktif bir session için anahtar yenileme gereksinimini tetikler (`RenewRequired`).
    /// `session` cutover'a kadar `send` kabul etmeye devam eder (H-CUTOVER).
    pub fn request_renew(&mut self, session: SessionHandle) {
        assert!(
            self.session_peer.contains_key(&session) && !self.superseded.contains(&session),
            "request_renew: bilinmeyen veya zaten supersede edilmiş session"
        );
        self.events.push_back(SessionEvent::RenewRequired(session));
    }

    /// Atomik cutover: yeni `SessionHandle` aktif olur, eski `SessionSuperseded` ile kapanır.
    /// Olay sırası (H-EVENTS): `Ready(yeni)` sonra `Closed(eski, Renewed)`.
    pub fn complete_renew(&mut self, old: SessionHandle) -> SessionHandle {
        let peer = self
            .session_peer
            .get(&old)
            .cloned()
            .expect("complete_renew: bilinmeyen session");
        assert!(
            !self.superseded.contains(&old),
            "complete_renew: session zaten supersede edilmiş"
        );

        let new_handle = SessionHandle::from_raw(self.fresh_id());
        self.active_by_peer.insert(peer.clone(), new_handle);
        self.session_peer.insert(new_handle, peer.clone());
        self.superseded.insert(old);

        self.events.push_back(SessionEvent::Ready(new_handle, peer));
        self.events
            .push_back(SessionEvent::Closed(old, CloseReason::Renewed));
        new_handle
    }
}

impl KamaTransport for MockTransport {
    fn capabilities(&self) -> TransportCapabilities {
        self.capabilities
    }

    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError> {
        if let Some(&handle) = self.active_by_peer.get(&peer) {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }
            return Ok(ConnectResult::Ready(handle));
        }
        if let Some(&ticket) = self.pending_by_peer.get(&peer) {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }
            return Ok(ConnectResult::Pending(ticket));
        }

        let ticket = ConnectTicket::from_raw(self.fresh_id());
        self.pending_by_peer.insert(peer.clone(), ticket);
        self.ticket_peer.insert(ticket, peer);
        if let Some(unit) = early_unit {
            self.pending_early.insert(ticket, unit.bytes);
        }
        Ok(ConnectResult::Pending(ticket))
    }

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        _policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        if self.superseded.contains(&session) {
            return Err(TransportError::SessionSuperseded);
        }
        if !self.session_peer.contains_key(&session) {
            return Err(TransportError::UnknownSession);
        }
        if unit.bytes.len() as u32 > self.capabilities.max_plaintext_unit_bytes {
            return Err(TransportError::UnitTooLarge);
        }
        // Loopback: MockTransport peer'ı simüle etmez, gönderilen birimi doğrudan kendi
        // inbox'ına yansıtır (H-ROUNDTRIP: send → recv byte-identical).
        self.inbox.push_back(Delivered {
            session,
            unit: unit.bytes,
        });
        Ok(())
    }

    fn recv(&mut self) -> Option<Delivered> {
        self.inbox.pop_front()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }

    /// Handle'ı tamamen unutur (`UnknownSession`'a geçer — H-EVENTS: `Closed` sonrası bu
    /// çağrılınca `send` artık `UnknownSession` döner; cutover'ın hemen ardından, `close()`
    /// çağrılmadan önce hâlâ `SessionSuperseded` döner — H-CUTOVER ile çelişmez, iki ayrı an).
    fn close(&mut self, session: SessionHandle) {
        if let Some(peer) = self.session_peer.remove(&session) {
            // Yalnız bu handle o peer'in HÂLÂ aktif handle'ıysa eşlemeyi temizle — supersede
            // edilmiş eski bir handle kapatılırken peer'in YENİ aktif handle'ını silme.
            if self.active_by_peer.get(&peer) == Some(&session) {
                self.active_by_peer.remove(&peer);
            }
        }
        self.superseded.remove(&session);
    }
}
