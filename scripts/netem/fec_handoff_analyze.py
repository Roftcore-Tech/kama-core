#!/usr/bin/env python3
"""[HANDOFF] t_first_confirmed çevresinde parity ve teslim — cliff var mı?

Pencereler (ms, t_conf'a göre): pre=[−1000,0) · post1=[0,+1000) · post3=[+1000,+3000).
Birim = REP; hücre özetleri MEDYAN. 'asla' = onay hiç gelmedi (kısa akış kontrolü).
Saat notu: ts.csv runtime saatinde, out.jsonl bench saatinde; ofset ~ms (1 s pencerelerde ihmal).
"""
import sys, glob, re, json, statistics, csv
root = sys.argv[1]; D = 400_000_000
REASON = ["None","ConfFuzzy","ConfPolicy","StructGate","SpecBoot","SpecPersist","DeadlineAsst"]

def load(d):
    t = open(f"{d}/c.err", errors="replace").read()
    rt = re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)", t)
    fe = re.search(r"flow_end=(\d+)", t)
    if not (rt and fe): return None
    t_susp, t_conf = int(rt.group(1))/1e6, int(rt.group(2))/1e6
    flow_end = int(fe.group(1))/1e6
    ts = []
    try:
        for row in csv.DictReader(open(f"{d}/ts.csv")):
            ts.append({k: int(v) for k, v in row.items()})
    except FileNotFoundError:
        return None
    msgs = []
    for i, line in enumerate(open(f"{d}/out.jsonl")):
        if i == 0: continue
        e = json.loads(line).get("event", {})
        msgs.append((e["sent_ns"]/1e6, (e.get("delivered_ns") or 10**18) - e["sent_ns"] <= D))
    g = lambda p, dflt=0: int(re.search(p, t).group(1)) if re.search(p, t) else dflt
    return dict(t_susp=t_susp, t_conf=t_conf if t_conf else None, flow_end=flow_end, ts=ts, msgs=msgs,
                fec=g(r"fec=(\d+) "), tx=g(r"tx=(\d+)B"))

def win(ts, lo, hi):
    return [x for x in ts if lo <= x["t_ms"] < hi]
def mean(v): return sum(v)/len(v) if v else float("nan")
def modal_reason(rows):
    if not rows: return "-"
    c = {}
    for x in rows: c[x["reason_idx"]] = c.get(x["reason_idx"], 0) + 1
    return REASON[max(c, key=c.get)]

cells = {}
for d in sorted(glob.glob(f"{root}/*/rep*")):
    r = load(d)
    if r: cells.setdefault(d.split("/")[-2], []).append(r)

def key(c):
    m = re.match(r"m(\d+)_l(\d+)_r(\d)", c); return (int(m.group(1)), int(m.group(2)), int(m.group(3)))

print("=" * 128)
print("HANDOFF — rep başına: t_conf; final_ratio ort. (‰) pre/post1/post3; hüküm sebebi; teslim≤2×RTT pre/post")
print("=" * 128)
print(f"{'hücre':14s} {'rep':>3s} {'t_conf':>7s} {'süre':>6s} {'fin_pre':>7s} {'fin_p1':>7s} {'fin_p3':>7s} "
      f"{'spec_pre':>8s} {'spec_p1':>7s} {'gate_p3':>7s} {'sebep_pre':>11s} {'sebep_p1':>11s} {'sebep_p3':>11s} "
      f"{'del_pre':>7s} {'del_p1':>7s} {'del_p3':>7s} {'fec_B':>6s}")
print("-" * 128)
agg = {}
for c in sorted(cells, key=key):
    for i, r in enumerate(cells[c]):
        tc = r["t_conf"]
        if tc is None:
            print(f"{c:14s} {i+1:>3d} {'asla':>7s} {r['flow_end']:>6.0f} {'-':>7s} {'-':>7s} {'-':>7s} {'-':>8s} {'-':>7s} {'-':>7s} "
                  f"{'-':>11s} {'-':>11s} {'-':>11s} {'-':>7s} {'-':>7s} {'-':>7s} {r['fec']:>6d}")
            continue
        pre, p1, p3 = win(r["ts"], tc-1000, tc), win(r["ts"], tc, tc+1000), win(r["ts"], tc+1000, tc+3000)
        f = lambda rows, k: mean([x[k] for x in rows])
        dm = lambda lo, hi: mean([ok for (s, ok) in r["msgs"] if lo <= s < hi])
        row = dict(fin_pre=f(pre,"final_ratio"), fin_p1=f(p1,"final_ratio"), fin_p3=f(p3,"final_ratio"),
                   spec_pre=f(pre,"spec_prop"), spec_p1=f(p1,"spec_prop"), gate_p3=f(p3,"structure_prop"),
                   del_pre=dm(tc-1000, tc), del_p1=dm(tc, tc+1000), del_p3=dm(tc+1000, tc+3000))
        agg.setdefault(c, []).append(row)
        fmt = lambda v: "-" if v != v else f"{v:.0f}"
        fmtd = lambda v: "-" if v != v else f"{v:.2f}"
        print(f"{c:14s} {i+1:>3d} {tc:>7.0f} {r['flow_end']:>6.0f} {fmt(row['fin_pre']):>7s} {fmt(row['fin_p1']):>7s} {fmt(row['fin_p3']):>7s} "
              f"{fmt(row['spec_pre']):>8s} {fmt(row['spec_p1']):>7s} {fmt(row['gate_p3']):>7s} "
              f"{modal_reason(pre):>11s} {modal_reason(p1):>11s} {modal_reason(p3):>11s} "
              f"{fmtd(row['del_pre']):>7s} {fmtd(row['del_p1']):>7s} {fmtd(row['del_p3']):>7s} {r['fec']:>6d}")

print()
print("=" * 128)
print("ÖZET (medyan) — İNVARYANT: kanonik sinyal gelince final parity ve teslim SEBEPSİZ DÜŞMEMELİ")
print("=" * 128)
print(f"{'hücre':14s} {'n':>2s} {'fin_pre':>7s} {'fin_p1':>7s} {'fin_p3':>7s} {'Δ_p1':>6s} {'Δ_p3':>6s} "
      f"{'del_pre':>7s} {'del_p1':>7s} {'del_p3':>7s} {'Δdel_p1':>8s} {'HÜKÜM':>22s}")
print("-" * 128)
for c in sorted(agg, key=key):
    rows = agg[c]; n = len(rows)
    md = lambda k: statistics.median([x[k] for x in rows if x[k] == x[k]] or [float("nan")])
    fp, f1, f3 = md("fin_pre"), md("fin_p1"), md("fin_p3")
    dp, d1, d3 = md("del_pre"), md("del_p1"), md("del_p3")
    d_p1 = f1 - fp; d_p3 = f3 - fp; dd = d1 - dp
    verdict = "CLIFF (parity ↓)" if d_p1 <= -50 else ("aşırı-atış (parity ↑↑)" if d_p3 >= 150 else "düz")
    if dd == dd and dd <= -0.10: verdict += " + teslim ↓"
    print(f"{c:14s} {n:>2d} {fp:>7.0f} {f1:>7.0f} {f3:>7.0f} {d_p1:>+6.0f} {d_p3:>+6.0f} "
          f"{dp:>7.2f} {d1:>7.2f} {d3:>7.2f} {dd:>+8.2f} {verdict:>22s}")
