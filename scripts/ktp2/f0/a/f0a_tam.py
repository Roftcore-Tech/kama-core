#!/usr/bin/env python3
"""[KTP2-F0 · A] Tam set özeti — `f0a_analyze.py` çıktısı + ham koşu dizinlerinden TAM SONUÇ tabloları.

    python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <A> <ham kök>
    python3 -I scripts/ktp2/f0/a/f0a_tam.py --analiz <A> --csv reports/ktp2/f0/a/tam > <tablolar.md>

Yalnız standart kütüphane; ham veriye yazmaz. Çıktı:
  <csv>/kosular.csv    koşu başına satır (geçerlilik, gecikme, katmanlar, sayaçlar)
  <csv>/hucreler.csv   hücre × kol özeti
  <csv>/farklar.csv    taban − aday farkının %95 aralığı (koşu birimli bootstrap)
  stdout               RAPOR-A "TAM SONUÇ" tabloları (Markdown)

Tanımlar:
- Öğe gecikmesi = bob uygulama teslimi − alice uygulama çağrısı. Teslim edilmeyen öğe, gözlem
  sonundan (alice'in bitişi) bir ALT SINIRLA sayılır (sansürlü) ve ayrıca sayılır.
- Koşu başına p50/p95 en yakın sıra; hücre değeri koşular arası medyan [Q1–Q3] (min–maks).
- Katmanlar (`f0a_analyze.py` belgesi): fifo = gönderen yeni veri kuyruğu [türetilmiş],
  net = ağ + birimin kendi kayıp kurtarması [türetilmiş], reorder = alıcı oturum geneli sıralama
  (başka birimlerin kurtarmasını bekleme), consumer = uygulama tüketicisi. Katman p50/p95 yalnız
  teslim edilen öğeler üzerinden.
- Fark aralığı: taban ve aday koşu kümelerinden ayrı ayrı yerine koyarak örnekleme (B = 10 000,
  tohum 20261030), istatistik = medyan(taban) − medyan(aday), %2,5–%97,5 yüzdelikleri.
- Kuyruk sonu takılması (A-11): koşu bitmemiş, alice'te teyitsiz birim ≤ 2, yeni veri kuyruğu 0
  ve son ≥ 120 s'de hiç datagram gönderilmemiş.
- Hız çöküşü (A-10): koşunun sonunda yeni veri kuyruğu ≥ 50 birimken pacer < hattın %5'i.
"""

import argparse
import csv
import glob
import json
import math
import os
import random
import sys

LINK = {"r12k": 12500.0, "r1k": 1000.0, "r25": 25.0}
DELAY = {"d300": 0.3, "d1000": 1.0}
CLASSES = ("T", "S", "P", "K", "KP", "W")
LAYERS = ("fifo", "net", "reorder", "consumer")
BOOT = 10_000
SEED = 20261030


def rank(vals, p):
    if not vals:
        return None
    s = sorted(vals)
    k = max(1, math.ceil(p * len(s)))
    return s[min(k, len(s)) - 1]


def spread(vals):
    v = [x for x in vals if x is not None]
    if not v:
        return None
    return {"n": len(v), "med": rank(v, .5), "q1": rank(v, .25), "q3": rank(v, .75),
            "min": min(v), "max": max(v)}


def fs(s, scale=1.0, nd=1):
    if not s:
        return "—"
    f = lambda x: f"{x / scale:.{nd}f}"
    return f"{f(s['med'])} [{f(s['q1'])}–{f(s['q3'])}] ({f(s['min'])}–{f(s['max'])})"


def link_of(profile):
    for k, v in LINK.items():
        if profile.startswith(k + "_"):
            return v
    return None


def realized_loss(run_dir):
    try:
        post = json.load(open(os.path.join(run_dir, "realized_post.json")))
    except (OSError, ValueError):
        return None, None
    q = {x["dev"]: x for x in post.get("mid_qdisc", []) if x.get("kind") == "netem"}
    def loss(dev):
        x = q.get(dev)
        if not x or (x["packets"] + x["drops"]) == 0:
            return None
        return x["drops"] / (x["packets"] + x["drops"])
    return loss("mb0"), loss("ma0")


