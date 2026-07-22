# FU — REGIME OBSERVABILITY (Phase-G)

> **Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine eklenen
> proaktif FEC'in marjinal faydasıdır.** (Bu faz karar-öncesi ayrışabilirlik ölçer.)

## HÜKÜM

> **FEC WALL-CLOCK UTILITY IDENTIFIED — CONTROLLER OBSERVABILITY BLOCKED (mevcut fusion-girdileri);
> OBSERVABLE WITH CANDIDATE MEASUREMENT → SENSOR/MEASUREMENT DR REQUIRED.**
> Utility-rejimi (RTT-düşük + loss/deficit) karar-öncesi AYRIŞTIRILABİLİR — ama yalnız MEVCUT-fusion-
> vektörünün-DIŞINDA-kalan ama HALİHAZIRDA-HESAPLANAN bir ölçümle (srtt / deadline-slack). Loss-boyutu
> zaten-gözlemlenebilir (loss_ppm sağlam; FT). Eksik-boyut = RTT/deadline (fusion-girdisi-değil).

## 1. UTILITY-REJİMİ (FS'ten) — ne ayrıştırılmalı?

Proaktif-FEC marjinal-fayda-rejimi (MarginalFecBeneficial/Essential): **RTT ≤ 250ms** (FS). Nötr-rejim
(NoMarginalFecUtility): **RTT ≥ 1000ms**. Ayrıştırıcı-değişken = **RTT** (+ loss/deficit-varlığı, +deadline-
sıkılığı). Controller "proaktif-FEC-şimdi-faydalı-mı" için bu-rejimi karar-ÖNCESİ görmeli.

## 2. MEVCUT FUSION-GİRDİLERİ (karar-öncesi vektör)

`FusionInputs` (fusion.rs): `loss_ppm`, `bandwidth_bps`, `jitter_ppm`, `qd_ratio_permille`,
`ack_confidence_permille`. **RTT/srtt/deadline/time-to-deadline YOK** (grep-kanıtlı).

| Rejim-bileşeni | Mevcut-girdi | Gözlemlenebilir? |
|---|---|---|
| Loss/deficit-varlığı | `loss_ppm` (pre-recovery erasure; FT-sağlam) | **EVET** (0%→0, 30%→272k ppm) |
| RTT (ayrıştırıcı) | — (fusion-girdisi-DEĞİL) | **HAYIR** (mevcut-vektörde) |
| Deadline / time-to-deadline | — | **HAYIR** (app-katmanı; transport'a-threading-yok) |
| Headroom | `bandwidth_bps` + pacing (kısmi) | kısmi |

## 3. CANDIDATE MEASUREMENT — srtt HALİHAZIRDA-HESAPLANIR, RTT'yi-FAİTHFUL-İZLER

`srtt_us` `diag_rate()`-ile-expose (sender.rs:2851) ama fusion-girdisi-değil. Karar-ÖNCESİ (geçmiş-ACK'lerden;
no-leakage). ÖLÇÜLDÜ (faithful-harness --emit-fuzzy, warm-steady-p50):

| configured-RTT | srtt_us (warm-p50) | ayrışma |
|---|---|---|
| 100ms | **112 769** | — |
| 2500ms | **2 516 564** | temiz (22×; örtüşme-yok) |

⇒ srtt utility-rejimini (RTT-düşük-vs-yüksek) **TEMİZ-AYRIŞTIRIR** (113k-us ≪ 2517k-us). Candidate-türev:
`deadline_slack_ratio = time_to_deadline / est_arq_repair`, `est_arq_repair ≈ 1.5×srtt` (hepsi-pre-decision).

## 4. NO-LEAKAGE DOĞRULAMASI (§10.4)

Kullanılan-adaylar YALNIZ karar-öncesi: srtt (geçmiş-ACK-türevi), loss_ppm (windowed-geçmiş), deadline (app-
supplied, sabit), est_arq_repair (srtt-türevi-önsel). KULLANILMAYAN (label, input-değil): current-block-
gelecek-deficit'i, sonradan-oluşan-FEC-decode, retransmission, completion, deadline-sonucu. → sızıntı-YOK.

## 5. COLD-START vs STEADY-STATE (FM-ayrımı)

- **Warm-steady:** loss_ppm + srtt sağlam (yukarıda). Rejim-gözlemlenebilir (candidate-ile).
- **Cold-start:** srtt=None (PreConverge); loss time-threshold-DONUK, packet-threshold örneksiz (FT/FP).
  Utility-rejimi cold-start'ta zaten-yok (ilk-flight-loss FEC'ten-bağımsız-deadlock; FP). ⇒ ayrı-controller-
  rejimi-gerekir (cold-start'ta proaktif-FEC-kararı verilemez/anlamsız; steady-state'te candidate-ile-verilir).

## 6. OBSERVABILITY-SINIFLARI (ölçülen)

| Boyut | Sınıf |
|---|---|
| Loss/deficit | `ObservablePreDecision` (loss_ppm sağlam; FT) |
| RTT-rejimi | `UnobservableWithCurrentFusionInputs` → `ObservableWithCandidateMeasurement` (srtt mevcut, temiz-ayrıştırır) |
| Deadline | `ObservableWithCandidateMeasurement` (app-supplied deadline transport'a-threading-gerekir) |
| Cold-start-rejimi | `Confounded` (srtt=None; ayrı-rejim-şart) |

## 7. VERDICT + DR-KAPSAMI

Utility-VAR (FS: RTT≤250ms MarginalFecBeneficial/Essential) ama mevcut-fusion-vektörü RTT/deadline-boyutunu
GÖRMEZ → **OBSERVABILITY BLOCKED (mevcut-girdilerle)**. Ancak ayrıştırıcı-ölçüm (srtt) HALİHAZIRDA-hesaplanır
ve rejimi-temiz-ayrıştırır → **SENSOR/MEASUREMENT DR** (fuzzy-eşik-tuning DEĞİL):
- **DR-içeriği:** karar-vektörüne srtt (RTT) + deadline-slack-ratio (app-deadline-threading) ekle; cold-start
  için ayrı-rejim (srtt=None-iken proaktif-FEC-kararı-verme). Loss-sensörü ZATEN-sağlam (FT — ekleme-yok).
- **Loss-eşik-membership** (fuzzy_ratio=0@%30-loss; HIGH-eşik~%50) AYRI bir kalibrasyon-konusu, bu-DR-değil
  ve bu-turda-KAPSAM-DIŞI.

## 8. ÖNCEKİ-TUR DÜZELTMESİ

Son-tur: "UnobservableWithCurrentFusionInputs + Confounded; loss_windowed_ppm=0". Bu-tur: loss-sensörü-SAĞLAM
(FT), ayrıştırıcı=RTT (loss-değil), srtt-candidate-ile-gözlemlenebilir. ⇒ observability-DR loss-fidelity'den
RTT/deadline-threading'e DARALIR (loss-yarısı KAPANDI).

## 9. KAPSAM
Üretim FusionInputs DEĞİŞMEDİ (candidate-ölçüm eklenmedi — yalnız offline-separability). Fuzzy-eşik-tuning
YAPILMADI. Veri: `data/loss-fidelity/` (srtt+loss), FS wall-clock (utility-rejimi).
