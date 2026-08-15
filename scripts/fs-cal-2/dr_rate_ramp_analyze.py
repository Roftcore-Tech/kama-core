#!/usr/bin/env python3
"""DR-RATE-RAMP-1 — mechanical rate-lock classification from the update-equation chain.

The lock predicate is NOT a post-hoc speed threshold. It is derived from the sender's own chain:

    valid delivery sample produced   -> bw_offered increases after PreConverge exit
    estimator accepted / rejected    -> bw_accepted vs bw_rej_{invalid,interval,app_limited}
    new bandwidth state              -> btlbw_bps
    applied to the pacer             -> send_rate_bps  (and set_burst_cap -> burst_cap)
    token refill / emission changes  -> burst_cap, tokens, wire emissions, pif

MECHANICAL LOCK PREDICATE
    LOCKED := the flow LEFT PreConverge, yet `burst_cap` never rose above ONE sealed unit
              (`max(burst_cap) == SEALED`). `burst_cap` is the exact variable written by
              `TokenBucket::set_burst_cap(max(1 unit, rate x window))`, so this states
              "the channel-derived burst clamp never released past its 1-unit floor".

    Corroborating (recorded, not part of the predicate): btlbw settles at ~ one sealed unit per
    RTT, i.e. the estimator's fixed point is the sender's own paced output, not the path.

Namespaces: DR-RATE-RAMP-1-HIGHLOSS-V1 (rr2), DR-RATE-RAMP-1-LOWLOSS-V1 (rr3), rr1 = replay.
fit_use = FORBIDDEN, calibration_use = FORBIDDEN, result_bearing = false.
"""
import glob, hashlib, json, os, statistics, sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BASE = os.path.join(REPO, "reports", "fs-cal-2", "v3.2", "rate-ramp")
OUT = os.path.join(BASE, "DR-RATE-RAMP-1-ROOTCAUSE-v1.json")
CSV = os.path.join(BASE, "DR-RATE-RAMP-1-CELL-RESULTS-v1.csv")
SEALED = 101
# sample column order (must match spine::DiagSample serialization)
COLS = ["t_ns", "state", "send_rate_bps", "btlbw_bps", "srtt_us", "rttvar_ns", "inflight_bytes",
        "fate_known", "telemetry_age_ns", "delivered_src", "tx_data", "tokens", "burst_cap",
        "bw_offered", "bw_accepted", "bw_rej_invalid", "bw_rej_interval", "bw_rej_app_limited",
        "send_rate_gain_permille", "pif"]
IX = {c: i for i, c in enumerate(COLS)}


def load(layer):
    out = []
    for p in sorted(glob.glob(os.path.join(BASE, layer, "*.json"))):
        if p.endswith(".sha256"):
            continue
        out.append(json.load(open(p)))
    return out


