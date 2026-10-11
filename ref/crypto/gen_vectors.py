#!/usr/bin/env python3
"""A2.2–A2.7 — Session Model V3 test vektörleri (+ K4G-12: CompactDeviceCertV1 ve kimlik türetmesi,
ccr §8.1 / §13.1–§13.5); BAĞIMSIZ Python referansından üretilir.

    python3 ref/crypto/gen_vectors.py            # vectors/ altına yazar
    python3 ref/crypto/gen_vectors.py --check    # yeniden üretir ve DİSKTEKİYLE karşılaştırır (CI)

KURAL: buradaki hiçbir beklenen byte Rust'tan üretilmez. Kaynak yalnız `specs/ccr-format-spec.md`
(§5.1, §5.2, §8.1, §13.1–§13.5, §35.2–§35.9) ve bu dizindeki stdlib-only referanstır (kdf / x25519 /
ed25519 / chacha20poly1305). Rust implementasyonu bu dosyalara KARŞI koşar; tersi değil.

Girdiler deterministiktir: her anahtar `SHA-256("kama-vec:<ad>")`'dan türer. Rastgelelik yok,
dolayısıyla çıktı byte-for-byte tekrarlanabilir.

`result` sözlüğü (vectors/README §5): `drop` = sessiz atım — AEAD/replay başarısızlığı (S3 K5:
`open() → None`); `reject` = yapısal/parse reddi ya da AEAD'den ÖNCE düşen kural.
"""

import hashlib
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

import chacha20poly1305 as aead  # noqa: E402
import ed25519  # noqa: E402
import kdf  # noqa: E402
import x25519  # noqa: E402

SPEC = "ccr-format-spec@v1.0+2026-10-04b"
GEN = "ref/crypto/gen_vectors.py 1"


def sha256(b: bytes) -> bytes:
    return hashlib.sha256(b).digest()


def seed(name: str) -> bytes:
    return sha256(b"kama-vec:" + name.encode())


def hx(b: bytes) -> str:
    return b.hex()


def u16(v: int) -> bytes:
    return struct.pack(">H", v)


def u32(v: int) -> bytes:
    return struct.pack(">I", v)


def u64(v: int) -> bytes:
    return struct.pack(">Q", v)


# ── Sabit dünya ──────────────────────────────────────────────────────────────────────────────
PROTOCOL_VERSION = 0x01
PROFILE_ID = 0x01
CIPHER_SUITE_ID = 0x0001
DIR_A2B, DIR_B2A = 0x01, 0x02

ALICE_STATIC_SK = seed("alice-static-x25519")
ALICE_EPH_SK = seed("alice-init-ephemeral-x25519")
BOB_STATIC_SK = seed("bob-static-x25519")
BOB_EPH_SK = seed("bob-response-ephemeral-x25519")
ALICE_STATIC_PUB = x25519.public_key(ALICE_STATIC_SK)
ALICE_EPH_PUB = x25519.public_key(ALICE_EPH_SK)
BOB_STATIC_PUB = x25519.public_key(BOB_STATIC_SK)
BOB_EPH_PUB = x25519.public_key(BOB_EPH_SK)

# recipient_dest_hash = SHA-256(SHA-256(user_identity_ed25519_pub))  (ccr AC-SM3-10)
BOB_IDENTITY_PUB = ed25519.public_key(seed("bob-user-identity-ed25519"))
BOB_DEST_HASH = sha256(sha256(BOB_IDENTITY_PUB))

# Sertifika byte'ları bu suite'lerde OPAKTIR: SSI1 mührü yalnız "wire'da ne varsa aynen" kuralını
# sınar (ccr §35.3.1). İçerik desenli dolgudur; cert doğrulaması bu vektörlerin konusu DEĞİL.
COMPACT_CERT = bytes((i * 7 + 3) & 0xFF for i in range(152))
CERT_BUNDLE = bytes((i * 11 + 5) & 0xFF for i in range(260))
SENDER_CERT_REF = sha256(COMPACT_CERT)  # ref/inline/bundle için AYNI (ccr §35.2)

SESSION_CONTEXT = (
    bytes([PROTOCOL_VERSION, PROFILE_ID])
    + u16(CIPHER_SUITE_ID)
    + SENDER_CERT_REF
    + BOB_DEST_HASH
    + sha256(BOB_STATIC_PUB)
)
assert len(SESSION_CONTEXT) == 100

SS_E = x25519.x25519(ALICE_EPH_SK, BOB_STATIC_PUB)
SS_S = x25519.x25519(ALICE_STATIC_SK, BOB_STATIC_PUB)
INIT_ROOT = kdf.d1_init_root(SS_E, SS_S, SESSION_CONTEXT)
K_A2B_E0, K_B2A_E0 = kdf.d2_traffic_e0(INIT_ROOT)
SID = kdf.d3_session_id_internal(INIT_ROOT, SESSION_CONTEXT)
SS_E2 = x25519.x25519(BOB_EPH_SK, ALICE_EPH_PUB) + x25519.x25519(BOB_EPH_SK, ALICE_STATIC_PUB)
EPOCH1_ROOT = kdf.d4_epoch_1_root(INIT_ROOT, SS_E2)
K_A2B_E1, K_B2A_E1 = kdf.d5_traffic_e1(EPOCH1_ROOT)
K_INIT = kdf.d6_k_init(INIT_ROOT)
K_RESP = kdf.d7_k_resp(EPOCH1_ROOT)

# Alice tarafı aynı sırları kendi anahtarlarından türetebilmeli (DH simetrisi) — üretici kendini sınar.
assert SS_E == x25519.x25519(BOB_STATIC_SK, ALICE_EPH_PUB)
assert SS_S == x25519.x25519(BOB_STATIC_SK, ALICE_STATIC_PUB)
assert SS_E2 == x25519.x25519(ALICE_EPH_SK, BOB_EPH_PUB) + x25519.x25519(ALICE_STATIC_SK, BOB_EPH_PUB)
assert len({K_A2B_E0, K_B2A_E0, K_A2B_E1, K_B2A_E1, K_INIT, K_RESP}) == 6

WORLD = {
    "protocol_version_hex": hx(bytes([PROTOCOL_VERSION])),
    "profile_id_hex": hx(bytes([PROFILE_ID])),
    "cipher_suite_id_hex": hx(u16(CIPHER_SUITE_ID)),
}


def suite(name, description, notes, cases, spec=SPEC):
    used = {f for c in cases for f in c.get("flags", [])}
    assert used == set(notes), (name, used ^ set(notes))
    ids = [c["id"] for c in cases]
    assert len(ids) == len(set(ids)), name
    return {
        "schema_version": 2,
        "suite": name,
        "spec_version": spec,
        "generator": GEN,
        "description": description,
        "notes": notes,
        "case_count": len(cases),
        "cases": cases,
    }


def case(cid, kind, *, flags=None, note=None, spec_ref=None, input=None, input_hex=None,
         expected_hex=None, expected=None):
    c = {"id": cid, "kind": kind}
    if flags:
        c["flags"] = flags
    if note:
        c["note"] = note
    if spec_ref:
        c["spec_ref"] = spec_ref
    if input is not None:
        c["input"] = input
    if input_hex is not None:
        c["input_hex"] = input_hex
    if expected_hex is not None:
        c["expected_hex"] = expected_hex
    if expected is not None:
        c["expected"] = {"result": expected[0], "reason": expected[1]}
    return c


