# KAMA TRANSPORT V1 — FAZ 5: T0–T7 HOSTILE DOĞRULAMA

Karar odaklı rapor. Ham hücre kayıtları: [`phase5/RESULTS-RUNNING.md`](phase5/RESULTS-RUNNING.md).
Ön-kayıt: [`phase5/EXPECTATION.md`](phase5/EXPECTATION.md) ·
Erratum: [`phase5/ERRATUM-1-blackout-boundary.md`](phase5/ERRATUM-1-blackout-boundary.md) ·
Aşama-A: [`phase5/STAGE-A-VALIDATION.md`](phase5/STAGE-A-VALIDATION.md) ·
Metodoloji sözleşmesi: [`BENCHMARK-METHODOLOGY.md`](BENCHMARK-METHODOLOGY.md)

---

## 1. YÖNETİCİ HÜKMÜ

**Kanonik Kama v1, ilan edilmiş hostile zarfı içinde hiçbir hücrede TCP veya QUIC'e materyal
olarak kaybetmedi.** İki hücrede örtüşmeyen güven aralıklarıyla üstün, sekiz hücrede parite,
bir hücrede (üç kolun da çöktüğü rejimde) göreli üstün.

En önemli iki sonuç:

* **Birincil ürün iddiası N=30 ile doğrulandı (T6).** Dönüş yolunun %80'i yok edildiğinde Kama
  completion medyanı 1.000 (CI [1.000,1.000]) verirken quinn 0.740 (CI [0.210,0.990]) ve TCP
  0.060 (CI [0.050,0.630]) — **CI'lar örtüşmüyor.** Mekanizma da ölçüldü: enjekte %5 ileri kayıp
  → ölçülen 45.5‰; enjekte %80 dönüş ACK kaybı → ölçülen 793‰. Bozuk geri besleme ileri veri
  kaybı gibi okunmadı.
* **Önceden bankaya yazılmış bir üstünlük iddiası geri çekildi (T2b).** Denetimin n=6 sonucundaki
  "Kama burst'te quinn'i yeniyor" iddiası N=30'da yeniden üretilemedi; iki kol paritede
  (`P = 0.533`).

**Kodlama (FEC) hakkında dürüst tablo:** Kama'nın ölçülen üstünlükleri **ARQ + mesaj-granülü
teslimden** gelmektedir, kodlamadan değil. Altı hostile hücrenin beşinde parity önerildi ama
§8.5 bant kısıtı sonrası çoğunlukla hiç maddeleşmedi (BULGU FEC-1).

---

## 2. TABAN VE NİHAİ COMMIT

```
ön-kayıt tabanı   b1049e3956de6987b6f43adcaab291607ecc359e
ERRATUM-1 sonrası e3c1e8bb740bbc25acd2c9fc5a7fc478a3f15adc   ← TÜM Faz-5 ölçümleri bu SHA
```
990 manifestin **tamamı** tek commit damgası taşıyor: `e3c1e8bb…`. Karışık-sürüm koşu YOK.

## 3. DONMUŞ BEKLENTİ KAYDI

`EXPECTATION.md` herhangi bir performans sonucu görülmeden önce yazılıp commit edildi
(`ae861b23`). 11 kıyas hücresinin tamamı repoda **zaten var olan** profillerden verbatim alındı
(KAPI-1 §9 kataloğu, `product_vs_quinn.sh`, `r_returnpath_proof.sh`, `fec_persist_family.sh`);
hiçbir bozulma modeli icat edilmedi. Sonuçlar görüldükten sonra hücre tanımı, deadline, geometri,
N kuralı, birincil metrik ve hüküm eşikleri **değiştirilmedi**. Tek değişiklik ERRATUM-1'in
işaretli supersession notudur (yalnız taban commit).

## 4. KAMPANYA MATRİSİ

11 kıyas hücresi × 3 kol × 30 tekrar = **990 ölçüm koşusu** + T3b sondası (5, yalnız Kama).
Aşama A: 132 koşu (smoke, kanıt değil). Süre: 2026-09-08T15:02:57Z → 20:23:22Z (**5 sa 20 dk**).

