#!/usr/bin/env python3
"""[KTP2-F0 · Ajan C] TAM analiz — `analyze_c.py` koşu ölçülerinden hücre tabloları, farklar,
gecikme bileşenleri, kilit ayrıştırması ve B-C1/C2/C4 sayıları.

Kullanım:
  python3 -I scripts/ktp2/f0/c/tam_c.py --csv reports/ktp2/f0/c/tam KÖK \
      > reports/ktp2/f0/c/tam/tablolar.md

Çıktı: kosular.csv (koşu başına), hucreler.csv (hücre × kol dağılımı), farklar.csv (bootstrap),
ayristirma.csv, stdout: T.1–T.16 (Markdown). Deterministik (tohum = hücre|kol|ölçü FNV-1a).

Kurallar:
- Birim = koşu. Dörtte birlikler analyze_c.qtl (doğrusal ara değer).
- Fark: medyan farkı, koşu birimli yüzdelik bootstrap (B = 4000, kollar ayrı yeniden örneklenir;
  koşular eşlenemez). Birincil aile (N = 30 hücre, ana ölçü, 4 kol × 2 ikili) için ayrıca
  Bonferroni aralığı (B = 20 000, α = 0,05 / aile büyüklüğü).
- Ana ölçü: d300 ve r1k → T1-800; d1000 → T2-800 (Tanım-1 fiziksel olarak olanaksız).
- Durum: aralık 0'ı kapsıyorsa "fark gösterilmedi"; dışlıyor ama yakın uç < 0,02 ise "sınırda";
  aksi "ayrıştı".
- Ayrıştırma ortalamayla yapılır (toplamsal olması için), medyanla değil:
    toplam = ort(rs) − ort(arq) = [R − ort(arq)] + [ort(rs) − R],  R = ort(1 − p_ileri).
  R, ARQ kolunun gönderen birikimi olmasaydı beklenen zamanında oranıdır (yalnız doğrudan
  varış; ARQ yeniden gönderimi bu son tarihlere yetişmez). Birinci terim kilit bileşeni, ikinci
  terim son tarih içinde parite katkısıdır. R koşu sonucuna göre seçilmiş bir alt kümeye
  dayanmadığı için `kilitsiz` alt küme kıyasından bağımsız bir denetimdir.
"""

import argparse
import csv
import json
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import analyze_c as A  # noqa: E402
import model_c as M  # noqa: E402

KOLLAR = ["arq", "uyar", "rs2", "rs4", "tekrar"]
IKILI = ["taban", "aday"]
METRIK_FARK = ["ot_T1_400", "ot_T1_800", "ot_T2_400", "ot_T2_800", "ot_sirali_T1_800",
               "ot_sirali_T2_800", "ana", "ana_sirali", "B_cerceve_l2"]
DAG = METRIK_FARK + ["ot_sirali_T1_400", "ot_sirali_T2_400", "teslim_orani", "lat_p50_ms",
                     "lat_p95_ms", "parite_tx", "plan_parite", "parite_gitti_pay", "fec_kurtarilan",
                     "cozulen_blok", "parite_bekleme_ust_ms", "parite_bekleme_alt_ms",
                     "parity_denied", "clamp", "arb_oneri", "btlbw", "btlbw_sunulan",
                     "hiz_payi", "birikim_tepe_ms", "q_med_ms", "q_p95_ms", "tit_p50_ms",
                     "tit_p95_ms", "gec_pay", "gec_lat_ms", "eksik_pay", "ilk_p50_ms", "b0_ms",
                     "retx_B_cerceve", "rho_konusma", "ana_per_kB", "kilit_baslangic_s", "p_ileri",
                     "c1", "c2", "c3", "c4"]


def cell_info(name):
    m = A.CELL_RE.match(name)
    return {"on": m.group(1), "hiz": m.group(2), "gec": m.group(3), "kayip": int(m.group(4)),
            "demet": int(m.group(5))}


def f(x):
    if x in (None, ""):
        return None
    try:
        v = float(x)
    except (TypeError, ValueError):
        return None
    return v if math.isfinite(v) else None


def q3(vals):
    return A.qtl(vals, 0.25), A.qtl(vals, 0.5), A.qtl(vals, 0.75)


def fm(v, nd=2):
    if v is None or (isinstance(v, float) and not math.isfinite(v)):
        return "—"
    return f"{v:.{nd}f}".replace(".", ",")


def dag(vals, nd=2):
    v = [x for x in vals if x is not None]
    if not v:
        return "—"
    a, b, c = q3(v)
    return f"{fm(b, nd)} [{fm(a, nd)}–{fm(c, nd)}] ({fm(min(v), nd)}–{fm(max(v), nd)})"


def kisa(vals, nd=2):
    v = [x for x in vals if x is not None]
    if not v:
        return "—"
    a, b, c = q3(v)
    return f"{fm(b, nd)} [{fm(a, nd)}–{fm(c, nd)}]"


def durum(lo, hi):
    if lo is None:
        return "—"
    if lo <= 0 <= hi:
        return "fark gösterilmedi"
    return "sınırda" if min(abs(lo), abs(hi)) < 0.02 else "ayrıştı"


B_FARK, B_BONF, B_AYR = 4000, 20000, 4000  # --hizli yalnız kod denetimi içindir (rapor sayısı değil)


def boot_mean(items, fn, seed, n=None):
    """items: kol başına koşu listeleri; fn(örnekler) → değer. Yüzdelik aralık."""
    rng = random.Random(seed)
    out = []
    for _ in range(n or B_AYR):
        res = [[lst[rng.randrange(len(lst))] for _ in lst] for lst in items]
        out.append(fn(res))
    return A.qtl(out, 0.025), A.qtl(out, 0.975)


