//! [`HubLink`] — sans-IO hub istemcisi çekirdeği — ve [`HubEnvelopeLink`] (`EnvelopeLink` cephesi).
//!
//! Sorumluluklar (S4 §4):
//!
//! | yükümlülük | burada |
//! |---|---|
//! | C2 bind | [`super::bind`] makinesi; bind olmadan STORE ve heartbeat gitmez (kuyrukta bekler) |
//! | C3 INIT + T3 | `send_init` → `STORE{0, 0x01}`; aynı `request_id`, bayt-özdeş tekrar; yalnız `peer_ready` (eşleşen `h`), `{UNKNOWN_HANDLE, 0}`, çelişkili handle, `ATTEMPT_MAX_AGE`, aynı eşe yeni INIT, yeniden bind ya da terk (`cancel_attempt`) durdurur |
//! | C4 RESPONSE | `send_response` → `STORE{h, 0x02}`; yinelenen `(h, INIT)` → saklı RESPONSE yeniden (KK-H38); K3 tutma (KK-H37) |
//! | C5 ROUTE_MISS | `h` bırakılır; kurulu hat → `HandleLost`, `h`'si atanmış açılış denemesi → `AttemptEnded` (eş başına ≥ 5 s, ertelenen bildirim tür başına bir; KK-H39) |
//! | C6/C7 | [`super::heartbeat`]; "koptu" → yeniden bind; gönderilmiş, tamamlanmamış denemeler biter |
//! | C8 | `0x1D` → RPC, `0x1C` → heartbeat eşleyici, başka → sayaç; imzasız hata yalnız ipucu |
//! | C9 | [`HubLink::last_rx_age`] |
//!
//! Bütün tablolar sınırlıdır (SR-AUDIT-001); sınırlar aşağıdaki sabitlerdedir.
//!
//! Kilit kuralı (HB1 ↔ HB2 sözleşmesi): `HubLink` kilidi tutulurken kripto kilidi alınmaz;
//! `HubEndpoint` kripto kilidini bırakmadan `HubLink` kilidini almaz. `HubLink` hiçbir Session
//! durumuna (ARQ, PN, oturum tablosu) erişmez (C8, D1).
//!
//! Sürücü sözleşmesi (HB2): her girdiden (`on_control`, `send_*`, `on_tick`, …) sonra
//! `poll_transmit` `None` dönene dek, `poll_event` / `take_notice` boşalana dek çağrılır;
//! `next_deadline`'da `on_tick` (ya da `poll_transmit`) çağrılır.

use core::time::Duration;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;
use kama_noise_wire::entropy::EntropySource;

use kama_contracts::s4::{
    self, HubFrame, RpcBody, RpcMessage, S4Error, StoreMessageReq, ENVELOPE_RELAY_OPEN,
    ENVELOPE_SESSION_INIT, ENVELOPE_SESSION_INIT_RESPONSE, ERROR_HUB_CAPACITY, ERROR_RATE_LIMITED,
    METHOD_BIND_PROOF, METHOD_CLIENT_BIND, METHOD_STORE_MESSAGE, STATUS_OK, STATUS_PEER_UNBOUND,
    STATUS_UNKNOWN_HANDLE,
};

use super::bind::{rpc_frame_checked, BindDeps, BindMachine, BindOutcome, RequestIds};
use super::heartbeat::{HbOutcome, Heartbeat};
use super::t3::{t3_delay, ATTEMPT_MAX_AGE, BACKOFF_MIN};
use super::{BindSigner, HubConfig, HubCounters, HubError, HubNotice};
use crate::transport::{EnvelopeLink, LinkAttempt, LinkError, LinkEvent};

/// Eşzamanlı açılış denemesi tavanı = bind öncesi STORE kuyruğu (KK-H40; transport
/// `PENDING_INIT_CAP` ile aynı).
pub const MAX_ATTEMPTS: usize = 16;
/// Bind beklerken kuyruğa alınabilecek RESPONSE STORE'u (KK-H40).
pub const MAX_QUEUED_RESPONSES: usize = 16;
/// StoreResp'ten önce gelmiş, tutulan RESPONSE (KK-H37).
pub const MAX_HELD_RESPONSES: usize = 16;
/// Yanıtlayan `(h, INIT)` kaydı (KK-H38).
pub const MAX_RESPONDER_RECORDS: usize = 64;
/// Bilinen `h → eş` eşlemesi (ROUTE_MISS için; S4 `MAX_LINKS_PER_ENDPOINT` = 64).
pub const MAX_KNOWN_LINKS: usize = 64;
/// Biten denemelerin ve bırakılan hatların handle halkası (S4 §7 K9: geç yanıt ≠ çelişki).
pub const MAX_RETIRED_HANDLES: usize = 64;
/// RESPONSE STORE'larının `request_id` halkası (yanıt sınıflaması için).
pub const MAX_RESPONSE_STORE_IDS: usize = 64;
/// Gönderim kuyruğu.
pub const MAX_TX_QUEUE: usize = 64;
/// Transport'a giden olay kuyruğu.
pub const MAX_EVENTS: usize = 256;
/// Bildirim kuyruğu ve eş başına bildirim zamanı tablosu.
pub const MAX_NOTICES: usize = 64;
/// Eş başına bildirim aralığı ("eş başına ≥ 5 s geri çekilme"; KK-H39).
pub const NOTICE_BACKOFF: Duration = BACKOFF_MIN;

/// Başlatanın bir INIT denemesi.
struct InitAttempt {
    id: LinkAttempt,
    peer: PeerRef,
    request_id: u32,
    /// Bayt-özdeş STORE çerçevesi (T3 tekrarı bunu yollar).
    frame: Vec<u8>,
    started: Instant,
    sends: u32,
    next_send: Instant,
    assigned: Option<HubHandle>,
    /// Transport'a verilmiş RESPONSE baytları (aynı `(h, bayt)` ikinci kez çıkmaz).
    delivered_response: Option<Vec<u8>>,
}