| | |
|---|---|
| tamamlanan shard | **11/11** |
| BARRIER-FAIL (infra) | **0** |
| rc ≠ 0 (harness arızası) | **0** |
| üretilen manifest | **990/990** |
| profil sapmalı Kama koşusu | **0/330** |

## 5. KOŞUCU / CI DURUMU

Kampanya **yerel doğrulanmış netem harness'ında** koştu (`kapi3_harness.sh`, üç netns,
ileri = `mb0` egress, dönüş = `ma0` egress). Her shard kendi izole `unshare -rnm` netns'inde,
seri (`maxpar=1`), shard başına timeout ve `DONE`/`BARRIER-FAIL` damgasıyla.

**CI sınırı (beyan):** Faz-4'te eklenen gecelik netem adımı hosted/self-hosted CI'da HÂLÂ
koşmadı; `NETEM_RUNNER` değişkeni ve self-hosted netem runner'ı gerekiyor. Faz-5 kanıtı bu yüzden
gecelik CI'dan DEĞİL, yerel doğrulanmış harness'tan gelmektedir. 24 saatlik opak iş yaratılmadı.

## 6. İSTATİSTİK POLİTİKASI TEYİDİ

Faz-4 sözleşmesi aynen uygulandı: merkez = medyan, dağılım = IQR, medyan CI = sıra istatistiği
(dağılımdan bağımsız), oran CI = kesin Clopper–Pearson, RNG/bootstrap YOK. Tüm kıyas hücrelerinde
`n = 30` → hüküm sınıfı **`final`**. Stall katmanlaması (all-runs / stall-free / insidans+CI) her
hücrede raporlandı. Ek olarak varsayımsız sıra istatistiği `P(A koşusu > B koşusu)` verildi.
**Hiçbir hücrede N sonuca bakılarak artırılmadı.**

## 7–14. HÜCRE SONUÇLARI

| Hücre | Kama completion(20s) + CI | quinn + CI | TCP + CI | Blackout (K/q/T) | Stall (K/q/T) | Adillik | Kama aktivasyon (medyan) | HÜKÜM |
|---|---|---|---|---|---|---|---|---|
| **T0** temiz kontrol | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 0/0/0 | 0/0/0 | NON-NORM | parity 0 · retx 33 (hepsi tail probe) | **COMPARABLE / PARITY** |
| **T1a** kayıp %50 | **1.000 [1.000,1.000]** | 0.900 [0.530,1.000] | 0.120 [0.060,0.510] | 18/27/29 | 18/28/30 | NON-NORM | parity 0 · retx 95 · clamp 10 428 | **KAMA DOMINATES** |
| **T1b** kayıp %80 | 0.010 [0.000,0.340] | 0.000 [0,0] | 0.000 [0,0] | 30/13/17 | 30/30/30 | NON-NORM | 18/30 koşuda **hiç ACK yok** | **KAMA WINS** *(göreli; üçü de başarısız)* |
| **T1c** %50 + 200 ms | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 0.830 [0.600,1.000] | 23/27/30 | 23/28/30 | NON-NORM | parity 0 · clamp 12 230 | **COMPARABLE / PARITY** |
| **T2a** gemodel burst | 1.000 [0.980,1.000] | 1.000 [1.000,1.000] | 0.770 [0.210,1.000] | **4**/24/30 | **4**/25/30 | NON-NORM | parity **19** · denied 1 560 | **COMPARABLE / PARITY** |
| **T2b** korelasyon burst | 1.000 [0.990,1.000] | 1.000 [0.970,1.000] | 0.430 [0.300,0.920] | 21/22/27 | 21/24/29 | NON-NORM | parity 1 · denied 141 | **COMPARABLE / PARITY** |
| **T3a** dar bant 8 kbit | 1.000 [1.000,1.000] | 1.000 [0.967,1.000] | 1.000 [1.000,1.000] | 28/30/30 | 28/30/30 | NON-NORM | parity 0 · clamp 7 486 | **COMPARABLE / PARITY** |
| **T3b** 0.5 kbit sondası | — (yalnız Kama, n=5) | — | — | — | — | — | `would_zero = 0`, köşeye GİRİLDİ | **SPEC CONFLICT NOT REACHED** |
| **T4** RTT 200 ms | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 0/0/0 | 0/0/0 | NON-NORM | parity 0 · retx 1 | **COMPARABLE / PARITY** |
| **T5** asimetrik bant | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 0/0/0 | 0/0/0 | NON-NORM | retx 0 · asimetri GÖZLENMEDİ | **COMPARABLE / PARITY** *(zayıf kanıt)* |
| **T6** dönüş %80 kayıp | **1.000 [1.000,1.000]** | 0.740 **[0.210,0.990]** | 0.060 **[0.050,0.630]** | 20/10/29 | 20/19/30 | NON-NORM | fwd_loss 45.5‰ ↔ ret_ack 793‰ | **KAMA DOMINATES** |
| **T7** kayıp + reorder | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 1.000 [1.000,1.000] | 6/1/4 | 6/1/4 | NON-NORM | `spurious = 1` · fwd_loss 200‰ | **COMPARABLE / PARITY** |

