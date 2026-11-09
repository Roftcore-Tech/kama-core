Gösterim seti: K1. E1 (manşet, tuzaksız): K2 1.02, K2−c 0.56.

manifest sha256 37bcabe31107a464970a54f5073a36d1d0e65e1cba558c1bf363360c0cb56e1a · analog a_r12k_d150_l20_foto150k · etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta · N < 30: betimsel

## S (bağlayıcı) — uygun ['K1', 'K2', 'K2mc', 'K2me']; en düşük PTT %10 bandı ['K1', 'K2me', 'K2']

| kol | dosya bitiş med (s) | PTT p95 med (s) | emisyon/sunulan | (i) | (ii) | (iii) | (iv) R3 | uygun |
|---|---|---|---|---|---|---|---|---|
| K0 | 23.9 | 15.29 | 1.09 | — | — | — | — | — |
| K0r | 26.7 | 16.37 | — | — | — | — | — | — |
| K1 | 19.2 | 8.63 | 4.90 | True | True | True | True | True |
| K2 | 17.5 | 9.24 | 4.90 | True | True | True | True | True |
| K2−c | 20.8 | 10.22 | 3.69 | True | True | True | True | True |
| K2−e | 19.7 | 9.21 | 4.90 | True | True | True | True | True |

(iii) payda (KK-ORK-03): a_r12k_d300_l30_dosyasiz sunulan = 430.0 B/s [türetilmiş; mühürlü UDP yükü, pr ile aynı katman]; pay = alice pr koşu medyanı (Idle hariç)

## R3 (bağlayıcı): geçer = True · d1000 l0 p95 med K0 5.09 / K2 4.48 → True · r25 teslim@60, gönderim B: K0 (6.0, 2075.0) / K2 (6.0, 2123.5) → True

## R1 (raporlanan): manşet gönderim/gereken

| kol | kullanılan | tuzaksız n / n | medyan | bütün medyan | ≥ 0,90 |
|---|---|---|---|---|---|
| K0 | tuzaksız | 10 / 10 | 0.55 | 0.55 | False |
| K2 | tuzaksız | 7 / 10 | 1.02 | 1.00 | True |
| K2−c | tuzaksız | 8 / 10 | 0.56 | 0.53 | False |

## R4 (raporlanan)

| hücre | kol | sw95 med / 9. sıra (s) | pace med (B/s) | EAI (s) | kayıp eşiği (s) | RTprop med (s) | RTprop/K0 | probe_up med |
|---|---|---|---|---|---|---|---|---|
| manset | K0 | 12.71 / 34.89 | 4394 | 2.56 | 3.84 | 1.792 | 1.00 | 0 |
| manset | K2 | 6.28 / 63.75 | 7491 | 2.37 | 3.55 | 1.802 | 1.01 | 0 |
| manset | K2−c | 25.39 / 66.56 | 4377 | 2.56 | 3.84 | 1.755 | 0.98 | 1 |
| analog | K0 | — | 6264 | 2.42 | 3.63 | 0.324 | 1.00 | 4 |
| analog | K0r | — | 4030 | 2.60 | 3.90 | 0.326 | 1.01 | 4 |
| analog | K1 | — | 12262 | 2.26 | 3.39 | 0.324 | 1.00 | 1 |
| analog | K2 | — | 8055 | 2.35 | 3.52 | 0.324 | 1.00 | 2 |
| analog | K2−c | — | 10288 | 2.29 | 3.44 | 0.324 | 1.00 | 2 |
| analog | K2−e | — | 8423 | 2.34 | 3.51 | 0.324 | 1.00 | 3 |

Startup dönemi kaybı [0, 20) s: ölçülmedi (araç yok; capture eşlemesi gerekir)
diag_probe: ölçülmedi (aynada yok); probe_up_entries raporlanır
