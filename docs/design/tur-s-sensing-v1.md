# tur-s-sensing-v1 — Kanal-Durumu-Sensing Katmanı [r1 — ★ KISMİ-NO-GO: FAZ-1a-dedektör-çürüdü; fazlama+FAZ-0-ayakta]

## ★ r1 KISMİ-NO-GO — FAZ-1a-survivor-ritmi NAİF-FORMU-ÇÜRÜDÜ (doğrulama wf_ac16274a; dinamik-simli)

**FAZLAMA + FAZ-0 + bütçe-beyanı + kritiklik AYAKTA; çürüyen FAZ-1a'nın-dedektör-çekirdeği:**
- **HIGH-1 [benim-yön-transpozem]:** p5-hesabım %5-FORWARD-olasılığını %80-DÖNÜŞ-dedektörüne-taşımış.
  Doğrusu: P(3-ardışık-SACK-kaybı)=0.8³=0.512 → üçlü-kayıp-34.4s=eşik-TAM-SINIR; dörtlü-(P=0.41)-hep-aşar.
  Verifier-dinamik-simi: İKİ-UÇLU-başarısızlık — sabit-baseline'da sahte-kopma-FIRTINASI; adaptif-
  baseline'da medyan-eşik-100.7s → p5'te-tam-SAĞIR. **Bilgi-sınırı: q=0.2/E=8.39s'de α=0.05-eşiği-113s
  → p5'te-<2dk-kopmalar-BU-DEDEKTÖR-SINIFIYLA-AYIRT-EDİLEMEZ.** "Yapısal-sessiz"-iddiam-ÖLÜ.
- **HIGH-2:** baseline-B2-istisnası-DEĞİL dominant-yollarda: freq/tail-ritmi sender'ın-KENDİ-pacing'inden
  → kopma-öncesi-yavaşlama-launder'ı; FLOOR-tavanıyla-sınırlı-ama tespit-gecikmesi-×336-şişebilir
  (DPI/throttle-adversary-açabilir).
- **MED'ler:** clamp-kümülatif-ratchet (per-write-doğru-kümülatif-yanlış: 2.7×T-lineer-maliyetle-sınırsız;
  p5-doğal-gürültü-P≈0.26/çift-kendisi-yürütür → mutlak-üst-taban-≤c×FLOOR-pini-gerekli) ·
  FLOOR/T_SPACING=4.0=K_EPISODE-tam-eşitlik-bıçak-sırtı (kota→cadence-geçişi-garanti-sahte-epizot;
  çapraz-dedektör-zincirleme) · FLOOR-heartbeat-atfım-eksik (tail-pre-empt: kopmada-2×stale_last_gap-
  duplicate-SACK — bütçe-ihlali + bimodal-heartbeat).

