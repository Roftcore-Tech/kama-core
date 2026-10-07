//! A5.5 — GERÇEK S3 `SessionControl` (KM-SEAM-TC-001 §1.2, s3 v1.1) ve A5.4 epoch geçişinin ürün
//! yolu. `NoiseWireProtection`'ın (`crate::protector`) oturum kontrol düzlemidir: iki trait TEK
//! nesnededir, olay kuyruğu ve oturum tablosu ortaktır.
//!
//! **Kurulum (G1 — ccr §35.4 KAPALI-SİSTEM KİPİ):** [`NoiseWireProtection::with_control`] yerel
//! kimliği, doğrulanmış eş dizinini, monoton saati (K2) ve entropi kaynağını İSTER; dizinsiz ürün
//! kurulumu yoktur. Dizinde olmayan göndericinin INIT'i opak `Failed`; zincir/iptal doğrulaması bu
//! kipte çalışma zamanında yapılmaz (BL KATMAN 6 madde 7).
//!
//! **Komutlar:**
//! * `Initiate{peer}` → bekleyen INIT (REF kipi SSI1) + `Initiated{session}`. Tavan
//!   [`PENDING_RESPONSE_TRIAL_CAP`]; `early_unit` bu sprint DESTEKLENMEZ (sahip kararı S1: Transport
//!   saklar, Ready'de ilk `send`) → boş olmayan `early_unit` `Failed`. Çıktı byte-özdeş yeniden
//!   gönderim için Transport'ta önbelleklenir; Crypto aynı zarfı yeniden ÜRETMEZ (K9).
//! * `AcceptInit` → doğrula (dizin → DH → AEAD); aynı `(eş, init_counter)` ve BYTE-ÖZDEŞ INIT →
//!   önbellekli yanıt, yeni oturum/olay YOK (H-RETRY Bob); aynı sayaç farklı bayt → `Failed`. Eşle
//!   etkin oturum YOKSA `EstablishedAndReply` + `Ready`; VARSA `ProvisionalAndReply` (K10). Glare
//!   (K10): bu eşe bekleyen INIT'imiz varken INIT gelirse yerel `dest_hash` < eşinki → kazanan
//!   (`NoOutput`, INIT sessiz düşer); aksi → kaybeden: bekleyenimiz silinir, INIT işlenir.
//! * `AcceptResponse` → bekleyenler en-yeni-önce, en çok [`PENDING_RESPONSE_TRIAL_CAP`] deneme
//!   (K7); yanlış aday ile bozuk paket aynı görünür (`Failed`, state değişmez).
//! * `AssignHandle` → oturum ↔ hub tutamacı bağı (noise-wire §2.55); anahtarlar bağ gelince
//!   kurulur (tutamaçsız oturum mühürlemez: `seal` → `UnknownSession`).
//! * `Renew` → aynı eşe yeni bekleyen INIT; kurulunca `Ready(h')` ve eski oturum yalnız-alım
//!   drain'e düşer → `Closed(h, Superseded)`.
//! * `Close` → bekleyen ya da kurulu oturum; bilinen tutamaçta tam bir `Closed(_, UserClosed)`.
//!
//! **A5.4 (ürün yolu):** her oturumun [`DhEpochMachine`]'i vardır; protector'ın açma yolu her
//! AEAD-geçerli pakette bu düzleme haber verir ([`NoiseWireProtection`]'ın `open`'ı): epoch-1
//! paketi onayı / grace sayımını sürer (`N_g`), zaman (`T_g`) açma yolunda ve `poll_event`'te
//! uygulanır. Provisional oturumda ilk AEAD-geçerli epoch-1 paketi = onay → `Confirmed(new)`;
//! eski oturum yalnız-alım drain'e düşer (`N_g` paket ya da `T_g`, hangisi önce) →
//! `Closed(eski, Superseded)`.
//!
//! **Zaman (K2):** bekleyen INIT [`HALF_OPEN_TTL`] sonra düşer → `Closed(h, KeyHygiene)`;
//! onaylanmayan provisional [`PROVISIONAL_TTL`] sonra SESSİZ düşer (eski oturuma dokunulmaz);
//! [`SESSION_MAX_AGE`]'i aşan oturum bir kez `RenewRequired`.

use core::time::Duration;
use std::collections::BTreeMap;

use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s3::{HubHandle, SessionCommand, SessionControl, SessionEvent, SessionResult};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::clock::MonotonicClock;
use crate::deprecated::deprecated_format;
use crate::entropy::EntropySource;
use crate::envelope::EnvelopeProfile;
use crate::epoch_machine::{DhEpochMachine, EpochAction, RecvEpoch0, Role, GRACE_N_G, GRACE_T_G};
use crate::handshake::{
    self, CertMode, InitSecrets, Ssi1Seal, Ssr1Seal, CIPHER_SUITE_ID_V1, DEST_HASH_LEN,
    PROTOCOL_VERSION_V1, X25519_LEN,
};
use crate::kdf::{TrafficKeys, SESSION_ID_INTERNAL_LEN};
use crate::legacy::LegacyDirection;
use crate::peer_dir::{LocalIdentity, PeerDirectory};
use crate::protector::{NoiseWireProtection, SessionKeyMaterial, KEY_LEN};

