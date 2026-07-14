# srtt-decoupling-v1 — W_BTLBW srtt-Ayrımı [r2 — SAAT-HARİTASI (Y3-FAZLI); ONAY BEKLİYOR]

## ★ r2 — SAAT-HARİTASI (Y3-FAZLI; Abkarada-onaylı yaklaşım) [ONAY BEKLİYOR]

r1-NO-GO'nun cevabı: **tek-global-W-cevabı YOK; tüketici-başına re-clock.** Teşhis (wf_357cfcae, 3-lens)
altı-tüketiciyi ÜÇ ROL-SINIFINA ayırdı — her sınıf FARKLI doğru-saat (DV-1 qd→min/tepkisel→EWMA/
ölçek→kararlı ayrıştırmasının bir-katman-derin hali).

### ÜÇ PREMİS-DÜZELTMESİ (teşhis; r1'i sertleştirir)
- **P-1:** T3-Little `rate.rs:176` ÇIPLAK-srtt (ack_epoch-DEĞİL; sender.rs:1156) — flood'da özdeş ama
  sub-kbps'te EAI-koruma-tabanı YOK.
- **P-2 [circular-çekirdek]:** EAI srtt-BAĞIMSIZ-DEĞİL — `rate=min(pacing,cwnd/srtt)` üzerinden srtt-
  LAUNDER eder (srtt↑→rate↓→EAI↑). → srtt→rtprop-swap TEK-BAŞINA-ayrıştırmaz (max() şişkin-EAI-seçer).
  **Estimator'da TEK srtt-temiz-saat = rtprop** (kuyruk-değişmez sert-taban; DESEN-1'de 76.6ms SABİT).
  Return-prior term2 = 65e9/31 = 2.097s SABİT (decade-kör [CAL], §3.3-E-FULL'e-borç).
- **P-3:** qd_window (7. tüketici, ack_epoch×K_QD) — MIN-filtre-koruyucu, bug-değil (tamlık-notu).

### SAAT-HARİTASI (altı re-clock; rol-sınıfı ZM'i belirler)
| # | tüketici (dosya) | rol-sınıfı | doğru-saat + ZM-1 | circular | FAZ |
|---|---|---|---|---|---|
| T1 | W_BTLBW sender.rs:535 | PENCERE (MAX-filtre; ANA-KÖK) | delivered-cadence: **son-instant-delivered-EAI** (circular-kırar, ZM-1✓ gürültü→Y2) / **Y2 son-N-ring** (ZM-vakum✓, SACK-batch-açık) — rtprop-YASAK (cadence-değil→sub-kbps-çöküş) | son-instant: DÜŞÜK | 1 |
| T2 | W_loss sender.rs:837-839 | PENCERE (loss-ledger) | T1-aynı-saat; FAIL-ama-ZARARSIZ (INV-RATE-1: loss-rate-etkilemez→ZM-1-etkisi-NULL) | NULL | 1 (tutarlılık; ertelenebilir) |
| T3 | Little cwnd/srtt rate.rs:176 | DENOMINATOR=fiziksel-RTT (delay-tepkisi; Motor-2/SUSTAINER) | **cwnd/(rtprop+qd)** = cwnd/min_W_qd (qd=queue_delay rtt.rs:212, zaten-decoupled). rtprop-swap-YASAK (cwnd/rtprop=2×btlbw hep→hiç-bağlamaz→delay-tepkisi-SİLİNİR). SINIRSIZ-qd (cap-ÖNERİLMEZ, koruyucu-sabote). ZM-1: min-filtre-koruyucu✓ | DÜŞÜK | 1 (Motor-2; transient-kaynağı) |
| T4 | Drain-dwell sender.rs:933 | DWELL (byte-eşiği-BİRİNCİL) | qd-ailesi-KORU (srtt≈rtprop+qd≈qd tesadüfen-doğru-cins); rtprop-rebase-YASAK (3×0.076=0.23s→ZM-2-premature-exit). byte-çıkışı-birincil zaten-saat-sınıfsız | YOK (pasif-cap) | 3 (en-düşük) |
| T5 | DrainRtprop sender.rs:700,953 | DWELL + GİRİŞ-EŞİĞİ | **giriş-eşiği W_RTPROP/2=srtt×4=212s ASIL-FELÇ** (ZM-4-ihlali; circular-YÜKSEK — flood-kendi-ProbeRTT-barını-kaldırır) → rtprop-tabanlı-rebase; dwell qd/rtprop-ailesi | YÜKSEK (giriş) | 2 |
| T6 | ProbeUp-period sender.rs:1004 | PERİYOT (bant-arama-aralığı) | faz-RTT-mertebesi → rtprop-tabanlı (period srtt-şişince-seyrekleşir→bant-arama-körü); ZM-1: rtprop-değeri-kuyruk-değişmez✓ | ORTA | 2 |

### KRİTİK: T1-TEK-BAŞINA YETMEZ — İKİ-MOTOR (Abkarada-Nokta-2 doğrulandı)
T1-fix btlbw'yi-expire-eder (Motor-1) ama **YAVAŞ (~50-68s; delivered-cadence-doğru-W ama yavaş-kanalda
örnek-seyrek → N-örnek-kapsamı-zorunlu-uzun)**. O 50-68s-boyunca btlbw-hâlâ-bayat → **T3-Little cwnd/srtt
BAĞLAR (Motor-2/SUSTAINER) → flood-sürer.** → **FAZ-1 = T1 + T3 BİRLİKTE** (iki-motoru-birden-kes;
T3-tek-başına-btlbw-doğrulunca-serbest AMA transient'te-Motor-2-aktif). T3 delivered-cadence-değil,
cwnd/(rtprop+qd) — delay-tepkisi-korunur (INV-RATE-1).

### TUR-D1-KADERİ — GÜNCEL (yavaş-expiry aktif-demotion'ı YENİDEN-CANLANDIRIR)
r1'de "arşiv" demiştim (premis: ~10s-self-stop). Teşhis-düzeltti: **decoupling-sonrası-expiry SONLU-ama-
YAVAŞ (~50-68s), hızlı-DEĞİL.** Hızlı-expiry yalnız (rtprop-çöküşü-ZM-2-yasak) ya da (aktif-demotion-
TUR-D1). → **TUR-D1-kaderi Y3-ÖLÇÜMÜNE-bağlı KALIR:** FAZ-1 (T1+T3) sim'de flood-kabul-süresinde-durursa
(temiz-1k/P5-GATE-korunuyorsa) → TUR-D1-gereksiz (arşiv). Yavaş-expiry-transient-bir-kapıyı-düşürürse →
TUR-D1-yeniden-inşa (P6→rtprop + reseed→en-taze; hızlı-demotion-katmanı). DO-NOT-MERGE-marker-doğru.

### FAZLAMA (KIRMIZI-kapılı; biri-düşerse-DUR)
- **FAZ-1: T1 (delivered-cadence-W) + T3 (cwnd/(rtprop+qd)) + T2 (T1-aynı-saat, ucuz)** — iki-motor;
  DESEN-1-doğrudan-kök. Sim: flood-durur-mu (~kabul-süre) + srtt-runaway-eğim-çıkar-mı.
- **FAZ-2: T5 (giriş-eşiği-rebase; ProbeRTT-açar→min-rtt-tazeleme) + T6 (ProbeUp-period rtprop)** —
  recovery-hızlandırma; DESEN-1-flood-YARATMAZ ama toparlanma-geciktirir.
- **FAZ-3: T4 (Drain-dwell qd-koru)** — en-düşük (byte-eşiği-birincil; şişme-semptom-değil-kök).
- Her-faz: sim-teşhis (kalibrasyon: fix-öncesi-motor 3A2-repro infl~63×/srtt~53s) → KIRMIZI-kapılar
  (P5-GATE {100×12, S5-kurallı} + temiz-1k=45 + DESEN-2-ramp-13×/1s + 0.25k-btlbw-anchor) → geçerse-sonraki.

### AÇIK SORULAR (onayla)
- **S1: T1-saat = son-instant-delivered-EAI mı Y2-ring mi?** Öneri: son-instant (küçük-kod: last_sample_bw()
  getter; circular-kırar) — gürültü-riski (median-mini-ring→Y2'ye-yakınsar). Y2 daha-saf-ZM ama BtlBwFilter-
  yeniden-yapısı+TUR-5-regresyon. Sim ikisini-de-ölçebilir.
- **S2: T3 FAZ-1'e mi FAZ-1b-ayrı-mı?** Öneri: FAZ-1-birlikte (iki-motor; T1-yavaş-expiry T3-transient-
  flood'la-birleşir — ayrı-ölçüm atıf-karıştırır). Ama tek-değişken-disiplini T1-önce-T3-sonra-der →
  KARAR: T1-önce-sim (yavaş-expiry-flood-ölç), T3-transient-görülürse-EKLE (dal-yapılı).
- **S3: T5-giriş-eşiği rebase bu-fazda-mı?** rtprop-kusursuz-değil-kalıntısı (adversaryal-delik) — giriş-
  eşiği-rebase o-deliği-de-etkiler. Öneri: FAZ-2 (T1-sonrası; ProbeRTT-açılması flood-hızlandırabilir-bonus).
- **S4: TUR-D1-DO-NOT-MERGE korunur** (Y3-ölçüm-sonrası-karar). ✓

---

## ★ r1 NO-GO — DÖRT-HIGH; KÖK D0'DA-SANDIĞIMDAN-DERİN (doğrulama wf_e61d46e9)

**Abkarada'nın Nokta-1 VE Nokta-2 endişeleri İKİSİ DE haklı çıktı.** Dört-W-adayı da problematik +
"cerrahi-dar W_BTLBW-decouple yeter" iddiası çürüdü.

### DÖRT-W-ADAYININ HEPSİ ÇÜRÜK:
- **(A) saf-rtprop:** sub-kbps 10s-taban çöküşü (ZM-2) — zaten RED.
- **(C) sabit-W:** decade-kör (ZM-2) — zaten RED.
- **(D-i sayım-tabanlı, BAŞ-öneriydi): ZM-1-TUZAĞI [HIGH-1 — Nokta-1].** "kuyruk-değişmez" YALNIZ-
  sayı-için; N-round'un WALL-TIME'ı değil. Round marker-paketi (sender.rs:1065) kuyruğun-SONUNA
  girer → ancak ayakta-kuyruk boşalınca teslim → round-latency ≈ queue_delay ≈ srtt (53s). N=8-round
  ≈ Σsrtt, kuyrukla-BÜYÜR (Startup-3ms-tuzağının kardeşi: sayı-değişmez ama sayının-haritalandığı-saat
  srtt-bağlı). Ölümsüzlüğü KIRAR (sonlu-N→evict) ama self-stop ~10s DEĞİL, **51s+ kuyruk-bağımlı.**
- **(D-ii ack_epoch-swap): MTU-BİRİMDE ÇÖKER [HIGH-3].** dW/dt=0.82 unit=267-ARTİFAKTI; unit=1200
  (MTU) → dW/dt=3.68>1 → ölümsüzlük GERİ. Eşik unit≈326B. Ayrıca EAI=max()=2.097s>rtprop hep-baskın
  → D-ii srtt'yi EAI(=2×unit/[cwnd/srtt]) üzerinden LAUNDER-ediyor, ayrışmıyor.

### META-HÜKÜM: W_BTLBW-CERRAHİ-DECOUPLE TEK-BAŞINA YETMEZ.
srtt-şişkinliği ≥6-tüketiciye yayılıyor (MED-bulgular, 9-tüketici-haritam EKSİKTİ):
W_BTLBW · W_loss (#2-ikinci-kol) · Little cwnd/srtt (Motor-2) · **Drain-dwell (sender.rs:933) ·
DrainRtprop-dwell (953) · ProbeUp-period (1004)** — hepsi ack_epoch=max(srtt,EAI). Fix-sonrası-recovery
anlatım (DrainRtprop-boşaltma, Little-self-resolve) TAM-BU-srtt-fazlarına-dayanıyordu → decoupling
W_BTLBW'yi-çözse-bile srtt-transientte-şişik → bu-zamanlayıcılar-hâlâ-bozuk.
**Asıl-kök: srtt-EWMA'nın-KENDİSİ** (kuyruk-şişince-şişer — DOĞRU-ölçüm ama pencere/dwell-boyutlandırmada
YANLIŞ-saat). = BBR'nin srtt-smoothed'ı-pacing'de-merkezi-tutmama gerekçesi (M3-decoupling'in-tam-konusu).

### TUR-D1-ARŞİV KARARI ASKIDA [HIGH-2/HIGH-4]:
Arşiv-premisi "~10s-self-stop" YALNIZ-reddedilen-(A)'ya-aitti; gerçek D-i/D-ii self-stop 51-64s+ →
**tam-o-yavaş-rejim aktif-demotion (shrink) katmanının işe-yaradığı yer.** Arşiv yanlış-zemine-kuruluydu.
+ Arşiv-formu CLAUDE.md §6-ihlali (shrink-kodu-canlı, feature-flag-yok; branch-merge-riski PR#31-emsali).

### YÖN-UZAYI (karar-Abkarada; her-biri YENİ-teşhis-turu-ister — kod-yok):
- **(Y1) delivered-rate-tabanlı-EAI:** ack_epoch'taki EAI'yi PACING-rate(cwnd/srtt, kirli) yerine
  DELIVERED-rate(btlbw, kuyruk-değişmez INV-RATE-1) ile-hesapla. Örnek-kadansı=gerçekten-delivered-
  tabanlı. AÇIK-RİSK: flood'da btlbw-bayat → circular (çözülmek-istenen-değişken EAI-hesabında). Son-
  örnek-instant-delivered kurtarır-mı? — teşhis-ister.
- **(Y2) per-sample-D-i (round-DEĞİL):** son-N-RATESAMPLE-ring (marker-round-değil) → her-teslim-örneği
  sayılır; flood'da delivered-akıyor → N-örnek delivered-kadanslı (srtt-değil?). HIGH-1 round'u-çürüttü;
  per-sample-varyant ZM-1'den-kaçar-mı — teşhis-ister (SACK-batch/interval-mekaniği).
- **(Y3) srtt-EWMA-kök / M3-tam-refactor:** srtt'yi pencere+dwell+Little'dan-topyekûn-çıkar, rtprop-
  merkezli (BBR-emsali). En-büyük, en-kök. m45'te "M3 bağımsız-sertleştirme-sonra" demiştik → meğer
  DESEN-1-için-ÖN-KOŞUL (bir-katman-daha).
- **(Y4) çoklu-cerrahi-decouple:** 6-tüketicinin-hepsini ayrı-ayrı rtprop/delivered-tabanlı-yap.
  Dar-ama-çok-nokta; her-biri-kendi-ZM-analizi.


> **Statü:** D0-TASARIM-NOTU (Abkarada: EK-3 = DESEN-1'in-KÖKÜ, ertelenebilir-borç-değil; seçenek-b
> EK-3-önce onaylı). ONAYDAN SONRA kod. Bu **srtt-KÖK-ailesi** (DV-1 srtt-türev-ayrıştırmasının
> kardeşi) — 9-tüketici-haritası güncel-koda-karşı yeniden-çıkarıldı. Zemin: 3-lens kod-teşhis
> (wf_71d37450) + DV-3 §0.30 + TUR-D1-ölçüm-dekompozisyonu. **KOD YOK — dört-soru + TUR-D1-kaderi.**

## 0. KÖK-HÜKÜM — MAX/MIN asimetrisi (tek-cümle-kök)

**Bir MAX-filtre penceresi, kuyruk-büyüdükçe-büyüyen bir saatle (srtt) ölçeklenirse, flood'un-kendisi
(=büyüyen-kuyruk) pencereyi-büyütür → pozitif-geri-besleme → bayat-yüksek-örnek ÖLÜMSÜZ.**
- W_BTLBW = K_BW×ack_epoch, ack_epoch = max(srtt, EAI) (sender.rs:530,535; K_BW=8). Ölümsüzlük-eşiği:
  srtt-eğim > 1/K_BW = 0.125 s/s; **DESEN-1 ölçülü 0.92 s/s = 7.4×** → dW/dt=7.36≫1 → cutoff-GERİLER
  → 4119 asla-expire (kanal-375'in 11×'i). srtt-şişkinliği ÜÇ-pencereyi şişirir: W_BTLBW, W_RTPROP, W_loss.
- **rtprop TESADÜFEN-TEMİZ (76.6ms sabit, srtt 53s'e şişerken):** aynı-şişen-pencere MIN-filtre için
  KORUYUCU (eski-DÜŞÜK-örnek = gerçek-rtprop; kuyruk-fiziği sert-taban, gürültü tek-taraflı-yukarı),
  MAX-filtre için ZEHİR. rtprop **kaçak-tırmanışa GİREMEZ** (kuyruk-değişmez) — sabit-bias'la yanlış
  olabilir, runaway-olamaz. → W_BTLBW'nin doğru-saati rtprop-ailesi, srtt-değil.

## 1. SORU-A: W_BTLBW'yi srtt'den nasıl-ayır (ZM-kurallı; pencere-saatinin İKİ şartı)

Pencere-saati İKİ şartı sağlamalı; üç-basit-aday üçü de birini-kaçırır:
- **ŞART-1 (örnek-kapsamı):** ~N-örnek kapsamalı → örnek-VARIŞ-kadansı gerek (ack_epoch'ta EAI bunun-için).
- **ŞART-2 (kuyruk-değişmezliği):** kuyruk-büyüdükçe BÜYÜMEMELİ → srtt YASAK; rate=cwnd/srtt-türevi-EAI de YASAK.

| aday | DESEN-1 flood | sub-kbps (ZM-2) | hüküm |
|---|---|---|---|
| (A) W=K_BW×rtprop | ~10s'de durur (rtprop-terim taban-altı → 10s-tabana-DEJENERE) | EAI=19.3s>10s → tek-örnek-max → **btlbw 88→4 ÇÖKÜŞ** | RED (ŞART-1-kaçırır) |
| (B) W=K_BW×max(rtprop,EAI) | dW/dt=8×0.111×0.92=**0.82<1** sınırda-durur (~64s) | EAI-baskın → 8-örnek ✓ | KİRLİ+KIRILGAN: EAI∝rate=cwnd/srtt gizli-srtt-bağı; 0.82-marjı min_cwnd/unit'e bağlı, tasarım-garantili-değil → dik-srtt-eğimi marjı 1-üstüne itip ölümsüzlüğü-geri-getirir |
| (C) sabit-W (10s) | ~10s durur | decade-kör → çöküş | RED (ZM-2-ihlali; DV-1-yanlışladı) |

**CERRAHİ-KÖK:** ack_epoch'taki srtt-terimi. ŞART-1-görevini (RTT-mertebesi örnek-kapsamı) srtt-DEĞİL
**rtprop** (kuyruk-değişmez) görebilir. İki mekanizma-sınıfı (ikisi de yeni-mutlak-sabit-EKLEMEZ):

- **(D-i) SAYIM-TABANLI pencere [BAŞ-ÖNERİ]:** max-filtreyi wall-time yerine **son-N-TESLİM-örneği**
  üzerinde tut (delivered-anchor round; TUR-6 ZM-1-deseni — ACK-olay-sayımı DEĞİL, arz-yoğunluğu-
  bağımsız TESLİM-round, TUR-5-regresyon-dersi). BBR'ın "~10 RTT"sinin gerçek-okuması = round-SAYISI,
  wall-clock-değil. **Hem kuyruk-değişmez (srtt/rtprop/rate'ten bağımsız N-tut) HEM decade-değişmez
  (N-örnek her-decade'de aynı).** İki-şartı-birden-sağlar; ZM'den TAMAMEN-kaçar (sayı, saat-değil).
  Risk: BtlBwFilter yeniden-yapısı (zaman-retain → son-N-örnek-ring); TUR-5-regresyonu (teslim-sayımı
  ≠ ACK-olay-sayımı — arz-yoğunluğu-bağımsızlık şartı).
- **(D-ii) ack_epoch srtt→rtprop swap [DAR-ALTERNATİF]:** ack_epoch = max(rtprop, EAI) AMA (B)'nin
  EAI-kirliliğini kabul-eder (0.82-marjı). Daha-küçük-kod-değişimi, daha-kirli-hüküm. (D-i başarısızsa yedek.)

**rtprop-kusursuz-DEĞİL (beyanlı-kalıntı):** adversaryal-delik — kanal İLK-örnekten-tıkanık (temiz-
baseline-yok) ∧ ProbeRTT-felç (DrainRtprop-girişi W_RTPROP/2=8×srtt/2 → srtt-şişikken tetiklenmez) →
rtprop over-estimate (sert-tabanla-sınırlı, runaway-değil). **Fix rtprop'u kusursuz-VARSAYMAMALI:**
DrainRtprop/ProbeRTT giriş-kapısını da srtt-tabanlı→rtprop-tabanlı rebase et (W_RTPROP-şişmesi zararsız
ama giriş-eşiği felç-ediyor — §2-#3).

## 2. SORU-B: dokuz-srtt-tüketici haritası (güncel-koda-karşı; W_BTLBW-ayrımının etkisi)

| # | tüketici | srtt-bağı (dosya) | W_BTLBW-ayrım-etkisi |
|---|---|---|---|
| 1 | loss-eşiği 9/8·srtt | loss.rs:35-40; sender.rs:712 | ETKİLENMEZ (ayrı srtt-yolu); cadence-taban korur |
| **2** | **ack_epoch → W_BTLBW + W_loss** | sender.rs:530,535 (BW) + 837,839 (W_loss, K_WL=8) | **HEDEF. DİKKAT: cerrahi-dar-yapılırsa W_loss-kolu srtt-şişkin-KALIR** (§4-S2) |
| 3 | W_RTPROP = K×srtt | sender.rs:536; rtt.rs:114 | ETKİLENMEZ; şişme MIN-filtrede ZARARSIZ AMA DrainRtprop-giriş-eşiğini felç- eder → §1-rebase |
| 4 | tombstone-TTL | sender.rs:776-778 | ETKİLENMEZ; şişme zararsız (uzun-TTL = geç-ACK-fırsatı) |
| 5 | jitter = rttvar/srtt | sender.rs:840-843 | ETKİLENMEZ (ZM-4 tek-saat) |
| **6** | **Little cwnd/srtt** | rate.rs:147,176; sender.rs:1156 | **ETKİLENMEZ ama SORU-D'nin konusu (aşağıda — sorun-değil)** |
| 7 | liveness-dup aralığı srtt | sender.rs:1267,1279 | ETKİLENMEZ; şişme zararsız (probe-seyrekleşir) |
| 8-9 | EAI / conf-eşiği (srtt-türevli) | sender.rs:754,1394 | ETKİLENMEZ doğrudan; EAI (D-ii'de dolaylı) |

**Hüküm:** W_BTLBW-ayrımı cerrahi-DAR (yalnız #2-BW-kolu). Diğer-8-tüketici KIRILMAZ (srtt-şişmesi
onlarda ya-etkisiz ya-zararsız). TEK-yan-görev: #2'nin **W_loss-kolu** (aynı ack_epoch) — S2-kararı.

## 3. SORU-C: TUR-D1 KADERİ — **ARŞİV (kader-i)** [en-güçlü, kod-zeminli]

Decoupling-sonrası (D-i/D-ii) W küçülür → cutoff-artık-gerilemez → **ölümsüzlük-kırılır → bayat-4119
expire (flood boyunca taze-düşük-örnekler push → keep-last onu-diriltmez) → flood ~10s'de SELF-STOP.**
- **(1) flood kendiliğinden-durur:** EVET (ölümsüzlük-geri-beslemesi kırıldı).
- **(2) TUR-D1-shrink hâlâ-gerekli-mi: HAYIR.** shrink'in TÜM-var-oluş-sebebi "bayat-max-YAŞLANMAZ"
  (m45 §3) — bu koşul YALNIZCA srtt-kaplı-W-ölümsüzlüğünden doğuyordu; decoupling onu çözünce
  shrink'in niş'i BUHARLAŞIR. Keskin-düşüşte expire HEM-HIZLI (tek-10s vs shrink 11×-düşüş için
  log₂(11)≈3.5-ateşleme×K_SHRINK-LOW×P6-gap) HEM-İSABETLİ (gerçek-pencere-örnekleri vs geometrik-
  bayat-yarım-tohum 1675=4.5×kanal). Ölçüm-doğruladı (reset→1675, flood-KESİLMEDİ). Üstelik
  shrink-reset (samples.clear+tek-tohum) expire'ın-kullanacağı kayan-düşük-kanıtı ATAR → decoupled-
  zeminde shrink plain-expire'dan DAHA-KÖTÜ.
- **(3) TUR-D1 kendi-2-sorunundan (P6+reseed) kurtulur-mu: YALNIZ-KISMİ** → mevcut-form as-is
  çalışmaz (P6 hâlâ epoch_ref=max(srtt,EAI); reseed-bayatlık srtt-DIŞI-kökenli — sampler-gecikmesi +
  K_RATIO-geometrik-tasarım, temiz-srtt-çözmez).

**HÜKÜM:** TUR-D1-shrink **ARŞİV** (CLAUDE.md §6: sessizce-silme-YASAK → appendix + "MUST NOT
IMPLEMENT" / feature-flag-kapalı; kod branch'te-zaten-PR'sız). **YEDEK (ölçüm-kapılı):** eğer
~10s-expire-floor-transienti bir-kapıyı-düşürürse (temiz-1k=45 / kuyruk-spike / P5-GATE) → aktif-
hızlı-demotion-katmanı gerekir AMA mevcut-TUR-D1-DEĞİL (P6→rtprop + reseed→en-taze-örnek rebase-
edilmiş YENİDEN-İNŞA). **REDDEDİLEN:** kader-ii-saf (mevcut-TUR-D1-temiz-zeminde-yeniden).

## 4. SORU-D: Little-freni srtt-bağı aynı-kökte-mi — **SORUN-DEĞİL (Motor-1'e-tabi)**

DESEN-1 iki-motorlu: Motor-1 (TRIGGER, bayat-btlbw-ölümsüzlük) + Motor-2 (SUSTAINER, Little cwnd/srtt).
**Kritik:** btlbw-DOĞRUYKEN cwnd/srtt = 4800/0.076 = **63000 B/s ≫ 375** → Little **HİÇ-BAĞLAMAZ.**
Little yalnız btlbw-BAYATKEN (=flood-zaten-varken, srtt-şişik) 4800/srtt-bağlar (sustainer). Yani
Little-freni Motor-1'e-TABİ: Motor-1 (W-decoupling) çözülünce btlbw-doğru → Little serbest → srtt-bağı
kendiliğinden-çözülür. **TEK-FIX (W_BTLBW-decoupling) YETER; Little ayrı-fix GEREKMEZ.** (cwnd/srtt'yi
rtprop-tabanlı-yapmak INV-RATE-1/delay-tepkisi-kaybı-riski taşırdı — gereksiz, yapılmaz.)

## 5. Açık sorular (onayla)

- **S1: (D-i sayım-tabanlı) mi (D-ii ack_epoch-swap) mi?** Öneri: D-i (ZM'den-tam-kaçar, iki-şart-
  birden) — AMA BtlBwFilter-yeniden-yapısı + TUR-5-regresyon-riski (teslim≠ACK-olay-sayımı). D-ii
  yedek (küçük-kod, 0.82-marjı-kirli). Karar-Abkarada; sim-kalibrasyon ikisini-de-ölçebilir.
- **S2: W_loss-kolu (#2 ikinci-kol) da decouple-mu?** W_loss = loss-ledger-penceresi (recovery). Şişmesi
  ZARARSIZ-mı (uzun-pencere=daha-çok-loss-hatırlar; INV-RATE-1: loss-rate-etkilemez) yoksa yan-etki-mi?
  Öneri: aynı-saat-sınıfına-taşı (tutarlılık) — ama zararsızsa-ertelenebilir; sim-ölçer.
- **S3: DrainRtprop/ProbeRTT giriş-eşiği rebase (rtprop-kusursuz-değil kalıntısı)** bu-turda-mı? Öneri:
  EVET (aynı srtt-kök; giriş-eşiği W_RTPROP/2=srtt-tabanlı → felç). Küçük-ek, tek-turda-tutarlı.
- **S4: TUR-D1 arşiv-formu** — appendix+MUST-NOT mi, feature-flag-kapalı-kod-mu? Öneri: kod branch'te-
  kalır (PR'sız zaten), notta appendix-kayıt + "srtt-temiz-zeminde-YEDEK-yeniden-inşa-adayı".

## 6. Ölçüm-planı (kod-GO sonrası; SİM-KALİBRASYON-ŞARTLI — kalıcı-kural)

- **Sim-harness (D0-onay-sonrası inşa):** Sender + basamaklı-rate-kanal + inf-buffer + btlbw/srtt/infl-
  izleme. **KALİBRASYON: fix-ÖNCESİ-motorla 3A2-sim netem-flood'unu REPRO-etmeli** (infl~63×, srtt~53s;
  etmezse harness-güvenilmez, düzelt) → SONRA fix aynı-sim'de: **flood ~10s-self-stop-mu, srtt-runaway-
  eğimi-çıktı-mı, btlbw→kanal-mı.** Teşhis-GO/NO-GO, dakikalar.
- **Sim-teşhis-hücreleri:** 3A2 (ana), osc (salınım), yavaş-daralma (expire-yeter-mi), 0.25k (sub-kbps
  ŞART-1: btlbw-çökmedi-mi — D-i/D-ii ayrımının kritik-testi).
- **netem PR-doğrulama (yalnız bir-kez, teşhis-temizse):** P5-GATE {100×12} + temiz-1k=45 + DV-3A2/3B2 +
  0.25k + ileri-kayıp p1/p2/p3 (regresyon-yok-mu). Sim'de-yeşil ≠ PR-hazır.
- KIRMIZI: temiz-1k<45 / P5-GATE-bozulma / sub-kbps-btlbw-çöküş (ŞART-1-regresyonu) / DESEN-1-flood-sürmesi.
