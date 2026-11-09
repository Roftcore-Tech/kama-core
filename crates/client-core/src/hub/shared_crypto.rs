//! [S2-A3-2a / HB2] Hub kipinin TEK-AÇMA katmanı: [`HubCrypto`] ve sahipli kayıt kanalı
//! [`OpenedStash`] (tasarım notu `docs/design/HB2-ALIM-YOLU.md`, seçenek (a); KK-H52).
//!
//! # Neden
//!
//! Hub kipinde bir oturumun iki rolü (istemci ucu ve veri alıcısı) aynı `hub_handle` ile TEK
//! soketten gelir; rol yalnız AEAD'den sonra, çerçeve tipiyle bilinir. Açma replay penceresini
//! işler: datagram TAM BİR KEZ açılmalıdır. `HubEndpoint` datagramı bir kez açar ve açılmış kaydı
//! bu kanala TAŞIR; görevin `seam.open` çağrısı ([`HubCrypto::open`]) kaydı tam bayt eşleşmesiyle
//! ALIR, kripto ikinci kez çağrılmaz.
//!
//! # Sahiplik şartı (KK-H52 sahip kararı 2026-11-10)
//!
//! "Açılmış kayıt tüketici kabulüne kadar sahipliğini korur; kanal dolması, tüketici durması ve
//! oturum kapanması yollarında sessizce kaybolmaz." Bu modülde:
//!
//! * **Önce yer, sonra açma:** [`OpenedStash::reserve`] yuva ayırmadan `HubEndpoint` açmaz. Kanal
//!   doluysa datagram AÇILMADAN düşer ([`StashCounters::backpressure`]); kripto durumu değişmez.
//!   Kanal dolması kaybı YOKTUR.
//! * **Kabul = tüketim:** kayıt yalnız [`OpenedStash::take`] ile çıkar. Zil (yerel UDP) sahiplik
//!   taşımaz; tüketilmemiş kayıt [`OpenedStash::due_rings`] ile yeniden çalınır.
//! * **Tüketimsiz çıkış iki yoldur ve sayılır** ([`StashCounters::lost`] = `hub_stash_miss`):
//!   `Closed(session)` ([`OpenedStash::drop_session`]; [`HubCrypto::poll_event`] olayı geçirirken
//!   çağırır) ve ölü tüketici ([`StashConfig::dead_consumer_ttl`]).
//!
//! # Kilit sırası
//!
//! Transport dış kilidi (`Mutex<HubCrypto<C>>`) tutarken [`HubCrypto`] iç kripto kilidini YA DA
//! kanal kilidini alır, ikisini birlikte TUTMAZ. `HubEndpoint` dış kilidi hiç almaz ve iç kripto
//! ile kanal kilidini sırayla (birini bırakıp) alır.
//!
//! # Doğrudan kip
//!
//! [`HubCrypto::Direct`] saf geçiştir: aynı çağrılar aynı sırayla, ek kilit yok, sınır kırpması yok.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit, SessionCommand, SessionControl, SessionEvent, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;

use super::endpoint::MAX_DATAGRAM;

/// Kilitleme zehirlenmeye karşı panik yapmaz (transport.rs `lock` ile aynı kural).
fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ── Kayıt kanalı ─────────────────────────────────────────────────────────────────────────────

/// Kanal sınırları — `[KALİBRASYON]`, normatif DEĞİL (SR-AUDIT-001: sınırlı tampon).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StashConfig {
    /// Bir `hub_handle` için bekleyen (ayrılmış + dolu) kayıt tavanı. Duran tek bir tüketici
    /// yalnız kendi oturumunu durdurur.
    pub per_handle_cap: usize,
    /// Toplam kayıt tavanı.
    pub total_cap: usize,
    /// Tüketilmemiş kaydın zilinin yeniden çalınma aralığı.
    pub ring_interval: Duration,
    /// Oturum kapanmadan bu süre tüketilmeyen kayıt "ölü tüketici" sayılır ve düşer.
    pub dead_consumer_ttl: Duration,
}

