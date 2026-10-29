#!/usr/bin/env python3
"""KTP-2 taslak 0.3 denetimi (Ajan E) — §3.9 serileştirme alt sınırları, §5 sınır formülleri,
§3.2/§3.7 kredi pencereleri, §3.10/K-4 kesinti, §3.11 ACK tavanı, §4 KREDİ alan genişliği.

Kullanım: python3 -I scripts/ktp2/f0/e/sinirlar.py

Ortak model: tel = yük + 31 (zarf 22 + AKIŞ 9) + L, L ∈ {0, 28, 42} (bkz. iletim_dusuk_bant.py).
Kayıp bağımsız Bernoulli. "Pay" = zamanlayıcının akışa verdiği bayt payı (§3.6 ağırlıkları).
"""
from fractions import Fraction as F
from math import comb, floor

KTP2 = 31
ACK1 = 22 + 8 + 4  # ACK paketi, 1 aralık
L_SECENEK = [(0, "L yok"), (28, "IPv4+UDP"), (42, "Eth+IPv4+UDP")]


def S(hiz, t_ser):
    return max(64, min(1100, hiz * t_ser - KTP2))


def negbin_asim(n, p, esik):
    """İdeal ARQ: n parçanın teslimi için kayıp sayısı T ~ NegBin(n, q=1−p).
    P(T/n > esik) kesin (Fraction)."""
    q = 1 - p
    tmax = floor(esik * n)  # T ≤ tmax ⇔ T/n ≤ esik
    alt = sum(comb(n + k - 1, k) * q**n * p**k for k in range(0, tmax + 1))
    return 1 - alt


