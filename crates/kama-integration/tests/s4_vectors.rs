//! S2-A2.2 tüketici testi — `kama_contracts::s4` ↔ `vectors/hub/` (S4 v1.3.1 §2/§7).
//!
//! Beklenen baytların tamamı `vectors/hub/`'dan gelir; o dosyalar Rust'tan bağımsız üretilir
//! (`ref/hub/gen_vectors.py`). Bu dosya hiçbir beklenen bayt yazmaz.
//!
//! Pozitif vakalar: girdi alanlarından kurulan tip kodlanır → `expected_hex` ile bayt-özdeş; aynı
//! baytlar çözülür → kurulan tiple eşit (decode→encode bayt-özdeş). Negatif vakalar: çözme (ya da
//! STORE doğrulaması) reddeder ve hatanın kararlı adı (`S4Error::as_str`) `expected.reason` ile
//! aynıdır. Her girdi alanı tüketilmek zorundadır (kullanılmayan alan = sessiz kayma → test kırmızı).
//!
//! Kripto/özet: s4.rs kripto içermez. İmza vakaları `kama_noise_wire::ed25519_strict::verify_strict`
//! ile (katı kip) doğrulanır; TBS s4.rs'ten. BLAKE2s gerektiren iki tür (`cookie`,
//! `handle-candidate`) burada yalnız yapısal tüketilir (cookie girdisi s4.rs'ten kurulur); özet
//! değerleri H-2'de `hub-server`'ın kendi özetine karşı sınanır. `handle-alloc` vakaları S4 §2.6
//! kuralıyla, aynı dosyanın `handle-candidate` tablosu üzerinden simüle edilir.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use kama_contracts::s4::{
    ack_tbs, bind_tbs, cookie_input, domain_sep, hub_frame_decode, hub_frame_encode, AllocationKey,
    AppRpcFrame, BindProofReq, BindProofResp, ClientBindReq, ClientBindResp, CookieSrc, Deliver,
    ErrorPayload, HubFrame, RelayOpenEnvelope, RouteMiss, RpcBody, RpcMessage, StoreMessageReq,
    StoreMessageResp, CONTROL_MIN_LEN, FLAG_ERROR, FLAG_RESPONSE, HUB_TRAILER_LEN, MAX_DATAGRAM,
    METHOD_HUB_INITIATED, STATUS_CHALLENGE, STATUS_OK,
};
use kama_noise_wire::ed25519_strict::verify_strict;
use kama_noise_wire::handshake::cert_ref_of;
use kama_vectors::{Case, Expected, Input, VectorFile};

const SUITES: [&str; 4] = [
    "s4-hub-frame-v1",
    "s4-apprpc-v1",
    "s4-bind-v1",
    "s4-alloc-v1",
];

const KINDS: &[&str] = &[
    "hub-frame",
    "apprpc-frame",
    "client-bind-req",
    "client-bind-resp",
    "bind-proof-req",
    "bind-proof-resp",
    "store-message-req",
    "store-message-resp",
    "error",
    "deliver",
    "route-miss",
    "relay-open-envelope",
    "domain-sep",
    "bind-tbs",
    "bind-sig",
    "ack-tbs",
    "ack-sig",
    "cookie-input",
    "cookie",
    "allocation-key",
    "handle-candidate",
    "handle-alloc",
    "decode-hub-frame",
    "decode-apprpc",
    "store-validate",
];

/// BLAKE2s gerektiren, burada yalnız yapısal tüketilen türler (H-2'de hub-server'a karşı).
const HASH_DELEGATED: &[&str] = &["cookie", "handle-candidate"];

const SEAM_FLAG: &str = "seam-open-hub";

fn vector_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors/hub")
}

// ── Girdi alanları: her anahtar tüketilmeli ─────────────────────────────────────────────────────

struct Fields<'a> {
    id: &'a str,
    case: &'a Case,
    keys: Vec<String>,
    used: RefCell<BTreeSet<String>>,
}