def alice_series(run_dir, t0):
    """alice ts örnekleri (bob oturumu): (t, durum, pacer, txq, un, txd)."""
    out = []
    try:
        f = open(os.path.join(run_dir, "alice.jsonl"), encoding="utf-8")
    except OSError:
        return out
    with f:
        for line in f:
            if '"k":"ts"' not in line:
                continue
            try:
                ln = json.loads(line)
            except ValueError:
                continue
            s = next((x for x in ln.get("s", []) if x.get("peer") == "bob"), None)
            if not s or not s.get("rec"):
                continue
            out.append((ln["t"], s.get("st"), s.get("pr"), s["rec"]["txq"], s["rec"]["un"],
                        s.get("txd")))
    return out


def sends_info(run_dir):
    """Uygulama gönderim gecikmesi (çağrı − vade) ve geri basınç denemeleri, sınıf başına en çok."""
    delay, tries = {}, {}
    try:
        f = open(os.path.join(run_dir, "alice.jsonl"), encoding="utf-8")
    except OSError:
        return delay, tries
    with f:
        for line in f:
            if '"k":"send"' not in line:
                continue
            ln = json.loads(line)
            c = ln["c"]
            delay[c] = max(delay.get(c, 0.0), ln["t"] - ln["due"])
            tries[c] = max(tries.get(c, 0), ln.get("try", 0))
    return delay, tries


def run_row(rj, items_path):
    a = rj.get("alice_summary") or {}
    b = rj.get("bob_summary") or {}
    t0 = rj.get("t0")
    ended = a.get("ended")
    items = []
    if os.path.exists(items_path):
        with open(items_path, encoding="utf-8") as f:
            items = [json.loads(x) for x in f]
    row = {"cell": rj["cell"], "arm": rj["arm"], "rep": rj["rep"], "scenario": rj.get("scenario"),
           "profile": rj.get("profile"), "rc": rj.get("rc"), "alice_result": a.get("result"),
           "bob_result": b.get("result"), "valid_tool": rj.get("valid")}
    lf, lr = realized_loss(rj["dir"])
    row["loss_fwd"], row["loss_rev"] = lf, lr
    for c in CLASSES:
        its = [it for it in items if it["c"] == c]
        if not its:
            continue
        tots, cens = [], 0
        for it in its:
            if it.get("total") is not None:
                tots.append(it["total"])
            else:
                cens += 1
                lb = (ended - it["t_send"]) if ended else None
                tots.append(lb if lb is not None else float("inf"))
        row[f"{c}_n"] = len(its)
        row[f"{c}_undelivered"] = cens
        row[f"{c}_p50"] = rank(tots, .5)
        row[f"{c}_p95"] = rank(tots, .95)
        for l in LAYERS:
            vals = [it[l] for it in its if it.get(l) is not None]
            row[f"{c}_{l}_p50"] = rank(vals, .5)
            row[f"{c}_{l}_p95"] = rank(vals, .95)
        if c in ("P", "KP"):
            row[f"{c}_in800_pct"] = 100.0 * sum(1 for t in tots if t <= 800) / len(tots)
            row[f"{c}_in400_pct"] = 100.0 * sum(1 for t in tots if t <= 400) / len(tots)
        so = [it.get("stall_overlap") for it in its if it.get("stall_overlap") is not None]
        row[f"{c}_stall_p95"] = rank(so, .95) if so else None
    row["RC_p50"] = (rj.get("classes", {}).get("RC") or {}).get("latency_ms", {}).get("p50")
    c = rj.get("counters", {})
    for k in ("bob_delivery_channel_drops", "bob_not_ready_drops", "units_never_drained",
              "units_skipped_below_next", "bob_held_max", "alice_units_sent", "first_clamped"):
        row[k] = c.get(k)
    row["pacer_p50"] = (c.get("alice_pacer_bps") or {}).get("p50")
    link = link_of(rj.get("profile") or "")
    row["pacer_p50_pct_link"] = (100.0 * row["pacer_p50"] / link) if (link and row["pacer_p50"]) else None
    f = rj.get("file") or {}
    row["file_bob_status"] = f.get("bob_status")
    row["file_bob_ms"] = f.get("bob_received_ms")
    row["file_alice_receipt_timeout"] = bool(f.get("why") and "ReceiptTimeout" in f["why"])
    row["file_alice_failed_ms"] = f.get("alice_failed_ms")
    kt = rj.get("karsi_trigger")
    row["karsi_rel_ms"] = kt.get("rel_ms") if kt else None
    ser = alice_series(rj["dir"], t0)
    if ser:
        last = ser[-1]
        row["end_state"], row["end_pacer"], row["end_txq"], row["end_un"] = last[1], last[2], last[3], last[4]
        last_tx = max((t2 for (t1, *_r1), (t2, *_r2) in zip(ser, ser[1:]) if _r2[4] != _r1[4]),
                      default=None)
        row["silent_tail_s"] = (last[0] - last_tx) / 1000 if last_tx else None
        prs = [x[2] for x in ser if t0 and x[0] - t0 > 20_000 and x[2] is not None]
        row["pacer_min_after20"] = min(prs) if prs else None
    incomplete = a.get("result") != "complete"
    row["tail_stuck"] = bool(ser and incomplete and row.get("end_un") is not None and
                             row["end_un"] <= 2 and row["end_txq"] == 0 and
                             (row.get("silent_tail_s") or 0) >= 120)
    row["rate_collapse"] = bool(ser and link and row.get("end_txq", 0) >= 50 and
                                row.get("end_pacer") is not None and
                                row["end_pacer"] < 0.05 * link)
    d, t = sends_info(rj["dir"])
    row["send_delay_T_max_s"] = d.get("T", 0.0) / 1000 if d else None
    row["send_tries_T_max"] = t.get("T") if t else None
    row["listener_drops"] = (a.get("listener_drops") or 0) + (b.get("listener_drops") or 0)
    closes = 0
    for who in ("alice.jsonl", "bob.jsonl"):
        try:
            with open(os.path.join(rj["dir"], who), encoding="utf-8") as f:
                closes += sum(1 for line in f if '"e":"closed"' in line)
        except OSError:
            pass
    row["session_closed_events"] = closes
    return row


