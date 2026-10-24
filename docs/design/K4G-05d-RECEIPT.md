# K4G-05d — Bekleyen RECEIPT kuyruğu (R3) — tasarım notu

## 0. Künye
**K4G-05d** · dal `k4g/05d-receipt` · taban `main 3439fccd` (yalın `:n` = `crates/kama-app/src/messaging.rs`) · P0 `k4g/p0-taban` `4b7b27f1` · **TASLAK**: kod "K4G-05d ONAYLANDI" bekler; uygulanmadı, koşulmadı. Girdi: C9 §3 K4G-05d, §5; P0-OZET (R3, R3-tekil KIRMIZI); `verify-wf_f84f4d0e` "R3". **Kapsam dışı: PTT açıkken dosya parça sınırı (DUR-21), Sprint 3.**

## 1. Değişmez
> Teslim edilen TEXT'in RECEIPT'i, aynı oturumda Transport kabul edene dek eş başına sınırlı biçimde bekletilir; oturum değişimini beklemez (oturum değişince görev tekrar TEXT'e, S2, geçer). Kabulden sonrası Transport'undur.

## 2. Mekanizma (bugün) [kod]
`on_text` (:800-822) her TEXT'te `send_receipt`'i (:1110-1136) çağırır. Bu tek bir `send(.., Reliable)` yapar; `Err` ya da oturum yoksa yalnız `receipts_unsent` artar (:1133-1135).

`PeerState`'te (:858-878) bekleyen RECEIPT alanı yok. Tek yeniden sunum yolu `flush_resends`'tir (:653, :771-778 → `flush_peer` :1084-1107). Bu yol yalnız outbox TEXT'ini kapsar ve tutamaç başına bir kez sunar (:1093-1095). Tekrar TEXT ancak oturum değişiminde gelir (`adopt` :707-746).

Sonuç: `Backpressure` (`transport.rs:1212`; tetik: `files.pump` kuyruğu doldurur, `node.rs:464`) alan RECEIPT, gönderenin `Delivered`'ını oturum ömrünce (7 gün / 2^31 PN: `session_control.rs:87`, `protector.rs:42`) geciktirir; outbox yuvası dolu kalır. Yanlış "teslim edildi" yok.

## 3. Tasarım [tasarım]
1. **Durum.** `PeerState.pending_receipts: VecDeque<MsgId>`: FIFO, `(eş, msg_id)` tekil, eş başına ≤ `MAX_OUTBOX_UNACKED` = 256 (:162). Yeni sayı yok.
   S7'ye uyan gönderenin RECEIPT'siz TEXT'i ≤ 256'dır (`next_send_id` :934-936), yani kuyruk ancak uyumsuz eşte dolar. Bellek ≤ 64 KiB; kalıcı değil.
2. **Üretim** (`owe_receipt`).
   - Oturum yoksa RECEIPT bırakılır.
   - Varsa ve `msg_id` kuyrukta değilse sona eklenir (sınır denetimi eklemeden önce, SR-AUDIT-001). Ardından kuyruk baştan sunulur.
   - Kuyruk boşken bu bugünkü tek çağrıdır.
3. **Sunum** (baştan). `Ok` → çıkar, `receipts_sent`. `Backpressure` → dur (`flush_peer` deseni). Diğer `Err` → bırak, devam: oturum sonu (uç `None`, `transport.rs:873`/:1262) ya da sözleşme dışı. Yalnız `Backpressure` geçicidir; `file_transfer.rs` `is_transient`'tan (:1629-1637) bilinçli fark (orada alındı oturumlar arası bekler).
4. **Yeniden sunum noktası.** `Messenger::poll` başı, `flush_resends`'ten (:653) hemen sonra. Zamanlayıcı yok.
   `turn()` sırası değişmez: tur boşalmışsa `messenger.poll` `None` döndürmüştür (`ptt.rs:679-680`), yani sunum `files.pump`'tan önce koşar (`node.rs:458` → :464); trafiksiz de her `NODE_TICK`'te tur döner (`node.rs:27-28`).
5. **`Ok` sonrası.** Kayıt düşer; aynı oturumda bir daha sunulmaz. Sonraki kayıp Transport Reliable'ındır; bu ikinci bir ağ ARQ'su değildir.
6. **Oturum değişimi / kapanış: DÜŞÜR.** `adopt` (:739 `replace`'ten önce) ve `retire` (:756) kuyruğu boşaltır; düşen her kayıt `receipts_unsent`'e yazılır. Gerekçeler:
   - Yeni oturumda gönderen RECEIPT'siz her TEXT'i yeniden yollar (:744-745), alıcı RECEIPT'i yeniden üretir (:800-810). Kaydı taşımak ikinci bir RECEIPT olurdu.
   - Kuyruktaki her `msg_id` gönderenin outbox'ındadır; telafi kesindir.
   - Oturum yalnız :739/:756'da değişir. Dolu kuyruk oturumun `Some` olduğunu gösterir; tutamaç alanı gerekmez.
   - Oturumsuz kuyruk yaşamaz. `file_transfer.rs` süre dolumu (:1243) kopyalanmaz: TEXT gönderenin zamanlayıcısı yok, TTL R3'ü geri getirirdi.
7. **Taşma [DUR-05d-1]** (C9'da tanımsız). En eski kayıt düşer; `receipts_evicted` ve `receipts_unsent` artar.
   - Emsal: `file_transfer.rs:1220-1224` ve AMP §3 sinyalleşme kuralı (`specs/application-message-protocol.md:99`).
   - Alternatif: yeni kaydı reddetmek.
8. **Sayaçlar** (`MessengerStats` :329-358) **[DUR-05d-2]**.
   - `receipts_sent` aynı kalır.
   - `receipts_unsent` **yeni anlam**: kabul görmeden bırakılan RECEIPT (oturum yok · `Backpressure` dışı ret · oturum değişimi/kapanış · taşma).
   - Yeni: `receipts_deferred` (kuyruğa giren yeni kayıt), `receipts_evicted` (`unsent` alt kümesi), `receipts_pending` (`stats()` anındaki anlık toplam; sayaç değil), `Messenger::pending_receipts(peer)`.
   - RECEIPT sunumu `resent_on_new_session` / `resend_refused`'a yazılmaz. INT-02 `app_retransmit` = bu ikisinin toplamı (`k4g/05-int02` `int02_node.rs:2497-2500`).
9. **Beyanlı güncelleme (DUR-8 biçimi).** Belge cümleleri değişir: :78-80, :90-91, :186-187, :340-343, :646-647, :1110-1111 → Ek A.
   Eski davranışı sabitleyen test **yok**: `receipts_unsent` yalnız :343/:1134'te; bütün-yapı `MessengerStats` karşılaştırması yok; `text_receipt.rs:813`/`:1304` yalnız A→B'yi doldurur; `lose_next` testleri (:615, :964) RECEIPT'i kabul ettirip kaybeder. 2×2 gerekmez; mevcut test kırmızısı → DUR.

## 4. Dokunulan dosyalar
- `messaging.rs` (+ `mod tests` taşma testi).
- `crates/kama-app/tests/k4g_p0_receipt.rs`: `4b7b27f1` cherry-pick, iddialar aynen; yeni testler de buraya.
- **Dokunulmaz:** AMP/tel, S1 (`s1.rs` K8), `kama-node` (tur, `NodeMetrics`), `kama-mobile`, `file_transfer.rs`, `client-core`, `specs/`.
- **Spec yaması yok.**
  - AMP §3 (:68, :75) tekrar TEXT'e RECEIPT'i yazar, reddi yazmaz.
  - S1 K8 "bekler"e izin verir (`s1.rs:8-9`, `KM-SEAM-AT-001.md:121`).
  - `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:242` "aynı oturumda zamanlayıcılı retry yok" ile tutarlı: reddedilen birim tele çıkmamıştır, zamanlayıcı yok.

## 5. Kanıt planı
**P0.** `k4g_p0_receipt.rs` `p0_r3_receipt_survives_backpressure` (:662-710) ve `p0_r3_receipt_dedup_bounded` (:732-802) tabanda KIRMIZI. 05d sonrası iddialar değişmeden yeşil olmalı.

**Yeni testler.** Tabanda kırmızı beklenen test yeşil çıkarsa DUR.

| Test | İddia | Tabanda |
|---|---|---|
| T1 taşma | uyumsuz eş, 257 `msg_id`, B→A dolu → kuyruk 256, `evicted` 1, düşen en eski | kırmızı |
| T2 `Ok` sonrası | `lose_next` ile kaybolan RECEIPT `IDLE_ROUNDS` boyunca yeniden sunulmaz | yeşil (bekçi) |
| T3 oturum değişimi | bekleyen RECEIPT(X) + supersede → kuyruk 0; `h'` üzerinde RECEIPT(X) tam bir kez, tek `Delivered` | yeşil (bekçi) |
| T4 sınıf | oturum yok / `UnknownSession` → kuyruğa girmez | yeşil (bekçi) |
| T5 ilk ret | `poll` başına eş başına ≤ 1 reddedilen sunum | yeşil (bekçi) |
| T6 sayaç | `pending` = toplam; kapasite açılınca 0; `deferred` ≥ 1 | kırmızı |

**Mutasyon hedefleri:** tekilleştirme yok / `Ok`'ta çıkarmama → P0-2 · `poll` başı sunum yok → P0-1 · boşaltmama → T3 · sınır yok / yeniyi düşür → T1 · geniş geçici sınıf → T4 · `Backpressure`'da devam → T5 · `pending` = 0 → T6. Mevcut testler değişmeden yeşil.

**INT-02 hükmü** ("bekleyen receipt = 0").
- İki düğümün son sondasında `NodeProbe::messenger.receipts_pending` = 0 olmalı (`k4g/05-int02` `probe.rs:62`, `node.rs:813`).
- Düzenek `int02_node.rs:2452-2465` `app` nesnesine üç alanı ekler. Bu INT-02 dalının işidir; 05d `kama-node`'a dokunmaz.
- `receipts_unsent` 05d öncesi/sonrası kıyaslanmaz.

## 6. Bağımlılık · süre · risk · DUR
**Bağımlılık:** yok (05b/05c'den bağımsız). **Süre:** ≈ 0,5 iş günü.

**Riskler.**
- `Backpressure` sürerken her `poll` eş başına bir reddedilen `send` yapar (tur başına ≤ 1024, `node.rs:93`; `flush_resends` deseni). Maliyeti ölçülmedi.
- RECEIPT dosya parçalarının önüne geçer (kasıtlı).
- Kabul sonrası kayıp 05b/05c'nin konusu.

**DUR adayları:** 05d-1 taşma · 05d-2 üç pub sayaç alanı + erişimci · 05d-3 yalnız `Backpressure` geçici · 05d-4 düşür ↔ taşı. Mevcut test kırmızısı ya da tur sırası değişikliği → DUR (gerekmiyor).

---

## Ek A — beyanlı belge metinleri (öneri; `messaging.rs`)
- **:78-80** (modül, "# RECEIPT", son iki cümle) → "RECEIPT eşin güncel oturumundan gider. Transport `Backpressure` ile reddederse RECEIPT eş başına sınırlı bekleyen kuyruğa girer (`(eş, msg_id)` tekil, ≤ [`MAX_OUTBOX_UNACKED`]) ve sonraki [`Messenger::poll`] çağrılarında AYNI oturuma ilk kabule dek yeniden sunulur (K8 "bekler": reddedilen birim tele çıkmamıştır; zamanlayıcı yok). Kabulden sonrası Transport'undur. Oturum yoksa, ret başka bir hataysa ya da eşin oturumu değişir veya kapanırsa RECEIPT bırakılır: gönderen kaydı bir sonraki oturumda yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir. Kuyruk taşarsa en eski kayıt düşer."
- **:90-91** ("Retry yok" son cümlesine ek) → "Reddedilen RECEIPT de aynı kuralla bekler ("RECEIPT")."
- **:186-187** → "`RECEIPT(delivered)` bu olay üretilirken Transport'a sunuldu ya da (geri basınçta) bekleyen kuyruğa alındı."
- **:340-343** → `receipts_sent`: "Transport'un kabul ettiği RECEIPT sunumları (ilk sunum ya da bekleyen kuyruktan)." · `receipts_unsent`: "Transport'a kabul ettirilmeden bırakılan RECEIPT'ler: oturum yokken üretilen, `Backpressure` dışı bir hatayla reddedilen, eşin oturumu değişince ya da kapanınca kuyruktan düşen, kuyruk taşmasında düşen. Telafisi gönderenin oturum değişimindeki yeniden gönderimidir (S2)." · `receipts_deferred`: "`Backpressure` ile bekleyen kuyruğa giren RECEIPT (tekilleştirilen tekrar sayılmaz)." · `receipts_evicted`: "Kuyruk taşmasında düşen RECEIPT (`receipts_unsent`'in alt kümesi; S7'ye uyan gönderende 0)." · `receipts_pending`: "ANLIK değer, sayaç değil: `stats()` anında bütün eşlerin bekleyen RECEIPT sayısı."
- **:646-647** (`poll` "Sıra" cümlesi) → "(1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur; (2) bekleyen RECEIPT'ler eş başına baştan, ilk `Backpressure`'a dek sunulur; (3) Transport olayları, birer birer; (4) teslim edilen birimler."
- **:1110-1111** → "`RECEIPT(delivered)`'ı eşin bekleyen kuyruğuna koyar ve kuyruğu güncel tutamaca sunar (kuyruk boşken tek çağrı). Ayrıntı: modül belgesi, "RECEIPT"."
