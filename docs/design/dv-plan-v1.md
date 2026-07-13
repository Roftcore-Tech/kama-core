# DV-PLAN v1 — Gerçek-Dünya Doğrulama: üç eksen (Abkarada talimatı, 2026-07-13)

> **Statü:** ONAYLI PLAN (TUR-3c-sonrası talimat). Yürütme sırası: önce motor-temel (TUR-4
> dahil), sonra DV-1 → DV-2 → DV-3. Her DV-turu: beklenti-kaydı ÖNCE; tek-eksen; bulgu çıkarsa
> TEŞHİS (fix ayrı karar). Kampanya-öncesi kapanmalı.

## Kritik çerçeve (kayda, aynen)

Bu faz boyunca motorun yalnız BİR ekseni test edildi — **sürekli-bant**. Gerçek sivil/taktik
koşulların (LTE-kenar, GPRS, "çekmiyor"-bölgesi) asıl karakteri diğer iki eksende:
**aralıklılık** ve **rejim-geçişi**. Bunlar tasarımda niyet olarak var ama ÖLÇÜMDE YOK.
Motor "çalışıyor" demeden önce üçü de kanıtlanmalı.

**AÇIK UYARI (kullanıcının kendi tespiti, aynen kayda):** 0.25k-sürekli hedefi gerçek-sivil-koşulda
nadir; motor gerçek-dünya-uygunluğunu SÜREKLİ-bant ekseninde kanıtlıyor ama aralıklılık+geçiş
(baskın gerçek-desenler) henüz kör-nokta. Motor yanlış tasarlanmıyor — eksik test ediliyor, ve
eksik-olan gerçek-dünyada en-önemli-olan. DV-2/DV-3 bu kör-noktayı kapatana kadar
**"gerçek-dünyaya hazır" İDDİA EDİLMEZ** (sadece "sürekli-bant kanıtlı").

## Eksen-1: Sürekli-bant — büyük ölçüde kanıtlı; boşluk = hedef-bölge yoğunluğu

Durum: 0.25k + 1k yoğun test edildi; ProbeUp-sonrası btlbw-kaçış çalışıyor (p5r6 7.6×).
**Boşluk:** gerçek sivil taban 1-10 kbps (LTE/3G kenar, GPRS-gerçek) ama test-noktaları 0.25k/1k —
2-10 kbps hiç ölçülmedi. İronik: 0.25k-SÜREKLİ sivilde nadir (gerçek desen aralıklı) → gereğinden
zor noktada test edip gerçek-hedef bölgeyi atlıyoruz.

### DV-1 (4-decade + hedef-bölge)
Mevcut fix'leri **{0.05k, 0.1k, 1k, 2k, 5k, 10k, 100k, 900k}** noktalarında koş. İki soru:
- (a) [CAL] sabitleri (K_CONV/K_bw/K_c/K_TAIL/...) her decade'de geçerli mi, yoksa
  formül-doğru-ama-sabit-bir-decade'de-bozuluyor mu?
- (b) **Üst-uç regresyon:** srtt-fix + qd→min 500k/900k'da eski-davranışa dönüyor mu, regresyon mu?
  Özellikle qd→min: yüksek bantta min-RTT gürültülü, orada EWMA daha iyiydi — ısırıyor mu?
- Bulgu çıkarsa formülü GENELLEŞTİR, yeni sabit GİRME.

## Eksen-2: Aralıklılık — EN BÜYÜK BOŞLUK; gerçek sivil-tabanın asıl deseni; bu fazda HİÇ test yok

Gerçek "internet çekmiyor" = sürekli-0.05k DEĞİL — **çoğunlukla-kopuk + aralıklı birkaç-kbps
pencere**. Tasarım hazır OLMALI (PRE_CONVERGE + geometrik-kopya + forward-blast + connectionless
bootstrap tam bu desen için) AMA bu fazın TÜM testleri sürekli-kanaldı; p6-blackout/h4-jammer hiç
koşulmadı. Risk: sürekli-bant düzeltmeleri (srtt-fix, restore_burst, PRE_CONVERGE-çıkışı)
aralıklı-davranışı ETKİLEMİŞ olabilir — kopma-sonrası-yeniden-bağlanmada estimator ne yapıyor
bilinmiyor.

