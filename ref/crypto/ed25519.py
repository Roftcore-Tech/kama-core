"""Ed25519 (PureEdDSA) — RFC 8032 §5.1. BAĞIMSIZ referans (A2.1, R-7(d)).

Sabit-zamanlı DEĞİL; yalnız test vektörü üretir/doğrular. Üretim = `ed25519-dalek` + libsodium.

DOĞRULAMA KATILIĞI (RFC 8032 §5.1.7 + §8.4): bu referans
  * S >= L olan imzaları REDDEDER (malleability — yalnız kanonik S kabul),
  * kanonik olmayan nokta kodlamalarını REDDEDER,
  * denklemi KOFAKTÖRSÜZ kontrol eder: [S]B == R + [k]A.
Bu, `ed25519-dalek`'in varsayılan katılığıyla aynı hizadadır. libsodium
`crypto_sign_verify_detached` de S kanonikliğini zorlar.
"""

import hashlib

P = 2 ** 255 - 19
L = 2 ** 252 + 27742317777372353535851937790883648493
D = (-121665 * pow(121666, P - 2, P)) % P
I = pow(2, (P - 1) // 4, P)          # sqrt(-1)


def _sha512(b: bytes) -> bytes:
    return hashlib.sha512(b).digest()


# ── Genişletilmiş (extended) koordinatlar: (X, Y, Z, T), x = X/Z, y = Y/Z, xy = T/Z ──────
def _add(p, q):
    x1, y1, z1, t1 = p
    x2, y2, z2, t2 = q
    a = (y1 - x1) * (y2 - x2) % P
    b = (y1 + x1) * (y2 + x2) % P
    c = t1 * 2 * D * t2 % P
    dd = z1 * 2 * z2 % P
    e, f, g, hh = b - a, dd - c, dd + c, b + a
    return (e * f % P, g * hh % P, f * g % P, e * hh % P)


def _double(p):
    return _add(p, p)


def _mul(p, n):
    q = (0, 1, 1, 0)          # neutral
    while n > 0:
        if n & 1:
            q = _add(q, p)
        p = _double(p)
        n >>= 1
    return q


def _recover_x(y: int, sign: int):
    """RFC 8032 §5.1.3 — y'den x. Kanonik olmayan/çözümsüz kodlamada None."""
    if y >= P:
        return None                       # kanonik olmayan y (§5.1.3: y < p ZORUNLU)
    yy = y * y % P
    u = (yy - 1) % P
    v = (D * yy + 1) % P
    xx = u * pow(v, P - 2, P) % P
    x = pow(xx, (P + 3) // 8, P)
    if x * x % P != xx:
        x = x * I % P
    if x * x % P != xx:
        return None
    if x == 0 and sign:
        return None                       # x=0 iken sign biti 1 ⇒ kanonik değil (§5.1.3)
    if x & 1 != sign:
        x = P - x
    return x


def _decompress(s: bytes):
    if len(s) != 32:
        return None
    y = int.from_bytes(s, "little")
    sign = (y >> 255) & 1
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    return None if x is None else (x, y, 1, x * y % P)


def _compress(pt) -> bytes:
    x, y, z, _ = pt
    zi = pow(z, P - 2, P)
    x, y = x * zi % P, y * zi % P
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


_BY = 4 * pow(5, P - 2, P) % P
B = (_recover_x(_BY, 0), _BY, 1, _recover_x(_BY, 0) * _BY % P)


def _eq(p, q) -> bool:
    x1, y1, z1, _ = p
    x2, y2, z2, _ = q
    return (x1 * z2 - x2 * z1) % P == 0 and (y1 * z2 - y2 * z1) % P == 0


def _secret_expand(sk: bytes):
    if len(sk) != 32:
        raise ValueError("Ed25519 özel anahtar 32 byte olmalı")
    h = _sha512(sk)
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8            # clamp: alt 3 bit 0, bit254 set, bit255 clear
    a |= 1 << 254
    return a, h[32:]


def public_key(sk: bytes) -> bytes:
    a, _ = _secret_expand(sk)
    return _compress(_mul(B, a))


def sign(sk: bytes, msg: bytes) -> bytes:
    a, prefix = _secret_expand(sk)
    A = _compress(_mul(B, a))
    r = int.from_bytes(_sha512(prefix + msg), "little") % L
    R = _compress(_mul(B, r))
    k = int.from_bytes(_sha512(R + A + msg), "little") % L
    return R + ((r + k * a) % L).to_bytes(32, "little")


def verify(pk: bytes, msg: bytes, sig: bytes) -> bool:
    """RFC 8032 §5.1.7 + §8.4 katı doğrulama. Şüphede REDDET."""
    if len(sig) != 64 or len(pk) != 32:
        return False
    Rs, Ss = sig[:32], sig[32:]
    s = int.from_bytes(Ss, "little")
    if s >= L:
        return False                      # malleability: yalnız kanonik S kabul
    A = _decompress(pk)
    R = _decompress(Rs)
    if A is None or R is None:
        return False                      # kanonik olmayan kodlama
    k = int.from_bytes(_sha512(Rs + pk + msg), "little") % L
    return _eq(_mul(B, s), _add(R, _mul(A, k)))   # kofaktörsüz denklem
