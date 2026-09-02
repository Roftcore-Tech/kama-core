# FS-1-EXECUTION-CHECKPOINT (frozen FS-1 v1.3.1 result-bearing campaign — COMPLETE)

> **CAMPAIGN COMPLETE — 240/240 primary rows executed; all integrity gates PASS.** Operational/integrity record ONLY.
> **No FEC/ARQ interpretation, no arm ranking, no per-cell/per-arm breakdown, no timing/cost/deadline comparison** (§5/§8).
> Scientific analysis is a SEPARATELY authorized stage over this immutable dataset.

## Authorization
```text
authorized_by                 : Abkarada
authorization_scope           : full frozen FS-1 v1.3.1 result-bearing campaign
authorization_status          : AUTHORIZED_FOR_EXECUTION → EXECUTED
launch-authorization artifact : FS-1-LAUNCH-AUTHORIZATION-v1.3.1.json
launch-authorization sha256   : 5de69ea19892532f34f36ba05b42d5e3db8ff52f79ae115991ce0dcc2f730831
authorized_at_utc             : 2026-08-02T23:29:49Z
```

## Frozen inputs (all verified pre-launch == freeze-lock)
```text
freeze-lock (file) sha256     : 81eb908a1c35de27403239a46327d67353fca3cf36a32c60bd8c8d34e626a787  (FS-1-FREEZE-LOCK-v1.3.1.json)
execution binary sha256       : 8438991bf5cf01309b3a8c9f1b1ec7c2c648a1941e6ce0d49687c2595ac7e514  (reproduced pre- AND post-campaign)
final manifest sha256         : 12fc0a75da87e49bc078ed230e071c8d8751c981fe7ab9ae8226b591d35ddbcc  (FS-1-MANIFEST-v1.3.1.jsonl)
candidate manifest sha256     : e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10
prereg sha256                 : c12b2bee7c407cb1bcc089630c78a2fe8eabea4568129718230b2c2ed013660e  (FS-PREREG-v1.3.1.md)
execution-order sha256        : be692dea79334c7a4b78c808d0106b3d99ddff81775e311d01363bcfe5752623  (FS-1-EXECUTION-ORDER-v1.json)
raw-schema sha256             : c7ac2a85d25916111600e3a0d566774da2042bd037adc4a3b24623282217168c  (FS-1-RAW-SCHEMA-v1.json)
cargo_lock sha256             : 47893d24b4ea9b80cb1198f653931a09b8a15c8a4767a24375beb4337b046ad4
execution source commit       : a3ab2fe
toolchain                     : rustc 1.96.0 / cargo 1.96.0 (rust-toolchain.toml pinned; x86_64-unknown-linux-gnu)
superseded/invalidated lock   : FS-1-FREEZE-LOCK-v1.3.json (5a9bc09d; INVALIDATED-BEFORE-FIRST-RESULT; PRESERVED)
```

## §1 Pre-launch verification — ALL PASS
```text
binary SHA exact match            PASS (8438991b)
final manifest SHA exact match    PASS (12fc0a75 == lock.final)
execution-order SHA exact match   PASS (be692dea == lock)
raw-schema + prereg SHA           PASS (== lock)
new freeze-lock accepted          PASS (verify-chain PASS → gated STOP exit4, 0 writes)
old v1.3 freeze-lock rejected     PASS (binary-self-hash mismatch, exit3)
raw namespace empty               PASS (reports/fs-1/raw/v1.3.1 absent pre-launch)
primary ledger empty              PASS (reports/fs-1/ledger/v1.3.1-primary.log absent pre-launch)
result-bearing runs = 0           PASS (0 files under reports/fs-1 pre-launch)
local source == frozen baseline   PASS (crates/+Cargo.lock+toolchain zero-diff a3ab2fe→HEAD)
no output-path collision          PASS (240 unique run-id)
```

## Campaign execution
```text
execution parallelism             : 1 (serial; frozen execution order; one pairing group at a time)
invocation                        : fs1_runner --result-bearing --freeze-lock <v1.3.1> --final-manifest <v1.3.1>
                                    --out reports/fs-1/raw/v1.3.1 --ledger reports/fs-1/ledger/v1.3.1-primary.log
                                    --i-have-launch-authorization <auth>
campaign start (first raw) utc    : 2026-08-02T23:30:21Z
campaign end   (last raw)  utc    : 2026-08-02T23:30:56Z
process exit code                 : 0 (stderr empty)
execution backend                 : in-process deterministic channel (run_spine; BottleneckChannel) — netem/netns GEREKMEZ
```

