#!/usr/bin/env python3
"""S2-A2.3 — S4 (KM-SEAM-TH-001 v1.3.1) hub tel vektörleri; BAĞIMSIZ Python referansından üretilir.

    python3 ref/hub/gen_vectors.py            # vectors/hub/ altına yazar
    python3 ref/hub/gen_vectors.py --check    # yeniden üretir ve DİSKTEKİYLE karşılaştırır

KURAL: buradaki hiçbir beklenen bayt Rust'tan üretilmez ve hiçbir Rust tip adından türetilmez.
Tek kaynak `docs/seams/KM-SEAM-TH-001.md` §2 (tel), §6 (sabitler) ve §7 (vaka listesi). Ed25519
`ref/crypto/ed25519.py` (RFC 8032, katı kip), BLAKE2s-256 ve SHA-256 `hashlib`. Bağımlılık: stdlib.

Girdiler belirlenimlidir: her anahtar/nonce `SHA-256("kama-hub-vec:<ad>")`'dan türer. Rastgelelik
yoktur; çıktı bayt bayt tekrarlanabilir.

Vaka `kind` sözlüğü (tüketici bunun üzerinden dağıtır; `crates/kama-integration/tests/s4_vectors.rs`):
  hub-frame          tam kontrol datagramı `[0;4] ‖ frame_type ‖ gövde ‖ [0;16]` (§2.1, §2.2)
  apprpc-frame       ham AppRpcFrame (§2.3 başlığı; method/flags/request_id/payload)
  client-bind-req · client-bind-resp · bind-proof-req · bind-proof-resp · store-message-req ·
  store-message-resp · error · deliver · route-miss
                     §2.3 tablosunun satırları: girdi = alanlar, beklenen = tam AppRpcFrame baytları
  relay-open-envelope  §2.4(f) 16 B zarf
  domain-sep · bind-tbs · bind-sig · ack-tbs · ack-sig     §2.4(b)
  cookie-input       cookie özetinin `cookie_secret` SONRASI girdisi (§2.4(a); özet yok)
  cookie             tam `hub_nonce` = BLAKE2s-256(cookie_secret ‖ cookie-input)[0..16]
  allocation-key     §2.4(c) tahsis anahtarı baytları (77 B; özeti çağıran hesaplar)
  handle-candidate   §2.6 tek aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4]
  handle-alloc       §2.6 tahsis: beklenen = handle u32 ‖ alloc_counter_sonra u64 (handle 0 = tahsis yok)
  decode-hub-frame · decode-apprpc · store-validate        negatifler (`expected.reason` aşağıda)

Negatif `reason` sözlüğü (tüketici Rust hatasının kararlı adını bununla tam-dizge karşılaştırır):
  truncated · trailing-bytes · length-mismatch · too-long · bad-length · not-control ·
  trailer-nonzero · unknown-frame-type · unknown-method · bad-flags · bad-request-id · bad-field ·
  bad-code · zero-handle · unknown-envelope-type · bad-request
"""

import hashlib
import ipaddress
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "ref", "crypto"))

import ed25519  # noqa: E402

SPEC = "km-seam-th-001@v1.3.1"
GEN = "ref/hub/gen_vectors.py 1"
S4 = "KM-SEAM-TH-001.md "


def sha256(b: bytes) -> bytes:
    return hashlib.sha256(b).digest()


def blake2s(b: bytes) -> bytes:
    return hashlib.blake2s(b, digest_size=32).digest()


def seed(name: str) -> bytes:
    return sha256(b"kama-hub-vec:" + name.encode())


def hx(b: bytes) -> str:
    return b.hex()


def u8(v: int) -> bytes:
    return struct.pack(">B", v)


def u16(v: int) -> bytes:
    return struct.pack(">H", v)


def u32(v: int) -> bytes:
    return struct.pack(">I", v)


def u64(v: int) -> bytes:
    return struct.pack(">Q", v)


# ── S4 sabitleri (§2.1–§2.6, §6) ─────────────────────────────────────────────────────────────
MAX_DATAGRAM = 1280
CONTROL_PREFIX = bytes(4)
HUB_TRAILER = bytes(16)
RPC_HEADER = 10                                   # method u8 · flags u8 · request_id u32 · payload_len u32
MAX_RPC_PAYLOAD = MAX_DATAGRAM - 31               # §2.3 "payload_len ≤ MAX_DATAGRAM − 31"
RELAY_OPEN_MAX_ENVELOPE = 64

FRAME_HEARTBEAT, FRAME_HEARTBEAT_ACK, FRAME_PLANE1_RPC = 0x1B, 0x1C, 0x1D

CLIENT_BIND, STORE_MESSAGE, BIND_PROOF, DELIVER, ROUTE_MISS = 0x01, 0x05, 0x0B, 0x81, 0x84
F_REQUEST, F_RESPONSE, F_RESPONSE_ERROR = 0x00, 0x01, 0x03

ENV_INIT, ENV_RESPONSE, ENV_RELAY_OPEN = 0x01, 0x02, 0x05

OK = 0x0000
CHALLENGE = 0x0003
PEER_UNBOUND = 0x0301
UNKNOWN_HANDLE = 0x0302
POSSESSION_PROOF_FAIL = 0x0105
BAD_COOKIE = 0x0106
HUB_CAPACITY = 0x0501
RATE_LIMITED = 0x0502
BAD_REQUEST = 0x0601
ERROR_CODES = {
    "possession-proof-fail": POSSESSION_PROOF_FAIL,
    "bad-cookie": BAD_COOKIE,
    "hub-capacity": HUB_CAPACITY,
    "rate-limited": RATE_LIMITED,
    "bad-request": BAD_REQUEST,
}
ROUTE_MISS_UNKNOWN_HANDLE = 0x01
HANDLE_ALLOC_ATTEMPTS = 8


# ── Kodlayıcılar (S4 §2'nin doğrudan yazımı) ─────────────────────────────────────────────────
def domain_sep(label: bytes) -> bytes:
    """ccr §13.2 geleneği: ASCII ≤ 16 B + 0x00 dolgu."""
    assert len(label) <= 16 and label.isascii()
    return label + bytes(16 - len(label))


def hub_frame(frame_type: int, body: bytes) -> bytes:
    """§2.1: `[0x00000000][frame_type][gövde][0×16]` (trailer = PoC no-op seal_hub)."""
    d = CONTROL_PREFIX + u8(frame_type) + body + HUB_TRAILER
    assert len(d) <= MAX_DATAGRAM
    return d


def apprpc(method: int, flags: int, request_id: int, payload: bytes) -> bytes:
    """§2.3 AppRpcFrame."""
    return u8(method) + u8(flags) + u32(request_id) + u32(len(payload)) + payload


def client_bind_req(cert_ref: bytes, client_nonce: bytes) -> bytes:
    # device_cert_mode u8 = 0x00 · device_cert_ref bytes32 · presence_flags u8 (PoC: 0) · client_nonce bytes16
    return u8(0x00) + cert_ref + u8(0x00) + client_nonce


def client_bind_resp(hub_nonce: bytes) -> bytes:
    # bind_ok u8 = 0 · mailbox_cursor u64 = 0 · revocation_seq u64 = 0 · error_code u16 = CHALLENGE · hub_nonce
    return u8(0) + u64(0) + u64(0) + u16(CHALLENGE) + hub_nonce


