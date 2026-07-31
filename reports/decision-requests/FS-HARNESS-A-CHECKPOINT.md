# FS-HARNESS-A — COMPLETION CHECKPOINT (DR-FEC-FS-1)

> **STATÜ: FS-HARNESS-A TAMAMLANDI.** Result-bearing-YOK. **FS-1 KOŞULMADI.** FS-HARNESS-B'ye GEÇİLMEDİ.
> Bu, ölçüm-altyapısının (harness) bittiğine dair bir **checkpoint**tir — FEC-hükmü DEĞİL. FS-1 yalnız
> FS-HARNESS-B + tek-konsolide FS-PREREG-v1.2-FINAL freeze + tüm-pin-yeşil olunca başlar (Abkarada onayı).

## 0. Kapsam sınırı (bağlayıcı)
```text
YAPILDI     : FS-HARNESS-A (trace-replay + A1-EARLY + F0-integration + instrumentation/provenance + CLI/smoke).
YAPILMADI   : FS-HARNESS-B (F1/F3/F4 + family-mechanism-pins + reverse-blackout-schedule) · FS-1 result-runs ·
              FS-PREREG-v1.2-FINAL freeze · herhangi bir FEC product-utility HÜKMÜ.
```

## 1. GO 11-madde — tamamlanma tablosu
| # | Madde | Durum | Kanıt |
|---|---|---|---|
| 1 | Generic+legacy 9-ts ledger + **t_repair_usable** (USABLE_BEFORE_ARQ bundan) | ✅ | `BlockTiming` (fs_harness.rs:188); `parity_delay_class()` t_repair_usable-tabanlı |
| 2 | A1 causal ledger `a1_dup_hits`/`a1_dup_misses` | ✅ | `RunResult` alanları; smoke HIT=1/MISS=1 ayrışması |
| 3 | Path-timing ledger-only (6 alan) | ✅ | `PathTimingLedger`; snapshot-contamination §B0.6 korunur |
| 4 | F0_RECOVERED_SCOPE + ESTIMATOR-PURITY (6. pin) | ✅ | `f0_recovered_scope_estimator_purity`; before/after RECOVERED srtt/rtprop özdeş |
| 5 | Per-run instrument chain (faithful/overlap/provenance/arg-parity/trace-hash/intervention/snapshot) | ✅ | `PerRunInstrument` + `admitted()`; A0/A1/F0 dolduruyor |
| 6 | fs_runner CLI (`--arm a0\|a1\|f0`, machine-readable) | ✅ | `examples/fs_runner.rs`; KEY=VALUE çıktı |
| 7 | Binary loopback/closed-loop smoke (executable-path) | ✅ | `scripts/netem/fs_runner_smoke.sh` → 6/6 GREEN |
| 8 | Exact A1/F0 wire-accounting örnekleri (aynı-domain; realized-sealed-wire) | ✅ | §4 (aşağıda) |
| 9 | **REVERSE-SCHEDULE STATUS — zorunlu checkpoint-satırı** | ⚠️ **OPEN ITEM** | §6 (aşağıda) |
| 10 | FS-PREREG-v1.2 amendment DRAFT (DONMA-YOK) | ✅ | `FS-PREREG-v1.2-DRAFT.md` (hash-yok) |
| 11 | Tam FS-HARNESS-A checkpoint | ✅ | bu belge |

## 2. PIN durumu
```text
PIN-A (A1 direct emission-side neutrality)  : ✅ yapısal (A1EarlyEmitter Sender-ref-tutmaz) + smoke-doğrulı
PIN-B (pairing trace semantics; attempt-aware): ✅ fs_trace.rs 6-test; ExactDeficit yalnız InitialSource@0 siler
PIN-E (A1 egress-fairness; realized-wire)    : ✅ BottleneckChannel her-paket-serialize (dropped-dahil); smoke
PIN-C (F1/F3 mechanism wire-time-advance)    : ⏳ FS-HARNESS-B (slice-B; bu turda DEĞİL)
PIN-D (F4 mechanistic/exploratory)           : ⏳ FS-HARNESS-B (slice-B; bu turda DEĞİL)
```

