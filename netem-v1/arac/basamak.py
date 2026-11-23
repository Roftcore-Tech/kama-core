#!/usr/bin/env python3
"""Hız basamağı uygulayıcısı (ürün kodu değil). Koşunun t0'ına (sync/t0, Unix ns) bağlıdır.
kullanım (orta ad alanında): basamak.py <koşu dizini> "<ofset_s>:<hız> ..." <tbf burst>
Her basamak: hedef − 8 ms'de AYNI kuyruğun (ifb0: tbf 1: + pfifo 10:) tc -s kaydı, hedef anında
`tc qdisc change … tbf rate <hız>` (komut öncesi/sonrası saat), hemen ardından yine tc -s kaydı."""
import json, signal, subprocess, sys, time
rd, plan, burst = sys.argv[1], sys.argv[2], sys.argv[3]
steps = [(float(x.split(":")[0]), x.split(":")[1]) for x in plan.split()]
out = open(f"{rd}/basamak.jsonl", "w", buffering=1)
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
t0 = None
while t0 is None:
    try:
        s = open(f"{rd}/sync/t0").read().strip()
        if len(s) >= 18:
            t0 = int(s)
    except (OSError, ValueError):
        pass
    if t0 is None:
        time.sleep(0.002)
out.write(json.dumps({"k": "t0", "t0_unix_ns": t0, "gorulen_unix_ns": time.time_ns(), "plan": plan}) + "\n")
def stat():
    a = time.time_ns()
    r = subprocess.run(["tc", "-s", "-j", "qdisc", "show", "dev", "ifb0"], capture_output=True, text=True)
    b = time.time_ns()
    return {"t_once_ns": a, "t_sonra_ns": b, "q": json.loads(r.stdout)}
def sleep_until(ns):
    while True:
        d = ns - time.time_ns()
        if d <= 0:
            return
        if d > 1_500_000:
            time.sleep((d - 1_000_000) / 1e9)
for off, rate in steps:
    target = t0 + int(round(off * 1e9))
    sleep_until(target - 8_000_000)
    pre = stat()
    sleep_until(target)
    cmd = ["tc", "qdisc", "change", "dev", "ifb0", "root", "handle", "1:", "stab", "overhead", "-14",
           "linklayer", "ethernet", "tbf", "rate", rate, "burst", burst, "limit", "64"]
    c0 = time.time_ns(); m0 = time.monotonic_ns()
    r = subprocess.run(cmd, capture_output=True, text=True)
    c1 = time.time_ns(); m1 = time.monotonic_ns()
    post = stat()
    out.write(json.dumps({"k": "basamak", "ofset_s": off, "hiz": rate, "hedef_unix_ns": target,
                          "komut_once_unix_ns": c0, "komut_sonra_unix_ns": c1, "komut_once_mono_ns": m0,
                          "komut_sonra_mono_ns": m1, "rc": r.returncode, "stderr": r.stderr.strip(),
                          "komut": " ".join(cmd), "once": pre, "sonra": post}) + "\n")
out.write(json.dumps({"k": "bitti", "t_unix_ns": time.time_ns()}) + "\n")
while True:
    time.sleep(1)
