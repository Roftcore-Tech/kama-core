Gösterim seti: — (S uygulanmadı: şeritli gösterim kökü (--goster on)).

manifest sha256 37bcabe31107a464970a54f5073a36d1d0e65e1cba558c1bf363360c0cb56e1a · analog a_r12k_d150_l20_foto150k · etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta · N < 30: betimsel

## S (bağlayıcı) — uygulanmadı: şeritli gösterim kökü (--goster on)

| kol | dosya bitiş med (s) | PTT p95 med (s) | emisyon/sunulan | (i) | (ii) | (iii) | (iv) R3 | uygun |
|---|---|---|---|---|---|---|---|---|
| K0 | 34.3 | 4.98 | — | — | — | — | — | — |
| K0r | — | — | — | — | — | — | — | — |
| K1 | 30.3 | 3.66 | — | — | — | — | — | — |
| K2 | — | — | — | — | — | — | — | — |
| K2−c | — | — | — | — | — | — | — | — |
| K2−e | — | — | — | — | — | — | — | — |
| K1a | 27.6 | 2.65 | — | — | — | — | — | — |
| K1g | 28.3 | 3.27 | — | — | — | — | — | — |
| K1gr | 22.6 | 2.69 | — | — | — | — | — | — |
| K1grp | 24.4 | 2.91 | — | — | — | — | — | — |
| K1A | 21.2 | 3.01 | — | — | — | — | — | — |

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
| analog | K0 | — | 8374 | 2.34 | 3.51 | 0.324 | 1.00 | 4 |
| analog | K0r | — | — | — | — | — | — | — |
| analog | K1 | — | 14083 | 2.24 | 3.36 | 0.324 | 1.00 | 0 |
| analog | K2 | — | — | — | — | — | — | — |
| analog | K2−c | — | — | — | — | — | — | — |
| analog | K2−e | — | — | — | — | — | — | — |
| analog | K1a | — | 17732 | 2.21 | 3.32 | 0.324 | 1.00 | 0 |
| analog | K1g | — | 15400 | 2.23 | 3.34 | 0.324 | 1.00 | 0 |
| analog | K1gr | — | 17172 | 2.21 | 3.32 | 0.324 | 1.00 | 0 |
| analog | K1grp | — | 14814 | 2.23 | 3.35 | 0.324 | 1.00 | 0 |
| analog | K1A | — | 17192 | 2.21 | 3.32 | 0.324 | 1.00 | 0 |

Startup dönemi kaybı [0, 20) s: ölçülmedi (araç yok; capture eşlemesi gerekir)
diag_probe: ölçülmedi (aynada yok); probe_up_entries raporlanır

## PTT varış / oynatma (raporlanan; eşik yok) — alice gönderiminden; varış = bob Transport → Application (oynatma tamponu öncesi), oynatma = bob PttBundle olayı (= (ii) PTT p95)

| hücre | kol | goster | n | varış p95 med (s) | oynatma p95 med (s) | ilk demet varış (basışların en geci) med (s) | eşlenmemiş birim (toplam) |
|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K0 | True | 10 | 4.57 | 4.98 | 1.44 | 0 |
| a_r12k_d150_l20_foto150k | K1 | True | 10 | 1.94 | 3.66 | 1.49 | 0 |
| a_r12k_d150_l20_foto150k | K1a | True | 10 | 1.28 | 2.65 | 0.63 | 0 |
| a_r12k_d150_l20_foto150k | K1g | True | 10 | 1.51 | 3.27 | 1.06 | 0 |
| a_r12k_d150_l20_foto150k | K1gr | True | 10 | 1.29 | 2.69 | 0.67 | 0 |
| a_r12k_d150_l20_foto150k | K1grp | True | 10 | 1.41 | 2.91 | 0.89 | 0 |
| a_r12k_d150_l20_foto150k | K1A | True | 10 | 1.77 | 3.01 | 0.87 | 0 |

## E2a/A kıyası (raporlanan; eşik yok, betimsel) — kol / taban (oran); tek ikili, fark yalnız kolun bayrakları (K1a, K1g, K1gr, K1grp, K1A ↔ K1; K0a ↔ K0; açık bayraklar koşu başlığında yol3.e1_set / yol3.e2a_set)

| hücre | kol ↔ taban | n | alınmayan dosya | dosya bitiş med (s) | PTT p95 med (s) | PTT varış p95 med (s) | pace med (B/s) | srtt med (s) | RTprop med (s) | probe_up med |
|---|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K1a ↔ K1 | 10 / 10 | 0 / 0 | 27.6 / 30.3 (0.91) | 2.65 / 3.66 (0.72) | 1.28 / 1.94 (0.66) | 17732 / 14083 (1.26) | 0.336 / 0.361 (0.93) | 0.324 / 0.324 (1.00) | 0 / 0 (—) |
| a_r12k_d150_l20_foto150k | K1g ↔ K1 | 10 / 10 | 0 / 0 | 28.3 / 30.3 (0.93) | 3.27 / 3.66 (0.89) | 1.51 / 1.94 (0.78) | 15400 / 14083 (1.09) | 0.336 / 0.361 (0.93) | 0.324 / 0.324 (1.00) | 0 / 0 (—) |
| a_r12k_d150_l20_foto150k | K1gr ↔ K1 | 10 / 10 | 0 / 0 | 22.6 / 30.3 (0.74) | 2.69 / 3.66 (0.73) | 1.29 / 1.94 (0.66) | 17172 / 14083 (1.22) | 0.334 / 0.361 (0.92) | 0.324 / 0.324 (1.00) | 0 / 0 (—) |
| a_r12k_d150_l20_foto150k | K1grp ↔ K1 | 10 / 10 | 0 / 0 | 24.4 / 30.3 (0.81) | 2.91 / 3.66 (0.80) | 1.41 / 1.94 (0.72) | 14814 / 14083 (1.05) | 0.336 / 0.361 (0.93) | 0.324 / 0.324 (1.00) | 0 / 0 (—) |
| a_r12k_d150_l20_foto150k | K1A ↔ K1 | 10 / 10 | 0 / 0 | 21.2 / 30.3 (0.70) | 3.01 / 3.66 (0.82) | 1.77 / 1.94 (0.91) | 17192 / 14083 (1.22) | 0.330 / 0.361 (0.91) | 0.324 / 0.324 (1.00) | 0 / 0 (—) |

## Q1 alanları (raporlanan; ts satırlarından, koşu sonu değerin medyanı)

- gap_ack_emits: aynada yok
- regime_timer_emits: aynada yok
- measured_return_bw_bps: aynada yok
- ilan_retx_ms: aynada yok
