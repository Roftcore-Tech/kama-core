#!/usr/bin/env python3
"""[DÜZENEK v1] Kuyruk zaman çizelgesi (ürün kodu değil): orta ad alanında `tc -s -j qdisc show`
dönemsel örneği (bütün arayüzler: ifb0 darboğazı, mb0 / ma0 netem).

kullanım (orta ad alanında; kapi3_harness.sh `kapi3_yan_baslat` başlatır):
    kapi3_ornekleyici.py <çıktı.jsonl> <aralık_ms>
Satır: {"t_unix_ns" (komut öncesi), "t_unix_ns_son" (komut sonrası), "mono_ns", "load1", "q": [tc json]}
SIGTERM ile kapanır; satır tamponlu."""
import json, signal, subprocess, sys, time
out = open(sys.argv[1], "w", buffering=1)
per = int(sys.argv[2]) * 1_000_000
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
nxt = time.monotonic_ns()
while True:
    a = time.time_ns(); m = time.monotonic_ns()
    r = subprocess.run(["tc", "-s", "-j", "qdisc", "show"], capture_output=True, text=True)
    b = time.time_ns()
    try:
        q = json.loads(r.stdout)
    except ValueError:
        q = {"hata": r.stdout[:200] + r.stderr[:200]}
    with open("/proc/loadavg") as f:
        load1 = float(f.read().split()[0])
    out.write(json.dumps({"t_unix_ns": a, "t_unix_ns_son": b, "mono_ns": m, "load1": load1, "q": q}) + "\n")
    nxt += per
    now = time.monotonic_ns()
    if nxt < now:
        nxt = now
    time.sleep((nxt - now) / 1e9)