## Row accounting (operational totals only)
```text
primary rows planned              : 240
primary rows attempted            : 240
primary rows reaching terminal    : 240  (240/240)
VALID_COMPLETE count              : 176
VALID_CENSORED count              : 64
INVALID attempt count             : 0
retry count                       : 0    (run_spine deterministic → no INVALID → no retry path fired)
unresolved row count              : 0
attempt discipline                : all 240 rows terminal at attempt_index=0
```
*(VALID_COMPLETE / VALID_CENSORED are aggregate operational counts per §5/§8; NO arm/cell breakdown, ranking, or comparison is recorded here.)*

## Artifact + integrity verification — ALL PASS
```text
raw record files                  : 240   (reports/fs-1/raw/v1.3.1/<run_id>.attempt0.json)
sha256 sidecar files              : 240
stray/tmp/quarantine residue      : 0     (atomic temp→fsync→rename; no interrupted writes)
checksum triple-match             : 240/240  (sha256(raw file) == .sha256 sidecar == ledger raw_sha)
ledger lines                      : 480   (240 STARTED + 240 COMPLETE)
ledger entry-checksums valid      : 480/480
ledger validity == raw validity   : 240/240
missing artifact check            : 0 missing (ledger COMPLETE run-id set == manifest 240 == execution-order 240)
duplicate artifact check          : 0 duplicates (240 unique run-id; 1 STARTED@0 + 1 COMPLETE@0 per run)
execution-order compliance        : PASS (ledger STARTED order == frozen FS-1-EXECUTION-ORDER-v1 order, all 240)
result_bearing flag in records    : 240/240 = true
```

## Provenance compliance — PASS (with documented known-limitation)
```text
cryptographic provenance stamped  : 240/240 EXACT — freeze_lock_sha256=81eb908a · final_manifest_sha256=12fc0a75 ·
                                    execution_binary_sha256=8438991b  (in every raw record)
execution_source_commit field     : uniformly "unknown" in all 240 records — EXPECTED frozen-binary behavior, NOT
                                    corruption. The frozen binary's lock_field helper extracts only 64-hex tokens, so the
                                    7-char commit "a3ab2fe" is not parsed and falls back to "unknown" (deliberate default).
                                    Source-commit binding IS fully established out-of-band: freeze-lock v1.3.1
                                    (execution_source_commit=a3ab2fe) + launch-authorization + git (source zero-diff vs a3ab2fe).
                                    Binary is FROZEN (§7) → not modified; rows NOT re-run. Recorded as operational note.
```

## Source / binary zero-change proof
```text
binary sha256 post-campaign       : 8438991b… (identical pre- and post-campaign; reproduced from git-archive a3ab2fe)
crates/ + Cargo.lock + toolchain  : zero-diff a3ab2fe → HEAD (git diff empty)
production sender.rs / loss.rs     : untouched (harness-only; experimental-fs-harness)
no mid-campaign changes           : source/binary/manifest/prereg/lock/schema/deadlines/horizons/traces/seeds/arm-set/
                                    execution-order/retry-policy ALL unchanged during execution (§7)
```

## Dataset identity (immutable)
```text
raw namespace                     : reports/fs-1/raw/v1.3.1  (240 records + 240 sidecars)
primary ledger                    : reports/fs-1/ledger/v1.3.1-primary.log
ledger sha256                     : a3e332bc6c170d0027e73b0ad2efbf41e02d62b21972e9ab341cdb7a4a13c946
dataset concat sha256 (sorted)    : 4048e6633a190341e74b2aaf8f2e7d176944d9fe1850d754fde201ffaec23832
```

## Status
```text
CAMPAIGN-COMPLETE : YES — 240/240 primary rows terminal; 0 unresolved; all integrity/provenance gates PASS.
NO permitted-stop condition encountered (no freeze/binary/manifest/order/ledger/checksum/overwrite/provenance/
  environment failure). No stop for scientific outcomes (per §6).
This checkpoint contains NO scientific interpretation (§8).
NEXT (separately authorized) : frozen FS-1 analysis pipeline over this immutable completed raw dataset.
STOP after this checkpoint is committed and pushed. Do NOT modify or rerun scientific rows.
```
