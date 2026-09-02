# FS-PREREG-v1.2 — CONSOLIDATED FROZEN PRE-REGISTRATION (DR-FEC-FS-1)

> **STATÜ: FROZEN.** Bu belge FS-PREREG-v1.1'i (sha `60353e99…`) **SUPERSEDE** eder ve FS-HARNESS-A + FS-HARNESS-B
> boyunca biriken TÜM bağlayıcı netleştirmeleri TEK konsolide otoriteye toplar. Freeze SONRASI çelişki-sırası:
> **v2-prompt > FS-PREREG-v1.2 > v1.1(superseded) > coverage-v1.1 > v5-icra > denetim-raporları.**
>
> **BAĞLAYICI:** Bu hash freeze edildikten SONRA hiçbir FS-1 result-bearing-run'dan ÖNCE outcome-bearing-veri
> ÜRETİLMEMİŞTİR (§37 checkpoint teyidi). v1.1'in donmuş nicelikleri (deadline/horizon/faithfulness/oracle/
> censoring/verdict-path) — §D'de değişmeyenler — AYNEN geçerli. Detaylı A-türetmesi: FS-PREREG-v1.2-DRAFT.md
> (A1.0–A1.11; bu FINAL'e INCORPORATED, commit cd336e9 öncesi). Aşağıdaki §A onu ÖZETLER; §B slice-B'yi EKLER.

---

## §A — HARNESS-A KLARİFİKASYONLARI (INCORPORATED; DRAFT A1.0–A1.11 özeti)
```text
A1.0  t_repair_usable = redundancy'nin GERÇEKTEN-kullanılabilir anı (FEC: reconstruct-TAMAM + byte-hazır,
      codec-latency dahil; A1: dup-arrival). USABLE_BEFORE_ARQ = t_repair_usable < t_arq_arrival (parity-arrival DEĞİL).
A1.1  A1-EARLY = BLOK-TOPLAM redundancy (configured_r selected-source × 1-kopya; K×R DEĞİL). Target deficit-BAĞIMSIZ
      (a1_duplicate_target_set; lowest-hash-rank). dup_seed=H(m,"A1_DUP_TARGET") ⟂ loss_seed=H(m,"LOSS").
A1.2  C-P4 fairness = realized_sealed_wire_bytes (configured_r-eşitliği TEK-BAŞINA-YETMEZ). BUDGET_MISMATCH-guard.
A1.3  Blind-dup HONESTY: A1 yalnız (target ∩ D) kesişiminde kurtarır; aksi ARQ-fallback. a1_dup_hits/misses.
      Pin: A1-MISS ≡ A0 byte-özdeş (dup fayda-sağlamaz, yalnız wire-öder).
A1.4  PIN-A direct emission-side neutrality (post-feedback state-eşitliği İSTENMEZ). PIN-E egress-fairness
      (her paket — dropped-dahil — serialize; bedava-lane-YOK).
A1.5  F0 RECOVERED-scope + ESTIMATOR-PURITY (6.pin): decode→RecoveredControl RTT/BtlBw SENTEZLEMEZ. Hybrid residual-ARQ.
A1.6  Crate: F0 kama-integration'da (INV-ENGINE-SOC — adaptive-transport FEC-agnostik). Production lib-grafiği
      GENİŞLEMEZ (experimental-fs-harness test-only). flag-OFF byte-özdeş (INV-CS-5).
A1.7  F2 (interleaved parity) DROPPED-bu-kampanya (sealed DATA-before-parity ihlali) → PENDING DR-FEC-INT-1.
A1.8  → §B.2'ye taşındı ve KESİNLEŞTİ (B_REAL_TAIL_ERASURE).
A1.9  INVALID ⟂ INCOMPLETE ortogonal: admission-gate (faithful∧overlap) = VALID-mi; completion_ns=None =
      LEGITIMATE-INCOMPLETE veri-noktası (censoring; INVALID-DEĞİL).
A1.10 FS-5 PLUMBING_GAP çözüldü: path_timing_snapshot() READ-ONLY ledger; karar-tüketmez; production-değişiklik-YOK.
A1.11 → §WIRE-CORRECTION'a (aşağıda) KESİNLEŞTİ.
```

## §WIRE-CORRECTION — GROUNDED SEALED-DATAGRAM (review-blocking-fix; DONDU)
```text
TEK sealed-datagram modeli: fs_sealed_len(E) = E + FS_SHARD_FRAMING_BYTES.
FS_SHARD_FRAMING_BYTES = 37 = EXTENDED_OUTER_OVERHEAD(26: h_ext4+wfmt1+wflags1+pn4+tag16) +
  FEC_INNER_HDR_LEN(11: iv1+kind1+flags1+pid1+block4+esi2+k1). E=64 → sealed = 101 B.
SOURCE ve REPAIR shard sealed-BOYUTU AYNI (INV-ENGINE-SOC: transport boyutla ayırt-edemez; fark yalnız
  sealed-payload İÇİNDEKİ 1-byte inner_kind 0x01/0x02). Otorite: kama-noise-wire protector.rs "N7: 101B @ E=64".
ESKİ uydurma PARITY_FRAME=108 (44 framing) SUPERSEDED — gerçek 101/37; +7B over-count; "44" cold-start
  CTRL_CS_PROBE frame'iyle (26+6+12) RASTLANTISAL çakışmaydı, farklı-frame-sınıfı.
UYGULAMA: source|retransmit|A1-dup|F0-parity HEPSİ fs_sealed_len'den serialize+sayılır; OpaqueUnit.size = sealed
  wire-boyu (Sender wire-reality'de pace-eder). Muhasebe VE channel-serialization AYNI primitiften.
Pin: A1_WIRE_ACCOUNTING_REAL_DATAGRAM — bir dup: payload+framing==realized==channel-serialization AGREE.
Sonuç: A1 r=2 = F0 r=2 = 202 B (matched-r → matched-wire; C-P4 gerçek-adil).
YASAK: PARITY_FRAME=108 veya herhangi arm-özel synthetic frame-size const YENİDEN-EKLENEMEZ.
```

---

## §B — HARNESS-B KLARİFİKASYONLARI (YENİ; slice-B)

### B.0 — BASELINE FREEZE (BAĞLAYICI)
```text
A0 = MEVCUT ÜRÜN ARQ substratı — FS kampanyası boyunca DONUK. Slice-B'de Sender-recovery-fix UYGULANMADI:
  RTO/TLP YOK · yeni-tail-loss-recovery YOK · ack_confidence→RECOVER_FWD wire'lanmadı · A0 iyileştirilmedi.
Her Sender-recovery-fix baseline'ı MUTASYONA uğratır → ayrı ürün-DR (DR-ARQ-TAIL-1) gerektirir (bu kampanya-SONRASI).
Tail-loss FS-kanıtı = o gelecek-DR'nin requirements-kanıtı.
```

### B.1 — REVERSE-PATH DETERMINİSTİK TRACE (fs_trace.rs ReverseSpec)
```text
ReverseSpec{Clear | Scheduled{blackout_intervals_ns [lo,hi), loss_permille (ordinal-keyed iid), seed}}.
Reverse-blackout GROUND-TRUTH — eksik-ACK'ten ÇIKARILMAZ (şema gerçektir). recovery = aralık-DIŞI otomatik-PASS.
Forward'dan TAM-AYRI: decide_reverse + reverse_decision_log_hash + reverse_counts (ayrı-görünürlük).
Arm-BAĞIMSIZ ŞEMA (spec kollar-arası özdeş; realizasyon kola göre değişir). Determinism: aynı-reverse-trace×3
  → identical reverse-hash. Ledger'da forward_schedule_id / reverse_schedule_id AYRI-görünür.
```

### B.2 — TAIL-ERASURE = B_REAL_TAIL_ERASURE (SALT-KARAKTERİZASYON; Sender-fix YOK)
```text
Metadata (yalnız-gözlem): deficit_position{Head/Middle/Tail/Mixed/None} · deficit_is_tail · source_count ·
  largest_acked_observed · tail_recovery_path{NoneStalled/Arq/Duplicate}.
SINIFLANDIRMA (KESİN): A0 tail-deficit → PTO SADIK-servis-edilir (runner-defekt-YOK) → detect_time_losses
  "seq<largest_acked" yapısal-önkoşulu (loss.rs:383; RFC9002 §6.1) son-unit'i asla loss-ilan-edemez; RTO/TLP-YOK
  → stall (delivered=k−1, largest_acked=k−2, completion=None) — AMA admitted (faithful∧overlap). = B_REAL_TAIL_ERASURE,
  HARNESS-DEFEKT-DEĞİL. **Horizon-uzatma ÇÖZMEZ** (yapısal, timeout-değil).
FS-1 KURALI: tail-pozisyonları DIŞLAMA; sessiz-telafi-YOK; frozen-horizon'a göre censor. Analiz tail-vs-nontail
  STRATIFY etmeli — tail-spesifik-niş universal-FEC-avantajı olarak ETİKETLENEMEZ.
ZORUNLU-CÜMLE (rapor): "Current product ARQ structurally does not recover this tail-erasure class under the
  measured conditions; proactive redundancy may therefore have genuine utility in this regime. Product ARQ
  completeness remediation is deferred to DR-ARQ-TAIL-1 and is outside this FS baseline."
```

### B.3 — PAIRED REVERSE-BLACKOUT REPLAY (C-P3 dependency)
```text
C-P3 kolları (A0/A1/FEC) AYNI deterministik: forward-source-trace + reverse-loss-trace + reverse-blackout-aralığı.
Pairing = TRACE-IDENTITY-eşitliği (aynı-PRNG-seed TEK-BAŞINA yetmez): configured_schedule_hash (forward+reverse
  CONFIGURED-şema) kollar-arası ÖZDEŞ. Arm-çıktısı FARKLI-olabilir; channel-intervention (şema) FARKLI-OLAMAZ.
```

### B.4 — F1 (EARLY LOGICAL-CLOSE FEC)
```text
Experimental named-flag; production-default OFF; wire-format-değişikliği-YOK; DATA-before-parity-ihlali-YOK.
PIN-C (BAĞLAYICI): t_parity_wire(F1) < t_parity_wire(F0) pre-result mechanism-fixture'da GÖSTERİLMELİ.
  eligibility-ilerlemesi TEK-BAŞINA YETMEZ. t_repair_usable(F1) de kaydedilir.
KESİN-BULGU (fixture + investigation): t_parity_wire = f(enqueue-close, per-tick-DATA-first[sender.rs:2290],
  pacer-leftover-token). early-close YALNIZ enqueue'yu ilerletir; DATA-first+pacer regime'inde wire-time
  ilerlemez → **F1 = STRUCTURALLY NON-EFFECTIVE UNDER CURRENT ORDERING** (sıradan-perf-fail DEĞİL;
  ordering-mührü GEVŞETİLMEZ). Real-repair-decode = F0 (aynı K=8 tam-blok).
```

### B.5 — F3 (TIME/DEADLINE PARTIAL-BLOCK FEC)
```text
Experimental named-flag. Trigger AÇIK+DETERMINİSTİK: timer/deadline → partial logical-block-close (K'<K) →
  fixed-prereg-R → GERÇEK codec repair. PathTimingState adaptive-karar YOK (F3 ≠ F4).
PIN-C: t_parity_wire(F3) < t_parity_wire(F0) en-az-bir pre-result fixture'da. KESİN-BULGU: partial-block (K'<K)
  parity'nin önünde daha-az DATA → wire-time GERÇEKTEN ilerler → **WireTimeAdvanced** (ordering-mührü korunur;
  interleaved-YOK; close-API K-agnostik → Sender-değişikliği-YOK). Real-decode: profile-0x02(K=4) byte-equal.
```

### B.6 — F4 (ADAPTIVE-K MECHANISM ARM)
```text
YALNIZ F4 Sender::path_timing_snapshot() tüketebilir. Karar-alanları: srtt/min_rtt/rttvar/queue_delay_ns +
  sample_present/rtprop_fresh. FROZEN-kural (f4_select_k; ADR-17 saf-integer; RNG-YOK; GELECEK-bilgi-YOK;
  post-completion-etiket-YOK; FEC-utility-outcome K'yi ETKİLEMEZ): confidence-kapısı(sample∧fresh değilse
  K_COLD=2); base-RTT bucket <50ms→1 · 50–150ms→2 · ≥150ms→3. Eşikler golden-vector-DONUK.
PIN-D: iki-FARKLI prereg-path-state → FARKLI-K. F4 = MECHANISTIC/EXPLORATORY — path-timing discriminative-
  validity / online-selectability / controller-readiness / production-calibration KURMAZ (FS-5B).
```

### PIN-C / PIN-D (özet)
```text
PIN-C (F1/F3): t_parity_wire(candidate) < t_parity_wire(F0) [WIRE-time; eligibility yetmez] VEYA açık
  STRUCTURALLY_NON_EFFECTIVE_UNDER_CURRENT_ORDERING. Ordering-mührü GEVŞETİLMEZ; sahte-fail-gizleme-YOK.
PIN-D (F4): iki-state→farklı-K mekanizma-kanıtı; no-future-leakage; exploratory-etiket korunur.
```

---

## §D — v1.1'DEN DEĞİŞMEYENLER + KESİNLEŞENLER
```text
ARM-SET (F2-çıkışı sonrası KESİN): A0 · A1-EARLY · F0 · F1 · F3 · F4 = ALTI arm. F2 DROPPED-bu-kampanya /
  DEFERRED-sealed-ordering / NOT-REJECTED-family. FS-1 = 6 × 18-cell × 5 (v5 §canonical) düzenine göre.
STATISTICS: N=10 paired · exact hits/N · ≈%-YASAK (v1.1).
DEADLINE/HORIZON/FAITHFULNESS/ORACLE/CENSORING: v1.1 §B0.2–B0.6 AYNEN (deadline-formülü fizik-tabanlı,
  outcome-bağımsız; horizon-censoring; faithfulness-tolerances; oracle-ordering; snapshot-contamination §B0.6
  — A1.5/A1.10/B.6 GÜÇLENDİRİR: yalnız-F4 path-state tüketir, diğerleri ledger-only).
COST — İKİ AYRI KAVRAM (§8): (A) INTERVENTION-BUDGET (C-P4 coding-vs-dup fairness) = realized_sealed_wire_bytes
  (matched-r → r=2 A1=202B, FEC=202B; BUDGET_MISMATCH-guard AKTİF). (B) TOTAL-OUTCOME-COST (Pareto/oracle) =
  source + proaktif-dup + proaktif-FEC-repair + arq_retransmit_bytes + forward-recovery + diğer. FEC-repair-usable-
  before-ARQ-arrival AMA emitilen-ARQ = LEGITIMATE downstream-cost → spurious_retransmits + arq_retransmit_bytes
  AYRI kaydedilir; SPURIOUS-ARQ **BUDGET_MISMATCH-DEĞİL** (hybrid-coordination-cost).
VERDICT-PATH: FEC FULL-GO | NICHE-GO | RESEARCH-ONLY | K4-RETREAT (agent-önerir; Abkarada-onaylar).
STRICTLY-DOMINATED / hard-disqualifier (unbounded|starvation|invariant-ihlal|BUDGET_MISMATCH-coding-claim'inde): v1.1 §B0.8.
```

## PROVENANCE
```text
base       : FS-PREREG-v1.1 sha256=60353e99715cd4e51a42c53a94b6c1ad6963d8c3b3ef017ebc1bf2b9c3aaadf7 (SUPERSEDED-BY-v1.2)
draft-src  : FS-PREREG-v1.2-DRAFT.md (A1.0–A1.11 detaylı-türetme; INCORPORATED)
coverage   : FS-COVERAGE-PLAN-v1.1.md · seal-maps FS-SEAL-COLLISION-MAPS.md
harness    : FS-HARNESS-A-CHECKPOINT.md + FS-HARNESS-B-CHECKPOINT.md (mechanism-pins GREEN)
code-commit: cd336e9 (FS-HARNESS-B mechanism) — bu freeze o kod-durumuna karşılık gelir
statü      : FROZEN. Bu hash'ten ÖNCE hiçbir FS-1 result-bearing-run/outcome-veri ÜRETİLMEDİ.
sha256: da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
```
