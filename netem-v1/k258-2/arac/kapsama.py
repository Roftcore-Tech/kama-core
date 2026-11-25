#!/usr/bin/env python3
"""Hüküm ACK'i başına ölçüm aralığının uçları ↔ yoklama penceresi (ürün kodu değil; neden ataması ve hüküm üretmez).
kullanım: kapsama.py <koşu dizini> <çıktı öneki>
Zaman ekseni: τ = Unix zamanı − sync/t0.
Kayıtta OLAN: lg.note.first_seq / last_seq / rx_span_ns, lg.verdict{outcome, expect_ns, abs_q_ns, growth_ns, interval_g_ns},
  lg.pace_before / pace_after / raised / at_ns / t, ack_rx.largest_pn, bob ack.t / t_wire / largest_pn.
Kayıtta OLMAYAN (bu ikilide yoklamanın start_ns / until_ns alanı ve seq başına gönderim anı yok) → TÜRETME:
 T1 seq ↔ yakalama: Alice veri akışı (A_va0 `out`, hedef kapı = Bob kapısı, kaynak kapı ≠ o kapı) datagramlarından IP boyu
    53 ve 85 B olanlar motor seq'i taşımaz (oturum denetim datagramları) sayılır; kalanlar gönderim sırasıyla motor
    datagramıdır ve seq = motor sırası (0'dan) + s0. s0 her Bob ACK'inden ayrı okunur: ACK satırının `t` anından önce
    B_vb0'a varmış SON motor datagramının sırası ↔ o ACK'in Alice'teki lg.largest_seq'i. Varsayım iki denetimle sınanır;
    biri tutmazsa eşleme GEÇERSİZ yazılır:
    D1 bütün ACK'ler aynı s0'ı veriyor;
    D2 note taşıyan her ACK'te (B_vb0 varışı(last_seq) − B_vb0 varışı(first_seq)) − rx_span_ns farkı |.| ≤ 2 ms
       (rx_span 256 µs adımlı; alıcının kendi saatinde ilk varışlar).
 T2 aralığın SON ucu = last_seq'in datagramı; T3 aralığın İLK ucu = first_seq'in datagramı (ikisi de T1 eşlemesiyle; her
    hüküm satırında D2 farkı yanında yazılır).
 T4 yoklama penceresi: yoklama yalnız ACK işlenirken başlar (line_guard.rs `begin_probe` ← `on_ack_measured`, start_ns = now_ns).
    Başlangıç satırı = hüküm satırından önceki, önceki hüküm satırından sonraki, pace_after > pace_before VE raised = false olan
    son lg satırı. taban = o satırın pace_before'u, yoklama tavanı = pace_after'ı. Süre (probe_to, min_datagrams = 8):
    span = clamp(80 ms × taban / (tavan − taban), min(8 × 1 228 B × 1e9 / tavan, 3 s), 3 s); bitiş = başlangıç + span.
    Denetim: (tavan − taban) × span / taban ↔ kayıttaki expect_ns (fark yazılır; pace_after tam sayıya yuvarlı olabilir,
    bu yüzden tavan − 1 ile de denenir).
 Gönderim anı = A_va0 `out` yakalama anı (motorun send_ns'inden sonra; fark yakalama gecikmesi kadar).
 kapsıyor (ürün ölçütü, line_guard.rs `covers`) = ilk ucun gönderimi ≤ yoklama başlangıcı.
 tamamını kapsıyor = ilk ucun gönderimi ≤ yoklama başlangıcı VE son ucun gönderimi ≥ yoklama bitişi.
 mtu_ns (hesap) = 1 228 B × 1e9 / rate_before (ölçülmüş hız; yoksa profil hattı 12 500)."""
import json, sys, bisect
from collections import Counter, defaultdict
d, outp = sys.argv[1], sys.argv[2]
MTU_WIRE = 1228
PROBE_QUEUE_NS = 80_000_000
SPAN_MAX = 3_000_000_000
LINE = 12500
def jl(p):
    o = []
    for l in open(p):
        l = l.strip()
        if l:
            try: o.append(json.loads(l))
            except ValueError: pass
    return o
