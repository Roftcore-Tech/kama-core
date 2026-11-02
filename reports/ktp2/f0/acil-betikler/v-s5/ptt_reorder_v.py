#!/usr/bin/env python3
"""Bagimsiz yeniden hesap: PTT demetlerinin Reorder bekleyisi (salt okuma)."""
import bisect
import json
import os
import statistics
import sys

ROOT = "/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/a/out"


def load(path):
    out = []
    with open(path) as f:
        for ln in f:
            ln = ln.strip()
            if not ln:
                continue
            try:
                out.append(json.loads(ln))
            except Exception:
                pass
    return out


def q(xs, p):
    xs = sorted(xs)
    if not xs:
        return None
    k = (len(xs) - 1) * p
    lo = int(k)
    hi = min(lo + 1, len(xs) - 1)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def run(repdir):
    a = load(os.path.join(repdir, "alice.jsonl"))
    b = load(os.path.join(repdir, "bob.jsonl"))
    S = []
    for d in a:
        if d.get("k") == "tr":
            for r in d["r"]:
                if r[0] == "S":
                    S.append(r)
    D = {}
    R = []
    for d in b:
        if d.get("k") == "tr":
            for r in d["r"]:
                if r[0] == "D" and r[2] not in D:
                    D[r[2]] = r[4]
                elif r[0] == "R":
                    R.append((r[4], r[2]))
    sess_S = {r[1] for r in S}
    if len(sess_S) != 1:
        return None, "multi-session"
    S.sort(key=lambda r: r[2])
    sends = sorted((d for d in a if d.get("k") == "send"), key=lambda d: d["t"])
    if len(sends) != len(S):
        return None, f"sends {len(sends)} != S {len(S)}"
    # unit k <-> k-th send (cagri sirasi); boy kontrolu
    unit_cls = {}
    items = []
    for k, (s, r) in enumerate(zip(sends, S)):
        assert r[2] == k, (r, k)
        unit_cls[k] = s["c"]
        items.append({"c": s["c"], "sp": s.get("sp"), "i": s.get("i"), "unit": k,
                      "t_send": s["t"], "len": r[3]})
    R.sort()
    rel_t, rel_n, mx = [], [], -1
    for t, n in R:
        mx = max(mx, n)
        rel_t.append(t)
        rel_n.append(mx)

    def released(u):
        i = bisect.bisect_right(rel_n, u)
        return rel_t[i] if i < len(rel_t) else None

    def next_at(t):
        i = bisect.bisect_right(rel_t, t)
        return rel_n[i - 1] if i > 0 else 0

    rx = {}
    for d in b:
        if d.get("k") == "rx" and d.get("first", True) and d.get("c") == "P":
            rx.setdefault((d.get("sp"), d.get("i")), d["t"])
    out = []
    for it in items:
        if it["c"] != "P":
            continue
        u = it["unit"]
        dr = D.get(u)
        rl = released(u)
        x = rx.get((it["sp"], it["i"]))
        it["drained"] = dr
        it["released"] = rl
        it["rx"] = x
        it["reorder"] = (rl - dr) if (rl is not None and dr is not None) else None
        it["total"] = (x - it["t_send"]) if x is not None else None
        it["blk"] = None
        if dr is not None and rl is not None:
            n = next_at(dr)
            if n < u:
                it["blk"] = unit_cls.get(n, "X")
                if it["blk"] == "P":
                    pass
        out.append(it)
    return out, None


def cell(name, arm):
    base = os.path.join(ROOT, name, arm)
    reps = sorted((d for d in os.listdir(base) if d.startswith("rep")), key=lambda s: int(s[3:]))
    per = []
    pooled = []
    blk_w = {}
    for rp in reps:
        items, err = run(os.path.join(base, rp))
        if items is None:
            print("SKIP", name, arm, rp, err, file=sys.stderr)
            continue
        ok = [i for i in items if i["reorder"] is not None and i["total"] is not None]
        n = len(items)
        if not ok:
            continue
        ro = [i["reorder"] for i in ok]
        tot = [i["total"] for i in ok]
        alt = [i["total"] - i["reorder"] for i in ok]
        per.append({
            "n": n, "nok": len(ok),
            "ro50": sum(1 for v in ro if v > 50) / n,
            "ro_p50": q(ro, .5), "ro_p95": q(ro, .95),
            "tot_p50": q(tot, .5), "tot_p95": q(tot, .95),
            "le800": sum(1 for v in tot if v <= 800) / n,
            "alt800": sum(1 for v in alt if v <= 800) / n,
            "le2000": sum(1 for v in tot if v <= 2000) / n,
            "alt2000": sum(1 for v in alt if v <= 2000) / n,
        })
        for i in ok:
            pooled.append(i)
            if i["blk"] is not None and i["reorder"] > 50:
                key = i["blk"]
                if key == "P" or key == "O" or key == "C":
                    pass
                blk_w[key] = blk_w.get(key, 0.0) + i["reorder"]
    med = {k: statistics.median([p[k] for p in per]) for k in per[0]} if per else {}
    tw = sum(blk_w.values())
    print(f"== {name} / {arm}: runs={len(per)} demet={sum(p['n'] for p in per)} ok={sum(p['nok'] for p in per)}")
    for k in ("ro50", "ro_p50", "ro_p95", "tot_p50", "tot_p95", "le800", "alt800", "le2000", "alt2000"):
        print(f"   median[{k}] = {med.get(k):.4f}")
    # havuz
    ro = [i["reorder"] for i in pooled]
    print(f"   pooled ro>50 = {sum(1 for v in ro if v > 50)/len(ro):.4f}; pooled le800 = "
          f"{sum(1 for i in pooled if i['total'] <= 800)/len(pooled):.4f}; pooled alt800 = "
          f"{sum(1 for i in pooled if i['total']-i['reorder'] <= 800)/len(pooled):.4f}")
    print("   blk (bekleme agirlikli, ro>50):", {k: round(v / tw, 4) for k, v in sorted(blk_w.items())})


if __name__ == "__main__":
    for name in sys.argv[1:]:
        for arm in ("taban", "aday"):
            cell(name, arm)