impl Default for StashConfig {
    fn default() -> Self {
        Self {
            per_handle_cap: 32,
            total_cap: 256,
            ring_interval: Duration::from_millis(20),
            dead_consumer_ttl: Duration::from_secs(10),
        }
    }
}

/// Kanal sayaçları (salt-okunur anlık görüntü).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StashCounters {
    /// Kanala taşınan açılmış kayıt.
    pub inserted: u64,
    /// Tüketicinin aldığı kayıt (kabul).
    pub taken: u64,
    /// Yuva yok → datagram AÇILMADAN düştü (kripto durumu değişmedi).
    pub backpressure: u64,
    /// Eşleşmeyen `seam.open` (yinelenen zil ya da yabancı yerel datagram); kripto çağrılmadı.
    pub doorbell_dup: u64,
    /// Yeniden çalınan zil.
    pub rings: u64,
    /// `Closed(session)` ile tüketimsiz düşen kayıt.
    pub dropped_closed: u64,
    /// Ölü tüketici (`dead_consumer_ttl`) ile tüketimsiz düşen kayıt.
    pub dropped_expired: u64,
}

impl StashCounters {
    /// Tüketimsiz çıkan açılmış kayıt toplamı (`hub_stash_miss`).
    #[must_use]
    pub fn lost(&self) -> u64 {
        self.dropped_closed + self.dropped_expired
    }
}

/// [`OpenedStash::reserve`]'ün verdiği yuva: [`OpenedStash::fill`] ya da
/// [`OpenedStash::cancel`] ile kapatılmalıdır.
#[derive(Debug)]
#[must_use = "ayrılmış yuva fill ya da cancel ile kapatılmalı"]
pub struct Reservation {
    wire_handle: u32,
}

struct Record {
    wire_handle: u32,
    session: SessionHandle,
    bytes: Vec<u8>,
    opened: Opened,
    dest: SocketAddr,
    created: Instant,
    last_ring: Instant,
}

/// Açılmış kayıtların sınırlı, sahipli kanalı (bkz. modül belgesi).
pub struct OpenedStash {
    cfg: StashConfig,
    records: VecDeque<Record>,
    /// `hub_handle` → (dolu + ayrılmış) kayıt sayısı.
    per_handle: HashMap<u32, usize>,
    /// Ayrılmış ama henüz dolmamış yuva sayısı.
    reserved: usize,
    counters: StashCounters,
}

impl std::fmt::Debug for OpenedStash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Bayt ve çerçeve Debug'a GİRMEZ; yalnız sayımlar.
        f.debug_struct("OpenedStash")
            .field("records", &self.records.len())
            .field("reserved", &self.reserved)
            .field("counters", &self.counters)
            .finish_non_exhaustive()
    }
}

impl OpenedStash {
    /// Boş kanal.
    #[must_use]
    pub fn new(cfg: StashConfig) -> Self {
        Self {
            cfg,
            records: VecDeque::new(),
            per_handle: HashMap::new(),
            reserved: 0,
            counters: StashCounters::default(),
        }
    }

    /// Yapılandırma.
    #[must_use]
    pub fn config(&self) -> StashConfig {
        self.cfg
    }