t0 = int(open(f"{d}/sync/t0").read())
tau = lambda n: (n - t0) / 1e9
msns = lambda ms: int(round(ms * 1e6))
alice, bob = jl(f"{d}/alice.jsonl"), jl(f"{d}/bob.jsonl")
P = int([x for x in alice if x["k"] == "hdr"][0]["bob_addr"].split(":")[1])
capA = [x for x in jl(f"{d}/cap/A_va0.jsonl") if "ip_len" in x and x.get("proto") == 17]
capB = [x for x in jl(f"{d}/cap/B_vb0.jsonl") if "ip_len" in x and x.get("proto") == 17]
A = sorted((x for x in capA if x["dir"] == "out" and x.get("dport") == P and x.get("sport") != P), key=lambda x: x["t_unix_ns"])
SP = Counter(x["sport"] for x in A).most_common(1)[0][0]
A = [x for x in A if x["sport"] == SP]
barr = {}
for x in capB:
    if x["dir"] == "in" and x.get("dport") == P and x.get("sport") == SP: barr.setdefault(x["pid"], x["t_unix_ns"])
arrA = [barr.get(x["pid"]) for x in A]                      # k → Bob'a varış anı (None = varmadı)
recv = sorted((t, k) for k, t in enumerate(arrA) if t is not None)
recv_t = [t for t, _ in recv]
O = []; W = O.append; J = {}
W(f"Alice veri akışı (A_va0 out, kaynak kapı {SP} → {P}): {len(A)} datagram; Bob'a varan {len(recv)} (pid eşlemesi); aynı pid tekrarı (A) {len(A) - len(set(x['pid'] for x in A))}")
# ── ACK eşlemesi (ack_tablo.py ile aynı kural: largest_pn + sıra)
acks = [x for x in bob if x["k"] == "ack"]
arx = [x for x in alice if x["k"] == "ack_rx"]
lgs = sorted((x for x in alice if x["k"] == "lg"), key=lambda x: x["index"])
bq, aq = defaultdict(list), defaultdict(list)
for x in acks: bq[x["largest_pn"]].append(x)
for x in arx: aq[x["largest_pn"]].append(x)
lg_by_at = {x["at_ns"]: x for x in lgs}
pairs = []      # (bob ack, alice ack_rx, lg)
for pn, lst in bq.items():
    for i, x in enumerate(lst):
        a = aq[pn][i] if i < len(aq.get(pn, [])) else None
        pairs.append((x, a, lg_by_at.get(a["at_ns"]) if a else None))
pairs.sort(key=lambda p: p[0]["index"])
# ── T1: seq ↔ motor datagramı
NONMOTOR = (53, 85)
M = [k for k, x in enumerate(A) if x["ip_len"] not in NONMOTOR]      # motor sırası → k
ord_of_k = {k: i for i, k in enumerate(M)}
W(f"T1 · motor seq'i taşımaz sayılan (IP boyu 53 / 85 B): {len(A) - len(M)} datagram ({dict(Counter(x['ip_len'] for x in A if x['ip_len'] in NONMOTOR))}); motor datagramı {len(M)}; boy dağılımı {dict(Counter(x['ip_len'] for x in A).most_common())}")
recv_m = [(t, k) for t, k in recv if k in ord_of_k]
recv_mt = [t for t, _ in recv_m]
cs = []
for x, a, l in pairs:
    if not l: continue
    T = msns(x["t"])
    i = bisect.bisect_right(recv_mt, T) - 1
    cs.append((x["index"], None if i < 0 else l["largest_seq"] - ord_of_k[recv_m[i][1]], None if i < 0 else (T - recv_m[i][0]) / 1e6))
cc = Counter(c for _, c, _ in cs if c is not None)
s0 = cc.most_common(1)[0][0]
ok_d1 = cc[s0] == len(cs)
W(f"T1 · D1: s0 = {s0}; {cc[s0]}/{len(cs)} ACK aynı s0'ı verdi (dağılım {dict(cc)})")
W("  ACK başına (sıra: s0, son motor varışından ACK `t` anına ms): " + " ".join(f"{i}:{cv},{dt:.1f}" for i, cv, dt in cs if cv is not None))
def k_of_seq(q):
    i = q - s0
    return M[i] if 0 <= i < len(M) else None
