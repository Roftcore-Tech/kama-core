import json, csv, os, sys, glob, statistics as st
root = sys.argv[1]; cell=sys.argv[2]; offered=float(sys.argv[3])
def pct(xs, q):
    xs = sorted(xs)
    if not xs: return None
    k = (len(xs)-1)*q; f = int(k); c = min(f+1, len(xs)-1)
    return xs[f] + (xs[c]-xs[f])*(k-f)
for arm in ['taban','aday','quinn','tcp']:
    res=[]
    tot_rows=0; pre_pos=0; pre_pos_fin_lt63=0; fin_hist={}
    for rep in sorted(glob.glob(os.path.join(root, cell, arm, 'rep*'))):
        f=os.path.join(rep,'out.jsonl')
        if not os.path.exists(f): continue
        lines=open(f).read().splitlines()
        if not lines: continue
        meta=json.loads(lines[0])['meta']; act=meta.get('activation') or {}
        lat=[]
        for l in lines[1:]:
            e=json.loads(l).get('event')
            if e and e.get('delivered_ns'): lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
        p95=pct(lat,0.95)
        b=act.get('btlbw_bps')
        res.append((b, p95))
        tsf=os.path.join(rep,'ts.csv')
        if os.path.exists(tsf):
            for row in csv.DictReader(open(tsf)):
                tot_rows+=1
                pre=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop']))
                fin=int(row['final_ratio'])
                if pre>0:
                    pre_pos+=1
                    if fin<63: pre_pos_fin_lt63+=1
                    fin_hist[min(fin//10*10,250)]=fin_hist.get(min(fin//10*10,250),0)+1
    if not res: continue
    ps=[p for _,p in res if p is not None]
    print(arm,'N',len(res),'p95_med',round(st.median(ps),2) if ps else None)
    if res[0][0] is not None:
        ratios=sorted(round(b/offered,2) for b,_ in res)
        print('  btlbw/offered sorted:',ratios)
        print('  frac reps btlbw<offered/(1-0.3)=',sum(1 for b,_ in res if b<offered/0.7)/len(res))
        # correlation-ish: p95 for reps with btlbw<offered vs >=
        lo=[p for b,p in res if b<offered]; hi=[p for b,p in res if b>=offered]
        print('  p95 med btlbw<offered:',round(st.median(lo),2) if lo else None,'n',len(lo),' btlbw>=offered:',round(st.median(hi),2) if hi else None,'n',len(hi))
        print('  ts rows',tot_rows,'pre>0',pre_pos,'pre>0&final<63',pre_pos_fin_lt63, 'frac', round(pre_pos_fin_lt63/max(1,pre_pos),3))
        print('  final hist (pre>0 rows):',dict(sorted(fin_hist.items())))
