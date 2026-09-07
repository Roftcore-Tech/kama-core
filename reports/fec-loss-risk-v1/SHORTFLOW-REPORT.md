# FEC-LOSS-RISK v1 — KISA-AKIŞ DOĞRULAMASI

**Amaç:** `fec_loss_risk`'in VAROLUŞ GEREKÇESİNİ sınamak — *"akış, kanonik onay ufkundan önce
bitiyorsa FEC hiç kullanılabilir bilgi görmüyor"* hipotezi.
**Taban commit:** `6f9cb6db` · **Dal:** `dev/product-transport-fusion`
**Ham veri:** `shortflow-runs.tar.gz` · **Tablolar:** `SHORTFLOW-ANALYSIS.txt`
**Koşucu:** `scripts/netem/fec_risk_shortflow.sh` · **Çözümleyici:** `..._analyze.py`

Bu görevde DEĞİŞTİRİLMEDİ: Q18 dizleri · fuzzy kalibrasyonu · Suspected ağırlıkları ·
`EAI_RETURN_BW_PRIOR` · onay ufku · retransmit eşikleri · CC · pacing · wire · ACK/SACK ·
FEC çerçevesi · deadline politikası · alıcı davranışı. Spec amendment YAZILMADI.

---

## 1. Konfigürasyon ve tekrar sayısı

```
kama-product · profil 0x03 (K=8) · 256 B yük · adaptif FEC (Controller + yapı-gate)
96 mesaj · 20 ms sunum aralığı · max-deadline 20 s · loss_confirm_k = 0 (SEVK ufku)
RTT 200 ms · 1 mbit · reorder YOK · geri yön %2 kayıp
kayıp: 0 / 2 / 5 / 10 / 15 / 20 / 25 %      kol: --fec-risk OFF vs ON
n = 5 rep/hücre · 7 kayıp × 2 kol × 5 = 70 koşu · rep sırası ARALIKLI
```

Önceki (400 mesajlık) ailelerden **tek fark mesaj sayısıdır**; kıyaslanabilirlik korundu.

Enstrümantasyon eklendi: `ClientStats.flow_end_ns` — ana döngü çıkış anı, sender damgalarıyla
**aynı saat tabanında** (`elapsed_ns(start)`). Akış başlangıcı bu tabanda tanım gereği 0'dır.
Olmayan olay `asla` sentinel'i taşır; yapay büyük damgaya ÇEVRİLMEZ ve medyanlardan dışlanır
(ama oranı ayrıca raporlanır).

## 2. Çekirdek sonuç — hipotez DOĞRULANDI

| kayıp | kol | n | risk>0 | **onay>0** | parity>0 | risk<son | onay<son | **PAR<SON** | med_süre | med_risk | med_par | risk_ppm | par_B | tx_B |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 % | OFF | 5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 2117 | asla | asla | 0 | 0 | 42 918 |
| 0 % | ON | 5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 2117 | asla | asla | **0** | **0** | **42 918** |
| 2 % | OFF | 5 | 5/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 2117 | asla | asla | 0 | 0 | 43 519 |
| 2 % | ON | 5 | 5/5 | 0/5 | 0/5 | **5/5** | 0/5 | 0/5 | 2269 | 527 | asla | 11 666 | 0 | 43 646 |
| 5 % | OFF | 5 | 5/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 2557 | asla | asla | 0 | 0 | 45 057 |
| 5 % | ON | 5 | 5/5 | 0/5 | 0/5 | **5/5** | 0/5 | 0/5 | 2318 | 435 | asla | 35 000 | 0 | 45 248 |
| 10 % | OFF | 5 | 5/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 2718 | asla | asla | 0 | 0 | 47 824 |
| 10 % | ON | 5 | 5/5 | 0/5 | 0/5 | **5/5** | 0/5 | 0/5 | 2319 | 430 | asla | 74 375 | 0 | 47 669 |
| 15 % | OFF | 5 | 5/5 | 0/5 | **0/5** | 0/5 | 0/5 | 0/5 | 2738 | asla | asla | 0 | 0 | 49 760 |
| 15 % | ON | 5 | 5/5 | 0/5 | **1/5** | 5/5 | 0/5 | **1/5** | 2398 | 424 | 1560 | 107 916 | 0 | 50 937 |
| 20 % | OFF | 5 | 5/5 | 0/5 | **0/5** | 0/5 | 0/5 | 0/5 | 2915 | asla | asla | 0 | 0 | 53 450 |
| 20 % | ON | 5 | 5/5 | 0/5 | **3/5** | 5/5 | 0/5 | **3/5** | 2596 | 425 | 1141 | 137 083 | 1 840 | 54 801 |
| 25 % | OFF | 5 | 5/5 | **1/5** | **0/5** | 0/5 | 1/5 | 0/5 | 3155 | asla | asla | 0 | 0 | 56 804 |
| 25 % | ON | 5 | 5/5 | 0/5 | **5/5** | 5/5 | 0/5 | **5/5** | 2495 | 427 | 1434 | 188 125 | 2 576 | 59 728 |

