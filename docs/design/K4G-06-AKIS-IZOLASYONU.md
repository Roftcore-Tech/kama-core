# K4G-06 — Akış izolasyonu ve teslim sözleşmesi — tasarım notu rev3 (ADIM 1: sözleşme/arayüz taslağı + ayırt edici test planı)

## 0. Künye

- **Kart / dal / durum:** K4G-06 · `k4g/06-tasarim` (taban main `1b1425d2`) · **rev3 TASLAK**. Kod yok, ölçüm yok; deney dalı (`k4g/06-deney`) açılmadı.
- **Sürümler:** rev1 = `2656953c` (inceleme `k4g06/inceleme-wf_4a961b72.json`) · rev2 = `9cda1fab` (doğrulama `k4g06/rev2-dogrulama-wf_86a0d33e.json`; 31 kontrol, 26 yeni bulgu) · rev3 = bu sürüm. Her iki turun bütün bulgularının izlemesi Ek F'de. rev3'ün ana değişiklikleri: KARAR-1 için tarihli ek açıkça gerekli ilan edildi; FEC blok sınırı onarım çerçevesine taşındı (blok kimliği yeniden oturum boyu monoton); transport kontrolü ayrı kalıcı akışa alındı (TCTRL bayrağı kalktı); VO'da `REL` bayrağı; kredi canlılık kuralları tamamlandı; seçicide rezerv hiyerarşisi.
- **Otorite sırası:** CEVAP 15 EK-1 > CEVAP 15 v3 > 14 > …; donmuş belgeler kendi değişiklik kuralıyla (PoC-KAPSAM-001 yalnız sahibin tarihli ekiyle; ccr/noise-wire/transport-tdd yalnız sahip kararıyla). Sahibin iki ek notu bağlayıcı. İnceleme ve doğrulama kayıtları kanıt girdisidir.
- **Atıf biçimi:** `SHA:dosya:satır`. Kısa dosya adları: `runtime.rs`/`transport.rs`/`wire.rs`/`batch.rs` = `crates/client-core/src/`; `sender.rs`/`arq.rs` = `crates/adaptive-transport/src/`; `s1.rs` = `crates/kama-contracts/src/`; `ptt.rs`/`file_transfer.rs`/`messaging.rs` = `crates/kama-app/src/`; `node.rs` = `crates/kama-node/src/`. Belgeler tam yolla.
- **SHA'lar (origin'de, 2026-10-25):** `0c22a07d` = `k4g/05c-kod` (05b rev5 `35a02fcf` + 05c) · `37b78aad` = `k4g/05b-retx-fix`, **05b rev6** (B′, K3, DUR-40; Ek A A.4 rev6, Ek B.6 (g)(h)) — `0c22a07d` rev6'yı İÇERMEZ · `1b1425d2` = main · `cf270a6c` = 05d · `e5514e7a` = 05f rev1 · `0b216f07` = 05g origin ucu (yerel `c9c2ea22` YEREL, dayanak değil).
- **Ek A sha256:** EK_A_SHA_PLACEHOLDER
- **Etiketler:** [olgu] koddan · [hesap] aritmetik, ölçülmedi · [çıkarım] test edilmedi · [türetim] koddan önce yazılan sınır · [CAL] deney başlangıç değeri.

## Karar özeti

**Şimdi istenen onay:** yalnız §14 deney kapsamı, deneyden ÖNCE gereken iki yön (DUR-06-5, DUR-06-2) ve deney kapsam daraltması (DUR-06-20). Ürün kodu, merge ya da donmuş metin değişikliği istenmiyor.

