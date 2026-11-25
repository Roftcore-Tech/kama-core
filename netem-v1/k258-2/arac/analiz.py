#!/usr/bin/env python3
"""Senaryo v1 koşu analizi (ürün kodu değil). kullanım: analiz.py <koşu dizini> <çıktı öneki>
Tek zaman ekseni: τ = Unix zamanı − sync/t0 (alice'in Connected anı). Bayt = IP boyu (UDP/IP dahil)."""
import json, sys, statistics as st, collections, bisect
d, outp = sys.argv[1], sys.argv[2]
def jl(p):
    o = []
    for l in open(p):
        l = l.strip()
        if l:
            try: o.append(json.loads(l))
            except ValueError: pass
    return o
t0 = int(open(f"{d}/sync/t0").read())
tau = lambda ns: (ns - t0) / 1e9
tms = lambda ms: (ms * 1e6 - t0) / 1e9
alice, bob = jl(f"{d}/alice.jsonl"), jl(f"{d}/bob.jsonl")
hdr = [x for x in alice if x["k"] == "hdr"][0]
P = int(hdr["bob_addr"].split(":")[1])
asum, bsum = json.load(open(f"{d}/alice.json")), json.load(open(f"{d}/bob.json"))
END = tms(asum["ended"])
cap, cstat = {}, {}
for n in ("A_va0", "MID_ma0", "MID_ifb0", "MID_mb0", "B_vb0"):
    r = jl(f"{d}/cap/{n}.jsonl")
    cap[n] = [x for x in r if "ip_len" in x]
    cstat[n] = [x["capture_stats"] for x in r if "capture_stats" in x]
R = {}  # sonuç sözlüğü
out_lines = []
def P_(s=""):
    out_lines.append(s)
# ── akışlar
aout = [x for x in cap["A_va0"] if x["dir"] == "out"]
ain = [x for x in cap["A_va0"] if x["dir"] == "in"]
nonudp = [x for n in cap for x in cap[n] if x.get("proto") != 17]
flows = collections.Counter()
for n in ("MID_ma0", "MID_mb0", "MID_ifb0"):
    for x in cap[n]:
        if x.get("proto") == 17: flows[(n, x["dir"], x["sport"], x["dport"])] += 1
fwd_data = [x for x in aout if x.get("dport") == P and x.get("sport") != P]
fwd_ack = [x for x in aout if x.get("sport") == P]
rev_ack = [x for x in ain if x.get("sport") == P]      # B:P → A:geçici = Alice verisinin ACK/denetimi
rev_data = [x for x in ain if x.get("dport") == P]      # B:geçici → A:P = Bob'un verisi (alındılar)
ma_in = {x["pid"]: x for x in cap["MID_ma0"] if x["dir"] == "in" and x.get("proto") == 17}
ifb = {x["pid"]: x for x in cap["MID_ifb0"] if x.get("proto") == 17}
mbo = {x["pid"]: x for x in cap["MID_mb0"] if x["dir"] == "out" and x.get("proto") == 17}
bin_ = {x["pid"]: x for x in cap["B_vb0"] if x["dir"] == "in" and x.get("proto") == 17}
ao = {x["pid"]: x for x in aout if x.get("proto") == 17}
dup_pid = len([1 for x in aout if x.get("proto") == 17]) - len(ao)
# ── basamaklar ve kapasite
bas = [x for x in jl(f"{d}/basamak.jsonl") if x["k"] == "basamak"]
NSTEP = len([x for x in jl(f"{d}/basamak.jsonl") if x["k"] == "t0"][0]["plan"].split())
rate_of = {"50kbit": 6250, "100kbit": 12500}
steps = [(tau(x["komut_once_unix_ns"]), rate_of[x["hiz"]]) for x in bas]
def capacity(a, b):
    pts = [(0.0, 12500)] + steps
    tot = 0.0
    for i, (ts, r) in enumerate(pts):
        te = pts[i + 1][0] if i + 1 < len(pts) else 1e9
        lo, hi = max(a, ts), min(b, te)
        if hi > lo: tot += (hi - lo) * r
    return tot
def rate_at(t):
    r = 12500
    for ts, rr in steps:
        if t >= ts: r = rr
    return r
# ── kuyruk örnekleri
kq = jl(f"{d}/kuyruk.jsonl")
def qd(smp, kind, handle):
    for x in smp["q"]:
        if x.get("kind") == kind and x.get("handle") == handle: return x
ks = [(tau(s["t_unix_ns"]), qd(s, "tbf", "1:"), qd(s, "pfifo", "10:"), qd(s, "netem", "20:"), s["load1"]) for s in kq if isinstance(s["q"], list) and qd(s, "tbf", "1:")]
handles = set(tuple(sorted((x["kind"], x["handle"]) for x in s["q"] if x["kind"] in ("tbf", "pfifo", "netem", "ingress"))) for s in kq if isinstance(s["q"], list))
gaps = [(kq[i]["t_unix_ns"] - kq[i - 1]["t_unix_ns"]) / 1e6 for i in range(1, len(kq))]
# ── alice sondası (pacer tavanı)
PR_CAP = 12214  # link_pacing_cap_bps(12500) = 12500 × 1200 / 1228 (UDP gövdesi B/s)
ts = []
for x in alice:
    if x["k"] == "ts" and x["s"]:
        s0 = x["s"][0]
        ts.append((tms(x["t"]), s0["pr"], s0["bw"], s0["st"], s0["inb"], s0["txb"], s0["rtxb"], s0["txd"], s0["rxd"], s0["srtt"], (s0.get("rec") or {}).get("txq"), s0["par"], s0["dup"], s0["rtx"], x["out"]))
