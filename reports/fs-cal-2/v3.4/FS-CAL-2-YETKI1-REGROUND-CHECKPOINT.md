# FS-CAL-2-YETKI1-REGROUND-CHECKPOINT

## STATUS: **NOT STARTED** (FAZ-5 — single bulk YETKI-1 re-grounding)

**Reason:** turn ended after FAZ 1 completed and passed; no further behaviour phase was entered.

* Gated on all accepted product changes being complete.
* MUST re-run after FAZ 1: the product baseline changed (StartupPacingConfig default OFF->ON), which invalidates arq_contract_id, product contract, golden traces, M1/M2, DATA-PROCESSABLE, S0 and §4.2b.
* Contract remains CONTRACT-STALE-PENDING-FINAL-REGROUND.
* Fresh §4.2b must re-check the recorded eligibility side-effect from FAZ 1.
* FS-CAL YETKI-2 NOT RUN.

`result-bearing calibration runs = 0` · `FS-CAL YETKİ-2 NOT RUN` · no commit/push/PR.