# ══ A2.2 — ccr-kdf-v1 (V-KDF-1..7) ═══════════════════════════════════════════════════════════
def gen_kdf():
    s = "ccr-kdf-v1"
    ref = "ccr-format-spec.md §35.2.1 "
    hmac_key, hmac_msg = bytes(range(32)), b"kama-inv-kdf-3"
    keyed = hashlib.blake2s(hmac_msg, key=hmac_key).digest()
    cases = [
        case(f"{s}.v-kdf-1.init-root", "derive", flags=["d1"], spec_ref=ref + "V-KDF-1 (D1)",
             input={"label_hex": hx(kdf.L_INIT), "ss_e_hex": hx(SS_E), "ss_s_hex": hx(SS_S),
                    "session_context_v1_hex": hx(SESSION_CONTEXT)},
             expected_hex=hx(INIT_ROOT)),
        case(f"{s}.v-kdf-2.k-a2b-e0", "derive", flags=["d2", "direction-separation"],
             spec_ref=ref + "V-KDF-2 (D2)",
             input={"label_hex": hx(kdf.L_TRAFFIC), "secret_hex": hx(INIT_ROOT), "context_hex": "01"},
             expected_hex=hx(K_A2B_E0)),
        case(f"{s}.v-kdf-2.k-b2a-e0", "derive", flags=["d2", "direction-separation"],
             spec_ref=ref + "V-KDF-2 (D2)",
             input={"label_hex": hx(kdf.L_TRAFFIC), "secret_hex": hx(INIT_ROOT), "context_hex": "02"},
             expected_hex=hx(K_B2A_E0)),
        case(f"{s}.v-kdf-3.session-id-internal", "derive", flags=["d3"], spec_ref=ref + "V-KDF-3 (D3)",
             input={"label_hex": hx(kdf.L_SESSION_ID), "secret_hex": hx(INIT_ROOT),
                    "context_hex": hx(SESSION_CONTEXT), "out_len_hex": "10"},
             expected_hex=hx(SID)),
        case(f"{s}.v-kdf-3.trunc128-of-l32", "derive", flags=["d3", "prefix-identity"],
             note="L=32 çıktısı; ilk 16 baytı session_id_internal ile BYTE-ÖZDEŞ olmalı (T(1) öneki).",
             spec_ref=ref + "V-KDF-3 (D3)",
             input={"label_hex": hx(kdf.L_SESSION_ID), "secret_hex": hx(INIT_ROOT),
                    "context_hex": hx(SESSION_CONTEXT), "out_len_hex": "20"},
             expected_hex=hx(kdf.kdf(kdf.L_SESSION_ID, INIT_ROOT, SESSION_CONTEXT, 32))),
        case(f"{s}.v-kdf-4.epoch-1-root", "derive", flags=["d4"], spec_ref=ref + "V-KDF-4 (D4)",
             input={"label_hex": hx(kdf.L_EPOCH), "init_root_hex": hx(INIT_ROOT), "ss_e2_hex": hx(SS_E2),
                    "context_hex": "0001"},
             expected_hex=hx(EPOCH1_ROOT)),
        case(f"{s}.v-kdf-4.k-a2b-e1", "derive", flags=["d5", "epoch-separation"],
             spec_ref=ref + "V-KDF-4 (D5)",
             input={"label_hex": hx(kdf.L_TRAFFIC), "secret_hex": hx(EPOCH1_ROOT), "context_hex": "01"},
             expected_hex=hx(K_A2B_E1)),
        case(f"{s}.v-kdf-4.k-b2a-e1", "derive", flags=["d5", "epoch-separation"],
             spec_ref=ref + "V-KDF-4 (D5)",
             input={"label_hex": hx(kdf.L_TRAFFIC), "secret_hex": hx(EPOCH1_ROOT), "context_hex": "02"},
             expected_hex=hx(K_B2A_E1)),
        case(f"{s}.v-kdf-5.k-init", "derive", flags=["d6", "domain-separation"],
             spec_ref=ref + "V-KDF-5 (D6)",
             input={"label_hex": hx(kdf.L_INIT_DATA), "secret_hex": hx(INIT_ROOT), "context_hex": "01"},
             expected_hex=hx(K_INIT)),
        case(f"{s}.v-kdf-5.k-resp", "derive", flags=["d7", "domain-separation"],
             spec_ref=ref + "V-KDF-5 (D7)",
             input={"label_hex": hx(kdf.L_RESP_DATA), "secret_hex": hx(EPOCH1_ROOT), "context_hex": "02"},
             expected_hex=hx(K_RESP)),
        case(f"{s}.v-kdf-6.hmac-blake2s", "hmac", flags=["inv-kdf-3"], spec_ref=ref + "V-KDF-6",
             note="DOĞRU primitif: RFC 2104 HMAC, hash = BLAKE2s-256.",
             input={"key_hex": hx(hmac_key), "message_hex": hx(hmac_msg)},
             expected_hex=hx(kdf.hmac_hash(hmac_key, hmac_msg))),
        case(f"{s}.v-kdf-6.blake2s-keyed-is-not-hmac", "hmac-candidate", flags=["inv-kdf-3"],
             spec_ref=ref + "V-KDF-6 (INV-KDF-3)",
             note="candidate = BLAKE2s'in NATIVE keyed modu. Derlenir ve çalışır ama HMAC DEĞİLDİR; "
                  "HMAC yerine bunu üreten implementasyon reddedilmelidir.",
             input={"key_hex": hx(hmac_key), "message_hex": hx(hmac_msg), "candidate_hex": hx(keyed)},
             expected=("reject", "v-kdf-6-blake2s-keyed-is-not-hmac")),
    ]
    assert keyed != kdf.hmac_hash(hmac_key, hmac_msg)
    assert kdf.kdf(kdf.L_SESSION_ID, INIT_ROOT, SESSION_CONTEXT, 32)[:16] == SID
    notes = {
        "d1": "D1 init_root = KDF(SR-SESSION-INIT-V3, ss_e || ss_s, session_context_v1).",
        "d2": "D2 epoch-0 yön anahtarları (context = direction).",
        "d3": "D3 session_id_internal, L=16.",
        "d4": "D4 epoch_1_root; secret = init_root || ss_e2, context = u16_be(1).",
        "d5": "D5 epoch-1 yön anahtarları — D2 ile AYNI label, farklı secret.",
        "d6": "D6 k_init — SESSION_INIT mühür anahtarı.",
        "d7": "D7 k_resp — SESSION_INIT_RESPONSE mühür anahtarı.",
        "direction-separation": "INV-NONCE-DIR-1: k_A2B != k_B2A (iki vakanın expected_hex'i farklı olmalı).",
        "epoch-separation": "INV-NONCE-EPOCH-1: k_*_e1 != k_*_e0.",
        "domain-separation": "D6/D7 trafik anahtarlarından ayrık.",
        "prefix-identity": "L=16 çıktısı L=32'nin ilk 16 baytına eşittir.",
        "inv-kdf-3": "HMAC-BLAKE2s-256 != BLAKE2s-keyed.",
    }
    desc = ("V-KDF-1..6 exact byte'ları (ccr §35.2.1 D1-D7). V-KDF-7 (üç kaynak eşitliği) bir vaka "
            "değil bir SÜREÇ şartıdır: bu dosya bağımsız referanstır; Rust tarafı aynı değerleri "
            "üretince sağlanmış olur. KDF = Extract(salt = ASCII(label), IKM = secret) ardından "
            "Expand(info = context). out_len_hex verilmemişse L = 32.")
    return suite(s, desc, notes, cases)