/// app-rpc `STORE_MESSAGE.envelope_type` (AMENDMENT-7): SESSION_INIT.
pub const ENVELOPE_INIT: u8 = 0x01;
/// app-rpc `STORE_MESSAGE.envelope_type` (AMENDMENT-7): SESSION_INIT_RESPONSE.
pub const ENVELOPE_INIT_RESPONSE: u8 = 0x02;
/// ccr §35.15 KS-DOS-1: RESPONSE başına deneme tavanı = istemci bekleyen INIT tavanı.
pub const PENDING_RESPONSE_TRIAL_CAP: usize = 16;
/// ccr §35.15 KS-DOS-1 `HALF_OPEN_CAP`: gönderen kimliği başına yarım-açık kayıt (burada: eş başına
/// önbellekli yanıt) tavanı; dolunca en eski düşer.
pub const HALF_OPEN_CAP: usize = 64;
const SECS_PER_DAY: u64 = 86_400;
/// ccr §35.15 `HALF_OPEN_TTL` = S&F mailbox TTL (7 gün; türetilmiş).
pub const HALF_OPEN_TTL: Duration = Duration::from_secs(7 * SECS_PER_DAY);
/// İstemci `PROVISIONAL_TTL` — değeri `HALF_OPEN_TTL` ile aynı, ayrı ad (ccr §35.15; K10).
pub const PROVISIONAL_TTL: Duration = HALF_OPEN_TTL;
/// KS-AGE-1 `SESSION_MAX_AGE` = 7 gün (anahtar hijyeni; aşılınca `RenewRequired`, ölüm DEĞİL).
pub const SESSION_MAX_AGE: Duration = Duration::from_secs(7 * SECS_PER_DAY);
/// SSR1 `response_counter` (ccr §35.5 "epoch 1 başlangıç"; tek RESPONSE per kök — INV-RESP-KEY-1).
const RESPONSE_COUNTER_V1: u64 = 0;

/// Oturumların SESSION_DATA zarfı (oturum başına sabit; uç başına tek kip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeKind {
    /// 22 B legacy (ccr §35.6 / §35.9); grace içinde ≤ 2 anahtar denemesi.
    Legacy,
    /// 26 B extended (noise-wire §2.5); `dh_epoch` telde.
    Extended,
}

/// Ürün kurulumunun girdileri — hepsi ZORUNLU (G1, K2).
pub struct ControlConfig {
    pub identity: LocalIdentity,
    pub directory: PeerDirectory,
    pub clock: Box<dyn MonotonicClock>,
    pub entropy: Box<dyn EntropySource>,
    pub envelope: EnvelopeKind,
}

impl core::fmt::Debug for ControlConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ControlConfig")
            .field("directory", &self.directory)
            .field("envelope", &self.envelope)
            .finish_non_exhaustive()
    }
}

/// Bekleyen INIT (başlatan tarafı). Ephemeral sır Drop'ta silinir.
struct PendingInit {
    handle: SessionHandle,
    peer: [u8; DEST_HASH_LEN],
    ephemeral: StaticSecret,
    secrets: InitSecrets,
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    init_counter: u64,
    created_at: Duration,
    hub_handle: Option<HubHandle>,
    /// `Renew` ile açıldıysa yerini alacağı oturum.
    renews: Option<SessionHandle>,
}

