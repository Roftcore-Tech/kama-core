# K4G-05d — Bekleyen RECEIPT (R3) — tasarım notu rev3

## 0. Künye

- **Kart ve dal:** K4G-05d · dal `k4g/05d-receipt` @ `6619ad6b` (taban `2a0e2133`; kod aşamasında main'e rebase).
- **rev2 onayı geri alındı:** C12 §1, C13 §1. rev2 metni Ek D'dedir — **MUST NOT IMPLEMENT**.
- **Girdiler:** orkestratör mekanizma taslağı v2 (`k4g05d/rev3/MEKANIZMA-TASLAK.md`), çürütme turu (`curutme-wf_79d4be78.json`), olgu dosyası (`k4g05d/rev3/OLGULAR.md`).
- **Atıflar:**
  - Önek yoksa main `1b1425d2`.
  - `05d:` öneki dal `6619ad6b`'dir; yalnız daldaki 05d koduna verilir.
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
   - Taşmada en eskiyi atma yolu kalkar: `05d:messaging.rs:1210-1213`, `receipts_evicted`.
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
- PoC'de heartbeat yok; testler ACK'i ve S1 olaylarını ölçer.

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

- **(a)** İki yön: iki uçta dosya + TEXT yeniden sunumu + PTT, 256 TEXT karşılıklı. Geçiş sonrası oturumda bütün RECEIPT'ler kabul; kayıp 0, kilit yok; ACK ve S1 olayları çalıştı.
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
- Kapı retleri motorların `offers_refused`, `resend_refused` (sinyalleşme), `opens_resend_refused` sayaçlarına ve INT-02 `push_retries`'e de girer.
- kama-node ve kama-mobile'daki "Transport reddetti" ve "`RECEIPT(delivered)` gitti" cümleleri bayatlar (`kama-mobile/src/error.rs:340`, :402; `kama-node/src/event.rs:49`) → K4G-16.

## 8. DUR

(Tablo sonraki commit'te.)

## 9. Süre (benim görüşüm)

- M1 (bit haritası, taşıma, tut-ve-dur, `adopt` sırası, budama, kapanış borcu): ≈ 1 gün.
- M2 (motorların Messenger kapısına bağlanması, muafiyetler, işaret, sayaç): ≈ 0,5–1 gün.
- Testler (a)–(f) ve değişen dal testleri, `Link`'e `recv` anında geçiş dahil: ≈ 1–1,5 gün.
- Rebase: ≈ 0,25 gün.
- **Toplam ≈ 3–3,5 iş günü.** A1 seçilirse +1–1,5 gün (erişimci, trait, newtype, W_F ölçümü).
