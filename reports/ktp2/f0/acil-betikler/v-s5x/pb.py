import os, sys, re, collections
root = sys.argv[1]
pat = re.compile(r"param_bound_peers:\s*(\d+)")
for L in ("b", "c", "d"):
    c = collections.Counter()
    base = os.path.join(root, L, "out")
    for cell in os.listdir(base):
        cd = os.path.join(base, cell)
        if not os.path.isdir(cd): continue
        for arm in os.listdir(cd):
            ad = os.path.join(cd, arm)
            if not os.path.isdir(ad): continue
            for rep in os.listdir(ad):
                f = os.path.join(ad, rep, "s.err")
                if not os.path.isfile(f): continue
                try:
                    m = pat.search(open(f, errors="replace").read())
                except OSError:
                    continue
                c[(arm, m.group(1) if m else "NA")] += 1
    print("==", L)
    for k, v in sorted(c.items()): print("  ", k, v)