def bind_proof_req(cert_ref: bytes, client_nonce: bytes, hub_nonce: bytes, sig: bytes) -> bytes:
    return cert_ref + client_nonce + hub_nonce + sig


def bind_proof_resp(hub_sig: bytes) -> bytes:
    # bind_ok u8 = 1 · mailbox_cursor u64 = 0 · revocation_seq u64 = 0 · error_code u16 = 0 · hub_signature
    return u8(1) + u64(0) + u64(0) + u16(OK) + hub_sig


def store_req(hub_handle: int, envelope_type: int, recipient: bytes, envelope: bytes) -> bytes:
    return u32(hub_handle) + u8(envelope_type) + recipient + u32(len(envelope)) + envelope


def store_resp(status: int, assigned_handle: int) -> bytes:
    return u16(status) + u32(assigned_handle)


def error_payload(code: int) -> bytes:
    # ErrorPayload { error_code u16; detail_len u16 = 0 } (detail boş)
    return u16(code) + u16(0)


def deliver(hub_handle: int, sender: bytes, envelope_type: int, envelope: bytes) -> bytes:
    return u32(hub_handle) + sender + u8(envelope_type) + u32(len(envelope)) + envelope


def route_miss(hub_handle: int, reason: int) -> bytes:
    return u32(hub_handle) + u8(reason)


def bind_tbs(hub_pub: bytes, cert_ref: bytes, client_nonce: bytes, hub_nonce: bytes) -> bytes:
    t = domain_sep(b"KAMA-HUB-BIND-V1") + hub_pub + cert_ref + client_nonce + hub_nonce
    assert len(t) == 112
    return t


def ack_tbs(cert_ref: bytes, client_nonce: bytes, hub_nonce: bytes, bind_ok: int, error_code: int) -> bytes:
    t = domain_sep(b"KAMA-HUB-ACK-V1") + cert_ref + client_nonce + hub_nonce + u8(bind_ok) + u16(error_code)
    assert len(t) == 83
    return t


def normalize_ip(ip: bytes) -> tuple[int, bytes]:
    """§2.4(a): IPv4 → fam 0x04 + 4 B; IPv6 → 0x06 + 16 B; `::ffff:a.b.c.d` → IPv4'e normalize."""
    if len(ip) == 4:
        return 0x04, ip
    assert len(ip) == 16
    mapped = ipaddress.IPv6Address(ip).ipv4_mapped
    if mapped is not None:
        return 0x04, mapped.packed
    return 0x06, ip


def cookie_input(ip: bytes, port: int, cert_ref: bytes, client_nonce: bytes, minute: int, epoch: int) -> bytes:
    fam, nip = normalize_ip(ip)
    return u8(fam) + nip + u16(port) + cert_ref + client_nonce + u32(minute) + u32(epoch)


def cookie(secret: bytes, ip, port, cert_ref, client_nonce, minute, epoch) -> bytes:
    return blake2s(secret + cookie_input(ip, port, cert_ref, client_nonce, minute, epoch))[:16]


def allocation_key(sender: bytes, request_id: int, recipient: bytes, envelope_type: int, envelope: bytes) -> bytes:
    k = sender + u32(request_id) + recipient + u8(envelope_type) + blake2s(envelope)[:8]
    assert len(k) == 77
    return k


def handle_candidate(secret: bytes, counter: int) -> int:
    return struct.unpack(">I", blake2s(secret + u64(counter))[:4])[0]


def handle_alloc(secret: bytes, counter: int, taken: set[int]) -> tuple[int, int]:
    """§2.6: her deneme alloc_counter'ı ilerletir; 0 / aktif / tombstone atlanır; 8 denemede çakışma → yok."""
    for _ in range(HANDLE_ALLOC_ATTEMPTS):
        h = handle_candidate(secret, counter)
        counter += 1
        if h != 0 and h not in taken:
            return h, counter
    return 0, counter


# ── Sabit dünya ──────────────────────────────────────────────────────────────────────────────
class Device:
    def __init__(self, name: str):
        self.seed = seed(f"{name}-device-ed25519")
        self.pub = ed25519.public_key(self.seed)
        # cert: §3 H2 "device_ed25519_pub = cert[0..32]"; kalan 120 B desenli dolgu (bu süitte opak).
        self.cert = self.pub + bytes((i * 13 + len(name)) & 0xFF for i in range(120))
        assert len(self.cert) == 152
        self.cert_ref = sha256(self.cert)
        identity = ed25519.public_key(seed(f"{name}-user-identity-ed25519"))
        self.dest_hash = sha256(sha256(identity))


ALICE, BOB, CAROL = Device("alice"), Device("bob"), Device("carol")
HUB_SEED = seed("hub-ed25519")
HUB_PUB = ed25519.public_key(HUB_SEED)
OTHER_HUB_PUB = ed25519.public_key(seed("other-hub-ed25519"))

COOKIE_SECRET = seed("cookie-secret")      # 32 B (S4 boy belirtmez; KARAR-KAYDI)
HANDLE_SECRET = seed("handle-secret")      # 32 B (S4 boy belirtmez; KARAR-KAYDI)

IPV4 = ipaddress.IPv4Address("198.51.100.7").packed             # RFC 5737 TEST-NET-2
IPV6 = ipaddress.IPv6Address("2001:db8::7").packed               # RFC 3849
IPV4_MAPPED = ipaddress.IPv6Address("::ffff:198.51.100.7").packed
PORT = 40000
MINUTE = 1234

CLIENT_NONCE = seed("alice-client-nonce-1")[:16]
HUB_NONCE = cookie(COOKIE_SECRET, IPV4, PORT, ALICE.cert_ref, CLIENT_NONCE, MINUTE, 0)
TBS_BIND = bind_tbs(HUB_PUB, ALICE.cert_ref, CLIENT_NONCE, HUB_NONCE)
CLIENT_SIG = ed25519.sign(ALICE.seed, TBS_BIND)
TBS_ACK = ack_tbs(ALICE.cert_ref, CLIENT_NONCE, HUB_NONCE, 1, OK)
HUB_SIG = ed25519.sign(HUB_SEED, TBS_ACK)
assert ed25519.verify(ALICE.pub, TBS_BIND, CLIENT_SIG)
assert ed25519.verify(HUB_PUB, TBS_ACK, HUB_SIG)

RID_BIND1, RID_BIND2, RID_INIT, RID_RESP, RID_OPEN = 0x5A17C0DE, 0x5A17C0DF, 0x5A17C0E0, 0x2B0B0001, 0x5A17C0E1
ALLOC_COUNTER0 = 0x0000000000000100
H_INIT, _C1 = handle_alloc(HANDLE_SECRET, ALLOC_COUNTER0, set())
H_OPEN, _C2 = handle_alloc(HANDLE_SECRET, _C1, {H_INIT})
assert H_INIT != 0 and H_OPEN != 0 and H_INIT != H_OPEN

# SESSION_INIT / RESPONSE zarfları bu süitte OPAKTIR (hub zarfı açmaz, §1.3); desenli dolgu.
ENV_INIT_BYTES = bytes((i * 7 + 1) & 0xFF for i in range(180))
ENV_RESP_BYTES = bytes((i * 5 + 2) & 0xFF for i in range(120))
OPEN_NONCE = seed("alice-open-nonce-1")[:16]


