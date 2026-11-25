#!/usr/bin/env python3
"""Kirli ölçütü K2 ve K8'in ölçüm penceresi [t_start, t_end] içindeki değerlendirmesi (ürün kodu değil).
kullanım: kirli_pencere.py <koşu dizini (…/aday/rep1)> <kosu.log'un dizini> <çıktı>
K2p: k2-ornek.txt'te (0,5 s'de bir cargo/rustc süreç sayısı) anı [t_start, t_end] içinde olan bütün örnekler 0 VE pencere
     içinde en az 150 örnek var (pencere ≈ 100 s). k2-ornek.txt yoksa K2p DEĞERLENDİRİLEMEDİ.
K8p: MID_ma0 ve MID_ifb0 yakalamalarında UDP dışı IPv4 paketlerinden anı [t_start, t_end] içinde olan 0 VE koşunun UDP akışı 4
     VE ifb0 tbf 'sent packets' (son örnek) = ifb0'da yakalanan paket. Pencere dışındaki UDP dışı paketler ayrıca listelenir."""
import json, sys, os
d, top, outp = sys.argv[1:4]
def jl(p):
    o = []
    for l in open(p):
        l = l.strip()
        if l:
            try: o.append(json.loads(l))
            except ValueError: pass
    return o
def rd(n):
    s = open(f"{d}/{n}").read().strip().split()[0]
    a, b = (s.split(".") + ["0"])[:2]
    return int(a) * 10**9 + int((b + "000000000")[:9])
ts, te = rd("t_start"), rd("t_end")
t0 = int(open(f"{d}/sync/t0").read())
tau = lambda n: (n - t0) / 1e9
O = []; W = O.append
W(f"ölçüm penceresi: t_start τ={tau(ts):.3f} s, t_end τ={tau(te):.3f} s (süre {(te - ts) / 1e9:.3f} s)")
# K2p
p = os.path.join(top, "k2-ornek.txt")
if os.path.exists(p):
    smp = []
    for l in open(p):
        f = l.split()
        if len(f) >= 2:
            a, b = (f[0].split(".") + ["0"])[:2]
            smp.append((int(a) * 10**9 + int((b + "000000000")[:9]), int(f[1]), " ".join(f[2:])))
    inw = [s for s in smp if ts <= s[0] <= te]
    nz = [s for s in inw if s[1] != 0]
    outw = [s for s in smp if not (ts <= s[0] <= te) and s[1] != 0]
    k2 = (len(nz) == 0 and len(inw) >= 150)
    W(f"K2p {'tuttu' if k2 else 'TUTMADI'} · pencere içi örnek {len(inw)} (toplam {len(smp)}); pencere içinde cargo/rustc ≠ 0 olan örnek {len(nz)}" + (f" (ilk: τ={tau(nz[0][0]):.2f} {nz[0][2]})" if nz else "") + f"; pencere DIŞINDA ≠ 0 olan örnek {len(outw)}" + (f" (τ {tau(outw[0][0]):.2f} … {tau(outw[-1][0]):.2f}; {outw[0][2]})" if outw else ""))
else:
    k2 = None; W("K2p DEĞERLENDİRİLEMEDİ · k2-ornek.txt yok")
# K8p
cap = {n: [x for x in jl(f"{d}/cap/{n}.jsonl") if "ip_len" in x] for n in ("MID_ma0", "MID_ifb0")}
non = [(n, x) for n in cap for x in cap[n] if x.get("proto") != 17]
inw = [(n, x) for n, x in non if ts <= x["t_unix_ns"] <= te]
flows = {(x.get("sport"), x.get("dport")) for n in cap for x in cap[n] if x.get("proto") == 17}
kq = jl(f"{d}/kuyruk.jsonl")
last = None
for s in kq:
    if isinstance(s.get("q"), list):
        for q in s["q"]:
            if q.get("kind") == "tbf" and q.get("handle") == "1:": last = q
sent = last.get("packets") if last else None
k8 = (len(inw) == 0 and len(flows) == 4 and sent == len(cap["MID_ifb0"]))
W(f"K8p {'tuttu' if k8 else 'TUTMADI'} · darboğazda (MID_ma0 + MID_ifb0) UDP dışı paket: pencere içinde {len(inw)}, pencere dışında {len(non) - len(inw)}" +
  (" (" + "; ".join(f"{n} τ={tau(x['t_unix_ns']):.3f} proto {x.get('proto')} {x['ip_len']} B {x.get('dir', '')}" for n, x in non) + ")" if non else "") +
  f"; UDP akışı {len(flows)}; tbf sent {sent} = ifb0 yakalanan {len(cap['MID_ifb0'])}")
json.dump(dict(t_start_tau=tau(ts), t_end_tau=tau(te), k2p=k2, k8p=k8), open(outp + ".json", "w"))
open(outp + ".txt", "w").write("\n".join(O) + "\n"); print("\n".join(O))
