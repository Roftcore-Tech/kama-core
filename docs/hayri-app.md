# Hayri — Application (WS-B) durum defteri

**[TAŞIMA NOTU 2026-10-06 — DEVİR]** Hayri'nin bu sprintteki kalemleri (B1.*, B2.*) Abkarada'ya devredildi (iş emri 2026-10-06 §0): icra Claude Code, karar ve inceleme Abkarada. AMP §8 A3/A4 ve S1 §8 imzası Abkarada tarafından atıldı (2026-10-06).

> **YAMA KAYDI — 2026-10-05 [Hayri taşıması, iş emri §4 — karar Abkarada, uygulayan Claude Code]:** Bu defter `origin/feature/app`'ten (commit `f1da50ac`, Hayri) `feature/app-port` dalına taşındı. Yollar (`docs/seems` → `docs/seams`) ve durum satırları güncellendi. Hayri'nin özgün metni SİLİNMEDİ; geçerliliğini yitiren yerlerin üstü çizildi. `[TAŞIMA NOTU …]` ile başlayan her şey — ve böyle bir notun hemen altındaki madde listesi ya da kod bloğu — Hayri'nin değil taşımanın notudur ve Hayri'nin onayı DEĞİLDİR. Aynı şekilde taşımanın metnidir: "Taşıma sonrası durum" başlığı ve altındaki tablo, §7'nin tamamı, ve üstü çizili bir yolun hemen yanına yazılmış yeni yol (`docs/seams/…`). Bunların dışındaki her şey Hayri'nin özgün metnidir. Hayri adına hiçbir imza atılmadı; imza bekleyen yerler §7'de sayılı.

