> **[SUPERSEDED-BY-v1.1 — 2026-07-31]** Bu v1.0 (sha `cf005739`) tarihsel-referans olarak KORUNUR;
> donmuş-nicelikler [FS-PREREG-v1.1.md](FS-PREREG-v1.1.md)'deki dar-isimli-amendmanlarla güncellendi
> (B0-freeze). Çelişkide v1.1 üstündür.

# FS-PREREG — DR-FEC-FS-1 v2 EXECUTION BUDGET + COVERAGE + STATISTICS FREEZE

**Statü:** Sonuç-taşıyan-koşulardan ÖNCE dondurulur (§5/§27-6). Bütçe, hücre-üretim-kuralı, confirmatory-
set, arm-seti, pairing, eşikler, censoring, seeds sonuca-göre DEĞİŞTİRİLMEZ. Hash+timestamp §PROVENANCE.
Eşlik: [FS-COVERAGE-PLAN.md](FS-COVERAGE-PLAN.md) (exact cell/sampling; ayrı-hash). Sınırlar: DR-FEC-FS-1 §0.

## 0. ÖLÇÜM FELSEFESİ
A0 = ARQ+0 birincil-baseline. FEC-arm = ARQ + proaktif-coded-redundancy. Ölçülen = ARQ üzerine eklenen
proaktif-FEC'in **marjinal ürün-faydası**. §1-yasak-iddialar bağlayıcı (ARQ-vs-FEC, tek-hücre-genelleme,
RTT250→production-ready YASAK).

## 1. RUN BUDGET (§5.1-A; HARD MAX — kapasite-türevi)

```text
makine: tek-host (omarchy; rootless two-netns unshare -r), ÇALIŞMALAR SERİDİR (latency/RSS-duyarlı → §5.1-C).
ort. koşu-süresi tahmini: ~8–25 s/run (RTT/blackout profiline göre); yüksek-RTT+blackout üst-uçta.

HARD CEILING (result-bearing runs):
  FS-1 screening   ≤ 2400 run
  FS-2 surface     ≤ 1500 run
  FS-3 profile     ≤  900 run
  confirmatory     ≤ 1200 run   (C-P1..C-P5 + FS-7 held-out)
  long-duration    ≤  200 run
  GRAND TOTAL      ≤ 6000 run   (mutlak-tavan; aşılırsa STOP + BUDGET-LIMITED + §5.1-son principled-reduction)

BU-OTURUM GERÇEKÇİ-HEDEF (session-bounded; tavanın ALTINDA):
  öncelik-sırası: FS-CORRIGENDUM(✓) → FS-1-screening(bounded) → C-P1..C-P5 confirmatory → oracle-gate.
  Oturum-kapasitesi tam-yüzeyi kaldırmazsa: kapsanan-hücreler AÇIK-listelenir, kalan = BUDGET-LIMITED
  (sahte-interpolasyon YOK; §13.1 INSTRUMENT-LIMITED semantiği).
```

## 2. CELL-GENERATION RULE (§5.1-B; per-phase)

```text
FS-1  : STRATIFIED representative-set (Cartesian-DEĞİL). RTT×payload×deficit×impairment-family fixed-küçük-set.
        arm = factorial (her-hücre tüm-sağkalan-arm). seed = ≥5.
FS-2  : SPACE-FILLING screening → transition-region tespit → FOCUSED-factorial confirm. Cartesian-YASAK.
        250–1000ms RTT + crossover-bandına yüksek-yoğunluk.
FS-3  : bounded K∈{2,4,8,16,32} × R∈{0,1,2,4,8} (geçersiz-kombin atlanır) sağkalan-aile-üzerinde.
FS-4  : oracle = ölçülen-arm-üzerinden (hindsight); yeni-koşu-yok (missing-arm→ORACLE-ABSTAIN).
confirmatory : ön-ilan-edilmiş sabit-hücre (aşağıda §4); outcome-driven-seçim YASAK.
```
Parallelism (§5.1-C): tüm result-bearing runs **SERİ** (concurrency=1) — latency/RSS/completion contention-
duyarlı. Yalnız derleme paralel. Aynı-anda-ikinci-netns-run YASAK (netem/CPU contention sonucu-bozar).

