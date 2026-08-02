# FS-1-OBSERVATION-HORIZON-AUDIT (S5-A)

> **GROUNDED (frozen v1.1 §B0.3, v1.2-preserved) — DUR-B YOK.** Observation-horizon ≠ deadline. Horizon
> arm-independent + outcome-independent frozen-formülden türetilir: **699 / 2499 / 9249 ms**. deadline execution'ı
> SONLANDIRMAZ; tail-stall → VALID_CENSORED. 5099ms FS-1-deadline DEĞİL (RTT2500 candidate; kapsam-DIŞI).

## 0. Provenance + audit sources
```text
branch/HEAD : d1-sprint5c / 83787d4 (S4-D) · v1.2 immutable da4af3c4…034 · FS-1 0/240
audit-kaynakları : FS-PREREG-v1.1 §B0.3 (OBSERVATION HORIZON/CENSORING) + §B0.2 (deadline) · FS-PREREG-v1.2 §D
  ("v1.1 §B0.2–B0.6 AYNEN") · FS-PREREG-v1.2 §A1.8 (tail=B_REAL_TAIL_ERASURE; horizon-uzatma ÇÖZMEZ) · §A1.9
  (INVALID ⟂ INCOMPLETE) · fs_harness tail-fixtures · deadline-geometry (S3 ratified).
```

## 1. Deadline vs observation-horizon (AYRI kavramlar)
```text
deadline : completion-ZAMANINDA-mı (timeliness endpoint). T_ser + N_recovery(2)*RTT + M_fixed(50). = 199/599/2099.
horizon  : geçerli-bitmemiş execution ne-zaman CENSORED olur (P(complete ≤ T_horizon) endpoint). > deadline.
deadline execution'ı SONLANDIRMAZ; execution horizon'a-kadar-veya-completion'a-kadar koşar.
```

## 2. Frozen horizon formülü (FS-PREREG-v1.1 §B0.3; v1.2 AYNEN)
```text
T_horizon(cell) = T_serialize_upper + k_rtt × RTT + bounded_recovery_allowance + M_fixed_horizon
  k_rtt = 6 · bounded_recovery_allowance = 3 × RTT · M_fixed_horizon = 200 ms
⇒ T_horizon = ceil(T_serialize_source) + (6+3)×RTT + 200 = 49 + 9×RTT + 200 = 249 + 9×RTT
T_serialize_upper = ceil(4848B × 8 / 0.80C) = ceil(48.48ms) = 49 ms  (base-source envelope; ARM-INDEPENDENT —
  arm-redundancy DAHİL-EDİLMEZ ki 4-arm AYNI horizon'a sahip olsun, §4). fs_manifest::observation_horizon_ms.
```

## 3. Frozen horizon table (§4 gerekli-tablo)
```text
RTT(ms) | deadline_ms | observation_horizon_ms | authority          | arm-indep | outcome-indep | censoring
--------+-------------+------------------------+--------------------+-----------+---------------+-----------------
50      | 199         | 699                    | v1.1 §B0.3 (v1.2)  | EVET      | EVET          | timeout>699 → CENSORED
250     | 599         | 2499                   | v1.1 §B0.3 (v1.2)  | EVET      | EVET          | timeout>2499 → CENSORED
1000    | 2099        | 9249                   | v1.1 §B0.3 (v1.2)  | EVET      | EVET          | timeout>9249 → CENSORED
(future) 2500 | 5099 (candidate; NON-authoritative) | — FS-1-DIŞI (RTT2500 manifest'te YOK) —
```
rationale : 9×RTT + 200ms recovery-üst-zarfı, tek-recoverable-execution completion'ından (deadline ≈ 2×RTT-recovery)
  ÇOK-üstünde → horizon'da-bitmemiş = GERÇEKTEN-stalled (ör. tail-erasure; current-substrate RTO/TLP-YOK).

## 4. Hard requirements (§4) — teyit
```text
✓ 4-arm (A0/A1/F0/F3) AYNI horizon (RTT'ye bağlı; arm-redundancy'ye DEĞİL).
✓ horizon outcome-independent (frozen-formül; diagnostic-completion-percentile'den TÜRETİLMEDİ).
✓ deadline execution'ı sonlandırmaz (horizon > deadline; execution horizon'a-kadar koşar).
✓ tail-stall VALID_CENSORED olabilir (A0 tail = B_REAL_TAIL_ERASURE; v1.2 §A1.8 "horizon-uzatma ÇÖZMEZ" —
  yapısal, timeout-değil → censor, INVALID-DEĞİL).
✓ intervention-trafiği horizon'u UZATMAZ/KISALTMAZ (horizon yalnız RTT+base-source; arm-intervention GİRMEZ).
```

## 5. Validity + censoring states (§5; FROZEN top-level)
```text
INVALID       : execution-contract failure (manifest/binary/freeze-lock/trace/source-hash mismatch · schema-fail ·
                corrupted · event-loop-fail · impossible-typed-config). BİLİMSEL-outcome-DEĞİL (v1.2 §A1.9: INVALID⟂INCOMPLETE).
VALID_COMPLETE: 48 fragment / 8 app-unit / 2400 byte teslim + byte-eşit reassemble, horizon-ÖNCESİ.
VALID_CENSORED: execution-contract geçerli AMA full-workload-completion horizon-ÖNCESİ olMAZ. A0 tail-stall örnek.
                censored ≠ INVALID/failed-test/auto-retry/removable-unfavorable (v1.2 §A1.9 LEGITIMATE-INCOMPLETE;
                P(complete≤horizon)-endpoint'ine girer).
censoring-anında-kayıt : censor_time · delivered-fragments · delivered-bytes · remaining-fragments · tail-status ·
                last-meaningful-event · pending-recovery-state · completion-cause.
```

## 6. VERDICT
```text
OBSERVATION-HORIZON GROUNDED (frozen v1.1 §B0.3 formülü + S3 source-envelope). arm-independent + outcome-independent +
> deadline + tail→CENSORED + intervention-bağımsız. DUR-B YOK. Değerler: 699/2499/9249 (fs_manifest::observation_horizon_ms;
pin s5a_observation_horizon). 5099ms YALNIZ RTT2500-candidate (FS-1-DIŞI; non-authoritative). Censoring-states FROZEN (§5).
```
