# K4G-05f — Dosya zaman aşımı sayacı (kama-app) — tasarım notu

**K4G-05f** (CEVAP 14 §6, §7 (1), §8) · rev1 · taban main `1b1425d2` (bütün `dosya:satır` atıfları bu SHA'da) · 05c/05d atıfları kendi notlarına · **TASLAK:** kod 05d A1'den sonra gelir; hiçbir şey ölçülmedi.

## 1. Kusur

- Gönderen, `FILE_END` Transport kuyruğuna kabul edilince `AwaitingReceipt { since: now }` durumuna geçer (`crates/kama-app/src/file_transfer.rs:1538-1542`).
- `FILE_RECEIPT_TTL` (60 s, `:267`) o andan sayar (`:1471-1476`).
- `send` → `Ok` "kuyruğa alındı" demektir (`crates/kama-contracts/src/s1.rs:185`). Transport ≈ 1024 + 4096 birimi hemen kabul eder (`crates/client-core/src/runtime.rs:260`, `:1616`, `:2430`).
- 31 B/s'de tam bir birim (26 + 11 + 1013 = 1050 B, Ek) ≈ 33,9 s sürer. 150 KiB = 158 birim, tel baytıyla ≈ 88 dk. Sonuç: geçerli dosya `ReceiptTimeout` alır.
- İkinci açık: Offering fazında süre sınırı yok. Geçici retlerde kayıt bekler (`file_transfer.rs:1547-1548`); oturum yokken de bekler (`:1505-1508`). İlerlemeyen aktarım süresiz açık kalır.

## 2. İlerleme bilgisi uygulamaya nasıl ulaşır

- **Kaynak (client-core, oturum başına iki sayı):**
  - `accepted`: oturuma kabul edilmiş birim sayısı, yani sıradaki birimin kimliği. Bugün `Sess.sent` olarak var (`crates/client-core/src/transport.rs:334-336`, `:1308-1310`). S1'de fragmenter kapalı olduğu için birim = AMP kaydı (`transport.rs:498-500`).
  - `confirmed_floor`: en küçük teyitsiz birim kimliği; teyitsiz birim yoksa `accepted`. Runtime görevi bunu her teyit drenajından sonra atomik olarak yayımlar; desen `outstanding` ile aynı (`runtime.rs:1533`, `:2131-2143`, `:1714-1716`).
  - Bu bir **durumdur**, olay akışı değil. Dolu kanalda düşebilen `acked()` olaylarından (`runtime.rs:2136`, `:2145` `try_send`) bu yüzden etkilenmez.
- **Önkoşul 05c:** main'de store bir `HashMap` (`runtime.rs:1906`) ve silme motordan bağımsız. 05c ile store `BTreeMap` olur ve silme yalnız `take_confirmed_units` ile yapılır (05c notu §3, D.5). Taban oradan O(1) okunur.
- **Yüzey:**
  - 05d A1'in client-core erişimcisi aynı çağrıda `accepted` ve `confirmed_floor`'u da döndürür.
  - 05d A1'in kama-app trait'ine bir metot eklenir; kama-node newtype'ı bunu geçirir (05d notu, DUR-05d-7).
  - `s1.rs` ve tel değişmez.
- **A1 ile uyumsuzluk:**
  - 05d notu A1'i oturum başına "şu an kabul edilebilir birim", yani kapasite olarak tanımlar ve adını sabitlemez. 05f ise bir ilerleme **konumu** ister.
  - Aday "bekleyen birim = 0" anı tek başına yetmez. Sayı oturum genelidir. Aynı eşe sıradaki aktarım, öncekinin `FILE_END`'i kabul edilir edilmez akmaya başlar (`file_transfer.rs:61-62`, `:876-879`). RECEIPT ve PTT de aynı FIFO'dadır. Bu yüzden sıfır anı yük altında hiç gelmeyebilir.
  - `confirmed_floor > idx_end` aynı anı yük altında da verir; boştayken "bekleyen = 0" ile çakışır. → **DUR-05f-1**.
- **FILE_END'in tek başına ACK'i neden yetmez:** ACK seçicidir (SACK, `crates/adaptive-transport/src/ack.rs:1-6`); `FILE_END`'in teyidi önceki parçaların teyidini içermez. Gereken, `FILE_END` dahil bütün önekin teyididir.

## 3. Sayaç: başlangıç, durma, sıfırlama

Aktarım başına iki değer tutulur:
- `end_on`: `FILE_END`'i kabul eden tutamaç. Mevcut kural gereği `began_on` ile aynıdır (`:1509`).
- `idx_end`: `FILE_END` `send` → `Ok` döndükten hemen sonra okunan `accepted − 1`. Düğüm döngüsü tek iş parçacıklıdır (`crates/kama-node/src/node.rs:1`); `send` ile okuma arasına başka üretici giremez.

**Faz G (gönderim):**
- Başlangıç: `FILE_BEGIN` kabul edildiği an, `last_progress = now`.
- Sıfırlama: `end_on` oturumunun `confirmed_floor`'u her arttığında `last_progress = now`.
- **İlerleme kesildi:** taban ≤ `idx_end` iken (ya da `FILE_END` henüz kabul edilmemişken) `now − last_progress ≥ T_stall`. Sonuç: `SendFailed(Stalled)` (**DUR-05f-2**); eşe bir şey gitmez, alıcı kendi `FILE_IDLE_TTL`'iyle biter.
- Oturum yokken ilerleme olmaz; sayaç sürer.
- Faz G'nin sonu: taban > `idx_end` görüldüğü an.

**`T_stall` = `FILE_IDLE_TTL` (60 s, `:262`). Yeni sayı yok.**
- Türetim: 05c'den sonra teyit ancak alıcı kabulünden doğar (05c notu §1). Taban `T_stall` boyunca artmadıysa alıcı o süre boyunca o oturumdan yeni birim kabul etmemiştir.
- O zaman alıcı aktarımı kendi zamanlayıcısıyla `IdleTimeout`'a götürür (`:1165-1185`): en geç `T_stall` + ACK yolu kadar sonra.
- İki uç aynı eşiği iki ayrı tarafta ölçer; gönderen alıcıdan önce vazgeçmez.

**Faz A (alındı):**
- Başlangıç: taban > `idx_end` görüldüğü an.
- Süre: `FILE_RECEIPT_TTL` = 60 s, değişmez. Artık yalnız karşı tarafın işlemesi ve alındının dönüşü sayılır.
- Sıfırlanmaz. Durma: son olay (`Delivered` / `SendFailed`).

Okuma ve denetim `pump` içinde yapılır; zamanlayıcılar zaten orada (`:866-869`). Düğüm her 20 ms'de bir tur döner (`node.rs:478`, `lib.rs:127`).

## 4. Oturum geçişi

Sayılar oturuma bağlıdır: her oturumun ayrı bir ucu var (`transport.rs:1021-1048`) ve birim kimliği her oturumda 0'dan başlar (`runtime.rs:1920`).

- **`FILE_END` kabul edilmeden geçiş:** mevcut kural, değişmez: `FILE_ABORT(superseded)` → `SendFailed(Superseded)` (`file_transfer.rs:1509-1515`).
- **`FILE_END` kabul edilmiş, taban ≤ `idx_end` iken oturum değişmiş:** eşin güncel oturumu ≠ `end_on` (supersede, kapanış, yeniden bağlanma) ya da erişimci `None` döner (uç `Closed`'da bırakılır, `transport.rs:942`). **Faz A o anda başlar.**
  - Gerekçe: eski oturum gönderemez (drain'de `retire_send`, `crates/kama-noise-wire/src/session_control.rs:524-538`) ve birimler yeni oturuma taşınmaz (`transport.rs:45`). Teyit bilgisi bir daha gelmez.
  - Mevcut "`FILE_END`'i kabul edilmiş aktarım kapatılmaz, alındı bekler" kuralı (`file_transfer.rs:135-137`) korunur; bekleme artık süreyle sınırlıdır.
- **Yenileme:** adaptör cutover'ı eski oturum boşalana ya da 3 s dolana dek tutar (`transport.rs:780-799`, `:207-209`; `runtime.rs:269`). Bu yüzden taban çoğunlukla geçişten önce `idx_end`'i aşar.
- **Faz A'da geçiş:** etkisiz. Alındı eş anahtarlıdır ve herhangi bir oturumdan gelebilir (`file_transfer.rs:1135-1149`).

## 5. Testler

- **(a)** kama-app, sahte saat + A1 trait'ini uygulayan oranlı sahte transport. 31 B/s, 150 KiB, tek dosya, kayıpsız; birim başına teyit 1050 B / 31 B/s. Beklenen:
  - `ReceiptTimeout` = 0 ve `IdleTimeout` = 0;
  - alıcıda `Received`, baytlar özdeş;
  - gönderende `Delivered`;
  - eski kodda `ReceiptTimeout` (kanca kırmızısı).
- **(b)** İlerleme kesilir: taban dondurulur → `T_stall` + bir tur içinde tek bir `SendFailed(Stalled)`; alıcıda `IdleTimeout`. Alt kol: Offering'de sürekli `Backpressure` + donuk taban → aynı sonuç. Alt kol: oturum yok → aynı sonuç.
- **(c)** Oturum geçişi:
  - (c1) `FILE_END` kabul edilmiş, taban geçilmeden supersede → Faz A başlar; alındı gelirse `Delivered`, gelmezse 60 s sonra `ReceiptTimeout`;
  - (c2) kapanış;
  - (c3) kapanış + yeniden bağlanma;
  - (c4) `FILE_END` öncesi supersede → `Superseded` (mevcut davranış).
- **(d)** client-core erişimci: sırasız SACK teyidinde taban artmaz, önek tamamlanınca artar; uç bırakılınca `None`.
- **(e)** INT-02 kolu (05e, C14 §7 (1)): gerçek 31 B/s tek dosya, ≈ 88 dk. Birim testi değil.

## 6. Bağımlılık · 7. Süre

- **Sıra (C14 §8):** 05c (store `BTreeMap` + `take_confirmed_units`) → 05d A1 (erişimci + trait + newtype) → 05f kodu. 05f A1 tipine iki alan, trait'e bir metot ekler.
- **Süre ≈ 2 iş günü (1,5–2,5):**
  - erişimci alanları + (d): 0,5;
  - faz ve sayaç mantığı: 0,5;
  - `Stalled` nedeninin kama-node ve kama-mobile aynası: 0,25 (DUR-05f-2 (a) seçilirse);
  - testler (a)–(c): 0,75.

## 8. DUR adayları

| DUR | Tür | Soru · seçenekler | Öneri |
|---|---|---|---|
| **DUR-05f-1** | sözleşme | (a) A1 erişimcisi `accepted` + `confirmed_floor` döndürür, A1 trait'ine metot eklenir · (b) yalnız "bekleyen = 0" (yük altında başlamayabilir; Faz G sınırı yine gerekir) | **(a)** |
| **DUR-05f-2** | sözleşme | İlerleme kesilince neden: (a) yeni `SendFailure::Stalled` (kama-app pub enum, kama-node `FileFailReason::Outgoing` `event.rs:168-186`, kama-mobile UniFFI aynası değişir) · (b) `ReceiptTimeout` yeniden kullanılır (yüzey aynı kalır, anlam karışır) | **(a)** |
| **DUR-05f-3** | beyanlı sınır | Faz A ters yöndeki FIFO'yu da sayar. Alıcının giden kuyruğunda `FILE_RECEIPT`'in önünde 1,77'den fazla tam birim varsa (60 / 33,9) 31 B/s'de `ReceiptTimeout` yanlış çıkar. Ters FIFO sınırı W_F'ye bağlı (DUR-05d-8); ölçüm C14 §7 (2). (a) beyan · (b) ters yönde birim geldikçe sıfırla + W_F'den türetilmiş üst sınır (yeni sayı) | **(a)** |
| **DUR-05f-4** | beyanlı sınır | Alıcının `FILE_IDLE_TTL`'i gönderenin FIFO'sunu ve ARQ beklemesini de sayar. 31 B/s'de iki parça arasına tek bir tam birim ya da tek bir yeniden gönderim girerse aralık ≥ 67,7 s > 60 s olur ve geçerli dosya `IdleTimeout` alır [çıkarım]. Test (a) bu yüzden tek dosya ve kayıpsız; INT-02 kolunda beyan edilir. 05f kapsamı dışında | beyan |
| **DUR-05f-5** | beyanlı sınır | Teyit = alıcı ucun ACK'i (direct-mode; hub rölesi yok, `transport.rs:12-14`). Hub hop-ARQ gelirse taban hop teyidi olur | beyan |

---

**Ek — aritmetik.**
- Tel boyu: oturum dış ek yükü 26 B (`OUTER_HDR_LEN` 10 + `TAG_LEN` 16; `crates/kama-noise-wire/src/outer.rs:9-11`, `envelope.rs:56`, `:88-89`) + DATA başlığı 11 B (`client-core/src/wire.rs:17`) + birim 1013 B (`runtime.rs:245`) = 1050 B. 1050 / 31 = 33,87 s.
- Parça yükü 1013 − 5 − 18 = 990 B (`kama-app/src/codec/limits.rs:101-142`). 150 KiB = 153 600 B → 156 parça (son parça 150 B) + `FILE_BEGIN` + `FILE_END` = 158 birim.
- Tel toplamı ≈ 155 × 1050 + 210 + 105 (ad 10 B) + 50 ≈ 163,1 kB → 5 262 s ≈ 88 dk. Sahibin ≈ 83 dk'sı yalnız yük baytıdır (153 600 / 31).
- DUR-05f-4: 2 × 33,87 = 67,7 s.
