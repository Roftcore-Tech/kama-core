# K4G-05f — Dosya aktarımında ilerleme, bekleme ve yerel vazgeçme — tasarım notu rev2

## 0. Künye

- **Kart / dal / durum:** K4G-05f · `k4g/05f-dosya-ttl` (taban main `1b1425d2`; rev1 `e5514e7a` üstüne) · **rev2 TASLAK.** Kod yok, ölçüm yok. Tasarım GO ayrı bir mesajla gelir (C15 v3 §6).
- **Dayanak kararlar:** CEVAP 15 EK-1 > CEVAP 15 v3 §6 (05f), §5 (DUR-05d-8), §9 (K4G-06) > CEVAP 14 §6.
- **rev1 durumu:** ONAYLANMADI. rev1'in "taban 60 s artmadıysa alıcı kabul etmemiştir" çıkarımı yanlıştır: ACK kaybı ya da tek bir eksik eski birim tabanı dondurur. rev1 metni Ek C'de, **MUST NOT IMPLEMENT**.
- **Uyum:** K4G-06 rev3 (`dfd3a5e5`, origin) akış modeline göre yazıldı (akışlar §3, kredi §7, H_C_tavan §7.6, DUR-06-14). Çelişkiler §12'de.
- **Atıf biçimi:** `SHA:dosya:satır`.
  - Kısa adlar:
    - `file_transfer.rs`, `codec/mod.rs` → `crates/kama-app/src/`
    - `runtime.rs`, `transport.rs` → `crates/client-core/src/`
    - `receiver.rs` → `crates/adaptive-transport/src/`
    - `AMP` → `specs/application-message-protocol.md`
    - `06` → `docs/design/K4G-06-AKIS-IZOLASYONU.md`
    - `05b` → `docs/design/K4G-05b-RETX-FIX.md`
    - `05d` → `docs/design/K4G-05d-RECEIPT.md`
  - SHA'lar (hepsi origin'de): `1b1425d2` main · `0c22a07d` 05c-kod · `37b78aad` 05b rev6 · `cf270a6c` 05d · `dfd3a5e5` K4G-06 rev3 · `e5514e7a` 05f rev1.
- **Etiketler:** [olgu] koddan · [hesap] aritmetik, ölçülmedi · [türetim] koddan önce yazılmış sınır · [çıkarım] test edilmedi · [CAL] deney başlangıç değeri.
- **Ek A sha256:** `a6c546ae5d788dad4dee9a974e1796c1d40b1326aa70057bfc0f09afc46c2ea1`. Kapsam: `## Ek A` satırından `## Ek B`'den önceki son boş olmayan satıra dek, 38 satır, her satır `\n` ile (`~/kama-k4g-work/tools/eka-sha.sh`).

## 1. Karar özeti