hat = lambda pr: pr * 12500.0 / PR_CAP
# tavan değişim olayları
chg = []
for i in range(1, len(ts)):
    if ts[i][1] != ts[i - 1][1]: chg.append((ts[i][0], ts[i - 1][1], ts[i][1]))
ack_t = sorted(tau(x["t_unix_ns"]) for x in rev_ack if x["ip_len"] >= 65)  # ACK adayı: 0x14 en az 65 B, 0x15/0x16 en az 67 B; altı ACK olamaz (RECOVERED 56 + 2n)
# ── metin / alındı
send = {x["i"]: x for x in alice if x["k"] == "send" and x["c"] == "T"}
rx = {x["i"]: x for x in bob if x["k"] == "rx" and x["c"] == "T" and x.get("first")}
rc = {x["i"]: x for x in alice if x["k"] == "rcpt" and x["c"] == "T" and x.get("first")}
texts = []
for i in sorted(send):
    s_ = send[i]; r_ = rx.get(i); c_ = rc.get(i)
    texts.append((i, tms(s_["t"]), (r_["t"] - s_["t"]) if r_ else None, (c_["t"] - r_["t"]) if (c_ and r_) else None, (c_["t"] - s_["t"]) if c_ else None))
# ── dosya ilerlemesi: bob motor kabulü (iz D), birim boyu > 200 B = dosya parçası
fileD = []
for x in bob:
    if x["k"] == "tr":
        for r in x["r"]:
            if r[0] == "D" and r[3] > 200: fileD.append((tms(r[4]), r[3]))
fileD.sort()
fcum_t = [a for a, _ in fileD]; fcum = []
c = 0
for _, l in fileD:
    c += l; fcum.append(c)
def file_at(t):
    i = bisect.bisect_right(fcum_t, t)
    return fcum[i - 1] if i else 0
# ── paket kaderi
fate = {}
for pid, x in ma_in.items():
    if pid not in ifb: fate[pid] = "kuyruk"
    elif pid not in mbo: fate[pid] = "hat"
    else: fate[pid] = "teslim" if pid in bin_ else "?"
soj = {pid: (ifb[pid]["t_unix_ns"] - ma_in[pid]["t_unix_ns"]) / 1e6 for pid in ifb if pid in ma_in}
NB = int(END) + 1
rows = []
def in_bin(lst, key, b):
    return [x for x in lst if b <= key(x) < b + 1]
A_all = [(tau(x["t_unix_ns"]), x["ip_len"], x["pid"]) for x in aout if x.get("proto") == 17]
A_data = [(tau(x["t_unix_ns"]), x["ip_len"]) for x in fwd_data]
I_all = [(tau(x["t_unix_ns"]), x["ip_len"], pid) for pid, x in ifb.items()]
M_arr = [(tau(x["t_unix_ns"]), pid) for pid, x in ma_in.items()]
ACKS = [(tau(x["t_unix_ns"]), x["ip_len"]) for x in rev_ack]
RCP = [(tau(x["t_unix_ns"]), x["ip_len"]) for x in rev_data]
hdrline = "s | kapasite B | sunulan B (veri akışı B) | darboğazdan çıkan B | kuyruk p son/enç | bekleme ms medyan/enç | kuyrukta düşen p | hatta düşen p | ACK varışı (τ:B) | tavan B/s hat-eşdeğeri enaz–ençok (pr son) | BtlBw | txq | metin teslim ms | alındı dönüş / gidiş-dönüş ms | dosya kB (Bob)"
P_(hdrline)
for b in range(NB):
    capb = capacity(b, b + 1)
    off = sum(l for t, l, _ in A_all if b <= t < b + 1)
    offd = sum(l for t, l in A_data if b <= t < b + 1)
    ex = [(t, l, p) for t, l, p in I_all if b <= t < b + 1]
    exb = sum(l for _, l, _ in ex)
    q_in = [k for k in ks if b <= k[0] < b + 1]
    qlast = q_in[-1][2]["qlen"] if q_in else None; qmax = max((k[2]["qlen"] for k in q_in), default=None)
    sj = [soj[p] for _, _, p in ex if p in soj]
    qdrop = sum(1 for t, p in M_arr if b <= t < b + 1 and fate[p] == "kuyruk")
    ldrop = sum(1 for t, _, p in ex if fate.get(p) == "hat")
    acks = [f"{t:.2f}:{l}" for t, l in ACKS if b <= t < b + 1]
    tsb = [x for x in ts if b <= x[0] < b + 1]
    prs = [x[1] for x in tsb]
    tx_ = [x for x in texts if b <= x[1] < b + 1]
    rows.append(dict(s=b, cap=capb, off=off, offd=offd, exb=exb, qlast=qlast, qmax=qmax, sj_med=(st.median(sj) if sj else None), sj_max=(max(sj) if sj else None),
                     qdrop=qdrop, ldrop=ldrop, acks=acks, pr_min=(min(prs) if prs else None), pr_max=(max(prs) if prs else None), pr_last=(prs[-1] if prs else None),
                     bw=(tsb[-1][2] if tsb else None), txq=(tsb[-1][10] if tsb else None), st=(tsb[-1][3] if tsb else None),
                     txt=[(x[0], x[2], x[3], x[4]) for x in tx_], file=file_at(b + 1)))
    r = rows[-1]
    f = lambda v, fmt="{:.0f}": "–" if v is None else fmt.format(v)
    P_(f"{b:3d} | {capb:6.0f} | {off:6d} ({offd:6d}) | {exb:6d} | {f(qlast)}/{f(qmax)} | {f(r['sj_med'])}/{f(r['sj_max'])} | {qdrop} | {ldrop} | {' '.join(acks) or '–'} | "
       f"{f(hat(r['pr_min']) if prs else None)}–{f(hat(r['pr_max']) if prs else None)} ({f(r['pr_last'])}) | {f(r['bw'])} | {f(r['txq'])} | "
       f"{' '.join(f(x[1]) for x in r['txt']) or '–'} | {' '.join(f(x[2])+'/'+f(x[3]) for x in r['txt']) or '–'} | {r['file']/1000:.1f}")
