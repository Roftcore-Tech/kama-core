Gösterim seti: — (S uygulanmadı: analog hücrede K0 verisi yok; E2a kıyas kökü).

manifest sha256 37bcabe31107a464970a54f5073a36d1d0e65e1cba558c1bf363360c0cb56e1a · analog a_r12k_d150_l20_foto150k · etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta · N < 30: betimsel

## S (bağlayıcı) — uygulanmadı: analog hücrede K0 verisi yok

| kol | dosya bitiş med (s) | PTT p95 med (s) | emisyon/sunulan | (i) | (ii) | (iii) | (iv) R3 | uygun |
|---|---|---|---|---|---|---|---|---|
| K0 | — | — | — | — | — | — | — | — |
| K0r | — | — | — | — | — | — | — | — |
| K1 | 20.4 | 9.82 | — | — | — | — | — | — |
| K2 | — | — | — | — | — | — | — | — |
| K2−c | — | — | — | — | — | — | — | — |
| K2−e | — | — | — | — | — | — | — | — |
| K1a | 21.3 | 10.69 | — | — | — | — | — | — |
| K0a | 23.3 | 12.73 | — | — | — | — | — | — |

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
| analog | K0 | — | — | — | — | — | — | — |
| analog | K0r | — | — | — | — | — | — | — |
| analog | K1 | — | 12644 | 2.26 | 3.39 | 0.324 | — | 1 |
| analog | K2 | — | — | — | — | — | — | — |
| analog | K2−c | — | — | — | — | — | — | — |
| analog | K2−e | — | — | — | — | — | — | — |
| analog | K1a | — | 11826 | 2.27 | 3.40 | 0.324 | — | 1 |
| analog | K0a | — | 10556 | 2.29 | 3.43 | 0.324 | — | 3 |

Startup dönemi kaybı [0, 20) s: ölçülmedi (araç yok; capture eşlemesi gerekir)
diag_probe: ölçülmedi (aynada yok); probe_up_entries raporlanır

## E2a kıyası (raporlanan; eşik yok, betimsel) — kol / taban (oran); tek değişken: E1 seti aynı, fark yalnız E2a (M1 E2aFlags: tt_rfc g=1ms, rtt_rule_v11, iw_idle_hold, pto_probes=2)

| hücre | kol ↔ taban | n | alınmayan dosya | dosya bitiş med (s) | PTT p95 med (s) | pace med (B/s) | srtt med (s) | RTprop med (s) | probe_up med |
|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K1a ↔ K1 | 10 / 10 | 0 / 0 | 21.3 / 20.4 (1.04) | 10.69 / 9.82 (1.09) | 11826 / 12644 (0.94) | 0.331 / 0.339 (0.98) | 0.324 / 0.324 (1.00) | 1 / 1 (1.00) |
| a_r12k_d150_l20_foto150k | K0a ↔ K0 | 10 / 0 | 0 / 0 | 23.3 / — (—) | 12.73 / — (—) | 10556 / — (—) | 0.332 / — (—) | 0.324 / — (—) | 3 / — (—) |
