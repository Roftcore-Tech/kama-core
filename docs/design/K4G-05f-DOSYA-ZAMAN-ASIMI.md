# K4G-05f — Dosya aktarımında ilerleme, bekleme ve yerel vazgeçme — tasarım notu rev3

## 0. Künye

- **Kart / dal / durum:** K4G-05f · `k4g/05f-dosya-ttl` (taban main `1b1425d2`; rev1 `e5514e7a`, rev2 `3e0ec8fa` üstüne) · **rev3 TASLAK.** Kod yok, ölçüm yok. "K4G-05f ONAYLANDI" ayrı bir mesajla gelir. Ürün kodu K4G-06 ile birlikte gelir (C16 v2 §3, §5.4).
- **Dayanak kararlar:** CEVAP 16 v2 §3 (05f), §5.4 > 15 EK-1 > 15 v3 §6, §5, §9 > 14 §6. Bağımsız inceleme: `decisions/incelemeci/INCELEMECI-8-05b-rev6-inceleme.md`, "05f rev2" bölümü.
- **Sürümler:**
  - rev1 ONAYLANMADI (C15 v3 §6).
  - rev2 için **yön kabul edildi**, §8.2 süre türetimine GO verilmedi (C16 v2 §3).
  - rev3'ün değişiklikleri:
    - §8 yeniden türetildi: iki yöndeki kaybın çarpımsal etkileşimi ve koddan okunan gerçek ACK takvimi;
    - iki ayrı sözleşme: deterministik kabul zarfı (kapı) ve rastgele kayıp hücreleri (rapor);
    - D_tük = 0 kaldırıldı; tüketici tutması K4G-06'nın yaşam döngüsü bölümüne bağlandı (§7);
    - DUR tablosu sahibin kararlarıyla güncellendi, DUR-05f-13…-16 eklendi.
  - rev2'nin §8'i Ek C.2'de, MUST NOT IMPLEMENT.
- **Uyum:** K4G-06 rev4 (`1f0e032c`, origin). Ortak yaşam döngüsü bölümünün sahibi K4G-06 şerididir; rev5 yazılıyor, origin'de henüz yok. 05f bu bölüme atıf yapar ve kendi tarafındaki seçenekleri bedelleriyle yazar (§7, §12). İkinci bir geçici oturum-geneli erişimci ya da pencere tasarımı ÜRETİLMEZ (C16 v2 §5.4).
- **Atıf biçimi:** `SHA:dosya:satır`.
  - Kısa adlar:
    - `file_transfer.rs`, `codec/mod.rs` → `crates/kama-app/src/`
    - `runtime.rs`, `transport.rs` → `crates/client-core/src/`
    - `receiver.rs`, `ack.rs` → `crates/adaptive-transport/src/`
    - `AMP` → `specs/application-message-protocol.md`
    - `06` → `1f0e032c:docs/design/K4G-06-AKIS-IZOLASYONU.md`
    - `05b` → `593029eb:docs/design/K4G-05b-RETX-FIX.md`
    - `05d` → `cf270a6c:docs/design/K4G-05d-RECEIPT.md`
  - SHA'lar (hepsi origin'de): `1b1425d2` main · `0c22a07d` 05c-kod · `593029eb` 05b rev6 son adayı · `cf270a6c` 05d · `1f0e032c` K4G-06 rev4 · `e5514e7a` 05f rev1 · `3e0ec8fa` 05f rev2.
- **Etiketler:** [olgu] koddan · [hesap] aritmetik, ölçülmedi · [türetim] koddan önce yazılmış sınır · [çıkarım] test edilmedi · [CAL] deney başlangıç değeri.
- **Ek A:** rev2'den DEĞİŞMEDİ (A.1–A.6). Sha256 `a6c546ae5d788dad4dee9a974e1796c1d40b1326aa70057bfc0f09afc46c2ea1`, 38 satır. Kapsam: `## Ek A` satırından `## Ek B`'den önceki son boş olmayan satıra dek (`~/kama-k4g-work/tools/eka-sha.sh`). Tüketici tutmasının spec metni DUR-05f-14 / DUR-06-26 kararıyla gelir; o zaman Ek A değişir.

## 1. Karar özeti

1. **İlerleme ve faz sınırı aynı kaldı** (C16 v2 §3 kabul):
   - ilerleme = akışın yeni benzersiz teyidi;
   - taban yalnız faz sınırıdır;
   - yerel vazgeçme "karşı taraf almadı" demek değildir;
   - G/R/A saatleri akış başınadır.
2. **§8.2 türetimi düzeltildi.** rev2'nin `(k_f + k_r + 1)` çarpanı yanlıştı. Yalnız varışla tetiklenen ACK'te çarpan `(k_f + 1)(k_r + 1)` olur. Ürün alıcısı ise varıştan bağımsız periyodik SACK de gönderir: en geç T_ACK = `ACK_CADENCE_FLOOR` + `ACK_TICK` ≈ 8,41 s. Bu SACK'ler alıcı yuvası emekliye ayrılana dek sürer (`idle_hold` = 105 s). Bu yüzden k_r ≤ K_slot = 11 iken iki yönün katkısı toplanır, k_r > K_slot iken çarpımsal etkileşim yuva penceresi başına geri gelir. Bu bir türetim açığının düzeltilmesidir; runtime'da yanlış timeout oluştuğunun kanıtı değildir (C16 v2 §3).
3. **İki sözleşme:**
   - (i) **Deterministik kabul zarfı E_D:** N_o, D_o, k_f ve k_r girdidir. "Zarf içinde yerel vazgeçme = 0" yalnız burada KAPIDIR.
   - (ii) **Rastgele kayıp hücreleri:** sonuç ε'ye karşı RAPORLANIR, deterministik garanti yazılmaz.
4. **Tüketici tutması:** D_tük = 0 kaldırıldı. INT-02 `ek-tuketici-durak` (30 s, DUR-05c-3) kapsam içindedir. İki seçenek var: (a) tutma zarfa girdi olarak girer (N_h × D_h, D_h ≥ 30 s); (b) K4G-06'nın "tutuluyor" sinyali A saatini durdurur. Karar DUR-05f-14 = DUR-06-26 ile tek yaşam döngüsünde verilir; saatler o karardan önce kodlanmaz.
5. **Sahibin kararları işlendi:** DUR-05f-6 (a), DUR-05f-7 (a) yön, DUR-05f-9 (a), DUR-05f-10 biçim, DUR-05f-11 (D_tük cümlesi hariç), DUR-05f-12 (a) kapı yalnız deterministik hücrede. Açık ve yeni DUR'lar §13'te.

## 2. Olgular

**Bugünkü saatler** [olgu, `1b1425d2`]:
- **Gönderen:** `FILE_END` yerel kuyruğa kabul edilince `AwaitingReceipt { since }` (`file_transfer.rs:1538-1542`); 60 s içinde alındı yoksa `ReceiptTimeout` (`:267`, `:1189-1209`).
- **Alıcı:** son kabul edilen YENİ parçadan sonra 60 s geçerse `IdleTimeout` (`:262`, `:1054`, `:1165-1185`).
- **Alındı kuyruğu:** küresel 4 kayıtlık; taşınca ve 60 s'yi aşınca düşer (`:291`, `:1221-1223`, `:1243-1244`).
- **Erken iptal kaydı:** ömrü `FILE_IDLE_TTL` (`:150-152`).
- **Normatif metin:** `AMP:89-92`.

**Belirsizlikler** [olgu]:
- `send` → `Ok` yalnız "kuyruğa alındı" demektir (`1b1425d2:crates/kama-contracts/src/s1.rs:185`).
- `acked()` bildirimi kanal dolunca düşebilir (`1b1425d2:runtime.rs:2136`, `:2145`).
- Motor birimi terk etmez (`1b1425d2:transport.rs:26-27`).

**ACK takvimi** [olgu, `1b1425d2`] — §8.2'nin girdisi:
- **Varışa bağlı ACK:**
  - Yeni DATA'da her `ACK_FREQ_N = 2` varışta bir ACK (`receiver.rs:76`, `:172`, `:434-445`).
  - Tekrar (kopya) varışında ACK zorlanır (`:222-230`).
- **Zamanlayıcıya bağlı ACK:** sunucu her `ACK_TICK = 20 ms`'de bütün yuvalar için `on_ack_timer`'ı çağırır (`runtime.rs:69`, `:3408-3419`). `on_ack_timer` yalnız en az bir varış görüldüyse çalışır (`receiver.rs:268-271`). Üç yol:
  - **kuyruk SACK'i:** bekleyen varış varken ve sessizlik ≥ `K_TAIL` × son varış aralığı ise (`:282-292`; `K_TAIL = 2`, `:100`);
  - **epizot kotası:** `QUOTA_R_DEFAULT = 4` SACK, aralarında `T_SPACING` = 65/31 s ≈ 2,10 s (`:111`, `:115`, `:298-306`, `:332-336`);
  - **kadans:** bekleyen varış varsa clamp(`K_ARR` × son aralık, `T_SPACING`, FLOOR); yoksa FLOOR (`:309-323`). FLOOR = `ACK_CADENCE_FLOOR_NS` = 65 B / (0,25 × 31 B/s) ≈ 8,387 s (`:83-94`).
- **Sonuç:** bir varıştan sonra alıcı, yeni varış beklemeden en geç **T_ACK = FLOOR + ACK_TICK ≈ 8,41 s**'de bir kümülatif SACK yayar. Mevcut testler: ilk zamanlayıcı SACK'i muaf, sonraki kadansla gelir (`receiver.rs:600-635`).
- **SACK kapsamı:** SACK her zaman en yüksek blok olan block0'ı taşır; `MAX_ACK_RANGES = 16` kırpması yalnız alt blokları keser (`ack.rs:36`, `:121-132`). En yeni varışın teyidi bu kırpmayla kaybolmaz.
- **Yuva ömrü:** alıcı yuvası son varıştan `idle_hold` = 105 s sonra emekliye ayrılır (`runtime.rs:270`, `:3430`; varış damgası `:3544`). Yeni varış yeni bir `Receiver` açar ve önceki SACK durumu kaybolur (`runtime.rs:3129-3132`). Dolayısıyla periyodik SACK bir varıştan sonra en çok 105 s sürer.

