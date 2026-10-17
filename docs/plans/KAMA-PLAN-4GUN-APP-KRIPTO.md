# KAMA-PLAN-4GUN-APP-KRIPTO — Application + Kripto katmanı kapanış planı (4 gün) — **v1.1**

| Alan | Değer |
|---|---|
| Karar sahibi | Abkarada |
| Tarih | v1.0 2026-10-08 · **v1.1 2026-10-11** (Claude Code'un 2026-10-11 plan incelemesi sonrası; değişiklikler §6'da) |
| Durum | **ONAYLANDI** — Abkarada, 2026-10-11, §6'daki v1.1 tutarlılık düzeltmeleri dahil ("PLAN-4GUN ONAYLANDI" mesajı bu belgeyle birlikte Claude Code'a iletilir). Bundan sonra değişiklik yalnız tarihli ekle; yeni mimari tartışma açılmaz. İlk kontrol noktası K4G-01: oradan yeni güvenlik bulgusu çıkmazsa plan aynen icra edilir. |
| İki çıkış | **CORE-CLOSE** = Application + Kripto teknik kapanışı (D1, D2, kalıcılık, #105/#107/#108, teslim politikaları, bileşim kökü, INT-02 GEÇTİ). **MOBILE-HANDOFF** = UniFFI `.so` + Kotlin bağlayıcıları + release (K4G-15). İkisi ayrı izlenir; MOBILE-HANDOFF açık kalırsa CORE-CLOSE yine "kapandı" sayılır (D10). |
| Kime | Claude Code (orkestratör + şeritler). Bu belge tek başına yeterlidir; sohbet geçmişi, önceki iş emirleri ve raporlar **varsayılmaz**. Belgede olmayan bir şey gerekirse DUR (§4.4). |
| Üstünlük | Kapsamda `PoC-KAPSAM-001.md` üstündür. Teknik kararda bu belge, önceki iş emirlerinin ve BL'nin üstündedir; çelişen satır üstü çizilir ve buraya atıf yapılır. |
| Yerleşim | `kama-core/docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` |
| Süre | Gün 1 = **2026-10-12** · Gün 2 = 2026-10-13 · Gün 3 = 2026-10-14 · Gün 4 = **2026-10-15**. ("PLAN-4GUN ONAYLANDI" 2026-10-12'den sonra gelirse günler aynı sayıda kayar.) Sonrasında hub, UI ve DevOps sprinti başlar. |

---

## 0. Tanımlar (Claude Code için; başka anlam yüklenmez)

| Terim | Anlamı |
|---|---|
| **Abkarada** | Karar sahibi, tek insan kapısı. Rust tarafının mimarı. |
| **Claude Code / orkestratör / şerit** | İcra. Orkestratör şeritleri (paralel çalışan ajanları) yönetir; her şerit ayrı worktree ve ayrı PR'dır; merge yetkisi orkestratörün (squash merge). |
| **kama-core** | Bu belgenin bulunduğu Rust deposu; `origin/main`, başlangıç commit'i `6be4ff44`. |
| **Katmanlar** | Application (`crates/kama-app`) → Transport (`crates/client-core`, `crates/kama-transport-schema`; ARQ/pacing, düz metin) → Kripto (`crates/kama-noise-wire`; her datagramı tel kenarında mühürler) → UDP. Komşu olmayan katmanlar birbirini çağıramaz: **INV-NEIGHBOR-1**, `scripts/ci/layer_check.py` zorlar. |
| **S1 / S3 / S4 / S5** | Dikişler (seam). S1 Application↔Transport = `crates/kama-contracts/src/s1.rs` (FROZEN). S3 Transport↔Kripto = `crates/kama-contracts/src/s3.rs` v1.1 (FROZEN). S4 Transport↔Hub = `KM-SEAM-TH-001` (bu planda TASLAK). S5 kama-app↔Kotlin = Hasan'ın `ui-api v0.3` belgesi (FROZEN sayılır; Rust karşılığı UniFFI yüzeyi, K4G-15). |
| **FROZEN** | Yalnız Abkarada'nın yazılı kararıyla değişir. Bu planda FROZEN dosyaya dokunan tek iş yoktur; dokunmak gerekirse DUR. |
| **BL** | `SPRINT-BACKLOG-FREEZE.md` (depodaki mevcut yolunda). **KATMAN 6** = BL'nin "sonraki sprinte ertelenenler / KAPSAM DIŞI" bölümü. |
| **AMP** | Application Message Protocol spec'i (kama-app'in kayıt biçimleri; depodaki mevcut `AMP*.md`). Kayıt tipleri `0x01 TEXT … 0x0A MEDIA_BUNDLE`; bu plan `0x0B FILE_RECEIPT` ekler. |
| **Vektör kuralı** | Beklenen baytları **yalnız Python referansı** üretir (`vectors/` altındaki üretici betikler); Rust bunları tüketir. Rust'ın ürettiği bayt hiçbir zaman "beklenen" olarak kaydedilmez. |
| **Spec düzeltme kuralı** | Spec yalnız kod onunla çelişiyorsa ve bu belge "spec değişir" diyorsa değişir. Yöntem: **üstü çiz, silme** (`~~eski~~` + yeni metin) + tarihli **YAMA KAYDI** satırı + `scripts/ci/word_coverage.py` kapısı yeşil. |
| **DUR** | Şerit durur, orkestratör gün raporuna "DUR-<kart>: <soru>" yazar, cevap Abkarada'dan gelene kadar o kartta ilerlenmez; diğer kartlar devam eder. **ACİL DUR** = tüm şeritler durur. |
| **İnsan kapısı** | Yalnız Abkarada'nın **birebir** şu mesajlarıyla geçilir: "PLAN-4GUN ONAYLANDI", "G-IMPL-S3 GEÇTİ", "INT-02 GEÇTİ". Başka ifade kapı sayılmaz. |
| **NON-GATING** | Sprint kapanışını bloklamaz; zaman kalırsa yapılır; yapılmazsa KATMAN 6'ya yazılır. |
| **Kart** | §3'teki `KART K4G-xx` blokları. Her kart tek şeritte, tek PR'da biter; PR başlığı `K4G-xx: <kart başlığı>`. |
| **Gün raporu** | Her günün sonunda orkestratörün Abkarada'ya verdiği ≤ 40 satırlık rapor (§4.3 biçimi). |

---

## 1. Durum — 2026-10-08, `origin/main` 6be4ff44

Kaynak: Claude Code'un 2026-10-08 tarihli iki raporu (durum raporu + araştırma özeti). Derleme/test koşulmadan, kod okumasıyla doğrulanmış bulgular "(kod okuması)" ile işaretli.

### 1.1 Ne bitti

| Alan | Durum |
|---|---|
| Kripto kütüphanesi (`kama-noise-wire`) | A5.0–A5.5 KAPANDI: KDF 12/12 vektör; 22/26 B zarf; replay sırası precheck → AEAD → commit; SSI1/SSR1 el sıkışması (3 cert_mode, 8 DH çağrısı contributory denetimli, katı Ed25519); epoch makinesi N_g = 64 / T_g = 30 s; SessionControl (Initiate, AcceptInit, AcceptResponse, AssignHandle, Renew, Close). 68 ccr vakasının hepsi tüketiliyor; crate 380 test / 0 kırmızı; S3 real hedefi hosted CI'da yeşil. |
| Dikişler | G-S3 geçti (S3 FROZEN; sonradan yalnız K1 ile v1.1, #102). G-S1 geçti (S1 FROZEN, #96). `kama-contracts` sıfır bağımlılıklı; `handles.rs`, `s1.rs`, `s3.rs`. |
| Application (`kama-app`) | 10 tipli AMP codec (#98, zstd yok → KISMİ); Messenger TEXT + RECEIPT(delivered), eş başına u32 `msg_id`, dedup 4096, outbox 256 (#105); FileTransfer BEGIN/CHUNK/END/ABORT, BLAKE2s-256, 64 MiB tavanı (#108); Ptt OPEN/CLOSE + MEDIA_BUNDLE kabı (#107); bağlantı akışı `connect_state`/`cancel_connect`/`connect_with_text` (#110). 86 test (37 birim + 49 entegrasyon), **hepsi sahte transport'la**. AMP vektörleri 31 (11 pozitif + 20 ret). |
| Transport | `client_core::transport::ClientTransport` = S1'in gerçek hedefi, direct-mode (#109). S1 §6'nın 14 satırından 11'i mock + real'de yeşil; H-PROGRESS yalnız mock'ta; 2 satır statik. |
| Hayri'nin işi | B1.0–B1.4 (2026-09-23) `origin/feature/app` f1da50ac'te; #92 ile main'e taşındı (18 vektörün 12'si bayt-özdeş, 6'sı K1'e göre yeniden üretildi; MockTransport'a kayıp enjeksiyonu eklendi). B1.5/B2 Hayri tarafından başlanmadı; hepsi Claude Code şeritlerinde bitti. Dal main'in 48 commit gerisinde; PR yok. |
| CI | #96–#110 merged, PR kontrolleri yeşil; core-main 6be4ff44'te yeşil (9 iş). |

### 1.2 Ne açık (bu planın konusu)

| # | Açık kalem | Kanıt (rapor) | Bu planda |
|---|---|---|---|
| A1 | **Güvenlik (kod okuması):** `NoiseWireProtection::open`, tutamacı kayıtlı olmayan ve son 16 baytı sıfır olan her datagramı HubChannel bağlamına atfedip **AEAD'siz** açıyor (`protector.rs:1149-1168`, `open_hub` `:986-1003`). İstemci runtime'ı açılan çerçevenin bağlamına bakmadan onu ACK / RECOVERED / CS_ECHO / PARAM_ACK olarak işliyor (`client-core/src/runtime.rs:1317-1401`, `:2333`). Saldırı: yoldaki saldırgan sahte ACK ile birimi ARQ deposundan sildirir; birim bir daha gönderilmez. ~~Sınır: uygulama verisi enjekte edilemez (`transport.rs:863` Session dışı bağlamı düşürüyor).~~ **DÜZELTME (Ek 2026-10-11 (3), ACİL DUR raporu 17:53):** bu sınır **sunucu yolunda yanlıştı**. Sunucu eş yuvasını İLK datagramın bağlamıyla kuruyor (TOFU, `runtime.rs:2407`), sonrakileri karşılaştırmıyor, teslimi yuvanın bağlamıyla damgalıyor (`:2021`, `:2052`); `transport.rs:863` datagramın değil yuvanın bağlamına bakıyor. Sonuçlar: (i) kurulu bir eşin adresinden gelen anahtarsız HubChannel DATA o eşin oturumuna **uygulama verisi olarak** giriyor; (ii) açılan her datagram yeni adres için yuva yaratıyor → `max_peers = 64` yol-dışı doldurma (DoS); (iii) oturumlar arası karışma. Üçü de K4G-01 (#114) bağlam-önce sunucu kapısıyla kapanır. | Rapor §4 "Riskler"; ACİL DUR raporu 2026-10-11 | **D1 → K4G-01 (ACİL DUR, Gün 1 ilk iş)** |
| A2 | **Spec sapması:** `init_counter` bellekte, 0'dan başlıyor (`session_control.rs:184`, `:573`). noise-wire RESTART-POLİTİKASI (`:191-193`) kalıcı, monoton sayaç istiyor. Restart eden başlatıcının INIT'i karşı tarafın etkin sayacıyla çakışırsa sessizce `Failed` (`:642-645`). | Rapor §4 | **D2 → K4G-02** |
| A3 | **KS-DOS-1:** yanıtlayanda eş başına provisional oturum tavanı yok; HALF_OPEN_CAP yalnız yanıt önbelleğini kırpıyor (`session_control.rs:752-756`). İstismar için dizindeki bir eşin anahtarı gerekir. | Rapor §4 | **D3 → KATMAN 6 + PoC açığı** |
| A4 | Gecelik kapı KIRMIZI: `scripts/ci/seam-mutation.py:54` yeni `real.rs`'i kopyalamıyor; mini workspace yalnız `kama-contracts` ile kuruluyor, `real.rs` ise `client-core` ve `kama-noise-wire`'a bağlı (#109). | Rapor §5 | **K4G-03** |
| A5 | Haftalık kani + fuzz-deep main'de hiç yeşil koşmadı (onarımlardan sonra koşmadı; sıradaki 2026-10-11). | Rapor §4 | **D17 → K4G-04** |
| A6 | G-IMPL-S3 insan kapısı açık; okuma paketi PR #104 gövdesinde. G-D ve G-IMPL-S1 "geçti" dendi, repoda kayıt yok (G-D'de D1 "ara çerçeve kodda yok" ölçütü beyanlı karşılanmadı). | Rapor §5 | **K4G-04** |
| A7 | T5: `sch_netem` yüklü değil (sahip eylemi). INT-02 (L-INT) başlamadı. | Rapor §5 | **K4G-05** |
| A8 | DUR #105: gönderen yeniden başlarsa `msg_id` 0'dan; alıcı TEXT'i teslim etmiyor ama RECEIPT yolluyor → yanlış "teslim edildi". | Rapor §3 | **D4 → K4G-07** |
| A9 | DUR #107: PTT kayıtları oturum değişiminde yeniden gönderilmiyor; alıcı kuralları AMP §3'te yok. | Rapor §3 | **D5 → K4G-08** |
| A10 | DUR #108: FILE_ABORT'ta yön yok; alıcı iptali eşe bildirilemiyor; dosya alındısı ve yarım aktarım zamanlayıcısı yok. | Rapor §3 | **D6 → K4G-09** |
| A11 | Gerçek adaptör `BestEffort`'u `Reliable` gibi taşıyor (deadline yok, ARQ devrede) → canlı arama medyası için "geç gelen çöptür" gerçekleşmiyor (kod okuması). Raporun bunu PTT'ye bağlaması **yanlış**: PTT PoC'de Reliable kliptir (PoC-KAPSAM-001 §2); Ptt motoru `MEDIA_BUNDLE`'ı BestEffort işaretliyorsa bu kodun kapsam notuyla çelişmesidir. | Rapor §3 | **D7 → K4G-10** (canlı medya), **K4G-08** (PTT Reliable) |
| A12 | Kalıcılık yok: `msg_id`/`stream_id`/`transfer_id` sayaçları, outbox, dedup yalnız bellekte. Kimlik/anahtar deposu yok. | Rapor §3, §4 | **D8 → K4G-06** (sayaçlar, dedup, outbox); kimlik/eşler **depoya girmez**, bileşim kökü provisioning dosyasından bellekte tutar (K4G-11) |
| A13 | Composition-root yok: `NoiseWireProtection::with_control`'ü test dışında yalnız test fikstürü çağırıyor (`seam_mock.rs:640`; anahtarlar "GİZLİ DEĞİLDİR" `:586-589`). Gerçek anahtar, gerçek saat, gerçek entropi, gerçek UDP ile çalışan ürün yolu yok. | Rapor §4 | **K4G-11, K4G-12** |
| A14 | `send_signaling` / `on_signaling` API'si yok (codec'te 0x09 var). | Rapor §3 | **K4G-14** |
| A15 | UniFFI / FFI / facade yok. | Rapor §3 | **D10 → K4G-15** |
| A16 | Kapanış belgeleri: 15 kartta DURUM satırı yok; KATMAN 6'ya 4 erteleme yazılmadı (T3 INIT retry, INIT'e gömülü 0-RTT, `init_counter` kalıcılığı, BatchPayload veri yolu); S1/S3 statüleri hâlâ "SEAM-FROZEN HOLD"; AMP başlık/statü; CLAUDE.md crate listesi (16 üye, crypto crate'i #76'da silindi); ayrıntı listesi raporda. | Rapor §5 | **K4G-16** |
| A17 | Organizasyon: `feature/app` dalının akıbeti; WS-B (AMP) sahibi; devir Hayri'ye iletildi mi. | Rapor §5 | **D12, D13** |
| A18 | Küçükler: `EntropySource` iki yerde tanımlı (`client-core/src/seam.rs:41`, `kama-noise-wire/src/entropy.rs`); ADR-03 "libsodium primary" ama X25519/Ed25519 dalek'te, amendment yok; `protector.rs:651-653` "Tablo bu kapılarla sınırlıdır" cümlesi KS-DOS-1 yolu için yanlış. | Rapor §4 | **K4G-16 (belge), K4G-02 (EntropySource birleştirme NON-GATING)** |

### 1.3 Tek cümleyle

Kripto kütüphane olarak bitti, application bileşen olarak bitti; ikisi **birbirine ve gerçek dünyaya (anahtar, saat, entropi, UDP, disk) hiç bağlanmadı**, bir güvenlik kusuru ve iki spec sapması açık, iki insan kapısı ve INT-02 bekliyor. Bu plan dört günde tam olarak bunları kapatır ve Hasan'a UniFFI yüzeyini teslim eder.

---

## 2. Kararlar (D1–D18)

Her karar "PLAN-4GUN ONAYLANDI" ile KESİN olur. Claude Code bu kararları tartışmaz, uygular; uygulanamıyorsa DUR.

| # | Karar | Uygulayan kart |
|---|---|---|
| **D1** (v1.1) | A1 güvenlik bulgusu **ACİL DUR**'dur ve bu sprintte düzeltilir. Çözüm: **bağlam-önce (context-first) dağıtım.** İlke: *bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer.* Açılan datagramın `ProtectionContext`'i, yük **çözülmeden ve dağıtılmadan önce** doğrulanır: (a) `ctx == Session(h)` ve `h` bu runtime'ın beklediği oturum tutamacıyla **birebir** eşleşiyorsa → o oturumun Session dağıtıcısı; (b) `ctx == HubChannel` → ayrı `on_hub_frame()` dağıtıcısı, PoC'de **yalnız düşür + say** (`ctx_mismatch_drops` / `hub_frames_dropped`), hiçbir Session durumunu değiştiremez; (c) başka her şey → düşür. Yükteki kayıt tipi (ACK, RECOVERED, …) **bağlam seçmek için asla kullanılmaz** — HubChannel yükü AEAD'siz olduğundan saldırgan tipi de seçebilir; bu yüzden "tip → izin verilen bağlam" tablosu **yetersizdir ve yapılmaz**. Hem istemci hem sunucu runtime'ına uygulanır. Hop anahtarı (M4) gerektirmez. | K4G-01 |
| **D2** (v1.1) | `init_counter` kalıcılığı **bu sprintte** düzeltilir, çünkü sahadaki ilk uygulama yeniden başlatması bağlantıyı 7 gün kilitleyebilir. Çözüm: `EntropySource` ve `MonotonicClock` gibi **enjekte edilen** `CounterStore` trait'i, **tek işlemli ve hata dönebilen** arayüzle: `fn reserve_next(&self) -> Result<u64, CounterStoreError>` — depo mevcut değeri okur, artırır, **kalıcılaştırır (commit)**, ancak sonra değeri döndürür. Write-ahead garantisi deponundur, `SessionControl`'ün değil. `load()/store()` çifti **yapılmaz**; hata dönemeyen `load() -> u64` **yapılmaz**. Testlerde bellek içi, üründe redb destekli (K4G-06). Spec değişmez (kod spec'e uydurulur). | K4G-02 |
| **D3** | KS-DOS-1 (eş başına provisional tavan) bu sprintte **yapılmaz**; KATMAN 6'ya kart olarak yazılır ve `PoC-KAPSAM-001 §5` açıklar tablosunda zaten beyanlıdır. `protector.rs:651-653` yorumu düzeltilir (K4G-16). | — |
| **D4** (v1.1) | #105: eş başına `msg_id` sayacı **kalıcı** tutulur (`AppStore::reserve_next`, D2 ile aynı commit-önce semantiği). Alıcı tarafında yeni kural **yok**. Bu çözümün kapsadığı durum: süreç çökmesi / yeniden başlatma, **kalıcı durum yerinde**. Kapsamadığı ve **beyanlı PoC sınırı** olarak yazılan durum: kalıcı durum tamamen kaybolup (veri silme, yeniden kurulum) **aynı eski kimlik** aynı provisioning dosyasıyla yeniden kullanılırsa güvenli devam **garanti edilmez**; bu durumda **yeniden provisioning zorunludur** (`kama-provision` her koşuda yeni anahtar çifti üretir → yeni `dest_hash` → alıcıda yeni eş). Sınır `PROVISIONING-FORMAT-v0.md`'ye ve `PoC-KAPSAM-001 §5`'e (EK-1) yazılır. | K4G-07, K4G-12 |
| **D5** | #107: aktif PTT akışının `PTT_OPEN`'ı her **yeni oturumda bir kez** aynı `stream_id` ile yeniden gönderilir. Alıcı kuralları AMP §3'e yazılır (K4G-08'de birebir). `stream_id` ve `transfer_id` sayaçları da kalıcıdır. | K4G-08 |
| **D6** | #108: yeni kayıt tipi **`0x0B FILE_RECEIPT`** eklenir. `FILE_ABORT` **yalnız gönderen** tarafından kullanılır (düzeni değişmez, yön alanı eklenmez); alıcının iptali `FILE_RECEIPT(status = aborted_by_receiver)` ile bildirilir. Zamanlayıcılar: alıcıda `FILE_IDLE_TTL = 60 s` (CHUNK gelmezse yuva boşalır, `FILE_RECEIPT(timeout)`), gönderende `FILE_RECEIPT_TTL = 60 s` (END sonrası alındı gelmezse aktarım `Failed`). Oturum değişiminde yarım aktarım gönderen tarafından `FILE_ABORT` ile kapatılır; UI "yeniden gönder" sunar. Önce Python referansı + vektörler, sonra Rust. | K4G-09 |
| **D7** | `DeliveryPolicy::BestEffort` ve `DeadlineBounded` `ClientTransport`'ta **gerçek** olur: BestEffort → ARQ deposuna girmez, tek gönderim, yeniden gönderim yok; DeadlineBounded(d) → yeniden gönderim yalnız d dolana kadar, sonra düşer ve `Expired` olayı üretir. S1 FROZEN'dır; bu, S1'in zaten tanımladığı politikanın uygulanmasıdır, S1 değişmez. S1'de bu politikalar tanımlı değilse **DUR**. **Ek 2026-10-11:** bu politikalar yalnız **canlı sesli/görüntülü arama medyası** içindir. **PTT bu politikaları kullanmaz:** PoC-KAPSAM-001 §2 gereği `PTT_OPEN`, `MEDIA_BUNDLE` ve `PTT_CLOSE` **Reliable** taşınır; PTT'de geç gelen ses düşürülmez, progresif oynatma durur ve veri geldikçe devam eder (K4G-08). | K4G-10, K4G-08 |
| **D8** (v1.1) | Kalıcılık = **`redb`** (saf Rust, sabit sürüm). At-rest şifreleme **yok** (PoC-KAPSAM-001 §2). **Asgari kalıcılık, üç ayrı soyutlama, tek somut depo:** (1) `kama-app::AppStore` yalnız uygulama durumu: `msg_id` / `stream_id` / `transfer_id` sayaçları, dedup penceresi, outbox — **başka hiçbir şey**; (2) `kama-noise-wire::CounterStore` yalnız `init_counter` (D2); (3) kimlik, anahtarlar ve eş dizini **depoya girmez** — bileşim kökü (K4G-11) bunları provisioning JSON'undan okur ve bellekte tutar. `crates/kama-store::RedbStore` tek somut struct olarak hem `AppStore` hem `CounterStore`'u gerçekler; arayüzler **birleştirilmez**. Mesaj geçmişi ve ek (blob) dizini **bu sprintte yapılmaz** → KATMAN 6; PoC'de geçmiş bellek içidir (PoC-KAPSAM-001 K2: sahada uygulama yeniden başlatılmaz), alınan dosya diske yazılır ve yolu olayla (`FileReceived(path)`) Kotlin'e verilir. | K4G-06 |
| **D9** (v1.1) | Provisioning biçimi = **JSON v0**, `kama-provision` CLI üretir (K4G-12). Test kökü ile CompactDeviceCert, `cert_mode = ref`. Çalışma zamanında zincir/iptal doğrulaması yok (ccr §35.4 kapalı-sistem kipi, PR #101). **Kök gizli anahtarı sınırı:** `root.json` yalnız provisioning-otoritesi (laptop) eseridir; **cihaz config'ine, Git'e, release paketine ve `.so` zip'ine asla girmez**. CLI çıktı dizinine `.gitignore` yazar, dosyaları `0600` izinle oluşturur, `device-*.json` içinde kök gizli anahtarı bulunmadığını test eder. Cihaz gizli anahtarının JSON'da olması kabul edilmiş PoC riskidir; büyütülmez. | K4G-12 |
| **D10** (v1.1) | UniFFI yüzeyi (Hasan'ın planındaki AB-A8) **Abkarada/Claude Code** işidir; yeni crate `crates/kama-mobile` (cdylib). Hasan yalnız tüketir. Yüzey `ui-api v0.3`'ün PoC-KAPSAM-001 K6'ya göre daraltılmış halidir. **K4G-15 MOBILE-HANDOFF kapısıdır; CORE-CLOSE'u bloklamaz.** K4G-15 yetişmezse gün raporu "CORE-CLOSE kapandı, MOBILE-HANDOFF açık" der; "sprint başarısız" denmez. | K4G-15 |
| **D11** | **Bağımlılık onayları (yalnız bunlar):** `redb` (sabit sürüm), `serde` + `serde_json` (workspace'te yoksa; varsa mevcut sürüm), `uniffi` (sabit sürüm, codegen dahil). **Onaysız:** clap, tokio/async-std (mevcut değilse), zstd/ruzstd, sqlx/rusqlite, reqwest ve diğer her şey. Başka bir bağımlılık gerekirse DUR. `cargo deny` yeşil kalır. | tüm kartlar |
| **D12** | `origin/feature/app` **merge edilmez**; `archive/feature-app-2026-10-04` etiketi (tag) atılır, dal silinir. Devir Hayri'ye Abkarada tarafından sözlü iletildi ve kabul edildi; `docs/hayri-app.md`'ye tarihli not yazılır. `origin/feature/app`'e asla yazılmaz. | K4G-16 |
| **D13** (v1.1) | WS-B (AMP) sahibi: **bu sprint kapanana kadar Abkarada** (icra Claude Code). **K4G-16 kapanış PR'ının merge'inden sonraki ilk iş gününde Hayri'ye devredilir**; devir commit SHA'sı AMP başlığına ve `docs/hayri-app.md`'ye yazılır. Sabit tarih (2026-10-13) **kullanılmaz** — takvim kaydığı için o tarih artık K4G-08/09/14'ün Claude Code'da olduğu güne denk geliyor. | K4G-16 |
| **D14** | Tek TEXT ≤ 1002 B kabul; uzun TEXT bölme yok; `compression_id = NONE`; zstd çözücü kararı sonraki sprint (KATMAN 6 madde 8). | — |
| **D15** | `PlatformSignal.LinkChanged` → `SESSION_INIT_REF` (0-RTT) akışı **hub sprintinde**; bu planda NON-GATING (K4G-16 ek). | K4G-16 (NON-GATING) |
| **D16** (v1.1) | G-D ve G-IMPL-S1 geçmiş olayları **tarihsel gerçekleriyle** kaydedilir (`docs/gates/G-D-WAIVER-001.md`, `docs/gates/G-IMPL-S1-WAIVER-001.md`): kapı o gün hangi ölçütlerle geçildi, hangi ölçütler **karşılanmadı** ve bunlar **istisna/waiver** olarak hangi gerekçeyle kabul edildi. "PLAN-4GUN ONAYLANDI" mesajı geçmişte karşılanmamış bir ölçütü **geriye dönük "geçti" yapmaz**; yalnız waiver kaydının yazılmasını onaylar. Altı ay sonra belgeyi okuyan gerçeği görmelidir. | K4G-04 |
| **D17** | G-IMPL-S3'ten önce **elle bir kani koşusu** (weekly.yml, `workflow_dispatch`, `only=kani`) yapılır ve sonucu okuma paketine eklenir. | K4G-04 |
| **D18** (v1.1) | Heartbeat kod noktaları `HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C` **rezerve edilir** (kod noktası kayıt tablosuna satır); uygulaması hub sprintinde (direct-mode'da yanıt verecek hub yok). Kadans **daha önce karar verilmiştir** ve değişmez: ön plan 8 s / arka plan 25 s (DR-CONNECT-1; PoC-KAPSAM-001 §2 "Süreç ömrü"). Kaçırılan heartbeat eşiği, `LinkSuspect`/`LinkLost` geçişleri, `hub_handle` TTL'i ve mailbox sınırları **bu sprintte kararlaştırılmaz** → G-S4 / hub sprinti; S4 taslağında "AÇIK" olarak bırakılır. | K4G-13 (NON-GATING) |

---

## 3. Kartlar

Şerit haritası (paralel): **L-SEC** K4G-01 · **L-CTR** K4G-02 · **L-CI** K4G-03 · **L-GATE** K4G-04 · **L-INT** K4G-05 · **L-STORE** K4G-06 · **L-APP** K4G-07 → 08 → 09 → 14 (aynı şeritte **sırayla**, hepsi `kama-app`'e dokunur) · **L-TR** K4G-10 · **L-NODE** K4G-11 · **L-PROV** K4G-12 · **L-FFI** K4G-15 · **L-DOC** K4G-16 · *(NON-GATING, ancak diğer şeritler boşta kalırsa)* **L-S4** K4G-13.

**CORE-CLOSE kritik yolu:** K4G-01 → K4G-04 → "G-IMPL-S3 GEÇTİ" → K4G-05 → "INT-02 GEÇTİ"; ve K4G-02 + K4G-06 → K4G-11 (INT-02 bu kökü kullanır). **MOBILE-HANDOFF:** K4G-11 + K4G-14 → K4G-15. CORE-CLOSE kartları: 01, **01b** (Ek (3)), 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 12, 16. MOBILE-HANDOFF kartları: 14, 15. NON-GATING: 13.

### Gün 1 — Güvenlik, spec sapması, CI, kapı paketi

#### KART K4G-01 — Bağlam-önce dağıtım: bağlam dağıtıcıyı seçer, yük bağlamı seçemez (v1.1)
- **Sahip:** Abkarada (karar) · **İcra:** Claude Code L-SEC · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 1, **ilk iş, ACİL DUR**
- **Bağımlı:** — · **Bloklar:** K4G-04, K4G-05
- **DAYANAK:** Durum raporu §4 "Güvenlik": `crates/kama-noise-wire/src/protector.rs:1149-1168` ve `open_hub :986-1003` ("tutamacı kayıtlı olmayan ve son 16 baytı sıfır olan her datagramı HubChannel'a atfediyor ve AEAD'siz açıyor"); `crates/client-core/src/runtime.rs:1317-1401`, `:2333` ("açılan çerçevenin bağlamına bakmıyor; doğrudan ACK, RECOVERED, CS_ECHO ya da PARAM_ACK olarak işliyor"); ~~`crates/client-core/src/transport.rs:863` (teslim yolu Session dışı bağlamı düşürüyor)~~ — **DÜZELTME (Ek (3)):** `:863` yuvanın bağlamına bakar, datagramın değil; sunucu yolu bulguları §1.2 A1 düzeltmesinde. PoC-KAPSAM-001 K10. Karar D1 (v1.1).
- **ACİL DUR KAPANIŞ KOŞULU (Ek (3), DUR-K4G-01 CEVAP):** #114 şu üçünü de içerir: (1) **INV-ACK-DELIVER-1 — teslim yoluna kabul edilmeyen birim ACK'lenmez.** Bulgu: `sync_expected` (`transport.rs:553`) bekleyen niyet/yenileme/provisional/yanıtlayan tutamaçlarını beklenen kümeye koyar, `drive_deliveries` (`:922`) ise Ready olmayan tutamacın teslimini düşürür → yanıtlayanın RESPONSE'tan önce gelen epoch-0 DATA'sı (ör. erken metnin RECEIPT'i) ACK'lenip kaybolur. Varsayılan düzeltme: Ready olmayan tutamacın datagramı **ACK'siz düşer** (sayaç `not_ready_drops`), gönderen ARQ ile yeniden yollar; sınırlı tamponlama (≤ 8 birim/tutamaç) orkestratörün tercihine bırakılır, ikisi de INV'i sağlar. Regresyon testi MemoryHub relay denetimiyle belirlenimli: DATA RESPONSE'tan önce teslim edilir → ACK yok, Ready sonrası yeniden gönderim teslim olur. (2) `max_peers` DoS regresyon testi: `max_peers: 1` ile sunucu testi — anahtarsız datagram yuva açamaz, meşru ikinci eş reddedilmez. (3) 5(a) "bayt-özdeş" kanıtı: test belgesi düzeltilir (Crypto parmak izi bu yolda ayırt edici değil) ya da test-util anlık görüntüsü eklenir.
- **NE:**
  1. **İlke (kodda yorum olarak da yazılır):** "Bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer. Yük, bağlam doğrulanmadan çözülmez."
  2. `client-core` runtime'ında (istemci **ve** sunucu yolu; en az `runtime.rs:1317-1401` ve `:2333` bölgeleri) açılan datagramın `ProtectionContext`'i **yük çözümünden önce** üç kola ayrılır:
     - `Session(h)` ve `h` bu runtime'ın o eş için beklediği oturum tutamacıyla **birebir** eşit (`opened.ctx == expected_ctx`; yalnız "Session mı?" sorusu **yetmez**) → o oturumun Session dağıtıcısı (ACK, RECOVERED, CS_ECHO, PARAM_ACK, veri/batch, oturum kontrol);
     - `HubChannel` → `on_hub_frame(&opened)`: PoC'de **yalnız** `hub_frames_dropped` sayacını artırır ve döner; hiçbir Session durumuna (ARQ deposu, PN penceresi, oturum tablosu) erişimi **yoktur** (fonksiyon imzası bu durumlara referans almaz);
     - başka her şey → düşür, `ctx_mismatch_drops` (u64, telemetriye açık) artar.
  3. `fn allowed_context(frame_type)` gibi **tip-anahtarlı tablo yapılmaz**: HubChannel yükü AEAD'siz olduğundan saldırgan tipi de seçer; tabloya satır eklemeyi unutan ileride deliği yeniden açar.
  4. `protector.rs` davranışı **değişmez** (no-op HubChannel tasarım gereği); `open` sonucunun bağlamı (`ctx`) taşıdığı doğrulanır. Taşımıyorsa ve bunun için S3'e dokunmak gerekiyorsa **DUR** (S3 FROZEN).
  5. Regresyon testi `crates/client-core/tests/ctx_first_dispatch.rs`: (a) uçuşta bir Reliable birim varken, tutamacı kayıtsız + son 16 baytı sıfır bir datagram içine o birimi onaylayan **düzgün biçimli ACK** yerleştirilir ve beslenir → birim ARQ deposunda **kalır**, `hub_frames_dropped == 1`, Session durumu bayt-özdeş; (b) aynı şey RECOVERED, CS_ECHO, PARAM_ACK için; (c) **yanlış tutamaçlı** geçerli bir Session datagramı (başka bir oturumun tutamacı) → düşer, `ctx_mismatch_drops == 1`; (d) Session bağlamından, doğru tutamaçla gelen gerçek ACK hâlâ birimi siler (pozitif kol).
  6. `seam-coverage.json`'da H-CTX satırının "Session ≠ HubChannel anahtarı" yarısı için real hedef durumu dürüstçe yazılır ("real: green" iddiası varsa ve kanıt yoksa `declared` yapılır).
- **NEDEN:** Yoldaki saldırgan tek sahte ACK ile mesajı/dosyayı sessizce öldürebilir; PoC sahasında bu "Kama mesajı yemiş" görünür.
- **KAZANÇ:** Kusur hop anahtarı beklenmeden kapanır; yeni çerçeve tipi eklemek güvenlik kararı gerektirmez (bağlam zaten önce seçilmiş olur).
- **TRADE-OFF:** Hub sprintinde HubChannel dağıtıcısına gerçek handler'lar eklenirken "Session durumuna erişim yok" imzası korunmak zorunda (bilinçli kısıt).
- **SINIR:** Hop anahtarı, hub ARQ, heartbeat **uygulanmaz**. `protector.rs` ve `kama-contracts` **değişmez**.
- **GİRDİ:** Yukarıdaki dosya/satır referansları; S3 `kama-contracts::s3` tipleri.
- **DONE:** Test 5(a)–(d) yeşil; `cargo test -p client-core` ve S1/S3 conformance yeşil; `layer_check.py` yeşil; PR gövdesinde "D1 (v1.1) uygulandı", iki sayacın adı ve `on_hub_frame` imzası; gün raporunda "ACİL DUR kapandı" satırı.

#### KART K4G-01b — Eş yuvası ve `Reorder` oturuma anahtarlanır (Ek 2026-10-11 (3))
- **Sahip:** Abkarada · **İcra:** Claude Code L-SEC (aynı şerit, #114 merge'inden **hemen sonra**) · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 1
- **Bağımlı:** K4G-01 merged · **Bloklar:** K4G-10 (aynı dosyalar: `runtime.rs`, `transport.rs`), K4G-04 paketi
- **DAYANAK:** #114 incelemesi bulgu 4 ve 5: `ClientTransport` `Reorder` **adres-anahtarlı** ve oturum kapanınca silinmiyor → aynı porttan gelen yeni oturumun ilk birimleri sessizce atılabiliyor (ACK'li kayıp); hattaki saldırgan geçerli datagramı sahte adresten önce iletirse ek yuva açılır ve adres-anahtarlı Reorder S1'de çift teslim üretebilir. #114 beyanlı istisna 5: yenileme sonrası efemeral port yeniden kullanılırsa sunucu yuvası eski oturuma bağlı kaldığı için yeni oturum idle emekliliğe (105 s) dek düşer. PoC-KAPSAM-001 K4 (cancel+connect ve supersession sahada olağan), K5 (bağ yalnız el sıkışmayla değişir).
- **NE:**
  1. `Reorder` durumu `SessionHandle` ile anahtarlanır (adresle değil); oturum `Closed` olunca silinir.
  2. Sunucu eş yuvası anahtarı `(adres, SessionHandle)` olur. Yuva yalnız **beklenen kümedeki tutamaçla AEAD-geçerli** bir datagramla yaratılır (K4G-01 kapısı korunur). Aynı adresten yeni (beklenen) oturum → **yeni yuva**; eski yuva bağlı oturum kapanınca ya da `idle_hold` ile emekli olur. **Yuvanın oturum bağı veri datagramıyla asla değişmez** (K5 analoğu).
  3. Testler: (a) cancel+connect aynı porttan → yeni oturumun ilk birimi teslim olur, `ctx_mismatch_drops == 0`; (b) yenileme sonrası aynı port → 105 s beklenmez; (c) adres-sahteli kopya datagram → S1'de tek teslim (çift teslim yok); (d) `max_peers` sınırı yeni anahtarla da geçerli.
  4. Beyanlı kalan açık (INT-02 raporuna): hattaki saldırgan gözlediği geçerli datagramı sahte adresten önce ileterek `max_peers` içinde ek yuva açabilir; kapanış M4 / adres doğrulama (PoC-KAPSAM-001 §5).
- **NEDEN:** K4'ün tek kurtarma yolu (yeniden INIT) aynı adresten gelir; 105 s kara delik ve ACK'li kayıp demoda "mesaj gitti ama gelmedi" olur.
- **KAZANÇ:** Bulgu 3 (karışma), 4 ve 5 tek yapısal değişiklikle; K4G-10 temiz tabana oturur.
- **TRADE-OFF:** Yuva sayısı geçici olarak adres başına > 1 olabilir (max_peers içinde).
- **SINIR:** Adres doğrulama / path migration yok (M4). `protector.rs`, `kama-contracts` değişmez.
- **GİRDİ:** #114 inceleme raporu; `transport.rs` Reorder; `runtime.rs` yuva tablosu.
- **DONE:** Testler (a)–(d) yeşil; tam yerel kapı yeşil; #114'ün testleri hâlâ yeşil; PR gövdesinde yuva anahtarı ve emeklilik kuralı; INT-02 beyan satırı hazır.

#### KART K4G-02 — `init_counter` kalıcılığı: enjekte edilen `CounterStore`
- **Sahip:** Abkarada · **İcra:** Claude Code L-CTR · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 1
- **Bağımlı:** — · **Bloklar:** K4G-11 (ürün gerçeklemesi K4G-06 ile)
- **DAYANAK:** Durum raporu §4 "Spec sapması": `crates/kama-noise-wire/src/session_control.rs:184`, `:573` (bellekte, 0'dan); noise-wire RESTART-POLİTİKASI `:191-193` (kalıcı, monoton); `:642-645` ("aynı sayaç, farklı bayt" → sessiz `Failed`); "#104 YORUM 9 çakışan sayaç için yanlış". Karar D2.
- **NE:**
  1. `kama-noise-wire` içinde `pub trait CounterStore: Send + Sync { fn reserve_next(&self) -> Result<u64, CounterStoreError>; }` (aynı crate'teki `EntropySource` ile aynı enjeksiyon kalıbı; `std` dışı bağımlılık yok). Sözleşme (trait belgesine yazılır): "Döndürülen değer, çağrı dönmeden **önce** kalıcılaştırılmıştır; aynı depo aynı değeri iki kez döndürmez; depo hata verirse değer **ayrılmamıştır**." `MemoryCounterStore` (test; içinde `Mutex<u64>`) crate içinde. `load()/store()` çifti **yoktur**.
  2. `SessionControl` yapıcısı `CounterStore`'u alır; her INIT için `reserve_next()?` → INIT o değerle gönderilir. Hata → INIT **gönderilmez**, `Failed(CounterPersist)`. `SessionControl` sayacı kendisi artırmaz ve belleğe almaz.
  3. Mevcut tüm çağrı yerleri (`seam_mock.rs:640` fikstürü dahil) `MemoryCounterStore` ile güncellenir; S3 FROZEN olduğu için `s3.rs`'e dokunulmaz — yapıcı imzası S3'te tanımlıysa **DUR**.
  4. Test: (a) restart simülasyonu — aynı `CounterStore` ile ikinci `SessionControl` kurulur, ilk INIT sayacı öncekinden büyük; (b) çakışma senaryosu — karşı tarafta etkin oturum sayacı `n` iken restart eden taraf `n` ile INIT atarsa `Failed`, `n+1` ile atarsa kabul (bugünkü davranışın testi; #104 YORUM 9 düzeltilir); (c) depo hata döndürürse INIT gönderilmez ve sayaç ilerlemez (`FailingCounterStore` ile).
  5. NON-GATING: `EntropySource`'un iki tanımı (`client-core/src/seam.rs:41`, `kama-noise-wire/src/entropy.rs`) tek tanıma indirilir (F0.3 taşımasının tamamlanması).
- **NEDEN:** Sahada uygulama bir kez kapanıp açılırsa bağlantı 7 gün kurulamayabilir; demo için ölümcül.
- **KAZANÇ:** Spec'e uyum; ürün deposu takılınca (K4G-06) sıfır ek iş.
- **TRADE-OFF:** INIT başına bir disk yazması (saniyede değil, oturum başına; ihmal edilebilir).
- **SINIR:** redb gerçeklemesi bu kartta değil (K4G-06). Spec **değişmez**.
- **GİRDİ:** `session_control.rs`, `entropy.rs`, `seam_mock.rs`.
- **DONE:** 4(a)–(c) yeşil (hata veren depo testi dahil); kama-noise-wire ve S3 conformance yeşil; #104 YORUM 9 düzeltilmiş; PR gövdesinde trait imzası.

#### KART K4G-03 — Gecelik kapı onarımı (`seam-mutation.py`)
- **Sahip:** Abkarada · **İcra:** Claude Code L-CI · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 1
- **Bağımlı:** — · **Bloklar:** K4G-16
- **DAYANAK:** Durum raporu §5: "core-nightly 37748160454 (6be4ff44): seam-mutation derlenmiyor"; "`seam-mutation.py:54` bu dosyayı (`real.rs`) kopyalamıyor ve mini workspace'i yalnız kama-contracts ile kuruyor; real.rs ise client-core ve kama-noise-wire'a bağlı"; CLAUDE.md §6 "main her zaman yeşil".
- **NE:** `scripts/ci/seam-mutation.py` mini workspace'i `client-core` ve `kama-noise-wire`'ı (ve geçişli bağımlılıklarını) içerecek şekilde kurar **veya** mutasyon turu ana workspace içinde özellik bayrağıyla koşar — ikisinden hangisi daha az kod değiştiriyorsa o; seçim PR gövdesinde gerekçeli. `real.rs` mutasyon turuna dahil edilir. Yerelde `python3 scripts/ci/seam-mutation.py` yeşil koşturulur; sonra `workflow_dispatch` ile core-nightly main'de tetiklenir.
- **NEDEN:** Main gecelikte kırmızı; kural ihlali.
- **KAZANÇ:** Real hedef de mutasyon kapsamına girer (bugüne kadar yalnız mock).
- **TRADE-OFF:** Gecelik süresi uzar (kabul).
- **SINIR:** Mutasyon operatörleri değişmez.
- **GİRDİ:** `scripts/ci/seam-mutation.py`, `.github/workflows/*nightly*`.
- **DONE:** Elle tetiklenen core-nightly main'de **yeşil**; koşu kimliği PR gövdesinde.

#### KART K4G-04 — G-IMPL-S3 okuma paketi + kani koşusu + G-D / G-IMPL-S1 kaydı
- **Sahip:** Abkarada · **İcra:** Claude Code L-GATE · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 1 (akşam)
- **Bağımlı:** K4G-01, K4G-02 · **Bloklar:** K4G-05
- **DAYANAK:** Durum raporu §5 tablo: "G-IMPL-S3 GEÇİLMEDİ; okuma paketi PR #104 gövdesinde"; "G-D, G-IMPL-S1: orkestratör geçti dedi, repoda kayıt yok"; §4: "Haftalık kapı main'de hiç yeşil olmadı"; öneri "G-IMPL-S3'ten önce elle bir kani koşusu". Kararlar D16, D17.
- **NE:**
  1. `weekly.yml` → `workflow_dispatch`, `only=kani`; sonuç (koşu kimliği, yeşil/kırmızı, kırmızıysa neden) kaydedilir. Kırmızıysa **DUR** (kapı paketi yine hazırlanır, kırmızı beyan edilir). **DUR-K4G-04 CEVAP (Ek (3)):** kani adımı #89'dan beri derlenmiyor (`noise_conformance_core` hedefi `required-features=["test-util"]`); `weekly.yml:50`'deki kani komutuna **yalnız** `--features test-util` eklenir, başka workflow değişikliği yok; bu değişiklik K4G-04 PR'ındadır ve CI kararı olarak Abkarada onaylıdır. Hosted CI kilitliyse kani yerelde (`cargo kani`, kurulumu mümkünse) koşulur, mümkün değilse kırmızı beyan edilir.
  2. `docs/gates/G-IMPL-S3-OKUMA-PAKETI.md`: PR #104 gövdesindeki paket aynen + K4G-01 ve K4G-02 PR bağlantıları ve test sonuçları + kani sonucu + bilinen açıklar (KS-DOS-1, HubChannel no-op, 0-RTT yok, INIT retry yok, extended zarfın bağımsız Python vektörü yok). Sonunda tek satır: "Abkarada'nın 'G-IMPL-S3 GEÇTİ' mesajı bekleniyor."
  3. `docs/gates/G-D-WAIVER-001.md` ve `docs/gates/G-IMPL-S1-WAIVER-001.md` (D16 v1.1): her biri üç bölüm — (i) **Tarihsel olay:** kapı ne zaman, hangi commit'te, kim tarafından "geçti" ilan edildi; (ii) **Karşılanmayan ölçütler:** G-D: D1 "ara çerçeve kodda yok" karşılanmadı (BatchPayload geçişi KATMAN 6); G-IMPL-S1: H-PROGRESS real'de yok, zstd pending, blake2 dar istisnası, S1 real kama-app'i çalıştırmıyor; (iii) **Waiver:** bu ölçütlerin hangi gerekçeyle ve hangi kapanış kartıyla (KATMAN 6 satırı) istisna olarak kabul edildiği. Başlık: "WAIVER — geçmişte karşılanmamış ölçüt geriye dönük geçmiş sayılmaz; PLAN-4GUN ONAYLANDI yalnız bu kaydın yazılmasını onaylar." BL'deki kapı satırı "Geçti" değil, "**Geçti (waiver ile): G-D-WAIVER-001**" yazar.
- **NEDEN:** İnsan kapısı ancak kanıt paketiyle geçilir; iki kapı kayıtsız kaldı.
- **KAZANÇ:** INT-02 önündeki tek engel Abkarada'nın bir mesajı olur.
- **TRADE-OFF:** —
- **SINIR:** Kapıyı Claude Code geçmez; "GEÇTİ" yazmaz; waiver belgesi geçmiş ölçütü "karşılandı"ya çevirmez.
- **GİRDİ:** PR #104 gövdesi, `weekly.yml`, BL.
- **DONE:** Üç belge main'de; kani koşusu kaydı; gün raporunda "G-IMPL-S3 paketi hazır: <bağlantı>".

### Gün 2 — INT-02, kalıcılık, DUR çözümleri, gerçek teslim politikaları

#### KART K4G-05 — L-INT: INT-02 koşusu ve raporu
- **Sahip:** Abkarada · **İcra:** Claude Code L-INT · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 2 (kapı geçince)
- **Bağımlı:** "G-IMPL-S3 GEÇTİ" mesajı **ve** Abkarada'nın `sudo modprobe sch_netem` eylemi (T5) · **Bloklar:** sprint kapanışı
- **DAYANAK:** Durum raporu §5 "Bizde kalan iş / L-INT": `crates/kama-integration/{examples/int02_node.rs, tests/int02_loopback.rs}`; GATING (1) iki yönde `delay 200ms 200ms loss 30%` altında 100 TEXT + 100 RECEIPT; GATING (2) akış sırasında epoch 0→1, kayıp yok, tek oturum; assert Application retransmit = 0 ve Crypto retransmit = 0; recovery OFF negatif kolu; T4 sabit seed, 5 tohum, 5/5. Rapor §5 beyanlı boşluk listesi.
- **NE:**
  1. T5 yüklü değilse (`lsmod | grep sch_netem` boş) **başlanmaz**; gün raporuna "T5 bekleniyor" yazılır.
  2. İki düğüm (`int02_node.rs`) gerçek `ClientTransport` + gerçek `NoiseWireProtection` + `kama-app` Messenger ile loopback UDP üzerinde; netem iki yönde. Anahtarlar: K4G-12 hazırsa `kama-provision` çıktısı, değilse test fikstürü — hangisi kullanıldığı raporda **yazılır**.
  3. GATING (1), (2) ve T4 birebir yukarıdaki gibi; her tohum için ölçülen: teslim %, medyan/95p gecikme, epoch geçiş anı, ARQ yeniden gönderim sayısı (transport), application retransmit (0 olmalı), crypto retransmit (0 olmalı), `ctx_mismatch_drops` (0 olmalı).
  4. Rapor `docs/reports/INT-02-RAPORU.md`, **beyanlı boşluklar** bölümü zorunlu ve en az şunları içerir: 2026-10-08 raporundaki "uygulama verisi enjekte edilemez" iddiası sunucu yolunda yanlıştı, K4G-01 (#114) ve K4G-01b ile kapatıldı, regresyon `server_*` testleri (Ek (3)); hattaki saldırganın gözlediği geçerli datagramı sahte adresten önce iletmesiyle `max_peers` içinde ek yuva açılabilir (K4G-01b §4, kapanış M4); direct-mode'da hub rölesi / hop-ARQ / HubChannel ACK sınanmadı; INIT retry yok (T3); ara çerçeveden BatchPayload'a geçiş sonraki sprint; zstd yok; H-PROGRESS real'de sınanmadı; M4 yok → SSI1 alanları (`recipient_dest_hash`, `sender_cert_ref`, `init_counter`) UDP yan kanalında düz metin (transport-layer-tdd §1 ve CLAUDE.md §4 dest_hash kuralıyla çelişir, PoC-KAPSAM-001 §5'te beyanlı); anahtarların kaynağı; boş batch tel boyutu ölçülemiyorsa hesapla verilir (`batch.rs:51-52` codec'te 1 B) ve "hesap" diye işaretlenir.
  5. NON-GATING ek koşu: 100 TEXT + 100 RECEIPT + 1 dosya (≈150 KB) + 1 PTT klibi (K4G-08/09/10 merge olduysa) aynı netem profiliyle; sonuçlar raporun "ek" bölümüne. **(ek 2026-10-11)** İkinci NON-GATING koşu: akış ortasında **60 s tam kesinti** (`loss 100%`), sonra normal — aynı oturumda tüm TEXT'ler teslim oluyor mu, PTT klibi kaldığı yerden devam ediyor mu (PoC-KAPSAM-001 K8; hub mailbox olmadığı için bu yük tamamen gönderen outbox'ı + transport ARQ'dadır). Teslim olmuyorsa raporda **neden** (ARQ terk süresi, oturum durumu) yazılır; bu bir DUR değil, ölçümdür.
- **NEDEN:** kama-app gerçek transport ve gerçek kriptoyla hiç koşmadı; sprint çıkışı bu.
- **KAZANÇ:** İlk uçtan uca kanıt; demo ölçütleri için ilk sayı.
- **TRADE-OFF:** Loopback + netem saha değildir; raporda yazılır.
- **SINIR:** GATING kapsamı genişletilmez; hub yok.
- **GİRDİ:** Yukarıdaki dosyalar, PoC-KAPSAM-001 §5.
- **DONE:** 5/5 tohum yeşil; rapor main'de; gün raporunda "INT-02 raporu hazır, 'INT-02 GEÇTİ' bekleniyor".

#### KART K4G-06 — Asgari kalıcılık v0: `AppStore` trait'i + `crates/kama-store` (redb) (v1.1)
- **Sahip:** Abkarada · **İcra:** Claude Code L-STORE · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 2
- **Bağımlı:** K4G-02 (CounterStore trait'i) · **Bloklar:** K4G-07, K4G-11
- **DAYANAK:** Durum raporu §3 "Kalıcılık da yok: msg_id, stream_id ve transfer_id sayaçları, outbox ve dedup penceresi yalnız bellekte". PoC-KAPSAM-001 §2 "Kalıcılık (Rust)" (EK-1 ile daraltıldı). Kararlar D8 (v1.1), D11.
- **NE:**
  1. `crates/kama-app/src/store.rs`: `pub trait AppStore: Send + Sync` — **yalnız** şu işlemler: `reserve_next(peer, kind: CounterKind {MsgId, StreamId, TransferId}) -> Result<u64, AppStoreError>` (D2 ile aynı sözleşme: dönmeden önce kalıcı, aynı değer iki kez dönmez, hata = ayrılmadı); `dedup_load(peer) -> Result<DedupWindow>` / `dedup_save(peer, &DedupWindow)`; `outbox_put(peer, unit)` / `outbox_remove(peer, msg_id)` / `outbox_list(peer) -> Vec<unit>`. **Başka işlem yok**: kimlik, anahtar, eş dizini, mesaj geçmişi, blob dizini bu trait'e **girmez** (D8). `MemoryAppStore` (test; `Mutex` ile) aynı dosyada. Messenger/Ptt/FileTransfer sayaçlarını **yalnız** bu trait üzerinden alır (K4G-07/08'de bağlanır).
  2. `crates/kama-store` (yeni crate): tek somut `pub struct RedbStore` → `impl kama_app::AppStore for RedbStore` **ve** `impl kama_noise_wire::CounterStore for RedbStore`. Tek `redb` dosyası; tablolar: `app_counters` (anahtar: peer + kind), `init_counter` (tek satır), `dedup`, `outbox`. Her `reserve_next` tek redb yazma işlemi (transaction) içinde okur-artırır-commit eder, sonra döner.
  3. Bağımlılık yönü: `kama-store` → `kama-app` (trait için) ve `kama-noise-wire` (CounterStore için). `layer_check.py`'de `kama-store` **altyapı** sınıfı olarak kaydedilir: kimse ona bağımlı olamaz, yalnız bileşim kökü (K4G-11) kullanır.
  4. Testler: crash-safety — `reserve_next` döndükten sonra süreç "ölür" (store kapatılıp yeniden açılır), sonraki değer öncekinden büyük (üç `CounterKind` + `init_counter` için ayrı ayrı); dedup penceresi yeniden açılışta korunur; outbox yeniden açılışta aynı; iki iş parçacığından eşzamanlı `reserve_next` çakışmaz (değerler benzersiz).
- **NEDEN:** PoC'de uygulama yeniden başlayınca `msg_id` 0'a dönerse #105 hatası; outbox kaybolursa "gönderiliyor"daki mesaj yok olur.
- **KAZANÇ:** D2/D4/D5 kalıcılık tek dosyada, üç dar arayüzle; application katmanı kimlik/anahtar bilmez.
- **TRADE-OFF:** redb yeni bağımlılık (onaylı D11); şema v0, göç yok (PoC). Geçmiş/blob dizini KATMAN 6.
- **SINIR:** Şifreleme yok; şema göçü yok; arama/sorgu yok; geçmiş yok; kimlik yok.
- **GİRDİ:** kama-app Messenger/Ptt/FileTransfer sayaç kullanım noktaları; `CounterStore` (K4G-02).
- **DONE:** `cargo test -p kama-store -p kama-app` yeşil; `cargo deny` yeşil; `layer_check.py` yeşil; crash-safety ve eşzamanlılık testleri yeşil; `AppStore`'da yukarıdaki 5 işlemden fazlası yok.

#### KART K4G-07 — DUR #105 çözümü: kalıcı eş başına `msg_id`
- **Sahip:** Abkarada · **İcra:** Claude Code L-APP (1/4) · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 2
- **Bağımlı:** K4G-06 · **Bloklar:** K4G-08
- **DAYANAK:** Durum raporu §3 DUR #105: "Gönderen yeniden başlarsa msg_id 0'dan başlıyor. Alıcı yeni TEXT'i 'görülmüş' sayıp teslim etmiyor ama RECEIPT yolluyor; gönderen yanlışlıkla 'teslim edildi' görüyor." Karar D4.
- **NE:** Messenger `msg_id`'yi `AppStore::reserve_next(peer, MsgId)` ile alır (bellek içi sayaç kalkar). Outbox `AppStore`'a taşınır; yeniden başlatmada outbox'taki birimler yeni oturumda yeniden gönderilir (bugünkü oturum-değişimi yolu). Alıcı tarafında **değişiklik yok**. AMP §3'e iki cümle: "Gönderen `msg_id`'yi kalıcı tutar. **Beyanlı sınır (D4):** kalıcı durum tamamen kaybolup aynı kimlik yeniden kullanılırsa güvenli devam garanti edilmez; yeniden provisioning zorunludur (PROVISIONING-FORMAT-v0 §x)." Test: Messenger yeniden kurulur (aynı store) → ilk `msg_id` öncekinden büyük; alıcı teslim eder, RECEIPT doğru.
- **NEDEN:** Yanlış "teslim edildi" demoda görünür bir yalan olur.
- **KAZANÇ:** Alıcıda sezgisel kural gerekmez; dedup penceresi basit kalır.
- **TRADE-OFF:** Çözüm "tam" değildir: durum kaybı + aynı kimlik senaryosu yerel olarak tespit edilemez (geçmişi bilen kaynak yok). PoC için kabul; beyanlı sınır olarak yazılır, "çözüldü" denmez.
- **SINIR:** RECEIPT(read) yok; aynı oturumda zamanlayıcılı retry yok (PoC-KAPSAM-001).
- **GİRDİ:** `crates/kama-app` Messenger, `store.rs`.
- **DONE:** Test yeşil; AMP §3 cümlesi (üstü çiz/ekle + YAMA KAYDI); #105'teki DUR notu kapatıldı olarak işaretli.

#### KART K4G-08 — DUR #107 çözümü: PTT yeniden `OPEN` ve alıcı kuralları
- **Sahip:** Abkarada · **İcra:** Claude Code L-APP (2/4) · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 2
- **Bağımlı:** K4G-07 · **Bloklar:** K4G-09
- **DAYANAK:** Durum raporu §3 DUR #107: "PTT kayıtları oturum değişiminde yeniden gönderilmiyor. Alıcı kuralları AMP §3'te yazılı değil." Durum raporu §3 Ptt satırı: "OPEN/CLOSE Reliable, MEDIA_BUNDLE BestEffort" — bu, PoC-KAPSAM-001 §2 PTT satırıyla ("**Reliable** klip … ağ takılırsa ses bozulmaz, durur") **çelişir**; kapsam notu üstündür. Kararlar D5, D7 (ek).
- **NE:**
  0. **(ek 2026-10-11) PTT taşıma politikası:** Ptt motoru `PTT_OPEN`, **`MEDIA_BUNDLE`** ve `PTT_CLOSE`'un üçünü de **`DeliveryPolicy::Reliable`** ile gönderir. Kodda `MEDIA_BUNDLE` BestEffort ise Reliable'a çevrilir. Alıcıda `MEDIA_BUNDLE`'lar `seq` sırasıyla oynatılır; eksik parça gelene kadar oynatma **durur** (bozuk hece üretmek yasaktır), gelince devam eder. Bu, canlı arama medyasından (K4G-10, BestEffort/DeadlineBounded) **ayrıdır**.
  1. Ptt motoru aktif akış (OPEN gönderilmiş, CLOSE gönderilmemiş) için S1'in yeni-oturum/supersession olayında `PTT_OPEN`'ı **aynı `stream_id`** ile **bir kez** yeniden gönderir (Reliable). `stream_id` `AppStore::reserve_next(peer, StreamId)` ile alınır.
  2. AMP §3'e **birebir** şu alıcı kuralları eklenir (üstü çiz/ekle + YAMA KAYDI):
     - "PTT-R0: `PTT_OPEN`, `MEDIA_BUNDLE`, `PTT_CLOSE` Reliable taşınır; alıcı `MEDIA_BUNDLE`'ları sırayla oynatır, eksik parça gelene kadar bekler, geç gelen parçayı **düşürmez**."
     - "PTT-R1: Bilinen `stream_id` için tekrar gelen `PTT_OPEN` idempotenttir; yeni akış yaratmaz, hata değildir."
     - "PTT-R2: Bilinmeyen `stream_id` için gelen `PTT_CLOSE` sessizce yok sayılır."
     - "PTT-R3: Bilinmeyen `stream_id` için gelen `MEDIA_BUNDLE` düşürülür; sayaç `ptt_orphan_bundles` artar."
     - "PTT-R4: Bir akışa `PTT_STREAM_IDLE_TTL = 120 s` boyunca `MEDIA_BUNDLE` gelmezse alıcı akışı yerel olarak kapatır (CLOSE üretmez)." (120 s: PoC-KAPSAM-001 K8'deki 60 s kesinti senaryosunda akış yaşamalı; Reliable klip kesinti sonrası devam eder.)
     - "PTT-R5: Aynı eşten aynı anda en çok `PTT_MAX_ACTIVE_STREAMS = 2` akış; fazlası düşürülür."
  3. Testler: supersession sonrası OPEN yeniden gönderimi tam bir kez; R0 (sahte transport'ta `MEDIA_BUNDLE` politikası Reliable; sıra dışı/geç gelen parça oynatma kuyruğunda beklenir, düşmez); R1–R5 her biri bir test.
- **NEDEN:** Oturum değişiminde (K4) alıcı OPEN'ı kaçırırsa gelen ses "yetim" kalır. Ve demo ölçütü "sıfır bozuk hece" ancak Reliable taşımayla mümkündür.
- **KAZANÇ:** PTT, IP değişimini bir kez yeniden gönderilen 1 kayıtla atlatır; 60 s kesintiden sonra klip kaldığı yerden devam eder.
- **TRADE-OFF:** PTT gecikmesi kötü ağda büyür (kabul: "gecikme önemli değil, ne olursa olsun gitsin").
- **SINIR:** Ses yükü opak kalır; Lyra Rust'ta yok (medya motoru). Canlı arama medyası bu karta girmez.
- **GİRDİ:** `crates/kama-app` Ptt, AMP §3, PoC-KAPSAM-001 §2 PTT satırı.
- **DONE:** Testler yeşil; AMP §3 R0–R5 yazılı; word_coverage yeşil; #107 DUR kapatıldı; PR gövdesinde "MEDIA_BUNDLE politikası: Reliable (önce: <neydi>)".

#### KART K4G-09 — DUR #108 çözümü: `0x0B FILE_RECEIPT`, zamanlayıcılar, gönderen-tek `FILE_ABORT`
- **Sahip:** Abkarada · **İcra:** Claude Code L-APP (3/4) · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 2
- **Bağımlı:** K4G-08 · **Bloklar:** K4G-14
- **DAYANAK:** Durum raporu §3 DUR #108: "FILE_ABORT'ta yön alanı yok, bu yüzden alıcının iptali eşe bildirilemiyor. Dosya düzeyinde alındı ve yarım aktarım için zamanlayıcı da yok." §3 tablo: "Zamanlayıcı yok; takılan aktarım yuvayı tutuyor. Supersession'da yeniden gönderim yok." Vektör kuralı. Karar D6.
- **NE:**
  1. **Önce Python referansı** (`vectors/` altındaki AMP üreticisi): yeni tip `0x0B FILE_RECEIPT`, gövde sabit genişlik: `transfer_id` (FILE_BEGIN'deki `transfer_id` ile **aynı genişlik ve kodlama**) + `status u8` ∈ {`0x00 ok`, `0x01 aborted_by_receiver`, `0x02 hash_mismatch`, `0x03 timeout`, `0x04 rejected`}. Vektörler: 5 pozitif (her status) + 3 ret (bilinmeyen status, kısa gövde, uzun gövde). AMP tip kayıt tablosuna 0x0B eklenir; A2 sınır tablosuna gövde uzunluğu eklenir (üstü çiz/ekle + YAMA KAYDI).
  2. Rust codec 0x0B; vektörler tüketilir.
  3. FileTransfer: alıcı END + hash doğru → `FILE_RECEIPT(ok)`; hash yanlış → `FILE_RECEIPT(hash_mismatch)` (bugünkü alıcı tarafı `FILE_ABORT(hash_mismatch)` **kaldırılır**, çünkü FILE_ABORT artık gönderen-tek); kullanıcı iptali → `aborted_by_receiver`; yuva dolu / 64 MiB üstü → `rejected`; `FILE_IDLE_TTL = 60 s` CHUNK gelmezse → `timeout` + yuva boşalır. Gönderen END sonrası `FILE_RECEIPT_TTL = 60 s` içinde alındı yoksa `Failed(ReceiptTimeout)`; alındı `ok` → `Delivered`. Oturum değişiminde yarım aktarım: gönderen `FILE_ABORT` (mevcut neden kodları yetmiyorsa Python referansına `superseded` eklenir) ve `Failed(Superseded)`; UI "yeniden gönder" sunar (yeni `transfer_id`).
  4. `FILE_BEGIN` kabul edilmeden yapılan iptal artık sessiz düşmez: `FILE_RECEIPT(rejected)`.
  5. Zamanlayıcılar enjekte edilen saat üzerinden (`MonotonicClock`); testler sahte saatle: idle timeout, receipt timeout, hash mismatch yolu, rejected yolu, supersession abort.
- **NEDEN:** Alıcı iptali bildirilemiyor; takılan aktarım yuva tutuyor; gönderen "başarılı mı" bilmiyor — demoda fotoğraf "gönderiliyor"da kalır.
- **KAZANÇ:** Fotoğraf akışının üç ucu (başarı, iptal, zaman aşımı) kapanır; FILE_ABORT düzeni değişmediği için mevcut vektörler bozulmaz.
- **TRADE-OFF:** Yeni tip = 8 yeni vektör + spec yaması.
- **SINIR:** Supersession'da kaldığı yerden devam yok (tam yeniden gönderim); dosya bellekte kalır (64 MiB tavanı; PoC fotoğrafı ≈150 KB).
- **GİRDİ:** AMP spec, Python üretici, `crates/kama-app` FileTransfer + codec.
- **DONE:** 8 vektör Python'dan üretilmiş ve Rust'ta tüketiliyor; 5 test yeşil; AMP tip tablosu + A2 + §3 yamalı; word_coverage yeşil; #108 DUR kapatıldı.

#### KART K4G-10 — `ClientTransport`'ta gerçek `BestEffort` / `DeadlineBounded` — **ERTELENDİ (Ek 2026-10-12 (4), DUR-K4G-10 ikinci cevap)**

> **Karar:** Seçenek **B**. Şeridin bulgusu: motor (adaptive-transport) Reliable birimi hiç terk etmiyor ve alıcı `Reorder` boşluğu ancak 4096'da atlıyor; gerçek BestEffort için motor API'si (`mark_no_retransmit`, `retire_unit`) **ve** DATA telinde "sırasız" işareti gerekiyor — bu, FROZEN transport'un çerçeve biçimine dokunur ve interim 11 B çerçeve zaten BatchPayload'a geçişte emekli olacak (KATMAN 6). Bu sprintte **yapılmaz**; K4G-11'in bu karta bağımlılığı **kalkar**; D7 "ek 2026-10-12" ile bu sprint için askıya alındı (D7'nin PTT = Reliable yarısı aynen geçerli). KATMAN 6 kartı: "BestEffort/DeadlineBounded gerçek uygulaması = BatchPayload veri yolu geçişiyle **birlikte** (alt-mesaj başlığında teslim sınıfı; motor API'si; sırasız teslim), transport spec yaması Abkarada kararıyla." PoC sonucu (PoC-KAPSAM-001 EK-3): canlı arama medyası PoC'de kama transport'tan **geçmez**; medya motoru kendi UDP yolunda, oturumdan türetilen anahtarla; kama transport PoC'de yalnız Reliable taşır. INT-02 beyanı: "BestEffort/DeadlineBounded Reliable gibi taşınır; canlı medya transport üzerinden sınanmadı." Aşağıdaki kart metni **tarihsel** olarak kalır.
- **Sahip:** Abkarada · **İcra:** Claude Code L-TR · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 2
- **Bağımlı:** — · **Bloklar:** K4G-05 NON-GATING ek koşu, K4G-15
- **DAYANAK:** Durum raporu §3 Ptt satırı: "Gerçek adaptör BestEffort'u Reliable gibi taşıyor (deadline yok, ARQ devrede)." PoC-KAPSAM-001 §2 "Sesli / görüntülü arama" satırı ("Medya `BestEffort` / `DeadlineBounded` ile gider, ARQ yok, geç gelen düşer"). Karar D7 (+ ek 2026-10-11).
- **NE:**
  0. **(ek 2026-10-11) Kapsam:** Bu kart yalnız **canlı sesli/görüntülü arama medyasının** taşıma politikasını gerçek yapar. **PTT bu politikayı kullanmaz** (PTT = Reliable klip, K4G-08 §0). Bu kart Ptt motoruna ve AMP'ye **dokunmaz**.
  1. Önce `crates/kama-contracts/src/s1.rs`'te `DeliveryPolicy` okunur. `BestEffort` ve `DeadlineBounded` tanımlı değilse **DUR** (S1 FROZEN).
  2. `crates/client-core/src/transport.rs`: BestEffort birim ARQ deposuna **girmez**, tek datagram, PN tüketir, ACK beklenmez (gelen ACK yok sayılır). DeadlineBounded(d): ARQ deposuna girer; `now ≥ enqueue + d` olduğunda depodan çıkar ve bir daha gönderilmez. ~~`S1 Event::Expired { unit_id }` üretilir~~ **DUR-K4G-10 CEVAP (Ek (3)):** S1 §4 K8 ("süresi dolan birim sessizce düşer; `DeadlineExpired` yalnız `send()` anında; sonradan sinyal yoktur", `KM-SEAM-AT-001.md:141`) **geçerlidir, S1 değişmez**: süresi dolan birim **sessizce** düşer, olay yok; transport iç sayacı `deadline_expired_drops` artar (K4G-11 `metrics()` bunu açar); `send()` anında deadline zaten geçmişse mevcut `DeadlineExpired` dönüşü. Medya motoru kaybı kendi geri bildirimiyle öğrenir.
  3. Alıcı tarafında değişiklik yok (geç gelen datagramın "çöp" olduğuna medya motoru karar verir).
  4. S1 real conformance'a satırlar: BestEffort → ARQ deposu boyutu 0; DeadlineBounded → deadline sonrası depo 0, **olay yok**, `deadline_expired_drops == 1`; Reliable davranışı değişmedi (regresyon).
- **NEDEN:** Canlı arama medyası "geç gelen çöptür" ilkesiyle çalışır; Reliable gibi taşımak düşük bantta kuyruğu şişirir ve gecikmeyi büyütür. (PTT'nin ilkesi tersidir: "ne olursa olsun gitsin, gecikme önemli değil" → Reliable.)
- **KAZANÇ:** Medya motoru (Resul–Hasan) gerçek politikayla entegre olur; K8 sesli/görüntülü hedefleri ölçülebilir hale gelir.
- **TRADE-OFF:** BestEffort kayıpları artık görünür (zaten istenen).
- **SINIR:** FEC yok (ARQ-only kararı); pacing değişmez; PTT ve AMP'ye dokunulmaz.
- **GİRDİ:** `s1.rs`, `transport.rs`, S1 conformance harness.
- **DONE:** 3 conformance satırı mock + real yeşil; `seam-coverage.json` güncel; PR gövdesinde S1'in değişmediği beyanı.

### Gün 3 — Bileşim kökü, provisioning, S4 taslağı, sinyalleşme API'si

#### KART K4G-11 — Bileşim kökü `crates/kama-node`: gerçek anahtar + saat + entropi + UDP + disk
- **Sahip:** Abkarada · **İcra:** Claude Code L-NODE · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 3
- **Bağımlı:** K4G-02, K4G-06, ~~K4G-10~~ (Ek (4): K4G-10 ertelendi, bağımlılık kalktı; `metrics()`'te `deadline_expired_drops` **yok**), K4G-12 (biçim). **K4G-14'e bağımlı değildir**; K4G-14 açık kalması bu kartı ve CORE-CLOSE'u bloklamaz · **Bloklar:** K4G-05 (INT-02 bu kökü kullanır), K4G-14b, K4G-15
- **Ek 2026-10-12 (5) — DUR-K4G-11 CEVAP: zarf taşıyıcısı = Seçenek B.** Üründe `EnvelopeLink` yok (yalnız test-util `MemoryLink`/`MemoryHub`, `client-core/src/memory_link.rs:178`); direct-mode SSI1/SSR1 UDP çerçevesi spec'te yok (S4 taslağı yalnız hub Plane 1). Karar: `kama-node` içinde **süreç-içi zarf rölesi** (`LoopbackEnvelopeRelay`; ürün taşıyıcısı **değildir**, belge satırı: "iki cihaz arası taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti"). CORE testi ve INT-02 iki düğümü **tek süreçte** koşar: veri datagramları UDP loopback + netem'den, el sıkışma zarfları süreç-içi röleden **netem dışında** geçer (INT-02 beyanlı boşluk). (A) `DirectUdpLink` reddedildi: yeni tel (spec + vektör) ve INIT retry (T3) olmadan %30 kayıpta bağlanma kurulamayabilir; (C) reddedildi: bağlam-önce dağıtımı deler. Sonuç (K4G-15 README'ye yazılır): bu sprintin `.so`'su API entegrasyonu ve süreç-içi loopback içindir; **iki telefon arası oturum hub sprintiyle gelir**. **#125 beyanlı sapmaları KABUL:** `ptt_open(peer_id, codec, bundle_ms)` (değer uydurulmaz); dosya yolu `files/<eş>-<transfer_id>`; `KamaNode::start` tokio bağlamından çağrılamaz (`block_on`; K4G-05 testleri `spawn_blocking`/ayrı iş parçacığı, K4G-15'te JVM iş parçacığı zaten async değil — README'ye); `rtt_ms`/`loss_pct`/`bitrate_kbps` **şimdilik `None`** (client-core'da salt-okunur kaynak yok) — K8 overlay'i demoda bunları ister → KATMAN 6 kartı "motor sRTT/kayıp/bitrate salt-okunur erişimcileri + overlay, hub sprintinde, demo öncesi ZORUNLU"; `delivery_channel_drops` = `ServerStats::delivery_dropped` canlı aynası kabul.
- **Ek (4) brif şartları (orkestratörün K4G-08/09 bulgularından):** API tutamaçları **düğüm-tekil** (stream/transfer için çıplak tel `stream_id`/`transfer_id` API tutamacı **olmaz**; use-after-close'da kimliğin başka eşe gitmesi imkânsız olmalı); periyodik `poll` (PTT-R4 ve dosya zamanlayıcıları için); iki saat ayrı enjekte edilir (`kama_noise_wire` `MonotonicClock` + `kama_app::clock`). `drain_background()` Ek (2)'deki gibi.
- **DAYANAK:** Durum raporu §4: "Ürün tarafında kriptoyu kuran bir composition-root, kimlik/anahtar deposu ve ağ taşıyıcısı yok. `NoiseWireProtection::with_control`'ü test dışında yalnız test-util fikstürü çağırıyor (`seam_mock.rs:640`)". Rapor §3: "86 testin hepsi sahte Transport'larla koşuyor".
- **NE:**
  1. Yeni crate `crates/kama-node` (lib): `KamaNode::start(config_path: &Path, data_dir: &Path, listener: Box<dyn NodeListener>) -> Result<KamaNode>`. İçinde: provisioning JSON'unu (K4G-12 biçimi, `device-<ad>.json`) okur ve **kimlik, anahtarlar, eş dizinini bellekte tutar** (depoya yazmaz, D8); `RedbStore` açar (K4G-06; `AppStore` + `CounterStore` olarak aynı nesne); gerçek `EntropySource` (OS RNG — mevcut üretim gerçeklemesi hangisiyse o; yoksa `getrandom` zaten bağımlılıkta mı kontrol edilir, değilse DUR); gerçek `MonotonicClock`; `NoiseWireProtection::with_control(...)` gerçek anahtarlarla; `ClientTransport` gerçek UDP soketiyle (mevcut client-core ağ altyapısı; **yeni async runtime eklenmez**, D11); `kama-app` Messenger/FileTransfer/Ptt motorları (Signaling motoru **K4G-14 ile gelir**, bu kartta yok). Alınan dosya `<data_dir>/files/<transfer_id>` yoluna yazılır; dizin tutulmaz, yol olayla verilir.
  2. API (K4G-15 bunu birebir dışa açar): `connect(peer_id)`, `cancel_connect(peer_id)`, `send_text(peer_id, text) -> msg_id`, `send_file(peer_id, path) -> transfer_id`, `cancel_file(transfer_id)`, `ptt_open(peer_id) -> stream_id`, `ptt_push(stream_id, bytes)`, `ptt_close(stream_id)`, `on_platform_signal(signal)`, `drain_background()`, `metrics() -> NodeMetrics {rtt_ms, loss_pct, bitrate_kbps, epoch, last_heartbeat_age_s (PoC'de None), ctx_mismatch_drops, hub_frames_dropped, connect_state per peer}`, `shutdown()`. Olaylar `NodeListener::on_event(NodeEvent)`: ConnectProgress/Connected/Closed(reason), TextReceived, Receipt(msg_id), FileProgress/FileReceived(path)/FileFailed(reason), PttOpened/PttBundle/PttClosed, Metrics(tick). `send_signaling(peer_id, bytes)` ve `SignalingReceived` olayı **K4G-14'ün `KamaNode` uzantısıdır**; CORE-CLOSE için gerekli değildir. Mesaj geçmişi **tutulmaz** (olay akışı yeter; D8). **`drain_background()` anlamı (ek 2026-10-11, PoC-KAPSAM-001 EK-2):** yerel outbox'taki bekleyen birimleri ve transport'un yeniden gönderim kuyruğunu **yeniden pompalar** ve gerekirse eş başına `connect` tetikler; **hub mailbox PoC'de yoktur**, bu çağrı hiçbir şey "çekmez".
  3. `layer_check.py`: `kama-node` **bileşim kökü** sınıfı — INV-NEIGHBOR-1'in tek istisnası; yalnız kurma (wiring) kodu içerir, protokol mantığı içermez (CI'da `kama-node` içinde `kama_noise_wire::` çağrısı yalnız yapıcı düzeyinde olmalı; basit grep kuralı yeterli).
  4. CORE testi `crates/kama-node/tests/two_nodes_loopback.rs`: `kama-provision` ile üretilmiş 2 cihaz config'i (test fikstürü **değil**), iki `KamaNode` aynı süreçte loopback UDP; TEXT ↔ RECEIPT, 1 dosya (FILE_RECEIPT(ok) dahil), 1 PTT akışı; sonra iki düğüm de kapatılıp aynı `data_dir` ile yeniden açılır (restart persistence): outbox'taki mesaj teslim olur, `msg_id` ve `init_counter` artmış. **Signaling blob bu testte yok** (K4G-14'ün kendi testi).
- **NEDEN:** Bugün ürünü ayağa kaldıran tek yol test fikstürü; Hasan'ın bağlanacağı gerçek bir motor yok.
- **KAZANÇ:** Gerçek dünyayla ilk temas tek yerde; UniFFI yalnız ince sarmalayıcı olur.
- **TRADE-OFF:** —
- **SINIR:** Hub yok (direct-mode: config'teki eş adresine doğrudan UDP); heartbeat yok (D18); UI yok.
- **GİRDİ:** K4G-06, K4G-10, K4G-12 çıktıları; `seam_mock.rs:640` fikstürü (örnek olarak).
- **DONE:** `two_nodes_loopback.rs` (TEXT/RECEIPT + dosya + PTT + restart persistence) yeşil; `layer_check.py` yeşil; `cargo deny` yeşil; PR gövdesinde API listesi. Signaling **DONE'un parçası değildir**.

#### KART K4G-12 — `kama-provision` CLI ve PROVISIONING-FORMAT-v0
- **Sahip:** Abkarada · **İcra:** Claude Code L-PROV · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 3 (biçim Gün 2 sonunda hazır olmalı)
- **Bağımlı:** — · **Bloklar:** K4G-11, K4G-05 (anahtar kaynağı)
- **DAYANAK:** PoC-KAPSAM-001 §2 "Provisioning" ve "Anahtarlar"; Durum raporu §4 "anahtarların kaynağı yazılmalı, çünkü bugün tek kurulum yolu test fikstürü"; ccr §35.4 kapalı-sistem kipi (PR #101). Kararlar D4, D9, D11.
- **NE:**
  1. `docs/provisioning/PROVISIONING-FORMAT-v0.md`: `root.json` {test kökü Ed25519 açık/gizli, oluşturma zamanı} — **provisioning-otoritesi eseri, cihaza verilmez (D9)**; cihaz başına `device-<name>.json` {`device_id`, `identity_ed25519 {priv, pub}` (Ek (3): kimlik anahtarı, cihaz anahtarından ayrı), `identity_hash` (hex), `dest_hash` (hex), `x25519_static {priv, pub}`, `ed25519 {priv, pub}` (cihaz), `cert` (CompactDeviceCert, base64, test köküyle imzalı), `root_pub` (yalnız açık anahtar), `peers[] {name, identity_hash, dest_hash, x25519_pub, cert}`, `hub {addr, x25519_pub}` (PoC'de boş olabilir), `direct_addrs {name: "ip:port"}` (direct-mode için), `format_version: 0`}. Belgeye iki kural birebir: (a) D4: "Her `kama-provision init` koşusu yeni anahtar üretir; yeniden kurulum = yeniden provisioning = yeni kimlik. Kalıcı durum kaybından sonra aynı eski kimliğin yeniden kullanılması desteklenmez." (b) D9: "`root.json` cihaz config'ine, Git'e, release paketine ve `.so` zip'ine girmez."
  2. `crates/kama-provision` (bin): `kama-provision init --out <dir> --devices alice,bob[,…] [--hub host:port] [--direct alice=ip:port,bob=ip:port]`. Argümanlar `std::env::args_os` ile ayrıştırılır (clap yok). JSON `serde_json` (D11). ~~CompactDeviceCert mevcut kodlayıcıyla üretilir~~ **DUR-K4G-12 CEVAP (Ek (3)):** Rust'ta CompactDeviceCert kodlayıcısı ve `dest_hash` türetme fonksiyonu **yoktur** (handshake sertifikayı opak 152 B taşır); ikisi `kama_provision::cert` modülünde ccr §13.1/§13.2'den yazılır ve şu iki karar geçerlidir:
     - **(a) `dest_hash` kimlik anahtarından türetilir, cihaz anahtarından değil.** `kama-provision` cihaz başına, cihaz Ed25519/X25519 çiftinden **ayrı** bir **kimlik Ed25519 çifti** üretir (`identity_ed25519 {priv, pub}`; PoC'de bir kullanıcı = bir cihaz; `priv` çalışma zamanında kullanılmaz, SIGNED_CONTROL/attestation PoC dışı). `identity_hash = SHA-256(identity_pk_ed25519)`, `dest_hash = SHA-256(identity_hash)` (identity-and-trust-model formülü; ccr §13.3 "YANLIŞ: identity_hash = SHA-256(device_ed25519_pub)" kuralı **ihlal edilmez**). Şeridin önerdiği `SHA-256(SHA-256(device_ed25519_pub))` **reddedildi** (spec'in açıkça yasakladığı formül koda girmez). CompactDeviceCert düzeni ccr §13.1 ile aynı kalır (cihaz anahtarları; kimlik alanı eklenmez); kimlik↔cihaz bağı PoC'de provisioning dosyasındadır (dizin = güven çapası, PoC-KAPSAM-001 §2).
     - **(b) `domain_sep("SR_CDC_V1")` = 9 B ASCII + 7 × `0x00` dolgu (16 B)** kabul (Noise protokol-adı dolgusuyla aynı gelenek). Bu, SPEC-GAP S4-pki-discovery.6'yı kapatan **spec kararıdır**: ccr §13.2'ye YAMA KAYDI ("16 B etiket = ASCII etiket ≤ 16 B + `0x00` dolgu; `SR_CDC_V1` için bayt dizisi `53525f4344435f5631` + `00`×7") K4G-12 PR'ında yazılır (üstü çiz, silme; word_coverage).
     - **(c) Vektör kuralı geçerlidir:** `vectors/` altına Python (stdlib-only) referansı + 3 altın vektör: `identity_hash`/`dest_hash` (sabit kimlik açık anahtarı için), `domain_sep` + TBS baytları (sabit girdiler), tam CompactDeviceCert (repoda saf-Python Ed25519 referansı varsa imzalı; yoksa TBS'ye kadar ve imza doğrulaması Rust `verify_strict` ile). Rust testindeki "bağımsız yeniden hesap" bu vektörleri tüketir.
     - **(d) D11 açıklaması:** `ed25519-dalek`, `x25519-dalek`, `sha2`, `zeroize` zaten `Cargo.lock`'ta aynı sürümle kilitli; **yeni paket yok** → D11 ihlali değildir, onaylıdır.
     Çıktı dizinine `.gitignore` (`*`) yazılır; tüm dosyalar `0600` izinle oluşturulur (Unix; Windows'ta atlanır ve uyarı basılır).
  3. Test: `init` 3 cihaz üretir; her cihazın `peers` listesi diğer ikisini içerir; `dest_hash` mevcut türetme fonksiyonuyla doğrulanır; `cert` `verify_strict` ile test köküne karşı geçer; iki koşunun çıktıları farklı anahtarlar üretir; **hiçbir `device-*.json` kök gizli anahtarını içermez** (bayt araması); `.gitignore` ve `0600` mevcut.
- **NEDEN:** Sahada telefonlara gerçek anahtar dağıtmanın tek yolu bu; güvenlik iddiası ("listede olmayan oturum açamaz") bu listeye dayanır.
- **KAZANÇ:** INT-02 ve K4G-11 test fikstürü yerine gerçek üretim yolunu kullanır.
- **TRADE-OFF:** Cihaz gizli anahtarları düz JSON'da (PoC; `noBackupFilesDir`, FLAG_SECURE; at-rest şifreleme dışarıda) — kabul edilmiş risk, büyütülmez. Kök gizli anahtarı ise **yalnız laptopta** (D9).
- **SINIR:** QR, enrollment, PKI, iptal yok.
- **GİRDİ:** CompactDeviceCert kodlayıcı, `dest_hash` türetme, `verify_strict`.
- **DONE:** CLI 3 cihaz üretiyor; testler (kök-gizli-anahtar-yok testi dahil) yeşil; biçim belgesi main'de; `cargo deny` yeşil.

#### KART K4G-13 — S4 Transport↔Hub dikişi: `KM-SEAM-TH-001` arayüz taslak notu (**NON-GATING**, belge-yalnız) (v1.1)
- **Sahip:** Abkarada · **İcra:** Claude Code L-S4 (yalnız başka şerit boşta kalırsa; Gün 3–4) · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / NON-GATING
- **Bağımlı:** — · **Bloklar:** hiçbir CORE-CLOSE / MOBILE-HANDOFF kartını bloklamaz. Yapılmazsa hub sprintinin ilk kartı olur.
- **DAYANAK:** PoC-KAPSAM-001 §2 "Sözleşmeler", "Tek hub", "Süreç ömrü"; K4, K5. Spec'ler: `ccr-format-spec.md` §35.16 (LINK_ROUTE_MISS → SESSION_INIT_REF; "client keepalive YOK" — bu cümle DR-CONNECT-1 heartbeat kararıyla **çelişir**, yama hub sprintinde), `application-protocol-and-rpc-spec.md` §5 CLIENT_BIND, §6.8 SESSION_REBIND {link_ref bytes16; reason u8} → {status; new_handle}, §6.11.1 bounded rebind; `transport-layer-tdd.md` 84–96 (hub_handle "NAT-robust… TLS connection_id taşımaz"); DR-CONNECT-1 (heartbeat 0x1B/0x1C, 8 s/25 s, K=1024 → SESSION_REBIND). Karar D18 (v1.1).
- **NE:** Tek belge `docs/seams/KM-SEAM-TH-001-TASLAK.md`, **en çok 2 sayfa**, Durum: **ARAYÜZ TASLAĞI — G-S4 öncesi, davranış kararı içermez**. İçerik yalnız: (a) mesaj listesi ve yönü — CLIENT_BIND, SESSION_REBIND, LINK_ROUTE_MISS, HEARTBEAT 0x1B / HEARTBEAT_ACK 0x1C (rezerve), röle datagramı (hub_handle + opak yük; hub **hiçbir şeyi açmaz**); (b) **zaten karar verilmiş** kurallar, kaynağıyla: K5 (eş kimliği `hub_handle`; dönüş adresi = son bind/INIT adresi; veri datagramıyla güncellenmez; bilinmeyen handle sessiz drop; heartbeat slot yaratmaz), K4 (LinkChanged → SESSION_INIT_REF + early_unit), heartbeat kadansı 8 s / 25 s (DR-CONNECT-1), K=1024 → rebind (DR-CONNECT-1); (c) **AÇIK** başlığı altında, karar verilmeden: kaçırılan heartbeat eşiği ve `LinkSuspect`/`LinkLost` geçişleri, `hub_handle` TTL, handle başına kuyruk/hız, hata satırları (mailbox **AÇIK değildir**: PoC'de hub store-and-forward yok, PoC-KAPSAM-001 EK-2); (d) hub sprintinde yamalanacak spec cümleleri listesi: ccr §35.16 "client keepalive YOK"; transport-tdd 84–96 "NAT-robust". **`crates/kama-contracts/src/s4.rs` yazılmaz** (tipler G-S4 ile gelir).
- **NEDEN:** Hub sprinti (Yaaz) bir giriş noktası ister; ama bu sprintin amacı yeni dikiş tasarlamak değil, Application + Kripto'yu gerçek yolda kapatmak.
- **KAZANÇ:** Hub sprintine kararları doğru kaynağa bağlayan bir başlangıç notu; karar verilmemiş hiçbir şey "karar" gibi görünmez.
- **TRADE-OFF:** Hub sprinti S4'ü FROZEN'a kendisi götürür (ilk 1–2 günü).
- **SINIR:** Kod yok; tip yok; davranış parametresi yok; S1/S3 değişmez; spec yaması yapılmaz.
- **GİRDİ:** Yukarıdaki spec bölümleri, DR-CONNECT-1 belgesi, PoC-KAPSAM-001 K4/K5.
- **DONE:** Belge main'de **veya** gün raporunda "K4G-13 yapılmadı → hub sprinti" satırı. İkisi de geçerli kapanıştır.

#### KART K4G-14 — `SIGNALING_BLOB` gönder/al API'si (**MOBILE-HANDOFF**; `KamaNode` uzantısı)
- **Sahip:** Abkarada · **İcra:** Claude Code L-APP (4/4) · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 3
- **Bağımlı (Ek 2026-10-12 (4), ikiye bölündü):** **K4G-14a** = kama-app `Signaling` motoru (aşağıdaki NE (1)): yalnız K4G-09'a bağımlı, **K4G-11'i beklemeden başlar** (L-APP şeridi boşa çıkınca). **K4G-14b** = `KamaNode` uzantısı (NE (2)): K4G-11'e bağımlı. İkisi ayrı PR. · **Bloklar:** K4G-15. **CORE-CLOSE'u bloklamaz.**
- **Ek 2026-10-12 (5) — DUR-K4G-14a CEVAP:** alındısız `SIGNALING_BLOB`'un (0x09'da `msg_id` yok, RECEIPT yok) oturum değişiminde yeniden sunulması için tutma kuralı: **ilk kabulden 60 s** (= `FILE_RECEIPT_TTL`), **eş başına 256 kayıt** (= `MAX_OUTBOX_UNACKED`), taşmada en eski düşer. 120 s **reddedildi**: sinyalleşme zamana duyarlıdır (kurulum hedefi ≤ 1 s, çalma süresi ≤ 60 s); 60 s'den eski bir teklifi yeniden sunmak bayat çağrı üretir. AMP §3'teki "ÖNERİ" işareti bu değerlerle kesinleşir. Not (medya ekibine, K4G-15 README): sinyalleşme içeriği kendi `call_id`/sıra numarasını taşımalı; Kama 0x09 için dedup/alındı sağlamaz.
- **DAYANAK:** Durum raporu §3: "SIGNALING_BLOB: Codec düzeyinde var; alınan kayıt olduğu gibi geçiriliyor. `send_signaling`/`on_signaling` API'si yok". PoC-KAPSAM-001 §2 "Sesli / görüntülü arama", K7 (içerik protobuf, Rust için opak).
- **NE:** (1) `crates/kama-app` içinde `Signaling` motoru: `send(peer, bytes) -> Result<SignalingId>` (Reliable; boyut AMP A2'deki 0x09 sınırı; aşarsa `TooLarge`), gelen 0x09 → `Event::SignalingReceived { peer, bytes }`. İçerik **opak**; Rust'ta protobuf şeması yok (medya motoru sahibi). Dedup: `SIGNALING_BLOB` AMP'de `msg_id` taşıyorsa Messenger'ın dedup penceresi paylaşılır; taşımıyorsa dedup yok ve bu AMP §3'e yazılır. (2) `KamaNode` uzantısı: `send_signaling(peer_id, bytes)` çağrısı ve `NodeEvent::SignalingReceived` olayı K4G-11'in API'sine **bu kartta** eklenir. Testler: kama-app gidiş-dönüş; boyut aşımı; supersession'da yeniden gönderim (outbox üzerinden); `crates/kama-node/tests/signaling_loopback.rs` — iki düğüm loopback'te signaling blob gidiş-dönüş (K4G-11'in CORE testinden **ayrı**).
- **NEDEN:** Sesli/görüntülü arama kurulumu (≤ 1 s hedefi) bu kanaldan geçer; Resul–Hasan'ın medya motoru beklemede.
- **KAZANÇ:** Medya ekibi protobuf şemasını Rust'tan bağımsız evriltir.
- **TRADE-OFF:** Rust şemayı doğrulamaz (bilinçli).
- **SINIR:** Protobuf bağımlılığı Rust'a **eklenmez** (D11).
- **GİRDİ:** AMP 0x09 tanımı, Messenger outbox, K4G-11 `KamaNode`.
- **DONE:** kama-app testleri ve `signaling_loopback.rs` yeşil; `KamaNode` uzantısı merged; AMP §3 dedup cümlesi. Açık kalırsa gün raporunda MOBILE-HANDOFF satırına yazılır.

### Gün 4 — UniFFI teslimi, kapanış

#### KART K4G-15 — UniFFI yüzeyi `crates/kama-mobile` → Hasan'a teslim (**MOBILE-HANDOFF kapısı**, v1.1)
- **Sahip:** Abkarada · **İcra:** Claude Code L-FFI · **İnceleyen:** Abkarada (Hasan derleyip geri bildirir) · **Sprint:** 4GÜN / Gün 4
- **Bağımlı:** K4G-11, K4G-14 · **Bloklar:** UI sprinti. **CORE-CLOSE'u bloklamaz (D10):** yetişmezse rapor "CORE-CLOSE kapandı, MOBILE-HANDOFF açık" der ve kart ertesi güne taşınır.
- **DAYANAK:** PoC-KAPSAM-001 §2 "Sözleşmeler" (S5 = `ui-api v0.3`, Rust karşılığı UniFFI yüzeyi), K6 (facade değişmez, dar implemente edilir), §2 "Android" satırı. Hasan'ın `NOT-UI-ekibi_ui-api-v0.3.md` (RealKamaEngine via UniFFI → StateFlow, tek dispatcher, ConnectionMapper) ve `s06-kotlin-kutuphaneler.md` (minSdk 28, cargo-ndk, 16 KB hizalama). Karar D10, D11.
- **NE:**
  1. Yeni crate `crates/kama-mobile` (`crate-type = ["cdylib"]`), `uniffi` ile (proc-macro veya UDL; hangisi workspace için daha az sürtünmeliyse, PR'da gerekçeli). Dışa açılan nesne `KamaMobile` = `KamaNode` API'sinin birebir sarmalayıcısı (K4G-11 §2 listesi); olaylar UniFFI **callback interface** `EngineListener { fn on_event(ev: EngineEvent) }`; tüm çağrılar non-blocking (uzun işler düğümün kendi iş parçacığında).
  2. `docs/seams/KM-SEAM-AK-001.md` (S5 Rust karşılığı; Durum: PoC): `ui-api v0.3`'teki **her** çağrı/alan için tek satır — Rust karşılığı (`KamaMobile.xxx`) **veya** "PoC'de etkisiz gövde (K6)" **veya** "Kotlin tarafı (Hasan)". K6 listesi birebir: Enrollment → provisioning içe aktar → `Ready`; Contacts = provisioning listesi, `syncDirectory`/`lookupUsername` etkisiz; Devices tek satır; Push, `retryAfterTrustFailure`, Gate-3 bildirimi etkisiz gövde; `background.drain()` = `drain_background()`; `notifications.state`, `attachments.open`, `onPlatformSignal`, `CallsApi` kullanılır (`CallsApi` sinyalleşmesi `send_signaling` üzerinden, medya Kotlin/medya motorunda).
  3. Derleme: `cargo ndk -t arm64-v8a -o <out> build --release` ile `libkama_mobile.so`; `RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"` (16 KB hizalama); Kotlin bağlayıcıları `uniffi-bindgen` ile üretilir. Çıktı paketi `kama-mobile-<kısa sha>.zip` = `.so` + üretilmiş `.kt` + `KM-SEAM-AK-001.md` + `PROVISIONING-FORMAT-v0.md` + örnek ~~`device-alice.json`~~ `device-alice.json` + `device-bob.json` [YAMA KAYDI 2026-10-16 — DUR-K4G-15-B CEVAP, sahip] (**örnek amaçlı, `kama-provision` ile o an üretilmiş, `root.json` asla pakete girmez — D9**) + 20 satırlık `README-HASAN.md` (nasıl eklenir, `KamaMobile.start(configPath, dataDir, listener)` çağrısı, olay akışı). **Ek (5) README zorunlu satırları:** (1) "Bu sürümde iki telefon arası oturum YOK — el sıkışma zarfı taşıyıcısı hub Plane 1 rölesidir (hub sprinti); bu `.so` API entegrasyonu ve süreç-içi loopback (iki `KamaNode` aynı süreçte) içindir"; (2) "`start()` async/tokio bağlamından çağrılmaz; JVM iş parçacığından çağrılır"; (3) "`metrics()`'te `rtt_ms`/`loss_pct`/`bitrate_kbps` bu sürümde `None`; overlay alanları hub sprintinde dolar"; (4) "Sinyalleşme (`sendSignaling`) alındısız ve dedupsuz: içerik kendi `call_id`/sıra numarasını taşımalı; oturum değişiminde 60 s içindeki bloblar yeniden sunulabilir"; (5) "`pttOpen(peer, codec, bundleMs)` — codec ve demet süresi çağıran verir". GitHub release `poc-uniffi-v0` olarak yayımlanır.
  4. Test: Rust tarafında `kama-mobile` için smoke test (start → connect → send_text → event); Android cihaz testi **Hasan'da** (bu kartın DONE'u Rust teslimidir).
- **NEDEN:** Hasan'ın `RealKamaEngine`'i bağlanacak gerçek bir `.so` bekliyor; UI sprinti bununla başlar.
- **KAZANÇ:** S5 iki yönde de yazılı; Hasan `FakeKamaEngine`'den `RealKamaEngine`'e tek değişiklikle geçer.
- **TRADE-OFF:** UniFFI yeni bağımlılık (onaylı D11); callback interface gecikmesi (ms düzeyi, kabul).
- **SINIR:** Kotlin kodu yazılmaz (üretilen bağlayıcılar hariç); Android servis katmanı Hasan'ın.
- **GİRDİ:** K4G-11 API, Hasan'ın iki belgesi, PoC-KAPSAM-001 K6.
- **DONE:** `.so` arm64 derleniyor; release yayımlı; `KM-SEAM-AK-001.md` tüm `ui-api v0.3` satırlarını kapsıyor (eksik satır = DONE değil); smoke test yeşil; gün raporunda release bağlantısı.

#### KART K4G-16 — Kapanış: belge hijyeni, BL, KATMAN 6, organizasyon kararları (+ NON-GATING hub-sim)
- **Sahip:** Abkarada · **İcra:** Claude Code L-DOC · **İnceleyen:** Abkarada · **Sprint:** 4GÜN / Gün 4
- **Bağımlı:** **tüm CORE-CLOSE kartlarının PR'ları** (01, 02, 03, 04, 05 raporu, 06, 07, 08, 09, 10, 11, 12). **K4G-13/14/15 açık olabilir**; durumları kapanış raporunda ayrıca kaydedilir, bu kartı bloklamaz · **Bloklar:** CORE-CLOSE
- **DAYANAK:** Durum raporu §5 "Kapanış ölçütü" ve "Belge hijyeni (tek PR)" listesi; §4 "Küçükler"; "Sahipten beklenenler" 5. madde. Kararlar D3, D10, D12, D13, D14, D15.
- **NE (tek PR, yalnız belge + yorum + CLAUDE.md):**
  1. BL: 15 karta DURUM satırı (T-00.1–4, A3.1–3, A4.1–4, C1.1/1.3/1.5/1.6) ve 5 kapı başlığına durum; B2.2/B2.3/B2.4 "KAPANDI"; BL:399 B2.1 sayıları; BL:172; G-D ve G-IMPL-S1 satırları "Geçti (waiver ile): G-D-WAIVER-001 / G-IMPL-S1-WAIVER-001" (D16). **KATMAN 6'ya ekle:** T3 INIT retry; INIT'e gömülü 0-RTT (`early_unit`); BatchPayload veri yolu geçişi; KS-DOS-1 eş başına provisional tavan (D3); zstd çözücü kararı (D14); uzun TEXT bölme (D14); LinkChanged → SESSION_INIT_REF (D15); heartbeat uygulaması + eşik/TTL kararları (D18); mesaj geçmişi kalıcılığı ve ek blob dizini (D8); S4 `KM-SEAM-TH-001` FROZEN + `s4.rs` (K4G-13 yapılmadıysa taslak dahil); `EntropySource` birleştirme (K4G-02 §5 yapılmadıysa); ADR-03 libsodium/dalek amendment; extended 26 B zarf için bağımsız Python vektörü (S10). **Ek (4) ile eklenenler:** BestEffort/DeadlineBounded gerçek uygulaması = BatchPayload geçişiyle birlikte (K4G-10 ertelemesi; motor API'si + sırasız teslim işareti; transport spec yaması); #114/#116 beyanlı üç sınır — (i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; INV-ACK-DELIVER-1'i ihlal edebilir → sayaç `delivery_channel_drops` K4G-11 `metrics()`'te, INT-02'de 0 olmalı), (ii) `Closed` sonrası ACK'li kuyruktaki birimlerin düşmesi, (iii) provisional `NotReady` düşüşlerinin Karn/PreConverge RTT kestirimini şişirmesi (INT-02 GATING (2) epoch geçişinde ölçülür); K4G-07 beyanlı istisna 4 — iptal→abandon okunmadan senkron `connect()→Ready(h)` yarışında erken metnin "h'ye gönderilmiş" sayılması (canlılık; yanlış Delivered yok; main'de de vardı); K4G-09 alındı kuyruğu `MAX_PENDING_RECEIPTS` kalibrasyonu ve eş başına tahliye. **Ek (5) ile eklenenler:** iki cihaz arası el sıkışma zarfı taşıyıcısı = hub Plane 1 rölesi (S4) + T3 INIT retry — **hub sprintinin ilk kartları** (bu olmadan iki telefon oturum kuramaz); motor sRTT / kayıp / bitrate salt-okunur erişimcileri + K8 overlay alanları (hub sprinti, **demo öncesi zorunlu**); `arq_identity_is_the_pn_not_the_interim_seq_field` aralıklı testi (kart dışı test düzeltmesiyle kapandıysa DURUM: KAPANDI yazılır). (`init_counter` kalıcılığı K4G-02 ile kapandığı için KATMAN 6'ya **yazılmaz**, DURUM: KAPANDI.)
  2. S1/S3 statü satırları: "SEAM-FROZEN HOLD" → "FROZEN (G-S1 2026-10-06 / G-S3 …)" (`KM-SEAM-AT-001.md:5, :25`; `KM-SEAM-TC-001.md:20`).
  3. AMP: başlık "Sahip: Hayri" → "Sahip: Abkarada (icra Claude Code) — K4G-16 merge'i <SHA> sonrası ilk iş gününde Hayri'ye devredilir" (D13 v1.1); statü; §3/§8 durum hücreleri; 0x0B satırı (K4G-09'dan) tutarlı.
  4. `docs/hayri-app.md` §4–§5 ve :55: taşıma (#92) ve devir notu, tarihli: "Devir Abkarada tarafından Hayri'ye sözlü iletildi ve kabul edildi (2026-10-08). WS-B, K4G-16 kapanış merge'i (<SHA>) sonrası ilk iş gününde Hayri'ye geçer." `DR-CONNECT-1.md:53` düzeltmesi.
  5. Kod yorumları: `contract.rs:27-28`; `seam-coverage.json` GAP-SEAM-REAL-1 ve H-NO-CRYPTO-EDGE/H-NO-PLANE1 notları; `kama-app/src/lib.rs:9-11`; `noise_conformance_core/main.rs:9-13`; `ci.yml:249` ("4/24"); `protector.rs:651-653` ("Tablo bu kapılarla sınırlıdır" → KS-DOS-1 beyanı, D3).
  6. CLAUDE.md: crate listesi (mevcut 16 üye + bu planın **merge olmuş** yenileri: `kama-store`, `kama-node`, `kama-provision`; `kama-mobile` **yalnız K4G-15 merge olduysa**; silinen crypto crate'i çıkar); miri satırı; §4 dest_hash kuralına PoC-KAPSAM-001 §5 beyan atfı.
  7. `PoC-KAPSAM-001.md` ve bu plan `docs/decisions/` ve `docs/plans/` altına konur; BL'ye bağlantılar: `PROVISIONING-FORMAT-v0.md`, `INT-02-RAPORU.md` (CORE); `KM-SEAM-AK-001.md` ve `KM-SEAM-TH-001-TASLAK.md` **yalnız gerçekten mevcutlarsa** — yoksa BL'ye "açık: K4G-15 / K4G-13" satırı.
  8. Git (D12): `git tag archive/feature-app-2026-10-04 origin/feature/app` → push tag → uzak dal silinir. `origin/feature/app`'e **yazılmaz**.
  9. ~~hubsim~~ **Yapılmaz** (v1.1'de çıkarıldı; hub sprintine örnek bırakma işi hub sprintinin kendisidir).
- **NEDEN:** Kapanış ölçütü "her kart DURUM satırıyla"; belgeler kodun gerisinde; organizasyon kararları yazılmazsa Hayri ve Yaaz yanlış yerden başlar.
- **KAZANÇ:** Hub/UI/DevOps sprinti temiz bir BL ve dikiş belgeleriyle başlar.
- **TRADE-OFF:** —
- **SINIR:** Spec içeriği değişmez (yalnız statü/başlık/durum); kod davranışı değişmez.
- **GİRDİ:** Rapor §5 listesi, CORE-CLOSE kartlarının PR'ları; K4G-13/14/15'in o anki durumu.
- **DONE:** word_coverage yeşil; BL'de DURUM'suz kart 0; KATMAN 6 listesi yukarıdaki 13 kalemi içeriyor; tag atılmış, dal silinmiş; K4G-13/14/15 durumları BL'de ve raporda yazılı; gün 4 raporunda **iki tablo**: "CORE-CLOSE ÖLÇÜTLERİ" (D1 merged · D2 merged · kalıcılık merged · #105/#107/#108 kapandı · teslim politikaları merged · bileşim kökü merged · INT-02 GEÇTİ · INT-02 raporu · KAPSAM DIŞI listesi · C4 raporu · her kart DURUM) ve "MOBILE-HANDOFF" (K4G-14 · K4G-15 · release bağlantısı) — her satır VAR/YOK.

---

## 4. Kapılar, sıra, DUR ve rapor kuralları

### 4.1 Sıra ve iki çıkış
0. **BOOTSTRAP İSTİSNASI (ek 2026-10-11):** `K4G-00` yalnız onaylanmış plan ve kapsam belgelerini repoya koyan docs-only bootstrap PR'ıdır ve aşağıdaki K4G-01 merge kilidinin **dışındadır**. K4G-00 merge olur olmaz K4G-01 kilidi başlar.
1. Gün 1 (2026-10-12) sabah: K4G-01 **tek başına** merge olana kadar başka merge yok (ACİL DUR). Diğer Gün 1 kartları paralel başlar, K4G-01'den sonra merge edilir. **Ek (3):** K4G-01 merge sırası: #114 (1+2+3 ile) → K4G-03 ve K4G-02 (rebase + yeniden kapı) → K4G-01b → K4G-12 → sonra K4G-10 ve diğerleri. K4G-01b K4G-10'dan önce gelir (aynı dosyalar).
2. Gün 1 akşam raporu: "ACİL DUR kapandı", "G-IMPL-S3 paketi hazır", gecelik durumu.
3. Gün 2 (2026-10-13): Abkarada "G-IMPL-S3 GEÇTİ" + `sudo modprobe sch_netem` → K4G-05 başlar. Bu mesaj gelmezse K4G-06…10 yine ilerler; K4G-05 bekler ve raporda "bekliyor" yazar. INT-02 K4G-11 bileşim kökünü kullanır; K4G-11 hazır değilse Gün 3'e kayar.
4. Gün 3 (2026-10-14): K4G-11, K4G-12, K4G-14; INT-02 koşusu ve raporu.
5. Gün 4 (2026-10-15): INT-02 kapanışı, K4G-16, K4G-15.
6. **CORE-CLOSE** = §3 şerit haritasındaki CORE-CLOSE kartlarının hepsi merged **ve** "INT-02 GEÇTİ". "INT-02 GEÇTİ" gelmeden CORE-CLOSE **kapanmaz**; kapanmazsa Gün 4 raporu "CORE-CLOSE BEKLİYOR: INT-02" der, başka bir şey ertelenmez.
7. **MOBILE-HANDOFF** = K4G-14 + K4G-15 merged ve release yayımlı. Açık kalması CORE-CLOSE'u etkilemez (D10); rapor iki durumu ayrı satırda yazar.

### 4.2 Merge kuralları
- Her kart tek PR, squash merge, başlık `K4G-xx: …`, gövdede DONE maddeleri tek tek işaretli ve kanıt (test adı, koşu kimliği).
- `cargo test --workspace`, `cargo deny`, `layer_check.py`, `word_coverage.py`, `seam-api-drift` yeşil olmadan merge yok. Gecelik main'de kırmızıysa (K4G-03 öncesi) merge yapılır ama raporda yazılır.
- **CI-KİLİT İSTİSNASI (Ek 2026-10-11 (3); yalnız GitHub Actions faturalandırma kilidi sürdüğü sürece):** hosted CI koşamıyorsa, orkestratörün **yerel tam kapısı** (`ci.yml` PR adımları birebir, `k4g-gate.sh`, `summary.txt` PR gövdesine eklenir; `CARGO_BUILD_JOBS` serbest) hosted PR kontrollerinin yerine geçer ve merge yapılabilir. Koşullar: (i) kapı çıktısı PR'da; (ii) PR gövdesinde "CI-KİLİT İSTİSNASI ile merge" satırı; (iii) kilit kalkar kalkmaz `core-main` main'de elle tetiklenir, kırmızı çıkarsa **ACİL DUR** ve kırmızıyı üreten PR'lar geri alınır (revert PR). İstisna faturalandırma kilidiyle birlikte biter.
- **Haftalık branch-coverage (Ek (3)):** taban 82.0 **değişmez**. K4G-16'da yeniden ölçülür; hâlâ altındaysa Gün 4 raporunda kapsanmayan dalların listesi verilir, test mi/taban mı kararı Abkarada'nın.
- FROZEN dosyalar (`s1.rs`, `s3.rs`, `handles.rs`, `ui-api v0.3`) **değişmez**; `seam-api-drift` değişiklik gösteriyorsa PR kapatılır ve DUR.
- Vektör kuralı: her yeni/değişen vektör önce Python üreticisinde; PR'da üretici diff'i ve çıktı dosyası birlikte.
- Spec yaması: üstü çiz, silme; `YAMA KAYDI 2026-10-xx — K4G-xx — <bir cümle>`.

### 4.3 Gün raporu biçimi (≤ 40 satır, Türkçe)
```
GÜN <n> RAPORU — <tarih> — main <sha>
Kart | PR | Durum (MERGED / AÇIK / BEKLİYOR / DUR) | Beyanlı istisna
...
Kapılar: G-IMPL-S3 <bekliyor/geçti> · T5 <yok/var> · INT-02 <başlamadı/koşuyor/rapor hazır/geçti> · Gecelik <yeşil/kırmızı>
CORE-CLOSE: <açık/kapandı> — eksik: <kartlar>
MOBILE-HANDOFF: <açık/kapandı> — eksik: <kartlar>
DUR listesi: DUR-K4G-xx: <tek cümle soru> (yoksa "yok")
Yarın: <kartlar>
```

### 4.4 DUR tetikleyicileri (bunlarda sormadan ilerlenmez)
- FROZEN dosyaya dokunmak gerekiyor.
- D11 dışında bağımlılık gerekiyor.
- S1'de `BestEffort`/`DeadlineBounded` karşılığı yok (K4G-10). (`Expired` olayı Ek (3) ile kaldırıldı: S1 §4 K8 sessiz düşüş.)
- `SessionControl` yapıcı imzası S3'te tanımlı (K4G-02).
- kani koşusu kırmızı (K4G-04).
- Üretim `EntropySource` gerçeklemesi yok ve `getrandom` bağımlılıkta değil (K4G-11).
- Bu belgede olmayan bir karar gerekiyor (ör. yeni kayıt tipi, yeni sınır değeri).
- Güvenlik etkili yeni bulgu → **ACİL DUR**, K10.

### 4.5 Yapılmayacaklar (sprint içinde teklif de edilmez)
PoC-KAPSAM-001 §3 listesi aynen; ek olarak: hub sunucu kodu, hubsim, heartbeat uygulaması ve eşik/TTL kararları, `s4.rs`, LinkChanged akışı, Kotlin UI kodu, Android servis kodu, protobuf şeması, Lyra entegrasyonu, zstd, uzun TEXT bölme, RECEIPT(read), KS-DOS-1 tavanı, mesaj geçmişi kalıcılığı, blob dizini, kimlik/eş dizini depoda, ratchet, FEC, hop anahtarı, at-rest şifreleme, feature/app'e yazma.

---

## 5. Bu plandan sonra ne kalır (sonraki sprintin girdisi)

| Sahip | Kalem |
|---|---|
| Yaaz (hub + DevOps) | Hub sunucusu `KM-SEAM-TH-001`'e göre (G-S4 önce Abkarada'da FROZEN); heartbeat 0x1B/0x1C; K=1024 rebind; LinkChanged → SESSION_INIT_REF; VPS Docker deploy; APK/AAR hattı; CI (kararlar Abkarada). |
| Hasan (Android) | `RealKamaEngine` ↔ `KamaMobile`; ConnectionMapper; kalıcı FGS `specialUse` doğrulaması; ölçüm overlay'i; provisioning dosyası içe aktarma ekranı; `allowAudioPlaybackCapture=false`, FLAG_SECURE, VOICE_COMMUNICATION. |
| Resul–Hasan (medya) | Lyra + protobuf sinyalleşme `send_signaling` üzerinden; BestEffort/DeadlineBounded medya; görüntülü. |
| Hayri (Rust application, WS-B) | AMP bakımı; INIT retry kadansı (T3) uygulama tarafı; zstd çözücü kararı sonrası sıkıştırma; uzun TEXT bölme; RECEIPT(read) (PoC sonrası). |
| Abkarada (kripto/transport) | G-S4; KS-DOS-1 tavanı; INIT'e gömülü 0-RTT; BatchPayload veri yolu; extended zarf Python vektörü; ADR-03 amendment; M4 hop anahtarı (PoC sonrası). |
| Meriç | UI tasarımı (overlay dahil). |

Bu tablo, hub/UI/DevOps sprint kartlarının (aynı kart şablonuyla) yazılacağı listedir; kartlar "INT-02 GEÇTİ" sonrasında Abkarada ile birlikte yazılır.

---

## 6. Değişiklik kaydı v1.0 (2026-10-08) → v1.1 (2026-10-11)

Kaynak: Claude Code'un 2026-10-11 tarihli plan incelemesi. Kabul edilenler, gerekçeli reddedilenler ve değişmeyenler:

| Kalem | v1.0 | v1.1 | Gerekçe |
|---|---|---|---|
| D1 | tip → izin verilen bağlam tablosu | **bağlam-önce dağıtım**, birebir `Session(h)` eşleşmesi, HubChannel ayrı dağıtıcı (Session durumuna erişimsiz) | HubChannel yükü AEAD'siz → saldırgan tipi de seçer; tablo yanlış soyutlama, unutulan satır deliği yeniden açar |
| D2 | `load()/store()` | `reserve_next() -> Result<u64,_>` tek işlemli, commit-önce | Write-ahead garantisi deponun olmalı; hata dönemeyen `load` anlamsız |
| D4 | "yeniden kurulum = yeni kimlik" kuralı | + **beyanlı sınır**: durum kaybı + aynı kimlik desteklenmez; "çözüldü" denmez | Yerel tespit imkânsız; iddia sınırlanır |
| D8 | `AppStore` 7 işlem grubu (kimlik, eşler, geçmiş, blob dahil) | **asgari**: sayaçlar + dedup + outbox; `CounterStore` ayrı; kimlik/eşler bellekte (provisioning JSON); geçmiş + blob dizini KATMAN 6 | Application katmanı kimlik bilmemeli; bağımlılık grafı; 4 günlük bütçe |
| D9 | — | kök gizli anahtarı cihaza/Git'e/release'e girmez; `0600`; `.gitignore`; test | Otorite eseri ile cihaz paketi ayrımı |
| D10 | K4G-15 sprint kapanışının parçası | **CORE-CLOSE / MOBILE-HANDOFF** ayrı çıkışlar | Teknik kapanış ile Android teslimi ayrı sorular |
| D13 | sabit 2026-10-13 | K4G-16 merge'i sonrası ilk iş günü; SHA yazılır | Takvim 3 gün kaydı; sabit tarih sahiplik çatışması yaratıyordu |
| D16 | "PLAN-4GUN ONAYLANDI = geriye dönük kayıt onayı" | **WAIVER** belgeleri; geçmişte karşılanmamış ölçüt geçmiş sayılmaz | Kapı sisteminin anlamı |
| D18 | kod noktası + S4'te kadans/eşik taslağı | kod noktası rezerve; **kadans 8/25 s zaten karar (DR-CONNECT-1, değişmez)**; eşik/TTL/mailbox AÇIK → G-S4 | Claude Code bu kararı görmüyordu; yalnız eşikler yeniydi |
| K4G-13 | Gün 3, belge + `s4.rs` | **NON-GATING**, ≤ 2 sayfa arayüz notu, `s4.rs` yok; yapılmazsa hub sprintinin ilk kartı | Kapsam genişlemesi; CORE-CLOSE için gereksiz. Hub sprintinin bir giriş notuna ihtiyacı olduğu için tamamen silinmedi |
| hubsim | NON-GATING ek | **çıkarıldı** | Aynı gerekçe |
| Takvim | Gün 1 = 2026-10-09 | Gün 1 = 2026-10-12 … Gün 4 = 2026-10-15 | Onay gecikti |
| D3, D5, D6, D7, D11, D12, D14, D15, D17 | — | **değişmedi** | Claude Code onayladı |
| K4G-14 (Signaling API) | Gün 3 | **değişmedi**, MOBILE-HANDOFF kartı | Medya ekibinin çağrı kurulumu bunu bekliyor; küçük |

**v1.1 tutarlılık düzeltmeleri (2026-10-11, Claude Code ikinci inceleme; onay öncesi, v1.1'in parçası):**

| Kalem | Önce | Sonra | Neden |
|---|---|---|---|
| K4G-11 ↔ K4G-14 | K4G-11 `Bağımlı: … K4G-14`; `two_nodes_loopback.rs` DONE'unda signaling blob; `send_signaling` K4G-11 API'sinde | K4G-11 K4G-14'e **bağımlı değil**; CORE testi = TEXT/RECEIPT + dosya + PTT + restart persistence; `send_signaling` + `SignalingReceived` = **K4G-14'ün `KamaNode` uzantısı**, testi `signaling_loopback.rs` | K4G-14 MOBILE-HANDOFF'a ayrılmışken K4G-11 üzerinden dolaylı olarak CORE-CLOSE'u blokluyordu |
| K4G-16 bağımlılığı | "tüm kartların PR'ları" | "**tüm CORE-CLOSE kartlarının PR'ları**; K4G-13/14/15 açık olabilir, durumları raporda ayrı" | Aynı dolaylı bağlama |
| K4G-16 belge içeriği | `kama-mobile` CLAUDE.md listesinde; `KM-SEAM-AK-001.md` bağlantısı koşulsuz | `kama-mobile` yalnız K4G-15 merge olduysa; `KM-SEAM-AK-001.md` / `KM-SEAM-TH-001-TASLAK.md` yalnız mevcutsa | Var olmayan şeye bağlantı |
| K4G-02 DONE | `4(a)–(b)` | `4(a)–(c)` | Hata veren depo testi DONE'un parçası |

**Ek 2026-10-11 (2) — iş emri ve kapsam incelemesinden gelen son düzeltmeler (onay öncesi, v1.1'in parçası):**

| Kalem | Önce | Sonra | Neden |
|---|---|---|---|
| §4.1 | K4G-01 kilidi K4G-00'ı da kapsıyordu | **BOOTSTRAP İSTİSNASI**: K4G-00 docs-only, kilit dışı | Belgeleri repoya koyan PR'ın kendisi kilitleniyordu |
| D7 / K4G-10 | NEDEN "PTT/medya … geç gelen çöptür" | BestEffort/DeadlineBounded **yalnız canlı arama medyası**; PTT'ye ve AMP'ye dokunmaz | PoC-KAPSAM-001 §2: PTT = Reliable klip; plan kapsam notuyla çelişiyordu |
| K4G-08 | `MEDIA_BUNDLE` BestEffort (kodun bugünkü hali) | **PTT-R0**: OPEN/BUNDLE/CLOSE Reliable; geç gelen parça düşmez, oynatma durur-devam eder; PTT-R4 10 s → **120 s** | "Sıfır bozuk hece" ancak Reliable ile; 60 s kesinti senaryosunda akış yaşamalı |
| K4G-11 `drain_background()` | "mailbox çek" (K6 eski metni) | yerel outbox + transport yeniden pompalama; **hub mailbox PoC'de yok** | PoC-KAPSAM-001 EK-2: hop-ARQ (M4) olmadan hub mailbox çalışamaz; hub saf röle |
| K4G-13 AÇIK listesi | mailbox sınırı | çıkarıldı | aynı |
| K4G-05 | — | NON-GATING 60 s tam kesinti koşusu | Mailbox yokken K8 "60 s kesinti sonrası teslim" yükü gönderen outbox + ARQ'da; ölçülmeli |

**Ek 2026-10-11 (3) — ACİL DUR (17:53) ve ön kontrol DUR cevapları (Abkarada kararları; plan aynen, D1–D18 değişmedi):**

| Kalem | Karar | Yer |
|---|---|---|
| ACİL DUR / DUR-K4G-01 | Bulgu D1 (v1.1) kapsamında; #114 kapatır. #114'e 1 (INV-ACK-DELIVER-1), 2 (`max_peers: 1` testi), 3 (bayt-özdeş kanıt belgesi) eklenir; tek başına merge. A1 "enjekte edilemez" cümlesi üstü çizildi, düzeltme yazıldı | §1.2 A1, K4G-01 |
| Yeni kart K4G-01b | `Reorder` ve eş yuvası oturuma anahtarlanır; bağ yalnız el sıkışma/kapanışla değişir; #114'ten hemen sonra, K4G-10'dan önce | K4G-01b |
| DUR-K4G-10 | `Expired` olayı **yok**; S1 §4 K8 sessiz düşüş + `deadline_expired_drops` sayacı; S1 değişmez | K4G-10 §2, §4 |
| DUR-K4G-04 | `weekly.yml:50` kani komutuna yalnız `--features test-util`; başka workflow değişikliği yok | K4G-04 §1 |
| DUR-K4G-12 | (a) `dest_hash` ayrı **kimlik** Ed25519 anahtarından (`SHA-256(SHA-256(identity_pk))`), cihaz anahtarından **değil** — ccr §13.3 ihlali reddedildi; (b) `SR_CDC_V1` + 7×`0x00` kabul, ccr §13.2'ye YAMA KAYDI; (c) Python referans + 3 vektör; (d) mevcut kilitli crate'lere doğrudan bağımlılık = yeni bağımlılık değil | K4G-12 §1–§2 |
| Faturalandırma kilidi | Sahip eylemi (GitHub Billing). Kalkana kadar **CI-KİLİT İSTİSNASI** (yerel tam kapı) ile merge; kilit kalkınca `core-main` elle tetiklenir | §4.2 |
| Branch-coverage 81.27 < 82.0 | Taban değişmez; K4G-16'da yeniden ölçüm; karar Gün 4 | §4.2 |
| T5 | `sudo modprobe sch_netem` — sahip eylemi, INT-02 öncesi | K4G-05 |

**Ek 2026-10-12 (4) — Gün 1 sonrası kararlar (Abkarada; plan aynen, D1–D18'de yalnız D7 bu sprint için askıda):**

| Kalem | Karar | Yer |
|---|---|---|
| DUR-K4G-10 (ikinci) | **Seçenek B — ertelendi.** Motor terk etmiyor + Reorder 4096 boşluk → gerçek BestEffort motor API'si + tel işareti ister (FROZEN transport çerçevesi; BatchPayload geçişiyle birlikte yapılacak). K4G-11 bağımlılığı kalktı. Canlı medya PoC'de kama transport'tan geçmez (PoC-KAPSAM-001 EK-3). PTT Reliable kararı (D7 ek) aynen. | K4G-10, K4G-11, K4G-16 KATMAN 6 |
| DUR-K4G-01 (#114/#116 üç beyanlı sınır) | KATMAN 6 kartı + INT-02 beyanı; (i) için `delivery_channel_drops` sayacı K4G-11 `metrics()`'e, INT-02'de 0 olmalı; (iii) epoch geçişinde ölçülür | K4G-16, K4G-05 |
| K4G-09 kart §4 okuması | **(a)**: BEGIN'den önce gelen FILE_ABORT kayda alınır, BEGIN gelince `rejected`. (b) **değil**: S8'in sessizce düşürdüğü tutarsız/bozuk BEGIN'e alındı **gönderilmez** (S8 ret = drop; yansıtma/yükseltme yüzeyi açılmaz). Gönderenin yerel kabulden önceki iptali tele çıkmaz, yalnız yerel `SendFailed(Cancelled)` | K4G-09 |
| K4G-14 erken başlatma | **Evet.** K4G-14a (kama-app Signaling) K4G-09'dan hemen sonra, K4G-11'i beklemeden; K4G-14b (`KamaNode` uzantısı) K4G-11'den sonra | K4G-14 |
| K4G-07 beyanlı istisna 4 | KATMAN 6 kartı (bu sprintte dokunulmaz) + INT-02 beyanı | K4G-16 |
| K4G-11 brif şartları | Orkestratörün üç şartı plana yazıldı (düğüm-tekil tutamaç, periyodik poll, iki saat) | K4G-11 |
| G-IMPL-S3 | Paket `docs/gates/` (#118), kani yerel 9/9 (kani 0.68.0, weekly komutuyla birebir) → D17 sağlandı; kapı ifadesi Abkarada'dan ayrı mesajla. Hosted kani (38174686178) kırmızı çıkarsa K10 gereği ACİL DUR | K4G-04 |

**Ek 2026-10-12 (5) — #124 / #125 öncesi kararlar (Abkarada):**

| Kalem | Karar | Yer |
|---|---|---|
| DUR-K4G-11 | **Seçenek B**: `kama-node` içinde süreç-içi zarf rölesi (ürün taşıyıcısı değil); INT-02 tek süreç, zarflar netem dışında (beyan); iki telefon arası oturum hub sprintiyle (Plane 1 rölesi + T3). (A) ve (C) reddedildi | K4G-11 Ek (5), K4G-15 README, K4G-16 KATMAN 6 |
| #125 beyanlı sapmalar | Hepsi kabul: `ptt_open(peer, codec, bundle_ms)`; rtt/loss/bitrate `None` (KATMAN 6, demo öncesi zorunlu); `files/<eş>-<transfer_id>`; `start()` tokio dışı; `delivery_dropped` aynası | K4G-11 |
| DUR-K4G-14a | 60 s (ilk kabulden) / eş başına 256 / en eski düşer; 120 s reddedildi; spec "ÖNERİ" kesinleşir | K4G-14 Ek (5) |
| Aralıklı test `arq_identity_is_the_pn_not_the_interim_seq_field` | Kök neden doğrulandı (FEC paritesiyle kurtarılan birim gerçekten teyit ediliyor → test öncülü FEC-açık profilde geçersiz). **Tek dosyalık test düzeltmesi onaylı** (kart dışı, ayrı küçük PR, başlık `test-fix: …`): bu testte FEC kapalı `RuntimeConfig`; ürün profili ve testin amacı (ARQ kimliği = PN) değişmez; dosya başına neden yorumu | — |
| Belgeler | Plan Ek (4)+(5) ve PoC-KAPSAM-001 EK-3 **dosya olarak** `~/Downloads`'a (sohbete yapıştırma 50k'da kesiliyor); orkestratör sha256 ile bayt-özdeş docs PR'ı açar | K4G-00 devamı |