impl<'a> Fields<'a> {
    fn new(case: &'a Case) -> Self {
        let Input::Structured(map) = &case.input else {
            panic!("{}: yapısal girdi bekleniyordu", case.id)
        };
        Self {
            id: &case.id,
            case,
            keys: map.keys().cloned().collect(),
            used: RefCell::new(BTreeSet::new()),
        }
    }

    fn raw(&self, name: &str) -> Option<&'a str> {
        let Input::Structured(map) = &self.case.input else {
            return None;
        };
        map.get(name).and_then(|v| v.as_str())
    }

    fn bytes(&self, name: &str) -> Vec<u8> {
        self.used.borrow_mut().insert(name.to_owned());
        let v = self
            .raw(name)
            .unwrap_or_else(|| panic!("{}: `{name}` yok", self.id));
        kama_vectors::hex(v).unwrap_or_else(|e| panic!("{}: `{name}` hex değil: {e:?}", self.id))
    }

    fn arr<const N: usize>(&self, name: &str) -> [u8; N] {
        let b = self.bytes(name);
        b.as_slice()
            .try_into()
            .unwrap_or_else(|_| panic!("{}: `{name}` {N} B değil ({} B)", self.id, b.len()))
    }

    fn u8(&self, name: &str) -> u8 {
        self.arr::<1>(name)[0]
    }

    fn u16(&self, name: &str) -> u16 {
        u16::from_be_bytes(self.arr(name))
    }

    fn u32(&self, name: &str) -> u32 {
        u32::from_be_bytes(self.arr(name))
    }

    fn u64(&self, name: &str) -> u64 {
        u64::from_be_bytes(self.arr(name))
    }

    /// PoC'de sabit bir alanın vektördeki değeri codec'in yazdığı değerle aynı mı.
    fn constant(&self, name: &str, want: &[u8]) {
        assert_eq!(
            self.bytes(name),
            want,
            "{}: sabit alan `{name}` S4 değeriyle aynı değil",
            self.id
        );
    }

    fn rid_or_zero(&self) -> u32 {
        if self.keys.iter().any(|k| k == "request_id_hex") {
            self.u32("request_id_hex")
        } else {
            0
        }
    }

    fn finish(&self) {
        let used = self.used.borrow();
        let unused: Vec<_> = self.keys.iter().filter(|k| !used.contains(*k)).collect();
        assert!(
            unused.is_empty(),
            "{}: tüketilmeyen girdi alanları {unused:?}",
            self.id
        );
    }
}

fn expected_wire(case: &Case) -> &[u8] {
    match &case.expected {
        Expected::Wire(w) => w,
        Expected::Rejected { .. } => panic!("{}: pozitif vaka bekleniyordu", case.id),
    }
}

fn expected_reason(case: &Case) -> &str {
    match &case.expected {
        Expected::Rejected { reason, .. } => reason,
        Expected::Wire(_) => panic!("{}: negatif vaka bekleniyordu", case.id),
    }
}

fn input_wire(case: &Case) -> &[u8] {
    match &case.input {
        Input::Wire(w) => w,
        Input::Structured(_) => panic!("{}: input_hex bekleniyordu", case.id),
    }
}

/// S3 HubChannel `open_hub`'ın PoC no-op'u (S4 §2.1): datagram ≤ MAX_DATAGRAM, ≥ 21 B, son 16
/// bayt sıfır. Bu katman seam'indir; s4.rs kuyruğu eklemez/çıkarmaz.
fn open_hub(d: &[u8]) -> Result<&[u8], &'static str> {
    if d.len() > MAX_DATAGRAM {
        return Err("too-long");
    }
    if d.len() < CONTROL_MIN_LEN {
        return Err("truncated");
    }
    let (frame, trailer) = d.split_at(d.len() - HUB_TRAILER_LEN);
    if trailer.iter().any(|&b| b != 0) {
        return Err("trailer-nonzero");
    }
    Ok(frame)
}

