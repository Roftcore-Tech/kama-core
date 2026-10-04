//! Hedef katmanı — sözleşme süitinin "hangi implementasyona karşı koşuyorum" sorusu.
//!
//! Google SWE ch.13'ün contract-test kuralı ("the same tests run against both the real
//! implementation and the fake") derleme seviyesinde ancak iki hedef AYNI trait'i sağlarsa
//! tutar. Bu dosya o hedef soyutlamasını ve bugünkü tek hedefi (sahte) verir; süitin kendisi
//! [`super::contract`] içinde.
//!
//! # Bugünkü durum: tek hedef
//!
//! `mock` kayıtlı, `real` DEĞİL — ve bu **beyan edilmiş bir boşluktur** ([`GAP_REAL_ADAPTER`]),
//! sessiz bir atlama değil. `scripts/ci/seam-conformance.sh --target real` bugün çıkış kodu 3
//! döner; `seam_real_adapter` süiti boşluğun BEYAN EDİLDİĞİNİ sınar. Gerçek adapter kaydedildiği
//! an o sınama kırmızıya döner — kapsam sessizce değişemez.

#![allow(dead_code)]

use std::collections::VecDeque;

use super::api::{
    AbandonReason, CompressionPolicy, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy,
    KamaTransport, PeerRef, PreparedUnit, SessionEvent, SessionHandle, TransportCapabilities,
    TransportError, UnitBytes,
};

/// Gerçek adapter'ın yokluğunun kimliği. Runner ve `seam_real_adapter` bu sabiti paylaşır.
pub const GAP_REAL_ADAPTER: &str = "GAP-SEAM-REAL-1";

/// Hedef ailesi. `Mock` = sahte modül (geliştirme sırasında), `Real` = gerçek implementasyon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Mock,
    Real,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Mock => "mock",
            TargetKind::Real => "real",
        }
    }
}

/// Hedef kurulamadı — **typed**, panik değil, `todo!()` değil (CLAUDE.md §6).
///
/// Bu tip bilerek [`TransportError`] DEĞİLDİR: "implementasyon henüz yok" bir harness durumudur,
/// sözleşmenin §3 hata kümesine ait değildir. İkisini karıştırmak, uygulanmamış bir yolu
/// sözleşmenin bir davranışıymış gibi gösterirdi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TargetUnavailable {
    #[error("hedef uygulanmadı ({gap})")]
    NotImplemented { gap: &'static str },
}

/// Sözleşme süitinin parametresi: hedefi kuran fabrika.
pub trait SeamTarget {
    /// Süitin karşı koştuğu somut tip.
    type Transport: KamaTransport;
    /// Runner ve çıktı satırlarında görünen kimlik (`mock` / `real`).
    const ID: &'static str;
    /// Hedef ailesi — kategori sızıntısı sınamalarında kullanılır.
    const KIND: TargetKind;

    /// # Errors
    /// Hedef bu derlemede mevcut değilse [`TargetUnavailable`] (typed; beyan edilmiş boşluk).
    fn build() -> Result<Self::Transport, TargetUnavailable>;

    /// Hedefin iç ilerlemesini bir adım sürer; varsayılan: hiçbir şey yapmaz.
    ///
    /// **Bu kanca ilk sözleşme satırını (H-ROUNDTRIP) yazarken doğdu** ve walking-skeleton'un
    /// asıl getirisi budur: sahte, `connect()` sonrası `Ready`'yi aynı çağrı içinde kuyruğa
    /// koyar, gerçek adapter ise socket okumadan ilerleyemez. Kanca olmasaydı süit sahte-biçimli
    /// olurdu (`poll_event`'i döngüde çağırmak gerçek hedefte asla ilerlemezdi) ve bunu ancak
    /// gerçek hedefi yazarken, bütün satırları yeniden şekillendirerek öğrenirdik.
    fn pump(_transport: &mut Self::Transport) {}
}

/// Kayıtlı hedefler — `seam_real_adapter` bu listeye karşı "gerçek hedef yok" iddiasını sınar.
pub const REGISTERED_TARGETS: &[(&str, TargetKind)] = &[(MockTarget::ID, TargetKind::Mock)];

// ── Sahte hedef ──────────────────────────────────────────────────────────────────────────────

