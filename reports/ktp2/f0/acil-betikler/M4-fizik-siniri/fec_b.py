import json, math
M={}
for l in open("model_b.jsonl"):
    r=json.loads(l); M[(r["cell"],r["pol"])]=r
GEOM = {("r1k",65):0.450,("r1k",1000):3.6,("r12k",65):0.036,("r12k",1000):0.285}
RATES={"r1k":1000,"r12k":12500}; DEL={"d300":0.3,"d1000":1.0}
def binom_ge(n,p,k): return sum(math.comb(n,i)*p**i*(1-p)**(n-i) for i in range(k,n+1))
print("| hücre | D_rt≈max(9/8·RTT, RTT+I) s | F8: (K−1)I + R·s_rep/C s | F8 tur kaldırır? (en kötü konum) | F8 konum payı W_j<D_rt | r_res F8 | p95 rfc | p95 F8 | Δ F8 | ρ F8 | en iyi F (p95, ρ) |")
print("|"+"---|"*11)
cells=sorted({c for c,_ in M}, key=lambda c:(c.split('_')[0]!='r1k', int(c.split('_')[1][1:]), int(c.split('_')[2][1:]), int(c.split('_')[3][1:])))
for c in cells:
    rid,did,l,p=c.split("_"); q=int(l[1:])/100; p=int(p[1:]); I=GEOM[(rid,p)]; C=RATES[rid]; d=DEL[did]
    s_data=42+16+11+p; s_ack=73; s_rep=42+16+12+1024
    rtt=2*d+s_data/C+s_ack/C
    Drt=max(9/8*rtt, rtt+I)
    R=M[(c,"fec8_E1024")]["R"]
    W=7*I+R*s_rep/C
    frac=sum(1 for j in range(8) if (7-j)*I + R*s_rep/C < Drt)/8
    rres=q*binom_ge(8+R-1,q,R) if q>0 else 0
    rf=M[(c,"rfc")]["p95_med"]; f8=M[(c,"fec8_E1024")]["p95_med"]; rho=M[(c,"fec8_E1024")]["rho_fwd"]
    best=min((M[(c,k)]["p95_med"],k,M[(c,k)]["rho_fwd"]) for k in ("fec2_E1024","fec4_E1024","fec8_E1024"))
    print(f"| {c} | {Drt:.2f} | {W:.2f} | {'evet' if W<Drt else 'hayır'} | {frac:.3f} | {rres:.3f} | {rf:.2f} | {f8:.2f} | {f8-rf:+.2f} | {rho:.2f} | {best[1].replace('_E1024','')} {best[0]:.2f}, {best[2]:.2f} |")
