#!/usr/bin/env python3
"""[YOL3 · M2] EK-A §2 eksikleri (KK-M2-EKA-2…5): G2 teşhis koşusunun ham verisinden ayrıştırma.

    python3 -I reports/yol3/2026-11-09/g2/eka_eksikler.py                 # ham/g2/*.tar.zst
    python3 -I reports/yol3/2026-11-09/g2/eka_eksikler.py --kok <koşu kökü>
    python3 -I reports/yol3/2026-11-09/g2/eka_eksikler.py --json <çıktı.json>

Girdi: `reports/yol3/2026-11-09/ham/g2/*.tar.zst` (varsayılan; Python ≥ 3.14 `tarfile` 'r:zst') ya da
aynı yapıdaki açılmış koşu kökü (`<hücre>__<kol>/<kol>/rep<N>/`). Yalnız stdlib; B hücreleri için
deponun F0 analizörü (`scripts/ktp2/f0/b/analiz_b.py`, `fizik.py`; ikisi de stdlib) içe aktarılır.
Çıktı: stdout'a markdown tablolar (EK-A-EKSIKLER.md'deki tablolar buradan), `--json` ile koşu satırları.
Manifest (`docs/design/E1-TESHIS-G2.md`, sha256 37bcabe3…) ve okuma kuralı DEĞİŞMEZ; bu betik seçim
yapmaz, yalnız EK-A §2'nin istediği ayrıştırmaları üretir. Birim = koşu (rep); medyanlar koşu değerleri
üzerinden; N = 10, betimsel.

Katmanlar (koddan; ölçüm ağacı `yol2/o1` 5fdad83b):
  * A hücreleri (`f0_flows`, gerçek UDP): `ts` aynasındaki `txb` = client-core runtime.rs `seal_send!`
    içinde `stats.tx_bytes += sd.bytes.len()` (mühürlü datagram, UDP yükü; deneme düzlemi). `rtxb` =
    aynı noktada `TxClass::Retx` (ARQ + kuyruk sondası; sondalar da `TransmitKind::Retransmit`,
    sender.rs `act_tail_probe_emitted`). `rtx`, `tlp`, `dup`, `par` = motorun karar sayaçları.
    Yeni-veri ve yeni-kopya bayt sayacı aynada YOK: yeni veri = Transport'un kabul ettiği birimlerin
    (`tr` S kaydı `len`) her birine sabit mühür eki eklenerek türetilir. Ek = DATA başlığı 11 +
    mühür 22 = 33 B; koşunun ilk emisyonunda ölçülen fark (`txb` 108 − birim 75) ile doğrulanır.
  * B hücreleri (`bench`, kama-product): KP-CLIENT `tx=…B(data retx dup fec ctrl)` (UDP yükü) ve
    yakalama (`capb_*.csv.gz`) üzerinden analiz_b'nin sınıf baytları (`wfa_*` = enjekte, L2).
"""

import argparse
import gzip
import json
import math
import pathlib
import statistics
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True  # analiz_b içe aktarılır; depoya __pycache__ yazılmaz
HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[3]
F0B = REPO / "scripts" / "ktp2" / "f0" / "b"

SEAL_PLUS_HDR = 33          # 11 (HDR_DATA, client-core wire.rs) + 22 (mühür); `seal_overhead` ile denetlenir
FILE_UNIT = 1013            # dosya parçası birimi (f0a_analyze FILE_CHUNK_UNIT_BYTES)
OFFERED_IP_BPS = 3702.0     # manşet sunulan IP B/s (oku.py R1 ile aynı: acil-betikler/v-s3/bins.py `off`)
ANALOG = "a_r12k_d150_l20_foto150k"
DOSYASIZ = "a_r12k_d300_l30_dosyasiz"
MANSET = "b_r12k_d1000_l30_p1000"
L0 = "b_r12k_d1000_l0_p1000"
R25 = "b_r25_d300_l20_p65"
SHOW = {"K2mc": "K2−c", "K2me": "K2−e"}
ARM_ORDER = ["K0", "K0r", "K1", "K2", "K2mc", "K2me"]
POST_STARTUP = ("Drain", "Steady", "ProbeUp", "DrainRtprop")  # yalnız Startup'tan sonra erişilen


def show(a):
    return SHOW.get(a, a)


def med(xs):
    xs = [x for x in xs if x is not None and not (isinstance(x, float) and math.isnan(x))]
    return statistics.median(xs) if xs else None


def fmt(x, nd=2):
    if x is None:
        return "—"
    if isinstance(x, bool):
        return "evet" if x else "hayır"
    if isinstance(x, int):
        return str(x)
    return f"{x:.{nd}f}"


def rng(xs, nd=2):
    xs = [x for x in xs if x is not None]
    if not xs:
        return "—"
    return f"{fmt(min(xs), nd)}–{fmt(max(xs), nd)}"


# ── kaynaklar ─────────────────────────────────────────────────────────────────────────────────

