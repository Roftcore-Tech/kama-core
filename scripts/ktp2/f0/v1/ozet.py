#!/usr/bin/env python3
"""[SENARYO v1] Kancalı koşu özeti (ürün kodu değil; yorum/hüküm üretmez).

kullanım: ozet.py <koşu dizini> <ack-tablo.json> <çıktı.txt>
    <ack-tablo.json> = ack_tablo.py çıktısı (<önek>.json); <koşu dizini> ack_tablo.py ile aynı.
    (Eski dört argümanlı çağrı da kabul edilir: üçüncü argüman okunmaz.)
Yazdıkları: 20 s basamağından sonraki ilk tavan kesmesinin parça dökümü (hat değişimi → Bob kanıtı →
    ACK hazır → tel → Alice girişi → lg kararı), kesme anında darboğaz kuyruğu, [35, 58) s darboğaz
    beklemesi, daralmadaki ([20, 60) s) metin teslim süreleri, basamak anlarında kuyruk.
Varsayımlar (senaryo v1'e ÖZGÜ sabitler; başka planda sayılar anlamını yitirir):
    * KOVALI darboğaz: kuyruk.jsonl ve basamak.jsonl içinde ifb0 `tbf 1:` + `pfifo 10:` aranır
      (V1_KADEME=netem teşhis kolunda çalışmaz);
    * daralmış hız 6 250 B/s (backlog ÷ 6 250), bekleme eşiği 393 ms, pencereler [20, 60) ve [35, 58) s;
    * 20 s basamağından sonra tavanı düşüren (pace_after < pace_before) en az bir ACK satırı var
      (yoksa betik IndexError ile durur)."""
import json, sys, statistics as st
_a = sys.argv[1:]
if len(_a) == 4:
    d, tj, _okunmaz, outp = _a
elif len(_a) == 3:
    d, tj, outp = _a
else:
    sys.exit("kullanım: ozet.py <koşu dizini> <ack-tablo.json> <çıktı.txt>")
def jl(p):
    o = []
    for l in open(p):
        l = l.strip()
        if l:
            try: o.append(json.loads(l))
            except ValueError: pass
    return o
T = json.load(open(tj)); t0 = T["t0_unix_ns"]; rows = T["satirlar"]
tau = lambda n: (n - t0) / 1e9
O = []; W = O.append
# ── ilk tavan kesmesi: 20 s basamağından sonra pace_after < pace_before olan ilk lg satırı
cut = [r for r in rows if r["basvuru_s"] == 20.0 and r["ack_rx_t"] is not None and r["ack_rx_t"] > 0 and r["pace_after"] < r["pace_before"]][0]
W(f"İLK TAVAN KESMESİ: bob ACK sıra {cut['sira']} (largest_pn {cut['largest_pn']}, IP {cut['ip_B']} B) ↔ alice ack_rx sıra {cut['ack_rx_sira']}; lg {cut['lg_karar']} pace {cut['pace_before']}→{cut['pace_after']}, rate {cut['rate_before']}→{cut['rate_after']}")
W(f" başvuru: 20 s basamağı komut başlangıcı τ={cut['basvuru_komut_once_tau']:.6f} s; komut dönüşü +{cut['basvuru_komut_sonra_ms']:.3f} ms")
W(f" düşüş → kesme (lg satırı anı): {cut['ack_rx_t']:.3f} ms")
p = [("1 hat değişimi → Bob t_evidence", 0.0, cut["t_evidence"]),
     ("2 t_evidence → t_ready", cut["t_evidence"], cut["t_ready"]),
     ("3 t_ready → t_wire", cut["t_ready"], cut["t_wire"]),
     ("4 t_wire → Alice arayüz girişi (yakalama)", cut["t_wire"], cut["yak_alice_giris"]),
     ("5 Alice arayüz girişi → lg kararı (ack_rx.t)", cut["yak_alice_giris"], cut["ack_rx_t"])]
tot = 0.0
for n, a, b in p:
    W(f"  parça {n}: {b - a:.3f} ms"); tot += b - a
