# FEC-MARGINAL-R-V2 — ÖN-KAYIT (dondurulmuş)

**v1'in tanımlayıcı bulgusunu HÜKME çevirmek için, YENİ VERİYLE.** v1 verisini 2×RTT'de yeniden
skorlamak post-hoc endpoint kaymasıdır ve bu repo bunun bedelini defalarca ödedi; bu yüzden v2
bağımsız repler koşar. v1'in verisi yalnız GÜÇ HESABI için kullanıldı.

Taban sözleşme: `reports/fec-marginal-r-v1/PRE-REGISTRATION.md` (commit `9a39af89`).
Aşağıdaki DELTA dışında her şey aynen geçerlidir.

## Δ1 — Birincil uç nokta: `d_msg = 2×RTT` (400 ms)

v1 hipotezi doğruladı: etki sıkı deadline'da yaşıyor, gevşekte kayboluyor. v1'de birincil 4×RTT
donmuştu ve etki oraya düşmedi. v2 birincili sıkı eşiğe koyar.

## Δ2 — Çöküş ÇIPASI birincilden AYRI: `16×RTT`

v1'de çöküş ve birincil aynı eşikteydi; completion ~1.0'da toplandığı için sorunsuzdu. 2×RTT'de
r0 completion'ı ~0.53 — yani 0.5 çöküş eşiği dağılımın TAM ORTASINA düşer ve "çöküş" artık
kuruluş başarısızlığını değil deadline kaçırmayı ölçer; iki farklı olgu karışır.
**Çöküş = 16×RTT'de completion < 0.5** ("rep hiç işlemedi"). Birincil skor, işleyen repler
arasında 2×RTT'de.

## Δ3 — Kollar: `r ∈ {0, 2}` (r=4 DÜŞÜRÜLDÜ)

v1 ölçtü: r=4, r=2'nin üstüne kurtarma EKLEMİYOR (cor20 781 vs 782; iid20 737 vs 767) ama iki
katı parity gönderiyor (verim 0.16 → 0.08). Bu ÖN-KAYITLI bir karardır ve bütçeyi n'e çevirir.

## Δ4 — n = 60 / (kol × hücre)

v1'den ölçülen rep-düzeyi sd (2×RTT, işleyen repler) ≈ 0.15 → n=60'ta %80-güç
MDE ≈ 2.8·0.15·√(2/60) ≈ **7.7 pp**. v1'in gözlediği etki ~14-22 pp, yani tespit edilebilir.
2 kol × 2 hücre × 60 = **240 rep** ≈ 15 dk.

## Δ5 — Hüküm kuralı (v1 §8 ile aynı, birincil eşik 2×RTT)

- **MARGINAL-R-FAYDALI** ⟺ r2'nin koşullu completion'ı r0'a göre CI-ayrık ARTIŞ **ve** çöküş
  oranı CI-ayrık ARTMAZ.
- **ETKİSİZ** ⟺ CI'lar örtüşür (temas ayrık SAYILMAZ). **ZARARLI** ⟺ CI-ayrık AZALIŞ.
- Fayda ilan edilse bile teslim-başına byte birlikte raporlanır.

## Bu kampanyanın CEVAPLAMADIĞI şeyler
v1 ile aynı: sevk edilen politika · quinn kıyası · %20 dışı kayıp / 200 ms dışı RTT / 1 mbit dışı
bant / 256 B dışı yük · blackout.

## Freeze kaydı

- ön-uçuş `sha256 = 5793764a61385a149c4a958d3bd469d7c514984bba7c37e6bdf7be12c6a2b2ec`
- koşucu `scripts/netem/fec_marginal_r.sh` (`ARMS_OVERRIDE="0 2"`)
- analiz `scripts/netem/fec_marginal_r_analyze.py` (`PRIMARY_MULT=2 COLLAPSE_MULT=16`)
- Bu belge + ön-uçuş çıktısı ilk rep'ten ÖNCE, bu commit'te iner.