## 3. ARM SET (§10)

```text
A0        : ARQ + 0 proaktif-parity                                    [birincil-baseline]
A1-EARLY  : aynı-wire-byte source-duplication, r∈{1,2,4}, ERKEN per-source timing (blok-kapanışla-gecikmez)
A1-LATE   : (opsiyonel) blok-kapanış-benzeri-timing duplication → emisyon-timing confound'unu-açığa-çıkarır
F0        : mevcut block-FEC (parity@blok-kapanış)                     [tarihsel-kontrol]
F1        : erken logical-close block-FEC (parity-eligibility fiziksel-tam-dispatch-beklemez)
F2        : incremental/interleaved parity   [DUR-B eğer sealed DATA-before-parity ordering ile çelişirse]
F3        : time/deadline-tabanlı kısmi-blok (sparse/low-rate blok-fill-gecikmesini-sınırlar)
F4        : adaptive-K (K, RTT/bw/deadline-rejimine bağlı mı?)
F5        : güçlü sliding-window/streaming-FEC REFERANS — **OUT-OF-STACK harness (default)**; wire-entegre-DEĞİL
            etiketi zorunlu; in-stack-entegrasyon wire/protokol-genişletme-gerektirirse → DUR-B (otomatik-YOK)
```
**Budget-rule (§10):** redundancy-sınıfı configured-R'den ön-ilan-edilir (post-hoc-DEĞİL). Her eşleşen-hücre:
`configured-redundancy-class eşit ∧ realized-sealed-wire-redundancy-bytes ön-ilan-tolerans-içinde`; aksi →
`BUDGET-MISMATCH` (coding-vs-duplication nedensel-kanıtı-OLARAK-kullanılmaz). requested/planned/emitted-bytes
her-iki-arm-için kaydedilir.

**Family safety (§11):** her in-stack F-aile named-experimental-flag arkası, default-OFF, flag-OFF byte-özdeş,
A1/A2-pin-yeşil, kaynak-bound-korunur. Her-aile-öncesi seal-collision-map (wire?/ordering?/scheduler-precedence?/
resource/security/DUR-B?). **F2 özellikle:** interleaving DATA-before-parity-ordering ile çelişirse **STOP@DUR-B**.

## 4. CONFIRMATORY PRIMARY CELLS (§6.1; ön-ilan; screening-ÖNCESİ dondu)

Her hipotez: primary-metrik · effect-yönü · pratik-eşik · confidence · seed/trace · pass/fail.