def suite(name, description, notes, cases):
    used = {f for c in cases for f in c.get("flags", [])}
    assert used == set(notes), (name, used ^ set(notes))
    ids = [c["id"] for c in cases]
    assert len(ids) == len(set(ids)), name
    for c in cases:
        assert c.get("spec_ref", "").startswith(S4), c["id"]   # her vaka S4 §-referanslı
    return {
        "schema_version": 2,
        "suite": name,
        "spec_version": SPEC,
        "generator": GEN,
        "description": description,
        "notes": notes,
        "case_count": len(cases),
        "cases": cases,
    }


def case(cid, kind, *, spec_ref, flags=None, note=None, input=None, input_hex=None,
         expected_hex=None, expected=None):
    c = {"id": cid, "kind": kind}
    if flags:
        c["flags"] = flags
    if note:
        c["note"] = note
    c["spec_ref"] = S4 + spec_ref
    if input is not None:
        c["input"] = input
    if input_hex is not None:
        c["input_hex"] = input_hex
    if expected_hex is not None:
        c["expected_hex"] = expected_hex
    if expected is not None:
        c["expected"] = {"result": expected[0], "reason": expected[1]}
    return c


# ══ s4-hub-frame-v1 — §2.1 / §2.2 ═════════════════════════════════════════════════════════════
def gen_hub_frame():
    s = "s4-hub-frame-v1"
    token = 0x0000BEEF
    bind1 = apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, client_bind_req(ALICE.cert_ref, CLIENT_NONCE))
    bind2 = apprpc(BIND_PROOF, F_REQUEST, RID_BIND2,
                   bind_proof_req(ALICE.cert_ref, CLIENT_NONCE, HUB_NONCE, CLIENT_SIG))
    chal = apprpc(CLIENT_BIND, F_RESPONSE, RID_BIND1, client_bind_resp(HUB_NONCE))
    err = apprpc(BIND_PROOF, F_RESPONSE_ERROR, RID_BIND2, error_payload(BAD_COOKIE))
    rmiss = apprpc(ROUTE_MISS, F_REQUEST, 0, route_miss(H_INIT, ROUTE_MISS_UNKNOWN_HANDLE))
    # §3 H10 boyutları: meydan okuma 66 ≤ 81, BIND_PROOF hatası 35 ≤ 159, ROUTE_MISS 36.
    assert len(hub_frame(FRAME_PLANE1_RPC, bind1)) == 81
    assert len(hub_frame(FRAME_PLANE1_RPC, chal)) == 66
    assert len(hub_frame(FRAME_PLANE1_RPC, bind2)) == 159
    assert len(hub_frame(FRAME_PLANE1_RPC, err)) == 35
    assert len(hub_frame(FRAME_PLANE1_RPC, rmiss)) == 36

    def pos(local, ftype, body, ref, note=None, flags=None):
        return case(f"{s}.{local}", "hub-frame", spec_ref=ref, note=note, flags=flags,
                    input={"frame_type_hex": hx(u8(ftype)), "body_hex": hx(body)},
                    expected_hex=hx(hub_frame(ftype, body)))

    def neg(local, datagram, reason, ref, note=None, flags=None):
        return case(f"{s}.{local}", "decode-hub-frame", spec_ref=ref, note=note, flags=flags,
                    input_hex=hx(datagram), expected=("drop", reason))

    hb_empty = hub_frame(FRAME_HEARTBEAT, b"")
    hb_bad = CONTROL_PREFIX + u8(FRAME_HEARTBEAT) + b"\x00\x01" + HUB_TRAILER
    cases = [
        pos("heartbeat.empty", FRAME_HEARTBEAT, b"", "§2.2 HEARTBEAT (gövde boş)",
            note="21 B: en kısa kontrol datagramı."),
        pos("heartbeat.token", FRAME_HEARTBEAT, u32(token), "§2.2 HEARTBEAT (echo_token u32)"),
        pos("heartbeat-ack.empty", FRAME_HEARTBEAT_ACK, b"", "§2.2 HEARTBEAT_ACK (gelenle aynı uzunluk)"),
        pos("heartbeat-ack.token", FRAME_HEARTBEAT_ACK, u32(token), "§2.2 HEARTBEAT_ACK (token aynen)"),
        pos("plane1-rpc.client-bind-req", FRAME_PLANE1_RPC, bind1, "§2.1 / §2.2 PLANE1_RPC; §3 H10",
            note="CLIENT_BIND isteğinin tam datagramı: 81 B."),
        pos("plane1-rpc.client-bind-resp", FRAME_PLANE1_RPC, chal, "§2.1 / §2.2 PLANE1_RPC; §3 H10",
            note="CLIENT_BIND meydan okumasının tam datagramı: 66 B (≤ 81 B istek)."),
        pos("plane1-rpc.bind-proof-req", FRAME_PLANE1_RPC, bind2, "§2.1 / §2.2 PLANE1_RPC; §3 H10",
            note="BIND_PROOF isteğinin tam datagramı: 159 B."),
        pos("plane1-rpc.bind-proof-error", FRAME_PLANE1_RPC, err, "§2.1 / §2.2 PLANE1_RPC; §3 H10",
            note="BIND_PROOF hata yanıtının tam datagramı: 35 B (≤ 159 B istek)."),
        pos("plane1-rpc.route-miss", FRAME_PLANE1_RPC, rmiss, "§2.1 / §2.2 PLANE1_RPC; §3 H10",
            note="ROUTE_MISS tam datagramı: 36 B."),
        neg("neg.heartbeat-body-len-2", hb_bad, "bad-length", "§2.2 HEARTBEAT (başka uzunluk → reject)"),
        neg("neg.heartbeat-ack-body-len-5",
            CONTROL_PREFIX + u8(FRAME_HEARTBEAT_ACK) + bytes(5) + HUB_TRAILER, "bad-length",
            "§2.2 HEARTBEAT_ACK"),
        neg("neg.unknown-frame-type", hub_frame(0x1A, b""), "unknown-frame-type",
            "§2.2 (başka frame_type → sessiz drop); §3 H1"),
        neg("neg.unknown-frame-type-1e", hub_frame(0x1E, b""), "unknown-frame-type",
            "§2.2; §8 Y1 (0x1E/0x1F rezerve, PoC'de yok)"),
        neg("neg.prefix-nonzero", b"\x00\x00\x00\x01" + hb_empty[4:], "not-control",
            "§2.1 (ilk 4 bayt 0 değilse veri sınıfı)", note="Kontrol çözücüsü ilk 4 bayt ≠ 0'ı reddeder."),
        neg("neg.trailer-nonzero", hb_empty[:-1] + b"\x01", "trailer-nonzero", "§2.1 (open_hub: son 16 bayt sıfır)",
            flags=["seam-open-hub"]),
        neg("neg.shorter-than-21", hb_empty[:-1], "truncated", "§2.1 (en az 21 B)",
            flags=["seam-open-hub"]),
        neg("neg.over-max-datagram",
            hub_frame(FRAME_PLANE1_RPC, apprpc(STORE_MESSAGE, F_REQUEST, RID_INIT,
                                               store_req(0, ENV_INIT, BOB.dest_hash, bytes(1208))))
            [:-16] + b"\x00" + HUB_TRAILER,
            "too-long", "§2 (datagram ≤ MAX_DATAGRAM); §6 MAX_DATAGRAM = 1280",
            note="1281 B datagram.", flags=["seam-open-hub"]),
    ]
    assert len(bytes.fromhex(cases[-1]["input_hex"])) == MAX_DATAGRAM + 1
    notes = {
        "seam-open-hub": "Ret S3 HubChannel open_hub (seam) katmanında: 16 sıfır kuyruğu ve datagram boyu. "
                         "s4.rs bu baytları eklemez/çıkarmaz (A2.2); tüketici §2.1 kuralını uygular.",
    }
    desc = ("S4 §2.1/§2.2 HubChannel kontrol datagramı: HEARTBEAT/HEARTBEAT_ACK (0 B ve 4 B gövde), "
            "PLANE1_RPC sarmalayıcısı ve §3 H10 boyutları; negatifler (gövde boyu, bilinmeyen frame_type, "
            "önek, kuyruk, boy sınırları).")
    return suite(s, desc, notes, cases)


