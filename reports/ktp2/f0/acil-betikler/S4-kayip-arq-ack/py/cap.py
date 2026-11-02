import gzip,json,os,sys,statistics as st
def q(v,p):
    v=sorted(v)
    if not v: return float('nan')
    return v[max(0,min(len(v)-1,int(round(p*(len(v)-1)))))]
def rows(path):
    with gzip.open(path,'rt') as f:
        for l in f:
            if l.startswith('#'): continue
            p=l.rstrip('\n').split(',')
            if len(p)<7 or p[2]!='17': continue
            yield int(p[0]),p[1],int(p[3]),p[6]
def tag_pn(x):
    tail = x.split(':')[1] if ':' in x else x[-32:]
    return int(tail[:16],16)
def frame(x):
    if ':' in x: return x.split(':')[0]
    return x[:-32]
def parse_ack(fh):
    b=bytes.fromhex(fh)
    if not b or b[0] not in (0x14,0x10): return None
    largest=int.from_bytes(b[2:4],'big'); delay=int.from_bytes(b[4:6],'big'); b0=int.from_bytes(b[6:8],'big'); rc=b[8]
    ranges=[]; o=9
    for i in range(rc):
        ranges.append((int.from_bytes(b[o:o+2],'big'),int.from_bytes(b[o+2:o+4],'big'))); o+=4
    return largest,delay,b0,ranges
def acked(largest_full,b0,ranges):
    s=set(); cur=largest_full-b0+1
    s.update(range(cur,largest_full+1))
    for g,l in ranges:
        cur-=g
        if l==0: continue
        s.update(range(cur-l+1,cur+1)); cur-=l
    return s
def resolve(exp,w):
    # nearest full value with low16==w to exp
    base=exp & ~0xFFFF; c=base|w
    best=min((c-0x10000,c,c+0x10000),key=lambda v:abs(v-exp))
    return best
def analyze(rep):
    cli=os.path.join(rep,'capb_cli.csv.gz'); srv=os.path.join(rep,'capb_srv.csv.gz')
    meta=None; ev={}
    for l in open(os.path.join(rep,'out.jsonl')):
        d=json.loads(l)
        if 'meta' in d: meta=d['meta']
        elif 'event' in d: ev[d['event']['msg_id']]=d['event']
    t0=meta['t0_unix_ns']+meta.get('events_epoch_offset_ns',0)
    tx=[] # (t,pn,unit)
    ackrx=[] # (t, acked set, largest)
    maxpn=0
    for t,d,ln,x in rows(cli):
        if d=='o' and x[:2]=='00' and ln>40:
            fr=frame(x); pn=tag_pn(x); unit=int(fr[6:22],16)
            tx.append((t,pn,unit)); maxpn=max(maxpn,pn)
        elif d=='o':
            try: maxpn=max(maxpn,tag_pn(x))
            except: pass
        elif d=='i':
            a=parse_ack(frame(x))
            if a:
                lf=resolve(maxpn,a[0]); ackrx.append((t,acked(lf,a[2],a[3]),lf,a[1]))
    arr={}
    srv_ack_o=[]
    for t,d,ln,x in rows(srv):
        if d=='i' and x[:2]=='00' and ln>40:
            pn=tag_pn(x); arr.setdefault(pn,t)
        elif d=='o':
            a=parse_ack(frame(x))
            if a: srv_ack_o.append((t,a))
    return meta,ev,t0,tx,ackrx,arr,srv_ack_o
def main(cell,arm,maxrep=30):
    base=os.path.join(cell,arm)
    Q=[];DET=[];FB3=[];FB1=[];POST=[];CH=[];RET=[];HOLD=[];ACKINT=[]; nlost=0; nretx_after_ok=0; nretx=0; firstok=0; nunits=0
    for r in sorted(os.listdir(base))[:maxrep]:
        rep=os.path.join(base,r)
        if not os.path.exists(os.path.join(rep,'capb_cli.csv.gz')): continue
        meta,ev,t0,tx,ackrx,arr,sacks=analyze(rep)
        by={}
        for t,pn,u in tx: by.setdefault(u,[]).append((t,pn))
        for u,l in by.items():
            if u not in ev: continue
            nunits+=1
            sub=t0+ev[u]['sent_ns']
            Q.append((l[0][0]-sub)/1e9)
            ok=[i for i,(t,pn) in enumerate(l) if pn in arr]
            if ok and ok[0]==0: firstok+=1
            for i,(t,pn) in enumerate(l[:-1]):
                nretx+=1
                if ok and i>ok[0]: nretx_after_ok+=1
                if pn in arr: continue
                nlost+=1
                tn=l[i+1][0]
                DET.append((tn-t)/1e9)
                # earliest ack at client acking pn' >= pn+3 after t
                t3=next((ta for ta,s,lf,_ in ackrx if ta>t and lf>=pn+3 and any(p>=pn+3 for p in s)),None)
                t1=next((ta for ta,s,lf,_ in ackrx if ta>t and any(p>pn for p in s)),None)
                if t3: FB3.append((t3-t)/1e9); POST.append((tn-t3)/1e9)
                if t1: FB1.append((t1-t)/1e9)
            if ok:
                CH.append(ok[0])
        # ack hold at server: for each arrival, first server ack whose largest>=pn
        sl=sorted(arr.items(),key=lambda kv:kv[1])
        st_ack=[(t,a) for t,a in sacks]
        j=0
        prev=None
        for t,a in st_ack:
            if prev: ACKINT.append((t-prev)/1e9)
            prev=t
        for pn,ta in sl:
            while j<len(st_ack) and st_ack[j][0]<ta: j+=1
            if j<len(st_ack): HOLD.append((st_ack[j][0]-ta)/1e9)
    def s(name,v): print('  %-34s n=%5d p50=%6.2f p90=%6.2f p95=%6.2f mean=%6.2f'%(name,len(v),q(v,.5),q(v,.9),q(v,.95),st.mean(v) if v else float('nan')))
    print(arm,'units',nunits,'first-tx ok',firstok,'lost tx',nlost,'re-tx total',nretx,'re-tx after unit already arrived',nretx_after_ok)
    s('queue: submit->first wire tx',Q)
    s('lost tx -> next tx same unit',DET)
    s('lost tx -> 1st ACK w/ later pn',FB1)
    s('lost tx -> 1st ACK covering pn+3',FB3)
    s('ACK(pn+3) -> next tx same unit',POST)
    s('srv arrival -> next srv ACK emit',HOLD)
    s('srv ACK inter-emit',ACKINT)
    from collections import Counter
    print('  copies-before-success hist',sorted(Counter(CH).items()))
if __name__=='__main__':
    main(sys.argv[1],sys.argv[2],int(sys.argv[3]) if len(sys.argv)>3 else 30)
