#!/usr/bin/env python3
"""[SENARYO v1] Kancalı koşu: ACK başına tek satır (ürün kodu değil; yorum/hüküm üretmez).

kullanım: ack_tablo.py <koşu dizini> <çıktı öneki>
    <koşu dizini> = f0_cell.sh çıktısında <kök>/<CELL>/<kol>/rep<N>  (ör. …/v1_taban_t1/aday/rep1)
    çıktı: <önek>.json (satırlar) · <önek>.md (tablo) · <önek>-denetim.txt (eşleme + saat çapraz denetimi)
Girdi (koşu dizininde): sync/t0 · alice.jsonl · bob.jsonl · basamak.jsonl · cap/{B_vb0, MID_mb0, MID_ma0,
    A_va0, MID_ifb0}.jsonl — yani iki aşamalı ileri hat (FWD="v1 <hız>"), CAPTURE=1 ve paket kimliği
    ("pid") yazan pkt_capture.py.
Önkoşullar (sağlanmazsa betik hata verir; sessizce başka şey ölçmez):
    * ikili ACK gözlem kancalarını yazmalı: bob.jsonl `ack`, alice.jsonl `ack_rx` ve `lg` kayıtları
      (PR #254; kancasız f0_flows bu kayıtları üretmez);
    * hız planı 20 s, 60 s ve 85,0 s basamaklarını içermeli (v1_taban / v1_birlesik planı). 85,0 s
      basamağı olmayan planda (v1_kesmesiz) motor çıkışı τ ≥ 80 s olan ACK varsa betik durur.
Zaman ekseni: τ = Unix zamanı − sync/t0. Başvuru anı = hız basamağı komutunun BAŞLANGICI (basamak.jsonl
komut_once_unix_ns). Eşleme: bob ack.(largest_pn, sıra) ↔ alice ack_rx.(largest_pn, sıra); ack_rx.at_ns = lg.at_ns.
Yakalama eşlemesi: B_vb0 çıkışında ACK akışında (kaynak kapı = bob kapısı) IP boyu = ack.bytes + 50 olan ve
t_wire'a ±5 ms içinde TEK aday varsa o paket; paket kimliği (UDP gövdesi özeti) ile mb0 girişi / ma0 çıkışı /
va0 girişi izlenir. Aday tek değilse hücre boş kalır."""
import json, sys, statistics as st
if len(sys.argv) != 3:
    sys.exit("kullanım: ack_tablo.py <koşu dizini> <çıktı öneki>")
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
alice, bob = jl(f"{d}/alice.jsonl"), jl(f"{d}/bob.jsonl")
P = int([x for x in alice if x["k"] == "hdr"][0]["bob_addr"].split(":")[1])
ns = lambda ms: None if ms is None else ms * 1e6          # Unix ms → Unix ns
bas = [x for x in jl(f"{d}/basamak.jsonl") if x["k"] == "basamak"]
step = {x["ofset_s"]: x for x in bas}
cap = {n: [x for x in jl(f"{d}/cap/{n}.jsonl") if "ip_len" in x and x.get("proto") == 17] for n in ("B_vb0", "MID_mb0", "MID_ma0", "A_va0", "MID_ifb0")}
b_out = [x for x in cap["B_vb0"] if x["dir"] == "out" and x.get("sport") == P]
by = lambda n, dr: {x["pid"]: x for x in cap[n] if x["dir"] == dr and x.get("sport") == P}
mb_in, ma_out, a_in = by("MID_mb0", "in"), by("MID_ma0", "out"), by("A_va0", "in")
acks = [x for x in bob if x["k"] == "ack"]
arx = [x for x in alice if x["k"] == "ack_rx"]
lg = {x["at_ns"]: x for x in alice if x["k"] == "lg"}
# eşleme: largest_pn + sıra (aynı largest_pn birden çok kez geçerse sıra ile)
from collections import defaultdict
bq, aq = defaultdict(list), defaultdict(list)
for x in acks: bq[x["largest_pn"]].append(x)
for x in arx: aq[x["largest_pn"]].append(x)
pair = {}
for pn, lst in bq.items():
    for i, x in enumerate(lst):
        pair[x["index"]] = aq[pn][i] if i < len(aq.get(pn, [])) else None
