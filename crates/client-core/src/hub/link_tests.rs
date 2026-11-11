//! `HubLink` birim testleri — sahte zaman, deterministik entropi (KK-H33).
//!
//! Hub imzaları bu crate'te ÜRETİLEMEZ (client-core'da imza üretici bağımlılık yok; KK-H43). Hub
//! yanıtlarının imzaları Rust'tan bağımsız Python referansıyla (`ref/crypto/ed25519.py`, RFC 8032)
//! üretilmiş test girdileridir — beklenen çıktı DEĞİL, sahte hub'ın girdisidir. Yeniden üretim:
//!
//! ```text
//! sk = 0x11 × 32; cert_ref = 0xC1 × 32
//! tbs = b"KAMA-HUB-ACK-V1\0" ‖ cert_ref ‖ [c]×16 ‖ [hn]×16 ‖ 01 ‖ 0000
//! sig = ed25519.sign(sk, tbs)      # c ∈ 3..=9, hn ∈ {0xA0, 0xA1}
//! ```

use super::*;
use kama_contracts::s4;

/// S4 sabitleri (`kama_contracts::s4`, KK-H34) — test gövdelerinin kısa adları.
mod code {
    pub use kama_contracts::s4::{
        ERROR_BAD_COOKIE as BAD_COOKIE, ERROR_BAD_REQUEST as BAD_REQUEST,
        ERROR_HUB_CAPACITY as HUB_CAPACITY, ERROR_POSSESSION_PROOF_FAIL as POSSESSION_PROOF_FAIL,
        ERROR_RATE_LIMITED as RATE_LIMITED, STATUS_CHALLENGE as CHALLENGE, STATUS_OK as OK,
        STATUS_PEER_UNBOUND as PEER_UNBOUND, STATUS_UNKNOWN_HANDLE as UNKNOWN_HANDLE,
    };
}
mod envelope {
    pub use kama_contracts::s4::{
        ENVELOPE_RELAY_OPEN as RELAY_OPEN, ENVELOPE_SESSION_INIT as SESSION_INIT,
        ENVELOPE_SESSION_INIT_RESPONSE as SESSION_INIT_RESPONSE,
    };
}
mod flags {
    pub const REQUEST: u8 = 0x00;
    pub const RESPONSE: u8 = kama_contracts::s4::FLAG_RESPONSE;
    pub const RESPONSE_ERROR: u8 =
        kama_contracts::s4::FLAG_RESPONSE | kama_contracts::s4::FLAG_ERROR;
}
mod frame {
    pub use kama_contracts::s4::{
        FRAME_HEARTBEAT as HEARTBEAT, FRAME_HEARTBEAT_ACK as HEARTBEAT_ACK,
        FRAME_PLANE1_RPC as PLANE1_RPC,
    };
}
mod method {
    pub use kama_contracts::s4::{
        METHOD_BIND_PROOF as BIND_PROOF, METHOD_CLIENT_BIND as CLIENT_BIND,
        METHOD_DELIVER as DELIVER, METHOD_ROUTE_MISS as ROUTE_MISS,
        METHOD_STORE_MESSAGE as STORE_MESSAGE,
    };
}
use kama_noise_wire::entropy::EntropyError;
use std::net::{Ipv4Addr, SocketAddr};

const HUB_PUB: &str = "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737";
const CERT: [u8; 32] = [0xC1; 32];
/// `(client_nonce baytı, hub_nonce baytı, imza)`.
const ACK_SIGS: &[(u8, u8, &str)] = &[
    (0x03, 0xA0, "2dade184506faa02fba09416680ab59bfa355f585272b043f70f054180bcfa9cfe5ff25c90bd2dbe0f2cc6367dbf515f0ff27bbb16ced4ced86e262177791402"),
    (0x04, 0xA0, "4d3a925d6d69da061ca335ed6dd7eff8c87fe2a62e76d92a4d4c7807680914be4826f223ac5ac0c564a30f0d5a9a06dac0a113140e1fb0fd3691e09400862f09"),
    (0x05, 0xA0, "76b92ca08e3de1bb3174e92e5f570b56c947a0d37e4fdd3f8183a5868d977602383d463c8acf8500bd849e51f3f48fc89d14461d82ad37ce0b5331cfac390f00"),
    (0x06, 0xA0, "4162976ddc9e36214275752504d41ba1e961f909118deb15d8c557e5fdeacfc92db7c1d48450c60d2d1098c73873040ce36649dca93bed2e92e325ee81769609"),
    (0x04, 0xA1, "13d0d89ccc03bfecccfbfe9b4dfa3c7e1e670f9e05aa696f2f13af9d8569738d3d974569b99667aa7c33d2c67746dfba55f02b540b5345d85465859524d3fe07"),
];

fn hex32(s: &str) -> [u8; 32] {
    hex(s).try_into().unwrap()
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn ack_sig(c: u8, hn: u8) -> Vec<u8> {
    let (_, _, s) = ACK_SIGS
        .iter()
        .find(|(a, b, _)| *a == c && *b == hn)
        .expect("fikstür");
    hex(s)
}

/// Her `fill` çağrısında sayaç +1, tampon o değerle dolar: rid tabanı [1;4], token tabanı [2;4],
/// 1. client_nonce [3;16], 2. [4;16], …
struct SeqEntropy(u8);
impl EntropySource for SeqEntropy {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        self.0 += 1;
        dst.fill(self.0);
        Ok(())
    }
}

/// TBS'yi kaydeder, sabit imza döner (hub testte imzayı doğrulamaz).
struct RecSigner(Arc<Mutex<Vec<[u8; 112]>>>);
impl BindSigner for RecSigner {
    fn sign_bind(&self, tbs: &[u8; 112]) -> [u8; 64] {
        self.0.lock().unwrap().push(*tbs);
        [0x5A; 64]
    }
}

fn cfg() -> HubConfig {
    HubConfig {
        hub_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 7000)),
        hub_ed25519_pub: hex32(HUB_PUB),
        device_cert_ref: CERT,
        self_dest_hash: [0xD0; 32],
    }
}

fn s(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

struct T {
    link: HubLink,
    t0: Instant,
    tbs: Arc<Mutex<Vec<[u8; 112]>>>,
}

fn fresh() -> T {
    let t0 = Instant::now();
    let tbs = Arc::new(Mutex::new(Vec::new()));
    let link = HubLink::new(
        cfg(),
        Box::new(RecSigner(tbs.clone())),
        Box::new(SeqEntropy(0)),
        t0,
    )
    .unwrap();
    T { link, t0, tbs }
}

/// Giden çerçevenin alanları.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Out {
    method: u8,
    rid: u32,
    payload: Vec<u8>,
    raw: Vec<u8>,
}

fn parse(f: &[u8]) -> Out {
    assert_eq!(f[..4], [0; 4]);
    if f[4] == frame::HEARTBEAT {
        return Out {
            method: frame::HEARTBEAT,
            rid: u32::from_be_bytes(f[5..9].try_into().unwrap()),
            payload: vec![],
            raw: f.to_vec(),
        };
    }
    assert_eq!(f[4], frame::PLANE1_RPC);
    assert_eq!(f[6], flags::REQUEST);
    Out {
        method: f[5],
        rid: u32::from_be_bytes(f[7..11].try_into().unwrap()),
        payload: f[15..].to_vec(),
        raw: f.to_vec(),
    }
}

fn drain(l: &mut HubLink, now: Instant) -> Vec<Out> {
    std::iter::from_fn(|| l.poll_transmit(now))
        .map(|f| parse(&f))
        .collect()
}

