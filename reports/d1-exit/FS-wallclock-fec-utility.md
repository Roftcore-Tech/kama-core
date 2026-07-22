# FS — FAITHFUL WALL-CLOCK MARGINAL FEC UTILITY BOUNDARY (Phase-C/D/E)

> **Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine eklenen
> proaktif FEC'in marjinal faydasıdır.** ARQ/substrat (§9-retransmit + §11-forward-recovery) hiçbir kolda
> kapatılmadı. r0=ARQ+0 … r4=ARQ+4. Residual daima ARQ ile tamamlanır (hibrit).

## HÜKÜM (utility yarısı)

> **FEC WALL-CLOCK UTILITY IDENTIFIED — koşula-bağlı.** Proaktif-FEC'in marjinal WALL-CLOCK faydası
> **RTT ≤ 250ms rejiminde POZİTİF ölçüldü** (MarginalFecBeneficial; sıkı-deadline'da MarginalFecEssential);
> **RTT ≥ 1000ms'te NoMarginalFecUtility_thisCondition** (bu emisyon-modelinde). Bu, son-turda kurulamayan
> gerçek-zaman-kazancının FAITHFUL harness'ta İLK KEZ pozitif-ölçümüdür.

## 1. FAITHFUL HARNESS (Phase-B kapısı — geçildi)
Two-netns rootless netem (FQ; acceptance 6/6). Bu koşuda ölçülen-RTT: rtt250=250.09ms, rtt1000=1000.18ms,
rtt2500=2500.33ms (configured-exact). Rootless-tek-ns netem-yalan-gecikmesi DEĞİL. 48 koşu / 0-fail.

## 2. PHASE-C — TIMING FEASIBILITY (frozen-deadline; winner-görülmeden)
Hybrid; T_fec_avail = parity-arrival-rel; T_arq = substrat-onarım-arrival-rel (block-first'e göreli → one-
way-delay-iptal). Frozen-deadline-interval VAR ⟺ p99(T_fec) < p01(T_arq).

| RTT | deficit,r | N | T_fec p50/p99 (ms) | T_arq p50/p01 (ms) | interval? |
|---|---|---|---|---|---|
| 250ms | d1,r1 | 30 | 287 / **293** | 351 / **328** | **YES** |
| 250ms | d2,r2 | 30 | 287 / **296** | 368 / **315** | **YES** |
| 250ms | d4,r4 | 30 | 282 / **296** | 350 / **325** | **YES** |
| 1000ms | d1,r1 | 30 | 1062 / 1914 | 1074 / 1037 | no/overlap |
| 1000ms | d2,r2 | 30 | 1504 / 2266 | 1089 / 1038 | no/overlap |
| 1000ms | d4,r4 | 30 | 2480 / 3183 | 1141 / 1050 | no/overlap |
| 2500ms | d1,r1 | 30 | 2671 / 4870 | 2647 / 2602 | no/overlap |
| 2500ms | d4,r4 | 30 | 6401 / 8292 | 2875 / 2616 | no/overlap |

**Frozen-deadline (rtt250, pre-declared):** interval (296, 315) → deadline = **305ms**.

## 3. PHASE-D/E — MARGINAL FEC WALL-CLOCK UTILITY (hybrid; r>=d vs r<d; 60 blok/hücre)

| RTT | pin | deficit | r<d comp p50/p95 | r>=d comp p50/p95 | FEC-path(r>=d) | SINIF |
|---|---|---|---|---|---|---|
| 250ms | pin1 | 1 | 333/393 | **287/292** | **30/30** | **MarginalFecBeneficial** (Δ−46ms) |
| 250ms | pin2 | 2 | 352/372 | **300/307** | **30/30** | **MarginalFecBeneficial** (Δ−52ms) |
| 250ms | pin3 | 4 | 392/453 | **315/332** | **30/30** | **MarginalFecBeneficial** (Δ−77ms) |
| 1000ms | pin1 | 1 | 1074/1125 | 1042/1128 | 15/30 | NoMarginalFecUtility_thisCondition |
| 1000ms | pin2 | 2 | 1101/1195 | 1117/1203 | 6/30 | NoMarginalFecUtility_thisCondition |
| 1000ms | pin3 | 4 | 1211/1507 | 1194/1454 | 0/30 | NoMarginalFecUtility_thisCondition |
| 2500ms | pin1 | 1 | 2635/2835 | 2633/2786 | 14/30 | NoMarginalFecUtility_thisCondition |
| 2500ms | pin2 | 2 | 2683/3064 | 2685/2919 | 6/30 | NoMarginalFecUtility_thisCondition |
| 2500ms | pin3 | 4 | 3074/3523 | 3070/3726 | 0/30 | NoMarginalFecUtility_thisCondition |

**FROZEN-DEADLINE (305ms) uygulaması @ rtt250:** r<d (comp 333-453) → **DEADLINE MISS %100**; r>=d (comp
287-332) → **DEADLINE MEET ~%100** → deficit=1/2/4 için **MarginalFecEssential** (ARQ+0 deadline-kaçırır,
ARQ+r deadline-yakalar). Bu, direktifin `MarginalFecEssential` sınıfının tam-tanımıdır.

## 4. MEKANİZMA (neden düşük-RTT'de faydalı, yüksek-RTT'de değil — SEZGİYE-AYKIRI ama ÖLÇÜLÜ)

Naif-beklenti: FEC round-trip'ten-kaçındığı-için yüksek-RTT'de DAHA-faydalı olmalı. ÖLÇÜLEN tersi. Neden:
**parity BLOK-KAPANIŞINDA emit-edilir** (tüm-K-source dispatch-sonrası). Source dispatch ACK-clocked
(offered≤0.6C). Düşük-RTT'de blok-hızlı-kapanır → parity-erken-gider → T_fec (~intra-block-span+one-way,
RTT'den-görece-bağımsız ~287ms) ARQ-round-trip'ten (~350ms) ÖNCE gelir. Yüksek-RTT'de blok-kapanışı ACK-
clocking'le ~1-RTT-gecikir → parity-geç-gider → T_fec (~1062ms@1s, ~2671ms@2.5s) ARQ-onarımına (~1074/
2647ms) EŞİTLENİR/AŞAR. Yani proaktif-FEC'in "round-trip-atlama" avantajı, blok-kapanış-gecikmesiyle silinir.

