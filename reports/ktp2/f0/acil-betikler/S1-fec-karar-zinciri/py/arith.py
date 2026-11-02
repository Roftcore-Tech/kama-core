# Birebir sender.rs:3763-3796 + budget.rs:38-41 + sender.rs:1026-1031 aritmetiği
REF=125000; PCAP=900
def chain(btlbw, final, k, rmax):
    cap = PCAP if btlbw == 0 else min(btlbw*1000//REF, PCAP)
    clamped = min(final, cap, PCAP)
    if clamped == 0 and final > 0:
        out = final; tag='would_zero(bypass)'
    else:
        out = clamped; tag = 'clamped' if clamped < final else 'pass'
    r_unc = (out*k + 500)//1000
    return cap, out, tag, r_unc, min(r_unc, rmax)
print("btlbw | cap‰ | final‰(arbiter=250) | yol | r(K=8,rmax4) | r(K=4) | r(K=16,rmax8) | r(k_act=3) | r(k_act=1)")
for b in [0, 31, 100, 124, 125, 500, 1000, 3000, 3628, 7874, 7875, 12500, 15625, 23500, 31250]:
    cap,out,tag,_,r8 = chain(b,250,8,4)
    r4 = chain(b,250,4,4)[4]; r16 = chain(b,250,16,8)[4]; r3 = chain(b,250,3,4)[4]; r1 = chain(b,250,1,4)[4]
    print(f"{b:>6} | {cap:>4} | {out:>4} | {tag:<20} | {r8} | {r4} | {r16} | {r3} | {r1}")
# min btlbw for r>=1 per K, in clamp regime
for k in [1,2,3,4,5,6,7,8,16]:
    cmin = next(c for c in range(1,1001) if (c*k+500)//1000 >= 1)
    print(f"K={k}: r>=1 icin cap>={cmin}‰ -> btlbw>={cmin*REF//1000} B/s; r>=2 icin cap>={next((c for c in range(1,1001) if (c*k+500)//1000>=2), None)}‰ ; r(250‰)={(250*k+500)//1000}")
# spec floor 63 vs clamp
print('spec_min 63 sonra clamp, btlbw=3000:', chain(3000,63,8,4))
# emergency minimal parity bypass (clamp yok): 100/200
print('emergency 100‰ K8 ->', (100*8+500)//1000, '; 200‰ ->', (200*8+500)//1000)
