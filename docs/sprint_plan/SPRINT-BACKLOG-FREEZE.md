# SPRINT PLANI — Sözleşmeleri testle kilitle, sonra kodu yaz

**Tarih:** 2026-09-20 · v3.1 — kripto implementasyon risk taraması işlendi (A2.1/A2.3/A2.4/A2.7, A3.2, A4.4, A5.0–A5.5, WS-D) · v1/v2 superseded · **Okuyucu:** ekibe yeni katılan biri bu belgeyi açıp tek başına anlayabilmeli.

---

# KATMAN 1 — Bu sprintte ne yapıyoruz? (herkes için)

Kama'nın mimarisi dört katman: **Application → Transport → Crypto → UDP.** Application yalnız Transport'la konuşur, Crypto yalnız Transport tarafından çağrılır. Bu iki sınırın sözleşmeleri yazıldı ve onaylandı. Şimdi sözleşmeleri **testle kilitliyoruz** ki üç kişi birbirini beklemeden paralel kod yazabilsin.

**~~Hasan~~ karar Abkarada / icra Claude Code (test/CI)** *[2026-10-04, sahip kararı: bu katmanda şu an Hasan yok; girerse devralır]*
- Beklenen test çıktılarının tutulacağı ortak veri formatını kuruyor.
- Bu testlerin CI'da otomatik koşmasını sağlıyor.
- Mimari kuralları (kim kimi çağırabilir) otomatik kontrol eden betikleri ekliyor.

**Abdurrahman (kripto)**
- Kripto protokolündeki son sayısal sınırları belirliyor (kaç deneme, kaç saniye).
- Kriptonun "doğru çıktısı" nedir sorusuna Rust'tan bağımsız referans üretiyor (Python).
- Crypto↔Transport sınırını testlerle donduruyor.
- Sonra gerçek kripto kodunu bu testlere karşı yazıyor.

**Hayri (application)**
- Mesaj formatının (metin, dosya, PTT, okundu bilgisi) son kararlarını kapatıyor.
- Application'ın Transport'la konuştuğu API'nin tiplerini kuruyor.
- Sahte bir Transport'a karşı Application'ı bağımsız geliştiriyor.
- Sonra gerçek mesaj codec'ini ve yaşam döngülerini yazıyor.

**Transport geçişi (Abdurrahman ya da Resul)**
- Transport'un yeniden-gönderim kimliğini kriptonun paket sayacına bağlıyor, eski çerçeveyi kanonik formata taşıyor, iki ACK ailesinden birini seçiyor.
- Kripto çekirdeğini beklemez; yalnız uçtan uca testi (INT-02) açar.

**Sıra:** Hasan'ın veri formatı ilk saatlerde biter → üçü paralel çalışır → sınırlar ayrı ayrı "donar" → gerçek implementasyon aynı testlere karşı yazılır → ikisi, transport geçişiyle birlikte, INT-02'de (uçtan uca mesaj) buluşur.

**Bu sprintte YAPMADIĞIMIZ şey:** yeni tasarım. Mimari kapalı. Yeni bir konu çıkarsa dört kutudan birine gider: spec'e ekleme · sayı kalibrasyonu · test/kanıt · kod.

---

# KATMAN 2 — Sözlük