/// Eş başına aralık (KK-H39) dolmadan gelen, ertelenmiş bildirimler. Bir tür ötekinin yerini ALMAZ
/// — `AttemptEnded` (deneme yeni INIT ile yeniden başlar) ile `HandleLost` (kurulu oturum için yeni
/// INIT) bileşim kökünde farklı işlerdir. `AttemptEnded` en çok bir; `HandleLost` kaybolan HER hat
/// için bir.
///
/// [HLR-STALE] Kaybolan hatların hepsi tutulur (yalnız sonuncusu DEĞİL): bileşim kökü
/// `HandleLost{peer, h}`'yi eşin etkin oturumunun hattıyla süzer (eski hattınki düşer). Yalnız
/// sonuncusu tutulsaydı, aynı aralıkta önce etkin hattın, sonra eski bir hattın kaybı geldiğinde
/// verilen tek bildirim eski hattınki olur, süzgeçte düşer ve etkin hattın kaybı hiç görülmezdi
/// (hat `known_links`'ten çıktığı için ikinci bildirim de gelmez). Her kayıp `known_links`'teki bir
/// girdiden doğar ve onu siler: liste [`MAX_KNOWN_LINKS`]'i aşamaz (tavan yine denetlenir).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Deferred {
    attempt_ended: bool,
    /// Kaybolan hatlar, geliş sırasıyla (yinelenmez; ≤ [`MAX_KNOWN_LINKS`]).
    handle_lost: Vec<HubHandle>,
}

impl Deferred {
    /// Yalnız eş bildirimleri (`AttemptEnded`, `HandleLost`) ertelenir; diğerleri buraya gelmez.
    /// `true` = tavan doluydu, en eski kayıp düştü (sayılır).
    fn add(&mut self, n: HubNotice) -> bool {
        match n {
            HubNotice::AttemptEnded { .. } => self.attempt_ended = true,
            HubNotice::HandleLost { h, .. } => {
                if self.handle_lost.contains(&h) {
                    return false;
                }
                let full = self.handle_lost.len() >= MAX_KNOWN_LINKS;
                if full {
                    self.handle_lost.remove(0);
                }
                self.handle_lost.push(h);
                return full;
            }
            HubNotice::Bound | HubNotice::Disconnected => {}
        }
        false
    }

    fn is_empty(&self) -> bool {
        !self.attempt_ended && self.handle_lost.is_empty()
    }

    /// Ertelenmişleri verir ve boşaltır. Sıra: önce `AttemptEnded` (bitmiş denemenin yenilemesi
    /// yeniden başlar), sonra `HandleLost`'lar geliş sırasıyla (bekleyen yenileme varsa ikincisi
    /// başlamaz) — birlikte tek yeni INIT verir.
    fn take(&mut self, peer: PeerRef) -> Vec<HubNotice> {
        let d = core::mem::take(self);
        d.attempt_ended
            .then_some(HubNotice::AttemptEnded { peer })
            .into_iter()
            .chain(
                d.handle_lost
                    .into_iter()
                    .map(|h| HubNotice::HandleLost { peer, h }),
            )
            .collect()
    }
}

/// Yanıtlayanın `(h, INIT)` kaydı (KK-H38).
struct ResponderRecord {
    h: HubHandle,
    init: Vec<u8>,
    /// `send_response`'un ürettiği bayt-özdeş STORE{h, 0x02} çerçevesi.
    response_frame: Option<Vec<u8>>,
}

/// StoreResp'ten önce gelen RESPONSE (K3; KK-H37).
struct HeldResponse {
    h: HubHandle,
    /// DELIVER'ın `sender_dest_hash`'i (bilgi; yalnız denemeye eşleme için, güvenlik kararı değil).
    sender: PeerRef,
    envelope_type: u8,
    bytes: Vec<u8>,
    at: Instant,
}

/// Sans-IO hub istemcisi (S4 §4 C2–C9).
pub struct HubLink {
    cfg: HubConfig,
    counters: HubCounters,
    signer: Box<dyn BindSigner>,
    entropy: Box<dyn EntropySource>,
    ids: RequestIds,
    bind: BindMachine,
    hb: Heartbeat,
    /// Bir kez bağlanıldı mı (C7 yeniden bind ayrımı).
    ever_bound: bool,
    next_attempt: u64,
    attempts: Vec<InitAttempt>,
    responder: VecDeque<ResponderRecord>,
    held: VecDeque<HeldResponse>,
    known_links: VecDeque<(HubHandle, PeerRef)>,
    retired: VecDeque<HubHandle>,
    response_store_ids: VecDeque<u32>,
    queued_responses: VecDeque<Vec<u8>>,
    tx: VecDeque<Vec<u8>>,
    events: VecDeque<LinkEvent>,
    notices: VecDeque<HubNotice>,
    /// Eş başına son verilen bildirim zamanı ve ertelenmiş bildirimler.
    peer_notice: Vec<(PeerRef, Instant, Deferred)>,
    last_rx: Option<Instant>,
    last_tx: Instant,
}