open(outp + "-zaman-cizelgesi.txt", "w").write("\n".join(out_lines) + "\n")
# ── özet ve hükümler
def win(a, b):
    off = sum(l for t, l, _ in A_all if a <= t < b); ex = sum(l for t, l, _ in I_all if a <= t < b); c_ = capacity(a, b)
    sj = [soj[p] for t, _, p in I_all if a <= t < b and p in soj]
    qdrop = sum(1 for t, p in M_arr if a <= t < b and fate[p] == "kuyruk")
    qmax = max((k[2]["qlen"] for k in ks if a <= k[0] < b), default=None)
    return dict(a=a, b=b, kapasite=c_, sunulan=off, cikan=ex, sunulan_yuzde=100 * off / c_ if c_ else None, cikan_yuzde=100 * ex / c_ if c_ else None,
                bekleme_medyan=(st.median(sj) if sj else None), bekleme_p95=(sorted(sj)[int(0.95 * (len(sj) - 1))] if sj else None), bekleme_enc=(max(sj) if sj else None), kuyrukta_dusen=qdrop, kuyruk_enc_p=qmax, paket=len(sj))
def pr_at(t):
    v = None
    for x in ts:
        if x[0] <= t: v = x[1]
        else: break
    return v
def acks_after(t, n):
    a = [x for x in ack_t if x > t]
    return a[n - 1] if len(a) >= n else None
O = []
def W(s=""): O.append(s)
tb_last, nf_last = ks[-1][1], ks[-1][3]
tot_ma = len(ma_in); tot_ifb = len(ifb); tot_mb = len(mbo); tot_b = len(bin_)
W(f"koşu: t0={t0} bitiş τ={END:.3f} s; alice sonuç={asum['result']} metin {asum['sent'].get('T')} gönderildi, bob {bsum['counts'].get('T')} aldı, alındı {asum['receipts']}; dosya bob={bsum['file']['status']} (bitmemesi bekleniyordu)")
W(f"listener_drops alice/bob={asum['listener_drops']}/{bsum['listener_drops']} jsonl_errors={asum['jsonl_errors']}/{bsum['jsonl_errors']} probe_errors={asum['probe_errors']}/{bsum['probe_errors']}")
W(f"yakalama kaybı: { {n: (s[0]['drops'] if s else 'kayıt yok') for n, s in cstat.items()} }; UDP dışı IPv4 paket: {len(nonudp)}; yinelenen paket kimliği: {dup_pid}")
W(f"akışlar (orta ad alanı): { {f'{k[0]} {k[1]} {k[2]}→{k[3]}': v for k, v in sorted(flows.items())} }")
W(f"ileri paket: A çıkış {len(ao)} ({sum(x['ip_len'] for x in ao.values())} B; veri akışı {len(fwd_data)} p {sum(x['ip_len'] for x in fwd_data)} B, Alice'in ACK akışı {len(fwd_ack)} p {sum(x['ip_len'] for x in fwd_ack)} B) · darboğaza varan {tot_ma} · darboğazdan çıkan {tot_ifb} ({sum(x['ip_len'] for x in ifb.values())} B) · hat sonrası (mb0 çıkış) {tot_mb} · B giriş {tot_b}")
kd = sum(1 for v in fate.values() if v == "kuyruk"); hd = sum(1 for v in fate.values() if v == "hat")
W(f"KUYRUKTA düşen {kd} p (tbf 1: drops sayacı {tb_last['drops']}) · HATTA (rastgele kayıp) düşen {hd} p = darboğazdan çıkanın %{100*hd/tot_ifb:.2f}'i (netem 20: drops sayacı {nf_last['drops']}); koşu sonunda kuyrukta/hatta kalan {tot_ma-kd-tot_ifb} / {tot_ifb-hd-tot_mb} p")
W(f"qdisc sayaçları (son örnek τ={ks[-1][0]:.2f}): tbf 1: sent {tb_last['packets']} p / {tb_last['bytes']} B, drops {tb_last['drops']}, overlimits {tb_last['overlimits']}, backlog {tb_last['qlen']} p; netem 20: sent {nf_last['packets']} p, drops {nf_last['drops']}")
W(f"örnekleyici: {len(kq)} örnek, aralık medyan {st.median(gaps):.1f} ms, en büyük {max(gaps):.1f} ms; load1 en yüksek {max(k[4] for k in ks):.2f}; görülen qdisc kümeleri: {len(handles)} → {sorted(handles)}")
W(f"dönüş: ACK/denetim akışı (B:{P}→A) A'ya varan {len(rev_ack)} p {sum(x['ip_len'] for x in rev_ack)} B, boy dağılımı {dict(sorted(collections.Counter(x['ip_len'] for x in rev_ack).items()))}; Bob veri akışı (alındılar) {len(rev_data)} p {sum(x['ip_len'] for x in rev_data)} B boy {dict(sorted(collections.Counter(x['ip_len'] for x in rev_data).items()))}")
v2ok = [l for l in (x['ip_len'] for x in rev_ack) if (l - 65) % 4 == 0 and 65 <= l <= 129]
W(f"ACK akışında V2 (0x14) ACK boyuyla (65 + 4k B) bağdaşan {len(v2ok)} p, bağdaşmayan {len(rev_ack)-len(v2ok)} p (bitmap 0x15/0x16 boyu 67 + bitmap; 83 B = PARAM_ACK adayı)")
trig = []
for t, pa, pb in chg:
    c_ = [(ta, l) for ta, l in ACKS if 0 <= t - ta <= 0.16]
    if c_: trig.append((round(t, 2), round(c_[-1][0], 2), c_[-1][1]))
