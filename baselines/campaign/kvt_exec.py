#!/usr/bin/env python3
"""KAMA ↔ TCP kampanya YÜRÜTÜCÜSÜ (yalnız stdlib). `unshare -rnm` İÇİNDE koşar (kvt_run.sh sarar).

Donmuş manifesti SATIR SATIR, SERİ koşar. Fail-closed:
  * hücre değişiminde bağlantı kapısı: yalnız-gecikme netem + ping (RTT 2·OWD bandında) — tutmazsa
    topoloji yeniden kurulur (≤3), yine tutmazsa o satır NETEM-SKIP olarak LOGLANIR (sessiz düşürme yok);
  * her satırda netem `tc -j` dökümü beklenen spec + seed ile SAYISAL karşılaştırılır; offload durumu
    (TSO/GSO/GRO kapalı) topoloji kurulumunda ioctl ile kapatılıp GERİ OKUNARAK doğrulanır.
Satır başına: tc -s qdisc sayaçları (iki yön, önce/sonra), veth bayt sayaçları (ip -s link, va0), loadavg.
Devam-edilebilir: result.csv'de bulunan run_id'ler atlanır (yeniden koşulmaz, üstüne yazılmaz).
"""
import csv, json, os, signal, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
BENCH = os.environ.get("KVT_BENCH", os.path.join(REPO, "target/release/bench"))
TCP = os.environ.get("KVT_TCP", os.path.join(REPO, "baselines/target/release/tcp_adapter"))
ABL = os.environ.get("KVT_ABL", os.path.join(REPO, "target/release/examples/dr_arqtail_product"))
A_IP, B_IP = "10.10.0.1", "10.10.1.1"
T = {"MID": None, "CHLB": None}

def sh(cmd, ns=None, **kw):
    if ns: cmd = ["nsenter", "-t", str(T[ns]), "-n"] + cmd
    return subprocess.run(cmd, capture_output=True, text=True, **kw)

def log(f, msg):
    f.write(f"{time.strftime('%H:%M:%S')} {msg}\n"); f.flush()

def topo_up(lg):
    for attempt in range(1, 4):
        if T["MID"]:
            sh(["bash", os.path.join(HERE, "kvt_topo.sh"), "down", str(T["MID"]), str(T["CHLB"])])
        r = sh(["bash", os.path.join(HERE, "kvt_topo.sh"), "up"])
        kv = dict(x.split("=") for x in r.stdout.split() if "=" in x)
        if "MID" not in kv:
            log(lg, f"TOPO-RETRY {attempt}: {r.stderr.strip()[:200]}"); time.sleep(0.5); continue
        T["MID"], T["CHLB"] = int(kv["MID"]), int(kv["CHLB"])
        off = [sh(["python3", os.path.join(HERE, "kvt_offload.py"), "va0"]),
               sh(["python3", os.path.join(HERE, "kvt_offload.py"), "ma0"], ns="MID"),
               sh(["python3", os.path.join(HERE, "kvt_offload.py"), "mb0"], ns="MID"),
               sh(["python3", os.path.join(HERE, "kvt_offload.py"), "vb0"], ns="CHLB")]
        if all(o.returncode == 0 for o in off):
            log(lg, "TOPO-UP offload=OFF(verified) " + " | ".join(o.stdout.strip() for o in off)); return True
        log(lg, f"OFFLOAD-FAIL {attempt}: " + " | ".join((o.stdout + o.stderr).strip() for o in off))
    return False

def set_netem(fwd, rev):
    # [EK-1] `replace` aynı-tür qdisc'te DEĞİŞİKLİK yapar ve belirtilmeyen parametreyi (ör. rate) KORUR
    # (ölçüldü 2026-09-20: rate'li hücreden rate'siz hücreye geçişte 32000 B/s kaldı; fail-closed doğrulama yakaladı).
    # Bu yüzden önce SİL, sonra EKLE: her satır temiz bir netem ile başlar.
    sh(["tc", "qdisc", "del", "dev", "mb0", "root"], ns="MID"); sh(["tc", "qdisc", "del", "dev", "ma0", "root"], ns="MID")
    a = sh(["tc", "qdisc", "add", "dev", "mb0", "root", "netem"] + fwd.split(), ns="MID")
    b = sh(["tc", "qdisc", "add", "dev", "ma0", "root", "netem"] + rev.split(), ns="MID")
    return a.returncode == 0 and b.returncode == 0

def qdump(dev):
    return sh(["tc", "-s", "-j", "qdisc", "show", "dev", dev], ns="MID").stdout.strip() or "[]"

def verify(dev, spec, seed):
    r = sh(["python3", os.path.join(HERE, "kvt_netem_check.py"), qdump(dev), spec, str(seed)])
    return (r.returncode == 0, r.stdout.strip(), r.stderr.strip())

