#!/usr/bin/env python3
"""[betimsel, sonradan] sınıf ayrıştırması: PreConverge'de takılı / kaçan; kaçanlarda M1 ve srtt. Salt okur."""
import json, math, pathlib, statistics, sys
H = pathlib.Path(__file__).parent

def mwu(x, y):
    pooled = sorted([(v, 0) for v in x] + [(v, 1) for v in y], key=lambda t: t[0]); n = len(pooled)
    r2 = [0]*n; i = 0
    while i < n:
        j = i
        while j+1 < n and pooled[j+1][0] == pooled[i][0]: j += 1
        for k in range(i, j+1): r2[k] = (i+1)+(j+1)
        i = j+1
    m = len(x); obs = sum(r for r, (_, g) in zip(r2, pooled) if g == 0)
    dp = [dict() for _ in range(m+1)]; dp[0][0] = 1
    for r in r2:
        for k in range(m-1, -1, -1):
            for s, c in list(dp[k].items()): dp[k+1][s+r] = dp[k+1].get(s+r, 0)+c
    dist = dp[m]; tot = sum(dist.values()); mean2 = m*(n+1); dev = abs(obs-mean2)
    return sum(c for s, c in dist.items() if abs(s-mean2) >= dev)/tot

def fisher2(a, b, c, d):
    # iki yanlı kesin Fisher, tablo [[a,b],[c,d]]
    n = a+b+c+d; r1 = a+b; c1 = a+c
    def p(x): return math.comb(r1, x)*math.comb(n-r1, c1-x)/math.comb(n, c1)
    p0 = p(a); lo = max(0, c1-(n-r1)); hi = min(r1, c1)
    return sum(p(x) for x in range(lo, hi+1) if p(x) <= p0*(1+1e-9))

for prof in sys.argv[1:]:
    rows = json.load(open(H/f"{prof}_rows.json"))
    print(f"##### {prof}")
    for s in ("S1(a8c4e8c6)", "S2(49864510)", "POOL"):
        st = {}
        for a in ("taban", "aday"):
            R = [r for r in rows if (s == "POOL" or r["sess"] == s) and r["arm"] == a]
            stuck = [r for r in R if r["state_end"] == "PreConverge"]
            stuck_ack = [r for r in stuck if (r["first_conf_s"] is not None and r["first_conf_s"] < 20)]
            esc = [r for r in R if r["state_end"] != "PreConverge"]
            esc15 = [r for r in esc if r["left_preconv_s"] is not None and r["left_preconv_s"] < 15]
            st[a] = (R, stuck, esc, esc15)
            m1e = sorted(round(r["m1"], 2) for r in esc15)
            srtt = [r["srtt_us"] for r in esc if r["srtt_us"]]
            print(f" {s:<13} {a:<5} N={len(R)} PreConv-takılı={len(stuck)} (bunların D içinde ACK görmüş={len(stuck_ack)}) kaçan={len(esc)} "
                  f"kaçan<15s={len(esc15)} M1med(kaçan<15s)={statistics.median(m1e) if m1e else float('nan'):.3f} "
                  f"srtt_med(kaçan)={statistics.median(srtt)/1000 if srtt else float('nan'):.1f}ms  M1(kaçan<15s)={m1e}")
        t, a = st["taban"], st["aday"]
        fp = fisher2(len(t[1]), len(t[2]), len(a[1]), len(a[2]))
        x = [r["m1"] for r in a[3]]; y = [r["m1"] for r in t[3]]
        pm = mwu(x, y) if x and y else float('nan')
        print(f"   {s}: takılı oranı Fisher p={fp:.3g} ; kaçan<15s M1 MWU p={pm:.3g}")