/// Kurulmuş oturumun henüz protector'a kurulmamış trafik anahtarları. `e0 = None`: epoch-0 zaten
/// kurulu (başlatanın bekleyen oturumu hub tutamacı gelince 0-RTT için kurulmuştu).
struct Keys {
    e0: Option<TrafficKeys>,
    e1: TrafficKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetaState {
    Active,
    /// K10: onay bekleyen yeni oturum; `supersedes` tam çalışır.
    Provisional {
        supersedes: SessionHandle,
    },
    /// Yerini yenisine bıraktı: yalnız alım; `N_g` paket ya da `T_g` sonra kapanır.
    Draining {
        since: Duration,
        counted: u32,
    },
}

struct Meta {
    peer: [u8; DEST_HASH_LEN],
    machine: DhEpochMachine,
    created_at: Duration,
    state: MetaState,
    hub_handle: Option<HubHandle>,
    sid: [u8; SESSION_ID_INTERNAL_LEN],
    /// `None` = protector'a kuruldu.
    keys: Option<Keys>,
    age_renew_signalled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedKind {
    Established(SessionHandle),
    Provisional {
        new: SessionHandle,
        supersedes: SessionHandle,
    },
}

/// Önbellekli RESPONSE (ccr INV-RESP-KEY-1: retry = byte-özdeş yeniden gönderim).
struct CachedResponse {
    init_digest: [u8; 32],
    kind: CachedKind,
    ssr1: Vec<u8>,
}

/// Oturum kontrol düzlemi (bkz. modül belgesi).
pub(crate) struct ControlPlane {
    identity: LocalIdentity,
    directory: PeerDirectory,
    clock: Box<dyn MonotonicClock>,
    entropy: Box<dyn EntropySource>,
    envelope: EnvelopeKind,
    /// Eski → yeni; ≤ [`PENDING_RESPONSE_TRIAL_CAP`].
    pending: Vec<PendingInit>,
    sessions: BTreeMap<SessionHandle, Meta>,
    /// `(eş, init_counter)` → yanıt; eş başına ≤ [`HALF_OPEN_CAP`].
    responses: BTreeMap<([u8; DEST_HASH_LEN], u64), CachedResponse>,
    /// Başlatan: eş başına sonraki `init_counter`.
    init_counters: BTreeMap<[u8; DEST_HASH_LEN], u64>,
    /// Son `AcceptResponse`'taki aday denemesi (salt gözlem; K7 kanıtı).
    last_response_trials: usize,
}

impl ControlPlane {
    fn new(cfg: ControlConfig) -> Self {
        Self {
            identity: cfg.identity,
            directory: cfg.directory,
            clock: cfg.clock,
            entropy: cfg.entropy,
            envelope: cfg.envelope,
            pending: Vec::new(),
            sessions: BTreeMap::new(),
            responses: BTreeMap::new(),
            init_counters: BTreeMap::new(),
            last_response_trials: 0,
        }
    }

    fn active_session_with(&self, peer: &[u8; DEST_HASH_LEN]) -> Option<SessionHandle> {
        self.sessions
            .iter()
            .find(|(_, m)| m.peer == *peer && m.state == MetaState::Active)
            .map(|(h, _)| *h)
    }

    fn hub_handle_taken(&self, hub_handle: HubHandle) -> bool {
        self.sessions
            .values()
            .any(|m| m.hub_handle == Some(hub_handle))
            || self
                .pending
                .iter()
                .any(|p| p.hub_handle == Some(hub_handle))
    }

    /// Bir oturuma ait önbellekli yanıtları siler (kapanan oturumun INIT'i yeniden gelirse
    /// önbellekten ölü bir tutamaç dönmesin).
    fn forget_responses_of(&mut self, h: SessionHandle) {
        self.responses.retain(|_, c| match c.kind {
            CachedKind::Established(s) => s != h,
            CachedKind::Provisional { new, .. } => new != h,
        });
    }

    fn fresh_ephemeral(&mut self) -> Option<StaticSecret> {
        let mut bytes = Zeroizing::new([0u8; X25519_LEN]);
        self.entropy.fill(bytes.as_mut()).ok()?;
        Some(StaticSecret::from(*bytes))
    }

    #[cfg(feature = "test-util")]
    fn fingerprint(&self) -> String {
        let pending: Vec<String> = self
            .pending
            .iter()
            .map(|p| {
                format!(
                    "{:?}:{}:{:?}:{:?}",
                    p.handle, p.init_counter, p.hub_handle, p.renews
                )
            })
            .collect();
        let sessions: Vec<String> = self
            .sessions
            .iter()
            .map(|(h, m)| {
                format!(
                    "{h:?}:{:?}:{:?}:{:?}:installed={}:age_renew={}",
                    m.state,
                    m.machine,
                    m.hub_handle,
                    m.keys.is_none(),
                    m.age_renew_signalled
                )
            })
            .collect();
        let responses: Vec<String> = self
            .responses
            .iter()
            .map(|((_, n), c)| format!("{n}:{:?}", c.kind))
            .collect();
        format!(
            "pending={pending:?}|sessions={sessions:?}|responses={responses:?}|counters={:?}",
            self.init_counters.values().collect::<Vec<_>>()
        )
    }
}

/// Rol → (gönderim, alım) anahtarları (ccr §35.2.1: initiator `send = a2b`, responder tersi).
fn role_keys(
    role: Role,
    keys: &TrafficKeys,
) -> (Zeroizing<[u8; KEY_LEN]>, Zeroizing<[u8; KEY_LEN]>) {
    match role {
        Role::Initiator => (keys.a2b.clone(), keys.b2a.clone()),
        Role::Responder => (keys.b2a.clone(), keys.a2b.clone()),
    }
}

/// Oturumun zarf profili (oturum başına sabit; uç başına tek kip).
fn profile_for(
    envelope: EnvelopeKind,
    hub_handle: HubHandle,
    sid: [u8; SESSION_ID_INTERNAL_LEN],
    role: Role,
) -> EnvelopeProfile {
    match envelope {
        EnvelopeKind::Legacy => EnvelopeProfile::Legacy {
            hub_handle,
            protocol_version: PROTOCOL_VERSION_V1,
            cipher_suite_id: CIPHER_SUITE_ID_V1,
            session_id_internal: sid,
            send_direction: match role {
                Role::Initiator => LegacyDirection::AToB,
                Role::Responder => LegacyDirection::BToA,
            },
        },
        EnvelopeKind::Extended => EnvelopeProfile::Extended { h_ext: hub_handle },
    }
}

/// Bekleyen İŞLER: düzlem ödünç alınmışken protector'a dokunulamaz; etkiler toplanıp sonra
/// uygulanır.
#[derive(Default)]
struct Effects {
    actions: Vec<(SessionHandle, EpochAction)>,
    events: Vec<SessionEvent>,
    start_drain: Vec<SessionHandle>,
    close_superseded: Vec<SessionHandle>,
    drop_silently: Vec<SessionHandle>,
    renew: Vec<SessionHandle>,
    /// Düzlemden çıkmış bekleyen INIT'ler: protector'da kuruluysa (0-RTT) orada da kapanır.
    unpend: Vec<SessionHandle>,
}

impl NoiseWireProtection {
    /// ÜRÜN kurulumu: `PacketProtection` + `SessionControl`, eş dizini ve saat ZORUNLU (G1, K2).
    #[must_use]
    pub fn with_control(cfg: ControlConfig) -> Self {
        let mut p = Self::empty();
        p.control = Some(Box::new(ControlPlane::new(cfg)));
        p
    }

