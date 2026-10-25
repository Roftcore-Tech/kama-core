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

DUR'dan çıkarılanlar (C11 §3: uygulama tercihi ya da zaten karara bağlı): rev1 DUR-06-8 (kredi taşıyıcısı → DUR-06-5 (iv)), DUR-06-4 (yöntem sahibin 1. notudur → B11 kuralı; asıl karar DUR-06-24), DUR-06-9 (DUR-06-5'e girdi), DUR-06-16 ([CAL] başlangıç değerlerini orkestratör seçer ve raporlar; normatif olunca DUR-06-3/-17).

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
