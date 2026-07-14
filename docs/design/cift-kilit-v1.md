# cift-kilit-v1 — p5-Açlık Çift-Kilidi: Tohum-Tavanı × RecoverArq-Yapışması [TASLAK r2 — ONAY BEKLİYOR]

> **Statü:** D0-TASARIM-NOTU (İŞ-2). Abkarada'ya döner; ONAYDAN SONRA kod. **Kapsam-sınırı:**
> sender-taraf (sender.rs / bw.rs / rate.rs; +bench-TRAJ alanları); **receiver.rs'e SIFIR dokunuş.**
> **KOD-ZEMİNİ PİNLİ:** tüm sender.rs satır-referansları `tur6-plateau@4de2b25` (PR #36) —
> **TUR-6-merge bu paketin ÖN-KOŞULU** (β1'in plato/round etkileşimi o zemine tasarlandı; smoke
> temiz-1k≥45 kapısı da ancak o zeminde tanımlı). Zemin: 3-lens teşhis (wf_a99a5d61, oybirliği)
> + 24-koşu veri + doğrulama-turu (wf_9acd4d53: 21 bulgu — r1'in fix-kolonlarını kırdı, r2'de işli).
>
> **r1→r2 (doğrulama-bulgularıyla):** β2-cap-tabanı YAPISAL-ÖLÜ (token 534'e hiç birikmez —
> 1ms-tick'te 0.08-0.16B/olay) → tek işleyen mekanizma token-önceliği-payı; β1×TUR-6 pinlendi
> (plato-BASE-üstünde + recovery-dönüşü≠re-entry + base∈{Startup,Steady}); α1'in iki kolu da
> ölü/negatif (admission zaten bastırmıyor; dışlama decay-yolunu öldürür + KEEP-LAST-ratchet;
> re-anchor no-op) → **α-başı KOŞULLU-ERTELENDİ** (tetik-kriterli); §3-karşı-olgu m=0-hatası
> düzeltildi (kilit m=1'de ölçüldü → tohum=125 senaryosu ~342 B/s kilit = 78s = KURTULURDU);
> tavan-teoremi yumuşatıldı (gönderim-anı-gain + geç-ACK istisnaları, 6 ölçülü olay);
> ProbeUp-köprü 420s→~250-340s (sonuç ayakta); dal-B lotarya %65-ihlal gerçeği → K-mühür
> istisna-kuralı S-sorusu; temiz-1k×β1 nedenselliği düzeltildi (1k'da recovery HİÇ girmiyor —
> β1 1k-inert, ölçülü 0/240).

## 0. HÜKÜM: TEK-ZİNCİR-İKİ-MEKANİZMA (çift-kilit) — ve bir hipotez-düzeltmesi

Abkarada-sorusunun cevabı: **NE TEK-KÖK NE BAĞIMSIZ.** Tek nedensel zincirde iki AYRI kod-
mekanizması: tohum kilidin SEVİYESİNİ kurar (kilit-rate ≈ 0.91-0.98 × 2.885×tohum — dört
düşük-koşuda kilit m=1'de, yani İKİNCİ örnekte: motorun kendi-Startup-pacing'ini ölçen örnek
tavan olur), yapışma tavanı MÜHÜRLER (gain=1 → yeni-gönderim örnekleri ≤ btlbw). Kanıt iki
yönlü: tohumu düzelenler yapışma-ÖNCESİ kurtuldu (r4_3/v8_3: merdiven pend=0 iken); yapışması
kısmen çözülen v8_1 (RecoverFwd→ProbeUp t=118.5) tohumu düzeltemedi (135→131 donuk).

**HİPOTEZ-DÜZELTMESİ:** "pending sürekli-yenileniyor" VERİYLE ÇÜRÜDÜ — dört düşük-koşuda 120s
boyunca **retx=0 B, dup=0 B**. RecoverArq 44-107s **BOŞ NÖBET** (tek retransmit emit edemedi —
token-açlığı §2-B3); pending'in fiilî tek boşalma-yolu backlog-tükenmesi (38/39 olay sent_u=100).

## 1. Mekanizma-A: tohum + dairesel-TAVAN (r2-yumuşatılmış)

- **Tohum-formülü (6 koşu, sapma ≤%5):** tohum = k×267/t: 36(2;15.0s, r4_6 VE r4_3 özdeş) ·
  31(1;8.5s) · 50(2;10.7s) · 62(1;4.3s) · 33(5;40s) — dürüst-btlbw (≈10.269 = 267B/26ms)
  yüzde-birinin altı katları. Kod: `bw.rs:62-66` (pipe-boş çıpası) + `bw.rs:85-87` (ilk-SACK'te
  ack_elapsed = tüm deneme-gecikmesi; İLK ack'e kadar — sonrasında her ack delivered_time'ı
  tazeler, `bw.rs:82`).
- **TAVAN (r2):** token-pace'te örnek ≤ GÖNDERİM-anı-gain × btlbw. Kararlı gain=1 rejiminde
  yeni-gönderim örnekleri btlbw'yi BÜYÜTEMEZ (ölçüm: 4 stuck-koşuda 309s RecoverArq'ta büyüme 0;
  yalnız pencere-decay 88→82, 163→154, 135→131). İstisnalar (ölçülü 6 olay, hepsi dürüst-btlbw'ye
  ZATEN ulaşmış koşularda, ≤%1): Startup-artığı geç-ACK örnekleri + interval-sıkışması.
- **Tavan-kırıcı: kısa-gap SACK-ÇİFTİ** → örnek ≈ 2×rate; gain=2.885 altında bileşiklenir →
  merdiven ×285 (r4_3: 36→204→629→1863→10265, 4 örnek/14s; oran-kanıtı 204/103=1.98).
  **MÜKEMMEL-AYIRICI:** 2.örnek/rate = kurtulanlarda 1.98/1.98; düşüklerde 0.91-0.98 (4/4).
  Tohum DEĞERİ/ZAMANI kader değil (r4_6 vs r4_3 özdeş-tohum → 33 vs 100); belirleyen 2.SACK'in
  gap'i (2.5-3.0s vs 8-41.5s) ∧ kayıp-kanıtsızlığı.
- Mevcut geçerlilik-kapısı yakalamıyor: `sender.rs:507` valid=trivially-true; reverse_path_impaired
  kablosuz; MIN_RATE_INTERVAL=1ms (tohum-interval'i 4-40s). is_app_limited bu sınıfta daima false
  (yalnız on_sent/!more_pending — stuck'ta backlog hep dolu).

## 2. Mekanizma-B: yapışma — ÜÇ kod-yapısal bileşen

- **B1-latch:** `sender.rs:827-830` pending>0 → RecoverArq; Steady/ProbeUp erişilmez (ölçüm:
  düşük-koşularda RecoverArq-örneklerinin %100'ü pend>0; Steady-tick=0; probe=0/0/0/0).
- **B2-gain (INV-RATE-1 gerilimi — S1):** `sender.rs:1045-1050` recovery → gain 1.0; Startup-
  rampı kayıp-ilanıyla KESİLİR (r4_6: 96'da dondu; 2.885 sürseydi 277) ve çıkış Steady'ye
  (`sender.rs:951`) — 2.885'e dönüş yolu YOK. ProbeUp-köprüsü aritmetik-yetersiz (r2-düzeltilmiş):
  1.25×'le 131→10265 = 19.5 prob; periyot canlı-epoch'la küçülür → **≈250-340s ≫ 120s** —
  dal-A-köprüsünün kantitatif ölümü; kurtuluş RAMP-korunumundan geçer.
- **B3-token (ARQ-açlığı):** plan var (arq=200) ama 1ms-tick'te olay-başı refill 0.08-0.16B →
  token 267'yi İLK aştığında DATA-önce (`sender.rs:1072-1082`) tüketir → **ARQ-payı yapısal 0**
  (`sender.rs:1105-1106` break; retx=0'ın kökü). Cap-tabanı ÇÖZMEZ (rezervasyon değil üst-sınır).
- Saat-genleşmesi (ikincil, mevcut-sınıflı): EAI@rate96 = 7.66s (dürüst 2.15s) → T_loss 11.5s,
  conf-eşiği 15.3s, probe-periyodu ~30s, W'ler 61.3s.

## 3. Zincir ve doz-cevap (r2-düzeltilmiş)

%80-dönüş → ilk-SACK t → tohum → rate=2.885×tohum → arz endojen-düşük (iki uçtan rate-kilitli)
→ 2.SACK'e dek kayıp-kanıtı olasılığı yüksek → pend>0 → latch+gain=1+token-0 → kilit-rate =
m=1-örneği (0.91-0.98×2.885×tohum) → backlog-tükenme tek-çıkış: kilit 88-163 B/s → 164-303s >
120s → STUCK {30,33,55,63}; kilit 308-990 (ölçülü-zarf) → 27-87s → geç-kurtuluş→100.
**Break-even ≈ 222 B/s.** r1-karşı-olgusu DÜZELTİLDİ: tohum=125 olsaydı kilit ≈ 342 B/s →
78s → KURTULURDU — yani tohum-düzeltmesi TEK BAŞINA da kurtarıcı olabilirdi; ama A-başı için
elde işleyen mekanizma yok (§4-α gerekçesi) → yol β1 (ramp-korunumu: merdiven tohumdan bağımsız
işler, r4_3-kanıtı).

## 4. Fix-paketi (r2): **TEK-TUR-İKİ-KOMMİT + α-KOŞULLU-ERTELEME**

**ÖN-KOŞUL: TUR-6 (PR #36) merge** — β1 plato/round-etkileşimi o zemine pinli.

| commit | fix | pinli-kararlar | ZM | dokunuş |
|---|---|---|---|---|
| **β1 (ramp-korunumu)** | `base_phase ∈ {Startup, Steady}` alanı: recovery-overlay pacing-gain'i BASE'den alır; recovery-çıkışı BASE'e döner | (i) Drain/DrainRtprop/ProbeUp kesilirse base=Steady'ye düşer (bayat entry-ns sorunu kökten yok — o fazlar overlay-sonrası kendi tetikleyicileriyle yeniden girer; ProbeUp-preempt muhasebesi mevcut); (ii) **plato BASE-üstünde değerlendirilir** (base==Startup iken update_state HER kolunda round-ilerleme kontrol edilir — delivered-anchor round'lar recovery sırasında da tamamlanır; plato → base=Steady'ye terfi, Drain'e normal geçiş recovery-bitişinde); (iii) **recovery-dönüşü ≠ re-entry**: TUR-6 reset'i YALNIZ gerçek-girişte (Idle/PreConverge'den) — base korunduğundan dönüş girişe sayılmaz; (iv) eternal-2.885 riski: plato-on-base sayesinde ramp recovery'de de PLATO-sonlu; ayrıca cwnd/srtt + qd-soft-drain mevcut frenler (beyanlı, ölçümde izlenir) | saat YOK (gain-seçimi; faz-saatleri değişmez) | sender.rs |
| **β2 (ARQ token-payı)** | pending>0 iken her **K_ARQ_SHARE**'inci pacer-grant'i ARQ-önce kullanır (retx front-of-line); diğer grant'ler DATA-önce (ölçülü-karar korunur, pay sınırlı) | cap-tabanı seçeneği RED (yapısal-ölü — doğrulama-HIGH); başarı-metriği retx>0 wire-kanıtı + oran ≤1.25× | saat YOK (grant-sayacı; K_ARQ_SHARE boyutsuz [CAL {4,8}]) | sender.rs (schedule_transmit) |
| **α (tohum-dürüstlüğü) — KOŞULLU-ERTELİ** | TETİK: β1+β2-ölçümünde stuck-sınıfı sürer ∧ trajektoride tohum-düz (btlbw m=1-seviyesinde donuk) koşu görülürse → AYRI mini-not: mekanizma-adayı **örnek-anı ack_elapsed-clamp** (bw.rs on_ack; r1'in iki kolu doğrulamada ÖLDÜ: admission-yolu etkisiz/negatif — max-filter'da bastırma yok, dışlama decay'i öldürür + KEEP-LAST-ratchet; re-anchor no-op — zehir per-paket snapshot'ta) | atıf-netliği: stuck-kapanışı **β1'e** atfedilir (r1'deki çift-atıf çelişkisi kapandı) | clamp-eşiği boyutsuz-oran sınıfı olmalı (K_CIRC [CAL]); mutlak-zaman YASAK — tasarım tetiklenirse | bw.rs (tetiklenirse) |

**Ölçüm-planı:** commit-başına p5×6 (ayrı-atıflama) + smoke (p4-1.00× / temiz-1k≥45 / 0.25k-20/20).
β1'in temiz-1k'ya etkisi BEKLENMEZ (r2-düzeltme: 1k'da recovery hiç girmiyor — pend>0 örneği
0/240, ölçülü; eternal-Startup/inflight-cap TUR-6-zemin borcu olarak AYRI, §0.25).
**Paket-sonu P5-GATE TAM-MÜHÜR sınavı:** K-2 (min≥80) ∧ K-3 (tip-7 IQR≤20 ∧ min≥80, iki 6'lık seri).

**İki-dallı dürüst-tahmin:**
- dal-A: β1-ramp-korunumu → stuck-sınıfı r4_3-sınıfına döner (kayıp-ilanı merdiveni kesmez;
  2.örnek 1.98×-sınıfına kayar) + β2 retx>0 → K-2/K-3 → TAM-MÜHÜR.
- dal-B: **ölümcül-lotarya sınıfı** (ilk-SACK ≥ ~100s; ölçülü 2/24 ≈ %8.3 — hiçbir sender-fix'i
  kurtaramaz, kalan-süre <10s) fix'lerden bağımsız yaşar → **12-koşuluk mühür-serisinde P(≥1
  min<80) ≈ %65** (r1'in "nadir" nitelemesi YANLIŞTI — doğrulama-bulgusu). → **S5-kuralı şart.**

## 5. Beklenti-kaydına girecekler (kod-GO sonrası, ölçümden ÖNCE)

β1: p5 stuck→100-kayması (atıf: β1); 2.örnek/rate ayırıcısı 0.9x→1.9x-sınıfına; temiz-1k
DEĞİŞMEZ (recovery-inert, ölçülü); plato-on-base ile p5'te Startup-plato artık recovery-altında
da sonlanır (eternal-2.885 penceresi ölçümle izlenir). β2: retx>0 + oran≤1.25×. K-1a/K-1b
(kafes) KORUNUR. Lotarya-koşuları S5-kuralıyla işaretlenir.

## 6. Açık sorular (onayla birlikte)

- **S1 (spec-yorumu):** mevcut B2-gain davranışı INV-RATE-1 ihlali mi ("loss send_rate'i
  DÜŞÜRMEZ" — kayıp-ilanı gain'i 2.885→1.0 kesiyor)? β1 ihlal-fix'i olarak mı çerçevelenir?
- **S2 (β2):** K_ARQ_SHARE başlangıç-değeri {4,8} — öneri 4 (pending küçük; pay %25).
- **S3 (β1-mimarisi):** minimal base∈{Startup,Steady} (öneri — küçük-dokunuş) vs tam
  overlay-split (recovery hiç pacing-fazı olmasın — mimari-temiz ama büyük refactor; Beta'ya?).
- **S4 (MIN_CWND/inflight-cap borcu):** TUR-6-maskesiyle ölçülü-güçlendi (§0.25); β1'le
  nedensel bağı YOK (r2-düzeltme) — öne-çekilme ayrı-karar, bu pakete girmez (öneri: SON'da).
- **S5 (mühür-lotarya kuralı):** K-2/K-3 değerlendirmesinde "ölümcül-lotarya" koşusu (ilk-SACK
  ≥ ~100s; binom-uyumluluk şartıyla) mühür-kararında NASIL sayılır? Öneri: seri geçerli-sayılır,
  lotarya-koşusu İSTİSNA-işaretli raporlanır ve min-kapısı lotarya-dışı koşullarda aranır;
  istisna-sıklığı binom-üstüyse (≥3/12) seri KIRMIZI. Karar Abkarada'nın.
