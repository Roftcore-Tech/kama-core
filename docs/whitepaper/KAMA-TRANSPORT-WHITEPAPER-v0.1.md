# Kama Transport — Düşman Ağlarda Mesaj Teslim Kapasitesi: Ön-Kayıtlı Karşılaştırma Kampanyası (FAZ-1)

**Sürüm:** v0.1 TASLAK (iç inceleme; dış dağıtım öncesi Abkarada onayı gerekir) · **Tarih:** 2026-08-24
**Kanıt tabanı:** `reports/kapi-eval-v1/` (ham veri `ab946a33`, hükümler `1b39a4f0`, rapor `d677ab6b`)
**Metodoloji otoritesi:** `docs/tasks/v2-benchmark-gorev-r5-kosuya-hazir.md` (r5); tasarım KAPI-1; ön-kayıt KAPI-2.

---

## Yönetici Özeti

Kama, yüksek kayıp / düşük bant / asimetrik IP kanallarında mesaj teslimini korumak için tasarlanmış,
kurtarma-odaklı bir taktik haberleşme transport katmanıdır. Bu belge, Kama'nın **önceden kaydedilmiş
(preregistered)**, eşleştirilmiş tasarımlı ve eşit kanal-türevli byte-bütçeli bir kampanyada, aynı
kanal koşullarında koşan iki Quinn 0.11.11 (QUIC) referans konfigürasyonuna karşı ölçülen davranışını
raporlar.

**Ana bulgu (ön-kayıtlı, Holm-düzeltmeli):** %80 rastgele paket kaybı ve 200 kbit/s kanalda (p2),
eşit IP-byte bütçesinde, 120 s mesaj-deadline'ıyla Kama mesajların **medyan %50,5**'ini teslim etti;
Quinn DATAGRAM+ARQ referansı %4,0, Quinn stream referansı %2,0. Eşleştirilmiş fark **+38,4 puan**,
%95 güven aralığı **[+31,5, +44,9]**, Holm-sonrası p = 0,0003 (n = 50 tekrar).

**İkinci bulgu:** asimetrik ters-yol kanalında (p5: ileri %5 / geri %80 kayıp) her iki sistem de
mesajları tamamladı; Kama aynı işi belirgin biçimde daha az IP-byte ile yaptı — ön-kayıtlı ikincil
maliyet ucunda eşleştirilmiş byte-verimi farkı **+0,323** (%95 CI [+0,300, +0,347]).

**Sınır bulgusu:** %95 kayıpta (p3) hiçbir sistem 120 s içinde anlamlı teslim yapamadı (tüm kolların
medyanı 0); küçük ama anlamlı bir Kama farkı (+3,3 puan) ön-kayıtlı iddia eşiğinin (+15 puan) altında
kaldı ve **iddia olarak kullanılmadı**.

**Dürüstlük bulgusu (unadjusted exploratory):** patlamalı-kayıp (Gilbert-Elliott, %40-60) ve 250
kbit/s hücrelerinde Quinn referansları %100'e ulaşırken Kama %86-97'de kaldı. Bu, Kama'nın mevcut
kurtarma politikasının **patlamalı kayıp yapısında** rakibin gerisinde olduğunu gösterir ve ürün yol
haritasında öncelikli iyileştirme kalemidir (§7).

Bu kampanya FEC kolları içermez; bu nedenle bulgular yalnız **kurtarma / transport-entegrasyon**
farkına atfedilir. "QUIC mimarisine üstünlük" iddiası yapılmamaktadır (bkz. §6 iddia sınırları).

---

## 1. Araştırma Sorusu ve Konumlandırma

Kampanyanın dış-terimli araştırma sorusu (r5 C.3, aynen):

> "The campaign tests whether Kama retains useful message-delivery capability in synthetic
> high-loss/high-delay/asymmetric/sub-kilobit IP channels where preregistered Quinn configurations
> may experience connection-establishment or recovery limitations."

Kama, genel-amaçlı güvenli mesajlaşma sistemleriyle (ör. Signal) rekabet etmeyi hedeflemez; hedef,
bu sistemlerin ve standart QUIC yığınlarının teslim kapasitesini yitirdiği düşman kanal rejimlerinde
**kullanılabilir teslim kapasitesini korumaktır**. Hedef kullanıcı profili: taktik saha birimleri,
insansız platform veri-bağları ve afet haberleşmesi.

## 2. Karşılaştırma Kolları

| kol | tanım |
|---|---|
| **KAMA-TAIL-V1** (primary Kama kolu) | Kama motoru + doğrulanmış sınırlı kuyruk-kurtarma (`validated_v1`) |
| KAMA-ARQ | Kama motoru, kuyruk-kurtarma kapalı (ürün varsayılanı) |
| **Q-DGRAM-ARQ** (primary karşılaştırıcı) | Quinn 0.11.11 DATAGRAM + uygulama-ARQ (PTO tabanlı, politika ön-kayıtlı); FEC'siz semantik-eşlenik referans |
| Q-STREAM | Quinn 0.11.11, mesaj-başı güvenilir stream ("standart QUIC kullanımı") |