fn rpc(m: u8, fl: u8, rid: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0, 0, 0, 0, frame::PLANE1_RPC, m, fl];
    v.extend_from_slice(&rid.to_be_bytes());
    v.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    v.extend_from_slice(payload);
    v
}

fn challenge(rid: u32, hn: u8) -> Vec<u8> {
    let mut p = vec![0u8; 17];
    p.extend_from_slice(&code::CHALLENGE.to_be_bytes());
    p.extend_from_slice(&[hn; 16]);
    rpc(method::CLIENT_BIND, flags::RESPONSE, rid, &p)
}

fn bind_ack(rid: u32, sig: &[u8]) -> Vec<u8> {
    let mut p = vec![1u8];
    p.extend_from_slice(&[0; 16]);
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(sig);
    rpc(method::BIND_PROOF, flags::RESPONSE, rid, &p)
}

fn err(m: u8, rid: u32, c: u16) -> Vec<u8> {
    let mut p = c.to_be_bytes().to_vec();
    p.extend_from_slice(&[0, 0]);
    rpc(m, flags::RESPONSE_ERROR, rid, &p)
}

fn store_resp(rid: u32, status: u16, h: u32) -> Vec<u8> {
    let mut p = status.to_be_bytes().to_vec();
    p.extend_from_slice(&h.to_be_bytes());
    rpc(method::STORE_MESSAGE, flags::RESPONSE, rid, &p)
}

fn deliver(h: u32, sender: [u8; 32], ty: u8, env: &[u8]) -> Vec<u8> {
    let mut p = h.to_be_bytes().to_vec();
    p.extend_from_slice(&sender);
    p.push(ty);
    p.extend_from_slice(&u32::try_from(env.len()).unwrap().to_be_bytes());
    p.extend_from_slice(env);
    rpc(method::DELIVER, flags::REQUEST, 0, &p)
}

fn route_miss(h: u32) -> Vec<u8> {
    let mut p = h.to_be_bytes().to_vec();
    p.push(1);
    rpc(method::ROUTE_MISS, flags::REQUEST, 0, &p)
}

fn hb_ack(token: u32) -> Vec<u8> {
    let mut v = vec![0, 0, 0, 0, frame::HEARTBEAT_ACK];
    v.extend_from_slice(&token.to_be_bytes());
    v
}

/// İki turu `now`'da tamamlar; dönen değer: bağlandığı an.
fn complete_bind(l: &mut HubLink, now: Instant, nonce: u8) {
    let out = drain(l, now);
    let b1 = out
        .iter()
        .find(|o| o.method == method::CLIENT_BIND)
        .unwrap();
    assert_eq!(b1.payload[34..50], [nonce; 16], "client_nonce");
    l.on_control(now, &challenge(b1.rid, 0xA0));
    let out = drain(l, now);
    let b2 = out.iter().find(|o| o.method == method::BIND_PROOF).unwrap();
    l.on_control(now, &bind_ack(b2.rid, &ack_sig(nonce, 0xA0)));
    assert!(l.is_bound());
}

fn bound() -> T {
    let mut t = fresh();
    let t0 = t.t0;
    complete_bind(&mut t.link, t0, 3);
    assert_eq!(t.link.take_notice(), Some(HubNotice::Bound));
    t
}

fn peer(b: u8) -> PeerRef {
    PeerRef::from_dest_hash([b; 32])
}

fn events(l: &mut HubLink) -> Vec<LinkEvent> {
    std::iter::from_fn(|| l.poll_event()).collect()
}

fn notices(l: &mut HubLink) -> Vec<HubNotice> {
    std::iter::from_fn(|| l.take_notice()).collect()
}

// ── C2: PoC-BIND ─────────────────────────────────────────────────────────────────────────────

#[test]
fn c2_bind_two_rounds_tbs_and_verify_strict() {
    let mut t = fresh();
    let t0 = t.t0;
    let out = drain(&mut t.link, t0);
    assert_eq!(out.len(), 1);
    let b1 = &out[0];
    assert_eq!(b1.method, method::CLIENT_BIND);
    assert_eq!(b1.rid, 0x0101_0101, "request_id entropiden");
    // device_cert_mode 0 · cert_ref · presence 0 · client_nonce
    assert_eq!(b1.payload[0], 0);
    assert_eq!(b1.payload[1..33], CERT);
    assert_eq!(b1.payload[33], 0);
    assert_eq!(b1.payload[34..50], [3; 16]);
    assert!(!t.link.is_bound());

    t.link.on_control(t0, &challenge(b1.rid, 0xA0));
    let out = drain(&mut t.link, t0);
    let b2 = &out[0];
    assert_eq!(b2.method, method::BIND_PROOF);
    assert_eq!(b2.rid, b1.rid + 1, "her yeni istekte +1");
    assert_eq!(b2.payload[..32], CERT);
    assert_eq!(b2.payload[32..48], [3; 16]);
    assert_eq!(b2.payload[48..64], [0xA0; 16]);
    assert_eq!(b2.payload[64..128], [0x5A; 64], "BindSigner imzası");
    // TBS alan sırası S4 §2.4(b): domain ‖ hub_pub ‖ cert_ref ‖ client_nonce ‖ hub_nonce.
    let tbs = t.tbs.lock().unwrap()[0];
    assert_eq!(&tbs[..16], b"KAMA-HUB-BIND-V1");
    assert_eq!(tbs[16..48], hex32(HUB_PUB));
    assert_eq!(tbs[48..80], CERT);
    assert_eq!(tbs[80..96], [3; 16]);
    assert_eq!(tbs[96..112], [0xA0; 16]);

    // Yanlış hub imzası → bağlı değil.
    let mut bad = ack_sig(3, 0xA0);
    bad[0] ^= 1;
    t.link.on_control(t0, &bind_ack(b2.rid, &bad));
    assert!(!t.link.is_bound());
    assert_eq!(t.link.counters().bind_bad_signature, 1);
    // Başka nonce'un imzası da geçmez.
    t.link.on_control(t0, &bind_ack(b2.rid, &ack_sig(4, 0xA0)));
    assert!(!t.link.is_bound());
    // Doğru imza → bağlı.
    t.link.on_control(t0, &bind_ack(b2.rid, &ack_sig(3, 0xA0)));
    assert!(t.link.is_bound());
    assert_eq!(notices(&mut t.link), vec![HubNotice::Bound]);
}

#[test]
fn c2_lost_reply_resends_same_proof_three_times_one_second_apart() {
    let mut t = fresh();
    let t0 = t.t0;
    let b1 = drain(&mut t.link, t0).remove(0);
    t.link.on_control(t0, &challenge(b1.rid, 0xA0));
    let first = drain(&mut t.link, t0).remove(0);
    assert_eq!(drain(&mut t.link, t0 + ms(999)), vec![]);
    let r2 = drain(&mut t.link, t0 + s(1));
    let r3 = drain(&mut t.link, t0 + s(2));
    assert_eq!(r2, vec![first.clone()], "bayt-özdeş");
    assert_eq!(r3, vec![first.clone()]);
    // Üç gönderimden sonra o kanıt bir daha gitmez; tur tükenir, T3 + jitter bekler.
    assert_eq!(drain(&mut t.link, t0 + s(3)), vec![]);
    assert_eq!(t.link.counters().bind_rounds_exhausted, 1);
    let next = t.link.next_deadline().unwrap();
    assert!(
        next >= t0 + s(4) && next < t0 + s(4) + ms(250),
        "1 s + %25 pencere"
    );
    let again = drain(&mut t.link, next);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].method, method::CLIENT_BIND);
    assert_eq!(
        again[0].payload[34..50],
        [4; 16],
        "her denemede yeni client_nonce"
    );
    assert_ne!(again[0].rid, b1.rid);
}