# ══ A2.3 — ccr-ssi1-v1 ═══════════════════════════════════════════════════════════════════════
MODES = {
    "ref": (0x00, SENDER_CERT_REF),
    "inline": (0x01, COMPACT_CERT),
    "bundle": (0x02, CERT_BUNDLE),
}
INIT_COUNTER = 7
EARLY_DATA = b"kama early unit"


def ssi1_aad(flags: int, eph: bytes, cert_field: bytes, counter: int) -> bytes:
    return (b"SSI1" + bytes([PROTOCOL_VERSION]) + u16(CIPHER_SUITE_ID) + bytes([flags])
            + BOB_DEST_HASH + eph + cert_field + u64(counter) + SID)


def ssi1_wire(flags: int, cert_field: bytes, counter: int, early: bytes) -> bytes:
    aad = ssi1_aad(flags, ALICE_EPH_PUB, cert_field, counter)
    ct_tag = aead.seal(K_INIT, aead.nonce96(0), early, aad)
    return bytes([flags]) + BOB_DEST_HASH + ALICE_EPH_PUB + cert_field + u64(counter) + ct_tag


def ssi1_keys():
    return {
        **WORLD,
        "sender_ephemeral_sk_hex": hx(ALICE_EPH_SK),
        "sender_static_sk_hex": hx(ALICE_STATIC_SK),
        "recipient_static_sk_hex": hx(BOB_STATIC_SK),
        "recipient_static_pub_hex": hx(BOB_STATIC_PUB),
        "recipient_dest_hash_hex": hx(BOB_DEST_HASH),
        "sender_cert_ref_hex": hx(SENDER_CERT_REF),
    }


def gen_ssi1():
    s = "ccr-ssi1-v1"
    ref = "ccr-format-spec.md §35.3.1 "
    cases = []
    for name, (flags, cert) in MODES.items():
        wire = ssi1_wire(flags, cert, INIT_COUNTER, EARLY_DATA)
        aad = ssi1_aad(flags, ALICE_EPH_PUB, cert, INIT_COUNTER)
        assert aead.open_(K_INIT, aead.nonce96(0), wire[len(wire) - len(EARLY_DATA) - 16:], aad) == EARLY_DATA
        base = {**ssi1_keys(), "flags_hex": hx(bytes([flags])), "cert_field_hex": hx(cert),
                "init_counter_hex": hx(u64(INIT_COUNTER))}
        cases.append(case(f"{s}.v-ssi-1.aad.{name}", "aad", flags=["opaque-cert-bytes", "all-private-keys"],
                          spec_ref=ref + "V-SSI-1", input=base, expected_hex=hx(aad)))
        cases.append(case(f"{s}.v-ssi-2.seal.{name}", "seal", flags=["opaque-cert-bytes", "all-private-keys"],
                          spec_ref=ref + "V-SSI-2",
                          note="nonce = nonce96(0) = 12 sıfır bayt; KEY = k_init (D6).",
                          input={**base, "early_data_hex": hx(EARLY_DATA), "nonce_hex": hx(aead.nonce96(0))},
                          expected_hex=hx(wire)))
    # boş early_data (N = 0): yokluk değil, sıfır bayt.
    w0 = ssi1_wire(0x00, SENDER_CERT_REF, INIT_COUNTER, b"")
    assert len(w0) == 121
    cases.append(case(f"{s}.v-ssi-2.seal.ref-empty-early-data", "seal",
                      flags=["opaque-cert-bytes", "all-private-keys"], spec_ref=ref + "V-SSI-2",
                      note="early_data N=0 → app-envelope tam 121 B (ccr §35.3 byte tablosu).",
                      input={**ssi1_keys(), "flags_hex": "00", "cert_field_hex": hx(SENDER_CERT_REF),
                             "init_counter_hex": hx(u64(INIT_COUNTER)), "early_data_hex": "",
                             "nonce_hex": hx(aead.nonce96(0))},
                      expected_hex=hx(w0)))

    def neg(local, wire, result, reason, note, spec_ref):
        return case(f"{s}.{local}", "open", flags=["all-private-keys"], note=note, spec_ref=spec_ref,
                    input={**ssi1_keys(), "wire_hex": hx(wire)}, expected=(result, reason))

    good_inline = ssi1_wire(0x01, COMPACT_CERT, INIT_COUNTER, EARLY_DATA)
    good_ref = ssi1_wire(0x00, SENDER_CERT_REF, INIT_COUNTER, EARLY_DATA)
    # V-SSI-3: flags 1-bit. INLINE (01) → REF (00): yapısal olarak hâlâ ayrışır (uzunluk alanı yok),
    # ama cert_field/init_counter/ct sınırları kayar ve AAD değişir → AEAD düşer.
    f = bytearray(good_inline); f[0] ^= 0x01
    cases.append(neg("v-ssi-3.flags-cert-mode-bit-flip", bytes(f), "drop", "v-ssi-3-aead-fail",
                     "flags bit0 1→0 (INLINE → REF). Parser kabul eder; AEAD düşer. Sessiz atım.",
                     ref + "V-SSI-3"))
    f = bytearray(good_ref); f[0] ^= 0x10
    cases.append(neg("flags-reserved-bit-set", bytes(f), "reject", "ssi1-flags-reserved-nonzero",
                     "flags bit4 (reserved) set → parser reddi; AEAD'ye ULAŞMAZ.",
                     "ccr-format-spec.md §35.3 parser reject"))
    f = bytearray(good_ref); f[0] |= 0x04
    cases.append(neg("flags-init-variant-nonzero", bytes(f), "reject", "ssi1-init-variant-reserved",
                     "init_variant != 00 → parser reddi.", "ccr-format-spec.md §35.3 parser reject"))
    f = bytearray(good_ref); f[0] = 0x03
    cases.append(neg("flags-cert-mode-reserved", bytes(f), "reject", "ssi1-cert-mode-reserved",
                     "cert_mode = 11 → parser reddi.", "ccr-format-spec.md §35.3 parser reject"))
    # V-SSI-4: init_counter değişimi
    f = bytearray(good_ref); f[1 + 32 + 32 + 32 + 7] ^= 0x01
    cases.append(neg("v-ssi-4.init-counter-bit-flip", bytes(f), "drop", "v-ssi-4-aead-fail",
                     "init_counter'ın son bitinde değişim → AAD değişir → AEAD düşer.", ref + "V-SSI-4"))
    # V-SSI-5: cert_mode ↔ uzunluk
    cases.append(neg("v-ssi-5.ref-too-short", good_ref[:120], "reject", "v-ssi-5-length-mismatch",
                     "cert_mode=00 için asgari 121 B; 120 B → parser reddi (AEAD'ye ULAŞMAZ).",
                     ref + "V-SSI-5"))
    cases.append(neg("v-ssi-5.inline-too-short", good_inline[:240], "reject", "v-ssi-5-length-mismatch",
                     "cert_mode=01 için asgari 241 B; 240 B → parser reddi.", ref + "V-SSI-5"))

    # sender_ephemeral: all-zero ve sıfır-olmayan düşük-mertebe (u = 1) → DH tüm-sıfır → reject.
    def with_eph(eph):
        w = bytearray(good_ref); w[33:65] = eph
        return bytes(w)
    low = (1).to_bytes(32, "little")
    assert x25519.is_all_zero(x25519.x25519(BOB_STATIC_SK, low))
    cases.append(neg("sender-ephemeral-all-zero", with_eph(bytes(32)), "reject",
                     "x25519-public-key-all-zero", "sender_ephemeral tüm-sıfır → parse/validation reddi.",
                     "ccr-format-spec.md §5.1"))
    cases.append(neg("sender-ephemeral-low-order-nonzero", with_eph(low), "reject",
                     "x25519-shared-secret-all-zero",
                     "sender_ephemeral = u=1 (sıfır DEĞİL, düşük mertebe): all-zero kontrolünden geçer, "
                     "DH tüm-sıfır verir → reject. Yalnız 'pub != 0' bakan implementasyon bunu kaçırır.",
                     "ccr-format-spec.md §5.1"))
    notes = {
        "opaque-cert-bytes": "cert_field desenli dolgudur; SSI1 mührü yalnız 'wire'da ne varsa aynen' kuralını "
                             "sınar. Sertifika doğrulaması bu suite'in konusu DEĞİL.",
        "all-private-keys": "input TÜM özel anahtarları taşır (gönderen ephemeral + static, alıcı static): "
                            "her iki yön de vektörden türetilebilir. TEST anahtarlarıdır.",
    }
    desc = ("SESSION_INIT (SSI1) exact AAD ve mühür byte'ları, üç cert_mode için; artı negatifler. "
            "Türetim: ss_e = DH(eph, B_static), ss_s = DH(A_static, B_static), D1 → D3 (session_id_internal) "
            "→ D6 (k_init); session_context_v1 = pv || profile || suite || sender_cert_ref || "
            "recipient_dest_hash || SHA-256(recipient_static_pub). nonce = nonce96(0).")
    return suite(s, desc, notes, cases)