fn check_rpc(case: &Case, msg: &RpcMessage) {
    let want = expected_wire(case);
    let got = msg
        .encode()
        .unwrap_or_else(|e| panic!("{}: encode {e}", case.id));
    assert_eq!(got, want, "{}: encode baytları farklı", case.id);
    let back = RpcMessage::decode(want).unwrap_or_else(|e| panic!("{}: decode {e}", case.id));
    assert_eq!(&back, msg, "{}: decode tipi farklı", case.id);
    assert_eq!(
        back.encode().expect("re-encode"),
        want,
        "{}: decode→encode",
        case.id
    );
}

// ── Vaka koşucusu ───────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Tally {
    ran: usize,
    delegated: usize,
    by_kind: BTreeMap<String, usize>,
}

#[allow(clippy::too_many_lines)]
fn run_case(case: &Case, candidates: &BTreeMap<u64, u32>, tally: &mut Tally) {
    *tally.by_kind.entry(case.kind.clone()).or_default() += 1;
    if HASH_DELEGATED.contains(&case.kind.as_str()) {
        let f = Fields::new(case);
        if case.kind == "cookie" {
            // Yapısal: cookie girdisi s4.rs'ten kurulabilmeli; özet H-2'de.
            let _ = cookie_input_from(&f);
            let _ = f.bytes("cookie_secret_hex");
            assert_eq!(expected_wire(case).len(), 16, "{}: hub_nonce 16 B", case.id);
        } else {
            let _ = f.bytes("handle_secret_hex");
            let _ = f.u64("alloc_counter_hex");
            assert_eq!(expected_wire(case).len(), 4, "{}: aday 4 B", case.id);
        }
        f.finish();
        tally.delegated += 1;
        return;
    }
    match case.kind.as_str() {
        "decode-hub-frame" => {
            let reason = expected_reason(case);
            let seam = case.flags.iter().any(|f| f == SEAM_FLAG);
            match open_hub(input_wire(case)) {
                Err(r) => {
                    assert!(
                        seam,
                        "{}: seam reddi ama `{SEAM_FLAG}` bayrağı yok",
                        case.id
                    );
                    assert_eq!(r, reason, "{}", case.id);
                }
                Ok(opened) => {
                    assert!(
                        !seam,
                        "{}: `{SEAM_FLAG}` bayrağı var ama seam geçti",
                        case.id
                    );
                    let e = HubFrame::decode(opened).expect_err(&case.id);
                    assert_eq!(e.as_str(), reason, "{}", case.id);
                }
            }
        }
        "decode-apprpc" => {
            let e = RpcMessage::decode(input_wire(case)).expect_err(&case.id);
            assert_eq!(e.as_str(), expected_reason(case), "{}", case.id);
        }
        "store-validate" => {
            let msg = RpcMessage::decode(input_wire(case))
                .unwrap_or_else(|e| panic!("{}: yapısal çözülmeliydi: {e}", case.id));
            let RpcBody::StoreMessageReq(req) = msg.body else {
                panic!("{}: STORE_MESSAGE isteği bekleniyordu", case.id)
            };
            let e = req.validate().expect_err(&case.id);
            assert_eq!(e.as_str(), expected_reason(case), "{}", case.id);
        }
        kind => run_positive(kind, case, candidates),
    }
    tally.ran += 1;
}

fn cookie_input_from(f: &Fields<'_>) -> Vec<u8> {
    let ip = f.bytes("src_ip_hex");
    let port = f.u16("src_port_hex");
    let src = match ip.len() {
        4 => CookieSrc::v4(ip.as_slice().try_into().expect("4 B"), port),
        16 => CookieSrc::v6(ip.as_slice().try_into().expect("16 B"), port),
        n => panic!("{}: src_ip {n} B", f.id),
    };
    cookie_input(
        &src,
        &f.arr("device_cert_ref_hex"),
        &f.arr("client_nonce_hex"),
        f.u32("minute_hex"),
        f.u32("bind_epoch_hex"),
    )
}

