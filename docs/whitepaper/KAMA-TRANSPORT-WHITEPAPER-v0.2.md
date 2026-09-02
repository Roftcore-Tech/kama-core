# Kama Transport — Düşman Ağlarda Mesaj Teslim Kapasitesi: Ön-Kayıtlı Karşılaştırma Kampanyası (FAZ-1)

**Sürüm:** v0.2 TASLAK (iç inceleme; dış dağıtım öncesi Abkarada onayı gerekir) · **Tarih:** 2026-08-24
**Kanıt tabanı:** `reports/kapi-eval-v1/` (ham veri `ab946a33`, hükümler `1b39a4f0`, rapor `d677ab6b`)
**Metodoloji otoritesi:** `docs/tasks/v2-benchmark-gorev-r5-kosuya-hazir.md` (r5); tasarım KAPI-1; ön-kayıt KAPI-2.
**v0.2 notu:** İç iddia-dili denetimi (whitepaper-claim-audit, 36 doğrulanmış bulgu) sonrası revize.

---

## Yönetici Özeti

Kama, yüksek kayıp / düşük bant / asimetrik IP kanallarında mesaj teslimini korumak için tasarlanmış,
kurtarma-odaklı bir transport katmanıdır. Bu belge, Kama'nın **önceden kaydedilmiş (preregistered)**,
eşleştirilmiş tasarımlı ve eşit kanal-türevli byte-bütçeli bir kampanyada, aynı kanal koşullarında
koşan iki Quinn 0.11.11 (QUIC) referans konfigürasyonuna karşı ölçülen davranışını raporlar.

**Ana bulgu (ön-kayıtlı primary; kama-tail-v1 − q-dgram-arq; Holm-düzeltmeli):** %80 rastgele paket
kaybı ve 200 kbit/s kanalda (p2), eşit IP-byte bütçesinde, 120 s mesaj-deadline'ıyla `kama-tail-v1`
medyan **%50,5** teslim; primary karşılaştırıcı `q-dgram-arq` %4,0. Eşleştirilmiş **ortalama** fark
**+38,4 puan** (medyan eşleştirilmiş fark **+47,0 puan**), %95 CI **[+31,5, +44,9]**, Holm-p 0,0003
(n=50). Yan kollar (ön-kayıtta yalnız betimsel, test edilmez): `q-stream` %2,0, `kama-arq` %49,5.

**İkincil bulgu (p5 asimetrik):** ileri %5 / geri %80 kayıp kanalında medyan completion iki sistemde
de 1,00; ön-kayıtlı completion farkı +0,120 [+0,069, +0,177] (iddia eşiği +15 puanın **altında**).
Ön-kayıtlı ikincil maliyet ucunda (byte-verimi; §4 tanımı) eşleştirilmiş fark **+0,323**
[+0,300, +0,347]: `q-dgram-arq` 60 tekrarın **27'sinde** ileri-bütçe tavanına çarptı (completion Q1
0,75, min 0,25), Kama aynı kanalda daha az byte harcadı.

**Sınır bulgusu:** %95 kayıpta (p3) hiçbir sistem 120 s içinde anlamlı teslim yapamadı (tüm kolların
medyanı 0); küçük Kama farkı (+0,033) iddia eşiğinin altında, iddia olarak **kullanılmadı**.

**Negatif bulgu (unadjusted exploratory, kısmi):** 250 kbit/s Gilbert-Elliott patlamalı-kayıp
hücrelerinde (h3a/b/c; gemodel p=0,40/0,50/0,60; gerçekleşen kayıp ≈ %60) Quinn referansları medyan
%100, `kama-tail-v1` %92-97, `kama-arq` %86-91. **Kama patlamalı kayıpta referansların gerisinde.**
Bu, jammer/EW gibi patlamalı-kayıp rejimlerinin doğrudan konusudur; ürün yol haritasında **P0**
öncelikli düzeltme kalemidir (§7). *[Bu belge yazıldıktan sonra teşhis edildi ve düzeltme başladı;
düzeltmenin ampirik doğrulaması ayrı ekte raporlanacaktır.]*