Deadline eğrisi tekrar eden desen: **Kama tavana daha erken varıyor.**
T1c D=5 s: 0.960 ↔ 0.590 · T2b D=5 s: 0.940 ↔ 0.660 · T3a D=5 s: 0.900 ↔ 0.200.
20 s bütçede fark çoğu hücrede kapanıyor. Bu bir üstünlük iddiası değil, ölçülmüş davranış farkı.

## 15. QUINN KIYASI

quinn-reliable-stream, Kama'nın en güçlü rakibi: sekiz hücrede parite, T1a ve T6'da kaybediyor.
**Ayırt edici kırılma noktası geri besleme kalitesidir:** T6'da 30 koşunun 9'unda bağlantıyı hiç
kuramadı (ACK'lerin %80'i kaybolunca QUIC el sıkışması tamamlanamıyor), T1b'de 17/30.
Tel maliyetinde Kama çoğu hücrede quinn'in altında (T1a 54.7↔56.5 kB, T1c 55.5↔58.3 kB,
T3a 3.6↔6.4 kB, T7 39.7↔44.6 kB); T2a (+%16) ve T6 (+%9) istisna.

İkincil kol `quinn-datagram-app-retry` bu kampanyada **koşulmadı** — ayrı anlam sınıfı olduğu
için ayrı tablo gerektirir ve ön-kayıtta zorunlu tutulmamıştı.

## 16. TCP KIYASI

tcp-default hostile rejimlerin tamamında en zayıf kol: T1a medyan 0.120, T6 0.060, T2b 0.430.
Yalnız kolay hücrelerde (T0/T4/T5/T7) tavana ulaşıyor.

**Tel muhasebesi sınırı (metodoloji §10):** TCP sayıları YALNIZ uygulama katmanıdır; çekirdek içi
retransmit/ACK/başlık trafiği harness'a görünmez. Bu yüzden TCP'nin byte'ları Kama/quinn'in tam
tel muhasebesiyle **DOĞRUDAN KIYASLANAMAZ** ve bu raporda hiçbir "Kama TCP'den %X az tel harcıyor"
iddiası kurulmamıştır.

## 17. ADİLLİK / ANLAM SINIRLARI

11 hücrenin **tamamı NON-NORMALIZED**, hiçbiri BLOCKING değil: deney koşulları (yük, mesaj sayısı,
deadline, sunum aralığı, ileri ve dönüş bozulması, kurulum sınıfı) eşleşiyor; **anlam** eşleşmiyor.
Kama `reliable-messages` / mesaj granülünde **sırasız**; quinn ve TCP `reliable-ordered-stream` /
**tam sıralı**. Kama'nın kayıplı rejimlerdeki avantajının bir kısmı doğrudan bu **daha zayıf
garantiden** gelir (sıralı akışta tek kayıp ardındakileri bloklar). Bu fark normalize
**edilmemiştir** ve edilmemelidir. Hiçbir BLOCKING kıyastan performans hükmü üretilmedi.