| Terim | Anlamı | Belge |
|---|---|---|
| **S1** | Application ↔ Transport sözleşmesi (Application'ın çağırdığı tek API) | `docs/seams/KM-SEAM-AT-001.md` |
| **S3** | Transport ↔ Crypto sözleşmesi; iki arayüz: **PacketProtection** (her datagramı mühürle/aç) ve **SessionControl** (bağlantı aç/kabul et/yenile) | `docs/seams/KM-SEAM-TC-001.md` |
| **AMP** | Application Message Protocol — şifreli paketin İÇİNDEKİ mesajın formatı (TEXT / FILE_* / PTT_* / RECEIPT) | `specs/application-message-protocol.md` |
| **AMP A1–A4** | AMP'nin 4 açık kararı; her biri için belgede öneri yazılı, sahibi onaylayınca "KAPANDI": A1 gövde encoding kuralı · A2 boyut/sıkıştırma limitleri · A3 alan listeleri · A4 okundu-bilgisi kapsamı | AMP §8 |
| **KS-\*** ("kalibrasyon slotu") | Anlamı belgede tanımlı ama **sayısı henüz verilmemiş** sabit; `[KALİBRASYON]` işaretli | aşağıda tek tek |
| KS-DOS-1 | Hub'ın yarım-açık bağlantı sınırları (4 sayı) | `specs/ccr-format-spec.md` §35.15 |
| KS-GRACE-1 | Anahtar değişiminde eski anahtarın ne kadar kabul edileceği (`N_g` paket / `T_g` süre) | `specs/kama-noise-wire-tdd-v2.0.md` §5 |
| ~~KS-8~~ | ~~`connect()` cevap için ne kadar bekler~~ **KAPANDI 2026-10-03 (DR-CONNECT-1): deadline yoktur, sayı yok** | `docs/seams/KM-SEAM-AT-001.md` §4 |
| ~~KS-HANDLE-1~~ | ~~Hub bir routing etiketini yeniden vermeden ne kadar bekler (`HANDLE_TOMBSTONE`)~~ **KAPANDI 2026-10-03: = S&F mailbox TTL (bugün 7 gün)** | `specs/ccr-format-spec.md` §35.7 |
| ~~KS-PN-1~~ | hop-ACK almadan ardışık tüketilebilecek PN tavanı — **KAPANDI 2026-10-03 (DR-CONNECT-1): K = 1024** | `specs/transport-layer-tdd.md` §12 [AMENDMENT-9] |
| ~~KS-AGE-1~~ | `SESSION_MAX_AGE`: oturum kaç yaşında yenilenir — **KAPANDI 2026-10-04: 7 gün** | `specs/ccr-format-spec.md` §35.15 · identity §16 |
| ~~KS-HB-1~~ | hop heartbeat kadansı — **KAPANDI 2026-10-04: 8 s / 25 s / OS**; çerçeve `HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C` (yeni codepoint) | `specs/transport-layer-tdd.md` §10 [AMENDMENT-10] |
| ~~KS-UI-1~~ | `LongOutage` eşiği — **KAPANDI 2026-10-04: 60 s** (yalnız UI) | `docs/seams/KM-SEAM-AT-001.md` §4 |
| **H-\*** | Harness testi: bir sözleşme kuralını otomatik doğrulayan test; tanımı seam belgesinin §6'sında | S1: `KM-SEAM-AT-001.md` §6 · S3: `KM-SEAM-TC-001.md` §6 |
| **Golden / conformance vektörü** | "Bu girdiye bu byte'lar çıkmalı" veri dosyası; Rust'tan bağımsız üretilir | `vectors/` (bu sprintte açılıyor) |
| **conformance-core** | Sözleşmenin gözlenebilir davranışını test eden set; hem sahte (mock) hem gerçek modüle karşı koşar | bu belge T-00.4 |
| **mock-fixture** | Sahte modülün consumer'ı yanıltmadığını doğrulayan testler | bu belge T-00.4 |
| **real-adapter** | Gerçek implementasyona özgü testler (mock'ta anlamsız olanlar) | bu belge T-00.4 |
| **SEAM-FROZEN** | "Sözleşme kilitlendi": kararlar kapalı + sayılar verili + vektörler commit + testler çalışıyor + mock hazır + mimari kontroller yeşil | seam belgeleri başlığı |
| **IMPLEMENTATION-GO** | "Gerçek kod sözleşmeyi geçti": aynı vektörler + aynı testler | bu belge §Kapılar |
| **INV-NEIGHBOR-1** | Mimari kural: Application yalnız Transport'u, Crypto yalnız Transport'u görür | `specs/transport-layer-tdd.md` §0 |
| **Tema E** | İki ACK kodek ailesi (engine `ACK_V2` ve noise-wire `Control::Ack/Sack`); hangisinin ürün ACK'i olacağı açık audit kalemi — WS-D2'de kapanır | `specs/kama-noise-wire-tdd-v2.0.md` §2.5 notu — **KAPANDI 2026-10-04 (K3):** ürün ACK'i engine `ACK_V2`; `Control::Ack/Sack` silindi |

**Repo gerçeği (Claude Code envanteri, kısaca):** Application katmanı kodu **yok**. Kripto `crates/kama-noise-wire`'da; Transport'un kriptoyu çağırdığı arayüz `crates/client-core/src/seam.rs` (`SecuritySeam` trait'i; 61 çağrı noktası; sahte `IdentitySeam`). Test verisi dosyası hiç yok — beklenen değerler Rust testlerine gömülü. CI `.github/workflows/ci.yml`. Python 3.14, **yalnız standart kütüphane**. Spec'ler `specs/` altında.

---

# KATMAN 3 — Ön-koşullar (sprint öncesi, yarım gün — Abdurrahman + Claude Code)

| ID | Ne | Neden | Bitti |
|---|---|---|---|
| R-1 | ~~`docs/seems/` → `docs/seams/`; iki seam belgesi commit~~ **KAPANDI 2026-09-23** (dizin `git mv` ile yeniden adlandırıldı; `scripts/ci/seam-api-drift.py` ve `seam-coverage.json` içindeki yollar da güncellendi — yoksa #64'ün seam kapısı kırılırdı. Düzeltme: belgeler untracked DEĞİLDİ, izleniyorlardı) | ~~yazım hatası; belgeler untracked~~ | `git ls-files docs/seams/` iki dosya ✅ |
| R-2 | ~~Revize noise-wire içeriğini `specs/kama-noise-wire-tdd-v2.0.md`'ye taşı; untracked `-v2_0.md` sil~~ **KAPANDI 2026-09-23** (19 Eylül içeriği taşındı, `-v2_0.md` silindi; düzeltme: dosya untracked DEĞİL, izlenen bir dosyaydı) | ~~aynı belgenin iki kopyası var~~ | tek dosya, revize içerik ✅ |
| R-3 | `specs/application-message-protocol.md` commit | untracked | tracked |
| R-4 | ~~`CLAUDE.md §3` spec yolu `docs/specs/` → `specs/`~~ **KAPANDI 2026-09-23** (4 yerde düzeltildi: başlık notu, §2, §3, §5) | ~~yanlış yol~~ | düzeltildi ✅ |
| R-5 | engine-spec / app-rpc / transport-tdd revizyonları PR'a | commit'lenmemiş değişiklik | PR |
| R-6 | `vectors/` dizini aç | repo'da test verisi dizini yok | dizin + README |
| R-7 | ~~**Dört konvansiyon kararı (Abdurrahman)**~~ **KAPANDI 2026-09-23 — dört karar da sahip tarafından verildi:** **(a)** application crate adı = `crates/kama-app`. **(b)** S3 trait'leri `client-core/src/seam.rs`'te evrilir, implementasyon `kama-noise-wire`'da; `crates/crypto` iskeleti (3 satır, kullanılmıyor) SİLİNİR; kripto bağımlılıkları kama-noise-wire'a eklenir. **(c)** sözleşme testi adlandırması = main'e inen `<seam>_<katman>` dizin şeması (`seam_conformance_core` / `seam_mock_fixture` / `seam_real_adapter`); S3 karşılığı `noise_conformance_core` / `noise_mock_fixture` / `noise_real_adapter`. ~~öneri `conf_`~~ — TEK şema; plandaki `conf_` öneki TERK EDİLDİ (A3.1/A3.2/A4.4'teki `conf_demux`/`conf_epoch`/`conf_s3_*` yolları bu şemaya göre okunur). **(d)** Python referansı = saf-Python RFC 8439 + RFC 7748 + RFC 8032 (stdlib-only, RFC'nin kendi vektörleriyle doğrulanır); `cryptography` paketi ALINMADI. | ~~kararsız kalırsa üç kişi farklı yere yazar~~ | dört karar yazılı ✅ |

---

# KATMAN 4 — Task kartları

Her kart: **Neden · Ne yapılacak · Nereye · Neye bağlı · Bittiğini nasıl anlayacağız.**

---

## T-00 — Ortak zemin (~~Hasan~~ karar Abkarada / icra Claude Code; ilk saatler)

### T-00.1 — Test verisi formatını kilitle
**Neden:** Beklenen byte çıktıları üç ayrı araç tarafından okunacak: Python üretici, Rust testleri, CI doğrulayıcı. Aynı formatı okumazlarsa her biri kendi yorumunu yapar ve testler birbirini doğrulayamaz.
**Ne yapılacak:** Abdurrahman'ın kilitlediği JSON şemasını yaz: `schema_version:1 · suite · spec_version · cases[{id, kind, input|input_hex, expected_hex|expected{result,reason}}]`. Kurallar: hex küçük harf, `0x` yok; byte dizileri hex string; beklenen wire her zaman hex; negatif vakalarda `result ∈ {drop, reject, fail}` tam bu üç değer; `id` repo genelinde tekil; bilinmeyen `schema_version` gören araç **hata verir, sessiz geçmez**.
**Nereye:** `vectors/SCHEMA.json` (JSON Schema) + `vectors/README.md`.
**Neye bağlı:** R-6.
**Done:** doğrulayıcı bir pozitif ve bir negatif örnek dosyayı kabul ediyor; `schema_version: 2` yazılmış dosyayı reddediyor.

### T-00.2 — İki yükleyici/doğrulayıcı
**Neden:** Üretici (Python) ve tüketici (Rust) aynı dosyayı aynı kurallarla okumalı.
**Ne yapılacak:** Python doğrulayıcı (stdlib `json`; üçüncü-parti yok) + Rust yükleyici (yeni küçük crate; `serde_json`; tek `hex` yardımcı fonksiyonu — bugün her test dosyasında kopya duran `fn hex()`'lerin yerine).
**Nereye:** `scripts/vectors/validate.py` · `crates/kama-vectors/`.
**Neye bağlı:** T-00.1.
**Done:** ikisi aynı örnek dosyayı yüklüyor; şema ihlalinde ikisi de hata veriyor.

### T-00.3 — CI'da otomatik koşum
**Neden:** Vektör ya da test değişince kimsenin "koşmayı unutması" mümkün olmasın.
**Ne yapılacak:** `ci.yml`'e `conformance` job'u. **Tetik:** `vectors/**` · `ref/**` · `**/tests/conf_*` · `crates/kama-app/**` · `crates/kama-noise-wire/**` · `crates/client-core/src/seam.rs` · `crates/kama-vectors/**` — yani vektör **ya da** ilgili kod değişince; kod değişip vektör dosyasına dokunulmadığında da koşar.
**Nereye:** `.github/workflows/ci.yml` (mevcut `test-fast` yanına).
**Neye bağlı:** T-00.2.
**Done:** boş test setiyle yeşil; bozuk JSON commit'iyle kırmızı.

### T-00.4 — Üç katmanlı test iskeleti
**Neden:** Aynı sözleşme testi hem sahte modüle (geliştirme sırasında) hem gerçek modüle (bittiğinde) karşı koşabilmeli; sahte modülün doğruluğu ayrıca kanıtlanmalı; gerçek modüle özgü testler (örn. socket reset) sahtede anlamsız, ayrı durmalı.
**Ne yapılacak:** üç dizin/kategori: `conformance-core` (sözleşmenin gözlenebilir davranışı) · `mock-fixture` (sahte modül consumer'ı yanıltmıyor) · `real-adapter` (implementasyona özgü). Runner hedef seçer: `mock` / `real`.
**Nereye:** `crates/kama-integration/tests/` (mevcut `tests/common/` var).
**Neye bağlı:** —
**Done:** boş iskelet CI'da koşuyor; hedef parametresi çalışıyor.

---

## WS-A — Abdurrahman (kripto)

### A1.1 — Hub'ın yarım-açık bağlantı sınırlarını belirle (KS-DOS-1)
**Neden:** Biri hub'a `SESSION_INIT` gönderdiğinde hub bir "yarım-açık" kayıt tutar. Sınır olmazsa saldırgan sahte INIT'lerle hub'ın belleğini doldurur; sahte `RESPONSE`'larla da alıcının CPU'sunu (her deneme X25519) tüketir. Sınırların **anlamı** belgede donmuş (kaynak başına kota, dolunca en eskisi düşer, aşımda sessiz drop, "kaynak" = bind öncesi UDP adresi / bind sonrası kimlik); yalnız **sayılar** eksik.
**Ne yapılacak:** `HALF_OPEN_TTL · HALF_OPEN_CAP · PER_SOURCE_INIT_RATE · PENDING_RESPONSE_TRIAL_CAP` için değer + bir cümle gerekçe (dürüst bir istemcinin retry profili).
**Nereye:** `specs/ccr-format-spec.md` §35.15, `[KALİBRASYON]` işaretli yerler.
**Neye bağlı:** —
**Done:** dört `[KALİBRASYON]` sayıya dönüştü.
**DURUM (2026-09-24): KAPANDI — sahip kararı.** ccr §35.15 KS-DOS-1 KAPANIŞ bloğu (TTL = S&F TTL · CAP 64/kimlik · 1/s burst 32 · TRIAL_CAP 16 = istemci pending tavanı ≤ CAP zinciri). "Bind öncesi = UDP 4-tuple" INIT'ten çıkarıldı → `[KARAR — M6]`. Beta tetikleri `docs/PARK-LISTESI.md`.

### A1.2 — Anahtar değişimi penceresini belirle (KS-GRACE-1)
**Neden:** Bağlantı kurulunca taraflar geçici anahtardan kalıcı anahtara geçer (epoch 0→1). Geçiş anında yolda hâlâ eski anahtarla mühürlenmiş paketler olabilir; bunlar kaybolmasın diye kısa bir süre iki anahtar da kabul edilir. Ne kadar süre? Kural belgede: `N_g` geçerli yeni-anahtar paketi VEYA `T_g` süre — hangisi önce dolarsa pencere kapanır, bir daha açılmaz.
**Ne yapılacak:** `N_g` ve `T_g` değerleri.
**Nereye:** `specs/kama-noise-wire-tdd-v2.0.md` ~~§5~~ **§4 "NONCE/PN/ANTI-REPLAY" madde 5** KS-GRACE-1 *(işaretçi düzeltildi 2026-09-24: dosyada §5 diye bir yer yok)*.
**Done:** iki sayı yazıldı.
**DURUM (2026-09-24): KAPANDI — sahip kararı.** `N_g = 64` (yalnız AEAD-geçerli epoch-1) · `T_g = 30 s` (monotonic, implicit confirmation anından). Yerel politika, tel sabiti değil. Beta tetiği: PCS/ratchet.

### A1.3 — `connect()` bekleme süresi (KS-8)
**Neden:** Application `connect(peer)` deyince Transport karşı taraftan `RESPONSE` bekler. Karşı taraf çevrimiçiyken ne kadar bekleyip `ConnectFailed` diyeceğiz? (Çevrimdışı/mailbox yolunda süre yok; orada 7 güne kadar cevap gelebilir.)
**Ne yapılacak:** süre değeri.
**Nereye:** `docs/seams/KM-SEAM-AT-001.md` §4.
**Done:** sayı yazıldı.
**DURUM (2026-09-24): AÇIK — sayı uydurulmadı, sahip kararı.** `connect()` dört parçadır (hub kanalı kurulumu · Initiate · INIT'in Plane 1 ile hub'a ulaşması · Bob'un RESPONSE'unun dönüşü); yalnız 4. parça yeni sayı ister, 1 ve 3 transport'un donmuş zamanlayıcılarıdır. Ayrım saatle değil **hub'ın cevabıyla** yapılmalı: INIT mailbox'a kondu (Bob çevrimdışı → deadline yok, T-18c) / doğrudan teslim edildi (Bob çevrimiçi → KS-8). İki düzeltme bekliyor: S1 §4'teki "connect() süresi Transport'un Plane 1 Reliable deadline'ıdır" cümlesi YANLIŞ (o deadline 3. parça, 4. değil) · `cancel(ticket)` yok, zamanlayıcı transport'ta kalır (K9 ile tutarlı). **KANIT İSTEĞİ (5 kalem, kod/spec'ten sayı + dosya:satır):** `docs/sprint_plan/A1-KALIBRASYON-KARAR-FOYU.md` → KANIT PAKETİ. Sayılar gelince KS-8 = (1+3)'ün üstünde türetilir; KS-HANDLE-1 ve KS-PN-1 de aynı paketle koda bağlanır.
**DURUM (2026-10-03): KAPANDI — DR-CONNECT-1 (sahip kararı).** Sayı yok: `connect()` için deadline YOKTUR (INV-NO-IDLE-TEARDOWN). `ConnectFailed` kalktı; `cancel(ticket)` + `ConnectAbandoned(ticket, reason)` + `ConnectProgress(ticket, phase)` geldi. Normatif metin `docs/seams/KM-SEAM-AT-001.md` §1/§3/§4/§6; indeks `docs/decisions/DR-CONNECT-1.md`. S1 değişikliği Hayri imzası bekliyor.

### A1.4 — Routing etiketi yeniden-kullanım süresi (KS-HANDLE-1)
**Neden:** Hub her bağlantı bacağına 4 byte'lık bir etiket (`hub_handle`) verir. Bağlantı kapanınca etiket bir süre **askıda** kalmalı; yoksa geç gelen eski bir paket yeni bir bağlantının bacağına düşer. Süre uydurma olmasın: belgede formül var — *o etikete geç gelebilecek son meşru paketin ufku* (ARQ retry ufku, yeniden-sıralama penceresi, mailbox ufku — en büyüğü).
**Ne yapılacak:** formülü üç girdiyle hesapla, sayıyı yaz.
**Nereye:** `specs/ccr-format-spec.md` §35.7 KS-HANDLE-1.
**Done:** hesap gösterildi, sayı yazıldı.
**DURUM (2026-10-03): KAPANDI — DR-CONNECT-1 / kanıt K2.** `HANDLE_TOMBSTONE` = S&F mailbox TTL (bugün 7 gün; türetilmiş). Mailbox'tan teslim ESKİ handle'la çıkıyor (`[HANDLE_ASSIGN, INIT/extended]`, noise-wire §2.55) → ufuk mailbox ufku. Föydeki 105 s alternatifi dayanaksız çıktı (alıcı PeerSlot idle'ı; PeerSlot handle tutmuyor). ccr §35.7 yamalandı.

### A2.1 — Rust'tan bağımsız Python referansı
**Neden:** Rust kripto kodunu kendi ürettiği "beklenen" değerlerle test edersek, koddaki hata beklentiye de gömülür ve test kendi hatasını onaylar. İkinci, bağımsız bir implementasyon şart. Kural: Rust'tan **hiçbir kod ve sabit** paylaşmaz.
**Ne yapılacak:** Standart-kütüphane-only Python: HKDF-BLAKE2s — **`hmac.new(key, msg, hashlib.blake2s)`** ile (RFC 5869 Extract+Expand); **`hashlib.blake2s(key=…)` KULLANILMAZ**: o BLAKE2s'in anahtarlı modudur, HMAC değildir (ccr INV-KDF-3) ve aynı girdiyle tamamen farklı çıktı verir. R-7(d) kararına göre ChaCha20-Poly1305 + X25519 + **Ed25519** (RFC 8032 §6 illüstrasyonu + ccr §5.2'nin small-order ve kanonik-encoding kontrolleri). **Önce referansın kendisi doğrulanır:** RFC 5869 A.1–A.3 (SHA-256), RFC 8439, RFC 7748 ve RFC 8032 test vektörleri geçmeli. Ayrıca V-KDF-6 negatif testi: HMAC-BLAKE2s ile BLAKE2s'in kendi anahtarlı modu **farklı** çıktı vermeli — belgede bilinen bir tuzak.
**Nereye:** `ref/crypto/` (yeni).
**Neye bağlı:** R-7(d).
**Done:** RFC vektörleri geçiyor; V-KDF-6 negatifi geçiyor.
**DURUM (2026-09-23): KAPANDI.** `ref/crypto/` — `kdf.py` · `x25519.py` · `ed25519.py` · `chacha20poly1305.py` · `selftest.py` (stdlib-only, 3. parti paket yok). Geçen vektörler: RFC 5869 A.1–A.3 · RFC 7748 §5.2 (iter 1 ve 1000) + §6.1 · RFC 8032 §7.1 (TEST 1/2/3/SHA(abc)/1024) · RFC 8439 §2.3.2–§2.8.2 + A.5. V-KDF-6 negatifi geçiyor. Kapı `core-ci / vectors` job'ına bağlandı; 5 mutasyonla vakum olmadığı gösterildi (INV-KDF-3 karışıklığı dahil hepsi yakalandı). D1–D7 için burada **değer** vektörü YOK, yalnız yapısal özellikler — exact byte'lar A2.2'de üretilecek.

### A2.2 — Anahtar türetme vektörleri (V-KDF-1..7)
**Neden:** Bağlantı kurulurken 7 farklı anahtar/kimlik türetiliyor (D1–D7: kök, trafik anahtarları, oturum kimliği, epoch-1 kökü, INIT/RESPONSE mühür anahtarları). Rust'ın her birini byte-for-byte doğru türettiğini görmenin tek yolu bağımsız referansla karşılaştırmak.
**Ne yapılacak:** A2.1 ile 7 vektör üret; T-00.1 formatında commit.
**Nereye:** `vectors/ccr-kdf.json`. Tanım: `specs/ccr-format-spec.md` §35.2.1, liste §35.13.
**Neye bağlı:** A2.1, T-00.1.
**Done:** dosya commit; Python doğrulayıcı kabul ediyor.
**DURUM (2026-10-04): KAPANDI (M4).** `vectors/ccr-kdf-v1` — V-KDF-1..6 (12 vaka); V-KDF-7 süreç şartı. Üretici `ref/crypto/gen_vectors.py`; `--check` CI'da (`vectors` job).

### A2.3 — INIT paketi vektörleri (SSI1)
**Neden:** `SESSION_INIT` bağlantıyı başlatan paket; wire'daki tam byte'ları interop için sabit olmalı. Negatif vakalar da gerekli: bozulmuş paket **doğru şekilde** reddedilmeli (ve nasıl — AEAD'den önce mi sonra mı).
**Ne yapılacak:** 3 sertifika modu için pozitif exact byte; negatif: flags bit-flip ve init_counter bit-flip → `drop` (AEAD düşer); sertifika uzunluğu uyumsuz → `reject` (parser, AEAD'den önce); **sıfır olmayan düşük-mertebeli** sender ephemeral → `reject` (ccr §5.1: DH çıktısı sıfır). Vektör `input`'u tüm özel anahtarları (ephemeral + static) taşır — gerçek Crypto'nun exact byte karşılaştırması ancak ephemeral dışarıdan verilebilirse mümkün (A5.5).
**Nereye:** `vectors/ccr-ssi1.json`. Tanım: `ccr` §35.3, §35.3.1.
**Done:** commit.
**DURUM (2026-10-04): KAPANDI (M4).** `vectors/ccr-ssi1-v1` — üç cert_mode AAD + mühür, negatifler (16 vaka); cert_field opak. Üretici `ref/crypto/gen_vectors.py`; `--check` CI'da (`vectors` job).

### A2.4 — RESPONSE paketi vektörleri (SSR1)
**Neden:** A2.3 ile aynı, cevap paketi için.
**Ne yapılacak:** pozitif exact; negatif: karşı tarafın geçici anahtarı sıfır → `reject` (bu yalnız parse filtresi); **sıfır olmayan düşük-mertebeli** responder ephemeral → `reject` (ccr §5.1'in asıl zorunlu kontrolü: DH çıktısı sıfır — parse filtresi bunu yakalamaz); response_counter değiştirilmiş → `drop`. `input` tüm özel anahtarları taşır.
**Nereye:** `vectors/ccr-ssr1.json`. Tanım: `ccr` §35.5, §35.5.1.
**Done:** commit.
**DURUM (2026-10-04): KAPANDI (M4).** `vectors/ccr-ssr1-v1` — AAD + mühür, negatifler (8 vaka). Üretici `ref/crypto/gen_vectors.py`; `--check` CI'da (`vectors` job).

### A2.5 — Veri paketi vektörleri (SSD1)
**Neden:** Kurulmuş bağlantıda her veri datagramı 22 byte'lık sabit zarfla gider (`hub_handle 4 · seq 2 · şifreli veri · tag 16`). Nonce ve AAD'nin nasıl kurulduğu byte düzeyinde sabit.
**Ne yapılacak:** pozitif exact zarf + nonce96 + AAD vektörleri.
**Nereye:** `vectors/ccr-ssd1.json`. Tanım: `ccr` §35.6, §35.8, §35.9.
**Done:** commit.
**DURUM (2026-10-04): KAPANDI (M4).** `vectors/ccr-ssd1-v1` — 22 B zarf, D-GAP-1 far-future kabul, boş batch (15 vaka); 5 mühür vektörü mevcut Rust yoluyla bayt-özdeş. Üretici `ref/crypto/gen_vectors.py`; `--check` CI'da (`vectors` job).

### A2.6 — Eski formatların reddi
**Neden:** Protokolün önceki sürümlerinin paketleri (SAI1/SAA1/SAD1/SAC1, 39 byte'lık zarf, eski imza girdisi) artık geçersiz; yeni kod bunları **reddetmeli**, yanlışlıkla kabul etmemeli.
**Ne yapılacak:** her eski format için bir örnek → `reject`.
**Nereye:** `vectors/ccr-deprecated-neg.json`. Tanım: `ccr` §35-DEPRECATED, `identity` §27.
**Done:** commit.
**DURUM (2026-10-04): KAPANDI (M4).** `vectors/ccr-deprecated-neg-v1` — SAI1/SAA1/SAD1/SAC1/SRAM + DEPRECATED-REJECT-1/2 (7 vaka). Üretici `ref/crypto/gen_vectors.py`; `--check` CI'da (`vectors` job).

### A2.7 — Ed25519 doğrulama kipi vektörleri
**Neden:** RFC 8032 iki farklı doğrulama denklemine izin veriyor ve küçük-mertebeli bileşenler hakkında sessiz; kip seçilmezse iki implementasyon aynı imzayı farklı sınıflandırır. Kip artık `ccr` §5.2'de sabit; bu vektörler onu kilitler ve Rust kütüphanesinin gerçekten o kipi uyguladığını kanıtlar.
**Ne yapılacak:** her ret nedeni için ayrı negatif: `S ≥ ℓ` · küçük-mertebeli public key (A) · küçük-mertebeli R · kanonik olmayan nokta encoding'i → hepsi `reject`; bir pozitif (geçerli imza) kontrol vakası.
**Nereye:** `vectors/ccr-ed25519-neg.json`. Tanım: `ccr-format-spec.md` §5.2.
**Neye bağlı:** A2.1 (Ed25519 referansı), T-00.1.
**Done:** commit; kütüphane bir kuralı kapsamıyorsa A5'te açık kontrol eklenir.
**DURUM (2026-10-04): KAPANDI (M4).** `vectors/ccr-ed25519-neg-v1` — ccr §5.2'nin dört ret nedeni ayrı vakalarda (10 vaka). Üretici `ref/crypto/gen_vectors.py`; `--check` CI'da (`vectors` job).

### A3.1 — Response eşleştirme sınırını test et
**Neden:** Gelen `RESPONSE` paketinin hangi bekleyen bağlantıya ait olduğu wire'da yazmaz (byte tasarrufu + gizlilik). Crypto bunu bekleyen adayları sırayla deneyerek bulur. Saldırgan sahte RESPONSE'la sınırsız deneme yaptıramasın diye deneme sayısı sınırlı. Ayrıca "yanlış aday" ile "bozuk paket" dışarıdan **aynı** görünmeli — yoksa saldırgan hangi adayın doğru olduğunu öğrenir.
**Ne yapılacak:** `PENDING_RESPONSE_TRIAL_CAP` kadar aday oluştur; doğru aday ilk/orta/son pozisyondayken kabul; doğru aday sınır dışında kalınca `Failed`; yanlış aday ve bozuk paket aynı hata.
**Nereye:** `crates/kama-noise-wire/tests/conf_demux.rs`. Tanım: `KM-SEAM-TC-001.md` §6 H-DEMUX ve K7.
**Neye bağlı:** A1.1 (sayı koda gömülmez, spec'ten okunur).
**Done:** sınır testleri CI'da yeşil.

### A3.2 — Anahtar değişimi makinesini test et
**Neden:** Epoch 0→1 geçişinde `RESPONSE` kaybolabilir, geç gelebilir, sırası bozulabilir. Makine bunların hepsinde güvenli kalmalı. En kritik: saldırgan **sahte** bir yeni-anahtar paketi göndererek "geçiş tamamlandı" izlenimi yaratamamalı — geçiş yalnız AEAD'i geçen paketle onaylanır.
**Ne yapılacak:** matris: RESPONSE normal / kayıp / sırasız × AEAD-geçersiz epoch-1 paketi geçişi tetiklemez × pencere `N_g` VEYA `T_g` hangisi önce × **anahtar denemesi**: legacy 22 B formatında epoch biti yok, grace içinde en fazla iki anahtar denenir (noise-wire K-WIRE-TRIAL-1); replay precheck/commit **denenen anahtarın epoch penceresine** yapılır (identity T-19) — epoch-1 anahtarıyla açılan paket epoch-1 penceresine commit edilir, epoch-0 penceresi değişmez.
**Nereye:** `crates/kama-noise-wire/tests/conf_epoch.rs`. Tanım: `noise-wire` §4/§5.
**Neye bağlı:** A1.2.
**Done:** matris yeşil.

### A3.3 — Hub yarım-açık sınırlarını test et — **NON-GATING / real-adapter follow-up** (G-S3 bunu istemez)
**Neden:** A1.1'deki sınırların hub'da gerçekten uygulandığını görmek.
**Ne yapılacak:** kota dolunca en eski kaydın düştüğü, aşımda sessiz drop olduğu.
**Nereye:** real-adapter (hub-server iskeleti dolunca).
**Neye bağlı:** A1.1, hub-server.
**Done:** hub-server iskeleti dolunca; bu sprint'in kapılarını etkilemez.

### A4.1 — Yeni sözleşme tiplerini derlenir hâle getir
**Neden:** Transport bugün Crypto'yu `SecuritySeam` trait'iyle çağırıyor (61 noktadan). S3 sözleşmesi bu arayüzü değiştirdi: Transport artık paketin sayacını üretmiyor (Crypto üretiyor), bunun yerine "bu paket hangi bağlamda — oturum mu, hub kontrolü mü" bilgisini (`ctx`) veriyor; `seal()` şifreli byte'larla birlikte Crypto'nun atadığı sayacı da döndürüyor. İki taraf aynı imzaya karşı yazabilsin diye önce tipler derlenmeli.
**Ne yapılacak:** `PacketProtection` (`seal(ctx, frame) → SealedUnit{bytes, meta}` · `open` · `sealed_len` · `conservative_limits` · `limits`) ve `SessionControl` (`command(SessionCommand) → SessionResult`, `poll_event`) trait'leri; mevcut `SecuritySeam`'den evrilir; `IdentitySeam` yeni imzaya adapte, davranışı aynı.
**Nereye:** R-7(b): `crates/client-core/src/seam.rs`. Tanım: `KM-SEAM-TC-001.md` §1.1–§1.2.
**Neye bağlı:** R-7(b).
**Done:** `cargo build --workspace` yeşil; 61 çağrı noktası derleniyor.

### A4.2 — Sahte Crypto (Transport testleri için)
**Neden:** Transport tarafı testleri gerçek kripto bitmeden koşabilsin.
**Ne yapılacak:** `MockCrypto`: deterministik seal/open; `command()` her zaman `Established`; `AcceptResponse` sınırlı deneme yapar; `meta.full_wire_pn` ardışık artar. Mevcut `IdentitySeam` kalıbı, `test-util` feature'ı altında.
**Nereye:** `client-core/src/seam.rs` yanı veya `kama-noise-wire` (`test-util`).
**Neye bağlı:** A4.1.
**Done:** mock-fixture testleri yeşil (mock, Transport'u yanıltmıyor).

### A4.3 — Sahte Transport (Crypto testleri için)
**Neden:** Crypto kodu gerçek transport olmadan test edilebilsin. **Dikkat:** Crypto'nun komşusu Transport'tur, Application değil — bu sahte modül Application'a ait hiçbir tip içeremez (mimari kural INV-NEIGHBOR-1; C1.2 bunu otomatik kontrol eder).
**Ne yapılacak:** `SessionCommand` enjeksiyonu; `OutboundEnvelope`'u saklayıp byte-for-byte yeniden gönderme; zarf kayıp/sırasız/duplicate enjeksiyonu.
**Nereye:** `crates/kama-noise-wire/tests/common/`.
**Neye bağlı:** A4.1.
**Done:** mock-fixture yeşil; C1.2 statik kontrol yeşil.

### A4.4 — S3 sözleşme testleri (conformance-core)
**Neden:** Sözleşmenin her kuralı çalışan bir testle kanıtlanmalı. Her test neyi kanıtlıyor:
- `H-CTX` — aynı içerik "oturum" ve "hub kontrolü" bağlamında **farklı** anahtarla mühürleniyor.
- `H-PN-OWN` — Transport sıfırlansa/yeniden başlasa da Crypto'nun sayacı geri dönmüyor (nonce tekrarı yok).
- `H-FEC-ORDER` — sahte bir FEC parçası açılamadığı için decoder'a **hiç girmiyor**; yeterli geçerli parçayla kurtarma çalışıyor.
- `H-RETRY` — **Alice:** aynı INIT byte-for-byte yeniden gönderilebiliyor; yeni `Initiate` = yeni geçici anahtar. **Bob:** aynı INIT tekrar gelirse mevcut oturuma eşleniyor ve önbellekteki RESPONSE **byte-identical** dönüyor — yeni geçici anahtar, ikinci oturum YOK (yoksa iki rakip oturum doğar ve bağlantı sessizce kopar).
- `H-NO-MUTATE` — her başarısızlıkta iç durum (replay penceresi, bekleyenler, anahtarlar) byte-for-byte aynı kalıyor.
- `H-HANDLE` — kapanmış oturuma `seal` → hata; `HubHandle` ile `SessionHandle` tip düzeyinde ayrı.
- `H-REPLAY-POISON` — sahte paket replay penceresini ilerletmiyor; ardından aynı sayaçla gelen **geçerli** paket kabul ediliyor.
**Ne yapılacak:** yukarıdaki yedi test.
**Nereye:** `crates/kama-noise-wire/tests/conf_s3_*.rs`. Tanım: `KM-SEAM-TC-001.md` §6; replay kuralı `identity` §15.5.
**Neye bağlı:** A4.1, A4.2, A4.3.
**Done:** `MockCrypto`'ya karşı yeşil.

### 🚪 G-S3 — S3 SEAM-FROZEN
A1.1 + A1.2 sayıları · A2.* vektörleri commit · A4.* yeşil · C1.2 yeşil. Bu noktadan sonra Crypto'nun ne yapacağı yoruma açık değil.

### A5.0 — Kripto bağımlılıklarını ekle (G-S3'ü beklemez, erken yapılabilir)
**Neden:** `kama-noise-wire`'da bugün yalnız `chacha20poly1305`, `blake2`, `zeroize` var; HKDF, HMAC, X25519, Ed25519 ve entropi kaynağı yok. Lisansları `deny.toml` izin listesinde değilse ilk PR `security` job'unda kırmızıya düşer.
**Ne yapılacak:** `hkdf`, `hmac`, `x25519-dalek` (`reusable_secrets`/`static_secrets` + `zeroize` özellikleri), `ed25519-dalek`, `rand_core`/`getrandom`; `deny.toml` `[licenses]` kontrolü (dalek crate'leri BSD-3-Clause).
**Nereye:** `crates/kama-noise-wire/Cargo.toml` (R-7b kararına göre `crates/crypto`), `deny.toml`.
**Done:** `cargo deny check` yeşil.

### A5.1 — Anahtar türetme (D1–D7)
**Neden:** Gerçek kod. **Ne:** `hkdf::SimpleHkdf<blake2::Blake2s256>` ile 7 türetim. **Dikkat:** `hkdf::Hkdf<Blake2s256>`'in derlenmemesi beklenir (BLAKE2'nin blok API'si "lazy", `Hmac` "eager" ister) — **ilk iş bu derlemeyi teyit etmek**. Derleme hatasını "düzeltmek" için `blake2::Blake2sMac`'e geçmek YASAK: o BLAKE2s'in anahtarlı modudur, HMAC değildir (ccr INV-KDF-3) ve sessizce yanlış anahtar üretir — V-KDF-6 yakalar. **Nereye:** R-7(b). **Bağlı:** G-S3, A5.0. **Done:** A2.2 byte-for-byte.

### A5.2 — Mühürleme/açma + sayaç + nonce/AAD
**Ne:** seal/open; sayacı Crypto tutar; nonce96 ve AAD `ccr` §35.6–§35.9; her X25519 DH sonrası `SharedSecret::was_contributory()` (ccr §5.1) — birden çok DH'nin birinde unutmak en olası hata. **Nereye:** `kama-noise-wire`. **Done:** A2.3–A2.5 vektörleri (düşük-mertebe negatifleri dahil); `H-PN-OWN`.
**KURAL (2026-09-24, PR #68 incelemesinden — sahip kararı):** (a) **tag doğrulamasını asla kendin yazma** — AEAD crate'inin `decrypt`'i kullanılır (içeride `subtle` sabit-zamanlı); elle byte karşılaştırması YASAK. (b) **`seal()` girişinde uzunluk kontrolü, fail-closed** — RFC 8439 (key,nonce) başına 256 GiB sınırı yapısal olarak ulaşılamaz (kayıt başına nonce + `MAX_BODY_BYTES ≤ 65535`), yine de aşırı boy → **hata, sessiz sarma değil**; conformance testi "aşırı boy → hata". Bağlayıcı sınır bayt değil **nonce tekilliği**: PN sahipliği (S3 K2) + `SEND_PN_CAP`. TLS 1.3 aynı gerekçeyle ChaCha20-Poly1305'e kayıt sınırı koymaz.

### A5.3 — Replay koruması
**Ne:** `replay_precheck` (yalnız okur) + `replay_commit` (yalnız AEAD başarısında yazar). Tanım `identity` §15.5. **Done:** `H-REPLAY-POISON`, `H-NO-MUTATE`.

### A5.4 — Anahtar değişimi makinesi
**Ne:** confirmation-safe geçiş, pencere `N_g`/`T_g`; grace içinde en fazla iki anahtar denemesi, replay precheck/commit **denenen anahtarın epoch penceresine** (A3.2). Mevcut `kama-noise-wire/src/epoch_machine.rs` üstünde. **Done:** A3.2.

### A5.5 — Oturum kontrolü
**Ne:** Initiate / AcceptInit / AcceptResponse (sınırlı aday deneme) / Renew / Close; anahtar silme sırası (`identity` §15.6). Mevcut `session.rs`, `init_flight.rs`.
**Ephemeral tuzakları:** (a) `x25519-dalek` `EphemeralSecret`, `diffie_hellman(self)` çağrısında **tüketilir**; oysa `e_A` hem INIT'te, hem RESPONSE gelince, hem de başarısız aday denemelerinden sonra yeniden kullanılır (identity §15.6 P0-13: AEAD hatasında silinmez) — tüketilen tip seçilirse tek bir sahte RESPONSE meşru bağlantıyı öldürür → `ReusableSecret`/`StaticSecret` + açık zeroize. (b) Entropi kaynağı `test-util` altında enjekte edilebilir olmalı — A2.3/A2.4 exact vektör karşılaştırması ancak böyle mümkün; aksi hâlde test kendi seal'ini kendi open'ıyla açan round-trip'e düşer. (c) Bob: tekrarlanan INIT → mevcut oturum + önbellekteki RESPONSE (noise-wire §0.7-B, ccr INV-RESP-KEY-1).
**Done:** A3.1; `H-RETRY` (Alice + Bob), `H-HANDLE`, `H-NO-MUTATE`.

### 🚪 G-IMPL-S3 — Crypto IMPLEMENTATION-GO
Gerçek kod aynı vektörleri ve aynı conformance-core'u geçiyor; sözleşme dışında davranış yok.

---

## WS-B — Hayri (application) — Abdurrahman'ı BEKLEMEZ

### B1.0 — Application crate'i ve Transport API tipleri
**Neden:** Repo'da Application katmanı kodu hiç yok. Application'ın konuştuğu **tek** şey Transport API'si; önce onun tipleri derlenmeli ki hem Application kodu hem sahte Transport aynı imzaya karşı yazılsın.
**Ne yapılacak:** yeni crate (R-7a); `KamaTransport` trait'i: `connect(peer, early_unit?) → ConnectResult{Ready(h)|Pending(ticket)}` · `send(session, PreparedUnit, DeliveryPolicy)` · `recv()` · `poll_event()` · `close()` · `capabilities()`; hata tipi `TransportError{UnknownSession, SessionSuperseded, EarlyUnitConflict, UnitTooLarge, Backpressure, DeadlineExpired, NotConnected}`; olaylar `Ready / RenewRequired / Closed / ConnectFailed`.
**Nereye:** `crates/kama-app/` (öneri). **Kural:** `Cargo.toml`'da `kama-noise-wire` veya `crypto` bağımlılığı **olamaz** — Application Crypto'yu görmez.
**Neye bağlı:** R-7(a). Tanım: `KM-SEAM-AT-001.md` §1.
**Done:** derleniyor; C1.2 statik kontrol yeşil.

### B1.1 — Mesaj formatının açık kararlarını kapat (AMP A1–A4)
**Neden:** Kod yazılmadan mesaj formatı kesinleşmeli; belgede her karar için öneri hazır, sahibi onaylayınca kapanır.
**Ne yapılacak:** A3 (TEXT/FILE/PTT alan listeleri) ve A4 (okundu-bilgisi kapsamı: M3a'da `delivered`, `read` sonra) — Hayri onaylar; A1 (gövde encoding: protobuf yok, sabit-genişlik + uzunluk-önekli) ve A2 (limitler: `MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − 5)` vb.) — Abdurrahman onaylar.
**Nereye:** `specs/application-message-protocol.md` §8 (metinler §2, §3, §5).
**Done:** §8'de dört satır "KAPANDI".

**DURUM (2026-10-04): Abkarada tarafı KAPANDI (K1).** AMP A1 ve A2 kapandı; A3'te byte düzeltmesi işlendi (`msg_id`/`ref_msg_id` u32, `sent_at_ms` telde yok). Kalan: A3 ve A4 için Hayri imzası.

### B1.2 — Mesaj formatı vektörleri
**Neden:** Her mesaj tipinin exact byte'ları sabit olmalı; ve **Rust codec'i kendi beklentisini üretmemeli** (A2.1'deki aynı ilke). Vektörler codec yazılmadan **önce**, spec'ten okunarak üretilir.
**Ne yapılacak:** her tip için 1 pozitif exact record; negatif corpus: reserved tip, reserved flag, `body_len` uyumsuz, sıkıştırma: `uncompressed_len` sınırı aşıyor, kesik zstd akışı, expansion oranı aşımı, geçersiz frame. Üretici: spec'ten yazılmış bağımsız mini Python encoder (stdlib; production kodla paylaşım yok).
**Nereye:** `vectors/amp-*.json`, `ref/amp/encode.py`. Tanım: AMP §2, §3, §5, §7.
**Neye bağlı:** B1.1, T-00.1.
**Done:** commit; her negatif vakada `result` tam olarak `drop` ya da `reject`.

### B1.3 — Sahte Transport (Application testleri için)
**Neden:** Application, gerçek Transport/Crypto olmadan geliştirilebilsin.
**Ne yapılacak:** `KamaTransport`'u uygulayan `MockTransport`: kayıp/sırasız/duplicate enjeksiyonu; `Ready / RenewRequired / Closed` senaryoları; ikinci `connect()`'in aynı oturumu döndürmesi; `ConnectFailed` enjeksiyonu; kapasite sabitleri. **Hub-RPC ve Crypto içermez.**
**Nereye:** `crates/kama-app/tests/common/mock_transport.rs`.
**Neye bağlı:** B1.0.
**Done:** mock-fixture yeşil.

### B1.4 — S1 sözleşme testleri (conformance-core)
**Neden:** Her test neyi kanıtlıyor:
- `H-ROUNDTRIP` — gönderilen mesaj byte-for-byte aynı geliyor.
- `H-EVENTS` — olay sırası `Ready → RenewRequired → Ready(yeni) → Closed(eski)`; kapanmış oturuma `send` → hata.
- `H-CUTOVER` — anahtar yenilenirken eski oturum yeni hazır olana kadar çalışır; geçişten sonra eski oturuma `send` → `SessionSuperseded`. **Bu davranış Crypto'nun iç sayacından bağımsız.**
- `H-CONNECT-IDEMP` — aktif oturuma ikinci `connect` aynı handle'ı döndürür, yeni olay üretmez.
- `H-EARLY-CONFLICT` — bekleyen/aktif oturuma "ilk mesajla connect" → `EarlyUnitConflict`; mesaj hiçbir yere gitmez.
- `H-CAPS` — tam sınırda kabul, sınır+1'de `UnitTooLarge`; sınır Transport'tan okunur, koda gömülü değil.
- `H-OPAQUE-FAIL` — bağlantı başarısızlığının sebebi (yanlış anahtar / bozuk cevap / zaman aşımı) dışarıdan **ayırt edilemez**.
- `H-COMPRESS-BOUNDARY` — sıkıştırma/açma yalnız Application'da; Transport bayrağa göre davranış değiştirmez.
**Nereye:** `crates/kama-app/tests/conf_s1_*.rs`. Tanım: `KM-SEAM-AT-001.md` §6.
**Neye bağlı:** B1.0, B1.3.
**Done:** `MockTransport`'a karşı yeşil.

### B1.5 — ~~Bağlantı zaman aşımı testi~~ Bağlantı niyeti testleri: H-CANCEL / H-PROGRESS / H-CUTOVER(Superseded) **[DR-CONNECT-1, 2026-10-03]**
**Neden:** ~~KS-8 belirlenince `ConnectFailed`'ın tam zamanlaması test edilebilir.~~ `connect()` için deadline yok ve `ConnectFailed` kalktı (DR-CONNECT-1). Test edilecek olan artık bir süre değil, üç davranış: **H-CANCEL** (`cancel(ticket)` → `ConnectAbandoned(ticket, Cancelled)`; sanal saat ilerlese de niyet kendiliğinden sonlanmaz; bozuk/sahte RESPONSE olay üretmez) · **H-PROGRESS** (link yok → `Suspended` + 0 prob; link gelir → `Measuring` + anında prob; eşin canlılığı bildirilmez) · **H-CUTOVER(Superseded)** (~~bilinen eşten yeni INIT → `Ready(h') → Closed(h, Superseded)`; duplicate INIT supersession üretmez~~ **[2026-10-04]** provisional supersession: onaydan önce `h` tam çalışır; onayda `Ready(h')` → yalnız-alım drain → `Closed(h, Superseded)`; bayat INIT `h`'yi etkilemez; glare → tek `Ready`) · **H-SILENT-REJECT** [2026-10-04] (sahte/geçersiz RESPONSE olay üretmez, pending yaşar). Tanım: `KM-SEAM-AT-001.md` §6.
**Nereye:** `conf_s1_connect.rs`. **Bağlı:** A1.3. **Done:** yeşil.

### 🚪 G-S1 — S1 SEAM-FROZEN
B1.0 derleniyor · B1.1 kapalı · B1.2 commit · **B1.3 / B1.4 / B1.5 yeşil** · C1.2 yeşil · ~~KS-8 verili (B1.5 onun davranışsal testi)~~ DR-CONNECT-1 S1'e işlendi (B1.5 = H-CANCEL/H-PROGRESS/H-CUTOVER(Superseded)) + Hayri imzası. **Kripto vektörü beklemez** — S3'ten önce donabilir.

### B2.1 — Mesaj codec'i
**Ne:** encode/decode; sınırlı açma (decompression bomb koruması); A2 limitleri; zstd. **Nereye:** `crates/kama-app/src/amp/`. **Bağlı:** G-S1. **Done:** B1.2 byte-for-byte.

### B2.2 — Metin mesajı + teslim bilgisi
**Ne:** TEXT gönder/al; `RECEIPT(delivered)` — **Transport ACK'inden türetilmez**, uygulama kendi üretir. **`msg_id` davranışı (AMP §3, A3):** gönderen tarafında session-scope'ta monoton artan u64; aynı session'da duplicate üretilmez; **alıcı teslim sırasını `msg_id` ile zorlamaz** — sıra Transport'un Reliable teslimiyle gelir; `msg_id` yalnız UI sıralaması, duplicate tespiti ve `RECEIPT.ref_msg_id` için. **Done:** S1 core; duplicate `msg_id` üretilmediği ve alıcının sırasız `msg_id`'yi reddetmediği testleri yeşil.

### B2.3 — Dosya transferi
**Neden:** Dosya parçalara bölünür, her parça bir mesaj. **Kural:** Application parça yeniden-gönderimi, parça ACK'i, parça MAC'i **yapmaz** — kayıp Transport'un, bütünlük Crypto'nun işi. Application yalnız: `transfer_id`, offset, bütün-dosya hash'i, birleştirme.
**Nereye:** `kama-app`. Tanım: AMP §3; `app-rpc` §14.
**Done:** `MockTransport` altta kayıp/sırasızlık enjekte eder **ama `Reliable` teslim sözleşmesi gereği sonunda her `PreparedUnit`'i teslim eder** (kurtarma Transport'un işi); Application kendi chunk retransmit/ACK mekanizmasını **çalıştırmaz**; sonuçta `hash(kaynak) == hash(hedef)`. Hash tutmazsa çözüm `FILE_CHUNK` retry eklemek DEĞİL, Transport/mock'un Reliable sözleşmesine bakmaktır. Application kodunda retransmit yolu yok (`H-APP-RETRY-0`).

### B2.4 — PTT (bas-konuş) yaşam döngüsü
**Ne:** `PTT_OPEN/CLOSE`; codec ve bundle süresi basış boyunca **sabit**; ses verisinin kendisi media ekibinin (bu yalnız kap). **Done:** S1 core.

### B2.5 — Bağlantı açma ve anahtar yenileme davranışı
**Ne:** `ConnectResult`, `SessionSuperseded`, `EarlyUnitConflict` gerçek kodda. **Done:** `H-CUTOVER`, `H-CONNECT-IDEMP`, `H-EARLY-CONFLICT`.

### 🚪 G-IMPL-S1 — Application IMPLEMENTATION-GO
Aynı vektörler + aynı conformance-core; Crypto/hub-RPC bağımlılığı sıfır (statik).

---

## WS-C — ~~Hasan~~ karar Abkarada / icra Claude Code (test/CI) — bugün başlar
*[2026-10-04, sahip kararı] WS-C'nin (T-00, C1.2–C1.5) sahibi: **karar Abkarada / icra Claude Code**. Hasan bu katmana girerse devralır.*

### C1.1 — T-00.1–T-00.4 (yukarıda)

### C1.2 — Mimari kuralları otomatik kontrol et
**Neden:** Kural "Application Crypto'yu görmez, Crypto Application'ı görmez" belgede yazıyor ama bir geliştirici `Cargo.toml`'a bir satır ekleyince sessizce bozulur. Otomatik kontrol şart. (`cargo deny` bunu yapamaz — o crate sürüm/lisans için.)
**Ne yapılacak:** `cargo metadata` çıktısını okuyan stdlib-Python betiği: (a) application crate'i `kama-noise-wire`/`crypto`/`ccr-codec`'e bağımlı olamaz (`H-NO-CRYPTO-EDGE`) · (b) application crate'inde hub-RPC çerçeve tipleri (`AppRpcFrame`, `method_id`) geçemez (`H-NO-PLANE1`) · (c) kripto crate'leri application crate'ine bağımlı olamaz (`H-SPLIT`).
**Nereye:** `scripts/ci/layer_check.py` + `ci.yml` `static` job'una adım. Kural: `specs/transport-layer-tdd.md` §0 INV-NEIGHBOR-1.
**Neye bağlı:** B1.0 (crate adı).
**Done:** ihlal eden deneme PR'ı kırmızı; mevcut ağaç yeşil.

### C1.3 — Test koşucusu
**Ne:** aynı conformance-core testlerini hedef seçerek (`mock` / `real`) koşturma. **Nereye:** `kama-integration/tests/common/`. **Done:** parametrik koşum çalışıyor.

### C1.4 — Beklenen çıktının koda gömülmesini engelle
**Neden:** Sözleşme testinde "beklenen byte'lar" Rust dosyasına yazılırsa, Rust kodu kendi kendini doğrular (A2.1 ilkesi bozulur). Beklenen değerin **tek** kaynağı `vectors/` dosyaları olmalı. **Ama** protokol kodunda `0x01`, `0xF0`, bayrak maskeleri gibi sabitler meşru — bunlar yasak değil. Kural dar: *sözleşme testlerinde* beklenen çıktı literal'i yasak.
**Ne yapılacak:** yalnız `tests/conf_*.rs` dosyalarını tarayan betik: inline beklenen byte-array/hex literal → hata; exact-byte assertion yalnız vektör `id`'si üzerinden; `expected_hex` yalnız yükleyiciden. Mevcut `tv_*`/`golden*` dosyaları **kapsam dışı** (donmuş transport'un kendi golden'ları, dokunulmaz).
**Nereye:** `scripts/ci/no_inline_expected.py`.
**Done:** `conf_*` altında literal beklenen değer → kırmızı; ürün kodu etkilenmiyor.

### C1.5 — Kopya hex yardımcılarını tekleştir
**Ne:** yeni `conf_*` testleri `kama-vectors`'daki tek `hex` yardımcısını kullanır; eski test dosyaları dokunulmaz. **Done:** yeni testlerde kopya `fn hex()` yok.

---

## WS-D — Transport geçişi (Abdurrahman ya da Resul) — kripto çekirdeğini BEKLEMEZ, INT-02'yi AÇAR

### D1 — Yeniden-gönderim kimliğini kripto sayacına taşı + kanonik birim çerçevesi
**Neden:** S3 sözleşmesinde paket sayacını artık Crypto üretiyor (`seal()` → `meta.full_wire_pn`) ve kanonik birim çerçevesi `ccr` §35.6 `BatchPayload` (KARAR-1). Bugünkü transport ise kendi çerçevesinde (`[type][seq u16][unit_id u64]`, 11 B) kendi numaralandırmasını taşıyor. İkisi birlikte yaşayamaz; A4.1 yalnız derlemeyi garanti eder, davranışı değiştirmez.
**Ne yapılacak:** transport ARQ/ACK tablosu `meta.full_wire_pn` ile anahtarlanır (her yeniden gönderim yeni seal → yeni PN; tablo PN → mantıksal birim eşler — S3 K2/K4); interim 11 B çerçeve yerine `BatchPayload`; `SealArgs.wire_pn` üretimi kalkar, `ctx` verilir.
**Nereye:** `crates/client-core/src/runtime.rs` (seal/open çağrıları), `crates/client-core/src/wire.rs`, `crates/adaptive-transport` ARQ/ACK. Tanım: `KM-SEAM-TC-001.md` K2/K4, `ccr-format-spec.md` §35.6.
**Neye bağlı:** A4.1; D2 kararı.
**Done:** mevcut kayıp/kurtarma davranış testleri yeni kimlikle yeşil; interim çerçeve kodda yok.

### D2 — İki ACK ailesinden hangisi ürün ACK'i? (Tema E)
**Neden:** Bugün ürün ACK yolu engine `ACK_V2` (`adaptive-transport/src/ack.rs`); noise-wire'da ayrıca full-wire-PN aralıkları taşıyan `Control::Ack/Sack` ailesi kodlanmış ama tüketicisi yok. D1'den sonra ACK'in taşıdığı kimlik kripto sayacı olmak zorunda; iki ailenin birlikte kalması iki doğruluk kaynağı demek.
**Ne yapılacak:** önce **karar (spec amendment)**: ürün ACK'i hangi aile; diğeri silinir ya da açıkça "kullanılmaz" işaretlenir. Sonra kod.
**Nereye:** karar → `kama-noise-wire-tdd-v2.0.md` §2.57 + `adaptive-transport-engine-spec.md` ACK bölümü; kod → `adaptive-transport/src/ack.rs` veya `kama-noise-wire/src/control.rs`.
**Neye bağlı:** —
**Done:** tek ACK ailesi; spec'te amendment kaydı.
**DURUM (2026-10-04): KARAR KAPANDI (K3), KOD AÇIK.** Ürün ACK'i = engine `ACK_V2`; noise-wire `Control::Ack/Sack` (0x10/0x11) spec'te silindi; onay/keepalive taşıyıcısı = boş batch (OQ-14 kapandı). Kalan kod işi: `kama-noise-wire/src/control.rs` + `kama-transport-schema` + `kama-engine/src/recovered_map.rs`'ten Ack/Sack'in sökülmesi ve D1'de ACK_V2'nin `full_wire_pn`'e anahtarlanması (açık alt-soru: `dh_epoch` belirsizliği).

### D3 — Donmuş transport golden'larının güncellenmesi
**Neden:** D1 çerçeveyi ve numaralandırmayı değiştirdiği için `adaptive-transport/tests/tv_*.rs` ve `kama-transport-schema/tests/golden_v0.rs` golden'larının bir kısmı değişecek. Bunlar donmuş transport'un kanıtı; sessizce "düzeltilmemeli".
**Ne yapılacak:** etkilenen golden'ları listele, her değişikliği transport v1.x amendment'ıyla gerekçelendir, yeni değerleri türet.
**Nereye:** ilgili test dosyaları + `transport-layer-tdd.md` amendment kaydı.
**Neye bağlı:** D1.
**Done:** golden farkları amendment'ta listeli; CI yeşil.

### 🚪 G-D — Transport geçişi tamam
D1 + D2 + D3. INT-02'nin ön koşulu; kripto ve application kapılarından bağımsız.

---

# KATMAN 5 — Kapılar ve sıra

```
Ön-koşul  R-1..R-7                                   (Abdurrahman + Claude Code; yarım gün)
Gün 0     T-00 (~~Hasan~~ Abkarada/Claude Code) ──┬── A2.* vektörler           (Abdurrahman; sayı beklemez)
                         ├── B1.0–B1.4                (Hayri)
                         └── C1.2–C1.5                (~~Hasan~~ Abkarada/Claude Code)
Gün 0–1   A1 kalibrasyon oturumu                     (Abdurrahman; tek oturum)
Sonra     A3.* + B1.5                                (sayıya bağlı testler)
Paralel   WS-D transport geçişi (D1–D3)              (Abdurrahman/Resul; A4.1'e bağlı, G-S3'ü beklemez)
──────────────────────────────────────────────────
G-S1      S1 SEAM-FROZEN     ← B1.0–B1.5 + C1.2 + ~~KS-8~~ DR-CONNECT-1 (KS-8 değersiz kapandı); kripto vektörü BEKLEMEZ, önce donar
G-S3      S3 SEAM-FROZEN
──────────────────────────────────────────────────
G-S1 →    B2.* → G-IMPL-S1
G-S3 →    A5.* → G-IMPL-S3
G-IMPL-S1 + G-IMPL-S3 + G-D →   INT-02: uçtan uca mesaj (kama-integration)
          assert: Application retransmit = 0 · Crypto retransmit = 0
          (yeniden gönderim yalnız Transport'ta — negatifiyle: recovery OFF → kayıp KURTARILMAZ)
```

**SEAM-FROZEN** = kararlar kapalı + sayılar verili + vektörler commit + conformance-core çalışıyor + mock contract-conformant + mimari kontroller yeşil.
**IMPLEMENTATION-GO** = gerçek kod aynı vektörleri tüketiyor + aynı conformance-core'u geçiyor + sözleşme dışı davranış icat etmiyor.
**Süreç kuralı:** yeni konu → spec'e ekleme | sayı kalibrasyonu | test/kanıt | kod. Beşinci kutu yok; mimari yeniden açılmaz.
