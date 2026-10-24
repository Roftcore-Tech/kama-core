# K4G-05d — Bekleyen RECEIPT kuyruğu (R3) — tasarım notu (rev2)

## 0. Künye
**K4G-05d** · dal `k4g/05d-receipt` · taban `main 3439fccd` (yalın `:n` = `crates/kama-app/src/messaging.rs`) · P0 `k4g/p0-taban` `4b7b27f1` · rev1 `aa94c1a7` + inceleme `review-2026-10-24/05d.md` · **TASLAK**: kod "K4G-05d ONAYLANDI" bekler; uygulanmadı, koşulmadı. **Kapsam dışı: PTT açıkken dosya parça sınırı (DUR-21), Sprint 3.**

## 1. Değişmez (sahip metni, C9 §3 K4G-05d "Ne")
> "`(peer, msg_id)` ile tekilleştirilen **sınırlı** bekleyen-receipt kuyruğu; ilk başarılı transport kabulüne dek kapasite açıldığında yeniden sunulur; `Ok` sonrası tekrar transport'un. İkinci ağ ARQ'su **değildir** (zamanlayıcı yok)."

**Okuma [DUR-05d-4, öneri]:** kayıt yalnız eşin o anki oturumunda bekler. Oturum değişince ya da kapanınca düşer ve görev S2 tekrar TEXT'e geçer. Alternatif: kaydı yeni tutamaca taşımak.

## 2. Mekanizma (bugün) [kod]
`send_receipt` (:1110-1136) tek `send` yapar; `Err` ya da oturum yokken yalnız `receipts_unsent` artar. `PeerState`'te (:858-878) bekleyen alan yok; tekrar TEXT yalnız oturum değişiminde gelir (:1093-1095, `adopt` :707-746). `Backpressure` (`transport.rs:1212`; tetik `files.pump`, `node.rs:464`) alan RECEIPT, `Delivered`'ı oturum ömrünce (≤ 7 gün, `session_control.rs:87`) geciktirir; yanlış "teslim edildi" yok.

## 3. Tasarım [tasarım]
1. **Durum:** `PeerState.pending_receipts: VecDeque<MsgId>`; FIFO, `(eş, msg_id)` tekil, ≤ `MAX_OUTBOX_UNACKED` = 256 (:162); bellek ≤ 64 KiB (B.4); kalıcı değil.
2. **Üretim:** oturum yok → bırak. `msg_id` kuyruktaysa birleşir. Kuyruk boşsa hemen sunulur (bugünkü tek çağrı), `Backpressure` → kuyruğa. Kuyruk boş değilse yalnız sona eklenir (sınır denetimi önce, SR-AUDIT-001).
3. **Sunum [DUR-05d-3]:** dolu kuyruğu yalnız `Messenger::poll` başı, `flush_resends`'ten (:653) sonra, baştan sunar: `Ok` → çıkar; `Backpressure` → dur; diğer `Err` (oturum sonu ya da sözleşme dışı, B.4) → bırak, devam. Alternatif: `is_transient` sınıfı (`file_transfer.rs:1629-1637`). **Sonuç:** reddedilen kayıt kuyrukta kalır ve dolu kuyruğa yalnız `poll` başı sunar → bir `poll` çağrısında eş başına ≤ 1 `Backpressure`; o eşin kuyruğu aynı `poll`'da yeniden denenmez.
4. **Zamanlayıcı yok; `turn()` değişmez:** boşalan turda `messenger.poll` `None` döndürmüştür (`ptt.rs:679-680`), sunum `files.pump`'tan önce koşar (`node.rs:458` → :464). `Ok` sonrası Transport'undur.
5. **Oturum değişimi / kapanış [DUR-05d-4], öneri DÜŞÜR:** `adopt` (:739 öncesi) ve `retire` (:756) kuyruğu boşaltır; gönderen RECEIPT'siz her TEXT'i yeniden yollar (:744-745), RECEIPT yeniden üretilir (:800-810). TTL gerekmez (`file_transfer.rs:1243` kopyalanmaz). Alternatif: `adopt` sonunda yeni tutamaca sun (fazlası `receipts_ignored`, :835-836). Gerekçe ve erişilebilirlik: B.4.
6. **Taşma [DUR-05d-1], öneri en eski düşer** (+ `receipts_evicted`, `receipts_unsent`; emsal `file_transfer.rs:1220-1224`). [çıkarım] S7'ye uyan gönderende de oturum değişiminden sonra olabilir ve düşen kayıt canlı olabilir (B.4). Alternatif: yeniyi reddet; sınır 2 × 256 (C9 "256"dan sapma).
7. **Sayaçlar [DUR-05d-2]:** `receipts_unsent` yeni anlam (kabulsüz bırakılan); yeni `receipts_deferred`, `receipts_evicted`, `receipts_pending` (anlık) + `pending_receipts(peer)`; tanımlar Ek A. Değişmez: `deferred` = kuyruktan kabul + kuyruktan düşen + `pending`. RECEIPT ertelemesi `resend_refused`'a yazılmaz → INT-02 `app_retransmit`'e (`int02_node.rs:2497-2500`) girmez (tele çıkmamış ilk sunum).
8. **Beyanlı güncelleme:** belge cümleleri Ek A (eski → yeni); eski davranışı sabitleyen test yok (B.1); mevcut test kırmızısı → DUR.

