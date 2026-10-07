//! `MockCrypto` — Transport testleri için sahte Crypto (A4.2; KM-SEAM-TC-001 §7). YALNIZ TEST
//! (`test-util` özelliği; ürün derlemesinde bu modül YOKTUR).
//!
//! Gerçek kripto bitmeden Transport tarafı [`PacketProtection`] + [`SessionControl`]'e karşı
//! koşabilsin diye vardır. **GÜVENLİK İDDİASI YOK:** "tag" bir sağlama toplamıdır, gizlilik yoktur.
//!
//! # Neden bu crate'te
//!
//! Sahte, gerçeğinin sahibi olan crate'te yaşar (sahip kararı 2026-10-05): gerçek Crypto burada
//! yazılacak ve S3 sözleşme süiti (`tests/noise_conformance_core`) ikisini AYNI gövdelerle sınar.
//! Tüketicisi Transport'tur (`client-core`); onu dev-dependency ile, bu crate'in `test-util`
//! özelliğini açarak alır. Ters yön (`kama-noise-wire → client-core`) katman kuralıyla yasaktır.
//!
//! # Sahtenin SADIK kaldığı sözleşme (Transport'u yanıltmaması gereken yerler)
//!
//! * **K1** — bağlam çağırandan gelir; `Session` ve `HubChannel` AYRI PN uzayıdır.
//! * **K2** — PN'i sahte sayar; `meta.full_wire_pn` bağlam başına ardışıktır ve ASLA sarmaz.
//! * **K5** — `open` başarısızlığı sessizdir (`None`) ve HİÇBİR state'i ilerletmez (bozuk tag,
//!   bilinmeyen oturum, replay hepsi aynı görünür).
//! * `sealed_len` KESİNDİR: `seal(ctx, f)?.bytes.len() == sealed_len(ctx, f.len())?`.
//! * `open` replay'i reddeder (aynı PN ikinci kez teslim edilmez) — gerçek seam de öyle; kabul eden
//!   bir sahte, Transport'un duplicate testlerini anlamsız kılardı.
//! * **K7** — `AcceptResponse` pending kümesini EN YENİ ÖNCE ve en fazla
//!   [`PENDING_RESPONSE_TRIAL_CAP`] deneme ile tarar; sonuç opaktır (`Failed`).
//!
//! * **H-RETRY (2026-10-06)** — `Initiate` deterministik sahte INIT zarfı üretir (`init_counter`
//!   her çağrıda artar → ikinci `Initiate` FARKLI byte); `AcceptInit` `init_counter`'a göre sahte
//!   SSR1 üretir ve ÖNBELLEKLER: aynı `(oturum, init_counter)` ile tekrar gelen INIT → byte-özdeş
//!   aynı SSR1, yeni oturum YOK, yeni olay YOK.
//! * **s3 v1.1 (2026-10-08)** — `Initiate` → `Initiated{session, …}` (bekleyen oturumun
//!   tutamacı); sahte INIT başlatanın kimliğini ([`MockCrypto::with_identity`]) taşır ve
//!   `AcceptInit` onu `EstablishedAndReply.peer` olarak verir; `AssignHandle` oturum ↔ hub
//!   tutamacı bağını tutar (tekrar idempotent; çakışma / yeniden bağlama / bilinmeyen oturum
//!   `Failed`, state değişmez); `Close` bekleyen INIT'leri ve bağı da siler, bilinen tutamaçta tam
//!   bir `Closed(_, UserClosed)` yayınlar.
//! * **K1 kayıt kancası** — `seal()`'e verilen bağlam kaydedilir ([`MockCrypto::last_seal_ctx`]).
//! * **PN sürekliliği** — [`MockCrypto::mock_set_epoch`] yalnız `meta.dh_epoch`'u değiştirir;
//!   sayaç SIFIRLANMAZ (noise-wire §4 EPOCH-PN-INIT, 2026-10-06).
//!
//! # Sahtenin MODELLEMEDİĞİ (bunlar gerçek implementasyonun işi)
//!
//! Anahtar türetme · grace penceresi ve epoch-0 discard · provisional supersession / `Confirmed` ·
//! glare tie-break · sertifika doğrulaması (`peer` doğrulanmaz, INIT'te yazılanıdır) · `Renew`'un
//! yeni oturumu (sahtede `Renew` `RenewRequired` yayınlar ve `Established{aynı}` döner) · tel
//! formatı (sahtenin zarfları ccr §35.3/§35.5/§35.6 DEĞİLDİR). Kurulum komutları tablo dolmadıkça
//! başarılıdır: `Initiate` → `Initiated`, `AcceptInit` → `EstablishedAndReply`, `AcceptResponse` →
//! `Established` (cap'li aday denemesi).

