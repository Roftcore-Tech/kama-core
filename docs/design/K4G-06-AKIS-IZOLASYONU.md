# K4G-06 — Akış izolasyonu ve teslim sözleşmesi — tasarım notu rev5.1 (ADIM 1: sözleşme/arayüz taslağı + ayırt edici test planı)

## 0. Künye

- **Kart / dal / durum:** K4G-06 · `k4g/06-tasarim` (taban main `1b1425d2`) · **rev5.1 TASLAK** (rev5 = `b8b2dac8`; rev4 = `1f0e032c`). Kod yok, ölçüm yok. **Deney kodu (`k4g/06-deney`) bu turda YOK**; dal açılmadı.
- **rev5.1:** rev5'in bağımsız doğrulaması (`k4g06/rev5-wf_77fe089a.json`, üç mercek) işlendi: `Independent` kredisinde tekilleştirme penceresi ile tüketim kredisi ayrıldı ve W_D derinliğindeki kalan beyan edildi (§3, §4.2); tek kredi tanımı ve hariç üst sınır (§7.1, §7.4); erken iptal kaydı R0 tabanlı (YD-5); `FlowEnded` bant içi sırası (YD-10); Live CLOSE→OPEN (§7.6); DUR-06-26 önerisi (a)'ya döndü (= DUR-05f-14); R3 için `resolve_flow` (YD-2); B29/B30/B31 yeniden; öneri/karar ayrımı (§16 başı); Ek A ve Ek F.
- **rev5:** CEVAP 16 v2 §4'ün üç kararı işlendi (Karar özeti, §13, Ek D); DUR-06-20 iki tabanlı deney planı (§14); AMP kontrol kayıtları envanteri ve kontrol sıralama tasarımı (§4.1–§4.2; DUR-06-27, DUR-06-28; B30, B31); ortak yaşam döngüsü bölümü (§16; D_tük = 0 reddi üzerine DUR-06-26 yeniden yazıldı; B29); Ek A değişti; Ek F son bölüm.
- **Sürümler:** rev1 = `2656953c` (inceleme `k4g06/inceleme-wf_4a961b72.json`) · rev2 = `9cda1fab` (doğrulama `k4g06/rev2-dogrulama-wf_86a0d33e.json`; 31 kontrol, 26 yeni bulgu) · rev3 = `dfd3a5e5` · rev4 = `1f0e032c` (05f rev2 `3e0ec8fa` ile dört hizalama) · rev5 = `b8b2dac8` (doğrulama `k4g06/rev5-wf_77fe089a.json`) · rev5.1 = bu sürüm. Bütün turların izlemesi Ek F'de. rev3'ün ana değişiklikleri: KARAR-1 için tarihli ek açıkça gerekli ilan edildi; FEC blok sınırı onarım çerçevesine taşındı (blok kimliği yeniden oturum boyu monoton); transport kontrolü ayrı kalıcı akışa alındı (TCTRL bayrağı kalktı); VO'da `REL` bayrağı; kredi canlılık kuralları tamamlandı; seçicide rezerv hiyerarşisi.
- **Otorite sırası:** CEVAP 16 v2 > CEVAP 15 EK-1 > CEVAP 15 v3 > 14 > …; donmuş belgeler kendi değişiklik kuralıyla (PoC-KAPSAM-001 yalnız sahibin tarihli ekiyle; ccr/noise-wire/transport-tdd yalnız sahip kararıyla). Sahibin iki ek notu bağlayıcı. İnceleme ve doğrulama kayıtları (bağımsız inceleme `decisions/incelemeci/INCELEMECI-8-05b-rev6-inceleme.md`, K4G-06 bölümü dahil) kanıt girdisidir.
- **Atıf biçimi:** `SHA:dosya:satır`. Kısa dosya adları: `runtime.rs`/`transport.rs`/`wire.rs`/`batch.rs` = `crates/client-core/src/`; `sender.rs`/`arq.rs` = `crates/adaptive-transport/src/`; `s1.rs` = `crates/kama-contracts/src/`; `ptt.rs`/`file_transfer.rs`/`messaging.rs` = `crates/kama-app/src/`; `node.rs` = `crates/kama-node/src/`. Belgeler tam yolla.
- **SHA'lar (2026-10-28; CEVAP 16 v2 dayanağı):** `0c22a07d` = `k4g/05c-kod` (05b rev5 `35a02fcf` + 05c) · `37b78aad` = `k4g/05b-retx-fix` rev6 ara ucu (B′, K3, DUR-40; bu notun rev6 satır atıfları buradandır) · **`593029eb` = #140 ucu, 05b rev6 son aday** (Ek A 283 satır `7bfa5e05…158c`; DUR-41 ortak kayıp kanıtı dahil; `37b78aad` atıfları burada kayabilir) — `0c22a07d` rev6'yı İÇERMEZ · `1b1425d2` = main · `cf270a6c` = 05d · `e5514e7a` = 05f rev1 · `3e0ec8fa` = 05f rev2 · `40c59dfb` = 05f rev3.1 (`k4g/05f-dosya-ttl`; §7 G_H türetimi, DUR-05f-14…-18) · `c9c2ea22` = 05g.
- **Ek A sha256:** `⟨SHA256-BEKLİYOR⟩` — rev5.1'de Ek A değişti (A.1, A.2, A.7, A.12, A.13, A.15, A.18, A.21, A.23, A.27, A.32, A.36, A.37); hash orkestratörce hesaplanır (kapsam `## Ek A` satırından `## Ek B`'den önceki son boş olmayan satıra dek, her satır `\n` ile; `~/kama-k4g-work/tools/eka-sha.sh`); 45 satır, 37 kalem (yeni kalem yok). rev5 Ek A (`450bf685…7579`, 45 satır), rev4 (`8f2ce1d4…1adcfe`), rev3 (`18b2ee65…`), rev2 (`013ec64b…`) ve rev1 (`e497640e…`) geçersizdir.
- **Etiketler:** [olgu] koddan · [hesap] aritmetik, ölçülmedi · [çıkarım] test edilmedi · [türetim] koddan önce yazılan sınır · [CAL] deney başlangıç değeri.

## Karar özeti