# ══ s4-apprpc-v1 — §2.3 / §2.4 / §2.5 ════════════════════════════════════════════════════════
def gen_apprpc():
    s = "s4-apprpc-v1"
    ref23 = "§2.3 mesaj tablosu: "

    def row(local, kind, method, flags, rid, payload, inp, ref, note=None, fl=None):
        frame = apprpc(method, flags, rid, payload)
        assert len(frame) <= MAX_DATAGRAM - 21
        full = dict(inp)
        if rid or (method & 0x80) == 0:
            full["request_id_hex"] = hx(u32(rid))
        return case(f"{s}.{local}", kind, spec_ref=ref, note=note, flags=fl, input=full,
                    expected_hex=hx(frame))

    def neg(local, kind, frame, reason, ref, note=None, result="reject"):
        return case(f"{s}.{local}", kind, spec_ref=ref, note=note, input_hex=hx(frame),
                    expected=(result, reason))

    big_env = bytes((i * 3 + 9) & 0xFF for i in range(MAX_RPC_PAYLOAD - 41))
    assert len(big_env) == 1208
    cases = [
        # ── ham çerçeve (§2.3 başlığı) ──
        case(f"{s}.frame.request", "apprpc-frame", spec_ref="§2.3 AppRpcFrame (flags 00 istek)",
             input={"method_id_hex": "01", "flags_hex": "00", "request_id_hex": hx(u32(RID_BIND1)),
                    "payload_hex": hx(client_bind_req(ALICE.cert_ref, CLIENT_NONCE))},
             expected_hex=hx(apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1,
                                    client_bind_req(ALICE.cert_ref, CLIENT_NONCE)))),
        case(f"{s}.frame.empty-payload", "apprpc-frame", spec_ref="§2.3 AppRpcFrame (payload_len = 0)",
             note="Çerçeve düzeyinde boş yük geçerlidir; mesaj düzeyinde her method kendi boyunu ister.",
             input={"method_id_hex": "05", "flags_hex": "01", "request_id_hex": "00000001", "payload_hex": ""},
             expected_hex=hx(apprpc(STORE_MESSAGE, F_RESPONSE, 1, b""))),
        # ── §2.3 tablosunun her satırı ──
        row("client-bind.request", "client-bind-req", CLIENT_BIND, F_REQUEST, RID_BIND1,
            client_bind_req(ALICE.cert_ref, CLIENT_NONCE),
            {"device_cert_mode_hex": "00", "device_cert_ref_hex": hx(ALICE.cert_ref), "presence_flags_hex": "00",
             "client_nonce_hex": hx(CLIENT_NONCE)},
            ref23 + "CLIENT_BIND istek; §2.4(a)"),
        row("client-bind.response-challenge", "client-bind-resp", CLIENT_BIND, F_RESPONSE, RID_BIND1,
            client_bind_resp(HUB_NONCE),
            {"bind_ok_hex": "00", "mailbox_cursor_hex": hx(u64(0)), "revocation_seq_hex": hx(u64(0)),
             "error_code_hex": hx(u16(CHALLENGE)), "hub_nonce_hex": hx(HUB_NONCE)},
            ref23 + "CLIENT_BIND yanıt (meydan okuma); §2.4(a); §2.5 CHALLENGE",
            note="hub_nonce = s4-bind-v1 cookie.ipv4.epoch-0 (aynı dünya)."),
        row("client-bind.error", "error", CLIENT_BIND, F_RESPONSE_ERROR, RID_BIND1, error_payload(BAD_REQUEST),
            {"method_id_hex": "01", "error_code_hex": hx(u16(BAD_REQUEST)), "detail_hex": ""},
            ref23 + "CLIENT_BIND hata; §2.5", fl=["code-choice"],
            note="S4 CLIENT_BIND hatası için kod saymaz; satırın biçim vektörü BAD_REQUEST ile."),
        row("bind-proof.request", "bind-proof-req", BIND_PROOF, F_REQUEST, RID_BIND2,
            bind_proof_req(ALICE.cert_ref, CLIENT_NONCE, HUB_NONCE, CLIENT_SIG),
            {"device_cert_ref_hex": hx(ALICE.cert_ref), "client_nonce_hex": hx(CLIENT_NONCE),
             "hub_nonce_hex": hx(HUB_NONCE), "client_signature_hex": hx(CLIENT_SIG)},
            ref23 + "BIND_PROOF istek; §2.4(b)", note="client_signature = s4-bind-v1 bind-sig.alice."),
        row("bind-proof.response-bound", "bind-proof-resp", BIND_PROOF, F_RESPONSE, RID_BIND2,
            bind_proof_resp(HUB_SIG),
            {"bind_ok_hex": "01", "mailbox_cursor_hex": hx(u64(0)), "revocation_seq_hex": hx(u64(0)),
             "error_code_hex": hx(u16(OK)), "hub_signature_hex": hx(HUB_SIG)},
            ref23 + "BIND_PROOF yanıt (bağlandı); §2.4(b)", note="hub_signature = s4-bind-v1 ack-sig.alice."),
    ]
    for name, code in [("bad-cookie", BAD_COOKIE), ("possession-proof-fail", POSSESSION_PROOF_FAIL),
                       ("rate-limited", RATE_LIMITED), ("hub-capacity", HUB_CAPACITY)]:
        cases.append(row(f"bind-proof.error.{name}", "error", BIND_PROOF, F_RESPONSE_ERROR, RID_BIND2,
                         error_payload(code),
                         {"method_id_hex": "0b", "error_code_hex": hx(u16(code)), "detail_hex": ""},
                         ref23 + "BIND_PROOF hata; §2.4(b); §2.5",
                         note="flags = 0x03, BIND_PROOF isteğiyle aynı request_id; imzasız."))
    cases += [
        row("store.request.init-new-link", "store-message-req", STORE_MESSAGE, F_REQUEST, RID_INIT,
            store_req(0, ENV_INIT, BOB.dest_hash, ENV_INIT_BYTES),
            {"hub_handle_hex": "00000000", "envelope_type_hex": "01", "recipient_dest_hash_hex": hx(BOB.dest_hash),
             "envelope_hex": hx(ENV_INIT_BYTES)},
            ref23 + "STORE_MESSAGE istek; §2.4(c) {0, SESSION_INIT}"),
        row("store.request.response-on-link", "store-message-req", STORE_MESSAGE, F_REQUEST, RID_RESP,
            store_req(H_INIT, ENV_RESPONSE, ALICE.dest_hash, ENV_RESP_BYTES),
            {"hub_handle_hex": hx(u32(H_INIT)), "envelope_type_hex": "02",
             "recipient_dest_hash_hex": hx(ALICE.dest_hash), "envelope_hex": hx(ENV_RESP_BYTES)},
            ref23 + "STORE_MESSAGE istek; §2.4(c) {h, SESSION_INIT_RESPONSE}",
            note="h = s4-alloc-v1 handle-alloc.first-free."),
        row("store.request.relay-open-new-link", "store-message-req", STORE_MESSAGE, F_REQUEST, RID_OPEN,
            store_req(0, ENV_RELAY_OPEN, BOB.dest_hash, OPEN_NONCE),
            {"hub_handle_hex": "00000000", "envelope_type_hex": "05", "recipient_dest_hash_hex": hx(BOB.dest_hash),
             "envelope_hex": hx(OPEN_NONCE)},
            ref23 + "STORE_MESSAGE istek; §2.4(c) {0, RELAY_OPEN}; §2.4(f) 16 B open_nonce"),
        row("store.request.max-envelope", "store-message-req", STORE_MESSAGE, F_REQUEST, RID_INIT,
            store_req(0, ENV_INIT, BOB.dest_hash, big_env),
            {"hub_handle_hex": "00000000", "envelope_type_hex": "01", "recipient_dest_hash_hex": hx(BOB.dest_hash),
             "envelope_hex": hx(big_env)},
            "§2.3 payload_len ≤ MAX_DATAGRAM − 31; §6 MAX_DATAGRAM", fl=["boundary"],
            note="1208 B zarf: payload 1249 B, datagram tam 1280 B."),
        row("store.response.ok", "store-message-resp", STORE_MESSAGE, F_RESPONSE, RID_INIT,
            store_resp(OK, H_INIT), {"status_hex": hx(u16(OK)), "assigned_handle_hex": hx(u32(H_INIT))},
            ref23 + "STORE_MESSAGE yanıt; §2.4(c) {OK, h}"),
        row("store.response.peer-unbound", "store-message-resp", STORE_MESSAGE, F_RESPONSE, RID_INIT,
            store_resp(PEER_UNBOUND, 0), {"status_hex": hx(u16(PEER_UNBOUND)), "assigned_handle_hex": "00000000"},
            ref23 + "STORE_MESSAGE yanıt; §2.6 adım 1 {PEER_UNBOUND, 0}"),
        row("store.response.unknown-handle-0", "store-message-resp", STORE_MESSAGE, F_RESPONSE, RID_INIT,
            store_resp(UNKNOWN_HANDLE, 0), {"status_hex": hx(u16(UNKNOWN_HANDLE)), "assigned_handle_hex": "00000000"},
            ref23 + "STORE_MESSAGE yanıt; KR-19 emekli anahtar {UNKNOWN_HANDLE, 0}",
            note="store-resp-unknown-handle-0: yeni-hat anahtarı tombstone'da; tahsis yok, DELIVER yok."),
    ]
    for name, code in [("bad-request", BAD_REQUEST), ("rate-limited", RATE_LIMITED),
                       ("hub-capacity", HUB_CAPACITY)]:
        cases.append(row(f"store.error.{name}", "error", STORE_MESSAGE, F_RESPONSE_ERROR, RID_INIT,
                         error_payload(code),
                         {"method_id_hex": "05", "error_code_hex": hx(u16(code)), "detail_hex": ""},
                         ref23 + "STORE_MESSAGE hata; §2.4(c); §2.5",
                         note="flags = 0x03, STORE isteğiyle aynı request_id."))
    cases += [
        row("deliver.init", "deliver", DELIVER, F_REQUEST, 0, deliver(H_INIT, ALICE.dest_hash, ENV_INIT, ENV_INIT_BYTES),
            {"hub_handle_hex": hx(u32(H_INIT)), "sender_dest_hash_hex": hx(ALICE.dest_hash), "envelope_type_hex": "01",
             "envelope_hex": hx(ENV_INIT_BYTES)},
            ref23 + "DELIVER; §2.4(d) (INIT)", note="flags = 0, request_id = 0 (hub başlatır)."),
        row("deliver.response", "deliver", DELIVER, F_REQUEST, 0,
            deliver(H_INIT, BOB.dest_hash, ENV_RESPONSE, ENV_RESP_BYTES),
            {"hub_handle_hex": hx(u32(H_INIT)), "sender_dest_hash_hex": hx(BOB.dest_hash), "envelope_type_hex": "02",
             "envelope_hex": hx(ENV_RESP_BYTES)},
            ref23 + "DELIVER; §2.4(d) (RESPONSE; handle ataması §4 C4)"),
        row("deliver.relay-open", "deliver", DELIVER, F_REQUEST, 0,
            deliver(H_OPEN, ALICE.dest_hash, ENV_RELAY_OPEN, OPEN_NONCE),
            {"hub_handle_hex": hx(u32(H_OPEN)), "sender_dest_hash_hex": hx(ALICE.dest_hash), "envelope_type_hex": "05",
             "envelope_hex": hx(OPEN_NONCE)},
            ref23 + "DELIVER; §2.4(d) (RELAY_OPEN; yalnız bilgi, KR-17)"),
        row("route-miss", "route-miss", ROUTE_MISS, F_REQUEST, 0, route_miss(H_INIT, ROUTE_MISS_UNKNOWN_HANDLE),
            {"hub_handle_hex": hx(u32(H_INIT)), "reason_hex": "01"},
            ref23 + "ROUTE_MISS; §2.4(e) reason 0x01 UNKNOWN_HANDLE"),
    ]

    # ── negatifler: çözme (yapısal) ──
    b1 = client_bind_req(ALICE.cert_ref, CLIENT_NONCE)
    init_req = store_req(0, ENV_INIT, BOB.dest_hash, ENV_INIT_BYTES)
    dl = deliver(H_INIT, ALICE.dest_hash, ENV_INIT, ENV_INIT_BYTES)
    hdr_bad_len = apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, b1)
    hdr_bad_len = hdr_bad_len[:6] + u32(len(b1) + 1) + hdr_bad_len[10:]
    too_long = apprpc(STORE_MESSAGE, F_REQUEST, RID_INIT, store_req(0, ENV_INIT, BOB.dest_hash, big_env + b"\x00"))
    cases += [
        neg("neg.header-short", "decode-apprpc", apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, b"")[:9], "truncated",
            "§2.3 (başlık 10 B)"),
        neg("neg.payload-len-mismatch", "decode-apprpc", hdr_bad_len, "length-mismatch",
            "§2.3 payload_len (kalan baytla tutmuyor)"),
        neg("neg.payload-over-max", "decode-apprpc", too_long, "too-long",
            "§2.3 payload_len ≤ MAX_DATAGRAM − 31", note="payload 1250 B."),
        neg("neg.unknown-method", "decode-apprpc", apprpc(0x02, F_REQUEST, RID_BIND1, b1), "unknown-method",
            "§2.3 mesaj tablosu (yalnız 0x01/0x05/0x0B/0x81/0x84)"),
        neg("neg.flags-error-without-response", "decode-apprpc", apprpc(CLIENT_BIND, 0x02, RID_BIND1,
                                                                        error_payload(BAD_REQUEST)),
            "bad-flags", "§2.3 flags (yalnız 00 / 01 / 03)"),
        neg("neg.flags-reserved-bit", "decode-apprpc", apprpc(CLIENT_BIND, 0x04, RID_BIND1, b1), "bad-flags",
            "§2.3 flags (diğer bitler 0)"),
        neg("neg.hub-initiated-with-response-flag", "decode-apprpc", apprpc(DELIVER, F_RESPONSE, 0, dl), "bad-flags",
            "§2.3 (bit7 = hub başlatır, yanıtı yoktur; flags = 0); §8 Y2"),
        neg("neg.hub-initiated-request-id-nonzero", "decode-apprpc", apprpc(DELIVER, F_REQUEST, 7, dl),
            "bad-request-id", "§2.3 request_id (hub başlatıyorsa 0); §8 Y2"),
        neg("neg.client-bind-req-short", "decode-apprpc", apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, b1[:-1]),
            "truncated", "§2.4(a) istek (50 B)", note="kısa gövde."),
        neg("neg.client-bind-req-trailing", "decode-apprpc",
            apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, b1 + b"\x00"), "trailing-bytes", "§2.4(a) istek (50 B)"),
        neg("neg.client-bind-req-cert-mode", "decode-apprpc",
            apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, b"\x01" + b1[1:]), "bad-field",
            "§2.4(a) device_cert_mode = 0x00"),
        neg("neg.client-bind-req-presence-flags", "decode-apprpc",
            apprpc(CLIENT_BIND, F_REQUEST, RID_BIND1, b1[:33] + b"\x01" + b1[34:]), "bad-field",
            "§2.4(a) presence_flags (PoC: 0)"),
        neg("neg.client-bind-resp-not-challenge", "decode-apprpc",
            apprpc(CLIENT_BIND, F_RESPONSE, RID_BIND1, client_bind_resp(HUB_NONCE)[:17] + u16(OK) + HUB_NONCE),
            "bad-field", "§2.4(a) yanıt error_code = 0x0003 CHALLENGE"),
        neg("neg.bind-proof-resp-bind-ok-0", "decode-apprpc",
            apprpc(BIND_PROOF, F_RESPONSE, RID_BIND2, b"\x00" + bind_proof_resp(HUB_SIG)[1:]), "bad-field",
            "§2.4(b) yanıt bind_ok = 1"),
        neg("neg.bind-proof-req-short", "decode-apprpc",
            apprpc(BIND_PROOF, F_REQUEST, RID_BIND2,
                   bind_proof_req(ALICE.cert_ref, CLIENT_NONCE, HUB_NONCE, CLIENT_SIG)[:-1]),
            "truncated", "§2.4(b) istek (128 B)"),
        neg("neg.store-envelope-len-mismatch", "decode-apprpc",
            apprpc(STORE_MESSAGE, F_REQUEST, RID_INIT, init_req[:-1]), "length-mismatch",
            "§2.4(c) envelope_len"),
        neg("neg.store-resp-unknown-status", "decode-apprpc",
            apprpc(STORE_MESSAGE, F_RESPONSE, RID_INIT, store_resp(CHALLENGE, 0)), "bad-code",
            "§2.3 STORE yanıt status ∈ {OK, PEER_UNBOUND, UNKNOWN_HANDLE}; §2.5"),
        neg("neg.store-resp-error-code-as-status", "decode-apprpc",
            apprpc(STORE_MESSAGE, F_RESPONSE, RID_INIT, store_resp(BAD_REQUEST, 0)), "bad-code",
            "§2.5 (hata kodları ErrorPayload'da, durum kodları yanıt alanında)"),
        neg("neg.store-resp-ok-handle-0", "decode-apprpc",
            apprpc(STORE_MESSAGE, F_RESPONSE, RID_INIT, store_resp(OK, 0)), "zero-handle",
            "§2.1 (hub hub_handle = 0 hiç vermez); §2.4(c) {OK, h}"),
        neg("neg.error-payload-status-code", "decode-apprpc",
            apprpc(STORE_MESSAGE, F_RESPONSE_ERROR, RID_INIT, error_payload(PEER_UNBOUND)), "bad-code",
            "§2.5 (PEER_UNBOUND yanıt alanıdır, ErrorPayload değil)"),
        neg("neg.error-payload-detail-len-mismatch", "decode-apprpc",
            apprpc(STORE_MESSAGE, F_RESPONSE_ERROR, RID_INIT, u16(BAD_REQUEST) + u16(1)), "length-mismatch",
            "§2.3 ErrorPayload detail_len"),
        neg("neg.deliver-handle-0", "decode-apprpc",
            apprpc(DELIVER, F_REQUEST, 0, deliver(0, ALICE.dest_hash, ENV_INIT, ENV_INIT_BYTES)), "zero-handle",
            "§2.1 (hub_handle = 0 hiç verilmez); §2.4(d)"),
        neg("neg.deliver-unknown-envelope-type", "decode-apprpc",
            apprpc(DELIVER, F_REQUEST, 0, deliver(H_INIT, ALICE.dest_hash, 0x03, ENV_INIT_BYTES)),
            "unknown-envelope-type", "§2.4(c)/(d) envelope_type ∈ {0x01, 0x02, 0x05}"),
        neg("neg.route-miss-reason", "decode-apprpc",
            apprpc(ROUTE_MISS, F_REQUEST, 0, route_miss(H_INIT, 0x02)), "bad-field",
            "§2.4(e) reason (0x01 UNKNOWN_HANDLE)"),
        neg("neg.route-miss-handle-0", "decode-apprpc",
            apprpc(ROUTE_MISS, F_REQUEST, 0, route_miss(0, ROUTE_MISS_UNKNOWN_HANDLE)), "zero-handle",
            "§2.1 (veri datagramında hub_handle ≠ 0); §2.4(e)"),
    ]

    # ── negatifler: STORE geçerli birleşimler (§2.4(c) tablosu) — çözülür, doğrulamada BAD_REQUEST ──
    def bad_store(local, handle, etype, env, ref, note=None):
        frame = apprpc(STORE_MESSAGE, F_REQUEST, RID_INIT, store_req(handle, etype, BOB.dest_hash, env))
        return neg(local, "store-validate", frame, "bad-request", ref, note=note)

    cases += [
        bad_store("neg.store-combo-0-response", 0, ENV_RESPONSE, ENV_RESP_BYTES, "§2.4(c) tablo: 0 · 0x02 geçersiz"),
        bad_store("neg.store-combo-h-init", H_INIT, ENV_INIT, ENV_INIT_BYTES, "§2.4(c) tablo: h · 0x01 geçersiz"),
        bad_store("neg.store-combo-h-relay-open", H_INIT, ENV_RELAY_OPEN, OPEN_NONCE,
                  "§2.4(c) tablo: h · 0x05 geçersiz (medya ACK'i E2E MEDIA_LINK, KR-17)"),
        bad_store("neg.store-envelope-type-03", 0, 0x03, ENV_INIT_BYTES, "§2.4(c) tablo: 0x03 geçersiz"),
        bad_store("neg.store-envelope-type-04", 0, 0x04, ENV_INIT_BYTES, "§2.4(c) tablo: 0x04 geçersiz"),
        bad_store("neg.store-relay-open-65", 0, ENV_RELAY_OPEN, bytes(65),
                  "§2.4(c) kurallar: RELAY_OPEN zarfı ≤ 64 B; §6 RELAY_OPEN_MAX_ENVELOPE"),
    ]
    cases.append(case(f"{s}.store.request.relay-open-64", "store-message-req",
                      spec_ref="§2.4(c) kurallar: RELAY_OPEN zarfı ≤ 64 B (sınır)", flags=["boundary"],
                      input={"hub_handle_hex": "00000000", "envelope_type_hex": "05",
                             "recipient_dest_hash_hex": hx(BOB.dest_hash), "envelope_hex": hx(bytes(64)),
                             "request_id_hex": hx(u32(RID_OPEN))},
                      expected_hex=hx(apprpc(STORE_MESSAGE, F_REQUEST, RID_OPEN,
                                             store_req(0, ENV_RELAY_OPEN, BOB.dest_hash, bytes(64))))))
    notes = {
        "code-choice": "S4 bu satır için hata kodu saymaz; vektör yalnız satırın biçimini (flags 0x03 + "
                       "ErrorPayload) sabitler. Kod seçimi KARAR-KAYDI ile.",
        "boundary": "Sınır değeri: izin verilen en büyük boy (bir bayt fazlası negatif vakadır).",
    }
    desc = ("S4 §2.3 AppRpcFrame ve mesaj tablosunun her satırı (istek / yanıt / hata ayrı vaka), §2.4(a)–(e) "
            "yükleri, §2.5 durum/hata kodları; negatifler: çözme (yapısal) ve STORE geçerli-birleşim "
            "doğrulaması (§2.4(c), BAD_REQUEST).")
    return suite(s, desc, notes, cases)


