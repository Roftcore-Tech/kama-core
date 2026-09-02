# FS-1 PRE-FLIGHT STOP — PREREG/MANIFEST INCONSISTENCY (DR-FEC-FS-1)

> **STATÜ: STOPPED BEFORE FIRST RESULT-BEARING RUN.** FS-1 execution directive'inin zorunlu ön-uçuş kapısı
> tetiklendi: frozen repository authority 540 result-bearing-run manifestini ÜRETEMEZ. Directive gereği:
> "If repository authority does NOT reproduce 540: STOP BEFORE FIRST RESULT-BEARING RUN; classify
> PREREG/MANIFEST INCONSISTENCY; report the discrepancy. Do not silently repair the matrix." (+ §28.B).
>
> **HİÇBİR FS-1 result-bearing-run KOŞULMADI. Matrix ONARILMADI. Frozen mekanizma DEĞİŞTİRİLMEDİ.
> FS-ANALYSIS-SPEC / F1-note FREEZE-EDİLMEDİ** (manifest çözülene dek prematüre — arm-set değişebilir).

## 0. PROVENANCE (resolved)
```text
branch            : d1-sprint5c
HEAD              : 0b1329a91bde258e43b1ae8ac43babe81d923ef2
harness-B commit  : cd336e9 (F1/F3/F4 = mechanism-fixture/pin)
FS-PREREG-v1.2    : FROZEN sha256 = da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
                    (TAM hash; reproduces: sha256(file − "sha256:" satırı) == stored ✓)
FS-PREREG-v1.1    : 60353e99715cd4e51a42c53a94b6c1ad6963d8c3b3ef017ebc1bf2b9c3aaadf7 (SUPERSEDED-BY-v1.2)
coverage authority: FS-COVERAGE-PLAN-v1.1.md sha256 = 2648370135bae58b6b9a9f8981e0d0da08c67ac3a9b2bbeb831f93d7ae0a4fc5
coverage base     : FS-COVERAGE-PLAN.md (v1.0) sha256 = 91b33e1e… (SUPERSEDED-BY-v1.1)
```