# ══ A2.4 — ccr-ssr1-v1 ═══════════════════════════════════════════════════════════════════════
RESPONSE_COUNTER = 0
RESPONSE_DATA = b"kama response"


def ssr1_aad(flags: int, eph: bytes, counter: int) -> bytes:
    return b"SSR1" + bytes([PROTOCOL_VERSION]) + u16(CIPHER_SUITE_ID) + bytes([flags]) + eph + u64(counter) + SID


def ssr1_wire(flags: int, counter: int, data: bytes) -> bytes:
    ct_tag = aead.seal(K_RESP, aead.nonce96(0), data, ssr1_aad(flags, BOB_EPH_PUB, counter))
    return bytes([flags]) + BOB_EPH_PUB + u64(counter) + ct_tag


def gen_ssr1():
    s = "ccr-ssr1-v1"
    ref = "ccr-format-spec.md §35.5.1 "
    keys = {**WORLD,
            "responder_ephemeral_sk_hex": hx(BOB_EPH_SK), "responder_static_sk_hex": hx(BOB_STATIC_SK),
            "initiator_ephemeral_sk_hex": hx(ALICE_EPH_SK), "initiator_static_sk_hex": hx(ALICE_STATIC_SK),
            "init_root_hex": hx(INIT_ROOT), "session_id_internal_hex": hx(SID)}
    wire = ssr1_wire(0x00, RESPONSE_COUNTER, RESPONSE_DATA)
    aad = ssr1_aad(0x00, BOB_EPH_PUB, RESPONSE_COUNTER)
    assert aead.open_(K_RESP, aead.nonce96(0), wire[41:], aad) == RESPONSE_DATA
    assert len(ssr1_wire(0x00, 0, b"")) == 57
    base = {**keys, "flags_hex": "00", "response_counter_hex": hx(u64(RESPONSE_COUNTER))}
    cases = [
        case(f"{s}.v-ssr-1.aad", "aad", flags=["all-private-keys"], spec_ref=ref + "V-SSR-1",
             input=base, expected_hex=hx(aad)),
        case(f"{s}.v-ssr-1.seal", "seal", flags=["all-private-keys"], spec_ref=ref + "V-SSR-1",
             note="ss_e2 = DH(resp_eph, init_eph_pub) || DH(resp_eph, init_static_pub); D4 → D7 (k_resp); "
                  "nonce = nonce96(0).",
             input={**base, "response_data_hex": hx(RESPONSE_DATA), "nonce_hex": hx(aead.nonce96(0))},
             expected_hex=hx(wire)),
        case(f"{s}.v-ssr-1.seal.empty-response-data", "seal", flags=["all-private-keys"],
             spec_ref=ref + "V-SSR-1", note="response_data N=0 → app-envelope tam 57 B (ccr §35.5).",
             input={**base, "response_data_hex": "", "nonce_hex": hx(aead.nonce96(0))},
             expected_hex=hx(ssr1_wire(0x00, RESPONSE_COUNTER, b""))),
    ]

    def neg(local, w, result, reason, note, spec_ref):
        return case(f"{s}.{local}", "open", flags=["all-private-keys"], note=note, spec_ref=spec_ref,
                    input={**keys, "wire_hex": hx(w)}, expected=(result, reason))

    def with_eph(eph):
        w = bytearray(wire); w[1:33] = eph
        return bytes(w)
    low = (1).to_bytes(32, "little")
    assert x25519.is_all_zero(x25519.x25519(ALICE_EPH_SK, low))
    cases += [
        neg("v-ssr-2.responder-ephemeral-all-zero", with_eph(bytes(32)), "reject",
            "x25519-public-key-all-zero", "responder_ephemeral tüm-sıfır → reject; AEAD'den ÖNCE.",
            ref + "V-SSR-2"),
        neg("responder-ephemeral-low-order-nonzero", with_eph(low), "reject",
            "x25519-shared-secret-all-zero",
            "responder_ephemeral = u=1 (düşük mertebe, sıfır değil) → DH tüm-sıfır → reject.",
            "ccr-format-spec.md §35.5 reject kuralları + §5.1"),
    ]
    f = bytearray(wire); f[1 + 32 + 7] ^= 0x01
    cases.append(neg("v-ssr-3.response-counter-bit-flip", bytes(f), "drop", "v-ssr-3-aead-fail",
                     "response_counter'da 1 bit → AAD değişir → AEAD düşer. Sessiz atım.", ref + "V-SSR-3"))
    f = bytearray(wire); f[0] = 0x01
    cases.append(neg("flags-bit0-set", bytes(f), "reject", "ssr1-flags-nonzero",
                     "[C-4] flags.bit0 = 0 zorunlu; bits1..7 reserved. Sıfır-olmayan flags → reject (fail-closed).",
                     "ccr-format-spec.md §35.5 [C-4]"))
    cases.append(neg("too-short", wire[:56], "reject", "ssr1-length-below-minimum",
                     "Asgari app-envelope 57 B; 56 B → parser reddi.", "ccr-format-spec.md §35.5"))
    notes = {"all-private-keys": "input TÜM özel anahtarları taşır (responder ephemeral + static, initiator "
                                 "ephemeral + static). TEST anahtarlarıdır."}
    desc = ("SESSION_INIT_RESPONSE (SSR1) exact AAD ve mühür byte'ları + negatifler. init_root ve "
            "session_id_internal ccr-ssi1-v1 dünyasından gelir (aynı anahtarlar).")
    return suite(s, desc, notes, cases)