#[allow(clippy::too_many_lines)]
fn run_positive(kind: &str, case: &Case, candidates: &BTreeMap<u64, u32>) {
    let f = Fields::new(case);
    let want = expected_wire(case);
    match kind {
        "hub-frame" => {
            let ft = f.u8("frame_type_hex");
            let body = f.bytes("body_hex");
            let mut datagram = hub_frame_encode(ft, &body).expect("hub_frame_encode");
            datagram.extend_from_slice(&[0; HUB_TRAILER_LEN]);
            assert_eq!(datagram, want, "{}: datagram", case.id);
            let opened = open_hub(want).expect("open_hub");
            assert_eq!(
                hub_frame_decode(opened),
                Ok((ft, body.as_slice())),
                "{}",
                case.id
            );
            let typed = HubFrame::decode(opened).expect("HubFrame::decode");
            assert_eq!(typed.encode().expect("re-encode"), opened, "{}", case.id);
            if let HubFrame::Plane1Rpc(rpc) = typed {
                RpcMessage::decode(rpc)
                    .unwrap_or_else(|e| panic!("{}: gövde RPC değil: {e}", case.id));
            }
        }
        "apprpc-frame" => {
            let frame = AppRpcFrame {
                method_id: f.u8("method_id_hex"),
                flags: f.u8("flags_hex"),
                request_id: f.u32("request_id_hex"),
                payload: f.bytes("payload_hex"),
            };
            assert_eq!(frame.encode().expect("encode"), want, "{}", case.id);
            assert_eq!(AppRpcFrame::decode(want), Ok(frame), "{}", case.id);
        }
        "client-bind-req" => {
            f.constant("device_cert_mode_hex", &[0]);
            f.constant("presence_flags_hex", &[0]);
            let body = RpcBody::ClientBindReq(ClientBindReq {
                device_cert_ref: f.arr("device_cert_ref_hex"),
                client_nonce: f.arr("client_nonce_hex"),
            });
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.u32("request_id_hex"),
                    body,
                },
            );
        }
        "client-bind-resp" => {
            f.constant("bind_ok_hex", &[0]);
            f.constant("mailbox_cursor_hex", &0u64.to_be_bytes());
            f.constant("revocation_seq_hex", &0u64.to_be_bytes());
            f.constant("error_code_hex", &STATUS_CHALLENGE.to_be_bytes());
            let body = RpcBody::ClientBindResp(ClientBindResp {
                hub_nonce: f.arr("hub_nonce_hex"),
            });
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.u32("request_id_hex"),
                    body,
                },
            );
        }
        "bind-proof-req" => {
            let body = RpcBody::BindProofReq(BindProofReq {
                device_cert_ref: f.arr("device_cert_ref_hex"),
                client_nonce: f.arr("client_nonce_hex"),
                hub_nonce: f.arr("hub_nonce_hex"),
                client_signature: f.arr("client_signature_hex"),
            });
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.u32("request_id_hex"),
                    body,
                },
            );
        }
        "bind-proof-resp" => {
            f.constant("bind_ok_hex", &[1]);
            f.constant("mailbox_cursor_hex", &0u64.to_be_bytes());
            f.constant("revocation_seq_hex", &0u64.to_be_bytes());
            f.constant("error_code_hex", &STATUS_OK.to_be_bytes());
            let body = RpcBody::BindProofResp(BindProofResp {
                hub_signature: f.arr("hub_signature_hex"),
            });
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.u32("request_id_hex"),
                    body,
                },
            );
        }
        "store-message-req" => {
            let req = StoreMessageReq {
                hub_handle: f.u32("hub_handle_hex"),
                envelope_type: f.u8("envelope_type_hex"),
                recipient_dest_hash: f.arr("recipient_dest_hash_hex"),
                envelope: f.bytes("envelope_hex"),
            };
            assert_eq!(
                req.validate(),
                Ok(()),
                "{}: geçerli birleşim olmalı",
                case.id
            );
            let body = RpcBody::StoreMessageReq(req);
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.u32("request_id_hex"),
                    body,
                },
            );
        }
        "store-message-resp" => {
            let body = RpcBody::StoreMessageResp(StoreMessageResp {
                status: f.u16("status_hex"),
                assigned_handle: f.u32("assigned_handle_hex"),
            });
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.u32("request_id_hex"),
                    body,
                },
            );
        }
        "error" => {
            let method_id = f.u8("method_id_hex");
            let body = RpcBody::Error {
                method_id,
                payload: ErrorPayload {
                    error_code: f.u16("error_code_hex"),
                    detail: f.bytes("detail_hex"),
                },
            };
            let msg = RpcMessage {
                request_id: f.u32("request_id_hex"),
                body,
            };
            assert_eq!(
                msg.to_frame().expect("to_frame").flags,
                FLAG_RESPONSE | FLAG_ERROR,
                "{}",
                case.id
            );
            check_rpc(case, &msg);
        }
        "deliver" => {
            let body = RpcBody::Deliver(Deliver {
                hub_handle: f.u32("hub_handle_hex"),
                sender_dest_hash: f.arr("sender_dest_hash_hex"),
                envelope_type: f.u8("envelope_type_hex"),
                envelope: f.bytes("envelope_hex"),
            });
            let msg = RpcMessage {
                request_id: f.rid_or_zero(),
                body,
            };
            assert_eq!(msg.request_id, 0, "{}: hub başlatımlı", case.id);
            assert_ne!(
                msg.to_frame().expect("frame").method_id & METHOD_HUB_INITIATED,
                0
            );
            check_rpc(case, &msg);
        }
        "route-miss" => {
            let body = RpcBody::RouteMiss(RouteMiss {
                hub_handle: f.u32("hub_handle_hex"),
                reason: f.u8("reason_hex"),
            });
            check_rpc(
                case,
                &RpcMessage {
                    request_id: f.rid_or_zero(),
                    body,
                },
            );
        }
        "relay-open-envelope" => {
            let env = RelayOpenEnvelope {
                open_nonce: f.arr("open_nonce_hex"),
            };
            assert_eq!(env.encode().as_slice(), want, "{}", case.id);
            assert_eq!(RelayOpenEnvelope::decode(want), Ok(env), "{}", case.id);
        }
        "domain-sep" => {
            let got = domain_sep(&f.bytes("label_hex")).expect("domain_sep");
            assert_eq!(got.as_slice(), want, "{}", case.id);
        }
        "bind-tbs" | "bind-sig" => {
            let tbs = bind_tbs(
                &f.arr("hub_ed25519_pub_hex"),
                &f.arr("device_cert_ref_hex"),
                &f.arr("client_nonce_hex"),
                &f.arr("hub_nonce_hex"),
            );
            if kind == "bind-tbs" {
                assert_eq!(tbs.as_slice(), want, "{}", case.id);
            } else {
                let _seed = f.bytes("device_ed25519_seed_hex"); // imzalama Python'da; burada doğrulama
                let device_pub = f.bytes("device_ed25519_pub_hex");
                let cert = f.bytes("device_cert_hex");
                assert_eq!(
                    cert.get(..32),
                    Some(device_pub.as_slice()),
                    "{}: S4 H2 cert[0..32]",
                    case.id
                );
                assert_eq!(
                    cert_ref_of(&cert),
                    f.arr("device_cert_ref_hex"),
                    "{}: cert_ref",
                    case.id
                );
                verify_strict(&device_pub, &tbs, want).unwrap_or_else(|e| {
                    panic!("{}: client_signature doğrulanmadı: {e:?}", case.id)
                });
            }
        }
        "ack-tbs" | "ack-sig" => {
            let tbs = ack_tbs(
                &f.arr("device_cert_ref_hex"),
                &f.arr("client_nonce_hex"),
                &f.arr("hub_nonce_hex"),
                f.u8("bind_ok_hex"),
                f.u16("error_code_hex"),
            );
            if kind == "ack-tbs" {
                assert_eq!(tbs.as_slice(), want, "{}", case.id);
            } else {
                let _seed = f.bytes("hub_ed25519_seed_hex");
                verify_strict(&f.bytes("hub_ed25519_pub_hex"), &tbs, want)
                    .unwrap_or_else(|e| panic!("{}: hub_signature doğrulanmadı: {e:?}", case.id));
            }
        }
        "cookie-input" => {
            assert_eq!(cookie_input_from(&f), want, "{}", case.id);
        }
        "allocation-key" => {
            let _envelope = f.bytes("envelope_hex"); // envelope_hash8'in kaynağı; özet çağıranın (H-2)
            let key = AllocationKey {
                sender_dest_hash: f.arr("sender_dest_hash_hex"),
                request_id: f.u32("request_id_hex"),
                recipient_dest_hash: f.arr("recipient_dest_hash_hex"),
                envelope_type: f.u8("envelope_type_hex"),
                envelope_hash8: f.arr("envelope_hash8_hex"),
            };
            assert_eq!(key.encode().as_slice(), want, "{}", case.id);
        }
        "handle-alloc" => {
            // S4 §2.6: her deneme sayacı ilerletir; 0 / aktif / tombstone atlanır; 8 denemede yok.
            let _secret = f.bytes("handle_secret_hex");
            let mut counter = f.u64("alloc_counter_hex");
            let taken: BTreeSet<u32> = f
                .bytes("taken_handles_hex")
                .chunks_exact(4)
                .map(|c| u32::from_be_bytes(c.try_into().expect("4 B")))
                .collect();
            let mut handle = 0u32;
            for _ in 0..8 {
                let h = *candidates.get(&counter).unwrap_or_else(|| {
                    panic!("{}: sayaç {counter:#x} aday tablosunda yok", case.id)
                });
                counter += 1;
                if h != 0 && !taken.contains(&h) {
                    handle = h;
                    break;
                }
            }
            let mut got = handle.to_be_bytes().to_vec();
            got.extend_from_slice(&counter.to_be_bytes());
            assert_eq!(got, want, "{}", case.id);
        }
        other => panic!("{}: bilinmeyen tür `{other}`", case.id),
    }
    f.finish();
}