Bu kampanya FEC kolları içermez; bulgular yalnız **kurtarma / transport-entegrasyon** farkına
atfedilir. "QUIC mimarisine üstünlük" iddiası yapılmamaktadır (§6).

---

## 1. Araştırma Sorusu ve Konumlandırma

Kampanyanın dış-terimli araştırma sorusu (r5 C.3, aynen):

> "The campaign tests whether Kama retains useful message-delivery capability in synthetic
> high-loss/high-delay/asymmetric/sub-kilobit IP channels where preregistered Quinn configurations
> may experience connection-establishment or recovery limitations."

**Kapsam sınırı:** FAZ-1 bu rejimlerden yalnız **yüksek-kayıp (p2, p3)** ve **asimetrik (p5)**
hücrelerini ön-kayıtlı olarak değerlendirdi; yüksek-gecikme (h1) ve sub-kilobit (Grup-C) hücreleri
freeze sonrası kararla FAZ-2'ye ertelendi (§6.7). Kama, kurtarma-odaklı bir transport katmanıdır;
hedef, ölçülen düşman kanal rejimlerinde kullanılabilir teslim kapasitesini korumaktır. Hedef
kullanıcı profili: taktik saha birimleri, insansız platform veri-bağları, afet haberleşmesi.

## 2. Karşılaştırma Kolları ve Hizmet Semantiği

| kol | tanım |
|---|---|
| **KAMA-TAIL-V1** (primary Kama kolu) | Kama motoru + doğrulanmış sınırlı kuyruk-kurtarma (`validated_v1`) |
| KAMA-ARQ (yan kol) | Kama motoru, kuyruk-kurtarma kapalı (ürün varsayılanı) |
| **Q-DGRAM-ARQ** (primary karşılaştırıcı) | Quinn 0.11.11 DATAGRAM + uygulama-ARQ; FEC'siz semantik-eşlenik referans |
| Q-STREAM (yan kol) | Quinn 0.11.11, mesaj-başı güvenilir stream |

**Q-DGRAM-ARQ politikası (ön-kayıtlı, KAPI-2 §1):** mesaj-başı PTO = `clamp(2×conn.rtt, 100 ms, 10 s)`,
her yeniden-gönderimde ×2 backoff (10 s tavan), mesajın kendi deadline'ına kadar. Uygulama-ACK'i
Kama alıcı kadansını kaynaktan aynalar: **N=2 yeni DATA + 20 ms timer + R-kota sessizlik-re-ACK**
(KAPI-1 Amendment-4). Quinn pinleri: PMTUD off, sabit MTU 1452, GSO off, 300 s idle, stream kredisi
512 — bilinçli dezavantaj yaratmama kilidi ("reference config preregistered to avoid knowingly
disadvantaging").

**Hizmet-semantiği (r5 C.4; her hücrede geçerli):**

| boyut | KAMA | Q-STREAM | Q-DGRAM-ARQ |
|---|---|---|---|
| güvenilirlik | ARQ (+tail-v1); bütçe/deadline sınırlı | QUIC reliable | app-ARQ (PTO, ×2 backoff); aynı sınır |
| sıralama | yok (unit bağımsız) | stream-içi FIFO (mesaj-başı-stream) | yok |
| mesaj-sınırları | unit=mesaj | stream=mesaj | datagram=mesaj |
| deadline-sonrası | harness D'de keser; geç teslim stale | aynı | aynı |
| dup-suppression | receiver unit_id | QUIC | app msg_id |
| app-ACK (birim/generation/coalescing/frequency) | SACK range / N=2+20ms timer / range-merge / eşik+timer | stream-FIN | kümülatif aralık / N=2+20ms+R-kota / aralık-merge / eşik+timer |
| connection-state | yok (motor durmaz) | QUIC conn (idle 300 s) | aynı |

**Bağlantı ölümü (r5 B.3):** DÜZLEM-A'da otomatik reconnect yok; oturum ölürse kalan mesajlar
deadline'da FAIL, termination-nedeni raporlanır; iki transport aynı kural.

## 3. Yöntem (özet; tam tanım KAPI-1/KAPI-2)

- **Düzlem A2 (established, impaired warm-up):** oturum temiz kanalda kurulur → bozulum uygulanır →
  aynı-sınıf ısınma trafiği (ölçüm dışı, impaired) → ortak barrier → t=0. Handshake ve **t=0 öncesi**
  ısınma byte'ları bütçeye girmez; **t=0 sonrası ısınma-artığı** (teslim edilmemiş ısınma birimlerinin
  retransmit'i) tüm kollarda bütçeye ve ölçüme dahildir (`warmup_residue`; §6.5). Transport durumu
  sıfırlanmaz (yalnız sayaçlar).
- **Eşit kanal-türevli bütçe:** paced token-bucket — `refill = C·u/8 B/s`, `bucket = max(3·MTU, 2 s·refill)`,
  `ceiling = C·W·u/8` (u=0,75). Paket-atomik hard-cap, iki yön ayrı, iki transporta özdeş governor.
  Bütçe hiçbir transportun tüketiminden türetilmez.
- **Wire muhasebesi (r5 H, aynen):** "The primary byte budget measures IP bytes injected by the
  endpoints into the emulated channel, including transmissions subsequently erased by the impairment
  model; it is not a measurement of physical RF airtime."
- **Kanal:** üç-namespace veth topolojisi, netem orta namespace'te (uç-egress yakalama yapısal olarak
  bozulum-öncesi), statik ARP pinleme, gerçekleşen-kaybın koşu-başına doğrulanması.
