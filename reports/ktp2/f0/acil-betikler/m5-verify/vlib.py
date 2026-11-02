import gzip, json, csv, os
BASE='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000'
FT={0x00:'data',0x05:'data_frag',0x01:'fec',0x02:'recovered',0x03:'param',0x04:'param_ack',0x06:'cs_probe',0x07:'cs_echo',0x10:'ack',0x14:'ack'}
def reps(arm):
    return sorted([d for d in os.listdir(f'{BASE}/{arm}') if d.startswith('rep')], key=lambda x:int(x[3:]))
def ack_decode(fr):
    if len(fr)<9: return set()
    largest=int.from_bytes(fr[2:4],'big'); b0=int.from_bytes(fr[6:8],'big'); n=fr[8]
    s=set()
    if b0==0: return s
    cur=(largest-b0+1)&0xFFFF
    for k in range(b0): s.add((cur+k)&0xFFFF)
    off=9
    for _ in range(n):
        if off+4>len(fr): break
        gap=int.from_bytes(fr[off:off+2],'big'); ln=int.from_bytes(fr[off+2:off+4],'big'); off+=4
        cur=(cur-gap)&0xFFFF
        for k in range(ln): s.add((cur-k)&0xFFFF)
        cur=(cur-ln)&0xFFFF
    return s
def read_cap(path):
    rows=[]
    with gzip.open(path,'rt',errors='replace') as f:
        for line in f:
            if not line or line[0]=='#': continue
            p=line.rstrip('\n').split(',',6)
            if len(p)<7 or p[2]!='17' or not p[6]: continue
            t=int(p[0]); d=p[1]; ipl=int(p[3]); x=p[6]
            if ':' in x:
                h,tl=x.split(':',1); fr=bytes.fromhex(h); tail=bytes.fromhex(tl)
                full=False
            else:
                b=bytes.fromhex(x)
                if len(b)<17: continue
                fr=b[:-16]; tail=b[-16:]; full=True
            pn=int.from_bytes(tail[:8],'big'); ft=FT.get(fr[0],'other')
            r={'t':t,'dir':d,'ipl':ipl,'udp':ipl-28,'pn':pn,'type':ft,'sport':int(p[4]),'dport':int(p[5])}
            if ft in ('data','data_frag') and len(fr)>=11: r['unit']=int.from_bytes(fr[3:11],'big')
            if ft=='ack' and full: r['acked16']=ack_decode(fr)
            rows.append(r)
    rows.sort(key=lambda r:r['t'])
    return rows
def load(arm,rep):
    d=f'{BASE}/{arm}/{rep}'
    L=[json.loads(l) for l in open(d+'/out.jsonl')]
    meta=L[0]['meta']; ev=[l['event'] for l in L if 'event' in l]
    t0=meta['t0_unix_ns']; off=meta.get('events_epoch_offset_ns',0)
    offer={e['msg_id']: t0+e['sent_ns']-off for e in ev if e.get('sent_ns') is not None}
    conf={e['msg_id']: (t0+e['delivered_ns']-off if e.get('delivered_ns') is not None else None) for e in ev}
    lat={e['msg_id']: ((e['delivered_ns']-e['sent_ns'])/1e9 if e.get('delivered_ns') is not None else None) for e in ev}
    cli=read_cap(d+'/capb_cli.csv.gz'); srv=read_cap(d+'/capb_srv.csv.gz')
    ts=list(csv.DictReader(open(d+'/ts.csv')))
    err=open(d+'/c.err').read()
    return dict(d=d,meta=meta,t0=t0,off=off,offer=offer,conf=conf,lat=lat,cli=cli,srv=srv,ts=ts,err=err)
def pctl(v,q):
    v=sorted(v)
    if not v: return None
    k=(len(v)-1)*q; f=int(k); c=min(f+1,len(v)-1)
    return v[f]+(v[c]-v[f])*(k-f)
