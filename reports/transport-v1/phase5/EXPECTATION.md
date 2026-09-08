# FAZ 5 — ÖN-KAYIT (EXPECTATION RECORD)

> Bu belge **hiçbir Faz-5 performans sonucu görülmeden ÖNCE** yazıldı ve donduruldu.
> Sonuçları gördükten sonra hücre tanımı, deadline, geometri, N kuralı, metrik ya da hüküm
> eşiği DEĞİŞTİRİLEMEZ. Metodolojide gerçek bir hata bulunursa: DUR → ERRATUM yaz → etkilenen
> koşuları geçersiz kıl → o hücreleri baştan koş. Sessiz yama YASAK.

Otorite: `reports/transport-v1/BENCHMARK-METHODOLOGY.md` (Faz-4'te donduruldu).
Çelişkide metodoloji sözleşmesi kazanır.

---

## 1. Taban commit

```
BASE_SHA = b1049e3956de6987b6f43adcaab291607ecc359e
dal      = dev/product-transport-fusion
kaynak ağacı = TEMİZ (crates/, .github/, scripts/ üzerinde commit edilmemiş değişiklik yok)
```

Ölçüm ikilisi bu SHA damgasıyla derlendi (`KAMA_GIT_SHA`), ve damganın manifeste ulaştığı
doğrulandı: `manifest.commit = b1049e3956de6987b6f43adcaab291607ecc359e`.

Herhangi bir düzeltme yapılırsa YENİ SHA atanır ve etkilenen hücreler **sıfırdan** koşulur;
düzeltme-öncesi ve düzeltme-sonrası tekrarlar ASLA aynı N≥30 örnekleminde birleştirilmez.

---

## 2. Kanonik Kama profil kimliği

Resmî kol: **`kama-v1-full`**, kaynak `KamaV1Profile::runtime_config()`.

Bayraksız `--transport kama-product` çağrısı kanonik profildir; doğrulandı:
`arm = kama-v1-full`, `profile_delta = []`.

`profile_delta` boş DEĞİLSE koşu `kama-custom`'dur ve **resmî Faz-5 ürün sonucu olarak
kullanılamaz**. Aşağıdaki bayraklardan HİÇBİRİ resmî kolda verilmez:

```
--fec-profile (varsayılan dışı) · --fec-r · --fec-mode · --initial-window-bytes
--bootstrap-rtt · --bootstrap-fast · --burst-fusion · --fec-risk · --loss-confirm-k
--gate-permille · --spec-l1-permille · --spec-l2-permille · --persist-* · --rp-suppress
```

Her koşu manifesti `arm` ve `profile_delta` alanlarıyla arşivlenir; toplama aşaması
`arm != kama-v1-full` olan bir koşuyu ürün sonucuna KATMAZ.

---

## 3. T0–T7 hücre tanımları

**Hiçbir bozulma modeli icat edilmedi.** Her hücre repoda ZATEN VAR OLAN bir profilden
alındı; kaynak her satırda gösterilmiştir. Değerler VERBATIM taşınmıştır.

Bozulma uygulayıcısı: `scripts/netem/kapi3_harness.sh` (üç netns).
İleri yön = ns-MID `mb0` egress (A→B, DATA); dönüş yönü = ns-MID `ma0` egress (B→A, ACK).
İstemci ns-A'da (parent), sunucu ns-B'de (`nsenter -t $CHLB`), IP `10.10.1.1`.

| Hücre | Profil kaynağı | ileri (fwd) | dönüş (rev) | geo | D |
|---|---|---|---|---|---|
| **T0** clean control | `h6_rtt20` (kapi_manifest.py:87) | `delay 10ms loss 1% rate 10mbit` | aynı | G1 | 20 s |
| **T1a** random loss | `p1_loss50` (kapi_manifest.py:41) | `loss 50% rate 100kbit` | aynı | G1 | 20 s |
| **T1b** random loss | `p2_loss80` (kapi_manifest.py:43) | `loss 80% rate 200kbit` | aynı | G1 | 20 s |
| **T1c** random loss + RTT | `iid50` (product_vs_quinn.sh:88) | `delay 200ms loss 50%` | `delay 200ms loss 10%` | G1 | 20 s |
| **T2a** burst (gemodel) | `h3b_burst50` (kapi_manifest.py:71) | `loss gemodel 50% 25% 90% 10% rate 250kbit` | `rate 250kbit` | G1 | 20 s |
| **T2b** burst (korelasyon) | `burst50` (product_vs_quinn.sh:89) | `delay 200ms loss 50% 50%` | `delay 200ms loss 10%` | G1 | 20 s |
| **T3a** low bandwidth | `h2_cell` (kapi_manifest.py:65) | `delay 300ms 200ms distribution normal loss 25% rate 8kbit` | aynı | G2 | 20 s |
| **T4** high latency | `h6_rtt200` (kapi_manifest.py:89) | `delay 100ms loss 1% rate 10mbit` | aynı | G1 | 20 s |
| **T5** asymmetric path | `r_returnpath_proof.sh:27` R3 | `delay 100ms rate 1mbit` | `delay 100ms rate 64kbit` | G1 | 20 s |
| **T6** degraded return | `p5_revpath` (kapi_manifest.py:51) | `loss 5% rate 100kbit` | `loss 80% rate 100kbit` | G1 | 20 s |
| **T7** mixed loss+reorder | `C4` (fec_persist_family.sh:26) | `delay 100ms 20ms loss 20% reorder 30% 50% rate 1mbit` | `delay 100ms loss 2% rate 1mbit` | G1 | 20 s |

Ayrıca **T3b — SPEC-ÇELİŞKİSİ SONDASI** (kıyas DEĞİL, teşhis):

| Sonda | Profil kaynağı | ileri | dönüş | geo | D | kol | n |
|---|---|---|---|---|---|---|---|
| **T3b** | `p9_l0` (kapi_manifest.py:104 Grup C) | `rate 0.5kbit` | aynı | G3 | 120 s | yalnız `kama-v1-full` | 5 |

### Geometriler

```
G1  msgs=100  payload=256B  submit-interval=50ms   (product_vs_quinn.sh yerleşik kıyas geometrisi)
G2  msgs=30   payload=64B   submit-interval=50ms   (KAPI-1 sınıf-II mesaj planı: msgs/payload)
G3  msgs=20   payload=32B   submit-interval=1000ms (KAPI-1 sınıf-III mesaj planı tabanlı)
```

**Geometri sapması AÇIKÇA KAYDEDİLİR:** KAPI-1 `CLASS_PLAN` sınıf-I için
`paced_interval_ms=1000`, sınıf-III için `30_000` öngörür. Faz-5 bunun yerine kıyas
geometrisinin yerleşik `50 ms` aralığını (G1/G2) kullanır. Gerekçe: 1 s aralık × 100 mesaj tek
başına 100 s sunum demektir ve deadline'ı ≥120 s'ye zorlar; bu, TCP'nin 60 s gözlem tavanını
(§4) ihlal eder ve kampanyayı ~30 saate çıkarır. Aralık **tüm kollara simetrik** uygulanır,
hiçbir taşıyıcıyı kayırmaz ve manifestte `submit_interval_ns` olarak kayıtlıdır.

### Fizibilite (test-tasarımı geçerlilik kontrolü — sonuçlardan ÖNCE hesaplandı)

Mesaj başına yaklaşık tel maliyeti ≈ payload + 26 B çerçeve + 28 B IPv4/UDP.

| Hücre | kapasite | tel yükü | asgari süre (kayıp çarpanı dahil) | D | pay |
|---|---|---|---|---|---|
| T0 | 1.25 MB/s | 31 kB | ~0.03 s | 20 s | ≫ |
| T1a | 12.5 kB/s | 31 kB | ~5.0 s (×2) | 20 s | 4× |
| T1b | 25 kB/s | 31 kB | ~6.2 s (×5) | 20 s | 3× |
| T1c | limitsiz | 31 kB | RTT-bağlı, 20 s ≈ 100 RTT | 20 s | ✔ |
| T2a | 31 kB/s | 31 kB | ~2.0 s (×2) | 20 s | 10× |
| T2b | limitsiz | 31 kB | RTT-bağlı, ≈100 RTT | 20 s | ✔ |
| T3a | 1.0 kB/s | 3.5 kB | ~4.7 s (×1.33) | 20 s | 4× |
| T4 | 1.25 MB/s | 31 kB | ~0.03 s + RTT | 20 s | ≫ |
| T5 | fwd 125 kB/s / rev 8 kB/s | 31 kB / ~2.5 kB ACK | ~0.6 s | 20 s | ≫ |
| T6 | 12.5 kB/s | 31 kB | ~2.6 s (fwd %5) | 20 s | 7× |
| T7 | 125 kB/s | 31 kB | ~0.3 s | 20 s | ≫ |
| T3b | 62 B/s | ~1.7 kB | ~28 s | 120 s | 4× |

Hiçbir hücrede "tüm kollar matematiksel olarak tamamlayamaz" durumu yok → §T4 uyarısındaki
test-tasarımı hatası ön-kayıt aşamasında elenmiştir.

---

## 4. Rakip kollar ve gözlem penceresi kısıtı

```
kama-v1-full             --transport kama-product   (bayraksız = kanonik)
quinn-reliable-stream    --transport quinn
tcp-default              --transport tcp
```

İkincil (ayrı tablo, ASLA reliable-stream ile aynı tabloda değil):
`quinn-datagram-app-retry` (`--transport qdgram`) — yalnız bilgilendirici olduğu hücrelerde.

**ÖN-KAYITLI GEÇERLİLİK KISITI — D ≤ 60 s.** Kod okuması (ölçümden önce yapıldı):

```
kama-product : koşu sonu = son-sunum + D            (kama_product.rs:752-755)
qdgram       : koşu sonu = son-sunum + D            (qdgram.rs:342-345)
quinn        : mesaj-başı pencere = D + kalan sunum (quic.rs:348-351) → koşu sonu = son-sunum + D
tcp          : koşu sonu = sunum + min(D, 60 s)     (tcp.rs:164)  ← TAVAN
```

D ≤ 60 s iken dört kolun gözlem penceresi ÖZDEŞTİR. D > 60 s'te TCP kolu kısalır ve kıyas
**BLOCKING** olur. Bu yüzden kıyas üreten HER hücre D = 20 s'tir. Tek istisna T3b'dir
(D = 120 s) ve bu yüzden **yalnız Kama koşar, kıyas üretmez** — teşhis sondasıdır.

---

## 5. Deadline

Birincil: **D = 20 s**.
Aynı koşulardan ek maliyet olmadan türetilen ikincil eğri: **D ∈ {5, 10, 20} s**
(teslim damgaları mutlaktır; `bench metrics --deadlines-s 5,10,20` aynı ham veriden hesaplar).

Deadline sonuçlara bakıldıktan SONRA değiştirilmeyecektir.

---

## 6. Kurulum görünümü (setup view)

Birincil: **steady-state** (kurulum ölçüm penceresi dışında, `setup_ns` ayrı raporlanır).
Ayrıca her hücrede **cold-flow** görünümü de raporlanır (aynı ham veriden türetilir:
`completion(D) = kararlı-durum completion(D − setup_ns)`).

İki görünüm ASLA tek tabloda karıştırılmaz; her satır hangi görünümde olduğunu yazar.

---

## 7. Birincil metrik

**completion(D)** = deadline içinde teslim edilen uygulama mesajı oranı.

Faz-5 boyunca bu metrik DEĞİŞTİRİLMEZ. Throughput, tel byte'ı, retransmit sayısı ya da başka
bir metrik, bir taşıyıcıyı avantajlı gösterdiği için birincil metriğin yerine GEÇMEZ.

## 8. İkincil metrikler (açıklayıcı, yerine geçmez)

blackout (ikili + süre + kurtarma) · stall katmanlaması · sonuç sınıfı dağılımı ·
tel maliyeti dökümü (data/retx/parity/ACK) · `setup_ns` · aktivasyon sayaçları ·
dönüş-yolu tahmincisi (fwd_loss‰, ret_ack_loss‰, health).

---

## 9. Beklenen mekanizma aktivasyonu (koddan türetildi, ölçümden ÖNCE)

| Hücre | Ateşlemesi BEKLENEN | Ateşlememesi beklenen |
|---|---|---|
| T0 | ~0 parity (yapı-gate kapalı), düşük retx, akış sonunda tail probe | FEC parity kütlesi, emergency |
| T1a/T1b/T1c | ARQ retx yüksek; yapı-gate açılır → parity; spekülatif L1/L2; ledger confirm | emergency (ack_conf yüksek kalır) |
| T2a/T2b | ARQ retx; korelasyonlu kayıpta yapı-gate davranışı farklı; `parity_token_denied` ≫ `parity_emitted` (Faz-3 gözlemi) | — |
| T3a | pacer bağlayıcı; `clamp` sayacı artar; `would_zero` = 0 beklenir (8 kbit ≫ 125 B/s köşesi) | would_zero |
| T4 | PreConverge çıkışı, tail probe, cold-start yolu; srtt≈200 ms | parity (kayıp %1) |
| T5 | dönüş-yolu gözlemcisi: fwd_loss ≈ 0, dönüş bant darlığı `ret_impair` olarak görünür | fwd_loss'un yükselmesi (INV-RETURN-1 ihlali olurdu) |
| T6 | `ret_ack_loss` yüksek, `fwd_loss` DÜŞÜK; health Degraded/Dead | fwd_loss'un ACK kaybını taklit etmesi (INV-RETURN-1 ihlali) |
| T7 | reorder baskısı `rp_suppress=300‰` eşiğini geçerse spekülatif öneri BASTIRILIR; spurious sayacı | denetimsiz spekülatif L2 patlaması |
| T3b | `would_zero > 0` (BtlBw ≤ 62 B/s ⇒ §8.5 cap = 0) | clamp'in uygulanması |

**Vakum sayaç uyarısı (ön-kayıtlı):** `KP-ACTIVATED` satırındaki `tail_probe=` alanı
`diag-sched-telemetry` feature'ına bağlıdır ve varsayılan üründe `n/a` okur. Faz-5 tail
kanıtı için YALNIZ manifestteki `activation.tail_probes` (her zaman açık) kullanılacaktır.
Manifestte `clamp`/`would_zero` alanı YOKTUR; bu iki sayaç yalnız istemci stderr'indeki
`fec2=[repair_not_arqed=… clamp=… would_zero=… partial_blocks=…]` satırından okunur — bu yüzden
her tekrarın `c.err` dosyası arşivlenir.

---

## 10. Beklenen nitel sonuç (TAHMİN — sonuçlardan önce yazıldı)

1. **T0:** Kama ≈ rakipler (hepsi ≈1.00). TCP/quinn'in küçük bir üstünlüğü kabul edilebilir;
   Faz-5 blocker'ı DEĞİL. Kama'da patolojik retransmit/blackout OLMAMALI.
2. **T1a/T1b:** Kama ≥ quinn bekliyorum. Gerekçe: %50–80 bağımsız kayıpta quinn'in sıralı
   akışı head-of-line bloklanır; Kama mesaj-granülüdür ve kodlama+ARQ birlikte çalışır.
3. **T1c (200 ms + %50):** Denetimin yeniden hesabı (n=6) quinn 0.995 / kp-iw2400 0.990
   vermişti — yani **PARITE** bekliyorum, Kama üstünlüğü DEĞİL.
4. **T2b (burst, 200 ms):** Denetim n=6'da quinn 0.895 / kp-iw2400 0.995 bulmuştu →
   **Kama üstünlüğü** bekliyorum. Bu, Faz-5'in doğrulamakla yükümlü olduğu tek önceden
   iddia edilmiş üstünlüktür ve ön-kayıtlı beklentidir.
5. **T2a (gemodel %50):** Kama ≥ quinn.
6. **T3a (8 kbit):** Dar bantta üç kol da düşük completion; **belirsiz**. Kama'nın parity'si
   dar bandı israf ederse Kama KAYBEDEBİLİR — bu gerçek bir risk olarak kaydediliyor.
7. **T4 (200 ms RTT, %1):** Parite bekliyorum; cold-start Kama'yı ilk saniyede geriletebilir.
8. **T5 (asimetrik bant):** Parite–hafif Kama üstünlüğü; asıl kanıt yön ayrımının telemetride
   görünmesidir.
9. **T6 (dönüş %80 kayıp):** Kama'nın **birincil hedefi**. Kama > quinn/TCP bekliyorum;
   quinn/TCP ACK kaybını tıkanıklık sanıp hız düşürmeli. Kama INV-RATE-1 gereği düşürmemeli.
10. **T7 (kayıp+reorder):** Parite; reorder bastırması sayesinde yanlış-spekülasyon düşük.

Bu tahminler **hüküm değildir**. Sapma çıkarsa sapma dekompozisyonla açıklanacak, tahmin
geriye dönük düzeltilmeyecektir.

---

## 11. Beklentiyi ÇÜRÜTECEK olan (falsification)

* **T6'da** `fwd_loss_permille` yükselirse (ACK kaybı ileri kayıp gibi okunuyorsa) →
  INV-RETURN-1 ihlali → **WIRING/IMPLEMENTATION BLOCKER**, performans sayısı okunmaz.
* **T5'te** manifest asimetrik ama runtime simetrik davranıyorsa → **WIRING BLOCKER**.
* Herhangi bir hücrede `arm != kama-v1-full` çıkarsa → o hücre ürün sonucu değildir.
* Herhangi bir hücrede beklenen mekanizma hiç ateşlemediyse (ör. T1'de parity = 0 ve
  gate hiç açılmadı) → **BAD DEFAULT / WIRING** araştırması, tuning DEĞİL.
* **T2b'de** Kama üstünlüğü tekrar ÜRETİLEMEZSE: denetimin n=6 sonucu ön-kayıtlı beklentiydi;
  N≥30'da düşerse beklenti **yanlışlanmış** sayılır ve rapor bunu açıkça yazar.
* T0'da Kama'da patolojik davranış (blackout > 0, ya da retx ≫ rakipler) → regresyon.

---

## 12. Koşu sayıları (ÖN-KAYITLI, sonradan artırılmaz)

```
AŞAMA A (SMOKE)   n = 4 / kol / hücre     — yalnız araç doğrulama, D = 10 s
                  hüküm sınıfı: smoke      — PERFORMANS SONUCU ÇIKARILMAZ
AŞAMA B (FINAL)   N = 30 / kol / hücre    — D = 20 s
T3b sondası       n = 5 (yalnız Kama)     — kıyas değil, çelişki tespiti
```

11 kıyas hücresi × 3 kol × 30 tekrar = **990 ölçüm koşusu** + 15 smoke-öncesi doğrulama.

**N artırma kuralı:** N sonuca bakılarak ARTIRILMAZ. Yalnız tek istisna, ön-kayıtlı olarak:
bir hücre `STATISTICALLY INCONCLUSIVE` çıkarsa, o hücre **bir kez** N=60'a çıkarılabilir ve
bu karar sonuç işaretine bakılmadan, yalnız güven aralığı örtüşmesine bakılarak verilir.
Kaybeden hücrelerin seçici olarak uzatılması YASAK.

---

## 13. Azami çalışma bütçesi

```
Koşu başına ≈ (msgs−1)×interval + D + kurulum/teardown ≈ 25 s (G1, D=20 s)
Aşama A     ≈ 11 hücre × 3 kol × 4 × 15 s          ≈ 33 dk
Aşama B     ≈ 11 hücre × 3 kol × 30 × 25 s         ≈ 6.9 sa (seri)
T3b sondası ≈ 5 × 130 s                            ≈ 11 dk
AZAMİ BÜTÇE: 10 saat duvar saati. Aşılırsa kampanya DURDURULUR ve kısmi kapsam raporlanır.
```

**Paralellik kararı (ön-kayıtlı ve DOĞRULANACAK):** hücre-shard'ları en fazla **4 eşzamanlı**
koşar (her shard kendi `unshare -rnm` netns'inde, tam izole). Bu, `kapi_calrun.sh`'ın
"hücreler arası paralellik YOK" disiplininden bir SAPMADIR, bu yüzden Aşama A'da
**doğrulanacaktır**: bir hücre hem seri hem 4-paralel koşulacak; completion medyanı ve srtt
materyal olarak ayrışırsa paralellik terk edilip tüm kampanya SERİ koşulur. Doğrulama
sonucu rapora yazılır.

---

## 14. Sonuç sınıfı ve istatistik politikası (Faz-4'ten devralındı, değiştirilmez)

* merkez = medyan, dağılım = IQR, medyan CI = sıra istatistiği (dağılımdan bağımsız)
* oran CI = kesin Clopper–Pearson
* n < 30 ⇒ hüküm sınıfı `smoke`, performans iddiası YASAK
* stall = blackout ∪ timeout ∪ connection-failure; all-runs / stall-free / insidans BİRLİKTE
* harness/infra arızası agregadan dışlanır ama insidansı raporlanır; N'e SAYILMAZ
* okunamayan koşu sessizce silinmez (dosya yok = infra, bozuk = harness)

---

## 15. Her ailenin üreteceği karar

| Aile | Ürettiği karar |
|---|---|
| T0 | Kama sağlıklı yolda patolojik mi? (regresyon kapısı) |
| T1 | Bağımsız kayıpta uzmanlaşma görünüyor mu? |
| T2 | Korelasyonlu kayıpta önceden iddia edilen üstünlük N≥30'da AYAKTA MI? ve `parity_denied ≫ parity_emitted` koruyucu mu, açlık mı? |
| T3a | Dar bantta kodlama israf mı? |
| T3b | §8.5/§11 çelişkisi ürün yolunda GERÇEKTEN tetikleniyor mu? |
| T4 | Yüksek RTT'de cold-start bedeli ne? |
| T5 | Yön ayrımı gerçek mi (asimetrik bant)? |
| T6 | Bozuk dönüş yolunda ileri teslim korunuyor mu? (birincil ürün iddiası) |
| T7 | Kayıp+reorder karışımında kurtarma hâlâ faydalı mı? |

## 16. T3b ön-kayıtlı DURDURMA KURALI

T3b tekrarlarının herhangi birinde istemci `c.err` dosyasında `would_zero=` değeri **> 0** ise:

* o alt-hücre DURDURULUR,
* hüküm **`BLOCKED — SPEC CONFLICT`** yazılır,
* rapor şu cümleyi taşır: *"§8.5 ile §11 arasındaki düşük-bant parity çelişkisi sahip
  kararını gerektirir"*,
* Faz-5 içinde taraf SEÇİLMEZ, kod DEĞİŞTİRİLMEZ.

`would_zero = 0` ise: çelişki köşesine girilmemiştir; bu da raporlanır ve T3b hükmü
`SPEC CONFLICT NOT REACHED` olur (blocker değil).

**Bilinen kapsam sınırı (ön-kayıtlı):** emergency yolundaki parity (`fec_controller_latch`
emergency dalı) `budget::parity_final`'i BAYPAS eder ve bu iki sayacı artırmaz. Dolayısıyla
`would_zero = 0` görmek "§8.5 bütçe otoritesi her yerde uygulandı" demek DEĞİLDİR. Bu sınır
rapora aynen yazılacaktır.