use std::collections::{BTreeMap, VecDeque};

use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s3::{
    HubHandle, Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits,
    SealError, SealedPacketMeta, SealedUnit, SessionCommand, SessionControl, SessionEvent,
    SessionResult,
};

/// ccr §35.15 KS-DOS-1: sahte-RESPONSE başına deneme tavanı (= istemci pending INIT tavanı).
pub const PENDING_RESPONSE_TRIAL_CAP: usize = 16;
/// Sahtenin eşzamanlı oturum tavanı (SR-AUDIT-001: sınırsız tablo yok).
pub const MOCK_MAX_SESSIONS: usize = 64;
/// Olay kuyruğu tavanı.
pub const MOCK_MAX_EVENTS: usize = 64;
/// app-rpc `STORE_MESSAGE.envelope_type` (AMENDMENT-7): INIT / INIT_RESPONSE.
pub const ENVELOPE_INIT: u8 = 0x01;
pub const ENVELOPE_INIT_RESPONSE: u8 = 0x02;

const INIT_MAGIC: [u8; 8] = *b"MOCKSSI1";
const RESP_MAGIC: [u8; 8] = *b"MOCKSSR1";
/// magic(8) + oturum(8) + init_counter(8) + başlatanın `dest_hash`'i(32); ardından opak
/// `early_unit`.
const INIT_HDR: usize = 56;
/// magic(8) + oturum(8) + init_counter(8) + yanıtlayan-tekilliği(8).
const RESP_LEN: usize = 32;

const TAG_CTX_HUB: u8 = 0x00;
const TAG_CTX_SESSION: u8 = 0x01;
/// ctx(1) + handle(8) + pn(8)
const HDR: usize = 17;
const TAG: usize = 16;
const OVERHEAD: usize = HDR + TAG;
const MAX_UDP_PAYLOAD: usize = 65_507;
const REPLAY_BITS: u64 = 64;

/// Bir bağlamın gönderim sayacı + alım replay durumu.
#[derive(Debug, Default, Clone, Copy)]
struct Lane {
    next_send_pn: u64,
    /// Kabul edilen en yüksek PN + 1 (0 = hiç kabul yok).
    recv_next: u64,
    /// `recv_next - 1 - i` PN'inin görülüp görülmediği (bit i).
    recv_seen: u64,
    /// `meta.dh_epoch` olarak raporlanan değer; sayacı ETKİLEMEZ.
    epoch: u8,
}

impl Lane {
    /// Saf karar: `pn` kabul edilebilir mi (state DEĞİŞTİRMEZ).
    fn can_accept(&self, pn: u64) -> bool {
        if pn >= self.recv_next {
            return pn != u64::MAX;
        }
        let back = self.recv_next - 1 - pn;
        back < REPLAY_BITS && self.recv_seen & (1u64 << back) == 0
    }

    /// Yalnız `can_accept` doğruysa çağrılır.
    fn commit(&mut self, pn: u64) {
        if pn >= self.recv_next {
            let shift = pn - self.recv_next + 1;
            self.recv_seen = if shift >= REPLAY_BITS {
                0
            } else {
                self.recv_seen << shift
            };
            self.recv_seen |= 1;
            self.recv_next = pn + 1;
        } else {
            self.recv_seen |= 1u64 << (self.recv_next - 1 - pn);
        }
    }
}

