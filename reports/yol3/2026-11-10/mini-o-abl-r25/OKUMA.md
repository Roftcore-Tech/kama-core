# mini-Ö v2 ablasyonu — b_r25_d300_l20_p65

Ön kayıt: ONKAYIT-V2.md sha256 136f4bc457a04d5708a59cd0fdd971c9b23a26c313f9f1fd924f7b480ba3ca53 (denetlendi) (§6). Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel; sığ tamponlu ağ gösterim kapsamı dışı. Değerler [ölçüldü], oran ve karar [türetilmiş].

| kol | çıkan bayrak | n | rc=0 | teslim@60 med | gönderim med B | teyit p95 med s | marj | R3 (taban K0) | manifest doğru |
|---|---|---|---|---|---|---|---|---|---|
| K0 | — | 20 | 20 | 6,00 | 2835 | 46,66 | — | taban | 20/20 |
| K1A6 | — | 20 | 20 | 6,00 | 2662 | 55,68 | — | evet | 20/20 |
| K1A6mA1 | A-1 gap_ack | 20 | 20 | 6,00 | 2516 | 54,53 | (0.887, -0.0) | evet | 20/20 |
| K1A6mA2 | A-2 return_prior_gated | 20 | 20 | 6,00 | 2538 | 51,48 | (0.895, -0.0) | evet | 20/20 |
| K1A6mA3 | A-3 a3_burst_floor | 20 | 20 | 6,00 | 2584 | 53,47 | (0.911, -0.0) | evet | 20/20 |
| K1A6mA4 | A-4 a4_probe_srtt | 20 | 20 | 6,00 | 2564 | 52,50 | (0.905, -0.0) | evet | 20/20 |
| K1A6mA5 | A-5 pto_rfc | 20 | 20 | 6,00 | 2262 | 49,21 | (0.798, -0.0) | evet | 20/20 |
| K1A6mA6 | A-6 a6_iw_floor | 20 | 20 | 6,00 | 2500 | 48,15 | (0.882, -0.0) | evet | 20/20 |

- Durum: **ihlal bu kökte yeniden üretilmedi → bayrak düşmez**
- Aday (çıkınca R3 geçen): A-1, A-2, A-3, A-4, A-5, A-6
- Düşen bayrak: **yok**
