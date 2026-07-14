# DV-PLAN v2 — Gerçek-Dünya Doğrulama: üç eksen (Abkarada talimatı, 2026-07-13)

> **Statü:** ONAYLI PLAN (TUR-4-sonrası senkronizasyon). v1→v2: dört senkron-patch işlendi
> (P5-GATE sırası fuzzy'den çıkarıldı; DV-3 iki-profile bölündü; DV-1 matrisine 0.25k-anchor+500k;
> FNR borcu DV-2'ye + byte-cap kapsam-kilidi). Yürütme: motor-temel (TUR-4 ✓) → iki determinizm-
> kilidi + P5-GATE → DV-1 → DV-2 → DV-3. Her DV: beklenti-kaydı ÖNCE; tek-eksen; bulgu=TEŞHİS
> (fix ayrı karar). Kampanya-öncesi kapanmalı.

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
Mevcut fix'leri **{0.05k, 0.1k, 0.25k, 1k, 2k, 5k, 10k, 100k, 500k, 900k}** noktalarında koş.
0.25k = yeni-keşif-noktası DEĞİL, **bit-özdeş referans/anchor** (yeni decade'leri eski-anchor'a
bağlar). 500k eklendi (üst-uç regresyon sorusu 500k/900k'yı soruyordu, 500k matriste yoktu). İki soru:
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
- (d) **FNR/W_loss borcu (TUR-4'ten taşındı):** W_loss payda-çürümesi nedeniyle oluşan anlık
  windowed-lppm sıçramalarında fnr/zero-weight ziyaretlerini say. Özellikle RecoverFwd sırasında
  `pend>0` ile çakışma var mı kaydet. TUR-4'te bu bölge davranışsal-nötrdü (RecoverFwd ∧ pend=0);
  aralıklı-koşulda pend>0 çakışması görülürse **davranışsal bulgu aç, bu turda fix UYGULAMA.**
- **Beklenti-kaydı önce.** Motorun EN ÇOK fark yaratacağı (rakip handshake-kuramaz) ama en az
  test-edilmiş senaryosu.

**Byte-cap kapsam-kilidi (kayda):** Byte-cap DV-1'in bağımsız kalibrasyon değişkeni DEĞİLDİR;
MIN_CWND ile aynı mekanizma-ailesinde, son kalibrasyon fazında ele alınacaktır (ProbeUp gain'i
byte-cap'e bağlı çıkmadı — kuyruksuz kanalda Little-tavanı 2×btlbw > 1.25×btlbw; tek etkileşim
MIN_CWND-maskesi, ölçüldü).

## Eksen-3: Rejim-geçişi — TASARLANMADI; test yok; en kırılgan halka; gerçek-dünyada EN SIK

Hareket eden araç (trafo-ekibi, sahra) bir oturumda 4 rejim geçer: 3G-alan(400k) → kenar-hücre(3k)
→ kopma → kalabalık-hub(bağlanamama). Tüm test-profilleri SABİT-parametre; hiçbiri geçiş içermiyor.
Risk: btlbw 400k'ya adapte olmuşken kanal aniden 3k'ya düşerse motor eski-yüksek-btlbw'yle FLOOD
eder mi (MIN_RATE-faciasının tersi)? Adaptif-formüller teoride destekliyor ama KESKİN geçiş hiç
düşünülmedi.

### DV-3 (rejim-geçişi) — İKİ kontrollü profil (saha-gerçekçiliği ve geçiş-stresi karışmasın)
Test-tasarımı kuralı: her ölçüm-sorusu profilde GERÇEKTEN VAR OLAN bir geçişe bağlı olmalı
(v1'de 3k→400k sorusu vardı ama profilde o geçiş yoktu — düzeltildi).

**DV-3A — saha-yolu (gerçekçilik):** `400k → 3k → 0(kopma) → 8k`, her basamakta N sn. Ölç:
- (a) Keskin-DÜŞÜŞte (400k→3k) flood var mı; yeni-banda yakınsama süresi?
- (b) Kopma→dönüşte (0→8k) PRE_CONVERGE/estimator-reset doğru mu?

**DV-3B — simetrik-geçiş stresi (estimator/ProbeUp izolasyonu):** `3k → 400k → 3k`, her basamakta N sn. Ölç:
- (c) Keskin-ÇIKIŞta (3k→400k) ProbeUp yeni-headroom'u buluyor mu; ne kadar sürede?
- (d) Geri-DÜŞÜŞte (400k→3k) eski-yüksek-btlbw ile flood var mı (MIN_RATE-faciasının tersi);
  estimator yeni-daralmaya ne kadar sürede uyuyor?

- Muhtemelen YENİ tasarım-kararı gerektirir (estimator'ın "kanal-değişti" tespiti) — çıkarsa
  ADIM-6-sonrası mini-faz olarak KAYDET; bu turlarda SADECE ölç+teşhis, fix DEĞİL.

## Sıra ve kapsam

1. **TUR-4 fuzzy TAMAMLANDI.** Golden'lar bit-özdeş, 30 suite + clippy temiz. **Dar-IQR kabulü
   fuzzy kapsamından ÇIKARILMIŞTIR** (TUR-4 onu kapatamadı ve kapsam gereği kapatamazdı — ölçümle
   dekompoze, atıf-düzeltmesi). Kalan iki determinizm-kilidi fuzzy-DIŞI, DV-1'den ÖNCE kapanmalı:
   - **(i) dönüş-yolundaki ilk-SACK-şansı.** ÖNCE AYRIŞTIR (Abkarada şartı): bu kilit
     §3.3-E-FULL-tam-decimation mı gerektiriyor, yoksa dar-bir-fix (ilk-SACK-cadence-muafiyeti,
     tail-simetriği) mi yeter? Dar-fix yetiyorsa oraya bağlanır; tam-decimation gerçekten
     gerekiyorsa GEREKÇE gösterilir (neden dar-fix yetmez). **Not:** bu kilit DV-2'nin
     (aralıklı, kopma-sonrası-ilk-SACK) kökeniyle AYNI olabilir — öyleyse DV-2 hem gerçek-dünya
     doğrulaması hem P5-GATE-kilidi olur (iki amaç tek tur); ayrıştırma bunu netleştirir.
   - **(ii) conf-LOW RecoverFwd dilimi** (ayrı, muhtemelen daha küçük fix).
   - Bu ikisi kapandığında **P5-GATE mühürlenir.** Sonra DV-1.
2. **Sonra DV-1 → DV-2 → DV-3** (bant → aralıklı → geçiş; kolaydan-kırılgana). (i)-ayrıştırması
   DV-2'yi öne çekebilir — ilk-SACK-şansı = DV-2-kökü ise özel dikkat.
3. Her DV: beklenti-kaydı önce; bulgu = teşhis (fix ayrı karar); DV-2/DV-3 muhtemelen yeni
   bulgu/tasarım açar — kaydet, aceleyle fix'leme.
4. **Kampanya-bağı:** DV-2/DV-3 sonuçları r5'in h-profillerini (h1-h6) besler — rejim-geçişi
   r5'te SABİT-profil, gerçek-dünya dinamik; DV-3 bulgusu r5'e "dinamik-geçiş profili"
   eklenmesini gerektirebilir. Kampanya-öncesi kapanmalı.

## DV-1 SONUÇ-EKİ (2026-07-14 — DV-1 KAPANDI; iki AÇIK-RİSK kampanyaya taşınıyor)

DV-1: 10/10 nokta KIRMIZI'sız (time-model §0.28). [CAL]-decade-tutarlılığı ölçümle kapandı
(yeni-sabit çıkmadı). İKİ dürüstlük-notu "geçti" DEĞİL "kampanyaya-taşınıyor":

**(a) qd→min üst-uç regresyonu ÖLÇÜLMEDİ-kanıtlanmadı:** hafif-yükte tetiklenmedi ≠ ağır-yükte
tetiklenmez. → **h6 (10Mbit ağır-yük) kampanya-profilinde qd→min-ısırığı ÖZEL-İZLENECEK**;
r5 §L (kripto-CPU) yanına "üst-uç estimator-regresyon izleme" satırı (Abkarada r5-dosyasına işler).

**(b) eternal-Startup latency-borcu — LATENCY-ETKİ-ANALİZİ (Abkarada-sorusuna sayısal cevap):**
DV-1 gecikme-medyanları wire-doyum alt-sınırının 0.99-1.06×'ı (1k-900k; alt-uç 1.18-1.39 =
cold-start). **Upfront-backlog iş-yükünde (mevcut bench) "teslim-eder-ama-geç" GÖRÜNMEZ** —
gecikme wire-bound, motor-kuyruğu uçtan-uca süreye eklenmiyor. **CEZA SEYREK-VARIŞLI iş-yükünde:**
paced/interactive mesaj motorun şişkin kuyruğu (srtt-sınıfı: 1k'da ~70s, 0.05k'da 416s) arkasına
düşer → tek-mesaj-RMST felaketi. HÜKÜM iki-dallı: r5-profilleri upfront-backlog'sa cwnd-enforcement
SON'da kalabilir; **paced-arrival profili varsa kampanya-öncesi LATENCY-KRİTİK tur** (D-4/2 üç-
ölçümle tam-haritalı, tasarım-verisi hazır). Karar-girdisi: r5-iş-yükü tipi (Abkarada).

## DV-2 SONUÇ-EKİ (2026-07-14 — DV-2 KAPANDI; time-model §0.29)
6-profil ölçüldü (p6a-düzeltmeli). Üç açık-risk sonuçlandı: count=0-deliği zarar-düşük (beyanlı-
sınır kalır-önerisi); sürüklenme YOK-kanıtlı (clamp'li-yazım medyan-reddi ölçüldü); FNR/pend>0
gerçekleşmedi (fallback hâlâ davranışsal-pasif — p6b-ilk-yorum düzeltildi). İKİ yeni-tasarım-
bulgusu karara: B1 kopma-körlüğü (%74-israf; ACK-sessizlik-kapısı mekanizma-önerisi), B2
aralıklı-RTT-kirliliği (1.02→19.6s; rtt_rej=0 guard-deliği; send-freeze → p6b 18/20).
Sıra-önerisi: B2-dar-turu DV-3-ÖNCESİ (DV-3-ölçümünü kirletmesin); B1 DV-3-SONRASI paket.

## ★ ARALIKLI-KANAL-SENSING-KATMANI (M4/M5) — meta-kök TEK-KAYIT (Abkarada, 2026-07-14)

**META-KÖK (iki-bulgu-tek-kök):** DV-2'nin İKİ bulgusu — B1 kopma-körlüğü (forward: ne-zaman-
göndermeyi-durdur) + B2 RTT-kirliliği (estimator: örneği-nasıl-yorumla) — TEK meta-köke indirgendi:
**aralıklı kanalda MEVCUT yerel-gözlemlerle ve TEK gözlemden "kopma" ile "meşru-yavaş" ayırt-edilemez.**
Dört-tur-B2 (yerel-eşik-ailesi yapısal-ölü) + tek-tur-B1 (aynı-aile, kod-satırlı) bunu kanıtladı.
İkisinin de doğru-cevabı gözlem-uzayını-genişleten bir SENSING-KATMANI.

**M4/M5-TURU (DV-3-sonrası, D0-notu-önce) — artık "B2-fix" DEĞİL "aralıklı-kanal-sensing-katmanı":**
- **Girdiler:** E-FULL ölçülen-return_bw + ACK-sessizlik-epizodu + measured-delivery-cadence +
  estimator-örnek-güven-sınıfı + DV-3-verisi (hangi-rejimler-gerçekten-karıştı).
- **Çıktı:** kanal-durumu-makinesi (M5) → "kopma-mı-yavaş-mı" AYRIMI (M4-dış-referansla) → hem
  B1'i (forward-akış-kesim kararı) hem B2'yi (estimator-örnek-kabul) BESLER.
- **B1-alt-kümesi:** 1a-dup-oran-azaltma sensing-katmanı "kopma-kesin" dediğinde uygulanır
  (yerel-eşiksiz — dış-referanslı); 1b-ÇIKARILDI (HIGH-2 SACK-truncation).
- **M3 (estimator-decoupling):** BAĞIMSIZ mimari-sertleştirme (defense-in-depth; M4/M5-rakip-değil-
  katman) — M4/M5-sonrası, ayrı.
- **KIRMIZI-şart:** M4/M5 P5-GATE {100×12}-mührünü KORUMALI (B1-parasitic-recovery-riski burada
  dış-referansla çözülür — kör-akış yalnız gerçek-kopmada kesilir, p5-yüksek-kayıp-ama-aktif'te DEĞİL).
