# FS-1-FREEZE-LOCK-v1.3 — INVALIDATION RECORD (OPT-R1)

> **FS-1-FREEZE-LOCK-v1.3.json (sha 5a9bc09d…) = INVALIDATED-BEFORE-FIRST-RESULT · reason = EXECUTOR GAP.**
> Eski-lock KORUNUR (overwrite-YOK; dosya byte-değişmez). 0 FS-1 result-bearing-row koştu → pre-outcome runtime/
> provenance amendment (scientific-design amendment DEĞİL). Yeni-kimlik: FS-1-FREEZE-LOCK-v1.3.1 (OPT-R1-sonrası).

## Karar
```text
S6 (08a35c2) LAUNCH-NOT-AUTHORIZED verdict KABUL. Abkarada OPT-R1 APPROVED.
Neden invalid : frozen fs1_runner (binary 75ca6eb7) result-bearing-EXECUTION-CONTRACT'ı yerine-getiremez
  (pozitif-yol kalıcı-refuse-stub; run+persist/append-only-raw-writer/ledger/checksum YOK).
Sınıf : INVALIDATED-BEFORE-FIRST-RESULT (reason=EXECUTOR GAP). result_bearing_runs_before_invalidation = 0.
Statü : PRESERVED (silme/overwrite YOK; FS-1-FREEZE-LOCK-v1.3.json + tüm S5-artifact yerinde kalır).
```

## Değişecek vs değişmeyecek (OPT-R1)
```text
DEĞİŞİR : fs1_runner executor+persistence-implementation · binary-hash · Cargo.lock (sha2/checksum-dep gerekirse) ·
  execution-baseline · execution_binary_sha256'e-bağlı provenance-hash'ler · yeni prereg-v1.3.1 + freeze-lock-v1.3.1.
DEĞİŞMEZ (FROZEN SCIENCE) : 12 base-cell · {A0,A1-EARLY,F0,F3} · 5 rep · 240 row · 60 group · workload-geometri ·
  ExactDeficit-target-set'ler · NaturalIid-seed'ler · RTT'ler · deadline'lar (199/599/2099) · horizon'lar (699/2499/9249) ·
  source-pacing (0.80C) · arm-mekanizmaları · execution-order · validity/censoring · retry-policy · analysis/oracle-authority.
  (invariance-proof: FS-1-SCIENCE-INVARIANCE-v1.3.1.md — R1-f.)
```

## Yeni-kimlik plan (R1)
```text
FS-PREREG-v1.3 → SUPERSEDED-BY FS-PREREG-v1.3.1 (executor-amendment; scientific-config UNCHANGED).
FS-1-FREEZE-LOCK-v1.3.json → INVALIDATED (preserved) ; FS-1-FREEZE-LOCK-v1.3.1.json → yeni one-way lock (executable-binary).
```