non14 = [x for x in trig if (x[2] - 65) % 4 != 0]
W(f"KİP · dolaylı tel kanıtı: tavanı güncelleyen (varıştan ≤ 160 ms sonra tavan değişti) ACK datagramları (tavan τ, varış τ, IP B): {trig}; bunlardan 0x14 boyuyla (65 + 4k) BAĞDAŞMAYAN {len(non14)}/{len(trig)}: {[x[2] for x in non14]} (RECOVERED 56 + 2n ve PARAM_ACK 83 B tavanı güncellemez; 0x15/0x16 = 67 + bitmap)")
W("")
W("HIZ BASAMAKLARI (aynı kuyruk ifb0 tbf 1: / pfifo 10:)")
for x in bas:
    pt, pf = [q for q in x["once"]["q"] if q["kind"] == "tbf"][0], [q for q in x["once"]["q"] if q["kind"] == "pfifo"][0]
    at, af = [q for q in x["sonra"]["q"] if q["kind"] == "tbf"][0], [q for q in x["sonra"]["q"] if q["kind"] == "pfifo"][0]
    c0 = x["komut_once_unix_ns"]
    ser = sum(1 for p, v in ifb.items() if x["once"]["t_once_ns"] <= v["t_unix_ns"] <= x["sonra"]["t_sonra_ns"])
    arr = sum(1 for p, v in ma_in.items() if x["once"]["t_once_ns"] <= v["t_unix_ns"] <= x["sonra"]["t_sonra_ns"])
    after = sorted(v["t_unix_ns"] for v in ifb.values() if v["t_unix_ns"] >= c0)[:6]
    ivs = [round((after[i] - after[i - 1]) / 1e6, 1) for i in range(1, len(after))]
    lens = [v["ip_len"] for v in sorted(ifb.values(), key=lambda v: v["t_unix_ns"]) if v["t_unix_ns"] >= c0][:6]
    W(f" τ={x['ofset_s']} s → {x['hiz']} ({rate_of[x['hiz']]} B/s): rc={x['rc']} komut τ={tau(c0):.6f} → {tau(x['komut_sonra_unix_ns']):.6f} s (hedeften {(c0-x['hedef_unix_ns'])/1e6:+.3f} ms)")
    W(f"   ÖNCE  ({(c0-x['once']['t_once_ns'])/1e6:.1f} ms önce): tbf hız {pt['options']['rate']} backlog {pt['qlen']} p / {pt['backlog']} B, drops {pt['drops']}, sent {pt['packets']} | pfifo 10: limit {pf['options']['limit']} backlog {pf['qlen']} p drops {pf['drops']}")
    W(f"   SONRA ({(x['sonra']['t_sonra_ns']-x['komut_sonra_unix_ns'])/1e6:.1f} ms sonra): tbf hız {at['options']['rate']} backlog {at['qlen']} p / {at['backlog']} B, drops {at['drops']}, sent {at['packets']} | pfifo 10: limit {af['options']['limit']} backlog {af['qlen']} p drops {af['drops']}")
    W(f"   iki kayıt arasında: darboğaza varan {arr} p, çıkan {ser} p; backlog farkı {at['qlen']-pt['qlen']:+d} p; drops farkı {at['drops']-pt['drops']:+d}; basamaktan sonraki ilk çıkış aralıkları {ivs} ms (paket boyları {lens} B)")