## 1. RUN-COUNT DERİVASYONU (repo authority'den)
18-hücre grid (FS-COVERAGE-PLAN.md §1, v1.1'de arm-seti güncellendi):
```text
RTT        = {50, 250, 1000} ms                      (3)
payload    = {300 B} sabit
deficit    = {1, 2, 4}                                (3)
impairment = {exact-deficit, IID-20%}                (2)
util       = 0.80C sabit
→ base-cells = 3 × 3 × 2 = 18
arms       = {A0, A1-EARLY, F0, F1, F3, F4}          (6; F2 DROPPED-DUR-B)
seeds      = 5
FS-1 TOPLAM = 18 × 6 × 5 = 540   (FS-COVERAGE-PLAN-v1.1 §1; FS-PREREG-v1.2 §D "6 × 18 × 5")
```
**Sayısal-count 540 REPRODUCES** (v1.1 §1 + v1.2 §D uyumlu). ⇒ count-check GEÇTİ.
**ANCAK count-eşleşmesi GEREK-ama-YETER-DEĞİL:** manifest, frozen harness tarafından EXECUTE-edilebilir olmalı.

## 2. DOKÜMANTER TUTARSIZLIK (minor; resolved-by-authority)
Directive "FS-PREREG-v1.2 / **FS-COVERAGE-PLAN-v1.2**"e atıf yapar; repo'da **FS-COVERAGE-PLAN-v1.2 YOK** —
yalnız v1.1 var. Otorite = FS-COVERAGE-PLAN-v1.1 (v1.2-prereg PROVENANCE'ı da onu referanslar). Count v1.1'den
türetildi (540). Bu kalem TEK-BAŞINA blocker değil (v1.1 otoriter); bilgi-için kaydedildi.

## 3. ⛔ PRIMARY INCONSISTENCY — HARNESS ARM-COVERAGE GAP (blocker)
Frozen manifest **6 EXECUTABLE result-bearing arm** ister. Frozen harness (cd336e9) yalnız **3'ünü** closed-loop
result-bearing olarak uygular:
```text
A0        : run_arm (fs_harness.rs:587)                         → full RunResult (completion/timing/cost) ✓
A1-EARLY  : run_arm (fs_harness.rs:587)                         → full RunResult ✓
F0        : run_f0  (fs_f0.rs:80)                               → full F0Result (real-codec/completion/timing) ✓
F1        : run_f1 YOK. YALNIZ f1_real_decode_is_f0 (=run_f0) + PIN-C ParityWireModel fixture (mechanism-null) ✗
F3        : run_f3 YOK. YALNIZ partial-block encode/decode FIXTURE (fs_fec_arms.rs:62; completion-loop-YOK) ✗
F4        : run_f4 YOK. YALNIZ f4_select_k KURALI (fs_harness.rs); closed-loop-YOK ✗
fs_runner : arm-dispatch YALNIZ {a0|a1|f0} (examples/fs_runner.rs:80,142). f1/f3/f4 çalıştıramaz ✗
```
**Sonuç:** F1/F3/F4 × 18-cell × 5 = **270 / 540 run EXECUTE-EDİLEMEZ** (frozen harness'ta closed-loop-arm-YOK;
FS-1 outcome-şeması [completion_time/deadline_hit/timing-ledger/total-cost] üretilemez).
**Neden freeze ile onarılamaz:** F1/F3/F4 için closed-loop-arm-inşası = "F1/F3/F4 semantics" + "fs_runner arm
behavior" değiştirmek → §1 ABSOLUTE FREEZE tarafından AÇIKÇA YASAK; ayrıca "silently repair the matrix" ihlali.
**Kök-neden:** FS-HARNESS-B directive'i F1/F3/F4'ü MEKANİZMA-PİNİ olarak scope'ladı (PIN-C fixture / adaptive-K
mechanism pin), FS-1-full-closed-loop-arm olarak DEĞİL. Harness-B tam-olarak istendiği-gibi mekanizma-pini üretti.
FS-1-execution ise 6-executable-arm ister. Frozen-coverage (6-arm) ile frozen-harness-kapasitesi (3-arm) arasında
MATERIAL fark var (§28.B: "expected manifest differs materially from frozen coverage").

## 4. ⛔ SECONDARY INCONSISTENCY — IMPAIRMENT + CELL-GRID PARAMETRİZASYON GAP (blocker; 3-arm'ı bile keser)
Uygulanan 3-arm bile 18-hücre-gridini kaplayamaz:
```text
(a) IMPAIRMENT: 18-hücrenin 9'u impairment=IID-20% ister. run_arm (fs_harness.rs:599) VE run_f0 (fs_f0.rs:86)
    YALNIZ TraceSpec::ExactDeficit kullanır — NaturalIid RUN-yoluna PLUMB-EDİLMEDİ (NaturalIid yalnız
    run_config_hash-dummy'sinde [829] + test'lerde [1092/1108]). ⇒ 9/18 hücre (IID-20%) HİÇBİR arm için
    EXECUTE-edilemez.
(b) PAYLOAD/UTIL/CELL-GRID: fs_runner (execution-tool) num_units=8, unit_size=64 HARDCODE (fs_runner.rs:62-63) —
    manifest payload=300B ister. Kabul-edilen-arg'lar {--arm,--r,--seed,--link-bps,--owd-ms,--deficit,
    --reverse-blackout} — payload / impairment / util / RTT-cell arg-YOK. ⇒ execution-tool 18-hücre-gridini
    parametrize-EDEMEZ.
```
Bunları eklemek = "forward impairment semantics" + "fs_runner arm behavior" + "wire accounting(payload)"
değiştirmek → §1 FREEZE YASAK + silent-repair.

## 5. CLASSIFICATION
```text
PREREG/MANIFEST INCONSISTENCY  (directive pre-flight-gate + §28.B "expected manifest differs materially
from frozen coverage"). Alt-sınıf: HARNESS-COVERAGE-GAP (primary) + IMPAIRMENT/CELL-GRID-PARAMETRIZATION-GAP
(secondary). Frozen coverage 540-executable-run varsayar; frozen harness ≤ (3 arm × 9 exact-deficit-cell × 5)
= 135 executable-run üretebilir (F1/F3/F4 + IID-cell'ler dışarıda). 540 − executable = MATERIAL-GAP.
```

## 6. NE YAPILMADI (disiplin)
```text
✗ Hiçbir FS-1 result-bearing-run koşulmadı (0/540).
✗ Matrix onarılmadı (F1=F0 reinterpretasyonu YOK; F3/F4 closed-loop-inşası YOK; IID-plumbing YOK; count-küçültme YOK).
✗ Frozen mekanizma değiştirilmedi (Sender/loss/fs_runner/arm-semantics/impairment DOKUNULMADI).
✗ FS-ANALYSIS-SPEC / F1-STRUCTURAL-NULL-note FREEZE-edilmedi (manifest-çözümü arm-set'i değiştirebilir →
   prematüre-freeze bias-riski; çözümden SONRA outcome-blind freeze edilecek).
```

## 7. ABKARADA-KARARI GEREKTİREN ÇÖZÜM SEÇENEKLERİ (UYGULANMADI — bilgi-için)
```text
Seçenek-1 (FULL): Yeni bir HARNESS-genişletme-DR'si ile (freeze-kaldırma) F1/F3/F4 closed-loop-arm + IID-impairment
  plumbing + fs_runner cell-grid-arg'ları inşa et; harness-checkpoint + manifest yeniden-türet; SONRA 540 koş.
  (Kapsam: yeni-kod = frozen-değil; ayrı-onay + yeni-freeze şart.)
Seçenek-2 (RESCOPE): FS-1'i frozen-executable-alt-küme'ye indir (ör. A0/A1/F0 × exact-deficit-9-cell × 5) +
  coverage/prereg'i AÇIKÇA amend-et (540 → yeni-count). Frozen-540 değişir → Abkarada-onayı + re-freeze şart.
  (F1/F3/F4 kanıtı Harness-B mechanism-pin'lerinde KALIR; FS-1-outcome-satırı üretmez.)
Seçenek-3 (AMEND): Coverage-v1.2 yaz; F1/F3/F4'ü FS-1'de "mechanism-only (outcome-satırı-yok)" olarak resmen-tanımla
  + result-bearing-count'u yalnız-outcome-üreten-arm'lara göre yeniden-türet.
HİÇBİRİ bu turda uygulanmadı — hepsi frozen-540'ı ve/veya frozen-mekanizmayı değiştirir → Abkarada-kararı.
```

## 8. STOP
FS-1 BAŞLATILMADI. Directive pre-flight-gate + §28.B uyarınca DURULDU. Abkarada'nın manifest-uyuşmazlığını
çözmesi bekleniyor (yukarıdaki seçenekler). Çözüm gelene dek: execution-YOK · analysis-spec-freeze-YOK ·
harness-değişikliği-YOK · matrix-onarımı-YOK.
```text
"A surprising/blocking pre-flight finding is evidence, not a reason to edit the mechanism." (FINAL EXECUTION PRINCIPLE)
```