def analyze(r):
    s = r.get("samples") or []
    if not s:
        return None
    g = lambda row, c: row[IX[c]]
    pc = r["preconverge_exit_ns"]
    post = [x for x in s if pc is not None and g(x, "t_ns") >= pc]
    last = s[-1]
    max_cap = max(g(x, "burst_cap") for x in s)
    max_pif = max(g(x, "pif") for x in s)
    max_btl = max(g(x, "btlbw_bps") for x in s)
    max_rate = max(g(x, "send_rate_bps") for x in s)
    srtt_ns = g(last, "srtt_us") * 1000
    one_unit_per_rtt = (SEALED * 1_000_000_000 // srtt_ns) if srtt_ns else 0
    left_pc = pc is not None
    locked = bool(left_pc and max_cap == SEALED)
    # chain-step observations
    offered = g(last, "bw_offered")
    accepted = g(last, "bw_accepted")
    rej_inv = g(last, "bw_rej_invalid")
    rej_int = g(last, "bw_rej_interval")
    rej_al = g(last, "bw_rej_app_limited")
    return {
        "rep": r["repetition_index"], "layer": r["layer"],
        "left_preconverge": left_pc,
        "preconverge_exit_ms": None if pc is None else round(pc / 1e6),
        "LOCKED": locked,
        "max_burst_cap": max_cap, "max_pif": max_pif,
        "max_btlbw_bps": max_btl, "max_send_rate_bps": max_rate,
        "final_btlbw_bps": g(last, "btlbw_bps"), "final_send_rate_bps": g(last, "send_rate_bps"),
        "one_unit_per_rtt_bps": one_unit_per_rtt,
        "btlbw_equals_one_unit_per_rtt": bool(max_btl and abs(max_btl - one_unit_per_rtt) <= 2),
        "bw_offered": offered, "bw_accepted": accepted,
        "bw_rej_invalid": rej_inv, "bw_rej_interval": rej_int, "bw_rej_app_limited": rej_al,
        "samples_after_pc_exit": len(post),
        "tx_data": r["tx_data"], "ack_batches": r["ack_batches"],
        "send_rate_gain_permille": g(last, "send_rate_gain_permille"),
        "epoch_trigger": r["epoch_trigger"],
        # per-class chain step status
        "chain": {
            "sample_produced": offered > 0,
            "sample_accepted": accepted > 0,
            "estimator_moved_off_zero": max_btl > 0,
            "pacer_tracks_estimator": max_rate >= max_btl,  # rate = gain x btlbw, gain >= 1
            "burst_clamp_released": max_cap > SEALED,
        },
    }


def classify(rows):
    """Assign a root-cause class using the chain, not a speed threshold."""
    locked = [x for x in rows if x["LOCKED"]]
    if not locked:
        return "NO-LOCK-OBSERVED", {}
    # which chain step is broken across the locked population?
    no_sample = [x for x in locked if not x["chain"]["sample_produced"]]
    rejected = [x for x in locked if x["chain"]["sample_produced"] and not x["chain"]["sample_accepted"]]
    not_reseeded = [x for x in locked if x["chain"]["sample_accepted"]
                    and x["btlbw_equals_one_unit_per_rtt"]]
    pacer_stuck = [x for x in locked if x["chain"]["estimator_moved_off_zero"]
                   and not x["chain"]["pacer_tracks_estimator"]]
    token = [x for x in locked if not x["chain"]["burst_clamp_released"]]
    detail = {
        "locked_runs": len(locked),
        "RAMP_NO_SAMPLE": len(no_sample),
        "RAMP_SAMPLE_REJECTED": len(rejected),
        "estimator_fixed_point_is_one_unit_per_rtt": len(not_reseeded),
        "RAMP_PACER_NOT_UPDATED": len(pacer_stuck),
        "burst_clamp_never_released": len(token),
    }
    if no_sample:
        return "RAMP-NO-SAMPLE", detail
    if rejected:
        return "RAMP-SAMPLE-REJECTED", detail
    if pacer_stuck:
        return "RAMP-PACER-NOT-UPDATED", detail
    if not_reseeded and token:
        return "RAMP-ESTIMATOR-NOT-RESEEDED + RAMP-TOKEN-DEADLOCK (coupled)", detail
    if token:
        return "RAMP-TOKEN-DEADLOCK", detail
    return "RAMP-AMBIGUOUS", detail


def main():
    res = {
        "study": "DR-RATE-RAMP-1",
        "fit_use": "FORBIDDEN", "calibration_use": "FORBIDDEN", "result_bearing": False,
        "cold_start_ladder": "OFF (baseline product build)",
        "lock_predicate": "left PreConverge AND max(burst_cap) == SEALED(101) "
                          "[burst_cap is the exact variable written by TokenBucket::set_burst_cap]",
    }
    rows_all = []
    for layer, label in (("rr1", "replay"), ("rr2", "high-loss GE-L10 p=20% RTT=1000"),
                         ("rr3", "low-loss control IID p=5% RTT=1000")):
        recs = load(layer)
        rows = [a for a in (analyze(r) for r in recs) if a]
        rows_all += [dict(r, layer=layer) for r in rows]
        cls, det = classify(rows)
        n = len(rows)
        res[layer] = {
            "condition": label, "n": n,
            "locked": sum(1 for x in rows if x["LOCKED"]),
            "lock_fraction": round(sum(1 for x in rows if x["LOCKED"]) / n, 4) if n else None,
            "left_preconverge": sum(1 for x in rows if x["left_preconverge"]),
            "class": cls, "detail": det,
            "median_max_btlbw_locked": statistics.median(
                [x["max_btlbw_bps"] for x in rows if x["LOCKED"]]) if any(x["LOCKED"] for x in rows) else None,
            "median_max_btlbw_healthy": statistics.median(
                [x["max_btlbw_bps"] for x in rows if not x["LOCKED"]]) if any(not x["LOCKED"] for x in rows) else None,
            "runs": rows,
        }
    hi, lo = res.get("rr2", {}), res.get("rr3", {})
    hi_lock = hi.get("lock_fraction") or 0
    lo_lock = lo.get("lock_fraction") or 0
    if hi_lock > 0 and lo_lock == 0:
        cond = "RAMP-LOSS-CONDITIONAL"
        why = ("The lock reproduces in the high-loss cell but the mechanical chain completes correctly "
               "in the low-loss control ⇒ the defect is conditional on sustained burst loss, not a "
               "structural break of the sample→estimator→pacer chain.")
    elif hi_lock > 0 and lo_lock > 0:
        cond = "RAMP-STRUCTURAL"
        why = ("The same chain failure appears in the low-loss control ⇒ structural, not loss-conditional.")
    elif hi_lock == 0 and lo_lock == 0:
        cond = "NO-LOCK-OBSERVED-IN-FRESH-CELLS"
        why = "No fresh run met the mechanical lock predicate; only the replay reproduced it."
    else:
        cond = "RAMP-AMBIGUOUS"
        why = "Lock appears only in the low-loss control — unexpected; needs a dedicated study."
    res["CONDITIONALITY"] = cond
    res["CONDITIONALITY_REASON"] = why
    res["ROOT_CAUSE_CLASS"] = res.get("rr2", {}).get("class") or res.get("rr1", {}).get("class")

    os.makedirs(BASE, exist_ok=True)
    body = json.dumps(res, indent=2, sort_keys=True)
    open(OUT, "w").write(body + "\n")
    open(OUT + ".sha256", "w").write(
        hashlib.sha256((body + "\n").encode()).hexdigest() + "  DR-RATE-RAMP-1-ROOTCAUSE-v1.json\n")
    hdr = ["layer", "rep", "LOCKED", "left_preconverge", "preconverge_exit_ms", "max_burst_cap",
           "max_pif", "max_btlbw_bps", "max_send_rate_bps", "one_unit_per_rtt_bps",
           "btlbw_equals_one_unit_per_rtt", "bw_offered", "bw_accepted", "bw_rej_invalid",
           "bw_rej_interval", "bw_rej_app_limited", "tx_data", "ack_batches", "epoch_trigger"]
    with open(CSV, "w") as f:
        f.write(",".join(hdr) + "\n")
        for r in rows_all:
            f.write(",".join(str(r.get(h, "")) for h in hdr) + "\n")
    open(CSV + ".sha256", "w").write(
        hashlib.sha256(open(CSV, "rb").read()).hexdigest() + "  DR-RATE-RAMP-1-CELL-RESULTS-v1.csv\n")

    for layer in ("rr1", "rr2", "rr3"):
        e = res.get(layer)
        if not e:
            continue
        print(f"{layer} ({e['condition']}): n={e['n']} locked={e['locked']} "
              f"({e['lock_fraction']}) class={e['class']}")
        print(f"     btlbw median locked={e['median_max_btlbw_locked']} healthy={e['median_max_btlbw_healthy']}")
        print(f"     detail={e['detail']}")
    print("\nCONDITIONALITY:", res["CONDITIONALITY"])
    print("ROOT_CAUSE_CLASS:", res["ROOT_CAUSE_CLASS"])
    print("written:", OUT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
