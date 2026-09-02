#!/usr/bin/env python3
"""FS-FUSION-CAL-1 Phase B — canonical calibration dataset builder (READ-ONLY over authoritative data).

Maps the three AUTHORITATIVE datasets into one canonical schema WITHOUT modifying or re-running them:
  FS1_IN_PROCESS   reports/fs-1/raw/v1.3.2/*.json            (240,  FS-1-RAW-SCHEMA-v2, 57 keys)
  R2A_IN_PROCESS   reports/dr-arq-tail-1/v1/raw/*.json       (8400, same 57-key schema)
  PRODUCT_NETNS    reports/dr-arq-product-tail-1/v1/result.csv (2290, 28 cols)

Domain separation is MANDATORY (§5.1): in-process and netns rows are never pooled into one population.
Runtime fusion-input telemetry is ABSENT in all three (verified) → every canonical row carries
runtime_features_present=0 + runtime_features_absent_reason, and no row may be used to fit a
runtime feature→action map. Authoritative rows ground CELL→ACTION→OUTCOME only.

Excluded / superseded sources (never read): reports/fs-1/raw/v1.3.1 (SUPERSEDED-INCOMPLETE-SCHEMA),
commit 44e0c50 dataset (INVALIDATED arm-label bug).

Output: canonical-calibration-data.csv (+ .sha256), ingest-ledger.json (accounting + exclusions).
"""
import csv, json, os, sys, hashlib, glob
from collections import Counter

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SCHEMA_VERSION = "FS-FUSION-CAL-1-CANONICAL-v1"

# Canonical action vocabulary. Domain is ALWAYS kept alongside — actions are never pooled across domains.
ACTION_MAP = {
    # FS-1 (in-process) canonical arms
    "A0": "ARQ_FROZEN",          # ACK-driven ARQ, tail-blind (INV-CS-4)
    "A1-EARLY": "BLIND_DUP",     # blind duplication at matched configured budget
    "F0": "FEC_R2",              # proactive coding r=2
    "F3": "FEC_R4",              # proactive coding r=4 (budget 2x F0)
    # R2A (in-process) canonical arms
    "A0-FROZEN": "ARQ_FROZEN",
    "A0-TAIL": "ARQ_TAIL",       # experiment-only bounded tail probe (deconfounder)
    # PRODUCT netns arms (tail recovery OFF/ON)
    "off": "ARQ_FROZEN",         # product default (tail probe disabled)
    "on": "ARQ_TAIL",            # product bounded tail probe (frozen validated config)
}

CANON_COLS = [
    # provenance / lineage
    "canonical_id", "domain", "source_artifact", "source_commit", "source_binary_sha256",
    "run_id", "cell_id", "arm_raw", "action", "repetition_index", "pairing_group_id",
    # workload
    "base_payload_bytes", "workload_units",
    # runtime observability (ABSENT in all authoritative data — explicit, never silently null)
    "runtime_features_present", "runtime_features_absent_reason",
    # impairment configuration — OFFLINE ANALYSIS ONLY, forbidden as a runtime policy input
    "cfg_stratum", "cfg_rtt_ms", "cfg_loss_label", "cfg_structure", "cfg_drop_count",
    # endpoints (horizon and deadline kept SEPARATE — never conflated)
    "horizon_ms", "horizon_complete", "censored", "completion_time_ns", "censor_time_ns",
    "deadline_ms", "deadline_hit",
    # cost (realized)
    "total_wire_bytes", "arq_retransmit_bytes", "arq_retransmit_count",
    "fec_parity_bytes", "probe_bytes", "probe_count", "control_bytes",
    # mechanism / integrity
    "completion_cause", "tail_status", "reassembly_equal", "remaining_fragments",
    "fec_decoded_sources", "spurious_count", "invariant_ok",
]

ABSENT_REASON = "NO_RUNTIME_TELEMETRY_IN_AUTHORITATIVE_DATASET"


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def ns(v):
    return "" if v is None else int(v)


