# TARAMA-KESİT-1 — karar-yüzeyi ilk-kesiti

> ## RESMÎ-STATÜ (Abkarada-denetimi, 2026-07-22):
> # **EXPLORATORY DECISION-SURFACE SCREENING — NOT YET CALIBRATION EVIDENCE**
>
> **CORRIGENDUM (orijinal-gövde-aşağıda-korunur; tam-denetim: [validity-audit](tarama-kesit-1-validity-audit.md)):**
> Bu-kesitten fuzzy-üyelik-eşiği veya ürün-politikası DOĞRUDAN-türetilemez: 3-seed-medyan · Scenario-knob ≠
> gerçek-FusionInputs · birçok-FEC-arm'da requested-r ↔ realized-parity AYRIŞIYOR (arm-collapse-tablosu-audit'te;
> d1000/d2500 r-crossover-için FULLY-COLLAPSED) · bazı-hücreler horizon-censored (bw-12pps 3/3; loss-30 ARQ-armları) ·
> deadline-farkları çoğunlukla metrik-çözünürlüğü-seviyesinde · ARQ-yoğunluk = SACK-cadence-PROXY · tek-eksen
> bileşik-fuzzy-kurallarını-doğrulamaz. **Terminoloji-düzeltmesi:** "7-arm/144-hücre-önde" → **6-distinct-executed-
> arms / 7-logical-labels (ARQ-orta≡ARQ-baseline; istatistikte-bir-kez-sayılır) / 24-scenario-points / 144-run-rows**;
> goodput-hükmü: **"24/24 scenario-noktasında en-yüksek goodput_per_byte bir-ARQ-arm'ında gözlendi"** (dominance-
> iddiası-değil; loss-30'da completion: FEC-r2/r4/r8=1.000 > ARQ-baseline=0.998 — goodput≠completion-objective).
> **§16-dil-düzeltmeleri:** "R1-doğrulandı"→"R1-yönelimiyle-TUTARLI (fuzzy-arm-koşulmadı; realized-parity-değişken —
> kural-doğrulaması-değil)" · "deadline-karar-değeri-gösterildi"→"küçük-tekrarlanması-gereken-ÖN-SİNYAL (seed-varyansı-
> bandında)" · "crossover-adayı loss×RTT×rev"→"arm-collapse-ayrıştırıldıktan-SONRA-araştırılacak etkileşim-HİPOTEZİ
> (daralmanın-token/pacer-baskısı-açıklaması audit'te-DOĞRULANDI — d1000-emit=20/20/20 r-bağımsız)" · "pattern-önceliği-
> düşük"→"eşleşmemiş-pattern-profillerinde-sıralama-değişmedi (realized-loss MATCHED-DEĞİL: iid5≈%2.2-6.8, ge-hafif≈
> %0.5-2 — pattern-vs-rate-ayrıştırılamaz)" · "ackconf-için-rev90-99-ayrıştırıcı"→"environment-space-screening;
> actual-ackconf-mapping-PENDING" · "headroom-sinyali-zayıf"→"yalnız-test-edilen-POZİTİF-headroom-bölgesinde (util
> 0.25-0.74) zayıf" · "FEC-düşük/orta/yüksek"→"requested-r2/r4/r8 (realized-parity-ayrıca)".

# [ORİJİNAL] TARAMA-KESİT-1 — karar-yüzeyi ilk-crossover-kesiti (tek-eksen-süpürmeleri)

> **Amaç-çerçevesi:** fuzzy-kalibrasyon/tasarım-verisi — ürün/white-paper-üstünlük-iddiası ÜRETİLMEZ.
> Harness-only (`m1_reverse_nis.rs::sweep_kesit_1`); mühürlü-koda-dokunulmadı. Ham-veri:
> [data/tarama-kesit-1.csv](data/tarama-kesit-1.csv) (144-satır). **Hücre=144 ≤ 200** (NETLEŞTİRME-1 ✓;
> etkileşim-eksenleri ikinci-kesit). Deadline-metriği (dl_hits 2s/5s/10s post-hoc-hit-rate) eksenden-ÖNCE
> harness'a-eklendi (NETLEŞTİRME-2 ✓ — deadline-satırı ayrı-koşu-gerektirmedi: teslim-zamanı-dağılımından).

## 1. Tasarım

**7-arm (6-koşu):** ARQ-baseline(sack50) · FEC-düşük-r2 / orta-r4 / yüksek-r8 (K=8-sabit; sack50) ·
ARQ-hafif(sack100) / **orta≡baseline** / agresif(sack25). **Beyan:** "ARQ-yoğunluk"-düğmesi = SACK-kadansı-
PROXY'si (feedback-frekansı; motor-içi-agresiflik mühürlü/dejenere — envanter). **Nominal-sabitler:** IID-5%,
rev=0, d=50ms (RTT≈150ms-yük-dahil), C=50pps, gap=40ms (offered≈C/2), W=400×1200B, T=300s, 3-seed-medyan.

**Eksenler (tek-eksen; diğerleri-nominal):** loss{1,3,5,10,20,30}% · pattern{IID5, GE-hafif(0.1/20%;5s/500ms),
GE-ağır(0.1/40%;2s/200ms)} · bw C{12,25,50,100}pps (headroom-sabit gap=2×ser) · headroom gap{80,53,40,27}ms ·
RTT d{50,250,1000,2500}ms · reverse{50,90,99}%. **E-ekseni:** bu-kesitte ANALİTİK (M3: crossover=bandwidth×
deadline; sim-shard-düzeyi-refactor=ikinci-kesit). evicted=0 144/144 (KARAR-2 ✓).

## 2. Sonuç-deseni (dürüst-özet; kesişme-tablosu CSV'de)

1. **goodput/sealed-byte:** ARQ-arm'ları 144-hücrenin-TÜMÜNDE-önde — sabit-FEC sabit-parity-vergisi-ödüyor,
   hiçbir-tek-eksen-noktasında-geçemiyor. **Bu-uzayda goodput-crossover'ı YOK.** (Çerçeve-uyumu: bu "ARQ>FEC"
   hükmü-değil; "sabit-ratio-adaptif-olmayan-FEC'in-vergi-karakteri" — controller-zaten-bu-rejimlerde-FEC'e-
   yönelmemeli → R1-kuralının-meyil-yönü-DOĞRULANIYOR.)