/// KM-SEAM-AT-001 §7'nin tam `MockTransport`'u DEĞİLDİR.
///
/// §7 deterministik kayıp/sırasız/duplicate enjeksiyonu, olay senaryoları ve
/// `ConnectAbandoned`/`ConnectProgress` enjeksiyonu ister (`GAP-SEAM-MOCK-7`). Buradaki sahte, walking-skeleton'un gerektirdiği en
/// ince dilimdir: **peer'ı LOOPBACK modeller** — gönderilen her birim aynı oturumda geri teslim
/// edilir. Loopback bir sözleşme iddiası değil, echo modelidir; H-ROUNDTRIP'in ölçtüğü şey
/// transport'un byte'lara DOKUNMAMASIDIR (çerçeve sızıntısı, kırpma, sıkıştırma yok) ve bu
/// iddia gerçek hedefte aynı testle, gerçek peer üzerinden ölçülür.
///
/// Sınırlar (SR-AUDIT-001, bounds-before-alloc): hiçbir kuyruk sınırsız büyümez; dolduğunda
/// typed `Backpressure` döner — sessiz düşürme YOK.
pub struct EchoMockTransport {
    caps: TransportCapabilities,
    next_id: u64,
    sessions: Vec<SessionRec>,
    events: VecDeque<SessionEvent>,
    inbox: VecDeque<Delivered>,
}

/// Eşzamanlı oturum tavanı (identity multi-device limit=1 ile karıştırılmamalı; bu yalnız
/// sahtenin bellek sınırıdır).
pub const MOCK_MAX_SESSIONS: usize = 4;
/// Teslim kuyruğu tavanı.
pub const MOCK_MAX_INBOX: usize = 64;
/// Olay kuyruğu tavanı.
pub const MOCK_MAX_EVENTS: usize = 64;

/// Sahtenin taşıdığı kapasite değerleri — **normatif değil**.
///
/// Gerçek değerler K5 zincirinden türeyecek (`Crypto conservative_limits → Transport bütçesi →
/// TransportCapabilities`). Süit bu sayılara BAĞLANMAZ: hedefin kendi `capabilities()`'ini okur.
pub const STUB_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Bkz. [`STUB_MAX_PLAINTEXT_UNIT_BYTES`] — normatif değil.
pub const STUB_PREFERRED_PLAINTEXT_UNIT_BYTES: u32 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    Pending,
    Active,
}

struct SessionRec {
    peer: PeerRef,
    handle: SessionHandle,
    ticket: ConnectTicket,
    state: SessionState,
    /// K6: `early_unit` yalnız yeni handshake'te tüketilir; `Ready` yayınlanınca teslim edilir.
    early: Option<UnitBytes>,
}

impl EchoMockTransport {
    pub fn new(caps: TransportCapabilities) -> Self {
        Self {
            caps,
            next_id: 1,
            sessions: Vec::new(),
            events: VecDeque::new(),
            inbox: VecDeque::new(),
        }
    }

    fn find_by_peer(&self, peer: PeerRef) -> Option<&SessionRec> {
        self.sessions.iter().find(|s| s.peer == peer)
    }

    fn activate(&mut self, handle: SessionHandle) {
        if let Some(rec) = self.sessions.iter_mut().find(|s| s.handle == handle) {
            rec.state = SessionState::Active;
        }
    }

    /// `Ready` sonrası bekleyen early birimi teslim kuyruğuna koyar (yer varsa).
    ///
    /// Yer yoksa birim REC'te bekler — sessizce düşürülmez (K6: "X sessizce düşürülmez").
    fn drain_early(&mut self, handle: SessionHandle) {
        if self.inbox.len() >= MOCK_MAX_INBOX {
            return;
        }
        let Some(rec) = self.sessions.iter_mut().find(|s| s.handle == handle) else {
            return;
        };
        if let Some(bytes) = rec.early.take() {
            self.inbox.push_back(Delivered {
                session: handle,
                unit: bytes,
            });
        }
    }
}

