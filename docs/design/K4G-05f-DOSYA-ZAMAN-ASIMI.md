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
- **Alındı kuyruğu:** küresel 4 kayıtlık; taşınca ya da 60 s'yi aşınca düşer (`:291`, `:1221-1223`, `:1243-1244`).
- **Erken iptal kaydı:** ömrü `FILE_IDLE_TTL` (`:150-152`).
- **Normatif metin:** `AMP:89-92`.

**Belirsizlikler** [olgu]:
- `send` → `Ok` yalnız "kuyruğa alındı" demektir (`1b1425d2:crates/kama-contracts/src/s1.rs:185`).
- `acked()` bildirimi kanal dolunca düşebilir (`1b1425d2:runtime.rs:2136`, `:2145`).
- Motor birimi terk etmez (`1b1425d2:transport.rs:26-27`).

**ACK'leri kim gönderir** [olgu, `1b1425d2`]:
- Her düğümde tek bir `ServerEndpoint` vardır (`transport.rs:510-517`) ve oturum başına bir `ClientEndpoint` (`transport.rs:1037-1046`).
- Yalnız sunucu görevi `Receiver` tutar (`runtime.rs:3349`); istemci görevi yalnız ACK tüketir (`runtime.rs:1791`).
- Dolayısıyla her yönde ACK'i **alan düğümün sunucusu** gönderir:
  - Dosya yönü (A → B): B'nin sunucusu, A'nın istemci soketini SACK'ler.
  - FILE_RECEIPT yönü (B → A): A'nın sunucusu, B'nin istemci soketini SACK'ler. Bu ayrı bir yuvadır ve ömrü ayrıdır.

**ACK takvimi** [olgu, `1b1425d2`] — §8.2'nin girdisi:
- **Varışa bağlı ACK:** yeni DATA'da her `ACK_FREQ_N = 2` varışta bir ACK (`receiver.rs:76`, `:172`, `:434-445`).
  - Aynı PN'nin tekrarı için zorlanmış ACK yolu vardır (`receiver.rs:222-230`), ama ürün yolunda erişilemez. Yeniden gönderim yeni bir PN taşır (`runtime.rs:2922-2925`); kripto aynı PN'yi AEAD'den önce reddeder (`crates/kama-noise-wire/src/protector.rs:559`, `:603` → `pn.rs:125`).
  - Bu yüzden bir kopya, `ACK_FREQ_N` sayacına giren sıradan bir yeni DATA'dır.
- **Zamanlayıcıya bağlı ACK:** sunucu her `ACK_TICK = 20 ms`'de bütün yuvalar için `on_ack_timer`'ı çağırır (`runtime.rs:69`, `:3408-3419`); fonksiyon yalnız en az bir varış görülmüşse çalışır (`receiver.rs:268-271`). Üç yol:
  - kuyruk SACK'i (`:282-292`; `K_TAIL = 2`, `:100`);
  - epizot kotası: 4 SACK, aralarında `T_SPACING` ≈ 2,10 s (`:111`, `:115`, `:298-306`, `:332-336`);
  - kadans: clamp(`K_ARR` × aralık, `T_SPACING`, FLOOR) ya da FLOOR (`:309-323`). FLOOR = 65 B / (0,25 × 31 B/s) ≈ 8,387 s, **sabit bir önselden** gelir (`RETURN_BW_PRIOR`, `:83-94`); ölçülen dönüş bandına bağlı değildir.
- **Sonuç:** bir varıştan sonra alıcı, yeni varış beklemeden ve kendiliğinden durmadan en geç **T_ACK = FLOOR + ACK_TICK ≈ 8,41 s**'de bir kümülatif SACK yayar. Mevcut kadans testi: `receiver.rs:624-647`.
- **SACK kapsamı:**
  - SACK en yüksek blok olan block0'ı her zaman taşır; `MAX_ACK_RANGES = 16` kırpması yalnız alt blokları keser (`ack.rs:36`, `:121-132`).
  - Dolayısıyla birim u, yalnız **oturum yuvasındaki en yüksek PN'li varış olduğu sürece ya da üstünde 16'dan az boşluklu blok varken** SACK'te kalır.
  - Yuva (adres, oturum) başınadır, akış başına değildir (`runtime.rs:3106`); başka akışlar ve FEC onarım parçaları da PN alır.
  - 31 B/s tek dosya hücrelerinde 105 s içinde ≤ 4 tam birim varır, koşul tutar [çıkarım]. Çok akışlı ya da FEC'li hücrede bu bir zarf koşuludur (§8.1).
