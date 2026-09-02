# FS-1-CROSS-ARM-BEHAVIORAL-COLLAPSE-AUDIT (S4-A)

> **VERDICT: CROSS-ARM RESULT ROW DUPLICATION — DUR-B. STOP.** S4-A cross-arm-collapse-audit'te (§11) F1 ve F4
> RESOLVED-behavior olarak fixed-K-arm'lara ÇÖKÜYOR: **F1 ≡ F0** (her hücre; K8 both; ACCEPTED structural-null) ve
> **F4 ≡ F3** (RTT50 K4) / **≡ F0** (RTT250,1000 K8). 360-row manifest'in **120 satırı** (F1:60 + F4:60)
> behaviorally-collapsed duplicate-execution. Directive §11/§13/§33 gereği: manifest ÜRETİLMEZ; arm tek-taraflı
> ÇIKARILMAZ; **Abkarada scope-kararı** beklenir. sender/loss/spine kod-DEĞİŞMEDİ (fabrikasyon-YOK).

## 0. Provenance + method
```text
branch/HEAD : d1-sprint5c / ccbe290 (S3-R3) · v1.2 immutable da4af3c4…034 · FS-1 0/360 · v1.3-freeze YOK
method : empirik paired run_spine (paced 0.80C [pacer=800k, dt=10µs] + burst) × {noloss, d2=[2,5], tail=[47]} ×
  outcome-tuple + forward_trace_hash field-by-field. Kanıt-tablosu: FS-1-RESOLVED-ARM-SEMANTICS.md §3.
resolved_behavior_semantics_hash (§10) kavramı: arm-display-name HARİÇ; K/profile/blok/R/repair/close/schedule/
  trace DAHİL. F1/F4'ün fixed-K-arm'la tek farkı (eligibility-flag, K-seçim-yolu) bu hash'i DEĞİŞTİRMEZ çünkü
  result-bearing-execution'ı DEĞİŞTİRMEZ (§11: "farklı arm-enum/karar-yolu tek-başına distinct-behavior kurmaz").
```

