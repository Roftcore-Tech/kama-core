#!/usr/bin/env python3
"""kama-target-net-v2 — kampanya KOORDİNATÖRÜ (yalnız stdlib). Ad alanı DIŞINDA koşar.

Ne yapar:
  1. Manifestleri belirlenimli üreticiyle (tnet_manifest.py) OUT altına üretir ve donma kaydındaki
     sha256'larla karşılaştırır (reports/kama-target-net-v2/FREEZE.json; uyuşmazlıkta DUR).
  2. Ölçülen kodun donmuş koddan farkı var mı bakar (`git diff --quiet <code_sha> HEAD -- <yollar>`;
     fark varsa DUR). ENV kaydını yazar (tnet_env.py; her başlatmada ENV-<n>.json, ilkinde ENV.json).
  3. Sonuç CSV'sindeki (OUT/results.csv) run_id'leri atlar; kalan blokları tahmini maliyete göre
     azalan sırayla (LPT) en çok J eşzamanlı işçiye dağıtır. Her işçi `unshare -rnm` içinde kendi
     3-netns topolojisini kurar ve bir bloğu seri koşar (tnet_exec.py).
  4. Kesilirse (Ctrl-C/SIGTERM) işçileri durdurur; aynı komut kaldığı yerden sürer.

Kullanım:
  tnet_campaign.py --out OUT [--jobs 10] [--replicas R1,R2] [--bench P] [--tcp P]
  tnet_campaign.py --out OUT --pilot [--jobs 10]          # süre pilotu (replika P)
  tnet_campaign.py --out OUT --project PILOT_OUT [--jobs 10]   # pilot ölçümünden süre izdüşümü
  Duman (yalnız bulut): --no-netem --only REGEX --max-blocks N  (FREEZE ve kod denetimi atlanır)
"""
import csv
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
FREEZE = os.path.join(REPO, "reports", "kama-target-net-v2", "FREEZE.json")
CODE_PATHS = ["Cargo.toml", "Cargo.lock", "crates", "baselines", "scripts/tnet", "scripts/netem/kapi3_harness.sh"]


def arg(a, k, default=None):
    return a[a.index(k) + 1] if k in a else default


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def log(out, msg):
    line = f"{time.strftime('%Y-%m-%dT%H:%M:%S')} {msg}"
    print(line, flush=True)
    with open(os.path.join(out, "campaign.log"), "a") as f:
        f.write(line + "\n")


def gen_manifest(replica, path, pilot=False):
    cmd = [sys.executable, os.path.join(HERE, "tnet_manifest.py"), "--replica", replica, "--out", path]
    if pilot:
        cmd.append("--pilot")
    subprocess.run(cmd, check=True, capture_output=True)
    return sha256(path)


def done_ids(path):
    if not os.path.exists(path):
        return set()
    with open(path, newline="") as f:
        return {r["run_id"] for r in csv.DictReader(f)}


def project(out, pilot_out, jobs):
    """Pilot satırlarının ölçülen duvar süresi / model maliyeti oranını gruba göre tam manifeste uygular."""
    sys.path.insert(0, HERE)
    import tnet_manifest as tm
    pr = list(csv.DictReader(open(os.path.join(pilot_out, "results.csv"))))
    pm = {r["run_id"]: r for r in csv.DictReader(open(os.path.join(pilot_out, "manifest-P.csv")))}
    meas, model = defaultdict(float), defaultdict(float)
    for r in pr:
        if r["run_id"] in pm:
            meas[r["group"]] += float(r["row_wall_s"] or 0)
            model[r["group"]] += float(pm[r["run_id"]]["est_cost_s"])
    ratio = {g: meas[g] / model[g] for g in model if model[g] > 0}
    # Pilotu olmayan grup (control) için en büyük oran (kötümser).
    worst = max(ratio.values()) if ratio else 1.0
    blocks = defaultdict(float)
    for rep in ("R1", "R2"):
        for r in tm.build(rep):
            blocks[r[24]] += r[25] * ratio.get(r[2], worst)
    load = tm.lpt(blocks, jobs)
    print(f"pilot oranları (ölçülen/model): {json.dumps({g: round(v, 2) for g, v in ratio.items()})}")
    print(f"izdüşüm J={jobs}: {max(load) / 3600:.1f} sa duvar · toplam {sum(load) / 3600:.1f} işçi-saat")
    print("ONKAYIT §6.3: izdüşüm > 26 sa ise kesme sırası uygulanır (manifest değişmeden, --skip-groups ile DEĞİL; "
          "yeni donma commit'iyle).")


