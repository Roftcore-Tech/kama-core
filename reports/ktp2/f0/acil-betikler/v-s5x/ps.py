# Akis-basina (PTT stream = O + P ayni sp) siralama karsi-olgusu: ham jsonl'dan.
import os, sys, json, statistics, math, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import importlib.util
spec = importlib.util.spec_from_file_location("reo", os.path.join(os.path.dirname(os.path.abspath(__file__)), "reo.py"))
src = open(spec.origin).read().replace("\nmain()\n", "\n")
ns = {}
exec(compile(src, "reo", "exec"), ns)
load = ns["load"]; q = ns["q"]; med = ns["med"]
import bisect
def run(rd):
    A = load(os.path.join(rd, "alice.jsonl")); B = load(os.path.join(rd, "bob.jsonl"))
    S = sorted([r for d in A if d["k"] == "tr" for r in d["r"] if r[0] == "S"], key=lambda r: r[4])
    at = [r[4] for r in S]
    info = {}
    for s in (d for d in A if d["k"] == "send"):
        j = bisect.bisect_left(at, s["tr"]); best = min([k for k in (j-1, j, j+1) if 0 <= k < len(at)], key=lambda k: abs(at[k]-s["tr"]))
        info[S[best][2]] = (s["c"], s.get("sp"), s["t"])
    D = {}; Rel = []
    for d in B:
        if d["k"] == "tr":
            for r in d["r"]:
                if r[0] == "D" and r[2] not in D: D[r[2]] = r[4]
                elif r[0] == "R": Rel.append((r[4], r[2]))
    Rel.sort(); rel_at = {}; mx = 0
    for a, n in Rel:
        if n > mx:
            for u in range(mx, n): rel_at.setdefault(u, a)
            mx = n
    rows = []
    for u in sorted(info):
        c, sp, ts = info[u]
        if c != "P" or u not in D or u not in rel_at: continue
        prev = [D.get(v) for v in info if v <= u and info[v][0] in ("O", "P") and info[v][1] == sp]
        if any(x is None for x in prev): continue
        rps = max(prev)
        rows.append(dict(reo=rel_at[u]-D[u], reo_ps=rps-D[u], tot=rel_at[u]-ts, tot_ps=rps-ts, pre=D[u]-ts))
    return rows
root = sys.argv[1]
for cell in sys.argv[2].split(","):
    for arm in ("taban", "aday"):
        ad = os.path.join(root, cell, arm); per = []
        for rep in sorted(os.listdir(ad)):
            if not rep.startswith("rep"): continue
            rows = run(os.path.join(ad, rep))
            if not rows: continue
            n = len(rows); sr = sum(r["reo"] for r in rows)
            per.append(dict(remain=(sum(r["reo_ps"] for r in rows)/sr) if sr > 0 else None,
                            held_ps=sum(1 for r in rows if r["reo_ps"] > 50)/n,
                            w800=sum(1 for r in rows if r["tot"] <= 800)/n,
                            w800ps=sum(1 for r in rows if r["tot_ps"] <= 800)/n,
                            w800pre=sum(1 for r in rows if r["pre"] <= 800)/n,
                            reo50=q([r["reo"] for r in rows], .5), reo_ps50=q([r["reo_ps"] for r in rows], .5),
                            tot95=q([r["tot"] for r in rows], .95), tot_ps95=q([r["tot_ps"] for r in rows], .95)))
        print(cell, arm, "N=", len(per), json.dumps({k: round(med([p[k] for p in per]), 3) for k in per[0]}))
