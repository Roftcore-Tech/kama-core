Gösterim seti: K0. E1 (manşet, tuzaksız): K2 —, K2−c —.

manifest sha256 37bcabe31107a464970a54f5073a36d1d0e65e1cba558c1bf363360c0cb56e1a · analog a_r12k_d150_l20_foto150k · etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta · N < 30: betimsel

## S (bağlayıcı) — hiçbir E1 kolu uygun değil; K0r şartı sağlanmadı → K0

| kol | dosya bitiş med (s) | PTT p95 med (s) | emisyon/sunulan | (i) | (ii) | (iii) | (iv) R3 | uygun |
|---|---|---|---|---|---|---|---|---|
| K0 | 23.9 | 15.29 | — | — | — | — | — | — |
| K0r | 26.7 | 16.37 | — | False | False | — | — | — |
| K1 | 19.2 | 8.63 | — | True | True | None | False | False |
| K2 | 17.5 | 9.24 | — | True | True | None | False | False |
| K2−c | 20.8 | 10.22 | — | True | True | None | False | False |
| K2−e | 19.7 | 9.21 | — | True | True | None | False | False |

## R3 (bağlayıcı): geçer = False · d1000 l0 p95 med K0 — / K2 — → None · r25 teslim@60, gönderim B: K0 (None, None) / K2 (None, None) → None

## R1 (raporlanan): manşet gönderim/gereken

| kol | kullanılan | tuzaksız n / n | medyan | bütün medyan | ≥ 0,90 |
|---|---|---|---|---|---|
| K0 | bütün (tuzaksız < 7) | 0 / 0 | — | — | None |
| K2 | bütün (tuzaksız < 7) | 0 / 0 | — | — | None |
| K2−c | bütün (tuzaksız < 7) | 0 / 0 | — | — | None |

## R4 (raporlanan)

| hücre | kol | sw95 med / 9. sıra (s) | pace med (B/s) | EAI (s) | kayıp eşiği (s) | RTprop med (s) | RTprop/K0 | probe_up med |
|---|---|---|---|---|---|---|---|---|
| manset | K0 | — / — | — | — | — | — | — | — |
| manset | K2 | — / — | — | — | — | — | — | — |
| manset | K2−c | — / — | — | — | — | — | — | — |
| analog | K0 | — | 6264 | 2.42 | 3.63 | 0.324 | 1.00 | 4 |
| analog | K0r | — | 4030 | 2.60 | 3.90 | 0.326 | 1.01 | 4 |
| analog | K1 | — | 12262 | 2.26 | 3.39 | 0.324 | 1.00 | 1 |
| analog | K2 | — | 8055 | 2.35 | 3.52 | 0.324 | 1.00 | 2 |
| analog | K2−c | — | 10288 | 2.29 | 3.44 | 0.324 | 1.00 | 2 |
| analog | K2−e | — | 8423 | 2.34 | 3.51 | 0.324 | 1.00 | 3 |

Startup dönemi kaybı [0, 20) s: ölçülmedi (araç yok; capture eşlemesi gerekir)
diag_probe: ölçülmedi (aynada yok); probe_up_entries raporlanır

## Gösterim sahnesi — Kama ve quinn kolları aynı ölçülerle (raporlanan; S'ye girmez)

dosya bitiş = (i), PTT p95 = (ii) tanımları aynen; teslim = koşu başına teslim / gönderilen (PTT: P; öğe: W, T, S, P, KP, K, O, C), koşu medyanı; kurulum = alice t0 − connect_requested (Kama zarfları Unix soketinden, netem dışı; quinn el sıkışması netem içinde; ikisi de pencere dışında). N < 30: betimsel.

| kol | N | geçerli | tamam | dosya alındı (doğru) | dosya bitiş med (s) | PTT p95 med (s) | PTT teslim med % | öğe teslim med % | kurulum med (s) |
|---|---|---|---|---|---|---|---|---|---|
| K0 | 10 | 10 | 9 | 9 | 23.9 | 15.29 | 100.0 | 100.0 | 0.021 |
| K0r | 10 | 10 | 7 | 7 | 26.7 | 16.37 | 100.0 | 100.0 | 0.021 |
| K1 | 10 | 10 | 10 | 10 | 19.2 | 8.63 | 100.0 | 100.0 | 0.021 |
| K2 | 10 | 10 | 10 | 10 | 17.5 | 9.24 | 100.0 | 100.0 | 0.021 |
| K2−c | 10 | 10 | 10 | 10 | 20.8 | 10.22 | 100.0 | 100.0 | 0.021 |
| K2−e | 10 | 10 | 10 | 10 | 19.7 | 9.21 | 100.0 | 100.0 | 0.021 |
| Q-bbr | 10 | 10 | 10 | 10 | 13.6 | 2.16 | 100.0 | 100.0 | 0.502 |
| Q-bbr-rfc | 10 | 10 | 10 | 10 | 14.8 | 1.51 | 100.0 | 100.0 | 0.502 |
| Q-cubic | 10 | 10 | 10 | 10 | 26.8 | 1.79 | 100.0 | 100.0 | 0.502 |