impl KamaTransport for EchoMockTransport {
    fn capabilities(&self) -> TransportCapabilities {
        self.caps
    }

    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError> {
        // K6 / KARAR-1: aktif ya da pending peer'a early_unit ile gelmek yapısal hatadır;
        // hiçbir byte tüketilmez.
        if let Some(rec) = self.find_by_peer(peer) {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }
            return Ok(match rec.state {
                SessionState::Active => ConnectResult::Ready(rec.handle),
                SessionState::Pending => ConnectResult::Pending(rec.ticket),
            });
        }

        if self.sessions.len() >= MOCK_MAX_SESSIONS || self.events.len() >= MOCK_MAX_EVENTS {
            return Err(TransportError::Backpressure);
        }

        let early = match early_unit {
            Some(u) => {
                if u.bytes.len() > self.caps.max_plaintext_unit_bytes as usize {
                    return Err(TransportError::UnitTooLarge);
                }
                Some(u.bytes)
            }
            None => None,
        };

        let id = self.next_id;
        self.next_id += 1;
        let handle = SessionHandle::from_raw(id);
        let ticket = ConnectTicket::from_raw(id);
        self.sessions.push(SessionRec {
            peer,
            handle,
            ticket,
            state: SessionState::Pending,
            early,
        });
        self.events.push_back(SessionEvent::Ready(handle, peer));
        Ok(ConnectResult::Pending(ticket))
    }

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        _policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        // K4: CompressionPolicy Transport davranışını DEĞİŞTİRMEZ — burada da okunmaz.
        debug_assert!(matches!(
            unit.compression_policy,
            CompressionPolicy::Never | CompressionPolicy::AppCompressed
        ));
        let active = self
            .sessions
            .iter()
            .any(|s| s.handle == session && s.state == SessionState::Active);
        if !active {
            return Err(TransportError::UnknownSession);
        }
        if unit.bytes.len() > self.caps.max_plaintext_unit_bytes as usize {
            return Err(TransportError::UnitTooLarge);
        }
        if self.inbox.len() >= MOCK_MAX_INBOX {
            return Err(TransportError::Backpressure);
        }
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
        let ev = self.events.pop_front()?;
        if let SessionEvent::Ready(h, _) = ev {
            self.activate(h);
            self.drain_early(h);
        }
        Some(ev)
    }

    fn close(&mut self, session: SessionHandle) {
        // Olay yayınlanmıyor. `CloseReason::UserClosed` artık var (küme 2026-10-05'te kapandı), ama
        // S1 Application'ın KENDİ `close()` çağrısının bir `Closed` olayı üretip üretmediğini
        // söylemiyor (§1 yalnız imzayı verir; §6 H-EVENTS `Closed` sonrasını tanımlar). Spec'in
        // söylemediği bir olayı sahteye koymaktansa üretmemek dürüst olan.
        self.sessions.retain(|s| s.handle != session);
    }

    fn cancel(&mut self, ticket: ConnectTicket) {
        // DR-CONNECT-1 H9 (§4): yalnız PENDING niyet iptal edilir; aktif ya da bilinmeyen ticket
        // için hiçbir şey olmaz (ticket `Ready` ile zaten tüketilmiştir).
        let Some(pos) = self
            .sessions
            .iter()
            .position(|s| s.ticket == ticket && s.state == SessionState::Pending)
        else {
            return;
        };
        // `early_unit` teslim EDİLMEZ: kayıt, taşıdığı early birimle birlikte düşer.
        let handle = self.sessions.remove(pos).handle;
        // Kuyruktaki `Ready(handle, _)` geri çekilir; yerine tek olay girer, kuyruk büyümez.
        self.events
            .retain(|e| !matches!(e, SessionEvent::Ready(h, _) if *h == handle));
        self.events.push_back(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::Cancelled,
        ));
    }
}

/// Sahte hedef kaydı.
pub struct MockTarget;

impl SeamTarget for MockTarget {
    type Transport = EchoMockTransport;
    const ID: &'static str = "mock";
    const KIND: TargetKind = TargetKind::Mock;

    fn build() -> Result<Self::Transport, TargetUnavailable> {
        Ok(EchoMockTransport::new(TransportCapabilities {
            max_plaintext_unit_bytes: STUB_MAX_PLAINTEXT_UNIT_BYTES,
            preferred_plaintext_unit_bytes: STUB_PREFERRED_PLAINTEXT_UNIT_BYTES,
        }))
    }
}
