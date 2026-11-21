//! Bayt düzenleri — S4 §2.3 mesaj tablosunun satır satır karşılığı.
//!
//! Her satırın beklenen datagramı burada ALAN ALAN elle dizilir (codec'in kodlayıcısı kullanılmadan) ve
//! iki yönde sınanır: ortak sözleşmenin codec'i (`kama_contracts::s4`, `hub_server::wire` üzerinden) ve
//! hub'ın kendi sarması (önek ‖ `0x1D` ‖ çerçeve ‖ 16 sıfır). Altın vektörlerin (`vectors/hub/`) YERİNE
//! GEÇMEZ: aynı elden çıkar. Alan değerleri ayırt edilebilir seçilmiştir; yer değiştiren iki alan testi
//! düşürür.

use hub_server::wire::{
    classify, BindProofReq, BindProofResp, ClientBindReq, ClientBindResp, Datagram, Deliver,
    ErrorPayload, RouteMiss, RpcBody, RpcFrame, RpcMessage, StoreMessageReq, StoreMessageResp,
    ERROR_BAD_COOKIE, ERROR_BAD_REQUEST, ERROR_HUB_CAPACITY, ERROR_POSSESSION_PROOF_FAIL,
    ERROR_RATE_LIMITED,
};

const R: [u8; 4] = [0x0a, 0x0b, 0x0c, 0x0d];
const CERT_REF: [u8; 32] = [0xcf; 32];
const CN: [u8; 16] = [0x1c; 16];
const HN: [u8; 16] = [0x2b; 16];
const SIG: [u8; 64] = [0x5e; 64];
const BOB: [u8; 32] = [0xb0; 32];
const ZARF: &[u8] = &[0x7a, 0x61, 0x72, 0x66, 0x00, 0xff];

/// `[0;4] ‖ 0x1D ‖ method ‖ flags ‖ request_id ‖ payload_len ‖ payload ‖ [0;16]`.
fn datagram(method_id: u8, flags: u8, request_id: [u8; 4], payload: &[&[u8]]) -> Vec<u8> {
    let body: Vec<u8> = payload.concat();
    let mut out = vec![0, 0, 0, 0, 0x1d, method_id, flags];
    out.extend_from_slice(&request_id);
    out.extend_from_slice(&u32::try_from(body.len()).expect("u32").to_be_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&[0; 16]);
    out
}

/// Hata satırı: `detail` boş (hub böyle yollar) → yük `code ‖ 00 00`.
fn error(method_id: u8, error_code: u16) -> RpcBody {
    RpcBody::Error {
        method_id,
        payload: ErrorPayload {
            error_code,
            detail: Vec::new(),
        },
    }
}

fn check(
    expected: &[u8],
    method_id: u8,
    flags: u8,
    request_id: u32,
    row: RpcBody,
    payload_len: usize,
) {
    let Ok(Datagram::Control {
        frame_type: 0x1d,
        body,
    }) = classify(expected)
    else {
        panic!("kontrol datagramı değil");
    };
    let frame = RpcFrame::decode(body).expect("çerçeve");
    assert_eq!(
        (frame.method_id, frame.flags, frame.request_id),
        (method_id, flags, request_id)
    );
    assert_eq!(frame.payload.len(), payload_len, "tablodaki alan boyları");
    let message = RpcMessage {
        request_id,
        body: row,
    };
    assert_eq!(message.encode().as_deref(), Ok(body), "kodlama");
    assert_eq!(RpcMessage::decode(body), Ok(message), "çözme");
    let mut encoded = Vec::new();
    RpcFrame {
        method_id,
        flags,
        request_id,
        payload: frame.payload,
    }
    .encode(&mut encoded)
    .expect("kodla");
    assert_eq!(encoded, expected);
}

