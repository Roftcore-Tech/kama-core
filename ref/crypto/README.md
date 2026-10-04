# `ref/crypto` — bağımsız kripto referansı (A2.1)

Rust implementasyonundan **BAĞIMSIZ**, yalnız Python standart kütüphanesiyle yazılmış referans.
Üçüncü-parti paket yok (R-7(d) kararı: `cryptography` paketi KULLANILMAZ).

## Neden var

Test vektörlerini Rust'ın kendisi üretirse, test "implementasyon kendi çıktısına eşit mi" sorusunu
sorar — spec'e uygunluğu değil. Bu ağaç o döngüyü kırar: vektörler burada, spec metninden ve
RFC'lerden yazılmış ayrı bir implementasyonla üretilir/doğrulanır.

## Ne ÜRETİM DEĞİL

Bu kod **sabit-zamanlı değildir**, üretim yolunda asla çalışmaz ve çalıştırılmamalıdır.
**Sabit-zamanlılık bu ağaç için gereksinim değil, açıkça KAPSAM DIŞIDIR** (sahip kararı, 2026-09-24): sabit-zamanlılık yalnız zamanlamayı ölçebilen bir saldırganın olduğu yerde gerekir; bu referans çevrimdışı, test anahtarlarıyla, ölçen kimsenin olmadığı ortamda koşar ve Python `int` ile zaten sabit-zamanlı yazılamaz. RFC'lerin kendi Python illüstrasyonları aynı uyarıyla gelir.
Üretim primitifleri: libsodium (ADR-03) + `x25519-dalek` / `ed25519-dalek` (pki §13 registry).

## Dosyalar

| dosya | ne | otorite |
|---|---|---|
| `kdf.py` | HKDF-BLAKE2s + §35.2.1 kanonik sarmalayıcı + D1–D7 türetmeleri | `specs/ccr-format-spec.md` §35.2.1 |
| `x25519.py` | X25519 Montgomery ladder | RFC 7748 |
| `ed25519.py` | Ed25519 PureEdDSA (katı doğrulama) | RFC 8032 |
| `chacha20poly1305.py` | ChaCha20-Poly1305 AEAD + `nonce96()` | RFC 8439 · `specs/pki-ca-ra-hsm-tdd.md:455` |
| `selftest.py` | dört süitin tek kapısı | — |
| `gen_vectors.py` | A2.2–A2.7 vektör üreticisi → `vectors/ccr-*`; `--check` diskteki ile karşılaştırır (CI) | ccr §5.1, §5.2, §35.2–§35.9 |

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
  düşük-mertebeli nokta (tüm-sıfır) reddi, ladder sabiti `a24 == (A-2)/4 == 121665`.
- **Ed25519**: RFC 8032 §7.1 TEST 1/2/3/SHA(abc)/1024 (pubkey + sign + verify), katılık
  negatifleri (S ≥ L malleability, bozuk R/S/mesaj/pubkey), taban noktasının mertebesi
  (`[L]B == nötr`, `[r]B == [r mod L]B`), ve **`[P, 2^255)` aralığının tamamı**: 19 y × 2 sign
  = 38 kanonik-olmayan kodlamanın 38/38'i hem `_decompress` hem `verify` tarafından reddediliyor.
- **ChaCha20-Poly1305**: RFC 8439 §2.3.2 / §2.4.2 / §2.5.2 / §2.6.2 / §2.8.2 / A.5,
  authenticate-before-use negatifleri (tag/ct/aad/nonce bozulması ⇒ `None`), `nonce96` mapping,
  32-bit blok sayacı taşmasının reddi (§2.8 tek `(key, nonce)` sınırı).

## İnceleme notları (kodda da yazılı, yeniden açılmasın)

| soru | cevap |
|---|---|
| `A24` neden 486662 değil? | `A = 486662` eğri parametresi (§4.1); `a24 = (A-2)/4 = 121665` ladder sabiti (§5). 486662 yazmak §5.2 vektörlerinin 8/8'ini düşürür. |
| `sign()` içindeki `r mod L` hatalı mı? | Hayır. RFC 8032 §5.1.6 adım 3 açıkça izin verir; `B`'nin mertebesi tam olarak `L`, kofaktör bileşeni yok. İki biçim byte-özdeş imza üretir. |
| bit-255 maskelemesi `y >= P` kapısını atlatır mı? | Hayır. Kapı `_recover_x`'te, her aritmetikten önce. 38/38 kodlama testle kilitli. |

## Mutasyon bataryası

Kapının vakum olmadığı 8 mutasyonla gösterildi (hepsi yakalanıyor): INV-KDF-3 karışıklığı,
HKDF Expand sayacı 1→0, `a24`→486662, Ed25519 `S < L` kapısının kaldırılması, kanoniklik
kapısının kaldırılması, AEAD veri counter'ı 1→0, taşma kapısının kaldırılması,
`compare_digest`'in devre dışı bırakılması.

## Vektörler (A2.2–A2.7)

`python3 ref/crypto/gen_vectors.py` altı suite üretir (68 vaka): `ccr-kdf-v1` (V-KDF-1..6) ·
`ccr-ssi1-v1` (üç cert_mode + negatifler) · `ccr-ssr1-v1` · `ccr-ssd1-v1` (22 B zarf; D-GAP-1
far-future kabul; boş batch) · `ccr-deprecated-neg-v1` · `ccr-ed25519-neg-v1` (ccr §5.2'nin dört ret
nedeni ayrı vakalarda). Beklenen byte'ların HİÇBİRİ Rust'tan üretilmedi. Üçüncü-kaynak teyitleri:
SSD1 AAD'si repodaki Rust golden'ıyla, SSD1 mühür vektörlerinin 5'i mevcut Rust `LegacySessionProtector`
ile bayt-özdeş çıktı (2026-10-04, geçici testle ölçüldü; kalıcı tüketici A5.2).

`ed25519.py` artık ccr §5.2 kipini tam uygular: small-order A ve R reddi dahil; `verify_reason`
ilk düşen kuralı kararlı bir token olarak döner.

## Bilinen sınır

SSI1'in `cert_field`'ı OPAK dolgudur: vektörler mührü ve AAD'yi sınar, sertifika doğrulamasını değil.
V-KDF-7 (üç kaynak eşitliği) bir vaka değil süreç şartıdır; Rust tarafı (A5.1) aynı değerleri üretince
sağlanır. Bu dizindeki hiçbir sayı Rust çıktısından alınmamıştır.