- **İstatistik:** n = tekrar sayısı (mesaj-bazlı örneklem yok); eşleştirilmiş tasarım; 10 000
  yeniden-örneklemeli paired-bootstrap %95 CI; **üç primary hücrede** Holm düzeltmesi; başarı kuralı:
  Holm-sonrası anlamlı ∧ CI>0 ∧ etki ≥ +15 puan. Yan kollar (kama-arq, q-stream) yalnız betimsel
  raporlanır — paired test/CI/Holm-p HESAPLANMAZ. Tohumlar, konfigürasyon dondurulduktan **sonra**
  drand kamu-rastgeleliğinden türetildi (Model A kör-ayrım).

## 4. Primary Sonuçlar (ön-kayıtlı; kama-tail-v1 − q-dgram-arq)

| kanal | n | ort. Δ | medyan Δ | %95 CI | Holm-p | hüküm |
|---|---|---|---|---|---|---|
| p2: %80 kayıp, 200 kbit/s | 50 | **+0,384** | **+0,470** | [+0,315, +0,449] | 0,0003 | **KAMA ÜSTÜN** |
| p3: %95 kayıp, 100 kbit/s | 30 | +0,033 | 0,000 | [+0,008, +0,062] | 0,0062 | ayrışma var, iddia yok |
| p5: ileri %5 / geri %80 | 60 | +0,120 | 0,000 | [+0,069, +0,177] | 0,0003 | ayrışma var, iddia yok |

Medyan completion (D=120 s, paced 1 mesaj/s, 100×256 B):

| kanal | kama-tail-v1 | kama-arq | q-dgram-arq | q-stream |
|---|---|---|---|---|
| p2 | **0,505** | 0,495 | 0,040 | 0,020 |
| p3 | 0,000 | 0,000 | 0,000 | 0,000 |
| p5 | 1,000 | 1,000 | 1,000 (Q1 0,75; min 0,25; 27/60 rep bütçe-tavanı) | 0,890 |

**İkincil maliyet ucu** — byte-verimi = `teslim-payload / ileri-governor-enjekte-IP-byte`
(client-egress governor sayacı; ters-yön governor sayacı bu orana girmez — kesin tanım, r5 H toplam-
enjeksiyon ile yeniden-hesap FAZ-2 rafine kalemi). Eşleştirilmiş fark: p2 +0,054 [+0,040, +0,068] ·
p3 +0,002 [−0,009, +0,011] · **p5 +0,323 [+0,300, +0,347]**.

### İzinli iddia cümlesi (FAZ-1)

