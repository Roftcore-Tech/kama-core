import sys, re, pathlib, statistics
root = pathlib.Path(sys.argv[1])
for c in sys.argv[2:]:
    for a in ("taban", "aday"):
        tp = rx = dp = n = 0; paces = []; srtts = []; rttv=[]
        for rep in (root / c / a).glob("rep*"):
            p = rep / "c.err"
            if not p.exists():
                continue
            t = p.read_text(errors="replace")
            m = re.search(r"tail_probe=(\d+)@.*?retx=(\d+) dup=(\d+)", t)
            if m:
                tp += int(m[1]); rx += int(m[2]); dp += int(m[3]); n += 1
            m = re.search(r" pace=(\d+)B/s", t)
            if m: paces.append(int(m[1]))
            m = re.search(r" srtt=(\d+)us", t)
            if m: srtts.append(int(m[1]) / 1e6)
        med = lambda v: statistics.median(v) if v else float("nan")
        # EAI floor with unit ~1069B: 1.5*(2*unit/pace + 65/31)
        pm = med(paces)
        eai = 2 * 1069 / pm + 65 / 31 if pm == pm and pm > 0 else float("nan")
        print(f"{c} {a}: reps={n} tail_probe={tp} retx={rx} dup={dp} pace_med={pm} srtt_med={med(srtts):.3f}s "
              f"-> 9/8*srtt={9/8*med(srtts):.2f}s  1.5*EAI(unit1069)={1.5*eai:.2f}s")
