# FS-HARNESS-A — DESIGN + PROGRESS (DR-FEC-FS-1 v5; Abkarada 2-karar + 4-pin)

**Statü:** FS-HARNESS tek-mantıksal-faz, iki-slice (A+B). Result-bearing-YOK; FS-1 YALNIZ her-iki-slice +
tüm-pin'ler-yeşil olunca koşulur. Bu belge slice-A tasarımı + ilerlemesi. Sınırlar: DR-FEC-FS-1 §2.

## 0. F2 DUR-B — HÜKÜM (Abkarada onayı)

> **F2 — DROPPED FROM DR-FEC-FS-1 PRE-ORACLE CAMPAIGN; DEFERRED BY SEALED ORDERING CONFLICT.
> NOT REJECTED AS A FEC FAMILY.**

DATA-before-parity mührü (`sender.rs:2287`) bu turda AÇILMAZ. F2 genel-FEC-family-reddi DEĞİL; yalnız bu
kampanyada interleaved-parity için transport-scheduling-semantiği değiştirilmez. Gerekirse oracle-sonrası
ayrı **DR-FEC-INT-1 (INTERLEAVED PARITY / ORDERING AMENDMENT)** ile — şu koşullarda: F1/F3 mekanik-parity-
head-start-üretemez VEYA oracle FEC-specific-upside-yok AMA sealed-ordering FEC-uzayını yapay-kısıtladığına
güçlü-kanıt. Şimdi açmak scope-explosion. **FS-1 canonical: 18 × 6 × 5 = 540.**

## 1. FS-HARNESS KADANSI (Abkarada onayı)
```text
FS-HARNESS-A (bu iş): trace-replay + A1-EARLY + F0-integration + instrumentation/provenance
FS-HARNESS-B        : F1 + F3 + F4 + family-mechanism-pins
FS-HARNESS COMPLETE = A + B (arada product/result-hükmü YOK)
No FS-1 result-bearing run until BOTH slices complete ∧ tüm family-sanity/instrument pin'leri yeşil.
```

## 2. DÖRT BAĞLAYICI PİN (kaydedildi)

```text
PIN-A  A1-EARLY STATE-NEUTRAL: proactive-duplication ARQ/loss/recovery/PTO/RTT-estimator-semantiğini
       DEĞİŞTİRMEZ. MUST-NOT: loss-state-üret · RECOVER_FWD-zorla · ARQ-retransmit-eligibility-değiştir ·
       PTO/recovery-timer-yeniden-kur · RECOVERED-semantics-üret · RTT/BtlBw/loss-estimator'a-sahte-feedback.
       MAY-ONLY: same-source-payload bounded-redundant-transmission + exact-redundancy-byte-accounting.
       Test: a1_early_duplicate_is_transport_state_neutral — before/after TAM-snapshot karşılaştırması
       (loss-state · ARQ-eligibility · PTO-deadline/state · recovery-mode · retx-counters · RTT-estimator ·
       BtlBw-estimator · cold-start-state · FEC-state); A1 ON/OFF arasında duplicate-emisyon-HARİCİNDE-özdeş.
PIN-B  PAIRING TRACE SEMANTICS FROZEN: **exact-deficit = INITIAL-SOURCE-TRANSMISSION ERASURE** (source-
       identity erasure DEĞİL) — key=(source_id, role, attempt); D yalnız role=InitialSource siler;
       retransmit(A0)/duplicate(A1)/parity(F0) BAĞIMSIZ-admissible (default PassAll; redundancy-policy
       D'den bağımsız scriptable). natural-IID/burst=arm-independent-deterministic. wire-ordinal-drop-replay
       GEÇERSİZ. A1 duplicate-selection D'den BAĞIMSIZ + future-loss-knowledge-YOK. → [UYGULANDI §4; DÜZELTİLDİ].
PIN-E  A1-EARLY EGRESS-FAIRNESS (Abkarada düzeltme-2): runner-duplication Sender-recovery-STATE'ini bypass-
       edebilir AMA experiment'in PHYSICAL-egress-cost'unu bypass-EDEMEZ. Her duplicate: aynı-UDP/netem-path ·
       gerçek link-serialization/bandwidth · exact-sealed-wire-accounting · bottleneck-queue/source-displacement.
       Bedava-unmetered-lane YOK. Sender-pacing-SONRASI dup'lanıyorsa bu-ayrım-açıkça-ölçülüp-raporlanır.
       Test: a1_early_duplicate_consumes_wire_budget.
PIN-C  F1/F3 MECHANISM PIN (slice-B): yalnız-eligibility DEĞİL, GERÇEK wire-time-advancement:
       t_parity_wire(candidate) < t_parity_wire(F0) prereg-mechanism-fixture'da, DATA-before-parity-
       ihlal-etmeden. Yapamıyorsa → "STRUCTURALLY NON-EFFECTIVE UNDER CURRENT ORDERING" (ordering-
       constrained; yapay-"başarısız"-DEĞİL).
PIN-D  F4 MECHANISTIC/EXPLORATORY (slice-B): PathTimingState-mevcudiyeti online-selectability-İMA-ETMEZ;
       FS-5B-açık → controller-iddiası-YOK. FS-1'de F4 = "adaptive-K mekanizması çalışıyor + farklı-K
       üretebiliyor" kanıtı (policy-iddiası DEĞİL).
```

