import sys, os, re, json, glob, statistics as st
root = sys.argv[1]
cells = sys.argv[2:]
def pct(v, q):
    v = sorted(v)
    if not v: return float('nan')
    k = (len(v)-1)*q
    f = int(k); c = min(f+1, len(v)-1)
    return v[f] + (v[c]-v[f])*(k-f)
for cell in cells:
    for arm in ['taban','aday','quinn','tcp']:
        reps = sorted(glob.glob(os.path.join(root, cell, arm, 'rep*')))
        rows = []
        for r in reps:
            try:
                ce = open(os.path.join(r,'c.err')).read()
            except Exception:
                ce = ''
            d = {}
            m = re.search(r'btlbw=(\d+)B/s srtt=(\d+)us rtprop=(\d+)us qdelay=(\d+)us pace=(\d+)B/s', ce)
            if m: d['btlbw'],d['srtt'],d['rtprop'],d['qd'],d['pace'] = map(int, m.groups())
            m = re.search(r'transitions=(\d+) recover_arq=(\d+) recover_fwd=(\d+) probe_up=(\d+)', ce)
            if m: d['trans'],d['rarq'],d['rfwd'],d['pup'] = map(int, m.groups())
            m = re.search(r'tx=(\d+)B\(data=(\d+) retx=(\d+) dup=(\d+) fec=(\d+)', ce)
            if m: d['tx'],d['data'],d['retx'],d['dup'],d['fec'] = map(int, m.groups())
            lat = []; last = 0; n=0
            try:
                for line in open(os.path.join(r,'out.jsonl')):
                    j = json.loads(line)
                    if 'event' in j:
                        e = j['event']; n+=1
                        if e.get('delivered_ns') is not None:
                            lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
                            last = max(last, e['delivered_ns']/1e9)
            except Exception:
                pass
            d['n']=n; d['ndel']=len(lat)
            d['p50']=pct(lat,0.5); d['p95']=pct(lat,0.95); d['last']=last
            rows.append(d)
        if not rows: continue
        def med(k):
            v=[x[k] for x in rows if k in x and x[k]==x[k]]
            return st.median(v) if v else float('nan')
        print(f"{cell:28s} {arm:6s} N={len(rows):2d} p50={med('p50'):6.2f} p95={med('p95'):6.2f} last={med('last'):6.1f} btlbw={med('btlbw'):8} pace={med('pace'):8} pup={med('pup')} rarq={med('rarq')} rfwd={med('rfwd')} tx={med('tx')} data={med('data')} retx={med('retx')} fec={med('fec')} srtt={med('srtt')}")