impl HubLink {
    /// Yeni istemci; bind hemen başlar (ilk `poll_transmit` CLIENT_BIND'i verir).
    ///
    /// `entropy`: üründe `OsEntropy`; testte sabit tohum. Sırayla çekilir: `request_id` başlangıcı
    /// (4 B), heartbeat token başlangıcı (4 B), her bind denemesinin `client_nonce`'u (16 B).
    ///
    /// # Errors
    /// [`HubError::Entropy`] — başlangıç değerleri ya da ilk `client_nonce` alınamadı.
    pub fn new(
        cfg: HubConfig,
        signer: Box<dyn BindSigner>,
        mut entropy: Box<dyn EntropySource>,
        now: Instant,
    ) -> Result<Self, HubError> {
        let mut rid = [0u8; 4];
        let mut tok = [0u8; 4];
        entropy.fill(&mut rid).map_err(|_| HubError::Entropy)?;
        entropy.fill(&mut tok).map_err(|_| HubError::Entropy)?;
        let mut ids = RequestIds::starting_at(u32::from_be_bytes(rid));
        let mut counters = HubCounters::default();
        let bind = BindMachine::start(
            now,
            &mut BindDeps {
                cfg: &cfg,
                signer: signer.as_ref(),
                entropy: entropy.as_mut(),
                ids: &mut ids,
                counters: &mut counters,
            },
        );
        if counters.entropy_errors > 0 {
            return Err(HubError::Entropy);
        }
        Ok(Self {
            cfg,
            counters,
            signer,
            entropy,
            ids,
            bind,
            hb: Heartbeat::new(u32::from_be_bytes(tok)),
            ever_bound: false,
            next_attempt: 1,
            attempts: Vec::with_capacity(MAX_ATTEMPTS),
            responder: VecDeque::new(),
            held: VecDeque::new(),
            known_links: VecDeque::new(),
            retired: VecDeque::new(),
            response_store_ids: VecDeque::new(),
            queued_responses: VecDeque::new(),
            tx: VecDeque::new(),
            events: VecDeque::new(),
            notices: VecDeque::new(),
            peer_notice: Vec::new(),
            last_rx: None,
            last_tx: now,
        })
    }

    /// Yapılandırma (salt-okunur).
    #[must_use]
    pub fn config(&self) -> &HubConfig {
        &self.cfg
    }

    // ── girişler ─────────────────────────────────────────────────────────────────────────────

    /// Kriptonun açtığı bir HubChannel çerçevesi: `[0;4] ‖ frame_type ‖ gövde` (16 B kuyruk YOK).
    pub fn on_control(&mut self, now: Instant, opened_frame: &[u8]) {
        bump(&mut self.counters.rx_frames);
        self.last_rx = Some(now);
        match HubFrame::decode(opened_frame) {
            Err(S4Error::UnknownFrameType) | Ok(HubFrame::Heartbeat(_)) => {
                bump(&mut self.counters.hub_frames_dropped);
            }
            Err(_) => bump(&mut self.counters.rx_malformed),
            Ok(HubFrame::HeartbeatAck(ack)) => {
                if self.bind.is_bound() && ack.token.is_some_and(|t| self.hb.on_ack(t)) {
                    bump(&mut self.counters.heartbeat_acks);
                } else {
                    bump(&mut self.counters.heartbeat_ack_unmatched);
                }
            }
            Ok(HubFrame::Plane1Rpc(body)) => match RpcMessage::decode(body) {
                Ok(msg) => self.on_rpc(now, msg),
                Err(S4Error::UnknownMethod) => bump(&mut self.counters.hub_frames_dropped),
                Err(_) => bump(&mut self.counters.rx_malformed),
            },
        }
        self.on_tick(now);
    }

    fn on_rpc(&mut self, now: Instant, msg: RpcMessage) {
        let rid = msg.request_id;
        match msg.body {
            RpcBody::Deliver(d) => self.on_deliver(now, &d),
            RpcBody::RouteMiss(r) => self.on_route_miss(now, r.hub_handle),
            RpcBody::StoreMessageResp(r) => {
                self.on_store_response(now, rid, r.status, r.assigned_handle, false);
            }
            RpcBody::Error {
                method_id: METHOD_STORE_MESSAGE,
                payload,
            } => self.on_store_response(now, rid, payload.error_code, 0, true),
            body @ (RpcBody::ClientBindResp(_)
            | RpcBody::BindProofResp(_)
            | RpcBody::Error {
                method_id: METHOD_CLIENT_BIND | METHOD_BIND_PROOF,
                ..
            }) => {
                let was_bound = self.bind.is_bound();
                let out = self.bind.on_response(
                    now,
                    rid,
                    &body,
                    &mut BindDeps {
                        cfg: &self.cfg,
                        signer: self.signer.as_ref(),
                        entropy: self.entropy.as_mut(),
                        ids: &mut self.ids,
                        counters: &mut self.counters,
                    },
                );
                match out {
                    BindOutcome::Nothing => {}
                    BindOutcome::Send(f) => self.push_tx(f),
                    BindOutcome::Bound if !was_bound => self.on_bound(now),
                    BindOutcome::Bound => {}
                }
            }
            // İstek satırları (tel → hub) hub'dan gelmez.
            _ => bump(&mut self.counters.hub_frames_dropped),
        }
    }

    fn on_bound(&mut self, now: Instant) {
        self.hb.reset();
        if self.ever_bound {
            // C7: gönderilmiş ama tamamlanmamış açılış denemeleri yeni INIT + yeni request_id ile
            // yeniden başlamalı (hub yeniden başlamış olabilir). Hiç gönderilmemişler kalır.
            let mut i = 0;
            while i < self.attempts.len() {
                if self.attempts[i].sends > 0 {
                    let a = self.take_attempt(i);
                    bump(&mut self.counters.attempt_rebind_ended);
                    self.peer_notice(now, a.peer, HubNotice::AttemptEnded { peer: a.peer });
                } else {
                    i += 1;
                }
            }
        }
        self.ever_bound = true;
        self.push_notice(HubNotice::Bound);
        while let Some(f) = self.queued_responses.pop_front() {
            self.push_tx(f);
        }
    }

