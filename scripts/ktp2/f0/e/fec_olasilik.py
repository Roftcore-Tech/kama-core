#!/usr/bin/env python3
"""KTP-2 taslak 0.3 denetimi (Ajan E) — §3.8 FEC olasılıkları, §2 adım 4, acil artıklık d.

Kullanım: python3 -I scripts/ktp2/f0/e/fec_olasilik.py

Model (bütün satırlar için):
  - Her paket bağımsız olarak p olasılığıyla kaybolur (Bernoulli, ileri yön).
  - Reed-Solomon (K, R): K kaynak + R onarım; herhangi K paket bloğu çözer (MDS).
  - Blok kurtarma  = P(K+R paketten en az K'sı gelir).
  - Çerçeve oynatılabilirliği = q + p * P(kalan K+R-1 paketten en az K'sı gelir), q = 1 - p.
  - Zamanlama YOK (son tarih ayrı hesap: k6_ses.py).
Bütün olasılıklar fractions.Fraction ile kesin; yalnız yazdırırken yuvarlanır.
"""
from fractions import Fraction as F
from math import comb, ceil, log


def p_en_az(n, k, q):
    """X ~ Bin(n, q) için P(X >= k), kesin."""
    if k <= 0:
        return F(1)
    return sum(comb(n, i) * q**i * (1 - q) ** (n - i) for i in range(k, n + 1))


def blok(K, R, p):
    return p_en_az(K + R, K, 1 - p)


def cerceve(K, R, p):
    q = 1 - p
    return q + p * p_en_az(K + R - 1, K, q)


def yuzde(x, n=1):
    return f"{float(x) * 100:.{n}f}"


