#!/usr/bin/env python3
"""[KTP2-F0 / Ajan D] Ortak ızgara kıyas analizi: Kama (taban, aday) ↔ quinn ↔ tcp.

Girdi: f0_cell.sh çıktı kök(ler)i — <kök>/<CELL>/<kol>/rep<N>/{out.jsonl, manifest.json, c.err,
s.err, rc, realized_pre.json, realized_post.json, (rakip kolları) snmp_*.txt netstat_*.txt tcp_cc.txt}.

Çıktı (stdout ya da --md/--json): hücre × kol dağılımları, kazanılan/kaybedilen hücreler, K-2
kesişim noktası (teslim oranının %90 altına düştüğü kayıp), kurulum maliyeti, tel muhasebesi,
kıyas-adaleti denetimi.

Kurallar (ORTAK-KURALLAR + KATALOG):
* Birim = koşu. Hüküm için kol başına N >= 30; N < 30 hücre "betimsel" etiketlenir, hüküm yok.
* Her hücrede koşular arası dağılım: medyan, IQR, min–maks.
* Düşen koşu (rc != 0 ya da out.jsonl yok) sessizce atılmaz: completion 0 sayılır ve sayılır.
* Gecikme kuantilleri sansürlüdür: teslim edilmeyen mesaj = +inf (p95 = inf ⇔ completion < %95).
* İki görünüm karıştırılmaz: steady-state (kurulum pencere dışında) ve cold-flow (mesaj gecikmesine
  koşunun setup_ns'i eklenir; paced yükte geç mesajlar için KÖTÜMSER üst sınırdır).
* Kazanma/kaybetme kuralı (önceden yazılı): aynı hücrede iki kolun completion(D) medyanlarının
  %95 dağılımdan-bağımsız güven aralıkları (sıra istatistiği, binom) ÖRTÜŞMÜYORSA kazanılan/
  kaybedilen; örtüşüyorsa "ayrışmadı". Ek olarak gecikme p95'i betimsel verilir.
* Rastgelelik YOK (bootstrap kullanılmaz); aynı girdi → aynı çıktı.

Kullanım:
  python3 -I rakip_analiz.py KÖK [KÖK…] [--deadlines 10,30,60] [--primary-deadline 60]
        [--kama taban,aday] [--bench /yol/bench] [--md rapor.md] [--json veri.json]
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field

INF = math.inf
NS = 1e9
PROFILE_RE = re.compile(r"(r25|r1k|r12k)_(d300|d1000)_l(\d+)")
RATE_BPS = {"r25": 25.0, "r1k": 1000.0, "r12k": 12500.0}
RTT_S = {"d300": 0.6, "d1000": 2.0}


# ───────────────────────────── okuma ─────────────────────────────


def read_text(p: str) -> str | None:
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return None


def read_json_objects(p: str) -> list[dict]:
    """Dosyadaki ardışık JSON nesneleri (kapi3_realized `>>` ile ekler)."""
    t = read_text(p)
    if not t:
        return []
    dec = json.JSONDecoder()
    out, i = [], 0
    while i < len(t):
        while i < len(t) and t[i].isspace():
            i += 1
        if i >= len(t):
            break
        try:
            obj, j = dec.raw_decode(t, i)
        except json.JSONDecodeError:
            break
        out.append(obj)
        i = j
    return out


def read_jsonl(p: str) -> tuple[dict | None, list[dict]]:
    t = read_text(p)
    if t is None:
        return None, []
    meta, ev = None, []
    for line in t.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            o = json.loads(line)
        except json.JSONDecodeError:
            continue
        if "meta" in o:
            meta = o["meta"]
        elif "event" in o:
            ev.append(o["event"])
    return meta, ev


def qdisc_netem(obj: dict, dev: str) -> dict | None:
    for q in obj.get("mid_qdisc", []) or []:
        if q.get("dev") == dev and q.get("kind") == "netem":
            return q
    return None


def qdisc_delta(pre: list[dict], post: list[dict], dev: str) -> dict:
    a = qdisc_netem(pre[-1], dev) if pre else None
    b = qdisc_netem(post[-1], dev) if post else None
    r = {"ok": b is not None}
    if b is None:
        return r
    reset = a is None or a.get("handle") != b.get("handle") or b.get("bytes", 0) < (a or {}).get("bytes", 0)
    for k in ("bytes", "packets", "drops"):
        r[k] = b.get(k, 0) - (0 if reset else (a or {}).get(k, 0))
    r["reset"] = reset
    r["pre_backlog"] = (a or {}).get("backlog", 0)
    r["pre_qlen"] = (a or {}).get("qlen", 0)
    r["post_backlog"] = b.get("backlog", 0)
    r["post_qlen"] = b.get("qlen", 0)
    return r


def snmp_tcp(p: str) -> dict:
    t = read_text(p)
    if not t:
        return {}
    lines = [ln.split() for ln in t.splitlines() if ln.startswith("Tcp:")]
    if len(lines) < 2:
        return {}
    keys, vals = lines[0][1:], lines[1][1:]
    out = {}
    for k, v in zip(keys, vals):
        try:
            out[k] = int(v)
        except ValueError:
            pass
    return out


def netstat_tcpext(p: str) -> dict:
    t = read_text(p)
    if not t:
        return {}
    lines = [ln.split() for ln in t.splitlines() if ln.startswith("TcpExt:")]
    if len(lines) < 2:
        return {}
    out = {}
    for k, v in zip(lines[0][1:], lines[1][1:]):
        try:
            out[k] = int(v)
        except ValueError:
            pass
    return out


KV_RE = re.compile(r"([A-Za-z_]+)=([^\s\]]+)")


def parse_quinn_stats(cerr: str | None) -> dict:
    if not cerr:
        return {}
    for line in cerr.splitlines():
        if line.startswith("QUINN-STATS"):
            d = {}
            for k, v in KV_RE.findall(line):
                d[k] = v
            return d
    return {}


# ───────────────────────────── istatistik ─────────────────────────────


def quantile_nr(sorted_vals: list[float], p: float) -> float:
    """Yakın-sıra kuantili (sansürlü değerler +inf olarak listede)."""
    if not sorted_vals:
        return float("nan")
    k = max(1, math.ceil(p * len(sorted_vals)))
    return sorted_vals[k - 1]


def quartiles(vals: list[float]) -> tuple[float, float, float, float, float]:
    """(min, q1, medyan, q3, maks) — doğrusal ara değer (sonlu değerler için)."""
    v = sorted(vals)
    if not v:
        nan = float("nan")
        return nan, nan, nan, nan, nan

    def q(p: float) -> float:
        if len(v) == 1:
            return v[0]
        x = p * (len(v) - 1)
        lo, hi = math.floor(x), math.ceil(x)
        if v[lo] == v[hi] or math.isinf(v[lo]) or math.isinf(v[hi]):
            return v[hi] if x - lo >= 0.5 else v[lo]
        return v[lo] + (v[hi] - v[lo]) * (x - lo)

    return v[0], q(0.25), q(0.5), q(0.75), v[-1]


def binom_cdf(n: int, k: int) -> float:
    """P(X <= k), X ~ Binom(n, 1/2)."""
    if k < 0:
        return 0.0
    s = 0
    for i in range(0, min(k, n) + 1):
        s += math.comb(n, i)
    return s / (2 ** n)


def median_ci(vals: list[float], conf: float = 0.95) -> tuple[float, float] | None:
    """Medyan için dağılımdan-bağımsız güven aralığı (sıra istatistiği). n < 6 → None."""
    v = sorted(vals)
    n = len(v)
    if n < 6:
        return None
    alpha = 1.0 - conf
    k = 0
    for j in range(1, n // 2 + 1):
        if binom_cdf(n, j - 1) <= alpha / 2:
            k = j
        else:
            break
    if k == 0:
        return None
    return v[k - 1], v[n - k]


def fmt(x: float, nd: int = 3) -> str:
    if x is None:
        return "—"
    if isinstance(x, float) and math.isnan(x):
        return "—"
    if isinstance(x, float) and math.isinf(x):
        return "∞"
    return f"{x:.{nd}f}"


# ───────────────────────────── koşu ─────────────────────────────


@dataclass
class Run:
    cell: str
    arm: str
    rep: int
    path: str
    rc: int | None = None
    ok: bool = False
    transport: str = ""
    cc: str = ""
    label: str = ""
    profile_id: str = ""
    setup_ns: int = 0
    n: int = 0
    payload: int = 0
    interval_ns: int = 0
    max_deadline_ns: int = 0
    lat_s: list[float] = field(default_factory=list)  # teslim edilmeyen = inf
    udp_tx: int = 0
    udp_rx: int = 0
    tx_comp: dict = field(default_factory=dict)
    fwd: dict = field(default_factory=dict)
    rev: dict = field(default_factory=dict)
    quinn: dict = field(default_factory=dict)
    tcp_snmp: dict = field(default_factory=dict)
    tcp_ext: dict = field(default_factory=dict)
    tcp_cc: str = ""
    manifest: dict | None = None
    note: str = ""

    def completion(self, d_s: float, cold: bool = False) -> float:
        if not self.ok or self.n == 0:
            return 0.0
        add = self.setup_ns / NS if cold else 0.0
        return sum(1 for x in self.lat_s if x + add <= d_s) / self.n

    def contaminated(self) -> bool:
        return any(
            (side.get("pre_qlen", 0) or 0) > 0 or (side.get("pre_backlog", 0) or 0) > 0
            for side in (self.fwd, self.rev)
            if side
        )


def load_run(cell: str, arm: str, rep: int, d: str) -> Run:
    r = Run(cell=cell, arm=arm, rep=rep, path=d)
    rc = read_text(os.path.join(d, "rc"))
    try:
        r.rc = int(rc.strip()) if rc is not None else None
    except ValueError:
        r.rc = None
    meta, ev = read_jsonl(os.path.join(d, "out.jsonl"))
    if meta is not None:
        r.ok = r.rc == 0 or r.rc is None
        r.transport = meta.get("transport", "")
        r.cc = meta.get("cc", "")
        r.label = meta.get("kama_arm", "")
        r.profile_id = meta.get("profile_id", "")
        r.setup_ns = int(meta.get("setup_ns", 0) or 0)
        r.n = int(meta.get("msg_count", len(ev)) or len(ev))
        r.payload = int(meta.get("payload_bytes", 0) or 0)
        r.interval_ns = int(meta.get("submit_interval_ns", 0) or 0)
        r.max_deadline_ns = int(meta.get("max_deadline_ns", 0) or 0)
        r.udp_tx = int(meta.get("udp_tx_bytes", 0) or 0)
        r.udp_rx = int(meta.get("udp_rx_bytes", 0) or 0)
        r.tx_comp = meta.get("tx", {}) or {}
        lat = []
        for e in ev:
            dn = e.get("delivered_ns")
            if dn is None:
                lat.append(INF)
            else:
                lat.append(max(0, dn - e.get("sent_ns", 0)) / NS)
        # Olay sayısı msg_count'tan azsa eksikler teslim edilmemiş sayılır.
        lat += [INF] * max(0, r.n - len(lat))
        r.lat_s = sorted(lat)
        if r.rc not in (0, None):
            r.note = f"rc={r.rc} ama out.jsonl var"
    else:
        r.note = f"out.jsonl yok/bozuk (rc={r.rc})"
    pre = read_json_objects(os.path.join(d, "realized_pre.json"))
    post = read_json_objects(os.path.join(d, "realized_post.json"))
    r.fwd = qdisc_delta(pre, post, "mb0")
    r.rev = qdisc_delta(pre, post, "ma0")
    cerr = read_text(os.path.join(d, "c.err"))
    r.quinn = parse_quinn_stats(cerr)
    a, b = snmp_tcp(os.path.join(d, "snmp_pre.txt")), snmp_tcp(os.path.join(d, "snmp_post.txt"))
    if a and b:
        r.tcp_snmp = {k: b[k] - a.get(k, 0) for k in ("OutSegs", "RetransSegs", "InSegs") if k in b}
    a, b = netstat_tcpext(os.path.join(d, "netstat_pre.txt")), netstat_tcpext(os.path.join(d, "netstat_post.txt"))
    if a and b:
        keys = ("TCPTimeouts", "TCPLossProbes", "TCPSpuriousRTOs", "TCPLostRetransmit", "TCPSynRetrans")
        r.tcp_ext = {k: b[k] - a.get(k, 0) for k in keys if k in b}
    cc = read_text(os.path.join(d, "tcp_cc.txt"))
    r.tcp_cc = (cc or "").strip()
    mt = read_text(os.path.join(d, "manifest.json"))
    if mt:
        try:
            r.manifest = json.loads(mt)
        except json.JSONDecodeError:
            r.manifest = None
    return r


def discover(roots: list[str]) -> dict[str, dict[str, list[Run]]]:
    cells: dict[str, dict[str, list[Run]]] = {}
    for root in roots:
        for cell in sorted(os.listdir(root)):
            cdir = os.path.join(root, cell)
            if not os.path.isdir(cdir):
                continue
            for arm in sorted(os.listdir(cdir)):
                adir = os.path.join(cdir, arm)
                if not os.path.isdir(adir):
                    continue
                for rep in sorted(os.listdir(adir)):
                    m = re.fullmatch(r"rep(\d+)", rep)
                    if not m:
                        continue
                    run = load_run(cell, arm, int(m.group(1)), os.path.join(adir, rep))
                    cells.setdefault(cell, {}).setdefault(arm, []).append(run)
    return cells


# ───────────────────────────── özet ─────────────────────────────


def arm_summary(runs: list[Run], deadlines: list[float]) -> dict:
    n = len(runs)
    s = {"N": n, "dusen": sum(1 for r in runs if not r.ok), "kirli": sum(1 for r in runs if r.contaminated())}
    s["claim"] = "hüküm" if n >= 30 else "betimsel"
    for d in deadlines:
        for cold in (False, True):
            vals = [r.completion(d, cold) for r in runs]
            key = f"{'cold' if cold else 'ss'}@{d:g}"
            s[key] = {
                "q": quartiles(vals),
                "ci": median_ci(vals),
                "pooled": (
                    sum(r.completion(d, cold) * r.n for r in runs) / max(1, sum(r.n for r in runs))
                ),
                "vals": vals,
            }
    pooled = sorted(x for r in runs for x in (r.lat_s if r.ok else [INF] * r.n))
    s["lat_pooled"] = {p: quantile_nr(pooled, p) for p in (0.5, 0.95, 0.99)}
    s["lat_p95_run"] = quartiles([quantile_nr(r.lat_s, 0.95) if r.ok and r.lat_s else INF for r in runs])
    s["first_lat"] = quartiles([r.lat_s[0] if r.ok and r.lat_s else INF for r in runs])
    s["setup_s"] = quartiles([r.setup_ns / NS for r in runs if r.ok])
    deliv = [sum(1 for x in r.lat_s if not math.isinf(x)) for r in runs]
    wire = []
    useful = []
    for r, k in zip(runs, deliv):
        if r.fwd.get("ok") and r.rev.get("ok"):
            tot = r.fwd.get("bytes", 0) + r.rev.get("bytes", 0)
            if k > 0:
                wire.append(tot / k)
            if r.fwd.get("bytes", 0) > 0:
                useful.append(k * r.payload / r.fwd["bytes"])
    s["wire_per_msg"] = quartiles(wire)
    s["useful_fwd"] = quartiles(useful)
    s["fwd_bytes"] = quartiles([r.fwd.get("bytes", 0) for r in runs if r.fwd.get("ok")])
    s["rev_bytes"] = quartiles([r.rev.get("bytes", 0) for r in runs if r.rev.get("ok")])
    s["transport"] = sorted({r.transport for r in runs if r.transport})
    s["cc"] = sorted({r.cc for r in runs if r.cc})
    s["tcp_cc"] = sorted({r.tcp_cc for r in runs if r.tcp_cc})
    s["retx_segs"] = quartiles([r.tcp_snmp.get("RetransSegs", 0) for r in runs if r.tcp_snmp])
    s["quinn_lost"] = quartiles([float(r.quinn.get("lost_packets_t0", r.quinn.get("lost_packets", 0))) for r in runs if r.quinn])
    s["kama_retx"] = quartiles([float(r.tx_comp.get("retx", 0)) for r in runs
                                if r.tx_comp and r.transport.startswith("kama")])
    s["notes"] = sorted({r.note for r in runs if r.note})
    return s


def decide(a: dict, b: dict) -> str:
    """a, b: bir completion anahtarının özeti. Önceden yazılı kural (bkz. modül dokümanı)."""
    if a is None or b is None:
        return "—"
    ca, cb = a["ci"], b["ci"]
    if ca is None or cb is None:
        return "ölçülemedi (n<6)"
    if ca[0] > cb[1]:
        return "KAZANILDI"
    if ca[1] < cb[0]:
        return "KAYBEDİLDİ"
    return "ayrışmadı"


def fairness_py(runs_by_arm: dict[str, list[Run]]) -> list[str]:
    """Engelleyici deney-koşulu farkları (bench fairness'in `blocking` sınıfının aynası)."""
    probe = {}
    for arm, runs in runs_by_arm.items():
        m = next((r.manifest for r in runs if r.manifest), None)
        if m:
            probe[arm] = m
    issues = []
    keys = [
        ("payload_bytes", "payload"),
        ("msg_count", "msg_count"),
        ("deadline_ns", "deadline"),
        ("submit_interval_ns", "submit_interval"),
        ("setup_class", "setup_class"),
    ]
    arms = sorted(probe)
    for i in range(len(arms)):
        for j in range(i + 1, len(arms)):
            a, b = probe[arms[i]], probe[arms[j]]
            for k, name in keys:
                if a.get(k) != b.get(k):
                    issues.append(f"{arms[i]}↔{arms[j]}: {name} {a.get(k)} != {b.get(k)}")
            ia, ib = a.get("impairment", {}), b.get("impairment", {})
            for side in ("forward", "reverse"):
                if ia.get(side) != ib.get(side):
                    issues.append(f"{arms[i]}↔{arms[j]}: {side} impairment farklı")
    return issues


def fairness_bench(bench: str, runs_by_arm: dict[str, list[Run]]) -> list[str]:
    paths = []
    for arm in sorted(runs_by_arm):
        p = next((os.path.join(r.path, "manifest.json") for r in runs_by_arm[arm]
                  if os.path.exists(os.path.join(r.path, "manifest.json"))), None)
        if p:
            paths.append(p)
    if len(paths) < 2:
        return ["(fairness: < 2 manifest)"]
    try:
        cp = subprocess.run([bench, "fairness", "--manifests", *paths], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as e:
        return [f"(fairness çalıştırılamadı: {e})"]
    lines = [ln for ln in (cp.stdout + cp.stderr).splitlines()
             if ln.startswith(("BLOCKING", "NON-NORMALIZED", "NORMALIZED", "  semantics", "comparison=", "INVALID", "DENEY"))]
    return [f"rc={cp.returncode}"] + lines


def k2_crossing(cells: dict, deadline: float, cold: bool) -> list[dict]:
    """(hız, gecikme, yük, kol) başına: completion medyanının 0,90 altına düştüğü ilk kayıp."""
    groups: dict[tuple, dict[int, list[Run]]] = {}
    for cell, arms in cells.items():
        m = PROFILE_RE.search(cell)
        if not m:
            continue
        rate, delay, loss = m.group(1), m.group(2), int(m.group(3))
        for arm, runs in arms.items():
            pay = next((r.payload for r in runs if r.payload), 0)
            groups.setdefault((rate, delay, pay, arm), {})[loss] = runs
    out = []
    for (rate, delay, pay, arm), byloss in sorted(groups.items()):
        losses = sorted(byloss)
        meds, ns, cross = {}, {}, None
        for l in losses:
            vals = [r.completion(deadline, cold) for r in byloss[l]]
            meds[l] = quartiles(vals)[2]
            ns[l] = len(vals)
            if cross is None and meds[l] < 0.90:
                cross = l
        out.append({
            "rate": rate, "delay": delay, "payload": pay, "arm": arm, "losses": losses,
            "median": meds, "n": ns, "cross": cross,
            "min_n": min(ns.values()) if ns else 0,
        })
    return out


# ───────────────────────────── rapor ─────────────────────────────


def is_kama(arm: str, kama: list[str]) -> bool:
    return any(arm == k or arm.startswith(k + "_") for k in kama)


def render(cells, deadlines, primary, kama, bench) -> tuple[str, dict]:
    md = []
    data = {"cells": {}, "k2": {}}
    md.append("# KTP2-F0 / D — rakip kıyas analizi (otomatik)\n")
    md.append(f"Teslim pencereleri: {', '.join(f'{d:g} s' for d in deadlines)}; birincil: {primary:g} s. "
              "Kural: N ≥ 30 hüküm, aksi betimsel. ss = steady-state (kurulum hariç), "
              "cold = koşunun setup_ns'i her mesaja eklenir (kötümser).\n")
    for cell in sorted(cells):
        arms = cells[cell]
        summ = {arm: arm_summary(runs, deadlines) for arm, runs in arms.items()}
        data["cells"][cell] = {a: {k: v for k, v in s.items() if k != "vals"} for a, s in summ.items()}
        md.append(f"\n## {cell}\n")
        md.append("| kol | taşıma/cc | N | düşen | kirli | " +
                  " | ".join(f"ss@{d:g} medyan [IQR] (min–maks)" for d in deadlines) +
                  f" | cold@{primary:g} medyan | p95 gecikme (havuz) | kurulum s medyan | tel B/mesaj | yararlı/ileri |")
        md.append("|" + "---|" * (10 + len(deadlines)))
        for arm in sorted(arms):
            s = summ[arm]
            cols = []
            for d in deadlines:
                q = s[f"ss@{d:g}"]["q"]
                cols.append(f"{fmt(q[2])} [{fmt(q[1])},{fmt(q[3])}] ({fmt(q[0])}–{fmt(q[4])})")
            qc = s[f"cold@{primary:g}"]["q"]
            tr = ",".join(s["transport"]) + ("/" + ",".join(s["cc"]) if s["cc"] else "")
            if s["tcp_cc"]:
                tr += f" (tcp_cc={','.join(s['tcp_cc'])})"
            md.append(f"| {arm} | {tr} | {s['N']} ({s['claim']}) | {s['dusen']} | {s['kirli']} | " +
                      " | ".join(cols) +
                      f" | {fmt(qc[2])} | {fmt(s['lat_pooled'][0.95], 2)} | {fmt(s['setup_s'][2], 3)} | "
                      f"{fmt(s['wire_per_msg'][2], 0)} | {fmt(s['useful_fwd'][2])} |")
        # kazanılan / kaybedilen
        kama_arms = [a for a in arms if is_kama(a, kama)]
        rivals = [a for a in arms if a not in kama_arms]
        if kama_arms and rivals:
            md.append("")
            md.append(f"Karşılaştırma (completion@{primary:g} s, medyan %95 GA örtüşmesi):")
            md.append("")
            md.append("| Kama kolu | rakip | ss | cold | not |")
            md.append("|---|---|---|---|---|")
            for k in sorted(kama_arms):
                for r in sorted(rivals):
                    sk, sr = summ[k], summ[r]
                    if sk["N"] < 30 or sr["N"] < 30:
                        verdict_ss = verdict_cold = "betimsel (N<30)"
                    else:
                        verdict_ss = decide(sk[f"ss@{primary:g}"], sr[f"ss@{primary:g}"])
                        verdict_cold = decide(sk[f"cold@{primary:g}"], sr[f"cold@{primary:g}"])
                    note = (f"p95 {fmt(sk['lat_pooled'][0.95], 2)} s ↔ {fmt(sr['lat_pooled'][0.95], 2)} s; "
                            f"kurulum {fmt(sk['setup_s'][2], 3)} ↔ {fmt(sr['setup_s'][2], 3)} s")
                    md.append(f"| {k} | {r} | {verdict_ss} | {verdict_cold} | {note} |")
        extra = []
        for arm in sorted(arms):
            s = summ[arm]
            if s["retx_segs"][2] == s["retx_segs"][2]:  # nan değil
                extra.append(f"{arm}: TCP RetransSegs medyan {fmt(s['retx_segs'][2], 0)}")
            if s["quinn_lost"][2] == s["quinn_lost"][2]:
                extra.append(f"{arm}: quinn lost_packets(t0) medyan {fmt(s['quinn_lost'][2], 0)}")
            if s["kama_retx"][2] == s["kama_retx"][2]:
                extra.append(f"{arm}: Kama retx B medyan {fmt(s['kama_retx'][2], 0)}")
            for nt in s["notes"]:
                extra.append(f"{arm}: {nt}")
            if s["kirli"]:
                extra.append(f"{arm}: {s['kirli']} koşuda koşu öncesi netem kuyruğu BOŞ DEĞİLDİ (önceki koşunun artığı)")
        if extra:
            md.append("")
            md += [f"- {x}" for x in extra]
        issues = fairness_py(arms)
        md.append("")
        md.append("Kıyas adaleti (deney koşulu): " + ("eşleşiyor" if not issues else "; ".join(issues)))
        if bench:
            md.append("")
            md.append("`bench fairness` (rep1 manifestleri):")
            md.append("```")
            md += fairness_bench(bench, arms)
            md.append("```")
    for cold in (False, True):
        tag = "cold" if cold else "ss"
        k2 = k2_crossing(cells, primary, cold)
        data["k2"][tag] = k2
        if not k2:
            continue
        md.append(f"\n## K-2 kesişim noktası ({tag}, completion@{primary:g} s medyanı < 0,90 olan ilk kayıp)\n")
        md.append("| hız | gecikme | yük B | kol | kayıp → medyan (N) | kesişim |")
        md.append("|---|---|---|---|---|---|")
        for g in k2:
            seq = ", ".join(f"l{l}→{fmt(g['median'][l])}({g['n'][l]})" for l in g["losses"])
            cross = f"l{g['cross']}" if g["cross"] is not None else f"> l{max(g['losses'])} (görülmedi)"
            tag_n = "" if g["min_n"] >= 30 else " [betimsel N<30]"
            md.append(f"| {g['rate']} | {g['delay']} | {g['payload']} | {g['arm']} | {seq} | {cross}{tag_n} |")
        # Kama ↔ quinn kesişim karşılaştırması (taslak §6 K-2: Kama'nınki QUIC'inkinden ileride mi?)
        by_key: dict[tuple, dict[str, dict]] = {}
        for g in k2:
            by_key.setdefault((g["rate"], g["delay"], g["payload"]), {})[g["arm"]] = g
        lines = []
        for key, arms_g in sorted(by_key.items()):
            for k in sorted(a for a in arms_g if is_kama(a, kama)):
                for q in sorted(a for a in arms_g if a.startswith("quinn")):
                    gk, gq = arms_g[k], arms_g[q]
                    ck = INF if gk["cross"] is None else gk["cross"]
                    cq = INF if gq["cross"] is None else gq["cross"]
                    if gk["losses"] != gq["losses"]:
                        rel = "kayıp kümeleri farklı — kıyaslanamaz"
                    elif ck > cq:
                        rel = "Kama'nın kesişimi daha yüksek kayıpta"
                    elif ck < cq:
                        rel = "Kama'nın kesişimi daha düşük kayıpta"
                    else:
                        rel = "aynı kayıp düzeyi (ya da ikisi de görülmedi)"
                    if min(gk["min_n"], gq["min_n"]) < 30:
                        rel += " [betimsel N<30]"
                    ks = f"l{gk['cross']}" if gk["cross"] is not None else ">max"
                    qs = f"l{gq['cross']}" if gq["cross"] is not None else ">max"
                    lines.append(f"- {key[0]} {key[1]} {key[2]} B: {k} {ks} ↔ {q} {qs}: {rel}")
        if lines:
            md.append("")
            md += lines
    return "\n".join(md) + "\n", data


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("roots", nargs="+")
    ap.add_argument("--deadlines", default="10,30,60")
    ap.add_argument("--primary-deadline", type=float, default=60.0)
    ap.add_argument("--kama", default="taban,aday", help="Kama kol adları (önek eşleşmesi: taban_*)")
    ap.add_argument("--bench", default="", help="bench ikilisi (verilirse `fairness` çağrılır)")
    ap.add_argument("--md", default="")
    ap.add_argument("--json", default="")
    a = ap.parse_args()
    deadlines = [float(x) for x in a.deadlines.split(",") if x]
    if a.primary_deadline not in deadlines:
        deadlines.append(a.primary_deadline)
    cells = discover(a.roots)
    if not cells:
        print("hücre bulunamadı", file=sys.stderr)
        return 2
    md, data = render(cells, deadlines, a.primary_deadline, [k for k in a.kama.split(",") if k], a.bench)
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(md)
    else:
        sys.stdout.write(md)
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=1, default=str)
    return 0


if __name__ == "__main__":
    sys.exit(main())
