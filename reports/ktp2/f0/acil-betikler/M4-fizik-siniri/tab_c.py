import json, csv, sys
from collections import defaultdict
sys.path.insert(0, ".")
from sim import pct
M={}
for l in open("model_c.jsonl"):
    r=json.loads(l); M[(r["cell"],r["pol"])]=r
H={}
for r in csv.DictReader(open(sys.argv[1]+"/hucreler.csv")):
    H[(r["hucre"],r["kol"])]=r
# kilitsiz koşular (kosular.csv)
KS=defaultdict(list)
for r in csv.DictReader(open(sys.argv[1]+"/kosular.csv")):
    KS[(r["hucre"],r["kol"])].append(r)
def ana_key(cell): return "T2_800" if "d1000" in cell else "T1_800"
f=lambda x: "–" if x is None else f"{x:.3f}"
cells=sorted({c for c,_ in M})
print("| hücre | ölçü | oracle | rfc | rs2 (K2,E64) | rs4 (K4,E64) | ürün K8R2E1024 | K8R2E64 | arq taban·aday (ölçülen) | arq kilitsiz med (n) | rs2 taban·aday | rs4 taban·aday | uyar taban·aday |")
print("|"+"---|"*13)
for c in cells:
    k=ana_key(c)
    g=lambda p: M[(c,p)][k]
    def meas(arm):
        a=H.get((c,arm+"_taban")); b=H.get((c,arm+"_aday"))
        return f"{f(float(a['ana_med'])) if a else '–'}·{f(float(b['ana_med'])) if b else '–'}"
    unl=[float(r["ana"]) for r in KS.get((c,"arq_taban"),[])+KS.get((c,"arq_aday"),[]) if r.get("kilitli") in ("0","False","false","") and r.get("ana") not in (None,"")]
    print(f"| {c} | {k} | {f(g('oracle'))} | {f(g('rfc'))} | {f(g('rs2'))} | {f(g('rs4'))} | {f(g('urun_K8R2E1024'))} | {f(g('K8R2E64'))} | {meas('arq')} | {f(pct(unl,.5)) if unl else '–'} ({len(unl)}) | {meas('rs2')} | {meas('rs4')} | {meas('uyar')} |")