    fn on_store_response(
        &mut self,
        now: Instant,
        request_id: u32,
        status: u16,
        h: u32,
        is_error: bool,
    ) {
        let Some(i) = self
            .attempts
            .iter()
            .position(|a| a.request_id == request_id)
        else {
            if self.response_store_ids.contains(&request_id) {
                bump(&mut self.counters.response_store_resp);
            } else {
                bump(&mut self.counters.late_store_resp);
            }
            return;
        };
        match (is_error, status) {
            (false, STATUS_OK) if h != 0 => {
                let h = HubHandle::from_raw(h);
                match self.attempts[i].assigned {
                    None if self.held_conflicts(i, h) => {
                        // S4 C3: aynı denemede tutulan DELIVER{RESPONSE} başka h söylüyordu.
                        let peer = self.attempts[i].peer;
                        self.held.retain(|r| r.sender != peer);
                        let a = self.take_attempt(i);
                        bump(&mut self.counters.attempt_handle_conflict);
                        self.peer_notice(now, a.peer, HubNotice::AttemptEnded { peer: a.peer });
                    }
                    None => {
                        self.attempts[i].assigned = Some(h);
                        bump(&mut self.counters.handles_assigned);
                        let id = self.attempts[i].id;
                        self.push_event(LinkEvent::HandleAssigned {
                            attempt: id,
                            hub_handle: h,
                        });
                        self.release_held(i, h);
                        // T3 SÜRER (KR-18): StoreResp OK el sıkışma değildir.
                    }
                    Some(a) if a == h => bump(&mut self.counters.store_ok_duplicate),
                    Some(_) => {
                        let a = self.take_attempt(i);
                        bump(&mut self.counters.attempt_handle_conflict);
                        self.peer_notice(now, a.peer, HubNotice::AttemptEnded { peer: a.peer });
                    }
                }
            }
            (false, STATUS_UNKNOWN_HANDLE) => {
                // {UNKNOWN_HANDLE, 0}: deneme hub'da bitmiştir (KR-19).
                let a = self.take_attempt(i);
                bump(&mut self.counters.attempt_unknown_handle);
                self.peer_notice(now, a.peer, HubNotice::AttemptEnded { peer: a.peer });
            }
            (false, STATUS_PEER_UNBOUND) | (true, ERROR_RATE_LIMITED | ERROR_HUB_CAPACITY) => {
                let a = &mut self.attempts[i];
                a.next_send = a.next_send.max(now + BACKOFF_MIN);
                bump(&mut self.counters.store_backoffs);
            }
            _ => {
                // BAD_REQUEST ya da beklenmeyen durum: imzasız, yalnız ipucu; T3 kendi kadansıyla.
                bump(&mut self.counters.store_backoffs);
                let a = &mut self.attempts[i];
                a.next_send = a.next_send.max(now + BACKOFF_MIN);
            }
        }
    }

    /// K3: atanmış `h`'yi bekleyen tutulan RESPONSE varsa `HandleAssigned`'dan HEMEN SONRA ver.
    fn release_held(&mut self, i: usize, h: HubHandle) {
        if let Some(pos) = self.held.iter().position(|r| r.h == h) {
            if let Some(r) = self.held.remove(pos) {
                self.attempts[i].delivered_response = Some(r.bytes.clone());
                bump(&mut self.counters.responses_delivered);
                self.push_event(LinkEvent::Response {
                    hub_handle: h,
                    envelope_type: r.envelope_type,
                    bytes: r.bytes,
                });
            }
        }
    }

    fn on_deliver(&mut self, now: Instant, d: &s4::Deliver) {
        // `Deliver::decode` hub_handle ≠ 0 ve bilinen envelope_type'ı zaten denetledi.
        let h = HubHandle::from_raw(d.hub_handle);
        match d.envelope_type {
            ENVELOPE_SESSION_INIT => self.on_deliver_init(h, d.envelope_type, &d.envelope),
            ENVELOPE_SESSION_INIT_RESPONSE => {
                let sender = PeerRef::from_dest_hash(d.sender_dest_hash);
                self.on_deliver_response(now, h, sender, d.envelope_type, &d.envelope);
            }
            ENVELOPE_RELAY_OPEN => bump(&mut self.counters.deliver_relay_open),
            _ => bump(&mut self.counters.hub_frames_dropped),
        }
    }

    fn on_deliver_init(&mut self, h: HubHandle, envelope_type: u8, bytes: &[u8]) {
        if let Some(r) = self.responder.iter().find(|r| r.init == bytes) {
            if r.h != h {
                // Aynı INIT baytları farklı h ile: yok say (C4 init_handle_conflict).
                bump(&mut self.counters.init_handle_conflict);
                return;
            }
            // Aynı (h, INIT): yeni oturum açılmaz.
            if let Some(f) = r.response_frame.clone() {
                bump(&mut self.counters.init_duplicate_resend);
                self.send_or_queue_response(f);
            } else {
                bump(&mut self.counters.init_duplicate_pending);
            }
            return;
        }
        if self.responder.len() >= MAX_RESPONDER_RECORDS {
            self.responder.pop_front();
            bump(&mut self.counters.bounded_drops);
        }
        self.responder.push_back(ResponderRecord {
            h,
            init: bytes.to_vec(),
            response_frame: None,
        });
        bump(&mut self.counters.incoming_inits);
        self.push_event(LinkEvent::IncomingInit {
            hub_handle: h,
            envelope_type,
            bytes: bytes.to_vec(),
        });
    }

