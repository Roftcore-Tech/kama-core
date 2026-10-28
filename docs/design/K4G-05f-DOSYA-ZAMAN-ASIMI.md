# K4G-05f — Dosya aktarımında ilerleme, bekleme ve yerel vazgeçme — tasarım notu rev3.1

## 0. Künye

- **Kart / dal / durum:** K4G-05f · `k4g/05f-dosya-ttl` (taban main `1b1425d2`; rev1 `e5514e7a` → rev2 `3e0ec8fa` → rev3 `b96f7d6d` → **rev3.1 TASLAK**). Kod yok, ölçüm yok. "K4G-05f ONAYLANDI" ayrı bir mesajla gelir. Ürün kodu K4G-06 ile birlikte gelir (C16 v2 §3, §5.4).
- **Dayanaklar:**
  - Kararlar: CEVAP 16 v2 §3, §5.4 > 15 EK-1 > 15 v3 §6, §5, §9 > 14 §6.
  - Bağımsız inceleme: `decisions/incelemeci/INCELEMECI-8-05b-rev6-inceleme.md`, "05f rev2".
  - **rev3 doğrulaması:** `k4g05f/rev3-wf_1699fc80.json`, üç mercek.
    - Doğrulananlar: aritmetik; K_slot = 11 (kesintisiz); K_slot = 0'da (k_f + 1)(k_r + 1)'e iniş; ACK takviminin çekirdek okuması; Ek A bayt-özdeş; iki sözleşmenin ayrılığı; D_tük = 0'ın gövdeden çıkması.
    - Çürütülenler rev3.1'de düzeltildi:
      - ileri zincir (§8.2 Adım 1);
      - kesintinin yuva penceresini tüketmesi (Adım 2);
      - D_ack ve T_R (Adım 3);
      - k_f / k_r'nin kapsamı (§8.1);
      - 05b fikstür girdileri (§2);
      - (b)'nin ilk girdi beklemesi (§7);
      - minor bulgular (§2, §5, §13, Ek B, Ek C).
- **Sürümler:**
  - rev1 ONAYLANMADI (C15 v3 §6).
  - rev2'nin yönü kabul edildi, §8.2'ye GO verilmedi (C16 v2 §3).
  - rev3'te §8.2'nin bazı sınırları üst sınır değildi (wf_1699fc80).
  - rev2'nin ve rev3'ün yerine geçen metinleri Ek C'de, MUST NOT IMPLEMENT.
- **Uyum:**
  - K4G-06 rev5.2 (`24bc6314`, origin, dal `k4g/06-tasarim`), ortak yaşam döngüsü `06 §16 YD-1…YD-10`. Bu bölümün sahibi K4G-06'dır; 05f ona `06 §16 YD-n` biçiminde atıf verir. **Hizalamanın tek kaynağı `06 §16 YD-9`'un 12 satırlık eşleme tablosudur** (`06:497-514`).
  - K4G-06'nın önerileri (DUR-06-26/27/28) sahip kararı değildir. 05f bunlara koşullu bağlanır; ortak seçenek DUR satırında iki numarayla yazılır (§12, §13).
  - İkinci bir geçici oturum-geneli erişimci ya da pencere tasarımı ÜRETİLMEZ (C16 v2 §5.4).
- **Atıf biçimi:** `SHA:dosya:satır`.
  - Kısa adlar:
    - `file_transfer.rs`, `codec/mod.rs` → `crates/kama-app/src/`
    - `runtime.rs`, `transport.rs` → `crates/client-core/src/`
    - `receiver.rs`, `ack.rs` → `crates/adaptive-transport/src/`
    - `AMP` → `specs/application-message-protocol.md`
    - **`06` → `24bc6314:docs/design/K4G-06-AKIS-IZOLASYONU.md`** (rev5.2)
    - `05b` → `593029eb:docs/design/K4G-05b-RETX-FIX.md`
    - `05d` → `cf270a6c:docs/design/K4G-05d-RECEIPT.md`
  - SHA'lar (hepsi origin'de): `1b1425d2` main · `0c22a07d` 05c-kod · `593029eb` 05b rev6 son adayı · `cf270a6c` 05d · `24bc6314` K4G-06 rev5.2 (`11f91572` rev5.1) · `e5514e7a` / `3e0ec8fa` / `b96f7d6d` 05f rev1 / rev2 / rev3.
- **Etiketler:** [olgu] koddan · [hesap] aritmetik, ölçülmedi · [türetim] koddan önce yazılmış sınır · [çıkarım] test edilmedi · [CAL] deney başlangıç değeri.
- **Ek A:** rev2 ve rev3'ten DEĞİŞMEDİ (A.1–A.6). Sha256 `a6c546ae5d788dad4dee9a974e1796c1d40b1326aa70057bfc0f09afc46c2ea1`, 38 satır. Kapsam: `## Ek A` satırından `## Ek B`'den önceki son boş olmayan satıra dek (`~/kama-k4g-work/tools/eka-sha.sh`). Bekleyen iki değişiklik var:
  - DUR-05f-18 = DUR-06-28 (a) seçilirse A.2 ve A.5'in FILE_ABORT cümleleri Faz A'ya daralır;
  - DUR-05f-14 = DUR-06-26 kararıyla A.5'e tutma kuralı girer.
  - İkisi de karardan sonra yazılır; o zaman Ek A ⟨SHA256-BEKLİYOR⟩ olur.

## 1. Karar özeti

1. **İlerleme ve faz sınırı aynı kaldı** (C16 v2 §3 kabul): ilerleme = akışın yeni benzersiz teyidi; taban yalnız faz sınırıdır; yerel vazgeçme "karşı taraf almadı" demek değildir; G/R/A saatleri akış başınadır.
2. **§8.2 koddan yeniden türetildi:**
   - **İleri zincir:** H-sahiplenme birimin kendi son iletiminden sayılır, kopya ≤ P_c sonra gelir. Dolayısıyla ilk yeniden gönderim ≤ H + P_c, sonrakiler ≤ T_tx sonradır.
   - **Yuva penceresi:** bir varıştan sonra en çok B(D) = ⌊(105 − D)/8,41⌋ periyodik SACK vardır; kesinti pencereyi kısaltır. K_slot(0) = 11, K_slot(60 s) = 4. k_r + 1 > B₁ ise çarpımsal m tur. Oturum kapanınca yuva ≈ 1 s içinde ölür.
   - **D_ack, T_R ve T_ctrl** yeniden yazıldı.
3. **Sınıf başına kayıp tavanı:** k_f (A → B: dosya birimi, kontrol birimi, BLOCKED) ve k_r (B → A: SACK, FLOW_CREDIT/HELD, FILE_RECEIPT) her ileti sınıfına ayrı ayrı uygulanır. Enjektör buna göre tanımlıdır.
4. **İki sözleşme:**
   - (i) Deterministik kabul zarfı E_D. Girdileri: N_o, D_o, sınıf başına k_f / k_r, d, φ_min, tutma. Kapı yalnız burada.
   - (ii) Rastgele hücreler. ε, N_akt ve E_D'den çıkma olasılığının üst sınırıyla RAPOR.
   - p = 0 olan B22 hücreleri kapıya taşındı.
5. **05b girdileri T1/T2 fikstür değeridir** (P_pf = 0, d = 200 ms). Ürün sabitleri T1′/T2′'ye bağlıdır (C16 v2 §1.2).
6. **Tüketici tutması** (DUR-05f-14 = DUR-06-26):
   - (a) N_h·D_h.
   - (b) K4G-06 YD-3'ün birikimli `HELD` girdisi. 05f'nin G_H'ı YD-3 taslağına üç terim ekler.
   - Bedeller eşit ölçütle yazıldı: (a) D_h, (b) G_H. Ölçüt: karşılanacak tutma D ≤ G_H ise (a) daha ucuzdur.
   - Saat karardan önce kodlanmaz.
7. **YD-9 hizalaması koşullu yazıldı** (§12):
   - G dolumu `reset_flow(f, 0x04)` (DUR-05f-18 = DUR-06-28 (a));
   - R dolumu `stop_flow`;
   - erken iptal kaydı kendi oturumu dahil düşer;
   - T_ctrl'nin ön koşulu `Independent` şerit (DUR-05f-17 = DUR-06-27 (c)).

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

**Motor durumu** [olgu, `593029eb:crates/adaptive-transport/src/sender.rs`]:
- `Sender::new` Idle'da başlar (`:2161`). srtt yokken ilk AppData PreConverge'e geçirir (`:2586-2592`); PreConverge'den yalnız srtt oluşunca çıkılır (`:4442-4448`).
- PreConverge / Idle'da:
  - H-sahiplenmesi kapalıdır: `adopt_due_ns` → `None` (`:6867-6871`);
  - kayıp ilanı `!PreConverge ∨ srtt` koşuluna bağlıdır (`:3632`);
  - kopya aralığı ×2 büyür ve RTO_MAX tavanı yalnız PreConverge dışında uygulanır (`:6707-6715`);
  - ilk kopya gönderimden 1,5·(boy + 65)/31 s sonra vadelidir (`:7583-7597`).
- **RTT örneği:** doygun ya da şişmiş `ack_delay`'li ACK örnek vermez (`:3598-3607`).
- **Ürün profili:** CS-probu / bootstrap kapalıdır (`1b1425d2:runtime.rs:272`, `bootstrap_rtt: false`).
- **Sonuç:** her yeni oturumun motoru, ilk kabul edilen RTT örneğine dek PreConverge'dedir. Bu, Faz A'da yeni oturuma taşınan FILE_RECEIPT'i de kapsar (§9).
- **Sahiplenmenin çapası:** max(son ilerleme, son iletim) + H (`:6866-6878`). Son ilerleme, yeni benzersiz teyit üretmeyen ACK'lerde de (tekrar / tombstone) ilerleyebilir (`:3486-3490`, `:3654-3659`).
- **H_maks = 729,8 s'nin iki öncülü** (`:3849-3858`, `:6848-6861`):
  - 2·EAI ≤ 3·L, yani 1050 B birimde pacer ≳ 19,6 B/s;
  - `escalate_permille` = 300.