**YAPISAL-SONUÇ (Abkarada'ya):** %80-kayıplı-dönüşte YEREL-RİTİM-GÖZLEMİ-yetmez — B2-meta-kökün
dönüş-yolu-İZOMORFU. **FAZ-1b'nin-kümülatif-emisyon-sayacı (kayba-bağışık) opsiyonel-keskinleştirme-
DEĞİL, dedektörün-ÇEKİRDEK-ön-koşulu-olabilir** (sayaç-delta → kayıp-kestirimi → eşik-kayıp-düzeltmeli
→ fırtına+sağırlık-ikisi-birden-çözülür-Mü — TEORİLEME-YAPILMIYOR, sim-ölçer: verifier-lens1_sim.py-
temeli + emisyon-sayaçlı-varyant). FAZ-yeniden-sıralama-adayı: 1b-çekirdeğe (V3-frame-öne) — karar-
Abkarada. FAZ-0-qd-fix'i bu-çürümeden-BAĞIMSIZ (S1-hemen-önerisi-ayakta).


> **Statü:** D0-TASARIM-NOTU (Abkarada TUR-S-GO, yapı-şartlı). KOD YOK. Zemin: 3-lens kod-teşhis
> (wf_ec93d9b8) + Y3/m45/B2-DUR/return-supply-kayıtları. **İLK-GÖREV FAZLAMA** (madde-1):
> altı-girdi tek-tura-sığmaz — bu-fazın-kanıtlı-dersi.

## 0. TEŞHİSİN İKİ ALTIN-BULGUSU (fazlamayı şekillendirir)

- **ÖLÜ-6B:** V2-SACK'te `seen_count`(4B) + `arrival_rate`(2B) HER-SACK'te ZATEN-ödeniyor (65B-wire'ın
  %9.2'si) ve sender-motor SIFIR-tüketiyor (bench kama.rs:245 düşürüyor; arrival_rate receiver'da-da
  sentinel-0xFFFF). r4-dersi-muhasebesinin-ilk-kalemi: **yeni-byte-eklemeden-önce ödenmiş-byte'lar
  kablolanır.** → FAZ-1a'nın-yarısı sıfır-wire-maliyetli.
- **qd-dar-fix TEK-TOKEN:** sender.rs:521 `ack_epoch → w_epoch` (Y3-w_epoch zaten-513-517'de-hesaplı).
  Little-bit-değişmez (rate.rs-girdisi-srtt; qd-tüketicileri yalnız fusion-sr1 + ProbeUp-abort/giriş);
  çift-fren-yapısal-imkânsız (min()-kompozisyon); sağlıklı-rejimde-DEĞER-ÖZDEŞ (rate≈dbw → EAI≡w_epoch;
  tüm-DV-1-decade'lerde ~0-değişim); kazanç: DESEN-1'de qd onset+8s-canlanır (36.600‰ ≫ 2000-omuz) +
  expiry-sonrası-kalıcı-12.8s-kuyruk soft-drain'le-~90s'de-sıfırlanır + ProbeUp-flood-içine-girme-bloke.
  → **TUR-S'ten-AYRIL: S-FAZ-0 mini-fix** (kendi-KIRMIZI-seti; kampanya-öncesi-zorunlu-statü-korunur).

## 1. FAZ-HARİTASI (iskelet-onaylı + iki-gerekçeli-düzeltme)

```
S-FAZ-0  qd-deadness dar-fix (TEK-TOKEN; TUR-S-öncesi-mini)        [K8; kampanya-öncesi-ZORUNLU]
S-FAZ-1a Sensing-çekirdeği, SIFIR-WIRE-MALİYET:                     [kampanya-öncesi-ZORUNLU]
         · survivor-ritmi kopma-tespiti (sender-taraf SACK-inter-arrival ring + median-baseline
           + K-çarpan — receiver-epizot-dedektörünün sender-aynası)
         · seen_count-KABLOLAMA (ölü-4B → yön-atfı: return-dead ⊥ forward-dead)
         · klempli-pasif-goodput: return_bw_est = max(SACK-varış-oranı, 31-prior) (0B)
         · DÖNÜŞ-SINIR-CLAMP-kuralı (aşağıda §3 — ters-ratchet-kilidi)
S-FAZ-1b +2B V3-frame: receiver-emisyon-sayacı → hayatta-kalma-oranı + yön-atfı-TAM      [aşağıda]
S-FAZ-2  dönüş-tespiti-tüketicileri: RecoverArq-bant-keşfi (Ş2-uyumlu) + frozen-W-çöz    [ZORUNLU-aday]
E-FULL-BAĞLANMA (2.097s-prior üç-tüketicisi) — İÇ-SIRA (lens-2-riski):
         (3) kopma-süre-saati (yeni-tüketici, mühür-geçmişi-yok — ÖNCE)
         (2) delivered-EAI-term2 (K6R-mühür-yeniden-doğrulama-ister — SONRA)
         (1) T_SPACING (p5-emisyon-takviminin-kendisi = mühür-çekirdeği — EN-SON, kendi-{100×12})
```
**Düzeltme-1 (iskeletten):** E-FULL+kopma "ayrılamaz-çift"i İKİ-alt-adıma-bölündü (1a/1b) — çünkü
kopma-tespiti return_bw-ÖLÇÜMÜNE-değil return_bw-SAATLİ-RİTME-bakar; ritim 0B-pasif-ölçülür (1a),
tam-bant/hayatta-kalma +2B-ister (1b). **Düzeltme-2:** qd TUR-S-dışı S-FAZ-0 (tek-token; sensing-
tasarımını-beklemesi-gereksiz-risk).

## 2. E-FULL DÖNÜŞ-BÜTÇE-BEYANI (madde-2)

**PASİF-FIRST; AKTİF-PROB YOK.** Üç-pasif-aday (lens-1):
| aday | wire-maliyet | p5-%80-kayıpta | hüküm |
|---|---|---|---|
| (i) SACK-varış-oranı+boyut (klempli) | **0B** | KÖR-ama-ZARARSIZ (31-prior-klempinde-kalır; ham-kullanım-YASAK: 1.55 B/s-ölçer → term2-42s ANTİ-FİX) | FAZ-1a ✓ |
| (ii) emisyon-sayacı (u16-wrap-21.8dk) | **+2B** (%3.1-serializasyon → T_SPACING+64ms/FLOOR+258ms [CAL]; kayıp-rejiminde-≈0-maliyet — netem-loss-paket-başına) | kümülatif→KAYBA-BAĞIŞIK: tek-survivor aradaki-tüm-emisyonu-taşır; R=4-epizotta P(≥1-survivor)=0.59; n=10'da-±12.6-puan (decade-doğruluk-yeter, ZM-2) | FAZ-1b ✓ — **V3-frame (0x15) = engine-spec-§2.2-NORMATİF-değişiklik** (V1→V2-TUR-1x-emsali; eski-decode-korunur) |
| (iii) ack_delay-türetimi | 0B | birincil-RED (p5-gürültü-saniyeler); ikincil: emisyon-anı-rekonstrüksiyon-çapraz-doğrulama | destekleyici |
**T_SPACING-yeterlilik-notu:** ölçüm-sender'da-doğar; receiver-tüketicisi (T_SPACING) forward-yönlü-
feedback-alanı-ister (DATA-header — dönüş-bütçesine-0B) → T_SPACING-bağlanması-zaten-EN-SON (iç-sıra).
**P5-GATE-mekanizma-koruması:** survivor-ritmi p5'te-yapısal-SESSİZ — p5-baseline'ı-zaten-8.6s-survivor-
aralığı; çift-kayıp-25.8s < K=4-eşiği-34.4s → sahte-kopma-üretmez. + Receiver-kopma-heartbeat'i
(FLOOR-8.39s-tekrarları, receiver.rs:295-299) sender-rate'ten-bağımsız srtt-free-ritim-tabanı.

## 3. KOPMA-TESPİTİ — ratchet-analizi (B2-DUR-meta-köküne-karşı)

- **Kopma-SIRASINDA ratchet YAPISAL-İMKÂNSIZ:** return_bw/ritim-baseline'ı kopmada-örnek-ÜRETMEZ
  (donuk); sessizlik-sayacı wall-clock-monoton, eşik-sabit → sınırlı-sürede-ateşler. (srtt-ratchet'in
  girdi-kanalı — fenomenin-ürettiği-örnekle-eşik-şişmesi — fiziksel-kapalı.)
- **DÖNÜŞ-SINIRINDA ZORUNLU-CLAMP:** epizotu-sonlandıran-gap ham-girerse ters-işaretli-ratchet
  (65B/30s≈2 B/s → sonraki-eşik≈K×önceki-kopma-süresi; p6-tekrarlı-kopmada-kilitlenir). KURAL:
  epizot-sonlandıran-gap baseline'a clamp'li-girer/girmez (receiver.rs:199-204-aynası; well-founded:
  tespit-örnekten-ÖNCE). Kalıntı: eşik-altı-kopma-gap'leri ≤K×baseline-sınırlı-zehir (r3-geometrik-
  yakınsama-sınıfı) — BEYANLI.
- Tüketiciler (FAZ-eşleme): kopma-tespiti → **W-pencere-DONDUR** (btlbw+qd frozen-W-politikası; Y3-
  kopma-frozen-sınır-vakası burada-kapanır) + **B2-örnek-sınıfı** (kopma-dönemi-straggler INELIGIBLE —
  arşiv-B2'nin-dış-referanslı-dar-hali) + (B1-kör-akış-kesi FAZ-2-değerlendirmesi).

## 4. S-FAZ-2 — dönüş-tespiti + RecoverArq-bant-keşfi (Ş2-RED-uyumlu)

Dönüş-sinyali: ilk-taze-SACK + ritim-canlanması (+1b-varsa Δseen>0 forward-canlı-teyidi).
**RecoverArq-keşfi Ş2-uyumlu-tasarım-alanı** (recovery-İÇİ-ProbeUp-YASAK — P5-GATE-mühür-bölgesi):
kod-gerçeği: pending≠∅ → RecoverArq-her-olayda (sender.rs:871-874); ProbeUp yalnız-Steady'den.
Aday-sınıf: dönüş-tespitinde (a) btlbw-frozen-W-ÇÖZ + (b) RecoverArq-ÇIKIŞ-sonrası-İLK-Steady'de
period-muafiyetli-probe (TUR-D1-S4-emsali: yalnız-period-bileşeni; qd_ok∧backlog-bit-aynı) —
faz-önceliğine-DOKUNMAZ (retx-drenajı-tamamlanır, sonra-keşif). Üç-kanıt-ticket'i (p5-probe=0 +
sim-P4-45s + netem-P4-delta) burada-kapanır. Detay-tasarım FAZ-2-D0-mini'sinde (bu-D0-fazlama-notu).

## 5. Ş4-KAMPANYA-KRİTİKLİK + FEC-SIRALAMA-GİRDİSİ (madde-3)

| iş | p6/h4-RMST-etkisi | hüküm |
|---|---|---|
| S-FAZ-0 qd-fix | expiry-sonrası-kalıcı-kuyruk-drenajı (12.8s-srtt*-silinir) → paced-RMST-doğrudan | **ZORUNLU** |
| S-FAZ-1a sensing-çekirdeği | kopma-tespiti = frozen-W + B2-sınıfı → p6-toparlanma-doğruluğu | **ZORUNLU** |
| S-FAZ-1b +2B-V3 | 1a'yı-keskinleştirir (yön-atfı-tam); RMST-ikincil | ERTELENEBİLİR-aday (V3-frame-işi-büyük) |
| S-FAZ-2 dönüş+keşif | P4-drenaj (+108-vs-+180) h4/p6-RMST'yi-DOĞRUDAN-etkiler | **ZORUNLU-aday** (P4-RMST-mertebesi-netem-K6R-verisinden: delta-%3.6-teslim ≈ RMST-görünür) |
| E-FULL-(1) T_SPACING-bağlanması | p5-emisyon-takvimi — mühür-çekirdeği | kampanya-SONRASI-aday (kendi-{100×12}-turu) |
**FEC-PR-2-sıralama-önerisi:** S-FAZ-0 (mini, günler-değil-saatler) → FEC-PR-2 (kampanya-baseline-
ön-koşulu) → S-FAZ-1a → S-FAZ-2 → (1b/T_SPACING kampanya-sonrası). Gerekçe: FAZ-1a/2 tasarım-ağır;
FEC-PR-2-bağımsız; qd-fix-tek-token-hemen. KARAR-Abkarada (madde-5).

## 6. SİM-HARNESS-İHTİYACI (madde-4) + KIRMIZI

- desen1_sim-GENİŞLETMESİ gerekir: **rev-link'e kayıp-modeli** (survivor-ritmi-senaryoları) +
  kopma/dönüş-senaryosu (3A2'de-var). **Kalibrasyon (mekanizma-izi):** p5-survivor-aralığı-8.6s +
  DV-2-p6b-epizot-deseni repro; etmezse-harness-düzelt.
- **KIRMIZI (tüm-fazlar):** temiz-1k=45 · P5{100×12,S5} · DESEN-2-ramp · 0.25k-anchor ·
  **Y3-pinleri** (uzun-daralma-yakınsama + 4-nokta-anchor — desen1_sim-assert'leri) ·
  S-FAZ-0-için-ek: DESEN-2-ramp-özel-nöbet (ProbeUp-giriş-histerezisi-canlanıyor — S4-emsali).

## 7. Açık-sorular (onayla)
- **S1: FAZ-0-qd-fix'i hemen-mi** (tek-token; TUR-S-D0-onayıyla-birlikte-kod)? Öneri: EVET.
- **S2: FAZ-1b-V3-frame** kampanya-öncesi-mi-sonrası-mı? Öneri: sonrası (1a-yeter-başlangıç; V3-
  normatif-değişiklik-kendi-turunu-hak-eder).
- **S3: FEC-PR-2-sırası** (§5-önerisi): FAZ-0 → FEC → FAZ-1a → FAZ-2?