## 4. Dosyalar
Değişen: `messaging.rs`; `crates/kama-app/tests/k4g_p0_receipt.rs` (`4b7b27f1` cherry-pick; P0 gövde ve iddiaları aynen; `Link` uzantısı B.2). Dokunulmaz: AMP/tel, S1, `kama-node`, `kama-mobile`, `file_transfer.rs`, `client-core`, `specs/`. `kama-node` `event.rs:49` / `kama-mobile` `event.rs:50` "RECEIPT gitti" cümlesi bayat kalır → K4G-16 kalemi. Spec yaması yok (B.3).

## 5. Kanıt
Ölçüm `Link` gönderim kaydından; tablo ve düzenek B.2. **Tabanda kırmızı (derlenir):** P0-1, P0-2 (:662-710, :732-802; iddialar aynen), T1' taşma, T5 ret sınırı. **Tabanda yeşil bekçi:** T2 `Ok` sonrası, T3 oturum değişimi, T4 sınıf. **Yalnız 05d** (yeni API, tabanda derlenmez): T1 `evicted`, T3b, T4 `pending`, T6 sayaç değişmezi. Tabanda kırmızı beklenen yeşilse DUR.

**INT-02** (INT-02 dalının işi): son sondada iki düğümde `receipts_pending` = 0 (`probe.rs:62`) ve zaman çizelgesinin her örneğinde (`timeline_entry`, `int02_node.rs:1395`); `deferred` / `evicted` raporlanır. **Boş geçiş:** INT-02'de R3'ü tetikleyen kol yok (TEXT ve dosya Alice→Bob); `deferred` = 0 iken `pending` = 0 yalnız bekçidir, R3 kanıtı P0-1/P0-2/T1'/T5'tir. `receipts_unsent` 05d öncesi/sonrası kıyaslanmaz.

## 6. Bağımlılık · süre · risk · DUR
Bağımlılık yok; ≈ 0,5 iş günü + düzenek. Risk: `Backpressure` sürerken `poll` çağrısı başına eş başına ≤ 1 ret; tur başına çağrı sayısı (`node.rs:93`, `ptt.rs:678-685`) türetilmedi, maliyet ölçülmedi. RECEIPT dosya parçalarının önüne geçer (kasıtlı); kabul sonrası kayıp 05b/05c'nin.

**DUR:** 05d-1 taşma (öneri en eski; uyumlu eşte de tetiklenebilir) · 05d-2 üç pub sayaç + erişimci, ertelemenin `app_retransmit` dışında kalması — sahip onayı · 05d-3 yalnız `Backpressure` geçici · 05d-4 düşür ↔ taşı. Mevcut test kırmızısı / tur sırası değişikliği → DUR (gerekmiyor).

---

