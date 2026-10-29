#!/usr/bin/env python3
"""[KTP2-F0 · A] Akış bağımsızlığı analizi — f0_flows koşu dizinlerinden öğe başına katman kırılımı.

    python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <analiz dizini> <ızgara kökü> [<kök> …]

Girdi: `<kök>/<HÜCRE>/<kol>/rep<N>/{alice.jsonl, bob.jsonl, alice.json, bob.json, rc}`
(`scripts/ktp2/f0/f0_cell.sh` düzeni). Yalnız standart kütüphane. Ham veriye YAZMAZ.

Çıktı (`--out`):
  runs/<hücre>__<kol>__rep<N>.json   koşu özeti: sınıf başına gecikme + katman kırılımı + sayaçlar
  items/<hücre>__<kol>__rep<N>.jsonl öğe başına satır (bütün katman anları)
  cells.json                         hücre × kol: koşular arası dağılım (medyan, IQR, min–maks)
  OZET.md                            tablolar (K-1 karşılaştırması dahil)

Katmanlar (öğe başına, ms; anlar Unix epoch, iki süreç aynı makinede):
  t_send     uygulama çağrısı (alice `send.t`)
  t_accept   Transport kabulü (alice iz `S`: aynı boy, çağrı penceresi içinde)
  t_first    motor ilk iletimi — TÜRETİLMİŞ üst sınır: alice `ts` örneğinde
             ilk_iletilen = kabul_sayısı(ta) − kanalda(out − un) − txq > birim olan ilk örneğin anı;
             alt sınır bir önceki örnek (çözünürlük ≈ --probe-ms + 200 ms görüntü aralığı)
  t_drained  alıcı motorun sırasız teslimi (bob iz `D`)
  t_released sıralı teslim (bob iz `R`: next > birim olan ilk kayıt)
  t_rx       uygulama teslimi (bob `rx`)
Bileşenler: adapter = accept − send · fifo = first − accept · net = drained − first (ağ + birimin
kendi kayıp kurtarması) · reorder = released − drained · consumer = rx − released · total = rx − send.
fifo/net ayrımı türetilmiştir (± örnekleme aralığı); toplamları kesindir.

Etiket: bu betiğin çıktısı [ölçüldü]dür; t_first'e dayanan bileşenler [türetilmiş]dir. N < 30
hücreler OZET.md'de "betimsel" işaretlenir. Hüküm yalnız birincil hücrelerde (N ≥ 30).
"""

import argparse
import bisect
import json
import math
import os
import re
import sys

PROFILE_RATE = {"r25": 25.0, "r1k": 1000.0, "r12k": 12500.0}
PROFILE_DELAY_MS = {"d300": 300.0, "d1000": 1000.0}
# Dosya parçası birimi 1013 B + DATA başlığı 11 + legacy zarf 22 + L2/IP/UDP 42 (netem sayar).
MAX_PACKET_LINE_BYTES = 1013 + 11 + 22 + 42
KARSI_SPURT = 90
DEADLINES_MS = (400.0, 800.0)
CLASSES = ("W", "T", "S", "P", "K", "KP", "O", "C")
LAYERS = ("adapter", "fifo", "net", "reorder", "consumer", "total")


# ── yardımcılar ──────────────────────────────────────────────────────────────────────────────

def read_jsonl(path):
    out = []
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    out.append(json.loads(line))
                except ValueError:
                    out.append({"k": "_bad"})
    except OSError:
        return None
    return out


def read_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.loads(f.read().strip() or "null")
    except (OSError, ValueError):
        return None


