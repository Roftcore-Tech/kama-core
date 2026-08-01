# FS-MULTIBLOCK-SCHEDULING-AUTHORITY (FINAL BUILD S0 gates)

> **STATÜ: S0-A + S0-B GATES RESOLVED — CONTINUE (DUR-B YOK).** İki-semantik-yön-gate spine-inşasından-ÖNCE
> koddan-türetildi. S0-A: F4-yön human-declared-OPTION-D ile-tutarlı (çelişki-YOK). S0-B: gerçek sender-wide
> DATA-first-gate → F3 çok-blokta parity-wire-time ilerletemez → **F3 STRUCTURALLY NON-EFFECTIVE UNDER
> MULTI-BLOCK ORDERING** (dürüst-sınıflandırma; scheduler DEĞİŞTİRİLMEZ; arm executable). Kod-YOK bu-turda
> (S0 = audit/gate). FS-1 KOŞULMADI. sender.rs/loss.rs DEĞİŞMEDİ.

## §S0-A — F4 DIRECTIONAL-HYPOTHESIS GATE
Koddan (fs_harness.rs f4_select_k; OPTION-D direct-K):
```text
predicate (min_rtt_ns = base RTT)          → CodecK  → profile_id
  base <  50_000_000ns (<50ms)             → K2      → 0x01
  base ∈ [50_000_000, 149_999_999] (50-150ms) → K4  → 0x02
  base ≥ 150_000_000ns (≥150ms)            → K8      → 0x03
  confidence-gate (¬sample_present ∨ ¬rtprop_fresh) → K_COLD = K4
```
GRID (RTT∈{50,250,1000}ms; base≈RTT) SEÇİMLERİ:
```text
  RTT=50ms   → base≈50ms   → K4 (0x02)   [50ms sınırda; 50_000_000ns → 50-150 bucket → K4]
  RTT=250ms  → base≈250ms  → K8 (0x03)
  RTT=1000ms → base≈1000ms → K8 (0x03)
  → grid YALNIZ K4(RTT50) ve K8(RTT250/1000) üretir; K2 realize-edilebilir AMA 18-hücre-grid'inde EXERCISE-EDİLMEZ
    (K2 base<50ms ister; mechanism-fixture'da synthetic base=30ms ile pinlenir).
```
DIRECTIONAL HYPOTHESIS (açık; MECHANISTIC/EXPLORATORY — outcome-görülmeden):
```text
YÖN: artan base-RTT → BÜYÜK K (K4→K8). Design-intent-rasyoneli: yüksek-RTT'de proaktif-redundancy-overhead'i
  (R/K oranı) DÜŞÜK-tutmak için büyük-blok (K8: R=2 → %25 overhead vs K2: R=2 → %100). Düşük-RTT'de ARQ-ucuz →
  küçük-blok yeterli. **Bu rasyonel VALIDATE-EDİLMEDİ** (F4 exploratory; optimal-yön-iddiası YOK).
VERDICT: kod-yönü = human-declared OPTION-D mapping (region1→K2·region2→K4·region3→K8). Frozen-authority'de
  ÇELİŞEN "high-RTT→smaller-K" hipotezi YOK (grep-boş). → **DIRECTIONAL-HYPOTHESIS CONTRADICTION YOK → CONTINUE.**
  v1.3 kuralı numeric-predicate + gerçek-CodecK kullanır (yukarıdaki tablo; region-label DEĞİL).
```

## §S0-B — MULTI-BLOCK SCHEDULING AUTHORITY (koddan)
Otorite: `schedule_transmit` (crates/adaptive-transport/src/sender.rs:2265-2299). Per-tick çağrı-sırası:
```text
  [1] (arq-share) plan_arq_retransmits (K_ARQ_SHARE'de)
  [2] drain_new_data          (sender.rs:2280) — TÜM kuyruktaki DATA (herhangi-blok)
  [3] RecoverArq/RecoverFwd
  [4] drain_due_duplicates    (2286)
  [5] plan_fec_parity         (2290) — parity, DATA/recovery SONRASI. "DATA-ÖNCE-sıralaması MÜHÜRLÜ" (DR-S5-1/Commit-K)
```
**DATA-before-parity = SENDER-WIDE per-tick (per-block DEĞİL).** drain_new_data kuyruktaki HERHANGİ-blok'un DATA'sını
plan_fec_parity'den ÖNCE boşaltır → parity(block-n) tx_queue'da-bekleyen sonraki-blokların-DATA'sını GEÇEMEZ.

