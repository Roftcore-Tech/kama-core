//! Bind ve heartbeat (S4 §2.4(a)(b), H2, H3, H8, H11; KM-HUB-TDD-001 §3 `on_bind1` / `on_bind2` /
//! `on_heartbeat`).
//!
//! ```text
//! 1. tur  CLIENT_BIND{cert_ref, client_nonce} → {CHALLENGE, hub_nonce = cookie}    hub HİÇBİR ŞEY yazmaz
//! 2. tur  BIND_PROOF{cert_ref, client_nonce, hub_nonce, imza} → {bind_ok = 1, hub imzası}
//! ```
//! **Cookie** = `BLAKE2s-256(cookie_secret ‖ fam ‖ ip ‖ port ‖ cert_ref ‖ client_nonce ‖ minute ‖
//! bind_epoch)[0..16]`. `bind_epoch` ucun kabul edilmiş bind sayısıdır: her kabul, ondan önce verilmiş
//! BÜTÜN cookie'leri geçersiz kılar → eski/gecikmiş/yeniden oynatılmış kanıt adresi geri taşıyamaz,
//! hatırlanacak nonce listesi yoktur.
//!
//! **2. tur sırası** (S4 §2.4(b) 1–6; ucuzdan pahalıya): (1) `cert_ref` dizinde değilse SESSİZ →
//! (2) kayıp ACK tekrarı (son kabul edilen kanıt, aynı adres) → bayt-özdeş yanıt, durum değişmez →
//! (3) cookie[şu anki epoch]; yanlışsa `BAD_COOKIE`, HİÇBİR durum/sayaç yazılmaz → (4) kaynak adres
//! kilidi (sessiz) + küresel `VERIFY_BUDGET` (`RATE_LIMITED`) → (5) imza; yanlışsa
//! `POSSESSION_PROOF_FAIL` ve `bind_fail[src] += 1` → (6) kabul.

use core::fmt;
use core::time::Duration;
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;

use blake2::{Blake2s256, Digest};
use ed25519_dalek::{Signer, SigningKey};
use kama_noise_wire::ed25519_strict::verify_strict;
use kama_noise_wire::entropy::EntropySource;

use crate::config::{HubSecret, Limits};
use crate::stats::{self, ShortId};
use crate::table::LastAccepted;
use crate::wire::{
    bind_ack_tbs, bind_proof_tbs, code, cookie_input, frame, BindProofRequest, BindProofResponse,
    CertRef, ClientBindRequest, ClientBindResponse, Nonce, Payload, RpcFrame, Signature,
    ECHO_TOKEN_LEN, NONCE_LEN, PUBLIC_KEY_LEN, SECRET_LEN,
};
use crate::{Clock, EntropyUnavailable, Hub, Sink};

/// Cookie'nin bağlandığı zaman dilimi (dakika) ve kabul edilen dilimler: bu dakika + bir önceki
/// (`COOKIE_WINDOW` = 2 dakika).
pub const COOKIE_MINUTE: Duration = Duration::from_secs(60);

/// Cookie sırları: geçerli ve (yenilemeden sonra) önceki.
pub struct CookieJar {
    current: [u8; SECRET_LEN],
    previous: Option<[u8; SECRET_LEN]>,
    since: Duration,
}

impl fmt::Debug for CookieJar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CookieJar(<redacted>)")
    }
}

impl CookieJar {
    #[must_use]
    pub fn new(secret: [u8; SECRET_LEN], now: Duration) -> Self {
        Self {
            current: secret,
            previous: None,
            since: now,
        }
    }

    /// `COOKIE_ROTATE` dolduysa yeni sır; önceki saklanır. Rastgelelik yoksa eskisi kalır. Yenilendiyse
    /// `true`.
    pub fn rotate_if_due(
        &mut self,
        now: Duration,
        period: Duration,
        rng: &mut dyn EntropySource,
    ) -> bool {
        if now.saturating_sub(self.since) < period {
            return false;
        }
        let mut fresh = [0u8; SECRET_LEN];
        if rng.fill(&mut fresh).is_err() {
            return false;
        }
        self.previous = Some(core::mem::replace(&mut self.current, fresh));
        self.since = now;
        true
    }