    // ── Veri yolu kancaları (protector `open` çağırır) ───────────────────────────────────────

    /// AEAD-geçerli bir paket `h` oturumunda `epoch` ile açıldı (commit SONRASI).
    pub(crate) fn after_session_open(&mut self, h: SessionHandle, epoch: u8) {
        let Some(cp) = self.control.as_deref_mut() else {
            return;
        };
        let now = cp.clock.now();
        let mut fx = Effects::default();
        if let Some(meta) = cp.sessions.get_mut(&h) {
            if let MetaState::Draining { since, counted } = meta.state {
                let counted = counted.saturating_add(1);
                meta.state = MetaState::Draining { since, counted };
                if counted >= GRACE_N_G {
                    fx.close_superseded.push(h);
                }
            }
            if epoch == 1 {
                for a in meta.machine.on_valid_epoch1_received(now) {
                    fx.actions.push((h, a));
                }
                if let MetaState::Provisional { supersedes } = meta.state {
                    // K10 onayı: provisional oturumda ilk AEAD-geçerli epoch-1 paketi.
                    meta.state = MetaState::Active;
                    fx.events.push(SessionEvent::Confirmed(h));
                    fx.start_drain.push(supersedes);
                }
            }
        }
        self.apply_effects(fx, now);
    }

    /// Açma yolunda TEK oturumun zaman kuralları (`T_g`): epoch-0 grace ve drain.
    pub(crate) fn session_tick(&mut self, h: SessionHandle) {
        let Some(cp) = self.control.as_deref_mut() else {
            return;
        };
        let now = cp.clock.now();
        let mut fx = Effects::default();
        if let Some(meta) = cp.sessions.get_mut(&h) {
            Self::tick_meta(h, meta, now, &mut fx);
        }
        self.apply_effects(fx, now);
    }

    fn tick_meta(h: SessionHandle, meta: &mut Meta, now: Duration, fx: &mut Effects) {
        if let Some(a) = meta.machine.on_time(now) {
            fx.actions.push((h, a));
        }
        match meta.state {
            MetaState::Draining { since, .. } if now.saturating_sub(since) >= GRACE_T_G => {
                fx.close_superseded.push(h);
            }
            MetaState::Provisional { .. }
                if now.saturating_sub(meta.created_at) >= PROVISIONAL_TTL =>
            {
                fx.drop_silently.push(h);
            }
            MetaState::Active
                if !meta.age_renew_signalled
                    && now.saturating_sub(meta.created_at) >= SESSION_MAX_AGE =>
            {
                meta.age_renew_signalled = true;
                fx.renew.push(h);
            }
            _ => {}
        }
    }

    /// `poll_event`'in önü: bütün zaman kuralları (bekleyen INIT TTL dahil).
    fn control_sweep(&mut self) {
        let Some(cp) = self.control.as_deref_mut() else {
            return;
        };
        let now = cp.clock.now();
        let mut fx = Effects::default();
        let (expired, kept): (Vec<PendingInit>, Vec<PendingInit>) =
            core::mem::take(&mut cp.pending)
                .into_iter()
                .partition(|p| now.saturating_sub(p.created_at) >= HALF_OPEN_TTL);
        cp.pending = kept;
        for p in expired {
            fx.unpend.push(p.handle);
            fx.events
                .push(SessionEvent::Closed(p.handle, CloseReason::KeyHygiene));
        }
        for (h, meta) in &mut cp.sessions {
            Self::tick_meta(*h, meta, now, &mut fx);
        }
        self.apply_effects(fx, now);
    }