W(f"  parçalar toplamı {tot:.3f} ms; düşüş → kesme {cut['ack_rx_t']:.3f} ms; kapanmayan fark {cut['ack_rx_t'] - tot:.3f} ms")
W(f"  parça 3 alt döküm: t_ready → t (motor çıkışı) {cut['t'] - cut['t_ready']:.3f} ms; t → t_wire {cut['t_wire'] - cut['t']:.3f} ms")
W(f"  parça 4 alt döküm (yakalama): t_wire → Bob arayüz çıkışı {cut['yak_bob_cikis'] - cut['t_wire']:.3f} ms; Bob arayüz çıkışı → orta düğüm girişi (mb0) {cut['yak_mb0_giris'] - cut['yak_bob_cikis']:.3f} ms; mb0 girişi → dönüş hattı çıkışı (ma0, netem sonrası) {cut['yak_donus_hatti_cikis'] - cut['yak_mb0_giris']:.3f} ms; dönüş hattı çıkışı → Alice arayüz girişi {cut['yak_alice_giris'] - cut['yak_donus_hatti_cikis']:.3f} ms")
W(f"  aynı ACK'in öbür Bob anları (20 s basamağına göre ms): t_detect {cut['t_detect']:.3f}, t_first_try {cut['t_first_try']:.3f}, t_first_defer {cut['t_first_defer']:.3f}, t_alarm {cut['t_alarm']:.3f}; erteleme {cut['defer']}, alarm sonrası {cut['defer_alarm']}; detects={cut['detects']}")
# ── kesme anında darboğaz kuyruğu (ifb0 örnekleyici)
cut_ns = t0 + int(round((cut["basvuru_komut_once_tau"] + cut["ack_rx_t"] / 1e3) * 1e9))
kq = jl(f"{d}/kuyruk.jsonl")
def q(s, kind, h):
    for x in s["q"]:
        if x.get("kind") == kind and x.get("handle") == h: return x
prev = [s for s in kq if s["t_unix_ns_son"] <= cut_ns][-1]; nxt = [s for s in kq if s["t_unix_ns"] >= cut_ns][0]
for lab, s in (("kesmeden önceki son örnek", prev), ("kesmeden sonraki ilk örnek", nxt)):
    tb, pf = q(s, "tbf", "1:"), q(s, "pfifo", "10:")
    W(f"KUYRUK · {lab}: τ={tau(s['t_unix_ns']):.3f} s ({(s['t_unix_ns'] - cut_ns) / 1e6:+.1f} ms); ifb0 tbf 1: backlog {tb['backlog']} B / {tb['qlen']} p, drops {tb['drops']}; pfifo 10: backlog {pf['backlog']} B / {pf['qlen']} p, drops {pf['drops']}; backlog ÷ 6 250 B/s = {tb['backlog'] / 6250:.3f} s")
