# PERSIST — reorder horizon kalibrasyonu (A/B adaylarıyla)

**Soru (owner):** reorder yüzünden L2 ne zaman yanlış açılıyor; cold-start'ı geciktirmeden nasıl engellenir?
**Beklenti kaydı:** `EXPECTATION.md` (koşudan ÖNCE). **Kod:** bkz. commit mesajı. **n=3 (tarama).**
Modlar P0 fixed 0.5×srtt (sevk) · P1 learned Q90·1.25 · P2 learned Q95·1.25 (taban 0.5) · P3 fixed 1.0×srtt
(kör referans). Politikalar A `250-250-250` · B `250-375-375`. Her tablo ÜÇLÜ: tüm-rep / stall-dışı / stall.

## 0. Veri kalitesi ve tasarım düzeltmeleri (sonuçlardan ÖNCE)

**C2/C3 vakum çıktı.** `delay … jitter` ile `rate` birlikte kullanılınca netem gönderim zamanlarını
monoton kılar → jitter reorder ÜRETMEZ (`spurious = 0`, L2 = 0, örnek 0, tüm kollar). O iki hücre hiçbir
şey ölçmedi; öngörü 4/5 bu hâlleriyle sınanmadı. **Yeniden tasarım:** iki-aşamalı netem — reorder
aşaması gönderici tarafında (`va0`, rate'siz), gecikme+rate MID'de. netem `reorder` semantiğinde reorder'lı
paket ERKEN gelir → ömür ≤ aşamanın base delay'i ≤ OWD_fwd ≤ **0.5×RTT (simetrik yolda üst sınır)**;
ömür > 0.5×srtt yalnız **asimetrik** yolda mümkün. Yeni hücreler: **C2p** (RTT 200, ömür ≈ 20 ms = 0.1 srtt —
"P0 zaten yeterli" kontrolü) ve **C3p** (asimetrik: ileri 40 ms reorder-lag, geri 5 ms → RTT ≈ 45, ömür ≈
0.9 srtt — stres). C3p'de RTT bimodal (reorder'lı paketler ~5 ms) → srtt/rtprop tahmincileri de zorlanır;
confound olarak kaydedildi. Beklenti kaydına EK olarak koşudan ÖNCE yazıldı.

**Stall tabakası:** her tablo üçlü (tüm-rep / stall-dışı / stall). C4/C5'te stall sık (%20 kayıp).

**Wire Δ% reorder hücrelerinde gürültülü:** parity 0.5–1 kB iken retx varyansı 0.5–2.8 kB → persistence
maliyetinin doğru ölçüsü **parity byte'ı ve L2 sayısı**, wire Δ% değil.

## 3. Cold-start kapısı — L1 moddan bağımsız (SAĞLANDI)

Stall-dışı rep'lerde `t_first_L1 − t_first_kuşku` medyanı her hücre ve her modda **0 ms** (n=2 önizleme;
tek istisna C1a/96/B P3 = 245 ms, n=2 gürültüsü; n=3 tablosu §2'de). L1 kod yolu persistence ufkunu
okumaz — birim testi `persist_modu_l1_zamanlamasini_degistirmez` bunu pinler; netem yalnız sağlama.
**Cold-start geri gelmedi.**

## 5. C4 (kayıp + reorder) — n=3'te SONUÇSUZ; mekanizmalar tespit edildi

* **Parity token açlığı:** A-P2 rep'lerinde latch `enqueued 471 / emitted 274` (P0: 464/464) — parity
  kuyruğa girip emit edilmedi; koşu sonu durumu `RecoverArq` / `DrainRtprop`. Kayıp+reorder'da state
  machine parity emisyonunu aç bırakıyor. INV-RATE-1'in harfine değil ruhuna dokunan bir gözlem
  (kayıp → state → parity kapasitesi); persistence'ın işi değil, ayrı kayıt.
* **Gate açılış varyansı:** StructGate kovası A-P2'de 3034, A-P0'da 0 — reorder fate-estimator'ın
  `burst_delta`'sını oynatıyor, gate koşudan koşuya açılıyor/kapanıyor.
