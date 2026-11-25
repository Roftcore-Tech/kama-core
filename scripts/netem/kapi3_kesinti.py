#!/usr/bin/env python3
"""[DÜZENEK v1] Zamanlı tam kesinti uygulayıcısı (ürün kodu değil). Koşunun t0'ına (koşu dizinindeki
sync/t0, Unix ns; f0_flows alice yazar) bağlıdır; t0 yazılmadıkça bekler.

kullanım (orta ad alanında; kapi3_harness.sh `kapi3_yan_baslat` başlatır, hücrede KES_PLAN + KES_TMPL):
    kapi3_kesinti.py <koşu dizini> "<ofset_ms>:<kayıp %> ..." "<netem şablonu, @L@ = kayıp>"
    ör. KES_PLAN="16250:100 16550:0"  KES_TMPL="delay 300ms loss @L@% rate 100kbit -14 limit 64"
Her adım: hedef − 8 ms'de mb0 `tc -s -j` kaydı, hedef anında `tc qdisc change dev mb0 root netem <şablon>`
(komut öncesi/sonrası duvar saati + monoton saat), hemen ardından yine `tc -s -j` kaydı. Yalnız kayıp
değişsin diye şablonun kalanı hücrenin ileri hat dizesiyle AYNI olmalı. Çıktı: <koşu dizini>/kesinti.jsonl."""
import json, signal, subprocess, sys, time
rd, plan, tmpl = sys.argv[1], sys.argv[2], sys.argv[3]
steps = [(float(x.split(":")[0]), x.split(":")[1]) for x in plan.split()]
out = open(f"{rd}/kesinti.jsonl", "w", buffering=1)
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
out.write(json.dumps({"k": "t0", "t0_unix_ns": t0, "gorulen_unix_ns": time.time_ns(), "plan": plan, "sablon": tmpl}) + "\n")
def stat():
    a = time.time_ns()
    r = subprocess.run(["tc", "-s", "-j", "qdisc", "show", "dev", "mb0"], capture_output=True, text=True)
    b = time.time_ns()
    return {"t_once_ns": a, "t_sonra_ns": b, "q": json.loads(r.stdout)}
def sleep_until(ns):
    while True:
        d = ns - time.time_ns()
        if d <= 0:
            return
        if d > 1_500_000:
            time.sleep((d - 1_000_000) / 1e9)
for off_ms, loss in steps:
    target = t0 + int(round(off_ms * 1e6))
    sleep_until(target - 8_000_000)
    pre = stat()
    sleep_until(target)
    cmd = ["tc", "qdisc", "change", "dev", "mb0", "root", "netem"] + tmpl.replace("@L@", loss).split()
    c0 = time.time_ns(); m0 = time.monotonic_ns()
    r = subprocess.run(cmd, capture_output=True, text=True)
    c1 = time.time_ns(); m1 = time.monotonic_ns()
    post = stat()
    out.write(json.dumps({"k": "adim", "ofset_ms": off_ms, "kayip_pct": loss, "hedef_unix_ns": target,
                          "komut_once_unix_ns": c0, "komut_sonra_unix_ns": c1, "komut_once_mono_ns": m0,
                          "komut_sonra_mono_ns": m1, "rc": r.returncode, "stderr": r.stderr.strip(),
                          "komut": " ".join(cmd), "once": pre, "sonra": post}) + "\n")
out.write(json.dumps({"k": "bitti", "t_unix_ns": time.time_ns()}) + "\n")
while True:
    time.sleep(1)
