"""X25519 — RFC 7748 §5 Montgomery ladder. BAĞIMSIZ referans (A2.1, R-7(d)).

Sabit-zamanlı DEĞİLDİR ve olması da gerekmez: bu kod yalnız test vektörü üretir/doğrular,
üretim yolunda ASLA çalışmaz. Üretim = `x25519-dalek` (pki §13 registry).
"""

P = 2 ** 255 - 19
A24 = 121665


def _decode_scalar(k: bytes) -> int:
    """RFC 7748 §5 decodeScalar25519 — clamping."""
    if len(k) != 32:
        raise ValueError("skaler 32 byte olmalı")
    k = bytearray(k)
    k[0] &= 248
    k[31] &= 127
    k[31] |= 64
    return int.from_bytes(k, "little")


def _decode_u(u: bytes) -> int:
    """RFC 7748 §5: en yüksek bit MASKELENİR (yok sayılır), sonra mod p."""
    if len(u) != 32:
        raise ValueError("u-koordinatı 32 byte olmalı")
    u = bytearray(u)
    u[31] &= 127
    return int.from_bytes(u, "little") % P


def _cswap(swap: int, a: int, b: int) -> tuple[int, int]:
    dummy = (-swap) & (a ^ b) & (2 ** 256 - 1)
    return a ^ dummy, b ^ dummy


def x25519(k: bytes, u: bytes) -> bytes:
    """RFC 7748 §5 X25519(k, u) -> 32B."""
    kk = _decode_scalar(k)
    x1 = _decode_u(u)
    x2, z2, x3, z3, swap = 1, 0, x1, 1, 0

    for t in range(254, -1, -1):
        kt = (kk >> t) & 1
        swap ^= kt
        x2, x3 = _cswap(swap, x2, x3)
        z2, z3 = _cswap(swap, z2, z3)
        swap = kt

        a = (x2 + z2) % P
        aa = a * a % P
        b = (x2 - z2) % P
        bb = b * b % P
        e = (aa - bb) % P
        c = (x3 + z3) % P
        d = (x3 - z3) % P
        da = d * a % P
        cb = c * b % P
        x3 = (da + cb) % P
        x3 = x3 * x3 % P
        z3 = (da - cb) % P
        z3 = x1 * z3 % P * z3 % P
        x2 = aa * bb % P
        z2 = e * (aa + A24 * e) % P

    x2, x3 = _cswap(swap, x2, x3)
    z2, z3 = _cswap(swap, z2, z3)
    return (x2 * pow(z2, P - 2, P) % P).to_bytes(32, "little")


BASE_U = (9).to_bytes(32, "little")


def public_key(private: bytes) -> bytes:
    """RFC 7748 §6.1: pub = X25519(sk, 9)."""
    return x25519(private, BASE_U)


def is_all_zero(shared: bytes) -> bool:
    """RFC 7748 §6.1 kontrolü: tüm-sıfır çıktı düşük-mertebeli nokta ⇒ REDDET."""
    return all(b == 0 for b in shared)