def gate(owd_ms, lg):
    """Bağlantı kapısı: yalnız-gecikme netem altında ping; RTT ∈ [2·OWD, 2·OWD·1.15 + 3 ms]."""
    d = f"delay {owd_ms}ms" if int(owd_ms) > 0 else "delay 0ms"
    if not set_netem(d, d): return False
    w = max(2, int(2 * int(owd_ms) / 1000) + 2)
    r = sh(["ping", "-c", "3", "-i", "0.3", "-W", str(w), B_IP])
    rtts = [float(x.split("time=")[1].split()[0]) for x in r.stdout.splitlines() if "time=" in x]
    lo, hi = 2 * int(owd_ms), 2 * int(owd_ms) * 1.15 + 3
    ok = len(rtts) >= 2 and all(lo - 0.5 <= t <= hi for t in rtts)
    log(lg, f"GATE owd={owd_ms}ms ping_rtts={rtts} band=[{lo},{hi:.1f}] -> {'OK' if ok else 'FAIL'}")
    return ok

def wait_ready(path, needle, timeout=4.0):
    t = time.time()
    while time.time() - t < timeout:
        try:
            if needle in open(path, errors="replace").read(): return True
        except OSError: pass
        time.sleep(0.02)
    return False

def run_row(m, d, port, lg):
    arm, n, pay, hz = m["arm"], m["msgs"], m["payload"], int(m["horizon_s"])
    srv_err, cli_err = open(os.path.join(d, "s.err"), "w"), open(os.path.join(d, "c.err"), "w")
    pre = ["nsenter", "-t", str(T["CHLB"]), "-n"]
    if arm == "kama-v1-full":
        srv = pre + [BENCH, "serve", "--bind", f"{B_IP}:{port}", "--transport", "kama-product", "--payload", pay]
        cli = [BENCH, "client", "--server", f"{B_IP}:{port}", "--transport", "kama-product", "--msgs", n,
               "--payload", pay, "--submit-interval-ms", "0", "--max-deadline-s", str(hz),
               "--warmup-msgs", m["warmup_msgs"], "--seed", m["rep"], "--repeat", m["rep"],
               "--fwd-impair", m["netem_fwd"], "--rev-impair", m["netem_rev"],
               "--manifest", os.path.join(d, "manifest.json"), "--out", os.path.join(d, "out.jsonl")]
        ready = ("KP-MANIFEST", 4.0)
    elif arm == "quinn":   # [EK-1] bench quinn kolu (quinn-reliable-stream; CUBIC; TLS 1.3 — quinn'de kapatılamaz)
        srv = pre + [BENCH, "serve", "--bind", f"{B_IP}:{port}", "--transport", "quinn"]
        cli = [BENCH, "client", "--server", f"{B_IP}:{port}", "--transport", "quinn", "--msgs", n,
               "--payload", pay, "--submit-interval-ms", "0", "--max-deadline-s", str(hz),
               "--warmup-msgs", m["warmup_msgs"], "--seed", m["rep"], "--repeat", m["rep"],
               "--fwd-impair", m["netem_fwd"], "--rev-impair", m["netem_rev"],
               "--manifest", os.path.join(d, "manifest.json"), "--out", os.path.join(d, "out.jsonl")]
        ready = (None, 1.0)
    elif arm == "tcp-std":
        srv = pre + [TCP, "server", "--bind", f"{B_IP}:{port}"]
        cli = [TCP, "client", "--server", f"{B_IP}:{port}", "--msgs", n, "--payload", pay,
               "--submit-interval-ms", "0", "--warmup-msgs", m["warmup_msgs"], "--horizon-ms", str(hz * 1000),
               "--mode", m["mode"], "--run-id", m["run_id"], "--out", os.path.join(d, "out.jsonl")]
        ready = ("listening", 4.0)
    elif arm in ("abl-off", "abl-on"):
        srv = pre + [ABL, "--role", "rx", "--bind", f"{B_IP}:{port}", "--w-units", n, "--drop-terminal", "0",
                     "--ext-horizon-ms", str(hz * 2000 + 4000)]
        cli = [ABL, "--role", "tx", "--bind", f"{A_IP}:{port + 1}", "--peer", f"{B_IP}:{port}",
               "--arm", arm[4:], "--w-units", n, "--unit-bytes", pay, "--op-horizon-ms", m["deadline_ms"],
               "--lb-ms", m["deadline_ms"], "--ext-horizon-ms", str(hz * 1000), "--run-id", m["run_id"],
               "--cell", m["cell"], "--struct", "iid", "--loss", m["loss_pct"], "--rtt", m["rtt_class"],
               "--rep", m["rep"], "--result-out", os.path.join(d, "abl.csv")]
        ready = (None, 0.3)
    else:
        raise SystemExit(f"bilinmeyen kol: {arm}")
    sp = subprocess.Popen(srv, stdout=subprocess.DEVNULL, stderr=srv_err)
    if ready[0]:
        if not wait_ready(os.path.join(d, "s.err"), ready[0], ready[1]):
            log(lg, f"SERVER-NOT-READY {m['run_id']} (devam: satır ERROR olarak kaydedilir)")
    else:
        time.sleep(ready[1])
    # istemci tavanı: ısıtma tavanı (30 s) + connect (20 s) + horizon + pay (harness kilitlenmesin)
    budget = hz + 60 + 20 + 30   # [EK-1] ısıtma: yazma tavanı 30 s + bekleme 30 s; connect 20 s; pay 30 s
    try:
        rc = subprocess.run(cli, stdout=subprocess.DEVNULL, stderr=cli_err, timeout=budget).returncode
    except subprocess.TimeoutExpired:
        rc = 124
    sp.send_signal(signal.SIGTERM)
    try: sp.wait(timeout=3)
    except subprocess.TimeoutExpired: sp.kill()
    srv_err.close(); cli_err.close()
    return rc

