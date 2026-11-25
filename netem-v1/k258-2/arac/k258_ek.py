#!/usr/bin/env python3
"""#258 önce/sonra eki (ürün kodu değil; neden ataması ve hüküm üretmez).
kullanım: k258_ek.py <koşu dizini> <analiz-veri.json> <çıktı öneki>
Zaman ekseni: τ = Unix zamanı − sync/t0. Bayt = IP boyu (UDP/IP dahil; `tc` backlog ile aynı birim).
Tanımlar (koşudan ÖNCE donduruldu, on-kayit.txt):
 dönüş sıra beklemesi = (A_va0 `in` − B_vb0 `out`) − IP boyu / 62 B/s − 300 ms   (yakalamadan)
 gerçek darboğaz kuyruğu(T) = MID_ma0 `in` anı ≤ T < MID_ifb0 anı olan ileri paketlerin IP boyu toplamı (yakalamadan);
   yanında kuyruk.jsonl örnekleyicisinin T'den önceki son / sonraki ilk örneği (ifb0 tbf 1: backlog)
 yazılan = lg.backlog_bytes + lg.extra_backlog_bytes (ikisi de null ise "yazılmadı")
 sahte tutma = yazılan > gerçek(T) + 1 datagram; 1 datagram = 1 228 B (DEFAULT_MTU 1 200 + UDP/IP 28)
 sınıf: lg.verdict dolu → "yoklama hükmü"; lg.verdict null → "yoklamasız kuyruk tahmini"
 türetilmiş (backlog alanı olmayan kayıt için; iki koşuda da hesaplanır): lg anından sonraki ilk Alice veri
   datagramına (A_va0 `out`, veri akışı) dek süre ≥ 0,5 s ise türetilmiş yazılan = süre × (rate_after, yoksa pace_after)
 gönderici susması = Alice veri akışında (A_va0 `out`, hedef kapı = Bob kapısı, kaynak kapı ≠ o kapı) ardışık iki
   datagram arası ≥ 0,5 s; "boş hat süresi" = susmanın, gerçek darboğaz kuyruğunun 0 paket olduğu kısmı"""
import json, sys, statistics as st, bisect
from collections import defaultdict
d, aj, outp = sys.argv[1:4]
DG = 1228
RET_BPS = 62.0
RET_DELAY_MS = 300.0
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
msns = lambda ms: None if ms is None else ms * 1e6
alice, bob = jl(f"{d}/alice.jsonl"), jl(f"{d}/bob.jsonl")
asum, bsum = json.load(open(f"{d}/alice.json")), json.load(open(f"{d}/bob.json"))
AV = json.load(open(aj))
END = AV["end"]
P = int([x for x in alice if x["k"] == "hdr"][0]["bob_addr"].split(":")[1])
cap = {n: [x for x in jl(f"{d}/cap/{n}.jsonl") if "ip_len" in x and x.get("proto") == 17] for n in ("A_va0", "MID_ma0", "MID_ifb0", "MID_mb0", "B_vb0")}
bas = [x for x in jl(f"{d}/basamak.jsonl") if x["k"] == "basamak"]
step = {x["ofset_s"]: x["komut_once_unix_ns"] for x in bas}
O = []; W = O.append
J = {}
# ── ileri hat: darboğaz kuyruğu (yakalamadan)
ma_in = {x["pid"]: x for x in cap["MID_ma0"] if x["dir"] == "in"}
ifb = {x["pid"]: x for x in cap["MID_ifb0"]}
pk = sorted((x["t_unix_ns"], (ifb[p]["t_unix_ns"] if p in ifb else None), x["ip_len"]) for p, x in ma_in.items())
def q_at(T):
    n = b = 0
    for ti, to, l in pk:
        if ti > T: break
        if to is None or to > T:
            n += 1; b += l
    return n, b
ev = []
for ti, to, l in pk:
    ev.append((ti, 1)); ev.append((to if to is not None else t0 + int((END + 5) * 1e9), -1))
ev.sort()
empty = []   # kuyruğun 0 paket olduğu aralıklar (ns)
c = 0; last = t0
for t, k in ev:
    if c == 0 and t > last: empty.append((last, t))
    c += k; last = t
empty.append((last, t0 + int((END + 5) * 1e9)))
def empty_in(a, b):
    return sum(max(0, min(b, e1) - max(a, e0)) for e0, e1 in empty)
