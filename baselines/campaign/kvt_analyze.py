#!/usr/bin/env python3
"""KAMA ↔ TCP kampanyası — DONMUŞ ANALİZ (yalnız stdlib). Her sayı ham CSV'den yeniden türetilir.

Kullanım: kvt_analyze.py --manifest R1=.../manifest-R1.csv --manifest R2=... --result R1=.../result-R1.csv
                         --result R2=... --out OUTDIR
Çıktılar (OUTDIR): ACCOUNTING.md · CELL-RESULTS.csv · GRAPH.csv · TABLES.md

Kurallar (ölçümden ÖNCE donduruldu):
  * oranlar Wilson %95 aralığıyla; "ÖNDE" hükmü YALNIZ iki kolun aralıkları AYRIŞIYORSA, aksi hâlde
    "ayırt edilemedi". Hüküm metriği = horizon-complete oranı (R1+R2 havuzlanmış).
  * horizon-complete ve deadline-complete AYRI sütunlar; asla birleştirilmez.
  * status != OK satırlar (ERROR/PROFILE-MISMATCH) paydada KALIR ve başarısız sayılır; ayrıca listelenir.
  * loadavg1 > 2.0 satırlar işaretlidir; duyarlılık tablosu onları dışlayarak yeniden hesaplar.
  * ablasyon (abl-off/abl-on, çıplak Sender) ve COLD senaryosu AYRI tablolardadır; ana tabloya karışmaz.
"""
import collections, csv, math, os, statistics, sys

Z = 1.959963984540054

def wilson(x, n):
    if n == 0: return (0.0, 1.0)
    p = x / n; d = 1 + Z * Z / n
    c = (p + Z * Z / (2 * n)) / d; h = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))

def pct(v, q):
    if not v: return ""
    s = sorted(v); return s[min(len(s) - 1, int(math.ceil(q * len(s))) - 1)]

def args(flag):
    a = sys.argv[1:]; return [a[i + 1] for i, x in enumerate(a) if x == flag]

