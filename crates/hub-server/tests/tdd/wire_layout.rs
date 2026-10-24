//! Bayt düzenleri — S4 §2.3 mesaj tablosunun satır satır karşılığı.
//!
//! Her satırın beklenen datagramı burada ALAN ALAN elle dizilir (codec'in kodlayıcısı kullanılmadan) ve
//! iki yönde sınanır. Bu, A2'nin Python'dan üreteceği altın vektörlerin (`vectors/hub/`) YERİNE
//! GEÇMEZ: aynı elden çıkar. Vektörler gelene dek tablonun kodla ayrışmasını yakalar. Alan değerleri
//! ayırt edilebilir seçilmiştir; yer değiştiren iki alan testi düşürür.

use hub_server::wire::{
    classify, code, BindProofRequest, BindProofResponse, ClientBindRequest, ClientBindResponse,
    Datagram, Deliver, ErrorPayload, Payload, RouteMiss, RpcFrame, StoreRequest, StoreResponse,
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

fn check<'a, P: Payload<'a> + PartialEq + std::fmt::Debug>(
    expected: &'a [u8],
    method_id: u8,
    flags: u8,
    request_id: u32,
    payload: P,
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
    assert_eq!(frame.payload, payload.to_vec().as_slice(), "kodlama");
    assert_eq!(P::decode(frame.payload), Ok(payload), "çözme");
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
        ClientBindRequest {
            cert_ref: CERT_REF,
            client_nonce: CN,
        },
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
        ClientBindResponse { hub_nonce: HN },
        35,
    );
    // CLIENT_BIND hata
    let d = datagram(0x01, 0x03, R, &[&[0x06, 0x01], &[0, 0]]);
    check(
        &d,
        0x01,
        0x03,
        rid,
        ErrorPayload {
            code: code::BAD_REQUEST,
        },
        4,
    );
    // BIND_PROOF istek: cert_ref · client_nonce · hub_nonce · imza
    let d = datagram(0x0b, 0x00, R, &[&CERT_REF, &CN, &HN, &SIG]);
    check(
        &d,
        0x0b,
        0x00,
        rid,
        BindProofRequest {
            cert_ref: CERT_REF,
            client_nonce: CN,
            hub_nonce: HN,
            signature: SIG,
        },
        128,
    );
    // BIND_PROOF yanıt: bind_ok 1 · 0 · 0 · OK · hub imzası
    let d = datagram(0x0b, 0x01, R, &[&[0x01], &[0; 8], &[0; 8], &[0, 0], &SIG]);
    check(
        &d,
        0x0b,
        0x01,
        rid,
        BindProofResponse { hub_signature: SIG },
        83,
    );
    // BIND_PROOF hataları
    for (bytes, value) in [
        ([0x01, 0x06], code::BAD_COOKIE),
        ([0x01, 0x05], code::POSSESSION_PROOF_FAIL),
        ([0x05, 0x02], code::RATE_LIMITED),
        ([0x05, 0x01], code::HUB_CAPACITY),
    ] {
        let d = datagram(0x0b, 0x03, R, &[&bytes, &[0, 0]]);
        check(&d, 0x0b, 0x03, rid, ErrorPayload { code: value }, 4);
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
        StoreRequest {
            hub_handle: 0,
            envelope_type: 0x01,
            recipient: BOB,
            envelope: ZARF,
        },
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
        StoreRequest {
            hub_handle: 0x1122_3344,
            envelope_type: 0x02,
            recipient: BOB,
            envelope: ZARF,
        },
        47,
    );
    // STORE_MESSAGE yanıt: status u16 · assigned_handle u32 (OK / PEER_UNBOUND / UNKNOWN_HANDLE)
    for (status, handle) in [
        ([0x00, 0x00], [0x11, 0x22, 0x33, 0x44]),
        ([0x03, 0x01], [0; 4]),
        ([0x03, 0x02], [0; 4]),
    ] {
        let d = datagram(0x05, 0x01, R, &[&status, &handle]);
        let expected = StoreResponse {
            status: u16::from_be_bytes(status),
            assigned_handle: u32::from_be_bytes(handle),
        };
        check(&d, 0x05, 0x01, rid, expected, 6);
    }
    // STORE_MESSAGE hata
    let d = datagram(0x05, 0x03, R, &[&[0x06, 0x01], &[0, 0]]);
    check(
        &d,
        0x05,
        0x03,
        rid,
        ErrorPayload {
            code: code::BAD_REQUEST,
        },
        4,
    );
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
        Deliver {
            hub_handle: 0x1122_3344,
            sender,
            envelope_type: 0x05,
            envelope: ZARF,
        },
        47,
    );
    // ROUTE_MISS: hub_handle · reason 0x01
    let d = datagram(0x84, 0x00, [0; 4], &[&[0x11, 0x22, 0x33, 0x44], &[0x01]]);
    check(
        &d,
        0x84,
        0x00,
        0,
        RouteMiss {
            hub_handle: 0x1122_3344,
            reason: 0x01,
        },
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