dup_pn = {pn: len(v) for pn, v in bq.items() if len(v) > 1}
def ref_of(t_tau):
    return 20.0 if t_tau < 58 else (60.0 if t_tau < 80 else 85.0)
rows = []
for x in acks:
    t_tau = (ns(x["t"]) - t0) / 1e9
    ref = ref_of(t_tau)
    if ref not in step:
        sys.exit(f"ack_tablo.py: basamak.jsonl içinde {ref:g} s basamağı yok (ACK sıra {x['index']}, τ={t_tau:.3f} s)")
    r0 = step[ref]["komut_once_unix_ns"]
    rel = lambda v: None if v is None else (v - r0) / 1e6        # ns → başvuruya göre ms
    c = [p for p in b_out if p["ip_len"] == x["bytes"] + 50 and x["t_wire"] is not None and abs(p["t_unix_ns"] - ns(x["t_wire"])) <= 5e6]
    pk = c[0] if len(c) == 1 else None
    pid = pk["pid"] if pk else None
    a = pair.get(x["index"]); l = lg.get(a["at_ns"]) if a else None
    da = x["defer_alarm"]; df = x["defer"]
    rows.append(dict(
        sira=x["index"], largest_pn=x["largest_pn"], tur=x["type"], cerceve_B=x["bytes"], ip_B=(pk["ip_len"] if pk else None),
        basvuru_s=ref, basvuru_komut_once_tau=(r0 - t0) / 1e9, basvuru_komut_sonra_ms=rel(step[ref]["komut_sonra_unix_ns"]),
        alarm=x["alarm"], rise=x["rise"], wait_open=x["wait_open"], detects=x["detects"], timer=x["timer"], wire=x["wire"],
        t_detect=rel(ns(x["t_detect"])), t_evidence=rel(ns(x["t_evidence"])), t_alarm=rel(ns(x["t_alarm"])),
        t_first_try=rel(ns(x["t_first_try"])), t_first_defer=rel(ns(x["t_first_defer"])), t_ready=rel(ns(x["t_ready"])),
        defer=df, defer_alarm=da, t=rel(ns(x["t"])), t_wire=rel(ns(x["t_wire"])),
        yak_bob_cikis=rel(pk["t_unix_ns"]) if pk else None,
        yak_mb0_giris=rel(mb_in[pid]["t_unix_ns"]) if pid in mb_in else None,
        yak_donus_hatti_cikis=rel(ma_out[pid]["t_unix_ns"]) if pid in ma_out else None,
        yak_alice_giris=rel(a_in[pid]["t_unix_ns"]) if pid in a_in else None,
        yak_aday_sayisi=len(c), paket_kimligi=pid,
        ack_rx_sira=(a["index"] if a else None), ack_rx_t=rel(ns(a["t"])) if a else None, ack_delay_us=(a["ack_delay_us"] if a else None),
        lg_karar=(l["decision"] if l else None), lg_measured=(l["measured"] if l else None),
        pace_before=(l["pace_before"] if l else None), pace_after=(l["pace_after"] if l else None),
        rate_before=(l["rate_before"] if l else None), rate_after=(l["rate_after"] if l else None),
        raised=(l["raised"] if l else None), probe_failed=(l["probe_failed"] if l else None),
        tau_t=t_tau, tau_ack_rx=((ns(a["t"]) - t0) / 1e9 if a else None)))
json.dump(dict(t0_unix_ns=t0, bob_kapi=P, ayni_largest_pn=dup_pn, satirlar=rows,
               basamaklar=[dict(ofset_s=x["ofset_s"], hiz=x["hiz"], komut_once_tau=(x["komut_once_unix_ns"] - t0) / 1e9, komut_sonra_tau=(x["komut_sonra_unix_ns"] - t0) / 1e9, rc=x["rc"]) for x in bas]),
          open(outp + ".json", "w"), ensure_ascii=False, indent=1)
f = lambda v: "" if v is None else f"{v:+.0f}"
b = lambda v: "" if v is None else ("E" if v else "H")
def defer_s(r):
    df, da = r["defer"], r["defer_alarm"]
    s = "/".join(str(df[k]) for k in ("budget", "yield", "no_info", "first_window"))
    return s + (f" · alarm {da['budget']}/{da['yield']}" if r["alarm"] else "")
