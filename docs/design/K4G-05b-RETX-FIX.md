# K4G-05b — Yeniden-gönderim sahipliği (transport v1.1) — tasarım notu

## 0. Künye
Kart **K4G-05b** · **rev4, CEVAP PAKETİ 9** (2026-10-24; CEVAP PAKETİ 8, 2026-10-19 ile; çelişkide C9) · taban `d46ec628` = main `3439fccd` (`crates/adaptive-transport`, `crates/client-core` bayt-özdeş; satırlar buradan; `k4g/05-int02` rebase'i sonrası ofset: `sender.rs` ≥ 760 +56, ≥ 4627 +132; `arq.rs` ≥ 102 +6; `runtime.rs` :1290-1336 +457, :1785/:1835 +563; spec ve Ek A hedefleri değişmez) · P0 `k4g/p0-taban` @`df7c9e62` · **TASLAK — `c4dfd1fd` (rev3) revizyonu; onay rev4 SHA'sına yazılır (C9 DUR-17; C8 §2'deki c4dfd1fd onayının yerine).** Kod "K4G-05b ONAYLANDI" bekler; "INT-02 GEÇTİ" = 05b rev4 + 05c + 05d merge + C9 §5. Uygulanmadı. Gövde 1 863 kelime (rev3 1 534); **Ek A** 206 satır, sha256 `4444023a30c72e6d6e4d04dbe76e4420ba56e23d14d1f334caad87dfbcb9156f`; **Ek B** DUR-1..21 kapanışları, açık DUR-23..26, B.2 tanımlar, B.3 değişen eski testler. Etiket: [ölçüm] · [kod] · [tasarım] ölçülmemiş · [çıkarım] koşulmamış.

## 1. Değişmez (sahip metni, CEVAP PAKETİ 5, 2026-10-16)
> "Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir."

Okuma: sessizlik → canlılık kopyası (Duplicate, kayıp değil; INV-001, `tv_loss.rs:51-56`); later-ACK → dedektörle ilan + yeniden plan. Reliable = her motor birimi; RecoverFwd girişine ve ACK sessizliğine dayanmaz; izleme teyide ya da oturum sonuna (INV-NO-IDLE-TEARDOWN) dek. **Kabul (C9), üç koşul birlikte:** birim kayıtlı · payload mevcut · hizmet verilebilir bütçede yeniden seçilebilir; ilk ikisi üçüncüyü sağlamaz. `confirmed_units` dışında payload'u silen yol yok (test). Kusur oturum düzeyindedir: başka birimin ACK'i prob bütçesini yeniler (`sender.rs:3166`) ya da later-ACK ile prob kopyasını kayıp ilan ettirir (`loss.rs:692`).

## 2. Mekanizma (satırlar `sender.rs`; P0 ölçümleri §6)
- **D1** [ölçüm diag-72cb772c, m12 s3 u71, P0; kod]: en yeni kayıt prob (:5796), later-ACK yok → ilan yok (`loss.rs:692`), bütçe yalnız uçuş küçülünce yenilenir (:3166), canlılık yalnız Data kopyalar (:5822); gerçek alıcı açık ters yolda ≈ 8.39 s'de bir bayat SACK yollar (`receiver.rs:93-94`, :268) → `last_ack_ns` tazelenir (:2971), RECOVER_FWD'ye girilmez; gizli varyantlar :5696, :5699.
- **D2** [ölçüm kesinti s1/s3: dup_queue 94→261 / 104→335, HOL 125 s; kod]: fire-once kopyalar Vec sonunda, dedupe yok (:5836-5847), FIFO + ilk token reddinde `break` (:5943-5948); aralık tavansız (:5966).
- **BURST** [kod; P0]: taban yalnız tx başı (:2310, :2322-2325), `set_burst_cap` token'ı kırpar (`pacer.rs:72`), tavanı aşan boya uyanma yok (`pacer.rs:116`). **PAY**: DATA önce (:4236), kopya sonda (:4251), pay yalnız RECOVER_ARQ'da ARQ'ya (:4229-4235).
- **KAYNAK** [kod; P0]: teyitli birime karar üretilir — P0'daki ikisi de kuyruk probundan (hedef :3227-3238, emisyon :5758-5805); runtime her birim taşıyan kararı `seq_to_unit`/`unit_seqs`'e ekler (`runtime.rs:1469-1470`), budama `store.remove` başarısına bağlı (:1736-1743).
- **srtt 60.5 s, Karn değil:** en yüksek yeni-onaylı Data örneklenir (:3034/:3057/:3117), `ack_delay` en büyüğün (`receiver.rs:405`), n = 64 (`rtt.rs:145`). **ESI:** parite `unit.id = esi` (:5255) `acked_units`'e girer (:3027/:3043) → zincir iptali (:3154); prob pariteyi hedefler (:3228), `unit_id = esi` Retransmit yayar (:5796); runtime bunu kaynak sayar (`runtime.rs:1464-1470`).

## 3. Tasarım [tasarım] (`sender.rs` :taban — değişiklik — madde)
1. ESI, **bütün yollar**: ACK eşleşmesi (uçuş :3027, tombstone :3043, un-loss :3104), prob hedefi (:3228, yeniden doğrulama :5765-5770), RECOVERED (:5080-5110, zaten `unit_id`); iç tipler `UnitId` / `Seq` / `Fec{block, esi}` (tel değişmez). **Yalnız-parite kaydı:** Reliable birim değil, sahiplenilmez; kendi ACK'i, kayıp ilanı ya da süre dolumuyla (madde 11 ufku) kapanır; ARQ/prob'a ve runtime tablolarına girmez — ESI.
2. `confirmed_units` — teyitli birime karar üretilmez: kayıp kuyruğu (:3393/:3437), ARQ planı (:5632-5699), **prob hedef seçimi (:3227-3238) ve emisyonu (:5758-5805)**; kalan kayıtlar yalnız muhasebede; küme sınırlı — D2, KAYNAK.
3. `plan_arq_retransmits` :5685-5699, `plan_forward_duplication` :5836 — her politikada tek kalıcı geometrik sahip, düşürme yok; `RetxConfig::fixture` (cap 3, KALİBRE DEĞİL, :1860) kalır, cap yalnız yolu değiştirir; :5963-5971 PreConverge dışına genelleşir — D1.
4. `adopt_unowned` (:4244 ardı, PreConverge/Idle dışı) — (a) canlılık `origin != FecParity`; (b) **devir prob bütçesinin tükendiği turda, hemen** (vade = devir anı, v1 emsali :5693/:5822; aynı turda ikinci emisyon yok) — ana yol, ACK sessizliğine dayanmaz; (c) H-sahiplenme **yalnız prob hedefi olmayan birim** için, son ilerleme/iletimden H sonra; çapa ilerleme, bayat SACK sıfırlamaz; (d) geometrik kaydı varken NO-OP — D1.
5. `drain_due_duplicates` :5932 — PreConverge dışı en küçük birim kimliği önce, birim/tur ≤ 1, token reddinde `break`, aralık ← min(2 × aralık, RTO_MAX); seçim, BURST_CAP tabanı ve uyanma (kaynak 3, :2566-2577) tek `copy_queue_head` — D2.
6. Sıfırlama: mandal H'de; :3153-3155 ardında ilk ilerlemeli ACK vadeleri tabana döndürür; emisyon pace + BURST_CAP'e tabi (INV-RATE-1), sıra madde 5; prob v1 (:3166).
7. RTT :3010-3149 — KABUL KURALI rev4 (A.3): (a) çerçevenin en büyük yeni-onaylı seq'inin kendi kaydı · (b) ilk kez onay · köken Data/Retransmit/Duplicate, zaman o iletimin gönderim anı (taze seq → tek anlamlı; Karn dışlaması kalkar) · FEC parite örnek vermez · (d) doygun `ack_delay` yok · (e) guard; srtt-yok istisnası (DUR-1 a) kalkar, DUR-14 düşer. Tazelik: son kabul edilen örnek yaşı ≤ max(10 × srtt, RTO_MAX), aşılırsa `rtt_stale` (yalnız gözlem). Teslim, BW-örneği, delivered-kredi RTT kararından bağımsız.
8. `on_event_core` :2287 — PreConverge çıkışında bekleyen kopyalar RTO_MAX'a kırpılır.
9. `track_sent` :6079 — PreConverge dışı yalnız vadeli kopya "bekleyen iş" — INV-RATE-1.
10. `emit_tail_probe` :5796 `saturating_add`; `arq.rs:27-34` yalnız doc.
11. **Kaynak/tarihçe (DUR-20)** — birim başına iletim eşlemesi + zamanlama kaydı (uçuş/tombstone, `loss.rs:210`) ≤ **K_MAP = 8**, en eski budanır; **geç-ACK ufku = K_MAP × RTO_MAX = 240 s** (beyanlı; türetim B.2: RTT_max + RTO_MAX ≤ 240 s ⇒ RTT_max ≤ 210 s, sapma gerekçesi yok); ufuk dışı geç ACK → Expired/Unknown: teyit etmez, birim sahipli kalır, yeniden gider, RTT vermez. Kopya işi ≤ 3 (var); payload ≤ store (var); alıcı tamponu → 05c; teyit sonrası bayat kararın runtime tablolarına girmesi (`runtime.rs:1469-1470`): kaynak madde 2, bekçi 05c (runtime eşlemeyi aynı K_MAP kuralıyla budamalı; B.2). Büyüme: v1 logaritmik (:5966); v1.1 kayıt ≤ K_MAP, zincir emisyonu ≤ m + ⌊Δ/RTO_MAX⌋ (DUR-11 a; m = 7, b = 0.4 s); A.7 "doğrusal" → "sınırlı". AMENDMENT-9 K = 1024 bellek sınırı değil; DEMO-01 ön koşulu geri çekildi (C9).

**BURST_CAP tur başı tabanı (A.4)** [kod]: :2322 `burst_lo` = max(tx başı, ARQ planının ilk kaydı, bekleyen prob hedefi, ilk vadeli kopya kaydı), :2325'teki ilk kırpmadan önce; `burst_hi` ≥ `burst_lo` (`u64::clamp` önkoşulu; boy sınırsız :82); token üretilmez (`pacer.rs` dokunulmaz), birikime izin; seçim ve uyanma aynı başlar; "yol girişinde yükseltme" çıktı. PreConverge'de taban v1 (7-EK B1): üründe iw = 2400 B (`runtime.rs:237/:260/:1332`) ≥ birim (≤ 1061 B [çıkarım]); motor iw = 0 (:2034) beyanlı.

**Sınıflar arası pay (DUR-9 a → b-lite)**: vadesi gelmiş kopya kuyruğu başı en geç her `K_ARQ_SHARE`'inci turda DATA'dan önce hizmet alır — :4229-4235 mekanizması genişler (aynı sayaç, `K_ARQ_SHARE` = 4 :1479; yeni sayı, ayrı kova yok); ikisi de vadeliyse ARQ → kopya başı → DATA. Tur = bir `schedule_transmit` çağrısı, kural her durumda [DUR-24]. Gecikme sözü yok.

**B2 — birim başına kopya kaydı ≤ N = 3:** G ≤ 1 (kanıt yolları vadeyi çeker, sahiplenme NO-OP), F ≤ D − 1 (:6005-6013), ARQ kuyruğa girmez, FecParity kayıt almaz; N = D_max = 3 (`DEFAULT_DUP_FACTOR` 2 :1425; `dup3_factor` 3 `emergency.rs:120-121`).

**Sınırlar (DEVIR):** RTO_MAX = `validated_v1().max_backoff_pto_ns` (`tail_recovery.rs:78`); H = max(3·srtt, 2·EAI)·1000/300 ≥ ≈ 13.978 s (:6139-6150), RTO_MAX'la sınırlanmaz. **Uygunluk** (vadeli kaydı olan sahip) ≠ **emisyon** (token'a bağlı, ayrı raporlanır). Prob hedefinde devir ≤ t_arm + 60 s (t_arm = max(son ilerleme, ilk uçuş tiki); varsayım: ilerleme yok, prob token beklemiyor; token beklemesi eklenir — türetim A.7, B.2); prob hedefi olmayan birim ≤ son ilerleme/iletim + H + 1 tik; sonra ≤ RTO_MAX arayla.

## 4. Dokunulan dosyalar (tam liste)
- `crates/adaptive-transport/src/sender.rs` (+ salt-okur `&self` erişimciler, davranış değişmez: `diag_dup_queue_units` [DUR-23 a], RTT örnek yaşı + `rtt_stale`), `src/arq.rs` (doc), `tests/k4g_p0_engine.rs` (P0'dan) + yeni `tests/k4g05b_*.rs`; değişen eski testler (Ek B.3): `tests/dr_cs1_acceptance.rs` :376/:471/:553, `src/sender.rs` :7580/:8614/:8659, `crates/client-core/tests/transport_v1_freeze.rs:20-21`; risk `tests/dr_cs3_structural_oracle.rs:2739-2745`.
- Kimlik/freeze: `client-core/src/runtime.rs:217/:220` (`api_surface` pin güncellemesi yok); kimlik adı: `crates/bench/src/{manifest,main,event,kama_product}.rs`, `.github/workflows/nightly.yml:201/:214`.
- Faz-5 betikleri ve v1 bankası bayt-özdeş; v1.1: `reports/transport-v1.1/tcell/` + `tools/` (ön-kayıt Ek B satır 6).
- Spec (Ek A): engine A.1–A.4, A.8 (§16.2 tek satır: aralık tavanı, H, geç-ACK ufku, tazelik), A.9a; TDD A.5–A.7; işaretçiler A.9b–f; DR-CS-1 A.10; EK4-A2 sha256-pinli → dokunulmaz.
- `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md` (CEVAP 4–9) ve bu not.
- **B3** [kod]: kimlik dizgeleri tele, Noise prologue'una, CCR'ye, el sıkışmaya ULAŞMAZ (tanım, freeze testi, bench etiketleri `kama_product.rs:352-353`, `event.rs:148-151`, `nightly.yml`, Faz-5 etiketleri; kimlikler sayısal `kama-noise-wire/src/handshake.rs:39/:41/:43`, `kama-transport-schema/src/param.rs:24`).
- **05b'de değil:** `client-core` alıcı kabulü, reorder, runtime tabloları → 05c (eski testleri orada: `transport.rs:1323-1338`, `tests/runtime_e2e.rs:255-305`, `runtime.rs:3334-3372`, `kama-node/src/node/tests.rs:415-456`, doc `transport.rs:564-567`); `kama-app/src/messaging.rs` → 05d; teşhis yazıcısı + bayrak → #136 (DUR-19). `transport.rs:25-27` ("motor birimi terk etmez") 05b merge'üyle doğru olur.
- Başka dosya yok; tel, codec, `tv_*`, S1/S3/noise-wire/ccr/kama-node, FEC parite, DR-1x, `dr_cs3_panel.rs` dokunulmaz.

## 5. Spec yama metinleri → Ek A
Ek A = son metin; kaynağı bu nottur. Silme kapısı: 21 blok nottan okunup rev3 çapa/`apply()`/`text_checks` (`patches.py` `4f1a262b…`; rev4 eki yalnız :475 işlemi ve B7 tarih deseni) ve `word_coverage.py` (`4d0c8b53…`) ile `git show d46ec628` metnine bellekte uygulandı: 8 hedefte kayıp 0 / yer değiştirme 0 (engine +53, TDD +78, altı işaretçi/DR-CS-1 dosyası +2'şer, `word_coverage` kapsamı dışı); iki çizgisiz kontrol (:475, :479) kırmızı (0/1); metin denetimleri geçti — yalnız 7-EK A1(1) birebir cümlesi bilinçli olarak yok (C9: "doğrusal" → "sınırlı").

## 5b. Ölçüm (CEVAP 6; CEVAP 7 kabul)
m12 (`~/kama-k4g-work/int02/meas-1f6023ff/m12.md`, beklenti `cb2e4fc1…` kilitli): D1 tekrarlandı (s3 71/100); bayat SACK ilerleme üretmedi; ≈28 B/s yeniden üretilmedi. **srtt teyidi** [ölçüm]: 60.5 s = diag-72cb772c kesinti s1'in `srtt` alanı (60 492.107 ms; `min_rtt` 60 503.792 ms ayrı); m12'de 60.5 s yok — aynı alan (`diag.srtt_ms`) ek-kesinti s1/s3'te 43 621.452 / 43 625.932 ms; m12.md sRTT sütunu yalnız gating2 (237–671 ms).

## 6. Kanıt
**Kural (C9):** önce tabanda kırmızı; sahte saat + gerçek `Receiver`; yanlışlanan madde "yanlışlandı: <test>" — **P0'da yanlışlanan YOK.**
**P0** [ölçüm; `df7c9e62`, taban `3439fccd`, varsayılan = `--all-features`; `P0-OZET.md`], `crates/adaptive-transport/tests/k4g_p0_engine.rs`: `p0_d1_orphan_after_probe_budget` KIRMIZI (u4 t = 400 ms; problar 1430/3480/7580 ms; sonra 592 s 0 yeniden gönderim) · `p0_esi_parity_ack_cancels_unrelated_dup` KIRMIZI (esi0 ACK'i u0 kopyasını iptal; uçuş + tombstone) · `p0_esi_probe_targets_parity` KIRMIZI (`unit_id = esi` Retransmit; store-miss 3) · `p0_burst_cap_clamp_starves_recovery` KIRMIZI (1050 B 578 s gönderilmedi, azami token 217 B; 37 B kontrol gitti) · `p0_class_share_saturated_data_starves_copy` KIRMIZI (vade 7983 ms, ilk hizmet 13 600 ms) · `p0_stale_decision_after_confirm` KIRMIZI (teyitli u4'e 1670/2810 ms'de iki prob kararı) · `p0_rtt_path_change_converges` YEŞİL (0.4 → 2 s; 385 örnek, 76 Retransmit; rev4'te yeşil kalmalı). 05c/05d P0'ları (`client-core/src/transport.rs` R1, `client-core/tests/k4g_p0_receiver.rs` R2, `kama-app/tests/k4g_p0_receipt.rs` R3) o kartlarda; sızıntı testi yazılmadı (05c).
**05b testleri** (P0'lar PR'a taşınır, iddiaları gevşetilmez; tanımlar B.2): D1 tam (üç kol; uygunluk + emisyon) · ESI (parite ESI = `UnitId` 0..3, uçuş + tombstone, en eski kayıt parite; parite akıbeti) [DUR-25] · üç kabul koşulu (her silmede birim `confirmed_units`'te, her yeniden emisyonda payload mevcut) · BURST (+ "taban yalnız yol girişinde" mutantı kırmızı) · PAY (≤ K_ARQ_SHARE tur) · K_MAP (kayıt ≤ 8; ufuk dışı geç ACK) · sıfırlama (yalnız rev4: tur başına emisyon ≤ bütçe, sıra HOL) · RTT (yol değişimi; kopya/FEC ağırlıklı 600 s'de örnek yaşı ≤ sınır, yalnız rev4; soğuk başlangıç A/B/C kilitlenmez — kilitlenirse DUR; teslim/BW/delivered-kredi bağımsız; `k4g05b_d2_stale_rtt_after_outage` srtt ≤ 1 s) · D2 (3 düzenek) · H `k4g05b_adopt_after_h`.
- Mevcut testler + `tv_*` değişmeden yeşil, B.3 hariç (DUR-8 biçimi; A.10); `kama-integration` PR CI dışı (`ci.yml:94`) → PR öncesi koşulur. B8: `dr_cs3_panel` koşulur, kırmızıysa DUR-15.
- **Mutasyon:** 56 (C8) + rev4 9 (BURST 1 · PAY 1 · K_MAP 2 · RTT 3 · parite akıbeti 1 · prob süzgeci 1) = 65 [tasarım; kesin sayı PR'da].
- **T-hücre** tam set bu kartta; **INT-02** C9 §5'e göre — 05b tek başına "INT-02 GEÇTİ" üretmez.

## 7. Süre
Orkestratör ≈ 10 iş günü: 05b 3 · 05c 2,5 · 05d 0,5 · spec/belge 1 · mutasyon + kapılar 1 · T-hücre gece · INT-02 ≈ 1 · rapor 0,5 → onay 27 Ekim varsayımıyla ≈ 7 Kasım; sahip 5–6 Kasım (C9 §6). [aritmetik: 27 Ekim Salı'dan Pzt–Cum 10. iş günü 9 Kasım Pazartesi; 7 Kasım Cumartesi.] Riskler: CI fatura kilidi (CEVAP 8 §1 (2) istisnası); `cargo deny`'ı kıran yeni RustSec uyarıları (ayrı bağımlılık PR'ı).

## 8. Sıra
C9 §6: P0 (bitti) → rev4 + onay → **05b** → **05c** → **05d** → madde 11 / A.7 / §16.2 → mutasyon + kapılar → T-hücre → INT-02 → rapor → "INT-02 GEÇTİ" → K4G-16; her kart ayrı PR; 05b PR'ı #136 merge'ünü bekler (C8 §2; rebase §0 ofseti). A şeridi `crates/adaptive-transport/**`, `client-core/src/{runtime.rs, transport.rs}` (05c'ye dek), `transport_v1_freeze.rs`, `kama-app/src/messaging.rs`, `crates/bench/**`, `nightly.yml`'e dokunmaz. PR açıklaması: EVOLUTION §22 altı beyanı + bayat atıf listesi + karar kaydı yolu.

## 9. DUR
**DUR-1..21 kapandı (Ek B).** C9 ile değişenler: DUR-1 (a) srtt-yok istisnası · DUR-2 (2) DEMO-01 ön koşulu · DUR-9 (a) → b-lite · DUR-11 kayıt K_MAP'le sınırlı · DUR-14 düştü. **Açık** (öneri kalın; Ek A kalın seçenekleri içerir): **23** dup_queue sahipliği erişimcisi — **(a) salt-okur `diag_dup_queue_units`** · (b) yalnız emisyon · **24** PAY kapsamı — **(a) her durumda** · (b) PreConverge dışı · **25** ESI parite kolu statik-r — **(a) beyan** · (b) Controller kolu · **26** (gözlem 2) kontrol kolu — **(a) "prob kapalı × ters yol kesik"** · (b) kol düşer. Ayrıntı Ek B.

## Ek A — yama metinleri (aynen; rev4; kaynak bu not — rev3 `patches.py` çıktısı bu Ek A değildir; `<ONAY-TARİHİ>` = onay günü, boş; sha256 kapsamı bu satırdan `## Ek B`'den önceki son boş olmayan satıra dek, her satır `\n` ile sonlanır (`~/kama-k4g-work/tools/eka-sha.sh`) — satır sayısı ve değer §0'da)
**A.1 engine — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — RTT örnek kabulü + kopya kuyruğu sırası + BURST_CAP tur başı tabanı + sınıflar arası kopya payı + §16.2 saat-sınıfı satırı; sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 7, 2026-10-18 (DUR-3 (b), DUR-9 (a)); CEVAP PAKETİ 7-EK, 2026-10-18 (B1); CEVAP PAKETİ 9, 2026-10-24 (RTT kabulü, BURST_CAP, sınıflar arası pay; DUR-1 (a) srtt-yok istisnası kaldırıldı) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §6 Girdi satırına **v1.1 KABUL KURALI** eklendi (çerçevenin en büyük yeni-onaylı seq'inin kendi kaydı · bu çerçeveyle ilk kez onay · köken Data, Retransmit ya da Duplicate, zaman o iletimin gönderim anı; FEC onarım sembolü örnek vermez · doygun olmayan ack_delay · tutma süresiyle şişmemiş örnek; geri düşüş yok; srtt yokken de aynı kural) ve **tazelik ölçütü** (yalnız gözlem: `rtt_stale` sayacı); §6 min-rtt guard'ının RAW kolu ve "örnek ATLANMAZ" hükmü bu kurala bağlandı; §5 TUR-2a/TUR-2b kol-1 ve TUR-2c kol-2 RTT'si bu kurala bağlandı (superseded işareti RTT'yi dışlamaz; BW-örneği ve delivered-kredi DEĞİŞMEDİ; birim başına kayıt budamasıyla düşen kaydın geç-ACK'i Expired/Unknown); §5'te kol-2 "RTT ALINMAZ" ve ZEYL'deki "(superseded iletimin RTT'si güven-dışı)" üstü çizildi; §13'e kopya kuyruğu içi sıra (en eski onaysız birim önce), BURST_CAP tur başı tabanı (DATA, ARQ planı, kuyruk-probu hedefi ve kopya sırası başı; ilk kırpmadan önce; token üretilmez — PRE_CONVERGE'de v1, beyanlı), RTO_MAX tavanı ve sınıflar arası sıranın TEK istisnası (vadesi gelmiş kopya kuyruğu başı en geç her K_ARQ_SHARE'inci turda DATA'dan önce) eklendi; §16.2 saat-sınıfı tablosuna TEK satır eklendi (kopya kuyruğu aralık tavanı + sahipsizlik süresi + geç-ACK ufku + RTT tazelik ölçütü). Otoriter v1.1 kaydı: transport-layer-tdd §12 [AMENDMENT-12]. Kuantum döngüsü, tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Üstü çizili metin silinmedi.
```
**A.2 engine §5/§6 — taban satırı yerine (eski kelimeler sırasıyla korunur; çizilen `~~…~~`):**
- taban :464 →
```text
  RTT örneği GEÇERLİ [kol-1] **[v1.1: retx_count'tan bağımsız, yalnız §6 KABUL KURALI sağlanırsa — kayıt ilan
  anında largest_acked'in altındadır, alıcının en büyüğü monoton büyür (INV-003): ürün yolunda yalnız ters-yol ACK
  yeniden sıralamasında erişilir]**); pencere-dışına yaşlanan olay düşer (Expired → doğal decay).
```
- taban :473 →
```text
  RTT **[v1.1: köken Data, Retransmit ya da Duplicate; yalnız §6 KABUL KURALI sağlanırsa (TUR-2a erişim notu);
  BW-örneği koşulsuz]** (sansür biter — K-1 sansür-kolu kapanışı); **kol-2 [TUR-2c, NORMATİF]** (superseded —
```
- taban :475 →
```text
  RTT ~~ALINMAZ~~ **[v1.1: §6 KABUL KURALI'yla — superseded işareti RTT'yi dışlamaz; örnek o iletimin gönderim
  anından]** + bu ACK'ten rate-örneği ÜRETİLMEZ + **delivered-KREDİSİ girer** (`credit_delivered`:
```
- taban :479 →
```text
  ~~(superseded iletimin RTT'si güven-dışı).~~ **[v1.1: RTT için kalktı — §6 KABUL KURALI; muhafazakâr kural
  BW-örneği için DEĞİŞMEDİ]** Retransmit-seq'in KENDİ ACK'i inflight-yolundan tam örnek
```
- taban :480 →
```text
  verir (belirsizlik yok) **[v1.1: RTT yalnız §6 KABUL KURALI sağlanırsa]**. Süresi/tavanı dolan kaydın **[v1.1: ya
  da birim başına K_MAP budamasıyla düşen kaydın — transport-layer-tdd §12 AMENDMENT-12; birimi TEYİT ETMEZ]**
  geç-ACK'i → Expired/Unknown (örneksiz;
```
- taban :538 →
```text
Girdi: ~~her acked paket için~~ **[v1.1: aşağıdaki KABUL KURALI'nı sağlayan iletim için; ACK çerçevesi başına ≤ 1]**
       rtt_sample = ack_recv_time − pkt.send_time − ack.ack_delay
```
- taban :542 →
```text
       → RAW sample işlenir **[v1.1: rtprop VARKEN ack_delay ≥ rtprop ise örnek ALINMAZ — KABUL KURALI (e)]**, örnek
       ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı YASAK) **[v1.1: yasak kabul edilmiş örneğe aittir;
       KABUL KURALI reddi beyanlı estimator-donmasıdır, tazelik ölçütüyle izlenir]**.
```
**A.3 engine §6 — :545 ardına ekle (kod çiti içinde):**
```text
       [v1.1 · K4G-05b, NORMATİF — sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 9, 2026-10-24] KABUL
       KURALI: ACK çerçevesi başına en çok BİR örnek; yalnız şunlar birlikte sağlanırsa alınır — (a) örnek, çerçevenin
       gönderilmiş (sender'ın ürettiği) en büyük onaylı seq'inin KENDİ kaydından alınır (en yeni iletim, §3.1;
       ack_delay onundur, §2.1); (b) o seq bu çerçeveyle İLK KEZ onaylanır (uçuştaki kayıt, kol-1 ya da kol-2
       tombstone, un-loss eşleşmesi) — (a) ile (b) birlikte: çerçevenin en büyük yeni-onaylı seq'i; (c) kaydın kökeni
       Data, Retransmit ya da Duplicate'tir ve gönderim zamanı O İLETİMİN gönderim anıdır — her iletim taze seq alır,
       eşleşme tek anlamlıdır, Karn dışlaması gerekmez; FEC onarım sembolü örnek VERMEZ; (d) ack_delay kodlama
       tavanında DEĞİLDİR (65535 × 256 µs ≈ 16.78 s, §2.1 satürasyonu: alıcının tutma süresi bilinmez); (e) rtprop
       varken min-rtt guard düzeltmeyi reddederse ack_delay < rtprop olmalıdır — aksi hâlde RAW, tutma süresiyle en az
       rtprop kadar şişmiştir (FLOOR kodlama: decoded ≤ gerçek tutma, §2.1). Daha küçük yeni-onaylı kayda GERİ
       DÜŞÜLMEZ; bayat örnek alınmaz. Kural srtt yokken (ilk örnek) de AYNEN geçerlidir. Kesinti sonrası ilk taze örnek
       döngüyü yeniden başlatır: DEĞİŞMEYEN kuantum döngüsü Δt ≥ 64·T iken n = 64 uygular. Beyanlı sonuç: çerçevenin
       en büyüğü FEC onarım sembolü ya da daha önce onaylanmış bir seq olduğunda örnek alınmaz — srtt donar, şişmez.
       TAZELİK ÖLÇÜTÜ (yalnız gözlem; karar girdisi DEĞİL): son kabul edilen örneğin yaşı ≤ max(10 × srtt, RTO_MAX)
       (RTO_MAX = 30 s, transport-layer-tdd §12 [AMENDMENT-12]); aşılırsa `rtt_stale` sayacı artar (eşiği aşan her
       dönemde bir kez). Teslim teyidi, BW-örneği ve delivered-kredi bu kurala BAĞLI DEĞİLDİR (§5 kol-1/kol-2 aynen).
       Karar metinleri: `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`.
```
**A.4 engine §13 — :959 ardına ekle (:960 kapanış çitinden önce):**
```text
[v1.1 · K4G-05b, NORMATİF — sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 7, 2026-10-18 (DUR-9 (a));
  CEVAP PAKETİ 7-EK, 2026-10-18 (B1); CEVAP PAKETİ 9, 2026-10-24 — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]
  KOPYA KUYRUĞU İÇİ SIRA (PRE_CONVERGE DIŞINDA): vadesi gelmiş kopya kayıtlarından EN ESKİ ONAYSIZ birimin kaydı
  önce gönderilir (ölçüt: en küçük birim kimliği — ürün yolunda kimlik oturum içi gönderim sırasıdır; eşitlikte
  erken vade, sonra kuyruk sırası); bir birim bir planlama turunda en çok bir kez; token yetmezse o tur DURUR —
  arkadaki kayıt sıranın başını GEÇEMEZ, istisnasız. Teyit edilmiş birimin kaydı gönderilmez. BURST_CAP TUR BAŞI
  TABANI (PRE_CONVERGE DIŞINDA): her planlama turunun başında, İLK kırpmadan önce, BURST_CAP alt sınırı = max(kuyruk
  başındaki DATA biriminin boyutu, ARQ planının ilk kaydının boyutu, bekleyen kuyruk-probu hedefinin boyutu,
  yukarıdaki ölçüte göre ilk vadesi gelmiş kopya kaydının boyutu); üst sınır en az alt sınırdır. Token ÜRETİLMEZ;
  token tavana dek birikebilir; pace_rate DEĞİŞMEZ (INV-RATE-1). Yol seçimi ve bir sonraki uyanma aynı başları
  kullanır. PRE_CONVERGE'de taban v1'dir (max(kuyruk başındaki DATA biriminin boyutu, ilk-uçuş bütçesi)): ilk-uçuş
  bütçesi kaydın boyutundan küçükken BURST_CAP'i aşan kayıt bu kayıtla kapsanmaz (beyanlı istisna,
  transport-layer-tdd §12 [AMENDMENT-12]); çıkış turu tek turdur. Geometrik aralık ×2, üst sınır RTO_MAX
  (transport-layer-tdd §12 [AMENDMENT-12]). PRE_CONVERGE'de §3.3-B sırası ve takvimi DEĞİŞMEZ; PRE_CONVERGE
  çıkışında bekleyen kopyaların aralığı ve vadesi RTO_MAX'a kırpılır. SINIFLAR ARASI SIRA tek istisnayla DEĞİŞMEDİ:
  v1 tahkimi kod sırasıdır (DATA → ARQ → kuyruk-probu → kuruluş probu → kopya kuyruğu → FEC parity; RECOVER_ARQ'da
  her K_ARQ_SHARE'inci tur ARQ ile başlar). İSTİSNA: vadesi gelmiş kopya kuyruğu başı en geç her K_ARQ_SHARE'inci
  planlama turunda DATA'dan ÖNCE hizmet alır — her durumda (PRE_CONVERGE dahil; §3.3-B iç sırası ve takvimi
  değişmez); aynı sayaç ve aynı K_ARQ_SHARE, yeni sayı ve ayrı token kovası YOK; aynı turda ARQ payı da vadeliyse
  ARQ önce, sonra kopya başı; gecikme sınırı VERİLMEZ. Yukarıdaki priority satırıyla ayrışması bu kaydın kapsamı
  dışındadır.
```
**A.5 TDD — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — yeniden-gönderim sahipliği; sahip kararları CEVAP PAKETİ 4 ve 5 (2026-10-16), CEVAP PAKETİ 6, 7 ve 7-EK (2026-10-18), CEVAP PAKETİ 8 (2026-10-19), CEVAP PAKETİ 9 (2026-10-24) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §12'ye [AMENDMENT-12]: INV-NO-ORPHAN (sahip metni) ve üç kabul koşulu (kayıtlı · payload mevcut · hizmet verilebilir bütçede yeniden seçilebilir) · ARQ_RETX_CAP aşımı ve kuyruk-probu bütçesinin tükenmesi yolu değiştirir, birimi düşürmez; prob bütçesi tükenince devir aynı turda · **RTO_MAX = 30 s** normatif sabit (koddan belgeye: `TailRecoveryConfig::validated_v1().max_backoff_pto_ns`; yeni sayı yok) · kesinti sonrası ilk taze ACK'te kopya geri çekilmesi sıfırlanır · FEC onarım sembolünün ACK'i kaynak birimi teyit etmez, onarım sembolü sahiplenilmez · birim başına iletim kaydı ≤ K_MAP = 8, geç-ACK ufku K_MAP × RTO_MAX = 240 s (ufuk dışı geç ACK teyit etmez) · kimlik `kama-v1-full` → `kama-v1.1-full`, `transport-v1` → `transport-v1.1` (AMENDMENT-11'in beyan ettiği `transport-v1.x` de bununla sabitlere işlenir). §5.3 "RTO↑" satırına tavan işareti. Engine-spec §5/§6 (RTT kabul kuralı + tazelik ölçütü), §13 (kopya kuyruğu sırası, BURST_CAP tur başı tabanı, sınıflar arası kopya payı) ve §16.2 (tek saat-sınıfı satırı) v1.1 yamaları aynı kartta; yetki dışındaki belgelere tek satırlık AMENDMENT-12 işaretçileri, DR-CS-1 belgesine kabul fikstürü YAMA KAYDI. Tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Bu belgede metin silinmedi (yalnız ekleme).
```
**A.6 TDD §5.3 — :623 satır sonuna ekle:**
```text
   [v1.1: RTO↑ ≤ RTO_MAX = 30 s — §12 AMENDMENT-12]
```
**A.7 TDD §12 — :846 önüne ekle (+ boş satır):**
```text
> **[AMENDMENT-12 — <ONAY-TARİHİ>, v1.1 · K4G-05b: YENİDEN-GÖNDERİM SAHİPLİĞİ + RTO_MAX · NORMATİF · DAVRANIŞ DEĞİŞTİRİR · TEL DEĞİŞMEZ]**
>
> **Dayanak:** sahip kararları CEVAP PAKETİ 4 ve 5 (2026-10-16), CEVAP PAKETİ 6, 7 ve 7-EK (2026-10-18), CEVAP PAKETİ 8
> (2026-10-19), CEVAP PAKETİ 9 (2026-10-24) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`; tasarım notu
> `docs/design/K4G-05b-RETX-FIX.md`.
> **INV-NO-ORPHAN (sahip metni, CEVAP PAKETİ 5, 2026-10-16):** Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim
> zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden
> gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir.
> **Kabul (NORMATİF, CEVAP PAKETİ 9, 2026-10-24):** her onaysız Reliable birim için üç koşul birlikte sağlanır — (i) birim
> kayıtlıdır (bir izleyen yolun kaydı vardır); (ii) payload'u mevcuttur; (iii) hizmet verilebilir ağ ve gönderim
> bütçesinde yeniden seçilebilir (kaydı BURST_CAP tabanınca kapsanır, sıra başına geldiğinde token birikimiyle
> gönderilir — engine §13 v1.1 kaydı). (i) ve (ii) (iii)'ü sağlamaz: token reddi altında bekleyen kopya kaydı ilerleme
> kanıtı değildir. Payload yalnız teyitle silinir; başka hiçbir yol silmez.
> **Okuma (NORMATİF):** kapsam motorun taşıdığı her birimdir. Sessizlik kayıp İLAN ETMEZ (INV-001): sessizlikte izleyen
> yol birimi canlılık için yeniden gönderir (kopya; loss sayılmaz); later-ACK kanıtı gelince izlenen kayıt — kuyruk-probu
> kopyası dahil — mevcut dedektörle kayıp ilan edilir ve yeniden planlanır. ARQ_RETX_CAP aşımı (her politika) ve
> kuyruk-probu bütçesinin tükenmesi birimi kopya kuyruğuna devreder, ASLA düşürmez; devir bütçenin tükendiği planlama
> turunda, hemen yapılır (kaydın vadesi = devir anı; birim o turda ikinci kez gönderilmez) ve ACK sessizliğine DAYANMAZ.
> **Sahipsizlik süresi** H = max(ACK_SILENCE_SRTT_MULT·srtt, K_CONF_EAI·EAI) × 1000 / `HysteresisConfig::escalate_permille`
> (kodun ack_confidence eşiği; saf sessizlikte RECOVER_FWD'nin girdiği an; eşik yeniden kalibre edilirse H onu izler):
> kuyruk-probu hedefi OLMAYAN birim, ilerleme olmadan (ilerleme = en az bir iletim kaydını ilk kez onaylayan ACK; bayat
> ACK sayılmaz) son iletiminden beri H geçince kopya kuyruğuna alınır. H'nin çapası ilerlemedir: alıcının sessizlikte
> tekrarladığı bayat SACK RECOVER_FWD girişini erteleyebilir, H sahiplenmesini ertelemez. Değişmez RECOVER_FWD
> girişine ve ACK sessizliğine DAYANMAZ. İzleme teyide ya da §0 kapalı listesindeki bir sonlandırmaya dek sürer
> (INV-NO-IDLE-TEARDOWN). Teyit = birimin bir kaynak iletiminin ACK'i ya da FEC RECOVERED bildirimi; FEC onarım
> sembolünün ACK'i kaynak birimi teyit ETMEZ (INV-004). Birim kimliği, iletim seq'i ve FEC (blok, ESI) ayrı tutulur;
> ACK, tombstone, un-loss, kuyruk-probu hedefi ve RECOVERED eşleşmelerinin HEPSİ bu ayrıma uyar. FEC onarım sembolü
> Reliable birim DEĞİLDİR: sahiplenilmez, ARQ'ya ve kuyruk-probuna girmez; kaydı kendi ACK'i, kayıp ilanı ya da süre
> dolumuyla (aşağıdaki geç-ACK ufku) kapanır. Teyit edilmiş birimin kalan kayıtları yalnız muhasebeye girer (dedektör,
> ledger ve kader-kanıtı v1 gibi); ARQ, kuyruk-probu ve kopya yolları teyitli birim için karar ÜRETMEZ. Birim başına
> ≤ 1 geometrik kopya kaydı; birim bu kayda sahipken sahiplenme yolları (sessizlik canlılığı, sahipsizlik devri) kaydın
> vadesine ve aralığına dokunmaz, vadeyi yalnız later-ACK kayıp kanıtı öne çeker. Bu kaydın getirdiği iletimler ortak
> token kovasındandır (INV-RATE-1); PRE_CONVERGE dışında vadesi gelmemiş kopya kaydı DATA örneğini app-limited dışı
> saydırmaz.
> **Uygunluk sınırı (BEYANLI; uygunluk = vadesi gelmiş kaydı olan izleyen yol; emisyon token'a bağlıdır, ayrı ölçülür,
> gecikme sözü VERİLMEZ):** kuyruk-probu zamanlayıcısı t_arm = max(son ilerleme, birimin uçuşta görüldüğü ilk
> zamanlayıcı tiki) anında kurulur; t_arm'dan sonra ilerleme olmaz ve prob emisyonları token beklemezse son prob ve devir
> ≤ t_arm + Σ_{i=0..2} min(taban-PTO·2^i, RTO_MAX) ≤ t_arm + 10 + 20 + 30 s = t_arm + 60 s (taban-PTO ≤ `max_pto_ns`
> 10 s; `max_tail_probes` 3); vadesi gelen ama token bulamayan prob hakkı harcamaz, sonraki PTO gönderimden sayılır — bu
> bekleme toplama eklenir; ilerleme zamanlayıcıyı yeniden kurar. Kuyruk-probu hedefi olmayan birimde ilk uygunluk ≤ son
> ilerleme ya da iletim + H + bir planlama tiki. Sonraki uygunluklar ≤ RTO_MAX arayla.
> **RTO_MAX = 30 s (NORMATİF sabit — koddan belgeye, yeni sayı DEĞİL; CEVAP PAKETİ 5, 2026-10-16, madde 2) [CAL — DR-ARQ-PRODUCT-TAIL-1, tuning pending]:**
> kaynak `TailRecoveryConfig::validated_v1().max_backoff_pto_ns` (PRODUCT-PROFILE "backoff tavanı 30 s", SHIP; pin
> `crates/client-core/tests/transport_v1_freeze.rs`). Üst sınırı olduğu aralıklar: (i) kuyruk-probu PTO geri
> çekilmesi (v1'de de); (ii) PRE_CONVERGE dışında kopya kuyruğunun geometrik aralığı ve tabanı (v1'de tavansızdı;
> PRE_CONVERGE çıkışında bekleyen kopyalar RTO_MAX'a kırpılır). Geç-ACK ufkunun birimi (aşağıda) ve RTT tazelik
> ölçütünün tabanıdır (engine §6). Taban srtt-türevidir; taban-PTO tavanı `max_pto_ns` = 10 s ayrıdır; H kanal-türevidir
> ve RTO_MAX'la sınırlanmaz. ZM-2 4-decade doğrulaması BEKLİYOR (beyanlı: v1.1 T-hücreleri D = 20 s ile bu tavana
> ulaşmaz; uzun pencereli hücreler DEMO-01 öncesi ön-kayıtlı gece kartında — CEVAP PAKETİ 7, 2026-10-18, DUR-3 (b)).
> Değeri değiştirmek ayrı sahip kararıdır.
> **Sıfırlama (NORMATİF):** (i) kuyruk-probu: uçuştaki kaydı azaltan ilk ACK probe_index'i sıfırlar (v1 kodu, sender
> `on_ack_received`; DEĞİŞMEDİ); (ii) kopya kuyruğu: ilerlemesiz süre H'yi aştıktan sonra en az bir kaydı ilk kez
> onaylayan İLK ACK bütün geometrik kopya aralıklarını tabana döndürür (sonraki kopya en geç son kopya + taban); bunun
> doğurduğu emisyon ortak pace'e ve BURST_CAP'e tabidir (INV-RATE-1), sıra engine §13 kopya kuyruğu içi sırasıdır. Bayat
> ACK sıfırlamaz.
> **Beyanlı istisna (CEVAP PAKETİ 7-EK, 2026-10-18, B1):** PRE_CONVERGE'de BURST_CAP tabanı v1'dir; ilk-uçuş bütçesi
> kaydın boyutundan küçükken BURST_CAP'i aşan kayıt izlenir, ama token'a yetişmeyebilir (engine §13 v1.1 kaydı).
> **Kayıt sınırı ve geç-ACK ufku (NORMATİF, CEVAP PAKETİ 9, 2026-10-24, DUR-20):** birim başına iletim eşlemesi ve
> zamanlama kaydı (sender uçuş/tombstone tablosu ve kayıp dedektörü haritası) ≤ K_MAP = 8; sınır aşılırken birimin EN
> ESKİ kaydı budanır. Geç-ACK ufku = K_MAP × RTO_MAX = 240 s (beyanlı): kararlı durumda (aralık RTO_MAX'a ulaşmış)
> budanan kaydın yaşıdır; rampa ve zincir dışı kayıtlar budamayı öne çeker. Türetim: ufuk ≥ desteklenen azami RTT + bir
> tam RTO_MAX; K_MAP = 8 bunu azami RTT ≤ (K_MAP − 1) × RTO_MAX = 210 s için sağlar. Ufuk dışı (budanmış kaydın) geç
> ACK'i → Expired/Unknown: birimi TEYİT ETMEZ, birim sahipli kalır ve yeniden gönderilir, RTT örneği VERMEZ. Diğer
> sınırlar: bekleyen kopya işi birim başına ≤ 3 (D_max); payload sayısı ≤ birim deposu; alıcı tamponu ve runtime eşleme
> tabloları bu kaydın kapsamı dışındadır (K4G-05c).
> **Beyanlı sınırlar (CEVAP PAKETİ 7-EK, 2026-10-18, A1; CEVAP PAKETİ 9, 2026-10-24):** v1'de aralık tavansız olduğu için
> kayıt büyümesi logaritmikti; v1.1'de birim başına kayıt SINIRLIDIR (ufuk K_MAP × RTO_MAX). Emisyon sessizlikte sürer —
> canlılık için bilinçli takas (INV-NO-IDLE-TEARDOWN): bir kopya zinciri Δ sürede birim başına ≤ m + ⌊Δ/RTO_MAX⌋ iletim
> üretir (rampadan sonra birim başına 30 s'de ≤ 1); m = ⌈log2(RTO_MAX/b)⌉, b srtt-türevi kopya tabanı; ilk m iletimin
> rampası v1 ile aynıdır; zincir dışı iletimler (orijinal, RECOVER_FWD ek kopyaları, ARQ ve kuyruk-probu yeniden
> gönderimleri) Δ ile büyümez. [AMENDMENT-9] K = 1024 hop-ACK'siz ardışık PN tavanıdır, bellek ya da tarihçe sınırı
> DEĞİLDİR (tavanda hub fallback ister; bugün SPEC-ONLY); uygulandığında bu yolların emisyonları ona TABİDİR (izleme
> sürer). CEVAP PAKETİ 7-EK (2026-10-18) A1'in "AMENDMENT-9 uygulama kartı DEMO-01 ön koşulu" kararı CEVAP PAKETİ 9
> (2026-10-24) ile geri çekildi. Bayatlayan atıflar ve EVOLUTION §22 altı beyanı: K4G-05b PR açıklaması; yetki dışı
> belgelerde tek satırlık AMENDMENT-12 işaretçileri (CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b)).
```
**A.8 engine §16.2 saat-sınıfı tablosu — :1144 ("geometrik kopya deadline'sızlığı") satırının ardına TEK satır ekle (:1145 SAFE satırından önce):**
```text
| [v1.1] kopya kuyruğu aralık tavanı + sahipsizlik süresi + geç-ACK ufku + RTT tazelik ölçütü | sender dup_queue / adopt_unowned / iletim eşlemesi / RTT kabulü | RTO_MAX = 30 s wall-clock TAVAN (taban srtt-türevi); H = max(3·srtt, 2·EAI)·1000/escalate_permille kanal-türevi; geç-ACK ufku K_MAP × RTO_MAX = 240 s; tazelik max(10·srtt, RTO_MAX) (yalnız gözlem, `rtt_stale`) | ölü kanalda tam sessizlik / sahipsiz birim / birim başına tavansız kayıt / görünmez estimator donması (v1) | 4-decade kanıtı BEKLİYOR (v1.1 T-hücreleri D = 20 s tavana ulaşmaz; uzun pencereli hücreler DEMO-01 öncesi ön-kayıtlı gece kartında) — transport-layer-tdd AMENDMENT-12 |
```
**A.9a engine — :3 önüne, A.1'in ardına ekle (+ boş satır; DUR-4 (b) işaretçisi):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Yama yetkisi (§5/§6/§13 + §16.2'de tek satır) dışında kalıp v1.1'de bayatlayan şu ifadeler DEĞİŞTİRİLMEDİ; v1.1 için otorite transport-layer-tdd §12 [AMENDMENT-12]: §2.1 `ack_delay_saturated` "beklenen 0" (en büyük seq'in alımından 16.78 s sonra gönderilen ACK doyar; v1.1'de doygun ack_delay'li örnek §6 KABUL KURALI (d) ile alınmaz) · §5.1 sessizlik eşiği `ACK_SILENCE_MULT × srtt` (v1 kodu: max(3·srtt, K_CONF_EAI·EAI); sahipsizlik süresi H bu eşikten türer) · §9.1 ARQ_RETX_CAP "veya connection-fail" (v1.1: hiçbir politikada düşürme yok, birim kopya kuyruğuna devredilir) · §12 "PTO backoff exponential (2^n)" (üst sınır RTO_MAX = 30 s) · §16.2 "geometrik kopya deadline'sızlığı" satırı (PRE_CONVERGE dışı aralık tavanı hemen altındaki v1.1 satırında) ve K-1 KAPANIŞ DURUMU'ndaki kol-1 RTT'si (§6 KABUL KURALI'na bağlı) · EK4-A2 uyanma envanteri (`reports/ek4-a2/EK4-A2-NEXT-WAKEUP-CONTRACT.md`; sha256-pinli, dokunulmadı): v1.1'de 7. kaynak = sahipsizlik vadesi; 3. kaynak (kopya kuyruğu) drenajla aynı sıra başını raporlar, aralığı PRE_CONVERGE dışında RTO_MAX'la sınırlı. Düzeltmeleri "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9b `docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararları CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b); CEVAP PAKETİ 9, 2026-10-24 (RTT kabulü, devir) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §4 karar bloğunun RESET/CANCEL, BOUND, PROBE LOSS ("then give up (bounded failure) — no infinite retransmit") ve FAIL-CLOSED ("never unbounded emission") satırları v1.1'de AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): MAX_TAIL_PROBES bütçesi tükenince birim düşürülmez, aynı turda kopya kuyruğuna devredilir ve teyide ya da oturum sonuna dek RTO_MAX = 30 s tavanlı aralıkla yeniden gönderilir; prob kopyası da later-ACK kanıtıyla kayıp ilan edilip yeniden planlanır; sıfırlama kuralları AMENDMENT-12 "Sıfırlama" paragrafındadır. §2 "retx RTT-sample safety" satırı ve §4 KARN satırı v1.1'de engine §6 KABUL KURALI'na tabidir: prob kopyası taze seq taşır, örnek o iletimin kendi gönderim anından alınır (Karn dışlaması kalktı). Prob mekanizmasının kendi bütçesi (`max_tail_probes`) DEĞİŞMEDİ. Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9c `docs/transport-v1/ARCHITECTURE.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararları CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b); CEVAP PAKETİ 9, 2026-10-24 (RTT kabulü, sınıflar arası pay) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Bu belge v1'i anlatır; v1.1'de şu anlatımlar AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): §4.5 "cap aşılırsa `HandToForward`" (her politikada birim kopya kuyruğuna devredilir, düşürülmez; FEC onarım sembolünün ACK'i kaynak birimi teyit etmez) · §4.6 "en fazla 3 probe" (bütçe bitince birim aynı turda kopya kuyruğuna devredilir) ve "(Karn-dışlama korunur)" (v1.1'de Karn dışlaması kalktı; örnek o iletimin gönderim anından, engine §6 v1.1 KABUL KURALI) · §4.9 "Öncelik/tahkim mevcut kod sırasıdır" (sınıflar arası sıra tek istisnayla DEĞİŞMEDİ: vadesi gelmiş kopya kuyruğu başı en geç her K_ARQ_SHARE'inci turda DATA'dan önce; kopya kuyruğu İÇİNDE en eski onaysız birim önce, aralık tavanı RTO_MAX = 30 s, BURST_CAP tur başı tabanı — engine §13). Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9d `docs/transport-v1/PRODUCT-PROFILE.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Bu sözleşme v1'i (`kama-v1-full`) tanımlar; v1.1'de KURAL satırındaki ve "Kimlik" tablosundaki kimlik `kama-v1.1-full` / `transport-v1.1`'dir (`TRANSPORT_V1_PROFILE_ID` / `TRANSPORT_V1_ARCHITECTURE`). `RuntimeConfig` alanları ve tablolardaki değerler değişmez; davranışı değişen satırlar AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): Tail-recovery (`max_tail_probes 3` bütçesi bitince kopya kuyruğuna devir; "backoff tavanı 30 s" = normatif RTO_MAX, aynı zamanda kopya aralığı tavanı) · ARQ ("cap aşımı → HandToForward" düşürmesiz, kalıcı kopya sahipliğine devir). Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9e `docs/transport-v1/INVARIANTS.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararları CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b); CEVAP PAKETİ 9, 2026-10-24 (RTT kabulü) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** v1.1'de AMENDMENT-12'ye tabi (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): INV-008/INV-009 kanonik kimliği `kama-v1.1-full`'dür (`TRANSPORT_V1_PROFILE_ID`; tablodaki `kama-v1-full` v1 içindir) · INV-012 "RTT Karn-dışlamalı" v1.1'de engine §6 KABUL KURALI'yla değişti (her iletim taze seq; örnek çerçevenin en büyük yeni-onaylı seq'inin kendi gönderim anından; FEC onarım sembolü örnek vermez; Karn dışlaması kalktı) · INV-004: FEC onarım sembolünün ACK'i kaynak birimi teyit ETMEZ (motor ACK yolunda ESI'nin birim kimliğiyle çakışması v1.1'de kapatılır) · yeni INV-NO-ORPHAN (sahip metni) AMENDMENT-12'dedir. Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9f `docs/transport-v1/README.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** "Durum" ve "Kanonik kimlik" satırları transport v1'i tanımlar; v1.1'de kod kimliği `kama-v1.1-full` / `transport-v1.1`'dir (`TRANSPORT_V1_PROFILE_ID` / `TRANSPORT_V1_ARCHITECTURE`); yeniden-gönderim sahipliği, RTT örnek kabulü, kopya kuyruğu sırası ve RTO_MAX = 30 s AMENDMENT-12'dedir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]). v1 kanıt bankası (`reports/transport-v1/`) ve Faz-5 betikleri bayt-özdeş kalır; v1.1 T-hücre kanıtı `reports/transport-v1.1/tcell/` altında ayrı etiketlidir (CEVAP PAKETİ 7, 2026-10-18, DUR-6 (a); CEVAP PAKETİ 7-EK, 2026-10-18, A2). Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.10 `reports/decision-requests/DR-CS-1-minimal-bootstrap.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — kabul fikstürü düzeltmesi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-8 (a) koşul 4 — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** v1.1'de kabul fikstürleri düzeltildi; neden: teyit edilmiş birimi yeniden planlayan v1 davranışına dayanıyordu. Kapsam: `crates/adaptive-transport/tests/dr_cs1_acceptance.rs` DI-1 (`f2b_di1_first_data_loss_bootstrap_recovery`), DI-1C (`f2b_di1c_rearmed_pto_lifecycle_exposure`), DI-2 (`f2b_di2_single_ack_erasure_spurious_expected`); yalnız ACK'lenen seq'lerin seçimi değişir, assertion ve eşikler DEĞİŞMEDİ; eski/yeni kod × eski/yeni fikstür matrisi ve mutasyon çıktısı K4G-05b PR açıklamasında. Otoriter v1.1 kaydı: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]. Bu belgenin metni değiştirilmedi.
```

## Ek B — DUR kapanışları ve açık DUR'lar (sahip kararları CEVAP PAKETİ 7 + EK, 2026-10-18; CEVAP PAKETİ 8, 2026-10-19; CEVAP PAKETİ 9, 2026-10-24 — metinler `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`)

**B.1 — DUR-1..10** (CEVAP 7 + EK ile düzeltilmiş hâli; C9'un değiştirdiği yer satırda **C9:** ile)
| # | bulgu | sahip kararı (son) | notta |
|---|---|---|---|
| 1 | srtt yokken katı kural ilk örneği kilitleyebilir (§3.3-D, §3.3-B, DR-EST-1 :1434) | **(a)**: srtt yokken ilk örnek v1 seçimiyle, ilk örnek varken yeni kural; soğuk başlangıç + %30 kayıpta srtt'nin N örnekte yakınsadığını gösteren regresyon testi (N notta). **EK B6:** bayat tohumdan sonra N örnekte toparlanma aynı testte. **C9 (2026-10-24): srtt-yok istisnası KALKTI** — kural ilk örnekte de geçerli; köken Duplicate/Retransmit örnek verebildiği için PRE_CONVERGE kopyaları ilk örneği kilitlemez; soğuk başlangıç testi kalır (kilitlenirse DUR); "ilk örnek RTO_MAX ile sınırlı" öncülü (DUR-14) düştü | A.3; §6 soğuk başlangıç (N = 64; kollar A/B/C B.2) |
| 2 | süresiz kesintide uçuş kayıtları ve `loss.rs:210` haritası büyür | **(a)**; CEVAP 7'nin "birim başına O(1) durum" koşulu EK A1 ile geri çekildi; EK A1: (1) A.7 sayısal + yön; (2) AMENDMENT-9 K = 1024 uygulama kartı DEMO-01 ön koşulu; (3) D2'ye 600 s sınır iddiası. **C9 (2026-10-24, DUR-20):** (2) GERİ ÇEKİLDİ (K = 1024 hop-ACK'siz PN tavanı, hub fallback ister, bellek sınırı değil); yerine madde 11 (K_MAP = 8, ufuk K_MAP × RTO_MAX); (1) "doğrusal" → "sınırlı" | §3 madde 11; A.5, A.7; §6 D2 kol 3 (B.2) |
| 3 | §16.2 ZM-2/ZM-3: H ve RTO_MAX duvar-saati tavanı yeni zaman mekanizması | **(b)**: §16.2 tablosunda TEK satır yetkisi (YAMA KAYDI; yalnız o satır); A.8 KOŞULSUZ (EK C); uzun pencereli hücreler DEMO-01 öncesi gece kartında. rev4: aynı satıra geç-ACK ufku ve RTT tazelik ölçütü (C9 §6 "§16.2") | A.1, A.7, A.8; D listesi |
| 4 | yetki/dosya dışı bayatlayan atıflar | **(b)**: yetki dışı belgelere tek satırlık AMENDMENT-12 işaretçisi (YAMA KAYDI biçimi, silme yok); asıl düzeltme "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı. **EK B8:** `dr_cs3_panel.rs:996-1001` referans mı assertion mı — assertion ise DUR-8 gibi 2×2, referans ise DUR-4 (b) işaretçisi yeter | A.9a–f; §6 B8: referans (`lineage`); `f3`/`f4` (:968-969) `control_12b` tavanına girer (:3372) → panel koşulur, kırmızıysa DUR-15; D listesi |
| 5 | "`recover_fwd_entries = 0` (330/330)" yanlış: ham veri 15/330 (T1a 2, T1b 7, T6 6) | **(a)**: ayrı ERRATUM PR'ı (#135); donmuş belgelerde ERRATUM bandı; tasarım etkilenmez | bu kartın dışında |
| 6 | T-hücre artefaktları + Faz-5 betikleri v1 bankasına bağlı; ortam farkı | **(a) düzeltmeli + EK A2**: Faz-5 betikleri ve v1 kanıt bankası BAYT-ÖZDEŞ; v1.1 koşusu `reports/transport-v1.1/tcell/*`; betik kopyaları `reports/transport-v1.1/tcell/tools/` (yalnız etiket + ROOT farkı; orijinal ve kopya sha256 ön-kayıtta); ön-kayıt koşudan ÖNCE commit (hücreler, eşikler, kabul ölçütü, koşulacak commit SHA, `Cargo.lock` SHA'sı, çekirdek 7.2.8-8-omarchy-bore — T-hücre + INT-02 aynı, rakip kollar aynı ortamda yeniden koşar) | §4; A.9f |
| 7 | INT-02 harness `k4g/05-int02`'de; iki şerit `sender.rs`'e dokunur | **(a)**: `k4g/05-int02` ayrı PR (#136) → merge → `k4g/05b-retx-fix` rebase | §0 ofset; §8 |
| 8 | `dr_cs1_acceptance.rs` f2b_di1/di1c/di2 teyitli birimin yeniden planlanmasına dayanıyor | **(a) dört koşulla**: (1) test başına PR paragrafı: neden yeşildi · düzeltmeyle neden kırmızı · fikstür tam olarak neyi değiştirir · assertion/eşik DEĞİŞMEZ; (2) çıktılı 2×2: eski kod + eski fikstür yeşil · yeni kod + eski fikstür KIRMIZI · yeni kod + yeni fikstür yeşil · eski kod + yeni fikstür de yeşil (kırmızıysa (a) değil, DUR); (3) mutasyon üç testin mutant öldürdüğünü gösterir; (4) DR-CS-1 belgesinde YAMA KAYDI | §6; A.10; B.3 |
| 9 | sınıflar arası sıra aynı; HOL DATA/ARQ arkasında bekleyebilir | **(a)**: HOL-önce yalnız kopya kuyruğu içinde; KOŞUL: D2 testi eşzamanlı yeni trafikle (kol 2, EK B5); dosya kolu INT-02'de ölçer. **C9 (2026-10-24): (a) → b-lite** — vadesi gelmiş kopya kuyruğu başı en geç her K_ARQ_SHARE'inci turda DATA'dan önce (yeni sayı, ayrı kova yok; gecikme sözü yok); A.4 "sınıflar arası sıra DEĞİŞMEDİ" tek istisnayla | §3 PAY; A.4; §6 PAY |
| 10 | biçim: "≤ 2 sayfa" ile "yama metni aynen" çakışır | **(a)**: gövde ≈ 2 sayfa + aynı dosyada Ek A (aynen spec'e) + Ek B | bu not |

**B.1b — DUR-11..21** (CEVAP 8, 2026-10-19: DUR-11..15; CEVAP 9, 2026-10-24: DUR-16..21 ve C8 maddelerinin değişikliği)
| # | bulgu | sahip kararı (son) | notta |
|---|---|---|---|
| 11 | doğrusal sınır: A1(1) yalnız eğim; "≈ 1000" türetilmedi | **(a)** C8: A.7 kesin biçimi (m + ⌊Δ/RTO_MAX⌋) normatif; "≈ 1000" sahip tahmini; "≤ 1" rampa sonrası eğim; D2 kol 3 fikstür sayısı PR gövdesine, INT-02 raporu gerçek U ve kayıt sayısını yazar. **C9 (DUR-20) ile:** kayıt birim başına ≤ K_MAP; kesin biçim zincirin EMİSYON sayısı olarak kalır; kayıt toplamı ≤ U × K_MAP ("≈ 1000" kayıt tahmini bu sınırla aşıldı [çıkarım]) | §3 madde 11; A.7; B.2 D2 kol 3 |
| 12 | ARQ (:5656-5661) ve prob (:5775-5781) aynı BURST_CAP deliği | **(a)** C8: bu kartta (+2 kol, +2 mutant = 56). **C9:** taban tur başında (:2322, ilk kırpmadan önce); "yol girişinde yükseltme" çıktı | §3 BURST; A.4; §6 |
| 13 | B2 sapması: F ≤ D − 1 | **(a)** C8: N = D_max = 3, mevcut sabitlerden; ölçümde sapma → DUR | §3 B2; B.2 |
| 14 | "ilk örnek zaten RTO_MAX ile sınırlı" kodda yok | (a) C8: beyan; CEVAP 7 cümlesi geri çekildi, yerine soğuk başlangıç kol B. **C9: DÜŞTÜ** (srtt-yok istisnası kalktı) | A.3; B.2 soğuk başlangıç |
| 15 | `dr_cs3_panel` düzeltmeyle kırmızıya düşerse | C8 kuralı: kırmızı yalnız v1.1'in kasıtlı değiştirdiği davranışı (fire-once, tavan) iddia eden satırdansa iddia DUR-8 biçiminde güncellenir (paragraf, 2×2, mutasyon) + DR-CS-3 kaydına YAMA KAYDI; eşik/amaç başka nedenle değişirse DUR — o test atlanmadan diğerleri sürer, PR o DUR kapanmadan açılmaz | §6 B8; B.3 (oracle ve iddiası değişen testler aynı biçimde) |
| 16 | R1/R2/R3 + tablo sızıntısı kapsamı | **(b)** C9: iki kart — K4G-05c alıcı kabul bütünlüğü (R1 + R2 + sızıntı; `client-core`), K4G-05d bekleyen RECEIPT kuyruğu (R3; `kama-app`); "Reliable düzeldi" üçü merge olmadan söylenmez; "INT-02 GEÇTİ" üçünü bekler; INT-02'ye `ek-tuketici-durak` | §4 "05b'de değil"; §6; §8 |
| 17 | not revizyonu | **Evet** C9: rev4 → dala push → SHA + Ek A sha256; onay rev4 SHA'sına (19 Ekim onay metni varsa rev4'le yenilenir); kod onaydan önce yazılmaz | §0 |
| 18 | R2 statüsü | C9: K4G-05c'de kapatılır, beyanlı sınır olmaktan çıkar; DUR-K4G-01 (i) geri çekildi; `delivery_channel_drops == 0` yapısal sağlanır | §4 (05c) |
| 19 | #136 gözlem düzeneği | C9: (i) `INT-02-RAPORU.md:127` cümlesi düzelir; (ii) teşhis yazıcısı + `ClientLiveDiag` tur-başı yazımı `RuntimeConfig` bayrağı arkasında, varsayılan kapalı; (iii) birleşik adayda bayrak açık/kapalı karşılaştırma koşusu (gating2, 2 tohum) | §4 (#136); 05b yalnız salt-okur erişimci ekler (DUR-23) |
| 20 | K_MAP (kaynak/tarihçe) | C9: **K_MAP = 8** (ufuk 240 s = 8 × RTO_MAX; türetim notta: desteklenen azami RTT + bir tam RTO_MAX; sapma gerekçesi varsa DUR) | §3 madde 11; A.7; B.2 (sapma yok) |
| 21 | PTT açıkken dosya parça sınırı | C9: madde 05d'den ÇIKAR; karma trafik INT-02'de ölçülür, karar Sprint 3 | bu kartın dışında |

**B.1c — açık DUR'lar** (orkestratör, P0 sırasında; öneri kalın, Ek A kalın seçenekleri içerir; başka seçenek Ek A sha256'sını değiştirir)
| # | bulgu | seçenekler | durum | notta |
|---|---|---|---|---|
| 23 | D1 "uygunluk" ölçümü: kopya kuyruğu sahipliği kamu API'sinde görünmüyor (P0 D1 çıktısı) | **(a) salt-okur `&self` `Sender::diag_dup_queue_units()` (`sender.rs`, 05b dosyası; mevcut `diag_*` deseni :2114-4664; davranış değişmez)** · (b) uygunluk ölçülmez, yalnız emisyon | açık | §4; §6 D1; B.2 D1 |
| 24 | PAY: sahip metni duruma bağlı değil; :4229-4235 mekanizması yalnız RECOVER_ARQ'da | **(a) tur = `schedule_transmit` (pipeline turu) çağrısı; kural her durumda (PRE_CONVERGE dahil; §3.3-B iç sırası ve takvimi değişmez, yalnız DATA'ya karşı payı)** · (b) PRE_CONVERGE dışında (A.4 değişir) | açık | §3 PAY; A.4 |
| 25 | ESI P0 testleri parite için statik-r (Manual) kolunu kullanır (`k4g_p0_engine.rs` `product_sender_static_r`); Controller varsayılanında parite PreConverge'de kapalı, ürün uçuş kolu PreConverge sonrası kayıplı kanalda erişilir (`runtime.rs:1866-1878` mandalı) | **(a) beyan: parite emisyonu ve ACK yolları iki kipte ortak (`sender.rs:5236-5286`, :3019-3043); Controller kolu INT-02/T-hücrede gözlenir** · (b) 05b'ye Controller kipinde kayıplı kanal kolu | açık | §6 ESI; B.2 |
| 26 | gözlem 2: gerçek `Receiver` açık ters yolda ≈ 8.39 s'de bir bayat SACK yollar (`receiver.rs:93-94` `ACK_CADENCE_FLOOR_NS`, :268) → `last_ack_ns` (`sender.rs:2971`) ve ack_confidence tazelenir, RECOVER_FWD'ye girilmez; rev3 B.2'nin "kapalı × sessizlik" kontrol kolu gerçek alıcıyla kurulamayabilir | **(a) kontrol kolu "prob kapalı × ters yol kesik" (ACK'ler fikstürde düşer); sahiplik devri H'ye dayanmaz (madde 4 b; H yalnız prob hedefi olmayan birim, çapa ilerleme)** · (b) kontrol kolu düşer, beyan | açık | §3 madde 4; A.7; B.2 D1 |

**D — KATMAN 6 / Sprint 2 birikimi (bu kart değil; CEVAP PAKETİ 7-EK, 2026-10-18; C9 değişiklikleri satırda):**
- `RetxConfig::fixture` (cap 3, KALİBRE DEĞİL) adlandırma + kalibrasyon kartı (DR-1x ile).
- HEAD bench aracı v1 arşiv manifestlerini `canonical_kama=false` sayar → iki kimliği tanıyan araç (M sonrası).
- ~~AMENDMENT-9 K = 1024 uygulama kartı — DEMO-01 ön koşulu (A1).~~ C9 (2026-10-24): ön koşul geri çekildi (K = 1024 bellek sınırı değil; hub fallback ister); yerine madde 11.
- DUR-4 bayat-atıf doküman PR'ı ("INT-02 GEÇTİ" sonrası; tam liste K4G-05b PR açıklamasında).
- ZM-2 4-decade uzun pencereli hücreler (0.25k/1k/100k/1M, D 30 s'ye ulaşır, N ≥ 30) — DEMO-01 öncesi ön-kayıtlı gece kartı.
- ~~DUR-9 (b) sınıflar arası HOL yuvası — INT-02 dosya kolu açlık gösterirse ayrı §13 kartı.~~ C9: b-lite bu kartta (K_ARQ_SHARE kopya payı); dosya kolu yine ölçülür.

**B.2 — §6 tanımları ve ayrıntı (gövde boyu için buraya taşındı; DUR-10 (a)).** Taban beklentileri [çıkarım; koşulmadı], P0 satırları [ölçüm].
- **Düzenek** [tasarım]: `crates/adaptive-transport/tests/k4g05b_*.rs` + P0'dan `k4g_p0_engine.rs`; yeni bağımlılık yok (dev-dependencies yalnız `tokio`). Kurulum = P0 `product_sender` (`ClientSession::new`, `lib.rs:89-93` + `runtime.rs:1290-1351` ürün ayarlayıcılarının aynası; `set_cs_probe_lane(false)` :1313, `bootstrap_rtt: false` :261; ilk-uçuş 2400 B :260/:1332); ürün tikleri (`runtime.rs:65/:67/:69`); tek yön 200 ms; gerçek `Receiver` (quota 4, 20 ms ACK tiki). Runtime aynası (P0 `RuntimeMirror`: store, `seq_to_unit`/`unit_seqs`, store-miss) — 05b testinde silme yalnız motor teyidiyle ve eşleme K_MAP kuralıyla (05c bekçisinin modeli). FEC parite kolu `product_sender_static_r` (Manual) [DUR-25].
- **D1 (rev4 fikstürü, C9):** PreConverge dışında (RTT önce kurulur; çıkış `sender.rs:3926-3933`); kurban taze ACK'in işlendiği tikte en yüksek seq ile gönderilir, ondan sonra ACK'lenen seq yok; yeni uygulama verisi yok (başka birimin ACK'i bütçeyi yenilerdi); her yeniden emisyonda runtime aynasında payload mevcut (store-miss 0). Kollar: (1) prob açık × bayat SACK — P0 KIRMIZI; rev4 iddiası: devir ≤ t_arm + Σ PTO_i (+ prob token beklemesi), sonra uygunluk ≤ RTO_MAX arayla; (2) prob kapalı × bayat SACK — H yolu; tabanda kırmızı beklenir (bayat SACK RECOVER_FWD'yi engeller, later-ACK yok); rev4: ilk sahiplenme ≤ son iletim + H + 1 tik; (3) kontrol prob kapalı × ters yol kesik [DUR-26 (a)] — tabanda RECOVER_FWD canlılığıyla yeşil beklenir (`sender.rs:5816-5831`). Uygunluk (`diag_dup_queue_units` [DUR-23] + prob hedefi + ARQ planı) ve emisyon (gerçek `TransmitDecision`) ayrı raporlanır; owned(U) her olayda.
- **Uygunluk türetimi (A.7)** [kod + aritmetik]: PTO_i = min(taban-PTO·2^i, `max_backoff_pto_ns` 30 s), taban-PTO = clamp(srtt + 4·rttvar + 25 ms, 10 ms, 10 s) (`tail_recovery.rs:155-173`); zamanlayıcı t_arm = max(şimdi, son ilerleme) (:217-220), her gerçek prob emisyonunda yeniden kurulur (:260-262), bütçe 3 (:80, :221) → 3. prob ve devir ≤ t_arm + 10 + 20 + 30 s; vadeli ama token'sız prob hakkı harcamaz (`sender.rs:5775-5781`), bekleme toplama eklenir; ilerleme sıfırlar (`sender.rs:3165-3168`). P0 örneği: t_arm ≈ 400 ms, problar 1430/3480/7580 ms (taban-PTO ≈ 1.03 s) → devir 7580 ms.
- **K_MAP türetimi** [aritmetik]: ufuk ≥ desteklenen azami RTT + bir tam RTO_MAX ⇔ K_MAP ≥ ⌈RTT_max/RTO_MAX⌉ + 1; K_MAP = 8 ⇒ RTT_max ≤ 7 × 30 s = 210 s. Azami RTT kodda/spec'te sabit değil; en uzun kodlanabilir alıcı tutması ack_delay tavanı 65535 × 256 µs ≈ 16.78 s (`receiver.rs:77-80`), ölçülen en büyük (bayat) srtt 60.5 s (§5b) — ikisi de altında; asgari K (RTT_max = 60.5 s) = 4, 8 sahip kararı → sapma gerekçesi yok, DUR yok. Saf zincirde (ilk kopya vade = an, aralık srtt'den ×2, ≤ RTO_MAX; b = 0.4 s) kopyalar 0, 0.8, 2.4, 5.6, 12.0, 24.8, 50.4 s, sonra 30 s arayla; zincir dışı kayıtlar (F ≤ 2, ARQ, prob) kökün budanmasını öne çeker → o ufukta geç ACK Expired (güvenli). Runtime eşlemesi aynı kuralla budanmazsa ufuk dışı geç ACK'te payload silinir (`runtime.rs:1736-1743`) ama motor teyit etmez; birim teslim edilmiştir, sonraki yeniden gönderim başlık-only gider (`runtime.rs:1539-1548`) ve ACK'i teyit eder [çıkarım] → kural 05c bekçisine (C9 §3) girer.
- **D2 simgeleri:** T_r kesinti bitişi · T_a T_r sonrası ilk ilerlemeli ACK · T_H sıfırlamadan sonra u₀ kopya kaydının vadesi (≤ son kopya + taban; A.7 Sıfırlama (ii)) · u₀ T_r'deki en eski onaysız birim · S u₀ boyu · X T_H ile u₀ emisyonu arasında kopya sınıfı dışında gönderilen bayt · min_rate 31 B/s (`budget.rs:29`; hız tabanı `sender.rs:4212`) · R₀ kesinti başındaki uçuş kaydı sayısı · Δ kesinti süresi · D birimin RECOVER_FWD kopya faktörü (≤ D_max = 3) · m = ⌈log2(RTO_MAX/b)⌉ · U kesintideki onaysız birimler.
- **D2 iddiaları:** kol 1 (60 s iki yönlü kesinti, saf drenaj) ve kol 2 (kol 1 + T_r sonrası aynı boy DATA ≤ min_rate/2) — T_a sonrası ilk Duplicate u₀'nın; u₀ emisyonu ≤ T_H + ⌈(S + X)/min_rate⌉ + 1 ms (+ PAY: ≤ K_ARQ_SHARE tur); owned(u₀) her olayda. Kol 3 (600 s kesinti) — birim başına kayıt ≤ K_MAP, kopya işi ≤ D_max, zincir emisyonu ≤ m + ⌊Δ/RTO_MAX⌋ (DUR-11 a); tabanda kırmızı beklenir [aritmetik: v1 zinciri 600 s'de 10 kopya (0 … 408.8 s, ×2 tavansız) + orijinal = 11 > 8].
- **D2 fikstür koşulları K1–K4** (`dr_cs3_panel` F1–F9 ile karışmasın diye K; her koşuda iddia edilir; düşerse fikstür düzeltilir, beyan): K1 u₀ T_r'de en eski onaysız birim · K2 u₀'nın son kaydı Retransmit kökenli (canlılık `sender.rs:5822` dışlar) · K3 T_r'de kuyrukta u₀'nın önünde vadesi gelmiş ≥ 1 fire-once kayıt (:5843-5847) · K4 T_r sonrası token sınırlı (pace_rate × tik < S; FIFO + `break` :5943-5948).
- **Soğuk başlangıç (rev4):** RTT 400 ms, deterministik %30 kayıp, 600 s; kollar A soğuk başlangıç · B ilk ACK T_hold > 16.78 s tutulur → rev4'te (d) ile reddedilir, bayat tohum oluşmaz · C ilk çerçevenin en büyüğü kuruluş probu (Duplicate) → örnek onun gönderim anından. İddia: kilitlenme yok (ilk kabul sonlu sürede; kilitlenirse DUR) ve **N = 64** kabulde RTT ≤ srtt ≤ RTT + ⌈(7/8)⁶⁴·(srtt_r − RTT)⌉ + 256 µs; her kabul ≥ 1 kuantum adımı (`rtt.rs:145`, `QUANTUM_CAP` 64 :131); iki derleme bit-özdeş. Taban kırmızısı: v1 en büyük olmayan Data'yı da örnekler (`sender.rs:3034`) → kabul sayısı kâhin sınırını aşar.
- **RTT tazelik ve bağımsızlık (yalnız rev4):** RECOVER_FWD'de DATA başına kopya + FEC açık, 600 s: her olayda son kabul edilen örnek yaşı ≤ max(10 × srtt, RTO_MAX) ya da `rtt_stale` artmış; yol RTT'si 0.4 → 2 s (t = 60 s) kopyalı trafikte bantta yakınsar (P0 kolu yeşil kalır); teslim teyidi, BW-örneği ve delivered-kredi her çerçevede RTT kabul/ret kararından bağımsız (v1 ile aynı).
- **ESI ve parite akıbeti:** parite ESI = `UnitId` 0..3; uçuş + tombstone kolları; en eski kayıt parite → prob pariteyi hedeflemez, parite kaydı kendi ACK'i / kayıp ilanı / ufuk dolumuyla kapanır, ARQ'ya girmez, runtime aynasına girmez; ilgisiz birimin kopyası iptal edilmez.
- **BURST (B1) kolları:** P0 kolu (hız 1064 B/s, tx başı 37 B, kayıt 1050 B, 600 s; emisyon iddia edilir, cap değeri değil); karışık boy; kontrol L = DATA boyu (tabanda yeşil); 2·MTU + 1; aynı tur; ARQ planı başı, bekleyen prob hedefi (DUR-12 a); mutant "taban yalnız yol girişinde" KIRMIZI olmalı; PreConverge kontrolü (ilk-uçuş 2400 B, birim ≤ 2400 B; v1); çıkış turu v1.
- **B1 karşı örneği** [aritmetik; P0 ölçtü]: pencere = hız × 40 ms; boy motorda sınırsız (`sender.rs:82`); üründe birim ≤ 1061 B [çıkarım; `runtime.rs:227/:234/:1835`]. Hız 1064 B/s, tx başı 37 B → tavan 42 B < 1050 B → `break` (:5948), uyanma yok (`pacer.rs:116`), AppData token'ı kırpar (`pacer.rs:72`). PreConverge'de DR-EST-1 kuruluş kolu en erken vadeli §3.3-B kopyasını token-muaf ≤ 8 s'de bir taşır (:5888, :5914).
- **PAY ve sıfırlama:** pace'i dolduran DATA + vadesi gelmiş eski kopya → kopya başı ≤ K_ARQ_SHARE tur içinde hizmet alır (P0: vade 7983 ms, tabanda ilk hizmet 13 600 ms); sıfırlama: kesinti sonrası ilk ilerlemeli ACK'ten sonra her turda emisyon ≤ token bütçesi (pace × tik, ≤ BURST_CAP) ve kopyalar en küçük birim kimliği sırasıyla.
- **B2 ekleme yerleri** [kod]: `sender.rs` :3395, :5693, :5827, :5843, :6008, :6024 (:5904/:5967 net 0); toplam ≤ 3 × 4096 = 12 288 (`runtime.rs:248/:1785`) [çıkarım].
- **kama-integration:** 11 test dosyası `adaptive_transport::` kullanır; komut `main.yml:47` `cargo test --workspace --all-features`.

**B.3 — Değişen eski testler (DUR-8 biçimi; C7 DUR-8 (a), C8 DUR-15 kuralı).** 2×2 = (eski/yeni kod) × (eski/yeni test); "fikstür" = yalnız girdi değişir, assertion ve eşik aynı; "iddia" = v1.1'in kasıtlı değiştirdiği davranışın iddiası güncellenir — o testlerde "eski kod + yeni test" hücresi KIRMIZI beklenir (test değişikliği ayırt ediyor; yeşilse DUR). Her test için PR'da paragraf + çıktılı 2×2 + mutasyon.
1. `crates/adaptive-transport/tests/dr_cs1_acceptance.rs:376` `f2b_di1_first_data_loss_bootstrap_recovery` (DI-1) — **fikstür.** Yeşildi: ACK'lenen seq seçimi teyitli birimin yeniden planlanmasına dayanır (C7 DUR-8); kırmızı olur: madde 2 teyitli birime karar üretmez; değişir: yalnız ACK'lenen seq'ler (A.10). 2×2: E/E yeşil · Y/E KIRMIZI · Y/Y yeşil · E/Y yeşil (kırmızıysa DUR).
2. `:471` `f2b_di1c_rearmed_pto_lifecycle_exposure` (DI-1C) — **fikstür**, gerekçe ve 2×2 DI-1 ile aynı.
3. `:553` `f2b_di2_single_ack_erasure_spurious_expected` (DI-2) — **fikstür**, gerekçe ve 2×2 DI-1 ile aynı.
4. `crates/adaptive-transport/src/sender.rs:7580` `tail_probe_ack_is_karn_excluded_from_rtt` — **iddia.** Yeşildi: v1 RTT'yi yalnız `origin == Data` kaydından alır (:3034), prob Retransmit'tir; kırmızı olur: rev4 kuralı kökeni Retransmit olan en büyük yeni-onaylı seq'ten o iletimin gönderim anıyla örnek alır; değişir: "srtt değişmez" → "srtt, en büyük prob seq'inin kendi gönderim anından örnekle güncellenir" (ad bu anlamla güncellenir, eski ad doc yorumunda). 2×2: E/E yeşil · Y/E KIRMIZI · Y/Y yeşil · E/Y KIRMIZI.
5. `sender.rs:8614` `karn_superseded_late_ack_credits_without_samples` — **iddia.** Yeşildi: kol-2 "RTT ALINMAZ" (supersede işareti); kırmızı olur: tek-seq'li çerçevede seq 1 en büyük yeni-onaylı, köken Data → örnek (ham 400 − 20 = 380 ms); değişir: yalnız RTT iddiası; rate-örneği üretilmez, delivered-kredi, `(hits, ambig, bw_rec)` ve reorder iddiaları aynen (teslim/BW/delivered-kredi RTT kararından bağımsız). 2×2: E/E yeşil · Y/E KIRMIZI · Y/Y yeşil · E/Y KIRMIZI.
6. `sender.rs:8659` `karn_both_transmissions_acked_wire_byte_semantics` — **iddia.** Yeşildi: orijinalin geç ACK'i kol-2'den örneksiz; kırmızı olur: rev4'te o ACK (tek-seq çerçeve, ilk kez onay) seq 1'in gönderim anından örnek verir; değişir: "RTT değişmez" iddiası; wire-byte kredisi ve "rate-örneği üretilmez" aynen. 2×2 aynı.
7. `crates/client-core/tests/transport_v1_freeze.rs:20-21` `kimlik_sabitleri_donmus` — **iddia.** Kimlik `kama-v1.1-full` / `transport-v1.1` (CEVAP 5); iki `assert_eq!` değeri; `api_surface` pini değişmez. 2×2 aynı.
8. Risk: `crates/adaptive-transport/tests/dr_cs3_structural_oracle.rs:2739-2745` — `burst_cap == window_bytes.clamp(front_size, max(front_size, 2·MTU))` kimliğini pinler; rev4 tabanı yol başlarını kapsayınca kurtarma başı olan parmak izlerinde ayrışabilir [çıkarım] → düzeltmede koşulur; kırmızıysa C8 DUR-15 kuralı (yalnız kasıtlı değişen tavan formülündense iddia bu biçimde güncellenir + DR-CS-3 YAMA KAYDI metni sahibe — Ek A'da yok; başka nedenle DUR).
9. 05c'ye düşenler (yalnız atıf; orada aynı biçimde): `client-core/src/transport.rs:1323-1338` (boşluk atlama), `client-core/tests/runtime_e2e.rs:255-305` (taşma ACK'i etkilemez), `client-core/src/runtime.rs:3334-3372`, `kama-node/src/node/tests.rs:415-456`, doc `client-core/src/transport.rs:564-567`.