    /// Dolu kayıt sayısı.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Kanal boş mu (ayrılmış yuva da yok).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.reserved == 0
    }

    /// Sayaçlar.
    #[must_use]
    pub fn counters(&self) -> StashCounters {
        self.counters
    }

    /// Açmadan ÖNCE yuva ayırır. `None` → yer yok: çağıran datagramı AÇMADAN düşürür (sayaç
    /// burada artar).
    pub fn reserve(&mut self, wire_handle: u32) -> Option<Reservation> {
        let used = self.records.len() + self.reserved;
        let mine = self.per_handle.get(&wire_handle).copied().unwrap_or(0);
        if used >= self.cfg.total_cap || mine >= self.cfg.per_handle_cap {
            self.counters.backpressure += 1;
            return None;
        }
        *self.per_handle.entry(wire_handle).or_insert(0) += 1;
        self.reserved += 1;
        Some(Reservation { wire_handle })
    }

    /// Ayrılmış yuvayı açılmış kayıtla doldurur. Kayıt artık kanalındır.
    pub fn fill(
        &mut self,
        r: Reservation,
        session: SessionHandle,
        bytes: Vec<u8>,
        opened: Opened,
        dest: SocketAddr,
        now: Instant,
    ) {
        self.reserved = self.reserved.saturating_sub(1);
        self.records.push_back(Record {
            wire_handle: r.wire_handle,
            session,
            bytes,
            opened,
            dest,
            created: now,
            last_ring: now,
        });
        self.counters.inserted += 1;
    }

    /// Ayrılmış yuvayı kayıt olmadan bırakır (açma başarısız, bağlam uyumsuz, tip bilinmez …).
    pub fn cancel(&mut self, r: Reservation) {
        self.reserved = self.reserved.saturating_sub(1);
        self.release_handle(r.wire_handle);
    }

    fn release_handle(&mut self, wire_handle: u32) {
        if let Some(n) = self.per_handle.get_mut(&wire_handle) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.per_handle.remove(&wire_handle);
            }
        }
    }

    /// Kabul: baytları TAM eşleşen kaydı kanaldan alır. Eşleşme yoksa `None` (`doorbell_dup`).
    pub fn take(&mut self, wire: &[u8]) -> Option<Opened> {
        let Some(i) = self.records.iter().position(|r| r.bytes == wire) else {
            self.counters.doorbell_dup += 1;
            return None;
        };
        let rec = self.records.remove(i)?;
        self.release_handle(rec.wire_handle);
        self.counters.taken += 1;
        Some(rec.opened)
    }

    /// Zili yeniden çalınacak kayıtlar (hedef, baytlar) ve ölü tüketicinin kayıtlarının düşmesi.
    pub fn due_rings(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let ttl = self.cfg.dead_consumer_ttl;
        let before = self.records.len();
        let mut expired = Vec::new();
        self.records.retain(|r| {
            let dead = now.saturating_duration_since(r.created) >= ttl;
            if dead {
                expired.push(r.wire_handle);
            }
            !dead
        });
        self.counters.dropped_expired += (before - self.records.len()) as u64;
        for h in expired {
            self.release_handle(h);
        }
        let interval = self.cfg.ring_interval;
        let mut out = Vec::new();
        for r in &mut self.records {
            if now.saturating_duration_since(r.last_ring) >= interval {
                r.last_ring = now;
                out.push((r.dest, r.bytes.clone()));
            }
        }
        self.counters.rings += out.len() as u64;
        out
    }

    /// Oturum kapandı: kayıtları tüketimsiz düşer (sayılır). Döner: düşen kayıt sayısı.
    pub fn drop_session(&mut self, session: SessionHandle) -> usize {
        let mut gone = Vec::new();
        self.records.retain(|r| {
            let hit = r.session == session;
            if hit {
                gone.push(r.wire_handle);
            }
            !hit
        });
        for h in &gone {
            self.release_handle(*h);
        }
        self.counters.dropped_closed += gone.len() as u64;
        gone.len()
    }
}

// ── HubCrypto ────────────────────────────────────────────────────────────────────────────────

/// Hub kipinin sınır kırpması (KK-H54): görev 1280 B'yi aşan datagram üretemesin.
fn clamp(l: ProtectionLimits) -> ProtectionLimits {
    let ceiling = (MAX_DATAGRAM as u32).saturating_sub(l.wire_overhead_bytes);
    ProtectionLimits {
        wire_overhead_bytes: l.wire_overhead_bytes,
        max_plaintext_bytes: l.max_plaintext_bytes.min(ceiling),
    }
}