L = []
L.append("ACK başına tek satır — Bob'un alıcı motorundan çıkan her ACK (bob.jsonl `ack`), Alice'teki karşılığı (`ack_rx`, `lg`) ve yakalamadaki izi.")
L.append("Anlar ms, satırın başvurduğu hız basamağı komutunun BAŞLANGICINA göre (baş. s sütunu; motor çıkışı τ < 58 s → 20 s, < 80 s → 60 s, öbürleri → 85,0 s basamağı). Boş hücre = kayıtta alan yok / eşleşme yok.")
L.append("Basamak komutları (τ s, başlangıç → dönüş): " + "; ".join(f"{x['ofset_s']:g} s {x['hiz']}: {(x['komut_once_unix_ns'] - t0) / 1e9:.6f} → {(x['komut_sonra_unix_ns'] - t0) / 1e9:.6f}" for x in bas))
L.append("Sütunlar: t_detect…t_wire = bob `ack` alanları; erteleme = `defer` budget/yield/no_info/first_window, alarm ACK'inde ayrıca `defer_alarm` budget/yield; Bob arayüz çıkışı = B_vb0 `out`; dönüş hattı çıkışı = MID_ma0 `out` (dönüş netem'i sonrası); Alice arayüz girişi = A_va0 `in`; ack_rx.t = alice `ack_rx.t` (= `lg` satırı anı); pace = `lg.pace_before`→`lg.pace_after` (B/s).")
L.append("")
L.append("| baş. s | sıra | pn | IP B | alarm/rise/wait_open | t_detect | t_evidence | t_alarm | t_first_try | t_ready | erteleme (bütçe/yol/bilgisiz/ilk-pencere · alarm sonrası bütçe/yol) | t (motor) | t_wire | Bob arayüz çıkışı | dönüş hattı çıkışı (ma0) | Alice arayüz girişi | ack_rx.t | lg kararı | pace önce→sonra |")
L.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for r in rows:
    pace = "" if r["pace_before"] is None else f"{r['pace_before']}→{r['pace_after']}"
    ext = ""
    if r["raised"]: ext += " (raised)"
    if r["probe_failed"]: ext += " (probe_failed)"
    L.append(f"| {r['basvuru_s']:g} | {r['sira']} | {r['largest_pn']} | {'' if r['ip_B'] is None else r['ip_B']} | {b(r['alarm'])}/{b(r['rise'])}/{b(r['wait_open'])} | {f(r['t_detect'])} | {f(r['t_evidence'])} | {f(r['t_alarm'])} | {f(r['t_first_try'])} | {f(r['t_ready'])} | {defer_s(r)} | {f(r['t'])} | {f(r['t_wire'])} | {f(r['yak_bob_cikis'])} | {f(r['yak_donus_hatti_cikis'])} | {f(r['yak_alice_giris'])} | {f(r['ack_rx_t'])} | {r['lg_karar'] or ''}{ext} | {pace} |")
open(outp + ".md", "w").write("\n".join(L) + "\n")
# saat tabanı çapraz denetimi (düğüm saati ↔ yakalama saati)
dw = [r["yak_bob_cikis"] - r["t_wire"] for r in rows if r["yak_bob_cikis"] is not None]
da_ = [r["ack_rx_t"] - r["yak_alice_giris"] for r in rows if r["ack_rx_t"] is not None and r["yak_alice_giris"] is not None]
O = []
O.append(f"ACK satırı (bob) {len(acks)}; ack_rx (alice) {len(arx)}; lg {len(lg)}; eşlenen {sum(1 for r in rows if r['ack_rx_sira'] is not None)}; alice satırı olmayan bob ACK sıraları {[r['sira'] for r in rows if r['ack_rx_sira'] is None]}; aynı largest_pn tekrarları {dup_pn}")
O.append(f"yakalama eşlemesi: tek aday {sum(1 for r in rows if r['yak_aday_sayisi']==1)}/{len(rows)}; aday sayıları {[r['yak_aday_sayisi'] for r in rows]}")
O.append(f"saat çapraz denetimi: (Bob arayüz yakalaması − t_wire) ms en az/medyan/en çok = {min(dw):.3f}/{st.median(dw):.3f}/{max(dw):.3f}; (ack_rx.t − Alice arayüz yakalaması) ms en az/medyan/en çok = {min(da_):.3f}/{st.median(da_):.3f}/{max(da_):.3f}")
open(outp + "-denetim.txt", "w").write("\n".join(O) + "\n")
print("\n".join(O))