d2 = []
for x, a, l in pairs:
    n = l.get("note") if l else None
    if not n or n["rx_span_ns"] is None: continue
    kf, kl = k_of_seq(n["first_seq"]), k_of_seq(n["last_seq"])
    if kf is None or kl is None or arrA[kf] is None or arrA[kl] is None:
        d2.append((l["index"], None)); continue
    d2.append((l["index"], ((arrA[kl] - arrA[kf]) - n["rx_span_ns"]) / 1e6))
bad2 = [i for i, r in d2 if r is None or abs(r) > 2.0]
ok_d2 = not bad2
W(f"T1 · D2: rx_span taşıyan {len(d2)} ACK'te (varış farkı − rx_span) ms: " + " ".join(f"{i}:{'VARMADI' if r is None else f'{r:+.3f}'}" for i, r in d2) + f"; |fark| > 2 ms ya da uç Bob'a varmamış: {len(bad2)} (sıralar {bad2})")
ok_map = ok_d1 and ok_d2
J["esleme"] = dict(s0=s0, d1_uyan=cc[s0], d1_toplam=len(cs), d2=d2, d2_tutmayan=bad2, gecerli=ok_map, motor_disi=len(A) - len(M))
W(f"T1 · eşleme {'GEÇERLİ (D1 ve D2 tuttu)' if ok_map else 'GEÇERSİZ — aşağıdaki gönderim anları kullanılmamalı'}")
def probe_window(i_row):
    """hüküm satırı i_row (lgs dizini) için başlangıç satırı ve pencere"""
    j = i_row - 1
    while j >= 0:
        r = lgs[j]
        if r.get("verdict"): return None
        if r["pace_after"] > r["pace_before"] and not r["raised"]: break
        j -= 1
    if j < 0: return None
    r = lgs[j]
    base = r["pace_before"]; exp_rec = lgs[i_row]["verdict"]["expect_ns"]
    best = None
    for up in (r["pace_after"], r["pace_after"] - 1, r["pace_after"] + 1):
        if up <= base: continue
        min_span = 8 * MTU_WIRE * 1_000_000_000 // up
        span = PROBE_QUEUE_NS * base // (up - base)
        span = max(min(min_span, SPAN_MAX), min(span, SPAN_MAX))
        exp = (up - base) * span // base
        cand = (abs(exp - exp_rec), up, span, exp)
        if best is None or cand < best: best = cand
    _, up, span, exp = best
    v = lgs[i_row]
    start = msns(v["t"]) - (v["at_ns"] - r["at_ns"])     # hüküm satırının Unix anı − at_ns farkı
    return dict(bas_sira=r["index"], bas_tau=tau(start), bit_tau=tau(start + span), start=start, end=start + span, span_ns=span, taban=base, tavan=up,
                tavan_kayit=r["pace_after"], expect_hesap=exp, expect_kayit=exp_rec, expect_fark_ns=exp - exp_rec)