> Kama, preregistered Quinn 0.11.11 **Q-DGRAM-ARQ primary comparison baseline**'ını p2 profilinde eşit
> kanal-türevli IP-byte bütçesinde, Holm-düzeltmeli ön-kayıtlı testle geçti (ortalama eşleştirilmiş
> fark +38,4 puan; medyan +47,0; %95 CI [+31,5, +44,9]). Q-STREAM yan kolunda medyan completion 0,505
> vs 0,020 (betimsel; ön-kayıtlı test yok). Bu sonuç yalnız kurtarma / transport-entegrasyon farkına
> atfedilir; "QUIC mimarisine üstünlük" iddiası FAZ-2 (ISOLATION+REFERENCE conjunctive) olmadan yapılmaz.

*Not: c(t)/Kaplan-Meier completion-vs-zaman eğrileri ve delivered-payload-vs-injected-byte eğrileri
(r5 D.4/ana-çıktı) bu taslakta henüz üretilmedi — dış-rapor öncesi eklenecek (açık kalem).*

## 5. Sağlamlık Yüzeyi (ön-kayıtlı sekonder eksenler — *unadjusted exploratory*, iddia taşımaz)

| eksen | hücre | n | Δ (ktv1−qd) | not |
|---|---|---|---|---|
| burst workload | p2 | 9 | +0,64 | 1 exogenous-INVALID (q-dgram rep6) düşürüldü; yön korunur |
| burst workload | p3 | 10 | +0,03 | taban; tüm medyanlar 0 |
| burst workload | p5 | 9 | −0,11 | 1 exogenous-INVALID (q-dgram rep8) + **tek rep (rep5) kama-tail-v1 0/100 teslim** (gönderici ~4 kB sonra sustu; ayrı stall — teşhis kuyruğunda); diğer 3 kol 1,0 |
| A1 düzlemi (geçiş-şoku) | p2/p3/p5 | 10 | +0,48 / +0,24 / +0,12 | A1'de Kama %95 kayıpta bile %23 teslim (temiz-kanal yakınsama) |
| bütçe seviyesi | p2, u=0,50 | 10 | +0,27 | bütçe u=0,75→0,50 (×0,67) düşürülünce de yön korunur |
| bütçe seviyesi | p2, u=0,90 | 10 | +0,43 | *[u=0,90 hücresinin son 14 koşusu pacman-güncelleme penceresinde karantinaya alındı; değer rep0–rep9 üstünden; karantina dolgusu backlog]* |
| sabit-byte, değişen-süre | p2, D=60 / 300 | 10 | +0,34 / +0,52 | fark süreyle büyür |

### Exploratory Grup-B (kısmi, 157 koşu toplandı / 156 çift-kullanılabilir; *unadjusted*)

| hücre (GE = gemodel p/r/1−h/1−k; gerçekleşen kayıp ≈%60) | n | kama-tail-v1 | kama-arq | q-dgram-arq | q-stream |
|---|---|---|---|---|---|
| h2: 8 kbit/s, %25 kayıp, jitter | 10 | 1,000 | 0,967 | 1,000 | 1,000 |
| h3a: GE(0,40/0,25/0,90/0,10), 250 kbit/s | 10 | 0,970 | 0,910 | **1,000** | **1,000** |
| h3b: GE(0,50/…), 250 kbit/s | 10 | 0,920 | 0,860 | **1,000** | **1,000** |
| h3c: GE(0,60/…), 250 kbit/s | 9 | 0,920 | 0,860 | **1,000** | **1,000** |

> **Scenario labels indicate the motivating operational condition; the impairment models are not
> claimed to reproduce the corresponding RF channel.** (r5 F, aynen.)

h3c'de rep9 MISSING (q-stream koştu, diğer 3 kol koşulmadan güç kaybı; PARTIAL-README). Patlamalı-kayıp
rejiminde `kama-tail-v1` medyanı q-dgram'ın **gerisinde** (h3a/b/c'de 10/10, 10/10, 9/9 tekrarda
Δ ≤ 0). Byte-verimi Kama lehine kalsa da teslim oranı önceliklidir; §7 P0.

## 6. İddia Sınırları ve Sınırlılıklar

1. **FEC kolları yok (FAZ-1):** "QUIC mimarisine üstünlük" iddiası yapılmaz; tek yolu aynı-implementasyonlu
   FEC-izolasyon + bağımsız FEC-referans kollarının birlikte geçilmesidir (FAZ-2, conjunctive kural).
