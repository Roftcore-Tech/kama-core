# Z3 — P15/F15 KARAR-RAPORU: paired-outcome + fairness-vs-concentration + Pareto + SHORTLIST

> Kanıt: Z2-sweep (paired-trace; blok-düzeyi). Bu-rapor-shortlist-üretir; PRODUCTION-DEFAULT-SEÇMEZ.

## 1. Paired-blok-sınıflandırması (§5; tam-CSV: pos-paired-classes.csv)
rev99 (ayrıştıran-hücre): all_succeeded-418 · all_failed-12 · only_L-11 · only_P15-5 · **only_F15-13** ·
P15∧F15-rescued-35 · F15_harmed-3 / P15_harmed-3 (simetrik) · F15-vs-P15-doğrudan: **16-rescued / 8-ters**.
blackout-30s: F15_harmed-10 (tek-yönlü-maliyet-hücresi; aşağıda). gap80/manyblock/d1000: harmed≈0;
more-parity-no-outcome-change-çoğunluk (deadline-doygun-hücrelerde-beklenen).

## 2. Fairness-vs-concentration HÜKMÜ
**H1 (fairness) KAZANDI:** F15 decode'ları TÜM-deficit-sınıflarında ≥P15 (rev99 d1: 133-71; d2: 75-44;
d3+: 17-12; diğer-hücreler-aynı-yön). **H2 (concentration) desteklenmedi:** P15-yoğunlaşması-hiçbir-
sınıfta-decode-üstünlüğü-üretmedi. "F15-tek-yetersiz-parity-dağıtıyor"-endişesi-ÇÜRÜDÜ (deficit-2/3+
sınıflarında-da-F15-önde — ticket+bulk-birlikte-çalışıyor).

## 3. Utility (parity-başına-fayda; NA-explicit)
retx_efficiency (avoided-retx-bytes/parity-bytes): rev99 F15-0.099 vs P15-0.049 vs L-0.003;
blackout-30s F15-0.090-vs-P15-0.007; decode/MB: F15-tutarlı-2-10× P15. F15-moot-parity-düşük
(cancel-erken — ledger'da). Zero-parity-blok: F15≈0 (ticket-kapsaması) vs P15-yüksek-contended'da.

## 4. Benefit/Cost-Pareto (r4; tek-skor-YOK — objective-sıralaması-kullanıcıda)
| aday | benefit (dl_op@rev99 / decodes / avoided-retx / worst-decile) | cost (parity-B / ndisp+rdisp / harmed) |
|---|---|---|
| L | 0.870 / taban / taban / kötü | 0 -displacement; parity-israfı-yüksek (moot) |
| P15 | 0.922 / orta / orta / orta | ≈F15-displacement; zero-parity-blok-bırakır |
| **F15** | **0.938 / en-yüksek / en-yüksek / en-iyi** | ≈P15-displacement; blackout-30s'te-10-harmed |
Dominasyon: **P15, F15-tarafından-pratikte-domine** (aynı-maliyet-bandında-her-benefit-ekseninde-≤F15;
tek-istisna-blackout-30s-dl_op — aşağıda). L: düşük-maliyet/düşük-fayda-köşesi (dominate-edilmez;
maliyetsiz-taban). F10/F20: monoton-uçlar (F15-orta-nokta; [CAL]-kalibrasyona).

## 5. §19-kriterleri
Minimum-fayda ✓ (rev99: dl_op +0.068 band-üstü + decode + avoided-retx; ≥2-impairment-ailesi: reverse-
starvation + burst-loss + long-RTT-yönü) · cost-bound ✓ (cap/waste/queue-ihlal-0; displacement-envelope-
içi) · robustness ✓ (seed-cluster'a-bağlı-değil; gap80-bimodal-raporlu; BtlBw-artefaktı-değil — bw12-
nötr; Leftover-non-regression-korundu) · **P15/F15-farkı: F15 "daha-fazla-successful-decode + daha-
fazla-deadline-rescued + daha-iyi-worst-decile" ÜÇÜNÜ-de-gösterdi** → ek-karmaşıklığını-hak-ediyor.

## 6. DETERMİNİSTİK-SHORTLIST: **S1 — F15**
Karşı-sinyal-şerhi: blackout-30s-F15_harmed-10 (dl_op-0.96-vs-0.98) — netem-doğrulama-arm'larında-
İZLENECEK-madde; karanlık-pencere-displacement'ı [CAL]-share-düşürmeyle-yumuşatılabilir (F10-yönü).
Netem-arm-seçimi (S1-gereği): **Leftover + F15** (+P15-yalnız-kararsızlık-çıkarsa).
