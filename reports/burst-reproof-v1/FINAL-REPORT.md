# V1 — Burst Re-Proof: FINAL RAPOR (dondurulmuş analiz)

**Tarih:** 2026-09-03 · **Ön-kayıt:** `PRE-REGISTRATION.md` (2026-08-26, sonuç görülmeden donduruldu)
**Statü:** KAMPANYA TAMAMLANDI — 120/120 rep-kol, **0 INVALID (exogenous), 0 MISSING**.

---

## 0. HÜKÜM (ön-kayıtlı karar kuralı, değiştirilmedi)

> Karar kuralı (PRE-REGISTRATION §3): CI alt-sınır ≥ 0 → NOT-WORSE · CI üst-sınır < 0 → **STILL-WORSE**
> · 0'ı kapsıyor → INCONCLUSIVE @ n=10.

| profil (GE, 250 kbit/s) | n | Δ̂ (kama-tail-v1 − q-dgram-arq) | medyan Δ | %95 CI (paired-bootstrap) | boot-p | **HÜKÜM** |
|---|---|---|---|---|---|---|
| h3a burst40 | 10 | **−0,0220** | −0,0200 | [−0,0360, −0,0090] | 0,0004 | **STILL-WORSE** |
| h3b burst50 | 10 | **−0,0800** | −0,0100 | [−0,2150, −0,0060] | 0,0001 | **STILL-WORSE** |
| h3c burst60 | 10 | **−0,0110** | −0,0100 | [−0,0180, −0,0050] | 0,0002 | **STILL-WORSE** |

**Ön-kayıtlı sonuç cümlesi:** *"STILL-WORSE — ARQ-fix yetersiz → V2 (FEC) gerekli."*
P0 ARQ-fix'i patlamalı-kayıp açığını **belirgin biçimde daralttı ama KAPATMADI**; sevk edilen Kama
(kama-tail-v1) bu üç jammer/EW-imzalı hücrede quinn'in ölçülebilir biçimde gerisindedir.