# ══ s4-bind-v1 — §2.4(a) cookie, §2.4(b) TBS / imzalar ════════════════════════════════════════
def gen_bind():
    s = "s4-bind-v1"
    cases = [
        case(f"{s}.domain-sep.bind", "domain-sep", spec_ref="§2.4(b) domain_sep(\"KAMA-HUB-BIND-V1\")",
             note="16 karakter: dolgu yok.", input={"label_hex": hx(b"KAMA-HUB-BIND-V1")},
             expected_hex=hx(domain_sep(b"KAMA-HUB-BIND-V1"))),
        case(f"{s}.domain-sep.ack", "domain-sep", spec_ref="§2.4(b) domain_sep(\"KAMA-HUB-ACK-V1\")",
             note="15 karakter + bir 0x00.", input={"label_hex": hx(b"KAMA-HUB-ACK-V1")},
             expected_hex=hx(domain_sep(b"KAMA-HUB-ACK-V1"))),
    ]

    def ck(local, ip, epoch, ref, note=None, minute=MINUTE):
        inp = {"src_ip_hex": hx(ip), "src_port_hex": hx(u16(PORT)), "device_cert_ref_hex": hx(ALICE.cert_ref),
               "client_nonce_hex": hx(CLIENT_NONCE), "minute_hex": hx(u32(minute)), "bind_epoch_hex": hx(u32(epoch))}
        ci = cookie_input(ip, PORT, ALICE.cert_ref, CLIENT_NONCE, minute, epoch)
        full = cookie(COOKIE_SECRET, ip, PORT, ALICE.cert_ref, CLIENT_NONCE, minute, epoch)
        return [
            case(f"{s}.cookie-input.{local}", "cookie-input", spec_ref=ref, note=note, input=inp,
                 expected_hex=hx(ci)),
            case(f"{s}.cookie.{local}", "cookie", spec_ref=ref, note=note,
                 input={**inp, "cookie_secret_hex": hx(COOKIE_SECRET)}, expected_hex=hx(full)),
        ]

    cases += ck("ipv4.epoch-0", IPV4, 0, "§2.4(a) hub_nonce (fam 0x04, bind_epoch 0)",
                note="Bu cookie s4-apprpc-v1 meydan okumasındaki hub_nonce'tır.")
    cases += ck("ipv4.epoch-7", IPV4, 7, "§2.4(a) hub_nonce (fam 0x04, bind_epoch 7)",
                note="Aynı girdi, epoch 7: farklı cookie (her kabul önceki cookie'leri geçersizleştirir).")
    cases += ck("ipv4.prev-minute", IPV4, 0, "§2.4(a) minute; §6 COOKIE_WINDOW", minute=MINUTE - 1,
                note="Bir önceki dakika (hub doğrulamada şimdi ve bir önceki dakikayı dener).")
    cases += ck("ipv6.epoch-0", IPV6, 0, "§2.4(a) hub_nonce (fam 0x06, 16 B src_ip)")
    cases += ck("ipv4-mapped.epoch-0", IPV4_MAPPED, 0, "§2.4(a) IPv4-eşlemeli IPv6 → IPv4 normalizasyonu",
                note="::ffff:198.51.100.7 — ipv4.epoch-0 ile BAYT-ÖZDEŞ olmalı.")
    assert cases[-1]["expected_hex"] == cases[3]["expected_hex"]
    assert cases[-2]["expected_hex"] == cases[2]["expected_hex"]
    assert cases[3]["expected_hex"] != cases[5]["expected_hex"]
    assert cases[3]["expected_hex"] == hx(HUB_NONCE)

    base = {"hub_ed25519_pub_hex": hx(HUB_PUB), "device_cert_ref_hex": hx(ALICE.cert_ref),
            "client_nonce_hex": hx(CLIENT_NONCE), "hub_nonce_hex": hx(HUB_NONCE)}
    other_tbs = bind_tbs(OTHER_HUB_PUB, ALICE.cert_ref, CLIENT_NONCE, HUB_NONCE)
    ack_base = {"device_cert_ref_hex": hx(ALICE.cert_ref), "client_nonce_hex": hx(CLIENT_NONCE),
                "hub_nonce_hex": hx(HUB_NONCE), "bind_ok_hex": "01", "error_code_hex": hx(u16(OK))}
    cases += [
        case(f"{s}.bind-tbs.alice", "bind-tbs", spec_ref="§2.4(b) client_signature TBS (112 B)",
             input=base, expected_hex=hx(TBS_BIND)),
        case(f"{s}.bind-tbs.other-hub", "bind-tbs", spec_ref="§2.4(b) TBS hub_ed25519_pub'a bağlı",
             note="Başka hub'ın açık anahtarı: farklı TBS (kanıt başka hub'a taşınamaz).",
             input={**base, "hub_ed25519_pub_hex": hx(OTHER_HUB_PUB)}, expected_hex=hx(other_tbs)),
        case(f"{s}.bind-sig.alice", "bind-sig", spec_ref="§2.4(b) client_signature (Ed25519, RFC 8032)",
             note="Cihaz Ed25519 = sertifikanın ilk 32 baytı (§3 H2); doğrulama katı kip.",
             input={**base, "device_ed25519_seed_hex": hx(ALICE.seed), "device_ed25519_pub_hex": hx(ALICE.pub),
                    "device_cert_hex": hx(ALICE.cert)},
             expected_hex=hx(CLIENT_SIG)),
        case(f"{s}.ack-tbs.alice", "ack-tbs", spec_ref="§2.4(b) hub_signature TBS (83 B)",
             input=ack_base, expected_hex=hx(TBS_ACK)),
        case(f"{s}.ack-sig.alice", "ack-sig", spec_ref="§2.4(b) hub_signature (Ed25519, RFC 8032)",
             input={**ack_base, "hub_ed25519_seed_hex": hx(HUB_SEED), "hub_ed25519_pub_hex": hx(HUB_PUB)},
             expected_hex=hx(HUB_SIG)),
    ]
    assert sha256(ALICE.cert) == ALICE.cert_ref and ALICE.cert[:32] == ALICE.pub
    desc = ("S4 §2.4(a) durumsuz cookie (fam 0x04/0x06, IPv4-eşlemeli normalizasyon, minute, bind_epoch 0/7) ve "
            "§2.4(b) BIND_PROOF/ACK TBS + Ed25519 imzaları; sabit cihaz ve hub tohumları. cookie_secret 32 B.")
    return suite(s, desc, {}, cases)