/// Sahte Crypto. Endpoint başına bir örnek; iki uç aynı tutamaç değerlerini paylaşır
/// (`Initiate{peer}` → [`MockCrypto::session_of`]`(peer)`; karşı uç aynı tutamacı `AcceptInit`'in
/// zarfından okur).
#[derive(Debug, Default)]
pub struct MockCrypto {
    /// Bu ucun kimliği — yalnız sahte INIT'e yazılır (`None` = sıfır `dest_hash`).
    identity: Option<PeerRef>,
    sessions: BTreeMap<SessionHandle, Lane>,
    /// `AssignHandle` bağları (oturum → hub tutamacı; ≤ oturum sayısı).
    hub_handles: BTreeMap<SessionHandle, HubHandle>,
    hub: Lane,
    /// Bekleyen INIT'ler `(oturum, init_counter)` (eski → yeni).
    pending: Vec<(u64, u64)>,
    /// Sonraki `Initiate`'in `init_counter`'ı.
    next_init_counter: u64,
    /// Her TAZE `AcceptInit`'te artar (yanıtlayan ephemeral'ının yerine geçer): SSR1 yeniden
    /// ÜRETİLSEYDİ byte'ları farklı olurdu; önbellek bunu engeller.
    next_responder_nonce: u64,
    /// `(oturum, init_counter)` → önbellekli SSR1 (ccr INV-RESP-KEY-1).
    response_cache: BTreeMap<(u64, u64), Vec<u8>>,
    events: VecDeque<SessionEvent>,
    /// Son `AcceptResponse`'ta yapılan deneme sayısı (K7 cap'inin gözlemlenebilir kanıtı).
    pub last_response_trials: usize,
    /// `seal()`'e verilen bağlamların kaydı (K1 kancası; sınırlı).
    seal_ctx_log: VecDeque<ProtectionContext>,
}

impl MockCrypto {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Kimliği belli bir uç: `Initiate`'in sahte INIT'i bu kimliği taşır, karşı ucun
    /// `EstablishedAndReply.peer`'i bu olur.
    #[must_use]
    pub fn with_identity(identity: PeerRef) -> Self {
        Self {
            identity: Some(identity),
            ..Self::default()
        }
    }

    /// Bu ucun kimliği (verilmediyse sıfır `dest_hash`).
    #[must_use]
    pub fn identity(&self) -> PeerRef {
        self.identity
            .unwrap_or(PeerRef::from_dest_hash([0u8; 32]))
    }

    /// `AssignHandle` ile oturuma bağlanmış hub tutamacı.
    #[must_use]
    pub fn hub_handle_of(&self, session: SessionHandle) -> Option<HubHandle> {
        self.hub_handles.get(&session).copied()
    }