**CEVAP 16 v2 §4 kararları (yalnız deney yönü; ürün ya da merge onayı DEĞİL):**
- **DUR-06-5 — (B + B-iii) yön GO, yalnız deney dalı için.** KARAR-1 ve ccr §35.14'e tarihli sahip ekinin GEREKLİ olduğu KABUL edildi. Ek şimdi YAZILMAZ; ürün aşamasında Ek A'nın tam yama metniyle yazılır (A.9). Hub'ın gördüğü zarf değişmez.
- **DUR-06-2 — (b2) + (i) yön GO.** Motor içinde tek saf seçici; eklemeli `AppDataTagged(OpaqueUnit, ServiceTag)`; etiketsiz = rev6; rezerv hiyerarşisi. DUR-38 ile ORTAK seçim/rezerv: iki istisna sistemi üst üste büyümez.
- **DUR-06-20 — (a) KOŞULLU.** Deney kodunda FEC kapalı. Deney İKİ tabanda koşar: H `0c22a07d` (tarihsel) ve G güncel birleşik aday (#140 merge + yeniden tabanlanmış 05c, hazır olunca). **Karar verdiren koşu G'dir** (§14). S7(a) reddi ön-kayıtlı (B2).

**rev5'te istenen:** DUR-06-27 (kontrol şeritlerinin teslim kipi; deneyden ÖNCE, çünkü B30 deneyde koşar) · DUR-06-26 ve DUR-06-28 yönü (05f ile ortak; 05f rev3 bunlara göre yazılır). Ürün kodu, merge ya da donmuş metin değişikliği istenmiyor.

1. **Birim çerçevesi (DUR-06-5, KARAR):** KARAR-1'in canonical `BatchPayload` alt mesaj çerçevesi + tarihli ek: v0 kabuk tip baytı, alt mesaj akış başlığı `flags · flow_id u16 · flow_seq u32`, S1 kipinde `submsg_count ∈ {0, 1}`, 0x05 parçalayıcı istisnası; örtük kip de FLOW_BATCH. Başlık 10 B (kayıt ≤ 239 B) / 12 B; bugün 11 B [hesap].
2. **FEC kimliği (B-iii, KARAR):** `block_id` oturum + yön boyu monoton (§2.6 ID-SCOPE değişmez); blok tek akışın ardışık birimleri; blok sınırı onarım çerçevesinde (`first_seq` u32, +4 B/onarım sembolü); kendini tanıtan önek (`message_id := flow_seq`, `chunk_offset := flow_id`, FIN, REL).
3. **S7 (b2) + (i) (KARAR):** tek saf seçici, eklemeli API; katman 0/1 kova payı içinde katman-2 rezervinin önüne geçer; K1 (a) katman 2 içinde aynen.
4. **Kredi ve kontrol:** akış başına mutlak artan sınırlar; tek kredi kuralı `max_seq = min(floor_a + W_D, c + W)` (hariç üst sınır; §3, §7.4); sayım sınırları aynı FLOW_CREDIT'te; BLOCKED iki boyutlu, çok girdili, etkin sınır artana dek tekrarlanır. **Öneri (DUR-06-27 (c)):** kontrol şeritleri (akış 0 ve 1) `Independent` kipte — sırasız, oturum içinde tam bir kez; bağımsızlık W_D derinliğine kadar, W_D zarf içinde erişilemeyecek biçimde türetilir (AMP denetimi §4.1; tasarım §4.2). **Öneri (DUR-06-28 (a)):** Faz G'deki gönderen iptali akış RESET'ine uygulama koduyla bağlanır. Kabul kapısı yalnız `capacity_defect = 0` (sahibin notu 2).
5. **Ortak yaşam döngüsü (§16):** alıcı kredisi, kontrol yükümlülükleri, iptal, FIN ve zaman aşımı tek tabloda; R3 açık S1 çağrısıyla (`resolve_flow`); `FlowEnded` bant içi (YD-10). 05f ve 05d A1 buna atıf verir. D_tük = 0 reddedildi; **öneri (DUR-06-26 = DUR-05f-14 (a)):** FIN sonrası tutma zarfa D_h = 30 s olarak girer. "Tutuluyor" girdisi (b), ancak D > G_H olan tutmalar gerekirse daha ucuzdur.
6. **PTT ve S5:** canlı PTT PoC-KAPSAM-001'in PTT satırını, K8 "sıfır bozuk hece" ölçütünü ve S5 dondurmasını (`:34`, K6 `:54`) değiştirir; tek tarihli sahip ekiyle (EK-4, DUR-06-18). Live'a bağlı DUR'lar (06-1, 06-6, 06-11 Live, 06-23) DUR-06-18 (a)'ya koşulludur.

**Deneyden ÖNCE gerekenler:** DUR-06-27 · §14'ün ön-kayıtları (T_BE, T_BE^rec(H), T_BE^rec(G), B30 taban beklentisi) K1'den önce commit · orkestratörün GO'su.

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

**Üçüncü kip — `Independent`** (yalnız kalıcı kontrol akışları 0 ve 1; DUR-06-27 (c) önerisi): güvenilir, sırasız, oturum içinde tam bir kez. Birim geldiği anda şeridin hazır kuyruğuna girer. Alıcı iki ayrı büyüklük tutar (rev5.1; rev5'teki tek `floor` hem alım hem tüketim anlamında kullanılıyordu):
- **Tekilleştirme penceresi:** `floor_a` = ilk alınmamış `flow_seq`; [`floor_a`, `floor_a` + W_D) bit haritası. `flow_seq < floor_a` ya da biti kurulu birim → ACK'lenir, teslim yok, `dup_ctrl` sayılır. Bellek W_D bit.
- **Tüketim kredisi:** `c` = şerit tüketicisinin aldığı farklı birim sayısı (akış 0'da transport, akış 1'de uygulama).
- **İlan:** `max_seq = min(floor_a + W_D, c + W_c)`; hariç üst sınırdır (§7.1). Alınmış ama tüketilmemiş birim ≤ W_c (bellek sınırı). Durgun tüketicide kredi durur; bu bir kapasite beklemesidir, ACK kesilmez. Bir boşluk ancak arkasında W_D − 1 birim birikince krediyi bekletir.
- **Kalan bağımlılık (beyanlı sınır):** sırasızlık W_D derinliğine kadardır. Kayıp birimin kurtarılması sürerken kontrol kovası en çok b_C + ρ_C·r·T_rec bayt çıkarır. W_D ≥ ⌈(b_C + ρ_C·r·T_rec)/S_min⌉ + 1 ise zarf içinde pencere beklemesi doğmaz [türetim taslağı; T_rec = birimin kendi kurtarma sınırı = 05f T_ctrl(k)]. [hesap] 31 B/s, ρ_C = 0,1, b_C = 64 B [CAL], S_min = 42 B (22 B mühür + 10 B başlık + 10 B RECEIPT kaydı), T_rec = 1 863,6 s (05f rev3.1 `40c59dfb` §7, D-3): ≈ 139 birim → **W_D = 256 [CAL]** (şerit ve oturum başına 32 B). T_rec yaklaşık 1/r ile ölçeklendiği için çarpım hıza bağlı değildir [çıkarım]. Zarf dışında boşluk, `floor_a` ilerleyene dek krediyi bekletir: kayıt düşmez, ihlal sayılmaz.
- FEC ile kurtarılan birim kendini tanıttığı için (önek, yukarıda) aynı yoldan geçer. Oturumlar arası yeniden sunum yeni `flow_seq` alır; onu uygulama kaydın kendi kimliğiyle tekilleştirir (§4.1; FILE_ABORT için sınır §16 YD-5).

**Oturum sınırı:** akışlar oturuma aittir; yenileme/supersession akış taşımaz (S1 K7); bağlam-önce dağıtım eski oturumun DATA/ACK/kredisini yeni oturuma ulaştırmaz [`0c22a07d:transport.rs:56-62`].

## 4. Sınıflar ve kontrol

| Trafik | Sınıf | Akış | Kip / politika |
|---|---|---|---|
| FLOW_RESET (+ uygulama kodu, DUR-06-28) | CTRL (transport) | 0 | `Independent`, `Reliable`; transport geldiği anda tüketir (DUR-06-27) |
| RECEIPT, FILE_RECEIPT, FILE_ABORT (Faz A ve oturum değişimi; DUR-06-28) | CTRL (uygulama) | 1 | `Independent`, `Reliable`, birim boyu tavanlı (DUR-06-27; rev4'te `Ordered`) |
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

**4.1 AMP kontrol kayıtları — sıra denetimi** [olgu: kod ve spec satırları; "sıra gereksinimi" sütunu çıkarım]. Soru (C16 v2 §4): hangi kayıt gerçekten sıra ister, neye göre; kimlikle tekilleştirilebilir mi; kayıp ve tekrar neyi bozar. AMP = `1b1425d2:specs/application-message-protocol.md`; kod `0c22a07d`.

| Kayıt | Üreten → uygulandığı | Şerit (rev5) | Tekilleştirme kimliği | Sıra gereksinimi (neye göre) | Kayıp olursa | Tekrar olursa | Kaynak |
|---|---|---|---|---|---|---|---|
| RECEIPT(delivered) 0x08 | TEXT alıcısı → göndericinin outbox'ı | 1 | (eş, `ref_msg_id`) | **Şeritteki hiçbir kayda göre yok.** TEXT(n)'e göre doğum nedenselliği var (RECEIPT(n), TEXT(n) işlenince doğar); bu bir şerit sırası değildir. Uygulama teslim sırasını `msg_id` ile zorlamaz. `read` (0x02) bugün işlenmez; eklenirse durum kafesi delivered ≤ read, büyüğü alınır | outbox kaydı açık kalır; 05d tut-ve-dur yeniden sunar; oturum değişiminde TEXT eski `msg_id` ile yeniden gider ve RECEIPT yeniden doğar | olay yok (`receipts_ignored`); kalıcı silme idempotent | AMP :68, :73, :75; `messaging.rs:792-795` (yalnız `delivered`), :833-855; test `kama-app/tests/text_receipt.rs:564` (:576-580 tekrar + bilinmeyen → `receipts_ignored` = 2), :585 (sırasız `msg_id` kabul) |
| FILE_RECEIPT 0x0B | dosya alıcısı → göndericinin GİDEN aktarımı | 1 | (eş, `transfer_id`), giden haritası | **Şeritteki hiçbir kayda göre yok.** FILE_ABORT ters yönlüdür ve başka bir kimlik uzayındadır: FILE_RECEIPT benim gelen, FILE_ABORT benim giden aktarımımı anlatır. Aktarımın verisine göre nedensellik: `ok` FILE_END + bütün parçalar + özetten sonra doğar; `rejected` FILE_BEGIN'den hemen sonra doğabilir ve gönderen Faz G'deyken gelir (faz tablosu karşılar) | Faz A'da gönderenin A saati, Faz G'de G saati sürer (05f); yuva düşmez (A.21) | aktarım tablodan çıkmıştır → `receipts_ignored` | AMP :71, :89 ("aktarım başına en çok bir kez"); `file_transfer.rs:1135-1160`, :1598-1625 (`receipt_outcome`); test `src/file_transfer.rs:2349` (iki kayıt zıt yönlere uygulanır) |
| FILE_ABORT 0x05 | dosya göndereni → alıcının GELEN aktarımı | 1 (DUR-06-28 (a) ile yalnız Faz A ve oturum değişimi) | (eş, `transfer_id`), gelen haritası; ilk neden kalır | **Şeritteki kayıtlara göre yok. Aynı aktarımın FILE_BEGIN'ine göre VAR ve şeritler arasıdır:** FILE_BEGIN(t) BK akışı k'nin ilk birimidir, FILE_ABORT(t) akış 1'dedir → FILE_ABORT önce işlenebilir. Bugün yalnız oturumlar arasında olur (erken iptal kaydı); akış ayrımıyla oturum İÇİNDE de olur (§4.2 (D)) | alıcı aktarımı tutar (bugün `FILE_IDLE_TTL`; 05f'de R saati). DUR-06-28 (a) önerisinde Faz G iptali FILE_ABORT ile değil RESET koduyla yapılır | etkisiz. Aktarım çıkmışsa bugünkü kod erken iptal kaydı açar (`file_transfer.rs:1120-1130`). TTL'li bugünkü kodda bu zararsızdır, zamanlayıcısız kayıtta sızar → §16 YD-5: kayıt yalnız bağlanmamış (R0) akış varken doğar, yoksa `aborts_after_terminal` sayılır | AMP :65, :88, :92; `file_transfer.rs:1114-1132`, :1018-1025, :139-156 (kayıt ve türetimi), :295, :1295-1322; `cancel_send` :812-831 (FILE_BEGIN kabulünden önce tele bir şey çıkmaz, :812-825; FILE_END kabulünden sonra `AlreadySent`, :831, :1539-1541) |
| FLOW_RESET (transport) | gönderen transport → alıcı transport | 0 | (oturum, `flow_id`); kapalı akışa gelen = bayat | **Şeritteki kayıtlara göre yok.** Kendi akışının birimlerine göre: RESET'ten sonra gelen birim `late_after_close`; `flow_id > max_seen` → örtük açma + kapalı | alıcı akışın kaynaklarını kurtarılana dek tutar | `stale_ctrl_entries` | §7.6 |
| FLOW_CREDIT / FLOW_BLOCKED (transport) | alıcı ↔ gönderen | şerit dışı (ACK yolu / `TransmitKind::Control`) | mutlak artan sınırlar | **Yok:** büyüğü alınır; sırasız ya da bayat girdi zararsız (B4) | §7.4 (iii) tetiği + BLOCKED tekrarı | idempotent | §7.4–§7.5 |
| `HELD(flow_id, held_ms)` (transport; yalnız DUR-06-26 (b) seçilirse) | alıcı → gönderen | şerit dışı (FLOW_CREDIT çerçevesinde girdi) | (oturum, `flow_id`); `held_ms` tekdüze, büyüğü alınır | **Yok** | sonraki T_BLK'da yinelenir; birikimli olduğu için kayıp birikmez | idempotent | §16 YD-3 |
| FILE_BEGIN / FILE_CHUNK / FILE_END 0x02–0x04 | dosya göndereni → alıcı | BK akışı k (`Ordered`) | FILE_BEGIN (eş, `transfer_id`); parça `offset` bit haritası | **Akış içinde VAR:** FILE_BEGIN önce gelir (ön tampon yok; bilinmeyen aktarımın parçası DROP). Parçalar `offset` adreslidir, birbirine göre sıra gerekmez. Eksik parça varken FILE_END aktarımı bitirmez | yalnız kendi aktarımı bekler | `duplicate_records`; durum bozulmaz | AMP :62-64; `file_transfer.rs:74-76`, :1003-1013 |
| PTT_OPEN / PTT_CLOSE 0x06/0x07 (REL) | konuşan → dinleyen | 4 (`Live`) | (eş, `stream_id`); OPEN idempotent (PTT-R1); bilinmeyen akışın CLOSE'u yok sayılır (PTT-R2) | **Akış içinde VAR:** demetlere göre OPEN önce (sınırlı ön tampon, A.18), CLOSE sonra (§7.6 Live sonlanma kuralı). **REL'ler arası da VAR (rev5.1):** `Live`'da REL birimleri sırasız teslim edildiğinden PTT_CLOSE, kurtarılan PTT_OPEN'dan önce gelebilir; bugünkü kurallarla konuşmanın tamamı düşer (`ptt.rs:820-828`, :778-783, :878-880) → §7.6 kuralı (DUR-06-18 (a)'ya koşullu). Kontrol şeritlerine göre yok | §7.6; yedek PTT-R4 | W_LIVE dedup, tam bir kez (§3) | AMP :66-67, :79-84; `ptt.rs:630-658` (CLOSE tek kez), :112-120 (kapanmış akış kaydının oturumlar arası türetimi) |
| MEDIA_BUNDLE 0x0A | konuşan → dinleyen | 4 (`Live`) | `flow_seq` (W_LIVE) | **Akış içinde VAR:** OPEN'dan önce gelirse ön tampon; CLOSE'a göre §7.6 sonlanma kuralı; oynatma sırası `flow_seq` ile | süre dolumu (`media_expired_tx`); geç demet oynatılmaz | W_LIVE dedup | AMP :70, :79; §3, §7.6 |
| TEXT 0x01 | → | 2 (`Ordered`) | (eş, `msg_id`); dedup 4096 | Uygulama zorlamaz (AMP :73), ama kullanıcı sırası transporttan beklenir → `Ordered` kalır | outbox + yeniden sunum | dedup; RECEIPT yeniden | AMP :61, :73, :75; `messaging.rs:801-821` |
| SIGNALING_BLOB 0x09 | → | 3 (`Ordered`) | kimlik yok; dedup YOK (AMP :98) | **Seam sırayı GARANTİ ETMEZ** (`1b1425d2:specs/signaling-seam-v0.md:43-44`). `Ordered` doğruluk açısından zararsızdır; şerit içi HOL gecikmesi (kayıp bir blob arkadaki trickle-ICE blob'larını bekletir) ölçülmedi → DUR-06-15 aynen; `Independent` mümkün ama gerekmez | yeniden sunum (K4G-14a) | medya motoru idempotent | AMP :69, :98 |

**Denetimin sonucu:**
- Akış 1'in üç kaydının hiçbiri şerit içindeki başka bir kayda göre sıra istemez. RECEIPT ve FILE_RECEIPT kendi kimlikleriyle idempotenttir; kodda ve testte görülür (satırlar tabloda). **FILE_ABORT, aktarım bittikten sonra gelirse kimlikle tekilleştirilemez** (aktarım tablodan çıkmıştır); bunun sınırı §16 YD-5'tedir. Akış 0'ın tek kaydı (FLOW_RESET) akış kimliğiyle idempotenttir. `Independent` kipte bu bağımsızlık W_D derinliğine kadardır (§3).
- Sıra isteyen bağımlılıklar akış içindedir (FILE_BEGIN önce; PTT_OPEN önce) ve kendi akışlarında korunur.
- **Şeritler arası ve şerit–olay bağımlılıkları** (rev5'te yalnız ilki vardı; ikisini rev5.1 doğrulaması buldu). Üçü de kontrol şeridinin kipinden bağımsızdır; akış ayrımının kendisinden doğar:
  1. FILE_ABORT(t) ↔ FILE_BEGIN(t) (§4.2 (D), §16 YD-5);
  2. `FlowEnded { Reset }` olayı ↔ akışın uygulamanın henüz okumadığı birimleri (§16 YD-10);
  3. `Live`'da PTT_CLOSE ↔ PTT_OPEN (§7.6; DUR-06-18 (a)'ya koşullu).
- Bütün kontrol kayıtları körlemesine sırasız yapılmaz: sırasızlık yalnız denetimin bağımsız bulduğu 0 ve 1'e uygulanır. TEXT, BK ve VO'nun akış içi sıra kuralları aynen kalır.

**4.2 Kontrol sıralama tasarımı.**

**(K) Karşı örnek** (C16 v2 §4; INCELEMECI-8, K4G-06 bölümü). rev4'te RECEIPT, FILE_RECEIPT ve FILE_ABORT aynı `Ordered` akış 1'deydi. FILE_RECEIPT(n) kaybolursa RECEIPT(n+1) karşıya ulaşsa bile alıcının akış-1 sıra tamponunda n kurtarılana dek bekler. Böylece TEXT'in "teslim edildi" durumu bir dosya kaydının kurtarılmasına bağlanır. Kontrol katmanının önceliği bunu kaldırmaz: öncelik gönderendeki emisyon sırasını belirler, alıcıdaki sıra beklemesini değil. Ek gecikme ≥ kayıp ilanı + kurtarma emisyonunun hizmeti; 31 B/s'de onlarca saniye [çıkarım; B30-taban ölçer].

**Seçenekler (DUR-06-27):**
- **(a)** Tek `Ordered` akış 1 (rev4): karşı örnek yapısal olarak kırmızı.
- **(b)** Ayrı `Ordered` şeritler: RECEIPT şeridi + dosya kontrol şeridi (FILE_RECEIPT, FILE_ABORT). Karşı örnek yeşile döner. **Kalan:** şerit içinde 1 derinlikte HOL. Bu bağımlılıkların semantik gerekçesi yoktur: RECEIPT(n) kaybı RECEIPT(n+1)'i bekletir; FILE_RECEIPT(t1) kaybı FILE_RECEIPT(t2)'yi ve FILE_ABORT'u bekletir. İkincisi, kontrol düzeyinde "bir dosya diğerini kilitlemez" kuralını (C15 v3 §9.3) bozar. Bedel: bir kalıcı akış daha (kimlikler kayar; uygulama öncesinde bedeli ≈ 0); alıcıda şerit başına sıra tamponu; H_C_tavan bölünür.
- **(c)** **`Independent` kip, akış 0 ve 1** (§3). Teslim sırasızdır, oturum içinde tam bir kezdir: transport `flow_seq` ile tekilleştirir (pencere W_D). Oturumlar arası tekrarı uygulama, kaydın kimliğiyle tekilleştirir (§4.1). Kredi tüketim sayısıyla ilerler ve tekilleştirme penceresinden ayrıdır (§3). **Kalan:** W_D derinliğinde bağımlılık. W_D, zarf içinde erişilemeyecek biçimde türetilir (§3 [hesap]: 31 B/s'de ≈ 139 birim → W_D = 256 [CAL]). Zarf dışında kredi beklemesi olur (kayıp yok, ihlal yok). Karşı örnek ve kardeşleri W_D içinde yeşile döner: FILE_RECEIPT(t1) ↔ (t2), ters yön RECEIPT ↔ FILE_RECEIPT, FLOW_RESET(k1) ↔ (k2). Bedel: yeni `FlowMode::Independent`, yani FROZEN S1 kapalı kümesi değişir (A.1); yeni seam değişmezi (A.7); alıcıda şerit başına W_D bit; kama-app'te FILE_* payı (§7.6).
- **(d)** Kayıt başına sıralama anahtarı (aynı anahtarlı kayıtlar sıralı). Denetim şerit içinde sıra isteyen bir anahtar bulmadı. Anahtar ya transporta uygulama anlamı taşır ya da nesne başına akış ister → reddedilir.
- **Eşit ölçütle karşılaştırma (rev5.1):**

| Ölçüt | (b) ayrı `Ordered` şeritler | (c) `Independent` |
|---|---|---|
| Karşı örnek (FILE_RECEIPT ↔ RECEIPT) | yeşil | yeşil |
| FILE_RECEIPT(t1) ↔ FILE_RECEIPT(t2) | kırmızı (1 derinlik) | yeşil (W_D içinde) |
| RECEIPT(n) ↔ RECEIPT(n+1) | kırmızı (1 derinlik) | yeşil (W_D içinde) |
| Kalan derinlik | 1 | W_D (zarf içinde erişilemez; türetim taslağı) |
| Tel | değişmez | değişmez (ayırt edici değil) |
| S1 / seam | akış kimlikleri kayar; kip kümesi aynı | FROZEN S1 kapalı kümesine `Independent`; yeni seam değişmezi |
| Alıcı belleği | şerit başına sıra tamponu (≤ W birim) | W_D bit + ≤ W_c birim hazır kuyruk |
| Deneyde ölçülen | yok; B30 sonucu [çıkarım] (`Ordered` ve `Independent` kollarından) | B30 `Independent` kolu |

- **Öneri (c).** t1 ↔ t2 ve RECEIPT ↔ RECEIPT bağımsızlığını yalnız (c) verir. Alıcıda yük tamponu yerine anında teslim olur ve 05c `AcceptState` kalıbı yeniden kullanılır. Bedeli FROZEN S1 kapalı kümesinin değişmesidir; sahip kararı gerekir.

**(D) Şeritler arası bağımlılık: FILE_ABORT ↔ FILE_BEGIN (DUR-06-28).**
- **Durum:** rev4'te gönderenin Faz G iptali = FILE_ABORT (akış 1) + `reset_flow` (akış 0). Bu iptal, aktarımın FILE_BEGIN'ini (akış k) geçebilir. FILE_BEGIN, RESET yüzünden hiç teslim edilmeyebilir de: A.7 istisna (ii), ACK'lenmiş ama teslim edilmemiş birim atılır.
- **Bugünkü erken iptal kaydı neden yetmez:** kayıt (`file_transfer.rs:139-156`) yalnız oturumlar arası sırasızlık için türetilmiştir (drain T_g 30 s < `FILE_IDLE_TTL` 60 s; eş başına ≤ 2 kayıt). Oturum içinde "FILE_BEGIN(t) hiç gelmeyecek" sinyali yoktur. Bu yüzden kayıtlar birikir ve sayım tavanında en eski düşer. Düşen kayıt hâlâ gerekli olabilir; o durumda geç gelen FILE_BEGIN hayalet aktarım başlatır. 05f `FILE_IDLE_TTL`'i kaldırdığı için zaman sınırı da kalkar.

Seçenekler:
- **(a)** **Faz G iptali = `reset_flow(k, kod u8)`.**
  - FLOW_RESET gövdesi bir uygulama kodunu opak taşır. Emsal: RFC 9000 §19.4 RESET_STREAM "Application Protocol Error Code". Transport kodu yorumlamaz (D4).
  - Alıcıya `FlowEnded { Reset { kod } }` verilir. Kod değer kümesi = FILE_ABORT.reason (AMP §3).
  - FILE_ABORT üç yerde kalır: Faz A (05f A dolumu), oturum değişimi (`superseded`) ve RESET'i teyit edilmeden oturumu değişen Faz G iptali (nedeni korunur; §16 YD-4).
  - Sonuç: Faz G iptalinde erken iptal kaydı gerekmez. Diğer durumlarda kayıt yalnız eşin bağlanmamış (R0) bir akışı varken doğar ve andığı R0 akışları bağlanınca ya da bitince düşer; zamanlayıcısızdır, bir zaman eşitsizliğine dayanmaz (§16 YD-5, rev5.1). rev5'teki "Faz A'da FILE_BEGIN teyitlidir" öncülü 05f §9 ile çelişiyordu (oturum `floor ≤ fin_seq` iken biterse Faz A teyitsiz başlar) ve kaldırıldı.
  - **Üçüncü durum (rev5.1):** alıcı aktarımı RESET gelmeden bitirmiş olabilir. G dolumu FIN sunulduktan sonra, FIN teyidi gelmeden olabilir; o zaman RESET alıcıda bayattır, dosya alıcıda kalır, gönderen yerel vazgeçme gösterir ve sonradan gelen `ok` yok sayılır. Bu beyanlı bir ayrışmadır (DUR-05f-11). Kullanıcı iptali bugünkü sınırla FILE_END kabulünden önce kalır (`file_transfer.rs:831`, :1539-1541); bu yüzden kullanıcı iptalinde üçüncü durum doğmaz.
  - Bedel: FLOW_RESET +1 B; S1 `reset_flow(flow, code)` ve `FlowEnded { Reset { code } }` (A.1, A.2); AMP §3 FILE_ABORT kullanım metni (A.18); 05f §8.3 G dolumunun 2. adımı kalkar (kod 1. adımda gider).
- **(b)** FILE_ABORT akış 1'de kalır; erken iptal kaydı oturumlu anahtar ve sayım tavanıyla tutulur. Kayıt düşerse hayalet aktarım `FlowEnded { Reset }` ile kapanır, ama neden bilinmez. Yeni tel yok.
- **(c)** Bağımlılık işaretçisi: akış-1 birimine `DEP` bayrağı + `dep_flow u16`. Alıcı transport birimi, `dep_flow`'un ilk birimi teslim edilene ya da akış kapanana dek tutar. Kesin bir çözümdür; ama S1 `send`'e bağımlılık parametresi ve tele bayrak ekler, tek bir kayıt türü için genel bir mekanizma kurar.
- **Öneri (a)** (05f ile ortak).

**Alıcının sonlandırdığı akış:** `rejected`, `aborted_by_receiver` ve 05f R dolumu gibi durumlarda alıcı terminal FILE_RECEIPT'i akış 1'e verir ve akış k'yi yerelde sıfırlar (`stop_flow`, §16 YD-2). Tamponlu birimler atılır, taahhüt serbest kalır, yeni kredi ilan edilmez, sonraki birimler ACK'lenip atılır (`late_after_close` yolu). Gönderen alındıyı alınca birimlerini yerelde iptal eder (`reset_flow_local`; telde FLOW_RESET yok, §16 YD-4).

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
- **Akış kredisi:** `max_seq` (u32; **hariç** üst sınır: veri birimi `flow_seq < max_seq`) ve `max_bytes` (u32; dosya akışında tavan `MAX_REASSEMBLY_BYTES` 64 MiB, `0c22a07d:file_transfer.rs:257`; aşımda tipli hata). Yeniden gönderim/kopya/FEC kurtarımı aynı `flow_seq` → ikinci kredi yok. Açılışta FIN için 1 seq'lik yuva ayrılır: FIN'li boş birim `flow_seq = max_seq`'te de kabul edilir (tampon tutmaz). Alıcı yalnız `flow_seq > max_seq`'i ve FIN'siz `flow_seq = max_seq`'i ihlal sayar (rev5.1; rev5'te `max_seq` dahil/hariç belirsizdi). Kalıcı akışlarda (0–4) FIN yuvası yoktur.
- **Sayım sınırları:** `bk_flow_limit` (u16; §3) ve VO için `max_rel` (u32; REL birim sayısı). İlerleme: `bk_flow_limit` çözümlenen BK akışı başına +1; `max_rel` uygulamaya verilen REL birim başına +1.
- **VO demetleri** `flow_seq` tüketir ama seq kredisinden muaftır; kabul politikası DUR-06-1 (b) (P_VO havuzu; yalnız REL bayrağı olmayan birim düşebilir).

**7.2 Havuz ve taahhüt:** oturum başına sınıf havuzları P_TCTRL, P_CTRL, P_BE, P_VO, P_BK (birim + bayt); taahhüt = Σ_f (`max_seq_f` + FIN yuvası − `c_f`) (`c_f` = akışın tüketicisince alınmış farklı birim sayısı; §7.4) ≤ havuz; BK kredisi yalnız P_BK'den. İki RuntimeConfig değişmezi: Σ W + kontrol tavanı < `max_inflight_units`; `delivery_queue` ≥ `max_peers` × Σ havuz. [hesap] [CAL] değerleriyle oturum başına ≈ 360 birim / ≈ 321 KiB → `max_peers` 64'te ≈ 23 040 birim / ≈ 20 MiB (bugünkü `delivery_queue` 1024, `runtime.rs:271`); bu bellek SR-AUDIT açısından kabul edilemez büyüklüktedir → deneyde etkin S1 oturumu ≤ 8 (≈ 2 880 birim / ≈ 2,5 MiB); normatif sayı DUR-06-3. FEC blok tavanı (`RX_BLOCKS_CAP` 32) sınıf başına bölünür.
**7.3 İlk kredi:** `W0` (sınıf başına), `W_TCTRL0`, `W_CTRL0`, `bk_flow_limit0`, `max_rel0` profil sabitidir; ilk birim ve ilk kontrol kontrol turu beklemez.
**7.4 İlan tetikleri (alıcı):** (i) bir sınır W/2 ilerleyince; (ii) bir BLOCKED bölümünden sonraki **ilk** artış hemen; (iii) **her BLOCKED(girdi) alındığında, güncel sınır girdideki değerleri aşıyorsa güncel sınır yeniden ilan edilir** (akış/sayım başına ≤ 1 / T_min; değişmemiş sınır, gönderen tarafındaki kayıp ilanı kurtarır); (iv) birden çok due girdi tek çerçevede birleştirilir. **Tek kural (rev5.1):** `max_seq = min(floor_a + W_D, c + W)`; `c` = tüketicinin aldığı farklı birim sayısı (akış 0'da transport, diğerlerinde uygulama), `floor_a` = ilk alınmamış `flow_seq`. `Ordered` akışta W_D = W ve `c` = teslim öneki olduğu için formül `c + W`'ye indirgenir. `Independent` akışta W_D ayrı türetilir (§3). `max_bytes` aynı kural: tüketilmiş bayt + W_bayt.
**7.5 Canlılık (gönderen):** **etkin sınır artışı** = bekleyen başı her iki boyutta (`seq`, bayt) açan artış; açmayan artıştan sonra yeni BLOCKED bölümü başlar. BLOCKED girdisi `(flow_id, seq, bytes)` ya da sayım girdisi `(tür, değer)` taşır; bütün bloklu akışlar ve sayımlar tek çok girdili çerçevede (≤ FLOW_CREDIT_MAX_ENTRIES). Aralık 1·srtt → ×2 → tavan min(RTO_MAX = 30 s, `idle_hold`/2); tekrar, etkin sınır artana dek sürer ve alıcının sunucu yuvasını canlı tutar (`idle_hold` 105 s, `runtime.rs:280`). Bu kurallarla "artış ilanı kayboldu, tüketilecek veri kalmadı" durumu (rev2 senaryo A) sonraki BLOCKED + (iii) ile kapanır.
**7.6 Yaşam döngüsü:**
- **FIN:** son birimde bayrak (ya da FIN yuvasındaki boş birim). Alıcı `fin_seq`'e dek her birimi uygulamaya verince akış ve taahhüt serbest; tutulan akışın kapanışı uygulama tüketene dek bekler. **FIN sonrası tutma:** §16 YD-3 (D_tük = 0 reddedildi; iki seçenek, öneri (a) D_h = 30 s; DUR-06-26 = DUR-05f-14; test B29).
- **RESET:** `reset_flow(flow, kod)` → motor akışın birimlerini iptal eder (§8.3) → akış 0'da güvenilir `FLOW_RESET(flow, final_seq, kod u8)` (kod uygulamanındır, transport yorumlamaz; DUR-06-28). İptal ve sonlandırma sinyallerinin tamamı §16 YD-4'te. Akış 0'ın tüketicisi transporttur (uygulama durgun olsa da işlenir); teyidi motorun teyit çıktısıdır. Alıcı akışın tamponunu — ACK'lenmiş ama teslim edilmemiş birimler dahil — atar (INV-ACK-DELIVER-1 istisnası, A.7) ve akışı kapalı işaretler.
- **Kontrol yükümlülüğüne yer — H_C_tavan (eş başına, şerit başına) [türetim]:**
  - akış 1 (uygulama kontrolü): FILE_ABORT ≤ `MAX_OUTGOING_TRANSFERS` 4 + FILE_RECEIPT ≤ 2·`bk_flow_limit0` = 8 = **12**. FILE_RECEIPT'te kabul edilen ve REDDEDİLEN her gelen BK akışı bir alındı doğurur (`0c22a07d:file_transfer.rs:1018-1034`); drain oturumundaki akışların alındıları da güncel oturumun akış 1'inden gider (`:1247`), dolayısıyla eş başına iki oturum sayılır (rev5'te 4 + 4 = 8 idi). RECEIPT(delivered) 05d bit haritasında tut-ve-dur (≤ `MSG_DEDUP_WINDOW`). W_c = W_CTRL0 ≥ 12 + RECEIPT payı; başlangıç 32 [CAL] (rev5: 16). **Pay uygulamada ayrılır:** transport kayıt türünü görmez (D4). kama-app, akış 1'in kullanılabilir kredisi (`FlowTxProgress.available`, A.36) ≤ 12 iken RECEIPT sunmaz; RECEIPT 05d tut-ve-dur kuyruğunda bekler, FILE_* yükümlülükleri bu payı kullanır.
  - akış 0 (transport kontrolü): FLOW_RESET ≤ açık giden BK akışı ≤ eşin `bk_flow_limit0` 4 → W_TCTRL0 ≥ 4.
  - VO: PTT_OPEN + PTT_CLOSE + yeniden PTT_OPEN ≤ 3 / giden konuşma → `max_rel0` ≥ 4 [CAL].
  - FILE_RECEIPT yuvası alıcıda BK akışı açılırken ayrılır; `bk_flow_limit` ancak akış çözümlenip FILE_RECEIPT akış 1'e kabul edildikten sonra ilerler → bekleyen alındı ≤ `bk_flow_limit0`. kama-app'in küresel, en eskiyi düşüren alındı kuyruğu kalkar (A.21). Yuvası ayrılamayan yeni aktarım/PTT → `Backpressure`; mevcut yükümlülük hiç düşmez.
- **Oturum değişimi:** akış taşınmaz; kama-app kuralları (TEXT eski `msg_id`; dosya `FILE_ABORT(superseded)`; PTT yeniden OPEN).
- **Live:** VO akışı kalıcı (churn yok); sıra boşluğu teslimi durdurmaz; konuşma sonu uygulamada: CLOSE alındı ∧ (CLOSE'tan küçük `flow_seq`'li demetler geldi ∨ CLOSE varışından D_VO sonra) → `PttEnded`; yedek PTT-R4 TTL.
- **Live CLOSE → OPEN (rev5.1; DUR-06-18 (a)'ya koşullu):** REL birimler sırasız teslim edildiğinden PTT_CLOSE(s), kaybolup kurtarılan PTT_OPEN(s)'ten önce gelebilir. Bugünkü kurallar bu durumda konuşmanın tamamını düşürür: bilinmeyen akışın CLOSE'u yok sayılıp kapanmış akış kaydına girer (PTT-R2, `0c22a07d:ptt.rs:820-828`); kayıttaki akışa gelen geç OPEN `opens_after_close` sayılır ve akış başlamaz (:778-783); demetler PTT-R3 ile yetim kalır (:878-880).
  - **Kural:** OPEN'ı görülmemiş akışın CLOSE'u, demetlerle aynı sınırlı ön tamponda tutulur. OPEN gelince akış başlar, oynatılabilir demetler verilir ve CLOSE uygulanır ("başladı + bitti"); süresi dolmuş demetler oynatılmaz, sayılır.
  - **Zamanlayıcısız:** REL OPEN oturum içinde kesin gelir (W_LIVE gönderen kapısı, §3); tampondaki CLOSE oturum sonunda atılır.
  - **Tavan:** eş başına `PTT_MAX_ACTIVE_STREAMS` (2) akış × (N demet + 1 CLOSE); aşımda PTT-R5 kuralı.
  - **Kapanmış akış kaydı** (`ptt.rs:112-120`, eş başına 2, ömür 120 s) yalnız oturumlar arası geç OPEN için türetilmiştir. Oturum içinde REL tam bir kez teslim edildiğinden geç bir OPEN yinelemesi gelemez, kayıt gerekmez [türetim taslağı]. B13 (d) kolu.
- **Churn (BK):** açık küme ≤ `bk_flow_limit0`; kapalılık `max_seen` + açık kümeyle temsil edilir (§3).
**7.7 Sayaçlar ve kapı:** krediyi ya da W_CTRL/W_TCTRL'yi aşan birim → ACK'siz ret + `credit_violations`; kredili birim (akış 0/1 dahil) için yer yoksa `capacity_defect`; **kabul kapısı yalnız `capacity_defect = 0`** (sahibin notu 2). VO demetinin havuz yüzünden düşmesi `media_dropped_rx` (ACK'lenir; yalnız REL'siz birim). Kapalı/bilinmeyen akış girdisi, RESET/FIN ile yarışan BLOCKED ya da kredi girdisi → girdi düzeyinde yok sayılır, `stale_ctrl_entries` (ihlal sayılmaz). `Independent` şeritte yinelenen birim → ACK'lenir, `dup_ctrl` (ihlal sayılmaz). Erken iptal kaydı: R0 akışı yokken gelen FILE_ABORT → `aborts_after_terminal`; tavan aşımı → `early_abort_overflow` (rapor; §16 YD-5).
**7.8 Kontrol çerçevesi sınırları (ACK yolu ve DoS):** alıcıda srtt yoktur (`0c22a07d:runtime.rs:3654-3670`) → yanıt aralığı sabit T_min [CAL, ör. 1 s]. **ACK yolunda zorunlu sınır:** sunucu yuvası (eş) başına ≤ 1 FLOW_CREDIT çerçevesi / T_min; due girdiler birleştirilir (≤ FLOW_CREDIT_MAX_ENTRIES); BLOCKED, yanıtının boyuna dolgulanır (amp ≤ 1, emsal `0c22a07d:wire.rs:220-221`). Olağan yük tetiklerle (W/2, BLOCKED) sınırlıdır; T_min yalnız kötü niyetli BLOCKED seline karşı tavandır [hesap: eş başına ≤ 39–72 B/s ters yön (1–4 girdili çerçeve / T_min), yalnız saldırı altında]. Girdiler sabit boylu; çerçeve düzeyinde ret yalnız biçim hatalarında (ayrılmış bit, girdi sayısı aşımı) + kol sayacı. Gönderen store'u daima min(eşin kredisi, yerel sabit tavan) ile sınırlı.

## 8. FEC / ARQ / kopya; süre dolumu ve iptal

**8.1 FEC:** §3 (B-iii). Live ilk uygulamada FEC dışı [DUR-06-11]; `Ordered` akışta kısmi blok T_block içinde kapanır ve blok kimliği yeniden kullanılmaz.
**8.2 ARQ ve kopya** akış etiketini taşır; ARQ_RETX_CAP aşımında kopyaya devir (AMENDMENT-12) aynen; süresi dolan `DeadlineBounded` birimde devir yerine iptal.
**8.3 Tek sonlandırma çıktısı:** motor birime `iptal` durumu verir (`expires_ns` dolumu; RESET: `Event::CancelFlow`). İptal edilmiş birim `tx_queue`, `pending_lost`, `dup_queue` ve parite adaylığından çıkar; H-sahiplenmesi, kayıp ilanı, kuyruk-probu hedefi ve teyit yolları onu atlar; uçuş kayıtları yalnız muhasebede kalır. Çıktı `take_finished_units` = (birim, Teyit | İptal), birim başına oturumda en çok bir kez; aynı olayda çakışmada **teyit kazanır**. Motora hiç verilmemiş birim (offer kanalı, akış başına yerel tampon) payload sahipliği doğurmaz: runtime onu kendi tek çıktısıyla (İptal, birim başına bir kez) düşürür. Bu, AMENDMENT-12'nin ve YAMA KAYDI'nın yedi donmuş cümlesini değiştirir ve INV-NO-IDLE-TEARDOWN'a bir açıklama ekler (A.16).

## 9. 05c kapısının akış başına dönüşümü

`AcceptState` (oturum) → (oturum, akış); pencere = kredi; `RefuseWindow` = kredi ihlali; `RefuseFull` kredili birim için yapısal olarak erişilemez (erişilirse `capacity_defect`). Sıra başı rezervi (`0c22a07d:runtime.rs:3284-3297`) S1 kipinde kredi taahhüdüyle yer değiştirir (akış 0/1 dahil); örtük kip tek akışla aynı kuralla. "Durum unutmaz" (`runtime.rs:3359-3361`) akış ömrü boyunca; kapandıktan sonra `late_after_close`. Mesafe kapısı S1 kipinde kalkar (B1b).

## 10. kama-app ataması, PTT, sesli mesaj, pompa

- **Atama:** §4. Erken birim (`connect` K6): `connect`'e erken birimin sınıfı; `Ready`'de o kalıcı akışın ilk birimi (DUR-06-7).
- **Pompa** akış başına (A.21; B23). **Alındılar** gelen BK akışı başına yuvada; reddedilen FILE_BEGIN dahil (§7.6).
- **Tek kabul yolu (C15 v3 §5(ii)):** her üretici akış açma/gönderme yolundan; `Messenger::transport_mut()` baypası (`0c22a07d:messaging.rs:425`; `file_transfer.rs:1520`, `ptt.rs:960`, `signaling.rs:426`, :583) kaldırılır ya da daraltılır (B24).
- **PTT (DUR-06-18'e bağlı):** canlı kipte demet süresi = sunum + D_VO [CAL]; gönderen süresi dolanı iptal eder; alıcıda OPEN'dan önce gelen demet sınırlı ön tamponda (N [CAL]); geç demet oynatılmaz ve sayılır. Değişen donmuş metinler: AMP PTT-R0/R3 (A.18); PoC-KAPSAM-001 §2 PTT satırı ve "bekliyor" göstergesi, EK-2 md. 1, EK-3, **K8 PTT satırı ("sıfır bozuk hece", (a) seçilirse)**, §2 "Sözleşmeler" S5 dondurması ve K6 "facade değişmez" (A.19).
- **Desteklenmeyen profil:** VO yalnız min(profil, ρ_V,max·r(t)) profili karşılıyorsa açılır ve sürer; değilse tipli hata/olay; sesli mesaja otomatik/görünmez geçiş YOK (S5; B19 (b)). Sesli mesaj ayrı, görünür eylem (DUR-06-12).
- **05d / 05f:** RECEIPT ve FILE_RECEIPT akış 1'de; 05d A1 akış bütçesine dönüşür, tek kabul yolu devralınır (DUR-06-14). **05f rev2** (`3e0ec8fa:docs/design/K4G-05f-DOSYA-ZAMAN-ASIMI.md`): dosya ilerlemesi = akışta **daha önce teyit edilmemiş bir birimin ilk teyidi** (C15 v3 §6 (1)); bayat ACK/SACK ilerleme sayılmaz. Akışın teyit tabanı (`floor`) yalnız **faz sınırıdır** ("FIN dahil önek teyitli" → Faz A başlar); alıcının hareketsizliği tabandan çıkarılmaz. Bekleme saatleri (G, R, A) ve yerel vazgeçme 05f §5–§8'dedir; ortak "kapasite · kuyruk gecikmesi · zaman aşımı" bölümü 05f §4. **Tek yaşam döngüsü (kredi, kontrol yükümlülükleri, iptal, FIN, zaman aşımı) bu notun §16'sındadır;** 05f rev3 ve 05d A1 ona atıf verir (C16 v2 §5 m.4). **05d M2 (rev5.1 önerisi; DUR-06-14):** akış-1 kredisi ve katman 0 varken M2 öncelik kapısı (`cf270a6c:docs/design/K4G-05d-RECEIPT.md:54-61`) kaldırılır. Kalırsa, Backpressure ile reddedilen bir RECEIPT 05d kuyruğunda bekler ve boş olmayan kuyruk, eşin TEXT, dosya, MEDIA_BUNDLE ve sinyalleşme sunumlarını durdurur; böylece akış-1 kredi beklemesi bütün sınıflara yayılır [çıkarım].
- **UniFFI/S5 (DUR-06-23 + DUR-06-18):** `PttBundle`'a `flow_seq` ve anlamı ("akış sırası; geç demet oynatılmaz"); `pttPush` için `DeadlineExpired` ve canlı kipte "reddedileni yeniden ver" kuralının kalkması; `UnknownFlow` aynası; `node.rs` eşlemesine `FlowOpened`/`FlowEnded` (`0c22a07d:node.rs:560-584`). ui-api v0.3 kaynağı `kama-engine docs/ui-api-v0.md` (tag `ui-api-v0.3` = `5de9be16`, KM-SEAM-AK-001:9) — **facade'a dokunur:** oynatıcı sıra bilgisine ihtiyaç duyar ve "bekliyor" davranışı değişir; bu yüzden K6 ve §2 "Sözleşmeler" satırı da EK-4'e girer (A.19, A.24).

## 11. Fiziksel sınırlar, yan kanal, karma sürüm, kapılar

- Eksik parçayı bekleme kalkar, hat meşguliyeti kalkmaz; iletimdeki paket kesilemez: 1050 B 31 B/s'de ≈ 33,9 s, 300 B/s'de 3,5 s [hesap]; BK etkinken VO gecikmesi ≥ S_BK/r (DUR-06-10). Rezerv hiyerarşisiyle bu, VO'nun gördüğü tek BK terimidir (§5 m.5).
- 1,2 kbit/s ses ≈ 150 B/s payload → 31 B/s'de canlı ses yok; 31 B/s'de dosya kapsamda (S2; ağ zarfı B22).
- Kontrol datagramı ≥ mühür 22/26 B + gövde; S2 kontrol gövdeleri ayrıca `control_id` u32 [`1b1425d2:specs/kama-noise-wire-tdd-v2.0.md:347`]: 31 B/s'de tek girdili 13 B'lik kredi çerçevesi ≈ (26 + 13) / 31 ≈ 1,3 s, S2'de (`control_id` +4 B) ≈ 1,4 s hat süresi [hesap].
- **Yan kanal [çıkarım]:** datagram boyu/zamanlaması hub'a açık; sabit boylu girdiler akış sayısı sızıntısını azaltır; kredi ritmi sızar — bilinçli kabul, ölçülür.
- **Karma sürüm:** eski alıcı FLOW_BATCH'i bilinmeyen çerçeve sayıp ACK'siz düşürür [`0c22a07d:runtime.rs:4089`] → sürüm/yetenek el sıkışmada, `Ready` öncesi fail-closed (DUR-06-13; B25).
- **Kalite kapıları:** bench ürün kolu bugün örtük kipte koşar (`0c22a07d:crates/bench/src/kama_product.rs:1`, :36-41). Örtük kip FLOW_BATCH'e geçtiği için netem 7 profil ve T-hücreleri çerçeveyi ve akış başına FEC'i ölçer; **çoklu akış, kredi ve BLOCKED'i ölçmez.** Öneri: bench'e S1-kipi ürün kolu (`kama-product-s1`; manifestte `kip` alanı; B20 ön koşulu) — DUR-06-25. Yapılmazsa A.26'da "T-hücreleri kredi/çoklu akışı ölçmez" beyanı.

## 12. Geçiş sırası ve çakışmalar

1. 05b #140 (rev6) → 05g → 05c merge (05c rev6 üstüne yeniden tabanlanır).
2. **Deney dalı** (§14; merge yok; FROZEN dosyalar yalnız orada "DENEY" etiketiyle).
3. Deney (G) + CEVAP 17 → ürün kodu (API-first; 05d A1 + 05f rev3 + K4G-06 birlikte, §16; C16 v2 §5 m.4): (i) S1 + KM-SEAM-AT-001 + drift · (ii) client-core çerçeve + akış/kredi + alıcı · (iii) motor S7(b) + engine §13 / AMENDMENT-13 · (iv) kama-app + AMP + PoC-KAPSAM EK-4 + 05d A1 / 05f rev3 · (v) UniFFI/S5 + kama-node + INT-02 + bench S1 kolu.
- **Çakışmalar:** 05c kabul/mesafe kodu (§9); 05g etkinleştirme kolları (akış yalnız `Ready` sonrası; `Closed` sonrası `UnknownFlow`); 05d M1 korunur, M2/A1 dönüşür; 05f rev2 akış başına; rev6 K-0…K-5 düzey-2 olur; KARAR-1 geçişi (`0c22a07d:specs/transport-layer-tdd.md:972-974`) K4G-06 ile birlikte.

## 13. DUR özeti (öncelikli; ayrıntı Ek D)

| Öncelik | DUR | Soru | Önerim / karar |
|---|---|---|---|
| **1 — deneyden ÖNCE** | DUR-06-5 | Birim çerçevesi + FEC kimliği + **KARAR-1/ccr tarihli eki** + `flow_id` uzayı | **KARAR (C16 v2 §4): B + B-iii yön GO, yalnız deney; ek gerekli kabul, ürün aşamasında yazılır** |
| 1 | DUR-06-2 | S7 seçim sözleşmesi + rezerv hiyerarşisi | **KARAR: (b2) + (i) yön GO; DUR-38 ile ortak** |
| 1 | DUR-06-20 | Deney kapsamı; FEC kolunun yeni kodda ölçülmemesi | **KARAR: (a) KOŞULLU — iki taban (H, G), karar G'de** (§14) |
| 1 (rev5) | **DUR-06-27** | Kontrol şeritlerinin (akış 0, 1) teslim kipi — kontrol-alındısı karşı örneği | (c) `Independent`: sırasız, tam bir kez; W_D derinliğine kadar (zarf içinde erişilemez, türetim); tekilleştirme ve tüketim kredisi ayrı |
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
| 3 | DUR-06-14 | 05d A1/M2 devri + tek kabul yolu | A1 ayrıca kodlanmaz; M2 kaldırılır (rev5.1) |
| 3 | DUR-06-15 | Sinyalleşme sınıfı | BE, kalıcı akış 3 |
| 3 | DUR-06-12 | Sesli mesaj PoC'de kod mu | hayır |
| 3 | DUR-06-21 | ~190 testte DUR-8 yöntemi | toplu yöntem |
| 3 | DUR-06-22 | INT-02/INT-03 kapı metni | `capacity_defect = 0` + sebep sütunları |
| 3 | DUR-06-25 | netem/T-hücre ölçüm kolu | bench S1-kipi kolu |
| 3 (05f ile ortak; 05f rev3'ten ÖNCE yön) | DUR-06-26 | FIN sonrası tüketici tutması × 05f A saati (**D_tük = 0 REDDEDİLDİ**, C16 v2 DUR-05f-8) | (a) D_h = 30 s zarfa (= DUR-05f-14; rev5'in (b) önerisi geri çekildi) (§16 YD-3) |
| 3 (05f ile ortak; 05f rev3'ten ÖNCE yön) | **DUR-06-28** | Gönderen iptalinin akışa bağlanması (FILE_ABORT ↔ FILE_BEGIN çapraz şerit yarışı) | (a) Faz G'de `reset_flow(k, kod)`; FILE_ABORT Faz A, oturum değişimi ve RESET'i teyitsiz oturum değişiminde (§4.2, §16) |
| **4 — Aşama 7** | DUR-06-24 | PTT eşik dosyası onayı | kalibrasyondan sonra (yöntem sahibin 1. notu) |

DUR'dan çıkarılanlar (C11 §3): rev1 DUR-06-4, -8 (→ 06-5 (iv)), -9 (→ 06-5 (iii)), -16 ([CAL] başlangıç değerleri orkestratörün; normatif olunca 06-3/-17).

## 14. Deney dalı (ADIM 2) — kapsam (DUR-06-20 (a) KOŞULLU: iki taban)

**Durum:** deney kodu bu turda YOK; `k4g/06-deney` orkestratörün GO'suyla açılır. Kurallar EK-1 §3 (merge yok; FROZEN dosyalara yalnız bu dalda "DENEY" etiketiyle; vektör/seam değişmez).

**Tabanlar:**
- **H (tarihsel)** = `0c22a07d` (05b rev5 motoru + 05c). Etiket: "tarihsel; rev5 motoru; FEC kapalı; **karar vermez**".
- **G (güncel birleşik aday; karar verdiren)** = #140 merge'ünden sonraki main (05b rev6 son hâli; bugün `593029eb`) + 05g + main üstüne yeniden tabanlanmış 05c. SHA henüz yok. "Hazır" = C16 v2 §5 m.1–m.2 kapandı ve yeniden tabanlanmış 05c tam yerel kapıdan (1.96.0, `--locked`) geçti. Etiket: "ürün profili dışı (FEC kapalı; deney seçicisi)". **Karar verdiren koşu G'dir:** rev5'in bilinen zamanlayıcı kusurları (kara delik; C16 v2 "iki kalıcı kayıt" (1)) yeni mimari kararını belirlemez.
- rev4'teki `37b78aad` motor-only B2 kolu, G yokken rev6'nın vekiliydi. G rev6'nın son hâlini içerdiği için G'deki B2-taban onun yerine geçer. G gecikirse `593029eb` motor-only koşusu "vekil; karar vermez" etiketiyle koşulur.

**Koşu matrisi:**

| Koşu | H | G | Hüküm |
|---|---|---|---|
| **K1 — taban (kod değişmeden):** B1-taban, B1b-taban, B2-taban (ARQ, kopya, FEC — FEC ürün profilinde açık, `0c22a07d:runtime.rs:292`), **B30-taban** | evet (hemen) | evet (G hazır olunca) | ön-kayıtlı taban beklentisi G'de hükümdür; H tarihsel |
| **K2–K4 — deney kodu:** B1 (en küçük), B1b, B2 (a)(b), **B30 (a)(b)**; B3 (b)(e) isteğe bağlı | G hazır değilse burada yazılır ve koşar | taşınır (ya da doğrudan yazılır) ve koşar | **yalnız G** |
| **Regresyon alt kümesi:** `tv_*`, tek akış `k4g05b_scheduling`, INV-ACK-DELIVER-1 testleri; G'de ayrıca rev6 T1/T2 | evet | evet | G |

**Ön-kayıt (K1'den ÖNCE commit; sonuç görülmeden):** T_BE (B2) · T_BE^rec(H) = (S_iletimde + S_t)/r + K_ARQ_SHARE·τ [türetim: rev5'te katı öncelik; t'nin kurtarma başı yalnız iletimdeki paketi ve kendi token birikimini bekler] · **T_BE^rec(G) = (S_iletimde + S_rez + S_t)/r + K_ARQ_SHARE·τ** [türetim: rev6'da K1 (a) katman 2 içinde aynen; BE başı, kurulu bir BK rezervinin dolmasını en çok S_rez ≤ S_max kadar bekler] · B30 taban beklentisi ve geçme ölçütü · [CAL] değerleri. H ve G aynı ön-kayıtla koşar; G'de sınır ifadesi değişirse yeni ön-kayıt sonuç görülmeden yazılır.

**Kod parçaları:**
- **K1** (kod yok): H ≈ 1 gün; G ≈ 0,5 gün (G hazır olunca).
- **K2 client-core:** FLOW_BATCH (B; FEC kapalı), (oturum, akış) kabul + sıra, örtük açma + `max_seen`/açık küme, akış başına hazır kuyruk + test-util `hold_flow`, tek kredi türü (birim; BLOCKED tek boyutlu `seq`) ACK yolunda §7.4 (i)–(iii) tetikleri ve T_min sınırıyla; S1 kipinde mesafe kapısı kalkar (örtük kip tek akışla); **akış 1 kipi anahtarlı (`Ordered` | `Independent`)**; `Independent` kolunda alıcıda `floor_a` + W_D bit haritası ve tüketim kredisi `c` + W_c (§3; DUR-06-27 (c) önerisi, karar bekliyor); B30 iki kolu da ister. ≈ 2,5–3,5 gün.
- **K3 motor ("DENEY"):** `AppDataTagged` + katman başına DATA kuyruğu + **deney seçicisi = katı öncelik CTRL (akış 1) > BE > BK** (ARQ, kopya, DATA başı için); kova ve DRR yok (beyan; sıra 2, isteğe bağlı). ≈ 1–1,5 gün.
- **K4 testler:** B1-en-küçük, B1b, B2 (a)(b), B30 (a)(b); B3 (b)(e) isteğe bağlı. ≈ 1,25–1,5 gün.
- **Regresyon:** ≈ 0,5 gün.

**Aday hazır değilken (H'de yaz, G'ye taşı):** kod `k4g/06-deney`'de H üstünde yazılır ve koşar; H koşusunun commit'i etiketle korunur. G hazır olunca kod G üstüne yeniden kurulur (aynı dalda yeniden tabanlama ya da `k4g/06-deney-g`; dal adı orkestratörün). **Taşıma yüzeyi [çıkarım; çakışma sayısı ölçülmedi]:**
- (i) K2 — alıcı/gönderen (`runtime.rs`) ve `Reorder` (`transport.rs`): yeniden tabanlanmış 05c ve 05g'nin etkinleştirme kolları (`activate`, `abandon_successor`) aynı dosyalarda → ≈ 0,5–1 gün.
- (ii) K3 — rev6 seçicisi (`refresh_reserve`, `assign_reserve_to_holder`, `ServiceRights`, yaşa göre kopya seçimi, DUR-41 ortak kayıp kanıtı) rev5'ten yapısal olarak farklı. Katı öncelik rev6'nın hak/rezerv yapısına yeniden uygulanır; mekanik taşıma değildir → ≈ 1–1,5 gün.
- (iii) K4, regresyon, G'de K1 ve yeniden koşu → ≈ 0,75–1 gün.
- **Taşıma toplamı ≈ 2,25–3,5 iş günü [tahmin].**

**Süre [tahmin; toplanıp tarih üretilmez, C16 v2 §5]:** G hazırken doğrudan G'de ≈ 6,75–8,5 iş günü (K1 H + G 1,5 · K2 2,5–3,5 · K3 1–1,5 · K4 1,25–1,5 · regresyon 0,5). H'de yazılıp taşınırsa ≈ 6,25–8 + 2,25–3,5 ≈ 8,5–11,5 iş günü. rev4 tahmininden (5,5–7) artış: B30 ve `Independent` kipi (+0,75–1,25) ve ikinci taban (G'de K1, ya da taşıma). **Önerim:** K1 H'de hemen; K2 başlarken G hazırsa kod doğrudan G'de yazılır (taşıma maliyeti doğmaz), değilse H'de.

**Hüküm kapsamı:** deney yalnız şunları verir: S7(a)/(b) ayrımı, kontrol-alındısı bağımsızlığı (B30) ve izolasyonun en küçük kanıtı. FEC kapalı, katı öncelikli bir deneydir. **FEC uyumu, BK açlıksızlığı (DRR), canlı PTT kalitesi ve ürünün bütünü için kabul kanıtı DEĞİLDİR** (INCELEMECI-8, K4G-06 bölümü). Rezerv hiyerarşisi, DRR ve rev6 + seçici etkileşimi U aşamasında sınanır (B10 (e), B14, B17). Öncelik 2–4 kararları deney kanıtına dayanır (CEVAP 17).

## 15. Bu notun sınırı

ADIM 1: sözleşme/arayüz taslağı + ayırt edici test planı. Hiçbir mekanizma uygulanmadı ya da ölçülmedi; §5, §7 ve §16 sınırları türetim taslağıdır. Ek A tam liste olup yama metni değildir.

## 16. Ortak yaşam döngüsü (YD) — alıcı kredisi · kontrol yükümlülükleri · iptal · FIN · zaman aşımı

**Kapsam ve sahiplik.** C16 v2 §3 (DUR-05f-8) ve §5 m.4 şunu ister: alıcı kredisi, kontrol yükümlülükleri, iptal, FIN ve timeout tek yaşam döngüsünde birleşir; ikinci bir geçici oturum-geneli erişimci ya da pencere tasarımı üretilmez. Bu bölüm, bir BK akışının ve onun AMP aktarımının tek yaşam döngüsüdür. Sahiplik:
- **Bu bölüm (K4G-06):** durumlar, geçişler, sinyaller ve her saatin hangi koşulda çalışıp durduğu.
- **05f rev3.x:** sınır sayıları ve zarf türetimi (T_G, T_R, T_A, T_ctrl, G_H, k_f, k_r, ε).
- **05d:** RECEIPT tut-ve-dur.

05f ve 05d A1 bu bölüme `06 §16 YD-n` biçiminde atıf verir. YD numaraları sonraki revizyonlarda değişmez; yeni madde sona eklenir (rev5.1: YD-10).

**Öneri ile karar ayrımı (rev5.1):** DUR-06-26, DUR-06-27 ve DUR-06-28'e bağlı kurallar ÖNERİDİR; sahip kararı bekler. Bu bölümde "DUR-06-nn (x) seçilirse" biçiminde yazılır. Kararlaştırılmış olanlar yalnız C16 v2 §4'teki üç yön kararı ile §3'teki D_tük = 0 reddidir.

**YD-1 Durumlar.**

| Taraf | Durum | Giriş | Çıkış |
|---|---|---|---|
| Gönderen | S0 sunum | `send_file` | FILE_BEGIN akış k'de kabul edildi → S1 · kabulden önce iptal → `Cancelled`; tele bir şey çıkmaz (`0c22a07d:file_transfer.rs:812-825`) |
| Gönderen | S1 Faz G | FILE_BEGIN kabulü | `floor > fin_seq` → S2 (05f §5) · iptal ya da G dolumu → YD-4 · terminal FILE_RECEIPT → YD-4 |
| Gönderen | S2 Faz A | FIN dahil bütün birimler teyitli **ya da** akışın oturumu `floor ≤ fin_seq` iken bitti (05f §9; bu ikinci girişte FILE_BEGIN dahil birimler teyitsiz olabilir) | FILE_RECEIPT → `Delivered` / `SendFailed(statü)` · A dolumu → YD-4 |
| Alıcı | R0 bağlanmamış | `k` açık kümeye girdi; FILE_RECEIPT yuvası ayrıldı (§7.6). Uygulama aktarımı görmez; transport sayar (`FlowRxProgress`, A.36) | ilk birim uygulamaca okundu → R1 · FLOW_RESET(k) → kapalı; aktarım hiç başlamadığı için olay yok (YD-10) |
| Alıcı | R1 bağlı | FILE_BEGIN(t) okundu: (oturum, k) ↔ t | FIN'e dek okuma + özet → FILE_RECEIPT(`ok` / `hash_mismatch`) · `FlowEnded { Reset { kod } }` → `Aborted(kod)` · FILE_ABORT (Faz A) → `Aborted` + `stop_flow` · ret, alıcı iptali, R dolumu → YD-4 |
| Alıcı | R2 FIN kabul edildi, tüketilmedi | FIN dahil bütün birimler kabul edildi; uygulama FIN'i henüz almadı (tutma ya da tüketici durması) | tüketim → R1'in sonu · R2 sürerken YD-3 |
| Alıcı | R3 çözümlendi | `resolve_flow(k)` (YD-2) ∨ RESET işlendi (transport kendiliğinden) | `bk_flow_limit` +1 (§3) |

**YD-2 Kredi, `stop_flow` ve R3 mekanizması.**
- Akış açılınca W0_BK verilir. Kredi yalnız tüketimle ilerler (§7.4 tek kural). Tutma ve tüketici durması krediyi durdurur; bu bir kapasite beklemesidir, saate girmez (05f R1).
- **`stop_flow(k)` (alıcı; yerel sıfırlama):** tamponlanmış ve uygulamaca okunmamış birimleri atar, taahhüdü serbest bırakır, akışı kapalı işaretler. Sonraki birimler ACK'lenip atılır (`late_after_close`; INV-ACK-DELIVER-1 istisna (iii), A.7). Yeni kredi ilan edilmez. FILE_RECEIPT yuvası R3'e dek kalır. Gönderen en çok FILE_RECEIPT varışına dek BLOCKED kalır (≈ 39 B / aralık [hesap]).
- **RESET:** uygulamaca okunmamış birimler dahil tampon atılır (YD-10), taahhüt serbest kalır.
- **R3 mekanizması (rev5.1):** `bk_flow_limit` yalnız R3'te ilerler. Transport AMP'yi görmez (D4), bu yüzden R3 iki yoldan gelir:
  1. RESET işlendi → transport kendiliğinden çözümler (yükümlülük RESET'le düşer).
  2. Alıcı uygulaması `resolve_flow(k)` çağırır (yeni S1 çağrısı, A.2). Çağrı noktaları: FILE_RECEIPT akış 1'e kabul edildiğinde (`send` → `Ok`); FILE_ABORT uygulandığında (Faz A ya da `superseded`; önce `stop_flow(k)`); reddedilen FILE_BEGIN'in alındısı kabul edildiğinde.
  - `resolve_flow` yalnız bitmiş (FIN tüketildi), durdurulmuş ya da RESET'lenmiş akışta geçerlidir; değilse `UnknownFlow` döner. Çağrı tek kabul yolunun parçasıdır (B24) ve B13 (f) / B31 (d) ile sınanır.
  - rev5'te R3 transportun göremediği bir olaya bağlıydı ve Faz A FILE_ABORT yolunda hiç ateşlemiyordu: `bk_flow_limit0` = 4 Faz A zaman aşımından sonra eşin yeni BK akışları oturum boyunca `Backpressure` alırdı.
- Kontrol şeritleri (0, 1) §3 ve §7.4'teki tek kuralla ilerler (W_TCTRL, W_c = W_CTRL; `Independent` kipte W_D ayrıca).

**YD-3 FIN sonrası tüketici tutması — iki seçenek (DUR-06-26 = DUR-05f-14), öneri (a).**
- **Durum:** alıcı R2'de, gönderen S2'de; A saati yalnız alındıyı bekler. C16 v2 DUR-05f-8: **D_tük = 0 REDDEDİLDİ**, çünkü INT-02'nin `ek-tuketici-durak` kolundaki 30 s tüketici durması (DUR-05c-3) kapsam içindeki ürün senaryosudur.
- **(a) Tutma zarfa girdi olarak girer — öneri.** T_A = T_işlem + N_h·D_h + T_ctrl(E); N_h = 1, D_h ≥ 30 s [CAL] (05f rev3.1 §7). Tel değişmez, alıcıda mekanizma yoktur. Bedel: her aktarımın T_A'sına D_h eklenir. D_h'yi aşan tutma zarf dışıdır: vazgeçme mümkündür, > D_h + T_ctrl'de kesindir. Alıcının bütün veriye sahip olduğu durumda bu yanlış "başarısız" demektir (DUR-05f-11 beyanı).
- **(b) Birikimli, tavanlı `HELD` girdisi.**
  - Alıcı, R2 ≥ T_min iken FLOW_CREDIT'e `HELD(flow_id, held_ms u32)` girdisini koyar (11 B; `held_ms` = R2'nin başından beri geçen süre, alıcının monoton saatiyle; u32 ms'de doyar). R2 sürdükçe girdiyi her T_BLK'da yineler.
  - Gönderen: A(t) = max(0, (t − t_S2) − min(H, D_tut,max)); H = alınan en büyük `held_ms`; D_tut,max [CAL; ≥ 30 s; aday 600 s].
  - **Gönderende durum ömrü:** (oturum, akış) ilerleme kaydı FIN teyidinden FILE_RECEIPT'e, A dolumuna ya da `Closed`'a dek tutulur. Bu sürede gelen HELD bayat sayılmaz (§7.7'nin kapalı akış kuralının istisnası). S1'in sonunda, FIN teyidinden önce gelen HELD de H'ye katılır.
  - T_A = T_işlem + T_ctrl(E) + G_H(E). **G_H (05f rev3.1 §7 türetimi):** T_min + (k_r + 1)·(T_BLK + T_min) + N_o·D_o + L. Terimler: ilk girdi ofseti; kayıp girdiler (girdi başına T_min hız sınırıyla); kesinti; son girdinin teslim gecikmesi L (31 B/s'de iletimdeki paket ≈ 33,9 s dahil). [hesap, 05f §7] G_H = 105–506 s. rev5'in G_H taslağı ilk ofseti, hız sınırını ve L'yi atlıyordu.
  - **Fazla düşme:** H, R2'nin başından sayılır ve R2, S2'den önce başlar. Düşülen fazlalık ≤ min(D_tut,max, t_S2 − t_R2); t_S2 − t_R2 = FIN teyidinin geri dönüşü (05f ACK kurtarma terimi D_ack) + N_o·D_o. Güvenli yöndedir (yanlış vazgeçme üretmez), gerçek vazgeçmeyi geciktirir. rev5'in "en çok bir ACK yolu" cümlesi yanlıştı.
- **Karşılaştırma (05f rev3.1 §7 ölçütüyle aynı):**
  - Karşılanacak tutma D ise (a)'nın T_A'ya bedeli D, (b)'ninki G_H'dir. G_H bütün hücrelerde > 30 s olduğundan INT-02'nin 30 s'si için (a) daha ucuzdur.
  - Ölü eş (a)'da daha erken fark edilir (T_işlem + D_h + T_ctrl < T_işlem + G_H + T_ctrl).
  - (b) yalnız D > G_H olan tutmaları (dakikalar) karşılamak gerekirse kazanır.
  - HELD aralığını kısaltmak bunu değiştirmez. L terimi 31 B/s'de tek başına ≈ 73 s'dir (05f L) ve 30 s'den büyüktür. Bant bedeli de artar: 5 s aralıkta ≈ 39 B / 5 s ≈ 7,8 B/s, yani 31 B/s'nin ≈ %25'i [hesap].
  - rev5'in "(b) ölü eşte A'yı uzatmaz" ve "30 s durma tasarımla karşılanır" cümleleri geri çekildi: G_H sabit eşiğe koşulsuz girer, ve 30 s'yi D_tut,max değil G_H karşılıyordu.
- **Öneri (a), D_h = 30 s [CAL].** (b), ürün D > G_H gerektirirse açılır; örneğin INT-03'te mobil cihazda dakikalarca süren FIN sonrası tüketici durması ölçülürse. Tel ve Ek A'daki HELD kalemleri "(b) seçilirse" koşulludur. Karar sahibindir. Hangi seçenek olursa olsun saat bu karardan önce kodlanmaz (C16 v2 §3).
- **Beyanlar (iki seçenekte de):**
  - (i) Alıcı süreci askıya alınırsa (mobil arka plan; monoton `Instant` askıyı saymaz) ne alındı ne HELD gider. A işler; D_h, (b)'de G_H/D_tut,max aşılınca gönderende yerel vazgeçme olur. Yanlış "başarısız" mümkündür, yanlış "teslim edildi" yoktur (DUR-05f-11). INT-03'te arka plan sıradan bir durumdur; ölçülecek [çıkarım].
  - (ii) Faz A'da kullanıcı iptali yoktur (`cancel_send` → `AlreadySent`, `file_transfer.rs:831`). Kullanıcının azami beklemesi (a)'da T_A(E), (b)'de T_A(E) + D_tut,max. 05f D-3 hücresinde (a) 1 894,6 s, (b) 2 370,6 s + 600 s [hesap, 05f rev3.1 §7].

**YD-4 İptal ve sonlandırma sinyalleri** (DUR-06-28 (a) seçilirse; seçilmezse rev4 kuralı: Faz G iptali = FILE_ABORT + `reset_flow`).

| Kim / ne zaman | Sinyal | Alıcıda | Yükümlülük |
|---|---|---|---|
| Gönderen, S0 | yok | — | yok |
| Gönderen, S1 (Faz G): kullanıcı iptali (yalnız FILE_END kabulünden önce; bugünkü sınır `file_transfer.rs:831`, :1539-1541 korunur) ya da G dolumu | `reset_flow(k, kod)` (kod: `user_cancel` / `timeout`); **FILE_ABORT YOK** | `FlowEnded { Reset { kod } }` (YD-10): R1/R2'de `Aborted(kod)`; R0'da olay yok. G dolumu FIN sunulduktan sonra olduysa alıcı aktarımı bitirmiş olabilir: RESET bayattır, dosya alıcıda kalır (beyanlı ayrışma, DUR-05f-11) | RESET'le düşer → R3 (transport) |
| Gönderen, Faz G iptali sonrası, RESET teyit edilmeden oturum değişti | yeni oturumda FILE_ABORT(kod) — bugünkü AMP :92 kuralının karşılığı: bekleyen kullanıcı iptali yeni oturumdan gider, nedeni korunur | `Aborted(kod)` ya da erken iptal kaydı (YD-5) | 05f §9 |
| Gönderen, S2 (Faz A): A dolumu | FILE_ABORT(`timeout`) akış 1 | bağlıysa `Aborted(timeout)` + `stop_flow` + `resolve_flow`; bağlı değil ve R0 varsa erken iptal kaydı; R0 yoksa `aborts_after_terminal` (YD-5) | FILE_ABORT uygulanınca düşer → `resolve_flow` |
| Gönderen, oturum değişimi (FIN kabul edilmemiş) | yeni oturumda FILE_ABORT(`superseded`) | `Aborted(superseded)` ya da kayıt (YD-5) | 05f §9 |
| Alıcı: FILE_BEGIN reddi, kullanıcı iptali, R dolumu | `stop_flow(k)` + FILE_RECEIPT(`rejected` / `aborted_by_receiver` / `timeout`) akış 1 | gönderen S1'de `SendFailed(statü)` + **`reset_flow_local(k)`** (motor `CancelFlow`; telde FLOW_RESET YOK); S2'de `SendFailed(statü)` | FILE_RECEIPT kabulünde `resolve_flow` |
| Herhangi biri: oturum sonu | `Closed` | 05f §9 tablosu aynen | 05f §9 |

- Her aktarımın tek terminal olayı vardır (05f P2, P5; ilk neden kalır). Yarışan iki sinyalde (RESET ve FILE_RECEIPT) ilk işlenen kazanır; ikincisi `receipts_ignored` ya da bayat girdi olarak sayılır.
- rev5'teki `reset_flow(k, 0x00)` yanıtı kalktı. O yanıt W_TCTRL0 sınırını bozuyordu (alıcının R3'ü RESET teyidinden önce gelir; bekleyen FLOW_RESET 2·`bk_flow_limit0`'a çıkabilirdi) ve `stale_ctrl_entries`'i her olağan retle kirletiyordu.
- **Bilinmeyen RESET kodu:** akış yine sıfırlanır. Uygulama olayı `Aborted { reason: Unspecified }` olur (A.37). FILE_ABORT'un "bilinmeyen → DROP" kuralı RESET'e uygulanamaz, çünkü akış zaten atılmıştır. Artık üretilmeyen 0x02/0x03 kodları da `Unspecified` sayılır.

**YD-5 Erken iptal kaydı (alıcı) — R0 tabanlı, zamanlayıcısız (rev5.1).**
- **Doğum:** kayıt yalnız eşin canlı (`Closed` olmamış) bir oturumunda bağlanmamış (R0) bir gelen BK akışı varken doğar. Yoksa kayıt açılmaz, `aborts_after_terminal` sayılır; tipik örnekler A dolumunun alıcının bitirdiği aktarıma gelmesi ve FILE_ABORT tekrarı. rev5 kuralı bu durumda kodun açtığı kaydı (`0c22a07d:file_transfer.rs:1120-1130`) oturum boyunca tutuyordu.
- **Anahtar ve bağ:** (eş, `transfer_id`). Kayıt, doğduğu andaki R0 akışlarının kümesini (oturum, akış) anar.
- **Düşme:** FILE_BEGIN(t) okundu (→ FILE_RECEIPT(`rejected`), bugünkü kural); ya da andığı R0 akışlarının hepsi bağlandı ya da bitti (RESET, `stop_flow`, oturum `Closed`). FILE_BEGIN akışın ilk birimidir; bu yüzden FILE_BEGIN(t) ancak bu akışlardan birinde gelebilir ve küme tükenince kayıt kesin olarak gereksizdir. Zamanlayıcı yoktur; T_A > T_g gibi bir zaman eşitsizliğine de dayanmaz.
- **05f'nin iki Faz A girişini kapsar:** (i) FIN teyitliyse FILE_BEGIN ya okunmuştur (aktarım bağlıdır) ya da akış R0'dır; (ii) oturum `floor ≤ fin_seq` iken bittiyse o oturumun akışları `Closed` ile biter, kayıt ya hiç doğmaz ya da düşer.
- **Tavan [türetim taslağı]:** her canlı kayıt en az bir R0 akışını anar; R0 akışları eş başına ≤ 2·`bk_flow_limit0` (iki oturum; §3). Bir R0 kümesi canlıyken uyumlu eşten gelebilecek FILE_ABORT'lar: kümenin akışlarına karşılık gelen aktarımlar (≤ 8) + o sırada Faz A'sı dolan aktarımlar (≤ eşin `MAX_OUTGOING_TRANSFERS` = 4; bu eşin kalibrasyonudur). Tavan C_EA = 12.
- **Aşım:** yeni kayıt açılmaz, `early_abort_overflow` sayılır (rapor, kapı değil). Sonuç: geç FILE_BEGIN aktarımı başlatır. Faz A'da aktarım tamamlanır ve gönderen alındıyı yok sayar (DUR-05f-11 ayrışması); `superseded` durumunda eski oturum kapanınca 05f §9 satır 3 uygulanır. Yanlış "teslim edildi" yoktur.

**YD-6 Kontrol yükümlülükleri: doğum, yer, ölüm** (H_C_tavan §7.6).

| Yükümlülük | Şerit | Doğar (yer ayrılır) | Ölür | Tavan |
|---|---|---|---|---|
| FILE_RECEIPT | 1 | gelen BK akışı açık kümeye girince (örtük açma dahil) | akış 1'e kabul edildi (oturumlar arası taşınır, 05f §9) ∨ RESET'le düştü ∨ FILE_ABORT uygulandı (gönderen alındı beklemez); her durumda R3 (YD-2) | ≤ 2·`bk_flow_limit0` (drain dahil, §7.6) |
| FILE_ABORT (Faz A, `superseded`, oturumu değişen Faz G iptali) | 1 | FILE_BEGIN kabulü | FILE_ABORT kabul edildi ∨ terminal FILE_RECEIPT geldi ∨ `Delivered` | ≤ `MAX_OUTGOING_TRANSFERS` 4 |
| FLOW_RESET (+ kod) | 0 | giden BK akışı açılınca | FLOW_RESET teyidi ∨ FIN teyidi ∨ `reset_flow_local` | ≤ açık giden BK akışı ≤ eşin `bk_flow_limit0` |
| `HELD` girdisi (yalnız DUR-06-26 (b) seçilirse) | ACK yolu (şerit değil) | R2 | R2 bitti | akış başına 1 girdi; çerçeve ≤ FLOW_CREDIT_MAX_ENTRIES |
| RECEIPT | 1 | TEXT işlendi | 05d (tut-ve-dur; canlı RECEIPT silinmez) | 05d; akış-1 kredisinde FILE_* payının dışında (§7.6) |
| PTT_OPEN / PTT_CLOSE | 4 | konuşma | REL teyidi | `max_rel0` |

Akış 1'de W_c ≥ 12 + RECEIPT payı (§7.6). `Independent` kip H_C_tavan'ı değiştirmez; yalnız teslim sırasını değiştirir.

**YD-7 Saatler** (05f'nin G, R, A saatleri; sınırlar 05f'de).

| Koşul | G (gönderen, S1) | R (alıcı, R1) | A (gönderen, S2) |
|---|---|---|---|
| Kredi bloklu, teyitsiz birim yok | durur | — (bekleten alıcı) | — |
| FIN öncesi tutma / tüketici durması | durur (BLOCKED) | durur | — |
| FIN sonrası tutma / tüketici durması (R2) | — | — (kredi dolu) | (a): çalışır, T_A'da D_h payı · (b) seçilirse: bildirilen tutma düşülür, en çok D_tut,max (YD-3) |
| Kayıp kurtarma, ACK kaybı, hizmet sırası | çalışır | çalışır | — |
| Alıcı işlemesi (T_işlem), FILE_RECEIPT yolculuğu | — | — | çalışır |
| `stop_flow` sonrası (FILE_RECEIPT yolda) | durur (BLOCKED; yükümlülük düşmez, alındı gelir) | — (akış kapalı) | — |
| FILE_RECEIPT, gönderenin KENDİ durgun akış-1 tüketicisi yüzünden kredi bekliyor | — | — | kapasite beklemesidir, A'ya girmez (05f R1; YD-9 m.5) |
| Oturumsuz aralık | 05f §9 | 05f §9 | çalışır (kesinti bütçesi) |

**YD-8 Testler:** B13 (b)(d)(f), B26 (akış-1 seli + durgun tüketici), B29 (FIN sonrası tutma), B30 (kontrol şeridi bağımsızlığı), B31 (iptalin akışa bağlanması, erken iptal kaydı, `FlowEnded` sırası); 05f Ek B.

**YD-9 05f ile hizalama (koşullu; sahip kararları bekleniyor):**
1. **DUR-06-26 = DUR-05f-14:** iki not aynı ölçütü ve aynı öneriyi taşır: (a) D_h = 30 s. (b) seçilirse T_A'dan D_h çıkar; G_H (05f §7 formülü) ve D_tut,max girer.
2. **DUR-06-28 (a) seçilirse:** G dolumu = `reset_flow(f, 0x04)`, FILE_ABORT adımı kalkar. A dolumu aynen (FILE_ABORT akış 1); alıcıda `stop_flow` + `resolve_flow`.
3. **R dolumu:** `stop_flow` (YD-2) + FILE_RECEIPT(`timeout`); kabulünde `resolve_flow`.
4. **Erken iptal kaydı:** YD-5'in R0 kuralı. 05f §9'un son satırı buna göre: "kaydın andığı R0 akışları bağlanınca ya da bitince".
5. **T_ctrl(E):** DUR-06-27 (c) seçilirse tek kontrol biriminin kendi kurtarma sınırı + pencere beklemesidir:
   - (i) tekilleştirme penceresi beklemesi: W_D türetimiyle zarf içinde 0 (§3);
   - (ii) tüketim kredisi beklemesi: yalnız alındıyı bekleyen tarafın KENDİ akış-1 tüketicisi durgunken doğar. Kendi saatinde bir kapasite beklemesidir (05f R1) ve A'ya girmez; 05f bunu A'nın durma koşulu olarak yazar.
   - (a)/(b) `Ordered` seçilirse öndeki kayıtların kurtarılması T_ctrl'ye eklenir.
6. Çarpan ve ACK takvimi 05f'nindir. Bu bölüm yalnız saatlerin durma/çalışma koşullarını verir.

**YD-10 `FlowEnded` ve teslim sırası (rev5.1).**
- **Sorun:** `FlowEnded` bir `SessionEvent`'tir ve kama-app olayları birimlerden ÖNCE okur (`0c22a07d:messaging.rs:646-663`; adaptörde ayrı `events` ve `inbox` kuyrukları, `transport.rs:448-449`). Okunmamış bir FILE_BEGIN varken RESET işlenirse uygulama önce `FlowEnded`'i görür (bağ yok → olay yok), sonra FILE_BEGIN'i: hayalet aktarım. DUR-06-28 (a)'nın "ya teslim almıştır ya hiç almamıştır" ikiliği yalnız transport düzeyinde doğruydu.
- **Kural:**
  1. Runtime `FlowEnded`'i teslim kanalında, akışın son teslim birimlerinin ARDINA bant içi işaret olarak koyar.
  2. Adaptör işareti alınca, `Reset` için akışın uygulamaca okunmamış birimlerini (akış hazır kuyruğu ve inbox) atomik olarak atar ve olayı ancak ondan sonra yayımlar. `Finished` için olayı, akışın hazır kuyruğu uygulamaca boşaltılınca yayımlar.
- **Sonuç:** `Reset`'te uygulama FILE_BEGIN'i ya olaydan önce okumuştur ya da hiç okumaz; `Finished`'ta bütün birimleri olaydan önce okur.
- A.7 (ii) ve A.15'e yazılır; B31 (k).

## Ek A — Onay gerektiren değişikliklerin TAM listesi (liste; yama metni DEĞİL)

**Kural:** hiçbir donma sessizce aşılmaz. Her satır kod yazılmadan önce sahibin yazılı kararını ister: "DUR" sütunundaki DUR'un cevabı (CEVAP 16 ya da sonraki tur) ya da "yama onayı" (tasarım onayından sonra, yama metniyle). Yama metinleri (üstü çiz + tarihli YAMA KAYDI + `word_coverage`) kodla aynı PR'da yazılır. Deney dalı bu dosyalara yalnız "DENEY" etiketiyle dokunur ve merge edilmez. DUR-05g-1'in dar S1 K7 istisnası bu kalemlerin hiçbirinin yerine geçmez. Sütunlar: FROZEN = donmuş sözleşme/spec/kapsam/ürün profili · Tel = iç çerçevede bayt değişimi (hub zarfı her satırda aynı) · Drift = `seam-api-drift.py` / `seam-coverage.json` beyanı.

| # | Dosya / öğe | Değişiklik (tasarım düzeyi) | FROZEN | DUR / onay | Tel / vektör | Drift |
|---|---|---|---|---|---|---|
| A.1 | `crates/kama-contracts/src/s1.rs` — tipler | `FlowClass {Control, Voice, BestEffort, Background}`, `FlowMode {Ordered, Live, Independent}` (`Independent` yalnız kalıcı kontrol akışları; DUR-06-27 önerisi), `FlowSpec`, opak `FlowRef`; `Delivered`'a `flow`, `class`, `flow_seq`; `SessionEvent`'e `FlowOpened`, `FlowEnded {Finished, Reset { code: u8 }}` (kod uygulamanın, transport yorumlamaz; DUR-06-28 önerisi; teslim sırasına göre yayımlanır, §16 YD-10); `TransportCapabilities`'e `max_control_unit_bytes` | EVET (S1) | DUR-06-7, DUR-06-27, DUR-06-28 | yok | §1 trait bloğu; `closed_sets_are_exhaustive` |
| A.2 | `s1.rs` — `KamaTransport` | `open_flow`, `close_flow` (FIN), `reset_flow(flow, code: u8)`, **`reset_flow_local(flow)`** (gönderen; motor `CancelFlow`, telde FLOW_RESET yok; alıcının sonlandırdığı akış için), `hold_flow`, **`stop_flow(flow)`** (alıcı, gelen akış; yerel sıfırlama: tamponlu birimler atılır, taahhüt serbest, akış kapalı, yeni kredi yok; §16 YD-2), **`resolve_flow(flow)`** (alıcı; R3: FILE_RECEIPT kabulü / FILE_ABORT uygulanması / ret alındısı sonrası, yalnız bitmiş, durdurulmuş ya da RESET'lenmiş akışta, değilse `UnknownFlow`; §16 YD-2); `send`'in hedefi `FlowRef`; `recv()` imzası aynı, anlamı "tutulmamış akışlardan" | EVET | DUR-06-7, DUR-06-28 | yok | 12 `impl KamaTransport` (Ek C.G) |
| A.3 | `s1.rs` — `TransportError` | `UnknownFlow` varyantı; kredi/akış sayısı/VO profili dolu → `Backpressure` | EVET (kapalı küme) | DUR-06-7 | yok | §3 hata kümesi |
| A.4 | `s1.rs` — `DeliveryPolicy` anlamı | `DeadlineBounded` süresi dolunca motor boyunca iptal (bugün yalnız sunumda, `0c22a07d:transport.rs:1297-1305`); `BestEffort` tanımı (Live akışta) | EVET (§4 anlamı) | DUR-06-18, DUR-06-7 | yok | §4 metni |
| A.5 | S1 K6 `early_unit` + `connect` (`docs/seams/KM-SEAM-AT-001.md` K6 YAMA KAYDI 2026-10-08, :9; `0c22a07d:transport.rs:1000-1014`) | Erken birimin akışı: `connect`'e erken birimin sınıfı; `Ready`'de o kalıcı akışın ilk birimi; `Delivered.flow`/`flow_seq` tanımı | EVET | DUR-06-7 | yok | §1, H-EARLY-CONFLICT |
| A.6 | `handles.rs`, `s3.rs`, kama-noise-wire `envelope.rs`/`protector.rs`, hub-server codec | **Değişmez:** dış zarf 22/26 B, PN/nonce, `ProtectionContext`. (rev1'deki geniş "noise-wire değişmez" beyanı daraltıldı: iç düzen ve kimlik kapsamı A.10–A.11'de) | EVET | beyan | yok | beyan |
| A.7 | `docs/seams/KM-SEAM-AT-001.md` + `scripts/ci/seam-coverage.json` + `scripts/ci/seam-mutation.py` | §1 trait bloğu (A.1–A.5); §3; §4 "aynı session içinde Reliable birimler gönderim sırasında" (`0c22a07d:…KM-SEAM-AT-001.md:153`) → "aynı akış içinde"; Live sırasız + `flow_seq`; INV-ACK-DELIVER-1 (:154) akış başına + **üç istisna:** (i) kapatılmış/RESET edilmiş akışın geç birimi ACK'lenip atılır (`late_after_close`); (ii) RESET edilen akışın ACK'lenmiş ama **uygulamaca okunmamış** birimleri (teslim kanalı, hazır kuyruk, inbox) atılır (oturum `Closed` beyanlı sınırının akış karşılığı; §16 YD-10); (iii) alıcının `stop_flow` ettiği akışın tamponlu ve sonraki birimleri atılır, sonrakiler ACK'lenir; **`Independent` kip: sırasız, oturum içinde tam bir kez (`flow_seq`; tekilleştirme penceresi W_D, tüketim kredisi ayrı); oturumlar arası tekrarı uygulama tekilleştirir**; **`FlowEnded` sırası:** `Reset` uygulamaca okunmamış birimleri attıktan sonra, `Finished` akışın birimleri okunduktan sonra yayımlanır; "kapasite durması ACK kesilerek gösterilmez"; §6 H-FLOW-ISOLATION, H-FLOW-CREDIT, H-LIVE-EXPIRY, H-CTRL-SERVICE, **H-CTRL-INDEPENDENT** ("kontrol kaydı aynı şeritteki başka bir kaydın kurtarılmasını W_D − 1 kayda dek beklemez; ötesinde `floor_a` ilerleyene dek kredi bekler"; B30), **H-FLOW-END-ORDER** (B31 (k)), **H-FLOW-RESOLVE** (`bk_flow_limit` yalnız R3'te; B13 (f)), **H-STOP-FLOW** (B31 (f)) + her biri için sahte mutasyon operatörü | EVET (G-S1) | DUR-06-7, DUR-06-27, DUR-06-28 + yama onayı | yok | 8 yeni H-* satırı |
| A.8 | `crates/client-core/src/wire.rs` (v0 kabuk) | `FLOW_BATCH` tip baytı + BatchPayload codec'i (`batch.rs`) veri yoluna bağlanır; **örtük kip de FLOW_BATCH'e geçer (tek kalıcı akış 2), 0x00 emekli**; 0x05 yalnız açık opt-in parçalayıcıda beyanlı istisna; store-miss → `[tip][boş BatchPayload]` (2 B; BatchPayload kısmı canonical 1 B); onarım çerçevesinde ölü `seq u16` → `flow_id`, + `first_seq u32` (+4 B); RECOVERED düzeni aynı; yeni FLOW_CREDIT (akış + sayım girdileri; `max_seq` hariç üst sınır, §7.1; DUR-06-26 (b) seçilirse `HELD(flow_id, held_ms u32)` "tutuluyor" girdisi, 11 B), FLOW_BLOCKED (iki boyutlu, çok girdili); **akış-0 FLOW_RESET gövdesi `flow_id · final_seq · kod u8`** (DUR-06-28) | EVET (`0c22a07d:runtime.rs:230` "tel, codec ve `tv_*` değişmez") | DUR-06-5, DUR-06-26, DUR-06-28 | **EVET** (zarf içi) | `api_surface_v0` (A.28) |
| A.9 | `specs/ccr-format-spec.md` §35.6 / §35.14 `BatchPayload` (`1b1425d2:…:3017-3043`) + `docs/seams/KM-SEAM-TC-001.md` KARAR-1 (`1b1425d2:…:82`, :173) + `batch.rs:28-29` + `specs/transport-layer-tdd.md` geçiş beyanı (`0c22a07d:…:972-974`) | **KARAR-1'e ve ccr §35.14'e tarihli sahip eki GEREKİR** (rev2'deki "çelişmeyen tek seçenek" geri çekildi). Ekin içeriği: (1) `BatchSubMessage`'a sabit 7 B akış başlığı (`flags · flow_id · flow_seq`) — plan KATMAN 6 "alt mesaj başlığında teslim sınıfı" (`1b1425d2:docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:287`); (2) v0 kabuk tip baytı (ccr'de yok; v0 çoğullayıcı) ve S2 eşlemesi: extended yolda yeni `inner_kind`, legacy yolda KARAR-1'in açık "ayrım" kararı; (3) S1 kipinde `submsg_count ∈ {0, 1}`, ≥ 2 → fail-closed ret + sayaç (ccr çoklu alt mesajı kısıtlanır); (4) örtük kip de FLOW_BATCH → ürün runtime'ında tek veri çerçevesi; 0x05 parçalayıcı istisnası açıkça | EVET (ccr bayt otoritesi; KARAR-1 sahip kararı) | DUR-06-5 | **EVET** (canonical çerçeve) | KM-SEAM-TC-001 statü satırı |
| A.10 | `specs/kama-noise-wire-tdd-v2.0.md` §2.6 önek tablosu (`1b1425d2:…:424`) ve ID-SCOPE (:430-433), §2.7 allowlist (:437), §3 DÖRT-KİMLİK (:452-458), §2.56 codepoint kaydı (:325-340), §2.57 gövdeler (:347) | `source_flags` bit anlamları: FIN, REL (bugün yalnız FINAL); **`block_id` kapsamı DEĞİŞMEZ** (oturum + yön monoton, NO-REUSE; B-iii); S1 parçalamasız kipte `message_id := flow_seq` (NO-REUSE kapsamı akış + yöne daralır) ve `chunk_offset := flow_id`; §3'ün (4) "message/chunk-id" kimliğinin S1 anlamı = (`flow_id`, `flow_seq`) — FEC dışı akışta dedup ve sıra kimliği budur; yeni codepoint'ler 0x1D'den K-WIRE-CTRL-1 ile ve §2.7 allowlist'ine; S2 gövdelerine `control_id` | EVET (KİLİTLİ) | DUR-06-5 | EVET (yeni codepoint; FEC düzeni aynı) | — |
| A.11 | `crates/kama-transport-schema/src/{prefix.rs, inner_kind.rs, control.rs, lib.rs}` + `crates/kama-noise-wire/tests/golden_wire.rs` | `source_flags`'a FIN ve REL bitleri (bugün ayrılmış → ret, `0c22a07d:…/prefix.rs:15-16`, :36); `chunk_offset`'un S1 anlamı; yeni `inner_kind`/`Control` girdileri; DTO düzenleri değişmez; golden_wire **değişmez hedefi** | EVET | DUR-06-5 | EVET (önek bayrakları; tek kaynak schema) | — |
| A.12 | `crates/client-core/src/runtime.rs` — gönderen | Store girdisine (akış, sıra) + ters dizin; S1 kipinde mesafe kapısı (`:2511`, :2575-2581) → akış kredi kapısı (`flow_seq < max_seq`, hariç üst sınır); akış başına yerel tampon (offer FIFO dışında); akış başına FEC biriktirici + blok tablosu (`block_id` → akış, `first_seq`) + T_block kısmi kapanış (`:2763-2811`'in genelleşmesi); **`repair_store` sınıf başına, tahliye sınıf içinde en eski** (`:2015-2018`, :2715-2716); `AppDataTagged`; BLOCKED üretimi (§7.5); VO gönderen kapısı (W_LIVE); **akış 0/1 `Independent` gönderen tarafı (kalıcı akış `flow_seq`'i, `min(floor_a + W_D, c + W_c)` kredisi)**; **`reset_flow_local`**; motor öncesi birim için runtime'ın kendi iptal çıktısı; **DUR-06-26 (b) seçilirse `HELD` alımı** (FIN teyidinden FILE_RECEIPT / A dolumu / `Closed`'a dek ilerleme kaydı; bu sürede HELD bayat değil; §16 YD-3) | EVET (ürün davranışı) | DUR-06-5, DUR-06-2, DUR-06-26, DUR-06-27, DUR-06-28 + EVOLUTION §22 | EVET (A.8) | — |
| A.13 | `runtime.rs` — alıcı (`DeliveryGate`, `AcceptState`, `DeliveryLedger`, `PeerSlot`) | (oturum, akış) kabul; örtük açma + `max_seen`/açık küme; **tek kredi kuralı `max_seq = min(floor_a + W_D, c + W)` (hariç üst sınır; §3, §7.4)**; `RefuseWindow` = `credit_violations`; `RefuseFull` → `capacity_defect`; S1 kipinde sıra başı rezervi (`:3284-3297`) kredi taahhüdüyle; **boş batch PN'i kabul kapısına girmeden ACK'lenir**; akış 0'ı transport tüketir; kaynak sembol halkası (akış başına) + **`slot.blocks` sınıf başına** (`:3192-3193`, :3894-3895); VO dedup penceresi; **akış 0/1 `Independent`: tekilleştirme penceresi (`floor_a` + W_D bit, `dup_ctrl`) tüketim kredisinden (`c` + W_c) ayrı**; **`stop_flow` (yerel sıfırlama)**; **R3: `resolve_flow` ya da RESET → `bk_flow_limit` +1**; **`FlowEnded` bant içi işareti teslim kanalında akışın son teslim birimlerinin ardına**; **DUR-06-26 (b) seçilirse `HELD(flow_id, held_ms)` üretimi** (R2 ≥ T_min, T_BLK aralık, `held_ms` doyar); FLOW_CREDIT ACK yolunda tetiklerle, eş başına ≤ 1 çerçeve / T_min; `stale_ctrl_entries`; `flag_mismatch` | EVET (05c A.6'nın devamı) | DUR-06-3, DUR-06-5, DUR-06-26, DUR-06-27, DUR-06-28 | EVET | KM-SEAM-AT-001 §4 (A.7) |
| A.14 | `runtime.rs` — `RuntimeConfig` / `KamaV1Profile` / kimlik (`:236`, `:241`) | Havuzlar, `W0`, `W_TCTRL0`, `W_CTRL0`, `bk_flow_limit0`, `max_rel0`, ρ_C, ρ_V,max, Q_BE:Q_BK, T_block, D_VO, W_LIVE, T_min, FLOW_CREDIT_MAX_ENTRIES [CAL]; iki doğrulama değişmezi ve bellek sonucu (§7.2: 64 eşte ≈ 20 MiB → normatif sayı DUR-06-3); yeni kimlik | EVET (`transport_v1_freeze.rs`, `product_profile_v1.rs`) | DUR-06-13, DUR-06-3 | yok | `api_surface_v0` |
| A.15 | `crates/client-core/src/transport.rs` | (oturum, akış) `Reorder`; akış başına hazır kuyruk + `hold_flow`; `open/close/reset_flow`, **`reset_flow_local`, `stop_flow`, `resolve_flow`**; `FlowOpened/Ended`; **`FlowEnded` sırası: `Reset` işaretinde akışın uygulamaca okunmamış birimleri (hazır kuyruk + `inbox`, bugün `0c22a07d:transport.rs:448-449`) atomik olarak atılır, olay ondan sonra; `Finished` olayı hazır kuyruk boşalınca** (§16 YD-10); 05g etkinleştirme kollarıyla uyum; erken birim (A.5) | Hayır (adaptör; davranışı S1'e bağlı) | tasarım onayı | yok | H-* real hedef (H-FLOW-END-ORDER) |
| A.16 | `specs/transport-layer-tdd.md` §12 AMENDMENT-12 ve YAMA KAYDI (`0c22a07d:…:5`, :855-899) + §0 INV-NO-IDLE-TEARDOWN (:57-63) + yeni AMENDMENT-13 | Değişen donmuş cümleler: (1) INV-NO-ORPHAN (:855) — "uygulamanın RESET ettiği akışın ve süresi dolan `DeadlineBounded` birimin iptali hariç"; (2) "Payload yalnız teyit çıktısıyla silinir … Başka hiçbir yol silmez" (:862-864) → "yalnız tek sonlandırma çıktısıyla (motorun Teyit/İptal'i ya da motora verilmemiş birim için runtime'ın İptal'i; birim başına oturumda en çok biri)"; (3) "Okuma (NORMATİF): kapsam motorun taşıdığı her birimdir" (:867) — aynı istisna; (4) "ARQ_RETX_CAP aşımı (her politika) … ASLA düşürmez" (:869-870) — iptal edilmiş birim devredilmez; (5) "İzleme teyide ya da §0 kapalı listesindeki bir sonlandırmaya dek sürer" (:879) → "İzleme teyide, iptale (yalnız RESET'li akış / süresi dolan `DeadlineBounded`) ya da §0 kapalı listesindeki bir OTURUM sonlandırmasına dek sürer" — **§0 listesine dokunulmaz** (rev2'nin "listeye süre dolumu" önerisi geri çekildi: liste oturum sonlandırma nedenleridir ve "zaman"ı dışlar); (6) "Teyit çıktısı (NORMATİF)" (:892-899) → "Sonlandırma çıktısı"; aynı olayda teyit kazanır; iptal teyit ÜRETMEZ; (7) YAMA KAYDI (:5) "birimi düşürmez" — aynı istisna; (8) INV-NO-IDLE-TEARDOWN (:61) "NİYET … zamanla ölmez" → sahip metniyle açıklama: "sunumda beyan edilen `DeadlineBounded` süresi uygulamanın birim iptalidir; oturum ve niyet için zaman sonlandırma nedeni değildir" (:57-63 bu açıklamayla "değişir"); AMENDMENT-13: akış/kredi/seçici | EVET (KAT GO-FREEZE; sahip metni) | yama onayı (DUR-06-2'ye bağlı) | yok | — |
| A.17 | `crates/adaptive-transport/src/sender.rs` (S7 b; taban 05b rev6 `37b78aad`) | Eklemeli `Event::AppDataTagged(OpaqueUnit, ServiceTag)` (etiketsiz = tek akış); katman/akış başına DATA kuyruğu; tek saf seçici: rev6 `refresh_reserve` (:7334), `assign_reserve_to_holder` (:6311), `on_class_emitted` (:6333), `ServiceRights` (:1423), `serve_copy_share` (:7292), `copy_queue_head` (:7243) seçicinin düzey-2'si olur, haklar (akış, sınıf) başına; `TransmitKind::Control`; iptal durumu + `Event::CancelFlow` + `take_finished_units` | **EVET (FROZEN v1.1 motor)** | DUR-06-2 | yok; `tv_*` değişmez HEDEFİ (kayarsa DUR) | — |
| A.18 | `specs/application-message-protocol.md` §3 | PTT-R0 → OPEN/CLOSE `Reliable`, demet `DeadlineBounded`; PTT-R3 → sınırlı ön tampon; **PTT-R2 → OPEN'ı görülmemiş akışın CLOSE'u ön tamponda tutulur, OPEN gelince "başladı + bitti" (DUR-06-18 (a)'ya koşullu; §7.6)**; "FILE_BEGIN her zaman önce gelir" gerekçesi aynı akışa (`0c22a07d:file_transfer.rs:74-75`); RECEIPT/FILE_RECEIPT/FILE_ABORT akış 1 (DUR-06-27 (c) seçilirse `Independent`; rev4'teki "akış 0" yazımı yanlıştı); **kontrol kayıtlarının sırasız teslim altında idempotentlik kuralları normatif yazılır** (RECEIPT `(eş, ref_msg_id)`, FILE_RECEIPT aktarım başına bir, FILE_ABORT ilk neden kalır; bitmiş aktarıma gelen FILE_ABORT kayıt açmaz; §4.1, §16 YD-5); **DUR-06-28 (a) seçilirse FILE_ABORT'un kullanımı daralır:** Faz G iptali akış RESET'inin koduyla (kod kümesi = FILE_ABORT.reason; bilinmeyen / 0x02 / 0x03 → `Unspecified`); FILE_ABORT Faz A'da, oturum değişiminde ve RESET'i teyitsiz oturum değişen Faz G iptalinde kalır (AMP :92 "bekleyen kullanıcı iptali yeni oturumdan gider" cümlesinin karşılığı); kullanıcı iptali FILE_END kabulünden önce kalır; erken iptal kaydı R0 tabanlı, zamanlayıcısız (§16 YD-4, YD-5); kayıt baytları DEĞİŞMEZ | EVET (AMP donmuş metin) | DUR-06-18, DUR-06-27, DUR-06-28 | AMP baytları ve `amp-record*-v1.json` değişmez | — |
| A.19 | `docs/decisions/PoC-KAPSAM-001.md` §2 PTT satırı ve "bekliyor" göstergesi (`1b1425d2:…:26`), §2 "Sözleşmeler" S5 dondurması (:34), EK-2 md. 1 (:98), EK-3 (:27, :103-105), **K6 "facade değişmez" (:54)**, **K8 PTT satırı (:62)** + `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` D7 ek (:94, :287) + K4G-08 PTT-R0 + `ptt.rs:18-24` modül belgesi | DUR-06-18 (a) seçilirse hepsi sahibin tarihli **EK-4**'üyle değişir: canlı PTT; K8 "sıfır bozuk hece" EK-4'te yeniden tanımlanır (eksik demet sessizlik/PLC; bozuk çerçeve oynatılmaz); S5 ve K6 PttBundle/pttPush değişikliğine izin verir. (b) seçilirse K8, S5 ve K6 aynen kalır, yalnız PTT'nin kendi VO akışına alınması eklenir | **EVET (KESİN; yalnız sahibin tarihli eki, :7-8)** | **DUR-06-18** | yok | — |
| A.20 | `specs/adaptive-transport-engine-spec.md` (§3 ana döngü, §9, §11, §13 — taban 05b rev6 metni `37b78aad`, "istisna ve karşılık kuralıyla" —, §16.2) + `docs/transport-v1/{INVARIANTS, PRODUCT-PROFILE, ARCHITECTURE, EVOLUTION §22, TRANSPORT-CRYPTO-HANDOFF (:18, :60 "mesaj = birim", "DATA 0x00"), KNOWN-LIMITATIONS}.md` | İki düzeyli seçici, ServiceTag, katman 0, iptal; kimlik ve altı beyan | EVET | yama onayı (bölüm listesiyle; DUR-06-2, -13) | yok | — |
| A.21 | kama-app: `messaging.rs`, `file_transfer.rs`, `ptt.rs`, `signaling.rs` | Akış ataması; **pompa akış başına** (bugün eş başına en eski aktarım, `0c22a07d:file_transfer.rs:862-881`); dosya tüketicisi `hold_flow`; RECEIPT/FILE_RECEIPT/FILE_ABORT akış 1; **alındı kuyruğu (`MAX_PENDING_RECEIPTS` küresel 4, taşmada en eskiyi düşürür, `file_transfer.rs:286-291`, :1216-1224) → gelen BK akışı başına yuva, düşürme yok; reddedilen FILE_BEGIN'in FILE_RECEIPT(rejected)'i de yuvada (:1018-1034)**; **akış-1 FILE_* payı: kullanılabilir kredi ≤ 12 iken RECEIPT sunulmaz (§7.6)**; PTT ön tampon + `flow_seq` + REL + **OPEN-önce-CLOSE kuralı** (§7.6); **tek kabul yolu: `transport_mut()` (`messaging.rs:425`) kaldırılır ya da daraltılır**; **05d M2 kapısı kaldırılır** (§10); **gelen aktarım ↔ (oturum, akış) bağı; `resolve_flow` çağrı noktaları (§16 YD-2); DUR-06-28 (a) seçilirse Faz G iptali `reset_flow(k, kod)`, `FlowEnded { Reset { kod } }` → `Aborted(kod)`, terminal FILE_RECEIPT'te alıcıda `stop_flow`, gönderende `reset_flow_local`; erken iptal kaydı R0 tabanlı (doğum, R0 kümesi, düşme, C_EA, `aborts_after_terminal`; `file_transfer.rs:139-156`, :295, :1120-1130, :1295-1322 → §16 YD-5)** | Hayır (ürün kodu); 05d kararlarını değiştirir | DUR-06-14, DUR-06-27, DUR-06-28 | yok | — |
| A.22 | 05d DUR-05d-7 A1 / DUR-05d-8 kararları (C14 §4, C15 v3 §5) | A1 ayrıca kodlanmaz → akış bütçesi erişimcisi; W_F → BK penceresi; H_C → H_C_tavan (§7.6); §5(ii) "tek kabul yolu, ürün derlemesinde zorlanır" şartı devralınır | karar metni | DUR-06-14 | yok | — |
| A.23 | 05f (rev2 `3e0ec8fa`, rev3.1 `40c59dfb`: `docs/design/K4G-05f-DOSYA-ZAMAN-ASIMI.md`; rev1 `e5514e7a` reddedildi) | İlerleme = akışta daha önce teyit edilmemiş birimin ilk teyidi; taban yalnız faz sınırı; G/R/A saatleri; yerel vazgeçme zarfı E (§8); `FILE_IDLE_TTL` ve `FILE_RECEIPT_TTL` kalkar; AMP §3 metinleri (05f Ek A). **C16 v2: D_tük = 0 reddedildi. Öneri (DUR-06-26 = DUR-05f-14 (a)): T_A = T_işlem + D_h + T_ctrl(E), D_h = 30 s [CAL]; (b) seçilirse T_A = T_işlem + T_ctrl(E) + G_H(E), D_tut,max. DUR-06-28 (a) seçilirse G dolumu `reset_flow(f, timeout)`; R dolumu `stop_flow`; saatlerin durma/çalışma koşulları §16 YD-7.** Ortak sözleşme kalemleri A.36 ve A.37'de | EVET (AMP §3 donmuş metin) | **DUR-05f-6…-18** (05f §13) + DUR-06-26, DUR-06-27, DUR-06-28 (öneri; sahip kararı bekliyor) | tel baytı ve vektörler değişmez | — |
| A.24 | UniFFI/S5: `crates/kama-mobile/src/{error.rs (:218-250, :374-440), event.rs (:107-115)}`, `docs/seams/KM-SEAM-AK-001.md` (:9 S5 kaynağı, :57 `pttPush`, :75 `PttBundle`), ui-api v0.3 kaynağı (`kama-engine` `docs/ui-api-v0.md`, tag `ui-api-v0.3` = `5de9be16`), `crates/kama-mobile/README-HASAN.md` (:15), `crates/kama-node/src/event.rs` (:108-116), `node.rs` SessionEvent eşlemesi (`0c22a07d:node.rs:560-584`) | `UnknownFlow` aynası; `PttBundle`'a `flow_seq` ve anlamı; `pttPush` için `DeadlineExpired` ve canlı kipte "reddedileni yeniden ver" kuralının kalkması; `FlowOpened/FlowEnded` eşlemesi; yeni sayaçlar `NodeMetrics`'te. **Kotlin facade'a dokunur** (oynatıcı sıra bilgisi; "bekliyor" davranışı) | EVET (S5 = ui-api v0.3; PoC-KAPSAM-001 :34 ve K6 :54 ile dondurulmuş) | **DUR-06-23 (Hasan ile ortak onay) + DUR-06-18 (EK-4)** | yok | — |
| A.25 | INT-02/INT-03 kapı metni (C15 v3 §7 Aşama 7; C9 §5; `1b1425d2:scripts/netem/int02_analyze.py:18`, :969-1011) | "kapasite reddi kaynaklı boşa bayt ≈ 0" → "`capacity_defect = 0` (akış 0/1 dahil, yalnız geçerli krediye uyan güvenilir veri)"; ağ kaybı / protokol ihlali / medya expiry / kontrol baytları ayrı sütun; PTT `in_order_complete` hükmü → B11 ölçüleri | kapı metni | **DUR-06-22** | — | — |
| A.26 | `docs/transport-v1/adr/ADR-transport-v1-freeze.md` (`0c22a07d:…:18-24`) + `ADR-hostile-validation-contract.md` (:8 "ürün profilini ölçmemiş" tuzağı) + **`ADR-fec-v1-role.md`** (Decision 1 "KamaV1Profile FEC ayarı aynen sevk edilir"; akış başına blok ve T_block bunu değiştirir) + ADR-18 netem 7 profil + §4.4 + `crates/bench/src/{kama_product.rs, manifest.rs}` | Etkilenen T-hücreleri yeni kimlikle yeniden koşulur; **bench'e S1-kipi ürün kolu** (`kama-product-s1`, manifestte `kip` alanı; çoklu akış, kredi, BLOCKED ölçülür) — B20'nin ön koşulu; yapılmazsa "T-hücreleri kredi/çoklu akışı ölçmez" beyanı; netem 7 profil ön-kayıtlı tahminle (B20); kayarsa DUR | EVET (ADR) | DUR-06-13, DUR-06-25, DUR-06-11 + yama onayı | — | — |
| A.27 | `vectors/` + yeni Python referansı (`ref/transport/` önerisi) + `crates/kama-vectors` + `fuzz/fuzz_targets/wire_v0.rs` | FLOW_BATCH, FLOW_CREDIT, FLOW_BLOCKED, **FLOW_RESET gövdesi (`flow_id · final_seq · kod u8`)** ve **(DUR-06-26 (b) seçilirse) `HELD(flow_id, held_ms)` girdisi** vektörleri YALNIZ Python referansından; paket **canonical** (A.9 eki onaylanırsa) — interim değil; `SCHEMA.json` değişmez; fuzz hedefine yeni çözücüler | vektör kuralı | DUR-06-5, DUR-06-26, DUR-06-28 | EVET (yeni paket) | — |
| A.28 | `crates/client-core/tests/api_surface_v0.rs` + `docs/design/P4-TRANSPORT-API-FREEZE.md` (:126) | wire-v0 imzaları, `Delivery`, `RuntimeConfig` deseni, `Event` re-export'u (yeni varyant) | EVET (P4) | DUR-06-21 (yöntem) + yama onayı | — | — |
| A.29 | `crates/adaptive-transport/src/arq.rs` | **Değişmez hedefi:** akış içi plan sırası (`0c22a07d:arq.rs:124-127`), `tv_arq` | EVET | beyan | yok | — |
| A.30 | `crates/kama-engine` (`src/recovered_map.rs` (block, esi)→birim eşlemesi; testlerde `OpaqueUnit` literal'leri) | **Değişmez hedefi** (rev1'deki "taranmadı" kaldırıldı): `block_id` tekilliği korunduğu için eşleme aynen çalışır [çıkarım]; eklemeli API ile test kaynakları aynı kalır | — | beyan (kayarsa DUR) | yok | — |
| A.31 | `docs/seams/KM-SEAM-TH-001-TASLAK.md` A3 (`1b1425d2:…:63`) | Gereksinim notu: hub sınıf göremez → handle başına kuyruk sığ ve sınırlı olmalı (INT-03 izolasyonu) | S4 taslak (NON-GATING) | yama onayı (S4 sahibine iletilir) | — | — |
| A.32 | `crates/kama-node/{event.rs, probe.rs, node.rs}` + `crates/kama-integration/examples/int02_node.rs` + `scripts/netem/int02_analyze.py` | Yeni sayaçlar: `capacity_defect`, `credit_violations`, `media_expired_tx`, `media_dropped_rx`, `media_late_rx`, `late_after_close`, `stale_decisions`, `stale_ctrl_entries`, `flag_mismatch`, `batch_multi_rejected`, `vo_rate_deficit`, `held_above_floor_peak`, **`dup_ctrl`, `aborts_after_terminal`, `early_abort_overflow`, `held_entries` / `held_bytes` ((b) seçilirse)**, akış başına ARQ emisyonu, kredi yeniden ilanı, kredi/BLOCKED baytları; yeni kollar (Ek B) | Hayır | tasarım onayı | yok | — |
| A.33 | `scripts/ci/layer_check.py` | `adaptive-transport` kama-app'e ve kama-contracts'a bağımlı olmaz (`ServiceTag` motorun tipi) — kural kapsamı doğrulanır | Hayır | tasarım onayı | — | — |
| A.34 | `specs/ccr-format-spec.md` §35.2/§35.3 el sıkışma (sürüm/yetenek) | Karma sürüm: sürüm/yetenek kimliği doğrulanmış el sıkışmada, `Ready` öncesi fail-closed (seçenek DUR-06-13'te) | EVET | DUR-06-13 | EVET (seçilirse) | S1 olayı (DUR-06-13) |
| A.35 | `crates/kama-store`, `crates/kama-fec-codec` | **Değişmez:** kalıcı sayaçlar ve RS kodlayıcı akış bilmez; blok boyu/ESI geometrisi aynı | — | beyan | yok | — |
| A.36 | **Ortak (05f + K4G-06):** akış ilerleme ve bütçe erişimcisi — `FlowTxProgress { accepted, floor, confirmed, blocked, available, peer_held }` ve `FlowRxProgress { received, delivered, granted, held, fin_pending, bound }`, (oturum, akış) başına; client-core somut tip + kama-app trait, S1 dışı. `available` = kullanılabilir kredi (akış-1 FILE_* payı için, §7.6); `bound` = akışın ilk birimi uygulamaca okundu mu (R0 sayımı, §16 YD-5); `fin_pending` = R2; `peer_held` = alınan en büyük `held_ms` (yalnız DUR-06-26 (b) seçilirse) | 05d A1 erişimcisinin yerine geçer (A.22); `floor` motora henüz verilmemiş birimleri de sayar (05f B.1 tuzağı); `confirmed` yalnız Teyit çıktısından, İptal sayılmaz | Hayır (S1 dışı) ama 05d A1 kararını değiştirir | **DUR-05f-6 (a) + DUR-06-14 (a)** (b seçilirse S1 v2: DUR-06-7) | yok | — |
| A.37 | **Ortak (05f + K4G-06):** terminal yüzey — `SendFailure::LocallyAbandoned`, `FileFailure::LocallyAbandoned`, `FileFailure::SessionEnded` (kama-app); **`Aborted { reason: Unspecified }`** (bilinmeyen RESET kodu; DUR-06-28 (a) seçilirse); kama-node `FileFailReason` (`1b1425d2:crates/kama-node/src/event.rs:168-186`); UniFFI aynası | Yeni varyantlar; mevcut `ReceiptTimeout`/`IdleTimeout` anlamı daralır | EVET (UniFFI/S5; PoC-KAPSAM-001 :34, K6 :54) | **DUR-05f-9 + DUR-06-23 + DUR-06-18 (EK-4)** + DUR-06-28 | yok | — |

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

**Sayaç sözlüğü** (B7 ayırt eder): `net_loss_recovery_bytes` · `dup_ctrl` (`Independent` şeritte yinelenen birim; ihlal değil) · `aborts_after_terminal` (R0 akışı yokken gelen FILE_ABORT; kayıt açılmaz) · `early_abort_overflow` (C_EA aşımı; rapor) · `held_entries` / `held_bytes` ("tutuluyor" girdileri; yalnız DUR-06-26 (b) seçilirse) · `credit_violations` (kredi ya da W_CTRL aşımı; ACK'siz ret) · `media_expired_tx` · `media_dropped_rx` (yalnız `DeadlineBounded`) · `media_late_rx` / `media_late_unplayed` · `capacity_defect` (kredili güvenilir birim — akış 0/1 dahil — için yer yok; **kabul kapısı yalnız bu**) · `late_after_close` · `stale_decisions` · `vo_rate_deficit` · `ctrl_credit_bytes`, `ctrl_blocked_bytes`.

| # | Amaç | Düzenek | Ön koşullar (assert) | Ölçülen | Geçer / taban | Aşama |
|---|---|---|---|---|---|---|
| **B1** | §9.5 ilk kabul: dosya tüketicisi durmuş + dosyada kayıp birim → TEXT teslim, medya süre politikası, bellek sınırlı; açılınca dosya tamamlanır | R (D, U) · N+ (U) · I (yalnız gözlem) | `hold_flow(dosya)`; birim k ≥ 1 kez düştü ve k'den büyük ≥ 1 birim kabul edildi; dosya akışı kredi sınırında (`flow_blocked` ≥ 1); TEXT (U'da VO) bunlardan SONRA sunuldu; `distance_holds = 0` | TEXT teslim süresi; dosya tamponu; alıcı toplam tampon; sebep sayaçları | TEXT tutulu dosya varken teslim; dosya tamponu ≤ verilmiş kredi (D) / ≤ havuz (U); `capacity_defect = 0`, `credit_violations = 0`; `media_dropped_rx` hükmü kırmaz ama raporlanır; açılınca dosya sırayla tamamlanır, özet tutar | D (TEXT + dosya, FEC kapalı) · U · I |
| B1-taban | B1'in eski API'yle yazılabilen varyantı (F8) | R · I | bütün tüketici durur (`ek-tuketici-durak` tarifi) + dosya birimi kaybolur | TEXT teslimi | **taban beklentisi (ön-kayıtlı):** TEXT, k kurtarılana dek teslim edilmez (oturum `Reorder`, `0c22a07d:transport.rs:396-419`) → KIRMIZI | D (K1) |
| **B1b** | 05c mesafe kapısının yeni akışları eski dosyanın eksik birimine bağlamadığı | R | `max_inflight_units = 8`; dosya birimi k'nin bütün iletimleri bir süre düşer; k'den sonra > 8 TEXT | `distance_holds`, TEXT teslimi | **taban:** `distance_holds ≥ 1`, TEXT durur (`0c22a07d:runtime.rs:2575-2581`, :2608) → KIRMIZI · **deney/ürün:** TEXT teslim, `distance_holds = 0` | D · U |
| **B2** | EK-1 §1.2 zorunlu karşı örnek | M | offer kanalı ve `tx_queue`'da dosya DATA'sı yok; N_u ≥ max(8, ⌈T_BE·r/S_u⌉ + 1) eski dosya birimi teyitsiz ve her kolda vadeli kurtarma işi: (a) ARQ (RECOVER_ARQ, `pending_lost`) · (b) kopya (`dup_queue`) · (c) FEC (bekleyen parite); sonra TEXT t sunulur, t'nin ilk iletimi düşer; (a)'da later-ACK yolu kısa süre açık tutulup t'nin kaybı doğal ilan edilir, sonra kesilir; (b)'de t'nin kendi kopya zinciri oluşur | **asıl:** t'nin KENDİ kurtarma emisyonunun ilk hizmet gecikmesi ve ondan önce çıkan u-kurtarma emisyon sayısı/baytı · **ayrı satır:** t'nin ilk emisyonu (S7 hükmüne bağlanmaz) | **taban beklentileri (ön-kayıtlı; T_BE ve T_BE^rec K1'den önce [CAL] değerlerle commit'lenir):** (a) `0c22a07d` ve `37b78aad`: t'nin retransmit'i önce ilan edilmiş N_u kaydın hepsinden sonra (`arq.rs:124-127`; vade = ilan anı `0c22a07d:sender.rs:3846-3849`, `37b78aad:sender.rs:3911`) → ≥ N_u·S_u/r [hesap: 8·1050/300 = 28 s] → KIRMIZI · (b) `0c22a07d`: en küçük kimlik HOL'u (`:7103-7110`) → KIRMIZI; `37b78aad`: yaş tavanı (DUR-40, `:7243-7265`) yalnız vadesini RTO_MAX aşmış kayıtta; pencere < RTO_MAX'ta HOL → KIRMIZI · (c) parite FIFO + oturum geneli blok → ölçülür (KIRMIZI beklenir) · G tabanının rev6 beklentisi `37b78aad` satırlarıyla aynıdır (yaşa göre kopya seçimi `593029eb`'de korunur; satırlar kayabilir) · **deney (katı öncelik CTRL > BE > BK):** H'de T_BE^rec(H) = (S_iletimde + S_t)/r + K_ARQ_SHARE·τ, G'de T_BE^rec(G) = (S_iletimde + S_rez + S_t)/r + K_ARQ_SHARE·τ içinde [türetim, §14; G hüküm verir] · **ürün (U):** düzey-1 DRR türetimiyle (§5 m.3–m.5). Hüküm yalnız S7(a)/(b) ayrımıdır | D (taban: üç kol H ve G; yeni kod (a), (b); hüküm G'de) · U (üç kol, rev6 + seçici) |
| B3 | Kredi güncellemesi kaybı, canlılık | R (U; D'de (b)(e) isteğe bağlı) · I (gözlem) | (a) N_k FLOW_CREDIT düşürüldü; (b) BLOCKED yanıtlandıktan SONRA artan kredi çerçevesi düşürüldü; (c) `hold_flow` > `idle_hold`; **(d) bayttan bloklu** (`max_bytes` bağlayıcı, `max_seq` değil); **(e) artış ilanı kayboldu, tüketilecek veri kalmadı, sonra BLOCKED** (rev2 senaryo A); **(f) `bk_flow_limit` / `max_rel` ilanı kaybı**; **(g) 31 B/s'de ≥ 3 bloklu akış** | BLOCKED sayısı/aralığı; blokta kalma; `ctrl_*_bytes`; kontrol kovası doluluğu | akış/sayım, BLOCKED aralık tavanı + RTT + T_min türetilmiş sınırında ilerler; (e)'de §7.4 (iii) yanıtı; veri kaybı yok; `capacity_defect = 0`; (c)'de yuva emekliye ayrılmaz; (g)'de tek çok girdili BLOCKED kontrol kovasını aşmaz, akış 0/1 işi aç kalmaz | D-sıra 2 · U · I |
| B4 | Sırasız / bayat / tekrar / eski oturum kredisi | R | röle daha küçük sınırlı GERÇEK çerçeveyi büyüğünden sonra oynatır; tekrar; yenileme sonrası eski bağlamlı çerçeve | etkin sınır; gönderilen en büyük `flow_seq` | sınır hiç azalmaz; gönderen sınırı aşmaz; eski oturum çerçevesi bağlam-önce dağıtımda düşer | U |
| B5 | İki yönlü doygunluk | N · I | iki yönde dosya kredide bloklu ≥ 1 bölüm; iki yönde bekleyen RECEIPT/FILE_RECEIPT; oturum değişmedi | tamamlanma; RECEIPT/FILE_RECEIPT/kredi gecikmesi | hepsi tamamlanır; canlı RECEIPT silinmez; `capacity_defect = 0`; kredi ilanı iki yönde > 0 | U · I |
| B6 | Sürekli yeni yük: sonlu hedef küme biter | R · I | hedef küme BAŞTAN sabit; ayrı arka plan üreticisi kapasite üstü yükü durmadan sürdürür ve `Backpressure` alır | hedef birim tamamlanma süreleri | her hedef birim türetilmiş sınırda; arka planın durmasına bağlı değil | U · I |
| B7 | HER BİT DÜŞMAN: beş sebep ayrık | R | beş alt koşu, her birinde tek sebep: (1) ağ kaybı; (2) krediyi aşan sahte gönderen; (3) VO süre dolumu; (4) test kancasıyla küçültülmüş havuz; (5) akış 1 tüketicisi durgun + W_CTRL'ye dayanan RECEIPT seli | sözlükteki bütün sayaçlar | her alt koşuda yalnız kendi sayacı artar; (4)'te `capacity_defect > 0` ve hüküm KIRMIZI; (5)'te `capacity_defect = 0`, canlı RECEIPT kaybı yok; ürün koşularında `capacity_defect = 0` ŞART | U · I (sütunlar) |
| B8 | PTT altında TEXT gecikmesi | N+ · I | VO desteklenen profilde ölçüm boyunca demet üretiyor | TEXT gecikme dağılımı | ölçülür/raporlanır + türetilmiş üst sınır içinde | U · I |
| B9 | Sürekli TEXT altında dosya biter | M · R · I | BE ölçüm boyunca backlog'lu — motor sayacıyla ("BE boşken BK hizmeti" = 0; örnekleme değil) | dosya tamamlanma; BK bayt payı | Q_BK payından türetilen sınırda; pay alt sınırın altına inmez | U · I |
| B10 | EK-1 §1.1 kontrol (iki yönlü) + rezerv hiyerarşisi | M · R · I | (a) VO ölçüm boyunca backlog'lu; (b) kontrol katmanı ölçüm boyunca uygun işte; (c) bütün veri akışları kredide bloklu; (d) P_BK tam taahhüt + akış 1 tüketicisi durgun; **(e) katman-2 BK rezervi kurulu iken VO ve kontrol başı uygun** | (a) ACK_V2 aralığı her iki yönde, uçuştaki birimin teyit gecikmesi, kredi ilanı, FLOW_RESET, PTT_OPEN/CLOSE, RECEIPT gecikmesi; (b) BK/BE payı; (c) kredi ilanı ve BLOCKED çıkıyor mu; (d) kabul; (e) VO/kontrol başının gecikmesi ve `reserve_wait` sayacı | (a) türetilmiş kontrol sınırı (§5 m.5); (b) kontrol ≤ ∫ρ_C·r + b_C, BK/BE alt sınırın üstünde; (c) kontrol veri kredisi olmadan gider; (d) RECEIPT/FILE_* ve TEXT kabul edilir, FLOW_RESET akış 0'da yine işlenir; (e) VO/kontrol BK rezervini beklemez (`reserve_wait` = 0), katman-2 rezervi sonlu sürede çıkar | U · I ((a)) |
| B11 | EK-1 §1.3 PTT ölçüleri ve eşik sabitleme | I (U: araçlar) | yazılı profil: hız, RTT, kayıp, kesinti ve BK/BE birim boyu; VO açık; B1/B9 yükü eşzamanlı | oynatılabilir oran; konuşma başlangıcı; uçtan uca gecikme; **gecikme bileşenleri** (iletimdeki BK paketi, katman-0 beklemesi, `reserve_wait`, `vo_rate_deficit`); sebep sayaçları | **sahibin 1. notu:** (1) KALİBRASYON koşusu (hüküm yok); (2) sahip eşikleri **ürün hedefinden — PoC-KAPSAM-001 K8 (bas→ilk ses ≤ 1,5 s / ≤ 3 s; "sıfır bozuk hece" EK-4'teki tanımıyla, DUR-06-18)** — ve kalibrasyondan seçer; `PTT-ESIK-v1` = profil, ölçü tanımları, eşikler, koşu/tohum sayısı, kalibrasyon SHA'sı, [CAL] değerleri, karar istatistiği (tohum başına en kötü); kabulden ÖNCE commit + onay; (3) kabul YENİ koşu; (4) VO yolunu etkileyen değişiklik → yeni kalibrasyon + yeni dosya + yeni kabul. Geç demet hiç oynatılmaz | U · I |
| B12 | Gönderici deadline: süresi dolan medya hiçbir kuyrukta yaşamaz; tek sonlandırma | M · R | VO birimi v süre dolmadan şu yerlerden birinde (ayrı kollar): offer kanalı, akış başına yerel tampon, runtime store, `tx_queue`, `pending_lost`, `dup_queue`, kuyruk-probu hedefi, H-sahiplenme adayı, kayıp ilanı anı; (DUR-06-11 (b) ise `pending_fec`); dönüş yolu kesik; ufuk ≥ H_maks + RTO_MAX | süre dolumundan sonra v'nin varlığı ve emisyonu; geç ACK etkisi; sonlandırma çıktıları | motor kollarında v hiçbir adaylıkta değil, tele çıkmaz, `take_finished_units`'te tam bir kez İptal; **motor öncesi kollarda (offer, yerel tampon) runtime'ın kendi iptal çıktısında tam bir kez, motor çıktısında hiç**; geç ACK ne teyit ne ikinci silme; aynı olayda teyit + iptal → Teyit; güvenilir birim süre nedeniyle iptal edilmez | U (D'de değil) |
| B13 | EK-1 §1.4 kredi yaşam döngüsü | R · A+ | (a) hiç FLOW_CREDIT görülmeden ilk birim + erken birim (`connect` K6) kolu; (b) FIN ve RESET + FIN'li birim FEC ile kurtarıldı + **FLOW_RESET birimi (akış 0) FEC ile kurtarıldı** + RESET'te ACK'li-teslim-edilmemiş birim; (c) eski oturumdan geç çerçeve gerçekten geldi; (d) Live: eski konuşmanın kayıp/süresi dolmuş demetleri + yeni konuşma + **kesinti sırasında OPEN/CLOSE kaybı** (gönderen kapısı) + **OPEN kayıp, demetler ve CLOSE önce gelir (rev5.1; DUR-06-18 (a)'ya koşullu)**; (e) TEXT akışı baştan sona açıkken K kez dosya aç-kapa + FIN sonrası geç kopya + **k+1'in ilk birimi k'nınkinden önce geldi**; (f) yükümlülük yuvaları dolu — **reddedilen FILE_BEGIN'in alındısı dahil**; (g) P_BK tam taahhüt | (a) kabul; (b) serbest kalma anı, `late_after_close`, RESET'in uygulama durgunken akış 0'da işlenmesi; (c) uygulanmama; (d) teslim + OPEN/CLOSE tekrar sayısı; (e) kayıt/bellek, k'nın birimlerinin teslimi; (f) yeni aktarım/PTT sonucu, `bk_flow_limit` ilerlemesi; (g) kabul | (a) kontrol turu beklenmez; (b) FIN görünür, kurtarılan FLOW_RESET transportça işlenir (AMP'ye teslim yok), kaynaklar serbest, `capacity_defect = 0`; (c) uygulanmaz; (d) yeni ses durmaz; OPEN/CLOSE tam bir kez (pencere altına düşmez); OPEN-önce-CLOSE kolunda OPEN kurtarılınca "başladı + bitti": oynatılabilir demetler verilir, konuşma kaybolmaz, `opens_after_close` = 0; (e) ≤ türetilmiş tavan, k'nın birimleri teslim (örtük açma), `credit_violations = 0`; (f) FILE_ABORT, FILE_RECEIPT (rejected dahil), FLOW_RESET, PTT_CLOSE, RECEIPT hiç düşmez; `bk_flow_limit` yalnız R3'te ilerler: `resolve_flow` (FILE_RECEIPT kabulü, FILE_ABORT uygulanması, ret alındısı) ya da RESET; Faz A FILE_ABORT yolunda da ilerler (rev5.1); yuva yokken yeni iş `Backpressure`; (g) kabul | U |
| B14 | EK-1 §1.5 T_birim (akışlar arası) | M | türetim koddan ÖNCE (rev6 formülü akış içi, §5 m.4); ≥ 2 akışta ≥ 2'şer teyitsiz zincir; dönüş yolu kesik; ≥ 2 × T_birim; kollar: prob/parite açık (P_pf ≠ 0) · PRE_CONVERGE (beyanlı istisna) | her birimin son hizmet yaşı, yeniden seçilmesi, ARQ→kopya geçişinde izleme | ölçülen ≤ T_birim; 31 ve 100 B/s; "ölçüldü" | U |
| B15 | FEC akışları yeniden bağlamaz; blok kimliği | R | iki akış FEC açık, her birinde kalıcı kaynak kaybı; BK blok seli (> 64 blok); **T_block ile kısmi kapanan blok + aynı akışta yeni birim**; alt mesaj bayrakları ile kurtarılan önek bayrakları çelişen sahte çerçeve | kurtarılan birimin teslim sayısı/kredisi; çapraz akış iptal/teyit; blok tahliyesi (gönderen `repair_store`, alıcı `slot.blocks`); `fec_repair_missing`; T_block; parite ve küçük paket maliyeti | her blok tek akış; RECOVERED başka akışın birimini teyit etmez; **T_block sonrası yeni birim yeni `block_id` alır, eski `block_id` yeniden kullanılmaz**; BK seli BE/CTRL bloğunu iki uçta da tahliye edemez (BE/CTRL'de `fec_repair_missing` = 0); kurtarılan birim tam bir kez; bayrak çelişkisi → fail-closed + `flag_mismatch`; maliyet raporlanır | U |
| B16 | Kuyruk derinliği (S7 denetimi) | M · R | her katmanda dosya backlog'u gerçekten var | TEXT sunulduğunda ondan önce hizmet alacak BK baytı, **her bayta neden etiketiyle** (fiziksel iletim / DRR payı / FIFO) | FIFO kaynaklı BK baytı = 0; diğerleri türetilmiş sınırda | U |
| B17 | DUR-38 regresyonu: tek akışta rev6 | M | C15 v3 §2.4 T1, T2 aynen (31 ve 100 B/s; enjeksiyonsuz; ayrı arka plan üreticisi), tek akış; + prob kolu | rev6 Ek B.6 (h) sınırları | rev6 (`37b78aad`) değerleriyle aynı | U |
| B18 | Korunan doğruluk düzeltmeleri | R · N · I | INV-ACK-DELIVER-1 akış başına (sahte ihlalci); store-miss (motor teyitli birime karar üretir); iptal yarışı; 05d RECEIPT oturum değişiminde taşınır; 05g `Ready` olmayan oturumda `open_flow`; INT-02 `ek-tuketici-durak` | sayaçlar, olaylar | ilgili mevcut testler yeni iddialarıyla yeşil; store-miss → boş batch, `stale_decisions` sayılır, sahte koordinat yok; `Closed` sonrası `UnknownFlow`; **bağımlılık:** `ek-tuketici-durak` kolu kodda henüz yok (`1b1425d2:int02_node.rs:198-205`) | U · I |
| B19 | (a) BK kimlik tükenmesi · (b) VO kabulü ve hız düşüşü | R · N | (a) test kancasıyla kimlik uzayı sona yakın, süren iki aktarım (iki yön); (b) r(t) profilin altına iner | (a) yeni aktarımın sonucu, süren aktarımların akıbeti; (b) `vo_rate_deficit`, uygulama olayı | (a) yeni aktarım tipli hata; `RenewRequired`; süren aktarımlar sessiz kaybolmaz (FILE_ABORT(superseded) + olay); (b) tipli "desteklenmeyen profil" olayı; **sesli mesaja otomatik/görünmez geçiş YOK** | U |
| B20 | Netem 7 profil regresyonu (ADR-18, §4.4) | netem (nightly) | **ön koşul (assert): bench ürün kolu S1 kipinde (`kama-product-s1`, manifest `kip = s1`)** — yoksa koşu "örtük kip; kredi/çoklu akış ölçülmedi" etiketiyle raporlanır, kapı sayılmaz; ADR-hostile-validation-contract düzeni | 7 profil hücre sonuçları | **ön-kayıtlı tahmin:** tek akışlı ürün trafiğinde sonuçlar rev6 tabanının hücre CI'ları içinde; çok akışlı kolda tamamlanma ≥ tek akış; kayarsa DUR (yeşile boyanmaz) | U |
| B21 | FILE_RECEIPT gecikmesi — S1 ölçüm tanımı | R · I | dosya kabul edildi; dönüşte yük | başlangıç = yükümlülük doğumu (dosya kabulü / ilk sunum); bitiş = FILE_RECEIPT'in ilk emisyonu; ACK ve karşı uygulamaya teslim ayrı sütun; kesinti dahil/hariç ayrı kol | 120 s yalnız FILE_RECEIPT için geçici deney hedefi (S1); normatif değil; "ölçüldü" | U · I |
| B22 | 31 B/s + kayıpta dosya tamamlanır — S2 zarfı (05f Ek B.8 ve DUR-05f-12 ile hizalı) | netem | **ön-kayıtlı iki kol** (HOSTILE-ENVELOPE §1'in dışında olduğu beyanıyla, `0c22a07d:docs/transport-v1/HOSTILE-ENVELOPE.md:1-6`): **(i) 31 B/s, 150 KiB, kayıpsız, kesintisiz** (C14 §6; ≈ 88 dk tel baytıyla, 05f B.8) · **(ii) kayıplı hücreler, 10 KiB:** rastgele kayıp {0, %10} × dönüş yolu kaybı {0, %80 (T6 benzeri)} × kesinti {yok, 1 × 60 s}; zarf E = 05f §8.1 | tamamlanma ve süre; G / R / A yerel vazgeçme sayısı; en uzun G ve R aralığı; FILE_RECEIPT gecikmesi (B21 tanımıyla); sebep sayaçları | her kolda tamamlanma; **zarf içinde yerel vazgeçme = 0 (kapı; DUR-05f-12 (a))**; en uzun G/R aralığı 05f §8.2 sınırının altında; süreler "ölçüldü"; canlı sesin aynı hızda mümkün olduğu iddia edilmez | I |
| B23 | Aynı eşe eşzamanlı dosyalar: biri diğerini kilitlemez | A+ · R | (a) iki aktarım, biri tutulu ya da kayıplı; **(b) 1 tutulu + ≥ 4 ardışık aktarım** (`bk_flow_limit0` = 4) | diğerlerinin tamamlanması | (a) diğeri tamamlanır (pompa akış başına); (b) tutulu aktarım tek yuva tutar, ardışık aktarımlar sırayla tamamlanır | U |
| B24 | Tek kabul yolu: baypas yok | A+ / derleme | `transport_mut()` ile doğrudan `send` denemesi | derleme ya da sonuç | baypas derlenmez ya da bütçeye bağlı yoldan reddedilir | U |
| B25 | Karma sürüm | N | eski ↔ yeni uç | olay, kurtarma baytı | `Ready` öncesi tipli ret; kurtarma baytı = 0 | U |
| B26 | Kontrol çerçevesi DoS sınırları | R | BLOCKED seli; u32::MAX kredi; bozuk çerçeveler (girdi sayısı aşımı, ayrılmış bit, kip değişimi); **FIN/RESET ile yarışan iyi niyetli kredi/BLOCKED girdisi**; **(rev5.1) akış-1 seli: kimliği doğrulanmış kötü niyetli eş hat hızında akış-1 birimi gönderir, alıcının akış-1 tüketicisi durgun** | CREDIT hızı ve boyu; gönderen store; ret ve `stale_ctrl_entries` sayaçları; akış-1 alıcı belleği | eş başına ≤ 1 CREDIT çerçevesi / T_min, amp ≤ 1; store ≤ yerel tavan; biçim hatası → fail-closed + kendi sayacı; yarışan girdi girdi düzeyinde yok sayılır, `stale_ctrl_entries` artar, `credit_violations` ARTMAZ, aynı çerçevedeki diğer girdiler uygulanır; akış-1 selinde alıcı belleği ≤ W_c birim + W_D bit, aşan birim `credit_violations` (ACK'siz), `capacity_defect = 0` | U |
| B27 | Bağlam-önce dağıtım FLOW_* için | R (`ctx_first_dispatch` kolu) | FLOW_* çerçevesi HubChannel / NotReady / Mismatch bağlamında | yük işlendi mi | yüke dokunulmadan düşer, sayılır | U |
| B28 | Birim çerçevesi: tek tanım, alt mesaj kısıtı, boş batch | R · M | S1 ve örtük kip; `submsg_count` 0, 1, 2; store-miss kararı | çerçeve tipi; kabul; PN ACK; sayaçlar | ürün runtime'ında tek veri çerçevesi (FLOW_BATCH; örtük kipte de); `submsg_count ≥ 2` → fail-closed + `batch_multi_rejected`; boş batch PN'i kabul kapısına girmeden ACK'lenir, teslim yok, `stale_decisions` sayılır | U |
| B29 | FIN sonrası tüketici tutması × 05f A saati (DUR-06-26 = DUR-05f-14; §16 YD-3) | R · A+ (U, sabit saat) · I (yalnız gözlem: INT-02 `ek-tuketici-durak`) | FIN dahil bütün birimler kredi içinde kabul ve teyit edildi (gönderen S2); alıcı R2'de (FIN kabul edildi, uygulamaya verilmedi). **(a) kolları:** (i) **ayırt edici:** tutma s = T_işlem + T_ctrl(E) + D_h/2 (fikstürün E değerleriyle; tutma `recv` çağrılmayarak, `hold_flow` yok); (i-mut) aynı kol D_h = 0 mutantıyla; (i′) INT-02 eşdeğeri 30 s tutma (rapor; T_ctrl payı yüzünden ayırt edici DEĞİL, beyan); (ii) tutma > D_h + T_ctrl(E); (iii) tutma sırasında alıcının oturumu biter; (iv) alıcı süreci donmuş (ne alındı ne girdi). **(b) seçilirse ek kollar:** (v) tutma ∈ (G_H, D_tut,max); (v-mut) HELD kapalı mutant; (vi) HELD çerçeveleri k_r kez ardışık düşer (deterministik) ve %80 rastgele kayıp (rapor); (vii) `held_ms = u32::MAX` gönderen eş | gönderende vazgeçme sayısı ve anı (köken t_S2); A'nın değeri; `held_entries`/`held_bytes`; FILE_RECEIPT anı | (i) vazgeçme yok, tüketici açılınca `ok` → `Delivered` · (i-mut) vazgeçme tam bir kez → mutant KIRMIZI (D_h'nin işe yaradığının kanıtı) · (i′) vazgeçme yok, "ölçüldü" · (ii) vazgeçme tam bir kez, t_S2 + T_A(E) civarında; yanlış "teslim edildi" yok · (iii) 05f §9; yanlış "teslim edildi" yok · (iv) vazgeçme t_S2 + T_A(E) içinde · (v) vazgeçme yok · (v-mut) vazgeçme → KIRMIZI · (vi) deterministik kolda vazgeçme yok (G_H bütçesi), rastgele kol rapor (DUR-05f-8 (ii)) · (vii) `held_ms` doyar; vazgeçme en geç t_S2 + T_A(E) + D_tut,max | U · I ((i′) gözlem) |
| **B30** | **Kontrol şeridinde kayıtlar birbirini beklemez** — C16 v2 §4 zorunlu karşı örnek (DUR-06-27; §3, §4.2) | **D: R** (client-core röle; kama-app yok → ölçüler S1/transport düzeyinde; "TEXT teslim onayı"nın **transport vekili**) · **U: N+ / A+** (kama-app ölçüleri) | İki uç A, B. (1) B, akış 1'e önce birim F'yi (FILE_RECEIPT; D'de opak), sonra birim R'yi (RECEIPT(m)) sunar; assert: F'nin birim kimliği / `flow_seq`'i R'ninkinden küçük. (2) Röle F'nin bütün iletimlerini (ilk, ARQ, kopya; U'da F'nin bloğunun paritesi de) **ölçüm penceresi W_ölç boyunca** düşürür; W_ölç ≥ RTO_MAX + türetilmiş ilk emisyon sınırı, ön-kayıtlı. (3) **Boş geçmeme ön koşulu:** F'nin düşürülmesi sürerken R'nin röleden geçiş sayısı ≥ 1 (röle sayacı). (4) U FEC kolunda ayrıca pencere boyunca `fec_recovered(F) = 0` assert. Kollar: (a) ana kol; (b) F(t1) kayıp, F(t2) geçer; (c) **R'nin ACK'i tutulur** → ARQ R'yi yeni PN'le yeniden gönderir + kopya kolu (aynı-PN tekrarı yalnız anti-replay denetimidir: Noise replay penceresi reddeder, `dup_ctrl = 0`); (d) F kayıp, ardından ≥ W_D kayıt (test kancasıyla küçük W_D); (e) ters kol: R kayıp, F geçer; (f) U: akış 0 kardeşi — FLOW_RESET(k1) düşer, FLOW_RESET(k2) işlenir | **D:** A'da R biriminin S1 `Delivered` anı (akış 1, `flow_seq`); F'nin S1 teslim sayısı; `dup_ctrl`; akış-1 `max_seq`, BLOCKED · **U:** `Delivered{m}`, `receipts_ignored`, t'nin `Delivered`'ı | **Sıra tabanlı iddialar (ön-kayıtlı):** **taban (H, G) ve `Ordered` kolu:** R ancak F teslim edildikten SONRA teslim edilir → KIRMIZI · **`Independent` kolu:** R, F'nin S1 teslim sayısı = 0 iken ve düşürme sürerken A'da S1-teslim edilir; F, düşürme bitince tam bir kez · (b) F(t2), F(t1)'den önce · (c) R'nin S1 teslimi tam bir kez, `dup_ctrl` ≥ 1; aynı-PN tekrarında `dup_ctrl = 0` · (d) ilk W_D − 1 kayıt teslim edilir, W_D-ıncı kredi bekler (BLOCKED); `credit_violations = 0`, `capacity_defect = 0`; F gelince akış sürer · (e) F, R'yi beklemez · (f) k2'nin kaynakları k1'i beklemeden serbest · her kolda `capacity_defect = 0`, `credit_violations = 0` · **U:** `Delivered{m}` aynı sıra iddiasıyla; t'nin `Delivered`'ı tam bir kez · **DUR-06-27 (b)'nin sonucu ölçülmez:** ana kol yeşil, (b) ve (e) kolları kırmızı beklenir [çıkarım] | D (K1: taban H ve G; K4: `Ordered` ve `Independent` kolları (a)–(e)) · U ((f) ve kama-app ölçüleri) |
| B31 | İptal akışa bağlıdır; erken iptal kaydı; `FlowEnded` sırası (DUR-06-28; §16 YD-2, YD-4, YD-5, YD-10) | R · A+ | Kollar: (a) Faz G iptali; FILE_BEGIN(t) biriminin bütün iletimleri düşer, RESET gider; (b) Faz G iptali; FILE_BEGIN okunmuşken; (c) Faz G iptali; FILE_BEGIN alıcıda ACK'li ama okunmamış (tüketici durgun), RESET tamponu atar; (d) Faz A dolumu (FILE_ABORT akış 1); FILE_BEGIN henüz okunmamış (R0 akış var); (e) oturum değişimi: `superseded` FILE_ABORT, FILE_BEGIN eski oturumun drain'inde; (f) Faz G'de alıcı reddi ya da alıcı iptali (`stop_flow`); (g) art arda iki supersession + hızlı iptaller (C_EA = 12'ye yaklaşan); **(h) FILE_RECEIPT(ok) kaybolur → A dolumu → FILE_ABORT(timeout); alıcı aktarımı bitirmiş (R0 akış yok);** **(i) aynısı bir supersession üzerinden (FILE_RECEIPT kabul edildi, teslim edilmeden oturum değişti);** **(j) oturum `floor ≤ fin_seq` iken bitti → Faz A → A dolumu yeni oturumda;** **(k) FILE_BEGIN adaptörün inbox'ında okunmamışken FLOW_RESET işlenir;** (l) G dolumu FIN sunulduktan sonra, alıcı aktarımı bitirmiş (üçüncü durum); (m) Faz G iptali, RESET teyidinden önce oturum değişti; (n) bilinmeyen RESET kodu | alıcı olayları; erken iptal kaydı sayısı; `aborts_after_terminal`; `early_abort_overflow`; gönderenin terminal olayı; `bk_flow_limit`; telde FLOW_RESET sayısı; boşa bayt | (a) alıcıda olay yok, kayıt = 0 · (b) `Aborted(user_cancel)` tam bir kez · (c) olay yok, kayıt = 0, `capacity_defect = 0` · (d) kayıt doğar; FILE_BEGIN okununca `rejected` ve kayıt düşer; **`resolve_flow` sonrası `bk_flow_limit` +1** · (e) eski oturum `Closed` olunca kayıt düşer (zamanlayıcısız) · (f) gönderende `SendFailed(statü)` tam bir kez + `reset_flow_local`; **telde FLOW_RESET = 0**; `stop_flow` sonrası alıcı uygulamasına teslim yok; boşa bayt ≤ BLOCKED + yoldaki birimler · (g) kayıt ≤ C_EA, `early_abort_overflow = 0` · **(h), (i) kalıcı kayıt yok, `aborts_after_terminal` +1** · (j) kayıt doğmaz ya da eski oturum kapanınca düşer · **(k) uygulama FILE_BEGIN'i ve olayı görmez ya da önce FILE_BEGIN'i sonra `Aborted`'ı görür; hayalet aktarım yok** · (l) dosya alıcıda, gönderende yerel vazgeçme; sonra gelen `ok` yok sayılır (beyanlı) · (m) yeni oturumda FILE_ABORT(`user_cancel`); alıcıda `Aborted(user_cancel)` · (n) `Aborted { reason: Unspecified }` · her aktarımın tek terminal olayı | U |

**Deney dalında koşulanlar (yalnız §14; iki taban, hüküm G'de):** B1 (en küçük), B1-taban, B1b, B2 (taban üç kol H ve G; yeni kod (a), (b), katı öncelik seçicisiyle), **B30** (taban H ve G; yeni kodda akış 1 `Ordered` ve `Independent` kolları + (b)(c) kolları), B3 (b)(e) en küçük hâli (isteğe bağlı, sıra 2) + regresyon alt kümesi. Diğerleri U/I'dir.

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
| `client-core/tests/api_surface_v0.rs:70` `transport_api_v0_surface_is_frozen` (:373-376, :415-419; `Delivery` :246-257) | wire-v0 imzaları, `Delivery{unit_id}` | D8 (A.28) |
| `fuzz/fuzz_targets/wire_v0.rs:10` (+ 7 tohum `fuzz/corpus/wire_v0/`) | panik yok | D8: yeni decode'lar eklenir |
| `client-core/tests/runtime_e2e.rs:1287` `arq_identity_is_the_pn_not_the_interim_seq_field` (:1264-1268) | `seq u16` alanı okunmaz | D8 (B'de FLOW_BATCH'te `seq` alanı yok; iddia "ARQ kimliği PN" K) |
| `runtime_e2e.rs:423`, `:602`, `:1125` (DropUnitsSeam :402-404) | birim seçimi `decode_data` ile | D8 (akış koordinatı) |
| `client-core/tests/ctx_first_dispatch.rs:628`, `:684`, `:762` (yardımcı :206); `:444`, `:473`, `:501`, `:533`, `:557`, `:580` (ClientRig :331) | ham DATA çerçevesi | D8 (yardımcı güncellenir; bağlam-önce iddiaları K) |
| `session_keyed_slots.rs` yardımcıları `:260`, `:697` → testler `:375, :442, :508, :622, :735, :827, :936, :1042` | ham `encode_data` | D8 yardımcı |
| `client-core/tests/k4g05b_runtime_ownership.rs:316` (NetState) → testler `:862, :912, :1010, :1086, :1165, :1209, :1267, :1419, :1527, :1587, :1639` | röle çerçeveyi `decode_data`/`decode_fec` ile sınıflar | D8 yardımcı; sahiplik iddiaları K |

**C.C — Mesafe kapısı / oturum penceresi / sıra başı rezervi / teslim kanalı** — 18 test
| Test | İddia | İşlem |
|---|---|---|
| `client-core/tests/transport_v1_freeze.rs:104` `sinirlar_ve_kotalar_donmus` (:106-108) | 4096 / 1024 / 64 | D8 (A.14) |
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
| `api_surface_v0.rs:77-108` | `RuntimeConfig` tam deseni | D8 (A.14/A.28) |
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
| `adaptive-transport/tests/tv_arq.rs:19 (:39), :57, :106 (:122), :144 (:152), :163` | `DeadlineArqScheduler` sırası | K (A.29) |

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
| `transport_v1_freeze.rs:27` (:28-29) `kimlik_sabitleri_donmus`; `:34, :44, :71, :84, :104` | D8 (DUR-06-13 kimlik; A.14 alanlar) |
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
| derleme pinleri `error.rs:218-250`, `:374-440` (tam `From` eşlemeleri) | D8 (A.24) |

**C.L — Kapılar (rev1'de yoktu; F13, E4)**
| Kapı | İşlem |
|---|---|
| netem 7 profil (ADR-18, nightly; §4.4) | ön-kayıtlı tahminle yeniden koşu (B20); kayarsa DUR |
| T-hücreleri (ADR-hostile-validation-contract) | yeni kimlikle yeniden koşu (A.26) |
| bench ürün kolu bayt muhasebesi (`bench/src/kama_product.rs`, `bench/src/manifest.rs`) | D8 (kimlik + yeni sınıf sayaçları) |
| `scripts/netem/int02_analyze.py` hükümleri (`1b1425d2:…:18` `delivery_channel_drops = 0`; :969-1011 PTT `in_order_complete`; :1134-1141 öz-test fikstürleri) | D8 (A.25, DUR-06-22) |

**C.M — Kontrol şeridi kipi, iptalin akışa bağlanması, erken iptal kaydı, FIN sonrası tutma (rev5; DUR-06-26/-27/-28)** — 9 test (`0c22a07d`)
| Test | İddia | İşlem |
|---|---|---|
| `kama-app/tests/text_receipt.rs:564` `text_is_delivered_once_and_receipted_once` (:576-580) | tekrar ve bilinmeyen RECEIPT olay üretmez | K (`Independent` şeritte idempotentliğin dayanağı) |
| `text_receipt.rs:585` `receiver_accepts_out_of_order_msg_ids` | sırasız `msg_id` kabul edilir | K |
| `kama-app/src/file_transfer.rs:2349` `file_abort_and_file_receipt_apply_to_opposite_directions` | iki kayıt zıt yönlere uygulanır | K |
| `src/file_transfer.rs:2531` `early_abort_record_is_bounded_and_expires` | eş başına 2, toplam tavan, ömür `FILE_IDLE_TTL` | D8 (zamanlayıcısız düşme, tavan 2·`bk_flow_limit0`; §16 YD-5) |
| `kama-app/tests/file_transfer.rs:1882` `synchronous_supersession_aborts_and_an_overtaken_file_begin_is_rejected`; `:1841` `supersession_closes_a_half_transfer_with_file_abort_superseded` | oturumlar arası: geçilen FILE_BEGIN reddedilir; `superseded` FILE_ABORT | K (oturumlar arası kural aynı) |
| `tests/file_transfer.rs:1777` `a_cancel_while_an_abort_is_pending_adds_no_second_event` | Faz G iptali FILE_ABORT ile; tek olay | D8 (Faz G iptali `reset_flow(k, kod)`; tek olay iddiası K; DUR-06-28 (a)) |
| `tests/file_transfer.rs:1479` `receiver_cancel_sends_aborted_by_receiver_and_the_sender_stops` | alıcı iptali; gönderen durur | D8 (`stop_flow` + gönderen `reset_flow(k, 0x00)`; §16 YD-4) |
| `tests/file_transfer.rs:1091` `sender_receipt_timeout_fails_the_transfer_and_a_queued_receipt_wins`; `:1176` `sender_receipt_timer_starts_when_file_end_is_accepted` | `FILE_RECEIPT_TTL` | D8 (05f A saati + `HELD`; §16 YD-3, YD-7) |
| sahte A+ (Ek B düzenekleri) | — | F: `Independent` kip, RESET kodu, `stop_flow`, `HELD` |

**Toplam (kesişimli):** A 16 · B 34 · C 18 · D 16 · E 33 · F 149 literal (eklemeli API'de K) + 4 pin · G 4 + düzenek · H 12 · I 16 · J 14 + 20 · K 2 · L 4 kapı · M 9 (rev5). kama-engine taranmadı değil: A.30 (eklemeli API'de değişmez hedefi); kama-noise-wire ve kama-store testleri A.6/A.35 gereği değişmez hedefidir, uygulama turunda doğrulanır.

## Ek D — DUR'lar (CEVAP 16 v2 kararları işlendi; açıklar CEVAP 17'ye): seçenekler, öneri, gerekçe — öncelik sırasıyla

C11 §3 süzgeci: DUR yalnız değişmez/sözleşme metni, yeni sayı, tel/vektör, FROZEN dosya, beyanlı sınır ya da bir testin amacı/eşiği değişince. Uygulama tercihleri ve deney başlangıç değerleri DUR değildir; orkestratör seçer ve raporlar. **Canlılık/kilitlenme açığı "deneyde görülür" diye bırakılmaz:** rev5'te bilinen her canlılık açığının ya metinde çözümü ya da aşağıda açık DUR'u vardır.

### Öncelik 1 — deneyden ÖNCE

**DUR-06-5 — Birim çerçevesi, FEC kimliği, KARAR-1/ccr tarihli eki, `flow_id` uzayı** (A.8–A.11, A.27).
**KARAR (CEVAP 16 v2 §4, 2026-10-26): (B + B-iii) yön GO — yalnız deney dalı için.** KARAR-1 ve ccr §35.14'e tarihli sahip ekinin GEREKLİ olduğu KABUL edildi. Ek şimdi YAZILMAZ; ürün aşamasında (Öncelik 3 onay turu) Ek A'nın tam yama metniyle yazılı karar olur. ccr KAT-1..5 donmuş spec'tir: zarf içi olsa da sessiz değişmez. Hub'ın gördüğü zarf değişmez (Yaaz etkilenmez). Aşağıdaki seçenek tablosu kayıt içindir.
Seçenekler: (W1) birim kimliği telde kalır + akış başlığı, yeni interim çerçeve · (W2) birim kimliği telden çıkar, yeni interim çerçeve (rev1) · **(B) akış başlığı `BatchSubMessage`'ta + FEC B-iii** · (B′) KARAR-1'i tarihli ekle değiştirip W2'yi canonical yapmak.

| Ölçüt | W1 | W2 | **B (+ B-iii)** |
|---|---|---|---|
| Veri başlığı, kayıt ≤ 239 / ≥ 240 B [hesap; bugün 11 B] | 16 / 16 B | 8 / 8 B | **10 / 12 B** |
| Onarım sembolü başlığı [hesap; bugün 12 B] | 12 B | 14 B | **16 B** (`first_seq` +4; `flow_id` ölü `seq` alanında) |
| KARAR-1 (`1b1425d2:docs/seams/KM-SEAM-TC-001.md:82`) | ✗ yeni interim çerçeve ("iki çerçeve tanımı" reddedildi) | ✗ aynı | alt mesaj çerçevesi canonical kalır; **ama tarihli KARAR-1/ccr §35.14 eki gerekir**: tip baytı, alt mesaj başlığı, `submsg_count ∈ {0,1}`, 0x05 istisnası; örtük kip de FLOW_BATCH → ürün runtime'ında tek çerçeve |
| RECOVERED → birim (bugün `block·K + esi`, `0c22a07d:runtime.rs:2379-2382`) | oturum geneli blokta çalışır ama blok akışları bağlar | çerçeve + tablo gerekir | gönderen blok tablosu (`block_id` → akış, `first_seq`) + ters dizin; başka akışın birimi teyit edilemez |
| Blok numarası çakışması / yeniden kullanımı | yok | **VAR** | yok: `block_id` oturum boyu monoton (§2.6 ID-SCOPE aynen) |
| T_block kısmi blok kapanışı | — | `flow_seq div K` ile çakışır | serbest; sonraki birim yeni blok |
| FIN / REL FEC kurtarımında | görünmez | görünmez | `source_flags`'ta → görünür; akış ve sıra önekte (`chunk_offset := flow_id`, `message_id := flow_seq`) |
| Store-miss (`runtime.rs:2263-2288`) | koordinatsız çerçeve sorunlu | koordinat yok | `[tip][boş BatchPayload]` 2 B; PN ACK'lenir, teslim yok |
| `pop_first` tahliyesi (`runtime.rs:2715-2716`, :3894-3895) | en eski | en küçük akış (yanlış) | monoton `block_id` + sınıf başına depo → sınıf içinde en eski |
| Motor blok tipleri | aynı | çakışma riski | aynı; motor blok kimliğini yalnız eşitlikle kullanır [olgu: `37b78aad:sender.rs:5690`, :5729-5735, :6170-6173] |

**Öneri B + B-iii.** Gerekçe: alt mesaj çerçevesini canonical tutan tek seçenek; W2'nin ve rev2 B'sinin FEC kusurlarını (yeniden kullanım, T_block çelişkisi, çapraz akış teyidi, bayrak görünmezliği, ters tahliye) yapısal olarak kapatır; veri başlığında bugünkü 11 B'den ≤ 1 B fazla. **"KARAR-1 ile çelişmez" demiyoruz: tarihli ek gerekir.** Alt sorular:
- (i) Başlık alt mesajda (KATMAN 6 emsali) — öneri; BatchPayload üstünde (alternatif; tek alt mesajda bayt eşit).
- (ii) S1 parçalamasız kipte `message_id := flow_seq` (NO-REUSE kapsamı akış + yöne daralır), `chunk_offset := flow_id`.
- (iii) `flow_id` u16; kalıcı 0–4, dinamik BK 5…65535; tükenmede `RenewRequired` (u32 +2 B yalnız tükenme ölçülürse). Kalıcı akışa close/reset → `UnknownFlow`.
- (iv) Kredi taşıyıcısı (eski DUR-06-8): FLOW_CREDIT ayrı kontrol çerçevesi, alıcının ACK yolunda. BatchSubMessage yükü AMP kaydıdır (ccr §35.14); transport kontrolünü oraya koymak sahiplik sınırını bozar; S2'de kontrol ayrı `inner_kind`. ACK_V2 ile aynı mühürde birleştirme FROZEN motor codec'ini değiştirir; bayt kazancı [hesap: datagram başına 22/26 B] ölçülmeden takas yapılmaz.
- (v) **FEC blok sınırı:** (v-a) `(flow_block u16, esi u8)` her veri alt mesajında (+3 B/birim → 13/15 B; blok başına 8 × 3 = 24 B [hesap]) · (v-b) T_block kaldırılır, kısmi blok yalnız FIN'de (kalıcı TEXT/CTRL akışında son < K birim FEC'siz: `0c22a07d:runtime.rs:2758-2762`'deki B-2 kusuru geri gelir) · **(v-c) blok sınırı onarım çerçevesinde (`first_seq` u32) + kendini tanıtan önek** (onarım sembolü başına +4 B; r = 2'de blok başına 8 B [hesap]). Öneri (v-c).
- (vi) Örtük kip: **FLOW_BATCH'e geçer (tek kalıcı akış 2)**, 0x00 emekli; 0x05 yalnız açık opt-in parçalayıcıda beyanlı istisna — alternatif: KARAR-1 ekine "örtük kipte 0x00 kalır" istisnası (reddedilir: iki çerçeve tanımı).
- (vii) S2 eşlemesi: extended yolda FLOW_BATCH için yeni `inner_kind` (0x1D+); legacy yolda ayrım baytı ccr'de yok → ekle karara bağlanır. S2'nin FEC_SOURCE yolu `block_id`/`esi`'yi zaten taşır; akış ve sıra önekte → S2 FEC düzeni değişmez.

**Kalan:** ek metni ve kesin bayt düzeni ürün aşamasında (A.9); deney yalnız yön kararıyla yazılır.

**DUR-06-2 — S7 seçim sözleşmesi ve rezerv hiyerarşisi** (A.17, A.16, A.20).
**KARAR (CEVAP 16 v2 §4): (b2) + (i) yön GO.** Motor içinde tek saf seçici; eklemeli `Event::AppDataTagged(OpaqueUnit, ServiceTag { tier, flow (opak), expires_ns })`; etiketsiz = tek akış = rev6 davranışı; rezerv hiyerarşisi (katman 0/1 kova payı içinde katman-2 rezervinin önüne geçer; K1 (a) katman 2 içinde aynen). Motor kama-app mesaj tipini çözmez. **DUR-38 ile ORTAK seçim/rezerv mantığı:** iki istisna sistemi üst üste büyümez (EK-1 §1.5) — ürün kodunda rev6'nın `ServiceRights` / rezerv yapısı seçicinin düzey-2'si olur, yanına ikinci bir rezerv mekanizması yazılmaz (A.17).
Sözleşme: (a) sığ giriş · (b1) `OpaqueUnit`'e alan (149 literal D8; K3 +1–2 gün) · **(b2) eklemeli `Event::AppDataTagged`** · (c) enjekte `ServiceSelector`. Rezerv: **(i) katman başına rezerv; katman 0/1 kova payları içinde katman-2 rezervinin önüne geçer; K1 (a) katman 2 içinde aynen** · (ii) tek küresel rezerv, K1 (a) bütün katmanlara (VO ve kontrol BK başının token birikimini bekler: 300 B/s'de 1050 B için +3,5 s [hesap]; m.5 ve B11'e "rezerv beklemesi" terimi eklenir). **Öneri (b2) + (i).** Gerekçe: (a) önceden kabul edilmiş kurtarma işini sıralamaz (B2 tabanı KIRMIZI beklenir); (b2) pinleri korur, tek akışta rev6'ya indirgenir; (c) SoC/ADR-17 denetimini zorlaştırır; (i) §5 m.3'ün (1 − ρ_C − ρ_V,max)·r paydasıyla tutarlıdır ve K8'in bas→ilk ses hedefine BK rezerv beklemesi eklemez; katman 0/1 kovayla sınırlı olduğundan katman-2 rezervinin ömrü sonlu kalır. Bedel: FROZEN v1.1 motor revizyonu ve AMENDMENT-12 cümleleri (A.16).

**DUR-06-20 — Deney kapsamı ve FEC kolu** (§14).
**KARAR (CEVAP 16 v2 §4): (a) KOŞULLU.** Deney kodunda FEC kapalı kabul (ürün profili dışı; B2'nin FEC kolu yalnız tabanda). **Koşul:** deney İKİ tabanda koşar — H `0c22a07d` (rev5 motorlu; tarihsel karşılaştırma) VE G = rev6 (#140 merge'ü) + yeniden tabanlanmış 05c'nin güncel birleşik adayı (hazır olunca). **Karar verdiren koşu G'dir;** bilinen eski zamanlayıcı kusurları yeni mimari kararını belirlemez. S7(a)'nın reddi ön-kayıtlıdır (B2: TEXT'in KENDİ kurtarma işi; iki taban). Uygulaması §14'te: koşu matrisi, ön-kayıt (T_BE^rec(H), T_BE^rec(G)), aday hazır değilken H'de yazıp G'ye taşıma ve maliyeti (≈ 2,25–3,5 iş günü [tahmin]). Kayıt için seçenekler: (a) yukarıdaki · (b) yeni kodda FEC dahil (+2–3 gün) · (c) yalnız K1.

**DUR-06-27 — Kontrol şeritlerinin (akış 0 ve 1) teslim kipi** (§3, §4.1–§4.2; A.1, A.7, A.13, A.18, A.21; B30). Kaynak: CEVAP 16 v2 §4'ün zorunlu karşı örneği (INCELEMECI-8). Seçenekler:
- (a) tek `Ordered` akış 1 (rev4);
- (b) ayrı `Ordered` şeritler (RECEIPT / dosya kontrolü);
- **(c) akış 0 ve 1 `Independent`:** sırasız; oturum içinde tam bir kez. Tekilleştirme penceresi (`floor_a` + W_D bit) ile tüketim kredisi (`c` + W_c) ayrıdır (§3, rev5.1). Oturumlar arası tekrar uygulama kimliğiyle ayıklanır;
- (d) kayıt başına sıralama anahtarı.

**Öneri (c).** Gerekçe:
- AMP denetimi (§4.1) akış 1'in üç kaydında ve akış 0'ın tek kaydında şerit içi sıra gereksinimi bulmadı. RECEIPT ve FILE_RECEIPT kodda kimlikle idempotent: `0c22a07d:messaging.rs:833-855`, `file_transfer.rs:1135-1160`; testler `kama-app/tests/text_receipt.rs:564`, `src/file_transfer.rs:2349`. FILE_ABORT'un bitmiş aktarımdaki sınırı §16 YD-5'te.
- Eşit ölçütle karşılaştırma §4.2'deki tablodadır. Kalanlar: (b)'de şerit içinde 1 derinlik HOL (RECEIPT ↔ RECEIPT, FILE_RECEIPT(t1) ↔ (t2)); (c)'de W_D derinliği, zarf içinde erişilemeyecek biçimde türetilmiş (§3 [hesap]).
- "Tel değişmez" iki seçenekte de doğrudur, ayırt edici değildir. (b)'nin B30 sonucu deneyde ölçülmez, [çıkarım] olarak yazılır.
- (d) transporta uygulama anlamı taşır.

Bedel (c): yeni teslim kipi — FROZEN S1 kapalı kümesi değişir (A.1); seam değişmezi (A.7); alıcıda şerit başına W_D bit; kama-app'te FILE_* kredi payı (§7.6). **Kalan beyanı:** kontrol kaydı aynı şeritteki başka bir kaydın kurtarılmasını W_D − 1 kayda dek beklemez; ötesinde `floor_a` ilerleyene dek kredi bekler (kayıp yok, ihlal yok). Sırasızlık yalnız 0 ve 1'e uygulanır; TEXT, BK ve VO sıra kuralları aynen kalır (körlemesine sırasızlık yok). **Deneyden önce:** yön. Deney K2'de akış 1'in kipi anahtarlıdır (`Ordered` | `Independent`): `Ordered` kolu (a)'yı, `Independent` kolu (c)'yi sınar (B30).

### Öncelik 2 — deney sonucunu bekler

**DUR-06-10 — VO açıkken BK/BE birim boyu.** **(a) PoC'de beyan + ölçüm** (VO gecikmesi ≥ S_BK/r; B11 gecikme bileşeni) · (b) sınıf başına dinamik `preferred_plaintext_unit_bytes` · (c) BK transport parçalaması. Öneri (a): aktarım ortasında parça boyu değişemez (`file_transfer.rs:55-63`).

**DUR-06-11 — FEC kapsamı (Ordered).** **(a) akış başına blok (B-iii); kısmi blok T_block içinde; alıcı ve gönderen blok depoları sınıf başına** · (b) oturum geneli blok (reddedilir). Öneri (a). Live kısmı öncelik 3'te (DUR-06-18'e koşullu). ADR-fec-v1-role Decision 1 değişir (A.26).

**DUR-06-3 — Kontrol katmanının açık kalan kısmı.** Normatif sayılar (ρ_C, b_C, W_TCTRL0, W_CTRL0, T_min, havuzlar) deney ölçümünden sonra. Bellek: [hesap] [CAL] değerleriyle oturum başına ≈ 360 birim / ≈ 320 KiB; `max_peers` 64'te ≈ 23 040 birim / ≈ 20 MiB → kabul edilemez; seçenekler: (a) S1 oturum tavanı ≤ 8 (PoC'de iki telefon) · (b) W0 küçültme · (c) düğüm geneli havuz + oturum başına alt sınır. Öneri (a) + (c). Envanter:

| Kayıt | Yol | İlerlettiği | Hizmet hakkı | Maliyet [hesap: mühür 26 B dahil] | 31 B/s | 300 B/s |
|---|---|---|---|---|---|---|
| ACK_V2 (transport ACK; PN) | alıcı sunucu soketi, pacer dışı (`0c22a07d:runtime.rs:3971-3981`) | gönderen teyit, RTT | ≤ DATA/ACK_FREQ_N (2, `adaptive-transport/src/receiver.rs:76`) + 20 ms zamanlayıcı | 45 B (1 aralık) | 1050 B birimle ≈ 1,3 B/s ters | ≈ 6,5–13 B/s ters |
| FLOW_CREDIT (akış + sayım girdileri, 11 B/girdi) | alıcı ACK yolu; eş başına ≤ 1 çerçeve / T_min | gönderenin yeni verisi, yeni akış, yeni REL | tetikli (§7.4) | 2 + 11·n + 26 B; n = 1: 39 B | W0_BK 64 → ≈ 0,04 B/s; BLOCKED bölümünde ≤ 1,3 B/s | ≈ 0,35 B/s |
| FLOW_BLOCKED (11 B/girdi, yanıt boyunda) | gönderen katman 0 (`TransmitKind::Control`) | alıcının yeniden ilanı | katman 0 içinde önce; aralık srtt → ×2 → ≤ 30 s | 2 + 11·n + 26 B | n = 3: 61 B / 30 s ≈ 2,0 B/s (ρ_C·r = 3,1 B/s içinde) | aynı |
| FLOW_RESET | akış 0 (transport tüketir) | alıcının akış kaynakları | katman 0; W_TCTRL | iptal başına | — | — |
| RECEIPT(delivered) — uygulama alındısı, ACK değil | akış 1 | gönderen outbox | katman 0; W_CTRL; 05d tut-ve-dur | TEXT başına bir kayıt | TEXT hızıyla | TEXT hızıyla |
| FILE_RECEIPT (rejected dahil) / FILE_ABORT | akış 1 | aktarım sonu | katman 0; yuva akış açılışında | gelen BK akışı başına 1 / iptal başına 1 | — | — |
| PTT_OPEN / PTT_CLOSE (REL) | akış 4 | konuşma başı/sonu | katman 1; `max_rel` | konuşma başına ≤ 3 | VO yok | konuşma başına |
| PARAM_SET/ACK, RECOVERED, CS_PROBE/ECHO | mevcut (PARAM sınırlı tekrar `runtime.rs:2732-2743`; CS probu ürün profilinde kapalı) | FEC bağlaması, teyit, kuruluş | mevcut; akışsız problar katman 0 | mevcut | — | — |
| SSI1/SSR1 (INIT/RESPONSE; yenileme, supersession) | `EnvelopeLink`, veri runtime'ı dışında (`0c22a07d:transport.rs:152-181`) | oturum kurulumu/yenilemesi | veri yükünden bağımsız | oturum olayı başına | — | — |
| HANDLE_ASSIGN, HEARTBEAT 0x1B/0x1C | hub kanalı | hub yolu | direct-mode/PoC'de yok (S4) | — | — | — |

Sürekli dosya aktarımında toplam kontrol [hesap]: 31 B/s'de ≈ 1,3 + 0,04 ≈ 1,4 B/s (≈ %4,5); n bloklu akışta BLOCKED ≈ (2 + 11·n + 26) / 30 B/s her yönde.

**DUR-06-17 — §5/§7 sınırlarının sözleşmeye girişi.** **(a) KM-SEAM-AT-001 §4 ve engine §13'e "koşullu garanti" biçiminde** (05b K2 deseni) · (b) yalnız notta (C15 v3 §2.2 ile reddedilir). Öneri (a).

### Öncelik 3 — uygulamadan önce (ayrı onay turu)

**DUR-06-18 — PoC-KAPSAM-001 EK-4: PTT, K8 ve S5** (A.18, A.19, A.24, A.4). Belge "KESİN; yalnız Abkarada'nın tarihli ekiyle" (`1b1425d2:docs/decisions/PoC-KAPSAM-001.md:7-8`). Değişen satırlar (a) seçilirse: §2 PTT "Reliable klip … bozulmaz, durur ('bekliyor' göstergesi)" (:26) · §2 "Sözleşmeler" S5 "FROZEN sayılır, değişmez" (:34) · K6 "facade değişmez" (:54) · K8 PTT "sıfır bozuk hece" (:62) · EK-2 md. 1 (:98) · EK-3 (:27, :103-105). Seçenekler: **(a) EK-4: canlı PTT** — OPEN/CLOSE `Reliable` (REL), demet `DeadlineBounded` (D_VO); geç demet oynatılmaz; "sıfır bozuk hece" EK-4'te yeniden tanımlanır ("eksik demet sessizlik/PLC; bozuk çerçeve hiç oynatılmaz; oynatılabilir oran ≥ eşik"); S5/K6, `PttBundle.flow_seq` ve `pttPush` kuralına izin verecek biçimde değişir; EK-3 "yalnız Reliable" BatchPayload geçişiyle kalkar · (b) Reliable klip kalır: PTT kendi VO akışında `Ordered Reliable`; K8, S5, K6 AYNEN; Live, süre dolumu, REL ve dedup penceresi PoC dışı → K4G-06 kapsamı ve süresi küçülür; DUR-06-1/-6/-11 (Live)/-23 düşer · (c) canlı varsayılan + Reliable klip yalnız açık "sesli mesaj" eylemi olarak. **Öneri (a)** (sahibin ifadesi: "Canlı konuşmada eski sesi sonsuza kadar kurtarmaya çalışmak yanlış davranıştır"); (b) bilinçli kapsam küçültme seçeneği.

**DUR-06-1 — Canlı medya kredisi** (DUR-06-18 (a)'ya koşullu). (a) aynı kredi (süresi dolan demet kredi "deliği" bırakır) · **(b) ayrı sınırlı kabul:** demet `flow_seq` tüketir ama seq kredisinden muaf; P_VO havuzu; doluysa yalnız REL'siz birim düşer, ACK'lenir (`media_dropped_rx`); REL birim `max_rel` ile sınırlı ve asla düşmez · (c) karma. **Öneri (b).** QUIC DATAGRAM akış kredisi kullanmaz (RFC 9221 §5.3).

**DUR-06-6 — Live teslim, REL ve gönderen kapısı** (DUR-06-18 (a)'ya koşullu). **(a) sırasız + `flow_seq` + alıcı dedup penceresi W_LIVE + REL bayrağı + gönderen kapısı (en eski teyitsiz REL ile yeni `flow_seq` arası < W_LIVE)**; W_LIVE ≥ ⌈(D_VO + 2·srtt) / bundle_ms⌉ [türetim taslağı]; kapı sayesinde REL birim pencere altına düşmez ("tam bir kez" yapısal); bedel: kesintide yeni VO birimi bekler ve süresi dolar · (b) sıralı + alıcı boşluk zamanlayıcısı · (c) REL birimler için ayrı mutlak dedup kümesi (`rel_seq` +2 B/REL birimi). **Öneri (a).** rev2'nin "W_LIVE ≥ (RTO_MAX + D_VO)/bundle_ms" türetimi geri çekildi (REL'i korumuyordu).

**DUR-06-11 (Live kısmı)** (DUR-06-18 (a)'ya koşullu): Live ilk uygulamada FEC dışı; değeri ayrı ölçüm (`0c22a07d:runtime.rs:1595-1599`).

**DUR-06-23 — UniFFI/S5 yüzeyi** (A.24; **Hasan ile ortak onay + DUR-06-18 EK-4**). `PttBundle.flow_seq` ve anlamı; `pttPush` için `DeadlineExpired`, canlı kipte "reddedileni yeniden ver" kuralının kalkması; `UnknownFlow` aynası; yeni `NodeMetrics` sayaçları. Kotlin facade'a dokunur (oynatıcı sıra bilgisi, "bekliyor" davranışı). (b) seçilirse yalnız `UnknownFlow` ve sayaçlar — o da S5'e girdiği için yine EK-4 ister.

**DUR-06-7 — S1 yüzeyi ve `early_unit`** (A.1–A.5, A.7). **(a) `send(flow, …)` + `open_flow`/`close_flow`/`reset_flow`/`hold_flow` + `FlowOpened`/`FlowEnded` + `UnknownFlow`; `connect(peer, early: Option<(FlowClass, PreparedUnit)>)`** · (b) `send_on` + gizli BE akışı · (c) `recv_flow`. Öneri (a). VO "desteklenmeyen profil": yeni varyant yok; `Backpressure` + yetenek alanı + olay.

**DUR-06-13 — Sürüm kimliği ve karma sürüm** (A.14, A.26, A.34). Kimlik: **(b) `transport-v2` / `kama-v2-full`**. Karma sürüm: **(i) sürüm/yetenek el sıkışmada (ccr §35.2/§35.3), `Ready` öncesi fail-closed; `ConnectAbandoned(LocalPermanent)` + sayaç** · (ii) legacy AAD `protocol_version` (sessiz AEAD reddi → reddedilir) · (iii) yalnız beyan. Öneri (b) + (i).

**DUR-06-14 — 05d A1/M2 devri** (A.21, A.22). **(a) A1 ayrıca kodlanmaz; akış bütçesi erişimcisine dönüşür; W_F → BK penceresi; H_C → şerit başına H_C_tavan (§7.6); C15 v3 §5(ii) tek kabul yolu ve `transport_mut()` baypasının kaldırılması devralınır (B24); M1 aynen; **M2 kaldırılır (rev5.1): akış-1 kredisi ve katman 0 varken M2 yalnız akış-1 kredi beklemesini bütün sınıflara yayar (§10)** · (b) A1 şimdi kodlanır. Öneri (a).

**DUR-06-15 — Sinyalleşmenin sınıfı.** **(a) BE, kalıcı akış 3** · (b) TEXT akışını paylaşır · (c) CTRL. Öneri (a).

**DUR-06-12 — Sesli mesaj PoC'de kod mu.** Davranış S5 ile karara bağlı. **(a) PoC'de yalnız tasarım** · (b) kod. Öneri (a).

**DUR-06-21 — Toplu DUR-8 yöntemi (~190 test).** **(b) sınıf başına temsilî 2×2 (eski API ile yazılabilen davranış varyantıyla) + her D8 sınıfı için ≥ 1 hedefli mutant kırmızı + tek akışlı yapılandırmada davranış eşdeğerliği.** Öneri (b).

**DUR-06-22 — INT-02/INT-03 kapı metni** (A.25): "`capacity_defect = 0` (akış 0/1 dahil, yalnız geçerli krediye uyan güvenilir veri); ağ kaybı / protokol ihlali / medya expiry / kontrol baytları ayrı sütun; PTT hükmü B11 ölçüleriyle".

**DUR-06-25 — netem / T-hücre ölçüm kolu** (A.26, B20). Bench ürün kolu bugün örtük kipte koşar (`0c22a07d:crates/bench/src/kama_product.rs:1`, :36-41). (a) **bench'e S1-kipi ürün kolu (`kama-product-s1`, manifest `kip` alanı); B20 ön koşulu** · (b) yalnız örtük kipin FLOW_BATCH'e geçmesi (çerçeve ve tek akış FEC ölçülür; kredi, çoklu akış, BLOCKED ölçülmez) + A.26 beyanı · (c) yalnız beyan. Öneri (a); (a) Aşama 7'ye yetişmezse (b) + beyan.

**DUR-06-26 = DUR-05f-14 — FIN sonrası tüketici tutması × 05f A saati** (§16 YD-3, YD-7; A.8, A.13, A.23, A.36; B29). İki notta aynı ölçüt ve aynı öneri (05f `40c59dfb` §7, §13).
- **Girdi (CEVAP 16 v2 DUR-05f-8): D_tük = 0 REDDEDİLDİ.** INT-02'nin `ek-tuketici-durak` kolu (30 s tüketici durması, DUR-05c-3) kapsam içindeki ürün senaryosudur. Sahibin koşulu: ya tutma zarfa girer ya da "tutuluyor" sinyali A saatini durdurur; iki not tek yaşam döngüsünde uzlaşmadan saat kodlanmaz.
- **Durum:** FIN dahil bütün birimler kredi içinde kabul edilip teyit edildikten sonra alıcının tüketicisi akışı tutar ya da durursa FILE_RECEIPT doğmaz. Gönderen Faz A'dadır; A saati tutmayı göremez. Faz G'deki tutma sorun değildir (kredi bloklu → G durur).

Seçenekler:
- **(a)** Tutma zarfa girdi olarak girer: T_A = T_işlem + N_h·D_h + T_ctrl(E); N_h = 1, D_h ≥ 30 s [CAL]. Tel değişmez, alıcıda mekanizma yoktur. Bedel: her aktarımın T_A'sına D_h. D_h'yi aşan tutma zarf dışıdır → yanlış "başarısız" mümkün (DUR-05f-11 beyanı).
- **(b)** Birikimli, tavanlı `HELD(flow_id, held_ms)` (§16 YD-3): T_A = T_işlem + T_ctrl(E) + G_H(E); G_H = T_min + (k_r + 1)·(T_BLK + T_min) + N_o·D_o + L (05f türetimi; 105–506 s [hesap]). D_tut,max ≥ 30 s [CAL] tavanı. Bedel: yeni girdi türü (tel, A.8), istenmemiş FLOW_CREDIT emisyonu yapan alıcı zamanlayıcısı, tutma süresince ≈ 1,3 B/s (31 B/s'de) [hesap].
- **(c)** FIN'den sonra `hold_flow` yasak. Reddedilir: tüketicinin G/Ç durması alıcının elinde değildir; INT-02'nin 30 s'si de `hold_flow` değil tüketici durmasıdır.

**Öneri (a), D_h = 30 s [CAL]** (rev5'te (b) önerilmişti; rev5.1'de geri çekildi). Ölçüt: karşılanacak tutma D ise (a)'nın T_A'ya bedeli D, (b)'ninki G_H'dir.
- D ≤ G_H → (a) daha ucuzdur. INT-02'nin 30 s'si için bütün hücrelerde böyledir; ölü eş de (a)'da daha erken fark edilir.
- D > G_H → (b) daha ucuzdur (ör. D = 600 s).
- rev5'in (b) gerekçesi kendi zarfıyla çelişiyordu: HELD her T_BLK = 30 s'de bir yinelendiği için G_H ≥ (k_r + 1)·T_BLK koşulsuz T_A'ya girer. Bu yüzden (b) her aktarıma (ölü eş dahil) ≥ 30 s ekler ve INT-02'nin 30 s'sini D_tut,max değil G_H karşılar. HELD aralığını kısaltmak da yetmez: L terimi 31 B/s'de tek başına ≈ 73 s'dir; 5 s aralığın bant bedeli ≈ %25'tir [hesap].

(b), ürün D > G_H gerektirirse açılır (ör. INT-03'te ölçülen mobil arka plan durması); o zaman bu satır yeniden açılır. Karar sahibindir; sayılar 05f'de türetilir ve "K4G-05f ONAYLANDI" ile onaylanır.

**DUR-06-28 = DUR-05f-18 — Gönderen iptalinin akışa bağlanması** (05f ile ortak; §4.2 (D), §16 YD-2, YD-4, YD-5, YD-10; A.1, A.2, A.8, A.15, A.18, A.21; B31).
- **Sorun:** Faz G iptalinde FILE_ABORT (akış 1) aktarımın FILE_BEGIN'ini (akış k) geçebilir; FILE_BEGIN RESET ile hiç teslim edilmeyebilir. Bugünkü erken iptal kaydının türetimi (oturumlar arası, `FILE_IDLE_TTL`) oturum içinde geçmez ve 05f `FILE_IDLE_TTL`'i kaldırır.

Seçenekler:
- **(a) Faz G iptali = `reset_flow(k, kod u8)`:** FLOW_RESET uygulama kodunu opak taşır (RFC 9000 RESET_STREAM emsali); alıcıda `FlowEnded { Reset { kod } }`, bant içi ve okunmamış birimleri atarak (YD-10). FILE_ABORT yalnız Faz A'da, oturum değişiminde ve RESET'i teyit edilmeden oturumu değişen Faz G iptalinde kalır. Erken iptal kaydı R0 tabanlı ve zamanlayıcısızdır (YD-5).
- (b) FILE_ABORT akış 1'de kalır + oturumlu anahtar ve sayım tavanlı kayıt. Düşen kayıt hayalet aktarım doğurabilir; neden kaybolur.
- (c) Bağımlılık işaretçisi (`DEP` + `dep_flow u16`; S1 `send`'e parametre).

**Öneri (a).** Gerekçe: iptal sinyali iptal edilen akışın kimliğine bağlanır, böylece "FILE_BEGIN gelecek mi" sorusu Faz G iptalinde doğmaz. YD-10 kuralıyla bu, uygulama düzeyinde de geçerlidir: uygulama FILE_BEGIN'i ya `FlowEnded`'den önce okumuştur (aktarım bağlı → `Aborted(kod)`) ya da hiç okumaz (aktarım hiç başlamamıştır → olay yok).

Durumlar ve sınırlar (rev5.1):
- **Üçüncü durum:** alıcı, RESET gelmeden aktarımı bitirmiş olabilir. G dolumu FIN sunulduktan sonra olursa RESET bayattır, dosya alıcıda kalır ve beyanlı ayrışma doğar (DUR-05f-11). Kullanıcı iptali bugünkü sınırda kalır (FILE_END kabulünden önce; `file_transfer.rs:831`, :1539-1541), dolayısıyla kullanıcı iptalinde bu durum doğmaz.
- **Alıcının sonlandırdığı akış:** gönderende `reset_flow_local(k)`; telde FLOW_RESET gitmez. Böylece W_TCTRL0 ≥ `bk_flow_limit0` sınırı korunur.
- **RESET teyidinden önce oturum değişimi:** yeni oturumda FILE_ABORT(kod) gider; neden korunur (AMP :92'nin karşılığı).
- **Bilinmeyen kod:** `Aborted { reason: Unspecified }` (A.37).

Bedel: FLOW_RESET +1 B; S1 `reset_flow(flow, code)`, `reset_flow_local`, `FlowEnded { Reset { code } }` (A.1, A.2; DUR-06-7 ile); AMP §3 FILE_ABORT kullanım metni (A.18); 05f §8.3 G dolumunda FILE_ABORT adımı kalkar. Yeni tel kaydı yok: FLOW_RESET zaten K4G-06'nın yeni birimidir.

### Öncelik 4 — Aşama 7

**DUR-06-24 — PTT eşik dosyasının onayı.** Yöntem sahibin 1. notudur (B11); DUR olan, kalibrasyondan sonra `PTT-ESIK-v1`'in kabul koşusundan önce sahip onayıdır.

**DUR'dan çıkarılanlar:** rev1 DUR-06-4, -8 (→ 06-5 (iv)), -9 (→ 06-5 (iii)), -16 ([CAL]: ρ_C 0,1 · Q_BE:Q_BK 3:1 · W0_BE 32/32 KiB · W0_BK 64/64 KiB · `bk_flow_limit0` 4 · `max_rel0` 4 · W_TCTRL0 4 · W_CTRL0 16 · ρ_V,max 0,7 · T_min 1 s · W_LIVE, D_VO, T_block, N ölçülür).

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
| §1.2 DUR-38 ile ortak seçim/rezerv; geçemeyen mekanizma tutulmaz | §5 (rezerv hiyerarşisi, akışsız emisyonlar) | A.17, A.20 | B17, B14, B10 (e) | C.E | taban 05b rev6 `37b78aad`; DUR-06-2 (i) |
| §1.3 oynatılabilir oran, başlangıç, uçtan uca, yük altında, geç oynatılmaz, sebep sayaçları | §10 | A.24, A.32 | B11 | C.I | gecikme bileşeni sayaçları eklendi |
| §1.3 eşikler ürün hedefi + ölçüm (+ sahibin notu 1) | — | — | B11 kuralı | — | K8 hedefleri girdisi; DUR-06-24 |
| §1.3 deadline göndericide; bütün sahiplik zinciri; geç ACK güvenli; iki ilke | §8.3, §10 | A.4, A.16, A.17 | B12 | — | tek sonlandırma çıktısı |
| §1.3 S5: 300 B/s başlangıç; paket boyu ses kabulünün parçası | §10, §11 | — | B11 (birim boyu profilde), B19 (b) | — | DUR-06-10 |
| §1.3 sesli mesaj ayrı, görünür; maliyet ayrı | §10 | — | B19 (b) (otomatik geçiş yok) | — | DUR-06-12 |
| §1.4 açılış: ilk kredi + ilk kontrol | §3, §7.3 | A.5 | B13 (a) (+ erken birim) | — | |
| §1.4 akış sonu/iptal: kullanılmamış kredi, gelmemiş veri, serbest kalma | §7.6 | A.7 (iki istisna), A.16 | B13 (b) | — | FIN yuvası; RESET akış 0'da, transport tüketir |
| §1.4 oturum değişimi sonrası eski kredi/DATA/ACK uygulanmaz | §3, §7.5 | — | B4, B13 (c) | — | bağlam-önce dağıtım mevcut |
| §1.4 medya expiry kredi ve sıra muhasebesi | §3 (REL + gönderen kapısı), §7.1, §7.6 | A.4 | B13 (d) | — | DUR-06-1, DUR-06-6 (DUR-06-18 (a)'ya koşullu) |
| §1.4 aç-kapa kayıt/bellek sınırı | §3 (örtük açma, `max_seen` + açık küme), §7.2, §7.6 | A.13, A.14 | B13 (e), B23 (b) | — | bellek sonucu §7.2 / DUR-06-3 |
| §1.4 yeni OPEN'da gelecekteki CLOSE yükümlülüğüne yer | §7.6 H_C_tavan (şerit başına) | A.21, A.22 | B13 (f) | — | FILE_RECEIPT reddedilen FILE_BEGIN dahil; `bk_flow_limit` alındı kabulüne bağlı |
| §1.4 kullanılmamış taahhüt dahil dosya yükü rezervi tüketemez | §7.2 | A.13 | B13 (g), B10 (d) | — | |
| §1.4 medya kredisi DUR; QUIC stream/datagram karıştırılmaz | §7.1 | — | — | — | DUR-06-1 |
| §1.4 test ayrımı; dört sebep ayrı; kapı yalnız kapasite kusuru (+ sahibin notu 2) | §7.7 | A.25 | B7 (beş alt koşu), sayaç sözlüğü | C.L | akış 0/1 dahil |
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
| ccr §35.6/§35.14 + KM-SEAM-TC-001 KARAR-1 | §3, Karar özeti 1 | A.8, A.9, A.27 | B1 (D), B28 | C.B, C.D | **tarihli KARAR-1/ccr eki gerekir** (DUR-06-5); örtük kip de FLOW_BATCH |
| PoC-KAPSAM-001 §2 PTT ve "bekliyor" (:26) / S5 "Sözleşmeler" (:34) / K6 (:54) / K8 (:62) / EK-2 / EK-3 | Karar özeti 5, §10 | A.18, A.19, A.24 | B11 | C.I, C.K | tek EK-4 (DUR-06-18); Live DUR'ları buna koşullu |
| Kredi canlılığı (C15 v3 §9.3 (2) "kayıp sonrası yeniden duyurulur") | §7.4, §7.5 | A.13 | B3 (a)–(g) | — | BLOCKED iki boyutlu, sayım girdileri, (iii) tetiği |
| Kalite kapıları (ADR-18 netem, T-hücre, §4.4) | §11 | A.26 | B20 | C.L | S1-kipi bench kolu (DUR-06-25) |
| C16 v2 §4 kontrol-alındısı karşı örneği; önce AMP semantiği; körlemesine sırasızlık yok | §3 (`Independent`), §4.1, §4.2 | A.1, A.7, A.13, A.18, A.21 | **B30** | C.M | DUR-06-27; rev5'te yeni |
| C16 v2 §4 DUR-06-20: iki taban, karar G'de | §14 | — | B1-taban, B2, B30 (taban H ve G) | — | taşıma yüzeyi ve maliyeti §14 |
| C16 v2 §3 DUR-05f-8 (D_tük = 0 reddi) + §5 m.4 tek yaşam döngüsü | §16 (YD-1…YD-9) | A.2, A.8, A.13, A.21, A.23, A.36 | B29, B31 | C.M | DUR-06-26, DUR-06-28; 05f rev3 ve 05d A1 §16'ya atıf verir |
| INCELEMECI-8: deney FEC uyumu, BK açlıksızlığı, PTT kalitesi ve ürünün bütünü için kabul kanıtı değil | §14 hüküm kapsamı | — | — | — | beyan |

## Ek F — İnceleme izleme tablosu (`k4g06/inceleme-wf_4a961b72.json`, rev1 `2656953c`)

Mercekler: **E** = EK-1 kapsama · **T** = teknik doğruluk · **W** = tel/güvenlik/donma · **F** = uygulanabilirlik. Önem: B = blocker, M = major, m = minor. 73 bulgu: 70 işlendi, 3 kısmen reddedildi (gerekçe satırda).

| Bulgu | Önem | Kısa başlık | Karar | Notta yeri |
|---|---|---|---|---|
| E1 | B | FLOW_DATA × ccr §35.6/KARAR-1/batch.rs | işlendi: W2 geri çekildi, öneri B — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Ac1)** | Karar özeti 1, §3, A.8, A.9, DUR-06-5 |
| E2 | B | PoC-KAPSAM-001 PTT hükmü sessizce aşılıyor | işlendi: Ek A satırı + ayrı DUR; K8 → B11/DUR-06-24 | Karar özeti 4, §10, A.18, A.19, DUR-06-18, B11 |
| E3 | M | noise-wire §2.6/§3/§2.57, schema eksik | işlendi; A.6 daraltıldı | A.6, A.10, A.11, DUR-06-5 (ii) |
| E4 | M | T-hücre, netem 7 profil, §4.4 yok | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Ac4)** | A.26, B20, C.L |
| E5 | M | `early_unit` (K6) sahipsiz | işlendi | A.5, DUR-06-7, B13 (a) |
| E6 | M | UniFFI/S5 PttBundle, pttPush, AK-001, README-HASAN, node.rs | işlendi | §10, A.24, DUR-06-23 |
| E7 | M | Live kredi/sıra muhasebesi çelişkili | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc8)** | §7.1, §7.6, DUR-06-1, B13 (d) |
| E8 | M | Akış 0 akış kontrolü ve taşma sayacı | işlendi: W_CTRL | §4, §7.7, B7 (5) |
| E9 | M | FLOW_CREDIT tavanı ACK yolunda uygulanamıyor | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc3)** | §7.4, §7.8, DUR-06-3 tablo |
| E10 | M | FILE_RECEIPT/FILE_ABORT/RECEIPT yeri; H_C_tavan yok | işlendi: H_C_tavan = 12 — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc11)** | §7.6, A.22, B13 (f) |
| E11 | M | Pompa eş başına → dosya dosyayı kilitler | işlendi | §6, §10, A.21, B23 |
| E12 | M | Akışsız emisyonlar, K1 (a), hak kapsamı | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc14)** | §5 |
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
| T3 | M | Kredi canlılığı: kalıcı durma | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc1)** | §7.4, §7.5, B3 (b)(c) |
| T4 | M | Kapalı akış izleme sınırsız; geç kopya kuralı yok | işlendi: kalıcı/dinamik kimlik uzayı, `late_after_close` — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc5)** | §3, §7.6, §9, B13 (e) |
| T5 | M | Live sonlanma, MAX_FLOWS_VO, dedup, düşür+ACK | işlendi: VO kalıcı akış; dedup penceresi; yalnız `DeadlineBounded` düşer — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc7)** | §3, §7.6, §7.7, DUR-06-6 |
| T6 | M | Akış 0 kredisiz; sıra başı yeri kayboluyor | işlendi | §4, §7.7, §9, B10 (d), B7 (5) |
| T7 | M | Deadline/iptal × `take_confirmed_units` ve donmuş cümleler | işlendi: tek sonlandırma çıktısı — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Ac8)** | D3, §8.3, A.16, B12 |
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
| W9 | M | KARAR-1 çelişkisi; DUR-06-8 gerekçesi | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Ac1)** | = E1; DUR-06-5 (iv) |
| W10 | M | noise-wire/şema hizası, codepoint, tek kaynak, `control_id` | işlendi: 0x1D+ K-WIRE-CTRL-1; tek kaynak schema | A.10, A.11, §11 |
| W11 | M | Karma sürüm davranışı tanımsız | işlendi | §11, DUR-06-13, A.34, B25 |
| W12 | M | Live dedup yok | işlendi — **rev2 doğrulamasında kısmen; rev3'te tamamlandı (Bc10)** | §3, DUR-06-6 |
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

### rev2 doğrulama turu (`k4g06/rev2-dogrulama-wf_86a0d33e.json`, rev2 `9cda1fab`)

Kapsam A (blocker'lar, Ek A, tel) 14 kontrol + 13 yeni; Kapsam B (protokol canlılığı) 17 kontrol + 13 yeni. Doğrulanan 20 kontrol (Ac2, Ac3, Ac5–Ac7, Ac9–Ac14, Bc2, Bc4, Bc6, Bc9, Bc12, Bc13, Bc15–Bc17) aynen kaldı; aşağıda tutmayan 11 kontrol ve 26 yeni bulgu. Yukarıdaki rev1 tablosunda bu turun "kısmen" bulduğu satırlar (E1, W9, E4, E7, E9, E10, E12, T3, T4, T5, T7, W12) düzeltildi. Bu turda reddedilen bulgu yok; üç bulguda önerilen yol yerine eşdeğer başka yol seçildi (satırda).

| Bulgu | Önem | Kısa başlık | Karar (rev3) | Notta yeri |
|---|---|---|---|---|
| Ac1 | — | E1/W9 KARAR-1 "çelişmez" iddiası tutmuyor | işlendi: "tarihli KARAR-1/ccr eki gerekir" açıkça; örtük kip de FLOW_BATCH; boş batch etiketi; `submsg_count ∈ {0,1}` | Karar özeti 1, §3, A.8, A.9, DUR-06-5, B28 |
| Ac4 | — | E4 kapılar örtük kipte, FLOW_BATCH ölçülmüyor | işlendi | §11, A.26, DUR-06-25, B20 |
| Ac8 | — | T7 donmuş cümle listesi eksik, §0 çelişkisi | işlendi | §8.3, A.16 (1)–(8) |
| Bc1 | — | T3 BLOCKED'e yanıt tetiği yok, BLOCKED tek boyutlu | işlendi | §7.4 (iii), §7.5, B3 (d)(e) |
| Bc3 | — | E9 FLOW_CREDIT sınırı, srtt yok, çok akışlı maliyet | işlendi | §7.4, §7.8, DUR-06-3 tablo |
| Bc5 | — | T4 örtük açma ve `bk_flow_limit` ilerlemesi yok | işlendi | §3, §7.6, B13 (e), B23 (b) |
| Bc7 | — | T5 "yalnız DeadlineBounded düşer" uygulanamıyor | işlendi: REL bayrağı | §3, §7.1, §7.7, DUR-06-1 |
| Bc8 | — | E7 `max_rel` taşıyıcı/ilerleme/BLOCKED yok | işlendi: sayım girdisi + ilerleme kuralı | §7.1, §7.4, §7.5 |
| Bc10 | — | W12 W_LIVE REL'i kesintide korumuyor | işlendi: gönderen kapısı (önerilen "tam küme" yerine; alternatif DUR-06-6 (c)) | §3, DUR-06-6, B13 (d) |
| Bc11 | — | E10 H_C_tavan türetimi yanlış | işlendi | §7.6, A.21, B13 (f) |
| Bc14 | — | E12 K1 (a) ile katman önceliği çelişkisi | işlendi: rezerv hiyerarşisi (i) | §5, DUR-06-2, B10 (e) |
| An1 | M | T_block × `block_id` aritmetiği; yeniden kullanım | işlendi: B-iii (blok sınırı onarımda; önerilen (i)/(ii) yerine (v-c), üçü bayt bedeliyle DUR-06-5 (v)'te) | §3, §8.1, DUR-06-5 (v), B15 |
| An2 | M | TCTRL FEC kurtarımında görünmez | işlendi: transport kontrolü ayrı kalıcı akış 0 (bayrak kalktı); kurtarılan sembol `chunk_offset := flow_id` ile akışını tanıtır | §3, §4, A.10, A.11, B13 (b) |
| An3 | M | KARAR-1'in dört boşluğu | işlendi (= Ac1) | §3, A.9, DUR-06-5 (vi)(vii) |
| An4 | M | bench örtük kip; T-hücreleri teli ölçmüyor | işlendi (= Ac4) | §11, A.26, DUR-06-25, B20 |
| An5 | M | A.16 eksik; §0 listesine "süre dolumu" çelişkisi | işlendi (= Ac8) | A.16 |
| An6 | M | `pop_first` ters tahliye (runtime) | işlendi: monoton `block_id` + sınıf başına depo; motor için [olgu] | §2, §3, A.12, A.13, B15 |
| An7 | M | A.19 "K8 korunur" × DUR-06-18 (a) | işlendi | Karar özeti 5, A.19, B11, DUR-06-18 |
| An8 | M | S5 dondurması (PoC-KAPSAM :34, K6 :54) | işlendi | §10, A.19, A.24, DUR-06-23 |
| An9 | m | noise-wire :424/:430/:437; "beşinci kimlik" | işlendi | §3, A.10 |
| An10 | m | ADR-fec-v1-role eksik | işlendi | A.26 |
| An11 | m | Ek C'de bayat Ek A atıfları | işlendi | Ek C |
| An12 | m | H_C_tavan PTT birimleri yanlış akışta | işlendi: şerit başına | §7.6 |
| An13 | m | Live DUR'ları yanlış öncelikte | işlendi: DUR-06-18'e koşullu, öncelik 3 | §13, Ek D |
| Bn1 | M | BLOCKED yeniden ilan kuralı | işlendi (= Bc1); K2 deneyi bu kuralla | §7.4, §7.5, §14 K2, B3 |
| Bn2 | M | `bk_flow_limit`/`max_rel` canlılığı | işlendi | §7.1, §7.4, §7.5, B3 (f), B13 (f) |
| Bn3 | M | VO'da Reliable/DeadlineBounded telde ayrılamıyor | işlendi: REL bayrağı + gönderen kapısı | §3, DUR-06-6, B13 (d) |
| Bn4 | M | TCTRL FEC'te yok | işlendi (= An2) | §3 |
| Bn5 | M | H_C_tavan iki hata; alındı kuyruğu düşürüyor | işlendi (= Bc11) | §7.6, A.21 |
| Bn6 | M | K1 (a) × katman 0/1; rezerv beklemesi | işlendi (= Bc14) | §5, B10 (e), B11 |
| Bn7 | M | örtük açma, `bk_flow_limit`, kalıcı akışa close/reset | işlendi (= Bc5) | §3, §7.6 |
| Bn8 | m | RESET'te ACK'li tampon istisnası; akış 0 tüketici bağı | işlendi: A.7 istisna (ii); akış 0'ı transport tüketir | §7.6, A.7 |
| Bn9 | m | motor öncesi süre dolumu tek çıktıya bağlanamıyor | işlendi: runtime'ın kendi iptal çıktısı | D3, §8.3, B12 |
| Bn10 | m | alıcıda srtt yok; iyi niyetli yarış "ret" | işlendi | §7.7, §7.8, B26 |
| Bn11 | m | katman 0 içi sıra; çok akışlı BLOCKED maliyeti | işlendi | §4, §7.5, DUR-06-3, B3 (g) |
| Bn12 | m | havuz/teslim kuyruğu sayısal sonucu; FIN yuvası | işlendi | §7.2, DUR-06-3 |
| Bn13 | m | B2 yeni-kod kolu rev5'te; T_BE^rec tanımsız | işlendi | §14, B2 |

### 05f rev2 hizalaması (`k4g/05f-dosya-ttl` @`3e0ec8fa`, §4 ortak bölüm ve §12 çelişkiler)

| 05f §12 | Çelişki | Karar (rev4) | Notta yeri |
|---|---|---|---|
| 1 | §10 "dosya ilerlemesi = akışın teyit tabanı" ↔ C15 v3 §6 (1) | işlendi: ilerleme = akışta daha önce teyit edilmemiş birimin ilk teyidi; taban yalnız faz sınırı | §10 |
| 2 | A.23 "yeni DUR yok" ↔ 05f'nin yeni sözleşme, sayı ve spec yaması | işlendi: A.23 DUR-05f-6…-12'ye bağlandı; erişimci (A.36) ve terminal yüzey (A.37) ortak satır | A.23, A.36, A.37 |
| 3 | B22 10 KiB ↔ C14 §6 (31 B/s'de 150 KiB) | işlendi: iki kol (150 KiB kayıpsız + 10 KiB kayıplı hücreler); "zarf içinde vazgeçme = 0" kapı | B22 |
| 4 | §7.6 "tutulan akışın kapanışı tüketene dek bekler" ↔ D_tük = 0 | işlendi: FIN sonrası tutma açıkça yazıldı; öneri "tutuluyor" girdisi (A saati durur); karar DUR-06-26 (DUR-05f-8 ile) | §7.6, §13, DUR-06-26, B29 |
| 5 | uyumlu satırlar (H_C_tavan, erken iptal ↔ `max_seen`, `reset_flow`) | değişiklik yok | — |

### CEVAP 16 v2 §4 + bağımsız inceleme (INCELEMECI-8, K4G-06 bölümü) — rev4 `1f0e032c` → rev5

| Kalem | Kaynak | Karar (rev5) | Notta yeri |
|---|---|---|---|
| DUR-06-5 (B + B-iii) yön GO, yalnız deney; KARAR-1/ccr eki gerekli kabul, şimdi yazılmaz; hub zarfı değişmez | C16 v2 §4 | işlendi | Karar özeti, §13, Ek D |
| DUR-06-2 (b2) + (i) yön GO; DUR-38 ile ortak seçim/rezerv | C16 v2 §4 | işlendi | Karar özeti, §13, Ek D, A.17 |
| DUR-06-20 (a) KOŞULLU: iki taban, karar G'de | C16 v2 §4; INCELEMECI-8 | işlendi: koşu matrisi, ön-kayıt T_BE^rec(H)/(G), H'de yazıp G'ye taşıma ve maliyeti | §14, Ek D, B2 |
| Kontrol-alındısı karşı örneği zorunlu | C16 v2 §4; INCELEMECI-8 | işlendi: önce AMP denetimi (§4.1), sonra seçenekler ve öneri (§4.2); B30 deneyde | §3, §4.1, §4.2, DUR-06-27, B30, C.M |
| "Bütün kontrol kayıtları körlemesine sırasız yapılmaz" | C16 v2 §4 | işlendi: sırasızlık yalnız akış 0/1; TEXT, BK, VO aynen; şeritler arası bağımlılık (FILE_ABORT ↔ FILE_BEGIN) ayrıca korundu | §4.1 sonucu, §4.2 (D), DUR-06-28, B31 |
| D_tük = 0 reddi: ya D_tük ≥ 30 s zarfa ya da "tutuluyor" A'yı durdurur | C16 v2 §3 DUR-05f-8 | işlendi: (b) tavanlı "tutuluyor" önerildi (D_tut,max ≥ 30 s); geri dönüş (a) | §16 YD-3, DUR-06-26, B29 |
| Tek yaşam döngüsü (kredi, kontrol yükümlülükleri, iptal, FIN, timeout) | C16 v2 §5 m.4; INCELEMECI-8 "Önerilen uygulama sırası" m.4 | işlendi: kalıcı başlık §16; 05f rev3'e devredilenler YD-9 | §16 |
| Deney kapsam sınırı | INCELEMECI-8 | işlendi | §14 hüküm kapsamı |
| A.18'de "RECEIPT/FILE_RECEIPT/FILE_ABORT akış 0" yazımı (rev3 akış ayrımıyla çelişiyordu) | rev5 öz-denetim | düzeltildi: akış 1 | A.18 |
| Ek A'da A.35 ile A.36 arasındaki boş satır tabloyu bölüyordu | rev5 öz-denetim | düzeltildi (Ek A hash'i zaten değişiyor) | Ek A |
