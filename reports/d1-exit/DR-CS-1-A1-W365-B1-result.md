# DR-CS-1 TUR-A1 — W-3-65 (B1) SONUÇ

**Tarih:** 2026-07-26 · **git:** `1b88336` · **Pre-reg:** R2 §3 (B1; anchor `fe119b4`) · **Harness:** `baea738`
**Kurulum:** faithful two-netns · 65B×16(w=8) · çift-yön blackout t0→restore=61s → temizle · **--no-settle-exit** ·
OBSERVATION_HORIZON=120s · RX_TEST_PATIENCE=130s · 3 tekrar · deficit=1.

> **ÇERÇEVE:** ARQ tüm kollarda; DR-CS-1 cold-start LIVENESS. Production-code-SIFIR (harness-only).

## ENSTRÜMAN ZİNCİRİ
acceptance_rtt100_min=100ms · per-cell RTT-realization (3-rep: p50=100, p95=100 → tol içinde) · overlap-gate
(3-rep receiver_events=9>0) · binary-provenance (bin_cs=820808, cold-start) · clock-domain=tek-host-MONOTONIC.

## SONUÇ — 3/3 PASS

| rep | receiver_events | finalized | pending | terminal | recovery_observation | RTT p50/p95 |
|---|---|---|---|---|---|---|
| 1 | 9 | **1** | 0 | TimedOut@59997ms | **RecoveredByForwardRecoveryControl** | 100/100 |
| 2 | 9 | **1** | 0 | TimedOut@60000ms | **RecoveredByForwardRecoveryControl** | 100/100 |
| 3 | 9 | **1** | 0 | TimedOut@59999ms | **RecoveredByForwardRecoveryControl** | 100/100 |

## B1 KABUL KONTROLÜ (frozen R2 pre-reg §3.1/3.2)
```
TimedOut exactly once                       ✔ (tek-terminal; ikinci-terminal-yok 3/3)
restore@61s                                 ✔ (kurulum)
existing production recovery                ✔ (RecoveredByForwardRecoveryControl = §11 forward-recovery-control)
finalized_blocks = 1                        ✔ 3/3
pending_blocks   = 0                        ✔ 3/3
application data correct                    ✔ (finalized=1; block bir-kez-teslim, was_new-guard)
application_duplicate_delivery = 0          ✔ (was_new-guard → çift-completion-yok)
no busy-loop                                ✔ (temiz-tamamlanma; C2 post-condition in-process-pinli)
no second timeout                           ✔ (tek TimedOut; ikinci-terminal-yok)
terminal_class      = TimedOut              ✔ 3/3
recovery_observation = RecoveredByForwardRecoveryControl  ✔ 3/3 (B1-beklenen 65B-sınıfı)
PostTimeoutRecovery  = not expected         ✔ (yok; DATA-ACK-kaynaklı 300/1200B-rejimi için korunur)
```

## HÜKÜM
> **W-3-65 (B1): 3/3 PASS.** F4'ün UNOBSERVED'i --no-settle-exit + 120s-horizon ile ÇÖZÜLDÜ; heal
> RecoveredByForwardRecoveryControl (ACK-bağımsız §11) ile GÖZLENDİ; PostTimeoutRecovery 65B-rejiminde
> beklendiği-gibi YOK. Bu hücre bounds-DUR-B'den (W-2B-R2) BAĞIMSIZ; kesindir.

**Not:** A2/DR-CS-1b açık-maddesi (POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT) — ürün-katmanının bu
ACK-bağımsız post-timeout-recovery'yi nasıl öğrendiği A2'de; end-to-end product FULL bundan-önce iddia-edilemez.