```text
C-P1  RTT250 REPLICATION (düzeltilmiş-enstrüman)
  cell: rtt250, payload 300B, deficit∈{1,2,4}, IID-loss matched-deficit, util 0.80C
  arms: A0, A1-EARLY(r=d), F0(r=d)
  primary: p50 completion-latency (rel_ms)
  yön: F0(r≥d) < A0  (proaktif-parity daha-hızlı)
  eşik: Δp50 ≥ max(20ms, 10%×A0)
  confidence: 10 paired-trace; %95 paired-bootstrap CI Δ<0'ı-dışlamalı
  pass: eşik+CI sağlanır ∧ FEC-path-used≥ çoğunluk. fail: aksi.
C-P2  HIGH-RTT EMISSION HYPOTHESIS (yeni-aileler head-start'ı-geri-getirir-mi?)
  cell: rtt∈{1000,2500}, payload 300B, deficit∈{1,2}, IID+short-burst, util 0.80C
  arms: A0, F0, F1, F3   (F0=tarihsel-nötr; F1/F3=erken/partial-emisyon)
  primary: p50 completion + deadline-hit(ön-ilan-cell-deadline)
  yön: F1/F3(r≥d) < F0(r≥d) ∧ F1/F3 < A0
  eşik: Δp50 ≥ max(20ms,10%)
  confidence: 10 paired; near-equiv→30
  pass: ≥1 emisyon-aile yüksek-RTT'de A0'a-karşı-eşik-geçer. fail: hiçbiri (emisyon-modeli-sınırı-doğrulanır).
C-P3  REVERSE-BLACKOUT NICHE (forward-açık; reverse-feedback-blackout/loss)
  cell: rtt 250, forward IID 10%, reverse blackout∈{5s,30s} + reverse-loss 90%, util 0.50C
  arms: A0, A1-EARLY(r=2), F0(r=2), F1(r=2)
  primary: completion-probability-within-T ∧ p95 completion
  yön: proaktif-redundancy(F/A1) > A0 (ACK-starved reverse'te forward-recovery-avantajı)
  eşik: ΔP(complete)≥0.10 VEYA Δp95≥max(50ms,15%)
  confidence: 10 paired-trace (reverse-schedule trace-driven)
  pass: F veya A1 A0'ı-eşik-geçer. fail: eşdeğer.
C-P4  CODING vs SAME-BYTE DUPLICATION (matched-budget nedensel)
  cell: rtt∈{250,1000}, payload 300B, deficit∈{1,2,4}, IID 20% + short-burst
  arms: A1-EARLY(r), F0(r), F1(r)  — configured-redundancy-class EŞİT (§10 budget-rule; BUDGET-MISMATCH-guard)
  primary: p50 completion ∧ P(complete)
  yön: coding(F) < duplication(A1)  (coding-gain > "erken-ekstra-byte"-gain mı?)
  eşik: Δp50 ≥ max(20ms,10%) matched-budget'ta
  confidence: 10 paired; near-equiv→30
  pass: F, A1'i-eşik-geçer→coding-gain-var. fail/equiv: coding≈duplication→RESEARCH-ONLY-sinyali.
C-P5  DEADLINE-CRITICAL (+ FS-CORRIGENDUM r=0-baseline-eksiğini-kapatır)
  cell: rtt250, payload 300B, deficit∈{1,2,4}, deadline ön-ilan (per-deficit; 305ms d1-referans)
  arms: A0(r=0 GERÇEK-baseline; d2/d4-dahil — corrigendum-eksiği), A1-EARLY(r=d), F0(r=d), F1(r=d)
  primary: deadline-hit-rate (hits/N; §9-exact, "~%" YASAK)
  yön: F0/F1(r≥d) deadline-hit > A0
  eşik: Δhit-rate ≥ 0.20 (mutlak) ∧ N=30
  pass: r≥d-arm A0'dan-≥20pp-yüksek-hit. fail: eşdeğer/ters (d4-corrigendum-benzeri).
```
Diğer-tüm-hücreler **EXPLORATORY** (§6.2; tek-başına headline-DESTEKLEMEZ; post-hoc-promotion-YASAK).

## 5. STATISTICS / SEEDS / PAIRING (§7)

```text
screening seed   : ≥5 bağımsız-trace/hücre (varsayılan ≥10 YASAK)
confirmatory     : ≥10 paired-trace (gerçek-pairing-varsa); pratik-eşdeğerlik-sınırına-yakın→30 (expand-rule dondu)
PAIRING tanımı   : bir-koşu PAIRED ⟺ yarışan-arm'lar AYNI deterministik-impairment-trace'i tüketir.
  yöntem: TRACE-DRIVEN ingress replay-injector (loss-pozisyonları · burst-schedule · reorder-events ·
          reverse-blackout/loss-schedule). Aynı-PRNG-seed YETMEZ (netem-timing/order realized-trace-değiştirir).
  netem-random-hücre (exact-replay-imkansız) → UNPAIRED-etiketi + bağımsız-seed + unpaired-istatistik;
  paired-power İDDİA-EDİLMEZ.
CI               : paired→paired-difference-bootstrap; unpaired→Mann-Whitney/unpaired-bootstrap. %95.
```

## 6. MEASUREMENT INTEGRITY (§8; outcome-ÖNCESİ dondu)