# ══ s4-alloc-v1 — §2.4(c) tahsis anahtarı, §2.4(f) RELAY_OPEN zarfı, §2.6 handle tahsisi ═══════
def gen_alloc():
    s = "s4-alloc-v1"
    cases = [
        case(f"{s}.relay-open-envelope", "relay-open-envelope",
             spec_ref="§2.4(f) RELAY_OPEN zarfı = open_nonce bytes16 (call_id yok)",
             input={"open_nonce_hex": hx(OPEN_NONCE)}, expected_hex=hx(OPEN_NONCE)),
    ]

    def ak(local, sender, rid, recipient, etype, env, ref, note=None):
        return case(f"{s}.allocation-key.{local}", "allocation-key", spec_ref=ref, note=note,
                    input={"sender_dest_hash_hex": hx(sender), "request_id_hex": hx(u32(rid)),
                           "recipient_dest_hash_hex": hx(recipient), "envelope_type_hex": hx(u8(etype)),
                           "envelope_hex": hx(env), "envelope_hash8_hex": hx(blake2s(env)[:8])},
                    expected_hex=hx(allocation_key(sender, rid, recipient, etype, env)))

    ref_k = "§2.4(c) allocation_key (KR-19; = idempotency-key, §7)"
    cases += [
        ak("init", ALICE.dest_hash, RID_INIT, BOB.dest_hash, ENV_INIT, ENV_INIT_BYTES, ref_k,
           note="envelope_hash8 = BLAKE2s-256(envelope)[0..8]; s4.rs özet almaz, çağıran hesaplar."),
        ak("init-other-recipient", ALICE.dest_hash, RID_INIT, CAROL.dest_hash, ENV_INIT, ENV_INIT_BYTES, ref_k,
           note="Aynı request_id, farklı alıcı → farklı anahtar."),
        ak("init-other-envelope", ALICE.dest_hash, RID_INIT, BOB.dest_hash, ENV_INIT, ENV_RESP_BYTES, ref_k,
           note="Aynı request_id, farklı içerik → farklı anahtar (yeniden başlamış telefon, §2.4(c))."),
        ak("relay-open", ALICE.dest_hash, RID_OPEN, BOB.dest_hash, ENV_RELAY_OPEN, OPEN_NONCE, ref_k),
    ]
    keys = [c["expected_hex"] for c in cases[1:]]
    assert len(set(keys)) == len(keys)

    def cand(counter, ref="§2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4]"):
        return case(f"{s}.handle-candidate.c{counter:x}", "handle-candidate", spec_ref=ref,
                    input={"handle_secret_hex": hx(HANDLE_SECRET), "alloc_counter_hex": hx(u64(counter))},
                    expected_hex=hx(u32(handle_candidate(HANDLE_SECRET, counter))))

    def alloc(local, counter, taken, ref, note=None):
        h, after = handle_alloc(HANDLE_SECRET, counter, set(taken))
        return case(f"{s}.handle-alloc.{local}", "handle-alloc", spec_ref=ref, note=note,
                    input={"handle_secret_hex": hx(HANDLE_SECRET), "alloc_counter_hex": hx(u64(counter)),
                           "taken_handles_hex": "".join(hx(u32(t)) for t in taken)},
                    expected_hex=hx(u32(h) + u64(after))), h, after

    c0 = ALLOC_COUNTER0
    cands = [handle_candidate(HANDLE_SECRET, c0 + i) for i in range(10)]
    assert 0 not in cands and len(set(cands)) == 10
    cases += [cand(c0 + i) for i in range(9)]
    a1, h1, n1 = alloc("first-free", c0, [], "§2.6 (ilk aday boş → tahsis; sayaç +1)",
                       note="beklenen = handle u32 ‖ alloc_counter_sonra u64.")
    a2, h2, n2 = alloc("skip-taken", c0, [cands[0]], "§2.6 (aktif/tombstone aday atlanır; sayaç her denemede +1)",
                       note="İlk aday aktif tabloda: ikinci aday verilir, sayaç +2.")
    a3, h3, n3 = alloc("eight-collisions", c0, cands[:8],
                       "§2.6 (8 denemede de çakışma → tahsis yok, HUB_CAPACITY; üstüne yazma yok); §7 K6",
                       note="handle 0 = tahsis yok; alloc_counter 8 ilerlemiş.")
    a4, h4, n4 = alloc("after-eight-collisions", n3, cands[:8],
                       "§2.6 (sonraki istek aynı adayları yeniden denemez); KM-HUB-TDD-001 T-M5",
                       note="Bir önceki vakanın sonraki sayacından: farklı aday, tahsis başarılı.")
    assert (h1, n1) == (cands[0], c0 + 1) and h1 == H_INIT
    assert (h2, n2) == (cands[1], c0 + 2)
    assert (h3, n3) == (0, c0 + 8)
    assert (h4, n4) == (cands[8], c0 + 9)
    cases += [a1, a2, a3, a4]
    desc = ("S4 §2.4(c) allocation_key baytları (sabit 77 B), §2.4(f) RELAY_OPEN zarfı (16 B open_nonce) ve §2.6 "
            "handle tahsisi (aday üretici, çakışma atlama, 8 başarısız deneme, sayacın ilerlemesi); "
            "handle_secret 32 B. 0 adayının atlanması doğal girdiyle üretilemez (olasılık 2^-32); kural "
            "birim testte sahte üreticiyle sınanır.")
    return suite(s, desc, {}, cases)


