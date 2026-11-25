#!/usr/bin/env python3
"""#258 yorumu için (a)–(d) tabloları (ürün kodu değil; neden ataması yok).
kullanım: yorum_tablo.py <çıktı.md> <etiket>=<k258-ek.json>,<analiz-veri.json>,<ack-tablo.json> ...   (ilk koşu = önce)"""
import json, sys
outp = sys.argv[1]
runs = []
for a in sys.argv[2:]:
    lab, rest = a.split("=", 1); ek, av, at = rest.split(",")
    E_ = json.load(open(ek)); E_["ack_tablo"] = json.load(open(at))["satirlar"]
    runs.append((lab, E_, json.load(open(av))))
tr = lambda v, nd=0: "–" if v is None else (f"{v:,.{nd}f}".replace(",", " ").replace(".", ","))
L = []; W = L.append
# ── (a)
W("**(a) Hüküm ACK'leri.** Süreler ms. Dönüş sıra beklemesi = (Alice arayüz girişi − Bob arayüz çıkışı) − IP boyu / 62 B/s − 300 ms (yakalamadan). Önce kaydında `verdict` alanı yok; karşılığı `raised` / `probe_failed` satırları ve `g_ns`.")
W("")
def eski(e):   # yalnız cephe ölçüsüyle (911f2c6d öncesi kural) sonuç; kayıttaki girdilerden hesap
    if e["outcome"].startswith("(alan yok)"): return "–"
    if e["outcome"] != "Inconclusive" or e["interval_g_ms"] is None: return e["outcome"]
    h = e["half_ms"]
    return "Passed" if (e["abs_q_ms"] <= h and e["growth_ms"] <= h) else ("Failed" if (e["abs_q_ms"] > h and e["growth_ms"] > h) else "Inconclusive")