def main():
    print("== S39-01  §3.9 '(64+31)/2 ≈ 47,5 B/s' ve '31 B/s'de ≈ 3,1 s' ==")
    for L, ad in L_SECENEK:
        en_kucuk = 64 + KTP2 + L
        print(f"  {ad}: en küçük paket {en_kucuk} B → T_ser 2 s eşiği {en_kucuk / 2:.2f} B/s ; 31 B/s'de {en_kucuk / 31:.3f} s ;"
              f" T_ser 8 s eşiği {en_kucuk / 8:.3f} B/s")
    s = S(31, 8)
    print(f"  31 B/s, ses YOK (T_ser 8 s): S={s} B → paket {s + KTP2} B → {(s + KTP2) / 31:.2f} s ; ses etkin: S={S(31, 2)} B")
    print(f"  64 B parçada başlık payı 31/95 = {31 / 95 * 100:.1f}% ; payın 1/3'ü geçtiği eşik S < 62 B (31/(62+31) = 1/3)")
    print(f"  L=42 ile 64 B parçada başlık payı 73/137 = {73 / 137 * 100:.1f}%")

    print("== S5-01  §5 hizmet payı: 1000 B/s, ses 300 B/s, mesaj+dosya dolu → mesaj ≥ 560, dosya ≥ 140 ==")
    r = 1000
    for ad, r_ses, w in (
        ("taslak (ses 300, kontrol boş)", 300, (4, 1, 0)),
        ("kontrol da dolu (4/4/1)", 300, (4, 1, 4)),
        ("ses §3.6 tavanında (1,5×305=457,5)", F(915, 2), (4, 1, 0)),
        ("ses %30 FEC (4,4), 200 ms: 625", 625, (4, 1, 0)),
    ):
        kalan = r - r_ses
        top = sum(w)
        print(f"  {ad}: r−r_ses={float(kalan):.1f} → mesaj ≥ {float(kalan * w[0] / top):.1f}, dosya ≥ {float(kalan * w[1] / top):.1f} B/s (tel baytı)")

    print("== S5-02  §5 teyitsiz veri örneği: 31 B/s, W=16, S=64, pay %20 → ≈ 245 s ==")
    for ad, s_, pay in (
        ("taslak (S=64, pay 1/5)", 64, F(1, 5)),
        ("§3.9 ses yok (S=217), pay 1/5", 217, F(1, 5)),
        ("S=64, kontrol da dolu (pay 1/9)", 64, F(1, 9)),
    ):
        for L, lad in L_SECENEK[::2]:
            t = 16 * (s_ + KTP2 + L) / (31 * pay)
            print(f"  {ad}, {lad}: 16×{s_ + KTP2 + L}/{float(31 * pay):.3f} = {float(t):.1f} s")

    print("== S5-03  §5 'bugünkü motorun aynı koşuldaki en kötü sınırı ≈ 6,4 gün' (kaynağın yeniden kurulumu) ==")
    print("  kaynak: 05b notu (aday 49864510) docs/design/K4G-05b-RETX-FIX.md B.1d satır 45b ve B.6 h.9.7:")
    print("  T_birim(R1) ≤ max(H_maks, A) + A + N_cap·T_max(kopya) + τ; N_cap = 4096, birim 1046 B")
    for hiz, not_s in ((31, 553350), (100, 171590)):
        baskin = 4096 * 4 * 1046 / hiz
        print(f"  {hiz} B/s: baskın terim 4096×4×1046/{hiz} = {baskin:.0f} s = {baskin / 86400:.3f} gün ; notun değeri {not_s} s = {not_s / 86400:.3f} gün"
              f" (fark %{abs(baskin - not_s) / not_s * 100:.2f})")
    b64 = 4096 * 4 * (64 + 33) / 31
    print(f"  aynı baskın terim 64 B yükle (bugünkü tel 64+33=97 B), 31 B/s tam hız: {b64:.0f} s = {b64 / 3600:.1f} sa ;"
          f" pay %20 ile ×5: {5 * b64 / 86400:.2f} gün")

    print("== S5-04  §5 israf sınırı: yeniden gönderim / teslim ≤ 1,5·p/(1−p) ==")
    for p in (F(1, 10), F(2, 10), F(3, 10)):
        ideal = p / (1 - p)
        print(f"  p={float(p):.1f}: ideal ARQ beklenen oran p/(1−p) = {float(ideal):.4f} ; sınır 1,5× = {float(F(3, 2) * ideal):.4f}")
    print("  [kesin] ideal ARQ'nun tek koşuda sınırı AŞMA olasılığı, n parça (T ~ NegBin(n, 1−p)):")
    for p in (F(1, 10), F(2, 10), F(3, 10)):
        esik = F(3, 2) * p / (1 - p)
        satir = f"  p={float(p):.1f}:"
        for n in (1, 10, 30, 100, 300, 1000):
            satir += f" n={n}: {float(negbin_asim(n, p, esik)) * 100:.2f}%;"
        print(satir)
    print("  FEC onarımı sayılırsa, R = ⌈K·p/(1−p)·1,25⌉ ile R/K (sınırla kıyas):")
    for K in (4, 8):
        for p in (F(1, 10), F(2, 10), F(3, 10)):
            x = K * p / (1 - p) * F(5, 4)
            R = -(-x.numerator // x.denominator)
            sinir = F(3, 2) * p / (1 - p)
            print(f"    K={K} p={float(p):.1f}: R={R}, R/K={float(F(R, K)):.4f} vs sınır {float(sinir):.4f} → {'AŞAR' if F(R, K) > sinir else 'içinde'}")

    print("== S5-05  §5 bekleme sınırına kesilemeyen paket terimi (taslağın kendi K-1/§3.9 terimi) ==")
    for hiz in (31, 1000, 12500):
        for t_ser, ad in ((8, "ses yok"), (2, "ses etkin")):
            s_ = S(hiz, t_ser)
            print(f"  {hiz} B/s {ad}: en büyük paket {s_ + KTP2} B → (S_maks+başlık)/r = {(s_ + KTP2) / hiz:.3f} s")

    print("== KR-01  §3.2 kredi pencereleri: parça × S_maks ve bant-gecikme çarpımı ==")
    for ad, parca, kb in (("STANDART mesaj", 32, 32), ("STANDART sinyal", 16, 16), ("ARKA PLAN dosya", 16, 64)):
        print(f"  {ad}: {parca} × 1100 = {parca * 1100} B ; bayt sınırı {kb} KB = {kb * 1024} B → "
              f"{'bayt sınırı HİÇ bağlamaz' if parca * 1100 < kb * 1024 else 'bayt sınırı S > ' + str(kb * 1024 // parca) + ' B iken bağlar'}")
    for hiz in (25, 1000, 12500):
        for rtt in (F(6, 10), F(2)):
            bdp = hiz * rtt
            s_ = S(hiz, 8)
            w_dosya = min(16 * s_, 64 * 1024)
            tavan = w_dosya / rtt
            print(f"  r={hiz} B/s RTT={float(rtt)} s: BDP={float(bdp):.0f} B ; dosya penceresi 16×{s_}={w_dosya} B → "
                  f"pencere/RTT={float(tavan):.0f} B/s ({'≥ r' if tavan >= hiz else f'r’nin %{float(tavan / hiz) * 100:.1f}'})")

    print("== KR-02  §4 KREDİ 'oturum bayt sınırı 4 B' + 'mutlak, yalnız artar' → u32 taşma süresi ==")
    for hiz, ad in ((25, "25 B/s"), (1000, "1 kB/s"), (12500, "12,5 kB/s"), (1_250_000, "10 Mbit/s (K-9)")):
        t = 2**32 / hiz
        print(f"  {ad}: 2^32 B / r = {t:.0f} s = {t / 86400:.2f} gün = {t / 60:.1f} dk")

    print("== KR-03  §3.7 kredi güncellemesi '3 ACK'te' — üçünün de kaybolma olasılığı p_dönüş³ ==")
    for pr in (F(1, 10), F(3, 10), F(5, 10), F(8, 10)):
        print(f"  p_dönüş={float(pr):.1f}: {float(pr**3) * 100:.1f}%")

    print("== AK-01  §3.11 ACK yükü ≤ ileri veri hızının %5'i → ACK aralığı alt sınırı ==")
    for hiz in (25, 1000, 12500):
        for L, ad in L_SECENEK[::2]:
            print(f"  ileri {hiz} B/s, ACK {ACK1 + L} B ({ad}): aralık ≥ {(ACK1 + L) / (F(5, 100) * hiz):.3f} s")

    print("== PT-01  §3.10 'PTO yoklaması 30 s'de bir, ≤ 48 B (≤ 1,6 B/s)' ile §3.4 'PTO en eski teyitsiz parçayı gönderir' ==")
    print(f"  48/30 = {48 / 30:.2f} B/s")
    for s_ in (64, 217, 1100):
        for L, ad in L_SECENEK[::2]:
            print(f"  parça S={s_}: paket {s_ + KTP2 + L} B ({ad}) / 30 s = {(s_ + KTP2 + L) / 30:.2f} B/s")
    print(f"  48 B'a sığan içerik: zarf 22 + YOKLAMA 1 + ACK 8+4n → n ≤ {(48 - 22 - 1 - 8) // 4} aralık; AKIŞ çerçevesiyle en çok {48 - 31} B yük")

    print("== K4-01  K-4 'kesinti sonrası ilk teslim ≤ 30 s + 2 RTT' ==")
    for hiz in (25, 1000, 12500):
        s_ = S(hiz, 8)
        ser = (s_ + KTP2) / hiz
        for rtt in (0.6, 2.0):
            veri = 30 + ser + rtt / 2  # PTO veri parçası taşıyorsa
            yok = 30 + 48 / hiz + rtt + ser + rtt / 2  # yalnız 48 B yoklama → ACK → veri
            print(f"  r={hiz} RTT={rtt}: sınır {30 + 2 * rtt:.2f} s | PTO veri taşır: {veri:.2f} s | yalnız yoklama: {yok:.2f} s")


if __name__ == "__main__":
    main()