# hız telde: plan dilimlerinde darboğaz çıkışı
segs = [(0, 20), (20, 60), (60, 85), (85.0, 85.5), (85.5, END)]
W("")
W("HIZ TELDE (darboğaz çıkışı, kuyruk dolu dönemlerde ardışık çıkış aralığından): dilim · çıkan B / kapasite B · bekleme>0 paketlerde (boy/aralık) medyan B/s")
for a, b in segs:
    evs = sorted((v["t_unix_ns"], v["ip_len"], p) for p, v in ifb.items() if a <= tau(v["t_unix_ns"]) < b)
    rr = []
    for i in range(1, len(evs)):
        if soj.get(evs[i][2], 0) > 20 and evs[i][1] >= 500:  # kuyrukta beklemiş (arkası dolu) tam boy paket
            dt = (evs[i][0] - evs[i - 1][0]) / 1e9
            if dt > 0: rr.append(evs[i][1] / dt)
    w_ = win(a, b)
    W(f" [{a}, {b:.1f}) s: çıkan {w_['cikan']} / kapasite {w_['kapasite']:.0f} B (%{w_['cikan_yuzde']:.1f}); sunulan {w_['sunulan']} B (%{w_['sunulan_yuzde']:.1f}); ardışık çıkış hızı medyan {st.median(rr) if rr else float('nan'):.0f} B/s (n={len(rr)})")
W("")
W(f"PACER TAVANI (alice sondası pr = pacer hızı, UDP gövdesi B/s; geri bildirimi aç kuralda pr = cc_feedback_cap; hat-eşdeğeri = pr × 12500 / {PR_CAP}); örnek aralığı {hdr['params']['probe_ms']} ms")
W(" değişimler (τ s: önce → sonra [hat-eşdeğeri]): " + "; ".join(f"{t:.2f}: {a}→{b} [{hat(b):.0f}]" for t, a, b in chg))
W(" ACK/denetim varışları (τ s:IP B): " + " ".join(f"{t:.2f}:{l}" for t, l in ACKS))
W("")
# CC
a1, a2 = acks_after(steps[0][0], 1), acks_after(steps[0][0], 2)
pre20 = [x for x in ts if 2.0 <= x[0] < steps[0][0]]
pre20_min = min((x[1] for x in pre20), default=None)
first_in = next((x[0] for x in ts if x[0] > steps[0][0] and 5625 <= hat(x[1]) <= 6875), None)
pr_a2 = pr_at(a2 + 0.15) if a2 else None
W("HÜKÜM GİRDİLERİ (ACK adayı = ACK/denetim akışında IP boyu ≥ 65 B datagram; daha kısası ACK olamaz)")
ex20 = sorted((tau(ifb[p_]["t_unix_ns"]), soj[p_]) for p_ in soj if 20 <= tau(ifb[p_]["t_unix_ns"]) < 60)
pk = max(ex20, key=lambda x: x[1])
after_pk = [x for x in ex20 if x[0] > pk[0]]
first_ok = next((x[0] for x in after_pk if x[1] <= 393), None)
last_bad = max((x[0] for x in ex20 if x[1] > 393), default=None)
W(f" KUYRUK (daralma 20 s): bekleme tepe {pk[1]:.0f} ms (çıkış τ={pk[0]:.2f}); tepeden sonra ≤ 393 ms'ye ilk iniş τ={first_ok} (basamaktan {None if first_ok is None else round(first_ok-steps[0][0],2)} s sonra); [20,60) içinde > 393 ms beklemenin son görüldüğü çıkış τ={last_bad}; > 393 ms bekleyen paket {sum(1 for x in ex20 if x[1]>393)}/{len(ex20)}")
W(f" CC: basamak τ={steps[0][0]:.3f}; sonraki 1. ACK τ={a1}, 2. ACK τ={a2}; 2. ACK'ten 150 ms sonra tavan pr={pr_a2} → hat-eşdeğeri {hat(pr_a2) if pr_a2 else None}; tavanın 5 625–6 875 aralığına ilk girdiği an τ={first_in} (basamaktan {None if first_in is None else round(first_in-steps[0][0],2)} s sonra)")
W(f"     20 s öncesi (τ 2–20) tavan en düşük pr={pre20_min} → hat-eşdeğeri {hat(pre20_min) if pre20_min else None}; τ<20 tavan izi: " + "; ".join(f"{t:.2f}:{b}" for t, a, b in chg if t < steps[0][0]))
w1 = win(35, 58)
W(f" HIZ SINIRLAMA [35, 58): sunulan/kapasite %{w1['sunulan_yuzde']:.1f} ({w1['sunulan']}/{w1['kapasite']:.0f} B), çıkan/kapasite %{w1['cikan_yuzde']:.1f}; kuyrukta düşen {w1['kuyrukta_dusen']} p; darboğaz beklemesi medyan {w1['bekleme_medyan']:.0f} ms, p95 {w1['bekleme_p95']:.0f} ms, en çok {w1['bekleme_enc']:.0f} ms (eşik ≈ 393 ms); kuyruk en çok {w1['kuyruk_enc_p']} p")
tx_nar = [x for x in texts if 20 <= x[1] < 60]
W(f"     daralmada (τ 20–60) metin teslim süresi ms: " + " ".join("–" if x[2] is None else f"{x[2]:.0f}" for x in tx_nar) + f" → ≤ 1 s olan {sum(1 for x in tx_nar if x[2] is not None and x[2] <= 1000)}/{len(tx_nar)}, en çok {max((x[2] for x in tx_nar if x[2] is not None), default=float('nan')):.0f} ms")
w2 = win(86.5, 98)
W(f"     kısa daralma sonrası [86,5, 98): sunulan/kapasite %{w2['sunulan_yuzde']:.1f}, çıkan/kapasite %{w2['cikan_yuzde']:.1f}, bekleme medyan {w2['bekleme_medyan']:.0f} ms en çok {w2['bekleme_enc']:.0f} ms, kuyrukta düşen {w2['kuyrukta_dusen']}, tavan (hat-eşd.) en az {hat(min(x[1] for x in ts if 86.5<=x[0]<98)):.0f} en çok {hat(max(x[1] for x in ts if 86.5<=x[0]<98)):.0f}  [karşılaştırma: kesmesiz koşu YOK → sınanmadı]")
w0 = win(70, 84)
W(f"     başvuru (kesme öncesi [70, 84)): sunulan/kapasite %{w0['sunulan_yuzde']:.1f}, çıkan/kapasite %{w0['cikan_yuzde']:.1f}, bekleme medyan {w0['bekleme_medyan']:.0f} ms en çok {w0['bekleme_enc']:.0f} ms")
pr58 = pr_at(58.0)
b1, b3 = acks_after(steps[1][0], 1), acks_after(steps[1][0], 3)
t_up = next((x[0] for x in ts if x[0] > steps[1][0] and hat(x[1]) >= 11875), None)
kalici = None
if t_up is not None:
    kalici = all(hat(x[1]) >= 11875 for x in ts if t_up <= x[0] < 84.0)
