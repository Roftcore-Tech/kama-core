#!/usr/bin/env python3
"""KTP-2 taslak 0.3 denetimi (Ajan E) — §4 çerçeve boyları, §4.1 bant yükü tablosu, §1 '64 B
sembole 37 B çerçeve'. Bugünkü tel değerleri a858f3fc kaynağından (dosya:satır yorumlarda).

Kullanım: python3 -I scripts/ktp2/f0/e/yuk_tablosu.py

Kaynak sabitleri (a858f3fc):
  LEGACY zarf 22 B ........ crates/kama-noise-wire/src/legacy.rs:25 (ürün: kama-node wiring.rs:47 Legacy)
  EXTENDED zarf 26 B ...... crates/kama-transport-schema/src/lib.rs (EXTENDED_OUTER_OVERHEAD)
  DATA başlığı 11 B ....... crates/client-core/src/wire.rs:17 (HDR_DATA)
  DATA_FRAG başlığı 22 B .. crates/client-core/src/wire.rs:59 (HDR_DATA + SOURCE_PREFIX_LEN)
  FEC çerçevesi 12 B ...... crates/client-core/src/wire.rs:105 (HDR_FEC)
  FEC iç başlığı 11 B ..... crates/kama-noise-wire/src/inner.rs:77 (FEC_INNER_HDR_LEN; noise-wire yolu)
  SOURCE_PREFIX 11 B ...... crates/kama-transport-schema/src/lib.rs (sembolün İÇİNDE)
  ürün FEC profili 0x05: K 8, R_max 4, E 1024 .. client-core/src/runtime.rs:240, kama-fec-codec/src/profile.rs:86-91
"""
from math import ceil

LEGACY = 22
EXT = 26
HDR_DATA = 11
HDR_DATA_FRAG = 22
HDR_FEC = 12
FEC_INNER = 11
PREFIX = 11


def main():
    print("== Y4-01  §4 çerçeve alan toplamları ==")
    cer = {
        "AKIŞ": ([1, 2, 4, 2], 9),
        "ONARIM": ([1, 2, 4, 1, 1, 1, 2], 12),
        "ACK (n=0)": ([1, 4, 2, 1], 8),
        "KURTARILDI": ([1, 2, 4, 2], 9),
        "KREDİ (n=0)": ([1, 4, 1], 6),
        "AKIŞ BİTİŞİ": ([1, 2, 4], 7),
        "AKIŞ İPTALİ": ([1, 2, 1], 4),
        "BLOK DURUMU": ([1, 2, 4, 1], 8),
        "YOKLAMA": ([1], 1),
    }
    for ad, (alan, t) in cer.items():
        print(f"  {ad}: {'+'.join(map(str, alan))} = {sum(alan)} (taslak {t}) {'✓' if sum(alan) == t else '✗'}")
    print(f"  KREDİ tek girdi: 6 + 6·1 = 12 B (taslak §4.1 'akış başına 6 B' yalnız ek girdiyi sayar)")

    print("== Y41-01  §4.1 veri paketi başlığı: bugün 22 + 11 = 33, KTP-2 22 + 9 = 31 ==")
    print(f"  bugün LEGACY {LEGACY} + DATA {HDR_DATA} = {LEGACY + HDR_DATA} ; KTP-2 {LEGACY} + 9 = {LEGACY + 9}")

    print("== Y41-02  §4.1 '300 B mesajın FEC yolundaki tel boyu (onarım hariç) = 6 × (64 + 37) = 606 B' ==")
    E = 64
    n = ceil(300 / (E - PREFIX))
    print(f"  (a) taslağın modeli = noise-wire EXTENDED FecShard, E=64: yük/sembol {E - PREFIX} B → {n} sembol × ({E} + {EXT}+{FEC_INNER}) = {n * (E + EXT + FEC_INNER)} B")
    print(f"  (b) ÜRÜN a858f3fc (profil 0x05, parçalayıcı kapalı): kaynak = 0x00 DATA, sembol tele ÇIKMAZ:")
    print(f"      kaynak {LEGACY}+{HDR_DATA}+300 = {LEGACY + HDR_DATA + 300} B ; her onarım {LEGACY}+{HDR_FEC}+1024 = {LEGACY + HDR_FEC + 1024} B")
    full, last = divmod(300, E - PREFIX)
    frag = full * (LEGACY + HDR_DATA_FRAG + E - PREFIX) + (LEGACY + HDR_DATA_FRAG + last if last else 0)
    print(f"  (c) E=64 profili + parçalayıcı açık (0x05 DATA_FRAG, client-core yolu): {full}×{LEGACY + HDR_DATA_FRAG + E - PREFIX} + {LEGACY + HDR_DATA_FRAG + last} = {frag} B ;"
          f" her onarım {LEGACY}+{HDR_FEC}+{E} = {LEGACY + HDR_FEC + E} B")
    print(f"  KTP-2: 31 + 300 = 331 B ; onarım 22 + 12 + 300 = 334 B")
    print(f"  (a)→KTP-2: tel {606}→331 = %{331 / 606 * 100:.1f} ; ek yük {606 - 300}→31 = %{31 / 306 * 100:.1f}")
    print(f"  (b)→KTP-2: tel {LEGACY + HDR_DATA + 300}→331 = %{331 / 333 * 100:.1f}")

    print("== Y1-01  §1 '64 baytlık sembollere 37 bayt çerçeve (taban yükü yaklaşık iki kat)' ==")
    print(f"  E=64, EXTENDED: (64+37)/64 = {(64 + 37) / 64:.3f}× sembol başına ; (64+37)/53 = {(64 + 37) / 53:.3f}× yük başına")
    print(f"  ürün (E=1024) kaynak: ek yük 33 B (FEC'siz DATA ile aynı) ; onarım: 1058 B (yükten bağımsız)")
    for yuk in (65, 300, 1013):
        src = LEGACY + HDR_DATA + yuk
        print(f"    {yuk} B mesaj: kaynak {src} B, tek onarım 1058 B = kaynağın {1058 / src:.2f} katı")


if __name__ == "__main__":
    main()
