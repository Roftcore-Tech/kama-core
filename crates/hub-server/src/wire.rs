//! Tel biçimi — `KM-SEAM-TH-001` (S4 v1.3.1) §2.
//!
//! Codec, sabitler, TBS ve tahsis anahtarı ortak sözleşmeden gelir: [`kama_contracts::s4`] buradan
//! aynen yeniden ihraç edilir. Bu dosyada yalnız hub'a özgü olanlar durur:
//!
//! - datagram sınıfı ([`classify`]) ve kuyruklu kontrol datagramı ([`control_datagram`]) — `s4` kuyruğu
//!   seam'e bırakır, hub PoC no-op `seal_hub`'ı kendisi ekler ve doğrular;
//! - [`RpcFrame`]: `AppRpcFrame`'in ödünç alan, bellek ayırmayan görünümü (aynı baytlar, aynı ret
//!   kümesi). Alım yolu ve CLIENT_BIND 1. turu bellek AYIRMAZ (S4 H3; `tests/dos_memory.rs`);
//!   `s4::AppRpcFrame` yükü sahiplenir. Aynı sebeple [`client_bind_challenge`];
//! - sırlı [`cookie_input`] (`s4::cookie_input` sırdan SONRAKİ baytları `Vec` olarak verir; buradaki
//!   sırrı başa koyar ve yığında kalır) ve [`handle_candidate_input`].
//!
//! İki datagram sınıfı (S4 §2.1, hub yalnız ilk dört bayta bakar):
//! ```text
//! kontrol : [00 00 00 00][frame_type u8][gövde][16 × 00]      en az 21 B
//! veri    : [hub_handle u32 ≠ 0][opak yük]                    en az 5 B
//! ```
//! Veri datagramının ilk dört baytı dışındaki hiçbir bayta bu modülde bakılmaz.

use core::num::NonZeroU32;
use std::net::SocketAddr;

pub use kama_contracts::s4::*;

/// Kontrol datagramının ilk dört baytı.
pub const CONTROL_PREFIX: [u8; CONTROL_PREFIX_LEN] = [0; CONTROL_PREFIX_LEN];
/// PoC no-op `seal_hub`: çerçevenin sonuna 16 sıfır bayt.
pub const HUB_TRAILER: [u8; HUB_TRAILER_LEN] = [0; HUB_TRAILER_LEN];
/// Başarılı BIND_PROOF yanıtındaki `bind_ok` değeri.
pub const BIND_OK: u8 = 1;

// ── Datagram sınıfı ────────────────────────────────────────────────────────────────────────────

/// Datagramın sınıfı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Datagram<'a> {
    /// HubChannel kontrol çerçevesi; `body` kuyruktan önceki gövdedir.
    Control { frame_type: u8, body: &'a [u8] },
    /// Veri datagramı; hub yalnız handle'ı okur.
    Data { handle: NonZeroU32 },
}

/// İlk dört bayta göre sınıflar. Kontrol için kuyruğu doğrular; veri için yüke BAKMAZ.
///
/// # Errors
/// [`S4Error::Truncated`] — dört bayttan kısa; [`S4Error::NotControl`] — kontrol öneki var ama
/// datagram 21 bayttan kısa ya da son 16 bayt sıfır değil.
pub fn classify(datagram: &[u8]) -> Result<Datagram<'_>, S4Error> {
    let (head, rest) = datagram
        .split_first_chunk::<4>()
        .ok_or(S4Error::Truncated)?;
    if let Some(handle) = NonZeroU32::new(u32::from_be_bytes(*head)) {
        return Ok(Datagram::Data { handle });
    }
    let (inner, trailer) = rest.split_last_chunk::<16>().ok_or(S4Error::NotControl)?;
    if *trailer != HUB_TRAILER {
        return Err(S4Error::NotControl);
    }
    let (frame_type, body) = inner.split_first().ok_or(S4Error::NotControl)?;
    Ok(Datagram::Control {
        frame_type: *frame_type,
        body,
    })
}