def boot_diff(xs, ys, rng):
    xs = [x for x in xs if x is not None]
    ys = [y for y in ys if y is not None]
    if len(xs) < 3 or len(ys) < 3:
        return None
    est = rank(xs, .5) - rank(ys, .5)
    if math.isinf(est) or math.isnan(est):
        return None
    ds = []
    for _ in range(BOOT):
        bx = [xs[rng.randrange(len(xs))] for _ in xs]
        by = [ys[rng.randrange(len(ys))] for _ in ys]
        ds.append(rank(bx, .5) - rank(by, .5))
    ds = [d for d in ds if not (math.isinf(d) or math.isnan(d))]
    ds.sort()
    return est, ds[int(0.025 * len(ds))], ds[min(len(ds) - 1, int(0.975 * len(ds)))]


def fisher_two_sided(a, n1, b, n2):
    """2×2 kesin test (iki yönlü, olasılığı gözlenenden büyük olmayan tablolar)."""
    k, n = a + b, n1 + n2
    def pr(x):
        return math.comb(n1, x) * math.comb(n2, k - x) / math.comb(n, k)
    p0 = pr(a)
    lo, hi = max(0, k - n2), min(k, n1)
    return min(1.0, sum(pr(x) for x in range(lo, hi + 1) if pr(x) <= p0 * (1 + 1e-9)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--analiz", required=True)
    ap.add_argument("--csv", required=True)
    a = ap.parse_args()
    os.makedirs(a.csv, exist_ok=True)
    rows = []
    for p in sorted(glob.glob(os.path.join(a.analiz, "runs", "*.json"))):
        rj = json.load(open(p, encoding="utf-8"))
        rows.append(run_row(rj, p.replace(os.sep + "runs" + os.sep, os.sep + "items" + os.sep)
                            .replace(".json", ".jsonl")))
    keys = sorted({k for r in rows for k in r})
    first = ["cell", "arm", "rep", "scenario", "profile", "rc", "alice_result", "bob_result"]
    keys = first + [k for k in keys if k not in first]
    with open(os.path.join(a.csv, "kosular.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        for r in sorted(rows, key=lambda r: (r["cell"], r["arm"], r["rep"])):
            w.writerow({k: ("" if r.get(k) is None else r.get(k)) for k in keys})
    cells = {}
    for r in rows:
        cells.setdefault((r["cell"], r["arm"]), []).append(r)
    ordered = sorted(cells, key=lambda k: (k[0], 0 if k[1] == "taban" else 1))
    P = print
    # 1) Geçerlilik
    P("### T.1 Geçerlilik\n")
    P("| hücre | kol | N | rc | alice sonuç (tamam/eksik/yok) | ileri kayıp % | dönüş kayıp % | dinleyici/iz düşüşü |")
    P("|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        rc = {}
        for r in rs:
            rc[r["rc"]] = rc.get(r["rc"], 0) + 1
        res = (sum(1 for r in rs if r["alice_result"] == "complete"),
               sum(1 for r in rs if r["alice_result"] == "incomplete"),
               sum(1 for r in rs if r["alice_result"] is None))
        P(f"| {k[0]} | {k[1]} | {len(rs)} | {', '.join(f'{c}×{n}' for c, n in sorted(rc.items(), key=str))} | "
          f"{res[0]}/{res[1]}/{res[2]} | {fs(spread([r['loss_fwd'] for r in rs]), 0.01)} | "
          f"{fs(spread([r['loss_rev'] for r in rs]), 0.01)} | {sum(r['listener_drops'] for r in rs)} |")
    # 2) Gecikme + katmanlar
    P("\n### T.2 Teslim gecikmesi ve katman kırılımı (s; koşular arası medyan [Q1–Q3] (min–maks))\n")
    for cl in ("T", "P", "S"):
        P(f"\n**Sınıf {cl}**\n")
        P("| hücre | kol | N | teslim edilmeyen (koşu medyanı) | p50 | p95 | p50: fifo / net / reorder | "
          "p95: fifo / net / reorder |")
        P("|---|---|---|---|---|---|---|---|")
        for k in ordered:
            rs = [r for r in cells[k] if r.get(f"{cl}_n")]
            if not rs:
                continue
            med = lambda key: rank([r[key] for r in rs if r.get(key) is not None], .5)
            l50 = " / ".join("—" if med(f"{cl}_{l}_p50") is None else f"{med(f'{cl}_{l}_p50') / 1000:.1f}"
                             for l in ("fifo", "net", "reorder"))
            l95 = " / ".join("—" if med(f"{cl}_{l}_p95") is None else f"{med(f'{cl}_{l}_p95') / 1000:.1f}"
                             for l in ("fifo", "net", "reorder"))
            P(f"| {k[0]} | {k[1]} | {len(rs)} | {rank([r[f'{cl}_undelivered'] for r in rs], .5)} | "
              f"{fs(spread([r[f'{cl}_p50'] for r in rs]), 1000)} | {fs(spread([r[f'{cl}_p95'] for r in rs]), 1000)} | "
              f"{l50} | {l95} |")
    # 3) Farklar
    rng = random.Random(SEED)
    P("\n### T.3 Taban − aday farkı (medyan farkı, s; %95 aralığı koşu birimli bootstrap B=10 000)\n")
    P("| hücre | ölçü | taban − aday | %95 aralığı |")
    P("|---|---|---|---|")
    drows = []
    for cell in sorted({k[0] for k in cells}):
        tb, ad = cells.get((cell, "taban"), []), cells.get((cell, "aday"), [])
        for metric in ("T_p50", "T_p95", "T_reorder_p95", "T_fifo_p95", "P_p50", "P_p95",
                       "P_reorder_p95", "K_p50", "KP_p50"):
            r = boot_diff([x.get(metric) for x in tb], [x.get(metric) for x in ad], rng)
            if r is None:
                continue
            est, lo, hi = r
            drows.append({"cell": cell, "metric": metric, "diff_ms": est, "lo_ms": lo, "hi_ms": hi,
                          "n_taban": len(tb), "n_aday": len(ad)})
            P(f"| {cell} | {metric} | {est / 1000:+.1f} | [{lo / 1000:+.1f}, {hi / 1000:+.1f}] |")
    with open(os.path.join(a.csv, "farklar.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=["cell", "metric", "diff_ms", "lo_ms", "hi_ms", "n_taban", "n_aday"])
        w.writeheader()
        w.writerows(drows)
    # 4) Karşı örnek
    P("\n### T.4 Karşı örnek (gönderen yeni veri kuyruğu boş + eski dosya parçalarının kurtarması canlı)\n")
    P("| hücre | kol | ateşlendi / N | tetik anı (s) | K TEXT p50 | K TEXT reorder p50 | K TEXT fifo p50 | KP PTT p50 | KP reorder p50 |")
    P("|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        if not any(r["scenario"] and "dosyali" in r["scenario"] for r in rs):
            continue
        fired = [r for r in rs if r.get("karsi_rel_ms") is not None and r.get("K_n")]
        P(f"| {k[0]} | {k[1]} | {len(fired)}/{len(rs)} | {fs(spread([r['karsi_rel_ms'] for r in fired]), 1000)} | "
          f"{fs(spread([r.get('K_p50') for r in fired]), 1000)} | {fs(spread([r.get('K_reorder_p50') for r in fired]), 1000)} | "
          f"{fs(spread([r.get('K_fifo_p50') for r in fired]), 1000, 2)} | {fs(spread([r.get('KP_p50') for r in fired]), 1000)} | "
          f"{fs(spread([r.get('KP_reorder_p50') for r in fired]), 1000)} |")
    # 5) ACK'li düşüş
    P("\n### T.5 Teslim kanalı taşması (`delivery_channel_drops`) ve sonrası\n")
    P("| hücre | kol | N | düşüşlü koşu | düşüş (düşüşlü koşularda medyan, min–maks) | hiç teslim edilmeyen birim | "
      "atlanan birim | TEXT teslim edilmeyen (düşüşlü koşularda medyan) | bob dosya: alındı/hata |")
    P("|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        dr = [r for r in rs if (r.get("bob_delivery_channel_drops") or 0) > 0]
        P(f"| {k[0]} | {k[1]} | {len(rs)} | {len(dr)} | "
          f"{'—' if not dr else fs(spread([r['bob_delivery_channel_drops'] for r in dr]), 1, 0)} | "
          f"{'—' if not dr else fs(spread([r['units_never_drained'] for r in dr]), 1, 0)} | "
          f"{'—' if not dr else fs(spread([r['units_skipped_below_next'] for r in dr]), 1, 0)} | "
          f"{'—' if not dr else rank([r.get('T_undelivered') for r in dr if r.get('T_undelivered') is not None], .5)} | "
          f"{sum(1 for r in rs if r['file_bob_status'] == 'received')}/{sum(1 for r in rs if r['file_bob_status'] == 'failed')} |")
    # 6) A-7, A-9, A-10, A-11
    P("\n### T.6 A-7 / A-8 / A-9 / A-10 / A-11 sayıları\n")
    P("| hücre | kol | N | A-7 alice ReceiptTimeout | A-8 bob IdleTimeout | bob dosya alındı | A-9 pacer p50 (% hat) | "
      "A-10 hız çöküşü | A-11 kuyruk sonu takılması | TEXT gönderim gecikmesi en çok (s, medyan) |")
    P("|---|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        filed = [r for r in rs if r["file_bob_status"] is not None]
        send_delay = rank([r.get("send_delay_T_max_s") or 0.0 for r in rs], .5) or 0.0
        P(f"| {k[0]} | {k[1]} | {len(rs)} | "
          f"{sum(1 for r in rs if r['file_alice_receipt_timeout'])}/{len(filed) if filed else '—'} | "
          f"{sum(1 for r in rs if r['file_bob_status'] == 'failed')} | {sum(1 for r in rs if r['file_bob_status'] == 'received')} | "
          f"{fs(spread([r['pacer_p50_pct_link'] for r in rs]), 1, 1)} | "
          f"{sum(1 for r in rs if r['rate_collapse'])} | {sum(1 for r in rs if r['tail_stuck'])} | "
          f"{send_delay:.1f} |")
    # 7) Senaryo karşıtları (kol içi): dosya etkisi, durma etkisi
    P("\n### T.7 Kol içi senaryo farkı (medyan farkı, s; %95 aralığı koşu birimli bootstrap B=10 000)\n")
    P("| profil | kol | karşıt | ölçü | fark | %95 aralığı |")
    P("|---|---|---|---|---|---|")
    pairs = (("dosyali", "dosyasiz"), ("dosyali_dur", "dosyali"), ("dosyasiz_dur", "dosyasiz"),
             ("dosyali_dur", "dosyasiz"))
    for prof in ("r12k_d300_l30", "r1k_d300_l30", "r12k_d1000_l30"):
        for arm in ("taban", "aday"):
            for x, y in pairs:
                cx, cy = cells.get((f"a_{prof}_{x}", arm)), cells.get((f"a_{prof}_{y}", arm))
                if not cx or not cy:
                    continue
                for metric in ("T_p95", "P_p95", "T_p50"):
                    r = boot_diff([q.get(metric) for q in cx], [q.get(metric) for q in cy], rng)
                    if r is None:
                        continue
                    est, lo, hi = r
                    P(f"| {prof} | {arm} | {x} − {y} | {metric} | {est / 1000:+.1f} | [{lo / 1000:+.1f}, {hi / 1000:+.1f}] |")
    # 8) Sayım karşıtları taban ↔ aday (Fisher kesin, iki yönlü)
    P("\n### T.8 Sayım farkları taban ↔ aday (Fisher kesin testi, iki yönlü)\n")
    P("| küme | ölçü | taban | aday | p |")
    P("|---|---|---|---|---|")
    groups = {
        "r12k_d300 dosyalı (dosyali + dosyali_dur)": ("a_r12k_d300_l30_dosyali", "a_r12k_d300_l30_dosyali_dur"),
        "r1k_d300 dosyalı (dosyali + dosyali_dur)": ("a_r1k_d300_l30_dosyali", "a_r1k_d300_l30_dosyali_dur"),
        "bütün hücreler": tuple(sorted({k[0] for k in cells})),
    }
    for gname, cnames in groups.items():
        tb = [r for c in cnames for r in cells.get((c, "taban"), [])]
        ad = [r for c in cnames for r in cells.get((c, "aday"), [])]
        for mname, fn in (("A-8 bob IdleTimeout", lambda r: r["file_bob_status"] == "failed"),
                          ("A-11 kuyruk sonu takılması", lambda r: r["tail_stuck"]),
                          ("A-10 hız çöküşü", lambda r: r["rate_collapse"]),
                          ("teslim kanalı düşüşlü koşu", lambda r: (r.get("bob_delivery_channel_drops") or 0) > 0),
                          ("bitmemiş koşu", lambda r: r["alice_result"] != "complete")):
            a1, b1 = sum(1 for r in tb if fn(r)), sum(1 for r in ad if fn(r))
            P(f"| {gname} | {mname} | {a1}/{len(tb)} | {b1}/{len(ad)} | {fisher_two_sided(a1, len(tb), b1, len(ad)):.3g} |")
    P(f"\nOturum kapanma olayı (Closed, iki uç toplamı, bütün koşular): {sum(r['session_closed_events'] for r in rows)}")
    # hücre özeti CSV
    hk = ["cell", "arm", "n", "complete", "T_p50_med", "T_p95_med", "P_p50_med", "P_p95_med",
          "T_reorder_p95_med", "T_fifo_p95_med", "dcd_runs", "tail_stuck", "rate_collapse",
          "receipt_timeout", "bob_idle_timeout", "pacer_pct_link_med"]
    with open(os.path.join(a.csv, "hucreler.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=hk)
        w.writeheader()
        for k in ordered:
            rs = cells[k]
            m = lambda key: rank([r[key] for r in rs if r.get(key) is not None], .5)
            w.writerow({"cell": k[0], "arm": k[1], "n": len(rs),
                        "complete": sum(1 for r in rs if r["alice_result"] == "complete"),
                        "T_p50_med": m("T_p50"), "T_p95_med": m("T_p95"), "P_p50_med": m("P_p50"),
                        "P_p95_med": m("P_p95"), "T_reorder_p95_med": m("T_reorder_p95"),
                        "T_fifo_p95_med": m("T_fifo_p95"),
                        "dcd_runs": sum(1 for r in rs if (r.get("bob_delivery_channel_drops") or 0) > 0),
                        "tail_stuck": sum(1 for r in rs if r["tail_stuck"]),
                        "rate_collapse": sum(1 for r in rs if r["rate_collapse"]),
                        "receipt_timeout": sum(1 for r in rs if r["file_alice_receipt_timeout"]),
                        "bob_idle_timeout": sum(1 for r in rs if r["file_bob_status"] == "failed"),
                        "pacer_pct_link_med": m("pacer_p50_pct_link")})
    print(f"\n<!-- {len(rows)} koşu -->", file=sys.stderr)


if __name__ == "__main__":
    main()
