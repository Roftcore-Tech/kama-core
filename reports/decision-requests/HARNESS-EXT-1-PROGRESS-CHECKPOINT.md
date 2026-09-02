# HARNESS-EXT-1 — PROGRESS CHECKPOINT (FINAL BUILD, increment 1)

> **STATÜ: FOUNDATION BUILT + GATE-GREEN. FS-PREREG-v1.3 FREEZE-EDİLMEDİ. FS-1 KOŞULMADI (0/540).**
> Bu tur ratified-mapping'in UNAMBIGUOUS-çekirdeğini gerçek+test-edilmiş+gate-yeşil olarak inşa etti (F4 direct-K
> OPTION-D · canonical 48-symbol fragmenter/reassembly/global-id · §17/§18 deadline-geometry). Kalan HARNESS-EXT-1
> (6-arm multi-block execution-spine + IID-wiring + §17-source-pacing-proof + manifest + 540/540-executability +
> coverage-v1.2 + analysis-spec + F1-note + v1.3-freeze) BÜYÜK-çok-parçalı-iş — TEK-turda doğru+gate-yeşil
> tamamlanamaz. **Sahte-freeze YAPILMADI** (proje-disiplini: gate-yeşil, lab-doğrulanabilir, kanıtsız-iddia-YOK).

## 0. PROVENANCE
```text
branch/HEAD    : d1-sprint5c / (bu commit)
FS-PREREG-v1.2 : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable; DEĞİŞMEDİ)
F4 karar       : OPTION-D (Abkarada) · deadline-geometri ratified (§17-code-grounding-gate'e-bağlı)
sender.rs/loss.rs: bu-turda DEĞİŞMEDİ (git diff boş) · production-graph GENİŞLEMEDİ · defaults DEĞİŞMEDİ
```

## 1. BU TUR İNŞA EDİLEN (gate-yeşil; §1-scope-içi)
```text
[✓] F4 DIRECT-K (OPTION-D; §13/§14): CodecK{K2,K4,K8} strongly-typed; f4_select_k → CodecK (ordinal-{1,2,3}-YOK;
    K=1/K=3-YOK). region1(<50ms)→K2 · region2(50-150ms)→K4 · region3(≥150ms)→K8; K_COLD=K4. profile_id-map
    (0x01/0x02/0x03). Pin: two-state-farklı-K + realizability(∈{2,4,8}) + no-leakage. (fs_harness.rs)
[✓] CANONICAL 48-SYMBOL WORKLOAD (§2/§3/§5; harness-only): fs_workload.rs — 8×300B app-unit → 48 E=64 source-symbol
    (production SourcePrefix FORMAT; offsets{0,53,106,159,212,265}, len{53×5,35}, canonical_zero_pad, SOURCE_FLAG_FINAL,
    8-distinct-message_id) + reassemble() + global_source_fragment_id 0..47 (unit-major; RULE-4 partition-independent) +
    sequence_hash. 6-PIN (fs_workload_pins.rs): offsets/len/pad · 8-msg-id · serialize/parse-eq · full-2400B-reassembly-
    byte-eq · global-id-stable-dense · geometry-const(53/6/48/2400).
[✓] §17/§18 DEADLINE-GEOMETRY primitives (arm-independent; ratified-formula): fs_effective_source_bps=min(pacer,cap) ·
    fs_t_serialize_source_ns · fs_deadline_ms. Pin: ortak 48×101=4848B @ effective-0.80C(=800_000 bits/s) → 48.48ms
    source-serialize → deadline{RTT50→199, RTT250→599, RTT1000→2099, RTT2500→5099}ms (§18 ratified; DONDURULMADI).
```

## 2. GATE (ölçülen)
```text
fmt-check CLEAN · clippy -D warnings (AT exp+flag-off · KI fs-harness+flag-off) CLEAN
FLAG-OFF workspace : 382 / 0  (PIN=382 UNCHANGED; INV-CS-5)
adaptive-transport EXP : 241 / 0  (+F4-realizability +deadline-geometry pin)
kama-integration fs-harness : 28 / 0  (+6 canonical-workload pin)
kama-integration cold-start A2 : 44 / 0 (intact)
sender.rs / loss.rs : ZERO-DIFF (§31 hard-invariant korundu)
```

