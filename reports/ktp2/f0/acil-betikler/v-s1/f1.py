import sys, re, statistics
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
P1 = re.compile(r'^b_r(12k|1k)_d(300|1000)_l(0|10|20|30)_p(65|1000)$')
n=0; nrep=0; tot=0; cells=set(); missing=0
per=[]
for cell, arm, rep, p in runs('b'):
    if not P1.match(cell) or arm not in ('taban','aday'): continue
    cells.add(cell)
    d = parse_cerr(p)
    n+=1
    if 'fec_repairs_tx' not in d: missing+=1; continue
    r = d['fec_repairs_tx']
    if r>0: nrep+=1; tot+=r
    per.append((cell,arm,rep,r,d.get('est',(None,))[0],d.get('fec2')))
print('cells',len(cells),'runs',n,'missing',missing,'runs_with_repair',nrep,'total_repairs',tot)
# l30 p1000 r12k d1000
import collections
by=collections.defaultdict(list)
for c,a,rep,r,bw,f2 in per: by[c].append((r,bw))
for c in sorted(by):
    bws=[b for r,b in by[c] if b is not None]
    print(c, len(by[c]), 'rep>0:',sum(1 for r,b in by[c] if r>0), 'bw p50', statistics.median(bws), 'cap', int(statistics.median(bws))*1000//125000, 'max', max(bws))