    /// Sahtenin bir eşe bastığı oturum tutamacının ham değeri: `dest_hash`'in ilk sekiz baytı
    /// (big-endian). Deterministiktir; iki uç aynı eş için aynı tutamaca varır.
    #[must_use]
    pub fn session_of(peer: PeerRef) -> u64 {
        let h = peer.to_raw_for_mock();
        u64::from_be_bytes([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]])
    }

    /// `Initiate`'in ürettiği / `AcceptInit`'in beklediği sahte INIT zarfı (`early_unit`'siz);
    /// `initiator` = INIT'i gönderen ucun kimliği.
    #[must_use]
    pub fn init_envelope(session: u64, init_counter: u64, initiator: PeerRef) -> Vec<u8> {
        let mut w = Vec::with_capacity(INIT_HDR);
        w.extend_from_slice(&INIT_MAGIC);
        w.extend_from_slice(&session.to_be_bytes());
        w.extend_from_slice(&init_counter.to_be_bytes());
        w.extend_from_slice(&initiator.to_raw_for_mock());
        w
    }

    fn ssr1(session: u64, init_counter: u64, nonce: u64) -> Vec<u8> {
        let mut w = Vec::with_capacity(RESP_LEN);
        w.extend_from_slice(&RESP_MAGIC);
        w.extend_from_slice(&session.to_be_bytes());
        w.extend_from_slice(&init_counter.to_be_bytes());
        w.extend_from_slice(&nonce.to_be_bytes());
        w
    }

    /// [`MockCrypto::add_pending`] ile kaydedilen bir pending'e karşılık gelen sahte RESPONSE zarfı.
    #[must_use]
    pub fn response_envelope(token: u64) -> Vec<u8> {
        Self::ssr1(token, 0, 0)
    }

    fn parse_init(wire: &[u8]) -> Option<(u64, u64, PeerRef)> {
        if wire.len() < INIT_HDR || wire.len() > MAX_UDP_PAYLOAD || wire[..8] != INIT_MAGIC {
            return None;
        }
        Some((
            u64::from_be_bytes(wire[8..16].try_into().ok()?),
            u64::from_be_bytes(wire[16..24].try_into().ok()?),
            PeerRef::from_dest_hash(wire[24..INIT_HDR].try_into().ok()?),
        ))
    }

    fn parse_response(wire: &[u8]) -> Option<(u64, u64)> {
        if wire.len() != RESP_LEN || wire[..8] != RESP_MAGIC {
            return None;
        }
        Some((
            u64::from_be_bytes(wire[8..16].try_into().ok()?),
            u64::from_be_bytes(wire[16..24].try_into().ok()?),
        ))
    }

    /// `seal()`'e en son verilen bağlam (K1: bağlam çağırandan gelir, seam'e DEĞİŞMEDEN ulaşır).
    #[must_use]
    pub fn last_seal_ctx(&self) -> Option<ProtectionContext> {
        self.seal_ctx_log.back().copied()
    }

    /// Bir oturumun raporlanan `dh_epoch`'unu değiştirir. Sayaç SIFIRLANMAZ (PN epoch'lar boyunca
    /// sürekli). Oturum yoksa `false`.
    pub fn mock_set_epoch(&mut self, session: SessionHandle, epoch: u8) -> bool {
        match self.sessions.get_mut(&session) {
            Some(lane) => {
                lane.epoch = epoch & 1;
                true
            }
            None => false,
        }
    }

    /// Oturum/replay/pending/önbellek/olay state'inin parmak izi. Salt-gözlem alanları
    /// (`last_response_trials`, bağlam kaydı) DIŞARIDADIR — H-NO-MUTATE karşılaştırmaları için.
    #[must_use]
    pub fn state_fingerprint(&self) -> String {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}|{:?}",
            self.sessions,
            self.hub_handles,
            self.hub,
            self.pending,
            self.next_init_counter,
            self.next_responder_nonce,
            self.response_cache,
            self.events
        )
    }

    /// Yanıt beklenen bir INIT kaydeder (`AcceptResponse` cap testleri için).
    ///
    /// # Errors
    /// Pending kümesi [`PENDING_RESPONSE_TRIAL_CAP`]'te — yeni kayıt reddedilir (zincir kuralı:
    /// trial cap = pending tavanı; küme cap'ten büyük olamaz).
    pub fn add_pending(&mut self, token: u64) -> Result<(), SealError> {
        if self.pending.len() >= PENDING_RESPONSE_TRIAL_CAP {
            return Err(SealError::CapReached);
        }
        self.pending.push((token, 0));
        Ok(())
    }

    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Oturum yoksa kurar ve `Ready` yayınlar; varsa DOKUNMAZ (olay yok). Tablo doluysa `false`.
    fn ensure_session(&mut self, h: SessionHandle) -> bool {
        if self.sessions.contains_key(&h) {
            return true;
        }
        if self.sessions.len() >= MOCK_MAX_SESSIONS {
            return false;
        }
        self.sessions.insert(h, Lane::default());
        self.push_event(SessionEvent::Ready(h));
        true
    }

    fn push_event(&mut self, ev: SessionEvent) {
        if self.events.len() < MOCK_MAX_EVENTS {
            self.events.push_back(ev);
        }
    }

    fn lane(&self, ctx: ProtectionContext) -> Option<&Lane> {
        match ctx {
            ProtectionContext::HubChannel => Some(&self.hub),
            ProtectionContext::Session(h) => self.sessions.get(&h),
        }
    }

    fn lane_mut(&mut self, ctx: ProtectionContext) -> Option<&mut Lane> {
        match ctx {
            ProtectionContext::HubChannel => Some(&mut self.hub),
            ProtectionContext::Session(h) => self.sessions.get_mut(&h),
        }
    }

    const LIMITS: ProtectionLimits = ProtectionLimits {
        wire_overhead_bytes: OVERHEAD as u32,
        max_plaintext_bytes: (MAX_UDP_PAYLOAD - OVERHEAD) as u32,
    };
}

