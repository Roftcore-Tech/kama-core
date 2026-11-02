import json, sys, os, glob, statistics as st
cell=sys.argv[1]
for arm in ['taban','aday','quinn']:
    buckets=[[] for _ in range(10)]
    for rep in sorted(glob.glob(os.path.join(cell,arm,'rep*'))):
        f=os.path.join(rep,'out.jsonl')
        if not os.path.exists(f): continue
        for line in open(f):
            o=json.loads(line)
            if 'event' in o:
                e=o['event']
                if e.get('delivered_ns') is None: continue
                b=min(9,e['msg_id']//10)
                buckets[b].append((e['delivered_ns']-e['sent_ns'])/1e9)
    print(arm,' '.join('%.2f'%st.median(b) if b else 'na' for b in buckets))
