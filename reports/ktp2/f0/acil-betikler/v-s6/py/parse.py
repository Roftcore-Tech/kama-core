import os, re, sys, json, glob
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
OUT=sys.argv[1]
rows=[]
def gi(pat, s, cast=int):
    m=re.search(pat, s)
    return cast(m.group(1)) if m else None
def tup(pat, s):
    m=re.search(pat, s)
    if not m: return None
    return [int(x) for x in re.findall(r'-?\d+', m.group(1))]
for lane in 'abcd':
    base=os.path.join(ROOT,lane,'out')
    if not os.path.isdir(base): continue
    for cell in sorted(os.listdir(base)):
        cp=os.path.join(base,cell)
        if not os.path.isdir(cp): continue
        for arm in sorted(os.listdir(cp)):
            ap=os.path.join(cp,arm)
            if not os.path.isdir(ap): continue
            for rep in sorted(os.listdir(ap)):
                rp=os.path.join(ap,rep)
                f=os.path.join(rp,'c.err')
                if not os.path.isfile(f): continue
                try: s=open(f,errors='replace').read()
                except Exception: continue
                r={'lane':lane,'cell':cell,'arm':arm,'rep':rep}
                m=re.search(r'^KP-MANIFEST role=client profile=(\S+).*?fec=(\S+) (\S+) (\S+)', s, re.M)
                if m: r['profile']=m.group(1); r['fec']=' '.join(m.groups()[1:])
                else: r['profile']=None; r['fec']=None
                r['has_kp']= 'KP-CLIENT ' in s
                for k,pat in [('fec_blocks',r'KP-CLIENT .*?fec_blocks=(\d+)'),('fec_repairs_tx',r'KP-CLIENT .*?fec_repairs_tx=(\d+)'),
                              ('fec_bytes',r'KP-CLIENT .*?\bfec=(\d+) ctrl'),('clamp',r'clamp=(\d+)'),('would_zero',r'would_zero=(\d+)'),
                              ('partial_blocks',r'partial_blocks=(\d+)'),('preconv_blocked',r'preconv_parity_blocked=(\d+)'),
                              ('btlbw',r'est=\[btlbw=(\d+)B/s'),('srtt_us',r'srtt=(\d+)us'),('left_preconv_ms',r'left_preconv=(\d+)ms'),
                              ('parity_emitted',r'fired=\[.*?parity=(\d+)'),('parity_denied',r'parity_denied=(\d+)'),('recover_fwd',r'recover_fwd=(\d+)'),
                              ('delivered',r'delivered=(\d+)/'),('flow_end',r'flow_end=(\d+)'),('return_bw',r'return_path=\[bw=(\d+)B/s')]:
                    r[k]=gi(pat,s)
                r['deadline']=gi(r'deadline_ns=(\S+) fusion', s, str)
                r['fusion']=tup(r'fusion=\[([^\]]*)\]', s)
                r['latch']=tup(r'latch=\(([^)]*)\)', s)
                r['risk_t']=tup(r'risk_t=\(([^)]*)\)', s)
                r['arb']=tup(r'arb=\[([^\]]*)\]', s)
                r['suppress']=gi(r'suppress=(\S+) risk_t', s, str)
                r['ts']=os.path.isfile(os.path.join(rp,'ts.csv'))
                rows.append(r)
json.dump(rows, open(OUT,'w'))
print(len(rows))