#[test]
fn c2_client_bind_round_resends_then_new_nonce() {
    let mut t = fresh();
    let t0 = t.t0;
    let a = drain(&mut t.link, t0).remove(0);
    assert_eq!(drain(&mut t.link, t0 + s(1)), vec![a.clone()]);
    assert_eq!(drain(&mut t.link, t0 + s(2)), vec![a.clone()]);
    assert_eq!(drain(&mut t.link, t0 + s(3)), vec![]);
    let d = t.link.next_deadline().unwrap();
    let b = drain(&mut t.link, d).remove(0);
    assert_eq!(b.payload[34..50], [4; 16]);
    // Geç gelen eski meydan okuma (eski request_id) yeni denemeye atanmaz.
    t.link.on_control(d, &challenge(a.rid, 0xA0));
    assert_eq!(drain(&mut t.link, d), vec![]);
    assert_eq!(t.link.counters().bind_unmatched, 1);
    complete_bind_from(&mut t.link, d, &b, 4);
}

fn complete_bind_from(l: &mut HubLink, now: Instant, b1: &Out, nonce: u8) {
    l.on_control(now, &challenge(b1.rid, 0xA0));
    let b2 = drain(l, now).remove(0);
    l.on_control(now, &bind_ack(b2.rid, &ack_sig(nonce, 0xA0)));
    assert!(l.is_bound());
}

#[test]
fn c2_bad_cookie_returns_to_round_one_and_old_proof_never_resent() {
    let mut t = fresh();
    let t0 = t.t0;
    let b1 = drain(&mut t.link, t0).remove(0);
    t.link.on_control(t0, &challenge(b1.rid, 0xA0));
    let proof = drain(&mut t.link, t0).remove(0);
    t.link
        .on_control(t0, &err(method::BIND_PROOF, proof.rid, code::BAD_COOKIE));
    let r1 = drain(&mut t.link, t0);
    assert_eq!(r1.len(), 1);
    assert_eq!(r1[0].method, method::CLIENT_BIND);
    assert_eq!(r1[0].payload[34..50], [4; 16], "yeni nonce");
    assert_eq!(t.link.counters().bind_bad_cookie, 1);
    // Eski kanıt hiçbir zamanda yeniden gitmez.
    for k in 1..40 {
        let out = drain(&mut t.link, t0 + s(k));
        assert!(out.iter().all(|o| o.raw != proof.raw));
    }
}

#[test]
fn c2_bad_cookie_then_bind_with_new_cookie() {
    let mut t = fresh();
    let t0 = t.t0;
    let b1 = drain(&mut t.link, t0).remove(0);
    t.link.on_control(t0, &challenge(b1.rid, 0xA0));
    let proof = drain(&mut t.link, t0).remove(0);
    t.link
        .on_control(t0, &err(method::BIND_PROOF, proof.rid, code::BAD_COOKIE));
    let b1b = drain(&mut t.link, t0).remove(0);
    t.link.on_control(t0, &challenge(b1b.rid, 0xA1));
    let p2 = drain(&mut t.link, t0).remove(0);
    assert_eq!(p2.payload[32..48], [4; 16]);
    assert_eq!(p2.payload[48..64], [0xA1; 16]);
    t.link.on_control(t0, &bind_ack(p2.rid, &ack_sig(4, 0xA1)));
    assert!(t.link.is_bound());
}

#[test]
fn c8_unsigned_bind_errors_do_not_drop_bind() {
    let mut t = fresh();
    let t0 = t.t0;
    let b1 = drain(&mut t.link, t0).remove(0);
    t.link
        .on_control(t0, &err(method::CLIENT_BIND, b1.rid, code::RATE_LIMITED));
    // Tur sürer: aynı istek 1 s sonra.
    assert_eq!(drain(&mut t.link, t0 + s(1)), vec![b1.clone()]);
    t.link.on_control(t0 + s(1), &challenge(b1.rid, 0xA0));
    let p = drain(&mut t.link, t0 + s(1)).remove(0);
    for c in [
        code::POSSESSION_PROOF_FAIL,
        code::RATE_LIMITED,
        code::HUB_CAPACITY,
    ] {
        t.link
            .on_control(t0 + s(1), &err(method::BIND_PROOF, p.rid, c));
    }
    assert_eq!(t.link.counters().bind_error_hints, 4);
    assert_eq!(
        drain(&mut t.link, t0 + s(2)),
        vec![p.clone()],
        "kanıt tekrarı sürer"
    );
    t.link
        .on_control(t0 + s(2), &bind_ack(p.rid, &ack_sig(3, 0xA0)));
    assert!(t.link.is_bound());
    // Bağlıyken sahte imzasız hata bind'i düşürmez.
    t.link
        .on_control(t0 + s(2), &err(method::BIND_PROOF, p.rid, code::BAD_COOKIE));
    t.link
        .on_control(t0 + s(2), &err(method::CLIENT_BIND, 9, code::BAD_REQUEST));
    assert!(t.link.is_bound());
}

#[test]
fn c2_nothing_but_bind_is_sent_before_bind() {
    let mut t = fresh();
    let t0 = t.t0;
    let _ = drain(&mut t.link, t0);
    let a = t.link.send_init(t0, peer(0xB0), 1, vec![1, 2, 3]).unwrap();
    assert_eq!(a, LinkAttempt(1));
    for k in 0..3 {
        let out = drain(&mut t.link, t0 + s(k));
        assert!(out.iter().all(|o| o.method == method::CLIENT_BIND));
    }
    // ≤ 16 bekleyen; 17.si reddedilir.
    for b in 1..16u8 {
        t.link.send_init(t0, peer(b), 1, vec![b]).unwrap();
    }
    assert_eq!(
        t.link.send_init(t0, peer(0x40), 1, vec![0]),
        Err(HubError::QueueFull)
    );
}

// ── C6 / C7 heartbeat ────────────────────────────────────────────────────────────────────────

#[test]
fn c6_heartbeat_only_after_outbound_silence_with_jitter() {
    let mut a = bound();
    let mut b = bound();
    let t0 = a.t0;
    let da = a.link.next_deadline().unwrap();
    let db = b.link.next_deadline().unwrap();
    assert!(da >= t0 + s(8) && da < t0 + s(10), "[8 s, 10 s)");
    assert_eq!(da - a.t0, db - b.t0, "aynı girdide aynı vade");
    assert_eq!(drain(&mut a.link, da - ms(1)), vec![]);
    // Giden oturum datagramı sessizliği bozar → vade kayar.
    a.link
        .on_session_traffic(da - ms(1), HubHandle::from_raw(9), true);
    assert_eq!(drain(&mut a.link, da), vec![]);
    let d2 = a.link.next_deadline().unwrap();
    assert!(d2 >= da - ms(1) + s(8));
    // Gelen trafik vadeyi değiştirmez (yalnız C9 yaşı).
    b.link
        .on_session_traffic(db - ms(1), HubHandle::from_raw(9), false);
    let hb = drain(&mut b.link, db);
    assert_eq!(hb.len(), 1);
    assert_eq!(hb[0].method, frame::HEARTBEAT);
    assert_eq!(hb[0].rid, 0x0202_0202, "token entropi tabanından");
    assert_eq!(b.link.last_rx_age(db), Some(ms(1)));
}

