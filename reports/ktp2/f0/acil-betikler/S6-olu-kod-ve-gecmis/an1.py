import csv, sys, collections
rows=list(csv.DictReader(open(sys.argv[1]),delimiter='\t'))
ad=[r for r in rows if r['fec'].startswith('ON adaptive p0x05')]
def i(x):
    try: return int(x)
    except: return None
n=len(ad)
c=collections.Counter()
btl=[]
for r in ad:
    c['runs']+=1
    if (i(r['fec_repairs_tx']) or 0)>0: c['repairs_tx>0']+=1
    if (i(r['fec_blocks']) or 0)>0: c['fec_blocks>0']+=1
    if (i(r['fec_bytes']) or 0)>0: c['fec_bytes>0']+=1
    if (i(r['clamp']) or 0)>0: c['clamp>0']+=1
    if (i(r['gate_opened']) or 0)>0: c['gate_opened>0']+=1
    if (i(r['would_zero']) or 0)>0: c['would_zero>0']+=1
    if (i(r['preconv_blocked']) or 0)>0: c['preconv_blocked>0']+=1
    if (i(r['recover_fwd']) or 0)>0: c['recover_fwd>0']+=1
    if r['latch'].replace(' ','')!='0,0,0,0,0': c['latch!=0']+=1
    if r['suppress']!='None': c['suppress!=None:'+r['suppress']]+=1
    b=i(r['btlbw'])
    if b is not None:
        btl.append(b)
        cap=min(b*1000//125000,900)
        rr=(min(cap,250)*8+500)//1000
        c['derived_r_final_btlbw=%d'%rr]+=1
for k,v in sorted(c.items()): print(k,v)
btl.sort()
q=lambda p: btl[int(p*(len(btl)-1))]
print('btlbw final B/s: min',btl[0],'p50',q(.5),'p95',q(.95),'max',btl[-1], 'n',len(btl), '>=7875:',sum(1 for b in btl if b>=7875))
