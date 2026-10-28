//! Test aracı: sahte saat, belirlenimli rastgelelik, bellek içi "soket", sertifikalı test telefonları
//! ve `hub.json` v1 üretici. Hub IO'suzdur; [`World::deliver`] bir datagramı hub'a verir ve o
//! datagramın ürettiği çıkışı döndürür. Zaman yalnız test ilerletince akar.

use std::cell::Cell;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::rc::Rc;
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use hub_server::config::{self, Config, Limits};
use hub_server::wire::{
    bind_ack_tbs, bind_proof_tbs, classify, code, control_datagram, envelope, flags, frame, method,
    BindProofRequest, BindProofResponse, CertRef, ClientBindRequest, ClientBindResponse, Datagram,
    Deliver, DestHash, ErrorPayload, Nonce, Payload, RouteMiss, RpcFrame, StoreRequest,
    StoreResponse, BIND_OK,
};
use hub_server::{Clock, Hub, Sink};
use kama_noise_wire::ed25519_strict::verify_strict;
use kama_noise_wire::entropy::{EntropyError, EntropySource};
use kama_noise_wire::handshake::cert_ref_of;

pub const HUB_SEED: [u8; 32] = [0x48; 32];

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut padded = [0u8; 3];
        padded[..chunk.len()].copy_from_slice(chunk);
        let n = u32::from_be_bytes([0, padded[0], padded[1], padded[2]]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn hub_public() -> [u8; 32] {
    SigningKey::from_bytes(&HUB_SEED).verifying_key().to_bytes()
}

// ── Saat ve rastgelelik ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct FakeClock(Rc<Cell<Duration>>);

impl FakeClock {
    pub fn advance(&self, by: Duration) {
        self.0.set(self.0.get() + by);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}

/// SplitMix64 — std-only.
#[derive(Clone)]
pub struct TestRng(pub u64);

impl TestRng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    pub fn fill_bytes(&mut self, dst: &mut [u8]) {
        for chunk in dst.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }

    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        self.fill_bytes(&mut out);
        out
    }

    pub fn addr(&mut self) -> SocketAddr {
        let ip = Ipv4Addr::from((self.next_u64() >> 32) as u32);
        SocketAddr::new(IpAddr::V4(ip), ((self.next_u64() >> 48) as u16).max(1))
    }
}

impl EntropySource for TestRng {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        self.fill_bytes(dst);
        Ok(())
    }
}

// ── Bellek içi soket ────────────────────────────────────────────────────────────────────────────

pub type Sent = (SocketAddr, Vec<u8>);

#[derive(Default)]
pub struct Net {
    pub sent: Vec<Sent>,
    pub logs: Vec<String>,
}

impl Sink for Net {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) {
        self.sent.push((to, datagram.to_vec()));
    }

    fn log(&mut self, line: fmt::Arguments<'_>) {
        self.logs.push(line.to_string());
    }
}

// ── Çerçeveler ──────────────────────────────────────────────────────────────────────────────────

pub fn rpc(method_id: u8, flags: u8, request_id: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    RpcFrame {
        method_id,
        flags,
        request_id,
        payload,
    }
    .encode(&mut out)
    .expect("kodlanmalı");
    out
}

pub fn heartbeat(token: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    control_datagram(&mut out, frame::HEARTBEAT, token);
    out
}

pub fn data(handle: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = handle.to_be_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

/// Hub'dan gelen bir RPC çerçevesi (sahipli).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub method_id: u8,
    pub flags: u8,
    pub request_id: u32,
    pub payload: Vec<u8>,
}

pub fn reply(datagram: &[u8]) -> Reply {
    let Ok(Datagram::Control {
        frame_type: frame::PLANE1_RPC,
        body,
    }) = classify(datagram)
    else {
        panic!("PLANE1_RPC bekleniyordu: {datagram:02x?}");
    };
    let f = RpcFrame::decode(body).expect("AppRpcFrame");
    Reply {
        method_id: f.method_id,
        flags: f.flags,
        request_id: f.request_id,
        payload: f.payload.to_vec(),
    }
}