- **Yuva ömrü** — SACK durumunun kaybolduğu iki yol vardır:
  - (i) Son varıştan `idle_hold` = 105 s sonra emeklilik (`runtime.rs:270`, `:3430`; varış damgası `:3544`; 1 s'lik `PATIENCE_TICK` ile gerçekte 105–106 s, `:71`).
  - (ii) **Oturum kapanınca ≈ 1 s içinde** emeklilik, `idle_hold` beklenmez (`runtime.rs:3428`, `:3154-3158`).
  - Her iki yoldan sonra yeni bir varış yeni bir `Receiver` açar (`runtime.rs:3129-3132`). Periyodik SACK bir varıştan sonra en çok `idle_hold` sürer.

**05c sonrası** [olgu, `0c22a07d`]:
- Teyit yalnız `take_confirmed_units` çıktısıdır, birim oturumda en çok bir kez çıkar (`runtime.rs:2138-2156`).
- Store `BTreeMap` (`:2024`).
- Kabul edilmiş birimin tekrarı ACK'lenir (`:3396-3398`).

**Gönderenin kurtarma takvimi** [olgu, `593029eb:crates/adaptive-transport/src/sender.rs`]:
- H-sahiplenmesinin vadesi = max(son ilerleme, adayın **kendi son iletimi**) + H (`:6866-6878`).
- Sahiplenme yalnız vadesi o an olan geometrik bir kopya kaydı açar (`:6882-6887`). Kopya sınıfı bu kaydı ≤ P_c içinde hizmete alır (`05b:595`, `:605`).
- Dolayısıyla ilk yeniden gönderim, birimin son iletiminden **≤ H + P_c** sonradır. Sonraki her yeniden gönderim ≤ T_tx = A + P_c sonradır.

**05b rev6 girdileri** [türetim; **T1/T2 FİKSTÜR değerleri, ürün sınırı DEĞİL**, `05b`]:
- Değerler (31 B/s): T_max(DATA) = 101,68 s, T_max(kopya) = P_c = 135,56 s (`:612-615`); A = RTO_MAX = 30 s (`:590`); H_maks (T2, srtt ≤ L) = 729,8 s (`:595`); L = (tavan + S_maks)/R + 2d = 72,98 s (`:619`).
- Fikstür koşulları: τ = 10 ms, S = 1050 B, **P_pf = 0 (kuyruk-probu ve FEC kapalı, `:592`)**, d = 200 ms tek yön.
- Ürün kipindeki sınırlar C16 v2 §1.2'nin T1′/T2′ sonucuna bağlıdır; 05b şeridi bunları türetiyor.
- C16 v2'nin dayanak satırının gerçekte teyit ettiği: T1/T2'nin dört testi yeşil ve raporla birebir — 31 B/s'de DATA 67,73 s / ARQ 68,92 s / kopya 101,6 s, sınırlar 101,68 / 101,68 / 135,56 s, T_birim 1444,5 s, T2 en uzun bekleme 124,9 s (`decisions/C16-v2.md:7`). H_maks ve L orada ayrıca teyit edilmedi.

**K4G-06 rev5** [tasarım, `06` = `a517d4a0`]:
- Akışlar (`06:59-60`). Kontrol şeritleri 0 ve 1 `Independent` kipte (`06:81`, `:89-90`; DUR-06-27 önerisi).
- FLOW_CREDIT tetiklidir (`06:95`, `:201`). BLOCKED aralığı ≤ min(RTO_MAX, `idle_hold`/2) (`06:202`). FLOW_CREDIT, eş başına ≤ 1 çerçeve / T_min (`06:215`).
- Kontrol kovası ve kontrol başı sınırı (`06:101`, `:170`).
- FIN, RESET ve H_C_tavan (`06:204-210`). Oturum değişimi (`06:211`). Tek sonlandırma çıktısı (`06:221`).
- **Ortak yaşam döngüsü:** `06 §16 YD-1…YD-9` (`06:324-418`). YD-3 (`:352`), YD-4 (`:363`), YD-5 (`:377`), YD-7 (`:398`), YD-9 (`:412`).
- DUR-06-26 / -27 / -28 açıktır (`06:786`, `:720`, `:803`); sahip kararı değildir.

## 3. İlkeler

- **P1 — kanıt yoksa çıkarım yok.** Yeni teyit, yeni kabul ya da alındı gelmemesi karşı taraf hakkında bir durum türetmez.
- **P2 — terminal durumlar açık sinyalle oluşur** (`06 §16 YD-4`): alındı · `FILE_ABORT` · `FLOW_RESET` (kodlu; DUR-06-28 (a) ile) · `stop_flow` + FILE_RECEIPT · oturum sonu kuralı (§9) · kullanıcı iptali · yerel vazgeçme (P3).
- **P3 — yerel vazgeçme bir politikadır.** Yalnız yerel bir saat, E_D'den türetilmiş sınırını aşınca tetiklenir; nedeni "yerel vazgeçme"dir; karşıya açık mesajla bildirilir. E_D içinde tetiklenmez (KAPI, Ek B.4). Rastgele hücrelerde tetiklenebilir (RAPOR, Ek B.8 (ii)).
- **P4 — akış başına.** Oturum geneli taban, oturum geneli "bekleyen = 0" ve ikinci bir geçici erişimci kullanılmaz (C16 v2 §5.4).
- **P5 — vazgeçme yeni kapasite istemez.** H_C_tavan içinde yer önceden ayrılmıştır (`06:206-210`; `06 §16 YD-6`). Aktarım başına en çok bir FILE_ABORT (ilk neden kalır, `1b1425d2:file_transfer.rs:134`); gelen akış başına en çok bir FILE_RECEIPT.

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
- **R4 — saatlerin sırası:** T_R = max(D_ack + T_G + T_ctrl(k_f), T_kredi + F_yeni) (§8.2). İlk terim, gönderenin vazgeçmesinin ve bildiriminin alıcıdan önce gelmesini sağlar; ikinci terim E_D içinde R sessizliğinin sınırıdır. T_A: §7.
- **R5 — 120 s (S1) saat değildir.** FILE_RECEIPT için ölçüm hedefidir: C15 v3 §5 (i) tanımıyla doğum → ilk emisyon; ACK ve karşı uygulamaya teslim ayrı; kesintili ve kesintisiz kol ayrı. TEXT/PTT'ye uygulanmaz.

**4.3 Bir aktarımın beklemeleri.**

| Bekleme | Kapasite mi? | Saat | Sınır terimi (§8.2) |
|---|---|---|---|
| Akış bütçesi yok (`admissible` = 0, A.36) ya da kredi bloklu, motora verilmiş teyitsiz birim yok | evet | G durur (§5) | — (bekleten tarafın saati) |
| Kredi yenilemesi (BLOCKED → FLOW_CREDIT) | evet | G durur; alıcıda R çalışır | T_kredi (R'nin ikinci terimi) |
| Hizmet sırası (DRR, katman) | hayır | G çalışır | T_max(DATA), P_f, φ_min |
| İleri kayıp (ARQ / kopya) | hayır | G çalışır | F(n): H + P_c ilk yeniden gönderim, sonra T_tx |
| SACK kaybı, yuva penceresi | hayır | G çalışır | (min(k_r, K_slot(0)) + 1)·T_ACK; k_r + 1 > B₁ ise m tur; kesinti B₁'i küçültür |
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
- **G saati:** çalışır = motora verilmiş teyitsiz birim var ∨ (kredi ve akış bütçesi var ∧ sunulmayı bekleyen kayıt var). Durur = (`blocked` ∨ akış bütçesi yok, A.36) ∧ motora verilmiş teyitsiz birim yok — §4.3 ile aynı koşul. Sıfırlanır = yalnız `confirmed` artınca. Dolum = T_G(E_D) → §8.3.
- **Tuzak:** store boşken FIN offer kanalında ya da yerel tamponda olabilir; o anda `floor` = o kaydın `flow_seq`'idir, `accepted` değildir (Ek B.1).

## 6. Alıcı dosya-idle (R saati) — `FILE_IDLE_TTL`'in yerine

Erişimci `FlowRxProgress(oturum, akış)` — A.36 (`06:461`):
- `received`: ilk kez kabul edilen farklı `flow_seq`; sırasızlar dahil.
- `delivered`: uygulamaya verilen sıradaki `flow_seq`.
- `granted`: ilan edilmiş `max_seq` (`06:201`).
- `held`: `hold_flow` etkin.

R saati — R1'de (`06 §16 YD-1`, `YD-7`):
- **Çalışır:** `granted`'ın altında henüz kabul edilmemiş `flow_seq` varken.
- **Durur:**
  - `held`;
  - verilmiş kredinin tamamı kabul edilmiş;
  - FIN dahil önek kabul edilmiş (R2: bundan sonrası A saatinin ve YD-3'ün konusudur);
  - `stop_flow` edilmiş.
- **Sıfırlanır:** `received` artınca. Gönderenin G sıfırlanmalarına bağlı değildir. Ölen bir yuvanın gecikmiş eski teyitleri R'yi etkilemez (T_R'nin ikinci terimi, §8.2).
- **Dolum:** T_R(E_D) (§8.2) → §8.3.

31 B/s'de iki tam birimin arasındaki 67,7 s, R saatini sıfırlayan bir ilerleme aralığıdır; vazgeçme yalnız T_R aşılırsa olur (C15 v3 §6: DUR-05f-4 gereksinimi).

## 7. Alındı bekleme (A saati) ve tüketici tutması — `06 §16 YD-3` ile hizalı

- **Yer:**
  - FILE_RECEIPT akış 1'de gider ve yuvası düşürülmez (`06:90`, A.21 `06:446`; yükümlülük `06 §16 YD-6`).
  - Akış 1 `Independent` ise FILE_RECEIPT öndeki bir kontrol kaydını beklemez (DUR-06-27 (c) = DUR-05f-17).
- **A saati:**
  - S2'de, yani Faz A'nın başında başlar (`06 §16 YD-1`).
  - Oturumsuz aralık N_o·D_o'dan sayılır.
  - Dolum = T_A(E_D) → §8.3.
- **Durum:** FIN dahil bütün birimler teyitliyken alıcının tüketicisi FIN'i almazsa (R2, `06 §16 YD-1`) FILE_RECEIPT doğmaz. Faz G'deki tutma sorun değildir: kredi bloklu olduğu için G durur (`06 §16 YD-7`; B1 `06:484`).

**Seçenekler (DUR-05f-14 = DUR-06-26; ikisi birlikte karar).**

**(a) Tutma zarfa girdi olarak girer:**
- T_A = T_işlem + N_h·D_h + T_ctrl(k_r), N_h = 1, D_h ≥ 30 s [CAL].

**(b) K4G-06 önerisi kabul edilirse** — `06 §16 YD-3`: birikimli `HELD(flow_id, held_ms)`.
- Alıcı: R2 ≥ T_min olunca girdiyi koyar, R2 sürdükçe her T_BLK'da yineler (eş başına ≤ 1 çerçeve / T_min).
- Gönderen: A(t) = max(0, (t − t_S2) − min(H, D_tut,max)). H = S2'de alınan en büyük `held_ms`. D_tut,max ≥ 30 s [CAL], aday 600 s.
- D_tut,max zarf girdisidir (§8.1). T_A = T_işlem + T_ctrl(k_r) + G_H.

**G_H türetimi** — 05f'nin kendi sınaması [türetim; ölçülmedi]:
- R2'nin başı t_R2, FIN kabulüdür; S2 ise t_S2 ≥ t_R2'de, FIN'in teyidiyle başlar.
- A(t) = (t − t_S2) − H_alınan(t) ≤ (t − t_R2) − H_alınan(t) =: **gecikme(t)**. Bu, tutma süresinin henüz gönderene ulaşmamış kısmıdır.
- Gecikmenin bileşenleri:
  1. **İlk girdinin ofseti:** girdi R2 ≥ T_min olunca doğar → ≤ T_min.
  2. **Kayıp girdiler:** HELD girdileri FLOW_CREDIT sınıfındadır (k_r). Ardışık en çok k_r girdi kaybolur; yinelemeler T_BLK aralıklıdır ve her biri T_min hız sınırıyla gecikebilir → ≤ (k_r + 1)·(T_BLK + T_min).
  3. **Kesinti:** kesintide kaybolanlar k_r'ye sayılmaz → + N_o·D_o.
  4. **Uçuş:** en son girdinin varışı → + L.
- **G_H = T_min + (k_r + 1)·(T_BLK + T_min) + N_o·D_o + L.**
  - Bu sınır, tutma sırasında A'nın zirvesini de, tutma bitince A'ya kalan sızıntıyı (t_end − t_son girdi) da kapsar.
  - Birikimli girdi sayesinde kayıp girdiler birikmez; tek bir başarılı girdi öncesindeki bütün tutmayı kapsar (YD-3'ün kuralı doğrulandı).
- **YD-3 taslağıyla fark:** YD-3 (`06:352`) G_H ≤ (k_r + 1)·T_BLK + N_o·D_o diyor. 05f'nin türetimi buna üç terim ekler: ilk girdi ofseti T_min, girdi başına hız sınırı T_min ve son girdinin uçuşu L.
- **Doğrulamanın 6. bulgusu kapanır:** "ilk taze girdiye dek bekleme" G_H'ın 1. ve 2. terimleridir; T_A(b)'de artık sıfır pay yoktur.
- **Tavan:** H > D_tut,max olursa A yeniden işler. D_tut,max'ı aşan tutma zarf dışıdır; vazgeçme mümkündür, > D_tut,max + G_H + T_ctrl'de kesindir.
- **Güvenli yön:** t_R2 ≤ t_S2 olduğu için t_R2'den sayılan H, A'yı en çok bir ACK yolu kadar fazla düşer. Bu yanlış vazgeçme üretmez, gerçek vazgeçmeyi geciktirir (YD-3 ile aynı sonuç).
- **Saat oranı:** iki taraf süre ölçer, mutlak zaman taşımaz. Monoton saatlerin oran farkı ppm mertebesidir ve ihmal edildi [çıkarım].

**[hesap]** 31 B/s (§8.2 girdileri; T_işlem = 1 s; T_min = 1 s):

| Hücre | T_ctrl(k_r) | (a) T_A, D_h = 30 s | G_H | (b) T_A |
|---|---|---|---|---|
| D-0 | 124,6 s | 155,6 s | 105,0 s | 230,6 s |
| D-1 | 966,0 s | 997,0 s | 165,0 s | 1 132,0 s |
| D-2 | 1 210,8 s | 1 241,8 s | 258,0 s | 1 469,8 s |
| D-3′ | 1 292,4 s | 1 323,4 s | 289,0 s | 1 582,4 s |
| D-3 | 1 863,6 s | 1 894,6 s | 506,0 s | 2 370,6 s |

**Bedeller — eşit dürüstlükle:**

| | (a) N_h·D_h | (b) HELD, birikimli, tavanlı |
|---|---|---|
| Her aktarımın T_A'sına eklenen | D_h (30 s) | G_H (105–506 s). Eş ölü olsa da sabit eşiğe girer, çünkü ilk girdiler kaybolmuş olabilir |
| Karşıladığı tutma | ≤ D_h | ≤ D_tut,max (aday 600 s) |
| Ölü eşin fark edilmesi | T_işlem + D_h + T_ctrl | T_işlem + G_H + T_ctrl. **(a)'dan geç**, çünkü G_H > 30 s; YD-3'ün "ölü eşte A uzamaz" ifadesi dinamik değer için doğru, sabit eşik için değil |
| Sınırı aşan tutma | vazgeçme mümkün; > D_h + T_ctrl'de kesin. Alıcı bütün veriye sahipken aktarım biter (yanlış "başarısız") | vazgeçme mümkün; > D_tut,max + G_H + T_ctrl'de kesin |
| Tel | değişmez | yeni girdi türü `HELD` (A.8 `06:433`) |
| Alıcı mekanizması | yok | R2 sırasında T_BLK aralıklı, **istenmemiş** FLOW_CREDIT emisyonu: yeni bir alıcı zamanlayıcısı (bugün FLOW_CREDIT tetiklidir, `06:95`) |
| Bant | yok | ≈ 39 B / T_BLK ≈ 1,3 B/s, 31 B/s'de, tutma süresince [hesap, `06:786`] |
| Yeni sayı | N_h, D_h [CAL] | D_tut,max [CAL]; G_H türetilmiştir |
| Spec | Ek A A.5'e tutma terimi | Ek A A.5 yeniden yazılır ("bildirilen tutma A'dan düşülür"); şu an Ek A bunu söylemiyor |
| C14 §6 "sonsuza kadar açık kalmasın" | sağlar | tavanlı biçim sağlar (YD-3); tavansız biçim sahip istisnası isterdi (DUR-05f-7) |

**Ölçüt (05f):** karşılanması gereken tutma süresi D ise (a)'nın bedeli D, (b)'ninki G_H'tir. Dolayısıyla:
- D ≤ G_H → (a) daha ucuz. INT-02'nin 30 s'si için bütün hücrelerde böyle.
- D > G_H → (b) daha ucuz (D = 600 s gibi).

Karar sahibindir. Hangi seçenek olursa olsun saat bu karardan önce kodlanmaz (C16 v2 §3).

## 8. Yerel vazgeçme politikası — iki sözleşme ve türetim

**8.1 İki sözleşme.**

**(i) Deterministik kabul zarfı E_D — KAPI** (DUR-05f-12 (a), DUR-05f-13). Girdiler:

| Girdi | Anlam | Kapı hücreleri |
|---|---|---|
| r_min | Hat hızı tabanı (S2: C15 EK-1 §2). `PRE_CONVERGE_FLOOR_BW` = 31 yalnız aynı sayıdır; kapsamı PRE_CONVERGE'dir (`1b1425d2:crates/adaptive-transport/src/sender.rs:1485`) | 31 B/s |
| d | Tek yön yol gecikmesi. L ve H_maks bunu içerir | 200 ms (05b fikstürü) |
| φ_min | Sunulan yük altında akışın hizmet payı tabanı (`06:168`; sayı DUR-06-3) | 1 (tek dosya) |
| N_o, D_o | Kesinti sayısı ve azami süresi, iki yön. Oturumsuz aralık da kesintidir (§9) | N_o ∈ {0, 1}, D_o = 60 s |
| **k_f** (A → B) | **Her sınıf için ayrı ayrı** ardışık kayıp tavanı: (1) dosya akışının birim iletimleri; (2) akış 0 / 1 kontrol birimleri (FLOW_RESET, FILE_ABORT); (3) FLOW_BLOCKED çerçeveleri. Hepsi A'nın istemci soketinden | {0, 3} |
| **k_r** (B → A) | **Her sınıf için ayrı ayrı:** (1) B sunucusunun SACK emisyonları; (2) aynı yoldaki FLOW_CREDIT çerçeveleri ve HELD girdileri; (3) B istemcisinden akış 1 FILE_RECEIPT iletimleri (A'nın sunucusuna; ayrı soket, ayrı yuva) | {0, 3, 4, 11} |
| — | A sunucusunun B istemcisine SACK'leri | tavan gerekmez: T_ctrl yalnız varışa bağlı |
| SACK kapsamı | u'nun varışından ilk başarılı SACK'e dek u'nun üstünde < 16 boşluklu PN bloğu (§2) | tek dosya hücrelerinde tutar [çıkarım] |
| Tutma | DUR-05f-14: (a) N_h × D_h, ya da (b) D_tut,max | N_h = 1, D_h = 30 s · D_tut,max ≥ 30 s |
| Kontrol şeridi | Akış 0 / 1 `Independent` (DUR-06-27 (c) = DUR-05f-17) | ön koşul |

- **Kayıp modeli:** kesinti dışındaki kayıplar yalnız bu sınıf başına ardışık tavanlarla sınırlıdır. Tavanlar sınıflar arasında bağımsızdır; en kötü hâlde hizalanır.
- **Enjektör:** her sınıfın akışına ayrı bir deterministik desen uygulanır. İstemci ve sunucu çıkışları ayrı kapılardır (`EgressPolicy`; sunucu çıkışı `runtime.rs` sunucu görevinde ayrıdır).
- **Sözleşme:** E_D içinde G, R ve A saatlerinin hiçbiri dolmaz → yerel vazgeçme = 0. Kapı Ek B.4 ve B.8 (i)'dir.

**(ii) Rastgele kayıp hücreleri — RAPOR** (B22 kayıplı hücreleri `06:507`; p = 0 olan B22 hücreleri deterministiktir ve B.8 (i) kapısına girer).
- Ürün aynı sabitlerle koşar. Gözlenen vazgeçme sayısı raporlanır; **deterministik garanti yazılmaz** (C16 v2 §3).
- Rapor şunları içerir:
  - ε = 10⁻³ [CAL];
  - koşulan aktarım sayısı N_akt;
  - ölçülen fırsat sayıları N_fırsat,s;
  - model değeri Σ_s N_fırsat,s · p_s^(k_s + 1).
- Model değeri, bağımsız Bernoulli kaybı varsayımıyla **E_D'den çıkma olasılığının üst sınırıdır** (birleşim sınırı). Vazgeçme tahmini değildir.
- Gözlenen vazgeçme oranı (vazgeçme / N_akt) bu üst sınır ve ε ile yan yana yazılır; hüküm yoktur.
- [hesap] D-3 tavanlarında p_r = %80 için 0,8¹² ≈ 0,069 / koşu: bu hücre E_D'nin dışındadır.

**8.2 Türetim** [türetim; koddan önce; ölçülmedi; girdiler §2'deki T1/T2 fikstür değerleri].

Bu bölüm bir türetim açığını kapatır; runtime'da yanlış timeout oluştuğunun kanıtı değildir (C16 v2 §3).

Girdiler (31 B/s): T_D = T_max(DATA) = 101,68 s · P_c = 135,56 s · A = 30 s · T_tx = A + P_c = 165,56 s · H = H_maks = 729,8 s · L = 72,98 s · T_ACK = 8,41 s · `idle_hold` = 105 s · T_BLK = 30 s · T_min = 1 s [CAL, `06:215`].

**Adım 1 — ileri zincir (koddan; §2 "kurtarma takvimi"):**
- x₁ ≤ T_D: yeni birimin ilk gönderimi, ya da G sıfırlandığında zaten uçuşta olan gönderim.
- x₂ ≤ x₁ + H + P_c: sahiplenme birimin **kendi son iletiminden** H sonra; kopya ≤ P_c sonra.
- xⱼ ≤ xⱼ₋₁ + T_tx, j ≥ 3.
- Varış n'inci iletimdedir. n = 1 + sayılan kayıplar + kesintinin yuttuğu iletimler.
- **F(n) = T_D + [n ≥ 2]·(H + P_c + (n − 2)·T_tx).**
- Kesinti, F'ye ayrıca + N_o·D_o ekler. Bu, sık iletim rejiminde kesinti bitiminden sonraki ilk iletimin ≤ T_tx gecikmesini karşılar. Türetim N_o ≤ 1 içindir.

**Adım 2 — yuva pencereleri:**
- Bir varıştan sonra yuvanın kesintisiz SACK fırsatı B(D) = ⌊(`idle_hold` − D)/T_ACK⌋'dır; D, pencereye düşen kesinti süresidir. K_slot(D) = B(D) − 1.
  - K_slot(0) = ⌊105/8,41⌋ − 1 = **11**.
  - K_slot(60 s) = ⌊45/8,41⌋ − 1 = **4** [hesap].
- Pencere sayısı: m = 1 eğer k_r + 1 ≤ B₁; aksi hâlde m = 1 + ⌈(k_r + 1 − B₁)/B(0)⌉. B₁ = B(D_o) (N_o = 1), yoksa B(0).
- K_slot = 0'da bu ifade incelemedeki (k_f + 1)(k_r + 1)'e iner.
- Oturum kapanınca yuva ≈ 1 s içinde ölür (§2). Bu G'de değil, §9 kuralıyla (Faz A) işlenir.

**Adım 3 — sınırlar:**
- **İletim sayısı:** n_tx = k_f + N_o + 1 + (m − 1)·(k_f + 1).
- **T_G** = T_D + [n_tx ≥ 2]·(H + P_c + (n_tx − 2)·T_tx) + 2·N_o·D_o + (min(k_r, K_slot(0)) + 1)·T_ACK + 2L.
  - Kesinti hem ileri zincire hem SACK penceresine yazılır; bu muhafazakârdır.
- **D_ack** (kabulden teyidin gönderene ulaşmasına) = N_o·D_o + [m ≥ 2]·(H + P_c + ((m − 1)(k_f + 1) − 1)·T_tx) + (min(k_r, K_slot(0)) + 1)·T_ACK + L.
  - Yuva ölürse birimin teyidi yeni bir ileri tur ister; bu terim bunu karşılar.
- **F_yeni** (yeni ya da kabul edilmemiş bir birimin alıcıda kabulü; SACK yolundan bağımsız) = T_D + [k_f + N_o ≥ 1]·(H + P_c + (k_f + N_o − 1)·T_tx) + N_o·D_o + L.
- **T_kredi** = N_o·D_o + (k_f + 1)(k_r + 1)·T_BLK + (k_r + 1)·T_min + 2L.
  - K4G-06'da FLOW_CREDIT BLOCKED'e tetikli yanıttır (`06:95`, `:201`); her yanıt eş başına ≤ 1 / T_min (`06:215`). Bu yüzden çarpım gerçektir.
  - Bir öneri: güncel sınır periyodik SACK'e bindirilirse terim toplanır hâle gelir (§12).
- **T_ctrl(k)** = T_C + [k + N_o ≥ 1]·(H_C + P_C + (k + N_o − 1)·(A + P_C)) + N_o·D_o + L.
  - Tek kontrol biriminin karşı uygulamaya varışı; bekleyen taraf yalnız varışı gözler, çarpım yoktur.
  - k, birimin sınıf tavanıdır: FILE_RECEIPT k_r, FILE_ABORT / FLOW_RESET k_f.
  - T_C = (S_iletimde + S_baş,C)/r + S_baş,C/(ρ_C·r) + τ (`06:170`); S_baş,C ≈ 50 B; ρ_C = 0,1 [CAL]; T_C = 51,6 s.
  - **P_C := T_C** [türetim: `06:170` sınırı her katman-0 başına uygulanır, çünkü bir kontrol biriminin kopyası katman-0 işidir (`06` §5)]. A + P_C = 81,6 s; H_C ≤ H.
  - **Ön koşul:** kontrol şeridi `Independent` (DUR-06-27 (c) = DUR-05f-17). `Ordered` seçilirse, öndeki kayıtların kurtarılması T_ctrl'ye eklenir ve bu sınır geçersizdir.
- **T_R** = max(D_ack + T_G + T_ctrl(k_f), T_kredi + F_yeni).
  - İlk terim R4 sırasıdır: gönderen önce vazgeçer ve bildirimi gelir.
  - İkinci terim, E_D içinde R sessizliğinin doğrudan sınırıdır: yeni bir kabul, gönderenin krediyi bilmesini ve bir ileri varışı ister. Gönderenin G sıfırlanmalarına bağlı değildir; yuva ölümüyle gecikmiş eski teyitler R'yi etkilemez.
- **T_A:** §7 (DUR-05f-14).

**[hesap]** 31 B/s, φ = 1, N_h = 1. Gevşek üst sınırlardır:

| Hücre | N_o×D_o | k_f / k_r | m | n_tx | T_G | D_ack | F_yeni | T_kredi | T_ctrl(k_r) | T_ctrl(k_f) | T_R |
|---|---|---|---|---|---|---|---|---|---|---|---|
| D-0 | 0 | 0 / 0 | 1 | 1 | 256,1 s | 81,4 s | 174,7 s | 177,0 s | 124,6 s | 124,6 s | 462,0 s |
| D-1 | 1 × 60 s | 0 / 0 | 1 | 2 | 1 241,4 s | 141,4 s | 1 100,0 s | 237,0 s | 966,0 s | 966,0 s | 2 348,8 s |
| D-2 | 1 × 60 s | 3 / 3 | 1 | 5 | 1 763,3 s | 166,6 s | 1 596,7 s | 690,0 s | 1 210,8 s | 1 210,8 s | 3 140,7 s |
| D-3′ | 1 × 60 s | 3 / 4 (= K_slot(60)) | 1 | 5 | 1 771,7 s | 175,0 s | 1 596,7 s | 811,0 s | 1 292,4 s | 1 210,8 s | 3 157,5 s |
| D-3 | 1 × 60 s | 3 / 11 | 2 | 9 | 2 492,8 s | 1 595,9 s | 1 596,7 s | 1 658,0 s | 1 863,6 s | 1 210,8 s | 5 299,6 s |

- **Doğrulamanın karşı örnekleri yeni sınırların altında** (wf_1699fc80):
  - D-1: 1 121,4 s < 1 241,4 s.
  - D-2: 1 477,8 s ve 1 643,3 s < 1 763,3 s.
  - D-3: 2 066–2 138 s < 2 492,8 s.
- **Ürün sabitleri:** E_D'nin bir hücresinden, ürün kipi sınırlarıyla (T1′/T2′) hesaplanır. Fikstür değerleriyle hesaplanmaz (DUR-05f-13).

**8.3 Vazgeçme eylemleri** — `06 §16 YD-4` ile hizalı. Varyant adları DUR-05f-9 (a) kararıdır.

| Dolum | DUR-06-28 = DUR-05f-18 (a) kabul edilirse (K4G-06 önerisi) | (a) reddedilirse (rev3 kuralı) |
|---|---|---|
| **G (S1, Faz G)** | `reset_flow(f, 0x04 timeout)`: motor akışın birimlerini iptal eder (`06:205`, `:221`); **FILE_ABORT yok**. Alıcı `FlowEnded { Reset { 0x04 } }` → R1 / R2'de `Aborted(timeout)`, R0'da olay yok. Yükümlülük düşer → R3 | `reset_flow(f)` + akış 1'e `FILE_ABORT(0x04 timeout)` |
| **A (S2, Faz A)** | akış 1'e `FILE_ABORT(0x04 timeout)` (aynen). FILE_BEGIN teslim edilmediyse alıcıda erken iptal kaydı (`06 §16 YD-5`) | aynı |
| **R (alıcı, R1)** | **`stop_flow(k)`** (yeni kredi yok; gelen birimler ACK'lenip atılır, `06 §16 YD-2`) + akış 1'e `FILE_RECEIPT(0x03 timeout)`. Gönderen S1'de `SendFailed(ReceiverTimeout)` + `reset_flow(k, 0x00)`; S2'de `SendFailed(ReceiverTimeout)` | akışı yerelde kapat (`late_after_close`) + `FILE_RECEIPT(0x03)` |
| Her biri | Tek `SendFailed(LocallyAbandoned)` ya da `Failed(LocallyAbandoned)`. Yarışan iki sinyalde ilk işlenen kazanır (YD-4) | aynı |

- **Sonrası:**
  - Geç gelen `ok` → `receipts_ignored` (`1b1425d2:file_transfer.rs:1152`).
  - `stop_flow` sonrası gönderen BLOCKED kalır ve G durur (`06 §16 YD-7`); FILE_RECEIPT gelince biter.
- **Ek A etkisi:** DUR-06-28 (a) seçilirse Ek A A.2 ve A.5'in FILE_ABORT cümleleri ("kalan birimleri iptal eder ve FILE_ABORT(timeout) gönderir") Faz A'ya daralır. AMP'deki kullanım metni K4G-06 A.18'dedir. Ek A o karardan sonra değişir.

## 9. Oturum geçişi

Ortak yaşam döngüsünün oturum satırları K4G-06'dadır (`06:211`; `06 §16 YD-4` son satır). 05f'nin kuralları:

| Durum | Gönderen | Alıcı |
|---|---|---|
| Faz G'de oturum değişti (FIN kabul edilmemiş) | yeni oturumdan `FILE_ABORT(superseded)`, `SendFailed(Superseded)` (`1b1425d2:file_transfer.rs:1509-1515`; `AMP:92`; YD-4) | `Aborted(superseded)`; FILE_BEGIN eski oturumun drain'indeyse erken iptal kaydı (YD-5) |
| FIN kabul edilmiş, `floor ≤ fin_seq` iken oturum bitti | Faz A o anda başlar (`1b1425d2:transport.rs:45`). Yuva ≈ 1 s içinde ölür ve SACK durumu kaybolur (`1b1425d2:runtime.rs:3428`, `:3154-3158`), dolayısıyla teyit bir daha gelmez | — |
| Alıcıda `Closed(h)`, akış eksik | — | Teslim artık gelmez (`1b1425d2:transport.rs:1069-1072`) → `Failed(SessionEnded)` + sıradaki oturumda `FILE_RECEIPT(0x03)` (yükümlülük oturumlar arası taşınır, YD-6) |
| Faz A'da oturum değişti | Etkisiz (`1b1425d2:file_transfer.rs:1135-1149`) | Alındı yuvadan yeni oturuma gider |
| Yeniden bağlanma | Oturumsuz aralık N_o·D_o'ya yazılır | — |
| Erken iptal kaydı (YD-5) | — | **Zamanlayıcısız düşer:** FILE_BEGIN(t) teslim edildiğinde (→ `FILE_RECEIPT(rejected)`), ya da eşin **kayıt anındaki bütün oturumları — kaydın kendi oturumu dahil —** `Closed` olunca |

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

## 12. K4G-06 ile hizalama (rev5 `a517d4a0`, `06 §16`)

K4G-06'nın önerileri sahip kararı değildir (DUR-06-26/27/28 açık). 05f bunlara koşullu bağlanır; ortak seçenekler DUR satırlarında iki numarayla birlikte yazılır.

| YD-9 maddesi | 05f rev3.1'de | Koşul |
|---|---|---|
| 1. T_A'dan D_tük çıkar; G_H ve D_tut,max zarfa girer | §7 (b), §8.1. 05f'nin G_H'ı YD-3 taslağından üç terim fazladır (T_min, girdi başına T_min, L) | DUR-05f-14 = DUR-06-26 (b) |
| 2. G dolumu = `reset_flow(f, 0x04)`, FILE_ABORT yok; A dolumu aynen | §8.3 | DUR-05f-18 = DUR-06-28 (a) |
| 3. R dolumu = `stop_flow` | §8.3, §6 | YD-2 / YD-4 (DUR-06-28 ile) |
| 4. Erken iptal kaydı kendi oturumu kapanınca da düşer | §9 son satır | YD-5 |
| 5. T_ctrl şerit sırası beklemesi içermez | §8.2 T_ctrl ön koşulu | DUR-05f-17 = DUR-06-27 (c) |
| 6. Çarpan ve ACK takvimi 05f'nindir | §2, §8.2 | — |

**K4G-06 rev5'e öneriler** (05f'nin türetiminden):
1. **YD-3'ün G_H taslağı eksik.** İlk girdi ofseti T_min, girdi başına T_min hız sınırı ve son girdinin uçuşu L eklenmeli (§7). YD-3'ün "(b) ölü eşte A'yı uzatmaz" cümlesi yalnız A'nın dinamik değeri için doğrudur. Sabit eşik T_A(b), G_H kadar büyür ve G_H bütün hücrelerde > 30 s'dir. §7 tablosu bu yüzden (a) ile (b)'yi eşit ölçütle karşılaştırır.
2. **Kredi yolu çarpımı.** FLOW_CREDIT yalnız BLOCKED'e tetikli yanıtsa (`06:95`, `:201`) T_kredi'de (k_f + 1)(k_r + 1)·T_BLK çarpımı vardır (D-3'te 1 440 s). Güncel sınırın periyodik SACK'e bindirilmesi bunu toplanır yapar. Bunu yalnız K4G-06 karar verebilir.
3. **Yuva emekliliği.** Periyodik SACK yalnız yuva ömrü boyunca sürer; kesinti ve oturum kapanışı pencereyi kısaltır (§2, §8.2 Adım 2). BLOCKED yuvayı canlı tutar (`06:202`), ama yalnız bloklu akışta (DUR-05f-15).
4. **B22 hüküm sütunu** (`06:507`). "Zarf içinde yerel vazgeçme = 0 (kapı)" yalnız deterministik hücrelere uygulanmalı. p = 0 hücreleri deterministik kapıya, kayıplı hücreler rapora (DUR-05f-12 (a)).
5. **Uyumlu, değişiklik yok:** H_C_tavan (`06:206-210`, YD-6; aktarım başına bir FILE_ABORT, gelen akış başına bir FILE_RECEIPT), `max_seen` + açık küme, tek sonlandırma çıktısı (`06:221`).

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

**B.4′ E_D dışı, çarpımsal rejim — RAPOR, kapı değil:** k_r = K_slot + 1 = 12; k_f = 3; 1 × 60 s.
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
