#!/usr/bin/env python3
"""[KTP2-F0 / Ajan D] Karışık akış (dosya + TEXT + PTT) kıyas analizi — TAŞIMA DÜZEYİ.

Girdi: f0_cell.sh kökü; kol dizinlerinde `bench mixed-client` çıktısı `mixed_tx.jsonl` ve
`bench mixed-serve` alım günlüğü `mixed_rx.jsonl`. Kol adları `<önek>_mix` / `<önek>_nofile`
(önek: taban, aday, quinn, tcp …); aynı önekin iki kolu K-1 çifti olarak eşlenir.

Ölçüler (koşu başına, sonra kol başına dağılım):
* gecikme = alıcı uygulamaya teslim (rx_unix_ns) − PLANLI sunum (sent_unix_ns); teslim edilmeyen = inf.
* PTT: tek-yön son tarihe (varsayılan 0,4 s ve 0,8 s) yetişen çerçeve oranı. Bu TAŞIMA gecikmesidir;
  kodlayıcı demeti, blok oluşturma ve titreşim tamponu DAHİL DEĞİLDİR (ağızdan kulağa değil).
* TEXT: p50/p95/p99 (sansürlü), 10 s / 30 s içinde teslim oranı.
* Dosya: bütün parçaların alındığı an (tamamlanmadıysa inf), alınan parça oranı.
* Tel: netem kuyruğu sayaçları (ileri mb0, dönüş ma0; L2 dahil) — dört kolda simetrik.
K-1 uyarlaması (taşıma düzeyi; taslak §6): TEXT p95(dosyalı) ≤ p95(dosyasız) + 2 × en büyük paket
serileştirmesi + 1 RTT; PTT son tarih oranı(dosyalı) ≥ 0,95 × (dosyasız). N < 30 → betimsel.

Kullanım: python3 -I karisik_analiz.py KÖK [KÖK…] [--ptt-deadlines 0.4,0.8] [--md X] [--json Y]
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rakip_analiz import (  # noqa: E402  (aynı dizindeki yardımcılar)
    INF, PROFILE_RE, RATE_BPS, RTT_S, fmt, median_ci, quantile_nr, quartiles, read_json_objects,
    read_text, qdisc_delta,
)

MAX_PKT_L2 = 1514  # Ethernet MTU 1500 + 14


def read_lines(p: str) -> list[dict]:
    t = read_text(p)
    out = []
    if not t:
        return out
    for line in t.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return out


def load_run(d: str) -> dict:
    r = {"path": d, "ok": False, "note": ""}
    rc = read_text(os.path.join(d, "rc"))
    try:
        r["rc"] = int(rc.strip()) if rc is not None else None
    except ValueError:
        r["rc"] = None
    tx = read_lines(os.path.join(d, "mixed_tx.jsonl"))
    rx = read_lines(os.path.join(d, "mixed_rx.jsonl"))
    meta = next((o["mixed_meta"] for o in tx if "mixed_meta" in o), None)
    if meta is None:
        r["note"] = f"mixed_tx.jsonl yok/bozuk (rc={r['rc']})"
        return r
    r["meta"] = meta
    r["ok"] = bool(meta.get("connected")) and r["rc"] in (0, None)
    if r["rc"] not in (0, None):
        r["note"] = f"rc={r['rc']}"
    if not meta.get("connected"):
        r["note"] = (r["note"] + " bağlantı kurulamadı").strip()
    rx_at = {}
    for o in rx:
        x = o.get("rx")
        if x and x.get("id") not in rx_at:
            rx_at[x["id"]] = x["rx_unix_ns"]
    flows: dict[str, list[float]] = {"ptt": [], "text": [], "file": []}
    neg = 0
    for o in tx:
        t = o.get("tx")
        if not t:
            continue
        got = rx_at.get(t["id"]) if r["ok"] else None
        if got is None:
            lat = INF
        else:
            lat = (got - t["sent_unix_ns"]) / 1e9
            if lat < 0:
                neg += 1
                lat = 0.0
        flows.setdefault(t["flow"], []).append(lat)
    r["flows"] = {k: sorted(v) for k, v in flows.items()}
    r["neg"] = neg
    pre = read_json_objects(os.path.join(d, "realized_pre.json"))
    post = read_json_objects(os.path.join(d, "realized_post.json"))
    r["fwd"] = qdisc_delta(pre, post, "mb0")
    r["rev"] = qdisc_delta(pre, post, "ma0")
    return r


def per_run(r: dict, ptt_d: list[float]) -> dict:
    f = r.get("flows", {})
    m = {}
    ptt = f.get("ptt", [])
    for d in ptt_d:
        m[f"ptt@{d:g}"] = (sum(1 for x in ptt if x <= d) / len(ptt)) if ptt else float("nan")
    text = f.get("text", [])
    for p in (0.5, 0.95, 0.99):
        m[f"text_p{int(p * 100)}"] = quantile_nr(text, p) if text else float("nan")
    for d in (10.0, 30.0):
        m[f"text@{d:g}"] = (sum(1 for x in text if x <= d) / len(text)) if text else float("nan")
    fl = f.get("file", [])
    if fl:
        m["file_done_s"] = fl[-1]
        m["file_frac"] = sum(1 for x in fl if not math.isinf(x)) / len(fl)
    m["fwd_bytes"] = r.get("fwd", {}).get("bytes", float("nan"))
    m["rev_bytes"] = r.get("rev", {}).get("bytes", float("nan"))
    return m


def summarize(runs: list[dict], ptt_d: list[float]) -> dict:
    pr = [per_run(r, ptt_d) for r in runs]
    keys = sorted({k for m in pr for k in m})
    s = {"N": len(runs), "dusen": sum(1 for r in runs if not r["ok"]),
         "notes": sorted({r["note"] for r in runs if r["note"]}),
         "neg": sum(r.get("neg", 0) for r in runs),
         "transport": sorted({r.get("meta", {}).get("transport", "") for r in runs} - {""}),
         "setup_s": quartiles([r["meta"]["setup_ns"] / 1e9 for r in runs if r.get("meta")])}
    for k in keys:
        vals = [m[k] for m in pr if k in m and not (isinstance(m[k], float) and math.isnan(m[k]))]
        s[k] = {"q": quartiles(vals), "ci": median_ci(vals)}
    # havuzlanmış (koşular birleşik) TEXT kuantilleri
    pooled = sorted(x for r in runs for x in r.get("flows", {}).get("text", []))
    s["text_pooled_p95"] = quantile_nr(pooled, 0.95) if pooled else float("nan")
    pp = [x for r in runs for x in r.get("flows", {}).get("ptt", [])]
    for d in ptt_d:
        s[f"ptt_pooled@{d:g}"] = (sum(1 for x in pp if x <= d) / len(pp)) if pp else float("nan")
    return s


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("roots", nargs="+")
    ap.add_argument("--ptt-deadlines", default="0.4,0.8")
    ap.add_argument("--md", default="")
    ap.add_argument("--json", default="")
    a = ap.parse_args()
    ptt_d = [float(x) for x in a.ptt_deadlines.split(",") if x]
    cells: dict[str, dict[str, list[dict]]] = {}
    for root in a.roots:
        for cell in sorted(os.listdir(root)):
            cdir = os.path.join(root, cell)
            if not os.path.isdir(cdir):
                continue
            for arm in sorted(os.listdir(cdir)):
                adir = os.path.join(cdir, arm)
                if not os.path.isdir(adir):
                    continue
                for rep in sorted(os.listdir(adir)):
                    if re.fullmatch(r"rep\d+", rep):
                        run = load_run(os.path.join(adir, rep))
                        if "meta" in run or run["note"]:
                            cells.setdefault(cell, {}).setdefault(arm, []).append(run)
    md = ["# KTP2-F0 / D — karışık akış kıyası (taşıma düzeyi, otomatik)\n",
          "PTT oranları TEK-YÖN taşıma gecikmesidir (demet/blok/titreşim tamponu hariç). "
          "kama-product kolu motorun SIRASIZ teslimini ölçer; S1 adaptörünün oturum-geneli sıralaması "
          "bu yolda yoktur (ürün uygulama yolu Ajan A'da).\n"]
    data = {}
    for cell in sorted(cells):
        arms = cells[cell]
        summ = {arm: summarize(runs, ptt_d) for arm, runs in arms.items()}
        data[cell] = summ
        md.append(f"\n## {cell}\n")
        cols = [f"PTT@{d:g}s" for d in ptt_d] + ["TEXT p50", "TEXT p95", "TEXT p99", "TEXT@10s",
                                                   "dosya bitiş s", "dosya oranı", "ileri B", "dönüş B",
                                                   "kurulum s"]
        md.append("| kol | taşıma | N (düşen) | " + " | ".join(cols) + " |")
        md.append("|" + "---|" * (3 + len(cols)))
        for arm in sorted(arms):
            s = summ[arm]

            def med(k: str, nd: int = 3) -> str:
                if k not in s:
                    return "—"
                q = s[k]["q"]
                return f"{fmt(q[2], nd)} [{fmt(q[1], nd)},{fmt(q[3], nd)}]"

            row = [med(f"ptt@{d:g}") for d in ptt_d] + [
                med("text_p50", 2), med("text_p95", 2), med("text_p99", 2), med("text@10"),
                med("file_done_s", 1), med("file_frac"), med("fwd_bytes", 0), med("rev_bytes", 0),
                fmt(s["setup_s"][2], 3)]
            tag = "" if s["N"] >= 30 else " betimsel"
            md.append(f"| {arm} | {','.join(s['transport'])} | {s['N']} ({s['dusen']}){tag} | " + " | ".join(row) + " |")
        # K-1 uyarlaması
        m = PROFILE_RE.search(cell)
        rate, delay = (m.group(1), m.group(2)) if m else (None, None)
        if rate and delay:
            slack = 2 * MAX_PKT_L2 / RATE_BPS[rate] + RTT_S[delay]
            md.append("")
            md.append(f"K-1 (taşıma düzeyi uyarlaması): TEXT p95 payı = 2 × {MAX_PKT_L2} B / "
                      f"{RATE_BPS[rate]:g} B/s + {RTT_S[delay]:g} s = {slack:.3f} s; PTT oranı ≥ 0,95 × dosyasız.")
            md.append("")
            md.append("| önek | TEXT p95 dosyalı (havuz) | dosyasız (havuz) | pay içinde | "
                      + " | ".join(f"PTT@{d:g} dosyalı/dosyasız" for d in ptt_d) + " | hüküm |")
            md.append("|" + "---|" * (5 + len(ptt_d)))
            prefixes = sorted({arm.rsplit("_", 1)[0] for arm in arms if arm.endswith(("_mix", "_nofile"))})
            for p in prefixes:
                a_mix, a_no = summ.get(f"{p}_mix"), summ.get(f"{p}_nofile")
                if not a_mix or not a_no:
                    continue
                t_mix, t_no = a_mix["text_pooled_p95"], a_no["text_pooled_p95"]
                text_ok = (not math.isinf(t_mix)) and (t_mix <= t_no + slack)
                ptt_cells, ptt_ok = [], True
                for d in ptt_d:
                    x, y = a_mix[f"ptt_pooled@{d:g}"], a_no[f"ptt_pooled@{d:g}"]
                    ptt_cells.append(f"{fmt(x)}/{fmt(y)}")
                    if not (x >= 0.95 * y):
                        ptt_ok = False
                enough = a_mix["N"] >= 30 and a_no["N"] >= 30
                verdict = ("GEÇTİ" if text_ok and ptt_ok else "GEÇMEDİ") if enough else "betimsel (N<30)"
                md.append(f"| {p} | {fmt(t_mix, 2)} | {fmt(t_no, 2)} | {'evet' if text_ok else 'hayır'} | "
                          + " | ".join(ptt_cells) + f" | {verdict} |")
        for arm in sorted(arms):
            for nt in summ[arm]["notes"]:
                md.append(f"- {arm}: {nt}")
            if summ[arm]["neg"]:
                md.append(f"- {arm}: {summ[arm]['neg']} öğede alım anı plandan önce (saat/plan anomalisi; 0'a kırpıldı)")
    out = "\n".join(md) + "\n"
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(out)
    else:
        sys.stdout.write(out)
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=1, default=str)
    return 0


if __name__ == "__main__":
    sys.exit(main())