    fn on_deliver_response(
        &mut self,
        now: Instant,
        h: HubHandle,
        sender: PeerRef,
        envelope_type: u8,
        bytes: &[u8],
    ) {
        if let Some(a) = self.attempts.iter_mut().find(|a| a.assigned == Some(h)) {
            if a.delivered_response.as_deref() == Some(bytes) {
                bump(&mut self.counters.response_duplicate);
                return;
            }
            a.delivered_response = Some(bytes.to_vec());
            bump(&mut self.counters.responses_delivered);
            self.push_event(LinkEvent::Response {
                hub_handle: h,
                envelope_type,
                bytes: bytes.to_vec(),
            });
            return;
        }
        // Biten bir denemenin ya da bilinen (kurulmuş / bırakılmış) bir hattın yanıtı: geç (K9).
        if self.is_old_handle(h) {
            bump(&mut self.counters.late_response);
            return;
        }
        // Bu eşin açık denemesi (eş başına tek deneme; `send_init` eskisini sonlandırır).
        let Some(i) = self.attempts.iter().position(|a| a.peer == sender) else {
            bump(&mut self.counters.late_response);
            return;
        };
        match self.attempts[i].assigned {
            Some(_) => {
                // S4 C3 çelişkili handle: StoreResp h_1 dedi, aynı denemenin RESPONSE'u h_2 diyor.
                // "Yok say ve başarılı say" değil: deneme biter, farklı h oturuma yazılmaz.
                let a = self.take_attempt(i);
                bump(&mut self.counters.attempt_handle_conflict);
                self.peer_notice(now, a.peer, HubNotice::AttemptEnded { peer: a.peer });
            }
            None if self.attempts[i].sends > 0 => {
                // K3 (KK-H37): StoreResp henüz gelmedi; o denemenin StoreResp{OK, h}'si T3 ile gelir.
                if self.held.iter().any(|r| r.h == h && r.bytes == bytes) {
                    bump(&mut self.counters.response_duplicate);
                    return;
                }
                if self.held.len() >= MAX_HELD_RESPONSES {
                    self.held.pop_front();
                    bump(&mut self.counters.bounded_drops);
                }
                self.held.push_back(HeldResponse {
                    h,
                    sender,
                    envelope_type,
                    bytes: bytes.to_vec(),
                    at: now,
                });
                bump(&mut self.counters.response_held);
            }
            None => bump(&mut self.counters.late_response),
        }
    }

    /// `i`'inci denemenin eşinden, `h`'den farklı ve eski olmayan bir handle ile tutulmuş yanıt var mı.
    fn held_conflicts(&self, i: usize, h: HubHandle) -> bool {
        let peer = self.attempts[i].peer;
        self.held
            .iter()
            .any(|r| r.sender == peer && r.h != h && !self.is_old_handle(r.h))
    }

    fn is_old_handle(&self, h: HubHandle) -> bool {
        self.retired.contains(&h) || self.known_links.iter().any(|(k, _)| *k == h)
    }

    /// Denemeyi kaldırır; atanmış handle'ı emekli halkasına yazar (K9 ayrımı).
    fn take_attempt(&mut self, i: usize) -> InitAttempt {
        let a = self.attempts.remove(i);
        if let Some(h) = a.assigned {
            self.retire(h);
        }
        a
    }

    fn retire(&mut self, h: HubHandle) {
        if self.retired.contains(&h) {
            return;
        }
        if self.retired.len() >= MAX_RETIRED_HANDLES {
            self.retired.pop_front();
        }
        self.retired.push_back(h);
    }

    fn on_route_miss(&mut self, now: Instant, raw: u32) {
        let h = HubHandle::from_raw(raw);
        // [HLR] `h`'si atanmış açılış denemesi biter: C3'teki gibi `AttemptEnded` (niyet ya da
        // yenileme yeni INIT ile yeniden başlar). `HandleLost` kurulu hattındır; deneme için
        // verilseydi bileşim kökü yenilenecek oturum bulamaz, deneme sessizce ölürdü.
        let attempt = self
            .attempts
            .iter()
            .position(|a| a.assigned == Some(h))
            .map(|i| self.take_attempt(i).peer);
        let link = self
            .known_links
            .iter()
            .position(|(k, _)| *k == h)
            .and_then(|i| self.known_links.remove(i))
            .map(|(_, p)| p);
        self.retire(h);
        self.responder.retain(|r| r.h != h);
        if attempt.is_none() && link.is_none() {
            bump(&mut self.counters.route_miss_unknown);
            return;
        }
        bump(&mut self.counters.route_miss);
        if let Some(peer) = attempt {
            self.peer_notice(now, peer, HubNotice::AttemptEnded { peer });
        }
        if let Some(peer) = link {
            self.peer_notice(now, peer, HubNotice::HandleLost { peer, h });
        }
    }

    // ── zarf API'si ──────────────────────────────────────────────────────────────────────────

