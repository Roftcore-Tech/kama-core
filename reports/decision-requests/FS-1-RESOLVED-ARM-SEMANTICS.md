# FS-1-RESOLVED-ARM-SEMANTICS (S4-A)

> Her (RTT × impairment-aile × arm) için RESOLVED execution-mekanizması. S4-A cross-arm-collapse-audit'in
> kanıt-tabanı. DIAGNOSTIC (spine-gözlemli; result-bearing-DEĞİL). Empirik-ölçüm: paired run_spine (paced 0.80C +
> burst; noloss/d2/tail fixture'ları) — outcome + forward_trace_hash field-by-field.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / ccbe290 (S3-R3) · v1.2 immutable da4af3c4…034 · FS-1 0/360
arm-config otoritesi : S1/S2 (DEĞİŞMEDİ). A0 r=0 · A1/F0/F1/F3/F4 r=2 (diag). F4 direct-K = f4_select_k(base_rtt).
```

## 1. Resolved arm-mekanizması (§9)
```text
arm | proaktif-mekanizma        | K   | blok | R/blok | repair | close/eligibility        | structural-class
----+---------------------------+-----+------+--------+--------+--------------------------+---------------------
A0  | YOK (saf ARQ)             | N/A | N/A  | 0      | 0      | —                        | N/A
A1  | blind duplicate (blok-tot)| N/A | N/A  | r=2    | 0      | per-source-adjacent-early| N/A
F0  | fixed-K FEC               | 8   | 6    | 2      | 12     | block0-early             | N/A
F1  | fixed-K early-logical-cl. | 8   | 6    | 2      | 12     | any-block-early          | BULK_PARITY_WIRE_NULL
F3  | K'=4 partial-block        | 4   | 12   | 2      | 24     | any-block-early          | BULK_PARITY_WIRE_NULL
F4  | adaptive direct-K         | *   | *    | 2      | *      | any-block-early          | N/A (mechanistic)
```

## 2. F4 resolved-K per FS-1 RTT (f4_select_k; base_rtt = cell-RTT)
```text
RTT   | base<50→K2 / 50-150→K4 / ≥150→K8 | resolved-K | blok | profile | repair | ÇAKIŞAN fixed-arm
------+----------------------------------+------------+------+---------+--------+-------------------
50    | 50ms ∈ [50,150) → K4             | 4          | 12   | 0x02    | 24     | F3 (K4)
250   | 250ms ≥150 → K8                  | 8          | 6    | 0x03    | 12     | F0/F1 (K8)
1000  | 1000ms ≥150 → K8                 | 8          | 6    | 0x03    | 12     | F0/F1 (K8)
```
NOT: FS-1'de K2 ASLA seçilmez (min-RTT 50→K4). F4 her hücrede mevcut bir fixed-K-arm'ın K'sine ÇÖZÜLÜR.

## 3. Empirik resolved-outcome eşitliği (paced 0.80C + burst; 3 fixture)
```text
compare (aynı cfg: trace/drop/r/reverse/schedule)      | outcome-eşit? | forward_trace_hash-eşit?
-------------------------------------------------------+---------------+--------------------------
RTT50  F4(K4)  vs F3(K4)                                | EVET          | EVET (81cf92f4… / f9050464… / d431a4be…)
RTT250 F4(K8)  vs F0(K8)                                | EVET          | EVET (24a6398c… / bf48c2e0… / f0501229…)
RTT250 F4(K8)  vs F1(K8)                                | EVET          | EVET
RTT250 F0(K8)  vs F1(K8)                                | EVET          | EVET  ← F0 ve F1 de ÖZDEŞ
```
outcome-tuple = (k,blok,r,repair, completed,completion_ns,delivered,reasm, retx,fecdec,recbyte,
base,interv,arq,total, t_par_elig,t_par_wire,t_finalsrc,t_repuse,t_full, forward_trace_hash) — HEPSİ eşit.

## 4. DISTINCT vs COLLAPSED (S4-A sınıflandırma; detay → FS-1-CROSS-ARM-BEHAVIORAL-COLLAPSE-AUDIT.md)
```text
DISTINCT resolved-behavior (4): A0 (redundancy-YOK) · A1 (duplicate; interv-mekanizma farklı) ·
  F0 (K8; interv 1212B) · F3 (K4; interv 2424B — F0'dan farklı K/cost).
COLLAPSED: F1 → F0 (K8 both; early-close bulk'ta non-effective = ACCEPTED structural-null → outcome ÖZDEŞ) ·
  F4 → F3 (RTT50 K4) / F0 (RTT250,1000 K8) (adaptive mevcut-K'ye çözülür → outcome ÖZDEŞ).
```
```text
Neden collapse GERÇEK (fabrikasyon-DEĞİL): F1/F4'ün fixed-K-arm'dan tek farkı (eligibility-flag / K-seçim-yolu)
result-bearing-outcome'u DEĞİŞTİRMEZ — S0-B/S2 structural-null'ın (sender-wide DATA-first) doğrudan sonucu.
"farklı arm-enum / karar-yolu tek-başına distinct-result-behavior kurmaz" (§11).
```