    /// Bu dakikanın cookie'si (geçerli sırla). Durum TUTMAZ.
    #[must_use]
    pub fn issue(
        &self,
        src: SocketAddr,
        cert_ref: &CertRef,
        client_nonce: &Nonce,
        now: Duration,
        epoch: u32,
    ) -> Nonce {
        cookie(
            &self.current,
            src,
            cert_ref,
            client_nonce,
            minute_of(now),
            epoch,
        )
    }

    /// `hub_nonce` bu ya da bir önceki dakikada, geçerli ya da önceki sırla, verilen epoch için mi
    /// üretilmiş (≤ 4 BLAKE2s; erken çıkış yok).
    #[must_use]
    pub fn check(
        &self,
        src: SocketAddr,
        cert_ref: &CertRef,
        client_nonce: &Nonce,
        hub_nonce: &Nonce,
        now: Duration,
        epoch: u32,
    ) -> bool {
        let newest = minute_of(now);
        let minutes = [Some(newest), newest.checked_sub(1)];
        let mut matched = false;
        for minute in minutes.into_iter().flatten() {
            for secret in core::iter::once(&self.current).chain(self.previous.as_ref()) {
                let expected = cookie(secret, src, cert_ref, client_nonce, minute, epoch);
                matched |= constant_time_eq(&expected, hub_nonce);
            }
        }
        matched
    }
}

/// Hub açılışından beri geçen dakika (monoton saat).
fn minute_of(now: Duration) -> u32 {
    u32::try_from(now.as_secs() / COOKIE_MINUTE.as_secs()).unwrap_or(u32::MAX)
}

/// S4 §2.4(a) cookie.
#[must_use]
pub fn cookie(
    secret: &[u8; SECRET_LEN],
    src: SocketAddr,
    cert_ref: &CertRef,
    client_nonce: &Nonce,
    minute: u32,
    epoch: u32,
) -> Nonce {
    let (input, len) = cookie_input(secret, src, cert_ref, client_nonce, minute, epoch);
    let digest = Blake2s256::digest(input.get(..len).unwrap_or(&input));
    let mut nonce = [0u8; NONCE_LEN];
    for (slot, byte) in nonce.iter_mut().zip(digest.iter()) {
        *slot = *byte;
    }
    nonce
}