rowsJ = []
W("")
W("HÜKÜM SATIRLARI — aralığın uçları ↔ yoklama penceresi (τ s; farklar ms)")
W("| ACK sıra | hüküm an τ | outcome | first_seq–last_seq | ilk uç gönderim τ (k, IP boyu; D2 farkı ms) | son uç gönderim τ (k, IP boyu) | yoklama başlangıç τ (ACK sıra) | yoklama bitiş τ (süre ms) | ilk uç − başlangıç ms | son uç − bitiş ms | kapsıyor (ürün ölçütü: ilk ≤ başlangıç) | tamamını kapsıyor (ilk ≤ başlangıç VE son ≥ bitiş) | expect denetimi (hesap − kayıt ns) |")
W("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
lg_idx = {l["index"]: i for i, l in enumerate(lgs)}
arx_by_at = {x["at_ns"]: x for x in arx}
for x, a, l in pairs:
    if not l or not l.get("verdict"): continue
    v = l["verdict"]; n = l.get("note")
    e = dict(sira=l["index"], tau=tau(msns(l["t"])), outcome=v["outcome"], expect_ns=v["expect_ns"], abs_q_ns=v["abs_q_ns"], growth_ns=v["growth_ns"],
             interval_g_ns=v["interval_g_ns"], pace=(l["pace_before"], l["pace_after"]), rate=(l["rate_before"], l["rate_after"]),
             backlog=l.get("backlog_bytes"), ek=l.get("extra_backlog_bytes"), probe_failed=l["probe_failed"], raised=l["raised"])
    pw = probe_window(lg_idx[l["index"]])
    e["yoklama"] = pw
    if n:
        e["first_seq"], e["last_seq"], e["rx_span_ns"] = n["first_seq"], n["last_seq"], n["rx_span_ns"]
        kf, kl = k_of_seq(n["first_seq"]), k_of_seq(n["last_seq"])
        e["ilk_k"], e["son_k"], e["son_pn"] = kf, kl, a["largest_pn"]
        e["ilk_tau"] = tau(A[kf]["t_unix_ns"]) if kf is not None else None
        e["son_tau"] = tau(A[kl]["t_unix_ns"]) if kl is not None else None
        e["ilk_ip"], e["son_ip"] = (A[kf]["ip_len"] if kf is not None else None), (A[kl]["ip_len"] if kl is not None else None)
        e["ilk_bob_varis_tau"] = tau(arrA[kf]) if kf is not None and arrA[kf] is not None else None
        e["son_bob_varis_tau"] = tau(arrA[kl]) if kl is not None and arrA[kl] is not None else None
        e["d2_fark_ms"] = dict(d2).get(l["index"])
        if pw and kf is not None and kl is not None and ok_map:
            ft, lt = A[kf]["t_unix_ns"], A[kl]["t_unix_ns"]
            e["ilk_eksi_bas_ms"] = (ft - pw["start"]) / 1e6
            e["ilk_eksi_bit_ms"] = (ft - pw["end"]) / 1e6
            e["son_eksi_bit_ms"] = (lt - pw["end"]) / 1e6
            e["kapsiyor_urun"] = ft <= pw["start"]
            e["tamamini"] = ft <= pw["start"] and lt >= pw["end"]
    # hesap: b0659064 kuralının kayıttaki girdiler + türetilen kapsama ile vereceği sonuç
    rate = l["rate_before"] if l["rate_before"] else LINE
    mtu_ns = MTU_WIRE * 1_000_000_000 // rate
    half = v["expect_ns"] // 2
    fp = v["abs_q_ns"] <= half and v["growth_ns"] <= half
    ff = v["abs_q_ns"] > half and v["growth_ns"] > half
    ps = v["growth_ns"] <= v["expect_ns"] + mtu_ns
    e.update(mtu_ns=mtu_ns, yari_ns=half, cephe_gecti=fp, cephe_kaldi=ff, yoklama_boyu=ps, sinir_ns=v["expect_ns"] + mtu_ns)
    g = v["interval_g_ns"]; cov = e.get("kapsiyor_urun")
    if v["outcome"] in ("Superseded", "NoEvidence") or g is None or cov is None: e["kural_hesap"] = None
    else:
        passed = fp and g <= half
        failed = (g > half and (cov or ff)) or ((not cov) and ff and ps)
        e["kural_hesap"] = "Passed" if passed else ("Failed" if failed else "Inconclusive")
    e["beklenen_fazla_B"] = (pw["taban"] * v["expect_ns"] // 1_000_000_000) if pw else None
    rowsJ.append(e)
    f3 = lambda z: "–" if z is None else f"{z:.3f}"
    if n:
        ilk = f"{f3(e['ilk_tau'])} (k {e['ilk_k']}, {e['ilk_ip']} B; D2 farkı {'–' if e['d2_fark_ms'] is None else format(e['d2_fark_ms'], '+.3f')})"
        son = f"{f3(e['son_tau'])} (k {e['son_k']}, {e['son_ip']} B; ACK'in largest_pn'i {e['son_pn']})"
    else:
        ilk = son = "aralık yok"
    yn = lambda b: "–" if b is None else ("EVET" if b else "HAYIR")
    W(f"| {e['sira']} | {e['tau']:.3f} | {e['outcome']} | {e.get('first_seq', '–')}–{e.get('last_seq', '–')} | {ilk} | {son} | " +
      (f"{pw['bas_tau']:.3f} ({pw['bas_sira']}) | {pw['bit_tau']:.3f} ({pw['span_ns'] / 1e6:.1f}) | " if pw else "bulunamadı | – | ") +
      (f"{e['ilk_eksi_bas_ms']:+.1f} | {e['son_eksi_bit_ms']:+.1f} | " if "ilk_eksi_bas_ms" in e else "– | – | ") +
      f"{yn(e.get('kapsiyor_urun'))} | {yn(e.get('tamamini'))} | " + (f"{pw['expect_fark_ns']:+d} (taban {pw['taban']}, tavan {pw['tavan']}; kayıtta pace_after {pw['tavan_kayit']})" if pw else "–") + " |")
W("")
W("HÜKÜM SATIRLARI — kuralın girdileri (kayıt) ve türetilen kapsama ile kural hesabı (ms)")
W("| ACK sıra | outcome (kayıt) | expect | expect/2 | abs_q | growth | interval_g | mtu_ns (hesap) | expect + mtu | cephe geçti / kaldı | growth ≤ expect + mtu | kapsıyor (türetme) | kural hesabı (b0659064) | kayıtla aynı | yazılan backlog + ek B | taban × expect B (hesap) | pace önce→sonra | rate önce→sonra | probe_failed / raised |")
W("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for e in rowsJ:
    m = lambda z: "–" if z is None else f"{z / 1e6:.2f}"
    yn = lambda b: "–" if b is None else ("EVET" if b else "HAYIR")
    wr = "yazılmadı" if e["backlog"] is None and e["ek"] is None else f"{e['backlog']} + {e['ek']}"
    W(f"| {e['sira']} | {e['outcome']} | {m(e['expect_ns'])} | {m(e['yari_ns'])} | {m(e['abs_q_ns'])} | {m(e['growth_ns'])} | {m(e['interval_g_ns'])} | {m(e['mtu_ns'])} | {m(e['sinir_ns'])} | {yn(e['cephe_gecti'])} / {yn(e['cephe_kaldi'])} | {yn(e['yoklama_boyu'])} | {yn(e.get('kapsiyor_urun'))} | {e['kural_hesap'] or '–'} | {'–' if e['kural_hesap'] is None else ('evet' if e['kural_hesap'] == e['outcome'] else 'HAYIR')} | {wr} | {e['beklenen_fazla_B'] if e['beklenen_fazla_B'] is not None else '–'} | {e['pace'][0]}→{e['pace'][1]} | {e['rate'][0]}→{e['rate'][1]} | {e['probe_failed']} / {e['raised']} |")
J["hukum"] = rowsJ
# ── yoklama başlangıç satırları (hükmü bu koşuda görülmeyenler dahil)
W("")
st = [l for l in lgs if l["pace_after"] > l["pace_before"] and not l["raised"]]
W("YOKLAMA BAŞLANGIÇ SATIRLARI (pace_after > pace_before VE raised = false): " + ("; ".join(f"sıra {l['index']} τ={tau(msns(l['t'])):.3f} {l['pace_before']}→{l['pace_after']}" + (f" [aynı satırda hüküm {l['verdict']['outcome']}]" if l.get('verdict') else "") for l in st) or "yok"))
J["yoklama_baslangic"] = [dict(sira=l["index"], tau=tau(msns(l["t"])), pace=(l["pace_before"], l["pace_after"])) for l in st]
json.dump(J, open(outp + ".json", "w"), ensure_ascii=False, indent=1, default=str)
open(outp + ".txt", "w").write("\n".join(O) + "\n")
print("\n".join(O))