# ══ A2.5 — ccr-ssd1-v1 ═══════════════════════════════════════════════════════════════════════
HUB_HANDLE = 0x00C0FFEE
REPLAY_WINDOW_SIZE = 1024


def ssd1_aad(direction: int, epoch: int, seq16: int) -> bytes:
    return (b"SSD1" + bytes([PROTOCOL_VERSION]) + u16(CIPHER_SUITE_ID) + bytes([direction])
            + u16(epoch) + u16(seq16) + SID)


def ssd1_wire(key: bytes, direction: int, epoch: int, counter: int, payload: bytes) -> bytes:
    seq16 = counter & 0xFFFF
    ct_tag = aead.seal(key, aead.nonce96(counter), payload, ssd1_aad(direction, epoch, seq16))
    return u32(HUB_HANDLE) + u16(seq16) + ct_tag


def reconstruct(expected: int, seq16: int) -> int:
    """ccr §35.8: üç aday, argmin |c − expected|; eşitlikte geriye (düşük) aday."""
    base = expected & ~0xFFFF
    cands = [c for c in (base | seq16, (base - 0x10000) | seq16, (base + 0x10000) | seq16) if c >= 0]
    return min(cands, key=lambda c: (abs(c - expected), c))


def gen_ssd1():
    s = "ccr-ssd1-v1"
    ref = "ccr-format-spec.md "
    sess = {**WORLD, "session_id_internal_hex": hx(SID), "hub_handle_hex": hx(u32(HUB_HANDLE))}
    cases = []

    def seal_case(local, key, key_name, direction, epoch, counter, payload, note=None):
        w = ssd1_wire(key, direction, epoch, counter, payload)
        assert len(w) == 22 + len(payload)
        cases.append(case(
            f"{s}.seal.{local}", "seal", flags=["layer-22b"], note=note,
            spec_ref=ref + "§35.6 + §35.9",
            input={**sess, "key_hex": hx(key), "key_name_hex": hx(key_name.encode()),
                   "direction_hex": hx(bytes([direction])), "epoch_hex": hx(u16(epoch)),
                   "full_counter_hex": hx(u64(counter)), "payload_hex": hx(payload),
                   "aad_hex": hx(ssd1_aad(direction, epoch, counter & 0xFFFF)),
                   "nonce_hex": hx(aead.nonce96(counter))},
            expected_hex=hx(w)))
        return w

    seal_case("a2b-e1-counter-0", K_A2B_E1, "k_A2B_e1", DIR_A2B, 1, 0, b"hello")
    seal_case("b2a-e1-counter-0", K_B2A_E1, "k_B2A_e1", DIR_B2A, 1, 0, b"hello",
              "Aynı sayaç, ters yön: farklı anahtar + AAD'de farklı direction.")
    seal_case("a2b-e0-counter-3", K_A2B_E0, "k_A2B_e0", DIR_A2B, 0, 3, b"epoch zero")
    seal_case("a2b-e1-counter-65541", K_A2B_E1, "k_A2B_e1", DIR_A2B, 1, 65541, b"wrapped",
              "full_counter = 65536 + 5 → seq16 = 5; nonce TAM sayacı taşır (seq16 nonce DEĞİL).")
    seal_case("a2b-e1-empty-batch", K_A2B_E1, "k_A2B_e1", DIR_A2B, 1, 9, b"\x00",
              "BOŞ BATCH: BatchPayload{submsg_count = 0} = tek bayt 0x00 → 23 B UDP payload. Onay/keepalive "
              "taşıyıcısı (ccr; DR-CONNECT-1 K3). AEAD'den geçer, PN tüketir, Application'a teslim YOK.")
    assert len(ssd1_wire(K_A2B_E1, DIR_A2B, 1, 9, b"\x00")) == 23

    def open_case(local, expected_counter, seen, w, *, plaintext=None, neg=None, note=None, spec_ref=None,
                  flags=None):
        cases.append(case(
            f"{s}.open.{local}", "open", flags=flags, note=note, spec_ref=spec_ref,
            input={**sess, "key_hex": hx(K_A2B_E1), "direction_hex": hx(bytes([DIR_A2B])),
                   "epoch_hex": hx(u16(1)), "expected_counter_hex": hx(u64(expected_counter)),
                   "seen_counters_hex": [hx(u64(c)) for c in seen], "wire_hex": hx(w)},
            expected_hex=None if plaintext is None else hx(plaintext), expected=neg))

    # D-GAP-1: far-future AEAD-geçerli → ACCEPT
    far = ssd1_wire(K_A2B_E1, DIR_A2B, 1, 10_000, b"after outage")
    assert reconstruct(1, 10_000 & 0xFFFF) == 10_000
    open_case("far-future-aead-valid-accept", 1, [0], far, plaintext=b"after outage", flags=["d-gap-1"],
              note="expected = 1, gelen full_counter = 10 000 (eski MAX_FUTURE_GAP 4096 ve MAX_REORDER_DISTANCE "
                   "8192'nin ötesi). Far-future kapısı YOK: AEAD geçerliyse KABUL; highest 10 000'e sıçrar.",
              spec_ref=ref + "§35.8 [D-GAP-1]")
    in_order = ssd1_wire(K_A2B_E1, DIR_A2B, 1, 5, b"in order")
    open_case("in-order-accept", 5, [0, 1, 2, 3, 4], in_order, plaintext=b"in order", flags=["d-gap-1"],
              spec_ref=ref + "§35.8")
    late = ssd1_wire(K_A2B_E1, DIR_A2B, 1, 2000, b"late")
    assert 2000 + REPLAY_WINDOW_SIZE > 3000
    open_case("late-within-window-accept", 3001, [3000], late, plaintext=b"late", flags=["d-gap-1"],
              note="expected = 3001, gelen 2000: 2000 + 1024 > 3000 → pencere içinde ve görülmemiş → KABUL.",
              spec_ref=ref + "§35.8")
    open_case("duplicate", 6, [0, 1, 2, 3, 4, 5], in_order, neg=("drop", "ssd1-replay-duplicate"),
              note="full_counter = 5 zaten görüldü → replay. Sessiz atım; state değişmez.",
              spec_ref=ref + "§35.8 + §35.12")
    old = ssd1_wire(K_A2B_E1, DIR_A2B, 1, 100, b"too old")
    assert 100 + REPLAY_WINDOW_SIZE <= 5000
    open_case("too-old", 5001, [5000], old, neg=("drop", "ssd1-replay-too-old"),
              note="eski-paket kuralı: full_counter + REPLAY_WINDOW_SIZE <= highest → 100 + 1024 <= 5000.",
              spec_ref=ref + "§35.8")
    t = bytearray(in_order); t[-1] ^= 0x01
    open_case("tag-bit-flip", 5, [0, 1, 2, 3, 4], bytes(t), neg=("drop", "ssd1-aead-fail"),
              note="tag'in son biti → AEAD düşer; replay penceresi GÜNCELLENMEZ.", spec_ref=ref + "§35.12")
    wrong_dir = ssd1_wire(K_B2A_E1, DIR_B2A, 1, 5, b"in order")
    open_case("cross-direction", 5, [0, 1, 2, 3, 4], wrong_dir, neg=("drop", "ssd1-aead-fail"),
              note="B→A yönünde mühürlenmiş paket A→B alıcısına verildi: anahtar ve AAD direction farklı → AEAD düşer.",
              spec_ref=ref + "§35.9")
    wrong_epoch = ssd1_wire(K_A2B_E0, DIR_A2B, 0, 5, b"in order")
    open_case("cross-epoch", 5, [0, 1, 2, 3, 4], wrong_epoch, neg=("drop", "ssd1-aead-fail"),
              note="epoch-0 anahtarıyla mühürlenmiş paket epoch-1 state'ine verildi → AEAD düşer.",
              spec_ref=ref + "§35.9")
    beyond = ssd1_wire(K_A2B_E1, DIR_A2B, 1, 100_001 + 65_536 - 500, b"beyond")
    assert reconstruct(100_001, (100_001 + 65_036) & 0xFFFF) == 100_001 - 500
    open_case("beyond-structural-limit", 100_001, [100_000], beyond, neg=("drop", "ssd1-aead-fail"),
              flags=["d-gap-1"],
              note="Gerçek sayaç expected'ten 65 036 ileride (> 2^15): seq16 EN YAKIN adaya kurulur "
                   "(expected − 500, pencere içi) → nonce uyuşmaz → AEAD düşer. Kapı değil, yapısal sınır; "
                   "meşru trafikte gönderen korur (KS-PN-1, K = 1024).",
              spec_ref=ref + "§35.8 [D-GAP-1] yapısal sınır")
    open_case("too-short", 5, [], in_order[:21], neg=("reject", "ssd1-length-below-minimum"),
              note="Asgari UDP payload 22 B (hub_handle 4 + seq16 2 + tag 16); 21 B → parser reddi.",
              spec_ref=ref + "§35.6")
    notes = {
        "layer-22b": "UDP payload = hub_handle 4 + seq16 2 + ciphertext + tag 16 = 22 B + payload (AC-SM3-2).",
        "d-gap-1": "ccr §35.8 [D-GAP-1]: far-future reddi yok; geriye tek sınır eski-paket kuralı; yapısal "
                   "sınır |full − expected| < 2^15.",
    }
    desc = ("SESSION_DATA (SSD1) 22 B zarf: exact AAD (§35.9), nonce96 = 00x4 || LE64(full_counter), mühür "
            "byte'ları; artı alım kararları. `open` vakalarında alıcı state'i: expected_counter (= highest + 1) "
            "ve seen_counters (pencerede görülmüş sayaçlar). hub_handle AAD'ye GİRMEZ.")
    return suite(s, desc, notes, cases)


