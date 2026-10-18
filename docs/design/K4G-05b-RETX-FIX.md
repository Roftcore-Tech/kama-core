# K4G-05b — Yeniden-gönderim sahipliği (transport v1.1) — tasarım notu

## 0. Künye
Kart **K4G-05b** · taban `d46ec628` (satırlar buradan; kart dosyaları bugün dalda özdeş; `k4g/05-int02` rebase'i sonrası ofset: `sender.rs` ≥ 760 +56, ≥ 4627 +132; `arq.rs` ≥ 102 +6; `runtime.rs` :1290-1336 +457, :1785/:1835 +563; spec ve Ek A hedefleri değişmez) · **TASLAK — `d6d60be8` notunun CEVAP PAKETİ 7 + EK (2026-10-18) revizyonu;** kod "K4G-05b ONAYLANDI", kapanış ayrı "INT-02 GEÇTİ" bekler. Uygulanmadı, ölçülmedi. Gövde 1 534 kelime (`d6d60be8` 1 210, ≈ 1.27 ×); **Ek A** aynen (171 satır, sha256 `b68340c5c9efe7f8982de43f03e4905dfe609cb2c12ff9139db16c46f40861f7`); **Ek B** DUR-1..10 kapanışları + B.2 §6 tanımları. Etiket: [ölçüm] · [kod] · [tasarım] ölçülmemiş · [çıkarım] koşulmamış.

## 1. Değişmez (sahip metni, CEVAP PAKETİ 5, 2026-10-16)
> "Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir."

Okuma: sessizlik → canlılık kopyası (Duplicate, kayıp değil; INV-001, `tv_loss.rs:51-56`); later-ACK → dedektörle ilan + yeniden plan. Reliable = her motor birimi; RecoverFwd girişine ve ACK sessizliğine dayanmaz; izleme teyide ya da oturum sonuna (INV-NO-IDLE-TEARDOWN) dek.

## 2. Mekanizma (satırlar `sender.rs`)
- **D1 — öksüz prob kopyası** [ölçüm diag-72cb772c gating2 s3/s9, m12 s3 u71; kod; "cap 3 → HandToForward" bildirimim YANLIŞTI]: en yeni kayıt prob (:5796), later-ACK yok → ilan yok (`loss.rs:692`), prob bütçesi yalnız uçuş küçülünce sıfırlanır (:3166), canlılık yalnız Data kopyalar (:5822); gizli varyantlar :5696 (fire-once), :5699 (düşürme).
- **D2 — sıra başı (HOL) açlığı** [ölçüm kesinti s1/s3: dup_queue 94→261 / 104→335, HOL 125 s; kod]: fire-once kopyalar Vec sonunda, dedupe yok (:5836-5847), FIFO + ilk token reddinde `break` (:5943-5948); kopyalar DATA/ARQ/prob/kuruluş probundan sonra, FEC parity'den önce (:4236-4251; parity :4255); aralık tavansız (:5966).
- **srtt 60.5 s, Karn değil:** en yüksek yeni-onaylı Data örneklenir (:3034/:3057/:3117), `ack_delay` en büyüğün (`receiver.rs:405`), n = 64 (`rtt.rs:145`). **ESI:** parite `unit.id = esi` (:5255) `acked_units`'e girer (:3027/:3043) → zincir iptali (:3154); prob pariteyi hedefleyebilir (:3228).

## 3. Tasarım [tasarım] (`sender.rs` :taban — değişiklik — madde)
1. :3027/:3043, prob hedefi :3228 — FecParity teyit/hedef olmaz — ESI.
2. `confirmed_units` (:3393/:3437, :5762) — teyitli birimin kalan kayıtları yalnız muhasebede; birim son kaydıyla kümeden düşer (sınırlı) — D2.
3. `plan_arq_retransmits` :5685-5699, `plan_forward_duplication` :5836 — her politikada tek kalıcı geometrik sahip, düşürme yok; `RetxConfig::fixture` (cap 3, KALİBRE DEĞİL, :1860) kalır, cap yalnız yolu değiştirir; :5963-5971 PreConverge dışına genelleşir — D1.
4. `adopt_unowned` (:4244 ardı, PreConverge/Idle dışı) — (a) canlılık `origin != FecParity`; (b) prob bütçesi bitince aynı turda devir; (c) son ilerleme/iletimden H sonra sahiplen (uyanma kaynağı 7); (d) geometrik kaydı varken NO-OP — D1, M1.
5. `drain_due_duplicates` :5932 — PreConverge dışı en küçük birim kimliği önce, birim/tur ≤ 1, token reddinde `break`, aralık ← min(2 × aralık, RTO_MAX); baş tek `copy_queue_head` seçimiyle (drenaj, B1 tabanı, uyanma kaynağı 3 :2566-2577) — D2, B1.
6. Sıfırlama: mandal H'de; :3153-3155 ardında ilk ilerlemeli ACK aralıkları tabana döndürür; prob v1 (:3166) — D2.
7. RTT :3010-3149 — KABUL KURALI (A.3; (c) = köken Data, `retx_count` 0, uçuşta başka kayıt yok) — srtt.
8. `on_event_core` :2287 — PreConverge çıkışında bekleyen kopyalar RTO_MAX'a kırpılır.
9. `track_sent` :6079 — PreConverge dışı yalnız vadeli kopya "bekleyen iş" — INV-RATE-1.
10. `emit_tail_probe` :5796 `saturating_add`; `arq.rs:27-34` yalnız doc.

**B1 — imkânsızlık ispatlanamaz (karşı örnek Ek B.2), delik kapatılır** [kod]: BURST_CAP tabanı yalnız tx başı (:2310, :2322-2325). A.4: taban = max(tx başı, yolun başı: kopya sırası · ARQ planı · prob hedefi), yol girişinde yalnız yükseltir, token hibe yok. PreConverge'de taban v1 = max(tx başı, iw) (:2317-2322); üründe iw = 2·MTU = 2400 B (`runtime.rs:237/:260/:1332`) ≥ birim (≤ 1061 B [çıkarım; `runtime.rs:227/:234/:1835`]) → delik yok; motor varsayılanı iw = 0 (:2034) beyanlı artık (A.4, A.7); çıkış turu (:2288) tek tur.

**B2 — birim başına kopya kaydı ≤ N = 3** (ekleme yerleri Ek B.2). **G (geometrik) ≤ 1:** tek yuva (§3.3-B, P0, canlılık/`adopt_unowned`, HandToForward, drenaj i = 0); ekleme G varken yeni G açmaz — kanıt yolları (P0, HandToForward, drenaj) vadeyi min(mevcut, kendi)'ne çeker, sahiplenme yolları NO-OP. **F (fire-once) ≤ D − 1** (:6005-6013; parti yalnız bekleyen F yokken). ARQ kuyruğa girmez, FecParity kayıt almaz. N = 1 + (D_max − 1) = D_max = max(`DEFAULT_DUP_FACTOR` 2 :1425; `dup2_factor` 2 / `dup3_factor` 3 `emergency.rs:120-121`, "**KALİBRE DEĞİL**" :116) = **3**. Sapma → DUR-13.

**Kayıt büyümesi (EK A1; O(1) koşulu geri çekildi):** her emisyon taze seq + iki kayıt (:6074-6075, :6102, :6104), ilan later-ACK ister; v1 tavansız (:5966) → logaritmik, v1.1 doğrusal; A.7 = sahip cümlesi aynen + kesin biçim m + ⌊Δ/RTO_MAX⌋ (m = ⌈log2(RTO_MAX/b)⌉ = 7, b = 0.4 s) → DUR-11.

Sınırlar: RTO_MAX = `validated_v1().max_backoff_pto_ns` (`tail_recovery.rs:78`); H = max(3·srtt, 2·EAI)·1000/300 ≥ 2·(65/31 s)·1000/300 ≈ 13.978 s (:6139-6150; EAI tabanı :1440/:1442, `K_CONF_EAI` :1489, `emergency.rs:93`); ilk yeniden gönderim ≤ 60 s (prob açık) / H, sonra ≤ RTO_MAX + token bekleme.

## 4. Dokunulan dosyalar (tam liste)
- `crates/adaptive-transport/src/sender.rs`, `src/arq.rs` (doc), yeni `tests/k4g05b_*.rs`; DUR-8 (a) ile `tests/dr_cs1_acceptance.rs`.
- Kimlik/freeze: `crates/client-core/src/runtime.rs:217/:220`, `tests/transport_v1_freeze.rs:20-21` (`api_surface` pin güncellemesi yok); kimlik adı: `crates/bench/src/{manifest,main,event,kama_product}.rs`, `.github/workflows/nightly.yml:201/:214`.
- Faz-5 betikleri ve v1 bankası bayt-özdeş; v1.1: `reports/transport-v1.1/tcell/` + `tools/` (yalnız etiket + ROOT farklı kopyalar; ön-kayıt içeriği Ek B satır 6).
- Spec (Ek A): engine A.1–A.4, A.8 (§16.2 satırı), A.9a; TDD A.5–A.7; işaretçiler A.9b–f (`docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD.md`, `docs/transport-v1/{ARCHITECTURE,PRODUCT-PROFILE,INVARIANTS,README}.md`); DR-CS-1 A.10 (`reports/decision-requests/DR-CS-1-minimal-bootstrap.md`); EK4-A2 md sha256-pinli (`tests/ek4_a2_liveness.rs:4`) → dokunulmaz.
- `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md` (B7; orkestratör) ve bu not.
- **B3** [kod; tüm ağaç]: iki dizge ve iki sabit yalnız tanım, freeze testi, bench etiketleri (stderr `kama_product.rs:352-353`, yerel JSONL `event.rs:148-151`), `nightly.yml` ve Faz-5 etiketlerinde (`phase5_analyze.py:14`, `phase5_cell_report.py:6`; ikisi de `tools/`'a yalnız etiket — ve :4 ROOT — farklı kopyalanır) geçer; tele, Noise prologue'una (`crates/`'ta yok), CCR'ye, el sıkışma alanına ULAŞMAZ (kimlikler sayısal: `kama-noise-wire/src/handshake.rs:39/:41/:43`, `kama-transport-schema/src/param.rs:24`). DUR yok.
- Başka dosya yok; tel, codec, `tv_*`, S1/S3/noise-wire/ccr/kama-node, FEC parite, DR-1x, `dr_cs3_panel.rs` dokunulmaz.

## 5. Spec yama metinleri → Ek A
Ek A = son metin; kaynağı bu nottur (rev3 `patches.py` `4f1a262b…` `floor` çıktısı `cc1853b0…` bu Ek A değildir). Silme kapısı: 20 blok nottan okunup rev3 çapa/`apply()`/`text_checks` ve taban `word_coverage.py` ile `git show d46ec628` metnine bellekte uygulandı: 8 hedefte kayıp 0 / yer değiştirme 0 (engine +47, TDD +52, altı işaretçi/DR-CS-1 dosyası +2'şer, `word_coverage` kapsamı dışı); iki çizgisiz kontrol kırmızı; metin denetimleri geçti. Ek A 171 satır, sha256 `b68340c5c9efe7f8982de43f03e4905dfe609cb2c12ff9139db16c46f40861f7`.

## 5b. Ölçüm (CEVAP 6; CEVAP 7 kabul)
`~/kama-k4g-work/int02/meas-1f6023ff/m12.md` (beklenti `cb2e4fc1…` önceden kilitli): D1 tekrarlandı (s3 71/100); bayat SACK ilerleme üretmedi; ≈28 B/s yeniden üretilmedi → hız işi yok; kesinti kırmızılarının artefakt payı INT-02 raporunda.

## 6. Kanıt
**Kural:** sahte saat + gerçek `Receiver`; önce tabanda kırmızı, değilse DUR. Taban sonuçları **beklentidir** [çıkarım; koşulmadı]. Düzenek (adaptive-transport içinde, yeni bağımlılık yok), simgeler, koşullar: Ek B.2.
- **D1** `k4g05b_d1_no_orphan_600s` (prob açık/kapalı × bayat SACK/sessizlik): owned(U) her olayda + U'nun yeniden emisyonu ≤ §3 sınırı (ilk ≤ 60 s prob açık / H, sonra ≤ RTO_MAX + token bekleme). Tabanda kırmızı beklenir: prob açık — bütçe bitince 600 s'ye dek 0 yeniden gönderim; kapalı × bayat SACK — 0; kapalı × sessizlik = kontrol (:5816-5831).
- **D2** (EK B5, DUR-9 a): kol 1 (60 s kesinti) · kol 2 (60 s + DATA ≤ min_rate/2) · kol 3 (600 s kesinti, A1(3) doğrusal sınır). Kol 1–2: T_a sonrası ilk Duplicate u₀'nın, sınırlı sürede, owned(u₀); tabanda kırmızı beklenir [kod: :5822, :5843-5847, FIFO + `break`]. Kol 3 kayıt sınırı tabanda yeşil beklenir (bekçi). **Tabanda kırmızı yapılamazsa D2 koda girmez → DUR.** **Sınıflar arası doygunluk (DUR-9 b) bu testin konusu değildir.**
- **srtt** `k4g05b_d2_stale_rtt_after_outage` (≥ 4 bayat orijinal → srtt ≤ 1 s) · **ESI** `k4g05b_esi_parity_ack_does_not_confirm_source_unit` (+ prob hedefi parite olmaz).
- **Soğuk başlangıç** `k4g05b_cold_start_srtt_converges` (DUR-1 a, EK B6; düzenek ve kollar Ek B.2): **N = 64** kabul edilmiş örnekte RTT ≤ srtt ≤ RTT + ⌈(7/8)⁶⁴·(srtt_r − RTT)⌉ + 256 µs; tabanda kırmızı beklenir, yeşilse DUR.
- **B1** `k4g05b_b1_copy_head_within_burst_cap` (+2 kol DUR-12 a, PreConverge kontrolü) · **H** `k4g05b_adopt_after_h` (ilk sahiplenme [H, H + 1 tik]). Sayım: D1, D2 (3 düzenek), srtt, ESI, soğuk başlangıç, B1, H = 9 + 16 dar = 25.
- **Mevcut testler + `tv_*` değişmeden yeşil** (kırmızı = DUR); `kama-integration` PR CI dışı (`ci.yml:94`; yalnız merge sonrası `main.yml:46-47`) → crate bütünüyle tabanda ve düzeltmede PR öncesi koşulur.
- **DUR-8 (a)** dört koşul (Ek B satır 8): PR paragrafı, çıktılı 2×2 (dört hücre), mutasyon, A.10.
- **B8:** `dr_cs3_panel.rs:996-1001` `lineage` referanstır → Ek A işaretçisi yok (dosya kapsam dışı), bayat-atıf listesinde (DUR-4 b); `f3`/`f4` (:968-969) `control_12b` tavanına girer (:3372) → panel tabanda ve düzeltmede koşulur, kırmızıysa DUR-15.
- **Mutasyon (B4):** 38 (cap dalı, kopya planlaması, HOL sırası, sıfırlama, RTT, RTO tavanı) + 18 (ESI 2 · H eşiği/sahiplenme 4 · B1 5 + 2 · B2 3 · RTT (e) 1 · `confirmed_units` 1) = 56 (DUR-12 b: 54).
- **T-hücre** tam set bu kartta (≈ 5 sa 40 dk); **INT-02** 10 × 600 s 10/10; 5 × 60 s kesinti aynı oturumda %100; dosya kolu (D3) aynı koşuda ölçülür ve raporlanır (açlık → DUR-9 b).

## 7. Süre
≈ 4.5–5 iş günü (kod + test 2.5–3 g · mutasyon + kapılar 1 g, `kama-integration` dahil, süresi ölçülmedi · T-hücre gece · INT-02 ≈ 2–3.5 sa · rapor 0.5 g).

## 8. Sıra
Bu revizyon `d46ec628` tabanında, int02 merge'ünden önce. EK E: `k4g/05-int02` PR → merge → rebase (§0 ofseti) → push → SHA + Ek A sha256 tek satır → "K4G-05b ONAYLANDI" → tek PR (§6 sırası) → T-hücre → INT-02 → rapor → "INT-02 GEÇTİ" → K4G-16. PR açıklaması: EVOLUTION §22 altı beyanı + bayat atıf listesi + karar metinleri yolu `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md` (B7).

## 9. DUR
**DUR-1..10 CEVAP PAKETİ 7 + EK ile kapandı (Ek B).** Yeni (öneri kalın; Ek A kalın seçenekleri içerir, başkası Ek A sha256'sını değiştirir):
- **11 — doğrusal sınır:** A1(1) yalnız eğim; rampasız A1(3) v1.1'de kırmızı (b = 0.4 s: 50.4 s'de 7 > 2) [aritmetik]. **(a) A.7 kesin biçimi normatif; "≈ 1000" sahip tahmini (türetilmedi), "≤ 1" eğim olarak işaretli** · (b) kesin biçimi çıkar; "≈ 1000" türetilir: U × (m + ⌊60/30⌋) + zincir dışı, U ölçülmedi (≤ 4096, `runtime.rs:248`).
- **12 — ARQ (:5656-5661) ve prob (:5775-5781) aynı BURST_CAP deliği** [kod: `try_send_pre_parity` → `pacer.try_send` :5629]. **(a) taban ikisini de kapsar (A.4; +2 kol, +2 mutant)** · (b) beyan + ayrı kart: A.4 cümlesi çıkar, A.7'ye ARQ/prob istisnası girer.
- **13 — B2 sapması:** F birim başına ≤ D − 1 kayıt (:6007); P0 v1'de kayıt varken no-op (:3393), v1.1 vadeyi çeker. **(a) kabul: F ≤ D − 1, N = D_max = 3** · (b) F birim başına tek kayıt (N = 2).
- **14 — "ilk örnek zaten RTO_MAX ile sınırlı" kodda yok** (RTO_MAX yalnız `tail_recovery.rs:172`). **(a) beyan; büyük tohum aynı yasayla yakınsar** · (b) tohumu kırp (DUR-1 a ile çelişir).
- **15 (koşullu)** — panel düzeltmeyle kırmızıysa DUR-8 (a) dışı (eşik değişir) → sahibe.

## Ek A — yama metinleri (aynen; kaynak bu not — `patches.py` rev3 varyant `floor` çıktısından A.1, A.4, A.7 ve A.9a'da ayrışır; `<ONAY-TARİHİ>` = onay günü; sha256 kapsamı bu satırdan `## Ek B` satırından önceki satıra dek: 171 satır, `\n` ile birleşik, sonda newline yok — değer §0/§5'te)
**A.1 engine — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — RTT örnek kabulü + kopya kuyruğu sırası + §16.2 saat-sınıfı satırı; sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 7, 2026-10-18 (DUR-1 (a), DUR-3 (b), DUR-9 (a)); CEVAP PAKETİ 7-EK, 2026-10-18 (B1) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §6 Girdi satırına **v1.1 KABUL KURALI** eklendi (çerçevenin gönderilmiş en büyük onaylı seq'i · bu çerçeveyle ilk kez onay · birimin tek iletimi olan orijinal DATA · doygun olmayan ack_delay · tutma süresiyle şişmemiş örnek; geri düşüş yok; srtt yokken v1 seçimi aynen); §6 min-rtt guard'ının RAW kolu ve "örnek ATLANMAZ" hükmü bu kurala bağlandı; §5 TUR-2a/TUR-2b kol-1 RTT'si bu kurala bağlandı (ürün yolunda yalnız ters-yol ACK yeniden sıralamasında erişilir; BW-örneği ve delivered-kredi DEĞİŞMEDİ); §5 ZEYL'deki "tam örnek verir (belirsizlik yok)" üstü çizildi; §13'e kopya kuyruğu içi sıra eklendi (en eski onaysız birim önce, BURST_CAP alt sınırı kopya sırasının, ARQ planının ve kuyruk-probu hedefinin başını kapsar — PRE_CONVERGE'de v1, beyanlı; RTO_MAX tavanı; sınıflar arası sıra DEĞİŞMEDİ); §16.2 saat-sınıfı tablosuna TEK satır eklendi (kopya kuyruğu aralık tavanı + sahipsizlik süresi). Otoriter v1.1 kaydı: transport-layer-tdd §12 [AMENDMENT-12]. Kuantum döngüsü, tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Üstü çizili metin silinmedi.
```
**A.2 engine §5/§6 — taban satırı yerine (eski kelimeler sırasıyla korunur; çizilen `~~…~~`):**
- taban :464 →
```text
  RTT örneği GEÇERLİ [kol-1] **[v1.1: yalnız §6 KABUL KURALI sağlanırsa — kayıt ilan anında largest_acked'in
  altındadır, alıcının en büyüğü monoton büyür (INV-003): ürün yolunda yalnız ters-yol ACK yeniden sıralamasında
  erişilir]**); pencere-dışına yaşlanan olay düşer (Expired → doğal decay).
```
- taban :473 →
```text
  RTT **[v1.1: yalnız §6 KABUL KURALI sağlanırsa (TUR-2a erişim notu); BW-örneği koşulsuz]** (sansür biter —
  K-1 sansür-kolu kapanışı); **kol-2 [TUR-2c, NORMATİF]** (superseded —
```
- taban :479 →
```text
  (superseded iletimin RTT'si güven-dışı). Retransmit-seq'in KENDİ ACK'i inflight-yolundan ~~tam örnek~~
```
- taban :480 →
```text
  ~~verir (belirsizlik yok).~~ **[v1.1] BW-örneği verir, RTT örneği VERMEZ — birden çok iletimi olan birimden
  RTT alınmaz (§6 KABUL KURALI (c); kod v1'de de RTT'yi yalnız origin = Data kaydından alıyordu).**
  Süresi/tavanı dolan kaydın geç-ACK'i → Expired/Unknown (örneksiz;
```
- taban :538 →
```text
Girdi: ~~her acked paket için~~ **[v1.1: aşağıdaki KABUL KURALI'nı sağlayan iletim için; ACK çerçevesi başına ≤ 1]**
       rtt_sample = ack_recv_time − pkt.send_time − ack.ack_delay
```
- taban :542 →
```text
       → RAW sample işlenir **[v1.1: srtt VARKEN ack_delay ≥ rtprop ise örnek ALINMAZ — KABUL KURALI (e)]**, örnek
       ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı YASAK) **[v1.1: yasak kabul edilmiş örneğe aittir;
       KABUL KURALI reddi beyanlı estimator-donmasıdır]**.
```
**A.3 engine §6 — :545 ardına ekle (kod çiti içinde):**
```text
       [v1.1 · K4G-05b, NORMATİF — sahip kararı CEVAP PAKETİ 5, 2026-10-16] KABUL KURALI (srtt VARKEN): ACK
       çerçevesi başına en çok BİR örnek; yalnız şu beş koşul birlikte sağlanırsa alınır — (a) kayıt, çerçevenin
       gönderilmiş (sender'ın ürettiği) en büyük onaylı seq'idir (en yeni iletim, §3.1; ack_delay onundur, §2.1);
       (b) o seq bu çerçeveyle İLK KEZ onaylanır (uçuştaki kayıt, kol-1 tombstone ya da un-loss eşleşmesi);
       (c) kayıt birimin TEK iletimi olan orijinal DATA'dır (birimin uçuşta başka iletimi yok, birim daha önce
       teyit edilmemiş; FEC onarım sembolü birim sayılmaz); (d) ack_delay kodlama tavanında DEĞİLDİR (65535 ×
       256 µs ≈ 16.78 s, §2.1 satürasyonu: alıcının tutma süresi bilinmez); (e) min-rtt guard düzeltmeyi
       reddederse ack_delay < rtprop olmalıdır — aksi hâlde RAW, tutma süresiyle en az rtprop kadar şişmiştir
       (FLOOR kodlama: decoded ≤ gerçek tutma, §2.1). Daha küçük yeni-onaylı kayda GERİ DÜŞÜLMEZ; bayat örnek
       alınmaz. Kesinti sonrası ilk taze örnek döngüyü yeniden başlatır: DEĞİŞMEYEN kuantum döngüsü Δt ≥ 64·T
       iken n = 64 uygular. Beyanlı sonuç: çerçevenin en büyüğü kopya, yeniden gönderim ya da FEC onarım
       sembolü olduğunda (kesinti sonrası; RECOVER_FWD'de DATA başına kopya; FEC açıkken) ve birimin kopyası
       uçuştayken örnek alınmaz — srtt donar, şişmez; TUR-2a/2b kol-1 RTT'si ürün yolunda yalnız ters-yol ACK
       yeniden sıralamasında erişilir. srtt YOKKEN (ilk örnek, §3.3-D kilidi) v1 seçimi AYNEN geçerlidir [sahip
       kararı CEVAP PAKETİ 7, 2026-10-18, DUR-1 (a): PRE_CONVERGE kopyaları (§3.3-B, DR-EST-1) aksi hâlde ilk
       örneği kilitleyebilir]. Karar metinleri: `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`.
```
**A.4 engine §13 — :959 ardına ekle (:960 kapanış çitinden önce):**
```text
[v1.1 · K4G-05b, NORMATİF — sahip kararları CEVAP PAKETİ 5, 2026-10-16; CEVAP PAKETİ 7, 2026-10-18 (DUR-9 (a));
  CEVAP PAKETİ 7-EK, 2026-10-18 (B1) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`] KOPYA KUYRUĞU İÇİ SIRA
  (PRE_CONVERGE DIŞINDA) VE BURST_CAP ALT SINIRI: vadesi gelmiş kopya kayıtlarından EN ESKİ ONAYSIZ birimin kaydı
  önce gönderilir (ölçüt: en küçük birim kimliği — ürün yolunda kimlik oturum içi gönderim sırasıdır; eşitlikte
  erken vade, sonra kuyruk sırası); bir birim bir planlama turunda en çok bir kez; token yetmezse o tur DURUR —
  arkadaki kayıt sıranın başını GEÇEMEZ, istisnasız. PRE_CONVERGE ve PRE_CONVERGE çıkış turu dışında her planlama
  turunda ARQ planlaması, kuyruk-probu ve kopya drenajı girişinde BURST_CAP ≥ max(kuyruk başındaki DATA biriminin
  boyutu, o yolun başındaki kaydın boyutu — ARQ planının ilk kaydı, bekleyen kuyruk-probu hedefi, yukarıdaki
  ölçüte göre ilk vadesi gelmiş kopya kaydı): alt sınır üç yolun başını da kapsar, tavan gerekirse yalnız
  yükseltilir, token hibe edilmez; token'ın başa ne zaman yetişeceği sınıflar arası sıraya bağlıdır (aşağıda;
  kapsam dışı); pace_rate DEĞİŞMEZ (INV-RATE-1). PRE_CONVERGE'de taban v1'dir (max(kuyruk başındaki DATA
  biriminin boyutu, ilk-uçuş bütçesi)): ilk-uçuş bütçesi kaydın boyutundan küçükken BURST_CAP'i aşan kayıt bu
  kayıtla kapsanmaz (beyanlı istisna, transport-layer-tdd §12 [AMENDMENT-12]); çıkış turu tek turdur. Teyit
  edilmiş birimin kaydı gönderilmez. Geometrik aralık ×2, üst sınır RTO_MAX (transport-layer-tdd §12
  [AMENDMENT-12]). PRE_CONVERGE'de §3.3-B sırası ve takvimi DEĞİŞMEZ; PRE_CONVERGE çıkışında bekleyen kopyaların
  aralığı ve vadesi RTO_MAX'a kırpılır. Sınıflar arası sıra DEĞİŞMEDİ: v1 tahkimi kod sırasıdır (DATA → ARQ →
  kuyruk-probu → kuruluş probu → kopya kuyruğu → FEC parity; RECOVER_ARQ'da her K_ARQ_SHARE'inci tur ARQ ile
  başlar) — yukarıdaki priority satırıyla ayrışması bu kaydın kapsamı dışındadır.
```
**A.5 TDD — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — yeniden-gönderim sahipliği; sahip kararları CEVAP PAKETİ 4 ve 5 (2026-10-16), CEVAP PAKETİ 6, 7 ve 7-EK (2026-10-18) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §12'ye [AMENDMENT-12]: INV-NO-ORPHAN (sahip metni) · ARQ_RETX_CAP aşımı ve kuyruk-probu bütçesinin tükenmesi yolu değiştirir, birimi düşürmez · **RTO_MAX = 30 s** normatif sabit (koddan belgeye: `TailRecoveryConfig::validated_v1().max_backoff_pto_ns`; yeni sayı yok) · kesinti sonrası ilk taze ACK'te kopya geri çekilmesi sıfırlanır · FEC onarım sembolünün ACK'i kaynak birimi teyit etmez · beyanlı sınır: süresiz kesintide uçuş kaydı büyümesi v1.1'de doğrusal, AMENDMENT-9 uygulama kartı DEMO-01 ön koşulu; kimlik `kama-v1-full` → `kama-v1.1-full`, `transport-v1` → `transport-v1.1` (AMENDMENT-11'in beyan ettiği `transport-v1.x` de bununla sabitlere işlenir). §5.3 "RTO↑" satırına tavan işareti. Engine-spec §5/§6 (RTT kabul kuralı), §13 (kopya kuyruğu sırası) ve §16.2 (tek saat-sınıfı satırı) v1.1 yamaları aynı kartta; yetki dışındaki belgelere tek satırlık AMENDMENT-12 işaretçileri, DR-CS-1 belgesine kabul fikstürü YAMA KAYDI. Tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Bu belgede metin silinmedi (yalnız ekleme).
```
**A.6 TDD §5.3 — :623 satır sonuna ekle:**
```text
   [v1.1: RTO↑ ≤ RTO_MAX = 30 s — §12 AMENDMENT-12]
```
**A.7 TDD §12 — :846 önüne ekle (+ boş satır):**
```text
> **[AMENDMENT-12 — <ONAY-TARİHİ>, v1.1 · K4G-05b: YENİDEN-GÖNDERİM SAHİPLİĞİ + RTO_MAX · NORMATİF · DAVRANIŞ DEĞİŞTİRİR · TEL DEĞİŞMEZ]**
>
> **Dayanak:** sahip kararları CEVAP PAKETİ 4 ve 5 (2026-10-16), CEVAP PAKETİ 6, 7 ve 7-EK (2026-10-18) —
> `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`; tasarım notu `docs/design/K4G-05b-RETX-FIX.md`.
> **INV-NO-ORPHAN (sahip metni, CEVAP PAKETİ 5, 2026-10-16):** Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim
> zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden
> gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir.
> **Okuma (NORMATİF):** kapsam motorun taşıdığı her birimdir. Sessizlik kayıp İLAN ETMEZ (INV-001): sessizlikte izleyen
> yol birimi canlılık için yeniden gönderir (kopya; loss sayılmaz); later-ACK kanıtı gelince izlenen kayıt — kuyruk-probu
> kopyası dahil — mevcut dedektörle kayıp ilan edilir ve yeniden planlanır. ARQ_RETX_CAP aşımı (her politika) ve
> kuyruk-probu bütçesinin tükenmesi birimi kopya kuyruğuna devreder, ASLA düşürmez; prob bütçesi tükenince devir en geç
> probun bir sonraki (geri çekilmiş) vadesindedir. **Sahipsizlik süresi** H = max(ACK_SILENCE_SRTT_MULT·srtt, K_CONF_EAI·EAI) ×
> 1000 / `HysteresisConfig::escalate_permille` (kodun ack_confidence eşiği; saf sessizlikte RECOVER_FWD'nin girdiği an;
> eşik yeniden kalibre edilirse H onu izler): ilerleme olmadan (ilerleme = en az bir iletim kaydını ilk kez onaylayan
> ACK; bayat ACK sayılmaz) birimin son iletiminden beri H geçince birim kopya kuyruğuna alınır. Değişmez RECOVER_FWD
> girişine ve ACK sessizliğine DAYANMAZ. İzleme teyide ya da §0 kapalı listesindeki bir sonlandırmaya dek sürer
> (INV-NO-IDLE-TEARDOWN). Teyit = birimin bir kaynak iletiminin ACK'i ya da FEC RECOVERED bildirimi; FEC onarım
> sembolünün ACK'i kaynak birimi teyit ETMEZ (INV-004). Teyit edilmiş birimin kalan kayıtları yalnız muhasebeye girer
> (dedektör, ledger ve kader-kanıtı v1 gibi), yeniden planlanmaz. Birim başına ≤ 1 geometrik kopya kaydı; birim bu
> kayda sahipken sahiplenme yolları (sessizlik canlılığı, sahipsizlik devri) kaydın vadesine ve aralığına dokunmaz,
> vadeyi yalnız later-ACK kayıp kanıtı öne çeker. Bu kaydın getirdiği iletimler ortak token kovasındandır (INV-RATE-1);
> PRE_CONVERGE dışında vadesi gelmemiş kopya kaydı DATA örneğini app-limited dışı saydırmaz.
> **RTO_MAX = 30 s (NORMATİF sabit — koddan belgeye, yeni sayı DEĞİL; CEVAP PAKETİ 5, 2026-10-16, madde 2) [CAL — DR-ARQ-PRODUCT-TAIL-1, tuning pending]:**
> kaynak `TailRecoveryConfig::validated_v1().max_backoff_pto_ns` (PRODUCT-PROFILE "backoff tavanı 30 s", SHIP; pin
> `crates/client-core/tests/transport_v1_freeze.rs`). Üst sınırı olduğu aralıklar: (i) kuyruk-probu PTO geri
> çekilmesi (v1'de de); (ii) PRE_CONVERGE dışında kopya kuyruğunun geometrik aralığı ve tabanı (v1'de tavansızdı;
> PRE_CONVERGE çıkışında bekleyen kopyalar RTO_MAX'a kırpılır). Taban srtt-türevidir; taban-PTO tavanı `max_pto_ns` =
> 10 s ayrıdır; H kanal-türevidir ve RTO_MAX'la sınırlanmaz. ZM-2 4-decade doğrulaması BEKLİYOR (beyanlı: v1.1
> T-hücreleri D = 20 s ile bu tavana ulaşmaz; uzun pencereli hücreler DEMO-01 öncesi ön-kayıtlı gece kartında —
> CEVAP PAKETİ 7, 2026-10-18, DUR-3 (b)). Değeri değiştirmek ayrı sahip kararıdır.
> **Sıfırlama (NORMATİF):** (i) kuyruk-probu: uçuştaki kaydı azaltan ilk ACK probe_index'i sıfırlar (v1 kodu, sender
> `on_ack_received`; DEĞİŞMEDİ); (ii) kopya kuyruğu: ilerlemesiz süre H'yi aştıktan sonra en az bir kaydı ilk kez
> onaylayan İLK ACK bütün geometrik kopya aralıklarını tabana döndürür (sonraki kopya en geç son kopya + taban). Bayat
> ACK sıfırlamaz.
> **Beyanlı istisna (CEVAP PAKETİ 7-EK, 2026-10-18, B1):** PRE_CONVERGE'de BURST_CAP tabanı v1'dir; ilk-uçuş bütçesi
> kaydın boyutundan küçükken BURST_CAP'i aşan kayıt izlenir, ama token'a yetişmeyebilir (engine §13 v1.1 kaydı).
> **Beyanlı sınırlar (CEVAP PAKETİ 7-EK, 2026-10-18, A1):** süresiz kesintide uçuş kayıtları büyür — her yeniden
> gönderim taze seq ve kayıt alır, kaydın kayıp ilanı later-ACK ister (engine §14 gerilimi). v1'de aralık tavansız
> olduğu için büyüme logaritmikti; v1.1 RTO_MAX ile doğrusal (birim başına 30 s'de ≤ 1 kayıt; 60 s INT-02 kesintisi
> ≤ ≈ 1000) — canlılık için bilinçli takas. Parantezdeki sayılar sahip metnidir: "≤ 1" rampadan sonraki eğimdir
> (kesin biçim aşağıda); "≈ 1000" sahip tahminidir, türetilmedi (onaysız birim sayısına bağlıdır). Kesin biçim
> (K4G-05b D2 testi sınar): sessizlikte bir kopya zinciri Δ sürede birim başına ≤ m + ⌊Δ/RTO_MAX⌋ iletim kaydı
> üretir (sender uçuş tablosunda ve kayıp dedektörü haritasında ayrı ayrı); m = ⌈log2(RTO_MAX/b)⌉, b srtt-türevi
> kopya tabanı; ilk m kaydın rampası v1 ile aynıdır, fark zincir yaşı rampayı aşınca başlar; zincir dışı kayıtlar
> (orijinal, RECOVER_FWD ek kopyaları, ARQ ve kuyruk-probu yeniden gönderimleri) Δ ile büyümez. [AMENDMENT-9]
> K = 1024 tavanı uygulandığında bu yolların emisyonları ona TABİDİR (izleme sürer; tavan bugün SPEC-ONLY);
> AMENDMENT-9 uygulama kartı DEMO-01 ÖN KOŞULUDUR (Sprint 2 birikimi, "INT-02 GEÇTİ" sonrası). Bayatlayan atıflar ve
> EVOLUTION §22 altı beyanı: K4G-05b PR açıklaması; yetki dışı belgelerde tek satırlık AMENDMENT-12 işaretçileri
> (CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b)).
```
**A.8 engine §16.2 saat-sınıfı tablosu — :1144 ("geometrik kopya deadline'sızlığı") satırının ardına TEK satır ekle (:1145 SAFE satırından önce):**
```text
| [v1.1] kopya kuyruğu aralık tavanı + sahipsizlik süresi | sender dup_queue / adopt_unowned | RTO_MAX = 30 s wall-clock TAVAN (taban srtt-türevi); H = max(3·srtt, 2·EAI)·1000/escalate_permille kanal-türevi | ölü kanalda tam sessizlik / sahipsiz birim (v1) | 4-decade kanıtı BEKLİYOR (v1.1 T-hücreleri D = 20 s tavana ulaşmaz; uzun pencereli hücreler DEMO-01 öncesi ön-kayıtlı gece kartında) — transport-layer-tdd AMENDMENT-12 |
```
**A.9a engine — :3 önüne, A.1'in ardına ekle (+ boş satır; DUR-4 (b) işaretçisi):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Yama yetkisi (§5/§6/§13 + §16.2'de tek satır) dışında kalıp v1.1'de bayatlayan şu ifadeler DEĞİŞTİRİLMEDİ; v1.1 için otorite transport-layer-tdd §12 [AMENDMENT-12]: §2.1 `ack_delay_saturated` "beklenen 0" (en büyük seq'in alımından 16.78 s sonra gönderilen ACK doyar; v1.1'de doygun ack_delay'li örnek §6 KABUL KURALI (d) ile alınmaz) · §5.1 sessizlik eşiği `ACK_SILENCE_MULT × srtt` (v1 kodu: max(3·srtt, K_CONF_EAI·EAI); sahipsizlik süresi H bu eşikten türer) · §9.1 ARQ_RETX_CAP "veya connection-fail" (v1.1: hiçbir politikada düşürme yok, birim kopya kuyruğuna devredilir) · §12 "PTO backoff exponential (2^n)" (üst sınır RTO_MAX = 30 s) · §16.2 "geometrik kopya deadline'sızlığı" satırı (PRE_CONVERGE dışı aralık tavanı hemen altındaki v1.1 satırında) ve K-1 KAPANIŞ DURUMU'ndaki kol-1 RTT'si (§6 KABUL KURALI'na bağlı) · EK4-A2 uyanma envanteri (`reports/ek4-a2/EK4-A2-NEXT-WAKEUP-CONTRACT.md`; sha256-pinli, dokunulmadı): v1.1'de 7. kaynak = sahipsizlik vadesi; 3. kaynak (kopya kuyruğu) drenajla aynı sıra başını raporlar, aralığı PRE_CONVERGE dışında RTO_MAX'la sınırlı. Düzeltmeleri "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9b `docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §4 karar bloğunun RESET/CANCEL, BOUND, PROBE LOSS ("then give up (bounded failure) — no infinite retransmit") ve FAIL-CLOSED ("never unbounded emission") satırları v1.1'de AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): MAX_TAIL_PROBES bütçesi tükenince birim düşürülmez, kopya kuyruğuna devredilir ve teyide ya da oturum sonuna dek RTO_MAX = 30 s tavanlı aralıkla yeniden gönderilir; prob kopyası da later-ACK kanıtıyla kayıp ilan edilip yeniden planlanır; sıfırlama kuralları AMENDMENT-12 "Sıfırlama" paragrafındadır. Prob mekanizmasının kendi bütçesi (`max_tail_probes`) DEĞİŞMEDİ. Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9c `docs/transport-v1/ARCHITECTURE.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Bu belge v1'i anlatır; v1.1'de şu anlatımlar AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): §4.5 "cap aşılırsa `HandToForward`" (her politikada birim kopya kuyruğuna devredilir, düşürülmez; FEC onarım sembolünün ACK'i kaynak birimi teyit etmez) · §4.6 "en fazla 3 probe" (bütçe bitince birim kopya kuyruğuna devredilir) · §4.9 "Öncelik/tahkim mevcut kod sırasıdır" (sınıflar arası sıra DEĞİŞMEDİ; kopya kuyruğu İÇİNDE en eski onaysız birim önce, aralık tavanı RTO_MAX = 30 s — engine §13) · RTT örneği engine §6 v1.1 KABUL KURALI'yla alınır. Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9d `docs/transport-v1/PRODUCT-PROFILE.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** Bu sözleşme v1'i (`kama-v1-full`) tanımlar; v1.1'de KURAL satırındaki ve "Kimlik" tablosundaki kimlik `kama-v1.1-full` / `transport-v1.1`'dir (`TRANSPORT_V1_PROFILE_ID` / `TRANSPORT_V1_ARCHITECTURE`). `RuntimeConfig` alanları ve tablolardaki değerler değişmez; davranışı değişen satırlar AMENDMENT-12'ye tabidir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): Tail-recovery (`max_tail_probes 3` bütçesi bitince kopya kuyruğuna devir; "backoff tavanı 30 s" = normatif RTO_MAX, aynı zamanda kopya aralığı tavanı) · ARQ ("cap aşımı → HandToForward" düşürmesiz, kalıcı kopya sahipliğine devir). Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9e `docs/transport-v1/INVARIANTS.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** v1.1'de AMENDMENT-12'ye tabi (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]): INV-008/INV-009 kanonik kimliği `kama-v1.1-full`'dür (`TRANSPORT_V1_PROFILE_ID`; tablodaki `kama-v1-full` v1 içindir) · INV-012 "RTT Karn-dışlamalı" engine §6 v1.1 KABUL KURALI'yla daraltıldı · INV-004: FEC onarım sembolünün ACK'i kaynak birimi teyit ETMEZ (motor ACK yolunda ESI'nin birim kimliğiyle çakışması v1.1'de kapatılır) · yeni INV-NO-ORPHAN (sahip metni) AMENDMENT-12'dedir. Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.9f `docs/transport-v1/README.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — AMENDMENT-12 işaretçisi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** "Durum" ve "Kanonik kimlik" satırları transport v1'i tanımlar; v1.1'de kod kimliği `kama-v1.1-full` / `transport-v1.1`'dir (`TRANSPORT_V1_PROFILE_ID` / `TRANSPORT_V1_ARCHITECTURE`); yeniden-gönderim sahipliği, RTT örnek kabulü, kopya kuyruğu sırası ve RTO_MAX = 30 s AMENDMENT-12'dedir (otorite: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]). v1 kanıt bankası (`reports/transport-v1/`) ve Faz-5 betikleri bayt-özdeş kalır; v1.1 T-hücre kanıtı `reports/transport-v1.1/tcell/` altında ayrı etiketlidir (CEVAP PAKETİ 7, 2026-10-18, DUR-6 (a); CEVAP PAKETİ 7-EK, 2026-10-18, A2). Bu belgenin metni değiştirilmedi; düzeltmesi "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı.
```
**A.10 `reports/decision-requests/DR-CS-1-minimal-bootstrap.md` — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — kabul fikstürü düzeltmesi; sahip kararı CEVAP PAKETİ 7, 2026-10-18, DUR-8 (a) koşul 4 — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** v1.1'de kabul fikstürleri düzeltildi; neden: teyit edilmiş birimi yeniden planlayan v1 davranışına dayanıyordu. Kapsam: `crates/adaptive-transport/tests/dr_cs1_acceptance.rs` DI-1 (`f2b_di1_first_data_loss_bootstrap_recovery`), DI-1C (`f2b_di1c_rearmed_pto_lifecycle_exposure`), DI-2 (`f2b_di2_single_ack_erasure_spurious_expected`); yalnız ACK'lenen seq'lerin seçimi değişir, assertion ve eşikler DEĞİŞMEDİ; eski/yeni kod × eski/yeni fikstür matrisi ve mutasyon çıktısı K4G-05b PR açıklamasında. Otoriter v1.1 kaydı: `specs/transport-layer-tdd.md` §12 [AMENDMENT-12]. Bu belgenin metni değiştirilmedi.
```

## Ek B — DUR-1..10 kapanışları (sahip kararı CEVAP PAKETİ 7, 2026-10-18, EK ile düzeltilmiş hâli; `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`)
| # | bulgu | sahip kararı (son) | notta |
|---|---|---|---|
| 1 | srtt yokken katı kural ilk örneği kilitleyebilir (§3.3-D, §3.3-B, DR-EST-1 :1434) | **(a)**: srtt yokken ilk örnek v1 seçimiyle, ilk örnek varken yeni kural; soğuk başlangıç + %30 kayıpta srtt'nin N örnekte yakınsadığını gösteren regresyon testi (N notta). **EK B6:** bayat tohumdan sonra N örnekte toparlanma aynı testte. Artık iki yol [kod]: (i) > 16.78 s tutulan ilk ACK (`receiver.rs:459-466`); (ii) ilk çerçevenin en büyüğü prob/kopyayken v1 daha eski orijinali alır (:3034, :5894). "İlk örnek RTO_MAX ile sınırlı" öncülü → DUR-14 | A.3; §6 soğuk başlangıç (N = 64; kollar A/B/C Ek B.2) |
| 2 | süresiz kesintide uçuş kayıtları ve `loss.rs:210` haritası büyür | **(a)**; CEVAP 7'nin "birim başına O(1) durum, deneme listesi yok" koşulu **EK A1 ile GERİ ÇEKİLDİ**. Üç ek: (1) A.7 sayısal + yön (v1 logaritmik, v1.1 doğrusal; birim başına 30 s'de ≤ 1; 60 s ≤ ≈ 1000; bilinçli takas); (2) AMENDMENT-9 K = 1024 uygulama kartı Sprint 2'ye, DEMO-01 ÖN KOŞULU, sahibi bu oturum, "INT-02 GEÇTİ" sonrası; (3) D2'ye 600 s doğrusal sınır iddiası | §3; A.5, A.7; §6 D2 kol 3 (Ek B.2); rampa ve "≈ 1000" → DUR-11 |
| 3 | §16.2 ZM-2/ZM-3: H ve RTO_MAX duvar-saati tavanı yeni zaman mekanizması | **(b)**: §16.2 tablosunda TEK satır yetkisi (YAMA KAYDI; yalnız o satır); A.8 KOŞULSUZ (EK C); uzun pencereli hücreler DEMO-01 öncesi gece kartında | A.1, A.7, A.8; D listesi |
| 4 | yetki/dosya dışı bayatlayan atıflar | **(b)**: yetki dışı belgelere tek satırlık AMENDMENT-12 işaretçisi (YAMA KAYDI biçimi, silme yok); asıl düzeltme "INT-02 GEÇTİ" sonrası ayrı doküman PR'ı. **EK B8:** `dr_cs3_panel.rs:996-1001` referans mı assertion mı — assertion ise DUR-8 gibi 2×2, referans ise DUR-4 (b) işaretçisi yeter | A.9a–f; §6 B8: referans (`lineage`); dosya kapsam dışı → Ek A işaretçisi yok, bayat-atıf listesinde; `f3`/`f4` `control_12b` tavanına girer → panel koşulur, kırmızıysa DUR-15; D listesi |
| 5 | "`recover_fwd_entries = 0` (330/330)" yanlış: ham veri 15/330 (T1a 2, T1b 7, T6 6) | **(a)**: ayrı ERRATUM PR'ı; ham veri dosya yolları ERRATUM'da; donmuş belgelerde ERRATUM bandı (silme yok, üstü çizili); PHASE5 "Aşama A 5 sa 20 dk" etiketi aynı PR'da; tasarım etkilenmez | bu kartın dışında |
| 6 | T-hücre artefaktları + Faz-5 betikleri v1 bankasına bağlı; ortam farkı | **(a) düzeltmeli + EK A2**: Faz-5 betikleri ve v1 kanıt bankası BAYT-ÖZDEŞ; v1.1 koşusu repoda yeni dizinde, `reports/transport-v1.1/tcell/*` izni verildi ("ağaç dışı" reddedildi); betik kopyaları `reports/transport-v1.1/tcell/tools/` (yalnız etiket + ROOT farkı; orijinal ve kopya sha256 ön-kayıtta); ön-kayıt koşudan ÖNCE commit (hücreler, eşikler, kabul ölçütü, koşulacak commit SHA, koşu ağacının `Cargo.lock` SHA'sı, çekirdek 7.2.8-8-omarchy-bore — T-hücre + INT-02 aynı, rakip kollar aynı ortamda yeniden koşar); kanıt bankası iki etiketli (v1 dokunulmaz, v1.1 yeni) | §4; A.9f |
| 7 | INT-02 harness `k4g/05-int02`'de; iki şerit `sender.rs`'e dokunur | **(a)**: `k4g/05-int02` ayrı PR şimdi (orkestratör) → merge → `k4g/05b-retx-fix` rebase | §8 |
| 8 | `dr_cs1_acceptance.rs` f2b_di1/di1c/di2 teyitli birimin yeniden planlanmasına dayanıyor | **(a) dört koşulla**: (1) test başına PR paragrafı: neden yeşildi · düzeltmeyle neden kırmızı · fikstür tam olarak neyi değiştirir · assertion/eşik DEĞİŞMEZ; (2) çıktılı 2×2: eski kod + eski fikstür yeşil · yeni kod + eski fikstür KIRMIZI · yeni kod + yeni fikstür yeşil · eski kod + yeni fikstür de yeşil (kırmızıysa (a) değil, DUR); (3) mutasyon koşusu üç testin mutant öldürdüğünü gösterir; (4) DR-CS-1 belgesinde YAMA KAYDI ("v1.1'de kabul fikstürleri düzeltildi; neden: teyit edilmiş birimi yeniden planlayan v1 davranışına dayanıyordu") | §6; A.10 |
| 9 | sınıflar arası sıra aynı; HOL DATA/ARQ arkasında bekleyebilir | **(a)**: HOL-önce yalnız kopya kuyruğu içinde, sınıflar arası sıra değişmez; KOŞUL: D2 testi eşzamanlı yeni trafikle (kol 2, EK B5); dosya kolu INT-02'de ölçer; (b) ayrı §13 kartı yalnız dosya kolu açlık gösterirse | §6 D2; A.4; D listesi |
| 10 | biçim: "≤ 2 sayfa" ile "yama metni aynen" çakışır | **(a)**: gövde ≈ 2 sayfa + aynı dosyada Ek A (aynen spec'e) + Ek B | bu not |

**D — KATMAN 6 / Sprint 2 birikimi (bu kart değil; CEVAP PAKETİ 7-EK, 2026-10-18):**
- `RetxConfig::fixture` (cap 3, KALİBRE DEĞİL) adlandırma + kalibrasyon kartı (DR-1x ile).
- HEAD bench aracı v1 arşiv manifestlerini `canonical_kama=false` sayar → iki kimliği tanıyan araç (M sonrası).
- AMENDMENT-9 K = 1024 uygulama kartı — DEMO-01 ön koşulu (A1).
- DUR-4 bayat-atıf doküman PR'ı ("INT-02 GEÇTİ" sonrası; tam liste K4G-05b PR açıklamasında).
- ZM-2 4-decade uzun pencereli hücreler (0.25k/1k/100k/1M, D 30 s'ye ulaşır, N ≥ 30) — DEMO-01 öncesi ön-kayıtlı gece kartı.
- DUR-9 (b) sınıflar arası HOL yuvası — INT-02 dosya kolu açlık gösterirse ayrı §13 kartı.

**B.2 — §6 tanımları ve ayrıntı (gövde boyu için buraya taşındı; DUR-10 (a)).** Taban sonuçları beklentidir [çıkarım; koşulmadı].
- **Düzenek** [tasarım]: `crates/adaptive-transport/tests/k4g05b_*.rs`; yeni bağımlılık yok (`adaptive-transport/Cargo.toml` dev-dependencies yalnız `tokio`; `ClientSession` client-core tipidir, `client-core/src/lib.rs:68`). Kurulum = `Sender::new()` + `set_tail_recovery_config(TailRecoveryConfig::validated_v1())` (= `ClientSession::new`, `lib.rs:89-93`) + `runtime.rs:1290-1336` ürün ayarlayıcılarının aynası (`set_cs_probe_lane(false)` :1313, ürün `bootstrap_rtt: false` :261; ilk-uçuş 2400 B :260/:1332); ürün tikleri (`runtime.rs:65/:67/:69`); sabit olay sırası; tek yön 200 ms; gerçek `Receiver`. FEC Manual-off (faktör 3); üründe MinimalParity (`emergency.rs:194-199`) → faktör 2 = `dup_params` varsayılanı (`sender.rs:6063`, `DEFAULT_DUP_FACTOR` :1425) → ürün eşdeğerliği iddia edilmez.
- **D1:** prob bütçesi ≤ 10 + 20 + 30 = 60 s (`tail_recovery.rs:76/:78/:80`); owned(U) payı H + 1 tik; kapalı × sessizlik kontrolü tabanda RecoverFwd canlılığıyla yeşil beklenir (`sender.rs:5816-5831`).
- **D2 simgeleri:** T_r kesinti bitişi · T_a T_r sonrası ilk ilerlemeli ACK · T_H sıfırlamadan sonra u₀ kopya kaydının vadesi (≤ son kopya + taban; A.7 Sıfırlama (ii)) · u₀ T_r'deki en eski onaysız birim · S u₀ boyu · X T_H ile u₀ emisyonu arasında kopya sınıfı dışında (DATA/ARQ/prob) gönderilen bayt · min_rate 31 B/s (`budget.rs:29`; hız tabanı `sender.rs:4212`) · R₀ kesinti başındaki uçuş kaydı sayısı · Δ kesinti süresi · D birimin RECOVER_FWD kopya faktörü (≤ D_max = 3) · m = ⌈log2(RTO_MAX/b)⌉ · U kesintideki onaysız birimler.
- **D2 iddiaları:** kol 1 (60 s iki yönlü kesinti, saf drenaj; u₀ T_r'ye dek düşer) ve kol 2 (kol 1 + T_r sonrası aynı boy DATA ≤ min_rate/2) — T_a sonrası ilk Duplicate u₀'nın; u₀ emisyonu ≤ T_H + ⌈(S + X)/min_rate⌉ + 1 ms; owned(u₀) her olayda. Kol 3 (600 s kesinti, A1(3)) — kayıt ≤ R₀ + Σ_U (1 + (D − 1) + m + ⌊Δ/RTO_MAX⌋) + `max_tail_probes`, birim başına kopya kaydı ≤ D_max; v1 logaritmik → tabanda yeşil beklenir (bekçi); kol 3'ün taban kırmızısı aralık tavanı (≤ RTO_MAX; v1 tavansız `sender.rs:5966`) ve T_a sıfırlaması iddialarından beklenir.
- **D2 fikstür koşulları K1–K4** (`dr_cs3_panel` F1–F9 üretici kimlikleriyle karışmasın diye K; her koşuda iddia edilir; düşerse fikstür düzeltilir, beyan): K1 u₀ T_r'de en eski onaysız birim · K2 u₀'nın son kaydı Retransmit kökenli (canlılık `sender.rs:5822` dışlar) · K3 T_r'de kuyrukta u₀'nın önünde vadesi gelmiş ≥ 1 fire-once kayıt (:5843-5847) · K4 T_r sonrası token sınırlı (pace_rate × tik < S; FIFO + `break` :5943-5948 bağlar).
- **Soğuk başlangıç:** RTT 400 ms, deterministik %30 kayıp, 600 s; kollar A soğuk başlangıç · B ilk ACK T_hold > 16.78 s tutulur (bayat tohum, EK B6) · C ilk çerçevenin en büyüğü kuruluş probu. B tohumu = RTT + T_hold − 16.777 s (doygun `ack_delay` 65535 × 256 µs; C'de v1 daha eski orijinali alır → + gönderim farkı); T_hold fikstürde tohum ≤ RTO_MAX olacak seçilir → ⌈(7/8)⁶⁴ · 29.6 s⌉ + 256 µs ≈ 6.008 ms ≤ 6.01 ms [aritmetik]. Her kabul ≥ 1 kuantum adımı (`rtt.rs:145`, `QUANTUM_CAP` 64 :131); iki derleme bit-özdeş. Taban kırmızısı: v1 en büyük olmayan Data'yı da örnekler (`sender.rs:3034`) → kabul sayısı kâhin üst sınırını aşar.
- **B2 ekleme yerleri** [kod]: `sender.rs` :3395, :5693, :5827, :5843, :6008, :6024 (:5904/:5967 net 0); toplam ≤ 3 × 4096 = 12 288 (`runtime.rs:248/:1785`) [çıkarım].
- **B1 karşı örneği** [aritmetik]: pencere = hız × 40 ms; boy motorda sınırsız (`sender.rs:82`); üründe birim = `wire_len(başlık + chunk)` (`runtime.rs:1835`; chunk ≤ 1013 B :234; FEC-shard 1061 B :227) → ≤ 1061 B [çıkarım: mühür ek yükü doğrulanmadı]. Hız 1064 B/s, tx başı 37 B → tavan 42 B < 1050 B → `break` (:5948), uyanma yok (`pacer.rs:116`), AppData token'ı kırpar (`pacer.rs:72`). PreConverge'de ayrıca DR-EST-1 kuruluş kolu (varsayılan açık `sender.rs:1886`; yalnız testler kapatır) en erken vadeli §3.3-B kopyasını token-muaf ≤ 8 s'de bir taşır (:5888, :5914) [kod].
- **B1 kolları:** karışık boy; kontrol L = DATA boyu (tabanda yeşil beklenir); 2·MTU + 1; aynı tur; DUR-12 (a) +2 kol: ARQ planı başı, bekleyen prob hedefi; PreConverge kontrolü: ilk-uçuş 2400 B, birim ≤ 2400 B (v1; tabanda yeşil beklenir); çıkış turu = v1.
- **kama-integration:** 11 test dosyası `adaptive_transport::` kullanır; komut `main.yml:47` `cargo test --workspace --all-features`.
