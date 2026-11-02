import os, sys, glob, re, statistics as st
root = sys.argv[1]
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    for arm in ('rs2_taban', 'rs4_taban', 'arq_taban'):
        fr = []; retx = []; btl = []
        for rep in glob.glob(os.path.join(cd, arm, 'rep*')):
            try: ce = open(os.path.join(rep, 'c.err')).read()
            except Exception: continue
            d = re.search(r'parity_denied=(\d+)', ce); fe = re.search(r'flow_end=(\d+)', ce)
            b = re.search(r'est=\[btlbw=(\d+)B/s', ce)
            if d and fe: fr.append(int(d.group(1)) / (int(fe.group(1))/1e6))
            if b: btl.append(int(b.group(1)))
        if fr or btl:
            print(f"{cell:24s} {arm:10s} n={len(btl):2d} denied_per_flow_ms_med={st.median(fr) if fr else 0:.2f} btlbw_end_med={st.median(btl) if btl else None}")
