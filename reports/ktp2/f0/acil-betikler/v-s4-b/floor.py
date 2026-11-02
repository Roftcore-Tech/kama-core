import re, sys, pathlib, statistics as st
base = pathlib.Path("/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out")
for cell in sys.argv[1:]:
    for arm in ("taban", "aday"):
        paces, srtts, lpc = [], [], []
        for rep in sorted((base/cell/arm).glob("rep*")):
            t = (rep/"c.err").read_text(errors="replace")
            m = re.search(r"srtt=(\d+)us rtprop=\d+us qdelay=\d+us pace=(\d+)B/s", t)
            if not m: continue
            srtts.append(int(m.group(1))/1e6); paces.append(int(m.group(2)))
        p = st.median(paces); s = st.median(srtts)
        U = 1013 if "p1000" in cell else 80
        floor = 1.5*(2*U/p + 65/31)
        print(f"{cell:24s} {arm:5s} n={len(paces)} pace_med={p:.0f} srtt_med={s:.3f} floor(U={U})={floor:.2f} 9/8srtt={9/8*s:.2f}")