# ══ A2.6 — ccr-deprecated-neg-v1 ═════════════════════════════════════════════════════════════
def gen_deprecated():
    s = "ccr-deprecated-neg-v1"
    filler = bytes(range(48))
    cases = []
    for magic, what in (("SAI1", "ASYNC_INIT"), ("SAA1", "ASYNC_ACCEPT"), ("SAD1", "ASYNC_DATA"),
                        ("SAC1", "ASYNC_CONTROL"), ("SRAM", "eski tek-yapı AsyncMessage")):
        cases.append(case(
            f"{s}.async-v2.{magic.lower()}", "decode", flags=["deprecated-magic"],
            note=f"{what} magic'i '{magic}' aktif wire'da → reject. ACCEPT döndüren parser hatalıdır.",
            spec_ref="ccr-format-spec.md §35.12 / AC-SM3-12",
            input_hex=hx(magic.encode() + filler),
            expected=("reject", f"deprecated-{magic.lower()}")))
    for n, magic, what in ((1, "SRPK", "SignedNoisePrekey"), (2, "SRPB", "PrekeyBundle")):
        cases.append(case(
            f"{s}.deprecated-reject-{n}.{magic.lower()}", "decode", flags=["deprecated-magic", "spec-named-case"],
            note=f"{what} ('{magic}') aktif wire'da görünüyor → reject.",
            spec_ref=f"ccr-format-spec.md §30.7 DEPRECATED-REJECT-{n}",
            input_hex=hx(magic.encode() + filler),
            expected=("reject", f"deprecated-reject-{n}-{magic.lower()}")))
    notes = {
        "deprecated-magic": "MUST NOT IMPLEMENT bir formatın magic'i. Byte'ların geri kalanı desenli dolgudur; "
                            "ret kararı YALNIZ magic'ten verilir.",
        "spec-named-case": "ccr-format-spec.md §30.7'de ADI GEÇEN corpus vakası.",
    }
    desc = ("Deprecated formatların reddi: Async Session V2 (SAI1/SAA1/SAD1/SAC1), eski SRAM, ve §30.7 "
            "historical negative corpus (SRPK, SRPB). Aktif parser bunların HİÇBİRİ için ACCEPT döndüremez.")
    return suite(s, desc, notes, cases)