kq = jl(f"{d}/kuyruk.jsonl")
def qd(s, kind, h):
    if not isinstance(s["q"], list): return None
    for x in s["q"]:
        if x.get("kind") == kind and x.get("handle") == h: return x
ks = [(s["t_unix_ns"], s["t_unix_ns_son"], qd(s, "tbf", "1:")) for s in kq if qd(s, "tbf", "1:")]
ks_a = [x[0] for x in ks]; ks_b = [x[1] for x in ks]
def samp(T):
    i = bisect.bisect_right(ks_b, T) - 1          # komutu T'den önce bitmiş son örnek
    j = bisect.bisect_left(ks_a, T)               # komutu T'den sonra başlamış ilk örnek
    f = lambda k: None if not (0 <= k < len(ks)) else (ks[k][2]["backlog"], ks[k][2]["qlen"], (ks[k][0] - T) / 1e6)
    return f(i), f(j)
# ── Alice veri akışı çıkışı ve susmalar
a_out = sorted(x["t_unix_ns"] for x in cap["A_va0"] if x["dir"] == "out" and x.get("dport") == P and x.get("sport") != P)
end_ns = t0 + int(END * 1e9)
gaps = []
for i in range(1, len(a_out)):
    if a_out[i] - a_out[i - 1] >= 0.5e9: gaps.append((a_out[i - 1], a_out[i], False))
if a_out and end_ns - a_out[-1] >= 0.5e9: gaps.append((a_out[-1], end_ns, True))
def next_out(T):
    i = bisect.bisect_right(a_out, T)
    return a_out[i] if i < len(a_out) else None
def prev_out(T):
    i = bisect.bisect_right(a_out, T) - 1
    return a_out[i] if i >= 0 else None
# ── ACK eşlemesi (ack_tablo.py ile aynı kural)
acks = [x for x in bob if x["k"] == "ack"]
arx = [x for x in alice if x["k"] == "ack_rx"]
lgs = [x for x in alice if x["k"] == "lg"]
lg_by = {x["at_ns"]: x for x in lgs}
bq, aq = defaultdict(list), defaultdict(list)
for x in acks: bq[x["largest_pn"]].append(x)
for x in arx: aq[x["largest_pn"]].append(x)
b_out = [x for x in cap["B_vb0"] if x["dir"] == "out" and x.get("sport") == P]
a_in = {x["pid"]: x for x in cap["A_va0"] if x["dir"] == "in" and x.get("sport") == P}
rows = []
for pn, lst in bq.items():
    for i, x in enumerate(lst):
        a = aq[pn][i] if i < len(aq.get(pn, [])) else None
        cnd = [p for p in b_out if p["ip_len"] == x["bytes"] + 50 and x["t_wire"] is not None and abs(p["t_unix_ns"] - msns(x["t_wire"])) <= 5e6]
        pkt = cnd[0] if len(cnd) == 1 else None
        ain = a_in.get(pkt["pid"]) if pkt else None
        l = lg_by.get(a["at_ns"]) if a else None
        wait = None
        if pkt and ain:
            wait = (ain["t_unix_ns"] - pkt["t_unix_ns"]) / 1e6 - pkt["ip_len"] / RET_BPS * 1e3 - RET_DELAY_MS
        rows.append(dict(sira=x["index"], pn=pn, ip=(pkt["ip_len"] if pkt else None), alarm=x["alarm"], rise=x["rise"],
                         bob_out=(tau(pkt["t_unix_ns"]) if pkt else None), alice_in=(tau(ain["t_unix_ns"]) if ain else None),
                         donus_bekleme_ms=wait, ack_delay_us=(a["ack_delay_us"] if a else None), lg=l,
                         T=(int(msns(l["t"])) if l else None)))
