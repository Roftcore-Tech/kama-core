# DR-CONNECT-1 — `connect()` davranışı: "kalıcı niyet, sınırlı deneme"

**Karar sahibi:** Abkarada · **Tarih:** 2026-10-03 (13 hüküm) + 2026-10-04 (tamamlama: 6 karar) · **Bu belge İNDEKSTİR:** normatif metin spec'lerde yaşar; burada yalnız hüküm + işlendiği yer durur.

**Sorun:** S1 `connect(peer)` için bir deadline sabiti (KS-8) öngörüyordu. Kanıt bunun kurulamayacağını gösterdi: transport hub kanalı kuruluşunda pes etmiyor; "Plane 1 `Reliable` deadline'ı" yapısal olarak yok; STORE_MESSAGE yanıtı "doğrudan teslim / mailbox" ayrımını senkron taşıyamıyor. **Karar:** NİYET zamanla ölmez; DENEME maliyeti sınırlanır; sonlandırma yalnız olay-tetiklidir.

## 13 hüküm

| # | Hüküm (özet) | Normatif metin |
|---|---|---|
| H1 | **INV-NO-IDLE-TEARDOWN:** oturum / bekleyen niyet zaman geçtiği için sonlandırılmaz. Çelişen identity §16 "inaktivite timeout 30 gün" (ve §8 tablosu) üstü çizili | transport-layer-tdd §0 · identity §16 |
| H2 | Sonlandırma nedenleri kapalı liste: (a) cancel · (b) karşı tarafın close'u · (c) anahtar hijyeni · (d) kalıcı yerel hata | transport-layer-tdd §0 · identity §16 · KM-SEAM-AT-001 §4 |
| H3 | Prob maliyeti sınırlı: tavan bağlama göre 8 s / 25 s / OS · link yokken 0 emisyon · anında prob · zorunlu jitter (SPEC-ONLY, altı beyanlı) | transport-layer-tdd §10 [AMENDMENT-10] |
| H4 | Hop heartbeat = prob = NAT keepalive — **yeni codepoint** (karar 2) | transport-layer-tdd §10 [AMENDMENT-10] · engine-spec §1.1 · ccr §35.15, §35.16 |
| H5 | **KS-PN-1 = 1024**; tavanda REBIND → floor prob → hop-ACK'le devam | transport-layer-tdd §12 [AMENDMENT-9] · engine-spec §3.3-B/§11/§11.1 · ccr §35.15 |
| H6 | Peer-restart supersession — **provisional** (karar 3) | identity §16 · KM-SEAM-AT-001 K7, §6 · KM-SEAM-TC-001 K10 |
| H7 | DELIVER → mailbox fallback normatif; silme yalnız ACK_RANGE | app-rpc §6.11 |
| H8 | SESSION_MAX_AGE → RenewRequired → cutover; timeout değil, anahtar hijyeni; **KS-AGE-1 = 7 gün** | identity §16 · ccr §35.15 · KM-SEAM-AT-001 §4 |
| H9 | S1 API: `ConnectFailed` kalktı; `cancel(ticket)`, `ConnectAbandoned(ticket, reason)` (opak değil), `ConnectProgress(ticket, phase)` | KM-SEAM-AT-001 §1, §3, §4, §6, §7, §8 · ayna `tests/common/seam/api.rs` |
| H10 | **KS-8: deadline yoktur**, sayı yok | KM-SEAM-AT-001 §4 · ccr §35.15 |
| H11 | T-18c: 7 günde ephemeral silinir, niyet korunur, yeni INIT | identity §21 T-18c, §16 · KM-SEAM-AT-001 §4 |
| H12 | Ürün notu: Google servissiz dağıtımda arka plan erişilebilirliği foreground service ister | docs/PARK-LISTESI.md P-3 |
| H13 | OQ-14 AÇIK kalır (hop heartbeat E2E değildir) — ve provisional onayın kritik yoluna girdi | identity §28 OQ-14 |
| — | **KS-HANDLE-1:** `HANDLE_TOMBSTONE` = S&F mailbox TTL (7 gün). Kanıt: spec + test simülatörü; gerçek hub repoda yok | ccr §35.7 |

## Tamamlama — 6 karar (2026-10-04)

