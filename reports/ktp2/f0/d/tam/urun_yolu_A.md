**Aynı netem profilinde ürün yolu (Ajan A, f0_flows; FARKLI iş yükü — doğrudan fark alınmaz).**
Kaynak: `runs/tam/a/out/a_r12k_d300_l30_{dosyasiz,dosyali}`, `scripts/ktp2/f0/a/f0a_analyze.py` (D şeridinde
çalıştırıldı; değerler A'nın raporundaki tabloyla aynı olmalı). İş yükü: dosya 524 700 B (t0+5 s), TEXT 40 × 75 B
(2,5 s aralık), sinyal, PTT 2 × 25 demet; Noise + S1 `Reorder` + kama-app dahil. Bench karışık hücresinden
farkları: dosya ≈ 2,7 kat, TEXT seyrek, ürün kripto/sıralama katmanları var.

| ürün yolu (A) | N | TEXT p50 s | TEXT p95 s | PTT ≤0,8 s (havuz %) |
|---|---|---|---|---|
| taban dosyasız | 30 | 2,53 [1,52–5,77] | 13,49 [10,27–18,88] | 2 |
| aday dosyasız | 30 | 1,31 [0,96–3,09] | 10,74 [7,19–14,49] | 4 |
| taban dosyalı | 30 (p95 n=28) | 62,09 [33,53–95,45] | 107,09 [78,53–140,45] | 0 |
| aday dosyalı | 30 (p95 n=26) | 29,57 [22,70–39,87] | 73,05 [67,26–83,94] | 0 |