## 3. A1-EARLY — §11 REUSE-REJECTION + SEÇİLEN YOL (PIN-A)

**§11 forward-recovery duplication machinery REUSE-EDİLEMEZ (state-neutral değil):** `plan_forward_
duplication` (sender.rs:3248) YALNIZ `SenderState::RecoverFwd`'de çağrılır (2283), `pending_lost`'u
drain-eder, ve **`mark_tombstone_superseded(retx_count++)` çağırır → ARQ-retransmit-eligibility-DEĞİŞTİRİR**.
Bunları A1 için kullanmak PIN-A'yı doğrudan ihlal ederdi (recovery-mode-zorla + tombstone-supersede).

**Seçilen yol — RUNNER-LEVEL source-duplication (yapısal state-neutral):** A1-EARLY, harness-runner'ında
Sender bir source-DATA `TransmitDecision` verince, sealed-datagram'ı r+1 kez (orijinal + r kopya) erken-
timing'de kanala verir. **Sender kopyalardan HABERSİZ** (kendi state-machine'i dokunulmaz) → PIN-A yapısal-
sağlanır. Receiver zaten dedup-eder (tek-ACK/seq). Exact-accounting: r × sealed-source-bytes.
- Wire: yeni-format-YOK (source-kopyası = mevcut source-wire).
- Ordering: kopyalar DATA (source-sınıfı) → DATA-before-parity-mührü İHLAL-ETMEZ.
- state-neutrality-testi (slice-A tamamında): aynı Sender-event-dizisi altında Sender-iç-state
  (pending_lost · dup_queue · srtt/btlbw/loss · PTO-schedule · tombstone/retx) dup-ON ile dup-OFF ÖZDEŞ;
  yalnız wire-emission-sayısı + byte-accounting farklı.

## 4. TRACE-DRIVEN PAIRED CHANNEL — UYGULANDI (PIN-B)  ✅

`crates/adaptive-transport/src/fs_trace.rs` (feature `experimental-fec-budget`; default-OFF; flag-OFF
byte-özdeş): `TracePacketId{Source|Repair|Duplicate|Control}` + `TraceSpec{ExactDeficit|NaturalIid|
NaturalBurst}` + `TraceChannel::decide()` (saf-fonksiyon; RNG-YOK). Semantik:
- ExactDeficit: yalnız adlandırılmış `Source{block,esi}` düşer; repair/duplicate/control PASS →
  **deficit kollar-arası ÖZDEŞ** (arm-invariant).
- NaturalIid: per-semantic-id FNV-keyed drop (aynı-id→aynı-karar; arm-independent).
- NaturalBurst: time-domain [lo,hi) interval (packet-count'tan bağımsız).
- Determinism-pin: `decision_log_hash` — aynı-trace × N → identical.

**DÜZELTME (Abkarada düzeltme-1; attempt-aware):** ExactDeficit artık `drop_initial`(role=InitialSource) +
`RedundancyPolicy{PassAll|Iid}`; key=(source_id, role, attempt). Böylece kayıp-source'un **retransmit'i,
duplicate'ı, parity'si BAĞIMSIZ-admissible** — eski "source-id-keyed" biçimi A0-retransmit'i de öldürüp
FEC/redundancy lehine bias-üretirdi (yakalandı+düzeltildi).

**Testler (6/6 PASS):** `exact_deficit_erases_only_initial_source_transmission` (retransmit/dup/parity PASS) ·
`initial_source_deficit_is_arm_invariant_and_recovery_fair` (D özdeş ∧ recovery-attempt-hiç-düşmez) ·
`redundancy_policy_independently_scriptable` · `natural_iid_deterministic_arm_independent_attempt_aware` ·
`natural_burst_time_domain` · `trace_determinism_thrice_identical_hash`.
Kapılar: fmt + clippy(-D, 2cfg) + FLAG-OFF **382** (değişmedi; fs_trace flag-OFF-derlenmez) +
ALL-FEATURES **→507** + A1-pin sağlam.

## 5. FS-HARNESS-A KALAN İŞ (dürüst; henüz result-bearing-YOK)
```text
[✓] PIN-B TraceChannel (fs_trace.rs; 5-test)
[✓] F2 DUR-B verdict + A1-EARLY §11-reuse-rejection kararı (PIN-A yolu)
[ ] A1-EARLY runner-level duplication emission + exact-byte-accounting (duplicate-selection D-bağımsız)
[ ] a1_early_duplicate_is_transport_state_neutral testi (before/after TAM-state-snapshot; PIN-A)
[ ] a1_early_duplicate_consumes_wire_budget testi (aynı-path/serialization/accounting; PIN-E egress-fairness)
[ ] F0 integration (mevcut open_fec_block_authority yolu runner'a bağlanır)
[ ] 9-timestamp instrumentation (t_source_loss..t_block_complete tek-clock) + metadata (path_timing ledger-only)
[ ] per-run instrument-chain (binary-provenance · arg-diff · trace-hash · overlap-gate)
[ ] fs_runner (arm=a0|a1|f0; --trace; --arm; deterministic channel) + loopback-smoke
FS-HARNESS-B: F1 · F3 · F4 (PIN-C/PIN-D) + family-mechanism-pins.
```
Yukarıdakiler tamamlanana + pin'ler-yeşil olana kadar **FS-1 KOŞULMAZ**.
