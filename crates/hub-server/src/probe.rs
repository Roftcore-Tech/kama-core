//! `hub-server probe --device <device.json> --hub <host:port>` (KM-HUB-TDD-001 §5; KM-DEPLOY-TDD-001
//! §4): iki turlu bind + 3 heartbeat; sağlık kontrolü ve kurulum doğrulaması.
//!
//! Probe **kendi cihazıyla** bağlanır (`device-probe.json`, kama-provision v1 `--probe`). Alice/Bob
//! dosyasıyla probe yapılmaz: aynı kimlikle bind, telefonun dönüş adresini laptopa taşır.
//!
//! Cihaz dosyasından okunan alanlar (KM-HUB-TDD-001 v0.6.1 §5: v1 = bugünkü v0 anahtar adları +
//! `hub.ed25519_pub`; geri kalanı yok sayılır): `ed25519.priv` (64 hex — sertifikadaki cihaz anahtarı),
//! `cert` (204 base64), `hub.addr` (`--hub` verilmezse), `hub.ed25519_pub` (64 hex — hub imzasını
//! doğrulamak için; S4 C2 "hub imzası doğrulanmadan bind tamam sayılmaz").
//! Her istek en çok 3 kez, 1 s arayla yollanır (S4 C2).

use core::fmt;
use core::time::Duration;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::time::Instant;

use ed25519_dalek::{Signer, SigningKey};
use kama_noise_wire::ed25519_strict::verify_strict;
use kama_noise_wire::entropy::EntropySource;
use kama_noise_wire::handshake::{cert_ref_of, COMPACT_DEVICE_CERT_LEN};

use crate::config::{base64_decode, hex_decode};
use crate::wire::{
    bind_ack_tbs, bind_proof_tbs, classify, code, control_datagram, flags, frame, method,
    BindProofRequest, BindProofResponse, CertRef, ClientBindRequest, ClientBindResponse, Datagram,
    ErrorPayload, Nonce, Payload, RpcFrame, BIND_OK, ECHO_TOKEN_LEN, MAX_DATAGRAM, NONCE_LEN,
    PUBLIC_KEY_LEN,
};

/// Bir isteğin gönderim sayısı ve bekleme süresi (S4 C2: en çok 3 kez, 1 s arayla).
pub const ATTEMPTS: u32 = 3;
pub const WAIT: Duration = Duration::from_secs(1);
/// Heartbeat sayısı.
pub const HEARTBEATS: u32 = 3;

/// Probe'un cihazı.
pub struct ProbeDevice {
    pub cert_ref: CertRef,
    signing: SigningKey,
    pub hub_pub: [u8; PUBLIC_KEY_LEN],
    /// Cihaz dosyasındaki `hub.addr` (`host:port`); `--hub` verilmezse kullanılır.
    pub hub_addr: Option<String>,
}

impl fmt::Debug for ProbeDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProbeDevice(<redacted>)")
    }
}