def main():
    manifest, outdir = sys.argv[1], sys.argv[2]
    subset = sys.argv[3] if len(sys.argv) > 3 else ""
    os.makedirs(outdir, exist_ok=True)
    result = os.path.join(outdir, "result.csv"); lg = open(os.path.join(outdir, "run.log"), "a")
    rows = list(csv.DictReader(open(manifest)))
    if subset:
        import re
        rows = [r for r in rows if re.search(subset, r["run_id"])]
    if os.environ.get("KVT_WORKER") is not None:
        rows = [r for r in rows if r.get("worker") == os.environ["KVT_WORKER"]]
    done = set()
    if os.path.exists(result):
        done = {r["run_id"] for r in csv.DictReader(open(result))}
    else:
        hdr = sh(["python3", os.path.join(HERE, "kvt_row.py"), "--header"]).stdout
        open(result, "w").write(hdr)
    log(lg, f"EXEC-START manifest={manifest} rows={len(rows)} already_done={len(done)} "
            f"kernel={os.uname().release} tc={sh(['tc','-V']).stdout.strip()}")
    if not topo_up(lg):
        log(lg, "SETUP-FAILED (offload kapatılamadı ya da topoloji kurulamadı) — DUR"); sys.exit(3)
    last_cell, cell_ok, port, seq = None, False, 21000, len(done)
    for m in rows:
        if m["run_id"] in done: continue
        if m["cell"] != last_cell:
            cell_ok = False
            for _ in range(3):
                if gate(m["owd_ms"], lg): cell_ok = True; break
                topo_up(lg)
            last_cell = m["cell"]
        if not cell_ok:
            log(lg, f"NETEM-SKIP {m['run_id']} (bağlantı kapısı tutmadı)"); continue
        fwd = f"{m['netem_fwd']} seed {m['seed_fwd']}"; rev = f"{m['netem_rev']} seed {m['seed_rev']}"
        applied = False
        for na in range(1, 5):
            if set_netem(fwd, rev):
                vf, vr = verify("mb0", m["netem_fwd"], m["seed_fwd"]), verify("ma0", m["netem_rev"], m["seed_rev"])
                if vf[0] and vr[0]: applied = True; limit = vf[1]; break
                log(lg, f"NETEM-VERIFY-FAIL {m['run_id']} try={na} fwd='{vf[2]}' rev='{vr[2]}'")
            else:
                log(lg, f"NETEM-APPLY-FAIL {m['run_id']} try={na}")
            time.sleep(0.3)
        if not applied:
            log(lg, f"NETEM-SKIP {m['run_id']} (netem doğrulanamadı)"); continue
        d = os.path.join(outdir, "runs", m["run_id"]); os.makedirs(d, exist_ok=True)
        port += 3
        if port > 60000: port = 21000
        snap = {"q_fwd_before": json.loads(qdump("mb0")), "q_rev_before": json.loads(qdump("ma0")),
                "l_before": json.loads(sh(["ip", "-s", "-j", "link", "show", "va0"]).stdout)}
        rc = run_row(m, d, port, lg)
        snap.update({"q_fwd_after": json.loads(qdump("mb0")), "q_rev_after": json.loads(qdump("ma0")),
                     "l_after": json.loads(sh(["ip", "-s", "-j", "link", "show", "va0"]).stdout)})
        seq += 1
        snap.update({"m": m, "rc": rc, "seq_no": seq, "unix_ts": int(time.time()), "netem_limit": limit,
                     "loadavg1": open("/proc/loadavg").read().split()[0]})
        json.dump(snap, open(os.path.join(d, "row.json"), "w"))
        r = sh(["python3", os.path.join(HERE, "kvt_row.py"), d])
        if r.returncode != 0 or not r.stdout.strip():
            log(lg, f"ROW-ERROR {m['run_id']}: {r.stderr.strip()[:300]}"); continue
        open(result, "a").write(r.stdout)
        if seq % 25 == 0: log(lg, f"PROGRESS {seq}/{len(rows)} ({m['run_id']})")
    sh(["bash", os.path.join(HERE, "kvt_topo.sh"), "down", str(T["MID"]), str(T["CHLB"])])
    log(lg, f"EXEC-DONE rows_in_result={sum(1 for _ in open(result)) - 1}")

if __name__ == "__main__":
    main()
