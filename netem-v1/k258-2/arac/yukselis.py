#!/usr/bin/env python3
"""lg.raised = true satırları: alıcıdaki çift aralığı (b / g) dökümü (ürün kodu değil; neden ataması ve hüküm üretmez).
KOŞUDAN SONRA eklendi (ön kayıtta yok): b0659064 koşusunda τ=48,56 s'de dar hat evresinde tavanın 12 500'e çıktığı görüldükten sonra.
kullanım: yukselis.py <koşu dizini> <çıktı>
Her raised satırı için: o ACK'in Bob'daki satırı (alarm / rise / t_alarm), önceki Bob ACK'inin `t` anından bu ACK'in `t` anına dek
B_vb0'a varan Alice veri akışı datagramlarının ardışık çiftleri: g = varış aralığı, b = ikincinin UDP gövdesi (IP boyu − 28),
b / g (receiver.rs PairRate ile aynı tanım; alıcının kendi saatinden değil yakalamadan), aynı çiftin darboğaz çıkışındaki
(MID_ifb0) aralığı, çiftin iki datagramının darboğaz beklemesi. Alıcının sayma kuralı (receiver.rs PairRate::on_arrival) yakalama
anlarına uygulanır: g < 0,9 × (b + 28) / 12 500 B/s ise çift SIKIŞMIŞ sayılır ve ardındaki çiftle birlikte atılır; b ≥ 800 B ve
g ≥ 2 ms olanlar sayılır. Sayılan çiftlerden b / g en büyük 5'i ve en büyük ikincisi yazılır. Daralma alarmındaki sıfırlama /
saklama (restart_at_drop_alarm, restore) bu dökümde UYGULANMAZ: pencere yalnız önceki Bob ACK'inin `t` anından başlar; kayıttaki
pair_bound_wire ile fark yanına yazılır."""
import json, sys, bisect
from collections import Counter
d, outp = sys.argv[1], sys.argv[2]
def jl(p):
    o = []
    for l in open(p):
        l = l.strip()
        if l:
            try: o.append(json.loads(l))
            except ValueError: pass
    return o
t0 = int(open(f"{d}/sync/t0").read()); tau = lambda n: (n - t0) / 1e9
msns = lambda ms: int(round(ms * 1e6))
alice, bob = jl(f"{d}/alice.jsonl"), jl(f"{d}/bob.jsonl")
P = int([x for x in alice if x["k"] == "hdr"][0]["bob_addr"].split(":")[1])
cap = {n: [x for x in jl(f"{d}/cap/{n}.jsonl") if "ip_len" in x and x.get("proto") == 17] for n in ("MID_ma0", "MID_ifb0", "B_vb0")}
bin_ = [x for x in cap["B_vb0"] if x["dir"] == "in" and x.get("dport") == P]
sp = Counter(x["sport"] for x in bin_).most_common(1)[0][0]
bin_ = sorted((x for x in bin_ if x["sport"] == sp), key=lambda x: x["t_unix_ns"])
ifb = {x["pid"]: x["t_unix_ns"] for x in cap["MID_ifb0"]}
ma = {x["pid"]: x["t_unix_ns"] for x in cap["MID_ma0"] if x["dir"] == "in"}
bas = sorted((x["komut_once_unix_ns"], x) for x in jl(f"{d}/basamak.jsonl") if x["k"] == "basamak")
def hat(T):
    r = "100kbit (12 500 B/s)"
    for t, x in bas:
        if t <= T: r = str(x.get("hiz") or x.get("rate") or x.get("hedef") or x.get("ofset_s"))
    return r
