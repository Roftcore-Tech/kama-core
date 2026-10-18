# Kama Transport v1 — EVRİM KURALLARI

Amaç: gelecekteki geliştirici tüm araştırma tarihini yeniden yaşamadan Kama'yı geliştirebilsin.
Her aday için: v1 durumu · neden böyle donduruldu · yeniden-açma tetiği · gereken kanıt ·
etkilenen T-hücreleri · YAPMA kuralı.

## 9.1 Sıfır-ACK bootstrap

| | |
|---|---|
| v1 durumu | `bootstrap_rtt = false`, `bootstrap_fast_ladder = false`; doğrulanmamış kurtarma mekanizmaları sevk DIŞI |
| neden donduruldu | kalibre edilmemiş; açmak 990 koşuluk kanıtı geçersiz kılar (INV-008) |
| tetik | saha/ürün kanıtı "ilk uçuşta sıfır ACK" senaryosunun operasyonel hedef olduğunu gösterirse |
| gereken kanıt | temiz yol no-harm · rastgele-kayıp kuruluş (T1a/T1b eşdeğeri) · bozuk-dönüş (T6 eşdeğeri) · N≥30 + CI · yeni kimlik `transport-v1.x` |
| etkilenen hücreler | T0, T1a, T1b, T1c, T2b, T6 (kuruluş yolu geçen her hücre) |
| YAPMA | ilk pencereyi büyüterek "çözmek" (yalnız olasılık kaydırır); kör ACK'siz yeniden-iletim eklemek (yeni mekanizma) |

## 9.2 FEC maddeleşme ölü-bölgesi

| | |
|---|---|
| v1 durumu | bazı öneriler §8.5 kısıtı sonrası K=8'de tek onarım sembolünün altına iniyor; T1a/T1c/T3a'da sıfır parity çerçevesi; sayaç `rounded_to_zero_count` (Faz-6) |
| neden donduruldu | Kama bu hücrelerde kaybetmedi; T3a'da davranış KORUYUCU; aynı mekanizma iki rejimde zıt işaretli |
| tetik | YALNIZ gerçek kanıt: yetersiz onarım maddeleşmesine ATFEDİLEBİLİR zamanında-teslim zararı |
| gereken kanıt | nedenselliği KARARA BAĞLAYABİLEN karşı-olgusal/sınırlı test (ör. aynı tohumla r=0 ↔ r=1 kolları); "sayaç büyük" gerekçe DEĞİL |
| etkilenen hücreler | T1a, T1c, T2a, T2b, T3a, T7 (parity önerilen tüm hücreler) + T0 no-harm |
| YAPMA | `budget_ref_bps` / K / gate seviyesini "sayaca bakarak" ayarlamak; FEC'i kapatmak |

## 9.3 Dönüş-bant doygunluğu

| | |
|---|---|
| v1 durumu | T5 fiziksel asimetriyi uyguladı ama doyurmadı (dönüş kullanımı %3.5); doygun-dönüş davranışı zayıf doğrulanmış (LIM-004) |
| neden donduruldu | hücre sonuçlara bakılarak değiştirilemez (sözleşme); yönlü-gözlem şartı T6 ile karşılandı |
| tetik | asimetrik-bant ürün hedefi somutlaşınca |
| gereken kanıt | ACK-ağır geometri (küçük mesaj, yüksek oran) ya da dönüş kapasitesi ≤ ACK ihtiyacı olacak kadar dar hücre; runtime `ret_impair`/dönüş-bw tanısının asimetriyi GÖRDÜĞÜ kanıtı; N≥30 |
| etkilenen hücreler | yeni T5' hücresi (mevcut T5 aynen kalır, geriye dönük iddia değişmez) |
| YAPMA | T5'in mevcut sonucunu "asimetri kanıtı" diye alıntılamak |

## 9.4 FEC politika evrimi

Herhangi bir FEC politika değişikliği ŞUNLARI belirtmek zorundadır: etkilenen invaryant (INV-005,
006, 013), etkilenen profil alanları (`fec`, `structure_gate_level_permille`, `fec_loss_risk`,
`fec_structure_policy`), etkilenen T-hücreleri, T0 temiz-yol no-harm şartı (parity ≈ 0, tel ≤
quinn +%5), ve benchmark kimliğinin yeni sürüm olup olmadığı. Kapalı kollar (evidence-mass,
persistence yeniden-kalibrasyonu, repair sweep) kayıtlı sonuçlarıyla KAPALIDIR
(`reports/fec-*`); yeniden açmak yeni ön-kayıt ister.

