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

Tabanda hiç kayıt yoksa (kapı desteği olmayan eski taban): UYARI + yeşil (ilk PR'da taban yok).
Tabanda olup adayda olmayan hücre → KIRMIZI (hücre silinmiş). Adayda yeni hücre → bilgi.

Kullanım: cc_gate_compare.py <taban.jsonl> <aday.jsonl> [--allow <dosya>] [--tol 0.02]
Çıkış kodu: 0 yeşil, 1 kırmızı, 2 kullanım hatası. stdlib-only.
"""
import json
import statistics
import sys
from collections import defaultdict


def load(path):
    rows = defaultdict(dict)  # (engine, cell) -> {seed: row}
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                r = json.loads(line)
                rows[(r["engine"], r["cell"])][int(r["seed"])] = r
    except FileNotFoundError:
        return None
    return rows


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


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 2
    base_path, head_path = argv[1], argv[2]
    allow_path = None
    tol = 0.02
    i = 3
    while i < len(argv):
        if argv[i] == "--allow":
            allow_path = argv[i + 1]
            i += 2
        elif argv[i] == "--tol":
            tol = float(argv[i + 1])
            i += 2
        else:
            print(f"bilinmeyen argüman: {argv[i]}")
            return 2
    base = load(base_path)
    head = load(head_path)
    if head is None or not head:
        print("::error title=CC_GATE::aday kaydı yok (CC_GATE_OUT üretilmedi)")
        return 1
    if base is None or not base:
        print("::warning title=CC_GATE::tabanda kayıt yok (kapı desteği olmayan taban) — karşılaştırma atlandı, yeşil")
        return 0
    allow = load_allow(allow_path)
    red = []
    info = []

    for key in sorted(set(base) | set(head)):
        engine, cell = key
        b, h = base.get(key), head.get(key)
        tag = f"{engine} · {cell}"
        if b is None:
            info.append(f"yeni hücre: {tag}")
            continue
        if h is None:
            red.append(f"{tag}: hücre adayda YOK (silinmiş)")
            continue
        exempt = allow.get(key)
        seeds = sorted(set(b) & set(h))
        if not seeds:
            red.append(f"{tag}: ortak tohum yok")
            continue
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
        print(f"bilgi: {line}")
    if red:
        for line in red:
            print(f"::error title=CC_GATE::{line}")
        print(f"CC gerileme kapısı KIRMIZI ({len(red)} bulgu)")
        return 1
    print("CC gerileme kapısı yeşil")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
