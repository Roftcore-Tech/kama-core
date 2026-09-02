# V1 — Burst Re-Proof: Ön-Kayıt (FREEZE-BEFORE-OUTCOME)

**Tarih:** 2026-08-26
**Statü:** DONDU — koşu-öncesi. Sonuç görülmeden hipotez + analiz + karar-kuralı kilitli.
**Kaynak bulgu:** kapi-eval-v1 exploratory h3 (GE-burst) → Kama patlamalı-kayıpta quinn'in **gerisinde**
(0.86–0.97 vs 1.000). Jammer/EW iddiasının kalbi. P0 ARQ-fix (759d4aa8) yapıldı; **quinn'e karşı tam
yeniden-doğrulama YAPILMADI** (P0-CLOSURE §"Kalan" satır 50: n=10, h3a/b/c). V1 bunu kapatır.

---

## 0. Soru (tek cümle)

P0 ARQ-fix'inden SONRA, **sevk edilen Kama (kama-tail-v1)** patlamalı-kayıp (jammer/EW imzası)
rejiminde quinn'i **en az YAKALIYOR mu** (not-worse), yoksa hâlâ geride mi? İkincisi ise ARQ-fix tek
başına yetmiyor → V2 (FEC ileri-kurtarma) gerekli.

**Not (çerçeveleme):** Bu rejimde quinn ZATEN çalışıyor (≈1.000) — yani "quinn çalışamaz" rejimi DEĞİL.
Burada bar "üstünlük" değil, **negatif bulguyu nötrlemek**: sevk Kama'nın jammer-burst'te quinn'e karşı
bir yük/regresyon OLMADIĞINI göstermek. Üstünlük bonus; eşitlik yeterli.

---

## 2. Dondurulmuş girdiler (hash'li)

| girdi | değer |
|---|---|
| commit | `f8b7bca2` (P0 fix `759d4aa8` atada) |
| bench binary sha256 | `fed8a2f8416b8268d6139ac4c279c131cb3518f6dd9386a9d3c80570eb467675` |
| manifest sha256 | `4b0b1a280dc754a745953e20df60589e9750aee49b32a2f13f6c1394f0298bbf` |
| manifest | `reports/burst-reproof-v1/manifest-h3.jsonl` (30 cell-rep) |
| harness | `scripts/netem/kapi_calrun.sh` (gerçek 3-netns, unshare -rnm, netem GE) |

**Profiller** (Gilbert-Elliott, `[p, r, 1-h, 1-k]` = [good→bad%, bad→good%, loss-in-bad%, loss-in-good%],
paced d120 u0.75, 100×256 B @ 250 kbit/s):
- h3a_burst40: `[40, 25, 90, 10]`
- h3b_burst50: `[50, 25, 90, 10]`
- h3c_burst60: `[60, 25, 90, 10]`

**Kollar** (manifest .arms — 4):
- **kama-tail-v1** — SEVK primary (G2 validated_v1, tail-ON).
- **q-dgram-arq** — ön-kayıtlı primary quinn baseline (r5).
- kama-arq — tail-OFF (tail katkısı bağlamı, ikincil).
- q-stream — quinn stream (ikincil bağlam).

**n = 10 / profil.** Seed'ler banked exploratory ile AYNI (frozen) → pre-fix ile elmalar-elmalar
before/after karşılaştırması da mümkün.

---

## 3. Analiz planı (dondurulmuş)

**Metrik:** completion fraction = teslim edilen mesaj / 100 (deadline 120 s içinde), primary A2 düzlemi.

**Birincil karşılaştırma (profil başına):** eşli fark
`Δ_rep = completion(kama-tail-v1) − completion(q-dgram-arq)`, aynı rep+seed. Özet: ortalama Δ, medyan Δ,
**eşli-bootstrap %95 CI** (10k resample, kapi-eval-v1 ile aynı yöntem; RNG tohumu rapor edilir).

**Karar kuralı (profil başına — dondurulmuş):**
| CI | hüküm |
|---|---|
| CI alt-sınır ≥ 0 | **NOT-WORSE** — sevk Kama jammer-burst'te quinn'i yakalıyor; negatif bulgu nötr |
| CI üst-sınır < 0 | **STILL-WORSE** — ARQ-fix yetersiz → V2 (FEC) gerekli |
| 0'ı kapsıyor | **INCONCLUSIVE @ n=10** — Δ küçük; büyüklük rapor edilir |

**İkincil (dondurulmuş, hipotez-üretmez, yalnız bağlam):**
1. Before/after: yeni binary h3 completion vs banked pre-fix exploratory h3 (aynı seed) → fix büyüklüğü.
2. Tail katkısı: kama-tail-v1 vs kama-arq (eşli Δ) — tail'in burst'teki payı.

**Geçerlilik (exogenous-only invalidation, r5 §5 mirası):** her rep-kol realized GE forward-loss'u
harness'ın `realized_*.json`'undan okunur; configured'dan aşırı sapan (altyapı/barrier hatası) satır
INVALID sayılır, ölçüme girmez — ama primary hücre completion'ı outcome-temelli ATILMAZ. INVALID sayısı
raporlanır.

**Kısıtlar:** post-hoc tuning YOK. Tek koşu. Tek analiz. Yeni kampanya/eksen YOK. Bu belge sonuç
görülmeden donduruldu (bu commit).

---

## 4. Beklenen maliyet + makine

~120 rep-kol (4 kol × 30 cell-rep) × ~110–130 s ≈ **~4 saat**, seri (kapi_calrun izolasyonu).
kapi_calrun resumable (out.jsonl SKIP). Makine AC'de, koşu sırasında cargo/ağır-süreç YASAK.
Çıktı: `reports/burst-reproof-v1/out/`. Analiz sonrası: `FINAL-REPORT.md` (aynı commit disiplini).