def fisher(a, b, c, d):
    """İki yanlı Fisher kesin testi: [[a, b], [c, d]]."""
    n = a + b + c + d
    r1, c1 = a + b, a + c
    den = math.comb(n, c1)

    def pm(x):
        return math.comb(r1, x) * math.comb(n - r1, c1 - x) / den

    p0 = pm(a)
    return min(1.0, sum(pm(x) for x in range(max(0, c1 - (n - r1)), min(r1, c1) + 1)
                        if pm(x) <= p0 * (1 + 1e-9)))


def ayristir(row, x, ci):
    """Çerçeve düzeyi gecikme bileşenleri (`extra`dan). Birim ms."""
    lat, t_ms, ser, mu, sig = x["lat"], x["t_ms"], x["ser_d"], x["mu"], x["sig"]
    n = len(lat)
    nwin = 12
    wlen = max(1, n // nwin)
    wmins, w_idx = [], []
    for k in range(0, n - wlen + 1, wlen):
        v = [u for u in lat[k:k + wlen] if u < math.inf]
        wmins.append(min(v) if v else math.inf)
        w_idx.append(k)
    head = [u for u in wmins[:3] if u < math.inf]
    if not head:
        return {}
    b0 = min(head)
    q = []
    last = 0.0
    for i in range(n):
        w = min(i // wlen, len(wmins) - 1)
        if wmins[w] < math.inf:
            last = max(0.0, wmins[w] - b0)
        q.append(last)
    lim = mu + 4 * sig + ser
    res, gec = [], []
    for u, qq in zip(lat, q):
        if u == math.inf:
            continue
        if u - qq < lim:
            res.append(u - qq - ser - mu)
        else:
            gec.append(u - qq)
    first = [u for u in lat[:wlen] if u < math.inf]
    d = {"b0_ms": b0, "q_med_ms": A.qtl(q, 0.5), "q_p95_ms": A.qtl(q, 0.95),
         "tit_p50_ms": A.qtl(res, 0.5), "tit_p95_ms": A.qtl(res, 0.95),
         "gec_pay": len(gec) / n, "gec_lat_ms": A.qtl(gec, 0.5),
         "eksik_pay": sum(1 for u in lat if u == math.inf) / n,
         "ilk_p50_ms": A.qtl(first, 0.5) if first else None}
    # Kilit başlangıcı: ilk pencere ki birikim ≥ 300 ms ve sonraki pencerelerde sönmüyor (betimsel).
    onset = None
    for w in range(len(wmins)):
        if wmins[w] - b0 >= 300 and all(v - b0 >= 150 for v in wmins[w:] if v < math.inf):
            onset = w_idx[w] * t_ms / 1000.0
            break
    d["kilit_baslangic_s"] = onset
    return d


def yukle(roots):
    rows, konum = [], {}
    for cell, cm, arm, am, rdir in A.walk(roots):
        r, x = A.analyze_run(cell, cm, arm, am, rdir)
        info = cell_info(cell)
        r.update({k: info[k] for k in ("hiz", "gec", "kayip", "demet")})
        try:
            with open(os.path.join(rdir, "realized_pre.json")) as fh:
                pre = json.load(fh)
            r["pre_temiz"] = int(all(q.get("packets", 0) == 0 and q.get("qlen", 0) == 0
                                     and q.get("backlog", 0) == 0
                                     for q in pre.get("mid_qdisc", []) if q.get("kind") == "netem"))
        except (OSError, ValueError):
            r["pre_temiz"] = None
        if x:
            ana_k = "ot_T2_800" if info["gec"] == "d1000" else "ot_T1_800"
            r["ana"] = r.get(ana_k)
            r["ana_sirali"] = r.get("ot_sirali_T2_800" if info["gec"] == "d1000" else "ot_sirali_T1_800")
            r.update(ayristir(r, x, info))
            p = r.get("gerceklesen_kayip_ileri")
            r["p_ileri"] = p
            r["plan_parite"] = r.get("latch_enq_r")
            pl, tx = f(r.get("latch_enq_r")), f(r.get("parite_tx"))
            r["parite_gitti_pay"] = tx / pl if pl and tx is not None else None
            r["cozulen_blok"] = r.get("s_fec_blocks_decoded")
            alan = f(r.get("parite_alan_ms"))
            r["parite_bekleme_ust_ms"] = alan / tx if alan is not None and tx else None
            r["parite_bekleme_alt_ms"] = alan / pl if alan is not None and pl else None
            talk = r["cerceve"] * info["demet"] / 1000.0
            gerekli = ((f(r.get("data_B")) or 0) + (f(r.get("retx_B")) or 0)) / talk
            r["hiz_payi"] = f(r.get("btlbw")) / gerekli if gerekli and f(r.get("btlbw")) is not None else None
            l2 = f(r.get("l2_B"))
            r["ana_per_kB"] = r["ana"] * r["cerceve"] / (l2 / 1000.0) if l2 else None
            K = x["K"]
            lim = (mu_lim(info) if info["gec"] == "d1000" else 800.0)
            nf = len(x["m2e"])
            for ci in range(4):
                dil = x["m2e"][ci * nf // 4:(ci + 1) * nf // 4]
                r[f"c{ci + 1}"] = sum(1 for v in dil if v <= lim) / len(dil) if dil else None
            # Kilit başlangıcının oturum yaşı: t0'a kadar geçen (bench başlangıcı → t0) + başlangıç.
            with open(os.path.join(rdir, "out.jsonl")) as fh:
                meta = json.loads(fh.readline())["meta"]
            on = r.get("kilit_baslangic_s")
            r["kilit_yas_rtt"] = ((int(meta.get("events_epoch_offset_ns", 0)) / 1e9 + on)
                                  / (2 * M.DELAYS[info["gec"]][0] / 1000.0) if on is not None else None)
            if K:
                r.update(parite_profili(rdir, meta))
                d = konum.setdefault((cell, arm), [[0, 0] for _ in range(K)])
                for i, v in enumerate(x["m2e"]):
                    d[i % K][1] += 1
                    d[i % K][0] += 1 if v <= lim else 0
        rows.append(r)
    return rows, konum


def mu_lim(info):
    return M.DELAYS[info["gec"]][0] + 800.0


def parite_profili(rdir, meta):
    """pending_parity'nin t0'a göre 3 s'lik kutularda ortalaması (ts.csv, 50 ms örnek)."""
    arm0 = int(((meta.get("budget") or {}).get("armed_at_ns")) or 0) / 1e9
    txt = A.read_text(os.path.join(rdir, "ts.csv"))
    if not txt:
        return {}
    lines = txt.splitlines()
    hdr = lines[0].split(",")
    it, ip = hdr.index("t_ms"), hdr.index("pending_parity")
    kutu = {}
    for line in lines[1:]:
        c = line.split(",")
        if len(c) <= max(it, ip):
            continue
        t = int(c[it]) / 1000.0 - arm0
        b = math.floor(t / 3.0)
        if 0 <= b <= 7:
            kutu.setdefault(b, []).append(int(c[ip]))
    return {f"pq{b * 3}": sum(v) / len(v) for b, v in kutu.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--csv", required=True)
    ap.add_argument("--hizli", action="store_true", help="kod denetimi: B küçük; sayılar rapora girmez")
    ap.add_argument("roots", nargs="+")
    a = ap.parse_args()
    if a.hizli:
        global B_FARK, B_BONF, B_AYR
        B_FARK, B_BONF, B_AYR = 40, 40, 40
    os.makedirs(a.csv, exist_ok=True)
    rows, konum = yukle(a.roots)
    keys = []
    for r in rows:
        for k in r:
            if k not in keys:
                keys.append(k)
    with open(os.path.join(a.csv, "kosular.csv"), "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=keys)
        w.writeheader()
        w.writerows(rows)

    g = {}
    for r in rows:
        g.setdefault(r["hucre"], {}).setdefault(r["kol"], []).append(r)

    def sira(c):
        i = cell_info(c)
        return (i["on"] != "c", i["hiz"] != "r12k", i["gec"] != "d300", i["kayip"], i["demet"])

    hucreler = sorted(g, key=sira)

    def val(c, kol, m):
        return [f(r.get(m)) for r in g[c].get(kol, []) if r.get("gecerli") == 1]

    def nn(c, kol, m):
        return [v for v in val(c, kol, m) if v is not None]

    def birincil(c):
        i = cell_info(c)
        return i["on"] == "c" and i["kayip"] < 50 and len(g[c].get("arq_taban", [])) >= 30

    # hucreler.csv
    with open(os.path.join(a.csv, "hucreler.csv"), "w", newline="") as fh:
        wr = None
        for c in hucreler:
            for kol, rs in sorted(g[c].items()):
                s = {"hucre": c, "kol": kol, "N": len(rs), "N_gecerli": sum(1 for r in rs if r.get("gecerli") == 1),
                     "etiket": "hüküm" if birincil(c) else "betimsel",
                     "kilitli": sum(int(r.get("kilitli") or 0) for r in rs)}
                for m in DAG:
                    v = nn(c, kol, m)
                    if v:
                        s[f"{m}_med"], s[f"{m}_q1"], s[f"{m}_q3"] = q3(v)[1], q3(v)[0], q3(v)[2]
                        s[f"{m}_min"], s[f"{m}_max"] = min(v), max(v)
                    else:
                        for e in ("med", "q1", "q3", "min", "max"):
                            s[f"{m}_{e}"] = None
                if wr is None:
                    wr = csv.DictWriter(fh, fieldnames=list(s.keys()))
                    wr.writeheader()
                wr.writerow(s)

    # farklar.csv
    fark = {}
    with open(os.path.join(a.csv, "farklar.csv"), "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["hucre", "x", "y", "olcu", "n_x", "n_y", "med_x", "med_y", "fark", "lo", "hi", "durum"])
        for c in hucreler:
            ciftler = []
            for b in IKILI:
                for k in ("uyar", "rs2", "rs4", "tekrar"):
                    ciftler.append((f"{k}_{b}", f"arq_{b}"))
                ciftler.append((f"rs2_{b}", f"tekrar_{b}"))
                ciftler.append((f"rs4_{b}", f"tekrar_{b}"))
                ciftler.append((f"rs2_{b}", f"rs4_{b}"))
            for k in KOLLAR:
                ciftler.append((f"{k}_aday", f"{k}_taban"))
            for x, y in ciftler:
                if x not in g[c] or y not in g[c]:
                    continue
                for m in METRIK_FARK:
                    vx, vy = nn(c, x, m), nn(c, y, m)
                    d0, lo, hi = A.boot_diff(vx, vy, A.fnv(f"{c}|{x}|{y}|{m}"), B_FARK)
                    fark[(c, x, y, m)] = (d0, lo, hi)
                    w.writerow([c, x, y, m, len(vx), len(vy), A.med(vx), A.med(vy), d0, lo, hi,
                                durum(lo, hi) if m != "B_cerceve_l2" else ""])

    bi = [c for c in hucreler if birincil(c)]
    aile = len(bi) * 2 * 4
    # Bonferroni aralığı (birincil aile)
    bonf = {}
    alpha = 0.05 / aile
    for c in bi:
        for b in IKILI:
            for k in ("uyar", "rs2", "rs4", "tekrar"):
                vx, vy = nn(c, f"{k}_{b}", "ana"), nn(c, f"arq_{b}", "ana")
                rng = random.Random(A.fnv(f"bonf|{c}|{k}|{b}"))
                ds = []
                for _ in range(B_BONF):
                    rx = [vx[rng.randrange(len(vx))] for _ in vx]
                    ry = [vy[rng.randrange(len(vy))] for _ in vy]
                    ds.append(A.med(rx) - A.med(ry))
                bonf[(c, k, b)] = (A.qtl(ds, alpha / 2), A.qtl(ds, 1 - alpha / 2))

    P = print
    P("# Ajan C tam analiz tabloları (ÜRETİLDİ: scripts/ktp2/f0/c/tam_c.py)\n")
    P("Değerler koşular arası medyan [Q1–Q3] (min–maks), oran 0–1; birim = koşu; dörtte birlikler doğrusal ara değer.")
    P("Bench üst sınırı [D-9]: S1 `Reorder` ve Noise zarfı yok (zarf baytı dolguyla eşlendi). `sıralı` = alıcı damgalarından türetilmiş Reorder karşılığı.\n")

    # T.1
    P("## T.1 Geçerlilik (hücre başına)\n")
    P("| hücre | etiket | kol × N | rc dağılımı | geçerli | rx = sunucu teslim | ısınma artığı > 0 (koşu) | pre temiz | kayıp ileri med (min–maks) | kayıp dönüş med (min–maks) |")
    P("|---|---|---|---|---|---|---|---|---|---|")
    top = dict(k=0, g=0, rx=0, pre=0, ar=0)
    for c in hucreler:
        rs = [r for kol in g[c].values() for r in kol]
        rc = {}
        for r in rs:
            rc[r.get("rc")] = rc.get(r.get("rc"), 0) + 1
        fl = [f(r.get("gerceklesen_kayip_ileri")) for r in rs]
        rl = [f(r.get("gerceklesen_kayip_donus")) for r in rs]
        fl = [v for v in fl if v is not None]
        rl = [v for v in rl if v is not None]
        rxeq = sum(1 for r in rs if str(r.get("rx_satir")) == str(r.get("s_delivered")))
        ar = sum(1 for r in rs if (f(r.get("warmup_residue")) or 0) > 0)
        pre = sum(1 for r in rs if r.get("pre_temiz") == 1)
        ok = sum(1 for r in rs if r.get("gecerli") == 1)
        ns = sorted({len(v) for v in g[c].values()})
        top["k"] += len(rs); top["g"] += ok; top["rx"] += rxeq; top["pre"] += pre; top["ar"] += ar
        P(f"| {c} | {'hüküm' if birincil(c) else 'betimsel'} | {len(g[c])} × {ns} | {rc} | {ok}/{len(rs)} | {rxeq}/{len(rs)} | {ar} | {pre}/{len(rs)} | "
          f"{fm(A.med(fl), 3)} ({fm(min(fl), 3)}–{fm(max(fl), 3)}) | {fm(A.med(rl), 3)} ({fm(min(rl), 3)}–{fm(max(rl), 3)}) |")
    P(f"\nToplam: {top['k']} koşu, geçerli {top['g']}, rx = teslim {top['rx']}, pre temiz {top['pre']}, ısınma artığı > 0 {top['ar']}.\n")

    # T.2
    P("## T.2 Son-tarih oranı, sırasız (bench üst sınırı) ve tel baytı\n")
    P("T1 = ağızdan kulağa ≤ D; T2 = ağızdan kulağa ≤ OWD_nom + b. L2 = (IP + Ethernet) bayt/çerçeve, t0 sonrası ileri yön.\n")
    for c in hucreler:
        P(f"### {c} ({'hüküm' if birincil(c) else 'betimsel'})\n")
        P("| kol | N | kilitli | T1-400 | T1-800 | T2-400 | T2-800 | L2 B/çerçeve |\n|---|---|---|---|---|---|---|---|")
        for kol in sorted(g[c]):
            rs = g[c][kol]
            P(f"| {kol} | {len(rs)} | {sum(int(r.get('kilitli') or 0) for r in rs)} | {dag(val(c, kol, 'ot_T1_400'))} | {dag(val(c, kol, 'ot_T1_800'))} | "
              f"{dag(val(c, kol, 'ot_T2_400'))} | {dag(val(c, kol, 'ot_T2_800'))} | {kisa(val(c, kol, 'B_cerceve_l2'), 0)} |")
        P("")

    # T.3
    P("## T.3 Kola karşı r = 0 (arq) farkı, ana ölçü, N = 30 hücreler\n")
    P(f"ARQ her kolda açık; kıyas = ARQ + artıklık vs ARQ. Medyan farkı [%95 bootstrap] {{Bonferroni, aile = {aile}}}. Ana ölçü d300/r1k T1-800, d1000 T2-800.\n")
    P("| hücre | ikili | arq medyan | uyar − arq | rs2 − arq | rs4 − arq | tekrar − arq |\n|---|---|---|---|---|---|---|")
    for c in bi:
        for b in IKILI:
            hucre = [f"{c}", b, fm(A.med(nn(c, f"arq_{b}", "ana")), 3)]
            for k in ("uyar", "rs2", "rs4", "tekrar"):
                d0, lo, hi = fark[(c, f"{k}_{b}", f"arq_{b}", "ana")]
                bl, bh = bonf[(c, k, b)]
                hucre.append(f"{fm(d0, 3)} [{fm(lo, 3)}, {fm(hi, 3)}] {{{fm(bl, 3)}, {fm(bh, 3)}}} {durum(lo, hi)}")
            P("| " + " | ".join(hucre) + " |")
    P("")

    # T.4
    P("## T.4 Reorder türevi (sıralı teslim) — ana ölçü, N = 30 hücreler [türetilmiş, bench üst sınırı]\n")
    P("Sırasız değerin yanında. Sıralı değer, S1 `Reorder`'ın PTT-yalnız oturumdaki alt sınır gecikmesidir: başka akış yok, sıra başı boşlukları yalnız bu akıştan.\n")
    P("| hücre | ikili | arq sırasız / sıralı | uyar | rs2 | rs4 | tekrar | rs2 − arq (sıralı) [%95] | rs4 − arq (sıralı) [%95] |\n|---|---|---|---|---|---|---|---|---|")
    for c in bi:
        for b in IKILI:
            cols = []
            for k in KOLLAR:
                cols.append(f"{fm(A.med(nn(c, f'{k}_{b}', 'ana')))} / {fm(A.med(nn(c, f'{k}_{b}', 'ana_sirali')))}")
            d2 = fark[(c, f"rs2_{b}", f"arq_{b}", "ana_sirali")]
            d4 = fark[(c, f"rs4_{b}", f"arq_{b}", "ana_sirali")]
            P(f"| {c} | {b} | " + " | ".join(cols) + f" | {fm(d2[0], 3)} [{fm(d2[1], 3)}, {fm(d2[2], 3)}] | {fm(d4[0], 3)} [{fm(d4[1], 3)}, {fm(d4[2], 3)}] |")
    P("")

    # T.5
    P("## T.5 Gecikme bütçesi bileşenleri (ms) — N = 30 hücreler\n")
    P("`demet` ve `blok` yapılandırma; `seri` hesap (L2/hız); `ilk p50` = ilk pencerede alıcı gecikmesi medyanı (ağ + seri, ölçüldü); `kuyruk` = gönderen birikimi (pencere-minimum artışı, çerçeve medyanı / p95; ölçüldü, türetilmiş); `titreşim` = doğrudan sınıf artığı p50 / p95; `geç` = doğrudan sınıf dışı çerçeve payı ve birikim hariç gecikme medyanı; `parite` = kuyruk beklemesi alt–üst sınır (Little). Koşular arası medyan.\n")
    P("| hücre | kol | demet | blok ort/maks | seri veri | ağ nominal | ilk p50 | kuyruk med / p95 | titreşim p50 / p95 | geç pay / gecikme | eksik pay | parite alt–üst |\n|---|---|---|---|---|---|---|---|---|---|---|---|")
    for c in bi:
        i = cell_info(c)
        ser = (M.data_udp(i["demet"]) + M.L2) / M.RATES[i["hiz"]] * 1000.0
        for kol in sorted(g[c]):
            kk = kol.split("_")[0]
            K = {"rs2": 2, "rs4": 4}.get(kk)
            blk = f"{(K - 1) * i['demet'] / 2:.0f}/{(K - 1) * i['demet']}" if K else "—"
            par = (f"{fm(A.med(nn(c, kol, 'parite_bekleme_alt_ms')), 0)}–{fm(A.med(nn(c, kol, 'parite_bekleme_ust_ms')), 0)}"
                   if K else "—")
            P(f"| {c} | {kol} | {i['demet']} | {blk} | {fm(ser, 1)} | {M.DELAYS[i['gec']][0]:.0f} | {fm(A.med(nn(c, kol, 'ilk_p50_ms')), 0)} | "
              f"{fm(A.med(nn(c, kol, 'q_med_ms')), 0)} / {fm(A.med(nn(c, kol, 'q_p95_ms')), 0)} | {fm(A.med(nn(c, kol, 'tit_p50_ms')), 0)} / {fm(A.med(nn(c, kol, 'tit_p95_ms')), 0)} | "
              f"{fm(A.med(nn(c, kol, 'gec_pay')), 3)} / {fm(A.med(nn(c, kol, 'gec_lat_ms')), 0)} | {fm(A.med(nn(c, kol, 'eksik_pay')), 3)} | {par} |")
    P("")
    # serileştirme doğrulaması
    P("Serileştirme denetimi (ölçülen b0 farkı r1k − r12k, d300, demet 200, %20, arq+uyar, ilk üç pencere minimumu, koşular arası medyan):")
    a1, a2 = [], []
    for k in ("arq_taban", "arq_aday", "uyar_taban", "uyar_aday"):
        a1 += nn("c_r1k_d300_l20_b200", k, "b0_ms")
        a2 += nn("c_r12k_d300_l20_b200", k, "b0_ms")
    P(f"r1k {fm(A.med(a1), 1)} − r12k {fm(A.med(a2), 1)} = {fm(A.med(a1) - A.med(a2), 1)} ms; hesap (118 B / 1000 − 118 B / 12500) = {fm(118.0 - 9.44, 1)} ms.\n")

    # T.6
    P("## T.6 Kilitli / kilitsiz alt kümeler (ana ölçü)\n")
    P("`kilitli` tanımı sonuca bağlıdır (gecikmeden türer): alt küme kıyasları betimseldir, nedensel değildir. n < 30 alt küme betimsel.\n")
    P("| hücre | kol | kilitli n / N | kilitsiz: ana medyan (n) | kilitli: ana medyan (n) | btlbw/sunulan | hız payı | tepe birikim ms (kilitli) | kilit başlangıcı s medyan (n) |\n|---|---|---|---|---|---|---|---|---|")
    for c in hucreler:
        for kol in sorted(g[c]):
            rs = [r for r in g[c][kol] if r.get("gecerli") == 1]
            u = [f(r.get("ana")) for r in rs if not r.get("kilitli")]
            k = [f(r.get("ana")) for r in rs if r.get("kilitli")]
            te = [f(r.get("birikim_tepe_ms")) for r in rs if r.get("kilitli")]
            te = [v for v in te if v is not None]
            on = [f(r.get("kilit_baslangic_s")) for r in rs]
            on = [v for v in on if v is not None]
            P(f"| {c} | {kol} | {len(k)}/{len(rs)} | {fm(A.med(u), 3)} ({len(u)}) | {fm(A.med(k), 3)} ({len(k)}) | {fm(A.med(nn(c, kol, 'btlbw_sunulan')))} | "
              f"{fm(A.med(nn(c, kol, 'hiz_payi')))} | {fm(A.med(te), 0)} | {fm(A.med(on), 1)} ({len(on)}) |")
    P("")

    # T.7 ayrıştırma
    P("## T.7 rs − arq farkının ayrıştırması (ortalama; N = 30 hücreler)\n")
    P("R = ort(1 − p_ileri), ana ölçü. kilit = R − ort(arq); parite = ort(rs) − R; toplam = kilit + parite. [%95 bootstrap, koşu birimli]. pay = kilit / toplam. `kurtarma` = parite / ort(p_ileri): kaybolan çerçevelerin son tarih içinde parite ile telafi edilen payı.\n")
    P("| hücre | ikili | kol | ort(arq) | R | ort(rs) | toplam | kilit | parite | kilit payı | kurtarma | arq kilitsiz ort (n) |\n|---|---|---|---|---|---|---|---|---|---|---|---|")
    ayr = []
    for c in bi:
        for b in IKILI:
            for kk in ("rs2", "rs4"):
                arq = [r for r in g[c][f"arq_{b}"] if r.get("gecerli") == 1]
                rs = [r for r in g[c][f"{kk}_{b}"] if r.get("gecerli") == 1]
                ia = [(f(r["ana"]), f(r["p_ileri"])) for r in arq if f(r.get("p_ileri")) is not None]
                ir = [(f(r["ana"]), f(r["p_ileri"])) for r in rs if f(r.get("p_ileri")) is not None]

                def calc(sa, sr):
                    ma = sum(v[0] for v in sa) / len(sa)
                    mr = sum(v[0] for v in sr) / len(sr)
                    pp = sum(v[1] for v in sa + sr) / (len(sa) + len(sr))
                    return ma, mr, 1 - pp, pp

                ma, mr, R, pp = calc(ia, ir)
                tot, kil, par = mr - ma, R - ma, mr - R
                seed = A.fnv(f"ayr|{c}|{b}|{kk}")
                tl, th = boot_mean([ia, ir], lambda s: calc(s[0], s[1])[1] - calc(s[0], s[1])[0], seed)
                kl, kh = boot_mean([ia, ir], lambda s: calc(s[0], s[1])[2] - calc(s[0], s[1])[0], seed + 1)
                pl, ph = boot_mean([ia, ir], lambda s: calc(s[0], s[1])[1] - calc(s[0], s[1])[2], seed + 2)
                un = [f(r["ana"]) for r in arq if not r.get("kilitli")]
                um = sum(un) / len(un) if un else None
                ayr.append({"hucre": c, "ikili": b, "kol": kk, "ort_arq": ma, "R": R, "ort_rs": mr, "toplam": tot,
                            "toplam_lo": tl, "toplam_hi": th, "kilit": kil, "kilit_lo": kl, "kilit_hi": kh,
                            "parite": par, "parite_lo": pl, "parite_hi": ph, "kilit_payi": kil / tot if tot else None,
                            "kurtarma": par / pp, "arq_kilitsiz_ort": um, "arq_kilitsiz_n": len(un)})
                P(f"| {c} | {b} | {kk} | {fm(ma, 3)} | {fm(R, 3)} | {fm(mr, 3)} | {fm(tot, 3)} [{fm(tl, 3)}, {fm(th, 3)}] | {fm(kil, 3)} [{fm(kl, 3)}, {fm(kh, 3)}] | "
                  f"{fm(par, 3)} [{fm(pl, 3)}, {fm(ph, 3)}] | {fm(kil / tot if tot else None, 2)} | {fm(par / pp, 2)} | {fm(um, 3)} ({len(un)}) |")
    P("")
    with open(os.path.join(a.csv, "ayristirma.csv"), "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(ayr[0].keys()))
        w.writeheader()
        w.writerows(ayr)

    # T.8
    P("## T.8 Kilitsiz alt kümede tekrar ↔ rs (ana ölçü, betimsel)\n")
    P("| hücre | ikili | tekrar kilitsiz medyan (n) | rs2 kilitsiz medyan (n) | rs4 kilitsiz medyan (n) | 1 − p̄ | 1 − p̄² | rs2 binom | rs parite beklemesi alt–üst (rs2) |\n|---|---|---|---|---|---|---|---|---|")
    for c in bi:
        i = cell_info(c)
        for b in IKILI:
            def alt(kol):
                v = [f(r.get("ana")) for r in g[c][f"{kol}_{b}"] if r.get("gecerli") == 1 and not r.get("kilitli")]
                return f"{fm(A.med(v), 3)} ({len(v)})"

            pp = A.med(nn(c, f"arq_{b}", "p_ileri"))
            r2 = M.r_select("rs2", i["kayip"], i["demet"], M.RATES[i["hiz"]])
            P(f"| {c} | {b} | {alt('tekrar')} | {alt('rs2')} | {alt('rs4')} | {fm(1 - pp, 3)} | {fm(1 - pp ** 2, 3)} | {fm(M.p_play(2, r2, i['kayip'] / 100.0), 3)} | "
              f"{fm(A.med(nn(c, f'rs2_{b}', 'parite_bekleme_alt_ms')), 0)}–{fm(A.med(nn(c, f'rs2_{b}', 'parite_bekleme_ust_ms')), 0)} |")
    P("")

    # T.9
    P("## T.9 B-C1: uyarlamalı kol, parite ve §8.5 kırpması\n")
    P("| hücre | kol | N | parite > 0 koşu | parite toplam | t0 sonrası fec bayt > 0 koşu | kırpma medyan | arbiter öneri medyan | btlbw medyan B/s | final oran ort ‰ |\n|---|---|---|---|---|---|---|---|---|---|")
    ttop = {}
    for c in hucreler:
        for kol in sorted(g[c]):
            if kol.split("_")[0] not in ("uyar", "uyar64", "pol"):
                continue
            rs = [r for r in g[c][kol] if r.get("gecerli") == 1]
            pz = [f(r.get("parite_tx")) or 0 for r in rs]
            fb = sum(1 for r in rs if (f(r.get("fec_B")) or 0) > 0)
            t = ttop.setdefault(kol, [0, 0, 0, 0])
            t[0] += len(rs); t[1] += sum(1 for v in pz if v > 0); t[2] += int(sum(pz)); t[3] += fb
            P(f"| {c} | {kol} | {len(rs)} | {sum(1 for v in pz if v > 0)} | {int(sum(pz))} | {fb} | {fm(A.med(nn(c, kol, 'clamp')), 0)} | {fm(A.med(nn(c, kol, 'arb_oneri')), 0)} | "
              f"{fm(A.med(nn(c, kol, 'btlbw')), 0)} | {fm(A.med([f(r.get('final_ratio_ort')) for r in rs if f(r.get('final_ratio_ort')) is not None]), 1)} |")
    P("\nToplam (kol: koşu, parite > 0 koşu, parite adedi, t0 sonrası fec baytı > 0 koşu): " + "; ".join(f"{k}: {v[0]}, {v[1]}, {v[2]}, {v[3]}" for k, v in sorted(ttop.items())) + "\n")

    # T.10
    P("## T.10 B-C4: parite akıbeti (sabit RS kolları)\n")
    P("Plan = latch `enq_r`; tx = tele çıkan; bekleme alt = ∫kuyruk / plan, üst = ∫kuyruk / tx. Sunucu: FEC'le ilk ulaşan birim / çözülen blok (ısınma dahil). Uyarı: r ≥ K kollarında (rs2 r = 2) `çözülen blok` tamamlanmış bloğu geç onarımla yeniden çözen koşmaları da sayar (`fec_ingest`, `runtime.rs:3232`/`:3255`); oran parite gecikmesi ölçüsü değildir.\n")
    P("| hücre | kol | N | kilitli | tx / plan payı | bekleme alt–üst ms | jeton reddi | FEC ilk ulaşan | çözülen blok | ilk ulaşan / çözülen |\n|---|---|---|---|---|---|---|---|---|---|")
    for c in hucreler:
        for kol in sorted(g[c]):
            if kol.split("_")[0] not in ("rs2", "rs4"):
                continue
            rs = g[c][kol]
            fk, cz = A.med(nn(c, kol, "fec_kurtarilan")), A.med(nn(c, kol, "cozulen_blok"))
            P(f"| {c} | {kol} | {len(rs)} | {sum(int(r.get('kilitli') or 0) for r in rs)} | {kisa(val(c, kol, 'parite_gitti_pay'))} | "
              f"{fm(A.med(nn(c, kol, 'parite_bekleme_alt_ms')), 0)}–{fm(A.med(nn(c, kol, 'parite_bekleme_ust_ms')), 0)} | {fm(A.med(nn(c, kol, 'parity_denied')), 0)} | {fm(fk, 0)} | {fm(cz, 0)} | "
              f"{fm(fk / cz if cz else None, 2)} |")
    P("")

    # T.11
    P("## T.11 B-C2: bloktaki konuma göre ana ölçü (çerçeve paydalı, koşular havuzlu, betimsel)\n")
    P("| hücre | kol | konum 0…K−1 oranı |\n|---|---|---|")
    for (c, kol), d in sorted(konum.items(), key=lambda kv: (sira(kv[0][0]), kv[0][1])):
        if birincil(c):
            P(f"| {c} | {kol} | " + " · ".join(fm(ok / n if n else None, 3) for ok, n in d) + " |")
    P("")

    # T.12
    P("## T.12 Bant bütçesi eşitliği (L2 B/çerçeve, medyan; ±%10 eşit sayılır)\n")
    P("| hücre | ikili | arq | rs2 (oran/arq) | rs4 (oran/arq) | tekrar (oran/arq) | rs2 / tekrar | eşit bütçe çiftleri | ana/kB: arq · rs2 · rs4 · tekrar |\n|---|---|---|---|---|---|---|---|---|")
    for c in bi:
        for b in IKILI:
            m = {k: A.med(nn(c, f"{k}_{b}", "B_cerceve_l2")) for k in KOLLAR}
            pk = {k: A.med(nn(c, f"{k}_{b}", "ana_per_kB")) for k in KOLLAR}
            es = []
            for x, y in (("rs2", "tekrar"), ("rs4", "tekrar"), ("rs2", "rs4")):
                if abs(m[x] / m[y] - 1) <= 0.10:
                    es.append(f"{x}≈{y}")
            P(f"| {c} | {b} | {fm(m['arq'], 0)} | {fm(m['rs2'], 0)} ({fm(m['rs2'] / m['arq'])}) | {fm(m['rs4'], 0)} ({fm(m['rs4'] / m['arq'])}) | {fm(m['tekrar'], 0)} ({fm(m['tekrar'] / m['arq'])}) | "
              f"{fm(m['rs2'] / m['tekrar'])} | {', '.join(es) or 'yok'} | {fm(pk['arq'])} · {fm(pk['rs2'])} · {fm(pk['rs4'])} · {fm(pk['tekrar'])} |")
    P("")

    # T.13
    P("## T.13 taban ↔ aday (05b): kilit insidansı ve ana ölçü farkı\n")
    P("Fisher iki yanlı (N = 30 hücre, kol başına). Fark = aday − taban, medyan [%95].\n")
    P("| hücre | kol | kilitli taban / aday | Fisher p | ana aday − taban [%95] | durum |\n|---|---|---|---|---|---|")
    for c in bi:
        for k in KOLLAR:
            rt, ra = g[c][f"{k}_taban"], g[c][f"{k}_aday"]
            kt, ka = sum(int(r.get("kilitli") or 0) for r in rt), sum(int(r.get("kilitli") or 0) for r in ra)
            p = fisher(kt, len(rt) - kt, ka, len(ra) - ka)
            d0, lo, hi = fark[(c, f"{k}_aday", f"{k}_taban", "ana")]
            P(f"| {c} | {k} | {kt}/{len(rt)} · {ka}/{len(ra)} | {fm(p, 3)} | {fm(d0, 3)} [{fm(lo, 3)}, {fm(hi, 3)}] | {durum(lo, hi)} |")
    P("")
    for k in KOLLAR:
        tt = sum(sum(int(r.get("kilitli") or 0) for r in g[c][f"{k}_taban"]) for c in bi)
        ta = sum(sum(int(r.get("kilitli") or 0) for r in g[c][f"{k}_aday"]) for c in bi)
        n = sum(len(g[c][f"{k}_taban"]) for c in bi)
        P(f"- Havuz (11 N = 30 hücre, betimsel) {k}: taban {tt}/{n}, aday {ta}/{n}.")
    P("")

    # T.14
    P("## T.14 Kilit başlangıcı (s, konuşma başından; ilk ≥ 300 ms kalıcı birikim penceresi)\n")
    P("Son sütun: başlangıç anındaki oturum yaşı (t0'a kadar geçen + başlangıç) nominal RTT biriminde, arq + uyar havuzu. Startup platosu RTT saatlidir (`sender.rs:4208-4235`).\n")
    P("| hücre | arq taban (n) | arq aday (n) | uyar taban (n) | uyar aday (n) | oturum yaşı / RTT medyan [Q1–Q3] (n) |\n|---|---|---|---|---|---|")
    for c in bi:
        cols = []
        havuz = []
        for kol in ("arq_taban", "arq_aday", "uyar_taban", "uyar_aday"):
            v = nn(c, kol, "kilit_baslangic_s")
            cols.append(f"{kisa(v, 1)} ({len(v)})")
            havuz += nn(c, kol, "kilit_yas_rtt")
        P(f"| {c} | " + " | ".join(cols) + f" | {kisa(havuz, 1)} ({len(havuz)}) |")
    P("")

    # T.15
    P("## T.15 Ön-kayıt (model_c, varyant pacer1.25 [aralık]) ↔ ölçüm (ana ölçü, koşular arası medyan)\n")
    with open(os.path.join(HERE, "..", "..", "..", "..", "reports", "ktp2", "f0", "c", "model", "beklenen.csv")) as fh:
        bek = list(csv.DictReader(fh))
    P("| hücre | kol | ön-kayıt | ölçüm taban | ölçüm aday | ölçüm kilitsiz (taban+aday, n) |\n|---|---|---|---|---|---|")
    for c in bi:
        i = cell_info(c)
        tag = "T2_800_ort" if i["gec"] == "d1000" else "T1_800_ort"
        for k in KOLLAR:
            mk = {"uyar": "arq", "tekrar": "tekrar2"}.get(k, k)
            s = [r for r in bek if r["hiz"] == i["hiz"] and r["gecikme"] == i["gec"] and int(r["kayip"]) == i["kayip"]
                 and int(r["demet_ms"]) == i["demet"] and r["kol"] == mk]
            vv = [float(r[tag]) for r in s]
            mid = [float(r[tag]) for r in s if r["varyant"] in ("pacer1.25",)] or vv
            tx = fm(mid[0], 3) + (f" ({fm(min(vv), 3)}–{fm(max(vv), 3)})" if len(vv) > 1 else "")
            u = [f(r.get("ana")) for b in IKILI for r in g[c][f"{k}_{b}"] if not r.get("kilitli")]
            P(f"| {c} | {k} | {tx} | {fm(A.med(nn(c, f'{k}_taban', 'ana')), 3)} | {fm(A.med(nn(c, f'{k}_aday', 'ana')), 3)} | {fm(A.med(u), 3)} ({len(u)}) |")
    P("")

    # T.16
    P("## T.16 Kayıp ekseninde özet (rs kolları, ana ölçü: parite katkısı ve kurtarma; ayristirma.csv'den)\n")
    P("| gecikme / hız | kayıp | demet | kol | parite [%95] taban | parite [%95] aday | kilit taban | kilit aday |\n|---|---|---|---|---|---|---|---|")
    for c in bi:
        i = cell_info(c)
        for kk in ("rs2", "rs4"):
            t = next(x for x in ayr if x["hucre"] == c and x["ikili"] == "taban" and x["kol"] == kk)
            d = next(x for x in ayr if x["hucre"] == c and x["ikili"] == "aday" and x["kol"] == kk)
            P(f"| {i['gec']} / {i['hiz']} | %{i['kayip']} | {i['demet']} | {kk} | {fm(t['parite'], 3)} [{fm(t['parite_lo'], 3)}, {fm(t['parite_hi'], 3)}] | "
              f"{fm(d['parite'], 3)} [{fm(d['parite_lo'], 3)}, {fm(d['parite_hi'], 3)}] | {fm(t['kilit'], 3)} | {fm(d['kilit'], 3)} |")
    P("")

    # T.17
    P("## T.17 Konuşma çeyreklerine göre ana ölçü (çerçeve sırası; koşular arası medyan)\n")
    P("Konuşma ≈ 20 s; çeyrek ≈ 5 s. Kuyruk ve kilit konuşma içinde büyüdüğü için oran çeyreklere göre okunur.\n")
    P("| hücre | kol | Ç1 | Ç2 | Ç3 | Ç4 |\n|---|---|---|---|---|---|")
    for c in bi:
        for kol in sorted(g[c]):
            P(f"| {c} | {kol} | " + " | ".join(fm(A.med(nn(c, kol, f"c{i}")), 3) for i in (1, 2, 3, 4)) + " |")
    P("")

    # T.18
    P("## T.18 Parite kuyruğu zaman profili (ortalama bekleyen parite, t0'dan itibaren 3 s kutu; koşular arası medyan)\n")
    P("| hücre | kol | 0–3 | 3–6 | 6–9 | 9–12 | 12–15 | 15–18 | 18–21 | 21–24 |\n|---|---|---|---|---|---|---|---|---|---|")
    for c in bi:
        for kol in sorted(g[c]):
            if kol.split("_")[0] in ("rs2", "rs4"):
                P(f"| {c} | {kol} | " + " | ".join(fm(A.med(nn(c, kol, f"pq{b}")), 1) for b in (0, 3, 6, 9, 12, 15, 18, 21)) + " |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