```text
metric defs      : completion=rel_ms(complete_ms−first_ms; one-way-cancel) · P(complete≤T) · deadline-hit=comp≤DL ·
                   sealed_wire_bytes(exact) · CPU(proc-time) · RSS(/proc/self/statm) · source_starvation
pratik-eşdeğerlik: ±max(20ms, 10%×baseline)
anlamlı-latency  : Δp50 > pratik-eşdeğerlik-bandı
deadline-success : per-confirmatory-cell ön-ilan (C-P5)
max wire-cost    : regresyon-CAP-yok ama exact-accounting-zorunlu (requested/planned/emitted); rapor-edilir
max CPU/RSS reg. : RSS bounded (A2-ruhu; ≤16MiB-A2-cold + FEC-state-bounded); CPU-rapor
source-starvation: 0 kalıcı-starvation (aksi early-eliminate §12)
censoring/horizon: per-cell observation-horizon ön-ilan; timeout→CENSORED (latency-değeri-DEĞİL; §14)
```

### 6.1 PER-RUN INSTRUMENT CHAIN (§8.0; veri-kabul-ÖN-KOŞULU)
```text
1. faithful-harness-pin : two-netns; per-cell realized-RTT/loss ön-ilan-tolerans-içinde (configured+realized kayıt)
2. overlap-gate         : receiver_events > 0
3. binary-provenance    : per-arm binary-kimlik + byte-size build-ledger'a-karşı doğrulanır (stale/wrong-feature=geçersiz)
4. per-arm ARG-PARITY   : bir-hücrenin-tüm-arm'ları AYNI-arg (yalnız ön-ilan-müdahale-farkı); otomatik-arg-diff eklenir
İHLAL → INVALID — NO EVIDENCE (protokol-FAIL-DEĞİL; enstrüman-onar + re-run; sessizce-dahil-etme YASAK).
```

### 6.2 MANDATORY TIMING CHAIN (§8.1; tek monotonic-clock)
```text
t_source_loss · t_parity_eligible · t_parity_enqueued · t_parity_wire · t_parity_arrival ·
t_arq_eligible · t_arq_wire · t_arq_arrival · t_block_complete
→ parity-delay-class: NOT_ELIGIBLE | ELIGIBLE_NOT_ENQUEUED | ENQUEUED_NOT_WIRE | WIRE_BUT_LATE | USABLE_BEFORE_ARQ
```
### 6.3 MANDATORY METADATA (§8.2): block_id·profile_id·K_actual·R_{requested,planned,enqueued,wire}·payload·
sealed_wire_bytes·{configured,actual}_{bandwidth,RTT}·{configured,realized}_{forward,reverse}_loss·burst-stats·
utilization/headroom·completion_cause·censored.

## 7. CENSORING / EXTREME (§14)
```text
extreme-cell (25bit/s, 90%-loss, 1.10–1.25C overload): primary-endpoint = P(complete within T) (uncensored-latency-DEĞİL)
overload: typed-reject/bounded-admission-fail = GÖZLEM (otomatik-test-FAIL-DEĞİL)
timeout/censored ASLA sayısal-latency-gözlemi-sayılmaz.
```

## 8. VERDICT PATH (§16/§17/§23)
Oracle-gate (FS-4): meaningful-FEC-upside-yoksa → controller-kalibrasyona-GEÇME; yalnız K4-minimum-bar/final-
karakterizasyon. K4-RETREAT yalnız §17-baraj (oracle + A1-duplication + confirmatory-niche + ≥1-alt-emisyon-aile +
observability-dışlaması) sağlanınca; aksi INCONCLUSIVE/RESEARCH-ONLY-PENDING. Nihai-hüküm FS-DUR-SON→Abkarada-onayı.

## PROVENANCE
```text
sha256: cf005739d4fca2d42faf75e5c22c761f379eb6a438f811843d34cd9b3fbee5dc
tarih : 2026-07-31T11:41:36Z
coverage-plan: FS-COVERAGE-PLAN.md (ayrı-hash)
```
