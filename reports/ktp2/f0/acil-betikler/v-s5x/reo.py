#!/usr/bin/env python3
# Bagimsiz yeniden hesap: PTT demetlerinin Reorder bekleyisi (bob D->R), toplam (rx-send),
# karsi-olgu (D-send), ilk engelleyici sinif. Yalniz ham jsonl okur.
import json, os, sys, statistics, collections, math, bisect

def q(v, p):
    v = sorted(x for x in v if x is not None)
    if not v:
        return None
    return v[max(0, math.ceil(p * len(v)) - 1)]

def med(v):
    v = [x for x in v if x is not None]
    return statistics.median(v) if v else None

def load(path):
    out = []
    with open(path) as f:
        for l in f:
            l = l.strip()
            if l:
                out.append(json.loads(l))
    return out

def run(rd):
    A = load(os.path.join(rd, "alice.jsonl"))
    B = load(os.path.join(rd, "bob.jsonl"))
    S = []
    sends = []
    for d in A:
        if d["k"] == "tr":
            for r in d["r"]:
                if r[0] == "S":
                    S.append(r)
        elif d["k"] == "send":
            sends.append(d)
    sess_S = collections.Counter(r[1] for r in S)
    # send kaydini S kaydina tr~at ile esle
    S_sorted = sorted(S, key=lambda r: r[4])
    at_list = [r[4] for r in S_sorted]
    unit_of = {}
    cls = {}
    tsend = {}
    bad = 0
    for s in sends:
        tr = s.get("tr")
        if tr is None:
            bad += 1
            continue
        j = bisect.bisect_left(at_list, tr)
        best = None
        for k in (j - 1, j, j + 1):
            if 0 <= k < len(at_list):
                if best is None or abs(at_list[k] - tr) < abs(at_list[best] - tr):
                    best = k
        if best is None or abs(at_list[best] - tr) > 5:
            bad += 1
            continue
        u = S_sorted[best][2]
        c = s["c"]
        unit_of[(c, s.get("sp"), s.get("i"))] = u
        cls[u] = c
        tsend[u] = s["t"]
    D = {}
    Rel = []  # (at, next)
    sessD = collections.Counter()
    for d in B:
        if d["k"] == "tr":
            for r in d["r"]:
                if r[0] == "D":
                    sessD[r[1]] += 1
                    if r[2] not in D:
                        D[r[2]] = r[4]
                elif r[0] == "R":
                    Rel.append((r[4], r[2]))
    Rel.sort()
    rx = {}
    for d in B:
        if d["k"] == "rx" and d.get("first", True):
            key = (d["c"], d.get("sp"), d.get("i"))
            if key not in rx:
                rx[key] = d["t"]
    # R(u): ilk R kaydi next>u
    rel_at = {}
    mx = -1
    for at, nxt in Rel:
        if nxt > mx:
            for u in range(mx if mx >= 0 else 0, nxt):
                if u not in rel_at:
                    rel_at[u] = at
            mx = nxt
    # Drained aninda next (ilk engelleyici): D(u) anindaki en son R degeri
    rel_times = [a for a, _ in Rel]
    rel_vals = []
    m = 0
    for _, n in Rel:
        m = max(m, n)
        rel_vals.append(m)
    rows = []
    for (c, sp, i), u in unit_of.items():
        if c != "P":
            continue
        d = D.get(u)
        r = rel_at.get(u)
        x = rx.get((c, sp, i))
        ts = tsend[u]
        reo = (r - d) if (r is not None and d is not None) else None
        tot = (x - ts) if x is not None else None
        pre = (d - ts) if d is not None else None
        blk = None
        if d is not None and reo is not None and reo > 50:
            k = bisect.bisect_right(rel_times, d) - 1
            nxt = rel_vals[k] if k >= 0 else 0
            if nxt <= u:
                blk = cls.get(nxt, "X")
        rows.append(dict(u=u, reo=reo, tot=tot, pre=pre, blk=blk))
    return rows, dict(bad=bad, sessS=dict(sess_S), sessD=dict(sessD), nsend=len(sends), nS=len(S))

def main():
    root = sys.argv[1]
    for cell in sys.argv[2].split(","):
        for arm in ("taban", "aday"):
            ad = os.path.join(root, cell, arm)
            if not os.path.isdir(ad):
                continue
            per = []
            bw = collections.Counter()
            meta_bad = 0
            sessinfo = collections.Counter()
            for rep in sorted(os.listdir(ad)):
                if not rep.startswith("rep"):
                    continue
                rd = os.path.join(ad, rep)
                try:
                    rows, meta = run(rd)
                except FileNotFoundError:
                    continue
                meta_bad += meta["bad"]
                sessinfo[(len(meta["sessS"]), len(meta["sessD"]))] += 1
                if not rows:
                    continue
                n = len(rows)
                for r in rows:
                    if r["blk"]:
                        bw[r["blk"]] += r["reo"]
                per.append(dict(
                    n=n,
                    held=sum(1 for r in rows if r["reo"] is not None and r["reo"] > 50) / n,
                    w800=sum(1 for r in rows if r["tot"] is not None and r["tot"] <= 800) / n,
                    w800p=sum(1 for r in rows if r["pre"] is not None and r["pre"] <= 800) / n,
                    w2k=sum(1 for r in rows if r["tot"] is not None and r["tot"] <= 2000) / n,
                    w2kp=sum(1 for r in rows if r["pre"] is not None and r["pre"] <= 2000) / n,
                    reo50=q([r["reo"] for r in rows], .5), reo95=q([r["reo"] for r in rows], .95),
                    tot50=q([r["tot"] for r in rows], .5), tot95=q([r["tot"] for r in rows], .95),
                    pre50=q([r["pre"] for r in rows], .5), pre95=q([r["pre"] for r in rows], .95),
                    reo_share=(sum(r["reo"] for r in rows if r["reo"] is not None) /
                               max(1e-9, sum(r["tot"] for r in rows if r["tot"] is not None))),
                ))
            W = sum(bw.values()) or 1
            res = {k: (round(med([p[k] for p in per]), 3) if med([p[k] for p in per]) is not None else None)
                   for k in ("held", "w800", "w800p", "w2k", "w2kp", "reo50", "reo95", "tot50", "tot95",
                             "pre50", "pre95", "reo_share")}
            print(cell, arm, "N=", len(per), "units=", sum(p["n"] for p in per), "badmap=", meta_bad,
                  "sess(S,D)=", dict(sessinfo))
            print("   ", json.dumps(res))
            print("    blk_share", {k: round(100 * v / W, 1) for k, v in bw.most_common()})

main()
