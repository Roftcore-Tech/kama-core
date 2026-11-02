import json, gzip, math, sys, os, statistics as st
R=sys.argv[1]
def qnr(v,p):
    v=sorted(v); k=max(1,math.ceil(p*len(v))); return v[k-1]
def rd(p):
    out=[]
    with gzip.open(p,'rt') as f:
        for l in f:
            if l.startswith('#'): continue
            a=l.rstrip('\n').split(',')
            if len(a)<7: continue
            out.append((int(a[0]),a[1],int(a[2]),int(a[3]),int(a[4]),int(a[5]),a[6]))
    return out
def port_of(d):
    c=open(f"{d}/cmd.txt").read()
    import re
    m=re.search(r'--server 10\.10\.1\.1:(\d+)',c); return int(m.group(1))
for arm in sys.argv[2].split(','):
    agg=dict(qw50=[],qw95=[],net50=[],size_mode=[],srv_pkts=[],srv_pkts_per_data=[],first_tx_after_submit_le_10ms=[], data_tx=[],cli_pkts_10s=[])
    for rep in range(1,31):
        d=f"{R}/{arm}/rep{rep}"
        L=[json.loads(l) for l in open(f"{d}/out.jsonl")]
        meta=L[0]['meta']; ev=[x['event'] for x in L[1:] if 'event' in x]
        t0=meta['t0_unix_ns']; off=meta['events_epoch_offset_ns']
        su={e['msg_id']:t0+e['sent_ns']-off for e in ev}
        port=port_of(d)
        cli=rd(f"{d}/capb_cli.csv.gz"); srv=rd(f"{d}/capb_srv.csv.gz")
        co=[r for r in cli if r[1]=='o' and r[2]==17 and r[5]==port]
        so=[r for r in srv if r[1]=='o' and r[2]==17 and r[4]==port]
        agg['srv_pkts'].append(len(so))
        if arm in('taban','aday'):
            first={}
            ndata=0
            for t,dr,pr,ipl,sp,dp,x in co:
                h=x.split(':')[0]
                b=bytes.fromhex(h)
                if b and b[0]==0x00 and len(b)>=11:
                    ndata+=1
                    u=int.from_bytes(b[3:11],'big')
                    if u not in first: first[u]=t
            qw=[(first[m]-su[m])/1e9 for m in su if m in first]
            agg['qw50'].append(st.median(qw)); agg['qw95'].append(qnr(qw,.95))
            agg['first_tx_after_submit_le_10ms'].append(sum(1 for x in qw if x<=0.010)/len(su))
            agg['data_tx'].append(ndata)
            sizes=[r[3] for r in co if bytes.fromhex(r[6].split(':')[0])[:1]==b'\x00']
            agg['size_mode'].append(st.mode(sizes))
            agg['srv_pkts_per_data'].append(len(so)/max(1,ndata))
        else:
            # quinn: short header 1-RTT packets after t0
            sh=[r for r in co if r[0]>=t0 and not (int(r[6][:2],16)&0x80)]
            sizes=[r[3] for r in sh]
            agg['size_mode'].append(st.mode(sizes) if sizes else 0)
            agg['data_tx'].append(sum(1 for r in sh if r[3]>500))
            sso=[r for r in so if r[0]>=t0]
            agg['srv_pkts_per_data'].append(len(sso)/max(1,sum(1 for r in sh if r[3]>500)))
            # packets departing in first 10 s after t0 vs messages offered in 10 s (~35)
            agg['cli_pkts_10s'].append(sum(1 for r in sh if r[0]<t0+10e9 and r[3]>500))
    print(arm, {k:(round(st.median(v),3) if v else None) for k,v in agg.items()})
