#!/usr/bin/env python3
"""DR-ARQ-PRODUCT-TAIL-1 §5/§6 — SINGLE typed experiment definition → manifest.

Emits manifest.csv: one row per result-bearing run (run_id, netem fwd/rev spec, drop_terminal, dynamic,
deadline/lb/horizon per the FROZEN deadline-feasibility formula, expected). Core grid = 36 cells × 2 arms
× 30 reps = 2160 rows. Adversarial slices appended (group=adv). The executor consumes this verbatim; the
manifest + this generator are frozen pre-outcome (§8). NO adaptive stopping / outcome-based cell change.

Frozen deadline-feasibility (TDD §3, formula UNCHANGED): workload 4848 B, pacer 0.80C=800 kbps,
src_serialize=48.5 ms; 1-tail recovery LB = serialize + 1.5·RTT; op deadline = preregistered tight;
horizon = observation window; benchmark deadline = 3×LB (driver computes from lb_ms).
"""
import sys, csv

W_UNITS, UNIT_BYTES = 48, 101           # 48×101 = 4848 B (frozen workload)
N_CORE = 30
LOSSES = [2, 5, 10, 20]
STRUCTS = ["iid", "burst", "terminal-tail"]
ARMS = ["off", "on"]
# RTT cells: (name, owd_ms, op_horizon_ms, lb_ms=ceil(48.5+1.5·RTT), ext_horizon_ms)  — frozen
RTT_CELLS = {
    "low":  (25,  199, 124, 699),    # RTT 50  : LB=123.5→124, 3×LB≈370
    "med":  (125, 599, 424, 2499),   # RTT 250 : LB=423.5→424, 3×LB≈1270
    "high": (500, 2099, 1549, 9249), # RTT 1000: LB=1548.5→1549, 3×LB≈4645
}

def base_netem(owd):
    return f"delay {owd}ms rate 1mbit"

def gemodel_p(L):   # steady-state bad fraction p/(p+r)=L/100 with r=50 → p=50L/(100-L)
    return round(50.0 * L / (100 - L), 2)

def terminal_drop(L):  # loss-label → contiguous terminal units dropped (positional; small clean tail, R2A-like)
    return {2: 1, 5: 2, 10: 3, 20: 4}[L]

def main():
    out = sys.argv[sys.argv.index("--out") + 1]
    rows = []
    cols = ["run_id","group","struct","loss","rtt","arm","rep","owd_ms","op_horizon_ms","lb_ms",
            "ext_horizon_ms","w_units","unit_bytes","netem_fwd","netem_rev","drop_terminal","dynamic"]

    # ── CORE GRID (2160) ──
    for st in STRUCTS:
        for L in LOSSES:
            for rt, (owd, op, lb, ext) in RTT_CELLS.items():
                b = base_netem(owd)
                if st == "iid":
                    fwd, rev, dt = f"{b} loss {L}%", b, 0
                elif st == "burst":
                    fwd, rev, dt = f"{b} loss gemodel {gemodel_p(L)}% 50%", b, 0
                else:  # terminal-tail
                    fwd, rev, dt = b, b, terminal_drop(L)
                for arm in ARMS:
                    for rep in range(N_CORE):
                        rid = f"core-{st}-L{L}-{rt}-{arm}-r{rep:02d}"
                        rows.append([rid,"core",st,L,rt,arm,rep,owd,op,lb,ext,W_UNITS,UNIT_BYTES,fwd,rev,dt,""])

    # ── ADVERSARIAL SLICES (§6; pre-outcome, explicit configs) ──
    def adv(name, rtt_cell, fwd, rev, dt, reps, dynamic="", loss=0):
        owd, op, lb, ext = RTT_CELLS[rtt_cell]
        for arm in ARMS:
            for rep in range(reps):
                rid = f"adv-{name}-{arm}-r{rep:02d}"
                rows.append([rid,"adv",name,loss,rtt_cell,arm,rep,owd,op,lb,ext,W_UNITS,UNIT_BYTES,fwd,rev,dt,dynamic])

    adv("noloss",      "low", "delay 25ms rate 1mbit",  "delay 25ms rate 1mbit",  0, 10)
    adv("reorder",     "med", "delay 125ms reorder 25% 50% rate 1mbit", "delay 125ms rate 1mbit", 0, 10)
    adv("delayedack",  "low", "delay 25ms rate 1mbit",  "delay 150ms rate 1mbit", 0, 10)
    adv("probeloss",   "med", "delay 125ms rate 1mbit loss 40%", "delay 125ms rate 1mbit", 1, 10, loss=40)
    adv("jitter",      "med", "delay 125ms 60ms distribution normal rate 1mbit", "delay 125ms rate 1mbit", 1, 10)
    adv("pathpause",   "med", "delay 125ms rate 1mbit", "delay 125ms rate 1mbit", 1, 5, dynamic="pause")
    adv("rttstepup",   "high","delay 25ms rate 1mbit",  "delay 25ms rate 1mbit",  1, 5, dynamic="stepup")
    adv("rttstepdown", "high","delay 500ms rate 1mbit", "delay 500ms rate 1mbit", 1, 5, dynamic="stepdown")

    with open(out, "w", newline="") as f:
        w = csv.writer(f); w.writerow(cols); w.writerows(rows)

    core = sum(1 for r in rows if r[1] == "core")
    advn = sum(1 for r in rows if r[1] == "adv")
    print(f"manifest rows: {len(rows)} (core={core} [expect 2160], adv={advn})")
    assert core == 2160, f"core count {core} != 2160"
    # expected-count summary per group
    from collections import Counter
    c = Counter((r[2],) for r in rows if r[1]=="core")
    print("core per-struct:", dict((k[0],v) for k,v in sorted(c.items())))

if __name__ == "__main__":
    main()
