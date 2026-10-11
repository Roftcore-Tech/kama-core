# PROVISIONING-FORMAT-v0 — `kama-provision` çıktı biçimi (JSON v0)

| Alan | Değer |
|---|---|
| Karar sahibi | Abkarada |
| Kart | K4G-12 (`KAMA-PLAN-4GUN-APP-KRIPTO` v1.1) — icra Claude Code L-PROV |
| Dayanak | Plan D4 (v1.1), D9 (v1.1), D11 · PoC-KAPSAM-001 §2 "Provisioning", "Anahtarlar", §5 (EK-1) · ccr §6.3, §8.1, §13.1–§13.5, §35.2, §35.4 KAPALI-SİSTEM KİPİ (PR #101) |
| Uygulama | `crates/kama-provision` — kütüphane `kama_provision::format` (tipler, yükleyici), `kama_provision::cert` (CompactDeviceCertV1), ikili `kama-provision` (CLI) |
| Tüketici | K4G-11 `crates/kama-node` (bileşim kökü) · K4G-05 INT-02 (anahtar kaynağı) · K4G-15 (örnek `device-alice.json`) |
| Durum | **v0 TASLAK** — biçim (alanlar, kodlamalar, dosya kuralları) bu belgededir; §4.4 ve §4.5'teki iki PoC tanımı **DUR-K4G-12** cevabını bekler |
| Tarih | 2026-10-11 |

---

## 0. Kapsam

`kama-provision` provisioning otoritesinin bilgisayarında (laptop) **çevrim dışı** koşan bir CLI'dır. Tek
komutu vardır: `init`. Bir test kökü (Ed25519) ve cihaz başına bir yapılandırma dosyası üretir. Cihaz
dosyası telefona aktarılır; uygulama onu uygulamaya özel dizine (`noBackupFilesDir`) yazar (PoC-KAPSAM-001
§2 "Provisioning").

Güven modeli ccr §35.4 KAPALI-SİSTEM KİPİ'dir: cihaz dosyasındaki eş listesi **önceden doğrulanmış eş
dizini**dir; çalışma zamanında zincir ve iptal doğrulaması **yapılmaz**. Güvenlik iddiası ("listede olmayan
hiçbir oturum açamaz") bu listeye dayanır.

**Kapsam dışı (kart SINIR):** QR, enrollment, PKI, iptal.

## 1. Kurallar (D4, D9)

**(a) D4:** Her `kama-provision init` koşusu yeni anahtar üretir; yeniden kurulum = yeniden provisioning = yeni kimlik. Kalıcı durum kaybından sonra aynı eski kimliğin yeniden kullanılması desteklenmez.

**(b) D9:** `root.json` cihaz config'ine, Git'e, release paketine ve `.so` zip'ine girmez.

**Beyanlı PoC sınırı (D4).** Uygulamanın kalıcı durumu (`msg_id` / `stream_id` / `transfer_id`
sayaçları, `init_counter`, dedup penceresi, outbox) tamamen kaybolur (veri silme, yeniden kurulum) ve
**aynı eski kimlik** aynı provisioning dosyasıyla yeniden kullanılırsa güvenli devam **garanti edilmez**
(`msg_id` tekrarı ve yanlış "teslim edildi" mümkündür; yerel tespit imkânsızdır). Bu durumda **yeniden
provisioning zorunludur**: `kama-provision init` her koşuda yeni anahtar çiftleri üretir → yeni
`dest_hash` → alıcıda yeni eş. Aynı sınır PoC-KAPSAM-001 §5'te (EK-1) beyanlıdır; AMP §3'teki `msg_id`
kalıcılığı cümlesi (K4G-07) bu bölüme atıf yapar.

**Kök gizli anahtarı (D9).** `root.json` yalnız provisioning otoritesinin eseridir. Cihaz dosyası kökün
yalnız **açık** anahtarını (`root_pub`) taşır; kök gizli anahtarı cihaz dosyasına hiçbir kod yolunda
yazılmaz (biçim tipinde böyle bir alan yoktur; testi §6).

**Cihaz gizli anahtarları.** Cihaz dosyası cihazın kendi X25519 ve Ed25519 gizli anahtarlarını düz JSON'da
taşır. Bu **kabul edilmiş PoC riskidir** (D9; PoC-KAPSAM-001 §5 EK-1/EK-2: at-rest koruması yok,
`noBackupFilesDir` yalnız yedeklemeyi sınırlar); büyütülmez.

## 2. CLI

```
kama-provision init --out <dir> --devices <ad>,<ad>[,…] [--hub <host:port>] [--direct <ad>=<ip:port>[,…]]
kama-provision --help
```

| Argüman | Kural |
|---|---|
| `--out <dir>` | Zorunlu. Dizin **yoksa** oluşturulur (Unix: `0700`; üst dizin var olmalı). **Varsa boş** bir dizin olmalıdır; dolu dizin ve sembolik bağ reddedilir (önceki bir provisioning'in üstüne yazılmaz, iki kimlik kümesi karışmaz). |
| `--devices` | Zorunlu. Virgülle ayrılmış **2–64** cihaz adı; ad `[a-z0-9][a-z0-9_-]{0,31}`; tekrar eden ad reddedilir. |
| `--hub <host:port>` | İsteğe bağlı. `host` boş olamaz ve boşluk içeremez, `port` 1–65535. Hub anahtarı CLI'da yoktur (§4.7). |
| `--direct` | İsteğe bağlı. `ad=ip:port` çiftleri (direct-mode). `ad` `--devices`'ta olmalı ve bir kez geçmeli; adres IP sabiti olmalı (`127.0.0.1:47001`, `[::1]:47001`; ana makine adı kabul edilmez), `port` 1–65535. |

Her argüman en çok bir kez verilir; bilinmeyen argüman reddedilir. Argümanlar `std::env::args_os` ile
ayrıştırılır (clap yok, D11); `--out` UTF-8 olmayan bir yol olabilir.

**Çıktı dizini ve dosya güvenliği (D9):**

1. Önce `.gitignore` yazılır, içeriği tek satır: `*` — dizin bir Git çalışma ağacının içindeyse hiçbir
   dosyası (`.gitignore` dahil) izlenmez.
2. Sonra `device-<ad>.json` dosyaları, en son `root.json`.
3. Her dosya **yeni** oluşturulur (`O_EXCL`; var olan dosyanın üstüne yazılmaz) ve Unix'te izni `0600`'dür
   (umask'tan bağımsız; oluşturulduktan sonra açıkça `0600` yapılır).
4. Bir dosya yazılamazsa bu koşunun oluşturduğu bütün dosyalar (ve dizini bu koşu oluşturduysa dizin)
   silinir; yarım bir anahtar kümesi bırakılmaz.
5. Windows ve diğer Unix dışı platformlarda izinler uygulanmaz; CLI standart hataya uyarı basar.
6. Standart çıktıya yalnız yazılan dosyaların yolları ve D9 hatırlatması basılır; **hiçbir anahtar,
   `dest_hash` ya da sertifika basılmaz**.

**Çıkış kodu:** `0` başarı · `1` çalışma hatası (dolu çıktı dizini, G/Ç, entropi, saat, öz-denetim) ·
`2` kullanım hatası. `2`'de ve `1`'in G/Ç dışı nedenlerinde hiçbir dosya yazılmaz; G/Ç hatasında bu
koşunun yazdıkları silinir (madde 4).

## 3. Dosyalar

```
<out>/
  .gitignore            "*"
  device-<ad>.json      cihaz başına — cihaza aktarılır
  root.json             yalnız provisioning otoritesi — cihaza aktarılmaz (D9)
```

### 3.1 `root.json`

```json
{
  "format_version": 0,
  "created_at_unix": 1791676800,
  "ed25519": {
    "priv": "<64 hex>",
    "pub": "<64 hex>"
  }
}
```

| Alan | Tip | Anlam |
|---|---|---|
| `format_version` | sayı | `0` |
| `created_at_unix` | sayı (u64) | Oluşturma zamanı, Unix saniyesi (UTC). Cihaz sertifikalarının `not_after`'ı buradan türer (§4.3). |
| `ed25519.priv` | 32 B, hex | Test kökünün Ed25519 gizli anahtarı (RFC 8032 §5.1.5 tohumu). |
| `ed25519.pub` | 32 B, hex | Test kökünün Ed25519 açık anahtarı. Cihaz dosyalarındaki `root_pub` budur. |

### 3.2 `device-<ad>.json`

```json
{
  "format_version": 0,
  "device_id": "alice",
  "dest_hash": "<64 hex>",
  "x25519_static": {
    "priv": "<64 hex>",
    "pub": "<64 hex>"
  },
  "ed25519": {
    "priv": "<64 hex>",
    "pub": "<64 hex>"
  },
  "cert": "<204 karakter base64>",
  "root_pub": "<64 hex>",
  "peers": [
    {
      "name": "bob",
      "dest_hash": "<64 hex>",
      "x25519_pub": "<64 hex>",
      "cert": "<204 karakter base64>"
    }
  ],
  "hub": null,
  "direct_addrs": {
    "alice": "127.0.0.1:47001",
    "bob": "127.0.0.1:47002"
  }
}
```

| Alan | Tip | Anlam |
|---|---|---|
| `format_version` | sayı | `0`. Başka değer reddedilir (v0'da göç yok). |
| `device_id` | dizge | Cihaz adı (`--devices`'taki ad); dosya adındaki `<ad>` ile ve diğer cihazların `peers[].name` alanıyla aynıdır. |
| `dest_hash` | 32 B, hex | Cihazın yönlendirme kimliği (§4.4). |
| `x25519_static.priv` / `.pub` | 32 B, hex | Statik X25519 (ccr §13.7). `pub = X25519(priv, 9)`. |
| `ed25519.priv` / `.pub` | 32 B, hex | Cihazın Ed25519 anahtarı (RFC 8032 tohumu / açık anahtarı); sertifikadaki `device_ed25519_pub`. |
| `cert` | 152 B, base64 | Cihazın CompactDeviceCertV1'i, test köküyle imzalı (§4.3). |
| `root_pub` | 32 B, hex | Test kökünün **yalnız açık** anahtarı. |
| `peers[]` | dizi (1–63) | Bu cihaz **dışındaki** bütün cihazlar, `--devices` sırasıyla. |
| `peers[].name` | dizge | Eşin `device_id`'si. |
| `peers[].dest_hash` | 32 B, hex | Eşin `dest_hash`'i. |
| `peers[].x25519_pub` | 32 B, hex | Eşin statik X25519 açık anahtarı. |
| `peers[].cert` | 152 B, base64 | Eşin CompactDeviceCertV1'i. |
| `hub` | `null` ya da `{addr, x25519_pub}` | `--hub` verilmediyse `null`. Verildiyse `addr` = `host:port`, `x25519_pub` = `null` (§4.7). |
| `direct_addrs` | nesne `{ad: "ip:port"}` | `--direct` çiftleri; boş nesne olabilir. Cihazın kendi adı varsa kendi dinleme adresi, eş adları eşlerin adresidir. |

## 4. Kodlamalar ve türetmeler

### 4.1 Kodlamalar

- **hex:** yalnız küçük harf `[0-9a-f]`, `0x` öneki yok; 32 B alan tam 64 karakter. Büyük harf, boşluk ve
  yanlış uzunluk reddedilir.
- **base64:** RFC 4648 §4 standart alfabe (`A–Z a–z 0–9 + /`), `=` dolgulu, kanonik (kullanılmayan son
  bitler sıfır). 152 B sertifika tam 204 karakterdir. Satır sonu ve boşluk reddedilir.
- Tüm nesnelerde bilinmeyen alan reddedilir.

### 4.2 Anahtarlar

Bütün gizli anahtarlar işletim sisteminin CSPRNG'inden alınır (`kama_noise_wire::entropy::OsEntropy`,
`rand_core::OsRng` → `getrandom`); her koşuda yenidir (D4). Tüm-sıfır bir tohum entropi arızası sayılır ve
koşu durur.

- **X25519 statik:** `priv` = 32 rastgele bayt, kenetlenmemiş (clamping kullanımda yapılır, RFC 7748 §5).
- **Ed25519 (cihaz ve kök):** `priv` = RFC 8032 §5.1.5 32 B tohum; `pub` RFC 8032'ye göre türetilir.

### 4.3 `cert` — CompactDeviceCertV1 (ccr §13.1 / §13.2)

```
CompactDeviceCertV1 (152 B, sabit; uzunluk öneki / magic / sürüm YOK)
  [  0.. 32) device_ed25519_pub   = ed25519.pub
  [ 32.. 64) device_x25519_pub    = x25519_static.pub
  [ 64.. 80) issuer_key_id        = SHA-256(0x01 ‖ root_pub)[0..16]
  [ 80.. 88) not_after            = u64_be(created_at_unix + 7 776 000)
  [ 88..152) issuer_signature     = Ed25519_Sign(root_priv, compact_device_cert_tbs)

compact_device_cert_tbs (104 B) =
  domain_sep("SR_CDC_V1") (16 B, §4.5) ‖ device_ed25519_pub ‖ device_x25519_pub ‖ issuer_key_id ‖ not_after
```

- **İmzalayan = test kökü** (D9; PoC-KAPSAM-001 §2 "Anahtarlar"). PoC'de Identity/User Cert katmanı
  yoktur; ccr §13.1'in `sk_user_identity`'si yerine kökün gizli anahtarı imzalar.
- **`issuer_key_id`** = imzalayanın `subject_key_id`'sinin ilk 16 baytı (ccr §13.4); `subject_key_id` =
  `SHA-256(spki_encoding(0x01, root_pub))`, `spki_encoding` = `alg (u8) ‖ key (32 B)` (ccr §8.1; `0x01` =
  Ed25519, ccr §6.3).
- **`not_after`** = oluşturma + 90 gün (ccr §13.1 "max +90 gün"). Kapalı-sistem kipinde çalışma zamanında
  denetlenmez; süresi geçen kurulum yeniden provisioning'le yenilenir (D4).
- **`cert_ref`** = `SHA-256(cert)` (ccr §35.2 `sender_cert_ref`; `kama_noise_wire::handshake::cert_ref_of`).
  Dosyada saklanmaz; tüketici hesaplar. INIT `cert_mode = ref` ile gider (D9): INIT'te sertifikanın
  kendisi değil `cert_ref` (32 B) taşınır, yanıtlayan göndereni eş dizininde `cert_ref` ile bulur.
- İmza çalışma zamanında doğrulanmaz (ccr §35.4 KAPALI-SİSTEM KİPİ). `kama_provision::cert` doğrulama
  yardımcısını (`verify_compact_device_cert`, katı kip `kama_noise_wire::ed25519_strict::verify_strict`)
  testler ve çevrim dışı araçlar için sunar.
- **Kodlayıcının yeri:** kartın GİRDİ satırındaki "mevcut kodlayıcı" `kama-noise-wire`'da yoktur
  (`handshake.rs` sertifika alanını opak bayt dizisi olarak taşır: `COMPACT_DEVICE_CERT_LEN = 152`,
  `cert_ref_of`). Kodlayıcı ccr §13.1 / §13.2'den `kama_provision::cert`'te yazılmıştır (DUR-K4G-12).

### 4.4 `dest_hash` — ÖNERİ, DUR-K4G-12 (a)

> **AÇIK — Abkarada kararı bekleniyor.** Kartın 3. maddesi "`dest_hash` mevcut türetme fonksiyonuyla
> doğrulanır" der; kodda böyle bir fonksiyon yoktur (Rust'ta `dest_hash` hep girdi olarak verilir; tek
> türetme Python referansında, `ref/crypto/gen_vectors.py`: `SHA-256(SHA-256(user_identity_ed25519_pub))`).

**Öneri (uygulanan):** `dest_hash = SHA-256(SHA-256(ed25519.pub))` — CAD-16 formülü (ccr §13.5 /
identity §14) cihazın Ed25519 açık anahtarına uygulanır. Gerekçe: PoC'de Identity/User Cert katmanı yoktur
ve çoklu cihaz kapsam dışıdır (PoC-KAPSAM-001 §3); biçimde cihaz başına tek Ed25519 anahtarı vardır ve
"kullanıcı kimlik anahtarı" rolünü o üstlenir.

**Çelişki:** ccr §13.3 "YANLIŞ (KULLANMA): `identity_hash = SHA-256(device_ed25519_pub)`" der; gerekçesi
çok cihazlı kullanıcının bütün cihazlarının aynı `dest_hash`'i taşımasıdır. Öneri bu cümleyle çelişir.

**Reddedilen alternatifler:** (i) formülü imzalayana (kök) uygulamak — bütün cihazlar aynı `dest_hash`'i
alır, `PeerDirectory` `DuplicateDestHash` ile reddeder; (ii) cihaz başına ayrıca bir "kimlik" Ed25519 çifti
— biçime hiçbir şey imzalamayan yeni bir gizli anahtar ekler; (iii) rastgele 32 B — kart 3. maddedeki
türetme doğrulaması yapılamaz.

### 4.5 `domain_sep("SR_CDC_V1")` — ÖNERİ, DUR-K4G-12 (b)

> **AÇIK — Abkarada kararı bekleniyor.** ccr §13.2 `domain_sep("SR_CDC_V1")` için "16B sabit etiket" der;
> `SR_CDC_V1` 9 ASCII bayttır ve 16 B'a nasıl getirileceği hiçbir spec'te tanımlı değildir
> (`reports/spec-audit/SPEC-GAP-REGISTRY-2026-08-27.md`, S4-pki-discovery.6, MAJOR, açık).

**Öneri (uygulanan):** 9 B ASCII + 7 B `0x00` (sağa sıfır dolgu):

```
53 52 5f 43 44 43 5f 56 31 00 00 00 00 00 00 00
```

Etkisi PoC'de dardır: imza çalışma zamanında doğrulanmaz (§4.3) ve tanım değişirse yeni bir `init` koşusu
yeni kimlikler üretir (D4); göç gerekmez.

### 4.6 `peers` ve `direct_addrs`

Her cihazın `peers` listesi diğer bütün cihazları içerir; değerler o cihazların kendi dosyalarındakiyle
bayt-özdeştir. `direct_addrs` direct-mode içindir (INT-02 / K4G-11; hub yok): cihaz kendi adının
adresinde dinler, eşin adına yazılı adrese UDP gönderir.

### 4.7 `hub`

`hub.addr` hub sprinti için ayrılmıştır. `hub.x25519_pub` v0'da **daima `null`**: CLI'ın hub anahtarı
kaynağı yoktur, HubChannel PoC'de no-op'tur (hop anahtarı M4) ve ham hub anahtarı otorite kaynağı
yapılmaz (CLAUDE.md §4 "cert-bound"). Yükleyici `null` olmayan değeri okur ama v0 tüketicileri bu alanı
kullanmaz.

## 5. Tüketici sözleşmesi (K4G-11)

```rust
let config = kama_provision::format::load_device_config(path)?;   // okur + doğrular
let identity = config.local_identity();                          // kama_noise_wire::peer_dir::LocalIdentity
let directory = config.peer_directory()?;                        // kama_noise_wire::peer_dir::PeerDirectory
```

- `local_identity()` = `LocalIdentity::new(StaticSecret(x25519_static.priv), dest_hash, cert_ref_of(cert))`.
- `peer_directory()` = `PeerDirectory::new(peers → PeerEntry { dest_hash, static_x25519_pub: x25519_pub,
  cert_ref: cert_ref_of(cert) })` — `ControlConfig.directory` (ccr §35.4 kapalı-sistem kipi).
- Kimlik, anahtarlar ve eş dizini **bellekte** tutulur, depoya yazılmaz (D8).

**Yükleyici `load_device_config` şunları denetler** (yapısal tutarlılık; imza, zincir, iptal ve `dest_hash`
türetmesi denetlenmez — kapalı-sistem kipi):

1. Dosya ≤ 1 MiB (önce boyut, sonra okuma); geçerli JSON; bilinmeyen alan yok; `format_version = 0`.
2. `x25519_static.pub = X25519(priv)`, `ed25519.pub` = `priv`'in açık anahtarı.
3. Kendi `cert`'inin `device_ed25519_pub` / `device_x25519_pub` alanları kendi anahtarlarıyla aynı.
4. `peers`: 1–63 kayıt; adlar geçerli, birbirinden ve `device_id`'den farklı; her eşin `cert`'indeki
   `device_x25519_pub` = `peers[].x25519_pub`; statik anahtarlar tüm-sıfır değil.
5. `dest_hash` değerleri (kendisi + eşler) ve `cert_ref` değerleri birbirinden farklı.
6. `direct_addrs` anahtarları `device_id` ya da bir eş adı, portları 1–65535; `hub.addr` §2'deki
   `host:port` kuralına uyar.

`root.json` cihaz yapılandırması olarak yüklenemez (alanları uymaz; yükleyici reddeder).

## 6. Test kanıtı (K4G-12 DONE)

`crates/kama-provision/tests/init_cli.rs` gerçek ikiliyi geçici dizinde (`std::env::temp_dir()` altında
benzersiz alt dizin; repoya dosya yazılmaz) koşturur: 3 cihaz üretilir; her cihazın `peers`'ı diğer ikisini
içerir; `dest_hash` türetme fonksiyonuyla ve bağımsız SHA-256 hesabıyla doğrulanır; `cert` katı Ed25519
ile test köküne karşı doğrulanır (yanlış kök ve bozulmuş bayt reddedilir); iki koşu farklı anahtarlar
üretir; hiçbir `device-*.json` kök gizli anahtarını içermez (bayt araması: hex, büyük harf hex, ham bayt,
base64); `.gitignore` = `*`, dosyalar `0600`, dizin `0700`; standart çıktıda gizli anahtar yok.

## 7. Uygulama sınırları (protokol sınırı değildir)

| Sınır | Değer | Gerekçe |
|---|---|---|
| Cihaz sayısı | 2–64 | Tek cihazın eşi olmaz. Dosya boyu cihaz sayısıyla doğrusal büyür; 64 cihazda bir cihaz dosyası < 40 KiB'tır. PoC 2 telefon kullanır. |
| Ad | `[a-z0-9][a-z0-9_-]{0,31}` | Dosya adına güvenle girer (yol ayırıcı, nokta, büyük harf yok). |
| Yükleyici dosya boyu | ≤ 1 MiB | En büyük geçerli v0 dosyasının (64 cihaz) 25 katından fazla pay; okumadan önce denetlenir (SR-AUDIT-001). |

## 8. Değişiklik kaydı

| Tarih | Sürüm | Değişiklik |
|---|---|---|
| 2026-10-11 | v0 TASLAK | İlk sürüm (K4G-12). §4.4 ve §4.5 DUR-K4G-12 cevabını bekliyor. |
