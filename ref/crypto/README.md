# `ref/crypto` — bağımsız kripto referansı (A2.1)

Rust implementasyonundan **BAĞIMSIZ**, yalnız Python standart kütüphanesiyle yazılmış referans.
Üçüncü-parti paket yok (R-7(d) kararı: `cryptography` paketi KULLANILMAZ).

## Neden var

Test vektörlerini Rust'ın kendisi üretirse, test "implementasyon kendi çıktısına eşit mi" sorusunu
sorar — spec'e uygunluğu değil. Bu ağaç o döngüyü kırar: vektörler burada, spec metninden ve
RFC'lerden yazılmış ayrı bir implementasyonla üretilir/doğrulanır.

## Ne ÜRETİM DEĞİL

Bu kod **sabit-zamanlı değildir**, üretim yolunda asla çalışmaz ve çalıştırılmamalıdır.
Üretim primitifleri: libsodium (ADR-03) + `x25519-dalek` / `ed25519-dalek` (pki §13 registry).

## Dosyalar

| dosya | ne | otorite |
|---|---|---|
| `kdf.py` | HKDF-BLAKE2s + §35.2.1 kanonik sarmalayıcı + D1–D7 türetmeleri | `specs/ccr-format-spec.md` §35.2.1 |
| `x25519.py` | X25519 Montgomery ladder | RFC 7748 |
| `ed25519.py` | Ed25519 PureEdDSA (katı doğrulama) | RFC 8032 |
| `chacha20poly1305.py` | ChaCha20-Poly1305 AEAD + `nonce96()` | RFC 8439 · `specs/pki-ca-ra-hsm-tdd.md:455` |
| `selftest.py` | dört süitin tek kapısı | — |

## Koşum

```
python3 ref/crypto/selftest.py      # hepsi;  çıkış 0 = geçti
```

## Doğrulanan vektörler

- **HKDF**: RFC 5869 A.1/A.2/A.3 (SHA-256 ile **yapı** doğrulaması — §35.2.1'in açık talimatı),
  HMAC-BLAKE2s'in RFC 2104 yapısı elle kurulmuş ipad/opad ile 35 kombinasyonda çapraz kontrol,
  **V-KDF-6 negatifi** (HMAC-BLAKE2s ≠ BLAKE2s-keyed, INV-KDF-3), §35.2.1 sarmalayıcı özellikleri,
  D1–D7 zincirinin yapısal ayrımları.
- **X25519**: RFC 7748 §5.2 tek adım + yüksek-bit maskeleme + iterasyon 1 ve 1000, §6.1 DH,
  düşük-mertebeli nokta (tüm-sıfır) reddi.
- **Ed25519**: RFC 8032 §7.1 TEST 1/2/3/SHA(abc)/1024 (pubkey + sign + verify), katılık
  negatifleri (S ≥ L malleability, kanonik olmayan nokta, bozuk R/S/mesaj/pubkey).
- **ChaCha20-Poly1305**: RFC 8439 §2.3.2 / §2.4.2 / §2.5.2 / §2.6.2 / §2.8.2 / A.5,
  authenticate-before-use negatifleri (tag/ct/aad/nonce bozulması ⇒ `None`), `nonce96` mapping.

## Bilinen sınır

D1–D7 için burada **değer** vektörü yok, yalnız yapısal özellikler var. V-KDF-1..7'nin exact
byte değerleri A2.2'de bu referanstan üretilip `vectors/` ağacına yazılacak; Rust tarafı onlara
karşı koşacak. Bu dosyadaki hiçbir sayı Rust çıktısından alınmamıştır.
