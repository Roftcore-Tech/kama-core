#!/usr/bin/env python3
"""W-2B-R3 analiz: 54-koşu marker'larından per-cell ON/OFF verdict (Abkarada sınıfları).
Kullanım: analyze_w2b_r3.py <DATA_DIR>"""
import sys, os, re, statistics

D = sys.argv[1]

def read_marker(path):
    d = {}
    if not os.path.exists(path):
        return None
    for line in open(path):
        if "=" in line:
            k, _, v = line.strip().partition("=")
            d[k] = v
    return d

def read_rtt(path):
    d = {}
    if os.path.exists(path):
        for tok in open(path).read().split():
            if "=" in tok:
                k, _, v = tok.partition("=")
                d[k] = v
    return d

def ms(v):
    if v is None or v == "none":
        return None
    m = re.match(r"(\d+)ms", v)
    return int(m.group(1)) if m else None

RTTS = [100, 250, 1000]
SIZES = [65, 300, 1200]
FEATS = ["on", "off"]
REPS = [1, 2, 3]

def cell_runs(rtt, sz, feat):
    out = []
    for rep in REPS:
        rid = f"r3-{rtt}-{sz}-{feat}-r{rep}"
        mk = read_marker(f"{D}/{rid}-rxdone.marker")
        rt = read_rtt(f"{D}/{rid}-rtt.txt")
        if mk is None:
            out.append(None); continue
        recv = int(mk.get("receiver_events", "0"))
        fin = int(mk.get("finalized_blocks", "0"))
        comp = ms(mk.get("w2b_completion_time"))
        trig = mk.get("w2b_completion_trigger_kind", "?")
        recp = mk.get("w2b_recovery_path", "?")
        ratio = mk.get("w2b_workload_to_initial_cwnd_ratio", "?")
        ffb = mk.get("w2b_first_flight_burst_units", "?")
        hardcap = mk.get("w2b_ack_cap_hard_hit", "false")
        appdup = int(mk.get("w2b_application_duplicate_delivery_count", "0"))
        tolp50 = rt.get("rtt_tol_p50", "?"); tolp95 = rt.get("rtt_tol_p95", "?")
        invalid = (recv == 0) or (hardcap == "true") or (tolp50 == "FAIL") or (tolp95 == "FAIL") or (appdup != 0)
        out.append(dict(rep=rep, recv=recv, fin=fin, comp=comp, trig=trig, recp=recp,
                        ratio=ratio, ffb=ffb, hardcap=hardcap, appdup=appdup,
                        tolp50=tolp50, tolp95=tolp95, invalid=invalid))
    return out

def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None

print("### W-2B-R3 PER-CELL VERDICT (ON vs OFF; Abkarada sınıfları)\n")
print("prov:", (open(f"{D}/R3-PROVENANCE.txt").read().strip().replace("\n", " | ") if os.path.exists(f"{D}/R3-PROVENANCE.txt") else "?"))
print()
hdr = "RTT  size  ONfin OFFfin  ON_comp_med   OFF_comp_med   trig(ON/OFF)          ratio  ff  VERDICT"
print(hdr); print("-"*len(hdr))
verdicts = {}
any_invalid = False
for rtt in RTTS:
    for sz in SIZES:
        on = cell_runs(rtt, sz, "on"); off = cell_runs(rtt, sz, "off")
        on_v = [r for r in on if r]; off_v = [r for r in off if r]
        inv = any(r and r["invalid"] for r in on+off) or len(on_v) < 3 or len(off_v) < 3
        on_fin = sum(1 for r in on_v if r["fin"] >= 1)
        off_fin = sum(1 for r in off_v if r["fin"] >= 1)
        on_comp = med([r["comp"] for r in on_v if r["fin"] >= 1])
        off_comp = med([r["comp"] for r in off_v if r["fin"] >= 1])
        on_trig = on_v[0]["trig"] if on_v else "?"
        off_trig = off_v[0]["trig"] if off_v else "?"
        ratio = on_v[0]["ratio"] if on_v else "?"
        ff = on_v[0]["ffb"] if on_v else "?"
        if inv:
            v = "INVALID — NO EVIDENCE"; any_invalid = True
        elif on_fin == 0 and off_fin == 0:
            v = f"NO_COMPLETION_WITHIN_HORIZON"
        elif on_fin >= 2 and off_fin == 0:
            v = "DECISIVE (ON completes, OFF NoCompletion)"
        elif off_fin >= 2 and on_fin == 0:
            v = "OFF-ONLY (unexpected — investigate)"
        else:
            # both complete → compare median completion
            if on_comp is not None and off_comp is not None:
                lo, hi = min(on_comp, off_comp), max(on_comp, off_comp)
                rel = (hi - lo) / hi if hi else 0
                if rel < 0.15:
                    v = f"NEUTRAL (ON≈OFF; Δ={hi-lo}ms, {rel*100:.0f}%)"
                elif on_comp < off_comp:
                    v = f"BENEFICIAL (ON<OFF by {off_comp-on_comp}ms)"
                else:
                    v = f"OFF-FASTER by {on_comp-off_comp}ms (unexpected)"
            else:
                v = "MIXED (partial completion)"
        verdicts[(rtt, sz)] = v
        oc = f"{on_comp}ms" if on_comp is not None else "—"
        fc = f"{off_comp}ms" if off_comp is not None else "—"
        print(f"{rtt:<4} {sz:<5} {on_fin}/3   {off_fin}/3    {oc:<12} {fc:<13} {on_trig[:10]}/{off_trig[:10]:<10} {ratio:<6} {ff}  {v}")

print()
print("### ÖZET")
from collections import Counter
cnt = Counter(v.split(" ")[0].split("(")[0] for v in verdicts.values())
print("verdict-dağılımı:", dict(cnt))
print("acceptance:", "TÜM-VALID" if not any_invalid else "INVALID-VAR (rerun gerekli)")