    /// INIT: `STORE_MESSAGE{0, 0x01, peer, env}` kuyruğu + T3 (C3). Bind öncesi kuyrukta bekler.
    /// Aynı eşin tamamlanmamış eski denemesi sonlandırılır (geç yanıtları yeni denemeye atanmaz).
    ///
    /// # Errors
    /// [`HubError::BadEnvelopeType`] · [`HubError::FrameTooLong`] · [`HubError::QueueFull`].
    pub fn send_init(
        &mut self,
        now: Instant,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, HubError> {
        if envelope_type != ENVELOPE_SESSION_INIT {
            return Err(HubError::BadEnvelopeType);
        }
        if bytes.len() > StoreMessageReq::MAX_ENVELOPE {
            return Err(HubError::FrameTooLong);
        }
        if let Some(i) = self.attempts.iter().position(|a| a.peer == peer) {
            self.take_attempt(i);
            bump(&mut self.counters.attempt_superseded);
        }
        if self.attempts.len() >= MAX_ATTEMPTS {
            return Err(HubError::QueueFull);
        }
        let request_id = self.ids.next();
        let frame = store_frame(request_id, 0, envelope_type, peer, bytes)?;
        let id = LinkAttempt(self.next_attempt);
        self.next_attempt = self.next_attempt.wrapping_add(1);
        self.attempts.push(InitAttempt {
            id,
            peer,
            request_id,
            frame,
            started: now,
            sends: 0,
            next_send: now,
            assigned: None,
            delivered_response: None,
        });
        self.on_tick(now);
        Ok(id)
    }

    /// RESPONSE: `STORE_MESSAGE{h, 0x02, peer, env}` (C4); baytlar `(h, INIT)` kaydına bağlanır
    /// (KK-H38). Bind beklerken kuyrukta bekler (≤ 16).
    ///
    /// # Errors
    /// [`HubError::BadEnvelopeType`] · [`HubError::FrameTooLong`] · [`HubError::QueueFull`].
    pub fn send_response(
        &mut self,
        now: Instant,
        peer: PeerRef,
        h: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), HubError> {
        if envelope_type != ENVELOPE_SESSION_INIT_RESPONSE || h.raw() == 0 {
            return Err(HubError::BadEnvelopeType);
        }
        if bytes.len() > StoreMessageReq::MAX_ENVELOPE {
            return Err(HubError::FrameTooLong);
        }
        if !self.bind.is_bound() && self.queued_responses.len() >= MAX_QUEUED_RESPONSES {
            return Err(HubError::QueueFull);
        }
        let request_id = self.ids.next();
        let frame = store_frame(request_id, h.raw(), envelope_type, peer, bytes)?;
        if let Some(r) = self
            .responder
            .iter_mut()
            .rev()
            .find(|r| r.h == h && r.response_frame.is_none())
        {
            r.response_frame = Some(frame.clone());
        } else {
            bump(&mut self.counters.response_unrecorded);
        }
        if self.response_store_ids.len() >= MAX_RESPONSE_STORE_IDS {
            self.response_store_ids.pop_front();
        }
        self.response_store_ids.push_back(request_id);
        self.remember_link(h, peer);
        self.send_or_queue_response(frame);
        self.on_tick(now);
        Ok(())
    }

    fn send_or_queue_response(&mut self, frame: Vec<u8>) {
        if self.bind.is_bound() {
            self.push_tx(frame);
        } else if self.queued_responses.len() < MAX_QUEUED_RESPONSES {
            self.queued_responses.push_back(frame);
        } else {
            bump(&mut self.counters.bounded_drops);
        }
    }

    /// Transport'a giden sıradaki zarf olayı (KK-H37, KK-H38 kurallarıyla).
    pub fn poll_event(&mut self) -> Option<LinkEvent> {
        self.events.pop_front()
    }

    /// Bileşim kökünden S1 `Ready` (KK-H36): `peer` için atanmış `h`'si EŞLEŞEN denemenin T3'ü durur.
    /// Eşleşmeyen (eski oturumun) `Ready`'si yeni denemeyi kapatmaz.
    ///
    /// [HLR] Eşleşmeyen `Ready`'nin `h`'si de kurulu hat olarak hatırlanır: denemesi bittikten
    /// sonra (yeniden bind, `{UNKNOWN_HANDLE, 0}`) RESPONSE'u boşalma için tutulmuş bir yenileme
    /// oturumunu o handle'da kurar; hatırlanmasaydı o hattın `ROUTE_MISS`'i `route_miss_unknown`
    /// olur, `HandleLost` verilmez, oturum yeni INIT'siz kalırdı. Fazladan hatırlanan bir handle'ın
    /// bedeli en çok bir `HandleLost`'tur (sınırı bileşim kökünde, S4 C5).
    pub fn peer_ready(&mut self, peer: PeerRef, h: HubHandle) {
        if let Some(i) = self
            .attempts
            .iter()
            .position(|a| a.peer == peer && a.assigned == Some(h))
        {
            self.take_attempt(i);
            bump(&mut self.counters.attempt_completed);
            self.remember_link(h, peer);
        } else if !self.known_links.iter().any(|&(k, p)| k == h && p == peer) {
            bump(&mut self.counters.ready_unmatched);
            self.remember_link(h, peer);
        }
    }

    /// [HLR] Terk edilen denemenin T3'ü durur (S4 C3: "bağlantı niyeti iptal edildi"; transport
    /// bekleyeni kapattı). Bildirim YOK (terk eden zaten biliyor); atanmış handle emekli halkasına
    /// yazılır (K9: geç yanıt ≠ çelişki). Bilinmeyen / bitmiş deneme etkisizdir.
    pub fn cancel_attempt(&mut self, id: LinkAttempt) {
        if let Some(i) = self.attempts.iter().position(|a| a.id == id) {
            self.take_attempt(i);
            bump(&mut self.counters.attempt_cancelled);
        }
    }

    // ── zaman ve çıkış ───────────────────────────────────────────────────────────────────────

    /// Gönderilecek sıradaki kontrol çerçevesi: `[0;4] ‖ frame_type ‖ gövde`. `HubEndpoint` mühürler
    /// (`seal(HubChannel, …)`) ve hub adresine yollar. Vadesi gelmiş zamanlayıcıları da işler.
    pub fn poll_transmit(&mut self, now: Instant) -> Option<Vec<u8>> {
        self.on_tick(now);
        let f = self.tx.pop_front()?;
        self.last_tx = now;
        bump(&mut self.counters.tx_frames);
        Some(f)
    }

    /// En yakın zamanlayıcı vadesi (yoksa `None`). Gönderim kuyruğu boş değilse sürücü
    /// `poll_transmit`'i zaten boşaltmış olmalıdır (sürücü sözleşmesi).
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let mut d = self.bind.next_deadline();
        let mut take = |t: Instant| d = Some(d.map_or(t, |x| x.min(t)));
        if let Some(seed) = self.bind.bound_nonce() {
            take(self.hb.next_deadline(self.last_tx, seed));
            for a in &self.attempts {
                take(a.next_send);
            }
        }
        for a in &self.attempts {
            take(a.started + ATTEMPT_MAX_AGE);
        }
        for r in &self.held {
            take(r.at + ATTEMPT_MAX_AGE);
        }
        for (_, at, pending) in &self.peer_notice {
            if !pending.is_empty() {
                take(*at + NOTICE_BACKOFF);
            }
        }
        d
    }

