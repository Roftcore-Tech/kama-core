import json,sys,glob,os
for cell in sys.argv[1:]:
  for rep in sorted(glob.glob(cell+'/taban/rep*'))+sorted(glob.glob(cell+'/aday/rep*')):
    p=rep+'/alice.jsonl'
    if not os.path.exists(p): continue
    t0=None; prev=0; inc=[]; sends=[]
    for l in open(p):
        o=json.loads(l)
        if o['k']=='hdr': continue
        if o['k']=='mark' and t0 is None: pass
        if o['k']=='ts' and o['s']:
            if t0 is None: t0=o['t']
            s=o['s'][0]
            if s['par']>prev: inc.append((round((o['t']-t0)/1000,1),s['par']-prev,s['st'],s['pr'],s['rec']['txq'])); prev=s['par']
        if o['k']=='send':
            sends.append(o)
    if inc: print(os.path.basename(cell),rep.split('/')[-2],rep.split('/')[-1],inc[:12])
