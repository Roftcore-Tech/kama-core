# FT — LOSS-SIGNAL FIDELITY (Phase-F)

> **Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine eklenen
> proaktif FEC'in marjinal faydasıdır.** (Bu faz SENSÖR-fidelity ölçer; utility değil.)

## HÜKÜM

> **LOSS-SIGNAL FIDELITY: SENSÖR SAĞLAM — PRE-RECOVERY ERASURE'I FAİTHFUL GÖRÜR.** Önceki-turun
> "loss_windowed_ppm=0 heryerde → sensör-kör → observability-blocked" bulgusu **GERİ ÇEKİLİR**: o ölçüm
> rootless-TEK-namespace netem'de-yapıldı ve **netem-loss-UYGULANMAMIŞTI** (gecikme-gibi; faithful-harness-
> kanıtı). Gerçek-forward-loss altında sensör true-loss-fraction'ı ~%82-91-doğrulukla NONZERO-okur.

## 1. KÖK-DÜZELTME: önceki loss=0 bir HARNESS-ARTEFAKTIYDI

Son-tur FE/FF: 231k-fuzzy-event, loss_windowed_ppm=0 (blackout30 dahil) → "sensör kaybı-görmüyor" hükmü.
**Bu ölçüm rootless-tek-ns veth+netem'de yapıldı.** FQ-fazında KANITLANDI: tek-ns-veth netem GECİKMEYİ
uygulamaz (ping RTT≈0.02ms). Aynı-mekanizmayla (eşleşen-uçlar-hızlı-yol egress-qdisc'i-atlar) **netem-loss
da uygulanmaz.** ⇒ önceki loss=0 DOĞRUYDU (gerçek-loss-yoktu), ama SENSÖR-KÖRLÜĞÜ-DEĞİL; harness-loss-
uygulamamasıydı.

## 2. KOD-ANALİZİ (loss.rs — pre-recovery erasure-metre)

`loss_windowed_ppm` (loss.rs:243) = Σ(Confirmed-event.units) / sent_window_units (ppm). Yalnız orijinal-DATA
(retransmit/dup units=0; sender.rs:2778 → ne-pay-ne-payda). Loss iki-eşikten-declare (packet-threshold-SACK-
reorder loss.rs:316-327 + time-threshold-PTO 351-365) → Suspected → confirm_age(1.5×EAI)-sonra Confirmed.
**Recovery SUBTRACT-ETMEZ:** retransmit fresh-seq-alır (2580-2588) → orijinal-seq asla-re-ACK'lenmez;
RecoveredControl self.loss'a-DOKUNMAZ (2034-2079). Un-loss YALNIZ orijinal-seq-ACK'lenirse (gerçek-arrival =
spurious/reorder) veya window-expiry. **⇒ PRE-RECOVERY erasure ölçer** (FEC-controller'ın-tam-istediği-şey).
Pencere = 8×max(srtt,EAI) sliding (loss.rs:222-238); fuzzy-oku öncesi güncellenir (ordering ok).

## 3. FAİTHFUL-LOSS DENEYİ (bu tur; two-netns harness; GERÇEK forward-loss)

`run_loss_fidelity.sh` (faithful-harness; forward=v0-egress GERÇEK-loss + delay-50ms; reverse-temiz;
tx --emit-fuzzy; w_units=400 warm-steady; 2-rep). warm-steady (son %50 event) loss_windowed_ppm:

| profil | configured-loss | warm loss_windowed_ppm (min/p50/max/mean) | true-fraction-oranı | ack_conf | fuzzy_ratio |
|---|---|---|---|---|---|
| loss0 | 0% (0 ppm) | 0 / 0 / 0 / **0** | baseline | 1000 | 0 |
| loss10 | 10% (100000 ppm) | 59440 / 77540 / 112676 / **81780** | **~%82** | 1000 | 0 |
| loss20 | 20% (200000 ppm) | 125475 / 169712 / 226890 / **167646** | **~%84** | 1000 | 0 |
| loss30 | 30% (300000 ppm) | 217228 / 260273 / 344947 / **271945** | **~%91** | 1000 | 0 |

**Sensör gerçek-loss'u GÖRÜR** (0%→0, 10%→82k, 20%→168k, 30%→272k ppm; monoton, true-fraction'a-yakın).
Hafif-under-read (%82-91): confirm-age(1.5×EAI)-lag + window-expiry + bir-miktar-reorder-un-loss.

## 4. FIDELITY-CAVEAT'LER (körlük-DEĞİL; ölçüm-nüansları)

1. **Lag:** confirm_age(1.5×EAI) + window(8×ack_epoch) → taze-drop bir-ufuk-görünmez; warm-steady'de-doğru.
   Cold-start-vs-warm: PreConverge'de PTO/time-threshold-loss DONUK (packet-threshold-SACK çalışır ama
   cold-start'ta-örneklem-yok) → cold-start-loss-sinyali-lag'li/eksik (FM-girdisi).
2. **Forward↔reverse ayrımı-YOK (loss.rs Q8):** ACK-delta-tabanlı; reverse-ACK-kaybı da loss-declare-eder.
   Bu-deneyde reverse-temiz-tutuldu → ölçülen-loss saf-forward. Karışık-koşulda over-read-riski.
3. **ack_confidence forward-loss'a-DUYARSIZ:** %30-forward-loss'ta bile ack_conf=1000 (FULL). ack_conf =
   reverse-silence-proxy, forward-erasure-sinyali-DEĞİL. Forward-loss'u YALNIZ loss_ppm taşır.
4. **fuzzy_ratio=0 @ %30-loss:** controller loss'u-GÖRÜR (loss_ppm=272k) ama proaktif-FEC=0-üretir çünkü
   loss-membership-HIGH-eşiği ~%50-60 (500k-600k ppm). Bu bir EŞİK-KALİBRASYON-konusu (bu-turda KAPSAM-DIŞI),
   sensör-körlüğü-DEĞİL. Ayrıca deficit=orta-loss'ta FEC-fayda-rejimi RTT-kapılı (FS) → ratio-tuning tek-
   başına-yetmez (RTT/deadline-girdisi-de-lazım; FU).

## 5. FIDELITY-VERDICT

**LOSS SIGNAL FIDELITY DEFECT — RED (yok).** loss_windowed_ppm gerçek-forward-erasure'ı faithful-görür
(pre-recovery; recovery-maskelemez). Önceki "sensör-kör" hükmü harness-artefaktıydı (**RESOLVED/CLOSED**).
Kalan-caveat'ler (lag/cold-start/forward-reverse-ayrım/ack_conf-forward-duyarsızlık) körlük-değil, ölçüm-
nüansları; observability-analizinde (FU) dikkate-alınır. Bu, önceki-turun "loss-signal-fidelity-DR"sinin
loss-yarısını KAPATIR.

## 6. KAPSAM
Üretim + fuzzy-config + LeftoverOnly DEĞİŞMEDİ; --emit-fuzzy salt-gözlem (decision-değişmez). Eşik-tuning
YAPILMADI. Veri: `data/loss-fidelity/` (8-koşu; `loss_fidelity_analyze.py`).
