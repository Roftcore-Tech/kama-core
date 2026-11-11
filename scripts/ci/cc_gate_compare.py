#!/usr/bin/env python3
"""CC gerileme kapısı — taban (PR'ın base SHA'sı) ↔ aday (PR başı) motor simülasyonu karşılaştırması.

Girdi: iki JSONL dosyası (`CC_GATE_OUT` ile `crates/adaptive-transport/tests/cc_port_sim.rs`
üretir). Satır: {"engine","cell","seed","completed_ns","delivered","sent_bytes","sent_retx",
"rev_bytes","unit_delay_p50_ns"}. Sim belirlenimli: aynı kod + aynı tohum → aynı sayı.

Kurallar (hücre başına, tohumlar eşleştirilerek):
  port (`cc_port`):
    K1 tabanda tamamlanan bir tohum adayda tamamlanmıyorsa → KIRMIZI.
    K2 iki tarafta da tamamlanan tohumların süre medyanı adayda taban × (1 + TOL) üstündeyse → KIRMIZI.
    K3 aynı tohumlarda ileri yön bayt medyanı adayda taban × (1 + TOL) üstündeyse → KIRMIZI.
    K4 ileti hücresinde (unit_delay_p50_ns > 0) birim teslim p50 medyanı adayda taban × (1 + TOL)
       üstündeyse → KIRMIZI.
    K5 teslim sayısı medyanı adayda tabanın altındaysa → KIRMIZI.
  eski motor (`legacy`, ürün geri alma yolu): her alan BAYT DÜZEYİNDE aynı olmalı; fark → KIRMIZI.
Muafiyet: `crates/adaptive-transport/tests/cc_gate_allow.txt` satırları `<motor>|<hücre>|<gerekçe>`;
muaf hücre için K1–K5 (ya da legacy eşitliği) uygulanmaz, yalnız raporlanır. Muafiyet PR'da görünür.

Kayıt bütünlüğü (her iki dosya): tabanda kayıt yoksa / dosya yoksa → KIRMIZI (taban koşusu
yutulmaz; workflow aynı düzeneği — HEAD'in sim harness'ını — iki checkout'ta koşar). Her hücrede
tohum kümesi eşit olmalı: adayda eksik ya da fazla tohum → KIRMIZI; yinelenen (aynı motor/hücre/tohum
iki kez) ya da yarım (alan eksik, sayı değil) kayıt → KIRMIZI. Tabanda olup adayda olmayan hücre →
KIRMIZI; adayda yeni hücre → KIRMIZI (beklenen küme = taban kümesi; yeni hücre taban desteği için
`--expect` dosyasıyla bildirilir, aksi hâlde kapı neyi koruduğunu bilmez).
`--expect <dosya>`: beklenen (motor|hücre|tohum_sayısı) satırları; verilirse HER iki dosya bu kümeye
karşı da doğrulanır (eksik hücre/tohum → KIRMIZI).

Kullanım: cc_gate_compare.py <taban.jsonl> <aday.jsonl> [--allow <dosya>] [--tol 0.02] [--expect <dosya>]
        cc_gate_compare.py --selftest        (kalıcı öz-test; CI'da koşar)
Çıkış kodu: 0 yeşil, 1 kırmızı, 2 kullanım hatası. stdlib-only.
"""
import json
import statistics
import sys
from collections import defaultdict


REQUIRED = ("engine", "cell", "seed", "completed_ns", "delivered", "sent_bytes", "sent_retx", "rev_bytes", "unit_delay_p50_ns")


