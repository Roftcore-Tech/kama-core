#!/usr/bin/env python3
"""kama-target-net-v2 — koşu ORTAMI kaydı (yalnız stdlib). Kişisel veri YAZMAZ: kullanıcı adı, ana
dizin ve makine adı kaydedilmez; yollar depo köküne göreli yazılır.

Kullanım: tnet_env.py --out ENV.json --bench P --tcp P --jobs J
"""
import hashlib
import json
import os
import platform
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))


def run(cmd):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=20)
        return (r.stdout or r.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as e:
        return f"UNAVAILABLE: {e.__class__.__name__}"


def sha256(path):
    try:
        h = hashlib.sha256()
        with open(path, "rb") as f:
            for b in iter(lambda: f.read(1 << 20), b""):
                h.update(b)
        return h.hexdigest()
    except OSError:
        return "missing"


def rel(p):
    p = os.path.abspath(p)
    return os.path.relpath(p, REPO) if p.startswith(REPO + os.sep) else os.path.basename(p)


def cpu_model():
    try:
        for line in open("/proc/cpuinfo"):
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unknown"


def sysctl(name):
    try:
        return open("/proc/sys/" + name.replace(".", "/")).read().strip()
    except OSError:
        return "unavailable"


def main():
    a = sys.argv[1:]
    out = a[a.index("--out") + 1]
    bench = a[a.index("--bench") + 1]
    tcp = a[a.index("--tcp") + 1]
    jobs = a[a.index("--jobs") + 1] if "--jobs" in a else ""
    mem_kb = ""
    try:
        for line in open("/proc/meminfo"):
            if line.startswith("MemTotal:"):
                mem_kb = line.split()[1]
    except OSError:
        pass
    env = {
        "schema": "tnet-env-v1",
        "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "kernel": platform.release(),
        "kernel_version": platform.version(),
        "machine": platform.machine(),
        "cpu_model": cpu_model(),
        "nproc": os.cpu_count(),
        "mem_total_kb": mem_kb,
        "jobs": jobs,
        "tc_version": run(["tc", "-V"]),
        "ip_version": run(["ip", "-V"]),
        "python": sys.version.split()[0],
        "rustc": run(["rustc", "--version"]),
        "git_head": run(["git", "-C", REPO, "rev-parse", "HEAD"]),
        "git_branch": run(["git", "-C", REPO, "rev-parse", "--abbrev-ref", "HEAD"]),
        "git_dirty_code": run(["git", "-C", REPO, "status", "--porcelain", "--untracked-files=no", "--", "crates", "baselines",
                               "scripts/tnet", "Cargo.toml", "Cargo.lock"]),
        "bench_path": rel(bench),
        "bench_sha256": sha256(bench),
        "tcp_adapter_path": rel(tcp),
        "tcp_adapter_sha256": sha256(tcp),
        "sysctl": {k: sysctl(k) for k in ("net.ipv4.tcp_congestion_control", "net.core.default_qdisc",
                                          "net.ipv4.tcp_available_congestion_control", "net.core.rmem_max",
                                          "net.core.wmem_max", "kernel.unprivileged_userns_clone")},
        "sch_netem_loaded": os.path.isdir("/sys/module/sch_netem"),
        "loadavg_at_start": run(["cat", "/proc/loadavg"]),
        "env_KAMA_TRANSPORT_ENGINE_inherited": os.environ.get("KAMA_TRANSPORT_ENGINE", "(unset)"),
    }
    with open(out, "w") as f:
        json.dump(env, f, indent=2, ensure_ascii=False)
        f.write("\n")


if __name__ == "__main__":
    main()
