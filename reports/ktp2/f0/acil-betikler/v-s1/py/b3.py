import os,re,csv,collections
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
rx=dict(
 rep=re.compile(r'fec_repairs_tx=(\d+)'),
 blk=re.compile(r'fec_blocks=(\d+)'),
 pb=re.compile(r'partial_blocks=(\d+)'),
 wz=re.compile(r'would_zero=(\d+)'),
 cl=re.compile(r'clamp=(\d+)'),
 rt=re.compile(r'risk_t=\((\d+), (\d+), (\d+), (\d+)\)'),
 fe=re.compile(r'flow_end=(\d+)'),
 bw=re.compile(r'est=\[btlbw=(\d+)B/s'),
 latch=re.compile(r'latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)'),
)
tot=0; withrep=0; after=collections.Counter(); inflow=[]; pbpos=0; pb_after=0; pb_notafter=[]
for c in sorted(os.listdir(base)):
    p0=os.path.join(base,c)
    if not os.path.isdir(p0): continue
    for arm in ('taban','aday'):
        d=os.path.join(p0,arm)
        if not os.path.isdir(d): continue
        for rep in os.listdir(d):
            ce=open(os.path.join(d,rep,'c.err')).read()
            g={k:v.search(ce) for k,v in rx.items()}
            if not g['rep']: continue
            tot+=1
            nrep=int(g['rep'].group(1)); pb=int(g['pb'].group(1)); wz=int(g['wz'].group(1))
            tfp=int(g['rt'].group(4)); fe=int(g['fe'].group(1))
            if pb>0:
                pbpos+=1
                if tfp>=fe or tfp==0: pb_after+=1
                else: pb_notafter.append((c,arm,rep,tfp,fe,nrep))
            if nrep>0:
                withrep+=1
                rate=c.split('_')[1]
                if tfp>=fe:
                    after[rate]+=1
                else:
                    # max final ratio in ts
                    mx=0
                    with open(os.path.join(d,rep,'ts.csv')) as fh:
                        for row in csv.DictReader(fh):
                            mx=max(mx,int(row['final_ratio']))
                    inflow.append((c,arm,rep,nrep,wz,mx,int(g['bw'].group(1))))
print('kama runs',tot,'with repairs',withrep,'after flow end',dict(after),sum(after.values()))
print('inflow',len(inflow),'wz>0',sum(1 for x in inflow if x[4]>0),'wz==0',sum(1 for x in inflow if x[4]==0))
for x in inflow:
    if x[4]==0: print('  wz0',x)
print('partial>0',pbpos,'pb with t_first_parity>=flow_end or 0',pb_after,'other',len(pb_notafter))
for x in pb_notafter[:10]: print('  ',x)