1. **Birim çerçevesi (DUR-06-5, öneri B): KARAR-1'in canonical `BatchPayload` alt mesaj çerçevesini korur, ama KARAR-1'e ve ccr §35.14'e tarihli sahip EKİ GEREKTİRİR** (rev2'nin "KARAR-1 ile çelişmeyen tek seçenek" iddiası geri çekildi). Ek gereken dört nokta: v0 kabuk tip baytı (ccr'de yok), alt mesaj akış başlığı `flags · flow_id u16 · flow_seq u32`, S1 kipinde `submsg_count ∈ {0, 1}`, açık parçalayıcı (0x05) istisnası. Örtük kip de FLOW_BATCH'e geçer; ürün runtime'ında tek veri çerçevesi tanımı kalır. Başlık 10 B (kayıt ≤ 239 B) / 12 B; bugün 11 B [hesap].
2. **FEC kimliği:** `block_id` bugünkü gibi oturum + yön boyu monoton (§2.6 ID-SCOPE değişmez); blok tek akışın ardışık birimlerinden oluşur; blok sınırı onarım çerçevesinde taşınır (`first_seq` u32, onarım sembolü başına +4 B). Kaynak sembol öneki S1 kipinde kendini tanıtır: `message_id := flow_seq`, `chunk_offset := flow_id`, `source_flags`'ta FIN ve REL. Kısmi blok kapanışı serbesttir, blok kimliği yeniden kullanılmaz.
3. **S7 (b): motor içinde tek, saf seçici; eklemeli API** (`Event::AppDataTagged`; etiketsiz = tek akış = rev6). Rezerv hiyerarşisi: katman 0/1, kova payları içinde katman-2 rezervinin önüne geçer; K1 (a) katman 2 içinde aynen. S7(a)'nın reddi B2'ye bağlı (TEXT'in KENDİ kurtarma işi; iki taban; ön-kayıtlı).
4. **Kredi ve kontrol:** akış başına mutlak artan sınırlar; sayım sınırları (`bk_flow_limit`, `max_rel`) aynı FLOW_CREDIT'te; BLOCKED iki boyutlu, çok girdili, etkin sınır artana dek tekrarlanır; alıcı her BLOCKED'e güncel sınırı ≤ 1 / T_min ile yeniden ilan eder. Transport kontrolü (akış 0) transportun kendi tükettiği şerit, uygulama kontrolü (akış 1) ayrı. Kabul kapısı yalnız `capacity_defect = 0` (sahibin notu 2).
5. **PTT ve S5:** canlı PTT (C15 v3 §9.3 / S5) PoC-KAPSAM-001'in PTT satırını, K8 "sıfır bozuk hece" ölçütünü ve S5 dondurmasını (`:34`, K6 `:54`) değiştirir; hepsi tek bir tarihli sahip ekiyle (EK-4, DUR-06-18) olur. Live'a bağlı DUR'lar (06-1, 06-6, 06-11 Live kısmı, 06-23) DUR-06-18 (a)'ya koşulludur.

**Deneyden ÖNCE gerekenler:** DUR-06-5 ve DUR-06-2 yön onayı (KARAR-1 ekinin gerekli olduğu kabulü dahil) · DUR-06-20 (deney kodunda FEC kapalı; B2 FEC kolu yalnız tabanda) · §14 kapsamı (≈ 5,5–7 iş günü; tahmin).

## 1. Amaç ve değişmeyenler

**Amaç:** tek güvenli oturum altında bağımsız mantıksal akışlar — akış başına sıra, WMM-örnekli sınıf önceliği (PTT → AC_VO, TEXT → AC_BE, FILE → AC_BK; AC_VI yok), alıcının önceden ilan ettiği kredi, ortak ve adil gönderim bütçesi. Dosyanın eksik birimi ya da durmuş tüketicisi TEXT/PTT'yi bekletmez; olağan kapasite durması ACK kesilerek gösterilmez (HER BİT DÜŞMAN). Teslim sözleşmesi revizyonudur (C15 v3 §9.1).

**Değişmeyenler:**
- **D1 Hub zarfı:** 22 B legacy / 26 B extended dış zarf ve hub-server codec'i değişmez [`1b1425d2:crates/kama-noise-wire/src/envelope.rs:80-83`]. Akış alanları AEAD altındaki iç çerçevededir ve yalnız `CtxRoute::Session` kolunda ayrıştırılır; diğer kollarda yüke dokunulmaz [`0c22a07d:runtime.rs:1510-1516`, :2320-2335].
- **D2 Noise:** sayaç/nonce akış başına sıfırlanmaz; yön başına tek PN uzayı; ARQ kimliği PN [`0c22a07d:runtime.rs:2028-2033`].
- **D3 Tek sonlandırma çıktısı:** payload, birim başına oturumda en çok bir kez, yalnız tek bir sonlandırma çıktısında düşer: motora verilmiş birim için motorun `take_finished_units`'i (Teyit | İptal), motora hiç verilmemiş birim için runtime'ın kendi iptal çıktısı — bir birim için yalnız biri (bugün: teyit çıktısı, `0c22a07d:runtime.rs:2019-2024`; §8.3, A.16). 05c INV-ACK-DELIVER-1 akış başına; 05g sahte `Ready` kapanışı; 05d "canlı RECEIPT silinmez".
- **D4 INV-ENGINE-SOC:** transport payload'u ve AMP kaydını çözmez; yalnız generic hizmet etiketini ve alt mesaj bayraklarını görür.
- **D5 INV-RATE-1:** tek pacer; kayıp hızı düşürmez. **D6 INV-FEC-1** aynen. **D7** ADR-05, ADR-17.
- **Kapsam dışı (gerekçeli):** Wi-Fi/DSCP; hub rölesi (direct-mode) — hub kuyruğu gereksinimi S4'e (A.31); media plane.

## 2. Bugün neden birbirlerini bekliyorlar [olgu → çıkarım]

- **Alıcı:** oturum başına tek `Reorder` [`0c22a07d:transport.rs:370-419`]; düğüm geneli tek `inbox` (1024, :118, :449) ve tek teslim kanalı (`runtime.rs:3053`, :271); kabul penceresi oturum geneli `max_inflight_units` [`runtime.rs:3447`, :3480-3482].
- **Gönderen:** tek offer FIFO'su (1024, `runtime.rs:1705`, :270); oturum geneli mesafe kapısı [:2511, :2575-2581, :2608].
- **Motor:** `OpaqueUnit{id, size}` [`0c22a07d:sender.rs:75-80`]; DATA FIFO [:2492, :7225-7247]; ARQ sırası kayıp ilanı anı [:3846-3849, `arq.rs:124-127`]; kopya başı rev5'te en küçük kimlik [`0c22a07d:sender.rs:7103-7110`], rev6'da RTO_MAX yaş tavanına dek en küçük kimlik (DUR-40) [`37b78aad:sender.rs:7243-7265`]; parite FIFO [`0c22a07d:sender.rs:1787`]; DATA-önce + karşılık hakları [`37b78aad:sender.rs:4684-4717`, :1405-1440, :6333-6350]. Motor blok kimliğini yalnız eşitlikle kullanır [olgu: `37b78aad:sender.rs:5690`, :5729-5735, :6170-6173].
- **FEC blokları trafikleri bağlar:** blok = `unit_id / K` [`0c22a07d:runtime.rs:2010-2014`]; kurtarılan birim = `block·K + esi` [:2379-2382]; kısmi blok bugün yalnız akış sonunda kapanır [:2763-2811]; gönderen `repair_store` ve alıcı `slot.blocks` en küçük anahtarı (`pop_first`) tahliye eder [:2715-2716, :3894-3895].
- **Süre yalnız sunumda:** `DeadlineBounded` yalnız `send` anında [`0c22a07d:transport.rs:1297-1305`]; ARQ süresi dolanı düşürmez [`arq.rs:92-94`].
- **kama-app:** PTT üç kaydı da `Reliable` [`0c22a07d:ptt.rs:18-24`]; dosya pompası eş başına yalnız en eski aktarım [`file_transfer.rs:862-881`]; alındı kuyruğu küresel 4, taşmada en eskiyi düşürür [`file_transfer.rs:286-291`, :1216-1224]; reddedilen FILE_BEGIN de FILE_RECEIPT(rejected) üretir [:1018-1034]; tek okuyucu zincir [`messaging.rs:652-660`; `node.rs:465-485`].
- **Sonuç [çıkarım]:** kayıp dosya birimi alıcıda TEXT/PTT'yi tutar; gönderende yeni TEXT FIFO'larda ve motor kurtarma kuyruklarında eski dosya işinin arkasındadır (`cf270a6c:docs/design/K4G-05d-RECEIPT.md:88`); kapasite dolunca ACK kesilir.

## 3. Akış modeli

**Akış** = oturum içinde tek yönlü mantıksal kanal; açan (gönderen) numaralar.

**Kimlik uzayı** (`flow_id` u16, yön başına):
- **Kalıcı akışlar** (oturum boyu, örtük açık, ayrılmış): `0` = **transport kontrol şeridi** (FLOW_RESET; tüketicisi transporttur, uygulamaya teslim yok) · `1` = **uygulama kontrol şeridi** (RECEIPT, FILE_RECEIPT, FILE_ABORT) · `2` = TEXT · `3` = SIGNALING · `4` = VO (`Live`; konuşmalar AMP `stream_id` ile çoğullanır). Kalıcı akışa `close_flow`/`reset_flow` → `UnknownFlow` (kalıcı akış kapatılamaz; yeni hata varyantı gerekmez).
- **Dinamik akışlar** (`5…65535`): yalnız BK (dosya aktarımı başına). Gönderen kimliği ilk biriminin kabul edildiği anda, boşluksuz ayırır. Alıcı `max_seen` + açık küme tutar. **Örtük açma:** k > `max_seen` gelince [`max_seen` + 1, k] aralığı açık kümeye girer (geç gelen ilk birimleri bekler). **Kapalılık:** k ≤ `max_seen` ∧ k ∉ açık küme. **Sayım sınırı:** gönderen yalnız k < `bk_flow_limit` kullanır; `bk_flow_limit` = `bk_flow_limit0` + çözümlenen akış sayısı (çözümlenme = FIN'e dek teslim ya da RESET işlendi ∧ o akışın FILE_RECEIPT yuvası boşaldı, §7.6). Böylece açık küme ≤ `bk_flow_limit0`; tutulan bir aktarım yalnız kendi yuvasını tutar, diğerleri ilerler. Kapalı akışın geç birimi ACK'lenip atılır (`late_after_close`).
- Tükenme [hesap]: 65 531 dosya aktarımı/oturum; tükenince yeni aktarım tipli hata, `RenewRequired` (DUR-06-5 (iii)).

**Üç kimlik:** PN (zarf; ACK, nonce) · birim kimliği (`OpaqueUnit.id`; gönderen runtime + motor) · (`flow_id`, `flow_seq`) (iç çerçeve; teslim yeri, sıra, kabul, dedup, kredi) — noise-wire §3'ün (4) "message/chunk-id" kimliğinin S1 kipindeki anlamı (A.10). Gönderen runtime (akış, sıra) ↔ birim ters dizinini tutar (≤ store).

**Birim çerçevesi — öneri B (DUR-06-5):**
`FLOW_BATCH = [tip u8 (v0 kabuk çoğullayıcısı)] ‖ BatchPayload { submsg_count u8 ∈ {0, 1} ; BatchSubMessage { flags u8 · flow_id u16_be · flow_seq u32_be · len_mode u8 [· length u16_be] · payload = AMP kaydı } }` [`1b1425d2:specs/ccr-format-spec.md:3017-3035`; `0c22a07d:batch.rs:1-29`].
- `flags`: FIN · REL (yalnız VO: `Reliable` birim) · ayrılmış (≠ 0 → ret). Sınıf ve kip `flow_id`'den çıkar.
- 10 B (kayıt ≤ 239 B) / 12 B [hesap]. `submsg_count ≥ 2` → fail-closed ret + `batch_multi_rejected` (ccr'nin çoklu alt mesajı S1 kipinde kısıtlanır → KARAR-1/ccr eki).
- **Store-miss** (05b bekçisi; motor teyitli/iptal birime karar üretti): v0 kabukta `[tip][boş BatchPayload]` = 2 B (BatchPayload kısmı ccr K3'ün canonical 1 B'si); alıcı PN'i kabul kapısına girmeden ACK'ler, teslim yok, `stale_decisions` sayılır.
- **Örtük kip** (bench, S1-dışı) de FLOW_BATCH'e (tek kalıcı akış 2) geçer; 0x00 emekli. 0x05 yalnız açık opt-in parçalayıcıda (S1'de yasak, `0c22a07d:transport.rs:508`) beyanlı istisna olarak kalır.
- **S2 eşlemesi:** extended yolda FLOW_BATCH için yeni `inner_kind` (0x1D+, K-WIRE-CTRL-1); legacy yolda ccr'de ayrım baytı yoktur → KARAR-1'in açık "ayrım" parçası (`batch.rs:28-29`), tarihli ekle karara bağlanır.

**FEC kimliği (öneri B-iii, DUR-06-5 (v)):**
- `block_id` bugünkü gibi oturum + yön boyu monoton u32 (§2.6 ID-SCOPE değişmez); blok **tek akışın** ardışık birimlerinden oluşur; akış başına biriktirici; kısmi blok T_block [CAL] dolunca kapanır, sonraki birim yeni `block_id` alır (yeniden kullanım yok).
- v0 onarım çerçevesi: ölü `seq u16` alanı → `flow_id`; ek `first_seq u32` (blok ilk biriminin `flow_seq`'i) → onarım sembolü başına +4 B [hesap]. `esi = flow_seq − first_seq`.
- Kaynak sembol öneki (iki uçta türetilir, kurtarılan sembolde okunur): `message_id := flow_seq`, `chunk_offset := flow_id` (S1'de parçalama yok, alan boş), `actual_payload_len`, `source_flags = FINAL | FIN | REL` → kurtarılan birim kendini tanıtır (akış, sıra, FIN, REL). v0 alt mesaj bayrakları ile kurtarılan öneğin bayrakları çelişirse fail-closed + `flag_mismatch`.
- Alıcı: kaynak sembolleri akış başına sınırlı halkada tutar, blok üyeliğini onarım çerçevesinden öğrenir; RECOVERED (`block_id`, `esi`) → gönderenin blok tablosu (`block_id` → akış, `first_seq`) → ters dizin → birim. Gönderen `repair_store` ve alıcı `slot.blocks` **sınıf başına bölünür**; `block_id` monoton olduğundan `pop_first` sınıf içinde en eskiyi tahliye eder.

**Teslim kipleri:** `Ordered` (akış içinde sıralı, boşluk atlanmaz) · `Live` (VO; akış içinde sırasız, `flow_seq` uygulamaya verilir). VO'da: demet = `DeadlineBounded`, REL birim (PTT_OPEN/CLOSE) = `Reliable`. Alıcı dedup penceresi W_LIVE: pencere içi her `flow_seq` tam bir kez; **gönderen kapısı:** en eski teyitsiz REL birimiyle yeni `flow_seq` arası < W_LIVE (kesintide yeni VO birimi beklemez, süresi dolar); böylece REL birim hiçbir zaman pencere altına düşmez ve "tam bir kez" yapısal olarak tutar.

**Oturum sınırı:** akışlar oturuma aittir; yenileme/supersession akış taşımaz (S1 K7); bağlam-önce dağıtım eski oturumun DATA/ACK/kredisini yeni oturuma ulaştırmaz [`0c22a07d:transport.rs:56-62`].

## 4. Sınıflar ve kontrol

| Trafik | Sınıf | Akış | Kip / politika |
|---|---|---|---|
| FLOW_RESET | CTRL (transport) | 0 | `Ordered`, `Reliable`; transport kabul sırasıyla tüketir |
| RECEIPT, FILE_RECEIPT, FILE_ABORT | CTRL (uygulama) | 1 | `Ordered`, `Reliable`, birim boyu tavanlı |
| TEXT | BE (AC_BE) | 2 | `Ordered`, `Reliable` |
| SIGNALING_BLOB | BE | 3 | `Ordered`, `Reliable` (DUR-06-15) |
| Canlı PTT | VO (AC_VO) | 4 | `Live`; demet `DeadlineBounded`, OPEN/CLOSE `Reliable` + REL (DUR-06-18'e bağlı) |
| Dosya | BK (AC_BK) | 5… | `Ordered`, `Reliable` |
| FLOW_CREDIT | CTRL | — (alıcının ACK yolu) | tetikli, idempotent, çok girdili |
| FLOW_BLOCKED | CTRL | — (`TransmitKind::Control`, izlenmez) | çok girdili, geri çekilmeli tekrar |

WMM adları niyet bildirir; garanti §5 seçicisinden gelir.

**Kontrol katmanı (S6):** VO'nun altında tek sıra değil, katman 0.
- **Tavan:** katman 0'ın bütün gönderen işi (akış 0 ve 1'in DATA/ARQ/kopya işi, BLOCKED) kontrol kovasındadır: ρ_C·r(t), kova b_C ≥ `max_control_unit_bytes` + mühür [CAL]. **Katman 0 içi sıra:** vadesi gelmiş BLOCKED (kendi zamanlayıcısıyla seyrek, tek çok girdili çerçeve) → akış 0 → akış 1.
- **Koşullu asgari hak:** uygun iş + kovada token → katman 0 önce; backlog'luyken ≥ ρ_C·r.
- **Veri kredisinden bağımsızlık:** akış 0 kredisi (W_TCTRL) transportun kendi tüketimiyle, akış 1 kredisi (W_CTRL) uygulama tüketimiyle ilerler; ikisi de veri kredisinden ayrıdır; kredi ilanı ACK yolunda gider, kredi tüketmez (B10 (c)).
- **Envanter, hizmet hakkı, toplam maliyet:** Ek D, DUR-06-3.

## 5. İki düzeyli seçici (S7 önerisi; DUR-38 ile ortak)

**Sözleşme [DUR-06-2 (b2)]:** `ServiceTag { tier: u8, flow: u32 (opak), expires_ns: Option<u64> }`; eklemeli `Event::AppDataTagged(OpaqueUnit, ServiceTag)`; etiketsiz `AppData` = tek akış, katman 2 = rev6 birebir (149 literal ve pinler değişmez). Seçim motor içinde tek, saf, deterministik fonksiyon. `TransmitKind::Control` yalnız FLOW_BLOCKED için (uçuşta izlenmez, PN tablosuna girmez).

**Düzey 1 — hangi akış:** katman 0 CTRL (kova) → katman 1 VO (kova: min(VO profil hızı, ρ_V,max·r(t)); kesilen pay `vo_rate_deficit`, sürekli açıkta tipli "desteklenmeyen profil" olayı) → katman 2 BE/BK bayt kuantumlu DRR (sınıf ağırlığı akış sayısından bağımsız, Q_BE : Q_BK = 3 : 1 [CAL]; sınıf içinde akışlar arası DRR).

**Rezerv hiyerarşisi [öneri; DUR-06-2 (i)]:** her katmanın en çok bir rezervi vardır. Katman 0 ya da 1'in uygun başı token bulamazsa o katmanın rezervi kurulur ve alt katmanların rezervinin **önüne geçer**; katman 0/1 tüketimi kova paylarıyla sınırlı olduğundan katman-2 rezervinin ömrü sonlu kalır. **05b K1 (a) katman 2 içinde aynen:** katman-2 rezerv sahibinin başı dışındaki her katman-2 tüketicisi (aynı katmanın kuyruk-probu ve paritesi dahil) rezervin altına inemez. Sonuç: VO ve kontrol, BK başının token birikimini beklemez (bekledikleri yalnız iletimdeki paket, §11). Haklar (rev6 K-2) **(akış, sınıf)** başına; K_ARQ_SHARE payı **akışın hizmet fırsatını** sayar.
**Akışsız emisyonlar:** CS/bootstrap probu ve kuruluş probu katman 0'a (kova) yazılır; kuyruk-probu, parite ve kopya ait oldukları birimin/bloğun akışının katmanındadır.
**Düzey 2 — akış içi iş:** 05b rev6 K-0…K-5 (`37b78aad:docs/design/K4G-05b-RETX-FIX.md:545-575`). Tek akışta rev6 T1/T2 regresyonu (B17).
**PRE_CONVERGE (K-0):** düzey 1 katman sırası uygulanır; düzey 2'de v1 sırası ve §3.3-B takvimi; hak ve rezerv yok (beyanlı; B14 kolu).

**Aç bırakılmama — ispat taslağı [türetim; sonuç "ölçüldü" diye yazılır].** r(t) = pacer hızı; pacer dışı ters yön trafik hariç (B5'te ayrıca ölçülür):
1. τ aralığında katman 0 + 1 + akışsız prob ≤ ∫(ρ_C + ρ_V)·r + b_C + b_V + P_pf; ρ_C + ρ_V,max < 1 her an (VO kovası r(t)'ye bağlı).
2. Katman 2'de backlog'lu sınıf c turda ≥ Q_c; tur ≤ Σ_c (Q_c + S_max); akış f, n_c akışlı sınıfta ≥ Q_c / n_c.
3. P_f ≤ n_c·[Σ_c (Q_c + S_max) + b_C + b_V + P_pf] / ((1 − ρ_C − ρ_V,max)·r) + τ (rezerv hiyerarşisi bu paydayla tutarlıdır).
4. **T_birim (akış içi):** rev6 T_birim = max(H_maks, A) + A + N_live·P_c + τ (`37b78aad:…RETX-FIX.md:571`); P_c = rev6 T_max(kopya) ifadesinde R → r_f = (Q_c / n_c) / P_f, τ → P_f.
5. **Kontrol ve VO:** uygun başı ≤ (S_iletimde + S_baş)/r + kendi kova dolumu + τ içinde çıkar (katman-2 rezerv beklemesi YOK).
- Örnek [hesap]: r = 300 B/s, VO yok, ρ_C = 0,1, bir BE + bir BK, Q_BE = 3150 B, Q_BK = 1050 B, S_max = 1050 B → tur ≈ 6300 B / 270 B/s ≈ 23,3 s.

## 6. Kuyruk derinliği denetimi (S7) ve S7(a) hükmü

| Katman | Bugün [olgu] | TEXT'in önünde kalabilecek dosya işi [hesap/çıkarım] | K4G-06 |
|---|---|---|---|
| kama-app dosya pompası | eş başına en eski aktarım (`0c22a07d:file_transfer.rs:862-881`) | ikinci dosya birincinin arkasında | akış başına hizmet (A.21) |
| S1 `send` → offer mpsc | tek FIFO 1024 | 1024 birim; 31 B/s'de ≈ 9,6 sa | akış bütçesi dışında birim kabul edilmez; yerel tampon akış başına, offer FIFO'su dışında |
| runtime store + mesafe kapısı | oturum geneli (`runtime.rs:2575-2581`) | eski dosya birimi yeni alımı durdurabilir | S1 kipinde mesafe kapısı kalkar → akış kredisi; store ≤ min(Σ kredi, yerel tavan) |
| motor `tx_queue` | FIFO | ≤ 4096 DATA | katman/akış başına kuyruk |
| motor `pending_lost` | kayıp ilanı sırası | önce kaybolan dosya birimi önce | düzey 1 akış seçer; akış içi `arq.rs` sırası aynen |
| motor `dup_queue` | rev5 en küçük kimlik; rev6 yaş tavanına dek en küçük kimlik | eski dosya kopyası önce | akış başına baş; düzey 1 |
| motor `pending_fec` / `repair_store` | FIFO; oturum geneli blok; `pop_first` | karışık blok paritesi; BK seli TEXT paritesini tahliye eder | blok akış başına; depo sınıf başına |
| pacer / soket / darboğaz | iletimdeki paket kesilemez | S/r (31 B/s'de 1050 B ≈ 33,9 s; 300 B/s'de 3,5 s) | fiziksel sınır (§11) |
| hub (INT-03) | S4'te kuyruk tanımsız (`1b1425d2:docs/seams/KM-SEAM-TH-001-TASLAK.md:63`) | hub sınıf göremez | gereksinim: sığ, sınırlı handle kuyruğu (A.31) |
| alıcı teslim kanalı | düğüm geneli FIFO 1024 | dolunca ACK'siz ret | kapasite ≥ Σ taahhüt (yapısal yer) |
| alıcı `Reorder` / `inbox` | oturum başına / düğüm geneli | dosya boşluğu ve durmuş tüketici herkesi tutar | (oturum, akış) başına; akış başına hazır kuyruk + `hold_flow` |
| kama-app olay zinciri | tek okuyucu | yavaş dosya işleyici herkesi tutar | dosya tüketicisi akışını tutar; zincir sürer |

**S7(a) hükmü [çıkarım; B2 ile sınanır]:** sığ besleme motora önceden kabul edilmiş ARQ/kopya/parite işini sıralamaz; yeni TEXT'in ilk emisyonunu bekletmez ama TEXT'in kendi kurtarma işini önce ilan edilmiş N_u dosya kaydının arkasına koyar. B2 tabanında kırmızı beklenir (ön-kayıtlı). **Öneri: S7(a) reddedilir, S7(b).**

## 7. Alıcı kredileri

**7.1 Sınırlar (hepsi mutlak, artan; FLOW_CREDIT'in sabit boylu girdilerinde):**
- **Akış kredisi:** `max_seq` (u32) ve `max_bytes` (u32; dosya akışında tavan `MAX_REASSEMBLY_BYTES` 64 MiB, `0c22a07d:file_transfer.rs:257`; aşımda tipli hata). Yeniden gönderim/kopya/FEC kurtarımı aynı `flow_seq` → ikinci kredi yok. Açılışta FIN için 1 seq'lik yuva ayrılır (`max_seq + 1`; boş FIN birimi tampon tutmaz).
- **Sayım sınırları:** `bk_flow_limit` (u16; §3) ve VO için `max_rel` (u32; REL birim sayısı). İlerleme: `bk_flow_limit` çözümlenen BK akışı başına +1; `max_rel` uygulamaya verilen REL birim başına +1.
- **VO demetleri** `flow_seq` tüketir ama seq kredisinden muaftır; kabul politikası DUR-06-1 (b) (P_VO havuzu; yalnız REL bayrağı olmayan birim düşebilir).

**7.2 Havuz ve taahhüt:** oturum başına sınıf havuzları P_TCTRL, P_CTRL, P_BE, P_VO, P_BK (birim + bayt); taahhüt = Σ_f (`max_seq_f` + FIN yuvası − uygulamaya/transporta verilmiş_f) ≤ havuz; BK kredisi yalnız P_BK'den. İki RuntimeConfig değişmezi: Σ W + kontrol tavanı < `max_inflight_units`; `delivery_queue` ≥ `max_peers` × Σ havuz. [hesap] [CAL] değerleriyle oturum başına ≈ 360 birim / ≈ 321 KiB → `max_peers` 64'te ≈ 23 040 birim / ≈ 20 MiB (bugünkü `delivery_queue` 1024, `runtime.rs:271`); bu bellek SR-AUDIT açısından kabul edilemez büyüklüktedir → deneyde etkin S1 oturumu ≤ 8 (≈ 2 880 birim / ≈ 2,5 MiB); normatif sayı DUR-06-3. FEC blok tavanı (`RX_BLOCKS_CAP` 32) sınıf başına bölünür.
**7.3 İlk kredi:** `W0` (sınıf başına), `W_TCTRL0`, `W_CTRL0`, `bk_flow_limit0`, `max_rel0` profil sabitidir; ilk birim ve ilk kontrol kontrol turu beklemez.
**7.4 İlan tetikleri (alıcı):** (i) bir sınır W/2 ilerleyince; (ii) bir BLOCKED bölümünden sonraki **ilk** artış hemen; (iii) **her BLOCKED(girdi) alındığında, güncel sınır girdideki değerleri aşıyorsa güncel sınır yeniden ilan edilir** (akış/sayım başına ≤ 1 / T_min; değişmemiş sınır, gönderen tarafındaki kayıp ilanı kurtarır); (iv) birden çok due girdi tek çerçevede birleştirilir. `max_seq` = uygulamaya (akış 0'da transporta) verilmiş + W; `max_bytes` aynı kural.
**7.5 Canlılık (gönderen):** **etkin sınır artışı** = bekleyen başı her iki boyutta (`seq`, bayt) açan artış; açmayan artıştan sonra yeni BLOCKED bölümü başlar. BLOCKED girdisi `(flow_id, seq, bytes)` ya da sayım girdisi `(tür, değer)` taşır; bütün bloklu akışlar ve sayımlar tek çok girdili çerçevede (≤ FLOW_CREDIT_MAX_ENTRIES). Aralık 1·srtt → ×2 → tavan min(RTO_MAX = 30 s, `idle_hold`/2); tekrar, etkin sınır artana dek sürer ve alıcının sunucu yuvasını canlı tutar (`idle_hold` 105 s, `runtime.rs:280`). Bu kurallarla "artış ilanı kayboldu, tüketilecek veri kalmadı" durumu (rev2 senaryo A) sonraki BLOCKED + (iii) ile kapanır.
**7.6 Yaşam döngüsü:**
- **FIN:** son birimde bayrak (ya da FIN yuvasındaki boş birim). Alıcı `fin_seq`'e dek her birimi uygulamaya verince akış ve taahhüt serbest; tutulan akışın kapanışı uygulama tüketene dek bekler (beyan).
- **RESET:** `reset_flow` → motor akışın birimlerini iptal eder (§8.3) → akış 0'da güvenilir `FLOW_RESET(flow, final_seq)`. Akış 0'ın tüketicisi transporttur (uygulama durgun olsa da işlenir); teyidi motorun teyit çıktısıdır. Alıcı akışın tamponunu — ACK'lenmiş ama teslim edilmemiş birimler dahil — atar (INV-ACK-DELIVER-1 istisnası, A.7) ve akışı kapalı işaretler.
- **Kontrol yükümlülüğüne yer — H_C_tavan (eş başına, şerit başına) [türetim]:**
  - akış 1 (uygulama kontrolü): FILE_ABORT ≤ `MAX_OUTGOING_TRANSFERS` 4 + FILE_RECEIPT ≤ `bk_flow_limit0` 4 (kabul edilen ve REDDEDİLEN her gelen BK akışı bir alındı doğurur, `0c22a07d:file_transfer.rs:1018-1034`) = **8**; RECEIPT(delivered) 05d bit haritasında tut-ve-dur (≤ `MSG_DEDUP_WINDOW`). W_CTRL0 ≥ 8 + RECEIPT payı [CAL].
  - akış 0 (transport kontrolü): FLOW_RESET ≤ açık giden BK akışı ≤ eşin `bk_flow_limit0` 4 → W_TCTRL0 ≥ 4.
  - VO: PTT_OPEN + PTT_CLOSE + yeniden PTT_OPEN ≤ 3 / giden konuşma → `max_rel0` ≥ 4 [CAL].
  - FILE_RECEIPT yuvası alıcıda BK akışı açılırken ayrılır; `bk_flow_limit` ancak akış çözümlenip FILE_RECEIPT akış 1'e kabul edildikten sonra ilerler → bekleyen alındı ≤ `bk_flow_limit0`. kama-app'in küresel, en eskiyi düşüren alındı kuyruğu kalkar (A.21). Yuvası ayrılamayan yeni aktarım/PTT → `Backpressure`; mevcut yükümlülük hiç düşmez.
- **Oturum değişimi:** akış taşınmaz; kama-app kuralları (TEXT eski `msg_id`; dosya `FILE_ABORT(superseded)`; PTT yeniden OPEN).
- **Live:** VO akışı kalıcı (churn yok); sıra boşluğu teslimi durdurmaz; konuşma sonu uygulamada: CLOSE alındı ∧ (CLOSE'tan küçük `flow_seq`'li demetler geldi ∨ CLOSE varışından D_VO sonra) → `PttEnded`; yedek PTT-R4 TTL.
- **Churn (BK):** açık küme ≤ `bk_flow_limit0`; kapalılık `max_seen` + açık kümeyle temsil edilir (§3).
**7.7 Sayaçlar ve kapı:** krediyi ya da W_CTRL/W_TCTRL'yi aşan birim → ACK'siz ret + `credit_violations`; kredili birim (akış 0/1 dahil) için yer yoksa `capacity_defect`; **kabul kapısı yalnız `capacity_defect = 0`** (sahibin notu 2). VO demetinin havuz yüzünden düşmesi `media_dropped_rx` (ACK'lenir; yalnız REL'siz birim). Kapalı/bilinmeyen akış girdisi, RESET/FIN ile yarışan BLOCKED ya da kredi girdisi → girdi düzeyinde yok sayılır, `stale_ctrl_entries` (ihlal sayılmaz).
**7.8 Kontrol çerçevesi sınırları (ACK yolu ve DoS):** alıcıda srtt yoktur (`0c22a07d:runtime.rs:3654-3670`) → yanıt aralığı sabit T_min [CAL, ör. 1 s]. **ACK yolunda zorunlu sınır:** sunucu yuvası (eş) başına ≤ 1 FLOW_CREDIT çerçevesi / T_min; due girdiler birleştirilir (≤ FLOW_CREDIT_MAX_ENTRIES); BLOCKED, yanıtının boyuna dolgulanır (amp ≤ 1, emsal `0c22a07d:wire.rs:220-221`). Olağan yük tetiklerle (W/2, BLOCKED) sınırlıdır; T_min yalnız kötü niyetli BLOCKED seline karşı tavandır [hesap: eş başına ≤ 38–80 B/s ters yön, saldırı altında]. Girdiler sabit boylu; çerçeve düzeyinde ret yalnız biçim hatalarında (ayrılmış bit, girdi sayısı aşımı) + kol sayacı. Gönderen store'u daima min(eşin kredisi, yerel sabit tavan) ile sınırlı.

## 8. FEC / ARQ / kopya; süre dolumu ve iptal

**8.1 FEC:** §3 (B-iii). Live ilk uygulamada FEC dışı [DUR-06-11]; `Ordered` akışta kısmi blok T_block içinde kapanır ve blok kimliği yeniden kullanılmaz.
**8.2 ARQ ve kopya** akış etiketini taşır; ARQ_RETX_CAP aşımında kopyaya devir (AMENDMENT-12) aynen; süresi dolan `DeadlineBounded` birimde devir yerine iptal.
**8.3 Tek sonlandırma çıktısı:** motor birime `iptal` durumu verir (`expires_ns` dolumu; RESET: `Event::CancelFlow`). İptal edilmiş birim `tx_queue`, `pending_lost`, `dup_queue` ve parite adaylığından çıkar; H-sahiplenmesi, kayıp ilanı, kuyruk-probu hedefi ve teyit yolları onu atlar; uçuş kayıtları yalnız muhasebede kalır. Çıktı `take_finished_units` = (birim, Teyit | İptal), birim başına oturumda en çok bir kez; aynı olayda çakışmada **teyit kazanır**. Motora hiç verilmemiş birim (offer kanalı, akış başına yerel tampon) payload sahipliği doğurmaz: runtime onu kendi tek çıktısıyla (İptal, birim başına bir kez) düşürür. Bu, AMENDMENT-12'nin altı donmuş cümlesini ve INV-NO-IDLE-TEARDOWN'a bir açıklamayı değiştirir (A.16).

## 9. 05c kapısının akış başına dönüşümü

`AcceptState` (oturum) → (oturum, akış); pencere = kredi; `RefuseWindow` = kredi ihlali; `RefuseFull` kredili birim için yapısal olarak erişilemez (erişilirse `capacity_defect`). Sıra başı rezervi (`0c22a07d:runtime.rs:3284-3297`) S1 kipinde kredi taahhüdüyle yer değiştirir (akış 0/1 dahil); örtük kip tek akışla aynı kuralla. "Durum unutmaz" (`runtime.rs:3359-3361`) akış ömrü boyunca; kapandıktan sonra `late_after_close`. Mesafe kapısı S1 kipinde kalkar (B1b).

## 10. kama-app ataması, PTT, sesli mesaj, pompa

- **Atama:** §4. Erken birim (`connect` K6): `connect`'e erken birimin sınıfı; `Ready`'de o kalıcı akışın ilk birimi (DUR-06-7).
- **Pompa** akış başına (A.21; B23). **Alındılar** gelen BK akışı başına yuvada; reddedilen FILE_BEGIN dahil (§7.6).
- **Tek kabul yolu (C15 v3 §5(ii)):** her üretici akış açma/gönderme yolundan; `Messenger::transport_mut()` baypası (`0c22a07d:messaging.rs:425`; `file_transfer.rs:1520`, `ptt.rs:960`, `signaling.rs:426`, :583) kaldırılır ya da daraltılır (B24).
- **PTT (DUR-06-18'e bağlı):** canlı kipte demet süresi = sunum + D_VO [CAL]; gönderen süresi dolanı iptal eder; alıcıda OPEN'dan önce gelen demet sınırlı ön tamponda (N [CAL]); geç demet oynatılmaz ve sayılır. Değişen donmuş metinler: AMP PTT-R0/R3 (A.18); PoC-KAPSAM-001 §2 PTT satırı ve "bekliyor" göstergesi, EK-2 md. 1, EK-3, **K8 PTT satırı ("sıfır bozuk hece", (a) seçilirse)**, §2 "Sözleşmeler" S5 dondurması ve K6 "facade değişmez" (A.19).
- **Desteklenmeyen profil:** VO yalnız min(profil, ρ_V,max·r(t)) profili karşılıyorsa açılır ve sürer; değilse tipli hata/olay; sesli mesaja otomatik/görünmez geçiş YOK (S5; B19 (b)). Sesli mesaj ayrı, görünür eylem (DUR-06-12).
- **05d / 05f:** RECEIPT ve FILE_RECEIPT akış 1'de; 05d A1 akış bütçesine dönüşür, tek kabul yolu devralınır (DUR-06-14). 05f rev2: dosya ilerlemesi = dosya akışının teyit tabanı.
- **UniFFI/S5 (DUR-06-23 + DUR-06-18):** `PttBundle`'a `flow_seq` ve anlamı ("akış sırası; geç demet oynatılmaz"); `pttPush` için `DeadlineExpired` ve canlı kipte "reddedileni yeniden ver" kuralının kalkması; `UnknownFlow` aynası; `node.rs` eşlemesine `FlowOpened`/`FlowEnded` (`0c22a07d:node.rs:560-584`). ui-api v0.3 kaynağı `kama-engine docs/ui-api-v0.md` (tag `ui-api-v0.3` = `5de9be16`, KM-SEAM-AK-001:9) — **facade'a dokunur:** oynatıcı sıra bilgisine ihtiyaç duyar ve "bekliyor" davranışı değişir; bu yüzden K6 ve §2 "Sözleşmeler" satırı da EK-4'e girer (A.19, A.24).

## 11. Fiziksel sınırlar, yan kanal, karma sürüm, kapılar

- Eksik parçayı bekleme kalkar, hat meşguliyeti kalkmaz; iletimdeki paket kesilemez: 1050 B 31 B/s'de ≈ 33,9 s, 300 B/s'de 3,5 s [hesap]; BK etkinken VO gecikmesi ≥ S_BK/r (DUR-06-10). Rezerv hiyerarşisiyle bu, VO'nun gördüğü tek BK terimidir (§5 m.5).
- 1,2 kbit/s ses ≈ 150 B/s payload → 31 B/s'de canlı ses yok; 31 B/s'de dosya kapsamda (S2; ağ zarfı B22).
- Kontrol datagramı ≥ mühür 22/26 B + gövde; S2 kontrol gövdeleri ayrıca `control_id` u32 [`1b1425d2:specs/kama-noise-wire-tdd-v2.0.md:347`]: 31 B/s'de 12 B'lik kredi çerçevesi ≈ 1,2 s, S2'de ≈ 1,4 s hat süresi [hesap].
- **Yan kanal [çıkarım]:** datagram boyu/zamanlaması hub'a açık; sabit boylu girdiler akış sayısı sızıntısını azaltır; kredi ritmi sızar — bilinçli kabul, ölçülür.
- **Karma sürüm:** eski alıcı FLOW_BATCH'i bilinmeyen çerçeve sayıp ACK'siz düşürür [`0c22a07d:runtime.rs:4089`] → sürüm/yetenek el sıkışmada, `Ready` öncesi fail-closed (DUR-06-13; B25).
- **Kalite kapıları:** bench ürün kolu bugün örtük kipte koşar (`0c22a07d:crates/bench/src/kama_product.rs:1`, :36-41). Örtük kip FLOW_BATCH'e geçtiği için netem 7 profil ve T-hücreleri çerçeveyi ve akış başına FEC'i ölçer; **çoklu akış, kredi ve BLOCKED'i ölçmez.** Öneri: bench'e S1-kipi ürün kolu (`kama-product-s1`; manifestte `kip` alanı; B20 ön koşulu) — DUR-06-25. Yapılmazsa A.26'da "T-hücreleri kredi/çoklu akışı ölçmez" beyanı.

## 12. Geçiş sırası ve çakışmalar

1. 05b #140 (rev6) → 05g → 05c merge (05c rev6 üstüne yeniden tabanlanır).
2. **Deney dalı** (§14; merge yok; FROZEN dosyalar yalnız orada "DENEY" etiketiyle).
3. rev4 + CEVAP 16 → ürün kodu (API-first): (i) S1 + KM-SEAM-AT-001 + drift · (ii) client-core çerçeve + akış/kredi + alıcı · (iii) motor S7(b) + engine §13 / AMENDMENT-13 · (iv) kama-app + AMP + PoC-KAPSAM EK-4 + 05d A1 / 05f rev2 · (v) UniFFI/S5 + kama-node + INT-02 + bench S1 kolu.
- **Çakışmalar:** 05c kabul/mesafe kodu (§9); 05g etkinleştirme kolları (akış yalnız `Ready` sonrası; `Closed` sonrası `UnknownFlow`); 05d M1 korunur, M2/A1 dönüşür; 05f rev2 akış başına; rev6 K-0…K-5 düzey-2 olur; KARAR-1 geçişi (`0c22a07d:specs/transport-layer-tdd.md:972-974`) K4G-06 ile birlikte.

## 13. DUR özeti (öncelikli; ayrıntı Ek D)

| Öncelik | DUR | Soru | Önerim |
|---|---|---|---|
| **1 — deneyden ÖNCE** | DUR-06-5 | Birim çerçevesi + FEC kimliği + **KARAR-1/ccr tarihli eki** + `flow_id` uzayı | B + B-iii; ek gerekir |
| 1 | DUR-06-2 | S7 seçim sözleşmesi + rezerv hiyerarşisi | (b2) + (i) |
| 1 | DUR-06-20 | Deney kapsamı; FEC kolunun yeni kodda ölçülmemesi | §14; B2-FEC yalnız tabanda |
| **2 — deney sonucunu bekler** | DUR-06-10 | VO açıkken BK/BE birim boyu | beyan + ölçüm |
| 2 | DUR-06-11 | FEC kapsamı (Ordered kısmı) | akış başına blok, B-iii |
| 2 | DUR-06-3 | Kontrol katmanı sayıları, akış 0/1 kredileri, havuz/bellek | §4/§7; sayılar deneyden sonra |
| 2 | DUR-06-17 | Sınırların sözleşmeye "koşullu garanti" girişi | evet, 05b K2 biçimi |
| **3 — uygulamadan önce** | DUR-06-18 | PoC-KAPSAM-001 EK-4 (PTT, K8, S5/K6) | (a) canlı PTT |
| 3 (06-18 (a)'ya koşullu) | DUR-06-1 | Canlı medya kredisi | ayrı sınırlı kabul + `max_rel` |
| 3 (06-18 (a)'ya koşullu) | DUR-06-6 | Live teslim + REL + gönderen kapısı | sırasız + `flow_seq` + W_LIVE kapısı |
| 3 (06-18 (a)'ya koşullu) | DUR-06-11 (Live) | Live FEC | FEC dışı |
| 3 (06-18'e bağlı) | DUR-06-23 | UniFFI/S5 yüzeyi (Hasan + EK-4) | `PttBundle.flow_seq` + `pttPush` |
| 3 | DUR-06-7 | S1 yüzeyi + `early_unit` | `send(flow)` + açık akış çağrıları |
| 3 | DUR-06-13 | Sürüm kimliği + karma sürüm | `transport-v2`; el sıkışmada fail-closed |
| 3 | DUR-06-14 | 05d A1/M2 devri + tek kabul yolu | A1 ayrıca kodlanmaz |
| 3 | DUR-06-15 | Sinyalleşme sınıfı | BE, kalıcı akış 3 |
| 3 | DUR-06-12 | Sesli mesaj PoC'de kod mu | hayır |
| 3 | DUR-06-21 | ~190 testte DUR-8 yöntemi | toplu yöntem |
| 3 | DUR-06-22 | INT-02/INT-03 kapı metni | `capacity_defect = 0` + sebep sütunları |
| 3 | DUR-06-25 | netem/T-hücre ölçüm kolu | bench S1-kipi kolu |
| **4 — Aşama 7** | DUR-06-24 | PTT eşik dosyası onayı | kalibrasyondan sonra (yöntem sahibin 1. notu) |

DUR'dan çıkarılanlar (C11 §3): rev1 DUR-06-4, -8 (→ 06-5 (iv)), -9 (→ 06-5 (iii)), -16 ([CAL] başlangıç değerleri orkestratörün; normatif olunca 06-3/-17).

## 14. Deney dalı (ADIM 2) — kapsam

**Taban:** `0c22a07d` (05c-kod, **05b rev5 motoru**; rev6 B′/K3/DUR-40 deneyde YOK). B2 tabanı ayrıca `37b78aad` üstünde (yalnız motor). Sonuçlar "**ürün profili dışı** (FEC kapalı; rev5 motoru)" etiketlidir. **Hüküm kapsamı:** deney yalnız S7(a)/(b) ayrımını ve izolasyonun en küçük kanıtını verir; rev6 + seçici etkileşimi ve rezerv hiyerarşisi U aşamasında sınanır (B10 (e), B14, B17).
- **K1 (kod değişmeden):** B2'nin üç kolu (ARQ, kopya, FEC — ürün profilinde açık, `0c22a07d:runtime.rs:292`) iki tabanda; T_BE ve deney seçicisine göre T_BE^rec (Ek B B2) K1'den ÖNCE commit'lenir; B1-taban. ≈ 1 gün.
- **K2 client-core:** FLOW_BATCH (B; FEC kapalı), (oturum, akış) kabul + sıra, örtük açma + `max_seen`/açık küme, akış başına hazır kuyruk + test-util `hold_flow`, tek kredi türü (birim; BLOCKED tek boyutlu `seq`) ACK yolunda §7.4 (i)–(iii) tetikleriyle ve T_min sınırıyla; **S1 kipinde mesafe kapısı kalkar** (örtük kip tek akışla). ≈ 2–3 gün.
- **K3 motor ("DENEY"):** `AppDataTagged` + iki katman (BE/BK) DATA kuyruğu + **deney seçicisi = katı sınıf önceliği BE > BK** (ARQ, kopya ve DATA başı için); DRR yok (sıra 2, isteğe bağlı). ≈ 1–1,5 gün.
- **K4 testler:** B1-en-küçük, B1b, B2 (ARQ, kopya; yeni kodda); B3 (b)(e) en küçük hâli isteğe bağlı (sıra 2). ≈ 1 gün.
- **Regresyon alt kümesi:** `tv_*`, `k4g05b_scheduling` (tek akış), INV-ACK-DELIVER-1 testleri. ≈ 0,5 gün.
- **Toplam ≈ 5,5–7 iş günü** (tahmin). Aşama 0 ≈ 9,5–12 iş günü (tasarım rev1–rev3 ≈ 2 gün harcandı); C15 v3 §7'nin "4–6 + 1–2"sinden ≈ +4 gün.

## 15. Bu notun sınırı

ADIM 1: sözleşme/arayüz taslağı + ayırt edici test planı. Hiçbir mekanizma uygulanmadı ya da ölçülmedi; §5 ve §7 sınırları türetim taslağıdır. Ek A tam liste olup yama metni değildir.

## Ek A — Onay gerektiren değişikliklerin TAM listesi (liste; yama metni DEĞİL)

**Kural:** hiçbir donma sessizce aşılmaz. Her satır kod yazılmadan önce sahibin yazılı kararını ister: "DUR" sütunundaki DUR'un cevabı (CEVAP 16 ya da sonraki tur) ya da "yama onayı" (tasarım onayından sonra, yama metniyle). Yama metinleri (üstü çiz + tarihli YAMA KAYDI + `word_coverage`) kodla aynı PR'da yazılır. Deney dalı bu dosyalara yalnız "DENEY" etiketiyle dokunur ve merge edilmez. DUR-05g-1'in dar S1 K7 istisnası bu kalemlerin hiçbirinin yerine geçmez. Sütunlar: FROZEN = donmuş sözleşme/spec/kapsam/ürün profili · Tel = iç çerçevede bayt değişimi (hub zarfı her satırda aynı) · Drift = `seam-api-drift.py` / `seam-coverage.json` beyanı.

| # | Dosya / öğe | Değişiklik (tasarım düzeyi) | FROZEN | DUR / onay | Tel / vektör | Drift |
|---|---|---|---|---|---|---|
| A.1 | `crates/kama-contracts/src/s1.rs` — tipler | `FlowClass {Control, Voice, BestEffort, Background}`, `FlowMode {Ordered, Live}`, `FlowSpec`, opak `FlowRef`; `Delivered`'a `flow`, `class`, `flow_seq`; `SessionEvent`'e `FlowOpened`, `FlowEnded {Finished, Reset}`; `TransportCapabilities`'e `max_control_unit_bytes` | EVET (S1) | DUR-06-7 | yok | §1 trait bloğu; `closed_sets_are_exhaustive` |
| A.2 | `s1.rs` — `KamaTransport` | `open_flow`, `close_flow` (FIN), `reset_flow`, `hold_flow`; `send`'in hedefi `FlowRef`; `recv()` imzası aynı, anlamı "tutulmamış akışlardan" | EVET | DUR-06-7 | yok | 12 `impl KamaTransport` (Ek C.G) |
| A.3 | `s1.rs` — `TransportError` | `UnknownFlow` varyantı; kredi/akış sayısı/VO profili dolu → `Backpressure` | EVET (kapalı küme) | DUR-06-7 | yok | §3 hata kümesi |
| A.4 | `s1.rs` — `DeliveryPolicy` anlamı | `DeadlineBounded` süresi dolunca motor boyunca iptal (bugün yalnız sunumda, `0c22a07d:transport.rs:1297-1305`); `BestEffort` tanımı (Live akışta) | EVET (§4 anlamı) | DUR-06-18, DUR-06-7 | yok | §4 metni |
| A.5 | S1 K6 `early_unit` + `connect` (`docs/seams/KM-SEAM-AT-001.md` K6 YAMA KAYDI 2026-10-08, :9; `0c22a07d:transport.rs:1000-1014`) | Erken birimin akışı: `connect`'e erken birimin sınıfı; `Ready`'de o kalıcı akışın ilk birimi; `Delivered.flow`/`flow_seq` tanımı | EVET | DUR-06-7 | yok | §1, H-EARLY-CONFLICT |
| A.6 | `handles.rs`, `s3.rs`, kama-noise-wire `envelope.rs`/`protector.rs`, hub-server codec | **Değişmez:** dış zarf 22/26 B, PN/nonce, `ProtectionContext`. (rev1'deki geniş "noise-wire değişmez" beyanı daraltıldı: iç düzen ve kimlik kapsamı A.10–A.11'de) | EVET | beyan | yok | beyan |
| A.7 | `docs/seams/KM-SEAM-AT-001.md` + `scripts/ci/seam-coverage.json` + `scripts/ci/seam-mutation.py` | §1 trait bloğu (A.1–A.5); §3; §4 "aynı session içinde Reliable birimler gönderim sırasında" (`0c22a07d:…KM-SEAM-AT-001.md:153`) → "aynı akış içinde"; Live sırasız + `flow_seq`; INV-ACK-DELIVER-1 (:154) akış başına + **istisna:** kapatılmış/RESET edilmiş akışın geç birimi ACK'lenip atılır (`late_after_close`); "kapasite durması ACK kesilerek gösterilmez"; §6 H-FLOW-ISOLATION, H-FLOW-CREDIT, H-LIVE-EXPIRY, H-CTRL-SERVICE + her biri için sahte mutasyon operatörü | EVET (G-S1) | DUR-06-7 + yama onayı | yok | 4 yeni H-* satırı |
| A.8 | `crates/client-core/src/wire.rs` (v0 kabuk) | `FLOW_BATCH` tip baytı + BatchPayload codec'i (`batch.rs`) veri yoluna bağlanır; store-miss → boş batch; yeni kontrol çerçeveleri FLOW_CREDIT, FLOW_BLOCKED; FEC/RECOVERED düzeni değişmez (`block_id` anlamı A.10); 0x00/0x05 yalnız örtük kip (bench) | EVET (`0c22a07d:runtime.rs:230` "tel, codec ve `tv_*` değişmez") | DUR-06-5 | **EVET** (zarf içi) | `api_surface_v0` (A.28) |
| A.9 | `specs/ccr-format-spec.md` §35.6 / §35.14 `BatchPayload` (`1b1425d2:…:3017-3035`) + `docs/seams/KM-SEAM-TC-001.md` KARAR-1 (`1b1425d2:…:82`, :173) + `batch.rs:28-29` + `specs/transport-layer-tdd.md` "veri yolu geçişi sonraki sprint" beyanı (`0c22a07d:…:972-974`) | `BatchSubMessage`'a sabit 7 B akış başlığı (`flags · flow_id · flow_seq`) — "alt mesaj başlığında teslim sınıfı" (plan KATMAN 6, `1b1425d2:docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:287`); KARAR-1'e tarihli sahip eki: canonical birim = başlıklı BatchPayload; interim 11 B çerçevenin emekliliği K4G-06 ile | EVET (ccr bayt otoritesi; KARAR-1 sahip kararı) | DUR-06-5 | **EVET** (canonical çerçeve) | KM-SEAM-TC-001 statü satırı |
| A.10 | `specs/kama-noise-wire-tdd-v2.0.md` §2.6 ID-SCOPE (`1b1425d2:…:430-433`), §3 DÖRT-KİMLİK (:452-458), §2.56 codepoint kaydı (:325-340), §2.57 gövdeler (:347) | `block_id` = `flow_id ‖ flow_block` (oturum + yön tekil, akış içinde monoton, NO-REUSE korunur); `message_id := flow_seq` (S1 parçalamasız kip); akış koordinatı FEC-LOGICAL-IDENTITY'nin YANINDA beşinci kimlik değil, (block_id, ESI)'nin türetim kaynağı; yeni kontrol codepoint'leri **0x1D'den** K-WIRE-CTRL-1 prosedürüyle (0x1B/0x1C heartbeat'e ayrılmış); S2 gövdelerine `control_id` | EVET (KİLİTLİ) | DUR-06-5 | EVET (yeni codepoint; FEC layout aynı) | — |
| A.11 | `crates/kama-transport-schema/src/{prefix.rs, inner_kind.rs, control.rs, lib.rs}` + `crates/kama-noise-wire/tests/golden_wire.rs` | `source_flags`'a akış sonu (FIN) biti (bugün ayrılmış bit → ret, `0c22a07d:…/prefix.rs:15-16`, :36); yeni `inner_kind`/`Control` girdileri; DTO'lar (`LogicalShardHeader`, `Control::Recovered`) düzen olarak değişmez; golden_wire (101 B) **değişmez hedefi** | EVET | DUR-06-5 | EVET (prefix bayrağı; tek kaynak schema) | — |
| A.12 | `crates/client-core/src/runtime.rs` — gönderen | Store girdisine (akış, sıra) + ters dizin; S1 kipinde mesafe kapısı (`:2511`, :2575-2581) → akış kredi kapısı; akış başına yerel tampon (offer FIFO dışında); akış başına FEC biriktirici; `AppDataTagged`; BLOCKED üretimi; sonlandırma çıktısına bağlı silme | EVET (ürün davranışı) | DUR-06-5, DUR-06-2 + EVOLUTION §22 | EVET (A.8) | — |
| A.13 | `runtime.rs` — alıcı (`DeliveryGate`, `AcceptState`, `DeliveryLedger`) | (oturum, akış) kabul; pencere = kredi; `RefuseWindow` = `credit_violations`; `RefuseFull` → `capacity_defect`; S1 kipinde sıra başı rezervi (`:3284-3297`) kredi taahhüdüyle yer değiştirir (akış 0 dahil); sınıf başına `RX_BLOCKS_CAP`; FLOW_CREDIT ACK yolunda, tetik kuralı ve DoS sınırlarıyla | EVET (05c A.6'nın devamı) | DUR-06-3, DUR-06-5 | EVET | KM-SEAM-AT-001 §4 (A.7) |
| A.14 | `runtime.rs` — `RuntimeConfig` / `KamaV1Profile` / kimlik (`:236`, `:241`) | Havuzlar, `W0`, `W_CTRL0`, `bk_flow_limit0`, `max_rel0`, ρ_C, ρ_V,max, Q_BE:Q_BK, T_block, D_VO, W_LIVE, FLOW_CREDIT_MAX_ENTRIES [CAL]; iki doğrulama değişmezi (§7.2); yeni kimlik | EVET (`transport_v1_freeze.rs`, `product_profile_v1.rs`) | DUR-06-13, DUR-06-3 | yok | `api_surface_v0` |
| A.15 | `crates/client-core/src/transport.rs` | (oturum, akış) `Reorder`; akış başına hazır kuyruk + `hold_flow`; `open/close/reset_flow`; `FlowOpened/Ended`; 05g etkinleştirme kollarıyla uyum; erken birim (A.5) | Hayır (adaptör; davranışı S1'e bağlı) | tasarım onayı | yok | H-* real hedef |
| A.16 | `specs/transport-layer-tdd.md` §12 AMENDMENT-12 donmuş cümleleri (`0c22a07d:…:855-899`) + §0 INV-NO-IDLE-TEARDOWN kapalı sonlandırma listesi (:57-63) + yeni AMENDMENT-13 | (1) INV-NO-ORPHAN (:855) kapsamı: "uygulamanın RESET ettiği akış ve süresi dolan `DeadlineBounded` birim hariç"; (2) "Payload yalnız teyit çıktısıyla silinir … Başka hiçbir yol silmez" (:862-864) → "yalnız sonlandırma çıktısıyla (teyit ya da iptal; birim başına oturumda en çok biri)"; (3) "İzleme teyide ya da §0 kapalı listesindeki bir sonlandırmaya dek sürer" (:879) → listeye RESET ve süre dolumu; (4) "Teyit çıktısı (NORMATİF)" (:892-899) → "Sonlandırma çıktısı"; aynı olayda teyit kazanır; iptal teyit ÜRETMEZ; AMENDMENT-13: akış/kredi/seçici | EVET (KAT GO-FREEZE; sahip metni) | yama onayı (DUR-06-2'ye bağlı) | yok | — |
| A.17 | `crates/adaptive-transport/src/sender.rs` (S7 b; taban 05b rev6 `37b78aad`) | Eklemeli `Event::AppDataTagged(OpaqueUnit, ServiceTag)` (etiketsiz = tek akış); katman/akış başına DATA kuyruğu; tek saf seçici: rev6 `refresh_reserve` (:7334), `assign_reserve_to_holder` (:6311), `on_class_emitted` (:6333), `ServiceRights` (:1423), `serve_copy_share` (:7292), `copy_queue_head` (:7243) seçicinin düzey-2'si olur, haklar (akış, sınıf) başına; `TransmitKind::Control`; iptal durumu + `Event::CancelFlow` + `take_finished_units` | **EVET (FROZEN v1.1 motor)** | DUR-06-2 | yok; `tv_*` değişmez HEDEFİ (kayarsa DUR) | — |
| A.18 | `specs/application-message-protocol.md` §3 | PTT-R0 → OPEN/CLOSE `Reliable`, demet `DeadlineBounded`; PTT-R3 → sınırlı ön tampon; "FILE_BEGIN her zaman önce gelir" gerekçesi aynı akışa (`0c22a07d:file_transfer.rs:74-75`); RECEIPT/FILE_RECEIPT/FILE_ABORT akış 0; kayıt baytları DEĞİŞMEZ | EVET (AMP donmuş metin) | DUR-06-18 | AMP baytları ve `amp-record*-v1.json` değişmez | — |
| A.19 | `docs/decisions/PoC-KAPSAM-001.md` §2 PTT satırı (`1b1425d2:…:26`), EK-2 md. 1 (:98), EK-3 cümlesi (:27, :103-105) + `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` D7 ek (:94, :287) + K4G-08 PTT-R0 + `ptt.rs:18-24` modül belgesi | "PTT Reliable klip; geç ses düşürülmez; transport PoC'de yalnız Reliable taşır" → sahibin tarihli **EK-4**'üyle canlı PTT (§10); K8 hedefleri (:62) korunur, B11'in ürün hedefi girdisi olur | **EVET (KESİN; yalnız sahibin tarihli eki, :7-8)** | **DUR-06-18** | yok | — |
| A.20 | `specs/adaptive-transport-engine-spec.md` (§3 ana döngü, §9, §11, §13 — taban 05b rev6 metni `37b78aad`, "istisna ve karşılık kuralıyla" —, §16.2) + `docs/transport-v1/{INVARIANTS, PRODUCT-PROFILE, ARCHITECTURE, EVOLUTION §22, TRANSPORT-CRYPTO-HANDOFF (:18, :60 "mesaj = birim", "DATA 0x00"), KNOWN-LIMITATIONS}.md` | İki düzeyli seçici, ServiceTag, katman 0, iptal; kimlik ve altı beyan | EVET | yama onayı (bölüm listesiyle; DUR-06-2, -13) | yok | — |
| A.21 | kama-app: `messaging.rs`, `file_transfer.rs`, `ptt.rs`, `signaling.rs` | Akış ataması; **pompa akış başına** (bugün eş başına en eski aktarım, `0c22a07d:file_transfer.rs:862-881`); dosya tüketicisi `hold_flow`; RECEIPT akış 0; PTT ön tampon + `flow_seq`; **tek kabul yolu: `transport_mut()` (`messaging.rs:425`) kaldırılır ya da daraltılır**; 05d M2 kapısının yeniden değerlendirilmesi | Hayır (ürün kodu); 05d kararlarını değiştirir | DUR-06-14 | yok | — |
| A.22 | 05d DUR-05d-7 A1 / DUR-05d-8 kararları (C14 §4, C15 v3 §5) | A1 ayrıca kodlanmaz → akış bütçesi erişimcisi; W_F → BK penceresi; H_C → H_C_tavan (§7.6); §5(ii) "tek kabul yolu, ürün derlemesinde zorlanır" şartı devralınır | karar metni | DUR-06-14 | yok | — |
| A.23 | 05f rev2 (`e5514e7a` reddedildi) | Dosya ilerlemesi = dosya akışının teyit tabanı + yeni benzersiz teyit; "yerel vazgeçme politikası" zarfı | — | C15 v3 §6 (yeni DUR yok) | yok | — |
| A.24 | UniFFI/S5: `crates/kama-mobile/src/{error.rs (:218-250, :374-440), event.rs (:107-115)}`, `docs/seams/KM-SEAM-AK-001.md` (:57 `pttPush`, :75 `PttBundle`), `crates/kama-mobile/README-HASAN.md` (:15), `crates/kama-node/src/event.rs` (:108-116), `node.rs` SessionEvent eşlemesi (`0c22a07d:node.rs:560-584`) | `UnknownFlow` aynası; `PttBundle`'a `flow_seq` ve anlamı ("akış sırası; geç demet oynatılmaz"); `pttPush` için `DeadlineExpired` ve canlı kipte "reddedileni yeniden ver" kuralının kalkması; `FlowOpened/FlowEnded` eşlemesi; yeni sayaçlar `NodeMetrics`'te | EVET (S5 = ui-api v0.3 FROZEN) | **DUR-06-23 (Hasan ile ortak onay)** | yok | — |
| A.25 | INT-02/INT-03 kapı metni (C15 v3 §7 Aşama 7; C9 §5; `1b1425d2:scripts/netem/int02_analyze.py:18`, :969-1011) | "kapasite reddi kaynaklı boşa bayt ≈ 0" → "`capacity_defect = 0` (akış 0 dahil, yalnız geçerli krediye uyan güvenilir veri)"; ağ kaybı / protokol ihlali / medya expiry / kontrol baytları ayrı sütun; PTT `in_order_complete` hükmü → B11 ölçüleri | kapı metni | **DUR-06-22** | — | — |
| A.26 | `docs/transport-v1/adr/ADR-transport-v1-freeze.md` (`0c22a07d:…:18-24`) + `ADR-hostile-validation-contract.md` + ADR-18 netem 7 profil + §4.4 kapısı | Transport davranışı değiştiği için etkilenen T-hücreleri yeni kimlikle yeniden koşulur; netem 7 profil regresyonu ön-kayıtlı tahminle (B20); kayarsa DUR | EVET (ADR) | DUR-06-13 + yama onayı | — | — |
| A.27 | `vectors/` + yeni Python referansı (`ref/transport/` önerisi) + `crates/kama-vectors` + `fuzz/fuzz_targets/wire_v0.rs` | FLOW_BATCH, FLOW_CREDIT, FLOW_BLOCKED vektörleri YALNIZ Python referansından; paket **canonical** (A.9 eki onaylanırsa) — interim değil; `SCHEMA.json` değişmez; fuzz hedefine yeni çözücüler | vektör kuralı | DUR-06-5 | EVET (yeni paket) | — |
| A.28 | `crates/client-core/tests/api_surface_v0.rs` + `docs/design/P4-TRANSPORT-API-FREEZE.md` (:126) | wire-v0 imzaları, `Delivery`, `RuntimeConfig` deseni, `Event` re-export'u (yeni varyant) | EVET (P4) | DUR-06-21 (yöntem) + yama onayı | — | — |
| A.29 | `crates/adaptive-transport/src/arq.rs` | **Değişmez hedefi:** akış içi plan sırası (`0c22a07d:arq.rs:124-127`), `tv_arq` | EVET | beyan | yok | — |
| A.30 | `crates/kama-engine` (`src/recovered_map.rs` (block, esi)→birim eşlemesi; testlerde `OpaqueUnit` literal'leri) | **Değişmez hedefi** (rev1'deki "taranmadı" kaldırıldı): `block_id` tekilliği korunduğu için eşleme aynen çalışır [çıkarım]; eklemeli API ile test kaynakları aynı kalır | — | beyan (kayarsa DUR) | yok | — |
| A.31 | `docs/seams/KM-SEAM-TH-001-TASLAK.md` A3 (`1b1425d2:…:63`) | Gereksinim notu: hub sınıf göremez → handle başına kuyruk sığ ve sınırlı olmalı (INT-03 izolasyonu) | S4 taslak (NON-GATING) | yama onayı (S4 sahibine iletilir) | — | — |
| A.32 | `crates/kama-node/{event.rs, probe.rs, node.rs}` + `crates/kama-integration/examples/int02_node.rs` + `scripts/netem/int02_analyze.py` | Yeni sayaçlar (`capacity_defect`, `credit_violations`, `media_expired_tx`, `media_dropped_rx`, `media_late_rx`, `late_after_close`, `stale_decisions`, `vo_rate_deficit`, `held_above_floor_peak`, akış başına ARQ emisyonu, kredi yeniden ilanı, kredi/BLOCKED baytları); yeni kollar (Ek B) | Hayır | tasarım onayı | yok | — |
| A.33 | `scripts/ci/layer_check.py` | `adaptive-transport` kama-app'e ve kama-contracts'a bağımlı olmaz (`ServiceTag` motorun tipi) — kural kapsamı doğrulanır | Hayır | tasarım onayı | — | — |
| A.34 | `specs/ccr-format-spec.md` §35.2/§35.3 el sıkışma (sürüm/yetenek) | Karma sürüm: sürüm/yetenek kimliği doğrulanmış el sıkışmada, `Ready` öncesi fail-closed (seçenek DUR-06-13'te) | EVET | DUR-06-13 | EVET (seçilirse) | S1 olayı (DUR-06-13) |
| A.35 | `crates/kama-store`, `crates/kama-fec-codec` | **Değişmez:** kalıcı sayaçlar ve RS kodlayıcı akış bilmez; blok boyu/ESI geometrisi aynı | — | beyan | yok | — |

**Değişmez beyanı (listenin parçası):** A.6, A.29, A.30, A.35 + ACK_V2 codec'i ve `tv_ack` + AMP kayıt baytları ve `amp-record*-v1.json` + `ccr-*` vektörleri (SSI1/SSR1/SSD1; dış zarf aynı) + `vectors/SCHEMA.json` + noise-wire `golden_wire` (FEC düzeni aynı). Uygulamada bunlardan biri değişmek zorunda kalırsa iş durur ve ayrı DUR açılır.

## Ek B — Kabul testleri (ayırt edici test planı)

**Ortak kurallar.** Fikstür ön koşulları açık assertion'dır; kurulmadıysa test KIRMIZI ("ölçülecek iş oluşmadı" yeşil sayılmaz; `None` = kırmızı). Dışarıdan ACK, kayıp ilanı ya da kredi ENJEKTE edilmez; röle gerçek çerçeveleri düşürebilir, geciktirebilir, yeniden oynatabilir ama çerçeve sentezlemez. Sınırlar koddan ÖNCE türetilip commit'lenir; sonuç "ölçüldü" diye yazılır. **Aşama:** D = deney dalı (yalnız §14'tekiler; YEREL/SHA kuralıyla) · U = Aşama 6 ürün uygulaması · I = Aşama 7 INT-02 (3 tohum). Ön-kayıtlı beklentiler "taban" satırındadır; tutmayan beklentide önce türetim, sonra kod sorgulanır.

**Düzenekler** (her ön koşul ancak assert edilebildiği düzeneğe konur):

| Kod | Düzenek | Ne assert edilebilir | Eksik → yeni ihtiyaç |
|---|---|---|---|
| M | motor-deterministik (`adaptive-transport` testleri, sabit saat; rev6 T1/T2 düzeni) | motor kuyrukları, haklar, rezerv, emisyon dizisi | — |
| R | client-core röle (`0c22a07d:crates/client-core/tests/k4g05b_runtime_ownership.rs:306-390`: çerçeve sınıflama, `drop_first`, `hold_target_acks`, chaos; identity seam) | hedefli çerçeve kaybı/gecikmesi, alıcı/gönderen runtime sayaçları | FLOW_* çerçeve sınıfları rölede tanımlanır |
| N | kama-node iki düğüm (`kama-node/tests/two_nodes_loopback.rs`) | uçtan uca olaylar, NodeMetrics | **N+:** hedefli kayıp kancası yok → test-util kayıp kancası |
| I | INT-02 netem (`1b1425d2:crates/kama-integration/examples/int02_node.rs:8-9`, lo, mühürlü) | yalnız gözlenebilir sayaçlar (A.32): `held_above_floor_peak`, akış başına ARQ emisyonu, kredi yeniden ilanı, sebep sayaçları | hedefli çerçeve kaybı yok → I'de hedefli ön koşul KURULMAZ, gözlenir |
| A | kama-app sahteleri — **üç ayrı model** (rev1 uyarısı düzeltildi): `mock_transport.rs:487-500` oturum geneli HOL; `tests/file_transfer.rs:419-423` `Link::recover` kayıp birimi sonrakilerin ARKASINA koyar (HOL yok); `tests/{ptt,text_receipt,signaling}.rs` kayıpsız tek FIFO | uygulama motoru davranışı | **A+:** akış başına sıra + kredi + `hold_flow` taşıyan tek referans sahte |

**Sayaç sözlüğü** (B7 ayırt eder): `net_loss_recovery_bytes` · `credit_violations` (kredi ya da W_CTRL aşımı; ACK'siz ret) · `media_expired_tx` · `media_dropped_rx` (yalnız `DeadlineBounded`) · `media_late_rx` / `media_late_unplayed` · `capacity_defect` (kredili güvenilir birim — akış 0 dahil — için yer yok; **kabul kapısı yalnız bu**) · `late_after_close` · `stale_decisions` · `vo_rate_deficit` · `ctrl_credit_bytes`, `ctrl_blocked_bytes`.

| # | Amaç | Düzenek | Ön koşullar (assert) | Ölçülen | Geçer / taban | Aşama |
|---|---|---|---|---|---|---|
| **B1** | §9.5 ilk kabul: dosya tüketicisi durmuş + dosyada kayıp birim → TEXT teslim, medya süre politikası, bellek sınırlı; açılınca dosya tamamlanır | R (D, U) · N+ (U) · I (yalnız gözlem) | `hold_flow(dosya)`; birim k ≥ 1 kez düştü ve k'den büyük ≥ 1 birim kabul edildi; dosya akışı kredi sınırında (`flow_blocked` ≥ 1); TEXT (U'da VO) bunlardan SONRA sunuldu; `distance_holds = 0` | TEXT teslim süresi; dosya tamponu; alıcı toplam tampon; sebep sayaçları | TEXT tutulu dosya varken teslim; dosya tamponu ≤ verilmiş kredi (D) / ≤ havuz (U); `capacity_defect = 0`, `credit_violations = 0`; `media_dropped_rx` hükmü kırmaz ama raporlanır; açılınca dosya sırayla tamamlanır, özet tutar | D (TEXT + dosya, FEC kapalı) · U · I |
| B1-taban | B1'in eski API'yle yazılabilen varyantı (F8) | R · I | bütün tüketici durur (`ek-tuketici-durak` tarifi) + dosya birimi kaybolur | TEXT teslimi | **taban beklentisi (ön-kayıtlı):** TEXT, k kurtarılana dek teslim edilmez (oturum `Reorder`, `0c22a07d:transport.rs:396-419`) → KIRMIZI | D (K1) |
| **B1b** | 05c mesafe kapısının yeni akışları eski dosyanın eksik birimine bağlamadığı | R | `max_inflight_units = 8`; dosya birimi k'nin bütün iletimleri bir süre düşer; k'den sonra > 8 TEXT | `distance_holds`, TEXT teslimi | **taban:** `distance_holds ≥ 1`, TEXT durur (`0c22a07d:runtime.rs:2575-2581`, :2608) → KIRMIZI · **deney/ürün:** TEXT teslim, `distance_holds = 0` | D · U |
| **B2** | EK-1 §1.2 zorunlu karşı örnek (yeniden tasarlandı) | M | offer kanalı ve `tx_queue`'da dosya DATA'sı yok; N_u ≥ max(8, ⌈T_BE·r/S_u⌉ + 1) eski dosya birimi teyitsiz ve her kolda vadeli kurtarma işi: (a) ARQ (RECOVER_ARQ, `pending_lost`) · (b) kopya (`dup_queue`) · (c) FEC (bekleyen parite); sonra TEXT t sunulur, t'nin ilk iletimi düşer; (a)'da later-ACK yolu kısa süre açık tutulup t'nin kaybı doğal ilan edilir, sonra kesilir; (b)'de t'nin kendi kopya zinciri oluşur | **asıl:** t'nin KENDİ kurtarma emisyonunun ilk hizmet gecikmesi ve ondan önce çıkan u-kurtarma emisyon sayısı/baytı · **ayrı satır:** t'nin ilk emisyonu (S7 hükmüne bağlanmaz) | **taban beklentileri (ön-kayıtlı; T_BE K1'den önce [CAL] değerlerle commit'lenir):** (a) `0c22a07d` ve `37b78aad`: t'nin retransmit'i önce ilan edilmiş N_u kaydın hepsinden sonra (plan vade sırası `arq.rs:124-127`; vade = ilan anı `0c22a07d:sender.rs:3846-3849`, `37b78aad:sender.rs:3911`) → gecikme ≥ N_u·S_u/r [hesap: 8·1050/300 = 28 s] → KIRMIZI · (b) `0c22a07d`: en küçük kimlik HOL'u (`:7103-7110`) → t'nin kopyası N_u kopyadan sonra → KIRMIZI; `37b78aad`: yaş tavanı (DUR-40, `:7243-7265`) yalnız vadesini RTO_MAX aşmış kayıtlarda; ölçüm penceresi < RTO_MAX'ta HOL → KIRMIZI · (c) parite FIFO + oturum geneli blok → t'nin bloğunun ilk paritesi önceki ≥ 1 bloğun paritesinden sonra → ölçülür (KIRMIZI beklenir) · **deney/ürün:** t'nin kurtarma emisyonu türetilmiş T_BE^rec içinde. Kırmızı çıkmazsa S7 kararı yalnız asıl ölçüyle yeniden açılır | D (taban: üç kol, iki SHA; yeni kod: (a), (b)) · U (üç kol) |
| B3 | Kredi güncellemesi kaybı, canlılık | R (U; D'de isteğe bağlı) · I (gözlem) | (a) N_k FLOW_CREDIT düşürüldü; (b) BLOCKED yanıtlandıktan SONRA artan kredi çerçevesi düşürüldü; (c) `hold_flow` > `idle_hold` | BLOCKED sayısı/aralığı; blokta kalma; `ctrl_*_bytes` | akış "BLOCKED aralık tavanı + RTT" türetilmiş sınırında ilerler; veri kaybı yok; `capacity_defect = 0`; (c)'de yuva emekliye ayrılmaz | D-sıra 2 · U · I |
| B4 | Sırasız / bayat / tekrar / eski oturum kredisi | R | röle daha küçük sınırlı GERÇEK çerçeveyi büyüğünden sonra oynatır; tekrar; yenileme sonrası eski bağlamlı çerçeve | etkin sınır; gönderilen en büyük `flow_seq` | sınır hiç azalmaz; gönderen sınırı aşmaz; eski oturum çerçevesi bağlam-önce dağıtımda düşer | U |
| B5 | İki yönlü doygunluk | N · I | iki yönde dosya kredide bloklu ≥ 1 bölüm; iki yönde bekleyen RECEIPT/FILE_RECEIPT; oturum değişmedi | tamamlanma; RECEIPT/FILE_RECEIPT/kredi gecikmesi | hepsi tamamlanır; canlı RECEIPT silinmez; `capacity_defect = 0`; kredi ilanı iki yönde > 0 | U · I |
| B6 | Sürekli yeni yük: sonlu hedef küme biter | R · I | hedef küme BAŞTAN sabit; ayrı arka plan üreticisi kapasite üstü yükü durmadan sürdürür ve `Backpressure` alır | hedef birim tamamlanma süreleri | her hedef birim türetilmiş sınırda; arka planın durmasına bağlı değil | U · I |
| B7 | HER BİT DÜŞMAN: beş sebep ayrık | R | beş alt koşu, her birinde tek sebep: (1) ağ kaybı; (2) krediyi aşan sahte gönderen; (3) VO süre dolumu; (4) test kancasıyla küçültülmüş havuz; (5) akış 0 tüketicisi durgun + W_CTRL'ye dayanan RECEIPT seli | sözlükteki bütün sayaçlar | her alt koşuda yalnız kendi sayacı artar; (4)'te `capacity_defect > 0` ve hüküm KIRMIZI; (5)'te `capacity_defect = 0`, canlı RECEIPT kaybı yok; ürün koşularında `capacity_defect = 0` ŞART | U · I (sütunlar) |
| B8 | PTT altında TEXT gecikmesi | N+ · I | VO desteklenen profilde ölçüm boyunca demet üretiyor | TEXT gecikme dağılımı | ölçülür/raporlanır + türetilmiş üst sınır içinde | U · I |
| B9 | Sürekli TEXT altında dosya biter | M · R · I | BE ölçüm boyunca backlog'lu — motor sayacıyla ("BE boşken BK hizmeti" = 0; örnekleme değil) | dosya tamamlanma; BK bayt payı | Q_BK payından türetilen sınırda; pay alt sınırın altına inmez | U · I |
| B10 | EK-1 §1.1 kontrol (iki yönlü) | M · R · I | (a) VO ölçüm boyunca backlog'lu; (b) kontrol katmanı ölçüm boyunca uygun işte; (c) bütün veri akışları kredide bloklu; (d) P_BK tam taahhüt + akış 0 tüketicisi durgun | (a) **ACK_V2 aralığı her iki yönde ve uçuştaki birimin teyit gecikmesi**, kredi ilanı, FLOW_RESET, PTT_OPEN/CLOSE, RECEIPT gecikmesi; (b) BK/BE payı; (c) kredi ilanı ve BLOCKED çıkıyor mu; (d) kabul | (a) türetilmiş kontrol sınırı (§5 m.5); (b) kontrol tüketimi ≤ ∫ρ_C·r + b_C, BK/BE alt sınırın üstünde; (c) kontrol veri kredisi olmadan gider; (d) RECEIPT/FILE_* ve TEXT kabul edilir | U · I ((a)) |
| B11 | EK-1 §1.3 PTT ölçüleri ve eşik sabitleme | I (U: araçlar) | yazılı profil: hız, RTT, kayıp, kesinti **ve BK/BE birim boyu**; VO açık; B1/B9 yükü eşzamanlı | oynatılabilir oran; konuşma başlangıcı (`open` → ilk oynatma); uçtan uca gecikme; **gecikme bileşeni sayaçları** (iletimdeki BK paketi beklemesi, katman-0 beklemesi, `vo_rate_deficit`); sebep sayaçları | **sahibin 1. notu:** (1) KALİBRASYON koşusu (hüküm yok); (2) sahip eşikleri **ürün hedefi (PoC-KAPSAM-001 K8: bas→ilk ses ≤ 1,5 s iyi / ≤ 3 s kötü ağ; sıfır bozuk hece)** + kalibrasyondan seçer; `PTT-ESIK-v1` = profil, ölçü tanımları, eşikler, koşu/tohum sayısı, **kalibrasyon SHA'sı, [CAL] değerleri, karar istatistiği (tohum başına en kötü değer)**; kabulden ÖNCE commit + onay; (3) kabul YENİ koşu (yeni kimlik, yeni tohumlar); (4) VO yolunu etkileyen kod/parametre değişikliği → yeni kalibrasyon + yeni dosya + yeni kabul. Geç demet hiç oynatılmaz | U · I |
| B12 | Gönderici deadline: süresi dolan medya hiçbir kuyrukta yaşamaz; tek sonlandırma | M · R | VO birimi v süre dolmadan şu yerlerden birinde (ayrı kollar): offer kanalı, runtime alımı/store, `tx_queue`, `pending_lost`, `dup_queue`, kuyruk-probu hedefi, H-sahiplenme adayı, kayıp ilanı anı; (DUR-06-11 (b) ise `pending_fec`); dönüş yolu kesik; ufuk ≥ H_maks + RTO_MAX | süre dolumundan sonra v'nin varlığı ve emisyonu; geç ACK etkisi; sonlandırma çıktısı | süre dolumundan sonra v hiçbir adaylıkta değil, tele çıkmaz; sonlandırma çıktısında tam bir kez İptal; geç ACK ne teyit ne ikinci silme; aynı olayda teyit + iptal → Teyit; güvenilir birim süre nedeniyle iptal edilmez | U (D'de değil) |
| B13 | EK-1 §1.4 kredi yaşam döngüsü | R · A+ | (a) hiç FLOW_CREDIT görülmeden ilk birim + **erken birim (`connect` K6) kolu**; (b) FIN ve RESET + **FIN'li birim FEC ile kurtarıldı**; (c) eski oturumdan geç çerçeve gerçekten geldi; (d) Live: eski konuşmanın kayıp/süresi dolmuş demetleri + yeni konuşma; (e) **TEXT akışı baştan sona açıkken** K kez dosya aç-kapa + FIN sonrası geç kopya; (f) yükümlülük yuvaları dolu; (g) P_BK tam taahhüt | (a) kabul; (b) serbest kalma anı, `late_after_close`; (c) uygulanmama; (d) teslim + OPEN/CLOSE tekrar sayısı; (e) kayıt/bellek; (f) `open_flow`/yeni aktarım/yeni PTT sonucu; (g) TEXT/kontrol kabulü | (a) kontrol turu beklenmez; (b) FIN görünür, kaynaklar serbest, RESET teyidi motor teyidiyle, `capacity_defect = 0`; (c) uygulanmaz; (d) yeni ses durmaz; OPEN/CLOSE tam bir kez; konuşma sonu kuralı (CLOSE + D_VO); (e) ≤ türetilmiş tavan, `credit_violations = 0`; (f) **FILE_ABORT, FILE_RECEIPT, FLOW_RESET, PTT_CLOSE, RECEIPT** yükümlülükleri hiç reddedilmez; yuva yokken yeni iş `Backpressure`; (g) kabul | U |
| B14 | EK-1 §1.5 T_birim (akışlar arası) | M | türetim koddan ÖNCE (rev6 formülü akış içi, §5 m.4); ≥ 2 akışta ≥ 2'şer teyitsiz zincir; dönüş yolu kesik; ≥ 2 × T_birim; kollar: prob/parite açık (P_pf ≠ 0) · PRE_CONVERGE (beyanlı istisna) | her birimin son hizmet yaşı, yeniden seçilmesi, ARQ→kopya geçişinde izleme | ölçülen ≤ T_birim; 31 ve 100 B/s; "ölçüldü" | U |
| B15 | FEC akışları yeniden bağlamaz | R | iki akış FEC açık, her birinde kalıcı kaynak kaybı; aynı `flow_block` numarası iki akışta; BK blok seli | kurtarılan birimin teslim sayısı/kredisi; çapraz akış iptal/teyit; blok tahliyesi; T_block; parite ve küçük paket maliyeti | her blok tek akış; RECOVERED başka akışın birimini teyit etmez; BK seli BE bloğunu tahliye edemez (sınıf başına tavan); kurtarılan birim tam bir kez; maliyet raporlanır | U |
| B16 | Kuyruk derinliği (S7 denetimi) | M · R | her katmanda dosya backlog'u gerçekten var | TEXT sunulduğunda ondan önce hizmet alacak BK baytı, **her bayta neden etiketiyle** (fiziksel iletim / DRR payı / FIFO) | FIFO kaynaklı BK baytı = 0; diğerleri türetilmiş sınırda | U |
| B17 | DUR-38 regresyonu: tek akışta rev6 | M | C15 v3 §2.4 T1, T2 aynen (31 ve 100 B/s; enjeksiyonsuz; ayrı arka plan üreticisi), tek akış; + prob kolu | rev6 Ek B.6 (h) sınırları | rev6 (`37b78aad`) değerleriyle aynı | U |
| B18 | Korunan doğruluk düzeltmeleri | R · N · I | INV-ACK-DELIVER-1 akış başına (sahte ihlalci); store-miss (motor teyitli birime karar üretir); iptal yarışı; 05d RECEIPT oturum değişiminde taşınır; 05g `Ready` olmayan oturumda `open_flow`; INT-02 `ek-tuketici-durak` | sayaçlar, olaylar | ilgili mevcut testler yeni iddialarıyla yeşil; store-miss → boş batch, `stale_decisions` sayılır, sahte koordinat yok; `Closed` sonrası `UnknownFlow`; **bağımlılık:** `ek-tuketici-durak` kolu kodda henüz yok (`1b1425d2:int02_node.rs:198-205`) | U · I |
| B19 | (a) BK kimlik tükenmesi · (b) VO kabulü ve hız düşüşü | R · N | (a) test kancasıyla kimlik uzayı sona yakın, süren iki aktarım (iki yön); (b) r(t) profilin altına iner | (a) yeni aktarımın sonucu, süren aktarımların akıbeti; (b) `vo_rate_deficit`, uygulama olayı | (a) yeni aktarım tipli hata; `RenewRequired`; süren aktarımlar sessiz kaybolmaz (FILE_ABORT(superseded) + olay); (b) tipli "desteklenmeyen profil" olayı; **sesli mesaja otomatik/görünmez geçiş YOK** | U |
| B20 | Netem 7 profil regresyonu (ADR-18, §4.4) | netem (nightly) | ürün kimliği yeni; ADR-hostile-validation-contract düzeni | 7 profil hücre sonuçları | **ön-kayıtlı tahmin:** tek akışlı ürün trafiğinde (yalnız TEXT ya da yalnız dosya) sonuçlar rev6 tabanının hücre CI'ları içinde; kayarsa DUR (yeşile boyanmaz) | U |
| B21 | FILE_RECEIPT gecikmesi — S1 ölçüm tanımı | R · I | dosya kabul edildi; dönüşte yük | başlangıç = yükümlülük doğumu (dosya kabulü / ilk sunum); bitiş = FILE_RECEIPT'in ilk emisyonu; ACK ve karşı uygulamaya teslim ayrı sütun; kesinti dahil/hariç ayrı kol | 120 s yalnız FILE_RECEIPT için geçici deney hedefi (S1); normatif değil; "ölçüldü" | U · I |
| B22 | 31 B/s + kayıpta dosya tamamlanır — S2 zarfı | netem | **ön-kayıtlı yeni hücreler** (HOSTILE-ENVELOPE §1'in dışında olduğu beyanıyla, `0c22a07d:docs/transport-v1/HOSTILE-ENVELOPE.md:1-6`): hız 31 B/s; rastgele kayıp {0, %10}; dönüş yolu kaybı {0, %80 (T6 benzeri)}; kesinti {yok, 1 × 60 s}; dosya 10 KiB [hesap ≈ 330 s serileştirme] | tamamlanma, süre, sebep sayaçları | her hücrede tamamlanma; süre "ölçüldü"; canlı sesin aynı hızda mümkün olduğu iddia edilmez | I |
| B23 | Aynı eşe iki eşzamanlı dosya: biri diğerini kilitlemez | A+ · R | iki aktarım; biri tutulu ya da kayıplı | diğerinin tamamlanması | diğeri tamamlanır (pompa akış başına) | U |
| B24 | Tek kabul yolu: baypas yok | A+ / derleme | `transport_mut()` ile doğrudan `send` denemesi | derleme ya da sonuç | baypas derlenmez ya da bütçeye bağlı yoldan reddedilir | U |
| B25 | Karma sürüm | N | eski ↔ yeni uç | olay, kurtarma baytı | `Ready` öncesi tipli ret; kurtarma baytı = 0 | U |
| B26 | Kontrol çerçevesi DoS sınırları | R | BLOCKED seli; u32::MAX kredi; bozuk çerçeveler (girdi sayısı aşımı, ayrılmış bit, bilinmeyen akış, kip değişimi) | CREDIT hızı ve boyu; gönderen store; ret sayaçları | CREDIT ≤ sınır, amp ≤ 1; store ≤ yerel tavan; her bozuk kol fail-closed + kendi sayacı | U |
| B27 | Bağlam-önce dağıtım FLOW_* için | R (`ctx_first_dispatch` kolu) | FLOW_* çerçevesi HubChannel / NotReady / Mismatch bağlamında | yük işlendi mi | yüke dokunulmadan düşer, sayılır | U |

**Deney dalında koşulanlar (yalnız §14):** B1 (en küçük), B1-taban, B1b, B2 (taban üç kol iki SHA; yeni kod (a), (b)), B3 (isteğe bağlı, sıra 2) + regresyon alt kümesi. Diğerleri U/I'dir; rev1'deki fazladan "D" etiketleri kaldırıldı.

## Ek C — Etkilenen mevcut testler (DUR-8 adayları)

Kaynak: salt okuma taraması, `0c22a07d` (öneksiz) ve `cf270a6c` (`05d:` önekli); hiçbir test koşulmadı; rev2'de motor satırları `37b78aad` (05b rev6) tabanına göre yeniden okundu. Satır = test `fn` satırı (parantezde ana assertion). **İşlem:** **D8** = iddia değişir · **K** = değişmeden yeşil kalmalı (kalmazsa DUR) · **F** = sahte modelin güncellenmesi. **DUR-8 biçimi bu ölçekte (≈190 test) 2×2 ile uygulanamaz:** API değiştiren testlerde "eski kod + yeni test" hücresi derlenmez (C15 v3 §4 emsali: "derlenemedi" kırmızı kanıt sayılmaz). Önerilen toplu yöntem DUR-06-21'dedir (sınıf başına temsilî 2×2 + mutasyon + davranış eşdeğerliği). **Sahte modeller ÜÇ ayrıdır** (rev1'deki tek model uyarısı düzeltildi): `kama-app/tests/common/mock_transport.rs:487-500` oturum geneli HOL (+ tek `MAX_INBOX` :468); `kama-app/tests/file_transfer.rs:419-423` `Link::recover` kayıp birimi sonrakilerin ARKASINA koyar (HOL yok); `kama-app/tests/{ptt,text_receipt,signaling}.rs` ve `05d:kama-app/tests/k4g_p0_receipt.rs` kayıpsız tek FIFO. Yalnız transport'u değiştirmek bu testleri kırmızıya çevirmez; referans sahte A+ (Ek B) ile bilinçli gözden geçirilir.

**C.A — Oturum geneli sıralı teslim (`Reorder`)** — 16 test + 3 sahte
| Test | İddia | İşlem |
|---|---|---|
| `crates/client-core/src/transport.rs:1425` `reorder_delivers_in_unit_order_and_skips_nothing` (:1434) | 0 gelmeden 1 teslim edilmez | D8 → akış başına |
| `transport.rs:1443` `reorder_is_bounded_and_never_skips_a_gap` (:1453) | boşluk atlanmaz, held ≤ cap | D8 → akış başına, cap = kredi |
| `transport.rs:1483` `p0_r1_reorder_never_skips_missing_reliable` (:1499) | 1,2,3,4,0 → [0..4] | K (akış içinde), D8 anahtar |
| `transport.rs:1513` `reorder_reports_window_violation_and_holds_nothing_outside` (:1521, :1528) | pencere ihlali sayılır | D8 → kredi ihlali |
| `transport.rs:1537` `read_only_accessors_on_a_fresh_product_adapter` (:1555) | `reorder_diags()` boş | D8 (teşhis API'si akış başına) |
| `crates/client-core/tests/k4g_p0_receiver.rs:341` `p0_r2_dropped_unit_blocks_followers` (:413, :422) | izleyenler tam sırayla | D8 → aynı akışta K, farklı akışta bloklamaz |
| `client-core/tests/session_keyed_slots.rs:375`, `:442`, `:508` (:585-591, :601-603), `:622` (:640), yardımcı `:338` | oturum başına `next`, rezerv, tekilleşme | D8 (anahtar (oturum, akış)) |
| `kama-integration/tests/common/seam/contract.rs:909` `h_app_retry_0` (:925), `:584` `h_cutover` (:642); mock + real (`seam_conformance_core/main.rs:18,20`) | gönderim sırası | D8 → akış içi sıra; H-* satırları |
| `kama-integration/tests/seam_mock_fixture/main.rs:307` (:325-358) | sahtede HOL | F + D8 |
| `kama-node/tests/signaling_loopback.rs:154` (:165, :175) | blob'lar gönderim sırasıyla | K (sinyalleşme tek akış) |
| `kama-node/tests/two_nodes_loopback.rs:133` (:232) | demetler gönderim sırasıyla (PTT-R0) | D8 (`Live` sırasız; sıra `flow_seq` ile) |
| `kama-app/tests/file_transfer.rs:924` `echo_peer_reassembles_through_mock_loss_and_recovery` | MockTransport HOL'üne dayanır (belirsiz) | F |
| sahteler (üç model, yukarıda) | oturum başına HOL / sonraya kurtarma / kayıpsız FIFO | F → A+ |

**C.B — DATA 0x00 düzeni, `HDR_DATA`, `encode_data`/`decode_data`** — 34 test + 1 fuzz hedefi (öneri B'de S1 kipi FLOW_BATCH'e geçer; 0x00 örtük kipte kalır → yardımcılar ikiye ayrılır)
| Test | İddia | İşlem |
|---|---|---|
| `client-core/tests/api_surface_v0.rs:70` `transport_api_v0_surface_is_frozen` (:373-376, :415-419; `Delivery` :246-257) | wire-v0 imzaları, `Delivery{unit_id}` | D8 (A.19) |
| `fuzz/fuzz_targets/wire_v0.rs:10` (+ 7 tohum `fuzz/corpus/wire_v0/`) | panik yok | D8: yeni decode'lar eklenir |
| `client-core/tests/runtime_e2e.rs:1287` `arq_identity_is_the_pn_not_the_interim_seq_field` (:1264-1268) | `seq u16` alanı okunmaz | D8 (W2'de alan yok; iddia "ARQ kimliği PN" K) |
| `runtime_e2e.rs:423`, `:602`, `:1125` (DropUnitsSeam :402-404) | birim seçimi `decode_data` ile | D8 (akış koordinatı) |
| `client-core/tests/ctx_first_dispatch.rs:628`, `:684`, `:762` (yardımcı :206); `:444`, `:473`, `:501`, `:533`, `:557`, `:580` (ClientRig :331) | ham DATA çerçevesi | D8 (yardımcı güncellenir; bağlam-önce iddiaları K) |
| `session_keyed_slots.rs` yardımcıları `:260`, `:697` → testler `:375, :442, :508, :622, :735, :827, :936, :1042` | ham `encode_data` | D8 yardımcı |
| `client-core/tests/k4g05b_runtime_ownership.rs:316` (NetState) → testler `:862, :912, :1010, :1086, :1165, :1209, :1267, :1419, :1527, :1587, :1639` | röle çerçeveyi `decode_data`/`decode_fec` ile sınıflar | D8 yardımcı; sahiplik iddiaları K |

**C.C — Mesafe kapısı / oturum penceresi / sıra başı rezervi / teslim kanalı** — 18 test
| Test | İddia | İşlem |
|---|---|---|
| `client-core/tests/transport_v1_freeze.rs:104` `sinirlar_ve_kotalar_donmus` (:106-108) | 4096 / 1024 / 64 | D8 (A.12) |
| `k4g05b_runtime_ownership.rs:1639` `sender_distance_gate_holds_offers_and_receiver_never_refuses_window` (:1654, :1660, :1677) | `distance_holds ≥ 1`, `reorder_skip = 0` | D8 → akış kredi kapısı |
| `k4g05b_runtime_ownership.rs:912` (:939-944); Rig penceresi :509-560 | mesafe kapısı içinde kalma | D8 yardımcı |
| `k4g05b_runtime_ownership.rs:1527` (:1496, :1569), `:1587` (:1628) | `full_refusals ≥ 1`, `delivery_dropped = 0` | D8: kredili akışta `full_refusals` yapısal 0, kol testi krediyi aşan sahteyle |
| `session_keyed_slots.rs:508` (:530-533, :537, :578), `:936` (:986-1028), `:1042` (:1067, :1073, :1087) | R = min(max_peers, C − 1); dolu kanal; pencere dışı ret | D8 (S1 kipinde rezerv kalkar; örtük kip K) |
| `k4g_p0_receiver.rs:99` `p0_r2_full_channel_never_acks_undelivered` (:198, :209, :218) | teslimsiz ACK yok | K (INV-ACK-DELIVER-1), fikstür krediyi aşan gönderenle |
| `runtime_e2e.rs:262` `delivery_overflow_refuses_without_ack_not_grown` (:302-306) | taşmada ACK'siz ret | K örtük kipte; S1 kipinde D8 |
| `crates/client-core/src/runtime.rs:4555` `overflow_is_refused_and_drop_mirror_stays_zero` (:4597-4599, :4614-4616) | dolu kanal ACK'siz | K (örtük kip) |
| `runtime.rs:4469` `hub_dispatch_only_counts` (:4484-4492); `ctx_first_dispatch.rs:221` `stats()` (:226-229) | `CtxDispatchStats` tam literal | D8 (yeni sayaçlar) |
| `kama-node/src/node/tests.rs:436` `delivery_channel_full_refuses_and_node_metrics_stay_zero` (:513-518) | Reliable demetler dolu kanalda | D8 (demet `Live`) |
| `node/tests.rs:945` (:1021-1027), `:1049` (:1160-1166) | `full_refusals = 0` (belirsiz) | K |
| `api_surface_v0.rs:77-108` | `RuntimeConfig` tam deseni | D8 (A.12/A.19) |
| zayıf: `kama-mobile/src/event.rs:914`, `kama-mobile/tests/smoke.rs:208` (:267), `kama-node/tests/two_nodes_loopback.rs:123`, `kama-node/tests/start_config.rs:152` | ölçü aynası | D8 (yeni sayaçlar) |

**C.D — FEC bloğu = `unit_id / K`, RECOVERED düzeni** — 16 test + 3 belirsiz kopya (öneri B: çerçeve düzeni aynı, `block_id` anlamı `flow_id ‖ flow_block`; S1 kipinde D8, örtük kipte K)
| Test | İddia | İşlem |
|---|---|---|
| `runtime_e2e.rs:423` (:490, :494-497, :503-506), `:602` (:629), `:1125` (:1161-1163) | ardışık 8 birim tek blok; kısmi blok | D8 → akış başına blok |
| `ctx_first_dispatch.rs:473` (:476, :497) | `encode_recovered(0,&[0])` = birim 0 | D8 (`flow_id` eklenir) |
| `k4g05b_runtime_ownership.rs:1086` (:1138), `:1209`, `:1527` (:1552), `:1587` (:1625) | blok 0 = birimler 0..K−1; ESI adaşı | D8 |
| `adaptive-transport/tests/k4g05b_ownership.rs:770`, `:860`, `:943` (:1064; fikstür :739); `k4g_p0_engine.rs:895`, `:932` | ESI ↔ birim kimliği çakışması | K (motor içi; runtime eşlemesi değişir) |
| `adaptive-transport/tests/budget_equivalence_pin.rs:223` | parite kendi bloğunun kaynağından önce gitmez | K |
| `api_surface_v0.rs:415-419` | RECOVERED codec | D8 |
| `adaptive-transport/src/sender.rs:7790` `dr_s5_4_recovered_cancels_and_ledger` | `RecoveredUnit` (belirsiz) | K |
| belirsiz kopyalar: `adaptive-transport/tests/live_budget_sweep.rs:129`, `policy_outcome_sweep.rs:168`, `kama-integration/tests/common/fs_spine.rs:308`, `loss_structure_observation.rs` | id/K eşlemesinin yerel kopyaları | gözden geçir |

**C.E — Motor seçim sırası** — 33 test + ~7 belirsiz (S7 (b) eklemeli API ile: etiketsiz yol = tek akış = rev6 → K hedefi; satır numaraları `0c22a07d`, rev6'da kayar)
| Test | İddia | İşlem |
|---|---|---|
| `adaptive-transport/tests/k4g05b_scheduling.rs:196, :225, :258, :304, :359` | BURST_CAP tur başı tabanı | K (tek akış) |
| `k4g05b_scheduling.rs:434` `k4g05b_pay_mixed_size_reservation` (:538-554) | K_ARQ_SHARE + DUR-31 rezervi | K tek akışta; seçiciye taşınınca D8 (DUR-38 kararına bağlı) |
| `k4g05b_scheduling.rs:578` (:682-684) | sıfırlama sonrası kopyalar kimlik sırasıyla | K tek akışta; çok akışta D8 |
| `adaptive-transport/tests/k4g05b_ownership.rs:1569, :1577` (d2_assert :1485-1544); `:1597, :232, :315, :379, :404` (belirsiz) | kopya başı en eski birim | K tek akışta |
| `adaptive-transport/tests/k4g_p0_engine.rs:1132`, `:1214`; `:551` (belirsiz) | DATA yükünde ARQ/kopya hizmeti | K |
| `sender.rs:9684` `arq_share_emits_retx_under_token_starvation`; `sender.rs:10150, :10244, :10317, :10403, :10474, :10502` `k4g05b_dur31_*` | pay, rezerv, PRE_CONVERGE DATA önce; rig `tx_queue`'ya doğrudan iter | D8 (iç yapı katman kuyruğuna dönüşür) |
| `adaptive-transport/tests/sprint5c_fec_emit.rs:73` (:98-101); `:43` (belirsiz) | DATA parite öncesi | K |
| `budget_equivalence_pin.rs:27, :163, :181 (:185-196), :203, :223` | Commit-W karar akışı pinleri | K tek akışta; kayarsa DUR |
| `adaptive-transport/tests/dr_cs3_structural_oracle.rs:2857, :2882, :2978, :3038` (ayna :663-680) | `tx_queue` katı FIFO | D8 (sınıf içi FIFO) |
| `adaptive-transport/tests/tv_arq.rs:19 (:39), :57, :106 (:122), :144 (:152), :163` | `DeadlineArqScheduler` sırası | K (A.15) |

**C.F — `OpaqueUnit` / `Event` / `TransmitKind` biçimi** — rev1'de eksikti (W13, F7)
| Kapsam | İşlem |
|---|---|
| `OpaqueUnit` struct literal'leri: **149 yer / 50 dosya** (adaptive-transport 32 dosya, kama-engine 7: `tests/{controller_actuation, emission_continuity, fuzzy_e2e, m1_reverse_nis, micro_sweep_1b, mode_boundary, n8_v2}.rs`, kama-integration 8, client-core 2: `lib.rs:151`, `runtime.rs:2664`, bench 1: `bench/src/kama.rs`) [olgu: `grep -rn 'OpaqueUnit *{'`, `0c22a07d`] | **K** — öneri DUR-06-2 eklemeli API (`Event::AppDataTagged`) ile kaynak değişmez; alana dayalı seçenek seçilirse hepsi D8 olur ve K3 süresi yeniden hesaplanır |
| `api_surface_v0.rs:45` re-export adları; `:70` (:432) `ClientSession::on_event` imzası | D8 (yeni `Event` varyantı) |
| `budget_equivalence_pin.rs:36-47, :100-107`, `dr_cs3_structural_oracle.rs:216`, `kama-integration/tests/common/dr_cs3_panel.rs:2240` — tam `TransmitKind` eşleşmesi | **D8** (yeni `TransmitKind::Control`; rev1'deki K yanlıştı) |
| belge pini `docs/design/P4-TRANSPORT-API-FREEZE.md:126` | D8 (A.28) |
| `tv_*` (OpaqueUnit kurmuyor) | K |

**C.G — S1 yüzey pinleri** — 4 pin + düzenek
| Test | İşlem |
|---|---|
| `crates/kama-contracts/src/s1.rs:225` `closed_sets_are_exhaustive`, `:269` `trait_is_object_safe`, `:275` `transport_error_is_a_std_error_with_stable_messages` | D8 (A.1–A.3) |
| `crates/kama-app/src/signaling.rs:886` (`ALL: [TransportError; 7]`); `05d:kama-app/src/messaging.rs:2447` (belirsiz) | D8 (yeni varyant) |
| `scripts/ci/seam-api-drift.py`, `scripts/ci/seam-coverage.json`, `kama-integration/tests/common/seam/contract.rs:1253, :1292` makroları, `seam_conformance_core/main.rs:17-20`, `common/seam/target.rs:174`, `seam_mock_fixture/main.rs` f1..f12 (:99-576), `seam_real_adapter/main.rs:54` | D8 + yeni H-* satırları |
| `impl KamaTransport for` (yeni metotlar hepsini kırar) — `0c22a07d`: `client-core/src/transport.rs:1198`; `kama-app/src/signaling.rs:647`, `ptt.rs:997`, `messaging.rs:1316`, `:1368`, `file_transfer.rs:1834`; `kama-app/tests/common/mock_transport.rs:396`; `kama-app/tests/ptt.rs:448`, `signaling.rs:319`, `text_receipt.rs:375`, `file_transfer.rs:463`; `kama-integration/tests/common/seam/real.rs:231` (12) · `05d:` ek `messaging.rs:2223` (ScriptedTransport), `tests/k4g_p0_receipt.rs:594` | F |

**C.H — Ürün profili / dondurma pinleri** — 12 + 3 belirsiz
| Test | İşlem |
|---|---|
| `transport_v1_freeze.rs:27` (:28-29) `kimlik_sabitleri_donmus`; `:34, :44, :71, :84, :104` | D8 (DUR-06-13 kimlik; A.12 alanlar) |
| `client-core/tests/product_profile_v1.rs:12, :32, :46, :59` | D8 |
| `bench/src/manifest.rs:330`, `bench/src/kama_product.rs:1035` | D8 (kimlik tek kaynaktan) |
| `api_surface_v0.rs:77-108` | D8 |
| belirsiz: `runtime_e2e.rs:852`, `:1097`, `client-core/src/lib.rs:226` | gözden geçir |

**C.I — PTT `Reliable` ve sıra garantisi** — 16
| Test | İddia | İşlem |
|---|---|---|
| `kama-app/tests/ptt.rs:740` `r0_all_ptt_records_are_reliable` (:760) | üç kayıt da Reliable | D8 (A.18: demet `DeadlineBounded`) |
| `ptt.rs:694` (:714) | Started, Audio×N, Ended sırası | D8 (`flow_seq` sırası) |
| `ptt.rs:817` `bundle_before_open_is_dropped` (:820, :831); `:851` (:858, :871) | aynı oturum sırasına dayanır | D8 (PTT-R3 → ön tampon) |
| `ptt.rs:1167` (:1200-1201), `:1234` (:1252, :1275, :1282), `:1307` (:1327, :1341), `:1458` (:1523-1524), `:1596` (:1623, :1638) | yeni oturumda OPEN demetlerden önce | D8 (OPEN akışın ilk birimi) |
| `ptt.rs:1660` `r0_late_bundles_from_a_retired_session_are_delivered_not_dropped` (:1666-1739) | geç demet asla düşmez | D8 (geç demet oynatılmaz; teslim edilir mi DUR-06-6) |
| `ptt.rs:1818`, `:2111` (:2186) | belirsiz | gözden geçir |
| `kama-node/tests/two_nodes_loopback.rs:133` (:232); `kama-node/src/node/tests.rs:436` (:513) | gerçek transport'ta sıra / dolu kanal | D8 |
| `kama-integration/tests/int02_loopback.rs:367` (:374), `:407` (:417) — `ptt_ok = done && in_order` (`examples/int02_node.rs:692-693`, :1758-1759) | PTT kesintisiz ve sıralı | D8 (B11 ölçüleri) |

**C.J — Kayıtlar arası oturum sırası** — `0c22a07d` 14 · `05d:` 20 (3 belirsiz)
| Test | İddia | İşlem |
|---|---|---|
| `kama-app/tests/file_transfer.rs:757` (:772) | FILE_BEGIN önce | K (aynı akış) |
| `file_transfer.rs:903` `a_receipt_refused_by_the_transport_waits_and_is_not_lost` | alıcının kendi parçaları ortak kuyruğu doldurur | D8 (FILE_RECEIPT kontrol akışında) |
| `file_transfer.rs:1882` (:1914), `:1016`, `:1091`, `:1396` (:1455) | bilinmeyen aktarım / sıra (belirsiz) | gözden geçir |
| `kama-node/src/node/tests.rs:266` (:333) | sonraki TEXT sıra bariyeri | D8 (akışlar arası bariyer yok) |
| `node/tests.rs:836`, `:945`, `:1049` (:1045); `:1476` | "teslim sıralıdır"; FILE_RECEIPT(ok) sonra TEXT | D8 (akışlar arası sıra sözü yok) |
| `kama-node/src/node/signaling_tests.rs:142` (:154, :181) | blob, text, blob sırası | D8 (ayrı akışlar) |
| `kama-app/tests/signaling.rs:615` (:636) | blob'lar sırayla | K |
| `kama-app/tests/text_receipt.rs:585` (:596-598) | RECEIPT varış sırası (belirsiz) | gözden geçir |
| `05d:kama-app/tests/k4g_p0_receipt.rs:1282, :1352, :1440, :1505, :1735, :1919, :2002, :2060, :2269, :2384` | `Rig::with_full_bob_to_alice_queue`: dosya pompası ortak kuyruğu doldurur → RECEIPT bekler | D8 (fikstür ön koşulu artık kurulamaz; iddia "canlı RECEIPT silinmez" K, kurulum CTRL akışının kredisi/tavanıyla yeniden yazılır) |
| `05d:k4g_p0_receipt.rs:1816` (:1859), `:1879` | iki yönlü doygunluk | K (B5'e taşınır) |
| `05d:k4g_p0_receipt.rs:2119` (:2160-2191); `:2335` (belirsiz) | `gated_by_receipts` kapısı | D8 / kaldırılabilir (DUR-06-14) |
| `05d:kama-app/src/messaging.rs:2706, :2774, :2882`; `:2388, :2635, :2830` (belirsiz) | kapı; adopt sırası; "geri basınç oturum başına" | D8 |

**C.K — kama-mobile UniFFI aynası** — 2 + 2 derleme pini
| Test | İşlem |
|---|---|
| `crates/kama-mobile/src/error.rs:552` (:560-569) | K (Backpressure aynası) + yeni varyant |
| `kama-mobile/src/event.rs:914` `metrics_mirror_keeps_every_field` | D8 (yeni sayaçlar) |
| derleme pinleri `error.rs:218-250`, `:374-440` (tam `From` eşlemeleri) | D8 (A.23) |

**C.L — Kapılar (rev1'de yoktu; F13, E4)**
| Kapı | İşlem |
|---|---|
| netem 7 profil (ADR-18, nightly; §4.4) | ön-kayıtlı tahminle yeniden koşu (B20); kayarsa DUR |
| T-hücreleri (ADR-hostile-validation-contract) | yeni kimlikle yeniden koşu (A.26) |
| bench ürün kolu bayt muhasebesi (`bench/src/kama_product.rs`, `bench/src/manifest.rs`) | D8 (kimlik + yeni sınıf sayaçları) |
| `scripts/netem/int02_analyze.py` hükümleri (`1b1425d2:…:18` `delivery_channel_drops = 0`; :969-1011 PTT `in_order_complete`; :1134-1141 öz-test fikstürleri) | D8 (A.25, DUR-06-22) |

**Toplam (kesişimli):** A 16 · B 34 · C 18 · D 16 · E 33 · F 149 literal (eklemeli API'de K) + 4 pin · G 4 + düzenek · H 12 · I 16 · J 14 + 20 · K 2 · L 4 kapı. kama-engine taranmadı değil: A.30 (eklemeli API'de değişmez hedefi); kama-noise-wire ve kama-store testleri A.6/A.35 gereği değişmez hedefidir, uygulama turunda doğrulanır.

## Ek D — DUR'lar (CEVAP 16'ya): seçenekler, öneri, gerekçe — öncelik sırasıyla

C11 §3 süzgeci: DUR yalnız değişmez/sözleşme metni, yeni sayı, tel/vektör, FROZEN dosya, beyanlı sınır ya da bir testin amacı/eşiği değişince. Uygulama tercihleri (ad, dosya düzeni, fikstür ayrıntısı, deney başlangıç değerleri) DUR değildir; orkestratör seçer ve raporlar.

### Öncelik 1 — deneyden ÖNCE

**DUR-06-5 — Birim çerçevesi, FEC kimliği, KARAR-1 eki, `flow_id` uzayı** (Ek A A.8–A.11, A.27). Seçenekler: (W1) birim kimliği telde kalır + akış başlığı, yeni interim çerçeve · (W2) birim kimliği telden çıkar, yeni interim çerçeve (rev1 önerisi) · **(B) KARAR-1 hizası:** akış başlığı ccr §35.14 `BatchSubMessage`'ta, FEC `block_id = flow_id ‖ flow_block` · (B′) KARAR-1'i tarihli sahip ekiyle değiştirip W2'yi canonical yapmak.

| Ölçüt | W1 | W2 | **B** |
|---|---|---|---|
| Başlık, kayıt ≤ 239 B / ≥ 240 B [hesap; bugün 11 B] | 16 / 16 B | 8 / 8 B | **10 / 12 B** |
| KARAR-1 (`1b1425d2:docs/seams/KM-SEAM-TC-001.md:82`: canonical BatchPayload; "iki çerçeve tanımı" reddedildi) | ✗ yeni interim çerçeve | ✗ yeni interim çerçeve | ✓ canonical; KARAR-1'in bekleyen "ayrım ve sıra kimliği" kararını (`batch.rs:28-29`) tamamlar; plan KATMAN 6 "alt mesaj başlığında teslim sınıfı" (`KAMA-PLAN-4GUN-APP-KRIPTO.md:287`) |
| RECOVERED → birim (bugün `block·K + esi`, `0c22a07d:runtime.rs:2379-2382`) | oturum geneli blokta çalışır ama blok akışları bağlar; akış başına blokta tablo gerekir | akış bilgisi yok → çerçeve değişir + tablo | `block_id`'den (akış, sıra) → ters dizin; çerçeve aynı; başka akışın birimi teyit edilemez |
| Akışlar arası blok numarası çakışması (`pending_fec.retain`, `record_recovered_logical`, `repair_store`, alıcı `slot.blocks`) | yok (oturum blokları) | **VAR** (`flow_seq/K` iki akışta aynı) | yok (`flow_id` blok kimliğinde) |
| FIN / sınıf FEC kurtarımında | görünmez (başlık sembol dışında) | görünmez | sınıf/kip `flow_id`'den, FIN `source_flags`'tan → görünür |
| Store-miss bekçisi (`runtime.rs:2263-2288`) | koordinatsız başlık-only çerçeve sorunlu | koordinat yok | boş batch (ccr §35.14 K3: geçerli, PN tüketir, teslim yok) — canonical |
| Alıcı dedup | (akış, sıra) | (akış, sıra) | (akış, sıra) |
| noise-wire §2.6 | değişmez | `block_id`/`message_id` kapsamı değişir | ID-SCOPE kapsam yaması; alan genişlikleri ve FEC düzeni aynı; golden_wire aynı |
| Motor blok tipleri (`SrLogicalRef::Fec`, `PendingFecParity`, `RecoveredUnit`) | aynı | çakışma riski | aynı (u32 tekil) — monoton olmayan blok kimliğine motorun toleransı [çıkarım; deneyde değil U'da doğrulanır] |

**Öneri B.** Gerekçe: KARAR-1 ile çelişmeyen tek seçenek; W2'nin dört gizli maliyetini (RECOVERED eşlemesi, blok çakışması, FIN görünmezliği, store-miss) yapısal olarak kapatır; bugünkü 11 B'den ≤ 1 B fazla. Alt sorular: (i) başlık alt mesajda mı BatchPayload'ın üstünde mi — öneri alt mesajda (KATMAN 6 emsali; tek alt mesajda bayt eşit); (ii) `message_id := flow_seq` (S1 parçalamasız kip); (iii) `flow_id` u16, kalıcı 0–3 + dinamik BK 4…65535, tükenmede `RenewRequired` (eski DUR-06-9; u32 (+2 B) yalnız BK tükenmesi ölçülürse); tükenme [hesap]: akış başına ≤ 65 536 blok × K 8 = 524 288 birim (≈ 531 MB > 64 MiB dosya tavanı), aşımda §2.6'nın "cap/wrap-öncesi renewal" kuralı; (iv) **kredi taşıyıcısı (eski DUR-06-8, KARAR-1'e göre yeniden gerekçe):** FLOW_CREDIT ayrı kontrol çerçevesidir, alıcının ACK yolunda gider. `BatchPayload` canonical modelde bir mühürde birden çok alt mesaj normatiftir, ama alt mesaj yükü AMP kaydıdır (ccr §35.14: "`payload` içi → application-message-protocol.md"); transport kontrolünü AMP yük alanına koymak sahiplik sınırını bozar. S2'de kontrol çerçeveleri ayrı `inner_kind`'dir (§2.56). ACK_V2 ile aynı mühürde birleştirmek FROZEN motor ACK codec'ini ya da "bir mühür = bir iç çerçeve" düzenini değiştirir; bayt kazancı [hesap: datagram başına 22/26 B] B3/B7'de ölçülmeden takas yapılmaz. **Deneyden önce istenen:** yön onayı (B); kesin bayt düzeni ve yama metni deneyden sonra.

**DUR-06-2 — S7 seçim sözleşmesi** (A.17, A.16, A.20). Seçenekler: (a) sığ giriş, motor opak · (b1) `OpaqueUnit`'e alan (149 literal / 50 dosya D8; K3 süresi +1–2 gün [tahmin]) · **(b2) eklemeli `Event::AppDataTagged(OpaqueUnit, ServiceTag)`**, etiketsiz = tek akış = rev6 · (c) client-core'dan enjekte `ServiceSelector`. **Öneri (b2).** Gerekçe: (a) önceden kabul edilmiş kurtarma işini sıralamaz (§6; B2 tabanı ön-kayıtlı KIRMIZI); (b2) pinleri ve literal'leri korur, tek akışta rev6'ya indirgenir (B17); (c) motor kararına motor dışı durum sokar (ADR-17 golden ve SoC denetimi zorlaşır). Bedel: FROZEN v1.1 motor revizyonu ve AMENDMENT-12 cümleleri (A.16). **Deneyden önce:** yön onayı.

**DUR-06-20 — Deney kapsamı ve FEC kolu** (§14). Seçenekler: **(a) §14 kapsamı: yeni kodda FEC kapalı ("ürün profili dışı" etiketi); B2'nin FEC kolu yalnız tabanda ölçülür** · (b) yeni kodda FEC dahil (+2–3 gün; B'nin FEC kimliği deneyde sınanır) · (c) yalnız K1 taban ölçümü. **Öneri (a).** Gerekçe: EK-1 §3 "en küçük uçtan uca kanıt"; S7 hükmü taban kırmızısı + yeni kodda ARQ/kopya yeşiliyle verilir; FEC kolu U aşamasında zorunlu (B2 U). Bu, zorunlu testin deneydeki kapsamını daraltır → sahip onayı.

### Öncelik 2 — deney sonucunu bekler

**DUR-06-1 — Canlı medya kredisi.** (a) VO aynı kredi (birim + bayt): her demet seq kredisi tüketir; süresi dolan demet kredi "deliği" bırakır → atlama bilgisi ve ek bayt gerekir · **(b) ayrı sınırlı kabul:** demet `flow_seq` tüketir ama seq kredisinden muaf; alıcıda P_VO (birim) havuzu; doluysa yalnız `DeadlineBounded` demet düşer, ACK'lenir, `media_dropped_rx`; VO akışındaki `Reliable` birimler (PTT_OPEN/CLOSE) ayrı mutlak sayaç `max_rel` ile sınırlı ve asla düşmez; gönderende VO kovası · (c) karma: küçük pencere + süresi dolan "tüketildi". **Öneri (b).** QUIC DATAGRAM da akış kredisi kullanmaz ve alıcı düşürebilir (RFC 9221 §5.3); güvenilir veri yükümlülüğü doğurmaz. Muhasebe B13 (d)'nin geçer ölçütüdür.

**DUR-06-6 — Live alıcı teslim kipi.** **(a) sırasız + `flow_seq` + kayan dedup penceresi W_LIVE** (W_LIVE ≥ (RTO_MAX + D_VO) / bundle_ms [türetim]; pencere altı geç birim atılır, `media_late_rx`; `Reliable` birim tam bir kez) · (b) sıralı + alıcı boşluk zamanlayıcısı · (c) sıralı + gönderenin "altını atla" bilgisi (+4 B/çerçeve). **Öneri (a).** Gerekçe: "eski ses yeni sesi durdurmaz" yapısal; ek bayt yok; S1 §4 bugün de `DeadlineBounded`'a sırasızlık izni verir (`0c22a07d:docs/seams/KM-SEAM-AT-001.md:153`). rev1'in "ek durum yok" gerekçesi geri çekildi (dedup penceresi gerekir).

**DUR-06-11 — FEC kapsamı.** **(a) akış başına blok; Live ilk uygulamada FEC dışı; `Ordered` akışta kısmi blok T_block içinde kapanır; alıcı blok tavanı sınıf başına** · (b) Live dahil · (c) oturum geneli blok (reddedilir: akışları bağlar). Öneri (a); Live FEC değeri ayrı ölçüm (kısa bütçede parite değeri bütçe/RTT oranına bağlı — `0c22a07d:runtime.rs:1595-1599`).

**DUR-06-10 — VO açıkken BK/BE birim boyu.** **(a) PoC'de beyan + ölçüm** (VO gecikmesi ≥ S_BK/r; B11 gecikme bileşeni sayacı) · (b) sınıf başına dinamik `preferred_plaintext_unit_bytes` · (c) BK için transport parçalaması (S1'de kapalı, `0c22a07d:transport.rs:508`). Öneri (a): aktarım ortasında parça boyu değişemez (alıcı L'yi ilk parçadan öğrenir, `file_transfer.rs:55-63`).

**DUR-06-3 — Kontrol katmanının açık kalan kısmı** ((a) VO-altı tek sıra ve (c) sınırsız öncelik S6 ile zaten reddedildi). Açık olan: akış 0 kredisi (W_CTRL0 ≥ H_C_tavan 12 + RECEIPT payı), ρ_C, b_C'nin normatif sayıları — deney ölçümünden sonra. Envanter:

| Kayıt | Yol | İlerlettiği | Hizmet hakkı | Maliyet sınırı [hesap: mühür 26 B dahil] | 31 B/s örnek | 300 B/s örnek |
|---|---|---|---|---|---|---|
| ACK_V2 (transport ACK; PN) | alıcı sunucu soketi, pacer dışı (`0c22a07d:runtime.rs:3971-3981`) | gönderen teyit, RTT | ≤ DATA/ACK_FREQ_N (2, `adaptive-transport/src/receiver.rs:76`) + kuyruk zamanlayıcısı 20 ms | 1 aralıkta 15 + 4 + 26 = 45 B | 1050 B birimle ≈ 1,3 B/s ters yön | ≈ 6,5–13 B/s ters yön |
| FLOW_CREDIT | alıcı ACK yolu | gönderenin yeni verisi | tetikli (W/2; BLOCKED başına, akış başına ≥ srtt) | 12 + 26 = 38 B | BK W = 64 birim → ≈ 0,04 B/s; blokta ≤ 1,3 B/s | ≈ 0,34 B/s |
| FLOW_BLOCKED | gönderen katman 0 (`TransmitKind::Control`) | alıcının yeniden ilanı | koşullu asgari hak; aralık srtt → ×2 → ≤ 30 s | 38 B (yanıt boyuna dolgulu, amp ≤ 1) | blok bölümünde ≤ 1,3 B/s | aynı |
| FLOW_RESET | akış 0 güvenilir TCTRL birimi | alıcının akış kaynakları | katman 0; yeri açılışta ayrılır | ≈ 10 + gövde + 26 B, iptal başına | iptal başına | iptal başına |
| RECEIPT(delivered) — uygulama alındısı, ACK değildir | akış 0 | gönderen outbox | katman 0; W_CTRL; 05d tut-ve-dur | TEXT başına bir kayıt | TEXT hızıyla | TEXT hızıyla |
| FILE_RECEIPT / FILE_ABORT | akış 0 | aktarım sonu | katman 0; yuva açılışta | aktarım başına ≤ 1 / iptal başına ≤ 1 | — | — |
| PTT_OPEN / PTT_CLOSE | VO akışı, `Reliable` | konuşma başı/sonu | katman 1; `max_rel` | konuşma başına 2 (+ yeniden OPEN) | VO yok (desteklenmez) | konuşma başına |
| PARAM_SET/ACK, RECOVERED, CS_PROBE/ECHO | mevcut (PARAM pacer dışı sınırlı tekrar `runtime.rs:2732-2743`; CS probu ürün profilinde kapalı) | FEC bağlaması, teyit, kuruluş | mevcut | mevcut | — | — |
| SSI1/SSR1 (INIT/RESPONSE; yenileme, supersession) | `EnvelopeLink`, veri runtime'ı dışında (`0c22a07d:transport.rs:152-181`) | oturum kurulumu/yenilemesi (`RenewRequired` yolu dahil) | veri yükünden bağımsız yol | oturum olayı başına | — | — |
| HANDLE_ASSIGN, HEARTBEAT 0x1B/0x1C | hub kanalı | hub yolu | direct-mode/PoC'de yok (hub sprinti, S4) | — | — | — |

Sürekli dosya aktarımında toplam kontrol yükü [hesap]: 31 B/s'de ≈ 1,3 + 0,04 ≈ 1,4 B/s (≈ %4,5); blok bölümü başına her yönde +≤ 1,3 B/s.

**DUR-06-17 — §5/§7 sınırlarının sözleşmeye girişi.** **(a) KM-SEAM-AT-001 §4 ve engine §13'e "koşullu garanti" biçiminde** (05b K2 deseni: koşullar + formül; sabit saniye vaadi yok; fikstür değerleri notta, testte üst sınır) · (b) yalnız notta. Öneri (a): C15 v3 §2.2 "yalnız notta duran sınır sözleşme değildir ve regresyon kapısına bağlanamaz".

### Öncelik 3 — uygulamadan önce (ayrı onay turu)

**DUR-06-18 — PoC-KAPSAM-001 PTT hükmü: sahibin tarihli eki** (A.18, A.19, A.4). Belge "KESİN; değişiklik yalnız Abkarada'nın tarihli ekiyle" (`1b1425d2:docs/decisions/PoC-KAPSAM-001.md:7-8`); §2 PTT "Reliable klip … bozulmaz, durur" (:26), EK-2 md. 1 "geç gelen ses düşürülmez" (:98), EK-3 "kama transport PoC'de yalnız Reliable taşır; gerçek BestEffort BatchPayload geçişiyle" (:27, :103-105). Sahibin C15 v3 §9.3 ve S5 kararı yeni yönü verir ama belgenin kendisi ek ister. Seçenekler: **(a) EK-4: canlı PTT** — OPEN/CLOSE `Reliable`, demet `DeadlineBounded` (D_VO), geç demet oynatılmaz; K8 "sıfır bozuk hece" → "oynatılabilir oran ≥ eşik; eksik demet sessizlik/PLC'dir, bozuk çerçeve oynatılmaz" (sahip tanımlar); EK-3 cümlesi K4G-06'nın BatchPayload geçişiyle kalkar · (b) Reliable klip kalır: PTT kendi VO akışında `Ordered Reliable` (izolasyon yine sağlanır; PTT yalnız kendi kaybını bekler); Live/süre dolumu/dedup penceresi PoC dışı → K4G-06 kapsamı ve süresi küçülür · (c) ikisi: canlı varsayılan + Reliable klip yalnız açık "sesli mesaj" eylemi olarak (S5). **Öneri (a)** (sahibin doğrudan ifadesi: "Canlı konuşmada eski sesi sonsuza kadar kurtarmaya çalışmak yanlış davranıştır"); (b) bilinçli bir kapsam küçültme seçeneği olarak masada. K8 hedefleri her seçenekte B11'in ürün hedefi girdisidir.

**DUR-06-7 — S1 yüzeyi ve `early_unit`** (A.1–A.5, A.7). **(a) `send(flow, …)` (eski `send(session, …)` kalkar) + `open_flow`/`close_flow`/`reset_flow`/`hold_flow` + `FlowOpened`/`FlowEnded` + `UnknownFlow`; `connect(peer, early: Option<(FlowClass, PreparedUnit)>)` — erken birim `Ready`'de o sınıfın kalıcı akışının ilk birimi** · (b) `send_on(flow)` eklenir, `send(session)` gizli BE akışına gider · (c) `hold_flow` yerine `recv_flow`. Öneri (a): gizli varsayılan akış "yan erişimciyle gizleme" yasağına (C15 v3 §9.1) aykırı; `hold_flow` tek okuyucu zincirini korur. VO "desteklenmeyen profil": yeni varyant değil, `Backpressure` + yetenek alanı + olay.

**DUR-06-13 — Sürüm kimliği ve karma sürüm** (A.14, A.26, A.34). Kimlik: (a) `transport-v1.2` · **(b) `transport-v2` / `kama-v2-full`** (S1 trait'i ve iç tel değişiyor). Karma sürüm: **(i) sürüm/yetenek kimliği doğrulanmış el sıkışmada (ccr §35.2/§35.3), `Ready` öncesi fail-closed; Application'a `ConnectAbandoned(LocalPermanent)` (yeni varyant yok) + sayaç** · (ii) legacy AAD `protocol_version` (ccr §35.9) — uyumsuzluk AEAD reddi = sessiz düşüş → kurtarma trafiği; reddedilir · (iii) yalnız beyan (PoC'de iki uç aynı derleme). Öneri (b) + (i); B25.

**DUR-06-14 — 05d A1/M2 devri** (A.21, A.22). **(a) A1 ayrıca kodlanmaz; erişimcisi akış bütçesi/kredi erişimcisine dönüşür; W_F → BK penceresi; H_C → H_C_tavan; C15 v3 §5(ii)'nin "tek kabul yolu ürün derlemesinde zorlanır, `transport_mut()` baypası kaldırılır ya da bütçeye bağlanır" şartı devralınır (B24); M1 aynen; M2 kapısı RECEIPT akış 0'da olduğu için yeniden değerlendirilir** · (b) A1 şimdi kodlanır, sonra değiştirilir. Öneri (a).

**DUR-06-15 — Sinyalleşmenin sınıfı.** **(a) BE, ayrı kalıcı akış (2)** · (b) TEXT akışını paylaşır · (c) CTRL. Öneri (a): blob'lar kontrol birim boyu tavanını aşabilir; TEXT ile aynı akışta birbirini HOL'ler. (Sahibin sınıf atama alanında olduğu için DUR'da tutuldu.)

**DUR-06-12 — Sesli mesaj PoC'de kod mu.** S5 ile davranış karara bağlı (ayrı, görünür; otomatik geçiş yok). Açık kalan yalnız kapsam: **(a) PoC'de yalnız tasarım** · (b) PoC'de kod. Öneri (a).

**DUR-06-21 — Toplu DUR-8 yöntemi (~190 test).** (a) Her test için 2×2 (API değişen testlerde "eski kod + yeni test" derlenmez; C15 v3 §4 emsalinde kanıt değil) · **(b) sınıf başına (Ek C.A–C.L) temsilî 2×2 — eski API ile yazılabilen davranış varyantıyla (ör. B1-taban) — + her D8 sınıfı için ≥ 1 hedefli mutant kırmızı + tek akışlı yapılandırmada eski iddianın yeni API'de aynen tuttuğunu gösteren davranış eşdeğerliği testi** · (c) yalnız mutasyon. Öneri (b).

**DUR-06-22 — INT-02/INT-03 kapı metni** (A.25): "kapasite reddi kaynaklı boşa bayt ≈ 0" → "`capacity_defect = 0` (akış 0 dahil, yalnız geçerli krediye uyan güvenilir veri); ağ kaybı / protokol ihlali / medya expiry / kontrol baytları ayrı sütun; PTT hükmü B11 ölçüleriyle". Öneri: bu metin.

**DUR-06-23 — UniFFI/S5 yüzeyi (Hasan ile ortak onay)** (A.24): `PttBundle.flow_seq` ve olay anlamı; `pttPush` için `DeadlineExpired` ve canlı kipte "reddedileni yeniden ver" kuralının kalkması; `UnknownFlow` aynası; yeni `NodeMetrics` sayaçları. Öneri: bu küme, DUR-06-18 (a) seçilirse; (b) seçilirse yalnız `UnknownFlow` ve sayaçlar.

### Öncelik 4 — Aşama 7

**DUR-06-24 — PTT eşik dosyasının onayı.** Yöntem sahibin 1. notudur (B11) ve DUR değildir. DUR olan: kalibrasyondan sonra `PTT-ESIK-v1`'in (içeriği B11) sahip onayı, kabul koşusundan önce.

**DUR'dan çıkarılanlar:** rev1 DUR-06-4 (yöntem → B11 kuralı), DUR-06-9 (→ DUR-06-5 (iii)), DUR-06-16 ([CAL] başlangıç değerleri: ρ_C 0,1 · Q_BE:Q_BK 3:1 · W0_BE 32 birim/32 KiB · W0_BK 64/64 KiB · `bk_flow_limit0` 4 · `max_rel0` 4 · W_CTRL0 16 · ρ_V,max 0,7 · W_LIVE, D_VO, T_block, N ölçülür — orkestratör seçer, raporlar; normatif olunca DUR-06-3/-17).

## Ek E — EK-1 kapsama tablosu (gövde · Ek A · Ek B · Ek C)

Boş hücre "—" = o ekte madde gerekmiyor; gerekçe "Not" sütununda.

| EK-1 / karar maddesi | Gövde | Ek A | Ek B | Ek C | Not |
|---|---|---|---|---|---|
| §1.1 PTT>TEXT>FILE yalnız uygulama verisi; kontrol ilerleme mekanizması | §4, §5 katman 0 | A.17 | B10 | — | kontrol testleri yeni |
| §1.1 (a) kontrol envanteri; ACK ≠ RECEIPT; hizmet hakkı; toplam maliyet | §4 | — | B10 (a) | — | tablo Ek D DUR-06-3 (31/300 B/s maliyet) |
| §1.1 (b) tavan + koşullu asgari hak | §4 Tavan, §5 m.1/m.5, §7.8 | A.13, A.17 | B10 (a)(b), B26 | — | FLOW_CREDIT tavanı ACK yolunda §7.8 |
| §1.1 (c) kontrol veri kredisine bağımlı değil | §4, §7.4 | A.13 | B10 (c) | — | |
| §1.1 (d) sürekli PTT'de ACK/kredi/OPEN/CLOSE ilerler; sürekli kontrolde veri aç kalmaz | §5 m.5 | — | B10 (a) (ACK_V2 dahil), (b) | — | |
| §1.1 (e) dosya kredisi bitince kontrol için ayrı kaynak | §4, §7.2 | A.13 | B10 (d), B13 (g) | — | |
| §1.1 WMM garanti vermez; DSCP kapsam dışı | §1, §4 | — | — | — | beyan |
| §1.2 OpaqueUnit id+size; kopya/ARQ/parite sırası | §2, §6 | A.17 | B2 | C.E, C.F | rev6 kopya kuralı düzeltildi |
| §1.2 ZORUNLU karşı örnek | §6 hüküm, §14 K1 | — | **B2** (yeniden tasarım) | C.E | FEC kolu deneyde tabanda (DUR-06-20) |
| §1.2 payload opak; generic sözleşme; motor kama-app'e bağımsız | §5 | A.17, A.33 | B17 | C.F | |
| §1.2 S7(a) yalnız uçtan uca kanıtla; (b) güçlü aday | §6, DUR-06-2 | A.17 | B2, B16 | C.E | |
| §1.2 05c oturum geneli pencerenin dönüşümü | §9 | A.12, A.13 | B1 (`distance_holds = 0`), **B1b** | C.C | |
| §1.2 DUR-38 ile ortak seçim/rezerv; geçemeyen mekanizma tutulmaz | §5 | A.17, A.20 | B17, B14 | C.E | taban 05b rev6 `37b78aad` |
| §1.3 oynatılabilir oran, başlangıç, uçtan uca, yük altında, geç oynatılmaz, sebep sayaçları | §10 | A.24, A.32 | B11 | C.I | gecikme bileşeni sayaçları eklendi |
| §1.3 eşikler ürün hedefi + ölçüm (+ sahibin notu 1) | — | — | B11 kuralı | — | K8 hedefleri girdisi; DUR-06-24 |
| §1.3 deadline göndericide; bütün sahiplik zinciri; geç ACK güvenli; iki ilke | §8.3, §10 | A.4, A.16, A.17 | B12 | — | tek sonlandırma çıktısı |
| §1.3 S5: 300 B/s başlangıç; paket boyu ses kabulünün parçası | §10, §11 | — | B11 (birim boyu profilde), B19 (b) | — | DUR-06-10 |
| §1.3 sesli mesaj ayrı, görünür; maliyet ayrı | §10 | — | B19 (b) (otomatik geçiş yok) | — | DUR-06-12 |
| §1.4 açılış: ilk kredi + ilk kontrol | §3, §7.3 | A.5 | B13 (a) (+ erken birim) | — | |
| §1.4 akış sonu/iptal: kullanılmamış kredi, gelmemiş veri, serbest kalma | §7.6 | A.7, A.16 | B13 (b) | — | FIN yuvası; RESET akış 0 birimi |
| §1.4 oturum değişimi sonrası eski kredi/DATA/ACK uygulanmaz | §3, §7.5 | — | B4, B13 (c) | — | bağlam-önce dağıtım mevcut |
| §1.4 medya expiry kredi ve sıra muhasebesi | §7.1, §7.6 | A.4 | B13 (d) | — | DUR-06-1 seçenek başına muhasebe |
| §1.4 aç-kapa kayıt/bellek sınırı | §3 kimlik uzayı, §7.6 | — | B13 (e) (TEXT açıkken) | — | |
| §1.4 yeni OPEN'da gelecekteki CLOSE yükümlülüğüne yer | §7.6 H_C_tavan | A.22 | B13 (f) | — | FILE_ABORT/FILE_RECEIPT/RESET/PTT_CLOSE/RECEIPT |
| §1.4 kullanılmamış taahhüt dahil dosya yükü rezervi tüketemez | §7.2 | A.13 | B13 (g), B10 (d) | — | |
| §1.4 medya kredisi DUR; QUIC stream/datagram karıştırılmaz | §7.1 | — | — | — | DUR-06-1 |
| §1.4 test ayrımı; dört sebep ayrı; kapı yalnız kapasite kusuru (+ sahibin notu 2) | §7.7 | A.25 | B7 (beş alt koşu), sayaç sözlüğü | C.L | akış 0 dahil |
| §1.5 T_birim girdileri | §5 m.4 | — | B14 | — | rev6 formülü akış içi |
| §1.5 sonlu küme; backpressure; token yokluğu ≠ iş yokluğu; vadesiz iş | §5 düzey 1 | — | B6, B14 | — | |
| §1.5 PRE_CONVERGE ve hizmet yaşının başlangıcı | §5 | — | B14 kolu | — | |
| §1.5 B′ + ARQ rezervi "ölçüldü" | §5 | — | B17 | C.E | |
| §2 S1: 120 s yalnız FILE_RECEIPT deney hedefi; ölçüm tanımı | — | — | **B21** | — | rev1'de yoktu |
| §2 S2: 31 B/s dosya kapsamda; ağ zarfı yazılı | §11 | — | **B22** | — | rev1 S2'yi yanlışlıkla B11'e bağlıyordu |
| §2 S3: K7 istisnası ayrı | §0 | Ek A kuralı | — | — | |
| §2 S4: PoC kapsamında | §1 | — | — | — | |
| §2 S5 | §10, §11 | A.19 | B11, B19 | — | PoC-KAPSAM eki DUR-06-18 |
| §2 S6 | §4 | A.13, A.17 | B10 | — | |
| §2 S7 | §5, §6 | A.17 | B2, B16 | C.E, C.F | |
| §3 deney dalı; merge yok; FROZEN yalnız "DENEY"; vektör/seam değişmez | §12, §14 | Ek A kuralı | Ek B son satır | — | |
| §4 takvim: Aralık risk; yeni tarih yok | §14 (tahmin) | — | — | — | |
| §5 her başlığa karşılık; SHA + komut + sonuç; YEREL | bu tablo, §0 | — | ortak kurallar | — | |
| C15 v3 §9.3 "bir dosya diğerini de kilitlemez" | §6, §10 | A.21 | **B23** | — | rev1'de yoktu |
| C15 v3 §5(ii) tek kabul yolu, ürün derlemesinde zorlama | §10 | A.21, A.22 | **B24** | — | rev1'de yoktu |
| C15 v3 §9.4 tel ve şema "sürümleme" | §11 | A.14, A.34 | **B25** | — | karma sürüm |
| PoC-KAPSAM-001 §2 PTT / EK-2 / EK-3 / K8 | Karar özeti 4, §10 | A.18, A.19 | B11 | C.I | DUR-06-18 |
| ccr §35.6/§35.14 + KM-SEAM-TC-001 KARAR-1 | §3, Karar özeti 1 | A.8, A.9, A.27 | B1 (D) | C.B, C.D | DUR-06-5 |

## Ek F — İnceleme izleme tablosu (`k4g06/inceleme-wf_4a961b72.json`, rev1 `2656953c`)

Mercekler: **E** = EK-1 kapsama · **T** = teknik doğruluk · **W** = tel/güvenlik/donma · **F** = uygulanabilirlik. Önem: B = blocker, M = major, m = minor. 73 bulgu: 70 işlendi, 3 kısmen reddedildi (gerekçe satırda).

| Bulgu | Önem | Kısa başlık | Karar | Notta yeri |
|---|---|---|---|---|
| E1 | B | FLOW_DATA × ccr §35.6/KARAR-1/batch.rs | işlendi: W2 geri çekildi, öneri B | Karar özeti 1, §3, A.8, A.9, DUR-06-5 |
| E2 | B | PoC-KAPSAM-001 PTT hükmü sessizce aşılıyor | işlendi: Ek A satırı + ayrı DUR; K8 → B11/DUR-06-24 | Karar özeti 4, §10, A.18, A.19, DUR-06-18, B11 |
| E3 | M | noise-wire §2.6/§3/§2.57, schema eksik | işlendi; A.6 daraltıldı | A.6, A.10, A.11, DUR-06-5 (ii) |
| E4 | M | T-hücre, netem 7 profil, §4.4 yok | işlendi | A.26, B20, C.L |
| E5 | M | `early_unit` (K6) sahipsiz | işlendi | A.5, DUR-06-7, B13 (a) |
| E6 | M | UniFFI/S5 PttBundle, pttPush, AK-001, README-HASAN, node.rs | işlendi | §10, A.24, DUR-06-23 |
| E7 | M | Live kredi/sıra muhasebesi çelişkili | işlendi | §7.1, §7.6, DUR-06-1, B13 (d) |
| E8 | M | Akış 0 akış kontrolü ve taşma sayacı | işlendi: W_CTRL | §4, §7.7, B7 (5) |
| E9 | M | FLOW_CREDIT tavanı ACK yolunda uygulanamıyor | işlendi | §7.4, §7.8, DUR-06-3 tablo |
| E10 | M | FILE_RECEIPT/FILE_ABORT/RECEIPT yeri; H_C_tavan yok | işlendi: H_C_tavan = 12 | §7.6, A.22, B13 (f) |
| E11 | M | Pompa eş başına → dosya dosyayı kilitler | işlendi | §6, §10, A.21, B23 |
| E12 | M | Akışsız emisyonlar, K1 (a), hak kapsamı | işlendi | §5 |
| E13 | M | S1/S2 karşılanmamış | işlendi | B21, B22, §11, Ek E |
| E14 | M | Tek kabul yolu / `transport_mut` devredilmemiş | işlendi | §10, A.21, A.22, DUR-06-14, B24 |
| E15 | m | B10 (a)'da ACK yok | işlendi | B10 |
| E16 | m | B12 motor öncesi zincir, parite kolu | işlendi | B12 |
| E17 | m | PRE_CONVERGE açık değil | işlendi | §5, B14 |
| E18 | m | T_birim akış içi terim | işlendi | §5 m.4, B14 |
| E19 | m | S5: birim boyu, gecikme sayaçları, otomatik geçiş | işlendi | B11, B19 (b) |
| E20 | m | Envanter eksik, toplam maliyet yok | işlendi | DUR-06-3 tablo |
| E21 | m | INV-ACK-DELIVER-1 RESET istisnası | işlendi | A.7, §9 |
| E22 | m | HANDOFF/KNOWN-LIMITATIONS, seam-mutation, kama-engine | işlendi | A.7, A.20, A.30, A.35 |
| E23 | m | RX_BLOCKS_CAP ortak | işlendi | §7.2, B15 |
| E24 | m | Hub kuyruğu gerekçesiz dışarıda | işlendi | §1, §6, A.31 |
| E25 | m | Ek E sütunları; 05c pencere assert'i | işlendi | Ek E, B1, B1b |
| T1 | M | B2 yanlış niceliği ölçüyor; taban SHA yok | işlendi | §6, B2 |
| T2 | M | 05b rev6 yok; kopya iddiası yanlış; T_birim eksik | işlendi | §0, §2, §5, §6, A.17, A.20 |
| T3 | M | Kredi canlılığı: kalıcı durma | işlendi | §7.4, §7.5, B3 (b)(c) |
| T4 | M | Kapalı akış izleme sınırsız; geç kopya kuralı yok | işlendi: kalıcı/dinamik kimlik uzayı, `late_after_close` | §3, §7.6, §9, B13 (e) |
| T5 | M | Live sonlanma, MAX_FLOWS_VO, dedup, düşür+ACK | işlendi: VO kalıcı akış; dedup penceresi; yalnız `DeadlineBounded` düşer | §3, §7.6, §7.7, DUR-06-6 |
| T6 | M | Akış 0 kredisiz; sıra başı yeri kayboluyor | işlendi | §4, §7.7, §9, B10 (d), B7 (5) |
| T7 | M | Deadline/iptal × `take_confirmed_units` ve donmuş cümleler | işlendi: tek sonlandırma çıktısı | D3, §8.3, A.16, B12 |
| T8 | M | ρ_C + ρ_V < 1 yalnız açılışta | işlendi: VO kovası min(profil, ρ_V,max·r(t)) | §5, §10, B19 (b) |
| T9 | m | Kontrol kovasının kapsamı belirsiz | işlendi | §4, §5 m.1, §7.8 |
| T10 | m | DRR indisleri; çağrı sayan pay; hakların kapsamı | işlendi | §5 düzey 1, tek rezerv |
| T11 | m | Kredi formülü, havuz, yerel tampon, değişmezler | işlendi | §6, §7.2, §7.4 |
| T12 | m | BLOCKED/RESET motor taşıması tanımsız | işlendi: RESET = akış 0 güvenilir birimi, BLOCKED = `TransmitKind::Control` | §4, §5, §7.6 |
| T13 | m | `close_flow` boş FIN birimi kredisi | işlendi: FIN yuvası | §7.6 |
| T14 | m | `max_bytes` u32 dosya tavanı | **kısmen reddedildi:** u64 (+4 B/girdi) yerine u32 + açık tavan (`MAX_REASSEMBLY_BYTES` 64 MiB) + tipli hata; alıcı 64 MiB üstünü zaten reddeder | §7.1 |
| T15 | m | Blok tavanı akışları yeniden bağlar | işlendi | §7.2, B15 |
| W1 | M | W2'de FEC kimlik türetimleri tanımsız | işlendi (B ile) | §3, DUR-06-5 tablo, B15 |
| W2 | M | Akış başına blok numarası çakışması | işlendi (B ile) | §3, §8.1, DUR-06-5 |
| W3 | M | FIN FEC kurtarımında görünmez | işlendi: `source_flags` FIN biti | §3, A.11, B13 (b) |
| W4 | M | Store-miss bekçisi; ikinci silme yolu | işlendi: boş batch; sonlandırma çıktısı | §3, D3, §8.3, B18 |
| W5 | M | `TransmitKind` kapalı küme çelişkisi | işlendi | §5, C.F (D8) |
| W6 | M | RESET teyidi yapısal olarak imkânsız | işlendi | §3, §7.6 |
| W7 | M | Kontrol çerçevesi DoS sınırları | işlendi | §7.8, B26 |
| W8 | M | Akış 0 tüketici durunca ACK'siz ret, sayaç yok | işlendi | §4, §7.7, B7 (5) |
| W9 | M | KARAR-1 çelişkisi; DUR-06-8 gerekçesi | işlendi | = E1; DUR-06-5 (iv) |
| W10 | M | noise-wire/şema hizası, codepoint, tek kaynak, `control_id` | işlendi: 0x1D+ K-WIRE-CTRL-1; tek kaynak schema | A.10, A.11, §11 |
| W11 | M | Karma sürüm davranışı tanımsız | işlendi | §11, DUR-06-13, A.34, B25 |
| W12 | M | Live dedup yok | işlendi | §3, DUR-06-6 |
| W13 | m | `OpaqueUnit` alanı 149 literal'i kırar | işlendi: eklemeli API | §5, C.F, DUR-06-2 |
| W14 | m | FEC ölü `seq` alanı yerine `flow_id` | **gereksiz kaldı (kısmen reddedildi):** B'de akış `block_id`'dedir, FEC çerçevesine alan eklenmez; ölü alan S2 geçişinde zaten düşer | §3, A.8 |
| W15 | m | `early_unit` akış ataması | işlendi | = E5 |
| W16 | m | HubChannel'da FLOW_* | işlendi | D1, B27 |
| W17 | m | Yama tabanı bayat (rev6) | işlendi | §0, A.17, A.20, B2 |
| W18 | m | `flow_id` tükenmesinin bedeli | işlendi: VO kalıcı (churn yok), BK 65 532 | §3, DUR-06-5 (iii), B19 (a) |
| W19 | m | Kredi çerçevesi boyu yan kanal | işlendi: sabit boylu girdi + beyan | §7.8, §11 |
| F1 | M | B2 nicelik ve ön-kayıtlı beklenti | işlendi | = T1 |
| F2 | M | D etiketleri §15 ile tutarsız | işlendi | §14, Ek B son satır |
| F3 | M | FEC kolu tabanda ölçülebilir | işlendi | §14 K1, DUR-06-20 |
| F4 | M | Deneyde mesafe kapısının durumu | işlendi | §14 K2, B1b |
| F5 | M | Düzenek sütunu; I'de hedefli düşürme yok; üç sahte model | işlendi | Ek B düzenekler, A.32, Ek C başı |
| F6 | M | Akış 0 kaybı kapıdan kaçabilir | işlendi | §7.7, B7 (5) |
| F7 | M | `OpaqueUnit` literal'leri; K3 tahmini | işlendi | = W13; §14 K3 |
| F8 | M | 2×2 bu ölçekte uygulanamaz | işlendi | DUR-06-21, B1-taban, Ek C başı |
| F9 | m | Rutin DUR'lar | işlendi: 06-4, 06-8, 06-9, 06-16 çıktı; 06-3 daraltıldı; 06-15 gerekçeyle kaldı | §13, Ek D başı |
| F10 | M | Ek A satırlarının DUR bağı; iki eksik DUR | işlendi | Ek A DUR sütunu, DUR-06-17, -20, -22, -23 |
| F11 | m | Eşik dosyası SHA/[CAL]/istatistik | işlendi | B11 |
| F12 | m | Ölçüt belirsizlikleri (B1, B4, B9, B16, B18) | işlendi | Ek B |
| F13 | m | Aşama 0 toplamı, fazlalıklar, regresyon alt kümesi, kapılar | işlendi | §14, C.L |
| F14 | M | Karar özeti, öncelik, Ek C'yi ayrı dosyaya taşıma | **kısmen reddedildi:** özet ve öncelik işlendi; Ek C notta kalır — EK-1 §5 Ek C'nin notta karşılık içermesini ister, tek dosya + tek hash izlenebilirliği korunur | Karar özeti, §13 |