def cell_dirs(args, tmp):
    """(hücre_dizini, taban, kol) üretir. --kok yoksa her tar.zst geçici dizine açılır."""
    if args.kok:
        kok = pathlib.Path(args.kok).expanduser().resolve()
        for cd in sorted(p for p in kok.iterdir() if p.is_dir() and "__" in p.name
                         and p.name[:2] in ("a_", "b_")):
            base, arm = cd.name.split("__", 1)
            yield cd, base, arm
        return
    ham = pathlib.Path(args.ham).expanduser().resolve()
    for tz in sorted(ham.glob("*.tar.zst")):
        name = tz.name[: -len(".tar.zst")]
        base, arm = name.split("__", 1)
        dest = pathlib.Path(tempfile.mkdtemp(dir=tmp))
        with tarfile.open(tz, mode="r:zst") as t:
            t.extractall(dest, filter="data")
        yield dest / name, base, arm


def read_jsonl(p):
    out = []
    try:
        with open(p, encoding="utf-8") as f:
            for ln in f:
                try:
                    out.append(json.loads(ln))
                except ValueError:
                    pass
    except OSError:
        return None
    return out


def read_json(p):
    try:
        with open(p, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def realized_loss(rep):
    """netem sayaçları (koşu öncesi/sonrası farkı): ileri = mb0, dönüş = ma0 (f0_cell.sh:49)."""
    pre, post = read_json(rep / "realized_pre.json"), read_json(rep / "realized_post.json")
    if not pre or not post:
        return None, None
    out = {}
    for q1 in post.get("mid_qdisc", []):
        if q1.get("kind") != "netem":
            continue
        q0 = next((q for q in pre.get("mid_qdisc", []) if q.get("dev") == q1["dev"]
                   and q.get("kind") == "netem"), {"packets": 0, "drops": 0})
        sent, drop = q1["packets"] - q0["packets"], q1["drops"] - q0["drops"]
        out[q1["dev"]] = drop / (sent + drop) if sent + drop else None
    return out.get("mb0"), out.get("ma0")


# ── A hücresi koşusu ──────────────────────────────────────────────────────────────────────────

def busiest(recs, kind):
    c = {}
    for r in recs:
        if r[0] == kind:
            c[r[1]] = c.get(r[1], 0) + 1
    return max(c.items(), key=lambda kv: kv[1])[0] if c else None


def tr_recs(lines):
    return [r for ln in lines if ln.get("k") == "tr" for r in ln.get("r", []) if len(r) == 5]


def a_run(rep):
    alice, bob = read_jsonl(rep / "alice.jsonl"), read_jsonl(rep / "bob.jsonl")
    a_sum, b_sum = read_json(rep / "alice.json") or {}, read_json(rep / "bob.json") or {}
    rc = (rep / "rc").read_text().strip() if (rep / "rc").exists() else None
    hdr = next(ln for ln in alice if ln.get("k") == "hdr")
    prm = hdr["params"]
    t0 = next(ln["t"] for ln in alice if ln.get("k") == "mark" and ln.get("m") == "t0")
    ts = []
    for ln in alice:
        if ln.get("k") != "ts":
            continue
        s = next((x for x in ln.get("s", []) if x.get("peer") == "bob"), None)
        if s:
            ts.append(((ln["t"] - t0) / 1000.0, s))
    a_tr, b_tr = tr_recs(alice), tr_recs(bob)
    a_sess, b_sess = busiest(a_tr, "S"), busiest(b_tr, "D")
    sent = sorted(((r[4] - t0) / 1000.0, r[2], r[3]) for r in a_tr
                  if r[0] == "S" and r[1] == a_sess)
    s_len = {u: ln for _, u, ln in sent}
    drained = {}
    for r in b_tr:
        if r[0] == "D" and r[1] == b_sess and r[2] in s_len and r[2] not in drained:
            drained[r[2]] = r[3]
    last = ts[-1][1]
    rec = last.get("rec") or {}
    txq_end = rec.get("txq", 0) or 0
    n_tx = len(sent) - txq_end
    new_data = sum(ln + SEAL_PLUS_HDR for _, _, ln in sent[:n_tx])
    closure = last["txd"] - (n_tx + last["rtx"] + last["dup"] + last["par"])
    first = next((s for _, s in ts if s["txb"] > 0), None)
    seal_overhead = (first["txb"] - sent[0][2]) if first and sent and first["txd"] == 1 else None
    useful_unit = sum(drained.values())
    useful_sealed = useful_unit + SEAL_PLUS_HDR * len(drained)

    def at(t):
        v = ts[0][1]
        for tt, s in ts:
            if tt > t:
                break
            v = s
        return v

    wins = []
    span = prm["ptt_bundles"] * prm["ptt_bundle_ms"] / 1000.0
    for a_ms in prm.get("ptt_at_ms", []):
        a, b = a_ms / 1000.0, a_ms / 1000.0 + span
        s0, s1 = at(a), at(b)
        off = sum(ln + SEAL_PLUS_HDR for t, _, ln in sent if a <= t < b)
        pr = med([s["pr"] for t, s in ts if a <= t < b])
        dtx, drtx = s1["txb"] - s0["txb"], s1["rtxb"] - s0["rtxb"]
        wins.append({"a": a, "b": b, "offered_Bps": off / span, "pr_med": pr,
                     "emit_Bps": dtx / span, "retx_share": drtx / dtx if dtx else None,
                     "new_Bps": (dtx - drtx) / span,   # yeni veri + §11 kopya (kopya adedi ayrı)
                     "d_rtx": s1["rtx"] - s0["rtx"], "d_tlp": s1["tlp"] - s0["tlp"],
                     "d_dup": s1["dup"] - s0["dup"]})
    ptt_unit = prm["ptt_bytes"] + 5 + 8
    ptt_off = (ptt_unit + SEAL_PLUS_HDR) / (prm["ptt_bundle_ms"] / 1000.0)
    pr_all = med([s["pr"] for _, s in ts if s.get("st") != "Idle"])
    # Tamamlanma (EKA-3): sınıf başına gönderilen ↔ bob'un saydığı.
    sent_c, bob_c = a_sum.get("sent", {}), b_sum.get("counts", {})
    missing = {c: (n, bob_c.get(c, 0)) for c, n in sent_c.items() if c != "F" and bob_c.get(c, 0) < n}
    f = a_sum.get("file") or {}
    bf = b_sum.get("file") or {}
    has_file = prm.get("file_bytes", 0) > 0
    bob_file_ok = bool(has_file and bf.get("status") == "received" and bf.get("match"))
    file_bob_s = ((bf["at"] - f["sent"]) / 1000.0) if bob_file_ok and f.get("sent") else None
    ptt_closed = sent_c.get("C", 0) > 0 and bob_c.get("C", 0) >= sent_c.get("C", 0) \
        and bob_c.get("P", 0) >= sent_c.get("P", 0)
    exit_ = next(((t, s) for t, s in ts if s["st"] in POST_STARTUP), None)
    fl, rl = realized_loss(rep)
    rel_next = max((r[2] for r in b_tr if r[0] == "R" and r[1] == b_sess), default=None)
    b_fail = next((ln for ln in bob if ln.get("k") == "ev" and ln.get("e") == "file_failed"), None)
    a_fail = next((ln for ln in alice if ln.get("k") == "ev" and ln.get("e") == "file_failed"), None)

    def since_file(ev):
        return (ev["t"] - f["sent"]) / 1000.0 if ev and f.get("sent") else None
    return {
        "rep": int(rep.name[3:]), "rc": rc, "alice_result": a_sum.get("result"),
        "file": has_file, "bob_file_ok": bob_file_ok, "file_bob_s": file_bob_s,
        "alice_receipt": f.get("delivered") is not None, "file_why": f.get("why"),
        "ptt_closed": ptt_closed, "missing": missing,
        "complete": (not has_file or bob_file_ok) and ptt_closed and not missing,
        "karsi": any(ln.get("k") == "mark" and ln.get("m") == "karsi_trigger" for ln in alice),
        "txb": last["txb"], "rtxb": last["rtxb"], "rtx": last["rtx"], "tlp": last["tlp"],
        "dup": last["dup"], "par": last["par"], "txd": last["txd"], "txq_end": txq_end,
        "new_data_B": new_data, "rest_B": last["txb"] - last["rtxb"] - new_data,
        "closure_dgrams": closure, "seal_overhead": seal_overhead,
        "useful_unit_B": useful_unit, "useful_sealed_B": useful_sealed,
        "useful_over_sent": useful_sealed / last["txb"] if last["txb"] else None,
        "loss_fwd": fl, "loss_rev": rl, "wins": wins,
        "oku_ratio": pr_all / ptt_off if pr_all is not None else None, "ptt_offered_Bps": ptt_off,
        "startup_exit_s": exit_[0] if exit_ else None,
        "startup_exit_state": exit_[1]["st"] if exit_ else None,
        "bw_at_exit": exit_[1]["bw"] if exit_ else None,
        "bw_max": max(s["bw"] for _, s in ts),
        "pr_med_10_60": med([s["pr"] for t, s in ts if 10 <= t < 60]),
        "wire_5_65_Bps": (at(65)["txb"] - at(5)["txb"]) / 60.0,
        "probe_up": last.get("pu"), "end_state": last.get("st"),
        "bob_units": len(drained), "bob_file_units": sum(1 for v in drained.values() if v == FILE_UNIT),
        "file_units": sum(1 for _, _, ln in sent if ln == FILE_UNIT),
        "bob_release_next": rel_next, "t_end_s": ts[-1][0],
        "bob_file_fail": (b_fail.get("why"), since_file(b_fail)) if b_fail else None,
        "alice_fail_after_s": since_file(a_fail),
    }


# ── B hücresi koşusu ──────────────────────────────────────────────────────────────────────────

def srate(rep, a=3, b=28):
    """oku.py `acil_srate` ile aynı: capb_cli 'o' satırlarının ip_len toplamı, [a, b) s, B/s."""
    try:
        with open(rep / "out.jsonl", encoding="utf-8") as f:
            z = json.loads(f.readline())["meta"]["t0_unix_ns"]
        tot = 0
        with gzip.open(rep / "capb_cli.csv.gz", "rt") as f:
            for ln in f:
                if ln.startswith("#"):
                    continue
                p = ln.split(",", 6)
                if p[1] == "o" and a <= (int(p[0]) - z) / 1e9 < b:
                    tot += int(p[3])
    except (OSError, KeyError, ValueError):
        return None
    return tot / (b - a)


def b_run(rep, g, arm, analiz_b):
    r = analiz_b.analyze_run(rep, g, arm)
    need = OFFERED_IP_BPS / (1 - g["loss"] / 100.0)
    sr = srate(rep)
    useful = (r.get("wfa_pay_useful_orig") or 0) + (r.get("wfa_pay_useful_copy") or 0)
    lpc = r.get("left_preconv")
    return {
        "rep": int(rep.name[3:]), "rc": r.get("rc"), "lpc_s": lpc / 1000.0 if lpc else lpc,
        "tuzaksiz": bool(lpc) and 0 < lpc <= 10_000,
        "sr": sr / need if sr is not None else None, "sw95": r.get("sender_wait_p95_s"),
        "p95": r.get("conf_p95_s"), "deliv_D": r.get("deliv_D"),
        "deliv_10": r.get("deliv_10s"), "deliv_30": r.get("deliv_30s"),
        "deliv_60": r.get("deliv_60s"), "n": r.get("n_msgs"), "srv_delivered": r.get("srv_delivered"),
        "wall_s": r.get("run_wall_s"), "state_end": r.get("state_end"),
        "probe_up": r.get("probe_up"), "recover_arq": r.get("recover_arq"),
        "transitions": r.get("transitions"),
        "tx": r.get("tx_total_B"), "tx_data": r.get("tx_data_B"), "tx_retx": r.get("tx_retx_B"),
        "tx_dup": r.get("tx_dup_B"), "tx_fec": r.get("tx_fec_B"), "tx_ctrl": r.get("tx_ctrl_B"),
        "retx_n": r.get("retx"), "tlp_n": r.get("tail_probes"), "dup_n": r.get("dup"),
        "useful_B": useful, "wfa_total": r.get("wfa_total"),
        "useful_over_tx": useful / r["tx_total_B"] if r.get("tx_total_B") else None,
        "useful_over_l2": useful / r["wfa_total"] if r.get("wfa_total") else None,
        "redund_copy_B": r.get("wfa_pay_redund_copy"), "lost_orig_B": r.get("wfa_pay_redund_orig"),
        "useful_copy_B": r.get("wfa_pay_useful_copy"),
        "loss_fwd_cap": (r["wf_lost_pkts"] / r["wfa_pkts"]) if r.get("wfa_pkts") else None,
        "btlbw_end": r.get("est_btlbw"), "pace_end": r.get("est_pace"),
    }


# ── tablolar ──────────────────────────────────────────────────────────────────────────────────

def arms_of(d, cell):
    return [a for a in ARM_ORDER if a in d.get(cell, {})]


def t_eka2_a(d, cell, windows=True):
    w_hdr = ("| PTT penceresi: izin/sunulan | PTT penceresi: tel/sunulan | PTT penceresi: yeni veri/sunulan "
             "| PTT penceresi retx payı ") if windows else ""
    L = [f"#### {cell} — emisyon ayrıştırması (koşu medyanı; parantezde min–maks)", "",
         "| kol | n | izin/sunulan (oku.py) " + w_hdr +
         "| koşu: yeni veri payı | koşu: retx payı | retx içinde sonda (tlp/rtx) "
         "| kopya adet | parite adet | datagram kapanış farkı | faydalı/gönderilen | 1 − p_ileri |",
         "|---|---|---" + ("|---|---|---|---" if windows else "") + "|---|---|---|---|---|---|---|---|"]
    for a in arms_of(d, cell):
        rs = d[cell][a]
        win = [w for r in rs for w in r["wins"] if w["offered_Bps"]]
        wcols = ""
        if windows:
            wcols = (
                f"{fmt(med([w['pr_med'] / w['offered_Bps'] for w in win if w['pr_med']]))} | "
                f"{fmt(med([w['emit_Bps'] / w['offered_Bps'] for w in win]))} "
                f"({rng([w['emit_Bps'] / w['offered_Bps'] for w in win])}) | "
                f"{fmt(med([w['new_Bps'] / w['offered_Bps'] for w in win]))} "
                f"({rng([w['new_Bps'] / w['offered_Bps'] for w in win])}) | "
                f"{fmt(med([w['retx_share'] for w in win]))} | ")
        L.append(
            f"| {show(a)} | {len(rs)} | {fmt(med([r['oku_ratio'] for r in rs]))} | " + wcols +
            f"{fmt(med([r['new_data_B'] / r['txb'] for r in rs]))} | "
            f"{fmt(med([r['rtxb'] / r['txb'] for r in rs]))} ({rng([r['rtxb'] / r['txb'] for r in rs])}) | "
            f"{fmt(med([r['tlp'] / r['rtx'] if r['rtx'] else None for r in rs]))} | "
            f"{fmt(med([r['dup'] for r in rs]), 0)} | {fmt(max(r['par'] for r in rs), 0)} | "
            f"{rng([r['closure_dgrams'] for r in rs], 0)} | "
            f"{fmt(med([r['useful_over_sent'] for r in rs]))} ({rng([r['useful_over_sent'] for r in rs])}) | "
            f"{fmt(med([1 - r['loss_fwd'] if r['loss_fwd'] is not None else None for r in rs]))} |")
    return L


def t_eka2_a_win(d, cell):
    L = [f"#### {cell} — PTT pencereleri, mutlak (B/s, mühürlü UDP yükü; koşu × basış medyanı)", "",
         "| kol | pencere sayısı | sunulan (pencerede kabul edilen birimler + 33 B) | pacer izni `pr` | "
         "tele çıkan | bunun yeniden iletimi | yeni veri (+ kopya) | pencerede rtx / tlp / kopya (medyan adet) |",
         "|---|---|---|---|---|---|---|---|"]
    for a in arms_of(d, cell):
        win = [w for r in d[cell][a] for w in r["wins"]]
        L.append(f"| {show(a)} | {len(win)} | {fmt(med([w['offered_Bps'] for w in win]), 0)} | "
                 f"{fmt(med([w['pr_med'] for w in win]), 0)} | {fmt(med([w['emit_Bps'] for w in win]), 0)} | "
                 f"{fmt(med([w['emit_Bps'] * (w['retx_share'] or 0) for w in win]), 0)} | "
                 f"{fmt(med([w['new_Bps'] for w in win]), 0)} | "
                 f"{fmt(med([w['d_rtx'] for w in win]), 0)} / {fmt(med([w['d_tlp'] for w in win]), 0)} / "
                 f"{fmt(med([w['d_dup'] for w in win]), 0)} |")
    return L


def t_eka2_b(d):
    L = [f"#### {MANSET} — emisyon ayrıştırması (KP-CLIENT UDP yükü; yakalama sınıfları L2)", "",
         "| kol | küme | n | gönderim/gereken [3, 28) s | yeni veri payı | retx payı | kopya payı | "
         "retx adet (sonda) | faydalı/UDP gönderilen | faydalı/L2 enjekte | gereksiz kopya / L2 | "
         "kayıp orijinal / L2 | p_ileri (yakalama) |",
         "|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for a in arms_of(d, MANSET):
        rs = d[MANSET][a]
        for name, sel in (("bütün", rs), ("tuzaksız", [r for r in rs if r["tuzaksiz"]]),
                          ("tuzaklı", [r for r in rs if not r["tuzaksiz"]])):
            if not sel:
                continue
            L.append(
                f"| {show(a)} | {name} | {len(sel)} | {fmt(med([r['sr'] for r in sel]))} | "
                f"{fmt(med([r['tx_data'] / r['tx'] for r in sel]))} | "
                f"{fmt(med([r['tx_retx'] / r['tx'] for r in sel]))} | "
                f"{fmt(med([r['tx_dup'] / r['tx'] for r in sel]))} | "
                f"{fmt(med([r['retx_n'] for r in sel]), 0)} ({fmt(med([r['tlp_n'] for r in sel]), 0)}) | "
                f"{fmt(med([r['useful_over_tx'] for r in sel]))} | {fmt(med([r['useful_over_l2'] for r in sel]))} | "
                f"{fmt(med([r['redund_copy_B'] / r['wfa_total'] for r in sel]))} | "
                f"{fmt(med([r['lost_orig_B'] / r['wfa_total'] for r in sel]))} | "
                f"{fmt(med([r['loss_fwd_cap'] for r in sel]))} |")
    return L


def why_of(r):
    """Eksik koşunun kısa nedeni (ölçülen alanlardan)."""
    parts = []
    if r["missing"]:
        parts.append("bob'da eksik " + ", ".join(f"{c} {b}/{n}" for c, (n, b) in sorted(r["missing"].items())))
    if r["file"] and r["bob_file_ok"] and not r["alice_receipt"]:
        parts.append(f"dosya bob'da {fmt(r['file_bob_s'], 0)} s; alice {r['file_why']} @ "
                     f"{fmt(r['alice_fail_after_s'], 1)} s")
    elif r["file"] and not r["bob_file_ok"]:
        s = f"dosya bob'da yok; alice {r['file_why']} @ {fmt(r['alice_fail_after_s'], 1)} s"
        if r["bob_file_fail"]:
            s += f"; bob {r['bob_file_fail'][0]} @ {fmt(r['bob_file_fail'][1], 0)} s"
        parts.append(s)
    return f"rep{r['rep']}: " + "; ".join(parts)


def t_eka3(d):
    L = ["#### A hücreleri — koşu başına tamamlanma", "",
         "| hücre | kol | n | rc = 0 | alice `complete` | dosya: bob aldı | dosya: alice alındısı (≤ 60 s) "
         "| PTT kapandı | metin/sinyal tam | **eksik koşu** | eksik koşular (rep: neden) |",
         "|---|---|---|---|---|---|---|---|---|---|---|"]
    for cell in (ANALOG, DOSYASIZ):
        for a in arms_of(d, cell):
            rs = d[cell][a]
            bad = [r for r in rs if not r["complete"] or (r["file"] and not r["alice_receipt"])]
            why = "; ".join(why_of(r) for r in bad)
            fileq = lambda k: (f"{sum(1 for r in rs if r[k])}/{len(rs)}" if rs[0]["file"] else "—")  # noqa: E731
            L.append(
                f"| {cell} | {show(a)} | {len(rs)} | {sum(1 for r in rs if r['rc'] == '0')}/{len(rs)} | "
                f"{sum(1 for r in rs if r['alice_result'] == 'complete')}/{len(rs)} | "
                f"{fileq('bob_file_ok')} | {fileq('alice_receipt')} | "
                f"{sum(1 for r in rs if r['ptt_closed'])}/{len(rs)} | "
                f"{sum(1 for r in rs if not r['missing'])}/{len(rs)} | **{len(bad)}** | {why or '—'} |")
    L += ["", "#### B hücreleri — koşu başına tamamlanma", "",
          "| hücre | kol | n | rc = 0 | koşu sonunda bütün mesajlar alıcıda | teslim@D = 1 | "
          "**eksik koşu** (alıcıda eksik mesaj) | eksik koşular (rep: alıcıda / sunulan) |",
          "|---|---|---|---|---|---|---|---|"]
    for cell in (MANSET, L0, R25):
        for a in arms_of(d, cell):
            rs = d[cell][a]
            bad = [r for r in rs if (r["srv_delivered"] or 0) < (r["n"] or 0)]
            L.append(
                f"| {cell} | {show(a)} | {len(rs)} | {sum(1 for r in rs if r['rc'] == 0)}/{len(rs)} | "
                f"{len(rs) - len(bad)}/{len(rs)} | {sum(1 for r in rs if r['deliv_D'] == 1.0)}/{len(rs)} | "
                f"**{len(bad)}** | "
                + ("; ".join(f"rep{r['rep']}: {r['srv_delivered']}/{r['n']}" for r in bad) or "—") + " |")
    return L


def t_eka3_cause(d):
    L = [f"#### {ANALOG} — eksik koşular: Startup çıkışı ve hız kilidi", "",
         "| kol | rep | Startup sonrası ilk durum @ s | o andaki BtlBw (B/s) | koşu BtlBw maks | "
         "pacer `pr` medyanı [10, 60) s | tel [5, 65) s (B/s) | ProbeUp girişi | rtx (koşu) | "
         "bob'da teslim birimi (dosya parçası / toplam) | sıralı serbest bırakma (sonraki) | koşu sonu Transport kuyruğu |",
         "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    ok = {}
    for a in arms_of(d, ANALOG):
        for r in d[ANALOG][a]:
            if r["complete"] and r["alice_receipt"]:
                ok.setdefault(a, []).append(r)
                continue
            L.append(f"| {show(a)} | {r['rep']} | "
                     + (f"{r['startup_exit_state']} @ {fmt(r['startup_exit_s'], 1)}"
                        if r["startup_exit_s"] is not None else "yok") +
                     f" | {fmt(r['bw_at_exit'])} | {r['bw_max']} | {fmt(r['pr_med_10_60'], 0)} | "
                     f"{fmt(r['wire_5_65_Bps'], 0)} | {r['probe_up']} | {r['rtx']} | "
                     f"{r['bob_units']} ({r['bob_file_units']}/{r['file_units']}) | {r['bob_release_next']} | {r['txq_end']} |")
    L += ["", "Tamamlanan koşular (karşılaştırma; aralık min–maks):", "",
          "| kol | n | Startup sonrası ilk durum anı (s) | o andaki BtlBw (B/s) | pacer `pr` medyanı [10, 60) s | "
          "tel [5, 65) s (B/s) |", "|---|---|---|---|---|---|"]
    for a, rs in ok.items():
        L.append(f"| {show(a)} | {len(rs)} | {rng([r['startup_exit_s'] for r in rs], 1)} | "
                 f"{rng([r['bw_at_exit'] for r in rs], 0)} | {rng([r['pr_med_10_60'] for r in rs], 0)} | "
                 f"{rng([r['wire_5_65_Bps'] for r in rs], 0)} |")
    return L


def t_eka4(d):
    L = [f"#### {MANSET} — koşu başına (tuzaklı = PreConverge çıkışı yok ya da > 10 s; oku.py R1 tanımı)", "",
         "| kol | rep | PreConverge çıkışı (s) | tuzak | gönderim/gereken | sw95 (s) | teslim p95 (s) | "
         "≤ 10 s | ≤ 30 s | ≤ 60 s | koşu sonunda alıcıda | süre (s) | son durum | BtlBw son (B/s) |",
         "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for a in arms_of(d, MANSET):
        for r in d[MANSET][a]:
            t = "" if r["tuzaksiz"] else "**tuzaklı**"
            L.append(f"| {show(a)} | {r['rep']} | {fmt(r['lpc_s'], 1)} | {t} | {fmt(r['sr'])} | "
                     f"{fmt(r['sw95'])} | {fmt(r['p95'])} | {fmt(r['deliv_10'])} | {fmt(r['deliv_30'])} | "
                     f"{fmt(r['deliv_60'])} | {r['srv_delivered']}/{r['n']} | {fmt(r['wall_s'], 1)} | "
                     f"{r['state_end']} | {fmt(r['btlbw_end'], 0)} |")
    L += ["", "#### Dışlamalı ve dışlamasız medyanlar", "",
          "| kol | küme | n | gönderim/gereken med | sw95 med | sw95 9. sıra | teslim p95 med | ≤ 60 s med | "
          "≤ 60 s en kötü |", "|---|---|---|---|---|---|---|---|---|"]
    for a in arms_of(d, MANSET):
        rs = d[MANSET][a]
        for name, sel in (("bütün", rs), ("tuzaksız", [r for r in rs if r["tuzaksiz"]]),
                          ("tuzaklı", [r for r in rs if not r["tuzaksiz"]])):
            if not sel:
                continue
            sw = sorted(r["sw95"] for r in sel if r["sw95"] is not None)
            L.append(f"| {show(a)} | {name} | {len(sel)} | {fmt(med([r['sr'] for r in sel]))} | "
                     f"{fmt(med(sw))} | {fmt(sw[8]) if len(sw) >= 9 else '—'} | "
                     f"{fmt(med([r['p95'] for r in sel]))} | {fmt(med([r['deliv_60'] for r in sel]))} | "
                     f"{fmt(min(r['deliv_60'] for r in sel))} |")
    return L


def resid(r):
    """Geçiş sayacının PreConverge→Startup (1), RecoverArq giriş+çıkış (2 × giriş; son durum RecoverArq
    ise 1 eksik) ve sondaki Idle (1) ile açıklanmayan kalanı. 0 = Drain/Steady/ProbeUp/DrainRtprop'a
    hiç geçilmedi [türetilmiş; sayaçlar analiz_b KP-ACTIVATION]."""
    if r["transitions"] is None:
        return None
    return (r["transitions"] - (1 if r["lpc_s"] else 0) - 2 * (r["recover_arq"] or 0)
            + (1 if r["state_end"] == "RecoverArq" else 0) - (1 if r["state_end"] == "Idle" else 0))


def t_eka5(d):
    L = [f"#### {R25} — koşu başına durum (N = 10 + 10)", "",
         "| kol | rep | PreConverge çıkışı (s; 0 = çıkılmadı) | son durum | geçiş | RecoverArq girişi | "
         "ProbeUp girişi | açıklanmayan geçiş | teslim@60 | gönderim (B) |",
         "|---|---|---|---|---|---|---|---|---|---|"]
    for a in arms_of(d, R25):
        for r in d[R25][a]:
            L.append(f"| {show(a)} | {r['rep']} | {fmt(r['lpc_s'], 1)} | {r['state_end']} | "
                     f"{fmt(r['transitions'], 0)} | {fmt(r['recover_arq'], 0)} | {fmt(r['probe_up'], 0)} | "
                     f"{fmt(resid(r), 0)} | {fmt((r['deliv_D'] or 0) * (r['n'] or 0), 0)} | {r['tx']} |")
    L += ["", "#### ProbeUp girişi — hücre × kol (koşu sayısı: ProbeUp'a hiç girmeyen / n; medyan)", "",
          "| hücre | kol | ProbeUp = 0 olan koşu | medyan | maks |", "|---|---|---|---|---|"]
    for cell in (MANSET, L0, R25, ANALOG, DOSYASIZ):
        for a in arms_of(d, cell):
            pu = [r["probe_up"] or 0 for r in d[cell][a]]
            L.append(f"| {cell} | {show(a)} | {sum(1 for x in pu if x == 0)}/{len(pu)} | "
                     f"{fmt(med(pu), 0)} | {fmt(max(pu), 0)} |")
    return L


def hyper_tail(n_a, n_b, k_total, k_a):
    """Tek yönlü kesin (hipergeometrik) olasılık: n_a + n_b koşuda k_total olay varken a grubuna
    ≥ k_a olay düşmesi. Betimsel; N = 10'da güç düşük (deney-istatistiği kuralı: küçük n kesin test)."""
    tot = math.comb(n_a + n_b, k_total)
    return sum(math.comb(n_a, i) * math.comb(n_b, k_total - i)
               for i in range(k_a, min(n_a, k_total) + 1)) / tot


def t_exact(d):
    an = d.get(ANALOG, {})
    lock = {a: sum(1 for r in an[a] if r["missing"] and r["file"] and not r["bob_file_ok"]) for a in an}
    base = [a for a in ("K0", "K0r") if a in an]
    e1 = [a for a in ("K1", "K2", "K2mc", "K2me") if a in an]
    k_base, k_e1 = sum(lock[a] for a in base), sum(lock[a] for a in e1)
    n_base, n_e1 = sum(len(an[a]) for a in base), sum(len(an[a]) for a in e1)
    ms = d.get(MANSET, {})
    trap = {a: sum(1 for r in ms[a] if not r["tuzaksiz"]) for a in ms}
    L = ["#### Kesin olasılıklar (tek yönlü hipergeometrik; betimsel)", ""]
    L.append(f"- Analog kilit deseni (bob yalnız ısınma metinleri, dosya yok): K0+K0r {k_base}/{n_base}, "
             f"E1 kolları {k_e1}/{n_e1}; olayların hepsinin K0+K0r'ye düşme olasılığı "
             f"p = {hyper_tail(n_base, n_e1, k_base + k_e1, k_base):.4f}.")
    if "K0r" in lock and "K0" in lock:
        L.append(f"- Aynı desen K0r {lock['K0r']}/{len(an['K0r'])} ↔ K0 {lock['K0']}/{len(an['K0'])}: "
                 f"p = {hyper_tail(len(an['K0r']), len(an['K0']), lock['K0r'] + lock['K0'], lock['K0r']):.3f}.")
    for a in ("K2", "K2mc"):
        if a in trap and "K0" in trap:
            L.append(f"- Manşet PreConverge tuzağı {show(a)} {trap[a]}/{len(ms[a])} ↔ K0 {trap['K0']}/{len(ms['K0'])}: "
                     f"p = {hyper_tail(len(ms[a]), len(ms['K0']), trap[a] + trap['K0'], trap[a]):.3f}.")
    return L


def checks(d):
    L = ["#### Ölçüm denetimleri", ""]
    ov = sorted({r["seal_overhead"] for c in (ANALOG, DOSYASIZ) for a in d.get(c, {})
                 for r in d[c][a] if r["seal_overhead"] is not None})
    L.append(f"- İlk emisyonda `txb` − birim uzunluğu (A hücreleri, bütün koşular): {ov} B "
             f"(beklenen {SEAL_PLUS_HDR}).")
    cl = sorted({r["closure_dgrams"] for c in (ANALOG, DOSYASIZ) for a in d.get(c, {}) for r in d[c][a]})
    L.append(f"- Datagram kapanışı `txd` − (ilk iletilen birim + rtx + kopya + parite), bütün A koşuları: "
             f"{cl} (0 = başka sınıf datagram yok: CS-probe/PARAM tele çıkmadı).")
    return L


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--ham", default=str(HERE.parent / "ham" / "g2"))
    ap.add_argument("--kok")
    ap.add_argument("--json")
    args = ap.parse_args()
    sys.path.insert(0, str(F0B))
    import analiz_b  # noqa: E402
    import fizik  # noqa: E402
    geoms = fizik.by_cell()
    d = {}
    with tempfile.TemporaryDirectory() as tmp:
        for cd, base, arm in cell_dirs(args, tmp):
            reps = sorted((cd / arm).glob("rep*"), key=lambda q: int(q.name[3:]))
            if base.startswith("a_"):
                d.setdefault(base, {})[arm] = [a_run(r) for r in reps]
            else:
                d.setdefault(base, {})[arm] = [b_run(r, geoms[base], arm, analiz_b) for r in reps]
    out = ["### EKA-2", ""] + t_eka2_a(d, DOSYASIZ) + [""] + t_eka2_a_win(d, DOSYASIZ) + [""] \
        + t_eka2_a(d, ANALOG, windows=False) + [""] + t_eka2_b(d) + [""] \
        + ["### EKA-3", ""] + t_eka3(d) + [""] + t_eka3_cause(d) + [""] \
        + ["### EKA-4", ""] + t_eka4(d) + [""] + ["### EKA-5", ""] + t_eka5(d) + [""] + t_exact(d) + [""] \
        + checks(d)
    print("\n".join(out))
    if args.json:
        with open(args.json, "w", encoding="utf-8") as f:
            json.dump(d, f, ensure_ascii=False, indent=1, default=str)
    return 0


if __name__ == "__main__":
    sys.exit(main())