~~**Güncelleme:** 2026-10-04 · **Branch:** `feature/app` (local, `main`'den ayrılmış, **push edilmedi**)~~
**[TAŞIMA NOTU 2026-10-05]** Güncelleme: 2026-10-05 · içerik `feature/app-port` dalıyla (taban `main`) taşındı. `feature/app` `origin`'de duruyor (uç `f1da50ac`); taşıma o dala YAZMADI.
**Kapsam:** `SPRINT-BACKLOG-FREEZE.md` içindeki **WS-B — Hayri (application)** iş paketi.
**Rol:** Application katmanı sahibi (`application-message-protocol.md` §Sahip). App-RPC (Plane 1,
Client↔Hub) ve Crypto bu kapsamın **dışında** — onlar Abkarada'nın (Transport/kama-core) işi.

---

## 1. Mimari konum (neyi niye yapıyoruz)

```
Application (BEN)
    ↓ transport.send(session, PreparedUnit)   ← lokal Rust API, wire RPC DEĞİL
Transport / kama-core (Abkarada)
    ↓ App-RPC (Plane 1) + Session Model V3 (Plane 2 hot-path)
Crypto (Abkarada)
```

Application'ın tek komşusu Transport'tur (`transport-layer-tdd.md` §0 INV-NEIGHBOR-1). Bu
sözleşmenin adı **S1**, otorite belgesi ~~`docs/seems/KM-SEAM-AT-001.md`~~ `docs/seams/KM-SEAM-AT-001.md`. Application'ın ürettiği
mesaj içeriğinin byte formatı ise **AMP** (Application Message Protocol), otorite belgesi
`specs/application-message-protocol.md`.

Sprint disiplini: **önce sözleşmeyi testle kilitle (SEAM-FROZEN), sonra gerçek kodu o sözleşmeye
karşı yaz (IMPLEMENTATION-GO).** Şu an birinci aşamadayız.

---

## 2. Tamamlanan görevler

| Görev | Ne yapıldı | Konum | Doğrulama |
|---|---|---|---|
| **B1.0** | ~~`kama-app` crate'i + `KamaTransport` trait'i ve tüm S1 tipleri (`PreparedUnit`, `TransportCapabilities`, `SessionEvent`, `TransportError`, `ConnectResult`, `SessionHandle`, `ConnectTicket`, `PeerRef`, `CloseReason`, `DeliveryPolicy`, `CompressionPolicy`) — `KM-SEAM-AT-001.md` §1'deki Rust imzasına birebir~~ | ~~`crates/kama-app/{Cargo.toml, src/lib.rs, src/transport.rs}`~~ | ~~`cargo build/clippy/fmt -p kama-app` temiz~~ |
| **B1.1** | ~~AMP §8'deki A3 (TEXT/FILE_BEGIN/FILE_ABORT/PTT_OPEN alan listeleri) ve A4 (RECEIPT kapsamı: M3a=`delivered`, M3b=`read`) kararları KAPANDI. A1/A2 (Abdurrahman'ın onayı) dokunulmadı.~~ | ~~`specs/application-message-protocol.md` §3, §8~~ | ~~doküman diff'i~~ |
| **B1.2** | Bağımsız Python encoder (`ref/amp/encode.py`, stdlib-only Python 3.14 `compression.zstd`, production Rust koduyla sıfır paylaşım) + AMP kayıt vektörleri: ~~8 `msg_type` için pozitif + 1 sıkıştırılmış örnek (9 vaka), 9 negatif vaka~~ **[TAŞIMA NOTU 2026-10-06]** 10 `msg_type` (0x01–0x0A) için pozitif + 1 sıkıştırılmış örnek (11 vaka), 15 ret vakası (F0.5 + F07.4) (reserved type/flag, iç alan/body_len uyumsuzluğu, sıkıştırma bomb korumaları, gerçek zstd ile kesilmiş/geçersiz frame, bilinmeyen compression_id/msg_version) | `ref/amp/encode.py`, `vectors/amp-record-v1.json`, `vectors/amp-record-reject-v1.json` | ~~`scripts/vectors/validate.py vectors/` → 4 dosya, 24 vaka, 0 bulgu~~ |
| **B1.3** | `MockTransport` — `KamaTransport`'u uygulayan sahte Transport. Deterministik enjeksiyon metodları (`complete_connect`, ~~`fail_connect`~~, `request_renew`, `complete_renew`); Plane 1/Crypto içermez | `crates/kama-app/tests/common/mock_transport.rs` | smoke test (`mock_transport_smoke.rs`) yeşil |
| **B1.4** | ~~S1 conformance-core — `KM-SEAM-AT-001.md` §6'daki 8 test (H-ROUNDTRIP, H-EVENTS, H-CUTOVER, H-CONNECT-IDEMP, H-EARLY-CONFLICT, H-CAPS, H-OPAQUE-FAIL, H-COMPRESS-BOUNDARY)~~ | ~~`crates/kama-app/tests/conf_s1_seam.rs`~~ | ~~`cargo test -p kama-app` → 9 test yeşil (8 conf + 1 smoke)~~ |

### Taşıma sonrası durum

**[TAŞIMA NOTU 2026-10-05]** Yukarıdaki tablo Hayri'nin `feature/app`'teki kaydıdır; üstü çizili hücreler main'de artık o hâliyle geçerli değildir. Main'e taşınan hâl:

