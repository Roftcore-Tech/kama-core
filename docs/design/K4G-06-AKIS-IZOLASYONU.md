# K4G-06 — Akış izolasyonu ve teslim sözleşmesi — tasarım notu rev2 (ADIM 1: sözleşme/arayüz taslağı + ayırt edici test planı)

## 0. Künye

- **Kart / dal / durum:** K4G-06 · `k4g/06-tasarim` (taban main `1b1425d2`) · **rev2 TASLAK**. Kod yok, ölçüm yok; deney dalı (`k4g/06-deney`) açılmadı.
- **rev1 → rev2:** rev1 = `2656953c`. Dört mercekli incelemenin (`k4g06/inceleme-wf_4a961b72.json`; 2 blocker, 42 major, 29 minor) bütün bulguları işlendi ya da gerekçeyle reddedildi; satır satır izleme Ek F'de. En büyük değişiklik: rev1'in W2 tel önerisi geri çekildi, KARAR-1 ile hizalı B önerisi geldi (DUR-06-5).
- **Otorite sırası:** CEVAP 15 EK-1 > CEVAP 15 v3 > 14 > …; donmuş belgeler kendi değişiklik kuralıyla (PoC-KAPSAM-001 yalnız sahibin tarihli ekiyle; ccr/noise-wire/transport-tdd yalnız sahip kararıyla). Sahibin iki ek notu bağlayıcı. İnceleme kayıtları kanıt girdisidir.
- **Atıf biçimi:** `SHA:dosya:satır`. Kısa dosya adları: `runtime.rs`/`transport.rs`/`wire.rs`/`batch.rs` = `crates/client-core/src/`; `sender.rs`/`arq.rs` = `crates/adaptive-transport/src/`; `s1.rs` = `crates/kama-contracts/src/`; `ptt.rs`/`file_transfer.rs`/`messaging.rs` = `crates/kama-app/src/`; `node.rs` = `crates/kama-node/src/`. Belgeler tam yolla.
- **SHA'lar (origin'de, 2026-10-25):** `0c22a07d` = `k4g/05c-kod` (05b rev5 `35a02fcf` + 05c) · `37b78aad` = `k4g/05b-retx-fix` ucu, **05b rev6** (B′ karşılık hakkı, K3 ARQ payı rezervi, DUR-40 yaş tavanlı HOL; Ek A A.4 rev6, Ek B.6 (g)(h)) — `0c22a07d` rev6'yı İÇERMEZ · `1b1425d2` = main · `cf270a6c` = 05d · `e5514e7a` = 05f rev1 · `0b216f07` = 05g origin ucu (yerel `c9c2ea22` YEREL, dayanak değil).
- **Etiketler:** [olgu] koddan · [hesap] aritmetik, ölçülmedi · [çıkarım] test edilmedi · [türetim] koddan önce yazılan sınır · [CAL] deney başlangıç değeri.

## Karar özeti

**Şimdi istenen onay:** yalnız §14 deney kapsamı ve deneyden ÖNCE gereken iki yön (DUR-06-5, DUR-06-2) ile deney kapsam daraltması (DUR-06-20). Ürün kodu, merge ya da donmuş metin değişikliği istenmiyor.

