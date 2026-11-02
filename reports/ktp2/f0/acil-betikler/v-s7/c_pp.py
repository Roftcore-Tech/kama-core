import json, os, glob, sys, csv, statistics as st
for cell in sys.argv[1:]:
    for arm in ['rs2_taban']:
        mp=[]; td=[]; mx=[]
        for rep in sorted(glob.glob(os.path.join(cell,arm,'rep*'))):
            ts=os.path.join(rep,'ts.csv')
            if not os.path.exists(ts): continue
            r=list(csv.DictReader(open(ts)))
            v=[int(x['pending_parity']) for x in r]
            if v: mp.append(sum(v)/len(v)); mx.append(max(v))
            meta=json.loads(open(os.path.join(rep,'out.jsonl')).readline())['meta']
            td.append(meta['activation']['parity_token_denied'])
        print(cell,arm,'N',len(mp),'median mean pending_parity %.2f'%st.median(mp),'median max',st.median(mx),'median tokden',st.median(td))
