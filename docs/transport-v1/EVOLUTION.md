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

### §22 — `transport-v1.x-goster` gösterim beyanı (2026-11; ISEMRI-YOL3-M2)

E1 gösterim bayrakları (`RuntimeConfig.e1_flags`: (c) `e1_startup_cwnd`, (d) `e1_applimited_plateau`,
DrainRtprop `e1_drainrt_gate`, (e-dar) `e1_probe_keep`, `retx_first`; CC §4.1, CC-K5) için altı beyan:

1. **Donmuş invaryant:** varsayılanda değişmez (bayraklar kapalı = önceki kod yolu). Bayrak açıkken
   `INVARIANTS.md` "Beyanlı sınırlar (gösterim, 2026-11)" üç satırı (CC §4.4) geçerlidir.
2. **`KamaV1Profile` alanı:** değer değişmez; yeni alan `e1_flags`, varsayılan kapalı
   (`PRODUCT-PROFILE.md`, `product_v1_profile_is_pinned`).
3. **Kimlik:** ürün `kama-v1-full` / `transport-v1` değişmez; gösterim düğümü `transport-v1.x-goster`.
   *(2026-11-11 notu: ürün kimliği `kama-v1.1-full` / `transport-v1.1` oldu — aşağıdaki
   "`transport-v1.1` beyanı". Bu madde yazıldığı anın kaydıdır; gösterim düğümü kimliği değişmedi.)*
4. **Geçersiz T0–T7 hücresi:** varsayılanda yok; gösterim profili T0–T7'de doğrulanmamıştır.
5. **Yeniden koşulacak doğrulama:** mini-Ö (G4, 13 Kas), Aralık G-CC.
6. **Sürüm:** `transport-v1.x` kalır.

Gösterim seti = K1 = (c) + (d) + DrainRtprop + (e-dar); `retx_first` kapalı (2026-11-09 teşhisi; sahip v4
EK-A §2). Profil satırları: `PRODUCT-PROFILE.md` "`transport-v1.x-goster`" bölümü.
Teşhis manifesti: `docs/design/E1-TESHIS-G2.md`.

### §22 — `transport-v1.1` beyanı (2026-11-11)

Sahip kararı (2026-11-11): "#174 kimliği 'transport-v1.1' (EVOLUTION §9.5; aynı PR'da profil adı +
değişen davranış listesi)". Dondurma (PR #52, squash `53e0c5c6`, 2026-09-19) ile `6716eb8d` (#174)
arasında ürün varsayılan yolunda davranış değişti; `kama-v1-full` adı §9.5 gereği korunamaz.

| | önce | sonra | kaynak |
|---|---|---|---|
| profil kimliği (benchmark `arm`) | `kama-v1-full` | `kama-v1.1-full` | `TRANSPORT_V1_PROFILE_ID` |
| mimari kimliği | `transport-v1` | `transport-v1.1` | `TRANSPORT_V1_ARCHITECTURE` |

Sabitlerin adları değişmedi. Bench, tarihî manifestlerdeki `kama-v1-full` etiketini okumaya devam
eder (`bench::manifest::PRIOR_PROFILE_IDS`); yeni koşu onu yazmaz. `reports/` altındaki tarihî
çıktılar değiştirilmedi.

**Altı beyan:**

