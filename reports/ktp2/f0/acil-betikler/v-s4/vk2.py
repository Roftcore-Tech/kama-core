#!/usr/bin/env python3
"""Verifier: adaptive-K sensitivity. Candidate kol-2 events = transmissions whose first ACK reaches the
client AFTER the unit's next transmission, with that next transmission after left_preconverge (PreConverge
copies keep the original inflight -> no note_spurious). dist = la(before that ACK) - seq; K := max(3, 2*maxdist)
from that ACK on (ρ=2/1, arq.rs fixture). Recompute wait from evK. Usage: vk2.py <cell> <arm>"""
import json, pathlib, re, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import vretx as V

def q(v, p):
    v = sorted(v); return v[max(0, min(len(v)-1, int(round(p*(len(v)-1)))))] if v else float('nan')

def analyze(rep):
    port = int(re.search(r"--bind\s+\S+:(\d+)", (rep / "cmd.txt").read_text()).group(1))
    meta = json.loads((rep / "out.jsonl").open().readline())["meta"]
    t0 = meta["t0_unix_ns"]; lpc = meta["activation"]["left_preconverge_ns"]; lpc_abs = t0 + lpc
    cli = V.rows_of(rep / "capb_cli.csv.gz"); srv = V.rows_of(rep / "capb_srv.csv.gz")
    tx = []
    for t, d, sp, dp, x in cli:
        if d != "o" or dp != port or not x: continue
        fr, tag = V.split(x)
        if fr and fr[0] in (0, 5) and len(fr) >= 11 and len(tag) >= 16:
            tx.append((t, int.from_bytes(tag[:8], "big"), int.from_bytes(fr[1:3], "big"), int.from_bytes(fr[3:11], "big")))
    arrived = set()
    for t, d, sp, dp, x in srv:
        if d == "i" and dp == port and x:
            fr, tag = V.split(x)
            if fr and fr[0] in (0, 5): arrived.add(int.from_bytes(tag[:8], "big"))
    pn16 = {pn & 0xFFFF: s for t, pn, s, u in tx}
    acks = []  # (t, set seqs)
    for t, d, sp, dp, x in cli:
        if d != "i" or sp != port or not x: continue
        fr, tag = V.split(x); a = V.ack_pns16(fr)
        if a is None: continue
        acks.append((t, {pn16[p] for p in a if p in pn16}))
    by_unit = {}
    for r in tx: by_unit.setdefault(r[3], []).append(r)
    nxt_of = {}
    for u, lst in by_unit.items():
        for i in range(len(lst)-1): nxt_of[lst[i][2]] = lst[i+1][0]
    seen = set(); la = -1; K = 3; runK = []  # (t, la_after, K_after)
    kraise = []
    for t, ss in acks:
        la_before = la
        for s in sorted(ss):
            if s in seen: continue
            seen.add(s)
            nx = nxt_of.get(s)
            if nx is not None and nx < t and nx >= lpc_abs and la_before >= 0:
                d = la_before - s
                if 2*d > K: K = 2*d; kraise.append(((t-t0)/1e9, d))
        if ss: la = max(la, max(ss))
        runK.append((t, la, K))
    waits3, waitsK, shift = [], [], 0
    for u, lst in by_unit.items():
        for i in range(len(lst)-1):
            t, pn, s, _ = lst[i]
            if pn in arrived: continue
            nx = lst[i+1][0]
            e3 = next((ta for ta, m, k in runK if ta >= t and m >= s+3), None)
            eK = next((ta for ta, m, k in runK if ta >= t and m >= s+k), None)
            if e3 is not None and nx >= e3:
                waits3.append((nx-e3)/1e9)
                if eK is not None and nx >= eK: waitsK.append((nx-eK)/1e9)
                else: shift += 1
    return waits3, waitsK, shift, kraise

cell = pathlib.Path(sys.argv[1]); arm = sys.argv[2]
W3, WK, SH, KR = [], [], 0, []
for rep in sorted((cell/arm).glob("rep*"), key=lambda p: int(p.name[3:])):
    a, b, c, k = analyze(rep); W3 += a; WK += b; SH += c
    if k: KR.append((rep.name, k))
print(f"== {cell.name}/{arm}: post_ev3(K=3) n={len(W3)} wait p50={q(W3,.5):.3f} p95={q(W3,.95):.3f} | adaptive-K: still post-evK n={len(WK)} p50={q(WK,.5):.3f} p95={q(WK,.95):.3f}; reclassified(retx before evK)={SH}")
print("  K raises (rep, [(t_s, dist)]):", KR[:12], f"... reps_with_raise={len(KR)}")