#[test]
fn bind_rows() {
    let rid = u32::from_be_bytes(R);
    // CLIENT_BIND istek: mode 0 · cert_ref · presence 0 · client_nonce
    let d = datagram(0x01, 0x00, R, &[&[0x00], &CERT_REF, &[0x00], &CN]);
    check(
        &d,
        0x01,
        0x00,
        rid,
        RpcBody::ClientBindReq(ClientBindReq {
            device_cert_ref: CERT_REF,
            client_nonce: CN,
        }),
        50,
    );
    // CLIENT_BIND yanıt: bind_ok 0 · cursor u64 0 · rev u64 0 · 0x0003 · hub_nonce
    let d = datagram(
        0x01,
        0x01,
        R,
        &[&[0x00], &[0; 8], &[0; 8], &[0x00, 0x03], &HN],
    );
    check(
        &d,
        0x01,
        0x01,
        rid,
        RpcBody::ClientBindResp(ClientBindResp { hub_nonce: HN }),
        35,
    );
    // CLIENT_BIND hata
    let d = datagram(0x01, 0x03, R, &[&[0x06, 0x01], &[0, 0]]);
    check(&d, 0x01, 0x03, rid, error(0x01, ERROR_BAD_REQUEST), 4);
    // BIND_PROOF istek: cert_ref · client_nonce · hub_nonce · imza
    let d = datagram(0x0b, 0x00, R, &[&CERT_REF, &CN, &HN, &SIG]);
    check(
        &d,
        0x0b,
        0x00,
        rid,
        RpcBody::BindProofReq(BindProofReq {
            device_cert_ref: CERT_REF,
            client_nonce: CN,
            hub_nonce: HN,
            client_signature: SIG,
        }),
        128,
    );
    // BIND_PROOF yanıt: bind_ok 1 · 0 · 0 · OK · hub imzası
    let d = datagram(0x0b, 0x01, R, &[&[0x01], &[0; 8], &[0; 8], &[0, 0], &SIG]);
    check(
        &d,
        0x0b,
        0x01,
        rid,
        RpcBody::BindProofResp(BindProofResp { hub_signature: SIG }),
        83,
    );
    // BIND_PROOF hataları
    for (bytes, value) in [
        ([0x01, 0x06], ERROR_BAD_COOKIE),
        ([0x01, 0x05], ERROR_POSSESSION_PROOF_FAIL),
        ([0x05, 0x02], ERROR_RATE_LIMITED),
        ([0x05, 0x01], ERROR_HUB_CAPACITY),
    ] {
        let d = datagram(0x0b, 0x03, R, &[&bytes, &[0, 0]]);
        check(&d, 0x0b, 0x03, rid, error(0x0b, value), 4);
    }
}