#[test]
fn c6_unanswered_probe_disconnects_then_rebinds() {
    let mut t = bound();
    let d = t.link.next_deadline().unwrap();
    let hb = drain(&mut t.link, d).remove(0);
    // ACK 1 s içinde yok → 3 × 1 s yoklama.
    let mut tokens = vec![hb.rid];
    for k in 1..=3 {
        let out = drain(&mut t.link, d + s(k));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].method, frame::HEARTBEAT);
        tokens.push(out[0].rid);
    }
    assert!(t.link.is_bound(), "yoklama sürerken bağ yerinde");
    let out = drain(&mut t.link, d + s(4));
    assert!(!t.link.is_bound());
    assert_eq!(notices(&mut t.link), vec![HubNotice::Disconnected]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].method, method::CLIENT_BIND);
    assert_eq!(out[0].payload[34..50], [4; 16], "aynı kimlik, yeni nonce");
    assert_eq!(t.link.counters().disconnects, 1);
    // Eski token'ın geç ACK'i hiçbir şey yapmaz.
    t.link.on_control(d + s(4), &hb_ack(tokens[0]));
    assert!(!t.link.is_bound());
    complete_bind_from(&mut t.link, d + s(4), &out[0], 4);
}

#[test]
fn c6_probe_answered_keeps_bind() {
    let mut t = bound();
    let d = t.link.next_deadline().unwrap();
    let _hb = drain(&mut t.link, d).remove(0);
    let p1 = drain(&mut t.link, d + s(1)).remove(0);
    t.link.on_control(d + s(1) + ms(200), &hb_ack(p1.rid));
    assert_eq!(drain(&mut t.link, d + s(5)), vec![]);
    assert!(t.link.is_bound());
    assert_eq!(t.link.counters().heartbeat_acks, 1);
}

#[test]
fn c6_six_hours_of_silence_with_answered_heartbeats_keeps_bind() {
    let mut t = bound();
    let end = t.t0 + s(6 * 3600);
    let mut now = t.t0;
    let mut beats = 0;
    while now < end {
        now = t.link.next_deadline().unwrap();
        for o in drain(&mut t.link, now) {
            assert_eq!(o.method, frame::HEARTBEAT);
            beats += 1;
            t.link.on_control(now + ms(50), &hb_ack(o.rid));
        }
    }
    assert!(t.link.is_bound());
    assert_eq!(t.link.counters().disconnects, 0);
    // 8–10 s kadans: 6 saatte ~2160–2700 heartbeat.
    assert!((2100..2800).contains(&beats), "{beats}");
    assert!(notices(&mut t.link).is_empty());
}

#[test]
fn c6_socket_error_starts_probe() {
    let mut t = bound();
    let now = t.t0 + s(1);
    t.link.on_socket_error(now);
    let out = drain(&mut t.link, now);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].method, frame::HEARTBEAT);
    t.link.on_control(now, &hb_ack(out[0].rid));
    assert!(t.link.is_bound());
}

// ── C3: INIT + T3 ────────────────────────────────────────────────────────────────────────────

fn init_out(t: &mut T, now: Instant) -> Vec<Out> {
    drain(&mut t.link, now)
        .into_iter()
        .filter(|o| o.method == method::STORE_MESSAGE)
        .collect()
}

#[test]
fn c3_store_layout_same_request_id_and_t3_intervals() {
    let mut t = bound();
    let t0 = t.t0;
    let a = t.link.send_init(t0, peer(0xB0), 1, vec![9, 9, 9]).unwrap();
    let first = init_out(&mut t, t0);
    assert_eq!(first.len(), 1);
    let f = &first[0];
    assert_eq!(f.payload[..4], [0; 4], "hub_handle 0 = yeni hat");
    assert_eq!(f.payload[4], envelope::SESSION_INIT);
    assert_eq!(f.payload[5..37], [0xB0; 32]);
    assert_eq!(f.payload[37..41], 3u32.to_be_bytes());
    assert_eq!(f.payload[41..], [9, 9, 9]);
    // StoreResp OK → tek HandleAssigned, T3 sürer.
    t.link
        .on_control(t0 + ms(10), &store_resp(f.rid, code::OK, 0x77));
    t.link
        .on_control(t0 + ms(20), &store_resp(f.rid, code::OK, 0x77));
    assert_eq!(
        events(&mut t.link),
        vec![LinkEvent::HandleAssigned {
            attempt: a,
            hub_handle: HubHandle::from_raw(0x77)
        }]
    );
    assert_eq!(t.link.counters().store_ok_duplicate, 1);
    // T3 tekrarları (heartbeat'ler cevaplanır): aynı request_id, bayt-özdeş.
    let mut resends = 0;
    while resends < 5 {
        let d = t.link.next_deadline().unwrap();
        for o in drain(&mut t.link, d) {
            if o.method == frame::HEARTBEAT {
                t.link.on_control(d, &hb_ack(o.rid));
            } else {
                assert_eq!(o, f.clone(), "aynı request_id, bayt-özdeş");
                resends += 1;
            }
        }
    }
    assert!(events(&mut t.link).is_empty(), "ikinci HandleAssigned yok");
}

#[test]
fn c3_t3_full_sequence_reaches_cap() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let _ = init_out(&mut t, t0);
    let mut times = vec![t0];
    while times.len() < 8 {
        let now = t.link.next_deadline().unwrap();
        for o in drain(&mut t.link, now) {
            if o.method == frame::HEARTBEAT {
                t.link.on_control(now, &hb_ack(o.rid));
            } else {
                times.push(now);
            }
        }
    }
    let gaps: Vec<u64> = times.windows(2).map(|w| (w[1] - w[0]).as_secs()).collect();
    assert_eq!(gaps, vec![1, 2, 4, 8, 16, 30, 30]);
}

#[test]
fn c3_peer_ready_matching_handle_stops_t3_unmatched_does_not() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    // Eski oturumun Ready'si (başka h) T3'ü durdurmaz.
    t.link.peer_ready(peer(0xB0), HubHandle::from_raw(0x55));
    assert_eq!(t.link.counters().ready_unmatched, 1);
    assert_eq!(init_out(&mut t, t0 + s(1)), vec![f.clone()]);
    // Eşleşen Ready → durur.
    t.link.peer_ready(peer(0xB0), HubHandle::from_raw(0x77));
    assert_eq!(t.link.counters().attempt_completed, 1);
    for k in 2..120 {
        assert!(init_out(&mut t, t0 + s(k)).is_empty());
    }
}

#[test]
fn c3_peer_unbound_backs_off_at_least_five_seconds() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link
        .on_control(t0 + ms(100), &store_resp(f.rid, code::PEER_UNBOUND, 0));
    assert!(init_out(&mut t, t0 + s(1)).is_empty());
    assert!(init_out(&mut t, t0 + ms(5099)).is_empty());
    assert_eq!(init_out(&mut t, t0 + ms(5100)), vec![f.clone()]);
    assert!(events(&mut t.link).is_empty(), "başarı sayılmaz");
    // Sonraki T3 vadesi 5.1 + 2 = 7.1 s; 6 s'de RATE_LIMITED → en erken 11 s; 11.1 s'de HUB_CAPACITY → 16.1 s.
    t.link.on_control(
        t0 + s(6),
        &err(method::STORE_MESSAGE, f.rid, code::RATE_LIMITED),
    );
    assert!(init_out(&mut t, t0 + ms(10_999)).is_empty());
    assert_eq!(init_out(&mut t, t0 + s(11)), vec![f.clone()]);
    t.link.on_control(
        t0 + ms(11_100),
        &err(method::STORE_MESSAGE, f.rid, code::HUB_CAPACITY),
    );
    assert!(init_out(&mut t, t0 + ms(16_099)).is_empty());
    assert_eq!(init_out(&mut t, t0 + ms(16_100)), vec![f.clone()]);
    assert_eq!(t.link.counters().store_backoffs, 3);
}