rows.sort(key=lambda r: r["sira"])
f0 = lambda v, fmt="{:.0f}": "–" if v is None else fmt.format(v)
# ── (a) hüküm ACK'leri
W("(a) HÜKÜM ACK'LERİ — lg.verdict dolu satırlar (kayıtta verdict alanı yoksa: raised / probe_failed satırları ve g_ns)")
W("| sıra | pn | an τ s | outcome | expect/2 ms | abs_q ms | growth ms | interval_g ms | dönüş sıra beklemesi ms | alarm ACK'i | pace önce→sonra | rate önce→sonra | yazılan backlog+ek B | raised/probe_failed |")
W("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
has_v = any("verdict" in r["lg"] for r in rows if r["lg"])
ver = []
for r in rows:
    l = r["lg"]
    if not l: continue
    v = l.get("verdict")
    if has_v and v is None: continue
    if not has_v and not (l["raised"] or l["probe_failed"]): continue
    ms = lambda n: None if n is None else n / 1e6
    wr = None if l.get("backlog_bytes") is None and l.get("extra_backlog_bytes") is None else (l.get("backlog_bytes") or 0) + (l.get("extra_backlog_bytes") or 0)
    e = dict(sira=r["sira"], pn=r["pn"], tau=tau(r["T"]), outcome=(v["outcome"] if v else ("(alan yok) " + ("probe_failed" if l["probe_failed"] else "raised"))),
             half_ms=(ms(v["expect_ns"]) / 2 if v else None), abs_q_ms=(ms(v["abs_q_ns"]) if v else None), growth_ms=(ms(v["growth_ns"]) if v else None),
             interval_g_ms=(ms(v["interval_g_ns"]) if v else ms(l["g_ns"])), donus_ms=r["donus_bekleme_ms"], alarm=r["alarm"],
             pace=(l["pace_before"], l["pace_after"]), rate=(l["rate_before"], l["rate_after"]), yazilan=wr, raised=l["raised"], probe_failed=l["probe_failed"],
             b1=(bool(v) and v["outcome"] == "Failed" and v["interval_g_ns"] is not None and v["interval_g_ns"] <= v["expect_ns"] / 2))
    ver.append(e)
    W(f"| {e['sira']} | {e['pn']} | {e['tau']:.3f} | {e['outcome']} | {f0(e['half_ms'], '{:.2f}')} | {f0(e['abs_q_ms'], '{:.2f}')} | {f0(e['growth_ms'], '{:.2f}')} | {f0(e['interval_g_ms'], '{:.3f}')} | {f0(e['donus_ms'], '{:.1f}')} | {'E' if e['alarm'] else 'H'} | {e['pace'][0]}→{e['pace'][1]} | {e['rate'][0]}→{e['rate'][1]} | {'yazılmadı' if wr is None else wr} | {e['raised']}/{e['probe_failed']} |")
J["hukum"] = ver
if has_v:
    from collections import Counter
    W(f"hüküm satırı {len(ver)}; outcome dağılımı {dict(Counter(e['outcome'] for e in ver))}")
    W(f"B1 · outcome=Failed VE interval_g_ns ≤ expect_ns/2 olan satır: {sum(1 for e in ver if e['b1'])} (sıralar {[e['sira'] for e in ver if e['b1']]})")
    inc = [e for e in ver if e["outcome"] == "Inconclusive"]
    W(f"B2 · outcome=Inconclusive satırları: {len(inc)}; " + "; ".join(f"sıra {e['sira']}: dönüş beklemesi {f0(e['donus_ms'], '{:.1f}')} ms, interval_g {f0(e['interval_g_ms'], '{:.3f}')} ms, abs_q {e['abs_q_ms']:.2f} / growth {e['growth_ms']:.2f} / yarı {e['half_ms']:.2f} ms, yazılan {'yazılmadı' if e['yazilan'] is None else e['yazilan']}, pace {e['pace'][0]}→{e['pace'][1]}, rate {e['rate'][0]}→{e['rate'][1]}, probe_failed {e['probe_failed']}" for e in inc))
else:
    W(f"(kayıtta verdict alanı yok) raised/probe_failed satırı {len(ver)}; probe_failed VE g_ns bilgisi: " + "; ".join(f"sıra {e['sira']}: g_ns {f0(e['interval_g_ms'], '{:.3f}')} ms, dönüş beklemesi {f0(e['donus_ms'], '{:.1f}')} ms" for e in ver if e["probe_failed"]))
W("")
# ── (b) sahte tutma
W("(b) lg SATIRI BAŞINA: yazılan kuyruk ↔ gerçek darboğaz kuyruğu ↔ dönüş sıra beklemesi")
W("| sıra | an τ s | karar | hüküm | front_wait ms | by_bytes | by_delay | yazılan (backlog+ek) B | gerçek kuyruk B / p (yakalama) | örnekleyici önce / sonra B | dönüş sıra beklemesi ms | ack_delay ms | sonraki veri datagramına dek s | türetilmiş yazılan B | sahte (kayıt) | sahte (türetilmiş) |")
W("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
B = []
for r in rows:
    l = r["lg"]
    if not l: continue
    T = r["T"]
    qn, qb = q_at(T)
    s_pre, s_post = samp(T)
    bb, xb = l.get("backlog_bytes"), l.get("extra_backlog_bytes")
    wr = None if bb is None and xb is None else (bb or 0) + (xb or 0)
    nx = next_out(T); hold = (nx - T) / 1e9 if nx else (end_ns - T) / 1e9
    rate = l["rate_after"] if l["rate_after"] is not None else l["pace_after"]
    der = hold * rate if hold >= 0.5 else None
    v = l.get("verdict")
    e = dict(sira=r["sira"], tau=tau(T), karar=l["decision"], hukum=(v["outcome"] if v else None), has_verdict_field=("verdict" in l),
             front_wait_ms=(l["front_wait_ns"] / 1e6 if l.get("front_wait_ns") is not None else None), by_bytes=l.get("by_bytes"), by_delay=l.get("by_delay"),
             backlog=bb, ek=xb, yazilan=wr, gercek_b=qb, gercek_p=qn, ornek_once=s_pre, ornek_sonra=s_post, donus_ms=r["donus_bekleme_ms"],
             ack_delay_ms=(r["ack_delay_us"] / 1e3 if r["ack_delay_us"] is not None else None), sonraki_veri_s=hold, sonraki_veri_kesik=(nx is None),
             turetilmis=der, sahte_kayit=(None if wr is None else wr > qb + DG), sahte_turetilmis=(None if der is None else der > qb + DG),
             sinif=("yoklama hükmü" if v else "yoklamasız kuyruk tahmini"), probe_failed=l["probe_failed"], raised=l["raised"], rate=rate)
    B.append(e)
    sm = lambda s: "–" if s is None else f"{s[0]}"
    W(f"| {e['sira']} | {e['tau']:.3f} | {e['karar']} | {e['hukum'] or '–'} | {f0(e['front_wait_ms'], '{:.1f}')} | {f0(e['by_bytes'])} | {f0(e['by_delay'])} | {'yazılmadı' if wr is None else f'{wr} ({bb}+{xb})'} | {qb} / {qn} | {sm(s_pre)} / {sm(s_post)} | {f0(e['donus_ms'], '{:.1f}')} | {f0(e['ack_delay_ms'], '{:.1f}')} | {hold:.3f}{' (koşu sonu)' if nx is None else ''} | {f0(der)} | {'–' if e['sahte_kayit'] is None else ('EVET' if e['sahte_kayit'] else 'hayır')} | {'–' if e['sahte_turetilmis'] is None else ('EVET' if e['sahte_turetilmis'] else 'hayır')} |")
J["lg"] = B
sk = [e for e in B if e["sahte_kayit"]]
sd = [e for e in B if e["sahte_turetilmis"]]
if any(e["yazilan"] is not None or e["has_verdict_field"] for e in B):
    W(f"SAHTE TUTMA (kayıttan; yazılan > gerçek + {DG} B): {len(sk)} satır; yoklama hükmünden {sum(1 for e in sk if e['sinif'] == 'yoklama hükmü')} (sıralar {[e['sira'] for e in sk if e['sinif'] == 'yoklama hükmü']}), yoklamasız kuyruk tahmininden {sum(1 for e in sk if e['sinif'] != 'yoklama hükmü')} (sıralar {[e['sira'] for e in sk if e['sinif'] != 'yoklama hükmü']}); yazılan − gerçek toplamı {sum(e['yazilan'] - e['gercek_b'] for e in sk)} B; (yazılan − gerçek) ÷ rate toplamı {sum((e['yazilan'] - e['gercek_b']) / e['rate'] for e in sk):.3f} s")
    W(f" kuyruk yazılan satır {sum(1 for e in B if e['yazilan'] is not None)} / {len(B)}; yazılmayan {sum(1 for e in B if e['yazilan'] is None)}")
else:
    W("SAHTE TUTMA (kayıttan): kayıtta backlog alanı yok → yalnız türetilmiş sayım")
cls = lambda e: ("yoklama hükmü" if (e["hukum"] or e["probe_failed"] or e["raised"]) else "yoklamasız")
W(f"SAHTE TUTMA (türetilmiş; lg anından sonraki veri datagramına dek ≥ 0,5 s VE süre × rate > gerçek + {DG} B): {len(sd)} satır (sıralar {[e['sira'] for e in sd]}); hüküm / probe_failed / raised taşıyan satır {sum(1 for e in sd if cls(e) == 'yoklama hükmü')} (sıralar {[e['sira'] for e in sd if cls(e) == 'yoklama hükmü']}), taşımayan {sum(1 for e in sd if cls(e) != 'yoklama hükmü')} (sıralar {[e['sira'] for e in sd if cls(e) != 'yoklama hükmü']}); süre toplamı {sum(e['sonraki_veri_s'] for e in sd):.3f} s")
W("")
W("GÖNDERİCİ SUSMALARI — Alice veri akışı çıkışında ≥ 0,5 s boşluk (τ ≥ 2 s; dosya koşu sonunda teslim edilmemiş: alice.json file.delivered = " + str(asum["file"]["delivered"]) + ", bob.json file.status = " + str(bsum["file"]["status"]) + ")")
W("| başlangıç τ s | bitiş τ s | süre s | başta gerçek kuyruk B / p | boş hat süresi s | boşluk içindeki lg satırları (sıra) | boşluğun içindeki metin gönderimleri (τ) |")
W("|---|---|---|---|---|---|---|")
txt = sorted(tau(int(msns(x["t"]))) for x in alice if x["k"] == "send" and x["c"] == "T")
G = []
for a, b, cut in gaps:
    if tau(a) < 2.0: continue
    qn, qb = q_at(a)
    em = empty_in(a, b) / 1e9
    inl = [r["sira"] for r in rows if r["T"] is not None and a <= r["T"] < b]
    tx = [round(t, 1) for t in txt if tau(a) <= t < tau(b)]
    G.append(dict(a=tau(a), b=tau(b), sure=(b - a) / 1e9, q_b=qb, q_p=qn, bos=em, lg=inl, metin=tx, kosu_sonu=cut))
    W(f"| {tau(a):.3f} | {tau(b):.3f}{' (koşu sonu)' if cut else ''} | {(b - a) / 1e9:.3f} | {qb} / {qn} | {em:.3f} | {inl} | {tx} |")
J["susma"] = G
Gb = [g for g in G if g["bos"] >= 0.5]
W(f"susma (≥ 0,5 s) {len(G)} adet, toplam {sum(g['sure'] for g in G):.3f} s; boş hat süresi ≥ 0,5 s olan {len(Gb)} adet, bunların süre toplamı {sum(g['sure'] for g in Gb):.3f} s, boş hat süresi toplamı {sum(g['bos'] for g in Gb):.3f} s")
for a_, b_ in ((2, 20), (20, 60), (60, 85), (85, END)):
    gg = [g for g in Gb if a_ <= g["a"] < b_]
    W(f"  başlangıcı [{a_}, {b_:.0f}) s: {len(gg)} adet, süre {sum(g['sure'] for g in gg):.3f} s, boş hat {sum(g['bos'] for g in gg):.3f} s")
W("")
# ── (c) yan yana girdileri
W("(c) ÖNCE ↔ SONRA GİRDİLERİ (bu koşu)")
lgr = [r for r in rows if r["lg"]]
s20, s60 = step[20.0], step[60.0]
cutr = [r for r in lgr if r["T"] > s20 and r["lg"]["pace_after"] < r["lg"]["pace_before"]]
C = {}
if cutr:
    c0 = cutr[0]; C["dusus_kesme_ms"] = (c0["T"] - s20) / 1e6
    W(f" düşüş → ilk kesme: {(c0['T'] - s20) / 1e6:.1f} ms (20 s basamağı komut başlangıcı τ={tau(s20):.6f} → lg sıra {c0['sira']} τ={tau(c0['T']):.3f}; {c0['lg']['decision']}, pace {c0['lg']['pace_before']}→{c0['lg']['pace_after']})")
else:
    C["dusus_kesme_ms"] = None; W(" düşüş → ilk kesme: YOK")
a20 = [r for r in lgr if r["T"] > s20]
C["ack_20"] = [(r["sira"], tau(r["T"]), r["lg"]["pace_before"], r["lg"]["pace_after"]) for r in a20[:3]]
W(" 20 s'den sonraki ilk üç ACK (sıra, varış τ, pace önce→sonra): " + "; ".join(f"{s}: τ={t:.3f} {a}→{b}" for s, t, a, b in C["ack_20"]) + f" → 2. ACK'te tavan {C['ack_20'][1][3] if len(C['ack_20']) > 1 else None} B/s (bant 5 625–6 875); analiz.py (sonda, 2. ACK adayı + 150 ms, hat-eşdeğeri) {AV['cc']['tavan_a2']}")
pre20 = [r["lg"]["pace_after"] for r in lgr if r["T"] <= s20]
C["pre20_min"] = min(pre20) if pre20 else None
W(f" 20 s'den önce lg pace_after en düşük {C['pre20_min']}; sonda (τ 2–20) hat-eşdeğeri en düşük {AV['cc']['pre20_min']}")
a60 = [r for r in lgr if r["T"] > s60]
C["ack_60"] = [(r["sira"], tau(r["T"]), r["lg"]["pace_before"], r["lg"]["pace_after"]) for r in a60[:3]]
C["pace_max_60"] = max((r["lg"]["pace_after"] for r in a60), default=None)
C["pace_after_60"] = [r["lg"]["pace_after"] for r in a60]
hz = AV["hz"]
W(" 60 s'den sonraki ilk üç ACK (sıra, varış τ, pace önce→sonra): " + "; ".join(f"{s}: τ={t:.3f} {a}→{b}" for s, t, a, b in C["ack_60"]))
W(f" 60 s'den sonra lg pace_after dizisi {C['pace_after_60']}; en büyük {C['pace_max_60']}; sonda en büyük tavan (hat-eşdeğeri, τ ≥ 60) {hz['tavan_max_after60']:.0f}; τ=58 tavan (sonda, hat-eşd.) {hz['pre58']}; τ=84 tavan {hz['tavan_84']:.0f}; ≥ 11 875 ilk an {hz['t_up']}; kalıcı {hz['kalici']}")
lg58 = [r for r in lgr if r["T"] <= t0 + int(58e9)]
C["pace_58"] = lg58[-1]["lg"]["pace_after"] if lg58 else None
W(f" τ=58'de geçerli lg pace_after (58 s'den önceki son ACK): {C['pace_58']}")
w = hz["w6085"]
C["w6085"] = w
W(f" [60, 85) s: sunulan {w['sunulan']} B = {w['sunulan'] / 25:.0f} B/s; kapasite {w['kapasite']:.0f} B; sunulan/kapasite %{w['sunulan_yuzde']:.1f}; darboğazdan çıkan {w['cikan']} B = {w['cikan'] / 25:.0f} B/s (%{w['cikan_yuzde']:.1f}); kuyrukta düşen {w['kuyrukta_dusen']}")
w35 = AV["hs"]["w"]
C["w3558"] = w35
soj = sorted((ifb[p]["t_unix_ns"] - ma_in[p]["t_unix_ns"]) / 1e6 for p in ifb if p in ma_in and 35 <= tau(ifb[p]["t_unix_ns"]) < 58)
C["bekleme_3558"] = dict(n=len(soj), medyan=st.median(soj), p95=soj[int(0.95 * (len(soj) - 1))], enc=max(soj), gt393=sum(1 for v in soj if v > 393))
W(f" [35, 58) s darboğaz beklemesi (çıkış anına göre, {len(soj)} paket): medyan {st.median(soj):.0f} ms, p95 {soj[int(0.95 * (len(soj) - 1))]:.0f} ms, en çok {max(soj):.0f} ms; > 393 ms {sum(1 for v in soj if v > 393)} paket; sunulan/kapasite %{w35['sunulan_yuzde']:.1f}, çıkan/kapasite %{w35['cikan_yuzde']:.1f}, kuyrukta düşen {w35['kuyrukta_dusen']}")
send = {x["i"]: x for x in alice if x["k"] == "send" and x["c"] == "T"}
rx = {x["i"]: x for x in bob if x["k"] == "rx" and x["c"] == "T" and x.get("first")}
tx = [(i, tau(int(msns(send[i]["t"]))), (rx[i]["t"] - send[i]["t"]) if i in rx else None) for i in sorted(send)]
C["metin"] = tx
def grp(lab, sel):
    g = [x for x in tx if sel(x[1])]
    late = [(x[0], round(x[1], 1), round(x[2])) for x in g if x[2] is not None and x[2] > 1000]
    W(f" metin {lab}: {len(g)} gönderim, teslimsiz {sum(1 for x in g if x[2] is None)}; > 1 s {len(late)} → (metin, gönderim τ, ms) {late}; en çok {max((x[2] for x in g if x[2] is not None), default=float('nan')):.0f} ms; hepsi ms: {' '.join('–' if x[2] is None else f'{x[2]:.0f}' for x in g)}")
    return dict(n=len(g), teslimsiz=sum(1 for x in g if x[2] is None), gec=late, enc=max((x[2] for x in g if x[2] is not None), default=None))
C["metin_grp"] = dict(kor=grp("kör pencere [20, 24)", lambda t: 20 <= t < 24), dar=grp("[24, 60)", lambda t: 24 <= t < 60), dar_tum=grp("[20, 60) (tümü)", lambda t: 20 <= t < 60),
                      once=grp("[0, 20)", lambda t: t < 20), sonra60=grp("[60, 85)", lambda t: 60 <= t < 85), son=grp("[85, bitiş)", lambda t: t >= 85))
d0, d1 = step[85.0], step[85.5]
arr = [(x["t_unix_ns"], x["ip_len"]) for x in ma_in.values() if d0 <= x["t_unix_ns"] < d1]
ext = [(x["t_unix_ns"], x["ip_len"]) for p, x in ifb.items() if d0 <= x["t_unix_ns"] < d1]
qn0, qb0 = q_at(d0)
C["dip"] = dict(sunulan_b=sum(l for _, l in arr), sunulan_p=len(arr), cikan_b=sum(l for _, l in ext), cikan_p=len(ext), bas_kuyruk_b=qb0, bas_kuyruk_p=qn0)
po, no_ = prev_out(d0), next_out(d0)
W(f" dip [85,0, 85,5) s (basamak komut başlangıçları τ={tau(d0):.6f} → {tau(d1):.6f}): darboğaza sunulan {C['dip']['sunulan_b']} B / {len(arr)} p; dip başında kuyruk {qb0} B / {qn0} p; dipte darboğazdan çıkan {C['dip']['cikan_b']} B / {len(ext)} p; Alice veri akışında dipten önceki son / sonraki ilk datagram τ={f0(tau(po) if po else None, '{:.3f}')} / {f0(tau(no_) if no_ else None, '{:.3f}')}")
last = acks[-1]["cum"] if acks else {}
C["alarm"] = dict(ack=len(acks), alarm=sum(1 for x in acks if x["alarm"]), rise=sum(1 for x in acks if x["rise"]), dusus_alarm=sum(1 for x in acks if x["alarm"] and not x["rise"]),
                  detects_satir=sum(x["detects"] for x in acks), cum=last, alice_lg=len(lgs), alice_ack_rx=len(arx), tipler=dict(asum["wire"]["ack_rx"]))
W(f" Bob ACK {len(acks)} (Alice ack_rx {len(arx)}, lg {len(lgs)}); alarm ACK'i {C['alarm']['alarm']} (yükseliş alarmı {C['alarm']['rise']}, daralma alarmı {C['alarm']['dusus_alarm']}); daralma algısı: satırlardaki detects toplamı {C['alarm']['detects_satir']}, son ACK satırı cum {last}; Alice'e varan tel türleri {asum['wire']['ack_rx']}")
rw = [(r["sira"], r["donus_bekleme_ms"]) for r in rows if r["donus_bekleme_ms"] is not None]
W(" ACK başına dönüş sıra beklemesi ms (sıra:ms): " + " ".join(f"{s}:{w_:.0f}" for s, w_ in rw))
C["donus_bekleme"] = rw
J["c"] = C
json.dump(J, open(outp + ".json", "w"), ensure_ascii=False, indent=1, default=str)
open(outp + ".txt", "w").write("\n".join(O) + "\n")
print("\n".join(O))
