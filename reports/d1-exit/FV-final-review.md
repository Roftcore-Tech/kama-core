# FV — FINAL RESEARCH REVIEW: cold-start liveness + faithful wall-clock FEC utility + sensor observability

> **Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine eklenen
> proaktif FEC'in marjinal faydasıdır.** ARQ/substrat (§9-retransmit + §11-forward-recovery) hiçbir kolda
> kapatılmadı. r0=ARQ+0 … r4=ARQ+4. Residual daima ARQ ile tamamlanır (hibrit).

## İKİ AYRI HÜKÜM

> **(LIVENESS) COLD-START LIVENESS DEFECT CONFIRMED — PROGRESS DR REQUIRED.**
>
> **(UTILITY/OBSERVABILITY) FEC WALL-CLOCK UTILITY IDENTIFIED — CONTROLLER OBSERVABILITY BLOCKED;
> SENSOR/MEASUREMENT DR REQUIRED.**

---

## SON-SUNUM (42 çıktı)

**1. Commit hash'leri:** FO=42d4ef0 · FP=9e0a4b5 · FQ=28f0f6a · FR=782fddd · FS=d57e8ae · FT=33803fa ·
FU=86a392c · FV=(bu). (Önceki-tur FJ+FK=501665e referans.)

**2. Commit-bazında dosyalar:**
- FO: `tests/cold_start_liveness.rs` + `data/cold-start/`
- FP: `FP-cold-start-liveness.md`
- FQ: `scripts/netem/faithful_harness.sh` + `faithful_acceptance.sh` + `data/faithful-harness/`
- FR: `src/udp_runner.rs` + `examples/udp_budget_runner.rs` + `tests/udp_runner_smoke.rs` +
  `run_wallclock_pins.sh` + `fec_wallclock_analyze.py`
- FS: `FS-wallclock-fec-utility.md` + `data/wallclock/` (48-koşu)
- FT: `run_loss_fidelity.sh` + `loss_fidelity_analyze.py` + `FT-loss-signal-fidelity.md` + `data/loss-fidelity/`
- FU: `FU-regime-observability.md` + `data/observability/`

**3. Production diff:** SIFIR. `src/udp_runner.rs` (feature-gated harness; üretim-yoluna-bağlı-değil) +
`examples/` + `tests/` + `scripts/` + `reports/` — üretim-sender/receiver/fusion/pacer/budget-kernel
byte-değişmedi. `state()` mevcut-accessor (yeni-değil).

**4. Feature/default:** `experimental-fec-budget` DEFAULT-OFF. Budget-policy=LeftoverOnly (değişmedi).
Üretim fuzzy-config değişmedi. Deficit-enjektör + drop_repair + emit-fuzzy test-only.

**5. PR:** #44 DRAFT (merge YOK).

### LIVENESS (Phase-A)
**6. Progress invariant (PRE-DECLARED):** ilk-eligible-send-sonrası ACK-bağımsız-Duplicate ≤2×first_delay
(`first_delay=(3/2)(size+65)/31 s`); state-ilerleme YAPISAL-ACK-kapılı. (FP §1; sender.rs sitasyonlu.)

**7. Cold-start senaryo matrisi:** 12-senaryo-sınıfı tasarlandı; direct-integration'da first-loss/first-ACK/
surviving-ACK/timer-only + wire-arm first-block-loss KOŞULDU (FP §2). [Kalan-senaryolar (çift-source,
reverse-blackout-varyantları) invariant'ı-değiştirmez — aynı-ACK-kapılı-mekanizma; direct-integration-kapsar.]

**8. First-source-loss:** ACK-yok → state PreConverge-PİNLİ; ilk-Duplicate first_delay'de (65B→6.3s, 300B→
17.7s, 1200B→**61.2s**, birebir; ×2-backoff). ProgressViaProactiveRepair (caller-clocked).

**9. First-ACK-loss:** Sender-perspektifi = ACK-yok → (8) ile ÖZDEŞ.