#[test]
fn c3_unknown_handle_ends_attempt_and_notices() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link
        .on_control(t0, &store_resp(f.rid, code::UNKNOWN_HANDLE, 0));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
    for k in 1..60 {
        assert!(init_out(&mut t, t0 + s(k)).is_empty(), "T3 durdu");
    }
    // Geç StoreResp OK sayılır, atanmaz (K9).
    t.link
        .on_control(t0 + s(60), &store_resp(f.rid, code::OK, 0x77));
    assert!(events(&mut t.link).is_empty());
    assert_eq!(t.link.counters().late_store_resp, 1);
}

#[test]
fn c3_attempt_max_age_ends_only_that_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let old = init_out(&mut t, t0).remove(0);
    let t1 = t0 + s(1800);
    t.link.send_init(t1, peer(0xB1), 1, vec![2]).unwrap();
    let young = drain(&mut t.link, t1)
        .into_iter()
        .find(|o| o.payload.get(5..37) == Some(&[0xB1; 32][..]))
        .unwrap();
    t.link.on_tick(t0 + s(3600));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
    assert_eq!(t.link.counters().attempt_expired, 1);
    // Genç deneme sürer.
    t.link
        .on_control(t0 + s(3600), &store_resp(young.rid, code::OK, 0x99));
    assert_eq!(events(&mut t.link).len(), 1);
    // Yaşlının geç yanıtı atanmaz.
    t.link
        .on_control(t0 + s(3600), &store_resp(old.rid, code::OK, 0x77));
    assert!(events(&mut t.link).is_empty());
}

#[test]
fn c3_conflicting_handle_ends_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x78));
    assert!(events(&mut t.link).is_empty(), "farklı h zorla yazılmaz");
    assert_eq!(t.link.counters().attempt_handle_conflict, 1);
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
}

#[test]
fn c3_new_init_to_same_peer_supersedes_old_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let old = init_out(&mut t, t0).remove(0);
    let a2 = t.link.send_init(t0 + s(1), peer(0xB0), 1, vec![2]).unwrap();
    let new = init_out(&mut t, t0 + s(1)).remove(0);
    assert_ne!(old.rid, new.rid, "yeni request_id");
    t.link
        .on_control(t0 + s(1), &store_resp(old.rid, code::OK, 0x77));
    assert!(events(&mut t.link).is_empty());
    t.link
        .on_control(t0 + s(1), &store_resp(new.rid, code::OK, 0x88));
    assert_eq!(
        events(&mut t.link),
        vec![LinkEvent::HandleAssigned {
            attempt: a2,
            hub_handle: HubHandle::from_raw(0x88)
        }]
    );
}

#[test]
fn c3_bad_inputs_are_rejected() {
    let mut t = bound();
    let t0 = t.t0;
    assert_eq!(
        t.link.send_init(t0, peer(1), 2, vec![1]),
        Err(HubError::BadEnvelopeType)
    );
    assert_eq!(
        t.link.send_init(
            t0,
            peer(1),
            1,
            vec![0; s4::StoreMessageReq::MAX_ENVELOPE + 1]
        ),
        Err(HubError::FrameTooLong)
    );
    t.link
        .send_init(t0, peer(1), 1, vec![0; s4::StoreMessageReq::MAX_ENVELOPE])
        .unwrap();
    let f = drain(&mut t.link, t0).remove(0);
    assert_eq!(f.raw.len() + 16, 1280, "çerçeve + 16 B ≤ MAX_DATAGRAM");
    assert_eq!(
        t.link
            .send_response(t0, peer(1), HubHandle::from_raw(5), 1, vec![1]),
        Err(HubError::BadEnvelopeType)
    );
}

// ── C4: RESPONSE, yanıtlayan, K3 ─────────────────────────────────────────────────────────────

#[test]
fn c4_responder_duplicate_init_resends_saved_response() {
    let mut t = bound();
    let t0 = t.t0;
    let init = vec![0x11; 40];
    t.link.on_control(t0, &deliver(0x77, [0xA9; 32], 1, &init));
    assert_eq!(
        events(&mut t.link),
        vec![LinkEvent::IncomingInit {
            hub_handle: HubHandle::from_raw(0x77),
            envelope_type: 1,
            bytes: init.clone()
        }]
    );
    // Transport cevap üretmeden ikinci DELIVER: verilmez.
    t.link.on_control(t0, &deliver(0x77, [0xA9; 32], 1, &init));
    assert!(events(&mut t.link).is_empty());
    assert_eq!(t.link.counters().init_duplicate_pending, 1);
    t.link
        .send_response(t0, peer(0xA9), HubHandle::from_raw(0x77), 2, vec![0x22; 30])
        .unwrap();
    let resp = init_out(&mut t, t0).remove(0);
    assert_eq!(resp.payload[..4], 0x77u32.to_be_bytes());
    assert_eq!(resp.payload[4], envelope::SESSION_INIT_RESPONSE);
    assert_eq!(resp.payload[5..37], [0xA9; 32]);
    // Aynı (h, INIT) yine gelir (RESPONSE DELIVER'ı kaybolmuş): saklı RESPONSE bayt-özdeş gider.
    t.link
        .on_control(t0 + s(1), &deliver(0x77, [0xA9; 32], 1, &init));
    assert!(events(&mut t.link).is_empty(), "yeni oturum açılmaz");
    assert_eq!(init_out(&mut t, t0 + s(1)), vec![resp.clone()]);
    assert_eq!(t.link.counters().init_duplicate_resend, 1);
    // Aynı INIT baytları farklı h ile: yok say.
    t.link
        .on_control(t0 + s(2), &deliver(0x78, [0xA9; 32], 1, &init));
    assert!(events(&mut t.link).is_empty());
    assert!(init_out(&mut t, t0 + s(2)).is_empty());
    assert_eq!(t.link.counters().init_handle_conflict, 1);
}

#[test]
fn c4_initiator_response_once_per_bytes() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    t.link
        .on_control(t0, &deliver(0x77, [0xB0; 32], 2, &[5; 20]));
    t.link
        .on_control(t0, &deliver(0x77, [0xB0; 32], 2, &[5; 20]));
    assert_eq!(
        events(&mut t.link),
        vec![LinkEvent::Response {
            hub_handle: HubHandle::from_raw(0x77),
            envelope_type: 2,
            bytes: vec![5; 20]
        }]
    );
    assert_eq!(t.link.counters().response_duplicate, 1);
}

#[test]
fn k3_response_before_store_resp_is_held_then_released_in_order() {
    let mut t = bound();
    let t0 = t.t0;
    let a = t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    // StoreResp kayboldu; RESPONSE önce gelir.
    t.link
        .on_control(t0 + ms(50), &deliver(0x77, [0xB0; 32], 2, &[5; 20]));
    assert!(events(&mut t.link).is_empty(), "tutulur");
    assert_eq!(t.link.counters().response_held, 1);
    // T3 tekrarı → hub aynı anahtarla aynı h'yi yeniden verir.
    assert_eq!(init_out(&mut t, t0 + s(1)), vec![f.clone()]);
    t.link
        .on_control(t0 + s(1), &store_resp(f.rid, code::OK, 0x77));
    assert_eq!(
        events(&mut t.link),
        vec![
            LinkEvent::HandleAssigned {
                attempt: a,
                hub_handle: HubHandle::from_raw(0x77)
            },
            LinkEvent::Response {
                hub_handle: HubHandle::from_raw(0x77),
                envelope_type: 2,
                bytes: vec![5; 20]
            }
        ]
    );
}

