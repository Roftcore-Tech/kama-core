# Hayri — Application (WS-B) durum defteri

**Güncelleme:** 2026-10-04 · **Branch:** `feature/app` (local, `main`'den ayrılmış, **push edilmedi**)
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
sözleşmenin adı **S1**, otorite belgesi `docs/seems/KM-SEAM-AT-001.md`. Application'ın ürettiği
mesaj içeriğinin byte formatı ise **AMP** (Application Message Protocol), otorite belgesi
`specs/application-message-protocol.md`.

Sprint disiplini: **önce sözleşmeyi testle kilitle (SEAM-FROZEN), sonra gerçek kodu o sözleşmeye
karşı yaz (IMPLEMENTATION-GO).** Şu an birinci aşamadayız.

---

## 2. Tamamlanan görevler

| Görev | Ne yapıldı | Konum | Doğrulama |
|---|---|---|---|
| **B1.0** | `kama-app` crate'i + `KamaTransport` trait'i ve tüm S1 tipleri (`PreparedUnit`, `TransportCapabilities`, `SessionEvent`, `TransportError`, `ConnectResult`, `SessionHandle`, `ConnectTicket`, `PeerRef`, `CloseReason`, `DeliveryPolicy`, `CompressionPolicy`) — `KM-SEAM-AT-001.md` §1'deki Rust imzasına birebir | `crates/kama-app/{Cargo.toml, src/lib.rs, src/transport.rs}` | `cargo build/clippy/fmt -p kama-app` temiz |
| **B1.1** | AMP §8'deki A3 (TEXT/FILE_BEGIN/FILE_ABORT/PTT_OPEN alan listeleri) ve A4 (RECEIPT kapsamı: M3a=`delivered`, M3b=`read`) kararları KAPANDI. A1/A2 (Abdurrahman'ın onayı) dokunulmadı. | `specs/application-message-protocol.md` §3, §8 | doküman diff'i |
| **B1.2** | Bağımsız Python encoder (`ref/amp/encode.py`, stdlib-only Python 3.14 `compression.zstd`, production Rust koduyla sıfır paylaşım) + AMP kayıt vektörleri: 8 `msg_type` için pozitif + 1 sıkıştırılmış örnek (9 vaka), 9 negatif vaka (reserved type/flag, iç alan/body_len uyumsuzluğu, sıkıştırma bomb korumaları, gerçek zstd ile kesilmiş/geçersiz frame, bilinmeyen compression_id/msg_version) | `ref/amp/encode.py`, `vectors/amp-record-v1.json`, `vectors/amp-record-reject-v1.json` | `scripts/vectors/validate.py vectors/` → 4 dosya, 24 vaka, 0 bulgu |
| **B1.3** | `MockTransport` — `KamaTransport`'u uygulayan sahte Transport. Deterministik enjeksiyon metodları (`complete_connect`, `fail_connect`, `request_renew`, `complete_renew`); Plane 1/Crypto içermez | `crates/kama-app/tests/common/mock_transport.rs` | smoke test (`mock_transport_smoke.rs`) yeşil |
| **B1.4** | S1 conformance-core — `KM-SEAM-AT-001.md` §6'daki 8 test (H-ROUNDTRIP, H-EVENTS, H-CUTOVER, H-CONNECT-IDEMP, H-EARLY-CONFLICT, H-CAPS, H-OPAQUE-FAIL, H-COMPRESS-BOUNDARY) | `crates/kama-app/tests/conf_s1_seam.rs` | `cargo test -p kama-app` → 9 test yeşil (8 conf + 1 smoke) |

**Commit geçmişi** (`feature/app`, `main`'in üzerinde, henüz push edilmedi):
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

---

## 3. Şu anda bloklu olan: B1.5

**Ne:** `connect()` zaman aşımı testi (`conf_s1_connect.rs`).
**Neye bağlı:** `KS-8` — "Transport, `connect()` çağrısında karşı taraftan `RESPONSE`'u ne kadar
bekler" sayısı. Kalibrasyonu **Abdurrahman'ın** işi (sprint planı A1.3), henüz verilmedi:

```
docs/seems/KM-SEAM-AT-001.md:104   ... Değeri `[KALİBRASYON — KS-8]`.
docs/seems/KM-SEAM-AT-001.md:134   | ⬛ KS-8 | `connect` Plane 1 deadline değeri | Abkarada |
```

**Önemli nüans:** G-S1 kapısı KS-8'i bekler ama Abdurrahman'ın **ağır kripto işini**
(A2.1-A2.6 vektör üretimi, A5.* gerçek kod) beklemez — doküman: *"KS-8 verili ... Kripto vektörü
BEKLEMEZ — S3'ten önce donabilir."* KS-8, A1.1/A1.2/A1.4 ile birlikte tek bir kalibrasyon
oturumunda kararlaştırılabilecek küçük, bağımsız bir sayı.

**Ayrıca gerekli (benim dışımda, Hasan'ın işi):** `C1.2` — mimari kuralları otomatik kontrol eden
script (`scripts/ci/layer_check.py`). Bu repoda henüz yok.

---

## 4. Kapı durumu

```
🚪 G-S1 (S1 SEAM-FROZEN)
   B1.0 ✅ · B1.1 ✅ · B1.2 ✅ · B1.3 ✅ · B1.4 ✅ · B1.5 ⬛ (KS-8 bekliyor) · C1.2 ⬛ (Hasan, yok)
   → KAPANMADI

🚪 G-IMPL-S1 (Application IMPLEMENTATION-GO)
   B2.1-B2.5 (gerçek mesaj codec'i + bağlantı davranışı) — G-S1 kapanmadan BAŞLANMAZ
   → sprint disiplini: "Beşinci kutu yok; mimari yeniden açılmaz"
```

---

## 5. Sırada ne var

1. **KS-8'i Abdurrahman'dan al** (muhtemelen A1.1/A1.2/A1.4 ile birlikte tek kalibrasyon
   oturumunda) → B1.5'i yaz.
2. **C1.2'yi Hasan'dan bekle/iste** → G-S1 kapısı kapanır.
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

- `SPRINT-BACKLOG-FREEZE.md` — bu sprintin tam plan dokümanı (görev kartları, kapılar).
- `docs/seems/KM-SEAM-AT-001.md` — S1 (Application↔Transport) sözleşme otoritesi.
- `specs/application-message-protocol.md` — AMP (mesaj içeriği formatı) otoritesi.
- `vectors/README.md`, `vectors/SCHEMA.json` — vektör dosya formatı (Hasan'ın T-00.1'i).