## 18. BLACKOUT / STALL ÖZETİ

Blackout tanımı T0–T7'den önce donduruldu ve ERRATUM-1 ile bir sınır artefaktından arındırıldı.
Kolay hücrelerde (T0/T4/T5) üç kolda da 0/30. Ağır hücrelerde Kama'nın en belirgin süreklilik
üstünlüğü **T2a**'dadır: 4/30 ↔ quinn 25/30, **CI'lar örtüşmüyor**.

**Beyan:** blackout sayıları, sonuç sınıfı dağılımları farklı olduğunda kollar arası doğrudan
kıyaslanamaz. T6'da quinn'in düşük blackout sayısı (10/30) daha iyi süreklilik değil **daha az
teslim** demektir (9 `connection-failure` + 8 `deadline-miss`); hiç bağlanamamış koşu blackout
üretmez.

## 19. AKTİVASYON KANITI

Her hücrede kanonik kimlik doğrulandı: **330/330 Kama koşusu `kama-v1-full`, sapma 0.**

| mekanizma | kanıt |
|---|---|
| ARQ / retransmit | tüm kayıplı hücrelerde aktif (retx 20–143 medyan) |
| tail probe | T0'da 33, T6'da 17 (her-zaman-açık sayaç) |
| yön ayrımı | T6: fwd 45.5‰ ↔ ret_ack 793‰, `dir_ev=true`, health Degraded/Dead |
| reorder bastırma | T7: `spurious = 1`, fwd_loss 200‰ = enjekte %20 (reorder şişirmedi) |
| §8.5 bütçe kısıtı | 5 hücrede aktif (clamp 4 300–13 000) |
| FEC parity | **yalnız T2a (19), T2b (1), T6 (3), T7 (4.5)** — bkz. BULGU FEC-1 |

## 20. GEÇERSİZ KILINAN KOŞULAR

**ERRATUM-1:** Aşama B'nin ilk koşumunun tamamı (T0 90 koşu + T1a kısmi) geçersiz kılındı.
Sebep: blackout ufku sınır artefaktı **yönlü ölçüm yanlılığı** üretiyordu (aynı fiziksel
davranışta Kama 16/30, quinn 1/30, TCP 0/30 — üçünün de completion'ı 1.000). Veri silinmedi,
`stageB-INVALIDATED-blackout-boundary/` olarak arşivlendi. Düzeltme öncesi/sonrası tekrarlar
aynı örneklemde **birleştirilmedi**; kampanya sıfırdan koşuldu.

Bunun dışında geçersiz koşu YOK: 0 harness arızası, 0 infra arızası, 0 sapmalı profil.

## 21. YAPILAN DÜZELTMELER VE YENİDEN KOŞUM SINIRI

Tek düzeltme ERRATUM-1'dir ve **yalnız ölçüm katmanındadır** (`crates/bench/src/outcome.rs`,
tek ifade). **Transport karar mantığına Faz-5 boyunca hiç dokunulmadı** — `adaptive-transport`,
`client-core`, `kama-*` crate'lerinde sıfır değişiklik. Düzeltme regresyon testiyle kilitlendi
(vaka gerçek kampanya verisinden üretildi) ve mevcut üç blackout testi değişmeden geçti.

## 22. KALAN BLOCKER'LAR VE FAZ-6 GİRDİLERİ

**Faz-5 blocker'ı YOK.** Aşağıdakiler Faz-6 dondurma incelemesinin karar vermesi gereken
kalemlerdir; hiçbiri Faz-5'te bir hostile kaybına yol açmamıştır ve **hiçbirine Faz-5'te
müdahale edilmemiştir**:

1. **BULGU T1b-1 — sıfır-ACK soğuk başlangıcında kalıcı durma.** %80 kayıpta 18/30 koşuda hiç
   ACK dönmedi → RTT örneği yok → retx zamanlayıcısı kurulmadı → akış ilk pencerede (1 698 B /
   25 600 B) 20 s boyunca durdu, hiç yeniden denemedi. Bu rejim için tasarlanmış mekanizma
   (`bootstrap_rtt`, `bootstrap_fast_ladder`) ürün profilinde **bilinçli kapalı**
   ("kalibre-değil", `runtime.rs:195`). *Karar: v1 böyle bir soğuk-başlangıç yoluyla sevk
   edilecek mi?*
2. **BULGU FEC-1 — kodlama önerisi sessizce maddeleşmiyor.** 5 hücrede parity binlerce kez
   önerildi (clamp 4 300–13 000), §8.5 bant kısıtı küçülttü, T1a/T1c/T3a'da **sıfır parity
   çerçevesi** çıktı (ne emisyon ne ret). Desteklenen çıkarım: küçültülen oran K=8'de tek onarım
   sembolüne yetmiyor. **Yönü tek değil** — T3a'da koruyucu (dar banda israf yok), T1a/T1c'de
   katkısız. *Karar: §8.5 kısıtı + `r` granülerliği hostile rejimlerin geniş bir sınıfında FEC'i
   fiilen devre dışı bırakıyor mu?*
3. **Telemetri boşluğu.** Aktivasyon manifesti *"hiç önerilmedi"* ile *"önerildi ama
   maddeleşmedi"* durumlarını ayırt edemiyor (ikisi de `parity=0, denied=0`). Ayrım yalnız
   stderr `clamp` sayacında. Faz-3'ün "yapılandırıldı ≠ ateşledi" ayrımının bir kademe altı.
4. **T5 test-tasarımı sınırı.** Ön-kayıtlı asimetrik-bant hücresi ekseni zorlamadı (dönüş yolu
   kullanımı %3.5). Sözleşmenin yönlü-gözlem şartı **T6 tarafından** karşılandı. Bant asimetrisi
   için ACK trafiğini dönüş kapasitesine yaklaştıran bir yük geometrisi gerekir.
5. **§8.5 / §11 çelişkisi çözülmedi (dokunulmadı).** T3b sondasında köşeye **girildi**
   (BtlBw 19 ve 1 B/s) ama `would_zero = 0`, çünkü arbiter hiç parity önermedi. Ayrıca beyan
   edilen kapsam sınırı: emergency yolu `budget::parity_final`'i baypas eder ve bu sayaçları
   artırmaz — yani `would_zero = 0`, "§8.5 otoritesi her yerde uygulanıyor" DEMEK DEĞİLDİR.
6. **CI kapısı doğrulanmadı.** Gecelik netem adımı henüz gerçek runner'da koşmadı.

## 22b. NİHAİ KAPI

```
cargo fmt --all --check                                              OK
cargo clippy --workspace --all-targets --all-features -- -D warnings   0 hata
cargo test  --workspace --all-features    1257 geçti / 0 başarısız / 18 ignored (167 test ikilisi)
```
18 `ignored` Faz-5'ten gelmez; gecelik-only işaretli mevcut sim testleridir.
Benchmark harness kapısı: `cargo test -p bench --all-features` 58 geçti / 0 başarısız.

## 23. FAZ-5 HÜKMÜ

Faz-5 kabul ölçütlerinin tamamı karşılandı: T0 regresyonsuz · T1–T7 donmuş metodolojiyle geçerli
kanıt · tüm kritik iddialar N=30 + CI · 330/330 koşu kanonik profil · ileri/dönüş bozulmaları
uygulandı ve (T6/T7'de) runtime'da gözlendi · aktivasyon manifestleri katılımı kanıtladı ·
BLOCKING adillik uyuşmazlığı yok · harness/infra arızaları ayrıldı (sıfır) · **hostile Kama kaybı
olmadı**, dolayısıyla taksonomi tetiklenmedi · yeni araştırma ekseni açılmadı.
