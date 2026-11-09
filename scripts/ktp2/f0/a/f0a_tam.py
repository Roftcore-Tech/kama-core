#!/usr/bin/env python3
"""[KTP2-F0 · A] Tam set özeti — `f0a_analyze.py` çıktısı + ham koşu dizinlerinden TAM SONUÇ tabloları.

    python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <A> <ham kök>
    python3 -I scripts/ktp2/f0/a/f0a_tam.py --analiz <A> --csv reports/ktp2/f0/a/tam > <tablolar.md>

Yalnız standart kütüphane; ham veriye yazmaz. Çıktı:
  <csv>/kosular.csv    koşu başına satır (geçerlilik, gecikme, katmanlar, sayaçlar)
  <csv>/hucreler.csv   hücre × kol özeti
  <csv>/farklar.csv    taban − aday farkının %95 aralığı (koşu birimli bootstrap); n = None
                       olmayan koşu sayısı
  stdout               RAPOR-A "TAM SONUÇ" tabloları (Markdown, T.1–T.15)

Tanımlar:
- Öğe gecikmesi = bob uygulama teslimi − alice uygulama çağrısı. Teslim edilmeyen öğe, gözlem
  sonundan (alice'in bitişi) bir ALT SINIRLA sayılır (sansürlü) ve ayrıca sayılır.
- Koşu başına p50/p95 en yakın sıra; hücre değeri koşular arası medyan [Q1–Q3] (min–maks).
  Medyan/Q1/Q3 de en yakın sıradır (⌈p·n⌉'inci değer; çift N'de medyan alt orta değer).
- Katmanlar (`f0a_analyze.py` belgesi) [türetilmiş]: fifo = kabul → motor ilk iletimi (offer
  kanalı + depo + tx_queue + pacing; motorun yeniden gönderim payı dahil); net = ilk iletim →
  alıcı adaptörün teslim kanalından çekmesi (ağ + birimin kendi kayıp kurtarması + runtime teslim
  kanalında bekleme + alıcı düğüm iş parçacığı blokajı; *_dur hücrelerinde 30 s durma buradadır);
  reorder = alıcı oturum geneli sıralama (başka birimleri bekleme); consumer = sıralı teslim →
  uygulama. Katman p50/p95 yalnız teslim edilen öğeler üzerinden (toplam p95 ise sansürlü alt
  sınırları içerir; ikisi aynı öğe kümesinden gelmez, toplanmaz).
- Fark aralığı: taban ve aday koşu kümelerinden ayrı ayrı yerine koyarak örnekleme (B = 10 000,
  tohum 20261030), istatistik = medyan(taban) − medyan(aday), %2,5–%97,5 yüzdelikleri.
- Karşı örnek (T.4): tetik = dosya gönderildi, bob dosyayı henüz almadı, gönderen yeni veri
  kuyruğu 0 ve teyitsiz birim ≥ 3 (f0_flows). "Kurtarma canlı" = tetik anında motorun
  `pending_lost` > 0 (ISEMRI koşulu); pl = 0 koşular ayrı satırdadır.
- Motor yeniden gönderim payı (T.14): alice ardışık ts örnekleri arasında, önceki örnekte yeni
  veri kuyruğu > 0 iken (dosyalı hücrelerde dosya gönderiminden sonra) Σ ΔARQ-yeniden-gönderim
  baytı / Σ Δtoplam tel baytı (`rtxb` / `txb`; kopya ve FEC baytı paya girmez).
- İlk engelleyici (T.10): reorder ≥ 1 s bekleyen TEXT öğesinin, kanaldan çekildiği anda Reorder'ın
  beklediği birimin sınıfı; pay = bekleme süresiyle ağırlıklı, hücredeki öğeler havuzlu.
- Kuyruk sonu takılması (A-11): koşu bitmemiş, alice'te teyitsiz birim ≤ 2, yeni veri kuyruğu 0
  ve son ≥ 120 s'de hiç datagram gönderilmemiş.
- Hız çöküşü (A-10): koşunun sonunda yeni veri kuyruğu ≥ 50 birimken pacer < hattın %5'i.
- IdleTimeout penceresi (T.11): bob `file_failed` anından önceki 60 s.
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
# K-1 eşiği (f0a_analyze.py ile aynı): en büyük tel paketi = 1013 + 11 + 22 + 42 B.
MAX_PACKET_LINE_BYTES = 1088
FILE_CHUNK_UNIT_BYTES = 1013
IDLE_WINDOW_MS = 60_000
BLOCK_MIN_MS = 1000
BLK_GROUPS = {"F": "F", "X": "X", "T": "T", "W": "T", "K": "T", "P": "P", "KP": "P", "O": "P",
              "C": "P", "S": "S"}
BLK_ORDER = ("F", "X", "T", "P", "S")
PRIMARY = ("dosyasiz", "dosyali", "dosyali_dur")


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


def rng_str(vals, nd=0):
    v = [x for x in vals if x is not None]
    if not v:
        return "—"
    return f"{min(v):.{nd}f}–{max(v):.{nd}f}"


def link_of(profile):
    for k, v in LINK.items():
        if profile.startswith(k + "_"):
            return v
    return None


def delay_of(profile):
    for k, v in DELAY.items():
        if f"_{k}_" in f"_{profile}_":
            return v
    return None


def netem_counters(run_dir, name):
    try:
        x = json.load(open(os.path.join(run_dir, name)))
    except (OSError, ValueError):
        return None
    return {q["dev"]: q for q in x.get("mid_qdisc", []) if q.get("kind") == "netem"}


def realized_loss(run_dir):
    q = netem_counters(run_dir, "realized_post.json")
    if q is None:
        return None, None
    def loss(dev):
        x = q.get(dev)
        if not x or (x["packets"] + x["drops"]) == 0:
            return None
        return x["drops"] / (x["packets"] + x["drops"])
    return loss("mb0"), loss("ma0")


def pre_nonzero(run_dir):
    q = netem_counters(run_dir, "realized_pre.json")
    if q is None:
        return None
    return any((x.get("packets") or 0) or (x.get("drops") or 0) for x in q.values())


def read_lines(run_dir, who):
    try:
        with open(os.path.join(run_dir, who), encoding="utf-8") as f:
            return [json.loads(line) for line in f if line.strip()]
    except (OSError, ValueError):
        return []


def alice_series(alice):
    """alice ts örnekleri (bob oturumu)."""
    out = []
    for ln in alice:
        if ln.get("k") != "ts":
            continue
        s = next((x for x in ln.get("s", []) if x.get("peer") == "bob"), None)
        if not s or not s.get("rec"):
            continue
        r = s["rec"]
        out.append({"t": ln["t"], "st": s.get("st"), "pr": s.get("pr"), "txq": r["txq"],
                    "un": r["un"], "pl": r["pl"], "inf": r["inf"], "txd": s.get("txd"),
                    "rxd": s.get("rxd"), "txb": s.get("txb"), "rtxb": s.get("rtxb"),
                    "tlp": s.get("tlp")})
    return out


def bob_series(bob):
    """bob ts örnekleri (alice oturumu): Reorder next / tutulan."""
    out = []
    for ln in bob:
        if ln.get("k") != "ts":
            continue
        s = next((x for x in ln.get("s", []) if x.get("peer") == "alice"), None)
        if s and s.get("reo"):
            out.append({"t": ln["t"], "n": s["reo"]["n"], "h": s["reo"]["h"]})
    return out


def rtx_share(ser, t_from):
    num = den = 0
    for a, b in zip(ser, ser[1:]):
        if a["t"] < t_from or a["txq"] <= 0:
            continue
        if None in (a["txb"], b["txb"], a["rtxb"], b["rtxb"]):
            continue
        dt, dr = b["txb"] - a["txb"], b["rtxb"] - a["rtxb"]
        if dt <= 0 or dr < 0:
            continue
        num += dr
        den += dt
    return num / den if den else None


def sends_info(alice):
    """Uygulama gönderim gecikmesi (çağrı − vade) ve geri basınç denemeleri, sınıf başına en çok."""
    delay, tries = {}, {}
    for ln in alice:
        if ln.get("k") != "send":
            continue
        c = ln["c"]
        delay[c] = max(delay.get(c, 0.0), ln["t"] - ln["due"])
        tries[c] = max(tries.get(c, 0), ln.get("try", 0))
    return delay, tries


def frac(xs, fn):
    return (sum(1 for x in xs if fn(x)) / len(xs)) if xs else None


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
    row["pre_netem_nonzero"] = pre_nonzero(rj["dir"])
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
    # İlk engelleyici: reorder ≥ 1 s bekleyen TEXT öğeleri, bekleme ağırlıklı.
    for g in BLK_ORDER:
        row[f"T_blk_{g}_ms"] = 0.0
    row["T_blk_items"] = 0
    for it in items:
        if it["c"] == "T" and (it.get("reorder") or 0) >= BLOCK_MIN_MS and it.get("blk_c"):
            row[f"T_blk_{BLK_GROUPS.get(it['blk_c'], 'X')}_ms"] += it["reorder"]
            row["T_blk_items"] += 1
    ks = [it for it in items if it["c"] == "K"]
    row["K_blk_F"] = sum(1 for it in ks if it.get("blk_c") == "F") if ks else None
    row["RC_p50"] = (rj.get("classes", {}).get("RC") or {}).get("latency_ms", {}).get("p50")
    c = rj.get("counters", {})
    for k in ("bob_delivery_channel_drops", "bob_not_ready_drops", "units_never_drained",
              "units_skipped_below_next", "bob_held_max", "alice_units_sent", "first_clamped"):
        row[k] = c.get(k)
    row["receipts"], row["texts_sent"] = a.get("receipts"), a.get("texts_sent")
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
    ks_ = (kt or {}).get("session") or {}
    row["karsi_pl"] = (ks_.get("rec") or {}).get("pl") if kt else None
    row["karsi_st"] = ks_.get("st") if kt else None
    alice = read_lines(rj["dir"], "alice.jsonl")
    bob = read_lines(rj["dir"], "bob.jsonl")
    ser = alice_series(alice)
    if ser:
        last = ser[-1]
        row["end_state"], row["end_pacer"], row["end_txq"], row["end_un"] = \
            last["st"], last["pr"], last["txq"], last["un"]
        row["end_inf"] = last["inf"]
        last_i = max((i + 1 for i, (p, q) in enumerate(zip(ser, ser[1:])) if q["txd"] != p["txd"]),
                     default=None)
        row["silent_tail_s"] = (last["t"] - ser[last_i]["t"]) / 1000 if last_i else None
        prs = [x["pr"] for x in ser if t0 and x["t"] - t0 > 20_000 and x["pr"] is not None]
        row["pacer_min_after20"] = min(prs) if prs else None
        t_from = (t0 + f["sent_rel_ms"]) if (t0 and f.get("sent_rel_ms") is not None) else (t0 or 0)
        row["rtx_share_txq"] = rtx_share(ser, t_from)
        row["rtx_share_all"] = (last["rtxb"] / last["txb"]) if last.get("txb") else None
    incomplete = a.get("result") != "complete"
    row["tail_stuck"] = bool(ser and incomplete and row.get("end_un") is not None and
                             row["end_un"] <= 2 and row["end_txq"] == 0 and
                             (row.get("silent_tail_s") or 0) >= 120)
    if row["tail_stuck"] and last_i:
        lt = ser[last_i]
        row["ts_last_tx_rel_s"] = (lt["t"] - t0) / 1000 if t0 else None
        row["ts_state_at_last_tx"] = lt["st"]
        row["ts_un_at_last_tx"] = lt["un"]
        rf = next((x for x in ser[last_i:] if x["st"] == "RecoverFwd"), None)
        row["ts_recoverfwd_after_s"] = (rf["t"] - lt["t"]) / 1000 if rf else None
        row["ts_rxd_after"] = (last["rxd"] - lt["rxd"]) if None not in (last["rxd"], lt["rxd"]) else None
        row["ts_tlp_end"] = last["tlp"]
    row["rate_collapse"] = bool(ser and link and row.get("end_txq", 0) >= 50 and
                                row.get("end_pacer") is not None and
                                row["end_pacer"] < 0.05 * link)
    # bob IdleTimeout öncesi 60 s penceresi.
    tf = next((ln["t"] for ln in bob if ln.get("k") == "ev" and ln.get("e") == "file_failed"), None)
    if tf is not None:
        bw = [x for x in bob_series(bob) if tf - IDLE_WINDOW_MS <= x["t"] <= tf]
        aw = [x for x in ser if tf - IDLE_WINDOW_MS <= x["t"] <= tf]
        row["idle_next_const"] = (len({x["n"] for x in bw}) == 1) if bw else None
        row["idle_held_min"] = min(x["h"] for x in bw) if bw else None
        row["idle_txq_pos_frac"] = frac(aw, lambda x: x["txq"] > 0)
        row["idle_pl_pos_frac"] = frac(aw, lambda x: x["pl"] > 0)
        row["idle_arq_frac"] = frac(aw, lambda x: x["st"] == "RecoverArq")
        st = rj.get("stall") or {}
        row["idle_stall_inside"] = bool(st.get("start") and st.get("end") and
                                        st["start"] >= tf - IDLE_WINDOW_MS and st["end"] <= tf)
    d, t = sends_info(alice)
    row["send_delay_T_max_s"] = d.get("T", 0.0) / 1000 if d else None
    row["send_tries_T_max"] = t.get("T") if t else None
    row["listener_drops"] = (a.get("listener_drops") or 0) + (b.get("listener_drops") or 0)
    row["session_closed_events"] = sum(1 for ln in alice + bob
                                       if ln.get("k") == "ev" and ln.get("e") == "closed")
    row["session_connected_events"] = sum(1 for ln in alice + bob
                                          if ln.get("k") == "ev" and ln.get("e") == "connected")
    # Dosya parçalarının Transport'a kabulü (alice iz `S`, 1013 B birim).
    if t0 and f.get("sent_rel_ms") is not None:
        ft = t0 + f["sent_rel_ms"]
        acc = [r[4] for ln in alice if ln.get("k") == "tr" for r in ln.get("r", [])
               if len(r) == 5 and r[0] == "S" and r[3] == FILE_CHUNK_UNIT_BYTES and r[4] >= ft]
        row["file_chunks_accepted"] = len(acc)
        row["file_accept_span_s"] = (max(acc) - ft) / 1000 if acc else None
        fe = f.get("fifo_empty_rel_ms")
        row["file_fifo_empty_s"] = fe / 1000 if fe is not None else None
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
    P("| hücre | kol | N | rc | alice sonuç (tamam/eksik/yok) | ileri kayıp % | dönüş kayıp % | "
      "dinleyici/iz düşüşü | pre netem sayacı ≠ 0 |")
    P("|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        rc = {}
        for r in rs:
            rc[r["rc"]] = rc.get(r["rc"], 0) + 1
        pre = ", ".join("rep%d" % r["rep"] for r in sorted(rs, key=lambda r: r["rep"])
                        if r["pre_netem_nonzero"]) or "0"
        res = (sum(1 for r in rs if r["alice_result"] == "complete"),
               sum(1 for r in rs if r["alice_result"] == "incomplete"),
               sum(1 for r in rs if r["alice_result"] is None))
        P(f"| {k[0]} | {k[1]} | {len(rs)} | {', '.join(f'{c}×{n}' for c, n in sorted(rc.items(), key=str))} | "
          f"{res[0]}/{res[1]}/{res[2]} | {fs(spread([r['loss_fwd'] for r in rs]), 0.01)} | "
          f"{fs(spread([r['loss_rev'] for r in rs]), 0.01)} | {sum(r['listener_drops'] for r in rs)} | "
          f"{pre} |")
    # 2) Gecikme + katmanlar
    P("\n### T.2 Teslim gecikmesi ve katman kırılımı (s; koşular arası medyan [Q1–Q3] (min–maks))\n")
    P("Katman sütunları yalnız teslim edilen öğelerden; 'katman N' = katman p95'i tanımlı koşu sayısı.")
    for cl in ("T", "P", "S"):
        P(f"\n**Sınıf {cl}**\n")
        P("| hücre | kol | N | katman N | teslim edilmeyen (koşu medyanı) | p50 | p95 | p50: fifo / net / reorder | "
          "p95: fifo / net / reorder |")
        P("|---|---|---|---|---|---|---|---|---|")
        for k in ordered:
            rs = [r for r in cells[k] if r.get(f"{cl}_n")]
            if not rs:
                continue
            med = lambda key: rank([r[key] for r in rs if r.get(key) is not None], .5)
            l50 = " / ".join("—" if med(f"{cl}_{l}_p50") is None else f"{med(f'{cl}_{l}_p50') / 1000:.1f}"
                             for l in ("fifo", "net", "reorder"))
            l95 = " / ".join("—" if med(f"{cl}_{l}_p95") is None else f"{med(f'{cl}_{l}_p95') / 1000:.1f}"
                             for l in ("fifo", "net", "reorder"))
            ln = sum(1 for r in rs if r.get(f"{cl}_reorder_p95") is not None)
            P(f"| {k[0]} | {k[1]} | {len(rs)} | {ln} | {rank([r[f'{cl}_undelivered'] for r in rs], .5)} | "
              f"{fs(spread([r[f'{cl}_p50'] for r in rs]), 1000)} | {fs(spread([r[f'{cl}_p95'] for r in rs]), 1000)} | "
              f"{l50} | {l95} |")
    # 3) Farklar
    rng = random.Random(SEED)
    P("\n### T.3 Taban − aday farkı (medyan farkı, s; %95 aralığı koşu birimli bootstrap B=10 000)\n")
    P("| hücre | ölçü | n taban / aday | taban − aday | %95 aralığı |")
    P("|---|---|---|---|---|")
    drows = []
    for cell in sorted({k[0] for k in cells}):
        tb, ad = cells.get((cell, "taban"), []), cells.get((cell, "aday"), [])
        for metric in ("T_p50", "T_p95", "T_reorder_p95", "T_fifo_p95", "P_p50", "P_p95",
                       "P_reorder_p95", "K_p50", "KP_p50"):
            r = boot_diff([x.get(metric) for x in tb], [x.get(metric) for x in ad], rng)
            if r is None:
                continue
            est, lo, hi = r
            nt = sum(1 for x in tb if x.get(metric) is not None)
            na = sum(1 for x in ad if x.get(metric) is not None)
            drows.append({"cell": cell, "metric": metric, "diff_ms": est, "lo_ms": lo, "hi_ms": hi,
                          "n_taban": nt, "n_aday": na})
            P(f"| {cell} | {metric} | {nt}/{na} | {est / 1000:+.1f} | [{lo / 1000:+.1f}, {hi / 1000:+.1f}] |")
    with open(os.path.join(a.csv, "farklar.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=["cell", "metric", "diff_ms", "lo_ms", "hi_ms", "n_taban", "n_aday"])
        w.writeheader()
        w.writerows(drows)
    # 4) Karşı örnek
    P("\n### T.4 Karşı örnek — tetik: yeni veri kuyruğu 0 + teyitsiz ≥ 3; 'kurtarma canlı' = tetik "
      "anında pending_lost > 0 [betimsel: ateşlenen alt küme, hücre başına < 30 koşu]\n")
    P("| hücre | kol | ateşlendi / N | pl>0 / pl=0 | tetik anında RecoverArq | pl>0: K TEXT p50 | "
      "pl>0: K reorder p50 | pl>0: KP PTT p50 | pl=0: K TEXT p50 | K öğesi ilk engelleyici dosya parçası |")
    P("|---|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        if not any(r["scenario"] and "dosyali" in r["scenario"] for r in rs):
            continue
        fired = [r for r in rs if r.get("karsi_rel_ms") is not None and r.get("K_n")]
        live = [r for r in fired if (r.get("karsi_pl") or 0) > 0]
        idle = [r for r in fired if (r.get("karsi_pl") or 0) == 0]
        kn = sum(r["K_n"] for r in fired)
        kf = sum(r.get("K_blk_F") or 0 for r in fired)
        P(f"| {k[0]} | {k[1]} | {len(fired)}/{len(rs)} | {len(live)} / {len(idle)} | "
          f"{sum(1 for r in fired if r.get('karsi_st') == 'RecoverArq')} | "
          f"{fs(spread([r.get('K_p50') for r in live]), 1000)} | {fs(spread([r.get('K_reorder_p50') for r in live]), 1000)} | "
          f"{fs(spread([r.get('KP_p50') for r in live]), 1000)} | {fs(spread([r.get('K_p50') for r in idle]), 1000)} | "
          f"{kf}/{kn} |")
    P("\nHavuzlu (dosyali + dosyali_dur), yalnız pl > 0 koşular; fark taban − aday, ayrı tohumlu "
      "bootstrap [betimsel; iki kolda ateşlenme seçimi farklı]:\n")
    P("| profil | n taban / aday | taban K TEXT p50 | aday K TEXT p50 | taban − aday | %95 aralığı |")
    P("|---|---|---|---|---|---|")
    krng = random.Random(SEED)
    for prof in ("r12k_d300_l30", "r1k_d300_l30"):
        sel = {}
        for arm in ("taban", "aday"):
            sel[arm] = [r.get("K_p50") for sc in ("dosyali", "dosyali_dur")
                        for r in cells.get((f"a_{prof}_{sc}", arm), [])
                        if r.get("karsi_rel_ms") is not None and r.get("K_n") and (r.get("karsi_pl") or 0) > 0]
        r = boot_diff(sel["taban"], sel["aday"], krng)
        P(f"| {prof} | {len(sel['taban'])}/{len(sel['aday'])} | {fs(spread(sel['taban']), 1000)} | "
          f"{fs(spread(sel['aday']), 1000)} | "
          f"{'—' if r is None else f'{r[0] / 1000:+.1f}'} | "
          f"{'— (n < 3)' if r is None else f'[{r[1] / 1000:+.1f}, {r[2] / 1000:+.1f}]'} |")
    # 5) ACK'li düşüş
    P("\n### T.5 Teslim kanalı taşması (`delivery_channel_drops`) ve sonrası\n")
    P("| hücre | kol | N | düşüşlü koşu | düşüş (düşüşlü koşularda medyan, min–maks) | hiç teslim edilmeyen birim | "
      "atlanan birim | TEXT teslim edilmeyen (düşüşlü koşular, rep:sayı) | TEXT alındısı (düşüşlü koşular, alındı/gönderilen) | "
      "bob dosya: alındı/hata | bob dosya hata: düşüşlü / düşüşsüz koşu |")
    P("|---|---|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        dr = sorted((r for r in rs if (r.get("bob_delivery_channel_drops") or 0) > 0), key=lambda r: r["rep"])
        nd = [r for r in rs if not (r.get("bob_delivery_channel_drops") or 0) > 0]
        filed = any(r["file_bob_status"] is not None for r in rs)
        fdr = sum(1 for r in dr if r["file_bob_status"] == "failed")
        fnd = sum(1 for r in nd if r["file_bob_status"] == "failed")
        und = ", ".join("%s:%s" % (r["rep"], r.get("T_undelivered")) for r in dr) or "—"
        rcp = ", ".join("%s/%s" % (r.get("receipts"), r.get("texts_sent")) for r in dr) or "—"
        P(f"| {k[0]} | {k[1]} | {len(rs)} | {len(dr)} | "
          f"{'—' if not dr else fs(spread([r['bob_delivery_channel_drops'] for r in dr]), 1, 0)} | "
          f"{'—' if not dr else fs(spread([r['units_never_drained'] for r in dr]), 1, 0)} | "
          f"{'—' if not dr else fs(spread([r['units_skipped_below_next'] for r in dr]), 1, 0)} | "
          f"{und} | {rcp} | "
          f"{sum(1 for r in rs if r['file_bob_status'] == 'received')}/{sum(1 for r in rs if r['file_bob_status'] == 'failed')} | "
          f"{f'{fdr}/{len(dr)} / {fnd}/{len(nd)}' if filed else '—'} |")
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
    P("\n### T.8 Sayım farkları taban ↔ aday (Fisher kesin testi, iki yönlü; çoklu karşılaştırma düzeltmesi YOK)\n")
    P("| küme | ölçü | taban | aday | p |")
    P("|---|---|---|---|---|")
    m_a8 = ("A-8 bob IdleTimeout", lambda r: r["file_bob_status"] == "failed")
    m_all = (m_a8,
             ("A-11 kuyruk sonu takılması", lambda r: r["tail_stuck"]),
             ("A-10 hız çöküşü", lambda r: r["rate_collapse"]),
             ("teslim kanalı düşüşlü koşu", lambda r: (r.get("bob_delivery_channel_drops") or 0) > 0),
             ("bitmemiş koşu", lambda r: r["alice_result"] != "complete"))
    groups = [
        (f"a_{p}_{s}", (f"a_{p}_{s}",), (m_a8,))
        for p in ("r12k_d300_l30", "r1k_d300_l30") for s in ("dosyali", "dosyali_dur")
    ] + [
        ("r12k_d300 dosyalı (dosyali + dosyali_dur)", ("a_r12k_d300_l30_dosyali", "a_r12k_d300_l30_dosyali_dur"), m_all),
        ("r1k_d300 dosyalı (dosyali + dosyali_dur)", ("a_r1k_d300_l30_dosyali", "a_r1k_d300_l30_dosyali_dur"), m_all),
        ("birincil (6 hücre)", tuple(f"a_{p}_{s}" for p in ("r12k_d300_l30", "r1k_d300_l30") for s in PRIMARY), m_all),
        ("bütün hücreler", tuple(sorted({k[0] for k in cells})), m_all),
    ]
    ntests = 0
    for gname, cnames, metrics in groups:
        tb = [r for c in cnames for r in cells.get((c, "taban"), [])]
        ad = [r for c in cnames for r in cells.get((c, "aday"), [])]
        for mname, fn in metrics:
            a1, b1 = sum(1 for r in tb if fn(r)), sum(1 for r in ad if fn(r))
            ntests += 1
            P(f"| {gname} | {mname} | {a1}/{len(tb)} | {b1}/{len(ad)} | {fisher_two_sided(a1, len(tb), b1, len(ad)):.3g} |")
    P(f"\nBu tablodaki test sayısı: {ntests} (kümeler iç içe; düzeltmesiz).")
    P(f"\nOturum olayları (iki uç toplamı, bütün koşular): connected {sum(r['session_connected_events'] for r in rows)}, "
      f"Closed {sum(r['session_closed_events'] for r in rows)}; koşu {len(rows)}")
    # 9) K-1
    P("\n### T.9 K-1 (taslak §6 K-1 (1) TEXT ölçütü; PTT ≤ 800 ms vekil) — sansürlü koşu p95'lerinin hücre medyanı\n")
    P("Eşik = dosyasız TEXT p95 (hücre medyanı) + 2 × 1088 B / hat hızı + 1 RTT (2 × tek yön gecikme).\n")
    P("| profil | kol | hücre | N | dosyasız TEXT p95 | eşik | dosyalı TEXT p95 | aşım (medyan − eşik) | "
      "eşiği aşan koşu | TEXT ölçütü | PTT ≤800 ms dosyasız % | dosyalı % | PTT vekil ölçütü | etiket |")
    P("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for prof in ("r12k_d300_l30", "r1k_d300_l30", "r12k_d1000_l30"):
        rate, dl = link_of(prof), delay_of(prof)
        for arm in ("taban", "aday"):
            base = cells.get((f"a_{prof}_dosyasiz", arm))
            if not base:
                continue
            b95 = rank([r["T_p95"] for r in base if r.get("T_p95") is not None], .5)
            thr = b95 + (2 * MAX_PACKET_LINE_BYTES / rate + 2 * dl) * 1000
            bp = rank([r["P_in800_pct"] for r in base if r.get("P_in800_pct") is not None], .5)
            for sc in ("dosyali", "dosyali_dur"):
                rs = cells.get((f"a_{prof}_{sc}", arm))
                if not rs:
                    continue
                f95 = rank([r["T_p95"] for r in rs if r.get("T_p95") is not None], .5)
                above = sum(1 for r in rs if r.get("T_p95") is not None and r["T_p95"] > thr)
                fp = rank([r["P_in800_pct"] for r in rs if r.get("P_in800_pct") is not None], .5)
                pok = "tanımsız (dosyasız %0)" if not bp else ("geçti" if fp >= 0.95 * bp else "KALDI")
                lab = "hüküm" if min(len(base), len(rs)) >= 30 else "betimsel"
                P(f"| {prof} | {arm} | {sc} | {len(rs)} | {b95 / 1000:.1f} | {thr / 1000:.1f} | {f95 / 1000:.1f} | "
                  f"{(f95 - thr) / 1000:+.1f} | {above}/{len(rs)} | {'geçti' if f95 <= thr else 'KALDI'} | "
                  f"{bp:.0f} | {fp:.0f} | {pok} | {lab} |")
    # 10) İlk engelleyici
    P("\n### T.10 Reorder ≥ 1 s bekleyen TEXT'te ilk engelleyici birimin sınıfı (bekleme ağırlıklı pay %, "
      "hücre öğeleri havuzlu) [türetilmiş]\n")
    P("F = dosya parçası, X = dosya denetimi / eşlenmemiş, T = TEXT (W/T/K), P = PTT (P/KP/açma/kapama), S = sinyal.\n")
    P("| hücre | kol | öğe | bekleme toplamı (s) | F | X | T | P | S |")
    P("|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        tot = {g: sum(r[f"T_blk_{g}_ms"] for r in rs) for g in BLK_ORDER}
        s = sum(tot.values())
        n = sum(r["T_blk_items"] for r in rs)
        if not n:
            continue
        P(f"| {k[0]} | {k[1]} | {n} | {s / 1000:.0f} | " +
          " | ".join(f"{100 * tot[g] / s:.1f}" for g in BLK_ORDER) + " |")
    # 11) IdleTimeout penceresi
    P("\n### T.11 bob IdleTimeout öncesi 60 s penceresi (hata veren koşular) [ölçüldü: ts örnekleri ≈200 ms]\n")
    P("| hücre | kol | hata | Reorder next sabit | tutulan > 0 (pencere boyunca) | alice txq > 0 (örneklerin ≥ yarısı) | "
      "alice pl > 0 (≥ yarısı) | alice RecoverArq (≥ yarısı) | 30 s durma pencerenin içinde |")
    P("|---|---|---|---|---|---|---|---|---|")
    for k in ordered:
        fr = [r for r in cells[k] if r["file_bob_status"] == "failed" and r.get("idle_next_const") is not None]
        if not fr:
            continue
        half = lambda key: sum(1 for r in fr if (r.get(key) or 0) >= 0.5)
        P(f"| {k[0]} | {k[1]} | {len(fr)} | {sum(1 for r in fr if r['idle_next_const'])} | "
          f"{sum(1 for r in fr if (r.get('idle_held_min') or 0) > 0)} | {half('idle_txq_pos_frac')} | "
          f"{half('idle_pl_pos_frac')} | {half('idle_arq_frac')} | {sum(1 for r in fr if r.get('idle_stall_inside'))} |")
    # 12) Hız çöküşü
    P("\n### T.12 A-10 hız çöküşü olan koşular\n")
    P("| hücre | kol | çöküş | sonda pacer B/s | 20 s sonrası en düşük pacer B/s | sonda yeni veri kuyruğu (birim) |")
    P("|---|---|---|---|---|---|")
    for k in ordered:
        rc_ = [r for r in cells[k] if r["rate_collapse"]]
        if rc_:
            P(f"| {k[0]} | {k[1]} | {len(rc_)} | {rng_str([r['end_pacer'] for r in rc_])} | "
              f"{rng_str([r['pacer_min_after20'] for r in rc_])} | {rng_str([r['end_txq'] for r in rc_])} |")
    rcall = [r for r in rows if r["rate_collapse"]]
    P(f"\nToplam {len(rcall)} koşu: sonda pacer {rng_str([r['end_pacer'] for r in rcall])} B/s, "
      f"20 s sonrası en düşük {rng_str([r['pacer_min_after20'] for r in rcall])} B/s, "
      f"kuyruk {rng_str([r['end_txq'] for r in rcall])} birim.")
    # 13) Kuyruk sonu takılması
    P("\n### T.13 A-11 kuyruk sonu takılması — koşu başına\n")
    P("| hücre | kol | rep | son emisyon (s) | o andaki durum | o andaki teyitsiz | RecoverFwd'ye geçiş (son emisyondan s) | "
      "sonrası alınan datagram | sonda teyitsiz / uçuş kaydı | sonda durum | tail-probe sayacı (son) | sessiz kuyruk (s) |")
    P("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for r in sorted((r for r in rows if r["tail_stuck"]), key=lambda r: (r["cell"], r["arm"], r["rep"])):
        rf = r.get("ts_recoverfwd_after_s")
        P(f"| {r['cell']} | {r['arm']} | {r['rep']} | {r['ts_last_tx_rel_s']:.1f} | {r['ts_state_at_last_tx']} | "
          f"{r['ts_un_at_last_tx']} | {'—' if rf is None else f'{rf:.1f}'} | {r['ts_rxd_after']} | "
          f"{r['end_un']} / {r['end_inf']} | {r['end_state']} | {r['ts_tlp_end']} | {r['silent_tail_s']:.0f} |")
    # 14) Motor kurtarma işi
    P("\n### T.14 Motor kurtarma işi: yeni veri kuyruğu doluyken ARQ yeniden gönderim baytı / toplam tel baytı "
      "(dosyalı hücrelerde dosya gönderiminden sonra; koşular arası medyan [Q1–Q3] (min–maks), %)\n")
    P("| hücre | kol | N (tanımlı) | txq > 0 iken pay | bütün koşu payı |")
    P("|---|---|---|---|---|")
    for k in ordered:
        rs = cells[k]
        v = [r.get("rtx_share_txq") for r in rs]
        P(f"| {k[0]} | {k[1]} | {sum(1 for x in v if x is not None)} | {fs(spread(v), 0.01)} | "
          f"{fs(spread([r.get('rtx_share_all') for r in rs]), 0.01)} |")
    # 15) Dosyanın Transport'a kabulü
    P("\n### T.15 Dosya parçalarının Transport'a kabulü (alice iz `S`, 1013 B) [ölçüldü]\n")
    P("| hücre | kol | N | kabul edilen parça (medyan, min–maks) | dosya gönderiminden son parça kabulüne (s) | "
      "yeni veri kuyruğunun dosyadan sonra ilk boşalması (s) |")
    P("|---|---|---|---|---|---|")
    for k in ordered:
        rs = [r for r in cells[k] if r.get("file_chunks_accepted") is not None]
        if rs:
            P(f"| {k[0]} | {k[1]} | {len(rs)} | {fs(spread([r['file_chunks_accepted'] for r in rs]), 1, 0)} | "
              f"{fs(spread([r['file_accept_span_s'] for r in rs]), 1, 3)} | "
              f"{fs(spread([r['file_fifo_empty_s'] for r in rs]), 1, 1)} |")
    # hücre özeti CSV
    hk = ["cell", "arm", "n", "complete", "T_p50_med", "T_p95_med", "P_p50_med", "P_p95_med",
          "T_reorder_p95_med", "T_fifo_p95_med", "dcd_runs", "tail_stuck", "rate_collapse",
          "receipt_timeout", "bob_idle_timeout", "pacer_pct_link_med", "rtx_share_txq_med"]
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
                        "pacer_pct_link_med": m("pacer_p50_pct_link"),
                        "rtx_share_txq_med": m("rtx_share_txq")})
    print(f"\n<!-- {len(rows)} koşu -->", file=sys.stderr)


if __name__ == "__main__":
    main()