2. **Vergi-DARALMA-bölgeleri (crossover'a-yaklaşma-yönü; kalibrasyon-sinyali):** FEC-orta-vs-ARQ-baseline
   goodput-farkı nominalde ≈−0.31 iken: **RTT-d1000ms → −0.056** · loss-30% → −0.117 · rev-99% → −0.136.
   Yön hipotezle-uyumlu (yüksek-RTT/loss/rev-çöküşü FEC-vergisini-görelileştiriyor) ama tek-eksende-kesişmiyor →
   **crossover-adayı = etkileşim-bölgesi (loss×RTT×rev)** — ikinci-kesitin-birincil-hedefi.
3. **Deadline-metriği-ayrıştırıyor (dl5s en-iyi-arm):** birkaç-noktada FEC-arm'ları-önde: loss-10%→FEC-r2 ·
   bw-25pps→FEC-r2 (+0.005) · headroom-gap80→FEC-r8 · gap27→FEC-r2 (+0.005). Farklar-KÜÇÜK (≤0.005) —
   ön-sinyal-seviyesi; ama goodput'tan-FARKLI-kazanan-vermesi deadline-girdisinin-karar-değerini-gösteriyor.
4. **Reverse-99% + ARQ-agresif(sack25) en-iyi** (gp+dl5s) — feedback-FREKANSI rev-çöküşünde-belirleyici;
   ackconf-sessizlik-proxy'sinin-yeterliliği-sorusu (envanter-kör-bölgesi) ikinci-kesit-blackout-epizotlarına.
5. **Pattern-ekseni (IID-vs-GE) arm-sıralamasını-DEĞİŞTİRMEDİ** (bu-metriklerde, bu-oran-sınıflarında) —
   patern-girdisi-önceliği-İLK-bakışta-düşük; burst-uzunluğu×blok-boyu-etkileşimi (K=8-sabitti) ikinci-kesitte.

## 3. Çıktı-sınıflandırması (çerçeve-madde-8)

**KALİBRASYON-verisi (mevcut-girdilerle-ifade-edilebilir):**
- loss-ekseni: R1 (loss.LOW→fec0) meyil-yönü-doğrulandı; loss-HIGH-tek-başına-FEC'i-haklı-çıkarmıyor (bw/jitter-
  koşullarıyla-birlikte — R2'nin-bileşik-yapısıyla-uyumlu). Üyelik-sınır-adayları: vergi-daralma-eğrisinin
  dikleştiği loss>20% bölgesi.
- bw-ekseni: mutlak-bw-taraması arm-sıralamasını-değiştirmedi (12-100pps) → bw-üyeliğinin-FEC-kararındaki-rolü
  tek-başına-zayıf (R2-bileşiği-anlamlı).
- ackconf-ekseni (rev-proxy): rev-99'da-feedback-frekans-hassasiyeti → ackconf-LOW-eşiği-kalibrasyonu-için
  rev{90-99}-bölgesi-ayrıştırıcı.

**TASARIM-verisi (mevcut-controller-göremiyor/üretemiyor):**
- **deadline:** dl5s goodput'tan-farklı-kazananlar-veriyor → deadline-girdisi/kuralı-adayı (M2-ekseni; girdi-YOK).
- **pattern:** bu-kesitte-ayrıştırmadı — "patern-girdisi-gerekli"-hipotezi-DESTEKLENMEDİ-henüz (ikinci-kesit:
  burst×blok-boyu).
- **E-size-seçimi:** analitik-crossover bandwidth×deadline (M3-düzeltmeli-hüküm); controller'a-bağlanmaz (tasarım).
- **direct-reverse-loss / headroom:** headroom-süpürmesi (gap-ekseni) sıralamayı-az-etkiledi; mutlak-bw'den-
  ayrışan-headroom-sinyali bu-kesitte-zayıf — ikinci-kesitte offered>C-aşırı-yük-bölgesi.

## 4. Sınırlar + ikinci-kesit-önerileri

Sim-idealize (kümülatif-SACK, sabit-C, IID/GE); adaptif-arm-YOK (fuzzy-FEC-aktüasyonu-kopuk — 1a-sonrası
senkron/adaptif-arm eklenebilir); ARQ-yoğunluk=SACK-proxy. **İkinci-kesit-önerisi (öncelik-sıralı):**
(1) etkileşim: loss×RTT ve rev×RTT (crossover-avı; daralma-yönleri-birleştirilerek) (2) blackout-EPİZOT-modeli
(sürekli-rev-yerine kesintili-karanlık — ackconf-kör-bölgesi) (3) burst×blok-boyu (K-varyasyonu) (4) E-sim
(shard-düzeyi) (5) 1a-sonrası: adaptif-ratio-arm + (1b/1d-sonrası) senkron-arm.