# ══ A2.7 — ccr-ed25519-neg-v1 ════════════════════════════════════════════════════════════════
def gen_ed25519():
    s = "ccr-ed25519-neg-v1"
    ref = "ccr-format-spec.md §5.2"
    sk = seed("ed25519-signer")
    pk = ed25519.public_key(sk)
    msg = b"kama ed25519 strict verification"
    sig = ed25519.sign(sk, msg)
    assert ed25519.verify_reason(pk, msg, sig) == ed25519.OK
    a, _ = ed25519._secret_expand(sk)

    # burulma alt grubu HESAPLANIR (ezber yok)
    y = 2
    while True:
        x = ed25519._recover_x(y, 0)
        if x is not None:
            t = ed25519._mul((x, y, 1, x * y % ed25519.P), ed25519.L)
            if ed25519.is_small_order(t) and not ed25519._eq(ed25519._mul(t, 4), ed25519.NEUTRAL):
                break
        y += 1
    torsion = [ed25519._compress(ed25519._mul(t, i)) for i in range(8)]

    cases = []

    def neg(local, pk_, msg_, sig_, reason, note, flags):
        assert ed25519.verify_reason(pk_, msg_, sig_) == reason, (local, ed25519.verify_reason(pk_, msg_, sig_))
        cases.append(case(f"{s}.{local}", "verify", flags=flags, note=note, spec_ref=ref,
                          input={"public_key_hex": hx(pk_), "message_hex": hx(msg_), "signature_hex": hx(sig_)},
                          expected=("reject", reason)))

    # 1) S >= L
    s_plus_l = sig[:32] + (int.from_bytes(sig[32:], "little") + ed25519.L).to_bytes(32, "little")
    assert ed25519.verify_cofactored_no_small_order_check(pk, msg, s_plus_l)
    neg("s-not-below-group-order", pk, msg, s_plus_l, ed25519.R_S_NOT_CANONICAL,
        "S' = S + L: aynı denklemi sağlar (mod L indirgeyen gevşek doğrulayıcı KABUL eder) ama kanonik değil → "
        "imza malleability.", ["lax-verifier-accepts"])
    # 2) small-order A — nötr eleman: R = [r]B, S = r HER mesajı 'imzalar'
    r = int.from_bytes(seed("ed25519-forgery-r"), "little") % ed25519.L
    forged = ed25519._compress(ed25519._mul(ed25519.B, r)) + r.to_bytes(32, "little")
    for i, label in ((0, "identity"), (4, "order-2"), (1, "order-8")):
        A = torsion[i]
        m = b"forged under small-order key"
        if i == 0:
            f_sig, flags = forged, ["lax-verifier-accepts", "universal-forgery"]
            assert ed25519.verify_cofactored_no_small_order_check(A, m, f_sig)
            note = ("A = nötr eleman: [k]A = O olduğundan R = [r]B, S = r HER mesaj için kofaktörsüz denklemi "
                    "sağlar. Small-order kontrolü olmayan doğrulayıcı bu anahtar altında her şeyi kabul eder.")
        else:
            f_sig, flags = forged, ["lax-verifier-accepts"]
            assert ed25519.verify_cofactored_no_small_order_check(A, m, f_sig)
            note = (f"A = mertebe-{label.split('-')[1]} nokta: [8][k]A = O olduğundan KOFAKTÖRLÜ denklem her "
                    "mesaj için sağlanır.")
        neg(f"small-order-public-key.{label}", A, m, f_sig, ed25519.R_SMALL_ORDER_A, note, flags)
    # 3) small-order R — R = T, S = k·a: kofaktörlü denklem sağlanır
    for i, label in ((1, "order-8"), (2, "order-4")):
        Rt = torsion[i]
        k = int.from_bytes(hashlib.sha512(Rt + pk + msg).digest(), "little") % ed25519.L
        sig_r = Rt + ((k * a) % ed25519.L).to_bytes(32, "little")
        assert ed25519.verify_cofactored_no_small_order_check(pk, msg, sig_r)
        neg(f"small-order-r.{label}", pk, msg, sig_r, ed25519.R_SMALL_ORDER_R,
            "R burulma noktası, S = k·a: [8][S]B = [8]R + [8][k]A sağlanır (kofaktörlü doğrulayıcı KABUL eder), "
            "kofaktörsüz denklem sağlanmaz.", ["lax-verifier-accepts"])
    # 4) kanonik olmayan kodlama
    p_plus_1 = (ed25519.P + 1).to_bytes(32, "little")
    neg("non-canonical-encoding.public-key-y-ge-p", p_plus_1, msg, sig, ed25519.R_NON_CANONICAL_POINT,
        "A'nın y'si p + 1 (≡ 1): indirgeyen decoder bunu nötr eleman olarak kabul ederdi. y >= p → reject.",
        ["non-canonical"])
    neg("non-canonical-encoding.r-y-ge-p", pk, msg, p_plus_1 + sig[32:], ed25519.R_NON_CANONICAL_POINT,
        "R'nin y'si p + 1 → reject.", ["non-canonical"])
    x0_sign = bytearray(torsion[0]); x0_sign[31] |= 0x80
    neg("non-canonical-encoding.x-zero-sign-bit-set", bytes(x0_sign), msg, sig, ed25519.R_NON_CANONICAL_POINT,
        "y = 1 (x = 0) iken sign biti 1: '−0' kodlaması. RFC 8032 §5.1.3 → reject.", ["non-canonical"])
    # ilk-düşen-kural sırası
    both = p_plus_1 + (int.from_bytes(sig[32:], "little") + ed25519.L).to_bytes(32, "little")
    neg("precedence.s-check-before-point-decoding", pk, msg, both, ed25519.R_S_NOT_CANONICAL,
        "Hem S >= L hem R kanonik değil: İLK düşen kural S kontrolüdür.", ["precedence"])
    notes = {
        "lax-verifier-accepts": "Gevşek bir doğrulayıcı (kofaktörlü denklem, small-order ve S < L kontrolü yok) bu "
                                "girdiyi KABUL eder — üretici bunu assert etti. Vektör 'bozuk imza' değil, 'kip "
                                "farkı'dır.",
        "universal-forgery": "Bu anahtar altında her mesaj için geçerli sayılan imza.",
        "non-canonical": "Nokta kodlaması kanonik değil (RFC 8032 §5.1.3).",
        "precedence": "Birden çok kuralı ihlal eden girdi; reason İLK düşen kuraldır (sıra: uzunluk → S < L → "
                      "nokta decoding → small-order A → small-order R → denklem).",
    }
    desc = ("Ed25519 doğrulama kipi negatifleri (ccr §5.2): S >= L · small-order A · small-order R · kanonik "
            "olmayan kodlama — her biri AYRI vaka. Kütüphane eşlemesi (ed25519-dalek verify_strict) bu "
            "vektörlerle teyit edilir. Burulma noktaları üreticide HESAPLANIR, ezberden yazılmaz.")
    return suite(s, desc, notes, cases)


# ══ K4G-12 — ccr-dest-hash-v1 + ccr-compact-device-cert-v1 (ccr §8.1 / §13; YAMA 2026-10-11) ════
# Plan KAMA-PLAN-4GUN-APP-KRIPTO Ek 2026-10-11 (3), DUR-K4G-12: (a) dest_hash cihazdan AYRI bir kimlik
# Ed25519 anahtarından; (b) domain_sep = ASCII etiket + 0x00 dolgu (ccr §13.2 YAMA 2026-10-11); (c) bu
# vektörler. PoC'de CompactDeviceCert'i test kökü imzalar (plan D9); üretimde imzalayan sk_user_identity.
SPEC_K4G12 = "ccr-format-spec@v1.0+2026-10-11"
DOMAIN_SEP_LEN = 16
SPKI_ALG_ED25519 = 0x01  # ccr §6.3
CDC_LEN = 152  # ccr §13.1
CDC_SIGNED_FIELDS_LEN = 32 + 32 + 16 + 8
CDC_NOT_AFTER = 1791676800 + 90 * 24 * 60 * 60  # 2026-10-11T00:00Z + 90 gün (ccr §13.1 üst sınırı)

IDENTITY_SK = seed("provision-identity-ed25519")
IDENTITY_PUB = ed25519.public_key(IDENTITY_SK)
DEVICE_ED25519_PUB = ed25519.public_key(seed("provision-device-ed25519"))
DEVICE_X25519_PUB = x25519.public_key(seed("provision-device-static-x25519"))
ROOT_SK = seed("provision-root-ed25519")
ROOT_PUB = ed25519.public_key(ROOT_SK)
assert len({IDENTITY_PUB, DEVICE_ED25519_PUB, ROOT_PUB}) == 3


def domain_sep(label: bytes) -> bytes:
    """ccr §13.2 [YAMA 2026-10-11 — K4G-12]: ASCII etiket (<= 16 B) + 16 B'a 0x00 dolgu (sağa)."""
    assert label.isascii() and 0 < len(label) <= DOMAIN_SEP_LEN, label
    return label + bytes(DOMAIN_SEP_LEN - len(label))


def spki_key_id16(pub: bytes) -> bytes:
    """ccr §8.1 subject_key_id = SHA-256(spki_encoding(alg, key)), spki = alg(u8) || key (33 B);
    CompactDeviceCert issuer_key_id = ilk 16 bayt (ccr §13.4)."""
    return sha256(bytes([SPKI_ALG_ED25519]) + pub)[:16]