**05c sonrası** [olgu, `0c22a07d`]:
- Teyit yalnız `take_confirmed_units` çıktısıdır, birim oturumda en çok bir kez çıkar (`runtime.rs:2138-2156`).
- Store `BTreeMap` (`:2024`).
- Kabul edilmiş birimin tekrarı ACK'lenir (`:3396-3398`).

**05b rev6 sınırları** [türetim, `05b`]: A = RTO_MAX = 30 s (`:590`) · yeniden vadeye girme ≤ A, H_maks (T2) = 729,8 s (`:595`) · T_birim (`:605`) · T_max(DATA) = 101,68 s, T_max(kopya) = P_c = 135,56 s (`:612-615`) · L = 72,98 s (`:619`). Değerler 31 B/s içindir. Son aday bunları yeniden üretti (C16 v2 dayanak satırı).

**K4G-06 rev4** [tasarım, `06`]:
- Akışlar: dosya dinamik BK akışıdır; FILE_RECEIPT ve FILE_ABORT akış 1'de (`06:53-54`, `:82`).
- FLOW_CREDIT tetiklidir (`06:87`, `:145`).
- BLOCKED aralığı ≤ min(RTO_MAX, `idle_hold`/2) (`06:146`).
- FIN ve FIN sonrası tutma (`06:148`); RESET (`06:149`); H_C_tavan (`06:150-154`); oturum değişimi (`06:155`); tek sonlandırma çıktısı (`06:165`).
- DUR-06-26 (`06:574`), B29 (`06:335`).
- A.36 ve A.37: ortak erişimci ve terminal yüzey (`06:282-283`).

## 3. İlkeler

- **P1 — kanıt yoksa çıkarım yok.** Yeni teyit, yeni kabul ya da alındı gelmemesi karşı taraf hakkında bir durum türetmez.
- **P2 — terminal durumlar açık sinyalle oluşur:** alındı · `FILE_ABORT` / `FLOW_RESET` · oturum sonu kuralı (§9) · kullanıcı iptali · yerel vazgeçme (P3).
- **P3 — yerel vazgeçme bir politikadır.** Yalnız yerel bir saat, E_D'den türetilmiş sınırını aşınca tetiklenir; nedeni "yerel vazgeçme"dir; karşıya açık mesajla bildirilir. E_D içinde tetiklenmez (KAPI, Ek B.4). Rastgele hücrelerde tetiklenebilir (RAPOR, Ek B.8 (ii)).
- **P4 — akış başına.** Oturum geneli taban, oturum geneli "bekleyen = 0" ve ikinci bir geçici erişimci kullanılmaz (C16 v2 §5.4).
- **P5 — vazgeçme yeni kapasite istemez.** H_C_tavan içinde yer önceden ayrılmıştır (`06:150-154`). Aktarım başına en çok bir FILE_ABORT (ilk neden kalır, `1b1425d2:file_transfer.rs:134`); gelen akış başına en çok bir FILE_RECEIPT.

## 4. Kapasite · kuyruk gecikmesi · zaman aşımı (05d A1 · K4G-06 · 05f ortak bölüm)

**4.1 Üç büyüklük, üç sahip.**

| Büyüklük | Soru | Sahibi / mekanizma | Sağladığı | Sağlamadığı |
|---|---|---|---|---|
| **Kapasite** | Bu birim şimdi kabul edilebilir mi? | Alıcı kredisi (`06` §7); 05d A1 → K4G-06 akış bütçesi erişimcisi (DUR-06-14 (a), `06:562`; A.36 `06:282`); rezerv `A − 1 ≥ H_R + H_C_tavan` (C15 v3 §5 (ii)–(iii)) | Kabul edilebilirlik; kontrol kaydı için yer | Bekleme süresi (`05d:198`) |
| **Kuyruk gecikmesi** | Kabul edilmiş birim ne zaman hizmet alır? | İki düzeyli seçici: P_f, T_max (`06` §5, `:114`); düzey 2 = 05b rev6 | Türetilmiş üst sınır | Ağ kaybından bağımsız tamamlanma |
| **Zaman aşımı** | Ne zaman vazgeçilir? | Yalnız yerel vazgeçme politikası (§8) | E_D'de sonlu terminal durum | Karşı taraf hakkında bilgi (P1) |

**4.2 Kurallar.**
- **R1 — kapasite beklemesi saate girmez.** Gönderen kredi bloklu ve teyitsiz birimi yokken G durur; alıcı tüketici tutarken R durur. Faz A'da tutma: §7 ve DUR-05f-14.
- **R2 — kuyruk gecikmesi türetilmiş sınırıyla girer.** Ürün sabitleri r_min = 31 B/s'de ve E_D'nin hizmet payı tabanı φ_min'de türetilir. Ölçülen hızla küçültülmez (C15 v3 §5 (i); DUR-05f-13).
- **R3 — ağ terimi E_D girdilerinden gelir** (§8.1). Ortalama kayıptan deterministik bir sınır üretilmez (C16 v2 §3).
- **R4 — saatlerin sırası:** T_R ≥ T_G + D_ack + T_kredi + T_ctrl(k_f) ve T_A ≥ T_işlem + T_tutma + T_ctrl(k_r) (§8.2). Gönderen vazgeçerse mesajı E_D içinde alıcıdan önce ulaşır.
- **R5 — 120 s (S1) saat değildir.** FILE_RECEIPT için ölçüm hedefidir: C15 v3 §5 (i) tanımıyla doğum → ilk emisyon; ACK ve karşı uygulamaya teslim ayrı; kesintili ve kesintisiz kol ayrı. TEXT/PTT'ye uygulanmaz.

**4.3 Bir aktarımın beklemeleri.**

