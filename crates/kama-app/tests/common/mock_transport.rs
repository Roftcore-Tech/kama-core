//! `MockTransport` — KM-SEAM-AT-001 §7'deki "Hayri için" karşı-taraf stub'u.
//!
//! **ported from feature/app (Hayri)**; güncel S1'e (DR-CONNECT-1, B1 `kama-contracts`) taşındı.
//!
//! Application'ı gerçek Transport/Crypto olmadan geliştirmeyi/test etmeyi sağlar. Plane 1 ve
//! Crypto **içermez** — yalnız `KamaTransport` sözleşmesinin gözlenebilir davranışını taklit eder.
//! Zamanlama (async handshake) gerçek zamanlı değildir: olaylar testin çağırdığı enjeksiyon
//! metotlarıyla **deterministik** tetiklenir; saat sanaldır.
//!
//! # TEK SAHTE, İKİ TÜKETİCİ
//!
//! Bu dosya iki yerden derlenir:
//! * `kama-app`'in kendi testleri (Application geliştirme sahtesi);
//! * `kama-integration/tests/seam_conformance_core` (S1 sözleşme süitinin `mock` hedefi; dosyayı
//!   `#[path]` ile alır).
//!
//! İkinci bir kopya YOKTUR: sözleşme süitinin sınadığı sahte ile Application'ın kullandığı sahte
//! aynı koddur. Bu yüzden dosya yalnız `kama_contracts`'a bağlıdır (`test-util` DEĞİL).
//!
//! # Hayri'nin özgün sürümünden farklar (2026-10-05)
//!
//! * `ConnectFailed` / `fail_connect` KALKTI (DR-CONNECT-1 H9). Yerine: `cancel(ticket)` →
//!   `ConnectAbandoned(ticket, Cancelled)`; [`MockTransport::abandon_connect`] (kapalı nedenlerle);
//!   `ConnectProgress` (link durumu).
//! * `CloseReason::Renewed` KALKTI: cutover'ın kapanışı `Closed(h, Superseded)`.
//! * `SessionSuperseded` → `UnknownSession` geçişi artık `close()` çağrısına değil `Closed` olayının
//!   TESLİMİNE bağlı: cutover ile `Closed` arasında `send(h)` → `SessionSuperseded` (H-CUTOVER),
//!   `Closed` Application'a ulaştıktan sonra → `UnknownSession` (H-EVENTS). Özgün sürümde `Closed`
//!   okunduktan sonra da `close()` çağrılana dek `SessionSuperseded` dönüyordu.
//! * `PeerRef` artık `dest_hash` sarmalayıcısı; haritalar sıralı (`BTreeMap`) — yineleme sırası
//!   deterministik.
//! * Eklenenler: pending INIT tavanı (H-PENDING-CAP), sanal saat + link durumu + prob sayacı
//!   (H-PROGRESS / H-CANCEL), sahte RESPONSE enjeksiyonu (H-SILENT-REJECT), kayıp enjeksiyonu +
//!   recovery anahtarı (H-APP-RETRY-0), sınırlı kuyruklar (typed `Backpressure`).
//!
//! # Sahtenin MODELLEMEDİĞİ
//!
//! Provisional supersession (bilinen eşten yeni-sayaçlı INIT) ve glare · `LongOutage` fazı ·
//! `DeadlineBounded` süresinin dolması · sırasız/duplicate teslim ENJEKSİYONU (§7 sayar; yok) ·
//! gerçek kayıp/kurtarma (buradaki "recovery" bir anahtardır, ARQ/FEC değil) · prob backoff'u ve
//! jitter'ı (sabit aralık) · `early_unit` için AYRI boyut sınırı (K6 `max_early_unit_bytes`;
//! sözleşme tipinde böyle bir alan yok, sahte `max_plaintext_unit_bytes` uygular) · AĞ: link
//! durumu yalnız prob emisyonunu ve `ConnectProgress` olaylarını etkiler — link yokken de `pump()`
//! handshake tamamlar ve `send` teslim eder. Bunlar gerçek hedefin işidir.
//!
//! # Sahteye ÖZGÜ davranış (S1'de karşılığı yok)
//!
//! Oturum tablosu dolunca (`MAX_SESSIONS`) `connect()` de `Backpressure` döner. S1 §3 `connect()`
//! için `Backpressure`'ı yalnız pending INIT tavanında tanımlar; buradaki ikinci koşul sahtenin
//! bellek sınırıdır, sözleşme davranışı değildir.

