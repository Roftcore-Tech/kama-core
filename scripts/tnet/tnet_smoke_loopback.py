#!/usr/bin/env python3
"""kama-target-net-v2 — loopback DUMANI: her kol × iş yükü bir aktarım, 127.0.0.1, netem YOK (yalnız stdlib).

Amaç yalnız "kol koşuyor ve çıktısı yürütücünün okuduğu biçimde" denetimidir; performans sayısı DEĞİLDİR.
Komutlar kampanya yürütücüsünün kendisinden gelir (`tnet_exec.commands`), ad alanı öneki çıkarılarak.

Kullanım: tnet_smoke_loopback.py --out DIR [--bench P] [--tcp P]
Çıkış 0 = her satır N/N teslim + Kama satırlarında motor/şerit/profil denetimleri geçti.
"""
import json
import os
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import tnet_exec as te  # noqa: E402
import tnet_manifest as tm  # noqa: E402


def main():
    a = sys.argv[1:]
    out = os.path.abspath(te.arg(a, "--out", tempfile.mkdtemp(prefix="tnet-smoke-")))
    bench = os.path.abspath(te.arg(a, "--bench", os.path.join(te.REPO, "target/release/bench")))
    tcp = os.path.abspath(te.arg(a, "--tcp", os.path.join(te.REPO, "baselines/target/release/tcp_adapter")))
    te.B_IP = "127.0.0.1"
    te.T["CHLB"] = os.getpid()   # `nsenter` öneki aşağıda çıkarılır
    rows = [dict(zip(tm.COLS, map(str, r))) for r in tm.build("S") if r[2] == "control" and r[16] == 0]
    port, fails = 23000, 0
    print("| kol | iş yükü | rc | teslim | t_tam ms | durum | kama_arm | engine |")
    print("|---|---|---|---|---|---|---|---|")
    for m in rows:
        d = os.path.join(out, m["run_id"])
        os.makedirs(d, exist_ok=True)
        port += 3
        srv, cli, ready = te.commands(m, d, port, bench, tcp)
        srv = srv[4:]   # ["nsenter","-t",pid,"-n"] öneki yok: loopback
        env = dict(os.environ, KAMA_TRANSPORT_ENGINE=te.ENGINE)
        with open(os.path.join(d, "s.err"), "w") as se, open(os.path.join(d, "c.err"), "w") as ce:
            sp = subprocess.Popen(srv, stdout=subprocess.DEVNULL, stderr=se, env=env)
            if ready[0]:
                te.wait_ready(os.path.join(d, "s.err"), ready[0], ready[1])
            else:
                time.sleep(ready[1])
            try:
                rc = subprocess.run(cli, stdout=subprocess.DEVNULL, stderr=ce, env=env, timeout=120).returncode
            except subprocess.TimeoutExpired:
                rc = 124
            sp.terminate()
            try:
                sp.wait(timeout=3)
            except subprocess.TimeoutExpired:
                sp.kill()
        zero = [{"kind": "netem", "packets": 0, "drops": 0}]
        link = [{"stats64": {"tx": {"bytes": 0}, "rx": {"bytes": 0}}}]
        snap = {"q_fwd_before": zero, "q_fwd_after": zero, "q_rev_before": zero, "q_rev_after": zero,
                "l_before": link, "l_after": link, "rc": rc, "seq_no": 0, "unix_ts": int(time.time()),
                "netem_limit": -1, "loadavg1": "0", "nproc": 1, "no_netem": False, "wall_s": 0.0}
        r = te.assemble(m, d, snap)
        good = r["status"] == "OK" and int(r["delivered"]) == int(m["msgs"]) and rc == 0
        fails += 0 if good else 1
        print(f"| {m['arm']} | {m['workload']} | {rc} | {r['delivered']}/{m['msgs']} | {r['t_complete_ms']} | "
              f"{r['status']}{'' if good else ' ✗'} | {r['kama_arm']} | {r['engine']} |")
        json.dump(r, open(os.path.join(d, "row.json"), "w"), ensure_ascii=False)
    print(f"\nloopback duman: {len(rows) - fails}/{len(rows)} satır geçti · dizin {out}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
