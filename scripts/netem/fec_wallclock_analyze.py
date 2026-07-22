#!/usr/bin/env python3
"""Commit-FR/FS — WALL-CLOCK marjinal-FEC-utility analizi (faithful two-netns harness; GERÇEK RTT).

ÇERÇEVE: Bu analiz ARQ-vs-FEC KARŞILAŞTIRMASI DEĞİLDİR. ARQ/substrat bütün-kollarda-aktif; ölçülen ARQ
üzerine eklenen proaktif-FEC-parity'nin marjinal WALL-CLOCK faydasıdır.

Girdi: reports/d1-exit/data/wallclock/*-rxledger.csv (kolon: run_id,block,first_ms,complete_ms,rel_ms,
dl_op,deficit_applied,decode_via,first_repair_ms,repair_kind,arq_repair_ms). run_id: LABEL-pinN-dD-rR-MODE-REP.
Yalnız deficit_applied>0 (warm-steady) bloklar.

Türetimler (per deficit-block; hepsi block-first-arrival'a göreli → one-way-delay-iptal):
  T_fec_avail = first_repair_ms - first_ms   (parity-arrival; FEC-decode-mümkün-anı-proxy'si)
  T_arq       = arq_repair_ms - first_ms     (substrat-onarım-arrival; §9-ARQ/§11-dup)
  completion  = rel_ms                        (blok-tamamlanma-latency = min(FEC-yol, ARQ-yol))
  decode_via  = 0 substrat / 1 FEC            (hangi-yol-KAZANDI)

Phase-C (timing feasibility): her RTT-profili × hybrid için p99(T_fec_avail) < p01(T_arq) mı → deadline-
interval-var-mı (frozen-deadline-önce-ilan). Phase-D/E: r>=d vs r<d completion-latency + decode-path +
utility-sınıfı.
"""
import csv
import glob
import os
import re
import statistics as st
import sys
from collections import defaultdict

DATA = sys.argv[1] if len(sys.argv) > 1 else "reports/d1-exit/data/wallclock"
RID = re.compile(r"(rtt\d+)-(pin\d)-d(\d+)-r(\d+)-(iso|hyb)-(\d+)")
K = 8

# rows[(rtt,deficit,r,mode)] = list of per-block dicts
rows = defaultdict(list)
for path in sorted(glob.glob(os.path.join(DATA, "*-rxledger.csv"))):
    base = os.path.basename(path)[: -len("-rxledger.csv")]
    m = RID.match(base)
    if not m:
        continue
    rtt, _pin, deficit, r, mode, _rep = m.groups()
    deficit, r = int(deficit), int(r)
    with open(path) as f:
        for row in csv.DictReader(f):
            if int(row["deficit_applied"]) == 0:
                continue
            first = int(row["first_ms"])
            comp = int(row["complete_ms"])
            dv = int(row["decode_via"])
            fr = int(row["first_repair_ms"])
            ar = int(row["arq_repair_ms"])
            rows[(rtt, deficit, r, mode)].append({
                "completion": (comp - first) if comp >= 0 else None,
                "decode_via": dv,
                "t_fec": (fr - first) if fr >= 0 else None,
                "t_arq": (ar - first) if ar >= 0 else None,
                "pending": comp < 0,
            })


def pct(xs, p):
    xs = sorted(x for x in xs if x is not None)
    if not xs:
        return None
    i = min(len(xs) - 1, max(0, int(round(p / 100 * (len(xs) - 1)))))
    return xs[i]


def summ(key):
    r = rows.get(key, [])
    n = len(r)
    comp = [x["completion"] for x in r if x["completion"] is not None]
    pend = sum(1 for x in r if x["pending"])
    via1 = sum(1 for x in r if x["decode_via"] == 1)
    via0 = sum(1 for x in r if x["decode_via"] == 0)
    tfec = [x["t_fec"] for x in r if x["t_fec"] is not None]
    tarq = [x["t_arq"] for x in r if x["t_arq"] is not None]
    return dict(n=n, pend=pend, via1=via1, via0=via0,
                comp_mean=(st.mean(comp) if comp else None),
                comp_p50=pct(comp, 50), comp_p95=pct(comp, 95),
                tfec_p50=pct(tfec, 50), tfec_p99=pct(tfec, 99),
                tarq_p50=pct(tarq, 50), tarq_p01=pct(tarq, 1))