/// Hata çerçevesi mi; kodunu döner.
pub fn error_code(datagram: &[u8]) -> u16 {
    let r = reply(datagram);
    assert_eq!(
        r.flags,
        flags::RESPONSE_ERROR,
        "hata çerçevesi bekleniyordu: {r:?}"
    );
    ErrorPayload::decode(&r.payload).expect("ErrorPayload").code
}

pub fn store_response(datagram: &[u8]) -> StoreResponse {
    let r = reply(datagram);
    assert_eq!(
        (r.method_id, r.flags),
        (method::STORE_MESSAGE, flags::RESPONSE),
        "{r:?}"
    );
    StoreResponse::decode(&r.payload).expect("StoreResp")
}

/// DELIVER: (handle, gönderen, tip, zarf).
pub fn deliver(datagram: &[u8]) -> (u32, DestHash, u8, Vec<u8>) {
    let r = reply(datagram);
    assert_eq!(
        (r.method_id, r.flags, r.request_id),
        (method::DELIVER, 0, 0),
        "{r:?}"
    );
    let d = Deliver::decode(&r.payload).expect("DELIVER");
    (d.hub_handle, d.sender, d.envelope_type, d.envelope.to_vec())
}

pub fn route_miss(datagram: &[u8]) -> RouteMiss {
    let r = reply(datagram);
    assert_eq!(
        (r.method_id, r.flags, r.request_id),
        (method::ROUTE_MISS, 0, 0),
        "{r:?}"
    );
    RouteMiss::decode(&r.payload).expect("ROUTE_MISS")
}

pub fn only(sent: &[Sent]) -> &Sent {
    assert_eq!(
        sent.len(),
        1,
        "tek datagram bekleniyordu: {} adet",
        sent.len()
    );
    &sent[0]
}

pub fn sent_to(sent: &[Sent], to: SocketAddr) -> &[u8] {
    let mut matching = sent.iter().filter(|(addr, _)| *addr == to);
    let (_, bytes) = matching
        .next()
        .unwrap_or_else(|| panic!("{to} adresine datagram yok"));
    assert!(
        matching.next().is_none(),
        "{to} adresine birden çok datagram"
    );
    bytes
}

// ── Telefon ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Phone {
    pub name: &'static str,
    pub key: SigningKey,
    pub cert: [u8; 152],
    pub cert_ref: CertRef,
    pub id: DestHash,
    pub addr: SocketAddr,
    next_rid: u32,
}