#[test]
fn k3_held_responses_are_bounded_and_expire() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let _ = init_out(&mut t, t0);
    for h in 1..=40u32 {
        t.link
            .on_control(t0, &deliver(h, [0xB0; 32], 2, &[h as u8; 4]));
    }
    assert_eq!(t.link.held.len(), MAX_HELD_RESPONSES);
    assert_eq!(
        t.link.counters().bounded_drops,
        40 - MAX_HELD_RESPONSES as u64
    );
    // Yanıt bekleyen deneme yoksa RESPONSE tutulmaz (geç yanıt).
    let mut u = bound();
    u.link.on_control(u.t0, &deliver(5, [0xB0; 32], 2, &[1]));
    assert_eq!(u.link.counters().late_response, 1);
    assert!(u.link.held.is_empty());
}

// ── C5: ROUTE_MISS ───────────────────────────────────────────────────────────────────────────

#[test]
fn c5_route_miss_releases_handle_with_per_peer_backoff() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    t.link.peer_ready(peer(0xB0), HubHandle::from_raw(0x77));
    let _ = (events(&mut t.link), notices(&mut t.link));
    t.link.on_control(t0 + s(1), &route_miss(0x77));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::HandleLost {
            peer: peer(0xB0),
            h: HubHandle::from_raw(0x77)
        }]
    );
    // Bırakılan h için ikinci ROUTE_MISS bilinmez.
    t.link.on_control(t0 + s(1), &route_miss(0x77));
    assert_eq!(t.link.counters().route_miss_unknown, 1);
    // Aynı eşin yeni denemesi hemen biterse bildirim ≥ 5 s ertelenir.
    t.link.send_init(t0 + s(2), peer(0xB0), 1, vec![2]).unwrap();
    let f2 = init_out(&mut t, t0 + s(2)).remove(0);
    t.link
        .on_control(t0 + s(2), &store_resp(f2.rid, code::UNKNOWN_HANDLE, 0));
    assert!(notices(&mut t.link).is_empty());
    let d = t.link.next_deadline().unwrap();
    assert!(d <= t0 + s(6));
    t.link.on_tick(t0 + s(6));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
}

#[test]
fn c5_route_miss_on_responder_handle() {
    let mut t = bound();
    let t0 = t.t0;
    t.link
        .on_control(t0, &deliver(0x77, [0xA9; 32], 1, &[1; 8]));
    t.link
        .send_response(t0, peer(0xA9), HubHandle::from_raw(0x77), 2, vec![2; 8])
        .unwrap();
    let _ = events(&mut t.link);
    t.link.on_control(t0, &route_miss(0x77));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::HandleLost {
            peer: peer(0xA9),
            h: HubHandle::from_raw(0x77)
        }]
    );
}

/// [HLR] Atanmış handle'ı ROUTE_MISS alan açılış denemesi biter → `AttemptEnded` (C3 gibi; niyet
/// ya da yenileme yeni INIT ile yeniden başlar). ESKİ DAVRANIŞ: `HandleLost` veriliyordu; bileşim
/// kökü kurulu oturum bulamaz, T3'ü durmuş deneme sessizce ölürdü.
#[test]
fn hlr_route_miss_on_an_attempts_handle_ends_the_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    t.link.on_control(t0, &route_miss(0x77));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
    assert_eq!(t.link.counters().route_miss, 1);
    for k in 1..60 {
        assert!(init_out(&mut t, t0 + s(k)).is_empty(), "T3 durdu");
    }
}

/// `p` için `0x55` handle'lı kurulu hat (yanıtlayan rolü) + `t0`'da verilmiş bir `AttemptEnded`
/// (eş başına aralığı `t0`'da başlatır).
fn link_and_first_notice(t: &mut T, p: u8) {
    let t0 = t.t0;
    t.link.on_control(t0, &deliver(0x55, [p; 32], 1, &[5; 8]));
    t.link
        .send_response(t0, peer(p), HubHandle::from_raw(0x55), 2, vec![6; 8])
        .unwrap();
    let _ = (events(&mut t.link), drain(&mut t.link, t0));
    t.link.send_init(t0, peer(p), 1, vec![1]).unwrap();
    let f = init_out(t, t0).remove(0);
    t.link
        .on_control(t0, &store_resp(f.rid, code::UNKNOWN_HANDLE, 0));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(p) }]
    );
}

/// [HLR] KK-H39 aralığında ertelenen farklı türler birbirinin yerini ALMAZ: ertelenmiş
/// `AttemptEnded`'den sonra gelen `HandleLost` ikisini de verir (önce `AttemptEnded`). ESKİ
/// DAVRANIŞ: tek erteleme yuvası vardı; `HandleLost` `AttemptEnded`'in yerini alıyordu (yenilemenin
/// bitmiş denemesi yeniden başlamazdı).
#[test]
fn hlr_deferred_notices_of_different_kinds_are_both_released() {
    let mut t = bound();
    let t0 = t.t0;
    link_and_first_notice(&mut t, 0xB0);
    t.link.send_init(t0 + s(1), peer(0xB0), 1, vec![2]).unwrap();
    let f = init_out(&mut t, t0 + s(1)).remove(0);
    t.link
        .on_control(t0 + s(1), &store_resp(f.rid, code::UNKNOWN_HANDLE, 0));
    t.link.on_control(t0 + s(2), &route_miss(0x55));
    assert!(notices(&mut t.link).is_empty(), "aralık içinde ertelenir");
    assert_eq!(t.link.next_deadline().map(|d| d <= t0 + s(5)), Some(true));
    t.link.on_tick(t0 + s(5));
    assert_eq!(
        notices(&mut t.link),
        vec![
            HubNotice::AttemptEnded { peer: peer(0xB0) },
            HubNotice::HandleLost {
                peer: peer(0xB0),
                h: HubHandle::from_raw(0x55)
            }
        ]
    );
}

/// [HLR] Aralık dolduktan sonra (zamanlayıcı henüz dönmeden) gelen bildirim eşin ertelenmişini
/// SİLMEZ: ikisi birlikte verilir. ESKİ DAVRANIŞ: ertelenmiş `AttemptEnded` sessizce düşüyordu.
#[test]
fn hlr_a_due_notice_flushes_the_deferred_one_instead_of_dropping_it() {
    let mut t = bound();
    let t0 = t.t0;
    link_and_first_notice(&mut t, 0xB0);
    t.link.send_init(t0 + s(1), peer(0xB0), 1, vec![2]).unwrap();
    let f = init_out(&mut t, t0 + s(1)).remove(0);
    t.link
        .on_control(t0 + s(1), &store_resp(f.rid, code::UNKNOWN_HANDLE, 0));
    assert!(notices(&mut t.link).is_empty());
    // Vadesi (t0 + 5 s) geçti ama `on_tick` çağrılmadı; ROUTE_MISS t0 + 6 s'de gelir.
    t.link.on_control(t0 + s(6), &route_miss(0x55));
    assert_eq!(
        notices(&mut t.link),
        vec![
            HubNotice::AttemptEnded { peer: peer(0xB0) },
            HubNotice::HandleLost {
                peer: peer(0xB0),
                h: HubHandle::from_raw(0x55)
            }
        ]
    );
}

