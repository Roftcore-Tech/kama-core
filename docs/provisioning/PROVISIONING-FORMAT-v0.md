# PROVISIONING-FORMAT-v0 — `kama-provision` çıktı biçimi (JSON v0)

| Alan | Değer |
|---|---|
| Karar sahibi | Abkarada |
| Kart | K4G-12 (`KAMA-PLAN-4GUN-APP-KRIPTO` v1.1) — icra Claude Code L-PROV |
| Dayanak | Plan D4 (v1.1), D9 (v1.1), D11 · **plan Ek 2026-10-11 (3) — DUR-K4G-12 cevabı** · PoC-KAPSAM-001 §2 "Provisioning", "Anahtarlar", §5 (EK-1) · ccr §6.3, §8.1, §13.1–§13.5 (§13.2 YAMA 2026-10-11), §35.2, §35.4 KAPALI-SİSTEM KİPİ (PR #101) |
| Uygulama | `crates/kama-provision` — kütüphane `kama_provision::format` (tipler, yükleyici), `kama_provision::cert` (CompactDeviceCertV1), ikili `kama-provision` (CLI) |
| Tüketici | K4G-11 `crates/kama-node` (bileşim kökü) · K4G-05 INT-02 (anahtar kaynağı) · K4G-15 (örnek `device-alice.json`) |
| Durum | **v0** — ~~§4.4 ve §4.5'teki iki PoC tanımı **DUR-K4G-12** cevabını bekler~~ **DUR-K4G-12 CEVAPLANDI (Ek (3)):** (a) `dest_hash` ayrı kimlik anahtarından (§4.4), (b) `domain_sep` kabul + ccr §13.2 YAMA (§4.5), (c) altın vektörler (§4.8) |
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

**Cihaz gizli anahtarları.** Cihaz dosyası cihazın kendi X25519 ve Ed25519 gizli anahtarlarını ve kimlik
Ed25519 gizli anahtarını (`identity_ed25519.priv`; PoC'de çalışma zamanında kullanılmaz) düz JSON'da
taşır. Bu **kabul edilmiş PoC riskidir** (D9; PoC-KAPSAM-001 §5 EK-1/EK-2: at-rest koruması yok,
`noBackupFilesDir` yalnız yedeklemeyi sınırlar); büyütülmez.

**Bellek hijyeni (CLI ve yükleyici).** Gizli anahtar alanı (`Secret32`) öbekte durur, yalnız ödünç
alınan kaynaktan kurulur ve Drop'ta sıfırlanır; JSON tamponları uzunluk önce ölçülerek TEK seferde tam
kapasiteyle ayrılır (büyüyen tamponun eski kopyası kalmaz) ve Drop'ta sıfırlanır. Önlenemeyen kalan
kopyalar: `StaticSecret::from` / `LocalIdentity::new` diziyi değerle ister ve dalek tipleri yığında
taşınır; Rust'ta taşınan değerin eski yeri sıfırlanmaz. Gizli anahtarlar diskte zaten düz JSON'da
durduğu için bu kopyalar riski büyütmez.

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
4. Bir dosya yazılamazsa bu koşunun yazdığı gizli dosyalar (`device-*.json`, `root.json`) silinir;
   `.gitignore` ve (bu koşu oluşturduysa) dizin YALNIZ bütün gizli dosyalar gerçekten silindiyse silinir —
   silinemeyen bir gizli dosya Git'e karşı korumasız kalmaz. Silinemeyen yollar hata mesajında (standart
   hata) yalnız yol adıyla listelenir, içerik basılmaz; çıkış kodu `1`.
5. Windows ve diğer Unix dışı platformlarda izinler uygulanmaz; CLI standart hataya uyarı basar.
6. Standart çıktıya yalnız yazılan dosyaların yolları ve D9 hatırlatması basılır; **hiçbir anahtar,
   `dest_hash` ya da sertifika basılmaz**.

**Çıkış kodu:** `0` başarı · `1` çalışma hatası (dolu çıktı dizini, G/Ç, entropi, saat, öz-denetim) ·
`2` kullanım hatası. `2`'de ve `1`'in G/Ç dışı nedenlerinde hiçbir dosya yazılmaz; G/Ç hatasında
temizlik madde 4'teki kurala uyar.

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
  "identity_ed25519": {
    "priv": "<64 hex>",
    "pub": "<64 hex>"
  },
  "identity_hash": "<64 hex>",
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
      "identity_hash": "<64 hex>",
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
| `identity_ed25519.priv` / `.pub` | 32 B, hex | **Kimlik** Ed25519 çifti — cihaz anahtarlarından AYRI (plan Ek (3); PoC'de bir kullanıcı = bir cihaz). `priv` PoC'de çalışma zamanında kullanılmaz (SIGNED_CONTROL / attestation PoC dışı). |
| `identity_hash` | 32 B, hex | `SHA-256(identity_ed25519.pub)` (§4.4). |
| `dest_hash` | 32 B, hex | Cihazın yönlendirme kimliği: `SHA-256(identity_hash)` (§4.4). |
| `x25519_static.priv` / `.pub` | 32 B, hex | Statik X25519 (ccr §13.7). `pub = X25519(priv, 9)`. |
| `ed25519.priv` / `.pub` | 32 B, hex | **Cihazın** Ed25519 anahtarı (RFC 8032 tohumu / açık anahtarı); sertifikadaki `device_ed25519_pub`. Kimlik türetmesine girmez (ccr §13.3). |
| `cert` | 152 B, base64 | Cihazın CompactDeviceCertV1'i, test köküyle imzalı (§4.3). |
| `root_pub` | 32 B, hex | Test kökünün **yalnız açık** anahtarı. |
| `peers[]` | dizi (1–63) | Bu cihaz **dışındaki** bütün cihazlar, `--devices` sırasıyla. |
| `peers[].name` | dizge | Eşin `device_id`'si. |
| `peers[].identity_hash` | 32 B, hex | Eşin `identity_hash`'i. |
| `peers[].dest_hash` | 32 B, hex | Eşin `dest_hash`'i (= `SHA-256(peers[].identity_hash)`). |
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
- **Ed25519 (cihaz, kimlik ve kök):** `priv` = RFC 8032 §5.1.5 32 B tohum; `pub` RFC 8032'ye göre
  türetilir. Cihaz başına İKİ ayrı Ed25519 çifti üretilir: cihaz (`ed25519`, sertifikada) ve kimlik
  (`identity_ed25519`, yalnız `identity_hash` / `dest_hash` türetmesinde).

### 4.3 `cert` — CompactDeviceCertV1 (ccr §13.1 / §13.2)

```
CompactDeviceCertV1 (152 B, sabit; uzunluk öneki / magic / sürüm YOK)
  [  0.. 32) device_ed25519_pub   = ed25519.pub
  [ 32.. 64) device_x25519_pub    = x25519_static.pub
  [ 64.. 80) issuer_key_id        = SHA-256(0x01 ‖ root_pub)[0..16]
  [ 80.. 88) not_after            = u64_be(created_at_unix + 7 776 000)
  [ 88..152) issuer_signature     = Ed25519_Sign(root_priv, compact_device_cert_tbs)

compact_device_cert_tbs (104 B) =
  domain_sep("SR_CDC_V1") (16 B, §4.5; ccr §13.2 YAMA 2026-10-11) ‖ device_ed25519_pub ‖ device_x25519_pub
  ‖ issuer_key_id ‖ not_after
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
  testler ve çevrim dışı araçlar için sunar; kapsamı yalnız ccr §13.8 adım 9'dur (imza) — zincir, süre,
  iptal PoC dışı.
- **Kodlayıcının yeri:** kartın GİRDİ satırındaki "mevcut kodlayıcı" `kama-noise-wire`'da yoktur
  (`handshake.rs` sertifika alanını opak bayt dizisi olarak taşır: `COMPACT_DEVICE_CERT_LEN = 152`,
  `cert_ref_of`). Kodlayıcı ccr §13.1 / §13.2'den `kama_provision::cert`'te yazılmıştır (DUR-K4G-12;
  ~~karar bekleniyor~~ **Ek (3) ile onaylandı**). Sertifika düzeni ccr §13.1 ile AYNIDIR: cihaz
  anahtarlarını taşır, kimlik alanı EKLENMEZ; kimlik↔cihaz bağı PoC'de provisioning dosyasındadır (dizin =
  güven çapası, PoC-KAPSAM-001 §2).

### 4.4 `identity_hash` / `dest_hash` — KARAR, DUR-K4G-12 (a) (plan Ek 2026-10-11 (3))

**Karar:** `dest_hash` **kimlik** anahtarından türetilir, cihaz anahtarından değil. `kama-provision` cihaz
başına, cihaz Ed25519/X25519 çiftlerinden AYRI bir kimlik Ed25519 çifti üretir (`identity_ed25519`):

```
identity_hash = SHA-256(identity_ed25519.pub)      # ham 32 B anahtar; spki DEĞİL (ccr §8.1)
dest_hash     = SHA-256(identity_hash)             # ccr §13.5, CAD-16
```

Bu, identity-and-trust-model formülüdür; ccr §13.3'ün "YANLIŞ (KULLANMA)" dediği
`identity_hash = SHA-256(device_ed25519_pub)` kuralı ihlal edilmez. PoC'de bir kullanıcı = bir cihaz;
`identity_ed25519.priv` çalışma zamanında kullanılmaz (SIGNED_CONTROL / attestation PoC dışı). Türetme
`kama_provision::cert::identity_hash` ve `kama_provision::cert::dest_hash`; altın vektör
`vectors/ccr-dest-hash-v1/` (§4.8). Yükleyici iki eşitliği ve kimlik anahtarının cihaz anahtarından farklı
olduğunu denetler (§5).

**Reddedilen öneri — reddedildi (Ek (3)):** ~~`dest_hash = SHA-256(SHA-256(ed25519.pub))` — CAD-16
formülü cihazın Ed25519 açık anahtarına uygulanır~~. Gerekçe: spec'in açıkça yasakladığı formül (ccr §13.3)
koda girmez. Şeridin önerdiği diğer alternatifler de kayıtta: (i) formülü imzalayana (kök) uygulamak —
bütün cihazlar aynı `dest_hash`'i alır; (iii) rastgele 32 B — türetme doğrulaması yapılamaz. ~~(ii) cihaz
başına ayrıca bir "kimlik" Ed25519 çifti — biçime hiçbir şey imzalamayan yeni bir gizli anahtar ekler~~ →
(ii) **seçildi** (Ek (3)).

### 4.5 `domain_sep("SR_CDC_V1")` — KARAR, DUR-K4G-12 (b) (plan Ek 2026-10-11 (3))

~~**AÇIK — Abkarada kararı bekleniyor.**~~ **Kabul — spec kararı.** ccr §13.2 `domain_sep("SR_CDC_V1")`
için "16B sabit etiket" der; `SR_CDC_V1` 9 ASCII bayttır ve dolgu tanımlı değildi
(`reports/spec-audit/SPEC-GAP-REGISTRY-2026-08-27.md`, S4-pki-discovery.6). ccr §13.2'ye **YAMA KAYDI
2026-10-11 — K4G-12** yazıldı (bu PR): 16 B etiket = ASCII etiket ≤ 16 B + `0x00` dolgu (Noise protokol adı
dolgusuyla aynı gelenek); SPEC-GAP S4-pki-discovery.6 kapandı.

```
domain_sep("SR_CDC_V1") = 53 52 5f 43 44 43 5f 56 31 00 00 00 00 00 00 00
```

Altın vektör `ccr-cdc-v1.domain-sep.sr-cdc-v1` (§4.8).

### 4.6 `peers` ve `direct_addrs`

Her cihazın `peers` listesi diğer bütün cihazları içerir; değerler o cihazların kendi dosyalarındakiyle
bayt-özdeştir. `direct_addrs` direct-mode içindir (INT-02 / K4G-11; hub yok): cihaz kendi adının
adresinde dinler, eşin adına yazılı adrese UDP gönderir.

### 4.7 `hub`

`hub.addr` hub sprinti için ayrılmıştır. `hub.x25519_pub` v0'da **daima `null`**: CLI'ın hub anahtarı
kaynağı yoktur, HubChannel PoC'de no-op'tur (hop anahtarı M4) ve ham hub anahtarı otorite kaynağı
yapılmaz (CLAUDE.md §4 "cert-bound"). Yükleyici `null` olmayan değeri okur ama v0 tüketicileri bu alanı
kullanmaz.

### 4.8 Altın vektörler — DUR-K4G-12 (c)

Beklenen baytları yalnız Python referansı üretir (`ref/crypto/gen_vectors.py`, stdlib-only; Ed25519 imzası
`ref/crypto/ed25519.py` ile HESAPLANIR ve katı kipte doğrulanır); Rust tüketir:

| Suite | Vakalar | Rust tüketicisi |
|---|---|---|
| `vectors/ccr-dest-hash-v1/` | `identity-hash`, `dest-hash` (sabit kimlik açık anahtarı) | `crates/kama-provision/tests/ccr_cert_vectors.rs` |
| `vectors/ccr-cdc-v1/` | `domain-sep`, `issuer-key-id`, `tbs`, `encode` (test köküyle imzalı tam 152 B) | aynı dosya; `tests/init_cli.rs` TBS etiketini bu vektörden okur |

`ccr-cdc-v1` (CompactDeviceCertV1 = `SR_CDC_V1`), `vectors/examples/example-positive.json`'daki yer tutucu
biçim örneğinden (`ccr-compact-device-cert-v1`; imzası hesaplanmamış desenli dolgu) AYRI bir suite'tir;
`encode` kind'ı iki dosyada farklı girdi sözleşmesi taşır, suite adı onları ayırır.

Üretim: `python3 ref/crypto/gen_vectors.py` · denetim (CI): `python3 ref/crypto/gen_vectors.py --check`.

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

**Yükleyici `load_device_config` şunları denetler** (yapısal tutarlılık; imza, zincir ve iptal
denetlenmez — kapalı-sistem kipi; ~~ve `dest_hash` türetmesi~~ `dest_hash` türetmesi Ek (3) ile
denetlenir):

1. Dosya ≤ 1 MiB (önce boyut, sonra okuma); geçerli JSON; bilinmeyen alan yok; `format_version = 0`.
2. `x25519_static.pub = X25519(priv)`, `ed25519.pub` ve `identity_ed25519.pub` kendi `priv`'lerinin açık
   anahtarları; `identity_ed25519.pub ≠ ed25519.pub` (kimlik cihaz anahtarı olamaz, ccr §13.3);
   `identity_hash = SHA-256(identity_ed25519.pub)`, `dest_hash = SHA-256(identity_hash)`.
3. Kendi `cert`'inin `device_ed25519_pub` / `device_x25519_pub` alanları kendi anahtarlarıyla aynı.
4. `peers`: 1–63 kayıt; adlar geçerli, birbirinden ve `device_id`'den farklı; her eşin `cert`'indeki
   `device_x25519_pub` = `peers[].x25519_pub`; statik anahtarlar tüm-sıfır değil;
   `peers[].dest_hash = SHA-256(peers[].identity_hash)`.
5. `dest_hash` değerleri (kendisi + eşler) ve `cert_ref` değerleri birbirinden farklı.
6. `direct_addrs` anahtarları `device_id` ya da bir eş adı, portları 1–65535; `hub.addr` §2'deki
   `host:port` kuralına uyar.

`root.json` cihaz yapılandırması olarak yüklenemez (alanları uymaz; yükleyici reddeder).

## 6. Test kanıtı (K4G-12 DONE)

`crates/kama-provision/tests/init_cli.rs` gerçek ikiliyi geçici dizinde (`std::env::temp_dir()` altında
benzersiz alt dizin; repoya dosya yazılmaz) koşturur: 3 cihaz üretilir; her cihazın `peers`'ı diğer ikisini
içerir; `identity_hash` / `dest_hash` kimlik anahtarından türetme fonksiyonuyla ve bağımsız SHA-256
hesabıyla doğrulanır, kimlik anahtarı cihaz anahtarından farklıdır ve reddedilen cihaz-anahtarı formülüyle
çakışmaz; `cert` katı Ed25519 ile test köküne karşı doğrulanır — TBS etiketi Python altın vektöründen
okunur (yanlış kök ve bozulmuş bayt reddedilir); iki koşu farklı anahtarlar
üretir; hiçbir `device-*.json` kök gizli anahtarını içermez (bayt araması: hex, büyük harf hex, ham bayt,
base64); `.gitignore` = `*`, dosyalar `0600`, dizin `0700`; standart çıktıda gizli anahtar yok.
`crates/kama-provision/tests/ccr_cert_vectors.rs` §4.8 vektörlerinin her vakasını tüketir.
`src/provision.rs` birim testleri §2 madde 4'ü (silinemeyen gizli dosyada `.gitignore` korunur, yol
raporlanır) ve JSON tamponunun tek ayırmasını (kapasite = uzunluk) sınar.

## 7. Uygulama sınırları (protokol sınırı değildir)

| Sınır | Değer | Gerekçe |
|---|---|---|
| Cihaz sayısı | 2–64 | Tek cihazın eşi olmaz. Dosya boyu cihaz sayısıyla doğrusal büyür; 64 cihazda bir cihaz dosyası < 48 KiB'tır. PoC 2 telefon kullanır. |
| Ad | `[a-z0-9][a-z0-9_-]{0,31}` | Dosya adına güvenle girer (yol ayırıcı, nokta, büyük harf yok). |
| Yükleyici dosya boyu | ≤ 1 MiB | En büyük geçerli v0 dosyasının (64 cihaz) 20 katından fazla pay; okumadan önce denetlenir (SR-AUDIT-001). |

## 8. Değişiklik kaydı

| Tarih | Sürüm | Değişiklik |
|---|---|---|
| 2026-10-11 | v0 TASLAK | İlk sürüm (K4G-12). §4.4 ve §4.5 DUR-K4G-12 cevabını bekliyor. |
| 2026-10-11 | v0 | DUR-K4G-12 cevabı (plan Ek (3)): (a) `identity_ed25519` + `identity_hash` alanları (cihaz ve `peers[]`), `dest_hash = SHA-256(SHA-256(identity_pub))`; cihaz-anahtarı formülü reddedildi. (b) `domain_sep` kabul, ccr §13.2 YAMA KAYDI 2026-10-11. (c) altın vektörler (§4.8). |
| 2026-10-11 | v0 | İnceleme düzeltmeleri (dosya biçimi değişmedi): §2 madde 4 temizlik kuralı (`.gitignore` yalnız bütün gizli dosyalar silindiyse silinir; kalan yollar standart hataya, çıkış `1`); §1 bellek hijyeni; §4.3 doğrulama kapsamı (yalnız ccr §13.8 adım 9); §4.8 CDC suite adı `ccr-cdc-v1` (yer tutucu örnekle ad çakışması giderildi). |
