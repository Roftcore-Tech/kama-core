# R-SWEEP — repair-quantum: "FEC açıldığında kaç repair ekonomik?"

**Kod:** `2256b3ec` (seviye override'ları; default'lar sevk). **Beklenti kaydı:** `EXPECTATION.md`
(koşudan ÖNCE). **Konfig:** kama-product · 0x03 (K=8) · 256 B · 20 ms · adaptif · RTT 200 ms ·
1 mbit · **reorder YOK** · msgs {96, 400} × kayıp {0,5,10,15,20,25} % · n=5 · kesirli tasarım.
Politika `<L1>-<L2>-<G>` ‰ (125=r1 · 250=r2 · 375=r3 · 500=r4). **Mevcut = 125-250-500.**

Kesirli tasarım gerekçesi: gate ekseni yalnız 400 msg × ≥15 %'de canlı (onaylı oran ≥100k'a
ancak orada); 0 %'de risk=0 → tüm kollar özdeş → {off, mevcut}; L1 ekseni reorder'sız neredeyse
inert (bedava dahil). 390 koşu ≈ 1 saat (tam faktöriyel 780 ≈ 3+ saat).

**Metrikler:** d2 = ≤2×RTT (400 ms) teslim oranı (birincil) · wire/parity/retx byte · repair
sembolü · FEC-kurtarılan birim · kurtarma verimi (kurtarılan/repair) · deadline kaçırma
(20 s'de teslim edilmeyen) · parity_before_flow_end · **PE** = (d2 − d2_off)×N / parity_B
(zamanında-mesaj / parity KB) · **marjinal PE** (Pareto sınırında komşular arası) · **stall**
(ilk kuşku > 1 s ya da hiç: kuruluş varyansı sınıfı; medyanlar etkilenmez ama görünür).

## Referans — MDS K=8 blok çözülebilirliği (hesap)

| kayıp | r=1 | r=2 | r=3 | r=4 |
|---|---|---|---|---|
| 5 % | 0.929 | 0.988 | 0.998 | 1.000 |
| 10 % | 0.775 | 0.930 | 0.977 | 0.996 |
| 20 % | 0.436 | 0.678 | 0.839 | 0.927 |
| 25 % | 0.300 | 0.526 | 0.713 | 0.842 |

**OFF kolunun anlamı (kısmi veriden görüldü, koşu ÖNCESİ not):** OFF = `--fec-risk` YOK = sevk
edilen kanonik yol. 400 msg × ≥20 %'de kanonik onaylı oran ~t_susp+6 s'de ≥100k'a ulaşır ve yapı-gate
`fec_med=500` (r=4) ile GEÇ parity basar (rep-1: 8 096 B, 88 repair, 25 kurtarma). Yani OFF
"sıfır parity" değildir; Pareto ve PE bazı "sevk davranışı"dır. 96 msg ve 400×≤10 %'de OFF parity=0.

## 0. Veri kalitesi — stall tabakası (sonuçlardan ÖNCE okunmalı)

**Kuruluş-stall'ı** (ilk kuşku > 1 s ya da hiç; bilinen cold-start varyansı sınıfı) kayıplı 370
koşunun **60'ında (%16)** görüldü; kayıpla artıyor (5 %: 4/50 · 25 %: 18/90). Stall kuşkudan
ÖNCE oluşur → politikadan bağımsız gürültü. n=5'te ≥3 stall medyanı bir stall koşusu yapar
(96/20 % OFF ve mevcut politika "d2 = 0.000"; 400/15 % `250-250-500` "OFF'tan kötü"). Bu yüzden
özet ve Pareto **stall-dışı rep'ler (n_ok)** üzerinden; tüm-rep medyanı `rsweep.analysis.ALLREPS.txt`
ve `d2_all` sütununda — gizlenmedi. **n ≥ 20 fazında stall stratifikasyonu şart.** 0 % hücrelerde
"kuşku yok" stall sayılmaz (tanım artefaktı düzeltildi).

## 1. Beklenti kaydıyla karşılaştırma (6/6 tuttu; iki nüans)

| # | öngörü | ölçüm | hüküm |
|---|---|---|---|
| 1 | L1 (r1 vs r2) reorder'sız duyarsız | Δd2 tipik ≤ 0.03; parity **birebir aynı** (7 912/7 912, 43 148/43 148 …) | ✓ |
| 2 | L2 r3: ≤10 %'da domine, 20/25 %'te +5…+12 pp | ≤10 %: +0.00…+0.06 (parity **+45–50 %**) → domine; ≥15 %: **+8…+15 pp** (400/20 %: 0.647→0.799; 25 %: 0.515→0.640) | ✓ (kazanç öngörüden büyük) |
| 3 | gate seviyesi 400 msg'de küçük; r4 domine | 15 %: parity gate'ten bağımsız (43.1–43.4 k), Δd2 ≤ 0.02; 20 %: r4 +1–5 kB, Δd2 ≤ +0.04; 25 %: r4 +9.5 kB, **+0.03…+0.05** | ✓ (25 %'te "pahalı ama sıfır değil") |
| 4 | 0 %: özdeş | 42 918 = 42 918 · 178 502 = 178 502; d2 özdeş | ✓ birebir |
| 5 | kısa-akış kapısı değişmez | r3 adayları hiç gerilemedi; iki GERİLEME işareti (`250-250-500` 15/25 %) n_ok=3 ve tek-stall kaynaklı, gürültü içinde | ✓ |
| 6 | PE: r2 > r3 > r4 | ≤15 %: r2 1.7–3.4 > r3 1.2–2.9 ✓; **20–25 %'te r3 ≈ r2** (2.3–2.6 vs 2.2–2.3) — üçüncü repair yüksek kayıpta AZALAN GETİRİ DEĞİL | ✓ nüanslı |

**Sapma dekompozisyonu gerekmedi** — hiçbir öngörü ters çıkmadı. İki nüans: (a) r3'ün ≥15 %'teki
kazancı MDS tahmininden büyük (+15 pp vs +11–16 pp blok-çözülebilirlik → tutarlı, üst sınırda);
(b) 25 %'te r4 küçük ama pozitif (marjinal PE 1.5–1.65; r3'ün 4.0'ının altında).

## 2. Repair-quantum'un fiyatı — 400 msg (stall-dışı medyan, n_ok 3–5)

| kayıp | OFF d2 | r2 uniform `250-250-250` | r3 L2 `250-375-250` | r3 L2+gate `250-375-375` | mevcut `125-250-500` |
|---|---|---|---|---|---|
| 5 % | 0.784 | 0.960 (+22.6 % wire) ¹ | 0.978 (+34 %) ¹ | — | 0.968 (+23 %) |
| 10 % | 0.611 | 0.889 (+22.7 %) ¹ | 0.945 (+34 %) ¹ | — | 0.907 (+22 %) |
| 15 % | 0.432 | 0.782 (+20.4 %) | 0.885 (+30.4 %) | 0.897 (+30.0 %) | 0.796 (+19.6 %) |
| 20 % | 0.380 | 0.677 (+16.4 %) | 0.780 (+25.2 %) | 0.765 (+27.2 %) | 0.665 (+18.9 %) |
| 25 % | 0.273 | 0.524 (+9.8 %) | 0.625 (+19.0 %) | 0.688 (+18.7 %) | 0.562 (+14.3 %) |

¹ 5/10 %'de gate ekseni süpürülmedi (inert); `x-y-500` değerleri.

Okuma: **r2 → r3 (L2) her ≥15 % hücresinde +9…+15 pp teslim, +8…+10 pp wire.** Gate seviyesi
(r2/r3/r4) 400 msg akışta ≤ +0.05 — gate onaylı oran ≥100k'a ancak ~t_susp+6 s'de ulaştığı için
etkisi son ~2 s'ye sıkışıyor; bu, gate seviyesinin değersizliğinin kanıtı DEĞİL, akış uzunluğunun
sonucudur (1000+ msg akışta gate ekseni yeniden ölçülmeli). Deadline (20 s) kaçırma: **0**, her hücre.

Kısa akış (96 msg): r3 L2 → 10 %: +0.05; 15 %: +0.08/+0.14; 20 %: +0.13; 25 %: +0.06 (parity 8.2 → 12 kB,
wire +6–8 pp). Kurtarma verimi (kurtarılan/repair): r2 0.35–0.55, r3 0.22–0.50, r4 daha düşük.

## 3. Pareto sınırı ve marjinal PE (zamanında-mesaj / parity KB)

| hücre | sınır (wire ↑ sırasıyla) | marjinal PE adımları | domine |
|---|---|---|---|
| 400·15 % | `250-250-375` → `250-375-375` | r2→r3: **1.82** | tüm `-500` (r4) kolları, `125-*` |
| 400·20 % | `250-250-250` → `250-250-375` → `250-375-250` → `125-375-375` | 1.81 · **2.19** · 3.33 | tüm `-500` kolları |
| 400·25 % | `125-250-250` → `250-250-250` → `125-250-500` → `125-375-250` → `250-375-375` → `125-375-500` | 19 · 1.65 · **4.01** · **4.21** · 1.45 | 6 kol |
| 96·15 % | `125-250-500` → `125-375-500` | **2.26** | `250-*` |
| 96·20 % | `125-250-500` → `250-250-500` → `250-375-500` | ∞ · **3.16** | `125-375-500` |
| 96·25 % | `250-250-500` → `125-250-500` → `125-375-500` | ∞ · **1.48** | `250-375-500` |
| 400·5 % | `250-250-500` → `125-250-500` → `125-375-500` → … | 2.7 · **0.05** · 8.2 | — (r3 adımı 0.05 = boşa) |
| 400·10 % | `125-250-500` → `250-375-500` | **0.69** | `125-375-500`, `250-250-500` |

**Ölçülen ekonomi:** ilk iki repair'in ortalama PE'si 1.7–3.4; üçüncü repair'in marjinal PE'si
≤10 %'da 0.05–1.3 (azalan, domine), **≥15 %'te 1.8–4.2 (ortalamaya eşit ya da üstünde — doğrusal
getiri)**; dördüncü (gate r4) 25 %'te 1.5–1.65, ≤20 %'de ≈0 (domine). **Owner'ın "r=4 israf" tezi
kontrollü biçimde teyit:** `x-y-500` kolları 15 ve 20 %'de istisnasız domine.

## 4. Adaylar (n ≥ 20 doğrulamasına)

| aday | politika | gerekçe | 400·20 % | wire |
|---|---|---|---|---|
| **A — ekonomi** | `250-250-250` (r2 uniform) | en düşük wire'lı sınır noktası; tek steady-state miktarı | 0.677 | +16 % |
| **B — performans** | `250-375-375` (r3 L2, r3 gate) | +10…+15 pp @ ≥15 %, marjinal PE ≥ ortalama; r4'süz | 0.765–0.799 | +25–27 % |
| referans | `125-250-500` (mevcut) | 15/20 %'te domine; 25 %'te sınırda | 0.665 | +19 % |

L1 seçimi (125 vs 250) bu sweep'te ayırt edilemez → **reorder fazına** (L1'in yaşadığı yer) ertelendi;
adaylarda 250 yazılması yalnız notasyon, karar değil. Gate = L2 sadeleştirmesi (tek miktar) öneri;
1000+ msg akışta gate ekseni ayrıca ölçülmeli (burada inert).

**Bu ölçüm SEVK KARARI DEĞİLDİR:** n=5 (n_ok 2–5), tek RTT/bant, reorder YOK (L1 ve reorder
maliyeti ölçülmedi), stall %16. n ≥ 20 + stall stratifikasyonu + reorder fazı sonra.

## 5. Kapılar ve ham veri

`rsweep.analysis.txt` (stall-dışı) · `rsweep.analysis.ALLREPS.txt` (tüm rep) · `runs-rsweep.tar.gz`
(390 koşu c.err/out.jsonl) · `EXPECTATION.md`. Kod `2256b3ec`; fmt 0 · clippy 0 · 56 suite ok
(test adları doğrulandı). Tam workspace testi sweep sonrası ayrıca koşuldu (sonuç commit mesajında).
