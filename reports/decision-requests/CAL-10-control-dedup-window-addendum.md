# CAL-10 ADDENDUM — CONTROL_DEDUP_WINDOW (F-CID-0) [SEALED]

**Statü:** F-CID-0 (DR-CS-1b/A2; full-scale-directive §8.3) çıktısı. freeze-r2'nin CONTROL_DEDUP_WINDOW=4096
CANDIDATE değerini normatif hard-bound türetimiyle DOĞRULAR ve mühürler. Değerler gözlemsel-ortalama DEĞİL;
kod/spec enforce-max'larından türetildi. Kod: `kama-noise-wire/src/control_id_gate.rs` (derleme-zamanı
`const _` assert'leriyle pinli).

## INPUTS (hard-bound'lar; return-supply/split-cap türevi)

```text
MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION = 32   id/s   (return-supply kotası QUOTA_R_DEFAULT=4 epizot-başı
                                                       × control-tip-çeşitliliği + RECOVERED payı; konservatif)
MAX_CONTROL_ID_BURST                    = 64   id     (tek-batch worst: ACK-split MAX_ACK_RANGES=16 +
                                                       SACK-split ~32 + RECOVERED 16)
MAX_CONTROL_REORDER_ALLOWANCE           = 16   id     (ağ-reorder derinliği)
MAX_RETRY_REORDER_HORIZON_S             = 105  s      (PARAM_SET/CLOSE bounded-retry + patience F0-(viii)-max;
                                                       retry=aynı-id §2.57 bu horizon içinde)
```

## FORMULA (§8.3)

```text
required_window = RATE × HORIZON + BURST + REORDER_ALLOWANCE
               = 32 × 105 + 64 + 16
               = 3360 + 80
               = 3440

CONTROL_DEDUP_WINDOW = next_power_of_two(3440) = 4096
```

## RESULT

```text
CONTROL_DEDUP_WINDOW = 4096   (≥ 3440 required; power-of-two; bitmap 512 B/yön)
```
freeze-r2 CANDIDATE (4096) DOĞRULANDI → SEALED. Bitmap-maliyeti 64-session × 2-yön × 512 B = 64 KiB
(side-state bütçesi içinde; freeze-r2 §7).

## HARD ENFORCEMENT POINTS

```text
1. ControlIdIssueGate.issue() — monoton control_id (A-3) + per-batch BURST-enforce (>MAX_BURST →
   BurstExceeded fail-closed) + wrap→RenewalRequired. Producer'lar (Ack/Sack/Recovered/BlockComplete/
   Abort/ParamSet/ParamAck/SessionClose/RenewRequired) YENİ-logical-control'da BU gate'ten id alır;
   retry AYNI-id (gate çağrılmaz). CsProbe/CsEcho gate'e GİRMEZ (A-2 stateless).
2. RATE'in zaman-katmanı enforce'u emisyon-tick'inde (F4.5 CAL-10-doğrulama-kapısı): gerçek producer
   normatif-bound'u aşarsa → DUR-B + CAL-10 yeni-versiyon (mühür-sonrası-değişiklik=yeniden-onay).
3. F3 ControlDedupWindow bu 4096 değeriyle kurulur (fixed bitmap; window-altı → ControlIdBelowDedupWindow).
```

## PROVENANCE

```text
sha256: a82efd24682eea8db167f4222d633c15545c481fe5a1e2b254e3b261030e2b67 (§PROVENANCE-sha256-satırı hariç gövde;
        doğrulama: grep -v '^sha256: ' <bu-dosya> | sha256sum)
tarih : 2026-07-30T18:25:47Z
```