print("BU ANALİZ ARQ-vs-FEC DEĞİL: ARQ/substrat bütün-kollarda-aktif; ölçülen marjinal-proaktif-FEC WALL-CLOCK-faydası.\n")

rtts = sorted({k[0] for k in rows}, key=lambda s: int(s[3:]))

print("=== PHASE-C: TIMING FEASIBILITY (hybrid; T_fec_avail vs T_arq; frozen-deadline-interval) ===")
print(f"{'RTT':8} {'def':3} {'r':2} {'N':4} {'T_fec_p50':9} {'T_fec_p99':9} {'T_arq_p50':9} {'T_arq_p01':9} interval?")
for rtt in rtts:
    for (deficit, r) in [(1, 1), (2, 2), (4, 4)]:  # r>=d hybrid: parity mevcut → T_fec ölçülür
        s = summ((rtt, deficit, r, "hyb"))
        if s["n"] == 0:
            continue
        # frozen-deadline-interval: p99(T_fec) < p01(T_arq) → FEC-decode ARQ-repair'den-ÖNCE ayrışır.
        iv = "YES" if (s["tfec_p99"] is not None and s["tarq_p01"] is not None and s["tfec_p99"] < s["tarq_p01"]) else "no/overlap"
        print(f"{rtt:8} {deficit:<3} {r:<2} {s['n']:<4} {str(s['tfec_p50']):9} {str(s['tfec_p99']):9} "
              f"{str(s['tarq_p50']):9} {str(s['tarq_p01']):9} {iv}")

print("\n=== PHASE-D/E: MARGINAL FEC WALL-CLOCK UTILITY (hybrid; r>=d vs r<d) ===")
print(f"{'RTT':8} {'pin':5} {'def':3} arm    N   pend via0/via1  comp_p50  comp_p95   class")
PINS = [("pin1", 1, 0, 1), ("pin2", 2, 1, 2), ("pin3", 4, 2, 4)]
for rtt in rtts:
    for pin, deficit, lo, hi in PINS:
        slo = summ((rtt, deficit, lo, "hyb"))
        shi = summ((rtt, deficit, hi, "hyb"))
        if slo["n"] == 0 and shi["n"] == 0:
            continue
        for arm, s, rr, feasible in [("ARQ+%d" % lo, slo, lo, lo >= deficit), ("ARQ+%d" % hi, shi, hi, hi >= deficit)]:
            print(f"{rtt:8} {pin:5} {deficit:<3} {arm:6} {s['n']:<3} {s['pend']:<4} "
                  f"{s['via0']}/{s['via1']:<6} {str(s['comp_p50']):9} {str(s['comp_p95']):9}", end="")
            print()
        # UTILITY-sınıfı: r>=d (hi) completion vs r<d (lo) completion.
        if slo["comp_p50"] is not None and shi["comp_p50"] is not None:
            lo_c, hi_c = slo["comp_p50"], shi["comp_p50"]
            # pratik-eşdeğerlik bandı ±10% veya ±20ms
            band = max(20, int(0.10 * lo_c))
            if hi_c < lo_c - band:
                cls = f"MarginalFecBeneficial (r>=d {hi_c}ms < r<d {lo_c}ms, band±{band})"
            elif hi_c > lo_c + band:
                cls = f"MarginalFecHarmful (r>=d {hi_c}ms > r<d {lo_c}ms)"
            else:
                cls = f"NoMarginalFecUtility_thisCondition (r>=d {hi_c}ms ~= r<d {lo_c}ms, band±{band})"
            # FEC-yol-kullanıldı-mı (decode_via=1)?
            fec_used = shi["via1"] > 0
            cls += f" | FEC-path-used(r>=d)={shi['via1']}/{shi['n']}"
            print(f"         └─ {rtt} {pin} d{deficit}: {cls}")

print("\n=== ISO ANCHOR (delay-bağımsız kombinatoryal; d1) ===")
for rtt in rtts:
    for r in [0, 1]:
        s = summ((rtt, 1, r, "iso"))
        if s["n"] == 0:
            continue
        verdict = "PENDING (FEC-imkansız)" if s["pend"] == s["n"] else ("FEC-COMPLETE" if s["via1"] == (s["n"] - s["pend"]) and s["pend"] == 0 else "mixed")
        print(f"  {rtt} iso d1 r{r}: N={s['n']} pend={s['pend']} via1={s['via1']} → {verdict}")