## Ek A — beyanlı belge güncellemeleri (`messaging.rs`; ~~eski, birebir~~ → yeni)
- **:11-12** ~~HER TEXT'e (tekrar dahil) `RECEIPT(delivered)` döner.~~ → "HER TEXT'e (tekrar dahil) `RECEIPT(delivered)` borcu doğar; aynı `msg_id` için bekleyen RECEIPT varsa tekrar onunla birleşir ("RECEIPT")."
- **:78-80** ~~RECEIPT eşin güncel oturumundan gider. Gönderilemezse (oturum yok ya da Transport reddi) kaybolur ve yeniden denenmez: gönderen kaydı bir sonraki oturum değişiminde yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir.~~ → "RECEIPT eşin güncel oturumundan gider. Eşin bekleyen kuyruğu boşsa hemen sunulur. `Backpressure` alırsa ya da önünde bekleyen varsa eş başına sınırlı kuyrukta bekler (`(eş, msg_id)` tekil, ≤ [`MAX_OUTBOX_UNACKED`]). Kuyruk her [`Messenger::poll`] başında baştan, ilk `Backpressure`'a dek AYNI oturuma sunulur (K8 "bekler": reddedilen birim tele çıkmamıştır; zamanlayıcı yok). Kabulden sonrası Transport'undur. Oturum yoksa, ret başka bir hataysa ya da eşin oturumu değişir veya kapanırsa RECEIPT bırakılır: gönderen kaydı bir sonraki oturumda yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir. Kuyruk taşarsa en eski kayıt düşer."
- **:91** (son cümleden sonra ek; silme yok) → "Reddedilen RECEIPT de aynı kuralla bekler ("RECEIPT")."
- **:186-187** ~~`RECEIPT(delivered)` bu olay üretilirken gönderildi.~~ → "`RECEIPT(delivered)` bu olay üretilirken Transport'a sunuldu, (geri basınçta) bekleyen kuyruğa alındı ya da (oturum yok / kalıcı ret) bırakıldı (`receipts_unsent`)."
- **:335, :337-338** ~~teslim yok, RECEIPT yeniden~~ (iki yerde) → "teslim yok; RECEIPT yeniden ya da bekleyenle birleşir".
- **:340** ~~Transport'un kabul ettiği RECEIPT'ler.~~ → "Transport'un kabul ettiği RECEIPT sunumları (ilk sunum ya da bekleyen kuyruktan)."
- **:342** ~~Gönderilemeyen RECEIPT'ler (oturum yok ya da Transport reddi; yeniden denenmez).~~ → "Transport'a kabul ettirilmeden bırakılan RECEIPT'ler: oturum yokken üretilen, `Backpressure` dışı bir hatayla reddedilen, eşin oturumu değişince ya da kapanınca kuyruktan düşen, taşmada düşen. Telafisi gönderenin oturum değişimindeki yeniden gönderimidir (S2)."
- **:343 sonrası (yeni alanlar)** → `receipts_deferred`: "Bekleyen kuyrukta kalan yeni RECEIPT (ilk sunumu `Backpressure` aldı ya da önünde bekleyen vardı; birleşen tekrar sayılmaz)." · `receipts_evicted`: "Kuyruk taşmasında düşen RECEIPT (`receipts_unsent`'in alt kümesi). Uyumsuz eşte ya da oturum değişimi sonrası birikimde > 0 olabilir." · `receipts_pending`: "ANLIK değer, sayaç değil: `stats()` anında bütün eşlerin bekleyen RECEIPT sayısı."
- **:646-647** ~~Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur; (2) Transport olayları, birer birer; (3) teslim edilen birimler.~~ → "Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur; (2) bekleyen RECEIPT'ler eş başına baştan, ilk `Backpressure`'a dek sunulur; (3) Transport olayları, birer birer; (4) teslim edilen birimler."
- **:1110-1111** ~~`RECEIPT(delivered)`'ı eşin güncel tutamacından verir. Tek deneme: oturum yoksa ya da Transport reddederse kaybolur (retry yok; modül belgesi, "RECEIPT").~~ → "`RECEIPT(delivered)` borcunu eşe kaydeder. Eşin kuyruğu boşsa borcu güncel tutamaca hemen sunar (tek çağrı); değilse kuyruğun sonuna ekler. Dolu kuyruğu `Messenger::poll` başı sunar. Ayrıntı: modül belgesi, "RECEIPT"."

## Ek B — ayrıntı
**B.1 Eski testler (DUR-8 taraması, taban).** Eski davranışı sabitleyen test bulunamadı:
- `receipts_unsent` yalnız :343 ve :1134'te geçiyor. `MessengerStats` hiçbir testte bütün yapı olarak karşılaştırılmıyor.
- `text_receipt.rs:813` ve `:1304` yalnız A→B yönünü dolduruyor; RECEIPT `Backpressure` almıyor.
- `lose_next` testi (:615) RECEIPT'i kabul ettirip kaybediyor (:626).
- `:964` TEXT'i `drop_next_units` ile düşürüyor; RECEIPT yolu yok.
- Ptt, signaling, file_transfer, connect_flow ve kama-node testlerinde RECEIPT `Backpressure` almıyor.

Bu nedenle 2×2 matris gerekmiyor.

**B.2 Testler, düzenek, mutasyon.**

