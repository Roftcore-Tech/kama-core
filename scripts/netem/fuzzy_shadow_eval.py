#!/usr/bin/env python3
"""FG — SHADOW-mode candidate değerlendirmesi (actuation-YOK; salt-analiz).
Baseline=FROZEN-fixture; Candidate=aday-eşik-seti. Her-recorded-event'te baseline-vs-candidate-ratio +
oracle-label karşılaştırılır. Oracle (FF): TÜM-profil EquivalentSet (FEC-faydasız) → her-non-zero-ratio
= OverProtection. Çıktı: disagreement + over/under-protection oranları."""
import csv, glob, sys, math
from collections import defaultdict
SP=sys.argv[1]
BIG=1<<62
def trap(x,a,b,c,d):
    if x<a or x>d: return 0.0
    if a<=x<b: return (x-a)/(b-a) if b>a else 1.0
    if b<=x<=c: return 1.0
    if c<x<=d: return (d-x)/(d-c) if d>c else 1.0
    return 0.0
def log2fp(bps): return int(round(math.log2(max(bps,1))*1000)) if bps>0 else 0
def make_eval(loss_lo, loss_hi):
    # loss_lo=(c,d)-LOW-shoulder; loss_hi=(a,b)-HIGH-shoulder; med-arası.
    lc,ld=loss_lo; ha,hb=loss_hi
    def ev(loss,bw,jit,ack):
        l=dict(low=trap(loss,0,0,lc,ld), med=trap(loss,lc,ld,ha,hb), high=trap(loss,ha,hb,BIG,BIG))
        x=log2fp(bw); l50,l100,l150,l200=[log2fp(v) for v in (50000,100000,150000,200000)]
        b=dict(low=trap(x,0,0,l50,l100),med=trap(x,l50,l100,l150,l200),high=trap(x,l150,l200,BIG,BIG))
        j=dict(low=trap(jit,0,0,100000,200000),med=0,high=trap(jit,400000,500000,BIG,BIG))
        r1=l['low']; r2=min(l['high'],b['high'],j['low']); r3=min(l['med'],b['med']); r4=min(l['low'],j['high'])
        tot=r1+r2+r3+r4
        return 0 if tot==0 else round((r2*800+r3*500)/tot)
    return ev
BASE=make_eval((200000,300000),(500000,600000))               # FROZEN-fixture
CAND_A=make_eval((30000,60000),(90000,150000))                 # aggressive: eşikler-gözlem-zarfına-çekildi
events=[]
for f in glob.glob(f"{SP}/*-fuzzy.csv"):
    events+=list(csv.DictReader(open(f)))
n=len(events); over=0; under=0; agree=0; base_nz=0; cand_nz=0
for e in events:
    loss=float(e["loss_windowed_ppm"]); bw=float(e["bandwidth_bps"]); jit=float(e["jitter_ppm"]); ack=float(e["ack_confidence_permille"])
    rb=BASE(loss,bw,jit,ack); rc=CAND_A(loss,bw,jit,ack)
    if rb>0: base_nz+=1
    if rc>0: cand_nz+=1
    # Oracle=EquivalentSet-her-profil → doğru-karar=r0 (ratio-0). Non-zero=OverProtect.
    if rc==rb: agree+=1
    if rc>0 and rb==0: over+=1   # candidate-fazladan-koruma (oracle-r0 → OverProtect)
    if rc==0 and rb>0: under+=1
print(f"events={n}")
print(f"baseline_nonzero_frac={base_nz/n:.4f}  candidate_nonzero_frac={cand_nz/n:.4f}")
print(f"agree_frac={agree/n:.4f}  candidate_overprotect_frac={over/n:.4f}  underprotect_frac={under/n:.4f}")
print(f"# oracle=EquivalentSet(TÜM-profil) → her-nonzero=OverProtection-no-benefit")
