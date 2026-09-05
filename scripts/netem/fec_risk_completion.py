#!/usr/bin/env python3
"""2×RTT (400 ms) içinde teslim oranı — FEC-MARGINAL-R'nin birincil uç noktası. Birim = REP."""
import json, sys, glob, re, statistics
root = sys.argv[1]; D = 400_000_000
def comp(path):
    n = ok = 0
    with open(path) as f:
        for i, line in enumerate(f):
            if i == 0: continue
            try: e = json.loads(line); e = e.get("event", e)
            except: continue
            s = e.get("sent_ns"); d = e.get("delivered_ns")
            if s is None: 
                return None, list(e.keys())
            n += 1
            if d is not None and d - s <= D: ok += 1
    return (ok / n if n else 0.0), None
cells = {}
for f in sorted(glob.glob(f"{root}/*/rep*/out.jsonl")):
    cell = f.split("/")[-3]; c, keys = comp(f)
    if keys: print("ANAHTARLAR:", keys); sys.exit()
    cells.setdefault(cell, []).append(c)
def lo(c): return int(re.search(r"l(\d+)_", c).group(1))
print(f"{'kayıp':>5s} {'kol':>4s} {'n':>2s} {'teslim≤2×RTT medyan':>20s} {'min':>6s} {'max':>6s}")
for c in sorted(cells, key=lambda x: (lo(x), x)):
    v = cells[c]; print(f"{lo(c):>4d}% {'ON' if c.endswith('_r1') else 'OFF':>4s} {len(v):>2d} {statistics.median(v):>20.3f} {min(v):>6.3f} {max(v):>6.3f}")
