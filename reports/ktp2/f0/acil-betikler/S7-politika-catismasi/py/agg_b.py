import json, csv, os, sys, glob, statistics as st
root = sys.argv[1]
cells = sys.argv[2:]
def pct(xs, q):
    xs = sorted(xs)
    if not xs: return None
    k = (len(xs)-1)*q
    f = int(k); c = min(f+1, len(xs)-1)
    return xs[f] + (xs[c]-xs[f])*(k-f)
for cell in cells:
    env = {}
    for line in open(os.path.join(root, cell, 'cell.env')):
        line=line.strip()
        if '=' in line and not line.startswith('#'):
            k,v=line.split('=',1); env[k]=v.strip('"')
    for arm in ['taban','aday']:
        rows=[]
        for rep in sorted(glob.glob(os.path.join(root, cell, arm, 'rep*'))):
            try:
                lines=open(os.path.join(rep,'out.jsonl')).read().splitlines()
            except Exception: continue
            if not lines: continue
            meta=json.loads(lines[0])['meta']
            act=meta.get('activation',{})
            lat=[]
            n=0
            for l in lines[1:]:
                e=json.loads(l).get('event')
                if not e: continue
                n+=1
                if e.get('delivered_ns'):
                    lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
            # ts.csv proposals
            mx_pre=0; mx_fin=0; n_pre=0; n_rows=0; fin_pos=0
            tsf=os.path.join(rep,'ts.csv')
            if os.path.exists(tsf):
                with open(tsf) as f:
                    r=csv.DictReader(f)
                    for row in r:
                        n_rows+=1
                        pre=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop']))
                        fin=int(row['final_ratio'])
                        if pre>0: n_pre+=1
                        if fin>0: fin_pos+=1
                        mx_pre=max(mx_pre,pre); mx_fin=max(mx_fin,fin)
            rows.append(dict(btlbw=act.get('btlbw_bps'), srtt=act.get('srtt_us'), par=act.get('parity_emitted'),
                             fec=meta['tx'].get('fec'), retx=meta['tx'].get('retx'), data=meta['tx'].get('data'),
                             p95=pct(lat,0.95), p50=pct(lat,0.5), deliv=len(lat)/max(n,1), mx_pre=mx_pre, mx_fin=mx_fin, n_pre=n_pre, n_rows=n_rows, fin_pos=fin_pos))
        if not rows: continue
        print(cell, arm, 'N=',len(rows), 'PAY',env.get('PAY'),'IVAL',env.get('IVAL_MS'),
              'btlbw_med', st.median([r['btlbw'] for r in rows if r['btlbw'] is not None]),
              'btlbw_min/max', min(r['btlbw'] for r in rows), max(r['btlbw'] for r in rows),
              'parity_sum', sum(r['par'] or 0 for r in rows), 'fec_bytes_sum', sum(r['fec'] or 0 for r in rows),
              'p95_med', round(st.median([r['p95'] for r in rows if r['p95'] is not None]),2),
              'deliv_med', round(st.median([r['deliv'] for r in rows]),3),
              'max_pre_prop', max(r['mx_pre'] for r in rows), 'max_final', max(r['mx_fin'] for r in rows),
              'rows_pre>0_frac', round(sum(r['n_pre'] for r in rows)/max(1,sum(r['n_rows'] for r in rows)),3),
              'retx/data', round(sum(r['retx'] for r in rows)/max(1,sum(r['data'] for r in rows)),3))
