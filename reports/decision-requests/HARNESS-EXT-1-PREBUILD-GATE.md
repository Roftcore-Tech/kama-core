# HARNESS-EXT-1 PRE-BUILD REALIZABILITY GATE (DR-FEC-WORKLOAD-1 ratified)

> **STATÜ: STOPPED BEFORE 6-ARM BUILD.** Mapping RATIFIED-WITH-AMENDMENTS kabul edildi; HARNESS-EXT-1 devam
> yetkili. ANCAK inşadan-ÖNCE 6-arm'ın ratified-48-symbol-geometride REALIZABILITY pre-flight'ı yapıldı
> (RULE-1 executability + RULE-2 grounding-chain). **F4 gerçek-codec-closed-loop olarak REALIZE-EDİLEMEZ**
> (frozen f4_select_k {1,2,3} vs codec k_shape {2,4,8,16}). Bu 540/540-executability'yi bloke-eder →
> §16-gate FAIL → freeze-YOK. **Kod-YAZILMADI · manifest-YOK · freeze-YOK · execution-YOK. v1.2 immutable.**
> Diğer-5-arm + tüm-mapping-check'leri REALIZABLE (aşağıda) — yalnız F4-semantics Abkarada-ratifikasyonu gerekir.

## 0. PROVENANCE
```text
branch/HEAD    : d1-sprint5c / 3351e21 (kod-değişikliği-YOK; pre-build audit)
ratified-map   : FS-1-PAYLOAD-LAYER-MAPPING-NOTE.md (3351e21) + ADDENDUM RATIFICATION (bu-tur binding)
FS-PREREG-v1.2 : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable)
```

## 1. ALL-ARM REALIZABILITY AUDIT (ratified 48-symbol geometri; RULE-1/RULE-2)
```text
canonical: 8×300B → 48 source-symbol (E=64, sealed=101B), global_source_fragment_id 0..47 (§5).
| arm | partition | codec-profil | realizable? |
| A0  | 48 opaque source (ARQ-only)                  | (codec-yok)              | ✓ |
| A1  | blind block-total dup of selected sources     | (codec-yok; source-copy) | ✓ |
| F0  | 48 → 6 × K=8 blok, R/blok                      | 0x03 (K8/R4/E64)         | ✓ |
| F1  | = F0 (early-close; STRUCTURALLY_NON_EFFECTIVE)| 0x03                     | ✓ |
| F3  | 48 → 12 × K'=4 blok, R/blok                    | 0x02 (K4/R4/E64)         | ✓ |
| F4  | 48 → K = f4_select_k(path) ∈ {1,2,3}          | **K=1 YOK · K=3 YOK**    | ✗ BLOKE |
5/6 arm REALIZABLE. Yalnız F4 bloke.
```

## 2. ⛔ F4-REALIZABILITY / SELECTED-K-SEMANTICS GAP (blocker)
```text
FROZEN f4_select_k (fs_harness.rs; v1.2 §B.6 sealed) → {1,2,3}: base<50ms→1 · 50-150ms→2 · ≥150ms→3 (K_COLD=2).
CODEC k_shape kataloğu (profile.rs): {2,4,8,16} YALNIZ — **K=1 ve K=3 PROFİLİ YOK**.
ADDENDUM §7: "F4 partitioned according to the frozen selected K; changing K may change BLOCK COUNT" → selected =
  BLOCK-K yorumu. Bu yorumla, 18-hücre-grid'inde:
    RTT=50ms  → base~50ms → K=2 → profile 0x01 ✓
    RTT=250ms → base~250ms → K=3 → **PROFİL-YOK → real-codec encode/decode İMKÂNSIZ**
    RTT=1000ms→ base~1000ms → K=3 → **PROFİL-YOK → İMKÂNSIZ**
  → 12/18 hücre (RTT250 + RTT1000; tüm-deficit/impairment) × 5-seed = **60 F4-run REALIZE-EDİLEMEZ**.
RULE-2 İHLALİ: F4-selected-K parametresinin codec-wire-davranışına transformation-chain'i YOK (K=3 → profil-yok →
  wire-davranışı-yok). Layer-ambiguous/ungrounded parametre freeze-edilemez. §16 540/540-executability → FAIL.
FROZEN-KURAL DEĞİŞTİRİLEMEZ (§1 absolute-freeze); E≈K=3-profili EKLENEMEZ (codec-DR; §14.D yasak). → Abkarada-karar.
```