## 9.5 Sürümleme kuralı

Donmuş Transport v1'de **davranış değiştiren** her değişiklik ya `transport-v1.x` ya da
`transport-v2` olur. Kimlik `client_core::runtime::TRANSPORT_V1_ARCHITECTURE`'da tek kaynaktır;
profil kimliği `TRANSPORT_V1_PROFILE_ID`. Donmuş ürün sözleşmesi değişirken `kama-v1-full`
adı KORUNAMAZ. Salt-dokümantasyon, salt-gözlem tanı, test ve CI değişiklikleri sürüm kırmaz.

## 9.6 §11 acil-durum yolunun saha-doğrulaması (Faz-6 bulgusu)

| | |
|---|---|
| v1 durumu | kablolu, birim-testli, ~~T0–T7'de hiç ateşlemedi (`recover_fwd_entries = 0`)~~ **[ERRATUM-2 (2026-10-18)]** T0–T7'de 15/330 koşuda `RecoverFwd` girişi var (T1a 2, T1b 7, T6 6), §11 MinimalParity emisyonu ayrı sayaçla ölçülmedi; T6'da health Dead iken `ack_confidence = 1000‰` |
| neden donduruldu | ~~Kama T6'yı ateşlemeden kazandı~~ **[ERRATUM-2 (2026-10-18)]** Kama T6'yı kazandı; T6'nın 6/30 koşusunda `RecoverFwd` girişi var, kazancın bu yola atfı ölçülmedi; yolu "zorla ateşletmek" yeni araştırma |
| tetik | çift-yön ≥ %80 kayıp ∧ sub-kbps rejimi ürün hedefi olursa |
| gereken kanıt | önce ölçüm: ack_confidence o köşede neden LOW'a inmiyor (PreConverge/idle'da 1000‰ zorlaması mı, tahminci mi); sonra T1b/T3b eşdeğeri N≥30 |
| YAPMA | hysteresis eşiklerini (300/700‰) ayarlamak; §11 istisnasını §8.5'e bağlamak/çözmek |

## 9.7 Telemetri şeması

`ActivationSummary` (manifest v1) donmuştur. `rounded_to_zero_count` ve `clamp`/`would_zero`
sayaçlarını manifeste taşımak `kama-bench-manifest/v2` işidir; şema sürümü artmadan alan
eklenmez (analizörler şemayı kontrol eder).

## 9.8 Kalıntı fuzzy çıktısı `FusionDecision.compress`

| | |
|---|---|
| v1 durumu | `fusion.rs:444` alan üretilir (`bw.low > 500‰ ∨ emergency`), HİÇ tüketilmez; zstd yok; TDD §8 AMENDMENT-4 ile sıkıştırma transport dışı |
| neden bırakıldı | kaldırmak `tv_fusion.rs:54,95` golden vektörlerini değiştirir → ADR-17 kararlılığı; davranış etkisi sıfır |
| tetik | ilk v1.x kod temizliği penceresi |
| gereken kanıt | alanın tüketicisiz olduğunun repo-genel taraması (bugün: yalnız fusion.rs + testler) |
| etkilenen hücreler | hiçbiri (çıktı hiçbir karara girmez) |
| YAPMA | alanı "kullanılabilir" diye bir tüketiciye BAĞLAMAK — sıkıştırma kararı TİP bilgisi ister (ses/video zaten sıkıştırılı), bu bilgi Application'dadır; transport opak birim taşır (TDD §8 AMENDMENT-4/6) |

## §22 — GELECEK DEĞİŞİKLİK POLİTİKASI (bağlayıcı)

Transport davranışını değiştiren HER değişiklik, PR açıklamasında ŞU ALTI beyanı yapar:

1. hangi donmuş invaryant (INVARIANTS.md) değişiyor,
2. hangi `KamaV1Profile` alanı değişiyor,
3. profil kimliği / sürüm kimliği değişiyor mu (`kama-v1-full` → ?),
4. hangi T0–T7 hücreleri GEÇERSİZ oluyor,
5. hangi doğrulama yeniden koşulacak (ADR-hostile-validation-contract ile),
6. değişiklik `transport-v1.x` mi kalıyor, `transport-v2` mi yaratıyor.

Hiçbir transport yaması doğrulanmış v1 mimarisini sessizce değiştirip aynı `kama-v1-full`
semantiğini iddia edemez. Bu beyanları taşımayan bir PR'ın performans/uyumluluk iddiası
geçersizdir.