/// HubChannel kontrol datagramı yazar: `[0;4] ‖ frame_type ‖ gövde ‖ [0;16]`. `out` önce boşaltılır.
pub fn control_datagram(out: &mut Vec<u8>, frame_type: u8, body: &[u8]) {
    out.clear();
    out.extend_from_slice(&CONTROL_PREFIX);
    out.push(frame_type);
    out.extend_from_slice(body);
    out.extend_from_slice(&HUB_TRAILER);
}

// ── AppRpcFrame, ödünç alan görünüm ─────────────────────────────────────────────────────────────

/// `AppRpcFrame` (S4 §2.3) — [`AppRpcFrame`] ile aynı baytlar; yükü kopyalamaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcFrame<'a> {
    pub method_id: u8,
    pub flags: u8,
    pub request_id: u32,
    pub payload: &'a [u8],
}

impl<'a> RpcFrame<'a> {
    /// `0x1D PLANE1_RPC` gövdesinden çözer. Ret kümesi [`AppRpcFrame::decode`] ile aynıdır.
    ///
    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TooLong`] · [`S4Error::LengthMismatch`].
    pub fn decode(body: &'a [u8]) -> Result<Self, S4Error> {
        let (header, payload) = body
            .split_first_chunk::<RPC_HEADER_LEN>()
            .ok_or(S4Error::Truncated)?;
        let [method_id, flags, r0, r1, r2, r3, l0, l1, l2, l3] = *header;
        let declared =
            usize::try_from(u32::from_be_bytes([l0, l1, l2, l3])).map_err(|_| S4Error::TooLong)?;
        if declared > MAX_RPC_PAYLOAD {
            return Err(S4Error::TooLong);
        }
        if declared != payload.len() {
            return Err(S4Error::LengthMismatch);
        }
        Ok(Self {
            method_id,
            flags,
            request_id: u32::from_be_bytes([r0, r1, r2, r3]),
            payload,
        })
    }

    /// Tam kontrol datagramını (`0x1D` + çerçeve + kuyruk) `out`'a yazar.
    ///
    /// # Errors
    /// [`S4Error::TooLong`] — datagram `MAX_DATAGRAM`'ı aşar; `out` boş kalır.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), S4Error> {
        out.clear();
        let len = u32::try_from(self.payload.len()).map_err(|_| S4Error::TooLong)?;
        if self.payload.len() > MAX_RPC_PAYLOAD {
            return Err(S4Error::TooLong);
        }
        out.extend_from_slice(&CONTROL_PREFIX);
        out.push(FRAME_PLANE1_RPC);
        out.push(self.method_id);
        out.push(self.flags);
        out.extend_from_slice(&self.request_id.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(self.payload);
        out.extend_from_slice(&HUB_TRAILER);
        Ok(())
    }
}

/// CLIENT_BIND meydan okuma yükü, yığında: [`ClientBindResp::encode`] ile bayt-özdeş (birim testi
/// kilitler). 1. tur seli bellek ayırmamalı (S4 H3; T-B10).
#[must_use]
pub fn client_bind_challenge(hub_nonce: &Nonce) -> [u8; ClientBindResp::LEN] {
    concat(&[&[0; 17], &STATUS_CHALLENGE.to_be_bytes(), hub_nonce])
}

// ── Hub'ın sırlı bayt birleştiricileri (S4 §2.4(a), §2.6; kripto YOK) ───────────────────────────

/// Cookie sırrı boyu.
pub const SECRET_LEN: usize = 32;
/// Cookie girdisinin en uzun hâli (IPv6).
pub const COOKIE_INPUT_MAX: usize = SECRET_LEN + 1 + 16 + 2 + CERT_REF_LEN + NONCE_LEN + 4 + 4;

/// Cookie girdisi (S4 v1.3.1 §2.4(a); IPv4-eşlemeli IPv6 IPv4'e normalize): `cookie_secret ‖ fam u8 ‖ src_ip (4|16) ‖ src_port u16 ‖
/// device_cert_ref ‖ client_nonce ‖ minute u32 ‖ bind_epoch u32`. `hub_nonce` bunun BLAKE2s-256'sının
/// ilk 16 baytıdır. Dönüş: tampon + kullanılan uzunluk. (`s4::cookie_input`'u bilerek gölgeler.)
#[must_use]
pub fn cookie_input(
    secret: &[u8; SECRET_LEN],
    src: SocketAddr,
    cert_ref: &CertRef,
    client_nonce: &Nonce,
    minute: u32,
    bind_epoch: u32,
) -> ([u8; COOKIE_INPUT_MAX], usize) {
    // Aile baytı ve `::ffff:a.b.c.d` normalizasyonu ortak sözleşmeden (S4 v1.3.1 §2.4(a)).
    let src = CookieSrc::from_socket_addr(src);
    let (ip, port): (&[u8], u16) = match &src {
        CookieSrc::V4 { ip, port } => (ip, *port),
        CookieSrc::V6 { ip, port } => (ip, *port),
    };
    let parts: [&[u8]; 8] = [
        secret,
        &[src.fam()],
        ip,
        &port.to_be_bytes(),
        cert_ref,
        client_nonce,
        &minute.to_be_bytes(),
        &bind_epoch.to_be_bytes(),
    ];
    let len = parts.iter().map(|part| part.len()).sum();
    (concat(&parts), len)
}

/// Handle adayı girdisi (S4 §2.6): `handle_secret ‖ alloc_counter u64`; aday = BLAKE2s-256'nın ilk 4
/// baytı (big-endian `u32`).
#[must_use]
pub fn handle_candidate_input(secret: &[u8; SECRET_LEN], counter: u64) -> [u8; SECRET_LEN + 8] {
    concat(&[secret, &counter.to_be_bytes()])
}

/// Tahsis anahtarındaki zarf özeti boyu (`BLAKE2s(envelope)[0..8]`).
pub const ENVELOPE_DIGEST_LEN: usize = 8;

/// Parçaları `N` baytlık diziye dizer; toplam `N`'den kısaysa kalan sıfır, uzunsa fazlası atılır
/// (çağıranlar sabit boy verir; birim testleri kilitler). Panik yok.
fn concat<const N: usize>(parts: &[&[u8]]) -> [u8; N] {
    let mut out = [0u8; N];
    for (slot, byte) in out
        .iter_mut()
        .zip(parts.iter().flat_map(|part| part.iter()))
    {
        *slot = *byte;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn classify_checks_prefix_trailer_and_reads_only_the_handle() {
        assert_eq!(classify(&[1, 2, 3]), Err(S4Error::Truncated));
        assert_eq!(
            classify(&[0, 0, 0, 7, 0xff]),
            Ok(Datagram::Data {
                handle: NonZeroU32::new(7).expect("sıfır değil")
            })
        );
        let mut control = Vec::new();
        control_datagram(&mut control, FRAME_HEARTBEAT, &[9, 9, 9, 9]);
        assert_eq!(control.len(), 25);
        assert_eq!(
            classify(&control),
            Ok(Datagram::Control {
                frame_type: FRAME_HEARTBEAT,
                body: &[9, 9, 9, 9]
            })
        );
        let mut dirty = control.clone();
        if let Some(last) = dirty.last_mut() {
            *last = 1;
        }
        assert_eq!(classify(&dirty), Err(S4Error::NotControl));
        assert_eq!(classify(&control[..20]), Err(S4Error::NotControl));
        control_datagram(&mut control, FRAME_HEARTBEAT, &[]);
        assert_eq!(control.len(), CONTROL_MIN_LEN);
        assert!(classify(&control).is_ok());
    }

    /// Ödünç alan görünüm `s4::AppRpcFrame` ile aynı baytları yazar, aynı girdiyi aynı adla reddeder.
    #[test]
    fn rpc_frame_matches_the_contract_frame() {
        let frame = RpcFrame {
            method_id: METHOD_STORE_MESSAGE,
            flags: 0,
            request_id: 0xdead_beef,
            payload: b"yuk",
        };
        let mut datagram = Vec::new();
        frame.encode(&mut datagram).expect("kodlanmalı");
        let Ok(Datagram::Control { frame_type, body }) = classify(&datagram) else {
            panic!("kontrol bekleniyordu");
        };
        assert_eq!(frame_type, FRAME_PLANE1_RPC);
        let owned = AppRpcFrame {
            method_id: METHOD_STORE_MESSAGE,
            flags: 0,
            request_id: 0xdead_beef,
            payload: b"yuk".to_vec(),
        };
        assert_eq!(body, owned.encode().expect("s4").as_slice());
        assert_eq!(RpcFrame::decode(body), Ok(frame));

        let mut longer = body.to_vec();
        longer.push(0);
        let over = [0x05, 0, 0, 0, 0, 0, 0, 0, 0x04, 0xe2]; // payload_len = 1250
        for bad in [&longer[..], &body[..body.len() - 1], &body[..9], &over[..]] {
            let expected = AppRpcFrame::decode(bad).expect_err("s4 reddeder");
            assert_eq!(RpcFrame::decode(bad), Err(expected));
        }

        let big = vec![0u8; MAX_RPC_PAYLOAD + 1];
        let mut out = vec![1];
        let frame = RpcFrame {
            method_id: 1,
            flags: 0,
            request_id: 0,
            payload: &big,
        };
        assert_eq!(frame.encode(&mut out), Err(S4Error::TooLong));
        assert!(out.is_empty());
        assert!(RpcFrame {
            payload: &big[1..],
            ..frame
        }
        .encode(&mut out)
        .is_ok());
        assert_eq!(out.len(), MAX_DATAGRAM);
    }

    #[test]
    fn client_bind_challenge_is_the_contract_encoding() {
        let hub_nonce = [0x22; 16];
        assert_eq!(
            client_bind_challenge(&hub_nonce).as_slice(),
            ClientBindResp { hub_nonce }.encode().as_slice()
        );
    }

    #[test]
    fn cookie_and_handle_inputs_have_the_documented_layout() {
        let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7)), 0x9c41);
        let (input, len) = cookie_input(&[0x55; 32], v4, &[0xbb; 32], &[0x11; 16], 3, 7);
        let mut expected = vec![0x55; 32];
        expected.push(4);
        expected.extend_from_slice(&[198, 51, 100, 7, 0x9c, 0x41]);
        expected.extend_from_slice(&[0xbb; 32]);
        expected.extend_from_slice(&[0x11; 16]);
        expected.extend_from_slice(&[0, 0, 0, 3, 0, 0, 0, 7]);
        assert_eq!(&input[..len], expected.as_slice());
        // Sırdan sonrası ortak sözleşmenin sırsız girdisidir.
        let shared = kama_contracts::s4::cookie_input(
            &CookieSrc::from_socket_addr(v4),
            &[0xbb; 32],
            &[0x11; 16],
            3,
            7,
        );
        assert_eq!(&input[SECRET_LEN..len], shared.as_slice());

        let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1);
        let (_, len6) = cookie_input(&[0x55; 32], v6, &[0xbb; 32], &[0x11; 16], 3, 7);
        assert_eq!(len6, COOKIE_INPUT_MAX);
        assert_eq!(len6, len + 12);

        let candidate = handle_candidate_input(&[9; 32], 0x0102);
        assert_eq!(&candidate[32..], &[0, 0, 0, 0, 0, 0, 1, 2]);
    }
}