| Bekleme | Kapasite mi? | Saat | Sınır terimi (§8.2) |
|---|---|---|---|
| S1 `Backpressure` / akış bütçesi | evet | G durur | — |
| Kredi bloklu, teyitsiz birim yok | evet | G durur | T_kredi (alıcının R'sinde) |
| Hizmet sırası (DRR, katman) | hayır | G çalışır | T_max(DATA), P_f, φ_min |
| İleri kayıp (ARQ / kopya) | hayır | G çalışır | H_maks + k_f · T_tx |
| SACK kaybı | hayır | G çalışır | (min(k_r, K_slot) + 1) · T_ACK; k_r > K_slot ise m tur |
| Alıcı tüketicisi tutuyor (Faz G) | evet | R durur, G durur (kredi bloklu) | — |
| Alıcı tüketicisi tutuyor (Faz A) | evet | DUR-05f-14 | (a) N_h·D_h · (b) A durur |
| Alıcı işlemesi | hayır | A çalışır | T_işlem |
| FILE_RECEIPT ters yönde | hayır | A çalışır | T_ctrl(k_r); akış 1 sıralaması §12-6 |
| Oturumsuz aralık | hayır | A çalışır | N_o·D_o |

**4.4 05d A1 ile uyum.** Değişmedi.
- Erişimci alanları A.36'dadır (DUR-05f-6 (a)).
- Alındı düşürme A.21 ile kalkar (`06:266`); 05d §10 (iii)'ün "süreli kayıt" satırı (`05d:183`) geçersizleşir.
- H_C_tavan değişmez.

## 5. Gönderen ilerlemesi (G saati) ve faz sınırı

Değişmedi (C16 v2 §3 kabul). Erişimci `FlowTxProgress(oturum, akış)` — A.36, DUR-05f-6 (a):
- `accepted`: sıradaki `flow_seq`.
- `confirmed`: Teyit çıktılarından akışa düşen farklı birim sayısı; İptal sayılmaz (`06:165`); FEC RECOVERED sayılır.
- `floor`: en küçük teyitsiz `flow_seq`; motora henüz verilmemiş birimler dahil. Ancak hiçbiri yoksa `floor = accepted`.
- `blocked`: BLOCKED bölümü etkin (`06:146`).

Saat ve fazlar:
- `fin_seq` = FIN `send` → `Ok` sonrası `accepted − 1`; düğüm döngüsü tek iş parçacıklıdır (`1b1425d2:crates/kama-node/src/node.rs:1`).
- **Faz G:** `FILE_BEGIN` kabulünden `floor > fin_seq`'e dek.
- **Faz A:** `floor > fin_seq` ya da akışın oturumu bittiğinde başlar (§9).
- **G saati:** çalışır = motora verilmiş teyitsiz birim var ∨ (kredi var ∧ sunulmayı bekleyen kayıt var). Durur = `blocked` ∧ teyitsiz birim yok. Sıfırlanır = yalnız `confirmed` artınca. Dolum = T_G(E_D) → §8.3.
- **Tuzak:** store boşken FIN offer kanalında ya da yerel tamponda olabilir; o anda `floor` = o kaydın `flow_seq`'idir, `accepted` değildir (Ek B.1).

## 6. Alıcı dosya-idle (R saati) — `FILE_IDLE_TTL`'in yerine

Erişimci `FlowRxProgress(oturum, akış)` — A.36:
- `received`: ilk kez kabul edilen farklı `flow_seq`; sırasızlar dahil.
- `delivered`: uygulamaya verilen sıradaki `flow_seq`.
- `granted`: ilan edilmiş `max_seq` (`06:145`).
- `held`: `hold_flow` etkin.

R saati:
- **Çalışır:** `granted`'ın altında henüz kabul edilmemiş `flow_seq` varken.
- **Durur:** `held` ya da verilmiş kredinin tamamı kabul edilmiş.
- **Sıfırlanır:** `received` artınca.
- **Dolum:** T_R(E_D) (§8.2) → §8.3.

31 B/s'de iki tam birimin arasındaki 67,7 s, R saatini sıfırlayan bir ilerleme aralığıdır; vazgeçme yalnız T_R aşılırsa olur (C15 v3 §6: DUR-05f-4 gereksinimi).

## 7. Alındı bekleme (A saati) ve tüketici tutması — tek yaşam döngüsüne bağlı

- **Yer:** FILE_RECEIPT akış 1'de gider ve yuvası düşürülmez (`06:82`, A.21 `06:266`). Kontrol başı toplu verinin arkasında beklemez (`06:114`).
- **A saati:** Faz A'nın başında başlar; oturumsuz aralık N_o·D_o'dan sayılır. Dolum = T_A(E_D) → §8.3.
- **Tüketici tutması (C16 v2 §3, DUR-05f-8):** FIN dahil bütün birimler teyitliyken alıcı tüketicisi akışı tutarsa FILE_RECEIPT doğmaz. Faz G'deki tutma sorun değildir: kredi bloklu olduğu için G durur (`06:305` B1). Faz A için iki seçenek; ortak yaşam döngüsü bölümünün sahibi K4G-06'dır (DUR-06-26, `06:574`; B29, `06:335`):

| | (a) Tutma zarfta: N_h × D_h, D_h ≥ 30 s | (b) "tutuluyor" sinyali A saatini durdurur (`06:148`, `:574`) |
|---|---|---|
| Mekanizma | T_A'ya T_tutma = N_h·D_h eklenir; yeni girdi, yeni sinyal yok | Alıcı, tutulan ve FIN'i almış akış için FLOW_CREDIT'e sabit boylu girdi koyar, aralık T_BLK. Girdi tazeyken A durur |
| Tel | değişmez | **değişir:** yeni girdi türü (A.8, `06:253`) |
| Yeni sayı | N_h, D_h [CAL]; INT-02 `ek-tuketici-durak` 30 s'yi kapsamak için D_h ≥ 30 s | Tazelik penceresi T_taze ≥ (k_r + 1)·T_BLK + L. k_r'den türer; yeni sayı değildir. Girdi dönüşte k_r kez üst üste kaybolabilir |
| Bant | yok | 31 B/s'de tutma süresince ≈ 39 B / T_BLK ≈ 1,3 B/s [hesap, `06:574`] |
| D_h'yi aşan tutma | Zarf dışıdır. Gönderen vazgeçer, `FILE_ABORT(timeout)` gider; alıcı **bütün veriye sahip olduğu hâlde** aktarımı iptal eder (yanlış "başarısız"; DUR-05f-11) | Zarf dışı değildir. Tutan eş sinyal verdikçe aktarım açık kalır; gönderende payload yoktur, yalnız aktarım kaydı bekler. Eş susarsa ya da oturum biterse A sürer ve sonludur |
| "Sonsuza kadar açık kalmasın" (C14 §6) | sağlar | bilerek tutan eş için sağlamaz (beyan gerekir); kaybolan eş için sağlar |
| Test | B29 (iii): D_h altı ve üstü kollar | B29 (i)–(ii) |

**05f tarafının görüşü:**
- (b), R1 ("kapasite beklemesi saate girmez") ile tutarlıdır ve D_h aşımında bütün veriye sahip bir alıcının aktarımı kaybetmesini önler.
- Bedelleri: tel değişikliği ve "bilerek tutan eş" beyanı.
- (a) tel değiştirmez ama yeni bir sayı ve veri kaybettiren bir aşım davranışı getirir.
- Karar tek yaşam döngüsünde, DUR-05f-14 = DUR-06-26 olarak verilir. **Saat o karardan önce kodlanmaz** (C16 v2 §3).

## 8. Yerel vazgeçme politikası — iki sözleşme ve türetim

**8.1 İki sözleşme.**

**(i) Deterministik kabul zarfı E_D — KAPI** (DUR-05f-12 (a), DUR-05f-13).

Girdiler:

| Girdi | Anlam |
|---|---|
| r_min | Pacer taban hızı: 31 B/s (`1b1425d2:crates/adaptive-transport/src/sender.rs:1485`) |
| φ_min | Sunulan yük altında akışın hizmet payı tabanı (`06` §5; sayı DUR-06-3). Tek dosya hücrelerinde φ = 1 |
| N_o, D_o | Kesinti sayısı ve azami süresi, iki yön. Oturumsuz aralık da kesintidir. D_o < `idle_hold` |
| k_f | İleri yolda akışın teyitsiz birimlerinin **ardışık** kaybolan iletim sayısının tavanı |
| k_r | Dönüş yolunda **ardışık** kaybolan SACK emisyonu sayısının tavanı |
| Tutma | DUR-05f-14 (a)'da N_h, D_h |

Kayıp modeli: kesinti dışındaki kayıplar yalnız bu ardışık tavanlarla sınırlıdır. Sözleşme: **E_D içinde G, R ve A saatlerinin hiçbiri dolmaz → yerel vazgeçme = 0.** Kapı Ek B.4 ve B.8 (i)'dir. Deterministik kayıp deseni `EgressPolicy` gibi bir örüntü enjektörüyle kurulur; rastgele değildir.

**(ii) Rastgele kayıp hücreleri — RAPOR** (B22 kayıplı hücreleri, `06:328`).
- Ürün aynı sabitlerle koşar (E_D'den türetilmiş, §8.2).
- Gözlenen G / R / A vazgeçme sayısı raporlanır. Deterministik garanti YAZILMAZ.
- Karşılaştırma referansı: bağımsız Bernoulli kaybı varsayımıyla, bir ardışık kayıp koşusunun k'yı aşma olasılığı ≈ N_fırsat · p^(k+1).
- Varsayımlar açıktır:
  - kayıpların bağımsızlığı (netem rastgele kaybı);
  - N_fırsat: ileride iletim sayısı, dönüşte SACK emisyon sayısıdır. Ölçülür, tahmin edilmez.
- ε = 10⁻³ [CAL] yalnız bu raporun eşiğidir (DUR-05f-8).
- [hesap] D-3 girdileriyle (k_f = 3, k_r = 11):
  - p_f = %10'da 0,1⁴ = 10⁻⁴ / ileri koşu;
  - p_r = %80'de 0,8¹² ≈ 0,069 / dönüş koşusu.
  - Yani %80 dönüş kaybı hücresi E_D'nin dışındadır ve vazgeçme olabilir. Bu bir rapor kalemidir, kapı değildir.

**8.2 Türetim** [türetim; koddan önce; ölçülmedi].

**Neden rev2'nin çarpanı yanlıştı (C16 v2 §3):**
- Teyit yalnız varışla tetiklenen ACK'le gelseydi, her dönüş kaybı yeni bir ileri deneme koşusu açardı.
- k_f = k_r = 1 örneği: veri kaybolur, ikinci deneme ulaşır, ACK'i kaybolur; üçüncü veri kaybolur, dördüncü veri ve ACK'i ulaşır. Yani 4 ileri deneme gerekir, 3 değil.
- En kötü çarpan (k_f + 1)(k_r + 1)'dir; `k_f + k_r + 1` bundan çıkmaz.

**Gerçek ACK takvimi çarpımı kırar, ama yalnız yuva penceresi içinde** (§2 ACK takvimi):
1. **İleri yol.** Akışta teyitsiz birim varken bir teyitsiz birim en geç T_tx = A + P_c'de yeniden iletilir. ACK sessizliğinden sonraki ilk sahiplenme H_maks'tır (`05b:595`, `:605`). En çok k_f ardışık iletim kaybolur, dolayısıyla ≤ H_maks + k_f·T_tx içinde bir teyitsiz birim u varır.
2. **Dönüş yolu.** u'nun varışından sonra alıcı, yeni varış beklemeden en geç T_ACK'de bir SACK yayar (kadans FLOOR). Bu SACK block0'da u'yu taşır. En çok k_r ardışık SACK kaybolur → (k_r + 1)·T_ACK + L içinde u teyit edilir. Bu, **varışla ilgisizdir**; ileri yolda yeni deneme gerekmez.
3. **Yuva penceresi.** Periyodik SACK son varıştan en çok `idle_hold` = 105 s sürer; sonra yuva emekliye ayrılır ve SACK durumu kaybolur (§2). 31 B/s'de T_tx = 165,56 s > 105 s olduğu için yuvayı yeniden göndermeler canlı tutamaz.
   - **K_slot** = ⌊`idle_hold` / T_ACK⌋ − 1 = ⌊105 / 8,41⌋ − 1 = **11** [hesap].
   - k_r ≤ K_slot → iki yön toplanır.
   - k_r > K_slot → her tur bir ileri varış ister: m = ⌈(k_r + 1) / (K_slot + 1)⌉ tur, her tur ≤ k_f + 1 ileri iletim. Yuva penceresi olmasaydı (K_slot = 0) bu ifade incelemedeki (k_f + 1)(k_r + 1)'e eşit olurdu.

**Sınırlar:**
- **T_G(E_D)** = N_o·D_o + T_baş + (m·(k_f + 1) − 1)·T_tx + (min(k_r, K_slot) + 1)·T_ACK + 2L.
  - T_baş = T_max(DATA) eğer k_f = 0 ∧ N_o = 0 ∧ k_r ≤ K_slot; aksi hâlde H_maks.
  - m = ⌈(k_r + 1) / (K_slot + 1)⌉.
- **D_ack(E_D)** = (min(k_r, K_slot) + 1)·T_ACK + L: kabul anından teyidin gönderene ulaşmasına dek. R saati gönderenin ilerlemesinden bu kadar geride kalabilir.
- **T_kredi(E_D)** = N_o·D_o + (k_f + 1)(k_r + 1)·T_BLK + 2L.
  - K4G-06 rev4'te FLOW_CREDIT, BLOCKED'e **tetikli** yanıttır (`06:145`; `06:87`); dolayısıyla burada çarpım gerçektir.
  - Güncel sınır periyodik SACK'e bindirilirse T_kredi = N_o·D_o + (k_f + 1)·T_BLK + (min(k_r, K_slot) + 1)·T_ACK + 2L olur (öneri: §12-7).
- **T_ctrl(E_D, k)** = T_C + L + [N_o > 0 ∨ k > 0]·(N_o·D_o + H_C + k·(A + P_C)).
  - Tek kontrol biriminin karşı uygulamaya ulaşması. Bekleyen taraf yalnız varışı gözler, ACK gerekmez → çarpım yoktur.
  - k, birimin gittiği yönün tavanıdır: FILE_RECEIPT için k_r, FILE_ABORT için k_f.
  - T_C = (S_iletimde + S_baş,C)/r + S_baş,C/(ρ_C·r) (`06:114`); S_baş,C ≈ 50 B; ρ_C = 0,1 [CAL]; H_C ≤ H_maks; P_C ≈ T_C.
  - **Koşul:** FILE_RECEIPT akış 1'de kendinden önceki kontrol birimlerinin kurtarılmasını beklemez (§12-6).
- **T_A(E_D)** = T_işlem + T_tutma + T_ctrl(E_D, k_r). T_tutma: (a) N_h·D_h, (b) 0 (A tutma sırasında durur).
- **T_R(E_D)** = T_G + D_ack + T_kredi + T_ctrl(E_D, k_f).

**[hesap]** 31 B/s, φ = 1. Kullanılan değerler: T_max(DATA) = 101,68 s; T_tx = A + P_c = 165,56 s; H_maks = H_C = 729,8 s; L = 72,98 s; T_ACK = 8,41 s; T_BLK = 30 s; T_C = 51,6 s; A + P_C = 81,6 s; T_işlem = 1 s. Deterministik hücreler (DUR-05f-13 [CAL]):

| Hücre | N_o×D_o | k_f / k_r | T_G | D_ack | T_kredi | T_ctrl(k_r) | T_ctrl(k_f) | T_A (b) / (a, D_h = 30 s) | T_R |
|---|---|---|---|---|---|---|---|---|---|
| D-0 | 0 | 0 / 0 | 256,1 s | 81,4 s | 176,0 s | 124,6 s | 124,6 s | 125,6 / 155,6 s | 638,0 s |
| D-1 | 1 × 60 s | 0 / 0 | 944,2 s | 81,4 s | 236,0 s | 914,4 s | 914,4 s | 915,4 / 945,4 s | 2 175,9 s |
| D-2 | 1 × 60 s | 3 / 3 | 1 466,1 s | 106,6 s | 686,0 s | 1 159,2 s | 1 159,2 s | 1 160,2 / 1 190,2 s | 3 417,8 s |
| D-3 | 1 × 60 s | 3 / 11 (= K_slot) | 1 533,4 s | 173,9 s | 1 646,0 s | 1 812,0 s | 1 159,2 s | 1 813,0 / 1 843,0 s | 4 512,4 s |

- Gevşek üst sınırlardır.
- **Ürün sabitleri** önerilen E_D = D-3 girdileriyle, r_min'de ve φ_min'de hesaplanır (DUR-05f-13). φ_min (DUR-06-3) gelince sayılar büyür; tek dosya hücrelerinde φ = 1.
- k_r > K_slot, örneğin 47 için: m = 4 → T_G = 60 + 729,8 + 15·165,56 + 12·8,41 + 145,96 ≈ 3 520 s [hesap]. Bu rejim DUR-05f-15'tir.

**8.3 Vazgeçme eylemleri** — rev2'den değişmedi:
- **G dolumu:** `reset_flow` + akış 1'e `FILE_ABORT(0x04 timeout)` + tek `SendFailed(LocallyAbandoned)`.
- **A dolumu:** `FILE_ABORT(0x04)` + `SendFailed(LocallyAbandoned)`; geç gelen `ok` → `receipts_ignored` (`1b1425d2:file_transfer.rs:1152`).
- **R dolumu:** akış yerelde kapanır (sonraki birimler ACK'lenir ve atılır, `late_after_close`, `06:54`) + `FILE_RECEIPT(0x03)` + `Failed(LocallyAbandoned)`.
- **Karşı taraf:** alıcıda `Aborted(Timeout)` (`1b1425d2:file_transfer.rs:1114-1132`), gönderende `SendFailed(ReceiverTimeout)` (`:351-360`).
- Varyant adları DUR-05f-9 (a) kararıdır.

## 9. Oturum geçişi

Kurallar rev2'den değişmedi; ortak yaşam döngüsünün oturum satırı K4G-06'dadır (`06:155`).

| Durum | Gönderen | Alıcı |
|---|---|---|
| Faz G'de oturum değişti | `FILE_ABORT(superseded)`, `SendFailed(Superseded)` (`1b1425d2:file_transfer.rs:1509-1515`; `AMP:92`) | `Aborted(Superseded)` |
| FIN kabul edilmiş, `floor ≤ fin_seq` iken oturum bitti | Faz A o anda başlar (`1b1425d2:transport.rs:45`) | — |
| Alıcıda `Closed(h)`, akış eksik | — | Teslim artık gelmez (`1b1425d2:transport.rs:1069-1072`) → `Failed(SessionEnded)` + sıradaki oturumda `FILE_RECEIPT(0x03)` |
| Faz A'da oturum değişti | Etkisiz (`1b1425d2:file_transfer.rs:1135-1149`) | Alındı yuvadan yeni oturuma gider |
| Yeniden bağlanma | Oturumsuz aralık N_o·D_o'ya yazılır | Erken iptal kaydı, eski oturumlar `Closed` olunca zamanlayıcısız düşer |

## 10. Bağımlılık ve sıra

- 05f ürün kodu yalnız K4G-06 ürün koduyla birlikte gelir (C16 v2 §5.4): 05d A1, alıcı kredisi, kontrol yükümlülükleri, iptal, FIN ve timeout tek yaşam döngüsünde.
- İkinci bir geçici oturum-geneli erişimci ya da pencere tasarımı üretilmez.
- Sıra: #140 → 05g → 05c → K4G-06 deneyi (iki taban) → K4G-06 rev5 + CEVAP 17 → DUR-05f-14 = DUR-06-26 kararı → ürün kodu (`06` §12 (ii), (iv)) → INT-02.

## 11. Süre (iş günü; tahmin; K4G-06 ürün kodunun içinde)

- Erişimci alanları + B.6: 0,5.
- Üç saat, fazlar, eylemler, oturum kuralları, erken iptal kaydı: 1–1,5.
- Tutma seçeneği: (a) 0,25 · (b) 0,5 (tel girdisi K4G-06'da).
- Ek A + mevcut test değişiklikleri (B.7): 0,5–1.
- Yeni testler B.1–B.5, deterministik örüntü enjektörü dahil: 1,25.
- kama-node / kama-mobile yüzeyi (K4G-06 aynasıyla tek pakette): 0,25.
- **Toplam ≈ 3,75–5.** INT-02 kolu ayrı. Rev2'nin 3,25–4,5'inden fark: tutma seçeneği ve deterministik enjektör.

## 12. K4G-06 ile çelişkiler ve öneriler (rev5 yaşam döngüsü bölümüne)

1. **Kapandı** (rev4 işledi, `06:178`): ilerleme = yeni benzersiz teyit.
2. **Kapandı** (rev4, `06:268`): A.23 ve A.36/A.37 05f DUR'larına bağlandı.
3. **Kapandı** (rev4, `06:328`): B22 iki kol. rev3'e göre kalan tek fark: B22 kayıplı hücreleri **rapor** olur, kapı değildir (DUR-05f-12 (a)). B22 hüküm sütunundaki "zarf içinde yerel vazgeçme = 0 (kapı)" yalnız deterministik hücrelere uygulanmalı.
4. **Açık — DUR-06-26 = DUR-05f-14:** FIN sonrası tutma, §7. Yaşam döngüsü bölümünün sahibi K4G-06; 05f'nin seçenekleri ve bedelleri §7'de.
5. **Uyumlu:** H_C_tavan, `reset_flow`, `max_seen` + açık küme.
6. **Yeni — akış 1 sıralaması ve T_ctrl** (C16 v2 §4 karşı örneği). FILE_RECEIPT, RECEIPT ve FILE_ABORT ile aynı `Ordered` akışta kalırsa, öndeki kayıp bir kontrol kaydı FILE_RECEIPT'i bekletir ve T_ctrl bu kurtarmayı da içermelidir. K4G-06'nın kontrol sıralama kararı (bağımsız kayıtlarda sırasız teslim ya da ayrı sıralama alanı) T_ctrl formülünü belirler. 05f'nin §8.2'si "FILE_RECEIPT sıra beklemez" koşuluyla yazıldı.
7. **Yeni öneri — kredi yolu çarpımı:** FLOW_CREDIT yalnız BLOCKED'e tetikli yanıtsa (`06:145`) kredi yenilemesi iki yönün kaybını çarpar: (k_f + 1)(k_r + 1)·T_BLK, D-3'te 1 440 s. Güncel sınırın periyodik SACK'e bindirilmesi bunu toplanır hâle getirir. Karar K4G-06'nındır; 05f'nin T_R'si iki hâli de kapsar.
8. **Yeni — yuva emekliliği:** periyodik SACK yalnız yuva ömrü boyunca sürer (`idle_hold`). K4G-06 BLOCKED'in yuvayı canlı tuttuğunu söyler (`06:146`), ama bu yalnız bloklu akış içindir. Veri akarken yuva ömrü K_slot'u belirler (DUR-05f-15).

## 13. DUR listesi

**Sahip kararları (C16 v2 §3):**

| DUR | Karar | rev3'te |
|---|---|---|
| DUR-05f-6 | **(a)** K4G-06 akış bütçesi erişimcisi (client-core somut tip + kama-app trait, S1 dışı) | §5, §6; A.36 |
| DUR-05f-7 | **(a) yön GO; §8.2 sayıları ONAYLI DEĞİLDİ** | §8.2 yeniden türetildi → **onay bekliyor** |
| DUR-05f-8 | ε = 10⁻³ ve T_işlem ≤ 1 s yalnız [CAL]. **D_tük = 0 reddedildi** | §7 → DUR-05f-14 |
| DUR-05f-9 | **(a)** `LocallyAbandoned` / `SessionEnded`; UniFFI aynası K4G-06 ile tek seferde | §8.3, §9 |
| DUR-05f-10 | Ek A biçimi kabul; onay rev3'ten sonra | Ek A değişmedi |
| DUR-05f-11 | Beyan kabul, D_tük cümlesi hariç. Yeni metin: "zarf dışında yanlış 'başarısız' mümkün, yanlış 'teslim edildi' asla; vazgeçmeden sonra gelen `ok` yok sayılır" | §8.3 |
| DUR-05f-12 | **(a)** iki kol; kapı yalnız deterministik hücrelerde | §8.1, Ek B.8 |
| DUR-05f-5 | beyan | — |

**Açık ve yeni:**

| DUR | Tür | Soru · seçenekler | Öneri |
|---|---|---|---|
| **DUR-05f-7 (sayılar)** | onay | §8.2 türetimi: K_slot'lu toplanır/çarpımsal formül ve D-0…D-3 tablosu | onay |
| **DUR-05f-13** | yeni sayı / sözleşme | E_D hücreleri ve ürün sabitleri: (a) D-0…D-3 kapı hücreleri; k_f = 3, k_r ∈ {3, 11} [CAL]; ürün sabitleri D-3 girdileriyle, r_min = 31 B/s ve φ_min'de; ölçülen hızla küçültülmez · (b) ürün sabitleri ölçülen hıza göre uyarlanır (C15 v3 §5 (i) ile çelişir) | (a) |
| **DUR-05f-14** | sözleşme (= DUR-06-26) | Faz A tüketici tutması: (a) N_h × D_h zarf girdisi, D_h ≥ 30 s · (b) "tutuluyor" sinyali A'yı durdurur (tel girdisi, T_taze ≥ (k_r + 1)·T_BLK + L) | 05f görüşü (b); karar tek yaşam döngüsünde (K4G-06) |
| **DUR-05f-15** | beyanlı sınır | k_r > K_slot = 11 iken toplanır sınır geçmez (yuva `idle_hold`'da emekliye ayrılır, SACK durumu sıfırlanır) → çarpımsal rejim (m tur): (a) beyan; E_D'nin k_r'si ≤ K_slot · (b) teyitsiz veri varken yuvayı canlı tutan bir kural (K4G-06 / runtime) | (a); (b) K4G-06'ya not |
| **DUR-05f-16** | test amacı | Rastgele hücre raporunun biçimi: model (bağımsız Bernoulli), ölçülen N_fırsat, öngörülen ve gözlenen vazgeçme sayısı yan yana; hüküm yok | kabul |

---

## Ek A — Onay gereken metin değişiklikleri (DUR-05f-10; biçim: üstü çiz, silme + tarihli YAMA KAYDI + `word_coverage`)

`<T>` = "K4G-05f ONAYLANDI" tarihi. Hedef: `specs/application-message-protocol.md` (`1b1425d2`). Bayt düzeni, alan listeleri ve vektörler DEĞİŞMEZ.

**A.1 — `AMP:8`'in önüne YAMA KAYDI (en yeni üstte):**
> **YAMA KAYDI <T> — K4G-05f —** §3 "Dosya alındısı, zamanlayıcılar" bölümünde `FILE_IDLE_TTL = 60 s` ve `FILE_RECEIPT_TTL = 60 s` üstü çizildi. Yerlerine akış başına ilerleme saatleri ve zarftan türetilmiş **yerel vazgeçme** kuralı geldi (`docs/design/K4G-05f-DOSYA-ZAMAN-ASIMI.md` §5–§9). `FILE_ABORT 0x04 timeout` gönderenin yerel vazgeçmesi oldu. `FILE_RECEIPT 0x03 timeout`'un anlamı genişledi. Oturum değişimi maddesine alıcı kuralı eklendi. Bayt düzeni, alan listeleri ve vektörler DEĞİŞMEDİ. Üstü çizili metin silinmedi.

**A.2 — `AMP:65` `FILE_ABORT` satırı:** `0x04 timeout` maddesinin ardına:
> **[<T> K4G-05f]** `0x04 timeout` = gönderenin yerel vazgeçmesi (ilerleme ya da alındı saati zarftan türetilmiş sınırı aştı); alıcı için "aktarım bitti" demektir, nedeni ağ hakkında bir iddia değildir.

**A.3 — `AMP:89` `FILE_RECEIPT` statüleri:**
- Eski: `` `timeout` = `FILE_IDLE_TTL` boyunca parça gelmedi ``
- Yeni: `` ~~`timeout` = `FILE_IDLE_TTL` boyunca parça gelmedi~~ **[<T> K4G-05f]** `timeout` = alıcı aktarımı tamamlanmadan bitirdi: alıcının yerel vazgeçmesi (ilerleme saati zarf sınırını aştı) ya da aktarımın oturumu tamamlanmadan kapandı ``

**A.4 — `AMP:90` "Alıcı zamanlayıcısı":**
- Maddenin bütünü üstü çizilir.
- Yeni madde:
> **[<T> K4G-05f] Alıcı ilerleme saati:**
> - Gelen dosya akışı başına tutulur.
> - Akışa verilmiş ama doldurulmamış kredi varken çalışır. Alıcının kendi tüketicisi tutarken ya da verilmiş kredinin tamamı kabul edilmişken durur.
> - Akışta ilk kez kabul edilen her yeni birimle sıfırlanır; sırasız tutulan dahil, uygulamaya teslim beklenmez.
> - Zarftan türetilmiş T_R'yi aşarsa alıcı yerel olarak vazgeçer: akışı kapatır (sonraki birimler ACK'lenir ve atılır), `FILE_RECEIPT(timeout)` gönderir.
> - Sabit saniye değeri yoktur; zarf ve türetim K4G-05f §8'de.

**A.5 — `AMP:91` "Gönderen zamanlayıcısı":**
- Maddenin bütünü üstü çizilir.
- Yeni madde:
> **[<T> K4G-05f] Gönderen ilerleme ve alındı saatleri:**
> - **Gönderim fazı:** `FILE_BEGIN` kabulünden FIN dahil bütün kayıtların teyit edildiği ana dek sürer. İlerleme yalnız akışın yeni (daha önce teyit edilmemiş) bir biriminin ilk teyididir; yinelenen ACK ilerleme sayılmaz. Saat kredi bloklu ve teyitsiz birim yokken durur. Teyit yokluğu alıcının almadığının kanıtı DEĞİLDİR.
> - **Alındı fazı:** bütün kayıtlar teyit edilince ya da aktarımın oturumu bitince başlar.
> - Bir saat zarftan türetilmiş sınırını (T_G, T_A) aşarsa gönderen yerel olarak vazgeçer: kalan birimleri iptal eder ve `FILE_ABORT(timeout)` gönderir. Sonra gelen alındı yok sayılır.
> - Sabit saniye değeri yoktur.

**A.6 — `AMP:92` "Oturum değişimi":**
- Maddenin sonuna:
> **[<T> K4G-05f]** Alıcı, aktarımın oturumunun `Closed`'unu okuduğunda aktarım tamamlanmamışsa onu bitirir ve eşin sıradaki oturumundan `FILE_RECEIPT(timeout)` gönderir. `FILE_END`'i kabul ettirilmiş ama teyidi gelmemiş aktarımın alındı fazı oturumun bittiği anda başlar.
- `AMP:99`'daki "`FILE_RECEIPT_TTL` … ile EŞİT SEÇİLDİ, onlara BAĞLI DEĞİLDİR" cümlesinin ardına:
> **[<T> K4G-05f]** `FILE_RECEIPT_TTL` kalktı; `SIGNALING_RESEND_TTL = 60 s` kendi değeridir.

## Ek B — Testler

Kapı yalnız deterministik hücrelerdedir (DUR-05f-12 (a)). Deterministik kayıp bir örüntü enjektörüyle kurulur (`EgressPolicy` deseni); netem'in rastgele kaybı yalnız B.8 (ii) raporunda kullanılır.

**B.1 Zorunlu tuzak** (client-core, `test-util`) — rev2'den aynen:
- Store'da akışın birimi yok; FIN dahil son kayıtlar offer kanalında ya da yerel tamponda bekletilir.
- İddia: `floor ≤ fin_seq` ve Faz A başlamaz.
- Serbest bırakılıp teyit edilince `floor = fin_seq + 1`.
- Mutant `floor = accepted` → kırmızı.

**B.2 31 B/s, 150 KiB, sahte saat** (kama-app, A+ sahte `06:299`; D-0 hücresi):
- İddialar:
  - hiçbir `SendFailed` / `Failed` olayı ve vazgeçme sayacı yok;
  - alıcıda `Received`, baytlar özdeş;
  - gönderende `Delivered`.
- Alt kol: iki parça arasına tek bir tam boy TEXT girer (≈ 67,7 s) → vazgeçme yok.
- Eski kod (`1b1425d2`) karşılığı kırmızı (kanca).

**B.3 Yerel vazgeçme — E_D dışı, deterministik:**
- (a) İleri yol T_G(D-3) + bir turdan uzun ölü:
  - tam bir `SendFailed(LocallyAbandoned)` olayı, [T_G, T_G + tur] içinde;
  - akış 1'de `FILE_ABORT(0x04)`;
  - `reset_flow` sonrası yeniden gönderim 0;
  - yol açılınca alıcıda `Aborted(Timeout)`.
- (b) Yol hiç açılmaz: alıcı T_R'de vazgeçer → `Failed(LocallyAbandoned)` + `FILE_RECEIPT(0x03)` yuvada. Gelen birimler ACK'lenip atılır.
- (c) Faz A'da FILE_RECEIPT kalıcı kayıp → T_A'da vazgeçme. Geç `ok` → `receipts_ignored`.

**B.4 KAPI — E_D içinde yanlış vazgeçme yok.** Her kol D-0…D-3 hücrelerinde, sınırın hemen altında; iddia her kolda vazgeçme = 0 ve tamamlanma:
- (a) **SACK kaybı:** k_r = 11 ardışık SACK düşer (K_slot kenarı). Gönderen ilk ACK'i periyodik SACK'le alır, yeni ileri deneme gerekmez. İleri iletim sayısı raporlanır; k_f = 0 iken fazladan ileri deneme gerekmez.
- (b) **İleri kayıp:** k_f = 3 ardışık iletim düşer.
- (c) **İki yönün etkileşimi** (C16 v2 §3 örneği; k_f = k_r = 1 deseni): veri kaybı → varış + SACK kaybı → veri kaybı → varış + SACK. Yuva canlıyken sınır toplanır kalır. Ölçülen teyit anı §8.2 formülünün altında.
- (d) **Tek eksik eski birim:** birim u k_f kez düşer, u'dan büyükler teyit edilir. `confirmed` artar, `floor` donar; alıcıda `received` artar. Vazgeçme yok.
- (e) **Bayat ACK:** aynı SACK tekrarlanır → `confirmed` sabit, saat sıfırlanmaz. Pozitif kontrol: gerçek durgunluk sınırı aşınca tam bir kez vazgeçme.
- (f) **Faz G'de kredi bloklu + tüketici tutuyor:** tutma ≥ 2·T_G → G ve R durur, vazgeçme yok.
- (g) **Faz A'da FILE_RECEIPT k_r kez düşer** → T_A içinde gelir → `Delivered`.
- (h) **60 s kesinti** (D-1, D-2, D-3).

**B.4′ E_D dışı, çarpımsal rejim — RAPOR, kapı değil:** k_r = K_slot + 1 = 12.
- Yuva `idle_hold`'da emekliye ayrılır, yeni bir ileri tur gerekir (DUR-05f-15).
- Ölçülen teyit anı ile formüldeki m = 2 sınırı (T_G ≈ 60 + 729,8 + 7·165,56 + 12·8,41 + 145,96 ≈ 2 195 s [hesap]) yan yana yazılır.

**B.5 Oturum geçişi** — rev2'den aynen: c1 FIN sonrası supersede · c2 alıcı `Closed`'da eksik → `SessionEnded` + `FILE_RECEIPT(0x03)` → `ReceiverTimeout` · c3 kapanış ve yeniden bağlanma · c4 FIN öncesi supersede · c5 erken iptal kaydı.

**B.6 Erişimci ve ACK takvimi girdileri:**
- **Erişimci:** `confirmed` birim başına tam bir kez artar; tekrar ACK, SACK ve RECOVERED artırmaz; İptal sayılmaz. `received` sırasız kabulde artar. Akış ya da oturum kapanınca `None`.
- **Takvim (adaptive-transport):** tek varıştan sonra yeni varış yokken SACK aralığı ≤ FLOOR + `ACK_TICK`. SACK block0'ı taşır (`ack.rs:121-124`). Yeni test; mevcut testler `receiver.rs:600-635` kadansı sınıyor, ama aralık iddiası yok.
- **Takvim (client-core):** yuva son varıştan `idle_hold` sonra emekliye ayrılır; yeni varış yeni `Receiver` açar. Bu K_slot'un girdisidir.

**B.7 Değişecek mevcut testler** (DUR-8 biçimi; kod commit'inde) — rev2'den aynen:
- `1b1425d2:crates/kama-app/tests/file_transfer.rs`'te 11 test;
- `1b1425d2:crates/kama-node/src/node/tests.rs:529-602`, `:678-707`;
- `1b1425d2:file_transfer.rs:2523-2598`, `:2645-2689`.

**B.8 INT-02 kolu** (05e; DUR-05f-12 (a)):
- **(i) KAPI — deterministik hücreler:**
  - gerçek 31 B/s, 150 KiB, kayıpsız (D-0; ≈ 88 dk tel baytıyla; C14 §6);
  - D-1…D-3 deseni bir örüntü enjektörüyle.
  - Hüküm: tamamlanma; G / R / A vazgeçme sayısı = 0; en uzun G ve R aralığı §8.2 sınırının altında.
- **(ii) RAPOR — rastgele hücreler:** B22 kayıplı hücreleri (`06:328`), 10 KiB.
  - Sütunlar: tamamlanma, süre, vazgeçme sayısı, ölçülen N_fırsat, bağımsızlık modeliyle öngörülen vazgeçme olasılığı (DUR-05f-16).
  - Hüküm yok.
- **Her iki kolda:** FILE_RECEIPT gecikmesi C15 v3 §5 (i) tanımıyla (doğum → ilk emisyon; ACK ve teslim ayrı; kesintili ve kesintisiz kol ayrı); 120 s yalnız karşılaştırmadır.

**B.9 Faz A tüketici tutması** — DUR-05f-14 = DUR-06-26; K4G-06 B29 (`06:335`):
- (a) seçilirse:
  - tutma ≤ D_h → vazgeçme yok;
  - tutma > D_h + T_ctrl → tam bir kez vazgeçme (yanlış "başarısız" beyanı).
- (b) seçilirse:
  - tutma ≥ 2·T_A ve "tutuluyor" girdisi k_r kez üst üste kaybolur → vazgeçme yok (T_taze);
  - tutma sırasında oturum biterse A sürer → tam bir kez vazgeçme.
- INT-02 `ek-tuketici-durak` (30 s, DUR-05c-3) bu testin ürün senaryosudur.

## Ek C — Tarihçe (MUST NOT IMPLEMENT)

> **MUST NOT IMPLEMENT.** Bu ekteki hiçbir ifade bağlayıcı değildir. Deprecated içerik appendix'e taşınır, sessizce silinmez (CLAUDE.md §6). Güncel metin gövde (§0–§13), Ek A ve Ek B'dir.

### C.1 — rev1 (`e5514e7a`)

> **MUST NOT IMPLEMENT.** rev1 CEVAP 15 v3 §6 ile reddedildi. Buradaki hiçbir ifade bağlayıcı değildir (CLAUDE.md §6: deprecated içerik appendix'e taşınır, sessizce silinmez).
>
> Yerine geçenler:
> - "taban 60 s artmadıysa alıcı kabul etmemiştir" ve `T_stall = FILE_IDLE_TTL` → §3 P1, §5 (ilerleme = yeni benzersiz teyit), §8 (zarftan türetilmiş yerel vazgeçme);
> - `SendFailed(Stalled)` (DUR-05f-2) → DUR-05f-7 / -9;
> - oturum geneli `accepted` / `confirmed_floor` → akış başına `FlowTxProgress` (§5);
> - DUR-05f-3 / -4 "beyanlı sınır" → gereksinim (§6, §7).
>
> Aşağıda rev1 gövdesi aynen, üstü çizili.

>
> ~~**K4G-05f — Dosya zaman aşımı sayacı (kama-app) — tasarım notu**~~
>
> ~~**K4G-05f** (CEVAP 14 §6, §7 (1), §8) · rev1 · taban main `1b1425d2` (bütün `dosya:satır` atıfları bu SHA'da) · 05c/05d atıfları kendi notlarına · **TASLAK:** kod 05d A1'den sonra gelir; hiçbir şey ölçülmedi.~~
>
> ~~**1. Kusur**~~
>
> - ~~Gönderen, `FILE_END` Transport kuyruğuna kabul edilince `AwaitingReceipt { since: now }` durumuna geçer (`crates/kama-app/src/file_transfer.rs:1538-1542`).~~
> - ~~`FILE_RECEIPT_TTL` (60 s, `:267`) o andan sayar (`:1471-1476`).~~
> - ~~`send` → `Ok` "kuyruğa alındı" demektir (`crates/kama-contracts/src/s1.rs:185`). Transport ≈ 1024 + 4096 birimi hemen kabul eder (`crates/client-core/src/runtime.rs:260`, `:1616`, `:2430`).~~
> - ~~31 B/s'de tam bir birim (26 + 11 + 1013 = 1050 B, Ek) ≈ 33,9 s sürer. 150 KiB = 158 birim, tel baytıyla ≈ 88 dk. Sonuç: geçerli dosya `ReceiptTimeout` alır.~~
> - ~~İkinci açık: Offering fazında süre sınırı yok. Geçici retlerde kayıt bekler (`file_transfer.rs:1547-1548`); oturum yokken de bekler (`:1505-1508`). İlerlemeyen aktarım süresiz açık kalır.~~
>
> ~~**2. İlerleme bilgisi uygulamaya nasıl ulaşır**~~
>
> - ~~**Kaynak (client-core, oturum başına iki sayı):**~~
>   - ~~`accepted`: oturuma kabul edilmiş birim sayısı, yani sıradaki birimin kimliği. Bugün `Sess.sent` olarak var (`crates/client-core/src/transport.rs:334-336`, `:1308-1310`). S1'de fragmenter kapalı olduğu için birim = AMP kaydı (`transport.rs:498-500`).~~
>   - ~~`confirmed_floor`: en küçük teyitsiz birim kimliği; teyitsiz birim yoksa `accepted`. Runtime görevi bunu her teyit drenajından sonra atomik olarak yayımlar; desen `outstanding` ile aynı (`runtime.rs:1533`, `:2131-2143`, `:1714-1716`).~~
>   - ~~Bu bir **durumdur**, olay akışı değil. Dolu kanalda düşebilen `acked()` olaylarından (`runtime.rs:2136`, `:2145` `try_send`) bu yüzden etkilenmez.~~
> - ~~**Önkoşul 05c:** main'de store bir `HashMap` (`runtime.rs:1906`) ve silme motordan bağımsız. 05c ile store `BTreeMap` olur ve silme yalnız `take_confirmed_units` ile yapılır (05c notu §3, D.5). Taban oradan O(1) okunur.~~
> - ~~**Yüzey:**~~
>   - ~~05d A1'in client-core erişimcisi aynı çağrıda `accepted` ve `confirmed_floor`'u da döndürür.~~
>   - ~~05d A1'in kama-app trait'ine bir metot eklenir; kama-node newtype'ı bunu geçirir (05d notu, DUR-05d-7).~~
>   - ~~`s1.rs` ve tel değişmez.~~
> - ~~**A1 ile uyumsuzluk:**~~
>   - ~~05d notu A1'i oturum başına "şu an kabul edilebilir birim", yani kapasite olarak tanımlar ve adını sabitlemez. 05f ise bir ilerleme **konumu** ister.~~
>   - ~~Aday "bekleyen birim = 0" anı tek başına yetmez. Sayı oturum genelidir. Aynı eşe sıradaki aktarım, öncekinin `FILE_END`'i kabul edilir edilmez akmaya başlar (`file_transfer.rs:61-62`, `:876-879`). RECEIPT ve PTT de aynı FIFO'dadır. Bu yüzden sıfır anı yük altında hiç gelmeyebilir.~~
>   - ~~`confirmed_floor > idx_end` aynı anı yük altında da verir; boştayken "bekleyen = 0" ile çakışır. → **DUR-05f-1**.~~
> - ~~**FILE_END'in tek başına ACK'i neden yetmez:** ACK seçicidir (SACK, `crates/adaptive-transport/src/ack.rs:1-6`); `FILE_END`'in teyidi önceki parçaların teyidini içermez. Gereken, `FILE_END` dahil bütün önekin teyididir.~~
>
> ~~**3. Sayaç: başlangıç, durma, sıfırlama**~~
>
> ~~Aktarım başına iki değer tutulur:~~
> - ~~`end_on`: `FILE_END`'i kabul eden tutamaç. Mevcut kural gereği `began_on` ile aynıdır (`:1509`).~~
> - ~~`idx_end`: `FILE_END` `send` → `Ok` döndükten hemen sonra okunan `accepted − 1`. Düğüm döngüsü tek iş parçacıklıdır (`crates/kama-node/src/node.rs:1`); `send` ile okuma arasına başka üretici giremez.~~
>
> ~~**Faz G (gönderim):**~~
> - ~~Başlangıç: `FILE_BEGIN` kabul edildiği an, `last_progress = now`.~~
> - ~~Sıfırlama: `end_on` oturumunun `confirmed_floor`'u her arttığında `last_progress = now`.~~
> - ~~**İlerleme kesildi:** taban ≤ `idx_end` iken (ya da `FILE_END` henüz kabul edilmemişken) `now − last_progress ≥ T_stall`. Sonuç: `SendFailed(Stalled)` (**DUR-05f-2**); eşe bir şey gitmez, alıcı kendi `FILE_IDLE_TTL`'iyle biter.~~
> - ~~Oturum yokken ilerleme olmaz; sayaç sürer.~~
> - ~~Faz G'nin sonu: taban > `idx_end` görüldüğü an.~~
>
> ~~**`T_stall` = `FILE_IDLE_TTL` (60 s, `:262`). Yeni sayı yok.**~~
> - ~~Türetim: 05c'den sonra teyit ancak alıcı kabulünden doğar (05c notu §1). Taban `T_stall` boyunca artmadıysa alıcı o süre boyunca o oturumdan yeni birim kabul etmemiştir.~~
> - ~~O zaman alıcı aktarımı kendi zamanlayıcısıyla `IdleTimeout`'a götürür (`:1165-1185`): en geç `T_stall` + ACK yolu kadar sonra.~~
> - ~~İki uç aynı eşiği iki ayrı tarafta ölçer; gönderen alıcıdan önce vazgeçmez.~~
>
> ~~**Faz A (alındı):**~~
> - ~~Başlangıç: taban > `idx_end` görüldüğü an.~~
> - ~~Süre: `FILE_RECEIPT_TTL` = 60 s, değişmez. Artık yalnız karşı tarafın işlemesi ve alındının dönüşü sayılır.~~
> - ~~Sıfırlanmaz. Durma: son olay (`Delivered` / `SendFailed`).~~
>
> ~~Okuma ve denetim `pump` içinde yapılır; zamanlayıcılar zaten orada (`:866-869`). Düğüm her 20 ms'de bir tur döner (`node.rs:478`, `lib.rs:127`).~~
>
> ~~**4. Oturum geçişi**~~
>
> ~~Sayılar oturuma bağlıdır: her oturumun ayrı bir ucu var (`transport.rs:1021-1048`) ve birim kimliği her oturumda 0'dan başlar (`runtime.rs:1920`).~~
>
> - ~~**`FILE_END` kabul edilmeden geçiş:** mevcut kural, değişmez: `FILE_ABORT(superseded)` → `SendFailed(Superseded)` (`file_transfer.rs:1509-1515`).~~
> - ~~**`FILE_END` kabul edilmiş, taban ≤ `idx_end` iken oturum değişmiş:** eşin güncel oturumu ≠ `end_on` (supersede, kapanış, yeniden bağlanma) ya da erişimci `None` döner (uç `Closed`'da bırakılır, `transport.rs:942`). **Faz A o anda başlar.**~~
>   - ~~Gerekçe: eski oturum gönderemez (drain'de `retire_send`, `crates/kama-noise-wire/src/session_control.rs:524-538`) ve birimler yeni oturuma taşınmaz (`transport.rs:45`). Teyit bilgisi bir daha gelmez.~~
>   - ~~Mevcut "`FILE_END`'i kabul edilmiş aktarım kapatılmaz, alındı bekler" kuralı (`file_transfer.rs:135-137`) korunur; bekleme artık süreyle sınırlıdır.~~
> - ~~**Yenileme:** adaptör cutover'ı eski oturum boşalana ya da 3 s dolana dek tutar (`transport.rs:780-799`, `:207-209`; `runtime.rs:269`). Bu yüzden taban çoğunlukla geçişten önce `idx_end`'i aşar.~~
> - ~~**Faz A'da geçiş:** etkisiz. Alındı eş anahtarlıdır ve herhangi bir oturumdan gelebilir (`file_transfer.rs:1135-1149`).~~
>
> ~~**5. Testler**~~
>
> - ~~**(a)** kama-app, sahte saat + A1 trait'ini uygulayan oranlı sahte transport. 31 B/s, 150 KiB, tek dosya, kayıpsız; birim başına teyit 1050 B / 31 B/s. Beklenen:~~
>   - ~~`ReceiptTimeout` = 0 ve `IdleTimeout` = 0;~~
>   - ~~alıcıda `Received`, baytlar özdeş;~~
>   - ~~gönderende `Delivered`;~~
>   - ~~eski kodda `ReceiptTimeout` (kanca kırmızısı).~~
> - ~~**(b)** İlerleme kesilir: taban dondurulur → `T_stall` + bir tur içinde tek bir `SendFailed(Stalled)`; alıcıda `IdleTimeout`. Alt kol: Offering'de sürekli `Backpressure` + donuk taban → aynı sonuç. Alt kol: oturum yok → aynı sonuç.~~
> - ~~**(c)** Oturum geçişi:~~
>   - ~~(c1) `FILE_END` kabul edilmiş, taban geçilmeden supersede → Faz A başlar; alındı gelirse `Delivered`, gelmezse 60 s sonra `ReceiptTimeout`;~~
>   - ~~(c2) kapanış;~~
>   - ~~(c3) kapanış + yeniden bağlanma;~~
>   - ~~(c4) `FILE_END` öncesi supersede → `Superseded` (mevcut davranış).~~
> - ~~**(d)** client-core erişimci: sırasız SACK teyidinde taban artmaz, önek tamamlanınca artar; uç bırakılınca `None`.~~
> - ~~**(e)** INT-02 kolu (05e, C14 §7 (1)): gerçek 31 B/s tek dosya, ≈ 88 dk. Birim testi değil.~~
>
> ~~**6. Bağımlılık · 7. Süre**~~
>
> - ~~**Sıra (C14 §8):** 05c (store `BTreeMap` + `take_confirmed_units`) → 05d A1 (erişimci + trait + newtype) → 05f kodu. 05f A1 tipine iki alan, trait'e bir metot ekler.~~
> - ~~**Süre ≈ 2 iş günü (1,5–2,5):**~~
>   - ~~erişimci alanları + (d): 0,5;~~
>   - ~~faz ve sayaç mantığı: 0,5;~~
>   - ~~`Stalled` nedeninin kama-node ve kama-mobile aynası: 0,25 (DUR-05f-2 (a) seçilirse);~~
>   - ~~testler (a)–(c): 0,75.~~
>
> ~~**8. DUR adayları**~~
>
> ~~| DUR | Tür | Soru · seçenekler | Öneri |~~
> ~~|---|---|---|---|~~
> ~~| **DUR-05f-1** | sözleşme | (a) A1 erişimcisi `accepted` + `confirmed_floor` döndürür, A1 trait'ine metot eklenir · (b) yalnız "bekleyen = 0" (yük altında başlamayabilir; Faz G sınırı yine gerekir) | **(a)** |~~
> ~~| **DUR-05f-2** | sözleşme | İlerleme kesilince neden: (a) yeni `SendFailure::Stalled` (kama-app pub enum, kama-node `FileFailReason::Outgoing` `event.rs:168-186`, kama-mobile UniFFI aynası değişir) · (b) `ReceiptTimeout` yeniden kullanılır (yüzey aynı kalır, anlam karışır) | **(a)** |~~
> ~~| **DUR-05f-3** | beyanlı sınır | Faz A ters yöndeki FIFO'yu da sayar. Alıcının giden kuyruğunda `FILE_RECEIPT`'in önünde 1,77'den fazla tam birim varsa (60 / 33,9) 31 B/s'de `ReceiptTimeout` yanlış çıkar. Ters FIFO sınırı W_F'ye bağlı (DUR-05d-8); ölçüm C14 §7 (2). (a) beyan · (b) ters yönde birim geldikçe sıfırla + W_F'den türetilmiş üst sınır (yeni sayı) | **(a)** |~~
> ~~| **DUR-05f-4** | beyanlı sınır | Alıcının `FILE_IDLE_TTL`'i gönderenin FIFO'sunu ve ARQ beklemesini de sayar. 31 B/s'de iki parça arasına tek bir tam birim ya da tek bir yeniden gönderim girerse aralık ≥ 67,7 s > 60 s olur ve geçerli dosya `IdleTimeout` alır [çıkarım]. Test (a) bu yüzden tek dosya ve kayıpsız; INT-02 kolunda beyan edilir. 05f kapsamı dışında | beyan |~~
> ~~| **DUR-05f-5** | beyanlı sınır | Teyit = alıcı ucun ACK'i (direct-mode; hub rölesi yok, `transport.rs:12-14`). Hub hop-ARQ gelirse taban hop teyidi olur | beyan |~~
>
>
>
> ~~**Ek — aritmetik.**~~
> - ~~Tel boyu: oturum dış ek yükü 26 B (`OUTER_HDR_LEN` 10 + `TAG_LEN` 16; `crates/kama-noise-wire/src/outer.rs:9-11`, `envelope.rs:56`, `:88-89`) + DATA başlığı 11 B (`client-core/src/wire.rs:17`) + birim 1013 B (`runtime.rs:245`) = 1050 B. 1050 / 31 = 33,87 s.~~
> - ~~Parça yükü 1013 − 5 − 18 = 990 B (`kama-app/src/codec/limits.rs:101-142`). 150 KiB = 153 600 B → 156 parça (son parça 150 B) + `FILE_BEGIN` + `FILE_END` = 158 birim.~~
> - ~~Tel toplamı ≈ 155 × 1050 + 210 + 105 (ad 10 B) + 50 ≈ 163,1 kB → 5 262 s ≈ 88 dk. Sahibin ≈ 83 dk'sı yalnız yük baytıdır (153 600 / 31).~~
> - ~~DUR-05f-4: 2 × 33,87 = 67,7 s.~~

### C.2 — rev2 (`3e0ec8fa`): §8 ve DUR-05f-8 / DUR-05f-11 satırları

> **MUST NOT IMPLEMENT.** CEVAP 16 v2 §3 ile reddedildi.
>
> - `(k_f + k_r + 1)` çarpanı iki yöndeki ardışık kayıp tavanlarından çıkmaz → §8.2 (gerçek ACK takvimi, K_slot; k_r > K_slot iken çarpımsal rejim).
> - ε ile türetilen k_f / k_r ve rastgele hücrelerin tablosu deterministik garanti gibi yazılmıştı → §8.1 iki sözleşme.
> - D_tük = 0 → §7 ve DUR-05f-14.
> - T_R'nin bileşenleri eksikti → §8.2 (D_ack, T_kredi, T_ctrl(k_f)).
>
> Atıflar rev2'nin kısa adlarıyladır: `06` = `dfd3a5e5`, `05b` = `37b78aad`. Aşağıda metin aynen, üstü çizili.
>
> ~~**8. Yerel vazgeçme politikası ve zarf**~~
>
> ~~**8.1 Zarf E** [türetim; ilk sürüm = K4G-06 B22 hücreleri, `06:323`]:~~
>
> ~~| Parametre | Anlam | B22 değerleri |~~
> ~~|---|---|---|~~
> ~~| r_min | Pacer hızı; akış payı r_f = φ·r_min, φ K4G-06 §5'ten | 31 B/s (`1b1425d2:crates/adaptive-transport/src/sender.rs:1485`); tek dosya → φ ≈ 1 |~~
> ~~| sunulan yük | Rakip sınıfların yükü (φ'yi belirler) | yok (tek dosya) |~~
> ~~| N_o, D_o | Kesinti sayısı ve azami süresi, iki yön; **oturumsuz aralık da kesinti sayılır** | {0}, {1 × 60 s} |~~
> ~~| p_f → k_f | İleri yolda ardışık kayıp tavanı | p_f ∈ {0, %10} |~~
> ~~| p_r → k_r | Dönüş yolunda ardışık kayıp tavanı (ACK, kredi, FILE_RECEIPT) | p_r ∈ {0, %80} |~~
> ~~| ε | Aktarım başına yanlış vazgeçme olasılığı tavanı | 10⁻³ [CAL] |~~
> ~~| D_tük | Alıcı tüketicisinin tutma payı (yalnız Faz A) | 0 [CAL] (ürün tüketicisi = düğüm turu) |~~
> ~~| T_işlem | Alıcının son kayıttan alındıya kadar işlemesi | ≤ 1 s [CAL] |~~
>
> ~~Kayıp dizisi sınırı rastgele kaybı sonlu hâle getirir. k = ⌈ln(ε/N) / ln p⌉ − 1, p = 0 ise k = 0; N = aktarımın birim sayısı. Bu yaklaşık bir değerdir: fırsat sayısı ≈ N alınır, ε birim fırsatlarına bölünür.~~
>
> ~~**8.2 Sınırlar** [türetim]:~~
> - ~~Girdiler 05b rev6 sınıf sınırlarıdır: R = r_f'de T_max(DATA), P_c = T_max(kopya), A, H_maks, L (`37b78aad:05b:556-585`).~~
> - ~~"Herhangi bir" teyitsiz birimin yeniden hizmeti ≤ A + P_c içinde olur: kayıt ≤ A içinde vadeli, sınıf ≤ P_c içinde hizmet verir (`:571`). Birim başına T_birim gerekmez, çünkü ilerleme herhangi bir birimin yeni teyidiyle oluşur. T_BLK = min(RTO_MAX, `idle_hold`/2) (`06:145`).~~
> - ~~**T_G(E)** iki durumda:~~
>   - ~~kayıpsız ve kesintisiz: L + T_max(DATA);~~
>   - ~~diğer: N_o·D_o + L + H_maks + (k_f + k_r + 1) · max(A + P_c, T_BLK).~~
>   - ~~Gerekçe: H_maks kesinti ya da kayıptan sonraki ilk sahiplenmedir. Her hizmet bir ileri denemedir (en çok k_f ardışık kayıp). Her varış ACK ister, kopya varışı ACK'i zorlar (`1b1425d2:receiver.rs:222-230`); dönüşte en çok k_r ardışık kayıp. İlk başarılı ACK, varan teyitsiz birimi yeni teyit olarak getirir.~~
> - ~~**T_ctrl(E):** tek bir kontrol biriminin (FILE_ABORT ya da FILE_RECEIPT) karşıya ulaşması.~~
>   - ~~kayıpsız: T_C + L;~~
>   - ~~diğer: T_C + L + N_o·D_o + H_maks + k·(A + P_C).~~
>   - ~~k, birimin gittiği yönün kayıp tavanıdır: FILE_RECEIPT için k_r, FILE_ABORT için k_f.~~
>   - ~~T_C = (S_iletimde + S_baş,C)/r + S_baş,C/(ρ_C·r) (`06:113`). S_baş,C ≈ 50 B (26 B mühür + 10 B FLOW_BATCH + 14 B kayıt) [hesap]. ρ_C = 0,1 [CAL, `06` §4]. P_C ≈ T_C.~~
> - ~~**T_A(E) = T_işlem + D_tük + T_ctrl(E)** · **T_R(E) = T_G(E) + T_ctrl(E).**~~
>
> ~~**[hesap]** 31 B/s, r_f = r. Kullanılan değerler: T_max(DATA) = 101,68 s; P_c = 135,56 s; A + P_c = 165,56 s; H_maks = 729,8 s; L = 72,98 s; T_C = 51,6 s; A + P_C = 81,6 s. Gevşek üst sınırlardır, ölçülmedi. K4G-06 DUR-06-3 sayılarıyla yeniden hesaplanır.~~
>
> ~~| Hücre (p_f / p_r / kesinti) | N | k_f / k_r | T_G | T_ctrl (alındı) | T_A | T_R |~~
> ~~|---|---|---|---|---|---|---|~~
> ~~| 0 / 0 / yok | — | 0 / 0 | 174,7 s | 124,6 s | 125,6 s | 299,2 s |~~
> ~~| 0 / 0 / 1 × 60 s | — | 0 / 0 | 1 028,3 s | 914,4 s | 915,4 s | 1 942,7 s |~~
> ~~| %10 / %80 / 1 × 60 s, 150 KiB | 158 | 5 / 53 | 10 630,8 s (2,95 sa) | 5 239,2 s | 5 240,2 s | 15 870,0 s (4,41 sa) |~~
> ~~| %10 / %80 / 1 × 60 s, 10 KiB (B22) | 13 | 4 / 42 | 8 644,1 s (2,40 sa) | 4 341,6 s | 4 342,6 s | 12 985,7 s |~~
>
> ~~Kayıpsız hücrede 31 B/s'de birimler arası ≈ 33,9 s'dir, alıcının R saati de bu kadar aralıkla sıfırlanır [hesap]. Yerel vazgeçmenin sınırı (174,7 s / 299,2 s) bu aralığın üstündedir.~~
>
> ~~**8.3 Vazgeçme eylemleri** (tek seferlik, tekilleştirilmiş):~~
> - ~~**G dolumu (gönderen):**~~
>   1. ~~`reset_flow(f)` → motor akışın birimlerini iptal eder; yeniden gönderim durur (`06:148`, `:164`).~~
>   2. ~~Akış 1'e `FILE_ABORT(0x04 timeout)`; H_C_tavan içinde, P5.~~
>   3. ~~Tek bir `SendFailed(<yerel vazgeçme>)` (DUR-05f-9).~~
> - ~~**A dolumu (gönderen):** akış 1'e `FILE_ABORT(0x04 timeout)` + `SendFailed(<yerel vazgeçme>)`. Sonradan gelen alındı yok sayılır (`receipts_ignored`, `1b1425d2:file_transfer.rs:1152`).~~
> - ~~**R dolumu (alıcı):**~~
>   1. ~~Akış yerelde kapanır: sonraki birimler ACK'lenip atılır (`late_after_close`, `06:53`); bu yüzden gönderen yeniden göndermeyi bırakır.~~
>   2. ~~Akış 1'e `FILE_RECEIPT(0x03 timeout)`.~~
>   3. ~~Tek bir `Failed(<yerel vazgeçme>)`.~~
> - ~~**Karşı tarafın gördüğü:**~~
>   - ~~Gönderen vazgeçtiyse alıcı `Aborted { reason: Timeout }` görür; bu mevcut olaydır (`1b1425d2:file_transfer.rs:1114-1132`).~~
>   - ~~Alıcı vazgeçtiyse gönderen `SendFailed(ReceiverTimeout)` görür (`:351-360`).~~
> - ~~**Zarf dışında:** yanlış "başarısız" mümkündür; yanlış "teslim edildi" mümkün değildir (DUR-05f-11).~~
>
> ~~| **DUR-05f-8** | yeni sayı | Zarfın üç [CAL] değeri: ε = 10⁻³ (aktarım başına) · D_tük = 0 · T_işlem ≤ 1 s. k_f/k_r bunlardan ve p'den türer (§8.1). Zarfın ilk sürümü B22 hücreleri | bu değerler; son değer INT-02'den sonra |~~
> ~~| **DUR-05f-11** | beyanlı sınır | Zarf dışında yanlış vazgeçme mümkün: yanlış "başarısız", yanlış "teslim edildi" değil; vazgeçmeden sonra gelen `ok` yok sayılır. D_tük = 0 ise alıcı sürecinin askıya alınması zarf dışıdır | beyan + B.8'de sayılır |~~
