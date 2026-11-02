from math import comb
def tail(n,q,r): return sum(comb(n,i)*q**i*(1-q)**(n-i) for i in range(r,n+1))
def rres(K,R,q): return q*tail(K+R-1,q,R)
for K,R,q in [(8,2,.1),(8,2,.2),(8,4,.2),(8,4,.3),(8,6,.3),(8,5,.3),(8,3,.2),(4,4,.3),(4,2,.2)]:
    print(K,R,q,round(rres(K,R,q),4))
for q in (.1,.2,.3):
    for K in (8,4):
        Rm=next(R for R in range(1,20) if rres(K,R,q)<0.05); print('Rmin',K,q,Rm,round(rres(K,Rm,q),4))
# W & D_rt
L234,TAG,HD,HF=42,16,11,12
for C,I,p,d,R in [(12500,.036,65,.3,2),(12500,.036,65,1.0,2),(12500,.285,1000,1.0,2),(12500,.285,1000,.3,2),(1000,.45,65,.3,2),(1000,3.6,1000,1.0,4),(1000,.45,65,1.0,2),(1000,3.6,1000,.3,2)]:
    srep=(L234+TAG+HF+1024)/C; sd=(L234+TAG+HD+p)/C; sa=(L234+TAG+15)/C
    rtt=2*d+sd+sa
    W=7*I+R*srep
    Drt=max(9/8*rtt, rtt+I)
    print(C,I,p,d,'W',round(W,3),'Drt',round(Drt,3),'rtt',round(rtt,3))
print('bytes ratio', 2*1094/(8*134), 4*1094/(8*134), 6*1094/(8*134), 6*1094/(8*1069), 2*1094/(8*1069), 1094/134)