impl Phone {
    pub fn new(name: &'static str, tag: u8, ip: [u8; 4], port: u16) -> Self {
        let key = SigningKey::from_bytes(&[tag; 32]);
        // CompactDeviceCertV1 düzeni: [0..32) cihaz Ed25519 · [32..64) X25519 · [64..80) issuer ·
        // [80..88) not_after · [88..152) imza. Hub yalnız ilk 32 baytı ve SHA-256'yı kullanır.
        let mut cert = [tag ^ 0x5a; 152];
        cert[..32].copy_from_slice(&key.verifying_key().to_bytes());
        let mut id = [0u8; 32];
        for (slot, offset) in id.iter_mut().zip(0u8..) {
            *slot = tag.wrapping_add(offset.wrapping_mul(0x11));
        }
        Self {
            name,
            key,
            cert,
            cert_ref: cert_ref_of(&cert),
            id,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port),
            next_rid: u32::from(tag) << 24,
        }
    }

    pub fn alice() -> Self {
        Self::new("alice", 0xa1, [198, 51, 100, 7], 40_001)
    }
    pub fn bob() -> Self {
        Self::new("bob", 0xc3, [192, 0, 2, 33], 50_022)
    }
    pub fn carol() -> Self {
        Self::new("carol", 0xe5, [203, 0, 113, 50], 41_000)
    }
    pub fn dave() -> Self {
        Self::new("dave", 0x17, [203, 0, 113, 51], 41_001)
    }
    pub fn erin() -> Self {
        Self::new("erin", 0x29, [203, 0, 113, 52], 41_002)
    }
    pub fn frank() -> Self {
        Self::new("frank", 0x3b, [203, 0, 113, 53], 41_003)
    }
    /// Dizinde OLMAYAN kimlik.
    pub fn mallory() -> Self {
        Self::new("mallory", 0x66, [203, 0, 113, 99], 6_666)
    }

    /// Dizindeki bütün test telefonları.
    pub fn directory() -> Vec<Phone> {
        vec![
            Self::alice(),
            Self::bob(),
            Self::carol(),
            Self::dave(),
            Self::erin(),
            Self::frank(),
        ]
    }

    pub fn moved_to(&self, ip: [u8; 4], port: u16) -> Self {
        let mut moved = self.clone();
        moved.addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port);
        moved
    }

    /// Yeni `request_id` (+1).
    pub fn rid(&mut self) -> u32 {
        self.next_rid = self.next_rid.wrapping_add(1);
        self.next_rid
    }

    pub fn bind1(&self, request_id: u32, client_nonce: Nonce) -> Vec<u8> {
        let body = ClientBindRequest {
            cert_ref: self.cert_ref,
            client_nonce,
        }
        .to_vec();
        rpc(method::CLIENT_BIND, flags::REQUEST, request_id, &body)
    }

    pub fn bind2(
        &self,
        request_id: u32,
        hub_pub: &[u8; 32],
        client_nonce: Nonce,
        hub_nonce: Nonce,
    ) -> Vec<u8> {
        self.bind2_signed_by(&self.key, request_id, hub_pub, client_nonce, hub_nonce)
    }

    pub fn bind2_signed_by(
        &self,
        key: &SigningKey,
        request_id: u32,
        hub_pub: &[u8; 32],
        client_nonce: Nonce,
        hub_nonce: Nonce,
    ) -> Vec<u8> {
        let tbs = bind_proof_tbs(hub_pub, &self.cert_ref, &client_nonce, &hub_nonce);
        let body = BindProofRequest {
            cert_ref: self.cert_ref,
            client_nonce,
            hub_nonce,
            signature: key.sign(&tbs).to_bytes(),
        }
        .to_vec();
        rpc(method::BIND_PROOF, flags::REQUEST, request_id, &body)
    }

    pub fn store(
        &self,
        request_id: u32,
        hub_handle: u32,
        envelope_type: u8,
        to: &Phone,
        envelope: &[u8],
    ) -> Vec<u8> {
        let body = StoreRequest {
            hub_handle,
            envelope_type,
            recipient: to.id,
            envelope,
        }
        .to_vec();
        rpc(method::STORE_MESSAGE, flags::REQUEST, request_id, &body)
    }

    pub fn init(&mut self, to: &Phone, envelope: &[u8]) -> (u32, Vec<u8>) {
        let rid = self.rid();
        (
            rid,
            self.store(rid, 0, envelope::SESSION_INIT, to, envelope),
        )
    }
}

// ── hub.json v1 ─────────────────────────────────────────────────────────────────────────────────

pub fn hub_json(devices: &[Phone]) -> String {
    let devices: Vec<String> = devices
        .iter()
        .map(|p| {
            format!(
                r#"{{ "name": "{}", "dest_hash": "{}", "cert": "{}" }}"#,
                p.name,
                hex(&p.id),
                base64(&p.cert)
            )
        })
        .collect();
    format!(
        r#"{{ "format_version": 1,
  "listen": "127.0.0.1:0",
  "hub_ed25519": {{ "priv": "{}", "pub": "{}" }},
  "devices": [ {} ] }}"#,
        hex(&HUB_SEED),
        hex(&hub_public()),
        devices.join(",\n               ")
    )
}

pub fn test_config() -> Config {
    config::parse(&hub_json(&Phone::directory())).expect("test hub.json geçerli")
}

// ── Dünya ───────────────────────────────────────────────────────────────────────────────────────