| # | Karar | Normatif metin |
|---|---|---|
| 1 | **Ayna aynı PR'da:** spec ile test aynası birlikte değişir; "spec X, ayna Y" aralığı main'de bırakılmaz. `H-OPAQUE-FAIL` düştü, `H-CANCEL` / `H-PROGRESS` / `H-SILENT-REJECT` geldi. WS-C sahibi: karar Abkarada / icra Claude Code | `tests/common/seam/{api,target}.rs` · `scripts/ci/seam-coverage.json` · KM-SEAM-AT-001 §6 · sprint backlog WS-C |
| 2 | **H4 heartbeat = yeni codepoint:** `HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C`, HubChannel ctx, sıfır payload + opsiyonel u32 echo token, kendi sayacı yok; REFRESH-ONLY (slot yaratmaz), CS probe bütçesi ve INV-CS-7 dışında, RTT observation-only. **KS-HB-1 = 8 s / 25 s / OS.** ccr §35.16 "client keepalive YOK" üstü çizildi | transport-layer-tdd §10 [AMENDMENT-10] · engine-spec §1.1 · ccr §35.15, §35.16 · noise-wire §2.56 (çakışma-önleme notu) |
| 3 | **H6 provisional supersession:** yeni-sayaçlı INIT → provisional; eski oturum tam çalışır; onay = ilk AEAD-geçerli epoch-1 paketi; sonra N_g/T_g yalnız-alım drain → `Closed(h, Superseded)`; onaylanmayan half-open TTL ile düşer. Renew ↔ restart ayrımı gerekmez | identity §16 · KM-SEAM-AT-001 K7, §6 · KM-SEAM-TC-001 K10 |
| 4 | **Glare:** `dest_hash`'i küçük olan kazanır; kazanan gelen INIT'i sessiz düşer, kaybeden pending'ini bırakıp gelen INIT'i işler | identity §16 · KM-SEAM-TC-001 K10 · KM-SEAM-AT-001 K7 |
| 5 | **Jitter RNG'siz:** `hash(client_id ‖ deneme_sayacı) mod pencere`; pencere = backoff adımının %25'i | transport-layer-tdd §10 [AMENDMENT-10] madde 5 |
| 6 | **Küçükler:** `LongOutage` = 60 s, yalnız UI (**KS-UI-1**) · KS-AGE-1 = 7 gün · S3'e `CloseReason::Superseded` · K5 (link/power, UniFFI) → PARK, o gelene kadar ön-plan kadansı · Hayri imzası G-S1'in ön koşulu, merge'in değil | KM-SEAM-AT-001 §4, §8 · ccr §35.15 · KM-SEAM-TC-001 §1.2 · PARK P-4 |

## Günün paketi — 5 karar (2026-10-04b)

| # | Karar | Normatif metin |
|---|---|---|
| K1 | **AMP A1 ve A2 KAPANDI**; A3 byte düzeltmesi: `TEXT.msg_id` ve `RECEIPT.ref_msg_id` u32, `sent_at_ms` telden çıktı (TEXT başlığı 23 B → 11 B) | application-message-protocol §3, §8 |
| K2 | **Glare kaybedeninin `early_unit`'i:** Transport saklar ve kazanan oturum `Ready` olunca ilk normal `send()` olarak yollar; Application yalnız `ConnectAbandoned`'da yeniden verir | KM-SEAM-AT-001 K6, §8 |
| K3 | **D2 / Tema E + OQ-14 KAPANDI:** ürün ACK'i = engine `ACK_FRAME_V2` (WS-D1'de `full_wire_pn`'e anahtarlanır) · noise-wire `Control::Ack/Sack` (0x10/0x11) silindi · onay/keepalive taşıyıcısı = boş batch `BatchPayload{submsg_count = 0}` | ccr (BatchPayload notu) · noise-wire §2.56, §2.57, §4 adım 2 · identity §28 OQ-14, §16 · engine-spec §2 · PARK |
| K4 | **Jitter tohumu `dest_hash` değil:** `BLAKE2s-256(seed ‖ u32_be(deneme))[0..4] mod pencere`; seed = `hub_handle` (bind sonrası) / hop ephemeral public (bind öncesi) | transport-layer-tdd §10 [AMENDMENT-10] madde 5 |
| K5 | **Provisional tipleri:** `PROVISIONAL_TTL` (istemci; = `HALF_OPEN_TTL` değeri, ayrı ad) · `SessionResult::ProvisionalAndReply{new, supersedes, envelope}` · `SessionEvent::Confirmed(new)`; Transport `Ready(new)`'i `Confirmed`'de yayınlar | ccr §35.15 · identity §16 · KM-SEAM-TC-001 §1.2, K10 |

**Kripto maliyeti notu:** bu kararların hiçbiri mesaj başına anahtar hesabı eklemez. Anahtar işi: oturum başına bir handshake, 7 günde bir yenileme, restart'ta bir handshake. Mesaj = tek AEAD.

**Kanıt (K1–K5):** `docs/sprint_plan/A1-KALIBRASYON-KARAR-FOYU.md` → KANIT PAKETİ 2.

**H4'te neden yeni codepoint:** CS_PROBE soğuk-başlangıç ölçüm merdivenidir (bütçeli, 4 atış, u16 sayaç sarmaz — INV-CS-7; 8 s kadansta ≈6 günde tükenir); heartbeat sürekli canlılık + NAT keepalive'dır (sınırsız). İki mekanizma, iki invariant; bindirilmez. Codepoint seçimi tüm registry'ler taranarak yapıldı (engine-spec §1.1, noise-wire §2.56, client-core v0 kabuğu, kama-transport-schema, app-rpc method_id).

**Açık kalanlar:** ~~OQ-14 (provisional onayın taşıyıcısı — WS-D2) · glare kaybedeninin `early_unit`'i · jitter'da `client_id` ve hash fonksiyonunun adı · K10 provisional'ı taşıyan `SessionResult` biçimi (A4.1)~~ *(2026-10-04b: dördü de kapandı — yukarıdaki K2–K5)* · S1 için Hayri imzası (G-S1 ön koşulu) · kod tarafı: HubChannel ctx, heartbeat, K tavanı, jitter hepsi SPEC-ONLY · **yeni:** WS-D1'de engine ACK_V2'nin `full_wire_pn` anahtarlamasında `dh_epoch` belirsizliği (epoch-1 PN 0'dan başlar; silinen E2E ACK `acked_dh_epoch` taşıyordu).
