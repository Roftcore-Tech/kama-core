#!/usr/bin/env python3
"""kama-target-net-v2 — BLOK yürütücüsü (yalnız stdlib). `unshare -rnm` İÇİNDE koşar (tnet_campaign.py çağırır).

Bir manifest bloğunu (aynı hücre, ardışık 10 rep × 5 kol) KENDİ 3-netns topolojisinde SERİ koşar
(kama-vs-tcp-v1 kvt_exec.py'nin blok-başına hâli). Fail-closed:
  * blok başında bağlantı kapısı: yalnız-gecikme netem + ping (RTT ∈ [2·OWD, 2·OWD·1,15 + 3 ms]) —
    tutmazsa topoloji yeniden kurulur (≤ 3); yine tutmazsa satırlar NETEM-SKIP olarak LOGLANIR, üretilmez;
  * her satırda netem önce SİLİNİR sonra EKLENİR (`replace` belirtilmeyen parametreyi korur — v1 EK-1)
    ve `tc -j` dökümü beklenen dize + tohumla SAYISAL karşılaştırılır; tutmazsa satır üretilmez;
  * offload (TSO/GSO/GRO) topoloji kurulumunda ioctl ile kapatılır ve GERİ OKUNUR.
Satır başına: tc -s sayaçları (iki yön, önce/sonra), va0 bayt sayaçları, loadavg, satır duvar süresi.
Satır ortak sonuç CSV'sine `flock` altında TEK satır olarak eklenir; sonuçta bulunan run_id atlanır
(devam-edilebilir; üstüne yazılmaz).

Kullanım: tnet_exec.py --manifest M --block B --out OUT [--results CSV] [--bench P] [--tcp P] [--no-netem] [--only RE]
  --no-netem: YALNIZ bulut/duman — netem uygulanmaz/doğrulanmaz, satır status=SMOKE-NO-NETEM yazılır
              (analizör bu satırları kampanya satırı saymaz).
"""
import csv
import fcntl
import json
import os
import re
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
A_IP, B_IP = "10.10.0.1", "10.10.1.1"
T = {"MID": None, "CHLB": None}
CHILDREN = []   # o anki satırın sunucu/istemci süreçleri (sinyalde öksüz kalmasın)
COLS = ["run_id", "replica", "group", "cell", "loss_pct", "loss_dir", "rtt_class", "owd_ms", "rate_fwd", "rate_rev",
        "workload", "msgs", "payload", "arm", "mode", "rep", "horizon_ms", "deadline_ms", "status", "delivered",
        "complete_horizon", "complete_deadline", "t_complete_ms", "wire_bytes", "wire_tx_bytes", "wire_rx_bytes",
        "fwd_pkts", "fwd_drops", "rev_pkts", "rev_drops", "netem_limit", "netem_seed_fwd", "netem_seed_rev",
        "loadavg1", "nproc", "load_flag", "warmup_residue", "setup_ns", "connect_failed", "profile_id",
        "profile_delta", "engine", "kama_arm", "bin_commit", "kama_fec_bytes", "kama_retx_bytes", "rc",
        "row_wall_s", "block", "worker_pid", "seq_no", "unix_ts"]
# Kama kollarının şerit kipi (ONKAYIT §3.1): küçük iş yükü = metin (TEXT), büyük = dosya (FILE).
LANE_OF = {"small": "text", "large": "file"}
# Ürün profilinden BEKLENEN sapma (ONKAYIT §3.2): yalnız FEC'in statik r=0 kipi.
NOFEC_DELTA = ["fec Some((5, Some(0))) != Some((5, None))"]
LOAD_FRAC = 0.75     # load_flag = loadavg1 > 0,75 × nproc (ONKAYIT §7)
ENGINE = "cc-port"   # ürün motoru; iki uçta AÇIKÇA verilir ve satırda doğrulanır


def sh(cmd, ns=None, **kw):
    if ns:
        cmd = ["nsenter", "-t", str(T[ns]), "-n"] + cmd
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def log(f, msg):
    f.write(f"{time.strftime('%Y-%m-%dT%H:%M:%S')} {msg}\n")
    f.flush()


