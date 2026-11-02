import gzip,os,json,statistics as st
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000/'
for arm in ('taban','aday'):
    rates=[];backlog_end=[]
    for rep in os.listdir(base+arm):
        d=base+arm+'/'+rep
        meta=json.loads(open(d+'/out.jsonl').readline())['meta']; t0=meta['t0_unix_ns']
        B=0; first=set(); 
        for line in gzip.open(d+'/capb_cli.csv.gz','rt'):
            f=line.split(',')
            if f[1]!='o': continue
            t=(int(f[0])-t0)/1e9
            h=f[6].split(':')[0]
            if h[:2]=='00' and len(h)>=22:
                if t<=28.5: B+=int(f[3])-28  # UDP payload bytes
                if t<=28.5: first.add(int(h[6:22],16))
        rates.append(B/28.5); backlog_end.append(100-len(first))
    print(arm,'UDP DATA emission rate in [0,28.5s] median %.0f B/s [%.0f..%.0f]; offered 3604; distinct units not yet sent at 28.5s median %d'%(st.median(rates),min(rates),max(rates),st.median(backlog_end)))