# kesme anına en yakın darboğaz çıkışının ölçülen beklemesi (ma0 girişi → ifb0 çıkışı, paket kimliğiyle)
ma = {x["pid"]: x for x in jl(f"{d}/cap/MID_ma0.jsonl") if x.get("dir") == "in" and x.get("proto") == 17}
ifb = [x for x in jl(f"{d}/cap/MID_ifb0.jsonl") if x.get("proto") == 17 and x.get("pid") in ma]
soj = [(x["t_unix_ns"], (x["t_unix_ns"] - ma[x["pid"]]["t_unix_ns"]) / 1e6, x["ip_len"]) for x in ifb]
bef = [s for s in soj if s[0] <= cut_ns][-1]; aft = [s for s in soj if s[0] > cut_ns][0]
W(f"KUYRUK · kesmeden önceki son darboğaz çıkışı τ={tau(bef[0]):.3f} s: bekleme {bef[1]:.0f} ms ({bef[2]} B); sonraki ilk çıkış τ={tau(aft[0]):.3f} s: bekleme {aft[1]:.0f} ms ({aft[2]} B)")
inq = [(a_, x) for a_, x in ((ma[x["pid"]]["t_unix_ns"], x) for x in ifb) if a_ <= cut_ns < x["t_unix_ns"]]
W(f"KUYRUK · kesme anında darboğazda bekleyen (yakalamadan: ma0'a girmiş, ifb0'dan çıkmamış) {len(inq)} p / {sum(x['ip_len'] for _, x in inq)} B; bunların sonuncusunun çıkışı kesmeden {((max(x['t_unix_ns'] for _, x in inq) - cut_ns) / 1e9) if inq else 0:.3f} s sonra")
pk = max(soj, key=lambda s: s[1] if 20 <= tau(s[0]) < 60 else -1)
W(f"KUYRUK · [20, 60) s en büyük bekleme {pk[1]:.0f} ms (çıkış τ={tau(pk[0]):.3f} s)")
w = sorted(s[1] for s in soj if 35 <= tau(s[0]) < 58)
W(f"[35, 58) s darboğaz beklemesi (çıkış anına göre, {len(w)} paket): medyan {st.median(w):.0f} ms, p95 {w[int(0.95 * (len(w) - 1))]:.0f} ms, en çok {max(w):.0f} ms, en az {min(w):.0f} ms; > 393 ms bekleyen {sum(1 for v in w if v > 393)}")
qs = [(tau(s["t_unix_ns"]), q(s, "tbf", "1:")) for s in kq if isinstance(s["q"], list) and q(s, "tbf", "1:")]
w2 = [x[1] for x in qs if 35 <= x[0] < 58]
W(f"[35, 58) s ifb0 örnekleyici ({len(w2)} örnek): backlog medyan {st.median(x['backlog'] for x in w2):.0f} B, en çok {max(x['backlog'] for x in w2)} B / {max(x['qlen'] for x in w2)} p; drops (pencere başı→sonu) {w2[0]['drops']}→{w2[-1]['drops']}")
# ── metin
alice, bob = jl(f"{d}/alice.jsonl"), jl(f"{d}/bob.jsonl")
send = {x["i"]: x for x in alice if x["k"] == "send" and x["c"] == "T"}
rx = {x["i"]: x for x in bob if x["k"] == "rx" and x["c"] == "T" and x.get("first")}
tx = [(i, (send[i]["t"] * 1e6 - t0) / 1e9, rx[i]["t"] - send[i]["t"] if i in rx else None) for i in sorted(send)]
dar = [x for x in tx if 20 <= x[1] < 60]
wk = max((x for x in dar if x[2] is not None), key=lambda x: x[2])
W(f"METİN · daralmada (gönderim τ ∈ [20, 60), {len(dar)} metin; teslimsiz {sum(1 for x in dar if x[2] is None)}): en kötü gönderim→Bob uygulama teslimi {wk[2]:.0f} ms (metin {wk[0]}, gönderim τ={wk[1]:.3f} s); ≤ 1 s {sum(1 for x in dar if x[2] is not None and x[2] <= 1000)}/{len(dar)}; hepsi ms: {' '.join(f'{x[2]:.0f}' for x in dar)}")
# ── basamaklarda kuyruk
for x in [x for x in jl(f"{d}/basamak.jsonl") if x["k"] == "basamak"]:
    g = lambda s, k: [y for y in s["q"] if y["kind"] == k][0]
    pt, pf, at, af = g(x["once"], "tbf"), g(x["once"], "pfifo"), g(x["sonra"], "tbf"), g(x["sonra"], "pfifo")
    W(f"BASAMAK {x['ofset_s']:g} s → {x['hiz']}: rc={x['rc']}; komut τ={tau(x['komut_once_unix_ns']):.6f} → {tau(x['komut_sonra_unix_ns']):.6f} s; ÖNCE ({(x['komut_once_unix_ns'] - x['once']['t_once_ns']) / 1e6:.1f} ms önce) tbf {pt['handle']} backlog {pt['backlog']} B / {pt['qlen']} p drops {pt['drops']} sent {pt['packets']}, pfifo {pf['handle']} backlog {pf['qlen']} p drops {pf['drops']}; SONRA ({(x['sonra']['t_sonra_ns'] - x['komut_sonra_unix_ns']) / 1e6:.1f} ms sonra) tbf {at['handle']} backlog {at['backlog']} B / {at['qlen']} p drops {at['drops']} sent {at['packets']}, pfifo {af['handle']} backlog {af['qlen']} p drops {af['drops']}")
open(outp, "w").write("\n".join(O) + "\n"); print("\n".join(O))