## 3. Sanity-pin envanteri (tümü yeşil; result-bearing-DEĞİL)
```text
fs_trace.rs                : 6 (attempt-aware exact-deficit · arm-invariant · redundancy-policy · iid · burst · determinism)
fs_harness.rs (A0/A1)      : 17 lib-test (emitter · target-set · state-neutral · egress-budget · emission-order ·
                             generic-ledger · bottleneck · run_arm ledger+instrument · a1-hit · a1-miss · trace-hash)
fs_f0_closed_loop.rs (F0)  : 7 (loopback · D1/R1-real-decode+t_repair_usable≥parity · D2/R2 · D2/R1-hybrid-ARQ ·
                             purity-fixed-R · RECOVERED-scope+estimator-purity · wire-accounting)
BINARY smoke               : 6 (clean-loopback×3 · f0-real-reconstruct · a1-hit · a1-miss≡a0-honesty) + tail-loss INFO
```

## 4. EXACT WIRE-ACCOUNTING (A1/F0 AYNI physical-domain; C-P4 madde-8)
Ortak muhasebe: her redundancy-paketi `BottleneckChannel.offer()` ile serialize (dropped-dahil), maliyet
`RedundancyCost.realized_sealed_wire_bytes` (payload + framing). Executable'dan ölçülen (k=8, unit=64B):

```text
A1-EARLY (r=2, seed=2026 → target={2,3}):
  duplicates_emitted = 2       (blok-total: 2 seçili-source × 1 kopya — K×R DEĞİL)
  realized_sealed_wire = 128 B (= 2 × 64 payload + 2 × 0 framing)   ; source-kopyası → framing=0
  redundancy_packet_count = 2

F0 (r=2, parity_frame=108B):
  parity_emitted = 2
  realized_sealed_wire = 216 B (= 2 × 64 payload + 2 × 44 framing)  ; parity-header → framing=88
  redundancy_packet_count = 2

C-P4 FAIRNESS SONUCU (madde-8 BUDGET_MISMATCH-guard'ın tam-nedeni):
  configured_r EŞİT (2=2) AMA realized_sealed_wire EŞİT-DEĞİL (128 vs 216 B; Δ=88 B framing).
  → "coding advantage" iddiası configured_r-eşitliğiyle YAPILAMAZ; realized-wire-eşitliği ŞART.
  → FS-1'de C-P4 karşılaştırması ya realized-wire-matched-A1 kurar (r≈3–4 dup ≈ 216 B) ya da
     farkı DUPLICATION_ADVANTAGE/CODING_ADVANTAGE sınıfına wire-dahil yazar. Otomatik guard:
     |wire_A1 − wire_F*| tolerans-aşarsa → BUDGET_MISMATCH-NO-CAUSAL-EVIDENCE.
```

## 5. GATE (ölçülen — tahmin-YOK)
```text
cargo fmt --check                                              : CLEAN
clippy -D warnings · adaptive-transport (fec-budget+cold-start): CLEAN
clippy -D warnings · adaptive-transport (FLAG-OFF)             : CLEAN
clippy -D warnings · kama-integration (experimental-fs-harness): CLEAN
clippy -D warnings · kama-integration (FLAG-OFF)               : CLEAN
WORKSPACE FLAG-OFF test                                        : 382 passed / 0 failed  (PIN=382 → UNCHANGED ✓ INV-CS-5)
adaptive-transport EXPERIMENTAL (fec-budget+cold-start)        : 231 passed / 0 failed
kama-integration experimental-fs-harness                      : 19 passed / 0 failed   (12 base + 7 yeni F0-pin)
kama-integration experimental-cold-start (A2 pins)            : 44 passed / 0 failed   (A2 INTACT ✓)
combined features (fs-harness + cold-start) compile            : OK
a2_rx_netem_runner example build                              : OK
BINARY smoke (scripts/netem/fs_runner_smoke.sh)               : 6/6 GREEN
```
Production lib-grafiği GENİŞLEMEDİ (kama-fec-codec zaten-prod-dep; adaptive-transport dev-dep; fs-harness test-only).