def ingest_inprocess(domain, pattern, source_label, rows, excl):
    """FS-1 v1.3.2 and R2A share FS-1-RAW-SCHEMA-v2 (57 keys)."""
    files = sorted(p for p in glob.glob(pattern) if not p.endswith(".sha256"))
    for p in files:
        try:
            d = json.load(open(p))
        except Exception as e:
            excl.append({"file": p, "reason": "UNREADABLE_JSON", "detail": str(e)})
            continue
        if not d.get("result_bearing", False):
            excl.append({"file": p, "reason": "NOT_RESULT_BEARING"})
            continue
        arm = d.get("arm", "")
        action = ACTION_MAP.get(arm)
        if action is None:
            excl.append({"file": p, "reason": "UNKNOWN_ARM_LABEL", "detail": arm})
            continue
        vc = d.get("validity_class", "")
        if vc not in ("VALID_COMPLETE", "VALID_CENSORED"):
            excl.append({"file": p, "reason": "INVALID_VALIDITY_CLASS", "detail": vc})
            continue
        co = d.get("completion_or_censoring") or {}
        complete = 1 if co.get("status") == "completed" else 0
        tmr = d.get("typed_manifest_row") or {}
        rows.append({
            "canonical_id": f"{domain}::{d.get('run_id','')}",
            "domain": domain,
            "source_artifact": os.path.relpath(p, REPO),
            "source_commit": d.get("execution_source_commit", ""),
            "source_binary_sha256": d.get("execution_binary_sha256", ""),
            "run_id": d.get("run_id", ""),
            "cell_id": d.get("base_cell_id", ""),
            "arm_raw": arm,
            "action": action,
            "repetition_index": d.get("repetition_index", ""),
            "pairing_group_id": d.get("pairing_group_id", ""),
            "base_payload_bytes": d.get("base_source_bytes", ""),
            "workload_units": "",
            "runtime_features_present": 0,
            "runtime_features_absent_reason": ABSENT_REASON,
            "cfg_stratum": tmr.get("stratum", ""),
            "cfg_rtt_ms": tmr.get("rtt_ms", ""),
            "cfg_loss_label": tmr.get("loss_ppm", tmr.get("p_loss", "")),
            "cfg_structure": tmr.get("impairment_kind", tmr.get("forward_kind", "")),
            "cfg_drop_count": tmr.get("drop_count", ""),
            "horizon_ms": d.get("observation_horizon_ms", ""),
            "horizon_complete": complete,
            "censored": 1 - complete,
            "completion_time_ns": ns(d.get("t_full_workload_complete")),
            "censor_time_ns": ns(d.get("censor_time_ns")),
            "deadline_ms": d.get("deadline_ms", ""),
            "deadline_hit": 1 if d.get("deadline_hit") else 0,
            "total_wire_bytes": d.get("total_outcome_bytes", ""),
            "arq_retransmit_bytes": d.get("arq_retransmit_bytes", 0),
            "arq_retransmit_count": d.get("retransmits", 0),
            "fec_parity_bytes": d.get("proactive_intervention_bytes", 0),
            # In-process A0-TAIL probes are emitted through the retransmit path; the schema has no
            # separate probe counter. Reported as "" (not 0) so absence is never read as a measured zero.
            "probe_bytes": "",
            "probe_count": "",
            "control_bytes": d.get("control_bytes", 0),
            "completion_cause": d.get("completion_cause", ""),
            "tail_status": d.get("tail_status", ""),
            "reassembly_equal": 1 if d.get("reassembly_equal") else 0,
            "remaining_fragments": d.get("remaining_fragments", ""),
            "fec_decoded_sources": d.get("fec_decoded_sources", 0),
            "spurious_count": d.get("spurious_retransmissions", 0),
            "invariant_ok": 1,
        })
    return len(files)