    fn apply_effects(&mut self, fx: Effects, now: Duration) {
        for h in fx.unpend {
            let _ = self.close(h);
        }
        for (h, a) in fx.actions {
            match a {
                EpochAction::InstallEpoch1AndSend => {
                    let _ = self.set_send_epoch(h, 1);
                }
                EpochAction::DiscardEpoch0Recv => {
                    let _ = self.discard_epoch0_recv(h);
                }
                EpochAction::StartGrace => {}
            }
        }
        for ev in fx.events {
            self.push_event(ev);
        }
        for old in fx.start_drain {
            self.start_drain(old, now);
        }
        for h in fx.renew {
            if self.mark_renew_signalled(h) {
                self.push_event(SessionEvent::RenewRequired(h));
            }
        }
        for h in fx.close_superseded {
            self.end_session(h, Some(CloseReason::Superseded));
        }
        for h in fx.drop_silently {
            self.end_session(h, None);
        }
    }

    /// Eski oturum yalnız-alım drain'e düşer (seal → `UnknownSession`; alım `N_g`/`T_g` sürer).
    fn start_drain(&mut self, old: SessionHandle, now: Duration) {
        let Some(cp) = self.control.as_deref_mut() else {
            return;
        };
        let Some(meta) = cp.sessions.get_mut(&old) else {
            return;
        };
        if matches!(meta.state, MetaState::Draining { .. }) {
            return;
        }
        meta.state = MetaState::Draining {
            since: now,
            counted: 0,
        };
        let _ = self.retire_send(old);
    }

    /// Oturumu kapatır: protector state'i, düzlem kaydı, önbellekli yanıtları; `reason` varsa tam
    /// bir `Closed(h, reason)`.
    fn end_session(&mut self, h: SessionHandle, reason: Option<CloseReason>) {
        let existed = self.control.as_deref_mut().is_some_and(|cp| {
            cp.forget_responses_of(h);
            cp.sessions.remove(&h).is_some()
        });
        if !existed {
            return;
        }
        let _ = self.close(h);
        if let Some(r) = reason {
            self.push_event(SessionEvent::Closed(h, r));
        }
    }

    /// Bağ ve anahtarlar hazırsa oturumu protector'a kurar ve makinenin durumunu uygular.
    fn try_install(&mut self, h: SessionHandle) {
        let Some(cp) = self.control.as_deref_mut() else {
            return;
        };
        let envelope = cp.envelope;
        let Some(meta) = cp.sessions.get_mut(&h) else {
            return;
        };
        let Some(hub_handle) = meta.hub_handle else {
            return;
        };
        let Some(keys) = meta.keys.take() else {
            return;
        };
        let role = meta.machine.role();
        let sid = meta.sid;
        let send_epoch = meta.machine.send_epoch();
        let discarded = meta.machine.recv0() == RecvEpoch0::Discarded;
        let draining = matches!(meta.state, MetaState::Draining { .. });
        if !self.sessions_contains(h) {
            let Some(e0) = keys.e0 else {
                return;
            };
            let (send0, recv0) = role_keys(role, &e0);
            if !self.install_session_at(
                h,
                SessionKeyMaterial {
                    profile: profile_for(envelope, hub_handle, sid, role),
                    send_e0: send0,
                    recv_e0: recv0,
                },
            ) {
                // Bağ AssignHandle'da çakışmaya karşı sınandı; buraya gelinmez. Anahtarlar geri
                // konmaz (yarım kurulum yok) — oturum mühürlemez, `Close` temizler.
                return;
            }
        }
        let (send1, recv1) = role_keys(role, &keys.e1);
        let _ = self.install_epoch1(h, &send1, &recv1);
        if send_epoch == 1 {
            let _ = self.set_send_epoch(h, 1);
        }
        if discarded {
            let _ = self.discard_epoch0_recv(h);
        }
        if draining {
            let _ = self.retire_send(h);
        }
    }

    // ── Komutlar ─────────────────────────────────────────────────────────────────────────────

    fn cmd_initiate(
        &mut self,
        peer: PeerRef,
        early_unit: Option<Vec<u8>>,
        renews: Option<SessionHandle>,
    ) -> SessionResult {
        if early_unit.is_some_and(|u| !u.is_empty()) {
            return SessionResult::Failed;
        }
        self.initiate_with(peer, renews, None)
    }