W(f" HIZLANMA: önkoşul τ=58 tavan pr={pr58} → hat-eşdeğeri {hat(pr58) if pr58 else None} (≈ 6 250 bekleniyor); basamak τ={steps[1][0]:.3f}; sonraki 1./3. ACK τ={b1}/{b3}; tavanın ≥ 11 875 olduğu ilk an τ={t_up}; [o an, 84) boyunca kalıcı mı: {kalici}; 3. ACK'te (+150 ms) tavan {hat(pr_at(b3+0.15)) if b3 else None}")
if t_up is not None and t_up + 1 < 84:
    w3 = win(t_up + 1, 84)
    W(f"     kararlı pencere [{t_up+1:.2f}, 84): sunulan/kapasite %{w3['sunulan_yuzde']:.1f}, çıkan/kapasite %{w3['cikan_yuzde']:.1f}, bekleme medyan {w3['bekleme_medyan']:.0f} ms, kuyrukta düşen {w3['kuyrukta_dusen']}")
else:
    W("     kararlı pencere: YOK (tavan 84 s'den önce ≥ 11 875 olmadı)")
W("")
dl = [x[2] for x in texts if x[2] is not None]; rl = [x[3] for x in texts if x[3] is not None]; rt = [x[4] for x in texts if x[4] is not None]
W(f"METİN: {len(texts)} gönderildi, {len(dl)} teslim; teslim süresi medyan {st.median(dl):.0f} ms, en çok {max(dl):.0f} ms, ≤ 1 s {sum(1 for v in dl if v<=1000)}/{len(texts)}")
W(f"ALINDI: {len(rt)} geldi; dönüş bacağı (bob uygulama teslimi → alice alındı) medyan {st.median(rl):.0f} ms, en çok {max(rl):.0f} ms, ≤ 4 s {sum(1 for v in rl if v<=4000)}/{len(texts)}; gidiş-dönüş medyan {st.median(rt):.0f} ms, en çok {max(rt):.0f} ms, > 4 s {sum(1 for v in rt if v>4000)}")
W(f"DOSYA: bob motor kabulü {fcum[-1] if fcum else 0} B / {hdr['params']['file_bytes']} B (τ={END:.0f} s); 20/60/85/bitiş: {file_at(20)}/{file_at(60)}/{file_at(85)}/{file_at(END)} B")
last = ts[-1]
W(f"ALICE son sonda: txb={last[5]} rtxb={last[6]} txd={last[7]} rxd={last[8]} parite={last[11]} kopya={last[12]} yeniden_iletim={last[13]} srtt={last[9]}")
f = lambda v, fmt="{:.0f}": "–" if v is None else fmt.format(v)
C = ["s | kapasite B | sunulan B | çıkan B | kuyruk p son/enç | bekleme ms med/enç | kuyrukta düşen | hatta düşen | ACK varışı τ:B | tavan B/s (hat-eşd.) enaz–ençok | metin teslim ms | alındı dönüş/gidiş-dönüş ms | dosya kB"]
for r in rows:
    C.append(f"{r['s']} | {r['cap']:.0f} | {r['off']} | {r['exb']} | {f(r['qlast'])}/{f(r['qmax'])} | {f(r['sj_med'])}/{f(r['sj_max'])} | {r['qdrop']} | {r['ldrop']} | {' '.join(r['acks']) or '–'} | "
             f"{f(hat(r['pr_min']) if r['pr_min'] is not None else None)}–{f(hat(r['pr_max']) if r['pr_max'] is not None else None)} | {' '.join(f(x[1]) for x in r['txt']) or '–'} | {' '.join(f(x[2])+'/'+f(x[3]) for x in r['txt']) or '–'} | {r['file']/1000:.0f}")
