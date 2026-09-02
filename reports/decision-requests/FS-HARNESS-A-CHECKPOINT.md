# FS-HARNESS-A — COMPLETION CHECKPOINT (DR-FEC-FS-1)

> **STATÜ: FS-HARNESS-A TAMAMLANDI (review-blocking-fix uygulandı).** Result-bearing-YOK. **FS-1 KOŞULMADI.**
> FS-HARNESS-B'ye GEÇİLMEDİ. Bu, ölçüm-altyapısının (harness) bittiğine dair bir **checkpoint**tir — FEC-hükmü
> DEĞİL. FS-1 yalnız FS-HARNESS-B + tek-konsolide FS-PREREG-v1.2-FINAL freeze + tüm-pin-yeşil olunca başlar.

> **REVIEW-RESPONSE (PASS-WITH-ONE-REQUIRED-FIX → RESOLVED):**
> - **A1_WIRE_ACCOUNTING_REAL_DATAGRAM (blocking):** ÇÖZÜLDÜ. Eski A1-dup raw-payload (128B/framing=0) vs
>   F0-parity uydurma-108 domain-hatası GİDERİLDİ. Tek GROUNDED sealed-datagram modeli (fs_sealed_len(E)=E+37;
>   otorite protector.rs "101B@E=64"); source==repair. A1 r=2 = F0 r=2 = **202B** (matched). Pin: bir dup için
>   payload+framing == realized == channel-serialization HEPSİ AGREE (fs_harness.rs test; §4). Reviewer'ın
>   accepted-108'i aslında +7B uydurmaydı → 101'e grounded-düzeltildi (transparanlık: F0 parity_frame 108→101).
> - **TAIL-LOSS "önce sınıflandır A-vs-B":** SINIFLANDIRILDI = **B_REAL_TAIL_ERASURE** (harness-defekti-DEĞİL;
>   runner PTO'yu sadık-servis-eder; Sender yapısal seq<largest_acked önkoşulu; horizon-uzatma ÇÖZMEZ). §6.
>   Resolution (Sender RTO/TLP/§5.1) FS-HARNESS-B [B2]'ye — reviewer'ın B-dalı talimatına uygun (preserve+metadata).

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
fs_harness.rs (A0/A1)      : 18 lib-test (emitter · target-set · state-neutral · egress-budget · emission-order ·
                             generic-ledger · bottleneck · run_arm ledger+instrument · a1-hit · a1-miss · trace-hash ·
                             **A1_WIRE_ACCOUNTING_REAL_DATAGRAM** [review-blocking-pin])
fs_f0_closed_loop.rs (F0)  : 7 (loopback · D1/R1-real-decode+t_repair_usable≥parity · D2/R2 · D2/R1-hybrid-ARQ ·
                             purity-fixed-R · RECOVERED-scope+estimator-purity · wire-accounting[grounded 101B])
BINARY smoke               : 6 (clean-loopback×3 · f0-real-reconstruct[FEC-completes+spurious-ARQ] · a1-hit ·
                             a1-miss≡a0-honesty) + tail-loss INFO
```

## 4. EXACT WIRE-ACCOUNTING — DÜZELTİLDİ (review-blocking-fix; GROUNDED tek-domain; C-P4 madde-8)
**REVIEW-FIX (A1_WIRE_ACCOUNTING_REAL_DATAGRAM):** eski A1-dup raw-payload (framing=0) · F0-parity uydurma-108
sayıyordu → FARKLI-domain (sahte C-P4). DÜZELTME: source|retransmit|A1-dup|F0-parity HEPSİ tek GROUNDED
sealed-datagram modelinden — `fs_sealed_len(E) = E + 37` (26 EXTENDED_OUTER + 11 FEC_INNER; otorite
kama-noise-wire protector.rs "101B @ E=64"). SOURCE ve REPAIR shard BOYUTU AYNI (INV-ENGINE-SOC; transport
ikisini boyutla ayırt-edemez). Executable'dan ölçülen (k=8, E=64 → sealed=101B):

```text
A1-EARLY (r=2, seed=2026 → target={2,3}):
  duplicates_emitted = 2
  realized_sealed_wire = 202 B (= 2 × 64 payload + 2 × 37 framing)   ; source-shard sealed = 101B
  redundancy_packet_count = 2

F0 (r=2):
  parity_emitted = 2
  realized_sealed_wire = 202 B (= 2 × 64 payload + 2 × 37 framing)   ; repair-shard sealed = 101B (source==repair)
  redundancy_packet_count = 2

A1_WIRE_ACCOUNTING_REAL_DATAGRAM (reviewer-pin; bir dup için HEPSİ AGREE):
  payload(64) + framing(37) == realized(101) == BottleneckChannel-serialization(101).  ✓
C-P4 FAIRNESS SONUCU:
  configured_r EŞİT (2=2) VE realized_sealed_wire EŞİT (202 = 202) — matched-r → matched-wire (GERÇEK budget-eşit).
  → C-P4 coding-vs-dup KIYASI artık ADİL: aynı-r'de A1 ve F0 aynı-wire öder; fark yalnız coding-vs-duplication
     mekanizmasında (BUDGET_MISMATCH-guard yine bekçi; |wire_A1−wire_F*| tolerans-dışı → NO-CAUSAL-EVIDENCE).
  NOT (F0-timing bulgusu): doğru-büyük-sealed'de F0-parity post-DATA GEÇ varır → D2/R2'de ARQ 2 SPURIOUS
     retransmit sızdırır AMA FEC yine ERKEN tamamlar (t_repair_usable=136ms < t_arq_arrival=145ms → cause=fec).
     GERÇEKÇİ FEC-utility nüansı (FEC-decode ARQ-emisyonundan geç-kalırsa retransmit boşa-gider) — FS-1 ölçer.
```

## 5. GATE (ölçülen — tahmin-YOK)
```text
cargo fmt --check                                              : CLEAN
clippy -D warnings · adaptive-transport (fec-budget+cold-start): CLEAN
clippy -D warnings · adaptive-transport (FLAG-OFF)             : CLEAN
clippy -D warnings · kama-integration (experimental-fs-harness): CLEAN
clippy -D warnings · kama-integration (FLAG-OFF)               : CLEAN
WORKSPACE FLAG-OFF test                                        : 382 passed / 0 failed  (PIN=382 → UNCHANGED ✓ INV-CS-5)
adaptive-transport EXPERIMENTAL (fec-budget+cold-start)        : 232 passed / 0 failed   (+1 A1_WIRE_ACCOUNTING pin)
kama-integration experimental-fs-harness                      : 19 passed / 0 failed   (12 base + 7 F0-pin)
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

TAIL-LOSS SINIFLANDIRILDI = **B_REAL_TAIL_ERASURE** (reviewer "önce sınıflandır A-vs-B" talebi; §A1.8):
  KANIT (workflow + doğrudan-kod-okuma teyit): run_arm PTO'yu SADIK servis-eder (Timer::Pto her-1ms, koşulsuz,
  20s-horizon; fs_harness.rs:612) — runner-defekti YOK. Sender YAPISAL kurtaramaz: detect_time_losses
  "seq < largest_acked" önkoşulu (loss.rs:383; RFC9002 §6.1) → SON-unit largest_acked'i geçemez → sonsuza-dek
  loss-ilan-EDİLEMEZ; RTO/TLP timer-i YOK ({Pto,RtpropProbe}). affects_slice_a_faithfulness = FALSE.
  → **horizon-uzatma ÇÖZMEZ** (timeout-değil, yapısal-önkoşul). GERÇEK tail-erasure rejimi = doğrudan FEC-lehine
     kanıt (FEC tail'i kurtarır, ARQ yapısal-KURTARAMAZ). C-P3'te KORUNUR; maskeleme-YOK.
  GERÇEK-FIX Sender'da (harness-DEĞİL; FS-HARNESS-A-kapsamı-DIŞI): RTO/TLP VEYA §5.1 ack_confidence→RECOVER_FWD.

SLICE-B'YE DEVREDİLEN İŞ-KALEMLERİ:
  [B1] reverse/ACK-path deterministic blackout-schedule (time-domain; TraceChannel'ın reverse-yön-eşleniği)
  [B2] tail-loss: (a) tail-erasure metadata (source-position) + frozen-horizon-censor + tail-pozisyon-DIŞLAMA-YOK;
       (b) Sender-fix (RTO/TLP/§5.1-liveness) — transport-crate işi. KARAR Abkarada (C-P3 bağımlılığı).
  [B3] reverse-blackout paired-replay (A0 vs A1 vs F* aynı reverse-schedule; C-P3 iki-verdict: NICHE/FEC-SPECIFIC)
  [F1][F3] PIN-C mechanism wire-time-advance fixture (t_parity_wire(candidate) < t_parity_wire(F0))
  [F4] PIN-D mechanistic/exploratory (adaptive-K çalışıyor + farklı-K üretir; policy-iddiası-YOK)
```
**Bu satır olmadan checkpoint INCOMPLETE sayılır (madde-9). Burada PASS-DEĞİL / OPEN-ITEM olarak kayıtlıdır.**

## 7. SONRAKİ ADIM (bu tur DIŞI — Abkarada onayına bağlı)
```text
1. FS-HARNESS-B: F1/F3 (PIN-C mechanism wire-time-advance) + F4 (PIN-D) + reverse-schedule [B1..B3] + tail [B2].
2. FS-PREREG-v1.2-FINAL: v1.2-DRAFT §A1.0–A1.11 + slice-B netleştirmeleri → TEK konsolide re-hash (freeze).
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
