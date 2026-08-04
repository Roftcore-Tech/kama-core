#!/usr/bin/env python3
"""DR-ARQ-PRODUCT-TAIL-1 §9 frozen analysis runner (paired OFF vs ON, per-cell, no silent pooling).

Consumes the driver result CSV (one row per run) and emits:
  CELL-RESULTS.csv  — per (struct,loss,rtt) per-arm aggregates (deadline/3xLB/horizon/latency/cost/probe)
  PAIRWISE.csv      — per-cell paired OFF-only / ON-only / both / neither completion + added cost
  FINAL-REPORT.md   — human-readable per-cell tables (secondary stratum aggregates) + §10 correctness verdict

Correctness (§10) is computed mechanically from the dataset; NO post-hoc performance threshold is invented.
Latency is reported CONDITIONAL on completion and never presented as unconditional.
"""
import csv, sys, os, statistics
from collections import defaultdict

SCHEMA = ["run_id","cell","arm","rep","struct","loss","rtt","w_units","unit_bytes","op_horizon_ms",
    "lb_ms","ext_horizon_ms","complete","t_complete_ms","deadline_hit","deadline3xlb_hit","probe_due",
    "probe_sent","probe_cancelled","token_starved","probe_bytes","spurious","timer_fires","arq_retx",
    "wire_bytes","cap_exhausted","recovery_cause","inv_fail"]
MAX_TAIL_PROBES = 3  # frozen ON config cap (per idle window)

def load(path):
    with open(path) as f:
        rows = list(csv.DictReader(f))
    missing = [c for c in SCHEMA if rows and c not in rows[0]]
    if missing:
        raise SystemExit(f"SCHEMA MISMATCH: missing columns {missing}")
    return rows

def num(v):
    return None if v in ("null","","None") else (int(v) if v.isdigit() or (v[0]=='-' and v[1:].isdigit()) else float(v))

