# KESİT-1 VALIDITY-AUDIT (Abkarada-denetim-direktifi; harness-only-instrumentation)

> Ham-veri: [kesit-1-seed-level.csv](data/kesit-1-seed-level.csv) (432-satır = 24-nokta × 6-executed-arm × 3-seed;
> audit-sütunları: parity enq/emit, blocks, censored, fwd-attempts/drops/burst, FusionInput-KISMİ-telemetri).
> Statü: kesit-1 = **EXPLORATORY SCREENING — NOT YET CALIBRATION EVIDENCE.**

## 1. Terminoloji (düzeltilmiş)
**6 distinct executed arms / 7 logical labels** (ARQ-orta≡ARQ-baseline; istatistikte-tek-sayılır) ·
**24 scenario points** · **144 run rows** (×3-seed = 432-koşu). ARQ-armları = **SACK-cadence-PROXY-arms**
("feedback cadence sensitivity screening"; ARQ-actuator-kalibrasyonu-DEĞİL).

## 2. 24-nokta goodput-hükmü (daraltılmış)
"Bu tek-eksen-uzayındaki 24-scenario-noktasının hiçbirinde sabit-FEC-arm'ı en-yüksek goodput_per_byte'a
ulaşmadı." ÇIKARILAMAYANLAR: ARQ-genel-üstünlüğü · FEC-gereksizliği · R1/R2-doğrulaması · yüzey-kalibrasyonu.
**loss-30% karşı-örneği (goodput≠completion):** completion — ARQ-baseline **0.998** (3/3-censored) · ARQ-hafif
0.998 · ARQ-agresif 1.000 · **FEC-r2/r4/r8 = 1.000/1.000/1.000 (tüm-seed'ler)**.

## 3. ARM-COLLAPSE / requested-vs-realized-parity (kesit-1'in-en-kritik-geçerlilik-bulgusu)

Nominal (koddan): W=400, K=8 → 50-tam-blok; blok-tamam-emisyon-şartlı `nominal = 50×r` (r2→100, r4→200, r8→400).
`enqueued` her-noktada-nominal'e-eşit-çıktı (blocks=50; **arm-constructor'lar-doğru-r-taşıyor** — tek-istisna
bw-12: blocks=45, 5-blok-horizon'da-hiç-tamamlanmadı). Ayrışma **emitted**-katmanında (token-artığı-yolu):

| nokta | r2 emit/nom | r4 | r8 | sınıf (r-crossover-için) |
|---|---|---|---|---|
| loss-20% | 98/100 | **98**/200 | **98**/400 | r4/r8 PARTIALLY-COLLAPSED (ortak-tavan≈98) |
| loss-30% | 43 | 87 | 169 | PARTIALLY-COLLAPSED (oran-korunmuş, ~0.43×) |
| **rtt-d1000** | **20** | **20** | **20** | **FULLY-COLLAPSED** (r-bağımsız-tavan) |
| rtt-d2500 | 93 | 93 | 93 | r2-OK / r4-r8-COLLAPSED (ortak-tavan) |
| rtt-d250 | 100 | 200 | 219 | r8-PART |
| rev-99% | 87 | 97 | 97 | r4/r8-COLLAPSED |
| rev-90% | 100 | 198 | 205 | r8-PART |
| **gap80** | **2 (seed-medyan; bimodal)** | 197 | 219 | r2-ANOMALOUS→çözüldü-§4 |
| bw-12pps | 90 (blocks=45) | 180 | 360 | CENSORED (+5-blok-hiç-açılmadı) |
| bw-25pps | 99 | 197 | 218 | r8-PART |
| nominal-noktalar (loss≤10, d50, iid/ge, gap27-53) | ~100 | ~197-200 | ~296-394 | VALID-SEPARATED |

**Hüküm:** d1000/d2500 hücreleri r-crossover-çıkarımı-için FULLY-COLLAPSED; "yüksek-RTT'de-vergi-daralması"
gözlemi **token/pacer-baskısı-açıklamasıyla (ihtimal-2) AÇIKLANIYOR** — emit-tavanı-r-bağımsız → "FEC-yüksek-
RTT'de-göreli-değerleniyor" (ihtimal-1) BU-VERİYLE-AYRIŞTIRILAMAZ → `loss×RTT×rev-crossover` hükmü kalibrasyon-
sonucu-DEĞİL, **arm-collapse-ayrıştırması-sonrası-araştırılacak-hipotez**.

## 4. GAP80-r2 ANOMALİSİ — KÖK-NEDEN (çözüldü)

Seed-bazlı: enq=100/blocks=50 HER-seed'de (constructor/profil-doğru); **emit: seed-11=2, seed-23=2, seed-37=100
(BİMODAL; medyan-2)**. FusionInput-telemetri kök-nedeni-gösterdi: seed-11/23 **BtlBw-p50=18000 B/s ≈ offered
(12.5pps×1237B≈15.5KB/s)** — BtlBw-delivered-ölçümü underutilized-linkte OFFERED'a-kilitleniyor (motor-C'yi-
hiç-görmüyor; ölçüm-doğru-motor-davranışı) → pacer-rate≈offered → DATA-token'ları-tam-tüketiyor → parity-artığı≈0.
Seed-37: erken-ACK-zamanlaması-farkı → BtlBw=26666 → token-headroom → parity=100. **Sınıf: harness-motor-
etkileşimi (BtlBw-offered-kilitlenmesi × token-artığı-parity); arm-mislabel/profil-override-DEĞİL.** Sonuç:
gap80-deadline-winner + headroom-etkisi + r-karşılaştırması bu-noktada KALİBRASYON-VERİSİ-DEĞİL. (Aynı-mekanizma
rtt-d1000-FULL-COLLAPSE'ı-açıklıyor: uzun-RTT→geç-ACK→düşük-BtlBw-dönemi-uzun.)

## 5. HORIZON-CENSORING

**Censored-hücreler:** bw-12pps TÜM-armlar 3/3-seed (comp=0.902, t95≈284.8s, T=300s) · loss-30 ARQ-baseline+
ARQ-hafif 3/3 (0.998) · tekil-seed: loss-10/ARQ-base, loss-20/ARQ-hafif, loss-30/ARQ-agresif, d2500/ARQ-hafif,
d250/ARQ-hafif+agresif. **t_p50/t_p95 yalnız-delivered-subset-quantile'ıdır** — censored-hücrelerde tamamlanmış-
popülasyon-latency'si-gibi-OKUNMAZ; crossover/eşik-çıkarımında-normal-hücrelerle-eş-ağırlıkta-KULLANILMAZ.
**bw-12-root-cause-notu:** 400-unit×(1-blok-tamamlanmaması dahil) — offered=gap-166ms≈6pps < C=12; blocks=45
(5-blok-horizon'da-tamamlanmadı); t95≈285s≈T → **channel+offered-horizon-sınırı** (tüm-armlar-özdeş-0.902 →
arm-bağımsız; pacer-underutilization-payı gap80-mekanizmasıyla-ortak — kesin-ayrıştırma kesit-2-finite-queue/
uzun-T-tasarımına).

## 6. PARETO (completion↑, dl5s↑, goodput↑; medyan) + objective-boşluğu

Non-dominated-sayısı/24-nokta: **ARQ-hafif-s100: 17** · ARQ-baseline: 6 · ARQ-agresif: 5 · **FEC-r2: 2**
(bw-25, gap27) · **FEC-r8: 1** (gap80 — §4-anomalili-nokta; geçersiz-sayılmalı). Nokta-listesi audit-script-
çıktısında. **"Winner"-dili objective-tanımlanmadan-KULLANILMAZ.** Olası-objective-çerçeveleri (bağlayıcı-seçim-
YAPILMADI): lexicographic (completion-floor → deadline-hit → goodput) · weighted-utility · constraint-based ·
Pareto-front + mode-specific-selection. **Controller-hangi-objective-sırasını-optimize-edecek → AÇIK-SORU**
(kalibrasyondan-önce-politika-fazında-karara-bağlanmalı).

## 7. Deadline / seed-belirsizliği

dl5s-farkları (örn. loss-10: FEC-r2 0.307/0.307/0.307 vs ARQ-base 0.305/0.307/0.302) **seed-içi-varyans-
bandında** (W=400'de 0.005≈2-unit). Hüküm: "deadline-objective'inin arm-sıralamasını-değiştirebileceğine-dair
KÜÇÜK-ÖN-SİNYAL; seed-level-tekrar + belirsizlik-analizi-gerekir." Seed-level-veri-artık-korunuyor (CSV).
**Kesit-2-istatistik-şartları:** paired-seed-seti-tüm-armlarda · ≥10-paired-seed (küçük-farkta→30) · medyan+
dağılım/bootstrap-CI · practical-equivalence-band DENEY-ÖNCESİ (yöntem: metrik-çözünürlüğü=1/W, seed-std ve
W'den türetilen eşik — bu-turda-keyfî-DONDURULMADI).

## 8. Scenario-knob ≠ FusionInput (kısmi-telemetri + plan)

Eklenen-KISMİ-telemetri (mevcut-public-diag'lar; 500ms-örnekleme): btlbw min/p50/p90/max · qd_ratio-permille ·
no_rule-sayacı. Örnek (environment≠input-kanıtı): rev-50 → btlbw-p50=62400; rev-90 → 60000; **rev-99 →
btlbw-p50=10000** (knob-yalnız-reverse'i-değiştirdi; controller'ın-GÖRDÜĞÜ-bandwidth-da-çöktü — eksen-karışımı).
gap80: §4-bimodal-btlbw. **EKSİK (production-diag-yok; 1e-impl-turunda additive-diag-planı):** loss_windowed_ppm ·
jitter_ppm · ack_confidence_permille · time-in-LOW/MED/HIGH · aktif-kural-ağırlıkları. Bu-eksikler-kapanmadan
"üyelik-sınır-adayı"-cümleleri **"environment-space screening candidate; FusionInput-space mapping pending"**
statüsündedir (rapor-düzeltmesi-uygulandı).

## 9. Pattern-ekseni (realized-loss)

Realized-mean-loss (ARQ-baseline; 3-seed): iid5 = %3.85/%2.20/%6.76 · ge-hafif = %0.50/%1.96/%1.96 ·
ge-ağır = %3.61/%1.48/%6.05; burst-mean 1.0-2.0, max≤4. **Profiller MATCHED-LOSS-DEĞİL** (ve kısa-koşuda-
seed-oynak) → "pattern-etkisi-vs-loss-rate-etkisi AYRIŞTIRILAMAZ"; düzeltilmiş-hüküm: "eşleşmemiş-pattern-
profillerinde-sıralama-değişmedi." Kesit-2: matched-realized-loss-çiftleri + daha-uzun-koşu; burst×K-ondan-sonra.

## 10. Headroom-ekseni (utilization-tablosu)

| gap | offered | utilization | headroom |
|---|---|---|---|
| 80ms | 12.5pps | 0.25 | 0.75 |
| 53ms | 18.9pps | 0.38 | 0.62 |
| 40ms | 25.0pps | 0.50 | 0.50 |
| 27ms | 37.0pps | 0.74 | 0.26 |

Tüm-süpürme `offered < C` bölgesinde → "headroom-sinyali-zayıf" hükmü **yalnız-pozitif-headroom (util≤0.74)
bölgesi-için**. Overload (util 0.95-1.25) + finite-queue/drop-policy + capacity-step = kesit-2.

## 11. Bu-audit'in-harness-ekleri (hepsi test-only)
FwdChan realized-loss/burst-sayaçları · Metrics{app_sent, data_units_emitted, blocks_enqueue_completed,
parity_enqueued, censored, fwd-stats, fi_btlbw/fi_qd/fi_no_rule} · `kesit1_cells()`-refactor ·
`audit_seed_level` (#[ignore]) seed-level-döküm. Production-kod-dokunulmadı.
