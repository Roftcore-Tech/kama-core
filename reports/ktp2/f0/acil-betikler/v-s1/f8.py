import sys, re, json, glob, collections
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
cnt=collections.Counter(); rows=[]
for cell, arm, rep, p in runs('c'):
    if not (arm.startswith('uyar') or arm.startswith('pol')): continue
    d=parse_cerr(p)
    meta=json.loads(open(p[:-5]+'out.jsonl').readline())['meta']
    txfec=meta['tx']['fec']; pe=meta['activation']['parity_emitted']
    cnt[arm]+=1
    cnt['txfec>0']+= txfec>0
    if d['fec_blocks']>0 or d['fec_repairs_tx']>0 or txfec>0:
        rows.append((cell,arm,rep,d['fec_blocks'],d['fec_repairs_tx'],txfec,pe,d['latch'],d['txfec'][4]))
print(cnt)
for r in rows: print(r)