/// 128-bit sağlama (FNV-1a iki tohumla). KRİPTOGRAFİK DEĞİLDİR; yalnız "byte bozulursa `open`
/// düşer" davranışını deterministik olarak üretir.
fn checksum(parts: &[&[u8]]) -> [u8; TAG] {
    let mut out = [0u8; TAG];
    for (i, seed) in [0xcbf2_9ce4_8422_2325u64, 0x8422_2325_cbf2_9ce4u64]
        .into_iter()
        .enumerate()
    {
        let mut h = seed;
        for p in parts {
            for &b in *p {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        out[i * 8..i * 8 + 8].copy_from_slice(&h.to_be_bytes());
    }
    out
}

fn header(ctx: ProtectionContext, pn: u64) -> [u8; HDR] {
    let mut h = [0u8; HDR];
    match ctx {
        ProtectionContext::HubChannel => h[0] = TAG_CTX_HUB,
        ProtectionContext::Session(s) => {
            h[0] = TAG_CTX_SESSION;
            h[1..9].copy_from_slice(&raw(s).to_be_bytes());
        }
    }
    h[9..17].copy_from_slice(&pn.to_be_bytes());
    h
}

/// `SessionHandle` Transport için opaktır; ham değeri yalnız basan taraf (Crypto / sahtesi) görür.
fn raw(h: SessionHandle) -> u64 {
    h.to_raw_for_mock()
}

impl PacketProtection for MockCrypto {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        if frame.len() > Self::LIMITS.max_plaintext_bytes as usize {
            return Err(SealError::FrameTooLarge);
        }
        let lane = self.lane_mut(ctx).ok_or(SealError::UnknownSession)?;
        let pn = lane.next_send_pn;
        let dh_epoch = lane.epoch;
        // Sayaç SARMAZ; u64::MAX hiçbir zaman mühürlenmez.
        lane.next_send_pn = pn.checked_add(1).ok_or(SealError::CapReached)?;
        if self.seal_ctx_log.len() >= MOCK_MAX_EVENTS {
            self.seal_ctx_log.pop_front();
        }
        self.seal_ctx_log.push_back(ctx);
        let hdr = header(ctx, pn);
        let tag = checksum(&[&hdr, frame]);
        let mut bytes = Vec::with_capacity(OVERHEAD + frame.len());
        bytes.extend_from_slice(&hdr);
        bytes.extend_from_slice(frame);
        bytes.extend_from_slice(&tag);
        let sealed_len = bytes.len() as u32;
        Ok(SealedUnit {
            bytes,
            meta: SealedPacketMeta {
                full_wire_pn: pn,
                dh_epoch,
                sealed_len,
            },
        })
    }

    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        if wire.len() < OVERHEAD || wire.len() > MAX_UDP_PAYLOAD {
            return None;
        }
        let (hdr, rest) = wire.split_at(HDR);
        let (frame, tag) = rest.split_at(rest.len() - TAG);
        let ctx = match hdr[0] {
            TAG_CTX_HUB if hdr[1..9] == [0u8; 8] => ProtectionContext::HubChannel,
            TAG_CTX_SESSION => ProtectionContext::Session(SessionHandle::from_raw(
                u64::from_be_bytes(hdr[1..9].try_into().ok()?),
            )),
            _ => return None,
        };
        let pn = u64::from_be_bytes(hdr[9..17].try_into().ok()?);
        // Sıra gerçek seam'le aynı: precheck (iz bırakmaz) → doğrulama → commit.
        let lane = self.lane(ctx)?;
        if !lane.can_accept(pn) {
            return None;
        }
        if checksum(&[hdr, frame]) != *tag {
            return None;
        }
        self.lane_mut(ctx)?.commit(pn);
        Some(Opened {
            ctx,
            frame: frame.to_vec(),
            meta: SealedPacketMeta {
                full_wire_pn: pn,
                dh_epoch: 0,
                sealed_len: wire.len() as u32,
            },
        })
    }

    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        self.lane(ctx).ok_or(SealError::UnknownSession)?;
        if plaintext_len > Self::LIMITS.max_plaintext_bytes as usize {
            return Err(SealError::FrameTooLarge);
        }
        Ok(plaintext_len + OVERHEAD)
    }

    fn conservative_limits(&self, _class: ProtectionClass) -> ProtectionLimits {
        Self::LIMITS
    }

    fn limits(&self, _ctx: ProtectionContext) -> ProtectionLimits {
        Self::LIMITS
    }
}

