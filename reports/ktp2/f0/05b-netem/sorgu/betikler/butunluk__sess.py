"""[betimsel, sonradan] aynı taban ikilisinin (14b5f7d3) iki oturumdaki M1 dağılımı + K0-dışı kol kıyası.
MWU fonksiyonu netem7_analyze.py'den birebir kopya (orta-sıra, kesin)."""
import csv,statistics,sys,re,pathlib
def mwu(x, y):
    pooled = sorted([(v, 0) for v in x] + [(v, 1) for v in y], key=lambda t: t[0]); n = len(pooled); ranks2 = [0] * n; i = 0
    while i < n:
        j = i
        while j + 1 < n and pooled[j + 1][0] == pooled[i][0]: j += 1
        r2 = (i + 1) + (j + 1)
        for k in range(i, j + 1): ranks2[k] = r2
        i = j + 1
    m = len(x); obs = sum(r for r, (_, g) in zip(ranks2, pooled) if g == 0)
    dp = [dict() for _ in range(m + 1)]; dp[0][0] = 1
    for r in ranks2:
        for k in range(m - 1, -1, -1):
            for s, c in dp[k].items(): dp[k + 1][s + r] = dp[k + 1].get(s + r, 0) + c
    dist = dp[m]; total = sum(dist.values()); mean2 = m * (n + 1); dev = abs(obs - mean2)
    return sum(c for s, c in dist.items() if abs(s - mean2) >= dev) / total
A=list(csv.DictReader(open(f'{sys.argv[1]}/run-a8c4e8c6.csv'))); B=list(csv.DictReader(open(f'{sys.argv[1]}/run-49864510.csv')))
def k0(r):
    t=(pathlib.Path('/home/ryuzaki/kama-k4g-work/k4g05b/netem7')/r['run']/r['profile']/r['arm']/f"rep{r['rep']}"/'c.err').read_text()
    return 'left_preconv=false' in t
print("taban(oturum a8c4e8c6) vs taban(oturum 49864510), aynı ikili 14b5f7d3:")
for p in ["p1_loss50","p2_loss80","p3_loss95","p4_jitter","p5_revpath","p6_blackout","p7_bbr"]:
    x=[float(r['m1']) for r in A if r['profile']==p and r['arm']=='taban']; y=[float(r['m1']) for r in B if r['profile']==p and r['arm']=='taban']
    print(f"  {p:<12} med {statistics.median(x):.3f} vs {statistics.median(y):.3f}  p={mwu(x,y):.4g}")
print("\np2 K0-dışı (koşu sonuna dek PreConverge'de kalanlar çıkarıldı) aday vs taban:")
for name,R in (("a8c4e8c6",A),("49864510",B)):
    t=[float(r['m1']) for r in R if r['profile']=='p2_loss80' and r['arm']=='taban' and not k0(r)]
    a=[float(r['m1']) for r in R if r['profile']=='p2_loss80' and r['arm']=='aday' and not k0(r)]
    print(f"  {name}: taban n={len(t)} med={statistics.median(t):.3f} | aday n={len(a)} med={statistics.median(a):.3f} | Δ={statistics.median(a)-statistics.median(t):+.3f} p={mwu(a,t):.4g}")
print("\np2 K0 göstergesi (0/1) üstünden: medyan kayması yalnız K0 sayısıyla nasıl oynar —")
for name,R in (("a8c4e8c6",A),("49864510",B)):
    for arm in ('taban','aday'):
        v=sorted(float(r['m1']) for r in R if r['profile']=='p2_loss80' and r['arm']==arm)
        print(f"  {name} {arm:<5} sıralı M1: {' '.join(f'{x:.2f}' for x in v)}")
