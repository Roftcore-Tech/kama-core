#!/usr/bin/env python3
"""Verifier: (1) spurious-retransmit candidates (ACK of a transmission arriving at client AFTER the
unit's next transmission) => possible note_spurious => K>3; (2) pending_lost>0 during wait windows
(ts.csv) for post-ev3 events. Usage: python3 -I vk.py <cell> <arm>"""
import csv, json, pathlib, re, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import vretx as V

def run(rep):
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep / "cmd.txt").read_text()).group(1))
    meta = json.loads((rep / "out.jsonl").open().readline())["meta"]
    t0 = meta["t0_unix_ns"]
    cli = V.rows_of(rep / "capb_cli.csv.gz")
    tx = []
    for t, d, sp, dp, x in cli:
        if d != "o" or dp != port or not x: continue
        fr, tag = V.split(x)
        if fr and fr[0] in (0, 5) and len(fr) >= 11 and len(tag) >= 16:
            tx.append((t, int.from_bytes(tag[:8], "big"), int.from_bytes(fr[1:3], "big"), int.from_bytes(fr[3:11], "big")))
    pn16 = {pn & 0xFFFF: (s, t, u) for t, pn, s, u in tx}
    first_ack = {}
    run_max = -1; la_at = []
    for t, d, sp, dp, x in cli:
        if d != "i" or sp != port or not x: continue
        fr, tag = V.split(x)
        a = V.ack_pns16(fr)
        if a is None: continue
        for p in a:
            if p in pn16:
                s = pn16[p][0]
                first_ack.setdefault(s, t)
                run_max = max(run_max, s)
        la_at.append((t, run_max))
    by_unit = {}
    for r in tx: by_unit.setdefault(r[3], []).append(r)
    spur = []
    for u, lst in by_unit.items():
        for i in range(len(lst) - 1):
            t, pn, s, _ = lst[i]
            fa = first_ack.get(s)
            if fa is not None and fa > lst[i + 1][0]:
                la = max((m for ta, m in la_at if ta <= fa), default=s)
                spur.append(la - s)
    ts = []
    p = rep / "ts.csv"
    if p.exists():
        with p.open() as f:
            for row in csv.DictReader(f):
                ts.append((t0 + int(row["t_ms"]) * 1_000_000, int(row["pending_lost"])))
    return spur, ts

def main():
    cell = pathlib.Path(sys.argv[1]); arm = sys.argv[2]
    reps = sorted((cell / arm).glob("rep*"), key=lambda p: int(p.name[3:]))
    nrep_sp = 0; allsp = []; cover = []; 
    for rep in reps:
        sp, ts = run(rep)
        if sp: nrep_sp += 1
        allsp += sp
        ev, _, _ = V.analyze(rep)
        for r in ev:
            if "wait" not in r or r["wait"] < 0.3: continue
            lo, hi = r["ev3"] + 150_000_000, r["nxt"] - 60_000_000
            w = [pl for tt, pl in ts if lo <= tt <= hi]
            if w: cover.append(sum(1 for pl in w if pl > 0) / len(w))
    print(f"== {cell.name}/{arm}: reps_with_spurious_candidate={nrep_sp}/{len(reps)} n_cand={len(allsp)} reorder_dist max={max(allsp) if allsp else None} sorted={sorted(allsp)[-10:]}")
    if cover:
        cover.sort()
        print(f"  post_ev3 waits>=0.3s with ts samples: n={len(cover)} frac_samples_pending_lost>0: p10={cover[len(cover)//10]:.2f} p50={cover[len(cover)//2]:.2f} min={cover[0]:.2f} all==1: {sum(1 for c in cover if c==1.0)}")
main()