impl SessionControl for MockCrypto {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        match cmd {
            // INIT: deterministik sahte zarf; `init_counter` her çağrıda artar. Pending tavanı
            // doluysa reddedilir ve HİÇBİR state değişmez (ccr §35.15 zinciri).
            SessionCommand::Initiate { peer, early_unit } => {
                let session = Self::session_of(peer);
                let h = SessionHandle::from_raw(session);
                let early = early_unit.unwrap_or_default();
                if self.pending.len() >= PENDING_RESPONSE_TRIAL_CAP
                    || early.len() > Self::LIMITS.max_plaintext_bytes as usize
                    || self.next_init_counter == u64::MAX
                    || !self.ensure_session(h)
                {
                    return SessionResult::Failed;
                }
                let init_counter = self.next_init_counter;
                self.next_init_counter += 1;
                self.pending.push((session, init_counter));
                let mut bytes = Self::init_envelope(session, init_counter, self.identity());
                bytes.extend_from_slice(&early);
                SessionResult::Initiated {
                    session: h,
                    envelope_type: ENVELOPE_INIT,
                    bytes,
                }
            }
            // Aynı `(oturum, init_counter)` → önbellekteki SSR1 byte-özdeş; yeni oturum/olay YOK.
            SessionCommand::AcceptInit { wire } => {
                let Some((session, init_counter, peer)) = Self::parse_init(&wire) else {
                    return SessionResult::Failed;
                };
                let h = SessionHandle::from_raw(session);
                if let Some(cached) = self.response_cache.get(&(session, init_counter)) {
                    return SessionResult::EstablishedAndReply {
                        session: h,
                        peer,
                        envelope_type: ENVELOPE_INIT_RESPONSE,
                        bytes: cached.clone(),
                    };
                }
                if self.response_cache.len() >= MOCK_MAX_SESSIONS
                    || self.next_responder_nonce == u64::MAX
                    || !self.ensure_session(h)
                {
                    return SessionResult::Failed;
                }
                let nonce = self.next_responder_nonce;
                self.next_responder_nonce += 1;
                let bytes = Self::ssr1(session, init_counter, nonce);
                self.response_cache
                    .insert((session, init_counter), bytes.clone());
                SessionResult::EstablishedAndReply {
                    session: h,
                    peer,
                    envelope_type: ENVELOPE_INIT_RESPONSE,
                    bytes,
                }
            }
            // [s3 v1.1] Oturum ↔ hub tutamacı bağı. Her ret yolunda state DEĞİŞMEZ.
            SessionCommand::AssignHandle {
                session,
                hub_handle,
            } => {
                if !self.sessions.contains_key(&session) {
                    return SessionResult::Failed;
                }
                match self.hub_handles.get(&session) {
                    Some(bound) if *bound == hub_handle => return SessionResult::NoOutput,
                    Some(_) => return SessionResult::Failed,
                    None => {}
                }
                if self.hub_handles.values().any(|bound| *bound == hub_handle) {
                    return SessionResult::Failed;
                }
                self.hub_handles.insert(session, hub_handle);
                SessionResult::NoOutput
            }
            // K7: en yeni önce, en fazla cap deneme; hangi adayın tuttuğu DIŞARIYA sızmaz.
            SessionCommand::AcceptResponse { wire } => {
                let want = Self::parse_response(&wire);
                let mut trials = 0usize;
                let mut hit = None;
                for (i, cand) in self.pending.iter().enumerate().rev() {
                    if trials == PENDING_RESPONSE_TRIAL_CAP {
                        break;
                    }
                    trials += 1;
                    if Some(*cand) == want {
                        hit = Some(i);
                        break;
                    }
                }
                self.last_response_trials = trials;
                match hit {
                    Some(i) => {
                        let h = SessionHandle::from_raw(self.pending[i].0);
                        if !self.ensure_session(h) {
                            return SessionResult::Failed;
                        }
                        self.pending.remove(i);
                        SessionResult::Established { session: h }
                    }
                    // Başarısızlıkta pending kümesi DEĞİŞMEZ (H-NO-MUTATE).
                    None => SessionResult::Failed,
                }
            }
            SessionCommand::Renew { session } => {
                if !self.sessions.contains_key(&session) {
                    return SessionResult::Failed;
                }
                self.push_event(SessionEvent::RenewRequired(session));
                SessionResult::Established { session }
            }
            // [s3 v1.1] Kurulu VE bekleyen oturumu kapatır: bekleyen INIT'ler ve hub bağı da
            // silinir (geç gelen RESPONSE `Failed`). Bilinen tutamaçta tam bir `Closed`.
            SessionCommand::Close { session } => {
                let raw_session = raw(session);
                let pending_before = self.pending.len();
                self.pending.retain(|(s, _)| *s != raw_session);
                let had_pending = self.pending.len() != pending_before;
                let had_session = self.sessions.remove(&session).is_some();
                self.hub_handles.remove(&session);
                if had_session || had_pending {
                    // `Close` komutu yerel bir kapatma isteğidir.
                    self.push_event(SessionEvent::Closed(session, CloseReason::UserClosed));
                }
                SessionResult::NoOutput
            }
        }
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }
}