open(outp + "-siki.txt", "w").write("\n".join(C) + "\n")
qmax_s = max(ks, key=lambda k: k[2]["qlen"])
sj_max = max(((v, p) for p, v in soj.items()), default=(0, None))
W("")
W(f"GENEL: kuyruk en çok {qmax_s[2]['qlen']} p / {qmax_s[2]['backlog']} B (τ={qmax_s[0]:.2f}); darboğaz beklemesi en çok {sj_max[0]:.0f} ms (çıkış τ={tau(ifb[sj_max[1]]['t_unix_ns']):.2f}); "
  + "evre başına bekleme medyan/en çok ms: " + "; ".join(f"[{a_},{b_}) {win(a_,b_)['bekleme_medyan']:.0f}/{win(a_,b_)['bekleme_enc']:.0f}" for a_, b_ in ((0,20),(20,35),(35,58),(58,60),(60,64),(64,84),(84,86.5),(86.5,98))))
W(f"       gönderimin durduğu en uzun aralık (A çıkış, veri akışı): " + (lambda g: f"{g[0]*1000:.0f} ms (τ={g[1]:.2f}→{g[2]:.2f})")(max(((A_data[i][0]-A_data[i-1][0], A_data[i-1][0], A_data[i][0]) for i in range(1, len(A_data))), default=(0,0,0))))
import os
pre = json.loads(open(f"{d}/realized_pre.json").read()); post = json.loads(open(f"{d}/realized_post.json").read())
klog = open(os.path.join(d, "../../../kosu.log")).read() if os.path.exists(os.path.join(d, "../../../kosu.log")) else ""
K = []
K.append(("K1 yük < 6,0", max(k[4] for k in ks) < 6.0, f"en yüksek {max(k[4] for k in ks):.2f}"))
K.append(("K2 cargo/rustc yok", klog.count("cargo_rustc=0") == 2, f"kosu.log'da cargo_rustc=0 sayısı {klog.count('cargo_rustc=0')}"))
cd = [l.split("coredump_son=")[1].strip() for l in klog.splitlines() if "coredump_son=" in l]
K.append(("K3 yeni çekirdek dökümü yok", len(cd) == 2 and cd[0] == cd[1], f"{cd}"))
K.append(("K4 ipv6 off, BARRIER-FAIL yok", pre["ipv6"] == "off" and post["ipv6"] == "off" and not os.path.exists(os.path.join(d, "../../BARRIER-FAIL.txt")), f"{pre['ipv6']}/{post['ipv6']}"))
K.append(("K5 kol başı sayaçlar 0", all(x["packets"] == 0 and x["drops"] == 0 and x["backlog"] == 0 for x in pre["mid_qdisc"] if x["kind"] != "noqueue"), ""))
K.append(("K6 aynı qdisc (tbf 1: + pfifo 10: 64p)", len(handles) == 1 and all(k[2]["options"]["limit"] == 64 for k in ks), f"{len(handles)} küme"))
K.append(("K7 yakalama kaybı 0", all(s and s[0]["drops"] == 0 for s in cstat.values()), ""))
fwdnon = [x for n in ("MID_ma0", "MID_ifb0") for x in cap[n] if x.get("proto") != 17]
K.append(("K8 hat dışı trafik yok", len(fwdnon) == 0 and len(flows) and tb_last["packets"] == len(cap["MID_ifb0"]) and len({(k[2], k[3]) for k in flows}) == 4, f"darboğazda UDP dışı {len(fwdnon)}; UDP akışı {len({(k[2], k[3]) for k in flows})}; tbf sent {tb_last['packets']} = yakalanan {len(cap['MID_ifb0'])}; UDP dışı (dönüş/bitiş sonrası) {[(round(tau(x['t_unix_ns']),2), x['proto']) for x in nonudp][:4]}"))
pn = open(f"{d}/kayit/pid_ns.txt").read() if os.path.exists(f"{d}/kayit/pid_ns.txt") else ""
K.append(("K9 iki süreç, ayrı ad alanı, rc, kayıp yok", ("alice" in pn and "bob" in pn) and open(f"{d}/rc").read().strip() == "0" and asum["listener_drops"] == 0 and bsum["listener_drops"] == 0 and asum["jsonl_errors"] == 0 and bsum["jsonl_errors"] == 0, "pid_ns: " + " | ".join(l for l in pn.splitlines() if "net=" in l and ("alice" in l or "bob" in l))[:260]))
K.append(("K10 basamak rc=0, |sapma| ≤ 20 ms", len(bas) == NSTEP and all(x["rc"] == 0 and abs(x["komut_once_unix_ns"] - x["hedef_unix_ns"]) <= 20e6 for x in bas), f"{[round((x['komut_once_unix_ns']-x['hedef_unix_ns'])/1e6,3) for x in bas]} ms"))
K.append(("K11 örnek aralığı ≤ 300 ms", max(gaps) <= 300, f"en büyük {max(gaps):.1f} ms"))
W("")
W("KİRLİ DENETİMİ: " + ("TEMİZ" if all(k[1] for k in K) else "KİRLİ"))
for n_, ok, note in K: W(f" {'tuttu' if ok else 'TUTMADI'} · {n_} {('· ' + note) if note else ''}")
for fn in ("node-alice-transport-start.txt", "node-bob-transport-start.txt"):
    W(f"{fn}: " + " | ".join(open(f"{d}/kayit/{fn}").read().strip().splitlines()[1:4]))