// Bu ortak modül birden fazla test ikilisi tarafından ayrı ayrı derlenir; her ikili enjeksiyon
// metotlarının hepsini kullanmayabilir.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;

use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    AbandonReason, ConnectPhase, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy,
    KamaTransport, PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};

/// İstemci pending INIT tavanı (ccr §35.15 KS-DOS-1; tutarlılık zinciri gereği
/// `PENDING_RESPONSE_TRIAL_CAP` ile EŞİT). Bu kadar bekleyen `connect` varken yeni bir eşe `connect`
/// → `Backpressure` (KM-SEAM-AT-001 §3, H-PENDING-CAP). Sözleşme satırı bu sabiti spec'teki sayıyla
/// karşılaştırır.
pub const PENDING_INIT_CAP: usize = 16;
/// Eşzamanlı oturum tavanı — yalnız sahtenin bellek sınırı. Pending tavanından BÜYÜK olmalıdır:
/// yoksa pending tavanına hiç ulaşılamaz ve H-PENDING-CAP tablo tavanını ölçerdi.
pub const MAX_SESSIONS: usize = 2 * PENDING_INIT_CAP;
/// Teslim kuyruğu tavanı.
pub const MAX_INBOX: usize = 64;
/// Olay kuyruğu tavanı.
pub const MAX_EVENTS: usize = 64;
/// Link açıkken hub yolu prob aralığı: transport-tdd §10 [AMENDMENT-10]'daki backoff TAVANI
/// (KS-HB-1, ön plan 8 s). Sahte sabit aralıkla prob atar; backoff rampasını (500 ms'den ×2) ve
/// jitter'ı modellemez. Sözleşme satırları bu sayıya bağlanmaz.
pub const PROBE_INTERVAL: Duration = Duration::from_secs(8);

/// Gözlem sayaçları — salt-okunur; sözleşme yüzeyi DEĞİLDİR.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MockStats {
    /// Hub yolu prob emisyonu (link kapalıyken ARTMAZ).
    pub probes: u64,
    /// Kayıp enjekte edilen birim sayısı.
    pub units_dropped: u64,
    /// Recovery AÇIKKEN Transport'un kendi içinde kurtardığı birim sayısı. Yalnız KAYBOLMUŞ
    /// birimler sayılır; sıra korunsun diye onların arkasında bekleyenler sayılmaz.
    pub units_recovered: u64,
    /// Sessizce yok sayılan sahte/geçersiz RESPONSE sayısı.
    pub invalid_responses: u64,
    /// Olay kuyruğu doluyken düşen olay sayısı (senaryo `poll_event`'i yeterince çağırmadı).
    pub events_dropped: u64,
}

/// Teslimi ertelenmiş birim.
struct Held {
    delivered: Delivered,
    /// `true` = ilk iletimi kayboldu, Transport kurtaracak · `false` = kaybolmadı, yalnız önündeki
    /// kayıp birimi geçmemek için bekliyor (§4 sıralaması).
    lost: bool,
}

pub struct MockTransport {
    capabilities: TransportCapabilities,
    next_id: u64,
    /// peer → aktif session (KARAR-1 idempotent connect'in kaynağı).
    active_by_peer: BTreeMap<PeerRef, SessionHandle>,
    /// peer → bekleyen ticket (henüz tamamlanmadı / iptal edilmedi).
    pending_by_peer: BTreeMap<PeerRef, ConnectTicket>,
    /// pending ticket'a bağlı `early_unit`; `complete_connect`'te teslim edilir (K6).
    pending_early: BTreeMap<ConnectTicket, Vec<u8>>,
    session_peer: BTreeMap<SessionHandle, PeerRef>,
    /// Cutover ile yerini yenisine bırakmış, `Closed` olayı henüz TESLİM EDİLMEMİŞ session'lar:
    /// `send` → `SessionSuperseded` (H-CUTOVER).
    superseded: BTreeSet<SessionHandle>,
    events: VecDeque<SessionEvent>,
    inbox: VecDeque<Delivered>,
    // ── senaryo durumu ──
    link_up: bool,
    /// Sanal saat; son prob emisyonundan beri geçen süre.
    since_probe: Duration,
    recovery: bool,
    drops_armed: usize,
    /// Recovery AÇIKKEN kaybolan birimler ve sıra korunsun diye onların arkasında bekleyenler;
    /// [`MockTransport::pump`] ile teslim edilir.
    in_recovery: VecDeque<Held>,
    stats: MockStats,
}