W("| koşu | ACK sıra | an τ s | outcome | yalnız cephe ölçüsüyle sonuç (hesap) | expect/2 | abs_q | growth | interval_g | dönüş sıra beklemesi | alarm ACK'i | pace önce→sonra | yazılan kuyruk + ek B |")
W("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for lab, E, A in runs:
    if not E["hukum"]:
        W(f"| {lab} | – | – | hüküm satırı yok | – | – | – | – | – | – | – | – | – |")
    for e in E["hukum"]:
        W(f"| {lab} | {e['sira']} | {tr(e['tau'], 3)} | {e['outcome']} | {eski(e)} | {tr(e['half_ms'], 2)} | {tr(e['abs_q_ms'], 2)} | {tr(e['growth_ms'], 2)} | {tr(e['interval_g_ms'], 3)} | {tr(e['donus_ms'], 1)} | {'E' if e['alarm'] else 'H'} | {e['pace'][0]}→{e['pace'][1]} | {'yazılmadı' if e['yazilan'] is None else e['yazilan']} |")
W("")
for lab, E, A in runs:
    hv = [e for e in E["hukum"] if not e["outcome"].startswith("(alan yok)")]
    if hv:
        from collections import Counter
        W(f"- {lab}: hüküm satırı {len(hv)}; outcome dağılımı " + ", ".join(f"{k} {v}" for k, v in Counter(e['outcome'] for e in hv).items()) + f"; `Failed` iken `interval_g_ns ≤ expect_ns/2` olan satır **{sum(1 for e in hv if e['b1'])}**; dönüş sıra beklemesi ≥ 100 ms olan hüküm satırı {sum(1 for e in hv if e['donus_ms'] is not None and e['donus_ms'] >= 100)}; yalnız cephe ölçüsüyle hesaplanan sonucu kayıttaki sonuçtan farklı satır {sum(1 for e in hv if eski(e) != e['outcome'])}.")
    elif E["hukum"]:
        pf = [e for e in E["hukum"] if e["probe_failed"]]
        W(f"- {lab}: `probe_failed` satırı {len(pf)}" + "".join(f" (sıra {e['sira']}: `g_ns` {tr(e['interval_g_ms'] * 1e6)} ns, dönüş sıra beklemesi {tr(e['donus_ms'], 1)} ms)" for e in pf) + f"; `raised` satırı {sum(1 for e in E['hukum'] if e['raised'])}.")
    else:
        W(f"- {lab}: hüküm satırı 0 (`verdict` her `lg` satırında `null`; `raised` / `probe_failed` satırı " + str(sum(1 for e in E['lg'] if e['raised'] or e['probe_failed'])) + ").")
W("")
has = lambda E: any(e["has_verdict_field"] for e in E["lg"])
def sinif(E, e):   # verdict alanı olan kayıtta verdict'e göre; olmayan kayıtta probe_failed / raised'e göre
    if has(E): return "yoklama hükmü" if e["hukum"] else "yoklamasız kuyruk tahmini"
    return "yoklama hükmü (probe_failed)" if e["probe_failed"] else ("yoklama hükmü (raised)" if e["raised"] else "yoklamasız kuyruk tahmini")
# ── (b)
W("**(b) Sahte tutma ayrımı.** yazılan = `backlog_bytes + extra_backlog_bytes` (boş = `null`, hat modeline kuyruk yazılmadı); gerçek = aynı anda darboğazda bekleyen bayt (yakalamadan: `MID_ma0 in` ≤ an < `MID_ifb0`; örnekleyicinin (`kuyruk.jsonl`) önceki / sonraki örneği yanında); sahte = yazılan > gerçek + 1 228 B (1 datagram). Yalnız kuyruk yazılan, dönüş sıra beklemesi ≥ 100 ms olan ya da lg anından sonra ≥ 0,5 s gönderim boşluğu izleyen satırlar; tam tablo ham dizinde `k258-ek.txt`.")
W("")
W("| koşu | ACK sıra | an τ s | hüküm | front_wait ms | by_bytes / by_delay B | yazılan B | gerçek B / p | örnekleyici önce / sonra B | dönüş sıra beklemesi ms | sonraki veri datagramına dek s | sahte (kayıt) | sahte (türetilmiş) | sınıf |")
W("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for lab, E, A in runs:
    for e in E["lg"]:
        if e["yazilan"] is None and (e["donus_ms"] is None or e["donus_ms"] < 100) and e["sonraki_veri_s"] < 0.5: continue
        sm = lambda s: "–" if s is None else str(s[0])
        yn = lambda v: "–" if v is None else ("EVET" if v else "hayır")
        W(f"| {lab} | {e['sira']} | {tr(e['tau'], 3)} | {(e['hukum'] or '–') + (' · probe_failed' if e['probe_failed'] else '') + (' · raised' if e['raised'] else '')} | {tr(e['front_wait_ms'], 1)} | {tr(e['by_bytes'])} / {tr(e['by_delay'])} | {'yazılmadı' if e['yazilan'] is None else str(e['yazilan']) + ' (' + str(e['backlog']) + ' + ' + str(e['ek']) + ')'} | {e['gercek_b']} / {e['gercek_p']} | {sm(e['ornek_once'])} / {sm(e['ornek_sonra'])} | {tr(e['donus_ms'], 1)} | {tr(e['sonraki_veri_s'], 3)} | {yn(e['sahte_kayit'])} | {yn(e['sahte_turetilmis'])} | {sinif(E, e) if (e['sahte_kayit'] or e['sahte_turetilmis']) else '–'} |")
W("")
W("| sayım | " + " | ".join(lab for lab, _, _ in runs) + " |")
W("|---|" + "---|" * len(runs))
def cell(f): return " | ".join(f(E, A) for _, E, A in runs)
W("| `lg` satırı / kuyruk yazılan satır | " + cell(lambda E, A: f"{len(E['lg'])} / " + (str(sum(1 for e in E['lg'] if e['yazilan'] is not None)) if has(E) else "alan yok")) + " |")
W("| sahte tutma, kayıttan — yoklama hükmünden | " + cell(lambda E, A: (str(sum(1 for e in E['lg'] if e['sahte_kayit'] and e['hukum'])) if has(E) else "alan yok")) + " |")
W("| sahte tutma, kayıttan — yoklamasız kuyruk tahmininden | " + cell(lambda E, A: ((lambda s: f"{len(s)}" + (" (sıra " + ", ".join(str(e['sira']) for e in s) + "; yazılan − gerçek " + " + ".join(str(e['yazilan'] - e['gercek_b']) for e in s) + " B = " + " + ".join(tr((e['yazilan'] - e['gercek_b']) / e['rate'], 3) for e in s) + " s)" if s else ""))([e for e in E['lg'] if e['sahte_kayit'] and not e['hukum']]) if has(E) else "alan yok")) + " |")
def der(E, A, withv):
    s = [e for e in E["lg"] if e["sahte_turetilmis"] and sinif(E, e).startswith("yoklama hükmü") == withv]
    return f"{len(s)}" + (" (sıra " + ", ".join(f"{e['sira']}: {tr(e['sonraki_veri_s'], 3)} s" for e in s) + ")" if s else "")
W("| sahte tutma, türetilmiş (boşluk ≥ 0,5 s) — yoklama hükmü satırında (önce: `probe_failed` / `raised`) | " + cell(lambda E, A: der(E, A, True)) + " |")
W("| sahte tutma, türetilmiş — yoklamasız satırlarda | " + cell(lambda E, A: der(E, A, False)) + " |")
W("| gönderici susması ≥ 0,5 s (τ ≥ 2 s): adet / toplam s | " + cell(lambda E, A: f"{len(E['susma'])} / {tr(sum(g['sure'] for g in E['susma']), 3)}") + " |")
W("| … darboğaz kuyruğu boşken (boş hat süresi ≥ 0,5 s): adet / süre toplamı s / boş hat süresi toplamı s | " + cell(lambda E, A: (lambda g: f"{len(g)} / {tr(sum(x['sure'] for x in g), 3)} / {tr(sum(x['bos'] for x in g), 3)}")([g for g in E['susma'] if g['bos'] >= 0.5])) + " |")
W("| susmalar (başlangıç → bitiş τ s; başta kuyruk B; boş hat s) | " + cell(lambda E, A: "; ".join(f"{tr(g['a'], 2)} → {tr(g['b'], 2)}; {g['q_b']}; {tr(g['bos'], 3)}" for g in E['susma']) or "yok") + " |")
W("")
# ── (c)
W("**(c) Önce ↔ sonra.**")
W("")
W("| ölçü | " + " | ".join(lab for lab, _, _ in runs) + " |")
W("|---|" + "---|" * len(runs))
C = lambda E: E["c"]
W("| düşüş (20 s basamağı komut başlangıcı) → ilk tavan kesmesi ms | " + cell(lambda E, A: tr(C(E)['dusus_kesme_ms'], 1)) + " |")
W("| 20 s'den sonraki 1. / 2. / 3. ACK'te tavan (`pace_after`, B/s) | " + cell(lambda E, A: " / ".join(str(x[3]) for x in C(E)['ack_20'])) + " |")
W("| … o ACK'lerin varış anı τ s | " + cell(lambda E, A: " / ".join(tr(x[1], 2) for x in C(E)['ack_20'])) + " |")
W("| 20 s'den önce en düşük tavan B/s | " + cell(lambda E, A: str(C(E)['pre20_min'])) + " |")
W("| τ = 58 s'de tavan: sonda hat-eşdeğeri / son `lg` `pace_after` | " + cell(lambda E, A: f"{tr(A['hz']['pre58'])} / {C(E)['pace_58']}") + " |")
W("| 60 s'den sonraki 1. / 2. / 3. ACK'te tavan B/s | " + cell(lambda E, A: " / ".join(str(x[3]) for x in C(E)['ack_60'])) + " |")
W("| … o ACK'lerin varış anı τ s | " + cell(lambda E, A: " / ".join(tr(x[1], 2) for x in C(E)['ack_60'])) + " |")
W("| 60 s'den koşu sonuna en büyük tavan: `lg` / sonda hat-eşdeğeri B/s | " + cell(lambda E, A: f"{C(E)['pace_max_60']} / {tr(A['hz']['tavan_max_after60'])}") + " |")
def pmin(E, lo, hi):
    v = [(r["pace_after"], r["tau_ack_rx"], r["sira"]) for r in E["ack_tablo"] if r["tau_ack_rx"] is not None and lo <= r["tau_ack_rx"] < hi]
    return "ACK yok" if not v else (lambda m: f"{m[0]} (ACK {m[2]}, τ={tr(m[1], 2)})")(min(v))
W("| [60, 85) s ACK'lerinde en düşük tavan (`pace_after`) | " + cell(lambda E, A: pmin(E, 60, 85)) + " |")
W("| 85 s'den koşu sonuna ACK'lerde en düşük tavan (`pace_after`) | " + cell(lambda E, A: pmin(E, 85, 1e9)) + " |")
W("| tavanın ≥ 11 875 olduğu ilk an τ s; 84 s'ye dek kalıcı | " + cell(lambda E, A: ("olmadı" if A['hz']['t_up'] is None else f"{tr(A['hz']['t_up'], 2)}; {'evet' if A['hz']['kalici'] else 'hayır'}")) + " |")
W("| [60, 85) s darboğaza sunulan: B/s; sunulan / kapasite | " + cell(lambda E, A: f"{tr(C(E)['w6085']['sunulan'] / 25)}; %{tr(C(E)['w6085']['sunulan_yuzde'], 1)}") + " |")
W("| [60, 85) s darboğazdan çıkan / kapasite; kuyrukta düşen | " + cell(lambda E, A: f"%{tr(C(E)['w6085']['cikan_yuzde'], 1)}; {C(E)['w6085']['kuyrukta_dusen']}") + " |")
W("| [35, 58) s sunulan / kapasite; çıkan / kapasite; kuyrukta düşen | " + cell(lambda E, A: f"%{tr(C(E)['w3558']['sunulan_yuzde'], 1)}; %{tr(C(E)['w3558']['cikan_yuzde'], 1)}; {C(E)['w3558']['kuyrukta_dusen']}") + " |")
W("| [35, 58) s darboğaz beklemesi ms: medyan / p95 / en çok (paket) | " + cell(lambda E, A: (lambda b: f"{tr(b['medyan'])} / {tr(b['p95'])} / {tr(b['enc'])} ({b['n']})")(C(E)['bekleme_3558'])) + " |")
W("| [35, 58) s > 393 ms bekleyen paket | " + cell(lambda E, A: str(C(E)['bekleme_3558']['gt393'])) + " |")
mg = lambda E, k: C(E)["metin_grp"][k]
late = lambda g: f"{len(g['gec'])}/{g['n']}" + (" (" + ", ".join(f"τ={tr(x[1])}: {x[2]}" for x in g['gec']) + ")" if g['gec'] else "")
W("| metin, kör pencere [20, 24) s: > 1 s olan / gönderilen (gönderim τ: ms) | " + cell(lambda E, A: late(mg(E, 'kor'))) + " |")
W("| metin [24, 60) s: > 1 s olan / gönderilen | " + cell(lambda E, A: late(mg(E, 'dar'))) + " |")
W("| metin [20, 60) s: ≤ 1 s olan / 20; en çok ms | " + cell(lambda E, A: f"{mg(E, 'dar_tum')['n'] - len(mg(E, 'dar_tum')['gec'])}/{mg(E, 'dar_tum')['n']}; {tr(mg(E, 'dar_tum')['enc'])}") + " |")
W("| metin [0, 20) s: > 1 s olan / gönderilen | " + cell(lambda E, A: late(mg(E, 'once'))) + " |")
W("| metin [60, 85) s: > 1 s olan / gönderilen | " + cell(lambda E, A: late(mg(E, 'sonra60'))) + " |")
W("| metin [85, bitiş) s: > 1 s olan / gönderilen | " + cell(lambda E, A: late(mg(E, 'son'))) + " |")
W("| metin teslim edilen / gönderilen (koşu) | " + cell(lambda E, A: f"{A['texts_del']}/{A['texts_sent']}") + " |")
W("| dip [85,0, 85,5) s: darboğaza sunulan B / p; dip başında kuyruk B; dipte çıkan B | " + cell(lambda E, A: (lambda d: f"{d['sunulan_b']} / {d['sunulan_p']}; {d['bas_kuyruk_b']}; {d['cikan_b']}" + (" → SINANMADI" if d['sunulan_b'] == 0 and d['bas_kuyruk_b'] == 0 else ""))(C(E)['dip'])) + " |")
W("| [86,5, 98) s sunulan / kapasite; bekleme medyan / en çok ms | " + cell(lambda E, A: (lambda w: f"%{tr(w['sunulan_yuzde'], 1)}; {tr(w['bekleme_medyan'])} / {tr(w['bekleme_enc'])}")(A['hs']['w_short'])) + " |")
W("| Bob ACK'i / alarm ACK'i (`alarm:true`) / yükseliş (`rise:true`) | " + cell(lambda E, A: f"{C(E)['alarm']['ack']} / {C(E)['alarm']['alarm']} / {C(E)['alarm']['rise']}") + " |")
W("| daralma algısı (`cum.drop_detects`, son ACK satırı); `cum.drop_alarms` / `rise_alarms` | " + cell(lambda E, A: f"{C(E)['alarm']['cum'].get('drop_detects')}; {C(E)['alarm']['cum'].get('drop_alarms')} / {C(E)['alarm']['cum'].get('rise_alarms')}") + " |")
W("| dönüş sıra beklemesi ≥ 100 ms olan ACK'ler (sıra: ms) | " + cell(lambda E, A: ", ".join(f"{s}: {tr(w)}" for s, w in C(E)['donus_bekleme'] if w >= 100) or "yok") + " |")
W("| darboğazdan çıkan paket; hatta düşen (%); kuyrukta düşen | " + cell(lambda E, A: f"{A['ifb']}; {A['hd']} (%{tr(A['loss_pct'], 2)}); {A['kd']}") + " |")
W("| dosya, Bob motor kabulü koşu sonunda B | " + cell(lambda E, A: tr(A['file']['end'])) + " |")
W("")
# ── (d)
W("**(d) Beş hedefin bu koşudaki karşılığı** (PTT'siz teşhis; birleşik kabul değil; hüküm Web'in).")
W("")
W("| hedef | " + " | ".join(lab for lab, _, _ in runs) + " |")
W("|---|" + "---|" * len(runs))
def h1(E, A):
    a = C(E)["ack_20"]
    if len(a) < 2: return "SINANMADI — 20 s'den sonra 2. ACK yok"
    ok = 5625 <= a[1][3] <= 6875
    return f"{'GEÇTİ' if ok else 'KALDI'} — 2. ACK'te (τ={tr(a[1][1], 2)}) tavan {a[1][3]}; 20 s'den önce en düşük {C(E)['pre20_min']}"
def h3(E, A):
    w, b, m, dp = C(E)["w3558"], C(E)["bekleme_3558"], mg(E, "dar_tum"), C(E)["dip"]
    s = [("tel", w["sunulan_yuzde"] >= 90, f"[35, 58) sunulan / kapasite %{tr(w['sunulan_yuzde'], 1)}, çıkan / kapasite %{tr(w['cikan_yuzde'], 1)}" + (" (> %100; işaretlenmedi)" if w["sunulan_yuzde"] > 100 else "")),
         ("kayıp", w["kuyrukta_dusen"] == 0, f"kuyrukta düşen {w['kuyrukta_dusen']}"),
         ("bekleme", b["enc"] <= 393, f"bekleme en çok {tr(b['enc'])} ms, > 393 ms {b['gt393']}/{b['n']} paket"),
         ("metin", len(m["gec"]) == 0 and m["teslimsiz"] == 0, f"[20, 60) metin ≤ 1 s {m['n'] - len(m['gec'])}/{m['n']} (kör pencere [20, 24): {mg(E, 'kor')['n'] - len(mg(E, 'kor')['gec'])}/{mg(E, 'kor')['n']})")]
    dip = ("dip SINANMADI (dipte sunulan 0 B)" if dp["sunulan_b"] == 0 and dp["bas_kuyruk_b"] == 0 else
           f"dip: sunulan {dp['sunulan_b']} B; 85 s'den sonra boş hatta susma {sum(1 for g in E['susma'] if g['a'] >= 84.5 and g['bos'] >= 0.5)}; kesmesiz karşılaştırma kolu yok")
    bad = [x for x in s if not x[1]]
    return f"{'KALDI' if bad else 'GEÇTİ'} — " + "; ".join(("✗ " if not x[1] else "") + x[2] for x in s) + "; " + dip
def h4(E, A):
    pre = A["hz"]["pre58"]; a = C(E)["ack_60"]
    if pre is None or not (5625 <= pre <= 6875): return f"SINANMADI — önkoşul τ=58 tavan {tr(pre)}"
    if len(a) < 3: return "SINANMADI — 60 s'den sonra 3. ACK yok"
    ok = a[2][3] >= 11875 and bool(A["hz"]["kalici"])
    return f"{'GEÇTİ' if ok else 'KALDI'} — önkoşul τ=58 tavan {tr(pre)}; 3. ACK'te (τ={tr(a[2][1], 2)}) tavan {a[2][3]}; ≥ 11 875 ilk an " + ("olmadı" if A['hz']['t_up'] is None else f"τ={tr(A['hz']['t_up'], 2)}, 84 s'ye dek kalıcı: {'evet' if A['hz']['kalici'] else 'hayır'}") + f"; en büyük tavan {C(E)['pace_max_60']}"
W("| 1 CC (2. ACK'e dek tavan 5 625–6 875) | " + cell(h1) + " |")
W("| 2 PTT | " + cell(lambda E, A: "SINANMADI — koşuda PTT yok") + " |")
W("| 3 Hız sınırlama | " + cell(h3) + " |")
W("| 4 Hızlanma (3. ACK'e dek tavan ≥ 11 875 ve kalıcı) | " + cell(h4) + " |")
W("| 5 Ürün/CI testleri | " + cell(lambda E, A: "SINANMADI — bu adım test koşmadı, derleme yapmadı") + " |")
open(outp, "w").write("\n".join(L) + "\n")
print("\n".join(L))