2. **Kripto:** Kama veri-çerçevesi gerçek AEAD değil, +16 B byte-eşdeğer maket; CPU/throughput iddiası yok.
3. **Aynı-host veth emülasyonu:** "loopback değil ama bağımsız fiziksel host da değil"; RF temsil iddiası yok.
4. **Yan-kol testi yok:** kama-tail-v1 vs kama-arq ve vs q-stream ön-kayıtlı paired-test DEĞİLDİR
   (yalnız betimsel medyan). Default-ON önerisi (§7) bu kampanyada test edilmedi; p2 medyanları yakın
   (0,505/0,495) + kuyruk senaryosunda ayrı doğrulama (banked tail-grid) dayanağıdır.
5. **Isınma artığı:** p2'de residue medyanı 9-10/10 birim (ısınmanın çoğu t=0'da teslim edilmemiş);
   tüm kollarda simetrik ölçüldü; post-t0 residue trafiği bütçeye/ölçüme dahil. "Steady-state" yerine
   "impaired warm-up, measured residue" daha doğru betimlemedir.
6. **Karşılaştırıcı ARQ'su bizim yazımımız** (politika ön-kayıtlı, §2; ACK kadansı kaynaktan aynalı).
7. **Kampanya kesintileri (dürüstlük kaydı):** üç makine-uykusu (~0,9 h, ~7,5 h, ~64 dk), bir CLI
   çökmesi (sürücü yetim sürdü), bir güç kaybı, iki reboot; tüm saatler CLOCK_MONOTONIC → ölçüm-çerçevesi
   uykuyu görmez; uykuyu kapsayan 3 koşu tolerans-içi doğrulandı (SUSPEND-NOTE). 756/960 koşu
   realized-loss taramasından geçti (yeterli-örneklemli); 2 exogenous-INVALID. Exploratory F4 partisi
   freeze sonrası kararla 19→9 hücreye daraltıldı (h1 + Grup-C FAZ-2'ye); kalan 9'dan h4a/h4p/h4s/h6×2
   koşulmadı.
8. **c(t)/Kaplan-Meier ve delivered-vs-byte eğrileri** (r5 ana-çıktı) henüz üretilmedi — dış-rapor açık kalemi.

## 7. Ürün Yol Haritasına Etkisi

- **P0 — patlamalı-kayıp kurtarma:** h3 bulgusu, kurtarma yolunda cap-aşımı sonrası tek-atış forward
  kopyasının kaybolunca birimi terk ettiğini gösterdi (birim-izinden doğrulandı). Düzeltme: forward-recovery
  kopyaları geometrik self-reschedule (ACK-ile-iptal) — "veri ulaşsın" tabanının ihlali kapatılır.
  Regresyon kapısı: `tests/burst_loss_recovery.rs` (deterministik, CI'da) + netem h3 doğrulaması.
- **Varsayılan-açma:** kuyruk-kurtarma (tail-v1) p2 medyanında nötr; kuyruk senaryolarında ayrı
  doğrulanmış (banked tail-grid) → default-ON adayı (karar Abkarada; bu kampanyada yan-kol testi yok).
- **Ürünleşme:** gerçek Noise handshake, üretim UDP çalışma-zamanı, yaşam-döngüsü, süreç-düzeyi e2e
  (`docs/roadmap/BITIRME-YOL-HARITASI-2026-08-18.md` §"REVİZYON 2026-08-24 — ÜRÜN-GRADE PLANI").

## Ek A — Tekrarlanabilirlik

Freeze commit `775750a2`; execution commit `5093f4bf` (primary/sekonder), `d2bee253` (exploratory,
freeze sonrası yalnız manifest-üreteci düzenlemesi); bench binary sha256 `f94dcaf0…` her iki partide
özdeş; drand round `DRAND.txt`; manifestler `manifest.jsonl` (240) + `manifest-exploratory.jsonl` (90)
sha'larıyla `provenance.txt`. Analiz `scripts/netem/kapi_analyze.py` + `kapi_eval_stats.py`
(deterministik, byte-eş tekrar). Ham JSONL + gerçekleşen-kanal kayıtları repoda; capture SHA-256 mühürlü.