/// Hub kipinde transport'un tuttuğu yarı: iç kripto + kanal.
pub struct HubShared<C> {
    inner: Arc<Mutex<C>>,
    stash: Arc<Mutex<OpenedStash>>,
}

/// S3 kripto sarmalayıcısı (bkz. modül belgesi).
pub enum HubCrypto<C> {
    /// Doğrudan kip: saf geçiş.
    Direct(C),
    /// Hub kipi: tek-açma kanalı.
    Hub(HubShared<C>),
}

impl<C> std::fmt::Debug for HubCrypto<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Direct(_) => "HubCrypto::Direct",
            Self::Hub(_) => "HubCrypto::Hub",
        })
    }
}

/// `HubEndpoint`'in kriptoya ve kanala erişimi (dış transport kilidi YOK).
pub trait HubCryptoAccess: Send + Sync {
    /// Datagramı TEK kez açar (iç kripto kilidi kısa süre).
    fn open(&self, wire: &[u8]) -> Option<Opened>;
    /// Kontrol çerçevesini `HubChannel` bağlamında mühürler (`‖ 0×16`; KK-H53).
    ///
    /// # Errors
    /// Kriptonun yapısal reddi.
    fn seal_hub(&self, frame: &[u8]) -> Result<SealedUnit, SealError>;
    /// Kayıt kanalı.
    fn stash(&self) -> &Arc<Mutex<OpenedStash>>;
}

/// [`HubCryptoAccess`]'in somut hâli ([`HubCrypto::hub`] verir).
pub struct HubCryptoTap<C> {
    inner: Arc<Mutex<C>>,
    stash: Arc<Mutex<OpenedStash>>,
}

impl<C: PacketProtection> HubCryptoAccess for HubCryptoTap<C> {
    fn open(&self, wire: &[u8]) -> Option<Opened> {
        lock(&self.inner).open(wire)
    }
    fn seal_hub(&self, frame: &[u8]) -> Result<SealedUnit, SealError> {
        lock(&self.inner).seal(ProtectionContext::HubChannel, frame)
    }
    fn stash(&self) -> &Arc<Mutex<OpenedStash>> {
        &self.stash
    }
}

impl<C: PacketProtection + SessionControl> HubCrypto<C> {
    /// Doğrudan kip.
    #[must_use]
    pub fn direct(crypto: C) -> Self {
        Self::Direct(crypto)
    }

    /// Hub kipi: sarmalayıcı (transport'a) + erişim ucu (`HubEndpoint`'e).
    #[must_use]
    pub fn hub(crypto: C, cfg: StashConfig) -> (Self, HubCryptoTap<C>) {
        let inner = Arc::new(Mutex::new(crypto));
        let stash = Arc::new(Mutex::new(OpenedStash::new(cfg)));
        let tap = HubCryptoTap {
            inner: Arc::clone(&inner),
            stash: Arc::clone(&stash),
        };
        (Self::Hub(HubShared { inner, stash }), tap)
    }

    /// Hub kipinde mi.
    #[must_use]
    pub fn is_hub(&self) -> bool {
        matches!(self, Self::Hub(_))
    }
}

impl<C: PacketProtection> PacketProtection for HubCrypto<C> {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        match self {
            Self::Direct(c) => c.seal(ctx, frame),
            Self::Hub(s) => {
                let mut c = lock(&s.inner);
                if frame.len() > clamp(c.limits(ctx)).max_plaintext_bytes as usize {
                    return Err(SealError::FrameTooLarge);
                }
                c.seal(ctx, frame)
            }
        }
    }

    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        match self {
            Self::Direct(c) => c.open(wire),
            // TEK AÇMA: kripto burada ÇAĞRILMAZ; açılmış kayıt yalnız kanaldan alınır.
            Self::Hub(s) => lock(&s.stash).take(wire),
        }
    }

    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        match self {
            Self::Direct(c) => c.sealed_len(ctx, plaintext_len),
            Self::Hub(s) => {
                let c = lock(&s.inner);
                if plaintext_len > clamp(c.limits(ctx)).max_plaintext_bytes as usize {
                    return Err(SealError::FrameTooLarge);
                }
                c.sealed_len(ctx, plaintext_len)
            }
        }
    }

    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        match self {
            Self::Direct(c) => c.conservative_limits(class),
            Self::Hub(s) => clamp(lock(&s.inner).conservative_limits(class)),
        }
    }

    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        match self {
            Self::Direct(c) => c.limits(ctx),
            Self::Hub(s) => clamp(lock(&s.inner).limits(ctx)),
        }
    }
}

