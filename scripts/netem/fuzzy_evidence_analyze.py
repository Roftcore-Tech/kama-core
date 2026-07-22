#!/usr/bin/env python3
"""FE+FF — fuzzy-calibration-evidence analizi.
FE: FusionInputs-observed-distribution + membership-support-vs-configured + traffic-weighted-no-rule.
FF: fixed-r-oracle-Pareto (profil×r dl_op/goodput) + current-fuzzy-vs-oracle-sınıflandırması.
Fuzzy-config = FROZEN-fixture (analytic-membership; rule_coverage_map-eval'iyle-aynı; anchor-validated).
Bölme-sıfır='NA'-explicit. Inference-birimi=run/profil (event-bağımsız-örnek-DEĞİL)."""
import csv, sys, glob, statistics as st
from collections import defaultdict

SP = sys.argv[1]
# ── Fixture breakpoints (FROZEN; fusion.rs) ──
def trap(x,a,b,c,d):
    if x<a or x>d: return 0.0
    if a<=x<b: return (x-a)/(b-a) if b>a else 1.0
    if b<=x<=c: return 1.0
    if c<x<=d: return (d-x)/(d-c) if d>c else 1.0
    return 0.0
BIG=1<<62
def loss_mem(x): return dict(low=trap(x,0,0,200000,300000), med=trap(x,200000,300000,500000,600000), high=trap(x,500000,600000,BIG,BIG))
def jit_mem(x): return dict(low=trap(x,0,0,100000,200000), med=trap(x,100000,200000,400000,500000), high=trap(x,400000,500000,BIG,BIG))
def ack_mem(x): return dict(low=trap(x,0,0,300,500), med=trap(x,300,500,700,900), high=trap(x,700,900,BIG,BIG))
import math
def log2fp(bps): return int(round(math.log2(max(bps,1))*1000)) if bps>0 else 0
def bw_mem(bps):
    x=log2fp(bps); l50,l100,l150,l200=[log2fp(v) for v in (50000,100000,150000,200000)]
    return dict(low=trap(x,0,0,l50,l100), med=trap(x,l50,l100,l150,l200), high=trap(x,l150,l200,BIG,BIG))
def eval_fuzzy(loss,bw,jit,ack):
    l,b,j,a=loss_mem(loss),bw_mem(bw),jit_mem(jit),ack_mem(ack)
    r1=l['low']; r2=min(l['high'],b['high'],j['low']); r3=min(l['med'],b['med']); r4=min(l['low'],j['high'])
    no_rule = (r1==0 and r2==0 and r3==0 and r4==0)
    tot=r1+r2+r3+r4
    ratio = 0 if no_rule else round((r1*0+r2*800+r3*500+r4*0)/tot)
    emerg = min(l['high'],b['low'],a['low'])>500
    return dict(r=[r1,r2,r3,r4], no_rule=no_rule, ratio=ratio, emergency=emerg, mem=dict(loss=l,bw=b,jit=j,ack=a))

# ── FE: fuzzy-event dataset ──
events=[]
for f in glob.glob(f"{SP}/*-fuzzy.csv"):
    for r in csv.DictReader(open(f)):
        events.append(r)
def pct(vals,p):
    if not vals: return 0
    v=sorted(vals); return v[min(len(v)-1,int(len(v)*p))]
fe=open(f"{SP}/fe-input-distribution.csv","w")
fe.write("input,min,p1,p5,p25,p50,p75,p95,p99,max,n\n")
for col in ["loss_windowed_ppm","qd_ratio_permille","jitter_ppm","ack_confidence_permille","bandwidth_bps","btlbw","srtt_us","pacing_rate"]:
    vals=[float(e[col]) for e in events if e.get(col)]
    if vals: fe.write(f"{col},{min(vals):.0f},{pct(vals,.01):.0f},{pct(vals,.05):.0f},{pct(vals,.25):.0f},{pct(vals,.5):.0f},{pct(vals,.75):.0f},{pct(vals,.95):.0f},{pct(vals,.99):.0f},{max(vals):.0f},{len(vals)}\n")
# Membership support (empirical: fraction of events with membership>0)
ms=open(f"{SP}/fe-membership-support.csv","w")
ms.write("variable,band,empirical_support_frac,mean_degree\n")
degs=defaultdict(list)
for e in events:
    ev=eval_fuzzy(float(e["loss_windowed_ppm"]),float(e["bandwidth_bps"]),float(e["jitter_ppm"]),float(e["ack_confidence_permille"]))
    for var in ["loss","bw","jit","ack"]:
        for band in ["low","med","high"]:
            degs[(var,band)].append(ev["mem"][var][band])