/// Probe hatası; dosya içeriği taşımaz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    DeviceUnreadable,
    DeviceInvalid(&'static str),
    Socket(String),
    /// Hub cevap vermedi (dizinde değil, erişilemiyor ya da kayıp).
    Timeout(&'static str),
    /// Hub hata çerçevesi döndü.
    Refused(u16),
    /// Hub imzası doğrulanmadı.
    BadHubSignature,
    /// Rastgelelik yok.
    Entropy,
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceUnreadable => f.write_str("device file unreadable"),
            Self::DeviceInvalid(what) => write!(f, "device file invalid: {what}"),
            Self::Socket(error) => write!(f, "socket: {error}"),
            Self::Timeout(step) => write!(f, "{step}: no answer after {ATTEMPTS} attempts"),
            Self::Refused(code) => write!(f, "hub refused: error 0x{code:04x}"),
            Self::BadHubSignature => f.write_str("hub signature does not verify"),
            Self::Entropy => f.write_str("entropy source unavailable"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// Probe sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// BIND_PROOF gidiş-dönüş süresi.
    pub bind_rtt: Duration,
    /// Cevaplanan heartbeat sayısı.
    pub heartbeats: u32,
}

impl Outcome {
    /// Probe yeşil mi: bind tamam ve bütün heartbeat'ler cevaplandı.
    #[must_use]
    pub fn healthy(&self) -> bool {
        self.heartbeats == HEARTBEATS
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bind ok rtt={} ms · heartbeat {}/{HEARTBEATS}",
            self.bind_rtt.as_millis(),
            self.heartbeats
        )
    }
}

/// Cihaz dosyasını okur.
///
/// # Errors
/// [`ProbeError::DeviceUnreadable`] · [`ProbeError::DeviceInvalid`].
pub fn load_device(path: &Path) -> Result<ProbeDevice, ProbeError> {
    let text = std::fs::read_to_string(path).map_err(|_| ProbeError::DeviceUnreadable)?;
    parse_device(&text)
}

/// Cihaz dosyası metnini çözer.
///
/// # Errors
/// [`ProbeError::DeviceInvalid`].
pub fn parse_device(text: &str) -> Result<ProbeDevice, ProbeError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| ProbeError::DeviceInvalid("not JSON"))?;
    let field = |path: &[&str]| {
        path.iter()
            .try_fold(&value, |node, key| node.get(key))
            .and_then(serde_json::Value::as_str)
    };
    let secret: [u8; 32] = field(&["ed25519", "priv"])
        .and_then(hex_decode)
        .ok_or(ProbeError::DeviceInvalid("ed25519.priv"))?;
    let cert: [u8; COMPACT_DEVICE_CERT_LEN] = field(&["cert"])
        .and_then(base64_decode)
        .ok_or(ProbeError::DeviceInvalid("cert"))?;
    let hub_pub: [u8; PUBLIC_KEY_LEN] = field(&["hub", "ed25519_pub"])
        .and_then(hex_decode)
        .ok_or(ProbeError::DeviceInvalid("hub.ed25519_pub"))?;
    let signing = SigningKey::from_bytes(&secret);
    let mut cert_pub = [0u8; PUBLIC_KEY_LEN];
    for (slot, byte) in cert_pub.iter_mut().zip(&cert) {
        *slot = *byte;
    }
    if signing.verifying_key().to_bytes() != cert_pub {
        return Err(ProbeError::DeviceInvalid(
            "ed25519.priv does not match cert",
        ));
    }
    Ok(ProbeDevice {
        cert_ref: cert_ref_of(&cert),
        signing,
        hub_pub,
        hub_addr: field(&["hub", "addr"]).map(str::to_owned),
    })
}

/// Hub'a bağlanır (iki tur) ve 3 heartbeat yollar.
///
/// # Errors
/// [`ProbeError`] — bind tamamlanamadı. Heartbeat kaybı hata değildir; [`Outcome::heartbeats`]'te görünür.
pub fn run(
    device: &ProbeDevice,
    hub: SocketAddr,
    rng: &mut dyn EntropySource,
) -> Result<Outcome, ProbeError> {
    let local: SocketAddr = if hub.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
    };
    let socket = UdpSocket::bind(local).map_err(|error| ProbeError::Socket(error.to_string()))?;
    socket
        .connect(hub)
        .map_err(|error| ProbeError::Socket(error.to_string()))?;
    socket
        .set_read_timeout(Some(WAIT))
        .map_err(|error| ProbeError::Socket(error.to_string()))?;

    // S4 C2/C8: imzasız BAD_COOKIE ya da RATE_LIMITED bind'i bitirmez → 1. tura dön (yeni nonce).
    let mut attempt = 0;
    let (request_id, bind_rtt) = loop {
        attempt += 1;
        match bind_once(&socket, device, rng) {
            Err(ProbeError::Refused(code::BAD_COOKIE | code::RATE_LIMITED))
                if attempt < ATTEMPTS =>
            {
                std::thread::sleep(WAIT);
            }
            other => break other?,
        }
    };

    // Heartbeat.
    let mut heartbeats = 0;
    for n in 0..HEARTBEATS {
        let token = request_id.wrapping_add(n).to_be_bytes();
        if heartbeat(&socket, token) {
            heartbeats += 1;
        }
    }
    Ok(Outcome {
        bind_rtt,
        heartbeats,
    })
}

