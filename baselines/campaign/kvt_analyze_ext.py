#!/usr/bin/env python3
"""EK-1 (kama-vs-tcp-v1-ext1) DONMUŞ ANALİZ — üç kol, ikili hükümler. Kurallar v1 ile AYNI (kvt_analyze.py'den alınır):
Wilson %95; "ÖNDE" yalnız aralıklar ayrışıyorsa; metrik horizon-complete (R1+R2 havuz); deadline ayrı sütun;
status≠OK paydada kalır; loadavg1>2.0 işaretli. Kullanım: kvt_analyze.py ile aynı bayraklar.
Çıktı: EXT-ACCOUNTING.md · EXT-CELL-RESULTS.csv · EXT-GRAPH.csv · EXT-TABLES.md
"""
import collections, csv, os, statistics, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from kvt_analyze import wilson, pct, args

ARMS = ["kama-v1-full", "tcp-std", "quinn"]; ORDER = {"low": 0, "med": 1, "high": 2}

def main():
    out = args("--out")[0]; os.makedirs(out, exist_ok=True)
    man = {k: list(csv.DictReader(open(v))) for k, v in (x.split("=", 1) for x in args("--manifest"))}
    res = {}
    for k, v in (x.split("=", 1) for x in args("--result")): res.setdefault(k, []).extend(csv.DictReader(open(v)))
    acc = ["# EK-1 satır muhasebesi\n", "| replika | beklenen | üretilen | eksik | fazla | yinelenen | status≠OK | load_flag |", "|---|---|---|---|---|---|---|---|"]
    rows = []
    for rep in sorted(man):
        exp = [m["run_id"] for m in man[rep]]; got = [r["run_id"] for r in res.get(rep, [])]; cnt = collections.Counter(got)
        missing = sorted(set(exp) - set(got)); extra = set(got) - set(exp); bad = [r for r in res.get(rep, []) if r["status"] != "OK"]
        acc.append(f"| {rep} | {len(exp)} | {len(got)} | {len(missing)} | {len(extra)} | {sum(c - 1 for c in cnt.values() if c > 1)} | {len(bad)} | {sum(int(r['load_flag']) for r in res.get(rep, []))} |")
        if missing: acc.append(f"\n**{rep} eksik ({len(missing)}):** " + ", ".join(missing[:30]))
        if bad: acc.append(f"\n**{rep} status≠OK:** " + ", ".join(f"{r['run_id']}={r['status']}(rc={r['rc']})" for r in bad[:30]))
        seen = set()
        for r in res.get(rep, []):
            if r["run_id"] not in seen and r["run_id"] not in extra: seen.add(r["run_id"]); rows.append(r)
    open(os.path.join(out, "EXT-ACCOUNTING.md"), "w").write("\n".join(acc) + "\n")

    def summ(sel):
        g = collections.defaultdict(list)
        for r in sel: g[(r["group"], int(r["loss_pct"]), r["rtt_class"], r["workload"], r["arm"])].append(r)
        o = {}
        for k, v in g.items():
            n = len(v); ok = [r for r in v if r["status"] == "OK"]
            xh = sum(int(r["complete_horizon"]) for r in ok); xd = sum(int(r["complete_deadline"]) for r in ok)
            tc = [float(r["t_complete_ms"]) for r in ok if r["complete_horizon"] == "1"]
            o[k] = dict(n=n, xh=xh, xd=xd, wh=wilson(xh, n), wd=wilson(xd, n), tc50=pct(tc, .5), tc90=pct(tc, .9),
                        wire50=statistics.median(int(r["wire_bytes"]) for r in v), flagged=sum(int(r["load_flag"]) for r in v))
        return o
    P = summ(rows); per = {rep: summ([r for r in rows if r["replica"] == rep]) for rep in sorted(man)}; U = summ([r for r in rows if r["load_flag"] == "0"])
    f3 = lambda x: "" if x == "" else f"{x:.3f}"
    with open(os.path.join(out, "EXT-CELL-RESULTS.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n"); w.writerow(["scope","group","loss_pct","rtt_class","workload","arm","n","horizon_x","horizon_lo","horizon_hi","deadline_x","deadline_lo","deadline_hi","t_p50_ms","t_p90_ms","wire_p50","load_flagged"])
        for sc, S in [("pooled", P)] + list(per.items()) + [("pooled-unflagged", U)]:
            for k in sorted(S, key=lambda k: (k[0], k[1], ORDER[k[2]], k[3], k[4])):
                s = S[k]; w.writerow([sc, *k, s["n"], s["xh"], f3(s["wh"][0]), f3(s["wh"][1]), s["xd"], f3(s["wd"][0]), f3(s["wd"][1]), f3(s["tc50"]), f3(s["tc90"]), s["wire50"], s["flagged"]])
    with open(os.path.join(out, "EXT-GRAPH.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n"); w.writerow(["loss_pct","rtt_class","workload","arm","completion_rate","n","x","wilson_lo","wilson_hi","deadline_rate","scope","group"])
        for sc, S in [("pooled", P)] + list(per.items()):
            for k in sorted(S, key=lambda k: (k[0], k[1], ORDER[k[2]], k[3], k[4])):
                s = S[k]; w.writerow([k[1], k[2], k[3], k[4], f3(s["xh"] / s["n"]), s["n"], s["xh"], f3(s["wh"][0]), f3(s["wh"][1]), f3(s["xd"] / s["n"]), sc, k[0]])

    def ver(a, b, na, nb):
        if a["wh"][0] > b["wh"][1]: return f"**{na}**"
        if b["wh"][0] > a["wh"][1]: return f"**{nb}**"
        return "—"
    T = []; NAME = {"kama-v1-full": "Kama", "tcp-std": "TCP", "quinn": "QUIC"}
    TITLE = {"quic": "Q — QUIC ekseni (IID simetrik, rate yok)", "band": "B — bant ekseni (rate 256 kbit iki yön, IID simetrik)", "burst": "G — burst (Gilbert–Elliott ileri yön; dönüş kayıpsız)"}
    for grp in ("quic", "band", "burst"):
        T.append(f"\n## {TITLE[grp]}\n")
        T.append("| kayıp | RTT | yük | Kama horizon [W95] | TCP horizon [W95] | QUIC horizon [W95] | deadline K/T/Q | t_tam p50 ms K/T/Q | tel B p50 K/T/Q | önde: K↔T | K↔Q | Q↔T |")
        T.append("|---|---|---|---|---|---|---|---|---|---|---|---|")
        tally = collections.Counter()
        for loss, rc, wl in sorted({k[1:4] for k in P if k[0] == grp}, key=lambda k: (k[0], ORDER[k[1]], k[2])):
            s = [P.get((grp, loss, rc, wl, a)) for a in ARMS]
            if not all(s): continue
            h = [f"{x['xh']}/{x['n']} [{x['wh'][0]:.2f},{x['wh'][1]:.2f}]" for x in s]
            v = [ver(s[0], s[1], "Kama", "TCP"), ver(s[0], s[2], "Kama", "QUIC"), ver(s[2], s[1], "QUIC", "TCP")]
            for name, val in zip(("K↔T", "K↔Q", "Q↔T"), v): tally[(name, val)] += 1
            T.append(f"| {loss}% | {rc} | {wl} | {h[0]} | {h[1]} | {h[2]} | {'/'.join(str(x['xd']) for x in s)} | {' / '.join(f3(x['tc50']) or '—' for x in s)} | {' / '.join(str(int(x['wire50'])) for x in s)} | {v[0]} | {v[1]} | {v[2]} |")
        T.append("\nÖzet: " + " · ".join(f"{k[0]} {k[1]}: {n}" for k, n in sorted(tally.items())))
    reps = sorted(per); dis = tot = 0
    if len(reps) == 2:
        for k, a in per[reps[0]].items():
            b = per[reps[1]].get(k)
            if b: tot += 1; dis += (a["wh"][0] > b["wh"][1] or b["wh"][0] > a["wh"][1])
        T.append(f"\n## R1 ↔ R2 uyumu\n\n{tot} kol×hücre çiftinin {dis} tanesinde replikaların Wilson aralıkları ayrışıyor.")
    ch = cmp_n = 0
    for k, a in P.items():
        if k[4] == "kama-v1-full":
            for other in ("tcp-std", "quinn"):
                b = P.get(k[:4] + (other,)); ua, ub = U.get(k), U.get(k[:4] + (other,))
                if b and ua and ub: cmp_n += 1; ch += ver(a, b, "K", "O") != ver(ua, ub, "K", "O")
    T.append(f"\n## Yük duyarlılığı\n\nİşaretli satır {sum(int(r['load_flag']) for r in rows)}/{len(rows)}; işaretliler dışlanınca DEĞİŞEN Kama-ikili hükmü: {ch}/{cmp_n}.")
    open(os.path.join(out, "EXT-TABLES.md"), "w").write("\n".join(T) + "\n")
    print(f"analyze-ext: rows={len(rows)} cells={len(P)}")

if __name__ == "__main__":
    main()
