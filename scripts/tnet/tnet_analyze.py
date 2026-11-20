#!/usr/bin/env python3
"""kama-target-net-v2 — DONMUŞ ANALİZ (yalnız stdlib). Her sayı ham sonuç CSV'sinden yeniden türetilir.

Kullanım: tnet_analyze.py --manifest R1=.../manifest-R1.csv --manifest R2=... --result .../results.csv
                          [--result ...] --out OUTDIR
Çıktılar (OUTDIR): ACCOUNTING.md · CELL-RESULTS.csv · GRAPH.csv · TABLES.md

Kurallar (ölçümden ÖNCE donduruldu; docs/reports/kama-target-net-v2-ONKAYIT.md §7):
  * hüküm metriği = horizon-complete oranı, R1+R2 havuz; Wilson %95. "ÖNDE" YALNIZ iki kolun aralıkları
    AYRIŞIYORSA, aksi hâlde "ayırt edilemedi" (kama-vs-tcp-v1 ile aynı).
  * ikili hükümler: kama-port ↔ tcp, kama-port ↔ quinn, kama-port ↔ quinn-bbr (ürün iddiası);
    kama-port ↔ kama-port-nofec (FEC'in nedensel katkısı). Çoklu karşılaştırma düzeltmesi YOK.
  * p50/p90 tamamlanma süresi ve tel/yük oranı BETİMSELDİR (aralık testi yok).
  * status ≠ OK satırlar paydada KALIR ve başarısız sayılır; ayrıca listelenir. ENGINE-/PROFILE-/
    LANE-MISMATCH satırı varsa ilgili kol GEÇERSİZDİR (TABLES.md başında uyarı).
  * SMOKE-NO-NETEM satırları ve `P` (pilot) replikası kampanya satırı DEĞİLDİR: dışlanır, sayılır.
  * loadavg işaretli satırlar (load_flag=1) duyarlılık tablosunda dışlanarak yeniden hükme bağlanır.
"""
import collections
import csv
import math
import os
import statistics
import sys

Z = 1.959963984540054
KAMA = "kama-port"
NOFEC = "kama-port-nofec"
RIVALS = ["tcp", "quinn", "quinn-bbr"]
ARMS = [KAMA, NOFEC, "quinn", "quinn-bbr", "tcp"]
GROUPS = ["control", "main", "band", "asym", "asym-twin"]
CONFIG_BAD = {"ENGINE-MISMATCH", "PROFILE-MISMATCH", "LANE-MISMATCH"}
EXCLUDED_STATUS = {"SMOKE-NO-NETEM"}


def wilson(x, n):
    if n == 0:
        return (0.0, 1.0)
    p = x / n
    d = 1 + Z * Z / n
    c = (p + Z * Z / (2 * n)) / d
    h = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def pct(v, q):
    if not v:
        return ""
    s = sorted(v)
    return s[min(len(s) - 1, int(math.ceil(q * len(s))) - 1)]


def verdict(a, b, na, nb):
    if a["wh"][0] > b["wh"][1]:
        return f"**{na} ÖNDE**"
    if b["wh"][0] > a["wh"][1]:
        return f"**{nb} ÖNDE**"
    return "ayırt edilemedi"


def f3(x):
    return "" if x == "" or x is None else f"{x:.3f}"


def f2(x):
    return "—" if x == "" or x is None else f"{x:.2f}"


def args(flag):
    a = sys.argv[1:]
    return [a[i + 1] for i, x in enumerate(a) if x == flag]


def load(manifest_args, result_args):
    man = {}
    for x in manifest_args:
        k, v = x.split("=", 1)
        man[k] = list(csv.DictReader(open(v)))
    res = []
    for v in result_args:
        res.extend(csv.DictReader(open(v)))
    return man, res


