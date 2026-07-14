# m45-kanal-durumu-v1 — Kanal-Durumu-Değişim-Tespiti: İki-Tur Ayrışımı + Daralma-Dar-Fix [TASLAK r2 — ONAY BEKLİYOR]

> **r1→r2 (doğrulama wf_8395cf95 — çekirdek SAĞLAM, muhasebe 4-yerinden sertleşti + aritmetiğim düzeltildi):**
> Çekirdek DOĞRULANDI-TEMİZ: anti-ratchet yönü + aktüatör-yan-etkisizliği kod-düzeyinde tutuyor;
> üç-kısıt cevap-varlığı TAM. Kırılanlar (B2-tipi meta-kök DEĞİL — somut fix-sınıflı muhasebe):
> (F1) kümülatif-SACK batch'i K_SHRINK'i TEK-olayda doldurur (kopma-dönüşünde yanlış-reset; "straggler
> dolduramaz" iddiam kod-yanlış) → **OLAY-BAŞINA SAYAÇ ≤1 + TAZE-SNAPSHOT-ŞARTI**; (F2) genişleme-
> sonrası bayat-kohort yeni-btlbw'ye karşı sayılır (SACK azalan-blok + batching → DESEN-2-geri-alma)
> → **YAŞ-KAPISI** (sent_ns ≥ btlbw-son-yükselme-anı; gerçek-daralmada saydam); (F3) gain-faz damgası
> gönderim-ANINDA SentPacket'e (Drain-0.347-kohortu Steady'de ack'lenir → her-başlangıçta yanlış-reset)
> + anti-vacuous-vektör; (F4) semantik-pin: geçersiz=ATLA (sıfırlamaz), sayaç yalnız GEÇERLİ-aykırıyla
> sıfırlanır. ARİTMETİK-DÜZELTMELERİM (dürüstlük): denge bağlayıcı-terimi min_cwnd (srtt*≈12.8s;
> "26.5s≈plato" retrofit'ti — srtt aslında MONOTON-tırmanıyor 0.84-0.92 s/s → tespit-zorunluluğu
> GÜÇLENİR: denge bile yok, kaçak-tırmanış); yanlış-tetik maliyeti "13×/1s" değil probe-GİRİŞ-gecikmeli
> (~26s-sınıfı, period-kapılı) → S4-önerisi: shrink-reset probe-timer'ı da sıfırlar → maliyet ~1-epoch.

> **Statü:** D0-TASARIM-NOTU (Abkarada M4/M5-GO; üç-zorunlu-kısıt a/b/c cevaplı). ONAYDAN SONRA kod.
> **Çerçeve (Abkarada):** "aralıklı-kanal-sensing" DEĞİL → **"kanal-durumu-DEĞİŞİM-tespiti"** —
> üç-geçiş: KOPMA (B1+B2) · DARALMA (DESEN-1) · GENİŞLEME (DESEN-2, KORUNUR). Zemin: 3-lens
> kod-teşhis-workflow'u (wf_898af776) + DV-2/DV-3 ölçümleri + B2-DUR meta-kök-kaydı.

## 0. KISIT-B CEVABI [TUR-SAYISI-BELİRLEYİCİ]: DESEN-1 bağımsız-dar-fix'e UYGUN → İKİ-TUR AYRIŞIMI

**Hüküm: EVET — daralma-tespiti M4/M5-sensing GEREKTİRMEZ; yerel-dar-fix.** Dört kod-gerekçe:
1. **Kanıt-altyapısı hazır:** DeliveryRateSampler (bw.rs:46-127, BBR-§7.1) her ACK'te RateSample
   üretiyor; girdileri SALT monoton-damga-FARKLARI — srtt/rtprop/EAI hiçbir-bacakta YOK.
2. **B2-meta-kök BULAŞMAZ:** kopma örnek-YOKLUĞU üretir, daralma düşük-değerli örnek-VARLIĞI —
   ayrım örnek-varlığı düzeyinde, tek-örnek-yorumu düzeyinde değil. B2-DUR'un daraltılmış-ifadesi
   ("ACK-dizisi + gönderim-geçmişi eklenince aynı gözlem-uzayında değiliz") delivery-DİZİSİNİ
   izinli-sınıfa koyar — ve bu M4-dış-referans GEREKTİRMEDEN mevcut-yerel-ölçümle kurulur.
3. **Öz-referans yönü DÜZELTİCİ (anti-ratchet):** zehirlenen-büyüklük (bayat-yüksek btlbw)
   tespit-EŞİĞİ değil tespit-KONUSU — btlbw ne kadar bayatsa `delivery ≪ btlbw/K` o kadar KOLAY
   ateşler (negatif-geri-besleme; B2'de zehir eşiği şişirip tespiti BASTIRIYORDU). EWMA-entegrasyon
   yok — B2-amplifikatörünün (kuantum-n=64) eşdeğeri yok; tek-aykırı-örnek max'ı oynatamaz.
4. **Mevcut-savunmalar DESEN-1'e KÖR (dar-fix gerekçesi):** Emergency sessizlik-anahtarlı
   (daralmada ACK akar → conf=1000 → hiç-görmez); fuzzy soft-drain tabanı 0.5× — 13×-sapmayı
   yapısal-düzeltemez; Little-freni toparlanma değil **ZEHİRLİ-DENGE** (aşağıda §1).

**→ TUR-YAPISI: (TUR-D1) daralma-dar-fix — M4-BEKLEMEZ, önce · (TUR-S) kopma-sensing —
E-FULL-return_bw'lu, sonra.** Etiket-dürüstlüğü: TUR-D1, §0.30-listesi madde-1'in dar-önalımıdır;
madde-2 (kopma-ayrımı) TUR-S'de KALIR; **"sensing çözüldü" iddiası YASAK.**

## 1. KISIT-A CEVABI: asimetri EPİSTEMİK; tek-yönlü-fix GEREK-ve-YETER (3 koruma-şartıyla)

**Asimetrinin doğası:** yüksek delivered-örnek kapasitenin KANITI (X teslim-edildi ⇒ kanal ≥ X) →
max-filter'ı 0-örnek-gecikmeyle yükseltir (bw.rs:175,193); düşük-örnek kapasitesizliğin kanıtı
DEĞİL (app-limited/gain<1/geçici olabilir) → max'a hiç-değmez; daralma-kanıtı ancak DİZİ-düzeyinde.
`delivery ≫ btlbw` sinyali yapısal-BOŞ (ilk-aşan-örnek max'ı zıplatır — sürekli-fazlalık
gözlemlenemez) → genişleme kendi-kendine-görünür, **tek-yönlü fix (yalnız-daralma) yeterli;
genişleme-yolu (bw.rs:170-194 + ProbeUp sender.rs:920-976 + rate.rs:37) BİT-DEĞİŞMEDEN kalır.**

**Neden tespit ZORUNLU (bekleyerek-düzelme yok) — iki ölçülü-mekanizma:**
- **Bayat-max ÖLÜMSÜZLÜK-aritmetiği (AYRI alt-kök):** W_BTLBW = max(10s, 8×ack_epoch),
  ack_epoch=max(srtt,EAI) (sender.rs:500-503) → daralma→kuyruk→srtt↑→W↑. d(cutoff)/dt =
  1−8×d(srtt)/dt: eğim > 0.125 s/s iken cutoff GERİLER. DESEN-1 traj-lineer-fit eğimi
  **0.84-0.92 s/s (eşiğin ~7×)** [r2: r1'deki "0.4-0.5" pencere-dışı-ortalamaydı, düzeltildi]
  → 4119 ölümsüzdü. (TUR-3a sub-kbps-pencere-fix'i
  daralma-rejiminde anti-fix'e dönüyor — W-türetim-alt-sorusu §4-S2.)
- **Little-freni frenler-AMA-durduramaz [r2-düzeltme — r1-aritmetiğim yanlıştı]:** bağlayıcı-terim
  min_cwnd=4800 (2×BDP=631B < 4800; rate.rs:145/173 — traj: rate×srtt=4798-4799 sabit), rtprop
  76.6ms (1s değil). Teorik-denge srtt*=4800/375≈12.8s AMA ölçülen srtt MONOTON-TIRMANIYOR
  (plato yok — r1'in "24-25s plato örtüşmesi" retrofit'ti, geri-çekildi) → durum r1-iddiamdan
  KÖTÜ: denge bile kurulamıyor, kaçak-tırmanış; kırılma yalnız dış-olayla (t=120 loss). Tespit-
  zorunluluğu-hükmü GÜÇLENEREK korunur.

**Üç koruma-şartı (DESEN-2 dokunulmazlığı):** (i) aktüatör YALNIZ-DÜŞÜRÜR (btlbw'yi asla
yükseltmez — yukarı-yol bit-aynı); (ii) yanlış-tetik maliyeti SINIRLI-ve-ölçülü: ProbeUp 13×/~1s
geri-tırmanır (DESEN-2-kanıtı) + dedektör kendiliğinden-sınırlı (demotion-sonrası delivered≈btlbw_yeni
→ oran≈1 → yeniden-ateşleyemez; yukarı-keşif ProbeUp'ın); (iii) **öz-kısma-muafiyeti:** gain<1
fazlarında (Drain 0.347× / DrainRtprop 0.75× / PreConverge) dedektör SUSAR — motor kendi kastığını
daralma-sanmaz (sender kendi-fazını bilir).

## 2. KISIT-C CEVABI: 3-kök × dış-referans matrisi

| kök | yerel-sinyal | ratchet-riski | E-FULL/M4 ihtiyacı |
|---|---|---|---|
| **DARALMA** | delivery-örnekleri vs btlbw — iki-bacak bw-domain, oran boyutsuz (SAATSİZ) | DÜŞÜK-yapısal, işaret TERS (anti-ratchet); tek-tuzak: sebat-saati süre-olursa srtt-bağı girer → **sebat = örnek-SAYISI** (varış=ACK=fiziksel-drain-ritmi; kanal-yaşadığından saat-yaşar) | **GEREKMEZ** |
| **KOPMA (B1+B2)** | delivered-delta=0/ACK-yokluğu — sinyal-katmanı temiz AMA | **YÜKSEK:** süre-eşiğinin TÜM canlı-saat-adayları srtt-türevli (3×srtt=b2-kanıtlı-körlük; EAI∝srtt rate.rs:176-zinciri; ack_epoch iki-dalı-kirli); ratchet epizotlar-ARASI kapanır (straggler→n=64→srtt↑→sonraki-kopma-eşiği↑) | **GEREKLİ** — 3-gerekçe: sınır-bölge ayrılamaz (sub-kbps kopma≈EAI-mertebesi; T_SPACING-izomorfu); süre-saatinin dürüst-kaynağı ölçülen-return_bw (E-FULL 3. tüketici: kopma-süre-saati); yön-atfı (forward-vs-return-kopma ancak dönüş-yolu-ölçümüyle) |
| **GENİŞLEME** | ProbeUp mevcut — 13×/~1s ölçülü-mükemmel | — | — (DOKUNMA-listesi) |

## 3. TUR-D1 MEKANİZMASI (D0-düzeyi; kod onay-sonrası)

```
DEDEKTÖR (r2 — olay-saatli + dört-kapılı):
  GEÇERLİ-örnek şartları (hepsi; sağlanmayan örnek = ATLA, sayacı SIFIRLAMAZ [F4-pin]):
    !sample.is_app_limited                        ← arz-düşüşü ≠ kanal (bw.rs:30 kablolu)
    ∧ gönderim-ANI-damgalı gain ≥ 1 [F3: SentPacket'e faz-alanı; tombstone-kol-1 de taşır]
    ∧ sent_ns ≥ btlbw_son_yükselme_ns [F2 YAŞ-KAPISI — gerçek-daralmada btlbw yükselmez → saydam;
      genişlemede bayat-kohort yapısal-dışlanır; K_RATIO=2 probe-bileşiğine karşı güvenli-kalır]
    ∧ TAZE-SNAPSHOT [F1: örneğin delivered_time-damgası bu-ACK-olayından-önceki-olayda tazelenmiş
      olmalı — kopma-yayılımlı bayat-interval örnekleri daralma-kanıtı DEĞİL, kopma-teşhis-sayacına]
  SAYAÇ [F1]: OLAY-BAŞINA (SACK-başına) EN-FAZLA +1 — batch N-örnek ≠ N-kanıt ("ARDIŞIK"ın saati
    varış-sırası; batching'de zaman-eşdeğerliği bozulur → olay-sayacı geri-kurar).
  ATEŞLEME: K_SHRINK [CAL {3,5}] ardışık-OLAY, her-birinde ≥1 geçerli-örnek < btlbw/K_RATIO [CAL {2,4}];
    GEÇERLİ-örnek oran-şartını sağlamazsa (≥ btlbw/K_RATIO) → sayaç-sıfırla (tek-sıfırlama-yolu).
AKTÜATÖR (yalnız-düşürür):
  btlbw-filter RESET → pencere := tetikleyen-olayların GEÇERLİ-örnekleri (recent-max; tek-örnek-
  reseed'den kaçınılmış). Yükseltme-KOD-yolu dokunulmaz. diag_shrink_resets++ + TRAJ "shrink=".
  [S4-önerisi] reset ProbeUp period-timer'ını da sıfırlar → yanlış-tetik-maliyeti probe-giriş-
  gecikmesinden (~26s-sınıfı, ölçülü) ~1-epoch'a iner; salınımlı-kanalda dip-sonrası hızlı geri-öğrenme.
```
Kopma-dönüş-analizi (F1-sonrası): dönüş-SACK'i sayacı en-fazla-1 artırır; sonraki-olaylar TAZE-
snapshot'lı gerçek-kanal-örnekleri getirir — kanal aynı-hızda döndüyse oran≈1 → sayaç-sıfırlanır
(p6b-tekrarlı-kopma K_SHRINK'e ULAŞAMAZ); kanal gerçekten-daraldıysa (40k→kopma→8k) ateşleme DOĞRU.
- **qd-eş-sinyal ZORUNLU-KAPI DEĞİL [ölçülü-gerekçe]:** DV-3-TRAJ floodda qd≈10µs DONUK-SIFIR
  ölçtü (W_qd-srtt-bağı; lens-3-öngörüsü doğrulandı) — qd-kapısı dedektörü tam-hedef-senaryoda
  öldürürdü. Salt-teşhis-sayacına iner.
- **Loss-ayrımı (INV-RATE-1-analizi):** kalıcı-kayıp-kanalda (p5) btlbw ZATEN kayıp-sonrası-
  delivered'dan öğrenilmiş → oran≈1 → ateşlemez (yapısal). Kayıp-REJİM-değişimi delivered'ı
  gerçekten düşürürse btlbw-düşüşü sözleşme-İÇİ (INV-RATE-1: rate=delivered-rate tanımı; yasak
  olan loss-SAYACI→rate). **Ampirik-kapı yine ZORUNLU: P5-GATE {100×12} KIRMIZI-şart** + temiz-1k=45.

## 4. Açık sorular (onayla)

- **S1: W_BTLBW-türetim alt-fix'i bu-turda-mı?** (ack_epoch=max(srtt,EAI)'nin srtt-dalı ölümsüzlük-
  kökü; rtprop-epoch/EAI-tabanlı-türetim adayı). ÖNERİ: bu-turda-DEĞİL (tek-değişken; dedektör
  ölümsüzlüğü zaten kırar — reset expiry-beklemez); ölçüm-sonrası-karar.
- **S2: reset-sonrası pencere-tohumu** recent-max mı son-örnek mi (öneri: son-K_SHRINK-örneğin
  max'ı — tek-örnek-reseed'den kaçın, B2-halka-4-dersi).
- **S3: TUR-S kapsam-onayı** (kopma-sensing: E-FULL ölçülen-return_bw + kanal-durumu-makinesi;
  girdiler dv-plan-M4/M5-kaydında; ayrı-D0 o-turda).
- **S4 [r2]: shrink-reset ProbeUp period-timer'ını sıfırlasın mı?** Öneri: EVET — yanlış-tetik-
  maliyeti ~26s(probe-giriş-gecikmesi)→~1-epoch; salınımlı-kanal (40k↔3k hücresel-deseni) dip-
  sonrası hızlı geri-öğrenir. Tek-satır-bağlaşım; DESEN-2-koruması bit-aynı kalır.
- **S5 [r2 — beyanlı-sınır]: app_limited kapsama-körlüğü** — arz yeni-kapasitenin ÜSTÜNDE ama
  pacing'in ALTINDA kalan ara-vakada tx_queue-boş → app_limited=true → tespit Little-freni
  backlog-kurana dek GECİKİR (kaçırılmaz, gecikir; yedek-dedektör yok — qd bilinçli-indirildi).
  Beyanlı-kalıntı; DV-3-yeniden-ölçümünde gecikme-sınıfı raporlanır.

## 5. Ölçüm-planı (kod-GO sonrası; beklenti-kaydı ölçümden-önce)

Birim: dedektör-vektörleri (daralma-ateşler / app-limited-susar / gain<1-susar / kopma-sessizlikte-
ateşleyemez / p5-kalıcı-kayıp-ateşlemez). Sistem: **DV-3A2/3B2 yeniden** (ana-sınav: flood-kesimi —
infl-63× → ~2×BDP-sınıfı; srtt-plato 24-25s → çöker; toparlanma 60s+ → K_SHRINK-örnek-süresi) +
**DESEN-2-korunma** (3B2-çıkış 13×/~1s AYNEN) + P5-GATE p5×6 + temiz-1k + 0.25k. KIRMIZI: P5-GATE
bozulması / DESEN-2-yavaşlaması / temiz-1k<45.
