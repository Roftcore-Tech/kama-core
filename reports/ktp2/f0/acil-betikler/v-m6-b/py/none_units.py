import json, sys, os, glob, bisect, collections
root = sys.argv[1]
c = collections.Counter()
for al in sorted(glob.glob(os.path.join(root, '*dosyasiz*', '*', 'rep*', 'alice.jsonl'))):
    S=[]; sends=[]
    for line in open(al, errors='replace'):
        try: r=json.loads(line)
        except Exception: continue
        if r.get('k')=='tr':
            for rec in r['r']:
                if rec[0]=='S': S.append((rec[4],rec[2],rec[3]))
        elif r.get('k')=='send': sends.append((r['t'],r['tr'],r['c']))
    sends.sort(); st=[s[0] for s in sends]
    for t,u,l in S:
        i=bisect.bisect_right(st,t)-1
        cl = sends[i][2] if i>=0 and sends[i][0]<=t<=sends[i][1]+0.001 else None
        c[(cl,l)]+=1
print(c.most_common())