impl<C: SessionControl> SessionControl for HubCrypto<C> {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        match self {
            Self::Direct(c) => c.command(cmd),
            Self::Hub(s) => lock(&s.inner).command(cmd),
        }
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        match self {
            Self::Direct(c) => c.poll_event(),
            Self::Hub(s) => {
                // İç kilit bırakıldıktan SONRA kanal kilidi (ikisi birlikte tutulmaz).
                let ev = lock(&s.inner).poll_event();
                if let Some(SessionEvent::Closed(session, _)) = ev {
                    lock(&s.stash).drop_session(session);
                }
                ev
            }
        }
    }
}

impl HubCrypto<NoiseWireProtection> {
    /// [`NoiseWireProtection::session_send_epoch`]'in geçişi (salt-okunur; T4).
    #[must_use]
    pub fn session_send_epoch(&self, session: SessionHandle) -> Option<u8> {
        match self {
            Self::Direct(c) => c.session_send_epoch(session),
            Self::Hub(s) => lock(&s.inner).session_send_epoch(session),
        }
    }
}

// ── Testler ──────────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kama_contracts::handles::PeerRef;
    use kama_contracts::s3::{HubHandle, SealedPacketMeta};
    use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
    use kama_noise_wire::session_control::EnvelopeKind;

    const ALICE: u8 = 0xA1;
    const BOB: u8 = 0xB0;
    const HUB: HubHandle = HubHandle::from_raw(0x7E57_0AB0);

    fn id(n: u8) -> PeerRef {
        let mut d = [0u8; 32];
        d[0] = 0x4B;
        d[1] = n;
        PeerRef::from_dest_hash(d)
    }

    fn node(me: u8) -> NoiseWireProtection {
        let dir = fixture_directory([ALICE, BOB].map(|n| *id(n).as_bytes())).expect("dizin");
        fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0
    }

    /// Aynı el sıkışma dizisini iki kriptoya (`a` başlatır) uygular; iki uca aynı hub tutamacı.
    fn handshake<A, B>(a: &mut A, b: &mut B) -> (SessionHandle, SessionHandle)
    where
        A: SessionControl,
        B: SessionControl,
    {
        let SessionResult::Initiated {
            session: ha,
            bytes: init,
            ..
        } = a.command(SessionCommand::Initiate {
            peer: id(BOB),
            early_unit: None,
        })
        else {
            panic!("Initiate");
        };
        let SessionResult::EstablishedAndReply {
            session: hb,
            bytes: resp,
            ..
        } = b.command(SessionCommand::AcceptInit { wire: init })
        else {
            panic!("AcceptInit");
        };
        assert_eq!(
            a.command(SessionCommand::AcceptResponse { wire: resp }),
            SessionResult::Established { session: ha }
        );
        for r in [
            a.command(SessionCommand::AssignHandle {
                session: ha,
                hub_handle: HUB,
            }),
            b.command(SessionCommand::AssignHandle {
                session: hb,
                hub_handle: HUB,
            }),
        ] {
            assert_eq!(r, SessionResult::NoOutput);
        }
        while a.poll_event().is_some() {}
        while b.poll_event().is_some() {}
        (ha, hb)
    }

    fn opened(session: SessionHandle, frame: &[u8]) -> Opened {
        Opened {
            ctx: ProtectionContext::Session(session),
            frame: frame.to_vec(),
            meta: SealedPacketMeta {
                full_wire_pn: 0,
                dh_epoch: 0,
                sealed_len: 0,
            },
        }
    }

    fn addr() -> SocketAddr {
        "127.0.0.1:9".parse().expect("adres")
    }

    fn small() -> StashConfig {
        StashConfig {
            per_handle_cap: 2,
            total_cap: 3,
            ring_interval: Duration::from_millis(20),
            dead_consumer_ttl: Duration::from_secs(10),
        }
    }

    /// `HubCrypto::Direct` saf geçiştir: aynı girdi dizisiyle çıplak `NoiseWireProtection` ile
    /// aynı mühür uzunluklarını, aynı açma sonuçlarını ve aynı sınırları verir. (Mühür baytları
    /// el sıkışmanın OS entropisiyle oturum başına farklıdır; bayt eşitliği aynı oturumun iki
    /// ucunda sınanır: Direct'in mühürlediğini çıplak uç açar, tersi de.)
    #[test]
    fn direct_is_a_pure_passthrough() {
        let mut bare = node(ALICE);
        let mut wrapped = HubCrypto::direct(node(BOB));
        let (ha, hb) = handshake(&mut bare, &mut wrapped);
        for class in [ProtectionClass::Session, ProtectionClass::HubChannel] {
            assert_eq!(
                wrapped.conservative_limits(class),
                node(BOB).conservative_limits(class)
            );
        }
        assert_eq!(
            wrapped.limits(ProtectionContext::Session(hb)),
            bare.limits(ProtectionContext::Session(ha))
        );
        for i in 0..8u8 {
            let frame = vec![i; 1 + usize::from(i) * 50];
            let s = wrapped
                .seal(ProtectionContext::Session(hb), &frame)
                .expect("mühür");
            assert_eq!(
                s.bytes.len(),
                wrapped
                    .sealed_len(ProtectionContext::Session(hb), frame.len())
                    .expect("boy")
            );
            let o = bare.open(&s.bytes).expect("çıplak uç açar");
            assert_eq!(o.frame, frame);
            assert_eq!(o.ctx, ProtectionContext::Session(ha));
            // Replay: aynı datagram ikinci kez açılmaz (geçiş kriptonun kuralını değiştirmez).
            assert!(bare.open(&s.bytes).is_none());
            let back = bare
                .seal(ProtectionContext::Session(ha), &frame)
                .expect("mühür");
            let o2 = wrapped.open(&back.bytes).expect("sarmalı uç açar");
            assert_eq!(o2.frame, frame);
            assert_eq!(o2.meta, back.meta);
            assert!(wrapped.open(&back.bytes).is_none());
        }
        // HubChannel: bayt özdeş (`frame ‖ 0×16`).
        let frame = [0u8, 0, 0, 0, 0x1B];
        let w = wrapped
            .seal(ProtectionContext::HubChannel, &frame)
            .expect("hub");
        let b = bare
            .seal(ProtectionContext::HubChannel, &frame)
            .expect("hub");
        assert_eq!(w.bytes, b.bytes);
        assert_eq!(w.meta, b.meta);
        assert!(wrapped.session_send_epoch(hb).is_some(), "epoch geçişi");
    }

    /// Hub kipi: kripto datagramı tap üzerinden BİR kez açar; transport'un `open`'ı kaydı
    /// kanaldan alır, kriptoyu çağırmaz (ikinci `open` → `None`, kripto durumu değişmez).
    #[test]
    fn hub_mode_opens_once_and_hands_the_record_over() {
        let mut alice = node(ALICE);
        let (mut hubc, tap) = HubCrypto::hub(node(BOB), StashConfig::default());
        let (ha, hb) = handshake(&mut alice, &mut hubc);
        let wire = alice
            .seal(ProtectionContext::Session(ha), b"data")
            .expect("mühür")
            .bytes;
        let r = lock(tap.stash()).reserve(HUB.raw()).expect("yer");
        let o = tap.open(&wire).expect("tek açma");
        assert_eq!(o.ctx, ProtectionContext::Session(hb));
        lock(tap.stash()).fill(r, hb, wire.clone(), o, addr(), Instant::now());
        // İkinci gerçek açma replay ile reddedilir: kayıt kanaldan gelmeli.
        assert!(tap.open(&wire).is_none(), "kripto ikinci açmayı reddeder");
        let got = hubc.open(&wire).expect("kanaldan kabul");
        assert_eq!(got.frame, b"data");
        assert!(hubc.open(&wire).is_none(), "yinelenen zil");
        let c = lock(tap.stash()).counters();
        assert_eq!(
            (c.inserted, c.taken, c.doorbell_dup, c.lost()),
            (1, 1, 1, 0)
        );
    }

    /// Taşma: yuva yoksa `reserve` `None` döner ve çağıran AÇMAZ → kripto durumu değişmez.
    #[test]
    fn overflow_rejects_before_open_and_crypto_state_is_unchanged() {
        let mut alice = node(ALICE);
        let (mut hubc, tap) = HubCrypto::hub(node(BOB), small());
        let (ha, hb) = handshake(&mut alice, &mut hubc);
        let mut stash = lock(tap.stash());
        for i in 0..2u8 {
            let r = stash.reserve(HUB.raw()).expect("yer");
            stash.fill(r, hb, vec![i], opened(hb, &[i]), addr(), Instant::now());
        }
        assert!(stash.reserve(HUB.raw()).is_none(), "tutamaç tavanı");
        assert_eq!(stash.counters().backpressure, 1);
        // Başka tutamaç için toplam tavanda bir yer var, sonra o da dolar.
        let r = stash.reserve(0x0101_0101).expect("toplam tavanda yer");
        stash.cancel(r);
        let r = stash.reserve(0x0101_0101).expect("iptal yeri geri verir");
        stash.fill(r, hb, vec![9], opened(hb, &[9]), addr(), Instant::now());
        assert!(stash.reserve(0x0202_0202).is_none(), "toplam tavan");
        drop(stash);
        // Reddedilen datagram açılmadı: aynı datagram yer açılınca HÂLÂ açılabilir.
        let wire = alice
            .seal(ProtectionContext::Session(ha), b"later")
            .expect("mühür")
            .bytes;
        assert!(lock(tap.stash()).reserve(HUB.raw()).is_none());
        // Tüketici bir kaydı kabul eder → tutamaçta yer açılır.
        assert_eq!(hubc.open(&[0u8]).expect("kabul").frame, [0u8]);
        let r = lock(tap.stash()).reserve(HUB.raw()).expect("yer açıldı");
        assert!(
            tap.open(&wire).is_some(),
            "reddedilen datagram kriptoya hiç gitmemişti"
        );
        lock(tap.stash()).cancel(r);
        assert_eq!(lock(tap.stash()).counters().lost(), 0);
    }

    /// Duran tüketici: kayıt kanalda kalır, zil yeniden çalınır; tüketici dönünce kayıp 0.
    #[test]
    fn stalled_consumer_keeps_the_record_and_it_is_rung_again() {
        let s = SessionHandle::from_raw(7);
        let mut stash = OpenedStash::new(StashConfig::default());
        let t0 = Instant::now();
        let r = stash.reserve(5).expect("yer");
        stash.fill(r, s, vec![1, 2, 3], opened(s, b"x"), addr(), t0);
        assert!(stash.due_rings(t0).is_empty(), "ilk zil fill'de çalındı");
        let rings = stash.due_rings(t0 + Duration::from_millis(25));
        assert_eq!(rings, vec![(addr(), vec![1, 2, 3])]);
        assert!(stash.due_rings(t0 + Duration::from_millis(30)).is_empty());
        assert_eq!(stash.due_rings(t0 + Duration::from_secs(9)).len(), 1);
        assert_eq!(stash.take(&[1, 2, 3]).expect("kabul").frame, b"x");
        let c = stash.counters();
        assert_eq!((c.rings, c.taken, c.lost()), (2, 1, 0));
        assert!(stash.is_empty());
    }

    /// Ölü tüketici: `dead_consumer_ttl` sonunda kayıt düşer ve SAYILIR.
    #[test]
    fn dead_consumer_record_expires_and_is_counted() {
        let s = SessionHandle::from_raw(7);
        let mut stash = OpenedStash::new(StashConfig::default());
        let t0 = Instant::now();
        for i in 0..3u8 {
            let r = stash.reserve(5).expect("yer");
            stash.fill(r, s, vec![i], opened(s, &[i]), addr(), t0);
        }
        assert_eq!(stash.due_rings(t0 + Duration::from_secs(10)).len(), 0);
        let c = stash.counters();
        assert_eq!((c.dropped_expired, c.lost()), (3, 3));
        assert!(stash.is_empty(), "yuvalar geri verildi");
    }

    /// Kapanan oturum: `Closed` olayı `poll_event`'ten geçerken kayıtlar düşer ve sayılır;
    /// başka oturumun kaydı kalır.
    #[test]
    fn closed_session_drops_its_records_and_counts_them() {
        let mut alice = node(ALICE);
        let (mut hubc, tap) = HubCrypto::hub(node(BOB), StashConfig::default());
        let (_ha, hb) = handshake(&mut alice, &mut hubc);
        let other = SessionHandle::from_raw(999);
        {
            let mut stash = lock(tap.stash());
            for i in 0..2u8 {
                let r = stash.reserve(HUB.raw()).expect("yer");
                stash.fill(r, hb, vec![i], opened(hb, &[i]), addr(), Instant::now());
            }
            let r = stash.reserve(0x0303_0303).expect("yer");
            stash.fill(
                r,
                other,
                vec![9],
                opened(other, &[9]),
                addr(),
                Instant::now(),
            );
        }
        assert_eq!(
            hubc.command(SessionCommand::Close { session: hb }),
            SessionResult::NoOutput
        );
        let mut saw_closed = false;
        while let Some(ev) = hubc.poll_event() {
            saw_closed |= matches!(ev, SessionEvent::Closed(s, _) if s == hb);
        }
        assert!(saw_closed);
        let stash = lock(tap.stash());
        let c = stash.counters();
        assert_eq!((c.dropped_closed, c.lost()), (2, 2));
        assert_eq!(stash.len(), 1, "başka oturumun kaydı kalır");
    }

    /// KK-H54: hub kipinde sınırlar 1280 − sabit yük olarak verilir; aşan çerçeve mühürlenmez.
    #[test]
    fn hub_mode_clamps_limits_to_max_datagram() {
        let mut alice = node(ALICE);
        let (mut hubc, _tap) = HubCrypto::hub(node(BOB), StashConfig::default());
        let (_ha, hb) = handshake(&mut alice, &mut hubc);
        let ctx = ProtectionContext::Session(hb);
        let l = hubc.limits(ctx);
        assert_eq!(
            (l.max_plaintext_bytes + l.wire_overhead_bytes) as usize,
            MAX_DATAGRAM
        );
        let max = l.max_plaintext_bytes as usize;
        let ok = hubc.seal(ctx, &vec![0; max]).expect("sınırda mühür");
        assert_eq!(ok.bytes.len(), MAX_DATAGRAM);
        assert_eq!(hubc.sealed_len(ctx, max), Ok(MAX_DATAGRAM));
        assert_eq!(
            hubc.seal(ctx, &vec![0; max + 1]).err(),
            Some(SealError::FrameTooLarge)
        );
        assert_eq!(hubc.sealed_len(ctx, max + 1), Err(SealError::FrameTooLarge));
        let cons = hubc.conservative_limits(ProtectionClass::Session);
        assert!((cons.max_plaintext_bytes + cons.wire_overhead_bytes) as usize <= MAX_DATAGRAM);
    }
}
