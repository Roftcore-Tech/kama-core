//! PoC-BIND istemci durum makinesi (S4 §2.4(a)–(b), C2, C8; KK-H40, KK-H44).
//!
//! ```text
//! Round1 ──CLIENT_BIND yanıtı (CHALLENGE, aynı request_id)──▶ Round2 ──imzalı yanıt + verify_strict──▶ Bound
//!   ▲  │ 3 gönderim × 1 s cevapsız                           │ │ BAD_COOKIE → Round1 (yeni nonce)
//!   │  ▼                                                     │ ▼ 3 gönderim × 1 s cevapsız
//!   └─ Backoff (T3 dizisi + KK-H48 jitter) ◀─────────────────┘
//! ```
//!
//! * Her bind denemesinde YENİ `client_nonce` (entropiden; C2). Aynı anda tek deneme: tek durum.
//! * Tur içi tekrar bayt-özdeştir (aynı nonce, aynı `request_id`, aynı kanıt; aynı adres HB2'nin tek
//!   soketidir). `BAD_COOKIE` sonrası eski kanıt bir daha GÖNDERİLMEZ.
//! * Hub imzası `kama_noise_wire::ed25519_strict::verify_strict` (katı kip) ile doğrulanmadan bind
//!   "tamam" sayılmaz. İmzasız hata çerçeveleri yalnız ipucudur, bind'i düşürmez (C8).
//! * Cihaz imzası [`BindSigner`] ile; TBS tipli alanlardan (`codec::bind_proof_tbs`), imza kâhini yok.

use core::time::Duration;
use std::time::Instant;

use kama_noise_wire::ed25519_strict::verify_strict;
use kama_noise_wire::entropy::EntropySource;

use super::codec::{self, code, flags, method, NONCE_LEN};
use super::heartbeat::jitter;
use super::t3::t3_delay;
use super::{BindSigner, HubConfig, HubCounters};

/// Bir turdaki (CLIENT_BIND ya da BIND_PROOF) azami gönderim (C2 "en çok 3 kez, 1 s arayla").
pub const BIND_ROUND_SENDS: u32 = 3;
/// Tur içi tekrar aralığı (jitter YOK; KK-H48).
pub const BIND_RESEND: Duration = Duration::from_secs(1);
/// Entropi hatası sonrası yeniden deneme.
const ENTROPY_RETRY: Duration = Duration::from_secs(1);

/// `request_id` üreticisi (S4 §2.3): süreç başında rastgele, her yeni istekte +1, sarmalar.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RequestIds(u32);

impl RequestIds {
    pub(crate) fn starting_at(v: u32) -> Self {
        Self(v)
    }

    pub(crate) fn next(&mut self) -> u32 {
        let v = self.0;
        self.0 = self.0.wrapping_add(1);
        v
    }
}

/// Bind makinesine bağımlılıklar (ödünç).
pub(crate) struct BindDeps<'a> {
    pub cfg: &'a HubConfig,
    pub signer: &'a dyn BindSigner,
    pub entropy: &'a mut dyn EntropySource,
    pub ids: &'a mut RequestIds,
    pub counters: &'a mut HubCounters,
}

/// Bir yanıtın bind makinesine etkisi.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BindOutcome {
    /// Durum değişmedi (ya da yalnız sayaç).
    Nothing,
    /// Bu çerçeve hemen gönderilmeli.
    Send(Vec<u8>),
    /// Hub imzası doğrulandı: bağlı.
    Bound,
}

enum State {
    Round1 {
        nonce: [u8; NONCE_LEN],
        request_id: u32,
        frame: Vec<u8>,
        sends: u32,
        next: Instant,
    },
    Round2 {
        nonce: [u8; NONCE_LEN],
        hub_nonce: [u8; NONCE_LEN],
        request_id: u32,
        frame: Vec<u8>,
        sends: u32,
        next: Instant,
    },
    Backoff {
        until: Instant,
    },
    Bound {
        nonce: [u8; NONCE_LEN],
    },
}

/// PoC-BIND makinesi.
pub(crate) struct BindMachine {
    state: State,
    /// Ardışık tükenen tur sayısı (T3 dizisinin indisi); kabulde sıfırlanır.
    exhausted: u32,
}

impl BindMachine {
    /// Yeni makine; ilk CLIENT_BIND `now`'da gönderilir.
    pub(crate) fn start(now: Instant, deps: &mut BindDeps<'_>) -> Self {
        let mut m = Self {
            state: State::Backoff { until: now },
            exhausted: 0,
        };
        m.new_round(now, deps);
        m
    }

    pub(crate) fn is_bound(&self) -> bool {
        matches!(self.state, State::Bound { .. })
    }