1. **Donmuş invaryant:** INV-RATE-1 (repo geneli, `CLAUDE.md` §4) 2026-11-11 yeni metniyle geçerli
   (#178): kayıp pacing hızını düşürmez; port motorunda o turdaki uçuş tavanını `recovery_window` ile
   sınırlar. Buna bağlı olarak INV-012'deki "loss_ppm rate'e girmez" ifadesi pacing hızı için geçerli,
   port motorunda uçuş tavanı için geçerli değil. INV-010'un `[IW]` 2 400 B referansı eski motorundur;
   port motorunda ilk uçuş `max(initial_window_bytes, 4·MTU)` = 4 800 B (#176). INV-008/009 kimlik
   değeri `kama-v1.1-full`. #174 `acked()` "ilk-ACK-kazanır" ifadesini daraltır: ilk ACK = motorun
   canlı kaydıyla eşleşen ilk ACK ya da RECOVERED (`docs/design/K4G-05b-RUNTIME-ENVANTER.md`).
   Diğer satırlar (INV-001..007, 011, 013..016) bu beyanda port motoruna karşı yeniden denetlenmedi.
2. **`KamaV1Profile` alanı:** `engine` eklendi (#175), değeri `CcPort` (#176). `e2a_flags` (#150),
   `e1_flags` (#161) ve `live_diag` (#136) eklendi, varsayılanları kapalı. `engine = CcPort` iken
   runtime `e2a_flags`'ı uygulamaz: port kurucusu E2a m1/m8/m6 ve PTO 2'yi kendisi açar; port ayrıca
   yeniden iletimi her zaman önce planlar ve retx durum koşulunu `≠ RecoverFwd` yapar (E1 `retx_first`
   ve `e1_probe_keep` ile aynı kural, bayraktan bağımsız). `RuntimeConfig` dışı sevk ayarları port
   motorunda: `TailRecoveryConfig::rfc9002_cc_port()` (`validated_v1` + `max_tail_probes = u32::MAX`),
   `RetxConfig::rfc9002_cc_port()` (`arq_retx_cap = 255`, ileri kopyaya devir yok),
   `Receiver::with_cc_port()` (sırasız varışta anında ACK, 25 ms zamanlayıcı; 8,39 s cadence tabanı ve
   R-kota yok). Diğer alan değerleri değişmedi (`transport_v1_freeze.rs`, `product_profile_v1.rs`).
3. **Kimlik:** `kama-v1-full` → `kama-v1.1-full`; `transport-v1` → `transport-v1.1`. Geri alma yolu
   (`engine: Legacy` / `KAMA_TRANSPORT_ENGINE=legacy`) v1.1 ürün profili değildir. Gösterim düğümü
   kimliği `transport-v1.x-goster` bu beyanla değişmedi.
4. **Geçersiz T0–T7 hücreleri:** tamamı (T0–T7). Bankadaki T0–T7 sonuçları `53e0c5c6` koduna aittir;
   ürün motoru (#176, #179), teyit sahipliği (#174), ACK kimliği (#106) ve alım yolu dağıtımı
   (#114, #116) o koddan sonra değişti. Port motoru yalnız sim kapısında (`cc_port_sim`) ve Ö-CC
   hücrelerinde ölçüldü; T0–T7 sözleşmesiyle ölçülmedi.
5. **Yeniden koşulacak doğrulama:** ADR-hostile-validation-contract ile T0 (temiz yol regresyon
   kapısı) ve T1–T7, N ≥ 30, ön-kayıtlı, etiket `kama-v1.1-full`; #174'ün istediği ürün kolu kesinti
   hücreleri ve INT-02 60 s kesinti hücresi. Bu beyanda hiçbiri koşulmadı.
6. **Sürüm:** `transport-v1.x` (v1.1), v2 değil: katman grafiği (App → Transport → Crypto → UDP) ve
   INV-ENGINE-SOC değişmedi; AMENDMENT-11'in tel anlamı değişikliği kendi beyanında `transport-v1.x`.

**Değişen davranış — ürün varsayılan yolu** (kaynak: `git log --first-parent 53e0c5c6..6716eb8d --
crates/adaptive-transport crates/client-core/src crates/kama-node/src`; her commit'in farkı okundu):

| PR | squash | davranış | varsayılan | geri alma |
|---|---|---|---|---|
| #106 | `995e037e` | ARQ/ACK kimliği = `full_wire_pn`: göndericide PN → motor seq çeviri tablosu (`PN_SEQ_WINDOW = 2^16`), ACK_V2 aralıkları PN uzayında, alıcı ara çerçevenin u16 seq alanını okumaz; noise-wire `Control::Ack/Sack` (0x10/0x11) RESERVED → ret (AMENDMENT-11; öncesindeki uçla birlikte çalışmaz) | açık | yok |
| #114 | `5dbd0b80` | Bağlam-önce dağıtım: açılan datagramın `ProtectionContext`'i yük çözülmeden yönlendirilir; HubChannel yalnız sayılır, beklenmeyen Session düşer (`ctx_mismatch_drops`), sunucuda NotReady oturuma ACK'siz düşüş (INV-ACK-DELIVER-1) | açık | yok |
| #116 | `b58717b7` | Sunucu eş yuvası (adres, `SessionHandle`) anahtarlı; yalnız beklenen kümede Ready + AEAD-geçerli datagramla açılır, bağı veri datagramıyla değişmez, `idle_hold` ya da oturum beklenen kümeden çıkınca emekli; `Reorder` oturuma anahtarlı | açık | yok |
| #170 | `23bffb7f` | `loss::time_threshold_lost` yüklemi `>` → `>=` (iki motorda ortak; vade anında kayıp ilanı). Port motoru bu PR'da eklendi, varsayılan değildi | açık | yok |
| #176 | `399b573f` | Ürün motoru = quinn-proto 0.11.18 BBR + RFC 9002 portu (`RuntimeConfig::engine = CcPort`): uçuş penceresi kapısı, BBR `pacing_rate` ile pacer, PTO tavansız ve pencereden muaf, retx önce ve tavansız, zaman eşiği 9/8·max(srtt, latest), alıcıda anında ACK + 25 ms; ilk uçuş `max(initial_window_bytes, 4·MTU)` = 4 800 B | açık | `KAMA_TRANSPORT_ENGINE=legacy` (kama-node, bench) ya da `engine: Legacy`; iki uç aynı motoru kurmalı |
| #179 | `2d0a6bc9` | U9: port motorunda Startup'ta da kayıp anında uçuş tavanı = uçuş + ACK (Linux BBR v1 paket koruması) | açık (port) | eski motor (yukarıdaki anahtar) |
| #174 | `6716eb8d` | Runtime payload sahipliği ve app teyidi yalnız motorun teyit çıktısından (`Sender::take_confirmed_units`); gövdesiz motor kararı tele çıkmaz (`stale_skipped`); motorda teyitli birimin `pending_lost`/`dup_queue` kayıtları iptal, uçuştaki diğer kopyaları kararsız işaretlenip uçuşta kalır (D1–D14) | açık (iki motor) | yok |

**Varsayılan dışı** (bayrak/yapılandırma arkasında, ürün profilinde kapalı):

| PR | squash | davranış | not |
|---|---|---|---|
| #150 | `a8721540` | E2a bayrakları (`RuntimeConfig.e2a_flags`): m1 zaman eşiği, m8 RTT kuralı, m6 ilk pencere, PTO 2 | eski motorda kapalı; port kurucusu aynı kuralları kendisi açar (#170/#176) |
| #161 | `0d683525` | E1 gösterim bayrakları + `retx_first` (`RuntimeConfig.e1_flags`) | kapalı; port retx-önce ve retx durum kuralını bayraktan bağımsız uygular |
| #163 | `b1ef5e55` | İnce şeritler (`ClientTransportConfig::lanes = None`); S1 v2 `send(.., class)` | şerit yokken sınıf etkisiz |
| #175 | `eef86723` | `RuntimeConfig::engine` + `KAMA_TRANSPORT_ENGINE` seçimi | bu PR'da varsayılan `Legacy`; #176 çevirdi |

**Eklenen yüzey** (mevcut `ClientEndpoint`/`ServerEndpoint` karar yolunu değiştirmeyen yeni ürün API'si
ya da kipi): #109 `47c17534` (S1 `ClientTransport`, direct-mode), #125 `0f1488a7` (`kama-node`
bileşim kökü), #131 `5e9d9fc2` (`send_signaling`); hub kipi (cihaz dosyasında hub varsa) #158
`b733c008`, #165 `c0d8975c`, #160 `8118413b`, #167 `1de8fe43`, #169 `c0547639`, #173 `55f9018c`.

**Sürüm kırmayan** (test, salt-gözlem tanı, ölçüm düzeneği, tip taşıma, bağlanmamış codec): 9 PR —
#77, #81, #87, #89, #99, #130, #136, #149, #171.

Kapsam dışı: listelenen üç yol dışındaki crate'ler (ör. `kama-noise-wire`, `kama-contracts`,
`hub-server`) bu listede yalnız yukarıdaki commit'lerin içinden göründükleri kadarıyla yer alır.
