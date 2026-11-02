import json, sys, collections
p = sys.argv[1]
lens = collections.Counter()
sends = []
S = []
for line in open(p, errors='replace'):
    try: r = json.loads(line)
    except Exception: continue
    if r.get('k') == 'tr':
        for rec in r['r']:
            if rec[0] == 'S':
                lens[rec[3]] += 1; S.append((rec[4], rec[2], rec[3]))
    elif r.get('k') == 'send':
        sends.append((r['t'], r['c'], r['i'], r.get('sp')))
print('S len hist', sorted(lens.items()))
print('n S', len(S), 'n send', len(sends))
# show first 40 S records with nearest send class
S.sort(); sends.sort()
j = 0
for t, u, l in S[:45]:
    while j + 1 < len(sends) and sends[j+1][0] <= t: j += 1
    print(round(t,1), u, l, sends[j][1:] if sends else None)