acks = sorted((x for x in bob if x["k"] == "ack"), key=lambda x: x["index"])
lgs = {x["index"]: x for x in alice if x["k"] == "lg"}
O = []; W = O.append
W("basamak kayıtları: " + "; ".join(f"τ={tau(t):.3f}: " + ", ".join(f"{k}={v}" for k, v in x.items() if k in ("ofset_s", "hiz", "rate", "rc")) for t, x in bas))
for i, a in enumerate(acks):
    l = lgs.get(a["index"])
    if not l or not l["raised"]: continue
    lo = msns(acks[i - 1]["t"]) if i else 0; hi = msns(a["t"]); after = [False]
    fx = lambda v: "–" if v is None else f"{tau(msns(v)):.3f}"
    W("")
    W(f"ACK {a['index']} · Alice lg τ={tau(msns(l['t'])):.3f}: pace {l['pace_before']}→{l['pace_after']}, rate {l['rate_before']}→{l['rate_after']}, raised {l['raised']}, verdict {'null' if l.get('verdict') is None else l['verdict']['outcome']}, decision {l['decision']}, g_ns {l['g_ns']}, pair_bound_wire {l.get('pair_bound_wire')}, note {l['note']}")
    W(f"  Bob ack satırı: t τ={fx(a['t'])}, t_wire τ={fx(a['t_wire'])}, alarm {a['alarm']}, rise {a['rise']}, wait_open {a['wait_open']}, t_alarm τ={fx(a['t_alarm'])}, t_evidence τ={fx(a['t_evidence'])}, t_detect τ={fx(a['t_detect'])}, detects {a['detects']}; önceki Bob ACK'i t τ={tau(lo):.3f}")
    rows = []
    for j in range(1, len(bin_)):
        x, p = bin_[j], bin_[j - 1]
        g = x["t_unix_ns"] - p["t_unix_ns"]; b = x["ip_len"] - 28
        sq = g * 1000 * 12500 < 900 * (b + 28) * 1_000_000_000
        skip = sq or after[0]; after[0] = sq
        if not (lo < x["t_unix_ns"] <= hi): continue
        rows.append(dict(sayilir=(not skip and b >= 800 and g >= 2_000_000), sikismis=sq, tau=tau(x["t_unix_ns"]), ip2=x["ip_len"], ip1=p["ip_len"], g_ms=g / 1e6, bg=b * 1e9 / g if g > 0 else None,
                         ifb_ms=(ifb[x["pid"]] - ifb[p["pid"]]) / 1e6, w1_ms=(ifb[p["pid"]] - ma[p["pid"]]) / 1e6, w2_ms=(ifb[x["pid"]] - ma[x["pid"]]) / 1e6,
                         onceki_ilk=p["t_unix_ns"] <= lo))
    big = sorted((r for r in rows if r["sayilir"]), key=lambda r: -r["bg"])[:5]
    W(f"  pencere (önceki ACK t, bu ACK t] içinde varan {len(rows)} datagram; sıkışmış sayılan çift {sum(1 for r in rows if r['sikismis'])}; sayılan çift {sum(1 for r in rows if r['sayilir'])}; sayılanlardan b / g en büyük 5:")
    W("  | ikincinin varışı τ | IP boyu ilk → ikinci B | g ms (Bob varışı) | b / g B/s | aynı çiftin darboğaz çıkış aralığı ms | ilkin darboğaz beklemesi ms | ikincinin darboğaz beklemesi ms |")
    W("  |---|---|---|---|---|---|---|")
    for r in big:
        W(f"  | {r['tau']:.3f} | {r['ip1']} → {r['ip2']} | {r['g_ms']:.2f} | {r['bg']:.0f} | {r['ifb_ms']:.2f} | {r['w1_ms']:.1f} | {r['w2_ms']:.1f} |")
    if len(big) >= 2: W(f"  sayılanların en büyük ikinci b / g = {big[1]['bg']:.0f} B/s (UDP gövdesi); × 1 228 / 1 200 = {big[1]['bg'] * 1228 / 1200:.0f} B/s (tel); kayıttaki pair_bound_wire {l.get('pair_bound_wire')}")
open(outp, "w").write("\n".join(O) + "\n"); print("\n".join(O))
