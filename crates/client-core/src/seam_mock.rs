//! `MockCrypto` — Transport testleri için sahte Crypto (A4.2; KM-SEAM-TC-001 §7). YALNIZ TEST.
//!
//! Gerçek kripto bitmeden Transport tarafı [`PacketProtection`] + [`SessionControl`]'e karşı
//! koşabilsin diye vardır. **GÜVENLİK İDDİASI YOK:** "tag" bir sağlama toplamıdır, gizlilik yoktur.
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
//! # Sahtenin MODELLEMEDİĞİ (bunlar gerçek implementasyonun ve A4.4'ün işi)
//!
//! Anahtar türetme · epoch 0→1 geçişi ve grace · provisional supersession / `Confirmed` · glare
//! tie-break · sertifika doğrulaması · tel formatı (sahtenin başlığı ccr §35.6 DEĞİLDİR).
//! `command()` kurulum komutlarında HER ZAMAN `Established` döner (sprint planı A4.2).

use std::collections::BTreeMap;

use crate::seam::{
    CloseReason, Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits,
    SealError, SealedPacketMeta, SealedUnit, SessionCommand, SessionControl, SessionEvent,
    SessionHandle, SessionResult,
};

/// ccr §35.15 KS-DOS-1: sahte-RESPONSE başına deneme tavanı (= istemci pending INIT tavanı).
pub const PENDING_RESPONSE_TRIAL_CAP: usize = 16;
/// Sahtenin eşzamanlı oturum tavanı (SR-AUDIT-001: sınırsız tablo yok).
pub const MOCK_MAX_SESSIONS: usize = 64;
/// Olay kuyruğu tavanı.
pub const MOCK_MAX_EVENTS: usize = 64;

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
/// (`Initiate{peer}` → `from_raw(peer)`; karşı uç aynı tutamacı `AcceptInit`'in zarfından okur).
#[derive(Debug, Default)]
pub struct MockCrypto {
    sessions: BTreeMap<SessionHandle, Lane>,
    hub: Lane,
    /// Bekleyen INIT belirteçleri (eski → yeni).
    pending: Vec<u64>,
    events: std::collections::VecDeque<SessionEvent>,
    /// Son `AcceptResponse`'ta yapılan deneme sayısı (K7 cap'inin gözlemlenebilir kanıtı).
    pub last_response_trials: usize,
}

impl MockCrypto {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `Initiate`'in ürettiği / `AcceptInit`'in beklediği sahte INIT zarfı.
    #[must_use]
    pub fn init_envelope(session: u64) -> Vec<u8> {
        session.to_be_bytes().to_vec()
    }

    /// Bir pending belirtecine karşılık gelen sahte RESPONSE zarfı.
    #[must_use]
    pub fn response_envelope(token: u64) -> Vec<u8> {
        token.to_be_bytes().to_vec()
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
        self.pending.push(token);
        Ok(())
    }

    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    fn establish(&mut self, h: SessionHandle) -> SessionResult {
        if !self.sessions.contains_key(&h) {
            if self.sessions.len() >= MOCK_MAX_SESSIONS {
                return SessionResult::Failed;
            }
            self.sessions.insert(h, Lane::default());
        }
        self.push_event(SessionEvent::Ready(h));
        SessionResult::Established { session: h }
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
        // Sayaç SARMAZ; u64::MAX hiçbir zaman mühürlenmez.
        lane.next_send_pn = pn.checked_add(1).ok_or(SealError::CapReached)?;
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
                dh_epoch: 0,
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
            // Kurulum komutları: HER ZAMAN `Established` (tablo doluysa `Failed`).
            SessionCommand::Initiate { peer, .. } => {
                self.establish(SessionHandle::from_raw(peer.to_raw_for_mock()))
            }
            SessionCommand::AcceptInit { wire } => match <[u8; 8]>::try_from(wire.as_slice()) {
                Ok(b) => self.establish(SessionHandle::from_raw(u64::from_be_bytes(b))),
                Err(_) => SessionResult::Failed,
            },
            // K7: en yeni önce, en fazla cap deneme; hangi adayın tuttuğu DIŞARIYA sızmaz.
            SessionCommand::AcceptResponse { wire } => {
                let want = <[u8; 8]>::try_from(wire.as_slice())
                    .ok()
                    .map(u64::from_be_bytes);
                let mut trials = 0usize;
                let mut hit = None;
                for (i, tok) in self.pending.iter().enumerate().rev() {
                    if trials == PENDING_RESPONSE_TRIAL_CAP {
                        break;
                    }
                    trials += 1;
                    if Some(*tok) == want {
                        hit = Some(i);
                        break;
                    }
                }
                self.last_response_trials = trials;
                match hit {
                    Some(i) => {
                        let tok = self.pending.remove(i);
                        self.establish(SessionHandle::from_raw(tok))
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
            SessionCommand::Close { session } => {
                if self.sessions.remove(&session).is_some() {
                    self.push_event(SessionEvent::Closed(session, CloseReason::Renewed));
                }
                SessionResult::NoOutput
            }
        }
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }
}