Quinn konfigürasyonları, bilinçli dezavantaj yaratmayacak şekilde ön-kayıtla sabitlendi ("reference
config preregistered to avoid knowingly disadvantaging"): PMTUD kapalı, sabit MTU, GSO kapalı,
300 s idle, stream kredisi 512; uygulama-ARQ politikası ve uygulama-ACK'i Kama'nın alıcı ACK
kadansını kaynaktan aynalar (N=2 + 20 ms).

## 3. Yöntem (özet; tam tanım KAPI-1/KAPI-2)

- **Düzlem A2 (established, steady-state):** oturum temiz kanalda kurulur → bozulum uygulanır →
  aynı-sınıf ısınma trafiği (ölçüm dışı) → ortak barrier → t=0. Handshake ve ısınma byte'ları
  bütçeye girmez; transport durumu sıfırlanmaz (yalnız sayaçlar).
- **Eşit kanal-türevli bütçe:** `B = C × u / 8` hızında token-bucket (u = 0,75), paket-atomik
  hard-cap, iki yön ayrı, iki transporta özdeş governor. Bütçe hiçbir transportun tüketiminden
  türetilmez.
- **Wire muhasebesi (r5 H, aynen):** "The primary byte budget measures IP bytes injected by the
  endpoints into the emulated channel, including transmissions subsequently erased by the
  impairment model; it is not a measurement of physical RF airtime."
- **Kanal:** üç-namespace veth topolojisi, netem orta namespace'te (uç-egress yakalama yapısal
  olarak bozulum-öncesi), statik ARP pinleme, gerçekleşen-kaybın koşu-başına doğrulanması.
- **İstatistik:** n = tekrar sayısı (mesaj-bazlı örneklem yok); eşleştirilmiş tasarım (aynı hücre×
  tekrar); 10 000 yeniden-örneklemeli paired-bootstrap %95 CI; üç primary hücrede Holm düzeltmesi;
  başarı kuralı: Holm-sonrası anlamlı ∧ CI>0 ∧ etki ≥ +15 puan. Tohumlar, konfigürasyon
  dondurulduktan **sonra** drand kamu-rastgeleliğinden türetildi (Model A kör-ayrım).
- **Ölçek:** kalibrasyon 240 koşu; evaluation 960 koşu (primary n = 50/30/60); iki koşu ön-kayıtlı
  gerçekleşen-kayıp toleransıyla geçersiz sayıldı (ikincil hücrelerde; primary etkilenmedi).

## 4. Primary Sonuçlar (ön-kayıtlı; kama-tail-v1 − q-dgram-arq)

| kanal | n | Δ completion | %95 CI | Holm-p | hüküm |
|---|---|---|---|---|---|
| p2: %80 kayıp, 200 kbit/s | 50 | **+0,384** | [+0,315, +0,449] | 0,0003 | **KAMA ÜSTÜN** |
| p3: %95 kayıp, 100 kbit/s | 30 | +0,033 | [+0,008, +0,062] | 0,0062 | ayrışma var, iddia yok |
| p5: ileri %5 / geri %80 | 60 | +0,120 | [+0,069, +0,177] | 0,0003 | ayrışma var, iddia yok |

Medyan completion (D = 120 s, paced 1 mesaj/s, 100 × 256 B):

| kanal | kama-tail-v1 | kama-arq | q-dgram-arq | q-stream |
|---|---|---|---|---|
| p2 | **0,505** | 0,495 | 0,040 | 0,020 |
| p3 | 0,000 | 0,000 | 0,000 | 0,000 |
| p5 | 1,000 | 1,000 | 1,000 | 1,000 |

İkincil maliyet ucu (byte-verimi = teslim-payload / enjekte-IP-byte; eşleştirilmiş fark):
p2 +0,054 [+0,040, +0,068] · p3 +0,002 [−0,009, +0,011] · **p5 +0,323 [+0,300, +0,347]**.

### İzinli iddia cümlesi (FAZ-1)

> Kama, preregistered Quinn 0.11.11 Q-STREAM ve Q-DGRAM-ARQ baseline'larını p2 profilinde eşit
> kanal-türevli IP-byte bütçesinde, Holm-düzeltmeli ön-kayıtlı testle geçti (+38,4 puan, %95 CI
> [+31,5, +44,9]). Bu sonuç yalnız kurtarma / transport-entegrasyon farkına atfedilir.

## 5. Sağlamlık Yüzeyi (ön-kayıtlı sekonder eksenler — *unadjusted exploratory*, iddia taşımaz)