/// [HLR] `known_links` dolunca önce yerini yenisi almış hatlar düşer: başka bir eşin hat
/// çalkantısı (64+ yanıt / yeniden bağlantı) uzun süredir sessiz bir eşin TEK hattını unutturmaz;
/// o hattın ROUTE_MISS'i `HandleLost` verir. ESKİ DAVRANIŞ: en eski girdi düşüyordu →
/// `route_miss_unknown`, bildirim yok, oturum yeni INIT'siz kalırdı.
#[test]
fn hlr_known_links_keep_a_quiet_peers_only_link_under_churn() {
    const CHURN: u32 = 8;
    let mut t = bound();
    let t0 = t.t0;
    t.link.on_control(t0, &deliver(1, [0xA1; 32], 1, &[1; 8]));
    t.link
        .send_response(t0, peer(0xA1), HubHandle::from_raw(1), 2, vec![1; 8])
        .unwrap();
    let last = u32::try_from(MAX_KNOWN_LINKS).unwrap() + CHURN;
    for i in 2..=last {
        let init = i.to_be_bytes();
        t.link.on_control(t0, &deliver(i, [0xA9; 32], 1, &init));
        t.link
            .send_response(t0, peer(0xA9), HubHandle::from_raw(i), 2, init.to_vec())
            .unwrap();
        let _ = drain(&mut t.link, t0);
    }
    let _ = events(&mut t.link);
    assert_eq!(t.link.known_links.len(), MAX_KNOWN_LINKS);
    t.link.on_control(t0, &route_miss(1));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::HandleLost {
            peer: peer(0xA1),
            h: HubHandle::from_raw(1)
        }]
    );
    assert_eq!(t.link.counters().route_miss_unknown, 0);
    // Son hat da bilinir (yeni girdi düşmez).
    t.link.on_control(t0, &route_miss(last));
    assert_eq!(t.link.counters().route_miss, 2);
}

/// [HLR] Terk edilen deneme (`cancel_attempt`; transport bekleyeni kapattı): T3 durur, bildirim
/// yok, atanmış handle emekli halkasında (geç RESPONSE'u geç sayılır, K9). ESKİ DAVRANIŞ: iptal
/// yolu yoktu; transport'un kapattığı yenilemenin T3'ü `ATTEMPT_MAX_AGE`'e (1 saat) dek sürüyordu.
#[test]
fn hlr_cancelled_attempt_stops_t3_without_notice() {
    let mut t = bound();
    let t0 = t.t0;
    let id = t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    t.link.cancel_attempt(id);
    assert_eq!(t.link.counters().attempt_cancelled, 1);
    assert!(notices(&mut t.link).is_empty(), "terk eden zaten biliyor");
    for k in 1..120 {
        assert!(init_out(&mut t, t0 + s(k)).is_empty(), "T3 durdu");
    }
    t.link.cancel_attempt(id);
    assert_eq!(
        t.link.counters().attempt_cancelled,
        1,
        "bitmiş deneme: etkisiz"
    );
    t.link
        .on_control(t0 + s(2), &deliver(0x77, [0xB0; 32], 2, &[9; 8]));
    assert!(events(&mut t.link).is_empty(), "geç RESPONSE");
    assert_eq!(t.link.counters().late_response, 1);
}

/// [HLR] Denemesi bittikten sonra kurulan oturumun (`Ready`'si hiçbir denemeyle eşleşmez) handle'ı
/// hatırlanır: o hattın ROUTE_MISS'i `HandleLost` verir. Sıra: yenilemenin RESPONSE'u boşalma için
/// tutulurken denemesi biter (`{UNKNOWN_HANDLE, 0}` ya da yeniden bind), yenileme yine de o handle'da
/// tamamlanır. ESKİ DAVRANIŞ: `route_miss_unknown`, bildirim yok — oturum yeni INIT'siz kalırdı.
#[test]
fn hlr_unmatched_ready_handle_is_remembered_for_route_miss() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    t.link
        .on_control(t0, &store_resp(f.rid, code::UNKNOWN_HANDLE, 0));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
    t.link.peer_ready(peer(0xB0), HubHandle::from_raw(0x77));
    assert_eq!(t.link.counters().ready_unmatched, 1);
    t.link.on_control(t0 + s(10), &route_miss(0x77));
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::HandleLost {
            peer: peer(0xB0),
            h: HubHandle::from_raw(0x77)
        }]
    );
    assert_eq!(t.link.counters().route_miss_unknown, 0);
}

// ── C7: yeniden bind ─────────────────────────────────────────────────────────────────────────

#[test]
fn c7_rebind_ends_sent_incomplete_attempts_and_keeps_queue() {
    let mut t = bound();
    let d = t.link.next_deadline().unwrap();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let _ = drain(&mut t.link, t0);
    let mut now = d;
    let mut last = Vec::new();
    // Heartbeat'leri cevapsız bırak → koptu.
    while t.link.is_bound() {
        now = t.link.next_deadline().unwrap();
        last = drain(&mut t.link, now);
    }
    assert_eq!(notices(&mut t.link), vec![HubNotice::Disconnected]);
    // Kopukken gelen yeni INIT ve RESPONSE kuyrukta bekler.
    t.link.send_init(now, peer(0xB1), 1, vec![2]).unwrap();
    t.link
        .send_response(now, peer(0xA9), HubHandle::from_raw(0x66), 2, vec![3])
        .unwrap();
    assert!(
        drain(&mut t.link, now).is_empty(),
        "bind olmadan STORE gitmez"
    );
    let b1 = last
        .into_iter()
        .find(|o| o.method == method::CLIENT_BIND)
        .unwrap();
    complete_bind_from(&mut t.link, now, &b1, 4);
    assert_eq!(
        notices(&mut t.link),
        vec![
            HubNotice::AttemptEnded { peer: peer(0xB0) },
            HubNotice::Bound
        ]
    );
    let out = init_out(&mut t, now);
    assert_eq!(out.len(), 2, "kuyruktaki RESPONSE + hiç gönderilmemiş INIT");
    assert_eq!(t.link.counters().attempt_rebind_ended, 1);
}

// ── C8 / C9 / sınırlar ───────────────────────────────────────────────────────────────────────

#[test]
fn c8_unknown_frames_counted_c9_rx_age() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.on_control(t0 + s(1), &[0, 0, 0, 0, 0x42, 1, 2]);
    t.link.on_control(t0 + s(1), &[0, 0, 0, 1, 0x1D]);
    t.link.on_control(t0 + s(1), &rpc(0x99, 0, 0, &[]));
    assert_eq!(t.link.counters().hub_frames_dropped, 2);
    assert_eq!(t.link.counters().rx_malformed, 1);
    assert_eq!(t.link.last_rx_age(t0 + s(4)), Some(s(3)));
    t.link.on_control(
        t0 + s(1),
        &deliver(5, [1; 32], envelope::RELAY_OPEN, &[0; 16]),
    );
    assert_eq!(t.link.counters().deliver_relay_open, 1);
    assert!(events(&mut t.link).is_empty());
}

