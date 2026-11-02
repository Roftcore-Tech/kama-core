**Aynı netem profilinde ürün yolu (Ajan A, f0_flows; FARKLI iş yükü — doğrudan fark alınmaz).**
Kaynak: `reports/ktp2/f0/a/tam/tablolar.md` (A'nın düzeltme turu 1 sonrası son tablo; T.9 ve sınıf tablosu).
Hücreler `a_r12k_d300_l30_{dosyasiz,dosyali}`. Değerler sansürlü koşu p50/p95'lerinin hücre medyanı [Q1–Q3], N = 30.
İş yükü: dosya 524 700 B (t0 + 5 s), TEXT 40 × 75 B (2,5 s aralık), sinyal, PTT 2 × 25 demet; Noise + S1 `Reorder` +
kama-app dahil. Bench karışık hücresinden farkları: dosya ≈ 2,7 kat, TEXT seyrek, ürün kripto ve sıralama katmanları var.

| ürün yolu (A) | N | TEXT p50 s | TEXT p95 s | PTT ≤0,8 s (%, koşu medyanı) |
|---|---|---|---|---|
| taban dosyasız | 30 | 2,5 [1,5–5,8] | 13,5 [10,3–18,9] | 2 |
| aday dosyasız | 30 | 1,3 [1,0–3,1] | 10,7 [7,2–14,5] | 4 |
| taban dosyalı | 30 | 68,6 [33,9–139,3] | 113,6 [78,9–184,3] | 0 |
| aday dosyalı | 30 | 31,2 [23,2–92,8] | 73,9 [67,7–134,7] | 0 |
