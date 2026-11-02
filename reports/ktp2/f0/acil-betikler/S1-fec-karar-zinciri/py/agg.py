import os, re, sys, json, csv, collections
ROOT = sys.argv[1]
OUT = sys.argv[2]
rows = []
re_kv = lambda k: re.compile(r'\b' + re.escape(k) + r'=([^\s]+)')
def grab(line, k):
    m = re.search(r'(?:^|\s)' + re.escape(k) + r'=(\[[^\]]*\]|\([^)]*\)|[^\s]+)', line)
    return m.group(1) if m else None
for lane in ['b', 'c', 'd']:
    base = os.path.join(ROOT, lane, 'out')
    if not os.path.isdir(base):
        continue
    for cell in sorted(os.listdir(base)):
        cdir = os.path.join(base, cell)
        if not os.path.isdir(cdir):
            continue
        for arm in sorted(os.listdir(cdir)):
            adir = os.path.join(cdir, arm)
            if not os.path.isdir(adir):
                continue
            for rep in sorted(os.listdir(adir)):
                rdir = os.path.join(adir, rep)
                cerr = os.path.join(rdir, 'c.err')
                if not os.path.isfile(cerr):
                    continue
                try:
                    txt = open(cerr, errors='replace').read()
                except Exception:
                    continue
                if 'KP-MANIFEST' not in txt:
                    continue
                man = [l for l in txt.splitlines() if l.startswith('KP-MANIFEST') and 'fec=' in l]
                kc = [l for l in txt.splitlines() if l.startswith('KP-CLIENT')]
                ka = [l for l in txt.splitlines() if l.startswith('KP-ACTIVATED')]
                kv = [l for l in txt.splitlines() if l.startswith('KP-ACTIVATION')]
                if not man or not kc:
                    continue
                m = man[0]
                fecmode = grab(m, 'fec')
                fm = re.search(r'fec=(\S+ \S+ \S+)', m)
                fecdesc = fm.group(1) if fm else fecmode
                prof = grab(m, 'profile')
                r = dict(lane=lane, cell=cell, arm=arm, rep=rep, profile=prof, fec=fecdesc)
                c = kc[0]
                for k in ['fec_blocks', 'fec_repairs_tx', 'gate_opened']:
                    v = grab(c, k)
                    r[k] = int(v) if v and v.isdigit() else None
                arb = grab(c, 'arb')
                r['arb'] = json.loads(arb) if arb else None
                lat = grab(c, 'latch')
                r['latch'] = lat
                if ka:
                    for k in ['clamp', 'would_zero', 'partial_blocks']:
                        v = grab(ka[0], k)
                        if v:
                            v = v.rstrip(']')
                        r[k] = int(v) if v and v.isdigit() else None
                if kv:
                    mm = re.search(r'btlbw=(\d+)B/s', kv[0]); r['btlbw_end'] = int(mm.group(1)) if mm else None
                    mm = re.search(r'pace=(\d+)B/s', kv[0]); r['pace_end'] = int(mm.group(1)) if mm else None
                    mm = re.search(r'parity=(\d+) parity_denied=(\d+)', kv[0])
                    if mm:
                        r['parity_emitted'] = int(mm.group(1)); r['parity_denied'] = int(mm.group(2))
                # ts.csv
                ts = os.path.join(rdir, 'ts.csv')
                if os.path.isfile(ts):
                    n = 0; prop_pos = 0; final_pos = 0; r_pos = 0; finals = collections.Counter(); maxprop = 0
                    with open(ts) as f:
                        rd = csv.DictReader(f)
                        for row in rd:
                            try:
                                cp = int(row['confirmed_prop']); sp = int(row['structure_prop']); spp = int(row['spec_prop']); fr = int(row['final_ratio'])
                            except Exception:
                                continue
                            n += 1
                            p = max(cp, sp, spp)
                            if p > 0: prop_pos += 1
                            if fr > 0: final_pos += 1
                            if (fr * 8 + 500) // 1000 >= 1: r_pos += 1
                            finals[fr] += 1
                            maxprop = max(maxprop, p)
                    r['ts_n'] = n; r['ts_prop_pos'] = prop_pos; r['ts_final_pos'] = final_pos; r['ts_r8_pos'] = r_pos
                    r['ts_final_top'] = finals.most_common(4); r['ts_maxprop'] = maxprop
                    r['ts_final_max'] = max(finals) if finals else None
                rows.append(r)
with open(OUT, 'w') as f:
    for r in rows:
        f.write(json.dumps(r) + '\n')
print(len(rows))