Süreler ms. Rep-başına ham tablo `SHORTFLOW-ANALYSIS.txt`'te; parity basmayan koşular
ORTALAMA İÇİNDE ERİTİLMEDİ, `parity>0` sütununda sayılır.

### En keskin iki olgu

**(1) Kanonik onay kısa akışta neredeyse HİÇ ateşlemiyor.** 70 koşunun **69'unda** onay akış
bitmeden gelmedi (`onay>0` sütunu 13 hücrede 0/5; tek istisna 25 % OFF rep5, akış 4257 ms
sürdü ve onay 3580 ms'de geldi). Kısa akışta ARQ-onaylı kayıp sinyali FEC için yok hükmündedir.

**(2) Sevk edilen kol hiçbir kayıp seviyesinde parity BASMADI.** OFF kolunda 7 kayıp seviyesi ×
5 rep = **35 koşunun 35'inde parity = 0**, %25 kayıp dahil. Kısa akışta mevcut adaptif FEC yolu
parity üretmeye YAPISAL OLARAK muktedir değil.

## 3. Zorunlu kıyaslar

**Kıyas 1 — tahminci gecikmesi** `t_risk − t_kuşku`:
n = 30, **min = medyan = max = 0.000 ms**. Sıfır ek gecikme; istatistiksel değil yapısal.

**Kıyas 2 — kanonik onay ufku** `t_onay − t_kuşku`: n = 1 (tek ateşleme), **3153 ms** — önceki
ailelerdeki 3149–3153 ms ile aynı. **ARQ onay semantiği değişmedi.**

**Kıyas 3 — kısa-akış kanıtı** `risk < akış_sonu < onay`: kayıplı ON koşularının **30/30**'unda
(%100) gözlendi. Tahmincinin varoluş gerekçesi doğrudan kanıtlanmıştır.

**Kıyas 4 — fiili FEC aktivasyonu** (OFF → ON, ilk parity):

| kayıp | OFF | ON | ON medyan ilk parity |
|---|---|---|---|
| 15 % | **asla** (0/5) | 1/5 | 1560 ms (akış sonu 2398 ms) |
| 20 % | **asla** (0/5) | 3/5 | 1141 ms (akış sonu 2596 ms) |
| 25 % | **asla** (0/5) | 5/5 | 1434 ms (akış sonu 2495 ms) |

ON'da parity basılan her koşuda parity **akış bitmeden** çıktı (`PAR<SON` = `parity>0`).

## 4. Sorulara doğrudan cevaplar

**Hangi kayıp seviyeleri risk > 0 üretti:** 2, 5, 10, 15, 20, 25 % → **hepsi** (5/5 rep).
%0'da risk 0 (5/5).

**Hangi kayıp seviyeleri fiili parity üretti:** yalnız **15 / 20 / 25 %**, ve yalnız ON kolunda.

**Kayıp seviyesi başına risk_ppm (bir sonraki kalibrasyon görevinin GİRDİSİ):**

```
 2 % →  11 666 ppm      15 % → 107 916 ppm
 5 % →  35 000 ppm      20 % → 137 083 ppm
10 % →  74 375 ppm      25 % → 188 125 ppm
```

Fuzzy'nin `fec > 0` için istediği ≈200 200 ppm eşiğine göre: **%25 kayıpta bile ölçülen risk
o eşiğin ALTINDA** (188 125). %15–25'te parity yine de çıktı — demek ki o koşularda parity'yi
üreten fuzzy'nin kendi dizi değil, **yapı-gate**tir (kayıp önkoşulu 100 000 ppm). Bu, bir
sonraki kalibrasyon görevinin dikkate alması gereken bir ayrımdır.

**Temiz yol ek yükü var mı:** **YOK.** %0'da risk 0/5, parity 0/5, ve toplam tx rep-başına
BİREBİR aynı (her iki kolda da {42 918 ×4, 42 867 ×1}). **Vaka D tetiklenmedi.**

## 5. Hüküm — Vaka A ve Vaka B, kayıp rejimine göre

* **%15–25 → Vaka A (güçlü doğrulama).** Parity *asla*'dan akış-ömrü-içine taşındı
  (0/5 → 1/5, 3/5, 5/5). Gözlenebilirlik sorunu bu rejimde çözüldü.
* **%2–10 → Vaka B (tahminci görünür, politika engelliyor).** Risk 5/5 rep'te ve akış hâlâ
  canlıyken üretildi (~430–530 ms), ama parity 0/5 kaldı. Bu **tahminci başarısızlığı DEĞİLDİR**;
  tahminci artık darboğaz değildir, Q18/controller kalibrasyonu darboğazdır.
* **Vaka C tetiklenmedi** — risk hiçbir hücrede geç kalmadı (`t_risk = t_kuşku`, medyan 424–527 ms,
  akış süresi 2117–3155 ms).
* **Vaka D tetiklenmedi** — temiz yolda sıfır ek yük.

**Sonuç: kanıt Q18/FEC politika kalibrasyonuna geçmeyi DESTEKLİYOR.**

## 6. Ölçülmeyenler / kısıtlar

* Tek RTT (200 ms), tek bant (1 mbit), tek yük (256 B), tek profil (0x03/K=8), reorder yok.
* n = 5; %15 ve %20'deki kısmi parity oranları (1/5, 3/5) küçük-n'dir, nokta tahmini değildir.
* `t_onay − t_kuşku` kıyası n = 1'e dayanır (kısa akışta onay neredeyse hiç ateşlemediği için).
  Ufkun değişmediğine dair asıl kanıt önceki ailedeki n = 30+ ölçümü ve `diag_confirm_path`
  eşitlik testidir.
* Kalibre politika kolu (`--fec-mode policy`) koşulmadı; aile adaptif kolla koştu.
* Reorder'lı kısa akış koşulmadı (önceki ailede 400 mesajla ölçüldü).

## 7. Ayrı raporlanan gözlem (bu görevde DÜZELTİLMEDİ)

`%25` kayıpta ölçülen risk (188 125 ppm) fuzzy'nin `fec > 0` dizinin (≈200 200 ppm) altında
kalıyor; parity yalnız yapı-gate'in 100 000 ppm önkoşulu üzerinden çıkıyor. Yani ürünün iki FEC
kapısı farklı eşiklerde çalışıyor ve yüksek kayıpta fiilen karar veren **fuzzy değil gate**.
Bu bir kalibrasyon bulgusudur; talimat gereği bu görevde değiştirilmedi.


---

# ERRATUM — 2026-09-05 (adversaryal denetim sonrası)

**§9-b / §9-c "≈200 200 ppm Q18 dizi" cümleleri bu bant rejiminde YANLIŞ.** Denetim (fusion.rs
formül replikasyonu + 106 koşunun yeniden ayrıştırılması): ölçülen 1 mbit hücresinde BtlBw ≤ 50 kB/s
→ fuzzy'nin `bw.med ≈ 0` → R3 kuralı ölü → **hiçbir ölçülen koşuda fuzzy `fec > 0` üretmedi.**
Parity üreten her koşuda oran tam **500‰ = fec_med = YAPI-GATE** (100 000 ppm önkoşulu).
Yani: "risk 195k'ya çıktı ama parity üretmedi çünkü dizler yüksek" cümlesi doğru mekanizmayı
yanlış katmana atfediyor — engel fuzzy dizi değil, gate'in 100k önkoşulu + fuzzy'nin bant üyeliği.

**§9-a "sevk yolu güvenilmez"** stall ile karışık: ilk-kuşku zamanı bimodal (≈425 ms
packet-threshold vs ≈3 134 ms = 1.5×65 B/31 B·s⁻¹ cadence tabanı) + aykırılar (3 706/4 920/
6 999 ms); parity basmayan OFF koşuları bu stall sınıfındandır. Gecikme/tepki iddialarında
`t_susp > 1 s` koşuları ayrı raporlanmalıdır.

**"reorder 195k vs loss 137k"** tepe-vs-plato ve yükler-arası kıyastı; ham tepeler 100 000 /
290 666 (n=2), akış sonunda `risk_last = 0`. Tepe ayırt edici istatistik değildir.