**Alıcı epizodu** [olgu, `1b1425d2:receiver.rs`]:
- Yeni epizot (4 SACK'lik kota) yalnız ilk varışta ya da aralık 4 × taban medyanını aşınca açılır (`:199-215`, `:349-361`).
- Kadans son emisyondan sayılır, varıştan değil (`:293`, `:309-323`). Bu yüzden mevcut bir `Receiver`'da bir varıştan sonraki ilk kadans SACK'i t_u + 8,41⁻ s'de düşebilir.

**05b rev6 girdileri** [türetim; **T1/T2 FİKSTÜR değerleri, ürün sınırı DEĞİL**, `05b`]:
- Değerler (31 B/s): T_max(DATA) = 101,68 s, T_max(kopya) = P_c = 135,56 s (`:612-615`); A = RTO_MAX = 30 s (`:590`); H_maks (T2, srtt ≤ L) = 729,8 s (`:595`); L = (tavan + S_maks)/R + 2d = 72,98 s (`:619`).
- Fikstür koşulları: τ = 10 ms, S = 1050 B, **P_pf = 0 (kuyruk-probu ve FEC kapalı, `:592`)**, d = 200 ms tek yön.
- Ürün kipindeki sınırlar C16 v2 §1.2'nin T1′/T2′ sonucuna bağlıdır; 05b şeridi bunları türetiyor.
- C16 v2'nin dayanak satırının gerçekte teyit ettiği: T1/T2'nin dört testi yeşil ve raporla birebir — 31 B/s'de DATA 67,73 s / ARQ 68,92 s / kopya 101,6 s, sınırlar 101,68 / 101,68 / 135,56 s, T_birim 1444,5 s, T2 en uzun bekleme 124,9 s (`decisions/C16-v2.md:7`). H_maks ve L orada ayrıca teyit edilmedi.

**K4G-06 rev5.2** [tasarım, `06` = `24bc6314`]:
- Akışlar (`06:61-62`). Kontrol şeritleri 0 ve 1 `Independent` kipte; tekilleştirme penceresi W_D ile tüketim kredisi W_c ayrı, `max_seq = min(floor_a + W_D, c + W_c)` (`06:83-87`, `:112-113`; DUR-06-27 (c) önerisi).
- W_D hıza bağlı türetilir: W_D ≥ W_D(r, E) = ⌈(b_C + ρ_C·r·T_rec)/S_min⌉ + 1, T_rec = T_ctrl + T_düzey2; seçim W_D = 2048 [CAL]; geçerlilik aralığı ve motor ön koşulu (`06:88-103`).
- Akış-0/1 kredisinin yenilenmesi: bugünkü kuralla çarpımsal T_kredi,ctrl, öneri R-KRD ile toplanır (`06:244-251`).
- FLOW_CREDIT tetiklidir (`06:118`, `:243`). BLOCKED aralığı ≤ min(RTO_MAX, `idle_hold`/2) (`06:252`). FLOW_CREDIT, eş başına ≤ 1 çerçeve / T_min (`06:270`).
- Kontrol kovası ve kontrol başı sınırı (`06:124`, `:212`).
- FIN, RESET ve H_C_tavan (`06:254-260`). Oturum değişimi (`06:261`). Tek sonlandırma çıktısı (`06:276`).
- **Ortak yaşam döngüsü:** `06 §16 YD-1…YD-10` (`06:379-522`). YD-2 (`:402`), YD-3 (`:415`), YD-4 (`:436`), YD-5 (`:452`), YD-6 (`:468`), YD-7 (`:481`), YD-9 (`:497`), YD-10 (`:516`).
- DUR-06-26 / -27 / -28 açıktır (`06:891`, `:824`, `:907`); sahip kararı değildir.

## 3. İlkeler

- **P1 — kanıt yoksa çıkarım yok.** Yeni teyit, yeni kabul ya da alındı gelmemesi karşı taraf hakkında bir durum türetmez.
- **P2 — terminal durumlar açık sinyalle oluşur** (`06 §16 YD-4`, YD-10): alındı · `FILE_ABORT` · `FLOW_RESET` (kodlu, bant içi `FlowEnded`; DUR-06-28 (a) seçilirse) · `stop_flow` + FILE_RECEIPT (gönderende `reset_flow_local`) · oturum sonu kuralı (§9) · kullanıcı iptali · yerel vazgeçme (P3). R3 `resolve_flow` ile (YD-2).
- **P3 — yerel vazgeçme bir politikadır.** Yalnız yerel bir saat, E_D'den türetilmiş sınırını aşınca tetiklenir; nedeni "yerel vazgeçme"dir; karşıya açık mesajla bildirilir. E_D içinde tetiklenmez (KAPI, Ek B.4). Rastgele hücrelerde tetiklenebilir (RAPOR, Ek B.8 (ii)).
- **P4 — akış başına.** Oturum geneli taban, oturum geneli "bekleyen = 0" ve ikinci bir geçici erişimci kullanılmaz (C16 v2 §5.4).
- **P5 — vazgeçme yeni kapasite istemez.** H_C_tavan içinde yer önceden ayrılmıştır (`06:256-260`; `06 §16 YD-6`). Aktarım başına en çok bir FILE_ABORT (ilk neden kalır, `1b1425d2:file_transfer.rs:134`); gelen akış başına en çok bir FILE_RECEIPT.

## 4. Kapasite · kuyruk gecikmesi · zaman aşımı (05d A1 · K4G-06 · 05f ortak bölüm)

**4.1 Üç büyüklük, üç sahip.**

| Büyüklük | Soru | Sahibi / mekanizma | Sağladığı | Sağlamadığı |
|---|---|---|---|---|
| **Kapasite** | Bu birim şimdi kabul edilebilir mi? | Alıcı kredisi (`06` §7); 05d A1 → K4G-06 akış bütçesi erişimcisi (DUR-06-14 (a), `06:879`; A.36 `06:565`); rezerv `A − 1 ≥ H_R + H_C_tavan` (C15 v3 §5 (ii)–(iii)) | Kabul edilebilirlik; kontrol kaydı için yer | Bekleme süresi (`05d:198`) |
| **Kuyruk gecikmesi** | Kabul edilmiş birim ne zaman hizmet alır? | İki düzeyli seçici: P_f, T_max (`06` §5, `:195`); düzey 2 = 05b rev6 | Türetilmiş üst sınır | Ağ kaybından bağımsız tamamlanma |
| **Zaman aşımı** | Ne zaman vazgeçilir? | Yalnız yerel vazgeçme politikası (§8) | E_D'de sonlu terminal durum | Karşı taraf hakkında bilgi (P1) |

**4.2 Kurallar.**
- **R1 — kapasite beklemesi saate girmez.** Gönderen kredi bloklu ve teyitsiz birimi yokken G durur; alıcı tüketici tutarken R durur. Faz A'da tutma: §7 ve DUR-05f-14.
- **R2 — kuyruk gecikmesi türetilmiş sınırıyla girer.** Ürün sabitleri r_min = 31 B/s'de ve E_D'nin hizmet payı tabanı φ_min'de türetilir. Ölçülen hızla küçültülmez (C15 v3 §5 (i); DUR-05f-13).
- **R3 — ağ terimi E_D girdilerinden gelir** (§8.1). Ortalama kayıptan deterministik bir sınır üretilmez (C16 v2 §3).
- **R4 — saatlerin sırası:** T_R = max(D_ack + T_G + T_ctrl(k_f, k_r), T_kredi + F_yeni) (§8.2). İlk terim, gönderenin vazgeçmesinin ve bildiriminin alıcıdan önce gelmesini sağlar. İkinci terim, G'nin durduğu kredi yenilemesi rejiminde R sessizliğinin sınırıdır; kapı hücrelerinde bağlayıcı değildir. T_A: §7.
- **R5 — 120 s (S1) saat değildir.** FILE_RECEIPT için ölçüm hedefidir: C15 v3 §5 (i) tanımıyla doğum → ilk emisyon; ACK ve karşı uygulamaya teslim ayrı; kesintili ve kesintisiz kol ayrı. TEXT/PTT'ye uygulanmaz.

**4.3 Bir aktarımın beklemeleri.**

| Bekleme | Kapasite mi? | Saat | Sınır terimi (§8.2) |
|---|---|---|---|
| Akış bütçesi yok (`admissible` = 0, A.36) ya da kredi bloklu, motora verilmiş teyitsiz birim yok | evet | G durur (§5) | — (bekleten tarafın saati) |
| Kredi yenilemesi (BLOCKED → FLOW_CREDIT) | evet | G durur; alıcıda R çalışır | T_kredi (R'nin ikinci terimi) |
| Hizmet sırası (DRR, katman) | hayır | G çalışır | T_max(DATA), P_f, φ_min |
| İleri kayıp (ARQ / kopya) | hayır | G çalışır | F(n): Δ_lp + H + N_live·P_c ilk yeniden gönderim, sonra T_tx(N_live) = A + N_live·P_c |
| SACK kaybı, yuva penceresi | hayır | G çalışır | (min(k_r, K_slot(0)) + 1)·T_ACK; kesinti SACK tarafında ⌈D_o/T_ACK⌉·T_ACK ve B₁ = B(D_o)'yu küçültür; k_r + 1 > B₁ ise m tur |
| Alıcı tüketicisi tutuyor (Faz G) | evet | R durur, G durur (kredi bloklu) | — |
| Alıcı tüketicisi tutuyor (Faz A) | evet | A çalışır (DUR-05f-14) | (a) T_A'da N_h·D_h payı · (b) seçilirse bildirilen tutma düşülür, en çok D_tut,max |
| Alıcı işlemesi | hayır | A çalışır | T_işlem |
| FILE_RECEIPT ters yönde | hayır | A çalışır | T_ctrl(E) = T_ctrl(k_r, k_f) + T_düzey2 + T_pencere + T_kredi,ctrl; ön koşullar P1–P4 (§8.1) |
| FILE_RECEIPT, akış-1 kredi ilanının kaybı yüzünden bekliyor | hayır | A çalışır | T_kredi,ctrl (yalnız şerit kredisi tükenmişse; §8.2; YD-7 `06:481`) |
| FILE_RECEIPT, alındıyı bekleyenin KENDİ akış-1 hazır kuyruğunda T_min'den uzun süredir okunmamış birim var (kendi durgun tüketicisi) | evet | **A durur** (R1; YD-7 `06:481`; `06 §16 YD-9` m.6) | — |
| Oturumsuz aralık (yalnız Faz A) | hayır | A çalışır | N_o·D_o; yeni oturumun motoru P3'ü sağlamaz (§8.2 PreConverge) |

**4.4 05d A1 ile uyum.** Değişmedi.
- Erişimci alanları A.36'dadır (DUR-05f-6 (a)).
- Alındı düşürme A.21 ile kalkar (`06:550`); 05d §10 (iii)'ün "süreli kayıt" satırı (`05d:183`) geçersizleşir.
- H_C_tavan değişmez.

## 5. Gönderen ilerlemesi (G saati) ve faz sınırı

Değişmedi (C16 v2 §3 kabul). Erişimci `FlowTxProgress(oturum, akış)` — A.36, DUR-05f-6 (a). **Yetkili alan listesi A.36'dır (`06:565`):** `available` ve `peer_held` de vardır. 05f'nin kullandığı alanlar:
- `accepted`: sıradaki `flow_seq`.
- `confirmed`: Teyit çıktılarından akışa düşen farklı birim sayısı; İptal sayılmaz (`06:276`); FEC RECOVERED sayılır.
- `floor`: en küçük teyitsiz `flow_seq`; motora henüz verilmemiş birimler dahil. Ancak hiçbiri yoksa `floor = accepted`.
- `blocked`: BLOCKED bölümü etkin (`06:252`).

Saat ve fazlar:
- `fin_seq` = FIN `send` → `Ok` sonrası `accepted − 1`; düğüm döngüsü tek iş parçacıklıdır (`1b1425d2:crates/kama-node/src/node.rs:1`).
- **Faz G:** `FILE_BEGIN` kabulünden `floor > fin_seq`'e dek.
- **Faz A:** `floor > fin_seq` olduğunda, ya da FIN kabul edilmişken (`floor ≤ fin_seq`) akışın oturumu bittiğinde başlar (§9; YD-1 S2).
- **G saati:** çalışır = motora verilmiş teyitsiz birim var ∨ (kredi ve akış bütçesi var ∧ sunulmayı bekleyen kayıt var). Durur = (`blocked` ∨ akış bütçesi yok, A.36) ∧ motora verilmiş teyitsiz birim yok — §4.3 ile aynı koşul. Sıfırlanır = yalnız `confirmed` artınca. Dolum = T_G(E_D) → §8.3.
- **Tuzak:** store boşken FIN offer kanalında ya da yerel tamponda olabilir; o anda `floor` = o kaydın `flow_seq`'idir, `accepted` değildir (Ek B.1).

## 6. Alıcı dosya-idle (R saati) — `FILE_IDLE_TTL`'in yerine

Erişimci `FlowRxProgress(oturum, akış)` — A.36 (`06:565`; yetkili liste orada, `fin_pending` ve `bound` dahil). 05f'nin kullandığı alanlar:
- `received`: ilk kez kabul edilen farklı `flow_seq`; sırasızlar dahil.
- `delivered`: uygulamaya verilen sıradaki `flow_seq`.
- `granted`: ilan edilmiş `max_seq` (`06:243`).
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

## 7. Alındı bekleme (A saati) ve tüketici tutması — `06 §16 YD-3`, YD-7 ile hizalı

- **Yer:**
  - FILE_RECEIPT akış 1'de gider ve yuvası düşürülmez (`06:113`, A.21 `06:550`; yükümlülük `06 §16 YD-6`).
  - Akış 1 `Independent` ise (DUR-05f-17 = DUR-06-27 (c) seçilirse) FILE_RECEIPT, öndeki kayıp bir kontrol kaydını **W_D − 1 kayda dek** beklemez. RECEIPT'ler kama-app'in FILE_* payı yüzünden bir boşluğun arkasında W_D − 13 kayıtta bekler (`06:103`).
- **A saati:**
  - S2'de, yani Faz A'nın başında başlar (`06 §16 YD-1`): FIN dahil bütün birimler teyitli, **ya da** FIN kabul edilmişken (`floor ≤ fin_seq`) akışın oturumu bitti (§5, §9).
  - Oturumsuz aralık N_o·D_o'dan sayılır.
  - **Durur** (`06 §16 YD-9` m.6, YD-7 `06:481`): alındıyı bekleyen tarafın **KENDİ** akış-1 hazır kuyruğunda T_min'den uzun süredir okunmamış birim varken. Bu, alındıyı bekleyenin kendi tüketicisinin kapasite beklemesidir (R1): onun akış-1 kredisi `c + W_c`'de durur, karşı tarafın FILE_RECEIPT'i kredi bekler. Transportta `FlowRxProgress` ile gözlenir (A.36 `06:565`).
  - Akış-1 kredi **ilanının kaybı** yüzünden bekleme A'yı durdurmaz; T_kredi,ctrl T_ctrl'ye girer (§8.2; YD-7).
  - Dolum = T_A(E_D) → §8.3.
- **Durum:** FIN dahil bütün birimler teyitliyken alıcının tüketicisi FIN'i almazsa (R2, `06 §16 YD-1`) FILE_RECEIPT doğmaz. Faz G'deki tutma sorun değildir: kredi bloklu olduğu için G durur (`06 §16 YD-7`; B1 `06:588`).

**Seçenekler (DUR-05f-14 = DUR-06-26; ikisi birlikte karar). Öneri (a), D_h = 30 s [CAL]; (b) yalnız karşılanacak tutma D > G_H gerekirse** (`06:431`; `06 §16 YD-9` m.1).

**(a) Tutma zarfa girdi olarak girer — öneri:**
- T_A = T_işlem + N_h·D_h + T_ctrl(E), N_h = 1, D_h = 30 s [CAL].
- Tel değişmez; alıcıda mekanizma yoktur.

**(b) Birikimli, tavanlı `HELD` girdisi** (`06 §16 YD-3`, `06:415`):
- Alıcı: R2 ≥ T_min olunca girdiyi koyar, R2 sürdükçe her T_BLK'da yineler (eş başına ≤ 1 çerçeve / T_min).
- Gönderen: A(t) = max(0, (t − t_S2) − min(H, D_tut,max)). H = alınan en büyük `held_ms`. D_tut,max ≥ 30 s [CAL], aday 600 s.
- T_A = T_işlem + T_ctrl(E) + G_H.
- **İki ek koşul** (`06 §16 YD-9` m.1):
  - **(b-i) Yuva ömrü.** HELD, B'nin sunucu yuvasındaki ACK yolunda gider (`06:422`). Faz A'da A'dan hiçbir şey o yuvayı tazelemez; yuva A'nın son datagramından 105–106 s sonra emekliye ayrılır (`1b1425d2:runtime.rs:3430`, `:71`). O andan sonra HELD gitmez.
    - (b) ancak HELD emisyonu yuvayı canlı tutarsa ya da A Faz A boyunca canlı tutma gönderirse çalışır; yoksa D_tut,max ≤ yuva ömrü (≈ 105 s) olmalıdır.
    - Koşul tutmazsa G_H üst sınır değildir. Örnek (kayıpsız D-0, tutma 300 s ≤ D_tut,max): son `held_ms` ≈ 91–101 s; A ≈ t_S2 + 331,6 s'de dolar; FILE_RECEIPT ≈ t_S2 + 425,6 s'de gelir. Eşikler: D-0'da > ≈ 206 s, D-2'de > ≈ 359 s tutma [hesap, wf_c90086d9 sonuç 0 bulgu 4].
    - Bu koşul (a)'yı ayrıca destekler.
  - **(b-ii) Fazla düşme.** H, R2'nin başından (t_R2) sayılır; S2 ise t_S2 ≥ t_R2'de başlar. Düşülen fazlalık ≤ min(D_tut,max, t_S2 − t_R2), ve t_S2 − t_R2 ≤ D_ack (FIN teyidinin dönüşü, §8.2).
    - D_ack kesinti terimini zaten içerir; ayrıca N_o·D_o eklenmez (rev5.1'in "+ N_o·D_o"su kesintiyi iki kez sayıyordu).
    - Güvenli yöndedir: yanlış vazgeçme üretmez, gerçek vazgeçmeyi geciktirir.
    - rev3.1'in "en çok bir ACK yolu" ifadesi yanlıştı ve kalktı.

**G_H türetimi** [türetim; ölçülmedi; (b-i) koşuluyla]:
- R2'nin başı t_R2, FIN kabulüdür; S2 ise t_S2 ≥ t_R2'de, FIN'in teyidiyle başlar.
- A(t) = (t − t_S2) − H_alınan(t) ≤ (t − t_R2) − H_alınan(t) =: **gecikme(t)**. Bu, tutma süresinin henüz gönderene ulaşmamış kısmıdır.
- Gecikmenin bileşenleri:
  1. **İlk girdinin ofseti:** girdi R2 ≥ T_min olunca doğar → ≤ T_min.
  2. **Kayıp girdiler:** HELD girdileri B → A yönündedir (k_r, §8.1). Bir akışın HELD yinelemeleri bir tekrar zinciridir: ardışık en çok k_r yineleme kaybolur. Yinelemeler T_BLK aralıklıdır ve her biri T_min hız sınırıyla gecikebilir → ≤ (k_r + 1)·(T_BLK + T_min).
  3. **Kesinti:** kesintide kaybolanlar k_r'ye sayılmaz → + N_o·D_o.
  4. **Uçuş:** en son girdinin varışı → + L.
- **G_H = T_min + (k_r + 1)·(T_BLK + T_min) + N_o·D_o + L** (`06:415` YD-3 ile aynı).
  - Bu sınır, tutma sırasında A'nın zirvesini de, tutma bitince A'ya kalan sızıntıyı (t_end − t_son girdi) da kapsar.
  - Birikimli girdi sayesinde kayıp girdiler birikmez; tek bir başarılı girdi öncesindeki bütün tutmayı kapsar.
- **Tavan:** H > D_tut,max olursa A yeniden işler. D_tut,max'ı aşan tutma zarf dışıdır; vazgeçme mümkündür, > D_tut,max + G_H + T_ctrl'de kesindir.
- **Saat oranı:** iki taraf süre ölçer, mutlak zaman taşımaz. Monoton saatlerin oran farkı ppm mertebesidir ve ihmal edildi [çıkarım].

**[hesap]** 31 B/s (§8.2 girdileri; T_işlem = 1 s; T_min = 1 s; N_live = 1; P1–P4; kapı hücrelerinde T_ctrl(E) = T_ctrl(k_r, k_f), çünkü T_düzey2 = T_pencere = T_kredi,ctrl = 0, §8.2):

| Hücre | T_ctrl(E) | (a) T_A, N_h = 1, D_h = 30 s | G_H | (b) T_A (yalnız (b-i) ile) |
|---|---|---|---|---|
| D-0 | 124,6 s | 155,6 s | 105,0 s | 230,6 s |
| D-1 | 1 047,4 s | 1 078,4 s | 165,0 s | 1 213,4 s |
| D-2 | 1 317,4 s | 1 348,4 s | 258,0 s | 1 576,4 s |
| D-3′ | 1 399,0 s | 1 430,0 s | 289,0 s | 1 689,0 s |
| D-3 | 1 970,2 s | 2 001,2 s | 506,0 s | 2 477,2 s |

**Bedeller — eşit dürüstlükle:**

| | (a) N_h·D_h — öneri | (b) HELD, birikimli, tavanlı |
|---|---|---|
| Her aktarımın T_A'sına eklenen | D_h (30 s) | G_H (105–506 s). İlk girdiler kaybolmuş olabileceği için eş ölü olsa da sabit eşiğe girer |
| Karşıladığı tutma | ≤ D_h | ≤ D_tut,max (aday 600 s), **yalnız (b-i) ile**; (b-i) yoksa ≤ yuva ömrü (≈ 105 s) |
| Ölü eşin fark edilmesi | T_işlem + D_h + T_ctrl | T_işlem + G_H + T_ctrl; (a)'dan geç, çünkü G_H > 30 s |
| Sınırı aşan tutma | vazgeçme mümkün; > D_h + T_ctrl'de kesin. Alıcı bütün veriye sahipken aktarım biter (yanlış "başarısız", DUR-05f-11) | vazgeçme mümkün; > D_tut,max + G_H + T_ctrl'de kesin |
| Tel | değişmez | yeni girdi türü `HELD` (A.8 `06:537`) |
| Alıcı mekanizması | yok | R2 sırasında T_BLK aralıklı, **istenmemiş** FLOW_CREDIT emisyonu: yeni bir alıcı zamanlayıcısı (bugün FLOW_CREDIT tetiklidir, `06:118`), ayrıca (b-i)'nin yuva kuralı |
| Bant | yok | ≈ 39 B / T_BLK ≈ 1,3 B/s, 31 B/s'de, tutma süresince [hesap, `06:891`] |
| Yeni sayı | N_h, D_h [CAL] | D_tut,max [CAL]; G_H türetilmiştir |
| Spec | Ek A A.5'e tutma terimi | Ek A A.5 yeniden yazılır ("bildirilen tutma A'dan düşülür"); şu an Ek A bunu söylemiyor |
| C14 §6 "sonsuza kadar açık kalmasın" | sağlar | tavanlı biçim sağlar (YD-3); tavansız biçim sahip istisnası isterdi (DUR-05f-7) |

**Ölçüt ve öneri** (05f ile `06 §16 YD-3` aynı): karşılanması gereken tutma süresi D ise (a)'nın bedeli D, (b)'ninki G_H'tir.
- D ≤ G_H → (a) daha ucuz. INT-02'nin 30 s'si için bütün hücrelerde böyle.
- D > G_H → (b) daha ucuz (D = 600 s gibi); ama yalnız (b-i) sağlanırsa.
- **Öneri: (a), D_h = 30 s [CAL]; (b) yalnız D > G_H gerekirse açılır** (örneğin INT-03'te dakikalarca süren FIN sonrası tüketici durması ölçülürse; `06:431`).
- Karar sahibindir. Hangi seçenek olursa olsun saat bu karardan önce kodlanmaz (C16 v2 §3).
- **Test eşlemesi** (`06 §16 YD-9` m.12): Ek B.9 (a)'nın ayırt edici fikstür kolu K4G-06 B29 (i)/(i-mut)'tır (`06:618`; kayıpsız fikstür koşuluyla).

## 8. Yerel vazgeçme politikası — iki sözleşme ve türetim

**8.1 İki sözleşme.**

**(i) Deterministik kabul zarfı E_D — KAPI** (DUR-05f-12 (a), DUR-05f-13). Girdiler:

| Girdi | Anlam | Kapı hücreleri |
|---|---|---|
| r_min | Hat hızı tabanı (S2: C15 EK-1 §2). `PRE_CONVERGE_FLOOR_BW` = 31 yalnız aynı sayıdır; kapsamı PRE_CONVERGE'dir (`1b1425d2:crates/adaptive-transport/src/sender.rs:1485`) | 31 B/s |
| d | Tek yön yol gecikmesi. L ve H_maks bunu içerir | 200 ms (05b fikstürü) |
| φ_min | Sunulan yük altında akışın hizmet payı tabanı (`06:210`; sayı DUR-06-3) | 1 (tek dosya) |
| N_o, D_o | **N_o = hücredeki toplam kesinti sayısı; bir kesinti iki yönü aynı anda keser.** D_o azami süredir. Oturumsuz aralık da kesintidir, **yalnız Faz A'da**: Faz G'de oturumsuz bir kesinti aktarımı `Superseded` ile bitirir (§9); o gerçekleşmenin beklenen sonucu `SendFailed(Superseded)`'dır ve tamamlanma kapısına girmez (Ek B.4) | N_o ∈ {0, 1}, D_o = 60 s |
| **k_f** (A → B) | Yöne göre sınıf (`06 §16 YD-9` m.7): A → B yönündeki **bütün** çerçeveler — dosya birimleri; akış 0 / 1 birimleri (FLOW_RESET, FILE_ABORT); FLOW_BLOCKED; A sunucusunun B istemcisine SACK'leri; B'nin akış 0 / 1'i için A'nın FLOW_CREDIT'i | {0, 3} |
| **k_r** (B → A) | B → A yönündeki bütün çerçeveler — B sunucusunun SACK'leri; FLOW_CREDIT ve HELD; akış-1 RECEIPT ve FILE_RECEIPT | {0, 3, 4, 11} |
| **N_live** | Bir motorda aynı anda iletilmiş, teyitsiz birim sayısının üst sınırı; kontrol birimleri dahil (05b tanımı, `05b:185`) | 1 (fikstür, aşağıda) · duyarlılık 2 |
| SACK kapsamı | u'nun varışından ilk başarılı SACK'e dek u'nun üstünde < 16 boşluklu PN bloğu (§2) | N_live = 1'de tutar |
| Tutma | DUR-05f-14: (a) N_h × D_h (öneri), ya da (b) D_tut,max (yalnız (b-i) ile, §7) | N_h = 1, D_h = 30 s · D_tut,max ≥ 30 s |
| **Ön koşullar P1–P4** (`06 §16 YD-9` m.5) | **P1** akış 0 / 1 `Independent` (DUR-05f-17 = DUR-06-27 (c)). **P2** W_D ≥ W_D(r, E) (`06:88`; r geçerlilik aralığında, `06:101`). **P3** motor ∉ {PreConverge, Idle}: ≥ 1 kabul edilmiş RTT örneği. **P4** akış-0 / 1 kredi ilan yolu zarftadır (k_f / k_r yöne göre, yukarıda) | ön koşul |
| P3'ün kapsamı | P3 yalnız T_ctrl'nin değil, §8.2'deki **bütün zincirlerin** ön koşuludur: ileri zincir H-sahiplenmesine ve kayıp ilanına dayanır, ikisi de PreConverge'de kapalıdır (§2) | her iki yönün motoru ölçülen koldan önce ısıtılır |
| Sınıfsız kalan iletimler | CS-probu / yankı: ürün profilinde kapalı (`1b1425d2:runtime.rs:272`), E_D'de yok. Kuyruk-probu ve FEC onarım sembolü: E_D'de yok (05b fikstürü P_pf = 0, `05b:592`). Ürün kipinde (T1′/T2′) bunlar ayrı tekrar zinciri olur ve son ilerlemeyi teyitsiz ilerletebilir; o zaman Δ_lp (§8.2) yeniden türetilir | yok |

- **Kayıp modeli — tekrar zinciri başına.** Yönün tavanı k (k_f ya da k_r) her tekrar zincirine ayrı ayrı uygulanır: zincirde ardışık en çok k üye kaybolur. Zincirler:
  1. Bir birimin ardışık iletimleri (veri ya da kontrol). Oturumlar arası taşınan yükümlülükte (FILE_RECEIPT, FILE_ABORT; YD-6) zincir oturumlar boyunca tektir.
  2. Bir birimi taşıyan SACK emisyonları: alıcı sunucusunun, u'nun son varışından sonraki ve u'yu taşıyan emisyonları. Yuvalar arasında tek zincirdir. u'yu taşımayan SACK'ler (ör. yeni bir yuvanın BLOCKED'e verdiği SACK) bu zincire girmez ve koşuyu bölmez.
  3. Bir akışın BLOCKED, FLOW_CREDIT ve HELD yinelemeleri, gönderen soket başına.
  - Yön içindeki ardışık datagram tavanı tek başına yetmez: araya giren başka başarılı çerçeveler u'nun zincirini sınırsız bırakırdı (wf_c90086d9 bulgu 3 (iii)).
  - Transport kayıt türünü ayırmaz (D4; `06:507`). Zincirler transport kimlikleridir (birim, yuva, akış); kayıt türü değildir.
- **Koşu semantiği:**
  - (i) Kesintide kaybolan üye sayılmaz ve koşuyu bölmez; koşu kesintinin üstünden sürer.
  - (ii) Koşu yuvalar ve oturumlar arasında sürer. Enjektör sayacı yuva ya da oturum değişiminde sıfırlanmaz.
- **Enjektör ve üyelik:**
  - Her gönderen soketinin çıkışına (`EgressPolicy`; sunucu çıkışı `runtime.rs` sunucu görevinde ayrıdır) deterministik bir desen uygulanır.
  - Bir koşunun E_D'de olduğu, koşu sonunda günlükten doğrulanır. Günlükte her iletimin (birim kimliği, tür, PN) ve kaderinin kaydı vardır. Denetlenenler: zincir başına ardışık kayıp ≤ k; N_live ≤ beyan; P3 (Ek B.4).
- **N_live fikstürü:** kapı hücrelerinde N_live = 1.
  - Düzenek sıradaki birimi, öncekinin `confirmed`'ı artınca sunar; hücrede başka akış yoktur.
  - N_live > 1'de her yeniden gönderim (N_live − 1)·P_c büyür (§8.2 duyarlılık satırı).
  - **Ürün sabitleri ürünün N_live'ıyla hesaplanır** (akış kredisi W0_BK + kontrol birimleri; DUR-05f-13).
- **P3 ısınması:** ölçülen koldan önce iki yön de ≥ 1 birim alıp verir; her motor ≥ 1 kabul edilmiş RTT örneği almıştır (`diag` sayacı).
  - P3'ün tutmadığı durumlar ve sayıları §8.2 "PreConverge" bölümündedir.
  - Ürün tarafı (PreConverge kopya aralığı) DUR adayıdır; orkestratör sahibe götürüyor (`06:102`).
- **Sözleşme:** E_D içinde G, R ve A saatlerinin hiçbiri dolmaz → yerel vazgeçme = 0. Kapı Ek B.4 ve B.8 (i)'dir.

**(ii) Rastgele kayıp hücreleri — RAPOR** (B22 kayıplı hücreleri `06:611`; p = 0 olan B22 hücreleri deterministiktir ve B.8 (i) kapısına girer).
- Ürün aynı sabitlerle koşar. Gözlenen vazgeçme sayısı raporlanır; **deterministik garanti yazılmaz** (C16 v2 §3).
- Rapor şunları içerir:
  - ε = 10⁻³ [CAL];
  - koşulan aktarım sayısı N_akt;
  - ölçülen fırsat sayıları N_fırsat,s: s yöndür (k_f / k_r); fırsat, bir tekrar zincirinde (§8.1) k_s + 1 ardışık üyenin kaybolabileceği bir konumdur;
  - model değeri Σ_s N_fırsat,s · p_s^(k_s + 1).
- Model değeri, bağımsız Bernoulli kaybı varsayımıyla **E_D'den çıkma olasılığının üst sınırıdır** (birleşim sınırı). Vazgeçme tahmini değildir.
- Gözlenen vazgeçme oranı (vazgeçme / N_akt) bu üst sınır ve ε ile yan yana yazılır; hüküm yoktur.
- [hesap] D-3 tavanlarında p_r = %80 için 0,8¹² ≈ 0,069 / koşu: bu hücre E_D'nin dışındadır.

**8.2 Türetim** [türetim; koddan önce; ölçülmedi; girdiler §2'deki T1/T2 fikstür değerleri].

Bu bölüm bir türetim açığını kapatır; runtime'da yanlış timeout oluştuğunun kanıtı değildir (C16 v2 §3).

Girdiler (31 B/s): T_D = T_max(DATA) = 101,68 s · P_c = 135,56 s · A = 30 s · H = H_maks = 729,8 s · L = 72,98 s · T_ACK = 8,41 s · `idle_hold` = 105 s · T_BLK = 30 s · T_min = 1 s [CAL, `06:270`] · N = N_live · T_tx(N) = A + N·P_c (N = 1'de 165,56 s).

**Adım 0 — motor durumu.** Adım 1–3'ün bütün zincirleri P3 altında yazılır (§8.1). P3 tutmadığında: bölüm sonundaki "PreConverge".

**Adım 1 — ileri zincir (koddan; §2 "kurtarma takvimi" ve "motor durumu"):**
- x₁ ≤ T_D: yeni birimin ilk gönderimi, ya da G sıfırlandığında zaten uçuşta olan gönderim.
- x₂ ≤ max(lp, x₁) + H + N·P_c. Sahiplenmenin çapası max(son ilerleme, son iletim)'dir (`593029eb:sender.rs:6866-6878`); kopya ≤ N·P_c içinde hizmet alır (`05b:605`).
- xⱼ ≤ xⱼ₋₁ + T_tx(N), j ≥ 3 (`05b:605`).
- **Çapa kayması Δ_lp:**
  - Son ilerleme yalnız bir PN'nin ilk ACK'iyle ilerler: canlı kayıt (`593029eb:sender.rs:3461-3462`), tombstone (`:3486-3490`) ya da kayıp ilanının geri alınması (`:3559`). Bayat SACK ilerletmez (`:3650-3658`).
  - N_live = 1'de x₁'den sonra son ilerlemeyi ancak u'dan önce gönderilmiş PN'ler ilerletebilir (teyit edilmiş birimin kopyaları, tombstone). Bunların ilk ACK'i en geç bir uçuş ve bir SACK zinciri sonra gelir.
  - u'nunkinden büyük bir PN'nin ACK'i ise u için kayıp ilanı üretir (ARQ yolu); bu yol H yolundan kısadır.
  - → **Δ_lp(k′) ≤ L + (min(k′, K_slot(0)) + 1)·T_ACK.** k′, ACK'lerin geldiği yönün tavanıdır; dosya birimi için k_r.
- **F(n) = T_D + [n ≥ 2]·(Δ_lp + H + N·P_c + (n − 2)·T_tx(N)).**
- **Kesinti:** kesintinin yuttuğu iletimler sayılmaz; kesinti bitince sonraki iletim en çok bir aralık sonra gelir → n'ye + N_o, F'ye + N_o·D_o. Türetim N_o ≤ 1 içindir.

**Adım 2 — yuva pencereleri (koddan; §2 "ACK takvimi" ve "alıcı epizodu"):**
- Mevcut bir `Receiver`'da kadans son emisyondan sayılır. Varıştan sonraki ilk emisyon t_u + T_ACK⁻'de olabilir (`1b1425d2:receiver.rs:293`, `:309-323`). Bir varıştan sonraki pencerede ⌊`idle_hold`/T_ACK⌋ = 12 emisyon vardır; D süreli bir kesinti bunların en çok ⌈D/T_ACK⌉ tanesini yutar.
- **B(D) = ⌊`idle_hold`/T_ACK⌋ − ⌈D/T_ACK⌉**; K_slot(D) = B(D) − 1.
  - K_slot(0) = 12 − 1 = **11**.
  - K_slot(60 s) = 12 − 8 − 1 = **3** [hesap].
  - Yeni bir `Receiver` 4'lük kota epizoduyla bir emisyon fazla tutabilir (`:199-215`); en kötüsü mevcut `Receiver`'dır.
  - rev3.1'in ⌊(`idle_hold` − D)/T_ACK⌋ formülü mevcut `Receiver`'da bir fazla sayıyordu (K_slot(60) = 4 yazıyordu; wf_c90086d9 bulgu 1).
- **SACK tarafında kesintinin bedeli:** N_o·⌈D_o/T_ACK⌉·T_ACK = 67,28 s; yalnız D_o değil. Faz payı, ilk emisyonun (… + 1)·T_ACK teriminde.
- **Pencere sayısı:** m = 1 eğer k_r + 1 ≤ B₁; aksi hâlde m = 1 + ⌈(k_r + 1 − B₁)/B(0)⌉. B₁ = B(D_o) (N_o = 1), yoksa B(0).
  - Koşu yuvalar arasında sürer (§8.1); ikinci pencerede yalnız kalan kayıplar sayılır.
  - k_r ≤ 11 < B(0) olduğundan E_D'de m ≤ 2. m = 2 yalnız kesinti ilk pencereye düştüğünde olur; ikinci turda kesinti yoktur.
- Oturum kapanınca yuva ≈ 1 s içinde ölür (§2). Bu G'de değil, §9 kuralıyla (Faz A) işlenir.

**Adım 3 — sınırlar (N = N_live):**
- **İletim sayısı:** n_tx = k_f + N_o + 1 + (m − 1)·(k_f + 1).
- **T_G** = F(n_tx) + N_o·D_o + N_o·⌈D_o/T_ACK⌉·T_ACK + (min(k_r, K_slot(0)) + 1)·T_ACK + 2L, Δ_lp = Δ_lp(k_r).
  - Kesinti hem ileri zincire hem SACK tarafına yazılır. N_o ≤ 1 tek bir yerde gerçekleşir; bu muhafazakârdır.
- **D_ack** (u'nun alıcıda kabulünden teyidin gönderene ulaşmasına):
  - m = 1: N_o·⌈D_o/T_ACK⌉·T_ACK + (min(k_r, K_slot(0)) + 1)·T_ACK + L.
  - m ≥ 2: Δ_lp(k_r) + H + N·P_c + ((m − 1)(k_f + 1) − 1)·T_tx(N) + L + (min(k_r, K_slot(0)) + 1)·T_ACK + L.
    - İki uçuş vardır: yeni turun ileri uçuşu ve dönüşü.
    - Kesinti ilk penceredeydi (m = 2 ancak öyle olur); ikinci turun kesinti terimi yoktur.
- **F_yeni** (yeni ya da kabul edilmemiş bir birimin alıcıda kabulü; SACK yolundan bağımsız) = T_D + [k_f + N_o ≥ 1]·(Δ_lp(k_r) + H + N·P_c + (k_f + N_o − 1)·T_tx(N)) + N_o·D_o + L.
- **T_kredi** (veri akışı) = N_o·(D_o + T_BLK) + (k_f + 1)(k_r + 1)·T_BLK + (k_r + 1)·T_min + 2L.
  - BLOCKED (A → B, k_f) ve (iii) yanıtı (B → A, k_r) iki ayrı zincirdir. Yanıt yalnız varan BLOCKED'e gelir (`06:243`) ve eş başına ≤ 1 / T_min'dir (`06:270`); bu yüzden çarpım gerçektir.
  - N_o·T_BLK, kesintinin bitişini içeren aralıktır (rev3.1'de eksikti).
- **T_ctrl(E)** — `06 §16 YD-9` m.5 biçimi: **T_ctrl(E) = T_ctrl(k, k′) + T_düzey2 + T_pencere + T_kredi,ctrl.**
  - **T_ctrl(k, k′)** = T_C + [k + N_o ≥ 1]·(Δ_lp(k′) + H_C + N·P_C + (k + N_o − 1)·(A + N·P_C)) + N_o·D_o + L.
    - Tek kontrol biriminin karşı uygulamaya varışıdır; bekleyen taraf yalnız varışı gözler, çarpım yoktur.
    - k, birimin yönünün tavanıdır; k′, birimin ACK'lerinin yönünün tavanıdır. FILE_RECEIPT (B → A): k = k_r, k′ = k_f. FILE_ABORT ve FLOW_RESET (A → B): k = k_f, k′ = k_r.
    - T_C = (S_iletimde + S_baş,C)/r + S_baş,C/(ρ_C·r) + τ (`06:212`); S_baş,C ≈ 50 B; ρ_C = 0,1 [CAL]; T_C = 51,6 s.
    - **P_C := T_C** [türetim]: `06:212` sınırı her katman-0 başına uygulanır, çünkü bir kontrol biriminin kopyası katman-0 işidir (`06` §5). A + P_C = 81,6 s.
    - H_C ≤ H: aynı öncüller (§2).
  - **T_düzey2** = [şeritte yarışan iş var]·(k + N_o)·D₂. Bu, yeniden gönderimin aynı şeritteki yeni DATA ile düzey-2 kuralı altındaki beklemesidir (`06:91`).
    - **05f okuması:** 05b'nin kopya dizisi (DATA ← ARQ ← DATA ← kopya, `05b:604`) kontrol kovasında D₂ ≤ 3·S_baş,C/(ρ_C·r) + (K_ARQ_SHARE + 4)·τ = 48,5 s verir, **iletim başına**.
    - K4G-06 taslağı K_ARQ_SHARE·S_baş,C/(ρ_C·r) = 64,5 s'dir ve tek terimdir. Fark §13'te DUR-05f-17 = DUR-06-27 satırında iki numarayla yazıldı.
    - Kapı hücrelerinde T_düzey2 = 0: şeritte tek birim vardır (N_live = 1, başka kayıt yok).
  - **T_pencere** = 0, P2 altında (`06:88`).
  - **T_kredi,ctrl** = 0, şerit kredisi tükenmemişse. Kapı hücrelerinde tek FILE_RECEIPT vardır ve akış-1 kredisi (W_c = 32 [CAL]) tükenmez.
    - Tükenmişse (13. FILE_* kaydı; `06:244-251`) bugünkü kuralla: N_o·D_o + (k_g + 1)(k_a + 1)·T_BLK + (k_g + 1)·T_min + 2L. k_g FILE_RECEIPT'in yönüdür (k_r), k_a A'nın ilan yönüdür (k_f).
    - R-KRD seçilirse toplanır biçim (`06:250`).
    - **R-KRD'nin 05f okuması** [türetim]: kesintisiz zarfta tutar. Koşulları: N_b ≥ k_a + 1; N_b·T_ACK ≤ `idle_hold` (12 × 8,41 = 100,9 s < 105 s); yinelemeler alıcı sunucu yuvasının kadans fırsatına biner ve yuva B'nin son varışıyla canlıdır.
    - Hücrenin k'siyle sıkı biçim: (k_a + 1)·T_ACK + L + N_o·(D_o + (k_g + 1)·T_BLK + (k_a + 1)·T_ACK + L). N_b ile yazılan biçim güvenli yönde bir üst sınırdır.
  - **Ön koşullar P1–P4** (§8.1). P1 tutmazsa (`Ordered`), öndeki kayıtların kurtarılması T_ctrl'ye eklenir ve bu sınır geçersizdir.
  - **W_D'ye girdi** (`06:89`): T_rec = T_ctrl(E) + T_düzey2.
    - rev3.2'de hıza bağlı olmayan kısım: β = [k + N_o ≥ 1]·((k + N_o − 1)·A + (min(k′, K_slot(0)) + 1)·T_ACK) + N_o·D_o. T_ACK sabit bir önselden gelir (`1b1425d2:receiver.rs:83-94`), hıza bağlı değildir.
    - D-3'te β = 423,6 s: rev5.2'nin 390 s'si ve Δ_lp'nin kadans payı 33,6 s.
    - Sonuç: W_D(31 B/s) = 148; W_D = 2048'in D-3 geçerliliği r ≤ ≈ 1,91 kB/s olur (rev5.2: ≈ 2,08 kB/s) [hesap]. Bkz. §12 ve §13.
- **T_R** = max(D_ack + T_G + T_ctrl(k_f, k_r), T_kredi + F_yeni).
  - İlk terim R4 sırasıdır: G'nin son sıfırlanması teyidin gönderene varışıdır (≤ D_ack); gönderen T_G'de vazgeçer, FLOW_RESET(0x04) T_ctrl içinde varır.
  - İkinci terim, G'nin durduğu kredi yenilemesi rejimidir (§4.3). Örüntü: alıcı yeni kredi ilan eder ve R çalışır; ilan kaybolur; gönderen bloklu kalır ve G durur; kredi T_kredi içinde varır; yeni birim F_yeni içinde kabul edilir.
  - İlk terim bu rejimi kapsamaz, bu yüzden ikinci terim max'ta kalır. Aşağıdaki hücrelerin hiçbirinde bağlayıcı değildir. rev3.1'in "doğrudan R-sessizliği sınırı" ifadesi kalktı.
- **T_A:** §7.

**[hesap]** 31 B/s, φ = 1, P1–P4. Kapı satırlarında N_live = 1 ve T_düzey2 = T_pencere = T_kredi,ctrl = 0. Değerler saniyedir ve gevşek üst sınırlardır:

| Hücre | N_o×D_o | k_f / k_r | m | n_tx | Δ_lp(k_r) | T_G | D_ack | F_yeni | T_kredi | T_ctrl(k_r, k_f) | T_ctrl(k_f, k_r) | T_R (1. terim / 2. terim) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| D-0 | 0 | 0 / 0 | 1 | 1 | — | 256,1 | 81,4 | 174,7 | 177,0 | 124,6 | 124,6 | **462,0** / 351,6 |
| D-1 | 1 × 60 s | 0 / 0 | 1 | 2 | 81,4 | 1 330,1 | 148,7 | 1 181,4 | 267,0 | 1 047,4 | 1 047,4 | **2 526,1** / 1 448,4 |
| D-2 | 1 × 60 s | 3 / 3 (= K_slot(60)) | 1 | 5 | 106,6 | 1 877,2 | 173,9 | 1 703,3 | 720,0 | 1 317,4 | 1 317,4 | **3 368,5** / 2 423,3 |
| D-3′ | 1 × 60 s | 3 / 4 (= K_slot(60) + 1) | 2 | 9 | 115,0 | 2 556,3 | 1 665,1 | 1 711,7 | 841,0 | 1 399,0 | 1 325,8 | **5 547,2** / 2 552,7 |
| D-3 | 1 × 60 s | 3 / 11 | 2 | 9 | 173,9 | 2 674,0 | 1 782,8 | 1 770,6 | 1 688,0 | 1 970,2 | 1 384,7 | **5 841,5** / 3 458,6 |
| *duyarlılık: D-2, N_live = 2* | 1 × 60 s | 3 / 3 | 1 | 5 | 106,6 | 2 419,5 | 173,9 | 2 245,6 | 720,0 | 1 523,8 | 1 523,8 | 4 117,2 / 2 965,5 |
| *duyarlılık: D-3, N_live = 2* | 1 × 60 s | 3 / 11 | 2 | 9 | 173,9 | 3 758,5 | 2 325,1 | 2 312,8 | 1 688,0 | 2 589,4 | 1 591,1 | 7 674,6 / 4 000,8 |

- Duyarlılık satırlarında T_düzey2 hariçtir; şeritte yarışan iş varsa (k + N_o)·D₂ eklenir.
- D-3′ artık çarpımsal rejimdedir (m = 2): k_r = 4 > K_slot(60) = 3. Toplanır rejimin kenarı D-2'dir.
- T_kredi,ctrl, şerit kredisi tükenmiş bir hücrede (kapı dışı; K4G-06 B30 (g)) bugünkü kuralla: 177,0 / 237,0 / 690,0 / 811,0 / 1 658,0 s. R-KRD'nin sıkı biçimiyle D-3'te 633,2 s.
- **Doğrulamanın karşı örnekleri yeni sınırların altında** (wf_c90086d9):
  - D_ack: D-1 148,7 s ≤ 148,7 s; D-2 173,9 s ≤ 173,9 s; D-3 1 620,9 s < 1 782,8 s; D-3′ 1 581 s < 1 665,1 s.
  - N_live = 2 örneği (D-3, u ve w): 2 090 s < 2 325,1 s.
  - rev3.1'in T_G karşı örnekleri (1 121–2 138 s) zaten altındaydı; yeni T_G'ler daha büyüktür.
- **Ürün sabitleri:** E_D'nin bir hücresinden, ürün kipi sınırlarıyla (T1′/T2′) ve ürünün N_live'ıyla hesaplanır. Fikstür değerleriyle hesaplanmaz (DUR-05f-13).

**PreConverge — P3 tutmadığında** [hesap; `593029eb:sender.rs:7583-7597`, `:7237-7241`, `:6709-6715`]:
- PreConverge'de kopyalar s_j ≤ c₁·(2^j − 1) + j·P anlarında gider: sonraki vade = gönderim + aralık, aralık ×2 ve tavansız. c₁ = 1,5·(S + 65)/31 s.
- P hizmet payıdır: kontrol birimi için P_C, dosya birimi için P_c. rev6 PAY kuralı PreConverge'de yoktur (`05b:29`); P burada üst sınır olarak alındı [çıkarım].
- Sahiplenme ve kayıp ilanı kapalıdır; tek kurtarma bu takvimdir.
- **Kontrol birimi** (S ≈ 50 B): c₁ = 5,56 s.
  - Oturum içi 60 s kesinti ilk dört iletimi yutar (0; 5,6; 16,7; 39,0 s) → w = 4.
  - Oturumsuz aralıkta (Faz A; FILE_RECEIPT yeni oturumda doğar) w = 0, ve + D_o.
  - T_ctrl,PC = T_C + s_n + [oturumsuz]·D_o + L, n = k + w.
- **Dosya birimi** (S = 1050 B): c₁ = 53,95 s. 60 s kesinti iki iletim yutar (w = 2). F_PC = T_D + s_n, n = k_f + w.

| Durum | n | PreConverge sınırı | Yakınsamış sınır | Kapsanıyor mu |
|---|---|---|---|---|
| FILE_RECEIPT, D-1, oturum içi kesinti | 4 | 414,5 s | T_ctrl 1 047,4 s | evet |
| FILE_RECEIPT, D-2, oturum içi | 7 | 1 192,5 s | 1 317,4 s | evet |
| FILE_RECEIPT, D-3′, oturum içi | 8 | 1 956,3 s | 1 399,0 s | **hayır** |
| FILE_RECEIPT, D-3, oturum içi | 15 | ≈ 1,8·10⁵ s | 1 970,2 s | **hayır** |
| FILE_RECEIPT, D-2, oturumsuz aralık | 3 | 378,3 s | 1 317,4 s | evet |
| FILE_RECEIPT, D-3′, oturumsuz aralık | 4 | 474,5 s | 1 399,0 s | evet |
| FILE_RECEIPT, D-3, oturumsuz aralık (Faz A yeniden bağlanma) | 11 | 12 142,7 s | 1 970,2 s | **hayır** |
| Yeni oturumun ilk dosya birimi, D-1 | 2 | 534,7 s | F(2) + D_o = 1 108,4 s | evet |
| Yeni oturumun ilk dosya birimi, D-2 / D-3 | 5 | 2 452,0 s | F(5) + D_o = 1 630,3 s | **hayır** |

- **Sonuç:** P3 tutmazsa D-3′ ve D-3'ün FILE_RECEIPT'i ile D-2 / D-3'te yeni oturumun ilk dosya birimi E_D'de sınırlı değildir.
  - Bu hücreler P3 ile kapıdadır.
  - P3'süz hâlleri (Faz A yeniden bağlanmasından sonraki FILE_RECEIPT, yeni oturumun ilk birimi) ürün kararına dek **E_D dışıdır** (DUR adayı; orkestratör sahibe götürüyor; `06:102`).
  - D-0…D-2'nin FILE_RECEIPT'i ve D-1'in ilk dosya birimi P3'süz de kapsanır.
- [çıkarım] Ürünün kuyruk-probu ve DR-CS-3 bootstrap'ı ek iletim ya da RTT örneği getirebilir; ikisi de E_D sınıfı değildir.
- [olgu] Doygun `ack_delay` (≥ 16,78 s) örnek vermez (`593029eb:sender.rs:3598-3607`, `:1601-1602`). Yalnız bayat SACK'lerin geçtiği bir desen motoru PreConverge'de tutar; P3 bu yüzden günlükten doğrulanır (Ek B.4).

**8.3 Vazgeçme eylemleri** — `06 §16 YD-4` (`06:436`) ile hizalı. Varyant adları DUR-05f-9 (a) kararıdır.

| Dolum | DUR-05f-18 = DUR-06-28 (a) seçilirse | (a) seçilmezse (rev4 kuralı) |
|---|---|---|
| **G (S1, Faz G)** | **Gönderen:** `reset_flow(f, 0x04 timeout)`; motor akışın birimlerini iptal eder (`06:255`, `:276`); **FILE_ABORT yok**. **Alıcı:** `FlowEnded { Reset { 0x04 } }` bant içi gelir (YD-10). R1 / R2'de `Aborted(timeout)`, R0'da olay yok. RESET transportta kendiliğinden R3'tür (YD-2). G dolumu FIN sunulduktan sonra olduysa RESET bayat olabilir; dosya alıcıda kalır (beyanlı ayrışma, DUR-05f-11) | `reset_flow(f)` + akış 1'e `FILE_ABORT(0x04)` |
| **G, oturum değişimiyle** | Faz G iptalinin (G dolumu ya da kullanıcı) RESET'i teyit edilmeden oturum değişirse yeni oturumdan `FILE_ABORT(kod)` gider; neden korunur (YD-4 3. satır; bugünkü `AMP:92` kuralının karşılığı). Alıcıda `Aborted(kod)` ya da erken iptal kaydı (YD-5) | aynı |
| **A (S2, Faz A)** | **Gönderen:** akış 1'e `FILE_ABORT(0x04 timeout)`. **Alıcı:** (i) aktarım bağlıysa `Aborted(timeout)` + `stop_flow` + `resolve_flow`; (ii) bağlı değilse erken iptal kaydı YD-5'in iki çapasıyla doğar (§9 son satır); (iii) iki çapa da boşsa kayıt açılmaz, `aborts_after_terminal` sayılır (tipik: alıcının bitirdiği aktarıma gelen A dolumu) | aynı |
| **R (alıcı, R1)** | **Alıcı:** `stop_flow(k)` + akış 1'e `FILE_RECEIPT(0x03 timeout)`; FILE_RECEIPT kabul edilince `resolve_flow` (YD-2). **Gönderen:** S1'de `SendFailed(ReceiverTimeout)` + **`reset_flow_local(k)`** (motor `CancelFlow`; **telde FLOW_RESET yok**, `06:436` R satırı; rev5'in `reset_flow(k, 0x00)` yanıtı kalktı). S2'de `SendFailed(ReceiverTimeout)` | akışı yerelde kapat + `FILE_RECEIPT(0x03)` |
| Her biri | Tek `SendFailed(LocallyAbandoned)` ya da `Failed(LocallyAbandoned)`. Yarışan iki sinyalde ilk işlenen kazanır (YD-4) | aynı |

- **R3 çağrı noktaları** (`06 §16 YD-2`, `06:402`; YD-9 m.11). `bk_flow_limit` yalnız R3'te ilerler; R3 iki yoldan gelir:
  1. RESET işlendi → transport kendiliğinden çözümler (G dolumu, Faz G iptali).
  2. Alıcı uygulaması `resolve_flow(k)` çağırır:
     - R dolumu: `FILE_RECEIPT(timeout)` akış 1'e kabul edilince (`send` → `Ok`);
     - A dolumu: FILE_ABORT uygulanınca, önce `stop_flow(k)`;
     - ret: reddedilen FILE_BEGIN'in alındısı kabul edilince;
     - bağsız akış (aktarımı bağlayan kayıt yok; ör. AMP S8 sessiz DROP'u): `stop_flow(k)` + `resolve_flow(k)`, alındı yok; bağsız akışta gelen `FlowEnded { Finished }` de çağrı noktasıdır.
  - **R3 akış başına en çok bir kez ateşler.** Çözümlenmiş akışta `resolve_flow` etkisiz `Ok` döner ve `resolve_noop` sayılır (ör. transport RESET'le çözümledikten sonra uygulamanın çağrısı).
- **Sonrası:**
  - Geç gelen `ok` → `receipts_ignored` (`1b1425d2:file_transfer.rs:1152`).
  - `stop_flow` sonrası gönderen BLOCKED kalır ve G durur (YD-7).
- **FILE_ABORT'un üç kullanımı** (DUR-05f-18 = DUR-06-28 (a) seçilirse; `06 §16 YD-9` m.3, A.18 `06:547`): (1) Faz A (A dolumu); (2) `superseded` (Faz G'de oturum değişimi); (3) RESET'i teyitsiz kalıp oturumu değişen Faz G iptali ya da G dolumu, nedeni korunarak. Faz G'de oturum içinde FILE_ABORT yoktur.
- **Ek A etkisi:** Ek A A.2 ve A.5'e bu üç kullanımla daralan (a) varyantı yazıldı; varyant DUR-05f-18 = DUR-06-28 kararıyla geçerli olur (Ek A).

## 9. Oturum geçişi

Ortak yaşam döngüsünün oturum satırları K4G-06'dadır (`06:261`; YD-4 son satır; YD-5).

| Durum | Gönderen | Alıcı |
|---|---|---|
| Faz G'de oturum değişti (FIN kabul edilmemiş) | Yeni oturumdan `FILE_ABORT(superseded)`, `SendFailed(Superseded)` (`1b1425d2:file_transfer.rs:1509-1515`; `AMP:92`; YD-4). Oturumsuz bir kesinti Faz G'de bu sonucu verir; E_D'de oturumsuz kesinti yalnız Faz A'dadır (§8.1) | `Aborted(superseded)` ya da erken iptal kaydı (YD-5) |
| Faz G iptali / G dolumu, RESET teyit edilmeden oturum değişti | Yeni oturumdan `FILE_ABORT(kod)`, kod korunur (YD-4 3. satır; DUR-05f-18 (a)) | `Aborted(kod)` ya da erken iptal kaydı |
| FIN kabul edilmiş, `floor ≤ fin_seq` iken oturum bitti | Faz A o anda başlar (`1b1425d2:transport.rs:45`; YD-1 S2'nin ikinci girişi). Yuva ≈ 1 s içinde ölür (`1b1425d2:runtime.rs:3428`, `:3154-3158`); teyit bir daha gelmez. **Faz A'nın FILE_RECEIPT'i yeni oturumdan gelir; o oturumun motoru P3'ü sağlamaz** (§8.1; sayılar §8.2 "PreConverge") | — |
| Alıcıda `Closed(h)`, akış eksik | — | Teslim artık gelmez (`1b1425d2:transport.rs:1069-1072`) → `Failed(SessionEnded)` + sıradaki oturumda `FILE_RECEIPT(0x03)` (yükümlülük taşınır, YD-6) |
| Faz A'da oturum değişti | Etkisiz (`1b1425d2:file_transfer.rs:1135-1149`) | Alındı yuvadan yeni oturuma gider |
| Yeniden bağlanma | Oturumsuz aralık N_o·D_o'ya yazılır (yalnız Faz A) | — |
| **Erken iptal kaydı (YD-5, `06:452`; YD-9 m.4)** | — | **Doğum:** eşten gelen FILE_ABORT(t) izlenen bir gelen aktarıma denk gelmezse, yalnız iki çapadan en az biri boş değilken: **(i) R0 çapası** — FILE_ABORT'un geldiği oturumda bağlanmamış (R0) gelen BK akışları; **(ii) eski oturum çapası** — eşin, FILE_ABORT'un oturumundan ESKİ ve kayıt anında `Closed` olmamış oturumları (bütün akışlarıyla, henüz açılmamışlar dahil). İkisi de boşsa kayıt açılmaz, `aborts_after_terminal` sayılır. Kural nedene bakmaz, yalnız oturum ilişkisine bakar. **Anahtar:** (eş, `transfer_id`). **Düşme:** FILE_BEGIN(t) okununca (→ `FILE_RECEIPT(rejected)`), ya da iki çapa da tükenince: (i)'nin akışlarının hepsi bağlandı ya da bitti, ve (ii)'nin oturumlarının hepsi `Closed` oldu. Zamanlayıcı yok. Tavan C_EA = 2·`bk_flow_limit0` + `MAX_OUTGOING_TRANSFERS` = 12 [CAL]; aşımda kayıt açılmaz, `early_abort_overflow` (rapor, kapı değil) |

- rev3.1'in "eşin kayıt anındaki bütün oturumları `Closed` olunca" kuralı kaydın kendi oturumunu bütünüyle sayıyordu ve sızıyordu; kalktı (Ek C.5). Örnek: A dolumu, alıcının bitirdiği aktarıma geldiğinde kayıt yeni oturum boyunca kalırdı (wf_c90086d9). YD-5'te kendi oturumu yalnız R0 akışlarıyla sayılır.
- rev5.1'in (ve 05f `5a46a07c`'nin) yalnız R0'a dayanan kuralı, eski oturumda henüz açılmamış taşıyıcı akışı kaçırıyordu; (ii) bunu kapatır (`06:452`).
- **İlgisiz R0:** bitmiş aktarıma gelen FILE_ABORT, ilgisiz bir R0 akışı varken kayıt açar; kayıt o akış bağlanınca düşer. Sınırlıdır, hayalet doğurmaz.
- **Test eşlemesi:** Ek B.5'in erken iptal kolları K4G-06 B31 (d)(e)(e′)(h)(h′)(i)(j) beklentileriyle eşlenir (`06:620`).

## 10. Bağımlılık ve sıra

- 05f ürün kodu yalnız K4G-06 ürün koduyla birlikte gelir (C16 v2 §5.4): 05d A1, alıcı kredisi, kontrol yükümlülükleri, iptal, FIN ve timeout tek yaşam döngüsündedir (`06 §16`).
- İkinci bir geçici oturum-geneli erişimci ya da pencere tasarımı üretilmez.
- **Karar takvimi — tek takvim** (`06 §16 YD-9` m.8, `06:508`; `06` §13 `:315`, `:333-334`):
  - **DUR-05f-17 = DUR-06-27 yönü:** K4G-06 deneyinden ÖNCE (B30 deneyde koşar).
  - **DUR-05f-14 = DUR-06-26** ve **DUR-05f-18 = DUR-06-28 yönü:** "K4G-05f ONAYLANDI"dan ÖNCE; deneyden sonra, CEVAP 17'de olabilir.
  - **Sayılar (DUR-05f-7, -13):** ürün kipi sınırları T1′/T2′'den (C16 v2 §1.2) SONRA.
- **Sıra:** #140 → 05g → 05c → DUR-06-27 yönü → K4G-06 deneyi (iki taban) → DUR-06-26/28 yönü → 05f onayı → T1′/T2′ → ürün sabitleri → ürün kodu (`06` §12 (ii), (iv)) → INT-02.

## 11. Süre (iş günü; tahmin; K4G-06 ürün kodunun içinde)

- Erişimci alanları + B.6: 0,5.
- Üç saat, fazlar, eylemler, oturum kuralları, erken iptal kaydı: 1–1,5.
- Tutma seçeneği: (a) 0,25 · (b) 0,5 (tel girdisi ve alıcı zamanlayıcısı K4G-06'da).
- Ek A + mevcut test değişiklikleri (B.7): 0,5–1.
- Yeni testler B.1–B.5 ve B.9, sınıf başına deterministik enjektör dahil: 1,5.
- kama-node / kama-mobile yüzeyi (K4G-06 aynasıyla tek pakette): 0,25.
- Ürün sabitlerinin T1′/T2′ ile yeniden hesabı: 0,25.
- **Toplam ≈ 4,25–5,5.** INT-02 kolu ayrı. rev3'ün 3,75–5'inden fark: sınıf başına enjektör ve sabitlerin yeniden hesabı.

## 12. K4G-06 ile hizalama (rev5.2 `24bc6314`, `06 §16 YD-9`)

K4G-06'nın önerileri sahip kararı değildir (DUR-06-26/27/28 açık). 05f bunlara koşullu bağlanır. İki notun bir seçeneği birlikte değiştirmesi gerektiğinde DUR satırına iki numara birlikte yazılır. İki not çelişirse karar verici sahiptir.

**Tek kaynak: `06 §16 YD-9`'un 12 satırı** (`06:497-514`). Her satır 05f rev3.2'de şu yerde uygulandı:

| # | Kural (YD-9) | 05f rev3.2'de | Koşul |
|---|---|---|---|
| 1 | DUR-06-26 = DUR-05f-14: öneri (a) N_h = 1, D_h = 30 s; (b) yalnız D > G_H; (b-i) yuva ömrü, (b-ii) fazla düşme ≤ min(D_tut,max, t_S2 − t_R2), t_S2 − t_R2 ≤ D_ack | §7 (başlık, (b-i), (b-ii), ölçüt ve öneri); §13; "bir ACK yolu" kalktı | DUR-05f-14 = DUR-06-26 |
| 2 | G dolumu `reset_flow(f, 0x04)`, FILE_ABORT yok; R dolumu `stop_flow` + FILE_RECEIPT(`timeout`), kabulünde `resolve_flow`, gönderende `reset_flow_local`; A dolumu FILE_ABORT(`timeout`), alıcıda `Aborted` + `stop_flow` + `resolve_flow` | §8.3 G / R / A satırları; `reset_flow(k, 0x00)` yok | DUR-05f-18 = DUR-06-28 (a) |
| 3 | FILE_ABORT'un üç kullanımı | §8.3 "G, oturum değişimiyle" satırı ve "üç kullanım" maddesi; §9 ikinci satır; Ek A A.2 / A.5 (a) varyantı | DUR-05f-18 = DUR-06-28 (a) |
| 4 | Erken iptal kaydı = YD-5 (iki çapa, `aborts_after_terminal`, C_EA) | §8.3 A satırı alıcı tarafı; §9 son satır; §1; Ek B.5 ↔ B31 (d)(e)(e′)(h)(h′)(i)(j) | YD-5 |
| 5 | T_ctrl(E) = T_ctrl(k) + T_düzey2 + T_pencere + T_kredi,ctrl; P1–P4 | §8.1 tablo (P1–P4, P3'ün kapsamı); §8.2 T_ctrl(E); §7 "W_D − 1 kayda dek (RECEIPT için W_D − 13)"; "A sunucusunun SACK'leri tavan gerekmez" satırı kalktı | DUR-05f-17 = DUR-06-27 (c) |
| 6 | A saatinin durma koşulu (kendi akış-1 hazır kuyruğunda > T_min okunmamış birim) | §7 A saati; §4.3 satırı | — |
| 7 | k sınıfları yöne göre | §8.1 k_f / k_r satırları; Ek B enjektörü | — |
| 8 | Tek DUR takvimi | §10 | — |
| 9 | Faz A girişi niteleyicisi | §5; §7; Ek A A.5 | — |
| 10 | A.36 yetkili alan listesi | §5, §6 | — |
| 11 | R3 çağrı noktaları; akış başına en çok bir kez | §8.3 "R3 çağrı noktaları" | YD-2 |
| 12 | B29 (i)/(i-mut) = 05f B.9 (a)'nın ayırt edici fikstür kolu | §7 son madde; Ek B.9 (a) | kayıpsız fikstür |

**05f'nin eklediği ya da farklı hesapladığı kalemler** (K4G-06 rev5.3'e; karar verici sahiptir):
1. **W_D'nin T_rec'i.** rev3.2'de T_ctrl(D-3) = 1 970,2 s (§8.2; çapa kayması ve B(D) düzeltmesiyle). Hıza bağlı olmayan kısım β = 423,6 s; rev5.2'nin β'sına (390 s) Δ_lp'nin kadans payı 33,6 s eklenir (T_ACK sabit önselden gelir). Sonuç [hesap]: W_D(31 B/s) = 148; W_D = 2048'in D-3 geçerliliği r ≤ ≈ 1,91 kB/s (rev5.2: ≈ 2,08 kB/s). Seçilen W_D değişmeyebilir; geçerlilik aralığı daralır. DUR-05f-17 = DUR-06-27 satırında iki sayı birlikte.
2. **T_düzey2.** 05f okuması iletim başına D₂ ≤ 3·S_baş,C/(ρ_C·r) + (K_ARQ_SHARE + 4)·τ = 48,5 s (05b kopya dizisi, `05b:604`). K4G-06 taslağı tek terim K_ARQ_SHARE·S_baş,C/(ρ_C·r) = 64,5 s. Biri iletim başına, öbürü tek; k + N_o ≥ 2'de 05f'nin toplamı daha büyüktür. DUR-05f-17 = DUR-06-27 satırında iki biçim birlikte.
3. **W_D'nin N_live'ı.** W_D(r, E) N_live = 1 ile yazılı. W_D'nin bağladığı rejimde (akış-1 seli) şeritte birden çok canlı birim vardır; her yeniden gönderim (N_live − 1)·P_C büyür (§8.2 duyarlılık). T_rec'in N_live'ı K4G-06'nın kalemidir.
4. **R-KRD.** 05f okumasıyla tutar (koşullar §8.2). Hücrenin k'siyle sıkı biçim N_b'li biçimden küçüktür; N_b'li biçim güvenli yönde.
5. **k sınıfları.** Üyelik yöne göre (m.7) aynen alındı. 05f tavanı tekrar zinciri başına uygular ve koşu semantiğini tanımlar (§8.1). Bu, k_f / k_r'nin sahibi olarak 05f'nin tanımıdır (`06:381-384`); çelişki değildir.
6. **PreConverge.** P3 aynen alındı. 05f, P3'süz hangi hücrelerin yine de sınırlı kaldığını hesapladı (§8.2 tablo): D-0…D-2'nin FILE_RECEIPT'i ve D-1'in ilk dosya birimi kapsanır; D-3′ / D-3 FILE_RECEIPT'i ve D-2 / D-3'te yeni oturumun ilk dosya birimi kapsanmaz. Bu, orkestratörün DUR adayına girdidir.
7. **Uyumlu, değişiklik yok:** H_C_tavan (`06:256-260`, YD-6), tek sonlandırma çıktısı (`06:276`), A.36 alan listesi (`06:565`), YD-5 iki çapa, YD-10 `FlowEnded`.

## 13. DUR listesi

**Sahip kararları (C16 v2 §3; aynen):**

| DUR | Karar | rev3.2'de |
|---|---|---|
| DUR-05f-6 | **(a)** — `FlowTxProgress` / `FlowRxProgress` K4G-06 akış bütçesi erişimcisinde (client-core somut tip + kama-app trait, S1 dışı; DUR-06-14 (a) ile tutarlı) | §5, §6; A.36 `06:565` |
| DUR-05f-7 | **(a) yön GO; §8.2 sayıları ONAYLI DEĞİL** — rev3'te yeniden türetim (çarpan + gerçek ACK takvimi). "Sonsuza kadar açık kalmasın" (C14 §6) ilkesi aynen | §8.2 rev3.2'de yeniden türetildi → **onay bekliyor**. C14 §6: tavanlı (b) ve (a) sağlar; tavansız (b) sahip istisnası isterdi (§7) |
| DUR-05f-8 | ε = 10⁻³, T_işlem ≤ 1 s yalnız [CAL] deney değerleri. **D_tük = 0 REDDEDİLDİ.** İki ayrı sözleşme: (i) deterministik kabul zarfı, (ii) rastgele kayıp hücreleri | §7 → DUR-05f-14; §8.1 |
| DUR-05f-9 | **(a)** `LocallyAbandoned` / `SessionEnded`; UniFFI aynası K4G-06'nın ayna değişiklikleriyle TEK seferde | §8.3, §9; A.37 `06:566` |
| DUR-05f-10 | Ek A biçimi kabul; onay "K4G-05f ONAYLANDI" ile, rev3'ten sonra | Ek A değişmedi; bekleyen iki değişiklik §0'da |
| DUR-05f-11 | Beyan kabul (yanlış "başarısız" mümkün, yanlış "teslim edildi" asla; vazgeçmeden sonra gelen `ok` yok sayılır), D_tük cümlesi hariç | §8.3 |
| DUR-05f-12 | **(a)** iki kol; kapı yalnız deterministik hücrelerde, rastgele kayıp hücreleri rapor | §8.1, Ek B.8; p = 0 B22 hücreleri kapıda |
| DUR-05f-5 | beyan | — |

**Açık ve yeni:**

| DUR | Tür | Soru · seçenekler | Öneri |
|---|---|---|---|
| **DUR-05f-7 (sayılar)** | onay | §8.2 rev3.2 türetimi: P1–P4 ve N_live; ileri zincir ve çapa kayması Δ_lp; B(D) = ⌊`idle_hold`/T_ACK⌋ − ⌈D/T_ACK⌉ ve m; D_ack; T_ctrl(E); T_R; D-0…D-3 tablosu; PreConverge tablosu | onay |
| **DUR-05f-13** | yeni sayı / sözleşme | **E_D:** hücreler D-0, D-1, D-2, D-3′, D-3. Girdiler: k_f ∈ {0, 3}; k_r ∈ {0, 3, 4, 11} — yöne göre (YD-9 m.7), tekrar zinciri başına, koşu kesintinin üstünden ve yuva / oturum boyunca sürer (§8.1); N_o ∈ {0, 1}, toplam, iki yön birden; oturumsuz kesinti yalnız Faz A; D_o = 60 s; d = 200 ms; φ = 1; **N_live = 1** (kapı fikstürü; duyarlılık 2); **P1–P4** (P3: iki yönün motoru ısınmayla yakınsamış); N_h = 1, D_h = 30 s ya da D_tut,max ≥ 30 s; SACK kapsam koşulu. **Ürün sabitleri:** (a) D-3 girdileriyle (kesintili turda m = 2; en geniş) · (a′) D-2 ile (k_r = K_slot(60) = 3; toplanır rejimin kenarı; rev3.1'de bu kenar D-3′ yazılıyordu, B(D) bir fazlaydı). İki durumda da r_min ve φ_min'de, **ürün kipi sınırlarıyla (T1′/T2′) ve ürünün N_live'ıyla**; fikstür değerleriyle değil; ölçülen hızla küçültülmez · (b) ölçülen hıza uyarlama (C15 v3 §5 (i) ile çelişir). **P3'süz hâller** (Faz A yeniden bağlanmasından sonraki FILE_RECEIPT; yeni oturumun ilk birimi) ürün kararına dek E_D dışıdır; ürün tarafı (PreConverge kopya aralığı) orkestratörün DUR adayıdır (§8.2) | (a); T1′/T2′ ve ürün N_live'ı gelince yeniden hesap |
| **DUR-05f-14 = DUR-06-26** | sözleşme | Faz A tüketici tutması (§7): (a) T_A = T_işlem + N_h·D_h + T_ctrl(E) · (b) `HELD` (birikimli, tavanlı): T_A = T_işlem + T_ctrl(E) + G_H; G_H = T_min + (k_r + 1)(T_BLK + T_min) + N_o·D_o + L; iki ek koşul: (b-i) HELD emisyonu B'nin sunucu yuvasını canlı tutar ya da A Faz A boyunca canlı tutma gönderir, yoksa D_tut,max ≤ yuva ömrü (≈ 105 s); (b-ii) fazla düşme ≤ min(D_tut,max, t_S2 − t_R2), t_S2 − t_R2 ≤ D_ack. Bedeller eşit ölçütle §7'de | **(a), D_h = 30 s [CAL]; (b) yalnız D > G_H gerekirse** (K4G-06 önerisiyle aynı, `06:431`; YD-9 m.1). Karar sahibindir |
| **DUR-05f-15** | beyanlı sınır | Yuva penceresi: B(D) = ⌊`idle_hold`/T_ACK⌋ − ⌈D/T_ACK⌉ (K_slot(0) = 11, K_slot(60) = 3); k_r + 1 > B₁ ise çarpımsal m tur; oturum kapanınca yuva ≈ 1 s. (a) beyan + E_D'nin k_r'si buna göre seçilir (D-2 kenar, D-3′ çarpımsal) · (b) teyitsiz veri varken yuvayı canlı tutan bir kural (K4G-06 / runtime) | (a); (b) K4G-06 için açık soru olarak kalır (yuva canlılığı (b-i) ile aynı ailedendir, §7) |
| **DUR-05f-16** | test amacı | Rastgele hücre raporu: ε, N_akt, N_fırsat,s (zincir konumu), birleşim üst sınırı (vazgeçme tahmini değil), gözlenen vazgeçme oranı; hüküm yok | kabul |
| **DUR-05f-17 = DUR-06-27** | sözleşme | Kontrol şeridi kipi ve T_ctrl: (c) akış 0 / 1 `Independent` → T_ctrl(E) §8.2 geçerli, ön koşullar P1–P4 · (a) / (b) `Ordered` → öndeki kayıtların kurtarılması T_ctrl'ye eklenir; 05f sınır vermez. Kapı ön koşulu (B.4, B.8 (i)). **İki notun farklı sayıları — birlikte, karar verici sahip:** (1) W_D = 2048'in D-3 geçerliliği: DUR-06-27 / K4G-06 rev5.2 r ≤ ≈ 2,08 kB/s (05f rev3.1 T_ctrl'si ile) · DUR-05f-17 / 05f rev3.2 r ≤ ≈ 1,91 kB/s (T_ctrl(D-3) = 1 970,2 s, β = 423,6 s). (2) T_düzey2: DUR-06-27 tek terim K_ARQ_SHARE·S_baş,C/(ρ_C·r) = 64,5 s · DUR-05f-17 iletim başına 3·S_baş,C/(ρ_C·r) + (K_ARQ_SHARE + 4)·τ = 48,5 s. (3) W_D'nin T_rec'i: DUR-06-27 N_live = 1 · DUR-05f-17 bağlayıcı rejimde N_live > 1 (§12 m.3) | (c) (K4G-06 önerisiyle aynı); yön deneyden ÖNCE (`06:508`) |
| **DUR-05f-18 = DUR-06-28** | sözleşme | Faz G iptali ve dolumlar: (a) G dolumu `reset_flow(f, 0x04)`, FILE_ABORT yok; R dolumu `stop_flow` + FILE_RECEIPT(`timeout`), gönderende `reset_flow_local`, telde RESET yok; A dolumu FILE_ABORT(`timeout`); FILE_ABORT'un üç kullanımı (Faz A; `superseded`; RESET'i teyitsiz kalıp oturumu değişen Faz G iptali ya da G dolumu); Ek A A.2 / A.5 (a) varyantı · (b) rev4 kuralı (`reset_flow` + FILE_ABORT akış 1'de) | (a) (K4G-06 önerisiyle aynı) |
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

Kapı yalnız deterministik hücrelerdedir (DUR-05f-12 (a)). **Enjektör:**
- Her ileti sınıfına ayrı bir deterministik ardışık kayıp deseni uygular (§8.1 tablosu):
  - A → B: dosya birimi · akış 0 / 1 kontrol · BLOCKED;
  - B → A: SACK · FLOW_CREDIT / HELD · FILE_RECEIPT.
- İstemci ve sunucu çıkışları ayrı kapılardır (`EgressPolicy`).
- Kesinti iki yönde, bütün sınıflara uygulanır.
- netem'in rastgele kaybı yalnız B.8 (ii) raporundadır.
- **Kapı ön koşulları:** DUR-05f-17 (c), yani akış 0 / 1 `Independent`; SACK kapsam koşulu (§8.1).

**B.1 Zorunlu tuzak** (client-core, `test-util`) — aynen:
- Store'da akışın birimi yok; FIN dahil son kayıtlar offer kanalında ya da yerel tamponda.
- İddia: `floor ≤ fin_seq` ve Faz A başlamaz.
- Serbest bırakılıp teyit edilince `floor = fin_seq + 1`.
- Mutant `floor = accepted` → kırmızı.

**B.2 31 B/s, 150 KiB, sahte saat** (kama-app, A+ sahte `06:582`; D-0):
- İddialar: vazgeçme ve terminal hata yok; alıcıda `Received`, baytlar özdeş; gönderende `Delivered`.
- Alt kol: iki parça arasına tek bir tam boy TEXT girer (≈ 67,7 s) → vazgeçme yok.
- Eski kod (`1b1425d2`) kırmızı (kanca).

**B.3 Yerel vazgeçme — E_D dışı:**
- (a) İleri yol T_G + bir turdan uzun ölü → [T_G, T_G + tur] içinde tam bir `SendFailed(LocallyAbandoned)`. Sinyal DUR-05f-18 kararına göre:
  - (a) seçilirse akış 0'da `FLOW_RESET(0x04)` ve alıcıda `Aborted(timeout)`;
  - (b) seçilirse akış 1'de `FILE_ABORT(0x04)`.
  - Her iki durumda `reset_flow` sonrası yeniden gönderim 0.
- (b) Yol açılmaz → alıcı T_R'de `stop_flow` + `FILE_RECEIPT(0x03)` + `Failed(LocallyAbandoned)`. Gelen birimler ACK'lenip atılır.
- (c) Faz A'da FILE_RECEIPT kalıcı kayıp → T_A'da `FILE_ABORT(0x04)`. Geç `ok` → `receipts_ignored`.

**B.4 KAPI — E_D içinde yanlış vazgeçme yok.** D-0, D-1, D-2, D-3′ ve D-3 hücrelerinde, her desen sınırın hemen altında. İddia her kolda vazgeçme = 0 ve tamamlanma:
- (a) **İleri zincir:** x₁ kesintiye düşer, sonra k_f = 3 ardışık kayıp (§8.2 Adım 1; doğrulamanın D-2 deseni). Varış x₅'te.
- (b) **Kesinti + SACK, toplanır kenar** (D-3′): varışın hemen ardından 60 s kesinti, sonra k_r = 4 ardışık SACK kaybı. Teyit aynı yuvada gelir.
- (c) **Kesinti + SACK, çarpımsal** (D-3): 60 s kesinti, sonra k_r = 11 SACK kaybı. Yuva ölür; ikinci ileri tur ve yeni yuvada teyit (m = 2; doğrulamanın 2 066–2 138 s deseni). Ölçülen teyit anı T_G'nin altında.
- (d) **İki yönün etkileşimi** (C16 v2 §3 örneği, k_f = k_r = 1, kesintisiz): yuva canlıyken toplanır.
- (e) **Tek eksik eski birim:** u k_f kez düşer, u'dan büyükler teyit edilir. `confirmed` artar, `floor` donar; alıcıda `received` artar.
- (f) **Bayat ACK:** aynı SACK durumu tekrarlanır → `confirmed` sabit. Pozitif kontrol: gerçek durgunluk sınırı aşınca tam bir kez vazgeçme.
- (g) **Faz G'de kredi bloklu + tüketici tutuyor** (≥ 2·T_G) → G ve R durur.
- (h) **Faz A'da FILE_RECEIPT k_r kez düşer** + 60 s kesinti → T_A içinde `Delivered`.
- (i) **Kredi yenilemesi:** BLOCKED k_f kez, FLOW_CREDIT k_r kez düşer; ilk BLOCKED T_min içinde gelir → R vazgeçmez (T_R'nin ikinci terimi).

**B.4′ E_D dışı, çarpımsal — RAPOR:**
- D-3 sabitleriyle 60 s kesinti + k_r = 17 → m = 1 + ⌈(18 − 5)/12⌉ = 3.
- Formülün m = 3 değeri ≈ 101,68 + 865,36 + 11·165,56 + 120 + 100,92 + 145,96 ≈ 3 155 s > T_G(D-3) [hesap] → vazgeçme beklenir.
- Ölçülen ile formül yan yana yazılır.

**B.5 Oturum geçişi** (§9'un her satırı):
- c1 FIN sonrası supersede → Faz A.
- c2 alıcı `Closed`'da eksik → `SessionEnded` + `FILE_RECEIPT(0x03)` → `ReceiverTimeout`.
- c3 kapanış ve yeniden bağlanma.
- c4 FIN öncesi supersede → `Superseded`.
- c5 erken iptal kaydı: FILE_BEGIN teslimiyle ya da kaydın kendi oturumu dahil bütün oturumlar `Closed` olunca düşer (YD-5).
- c6 oturum kapanınca alıcı yuvası ≈ 1 s içinde emekliye ayrılır; sonraki varış yeni `Receiver` açar.

**B.6 Erişimci ve takvim girdileri** (birim testleri; koda değil, Ek B'ye öneri):
- **Erişimci:** `confirmed` birim başına tam bir kez artar; İptal sayılmaz. `received` sırasız kabulde artar. Kapanınca `None`.
- **Pin testi (yeni):** ürün profilinde FLOOR + `ACK_TICK` ≤ 8,41 s, `idle_hold` = 105 s, `PATIENCE_TICK` = 1 s. K_slot ve kapı sayıları bunlara bağlıdır (`receiver.rs:83-94`; `runtime.rs:69`, `:71`, `:270`).
- **Kadans aralığı (yeni):** tek varıştan sonra, yeni varış yokken, ardışık SACK aralığı ≤ FLOOR + tick. Mevcut test (`receiver.rs:624-647`) kadansı sınar ama aralık iddiası yoktur.
- **Yuva ömrü:** son varıştan `idle_hold` sonra ve oturum kapanınca ≈ 1 s sonra emeklilik (`runtime.rs:3428-3430`).

**B.7 Değişecek mevcut testler** (DUR-8 biçimi) — aynen:
- `1b1425d2:crates/kama-app/tests/file_transfer.rs`'te 11 test;
- `crates/kama-node/src/node/tests.rs:529-602`, `:678-707`;
- `file_transfer.rs:2523-2598`, `:2645-2689`.

**B.8 INT-02 kolu** (05e; DUR-05f-12 (a)):
- **(i) KAPI — deterministik:**
  - 31 B/s, 150 KiB, kayıpsız (D-0; ≈ 88 dk tel baytıyla; C14 §6);
  - B22'nin p = 0 hücreleri: 0/0/yok ve 0/0/1 × 60 s (`06:611`);
  - D-1, D-2, D-3′, D-3 desenleri enjektörle.
  - Hüküm: tamamlanma; G / R / A vazgeçme = 0; en uzun G ve R aralığı §8.2 sınırının altında.
- **(ii) RAPOR — rastgele:** B22'nin kayıplı hücreleri (%10 / %80, `06:611`), 10 KiB.
  - Sütunlar: ε = 10⁻³; N_akt; ölçülen N_fırsat,s; birleşim üst sınırı Σ_s N_fırsat,s·p_s^(k_s+1) (E_D'den çıkma olasılığı; vazgeçme tahmini değil); gözlenen vazgeçme oranı.
  - Hüküm yok.
- **İki kolda da:** FILE_RECEIPT gecikmesi C15 v3 §5 (i) tanımıyla (doğum → ilk emisyon; ACK ve teslim ayrı; kesintili ve kesintisiz kol ayrı); 120 s yalnız karşılaştırmadır.

**B.9 Faz A tüketici tutması** — DUR-05f-14 = DUR-06-26; K4G-06 B29 (`06:618`). **En kötü sıra:** tutma S2'nin hemen başında başlar; 60 s kesinti tutmanın içinde; ilk girdiler kaybolur.
- **(a) seçilirse:**
  - tutma ≤ D_h → vazgeçme yok;
  - D_h < tutma ≤ D_h + T_ctrl → vazgeçme **mümkün**, iddia edilmez;
  - tutma > D_h + T_ctrl → tam bir kez vazgeçme.
- **(b) seçilirse** (`HELD`):
  - ilk k_r girdi kaybolur, her girdi T_min hız sınırına takılır, son girdi uçuşta → tutma ≤ D_tut,max iken vazgeçme yok (G_H payı; §7);
  - tutma > D_tut,max + G_H + T_ctrl → tam bir kez vazgeçme;
  - tutma sırasında oturum biter → girdi kesilir, A sürer → tam bir kez vazgeçme.
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

### C.3 — rev2 (`3e0ec8fa`): gövdeden çıkarılıp arşivlenmemiş kalan metinler

> **MUST NOT IMPLEMENT.** CEVAP 16 v2 §3 ile geçersiz.
>
> - R4 içindeki D_tük → §4.2 R4, §7.
> - §4.3 D_tük satırı → §4.3.
> - §7 "Sıfırlanmaz ve durmaz" → §7 (b).
> - DUR-05f-12 "(a); kapı" → kapı yalnız deterministik hücrelerde.
> - B.8 rastgele hücre kapısı → B.8 (ii) rapor.
>
> Kaynak satırlar `3e0ec8fa:docs/design/K4G-05f-DOSYA-ZAMAN-ASIMI.md:95`, `:107`, `:157`, `:267`, `:368-377`; atıflar rev2'nin kısa adlarıyladır. Aşağıda metin aynen, üstü çizili.
>
> - ~~**R4 — her bekleme tek saatte, saatler sıralı.** T_R = T_G + T_ctrl: zarf dışında gönderen önce vazgeçer, mesajı alıcıya T_ctrl içinde ulaşır. T_A ≥ T_işlem + D_tük + T_ctrl.~~
>
> ~~| Alıcı tüketicisi tutuyor | evet | R durur | D_tük (yalnız A'da) |~~
>
> - ~~**A saati:** Faz A'nın başında başlar. Sıfırlanmaz ve durmaz; oturumsuz aralık zarfın kesinti bütçesinden sayılır.~~
>
> ~~| **DUR-05f-12** | test amacı | INT-02: (a) iki kol — 150 KiB kayıpsız (C14 §6) + B22'nin kayıplı hücreleri 10 KiB · (b) tek boy. Ayrıca "zarf içinde vazgeçme = 0" kapı mı, rapor mu | (a); kapı |~~
>
> ~~**B.8 INT-02 kolu** (05e; DUR-05f-12):~~
> - ~~(i) Gerçek 31 B/s, 150 KiB, kayıpsız (≈ 88 dk tel baytıyla; C14 §6).~~
> - ~~(ii) B22 hücreleri (`06:323`), 10 KiB.~~
> - ~~Sütunlar:~~
>   - ~~tamamlanma ve süre;~~
>   - ~~G / R / A vazgeçme sayısı (zarf içinde 0 olmalı);~~
>   - ~~en uzun G ve R aralığı (§8.2 sınırının altında);~~
>   - ~~C15 v3 §5 (i) tanımıyla FILE_RECEIPT gecikmesi: doğum → ilk emisyon; ACK ve karşı uygulamaya teslim ayrı; kesintili ve kesintisiz kol ayrı; 120 s yalnız karşılaştırma.~~
> - ~~Sonuç "ölçüldü" diye yazılır.~~
>

### C.4 — rev3 (`b96f7d6d`): rev3.1'de yerine geçen metinler

> **MUST NOT IMPLEMENT.** wf_1699fc80 doğrulamasıyla çürütüldü.
>
> - §2: "tekrar varışında ACK zorlanır" (üründe erişilemez), koşulsuz block0 ifadesi, "son aday bunları yeniden üretti".
> - §7: (b)'de T_tutma = 0, eksik bedel tablosu.
> - §8.1–§8.2:
>   - T_baş = H_maks bir üst sınır değildi;
>   - K_slot kesintiyi hesaba katmıyordu;
>   - D_ack kesinti ve yuva ölümü terimi içermiyordu;
>   - k_f / k_r kontrol sınıflarını kapsamıyordu;
>   - D-3 m = 1 sanılıyordu.
> - §9: erken iptal kaydının düşme kuralı eksikti.
>
> Atıflar rev3'ün kısa adlarıyladır: `06` = `1f0e032c`. Aşağıda metin aynen, üstü çizili.
>
>   - ~~Tekrar (kopya) varışında ACK zorlanır (`:222-230`).~~
> - ~~**SACK kapsamı:** SACK her zaman en yüksek blok olan block0'ı taşır; `MAX_ACK_RANGES = 16` kırpması yalnız alt blokları keser (`ack.rs:36`, `:121-132`). En yeni varışın teyidi bu kırpmayla kaybolmaz.~~
> ~~**05b rev6 sınırları** [türetim, `05b`]: A = RTO_MAX = 30 s (`:590`) · yeniden vadeye girme ≤ A, H_maks (T2) = 729,8 s (`:595`) · T_birim (`:605`) · T_max(DATA) = 101,68 s, T_max(kopya) = P_c = 135,56 s (`:612-615`) · L = 72,98 s (`:619`). Değerler 31 B/s içindir. Son aday bunları yeniden üretti (C16 v2 dayanak satırı).~~
>
> ~~**7. Alındı bekleme (A saati) ve tüketici tutması — tek yaşam döngüsüne bağlı**~~
>
> - ~~**Yer:** FILE_RECEIPT akış 1'de gider ve yuvası düşürülmez (`06:82`, A.21 `06:266`). Kontrol başı toplu verinin arkasında beklemez (`06:114`).~~
> - ~~**A saati:** Faz A'nın başında başlar; oturumsuz aralık N_o·D_o'dan sayılır. Dolum = T_A(E_D) → §8.3.~~
> - ~~**Tüketici tutması (C16 v2 §3, DUR-05f-8):** FIN dahil bütün birimler teyitliyken alıcı tüketicisi akışı tutarsa FILE_RECEIPT doğmaz. Faz G'deki tutma sorun değildir: kredi bloklu olduğu için G durur (`06:305` B1). Faz A için iki seçenek; ortak yaşam döngüsü bölümünün sahibi K4G-06'dır (DUR-06-26, `06:574`; B29, `06:335`):~~
>
> ~~| | (a) Tutma zarfta: N_h × D_h, D_h ≥ 30 s | (b) "tutuluyor" sinyali A saatini durdurur (`06:148`, `:574`) |~~
> ~~|---|---|---|~~
> ~~| Mekanizma | T_A'ya T_tutma = N_h·D_h eklenir; yeni girdi, yeni sinyal yok | Alıcı, tutulan ve FIN'i almış akış için FLOW_CREDIT'e sabit boylu girdi koyar, aralık T_BLK. Girdi tazeyken A durur |~~
> ~~| Tel | değişmez | **değişir:** yeni girdi türü (A.8, `06:253`) |~~
> ~~| Yeni sayı | N_h, D_h [CAL]; INT-02 `ek-tuketici-durak` 30 s'yi kapsamak için D_h ≥ 30 s | Tazelik penceresi T_taze ≥ (k_r + 1)·T_BLK + L. k_r'den türer; yeni sayı değildir. Girdi dönüşte k_r kez üst üste kaybolabilir |~~
> ~~| Bant | yok | 31 B/s'de tutma süresince ≈ 39 B / T_BLK ≈ 1,3 B/s [hesap, `06:574`] |~~
> ~~| D_h'yi aşan tutma | Zarf dışıdır. Gönderen vazgeçer, `FILE_ABORT(timeout)` gider; alıcı **bütün veriye sahip olduğu hâlde** aktarımı iptal eder (yanlış "başarısız"; DUR-05f-11) | Zarf dışı değildir. Tutan eş sinyal verdikçe aktarım açık kalır; gönderende payload yoktur, yalnız aktarım kaydı bekler. Eş susarsa ya da oturum biterse A sürer ve sonludur |~~
> ~~| "Sonsuza kadar açık kalmasın" (C14 §6) | sağlar | bilerek tutan eş için sağlamaz (beyan gerekir); kaybolan eş için sağlar |~~
> ~~| Test | B29 (iii): D_h altı ve üstü kollar | B29 (i)–(ii) |~~
>
> ~~**05f tarafının görüşü:**~~
> - ~~(b), R1 ("kapasite beklemesi saate girmez") ile tutarlıdır ve D_h aşımında bütün veriye sahip bir alıcının aktarımı kaybetmesini önler.~~
> - ~~Bedelleri: tel değişikliği ve "bilerek tutan eş" beyanı.~~
> - ~~(a) tel değiştirmez ama yeni bir sayı ve veri kaybettiren bir aşım davranışı getirir.~~
> - ~~Karar tek yaşam döngüsünde, DUR-05f-14 = DUR-06-26 olarak verilir. **Saat o karardan önce kodlanmaz** (C16 v2 §3).~~
>
> ~~**8. Yerel vazgeçme politikası — iki sözleşme ve türetim**~~
>
> ~~**8.1 İki sözleşme.**~~
>
> ~~**(i) Deterministik kabul zarfı E_D — KAPI** (DUR-05f-12 (a), DUR-05f-13).~~
>
> ~~Girdiler:~~
>
> ~~| Girdi | Anlam |~~
> ~~|---|---|~~
> ~~| r_min | Pacer taban hızı: 31 B/s (`1b1425d2:crates/adaptive-transport/src/sender.rs:1485`) |~~
> ~~| φ_min | Sunulan yük altında akışın hizmet payı tabanı (`06` §5; sayı DUR-06-3). Tek dosya hücrelerinde φ = 1 |~~
> ~~| N_o, D_o | Kesinti sayısı ve azami süresi, iki yön. Oturumsuz aralık da kesintidir. D_o < `idle_hold` |~~
> ~~| k_f | İleri yolda akışın teyitsiz birimlerinin **ardışık** kaybolan iletim sayısının tavanı |~~
> ~~| k_r | Dönüş yolunda **ardışık** kaybolan SACK emisyonu sayısının tavanı |~~
> ~~| Tutma | DUR-05f-14 (a)'da N_h, D_h |~~
>
> ~~Kayıp modeli: kesinti dışındaki kayıplar yalnız bu ardışık tavanlarla sınırlıdır. Sözleşme: **E_D içinde G, R ve A saatlerinin hiçbiri dolmaz → yerel vazgeçme = 0.** Kapı Ek B.4 ve B.8 (i)'dir. Deterministik kayıp deseni `EgressPolicy` gibi bir örüntü enjektörüyle kurulur; rastgele değildir.~~
>
> ~~**(ii) Rastgele kayıp hücreleri — RAPOR** (B22 kayıplı hücreleri, `06:328`).~~
> - ~~Ürün aynı sabitlerle koşar (E_D'den türetilmiş, §8.2).~~
> - ~~Gözlenen G / R / A vazgeçme sayısı raporlanır. Deterministik garanti YAZILMAZ.~~
> - ~~Karşılaştırma referansı: bağımsız Bernoulli kaybı varsayımıyla, bir ardışık kayıp koşusunun k'yı aşma olasılığı ≈ N_fırsat · p^(k+1).~~
> - ~~Varsayımlar açıktır:~~
>   - ~~kayıpların bağımsızlığı (netem rastgele kaybı);~~
>   - ~~N_fırsat: ileride iletim sayısı, dönüşte SACK emisyon sayısıdır. Ölçülür, tahmin edilmez.~~
> - ~~ε = 10⁻³ [CAL] yalnız bu raporun eşiğidir (DUR-05f-8).~~
> - ~~[hesap] D-3 girdileriyle (k_f = 3, k_r = 11):~~
>   - ~~p_f = %10'da 0,1⁴ = 10⁻⁴ / ileri koşu;~~
>   - ~~p_r = %80'de 0,8¹² ≈ 0,069 / dönüş koşusu.~~
>   - ~~Yani %80 dönüş kaybı hücresi E_D'nin dışındadır ve vazgeçme olabilir. Bu bir rapor kalemidir, kapı değildir.~~
>
> ~~**8.2 Türetim** [türetim; koddan önce; ölçülmedi].~~
>
> ~~**Neden rev2'nin çarpanı yanlıştı (C16 v2 §3):**~~
> - ~~Teyit yalnız varışla tetiklenen ACK'le gelseydi, her dönüş kaybı yeni bir ileri deneme koşusu açardı.~~
> - ~~k_f = k_r = 1 örneği: veri kaybolur, ikinci deneme ulaşır, ACK'i kaybolur; üçüncü veri kaybolur, dördüncü veri ve ACK'i ulaşır. Yani 4 ileri deneme gerekir, 3 değil.~~
> - ~~En kötü çarpan (k_f + 1)(k_r + 1)'dir; `k_f + k_r + 1` bundan çıkmaz.~~
>
> ~~**Gerçek ACK takvimi çarpımı kırar, ama yalnız yuva penceresi içinde** (§2 ACK takvimi):~~
> 1. ~~**İleri yol.** Akışta teyitsiz birim varken bir teyitsiz birim en geç T_tx = A + P_c'de yeniden iletilir. ACK sessizliğinden sonraki ilk sahiplenme H_maks'tır (`05b:595`, `:605`). En çok k_f ardışık iletim kaybolur, dolayısıyla ≤ H_maks + k_f·T_tx içinde bir teyitsiz birim u varır.~~
> 2. ~~**Dönüş yolu.** u'nun varışından sonra alıcı, yeni varış beklemeden en geç T_ACK'de bir SACK yayar (kadans FLOOR). Bu SACK block0'da u'yu taşır. En çok k_r ardışık SACK kaybolur → (k_r + 1)·T_ACK + L içinde u teyit edilir. Bu, **varışla ilgisizdir**; ileri yolda yeni deneme gerekmez.~~
> 3. ~~**Yuva penceresi.** Periyodik SACK son varıştan en çok `idle_hold` = 105 s sürer; sonra yuva emekliye ayrılır ve SACK durumu kaybolur (§2). 31 B/s'de T_tx = 165,56 s > 105 s olduğu için yuvayı yeniden göndermeler canlı tutamaz.~~
>    - ~~**K_slot** = ⌊`idle_hold` / T_ACK⌋ − 1 = ⌊105 / 8,41⌋ − 1 = **11** [hesap].~~
>    - ~~k_r ≤ K_slot → iki yön toplanır.~~
>    - ~~k_r > K_slot → her tur bir ileri varış ister: m = ⌈(k_r + 1) / (K_slot + 1)⌉ tur, her tur ≤ k_f + 1 ileri iletim. Yuva penceresi olmasaydı (K_slot = 0) bu ifade incelemedeki (k_f + 1)(k_r + 1)'e eşit olurdu.~~
>
> ~~**Sınırlar:**~~
> - ~~**T_G(E_D)** = N_o·D_o + T_baş + (m·(k_f + 1) − 1)·T_tx + (min(k_r, K_slot) + 1)·T_ACK + 2L.~~
>   - ~~T_baş = T_max(DATA) eğer k_f = 0 ∧ N_o = 0 ∧ k_r ≤ K_slot; aksi hâlde H_maks.~~
>   - ~~m = ⌈(k_r + 1) / (K_slot + 1)⌉.~~
> - ~~**D_ack(E_D)** = (min(k_r, K_slot) + 1)·T_ACK + L: kabul anından teyidin gönderene ulaşmasına dek. R saati gönderenin ilerlemesinden bu kadar geride kalabilir.~~
> - ~~**T_kredi(E_D)** = N_o·D_o + (k_f + 1)(k_r + 1)·T_BLK + 2L.~~
>   - ~~K4G-06 rev4'te FLOW_CREDIT, BLOCKED'e **tetikli** yanıttır (`06:145`; `06:87`); dolayısıyla burada çarpım gerçektir.~~
>   - ~~Güncel sınır periyodik SACK'e bindirilirse T_kredi = N_o·D_o + (k_f + 1)·T_BLK + (min(k_r, K_slot) + 1)·T_ACK + 2L olur (öneri: §12-7).~~
> - ~~**T_ctrl(E_D, k)** = T_C + L + [N_o > 0 ∨ k > 0]·(N_o·D_o + H_C + k·(A + P_C)).~~
>   - ~~Tek kontrol biriminin karşı uygulamaya ulaşması. Bekleyen taraf yalnız varışı gözler, ACK gerekmez → çarpım yoktur.~~
>   - ~~k, birimin gittiği yönün tavanıdır: FILE_RECEIPT için k_r, FILE_ABORT için k_f.~~
>   - ~~T_C = (S_iletimde + S_baş,C)/r + S_baş,C/(ρ_C·r) (`06:114`); S_baş,C ≈ 50 B; ρ_C = 0,1 [CAL]; H_C ≤ H_maks; P_C ≈ T_C.~~
>   - ~~**Koşul:** FILE_RECEIPT akış 1'de kendinden önceki kontrol birimlerinin kurtarılmasını beklemez (§12-6).~~
> - ~~**T_A(E_D)** = T_işlem + T_tutma + T_ctrl(E_D, k_r). T_tutma: (a) N_h·D_h, (b) 0 (A tutma sırasında durur).~~
> - ~~**T_R(E_D)** = T_G + D_ack + T_kredi + T_ctrl(E_D, k_f).~~
>
> ~~**[hesap]** 31 B/s, φ = 1. Kullanılan değerler: T_max(DATA) = 101,68 s; T_tx = A + P_c = 165,56 s; H_maks = H_C = 729,8 s; L = 72,98 s; T_ACK = 8,41 s; T_BLK = 30 s; T_C = 51,6 s; A + P_C = 81,6 s; T_işlem = 1 s. Deterministik hücreler (DUR-05f-13 [CAL]):~~
>
> ~~| Hücre | N_o×D_o | k_f / k_r | T_G | D_ack | T_kredi | T_ctrl(k_r) | T_ctrl(k_f) | T_A (b) / (a, D_h = 30 s) | T_R |~~
> ~~|---|---|---|---|---|---|---|---|---|---|~~
> ~~| D-0 | 0 | 0 / 0 | 256,1 s | 81,4 s | 176,0 s | 124,6 s | 124,6 s | 125,6 / 155,6 s | 638,0 s |~~
> ~~| D-1 | 1 × 60 s | 0 / 0 | 944,2 s | 81,4 s | 236,0 s | 914,4 s | 914,4 s | 915,4 / 945,4 s | 2 175,9 s |~~
> ~~| D-2 | 1 × 60 s | 3 / 3 | 1 466,1 s | 106,6 s | 686,0 s | 1 159,2 s | 1 159,2 s | 1 160,2 / 1 190,2 s | 3 417,8 s |~~
> ~~| D-3 | 1 × 60 s | 3 / 11 (= K_slot) | 1 533,4 s | 173,9 s | 1 646,0 s | 1 812,0 s | 1 159,2 s | 1 813,0 / 1 843,0 s | 4 512,4 s |~~
>
> - ~~Gevşek üst sınırlardır.~~
> - ~~**Ürün sabitleri** önerilen E_D = D-3 girdileriyle, r_min'de ve φ_min'de hesaplanır (DUR-05f-13). φ_min (DUR-06-3) gelince sayılar büyür; tek dosya hücrelerinde φ = 1.~~
> - ~~k_r > K_slot, örneğin 47 için: m = 4 → T_G = 60 + 729,8 + 15·165,56 + 12·8,41 + 145,96 ≈ 3 520 s [hesap]. Bu rejim DUR-05f-15'tir.~~
>
>
> ~~| Yeniden bağlanma | Oturumsuz aralık N_o·D_o'ya yazılır | Erken iptal kaydı, eski oturumlar `Closed` olunca zamanlayıcısız düşer |~~
