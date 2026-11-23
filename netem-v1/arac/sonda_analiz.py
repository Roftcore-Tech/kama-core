#!/usr/bin/env python3
import json, sys, statistics as st
root = sys.argv[1]
def jl(p):
    o = []
    for l in open(p):
        l = l.strip()
        if l:
            o.append(json.loads(l))
    return o
def q(qs, dev_kind, handle):
    for x in qs:
        if x.get("kind") == dev_kind and x.get("handle") == handle:
            return x
    return None
delivered_sets = []
for rep in (1, 2):
    d = f"{root}/sonda/rep{rep}"
    print(f"\n===== rep{rep} =====")
    print(open(f"{d}/tc-d-qdisc-show.txt").read().strip())
    a = jl(f"{d}/sonda_a.jsonl"); b = jl(f"{d}/sonda_b.jsonl")
    t0 = [x for x in a if x["k"] == "t0"][0]["t0_unix_ns"]; base = [x for x in a if x["k"] == "taban"][0]["t_unix_ns"]
    tx = {x["seq"]: x for x in a if x["k"] == "tx"}
    rxb = {x["seq"]: x for x in b if x["k"] == "rx"}
    cap = {n: [x for x in jl(f"{d}/cap/{n}.jsonl") if "ip_len" in x] for n in ("A_va0", "MID_ma0", "MID_ifb0", "MID_mb0", "B_vb0")}
    stats = {n: [x for x in jl(f"{d}/cap/{n}.jsonl") if "capture_stats" in x] for n in cap}
    print("yakalama kaybı:", {n: (s[0]["capture_stats"]["drops"] if s else "yok") for n, s in stats.items()})
    port = [x for x in cap["A_va0"] if x["dir"] == "out"][0]["dport"]
    fwd = lambda x: x.get("ip_len") == 1000
    pid_seq = {}
    # A_va0 out sırası = gönderim sırası (tek iş parçacığı): kimlik eşlemesi
    aout = [x for x in cap["A_va0"] if x["dir"] == "out" and fwd(x)]
    seqs = sorted(tx)
    assert len(aout) == len(seqs), (len(aout), len(seqs))
    for x, s in zip(aout, seqs):
        pid_seq[x["pid"]] = s
    ma_in = [x for x in cap["MID_ma0"] if x["dir"] == "in" and fwd(x)]
    ifb = [x for x in cap["MID_ifb0"] if fwd(x)]
    mb = [x for x in cap["MID_mb0"] if x["dir"] == "out" and fwd(x)]
    bin_ = [x for x in cap["B_vb0"] if x["dir"] == "in" and fwd(x)]
    print(f"ileri 1000 B paket: A çıkış {len(aout)} · ma0 giriş kaydı {len(ma_in)} (ifb dönüşü ikinci kez görünür) · ifb0 çıkış {len(ifb)} · mb0 çıkış {len(mb)} · B giriş {len(bin_)} · B uygulama {len(rxb)}")
    ifb_t = {pid_seq[x["pid"]]: x["t_unix_ns"] for x in ifb}
    bt = {pid_seq[x["pid"]]: x["t_unix_ns"] for x in bin_}
    # basamaklar
    bs = [x for x in jl(f"{d}/basamak.jsonl") if x["k"] == "basamak"]
    print("\nBASAMAKLAR (aynı kuyruk: ifb0 tbf 1: / pfifo 10:)")
    step_t = []
    for s in bs:
        pre_t, pre_f = q(s["once"]["q"], "tbf", "1:"), q(s["once"]["q"], "pfifo", "10:")
        po_t, po_f = q(s["sonra"]["q"], "tbf", "1:"), q(s["sonra"]["q"], "pfifo", "10:")
        step_t.append(s["komut_once_unix_ns"])
        ser = sum(1 for sq, t in ifb_t.items() if s["once"]["t_once_ns"] <= t <= s["sonra"]["t_sonra_ns"])
        print(f" t0+{s['ofset_s']} s → {s['hiz']}: rc={s['rc']} komut öncesi t0+{(s['komut_once_unix_ns']-t0)/1e6:.3f} ms, sonrası t0+{(s['komut_sonra_unix_ns']-t0)/1e6:.3f} ms "
              f"(hedeften sapma {(s['komut_once_unix_ns']-s['hedef_unix_ns'])/1e6:+.3f} ms, komut süresi {(s['komut_sonra_unix_ns']-s['komut_once_unix_ns'])/1e6:.3f} ms)")
        print(f"   ÖNCE  (komuttan {(s['komut_once_unix_ns']-s['once']['t_once_ns'])/1e6:.2f} ms önce): tbf 1: hız={pre_t['options']['rate']} B/s backlog={pre_t['backlog']} B/{pre_t['qlen']} p drops={pre_t['drops']} sent={pre_t['packets']} p | pfifo 10: limit={pre_f['options']['limit']} backlog={pre_f['backlog']} B/{pre_f['qlen']} p drops={pre_f['drops']}")
        print(f"   SONRA (komuttan {(s['sonra']['t_sonra_ns']-s['komut_sonra_unix_ns'])/1e6:.2f} ms sonra): tbf 1: hız={po_t['options']['rate']} B/s backlog={po_t['backlog']} B/{po_t['qlen']} p drops={po_t['drops']} sent={po_t['packets']} p | pfifo 10: limit={po_f['options']['limit']} backlog={po_f['backlog']} B/{po_f['qlen']} p drops={po_f['drops']}")
        print(f"   iki kayıt arasında ifb0'dan çıkan paket: {ser}; backlog farkı (önce − sonra) = {pre_t['qlen']-po_t['qlen']} p; sent farkı = {po_t['packets']-pre_t['packets']} p; drop farkı = {po_t['drops']-pre_t['drops']}")
    # çıkış aralıkları (faz S: kimlik 0..79)
    sS = sorted(s for s in ifb_t if s < 100)
    ev = [(ifb_t[s], s) for s in sS]
    iv = [((ev[i][0] - ev[i-1][0]) / 1e6, ev[i][0], ev[i][1]) for i in range(1, len(ev))]
    def seg(lo, hi):
        return [x[0] for x in iv if lo < x[1] - 0 and x[1] <= hi and x[1] - x[0]*1e6 >= lo]
    c1, c2 = step_t
    for name, lo, hi in (("basamak 1 öncesi (12 500)", 0, c1), ("basamaklar arası (6 250)", c1, c2), ("basamak 2 sonrası (12 500)", c2, 1 << 62)):
        v = seg(lo, hi)
        if v:
            print(f" darboğaz çıkış aralığı · {name}: n={len(v)} medyan={st.median(v):.3f} ms min={min(v):.3f} max={max(v):.3f}  → {1000/st.median(v)*1000:.1f} B/s")
    for i, c in enumerate(step_t, 1):
        cross = [x for x in iv if x[1] - x[0]*1e6 < c <= x[1]]
        nxt = [x for x in iv if x[1] - x[0]*1e6 >= c][:3]
        print(f" basamak {i} geçiş aralığı (basamağı içeren): {[round(x[0],2) for x in cross]} ms; basamaktan sonraki ilk üç aralık: {[round(x[0],2) for x in nxt]} ms; "
              f"basamaktan sonraki ilk çıkış +{min((t-c)/1e6 for t,_ in ev if t>=c):.2f} ms")
    # sayım
    kq = jl(f"{d}/kuyruk.jsonl")
    last = kq[-1]["q"]; first = kq[0]["q"]
    tb, nf = q(last, "tbf", "1:"), q(last, "netem", "20:")
    handles = set()
    for smp in kq:
        handles.add(tuple(sorted((x["kind"], x["handle"], x.get("parent", "kök"), json.dumps(x["options"].get("limit")) if x["kind"] in ("pfifo",) else "") for x in smp["q"] if x["kind"] in ("tbf", "pfifo"))))
    gaps = [(kq[i]["t_unix_ns"] - kq[i-1]["t_unix_ns"]) / 1e6 for i in range(1, len(kq))]
    print(f"\nörnekleyici: {len(kq)} örnek, aralık medyan {st.median(gaps):.1f} ms en büyük {max(gaps):.1f} ms; en yüksek load1 {max(x['load1'] for x in kq)}; görülen darboğaz qdisc kümeleri: {handles}")
    print(f"ilk örnek sayaçları: tbf sent={q(first,'tbf','1:')['packets']} drops={q(first,'tbf','1:')['drops']} | netem 20: sent={q(first,'netem','20:')['packets']} drops={q(first,'netem','20:')['drops']}")
    print(f"son örnek: tbf 1: sent={tb['packets']} p / {tb['bytes']} B drops={tb['drops']} | netem 20: sent={nf['packets']} drops={nf['drops']} | en büyük backlog {max(q(x['q'],'tbf','1:')['qlen'] for x in kq)} p")
    S_off = [s for s in tx if s < 100]; F_off = [s for s in tx if s >= 100]
    S_exit = [s for s in ifb_t if s < 100]; F_exit = [s for s in ifb_t if s >= 100]
    S_del = [s for s in rxb if s < 100]; F_del = [s for s in rxb if s >= 100]
    print(f"FAZ S (basamak): sunulan {len(S_off)} · darboğazdan çıkan {len(S_exit)} · kuyrukta düşen {len(S_off)-len(S_exit)} · hatta (netem) düşen {len(S_exit)-len([s for s in bt if s<100])} · teslim {len(S_del)} → basamak yüzünden kaybolan = {len(S_off)-(len(S_off)-len(S_exit))-(len(S_exit)-len([s for s in bt if s<100]))-len(S_del)}")
    print(f"FAZ F (FIFO 64): sunulan {len(F_off)} · darboğazdan çıkan {len(F_exit)} (beklenen 1 + 64) · kuyrukta düşen {len(F_off)-len(F_exit)} · teslim {len(F_del)}; kuyrukta düşen kimlikler {sorted(set(F_off)-set(F_exit))[:3]}…")
    tot_exit = len(ifb_t); tot_del = len(rxb)
    print(f"kayıp (hat): darboğazdan çıkan {tot_exit}, teslim {tot_del}, düşen {tot_exit-tot_del} = %{100*(tot_exit-tot_del)/tot_exit:.1f}; netem 20: drops sayacı {nf['drops']}")
    # gecikme: darboğaz çıkışı → B varışı
    dl = [(bt[s] - ifb_t[s]) / 1e6 for s in bt]
    print(f"hat gecikmesi (darboğaz çıkışı → B girişi): medyan {st.median(dl):.3f} ms min {min(dl):.3f} max {max(dl):.3f}; ilk paket gönderim → B: {(bt[0]-tx[0]['t_unix_ns'])/1e6 if 0 in bt else float('nan'):.2f} ms")
    # kayıp sırası: hatta düşen paket hat süresi harcadı mı
    sdel = sorted(s for s in bt if s < 100 and c2 < ifb_t[s])
    skip = [(bt[sdel[i]] - bt[sdel[i-1]]) / 1e6 / (sdel[i] - sdel[i-1]) for i in range(1, len(sdel)) if sdel[i] - sdel[i-1] > 1]
    cons = [(bt[sdel[i]] - bt[sdel[i-1]]) / 1e6 for i in range(1, len(sdel)) if sdel[i] - sdel[i-1] == 1]
    if skip and cons:
        print(f"kayıp ↔ darboğaz sırası: B'de ardışık kimlikli varış aralığı medyan {st.median(cons):.2f} ms (n={len(cons)}); kayıp atlayan aralık / atlanan kimlik farkı medyan {st.median(skip):.2f} ms (n={len(skip)}) → düşen paket hat süresi harcadı = kayıp darboğazdan SONRA")
    # dönüş
    rv = sorted(x["t_unix_ns"] for x in a if x["k"] == "rx")
    btx = [x for x in b if x["k"] == "tx"]
    if len(rv) > 1:
        iv2 = [(rv[i] - rv[i-1]) / 1e6 for i in range(1, len(rv))]
        print(f"dönüş: 8 × 48 B → {len(rv)} vardı; aralık medyan {st.median(iv2):.3f} ms → {48/st.median(iv2)*1000:.2f} B/s; ilk varış gönderimden {(rv[0]-btx[0]['t_unix_ns'])/1e6:.1f} ms sonra")
    delivered_sets.append(sorted(rxb))
    pre = json.loads(open(f"{d}/realized_pre.json").read())
    print("kol başı sayaçlar (realized_pre):", [(x["kind"], x["handle"], x["packets"], x["drops"], x["backlog"]) for x in pre["mid_qdisc"] if x["kind"] in ("tbf","pfifo","netem")], "ipv6:", pre["ipv6"])
print("\ntohum belirlenimi: rep1 ve rep2 teslim kümeleri aynı mı:", delivered_sets[0] == delivered_sets[1], f"({len(delivered_sets[0])} / {len(delivered_sets[1])} teslim)")