def summarize(sel):
    g = collections.defaultdict(list)
    for r in sel:
        g[(r["group"], r["cell"], r["arm"])].append(r)
    o = {}
    for k, v in g.items():
        n = len(v)
        ok = [r for r in v if r["status"] == "OK"]
        xh = sum(int(r["complete_horizon"]) for r in ok)
        xd = sum(int(r["complete_deadline"]) for r in ok)
        tc = [float(r["t_complete_ms"]) for r in ok if r["complete_horizon"] == "1"]
        wire = [int(r["wire_bytes"]) for r in v if r["wire_bytes"] not in ("", None)]
        dpay = sum(int(r["delivered"] or 0) * int(r["payload"]) for r in v)
        done_ratio = [int(r["wire_bytes"]) / (int(r["msgs"]) * int(r["payload"]))
                      for r in ok if r["complete_horizon"] == "1" and r["wire_bytes"] not in ("", None)]
        dl = [int(r["delivered"]) / int(r["msgs"]) for r in ok if r["delivered"] != ""]
        fec = [int(r["kama_fec_bytes"]) for r in v if r.get("kama_fec_bytes") not in ("", None)]
        o[k] = dict(n=n, xh=xh, xd=xd, wh=wilson(xh, n), wd=wilson(xd, n), tc50=pct(tc, 0.5), tc90=pct(tc, 0.9),
                    wire50=statistics.median(wire) if wire else "",
                    wpb_all=(sum(wire) / dpay) if (wire and dpay > 0) else "",
                    wpb_done50=statistics.median(done_ratio) if done_ratio else "",
                    dmean=(sum(dl) / len(dl)) if dl else "",
                    fec_share=(sum(fec) / sum(wire)) if (fec and wire and sum(wire) > 0) else "",
                    flagged=sum(int(r["load_flag"] or 0) for r in v),
                    resid=sum(1 for r in v if r["warmup_residue"] not in ("", "0")),
                    bad=sum(1 for r in v if r["status"] != "OK"),
                    meta=v[0])
    return o


def cell_order(man):
    order, meta = [], {}
    for rows in man.values():
        for r in rows:
            k = (r["group"], r["cell"])
            if k not in meta:
                meta[k] = r
                order.append(k)
    order.sort(key=lambda k: (GROUPS.index(k[0]) if k[0] in GROUPS else 99, int(meta[k]["cell_idx"])))
    return order, meta