**10. PTO arm/fire:** PreConverge'de **DONUK** (sender.rs 1177) — arm/fire-YOK. Hayatta-kalan-ACK-sonrası
Startup'ta-açılır. → DEFECT-göstergesi.

**11. Pacing/dispatch:** PreConverge 1-birim-floor + 31 B/s-refill (~38.7s/1200B-birim). Glacial-ama-kalıcı-
değil.

**12. LIVENESS VERDICT:** **COLD-START LIVENESS DEFECT CONFIRMED — PROGRESS DR REQUIRED** (silent-deadlock-
değil; ama PTO-donuk + ACK-kapılı-state + 31 B/s-floor + ~61s@1200B-recovery = hostile-ağ-zaafı; iki-DEFECT-
göstergesi-literal-karşılandı). Direct-integration + wire-arm-uyumlu → GERÇEK-transport-özelliği. Fix
UYGULANMADI (4-seçenek + slow-channel-gerilimi; FP §5).

### FAITHFUL HARNESS (Phase-B)
**13. Topoloji:** two-netns rootless (unshare -rnm parent + child-netns PID-referanslı veth-move; cross-ns
netem SADIK). docker-daemon/root-YOK.

**14. Env manifest:** Linux 7.0.12-arch1; iproute2-7.0.0; git-501665e; ayrı-namespace-kimlikleri (FQ ENV.txt).

**15. RTT acceptance:** 100ms→100 / 1000ms→1000 / 2500ms→2500 (min-RTT exact). Sweep-içi: 250.09/1000.18/
2500.33ms (configured-exact).

**16. Yön-izolasyonu:** forward-only-500ms→RTT 501ms; reverse-only-500ms→501ms (temiz-ayrım).

**17. Cleanup:** host-veth-sızıntı-YOK (FQ doğrulandı).

### WALL-CLOCK PINS (Phase-C/D)
**18. Frozen-deadline yöntemi:** Phase-C timing-only-probe (winner-görülmeden) → p99(T_fec)<p01(T_arq) ⟺
interval-var. rtt250: (296,315) → deadline=**305ms** (pre-declared).

**19. Pin-1 (d1, ARQ+0 vs ARQ+1) wall-clock:** rtt250 comp 333→**287ms** (FEC-path 30/30); rtt1000
1074→1042 (tie, path 15/30); rtt2500 2635→2633 (tie, 14/30).

**20. Pin-2 (d2, ARQ+1 vs ARQ+2):** rtt250 352→**300ms** (30/30); rtt1000 tie (6/30); rtt2500 tie (6/30).

**21. Pin-3 (d4, ARQ+2 vs ARQ+4):** rtt250 392→**315ms** (30/30); rtt1000 tie (0/30); rtt2500 tie (0/30).

**22. Pin-4 (d5, ARQ+4 hibrit):** kombinatoryal FK'da (501665e; iso-pending residual-1 + hyb 31/29 FEC/ARQ-
split). **Wall-clock-faithful-sweep'te Pin-4-ayrıca-koşulmadı** (izleme-maddesi; d1/d2/d4 wall-clock-kapsandı).

**23. requested/planned/enqueued/emitted/received ledger:** rx-ledger (per-blok: first/complete/rel/dl_op/
deficit_applied/decode_via/first_repair/repair_kind/arq_repair) + tx-summary (enq/emit-r). `data/wallclock/`.

**24. FEC-decode timestamps:** T_fec_avail (parity-arrival-rel) p50: rtt250 ~287ms; rtt1000 1062-2480ms;
rtt2500 2671-6401ms (r↑ ile büyür).

**25. ARQ-repair timestamps:** T_arq (substrat-onarım-arrival-rel) p50: rtt250 ~350ms; rtt1000 ~1074-1141ms;
rtt2500 ~2647-2875ms (~1-1.15×RTT).

