"""KM-DR-STRUCT-GATE math corrigendum (rev.2).

ARITMETIK BEYANI — bolum bolum farkli, tek tip DEGIL:
  * C-2 (zarf ispati)     : TAM KESIR (fractions.Fraction). Sayac uzayinin tam sayimi.
  * C-1 / C-1b (ayrisma)  : FLOAT + bisection (tol 1 ppm). Normal yaklasim kullanilir.
    Onceki surum 0.001'lik d izgarasi kullaniyordu; rev.2'de bisection'a cevrildi.

C-1b ISIMLENDIRME (rev.2 duzeltmesi): eski kod uretecin GERCEK d'sini basiyordu ama
mu_B* diye etiketliyordu. Ikisi ayri buyukluk; ikisi de basiliyor:
  d_B*  = uretecin gercek p11-p01 farki (generator parametresi)
  mu_B* = Laplace-smoothed BEKLENEN GOZLENEN ortalama (mu_sm), d_B*'tan kucuk
"""
from fractions import Fraction as F
from statistics import NormalDist
import math
W=256; NT=W-1; AL=1.0
ND=NormalDist(); Z90=ND.inv_cdf(0.90)     # 1.2815515655

def smooth(pt,n,a=AL): return pt + (1-2*pt)*a/(n+2*a)
def mu_sm(p,d,N,a=AL):
    L=N*p; S=N*(1-p); p01=p*(1-d); p11=p01+d
    return smooth(p11,L,a)-smooth(p01,S,a)
def sd_of(p,d,N,a=AL):
    L=N*p; S=N*(1-p); p01=p*(1-d); p11=p01+d
    p01=min(max(p01,1e-12),1-1e-12); p11=min(max(p11,1e-12),1-1e-12)
    return math.sqrt(p11*(1-p11)*L/(L+2*a)**2 + p01*(1-p01)*S/(S+2*a)**2)

def auc(p,d,N):
    mI,sI=mu_sm(p,0.0,N),sd_of(p,0.0,N); mB,sB=mu_sm(p,d,N),sd_of(p,d,N)
    return ND.cdf((mB-mI)/math.sqrt(sI*sI+sB*sB))

def bisect(f,lo,hi,tol=1e-6):
    if f(lo)>0: return lo
    if f(hi)<0: return None
    for _ in range(200):
        m=(lo+hi)/2
        if f(m)>0: hi=m
        else: lo=m
        if hi-lo<tol: break
    return (lo+hi)/2

print("="*92)
print("C-1b (rev.2)  IKI AYRI OLCUT — 2s non-overlap ile AUC=0.90 AYNI SEY DEGIL")
print(f"              2s non-overlap:  mu_B - mu_I > 2(s_I + s_B)")
print(f"              AUC=0.90      :  (mu_B - mu_I)/sqrt(s_I^2+s_B^2) = {Z90:.4f}")
print("              sigma_I~sigma_B iken: 4.00s vs 1.81s  =>  2s-kriteri ~2.2x DAHA SERT")
print("="*92)
for p in (0.10,0.20,0.30):
    print(f"\n  p={p:.2f}")
    print(f"  {'N':>5} | {'2s: d_B*':>10} {'2s: mu_B*':>10} {'AUC@2s':>8} | {'0.90: d_B*':>11} {'0.90: mu_B*':>12}")
    for N in (100,150,200,255,400,512):
        f2=lambda d: (mu_sm(p,d,N)-2*sd_of(p,d,N)) - (mu_sm(p,0.0,N)+2*sd_of(p,0.0,N))
        fa=lambda d: auc(p,d,N)-0.90
        d2=bisect(f2,1e-6,0.999); da=bisect(fa,1e-6,0.999)
        print(f"  {N:>5} | {d2*1e6:>10,.0f} {mu_sm(p,d2,N)*1e6:>10,.0f} {auc(p,d2,N):>8.4f} | "
              f"{da*1e6:>11,.0f} {mu_sm(p,da,N)*1e6:>12,.0f}")

print()
print("="*92)
print("DOGRULAMA: 2s sinirinda AUC ZATEN >>0.90  =>  '2s asilmazsa AUC 0.90 imkansiz' YANLIS")
print("="*92)
p=0.20;N=255
for d in (0.15,0.20,0.25,0.272,0.30,0.35):
    lo=mu_sm(p,0.0,N)+2*sd_of(p,0.0,N); hi=mu_sm(p,d,N)-2*sd_of(p,d,N)
    print(f"  d_true={d*1e6:>7,.0f}  mu_B={mu_sm(p,d,N)*1e6:>8,.0f}  2s-bantlari ayrisik mi: "
          f"{'EVET' if hi>lo else 'HAYIR':<5}  AUC={auc(p,d,N):.4f}  "
          f"{'<-- 2s BASARISIZ ama AUC>0.90' if (hi<=lo and auc(p,d,N)>0.90) else ''}")
