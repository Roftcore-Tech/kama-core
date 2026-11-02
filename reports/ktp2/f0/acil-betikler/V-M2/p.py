import os, re, sys, csv
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
OUT=sys.argv[1]
def num(rx, s):
    m=re.search(rx,s)
    return int(m.group(1)) if m else None
rows=[]
for lane in 'bcd':
    base=os.path.join(ROOT,lane,'out')
    for cell in sorted(os.listdir(base)):
        cp=os.path.join(base,cell)
        if not os.path.isdir(cp): continue
        for arm in sorted(os.listdir(cp)):
            ap=os.path.join(cp,arm)
            if not os.path.isdir(ap): continue
            for rep in sorted(os.listdir(ap)):
                rp=os.path.join(ap,rep)
                cf=os.path.join(rp,'cmd.txt')
                if not os.path.isfile(cf): continue
                cmd=open(cf,errors='replace').read()
                if 'kama-product' not in cmd: continue
                ce=open(os.path.join(rp,'c.err'),errors='replace').read() if os.path.exists(os.path.join(rp,'c.err')) else ''
                se=open(os.path.join(rp,'s.err'),errors='replace').read() if os.path.exists(os.path.join(rp,'s.err')) else ''
                d=dict(lane=lane,cell=cell,arm=arm,rep=rep)
                d['payload']=num(r'--payload (\d+)',cmd)
                d['interval']=num(r'--submit-interval-ms (\d+)',cmd)
                d['msgs']=num(r'--msgs (\d+)',cmd)
                m=re.search(r' fec=(ON adaptive p0x05|ON static[^ ]*|OFF[^ ]*)',ce)
                d['fecmode']=m.group(1) if m else ''
                # client line
                kc=re.search(r'^(KP-CLIENT|MIXED-CLIENT) .*$',ce,re.M)
                kl=kc.group(0) if kc else ''
                d['tx_retx_B']=num(r'retx=(\d+)',kl)
                d['tx_fec_B']=num(r' fec=(\d+) ctrl',kl)
                d['fec_blocks']=num(r' fec_blocks=(\d+)',kl)
                d['fec_repairs_tx']=num(r' fec_repairs_tx=(\d+)',kl)
                d['recovered_ctrl']=num(r' fec_recovered_ctrl=(\d+)',kl)
                act=re.search(r'^KP-ACTIVATION .*$',ce,re.M); al=act.group(0) if act else ''
                d['parity']=num(r'fired=\[.*? parity=(\d+)',al)
                d['parity_denied']=num(r'parity_denied=(\d+)',al)
                d['retx_n']=num(r'fired=\[.*? retx=(\d+)',al)
                d['btlbw']=num(r'btlbw=(\d+)B/s',al)
                d['srtt']=num(r'srtt=(\d+)us',al)
                ad=re.search(r'^KP-ACTIVATED .*$',ce,re.M); adl=ad.group(0) if ad else ''
                d['would_zero']=num(r'would_zero=(\d+)',adl)
                d['clamp']=num(r'fec2=\[.*?clamp=(\d+)',adl)
                d['partial']=num(r'partial_blocks=(\d+)',adl)
                d['preconv_blocked']=num(r'preconv_parity_blocked=(\d+)',adl)
                d['latch']=(re.search(r' latch=\(([^)]*)\)',kl).group(1) if re.search(r' latch=\(([^)]*)\)',kl) else '')
                d['srv_rep_rx']=num(r'fec_repairs_rx: (\d+)',se)
                d['srv_rec']=num(r'fec_recovered_units: (\d+)',se)
                d['srv_dec']=num(r'fec_blocks_decoded: (\d+)',se)
                d['srv_rec_ctrl_tx']=num(r'fec_recovered_ctrl_tx: (\d+)',se)
                rows.append(d)
w=csv.DictWriter(open(OUT,'w',newline=''),fieldnames=list(rows[0].keys()))
w.writeheader(); [w.writerow(r) for r in rows]
print(len(rows))
