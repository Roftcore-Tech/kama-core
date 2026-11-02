import json,csv,collections
rows=json.load(open('rows.json'))
T="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
for r in rows:
    if r['lane']!='b' or not (r.get('parity') or 0)>0 or 'r25' in r['cell']: continue
    p=f"{T}/b/out/{r['cell']}/{r['arm']}/{r['rep']}/ts.csv"
    spans=[]; st=None; last=None; fr=set()
    for row in csv.DictReader(open(p)):
        t=int(row['t_ms']); pp=int(row['pending_parity'])
        if pp>0:
            fr.add(int(row['final_ratio']))
            if st is None: st=t
        elif st is not None: spans.append(t-st); st=None
        last=t
    if st is not None: spans.append(-(last-st))
    print(r['cell'],r['arm'],r['rep'],'parity',r['parity'],'rec',r.get('s_fec_recovered_units'),'pend_spans_ms',spans[:6],'final_ratio_during',sorted(fr)[:5])