5-SORU CEVABI:
```text
Q1 block-n-kapandıktan sonra parity'si sonraki-blok-DATA'sından ÖNCE scheduler'a çıkabilir mi? → HAYIR. Sender-wide
   DATA-first: pending_fec-parity, tx_queue'da-bekleyen (herhangi-blok) DATA drain-edilene-kadar bekler (2280<2290).
Q2 DATA-before-parity kapsamı? → SENDER-WIDE queue (per-block/per-workload DEĞİL). Her-tick tüm-admissible-DATA → sonra parity.
Q3 F0 sırası? → bulk-48-symbol-workload'da: DATA(TÜM-bloklar, paced) → parity(TÜM-bloklar). "DATA(b0)→parity(b0)→
   DATA(b1)" DEĞİL (b0-parity, b1..-DATA'yı geçemez).
Q4 tek-blok-fixture'da t_parity_wire(F3)<t_parity_wire(F0) neden? → ParityWireModel TEK-BLOK'u izole-modelledi:
   parity kendi-bloğunun data_before_parity'sinden sonra (K'=4 < K=8). Bu TEK-BLOK-doğru AMA sender-wide-değil.
Q5 48-symbol/çok-blok'ta korunur mu? → HAYIR. Bulk-workload'da 48-DATA-hepsi (herhangi-blok) parity'den-önce →
   t_parity_wire(F3-ilk-parity) ≈ t_parity_wire(F0) ≈ ~48-DATA-sonrası. F3-partial-blok wire-time-AVANTAJI-YOK.
```

## §S0-B VERDICT — F3 CLASSIFICATION
```text
**F3 STRUCTURALLY NON-EFFECTIVE UNDER MULTI-BLOCK ORDERING** (bulk 48-symbol workload). Gerçek sender-wide
  DATA-first-gate F3-partial-blok-erken-parity'sinin fiziksel-wire-time'ı ilerletmesini ENGELLER (tüm-source-DATA
  tüm-parity'den-önce). Tek-blok ParityWireModel-fixture'ı TEK-BLOK-model-olarak GEÇERLİ-KALIR (silinmez) AMA
  çok-blok-workload-implication'ı bu-sınıflandırma ile DÜZELTİLİR.
SCHEDULER DEĞİŞTİRİLMEZ (S0-B mandate). F3 EXECUTABLE-KALIR: gerçek RsSimdCodec 12×K'=4-blok encode/decode +
  full-workload-completion + coding-recovery (wire-time-avantajı-DEĞİL). F1 de aynı-nedenle NON-EFFECTIVE (full-K
  early-close; sender-wide-DATA-gate). → **CONTINUE (DUR-B YOK; arm executable, ordering-mührü korunur).**
BİLİMSEL-ETKİ: çok-blok gerçek-ordering'de NE F1 NE F3 parity-wire-time-ilerletir → "emission-timing" hipotezi
  bulk-multi-block-workload'da STRUCTURALLY-NON-EFFECTIVE. Ayrışan-mekanizmalar: F0-coding-recovery · F3-smaller-block-
  coding + artan-repair-count (§15) · F4-adaptive-K. F3>F0 (varsa) block-size/repair-count/coding'e-atfedilir,
  timing'e DEĞİL (§13 decomposition).
```

## PROVENANCE
```text
branch/HEAD : d1-sprint5c / (bu commit) · kod-değişikliği-YOK (S0 audit/gate)
authority   : sender.rs:2265-2299 (schedule_transmit; DATA-first sealed DR-S5-1/Commit-K) · fs_harness.rs f4_select_k
FS-PREREG-v1.2 : da4af3c4… immutable · FS-1 0/540 · sender.rs/loss.rs ZERO-DIFF
RULE-5 (multi-block semantic closure) + RULE-6 (rate-grounding) governance'a eklenecek (v1.3).
```
