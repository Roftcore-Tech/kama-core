#!/usr/bin/env python3
"""kama-target-net-v2 — donma kaydını (reports/kama-target-net-v2/FREEZE.json) üretir/doğrular (yalnız stdlib).

  tnet_freeze.py --code-sha SHA [--write]   # SHA = ölçülen kodun commit'i; --write yoksa yalnız yazdırır
  tnet_freeze.py --verify                   # kayıttaki sha256'lar bugünkü dosyalarla aynı mı (çıkış 0/1)
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(REPO, "reports", "kama-target-net-v2", "FREEZE.json")
FILES = ["scripts/tnet/tnet_manifest.py", "scripts/tnet/tnet_exec.py", "scripts/tnet/tnet_campaign.py",
         "scripts/tnet/tnet_analyze.py", "scripts/tnet/tnet_env.py", "scripts/tnet/tnet_offload.py",
         "scripts/tnet/tnet_topo.sh", "scripts/netem/kapi3_harness.sh", "baselines/src/bin/tcp_adapter.rs",
         "baselines/Cargo.toml", "baselines/Cargo.lock"]


def sha256(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def current(code_sha):
    man = {}
    with tempfile.TemporaryDirectory() as d:
        for rep in ("R1", "R2", "P"):
            p = os.path.join(d, f"m-{rep}.csv")
            cmd = [sys.executable, os.path.join(HERE, "tnet_manifest.py"), "--replica", rep, "--out", p]
            if rep == "P":
                cmd.append("--pilot")
            subprocess.run(cmd, check=True, capture_output=True)
            man[rep] = sha256(p)
    return {"schema": "tnet-freeze-v1", "campaign": "kama-target-net-v2", "code_sha": code_sha,
            "base_main_sha": "b88a047f56d3b2f46855e266da0f9f651aa7c5f4",
            "manifest_sha256": man, "files_sha256": {f: sha256(os.path.join(REPO, f)) for f in FILES}}


def main():
    a = sys.argv[1:]
    if "--verify" in a:
        frz = json.load(open(OUT))
        now = current(frz["code_sha"])
        bad = [k for k in ("manifest_sha256", "files_sha256") if now[k] != frz[k]]
        print("FREEZE " + ("UYUŞUYOR" if not bad else f"UYUŞMUYOR: {bad}"))
        sys.exit(1 if bad else 0)
    code_sha = a[a.index("--code-sha") + 1]
    rec = current(code_sha)
    txt = json.dumps(rec, indent=2, ensure_ascii=False) + "\n"
    if "--write" in a:
        os.makedirs(os.path.dirname(OUT), exist_ok=True)
        open(OUT, "w").write(txt)
    print(txt)


if __name__ == "__main__":
    main()