    /// `Initiate` gövdesi. `ephemeral` verilmezse enjekte edilen entropiden üretilir; verilmesi
    /// YALNIZ test kancasıdır ([`NoiseWireProtection::initiate_with_ephemeral_for_test`]).
    fn initiate_with(
        &mut self,
        peer: PeerRef,
        renews: Option<SessionHandle>,
        ephemeral: Option<StaticSecret>,
    ) -> SessionResult {
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        let Some(entry) = cp.directory.by_dest_hash(peer.as_bytes()).copied() else {
            return SessionResult::Failed;
        };
        if cp.pending.len() >= PENDING_RESPONSE_TRIAL_CAP {
            return SessionResult::Failed;
        }
        let counter = cp.init_counters.get(&entry.dest_hash).copied().unwrap_or(0);
        let Some(next_counter) = counter.checked_add(1) else {
            return SessionResult::Failed;
        };
        let Some(ephemeral) = ephemeral.or_else(|| cp.fresh_ephemeral()) else {
            return SessionResult::Failed;
        };
        let recipient_static = PublicKey::from(entry.static_x25519_pub);
        let cert_ref = cp.identity.cert_ref;
        let Ok((wire, secrets)) = handshake::seal_ssi1(&Ssi1Seal {
            cert_mode: CertMode::Ref,
            recipient_dest_hash: &entry.dest_hash,
            recipient_static_pub: &recipient_static,
            sender_static: cp.identity.static_secret(),
            sender_ephemeral: &ephemeral,
            sender_cert_ref: &cert_ref,
            cert_field: &cert_ref,
            init_counter: counter,
            early_data: &[],
        }) else {
            return SessionResult::Failed;
        };
        let now = cp.clock.now();
        let Some(h) = self.mint_handle() else {
            return SessionResult::Failed;
        };
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        cp.init_counters.insert(entry.dest_hash, next_counter);
        cp.pending.push(PendingInit {
            handle: h,
            peer: entry.dest_hash,
            ephemeral,
            secrets,
            init_counter: counter,
            created_at: now,
            hub_handle: None,
            renews,
        });
        SessionResult::Initiated {
            session: h,
            envelope_type: ENVELOPE_INIT,
            bytes: wire,
        }
    }

    fn cmd_accept_init(&mut self, wire: &[u8]) -> SessionResult {
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        if deprecated_format(wire).is_some() {
            return SessionResult::Failed;
        }
        let Ok(parsed) = handshake::parse_ssi1(wire) else {
            return SessionResult::Failed;
        };
        if parsed.recipient_dest_hash != cp.identity.dest_hash {
            return SessionResult::Failed;
        }
        let cert_ref = parsed.sender_cert_ref();
        let Some(entry) = cp.directory.by_cert_ref(&cert_ref).copied() else {
            return SessionResult::Failed;
        };
        let peer = entry.dest_hash;
        let peer_ref = PeerRef::from_dest_hash(peer);
        let digest: [u8; 32] = Sha256::digest(wire).into();

        // H-RETRY (Bob): byte-özdeş yeniden gönderim → önbellek; aynı sayaç farklı bayt → ret.
        if let Some(c) = cp.responses.get(&(peer, parsed.init_counter)) {
            if c.init_digest != digest {
                return SessionResult::Failed;
            }
            return match c.kind {
                CachedKind::Established(session) => SessionResult::EstablishedAndReply {
                    session,
                    peer: peer_ref,
                    envelope_type: ENVELOPE_INIT_RESPONSE,
                    bytes: c.ssr1.clone(),
                },
                CachedKind::Provisional { new, supersedes } => SessionResult::ProvisionalAndReply {
                    new,
                    supersedes,
                    peer: peer_ref,
                    envelope_type: ENVELOPE_INIT_RESPONSE,
                    bytes: c.ssr1.clone(),
                },
            };
        }

        // Glare (K10): kazanan INIT'i sessizce düşürür — state değişmez.
        let glare = cp.pending.iter().any(|p| p.peer == peer);
        if glare && cp.identity.dest_hash < peer {
            return SessionResult::NoOutput;
        }

        let sender_static = PublicKey::from(entry.static_x25519_pub);
        let Ok(opened) = handshake::open_ssi1(
            wire,
            cp.identity.static_secret(),
            &cp.identity.dest_hash,
            |r| (r == &cert_ref).then_some(sender_static),
        ) else {
            return SessionResult::Failed;
        };
        // S1 kararı: early data bu sprint taşınmaz (teslim yolu yok) — sessiz kayıp yerine ret.
        if !opened.early_data.is_empty() {
            return SessionResult::Failed;
        }
        let Some(responder_ephemeral) = cp.fresh_ephemeral() else {
            return SessionResult::Failed;
        };
        let Ok((ssr1, e1)) = handshake::seal_ssr1(&Ssr1Seal {
            init_root: &opened.secrets.init_root,
            session_id_internal: &opened.secrets.session_id_internal,
            initiator_ephemeral: &opened.sender_ephemeral,
            initiator_static: &opened.sender_static,
            responder_ephemeral: &responder_ephemeral,
            response_counter: RESPONSE_COUNTER_V1,
            response_data: &[],
        }) else {
            return SessionResult::Failed;
        };
        let now = cp.clock.now();
        let supersedes = cp.active_session_with(&peer);

        let Some(h) = self.mint_handle() else {
            return SessionResult::Failed;
        };
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        let mut lost = Vec::new();
        if glare {
            // Kaybeden: bu eşe bekleyen INIT'imiz silinir (ephemeral Drop'ta sıfırlanır). Olay
            // YOK [YORUM]: Transport niyetini eşle eşler; bu INIT'in oturumu `Ready` olarak gelir.
            cp.pending.retain(|p| {
                let keep = p.peer != peer;
                if !keep {
                    lost.push(p.handle);
                }
                keep
            });
        }
        let mut machine = DhEpochMachine::new(Role::Responder);
        machine.on_response_sent();
        let (state, kind) = match supersedes {
            Some(old) => (
                MetaState::Provisional { supersedes: old },
                CachedKind::Provisional {
                    new: h,
                    supersedes: old,
                },
            ),
            None => (MetaState::Active, CachedKind::Established(h)),
        };
        cp.sessions.insert(
            h,
            Meta {
                peer,
                machine,
                created_at: now,
                state,
                hub_handle: None,
                sid: opened.secrets.session_id_internal,
                keys: Some(Keys {
                    e0: Some(opened.secrets.traffic_e0),
                    e1: e1.traffic_e1,
                }),
                age_renew_signalled: false,
            },
        );
        // Eş başına önbellek tavanı: dolunca en eski (en küçük sayaç) düşer.
        let of_peer: Vec<u64> = cp
            .responses
            .keys()
            .filter(|(p, _)| *p == peer)
            .map(|(_, n)| *n)
            .collect();
        if of_peer.len() >= HALF_OPEN_CAP {
            if let Some(oldest) = of_peer.first() {
                cp.responses.remove(&(peer, *oldest));
            }
        }
        cp.responses.insert(
            (peer, parsed.init_counter),
            CachedResponse {
                init_digest: digest,
                kind,
                ssr1: ssr1.clone(),
            },
        );
        for h_lost in lost {
            let _ = self.close(h_lost);
        }
        match supersedes {
            Some(old) => SessionResult::ProvisionalAndReply {
                new: h,
                supersedes: old,
                peer: peer_ref,
                envelope_type: ENVELOPE_INIT_RESPONSE,
                bytes: ssr1,
            },
            None => {
                self.push_event(SessionEvent::Ready(h));
                SessionResult::EstablishedAndReply {
                    session: h,
                    peer: peer_ref,
                    envelope_type: ENVELOPE_INIT_RESPONSE,
                    bytes: ssr1,
                }
            }
        }
    }