def main():
    out = args("--out")[0]
    os.makedirs(out, exist_ok=True)
    man, res = load(args("--manifest"), args("--result"))
    man.pop("P", None)
    order, cmeta = cell_order(man)

    # ── 1) SATIR MUHASEBESİ ──
    acc = ["# Satır muhasebesi\n",
           "| replika | beklenen | üretilen | eksik | fazla | yinelenen | status≠OK | load_flag |",
           "|---|---|---|---|---|---|---|---|"]
    excluded = [r for r in res if r["status"] in EXCLUDED_STATUS or r["replica"] == "P"]
    camp = [r for r in res if not (r["status"] in EXCLUDED_STATUS or r["replica"] == "P")]
    foreign = [r for r in camp if r["replica"] not in man]
    rows = []
    for rep in sorted(man):
        exp = {m["run_id"] for m in man[rep]}
        got = [r for r in camp if r["replica"] == rep]
        cnt = collections.Counter(r["run_id"] for r in got)
        dup = sum(c - 1 for c in cnt.values() if c > 1)
        missing = sorted(exp - set(cnt))
        extra = sorted(set(cnt) - exp)
        bad = [r for r in got if r["status"] != "OK"]
        fl = sum(int(r["load_flag"] or 0) for r in got)
        acc.append(f"| {rep} | {len(exp)} | {len(got)} | {len(missing)} | {len(extra)} | {dup} | {len(bad)} | {fl} |")
        if missing:
            acc.append(f"\n**{rep} eksik satırlar ({len(missing)}):** " + ", ".join(missing[:40])
                       + (" …" if len(missing) > 40 else ""))
        if bad:
            acc.append(f"\n**{rep} status≠OK ({len(bad)}):** "
                       + ", ".join(f"{r['run_id']}={r['status']}" for r in bad[:40]) + (" …" if len(bad) > 40 else ""))
        seen = set()
        for r in got:   # yinelenen run_id: İLK satır geçerli (yürütücü üstüne yazmaz)
            if r["run_id"] in seen or r["run_id"] in extra:
                continue
            seen.add(r["run_id"])
            rows.append(r)
    st = collections.Counter(r["status"] for r in rows)
    acc.append("\n## status dağılımı\n")
    acc.append("\n".join(f"- {k}: {v}" for k, v in sorted(st.items())) or "- (satır yok)")
    commits = collections.Counter(r.get("bin_commit", "") for r in rows if r["arm"].startswith("kama-port"))
    acc.append("\n## Kama ikilisinin commit'i (KP-MANIFEST)\n")
    acc.append("\n".join(f"- `{k or '(boş)'}`: {v} satır" for k, v in sorted(commits.items())) or "- (Kama satırı yok)")
    acc.append(f"\n## Dışlanan satırlar\n\n- SMOKE-NO-NETEM / pilot (P): {len(excluded)}"
               f"\n- manifesti verilmeyen replika: {len(foreign)}")
    open(os.path.join(out, "ACCOUNTING.md"), "w").write("\n".join(acc) + "\n")

    # ── 2) HÜCRE × KOL özetleri ──
    pooled = summarize(rows)
    per = {rep: summarize([r for r in rows if r["replica"] == rep]) for rep in sorted(man)}
    unfl = summarize([r for r in rows if r["load_flag"] in ("0", 0)])

    def keyorder(S):
        idx = {k: i for i, k in enumerate(order)}
        return sorted(S, key=lambda k: (idx.get((k[0], k[1]), 10 ** 6), ARMS.index(k[2]) if k[2] in ARMS else 99))

    with open(os.path.join(out, "CELL-RESULTS.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["scope", "group", "cell", "loss_pct", "loss_dir", "rtt_class", "rate_fwd", "rate_rev", "workload",
                    "mode", "arm", "n", "horizon_x", "horizon_rate", "horizon_lo", "horizon_hi", "deadline_x",
                    "deadline_rate", "deadline_lo", "deadline_hi", "t_complete_p50_ms", "t_complete_p90_ms",
                    "wire_bytes_p50", "wire_per_delivered_payload_all", "wire_per_payload_completed_p50",
                    "fec_parity_share_of_wire", "delivered_frac_mean", "load_flagged", "warmup_residue_rows",
                    "status_not_ok"])
        for scope, S in [("pooled", pooled)] + list(per.items()) + [("pooled-unflagged", unfl)]:
            for k in keyorder(S):
                s = S[k]
                m = s["meta"]
                w.writerow([scope, k[0], k[1], m["loss_pct"], m["loss_dir"], m["rtt_class"], m["rate_fwd"], m["rate_rev"],
                            m["workload"], m["mode"], k[2], s["n"], s["xh"], f3(s["xh"] / s["n"]), f3(s["wh"][0]),
                            f3(s["wh"][1]), s["xd"], f3(s["xd"] / s["n"]), f3(s["wd"][0]), f3(s["wd"][1]),
                            f3(s["tc50"]), f3(s["tc90"]), s["wire50"], f3(s["wpb_all"]), f3(s["wpb_done50"]),
                            f3(s["fec_share"]), f3(s["dmean"]), s["flagged"], s["resid"], s["bad"]])

    # GRAPH.csv — kama-vs-tcp-v1 şekli (ilk 14 sütun aynı sırada), v2 eksenleri SONA eklendi.
    with open(os.path.join(out, "GRAPH.csv"), "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["loss_pct", "rtt_class", "workload", "arm", "completion_rate", "n", "x", "wilson_lo", "wilson_hi",
                    "deadline_rate", "delivered_frac_mean", "scope", "mode", "group",
                    "cell", "loss_dir", "rate_fwd", "rate_rev", "t_p50_ms", "t_p90_ms", "wire_per_delivered_payload"])
        for scope, S in [("pooled", pooled)] + list(per.items()):
            for k in keyorder(S):
                s = S[k]
                m = s["meta"]
                w.writerow([m["loss_pct"], m["rtt_class"], m["workload"], k[2], f3(s["xh"] / s["n"]), s["n"], s["xh"],
                            f3(s["wh"][0]), f3(s["wh"][1]), f3(s["xd"] / s["n"]), f3(s["dmean"]), scope, m["mode"],
                            k[0], k[1], m["loss_dir"], m["rate_fwd"], m["rate_rev"], f3(s["tc50"]), f3(s["tc90"]),
                            f3(s["wpb_all"])])

    # ── 3) TABLOLAR ──
    T = ["# kama-target-net-v2 — TABLOLAR (donmuş analizör çıktısı)\n"]
    badcfg = collections.Counter((r["arm"], r["status"]) for r in rows if r["status"] in CONFIG_BAD)
    if badcfg:
        T.append("> **GEÇERSİZLİK UYARISI:** yapılandırma uyuşmazlığı satırları var — ilgili kolun hücreleri ürün "
                 "sonucu SAYILAMAZ (ONKAYIT §8): " + ", ".join(f"{a}/{s}={n}" for (a, s), n in sorted(badcfg.items())))
    if excluded:
        T.append(f"> Not: {len(excluded)} duman/pilot satırı dışlandı (kampanya satırı değil).")

    def cell_label(m):
        rate = "" if m["rate_fwd"] == "none" else f" · ileri {m['rate_fwd']}"
        rev = "" if m["rate_rev"] == "none" else f" · dönüş {m['rate_rev']}"
        ld = "" if m["loss_dir"] == "both" else " (yalnız ileri)"
        return f"{m['loss_pct']}%{ld} | {m['rtt_class']} | {m['workload']}{rate}{rev}"

    def armfmt(s):
        if not s:
            return "—"
        return (f"{s['xh']}/{s['n']} [{s['wh'][0]:.2f},{s['wh'][1]:.2f}] · {f3(s['tc50']) or '—'}/{f3(s['tc90']) or '—'} ms"
                f" · {f2(s['wpb_all'])}×")

    tallies = {}
    behind = []
    slower = []
    for g in GROUPS:
        keys = [k for k in order if k[0] == g]
        if not keys:
            continue
        T.append(f"\n## {g} — kollar (horizon x/N [Wilson95] · t_tam p50/p90 · Σtel/Σteslim-yük) ve hükümler\n")
        T.append("| kayıp | RTT | yük/hız | " + " | ".join(ARMS) + " | K↔tcp | K↔quinn | K↔quinn-bbr |")
        T.append("|---|---|---|" + "---|" * (len(ARMS) + 3))
        tally = {rv: collections.Counter() for rv in RIVALS}
        for k in keys:
            m = cmeta[k]
            S = {a: pooled.get((k[0], k[1], a)) for a in ARMS}
            vs = []
            for rv in RIVALS:
                if S[KAMA] and S[rv]:
                    v = verdict(S[KAMA], S[rv], KAMA, rv)
                    tally[rv][v] += 1
                    vs.append(v)
                    if v == f"**{rv} ÖNDE**":
                        behind.append((g, cell_label(m), rv))
                    if (S[KAMA]["xh"] / S[KAMA]["n"] >= 0.9 and S[rv]["xh"] / S[rv]["n"] >= 0.9
                            and S[KAMA]["tc50"] != "" and S[rv]["tc50"] != "" and S[KAMA]["tc50"] > S[rv]["tc50"]):
                        slower.append(f"- {g} · {cell_label(m)} · {rv}: kama-port p50 {S[KAMA]['tc50']:.1f} ms ↔ "
                                      f"{rv} p50 {S[rv]['tc50']:.1f} ms")
                else:
                    vs.append("—")
            parts = cell_label(m).split(" | ")
            T.append(f"| {parts[0]} | {parts[1]} | {parts[2]} | " + " | ".join(armfmt(S[a]) for a in ARMS)
                     + " | " + " | ".join(vs) + " |")
        tallies[g] = tally
        for rv in RIVALS:
            T.append(f"\nÖzet {g} · kama-port ↔ {rv}: " + " · ".join(f"{k}: {v}" for k, v in sorted(tally[rv].items())))

    T.append("\n## ÖZET — kama-port'un hücre sayıları (horizon, aralıklar ayrışıyor)\n")
    T.append("| grup | rakip | kama-port ÖNDE | ayırt edilemedi | rakip ÖNDE |")
    T.append("|---|---|---|---|---|")
    for g, tally in tallies.items():
        for rv in RIVALS:
            t = tally[rv]
            T.append(f"| {g} | {rv} | {t.get(f'**{KAMA} ÖNDE**', 0)} | {t.get('ayırt edilemedi', 0)} | "
                     f"{t.get(f'**{rv} ÖNDE**', 0)} |")

    T.append("\n## KAMA'nın GERİDE olduğu hücreler (horizon, aralıklar ayrışıyor)\n")
    T.append("\n".join(f"- {g} · {c} · {rv} ÖNDE" for g, c, rv in behind) if behind else "- (yok)")
    T.append("\n### Tamamlanma SÜRESİNDE kama-port'un geride olduğu hücreler (iki kol da ≥ %90; p50; aralık testi YOK — betimsel)\n")
    T.append("\n".join(slower) if slower else "- (yok)")

    # FEC'in nedensel katkısı
    T.append("\n## FEC'in nedensel katkısı — kama-port ↔ kama-port-nofec (aynı ürün, yalnız parite sıfır)\n")
    T.append("| grup | kayıp | RTT | yük/hız | kama-port | kama-port-nofec | hüküm (horizon) | parite payı (tel) | "
             "Σtel/Σteslim-yük port ↔ nofec | p50 ms port ↔ nofec |")
    T.append("|---|---|---|---|---|---|---|---|---|---|")
    fec_t = collections.Counter()
    for k in order:
        a, b = pooled.get((k[0], k[1], KAMA)), pooled.get((k[0], k[1], NOFEC))
        if not a or not b:
            continue
        v = verdict(a, b, KAMA, NOFEC)
        lab = {f"**{KAMA} ÖNDE**": "FEC karşılığını verdi", f"**{NOFEC} ÖNDE**": "FEC zarar verdi"}.get(v, "ayırt edilemedi")
        fec_t[lab] += 1
        parts = cell_label(cmeta[k]).split(" | ")
        T.append(f"| {k[0]} | {parts[0]} | {parts[1]} | {parts[2]} | {a['xh']}/{a['n']} [{a['wh'][0]:.2f},{a['wh'][1]:.2f}] | "
                 f"{b['xh']}/{b['n']} [{b['wh'][0]:.2f},{b['wh'][1]:.2f}] | {lab} | {f3(a['fec_share']) or '—'} | "
                 f"{f2(a['wpb_all'])} ↔ {f2(b['wpb_all'])} | {f3(a['tc50']) or '—'} ↔ {f3(b['tc50']) or '—'} |")
    T.append("\nÖzet FEC: " + " · ".join(f"{k}: {v}" for k, v in sorted(fec_t.items())))

    # R1 ↔ R2
    T.append("\n## R1 ↔ R2 UYUMU (kol × hücre; horizon oranı)\n")
    reps = sorted(per)
    dis = tot = 0
    lines = []
    if len(reps) == 2:
        for k in keyorder(per[reps[0]]):
            a, b = per[reps[0]][k], per[reps[1]].get(k)
            if not b:
                continue
            tot += 1
            sep = a["wh"][0] > b["wh"][1] or b["wh"][0] > a["wh"][1]
            dis += sep
            if sep:
                lines.append(f"- {k[0]} · {k[1]} · {k[2]}: {reps[0]} {a['xh']}/{a['n']} ↔ {reps[1]} {b['xh']}/{b['n']}")
        T.append(f"{tot} kol×hücre çiftinin {dis} tanesinde replikaların Wilson aralıkları ayrışıyor "
                 f"(bağımsız örneklerde beklenen ≈ %5'in altı).")
        T.append("\n".join(lines) if lines else "- (ayrışan çift yok)")
    else:
        T.append(f"- iki replika yok ({reps}); uyum hesaplanmadı")

    # Yük duyarlılığı
    T.append("\n## YÜK İŞARETİ DUYARLILIĞI (load_flag=1 satırlar dışlanarak hükümler)\n")
    ch = cmpn = 0
    for k in order:
        for rv in RIVALS + [NOFEC]:
            a, b = pooled.get((k[0], k[1], KAMA)), pooled.get((k[0], k[1], rv))
            ua, ub = unfl.get((k[0], k[1], KAMA)), unfl.get((k[0], k[1], rv))
            if a and b and ua and ub:
                cmpn += 1
                ch += verdict(a, b, "K", "R") != verdict(ua, ub, "K", "R")
    fl = sum(int(r["load_flag"] or 0) for r in rows)
    T.append(f"İşaretli satır: {fl}/{len(rows)}. İşaretliler dışlanınca hükmü DEĞİŞEN karşılaştırma: {ch}/{cmpn}.")
    open(os.path.join(out, "TABLES.md"), "w").write("\n".join(T) + "\n")
    print(f"analyze: rows={len(rows)} excluded={len(excluded)} cells={len(order)} -> {out}")


if __name__ == "__main__":
    main()