## 3. §17 EFFECTIVE-RATE GROUNDING — DURUM
```text
İNCELENDİ: FS-harness BottleneckChannel source'u TEK link_bps'te serialize-eder (serialize_ns=sealed×8×1e9/link_bps);
  RunCfg'de util/0.80C/source-pacer YOK; historical-script'lerde de util=0.80C-mekanizması YOK (grep-boş).
  → mevcut-kod source'u C'de-serialize-eder, 0.80C'de-DEĞİL.
DUR-B DEĞİL: directive §17-formülü (effective=min(source_pacer, capacity)) inşa-edilecek-mekanizmayı TANIMLAR
  (source-pacer@0.80C, capacity=C → effective=0.80C). Bu yetkili-harness-işi.
BU-TUR: formül fs_effective_source_bps/fs_deadline_ms olarak grounded + pinned (§18 değerleri üretiliyor).
KALAN (§17-tam-proof): 6-arm-spine'ın source-EMISSION'ını gerçekten-0.80C'de-pace-ettiği + dry-run'da ölçülen-egress
  ≈0.80C pini — spine-source-emission gerektirir (aşağıda). v1.3-deadline-freeze bu-pin-yeşil-olunca (§17).
```

## 4. KALAN HARNESS-EXT-1 İŞİ (bu-turda İNŞA-EDİLMEDİ; büyük)
```text
[ ] COMMON 6-ARM MULTI-BLOCK EXECUTION SPINE (§7): 48-canonical-symbol → real-Sender + BottleneckChannel(source-pace
    @0.80C) → ExactDeficit/IID global_id-keyed → ACK/ARQ → per-block real-RS-encode/decode (K∈{4,8}+F4-K∈{2,4,8}) →
    reassembly → completion(TÜM-48) → canonical-RunResult. (run_arm+run_f0'ın multi-block-birleşik-refaktörü; intricate.)
[ ] 6-ARM: A0(§8)/A1(§9)/F0(§10)/F1(§11)/F3(§12)/F4(§13) closed-loop; §23 sanity-matrix.
[ ] IID-20% PLUMBING (§15) tüm-arm'lara (NaturalIid mevcut; global-id-keyed wiring).
[ ] §17 source-emission-pacing + effective-egress≈0.80C proof-pin.
[ ] FS-1-MANIFEST-v1.3.jsonl (§24; 540-row; deterministik; hash) + 540/540 EXECUTABILITY dry-run (§25;
    FS-1-MANIFEST-EXECUTABILITY.json/.md).
[ ] FS-COVERAGE-PLAN-v1.2.md (§26) · FS-ANALYSIS-SPEC-v1.md (§27; outcome-blind + validation-code + synthetic-test) ·
    F1-structural-null-note (§28).
[ ] FS-PREREG-v1.3 (§29; freeze + reproducible-SHA; v1.2→SUPERSEDED-BY-v1.3) — YALNIZ yukarıdakiler-yeşil-olunca.
[ ] Full 44-item HARNESS-EXT-1-CHECKPOINT (§32).
```

## 5. NEDEN TEK-TURDA FREEZE-EDİLMEDİ (dürüstlük)
```text
6-arm multi-block FEC+ARQ+reassembly+completion spine correctness-kritik + intricate (48-symbol / 6-24-blok /
  global-id-deficit / full-workload-completion). Onu + manifest + 540/540-executability + analysis-pipeline +
  4-frozen-doc + v1.3-freeze TEK-turda doğru+gate-yeşil üretmek gerçekçi-değil. Rushed-spine broken-artifact-riski
  (§31-gate-fail) → sahte/broken-freeze proje-disiplinini-ihlal-eder. Bu yüzden: UNAMBIGUOUS-ratified-çekirdek
  gate-yeşil inşa-edildi (F4+fragmenter+deadline); spine/manifest/freeze SONRAKİ-increment(ler)de bütünsel+doğru.
STOP-condition'ları (§33) HİÇBİRİ tetiklenmedi (F4∈{2,4,8}✓ · Sender/loss-değişmez✓ · 48-geometri-korunur✓ ·
  ExactDeficit-global-id✓ · IID-redefine-YOK · util-buildable✓ · deadline-arm-independent✓).
```

## 6. STOP (bu-increment)
```text
FS-1 KOŞULMADI (0/540) · v1.3 FREEZE-YOK · v1.2 immutable · sender.rs/loss.rs değişmez · production-default değişmez.
Sonraki-increment: COMMON 6-arm spine (F4-direct-K + fragmenter + deadline-geometri ZEMİNİ üstünde) → IID → manifest
→ 540/540-executability → coverage-v1.2 → analysis-spec → F1-note → FS-PREREG-v1.3-freeze → full-checkpoint → STOP.
```
