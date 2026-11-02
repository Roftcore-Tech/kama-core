import json, math, os, sys, statistics, re
ROOT = sys.argv[1]; CELLS = sys.argv[2].split(','); ARMS = ('taban','aday','quinn','tcp')
INF = math.inf
def nr(v, p):  # nearest-rank, INF included
    v = sorted(v); 
    if not v: return float('nan')
    return v[max(1, math.ceil(p*len(v)))-1]
def lin(v, p):
    v = sorted(x for x in v if not math.isinf(x))
    if not v: return None
    if len(v)==1: return v[0]
    pos = p*(len(v)-1); lo=math.floor(pos); hi=min(lo+1,len(v)-1)
    return v[lo]+(v[hi]-v[lo])*(pos-lo)
def med(v):
    v=[x for x in v if x is not None]
    return statistics.median(v) if v else None
def run(d, D):
    try:
        L = open(os.path.join(d,'out.jsonl')).read().splitlines()
    except OSError:
        return None
    meta=None; ev=[]
    for l in L:
        try: j=json.loads(l)
        except: continue
        if 'meta' in j: meta=j['meta']
        elif 'event' in j: ev.append(j['event'])
    if meta is None: return None
    n = meta.get('msg_count', len(ev))
    sent = {e['msg_id']: e['sent_ns'] for e in ev}
    conf = [ (e['delivered_ns']-e['sent_ns'])/1e9 if e.get('delivered_ns') is not None else INF for e in ev]
    conf += [INF]*(n-len(conf))
    t0 = meta.get('t0_unix_ns') or 0; off = meta.get('events_epoch_offset_ns') or 0
    tr = meta.get('transport','')
    if tr.startswith('tcp') and t0==0:
        t0 = int(float(open(os.path.join(d,'t_start')).read())*1e9) + meta.get('setup_ns',0)
    first={}; rxn=0
    try:
        for l in open(os.path.join(d,'rx_log.jsonl')):
            j=json.loads(l)
            if 'u' in j:
                rxn+=1
                u=j['u']
                if u not in first or j['t']<first[u]: first[u]=j['t']
    except OSError: pass
    ow=[]
    for i in range(n):
        if i in first and i in sent and t0:
            ow.append(max(0, first[i]-(t0+sent[i]-off))/1e9)
        else: ow.append(INF)
    rc = open(os.path.join(d,'rc')).read().strip() if os.path.exists(os.path.join(d,'rc')) else None
    # conf-delivered but not in rxlog, and vice versa
    c_not_rx = sum(1 for i,e in enumerate(ev) if e.get('delivered_ns') is not None and i not in first)
    rx_not_c = sum(1 for i,e in enumerate(ev) if e.get('delivered_ns') is None and i in first)
    return dict(n=n, rc=rc, t0=t0, conf_nr95=nr(conf,.95), conf_lin95=lin(conf,.95), confD=sum(1 for x in conf if x<=D)/n,
                ow_nr95=nr(ow,.95), ow_lin95=lin(ow,.95), owD=sum(1 for x in ow if x<=D)/n, deliv_any=sum(1 for x in conf if not math.isinf(x))/n,
                c_not_rx=c_not_rx, rx_not_c=rx_not_c, udp_tx=meta.get('udp_tx_bytes'), ow50=nr(ow,.5), conf50=nr(conf,.5))
out={}
for c in CELLS:
    cd=os.path.join(ROOT,c)
    m=re.search(r'_p(\d+)$', c)
    env=open(os.path.join(cd,'cell.env')).read()
    D = 60.0
    for arm in ARMS:
        ad=os.path.join(cd,arm)
        if not os.path.isdir(ad): continue
        rs=[]
        for rep in sorted(os.listdir(ad), key=lambda s:int(s[3:])):
            # find deadline from cmd
            cmd=open(os.path.join(ad,rep,'cmd.txt')).read()
            mm=re.search(r'--max-deadline-s (\d+)',cmd); D=float(mm.group(1))
            r=run(os.path.join(ad,rep), D)
            if r: r['rep']=int(rep[3:]); rs.append(r)
        out[(c,arm)]=rs
        cens = [min(r['conf_nr95'],D) for r in rs]
        print(f"{c} {arm} N={len(rs)} D={D} | conf p95 nr(cens) med={med(cens):.3f} | conf p95 lin(deliv only) med={med([r['conf_lin95'] for r in rs]):.3f} | ow p95 nr(cens) med={med([min(r['ow_nr95'],D) for r in rs]):.3f} ow lin med={med([r['ow_lin95'] for r in rs])} | confD med={med([r['confD'] for r in rs]):.3f} owD med={med([r['owD'] for r in rs]):.3f} | conf p50 {med([min(r['conf50'],D) for r in rs]):.3f} ow p50 {med([min(r['ow50'],D) for r in rs]):.3f} | c_not_rx sum={sum(r['c_not_rx'] for r in rs)} rx_not_c sum={sum(r['rx_not_c'] for r in rs)} t0zero={sum(1 for r in rs if not r['t0'])} rc!=0={sum(1 for r in rs if r['rc'] not in ('0',None))}")
import pickle; pickle.dump(out, open(sys.argv[3],'wb'))
