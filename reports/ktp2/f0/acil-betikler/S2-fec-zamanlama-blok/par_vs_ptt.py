import json,sys,glob,os,collections
root=sys.argv[1]
tot=collections.Counter()
for cell in sorted(glob.glob(root+'/a_*')):
  if not os.path.isdir(cell): continue
  c=collections.Counter()
  for rep in glob.glob(cell+'/*/rep*'):
    p=rep+'/alice.jsonl'
    if not os.path.exists(p): continue
    ts=[];sends=[]
    for l in open(p):
        o=json.loads(l)
        if o['k']=='ts' and o['s']: ts.append((o['t'],o['s'][0]['par']))
        elif o['k']=='send': sends.append((o['t'],o['c']))
    # spurts: group P sends with gap < 1000ms
    P=sorted(t for t,k in sends if k=='P')
    spurts=[]
    for t in P:
        if spurts and t-spurts[-1][1]<1000: spurts[-1][1]=t
        else: spurts.append([t,t])
    prev_t,prev=None,0
    for t,par in ts:
        if par>prev:
            n=par-prev
            # interval (prev_t,t] of increment
            mid=t
            inside=any(a<=mid<=b+250 for a,b in spurts)
            c['in_spurt' if inside else 'out_spurt']+=n
        prev,prev_t=par,t
    c['ptt_frames']+=len(P); c['runs']+=1
  print(os.path.basename(cell),dict(c)); tot+=c
print('TOTAL',dict(tot))