SUITES = [gen_hub_frame, gen_apprpc, gen_bind, gen_alloc]


def _sorted_deep(v):
    if isinstance(v, dict):
        return {k: _sorted_deep(v[k]) for k in sorted(v)}
    if isinstance(v, list):
        return [_sorted_deep(x) for x in v]
    return v


def render(doc) -> str:
    """Kanonik biçim (vectors/README §6) — `validate.py --canonical` bu çıktıyı aynen kabul eder."""
    out = dict(doc)
    out["cases"] = [
        {k: (_sorted_deep(v) if k == "input" else v) for k, v in c.items()} for c in doc["cases"]
    ]
    return json.dumps(out, indent=2, ensure_ascii=True) + "\n"


def main(argv) -> int:
    check = "--check" in argv
    bad = 0
    out_dir = os.path.join(ROOT, "vectors", "hub")
    expected_files = set()
    for g in SUITES:
        doc = g()
        name = doc["suite"] + ".json"
        expected_files.add(name)
        path = os.path.join(out_dir, name)
        text = render(doc)
        rel = os.path.relpath(path, ROOT)
        if check:
            try:
                with open(path, encoding="utf-8") as f:
                    on_disk = f.read()
            except OSError:
                on_disk = None
            if on_disk != text:
                print(f"FARK: {rel} üreticinin çıktısıyla aynı değil")
                bad += 1
            else:
                print(f"aynı: {rel} ({doc['case_count']} vaka)")
        else:
            os.makedirs(out_dir, exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)
            print(f"yazıldı: {rel} ({doc['case_count']} vaka)")
    if check and os.path.isdir(out_dir):
        for extra in sorted(set(os.listdir(out_dir)) - expected_files):
            print(f"FARK: vectors/hub/{extra} üreticinin çıktısında yok")
            bad += 1
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