def main():
    print("== FEC-01  §3.8 ses tablosu (taslak değeri → kesin binom) ==")
    tablo = [  # (p, K, R, taslak_carpani, taslak_cerceve, taslak_blok)
        (F(1, 10), 4, 1, "1,25", "96,6", "91,9"),
        (F(2, 10), 3, 2, "1,67", "96,4", "94,2"),
        (F(3, 10), 4, 4, "2", "96,2", "94,2"),
        (F(5, 10), 4, 9, "3,25", "96,4", "95,4"),
        (F(8, 10), 4, 32, "9", "95,2", "94,8"),
    ]
    for p, K, R, tc, tf, tb in tablo:
        c = cerceve(K, R, p)
        b = blok(K, R, p)
        carp = F(K + R, K)
        ok = (yuzde(c).replace(".", ",") == tf) and (yuzde(b).replace(".", ",") == tb)
        print(
            f"  p={float(p):.2f} (K,R)=({K},{R}) çarpan={float(carp):.4f} (taslak {tc})"
            f" | çerçeve={yuzde(c, 3)}% (taslak {tf}) | blok={yuzde(b, 3)}% (taslak {tb})"
            f" | ≥%95 çerçeve: {c >= F(95, 100)} | eşleşme: {'EVET' if ok else 'HAYIR'}"
        )

    print("== FEC-02  §3.8 '%50'de (4,8) ve (2,4)' örnekleri ==")
    for K, R, tf, tb in [(4, 8, "94,3", "92,7"), (2, 4, "90,6", "89,1")]:
        p = F(1, 2)
        c, b = cerceve(K, R, p), blok(K, R, p)
        print(
            f"  (K,R)=({K},{R}) çarpan={float(F(K + R, K)):.2f} çerçeve={yuzde(c, 3)}% (taslak {tf})"
            f" blok={yuzde(b, 3)}% (taslak {tb})"
        )

    print("== FEC-03  §2 adım 4: '%30 kayıpta 4 ses parçasına 3–4 onarım' ==")
    for R in (3, 4):
        p = F(3, 10)
        print(
            f"  (4,{R}) p=0,30: çerçeve={yuzde(cerceve(4, R, p), 3)}%  blok={yuzde(blok(4, R, p), 3)}%"
            f"  ≥%95 çerçeve: {cerceve(4, R, p) >= F(95, 100)}"
        )

    print("== FEC-04  [betimsel] her kayıpta ≥%95 çerçeve sağlayan en küçük çarpan (K ≤ 4) ==")
    for p in (F(1, 10), F(2, 10), F(3, 10), F(5, 10), F(8, 10)):
        adaylar = []
        for K in range(1, 5):
            for R in range(1, 64):
                if cerceve(K, R, p) >= F(95, 100):
                    adaylar.append((F(K + R, K), K, R))
                    break
        en_kucuk = min(a[0] for a in adaylar)
        esit = sorted((K, R) for c, K, R in adaylar if c == en_kucuk)
        print(
            f"  p={float(p):.2f}: en küçük çarpan {float(en_kucuk):.4f}, bu çarpanda (K,R) = {esit}"
            f" ; çerçeve: " + ", ".join(f"({K},{R})={yuzde(cerceve(K, R, p), 2)}%" for K, R in esit)
        )

    print("== FEC-05  §3.8 '8+4 RS: %10 → %99,6, %20 → %92,7' ==")
    for p, t in [(F(1, 10), "99,6"), (F(2, 10), "92,7")]:
        print(f"  p={float(p):.2f}: P(12'den ≥8) = {yuzde(blok(8, 4, p), 3)}% (taslak {t})")

    print("== FEC-06  §3.8 'paketlerin yarısını iki kez göndermek: %63 ve %34,8' ==")
    print("  model: 8 kaynaktan 4'ü iki kez (12 iletim = 1,5×); blok tam = 8 kaynağın hepsi gelir")
    for p, t in [(F(1, 10), "63"), (F(2, 10), "34,8")]:
        v = (1 - p) ** 4 * (1 - p**2) ** 4
        print(f"  p={float(p):.2f}: (1-p)^4 (1-p²)^4 = {yuzde(v, 3)}% (taslak {t})")

    print("== FEC-07  §3.8 güvenilir akış R = ⌈K·p/(1−p)·1,25⌉ ==")
    for K, p, t in [(8, F(2, 10), 3), (8, F(3, 10), 5)]:
        x = K * p / (1 - p) * F(5, 4)
        R = ceil(x)
        print(
            f"  K={K} p={float(p):.2f}: K·p/(1−p)·1,25 = {x} = {float(x):.4f} → R={R} (taslak {t});"
            f" [betimsel] blok çözme P(({K},{R})) = {yuzde(blok(K, R, p), 2)}%"
        )
    # kayan nokta bıçak sırtı (ADR-17: aynı girdi → aynı çıktı)
    for K, p in [(8, 1 / 3), (4, 0.2), (8, 0.2)]:
        xf = K * p / (1 - p) * 1.25
        print(f"  [betimsel] float: K={K} p={p!r}: {xf!r} → ⌈⌉={ceil(xf)}")

    print("== FEC-08  §3.8 acil artıklık d = ⌈ln 0,1 / ln p⌉, en çok 8 ==")
    for p in (0.05, 0.1, 0.2, 0.3, 0.5, 0.7, 0.75, 0.8, 0.9, 0.95):
        ham = log(0.1) / log(p)
        d = min(8, ceil(ham))
        teslim = 1 - F(p).limit_denominator(1000) ** d
        print(f"  p={p:.2f}: ln0,1/lnp={ham:.4f} → d={d}  P(en az biri gelir)={yuzde(teslim, 2)}%")
    print(f"  tavan 8'in bağladığı eşik: p > 0,1^(1/8) = {0.1 ** (1 / 8):.5f}")
    print("  uç durum p=0: ln 0 tanımsız (IEEE: -inf → oran +0 → ⌈0⌉ = 0 kopya); p=1: ln 1 = 0 → bölme sıfır")


if __name__ == "__main__":
    main()
