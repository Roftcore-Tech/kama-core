"""[betimsel, sonradan] düşük-M1 sınıflandırması ve konum/zaman/eşzamanlılık kırılımı.
Sınıflar (c.err KP-CLIENT/KP-ACTIVATION alanlarından):
  K0 kuruluş-çöküşü : state=PreConverge & left_preconv=false (koşu sonuna dek PreConverge'den çıkmadı)
  K1 geç-kuruluş    : PreConverge'den çıktı ama çıkış ≥ 10 s (left_preconv_ms) → teslimler D dışında
  K2 kurulmuş-düşük : PreConverge'den < 10 s'de çıktı, M1 < 0.80
  K3 kurulmuş-normal: M1 ≥ 0.80
"""
import csv,sys,re,statistics,collections,math,pathlib
def lp(run,prof,arm,rep):
    t=(pathlib.Path('/home/ryuzaki/kama-k4g-work/k4g05b/netem7')/run/prof/arm/f'rep{rep}'/'c.err').read_text()
    m=re.search(r'left_preconv=(\d+)ms',t); lpb=re.search(r'left_preconv=(true|false)',t)
    return (int(m.group(1)) if m else None, lpb.group(1) if lpb else None)
def fisher(a,b,c,d):
    n=a+b+c+d; r1=a+b; c1=a+c
    def pr(x): return math.comb(r1,x)*math.comb(n-r1,c1-x)/math.comb(n,c1)
    p0=pr(a); return sum(pr(x) for x in range(max(0,c1-(n-r1)),min(r1,c1)+1) if pr(x)<=p0*(1+1e-9))
out={}
for path in sys.argv[2:]:
    rows=list(csv.DictReader(open(path)))
    run=rows[0]['run']
    for r in rows:
        if r['profile']!=sys.argv[1]: continue
        lms,lpb=lp(run,r['profile'],r['arm'],r['rep'])
        m1=float(r['m1'])
        if lpb=='false': k='K0'
        elif lms is not None and lms>=10000: k='K1'
        elif m1<0.80: k='K2'
        else: k='K3'
        r['cls']=k; r['lp_ms']=lms
        out.setdefault(run,[]).append(r)
for run,rows in out.items():
    print(f"===== {run}  profil={sys.argv[1]}")
    for arm in ('taban','aday'):
        rs=sorted([r for r in rows if r['arm']==arm], key=lambda r:int(r['rep']))
        c=collections.Counter(r['cls'] for r in rs)
        print(f" {arm}: sınıf sayıları {dict(sorted(c.items()))}")
        for k in ('K0','K1','K2'):
            sel=[r for r in rs if r['cls']==k]
            if sel: print(f"   {k}: " + ", ".join(f"rep{r['rep']}(ord{r['order']},m1={r['m1']},lp={r['lp_ms']}ms,est={r['establish_tx']},srv_tx={r['s_tx_dgr']},cli_rx={r['udp_rx']}B)" for r in sel))
        nk0=[float(r['m1']) for r in rs if r['cls']!='K0']
        conv=[float(r['m1']) for r in rs if r['cls'] in ('K2','K3')]
        print(f"   M1 medyanı: tümü={statistics.median([float(r['m1']) for r in rs]):.3f}  K0-dışı={statistics.median(nk0) if nk0 else float('nan'):.3f} (n={len(nk0)})  kurulmuş(K2+K3)={statistics.median(conv) if conv else float('nan'):.3f} (n={len(conv)})")
        srtt=[int(r['srtt_us']) for r in rs if r['cls'] in ('K2','K3') and r['srtt_us']]
        if srtt: print(f"   kurulmuş koşularda srtt_us min/med/max = {min(srtt)}/{int(statistics.median(srtt))}/{max(srtt)}")
        # konum kırılımı
        for key,f in (('kol-sırası', lambda r: 'ord'+r['order']), ('faz(rep1-10 / 11-20)', lambda r: 'r1-10' if int(r['rep'])<=10 else 'r11-20')):
            cc=collections.Counter((f(r), r['cls']=='K0') for r in rs)
            print(f"   K0 {key}: " + ", ".join(f"{g}: {cc[(g,True)]}/{cc[(g,True)]+cc[(g,False)]}" for g in sorted(set(f(r) for r in rs))))
    t=[r for r in rows if r['arm']=='taban']; a=[r for r in rows if r['arm']=='aday']
    tk=sum(r['cls']=='K0' for r in t); ak=sum(r['cls']=='K0' for r in a)
    print(f" K0 oranı taban {tk}/20 vs aday {ak}/20 — Fisher iki yanlı p={fisher(tk,20-tk,ak,20-ak):.3f}")
    allk=collections.Counter((r['order'], r['cls']=='K0') for r in rows)
    print(f" K0 kol-sırası (iki kol birlikte): ord1 {allk[('1',True)]}/40, ord2 {allk[('2',True)]}/40")
