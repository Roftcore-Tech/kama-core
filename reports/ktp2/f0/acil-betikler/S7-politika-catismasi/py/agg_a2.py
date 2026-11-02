import json, os, sys, glob, statistics as st
root=sys.argv[1]
def pct(xs,q):
    xs=sorted(xs)
    if not xs: return None
    k=(len(xs)-1)*q; f=int(k); c=min(f+1,len(xs)-1); return xs[f]+(xs[c]-xs[f])*(k-f)
for cell in sys.argv[2:]:
    for arm in ['taban','aday']:
        p95s=[]
        for rep in sorted(glob.glob(os.path.join(root,cell,arm,'rep*'))):
            af=os.path.join(rep,'alice.jsonl'); bf=os.path.join(rep,'bob.jsonl')
            if not (os.path.exists(af) and os.path.exists(bf)): continue
            sent={}
            for l in open(af):
                d=json.loads(l)
                if d.get('k')=='send' and d.get('try')==0 and d.get('c')=='T' and 'i' in d: sent[d['i']]=d['due']
            lat=[]
            for l in open(bf):
                d=json.loads(l)
                if d.get('k')=='rx' and d.get('first') and d.get('c')=='T' and d.get('i') in sent: lat.append((d['t']-sent[d['i']])/1000)
            if lat: p95s.append(pct(lat,.95))
        print(cell,arm,'TEXT per-rep p95 median',round(st.median(p95s),1),'n',len(p95s))
