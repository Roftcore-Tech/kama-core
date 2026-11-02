import json, os, sys, glob, statistics as st
root=sys.argv[1]
def pct(xs,q):
    xs=sorted(xs)
    if not xs: return None
    k=(len(xs)-1)*q; f=int(k); c=min(f+1,len(xs)-1); return xs[f]+(xs[c]-xs[f])*(k-f)
for cell in sys.argv[2:]:
    for arm in ['taban','aday']:
        per={}
        nrep=0
        for rep in sorted(glob.glob(os.path.join(root,cell,arm,'rep*'))):
            af=os.path.join(rep,'alice.jsonl'); bf=os.path.join(rep,'bob.jsonl')
            if not (os.path.exists(af) and os.path.exists(bf)): continue
            nrep+=1
            sent={}
            for l in open(af):
                d=json.loads(l)
                if d.get('k')=='send' and d.get('try')==0 and 'i' in d:
                    sent[(d['c'],d['i'])]=d['due']
            for l in open(bf):
                d=json.loads(l)
                if d.get('k')=='rx' and d.get('first') and 'i' in d:
                    key=(d['c'],d['i'])
                    if key in sent:
                        per.setdefault(d['c'],[]).append((d['t']-sent[key])/1000.0)
        s=' '.join(f"{c}:n={len(v)} p50={pct(v,.5):.2f} p95={pct(v,.95):.2f}" for c,v in sorted(per.items()) if c in 'PTS')
        print(cell,arm,'reps',nrep,s)
