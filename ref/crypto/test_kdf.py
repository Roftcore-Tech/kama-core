"""A2.1 kapısı: (1) HKDF yapısı RFC 5869 SHA-256 vektörleriyle, (2) BLAKE2s tarafı
INV-KDF-3 negatifiyle sınanır. Spec §35.2.1'in kendi talimatı: "HKDF implementasyonunun
kendisi ÖNCE RFC 5869'un SHA-256 vektörleriyle sınanır, SONRA BLAKE2s vektörleri üretilir."

Koşum: python3 ref/crypto/test_kdf.py   (stdlib-only, 3. parti paket yok)
"""

import hashlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kdf  # noqa: E402

FAILS = []


def _try(fn):
    """fn() beklenen bir ValueError/TypeError firlatiyor mu."""
    try:
        fn()
    except (ValueError, TypeError):
        return True
    return False


def check(name, got, want):
    ok = got == want
    if not ok:
        FAILS.append(f"{name}\n    beklenen: {want}\n    gelen   : {got}")
    print(f"  [{'OK ' if ok else 'FAIL'}] {name}")


def h(s):
    return bytes.fromhex(s)


# ── 1. RFC 5869 Appendix A: HKDF-SHA256 (yapı doğrulaması) ───────────────────────────────
# Kaynak: RFC 5869 A.1/A.2/A.3. A.4-A.7 SHA-1'dir; BLAKE2s ile ilgisiz, alınmadı.
RFC5869 = [
    # (ad, IKM, salt, info, L, PRK, OKM)
    ("A.1 basic",
     "0b" * 22, "000102030405060708090a0b0c", "f0f1f2f3f4f5f6f7f8f9", 42,
     "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5",
     "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"),
    ("A.2 long inputs",
     "".join(f"{i:02x}" for i in range(0x50)),
     "".join(f"{i:02x}" for i in range(0x60, 0xb0)),
     "".join(f"{i:02x}" for i in range(0xb0, 0x100)), 82,
     "06a6b88c5853361a06104c9ceb35b45cef760014904671014a193f40c15fc244",
     "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c"
     "59045a99cac7827271cb41c65e590e09da3275600c2f09b8367793a9aca3db71"
     "cc30c58179ec3e87c14c01d5c1f3434f1d87"),
    ("A.3 zero-len salt/info",
     "0b" * 22, "", "", 42,
     "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04",
     "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"),
]

print("1. RFC 5869 Appendix A — HKDF-SHA256 (yapı doğrulaması)")
for name, ikm, salt, info, ln, prk, okm in RFC5869:
    p = kdf.extract(h(salt), h(ikm), hashlib.sha256)
    check(f"{name} · PRK", p.hex(), prk)
    check(f"{name} · OKM", kdf.expand(p, h(info), ln, hashlib.sha256).hex(), okm)

# ── 2. HMAC-BLAKE2s: RFC 2104 yapısının bağımsız doğrulaması ─────────────────────────────
# hmac.new(...)'ı, HMAC'i ipad/opad'den ELLE kuran bir implementasyonla karşılaştır.
# Böylece "stdlib ne derse o" değil, RFC 2104 tanımı doğrulanmış olur.
def manual_hmac_blake2s(key: bytes, msg: bytes) -> bytes:
    B = 64  # BLAKE2s blok boyu (RFC 7693 §3.3) — HMAC için kritik sabit
    if len(key) > B:
        key = hashlib.blake2s(key).digest()
    key = key + b"\x00" * (B - len(key))
    ipad = bytes(k ^ 0x36 for k in key)
    opad = bytes(k ^ 0x5c for k in key)
    return hashlib.blake2s(opad + hashlib.blake2s(ipad + msg).digest()).digest()


print("\n2. HMAC-BLAKE2s — RFC 2104 yapısı (elle ipad/opad ↔ hmac.new)")
for kl in (0, 1, 32, 63, 64, 65, 200):
    for ml in (0, 1, 64, 65, 500):
        key = bytes((i * 7 + kl) & 0xFF for i in range(kl))
        msg = bytes((i * 13 + ml) & 0xFF for i in range(ml))
        if kdf.hmac_hash(key, msg) != manual_hmac_blake2s(key, msg):
            FAILS.append(f"HMAC-BLAKE2s uyuşmazlık key={kl}B msg={ml}B")
