#!/usr/bin/env python3
"""LBS (canlı-bütçe-sweep) özet: median-tablo + MS1-pre-declared-validity/equivalence-kuralları."""
import csv, sys, statistics as st
from collections import defaultdict
rows = list(csv.DictReader(open(f"{sys.argv[1]}/lbs-seed-level.csv")))
by = defaultdict(list)
for r in rows: by[(r["cell"], r["arm"])].append(r)
out = open(f"{sys.argv[1]}/lbs-summary.csv", "w")
cols = ["completion","goodput","realization","par_emit","tk_missed","tk_late","tk_on_time",
        "new_data_displaced","retx_displaced","waste","t_p95","dl5","first_par_delay_p50","cap_suppressed"]
out.write("cell,arm,n," + ",".join(c+"_med" for c in cols) + "\n")
for (cell, arm), rs in sorted(by.items()):
    med = lambda k: st.median(float(x[k]) for x in rs)
    out.write(f"{cell},{arm},{len(rs)}," + ",".join(f"{med(c):.4f}" for c in cols) + "\n")
print("yazıldı: lbs-summary.csv")