/// Bir bind denemesi (iki tur); `(son request_id, BIND_PROOF gidiş-dönüşü)`.
fn bind_once(
    socket: &UdpSocket,
    device: &ProbeDevice,
    rng: &mut dyn EntropySource,
) -> Result<(u32, Duration), ProbeError> {
    let mut random = [0u8; 4 + NONCE_LEN];
    rng.fill(&mut random).map_err(|_| ProbeError::Entropy)?;
    let (rid_bytes, nonce_bytes) = random.split_at(4);
    let request_id = u32::from_be_bytes(rid_bytes.try_into().map_err(|_| ProbeError::Entropy)?);
    let client_nonce: Nonce = nonce_bytes.try_into().map_err(|_| ProbeError::Entropy)?;

    let bind1 = ClientBindRequest {
        cert_ref: device.cert_ref,
        client_nonce,
    };
    let (answer, _) = exchange(
        socket,
        method::CLIENT_BIND,
        request_id,
        &bind1.to_vec(),
        "CLIENT_BIND",
    )?;
    let hub_nonce = ClientBindResponse::decode(&answer)
        .map_err(|_| ProbeError::Timeout("CLIENT_BIND"))?
        .hub_nonce;

    let tbs = bind_proof_tbs(&device.hub_pub, &device.cert_ref, &client_nonce, &hub_nonce);
    let proof = BindProofRequest {
        cert_ref: device.cert_ref,
        client_nonce,
        hub_nonce,
        signature: device.signing.sign(&tbs).to_bytes(),
    };
    let proof_id = request_id.wrapping_add(1);
    let (answer, rtt) = exchange(
        socket,
        method::BIND_PROOF,
        proof_id,
        &proof.to_vec(),
        "BIND_PROOF",
    )?;
    let response =
        BindProofResponse::decode(&answer).map_err(|_| ProbeError::Timeout("BIND_PROOF"))?;
    let ack = bind_ack_tbs(
        &device.cert_ref,
        &client_nonce,
        &hub_nonce,
        BIND_OK,
        code::OK,
    );
    verify_strict(&device.hub_pub, &ack, &response.hub_signature)
        .map_err(|_| ProbeError::BadHubSignature)?;
    Ok((proof_id, rtt))
}

/// İstek yollar, eşleşen yanıtı bekler (en çok [`ATTEMPTS`] kez). Hata çerçevesi → [`ProbeError::Refused`].
fn exchange(
    socket: &UdpSocket,
    method_id: u8,
    request_id: u32,
    payload: &[u8],
    step: &'static str,
) -> Result<(Vec<u8>, Duration), ProbeError> {
    let mut datagram = Vec::new();
    RpcFrame {
        method_id,
        flags: flags::REQUEST,
        request_id,
        payload,
    }
    .encode(&mut datagram)
    .map_err(|_| ProbeError::Socket("request too long".to_owned()))?;
    for _ in 0..ATTEMPTS {
        let sent_at = Instant::now();
        socket
            .send(&datagram)
            .map_err(|error| ProbeError::Socket(error.to_string()))?;
        while let Some(reply) = receive(socket, sent_at) {
            let Ok(Datagram::Control {
                frame_type: frame::PLANE1_RPC,
                body,
            }) = classify(&reply)
            else {
                continue;
            };
            let Ok(answer) = RpcFrame::decode(body) else {
                continue;
            };
            if answer.method_id != method_id || answer.request_id != request_id {
                continue;
            }
            if answer.flags == flags::RESPONSE_ERROR {
                let code = ErrorPayload::decode(answer.payload).map_or(0, |error| error.code);
                return Err(ProbeError::Refused(code));
            }
            if answer.flags == flags::RESPONSE {
                return Ok((answer.payload.to_vec(), sent_at.elapsed()));
            }
        }
    }
    Err(ProbeError::Timeout(step))
}

/// Bir heartbeat; aynı token'lı ACK geldiyse `true`.
fn heartbeat(socket: &UdpSocket, token: [u8; ECHO_TOKEN_LEN]) -> bool {
    let mut datagram = Vec::new();
    control_datagram(&mut datagram, frame::HEARTBEAT, &token);
    for _ in 0..ATTEMPTS {
        let sent_at = Instant::now();
        if socket.send(&datagram).is_err() {
            return false;
        }
        while let Some(reply) = receive(socket, sent_at) {
            if let Ok(Datagram::Control {
                frame_type: frame::HEARTBEAT_ACK,
                body,
            }) = classify(&reply)
            {
                if body == token {
                    return true;
                }
            }
        }
    }
    false
}

/// `sent_at + WAIT`'e kadar bir datagram bekler.
fn receive(socket: &UdpSocket, sent_at: Instant) -> Option<Vec<u8>> {
    let remaining = WAIT.checked_sub(sent_at.elapsed())?;
    socket
        .set_read_timeout(Some(remaining.max(Duration::from_millis(1))))
        .ok()?;
    let mut buffer = vec![0u8; MAX_DATAGRAM + 1];
    let length = socket.recv(&mut buffer).ok()?;
    buffer.truncate(length);
    Some(buffer)
}