## 3. F4 ÇÖZÜM SEÇENEKLERİ (UYGULANMADI; Abkarada ratifiye-etmeli)
```text
OPT-A "adaptive-R" (block-K=8 SABİT; selected = R∈{1,2,3}/blok): F4 = 6×K8 (F0-gibi), R path'e-göre. Codec-realizable
  (0x03, R≤4). ⚠ §7 "changing K changes block count" ile ÇELİŞİR (R block-count-değiştirmez). "adaptive-K"→"adaptive-R"
  yeniden-çerçeveleme; frozen-rule-OUTPUT'u değişmez (yorumu değişir).
OPT-B "profile-ladder" (selected {1,2,3} → block-K {2,4,8} ordinal-map): codec-realizable (hepsi geçerli-profil);
  §7 block-count-değişir korunur. ⚠ rule-output'u literal-K yerine ordinal-index olarak yeniden-yorumlar (ratify).
OPT-C "yeni codec-profil K=3 (+K=1)": AYRI codec-DR (§14.D bu-turda YASAK; ürün-gerekçesi gerekir).
OPT-D "f4_select_k'i geçerli-block-K {2,4,8}'e yeniden-türet": frozen-rule-DEĞİŞİR → re-freeze (v1.2 supersede).
Öneri-NOT: OPT-A codec-realizable + frozen-output-korur AMA §7-block-count-ifadesiyle-çelişir; OPT-B §7-korur ama
  output-yorumunu-değiştirir. İkisi-de F4'ün MECHANISTIC/EXPLORATORY-statüsünü korur. Abkarada seçmeli.
```

## 4. GEÇEN PRE-BUILD CHECK'LERİ (DUR-B DEĞİL; inşaya-hazır)
```text
[✓] GLOBAL TRACE IDENTITY (§5, RULE-4): source_id=(0, global_source_fragment_id) TÜM-arm'larda AYNI kullanılırsa
    ExactDeficit aynı-global-fragment'leri düşürür (K/blok-partition-bağımsız). TraceChannel-SEMANTİK-DEĞİŞİKLİĞİ-YOK
    (block_id zaten-hep-0; esi=global_id 0..47; frozen decide() aynı). Arm-local (codec block,esi) = layout-metadata.
    → CROSS-ARM TRACE-IDENTITY KORUNUR (DUR-B DEĞİL).
[✓] CROSS-ARM PRE-INTERVENTION EQUIVALENCE (§4): tüm-6-arm AYNI 48-symbol source-dizisi (aynı app-byte/fragment-byte/
    count/order/sealed-size/base-wire/completion); yalnız intervention farklı. Ulaşılabilir (fragmenter tüm-arm'lar-
    için tek-kural). A0/A1 native-300B-YOK; hepsi E=64-canonical-source.
[✓] DEFICIT NO-MULTIPLICATION (§6): d∈{1,2,4} = TOPLAM d global-InitialSource-fragment (blok-sayısıyla-çarpılmaz).
    ExactDeficit global_id-set'i üstünden; per-block-tekrar-YOK. Satisfiable.
[✓] DEADLINE COMPARABILITY (§11): arm-independent-deadline türetilebilir (ortak 48-symbol source-envelope; intervention
    HARİÇ) → DUR-B DEĞİL. Ayrıntı: FS-1-DEADLINE-GEOMETRY-NOTE.md (bu-tur; onay-bekler).
```

## 5. NEDEN İNŞA-ÖNCESİ DURULDU (F4-önce-çöz)
```text
- 540/540-executability (§16) 6-arm-GEREKTİRİR; F4 60-run-realize-edilemez → gate-FAIL → freeze-imkânsız.
- F4-çözümü arm-mimarisini-belirler (OPT-A adaptive-R → F0-benzeri-spine; OPT-B/D block-K-ladder → variable-K-spine).
  6-arm-dispatcher/manifest F4-şekli-netleşmeden finalize-edilmemeli (rework-riski + RULE-2).
- Disiplin: uygun-yorum-SEÇİLMEZ (DR-WORKLOAD-1 §23); ambiguity → ratify. Fragmenter/spine F4-çözümünden-SONRA
  bütünsel-inşa (fragmenter F4-bağımsız-olsa-da 6-arm-spine F4-parametrizasyonuna-bağlı).
```

## 6. STOP
HARNESS-EXT-1 6-arm-build F4-realizability-ratifikasyonu (§3 OPT-A/B/C/D) + FS-1-DEADLINE-GEOMETRY-NOTE-onayı
bekliyor. Kod-YOK · manifest-YOK · freeze-YOK · execution-YOK · 0 FS-1 run. Diğer-5-arm + mapping-check'leri
inşaya-hazır. RULE-4 (partition-independent trace identity) kaydedildi.
```text
"Layer-ambiguous / ungrounded parametre freeze-edilemez." (RULE-2)  "A result-bearing ARM = closed-loop ∧
cell-grid-parameterized ∧ dispatcher-registered ∧ canonical-result-producing." (RULE-1)
```