def main():
    a = sys.argv[1:]
    out = os.path.abspath(arg(a, "--out"))
    os.makedirs(out, exist_ok=True)
    jobs = int(arg(a, "--jobs", "10"))
    if "--project" in a:
        project(out, os.path.abspath(arg(a, "--project")), jobs)
        return
    pilot = "--pilot" in a
    no_netem = "--no-netem" in a
    only = arg(a, "--only")
    max_blocks = int(arg(a, "--max-blocks", "0"))
    smoke = no_netem or only is not None or max_blocks > 0
    replicas = ["P"] if pilot else arg(a, "--replicas", "R1,R2").split(",")
    bench = os.path.abspath(arg(a, "--bench", os.path.join(REPO, "target/release/bench")))
    tcp = os.path.abspath(arg(a, "--tcp", os.path.join(REPO, "baselines/target/release/tcp_adapter")))
    for p in (bench, tcp):
        if not os.access(p, os.X_OK):
            sys.exit(f"ikili yok/çalıştırılamaz: {p}")

    # 1) manifestler + donma denetimi
    frz = json.load(open(FREEZE)) if os.path.exists(FREEZE) else None
    manifests = {}
    for rep in replicas:
        mp = os.path.join(out, f"manifest-{rep}.csv")
        h = gen_manifest(rep, mp, pilot=pilot)
        manifests[rep] = mp
        want = (frz or {}).get("manifest_sha256", {}).get(rep)
        if not smoke:
            if frz is None:
                sys.exit(f"DONMA KAYDI YOK ({FREEZE}) — ön-kayıt commit'lenmeden gerçek koşu başlatılamaz")
            if want != h:
                sys.exit(f"MANİFEST DONMA UYUŞMAZLIĞI {rep}: üretilen={h} donmuş={want} — DUR")
        log(out, f"MANIFEST {rep} sha256={h} donmuş={want or '-'}")
    if not smoke:
        code_sha = frz["code_sha"]
        r = subprocess.run(["git", "-C", REPO, "diff", "--quiet", code_sha, "HEAD", "--"] + CODE_PATHS)
        if r.returncode != 0:
            sys.exit(f"ÖLÇÜLEN KOD DONMUŞ KODDAN FARKLI (code_sha={code_sha}) — DUR")
        r = subprocess.run(["git", "-C", REPO, "status", "--porcelain", "--"] + CODE_PATHS,
                           capture_output=True, text=True)
        if r.stdout.strip():
            sys.exit("ÇALIŞMA AĞACINDA commit'lenmemiş kod değişikliği var — DUR:\n" + r.stdout)

    # 2) ENV
    n = len([x for x in os.listdir(out) if x.startswith("ENV-")]) + 1
    env_path = os.path.join(out, f"ENV-{n}.json")
    subprocess.run([sys.executable, os.path.join(HERE, "tnet_env.py"), "--out", env_path, "--bench", bench,
                    "--tcp", tcp, "--jobs", str(jobs)], check=True)
    if not os.path.exists(os.path.join(out, "ENV.json")):
        with open(env_path) as s, open(os.path.join(out, "ENV.json"), "w") as d:
            d.write(s.read())
    log(out, f"ENV {env_path}")

    # 3) bekleyen bloklar (LPT)
    results = os.path.join(out, "results.csv")
    done = done_ids(results)
    blocks = []   # (maliyet, blok, manifest)
    for rep, mp in manifests.items():
        cost = defaultdict(float)
        for r in csv.DictReader(open(mp)):
            if r["run_id"] in done or (only and not re.search(only, r["run_id"])):
                continue
            cost[r["block"]] += float(r["est_cost_s"])
        blocks += [(c, b, mp) for b, c in cost.items()]
    blocks.sort(key=lambda x: (-x[0], x[1]))
    if max_blocks:
        blocks = blocks[:max_blocks]
    log(out, f"CAMPAIGN-START replicas={replicas} jobs={jobs} done_rows={len(done)} pending_blocks={len(blocks)} "
             f"pending_model_h={sum(c for c, _, _ in blocks) / 3600:.1f} smoke={smoke} no_netem={no_netem}")

    running = {}
    stop = {"flag": False}

    def on_sig(signum, _frame):
        stop["flag"] = True
        log(out, f"SİNYAL {signum}: işçiler durduruluyor (devam için aynı komut)")
        for p in running:
            try:
                p.terminate()
            except OSError:
                pass

    signal.signal(signal.SIGINT, on_sig)
    signal.signal(signal.SIGTERM, on_sig)
    queue = list(blocks)
    while (queue or running) and not stop["flag"]:
        while queue and len(running) < jobs and not stop["flag"]:
            _, b, mp = queue.pop(0)
            cmd = ["unshare", "-rnm", sys.executable, os.path.join(HERE, "tnet_exec.py"), "--manifest", mp,
                   "--block", b, "--out", out, "--results", results, "--bench", bench, "--tcp", tcp]
            if no_netem:
                cmd.append("--no-netem")
            if only:
                cmd += ["--only", only]
            p = subprocess.Popen(cmd, stdout=open(os.path.join(out, "workers.stdout"), "a"), stderr=subprocess.STDOUT)
            running[p] = b
            log(out, f"BLOCK-START {b} pid={p.pid}")
        time.sleep(1.0)
        for p in [p for p in running if p.poll() is not None]:
            b = running.pop(p)
            log(out, f"BLOCK-{'DONE' if p.returncode == 0 else 'FAILED rc=' + str(p.returncode)} {b}")
    for p in running:
        p.wait()
    if stop["flag"]:
        log(out, "CAMPAIGN-INTERRUPTED")
        sys.exit(130)
    log(out, f"CAMPAIGN-DONE rows_in_result={len(done_ids(results))}")


if __name__ == "__main__":
    main()