fn constant_time_eq(a: &Nonce, b: &Nonce) -> bool {
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Kaynak adres başına başarısız imza kilidi (`bind_fail`; S4 H11): yalnız GEÇERLİ cookie'den sonra
/// yazılır; en çok `BIND_FAIL_ENTRIES` girdi (dolunca en eski dokunulan atılır, LRU); girdi ilk
/// başarısızlıktan `BIND_FAIL_HOLD` sonra düşer. `BIND_FAIL_MAX` başarısızlıkta adres o süre boyunca
/// sessizdir.
#[derive(Debug, Default)]
pub struct BindFailTable {
    entries: HashMap<SocketAddr, FailEntry>,
    order: BTreeMap<u64, SocketAddr>,
    clock: u64,
}

#[derive(Debug, Clone, Copy)]
struct FailEntry {
    fails: u32,
    since: Duration,
    stamp: u64,
}

impl BindFailTable {
    /// Adres kilitli mi.
    #[must_use]
    pub fn held(&self, src: SocketAddr, now: Duration, limits: &Limits) -> bool {
        self.entries.get(&src).is_some_and(|entry| {
            entry.fails >= limits.bind_fail_max
                && now.saturating_sub(entry.since) < limits.bind_fail_hold
        })
    }

    /// Bir başarısızlık yazar.
    pub fn hit(&mut self, src: SocketAddr, now: Duration, limits: &Limits) {
        self.clock = self.clock.wrapping_add(1);
        let stamp = self.clock;
        let fresh = FailEntry {
            fails: 1,
            since: now,
            stamp,
        };
        let entry = match self.entries.get(&src).copied() {
            Some(old) => {
                self.order.remove(&old.stamp);
                if now.saturating_sub(old.since) >= limits.bind_fail_hold {
                    fresh
                } else {
                    let fails = old.fails.saturating_add(1);
                    FailEntry {
                        fails,
                        // Kilit (BIND_FAIL_HOLD) sınıra ULAŞILDIĞI andan sayılır.
                        since: if fails == limits.bind_fail_max {
                            now
                        } else {
                            old.since
                        },
                        stamp,
                    }
                }
            }
            None => {
                while self.entries.len() >= limits.bind_fail_entries {
                    let Some((_, oldest)) = self.order.pop_first() else {
                        break;
                    };
                    self.entries.remove(&oldest);
                }
                fresh
            }
        };
        self.entries.insert(src, entry);
        self.order.insert(stamp, src);
    }

    /// `BIND_FAIL_HOLD` dolan girdileri siler.
    pub fn sweep(&mut self, now: Duration, limits: &Limits) {
        let order = &mut self.order;
        self.entries.retain(|_, entry| {
            let keep = now.saturating_sub(entry.since) < limits.bind_fail_hold;
            if !keep {
                order.remove(&entry.stamp);
            }
            keep
        });
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Hub'ın kimliği. `Debug` gizli anahtarı basmaz.
pub struct HubIdentity {
    signing: SigningKey,
    public: [u8; PUBLIC_KEY_LEN],
}

impl fmt::Debug for HubIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HubIdentity(<redacted>)")
    }
}

impl HubIdentity {
    #[must_use]
    pub fn new(secret: &HubSecret) -> Self {
        let signing = secret.signing_key();
        let public = signing.verifying_key().to_bytes();
        Self { signing, public }
    }

    #[must_use]
    pub fn public(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.public
    }

    /// Başarılı bind yanıtının imzası.
    #[must_use]
    pub fn sign_ack(
        &self,
        cert_ref: &CertRef,
        client_nonce: &Nonce,
        hub_nonce: &Nonce,
    ) -> Signature {
        let tbs = bind_ack_tbs(
            cert_ref,
            client_nonce,
            hub_nonce,
            crate::wire::BIND_OK,
            code::OK,
        );
        self.signing.sign(&tbs).to_bytes()
    }
}

/// Rastgele sır üretir.
pub(crate) fn secret(rng: &mut dyn EntropySource) -> Result<[u8; SECRET_LEN], EntropyUnavailable> {
    let mut out = [0u8; SECRET_LEN];
    rng.fill(&mut out).map_err(|_| EntropyUnavailable)?;
    Ok(out)
}

impl<C: Clock, R: EntropySource> Hub<C, R> {
    /// `0x1B HEARTBEAT` (H8): yalnız bağlı adrese aynı uzunlukta ACK; slot yaratmaz, adres
    /// değiştirmez, canlılık kararı vermez.
    pub(crate) fn on_heartbeat(
        &mut self,
        src: SocketAddr,
        body: &[u8],
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        if !(body.is_empty() || body.len() == ECHO_TOKEN_LEN) {
            stats::bump(&mut self.stats.drop_malformed);
            return;
        }
        let Some(me) = self.table.key_at(src) else {
            stats::bump(&mut self.stats.hb_unbound);
            return;
        };
        if let Some(endpoint) = self.table.endpoint_mut(&me) {
            endpoint.last_activity = now;
        }
        self.reply_control(sink, src, frame::HEARTBEAT_ACK, body);
    }

    /// CLIENT_BIND (1. tur): hiçbir şey YAZMAZ.
    pub(crate) fn on_bind1(
        &mut self,
        src: SocketAddr,
        request: &RpcFrame<'_>,
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        let Ok(bind) = ClientBindRequest::decode(request.payload) else {
            stats::bump(&mut self.stats.drop_malformed);
            return;
        };
        let Some(device) = self.directory.by_cert_ref(&bind.cert_ref) else {
            stats::bump(&mut self.stats.bind_unknown);
            return;
        };
        let epoch = self.table.bind_epoch(&device.dest_hash);
        let hub_nonce = self
            .cookies
            .issue(src, &bind.cert_ref, &bind.client_nonce, now, epoch);
        self.reply_rpc(sink, src, request, &ClientBindResponse { hub_nonce });
    }

    /// BIND_PROOF (2. tur): S4 §2.4(b) 1–6, bu sırayla.
    pub(crate) fn on_bind2(
        &mut self,
        src: SocketAddr,
        request: &RpcFrame<'_>,
        now: Duration,
        sink: &mut dyn Sink,
    ) {
        let Ok(proof) = BindProofRequest::decode(request.payload) else {
            stats::bump(&mut self.stats.drop_malformed);
            return;
        };
        // (1) Dizin kapısı — sessiz.
        let Some((dest_hash, device_pub)) = self
            .directory
            .by_cert_ref(&proof.cert_ref)
            .map(|device| (device.dest_hash, device.ed25519_pub))
        else {
            stats::bump(&mut self.stats.bind_unknown);
            return;
        };
        // (2) Kayıp ACK tekrarı: son kabul edilen kanıt, aynı adresten → aynı yanıt, durum değişmez.
        let repeat = self.table.endpoint(&dest_hash).and_then(|endpoint| {
            let last = endpoint.last_accepted.as_ref()?;
            (last.client_nonce == proof.client_nonce
                && last.hub_nonce == proof.hub_nonce
                && last.addr == src
                && endpoint.addr == Some(src))
            .then(|| last.ack.clone())
        });
        if let Some(ack) = repeat {
            sink.send(src, &ack);
            stats::bump(&mut self.stats.tx_reply);
            stats::bump(&mut self.stats.bind_ack_repeat);
            return;
        }
        // (3) Cookie, ucun ŞU ANKİ epoch'uyla. Yanlışsa hiçbir durum ya da sayaç (bind_fail) yazılmaz.
        let epoch = self.table.bind_epoch(&dest_hash);
        if !self.cookies.check(
            src,
            &proof.cert_ref,
            &proof.client_nonce,
            &proof.hub_nonce,
            now,
            epoch,
        ) {
            stats::bump(&mut self.stats.bind_bad_cookie);
            self.reply_error(sink, src, request, code::BAD_COOKIE);
            return;
        }
        // (4) Adres kilidi (sessiz) ve küresel doğrulama bütçesi.
        if self.bind_fail.held(src, now, &self.limits) {
            stats::bump(&mut self.stats.bind_held);
            return;
        }
        if !self.verify_budget.take(self.limits.verify_budget, now) {
            stats::bump(&mut self.stats.rate_verify);
            self.reply_error(sink, src, request, code::RATE_LIMITED);
            return;
        }
        // (5) İmza.
        stats::bump(&mut self.stats.verify_calls);
        let tbs = bind_proof_tbs(
            self.identity.public(),
            &proof.cert_ref,
            &proof.client_nonce,
            &proof.hub_nonce,
        );
        if verify_strict(&device_pub, &tbs, &proof.signature).is_err() {
            self.bind_fail.hit(src, now, &self.limits);
            stats::bump(&mut self.stats.bind_fail);
            self.reply_error(sink, src, request, code::POSSESSION_PROOF_FAIL);
            return;
        }
        // (6) Kabul.
        let hub_signature =
            self.identity
                .sign_ack(&proof.cert_ref, &proof.client_nonce, &proof.hub_nonce);
        let Some(ack) = self.encode_rpc(
            request,
            crate::wire::flags::RESPONSE,
            &BindProofResponse { hub_signature },
        ) else {
            return;
        };
        let change = self.table.accept_bind(
            dest_hash,
            proof.cert_ref,
            src,
            now,
            LastAccepted {
                client_nonce: proof.client_nonce,
                hub_nonce: proof.hub_nonce,
                addr: src,
                ack: ack.clone(),
            },
        );
        sink.send(src, &ack);
        stats::bump(&mut self.stats.tx_reply);
        stats::bump(&mut self.stats.bind_ok);
        if change.moved {
            stats::bump(&mut self.stats.bind_moved);
        }
        if change.displaced {
            stats::bump(&mut self.stats.bind_displaced);
        }
        stats::log_bind_ok(
            sink,
            ShortId::of(&dest_hash),
            self.table.bind_epoch(&dest_hash),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::BIND_ACK_TBS_LEN;
    use std::net::{IpAddr, Ipv4Addr};

    struct Counter(u8);

    impl EntropySource for Counter {
        fn fill(&mut self, dst: &mut [u8]) -> Result<(), kama_noise_wire::entropy::EntropyError> {
            self.0 = self.0.wrapping_add(1);
            dst.fill(self.0);
            Ok(())
        }
    }

    struct Dry;

    impl EntropySource for Dry {
        fn fill(&mut self, _dst: &mut [u8]) -> Result<(), kama_noise_wire::entropy::EntropyError> {
            Err(kama_noise_wire::entropy::EntropyError)
        }
    }

    fn addr(n: u8, port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, n)), port)
    }

    const REF: CertRef = [0xbb; 32];
    const CN: Nonce = [0x11; 16];

    /// T-B13 — adres ailesi: IPv4 `fam = 0x04` + 4 B, IPv6 `0x06` + 16 B; `::ffff:a.b.c.d` ile
    /// `a.b.c.d` aynı cookie'yi üretir (S4 v1.3.1 §2.4(a)).
    #[test]
    fn t_b13_ipv4_mapped_ipv6_is_normalized_to_ipv4() {
        let s = [5u8; 32];
        let v4 = addr(7, 40_001);
        let mapped = SocketAddr::new(
            IpAddr::V6(Ipv4Addr::new(198, 51, 100, 7).to_ipv6_mapped()),
            40_001,
        );
        assert_eq!(
            cookie(&s, mapped, &REF, &CN, 3, 1),
            cookie(&s, v4, &REF, &CN, 3, 1)
        );
        let v6 = SocketAddr::new(IpAddr::V6("2001:db8::7".parse().expect("v6")), 40_001);
        assert_ne!(
            cookie(&s, v6, &REF, &CN, 3, 1),
            cookie(&s, v4, &REF, &CN, 3, 1)
        );
        let (input, len) = cookie_input(&s, mapped, &REF, &CN, 3, 1);
        assert_eq!(
            (input[32], len),
            (crate::wire::FAMILY_IPV4, 32 + 1 + 4 + 2 + 32 + 16 + 8)
        );
        let (input, len) = cookie_input(&s, v6, &REF, &CN, 3, 1);
        assert_eq!(
            (input[32], len),
            (crate::wire::FAMILY_IPV6, 32 + 1 + 16 + 2 + 32 + 16 + 8)
        );
    }

    #[test]
    fn cookie_binds_every_input() {
        let s = [5u8; 32];
        let base = cookie(&s, addr(7, 1), &REF, &CN, 10, 0);
        assert_ne!(cookie(&s, addr(8, 1), &REF, &CN, 10, 0), base, "ip");
        assert_ne!(cookie(&s, addr(7, 2), &REF, &CN, 10, 0), base, "port");
        assert_ne!(
            cookie(&s, addr(7, 1), &[0xbc; 32], &CN, 10, 0),
            base,
            "cert_ref"
        );
        assert_ne!(
            cookie(&s, addr(7, 1), &REF, &[0x12; 16], 10, 0),
            base,
            "client_nonce"
        );
        assert_ne!(cookie(&s, addr(7, 1), &REF, &CN, 11, 0), base, "dakika");
        assert_ne!(cookie(&s, addr(7, 1), &REF, &CN, 10, 1), base, "epoch");
        assert_ne!(cookie(&[6; 32], addr(7, 1), &REF, &CN, 10, 0), base, "sır");
    }

    #[test]
    fn window_is_this_minute_and_the_previous_one() {
        let jar = CookieJar::new([1; 32], Duration::ZERO);
        let n = jar.issue(addr(7, 1), &REF, &CN, Duration::from_secs(125), 3);
        for (t, ok) in [
            (125, true),
            (179, true),
            (180, true),
            (239, true),
            (240, false),
        ] {
            assert_eq!(
                jar.check(addr(7, 1), &REF, &CN, &n, Duration::from_secs(t), 3),
                ok,
                "t={t}"
            );
        }
        assert!(
            !jar.check(addr(7, 1), &REF, &CN, &n, Duration::from_secs(125), 4),
            "epoch ilerledi"
        );
    }

    #[test]
    fn rotation_keeps_the_previous_secret() {
        // Sayaç 1'den başlar: yeni sır [2; 32] olur (eskisi [1; 32]).
        let mut rng = Counter(1);
        let mut jar = CookieJar::new([1; 32], Duration::ZERO);
        let n = jar.issue(addr(7, 1), &REF, &CN, Duration::from_secs(3590), 0);
        assert!(!jar.rotate_if_due(
            Duration::from_secs(3599),
            Duration::from_secs(3600),
            &mut rng
        ));
        assert!(jar.rotate_if_due(
            Duration::from_secs(3600),
            Duration::from_secs(3600),
            &mut rng
        ));
        assert_ne!(
            jar.issue(addr(7, 1), &REF, &CN, Duration::from_secs(3590), 0),
            n
        );
        assert!(
            jar.check(addr(7, 1), &REF, &CN, &n, Duration::from_secs(3600), 0),
            "önceki sırla"
        );
        assert!(!jar.rotate_if_due(
            Duration::from_secs(9000),
            Duration::from_secs(3600),
            &mut Dry
        ));
    }

    #[test]
    fn fail_table_holds_after_max_and_is_bounded() {
        let limits = Limits {
            bind_fail_entries: 3,
            ..Limits::S4
        };
        let mut table = BindFailTable::default();
        let t = Duration::from_secs(100);
        for _ in 0..9 {
            table.hit(addr(1, 1), t, &limits);
        }
        assert!(!table.held(addr(1, 1), t, &limits));
        table.hit(addr(1, 1), t, &limits);
        assert!(table.held(addr(1, 1), t, &limits));
        assert!(table.held(addr(1, 1), t + Duration::from_secs(59), &limits));
        // Sınıra geç ulaşılsa da kilit tam süre sürer: 9 hata t'de, 10. t+59'da → t+118'e dek kilitli.
        let mut late = BindFailTable::default();
        for _ in 0..9 {
            late.hit(addr(9, 9), t, &limits);
        }
        late.hit(addr(9, 9), t + Duration::from_secs(59), &limits);
        assert!(late.held(addr(9, 9), t + Duration::from_secs(118), &limits));
        assert!(!late.held(addr(9, 9), t + Duration::from_secs(119), &limits));
        assert!(!table.held(addr(1, 1), t + Duration::from_secs(60), &limits));
        for n in 2..=6 {
            table.hit(addr(n, 1), t, &limits);
        }
        assert_eq!(table.len(), 3, "LRU tavanı");
        assert!(!table.held(addr(1, 1), t, &limits), "en eski atıldı");
        table.sweep(t + Duration::from_secs(60), &limits);
        assert!(table.is_empty());
    }

    #[test]
    fn ack_signature_covers_the_documented_text() {
        let seed = [0x42u8; 32];
        let identity = HubIdentity {
            signing: SigningKey::from_bytes(&seed),
            public: SigningKey::from_bytes(&seed).verifying_key().to_bytes(),
        };
        let hn = [0x22; 16];
        let sig = identity.sign_ack(&REF, &CN, &hn);
        let tbs: [u8; BIND_ACK_TBS_LEN] = bind_ack_tbs(&REF, &CN, &hn, 1, 0);
        assert_eq!(verify_strict(identity.public(), &tbs, &sig), Ok(()));
        assert!(!format!("{identity:?}").contains("Signing"));
    }
}
