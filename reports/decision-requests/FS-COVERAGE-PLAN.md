> **[SUPERSEDED-BY-v1.1 — 2026-07-31]** Bu v1.0 (sha `91b33e1e`) tarihsel-referans; koşu-sayıları
> [FS-COVERAGE-PLAN-v1.1.md](FS-COVERAGE-PLAN-v1.1.md)'de güncellendi (F2-DUR-B → FS-1 540; confirmatory
> 670). Çelişkide v1.1 üstündür.

# FS-COVERAGE-PLAN — exact generated cells / sampling (DR-FEC-FS-1 v2 §5.1-D)

**Statü:** FS-2-ÖNCESİ dondurulur; FS-PREREG (sha ayrı) ile eşlik. Exact-hücre + sampling-kuralı; slow-diye-
sessizce-küçültme YASAK (§5.1). Bütçe-aşımında STOP + BUDGET-LIMITED + principled-reduction. Hash §PROVENANCE.

## 1. FS-1 SCREENING (stratified representative; Cartesian-DEĞİL)

```text
RTT           = {50, 250, 1000} ms          (düşük/mid/yüksek temsili; 3)
payload       = {300B} sabit                 (65/1200 → FS-2)
deficit       = {1, 2, 4}                     (3)
impairment    = {exact-deficit, IID-20%}      (2; ekonomik-temsili — IID5/short-burst FS-2'de)
util          = {0.80C} sabit
base-cells    = 3×3×2 = 18
arms          = {A0, A1-EARLY(r=d), F0, F1, F2, F3, F4} = 7   (F2 seal-map-sonrası; çelişki→DUR-B → arm-düşer)
seeds         = 5
FS-1 TOPLAM   = 18 × 7 × 5 = 630 run   (≤2400 tavan)
```
Aile-eleme yalnız §12-kriteri (mekanik-decode-fail / kalıcı-source-starvation / sınırsız-maliyet / parity-
sistematik-kullanılamaz / invariant-ihlali). Tek-kötü-hücre eleme-DEĞİL.

## 2. CONFIRMATORY (FS-PREREG §4; ön-ilan sabit-hücre; N=10 paired)

```text
C-P1 rtt250 × def{1,2,4} × {A0,A1(r=d),F0(r=d)} × 10        = 3×3×10 =  90
C-P2 rtt{1000,2500} × def{1,2} × {A0,F0,F1,F3} × 10         = 2×2×4×10 = 160
C-P3 rtt250 × revblk{5s,30s} × {A0,A1(r2),F0(r2),F1(r2)}×10 = 2×4×10 =  80
C-P4 rtt{250,1000} × def{1,2,4} × {A1(r),F0(r),F1(r)} × 10  = 2×3×3×10 = 180
C-P5 rtt250 × def{1,2,4} × {A0(r0),A1(r=d),F0(r=d),F1(r=d)}×10 = 3×4×10 = 120
CONFIRMATORY TOPLAM                                          = 630 run   (≤1200 tavan)
near-equivalence → 30-seed genişleme (yalnız sınıra-yakın-hücre; ek ≤ +1200 run, tavan-içinde)
```

## 3. FS-2 SURFACE (yalnız FS-1-sağkalan-arm; space-filling → focused-confirm)

```text
RTT           = {50,100,250,350,500,650,750,1000,1500,2500,5000} (250–1000 yüksek-yoğunluk)
bandwidth     = {25,50,100,250,500 bit/s · 1,2,5,10,50,100 kbps}  (faithful-üretilemeyen → INSTRUMENT-LIMITED)
IID-loss      = {1,5,10,20,30,40,60,90%}
burst         = matched-realized-mean {2,5,10,20 pkt}; persistence AYRI-varyasyon
reverse-loss  = {0,30,60,90,99%} · blackout {1,5,10,30,60s} (tekrarlı dahil) · **reverse-bw AYRI-eksen** (asimetrik return-kapasite)
jitter        = {0,10,50,200,500ms} · reorder {0,1,5,10%}
headroom      = {0.25,0.50,0.80,0.95,1.00,1.10,1.25}C (finite-queue + explicit-drop)
traffic-shape = {single-msg, msg-burst, continuous, mixed-data+control}  (warm vs cold-start AYRI-analiz; yeni-cold-FEC-YOK)
```
space-filling-sampler: budget-tavanına-kadar Latin-hypercube-benzeri stratified; exact-üretilen-liste FS-2-
öncesi bu-dosyaya-eklenir + re-hash (FS-1-sağkalanları bilinmeden tam-liste-donmaz — §13 "sağkalan-arm-only").

## 4. FS-3 PROFILE (bounded)
```text
K = {2,4,8,16,32} × R = {0,1,2,4,8}  (geçersiz atlanır; R≤K, R≤profile_r_max)
sağkalan-aile × rejim-temsili-alt-set × 5-seed. + DR-1b faithful-revalidation (Leftover vs sağkalan-policy).
```

## 5. SESSION-BOUNDED EXECUTION ORDER (principled; tavan-altı)
```text
1. FS-HARNESS BUILD (ön-koşul; result-bearing-DEĞİL): trace-driven replay-injector + A0/A1/F0–F4 arms +
   9-timestamp instrumentation + metadata + per-run-instrument-chain. (Bu inşa DUR-A#1-SONRASI.)
2. FS-1 screening (630 run) → DUR-A aile-eleme
3. C-P1..C-P5 confirmatory (630 run)
4. FS-4 oracle-gate → DUR-A
5. (oracle-upside-varsa) FS-2 surface → FS-5 observability → FS-6 policy → FS-7 held-out → FS-8 compare
Oturum-kapasitesi bir-adımı-kaldırmazsa: kapsanan AÇIK-listelenir, kalan=BUDGET-LIMITED (interpolasyon-YOK).
```

## PROVENANCE
```text
sha256: 91b33e1e2dee5d87f9809e986ea4b154f246c66c30cf3a52fb9cefa9a46cb8bb
tarih : 2026-07-31T11:41:36Z
prereg: FS-PREREG.md (ayrı-hash)
```
