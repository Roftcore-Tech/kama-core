# K4G-05d — Bekleyen RECEIPT (R3) — tasarım notu rev3

## 0. Künye

- **Kart ve dal:** K4G-05d · dal `k4g/05d-receipt` @ `6619ad6b` (taban `2a0e2133`; kod aşamasında main'e rebase).
- **rev2 onayı geri alındı:** C12 §1, C13 §1. rev2 metni Ek C'dedir — **MUST NOT IMPLEMENT**.
- **Girdiler:** orkestratör mekanizma taslağı v2 (`k4g05d/rev3/MEKANIZMA-TASLAK.md`), çürütme turu (`curutme-wf_79d4be78.json`), olgu dosyası (`k4g05d/rev3/OLGULAR.md`).
- **Atıflar:**
  - Önek yoksa main `1b1425d2`.
  - `05d::n` = dal `6619ad6b`'deki `crates/kama-app/src/messaging.rs` satırı; yalnız daldaki 05d koduna verilir.
  - Kısaltma: `messaging.rs`, `file_transfer.rs`, `ptt.rs`, `signaling.rs`, `store.rs` = `crates/kama-app/src/`; `runtime.rs` = `crates/client-core/src/`.
  - Dalın tabanı `2a0e2133` ile main arasında #135 ve #136 var: client-core `runtime.rs` / `transport.rs`, kama-node (`node.rs`, `lib.rs`, `probe.rs`, `carrier.rs`), adaptive-transport ve kama-integration değişti. Bu yüzden client-core ve kama-node atıfları yalnız main'e geçerlidir.
  - kama-app, kama-contracts, kama-store, kama-mobile ve AMP spec iki ağaçta aynıdır (`git diff 2a0e2133 1b1425d2` boş). Öneksiz kama-app atıfı tabana da geçerlidir.
  - Olgu dosyasının "yalnız iki dosya ayrışır" cümlesi bu nedenle yanlıştı.
- **Durum:** TASLAK. Kod "K4G-05d ONAYLANDI" bekler; koşulmadı.

## 1. Değişmezler (sahip, C11-EK §1)

- **(i)** Canlı RECEIPT hiçbir koşulda silinmez. Kapsamı için DUR-05d-6.
- **(ii)** RECEIPT kapasitesi bütün üreticilere (dosya parçası, TEXT yeniden sunumu, PTT demeti, sinyalleşme) karşı korunur. Güçlü okuması için DUR-05d-7.
- **(iii)** İki yön dolduğunda ilerleme sürer; karşılıklı bekleme yoktur.
- **(iv)** DATA kabulü duraklarken gerekli kontrol mesajları işlenir.

## 2. Mekanizma

### M1 — Kuyruk: düşürme yok, taşınır, pencereyle sınırlı

1. **Durum.** Eş başına `(eş, msg_id)` tekil yükümlülük kümesi. Bit haritası dedup penceresiyle hizalıdır (`messaging.rs:1161-1168`; 4096 bit = 512 B/eş, 64 eşte 32 KiB). Sunum sırası `msg_id` sırasıdır.
   - Taşmada en eskiyi atma yolu kalkar: `05d::1210-1213`, `receipts_evicted`.
2. **Taşıma.**
   - `adopt` ve `retire` kuyruğu boşaltmaz (bugünkü boşaltma: `05d::783`, `05d::803`).
   - `owe_receipt` oturum yokken de kuyruğa alır (bugün bırakıyor: `05d::1184-1189`).
   - Sunumda `Backpressure`, `SessionSuperseded`, `UnknownSession` ve `NotConnected` → **tut ve o eş için dur**. Kayıt kuyrukta kalır ve eşin güncel tutamacına sonra sunulur. Gerekçe:
     - Oturum sonu `recv` içindeki `drive`'da öğrenilir (`client-core/src/transport.rs:1314-1317`, :823-826, :936-937). Faz hemen hata verir (:1264-1268).
     - Poll başı sunum ise olay okumasından öncedir (`05d::693-706`).
     - Bugünkü "yalnız Backpressure geçici" kuralı (`05d::1243-1250`) bu yüzden kuyruğu tam geçişte siler.
   - Ulaşılamaz hatalar bırakılır ve sayılır (beyan): `UnitTooLarge`, `DeadlineExpired`, `EarlyUnitConflict` ve kodlama hatası. Gerekçe: RECEIPT gövdesi 5 B'tır (`05d::1241-1242`), `Reliable`'dır ve erken birim değildir. → **DUR-05d-5**.
3. **`adopt` sırası.** `session.replace`'ten (`messaging.rs:739`) sonra, `flush_peer`'dan (:745) önce eşin kuyruğu yeni tutamaca sunulur. Sıra: RECEIPT → TEXT yeniden sunumu → diğerleri.
4. **Bellek sınırı (yeni sayı yok).**
   - Gönderen, en eski RECEIPT'siz `msg_id` ile yenisi arası `MSG_DEDUP_WINDOW`'a ulaşınca `msg_id` ayırmaz (`messaging.rs:956-965`). `msg_id` eş başına kesin artar (`store.rs:246-256`).
   - Dolayısıyla eşten `m` geldiyse `≤ m − 4096` olan yükümlülük gönderende kapanmıştır.
   - Pencere altına düşen yükümlülük tutulmaz (`receipts_window_pruned`).
   - Pencere altı gelen TEXT'in RECEIPT'i bugünkü gibi tek atış sunulur (`messaging.rs:60-62`, :804-810, :1222-1223), kuyruğa girmez.
5. **Kapanış borcu (gönderen düzeltmesi).** Bugün `on_receipt`'te kalıcı silme hatası yalnız sayılıyor (`messaging.rs:846-848`); pencere denetimi yalnız bellekteki outbox'a bakıyor (:956). Yeniden başlatmada kalıntı en eski kayıt olarak canlanır (:1008-1029) ve alıcıda pencere altında budanır. Bu (i)'yi bozar.
   - Düzeltme: kalıcı silmesi başarısız `msg_id`'ler bellekte bir borç kümesinde tutulur ve her `poll`'da yeniden silinir.
   - Borç boş değilken pencere `min(outbox, borç)`'tan ölçülür.
   - Küme ≤ `MAX_OUTBOX_UNACKED`'tir: kalıntılar depo tavanına sayılır (`store.rs:369-370`, `kama-store/src/lib.rs:226-228`).
   - `messaging.rs:64-71` belgesi Ek A.2'de düzeltilir.
6. **Kapsam → DUR-05d-6.** (i) şu gönderen için koşulsuzdur: pencereye uyan, `msg_id`'si kesin artan, D4 kayıp ya da depo geri dönüşü yaşamamış gönderen. İhlalci eşte pencere altı sayılarak budanır; pencere içi hiçbir yükümlülük silinmez.
7. **Yeniden başlatma.** Kuyruk bellektedir ve kaybolur. Gönderenin kalıcı outbox'ı yeni oturumda yeniden sunar; RECEIPT tekrardan yeniden doğar (S2; `messaging.rs:744-745`, :801-822).

### M2 — Öncelik kapısı (yalnız kama-app; S1, kama-node API ve UniFFI değişmez)

- **Kapıdan geçen toplu üreticiler:**
  - `FILE_BEGIN` / `FILE_CHUNK` / `FILE_END` (`file_transfer.rs:1519-1521`);
  - `MEDIA_BUNDLE` (`ptt.rs:627`);
  - TEXT ilk gönderimi (`messaging.rs:633-636`) ve `flush_peer` (:1097-1099);
  - sinyalleşmenin yeni blob'u (`signaling.rs:425-428`; hata çağırana döner) ve yeniden sunumu (:582-584; kayıt bekler).
- **Kapının işi:** önce o eşin kuyruğunu sunar. Kuyruk hâlâ boş değilse geçici `Backpressure` döner ve `gated_by_receipts` artar. Bu sayaç INT-02'de ayrı sütun olur (DUR-05d-2 deseni).
- **Eş başına "reddedildi" işareti:** poll başında temizlenir. İşaret varken kapı kuyruğu yeniden sunmaz; "poll başına eş başına ≤ 1 ret" sınırı (T5) korunur.
- **Muaf kayıtlar:** `PTT_OPEN` ve `PTT_CLOSE` (`ptt.rs:549`, :656; `close` yeniden denenmez, karşıda ≤ 120 s hayalet akış doğardı), `FILE_RECEIPT` (`file_transfer.rs:1251-1260`), `FILE_ABORT`. `FILE_ABORT`, `Step::Abort` olarak dosya kayıtlarıyla aynı sunum noktasından geçer (:1433, :1513, :1519-1521); kapı adımı ayırt etmelidir. Erken metin yapısal olarak kapı dışıdır: client-core onu `Ready` okunmadan taze uca verir (`client-core/src/transport.rs:1000-1004`).

### M3 — Gelen tüketim hiçbir koşulda durmaz

"Gelen-kutusunu durdur" adayı **reddedildi**. Gerekçe:
- S1'de oturum başına `recv` ya da duraklatma yok (`kama-contracts/src/s1.rs:200`).
- Uygulamada tutmak geri basınç üretmez.
- Durdurma karşılıklı bekleme koşulları açar (OLGULAR §7.3 (a)–(c)).

### M4 — Kontrol yolu

- ACK sunucu soketinden gider, offer kanalından geçmez (`client-core/src/runtime.rs:3601-3605`).
- S1 olayları birimlerden önce okunur (`messaging.rs:655-658`).
- PoC'de heartbeat yok (`kama-node/src/event.rs:203`, `kama-node/src/lib.rs:66`; D18).
- Testler S1 olaylarını ölçer. `Link`'te ACK yolu yoktur; ACK yarısı INT-02'nin işidir (Ek B (a)).

## 3. Bekleme grafiği

İki kenar zinciri:
- A'nın B'ye RECEIPT'i ← A'nın B oturum kanalı/store'u (`runtime.rs:1527-1531`, :2430) ← B'nin ACK'i ← (05c) B'nin kabulü ← B'nin tüketimi. Tüketim M3 gereği koşulsuz.
- A'nın toplu üreticileri ← A'nın B kuyruğu ← aynı zincir.

"A'nın tüketimi ← A'nın giden kapasitesi" kenarı yok; döngü yok. 05c mesafe kapısı ağa bağlı bekleme ekler, döngü eklemez. Çürütme turunun canlılık merceği döngü bulamadı.

## 4. (ii)'nin güçlü okuması ve H

- **Ne karşılanıyor:** M2, RECEIPT doğduktan sonra üretici rekabetini kaldırır.
- **Ne karşılanmıyor:** kuyruk boşken pompa kanalı reddedilene dek doldurur (`file_transfer.rs:870-909`). Yeni RECEIPT, FIFO'da ≤ 1024 + 4096 dosya biriminin ardında bekler: 31–100 B/s'de saatler (OLGULAR §7.4).
- **Aynı kökün ikinci etkisi:** `FILE_RECEIPT_TTL` `FILE_END` kabul edilince başlar (`file_transfer.rs:1541`). Geçerli dosyada yanlış `ReceiptTimeout` riski doğar; ölçülmedi.
- **H = 512 kullanılmıyor:** "sınır − H" uygulamanın göremediği bir doluluk ister (S1 `s1.rs:168-216`; `outstanding()` yalnız client-core'da, `runtime.rs:1714`).
- **A1 seçilirse** rezerv türetilmiştir: H_R = `MAX_OUTBOX_UNACKED` = 256. Gerekçe: uyumlu göndericinin canlı RECEIPT'i ≤ outbox (`messaging.rs:934`); pencere içi ölü tekrarlar güvenceye gerek duymaz.
- → **DUR-05d-7**.

## 5. Testler (ayrıntı Ek B)

- **(a)** İki yön: iki uçta dosya + TEXT yeniden sunumu + PTT, 256 TEXT karşılıklı. Geçiş sonrası oturumda bütün RECEIPT'ler kabul; kayıp 0, kilit yok; S1 olayları her turda okundu (ACK: INT-02).
- **(b)** 300 TEXT tek yön.
  - **(b1)** Pencere içinde > 256 yükümlülük → silme yok.
  - **(b2)** Pencere ihlali → yalnız pencere altı sayılarak budanır.
- **(c)** Kapı ve muafiyetler.
- **(d)** `receipt_queue_survives_session_change_{a,b,c}`; geçiş hem `poll_event`'te hem `recv`'de.
- **(e)** Kapanış borcu.
- **(f)** `adopt` sırası.

**Değişen dal testleri:** t3, t4 (`BeforeText`, `DuringRecv`), birim t3b, birim t4 (a)/(b)/(c), birim t6. **Çıkan:** t1p, birim t1. **main'de kırılan test:** çürütme turunda bulunmadı; koşulmadı.

## 6. Ayrı bulgu (05d dışı; kart önerisi)

`activate` (`client-core/src/transport.rs:991-1018`) uç kurulamasa da oturumu `Active` yapar ve `Ready(h)` yayınlar. Sonuçlar:
- her `send` `NotConnected` alır (:1282);
- eş oturum değişimi görmez, S2 tetiklenmez;
- erken metin gönderilmeden `sent_on = h` işaretlenir (`messaging.rs:730`).

M1 ile bu oturumdaki kuyruk `Closed`'a dek bekler, kapı o eşe `NotConnected` yerine `Backpressure` döndürür. Düzeltme S1 davranışını etkiler; sahip kararı gerekir.

## 7. Beyanlı sınırlar

- FIFO gecikmesi (A3 seçilirse ölçülür).
- `FILE_RECEIPT_TTL` riski (§4).
- Yeniden başlatmada kuyruk kaybı (M1.7).
- D4 ve pencere ihlali (DUR-05d-6).
- Adalet sözü yok; PTT demeti kuyruk boşalana dek `Backpressure` alır, medya yeniden verir.
- `Messenger::transport_mut()` pub kalır (`messaging.rs:425`); yapısal baypastır.
- Kapı retleri motorların kendi geçici ret sayaçlarına da girer: dosya `offers_refused` (`file_transfer.rs:593-595`), sinyalleşme `resend_refused` (`signaling.rs:323-325`), `MEDIA_BUNDLE` için INT-02 `push_retries` (`kama-integration/examples/int02_node.rs:1203-1206`). Messenger'ın `resend_refused`'ına girmez; `kama-integration/tests/int02_loopback.rs:177` savı ve `app_retransmit` (`int02_node.rs:2497-2500`) değişmez.
- kama-node ve kama-mobile'daki "Transport reddetti" ve "`RECEIPT(delivered)` gitti" cümleleri bayatlar (`kama-mobile/src/error.rs:340`, :402; `kama-node/src/event.rs:49`) → K4G-16.

## 8. DUR

| DUR | Soru | Seçenekler | Önerim | Etkilenen test |
|---|---|---|---|---|
| **DUR-05d-5** | RECEIPT sunumunda `SessionSuperseded` / `UnknownSession` / `NotConnected` | (a) tut ve dur; `file_transfer.rs:1629-1637` sınıfı; ulaşılamaz hatalar bırakılır ve sayılır (beyan) · (b) yalnız `Backpressure` geçici kalır, geçişte silinir; telafi S2 yeniden üretimi, kanıt (d) · (c) poll'da sunum olaylardan sonra (T5/P0 sırası değişir) | **(a)** | t4 `DuringRecv`; birim t4 (a)/(b), t6; (d) |
| **DUR-05d-6** | (i)'nin pencere ihlali yapan eşte daraltılması | (a) (i) M1.6 kapsamında koşulsuz; ihlalde pencere altı sayılarak budanır, pencere içi silinmez; kural AMP'de normatif olur (Ek A.1) · (b) her koşulda (i): bellek sınırsız, SR-AUDIT-001'e aykırı · (c) pencere altına türetilmiş ek tavan (256) + ihlal sayacı | **(a)** | (b1), (b2), (e) |
| **DUR-05d-7** | (ii)'nin güçlü okuması | **A1:** client-core `ClientTransport`'a oturum başına "şu an kabul edilebilir birim" erişimcisi (`s1.rs` değil; tel değişmez; 05c sonrası mesafe penceresini de kapsar). kama-app'te küçük trait; kama-node'da newtype (orphan kuralı). Dosya ve `MEDIA_BUNDLE` yalnız boş yer ≥ H_R = 256 iken sunulur; dosya ayak izi ≤ W_F [CAL, INT-02'de ölçülür] · **A2:** `s1.rs` `send`'e sınıf işareti + client-core'da öncelikli kabul · **A3:** zayıf (ii) + beyanlı FIFO gecikmesi, INT-02'de 31–100 B/s ölçümü | **A1** (05d kodu M1–M4 ile ilerler; yalnız A1 kısmı bekler) | A1 ise (c2) rezerv testi |

**Eski kararlar:**
- **DUR-05d-1:** ~~en eski düşer + `receipts_evicted`, sınır 256~~ → düşürme yok; sınır gönderen penceresi (M1.4).
- **DUR-05d-3:** ~~yalnız `Backpressure` geçici~~ → DUR-05d-5.
- **DUR-05d-4:** ~~oturum değişiminde kuyruk düşer~~ → taşınır (M1.2–3; C11'in "kanıt yoksa taşı" kolu).
- **DUR-05d-2:** desen korunur. `receipts_evicted` çıkar; `receipts_window_pruned` ve `gated_by_receipts` girer.

## 9. Süre (benim görüşüm)

- M1 (bit haritası, taşıma, tut-ve-dur, `adopt` sırası, budama, kapanış borcu): ≈ 1 gün.
- M2 (motorların Messenger kapısına bağlanması, muafiyetler, işaret, sayaç): ≈ 0,5–1 gün.
- Testler (a)–(f) ve değişen dal testleri, `Link`'e `recv` anında geçiş dahil: ≈ 1–1,5 gün.
- Rebase: ≈ 0,25 gün.
- **Toplam ≈ 3–3,5 iş günü.** A1 seçilirse +1–1,5 gün (erişimci, trait, newtype, W_F ölçümü).

---

## Ek A — beyanlı belge ve spec güncellemeleri (rev3)

Kod aşamasında, kodla aynı PR'da uygulanır. Biçim: ~~eski, birebir~~ → "yeni".
- A.1 atıfları main `1b1425d2`'dir; AMP spec tabanda da aynıdır.
- A.2 atıfları `05d::n` (dal `6619ad6b`) numarasıdır; eski metin rev2 kodunun belgesidir.
- `file_transfer.rs`, `signaling.rs` ve `ptt.rs` 05d'de değişmedi; numaraları main ile aynıdır.
- rev2 Ek A'nın yerine geçer. rev2 Ek A'nın uygulanmış cümleleri aşağıda "eski" olarak durur.

### A.1 AMP spec (`specs/application-message-protocol.md`) — DUR-05d-6 (a) onayıyla

Gerekçe: alıcının pencere altı budaması gönderen penceresi kuralına dayanır. Bu kural bugün yalnız koddadır (`crates/kama-app/src/messaging.rs:64-71`, :956-965); AMP §3'te yoktur (:75).

- **:6 öncesi (yeni YAMA KAYDI; en yeni üstte)** → "> **YAMA KAYDI <ONAY-TARİHİ> — K4G-05d —** §3'e gönderen penceresi ve RECEIPT yükümlülüğü bloğu eklendi (DUR-05d-6, sahip kararı <ONAY-TARİHİ>). Gönderen penceresi kuralı (`MSG_DEDUP_WINDOW`; kalıcı kopyası henüz silinemeyen kayıt dahil) normatif oldu. Alıcının `RECEIPT(delivered)` yükümlülüğü eş ve `msg_id` başınadır, oturumdan bağımsızdır ve pencere altında kapanır. Uygulama `crates/kama-app/src/messaging.rs`. Tel formatı, alan listeleri ve bayt düzeni DEĞİŞMEDİ; üstü çizilen ya da silinen metin yok."
- **:75 sonrası (yeni blok; silme yok)** → "**[<ONAY-TARİHİ> — K4G-05d] Gönderen penceresi ve RECEIPT yükümlülüğü:** Gönderen, RECEIPT'i gelmemiş en eski `msg_id` (RECEIPT'i gelmiş ama kalıcı kopyası henüz silinemeyen dahil) ile atayacağı `msg_id` arası `MSG_DEDUP_WINDOW`'a ulaşınca yeni `msg_id` ayırmaz (tipli geri basınç). Alıcının `RECEIPT(delivered)` yükümlülüğü eş ve `msg_id` başınadır, oturumdan bağımsızdır: Transport'un kabul etmediği RECEIPT eşin güncel ya da sıradaki oturumuna sunulur; kabulden sonrası Transport'undur. Bu aynı-oturum retry'ı değildir (S1 K8): reddedilen birim tele çıkmamıştır. Görülen en büyük `msg_id`'nin `MSG_DEDUP_WINDOW` kadar altında kalan yükümlülük, bu kurala uyan gönderende kapanmıştır ve tutulmaz. Kurala uymayan gönderenin (D4 dahil) pencere altı kalan RECEIPT'i gelmeyebilir (beyanlı sınır)."

### A.2 `crates/kama-app/src/messaging.rs` (05d)

- **05d::62** ~~fail-closed: tekrar sayılır (teslim yok, `RECEIPT` yeniden).~~ → "fail-closed: tekrar sayılır (teslim yok; `RECEIPT` yeniden, tek atış: bekleyen kuyruğa girmez, "RECEIPT")."
- **05d::66-72** ~~Outbox eş başına en çok [`MAX_OUTBOX_UNACKED`] (S7) kayıt tutar. Ayrıca en eski RECEIPT'siz `msg_id` ile atanacak `msg_id` arası [`MSG_DEDUP_WINDOW`]'a ulaşamaz ([`MessengerError::UnackedSpanFull`]). Alıcının gördüğü en büyük `msg_id`, göndericinin atadığı en büyükten büyük olamaz; bu yüzden outbox'tan yeniden gönderilen her kayıt alıcı penceresinin İÇİNDE kalır ve dedup kararı kesindir. Pencere-altı durumu yalnız kalıcı durumunu yitirmiş bir göndericide doğar ("Beyanlı sınırlar", D4).~~ → "Outbox eş başına en çok [`MAX_OUTBOX_UNACKED`] (S7) kayıt tutar. Ayrıca en eski RECEIPT'siz `msg_id` ile atanacak `msg_id` arası [`MSG_DEDUP_WINDOW`]'a ulaşamaz ([`MessengerError::UnackedSpanFull`]; AMP §3, K4G-05d). RECEIPT'i gelmiş ama kalıcı kopyası silinemeyen kayıt (kapanış borcu) de bu ölçüye girer. Kapanış borcu bellektedir, ≤ [`MAX_OUTBOX_UNACKED`]'tir (kalıntılar depo tavanına sayılır) ve her [`Messenger::poll`]'da yeniden silinir. Alıcının gördüğü en büyük `msg_id`, göndericinin atadığı en büyükten büyük olamaz. Bu yüzden outbox'tan yeniden gönderilen her kayıt alıcı penceresinin İÇİNDE kalır, dedup kararı kesindir ve alıcı pencere altındaki RECEIPT yükümlülüğünü budayabilir ("RECEIPT"). Pencere-altı durumu yalnız kalıcı durumunu yitirmiş (D4) ya da deposu eski bir anlık görüntüye dönmüş göndericide doğar ("Beyanlı sınırlar")."
- **05d::78-86** ~~RECEIPT eşin güncel oturumundan gider. Eşin bekleyen kuyruğu boşsa hemen sunulur. `Backpressure` alırsa ya da önünde bekleyen varsa eş başına sınırlı kuyrukta bekler (`(eş, msg_id)` tekil, ≤ [`MAX_OUTBOX_UNACKED`]). Kuyruk her [`Messenger::poll`] başında baştan, ilk `Backpressure`'a dek AYNI oturuma sunulur (K8 "bekler": reddedilen birim tele çıkmamıştır; zamanlayıcı yok). Kabulden sonrası Transport'undur. Oturum yoksa, ret başka bir hataysa ya da eşin oturumu değişir veya kapanırsa RECEIPT bırakılır: gönderen kaydı bir sonraki oturumda yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir. Kuyruk taşarsa en eski kayıt düşer.~~ → "RECEIPT yükümlülüğü eş ve `msg_id` başınadır, oturumdan bağımsızdır (AMP §3, K4G-05d). Bekleyen RECEIPT'ler eş başına, dedup penceresiyle hizalı bir bit haritasında tutulur (`(eş, msg_id)` tekil, ≤ [`MSG_DEDUP_WINDOW`]). Eşin güncel tutamacına `msg_id` sırasıyla sunulur: her [`Messenger::poll`] başında, `adopt`'ta yeniden gönderimlerden önce ve toplu üreticilerin kapısında. Kabulden sonrası Transport'undur. `Backpressure`, `SessionSuperseded`, `UnknownSession` ya da `NotConnected` alan kayıt kuyrukta kalır ve o eşin sunumu o `poll`'da durur (K8 "bekler": reddedilen birim tele çıkmamıştır; zamanlayıcı yok). Oturum yoksa, değişirse ya da kapanırsa kuyruk eşin sıradaki tutamacını bekler. Canlı kayıt düşmez. Görülen en büyük `msg_id`'nin [`MSG_DEDUP_WINDOW`] kadar altında kalan kayıt pencereye uyan gönderende kapanmıştır ve budanır (`receipts_window_pruned`). Başka bir hata (RECEIPT için ulaşılamaz) kaydı bırakır (`receipts_unsent`). Toplu üreticiler (dosya kayıtları, `MEDIA_BUNDLE`, TEXT'in ilk gönderimi ve yeniden gönderimi, sinyalleşme) eşin kuyruğu boşalmadan Transport'a verilmez: geçici `Backpressure`, `gated_by_receipts`. Kapı dışı: `PTT_OPEN`, `PTT_CLOSE`, `FILE_RECEIPT`, `FILE_ABORT`, erken metin. Süreç yeniden başlarsa kuyruk kaybolur; telafi S2'dir."
- **05d::118-133 "Beyanlı sınırlar" (son maddeden sonra ek; silme yok)** → "* **Bekleyen RECEIPT kuyruğu kalıcı DEĞİLDİR (K4G-05d):** yeniden başlayan alıcı onu yitirir; telafi gönderenin yeni oturumdaki yeniden gönderimidir (S2). Pencereye uymayan gönderenin (D4, depo geri dönüşü) pencere altında kalan RECEIPT'i gelmeyebilir (`receipts_window_pruned`)."
- **05d::288** ~~Transport reddetti; kayıt kuyruğa ALINMADI (S1 §3). Yeniden deneme kararı çağıranındır.~~ → "Transport reddetti ya da eşin bekleyen RECEIPT'leri önde (K4G-05d kapısı: `Backpressure`, [`MessengerStats::gated_by_receipts`]); kayıt kuyruğa ALINMADI (S1 §3). Yeniden deneme kararı çağıranındır."
- **05d::350-352** ~~Transport'a kabul ettirilmeden bırakılan RECEIPT'ler: oturum yokken üretilen, `Backpressure` dışı bir hatayla reddedilen, eşin oturumu değişince ya da kapanınca kuyruktan düşen, taşmada düşen. Telafisi gönderenin oturum değişimindeki yeniden gönderimidir (S2).~~ → "Transport'a kabul ettirilmeden bırakılan RECEIPT'ler: kalıcı retle reddedilen (RECEIPT için ulaşılamaz: kodlama, `UnitTooLarge`, `DeadlineExpired`, `EarlyUnitConflict`; DUR-05d-5) ve pencere altı TEXT'in tek atışlık RECEIPT'i kabul edilmeyen. Geri basınç, oturum sonu ve oturum değişimi bırakmaz. Telafisi gönderenin oturum değişimindeki yeniden gönderimidir (S2)."
- **05d::354-355** ~~Bekleyen kuyrukta kalan yeni RECEIPT (ilk sunumu `Backpressure` aldı ya da önünde bekleyen vardı; birleşen tekrar sayılmaz).~~ → "Bekleyen kuyruğa giren yeni RECEIPT (ilk sunumu tut-ve-dur sınıfından bir ret aldı, önünde bekleyen vardı ya da eşin oturumu yoktu; birleşen tekrar sayılmaz)."
- **05d::357-359** ~~Kuyruk taşmasında düşen RECEIPT (`receipts_unsent`'in alt kümesi). Uyumsuz eşte ya da oturum değişimi sonrası birikimde > 0 olabilir.~~ → alan çıkar (taşma yok; DUR-05d-1 yerine M1.4).
- **05d::359 sonrası (yeni alanlar)**
  - `receipts_window_pruned`: "Görülen en büyük `msg_id`'nin [`MSG_DEDUP_WINDOW`] kadar altına düşerek budanan bekleyen RECEIPT'ler. Pencereye uyan gönderende budanan kayıt kapanmıştır (ör. oturum değişimi drain tekrarı); canlı kayıt yalnız pencereye uymayan eşte budanabilir (DUR-05d-6)."
  - `gated_by_receipts`: "Toplu üretici sunumlarından, eşin bekleyen RECEIPT'leri önde olduğu için Transport'a verilmeden geri çevrilenler (geçici `Backpressure`; K4G-05d M2). Transport reddi DEĞİLDİR; `resend_refused`'a yazılmaz."
- **05d::489-493** ~~Eşin bekleyen `RECEIPT(delivered)` sayısı: Transport'a henüz kabul ettirilemeyen ve sıradaki [`Messenger::poll`] başında eşin güncel oturumuna sunulacak borçlar (≤ [`MAX_OUTBOX_UNACKED`]; modül belgesi, "RECEIPT"). Motorun tanımadığı eş için 0; bütün eşlerin toplamı [`MessengerStats::receipts_pending`]'tir. Sayaç değişmezi (K4G-05d): `receipts_deferred` = kuyruktan kabul edilen + kuyruktan düşen + `receipts_pending`.~~ → "Eşin bekleyen `RECEIPT(delivered)` sayısı: Transport'a henüz kabul ettirilemeyen ve eşin güncel ya da sıradaki tutamacına sunulacak borçlar (≤ [`MSG_DEDUP_WINDOW`]; modül belgesi, "RECEIPT"). Motorun tanımadığı eş için 0; bütün eşlerin toplamı [`MessengerStats::receipts_pending`]'tir. Sayaç değişmezi (K4G-05d): `receipts_deferred` = kuyruktan kabul edilen + kuyruktan kalıcı retle bırakılan + pencere altında budanan + `receipts_pending`."
- **05d::686-688** ~~Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur; (2) bekleyen RECEIPT'ler eş başına baştan, ilk `Backpressure`'a dek sunulur; (3) Transport olayları, birer birer; (4) teslim edilen birimler.~~ → "Sıra: (0) kapanış borcunun kalıcı silmesi yeniden denenir; (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri öncelik kapısından geçerek yeniden sunulur (eşin bekleyen RECEIPT'leri önce); (2) kalan bekleyen RECEIPT'ler eş başına `msg_id` sırasıyla, ilk tut-ve-dur retine dek sunulur (bu `poll`'da reddedilmiş eşe yeniden sunulmaz); (3) Transport olayları, birer birer; (4) teslim edilen birimler."
- **05d::746-747** ~~Önceki güncel tutamaç emekliye ayrılır; RECEIPT'siz kayıtlar yeni tutamaçtan yeniden gönderilir (S1 K7).~~ → "Önceki güncel tutamaç emekliye ayrılır; eşin bekleyen RECEIPT'leri yeni tutamaca sunulur, sonra RECEIPT'siz kayıtlar yeni tutamaçtan yeniden gönderilir (S1 K7; K4G-05d)."
- **05d::929-933** ~~[K4G-05d] Transport'a henüz kabul ettirilemeyen `RECEIPT(delivered)` borçlarının `ref_msg_id`'leri, doğuş sırasıyla: eşte tekil, ≤ [`MAX_OUTBOX_UNACKED`] (taşmada en eski düşer), kalıcı DEĞİL. Yalnız `session` doluyken dolu olabilir: kayıt yalnız eşin o anki oturumunda bekler, oturum değişince ya da kapanınca kuyruk boşalır (modül belgesi, "RECEIPT").~~ → "[K4G-05d] Transport'a henüz kabul ettirilemeyen `RECEIPT(delivered)` borçları: dedup penceresiyle hizalı bit haritası (`msg_id % MSG_DEDUP_WINDOW`, 512 B). Eşte tekildir, ≤ [`MSG_DEDUP_WINDOW`], `msg_id` sırasıyla sunulur; düşürme yok, pencere altına düşen budanır; kalıcı DEĞİL. Oturumdan bağımsızdır: oturum yokken de dolabilir, oturum değişiminde ve kapanışta taşınır (modül belgesi, "RECEIPT")."
- **05d::933 sonrası (yeni alanlar)**
  - "[K4G-05d] Eşin RECEIPT sunumu bu `poll`'da tut-ve-dur retiyle durdu: kapı kuyruğu yeniden sunmaz; `poll` başında temizlenir."
  - "[K4G-05d] RECEIPT'i gelmiş ama kalıcı kopyası silinemeyen `msg_id`'ler (kapanış borcu; ≤ [`MAX_OUTBOX_UNACKED`]). Gönderen penceresi bunlardan da ölçülür; her `poll`'da yeniden silinir."
- **05d::1174-1176** ~~`RECEIPT(delivered)` borcunu eşe kaydeder. Eşin kuyruğu boşsa borcu güncel tutamaca hemen sunar (tek çağrı); değilse kuyruğun sonuna ekler. Dolu kuyruğu `Messenger::poll` başı sunar. Ayrıntı: modül belgesi, "RECEIPT".~~ → "`RECEIPT(delivered)` borcunu eşe kaydeder (oturum yokken de). Eşin kuyruğu boşsa ve oturumu varsa borcu güncel tutamaca hemen sunar (tek çağrı); değilse kuyruğa alır. Pencere altı TEXT'in borcu kuyruğa girmez, yalnız tek atış sunulur. Dolu kuyruğu `Messenger::poll` başı, `adopt` ve öncelik kapısı sunar. Ayrıntı: modül belgesi, "RECEIPT"."
- **05d::1219** ~~[K4G-05d] Bir RECEIPT sunumunun sonucu — yalnız `Backpressure` geçicidir (DUR-05d-3).~~ → "[K4G-05d] Bir RECEIPT sunumunun sonucu — `Backpressure`, `SessionSuperseded`, `UnknownSession` ve `NotConnected` tut-ve-dur'dur (DUR-05d-5)."
- **05d::1223** ~~`Backpressure`: birim tele çıkmadı; kayıt aynı oturumda bekler (S1 K8 "bekler").~~ → "Tut-ve-dur sınıfı: birim tele çıkmadı; kayıt kuyrukta kalır, eşin güncel ya da sıradaki tutamacına sunulur (S1 K8 "bekler")."
- **05d::1225** ~~Başka her hata (oturum sonu ya da sözleşme dışı) ve kodlama hatası: kayıt bırakılır.~~ → "Başka her hata (RECEIPT için ulaşılamaz) ve kodlama hatası: kayıt bırakılır (`receipts_unsent`)."
- **05d::1253-1256** ~~[K4G-05d] Eşin bekleyen RECEIPT'lerini güncel tutamaca baştan, her birini BİR KEZ sunar: kabul edilen çıkar, başka bir hatayla reddedilen bırakılır. İlk `Backpressure`'da durur: reddedilen baştaki kayıt ve arkası kuyrukta kalır, aynı çağrıda yeniden denenmez — bir [`Messenger::poll`] çağrısında eş başına en çok bir `Backpressure`. Zamanlayıcı yoktur.~~ → "[K4G-05d] Eşin bekleyen RECEIPT'lerini güncel tutamaca `msg_id` sırasıyla, her birini BİR KEZ sunar: kabul edilen çıkar, kalıcı retle reddedilen bırakılır. İlk tut-ve-dur retinde durur ve eşi bu `poll` için işaretler: reddedilen kayıt ve arkası kuyrukta kalır, aynı `poll`'da yeniden denenmez. Bir [`Messenger::poll`] çağrısında eş başına en çok bir ret olur. Oturum yoksa sunmaz. Zamanlayıcı yoktur."
- **05d::1277-1280** `drop_pending_receipts` belgesi işlevle birlikte çıkar (oturum değişiminde ve kapanışta boşaltma yok). Yerine pencere budaması gelir; belgesi: "[K4G-05d] Görülen en büyük `msg_id` ilerleyince pencere altına düşen bekleyen RECEIPT'leri budar ve `receipts_window_pruned`'da sayar (AMP §3 gönderen penceresi; DUR-05d-6)."
- Kod yorumları (`05d::781-783`, :802, :1185-1186, :1208-1209, :1264) kodla birlikte değişir; burada listelenmez.

### A.3 Diğer motorlar (`crates/kama-app/src/`; main = dal)

- **`file_transfer.rs:593-594`** ~~Transport'un geçici reddi (geri basınç, eski tutamaç) — gönderen kaydı ya da alındı: tele çıkmadı, kuyrukta bekler.~~ → "Transport'un geçici reddi (geri basınç, eski tutamaç) ya da Messenger'ın RECEIPT öncelik kapısı (K4G-05d; yalnız gönderen kaydı, `FILE_ABORT` hariç) — gönderen kaydı ya da alındı: tele çıkmadı, kuyrukta bekler."
- **`signaling.rs:278`** ve **`ptt.rs:341`** ~~Transport reddetti; kayıt kuyruğa ALINMADI (S1 §3).~~ (signaling'de devamı aynen kalır) → "Transport ya da Messenger'ın RECEIPT öncelik kapısı (K4G-05d, `Backpressure`) reddetti; kayıt kuyruğa ALINMADI (S1 §3)."
- **`signaling.rs:323-324`** ~~Transport'un GEÇİCİ reddettiği yeniden sunum denemeleri (kayıt kalır, sonraki denetimde yeniden sunulur).~~ → "Transport'un ya da Messenger'ın RECEIPT öncelik kapısının (K4G-05d) GEÇİCİ reddettiği yeniden sunum denemeleri (kayıt kalır, sonraki denetimde yeniden sunulur)."
- **Uygulanmaz, beyan:** `kama-mobile/src/error.rs:340`, :402 ("Transport reddetti") ve `kama-node/src/event.rs:49` ("`RECEIPT(delivered)` gitti") bayatlar. K4G-16'nın kalemidir; bu kart kama-node'a ve kama-mobile'a dokunmaz.

## Ek B — testler, düzenek, mutasyon

**B.1 Yeni testler.**

| Test | Düzenek ve iddia | Öldürdüğü mutant |
|---|---|---|
| (a) iki yön | Alice ve Bob'da aynı anda dosya, TEXT yeniden sunumu ve PTT; 256'şar TEXT karşılıklı; ortada `supersede`. Bütün RECEIPT'ler geçiş sonrası oturumda kabul; `receipts_unsent` = 0; her TEXT tam bir `Delivered`; türetilmiş tur sınırında biter (kilit yok); `Link` olayları her turda okunur. ACK yolu `Link`'te yoktur; M4'ün ACK yarısı INT-02'nin işidir. | kapı yok (RECEIPT dosya parçalarının ardında kalır); M3 ihlali (dolu yönde tüketimi durduran mutant kilitlenir) |
| (b1) | 300 TEXT tek yön, B→A dolu; pencere içinde > 256 yükümlülük. Hiçbiri düşmez, kuyruk ≤ 4096, kapasite açılınca her biri tam bir kez kabul. | 256 tavanı, en eskiyi düşürme |
| (b2) | Ham TEXT ile pencere ihlali (`m` sonra `m + 4096`). Yalnız pencere altı budanır, `receipts_window_pruned` = budanan sayısı; pencere içi hepsi kabul. | budama yok (bellek sınırsız); pencere içini de budayan |
| (c) | Kuyruk doluyken o eşe `FILE_CHUNK`, `MEDIA_BUNDLE`, TEXT ilk gönderimi, `flush_peer` ve sinyalleşme Transport'a ulaşmaz (`Link` gönderim kaydı); `gated_by_receipts` artar; `PTT_OPEN`, `PTT_CLOSE`, `FILE_RECEIPT`, `FILE_ABORT` geçer. Kapı ret sonrası aynı `poll`'da yeniden sunmaz (≤ 1 ret). | kapıyı atlayan motor; muafiyeti kaldıran; işareti yok sayan |
| (c2) | Yalnız A1 seçilirse: boş yer < 256 iken dosya ve `MEDIA_BUNDLE` sunulmaz. | rezervi yok sayan |
| (d) `receipt_queue_survives_session_change_{a,b,c}` | (a) kuyruk doluyken `Closed(h)` → yeni oturumda sunulur; (b) geçiş `recv` anında (`Link` yeni kip `supersede_on_next_recv`, client-core aynası `client-core/src/transport.rs:1314-1317` → :823-826) ve `adopt` / `retire` ters sırası; (c) iki ardışık değişim. Her TEXT tam bir `Delivered`. | `adopt`/`retire` boşaltır; DUR-05d-5 (b) (yalnız `Backpressure` geçici) |
| (e) kapanış borcu | `outbox_remove` bir kez hata verir (test deposu deseni `05d::1406-1467`); ardından 4096'dan fazla TEXT; sonra yeniden başlatma. `UnackedSpanFull` takılması yok, borçlu `msg_id` alıcıda budanmaz, kalıntı yeniden gönderilip kapanır. | borcu tutmayan (bugünkü `messaging.rs:846-848`); pencereyi yalnız outbox'tan ölçen |
| (f) `adopt` sırası | Yeni tutamaçta ilk kabul edilen RECEIPT, TEXT yeniden sunumundan önce gelir. | `adopt`'ta sunmayan; sırayı ters çeviren |

**B.2 Değişen 05d dal testleri** (main'de karşılığı yok; P0-1 ve P0-2 gövdeleri aynen kalır):
- **t3:** `h'`'de RECEIPT(X) 1 değil 2 kabul (taşınan kayıt + tekrar TEXT'in RECEIPT'i).
- **t4 `BeforeText`:** oturumsuz doğan RECEIPT(X) kuyruğa girer, `h'`'ye `adopt`'ta sunulur.
- **t4 `DuringRecv`:** `UnknownSession` sonrası kayıt tutulur; eski tutamaca bir daha sunulmaz.
- **birim t3b:** `retire` sonrası `pending` 0 değil 1.
- **birim t4:** (a) ve (b) `UnknownSession`, `SessionSuperseded`, `NotConnected` için tutar; `UnitTooLarge`, `DeadlineExpired`, `EarlyUnitConflict` için aynen kalır. (c) oturumsuz doğan kayıt kuyruğa girer.
- **birim t6:** adım 3'ün "kalıcı ret"i ulaşılamaz bir hatayla kurulur (`UnknownSession` artık tutar); adım 4 taşınır; adım 5 (taşma) yerine pencere budaması.
- **Değişmez:** P0 × 2, t2, t5, `re_announcing_the_same_handle_keeps_the_pending_receipts` (öldürdüğü mutantın tanımı bayatlar), `pending_receipts_are_per_peer_and_backpressure_is_per_session`. Sonuncusunun değişmemesi şuna bağlıdır: `poll` döngü sırası (`flush_resends` → `flush_receipts`) korunur ve kapı `flush_peer` içinde yalnız o eşin kuyruğunu sunar.
- **Çıkan:** t1p, birim t1.

**B.3 Kalan mutasyonlar:** tekilleştirme kaldırılır → P0-2 · `Ok`'ta kayıt çıkarılmaz → P0-2, t2 · `poll` başı sunum kaldırılır → P0-1 · `Backpressure`'da sunum sürer → t5.

---

## Ek C — Tarihçe: rev2 notu (`3d7882f1`) — MUST NOT IMPLEMENT

> Bu ek yalnız kayıttır; uygulanmaz. rev2 onayı C12 §1 ve C13 §1 ile geri alındı. Yerine geçenler: §2 (M1–M4), §8 (DUR-05d-5/-6/-7), Ek A ve Ek B.
> - rev2'nin atıfları `3439fccd` tabanına göredir.
> - Çürütülen ana varsayım şudur: oturum geçişi yalnız `poll_event`'te keşfedilir (rev2 B.2, B.4). client-core onu `recv` içindeki `drive`'da da keşfeder (§2 M1.2).
> - rev2'yi uygulayan dal kodu (`a30a50ba`, `0244ddec`, `5fd9b549`, `6619ad6b`) rev3 koduyla değişir.
> - Metin `6619ad6b`'deki nottan birebir alınmıştır. Başlıklar bir düzey indirildi; rev2'nin kendi üstü çizili parçaları «eski: …» olarak işaretlendi.

#### ~~K4G-05d — Bekleyen RECEIPT kuyruğu (R3) — tasarım notu (rev2)~~

#### ~~0. Künye~~
~~**K4G-05d** · dal `k4g/05d-receipt` · taban `main 3439fccd` (yalın `:n` = `crates/kama-app/src/messaging.rs`) · P0 `k4g/p0-taban` `4b7b27f1` · rev1 `aa94c1a7` + inceleme `review-2026-10-24/05d.md` · **TASLAK**: kod "K4G-05d ONAYLANDI" bekler; uygulanmadı, koşulmadı. **Kapsam dışı: PTT açıkken dosya parça sınırı (DUR-21), Sprint 3.**~~

#### ~~1. Değişmez (sahip metni, C9 §3 K4G-05d "Ne")~~
> ~~"`(peer, msg_id)` ile tekilleştirilen **sınırlı** bekleyen-receipt kuyruğu; ilk başarılı transport kabulüne dek kapasite açıldığında yeniden sunulur; `Ok` sonrası tekrar transport'un. İkinci ağ ARQ'su **değildir** (zamanlayıcı yok)."~~

~~**Okuma [DUR-05d-4, öneri]:** kayıt yalnız eşin o anki oturumunda bekler. Oturum değişince ya da kapanınca düşer ve görev S2 tekrar TEXT'e geçer. Alternatif: kaydı yeni tutamaca taşımak.~~

#### ~~2. Mekanizma (bugün) [kod]~~
~~`send_receipt` (:1110-1136) tek `send` yapar; `Err` ya da oturum yokken yalnız `receipts_unsent` artar. `PeerState`'te (:858-878) bekleyen alan yok; tekrar TEXT yalnız oturum değişiminde gelir (:1093-1095, `adopt` :707-746). `Backpressure` (`transport.rs:1212`; tetik `files.pump`, `node.rs:464`) alan RECEIPT, `Delivered`'ı oturum ömrünce (≤ 7 gün, `session_control.rs:87`) geciktirir; yanlış "teslim edildi" yok.~~

#### ~~3. Tasarım [tasarım]~~
1. ~~**Durum:** `PeerState.pending_receipts: VecDeque<MsgId>`; FIFO, `(eş, msg_id)` tekil, ≤ `MAX_OUTBOX_UNACKED` = 256 (:162); bellek ≤ 64 KiB (B.4); kalıcı değil.~~
2. ~~**Üretim:** oturum yok → bırak. `msg_id` kuyruktaysa birleşir. Kuyruk boşsa hemen sunulur (bugünkü tek çağrı), `Backpressure` → kuyruğa. Kuyruk boş değilse yalnız sona eklenir (sınır denetimi önce, SR-AUDIT-001).~~
3. ~~**Sunum [DUR-05d-3]:** dolu kuyruğu yalnız `Messenger::poll` başı, `flush_resends`'ten (:653) sonra, baştan sunar: `Ok` → çıkar; `Backpressure` → dur; diğer `Err` (oturum sonu ya da sözleşme dışı, B.4) → bırak, devam. Alternatif: `is_transient` sınıfı (`file_transfer.rs:1629-1637`). **Sonuç:** reddedilen kayıt kuyrukta kalır ve dolu kuyruğa yalnız `poll` başı sunar → bir `poll` çağrısında eş başına ≤ 1 `Backpressure`; o eşin kuyruğu aynı `poll`'da yeniden denenmez.~~
4. ~~**Zamanlayıcı yok; `turn()` değişmez:** boşalan turda `messenger.poll` `None` döndürmüştür (`ptt.rs:679-680`), sunum `files.pump`'tan önce koşar (`node.rs:458` → :464). `Ok` sonrası Transport'undur.~~
5. ~~**Oturum değişimi / kapanış [DUR-05d-4], öneri DÜŞÜR:** `adopt` (:739 öncesi) ve `retire` (:756) kuyruğu boşaltır; gönderen RECEIPT'siz her TEXT'i yeniden yollar (:744-745), RECEIPT yeniden üretilir (:800-810). TTL gerekmez (`file_transfer.rs:1243` kopyalanmaz). Alternatif: `adopt` sonunda yeni tutamaca sun (fazlası `receipts_ignored`, :835-836). Gerekçe ve erişilebilirlik: B.4.~~
6. ~~**Taşma [DUR-05d-1], öneri en eski düşer** (+ `receipts_evicted`, `receipts_unsent`; emsal `file_transfer.rs:1220-1224`). [çıkarım] S7'ye uyan gönderende de oturum değişiminden sonra olabilir ve düşen kayıt canlı olabilir (B.4). Alternatif: yeniyi reddet; sınır 2 × 256 (C9 "256"dan sapma).~~
7. ~~**Sayaçlar [DUR-05d-2]:** `receipts_unsent` yeni anlam (kabulsüz bırakılan); yeni `receipts_deferred`, `receipts_evicted`, `receipts_pending` (anlık) + `pending_receipts(peer)`; tanımlar Ek A. Değişmez: `deferred` = kuyruktan kabul + kuyruktan düşen + `pending`. RECEIPT ertelemesi `resend_refused`'a yazılmaz → INT-02 `app_retransmit`'e (`int02_node.rs:2497-2500`) girmez (tele çıkmamış ilk sunum).~~
8. ~~**Beyanlı güncelleme:** belge cümleleri Ek A (eski → yeni); eski davranışı sabitleyen test yok (B.1); mevcut test kırmızısı → DUR.~~

#### ~~4. Dosyalar~~
~~Değişen: `messaging.rs`; `crates/kama-app/tests/k4g_p0_receipt.rs` (`4b7b27f1` cherry-pick; P0 gövde ve iddiaları aynen; `Link` uzantısı B.2). Dokunulmaz: AMP/tel, S1, `kama-node`, `kama-mobile`, `file_transfer.rs`, `client-core`, `specs/`. `kama-node` `event.rs:49` / `kama-mobile` `event.rs:50` "RECEIPT gitti" cümlesi bayat kalır → K4G-16 kalemi. Spec yaması yok (B.3).~~

#### ~~5. Kanıt~~
~~Ölçüm `Link` gönderim kaydından; tablo ve düzenek B.2. **Tabanda kırmızı (derlenir):** P0-1, P0-2 (:662-710, :732-802; iddialar aynen), T1' taşma, T5 ret sınırı. **Tabanda yeşil bekçi:** T2 `Ok` sonrası, T3 oturum değişimi, T4 sınıf. **Yalnız 05d** (yeni API, tabanda derlenmez): T1 `evicted`, T3b, T4 `pending`, T6 sayaç değişmezi. Tabanda kırmızı beklenen yeşilse DUR.~~

~~**INT-02** (INT-02 dalının işi): son sondada iki düğümde `receipts_pending` = 0 (`probe.rs:62`) ve zaman çizelgesinin her örneğinde (`timeline_entry`, `int02_node.rs:1395`); `deferred` / `evicted` raporlanır. **Boş geçiş:** INT-02'de R3'ü tetikleyen kol yok (TEXT ve dosya Alice→Bob); `deferred` = 0 iken `pending` = 0 yalnız bekçidir, R3 kanıtı P0-1/P0-2/T1'/T5'tir. `receipts_unsent` 05d öncesi/sonrası kıyaslanmaz.~~

#### ~~6. Bağımlılık · süre · risk · DUR~~
~~Bağımlılık yok; ≈ 0,5 iş günü + düzenek. Risk: `Backpressure` sürerken `poll` çağrısı başına eş başına ≤ 1 ret; tur başına çağrı sayısı (`node.rs:93`, `ptt.rs:678-685`) türetilmedi, maliyet ölçülmedi. RECEIPT dosya parçalarının önüne geçer (kasıtlı); kabul sonrası kayıp 05b/05c'nin.~~

~~**DUR:** 05d-1 taşma (öneri en eski; uyumlu eşte de tetiklenebilir) · 05d-2 üç pub sayaç + erişimci, ertelemenin `app_retransmit` dışında kalması — sahip onayı · 05d-3 yalnız `Backpressure` geçici · 05d-4 düşür ↔ taşı. Mevcut test kırmızısı / tur sırası değişikliği → DUR (gerekmiyor).~~



#### ~~Ek A — beyanlı belge güncellemeleri (`messaging.rs`; «eski: eski, birebir» → yeni)~~
- ~~**:11-12** «eski: HER TEXT'e (tekrar dahil) `RECEIPT(delivered)` döner.» → "HER TEXT'e (tekrar dahil) `RECEIPT(delivered)` borcu doğar; aynı `msg_id` için bekleyen RECEIPT varsa tekrar onunla birleşir ("RECEIPT")."~~
- ~~**:78-80** «eski: RECEIPT eşin güncel oturumundan gider. Gönderilemezse (oturum yok ya da Transport reddi) kaybolur ve yeniden denenmez: gönderen kaydı bir sonraki oturum değişiminde yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir.» → "RECEIPT eşin güncel oturumundan gider. Eşin bekleyen kuyruğu boşsa hemen sunulur. `Backpressure` alırsa ya da önünde bekleyen varsa eş başına sınırlı kuyrukta bekler (`(eş, msg_id)` tekil, ≤ [`MAX_OUTBOX_UNACKED`]). Kuyruk her [`Messenger::poll`] başında baştan, ilk `Backpressure`'a dek AYNI oturuma sunulur (K8 "bekler": reddedilen birim tele çıkmamıştır; zamanlayıcı yok). Kabulden sonrası Transport'undur. Oturum yoksa, ret başka bir hataysa ya da eşin oturumu değişir veya kapanırsa RECEIPT bırakılır: gönderen kaydı bir sonraki oturumda yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir. Kuyruk taşarsa en eski kayıt düşer."~~
- ~~**:91** (son cümleden sonra ek; silme yok) → "Reddedilen RECEIPT de aynı kuralla bekler ("RECEIPT")."~~
- ~~**:186-187** «eski: `RECEIPT(delivered)` bu olay üretilirken gönderildi.» → "`RECEIPT(delivered)` bu olay üretilirken Transport'a sunuldu, (geri basınçta) bekleyen kuyruğa alındı ya da (oturum yok / kalıcı ret) bırakıldı (`receipts_unsent`)."~~
- ~~**:335, :337-338** «eski: teslim yok, RECEIPT yeniden» (iki yerde) → "teslim yok; RECEIPT yeniden ya da bekleyenle birleşir".~~
- ~~**:340** «eski: Transport'un kabul ettiği RECEIPT'ler.» → "Transport'un kabul ettiği RECEIPT sunumları (ilk sunum ya da bekleyen kuyruktan)."~~
- ~~**:342** «eski: Gönderilemeyen RECEIPT'ler (oturum yok ya da Transport reddi; yeniden denenmez).» → "Transport'a kabul ettirilmeden bırakılan RECEIPT'ler: oturum yokken üretilen, `Backpressure` dışı bir hatayla reddedilen, eşin oturumu değişince ya da kapanınca kuyruktan düşen, taşmada düşen. Telafisi gönderenin oturum değişimindeki yeniden gönderimidir (S2)."~~
- ~~**:343 sonrası (yeni alanlar)** → `receipts_deferred`: "Bekleyen kuyrukta kalan yeni RECEIPT (ilk sunumu `Backpressure` aldı ya da önünde bekleyen vardı; birleşen tekrar sayılmaz)." · `receipts_evicted`: "Kuyruk taşmasında düşen RECEIPT (`receipts_unsent`'in alt kümesi). Uyumsuz eşte ya da oturum değişimi sonrası birikimde > 0 olabilir." · `receipts_pending`: "ANLIK değer, sayaç değil: `stats()` anında bütün eşlerin bekleyen RECEIPT sayısı."~~
- ~~**:646-647** «eski: Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur; (2) Transport olayları, birer birer; (3) teslim edilen birimler.» → "Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur; (2) bekleyen RECEIPT'ler eş başına baştan, ilk `Backpressure`'a dek sunulur; (3) Transport olayları, birer birer; (4) teslim edilen birimler."~~
- ~~**:1110-1111** «eski: `RECEIPT(delivered)`'ı eşin güncel tutamacından verir. Tek deneme: oturum yoksa ya da Transport reddederse kaybolur (retry yok; modül belgesi, "RECEIPT").» → "`RECEIPT(delivered)` borcunu eşe kaydeder. Eşin kuyruğu boşsa borcu güncel tutamaca hemen sunar (tek çağrı); değilse kuyruğun sonuna ekler. Dolu kuyruğu `Messenger::poll` başı sunar. Ayrıntı: modül belgesi, "RECEIPT"."~~

#### ~~Ek B — ayrıntı~~
~~**B.1 Eski testler (DUR-8 taraması, taban).** Eski davranışı sabitleyen test bulunamadı:~~
- ~~`receipts_unsent` yalnız :343 ve :1134'te geçiyor. `MessengerStats` hiçbir testte bütün yapı olarak karşılaştırılmıyor.~~
- ~~`text_receipt.rs:813` ve `:1304` yalnız A→B yönünü dolduruyor; RECEIPT `Backpressure` almıyor.~~
- ~~`lose_next` testi (:615) RECEIPT'i kabul ettirip kaybediyor (:626).~~
- ~~`:964` TEXT'i `drop_next_units` ile düşürüyor; RECEIPT yolu yok.~~
- ~~Ptt, signaling, file_transfer, connect_flow ve kama-node testlerinde RECEIPT `Backpressure` almıyor.~~

~~Bu nedenle 2×2 matris gerekmiyor.~~

~~**B.2 Testler, düzenek, mutasyon.**~~

| ~~Test~~ | ~~İddia~~ | ~~Tabanda~~ |
|---|---|---|
| ~~P0-1, P0-2 (:662-710, :732-802)~~ | ~~iddialar aynen~~ | ~~kırmızı~~ |
| ~~T1' taşma~~ | ~~257 `msg_id`; 0'ın RECEIPT'i hiç kabul edilmez, 1..256'nınki tam bir kez (05d-1'e bağlı)~~ | ~~kırmızı~~ |
| ~~T5 ret sınırı~~ | ~~X1, X2 beklerken aynı `poll`'da önce X1 tekrarı, sonra yeni Y → o `poll`'da ≤ 1 `Backpressure`~~ | ~~kırmızı (taban 2)~~ |
| ~~T2 `Ok` sonrası~~ | ~~`lose_next` ile kaybolan RECEIPT `RECOVERY_ROUNDS` (:94) boyunca yeniden sunulmaz~~ | ~~yeşil (bekçi)~~ |
| ~~T3 oturum değişimi~~ | ~~geçiş `poll_event`'te; `h'`'de RECEIPT(X) tam 1 kabul (boşaltmayan mutant 2)~~ | ~~yeşil (bekçi)~~ |
| ~~T4 sınıf~~ | ~~oturum yok / `UnknownSession` sonrası RECEIPT(X) yalnız tekrar TEXT'ten~~ | ~~yeşil (bekçi)~~ |

~~Yalnız 05d (yeni API): T1 `evicted` = 1 · T3b `retire` sonrası `pending` 0 · T4 ret sonrası `pending` 0 · T6 `deferred` = kuyruktan kabul + kuyruktan düşen + `pending`, kapasite sonrası `pending` 0, `deferred` ≥ 1.~~

- ~~`Link`'e eklenecek supersede ve close: eski tutamaç, `Ready(h')` ya da `Closed(h)` `poll_event` ile alınana dek aktif ve dolu kalır; geçiş `poll_event`'te yapılır. Bu, client-core'un davranışının aynasıdır: `drive` (`transport.rs:575`) ürün yolunda yalnız `recv` / `poll_event`'ten çağrılır (:1233, :1238); faz `Superseded`'a `drive_crypto_events` üzerinden geçer (:744-745 → :757-759, :795-797).~~
- ~~T3 sırası:~~
  1. ~~Bob `Ready(h')`'yi okur.~~
  2. ~~Alice kuyruğunu boşaltır, `h'`'ye geçer ve TEXT'i yeniden gönderir.~~
  3. ~~Bob `poll` eder.~~
  ~~Boşaltmayan mutantta, X'in poll başı kopyası tekrar TEXT'ten önce kabul edilir → 2 kabul.~~
- ~~T1': ham TEXT `LINK_QUEUE_CAP`'lik partilerle enjekte edilir; B→A dolu tutulur. Kurtarma turu sayısı ⌈256 / `LINK_QUEUE_CAP`⌉ + 2'den türetilir.~~
- ~~Mutasyonlar ve onları öldüren testler:~~
  - ~~tekilleştirme kaldırılır → P0-2~~
  - ~~`Ok`'ta kayıt çıkarılmaz → P0-2, T2~~
  - ~~`poll` başı sunum kaldırılır → P0-1~~
  - ~~`adopt` boşaltmaz → T3~~
  - ~~`retire` boşaltmaz → T3b~~
  - ~~sınır kaldırılır ya da en yeni düşürülür → T1'~~
  - ~~geçici hata sınıfı genişletilir → T4~~
  - ~~kuyruk doluyken de hemen sunulur ya da `Backpressure`'da sunum sürer → T5~~
  - ~~`pending` sabit 0 → T6~~
- ~~Mevcut testler değişmeden yeşil kalmalı.~~

~~**B.3 Spec yaması yok.**~~
- ~~AMP §3 (:68, :75) tekrar TEXT'e RECEIPT gönderilmesini yazar, reddedilen RECEIPT'in ne olacağını yazmaz.~~
- ~~S1 K8 "bekler"e izin verir (`s1.rs:8-9`, `KM-SEAM-AT-001.md:121`).~~
- ~~Tasarım `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:242`'deki "aynı oturumda zamanlayıcılı retry yok" kuralıyla tutarlı: reddedilen birim tele çıkmamıştır ve zamanlayıcı yoktur.~~

~~**B.4 Seçim ayrıntıları.**~~
- ~~Bellek: `MAX_PEERS` 64 (:167) × 256 × 4 B (`MsgId` = u32, :150) = 64 KiB (+ `VecDeque` başlıkları).~~
- ~~Diğer `Err`: faz `Closed` / `Superseded` → `UnknownSession` / `SessionSuperseded` (`transport.rs:1182-1186`); uç görevi bitti → `NotConnected` (:1213 ← `runtime.rs:1041`). `is_transient` alternatifi yalnız 05d-4 "taşı" ile anlamlıdır (DÜŞÜR'de bu kayıtlar zaten boşaltılır).~~
- ~~DÜŞÜR: kuyruktaki her `msg_id` ya gönderenin outbox'ındadır (yeni oturumda yeniden gelir) ya da drain tekrarıysa gönderende zaten `Delivered`'dır → telafi kesin. Oturum yalnız :739 / :756'da değişir; dolu kuyruk ⇒ oturum `Some` ⇒ tutamaç alanı gerekmez. Boşaltma, değişim aynı `poll`'un olay okumasında keşfedilirse erişilir; daha önce keşfedildiyse kayıt `poll` başı sunumunda sınıf kuralıyla düşer — sonuç aynı.~~
- ~~Taşma senaryosu [çıkarım; test yok]: `Ready(h')` sonrası bütün outbox `h'`ye yeniden gelir; eski oturumun geç RECEIPT'leri (:88-90) gönderende yuva açar; yeni TEXT'ler aynı `Backpressure` altında birikir → kuyruk 256'yı aşabilir. En eskiyi düşürmek önce yeniden gönderimleri (bir kısmı zararsız drain tekrarı) vurur, yeniyi reddetmek her zaman canlı kaydı; düşen canlı kaydın telafisi sonraki oturum değişimidir. 2 × 256 = canlı (gönderen outbox'ı ≤ 256) + drain tekrarı (≤ 256). Emsal ayrıca AMP §3 sinyalleşme kuralı (`specs/application-message-protocol.md:99`).~~
- ~~Trafiksiz turda ret ≤ 1 / `NODE_TICK` (`node.rs:27-28`).~~