    /// Vadesi gelmiş zamanlayıcıları işler (bind tekrarı, heartbeat, yoklama, T3, deneme yaşı).
    pub fn on_tick(&mut self, now: Instant) {
        self.release_deferred_notices(now);
        self.expire(now);
        if !self.bind.is_bound() {
            let out = self.bind.on_tick(
                now,
                &mut BindDeps {
                    cfg: &self.cfg,
                    signer: self.signer.as_ref(),
                    entropy: self.entropy.as_mut(),
                    ids: &mut self.ids,
                    counters: &mut self.counters,
                },
            );
            if let Some(f) = out {
                self.push_tx(f);
            }
            return;
        }
        if let Some(seed) = self.bind.bound_nonce().copied() {
            match self.hb.on_tick(now, self.last_tx, &seed) {
                HbOutcome::Nothing => {}
                HbOutcome::Send(f) => {
                    bump(&mut self.counters.heartbeats_sent);
                    self.push_tx(f);
                }
                HbOutcome::Disconnected => {
                    self.on_disconnected(now);
                    return;
                }
            }
        }
        // T3: bağlıyken vadesi gelen INIT'ler bayt-özdeş gider.
        let mut due = Vec::new();
        for a in &mut self.attempts {
            if now >= a.next_send {
                due.push(a.frame.clone());
                if a.sends > 0 {
                    bump(&mut self.counters.init_retransmits);
                }
                a.next_send = now + t3_delay(a.sends);
                a.sends = a.sends.saturating_add(1);
                bump(&mut self.counters.init_stores_sent);
            }
        }
        for f in due {
            self.push_tx(f);
        }
    }

    fn on_disconnected(&mut self, now: Instant) {
        bump(&mut self.counters.disconnects);
        self.hb.reset();
        self.bind.restart(
            now,
            &mut BindDeps {
                cfg: &self.cfg,
                signer: self.signer.as_ref(),
                entropy: self.entropy.as_mut(),
                ids: &mut self.ids,
                counters: &mut self.counters,
            },
        );
        self.push_notice(HubNotice::Disconnected);
        self.on_tick(now);
    }

    fn expire(&mut self, now: Instant) {
        let mut i = 0;
        while i < self.attempts.len() {
            if now.saturating_duration_since(self.attempts[i].started) >= ATTEMPT_MAX_AGE {
                let a = self.take_attempt(i);
                bump(&mut self.counters.attempt_expired);
                self.peer_notice(now, a.peer, HubNotice::AttemptEnded { peer: a.peer });
            } else {
                i += 1;
            }
        }
        self.held
            .retain(|r| now.saturating_duration_since(r.at) < ATTEMPT_MAX_AGE);
    }

    /// Bir oturum datagramı hub'a gitti (`outbound = true`) ya da hub'dan geldi (`false`).
    /// Yalnız sessizlik ölçümü içindir (heartbeat vadesi, C9 yaşı); T3'ü DURDURMAZ (KK-H36).
    pub fn on_session_traffic(&mut self, now: Instant, h: HubHandle, outbound: bool) {
        let _ = h;
        if outbound {
            self.last_tx = self.last_tx.max(now);
        } else {
            self.last_rx = Some(now);
        }
    }

    /// Soket hatası bildirimi (HB2): bağlıysa aktif yoklamayı başlatır (KK-H41).
    pub fn on_socket_error(&mut self, now: Instant) {
        if self.bind.is_bound() {
            self.hb.start_probe(now);
            self.on_tick(now);
        }
    }

