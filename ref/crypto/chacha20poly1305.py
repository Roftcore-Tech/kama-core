"""ChaCha20-Poly1305 AEAD — RFC 8439. BAĞIMSIZ referans (A2.1, R-7(d)).

Registry: `specs/pki-ca-ra-hsm-tdd.md:455` — key 32B / nonce 12B / tag 16B.
Sabit-zamanlı DEĞİL; yalnız test vektörü üretir/doğrular. Üretim = libsodium (ADR-03).
"""

import struct

MASK32 = 0xFFFFFFFF


# ── RFC 8439 §2.1–2.3: ChaCha20 blok fonksiyonu ──────────────────────────────────────────
def _rotl(v, n):
    return ((v << n) | (v >> (32 - n))) & MASK32


def _qr(s, a, b, c, d):
    s[a] = (s[a] + s[b]) & MASK32; s[d] = _rotl(s[d] ^ s[a], 16)
    s[c] = (s[c] + s[d]) & MASK32; s[b] = _rotl(s[b] ^ s[c], 12)
    s[a] = (s[a] + s[b]) & MASK32; s[d] = _rotl(s[d] ^ s[a], 8)
    s[c] = (s[c] + s[d]) & MASK32; s[b] = _rotl(s[b] ^ s[c], 7)


_CONST = (0x61707865, 0x3320646E, 0x79622D32, 0x6B206574)  # "expand 32-byte k"


def chacha20_block(key: bytes, counter: int, nonce: bytes) -> bytes:
    """RFC 8439 §2.3 — 64B keystream bloğu."""
    if len(key) != 32 or len(nonce) != 12:
        raise ValueError("key 32B, nonce 12B olmalı (RFC 8439)")
    st = list(_CONST) + list(struct.unpack("<8I", key)) + [counter & MASK32] + list(struct.unpack("<3I", nonce))
    w = st[:]
    for _ in range(10):                       # 20 tur = 10 × (sütun + köşegen)
        _qr(w, 0, 4, 8, 12);  _qr(w, 1, 5, 9, 13);  _qr(w, 2, 6, 10, 14); _qr(w, 3, 7, 11, 15)
        _qr(w, 0, 5, 10, 15); _qr(w, 1, 6, 11, 12); _qr(w, 2, 7, 8, 13);  _qr(w, 3, 4, 9, 14)
    return struct.pack("<16I", *[(w[i] + st[i]) & MASK32 for i in range(16)])


def chacha20(key: bytes, counter: int, nonce: bytes, data: bytes) -> bytes:
    """RFC 8439 §2.4 — akış şifresi."""
    out = bytearray()
    for i in range(0, len(data), 64):
        ks = chacha20_block(key, counter + i // 64, nonce)
        out += bytes(a ^ b for a, b in zip(data[i:i + 64], ks))
    return bytes(out)


# ── RFC 8439 §2.5: Poly1305 ──────────────────────────────────────────────────────────────
_P1305 = (1 << 130) - 5


def poly1305_mac(msg: bytes, key: bytes) -> bytes:
    """RFC 8439 §2.5.1 — tek kullanımlık anahtarla 16B etiket."""
    if len(key) != 32:
        raise ValueError("Poly1305 one-time key 32B olmalı")
    r = int.from_bytes(key[:16], "little") & 0x0FFFFFFC0FFFFFFC0FFFFFFC0FFFFFFF
    s = int.from_bytes(key[16:], "little")
    acc = 0
    for i in range(0, len(msg), 16):
        blk = msg[i:i + 16]
        acc = (acc + int.from_bytes(blk + b"\x01", "little")) % _P1305   # 0x01 append (§2.5.1)
        acc = acc * r % _P1305
    return ((acc + s) & ((1 << 128) - 1)).to_bytes(16, "little")


def poly1305_key_gen(key: bytes, nonce: bytes) -> bytes:
    """RFC 8439 §2.6 — counter = 0 bloğunun ilk 32 byte'ı."""
    return chacha20_block(key, 0, nonce)[:32]


# ── RFC 8439 §2.8: AEAD ──────────────────────────────────────────────────────────────────
def _pad16(b: bytes) -> bytes:
    return b"" if len(b) % 16 == 0 else b"\x00" * (16 - len(b) % 16)


def _mac_data(aad: bytes, ct: bytes) -> bytes:
    return aad + _pad16(aad) + ct + _pad16(ct) + struct.pack("<QQ", len(aad), len(ct))


def seal(key: bytes, nonce: bytes, plaintext: bytes, aad: bytes = b"") -> bytes:
    """RFC 8439 §2.8.1 — ciphertext ‖ tag(16B). Veri counter = 1'den başlar."""
    ct = chacha20(key, 1, nonce, plaintext)
    return ct + poly1305_mac(_mac_data(aad, ct), poly1305_key_gen(key, nonce))


def open_(key: bytes, nonce: bytes, ct_and_tag: bytes, aad: bytes = b"") -> bytes | None:
    """Doğrulama BAŞARISIZ ⇒ None (plaintext ASLA dönmez — authenticate-before-use)."""
    if len(ct_and_tag) < 16:
        return None
    ct, tag = ct_and_tag[:-16], ct_and_tag[-16:]
    want = poly1305_mac(_mac_data(aad, ct), poly1305_key_gen(key, nonce))
    if not _ct_eq(tag, want):
        return None
    return chacha20(key, 1, nonce, ct)


def _ct_eq(a: bytes, b: bytes) -> bool:
    if len(a) != len(b):
        return False
    d = 0
    for x, y in zip(a, b):
        d |= x ^ y
    return d == 0


def nonce96(counter: int) -> bytes:
    """noise-wire §4.5 / ccr §35.6 mapping:  nonce96 = 0x00×4 ‖ LE64(counter)."""
    if not 0 <= counter < 2 ** 64:
        raise ValueError("counter 64-bit aralığında olmalı")
    return b"\x00" * 4 + counter.to_bytes(8, "little")