### DV-2 (aralıklı-rejim)
p6-blackout (2s-pencere/8s-kopuk) + h4-jammer-pulse + YENİ "çoğunlukla-kopuk" profili
(örn. %90-kopuk, %10-pencerede 3 kbps), mevcut fix'lerle. Ölç:
- (a) Kopma-sonrası ilk-pencerede motor ne kadar hızlı veri geçiriyor — PRE_CONVERGE yeniden
  devreye giriyor mu, yoksa eski-state'le mi açılıyor?
- (b) 8s-sessizlik sonrası ilk-SACK srtt-fix tarafından doğru mu yorumlanıyor ("8s = kayıp" mı
  sanıyor — D-1'in kökü aralıklıda geri döner mi)?
- (c) restore_burst kopma-sonrası spike üretiyor mu?
- **Beklenti-kaydı önce.** Motorun EN ÇOK fark yaratacağı (rakip handshake-kuramaz) ama en az
  test-edilmiş senaryosu.

## Eksen-3: Rejim-geçişi — TASARLANMADI; test yok; en kırılgan halka; gerçek-dünyada EN SIK

Hareket eden araç (trafo-ekibi, sahra) bir oturumda 4 rejim geçer: 3G-alan(400k) → kenar-hücre(3k)
→ kopma → kalabalık-hub(bağlanamama). Tüm test-profilleri SABİT-parametre; hiçbiri geçiş içermiyor.
Risk: btlbw 400k'ya adapte olmuşken kanal aniden 3k'ya düşerse motor eski-yüksek-btlbw'yle FLOOD
eder mi (MIN_RATE-faciasının tersi)? Adaptif-formüller teoride destekliyor ama KESKİN geçiş hiç
düşünülmedi.

### DV-3 (rejim-geçişi)
Dinamik-profil — tek koşuda bant **{400k → 3k → 0(kopma) → 8k}** basamakları, her basamakta N sn. Ölç:
- (a) Keskin-düşüşte (400k→3k) flood var mı; yeni-banda yakınsama süresi?
- (b) Keskin-çıkışta (3k→400k) ProbeUp yeni-headroom'u buluyor mu; ne kadar sürede?
- (c) Kopma→dönüşte PRE_CONVERGE/estimator-reset doğru mu?
- Muhtemelen YENİ tasarım-kararı gerektirir (estimator'ın "kanal-değişti" tespiti) — çıkarsa
  ADIM-6-sonrası mini-faz olarak KAYDET; bu turda SADECE ölç+teşhis, fix DEĞİL.

## Sıra ve kapsam

1. **Önce motor-temel bitsin:** TUR-4 (fuzzy — P5-GATE dar-IQR son-mührü, p5-determinizmi).
   Bitmeden DV-turları erken (kirli-zeminde aralıklı/geçiş ölçmek atıf-karıştırır).
2. **Sonra DV-1 → DV-2 → DV-3** (bant → aralıklı → geçiş; kolaydan-kırılgana).
3. Her DV: beklenti-kaydı önce; bulgu = teşhis (fix ayrı karar); DV-2/DV-3 muhtemelen yeni
   bulgu/tasarım açar — kaydet, aceleyle fix'leme.
4. **Kampanya-bağı:** DV-2/DV-3 sonuçları r5'in h-profillerini (h1-h6) besler — rejim-geçişi
   r5'te SABİT-profil, gerçek-dünya dinamik; DV-3 bulgusu r5'e "dinamik-geçiş profili"
   eklenmesini gerektirebilir. Kampanya-öncesi kapanmalı.