    /// Hub imzası doğrulanmış bind var mı.
    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.bind.is_bound()
    }

    /// Hub'dan gelen son datagramın (her tür) yaşı (C9 `last_heartbeat_age_s`).
    #[must_use]
    pub fn last_rx_age(&self, now: Instant) -> Option<Duration> {
        self.last_rx.map(|t| now.saturating_duration_since(t))
    }

    /// Bileşim köküne sıradaki bildirim.
    pub fn take_notice(&mut self) -> Option<HubNotice> {
        self.notices.pop_front()
    }

    /// Sayaçlar.
    #[must_use]
    pub fn counters(&self) -> HubCounters {
        self.counters
    }

    // ── sınırlı kuyruklar ────────────────────────────────────────────────────────────────────

    fn push_tx(&mut self, f: Vec<u8>) {
        if self.tx.len() >= MAX_TX_QUEUE {
            bump(&mut self.counters.bounded_drops);
            return;
        }
        self.tx.push_back(f);
    }

    fn push_event(&mut self, e: LinkEvent) {
        if self.events.len() >= MAX_EVENTS {
            bump(&mut self.counters.bounded_drops);
            return;
        }
        self.events.push_back(e);
    }

    fn push_notice(&mut self, n: HubNotice) {
        if self.notices.len() >= MAX_NOTICES {
            self.notices.pop_front();
            bump(&mut self.counters.bounded_drops);
        }
        self.notices.push_back(n);
    }

    /// Eş başına ≥ 5 s (KK-H39): aralık dolmadıysa bildirim ertelenir — `AttemptEnded` bir,
    /// kaybolan her hat için bir `HandleLost`; bir tür ötekinin yerini almaz ([`Deferred`]). Aralık
    /// dolduysa eşin ertelenmişleri de bu bildirimle birlikte hemen verilir (silinmez).
    fn peer_notice(&mut self, now: Instant, peer: PeerRef, n: HubNotice) {
        if let Some(i) = self.peer_notice.iter().position(|e| e.0 == peer) {
            let e = &mut self.peer_notice[i];
            if e.2.add(n) {
                bump(&mut self.counters.bounded_drops);
            }
            if now.saturating_duration_since(e.1) < NOTICE_BACKOFF {
                return;
            }
            e.1 = now;
            for due in e.2.take(peer) {
                self.push_notice(due);
            }
            return;
        }
        if self.peer_notice.len() >= MAX_NOTICES {
            // En eski, ertelenmişi olmayan girdiyi düşür; yoksa en eskiyi (ertelenmişleri verilir).
            let i = self
                .peer_notice
                .iter()
                .position(|e| e.2.is_empty())
                .unwrap_or(0);
            let (old_peer, _, mut old) = self.peer_notice.remove(i);
            for due in old.take(old_peer) {
                self.push_notice(due);
            }
            bump(&mut self.counters.bounded_drops);
        }
        self.peer_notice.push((peer, now, Deferred::default()));
        self.push_notice(n);
    }

    fn release_deferred_notices(&mut self, now: Instant) {
        let mut out = Vec::new();
        for e in &mut self.peer_notice {
            if !e.2.is_empty() && now.saturating_duration_since(e.1) >= NOTICE_BACKOFF {
                out.extend(e.2.take(e.0));
                e.1 = now;
            }
        }
        for n in out {
            self.push_notice(n);
        }
    }

    fn remember_link(&mut self, h: HubHandle, peer: PeerRef) {
        if let Some(i) = self.known_links.iter().position(|(k, _)| *k == h) {
            self.known_links.remove(i);
        }
        if self.known_links.len() >= MAX_KNOWN_LINKS {
            // [HLR] Önce, aynı eşin daha yeni bir hattı olan (yerini yenisi almış oturumun) en eski
            // girdisi düşer; ancak böyle girdi yoksa en eskisi. Eşin güncel hattı düşerse o hattın
            // ROUTE_MISS'i `HandleLost` üretemez (oturum yeni INIT'siz kalır).
            let superseded = (0..self.known_links.len()).find(|&i| {
                let p = self.known_links[i].1;
                p == peer || self.known_links.iter().skip(i + 1).any(|e| e.1 == p)
            });
            self.known_links.remove(superseded.unwrap_or(0));
            bump(&mut self.counters.bounded_drops);
        }
        self.known_links.push_back((h, peer));
    }
}

/// STORE_MESSAGE isteği (S4 §2.4(c)) — `kama_contracts::s4` codec'iyle (KK-H34).
fn store_frame(
    request_id: u32,
    hub_handle: u32,
    envelope_type: u8,
    peer: PeerRef,
    envelope: Vec<u8>,
) -> Result<Vec<u8>, HubError> {
    let req = StoreMessageReq {
        hub_handle,
        envelope_type,
        recipient_dest_hash: *peer.as_bytes(),
        envelope,
    };
    req.validate().map_err(|_| HubError::BadEnvelopeType)?;
    rpc_frame_checked(request_id, RpcBody::StoreMessageReq(req)).map_err(|_| HubError::FrameTooLong)
}

fn bump(c: &mut u64) {
    *c = c.saturating_add(1);
}

/// `EnvelopeLink` cephesinin zamanı. `EnvelopeLink` çağrıları zaman taşımaz; cephe onu buradan alır.
/// Üründe [`SystemHubClock`]; testte sahte saat.
pub trait HubClock: Send + Sync {
    fn now(&self) -> Instant;
}

/// `Instant::now()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemHubClock;

impl HubClock for SystemHubClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Transport'un `EnvelopeLink`'i: paylaşılan [`HubLink`] üstünde ince cephe.
#[derive(Clone)]
pub struct HubEnvelopeLink {
    link: Arc<Mutex<HubLink>>,
    clock: Arc<dyn HubClock>,
}

impl HubEnvelopeLink {
    /// Üretim saatiyle cephe.
    #[must_use]
    pub fn new(link: Arc<Mutex<HubLink>>) -> Self {
        Self::with_clock(link, Arc::new(SystemHubClock))
    }

    /// Verilen saatle cephe.
    #[must_use]
    pub fn with_clock(link: Arc<Mutex<HubLink>>, clock: Arc<dyn HubClock>) -> Self {
        Self { link, clock }
    }

    /// Paylaşılan çekirdek.
    #[must_use]
    pub fn shared(&self) -> &Arc<Mutex<HubLink>> {
        &self.link
    }

    fn with<T>(&self, f: impl FnOnce(&mut HubLink, Instant) -> T) -> T {
        let now = self.clock.now();
        let mut guard = self.link.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard, now)
    }
}

impl EnvelopeLink for HubEnvelopeLink {
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        self.with(|l, now| l.send_init(now, peer, envelope_type, bytes))
            .map_err(|_| LinkError)
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        self.with(|l, now| l.send_response(now, peer, hub_handle, envelope_type, bytes))
            .map_err(|_| LinkError)
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        self.with(|l, _| l.poll_event())
    }

    fn cancel_init(&mut self, attempt: LinkAttempt) {
        self.with(|l, _| l.cancel_attempt(attempt));
    }
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