## 6. ⚠️ REVERSE-SCHEDULE STATUS (ZORUNLU CHECKPOINT-SATIRI — madde-9)
```text
REVERSE-SCHEDULE STATUS = OPEN ITEM → FS-HARNESS-B'YE TAŞINDI (C-P3 bağımlılığı).

GEREKÇE: FS-HARNESS-A yalnız FORWARD-yön deterministik source-loss replay'ini kurar (ExactDeficit
  InitialSource-erasure + NaturalIid/NaturalBurst). C-P3 (reverse-blackout) senaryosunun gerektirdiği
  REVERSE/RETURN-path (ACK-yönü) deterministik-blackout-schedule'ı slice-A'da UYGULANMADI. ACK-yönü
  blackout'u ARQ'nun feedback-bağımlılığını sıkıştırır — proaktif-FEC/dup'ın niş-faydasının en-olası
  görüneceği yer tam da burasıdır; dolayısıyla C-P3 verdict'i bu schedule OLMADAN üretilemez.

EK-BAĞLI OPEN ITEM (FS-HARNESS-A'da KEŞFEDİLDİ — FS-PREREG-v1.2-DRAFT §A1.8):
  TAIL-LOSS CONFOUND — deficit son-source-unit'e düşünce ARQ (A0/A1-miss) fast-retransmit-tetikleyemez
  (ardıl-data-yok; RTO horizon'u-aşabilir) → blok tamamlanmaz (delivered=k−1, completion=None; smoke drop=7).
  Bu, tail-pozisyonda A0-vs-F0'ı ÇARPITIR (F0-parity tail'i deterministik-kurtarır). FS-HARNESS-B'de
  disambigüe: (1) horizon'u RTO-garanti-edecek-kadar-uzat → tail-deficit-adil, VEYA (2) tail-loss'u
  AÇIK ölçülen-FEC-boyutu-olarak-raporla. KARAR Abkarada'ya.

SLICE-B'YE DEVREDİLEN REVERSE-SCHEDULE İŞ-KALEMLERİ:
  [B1] reverse/ACK-path deterministic blackout-schedule (time-domain; TraceChannel'ın reverse-yön-eşleniği)
  [B2] tail-loss confound resolution (horizon-uzat VEYA tail-boyut-raporla; C-P3 öncesi)
  [B3] reverse-blackout paired-replay (A0 vs A1 vs F* aynı reverse-schedule; C-P3 iki-verdict: NICHE/FEC-SPECIFIC)
```
**Bu satır olmadan checkpoint INCOMPLETE sayılır (madde-9). Burada PASS-DEĞİL / OPEN-ITEM olarak kayıtlıdır.**

## 7. SONRAKİ ADIM (bu tur DIŞI — Abkarada onayına bağlı)
```text
1. FS-HARNESS-B: F1/F3 (PIN-C mechanism wire-time-advance) + F4 (PIN-D mechanistic) + reverse-schedule [B1..B3].
2. FS-PREREG-v1.2-FINAL: v1.2-DRAFT §A1.0–A1.10 + slice-B netleştirmeleri → TEK konsolide re-hash (freeze).
3. ANCAK O ZAMAN FS-1 result-bearing runs (540; 6-arm × 18-cell × 5) + C-P1..C-P5 + FS-4 ORACLE-GATE (DUR-A).
```

## PROVENANCE
```text
harness-slice   : FS-HARNESS-A (A+B'nin A'sı; result-bearing-YOK)
kaynak-kod      : adaptive-transport/src/{fs_trace.rs,fs_harness.rs} · kama-integration/tests/common/fs_f0.rs ·
                  kama-integration/tests/fs_f0_closed_loop.rs · kama-integration/examples/fs_runner.rs ·
                  scripts/netem/fs_runner_smoke.sh
prereg-draft    : FS-PREREG-v1.2-DRAFT.md (HASH-YOK; FS-HARNESS-B-sonrası FINAL-freeze)
base-prereg     : FS-PREREG-v1.1.md (sha 60353e99…) · coverage FS-COVERAGE-PLAN-v1.1.md · seal FS-SEAL-COLLISION-MAPS.md
reverse-schedule: OPEN ITEM (§6) → FS-HARNESS-B
```