    fn cmd_accept_response(&mut self, wire: &[u8]) -> SessionResult {
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        if deprecated_format(wire).is_some() {
            return SessionResult::Failed;
        }
        let mut trials = 0usize;
        let mut hit = None;
        for (i, p) in cp.pending.iter().enumerate().rev() {
            if trials == PENDING_RESPONSE_TRIAL_CAP {
                break;
            }
            trials += 1;
            if let Ok(opened) = handshake::open_ssr1(
                wire,
                &p.secrets.init_root,
                &p.secrets.session_id_internal,
                &p.ephemeral,
                cp.identity.static_secret(),
            ) {
                hit = Some((i, opened));
                break;
            }
        }
        cp.last_response_trials = trials;
        let Some((i, opened)) = hit else {
            return SessionResult::Failed;
        };
        if !opened.response_data.is_empty() {
            return SessionResult::Failed;
        }
        let now = cp.clock.now();
        let p = cp.pending.remove(i);
        let h = p.handle;
        // Bekleyen oturum hub tutamacı gelince epoch-0 için kurulmuş olabilir (0-RTT).
        let e0_installed = self.sessions_contains(h);
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        let mut machine = DhEpochMachine::new(Role::Initiator);
        let _ = machine.on_response_received();
        cp.sessions.insert(
            h,
            Meta {
                peer: p.peer,
                machine,
                created_at: now,
                state: MetaState::Active,
                hub_handle: p.hub_handle,
                sid: p.secrets.session_id_internal,
                keys: Some(Keys {
                    e0: (!e0_installed).then_some(p.secrets.traffic_e0),
                    e1: opened.secrets.traffic_e1,
                }),
                age_renew_signalled: false,
            },
        );
        let renews = p.renews;
        self.try_install(h);
        self.push_event(SessionEvent::Ready(h));
        if let Some(old) = renews {
            self.start_drain(old, now);
        }
        SessionResult::Established { session: h }
    }