| eksen | hücre | Δ (ktv1 − qd) | not |
|---|---|---|---|
| burst workload | p2 | +0,64 | yön korunur, etki büyür |
| geçiş-şoku düzlemi (A1) | p2 / p3 / p5 | +0,48 / +0,24 / +0,12 | A1'de Kama %95 kayıpta bile %23 teslim (temiz-kanal yakınsama etkisi) |
| bütçe seviyesi | p2, u = 0,50 / 0,90 | +0,27 / +0,43 | bütçe yarıya inince de yön korunur |
| sabit-byte, değişen-süre | p2, D = 60 / 300 | +0,34 / +0,52 | fark süreyle büyür |
| burst workload | p5 | −0,11 | yüksek-varyans hücre; medyanlar tavanda |

### Exploratory Grup-B (kısmi, 156/360 koşu; *unadjusted*)

| kanal | kama-tail-v1 | kama-arq | q-dgram-arq | q-stream |
|---|---|---|---|---|
| h2: 8 kbit/s, %25 kayıp, jitter | 1,000 | 0,967 | 1,000 | 1,000 |
| h3a: 250 kbit/s, GE-burst %40 | 0,970 | 0,910 | **1,000** | **1,000** |
| h3b: 250 kbit/s, GE-burst %50 | 0,920 | 0,860 | **1,000** | **1,000** |
| h3c: 250 kbit/s, GE-burst %60 | 0,920 | 0,860 | **1,000** | **1,000** |

**Patlamalı-kayıp rejiminde Kama referansların gerisinde** (Δ −2,5 … −8 puan; discordant çiftlerin
tamamı Quinn lehine). Byte-verimi Kama lehine kalsa da (h3'te ~0,23-0,35 vs ~0,17-0,25), teslim
oranı önceliklidir. Bu bulgu §7'deki ürün önceliğini belirler.

## 6. İddia Sınırları ve Sınırlılıklar

1. **FEC kolları yok (FAZ-1):** "QUIC mimarisine üstünlük" iddiası **yapılmaz**; bunun tek yolu
   aynı-implementasyonlu FEC-izolasyon ve bağımsız FEC-referans kollarının birlikte geçilmesidir
   (FAZ-2, conjunctive kural).
2. **Kripto:** Kama veri-çerçevesi gerçek AEAD değil, +16 B byte-eşdeğer maket; CPU/throughput
   iddiası yapılmaz.
3. **Aynı-host veth emülasyonu:** "loopback değil ama bağımsız fiziksel host da değil"; RF kanalı
   temsil iddiası yok (netem modeli).
4. **Isınma artığı:** ağır kayıpta t=0'da teslim edilmemiş ısınma birimleri kalır; tüm kollarda
   simetrik ölçülüp raporlandı.
5. **Kuyruk-kurtarma payı:** rastgele-kayıp rejiminde kama-arq ≈ kama-tail-v1; ayrımın taşıyıcısı
   motorun ARQ/pacing/rate bütünüdür. Kuyruk-kurtarmanın doğrulanmış alanı kuyruk senaryolarıdır.
6. **Karşılaştırıcı ARQ'su bizim yazımımız** (politika ön-kayıtlı, ACK kadansı kaynaktan aynalı).
7. **Exploratory kısmi** (makine kullanılamazlığı); sub-kbps hücreler ertelendi.

## 7. Ürün Yol Haritasına Etkisi

- **Patlamalı-kayıp kurtarma önceliği:** h3 bulgusu, mevcut ARQ/pacing politikasının burst-erasure
  yapısında yetersiz kaldığını gösterir. Bankadaki kanıt (füzyon hücresi, yapı-estimator'ü) yapıya
  duyarlı FEC/ARQ birleşiminin yolunu gösteriyor; ürün-öncesi kalibrasyon kalemi.
- **Varsayılan-açma kararları:** kuyruk-kurtarma (tail-v1) rastgele-kayıpta nötr, kuyruk
  senaryolarında doğrulanmış → ürün varsayılanı olarak açılabilir (karar Abkarada).
- **Ürünleşme:** gerçek Noise handshake, üretim UDP çalışma-zamanı, yaşam-döngüsü, süreç-düzeyi e2e
  (bkz. `docs/roadmap/BITIRME-YOL-HARITASI-2026-08-18.md` §PRODUCT-GRADE).

## Ek A — Tekrarlanabilirlik

Freeze commit `775750a2`; binary sha256 `provenance.txt`; drand round `DRAND.txt`; manifest 240
satır; analiz `scripts/netem/kapi_analyze.py` + `kapi_eval_stats.py` (deterministik, byte-eş
tekrar). Ham JSONL ağaçları ve gerçekleşen-kanal kayıtları repoda; capture katmanı SHA-256 mühürlü.