Yöntem: bootstrap `scripts/netem/kapi_eval_stats.py::boot` fonksiyonundan **import edildi**
(yeniden yazılmadı) — B=10.000, tohum = `SHA256("KAPI2-BOOT" ‖ cell_id)`; analiz komparatörü
`scripts/netem/burst_reproof_stats.py` **repoda izlenir** (evidence-hijyeni: komparatör /tmp'de bırakılmaz).
Holm düzeltmesi bu ön-kayıtta YOKTUR (kapi-eval-v1'den farklı); karar yalnız CI tabanlıdır.

---

## 1. Büyüklük — hüküm "geride" diyor, ama ne kadar geride?

Medyan completion (100 mesaj, D=120 s):

| profil | kama-tail-v1 | kama-arq | q-dgram-arq | q-stream |
|---|---|---|---|---|
| h3a | 0,980 | 0,975 | 1,000 | 1,000 |
| h3b | 0,990 | 0,960 | 1,000 | 1,000 |
| h3c | 0,990 | 0,970 | 1,000 | 0,999* |

**Tipik koşuda fark 1–2 puana inmiştir** (ön-kayıt-öncesi banked exploratory'de medyan 0,92–0,97 idi).
Ortalama farkın h3b'de −0,080'e çıkmasının sebebi tek bir çöken tekrardır (§3). Yani: *"quinn açık ara
kazanıyor"* tablosu artık geçerli DEĞİL; ama **ön-kayıtlı bar "eşitlik" idi ve o bar geçilemedi** —
quinn bu rejimde 1,000'de kusursuz, Kama ise nadiren çöküyor.

*q-stream h3c'de bir koşuda 0,99.

---

## 2. İKİNCİL (ön-kayıtlı, bağlam — hipotez üretmez)

### 2.1 P0 fix'inin ölçülen etkisi (eşli POST − PRE, AYNI rep+seed)

| profil | kama-tail-v1 Δ̂ | %95 CI | kama-arq Δ̂ | %95 CI |
|---|---|---|---|---|
| h3a | +0,0850 | [−0,0040, +0,2140] | +0,0500 | [+0,0150, +0,0810] |
| h3b | −0,0130 | [−0,1470, +0,0700] | +0,0190 | [−0,1370, +0,1150] |
| h3c | +0,0656 | **[+0,0378, +0,0911]** | +0,0844 | [−0,2089, +0,3256] |

Medyan bazında fix her hücrede pozitif (h3a +0,015 · h3b +0,050 · h3c +0,070); CI yalnız h3c/tail-ON'da
sıfırı dışlıyor. Fix işe yaramış ama etkisi **çöken-tekrar gürültüsüyle örtülüyor** (§3).

### 2.2 Maliyet (byte-verimi = teslim-payload / enjekte-IP-byte) — Kama ÜSTÜN

| profil | Δ̂ (kama-tail-v1 − q-dgram-arq) | %95 CI |
|---|---|---|
| h3a | **+0,0331** | [+0,0143, +0,0522] |
| h3b | **+0,0549** | [+0,0247, +0,0873] |
| h3c | **+0,0526** | [+0,0368, +0,0697] |

Üç hücrede de Kama aynı kanalda **daha az byte harcayarak** teslim ediyor. Ön-kayıt gereği bu tek başına
iddia taşımaz (teslim oranı önceliklidir), ama trade-off'un yönünü kaydeder.

### 2.3 Tail katkısı (kama-tail-v1 − kama-arq)

h3a +0,013 [−0,003, +0,030] · h3b +0,020 [−0,179, +0,222] · **h3c +0,120 [+0,006, +0,319]**.
Kuyruk-kurtarma patlamalı kayıpta da yardımcı; h3c'de (en şiddetli burst) CI sıfırı dışlıyor.

---

## 3. Kalan arıza modu: nadir ama tam çöken tekrarlar (YENİ BULGU — teşhis edilmedi)

Post-fix rep dağılımı (h3b):

```
kama-tail-v1  0.99 0.99 0.94 1.00 1.00 0.95 0.99 1.00 1.00 0.34   ← rep9 çöküyor
kama-arq      0.96 0.95 1.00 0.96 0.30 0.97 0.95 0.92 1.00 0.99   ← rep4 çöküyor
q-dgram-arq   1.00 1.00 1.00 1.00 1.00 1.00 1.00 1.00 1.00 1.00   ← hiç çökmüyor
```

- h3b rep9 (kama-tail-v1): PRE 0,91 → POST 0,34 (**−0,57**). Aynı seed'de fix ÖNCESİ sağlıklıydı.
- h3b rep4 (kama-arq): PRE 0,93 → POST 0,30. h3c rep3 (kama-arq): −0,88.
- Buna karşılık PRE tarafında da çöken tekrarlar vardı (h3a'da min 0,41 → POST 0,94'e düzeldi).

**Okuma:** çöküş olayları fix ile yok olmadı, **yer değiştirdi** (seed'e/zamanlamaya bağlı, düşük
frekanslı, yüksek şiddetli). n=10'da tek çöküş ortalamayı ~7 puan aşağı çeker — h3b'nin geniş CI'sının
([−0,215, −0,006]) kaynağı budur. quinn bu rejimde hiç çökmüyor; Kama'nın **varyans profili** hâlâ farklı.

Bu bulgu bu turda **teşhis EDİLMEDİ ve düzeltilmedi** (dondurulmuş kampanya; sonuç-güdümlü tuning
yasak). Ayrı bir teşhis kalemi olarak kaydedilir: *"burst rejiminde nadir-çöküş (rep-düzeyi ~%0,3–0,4
completion) — kök-neden bilinmiyor; unit-izi ile aranmalı (P0'ın kullandığı yöntem)."*

---

## 4. Bu rapor NEYİ lisanslar, neyi lisanslamaz

**Lisanslar:**
- "P0 ARQ-fix'i sonrası sevk Kama, GE-burst h3a/b/c hücrelerinde q-dgram-arq'ın **hâlâ gerisindedir**
  (ön-kayıtlı paired-bootstrap, üç hücrede de CI üst-sınırı < 0)."
- "Tipik (medyan) fark 1–2 puana inmiştir; ortalama fark çöken tekrarlar tarafından sürülür."
- "Aynı hücrelerde Kama byte-verimi bakımından üstündür."
- Ön-kayıtlı yönlendirme: **V2 = FEC ileri-kurtarma yolu**, bu rejim için endike.

**Lisanslamaz:**
- "Kama burst'te quinn'i yakaladı/geçti" — HAYIR (hüküm STILL-WORSE).
- "FEC bunu çözer" — bu kampanyada FEC kolu YOKTUR. S1-GATE screening'i (ayrı, ön-kayıtlı-eş DEĞİL)
  static-r4'ün h3b'de 5/5 %100 verdiğini gösterdi; bu bir **screening sinyalidir**, kanıt değil.
  Kanıt, ayrı ön-kayıtlı bir V2 kampanyası gerektirir (FEC-ON kolu + aynı GE hücreleri + aynı bütçe).
- Çöküş modunun kök-nedeni hakkında hiçbir şey (teşhis edilmedi).

---

## 5. Provenance / tekrarlanabilirlik

| öğe | değer |
|---|---|
| frozen commit | `f8b7bca2` |
| bench binary sha256 | `fed8a2f8416b8268d6139ac4c279c131cb3518f6dd9386a9d3c80570eb467675` (koşu öncesi `-j2` ile yeniden üretildi, **byte-özdeş** doğrulandı) |
| manifest sha256 | `4b0b1a280dc754a745953e20df60589e9750aee49b32a2f13f6c1394f0298bbf` (değişmedi) |
| harness | `scripts/netem/kapi_calrun.sh` (f8b7bca2'den beri değişmemiş — diff boş) |
| tohumlar | banked exploratory ile AYNI (frozen) → eşli before/after mümkün oldu |
| koşu | 2026-09-03, seri, 120/120 rep-kol; 15 rep-kol önceki (kesilen) oturumdan resume-SKIP ile devralındı |
| geçerlilik | exogenous-only invalidation; **0 INVALID, 0 MISSING** |
| analiz | `scripts/netem/kapi_analyze.py` (agregasyon) + `scripts/netem/burst_reproof_stats.py` (donmuş karar kuralı) |
| çıktı | `out/analysis/{runs,cells,pairs,FINAL-VERDICTS}.csv` |

**Not (kesinti dürüstlük kaydı):** kampanya 2026-08-26'da 15/120 rep-kolda kesilmişti (sürücü yetim
kaldı). Kalan 105 rep-kol 2026-09-03'te AYNI manifest, AYNI seed'ler ve byte-özdeş yeniden üretilmiş
binary ile tamamlandı; resume mekanizması bitmiş satırları yeniden koşmaz (`out.jsonl` varsa SKIP).
İki oturum arasında ürün kodu DEĞİŞMEDİ (binary sha özdeşliği bunun kanıtıdır).