def load(path, problems):
    """(engine, cell) -> {seed: row}. Yinelenen / yarım kayıt `problems`'a yazılır (KIRMIZI)."""
    rows = defaultdict(dict)
    try:
        with open(path, encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    r = json.loads(line)
                except json.JSONDecodeError:
                    problems.append(f"{path}:{n}: bozuk JSON satırı")
                    continue
                missing = [k for k in REQUIRED if k not in r]
                if missing:
                    problems.append(f"{path}:{n}: yarım kayıt, eksik alan {missing}")
                    continue
                key = (r["engine"], r["cell"])
                seed = int(r["seed"])
                if seed in rows[key]:
                    problems.append(f"{path}:{n}: yinelenen kayıt {key} tohum {seed}")
                    continue
                rows[key][seed] = r
    except FileNotFoundError:
        return None
    return rows


def load_expect(path):
    """(engine, cell) -> beklenen tohum sayısı."""
    exp = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            engine, cell, n = [p.strip() for p in line.split("|", 2)]
            exp[(engine, cell)] = int(n)
    return exp


def load_allow(path):
    allow = {}
    if not path:
        return allow
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                parts = [p.strip() for p in line.split("|", 2)]
                if len(parts) < 3 or not parts[2]:
                    print(f"::error title=CC_GATE::muafiyet satırı gerekçesiz: {line!r}")
                    sys.exit(2)
                allow[(parts[0], parts[1])] = parts[2]
    except FileNotFoundError:
        pass
    return allow


def median(xs):
    return statistics.median(xs) if xs else None


def compare(base_path, head_path, allow_path=None, tol=0.02, expect_path=None, out=print):
    problems = []
    base = load(base_path, problems)
    head = load(head_path, problems)
    red = list(problems)
    info = []
    if head is None or not head:
        red.append("aday kaydı yok (CC_GATE_OUT üretilmedi ya da dosya yok)")
    if base is None or not base:
        red.append("taban kaydı yok (taban koşusu başarısız ya da dosya yok) — karşılaştırma YAPILAMADI; yeşil sayılmaz")
    if red:
        for line in red:
            out(f"::error title=CC_GATE::{line}")
        out(f"CC gerileme kapısı KIRMIZI ({len(red)} bulgu)")
        return 1
    allow = load_allow(allow_path)
    if expect_path:
        exp = load_expect(expect_path)
        for name, rows in (("taban", base), ("aday", head)):
            for key, n in exp.items():
                got = len(rows.get(key, {}))
                if got != n:
                    red.append(f"{name}: {key[0]} · {key[1]}: beklenen {n} tohum, bulunan {got}")
            for key in rows:
                if key not in exp:
                    red.append(f"{name}: beklenmeyen hücre {key[0]} · {key[1]} (expect dosyasına ekle)")

    for key in sorted(set(base) | set(head)):
        engine, cell = key
        b, h = base.get(key), head.get(key)
        tag = f"{engine} · {cell}"
        if b is None:
            red.append(f"{tag}: hücre tabanda YOK (yeni hücre; expect dosyasıyla bildir)")
            continue
        if h is None:
            red.append(f"{tag}: hücre adayda YOK (silinmiş)")
            continue
        exempt = allow.get(key)
        missing = sorted(set(b) - set(h))
        extra = sorted(set(h) - set(b))
        if missing or extra:
            red.append(f"{tag}: tohum kümesi farklı — adayda eksik {missing}, fazla {extra}")
            continue
        seeds = sorted(set(b) & set(h))
        findings = []
        if engine == "legacy":
            for s in seeds:
                if b[s] != h[s]:
                    diff = {k: (b[s].get(k), h[s].get(k)) for k in b[s] if b[s].get(k) != h[s].get(k)}
                    findings.append(f"tohum {s} farklı: {diff}")
        else:
            base_done = [s for s in seeds if b[s]["completed_ns"] is not None]
            head_done = [s for s in seeds if h[s]["completed_ns"] is not None]
            lost = [s for s in base_done if s not in head_done]
            if lost:
                findings.append(f"K1 tabanda tamamlanan tohum adayda tamamlanmadı: {lost}")
            both = [s for s in base_done if s in head_done]
            if both:
                bt = median([b[s]["completed_ns"] for s in both])
                ht = median([h[s]["completed_ns"] for s in both])
                if ht > bt * (1 + tol):
                    findings.append(f"K2 süre medyanı {bt/1e9:.2f} → {ht/1e9:.2f} s (+{(ht/bt-1)*100:.1f}%)")
                bb = median([b[s]["sent_bytes"] for s in both])
                hb = median([h[s]["sent_bytes"] for s in both])
                if hb > bb * (1 + tol):
                    findings.append(f"K3 ileri bayt medyanı {bb} → {hb} (+{(hb/bb-1)*100:.1f}%)")
            bp = median([b[s]["unit_delay_p50_ns"] for s in seeds])
            hp = median([h[s]["unit_delay_p50_ns"] for s in seeds])
            if bp and hp and hp > bp * (1 + tol):
                findings.append(f"K4 birim teslim p50 medyanı {bp/1e9:.2f} → {hp/1e9:.2f} s (+{(hp/bp-1)*100:.1f}%)")
            bd = median([b[s]["delivered"] for s in seeds])
            hd = median([h[s]["delivered"] for s in seeds])
            if hd < bd:
                findings.append(f"K5 teslim medyanı {bd} → {hd}")
            # Bilgi: iyileşme de yazılır (kapı değil).
            if both:
                bt = median([b[s]["completed_ns"] for s in both])
                ht = median([h[s]["completed_ns"] for s in both])
                if ht < bt * (1 - tol):
                    info.append(f"{tag}: süre medyanı {bt/1e9:.2f} → {ht/1e9:.2f} s (iyileşme)")
            gained = [s for s in head_done if s not in base_done]
            if gained:
                info.append(f"{tag}: adayda yeni tamamlanan tohum {gained}")
        if findings:
            if exempt:
                info.append(f"{tag}: MUAF ({exempt}) — " + "; ".join(findings))
            else:
                red.append(f"{tag}: " + "; ".join(findings))

    for line in info:
        out(f"bilgi: {line}")
    if red:
        for line in red:
            out(f"::error title=CC_GATE::{line}")
        out(f"CC gerileme kapısı KIRMIZI ({len(red)} bulgu)")
        return 1
    out("CC gerileme kapısı yeşil")
    return 0


def _row(engine, cell, seed, completed=10_000_000_000, delivered=125, sent=150_000, retx=10, rev=5000, p50=0):
    return {"engine": engine, "cell": cell, "seed": seed, "completed_ns": completed, "delivered": delivered,
            "sent_bytes": sent, "sent_retx": retx, "rev_bytes": rev, "unit_delay_p50_ns": p50}


def selftest():
    """Kalıcı öz-test (CI): inceleme 2026-11-11'deki iki yanlış-yeşil dahil."""
    import os
    import tempfile

    def write(rows, name):
        path = os.path.join(tmp, name)
        with open(path, "w", encoding="utf-8") as f:
            for r in rows:
                f.write(json.dumps(r) + "\n")
        return path

    quiet = lambda *_: None
    tmp = tempfile.mkdtemp(prefix="cc_gate_selftest_")
    base = [_row("cc_port", "q6", s) for s in range(1, 11)] + [_row("legacy", "q6", s) for s in range(1, 11)]
    bp = write(base, "base.jsonl")
    cases = []
    # 1. aynı dosya → yeşil
    cases.append(("aynı", compare(bp, bp, out=quiet), 0))
    # 2. taban yok → KIRMIZI (eski davranış: uyarı + yeşil)
    cases.append(("taban yok", compare(os.path.join(tmp, "yok.jsonl"), bp, out=quiet), 1))
    # 3. adayda yalnız 1 tohum → KIRMIZI (eski davranış: ortak tohumla yeşil)
    hp = write([_row("cc_port", "q6", 1), _row("legacy", "q6", 1)], "one.jsonl")
    cases.append(("eksik tohum", compare(bp, hp, out=quiet), 1))
    # 4. yinelenen kayıt → KIRMIZI
    hp = write(base + [_row("cc_port", "q6", 3)], "dup.jsonl")
    cases.append(("yinelenen", compare(bp, hp, out=quiet), 1))
    # 5. yarım kayıt → KIRMIZI
    half = [dict(r) for r in base]
    del half[0]["sent_bytes"]
    hp = write(half, "half.jsonl")
    cases.append(("yarım", compare(bp, hp, out=quiet), 1))
    # 6. port tohum 3 tamamlanmıyor + %5 süre → KIRMIZI; legacy 1 bayt → KIRMIZI
    bad = []
    for r in base:
        r = dict(r)
        if r["engine"] == "cc_port":
            if r["seed"] == 3:
                r["completed_ns"] = None
            else:
                r["completed_ns"] = int(r["completed_ns"] * 1.05)
        if r["engine"] == "legacy" and r["seed"] == 1:
            r["sent_bytes"] += 1
        bad.append(r)
    hp = write(bad, "bad.jsonl")
    cases.append(("gerileme", compare(bp, hp, out=quiet), 1))
    # 7. muafiyetle port geçer, legacy farkı yine KIRMIZI
    ap = os.path.join(tmp, "allow.txt")
    with open(ap, "w", encoding="utf-8") as f:
        f.write("cc_port|q6|öz-test muafiyeti\n")
    cases.append(("muafiyet+legacy", compare(bp, hp, allow_path=ap, out=quiet), 1))
    # 8. iyileşme → yeşil (bilgi)
    good = [dict(r, completed_ns=int(r["completed_ns"] * 0.8)) if r["engine"] == "cc_port" else r for r in base]
    hp = write(good, "good.jsonl")
    cases.append(("iyileşme", compare(bp, hp, out=quiet), 0))
    # 9. expect dosyası: beklenen hücre eksik → KIRMIZI; tam → yeşil
    ep = os.path.join(tmp, "expect.txt")
    with open(ep, "w", encoding="utf-8") as f:
        f.write("cc_port|q6|10\nlegacy|q6|10\ncc_port|r25|10\n")
    cases.append(("expect eksik", compare(bp, bp, expect_path=ep, out=quiet), 1))
    with open(ep, "w", encoding="utf-8") as f:
        f.write("cc_port|q6|10\nlegacy|q6|10\n")
    cases.append(("expect tam", compare(bp, bp, expect_path=ep, out=quiet), 0))
    # 10. adayda yeni hücre → KIRMIZI
    hp = write(base + [_row("cc_port", "yeni", 1)], "new.jsonl")
    cases.append(("yeni hücre", compare(bp, hp, out=quiet), 1))
    bad_cases = [(n, got, exp) for n, got, exp in cases if got != exp]
    for n, got, exp in cases:
        print(f"öz-test {n}: çıkış {got} (beklenen {exp}) {'OK' if got == exp else 'HATA'}")
    if bad_cases:
        print(f"::error title=CC_GATE::öz-test KIRMIZI: {bad_cases}")
        return 1
    print("öz-test yeşil")
    return 0


def main(argv):
    if len(argv) >= 2 and argv[1] == "--selftest":
        return selftest()
    if len(argv) < 3:
        print(__doc__)
        return 2
    base_path, head_path = argv[1], argv[2]
    allow_path = None
    expect_path = None
    tol = 0.02
    i = 3
    while i < len(argv):
        if argv[i] == "--allow":
            allow_path = argv[i + 1]
            i += 2
        elif argv[i] == "--tol":
            tol = float(argv[i + 1])
            i += 2
        elif argv[i] == "--expect":
            expect_path = argv[i + 1]
            i += 2
        else:
            print(f"bilinmeyen argüman: {argv[i]}")
            return 2
    return compare(base_path, head_path, allow_path, tol, expect_path)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