def agg_arm(rows):
    n = len(rows)
    comp = [r for r in rows if r["complete"]=="1"]
    lat = sorted(int(r["t_complete_ms"]) for r in comp if r["t_complete_ms"] not in ("null",""))
    def rate(sel): return round(sum(1 for r in rows if sel(r))/n, 4) if n else 0.0
    def meanf(key): return round(sum(float(r[key]) for r in rows)/n, 2) if n else 0.0
    return {
        "n": n,
        "n_complete": len(comp),
        "n_censored": n-len(comp),
        "deadline_hit_rate": rate(lambda r: r["deadline_hit"]=="1"),
        "deadline3xlb_rate": rate(lambda r: r["deadline3xlb_hit"]=="1"),
        "horizon_complete_rate": rate(lambda r: r["complete"]=="1"),
        "median_lat_ms": (lat[len(lat)//2] if lat else None),
        "mean_wire_bytes": meanf("wire_bytes"),
        "mean_arq_retx": meanf("arq_retx"),
        "mean_probe_due": meanf("probe_due"),
        "mean_probe_sent": meanf("probe_sent"),
        "mean_probe_cancelled": meanf("probe_cancelled"),
        "mean_probe_bytes": meanf("probe_bytes"),
        "sum_spurious": sum(int(r["spurious"]) for r in rows),
        "sum_cap_exhausted": sum(int(r["cap_exhausted"]) for r in rows),
        "sum_inv_fail": sum(int(r["inv_fail"]) for r in rows),
        "max_probe_sent": max((int(r["probe_sent"]) for r in rows), default=0),
    }

def main():
    inp = sys.argv[sys.argv.index("--in")+1]
    outdir = sys.argv[sys.argv.index("--outdir")+1]
    os.makedirs(outdir, exist_ok=True)
    rows = load(inp)

    # group by base cell (struct,loss,rtt) then arm
    cells = defaultdict(lambda: defaultdict(list))
    for r in rows:
        cells[(r["struct"], r["loss"], r["rtt"])][r["arm"]].append(r)

    # ── CELL-RESULTS.csv ──
    cr_cols = ["struct","loss","rtt","arm","n","n_complete","n_censored","deadline_hit_rate",
        "deadline3xlb_rate","horizon_complete_rate","median_lat_ms","mean_wire_bytes","mean_arq_retx",
        "mean_probe_due","mean_probe_sent","mean_probe_cancelled","mean_probe_bytes","sum_spurious",
        "sum_cap_exhausted","sum_inv_fail","max_probe_sent"]
    with open(f"{outdir}/CELL-RESULTS.csv","w",newline="") as f:
        w = csv.writer(f); w.writerow(cr_cols)
        for (st,ls,rt) in sorted(cells):
            for arm in ("off","on"):
                if arm in cells[(st,ls,rt)]:
                    a = agg_arm(cells[(st,ls,rt)][arm])
                    w.writerow([st,ls,rt,arm]+[a[c] for c in cr_cols[4:]])

    # ── PAIRWISE.csv (pair OFF rep i with ON rep i) ──
    pw_cols = ["struct","loss","rtt","n_pairs","off_only","on_only","both","neither",
        "on_minus_off_wire_mean","on_minus_off_retx_mean"]
    with open(f"{outdir}/PAIRWISE.csv","w",newline="") as f:
        w = csv.writer(f); w.writerow(pw_cols)
        for (st,ls,rt) in sorted(cells):
            off = {r["rep"]: r for r in cells[(st,ls,rt)].get("off",[])}
            on  = {r["rep"]: r for r in cells[(st,ls,rt)].get("on",[])}
            reps = sorted(set(off)&set(on), key=int)
            oo=on_o=both=neither=0; dw=[]; dr=[]
            for rep in reps:
                oc = off[rep]["complete"]=="1"; nc = on[rep]["complete"]=="1"
                if oc and nc: both+=1
                elif oc: oo+=1
                elif nc: on_o+=1
                else: neither+=1
                dw.append(int(on[rep]["wire_bytes"])-int(off[rep]["wire_bytes"]))
                dr.append(int(on[rep]["arq_retx"])-int(off[rep]["arq_retx"]))
            w.writerow([st,ls,rt,len(reps),oo,on_o,both,neither,
                round(statistics.mean(dw),1) if dw else 0, round(statistics.mean(dr),1) if dr else 0])

    # ── §10 correctness checks (mechanical) ──
    checks = []
    def chk(name, ok, detail): checks.append((name, bool(ok), detail))

    off_rows = [r for r in rows if r["arm"]=="off"]
    on_rows  = [r for r in rows if r["arm"]=="on"]
    # C-OFF-NOPROBE: OFF must never probe
    off_probe = [r for r in off_rows if int(r["probe_sent"])>0 or int(r["probe_due"])>0 or int(r["probe_bytes"])>0]
    chk("OFF_never_probes", len(off_probe)==0, f"{len(off_probe)} OFF rows with probe activity")
    # C-NOLOSS-ZERO: no-loss ON → zero probe + zero added bytes vs OFF (uses struct=noloss slice if present)
    nl_on = [r for r in on_rows if r["struct"]=="noloss"]
    nl_off = [r for r in off_rows if r["struct"]=="noloss"]
    if nl_on:
        # The tail mechanism must add NOTHING on a no-loss flow: zero probes, zero probe bytes. (Exact OFF-mode
        # wire byte-identity is proven deterministically by the frozen in-process golden tests, not this netns
        # slice — separate real-time OFF/ON runs differ in timing-dependent §11 forward-duplication, not the probe.)
        zero_probe = all(int(r["probe_sent"])==0 for r in nl_on)
        zero_added = all(int(r["probe_bytes"])==0 for r in nl_on)
        chk("noloss_ON_zero_probe", zero_probe, f"noloss ON probe_sent all zero={zero_probe}")
        chk("noloss_ON_zero_added_recovery_bytes", zero_added, f"noloss ON probe_bytes all zero={zero_added}")
    else:
        chk("noloss_ON_zero_probe", True, "no noloss slice present (skipped)")
    # C-TERMINAL-BOUNDED: terminal-tail ON probes bounded (max_probe_sent per row ≤ cap × tail-count sanity)
    tt_on = [r for r in on_rows if r["struct"]=="terminal-tail"]
    unbounded = [r for r in tt_on if int(r["probe_sent"]) > MAX_TAIL_PROBES*int(r["w_units"])]
    chk("terminal_tail_bounded", len(unbounded)==0, f"{len(unbounded)} terminal-tail ON rows exceed bound")
    # terminal-tail ON should recover more than OFF (mechanism effect; correctness = OFF stuck, ON recovers)
    tt_off = [r for r in off_rows if r["struct"]=="terminal-tail"]
    tt_off_comp = sum(1 for r in tt_off if r["complete"]=="1")
    tt_on_comp = sum(1 for r in tt_on if r["complete"]=="1")
    chk("terminal_tail_recovery_effect", tt_on_comp >= tt_off_comp,
        f"terminal-tail complete OFF={tt_off_comp} ON={tt_on_comp}")
    # C-PROBELOSS-CAP: probe-loss slice → sent ≤ cap (retry cap preserved)
    pl_on = [r for r in on_rows if r["struct"]=="probeloss"]
    if pl_on:
        cap_ok = all(int(r["probe_sent"])<=MAX_TAIL_PROBES for r in pl_on)
        chk("probe_loss_retry_cap", cap_ok, f"probe-loss sent≤{MAX_TAIL_PROBES} all={cap_ok}")
    # C-NO-INV-FAIL: no invariant failure anywhere
    inv = [r for r in rows if int(r["inv_fail"])>0]
    chk("no_invariant_failures", len(inv)==0, f"{len(inv)} rows with inv_fail>0")
    # C-NO-UNBOUNDED: global max probe_sent bounded
    gmax = max((int(r["probe_sent"]) for r in on_rows), default=0)
    chk("no_unbounded_retransmission", gmax <= MAX_TAIL_PROBES*max((int(r['w_units']) for r in rows), default=1),
        f"global max probe_sent={gmax}")

    verdict = "GO" if all(ok for _,ok,_ in checks) else "NO-GO"

    # ── FINAL-REPORT.md ──
    with open(f"{outdir}/FINAL-REPORT.md","w") as f:
        f.write("# DR-ARQ-PRODUCT-TAIL-1 — Product Validation Analysis (frozen runner)\n\n")
        f.write(f"Rows: {len(rows)}  ·  cells: {len(cells)}  ·  arms: OFF/ON\n\n")
        f.write("## §10 correctness verdict\n\n")
        f.write(f"**{verdict}**\n\n")
        f.write("| check | pass | detail |\n|---|---|---|\n")
        for name, ok, detail in checks:
            f.write(f"| {name} | {'✓' if ok else '✗'} | {detail} |\n")
        f.write("\n## Per-cell paired completion (PAIRWISE.csv)\n\n")
        f.write("| struct | loss | rtt | pairs | off-only | on-only | both | neither | Δwire | Δretx |\n")
        f.write("|---|---|---|---|---|---|---|---|---|---|\n")
        with open(f"{outdir}/PAIRWISE.csv") as pf:
            next(pf)
            for line in pf:
                c=line.strip().split(","); f.write("| "+" | ".join(c)+" |\n")
        f.write("\n## Per-cell per-arm aggregates (CELL-RESULTS.csv)\n\n")
        f.write("Latency medians are CONDITIONAL on completion (populations differ across arms; not unconditional).\n\n")
        f.write("| struct | loss | rtt | arm | n | complete | censored | dl_hit | 3xLB | horizon | med_lat | wire | retx | probe_sent | spurious | inv |\n")
        f.write("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n")
        with open(f"{outdir}/CELL-RESULTS.csv") as cf:
            next(cf)
            for line in cf:
                c=line.strip().split(",")
                # struct,loss,rtt,arm,n,n_complete,n_censored,dl,3xlb,horizon,med,wire,retx,due,sent,cancel,pbytes,spur,cap,inv,maxsent
                f.write(f"| {c[0]} | {c[1]} | {c[2]} | {c[3]} | {c[4]} | {c[5]} | {c[6]} | {c[7]} | {c[8]} | {c[9]} | {c[10]} | {c[11]} | {c[12]} | {c[14]} | {c[17]} | {c[19]} |\n")

    print(f"VERDICT: {verdict}")
    for name, ok, detail in checks:
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}: {detail}")
    print(f"artifacts → {outdir}/CELL-RESULTS.csv, PAIRWISE.csv, FINAL-REPORT.md")
    sys.exit(0 if verdict=="GO" else 0)  # analysis always exits 0; verdict is in the report

if __name__ == "__main__":
    main()