| Görev | Taşıma sonrası durum | Konum (main) | Doğrulama |
|---|---|---|---|
| **B1.0** | **TAŞINMADI** (iş emri: `src/transport.rs` bırakıldı). S1 tipleri ve `KamaTransport` trait'i B1 kararıyla (R-7(b)) `kama-contracts` crate'indedir; `kama-app` yalnız ona bağlıdır ve bugün öğe içermez. Özgün tiplerden farklar: `ConnectFailed` yok → `cancel(ticket)` + `ConnectAbandoned(ticket, reason)` + `ConnectProgress(ticket, phase)` (DR-CONNECT-1) · `CloseReason = {Superseded, UserClosed, PeerClosed, KeyHygiene}` (özgün: `Renewed, LocalClose, PeerClose, Error`) · `PeerRef` = `dest_hash`'in (32 B) opak sarmalayıcısı, kurucu `from_dest_hash` (özgün: `from_bytes(Vec<u8>)`) | `crates/kama-contracts/src/{s1.rs, handles.rs}` · `crates/kama-app/{Cargo.toml, src/lib.rs}` | `scripts/ci/layer_check.py` yeşil · `scripts/ci/seam-api-drift.py --seam s1` yeşil |
| **B1.1** | **TAŞINMADI — ~~İMZA BEKLİYOR (Hayri)~~.** **[TAŞIMA NOTU 2026-10-06]** imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi); `feature/app`'teki onay main'e yine taşınmadı. `feature/app`'teki A3/A4 onayı (commit `2159ef63`, 2026-09-23) main'de YOK. Main'de A1/A2 2026-10-04'te kapandı (K1) ve aynı kararla A3 alan listesi DEĞİŞTİ: `TEXT.msg_id` u64 → u32, `sent_at_ms` telden çıktı, `RECEIPT.ref_msg_id` u64 → u32. Hayri'nin onayı bu değişiklikten ÖNCEKİ listeye verilmişti (§7) | `specs/application-message-protocol.md` §3, §8 (taşımada DOKUNULMADI) | — |
| **B1.2** | **TAŞINDI**, sonra K1'e güncellendi. Taşıma PR'ında (`feature/app-port`) iki adımda: önce üç dosya `origin/feature/app` ile bit-özdeş alındı; ardından kodlayıcı K1 düzenine getirildi ve bayatlayan altı vaka Python'dan yeniden üretildi: `encode.text`, `encode.receipt-delivered`, `encode.text-compressed`, `decode.reserved-flags-nonzero`, `decode.inner-field-exceeds-body-len`, `decode.unknown-msg-version`. Vaka sayısı değişmedi (9 pozitif + 9 negatif). **[TAŞIMA NOTU 2026-10-06]** Bugün 11 pozitif + 15 ret (F0.5: 0x09/0x0A pozitifleri ve tip-bağımsız ret vakaları; F07.4: sıkıştırılmış MEDIA_BUNDLE ret vakası). Kodlayıcının yazıcısı kanonik biçime getirildi (ASCII kaçışlı, `input` anahtarları sıralı — CI'daki `validate.py --canonical` kapısı özgün biçimi SV-011 ile reddediyordu; yalnız biçim, içerik aynı). `ref/amp/encode.py --check` eklendi (dosyalar kodlayıcının bugünkü çıktısıyla özdeş mi); ~~CI'a bağlı DEĞİL~~ **[TAŞIMA NOTU 2026-10-06]** gecelik işte koşar (`.github/workflows/nightly.yml` `amp-vectors-check`, F0.6; PR CI'ında değil) (Python 3.14 `compression.zstd` ister; ayrıca iki vakadaki zstd baytları kitaplık sürümüne bağlı olabilir). Pozitif dosyanın `description` alanı düzeltildi: özgün metin "her msg_type için 1 pozitif vektör" diyordu, vektörler registry'deki on tipin sekizini (0x01–0x08) kapsıyor. **AÇIK (taşımada kapatılmadı; AMP §7 her tip için ister):** ~~0x09 SIGNALING_BLOB ve 0x0A MEDIA_BUNDLE pozitifleri yok~~ **[TAŞIMA NOTU 2026-10-06]** 0x09/0x0A pozitifleri var (F0.5) · reserved tip yalnız 0x00 ile sınanıyor (0xF0..0xFF yok) · "bilinmeyen ama reserved olmayan tip → DROP" vakası yok · `uncompressed-len-exceeds-max` vakasının girdisi expansion oranı kuralını da çiğniyor | aynı üç dosya | `scripts/vectors/validate.py vectors/ --canonical --min-files 2 --require-cross-check` → ~~10 dosya, 92 vaka, 0 bulgu~~ **[2026-10-06]** 10 dosya, 100 vaka, 0 bulgu |
| **B1.3** | **TAŞINDI**, güncel S1'e getirildi. `fail_connect` kalktı → `cancel(ticket)` (sözleşme metodu) ve `abandon_connect(peer, reason)` (kapalı nedenler). Eklenenler: pending INIT tavanı · sanal saat + link durumu + prob sayacı · sahte RESPONSE enjeksiyonu · kayıp enjeksiyonu + recovery anahtarı · sınırlı kuyruklar. TEK kopya: S1 süiti aynı dosyayı `#[path]` ile kendi mock hedefi yapar. Sahtenin modellemedikleri dosyanın başında sayılı (sırasız/duplicate enjeksiyonu ve provisional supersession dahil) | `crates/kama-app/tests/common/mock_transport.rs` (aynı yol) | `cargo test -p kama-app` → smoke yeşil · `seam_mock_fixture` f1–f12 yeşil |
| **B1.4** | **TAŞINDI — yer değişti.** Satırlar hedef-agnostik gövdeler olarak ortak süite yazıldı (aynı gövde ileride gerçek Transport'a karşı da koşar); `conf_s1_seam.rs` dosyası taşınmadı. 8 testten 7'sinin satırı süitte (altısının gövdesi özgün testlerden yazıldı; H-ROUNDTRIP'in gövdesi süitte zaten vardı ve özgün testin iddialarını kapsıyor); **H-OPAQUE-FAIL kalktı** (DR-CONNECT-1: `ConnectFailed` yok; yerine H-SILENT-REJECT). Eklenen beş satır: H-APP-RETRY-0, H-CANCEL, H-PROGRESS, H-SILENT-REJECT, H-PENDING-CAP | `crates/kama-integration/tests/common/seam/contract.rs` (koşum: `seam_conformance_core`) · beyan `scripts/ci/seam-coverage.json` | `scripts/ci/seam-conformance.sh --target mock` → §6'nın 14 satırından 14'ü koşuyor: 12 davranış satırı mock'ta + 2 statik satır (H-NO-CRYPTO-EDGE, H-NO-PLANE1) `layer_check.py`'de. 12 davranış satırının hepsi `mock_only`: gerçek Transport hedefi yok (GAP-SEAM-REAL-1); bir satırın sahtede koşmayan ya da zayıf kalan kısmı beyanın `needs` alanında yazılı |
| **B1.5** | Kart yeniden tanımlandı (aşağıda §3). H-CANCEL, H-PROGRESS, H-SILENT-REJECT bu taşımada yazıldı ve mock'ta koşuyor. **H-CUTOVER'ın provisional supersession kolu YAZILMADI** (sahte modellemiyor); H-CUTOVER'ın yenileme kolu B1.4'te | aynı süit (`conf_s1_connect.rs` diye bir dosya yok) | yukarıdaki koşum |

**Commit geçmişi** (`feature/app`, `main`'in üzerinde, ~~henüz push edilmedi~~ **[TAŞIMA NOTU 2026-10-05]** `origin`'e push edilmiş, uç `f1da50ac`):
```
9d8f3f72 test: AMP kayıt vektörleri (B1.2)
49f1569d test: S1 conformance-core — 8 KM-SEAM-AT-001 §6 testi (B1.4)
160731af test: MockTransport (B1.3)
2159ef63 docs: AMP A3/A4 kararlarını kapat (B1.1)
4510a538 feat: kama-app crate + S1 KamaTransport API tipleri (B1.0)
```

### B1.3/B1.4 geliştirilirken bulunan ve düzeltilen tasarım hatası

`MockTransport::close()` ilk yazımda handle'ı `superseded` kümesine ekliyordu — bu, kapatılmış
bir session'ın **süresiz** `SessionSuperseded` döndürmesine yol açıyordu. `KM-SEAM-AT-001.md` §6
H-EVENTS kuralı ise "`Closed` + `close()` sonrası `send` → `UnknownSession`" diyor. Düzeltme:
`close()` artık handle'ı tamamen unutur (`UnknownSession`); cutover'ın hemen ardından (`close()`
çağrılmadan önceki kısa pencerede) hâlâ `SessionSuperseded` döner — bunlar iki ayrı an, birbiriyle
çelişmiyor (H-CUTOVER ayrı, H-EVENTS ayrı test ediyor).

**[TAŞIMA NOTU 2026-10-05]** **Taşınan sahtede bu geçiş `close()` çağrısına değil `Closed` olayının TESLİMİNE bağlıdır** — ve bu bir yorum farkıdır, karar Abkarada + Hayri'nindir:

- Taşınan hâl: cutover'dan itibaren (S1 K7: "`Ready(h')` anından itibaren"; sahtede `complete_renew` çağrıldığı an — özgün sürümdeki gibi), `Closed(h, Superseded)` okunana kadar `send(h)` → `SessionSuperseded`; `Closed` okunduktan sonra `send(h)` → `UnknownSession` (`close(h)` çağrılsın çağrılmasın). Özgünden farklı olan yalnız bu ikinci geçiştir.
- Dayanak: S1 §6 H-EVENTS satırının yazılı hâli "`Closed` sonrası `send(h)` → `UnknownSession`"dir; satırda `close()` geçmez. K7 de aynı dili kullanır ("`Ready` gelmeden `send(h)` → `UnknownSession`").
- Özgün testlerle fark: özgün `h_events` `UnknownSession`'ı `close(h)` çağrısından SONRA bekliyordu (taşınan satır ikisini de sınar: çağrıdan önce ve sonra); özgün `h_cutover` `SessionSuperseded`'ı `Closed` okunduktan SONRA bekliyordu (taşınan satır `Ready(h')` ile `Closed` ARASINDA bekler).
- Uygulama koduna etkisi: `Closed`'ı işleyip `close(h)` çağıran kod iki okumada da doğru çalışır.

---

## 3. Şu anda bloklu olan: B1.5

~~**Ne:** `connect()` zaman aşımı testi (`conf_s1_connect.rs`).~~
~~**Neye bağlı:** `KS-8` — "Transport, `connect()` çağrısında karşı taraftan `RESPONSE`'u ne kadar
bekler" sayısı. Kalibrasyonu **Abdurrahman'ın** işi (sprint planı A1.3), henüz verilmedi:~~

- ~~`docs/seems/KM-SEAM-AT-001.md:104   ... Değeri [KALİBRASYON — KS-8].`~~
- ~~`docs/seems/KM-SEAM-AT-001.md:134   | ⬛ KS-8 | connect Plane 1 deadline değeri | Abkarada |`~~

~~**Önemli nüans:** G-S1 kapısı KS-8'i bekler ama Abdurrahman'ın **ağır kripto işini**
(A2.1-A2.6 vektör üretimi, A5.* gerçek kod) beklemez — doküman: *"KS-8 verili ... Kripto vektörü
BEKLEMEZ — S3'ten önce donabilir."* KS-8, A1.1/A1.2/A1.4 ile birlikte tek bir kalibrasyon
oturumunda kararlaştırılabilecek küçük, bağımsız bir sayı.~~

~~**Ayrıca gerekli (benim dışımda, Hasan'ın işi):** `C1.2` — mimari kuralları otomatik kontrol eden
script (`scripts/ci/layer_check.py`). Bu repoda henüz yok.~~

**[TAŞIMA NOTU 2026-10-05]** Bu bölümün dayandığı iki şey değişti:

- **KS-8 sayısız kapandı (2026-10-03, DR-CONNECT-1 H10):** `connect()` için deadline YOKTUR; `ConnectFailed` kalktı. Bekleyen niyet yalnız üç yolla biter: `Ready` · `cancel(ticket)` · `ConnectAbandoned(ticket, reason)` (`docs/seams/KM-SEAM-AT-001.md` §4, §8; `docs/decisions/DR-CONNECT-1.md`). Dolayısıyla B1.5 artık bir "zaman aşımı testi" değildir.
- **B1.5 yeniden tanımlandı** (`docs/sprint_plan/SPRINT-BACKLOG-FREEZE.md` B1.5): H-CANCEL · H-PROGRESS · H-CUTOVER'ın provisional supersession kolu · H-SILENT-REJECT. İlk ikisi ve sonuncusu bu taşımada yazıldı (mock'ta koşuyor); provisional supersession kolu YAZILMADI.
- **C1.2 kapandı (2026-10-04):** `scripts/ci/layer_check.py` repoda ve CI'da (`static` job). WS-C'nin sahibi artık "karar Abkarada / icra Claude Code"dur (aynı belge, WS-C başlığı).

---

## 4. Kapı durumu

~~🚪 G-S1 (S1 SEAM-FROZEN) —
B1.0 ✅ · B1.1 ✅ · B1.2 ✅ · B1.3 ✅ · B1.4 ✅ · B1.5 ⬛ (KS-8 bekliyor) · C1.2 ⬛ (Hasan, yok)
→ KAPANMADI~~

**[TAŞIMA NOTU 2026-10-05]** Durum dökümü — **kapı kararı DEĞİLDİR** (karar Abkarada'nın):

```
🚪 G-S1 (S1 SEAM-FROZEN)                                                    [2026-10-05, taşıma sonrası]
   B1.0  derleniyor — tipler kama-contracts'ta (kama-app'te öğe yok)
   B1.1  ⬛ Hayri imzası bekliyor (AMP §8 A3, A4) — Abkarada tarafı kapalı (K1)
   B1.2  main'e taşındı, K1'e güncel; validate.py 0 bulgu
   B1.3  main'e taşındı; smoke + mock-fixture yeşil
   B1.4  7 satır mock'ta yeşil (H-OPAQUE-FAIL kalktı)
   B1.5  H-CANCEL / H-PROGRESS / H-SILENT-REJECT mock'ta yeşil; provisional supersession kolu YOK
   C1.2  kapalı (layer_check.py CI'da)
   S1    ⬛ Hayri imzası bekliyor (S1 §8)
   → KAPANMADI
```

**[TAŞIMA NOTU 2026-10-06]** Yukarıdaki döküm 2026-10-05 anlık görüntüsüdür. B1.1 (AMP §8 A3, A4) ve S1 §8 imzaları Abkarada tarafından atıldı (2026-10-06; devir, iş emri 2026-10-06 §0). G-S1: L-F07 main'e girince FROZEN (iş emri 2026-10-06 §8 ön-yetki).

```
🚪 G-IMPL-S1 (Application IMPLEMENTATION-GO)
   B2.1-B2.5 (gerçek mesaj codec'i + bağlantı davranışı) — G-S1 kapanmadan BAŞLANMAZ
   → sprint disiplini: "Beşinci kutu yok; mimari yeniden açılmaz"
```

---

## 5. Sırada ne var

1. ~~**KS-8'i Abdurrahman'dan al** (muhtemelen A1.1/A1.2/A1.4 ile birlikte tek kalibrasyon
   oturumunda) → B1.5'i yaz.~~ **[TAŞIMA NOTU 2026-10-05]** KS-8 sayısız kapandı; B1.5'in kalan parçası H-CUTOVER'ın
   provisional supersession kolu (§3).
2. ~~**C1.2'yi Hasan'dan bekle/iste** → G-S1 kapısı kapanır.~~ **[TAŞIMA NOTU 2026-10-05]** C1.2 kapalı; G-S1 için
   kalanlar §4 dökümünde (iki imza + B1.5'in kalan kolu).
3. G-S1 kapanınca **B2.1-B2.5**'e başla: gerçek `AppMessageCodec` (encode/decode, B1.2
   vektörlerine karşı byte-for-byte), TEXT+RECEIPT akışı, dosya transferi (retransmit/ACK/MAC
   YOK — bunlar Transport/Crypto'nun), PTT yaşam döngüsü, gerçek `ConnectResult`/
   `SessionSuperseded`/`EarlyUnitConflict` davranışı.
4. B2.* bitince **G-IMPL-S1** → WS-B tamamlanır.
5. Sprint'in tamamı için WS-B (ben) + WS-A'nın (Abdurrahman) **G-IMPL-S3**'ü birlikte
   **INT-02**'de (uçtan uca mesaj testi) buluşmalı; `Application retransmit = 0 · Crypto
   retransmit = 0` doğrulanmalı.

---

## 6. Referans belgeler

- `SPRINT-BACKLOG-FREEZE.md` — bu sprintin tam plan dokümanı (görev kartları, kapılar). **[TAŞIMA NOTU 2026-10-05]** yol: `docs/sprint_plan/SPRINT-BACKLOG-FREEZE.md`.
- ~~`docs/seems/KM-SEAM-AT-001.md`~~ `docs/seams/KM-SEAM-AT-001.md` — S1 (Application↔Transport) sözleşme otoritesi.
- `specs/application-message-protocol.md` — AMP (mesaj içeriği formatı) otoritesi.
- `vectors/README.md`, `vectors/SCHEMA.json` — vektör dosya formatı (Hasan'ın T-00.1'i).
- **[TAŞIMA NOTU 2026-10-05]** `crates/kama-contracts/src/s1.rs` — S1'in derlenen hâli (trait + tipler); `docs/decisions/DR-CONNECT-1.md` — `connect()` = kalıcı niyet kararının indeksi.

---

## 7. İmza bekleyen yerler (Hayri)

**[TAŞIMA NOTU 2026-10-05]** Taşıma Hayri adına imza TAŞIMAZ ve atmaz. ~~Aşağıdaki yerler Hayri'nin imzasını bekliyor;~~ **[TAŞIMA NOTU 2026-10-06]** aşağıdaki üç yerin imzası Abkarada tarafından atıldı (2026-10-06; Hayri'nin kalemleri bu sprint Abkarada'ya devredildi — Hayri adına imza DEĞİLDİR); işaret zaten ilgili belgelerde duruyor, burada yalnız toplandı:

| Yer | Bekleyen şey | Not |
|---|---|---|
| `specs/application-message-protocol.md` §8 **A3** | TEXT / FILE_BEGIN / FILE_ABORT / PTT_OPEN alan listeleri (PTT: + Yaaz) | Satırda "imza Hayri'de" yazıyor. `feature/app`'teki onay (`2159ef63`, 2026-09-23) K1'den (2026-10-04) ÖNCEKİ listeye verilmişti: TEXT `msg_id u64 · sent_at_ms u64 · text_len u16 · text`, RECEIPT `ref_msg_id u64 · kind u8`. Güncel liste: TEXT `msg_id u32 · text_len u16 · text`, RECEIPT `ref_msg_id u32 · kind u8`. O onay main'e taşınmadı **[TAŞIMA NOTU 2026-10-06]** imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi) |
| `specs/application-message-protocol.md` §8 **A4** | RECEIPT kapsamı: M3a'da `delivered`, `read` M3b | `feature/app`'te aynı commit'le onaylanmıştı; main'de satır PROPOSED. Taşınmadı **[TAŞIMA NOTU 2026-10-06]** imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi) |
| `docs/seams/KM-SEAM-AT-001.md` §8 **⬛ İMZA** | DR-CONNECT-1 API değişikliği (`cancel` / `ConnectAbandoned` / `ConnectProgress` / `ConnectFailed`'ın kalkması) | Belge başlığına göre iki imza olmadan sözleşme değildir; belgenin başındaki 2026-10-03 … 2026-10-06 tarihli yama kayıtları tek imzalıdır (Abkarada). İmza G-S1'in ön koşuludur, merge'in değil **[TAŞIMA NOTU 2026-10-06]** imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi) |

**[TAŞIMA NOTU 2026-10-05]** Ayrıca Hayri'nin görmesi gereken, onun kodunu etkileyen iki şey: `Closed` sonrası tutamaç semantiği (§2'deki taşıma notu; karar bekliyor) ve `kama-app`'in artık S1 tiplerini kendisi tanımlamayıp `kama-contracts`'tan alması.