# ── netem doğrulaması (kama-vs-tcp-v1 kvt_netem_check.py mantığı, aynen; içe alındı) ────────────
def parse_spec(spec):
    t = spec.split()
    want = {"delay": 0.0, "loss": 0.0, "rate": 0}
    i = 0
    while i < len(t):
        k = t[i]
        if k == "delay":
            want["delay"] = float(t[i + 1].rstrip("ms")) / 1000.0
            i += 2
        elif k == "loss":
            want["loss"] = float(t[i + 1].rstrip("%")) / 100.0
            i += 2
        elif k == "rate":
            v = t[i + 1].lower()
            mul = 1
            for suf, m in (("kbit", 1000), ("mbit", 1000_000), ("bit", 1)):
                if v.endswith(suf):
                    mul = m
                    v = v[: -len(suf)]
                    break
            want["rate"] = int(float(v) * mul / 8)   # tc JSON: bayt/s (aşağı yuvarlanır)
            i += 2
        else:
            raise ValueError(f"bilinmeyen netem belirteci: {k}")
    return want


def check_netem(qjson, spec, seed):
    """(ok, limit, açıklama). qjson = `tc -s -j qdisc show dev X` çıktısı (str)."""
    q = [x for x in json.loads(qjson or "[]") if x.get("kind") == "netem"]
    if len(q) != 1:
        return False, -1, "netem qdisc yok/çoklu"
    o = q[0].get("options", {})
    want = parse_spec(spec)
    got = {"delay": float((o.get("delay") or {}).get("delay", 0.0)),
           "loss": float((o.get("loss-random") or {}).get("loss", 0.0)),
           "rate": int((o.get("rate") or {}).get("rate", 0))}
    if o.get("loss-gemodel"):
        return False, -1, "beklenmeyen gemodel"
    ok = (abs(got["delay"] - want["delay"]) < 1e-6 and abs(got["loss"] - want["loss"]) < 1e-4
          and abs(got["rate"] - want["rate"]) <= max(1, want["rate"] // 1000))
    if seed is not None and str(o.get("seed")) != str(seed):
        return False, -1, f"seed uyuşmazlığı: beklenen={seed} gerçekleşen={o.get('seed')}"
    if not ok:
        return False, -1, f"netem uyuşmazlığı: beklenen={want} gerçekleşen={got}"
    return True, int(o.get("limit", -1)), ""


# ── topoloji ───────────────────────────────────────────────────────────────────────────────────
def topo_up(lg):
    for attempt in range(1, 4):
        if T["MID"]:
            sh(["bash", os.path.join(HERE, "tnet_topo.sh"), "down", str(T["MID"]), str(T["CHLB"])])
        r = sh(["bash", os.path.join(HERE, "tnet_topo.sh"), "up"])
        kv = dict(x.split("=", 1) for x in r.stdout.split() if "=" in x)
        if "MID" not in kv or not kv["MID"]:
            log(lg, f"TOPO-RETRY {attempt}: {r.stderr.strip()[:200]}")
            time.sleep(0.5)
            continue
        T["MID"], T["CHLB"] = int(kv["MID"]), int(kv["CHLB"])
        off = [sh(["python3", os.path.join(HERE, "tnet_offload.py"), "va0"]),
               sh(["python3", os.path.join(HERE, "tnet_offload.py"), "ma0"], ns="MID"),
               sh(["python3", os.path.join(HERE, "tnet_offload.py"), "mb0"], ns="MID"),
               sh(["python3", os.path.join(HERE, "tnet_offload.py"), "vb0"], ns="CHLB")]
        if all(o.returncode == 0 for o in off):
            log(lg, f"TOPO-UP ipv6={kv.get('IPV6', '')} offload=OFF(verified) "
                    + " | ".join(o.stdout.strip() for o in off))
            return True
        log(lg, f"OFFLOAD-FAIL {attempt}: " + " | ".join((o.stdout + o.stderr).strip() for o in off))
    return False


def topo_down():
    if T["MID"]:
        sh(["bash", os.path.join(HERE, "tnet_topo.sh"), "down", str(T["MID"]), str(T["CHLB"])])


def set_netem(fwd, rev):
    sh(["tc", "qdisc", "del", "dev", "mb0", "root"], ns="MID")
    sh(["tc", "qdisc", "del", "dev", "ma0", "root"], ns="MID")
    a = sh(["tc", "qdisc", "add", "dev", "mb0", "root", "netem"] + fwd.split(), ns="MID")
    b = sh(["tc", "qdisc", "add", "dev", "ma0", "root", "netem"] + rev.split(), ns="MID")
    return a.returncode == 0 and b.returncode == 0


def qdump(dev):
    return sh(["tc", "-s", "-j", "qdisc", "show", "dev", dev], ns="MID").stdout.strip() or "[]"


def gate(owd_ms, lg, no_netem):
    """Bağlantı kapısı: yalnız-gecikme netem altında ping; RTT ∈ [2·OWD, 2·OWD·1,15 + 3 ms]."""
    owd = int(owd_ms)
    if no_netem:
        owd = 0
    elif not set_netem(f"delay {owd}ms", f"delay {owd}ms"):
        log(lg, "GATE netem uygulanamadı")
        return False
    w = max(2, int(2 * owd / 1000) + 2)
    r = sh(["ping", "-c", "3", "-i", "0.3", "-W", str(w), B_IP])
    rtts = [float(x.split("time=")[1].split()[0]) for x in r.stdout.splitlines() if "time=" in x]
    lo, hi = 2 * owd, 2 * owd * 1.15 + 3
    ok = len(rtts) >= 2 and all(lo - 0.5 <= t <= hi for t in rtts)
    log(lg, f"GATE owd={owd}ms ping_rtts={rtts} band=[{lo},{hi:.1f}] -> {'OK' if ok else 'FAIL'}")
    return ok


def wait_ready(path, needle, timeout=4.0):
    t = time.time()
    while time.time() - t < timeout:
        try:
            if needle in open(path, errors="replace").read():
                return True
        except OSError:
            pass
        time.sleep(0.02)
    return False


# ── kollar (ONKAYIT §3) ─────────────────────────────────────────────────────────────────────────
def commands(m, d, port, bench, tcp):
    arm, n, pay, hz = m["arm"], m["msgs"], m["payload"], int(m["horizon_s"])
    pre = ["nsenter", "-t", str(T["CHLB"]), "-n"]
    common = ["--msgs", n, "--payload", pay, "--submit-interval-ms", "0", "--max-deadline-s", str(hz),
              "--warmup-msgs", m["warmup_msgs"], "--seed", m["rep"], "--repeat", m["rep"],
              "--fwd-impair", m["netem_fwd"], "--rev-impair", m["netem_rev"],
              "--manifest", os.path.join(d, "manifest.json"), "--out", os.path.join(d, "out.jsonl")]
    if arm in ("kama-port", "kama-port-nofec"):
        fec = ["--fec-mode", "static", "--fec-r", "0"] if arm == "kama-port-nofec" else []
        srv = pre + [bench, "serve", "--bind", f"{B_IP}:{port}", "--transport", "kama-product",
                     "--payload", pay] + fec
        cli = [bench, "client", "--server", f"{B_IP}:{port}", "--transport", "kama-product",
               "--lane", LANE_OF[m["workload"]]] + fec + common
        return srv, cli, ("KP-MANIFEST", 4.0)
    if arm in ("quinn", "quinn-bbr"):
        cc = "bbr" if arm == "quinn-bbr" else "cubic"
        srv = pre + [bench, "serve", "--bind", f"{B_IP}:{port}", "--transport", "quinn", "--cc", cc]
        cli = [bench, "client", "--server", f"{B_IP}:{port}", "--transport", "quinn", "--cc", cc] + common
        return srv, cli, (None, 1.0)
    if arm == "tcp":   # kama-vs-tcp-v1 tcp-std kolu, DEĞİŞMEDEN
        srv = pre + [tcp, "server", "--bind", f"{B_IP}:{port}"]
        cli = [tcp, "client", "--server", f"{B_IP}:{port}", "--msgs", n, "--payload", pay,
               "--submit-interval-ms", "0", "--warmup-msgs", m["warmup_msgs"], "--horizon-ms", str(hz * 1000),
               "--mode", m["mode"], "--run-id", m["run_id"], "--out", os.path.join(d, "out.jsonl")]
        return srv, cli, ("listening", 4.0)
    raise SystemExit(f"bilinmeyen kol: {arm}")


def run_row(m, d, port, bench, tcp, lg):
    srv, cli, ready = commands(m, d, port, bench, tcp)
    env = dict(os.environ)
    env["KAMA_TRANSPORT_ENGINE"] = ENGINE   # iki uçta açık; miras alınan değer ölçümü değiştiremez
    json.dump({"server": srv, "client": cli}, open(os.path.join(d, "cmd.json"), "w"))
    srv_err, cli_err = open(os.path.join(d, "s.err"), "w"), open(os.path.join(d, "c.err"), "w")
    sp = subprocess.Popen(srv, stdout=subprocess.DEVNULL, stderr=srv_err, env=env)
    CHILDREN.append(sp)
    if ready[0]:
        if not wait_ready(os.path.join(d, "s.err"), ready[0], ready[1]):
            log(lg, f"SERVER-NOT-READY {m['run_id']} (satır yine yazılır; çıktı yoksa ERROR-NO-RESULT)")
    else:
        time.sleep(ready[1])
    # İstemci tavanı: quinn el sıkışması ≤ H + ısıtma ≤ 10,3 s + pencere H + pay (ONKAYIT §5).
    budget = 2 * int(m["horizon_s"]) + 90
    cp = subprocess.Popen(cli, stdout=subprocess.DEVNULL, stderr=cli_err, env=env)
    CHILDREN.append(cp)
    try:
        rc = cp.wait(timeout=budget)
    except subprocess.TimeoutExpired:
        cp.kill()
        cp.wait()
        rc = 124
    sp.send_signal(signal.SIGTERM)
    try:
        sp.wait(timeout=3)
    except subprocess.TimeoutExpired:
        sp.kill()
        sp.wait()
    CHILDREN.clear()
    srv_err.close()
    cli_err.close()
    return rc


# ── satır ──────────────────────────────────────────────────────────────────────────────────────
def qd(snap):
    q = [x for x in snap if x.get("kind") == "netem"]
    return (q[0].get("packets", 0), q[0].get("drops", 0)) if q else (0, 0)


def link(snap):
    s = snap[0]["stats64"]
    return s["tx"]["bytes"], s["rx"]["bytes"]


def kp_commit(d):
    try:
        for line in open(os.path.join(d, "c.err"), errors="replace"):
            if line.startswith("KP-MANIFEST role=client"):
                for tok in line.split():
                    if tok.startswith("commit="):
                        return tok[len("commit="):]
    except OSError:
        pass
    return ""


def assemble(m, d, snap):
    """Ham koşu → tek CSV satırı (dict). HESAP yok: yalnız ölçülen + manifestten gelen."""
    out = dict.fromkeys(COLS, "")
    for k in ("run_id", "replica", "group", "cell", "loss_pct", "loss_dir", "rtt_class", "owd_ms", "rate_fwd",
              "rate_rev", "workload", "msgs", "payload", "arm", "mode", "rep", "deadline_ms", "block"):
        out[k] = m[k]
    out["horizon_ms"] = int(m["horizon_s"]) * 1000
    n = int(m["msgs"])
    status, delivered, t_complete = "OK", 0, ""
    meta, ev = None, []
    p = os.path.join(d, "out.jsonl")
    if os.path.exists(p):
        for line in open(p):
            if not line.strip():
                continue
            j = json.loads(line)
            if "meta" in j:
                meta = j["meta"]
            elif "event" in j:
                ev.append(j["event"])
    if meta is None or len(ev) != n:
        status = "ERROR-NO-RESULT"
    else:
        off = int(meta.get("events_epoch_offset_ns", 0) or 0)
        dl = [e["delivered_ns"] - off for e in ev if e.get("delivered_ns") is not None]
        delivered = len(dl)
        if delivered == n:
            t_complete = f"{max(dl) / 1e6:.3f}"
        out["profile_id"] = meta.get("profile_id", "")
        out["profile_delta"] = json.dumps(meta.get("profile_delta", []), ensure_ascii=False)
        out["engine"] = meta.get("engine", "")
        out["kama_arm"] = meta.get("kama_arm", "")
        out["warmup_residue"] = meta.get("warmup_residue", "")
        out["setup_ns"] = meta.get("setup_ns", "")
        out["connect_failed"] = int(bool(meta.get("connect_failed", False)))
        if m["arm"].startswith("kama-port"):
            tx = meta.get("tx", {}) or {}
            out["kama_fec_bytes"] = tx.get("fec", "")
            out["kama_retx_bytes"] = tx.get("retx", "")
            out["bin_commit"] = kp_commit(d)
            delta = meta.get("profile_delta", [])
            lane_sfx = "-lane-" + LANE_OF[m["workload"]]
            if meta.get("engine") != ENGINE:
                status = "ENGINE-MISMATCH"
            elif not str(meta.get("kama_arm", "")).endswith(lane_sfx):
                status = "LANE-MISMATCH"
            elif m["arm"] == "kama-port" and (meta.get("profile_id") != "kama-v1-full" or delta):
                status = "PROFILE-MISMATCH"
            elif m["arm"] == "kama-port-nofec" and (meta.get("profile_id") != "kama-custom" or delta != NOFEC_DELTA):
                status = "PROFILE-MISMATCH"
    if snap.get("no_netem"):
        status = "SMOKE-NO-NETEM" if status == "OK" else status
    ch = int(status == "OK" and delivered == n and t_complete != "" and float(t_complete) <= out["horizon_ms"])
    cd = int(ch == 1 and float(t_complete) <= float(m["deadline_ms"]))
    fp0, fd0 = qd(snap["q_fwd_before"])
    fp1, fd1 = qd(snap["q_fwd_after"])
    rp0, rd0 = qd(snap["q_rev_before"])
    rp1, rd1 = qd(snap["q_rev_after"])
    tx0, rx0 = link(snap["l_before"])
    tx1, rx1 = link(snap["l_after"])
    la, nproc = float(snap["loadavg1"]), int(snap["nproc"])
    out.update(status=status, delivered=delivered, complete_horizon=ch, complete_deadline=cd,
               t_complete_ms=t_complete, wire_tx_bytes=tx1 - tx0, wire_rx_bytes=rx1 - rx0,
               wire_bytes=(tx1 - tx0) + (rx1 - rx0), fwd_pkts=fp1 - fp0, fwd_drops=fd1 - fd0,
               rev_pkts=rp1 - rp0, rev_drops=rd1 - rd0, netem_limit=snap["netem_limit"],
               netem_seed_fwd=m["seed_fwd"], netem_seed_rev=m["seed_rev"], loadavg1=f"{la:.2f}", nproc=nproc,
               load_flag=int(la > LOAD_FRAC * nproc), rc=snap["rc"], row_wall_s=f"{snap['wall_s']:.2f}",
               worker_pid=os.getpid(), seq_no=snap["seq_no"], unix_ts=snap["unix_ts"])
    return out


def append_row(path, row):
    """Ortak sonuç CSV'sine tek satır (flock; başlık yoksa önce başlık)."""
    with open(path, "a", newline="") as f:
        fcntl.flock(f, fcntl.LOCK_EX)
        try:
            if f.tell() == 0 and os.path.getsize(path) == 0:
                csv.writer(f, lineterminator="\n").writerow(COLS)
            csv.writer(f, lineterminator="\n").writerow([row[c] for c in COLS])
            f.flush()
            os.fsync(f.fileno())
        finally:
            fcntl.flock(f, fcntl.LOCK_UN)


def done_ids(path):
    if not os.path.exists(path):
        return set()
    with open(path, newline="") as f:
        fcntl.flock(f, fcntl.LOCK_SH)
        try:
            return {r["run_id"] for r in csv.DictReader(f)}
        finally:
            fcntl.flock(f, fcntl.LOCK_UN)


def arg(a, k, default=None):
    return a[a.index(k) + 1] if k in a else default


def on_signal(signum, _frame):
    """Koordinatör durdurunca: satırın süreçlerini ve topolojiyi (ad alanı uyku süreçleri) kapat, çık.
    Yarım satır sonuç CSV'sine YAZILMAZ; yeniden başlatmada o satır baştan koşar."""
    for p in CHILDREN:
        try:
            p.kill()
        except OSError:
            pass
    topo_down()
    sys.exit(128 + signum)


def main():
    signal.signal(signal.SIGTERM, on_signal)
    signal.signal(signal.SIGINT, on_signal)
    a = sys.argv[1:]
    manifest, block, outdir = arg(a, "--manifest"), arg(a, "--block"), arg(a, "--out")
    bench = os.path.abspath(arg(a, "--bench", os.path.join(REPO, "target/release/bench")))
    tcp = os.path.abspath(arg(a, "--tcp", os.path.join(REPO, "baselines/target/release/tcp_adapter")))
    no_netem = "--no-netem" in a
    results = arg(a, "--results", os.path.join(outdir, "results.csv"))
    os.makedirs(os.path.join(outdir, "logs"), exist_ok=True)
    lg = open(os.path.join(outdir, "logs", block.replace("/", "__") + ".log"), "a")
    only = arg(a, "--only")   # yalnız duman: satır alt kümesi (koordinatör aynı ifadeyi geçirir)
    rows = [r for r in csv.DictReader(open(manifest))
            if r["block"] == block and (only is None or re.search(only, r["run_id"]))]
    done = done_ids(results)
    todo = [r for r in rows if r["run_id"] not in done]
    log(lg, f"EXEC-START block={block} rows={len(rows)} todo={len(todo)} kernel={os.uname().release} "
            f"tc={sh(['tc', '-V']).stdout.strip()} no_netem={no_netem} pid={os.getpid()}")
    if not todo:
        log(lg, "EXEC-DONE (yapılacak satır yok)")
        return
    if not topo_up(lg):
        log(lg, "SETUP-FAILED (topoloji kurulamadı ya da offload kapatılamadı) — blok DUR")
        sys.exit(3)
    cell_ok = False
    for _ in range(3):
        if gate(todo[0]["owd_ms"], lg, no_netem):
            cell_ok = True
            break
        topo_up(lg)
    if not cell_ok:
        for m in todo:
            log(lg, f"NETEM-SKIP {m['run_id']} (bağlantı kapısı tutmadı)")
        topo_down()
        sys.exit(4)
    port, seq, nproc = 21000, 0, os.cpu_count() or 1
    for m in todo:
        t_row = time.time()
        limit = -1
        if not no_netem:
            fwd = f"{m['netem_fwd']} seed {m['seed_fwd']}"
            rev = f"{m['netem_rev']} seed {m['seed_rev']}"
            applied = False
            for na in range(1, 5):
                if set_netem(fwd, rev):
                    vf = check_netem(qdump("mb0"), m["netem_fwd"], m["seed_fwd"])
                    vr = check_netem(qdump("ma0"), m["netem_rev"], m["seed_rev"])
                    if vf[0] and vr[0]:
                        applied, limit = True, vf[1]
                        break
                    log(lg, f"NETEM-VERIFY-FAIL {m['run_id']} try={na} fwd='{vf[2]}' rev='{vr[2]}'")
                else:
                    log(lg, f"NETEM-APPLY-FAIL {m['run_id']} try={na}")
                time.sleep(0.3)
            if not applied:
                log(lg, f"NETEM-SKIP {m['run_id']} (netem doğrulanamadı)")
                continue
        d = os.path.join(outdir, "runs", m["run_id"])
        os.makedirs(d, exist_ok=True)
        port = port + 3 if port < 60000 else 21000
        snap = {"q_fwd_before": json.loads(qdump("mb0")), "q_rev_before": json.loads(qdump("ma0")),
                "l_before": json.loads(sh(["ip", "-s", "-j", "link", "show", "va0"]).stdout)}
        rc = run_row(m, d, port, bench, tcp, lg)
        snap.update({"q_fwd_after": json.loads(qdump("mb0")), "q_rev_after": json.loads(qdump("ma0")),
                     "l_after": json.loads(sh(["ip", "-s", "-j", "link", "show", "va0"]).stdout)})
        seq += 1
        snap.update({"m": m, "rc": rc, "seq_no": seq, "unix_ts": int(time.time()), "netem_limit": limit,
                     "loadavg1": open("/proc/loadavg").read().split()[0], "nproc": nproc,
                     "no_netem": no_netem, "wall_s": time.time() - t_row})
        json.dump(snap, open(os.path.join(d, "row.json"), "w"))
        try:
            row = assemble(m, d, snap)
        except Exception as e:   # satır üretilemedi: LOGLANIR, sessiz düşme yok (yeniden başlatmada tekrar koşar)
            log(lg, f"ROW-ERROR {m['run_id']}: {e!r}"[:400])
            continue
        append_row(results, row)
        log(lg, f"ROW {m['run_id']} status={row['status']} delivered={row['delivered']}/{m['msgs']} "
                f"t={row['t_complete_ms']} rc={rc} wall={row['row_wall_s']}s")
    topo_down()
    log(lg, f"EXEC-DONE block={block} produced={seq}")


if __name__ == "__main__":
    main()
