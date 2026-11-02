import sys, re, collections, glob
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
man=re.compile(r'KP-MANIFEST role=(\S+) .*?fec=(ON adaptive|ON static[^ ]*|ON [^ ]+|OFF)')
arb=collections.defaultdict(lambda:[0]*7); latch=collections.defaultdict(lambda:[0]*5); n=collections.Counter(); modes=collections.Counter()
nolatch=collections.Counter()
for lane in 'bcd':
    for cell, arm, rep, p in runs(lane):
        t=open(p,errors='replace').read()
        ms=man.findall(t)
        if not ms: continue
        modes[(lane,ms[0][1])]+=1
        if not ms[0][1].startswith('ON adaptive'): continue
        d=parse_cerr(p)
        if 'arb' not in d: nolatch[lane]+=1; continue
        n[lane]+=1
        for i,v in enumerate(d['arb']): arb[lane][i]+=v
        for i,v in enumerate(d['latch']): latch[lane][i]+=v
print(modes)
print('adaptive runs', n, sum(n.values()), 'missing', nolatch)
for l in 'bcd': print(l, 'arb', arb[l], 'latch', latch[l])
tot=[sum(latch[l][i] for l in 'bcd') for i in range(5)]; print('latch total', tot)