#[test]
fn tables_are_bounded() {
    let mut t = bound();
    let t0 = t.t0;
    for i in 1..=200u32 {
        let init = i.to_be_bytes();
        t.link.on_control(t0, &deliver(i, [0xA9; 32], 1, &init));
        t.link
            .send_response(t0, peer(0xA9), HubHandle::from_raw(i), 2, init.to_vec())
            .unwrap();
        let _ = drain(&mut t.link, t0);
        let _ = events(&mut t.link);
    }
    assert_eq!(t.link.responder.len(), MAX_RESPONDER_RECORDS);
    assert_eq!(t.link.known_links.len(), MAX_KNOWN_LINKS);
    assert_eq!(t.link.response_store_ids.len(), MAX_RESPONSE_STORE_IDS);
    // Gönderim kuyruğu boşaltılmasa da sınırlı.
    for i in 0..200u8 {
        t.link.send_init(t0, peer(i), 1, vec![i]).ok();
    }
    assert!(t.link.attempts.len() <= MAX_ATTEMPTS);
    assert!(t.link.tx.len() <= MAX_TX_QUEUE);
}

#[test]
fn debug_does_not_print_identity() {
    let s = format!("{:?}", cfg());
    assert!(!s.contains("193") && !s.contains("c1") && !s.contains("208"));
}

// ── cephe ────────────────────────────────────────────────────────────────────────────────────

struct FakeClock(Mutex<Instant>);
impl HubClock for FakeClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

#[test]
fn envelope_link_facade_drives_core() {
    let t = bound();
    let t0 = t.t0;
    let shared = Arc::new(Mutex::new(t.link));
    let clock = Arc::new(FakeClock(Mutex::new(t0)));
    let mut el = HubEnvelopeLink::with_clock(shared.clone(), clock.clone());
    let a = el.send_init(peer(0xB0), 1, vec![1]).unwrap();
    let f = parse(&shared.lock().unwrap().poll_transmit(t0).unwrap());
    shared
        .lock()
        .unwrap()
        .on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    assert_eq!(
        el.poll(),
        Some(LinkEvent::HandleAssigned {
            attempt: a,
            hub_handle: HubHandle::from_raw(0x77)
        })
    );
    assert!(el.send_init(peer(0xB0), 3, vec![1]).is_err());
    assert_eq!(el.poll(), None);
}

// ── fix1: çelişkili handle (S4 C3) ve K9 ─────────────────────────────────────────────────────

#[test]
fn fix1_c3_store_resp_then_response_with_other_handle_ends_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    // Aynı denemenin eşinden, StoreResp'in söylediğinden farklı h ile RESPONSE.
    t.link
        .on_control(t0, &deliver(0x78, [0xB0; 32], 2, &[5; 20]));
    assert!(events(&mut t.link).is_empty(), "farklı h zorla yazılmaz");
    assert_eq!(t.link.counters().attempt_handle_conflict, 1);
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
    for k in 1..40 {
        assert!(init_out(&mut t, t0 + s(k)).is_empty(), "T3 durdu");
    }
}

#[test]
fn fix1_c3_held_response_then_store_resp_with_other_handle_ends_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    // StoreResp kayboldu; RESPONSE h = 0x78 ile önce gelir (tutulur).
    t.link
        .on_control(t0, &deliver(0x78, [0xB0; 32], 2, &[5; 20]));
    assert_eq!(t.link.counters().response_held, 1);
    // T3'ün getirdiği StoreResp başka h söylüyor.
    t.link
        .on_control(t0 + s(1), &store_resp(f.rid, code::OK, 0x77));
    assert!(events(&mut t.link).is_empty(), "HandleAssigned verilmez");
    assert_eq!(t.link.counters().attempt_handle_conflict, 1);
    assert_eq!(
        notices(&mut t.link),
        vec![HubNotice::AttemptEnded { peer: peer(0xB0) }]
    );
    assert!(t.link.held.is_empty(), "çelişen tutulan yanıt da düşer");
}

#[test]
fn fix1_k9_late_response_of_ended_attempt_does_not_end_new_attempt() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let old = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(old.rid, code::OK, 0x66));
    // Deneme 2 (aynı eşe yeni INIT) eskisini sonlandırır.
    let a2 = t.link.send_init(t0 + s(1), peer(0xB0), 1, vec![2]).unwrap();
    let new = init_out(&mut t, t0 + s(1)).remove(0);
    t.link
        .on_control(t0 + s(1), &store_resp(new.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    // Eski denemenin gecikmiş RESPONSE'u (h = 0x66): sayılır, deneme 2'yi bitirmez.
    t.link
        .on_control(t0 + s(2), &deliver(0x66, [0xB0; 32], 2, &[6; 20]));
    assert!(events(&mut t.link).is_empty());
    assert_eq!(t.link.counters().late_response, 1);
    assert_eq!(t.link.counters().attempt_handle_conflict, 0);
    assert!(notices(&mut t.link).is_empty());
    // Deneme 2 kendi h'siyle kurulur.
    t.link
        .on_control(t0 + s(2), &deliver(0x77, [0xB0; 32], 2, &[7; 20]));
    assert_eq!(
        events(&mut t.link),
        vec![LinkEvent::Response {
            hub_handle: HubHandle::from_raw(0x77),
            envelope_type: 2,
            bytes: vec![7; 20]
        }]
    );
    t.link.peer_ready(peer(0xB0), HubHandle::from_raw(0x77));
    assert_eq!(t.link.counters().attempt_completed, 1);
    let _ = a2;
}

#[test]
fn fix1_k9_late_response_while_store_resp_pending_is_not_held() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let old = init_out(&mut t, t0).remove(0);
    t.link
        .on_control(t0, &store_resp(old.rid, code::UNKNOWN_HANDLE, 0));
    let _ = notices(&mut t.link);
    // Önceki bir oturumun hattı (0x55) bilinen hat.
    t.link.send_init(t0 + s(1), peer(0xB0), 1, vec![2]).unwrap();
    let f = init_out(&mut t, t0 + s(1)).remove(0);
    t.link
        .on_control(t0 + s(1), &store_resp(f.rid, code::OK, 0x55));
    t.link.peer_ready(peer(0xB0), HubHandle::from_raw(0x55));
    let _ = events(&mut t.link);
    // Deneme 3; StoreResp kayıp. Bilinen eski hattın (0x55) yinelenen RESPONSE'u gelir.
    t.link
        .send_init(t0 + s(10), peer(0xB0), 1, vec![3])
        .unwrap();
    let f3 = init_out(&mut t, t0 + s(10)).remove(0);
    t.link
        .on_control(t0 + s(10), &deliver(0x55, [0xB0; 32], 2, &[5; 20]));
    assert!(t.link.held.is_empty(), "eski hattın yanıtı tutulmaz");
    assert_eq!(t.link.counters().late_response, 1);
    t.link
        .on_control(t0 + s(11), &store_resp(f3.rid, code::OK, 0x99));
    assert_eq!(t.link.counters().attempt_handle_conflict, 0);
    assert_eq!(events(&mut t.link).len(), 1, "HandleAssigned");
}

#[test]
fn fix1_response_from_other_peer_is_not_a_conflict() {
    let mut t = bound();
    let t0 = t.t0;
    t.link.send_init(t0, peer(0xB0), 1, vec![1]).unwrap();
    let f = init_out(&mut t, t0).remove(0);
    t.link.on_control(t0, &store_resp(f.rid, code::OK, 0x77));
    let _ = events(&mut t.link);
    t.link
        .on_control(t0, &deliver(0x78, [0xC0; 32], 2, &[5; 20]));
    assert_eq!(t.link.counters().attempt_handle_conflict, 0);
    assert_eq!(t.link.counters().late_response, 1);
    assert!(notices(&mut t.link).is_empty());
}