    /// Kabul edilen bind'in `client_nonce`'u (KK-H48 jitter tohumu).
    pub(crate) fn bound_nonce(&self) -> Option<&[u8; NONCE_LEN]> {
        match &self.state {
            State::Bound { nonce } => Some(nonce),
            _ => None,
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        match &self.state {
            State::Round1 { next, .. } | State::Round2 { next, .. } => Some(*next),
            State::Backoff { until } => Some(*until),
            State::Bound { .. } => None,
        }
    }

    /// "Koptu" (C7): aynı kimlik, yeni nonce ile 1. tur hemen.
    pub(crate) fn restart(&mut self, now: Instant, deps: &mut BindDeps<'_>) {
        self.exhausted = 0;
        self.new_round(now, deps);
    }

    /// 1. tur: yeni `client_nonce`, yeni `request_id`; ilk gönderim `now`'da.
    fn new_round(&mut self, now: Instant, deps: &mut BindDeps<'_>) {
        let mut nonce = [0u8; NONCE_LEN];
        if deps.entropy.fill(&mut nonce).is_err() {
            deps.counters.entropy_errors = deps.counters.entropy_errors.saturating_add(1);
            self.state = State::Backoff {
                until: now + ENTROPY_RETRY,
            };
            return;
        }
        let request_id = deps.ids.next();
        let frame = codec::encode_client_bind(request_id, &deps.cfg.device_cert_ref, &nonce);
        self.state = State::Round1 {
            nonce,
            request_id,
            frame,
            sends: 0,
            next: now,
        };
    }

    /// Vadesi gelmiş gönderim ya da geri çekilme sonu. En çok bir çerçeve döner.
    pub(crate) fn on_tick(&mut self, now: Instant, deps: &mut BindDeps<'_>) -> Option<Vec<u8>> {
        match &mut self.state {
            State::Round1 {
                frame, sends, next, ..
            }
            | State::Round2 {
                frame, sends, next, ..
            } => {
                if now < *next {
                    return None;
                }
                if *sends < BIND_ROUND_SENDS {
                    *sends += 1;
                    *next = now + BIND_RESEND;
                    return Some(frame.clone());
                }
                // Tur tükendi: T3 dizisiyle bekle (+ jitter, tohum = o turun client_nonce'u).
                let seed = match &self.state {
                    State::Round1 { nonce, .. } | State::Round2 { nonce, .. } => *nonce,
                    _ => [0; NONCE_LEN],
                };
                deps.counters.bind_rounds_exhausted =
                    deps.counters.bind_rounds_exhausted.saturating_add(1);
                let step = t3_delay(self.exhausted);
                let wait = step + jitter(&seed, self.exhausted, step);
                self.exhausted = self.exhausted.saturating_add(1);
                self.state = State::Backoff { until: now + wait };
                None
            }
            State::Backoff { until } => {
                if now < *until {
                    return None;
                }
                self.new_round(now, deps);
                self.on_tick(now, deps)
            }
            State::Bound { .. } => None,
        }
    }

    /// CLIENT_BIND / BIND_PROOF yanıtı ya da hata çerçevesi.
    pub(crate) fn on_response(
        &mut self,
        now: Instant,
        rpc: &codec::RpcFrame<'_>,
        deps: &mut BindDeps<'_>,
    ) -> BindOutcome {
        let c = &mut *deps.counters;
        match (&self.state, rpc.method_id) {
            (
                State::Round1 {
                    nonce, request_id, ..
                },
                method::CLIENT_BIND,
            ) if *request_id == rpc.request_id => {
                if rpc.flags == flags::RESPONSE_ERROR {
                    c.bind_error_hints = c.bind_error_hints.saturating_add(1);
                    return BindOutcome::Nothing;
                }
                let Ok(hub_nonce) = codec::decode_challenge(rpc.payload) else {
                    c.rx_malformed = c.rx_malformed.saturating_add(1);
                    return BindOutcome::Nothing;
                };
                let nonce = *nonce;
                let tbs = codec::bind_proof_tbs(
                    &deps.cfg.hub_ed25519_pub,
                    &deps.cfg.device_cert_ref,
                    &nonce,
                    &hub_nonce,
                );
                let sig = deps.signer.sign_bind(&tbs);
                let request_id = deps.ids.next();
                let frame = codec::encode_bind_proof(
                    request_id,
                    &deps.cfg.device_cert_ref,
                    &nonce,
                    &hub_nonce,
                    &sig,
                );
                self.state = State::Round2 {
                    nonce,
                    hub_nonce,
                    request_id,
                    frame: frame.clone(),
                    sends: 1,
                    next: now + BIND_RESEND,
                };
                BindOutcome::Send(frame)
            }
            (
                State::Round2 {
                    nonce,
                    hub_nonce,
                    request_id,
                    ..
                },
                method::BIND_PROOF,
            ) if *request_id == rpc.request_id => {
                if rpc.flags == flags::RESPONSE_ERROR {
                    if codec::decode_error(rpc.payload) == Ok(code::BAD_COOKIE) {
                        // 1. tura dön: yeni nonce, yeni cookie; eski kanıt bir daha gitmez.
                        c.bind_bad_cookie = c.bind_bad_cookie.saturating_add(1);
                        self.new_round(now, deps);
                        return self
                            .on_tick(now, deps)
                            .map_or(BindOutcome::Nothing, BindOutcome::Send);
                    }
                    // POSSESSION_PROOF_FAIL / RATE_LIMITED / HUB_CAPACITY / bozuk: yalnız ipucu (C8).
                    c.bind_error_hints = c.bind_error_hints.saturating_add(1);
                    return BindOutcome::Nothing;
                }
                let Ok(sig) = codec::decode_bind_ack(rpc.payload) else {
                    c.rx_malformed = c.rx_malformed.saturating_add(1);
                    return BindOutcome::Nothing;
                };
                let tbs = codec::bind_ack_tbs(
                    &deps.cfg.device_cert_ref,
                    nonce,
                    hub_nonce,
                    codec::BIND_OK,
                    code::OK,
                );
                if verify_strict(&deps.cfg.hub_ed25519_pub, &tbs, &sig).is_err() {
                    // Bağlı sayılmaz; tur kendi kadansıyla sürer.
                    c.bind_bad_signature = c.bind_bad_signature.saturating_add(1);
                    return BindOutcome::Nothing;
                }
                c.binds_accepted = c.binds_accepted.saturating_add(1);
                self.state = State::Bound { nonce: *nonce };
                self.exhausted = 0;
                BindOutcome::Bound
            }
            _ => {
                c.bind_unmatched = c.bind_unmatched.saturating_add(1);
                BindOutcome::Nothing
            }
        }
    }
}