**26. Deadline eşitsizlikleri:** rtt250: T_fec_p99(293-296) < deadline(305) < T_arq_p01(315-328) → SAĞLANIR.
rtt1000/2500: T_fec-T_arq-örtüşür → eşitsizlik-kurulmaz (TimingNonSeparating-o-RTT'de).

### UTILITY BOUNDARY (Phase-E)
**27. RTT-sınırı:** RTT≤250ms → fayda-VAR; RTT≥1000ms → fayda-YOK. (Ana-ayrıştırıcı; mekanizma FS §4.)

**28. Deficit-sınırı:** kombinatoryal `r≥deficit` (iso, her-RTT). Wall-clock: rtt250'de d1/d2/d4-hepsinde-
beneficial; yüksek-RTT'de FEC-path-kullanımı deficit↑ ile düşer (parity-daha-geç).

**29. Reverse-path-sınırı:** **wall-clock-utility-sweep'te reverse-symmetric-tutuldu; reverse-blackout/lossy
utility-ekseni AYRICA-KOŞULMADI** (harness-yeteneği-var — FQ blackout-pin; utility-matrisine-eklenmedi;
izleme-maddesi).

**30. Headroom-sınırı:** offered sabit (~0.6C, 40ms-pacing). **Headroom-ekseni (0.4/0.6/0.8C) AYRICA-
KOŞULMADI** (izleme-maddesi).

**31. Duplication-disabled:** iso-mod substrat-onarımı(§9+§11)-düşürür (saf-FEC). rtt-hepsi: r0-PENDING/r1-
FEC-COMPLETE. **§11-duplication-izole-ekseni ayrıca-taranmadı** (FK: bu-rejimde §11-dup=0-gözlendi).

**32. Production-duplication:** hyb-mod substrat-aktif (§9-ARQ-baskın; FK'da rk_dup=0). Wall-clock-hyb =
production-duplication-davranışı.

**33. MarginalFecEssential bölgeler:** rtt250, deadline=305ms, deficit 1/2/4 (r>=d-meet %100 / r<d-miss %100).

**34. MarginalFecBeneficial bölgeler:** rtt250, deadline-siz, deficit 1/2/4 (r>=d 46-77ms-hızlı; FEC-path 30/30).

**35. NoMarginalFecUtility_thisCondition:** rtt1000 + rtt2500, tüm-deficit (timing-overlap; parity-blok-
kapanış-gecikmesi).

**36. MarginalFecHarmful:** **ölçülmedi** (rtt2500 d4 T_fec≫T_arq ama completion-band-içinde; parity-byte-
waste-metriği completion'ı-kötüleştirmedi bu-koşuda).

**37. HybridResidualArqRequired:** iso r<d (FK, her-RTT): FEC-yetmez → ARQ-tamamlar.

### LOSS-SIGNAL + OBSERVABILITY (Phase-F/G)
**38. Loss-signal exact semantiği:** loss_windowed_ppm = PRE-RECOVERY erasure-metre (Confirmed-DATA-loss/
sent-window; retransmit-units=0; recovery-subtract-etmez; 8×ack_epoch-window; fuzzy-öncesi-güncellenir). FT §2.

**39. Cold/steady fidelity:** GERÇEK-forward-loss altında warm-steady sensör NONZERO-ve-true-yakın: 0%→0,
10%→81780, 20%→167646, 30%→271945 ppm (~%82-91). **Önceki loss=0 = broken-tek-ns-netem-artefaktı (DÜZELTME).**
Cold-start: time-threshold-donuk (lag/eksik). ⇒ LOSS-FIDELITY-DEFECT-RED (loss-yarısı-KAPANDI).

**40. Pre-decision observability:** utility-ayrıştırıcı=RTT; RTT/deadline **fusion-girdisi-DEĞİL** (loss_ppm/
bw/jitter/qd/ackconf). ⇒ mevcut-vektörle OBSERVABILITY-BLOCKED.

**41. Candidate-measurement separability:** srtt_us HALİHAZIRDA-hesaplanır (diag_rate), RTT'yi-temiz-izler:
100ms→112769us, 2500ms→2516564us (22×, örtüşme-yok) → utility-rejimini-AYRIŞTIRIR. No-leakage-doğrulandı.
⇒ ObservableWithCandidateMeasurement (srtt + deadline-slack).

**42. NİHAİ HÜKÜMLER:** aşağıda.

---

## NİHAİ HÜKÜMLER

### Liveness
> **COLD-START LIVENESS DEFECT CONFIRMED — PROGRESS DR REQUIRED.**
> Direct-integration (GERÇEK Sender) + wire-arm-uyumlu. Silent-deadlock-değil (geometrik-kopya first_delay'de)
> ama PTO-PreConverge-donuk + ACK-kapılı-state-ilerleme + 31 B/s-floor + ~61s@1200B-recovery = gerçek-cold-
> start-liveness-zaafı (hostile-ağ; first-flight-loss / reverse-blackout-at-start SIK). Fix bu-turda-UYGULANMADI;
> PROGRESS DR (bounded-cold-start-recovery + slow-channel-safety birlikte-optimize; 4-seçenek FP §5).

### Utility / Observability
> **FEC WALL-CLOCK UTILITY IDENTIFIED — CONTROLLER OBSERVABILITY BLOCKED; SENSOR/MEASUREMENT DR REQUIRED.**
> Proaktif-FEC marjinal-WALL-CLOCK-faydası **RTT≤250ms'te POZİTİF-ÖLÇÜLDÜ** (MarginalFecBeneficial; deadline=
> 305ms'te MarginalFecEssential) — son-turda-kurulamayan-gerçek-zaman-kazancının faithful-harness'ta-İLK-
> pozitif-ölçümü. RTT≥1000ms'te NoMarginalFecUtility (emisyon-modeline-bağlı; parity@blok-kapanış+ACK-clock).
> Fayda-rejimi TEK-timing-ailesinde (RTT≤250ms) görüldü → "calibration-may-proceed"-EŞİĞİ-KARŞILANMADI. Ayrıca
> ayrıştırıcı-RTT mevcut-fusion-vektöründe-YOK → OBSERVABILITY-BLOCKED. srtt-candidate temiz-ayrıştırır →
> SENSOR/MEASUREMENT DR (srtt+deadline-slack karar-vektörüne; loss-sensörü-zaten-sağlam).

### Önceki-tur düzeltmeleri (bu tur faithful-harness ile)
1. "loss_windowed_ppm=0 → sensör-kör" → **YANLIŞ** (broken-tek-ns-netem-loss-uygulamadı). Sensör-SAĞLAM.
2. "FEC-wall-clock-utility-ölçülemez (netem-yok)" → **ÇÖZÜLDÜ** (faithful-two-netns → RTT≤250ms-fayda-ölçüldü).
3. Cold-start-stall (FK'da-gözlenen) → **SINIFLANDIRILDI** (gerçek-transport-liveness-özelliği; DEFECT).

## DEFERRED / KOŞULMAYAN (dürüstlük; sessiz-tavan-yok)
- Pin-4 wall-clock-faithful (d5) ayrıca-koşulmadı (kombinatoryal-FK-var).
- Reverse-path utility-ekseni (blackout/lossy) utility-matrisine-eklenmedi (harness-yeteneği-var).
- Headroom-ekseni (0.4/0.6/0.8C) taranmadı.
- §11-duplication-izole utility-ekseni ayrıca-taranmadı.
- MarginalFecHarmful pozitif-ölçülmedi (byte-waste-metriği ayrı).
- Loss-membership-eşiği (fuzzy_ratio=0@%30-loss) = ayrı-kalibrasyon-konusu, KAPSAM-DIŞI.

## KAPSAM (değişmeyenler)
Üretim fuzzy-config · LeftoverOnly-default · üretim-byte-layout · F15/P15 · DR-1f/1d/Kesit-2-full/Sprint-6 —
HİÇBİRİ değişmedi/açılmadı. PR draft; merge YOK. Cold-start-fix + fuzzy-tuning + RTT/deadline-wiring hepsi
AYRI-onaya-tabi DR'lar.