impl MockTransport {
    pub fn new(capabilities: TransportCapabilities) -> Self {
        Self {
            capabilities,
            next_id: 1,
            active_by_peer: BTreeMap::new(),
            pending_by_peer: BTreeMap::new(),
            pending_early: BTreeMap::new(),
            session_peer: BTreeMap::new(),
            superseded: BTreeSet::new(),
            events: VecDeque::new(),
            inbox: VecDeque::new(),
            link_up: true,
            since_probe: Duration::ZERO,
            recovery: true,
            drops_armed: 0,
            in_recovery: VecDeque::new(),
            stats: MockStats::default(),
        }
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Olay kuyruğu sınırlıdır (sahte sınırsız büyümez); doluyken gelen olay DÜŞER ve
    /// [`MockStats::events_dropped`] artar — düşme sessiz değildir. Senaryolar tavanın çok altında
    /// kalır.
    ///
    /// Düşen bir `Closed` hiçbir zaman OKUNAMAZ; tutamaç sonsuza dek "supersede" kalmasın diye
    /// unutma o anda yapılır. Böylece supersede kümesi de sınırlıdır (her üyesinin kuyrukta bir
    /// `Closed` olayı vardır). Düşen bir `Ready(h)` oturumu yok etmez: Application tutamacı
    /// idempotent `connect()` dönüşünden öğrenebilir.
    fn push_event(&mut self, ev: SessionEvent) {
        if self.events.len() < MAX_EVENTS {
            self.events.push_back(ev);
        } else {
            self.stats.events_dropped += 1;
            if let SessionEvent::Closed(h, _) = ev {
                self.forget(h);
            }
        }
    }

    /// Tutamacı tamamen unutur: bundan sonra `send(h)` → `UnknownSession`.
    fn forget(&mut self, session: SessionHandle) {
        self.superseded.remove(&session);
        self.session_peer.remove(&session);
    }

    pub fn stats(&self) -> MockStats {
        self.stats
    }

    pub fn pending_len(&self) -> usize {
        self.pending_by_peer.len()
    }

    // ── bağlantı senaryoları ─────────────────────────────────────────────────────────────────

    /// Bekleyen bir `connect()`'i tamamlar ("geçerli RESPONSE geldi"): yeni `SessionHandle` üretir,
    /// `Ready` olayını kuyruğa koyar, varsa `early_unit`'i teslim eder (H-ROUNDTRIP).
    ///
    /// # Panics
    /// Peer için bekleyen connect yoksa.
    pub fn complete_connect(&mut self, peer: &PeerRef) -> SessionHandle {
        let ticket = self
            .pending_by_peer
            .remove(peer)
            .expect("complete_connect: peer için bekleyen connect yok");
        let handle = SessionHandle::from_raw(self.fresh_id());
        self.active_by_peer.insert(*peer, handle);
        self.session_peer.insert(handle, *peer);
        self.push_event(SessionEvent::Ready(handle, *peer));
        if let Some(early_bytes) = self.pending_early.remove(&ticket) {
            let delivered = Delivered {
                session: handle,
                unit: early_bytes,
            };
            if self.inbox.len() < MAX_INBOX {
                self.inbox.push_back(delivered);
            } else {
                // Teslim kuyruğu dolu: kabul edilmiş `early_unit` DÜŞÜRÜLMEZ; yer açılınca (`pump`)
                // teslim edilir. Bekleyen niyet sayısı sınırlı olduğundan bu bekleme de sınırlıdır.
                self.in_recovery.push_back(Held {
                    delivered,
                    lost: false,
                });
            }
        }
        handle
    }

    /// Bekleyen bütün connect'leri (eş sırasıyla) tamamlar.
    pub fn complete_all_pending(&mut self) {
        let peers: Vec<PeerRef> = self.pending_by_peer.keys().copied().collect();
        for peer in peers {
            self.complete_connect(&peer);
        }
    }

    /// Bekleyen bir niyeti KAPALI nedenlerden biriyle sonlandırır (§4: karşı taraf kapattı, anahtar
    /// hijyeni, kalıcı yerel hata). Kullanıcı iptali için `cancel(ticket)` kullanılır.
    ///
    /// # Panics
    /// Peer için bekleyen connect yoksa.
    pub fn abandon_connect(&mut self, peer: &PeerRef, reason: AbandonReason) {
        let ticket = self
            .pending_by_peer
            .remove(peer)
            .expect("abandon_connect: peer için bekleyen connect yok");
        self.pending_early.remove(&ticket);
        self.push_event(SessionEvent::ConnectAbandoned(ticket, reason));
    }

    /// Sahte/geçersiz bir RESPONSE geldi (yanlış peer material, bozuk tag, bayat yanıt…). HİÇBİR
    /// olay üretilmez, pending yaşar, ticket geçerli kalır (H-SILENT-REJECT; K9: kripto nedeni
    /// Application'a sızmaz). Yalnız gözlem sayacı artar.
    pub fn inject_invalid_response(&mut self, _peer: &PeerRef) {
        self.stats.invalid_responses += 1;
    }

    // ── yenileme / cutover ───────────────────────────────────────────────────────────────────

    /// Aktif bir session için anahtar yenileme gereksinimini tetikler (`RenewRequired`).
    /// `session` cutover'a kadar `send` kabul etmeye devam eder (H-CUTOVER).
    ///
    /// # Panics
    /// Session bilinmiyorsa ya da zaten supersede edilmişse.
    pub fn request_renew(&mut self, session: SessionHandle) {
        assert!(
            self.session_peer.contains_key(&session) && !self.superseded.contains(&session),
            "request_renew: bilinmeyen veya zaten supersede edilmiş session"
        );
        self.push_event(SessionEvent::RenewRequired(session));
    }

    /// Atomik cutover (KARAR-2): yeni `SessionHandle` aktif olur; eskisi `SessionSuperseded` ile
    /// reddeder. Olay sırası (H-EVENTS): `Ready(yeni)` sonra `Closed(eski, Superseded)`.
    ///
    /// # Panics
    /// Session bilinmiyorsa ya da zaten supersede edilmişse.
    pub fn complete_renew(&mut self, old: SessionHandle) -> SessionHandle {
        let peer = *self
            .session_peer
            .get(&old)
            .expect("complete_renew: bilinmeyen session");
        assert!(
            !self.superseded.contains(&old),
            "complete_renew: session zaten supersede edilmiş"
        );
        let new_handle = SessionHandle::from_raw(self.fresh_id());
        self.active_by_peer.insert(peer, new_handle);
        self.session_peer.insert(new_handle, peer);
        self.superseded.insert(old);
        self.push_event(SessionEvent::Ready(new_handle, peer));
        self.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
        new_handle
    }

    // ── link / saat ──────────────────────────────────────────────────────────────────────────

    /// Link durumunu değiştirir. Kapanınca bekleyen her niyet için `ConnectProgress(Suspended)` ve
    /// prob DURUR; açılınca `ConnectProgress(Measuring)` ve ANINDA bir prob (H-PROGRESS). Bu olaylar
    /// hub yolunun durumudur; eşin canlılığı hakkında hiçbir şey söylemezler.
    pub fn set_link(&mut self, up: bool) {
        if self.link_up == up {
            return;
        }
        self.link_up = up;
        let phase = if up {
            self.stats.probes += 1;
            self.since_probe = Duration::ZERO;
            ConnectPhase::Measuring
        } else {
            ConnectPhase::Suspended
        };
        let tickets: Vec<ConnectTicket> = self.pending_by_peer.values().copied().collect();
        for ticket in tickets {
            self.push_event(SessionEvent::ConnectProgress(ticket, phase));
        }
    }

    /// Sanal saati ilerletir. Link açıkken her [`PROBE_INTERVAL`]'de bir prob; kapalıyken SIFIR.
    /// Zaman hiçbir niyeti ve hiçbir oturumu sonlandırmaz (INV-NO-IDLE-TEARDOWN).
    pub fn advance_clock(&mut self, by: Duration) {
        if !self.link_up {
            return;
        }
        self.since_probe += by;
        while self.since_probe >= PROBE_INTERVAL {
            self.since_probe -= PROBE_INTERVAL;
            self.stats.probes += 1;
        }
    }

    // ── kayıp / kurtarma ─────────────────────────────────────────────────────────────────────

    /// Transport'un kayıp kurtarmasını açar/kapatır (varsayılan AÇIK).
    pub fn set_recovery(&mut self, on: bool) {
        self.recovery = on;
    }

    /// Sonraki `n` gönderimin ilk iletimini kaybettirir. Recovery AÇIKSA birim Transport'un İÇİNDE
    /// kurtarılır ([`MockTransport::pump`] ile teslim edilir); KAPALIYSA kalıcı olarak kaybolur.
    /// İki durumda da `send()` `Ok(())` döner: kabul teslim değildir (K8).
    pub fn drop_next_units(&mut self, n: usize) {
        self.drops_armed += n;
    }

    /// Hedefi bir adım sürer: bekleyen handshake'ler tamamlanır, kurtarılabilir kayıplar teslim edilir.
    pub fn pump(&mut self) {
        self.complete_all_pending();
        while self.inbox.len() < MAX_INBOX {
            let Some(held) = self.in_recovery.pop_front() else {
                break;
            };
            if held.lost {
                self.stats.units_recovered += 1;
            }
            self.inbox.push_back(held.delivered);
        }
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

        // Yeni niyet. Reddedilirse HİÇBİR state değişmez ve `early_unit` hiçbir yola girmez.
        if self.pending_by_peer.len() >= PENDING_INIT_CAP
            || self.session_peer.len() + self.pending_by_peer.len() >= MAX_SESSIONS
        {
            return Err(TransportError::Backpressure);
        }
        if let Some(unit) = &early_unit {
            if unit.bytes.len() > self.capabilities.max_plaintext_unit_bytes as usize {
                return Err(TransportError::UnitTooLarge);
            }
        }

        let ticket = ConnectTicket::from_raw(self.fresh_id());
        self.pending_by_peer.insert(peer, ticket);
        if let Some(unit) = early_unit {
            self.pending_early.insert(ticket, unit.bytes);
        }
        if !self.link_up {
            // Link yokken açılan niyet: prob yok, UI "ağ yok, bekliyor" görsün.
            self.push_event(SessionEvent::ConnectProgress(
                ticket,
                ConnectPhase::Suspended,
            ));
        }
        Ok(ConnectResult::Pending(ticket))
    }

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        _policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        // K4: `compression_policy` Transport davranışını DEĞİŞTİRMEZ — burada okunmaz.
        if self.superseded.contains(&session) {
            return Err(TransportError::SessionSuperseded);
        }
        if !self.session_peer.contains_key(&session) {
            return Err(TransportError::UnknownSession);
        }
        if unit.bytes.len() > self.capabilities.max_plaintext_unit_bytes as usize {
            return Err(TransportError::UnitTooLarge);
        }
        if self.inbox.len() + self.in_recovery.len() >= MAX_INBOX {
            return Err(TransportError::Backpressure);
        }
        // Loopback: sahte, eşi simüle etmez; gönderilen birimi kendi inbox'ına yansıtır
        // (H-ROUNDTRIP: send → recv byte-identical).
        let delivered = Delivered {
            session,
            unit: unit.bytes,
        };
        if self.drops_armed > 0 {
            self.drops_armed -= 1;
            self.stats.units_dropped += 1;
            if self.recovery {
                self.in_recovery.push_back(Held {
                    delivered,
                    lost: true,
                });
            }
            return Ok(());
        }
        // §4: aynı session içinde birimler GÖNDERİM SIRASINDA teslim edilir. Önünde kurtarılmayı
        // bekleyen bir birim varken sonraki birim onu geçemez; arkasına dizilir.
        if self
            .in_recovery
            .iter()
            .any(|held| held.delivered.session == session)
        {
            self.in_recovery.push_back(Held {
                delivered,
                lost: false,
            });
        } else {
            self.inbox.push_back(delivered);
        }
        Ok(())
    }

