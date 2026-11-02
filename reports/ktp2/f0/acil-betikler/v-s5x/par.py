import os, sys, json, statistics, collections
root = sys.argv[1]
cells = sorted(c for c in os.listdir(root) if os.path.isdir(os.path.join(root, c)))
def med(v): return statistics.median(v) if v else None
for cell in cells:
    for arm in ("taban", "aday"):
        ad = os.path.join(root, cell, arm)
        if not os.path.isdir(ad): continue
        pars = []; bw_pos = []; bw_zero = []; bwall_pos=[]; bwall_zero=[]
        for rep in sorted(os.listdir(ad)):
            p = os.path.join(ad, rep, "alice.jsonl")
            if not os.path.exists(p): continue
            last = None; bws = []; bwnz = []
            for l in open(p):
                if '"k":"ts"' not in l: continue
                d = json.loads(l)
                ss = [s for s in d.get("s", []) if s.get("peer") == "bob"]
                if not ss: continue
                s = ss[0]; last = s
                bws.append(s.get("bw", 0))
                if s.get("bw", 0) > 0: bwnz.append(s["bw"])
            if last is None: continue
            par = last.get("par", 0); pars.append(par)
            m = med(bws); m2 = med(bwnz)
            (bw_pos if par > 0 else bw_zero).append(m)
            (bwall_pos if par > 0 else bwall_zero).append(m2)
        print(f"{cell:34s} {arm:5s} par>0 {sum(1 for x in pars if x>0)}/{len(pars)} max {max(pars) if pars else None}"
              f" | bw(med incl0) par>0 {med(bw_pos)} par=0 {med(bw_zero)} | bw(med >0) par>0 {med(bwall_pos)} par=0 {med(bwall_zero)}")