pub struct World {
    pub hub: Hub<FakeClock, TestRng>,
    pub clock: FakeClock,
    pub net: Net,
    pub hub_pub: [u8; 32],
    nonce: u64,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self::with_limits(Limits::S4)
    }

    pub fn with_limits(limits: Limits) -> Self {
        let clock = FakeClock::default();
        Self {
            hub: Hub::with_limits(test_config(), limits, clock.clone(), TestRng(1)).expect("hub"),
            clock,
            net: Net::default(),
            hub_pub: hub_public(),
            nonce: 0,
        }
    }

    pub fn advance(&mut self, by: Duration) {
        self.clock.advance(by);
    }

    pub fn deliver(&mut self, from: SocketAddr, datagram: &[u8]) -> Vec<Sent> {
        let before = self.net.sent.len();
        self.hub.on_datagram(from, datagram, &mut self.net);
        self.hub
            .table()
            .check_invariants()
            .expect("tablo değişmezleri");
        self.net.sent[before..].to_vec()
    }

    /// Zamanlayıcı; ürettiği logları döndürür, datagram üretmediğini doğrular (H10).
    pub fn tick(&mut self) -> Vec<String> {
        let (sent, logs) = (self.net.sent.len(), self.net.logs.len());
        self.hub.on_tick(&mut self.net);
        assert_eq!(
            self.net.sent.len(),
            sent,
            "H10: zamanlayıcı datagram yollamaz"
        );
        self.net.logs[logs..].to_vec()
    }

    pub fn fresh_nonce(&mut self) -> Nonce {
        self.nonce += 1;
        let mut n = [0xa5u8; 16];
        n[..8].copy_from_slice(&self.nonce.to_be_bytes());
        n
    }

    /// 1. tur: `(request_id, client_nonce, hub_nonce)`.
    pub fn challenge(&mut self, phone: &mut Phone) -> (u32, Nonce, Nonce) {
        let rid = phone.rid();
        let cn = self.fresh_nonce();
        let out = self.deliver(phone.addr, &phone.bind1(rid, cn));
        let (to, bytes) = only(&out);
        assert_eq!(*to, phone.addr);
        let r = reply(bytes);
        assert_eq!(
            (r.method_id, r.flags, r.request_id),
            (method::CLIENT_BIND, flags::RESPONSE, rid)
        );
        let hn = ClientBindResponse::decode(&r.payload)
            .expect("CHALLENGE")
            .hub_nonce;
        (rid, cn, hn)
    }

    /// İki tur; başarı yanıtı ve hub imzası doğrulanır. Yanıt baytları döner.
    pub fn bind(&mut self, phone: &mut Phone) -> Vec<u8> {
        let (_, cn, hn) = self.challenge(phone);
        let rid = phone.rid();
        let out = self.deliver(phone.addr, &phone.bind2(rid, &self.hub_pub, cn, hn));
        let (to, bytes) = only(&out);
        assert_eq!(*to, phone.addr);
        self.expect_bound(bytes, phone, rid, cn, hn);
        bytes.clone()
    }

    pub fn expect_bound(&self, bytes: &[u8], phone: &Phone, rid: u32, cn: Nonce, hn: Nonce) {
        let r = reply(bytes);
        assert_eq!(
            (r.method_id, r.flags, r.request_id),
            (method::BIND_PROOF, flags::RESPONSE, rid),
            "{r:?}"
        );
        let ack = BindProofResponse::decode(&r.payload).expect("bind_ok yanıtı");
        let tbs = bind_ack_tbs(&phone.cert_ref, &cn, &hn, BIND_OK, code::OK);
        assert_eq!(
            verify_strict(&self.hub_pub, &tbs, &ack.hub_signature),
            Ok(()),
            "hub imzası"
        );
    }

    /// INIT ile hat açar; `{OK, h}` ve alıcıya DELIVER doğrulanır.
    pub fn open(
        &mut self,
        from: &mut Phone,
        to: &Phone,
        envelope_type: u8,
        envelope: &[u8],
    ) -> u32 {
        let rid = from.rid();
        let out = self.deliver(from.addr, &from.store(rid, 0, envelope_type, to, envelope));
        assert_eq!(out.len(), 2, "yanıt + DELIVER bekleniyordu");
        let response = store_response(sent_to(&out, from.addr));
        assert_eq!(response.status, code::OK);
        let (h, sender, kind, env) = deliver(sent_to(&out, to.addr));
        assert_eq!(
            (h, sender, kind, env.as_slice()),
            (response.assigned_handle, from.id, envelope_type, envelope)
        );
        response.assigned_handle
    }

    pub fn link_count_of(&self, phone: &Phone) -> usize {
        self.hub
            .table()
            .endpoint(&phone.id)
            .map_or(0, |e| e.link_count)
    }
}
