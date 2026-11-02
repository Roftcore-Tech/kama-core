import os, re, sys, json, gzip, collections
T="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
def kv(line, key):
    m=re.search(r"\b"+re.escape(key)+r"(?:=|: )(\d+)", line); return int(m.group(1)) if m else None
rows=[]
for lane in "bcd":
    od=f"{T}/{lane}/out"
    for cell in sorted(os.listdir(od)):
        cd=f"{od}/{cell}"
        if not os.path.isdir(cd): continue
        for arm in sorted(os.listdir(cd)):
            ad=f"{cd}/{arm}"
            if not os.path.isdir(ad): continue
            for rep in os.listdir(ad):
                rd=f"{ad}/{rep}"
                try: cmd=open(f"{rd}/cmd.txt").read()
                except: continue
                if "kama-product" not in cmd and "kama" not in arm: continue
                r=dict(lane=lane,cell=cell,arm=arm,rep=rep)
                r['fecargs']=" ".join(re.findall(r'--fec-\S+ \S+|--dup-copies \S+', cmd.split("CLI:")[-1]))
                r['transport']=(re.findall(r'--transport (\S+)',cmd) or ['?'])[0]
                ce=open(f"{rd}/c.err",errors='replace').read() if os.path.exists(f"{rd}/c.err") else ""
                se=open(f"{rd}/s.err",errors='replace').read() if os.path.exists(f"{rd}/s.err") else ""
                for l in ce.splitlines():
                    if l.startswith("KP-MANIFEST") and "fec=" in l:
                        m=re.search(r'fec=(ON adaptive \S+|ON static \S+|OFF|\S+)',l); r['fec']=m.group(1) if m else '?'
                        r['profile']=(re.findall(r'profile=(\S+)',l) or ['?'])[0]
                    if l.startswith("KP-CLIENT ") :
                        m=re.search(r'fec=(\d+)',l); r['tx_fec_B']=int(m.group(1)) if m else None
                        r['retx_B']=kv(l,'retx'); r['fec_blocks']=kv(l,'fec_blocks'); r['fec_repairs_tx']=kv(l,'fec_repairs_tx'); r['rec_ctrl']=kv(l,'fec_recovered_ctrl')
                    if l.startswith("KP-ACTIVATED"):
                        r['clamp']=kv(l,'clamp'); r['would_zero']=kv(l,'would_zero'); r['preconv_blk']=kv(l,'preconv_parity_blocked'); r['gate_opened']=kv(l,'gate_opened')
                        m=re.search(r'arbiter=\[([^\]]*)\]',l); r['arb']=m.group(1) if m else ''
                    if l.startswith("KP-ACTIVATION"):
                        r['parity']=kv(l,'parity'); r['retx']=kv(l,'retx'); r['recover_fwd']=kv(l,'recover_fwd'); r['recover_arq']=kv(l,'recover_arq')
                for l in se.splitlines():
                    if "ServerStats" in l:
                        for k in ['fec_repairs_rx','fec_recovered_units','fec_blocks_decoded','fec_recovered_ctrl_tx','delivered']:
                            r['s_'+k]=kv(l,k)
                # wire count
                for side in ['cli','srv']:
                    p=f"{rd}/capb_{side}.csv.gz"
                    if os.path.exists(p):
                        c=collections.Counter()
                        for ln in gzip.open(p,'rt'):
                            if ln.startswith('#'): continue
                            f=ln.rstrip('\n').split(',')
                            if len(f)<7 or f[2]!='17' or not f[6]: continue
                            c[(f[1],f[6][:2])]+=1
                        r['cap_'+side]=dict((f"{k[0]}{k[1]}",v) for k,v in c.items())
                rows.append(r)
json.dump(rows,open(sys.argv[1],'w'))
print(len(rows))