def gen_dest_hash():
    s = "ccr-dest-hash-v1"
    ref = "ccr-format-spec.md §13.3 / §13.5 (CAD-16)"
    identity_hash = sha256(IDENTITY_PUB)
    dest_hash = sha256(identity_hash)
    # Reddedilen formül (cihaz anahtarından) farklı sonuç verir — vektör bunu ayırt eder.
    assert dest_hash != sha256(sha256(DEVICE_ED25519_PUB))
    # Ham anahtar özeti spki özetinden farklıdır (ccr §8.1).
    assert identity_hash != sha256(bytes([SPKI_ALG_ED25519]) + IDENTITY_PUB)
    identity = {"identity_ed25519_pub_hex": hx(IDENTITY_PUB)}
    cases = [
        case(f"{s}.identity-hash", "identity-hash", flags=["raw-key-not-spki"], spec_ref=ref,
             note="identity_hash = SHA-256(user_identity_ed25519_pub) — ham 32 B anahtar üzerinden.",
             input=identity, expected_hex=hx(identity_hash)),
        case(f"{s}.dest-hash", "dest-hash", flags=["identity-key-not-device-key"], spec_ref=ref,
             note="dest_hash = SHA-256(identity_hash) = SHA-256(SHA-256(user_identity_ed25519_pub)).",
             input=identity, expected_hex=hx(dest_hash)),
    ]
    notes = {
        "raw-key-not-spki": "identity_hash HAM 32 B anahtarın özetidir; 33 B spki_encoding (alg || key) "
                            "özeti DEĞİLDİR (ccr §8.1 ayrımı).",
        "identity-key-not-device-key": "dest_hash cihazdan AYRI kimlik Ed25519 anahtarından türer; "
                                       "SHA-256(SHA-256(device_ed25519_pub)) ccr §13.3'te YASAK ve "
                                       "DUR-K4G-12 (a) ile reddedildi (plan Ek 2026-10-11 (3)).",
    }
    desc = ("Kimlik türetmesi (ccr §13.3 / §13.5, CAD-16): sabit bir kimlik Ed25519 açık anahtarı için "
            "identity_hash ve dest_hash. PoC'de kama-provision cihaz başına cihaz anahtarlarından AYRI bir "
            "kimlik çifti üretir (plan K4G-12 Ek (3)); bu vektörler türetmeyi sabitler.")
    return suite(s, desc, notes, cases, spec=SPEC_K4G12)


def gen_compact_device_cert():
    s = "ccr-compact-device-cert-v1"
    label = b"SR_CDC_V1"
    sep = domain_sep(label)
    assert hx(sep) == "53525f4344435f5631" + "00" * 7
    key_id = spki_key_id16(ROOT_PUB)
    signed = DEVICE_ED25519_PUB + DEVICE_X25519_PUB + key_id + u64(CDC_NOT_AFTER)
    assert len(signed) == CDC_SIGNED_FIELDS_LEN
    tbs = sep + signed
    assert len(tbs) == DOMAIN_SEP_LEN + CDC_SIGNED_FIELDS_LEN
    signature = ed25519.sign(ROOT_SK, tbs)
    assert ed25519.verify_reason(ROOT_PUB, tbs, signature) == ed25519.OK
    # Etiketsiz (ya da yanlış dolgulu) TBS üzerindeki imza bu imzayla doğrulanmaz.
    assert ed25519.verify_reason(ROOT_PUB, signed, signature) != ed25519.OK
    cert = signed + signature
    assert len(cert) == CDC_LEN
    fields = {
        "device_ed25519_pub_hex": hx(DEVICE_ED25519_PUB),
        "device_x25519_pub_hex": hx(DEVICE_X25519_PUB),
        "issuer_key_id_hex": hx(key_id),
        "not_after_hex": hx(u64(CDC_NOT_AFTER)),
    }
    cases = [
        case(f"{s}.domain-sep.sr-cdc-v1", "domain-sep", flags=["domain-sep-patch"],
             spec_ref="ccr-format-spec.md §13.2 (domain_sep, YAMA 2026-10-11)",
             note="16 B etiket = ASCII etiket (<= 16 B) + 0x00 dolgu.",
             input={"label_hex": hx(label)}, expected_hex=hx(sep)),
        case(f"{s}.issuer-key-id.root", "issuer-key-id", flags=["poc-root-issuer"],
             spec_ref="ccr-format-spec.md §8.1 / §13.4",
             note="issuer_key_id = SHA-256(0x01 || issuer_ed25519_pub)[0..16].",
             input={"issuer_ed25519_pub_hex": hx(ROOT_PUB)}, expected_hex=hx(key_id)),
        case(f"{s}.tbs.root-issued", "tbs", flags=["domain-sep-patch"],
             spec_ref="ccr-format-spec.md §13.2",
             note="compact_device_cert_tbs = domain_sep(\"SR_CDC_V1\") || device_ed25519_pub || "
                  "device_x25519_pub || issuer_key_id || not_after (104 B).",
             input=fields, expected_hex=hx(tbs)),
        case(f"{s}.encode.root-signed", "encode", flags=["poc-root-issuer"],
             spec_ref="ccr-format-spec.md §13.1 / §13.2",
             note="Tam 152 B; issuer_signature = Ed25519_Sign(issuer, tbs) (RFC 8032, belirlenimli).",
             input={**fields, "issuer_ed25519_seed_hex": hx(ROOT_SK)}, expected_hex=hx(cert)),
    ]
    notes = {
        "domain-sep-patch": "domain_sep dolgusu ccr §13.2 YAMA KAYDI 2026-10-11 (K4G-12) ile tanımlandı; "
                            "SPEC-GAP S4-pki-discovery.6 kapandı.",
        "poc-root-issuer": "PoC'de Identity/User Cert katmanı yok: CompactDeviceCert'i test kökü doğrudan "
                           "imzalar (plan D9); issuer_key_id kökün spki özetinden.",
    }
    desc = ("CompactDeviceCertV1 (ccr §13.1 / §13.2): domain_sep(\"SR_CDC_V1\"), issuer_key_id, TBS ve tam "
            "imzalı 152 B sertifika — sabit girdilerle. examples/example-positive.json'daki yer tutucu "
            "dosyanın aksine imza HESAPLANMIŞTIR (ref/crypto/ed25519.py, katı kipte doğrulandı).")
    return suite(s, desc, notes, cases, spec=SPEC_K4G12)


SUITES = [gen_kdf, gen_ssi1, gen_ssr1, gen_ssd1, gen_deprecated, gen_ed25519, gen_dest_hash,
          gen_compact_device_cert]


def _sorted_deep(v):
    if isinstance(v, dict):
        return {k: _sorted_deep(v[k]) for k in sorted(v)}
    if isinstance(v, list):
        return [_sorted_deep(x) for x in v]
    return v


def render(doc) -> str:
    """Kanonik biçim (vectors/README §6): üst düzey ve vaka anahtarları SCHEMA.json bildirim sırasında
    (üretici zaten o sırayla kurar), `input` haritaları özyinelemeli leksikografik; 2 boşluk girinti;
    ASCII kaçışlı. `scripts/vectors/validate.py --canonical` bu çıktıyı AYNEN kabul etmelidir."""
    out = dict(doc)
    out["cases"] = [
        {k: (_sorted_deep(v) if k == "input" else v) for k, v in c.items()} for c in doc["cases"]
    ]
    return json.dumps(out, indent=2, ensure_ascii=True) + "\n"


def main(argv) -> int:
    check = "--check" in argv
    bad = 0
    for g in SUITES:
        doc = g()
        path = os.path.join(ROOT, "vectors", doc["suite"], doc["suite"] + ".json")
        text = render(doc)
        if check:
            try:
                on_disk = open(path, encoding="utf-8").read()
            except OSError:
                on_disk = None
            if on_disk != text:
                print(f"FARK: {os.path.relpath(path, ROOT)} üreticinin çıktısıyla aynı değil")
                bad += 1
            else:
                print(f"aynı: {os.path.relpath(path, ROOT)} ({doc['case_count']} vaka)")
        else:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)
            print(f"yazıldı: {os.path.relpath(path, ROOT)} ({doc['case_count']} vaka)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
