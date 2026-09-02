"""KM-DR-STRUCT-GATE errata math. Exact rationals; every expected value verified by construction."""
from fractions import Fraction as F
import math
W=256; NT=W-1; A=F(1)

def counts(seq):
    n={'00':0,'01':0,'10':0,'11':0}
    for a,b in zip(seq,seq[1:]): n[f"{a}{b}"]+=1
    return n['00'],n['01'],n['10'],n['11']
def dval(c,a=A):
    n00,n01,n10,n11=c; Ld,Sd=n10+n11,n00+n01
    return F(n11+a,Ld+2*a)-F(n01+a,Sd+2*a)

def disperse(L):                      # max dispersion, interior (S...S at both ends)
    if 2*L+1>W: return None
    s=[0]*W; gaps=W-L
    base=gaps//(L+1); extra=gaps-base*(L+1); pos=base+(1 if extra>0 else 0); extra=max(extra-1,0)
    for i in range(L):
        s[pos]=1; step=base+(1 if extra>0 else 0); extra=max(extra-1,0); pos+=1+step
    return s if sum(s)==L and s[0]==0 and s[-1]==0 and not any(s[i] and s[i+1] for i in range(W-1)) else None
def altern(L):                        # L=128 ozel: alternating
    s=[0]*W
    for i in range(1,W,2): s[i]=1
    return s
def run(L,mode):
    s=[0]*W; st={'interior':(W-L)//2,'leading':0,'trailing':W-L}[mode]
    for i in range(st,st+L): s[i]=1
    return s

print("=== ZARF — insaayla dogrulandi, tam kesir (alpha=1, W=256) ===")
print(f"{'p':>5} {'L':>4} | {'d_min ppm':>10} {'sayaclar':>19} | {'d_max ppm':>10} {'sayaclar':>19}")
for p in (0.05,0.10,0.15,0.20,0.25,0.30,0.40,0.50):
    L=round(NT*p)
    s=disperse(L) or altern(L); clo=counts(s); lo=dval(clo)
    best=None
    for m in ('interior','leading','trailing'):
        c=counts(run(L,m)); v=dval(c)
        if best is None or v>best[0]: best=(v,c)
    print(f"{p:>5.2f} {L:>4} | {float(lo)*1e6:>10.0f} {str(clo):>19} | {float(best[0])*1e6:>10.0f} {str(best[1]):>19}")

print()
print("=== FIXTURE'LAR — beklenen deger TAM KESIR ===")
fx=[("W-SCATTERED-51 (izole, tam ic)", disperse(51)),
    ("W-RUN-TERMINAL-51 (tek run, sonda)", run(51,'trailing')),
    ("W-RUN-INTERIOR-51 (tek run, ic)", run(51,'interior')),
    ("W-RUN-LEADING-51 (tek run, basta)", run(51,'leading'))]
c=[0]*W; pos=3
for r in range(17):
    for j in range(3): c[pos+j]=1
    pos+=12
fx.insert(1,("W-CLUSTER-17x3 (ic)", c))
for name,seq in fx:
    cc=counts(seq); v=dval(cc)
    print(f"{name:<36} {str(cc):>20} N={sum(cc):>3} kayip={sum(seq):>3} bas={seq[0]} son={seq[-1]}")
    print(f"{'':<36} d = {v}  ->  {float(v)*1e6:>+10.2f} ppm")

print()
print("=== (N_DEC, LOSS_MIN) DIKDORTGENININ GUVENSIZ KOSELERI ===")
def bias(p,N,a=1.0):
    L=N*p; S=N*(1-p); return (1-2*p)*a*(1/(L+2*a)-1/(S+2*a))
def var_(p,N,pp=None,a=1.0):
    L=N*p; S=N*(1-p); q=pp if pp else p
    return q*(1-q)*(L/(L+2*a)**2)+ (q*(1-q))*(S/(S+2*a)**2)
def sd_iid(p,N,a=1.0):
    L=N*p;S=N*(1-p); return math.sqrt(p*(1-p)*(L/(L+2*a)**2+S/(S+2*a)**2))
def sd_b(d,p,N,a=1.0):
    L=N*p;S=N*(1-p);p01=p-d*p;p11=p01+d
    p01=min(max(p01,1e-9),1-1e-9);p11=min(max(p11,1e-9),1-1e-9)
    return math.sqrt(p11*(1-p11)*L/(L+2*a)**2 + p01*(1-p01)*S/(S+2*a)**2)
def nreq(p,T,z=2.0):
    for N in range(20,4000):
        if bias(p,N)+z*sd_iid(p,N) < T - z*sd_b(T,p,N): return N
    return None
for T in (0.30,0.35):
    for Ndec in (200,225,240):
        bad=[int(p*1e6) for p in [i/1000 for i in range(100,320,10)]
             if (lambda n: n is None or n>Ndec)(nreq(p,T))]
        bad=[b for b in bad if b>=160000]
        tag=f"T={int(T*1e6):,} N_DEC={Ndec}"
        print(f"{tag:<26} LOSS_MIN=160.000 varsayimiyla GUVENSIZ loss noktalari: "
              +(", ".join(f"{b:,}" for b in bad) if bad else "yok"))