for (var,band),ds in sorted(degs.items()):
    n=len(ds); sup=sum(1 for d in ds if d>0)/n if n else 0
    ms.write(f"{var},{band},{sup:.4f},{st.mean(ds):.4f}\n")
# Traffic-weighted coverage
n=len(events); nr=sum(1 for e in events if e["no_rule"]=="1")
zr=sum(1 for e in events if e["fuzzy_ratio_permille"]=="0")
modes=defaultdict(int)
for e in events: modes[e["mode"]]+=1
cov=open(f"{SP}/fe-empirical-coverage.csv","w")
cov.write("metric,value\n")
cov.write(f"total_events,{n}\n")
cov.write(f"no_rule_frac,{nr/n:.4f}\n" if n else "no_rule_frac,NA\n")
cov.write(f"ratio_zero_frac,{zr/n:.4f}\n" if n else "ratio_zero_frac,NA\n")
for m,c in sorted(modes.items()): cov.write(f"mode_{m},{c}\n")

# ── FF: fixed-r oracle (profil×r → dl_op/goodput medyan; rx-ledger) ──
# rx-ledger: run-id=prof-rN-rep → dl_op-rate
orc=defaultdict(list)
for f in glob.glob(f"{SP}/*-rxledger.csv"):
    rows=list(csv.DictReader(open(f)))
    if not rows: continue
    rid=rows[0]["run_id"]; parts=rid.rsplit("-",2)  # prof, rN, rep
    prof=parts[0]; r=parts[1]
    nb=len(rows); dlop=sum(int(x["dl_op"]) for x in rows)/nb if nb else 0
    comp=sum(1 for x in rows if int(x["complete_ms"])>=0)/nb if nb else 0
    orc[(prof,r)].append((dlop,comp))
ff=open(f"{SP}/ff-oracle.csv","w")
ff.write("profile,r,n,dl_op_med,complete_med\n")
prof_dlop=defaultdict(dict)
for (prof,r),vs in sorted(orc.items()):
    dl=st.median(x[0] for x in vs); cp=st.median(x[1] for x in vs)
    ff.write(f"{prof},{r},{len(vs)},{dl:.4f},{cp:.4f}\n")
    prof_dlop[prof][r]=dl
# Oracle label per profile (does any r>0 beat r0 by >0.01?)
lab=open(f"{SP}/ff-oracle-label.csv","w")
lab.write("profile,dlop_r0,dlop_r1,dlop_r2,dlop_r4,best_r,delta_best_vs_r0,oracle_label,current_fuzzy_class\n")
for prof,rm in sorted(prof_dlop.items()):
    r0=rm.get("r0",0)
    best_r=max(rm,key=lambda k:rm[k]); best=rm[best_r]
    delta=best-r0
    # ÇERÇEVE: r0=ARQ+0-proaktif-FEC; r>0=ARQ+N-proaktif-FEC. Karşılaştırma=marjinal-FEC-utility-over-ARQ.
    if delta>0.01: olab=f"MarginalFecUtility_{best_r.upper()}"  # ARQ+FEC-hibrit bu-koşulda-avantajlı
    elif max(rm.values())-min(rm.values())<=0.01: olab="NoMarginalFecUtility_thisCondition"  # ARQ-substrate-deadline'ı-zaten-karşıladı
    else: olab="Inconclusive"
    # current fuzzy in these profiles: loss stays <200k → R1 → ratio=0 → PreferR0-equivalent.
    # class: eğer oracle=EquivalentSet/PreferR0 → CorrectZero; eğer PreferR2/R4 ama fuzzy-ratio=0 → UnderProtect
    # cls: controller-proaktif-FEC=0-kararı bu-koşulda-doğru-mu (ARQ-substrate-yeterliyse EVET; global-DEĞİL).
    if olab.startswith("NoMarginalFecUtility"): cls="CorrectZeroProactiveFec_thisCondition"
    elif olab.startswith("MarginalFecUtility"): cls="UnderProvisionedProactiveFec"
    else: cls="Inconclusive"
    lab.write(f"{prof},{r0:.4f},{rm.get('r1',0):.4f},{rm.get('r2',0):.4f},{rm.get('r4',0):.4f},{best_r},{delta:.4f},{olab},{cls}\n")
print("yazıldı: fe-input-distribution / fe-membership-support / fe-empirical-coverage / ff-oracle / ff-oracle-label")