def ingest_product(csv_path, rows, excl):
    n = 0
    with open(csv_path) as f:
        for r in csv.DictReader(f):
            n += 1
            arm = r["arm"]
            action = ACTION_MAP.get(arm)
            if action is None:
                excl.append({"file": csv_path, "run_id": r.get("run_id"), "reason": "UNKNOWN_ARM_LABEL", "detail": arm})
                continue
            complete = 1 if r["complete"] == "1" else 0
            tc = r["t_complete_ms"]
            rows.append({
                "canonical_id": f"PRODUCT_NETNS::{r['run_id']}",
                "domain": "PRODUCT_NETNS",
                "source_artifact": os.path.relpath(csv_path, REPO),
                "source_commit": "c46e8fa3bf7ffdf999dc2a5b3599bb62e0d6790c",
                "source_binary_sha256": "a1113236e156fa60148e6154aa069c8702ca4e3b55f35c144aa22b3a6b33191f",
                "run_id": r["run_id"],
                "cell_id": r["cell"],
                "arm_raw": arm,
                "action": action,
                "repetition_index": r["rep"],
                # pairing unit for the product grid = cell + repetition (OFF rep i paired with ON rep i)
                "pairing_group_id": f"pg--{r['cell']}--rep{r['rep']}",
                "base_payload_bytes": int(r["w_units"]) * int(r["unit_bytes"]),
                "workload_units": r["w_units"],
                "runtime_features_present": 0,
                "runtime_features_absent_reason": ABSENT_REASON,
                "cfg_stratum": r["struct"],
                "cfg_rtt_ms": r["rtt"],
                "cfg_loss_label": r["loss"],
                "cfg_structure": r["struct"],
                "cfg_drop_count": "",
                "horizon_ms": r["ext_horizon_ms"],
                "horizon_complete": complete,
                "censored": 1 - complete,
                "completion_time_ns": (int(tc) * 1_000_000) if tc not in ("null", "") else "",
                "censor_time_ns": "" if complete else int(r["ext_horizon_ms"]) * 1_000_000,
                "deadline_ms": r["op_horizon_ms"],
                "deadline_hit": 1 if r["deadline_hit"] == "1" else 0,
                "total_wire_bytes": r["wire_bytes"],
                "arq_retransmit_bytes": "",   # product schema records count, not bytes, for ARQ retx
                "arq_retransmit_count": r["arq_retx"],
                "fec_parity_bytes": "",       # no FEC engaged in the product tail grid
                "probe_bytes": r["probe_bytes"],
                "probe_count": r["probe_sent"],
                "control_bytes": "",
                "completion_cause": r["recovery_cause"],
                "tail_status": "REAL_TAIL_ERASURE" if r["struct"] == "terminal-tail" else "NO_TAIL_ERASURE",
                "reassembly_equal": "",
                "remaining_fragments": "",
                "fec_decoded_sources": "",
                "spurious_count": r["spurious"],
                "invariant_ok": 1 if r["inv_fail"] == "0" else 0,
            })
    return n


def main():
    out_dir = os.path.join(REPO, "reports", "fs-fusion-cal-1")
    os.makedirs(out_dir, exist_ok=True)
    rows, excl = [], []
    seen = {}
    seen["FS1_IN_PROCESS"] = ingest_inprocess(
        "FS1_IN_PROCESS", os.path.join(REPO, "reports/fs-1/raw/v1.3.2/*.json"), "fs-1 v1.3.2", rows, excl)
    seen["R2A_IN_PROCESS"] = ingest_inprocess(
        "R2A_IN_PROCESS", os.path.join(REPO, "reports/dr-arq-tail-1/v1/raw/*.json"), "r2a", rows, excl)
    seen["PRODUCT_NETNS"] = ingest_product(
        os.path.join(REPO, "reports/dr-arq-product-tail-1/v1/result.csv"), rows, excl)

    rows.sort(key=lambda r: (r["domain"], r["run_id"]))
    out_csv = os.path.join(out_dir, "canonical-calibration-data.csv")
    tmp = out_csv + ".tmp"
    with open(tmp, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=CANON_COLS)
        w.writeheader()
        w.writerows(rows)
        f.flush(); os.fsync(f.fileno())
    os.replace(tmp, out_csv)
    digest = sha256_file(out_csv)
    open(out_csv + ".sha256", "w").write(f"{digest}  canonical-calibration-data.csv\n")

    by_domain = Counter(r["domain"] for r in rows)
    by_action = Counter((r["domain"], r["action"]) for r in rows)
    ledger = {
        "schema_version": SCHEMA_VERSION,
        "canonical_sha256": digest,
        "source_rows_seen": seen,
        "source_rows_total": sum(seen.values()),
        "canonical_rows": len(rows),
        "excluded_count": len(excl),
        "exclusions": excl[:200],
        "rows_by_domain": dict(by_domain),
        "rows_by_domain_action": {f"{d}|{a}": c for (d, a), c in sorted(by_action.items())},
        "runtime_features_present_rows": sum(1 for r in rows if r["runtime_features_present"] == 1),
        "runtime_features_absent_rows": sum(1 for r in rows if r["runtime_features_present"] == 0),
        "excluded_sources_never_read": [
            "reports/fs-1/raw/v1.3.1 (SUPERSEDED-INCOMPLETE-SCHEMA)",
            "commit 44e0c50 dataset (INVALIDATED arm-label bug)",
        ],
    }
    lp = os.path.join(out_dir, "ingest-ledger.json")
    with open(lp, "w") as f:
        json.dump(ledger, f, indent=2, sort_keys=True)
    print(json.dumps({k: ledger[k] for k in
                      ("source_rows_total", "canonical_rows", "excluded_count", "rows_by_domain",
                       "runtime_features_absent_rows", "canonical_sha256")}, indent=2))
    print("\nrows_by_domain_action:")
    for k, v in ledger["rows_by_domain_action"].items():
        print(f"  {k}: {v}")


if __name__ == "__main__":
    main()