    fn recv(&mut self) -> Option<Delivered> {
        self.inbox.pop_front()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        let ev = self.events.pop_front()?;
        // `Closed` Application'a ULAŞINCA eski tutamaç tamamen unutulur: bundan sonra `send(h)` →
        // `UnknownSession` (H-EVENTS). O ana kadar `SessionSuperseded` (H-CUTOVER).
        if let SessionEvent::Closed(h, _) = ev {
            self.forget(h);
        }
        Some(ev)
    }

    /// Tutamacı tamamen unutur (`UnknownSession`). Olay ÜRETMEZ: S1, Application'ın kendi `close()`
    /// çağrısının bir `Closed` olayı doğurup doğurmadığını söylemiyor.
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

    /// DR-CONNECT-1 H9 (§4): yalnız BEKLEYEN niyet iptal edilir; `early_unit` teslim EDİLMEZ; ticket
    /// geçersizleşir; tek olay `ConnectAbandoned(ticket, Cancelled)`. Bilinmeyen ya da tamamlanmış
    /// ticket için hiçbir şey olmaz.
    fn cancel(&mut self, ticket: ConnectTicket) {
        let Some(peer) = self
            .pending_by_peer
            .iter()
            .find(|(_, t)| **t == ticket)
            .map(|(p, _)| *p)
        else {
            return;
        };
        self.pending_by_peer.remove(&peer);
        self.pending_early.remove(&ticket);
        self.push_event(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::Cancelled,
        ));
    }
}