def read_text(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read().strip()
    except OSError:
        return None


def rank(values, p):
    """En yakın sıra (nearest-rank): artan sıradaki ⌈p·n⌉'inci değer."""
    if not values:
        return None
    s = sorted(values)
    k = max(1, math.ceil(p * len(s)))
    return s[min(k, len(s)) - 1]


def summary(values):
    vals = [v for v in values if v is not None]
    if not vals:
        return {"n": 0}
    return {
        "n": len(vals),
        "min": min(vals),
        "p50": rank(vals, 0.50),
        "p95": rank(vals, 0.95),
        "p99": rank(vals, 0.99),
        "max": max(vals),
    }


def spread(values):
    """Koşular arası dağılım: medyan, IQR (Q1, Q3), min–maks (en yakın sıra)."""
    vals = [v for v in values if v is not None]
    if not vals:
        return {"n": 0}
    return {
        "n": len(vals),
        "median": rank(vals, 0.50),
        "q1": rank(vals, 0.25),
        "q3": rank(vals, 0.75),
        "min": min(vals),
        "max": max(vals),
    }


def r3(x):
    return None if x is None else round(x, 3)


def item_class(c, sp):
    if c == "P" and sp == KARSI_SPURT:
        return "KP"
    return c


def profile_numbers(profile):
    parts = profile.split("_")
    rate = next((PROFILE_RATE[p] for p in parts if p in PROFILE_RATE), None)
    delay = next((PROFILE_DELAY_MS[p] for p in parts if p in PROFILE_DELAY_MS), None)
    return rate, delay


# ── koşu ─────────────────────────────────────────────────────────────────────────────────────

def unit_len(c, params):
    if c in ("W", "T"):
        return 5 + 6 + params["text_bytes"]
    if c == "K":
        return 5 + 6 + params["karsi_text_bytes"]
    if c == "S":
        return 5 + 2 + params["sig_bytes"]
    if c == "P":
        return 5 + 8 + params["ptt_bytes"]
    if c == "O":
        return 5 + 6
    if c == "C":
        return 5 + 4
    return None


def trace_records(lines):
    recs = []
    drops = 0
    for ln in lines:
        if ln.get("k") != "tr":
            continue
        drops += ln.get("drop", 0) or 0
        for r in ln.get("r", []):
            if len(r) == 5:
                recs.append({"kind": r[0], "sess": r[1], "unit": r[2], "len": r[3], "t": r[4]})
    return recs, drops


def busiest_session(recs, kind):
    counts = {}
    for r in recs:
        if r["kind"] == kind:
            counts[r["sess"]] = counts.get(r["sess"], 0) + 1
    if not counts:
        return None
    return max(counts.items(), key=lambda kv: kv[1])[0]


def session_of_peer(ts_line, peer):
    for s in ts_line.get("s", []):
        if s.get("peer") == peer:
            return s
    return None


def analyze_run(run_dir, cell, arm, rep):
    res = {"cell": cell, "arm": arm, "rep": rep, "dir": run_dir, "valid": False, "problems": []}
    rc_text = read_text(os.path.join(run_dir, "rc"))
    res["rc"] = int(rc_text) if rc_text and rc_text.lstrip("-").isdigit() else None
    alice = read_jsonl(os.path.join(run_dir, "alice.jsonl"))
    bob = read_jsonl(os.path.join(run_dir, "bob.jsonl"))
    a_sum = read_json(os.path.join(run_dir, "alice.json"))
    b_sum = read_json(os.path.join(run_dir, "bob.json"))
    res["alice_summary"] = a_sum
    res["bob_summary"] = b_sum
    if alice is None or bob is None:
        res["problems"].append("jsonl eksik")
        return res, []
    hdr = next((ln for ln in alice if ln.get("k") == "hdr"), None)
    if hdr is None:
        res["problems"].append("alice hdr yok")
        return res, []
    params = hdr["params"]
    res["scenario"] = hdr.get("scenario")
    res["profile"] = hdr.get("profile")
    res["params"] = params
    if res["rc"] != 0:
        res["problems"].append(f"rc={res['rc']}")
    if not a_sum:
        res["problems"].append("alice.json yok")
    elif a_sum.get("result") == "error":
        res["problems"].append("alice hata: " + str(a_sum.get("error")))
    if not b_sum:
        res["problems"].append("bob.json yok")
    for who, s in (("alice", a_sum), ("bob", b_sum)):
        if s and s.get("listener_drops", 0):
            res["problems"].append(f"{who} dinleyici düşüşü {s['listener_drops']}")

    marks = {}
    for ln in alice + bob:
        if ln.get("k") == "mark":
            marks.setdefault(ln.get("m"), ln)
    t0 = marks.get("t0", {}).get("t")
    stall = (marks.get("stall_start", {}).get("t"), marks.get("stall_end", {}).get("t"))
    karsi = marks.get("karsi_trigger")
    res["t0"] = t0
    res["stall"] = {"start": stall[0], "end": stall[1]}
    res["karsi_trigger"] = None if karsi is None else {
        "t": karsi.get("t"), "rel_ms": r3(karsi["t"] - t0) if t0 else None,
        "session": karsi.get("session")}

    # Birim izleri.
    a_tr, a_drop = trace_records(alice)
    b_tr, b_drop = trace_records(bob)
    if a_drop or b_drop:
        res["problems"].append(f"iz düşüşü alice={a_drop} bob={b_drop}")
    a_sess = busiest_session(a_tr, "S")
    b_sess = busiest_session(b_tr, "D")
    sent_recs = sorted((r for r in a_tr if r["kind"] == "S" and r["sess"] == a_sess),
                       key=lambda r: r["t"])
    sent_times = [r["t"] for r in sent_recs]
    drained = {}
    for r in b_tr:
        if r["kind"] == "D" and r["sess"] == b_sess and r["unit"] not in drained:
            drained[r["unit"]] = r["t"]
    rel = sorted(((r["t"], r["unit"]) for r in b_tr if r["kind"] == "R" and r["sess"] == b_sess))
    rel_t, rel_next, running = [], [], -1
    for t, nxt in rel:
        running = max(running, nxt)
        rel_t.append(t)
        rel_next.append(running)

    def released(u):
        i = bisect.bisect_right(rel_next, u)
        return rel_t[i] if i < len(rel_t) else None

    # Motor ilk iletimi (türetilmiş): alice ts örnekleri.
    fifo_samples = []
    for ln in alice:
        if ln.get("k") != "ts":
            continue
        s = session_of_peer(ln, "bob")
        if not s or not s.get("rec"):
            continue
        rec = s["rec"]
        ta = rec["ta"]
        accepted = bisect.bisect_right(sent_times, ta)
        in_channel = max(0, (ln.get("out") or 0) - rec["un"])
        fifo_samples.append((ta, accepted - in_channel - rec["txq"], rec["txq"], rec["un"]))
    fifo_samples.sort()
    ft, fc, run_max = [], [], -1
    for ta, cnt, _, _ in fifo_samples:
        run_max = max(run_max, cnt)
        ft.append(ta)
        fc.append(run_max)

    def first_tx(u):
        i = bisect.bisect_right(fc, u)
        if i >= len(ft):
            return None, None
        return ft[i], (ft[i - 1] if i > 0 else None)

    # Gönderimler → birimler.
    sends = [ln for ln in alice if ln.get("k") == "send"]
    sends.sort(key=lambda s: s["t"])
    used = set()
    items = {}
    order_oc = {"O": [], "C": []}
    for s in sends:
        c, sp, i = s["c"], s.get("sp"), s.get("i", 0)
        key = (c, -1 if sp is None else sp, i)
        it = {"c": item_class(c, sp), "raw_c": c, "sp": sp, "i": i, "t_send": s["t"],
              "t_ret": s.get("tr"), "due": s.get("due"), "try": s.get("try", 0), "unit": None,
              "t_accept": None}
        want = unit_len(c, params)
        if want is not None:
            lo = bisect.bisect_left(sent_times, s["t"] - 0.5)
            hi = bisect.bisect_right(sent_times, (s.get("tr") or s["t"]) + 0.5)
            for j in range(lo, hi):
                r = sent_recs[j]
                if j not in used and r["len"] == want:
                    used.add(j)
                    it["unit"] = r["unit"]
                    it["t_accept"] = r["t"]
                    break
        if c in order_oc:
            order_oc[c].append(key)
        items[key] = it
    # Alıcı teslimleri.
    rx_order = {"O": 0, "C": 0}
    for ln in bob:
        if ln.get("k") != "rx" or not ln.get("first", True):
            continue
        c = ln["c"]
        if c in ("O", "C"):
            idx = rx_order[c]
            rx_order[c] += 1
            key = order_oc[c][idx] if idx < len(order_oc[c]) else None
        elif c == "P":
            key = ("P", ln.get("sp"), ln.get("i"))
        elif c == "F":
            key = next((k for k in items if k[0] == "F"), None)
        else:
            key = (c, -1, ln.get("i"))
        if key in items and items[key].get("t_rx") is None:
            items[key]["t_rx"] = ln["t"]
    # Katmanlar.
    for it in items.values():
        u = it["unit"]
        it["t_first"], it["t_first_lo"] = first_tx(u) if u is not None else (None, None)
        it["t_drained"] = drained.get(u) if u is not None else None
        it["t_released"] = released(u) if u is not None else None
        it.setdefault("t_rx", None)
        a, f, d, rl, x = it["t_accept"], it["t_first"], it["t_drained"], it["t_released"], it["t_rx"]
        it["adapter"] = r3(a - it["t_send"]) if a is not None else None
        it["fifo"] = r3(max(0.0, f - a)) if (f is not None and a is not None) else None
        it["net"] = r3(d - f) if (d is not None and f is not None) else None
        it["reorder"] = r3(rl - d) if (rl is not None and d is not None) else None
        it["consumer"] = r3(x - rl) if (x is not None and rl is not None) else None
        it["total"] = r3(x - it["t_send"]) if x is not None else None
        if stall[0] and stall[1] and x is not None:
            it["stall_overlap"] = r3(max(0.0, min(stall[1], x) - max(stall[0], it["t_send"])))
        else:
            it["stall_overlap"] = None
        it["rel_send_ms"] = r3(it["t_send"] - t0) if t0 else None

    # Sınıf başına.
    classes = {}
    for c in CLASSES:
        its = [it for it in items.values() if it["c"] == c]
        if not its:
            continue
        tot = [it["total"] for it in its]
        entry = {
            "sent": len(its),
            "delivered": sum(1 for v in tot if v is not None),
            "mapped_units": sum(1 for it in its if it["unit"] is not None),
            "latency_ms": summary(tot),
            "layers_p50": {l: r3(rank([it[l] for it in its if it[l] is not None], 0.5))
                           for l in LAYERS},
            "layers_p95": {l: r3(rank([it[l] for it in its if it[l] is not None], 0.95))
                           for l in LAYERS},
        }
        entry["delivered_pct"] = r3(100.0 * entry["delivered"] / entry["sent"])
        if c in ("P", "KP"):
            entry["within_ms"] = {str(int(dl)): r3(100.0 * sum(1 for v in tot if v is not None and
                                                                v <= dl) / len(tot))
                                  for dl in DEADLINES_MS}
        classes[c] = entry
    res["classes"] = classes

    # Sayaçlar.
    def last_ts(lines):
        return next((ln for ln in reversed(lines) if ln.get("k") == "ts"), None)

    a_last, b_last = last_ts(alice), last_ts(bob)
    a_s = session_of_peer(a_last, "bob") if a_last else None
    b_s = session_of_peer(b_last, "alice") if b_last else None
    held_max = 0
    dcd_series = []
    for ln in bob:
        if ln.get("k") == "ts":
            s = session_of_peer(ln, "alice")
            if s and s.get("reo"):
                held_max = max(held_max, s["reo"].get("h") or 0)
            dcd_series.append((ln["t"], ln.get("dcd", 0)))
    first_dcd = next((t for t, v in dcd_series if v), None)
    final_next = rel_next[-1] if rel_next else 0
    never_below = [u for u in range(final_next) if u not in drained]
    sent_units = len(sent_recs)
    pending = [r["unit"] for r in sent_recs if r["unit"] not in drained]
    res["counters"] = {
        "alice_units_sent": sent_units,
        "bob_units_drained": len(drained),
        "units_never_drained": len(pending),
        "bob_final_next": final_next,
        "units_skipped_below_next": len(never_below),
        "bob_held_max": held_max,
        "bob_held_end": (b_s or {}).get("reo", {}).get("h") if b_s and b_s.get("reo") else None,
        "bob_delivery_channel_drops": (b_last or {}).get("dcd"),
        "bob_first_dcd_t_rel_ms": r3(first_dcd - t0) if (first_dcd and t0) else None,
        "bob_not_ready_drops": (b_last or {}).get("nrd"),
        "bob_ctx_mismatch_drops": (b_last or {}).get("cmd"),
        "bob_app": (b_last or {}).get("app"),
        "alice_app": (a_last or {}).get("app"),
        "alice_tx": None if not a_s else {k: a_s.get(k) for k in (
            "rtx", "tlp", "dup", "par", "sup", "txb", "rtxb", "txd", "rxd", "srtt", "bw", "pr")},
        "alice_receipts": (a_sum or {}).get("receipts"),
        "alice_texts_sent": (a_sum or {}).get("texts_sent"),
    }
    # Dosya.
    f_item = next((it for k, it in items.items() if k[0] == "F"), None)
    fa = (a_sum or {}).get("file", {}) if a_sum else {}
    res["file"] = None if f_item is None else {
        "sent_rel_ms": f_item["rel_send_ms"],
        "bob_received_ms": r3(f_item["t_rx"] - f_item["t_send"]) if f_item["t_rx"] else None,
        "alice_delivered_ms": r3(fa["delivered"] - f_item["t_send"]) if fa.get("delivered")
        else None,
        "alice_failed": fa.get("failed"), "why": fa.get("why"),
        "bob_status": ((b_sum or {}).get("file") or {}).get("status"),
        "bob_match": ((b_sum or {}).get("file") or {}).get("match"),
    }
    # FIFO'nun dosyadan sonra ilk boşaldığı an.
    if f_item is not None:
        empty = next((ta for ta, _, txq, _ in fifo_samples if ta > f_item["t_send"] + 1000 and
                      txq == 0), None)
        res["file"]["fifo_empty_rel_ms"] = r3(empty - f_item["t_send"]) if empty else None
    res["valid"] = not res["problems"] or all(p.startswith("iz düşüşü") for p in res["problems"])
    rows = sorted(items.values(), key=lambda it: it["t_send"])
    return res, rows


# ── hücre ve özet ────────────────────────────────────────────────────────────────────────────

def discover(roots):
    pat = re.compile(r"^rep(\d+)$")
    for root in roots:
        for cell in sorted(os.listdir(root)):
            cdir = os.path.join(root, cell)
            if not os.path.isdir(cdir) or not cell.startswith("a_"):
                continue
            for arm in sorted(os.listdir(cdir)):
                adir = os.path.join(cdir, arm)
                if not os.path.isdir(adir):
                    continue
                for rep in sorted(os.listdir(adir)):
                    m = pat.match(rep)
                    if m and os.path.isdir(os.path.join(adir, rep)):
                        yield cell, arm, int(m.group(1)), os.path.join(adir, rep)


def aggregate(runs):
    cells = {}
    for r in runs:
        cells.setdefault((r["cell"], r["arm"]), []).append(r)
    out = {}
    for (cell, arm), rs in sorted(cells.items()):
        valid = [r for r in rs if r["valid"]]
        entry = {"cell": cell, "arm": arm, "runs": len(rs), "valid_runs": len(valid),
                 "scenario": next((r.get("scenario") for r in rs if r.get("scenario")), None),
                 "profile": next((r.get("profile") for r in rs if r.get("profile")), None),
                 "problems": sorted({p for r in rs for p in r["problems"]}),
                 "classes": {}, "counters": {}}
        for c in CLASSES:
            per = [r["classes"][c] for r in valid if c in r.get("classes", {})]
            if not per:
                continue
            ce = {
                "delivered_pct": spread([p["delivered_pct"] for p in per]),
                "p50_ms": spread([p["latency_ms"].get("p50") for p in per]),
                "p95_ms": spread([p["latency_ms"].get("p95") for p in per]),
                "p99_ms": spread([p["latency_ms"].get("p99") for p in per]),
                "layers_p50_median": {l: rank([p["layers_p50"][l] for p in per
                                               if p["layers_p50"][l] is not None], 0.5)
                                      for l in LAYERS},
                "layers_p95_median": {l: rank([p["layers_p95"][l] for p in per
                                               if p["layers_p95"][l] is not None], 0.5)
                                      for l in LAYERS},
            }
            if c in ("P", "KP"):
                ce["within_ms"] = {dl: spread([p["within_ms"][dl] for p in per])
                                   for dl in per[0]["within_ms"]}
            entry["classes"][c] = ce
        for k in ("units_never_drained", "units_skipped_below_next", "bob_held_max",
                  "bob_delivery_channel_drops", "bob_not_ready_drops"):
            entry["counters"][k] = spread([r["counters"].get(k) for r in valid])
        entry["counters"]["alice_rtxb"] = spread([(r["counters"].get("alice_tx") or {}).get("rtxb")
                                                  for r in valid])
        entry["file_received_ms"] = spread([(r.get("file") or {}).get("bob_received_ms")
                                            for r in valid])
        entry["file_fifo_empty_ms"] = spread([(r.get("file") or {}).get("fifo_empty_rel_ms")
                                              for r in valid])
        entry["karsi_fired"] = sum(1 for r in valid if r.get("karsi_trigger"))
        entry["label"] = "hüküm" if len(valid) >= 30 else "betimsel (N<30)"
        out[f"{cell}|{arm}"] = entry
    return out


def fmt(x, nd=0):
    if x is None:
        return "—"
    if isinstance(x, float):
        return f"{x:.{nd}f}"
    return str(x)


def fmt_spread(s, scale=1.0, nd=0):
    if not s or not s.get("n"):
        return "—"
    v = lambda k: fmt(s[k] / scale if s.get(k) is not None else None, nd)
    return f"{v('median')} [{v('q1')}–{v('q3')}] ({v('min')}–{v('max')})"


def k1_table(cells):
    """K-1: p95(T, dosyalı[_dur]) ≤ p95(T, dosyasız) + 2·en büyük paket serileştirmesi + 1 RTT."""
    rows = []
    by = {}
    for e in cells.values():
        if not e["profile"]:
            continue
        by[(e["profile"], e["arm"], e["scenario"], e["cell"])] = e
    for (profile, arm, scen, cell), e in sorted(by.items()):
        if scen not in ("dosyali", "dosyali_dur"):
            continue
        base = next((b for (p, a, s, _), b in by.items()
                     if p == profile and a == arm and s == "dosyasiz"), None)
        rate, delay = profile_numbers(profile)
        if base is None or rate is None or delay is None:
            continue
        b95 = (base["classes"].get("T") or {}).get("p95_ms", {}).get("median")
        f95 = (e["classes"].get("T") or {}).get("p95_ms", {}).get("median")
        slack = 2 * MAX_PACKET_LINE_BYTES / rate * 1000 + 2 * delay
        thr = None if b95 is None else b95 + slack
        bp = ((base["classes"].get("P") or {}).get("within_ms") or {}).get("800", {}).get("median")
        fp = ((e["classes"].get("P") or {}).get("within_ms") or {}).get("800", {}).get("median")
        rows.append({
            "profile": profile, "arm": arm, "cell": cell, "base_p95_T_ms": b95,
            "file_p95_T_ms": f95, "threshold_ms": r3(thr),
            "k1_text_ok": None if (thr is None or f95 is None) else f95 <= thr,
            "ptt_800_base_pct": bp, "ptt_800_file_pct": fp,
            "k1_ptt_ok": None if (bp in (None, 0) or fp is None) else fp >= 0.95 * bp,
            "label": "hüküm" if min(base["valid_runs"], e["valid_runs"]) >= 30 else "betimsel",
        })
    return rows


def write_md(path, cells, k1):
    lines = ["# KTP2-F0 A — akış bağımsızlığı özeti (otomatik; f0a_analyze.py)", "",
             "Değerler: koşular arası medyan [Q1–Q3] (min–maks), saniye. Etiket: [ölçüldü]; "
             "fifo/net ayrımı [türetilmiş].", ""]
    lines += ["## K-1", "", "| profil | kol | hücre | p95 TEXT dosyasız | p95 TEXT dosyalı | eşik | "
              "TEXT | PTT ≤800 ms dosyasız % | dosyalı % | PTT | etiket |",
              "|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in k1:
        lines.append("| {profile} | {arm} | {cell} | {b} | {f} | {t} | {ok} | {bp} | {fp} | {pok} | "
                     "{label} |".format(
                         profile=r["profile"], arm=r["arm"], cell=r["cell"],
                         b=fmt(None if r["base_p95_T_ms"] is None else r["base_p95_T_ms"] / 1000, 1),
                         f=fmt(None if r["file_p95_T_ms"] is None else r["file_p95_T_ms"] / 1000, 1),
                         t=fmt(None if r["threshold_ms"] is None else r["threshold_ms"] / 1000, 1),
                         ok={True: "geçti", False: "KALDI", None: "—"}[r["k1_text_ok"]],
                         bp=fmt(r["ptt_800_base_pct"], 1), fp=fmt(r["ptt_800_file_pct"], 1),
                         pok={True: "geçti", False: "KALDI", None: "—"}[r["k1_ptt_ok"]],
                         label=r["label"]))
    lines += ["", "## Sınıf başına gecikme (s) ve katman kırılımı (p50 medyanı, s)", "",
              "| hücre | kol | N | sınıf | teslim % | p50 | p95 | p99 | fifo | net | reorder | "
              "consumer |", "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for e in cells.values():
        for c, ce in e["classes"].items():
            lp = ce["layers_p50_median"]
            lines.append("| {cell} | {arm} | {n} | {c} | {d} | {p50} | {p95} | {p99} | {fi} | {ne} | "
                         "{re} | {co} |".format(
                             cell=e["cell"], arm=e["arm"], n=e["valid_runs"], c=c,
                             d=fmt_spread(ce["delivered_pct"], 1, 0),
                             p50=fmt_spread(ce["p50_ms"], 1000, 1),
                             p95=fmt_spread(ce["p95_ms"], 1000, 1),
                             p99=fmt_spread(ce["p99_ms"], 1000, 1),
                             fi=fmt(None if lp["fifo"] is None else lp["fifo"] / 1000, 1),
                             ne=fmt(None if lp["net"] is None else lp["net"] / 1000, 1),
                             re=fmt(None if lp["reorder"] is None else lp["reorder"] / 1000, 1),
                             co=fmt(None if lp["consumer"] is None else lp["consumer"] / 1000, 2)))
    lines += ["", "## Alıcı kapasite / kayıp sayaçları", "",
              "| hücre | kol | N | etiket | teslim kanalı düşüşü | NotReady düşüşü | hiç teslim "
              "edilmeyen birim | atlanan birim | en çok tutulan | dosya alındı (s) | karşı örnek |",
              "|---|---|---|---|---|---|---|---|---|---|---|"]
    for e in cells.values():
        c = e["counters"]
        lines.append("| {cell} | {arm} | {n} | {lab} | {dcd} | {nrd} | {nd} | {sk} | {hm} | {fr} | "
                     "{k} |".format(
                         cell=e["cell"], arm=e["arm"], n=e["valid_runs"], lab=e["label"],
                         dcd=fmt_spread(c["bob_delivery_channel_drops"]),
                         nrd=fmt_spread(c["bob_not_ready_drops"]),
                         nd=fmt_spread(c["units_never_drained"]),
                         sk=fmt_spread(c["units_skipped_below_next"]),
                         hm=fmt_spread(c["bob_held_max"]),
                         fr=fmt_spread(e["file_received_ms"], 1000, 1),
                         k=e["karsi_fired"]))
    probs = [(e["cell"], e["arm"], e["problems"]) for e in cells.values() if e["problems"]]
    if probs:
        lines += ["", "## Sorunlu koşular", ""]
        for cell, arm, ps in probs:
            lines.append(f"- {cell} / {arm}: " + "; ".join(ps))
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser(description="KTP2-F0 A analizi")
    ap.add_argument("--out", required=True)
    ap.add_argument("roots", nargs="+")
    a = ap.parse_args()
    os.makedirs(os.path.join(a.out, "runs"), exist_ok=True)
    os.makedirs(os.path.join(a.out, "items"), exist_ok=True)
    runs = []
    for cell, arm, rep, d in discover(a.roots):
        res, rows = analyze_run(d, cell, arm, rep)
        runs.append(res)
        name = f"{cell}__{arm}__rep{rep}"
        with open(os.path.join(a.out, "runs", name + ".json"), "w", encoding="utf-8") as f:
            json.dump(res, f, ensure_ascii=False, indent=1)
        with open(os.path.join(a.out, "items", name + ".jsonl"), "w", encoding="utf-8") as f:
            for row in rows:
                f.write(json.dumps(row, ensure_ascii=False) + "\n")
    cells = aggregate(runs)
    k1 = k1_table(cells)
    with open(os.path.join(a.out, "cells.json"), "w", encoding="utf-8") as f:
        json.dump({"cells": cells, "k1": k1}, f, ensure_ascii=False, indent=1)
    write_md(os.path.join(a.out, "OZET.md"), cells, k1)
    print(f"{len(runs)} koşu, {len(cells)} hücre×kol → {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