T_fec'in r ile büyümesi (rtt1000 d4: 2480ms; rtt2500 d4: 6401ms) tutarlı: daha-çok-parity → blok-kapanış-
sonrası daha-çok-emit → daha-geç-arrival → FEC-path-kullanımı r↑ ile düşer (rtt1000: 15/30→6/30→0/30).

**SINIR ⇒ EMİSYON-MODELİNE-BAĞLI:** bu bulgu MEVCUT emisyon-modeli (parity@blok-kapanış + ACK-clocked-source)
içindir. Parity first-flight'ta-source-yanında-gönderilse yüksek-RTT'de-de-kazanabilir → gelecek-tasarım-
girdisi (bu-turda-DEĞİŞTİRİLMEDİ).

## 5. ISO ANCHOR (delay-bağımsız kombinatoryal; her-RTT'de)
rtt250/1000/2500 hepsi: iso d1 r0 → 30 PENDING (FEC-imkansız, 7<8); r1 → 30 FEC-COMPLETE (7+1=8).
Kombinatoryal-decodability (`r≥d`) RTT'den-BAĞIMSIZ (FK-bulgusu tekrar-doğrulandı). Wall-clock-LATENCY-
avantajı ise RTT-kapılı (§3-4).

## 6. UTILITY-SINIFLARI (ölçülen)
- `MarginalFecEssential` — rtt250, deadline=305ms, deficit 1/2/4 (r>=d meet, r<d miss).
- `MarginalFecBeneficial` — rtt250, deadline-siz (r>=d 46-77ms-hızlı; FEC-path 30/30).
- `NoMarginalFecUtility_thisCondition` — rtt1000/2500 tüm-deficit (parity blok-kapanış-gecikmesiyle ~ARQ).
- `MarginalFecHarmful` — ölçülmedi (rtt2500 d4 T_fec≫T_arq ama completion-band-içinde; parity-byte-waste
  ayrı-metrik, bu-koşuda-completion'ı-kötüleştirmedi).
- `HybridResidualArqRequired` — iso-mod r<d (FK): FEC-yetmez, ARQ-tamamlar (her-RTT).

## 7. KAPSAM
Üretim fuzzy-config + LeftoverOnly + byte-layout DEĞİŞMEDİ; deficit-enjektör test-only. Duplication
substrat-aktif (hybrid); §11-forward-recovery-izolasyonu bu-koşuda ayrı-taranmadı (izleme-maddesi).
Deadline offline-frozen (winner-görülmeden Phase-C'den). Veri: `data/wallclock/` (48 koşu, ANALYSIS via
`fec_wallclock_analyze.py`, RTT_PINS faithful-kanıt).