open(outp + "-ozet.txt", "w").write("\n".join(O) + "\n")

PH = [(0, 20), (20, 35), (35, 58), (58, 60), (60, 64), (64, 84), (84, 86.5), (86.5, 98)]
def cand_after(t, n):
    return acks_after(t, n)
bas_out = []
for x in bas:
    pt = [q for q in x["once"]["q"] if q["kind"] == "tbf"][0]; at = [q for q in x["sonra"]["q"] if q["kind"] == "tbf"][0]
    pf = [q for q in x["once"]["q"] if q["kind"] == "pfifo"][0]; af = [q for q in x["sonra"]["q"] if q["kind"] == "pfifo"][0]
    ser = sum(1 for p_, v in ifb.items() if x["once"]["t_once_ns"] <= v["t_unix_ns"] <= x["sonra"]["t_sonra_ns"])
    arr = sum(1 for p_, v in ma_in.items() if x["once"]["t_once_ns"] <= v["t_unix_ns"] <= x["sonra"]["t_sonra_ns"])
    bas_out.append(dict(ofset=x["ofset_s"], hiz=rate_of[x["hiz"]], t_once=tau(x["komut_once_unix_ns"]), t_sonra=tau(x["komut_sonra_unix_ns"]), sapma_ms=(x["komut_once_unix_ns"] - x["hedef_unix_ns"]) / 1e6, rc=x["rc"],
                        once=dict(hiz=pt["options"]["rate"], q=pt["qlen"], b=pt["backlog"], drops=pt["drops"], sent=pt["packets"], fq=pf["qlen"], fdrops=pf["drops"], flimit=pf["options"]["limit"], handle=pt["handle"] + "/" + pf["handle"]),
                        sonra=dict(hiz=at["options"]["rate"], q=at["qlen"], b=at["backlog"], drops=at["drops"], sent=at["packets"], fq=af["qlen"], fdrops=af["drops"], flimit=af["options"]["limit"], handle=at["handle"] + "/" + af["handle"]), varan=arr, cikan=ser))
t_up84 = t_up if (t_up is not None and t_up < 84) else None
OUT = dict(end=END, texts_sent=len(texts), texts_del=len(dl), rcpt=len(rt), a_out=len(ao), a_out_b=sum(x["ip_len"] for x in ao.values()), ifb=tot_ifb, kd=kd, hd=hd, loss_pct=100 * hd / tot_ifb, b_in=tot_b,
           tbf_drops=tb_last["drops"], netem_drops=nf_last["drops"], ack_flow=len(rev_ack), ack_flow_b=sum(x["ip_len"] for x in rev_ack), ack_sizes=dict(sorted(collections.Counter(x["ip_len"] for x in rev_ack).items())),
           ack_cand=len(ack_t), trig=trig, non14=len(non14), load_max=max(k[4] for k in ks), gap_max=max(gaps), clean=all(k[1] for k in K), K=[(n_, ok, note) for n_, ok, note in K],
           phases=[dict(win(a_, b_)) for a_, b_ in PH], steps=steps, bas=bas_out, chg=chg, acks=ACKS,
           cc=dict(a1=a1, a2=a2, tavan_a2=(hat(pr_a2) if pr_a2 else None), first_in=first_in, pre20_min=(hat(pre20_min) if pre20_min else None)),
           hs=dict(w=w1, texts_narrow=[x[2] for x in tx_nar], w_short=w2, w_ref=w0, pk=pk, first_ok=first_ok, last_bad=last_bad, tavan_short=[hat(min(x[1] for x in ts if 86.5 <= x[0] < 98)), hat(max(x[1] for x in ts if 86.5 <= x[0] < 98))]),
           hz=dict(pre58=(hat(pr58) if pr58 else None), b1=b1, b3=b3, t_up=t_up, kalici=kalici, tavan_b3=(hat(pr_at(b3 + 0.15)) if b3 else None), tavan_max_after60=hat(max(x[1] for x in ts if x[0] >= 60)), tavan_84=hat(pr_at(84.0)), w=(win(t_up + 1, 84) if (t_up is not None and t_up + 1 < 84) else None), w6085=win(60, 85)),
           text=dict(med=st.median(dl), mx=max(dl), le1=sum(1 for v in dl if v <= 1000)), rc_=dict(ret_med=st.median(rl), ret_max=max(rl), ret_le4=sum(1 for v in rl if v <= 4000), rt_med=st.median(rt), rt_max=max(rt), rt_gt4=sum(1 for v in rt if v > 4000)),
           file=dict(end=(fcum[-1] if fcum else 0), t20=file_at(20), t60=file_at(60), t85=file_at(85)), alice_last=dict(txb=last[5], rtxb=last[6], txd=last[7], rxd=last[8], par=last[11], dup=last[12], rtx=last[13]),
           qmax=dict(p=qmax_s[2]["qlen"], b=qmax_s[2]["backlog"], t=qmax_s[0]), sj_max=dict(ms=sj_max[0], t=tau(ifb[sj_max[1]]["t_unix_ns"])), pause=max(((A_data[i][0]-A_data[i-1][0], A_data[i-1][0], A_data[i][0]) for i in range(1, len(A_data))), default=(0,0,0)))
json.dump(OUT, open(outp + "-veri.json", "w"))
print("\n".join(O))
