#!/usr/bin/env python3
"""AG — netem-özet-toplama: policy×profil mechanism-dimension medyanları (summary-CSV'lerden).
Runner-özeti receiver-outcome (decode/completion) TAŞIMAZ → yalnız-MEKANİZMA-yönü (emit/waste/
displacement/ticket-servisi). Outcome-yönü deterministik-katmanda (named-residual)."""
import csv, sys, glob, statistics as st
from collections import defaultdict
SP = sys.argv[1]
by = defaultdict(list)
for f in glob.glob(f"{SP}/*-summary.csv"):
    for r in csv.DictReader(open(f)):
        rid = r["run_id"]  # prof-policy-rep
        parts = rid.rsplit("-", 2)
        prof, policy = parts[0], parts[1]
        by[(prof, policy)].append(r)
out = open(f"{SP}/netem-summary.csv", "w")
out.write("profile,policy,n,emit_med,waste_max,ndisp_med,rdisp_med,tk_adm_med,tk_on_med,tk_late_med,tk_miss_med\n")
prof_order = ["p1_rtt1s_burst","p2_rtt2500_ge","p3_blackout10","p4_blackout30","p7_gap80_under"]
pol_order = ["leftover","p15","f15","f10"]
def med(rs,k): 
    v=[float(x[k]) for x in rs]; return st.median(v) if v else 0
def mx(rs,k):
    v=[float(x[k]) for x in rs]; return max(v) if v else 0
for prof in prof_order:
    for pol in pol_order:
        rs = by.get((prof,pol))
        if not rs: continue
        out.write(f"{prof},{pol},{len(rs)},{med(rs,'emit'):.0f},{mx(rs,'waste'):.0f},{med(rs,'ndisp'):.0f},{med(rs,'rdisp'):.0f},{med(rs,'tk_adm'):.0f},{med(rs,'tk_on'):.0f},{med(rs,'tk_late'):.0f},{med(rs,'tk_miss'):.0f}\n")
print("yazıldı: netem-summary.csv")