    fn cmd_assign_handle(
        &mut self,
        session: SessionHandle,
        hub_handle: HubHandle,
    ) -> SessionResult {
        let in_use = self.wire_handle_in_use(hub_handle.raw());
        let Some(cp) = self.control.as_deref_mut() else {
            return SessionResult::Failed;
        };
        let current = if let Some(m) = cp.sessions.get(&session) {
            m.hub_handle
        } else if let Some(p) = cp.pending.iter().find(|p| p.handle == session) {
            p.hub_handle
        } else {
            return SessionResult::Failed;
        };
        match current {
            Some(bound) if bound == hub_handle => return SessionResult::NoOutput,
            Some(_) => return SessionResult::Failed,
            None => {}
        }
        if in_use || cp.hub_handle_taken(hub_handle) {
            return SessionResult::Failed;
        }
        let envelope = cp.envelope;
        let mut pending_material = None;
        if let Some(m) = cp.sessions.get_mut(&session) {
            m.hub_handle = Some(hub_handle);
        } else if let Some(p) = cp.pending.iter_mut().find(|p| p.handle == session) {
            p.hub_handle = Some(hub_handle);
            // Başlatanın bekleyen oturumu epoch-0 için hemen kurulur (0-RTT; ccr §0.7-A
            // "epoch-0-usable"). `Ready` yine `Established`'dadır.
            let (send_e0, recv_e0) = role_keys(Role::Initiator, &p.secrets.traffic_e0);
            pending_material = Some(SessionKeyMaterial {
                profile: profile_for(
                    envelope,
                    hub_handle,
                    p.secrets.session_id_internal,
                    Role::Initiator,
                ),
                send_e0,
                recv_e0,
            });
        }
        match pending_material {
            Some(material) => {
                let _ = self.install_session_at(session, material);
            }
            None => self.try_install(session),
        }
        SessionResult::NoOutput
    }

    fn cmd_renew(&mut self, session: SessionHandle) -> SessionResult {
        let peer = match self.control.as_deref() {
            Some(cp) => match cp.sessions.get(&session) {
                Some(m) if m.state == MetaState::Active => PeerRef::from_dest_hash(m.peer),
                _ => return SessionResult::Failed,
            },
            None => return SessionResult::Failed,
        };
        self.cmd_initiate(peer, None, Some(session))
    }

    fn cmd_close(&mut self, session: SessionHandle) -> SessionResult {
        let was_pending = self.control.as_deref_mut().is_some_and(|cp| {
            let before = cp.pending.len();
            cp.pending.retain(|p| p.handle != session);
            cp.pending.len() != before
        });
        if was_pending {
            let _ = self.close(session);
            self.push_event(SessionEvent::Closed(session, CloseReason::UserClosed));
            return SessionResult::NoOutput;
        }
        self.end_session(session, Some(CloseReason::UserClosed));
        SessionResult::NoOutput
    }

    // ── YALNIZ test (`test-util`) ─────────────────────────────────────────────────────────────

    /// Son `AcceptResponse`'taki aday denemesi (K7 gözlem noktası).
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn last_response_trials(&self) -> usize {
        self.control
            .as_deref()
            .map_or(0, |cp| cp.last_response_trials)
    }

    /// Verilen ephemeral'la `Initiate` (H-DEMUX fikstürü: yanıtı test bağımsızca hesaplar).
    /// Döner: bekleyen oturumun tutamacı; `Initiate`'in reddettiği durumda `None`.
    #[cfg(feature = "test-util")]
    pub fn initiate_with_ephemeral_for_test(
        &mut self,
        peer: PeerRef,
        ephemeral: [u8; X25519_LEN],
    ) -> Option<SessionHandle> {
        match self.initiate_with(peer, None, Some(StaticSecret::from(ephemeral))) {
            SessionResult::Initiated { session, .. } => Some(session),
            _ => None,
        }
    }

    /// Kurulu oturumun epoch durumu: (gönderim epoch'u, epoch-0 alım durumu).
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn epoch_state_for_test(&self, h: SessionHandle) -> Option<(u8, RecvEpoch0)> {
        self.control
            .as_deref()
            .and_then(|cp| cp.sessions.get(&h))
            .map(|m| (m.machine.send_epoch(), m.machine.recv0()))
    }

    /// Bekleyen INIT sayısı.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.control.as_deref().map_or(0, |cp| cp.pending.len())
    }

    /// Kontrol düzleminin parmak izi (salt-gözlem alanları HARİÇ).
    #[cfg(feature = "test-util")]
    pub(crate) fn control_fingerprint(&self) -> String {
        self.control
            .as_deref()
            .map_or_else(String::new, ControlPlane::fingerprint)
    }
}

impl SessionControl for NoiseWireProtection {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        match cmd {
            SessionCommand::Initiate { peer, early_unit } => {
                self.cmd_initiate(peer, early_unit, None)
            }
            SessionCommand::AcceptInit { wire } => self.cmd_accept_init(&wire),
            SessionCommand::AcceptResponse { wire } => self.cmd_accept_response(&wire),
            SessionCommand::AssignHandle {
                session,
                hub_handle,
            } => self.cmd_assign_handle(session, hub_handle),
            SessionCommand::Renew { session } => self.cmd_renew(session),
            SessionCommand::Close { session } => self.cmd_close(session),
        }
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.control_sweep();
        self.take_event()
    }
}