## 1. Zorunlu karşılaştırmalar (§11) — sınıflandırma
```text
karşılaştırma           | resolved-K | outcome+ftrace eşit? | SINIF
------------------------+------------+----------------------+---------------------------
RTT50  F4 vs F3         | K4 both    | EVET                 | BEHAVIORALLY COLLAPSED
RTT250 F4 vs F0         | K8 both    | EVET                 | BEHAVIORALLY COLLAPSED
RTT250 F4 vs F1         | K8 both    | EVET                 | BEHAVIORALLY COLLAPSED
RTT1000 F4 vs F0        | K8 both    | EVET                 | BEHAVIORALLY COLLAPSED
RTT1000 F4 vs F1        | K8 both    | EVET                 | BEHAVIORALLY COLLAPSED
(ek) F1 vs F0 (her RTT) | K8 both    | EVET                 | BEHAVIORALLY COLLAPSED  ← structural-null sonucu
```
Ek-bulgu (directive'in istediği "any other equal resolved K/profile"): **F1 ≡ F0** de collapse — F1'in
early-logical-close'u bulk'ta parity-wire'ı advance-ETMEZ (S0-B/S2 CONFIRMED structural-null) → outcome ÖZDEŞ.

## 2. "one arm's outcome derivable from another paired arm?" (§11)
```text
EVET — aynı paired-trace altında:
  F1(hücre,rep) outcome = F0(hücre,rep) outcome (byte-özdeş; ftrace-özdeş). F1 = F0'dan türetilebilir.
  F4(RTT50,hücre,rep) = F3(aynı) ; F4(RTT250/1000,hücre,rep) = F0(aynı). F4 = F3/F0'dan türetilebilir.
⇒ Bu satırları ayrıca çalıştırmak, AYNI deterministik-hesabı farklı-label ile TEKRAR çalıştırmaktır (RULE-11 ihlali
  eğer distinct-result sayılırsa).
```

## 3. Etkilenen satır-sayısı (candidate 360 üzerinden)
```text
candidate 360 = 12-cell × 6-arm × 5-rep.
  F1 satırları : 12 × 5 = 60 → HEPSİ F0'a collapse (F1 K8, F0 K8, her hücre).
  F4 satırları : 12 × 5 = 60 → RTT50-hücreleri(4×5=20) F3'e; RTT250/1000-hücreleri(8×5=40) F0'a collapse.
  TOPLAM collapsed = 120 / 360 (%33).
DISTINCT result-bearing = 240 (arm-set {A0,A1,F0,F3} × 12 × 5 = 4×60). 
```

## 4. Neden bu GERÇEK (spine-defekti/fabrikasyon-DEĞİL)
```text
- Collapse, KABUL-EDİLMİŞ S0-B/S2 structural-null'ın doğrudan sonucu: sender-wide DATA-first → tüm-source-SONRASI
  parity-wire → early-close(F1)/adaptive-emission(F4) bulk'ta NON-EFFECTIVE. F1/F3 parity-wire-null S2'de CONFIRMED.
- F4'ün bilimsel-değeri CROSS-cell (RTT-sweep'te en-iyi-K'yi izler); TEK-hücrede daima mevcut bir fixed-K'ye çözülür.
  FS-1'de K2 hiç tetiklenmez (min-RTT 50→K4) → F4 her hücrede F3 veya F0/F1 ile ÖZDEŞ.
- Directive §11/§12: farklı bilimsel-kimlik AMA aynı resolved-behavior → aynı result-bearing-row'u iki-kez çalıştırmayı
  OTOMATİK haklı-çıkarmaz. §33: "resolved F4 behavior collapses to another arm without preregistered justification" +
  "any other cross-arm behavioral collapse" → DUR-B.
- Kod DEĞİŞTİRİLMEDİ; distinct-lık FABRİKE-EDİLMEDİ; arm tek-taraflı ÇIKARILMADI (§11).
```

## 5. Repetition-semantics (S4-A ikinci-madde) — BLOKER-DEĞİL (bilgi-için)
```text
Repetition-uniqueness (§5) SAĞLANIR (blocker değil): her (base_cell, rep) için distinct trace-seed →
  ExactDeficit: distinct global-drop-set (deterministik lowest-hash-rank seçim); NaturalIid: distinct trace-seed.
  5-rep → 5 distinct lineage. EXACT_DEFICIT/IID_REPETITION_LINEAGE_UNIQUE tasarlanabilir-distinct.
Makine (seed + pin) HENÜZ İNŞA-EDİLMEDİ: arm-set scope-kararına bağlı (manifest-yapısı arm-set'e göre değişir) →
  collapse çözülmeden repetition-machinery + manifest üretmek prematüre. Collapse = birincil-blocker.
```

## 6. Abkarada scope-kararı (SEÇENEKLER; öneri-YOK-tek-taraflı-uygulama-YOK)
```text
OPT-A — ARM-SET İNDİR (distinct-only): FS-1 result-bearing = {A0, A1, F0, F3} (4 arm). F1/F4 → DIAGNOSTIC
  mechanism-validation (FS-1-DIŞI; §11 "mechanism-validation fixtures may remain diagnostic outside FS-1").
  count: 12 × 4 × 5 = 240 result-bearing. (F1/F4'ün structural-null/adaptive-değeri diagnostic-pinlerde zaten kanıtlı.)
OPT-B — F1/F4'ü GENUINELY-DISTINCT yap: sender-ordering/interleaved-parity redesign (DR-FEC-INT-1) ile early-close/
  adaptive-emission'ı result-affecting yap → F1/F4 distinct-outcome üretir. BÜYÜK-iş; FS-1 kapsamı-DIŞI (DR-FEC-INT-1).
OPT-C — COLLAPSE'İ KABUL ET + 360'ı KORU: F1/F4'ü preregistered-justification ile "confirmatory-replica" olarak
  tut (campaign'de structural-null'ı empirik-doğrular). Directive §18: "explicit prior human decision and prereg
  justification" gerekir — "No such permission is granted in this directive" → yalnız Abkarada verebilir.
OPT-D — KARMA: {A0,A1,F0,F3} result-bearing + F1 confirmatory-replica (structural-null empirik-teyit) + F4 diagnostic.
```

## 7. STOP
```text
CROSS-ARM RESULT ROW DUPLICATION — DUR-B. S4-B/C/D (manifest + 360/360-executability + checkpoint) BAŞLATILMADI.
FS-1-MANIFEST-v1.3-CANDIDATE.jsonl ÜRETİLMEDİ (§13 "only if S4-A passes without STOP"). arm ÇIKARILMADI.
kod/spec/frozen-matris DEĞİŞMEDİ; sender/loss zero-diff; FS-1 0/360; v1.2 immutable. Abkarada scope-kararı bekleniyor.
```
