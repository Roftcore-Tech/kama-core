# K4G-05b — Yeniden-gönderim sahipliği (transport v1.1) — tasarım notu

## 0. Künye
Kart **K4G-05b** · taban `d46ec628` (satırlar buradan; `main` `3439fccd`: #133/#134 kart dosyalarına dokunmaz, #133 rakip kolun rustls'ünü değiştirir) · **TASLAK — sahip onayı bekler;** kod "K4G-05b ONAYLANDI", kapanış ayrı "INT-02 GEÇTİ" ile. Uygulanmadı, ölçülmedi. Biçim (DUR-10): gövde §0–§9 ≈ 2 sayfa (≈ 1 390 kelime); **Ek A** yama metinleri aynen (117 satır, sha256 `f33e633239804279044c0a6e2b6bfcd82ae8b07eabd242f365e9215e212bf412`), **Ek B** DUR seçenekleri. Etiket: [ölçüm] veri · [kod] okuma · [tasarım] ölçülmemiş.

## 1. Değişmez (sahip metni)
> "Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir."

Okuma: sessizlik → canlılık kopyası (Duplicate, kayıp değil; INV-001, `tv_loss.rs:51-56`); later-ACK → dedektörle ilan + yeniden plan. Reliable = her motor birimi; RecoverFwd girişine ve ACK sessizliğine dayanmaz; izleme teyide ya da oturum sonuna (Sender düşer; INV-NO-IDLE-TEARDOWN) dek.

## 2. Mekanizma (ölçüm + kod okuması; satırlar `sender.rs`)
LostRec = kayıp ilanlı kayıt · DupRec = kopya kaydı · fire-once = tek atımlık · HOL = sıra başı.
- **D1 — öksüz prob kopyası** [ölçüm diag-72cb772c gating2 s3/s9; m12 s3 u71]. **Düzeltme kaydı:** "cap 3 → HandToForward" bildirimim YANLIŞTI. [kod] En yeni kayıt prob (`Retransmit`, :5796; orijinal tombstone'da); later-ACK yok → ilan yok (`loss.rs:692`); 3 probluk bütçe yalnız uçuş küçülünce sıfırlanır (:3166); canlılık kolu yalnız Data kopyalar (:5822). Gizli varyantlar [kod]: HandToForward fire-once (:5696), ConnectionFail düşürür (:5699).
- **D2 — HOL açlığı** [ölçüm diag-72cb772c kesinti s1/s3: dup_queue 94→261 / 104→335, HOL 125 s bekledi]. [kod] Drenaj LostRec başına 2 fire-once kopyayı Vec sonuna dedupe'suz ekler (:5836-5847); FIFO + ilk token reddinde `break` (:5943-5948); parity'den önceki son sınıf (:4251; parity :4255); aralık tavansız (:5966).
- **srtt 60.5 s (kesinti s1) — Karn değil** [ölçüm + kod]: kümülatif SACK kesinti öncesi orijinali, en büyüğü kopya olan çerçevede yeniden raporladı; örnekleyici en yüksek yeni-onaylı Data'yı alır (:3034/:3057/:3117), `ack_delay` en büyüğün (`receiver.rs:405`), n = 64 (`rtt.rs:145`) → 60492.107 ms; guard reddinde RAW tutma süresini içerir (`rtt.rs:244-246`).
- **ESI deliği** [kod]: parite `unit.id = esi` (:5255) `acked_units`'e girer (:3027/:3043) → gerçek birimin zinciri iptal (:3154); prob pariteyi hedefleyebilir (:3228).

## 3. Tasarım [tasarım] (`sender.rs` fonksiyon :taban — değişiklik — madde)
1. `on_ack_received` :3027/:3043, Pto prob hedefi :3228 — FecParity teyit ve hedef olmaz — ESI.
2. yeni `confirmed_units`; `queue_losses` :3393/:3437; `emit_tail_probe` :5762 — teyitli birimin kalan kayıtları yalnız muhasebede, yeniden plan yok — D2.
3. `plan_arq_retransmits` :5685-5699, `plan_forward_duplication` :5836 — her politikada tek kalıcı geometrik sahip, düşürme yok; `RetxConfig::fixture` (cap 3, KALİBRE DEĞİL; :1860 = sahip :1916) ürün yolunda kalır, cap yalnız yolu değiştirir → cap aşılıp kopya kuyruğu boş kalamaz; :5963-5971 (sahip :6095-6103) yeniden-planlaması PreConverge dışına genelleşir; kalibrasyon ayrı iş — D1.
4. yeni `adopt_unowned` (`schedule_transmit` :4244 ardı, PreConverge/Idle dışı her olay) — (a) canlılık `origin != FecParity`; (b) prob bütçesi bitince aynı turda devir; (c) son ilerleme/iletimden H sonra sahiplen, vade uyanma kümesinde (kaynak 7) — D1, M1.
5. `drain_due_duplicates` :5932 — PreConverge dışı en küçük birim kimliği önce, birim/tur ≤ 1, token reddinde `break`, aralık ← min(aralık × `PC_COPY_MULTIPLIER` (2, :1503), RTO_MAX) — D2.
6. **Sıfırlama yeri:** mandal H'de (`adopt_unowned`) kurulur; `on_ack_received` :3153-3155 `retain` ardında ilk ilerlemeli ACK geometrik aralıkları tabana döndürür; prob sıfırlaması v1 (:3166) — D2.
7. `on_ack_received` RTT :3010-3149 — §6 KABUL KURALI (Ek A.3); `rtt.rs` aynı — srtt.
8. `on_event_core` :2287 — PreConverge çıkışında bekleyen kopyalar RTO_MAX'a kırpılır — RTO.
9. `track_sent` :6079 — PreConverge dışı yalnız vadesi gelmiş kopya "bekleyen iş" (kopya BtlBw örnek sınıfını etkilemez; ölçülmedi) — INV-RATE-1.
10. `emit_tail_probe` :5796 `saturating_add`; `arq.rs:27-34` yalnız doc.

Sınırlar: RTO_MAX = `validated_v1().max_backoff_pto_ns` (`tail_recovery.rs:78`); H = max(3·srtt, 2·EAI)·1000/300 ≥ 13.98 s (EAI = beklenen ACK aralığı; :6139-6142); dup_queue ≤ 3/birim ≤ 12 288 (`emergency.rs:121`, `crates/client-core/src/runtime.rs:248`; bugün sınırsız); ilk yeniden gönderim ≤ 60 s (prob açık) / H, sonra ≤ RTO_MAX + token bekleme.

## 4. Dokunulan dosyalar (tam liste)
- `crates/adaptive-transport/src/sender.rs` (mantık + testler), `src/arq.rs` (doc); DUR-8 (a) ile `tests/dr_cs1_acceptance.rs`.
- Kimlik/freeze (kasıtlı, beyanlı): `crates/client-core/src/runtime.rs:217/:220`, `crates/client-core/tests/transport_v1_freeze.rs:20-21`; `api_surface_v0.rs` kimlik literali içermez, pub öğe değişmez → pin güncellemesi yok (PR'da beyan).
- Yalnız kimlik adı: `crates/bench/src/{manifest.rs, main.rs, event.rs, kama_product.rs}`, `.github/workflows/nightly.yml:201/:214`; beyan: `main.rs:665` kimlik-çifti koşulu (v1 INV-009 denetimi korunur), arşiv v1 manifestleri HEAD aracında `canonical_kama=false` (`manifest.rs:128`; v1 raporu v1 araç commit'iyle); Faz-5 betikleri DUR-6.
- `specs/adaptive-transport-engine-spec.md`, `specs/transport-layer-tdd.md` (Ek A), bu not. **Başka dosya yok;** tel, codec, `tv_*` değişmez. S1, S3, noise-wire, ccr, kama-node dokunulmaz; FEC parite planlama/emisyonu değişmez (ESI yalnız ACK/prob hedefi); DR-1x kalibrasyon kampanyası açılmaz.

## 5. Spec yama metinleri → Ek A
Ek A = tasarımın kapı-denetimli metni (`patches.py` sha256 `d52bc136…`; engine +36, TDD +35 satır); silme kapısı (`scripts/ci/word_coverage.py` `lost_words` + `displaced_lines`, taban `d46ec628`, bellekte) iki dosyada 0/0, çizgisiz kontrol kırmızı. Taslağın sıkıştırılmış metni normatif cümle düşürdüğü için geri çekildi. A.8 yalnız DUR-3 (a)/(b) ile.

## 5b. Ölçüm (CEVAP 6)
Kaynak `~/kama-k4g-work/int02/meas-1f6023ff/m12.md`; beklenti kaydı `docs/reports/INT-02-M1M2-BEKLENTI.md` (k4g/05-int02 @9578758e) sha256 `cb2e4fc1…` koşudan önce kilitli; motor düzeltme öncesi (`1f6023ff`).
- **D1 tekrarlandı:** gating2 s3 alındı 71/100; u71 retx 3/3, prob 3/3, son gönderim 597.5 s önce.
- **M1:** s3'te bayat SACK çerçevelerinin (17 + 38) hiçbiri ilerleme üretmedi; diziler 94.4 / 96.4 s; RecoverFwd alice→bob 2 kez (14.2 s sessizlik sonrası), bob→alice 0 → kayba bağlı.
- **M2:** ≈28 B/s yeniden üretilmedi: alice→bob s1/s3 pacer medyanı 1064 / 917 B/s, tabanda %30 / %29, BtlBw kalıcı dönüş 12.3 / 10.3 s; pacer 0.5×-eşiğe yalnız s1 alice→bob'da döndü (12.3 s; s3 alice→bob, s1 bob→alice: öncesi medyanı 5 örnek, Startup ağırlıklı) — INT-02'de izlenir.
- **Kesinti kolu:** 2/2, aynı oturum, kesinti bitişinden 21.3 / 21.8 s sonra teslim; eski 1/5 ve 0/2 kısmen 120 s pencere artefaktı. D1 tekrarlanabilir kusur; D2 gerçek, etkisi değişken.
- **Öneri:** (a) hız tabanı işi yok, INT-02'de izlenir; (b) değişmez RecoverFwd'den bağımsız (4c).

## 6. Kanıt
- **Yeni** (sahte saat, gerçek `Receiver`; önce tabanda kırmızı, değilse DUR); D1/D2'de her olaydan sonra (bayat ACK dahil) owned(U) yüklemi doğrudan sınanır. `k4g05b_d1_no_orphan_600s` (prob açık/kapalı × bayat SACK/sessizlik), tabanda kırmızı: prob açık — bütçe bitince (≤ 10 + 20 + 30 = 60 s; `tail_recovery.rs:76/:78/:80`) 600 s'ye dek 0 yeniden gönderim; kapalı × bayat SACK — 0; kapalı × sessizlik tabanda RecoverFwd canlılığıyla yeşil (:5816-5831) = kontrol. `k4g05b_d2_post_outage_hol_first_no_stale_rtt`: HOL sırası, srtt ≤ 1 s, aralık ≤ RTO_MAX, sıfırlama. Ayrı `k4g05b_esi_parity_ack_does_not_confirm_source_unit`. + 16 dar test (toplam 19).
- **Mevcut testler + `tv_*` değişmeden yeşil KALMALI** (kırmızı = DUR; bilinen DUR-8). **Mutasyon** (ağaç dışı, 38): cap dalı, kopya planlaması, HOL sırası, sıfırlama, RTT, RTO tavanı.
- **T-hücre:** değişen yollar Faz-5 v1'de T5 dışı 10 hücrede ateşledi (`stageB/*/kama-product/rep*/c.err` KP-ACTIVATION tail_probe/retx/dup > 0; T5 30/30 sıfır) → alt küme ≈ 5 sa, tam setten yalnız ≈ 20 dk kısa (T5 shard 20 dk 3 sn). Tam set (11 × 3 kol × 30 = 990): ölçülü 5 sa 20 dk 25 sn (`~/kama-phase5-results/campaign.out` 15:02:57Z → 20:23:22Z; 19.4 s/koşu) + Aşama A 8 dk 16 sn (`STAGE-A-VALIDATION.md:8`) + T3b ≈ 11 dk (`EXPECTATION.md:276`) ≈ 5 sa 40 dk; tavan 10 sa (`EXPECTATION.md:277`) < 1 gün → **bu kartta, KATMAN 6 kalemi yok.** T-hücrelerinde zamanlı kesinti yok (kanıt INT-02). Rakip kollar da koşar; sürüm/çekirdek farkı ön-kayda (Ek B DUR-6).
- **INT-02:** GATING (2) 10 × 600 s **10/10**; 60 s kesinti **5/5** aynı oturumda %100; dosya kolu (D3) harness düzeltmesiyle ölçülür, raporlanır; M2 hız tabanı izlenir.

## 7. Süre
Kod + test 2.5–3 gün · mutasyon + `ci.yml` kapıları 1 gün · T-hücre ≈ 5 sa 40 dk gece · INT-02 ≤ ≈ 2.7 sa (10 × ≈ 625 s ölçülü + 5 × ≈ 685 s türetim; biten tohum erken çıkar) + dosya kolu → ≈ 2–3.5 sa (`summary.txt`: run_g2 657 s, run_kes 211 s, ikisi 2 tohum; dosya kolu tahmin) · rapor 0.5 gün → ≈ 4.5–5 iş günü.

## 8. Sıra
Not → "K4G-05b ONAYLANDI" → tek PR/şerit: testler tabanda kırmızı → düzeltme → yeşil → mutasyon → kapılar → T-hücre → INT-02 → rapor → ayrı "INT-02 GEÇTİ". PR açıklaması: EVOLUTION §22 altı beyanı tam hâliyle (`EVOLUTION.md:85`) + bayatlayan atıfların tam listesi.

## 9. DUR (numaralar tasarım DUR-1..10; seçenekler Ek B)
Öneri: 1 srtt-yok tohum **a** · 2 kayıt büyümesi **a** · 3 §16.2 ZM-2/ZM-3 **b** · 4 bayat atıf **b** · 5 "330/330" (15/330) **a** · 6 ön-kayıt + Faz-5 betikleri (CEVAP 6'dan sapma) + çekirdek **a** · 7 şerit sırası **a** · 8 `dr_cs1` fikstür **a** · 9 sınıf-arası HOL **a** · 10 biçim **a**.

## Ek A — yama metinleri (aynen; kaynak tasarım `patches.py` sha256 `d52bc136dfc5bc918883c34c6313bdb44f98cea14cf6be6c32cbde91c133ce14`; `<ONAY-TARİHİ>` = onay günü; sha256 kapsamı bu satırdan `## Ek B` satırına dek)
**A.1 engine — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — RTT örnek kabulü + kopya kuyruğu sırası; sahip kararı CEVAP PAKETİ 5]:** §6 Girdi satırına **v1.1 KABUL KURALI** eklendi (çerçevenin gönderilmiş en büyük onaylı seq'i · bu çerçeveyle ilk kez onay · birimin tek iletimi olan orijinal DATA · doygun olmayan ack_delay · tutma süresiyle şişmemiş örnek; geri düşüş yok; srtt yokken v1 seçimi aynen); §6 min-rtt guard'ının RAW kolu ve "örnek ATLANMAZ" hükmü bu kurala bağlandı; §5 TUR-2a/TUR-2b kol-1 RTT'si bu kurala bağlandı (ürün yolunda yalnız ters-yol ACK yeniden sıralamasında erişilir; BW-örneği ve delivered-kredi DEĞİŞMEDİ); §5 ZEYL'deki "tam örnek verir (belirsizlik yok)" üstü çizildi; §13'e kopya kuyruğu içi sıra eklendi (en eski onaysız birim önce, RTO_MAX tavanı; sınıflar arası sıra DEĞİŞMEDİ). Otoriter v1.1 kaydı: transport-layer-tdd §12 [AMENDMENT-12]. Kuantum döngüsü, tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Üstü çizili metin silinmedi.
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
       [v1.1 · K4G-05b, NORMATİF — sahip kararı CEVAP PAKETİ 5] KABUL KURALI (srtt VARKEN): ACK çerçevesi
       başına en çok BİR örnek; yalnız şu beş koşul birlikte sağlanırsa alınır — (a) kayıt, çerçevenin
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
       kararına bağlı: PRE_CONVERGE kopyaları (§3.3-B, DR-EST-1) aksi hâlde ilk örneği kilitleyebilir].
```
**A.4 engine §13 — :959 ardına ekle (:960 kapanış çitinden önce):**
```text
[v1.1 · K4G-05b, NORMATİF — sahip kararı CEVAP PAKETİ 5] KOPYA KUYRUĞU İÇİ SIRA (PRE_CONVERGE DIŞINDA):
  vadesi gelmiş kopya kayıtlarından EN ESKİ ONAYSIZ birimin kaydı önce gönderilir (ölçüt: en küçük birim
  kimliği — ürün yolunda kimlik oturum içi gönderim sırasıdır; eşitlikte erken vade, sonra kuyruk sırası);
  bir birim bir planlama turunda en çok bir kez; token yetmezse o tur DURUR — arkadaki kayıt sıranın
  başını GEÇEMEZ (tek istisna: birim o anki BURST_CAP'ten büyükse token ona hiç yetişmez, tur sonraki
  birime geçer); teyit edilmiş birimin kaydı gönderilmez. Geometrik aralık ×2, üst sınır RTO_MAX
  (transport-layer-tdd §12 [AMENDMENT-12]). PRE_CONVERGE'de §3.3-B sırası ve takvimi DEĞİŞMEZ;
  PRE_CONVERGE çıkışında bekleyen kopyaların aralığı ve vadesi RTO_MAX'a kırpılır. Sınıflar arası sıra
  DEĞİŞMEDİ: v1 tahkimi kod sırasıdır (DATA → ARQ → kuyruk-probu → kuruluş probu → kopya kuyruğu → FEC
  parity; RECOVER_ARQ'da her K_ARQ_SHARE'inci tur ARQ ile başlar) — yukarıdaki priority satırıyla
  ayrışması bu kaydın kapsamı dışındadır.
```
**A.5 TDD — :3 önüne ekle (+ boş satır):**
```text
> **YAMA KAYDI — <ONAY-TARİHİ> [v1.1 · K4G-05b — yeniden-gönderim sahipliği; sahip kararları CEVAP PAKETİ 4/5/6]:** §12'ye [AMENDMENT-12]: INV-NO-ORPHAN (sahip metni) · ARQ_RETX_CAP aşımı ve kuyruk-probu bütçesinin tükenmesi yolu değiştirir, birimi düşürmez · **RTO_MAX = 30 s** normatif sabit (koddan belgeye: `TailRecoveryConfig::validated_v1().max_backoff_pto_ns`; yeni sayı yok) · kesinti sonrası ilk taze ACK'te kopya geri çekilmesi sıfırlanır · FEC onarım sembolünün ACK'i kaynak birimi teyit etmez; kimlik `kama-v1-full` → `kama-v1.1-full`, `transport-v1` → `transport-v1.1` (AMENDMENT-11'in beyan ettiği `transport-v1.x` de bununla sabitlere işlenir). §5.3 "RTO↑" satırına tavan işareti. Engine-spec §5/§6 (RTT kabul kuralı) ve §13 (kopya kuyruğu sırası) v1.1 yamaları aynı kartta. Tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Bu belgede metin silinmedi (yalnız ekleme).
```
**A.6 TDD §5.3 — :623 satır sonuna ekle:**
```text
   [v1.1: RTO↑ ≤ RTO_MAX = 30 s — §12 AMENDMENT-12]
```
**A.7 TDD §12 — :846 önüne ekle (+ boş satır):**
```text
> **[AMENDMENT-12 — <ONAY-TARİHİ>, v1.1 · K4G-05b: YENİDEN-GÖNDERİM SAHİPLİĞİ + RTO_MAX · NORMATİF · DAVRANIŞ DEĞİŞTİRİR · TEL DEĞİŞMEZ]**
>
> **INV-NO-ORPHAN (sahip metni):** Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim
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
> (dedektör, ledger ve kader-kanıtı v1 gibi), yeniden planlanmaz. Birim başına ≤ 1 geometrik kopya kaydı; bu kaydın
> getirdiği iletimler ortak token kovasındandır (INV-RATE-1); PRE_CONVERGE dışında vadesi gelmemiş kopya kaydı DATA
> örneğini app-limited dışı saydırmaz.
> **RTO_MAX = 30 s (NORMATİF sabit — koddan belgeye, yeni sayı DEĞİL) [CAL — DR-ARQ-PRODUCT-TAIL-1, tuning pending]:**
> kaynak `TailRecoveryConfig::validated_v1().max_backoff_pto_ns` (PRODUCT-PROFILE "backoff tavanı 30 s", SHIP; pin
> `crates/client-core/tests/transport_v1_freeze.rs`). Üst sınırı olduğu aralıklar: (i) kuyruk-probu PTO geri
> çekilmesi (v1'de de); (ii) PRE_CONVERGE dışında kopya kuyruğunun geometrik aralığı ve tabanı (v1'de tavansızdı;
> PRE_CONVERGE çıkışında bekleyen kopyalar RTO_MAX'a kırpılır). Taban srtt-türevidir; taban-PTO tavanı `max_pto_ns` =
> 10 s ayrıdır; H kanal-türevidir ve RTO_MAX'la sınırlanmaz. ZM-2 4-decade doğrulaması BEKLİYOR (beyanlı: v1.1
> T-hücreleri D = 20 s ile bu tavana ulaşmaz). Değeri değiştirmek ayrı sahip kararıdır.
> **Sıfırlama (NORMATİF):** (i) kuyruk-probu: uçuştaki kaydı azaltan ilk ACK probe_index'i sıfırlar (v1 kodu, sender
> `on_ack_received`; DEĞİŞMEDİ); (ii) kopya kuyruğu: ilerlemesiz süre H'yi aştıktan sonra en az bir kaydı ilk kez
> onaylayan İLK ACK bütün geometrik kopya aralıklarını tabana döndürür (sonraki kopya en geç son kopya + taban). Bayat
> ACK sıfırlamaz. **Beyanlı sınırlar:** süresiz kesintide uçuş kayıtları birim başına RTO_MAX'ta ≤ 1 artar (engine §14
> gerilimi); [AMENDMENT-9] K = 1024 tavanı uygulandığında bu yolların emisyonları ona TABİDİR (izleme sürer; tavan
> bugün SPEC-ONLY). Bayatlayan atıflar ve EVOLUTION §22 altı beyanı: K4G-05b PR açıklaması.
```
**A.8 (yalnız DUR-3 (a)/(b) yetkisiyle) engine §16.2 saat-sınıfı tablosuna satır ekle:**
```text
| [v1.1] kopya kuyruğu aralık tavanı + sahipsizlik süresi | sender dup_queue / adopt_unowned | RTO_MAX = 30 s wall-clock TAVAN (taban srtt-türevi); H = max(3·srtt, 2·EAI)·1000/escalate_permille kanal-türevi | ölü kanalda tam sessizlik / sahipsiz birim (v1) | 4-decade kanıtı BEKLİYOR (v1.1 T-hücreleri D = 20 s tavana ulaşmaz) — transport-layer-tdd AMENDMENT-12 |
```

## Ek B — DUR seçenekleri (numaralar tasarım DUR-1..10)
| # | bulgu | seçenekler | öneri |
|---|---|---|---|
| 1 | srtt yokken katı kural ilk örneği kilitleyebilir: §3.3-D kilidi (engine :387-388; INV-010); her PreConverge DATA'ya §3.3-B kopyası, DR-EST-1 0.5 s'de kopya (`sender.rs:1434`); INT-02 ping RTT 384–457 ms | (a) srtt yokken v1 seçimi aynen (Ek A.3 son cümle; artık: > 16.78 s tutulan ilk ACK bayat tohum verebilir) · (b) her yerde katı kural (PreConverge takılma riski) · (c) katı kural, §3.3-B/DR-EST-1 kopyaları sayılmaz (donmuş soğuk-başlangıç yolunda yeni kural) | (a) |
| 2 | süresiz kesintide uçuş kayıtları ve `loss.rs:210` haritası birim başına RTO_MAX'ta ≤ 1 büyür (engine §14 :969-973 gerilimi); normatif tavan AMENDMENT-9 K = 1024 SPEC-ONLY, kart dışı | (a) v1.1'de beyan (60 s INT-02 kesintisi ≤ ≈ 1000 kayıt); DEMO-01/saha öncesi AMENDMENT-9 uygulama kartı · (b) yalnız bağlantı-düzeyi HOL şeridine tavan · (c) aralıklar tavansız | (a) |
| 3 | §16.2 ZM-2/ZM-3 (engine :1093-1099, "İHLAL EDİLEMEZ"): H (kanal-türevi) ve kopya zincirinde RTO_MAX duvar-saati tavanı yeni zaman mekanizması; ZM-3 tablo satırı ister, yetki §5/§6/§13; ZM-2 4-decade kanıtı ister, T-hücreleri D = 20 s (EXPECTATION.md:69-80) 30 s tavana ulaşmaz, 0.25k/1k hücresi yok, T3b n = 5 | (a) tek §16.2 satırı için yetki (Ek A.8), kanıt açığı v1.1'de beyanlı · (b) (a) + DEMO-01 öncesi gece kartında ön-kayıtlı uzun pencereli hücreler (0.25k/1k/100k/1M, D 30 s'ye ulaşacak, N ≥ 30) · (c) yalnız TDD saat-sınıfı cümlesi | (b) |
| 4 | yetki/dosya dışı bayatlayan atıflar: engine §2.1 :194-195, §5.1 :517-518 (H'de EAI yok; kod :6138-6142), §9.1 :723, §12 :930, §16.2 :1144, :1155-1159; DR-ARQ-PRODUCT-TAIL-1-TDD :51/:52/:54-55/:63; docs/transport-v1 ARCHITECTURE.md:118, PRODUCT-PROFILE.md, INVARIANTS.md:17/:21, README.md:10; EK4-A2 uyanma envanteri (kaynak 7); `crates/kama-integration/tests/common/dr_cs3_panel.rs:996-1001` (F3/F4 hâlâ fire-once). Tam liste PR açıklamasında | (a) bu kartta işaret satırları (yetki + dosya genişler; metin önce notta gösterilir) · (b) AMENDMENT-12 işaretçisi + "INT-02 GEÇTİ" sonrası doküman PR'ı · (c) olduğu gibi | (b) |
| 5 | "`recover_fwd_entries = 0` (330/330)" yanlış [ölçüm]: ham `~/kama-phase5-results/stageB/*/kama-product/rep*/c.err` `reach=[… recover_fwd=N]` 15/330'da sıfırdan büyük (T1a 2, T1b 7, T6 6). Geçtiği yerler: engine :852-853 + 9 belge (KNOWN-LIMITATIONS.md:56, ARCHITECTURE.md:94, PRODUCT-PROFILE.md:51, HOSTILE-ENVELOPE.md:43, ADR-minimal-parity-exception.md:64-65, FINAL-MECHANISM-AUDIT.md:25, EVOLUTION.md:60, DENEY-DEFTERI.md:120/:194/:207, TRANSPORT-V1-CLOSURE.md:65) | (a) tüm yerler için ayrı ERRATUM PR'ı (liste PR'da) · (b) engine :852-853 bu kartta çizilir (yetki genişler), belgeler (a) ile | (a); v1.1 ön-kaydı RecoverFwd girişini hücre başına gözlem olarak kaydeder |
| 6 | T-hücre artefaktları liste dışı: yeni ön-kayıt `reports/transport-v1.1/tcell/*` (donmuş EXPECTATION.md ve BENCHMARK-METHODOLOGY.md `kama-v1-full` yazar). Faz-5 betikleri v1 bankasına bağlı (`phase5_cell_report.py:4` ROOT, :6 ve `phase5_analyze.py:14` etiket) → yeniden adlandırma v1 kanıtını yanlış etiketler; bu, CEVAP 6'nın "Faz-5 betikleri kimlik adı kadarıyla kapsamda" kararından sapmadır. Ortam: quinn-proto 0.11.16 (`Cargo.lock@e3c1e8bb`) → 0.11.18; rustls 0.23.41 → 0.23.45 (`d46ec628`) / 0.23.46 (main, #133); çekirdek Faz-5'te kayıtlı değil (7.0.12 = d1-exit FV ortamı, 2026-07), bugün 7.2.8-8-omarchy-bore | (a) tcell/* izni; betiklere ağaç içinde dokunma, yalnız etiket + ROOT farkı olan ağaç dışı kopyalar (sha256 ön-kayıtta); koşulan ağacın `Cargo.lock`'u ve çekirdek ön-kayda; mevcut çekirdek 7.2.8-8-omarchy-bore T-hücre + INT-02 için sabit · (b) betikler ağaç içinde ROOT parametresiyle yeniden adlandırılır · (c) T-hücreleri ayrı şerit/PR | (a) |
| 7 | INT-02 harness ve teşhisler `k4g/05-int02`'de (`1f6023ff`); iki şerit `sender.rs`'e dokunur | (a) önce gözlem-only K4G-05 şeridi, sonra K4G-05b rebase; INT-02 birleşik yapıda, manifestte iki SHA · (b) birleştirilmemiş kompozisyon dalından koşu | (a) |
| 8 | [kod] `crates/adaptive-transport/tests/dr_cs1_acceptance.rs` f2b_di1 (:376-421), f2b_di1c (:471-547), f2b_di2 (:553-586) seq 1 / 1,2'yi (:406/:490/:570) u2/u3 DATA sanıyor; DR-EST-1'den beri bunlar u1 kopyası → ACK u1'i teyit eder. Taban u1'in teslim edilmiş orijinalini yeniden planladığı için yeşil; madde 2 bunu kaldırır → kırmızı (:369 panik, :583 `induced == 1200`) | (a) yalnız bu üç testte beyanlı fikstür düzeltmesi: u2/u3 DATA seq'leri toplu çıktıdan okunur (f2b_di1a :427-468 gibi); PR'da önce kırmızı gösterilir · (b) madde 2'nin `queue_losses` atlaması kaldırılır (teslim edilmiş birimin boşa yeniden gönderimi döner) · (c) yalnız DR-EST-1 ile teyitli PreConverge orijinallerine istisna | (a) |
| 9 | sınıflar arası sıra aynı (DATA → ARQ → prob → kuruluş probu → kopya → parity, :4229-4255): sahibi yalnız kopya olan HOL DATA/ARQ birikimi arkasında bekleyebilir; burst-cap alt sınırı tx_queue başı (:2310, :2322-2325) → bloklayan yuva kilitlenebilir; gözlenen D2 kuyruk içiydi | (a) D2 = kopya kuyruğu içi (sahip metni); sınıflar arası sıra §13 yamasında DEĞİŞMEDİ diye beyanlı; INT-02 (özellikle dosya kolu) HOL bekleyişini kaydeder · (b) §13 yetkisiyle bloklamayan sınıflar arası HOL yuvası + D2b testi | (a); dosya kolu açlık gösterirse (b) ayrı §13 kartı |
| 10 | biçim: "≤ 2 sayfa" ile "yama metni notta aynen" çakışır (71 eklenen satır) | (a) gövde ≤ 2 sayfa + metinler aynı dosyada Ek A (satır sayısı + sha256 gövdede) · (b) ayrı ek dosya · (c) metni daha da sıkıştır (beyan kaybı; taslakta denendi, geri çekildi) | (a) — bu not |