| Test | İddia | Tabanda |
|---|---|---|
| P0-1, P0-2 (:662-710, :732-802) | iddialar aynen | kırmızı |
| T1' taşma | 257 `msg_id`; 0'ın RECEIPT'i hiç kabul edilmez, 1..256'nınki tam bir kez (05d-1'e bağlı) | kırmızı |
| T5 ret sınırı | X1, X2 beklerken aynı `poll`'da önce X1 tekrarı, sonra yeni Y → o `poll`'da ≤ 1 `Backpressure` | kırmızı (taban 2) |
| T2 `Ok` sonrası | `lose_next` ile kaybolan RECEIPT `RECOVERY_ROUNDS` (:94) boyunca yeniden sunulmaz | yeşil (bekçi) |
| T3 oturum değişimi | geçiş `poll_event`'te; `h'`'de RECEIPT(X) tam 1 kabul (boşaltmayan mutant 2) | yeşil (bekçi) |
| T4 sınıf | oturum yok / `UnknownSession` sonrası RECEIPT(X) yalnız tekrar TEXT'ten | yeşil (bekçi) |

Yalnız 05d (yeni API): T1 `evicted` = 1 · T3b `retire` sonrası `pending` 0 · T4 ret sonrası `pending` 0 · T6 `deferred` = kuyruktan kabul + kuyruktan düşen + `pending`, kapasite sonrası `pending` 0, `deferred` ≥ 1.

- `Link`'e eklenecek supersede ve close: eski tutamaç, `Ready(h')` ya da `Closed(h)` `poll_event` ile alınana dek aktif ve dolu kalır; geçiş `poll_event`'te yapılır. Bu, client-core'un davranışının aynasıdır: `drive` (`transport.rs:575`) ürün yolunda yalnız `recv` / `poll_event`'ten çağrılır (:1233, :1238); faz `Superseded`'a `drive_crypto_events` üzerinden geçer (:744-745 → :757-759, :795-797).
- T3 sırası:
  1. Bob `Ready(h')`'yi okur.
  2. Alice kuyruğunu boşaltır, `h'`'ye geçer ve TEXT'i yeniden gönderir.
  3. Bob `poll` eder.
  Boşaltmayan mutantta, X'in poll başı kopyası tekrar TEXT'ten önce kabul edilir → 2 kabul.
- T1': ham TEXT `LINK_QUEUE_CAP`'lik partilerle enjekte edilir; B→A dolu tutulur. Kurtarma turu sayısı ⌈256 / `LINK_QUEUE_CAP`⌉ + 2'den türetilir.
- Mutasyonlar ve onları öldüren testler:
  - tekilleştirme kaldırılır → P0-2
  - `Ok`'ta kayıt çıkarılmaz → P0-2, T2
  - `poll` başı sunum kaldırılır → P0-1
  - `adopt` boşaltmaz → T3
  - `retire` boşaltmaz → T3b
  - sınır kaldırılır ya da en yeni düşürülür → T1'
  - geçici hata sınıfı genişletilir → T4
  - kuyruk doluyken de hemen sunulur ya da `Backpressure`'da sunum sürer → T5
  - `pending` sabit 0 → T6
- Mevcut testler değişmeden yeşil kalmalı.

**B.3 Spec yaması yok.**
- AMP §3 (:68, :75) tekrar TEXT'e RECEIPT gönderilmesini yazar, reddedilen RECEIPT'in ne olacağını yazmaz.
- S1 K8 "bekler"e izin verir (`s1.rs:8-9`, `KM-SEAM-AT-001.md:121`).
- Tasarım `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:242`'deki "aynı oturumda zamanlayıcılı retry yok" kuralıyla tutarlı: reddedilen birim tele çıkmamıştır ve zamanlayıcı yoktur.

**B.4 Seçim ayrıntıları.**
- Bellek: `MAX_PEERS` 64 (:167) × 256 × 4 B (`MsgId` = u32, :150) = 64 KiB (+ `VecDeque` başlıkları).
- Diğer `Err`: faz `Closed` / `Superseded` → `UnknownSession` / `SessionSuperseded` (`transport.rs:1182-1186`); uç görevi bitti → `NotConnected` (:1213 ← `runtime.rs:1041`). `is_transient` alternatifi yalnız 05d-4 "taşı" ile anlamlıdır (DÜŞÜR'de bu kayıtlar zaten boşaltılır).
- DÜŞÜR: kuyruktaki her `msg_id` ya gönderenin outbox'ındadır (yeni oturumda yeniden gelir) ya da drain tekrarıysa gönderende zaten `Delivered`'dır → telafi kesin. Oturum yalnız :739 / :756'da değişir; dolu kuyruk ⇒ oturum `Some` ⇒ tutamaç alanı gerekmez. Boşaltma, değişim aynı `poll`'un olay okumasında keşfedilirse erişilir; daha önce keşfedildiyse kayıt `poll` başı sunumunda sınıf kuralıyla düşer — sonuç aynı.
- Taşma senaryosu [çıkarım; test yok]: `Ready(h')` sonrası bütün outbox `h'`ye yeniden gelir; eski oturumun geç RECEIPT'leri (:88-90) gönderende yuva açar; yeni TEXT'ler aynı `Backpressure` altında birikir → kuyruk 256'yı aşabilir. En eskiyi düşürmek önce yeniden gönderimleri (bir kısmı zararsız drain tekrarı) vurur, yeniyi reddetmek her zaman canlı kaydı; düşen canlı kaydın telafisi sonraki oturum değişimidir. 2 × 256 = canlı (gönderen outbox'ı ≤ 256) + drain tekrarı (≤ 256). Emsal ayrıca AMP §3 sinyalleşme kuralı (`specs/application-message-protocol.md:99`).
- Trafiksiz turda ret ≤ 1 / `NODE_TICK` (`node.rs:27-28`).
