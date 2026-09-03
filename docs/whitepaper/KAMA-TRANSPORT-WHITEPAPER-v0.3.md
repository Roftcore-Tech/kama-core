# Kama Transport — Düşman Ağlarda Mesaj Teslim Kapasitesi: Mimari ve Ön-Kayıtlı Karşılaştırma Kanıtı

**Sürüm:** v0.3 TASLAK (iç inceleme; dış dağıtım öncesi Abkarada onayı gerekir) · **Tarih:** 2026-09-02
**Bu sürüm v0.2'yi (2026-08-24) genişletir:** mimari/farklılaşma bölümü (§2), P0 patlamalı-kayıp düzeltme eki
(Ek B — v0.2'nin açık vaadi), screening haritası (Ek C), tarihsel-kıyas dürüstlük eki (Ek D) ve iki sayısal
düzeltme (§5 q-stream p5 hücresi; §7'de +0,323 tanım-kapsamı uyarısının güçlendirilmesi).
**Kanıt tabanı:** `reports/kapi-eval-v1/` (ham `ab946a33`, hükümler `1b39a4f0`) · `reports/p0-burst-recovery/` ·
`reports/g1-tail-revalidation/` · `reports/s2-region/` · `reports/dr-arq-product-tail-1/` · main @ `ca65e6cb`.
**Metodoloji otoritesi:** `docs/tasks/v2-benchmark-gorev-r5-kosuya-hazir.md` (r5); tasarım KAPI-1; ön-kayıt KAPI-2.

---

## Yönetici Özeti

Kama, yüksek kayıp / düşük bant / asimetrik IP kanallarında mesaj teslimini korumak için tasarlanmış,
**kurtarma-odaklı** bir transport katmanıdır. Bu belge (i) katmanın tasarım mimarisini ve genel-amaçlı
transportlardan yapısal farklarını, (ii) **ön-kayıtlı**, eşleştirilmiş tasarımlı ve eşit kanal-türevli
byte-bütçeli bir kampanyada iki Quinn 0.11.11 (QUIC) referans konfigürasyonuna karşı ölçülen davranışını
raporlar.

**Ana bulgu (ön-kayıtlı primary; kama-tail-v1 − q-dgram-arq; Holm-düzeltmeli):** %80 rastgele paket kaybı ve
200 kbit/s kanalda (p2), eşit IP-byte bütçesinde, 120 s mesaj-deadline'ıyla `kama-tail-v1` medyan **%50,5**
teslim; primary karşılaştırıcı `q-dgram-arq` **%4,0**. Eşleştirilmiş **ortalama** fark **+38,4 puan** (medyan
**+47,0**), %95 CI **[+31,5, +44,9]**, Holm-p 0,0003 (n=50). Yan kollar (yalnız betimsel): `q-stream` %2,0,
`kama-arq` %49,5.

**İkincil bulgu (p5 asimetrik):** ileri %5 / geri %80 kayıpta medyan completion iki sistemde de 1,00
(completion farkı iddia eşiğinin altında). Ön-kayıtlı ikincil maliyet ucunda eşleştirilmiş fark **+0,323**
[+0,300, +0,347]; `q-dgram-arq` 60 tekrarın 27'sinde ileri-bütçe tavanına çarptı (Q1 0,75, min 0,25).
Bu sayının tanım-kapsamı uyarısı için §7-m6 zorunludur.

**Sınır bulgusu:** %95 kayıpta (p3) hiçbir sistem 120 s içinde anlamlı teslim yapamadı; küçük fark iddia
olarak kullanılmadı.

**Negatif bulgu ve düzeltmesi:** 250 kbit/s Gilbert-Elliott patlamalı-kayıp hücrelerinde (h3a/b/c, gerçekleşen
kayıp ≈%60) Quinn referansları medyan %100 iken `kama-tail-v1` %92-97 — **Kama patlamalı kayıpta geride** (v0.2
bulgusu). Kök-neden birim-izinden kanıtlandı ve P0 düzeltmesi yazıldı (Ek B). **Tam yeniden-kanıt
2026-09-03'te TAMAMLANDI** (`burst-reproof-v1`, 120/120 rep-kol, 0 INVALID, frozen binary byte-özdeş):
dondurulmuş hüküm üç profilde de **STILL-WORSE** (Δ̂ −0,011 / −0,080 / −0,022; üç %95 CI de tamamen
sıfırın altında). Medyan fark 1–2 puana indi (kama-tail-v1 0,98–0,99 vs quinn 1,000) ve Kama aynı
hücrelerde byte-verimi bakımından üstün; ama ön-kayıtlı **"eşitlik" barı geçilemedi** — ortalamayı
nadir ama tam çöken tekrarlar sürüyor (quinn hiç çökmüyor). Ön-kayıtlı yönlendirme: bu rejim için
**V2 (FEC ileri-kurtarma)** endike. Ayrıntı ve sınırlar: Ek B.1.

Bu kampanya FEC kolları içermez; bulgular yalnız **kurtarma / transport-entegrasyon** farkına atfedilir.
"QUIC mimarisine üstünlük" iddiası yapılmamaktadır (§7). TCP/KCP kolları hiç koşulmamıştır; bu belgede
TCP/KCP hakkında hiçbir karşılaştırmalı sayı yoktur.

---

## 1. Araştırma Sorusu ve Konumlandırma

Kampanyanın dış-terimli araştırma sorusu (r5 C.3, aynen):

> "The campaign tests whether Kama retains useful message-delivery capability in synthetic
> high-loss/high-delay/asymmetric/sub-kilobit IP channels where preregistered Quinn configurations
> may experience connection-establishment or recovery limitations."

**Kapsam sınırı:** FAZ-1 yalnız yüksek-kayıp (p2, p3) ve asimetrik (p5) hücrelerini ön-kayıtlı değerlendirdi;
yüksek-gecikme ve sub-kilobit hücreleri FAZ-2'ye ertelendi. Hedef kullanıcı profili: taktik saha birimleri,
insansız platform veri-bağları, afet haberleşmesi. Kama gecikme-odaklı medya transportlarıyla yarışmaz; onların
"geç paket çöptür" dediği yerde "veri ULAŞSIN" der.

## 2. Mimari ve Yapısal Farklılaşma

*Bu bölüm TASARIM düzeyidir: davranışsal her iddia §5-6 ve Ek'lerdeki ölçümlerle sınırlıdır; ölçülmemiş hiçbir
üstünlük ima edilmez. Otorite: `specs/transport-layer-tdd.md` (WHAT) + `specs/adaptive-transport-engine-spec.md`
(HOW; "IMPLEMENTATION DRAFT" statüsünde).*

### 2.1 Üç kurucu invariant

1. **INV-RATE-1 — kayıp, gönderim hızını düşürmez.** Hız yalnız ölçülen teslim-oranından (BtlBw, BBR-tarzı
   delivered-rate) ve kuyruk-gecikmesinden türetilir; kayıp sinyali ayrı sayaçtır ve YALNIZ kurtarma kolunu
   (FEC/ARQ/çoğaltma) besler. Yapısal olarak uygulanır: loss, rate çıktısının girdisi bile değildir; faz-gain'i
   de kayıptan bağımsızdır (golden-kilit: `inv_rate_1_loss_declaration_preserves_phase_gain`). Gerekçe: düşman
   kanalda kayıp ≈ kanalın doğası; kayıpla yavaşlayan her protokol (Reno/CUBIC sınıfı) tam da hedef rejimde
   kendi kendini boğar.
2. **INV-FEC-1 — code-then-encrypt-authenticate-shards.** FEC, plaintext blokta kodlanır; her shard AYRI AEAD
   ile mühürlenir; alıcı önce doğrular, doğrulanmamış byte decoder'a girmez. "Ciphertext üstünde FEC" bilinçli
   olarak reddedilmiştir.
3. **INV-ENGINE-SOC — çift yönlü katman körlüğü.** Transport hiçbir anahtar/AEAD tutmaz, ne taşıdığını bilmez
   (tip düzeyinde: `ct||tag` tek opak blob; ayrı tag alanı derlenmez). Simetrik olarak güvenlik katmanı
   transport'un MÜŞTERİSİDİR: ACK/retry/congestion/NACK'i bilmez. Bu, güvenlik katmanının takılıp-çıkarılabilir
   olmasını sağlar (TLS 1.3 / DTLS / Noise / custom) ve QUIC'in TLS-bütünleşik tasarımından bilinçli ayrılıştır.

### 2.2 Bileşen özeti

Bağlantısız sıcak-yol (el-sıkışmasız SESSION_DATA; oturum kimliği anahtarda, TLS-1.3-benzeri bind) ·
kümülatif SACK-V2 (tek sağ-kalan ACK tüm rx-kümesini onaylar; bant-farkındalıklı kadans: hızlı kanalda ~20 ms,
0,25 kbps'te ~8,3 s) · kayıp = hipotez defteri (Suspected→Confirmed/Spurious/Expired; geç-ACK kanıt şartı;
tombstone ile örnek-sansürü önlenir) · seçici ARQ + sınırlı PTO kuyruk-kurtarma (RFC-9002-esinli, uyum iddiası
yok) · ACK-bağımsız acil ileri-çoğaltma (asimetrik histerezis: koruma anında, geri-çekilme ölçümle) ·
PreConverge cold-start (tahmincilere yazmayan davranışsal prior + geometrik kopya takvimi) · deterministik
fuzzy karar motoru (aynı girdi→aynı çıktı; golden-vektörlü) · Reed-Solomon FEC (blok/simge, mühürlü
DATA-önce-PARITY sırası) · zaman-modeli saat-sınıfları (ZM-1..4: event-count asla zaman vekili değildir;
mutlak-ms sabiti 4-dekat doğrulamasız giremez — motorun 94-mekanizmalık envanteri spec §16.2'de).

### 2.3 Genel-amaçlı transportlardan yapısal farklar (tasarım düzeyi)

| Eksen | TCP / QUIC / KCP yaklaşımı | Kama |
|---|---|---|
| Bağlantı | el sıkışma + idle-timeout | bağlantısız sıcak-yol; alıcı offline olabilir; oturum blackout'ta ölmez |
| Kayba tepki | hız düşür (loss-based/etkilenmiş CC) | hız kayıp-kör; kayıp yalnız kurtarmayı besler (INV-RATE-1) |
| ACK modeli | paket-başı(na yakın) ACK | seyrek kümülatif SACK; ters-yol %80 kayıpta tek sağ-kalan ACK yeter |
| Ters-yol ölümü | ARQ yapısal olarak ACK'e bağımlı | ACK-bağımsız ileri-çoğaltma/parite, her aktif fazdan erişilir |
| Strateji | sabit (hep-ARQ ya da hep-FEC) | ölçüme göre ARQ/FEC/hibrit/hiçbiri (deterministik karar motoru) |
| Kripto | QUIC: TLS transport'a gömülü | katman üstte ve değiştirilebilir; transport kriptoyu görmez |
| Teslim felsefesi | akış/gecikme öncelikli | deadline-sınırlı tamamlama; "geç ama ULAŞAN" |

**Dürüstlük çerçevesi:** bu tablo tasarım niyetini anlatır. Ölçülmüş davranış farkları yalnız §5-6 ve
Ek'lerdedir; iyi ağda (p7, %2 kayıp) Quinn'in önde olduğu ölçülmüş ve kabul edilmiştir — bu, tezin konusu
değildir (TDD §4.4.5 "dürüstlük profili").

## 3. Karşılaştırma Kolları ve Hizmet Semantiği

| kol | tanım |
|---|---|
| **KAMA-TAIL-V1** (primary Kama kolu) | Kama motoru + doğrulanmış sınırlı kuyruk-kurtarma (`validated_v1`) |
| KAMA-ARQ (yan kol) | Kama motoru, kuyruk-kurtarma kapalı (ürün varsayılanı) |
| **Q-DGRAM-ARQ** (primary karşılaştırıcı) | Quinn 0.11.11 DATAGRAM + uygulama-ARQ; FEC'siz semantik-eşlenik referans |
| Q-STREAM (yan kol) | Quinn 0.11.11, mesaj-başı güvenilir stream |

**Q-DGRAM-ARQ politikası (ön-kayıtlı, KAPI-2 §1):** mesaj-başı PTO = `clamp(2×conn.rtt, 100 ms, 10 s)`,
×2 backoff (10 s tavan), mesajın deadline'ına kadar. Uygulama-ACK kadansı Kama alıcı kaynağından aynalanır
(N=2 + 20 ms + R-kota). Quinn pinleri: PMTUD off, MTU 1452, GSO off, 300 s idle, stream kredisi 512 —
"knowingly disadvantaging" yasağı altında ön-kayıtlı referans konfigürasyonu.

Hizmet-semantiği tablosu ve bağlantı-ölümü kuralı (otomatik reconnect yok; iki transporta aynı kural)
v0.2 §2 ile aynıdır (`FINAL-REPORT.md` §96-104).

## 4. Yöntem (özet; tam tanım KAPI-1/KAPI-2)

- **Düzlem A2:** temiz kanalda kurulum → bozulum → ölçüm-dışı impaired ısınma → ortak bariyer → t=0.
  t=0 sonrası ısınma-artığı tüm kollarda bütçeye/ölçüme dahil (`warmup_residue`).
- **Eşit kanal-türevli bütçe:** paced token-bucket (`refill = C·u/8`, u=0,75; paket-atomik hard-cap; iki yön
  ayrı; governor iki transporta özdeş; hiçbir transportun tüketiminden türetilmez).
- **Wire muhasebesi (r5 H, aynen):** "The primary byte budget measures IP bytes injected by the endpoints into
  the emulated channel, including transmissions subsequently erased by the impairment model; it is not a
  measurement of physical RF airtime."
- **Kanal:** üç-namespace veth, netem ORTA namespace'te (uç-egress yakalama yapısal bozulum-öncesi), statik
  ARP pinleme, koşu-başına gerçekleşen-kayıp doğrulaması; INVALID yalnız exogenous nedenlerle.
- **İstatistik:** n = tekrar (mesaj-bazlı örneklem YASAK); eşleştirilmiş tasarım; 10.000 paired-bootstrap CI;
  üç primary hücrede Holm; başarı kuralı Holm-p<0,05 ∧ CI>0 ∧ etki ≥+15 puan. Tohumlar konfigürasyon
  dondurulduktan SONRA drand kamu-rastgeleliğinden (kör-ayrım).

## 5. Primary Sonuçlar (ön-kayıtlı; kama-tail-v1 − q-dgram-arq)

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
| p5 | 1,000 | 1,000 | 1,000 (Q1 0,75; min 0,25; 27/60 rep bütçe-tavanı) | 1,000 (Q1 0,98; min 0,29) |

*(v0.3 düzeltmesi: v0.2'de p5 q-stream hücresi 0,890 yazılmıştı; banked analiz medyanı 1,000'dir
(`out/analysis/cells.csv`) — 0,890 hiçbir banked istatistiğe karşılık gelmiyordu.)*

**İkincil maliyet ucu** — byte-verimi (sıfır-teslim-güvenli `teslim-payload / ileri-governor-enjekte-IP-byte`):
p2 +0,054 [+0,040, +0,068] · p3 +0,002 [−0,009, +0,011] · **p5 +0,323 [+0,300, +0,347]**. Tanım-kapsamı
uyarısı §7-m6'dadır ve bu sayıyla birlikte taşınmak zorundadır.

### İzinli iddia cümlesi (FAZ-1)

> Kama, preregistered Quinn 0.11.11 **Q-DGRAM-ARQ primary comparison baseline**'ını p2 profilinde eşit
> kanal-türevli IP-byte bütçesinde, Holm-düzeltmeli ön-kayıtlı testle geçti (ortalama eşleştirilmiş fark
> +38,4 puan; medyan +47,0; %95 CI [+31,5, +44,9]). Q-STREAM yan kolunda medyan completion 0,505 vs 0,020
> (betimsel; ön-kayıtlı test yok). Bu sonuç yalnız kurtarma / transport-entegrasyon farkına atfedilir;
> "QUIC mimarisine üstünlük" iddiası FAZ-2 (ISOLATION+REFERENCE conjunctive) olmadan yapılmaz.

*Açık kalem: c(t)/Kaplan-Meier ve delivered-payload-vs-injected-byte eğrileri (r5 ana-çıktı) hâlâ üretilmedi.*

## 6. Sağlamlık Yüzeyi ve Exploratory (iddia taşımaz)

v0.2 §5 tablosu aynen geçerlidir: burst-workload p2 +0,64 / p5 −0,11 (yüksek-varyans; tek rep stall'ı teşhis
kuyruğunda) · A1 geçiş-şoku +0,48/+0,24/+0,12 (A1'de %95 kayıpta bile %23 teslim) · bütçe u=0,50/0,90 yön
korunur · D=60/300 fark süreyle büyür. Exploratory Grup-B: h2 eşitlik; **h3a/b/c patlamalı-kayıp NEGATİF**
(Kama 0,92-0,97 vs Quinn 1,00; byte-verimi Kama lehine ama teslim önceliklidir) → Ek B.

> **Scenario labels indicate the motivating operational condition; the impairment models are not claimed to
> reproduce the corresponding RF channel.** (r5 F, aynen.)

## 7. İddia Sınırları ve Sınırlılıklar

1. **FEC kolları yok (FAZ-1)** → mimari-üstünlük iddiası yok; tek yol FAZ-2 conjunctive (hiç koşulmadı).
2. **Kripto maket:** Kama veri-çerçevesi gerçek AEAD değil, +16 B byte-eşdeğer stub; **hiçbir CPU/throughput
   iddiası yapılamaz**; gerçek Noise geldiğinde byte/latency bir miktar artar.
3. **Aynı-host veth emülasyonu**; RF temsil iddiası yok.
4. **Yan-kol testi yok** (kama-tail-v1 vs kama-arq / q-stream yalnız betimsel).
5. **Isınma artığı** ölçülü ve simetrik; doğru betimleme "impaired warm-up, measured residue".
6. **+0,323 byte-verimi tanım-kapsamı (GÜÇLENDİRİLMİŞ UYARI):** ön-kayıtlı tanım toplam-enjeksiyon iken
   raporlanan değer yalnız ileri/istemci-egress governor sayacıyla hesaplıdır; FAZ-2'deki tam-tanım
   yeniden-hesabı bu sayıyı küçültebilir veya işaretini değiştirebilir (spec-audit claims/whitepaper.11).
   Bu sayı, bu uyarı olmadan alıntılanamaz.
7. **Karşılaştırıcı ARQ'su bizim yazımımız** (politika ön-kayıtlı; ACK kadansı kaynaktan aynalı).
8. **Kampanya kesintileri** dürüstlük kaydındadır (üç uyku, bir CLI çökmesi, bir güç kaybı; CLOCK_MONOTONIC).
9. **TCP/KCP kıyası yoktur** — kollar planlandı, hiç inşa edilmedi; bu belgeden TCP/KCP hakkında hiçbir sayı
   türetilemez.
10. **Sürekli-blackout ölçülmedi** (p6 intermittent-proxy; BENCH-01 ikili metriği açık) — ne Kama ne Quinn
    için sürekli-blackout iddiası kurulabilir.
11. **Hüküm merdiveni:** p2 hücre-düzeyi banked üstünlük ≠ ürün-düzeyi değer kanıtı. §4.4 GO/NO-GO kapısı
    açıktır (en güçlü hücre 0,505 < %90 kategorik bar; FEC kapalıyken). İki roadmap hükmü ("kanıt fazı
    kapandı" / "değer önerisi kanıtlanmadı") bu merdivenle bağdaşır ve bu belge ikisini de bu sınırla aktarır.

## 8. Ürün Yol Haritasına Etkisi

- **P0 patlamalı-kayıp düzeltmesi UYGULANDI ve netem'de doğrulandı** (Ek B); kalanlar: kama-arq artığı,
  tam yeniden-kanıt (burst-reproof-v1'in tamamlanması), p5-burst tekil stall teşhisi.
- **Varsayılan-açma:** tail-v1 p2'de nötr; kuyruk senaryolarında ayrı doğrulanmış → default-ON adayı
  (karar Abkarada; bu kampanyada yan-kol testi yok).
- **Ürünleşme:** gerçek Noise handshake (Sprint-6; spec-audit A-kararlarına bağlı), FEC aktivasyonu için
  loss-yapısı girdisi (DR-FUSION-POLICY-1), FAZ-2 conjunctive FEC kampanyası.

## Ek A — Tekrarlanabilirlik

Freeze `775750a2`; execution `5093f4bf` / `d2bee253`; bench binary sha256 `f94dcaf0…` iki partide özdeş;
drand round `DRAND.txt`; manifestler sha'larıyla `provenance.txt`; analiz `kapi_analyze.py` +
`kapi_eval_stats.py` (deterministik). Ham JSONL + gerçekleşen-kanal kayıtları repoda; capture SHA-256 mühürlü.

## Ek B — P0 Patlamalı-Kayıp Düzeltmesi (v0.2'nin vaat ettiği ek)

**Kök-neden (birim-izinden, tahmin değil):** h3b'de teslim edilmeyen 12 birimin 9'unda özdeş imza:
`Data → Retransmit×3 → Duplicate → [koşu sonuna dek sessizlik]`. `arq_retx_cap=3` aşımında tek-atış ileri
kopya planlanıyor; kopya patlamada kaybolunca `queue_losses` birimi TERK ediyordu (~150 s). Bu, "veri ULAŞSIN"
tabanının ihlaliydi — bilinçli trade-off değil, kusur.

**Düzeltme:** Duplicate-kaynaklı kayıpta, birim hâlâ outstanding ise ileri kopya **geometrik self-reschedule**
(×2 backoff, ACK'te iptal, birim-başına ≤1 canlı dup) ile yeniden kurulur. INV-RATE-1'e, kayıp metriğine ve
FEC'e dokunmaz. Deterministik CI regresyon kapısı: `tests/burst_loss_recovery.rs` (Class-A düzeltme-öncesi
FAIL idi; ikisi de artık PASS).

**Netem doğrulaması (gerçek 3-netns, AYNI drand seed'leri, h3b rep0-2):**

| kol | önce | sonra | Δ |
|---|---|---|---|
| kama-tail-v1 | 0,927 | **0,983** | +0,057 (undelivered 7,3→1,7/koşu) |
| kama-arq | 0,910 | 0,917 | +0,007 (ayrı terk-yolu; teşhiste) |
| q-dgram-arq / q-stream (kontrol) | 1,000 | 1,000 | 0 |

Primary kolun q-dgram'a açığı **−7,3 → −1,7 puan.** G1 yeniden-doğrulaması: h3b/h3c tail-ON medyan 1,000
(n=3) ve p2 tail-ON ort. 0,523 ≈ banked 0,505 → **banked +38pp hükmü sarsılmadı** (mühendislik doğrulaması;
ön-kayıtlı test değildir).

### Ek B.1 — Yeniden-kanıt TAMAMLANDI (2026-09-03): hüküm **STILL-WORSE**

v0.3'ün ilk sürümünde "yarıda kalmış ön-kayıt borcu" olarak kaydedilen `burst-reproof-v1` kampanyası
tamamlandı: **120/120 rep-kol, 0 INVALID, 0 MISSING**; binary frozen sha'da byte-özdeş yeniden üretildi;
seed'ler banked exploratory ile aynı. Dondurulmuş karar kuralı (CI alt-sınır ≥ 0 → NOT-WORSE ·
CI üst-sınır < 0 → STILL-WORSE) uygulandı:

| profil | n | Δ̂ (kama-tail-v1 − q-dgram-arq) | %95 CI | hüküm |
|---|---|---|---|---|
| h3a burst40 | 10 | −0,022 | [−0,036, −0,009] | **STILL-WORSE** |
| h3b burst50 | 10 | −0,080 | [−0,215, −0,006] | **STILL-WORSE** |
| h3c burst60 | 10 | −0,011 | [−0,018, −0,005] | **STILL-WORSE** |

**Yani: "patlamalı kayıpta artık geri değiliz" cümlesi KURULAMAZ — hâlâ gerideyiz.** Ancak büyüklük
değişti: medyan completion 0,92–0,93'ten **0,98–0,99'a** çıktı (quinn 1,000), yani tipik koşuda fark
1–2 puana indi; ortalamayı aşağı çeken şey **nadir ama tam çöken tekrarlar**dır (h3b rep9: 0,34 —
diğer dokuz tekrar 0,94–1,00; quinn hiç çökmüyor). Fix'in eşli ölçülen etkisi medyanda her hücrede
pozitif, h3c/tail-ON'da CI sıfırı dışlıyor (+0,066 [+0,038, +0,091]). Aynı hücrelerde Kama **byte-verimi
bakımından üstün** (+0,033…+0,055, üç CI de sıfırın üstünde).

**Ön-kayıtlı yönlendirme:** STILL-WORSE kolu → **V2 = FEC ileri-kurtarma** bu rejim için endike.
Bu kampanyada FEC kolu YOKTU; S1-GATE screening'inin static-r4 ile h3b'de 5/5 %100 vermesi bir
screening sinyalidir, kanıt değil — kanıt ayrı ön-kayıtlı bir V2 kampanyası gerektirir.
Yeni açık kalem: **burst rejiminde nadir-çöküş modu** (kök-neden bilinmiyor; bu turda bilerek teşhis
edilmedi — dondurulmuş kampanyada sonuç-güdümlü tuning yasak). Tam rapor:
`reports/burst-reproof-v1/FINAL-REPORT.md`.

Ön-kaydın çıtası baştan dürüsttü: bu rejimde Quinn zaten ≈1,000 çalışıyor; hedef üstünlük değil,
negatif bulguyu nötrlemekti — ve **nötrlenemedi**.

## Ek C — Screening Haritası (ön-kayıtlı DEĞİL; yalnız yön gösterir)

*Etiket zorunlu: farklı binary'ler/metrikler; formal banka kapi-eval-v1'dir.*

- **%80 çift-yön kayıp @200 kbit (n=5):** quinn 0,0 % (iki rep de 0); Kama-OFF ~%78; Kama-FEC(static-r4) ~%82.
  Server-view (delivered) metrikle iki Kama kolu da ~%88-89 — acked-vs-delivered farkı ters-%80'in onay
  zincirini boğmasından kaynaklanan metrik artefaktıydı (K1 kapanışı). %90 barı bu hücrede hâlâ AÇIK.
- **Quinn'in stokastik ölümü (kayıp×RTT taraması):** %60 kayıp + 150 ms RTT'de quinn 5 koşunun 3'ünde TAM
  SIFIR (kuruluş-ölümü `udp_tx=0` veya kuruluş-sonrası stall); Kama aynı hücrelerde 36-40/40 bandında —
  ancak dürüst kayıt: %60/0 ms hücresinde bir Kama koşusu 30/40'a düştü, yani "her koşuda ≥%88" CÜMLESİ
  YANLIŞTIR ve kullanılmamalıdır (v0.2 sonrası tespit). Doğru çerçeve: **"deterministik sağkalım vs stokastik
  ölüm"** — Kama'nın varyansı dar, quinn'inki all-or-nothing.
- **Yüksek bant + yüksek kayıp + 300 ms (5 Mbit, n=3):** hipotez deadline-bağımlı çıktı — gevşek D=45 s'te bol
  bant Quinn'i çok-turlu ARQ ile kurtarıyor (%40 hücresini Quinn kazanır); gerçek-zaman sınıfında (D=10 s,
  %60 kayıp) Quinn pratikte işlevsiz (6,7/40), Kama 25,4/40 — ve Kama'nın kendi 1/3 tam-sıfırı dürüstçe
  kayıtlıdır. **DR-EST-1 düzeltmesi** (kuruluş-kopyasının PreConverge token kapısında boğulması) bu sınıfın
  Kama tarafını 2/6-tam-sıfırdan 0/6'ya taşıdı (ort. %56→%82; screening).

## Ek D — Tarihsel v1 Kıyası Hakkında Dürüstlük Notu

2026-07-11 tarihli `docs/results/kama-vs-quinn-v1.md` bu programın ilk baş-başa kıyasıdır ve iki kalıcı
ders içerir: (1) ölçüm aracı önce doğrulanır — üç ölçüm-bozan bug (delivered_ns, HoL+ALPN, sınırsız handshake)
sonuçlar okunmadan yakalandı; sınırsız-handshake düzeltmesinin mirası kalıcıdır: **bağlanamama = geçerli
0-completion ölçümüdür ve agregata girer**; (2) v1'in p5 başlığı (1,00 vs 0,00) sonradan **parazit** çıktı —
false-loss fırtınasının beslediği duplication p5'i taşıyordu; parazit söküldükten sonra p5 24/100'e düştü ve
meşru mekanizma ancak TUR-7 "çift-kilit" ile geri kazanıldı (P5-GATE {100×12}). Bu nedenle v1'in p5 sayısı
kullanılamaz; v1'in kalıcı değeri **rejim yapısıdır** (iyi ağda Quinn önde; orta düşmanlıkta Kama kategorik
farkla önde; %95'te iki taraf da tabanda) ve bu yapı r5'te ön-kayıtla yeniden üretilmiştir. v1-dönemi mutlak
sub-kbps sayıları ayrıca ARP-pinleme öncesi enstrüman kontaminasyonu taşır (simetrik olduğundan kıyas yönünü
bozmaz; mutlak değer taşınmaz). v1 dosyasının başına bu retraksiyon bandı eklenmiştir.
