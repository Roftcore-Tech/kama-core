# ╔═══════════════════════════════════════════════════════════════════════════════╗
# ║ KAPSAM DIŞI: SABİT-ZAMANLILIK — bu dosya ÜRETİM KODU DEĞİLDİR.                ║
# ║ Yalnız test vektörü üretir/doğrular: çevrimdışı, test anahtarlarıyla, zamanlama ║
# ║ ölçebilen bir saldırganın OLMADIĞI ortamda. Sabit-zamanlılık ancak zamanlamanın ║
# ║ ölçülebildiği yerde gereklidir; Python int aritmetiğiyle zaten yazılamaz.        ║
# ║ Üretim primitifleri: libsodium (ADR-03) + dalek (pki §13). Bu bir gereksinim     ║
# ║ değil, açık kapsam-dışı ilanıdır (sahip kararı 2026-09-24; RFC'lerin kendi       ║
# ║ Python illüstrasyonlarıyla aynı statü).                                          ║
# ╚═══════════════════════════════════════════════════════════════════════════════╝
"""HKDF-BLAKE2s — Kama'nın kanonik anahtar türetmesinin BAĞIMSIZ referansı (A2.1).

Otorite: `specs/ccr-format-spec.md` §35.2.1 (NORMATİF). Bu dosya Rust implementasyonundan
BAĞIMSIZDIR: yalnız Python standart kütüphanesi kullanılır, Rust kodu okunmadan spec'ten yazıldı.
Amaç, Rust'ın kendi ürettiği beklentiyle test edilmesini engellemek (A2.1 gerekçesi).

KRİTİK (§35.2.1, INV-KDF-3): HMAC-BLAKE2s-256 ≠ BLAKE2s-keyed.
  * DOĞRU : hmac.new(key, msg, hashlib.blake2s)      — RFC 2104 HMAC, hash = BLAKE2s-256
  * YANLIŞ: hashlib.blake2s(msg, key=key)            — BLAKE2s'in NATIVE keyed modu
Bu iki fonksiyon FARKLIDIR; sessiz interop kırılmasının en yaygın kaynağıdır (V-KDF-6 negatifi).
"""

import hashlib
import hmac as _hmac

HASH = hashlib.blake2s          # BLAKE2s-256
HASH_LEN = 32                   # §35.2.1 HashLen
BLOCK_SIZE = 64                 # BLAKE2s blok boyu (RFC 7693) — HMAC için gerekli


def hmac_hash(key: bytes, msg: bytes, hash_ctor=HASH) -> bytes:
    """RFC 2104 HMAC. `hashlib.blake2s`'in NATIVE keyed modu DEĞİL (INV-KDF-3)."""
    return _hmac.new(key, msg, hash_ctor).digest()


def extract(salt: bytes, ikm: bytes, hash_ctor=HASH) -> bytes:
    """RFC 5869 §2.2:  PRK = HMAC-Hash(key = salt, msg = IKM)."""
    return hmac_hash(salt, ikm, hash_ctor)


def expand(prk: bytes, info: bytes, length: int, hash_ctor=HASH) -> bytes:
    """RFC 5869 §2.3: T(i) zinciri, sayaç 0x01'den başlar. L <= 255*HashLen."""
    hash_len = hash_ctor().digest_size
    if length > 255 * hash_len:
        raise ValueError(f"L={length} > 255*HashLen={255 * hash_len} (RFC 5869 §2.3)")
    okm = b""
    t = b""
    counter = 1
    while len(okm) < length:
        t = hmac_hash(prk, t + info + bytes([counter]), hash_ctor)
        okm += t
        counter += 1
    return okm[:length]


def kdf(label: bytes, secret: bytes, context: bytes, length: int) -> bytes:
    """§35.2.1 KANONİK SARMALAYICI — belgedeki HER `HKDF(label, secret, context)` bu demektir.

        PRK = Extract(salt = ASCII(label), IKM = secret)
        OKM = Expand(PRK, info = context, L = length)

    INV-KDF-1: başka çağrım biçimi (Expand-only, salt=zeros, label'ı info'ya taşıma) YASAK.
    `label` ASCII byte'larıdır: NUL-terminator YOK, padding YOK.
    """
    if not isinstance(label, (bytes, bytearray)):
        raise TypeError("label bytes olmalı (ASCII); str verilmez")
    return expand(extract(bytes(label), bytes(secret)), bytes(context), length)