1. **Birim çerçevesi KARAR-1 ile hizalanır (DUR-06-5, öneri B; rev1'in W2'si geri çekildi).** Akış koordinatı ccr §35.14 `BatchPayload`'ın alt-mesaj başlığına girer: `[tip][submsg_count][flags · flow_id u16 · flow_seq u32][len_mode][AMP kaydı]` = 10 B (kayıt ≤ 239 B) / 12 B (bugün 11 B) [hesap]. FEC kimliği oturumda tekil kalır: `block_id` = `flow_id ‖ akış içi blok no` (u32 alanı aynı), `esi` = `flow_seq mod K`. Kurtarılan birimin akışı ve sırası `block_id`/`esi`'den, akış sonu (FIN) FEC korumalı sembol önekinden okunur. Bedel: ccr §35.14 alt-mesaj başlığı + KARAR-1'in tarihli sahip eki + noise-wire §2.6 ID-SCOPE yaması (Ek A A.9–A.11).
2. **S7 (b): motor içinde tek, saf seçici; etiket eklemeli API ile** (`Event::AppDataTagged(OpaqueUnit, ServiceTag)`; etiketsiz birim = tek akış = 05b rev6 davranışı; ≈149 `OpaqueUnit` literal'i değişmez). Düzey 2 = 05b rev6 K-0…K-5 akış içinde; düzey 1 = CTRL ve VO kovaları + BE/BK DRR. S7(a)'nın reddi yeniden tasarlanan karşı örnek B2'ye bağlıdır: yeni TEXT'in KENDİ kurtarma işi, N_u ≥ 8 eski dosya birimi, tabanlar `0c22a07d` ve `37b78aad` ayrı, ön-kayıtlı.
3. **Kredi:** akış başına mutlak, artan sınır; akış 0'ın (kontrol şeridi) veri kredisinden bağımsız kendi kredisi; BLOCKED, etkin sınır artana dek tekrarlanır; ilan alıcının ACK yolunda ve tetik kuralıyla sınırlıdır; kontrol yükümlülüklerinin yeri akış açılırken ayrılır. Kabul kapısı yalnız `capacity_defect = 0`'a bağlıdır (akış 0 dahil; sahibin notu 2).
4. **PTT:** sahibin C15 v3 §9.3 / S5 yönü (canlı ses, demet süreli) PoC-KAPSAM-001'in "Reliable klip, geç ses düşürülmez" hükmünü değiştirir; bu yalnız sahibin tarihli PoC-KAPSAM eki ile olur (DUR-06-18). Deney PTT'ye dokunmaz; karar uygulamadan önce gerekir. K8 demo hedefleri (bas→ilk ses ≤ 1,5 s / ≤ 3 s; sıfır bozuk hece) PTT eşiklerinin ürün hedefi girdisidir.
5. **Göndericide süre dolumu ve RESET payload için ikinci silme yolu açmaz:** 05b'nin teyit çıktısı "sonlandırma çıktısı"na (teyit | iptal; birim başına oturumda en çok bir kez) genişler. Bu AMENDMENT-12'nin donmuş cümlelerini değiştirir (Ek A A.16); uygulamadan önce sahip kararı gerekir.

**Deneyden ÖNCE gerekenler:** DUR-06-5 ve DUR-06-2 yön onayı · DUR-06-20 (deney kodunda FEC kapalı; B2'nin FEC kolu yalnız tabanda ölçülür) · §14 kapsamı (≈ 6–7,5 iş günü; tahmin, takvim sözü değil).

## 1. Amaç ve değişmeyenler

**Amaç:** tek güvenli oturum altında bağımsız mantıksal akışlar — akış başına sıra, WMM-örnekli sınıf önceliği (PTT → AC_VO, TEXT → AC_BE, FILE → AC_BK; AC_VI yok), alıcının önceden ilan ettiği kredi, ortak ve adil gönderim bütçesi. Dosyanın eksik birimi ya da durmuş tüketicisi TEXT/PTT'yi bekletmez; olağan kapasite durması ACK kesilerek kayıp gibi gösterilmez (HER BİT DÜŞMAN). Bu bir teslim sözleşmesi revizyonudur (C15 v3 §9.1).

**Değişmeyenler:**
- **D1 Hub zarfı:** 22 B legacy / 26 B extended dış zarf ve hub-server codec'i değişmez [`1b1425d2:crates/kama-noise-wire/src/envelope.rs:80-83`]. Akış alanları AEAD altındaki iç çerçevededir ve **yalnız `CtxRoute::Session` kolunda ayrıştırılır**; `HubChannel` (PoC'de AEAD'siz no-op), `Mismatch` ve `NotReady` kollarında yüke dokunulmadan düşer [`0c22a07d:runtime.rs:1510-1516`, :2320-2335].
- **D2 Noise:** sayaç/nonce akış başına sıfırlanmaz; yön başına tek PN uzayı; transport ARQ kimliği PN'dir [`0c22a07d:runtime.rs:2028-2033`].
- **D3 Doğruluk düzeltmeleri korunur:** payload yalnız motorun **sonlandırma çıktısında** düşer (bugün: teyit çıktısı, `0c22a07d:runtime.rs:2019-2024`; genişleme §8.3, Ek A A.16); 05c INV-ACK-DELIVER-1 akış başına; 05g sahte `Ready` kapanışı; 05d "canlı RECEIPT silinmez".
- **D4 INV-ENGINE-SOC:** motor payload'u ve kama-app tiplerini görmez; yalnız generic hizmet etiketi görür.
- **D5 INV-RATE-1:** tek pacer; akış başına pacer yok; kayıp hızı düşürmez.
- **D6 INV-FEC-1** aynen; **D7** ADR-05 (fixed-width, varint yok) ve ADR-17 (deterministik seçim).
- **Kapsam dışı (gerekçeli):** Wi-Fi/DSCP işaretleme; hub rölesi (direct-mode, INT-02) — ama hub kuyruğu gereksinimi S4'e iletilir (§6, Ek A A.31); media plane.

## 2. Bugün neden birbirlerini bekliyorlar [olgu → çıkarım]

- **Alıcı:** oturum başına tek `Reorder` [`0c22a07d:transport.rs:370-419`]; düğüm geneli tek `inbox` (1024, :118, :449) ve tek teslim kanalı (`delivery_queue` 1024, `runtime.rs:3053`, :271); kabul penceresi oturum geneli `max_inflight_units` [`runtime.rs:3447`, :3480-3482].
- **Gönderen:** tek offer FIFO'su (1024, `runtime.rs:1705`, :270); oturum geneli mesafe kapısı [:2511, :2575-2581, :2608].
- **Motor (sınıf körü):** `OpaqueUnit{id, size}` [`0c22a07d:sender.rs:75-80`]; yeni DATA FIFO [:2492, :7225-7247]; ARQ sırası kayıp ilanı anı [:3846-3849, `arq.rs:124-127`]; kopya başı: rev5'te en küçük birim kimliği [`0c22a07d:sender.rs:7103-7110`], **rev6'da RTO_MAX yaş tavanına dek en küçük kimlik, vadesini RTO_MAX aşmış kayıt varsa son hizmeti en eski birim** (DUR-40) [`37b78aad:sender.rs:7243-7265`]; parite FIFO [`0c22a07d:sender.rs:1787`]; sınıflar arası DATA-önce + karşılık hakları (rev6 K-2/K-3) [`37b78aad:sender.rs:4684-4717`, :1405-1440, :6333-6350].
- **FEC blokları trafikleri bağlar:** blok = `unit_id / K` [`0c22a07d:runtime.rs:2010-2014`]; kurtarılan birim = `block·K + esi` [:2379-2382].
- **Süre yalnız sunumda:** `DeadlineBounded` yalnız `send` anında denetlenir [`0c22a07d:transport.rs:1297-1305`]; sonra yalnız FEC politika girdisi [`runtime.rs:2612-2613`]; ARQ süresi dolanı düşürmez [`arq.rs:92-94`].
- **kama-app:** PTT'nin üç kaydı da `Reliable` (PoC-KAPSAM-001 §2 hükmü, `0c22a07d:ptt.rs:18-24`); dosya pompası eş başına yalnız en eski aktarıma hizmet eder [`file_transfer.rs:862-881`]; tek okuyucu olay zinciri [`messaging.rs:652-660`; `node.rs:465-485`].
- **Sonuç [çıkarım]:** oturum sırasındaki kayıp dosya birimi alıcıda TEXT/PTT'yi tutar; gönderende yeni TEXT FIFO'larda ve motor kuyruklarında eski dosya işinin arkasındadır (05d: `cf270a6c:docs/design/K4G-05d-RECEIPT.md:88`); kapasite ilan edilmez, dolunca ACK kesilir.

## 3. Akış modeli

**Akış** = oturum içinde tek yönlü mantıksal kanal; açan (gönderen) taraf numaralar.

**Kimlik uzayı** (`flow_id` u16, yön başına; rev1'in tek monoton uzayı geri çekildi):
- **Kalıcı akışlar** (oturum boyu, örtük açık, ayrılmış kimlik): `0` = kontrol şeridi (CTRL), `1` = TEXT (BE), `2` = SIGNALING (BE), `3` = VO (canlı PTT, `Live`; konuşmalar AMP `stream_id` ile bu akışta çoğullanır).
- **Dinamik akışlar** (`4…65535`): yalnız BK (dosya aktarımı başına bir akış). Gönderen kimliği ilk biriminin kabul edildiği anda ayırır (boşluk yok). Alıcı, sayım sınırı olarak mutlak `bk_flow_limit` ilan eder (QUIC MAX_STREAMS anlamı); "çözümlenmiş alt sınır" `bk_resolved_below` + açık küme (≤ MAX_FLOWS_BK) tutar. Kapalı ya da çözümlenmiş akışın geç birimi ACK'lenip atılır (`late_after_close`), kredi ihlali sayılmaz.
- Kalıcı akışlar watermark'ı tutmaz (rev1'in "RESET teyidi hiç ilerlemez" kusuru yapısal olarak kalkar). Tükenme [hesap]: 65 532 dosya aktarımı/oturum; tükenmede yeni aktarım tipli hata alır ve `RenewRequired` istenir (DUR-06-5 (iii)).

**Üç kimlik:** PN (Noise zarfı; ACK, nonce) · birim kimliği (`OpaqueUnit.id`; yalnız gönderen runtime + motor; sahiplik, teyit) · (`flow_id`, `flow_seq`) (iç çerçeve; teslim yeri, akış içi sıra, kabul, dedup, kredi). Gönderen runtime (akış, sıra) ↔ birim kimliği **ters dizinini** tutar (girdi ≤ store; sonlandırmada düşer).

**Birim çerçevesi — öneri B (DUR-06-5):** `FLOW_BATCH = [tip u8] ‖ BatchPayload { submsg_count u8 = 1 ; BatchSubMessage { flags u8 · flow_id u16_be · flow_seq u32_be · len_mode u8 [· length u16_be] · payload = AMP kaydı } }` [`1b1425d2:specs/ccr-format-spec.md:3017-3035`, `0c22a07d:batch.rs:1-29`]. 10 B (kayıt ≤ 239 B) / 12 B [hesap]. `flags`: FIN · TCTRL (akış 0'da transport kontrol birimi) · ayrılmış (≠ 0 → ret). Sınıf ve kip `flow_id`'den çıkar. PoC'de `submsg_count = 1` (bir motor birimi = bir AMP kaydı); çoklu alt mesaj ayrı karar.
- **FEC kimliği:** `block_id` = `flow_id ‖ flow_block` (u32; `flow_block = flow_seq div K`), `esi = flow_seq mod K`. Oturum + yön içinde tekil, akış içinde monoton, yeniden kullanılmaz; FEC/RECOVERED çerçeve düzeni değişmez. Kaynak sembol öneki iki uçta türetilir: `message_id := flow_seq`, `actual_payload_len = |AMP kaydı|`, `source_flags`'ta yeni FIN biti → FEC ile kurtarılan son birimin FIN'i görünür.
- **Store-miss (05b bekçisi):** store'da olmayan birim için motor kararı boş batch (`submsg_count = 0`; ccr §35.14 K3: geçerli, PN tüketir, teslim yok) olarak gider; alıcı PN'i ACK'ler, `stale_decisions` sayılır; sahte koordinat üretilmez.
- **Akış bilgisi genel sıralamadan önce görülür:** alıcının ilk kararı (kabul kapısı) alt mesaj başlığından okur; oturum geneli sıra alanı S1 kipinde kalkar.

**Teslim kipleri:** `Ordered` (güvenilir; akış içinde sıralı, boşluk atlanmaz) · `Live` (VO; akış içinde sırasız teslim, `flow_seq` uygulamaya verilir; alıcıda kayan dedup penceresi W_LIVE [CAL] — `Reliable` birimler (PTT_OPEN/CLOSE) "tam bir kez", pencere altı geç birim atılır, `media_late_rx`).

**Oturum sınırı:** akışlar oturuma aittir, yenileme/supersession akış taşımaz (S1 K7 aynen); bağlam-önce dağıtım eski oturumun DATA/ACK/kredisini yeni oturuma ulaştırmaz [`0c22a07d:transport.rs:56-62`].

## 4. Sınıflar ve kontrol

| Trafik | Sınıf | Akış | Kip / politika |
|---|---|---|---|
| Canlı PTT | VO (AC_VO) | 3 | `Live`; demet `DeadlineBounded`, OPEN/CLOSE `Reliable` (DUR-06-18'e bağlı) |
| TEXT | BE (AC_BE) | 1 | `Ordered`, `Reliable` |
| SIGNALING_BLOB | BE | 2 | `Ordered`, `Reliable` (DUR-06-15) |
| Dosya | BK (AC_BK) | 4… (aktarım başına) | `Ordered`, `Reliable` |
| RECEIPT, FILE_RECEIPT, FILE_ABORT; FLOW_RESET (TCTRL) | CTRL | 0 | `Ordered`, `Reliable`, birim boyu tavanlı |
| FLOW_CREDIT | CTRL | — (alıcının ACK yolu) | tetikli, idempotent |
| FLOW_BLOCKED | CTRL | — (`TransmitKind::Control`, izlenmez) | yerini alan, geri çekilmeli tekrar |

WMM adları niyet bildirir; Kama'da garanti §5 seçicisinden ve türetiminden gelir.

**Kontrol katmanı (S6, EK-1 §1.1):** VO'nun altında tek sıra değildir; ayrı katman 0'dır.
- **Tavan:** katman 0'ın BÜTÜN gönderen işi (akış 0'ın DATA/ARQ/kopya işi, BLOCKED tekrarları) kontrol kovasına yazılır: oran ρ_C·r(t), kova b_C ≥ `max_control_unit_bytes` + mühür ek yükü [CAL]. Alıcının ACK yolundaki FLOW_CREDIT için ayrı sınır (§7.8).
- **Koşullu asgari hak:** uygun kontrol işi varken ve kovada token varken katman 0 önce seçilir → backlog'luyken ≥ ρ_C·r hizmet; kova boşsa bekler.
- **Veri kredisinden bağımsızlık:** akış 0'ın kredisi (W_CTRL) veri kredisinden ayrıdır ve FLOW_CREDIT'in akış 0 girdisiyle ilerler; kredi ilanı alıcının ACK yolundan gider ve kredi tüketmez. "Kontrol göndermek için kontrol bekleme" döngüsü yoktur (B10 (c)).
- **Envanter, hizmet hakkı ve toplam maliyet:** Ek D, DUR-06-3 altındaki tablo (31 ve 300 B/s'de saniyede bayt üst sınırı; transport ACK ≠ uygulama RECEIPT).

## 5. İki düzeyli seçici (S7 önerisi; DUR-38 ile ortak)

**Sözleşme [öneri S7(b), DUR-06-2]:** `ServiceTag { tier: u8, flow: u32 (opak), expires_ns: Option<u64> }`, eklemeli olay `Event::AppDataTagged(OpaqueUnit, ServiceTag)`; etiketsiz `Event::AppData` = tek akış, katman 2 → 05b rev6 davranışı birebir (≈149 literal ve mühürlü pinler değişmez, Ek C.F). Seçim motor içinde tek, saf, deterministik bir fonksiyondur. Yeni `TransmitKind::Control` (CsProbe deseni: uçuşta izlenmez, PN tablosuna girmez) yalnız FLOW_BLOCKED içindir.

**Düzey 1 — hangi akış:** katman 0 CTRL (kova) → katman 1 VO (kova: oran **min(VO profil hızı, ρ_V,max·r(t))**, kova b_V; kesilen pay `vo_rate_deficit` sayacına düşer, sürekli açıkta uygulamaya tipli "desteklenmeyen profil" olayı gider) → katman 2 BE/BK bayt kuantumlu DRR: **sınıf ağırlığı akış sayısından bağımsız** (Q_BE : Q_BK = 3 : 1 [CAL]); sınıf içinde akışlar arası DRR (akış başına pay = sınıf payı / n_sınıf).
**Akışsız motor emisyonları:** CS/bootstrap probu ve kuruluş probu katman 0'a (kontrol kovası) yazılır; kuyruk-probu, parite ve kopya ait oldukları birimin/bloğun akışına aittir. 05b K1 (a) aynen: rezerv sahibinin başı dışındaki HER token tüketicisi (prob ve parite dahil) rezervin altına inemez.
**Tek rezerv:** küresel tek rezerv; sahiplik sırası önce düzey 1 (seçilen akış), akış içinde rev6 K-3 (ARQ > DATA > kopya). Haklar (rev6 K-2) **(akış, sınıf)** başına tutulur. K_ARQ_SHARE payı `schedule_transmit` çağrısını değil **akışın hizmet fırsatını** sayar (DUR-38'in kök nedeni çağrı sayan paydı).
**Düzey 2 — akış içinde hangi iş:** 05b rev6 K-0…K-5 (`37b78aad:docs/design/K4G-05b-RETX-FIX.md:545-575`), akış içinde. Tek akışlı yapılandırmada düzey 1 etkisizdir → rev6 T1/T2 regresyon testi olarak kalır (B17).
**PRE_CONVERGE (K-0):** düzey 1 katman sırası uygulanır (CTRL/VO DATA'sı BK DATA'sından önce); düzey 2'de v1 sırası, §3.3-B takvimi; hak ve rezerv yok. Beyanlı istisna; B14 kolu.

**Aç bırakılmama — ispat taslağı [türetim; sonuç "ölçüldü" diye yazılır].** r(t) = pacer hızı; pacer dışı ters yön ACK/kredi trafiği hariç (iki yönlü durumda ayrıca ölçülür, B5). Biriken token üzerinden (05b A.4 (iii) biçimi):
1. τ aralığında katman 0 + 1 + akışsız prob tüketimi ≤ ∫(ρ_C + ρ_V)·r + b_C + b_V + P_pf; **ρ_C + ρ_V,max < 1 her an korunur** (VO kovası r(t)'ye bağlı).
2. Katman 2'de backlog'lu sınıf c bir DRR turunda ≥ Q_c bayt alır; tur ≤ Σ_c (Q_c + S_max) bayt; akış f sınıf c içinde n_c akışla ≥ Q_c / n_c.
3. f'nin iki hizmet fırsatı arası P_f ≤ n_c·[Σ_c (Q_c + S_max) + b_C + b_V + P_pf] / ((1 − ρ_C − ρ_V,max)·r) + τ.
4. **T_birim (akış içi):** rev6 formülü T_birim = max(H_maks, A) + A + N_live·P_c + τ (`37b78aad:…RETX-FIX.md:571`), P_c yerine akışın düzey-2 sınıf hizmet aralığı = P_f × (K-3 sırasının üç baş boyu / Q payı) + K_ARQ_SHARE akış fırsatı. Girdiler B14'te koddan önce yazılır.
5. **Kontrol:** uygun kontrol başı ≤ (S_iletimde + S_ctrl)/r + kova dolum süresi + τ içinde çıkar.
- Örnek [hesap]: r = 300 B/s, VO yok, ρ_C = 0,1, bir BE + bir BK akışı, Q_BE = 3150 B, Q_BK = 1050 B, S_max = 1050 B → tur ≈ 6300 B / 270 B/s ≈ 23,3 s.

## 6. Kuyruk derinliği denetimi (S7) ve S7(a) hükmü

| Katman | Bugün [olgu] | TEXT'in önünde kalabilecek dosya işi [hesap/çıkarım] | K4G-06 |
|---|---|---|---|
| kama-app dosya pompası | eş başına yalnız en eski aktarım (`0c22a07d:file_transfer.rs:862-881`) | ikinci dosya birincinin arkasında | akış başına hizmet (A.21) |
| S1 `send` → offer mpsc | tek FIFO 1024 | 1024 birim; 31 B/s'de ≈ 9,6 sa | akış bütçesi dışında birim kabul edilmez; yerel tampon akış başına, offer FIFO'su dışında |
| runtime store + mesafe kapısı | oturum geneli (`runtime.rs:2575-2581`) | eski dosya birimi yeni alımı durdurabilir | S1 kipinde mesafe kapısı kalkar → akış kredisi; store ≤ min(Σ kredi, yerel tavan) |
| motor `tx_queue` | FIFO | ≤ 4096 DATA | katman/akış başına kuyruk |
| motor `pending_lost` | kayıp ilanı sırası | önce kaybolan dosya birimi önce | düzey 1 akış seçer; akış içi `arq.rs` sırası aynen |
| motor `dup_queue` | rev5 en küçük kimlik; rev6 yaş tavanına dek en küçük kimlik | eski dosya kopyası (yaş tavanı dolmadıkça) önce | akış başına baş; düzey 1 |
| motor `pending_fec` | FIFO; blok oturum geneli | karışık blok paritesi | blok akış başına (§3) |
| pacer / soket / darboğaz | tek kova; iletimdeki paket kesilemez | S/r (31 B/s'de 1050 B ≈ 33,9 s; 300 B/s'de 3,5 s) | değişmez — fiziksel sınır (§11) |
| **hub (INT-03)** | S4 taslağında kuyruk tanımsız (`1b1425d2:docs/seams/KM-SEAM-TH-001-TASLAK.md:63`) | hub sınıf göremez; derin eş FIFO'su önceliği etkisizleştirir | gereksinim: sığ, sınırlı handle kuyruğu → S4'e (A.31) |
| alıcı teslim kanalı | düğüm geneli FIFO 1024 | dolunca ACK'siz ret | kapasite ≥ Σ taahhüt (yapısal yer) |
| alıcı `Reorder` / `inbox` | oturum başına / düğüm geneli | dosya boşluğu ve durmuş tüketici herkesi tutar | (oturum, akış) başına; akış başına hazır kuyruk + `hold_flow` |
| kama-app olay zinciri | tek okuyucu | yavaş dosya işleyici herkesi tutar | dosya tüketicisi akışını tutar; zincir sürer |

**S7(a) hükmü [çıkarım; B2 ile sınanır].** Üst katmanın sığ beslemesi motora önceden kabul edilmiş ARQ/kopya/parite işini sıralamaz: ARQ planı kayıp ilanı sırasıyla, kopya başı (yaş tavanı dolmadıkça) en küçük kimlikle, parite FIFO ile çalışır. Bu, yeni TEXT'in **ilk** emisyonunu bekletmez (DATA-önce, rev6 karşılık hakları) ama TEXT'in **kendi kurtarma işini**, önce ilan edilmiş N_u eski dosya kaydının arkasına koyar. B2 bu niceliği ölçer; tabanda kırmızı beklenir (ön-kayıtlı, koda dayalı; Ek B B2). Kırmızı çıkmazsa önce türetim sorgulanır. **Öneri: S7(a) reddedilir, S7(b).**

## 7. Alıcı kredileri

**7.1 Birim:** akış başına iki mutlak, artan sınır: `max_seq` (u32) ve `max_bytes` (u32, akışın toplam payload baytı; dosya akışında tavan `MAX_REASSEMBLY_BYTES` = 64 MiB, `0c22a07d:file_transfer.rs:257` → u32 yeter; aşımda tipli hata). Yeniden gönderim, kopya ve FEC kurtarımı aynı `flow_seq`'i taşır → ikinci kredi yok. Live akış (VO): demetler `flow_seq` tüketir ama seq kredisinden muaftır (kabul politikası DUR-06-1); VO akışındaki `Reliable` birimler ayrı mutlak sayaç `max_rel` ile sınırlanır.
**7.2 Havuz ve taahhüt:** alıcı oturum başına sınıf havuzları P_CTRL, P_BE, P_VO, P_BK (birim + bayt); taahhüt = Σ_f (`max_seq_f` − uygulamaya verilmiş_f), kullanılmamış kredi dahil, ≤ havuz; BK kredisi yalnız P_BK'den. RuntimeConfig doğrulaması iki değişmez ister: Σ W + kontrol tavanı < `max_inflight_units`; `delivery_queue` ≥ `max_peers` × Σ havuz. Alıcı FEC blok tavanı (`RX_BLOCKS_CAP` 32, `runtime.rs:3193`) sınıf başına bölünür; BK blokları BE/CTRL bloklarını tahliye edemez, tahliye sayılır.
**7.3 İlk kredi:** sınıf başına `W0` ve `bk_flow_limit0`, `W_CTRL0`, `max_rel0` profil sabitidir; ilk birim ve ilk kontrol hiçbir kontrol turu beklemez.
**7.4 Güncelleme:** `max_seq` = **uygulamaya verilmiş + W_sınıf** (taşımadaki alım değil); `max_bytes` aynı kural. İlan: (i) sınır W/2 ilerleyince; (ii) son BLOCKED(seq)'in üstündeki her artış **hemen**.
**7.5 Canlılık:** gönderen bloklu kaldıkça BLOCKED'i **etkin sınır artana dek** tekrarlar; "yanıt" = sınır artışı, herhangi bir FLOW_CREDIT değil. Aralık 1·srtt'den başlar, ×2, tavan min(RTO_MAX = 30 s, `idle_hold`/2) — tekrarlar alıcının sunucu yuvasını da canlı tutar (`idle_hold` 105 s, `runtime.rs:280`). Mutlak değer + en büyükle birleştirme → bayat/sırasız/tekrar kredi zararsız.
**7.6 Yaşam döngüsü:**
- **FIN:** son birimde bayrak; `close_flow` son birimden sonra çağrılırsa boş FIN birimi, açılışta ayrılan 1 seq'lik FIN yuvasıyla (`max_seq + 1`) gider. Alıcı `fin_seq`'e dek her birimi uygulamaya verince akış durumu ve taahhüt serbest; `bk_flow_limit` ilerler. Tutulan akışın kapanışı uygulama tüketene dek bekler (beyan).
- **RESET:** `reset_flow` → motor akışın birimlerini iptal eder (§8.3) → akış 0'da güvenilir TCTRL birimi `FLOW_RESET(flow, final_seq)`; teyidi motorun kendi teyit çıktısıdır (tek mekanizma). Alıcı tamponu atar, akışı kapalı işaretler.
- **Gelecekteki kontrol yükümlülüğüne yer (H_C_tavan, eş başına) [türetim]:** FILE_ABORT ≤ `MAX_OUTGOING_TRANSFERS` 4 + FLOW_RESET ≤ açık giden BK akışı (≤ `bk_flow_limit0` 4) + FILE_RECEIPT ≤ `MAX_INCOMING_TRANSFERS_PER_PEER` 2 + PTT_CLOSE ve yeniden PTT_OPEN ≤ 2 = **12 akış-0 birimi**; RECEIPT(delivered) 05d bit haritasında (≤ `MSG_DEDUP_WINDOW` 4096) tut-ve-dur ile bekler. W_CTRL0 ≥ 12 + RECEIPT payı [CAL]. Yeni aktarım ya da yeni PTT ancak kendi yuvaları ayrılabiliyorsa kabul edilir; değilse `Backpressure`, sessiz düşürme yok.
- **Oturum değişimi:** akış taşınmaz; kama-app kuralları (TEXT eski `msg_id` ile, dosya `FILE_ABORT(superseded)`, PTT yeniden OPEN).
- **Medya expiry:** Live akışta sıra boşluğu teslimi durdurmaz; süresi dolan demet gönderende iptal edilir; konuşma sonu uygulamada: CLOSE alındı ∧ (CLOSE'tan küçük `flow_seq`'li demetler geldi ∨ CLOSE varışından D_VO sonra) → `PttEnded`; yedek PTT-R4 TTL.
- **Churn:** eşzamanlı BK akışı ≤ `bk_flow_limit` − `bk_resolved_below`; kapalı akış kaydı ≤ açık küme; VO akışı kalıcı olduğundan churn yok.
**7.7 05c reddi = savunma; sayaçlar:** krediyi aşan birim → ACK'siz ret + `credit_violations` (akış 0 dahil: W_CTRL aşımı); kredili birim (akış 0 dahil) için yer bulunamazsa `capacity_defect`. **Kabul kapısı yalnız `capacity_defect = 0`** (sahibin notu 2). Live demetin havuz yüzünden düşmesi `media_dropped_rx` (ACK'lenir; yalnız `DeadlineBounded` birim; `Reliable` birim asla).
**7.8 Kontrol çerçevesi sınırları (DoS):** FLOW_CREDIT yalnız 7.4 tetikleriyle; BLOCKED yanıtı akış başına ≥ 1 srtt aralıklı ve yalnız o akışın girdisini taşır; BLOCKED, yanıt boyuna dolgulanır (amp ≤ 1; CS probe/echo emsali `0c22a07d:wire.rs:220-221`). Girdiler sabit boylu (sızıntı azalır, §11), çerçeve başına ≤ FLOW_CREDIT_MAX_ENTRIES [CAL], fail-closed çözüm (emsal `wire.rs:188-198`); bilinmeyen/kapalı akış girdisi, kredi ötesi BLOCKED, ayrılmış bit ≠ 0, akış ortasında kip değişimi → ret + kol başına sayaç. Gönderen store'u daima min(eşin kredisi, yerel sabit tavan) ile sınırlanır (u32::MAX kredi belleği açmaz).

## 8. FEC / ARQ / kopya; süre dolumu ve iptal

**8.1 FEC:** blok tek akışın birimlerinden oluşur (`block_id` = `flow_id ‖ flow_block`); RECOVERED (`block_id`, `esi`) → (akış, sıra) → ters dizin → birim; başka akışın birimi teyit edilemez. Live akış ilk uygulamada FEC dışı [öneri DUR-06-11]; `Ordered` akışta kısmi blok T_block [CAL] içinde kapanır.
**8.2 ARQ ve kopya** akış etiketini taşır; ARQ_RETX_CAP aşımında kopyaya devir (AMENDMENT-12) aynen; `Live` birimde süre dolunca devir yerine iptal.
**8.3 Süre dolumu ve iptal — tek sonlandırma çıktısı:** motor birime `iptal` durumu verir (süre dolumu: `expires_ns`; RESET: yeni `Event::CancelFlow`). İptal edilmiş birim `tx_queue`, `pending_lost`, `dup_queue` ve parite adaylığından çıkar; sahiplenme (H), kayıp ilanı, kuyruk-probu hedefi ve teyit yolları onu atlar; uçuş kayıtları yalnız muhasebede kalır. Çıktı: `take_confirmed_units` → **`take_finished_units` = (birim, Teyit | İptal)**, birim başına oturumda en çok bir kez. Aynı olayda teyit ve iptal çakışırsa **teyit kazanır**. Runtime payload'u ve uygulama olayını YALNIZ bu çıktıya bağlar → ikinci silme yolu yok. Bu, AMENDMENT-12'nin dört donmuş cümlesini değiştirir (Ek A A.16).

## 9. 05c kapısının akış başına dönüşümü

`AcceptState` (oturum) → (oturum, akış); pencere = kredi; `RefuseWindow` = kredi ihlali; `RefuseFull` kredili birim için yapısal olarak erişilemez (erişilirse `capacity_defect`). Sıra başı rezervi (`DeliveryLedger`, `0c22a07d:runtime.rs:3284-3297`) S1 kipinde kredi taahhüdüyle yer değiştirir (akış 0 dahil: W_CTRL önceden ayrılmış yerdir); örtük kip (bench) aynen. "Durum unutmaz" (`runtime.rs:3359-3361`) akış ömrü boyunca korunur; kapandıktan sonra `late_after_close` kuralı devralır. Mesafe kapısı S1 kipinde kalkar; yeni akışlar eski dosyanın eksik biriminin kimliğine bağlanmaz (B1b).

## 10. kama-app ataması, PTT, sesli mesaj, pompa

- **Atama:** §4 tablosu. Erken birim (`connect` K6) için akış DUR-06-7'dedir (öneri: `connect`'e erken birimin sınıfı; Ready'de o kalıcı akışın ilk birimi).
- **Pompa:** eş başına değil **akış başına** hizmet; bir aktarımın krediden tıkanması aynı eşin diğer aktarımını bekletmez (A.21; B23).
- **Tek kabul yolu (C15 v3 §5(ii) devri):** kama-app'teki her üretici akış açma/gönderme yolundan çıkar; `Messenger::transport_mut()` baypası (`0c22a07d:messaging.rs:425`; kullanımlar `file_transfer.rs:1520`, `ptt.rs:960`, `signaling.rs:426`, :583) kaldırılır ya da daraltılır; baypas testi B24.
- **PTT (DUR-06-18'e bağlı):** önerilen canlı kipte demet süresi = sunum + D_VO [CAL]; gönderen süresi dolanı iptal eder; alıcıda OPEN'dan önce gelen demet sınırlı ön tamponda (N [CAL], ≤ oynatma bütçesi); geç demet oynatılmaz ve sayılır. AMP PTT-R0/PTT-R3 ve PoC-KAPSAM-001 §2/EK-2 md.1/EK-3 değişir (A.18, A.19).
- **Desteklenmeyen profil:** VO yalnız `min(profil, ρ_V,max·r(t))` profili karşılıyorsa açılır ve sürer; değilse tipli hata/olay; **sesli mesaja otomatik ya da görünmez geçiş yoktur** (S5; B19 (b)). Sesli mesaj ayrı, görünür kullanıcı eylemidir (DUR-06-12).
- **05d / 05f:** RECEIPT ve FILE_RECEIPT akış 0'da → dosya FIFO'sunun arkasında beklemez; 05d A1 akış bütçesine dönüşür, tek kabul yolu şartı devralınır (DUR-06-14). 05f rev2: dosya ilerlemesi = dosya akışının teyit tabanı + yeni benzersiz teyit.
- **UniFFI/S5:** `PttBundle` olayına `flow_seq` (sıra) alanı ve anlamı ("teslim sırası değil, akış sırası; geç demet oynatılmaz"); `pttPush` için `DeadlineExpired` ve "reddedileni yeniden ver" kuralının canlı kipte kalkması; KM-SEAM-AK-001 ve README-HASAN (Hasan ile ortak onay); `node.rs` SessionEvent eşlemesine `FlowOpened`/`FlowEnded` (`0c22a07d:node.rs:560-584`). (A.24, DUR-06-23)

## 11. Fiziksel sınırlar, yan kanal, karma sürüm

- Eksik parçayı bekleme kalkar; hat meşguliyeti kalkmaz. İletimdeki paket kesilemez: 1050 B paket 31 B/s'de ≈ 33,9 s, 300 B/s'de 3,5 s [hesap]; BK etkinken VO gecikmesi ≥ S_BK/r (DUR-06-10).
- 1,2 kbit/s ses yalnız payload olarak ≈ 150 B/s → 31 B/s'de canlı ses sürdürülemez. 31 B/s'de dosya kapsamdadır (S2): ağ zarfı `docs/transport-v1/HOSTILE-ENVELOPE.md` profilleriyle ve kesinti süresi/sayısı, rastgele kayıp, dönüş yolu kaybı yazılı olarak Ek B B22'de.
- Kontrol datagramı ≥ mühür ek yükü 22/26 B + gövde; S2 kontrol gövdeleri ayrıca `control_id` u32 taşır [`1b1425d2:specs/kama-noise-wire-tdd-v2.0.md:347`] → 31 B/s'de 12 B'lik kredi çerçevesi ≈ (26 + 12) / 31 ≈ 1,2 s, S2'de ≈ 1,4 s hat süresi [hesap].
- **Yan kanal [çıkarım]:** datagram boyu ve zamanlaması hub'a açıktır; sabit boylu kredi girdileri ve tetik kuralı akış sayısı sızıntısını azaltır, kredi ritmi sızmaya devam eder — bilinçli kabul, ölçülür (B3/B7).
- **Karma sürüm:** kimlik sabitleri telde müzakere edilmez; eski alıcı FLOW_BATCH'i bilinmeyen çerçeve sayıp ACK'siz düşürür [`0c22a07d:runtime.rs:4089`] → sonsuz kurtarma trafiği. Öneri: sürüm/yetenek kimliği doğrulanmış el sıkışmada, `Ready` öncesi fail-closed (DUR-06-13; B25).
- Soket/çekirdek/darboğaz kuyrukları bu tasarımın denetiminde değildir.

## 12. Geçiş sırası ve çakışmalar

1. 05b #140 (rev6) → 05g → 05c merge (C15 v3 §7 Aşama 2–4; 05c rev6 üstüne yeniden tabanlanır).
2. **Deney dalı** `k4g/06-deney` (§14; merge edilmez; FROZEN dosyalar yalnız orada "DENEY" etiketiyle).
3. rev3 + CEVAP 16 → ürün kodu (API-first): (i) S1 + KM-SEAM-AT-001 + drift (typed-error stub) · (ii) client-core çerçeve + akış/kredi + alıcı · (iii) motor S7(b) + engine §13 / AMENDMENT-13 · (iv) kama-app + AMP + PoC-KAPSAM eki + 05d A1 / 05f rev2 akış başına · (v) UniFFI/S5 + kama-node + INT-02 düzeneği.
- **Çakışmalar:** 05c kabul/mesafe kodu (§9); 05g `transport.rs` etkinleştirme kolları (akış yalnız `Ready` sonrası; `Closed` sonrası `FlowRef` → `UnknownFlow`); 05d M1 (korunur), M2/A1 (dönüşür); 05f rev2 akış başına; 05b rev6 K-0…K-5 seçicinin düzey-2 kuralı olur; KARAR-1 geçişi (transport-tdd'de "veri yolu geçişi sonraki sprint" beyanı, `0c22a07d:specs/transport-layer-tdd.md:972-974`) K4G-06 ile birlikte yapılır.

## 13. DUR özeti (öncelikli; ayrıntı Ek D)

| Öncelik | DUR | Soru | Önerim |
|---|---|---|---|
| **1 — deneyden ÖNCE** | DUR-06-5 | Birim çerçevesi, FEC kimliği, KARAR-1 eki, `flow_id` uzayı | B (alt mesaj başlığı; `block_id = flow_id‖flow_block`) |
| 1 | DUR-06-2 | S7 seçim sözleşmesi | (b) eklemeli `AppDataTagged` + motor içi saf seçici |
| 1 | DUR-06-20 | Deney kapsamı; FEC kolunun yeni kodda ölçülmemesi | §14 kapsamı; B2-FEC yalnız tabanda |
| **2 — deney sonucunu bekler** | DUR-06-1 | Canlı medya kredisi | ayrı sınırlı kabul + `max_rel` |
| 2 | DUR-06-6 | Live alıcı teslim kipi | sırasız + `flow_seq` + dedup penceresi |
| 2 | DUR-06-11 | FEC kapsamı | akış başına blok; Live FEC dışı |
| 2 | DUR-06-10 | VO açıkken BK/BE birim boyu | beyan + ölçüm |
| 2 | DUR-06-3 | Kontrol katmanının normatif sayıları ve akış 0 kredisi | §4 sözleşmesi; sayılar deneyden sonra |
| 2 | DUR-06-17 | §5 sınırlarının sözleşmeye "koşullu garanti" olarak girişi | evet, 05b K2 biçiminde |
| **3 — uygulamadan önce** | DUR-06-18 | PoC-KAPSAM-001 PTT hükmü (sahibin tarihli eki) | EK-4: canlı PTT |
| 3 | DUR-06-7 | S1 yüzeyi + `early_unit` | `send(flow)` + açık akış çağrıları; `connect`'e erken birim sınıfı |
| 3 | DUR-06-13 | Sürüm kimliği ve karma sürüm | `transport-v2`; el sıkışmada fail-closed |
| 3 | DUR-06-14 | 05d A1/M2 devri + tek kabul yolu | A1 ayrıca kodlanmaz; şart devralınır |
| 3 | DUR-06-15 | Sinyalleşmenin sınıfı | BE, ayrı kalıcı akış |
| 3 | DUR-06-12 | Sesli mesaj PoC'de kod mu | hayır, yalnız tasarım |
| 3 | DUR-06-21 | ~190 testte DUR-8 yöntemi | toplu yöntem (Ek D) |
| 3 | DUR-06-22 | INT-02/INT-03 kapı metni (A.25) | `capacity_defect = 0` + ayrı sebep sütunları |
| 3 | DUR-06-23 | UniFFI/S5 yüzeyi (Hasan ile) | `PttBundle.flow_seq` + `pttPush` kuralı |
| **4 — Aşama 7** | DUR-06-24 | PTT eşik dosyasının onayı | kalibrasyondan sonra (yöntem sahibin 1. notu; DUR değil) |

DUR'dan çıkarılanlar (C11 §3: uygulama tercihi ya da zaten karara bağlı): rev1 DUR-06-4 (yöntem sahibin 1. notudur → B11 kuralı; asıl karar DUR-06-24), DUR-06-9 (DUR-06-5'e girdi), DUR-06-16 ([CAL] başlangıç değerlerini orkestratör seçer ve raporlar; normatif olunca DUR-06-3/-17).

## 14. Deney dalı (ADIM 2) — kapsam

**Taban:** `0c22a07d` (05c-kod; DeliveryGate/UnitTxLog burada). B2 tabanı ayrıca `37b78aad` (rev6, yalnız motor düzeyi) üstünde koşulur. Deney sonuçları "**ürün profili dışı** (FEC kapalı)" etiketlidir.
- **K1 (kod değişmeden; taban ölçümü):** B2'nin üç kolu (ARQ, kopya, FEC — FEC ürün profilinde açık, `0c22a07d:runtime.rs:292`) iki tabanda; T_BE, §5'in [CAL] değerleriyle K1'den ÖNCE commit'lenir; ayrıca B1-taban (eski API: bütün tüketici durur + dosya birimi kaybolur = `ek-tuketici-durak` tarifi). ≈ 1 gün.
- **K2 client-core:** FLOW_BATCH (B; FEC kapalı), (oturum, akış) kabul + sıra, akış başına hazır kuyruk + test-util `hold_flow`, tek kredi türü (birim) ACK yolunda, BLOCKED; **S1 kipinde mesafe kapısı kalkar, yerini akış kredisi alır** (örtük kip aynen). ≈ 2–3 gün.
- **K3 motor ("DENEY"):** `AppDataTagged` + iki katman (BE/BK) DATA kuyruğu + akış etiketli ARQ/kopya başı seçimi; DRR 3:1 isteğe bağlı (sıra 2). ≈ 1–1,5 gün.
- **K4 testler:** B1-en-küçük, B1b (mesafe kapısı bağı), B2 (ARQ ve kopya kolları, yeni kodda); B3-en-küçük isteğe bağlı (sıra 2). ≈ 1 gün.
- **Regresyon alt kümesi:** `tv_*`, `k4g05b_scheduling` (tek akış), INV-ACK-DELIVER-1 testleri (Ek C.C K satırları). ≈ 0,5 gün.
- **Toplam ≈ 5,5–7 iş günü** (tahmin). Aşama 0 toplamı: tasarım rev1 + rev2 ≈ 1,5 gün harcandı + deney 5,5–7 + rev3 ≈ 1 + inceleme 1–2 ≈ **9–11,5 iş günü**; C15 v3 §7'nin "4–6 + 1–2" tahmininden +3–4 gün fazla. Vektör ve seam beyanı deneyde değişmez; sonuçlar SHA + komut + çıktı ile, origin'de değilse YEREL etiketiyle.

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
| A.10 | `specs/kama-noise-wire-tdd-v2.0.md` §2.6 ID-SCOPE (`1b1425d2:…:430-433`), §3 DÖRT-KİMLİK (:454-458), §2.56 codepoint kaydı (:325-340), §2.57 gövdeler (:347) | `block_id` = `flow_id ‖ flow_block` (oturum + yön tekil, akış içinde monoton, NO-REUSE korunur); `message_id := flow_seq` (S1 parçalamasız kip); akış koordinatı FEC-LOGICAL-IDENTITY'nin YANINDA beşinci kimlik değil, (block_id, ESI)'nin türetim kaynağı; yeni kontrol codepoint'leri **0x1D'den** K-WIRE-CTRL-1 prosedürüyle (0x1B/0x1C heartbeat'e ayrılmış); S2 gövdelerine `control_id` | EVET (KİLİTLİ) | DUR-06-5 | EVET (yeni codepoint; FEC layout aynı) | — |
| A.11 | `crates/kama-transport-schema/src/{prefix.rs, inner_kind.rs, control.rs, lib.rs}` + `crates/kama-noise-wire/tests/golden_wire.rs` | `source_flags`'a akış sonu (FIN) biti (bugün ayrılmış bit → ret, `0c22a07d:…/prefix.rs:15-16`, :36); yeni `inner_kind`/`Control` girdileri; DTO'lar (`LogicalShardHeader`, `Control::Recovered`) düzen olarak değişmez; golden_wire (101 B) **değişmez hedefi** | EVET | DUR-06-5 | EVET (prefix bayrağı; tek kaynak schema) | — |
| A.12 | `crates/client-core/src/runtime.rs` — gönderen | Store girdisine (akış, sıra) + ters dizin; S1 kipinde mesafe kapısı (`:2511`, :2575-2581) → akış kredi kapısı; akış başına yerel tampon (offer FIFO dışında); akış başına FEC biriktirici; `AppDataTagged`; BLOCKED üretimi; sonlandırma çıktısına bağlı silme | EVET (ürün davranışı) | DUR-06-5, DUR-06-2 + EVOLUTION §22 | EVET (A.8) | — |
| A.13 | `runtime.rs` — alıcı (`DeliveryGate`, `AcceptState`, `DeliveryLedger`) | (oturum, akış) kabul; pencere = kredi; `RefuseWindow` = `credit_violations`; `RefuseFull` → `capacity_defect`; S1 kipinde sıra başı rezervi (`:3284-3297`) kredi taahhüdüyle yer değiştirir (akış 0 dahil); sınıf başına `RX_BLOCKS_CAP`; FLOW_CREDIT ACK yolunda, tetik kuralı ve DoS sınırlarıyla | EVET (05c A.6'nın devamı) | DUR-06-3, DUR-06-5 | EVET | KM-SEAM-AT-001 §4 (A.7) |
| A.14 | `runtime.rs` — `RuntimeConfig` / `KamaV1Profile` / kimlik (`:236`, `:241`) | Havuzlar, `W0`, `W_CTRL0`, `bk_flow_limit0`, `max_rel0`, ρ_C, ρ_V,max, Q_BE:Q_BK, T_block, D_VO, W_LIVE, FLOW_CREDIT_MAX_ENTRIES [CAL]; iki doğrulama değişmezi (§7.2); yeni kimlik | EVET (`transport_v1_freeze.rs`, `product_profile_v1.rs`) | DUR-06-13, DUR-06-3 | yok | `api_surface_v0` |
| A.15 | `crates/client-core/src/transport.rs` | (oturum, akış) `Reorder`; akış başına hazır kuyruk + `hold_flow`; `open/close/reset_flow`; `FlowOpened/Ended`; 05g etkinleştirme kollarıyla uyum; erken birim (A.5) | Hayır (adaptör; davranışı S1'e bağlı) | tasarım onayı | yok | H-* real hedef |
| A.16 | `specs/transport-layer-tdd.md` §12 AMENDMENT-12 donmuş cümleleri (`0c22a07d:…:855-899`) + §0 INV-NO-IDLE-TEARDOWN kapalı sonlandırma listesi (:57-63) + yeni AMENDMENT-13 | (1) INV-NO-ORPHAN (:855) kapsamı: "uygulamanın RESET ettiği akış ve süresi dolan `DeadlineBounded` birim hariç"; (2) "Payload yalnız teyit çıktısıyla silinir … Başka hiçbir yol silmez" (:862-864) → "yalnız sonlandırma çıktısıyla (teyit ya da iptal; birim başına oturumda en çok biri)"; (3) "İzleme teyide ya da §0 kapalı listesindeki bir sonlandırmaya dek sürer" (:879) → listeye RESET ve süre dolumu; (4) "Teyit çıktısı (NORMATİF)" (:892-899) → "Sonlandırma çıktısı"; aynı olayda teyit kazanır; iptal teyit ÜRETMEZ; AMENDMENT-13: akış/kredi/seçici | EVET (KAT GO-FREEZE; sahip metni) | yama onayı (DUR-06-2'ye bağlı) | yok | — |
| A.17 | `crates/adaptive-transport/src/sender.rs` (S7 b; taban 05b rev6 `37b78aad`) | Eklemeli `Event::AppDataTagged(OpaqueUnit, ServiceTag)` (etiketsiz = tek akış); katman/akış başına DATA kuyruğu; tek saf seçici: rev6 `refresh_reserve` (:7334), `assign_reserve_to_holder` (:6311), `on_class_emitted` (:6333), `ServiceRights` (:1423), `serve_copy_share` (:7292), `copy_queue_head` (:7243) seçicinin düzey-2'si olur, haklar (akış, sınıf) başına; `TransmitKind::Control`; iptal durumu + `Event::CancelFlow` + `take_finished_units` | **EVET (FROZEN v1.1 motor)** | DUR-06-2 | yok; `tv_*` değişmez HEDEFİ (kayarsa DUR) | — |
| A.18 | `specs/application-message-protocol.md` §3 | PTT-R0 → OPEN/CLOSE `Reliable`, demet `DeadlineBounded`; PTT-R3 → sınırlı ön tampon; "FILE_BEGIN her zaman önce gelir" gerekçesi aynı akışa (`0c22a07d:file_transfer.rs:74-75`); RECEIPT/FILE_RECEIPT/FILE_ABORT akış 0; kayıt baytları DEĞİŞMEZ | EVET (AMP donmuş metin) | DUR-06-18 | AMP baytları ve `amp-record*-v1.json` değişmez | — |
| A.19 | `docs/decisions/PoC-KAPSAM-001.md` §2 PTT satırı (`1b1425d2:…:26`), EK-2 md. 1 (:98), EK-3 cümlesi (:27, :103) + `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` D7 ek (:94, :287) + K4G-08 PTT-R0 + `ptt.rs:18-24` modül belgesi | "PTT Reliable klip; geç ses düşürülmez; transport PoC'de yalnız Reliable taşır" → sahibin tarihli **EK-4**'üyle canlı PTT (§10); K8 hedefleri (:62) korunur, B11'in ürün hedefi girdisi olur | **EVET (KESİN; yalnız sahibin tarihli eki, :7-8)** | **DUR-06-18** | yok | — |
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
