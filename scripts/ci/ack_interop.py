#!/usr/bin/env python3
"""ACK birlikte çalışma matrisi (ACK referans düzeneği, ayak d). Python 3.9+ stdlib-only.

    python3 -I scripts/ci/ack_interop.py --old OLD_BIN --new NEW_BIN \
        --expect scripts/ci/ack-interop/expect.txt
    python3 -I scripts/ci/ack_interop.py --selftest --new NEW_BIN

OLD_BIN: SABİTLENMİŞ SHA'dan (scripts/ci/ack-interop/PIN) derlenmiş `ack_interop_peer`.
NEW_BIN: PR başından derlenmiş `ack_interop_peer`.

Dört eşleşme (gönderici–alıcı): eski–eski, eski–yeni, yeni–eski, yeni–yeni. Her eşleşmenin
BEKLENEN sonucu `expect.txt`'tedir (`ok` ya da `unsupported`); beklenti değişikliği PR'da görünür.

Sınıflar:
  ok           iki uç 0 ile çıktı VE raporlar tutarlı (gönderici her birimi teyit etti; teyitli
               her seq gönderilmiş ve alıcıda alınmış; ACK çerçevesi ≤ 79 B).
  unsupported  iki uç da 4 (UNSUPPORTED_PAIRING) ile, süre dolmadan çıktı.
  timeout      bir uç 2 (TIMEOUT) ile çıktı           → beklentiden bağımsız KIRMIZI.
  hang         süreç betik süresini aştı, öldürüldü   → beklentiden bağımsız KIRMIZI.
  error        diğer her durum                         → KIRMIZI.

ÖNEMLİ: Bugün eski ve yeni AYNI ACK biçimini kullanır; matrisin yeşil olması gelecekteki bir biçim
hakkında hiçbir şey kanıtlamaz. Altyapı hazırdır; gerçek dört yönlü uyum kanıtı yeni biçim
geldiğinde tamamlanır.

Çıkış: 0 matris beklentiyle aynı · 1 sapma · 2 kullanım/ortam hatası.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
import time

ACK_FRAME_MAX_BYTES = 79
PAIRS = [("old", "old"), ("old", "new"), ("new", "old"), ("new", "new")]


def run_pair(tx_bin, rx_bin, workdir, deadline_ms, rx_extra=(), tx_extra=(), rx_peer_override=None):
    rx_rep = os.path.join(workdir, "rx.json")
    tx_rep = os.path.join(workdir, "tx.json")
    for p in (rx_rep, tx_rep):
        if os.path.exists(p):
            os.remove(p)
    hard = deadline_ms / 1000.0 + 15.0
    t0 = time.monotonic()
    rx = subprocess.Popen([rx_bin, "rx", "--report", rx_rep, "--deadline-ms", str(deadline_ms), *rx_extra],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    line = rx.stdout.readline().strip()
    if not line.startswith("PORT "):
        rx.kill()
        return "error", f"alıcı PORT basmadı: {line!r} {rx.stderr.read()}", None, None
    port = rx_peer_override or int(line.split()[1])
    tx = subprocess.Popen([tx_bin, "tx", "--peer", f"127.0.0.1:{port}", "--report", tx_rep,
                           "--deadline-ms", str(deadline_ms), *tx_extra],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    codes, errs = {}, {}
    for name, p in (("tx", tx), ("rx", rx)):
        left = max(0.1, hard - (time.monotonic() - t0))
        try:
            _, err = p.communicate(timeout=left)
            codes[name], errs[name] = p.returncode, err.strip()
        except subprocess.TimeoutExpired:
            p.kill()
            p.communicate()
            codes[name], errs[name] = None, "öldürüldü"
    took = time.monotonic() - t0
    detail = f"tx={codes['tx']} rx={codes['rx']} süre={took:.1f}s tx_err={errs['tx']!r} rx_err={errs['rx']!r}"
    if None in codes.values():
        return "hang", detail, None, None
    if 2 in codes.values():
        return "timeout", detail, None, None
    if codes["tx"] == 4 and codes["rx"] == 4:
        return "unsupported", detail, None, None
    if codes["tx"] == 0 and codes["rx"] == 0:
        try:
            txr = json.load(open(tx_rep, encoding="utf-8"))
            rxr = json.load(open(rx_rep, encoding="utf-8"))
        except (OSError, ValueError) as e:
            return "error", f"{detail} rapor okunamadı: {e}", None, None
        problems = []
        if txr["confirmed"] != txr["units"]:
            problems.append(f"teyit {txr['confirmed']}/{txr['units']}")
        acked, sent, recv = set(txr["acked"]), set(txr["sent"]), set(rxr["received"])
        if not acked <= sent:
            problems.append(f"gönderilmemiş teyit {sorted(acked - sent)[:5]}")
        if not acked <= recv:
            problems.append(f"alınmamış teyit {sorted(acked - recv)[:5]}")
        if rxr["max_ack_frame_bytes"] > ACK_FRAME_MAX_BYTES:
            problems.append(f"ACK çerçevesi {rxr['max_ack_frame_bytes']} B > {ACK_FRAME_MAX_BYTES}")
        if rxr["acks"] == 0 or txr["acks_rx"] == 0:
            problems.append("vakum: ACK yok")
        if problems:
            return "error", f"{detail} tutarsız: {'; '.join(problems)}", txr, rxr
        return "ok", detail, txr, rxr
    return "error", detail, None, None


def load_expect(path):
    exp = {}
    for raw in open(path, encoding="utf-8"):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        tx, rx, res = line.split()
        if (tx, rx) not in PAIRS or res not in ("ok", "unsupported"):
            raise ValueError(f"geçersiz beklenti satırı: {raw!r}")
        if (tx, rx) in exp:
            raise ValueError(f"yinelenen beklenti: {tx} {rx}")
        exp[(tx, rx)] = res
    missing = [p for p in PAIRS if p not in exp]
    if missing:
        raise ValueError(f"eksik beklenti: {missing}")
    return exp


def gh_error(msg):
    print(f"::error title=ACK_INTEROP::{msg}")


def selftest(new_bin, workdir):
    """Kapının kendi kapısı: desteklenmeyen eşleşme TANIMLI hatayla ve süre dolmadan düşmeli;
    süre aşımı ayrı sınıflanmalı."""
    ok = True
    cases = [
        ("HELLO'da ortak tür yok", ["--emit-types", "99"], (), None, "unsupported"),
        ("çalışırken çözülemeyen ACK türü", ["--rewrite-ack-type", "99"], (), None, "unsupported"),
        ("karşı uç yok (süre aşımı sınıfı)", (), (), 9, "timeout"),
    ]
    for name, rx_extra, tx_extra, port, want in cases:
        deadline = 4000 if want == "timeout" else 20000
        t0 = time.monotonic()
        got, detail, _, _ = run_pair(new_bin, new_bin, workdir, deadline, rx_extra, tx_extra, port)
        took = time.monotonic() - t0
        fast_enough = want != "unsupported" or took < deadline / 1000.0 / 2
        status = "TAMAM" if got == want and fast_enough else "KIRMIZI"
        print(f"öz-test [{name}]: beklenen={want} gelen={got} ({detail}) → {status}")
        if status != "TAMAM":
            ok = False
            gh_error(f"öz-test {name}: beklenen {want}, gelen {got}")
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--old")
    ap.add_argument("--new", required=True)
    ap.add_argument("--expect")
    ap.add_argument("--deadline-ms", type=int, default=60000)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    for b in (a.old, a.new):
        if b and not os.access(b, os.X_OK):
            print(f"ikili yok ya da çalıştırılamaz: {b}")
            return 2
    with tempfile.TemporaryDirectory() as wd:
        if a.selftest:
            return 0 if selftest(a.new, wd) else 1
        if not a.old or not a.expect:
            print("--old ve --expect zorunlu")
            return 2
        try:
            exp = load_expect(a.expect)
        except (OSError, ValueError) as e:
            print(f"beklenti dosyası: {e}")
            return 2
        bins = {"old": a.old, "new": a.new}
        bad = 0
        print("ACK birlikte çalışma matrisi — altyapı hazır; gerçek dört yönlü uyum kanıtı yeni biçim "
              "geldiğinde tamamlanır (bugün eski ve yeni aynı ACK biçimini kullanır).")
        for tx, rx in PAIRS:
            got, detail, txr, _ = run_pair(bins[tx], bins[rx], wd, a.deadline_ms)
            want = exp[(tx, rx)]
            mark = "TAMAM" if got == want else "KIRMIZI"
            extra = f" teyit={txr['confirmed']}/{txr['units']} ack={txr['acks_rx']}" if txr else ""
            print(f"gönderici={tx:3s} alıcı={rx:3s} beklenen={want:11s} gelen={got:11s}{extra} → {mark}  ({detail})")
            if got != want:
                bad += 1
                gh_error(f"{tx}->{rx}: beklenen {want}, gelen {got}")
        return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
