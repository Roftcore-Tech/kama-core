# srtt-decoupling-v1 — W_BTLBW'yi srtt-Runaway'den Ayır (DESEN-1 kök-fix) + TUR-D1 Kaderi [TASLAK r1 — ONAY BEKLİYOR]

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