def main():
    out = args("--out")[0]; os.makedirs(out, exist_ok=True)
    man = {k: list(csv.DictReader(open(v))) for k, v in (x.split("=", 1) for x in args("--manifest"))}
    res = {}
    for k, v in (x.split("=", 1) for x in args("--result")):
        res.setdefault(k, []).extend(csv.DictReader(open(v)))

    # ── 1) SATIR MUHASEBESİ ──
    acc = ["# Satır muhasebesi\n", "| replika | beklenen | üretilen | eksik | fazla | yinelenen | status≠OK | load_flag |", "|---|---|---|---|---|---|---|---|"]
    rows = []
    for rep in sorted(man):
        exp = [m["run_id"] for m in man[rep]]; got = [r["run_id"] for r in res.get(rep, [])]
        cnt = collections.Counter(got); dup = sum(c - 1 for c in cnt.values() if c > 1)
        missing = sorted(set(exp) - set(got)); extra = sorted(set(got) - set(exp))
        bad = [r for r in res.get(rep, []) if r["status"] != "OK"]; fl = sum(int(r["load_flag"]) for r in res.get(rep, []))
        acc.append(f"| {rep} | {len(exp)} | {len(got)} | {len(missing)} | {len(extra)} | {dup} | {len(bad)} | {fl} |")
        if missing: acc.append(f"\n**{rep} eksik satırlar ({len(missing)}):** " + ", ".join(missing[:40]) + (" …" if len(missing) > 40 else ""))
        if bad: acc.append(f"\n**{rep} status≠OK:** " + ", ".join(f"{r['run_id']}={r['status']}" for r in bad[:40]))
        seen = set()
        for r in res.get(rep, []):
            if r["run_id"] in seen or r["run_id"] in extra: continue
            seen.add(r["run_id"]); rows.append(r)
    open(os.path.join(out, "ACCOUNTING.md"), "w").write("\n".join(acc) + "\n")

    # ── 2) HÜCRE × KOL özetleri ──
    def summarize(sel):
        g = collections.defaultdict(list)
        for r in sel: g[(r["group"], r["loss_pct"], r["rtt_class"], r["workload"], r["mode"], r["arm"])].append(r)
        o = {}
        for k, v in g.items():
            n = len(v); ok = lambda r: r["status"] == "OK"
            xh = sum(int(r["complete_horizon"]) for r in v if ok(r)); xd = sum(int(r["complete_deadline"]) for r in v if ok(r))
            tc = [float(r["t_complete_ms"]) for r in v if ok(r) and r["complete_horizon"] == "1"]
            wb = [int(r["wire_bytes"]) for r in v]
            dl = [int(r["delivered"]) / int(r["msgs"]) for r in v if ok(r) and r["delivered"] != ""]
            o[k] = dict(n=n, xh=xh, xd=xd, wh=wilson(xh, n), wd=wilson(xd, n), tc50=pct(tc, 0.5), tc90=pct(tc, 0.9),
                        wire50=statistics.median(wb) if wb else "", dmean=(sum(dl) / len(dl)) if dl else "",
                        flagged=sum(int(r["load_flag"]) for r in v),
                        resid=sum(1 for r in v if r["warmup_residue"] not in ("", "0")))
        return o
    pooled = summarize(rows); per = {rep: summarize([r for r in rows if r["replica"] == rep]) for rep in sorted(man)}
    unfl = summarize([r for r in rows if r["load_flag"] == "0"])

    f3 = lambda x: "" if x == "" else f"{x:.3f}"
    with open(os.path.join(out, "CELL-RESULTS.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["scope","group","loss_pct","rtt_class","workload","mode","arm","n","horizon_x","horizon_rate","horizon_lo","horizon_hi",
                    "deadline_x","deadline_rate","deadline_lo","deadline_hi","t_complete_p50_ms","t_complete_p90_ms","wire_bytes_p50",
                    "delivered_frac_mean","load_flagged","warmup_residue_rows"])
        for scope, S in [("pooled", pooled)] + [(k, v) for k, v in per.items()] + [("pooled-unflagged", unfl)]:
            for k in sorted(S, key=lambda k: (k[0], int(k[1]), k[2], k[3], k[4], k[5])):
                s = S[k]
                w.writerow([scope, *k, s["n"], s["xh"], f3(s["xh"] / s["n"]), f3(s["wh"][0]), f3(s["wh"][1]), s["xd"], f3(s["xd"] / s["n"]),
                            f3(s["wd"][0]), f3(s["wd"][1]), f3(s["tc50"]), f3(s["tc90"]), s["wire50"], f3(s["dmean"]), s["flagged"], s["resid"]])

    with open(os.path.join(out, "GRAPH.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["loss_pct","rtt_class","workload","arm","completion_rate","n","x","wilson_lo","wilson_hi","deadline_rate","delivered_frac_mean","scope","mode","group"])
        for scope, S in [("pooled", pooled)] + list(per.items()):
            for k in sorted(S, key=lambda k: (k[0], int(k[1]), k[2], k[3], k[4], k[5])):
                s = S[k]
                w.writerow([k[1], k[2], k[3], k[5], f3(s["xh"] / s["n"]), s["n"], s["xh"], f3(s["wh"][0]), f3(s["wh"][1]),
                            f3(s["xd"] / s["n"]), f3(s["dmean"]), scope, k[4], k[0]])

    # ── 3) TABLOLAR ──
    T = []
    def cellfmt(s):
        return (f"{s['xh']}/{s['n']} [{s['wh'][0]:.2f},{s['wh'][1]:.2f}]", f"{s['xd']}/{s['n']} [{s['wd'][0]:.2f},{s['wd'][1]:.2f}]",
                f"{f3(s['tc50'])} / {f3(s['tc90'])}", f"{s['wire50']}")
    def verdict(a, b, na, nb):
        if a["wh"][0] > b["wh"][1]: return f"**{na} ÖNDE**"
        if b["wh"][0] > a["wh"][1]: return f"**{nb} ÖNDE**"
        return "ayırt edilemedi"
    def pair_table(title, group, mode, A, B, S):
        T.append(f"\n## {title}\n")
        T.append(f"| kayıp | RTT | yük | {A} horizon x/N [Wilson95] | {B} horizon x/N [Wilson95] | {A} deadline | {B} deadline | {A} t_tam p50/p90 ms | {B} t_tam p50/p90 ms | {A} tel B p50 | {B} tel B p50 | hüküm (horizon) |")
        T.append("|---|---|---|---|---|---|---|---|---|---|---|---|")
        tally = collections.Counter(); behind = []
        keys = sorted({k[1:4] for k in S if k[0] == group and k[4] == mode}, key=lambda k: (int(k[0]), ["none", "low", "med", "high"].index(k[1]), k[2]))
        for loss, rc, wl in keys:
            a, b = S.get((group, loss, rc, wl, mode, A)), S.get((group, loss, rc, wl, mode, B))
            if not a or not b: continue
            fa, fb = cellfmt(a), cellfmt(b); v = verdict(a, b, A, B); tally[v] += 1
            if v == f"**{B} ÖNDE**": behind.append((loss, rc, wl))
            T.append(f"| {loss}% | {rc} | {wl} | {fa[0]} | {fb[0]} | {fa[1]} | {fb[1]} | {fa[2]} | {fb[2]} | {fa[3]} | {fb[3]} | {v} |")
        T.append(f"\nÖzet: " + " · ".join(f"{k}: {v}" for k, v in sorted(tally.items())))
        return behind
    behind = pair_table("ANA TABLO — WARM, ürün (kama-v1-full) ↔ tcp-std, R1+R2 havuz", "main", "warm", "kama-v1-full", "tcp-std", pooled)
    T.append("\n### KAMA'nın GERİDE olduğu hücreler (horizon, aralıklar ayrışıyor)\n")
    T.append("\n".join(f"- kayıp {l}% · RTT {r} · {w}" for l, r, w in behind) if behind else "- (yok)")
    # süre ekseninde gerilik: ikisi de tam/tama yakınken medyan süre
    T.append("\n### Tamamlanma SÜRESİNDE KAMA'nın geride olduğu hücreler (iki kol da ≥ %90 horizon-complete; p50 kıyas, aralık testi YOK — betimsel)\n")
    slow = []
    for k, a in pooled.items():
        if k[0] in ("main", "control") and k[4] == "warm" and k[5] == "kama-v1-full":
            b = pooled.get(k[:5] + ("tcp-std",))
            if b and a["xh"] / a["n"] >= 0.9 and b["xh"] / b["n"] >= 0.9 and a["tc50"] != "" and b["tc50"] != "" and a["tc50"] > b["tc50"]:
                slow.append(f"- {k[0]} · kayıp {k[1]}% · RTT {k[2]} · {k[3]}: kama p50 {a['tc50']:.1f} ms  vs  tcp p50 {b['tc50']:.1f} ms")
    T.append("\n".join(sorted(slow)) if slow else "- (yok)")
    pair_table("KONTROL (bozulmasız) — her kol N/N tamamlamalı", "control", "warm", "kama-v1-full", "tcp-std", pooled)
    pair_table("COLD senaryo (kurulum pencerenin İÇİNDE) — manşet hücreler", "cold", "cold", "kama-v1-full", "tcp-std", pooled)
    pair_table("BİLEŞEN ABLASYONU — çıplak Sender sürücüsü, tail OFF ↔ ON (ürün DEĞİL; TCP ile kıyaslanmaz)", "ablation", "warm", "abl-off", "abl-on", pooled)
    pair_table("KONTROL (ablasyon kolları)", "control", "warm", "abl-off", "abl-on", pooled)

    T.append("\n## R1 ↔ R2 UYUMU (kol × hücre; horizon oranı)\n")
    T.append("| grup | kayıp | RTT | yük | mod | kol | R1 x/N | R2 x/N | aralıklar | ")
    T.append("|---|---|---|---|---|---|---|---|---|")
    reps = sorted(per); dis = 0; tot = 0
    if len(reps) == 2:
        for k in sorted(per[reps[0]], key=lambda k: (k[0], int(k[1]), k[2], k[3], k[4], k[5])):
            a, b = per[reps[0]][k], per[reps[1]].get(k)
            if not b: continue
            tot += 1; sep = a["wh"][0] > b["wh"][1] or b["wh"][0] > a["wh"][1]; dis += sep
            T.append(f"| {k[0]} | {k[1]}% | {k[2]} | {k[3]} | {k[4]} | {k[5]} | {a['xh']}/{a['n']} | {b['xh']}/{b['n']} | {'**AYRIŞIYOR**' if sep else 'örtüşüyor'} |")
        T.append(f"\nÖzet: {tot} kol×hücre çiftinin {dis} tanesinde R1 ve R2 Wilson aralıkları ayrışıyor (beklenen yanlış-pozitif ≈ %5 altı).")

    T.append("\n## YÜK İŞARETİ DUYARLILIĞI (loadavg1 > 2.0 satırlar DIŞLANARAK ana tablo hükümleri)\n")
    ch = 0; cmp_n = 0
    for k, a in pooled.items():
        if k[0] == "main" and k[4] == "warm" and k[5] == "kama-v1-full":
            b = pooled.get(k[:5] + ("tcp-std",)); ua, ub = unfl.get(k), unfl.get(k[:5] + ("tcp-std",))
            if b and ua and ub:
                cmp_n += 1; ch += verdict(a, b, "K", "T") != verdict(ua, ub, "K", "T")
    fl_all = sum(int(r["load_flag"]) for r in rows)
    T.append(f"İşaretli satır: {fl_all}/{len(rows)}. İşaretliler dışlanınca hükmü DEĞİŞEN ana hücre: {ch}/{cmp_n}.")
    open(os.path.join(out, "TABLES.md"), "w").write("\n".join(T) + "\n")
    print(f"analyze: rows={len(rows)} cells={len(pooled)} -> {out}")

if __name__ == "__main__":
    main()