#[test]
fn store_deliver_and_route_miss_rows() {
    let rid = u32::from_be_bytes(R);
    // STORE_MESSAGE istek: hub_handle · tip · alıcı · envelope_len u32 · zarf
    let d = datagram(
        0x05,
        0x00,
        R,
        &[&[0, 0, 0, 0], &[0x01], &BOB, &[0, 0, 0, 6], ZARF],
    );
    check(
        &d,
        0x05,
        0x00,
        rid,
        RpcBody::StoreMessageReq(StoreMessageReq {
            hub_handle: 0,
            envelope_type: 0x01,
            recipient_dest_hash: BOB,
            envelope: ZARF.to_vec(),
        }),
        47,
    );
    let d = datagram(
        0x05,
        0x00,
        R,
        &[
            &[0x11, 0x22, 0x33, 0x44],
            &[0x02],
            &BOB,
            &[0, 0, 0, 6],
            ZARF,
        ],
    );
    check(
        &d,
        0x05,
        0x00,
        rid,
        RpcBody::StoreMessageReq(StoreMessageReq {
            hub_handle: 0x1122_3344,
            envelope_type: 0x02,
            recipient_dest_hash: BOB,
            envelope: ZARF.to_vec(),
        }),
        47,
    );
    // STORE_MESSAGE yanıt: status u16 · assigned_handle u32 (OK / PEER_UNBOUND / UNKNOWN_HANDLE)
    for (status, handle) in [
        ([0x00, 0x00], [0x11, 0x22, 0x33, 0x44]),
        ([0x03, 0x01], [0; 4]),
        ([0x03, 0x02], [0; 4]),
    ] {
        let d = datagram(0x05, 0x01, R, &[&status, &handle]);
        let expected = RpcBody::StoreMessageResp(StoreMessageResp {
            status: u16::from_be_bytes(status),
            assigned_handle: u32::from_be_bytes(handle),
        });
        check(&d, 0x05, 0x01, rid, expected, 6);
    }
    // STORE_MESSAGE hata
    let d = datagram(0x05, 0x03, R, &[&[0x06, 0x01], &[0, 0]]);
    check(&d, 0x05, 0x03, rid, error(0x05, ERROR_BAD_REQUEST), 4);
    // DELIVER: hub_handle · gönderen · tip · envelope_len · zarf (flags 0, request_id 0)
    let sender = [0xa1; 32];
    let d = datagram(
        0x81,
        0x00,
        [0; 4],
        &[
            &[0x11, 0x22, 0x33, 0x44],
            &sender,
            &[0x05],
            &[0, 0, 0, 6],
            ZARF,
        ],
    );
    check(
        &d,
        0x81,
        0x00,
        0,
        RpcBody::Deliver(Deliver {
            hub_handle: 0x1122_3344,
            sender_dest_hash: sender,
            envelope_type: 0x05,
            envelope: ZARF.to_vec(),
        }),
        47,
    );
    // ROUTE_MISS: hub_handle · reason 0x01
    let d = datagram(0x84, 0x00, [0; 4], &[&[0x11, 0x22, 0x33, 0x44], &[0x01]]);
    check(
        &d,
        0x84,
        0x00,
        0,
        RpcBody::RouteMiss(RouteMiss {
            hub_handle: 0x1122_3344,
            reason: 0x01,
        }),
        5,
    );
}

#[test]
fn heartbeat_and_data_rows() {
    for token in [&[][..], &[0xde, 0xad, 0xbe, 0xef][..]] {
        let mut d = vec![0, 0, 0, 0, 0x1b];
        d.extend_from_slice(token);
        d.extend_from_slice(&[0; 16]);
        assert_eq!(
            classify(&d),
            Ok(Datagram::Control {
                frame_type: 0x1b,
                body: token
            })
        );
        let mut built = Vec::new();
        hub_server::wire::control_datagram(&mut built, 0x1b, token);
        assert_eq!(built, d);
    }
    let d = [0x11, 0x22, 0x33, 0x44, 0xde];
    assert!(matches!(classify(&d), Ok(Datagram::Data { handle }) if handle.get() == 0x1122_3344));
}

/// H-2 U-1…U-4 (`docs/hub/H-2-WIRE-RS-KARSILASTIRMA-2026-11-11.md` (d)): eski `wire.rs` çözücülerinin
/// kabul ettiği sekiz negatif vektör `hub_server::wire` üzerinden çözülünce vektörün adıyla reddedilir.
#[test]
fn h2_u1_to_u4_negative_vectors_are_rejected() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vectors/hub/s4-apprpc-v1.json"
    );
    let suite: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("vektör dosyası"))
            .expect("JSON");
    let cases = suite["cases"].as_array().expect("cases");
    for name in [
        "store-resp-unknown-status",       // U-1
        "store-resp-error-code-as-status", // U-1
        "store-resp-ok-handle-0",          // U-1
        "error-payload-status-code",       // U-2
        "deliver-handle-0",                // U-3
        "deliver-unknown-envelope-type",   // U-3
        "route-miss-reason",               // U-4
        "route-miss-handle-0",             // U-4
    ] {
        let id = format!("s4-apprpc-v1.neg.{name}");
        let case = cases
            .iter()
            .find(|case| case["id"] == id.as_str())
            .unwrap_or_else(|| panic!("{id} vektörde yok"));
        let hex = case["input_hex"].as_str().expect("input_hex");
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
            .collect();
        let rejected = RpcMessage::decode(&bytes).expect_err(&id);
        assert_eq!(rejected.as_str(), case["expected"]["reason"], "{id}");
    }
}