print(f"  [{'OK ' if not FAILS else 'FAIL'}] 35 (keylen × msglen) kombinasyonu — blok boyu 64B doğrulandı")
check("blok boyu sabiti", kdf.BLOCK_SIZE, 64)
check("HashLen", kdf.HASH_LEN, hashlib.blake2s().digest_size)

# ── 3. V-KDF-6 NEGATİF: HMAC-BLAKE2s ≠ BLAKE2s-keyed (INV-KDF-3) ─────────────────────────
print("\n3. V-KDF-6 negatifi — HMAC-BLAKE2s ≠ BLAKE2s-keyed (INV-KDF-3)")
_key = bytes(range(32))
_msg = b"kama-inv-kdf-3"
_hmac = kdf.hmac_hash(_key, _msg)
_native = hashlib.blake2s(_msg, key=_key).digest()
check("iki fonksiyon FARKLI çıktı veriyor", _hmac != _native, True)
print(f"       HMAC-BLAKE2s  = {_hmac.hex()}")
print(f"       BLAKE2s-keyed = {_native.hex()}   ← YANLIŞ olan; bu değer ASLA kullanılmaz")

# ── 4. §35.2.1 kanonik sarmalayıcı özellikleri ───────────────────────────────────────────
print("\n4. kdf(label, secret, context, L) — §35.2.1 sarmalayıcı özellikleri")
_s, _c = bytes(range(32)), b"ctx"
check("label = Extract salt (info DEĞİL)",
      kdf.kdf(b"LBL", _s, _c, 32),
      kdf.expand(kdf.extract(b"LBL", _s), _c, 32))
check("label değişince çıktı değişir (domain separation)",
      kdf.kdf(b"LBL", _s, _c, 32) != kdf.kdf(b"LBM", _s, _c, 32), True)
check("context değişince çıktı değişir",
      kdf.kdf(b"LBL", _s, b"x", 32) != kdf.kdf(b"LBL", _s, b"y", 32), True)
# §35.2.1 NOT: L=16, "L=32 üret + ilk 16B al" ile BYTE-ÖZDEŞ (T(1) = ilk 32B).
check("L=16 ≡ L=32'nin ilk 16 byte'ı (T(1) prefix özelliği)",
      kdf.kdf(b"LBL", _s, _c, 16), kdf.kdf(b"LBL", _s, _c, 32)[:16])
check("Expand L üst sınırı (255*HashLen) zorlanıyor",
      _try(lambda: kdf.expand(_s, b"", 255 * 32 + 1)), True)

# ── 5. D1–D7: yapısal özellikler (değer vektörleri A2.2'de Rust'tan bağımsız üretilecek) ──
print("\n5. D1–D7 türetme zinciri — yapısal özellikler")
ss_e, ss_s, ss_e2 = bytes([1]) * 32, bytes([2]) * 32, bytes([3]) * 64
sctx = bytes(range(100))
root = kdf.d1_init_root(ss_e, ss_s, sctx)
a2b0, b2a0 = kdf.d2_traffic_e0(root)
sid = kdf.d3_session_id_internal(root, sctx)
root1 = kdf.d4_epoch_1_root(root, ss_e2)
a2b1, b2a1 = kdf.d5_traffic_e1(root1)
k_init, k_resp = kdf.d6_k_init(root), kdf.d7_k_resp(root1)
check("D1 init_root 32B", len(root), 32)
check("D2 yön anahtarları FARKLI (A2B ≠ B2A)", a2b0 != b2a0, True)
check("D3 session_id_internal 16B", len(sid), 16)
check("D4 epoch_1_root ≠ init_root", root1 != root, True)
check("D5 epoch-1 anahtarları epoch-0'dan FARKLI (aynı label, farklı secret)",
      (a2b1 != a2b0) and (b2a1 != b2a0), True)
check("D6/D7 seal anahtarları trafik anahtarlarından ayrık (KARAR-1=A)",
      len({k_init, k_resp, a2b0, b2a0, a2b1, b2a1}), 6)
check("D1 ss_e/ss_s sırası bağlayıcı (swap ⇒ farklı root)",
      kdf.d1_init_root(ss_s, ss_e, sctx) != root, True)
check("uzunluk kapıları zorlanıyor", _try(lambda: kdf.d1_init_root(b"kisa", ss_s, sctx)), True)

print()
if FAILS:
    print(f"SONUÇ: {len(FAILS)} BAŞARISIZ\n" + "\n".join("  - " + f for f in FAILS))
    sys.exit(1)
print("SONUÇ: TÜM KONTROLLER GEÇTİ")