fn candidate_table(file: &VectorFile) -> BTreeMap<u64, u32> {
    file.cases
        .iter()
        .filter(|c| c.kind == "handle-candidate")
        .map(|c| {
            let f = Fields::new(c);
            let want: [u8; 4] = expected_wire(c).try_into().expect("aday 4 B");
            (f.u64("alloc_counter_hex"), u32::from_be_bytes(want))
        })
        .collect()
}

#[test]
fn s4_hub_vectors() {
    let files = kama_vectors::load_dir(vector_dir()).expect("vectors/hub yüklenmeli");
    let names: BTreeSet<_> = files.iter().map(|f| f.suite.as_str()).collect();
    assert_eq!(names, SUITES.into_iter().collect(), "beklenen süitler");

    let mut tally = Tally::default();
    let mut total = 0;
    for file in &files {
        file.require_kinds(KINDS).expect("bilinen türler");
        total += file.case_count();
        let candidates = candidate_table(file);
        for case in &file.cases {
            assert!(
                case.spec_ref
                    .as_deref()
                    .is_some_and(|r| r.starts_with("KM-SEAM-TH-001.md §")),
                "{}: S4 §-referansı yok",
                case.id
            );
            run_case(case, &candidates, &mut tally);
        }
    }
    assert_eq!(
        tally.ran + tally.delegated,
        total,
        "her vaka koşmalı (vakum kapısı)"
    );
    assert!(tally.ran > 0 && tally.delegated < total);
    eprintln!(
        "s4_hub_vectors: {} vaka koştu, {} vaka özet-bağımlı (H-2'de hub-server'a karşı); türler {:?}",
        tally.ran, tally.delegated, tally.by_kind
    );
}
