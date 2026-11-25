#!/usr/bin/env python3
"""Hüküm satırlarının kayıttaki girdileriyle üç kuralın vereceği sonuç (HESAP; eski ikililerle koşu değildir; ürün kodu değil).
KOŞUDAN SONRA eklendi (ön kayıtta yok). kullanım: eski_kural.py <kapsama.json> <çıktı>
 yalnız cephe (911f2c6d öncesi): geçti = abs_q ≤ yarı VE growth ≤ yarı; kaldı = abs_q > yarı VE growth > yarı; aksi sonuç yok.
 911f2c6d: geçti = cephe geçti VE interval_g ≤ yarı; kaldı = cephe kaldı VE interval_g > yarı; aksi sonuç yok (kuyruk yazılmaz).
 b0659064: geçti = cephe geçti VE interval_g ≤ yarı; kaldı = (interval_g > yarı VE (kapsıyor VEYA cephe kaldı)) VEYA
           (kapsamıyor VE cephe kaldı VE growth ≤ expect + mtu); aksi sonuç yok (taban × expect yazılır).
 kapsıyor = kapsama.py'nin türettiği değer (ilk ucun gönderimi ≤ yoklama başlangıcı)."""
import json, sys
J = json.load(open(sys.argv[1])); O = []
O.append("| ACK sıra | kayıttaki outcome (b0659064 ikilisi) | yalnız cephe (hesap) | 911f2c6d kuralı (hesap) | b0659064 kuralı (hesap) |")
O.append("|---|---|---|---|---|")
for e in J["hukum"]:
    h, g, cov = e["yari_ns"], e["interval_g_ns"], e.get("kapsiyor_urun")
    fp, ff = e["cephe_gecti"], e["cephe_kaldi"]
    r = lambda p, f: "Passed" if p else ("Failed" if f else "Inconclusive")
    front = r(fp, ff)
    old = r(fp and g <= h, ff and g > h) if g is not None else front
    new = "–" if (g is None or cov is None) else r(fp and g <= h, (g > h and (cov or ff)) or ((not cov) and ff and e["yoklama_boyu"]))
    O.append(f"| {e['sira']} | {e['outcome']} | {front} | {old} | {new} |")
open(sys.argv[2], "w").write("\n".join(O) + "\n"); print("\n".join(O))