1. **İlerleme sinyali = akışın yeni benzersiz teyidi.** Dosya akışının daha önce teyit edilmemiş bir biriminin ilk teyidi sayılır. Bayat ACK/SACK sayılmaz. Taban (`confirmed_floor`) yalnız faz sınırıdır: "FIN dahil önek teyitli". Alıcının hareketsiz olduğu tabandan çıkarılmaz (C15 v3 §6, DUR-05f-1 (a)).
2. **Teyit ya da alındı yokluğu kanıt değildir.** Süre dolumu yalnız **yerel vazgeçme politikası** adıyla terminal olur. Süre yazılı bir zarftan (§8) türer. Vazgeçme karşı tarafa açık bir mesajla bildirilir: gönderen `FILE_ABORT(0x04 timeout)` ve `reset_flow` kullanır; alıcı `FILE_RECEIPT(0x03 timeout)` kullanır. Tel baytı değişmez.
3. **Üç bekleme tek tasarımda ve akış başına:**
   - gönderen ilerlemesi (G saati);
   - alıcı dosya-idle (R saati; bugünkü `FILE_IDLE_TTL = 60 s`'nin yerine geçer);
   - alındı bekleme (A saati; bugünkü `FILE_RECEIPT_TTL = 60 s`'nin yerine geçer).
4. **Kapasite beklemesi zaman aşımına sayılmaz.** Kuyruk gecikmesi türetilmiş üst sınırıyla sayılır. Ağ terimi zarftan gelir (§4, 05d A1 ve K4G-06 ile ortak).
5. **31 B/s ve kayıp altında dosya kapsamdadır (S2).** DUR-05f-3 ve DUR-05f-4 gereksinimdir, beyanlı sınır değildir. 31 B/s'de iki tam birimin arasındaki 67,7 s, R saatinde ilerleme olarak görülür. FILE_RECEIPT akış 1'de gider, toplu verinin arkasında kalmaz.
6. **Sayılar zarftan türer.** 31 B/s'nin dört köşe hücresi için [hesap] tablosu §8'de. Yeni sayılar yalnız ε, D_tük ve T_işlem [CAL] (DUR-05f-8). `SendFailure` ve `FileFailure` yüzeyi kural onaylanınca belirlenir (DUR-05f-9).

## 2. Olgular

**Bugünkü saatler** [olgu, `1b1425d2`]:
- **Gönderen:** `FILE_END` yerel kuyruğa kabul edilince `AwaitingReceipt { since }` (`file_transfer.rs:1538-1542`). 60 s içinde alındı yoksa `ReceiptTimeout` (`:267`, `:1189-1209`).
- **Alıcı:** son kabul edilen YENİ parçadan sonra 60 s geçerse `IdleTimeout` ve `FILE_RECEIPT(timeout)` (`:262`, `:1054`, `:1165-1185`).
- **Alındı kuyruğu:** küresel 4 kayıtlık; taşınca en eskisi düşer, 60 s'yi aşan da düşer (`:291`, `:1221-1223`, `:1243-1244`).
- **Erken iptal kaydı:** ömrü `FILE_IDLE_TTL`'dir (`:150-152`, `:1295`, `:1317`).
- **Normatif metin:** `AMP:89-92`.

**Bugünkü belirsizlikler** [olgu]:
- `send` → `Ok` yalnız "kuyruğa alındı" demektir (`1b1425d2:crates/kama-contracts/src/s1.rs:185`).
- Runtime'ın `acked()` bildirimi kanal dolunca düşebilir (`1b1425d2:runtime.rs:2136`, `:2145`).
- ACK seçicidir, SACK kullanır (`1b1425d2:crates/adaptive-transport/src/ack.rs:1-6`).
- Kopya varışı ACK'i zorlar (`1b1425d2:receiver.rs:222-230`).
- Motor birimi terk etmez (`1b1425d2:transport.rs:26-27`).

**05c sonrası** [olgu, `0c22a07d`]:
- Teyit yalnız `take_confirmed_units` çıktısıdır ve birim oturumda en çok bir kez çıkar (`runtime.rs:2138-2156`). Bu, "yeni benzersiz teyit"in kaynağıdır.
- Store bir `BTreeMap` (`:2024`).
- Kabul edilmiş birimin tekrarı ACK'lenir (`:3396-3398`).
- Offer kanalından store'a alım koşullu (`:2608`).

**Kullanılmayan kod noktası** [olgu]: `FILE_ABORT 0x04 timeout` kayıtlı ama bugün hiçbir yerde üretilmiyor (`1b1425d2:codec/mod.rs:55-56`; gönderen eşlemesi `file_transfer.rs:1583-1585`; alıcı her nedeni `Aborted` olarak uygular, `:1114-1132`).

**K4G-06** [`dfd3a5e5`, tasarım]:
- Dosya aktarım başına bir dinamik BK akışıdır; FILE_RECEIPT ve FILE_ABORT akış 1'dedir (`06:52-53`, `:81`).
- Kontrol başı katman-2 rezervini beklemez (`06:113`).
- BLOCKED aralığı en çok min(RTO_MAX, `idle_hold`/2) (`06:145`).
- `reset_flow` → FLOW_RESET (`06:148`).
- H_C_tavan (`06:149-153`); iptal tek çıktıdır (`06:164`).
- Oturum değişiminde akış taşınmaz (`06:154`).

**05b rev6 sınırları** [türetim, `37b78aad:05b`]: A = RTO_MAX = 30 s (`:556`) · H_maks (`:561`) · T_birim (`:571`) · tablo (`:578-581`) · L (`:585`).

## 3. İlkeler

- **P1 — kanıt yoksa çıkarım yok.** Yeni teyit, yeni kabul ya da alındı gelmemesi, karşı tarafın almadığını ya da vazgeçtiğini göstermez. Hiçbir kural bundan karşı taraf hakkında bir durum türetmez.
- **P2 — terminal durumlar açık sinyalle oluşur.** Açık sinyaller:
  - alındı (`FILE_RECEIPT`, beş statü);
  - `FILE_ABORT` / `FLOW_RESET`;
  - oturum sonu kuralı (§9);
  - kullanıcı iptali;
  - yerel vazgeçme (P3).
- **P3 — yerel vazgeçme bir politikadır, çıkarım değildir.** Yalnız yerel bir saatin zarftan türetilmiş sınırı aşmasıyla tetiklenir. Adı ve nedeni "yerel vazgeçme"dir. Karşıya açık mesajla bildirilir. Zarf içinde tetiklenmez; bunun testi Ek B.4'te.
- **P4 — akış başına.** Her saat (oturum, akış) başınadır. Oturum geneli taban ve oturum geneli "bekleyen = 0" kullanılmaz (C15 v3 §6, §9.5).
- **P5 — vazgeçme yeni kapasite istemez.** `FILE_ABORT` ve `FILE_RECEIPT` için yer H_C_tavan içinde önceden ayrılmıştır (`06:149-153`). Aktarım başına en çok bir FILE_ABORT gider (ilk neden kalır, `1b1425d2:file_transfer.rs:134`); gelen akış başına en çok bir FILE_RECEIPT gider.

## 4. Kapasite · kuyruk gecikmesi · zaman aşımı (05d A1 · K4G-06 · 05f ortak bölüm)

**4.1 Üç büyüklük, üç sahip.**

| Büyüklük | Soru | Sahibi / mekanizma | Sağladığı | Sağlamadığı |
|---|---|---|---|---|
| **Kapasite** | Bu birim şimdi kabul edilebilir mi? | Alıcı kredisi (`06` §7); yerel akış bütçesi = 05d A1 → K4G-06 akış bütçesi erişimcisi (DUR-06-14 (a), `06:556`); rezerv `A − 1 ≥ H_R + H_C_tavan` (`cf270a6c:05d` §10 (ii)–(iii); C15 v3 §5 (ii)–(iii)) | Kabul edilebilirlik; kontrol kaydı için yer | Bekleme süresi (`cf270a6c:05d:198`) |
| **Kuyruk gecikmesi** | Kabul edilmiş birim ne zaman hizmet alır? | İki düzeyli seçici: P_f, T_max (`06` §5, `:113`); düzey 2 = 05b rev6 K-0…K-5 | Türetilmiş üst sınır ("koşullu garanti", 05b K2) | Ağ kaybından bağımsız tamamlanma |
| **Zaman aşımı** | Ne zaman vazgeçilir? | Yalnız yerel vazgeçme politikası (§8) | Sonlu terminal durum | Karşı taraf hakkında bilgi (P1) |

**4.2 Kurallar.**
- **R1 — kapasite beklemesi saate girmez.** Kendi tarafında gözlenebilir: `Backpressure`, BLOCKED ya da yerel tutma. Süresi, bekleten tarafın saatinde sınırlanır: gönderen kredi bloklu ve teyitsiz birimi yokken G saati durur; alıcı tüketici tutarken R saati durur.
- **R2 — kuyruk gecikmesi türetilmiş sınırıyla girer.** T_max ve P_f kullanılır; ölçülmüş hız değil. W_F₀ = 240 yalnız deney adayıdır ve bir sınır vermez (C15 v3 §5 (i)). Bu yüzden 05f hiçbir saatte W_F'yi sınır olarak kullanmaz.
- **R3 — ağ terimi yalnız zarftan gelir** (§8): ortalama kayıp + RTO_MAX + taban hız yetmez (C15 v3 §6 (2)).
- **R4 — her bekleme tek saatte, saatler sıralı.** T_R = T_G + T_ctrl: zarf dışında gönderen önce vazgeçer, mesajı alıcıya T_ctrl içinde ulaşır. T_A ≥ T_işlem + D_tük + T_ctrl.
- **R5 — 120 s (S1) saat değildir.** FILE_RECEIPT için geçici deney hedefidir (C15 EK-1 §2 S1). Ölçüm tanımı C15 v3 §5 (i): başlangıç = yükümlülüğün doğumu (aktarımın kabulü ya da ilk sunum), bitiş = RECEIPT'in ilk emisyonu. ACK ve karşı uygulamaya teslim ayrı sütunlar; kesintili ve kesintisiz kol ayrı. TEXT ve PTT'ye uygulanmaz. T_A ile karıştırılmaz: T_A vazgeçme sınırıdır, 120 s ölçüm hedefidir.

**4.3 Bir aktarımın beklemeleri.**

| Bekleme | Kapasite mi? | Saat | Sınır terimi |
|---|---|---|---|
| S1 `Backpressure` / akış bütçesi (gönderen) | evet | G durur | — (R1) |
| Kredi bloklu, teyitsiz birim yok | evet | G durur | — (bekleten alıcının R saati) |
| Akış hizmet sırası (DRR, katmanlar) | hayır | G çalışır | T_max(DATA), P_f |
| Kayıp birimin kurtarılması (ARQ/kopya) | hayır | G çalışır | H_maks, A + P_c, k_f |
| ACK kaybı | hayır | G çalışır | k_r · max(A + P_c, T_BLK) |
| Alıcı tüketicisi tutuyor | evet | R durur | D_tük (yalnız A'da) |
| Alıcı işlemesi (özet, tur) | hayır | A çalışır | T_işlem |
| FILE_RECEIPT'in ters yönde gidişi | hayır | A çalışır | T_ctrl (akış 1; `06:113`) |
| Oturumsuz aralık | hayır | A çalışır | zarfın kesinti bütçesi N_o·D_o |

**4.4 05d A1 ile uyum.**
- 05f'nin erişimci alanları A1 erişimcisinin, ya da DUR-06-14 (a) kabul edilirse akış bütçesi erişimcisinin, parçasıdır (DUR-05f-6).
- `FILE_RECEIPT_TTL` ile alındı düşürme kalkar (A.21, `06:264`). 05d §10 (iii)'ün "süreli kayıt" satırı (`cf270a6c:05d:183`) bu yüzden geçersizleşir.
- H_C_tavan değişmez (P5).

## 5. Gönderen ilerlemesi (G saati) ve faz sınırı

**Erişimci (DUR-05f-6, gönderen tarafı)** — `FlowTxProgress(oturum, akış)`:
- `accepted`: sıradaki `flow_seq`.
- `confirmed`: `take_confirmed_units` / `take_finished_units` Teyit çıktılarından akışa düşen **farklı** birim sayısı. Tekdüze artar. İptal çıktısı sayılmaz (`06:164`). FEC RECOVERED teyidi sayılır.
- `floor`: akışın en küçük teyitsiz `flow_seq`'i. **Henüz motora verilmemiş birimler dahildir:** offer kanalı ya da akış başına yerel tampon (`06` §6 tablosu). Ancak hiçbiri yoksa `floor = accepted`.
- `blocked`: akışın BLOCKED bölümü etkin (`06:145`).

**Tanımlar:**
- `fin_seq`: FIN birimi `send` → `Ok` döndükten hemen sonra okunan `accepted − 1`. kama-app düğüm döngüsü tek iş parçacıklıdır (`1b1425d2:crates/kama-node/src/node.rs:1`).
- **Faz G (gönderim):** `FILE_BEGIN` kabulünden, `floor > fin_seq` ilk kez görülene dek.
- **Faz A (alındı):** `floor > fin_seq` ilk görüldüğünde ya da akışın oturumu bittiğinde (§9) başlar; hangisi önce olursa.

**G saati:**
- **Çalışır:** akışta motora verilmiş teyitsiz birim varken, ya da kredi varken ve sunulmayı bekleyen kayıt varken.
- **Durur:** `blocked` ve motora verilmiş teyitsiz birim yok (R1).
- **Sıfırlanır:** yalnız `confirmed` artınca. Bayat ACK/SACK, kopya varışının ACK'i ve yinelenen RECOVERED `confirmed`'i artırmaz, saati sıfırlamaz.
- **Dolum:** `T_G(E)` (§8) → yerel vazgeçme (§8.3).

**Tuzak:** store boşken FIN kaydı offer kanalında ya da yerel tamponda durabilir. Bu anda `floor` = o kaydın `flow_seq`'i olmalıdır; `accepted` olamaz. Aksi hâlde taban `fin_seq`'i aşar ve Faz A teslimden önce başlar. Zorunlu test: Ek B.1.

## 6. Alıcı dosya-idle (R saati) — `FILE_IDLE_TTL`'in yerine

**Erişimci (DUR-05f-6, alıcı tarafı)** — `FlowRxProgress(oturum, akış)`:
- `received`: akışta ilk kez kabul edilen **farklı** `flow_seq` sayısı. Sırasız tutulanlar dahil (`06` §9).
- `delivered`: uygulamaya verilen sıradaki `flow_seq`.
- `granted`: ilan edilmiş `max_seq` (`06:144`).
- `held`: `hold_flow` etkin.

**R saati:**
- **Çalışır:** verilmiş ama doldurulmamış kredi varken, yani `granted`'ın altında henüz kabul edilmemiş `flow_seq` varken.
- **Durur:** `held` ya da verilmiş kredinin tamamı kabul edilmiş (R1).
- **Sıfırlanır:** `received` artınca. Uygulamaya teslim beklenmez; böylece tek eksik eski birim, arkasındaki kabulleri gizlemez.
- **Dolum:** `T_R(E) = T_G(E) + T_ctrl(E)` → yerel vazgeçme (§8.3).

**Sonuç:** 31 B/s'de iki tam birimin arasındaki 67,7 s [hesap] bir ilerleme aralığıdır; yalnız `T_R(E)` aşılırsa vazgeçilir.

## 7. Alındı bekleme (A saati) — `FILE_RECEIPT_TTL`'in yerine

- **Yer:** FILE_RECEIPT akış 1'de gider (`06:81`). Gelen akış başına bir yuvası vardır, düşürülmez (A.21, `06:264`). Kontrol başı toplu verinin arkasında beklemez; uygun baş ≤ (S_iletimde + S_baş)/r + kova dolumu + τ içinde çıkar (`06:113`). Bugünkü ters FIFO (W_F'ye dek toplu birim; `cf270a6c:05d` §10 (i)) bu yolla kalkar.
- **A saati:** Faz A'nın başında başlar. Sıfırlanmaz ve durmaz; oturumsuz aralık zarfın kesinti bütçesinden sayılır.
- **Dolum:** `T_A(E) = T_işlem + D_tük + T_ctrl(E)` → yerel vazgeçme (§8.3).
- **Alındı gelince:** Delivered ya da `SendFailed(<statü>)` (bugünkü eşleme: `1b1425d2:file_transfer.rs:351-360`, `:1598-1625`).

## 8. Yerel vazgeçme politikası ve zarf

**8.1 Zarf E** [türetim; ilk sürüm = K4G-06 B22 hücreleri, `06:323`]:

| Parametre | Anlam | B22 değerleri |
|---|---|---|
| r_min | Pacer hızı; akış payı r_f = φ·r_min, φ K4G-06 §5'ten | 31 B/s (`1b1425d2:crates/adaptive-transport/src/sender.rs:1485`); tek dosya → φ ≈ 1 |
| sunulan yük | Rakip sınıfların yükü (φ'yi belirler) | yok (tek dosya) |
| N_o, D_o | Kesinti sayısı ve azami süresi, iki yön; **oturumsuz aralık da kesinti sayılır** | {0}, {1 × 60 s} |
| p_f → k_f | İleri yolda ardışık kayıp tavanı | p_f ∈ {0, %10} |
| p_r → k_r | Dönüş yolunda ardışık kayıp tavanı (ACK, kredi, FILE_RECEIPT) | p_r ∈ {0, %80} |
| ε | Aktarım başına yanlış vazgeçme olasılığı tavanı | 10⁻³ [CAL] |
| D_tük | Alıcı tüketicisinin tutma payı (yalnız Faz A) | 0 [CAL] (ürün tüketicisi = düğüm turu) |
| T_işlem | Alıcının son kayıttan alındıya kadar işlemesi | ≤ 1 s [CAL] |

Kayıp dizisi sınırı rastgele kaybı sonlu hâle getirir. k = ⌈ln(ε/N) / ln p⌉ − 1, p = 0 ise k = 0; N = aktarımın birim sayısı. Bu yaklaşık bir değerdir: fırsat sayısı ≈ N alınır, ε birim fırsatlarına bölünür.

**8.2 Sınırlar** [türetim]:
- Girdiler 05b rev6 sınıf sınırlarıdır: R = r_f'de T_max(DATA), P_c = T_max(kopya), A, H_maks, L (`37b78aad:05b:556-585`).
- "Herhangi bir" teyitsiz birimin yeniden hizmeti ≤ A + P_c içinde olur: kayıt ≤ A içinde vadeli, sınıf ≤ P_c içinde hizmet verir (`:571`). Birim başına T_birim gerekmez, çünkü ilerleme herhangi bir birimin yeni teyidiyle oluşur. T_BLK = min(RTO_MAX, `idle_hold`/2) (`06:145`).
- **T_G(E)** iki durumda:
  - kayıpsız ve kesintisiz: L + T_max(DATA);
  - diğer: N_o·D_o + L + H_maks + (k_f + k_r + 1) · max(A + P_c, T_BLK).
  - Gerekçe: H_maks kesinti ya da kayıptan sonraki ilk sahiplenmedir. Her hizmet bir ileri denemedir (en çok k_f ardışık kayıp). Her varış ACK ister, kopya varışı ACK'i zorlar (`1b1425d2:receiver.rs:222-230`); dönüşte en çok k_r ardışık kayıp. İlk başarılı ACK, varan teyitsiz birimi yeni teyit olarak getirir.
- **T_ctrl(E):** tek bir kontrol biriminin (FILE_ABORT ya da FILE_RECEIPT) karşıya ulaşması.
  - kayıpsız: T_C + L;
  - diğer: T_C + L + N_o·D_o + H_maks + k·(A + P_C).
  - k, birimin gittiği yönün kayıp tavanıdır: FILE_RECEIPT için k_r, FILE_ABORT için k_f.
  - T_C = (S_iletimde + S_baş,C)/r + S_baş,C/(ρ_C·r) (`06:113`). S_baş,C ≈ 50 B (26 B mühür + 10 B FLOW_BATCH + 14 B kayıt) [hesap]. ρ_C = 0,1 [CAL, `06` §4]. P_C ≈ T_C.
- **T_A(E) = T_işlem + D_tük + T_ctrl(E)** · **T_R(E) = T_G(E) + T_ctrl(E).**

**[hesap]** 31 B/s, r_f = r. Kullanılan değerler: T_max(DATA) = 101,68 s; P_c = 135,56 s; A + P_c = 165,56 s; H_maks = 729,8 s; L = 72,98 s; T_C = 51,6 s; A + P_C = 81,6 s. Gevşek üst sınırlardır, ölçülmedi. K4G-06 DUR-06-3 sayılarıyla yeniden hesaplanır.

| Hücre (p_f / p_r / kesinti) | N | k_f / k_r | T_G | T_ctrl (alındı) | T_A | T_R |
|---|---|---|---|---|---|---|
| 0 / 0 / yok | — | 0 / 0 | 174,7 s | 124,6 s | 125,6 s | 299,2 s |
| 0 / 0 / 1 × 60 s | — | 0 / 0 | 1 028,3 s | 914,4 s | 915,4 s | 1 942,7 s |
| %10 / %80 / 1 × 60 s, 150 KiB | 158 | 5 / 53 | 10 630,8 s (2,95 sa) | 5 239,2 s | 5 240,2 s | 15 870,0 s (4,41 sa) |
| %10 / %80 / 1 × 60 s, 10 KiB (B22) | 13 | 4 / 42 | 8 644,1 s (2,40 sa) | 4 341,6 s | 4 342,6 s | 12 985,7 s |

Kayıpsız hücrede 31 B/s'de birimler arası ≈ 33,9 s'dir, alıcının R saati de bu kadar aralıkla sıfırlanır [hesap]. Yerel vazgeçmenin sınırı (174,7 s / 299,2 s) bu aralığın üstündedir.

**8.3 Vazgeçme eylemleri** (tek seferlik, tekilleştirilmiş):
- **G dolumu (gönderen):**
  1. `reset_flow(f)` → motor akışın birimlerini iptal eder; yeniden gönderim durur (`06:148`, `:164`).
  2. Akış 1'e `FILE_ABORT(0x04 timeout)`; H_C_tavan içinde, P5.
  3. Tek bir `SendFailed(<yerel vazgeçme>)` (DUR-05f-9).
- **A dolumu (gönderen):** akış 1'e `FILE_ABORT(0x04 timeout)` + `SendFailed(<yerel vazgeçme>)`. Sonradan gelen alındı yok sayılır (`receipts_ignored`, `1b1425d2:file_transfer.rs:1152`).
- **R dolumu (alıcı):**
  1. Akış yerelde kapanır: sonraki birimler ACK'lenip atılır (`late_after_close`, `06:53`); bu yüzden gönderen yeniden göndermeyi bırakır.
  2. Akış 1'e `FILE_RECEIPT(0x03 timeout)`.
  3. Tek bir `Failed(<yerel vazgeçme>)`.
- **Karşı tarafın gördüğü:**
  - Gönderen vazgeçtiyse alıcı `Aborted { reason: Timeout }` görür; bu mevcut olaydır (`1b1425d2:file_transfer.rs:1114-1132`).
  - Alıcı vazgeçtiyse gönderen `SendFailed(ReceiverTimeout)` görür (`:351-360`).
- **Zarf dışında:** yanlış "başarısız" mümkündür; yanlış "teslim edildi" mümkün değildir (DUR-05f-11).

## 9. Oturum geçişi (akışlar oturuma aittir; `06:154`, `06` §3 "Oturum sınırı")

| Durum | Gönderen | Alıcı |
|---|---|---|
| Faz G'de oturum değişti (FIN kabul edilmemiş) | Mevcut kural: yeni oturumdan `FILE_ABORT(superseded)`, `SendFailed(Superseded)` (`1b1425d2:file_transfer.rs:1509-1515`; `AMP:92`); G saati biter | `Aborted(Superseded)`; eski oturumun `Closed`'u önce okunduysa satır 3 |
| FIN kabul edilmiş, `floor ≤ fin_seq` iken akışın oturumu bitti (supersede, kapanış, 05g ardılsız `Closed`) | Faz A o anda başlar. Eski oturum gönderemez ve birim taşınmaz (`1b1425d2:transport.rs:45`). A saati alındıyı bekler | — |
| Alıcıda `Closed(h)` okundu, h'nin akışı tamamlanmamış | — | Kapanan oturumun teslimi artık gelmez (`1b1425d2:transport.rs:1069-1072`; `06:154`) → `Failed(<oturum bitti>)` (DUR-05f-9) + eşin sıradaki oturumunda akış 1'den `FILE_RECEIPT(0x03 timeout)` (yuvası ayrılmış) |
| Faz A'da oturum değişti | Etkisiz; alındı eş anahtarlıdır (`1b1425d2:file_transfer.rs:1135-1149`) | Alındı yuvadan yeni oturuma gider |
| Yeniden bağlanma | Eski akış taşınmaz; oturumsuz aralık kesinti bütçesine yazılır | Erken iptal kaydı **zamanlayıcısız** düşer: kaydın oturumundan eski bütün oturumlar `Closed` olduğunda (bugün ömür `FILE_IDLE_TTL`, `:150-152`) |

Erken iptal kaydının neden zamanlayıcısız düşebildiği: `Closed`'dan sonra o oturumdan teslim gelmez, dolayısıyla geç `FILE_BEGIN` de gelemez. Bu kural ayrıca DUR gerektirmez.

## 10. Bağımlılık ve sıra

- 05f kodu yalnız K4G-06 ürün kodunun içinde gelir: `06` §12 (ii) client-core akış erişimcisi, (iv) kama-app. Arada bir oturum-geneli sürüm yazılmaz (C15 v3 §6, "geçiş maliyeti ödenmesin").
- Sıra: 05c merge → K4G-06 deney dalı → K4G-06 rev4 + CEVAP 16 → ürün kodu (ii) → (iv) 05f + 05d A1'in akış bütçesine devri (DUR-06-14) → INT-02 kolu.
- Önkoşullar:
  - A.21: alındı yuvası, düşürme yok;
  - `reset_flow` / iptal (`06:164`);
  - akış 1 kontrol şeridi;
  - akış başına sıra.

## 11. Süre (iş günü, tahmin; K4G-06 ürün kodunun içinde)

- client-core `FlowTxProgress` / `FlowRxProgress` alanları + B.6: 0,5.
- kama-app üç saat, faz sınırı, vazgeçme eylemleri, oturum kuralları, erken iptal kaydı: 1–1,5.
- Ek A spec yamaları + DUR-8 biçiminde mevcut test değişiklikleri (Ek B.7): 0,5–1.
- Yeni testler B.1–B.5: 1.
- kama-node / kama-mobile yüzeyi (DUR-05f-9 sonrası): 0,25–0,5.
- **Toplam ≈ 3,25–4,5.** INT-02 kolu (B.8) ayrıdır: ≈ 88 dk'lık bir koşu + B22 hücreleri.

## 12. K4G-06 ile çelişkiler (rev4'e)

1. **`06:177`** "dosya ilerlemesi = dosya akışının teyit tabanı" ↔ C15 v3 §6 (1) "ilerleme = yeni benzersiz teyit". Taban yalnız faz sınırıdır. → K4G-06 §10 metni güncellenir.
2. **`06:266` A.23** "(yeni DUR yok)" ↔ 05f rev2 yeni sayı (ε, D_tük, T_işlem), sözleşme (erişimci, terminal yüzey) ve spec yaması getirir. → DUR-05f-6…-12.
3. **`06:323` B22** dosya 10 KiB ↔ C14 §6 / §7 (1): 31 B/s'de 150 KiB ≈ 83 dk. → DUR-05f-12.
4. **`06:147` FIN** "tutulan akışın kapanışı uygulama tüketene dek bekler (beyan)" ↔ D_tük = 0. FIN kredi içindeyken alıcı tüketicisi tutarsa gönderen T_A ile vazgeçer. `06:300` B1 gönderim fazındadır (kredi bloklu → G durur), dolayısıyla B1 çelişmez. "FIN sonrası tutma" kolu K4G-06 Ek B'ye ya da D_tük > 0'a (DUR-05f-8) bağlanır.
5. **Uyumlu (değişiklik yok):** H_C_tavan (`06:149-153`; P5) · erken iptal kaydı ↔ `max_seen` + açık küme (`06:53`; §9) · `reset_flow` / FLOW_RESET (`06:148`).

## 13. DUR listesi (yalnız sözleşme, yeni sayı, beyanlı sınır, test amacı)

| DUR | Tür | Soru · seçenekler | Öneri |
|---|---|---|---|
| **DUR-05f-6** | sözleşme | DUR-05f-1 (a)'nın genişlemesi. `FlowTxProgress { accepted, floor, confirmed, blocked }` + `FlowRxProgress { received, delivered, granted, held }`; (oturum, akış) başına. Yer: (a) K4G-06 akış bütçesi erişimcisi — client-core somut tip + kama-app trait, S1 dışı (DUR-06-14 (a)) · (b) S1 v2'ye (DUR-06-7) | (a) |
| **DUR-05f-7** | sözleşme | (a) Yerel vazgeçme politikası: G/R/A saatleri, §8.2 sınırları, §8.3 eylemleri — tel baytı aynı · (b) otomatik vazgeçme yok: yalnız açık sonlar + kullanıcı iptali; tutulu aktarım yalnız kendi yuvasını tutar (`06:324` B23) | (a) — C14 §6 "sonsuza kadar açık kalmasın" |
| **DUR-05f-8** | yeni sayı | Zarfın üç [CAL] değeri: ε = 10⁻³ (aktarım başına) · D_tük = 0 · T_işlem ≤ 1 s. k_f/k_r bunlardan ve p'den türer (§8.1). Zarfın ilk sürümü B22 hücreleri | bu değerler; son değer INT-02'den sonra |
| **DUR-05f-9** | sözleşme | Terminal yüzey: (a) `SendFailure::LocallyAbandoned`, `FileFailure::LocallyAbandoned`, `FileFailure::SessionEnded` (kama-app pub enum; kama-node `FileFailReason`, `1b1425d2:crates/kama-node/src/event.rs:168-186`; UniFFI aynası) · (b) mevcut varyantların yeniden kullanımı (`ReceiptTimeout`, `IdleTimeout`; anlam karışır). Kural onaylanınca (C15 v3 §6 (3)) | (a) |
| **DUR-05f-10** | sözleşme (spec) | AMP §3 metinleri (Ek A A.1–A.6): `FILE_IDLE_TTL` ve `FILE_RECEIPT_TTL` üstü çizilir; `0x04 timeout` = gönderenin yerel vazgeçmesi; `0x03 timeout` alındısının anlamı genişler; tel baytı ve vektörler değişmez | Ek A |
| **DUR-05f-11** | beyanlı sınır | Zarf dışında yanlış vazgeçme mümkün: yanlış "başarısız", yanlış "teslim edildi" değil; vazgeçmeden sonra gelen `ok` yok sayılır. D_tük = 0 ise alıcı sürecinin askıya alınması zarf dışıdır | beyan + B.8'de sayılır |
| **DUR-05f-12** | test amacı | INT-02: (a) iki kol — 150 KiB kayıpsız (C14 §6) + B22'nin kayıplı hücreleri 10 KiB · (b) tek boy. Ayrıca "zarf içinde vazgeçme = 0" kapı mı, rapor mu | (a); kapı |
| DUR-05f-5 | beyanlı sınır | Teyit = alıcı ucun ACK'i (direct-mode; `1b1425d2:transport.rs:12-14`); hub hop-ARQ gelirse taban hop teyidi olur | beyan (C15 v3 §6) |

Kapananlar: DUR-05f-1 (a) EVET → DUR-05f-6 ile genişler · DUR-05f-2 reddedildi → DUR-05f-7 / -9 · DUR-05f-3 / -4 gereksinim oldu → §6, §7.

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

**B.1 Zorunlu tuzak** (client-core, `test-util`):
- Kurulum: akışın önceki bütün birimleri teyitli, store'da o akışa ait birim yok. FIN dahil son kayıtlar offer kanalında ya da akış başına yerel tamponda bekletilir; runtime görevi alımı bir test kancasıyla tutar ya da kredi bitmiştir.
- İddia: `floor ≤ fin_seq` ve Faz A başlamaz.
- Serbest bırakma ve teyitten sonra: `floor = fin_seq + 1`.
- Mutant: `floor = accepted` → kırmızı.

**B.2 31 B/s, 150 KiB, sahte saat:**
- Düzenek: kama-app; K4G-06 A+ tek referans sahte (`06:294`), oranlı model, birim başına 1050 B / 31 B/s, kayıpsız.
- İddialar:
  - hiçbir `SendFailed` / `Failed` olayı ve hiçbir vazgeçme sayacı yok;
  - alıcıda `Received`, baytlar özdeş;
  - gönderende `Delivered`.
- Alt kol: iki parça arasına tek bir tam boy TEXT girer (≈ 67,7 s aralık) → alıcıda vazgeçme yok (DUR-05f-4 gereksinimi).
- Eski kod (`1b1425d2`) karşılığı: `ReceiptTimeout` ve `IdleTimeout` kırmızısı beklenir (kanca kırmızısı).

**B.3 Yerel vazgeçme, zarf dışı:**
- (a) İleri yol `T_G(E)` + bir turdan uzun ölü:
  - tam bir `SendFailed(<yerel vazgeçme>)` olayı, [T_G, T_G + tur] içinde;
  - akış 1'de `FILE_ABORT(0x04)`;
  - `reset_flow` sonrası o akışın yeniden gönderimi 0;
  - yol açılınca alıcıda `Aborted(Timeout)`.
- (b) Yol hiç açılmaz: alıcı `T_R(E)`'de vazgeçer (`FILE_RECEIPT(0x03)` yuvada bekler, `Failed`). Gelen birimler ACK'lenip atılır.
- (c) Faz A'da alındı kalıcı kayıp: `T_A(E)`'de vazgeçme. Geç `ok` → `receipts_ignored`.

**B.4 Karşı örnekler — zarf içinde yanlış vazgeçme yok** (her kol sınırın hemen altında):
- (a) **ACK kaybı:** k_r ardışık ACK düşer → G saati sınıra dek çalışır, vazgeçme yok; ilk ACK yeni teyit getirir; tamamlanır.
- (b) **Tek eksik eski birim:** birim u k_f kez düşer, u'dan büyük birimler teyit edilir. Gönderende `confirmed` artar, `floor` donar. Alıcıda `received` artar, teslim u'da bekler. İki tarafta da vazgeçme yok; u gelince tamamlanır.
- (c) **Bayat ACK:** aynı SACK durumu tekrar tekrar gelir → `confirmed` sabit, saat sıfırlanmaz. Pozitif kontrol: gerçek durgunluk sınırı aşınca vazgeçme tam bir kez.
- (d) **Kredi bloklu + tüketici tutuyor (gönderim fazı):** tutma ≥ 2·T_G → G durur, R durur, vazgeçme yok; açılınca tamamlanır.
- (e) **FILE_RECEIPT k_r kez düşer** → T_A içinde gelir → `Delivered`.
- (f) **60 s kesinti** (C1 hücresi): vazgeçme yok.

**B.5 Oturum geçişi** (§9'un her satırı):
- (c1) FIN kabul, taban geçilmeden supersede → Faz A; alıcı drain'le tamamlarsa `Delivered`.
- (c2) Alıcı `Closed`'da eksik → `Failed(<oturum bitti>)` + `FILE_RECEIPT(timeout)` → gönderende `ReceiverTimeout`.
- (c3) Kapanış + yeniden bağlanma; oturumsuz aralık kesinti bütçesinden sayılır.
- (c4) FIN öncesi supersede → `Superseded` (mevcut davranış).
- (c5) Erken iptal kaydı, eski oturumlar `Closed` olunca düşer; zamanlayıcı yok.

**B.6 Erişimci** (client-core):
- `confirmed` birim başına tam bir kez artar; yinelenen ACK/SACK/RECOVERED artırmaz; İptal sayılmaz.
- `received` sırasız kabulde artar.
- Akış ya da oturum kapanınca `None`.

**B.7 Değişecek mevcut testler (DUR-8 biçimi; kod commit'inde):**
- `1b1425d2:crates/kama-app/tests/file_transfer.rs`'te `FILE_IDLE_TTL` / `FILE_RECEIPT_TTL` / `IdleTimeout` / `ReceiptTimeout`'a dayanan 11 test, örneğin:
  - `receiver_idle_timer_starts_at_file_begin_acceptance`
  - `sender_receipt_timer_starts_when_file_end_is_accepted`
  - `lost_chunk_is_never_resent_and_the_receiver_times_out`
- `1b1425d2:crates/kama-node/src/node/tests.rs:529-602`, `:678-707`.
- `1b1425d2:file_transfer.rs` birim testleri `:2523-2598` (erken iptal kaydı ömrü), `:2645-2689` (alındı TTL'i).
- Her biri için tablo: eski iddia · neden yeşildi · yeni iddia (aynı fikstür).

**B.8 INT-02 kolu** (05e; DUR-05f-12):
- (i) Gerçek 31 B/s, 150 KiB, kayıpsız (≈ 88 dk tel baytıyla; C14 §6).
- (ii) B22 hücreleri (`06:323`), 10 KiB.
- Sütunlar:
  - tamamlanma ve süre;
  - G / R / A vazgeçme sayısı (zarf içinde 0 olmalı);
  - en uzun G ve R aralığı (§8.2 sınırının altında);
  - C15 v3 §5 (i) tanımıyla FILE_RECEIPT gecikmesi: doğum → ilk emisyon; ACK ve karşı uygulamaya teslim ayrı; kesintili ve kesintisiz kol ayrı; 120 s yalnız karşılaştırma.
- Sonuç "ölçüldü" diye yazılır.

## Ek C — Tarihçe: rev1 (`e5514e7a`) — MUST NOT IMPLEMENT

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