# ── §35.2.1 TÜRETME TABLOSU: D1–D7 (label'lar INV-KDF-2 ile IMMUTABLE) ────────────────────
L_INIT      = b"SR-SESSION-INIT-V3"        # D1
L_TRAFFIC   = b"SR-SESSION-TRAFFIC-V3"     # D2 ve D5 (AYNI label; PRK farklı ⇒ key farklı)
L_SESSION_ID= b"SR-SESSION-ID-V3"          # D3
L_EPOCH     = b"SR-SESSION-EPOCH-V3"       # D4
L_INIT_DATA = b"SR-SESSION-INIT-DATA-V3"   # D6
L_RESP_DATA = b"SR-SESSION-RESP-DATA-V3"   # D7

DIR_A2B = b"\x01"    # initiator → responder (§35.9 direction kodlamasıyla AYNI)
DIR_B2A = b"\x02"


def d1_init_root(ss_e: bytes, ss_s: bytes, session_context_v1: bytes) -> bytes:
    """D1: secret = ss_e(32) || ss_s(32) = 64B; context = session_context_v1 (100B); L=32."""
    _need(ss_e, 32, "ss_e"); _need(ss_s, 32, "ss_s"); _need(session_context_v1, 100, "session_context_v1")
    return kdf(L_INIT, ss_e + ss_s, session_context_v1, 32)


def d2_traffic_e0(init_root: bytes) -> tuple[bytes, bytes]:
    """D2: (k_A2B_e0, k_B2A_e0)."""
    _need(init_root, 32, "init_root")
    return kdf(L_TRAFFIC, init_root, DIR_A2B, 32), kdf(L_TRAFFIC, init_root, DIR_B2A, 32)


def d3_session_id_internal(init_root: bytes, session_context_v1: bytes) -> bytes:
    """D3: L=16. NOT: L=16, "L=32 üret + ilk 16B" ile BYTE-ÖZDEŞtir (T(1) prefix özelliği)."""
    _need(init_root, 32, "init_root"); _need(session_context_v1, 100, "session_context_v1")
    return kdf(L_SESSION_ID, init_root, session_context_v1, 16)


def d4_epoch_1_root(init_root: bytes, ss_e2: bytes) -> bytes:
    """D4: secret = init_root(32) || ss_e2(64) = 96B; context = u16_be(1); L=32."""
    _need(init_root, 32, "init_root"); _need(ss_e2, 64, "ss_e2")
    return kdf(L_EPOCH, init_root + ss_e2, b"\x00\x01", 32)


def d5_traffic_e1(epoch_1_root: bytes) -> tuple[bytes, bytes]:
    """D5: (k_A2B_e1, k_B2A_e1) — D2 ile aynı label, farklı secret."""
    _need(epoch_1_root, 32, "epoch_1_root")
    return kdf(L_TRAFFIC, epoch_1_root, DIR_A2B, 32), kdf(L_TRAFFIC, epoch_1_root, DIR_B2A, 32)


def d6_k_init(init_root: bytes) -> bytes:
    """D6: SESSION_INIT seal key (KARAR-1 = A: ayrı domain-separated anahtar)."""
    _need(init_root, 32, "init_root")
    return kdf(L_INIT_DATA, init_root, DIR_A2B, 32)


def d7_k_resp(epoch_1_root: bytes) -> bytes:
    """D7: SESSION_INIT_RESPONSE seal key."""
    _need(epoch_1_root, 32, "epoch_1_root")
    return kdf(L_RESP_DATA, epoch_1_root, DIR_B2A, 32)


def _need(b: bytes, n: int, name: str) -> None:
    if len(b) != n:
        raise ValueError(f"{name}: {n} byte olmalı, {len(b)} verildi (§35.2.1)")