* Stall %25–50 (n=3'te 1–2 rep). Bu üç varyans kaynağı altında C4 kararı n ≥ 20 + tabaka ister.

## 1. Beklenti kaydıyla karşılaştırma (temel aile, n=3)

| # | öngörü | ölçüm | hüküm |
|---|---|---|---|
| 1 | C5/C0: P0 ≡ P1 ≡ P2 (spurious yok → taban) | ufuk **103/102/103 ms** özdeş; parity 8 280/8 280/8 464 (96) · 43 240/43 332/43 332 (400); L2 2 906/2 363/2 385 · 11 524/11 060/10 996 — varyans içinde. **"byte-özdeş" fazla iddialıydı**: rastgele kayıp çekimi byte'ı değiştirir; doğru form "konfigürasyon-özdeş, ufuk özdeş" | ✓ (dil düzeltildi) |
| 1b | P3 L2'yi +100 ms geciktirir, d2 −0.02…−0.05 | t_L2 633–639 vs 526–533 (+~105 ms) ✓; d2 farkı n=3'te çözülemedi (96 A +0.10, 400 A −0.05, B −0.03) | ✓ / belirsiz |
| 2 | cold-start: t_L1 moddan bağımsız | stall-dışı `t_L1 − t_kuşku`: reorder hücrelerinde 0 ms, C5'te 10–51 ms (instant ≥10k için birkaç kuşku), yayılım ≤ 34 ms; tek 245 ms (n=2) | ✓ |
| 3 | C1a/b: P1/P2 yanlış L2'yi düşürür, sıfır değil; P3 ≈ 0; B'nin cezası A'dan büyük | **C1b/96 A: 101 → 19 → 0 → 0; B: 52 → 9 → 26 → 0. C1b/400 A: 46 → 0 → 46 → 0; B: 117 → 0 → 0 → 0. C1a/400 A: 15 → 0 → 0 → 0.** Ufuk 107–146 ms (Q90/95 ≈ 85–117 ms × 1.25). B-P0 > A-P0 ✓ | ✓ (bir aykırı: C1b/400/A-P2 = 46) |
| 4 | C2 → **C2p** (EK): ömür ≈ 0.1 srtt ≪ P0 ufku → tüm modlarda yanlış L2 ≈ 0, P0 ≡ P1 ≡ P2 | yanlış L2 **0/0/0/0** her kolda (96 ve 400); ufuk P0/P1/P2 95–98 ms (= taban), P3 192–197; wireΔ ≤ +0.33 % | ✓ |
| 5 | C3 → **C3p** (EK): P0 (≈15–22 ms) ÇOK yanlış L2; P1/P2 ≈ 50–60 ms → belirgin düşer, sıfır değil; P3 (30–45 ms) kısmen; **confound: bimodal RTT srtt'yi zorlar** | P0 46–61; P1 2–24; P2 8–15; P3 0–58 (bıçak sırtı); ufuk P1/P2 31–62 ms; srtt_son ≈ 10–16 ms gözlendi (B-P3/96) = kayıtlı confound | ✓ |
| — | C2/C3 orijinal | **VAKUM** (netem `rate` jitter reorder'ı öldürdü, §0) | sınanmadı |
| 6 | C4: learned reorder-L2'yi eler, gerçek-kayıp L2 +50 ms | n=3'te SONUÇSUZ — §5 (stall, gate varyansı, parity açlığı) | belirsiz |

**Sapma dekompozisyonu:** öngörü 1'in "byte-özdeş" dili yanlış (öngörü hatası, kod hatası değil); C1b/400/A-P2
aykırısı n=3 gürültüsü ya da halkanın geç dolması (ilk ~100 ms'de ufuk = taban) — n ≥ 20'de ayrışır.

## 2a. Çakışma hücreleri — yanlış L2 ve maliyeti (stall-dışı medyan; saf reorder → her L2 yanlış)

| hücre | kol | yanlış L2: P0 → P1 → P2 → P3 | parity B (P0/P1/P2/P3) | ufuk ms (P0/P1/P2/P3) |
|---|---|---|---|---|
| C1a 15 % · 96 | A | 0 → 0 → 0 → 0 | 276/276/276/368 | 106/117/107/209 |
| C1a 15 % · 96 | B | 26 → 2 → 0 → 0 | 460/460/460/368 | 104/114/119/212 |
| C1a 15 % · 400 | A | 15 → 0 → 0 → 0 | 368/460/368/368 | 94/129/107/208 |
| C1a 15 % · 400 | B | 0 → 7 → 0 → 0 | 276/368/92/460 | 106/116/111/213 |
| C1b 30 % · 96 | A | **101 → 19 → 0 → 0** | 736/644/736/828 | 81/129/132/201 |
| C1b 30 % · 96 | B | 52 → 9 → 26 → 0 | 736/828/828/644 | 89/134/135/172 |
| C1b 30 % · 400 | A | 46 → 0 → 46 → 0 | 644/736/920/828 | 87/129/146/149 |
| C1b 30 % · 400 | B | **117 → 0 → 0 → 0** | 1 012/460/460/736 | 96/116/126/157 |

**Okuma:** (i) sabit 0.5×srtt (P0) çakışma hücresinde 15–117 yanlış L2 üretiyor; öğrenilen ufuk (P1/P2)
çoğu hücrede **0–19**'a indiriyor; P3 (1.0×srtt) hepsinde 0. (ii) **Mutlak maliyet küçük:** parity 0.3–1 kB,
telin ≤ %1.2'si — yanlış L2'nin bedeli bu hücrelerde owner'ın "%1–2" bütçesinin içinde, ama B'de (L2=r3)
iki kat. (iii) `emit/enq` her yerde eşit (parity açlığı YOK — saf reorder'da ARQ yükü düşük). (iv) d2 ≥ OFF
çoğunlukla; altına düşen 3 hücre (0.78–0.80 vs 0.84) n=3 gürültü tabanı (±0.1) içinde.

## 4. A vs B reorder altında

B (`250-375-375`) yanlış L2 başına daha çok parity öder (L2 = r3): P0'da B'nin yanlış L2'si A'nın
2–3 katı (C1a/96: 26 vs 0; C1b/400: 117 vs 46) ve parity 1 012 vs 644 B. Öğrenilen ufuk B'yi A'nın
düzeyine indiriyor (C1b/400: 117 → 0). **Persistence B için daha kritik.** Kayıp-only'de (C5) B, A'ya
+10…+13 pp teslim (400: 0.810 vs 0.682) — r-sweep'le tutarlı; reorder'sız persistence modu B/A sırasını
DEĞİŞTİRMİYOR.

## 2b. Yeniden tasarlanan reorder hücreleri C2p/C3p (108 koşu, n=3, stall 0 — hepsi stall-dışı)

**C2p** (RTT 200; `va0: delay 20ms 15ms reorder 30% 50%` + MID `delay 80ms rate 1mbit`; ömür ≈ 20 ms):
yanlış L2 **0** — 8 kol × 2 akış hepsinde. Spurious 4–8 birim/koşu var (ölçüm canlı), ama yaşları
P0 ufkunun (97 ms) çok altında → öğrenilen ufuk taban'a düşer (95–98 ms ≡ P0). wireΔ ≤ +0.33 % (96),
≤ +0.14 % (400); parity 0 (üç kolda 184 B = tek L1 bloğu, mod-bağımsız L1 gürültüsü). d2 0.875–0.998 vs
OFF 0.980–0.990 — n=3 retx gürültüsü (±0.1). **P0 yeterliyken öğrenme bedava** (öngörü 4 ✓).

**C3p** (RTT ≈ 45 asimetrik; `va0: delay 40ms 10ms reorder 30% 50%` + MID `rate 1mbit` + rev 5 ms;
ömür ≈ 40 ms ≈ 0.9 srtt — karar hücresi):

| akış | kol | yanlış L2: P0 → P1 → P2 → P3 | ufuk_son ms (P0/P1/P2/P3) | wireΔ % (P0/P1/P2/P3) | retx B |
|---|---|---|---|---|---|
| 96 | A | **46 → 8 → 10 → 48** | 18/31/60/47 | +1.84/+1.63/+3.29/+4.10 | 1 429/1 338/1 875/2 412 |
| 96 | B | **53 → 6 → 15 → 15** | 20/54/62/12† | +3.07/+0.66/+3.47/+4.29 | 1 875/546/1 957/2 221 |
| 400 | A | **59 → 24 → 8 → 58** | 22/50/31/42 | +0.99/+0.25/+0.49/+1.16 | 2 312/983/1 602/2 614 |
| 400 | B | **61 → 2 → 14 → 0** | 22/31/39/49 | +1.09/+0.30/+0.30/−0.01 | 2 576/1 247/1 247/719 |

† `ufuk_son` = son tick değeri; B-P3/96'da 9.7–16 ms, tepe 49–52 ms. P3 = 1.0×srtt olduğundan bu,
**srtt_son ≈ 10–16 ms** demektir (yol RTT'si ≥ 35 ms). Kaynak: netem `reorder` paketleri gecikmeyi
ATLATIP erken gönderir → RTT örnekleri bimodal (≈10 / ≈45 ms) → srtt son örneklere göre salınır. Bu
confound EK'de koşudan ÖNCE kaydedildi; Karn-sınıfı kusur DEĞİL. Öğrenilen ufuk spurious YAŞINI ölçer,
srtt'den bağımsızdır — C3p'de sabit-katsayı modlarına karşı avantajının bir kısmı bu.

**Okuma:** (i) P0 (0.5×srtt ≈ 20 ms < ömür 40 ms): 46–61 yanlış L2 — öngörü 5 ✓. (ii) Öğrenilen (P1/P2):
**8/8 karşılaştırmada P0'dan iyi, 2.5–30×** (2–24 kaldı; sıfır değil — öngörü "sıfır değil" ✓; kalan:
halka dolmadan önceki ilk örnekler + Q95 kuyruğu). (iii) **P3 bıçak sırtında:** A-P3 ≈ P0 (48/58), B-P3 15/0.
Rep düzeyi (400): B-P3 L2 = [0, 0, 32] ufuk_son 45–49 ms; A-P3 L2 = [105, 31, 58] ufuk_son 39–45 ms →
ömür 40 ms'in 42 ile 47 ms arasına düşmesi tesadüf; sabit katsayı ömrü DEĞİL srtt'yi izler — tasarım
değil yazı-tura. (iv) Tel: 96'da retx-hâkim (546–2 412 B; reorder'ın spurious retransmit'leri ARQ'nun,
persistence ARQ'ya dokunmaz → n=3 RNG gürültüsü), parity 92–368 B (≤ 0.4 %). 400'de öğrenilen modlar
toplam ek yükü **2–4× düşürür** (A +0.99 → +0.25/+0.49; B +1.09 → +0.30/+0.30). (v) d2 0.979–1.000 her
kolda (2×RTT = 90 ms > ömür) — mod etkisi yok ✓. (vi) Cold-start ofseti 0–5 ms ✓.

## 6. Hüküm

1. **Sabit 0.5×srtt (P0, sevk) ömür ≥ 0.5×srtt olan kanallarda yetersiz:** C1b 46–117, C3p 46–61 yanlış L2.
   Mutlak bedel bu hücrelerde küçük (C1b tel ≤ %1.2; C3p/400 +1 %) ama B'de (L2 = r3) iki kat ve reorder
   oranıyla büyür. Ömür, srtt'nin değil kanal asimetrisinin/jitter'ının fonksiyonu → srtt-katsayısı
   yanlış nicelik (sabit-kaydırma yasağıyla tutarlı).
2. **Öğrenilen Q90/Q95×1.25 (P1/P2):** kayıp-only'de inert (C5: ufuk = taban 103 ms, parity/L2 eşit), P0'ın
   yettiği yerde inert (C2p), P0'ın çöktüğü yerde yanlış L2'yi **4–30× düşürür** (C1a/b, C3p). Çakışma
   hücrelerindeki 24 karşılaştırmada (12 hücre×kol × {P1,P2} vs P0): **18 daha iyi, 5 eşit, 1 daha kötü**
   (C1a/400/B-P1: 7 vs 0, n=3). L1 zamanlaması hiçbir modda değişmedi (§3).
3. **P3 (1.0×srtt) elenir:** C1a/b'de 0 verir ama gerçek kayıpta L2'yi +105 ms geciktirir (C5) ve C3p'de
   bıçak sırtı (3-ii). Öğrenilen mod aynı korumayı kayıp-only'de SIFIR bedelle veriyor → P3 domine.
4. **P1 vs P2 n=3'te ayrışmaz** (C1b/96/A: 19 vs 0 P2 lehine; C3p/400/B: 2 vs 14 P1 lehine). Öneri: **P2
   (Q95) birincil** (kuyruğu kapsar, teorik kenar), P1 `[CAL]` alternatif olarak kalır; A ve B birlikte taşınır.
5. **SEVK KARARI YOK.** Default `FixedSrtt` kalır. Sevk-öncesi açık borç: karışık hücre (C4, kayıp+reorder)
   n=3'te SONUÇSUZ (§5) — yükselen ufkun gerçek-kayıp L2'sine bedeli (öngörü 6: +50 ms, d2 −0.00…−0.03)
   ölçülmedi. n≥20 kampanyası önce C4'e gitmeli (stall-stratifiye, parity-açlığı sayaçlarıyla).
6. **Ayrı kartlar (bu kampanyanın dışında):** (a) parity token açlığı `emitted < enqueued` (RecoverArq/
   DrainRtprop, §5); (b) kuruluş stall'ı %13–16 (owner: ayrı issue); (c) bimodal-RTT altında srtt/rtprop
   davranışı (C3p confound; pacing/zaman-eşiği tüketicileri) — bench diag satırına `srtt` alanı eklenmeli
   (bu kampanyada kod dondurulduğu için eklenmedi).
7. **Kapı durumu (tam workspace, `--all-features`, -j2, netem bittikten sonra):** 509 geçti / **2 FAILED**
   (client-core `runtime_e2e`), ikisi de izole ×3 (feature'lı ve feature'sız) GEÇİYOR → paralel yük altında
   zamanlama. Gizlenmedi; Rust koduna dokunulmadı (kampanya dondurulmuş). Kartlar: (i)
   `fragmenter_message_survives_a_permanently_erased_chunk_via_fec`: test ilk teslimden sonra `shutdown`
   çağırıyor; yük altında ikinci offer işlenmemiş kalıyor (`offered 1, unaccepted_offers 1, delivered 1`) →
   K=8 blok dolmuyor → assertion. Test yarışı; düzeltme testte (`acked ≥ 14` / ikinci teslim beklensin).
   (ii) `bootstrap_rtt_probe_and_echo_traverse_the_product_wire` OFF kolu: `bootstrap_rtt: false`'a rağmen
   `cs_probes_tx 1, cs_echoes_rx 1, rtt_samples 1, Startup` — CsProbe'un İKİ bağımsız yayıcısı var
   (DR-CS-3 merdiveni `bootstrap_rtt` ile kapılı; deneysel cold-start F2A kuyruğu `sender.rs:2698`
   `--all-features`'ta derlenir ve bu bayrağa bakmaz, tetikleyicisi zaman-bağımlı). Sunucu CS-probe'u
   mühürlü yolun dışında echo'lar → RTT örneği → PreConverge'den çıkış. Test "OFF ⇒ probe yok" iddiası
   kodun garanti ettiğinden güçlü → ya test deneysel kolu da kapatır ya F2A tetikleyicisi bayrağa bağlanır
   (ürün kararı, owner). Her iki test yalnız bu dalda (8a8b0ec9, cf769780) — hosted CI görmedi.
8. **Enstrüman notları:** netem `delay … jitter … rate` REORDER ÜRETMEZ (§0) — eski C2/C3 sonuçları (varsa)
   geçersiz; `ufuk_pk` (tepe) kolonu analizöre eklendi; bekleme döngümün "rc≠0: 108" sayacı yanlış anahtardı
   (`rc=0` biçimi), veri sorunu yok.

**Sonraki adım (owner sırası):** evidence-mass (payda tabanı yok) → sağ kalan adaylarda (A/B × P2, P1 yedek)
n≥20 stall-stratifiye, C4 öncelikli → `remaining_recovery_rounds` → birleşik regresyon → spec amendment EN SON.
