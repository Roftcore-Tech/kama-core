# EVIDENCE-MASS — kanıt kütlesi spekülatif L2 yetkisine girmeli mi? (tarama, n=5)

**Taban:** `f5b73da8` (persistence kalibrasyonu). **Soru (owner):** kanıt kütlesi küçükken risk tahmini ne kadar
FEC yetkisi taşımalı — ilk risk sinyalini GECİKTİRMEDEN. Kapsam: yalnız spekülatif L2 yetkisi; kanonik
kayıp/ARQ/pacing/Q18/yapı-gate/A-B/P2/wire dokunulmadı. Beklenti kaydı: `EXPECTATION.md` (koşudan ÖNCE).

## 0. Taban kapısı, tanım, enstrüman, politika kolu

**Taban kapısı (`f5b73da8`, tam workspace `--all-features`, -j2):** fmt 0 · clippy 0 · **test 101**:
`bootstrap_rtt_probe_and_echo_traverse_the_product_wire` tam-workspace koşusunda 2/2 kırmızı, izole 6/6 yeşil
(feature'lı/feature'sız) → paralel yük altında zamanlama; mekanizma persist raporu §6.7'de (CsProbe çift
yayıcı, F2A kuyruğu `bootstrap_rtt` bayrağına bakmaz). Bilinen ikinci flake: `client_survives_server_death_mid_run`
(sunucu-ölümü yarışı; izole 3/3 yeşil). **İkisi de bu görevin dışında, düzeltilmedi, sonuçlardan ayrı tutulur.**
Diğer 510 test yeşil; test-adı anlık görüntüsü alındı (388 ad).

**Kanıt-kütlesi tanımı:** `evidence_mass = sent_window_units` — `fec_loss_risk_split` paydasının KENDİSİ
(rolling W_loss penceresinde gönderilmiş birimler; kanonik `loss_windowed_ppm` ile aynı payda). Ayrım
(EXPECTATION §0): fate'i bilinmeyen in-flight birimleri içerir; olay üretmiş birimler `declared_units`.
Ham okunur, taban/normalizasyon YOK. Küçük-payda mekaniği: `l2_min_persistent_ppm = 10 000` ⇒ pencere ≤ 100
birimken TEK yaşlanmış gap L2'yi uygun kılar (96-mesajlık akışta pencere 100'ü aşmaz).

**Enstrüman (ürün yolu, salt gözlem):** `RiskSplit.sent_window_units` · `TsSample.{evidence_mass, declared_units}`
(50 ms seri; `t_ms` = akış yaşı) · `Sender::diag_spec_transitions()` = ilk L1 / ilk L2 anında
`(em, instant_ppm, persistent_ppm)` · `FateWindowDiag.fec_spec_transitions` · bench `em_l1=… em_l2=…` + ts.csv
kolonları. Mevcutlar korundu: risk_t (t_susp, t_conf, t_risk, t_parity), spec (…, t_L1, t_L2), lvl, persist.

**Politika kolu (bayrak, default OFF = byte-özdeş):** `FecLossRiskConfig.evidence_mode ∈ {Off, L2Floor, L2Scaled}`
+ `evidence_min_units` (M1) + `evidence_full_units` (M2). Yalnız `speculative_propose` içinde: M1 —
`persistent ≥ l2_min` ama `em < min` ⇒ seviye L1 (L1 tavanı; L0/L1 kararı kütleyi OKUMAZ); M2 — L2 tavanı
`l1 + (l2−l1)·min(em, full)/full` (tamsayı). Handoff klampı, reorder indirimi, `spec_min` tabanı, politika
tavanı SONRA ve değişmeden uygulanır. Birim testleri (7): default Off · Off knob okumaz (örnek dizisi özdeş) ·
floor L2→L1 ve L1 anı/instant değişmez, min=1 ⇒ Off'a özdeş · scaled aritmetiği 125/187/250 · scaled
seviyeyi korur, tavanı ölçekler · temiz yolda hiçbir mod ateşlemez · geçiş-diag'ı ts örneğiyle tutarlı.
Bench: `--em-mode off|floor|scaled --em-min N --em-full N` (unit test: RuntimeConfig'e akış).

**Smoke (E2-C1b, 1 rep):** invaryant `t_risk == t_susp` (374.98 ms = 374.98 ms) ✓; ts.csv yeni kolonlar dolu;
enstrümanın ilk gerçeği: **ilk ~200 ms boyunca `evidence_mass = 1`** — PreConverge ilk ACK'i bekler, kuyruk
sonra boşalır → em@L1 = 43 (öngörü 10–25'in üstü; sapma sınıfı (2), §1'de dekompoze edilir).

## 1. Faz-1 — enstrüman (M0 = mevcut davranış; off · A-M0 · B-M0; P2; 96 msg; n=5; 150 koşu, rc≠0: 0)

Ham: `phase1-M0.analysis.txt`, `runs-phase1-M0.tar.gz`. Birim = fragment (5 birim/mesaj; 96 msg = 480 birim).

**1.1 İnvaryant `t_first_risk == t_first_suspected`:** 86/90 ON koşu; 4 ihlalin HEPSİ stall koşusu
(t_susp 2.5–4.9 s; srtt = 0 iken Suspected'a güven verilmez → risk ilk RTT örneğini bekler — mevcut tasarım,
"ölçüm yokken risk uydurulmaz"). **Stall-dışı 86/86 tuttu.** Kütle bu yolu okumaz (M0'da okuyacak kod yok).

**1.2 Kütle yörüngesi (ts.csv, medyan):** em = **1** ilk ~200 ms (PreConverge ilk ACK'i bekler), 0.5 s'de
82–86, 1 s'de 255, 2 s'de 480 (akış bitti). Öngörü 2'nin "em@L1 ≈ 10–25"i YANLIŞTI: sapma (2) sınıfı —
mesaj ≠ birim (×5) ve kuruluş kuyruğu boşalması; enstrüman okuması düzeltildi, politika yorumu değil.

**1.3 E1 (kayıp-only, RTT 200) — kütle L2 anında KÜÇÜK DEĞİL:**

| kayıp | kol | stall | d2_ok (off) | wireΔ% | parity B | L2 sayacı | t_susp→t_L1→t_L2 ms | em@L1 | rsk@L1 k | em@L2 | prs@L2 k | tepe k @em (lvl) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2 % | A / B | 0 / 1 | 0.927 / 0.906 (0.885) | +13.5 / +11.7 | 5 704 / 4 968 | 1 619 / 1 100 | 446→877→928 / 556→576→858 | 220 / 104 | 10 / 11 | 235 / 202 | 12 / 12 | 18 / 14 @420–480 (2) |
| 5 % | A / B | 0 / 0 | 0.906 / 0.917 (0.703) | +14.9 / +22.6 | 7 360 / 11 408 | 2 085 / 2 075 | 431→588→627 / 435→570→620 | 107 / 102 | 10 / 11 | 118 / 112 | 13 / 16 | 33 / 30 @480 (2) |
| 10 % | A / B | 1 / 2 | 0.797 / 0.844 (0.635) | +18.2 / +27.4 | 8 188 / 12 328 | 2 176 / 2 226 | 455→499→557 / 432→483→534 | 82 / 78 | 12 / 10 | 96 / 90 | 11 / 11 | 65 / 70 @480 (2) |
| 15 % | A / B | 1 / 1 | 0.625 / 0.661 (0.427) | +16.0 / +24.6 | 8 372 / 12 282 | 2 489 / 2 262 | 432→466→534 / 443→485→545 | 75 / 79 | 12 / 12 | 90 / 92 | 11 / 16 | 103 / 105 @480 (2) |
| 20 % | A / B | 2 / 0 | 0.469 / 0.615 (0.292) | +15.5 / +24.4 | 8 464 / 12 328 | 2 469 / 2 238 | 427→450→529 / 431→469→532 | 71 / 75 | 11 / 12 | 88 / 89 | 11 / 11 | 139 / 143 @480 (2) |
| 25 % | A / B | 4 / 1 | 0.240 / 0.411 (0.224) | +17.3 / +22.3 | 8 648 / 12 512 | 2 836 / 2 432 | 425→434→527 / 425→448→527 | 68 / 70 | 12 / 12 | 84 / 86 | 12 / 12 | 184 / 183 @480 (2) |
| 0 % | A / B | 0 | 0.979 (0.979) | 0 | 0 | 0 | asla | — | — | — | — | risk 0 |

Okuma: (i) **em@L2 = 84–97 (Q10 85, Q90 94)** — 96-mesajlık akışta L2 hiçbir zaman "az kanıtla" açılmıyor;
ilk kuşku ≈ 430 ms'de pencere zaten ~70 birim. (ii) **Tepeler akış-sonu platosu** (em@pk = 480, seviye 2),
kayıp oranıyla ölçekli (2 % → 18k … 25 % → 184k) — küçük-payda tepesi DEĞİL; eski 195k/290k gözlemi bu
rejimde (RTT 200, kuruluş sonrası) yeniden üretilmedi. **→ Case D: tepe zararsız, "sayıyı düzeltme" yok.**
(iii) L2, `persistent ≈ 11–16k` ile açılıyor = **"1–2 yaşlanmış gap / ~90 birim"** — kısa akışta L2'yi
`l2_min_persistent_ppm = 10k` eşiği belirler; kütle geniş (≥84) olduğu için kütle-tabanı bunu ETKİLEMEZ.
(iv) 2–5 % kayıpta L2 (r2/r3) tel +12…+23 % ile d2 +2…+21 pp — bu rsweep bulgusudur (≤10 %'da r3 domine),
kütle sorusu değil: em@L2 orada 112–235 (BÜYÜK). (v) L1 gecikmesi `t_L1 − t_susp` 9–44 ms (≥10 %); 2 %'de
+431 ms = `l1_min_instant_ppm = 10k` tabanı (1 gap × 0.2–0.7 / 200 birim < 10k) — mevcut davranış.
(vi) Stall: off 0–3/5, A 0–4/5, B 0–2/5 (25 %'de A 4/5 → d2_all 0.000; medyan anlamsız) — politikadan
bağımsız, ayrı issue. Temiz yol (0 %): risk 0, parity 0, L1 = L2 = 0 ✓.

**1.4 E2 (saf reorder, P2 — her L2 yanlış):**

| hücre | kol | d2 (off) | wireΔ% | parity B | retx B (off) | L1 | yanlış L2 | t_susp = t_L1 → t_L2 ms | em@L1 | em@L2 | prs@L2 k | tepe k @em (lvl) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C1b 30 % RTT200 | A / B | 0.802 / 0.729 (0.771) | +2.21 / +1.55 | 736 / 460 | 810 / 792 (537) | 172 / 139 | **18 / 0** | 321→406 / 395→495 | 38 / 53 | 55 / 69 | 81 / 133 | 85 / 87 @55 (1) |
| C3p asim. RTT45 | A / B | 0.990 / 0.979 (1.000) | +1.65 / +1.47 | 184 / 276 | 1 247 / 1 074 (710) | 56 / 67 | **19 / 20** | 70→90 / 73→93 | 20 / 20 | 25 / 25 | 290 / 280 | 121 / 187 @25–28 (2) |

Okuma: küçük-em L2 **yalnız burada** (em@L2 10–80, med 25). Ama P2 altında bedeli: parity ≤ 736 B
(≤ %1.6 tel; C3p ≤ %0.5), telin kalanı ARQ'nun spurious retransmit'i (persistence/kütle dokunmaz), d2 etkisi
yok. **Kütle-tabanının kazanç tavanı ≈ %0.5–1.6 tel** — ve A'da sıfır (aşağıda).

**1.5 E3 (kayıp 20 % + reorder 30 %):** stall 2–3/5 her kolda → stall-dışı n = 2–3; d2_ok off 0.292 /
A 0.260 / B 0.297; em@L2 66–77, prs@L2 75–242k. n=5'te SONUÇSUZ (persist §5 ile aynı); faz-2'de n=10 ek.

**1.6 Yapısal gerçek (koddan):** A = 250-250-250 ⇒ L1 tavanı = L2 tavanı ⇒ `speculative_propose`'da L1 ile
L2 arasında ÖNERİ FARKI YOK ⇒ **M1/M2 A'da tanım gereği inert** (yalnız seviye sayacı değişir). Kütle
politikası yalnız B'de (375 vs 250) ölçülebilir; A kolları faz-2'de gürültü-tabanı kontrolüdür.

## 2. Eşik seçimi (önceden kayıtlı kural + erratum — EXPECTATION §3/EK)

E_lo = 26 (E2 yanlış-L2 em-medyanı 25'in üstü) · E_hi = 85 (E1 ≥15 % em@L2 Q10; kural metnindeki "Q90"
yazım hatası, parantezdeki tanım Q10'dur — erratum kayıtlı). E2 max 80 < E1 min 84: **ayrım var ama bu akış
geometrisinin ZAMAN artefaktı** (kütle ≈ kuruluştan beri geçen süre × 5 birim/msg / 20 ms): E1'de L2 ancak
t ≳ 500 ms'de (kuşku 430 + persist ~100) mümkün, E2'de reorder gap'leri 320 ms'den itibaren. Sabit bir
E_min bu zamanlamayı birim sayısı olarak kodlar → mesaj hızı/RTT/fragment boyutu değişince kayar
(sabit-kaydırma sınıfı). §4'te işlenir.

## 3. Faz-2 — M1 (floor) / M2 (scaled), setler 26:52 ve 85:170; P2; A/B; n=5 (400 koşu) + E3 ek n=10 (70 koşu)

Ham: `phase2-merged.analysis.txt` (ev1 + iki set birleşik), `phase2-E3sup-n10.analysis.txt`, `runs-phase2-*.tar.gz`.
rc≠0: 0/470. Kol etiketi: `B-M1@85` = floor em_min 85; `B-M2@170` = scaled em_full 170.

**3.1 İnvaryant ve L1 zamanı:** `t_first_risk == t_first_suspected`: **510 ON koşuda 17 ihlal, 17/17'si stall**
(t_susp > 1 s; srtt = 0 kuralı), **t_susp ≤ 1 s'de 0 ihlal** — her modda. L1: E2'de `t_L1 = t_susp` 11 kolun
hepsinde (C1b 318–395, C3p 70–88 ms; tek +2 ms); E1'de `t_L1 − t_susp` 44–135 ms, mod deseni yok — yapısal
olarak özdeş A kolları bile 44–101 ms yayılıyor (2./3. kayıp olayının zamanı, RNG). Temiz yol (E1-L0):
11 kolda risk 0 · parity 0 · L1 = L2 = 0.

**3.2 Gürültü tabanı (A kolları = aynı öneri yolu, farklı etiket; E1 stall-dışı, 5 kol havuzu):**

| kayıp | n | d2 medyan | d2 sd | d2 [min, max] | parity [min, max] B |
|---|---|---|---|---|---|
| 2 % | 25 | 0.906 | 0.019 | [0.89, 0.95] | **[0, 8 096]** |
| 5 % | 22 | 0.906 | 0.027 | [0.85, 0.95] | [3 128, 8 280] |
| 10 % | 22 | 0.802 | 0.046 | [0.69, 0.86] | [7 544, 8 464] |
| 15 % | 19 | 0.656 | 0.064 | [0.51, 0.78] | [7 728, 8 648] |
| 20 % | 18 | 0.500 | 0.048 | [0.41, 0.58] | [8 096, 8 648] |
| 25 % | 18 | 0.365 | 0.086 | [0.23, 0.51] | [8 280, 8 648] |

⇒ n=5'te çözülebilen en küçük etki: d2 ≈ ±0.05–0.09, parity ≈ ±5 % (≥10 %); **2–5 %'de parity 3×**
(kayıp-olayı zamanlaması piyangosu: t_L2 721–1 637 ms). Bunun altındaki her "fark" gürültüdür.

**3.3 E1 (gerçek kayıp) — B kolları (politikanın tek ölçülebilir olduğu yer):**

| kayıp | B-M0 d2 / parity / wireΔ | B-M1@26 | B-M1@85 | B-M2@52 | B-M2@170 | t_L2 ms (M0 → M1@85 → M2@170) |
|---|---|---|---|---|---|---|
| 10 % | 0.844 / 12 328 / +27.4 | 0.854 / 12 144 | 0.812 / 12 144 | 0.849 / 11 684 | 0.875 / 11 592 / +24.1 | 534 → 533 → 572 |
| 15 % | 0.661 / 12 282 / +24.6 | 0.708 / 12 236 | 0.677 / 12 328 | 0.677 / 12 512 | 0.781 / 12 144 | 545 → 528 → 531 |
| 20 % | 0.615 / 12 328 / +24.4 | 0.573 / 12 512 | 0.562 / 12 328 | 0.562 / 12 512 | 0.578 / 12 420 | 532 → 531 → 532 |
| 25 % | 0.411 / 12 512 / +22.3 | 0.479 / 12 328 | 0.464 / 12 512 | 0.438 / 12 512 | 0.479 / 12 328 | 527 → 527 → 531 |
| 5 % | 0.917 / 11 408 / +22.6 | 0.917 / 10 580 | 0.922 / 11 730 | 0.906 / 11 776 | 0.906 / 11 776 | 620 → 586 → 585 |
| 2 % | 0.906 / 4 968 / +11.7 | 0.906 / 8 280 | 0.896 / 4 692 | 0.906 / 3 772 | 0.901 / 4 968 | 858 → 1 368 → 766 |

Okuma: d2 farkları −0.05…+0.12, **işaret kayıp düzeyine göre dönüyor** (20 %'de dört politika da −0.04…−0.05,
25 %'de +0.03…+0.07) → gürültü (§3.2). Parity ±6 % (M2@170 10 %'de −6 %: em 85–90'da 312–319‰ → r2/r3
yuvarlama sınırı — küçük, gerçek olabilir, teslim etkisi yok). t_L2 değişmedi (em@L2 ≥ 83 ≥ taban; M2 seviyeyi
oynatmaz). **Kabul kriteri "d2 ≥ M0 − 0.03" bu n'de sınanamaz** (taban ±0.05); tutarlı zarar da kanıtlanamaz.

**3.4 E2 (saf reorder, P2) — yanlış L2 ve bedeli:**

| hücre | kol | yanlış L2: M0 → M1@26 → M1@85 → M2@52 → M2@170 | parity B: M0 → M1@26 → M1@85 → M2@52 → M2@170 | wireΔ% M0 → M1@85 |
|---|---|---|---|---|
| C1b | A | 18 → 27 → **0** → 32 → 0 | 736 → 736 → **736** → 736 → 552 | +2.21 → +2.19 |
| C1b | B | 0 → 13 → **0** → 0 → 9 | 460 → 644 → **460** → 552 → 552 | +1.55 → +1.04 |
| C3p | A | 19 → **0** → **0** → 21 → 13 | 184 → 92 → 92 → 276 → 184 | +1.65 → +0.93 |
| C3p | B | 20 → **0** → **0** → 1 → 26 | 276 → 184 → 92 → 184 → 368 | +1.47 → +1.42 |

Okuma: (i) **M1 (her iki eşik) C3p'de yanlış-L2'yi 0'a indiriyor** — öngörü 3 "20 → ~10–14"ten iyi: taban
halka dolana kadarki ilk ~50–100 ms'yi kapatıyor, sonrasını P2 zaten kapatıyor (zamanda tamamlayıcı).
C1b'de yalnız 85 (em@L2 55–69). (ii) **Ama parity kazancı ≤ 92–184 B (≤ %0.4 tel):** A'da yanlış L2'nin
bedeli SIFIR (L1 tavanı = L2 tavanı → 736 B değişmedi; §1.6), B'de P2 altında C1b'de zaten 0, C3p'de 276 B.
wireΔ retx-hâkim (528–2 066 B, ARQ'nun spurious retransmit'i; politika dokunmaz) → kazanç gürültü altında.
(iii) M2 yanlış-L2 sayısını düşürmez (seviye korunur), parity tutarsız (A-M2@52 276 > 184). (iv) d2: C3p
0.98–0.99 her kolda; C1b 0.73–0.98 yayılım (reorder'lı retx zamanlaması; yapısal-özdeş A kolları 0.80–0.98).

**3.5 E3 (kayıp 20 % + reorder 30 %, n=10):**

| kol | stall | n_ok | d2_all | d2_ok | wireΔ% | parity | L2 | t_L2 ms | em@L2 |
|---|---|---|---|---|---|---|---|---|---|
| off | 3 | 7 | 0.281 | **0.323** | — | 0 | 0 | asla | — |
| A-M0 / A-M1@85 / A-M2@170 | 4 / 1 / 2 | 6 / 9 / 8 | 0.214 / 0.276 / 0.266 | 0.286 / 0.281 / 0.281 | +15.3 / +16.8 / +15.7 | 7 912 / 7 912 / 7 636 | 5 332 / 4 432 / 3 364 | 487 / 518 / 536 | 73 / 85 / 92 |
| B-M0 / B-M1@85 / B-M2@170 | 2 / 0 / 4 | 8 / 10 / 6 | 0.219 / 0.229 / 0.198 | 0.250 / 0.229 / 0.260 | +23.6 / +23.2 / +21.7 | 11 086 / 10 626 / 10 396 | 2 857 / 2 732 / 2 757 | 451 / 544 / 412 | 71 / 85 / 64 |

Okuma: kütle modu etkisi yok (A üçlüsü 0.281–0.286; B üçlüsü 0.229–0.260, ±0.03). M1@85 L2'yi +93 ms
geciktiriyor (öngörü +45–100 ✓), teslim değişmiyor. **Asıl bulgu politika-dışı:** karışık hücrede TÜM ON
kolları OFF'un altında (0.23–0.29 vs 0.32; +15…+24 % tel) — persist §5'teki C4 mekanizmaları (parity token
açlığı, gate varyansı) n=10'da da görünüyor; ayrı kart (bu görevin dışı). Stall 0–4/10, politikadan bağımsız.

## 4. Hüküm

**Soru:** açık kanıt kütlesi, anlık cold-start davranışını koruyarak spekülatif L2 kararlarını tutmaya değecek
kadar iyileştiriyor mu? **Cevap: HAYIR — Case B (az değer) + Case D (tepe zararsız).**

1. **Kısa akışta (RTT 200, 96 msg) L2 hiçbir zaman "az kanıtla" açılmıyor:** em@L2 = 83–112 (Q10 85, n=111);
   kuruluş evresi pencereyi ilk kuşkudan ÖNCE dolduruyor (em = 1 → ~70 birim, ilk ACK'te). Küçük-payda
   rejimi yalnız ilk ~200–300 ms'de ve düşük-RTT/reorder hücrelerinde (C3p em 25) var.
2. **Tepeler patolojik değil (Case D):** E1'de tepe akış-sonu platosu, kayıp oranıyla ölçekli (2 % → 18k,
   25 % → 184k, em@pk = 480, seviye 2); eski 195k/290k sınıfı bu rejimde yeniden üretilmedi. C3p'de
   120–190k tepe em 25'te L2 ile çakışıyor ama bedeli ≤ 276 B parity. "Sayıyı düzeltme" gerekçesi YOK.
3. **Kütle-tabanı yanlış-L2 ETİKETİNİ siliyor, bedelini değil:** E2'de M1 → 0 yanlış L2, parity kazancı
   ≤ 92–184 B (≤ %0.4 tel). A'da yapısal olarak bedava (L1 tavanı = L2 tavanı), B'de P2 zaten kontrol ediyor.
4. **Teslim etkisi çözülemedi, tutarlı zarar da yok:** E1'de B farkları −0.05…+0.12, işaret dönüyor; taban
   ±0.05–0.09 (n=5). E3 n=10: ±0.03. Kabul kriteri ("≥ M0 − 0.03") bu n'de ne doğrulanır ne çürütülür → Case C
   kanıtlanmadı; ama Case A için gereken "materyal kazanç" da yok.
5. **Yapısal itiraz:** `sent_window_units` bu sistemde ≈ "kuruluştan beri geçen süre × mesaj hızı × 5 birim"
   — bir ZAMAN vekili. Sabit E_min = birim cinsinden sabit zaman ofseti (sabit-kaydırma sınıfı); faz-1'deki
   E1/E2 ayrımı (80 < 84) faz-2 verisinde kayboldu (E2 max 90 > E1 min 83). Kanal-türevli bir E_min tanımı
   bu veride yok.
6. **Gerçek küçük-payda mekanizması başka yerde:** L2, `persistent ≥ 10k` ORANIYLA açılıyor → pencere ≤ 100
   birimken TEK yaşlanmış gap yeter (prs@L2 = 11–16k ≈ 1–2/90). Bu payda değil PAY sorusudur ("≥ N kalıcı
   birim" gibi mutlak sayım) ve zararlı olduğu da gösterilmedi (5 %'de +20 pp teslim). Kapsam dışı; not düşüldü.

**Karar önerisi:** kanıt kütlesi POLİTİKA GİRDİSİ OLMASIN. Enstrüman (ts kolonları, geçiş-diag'ı) kalsın —
ucuz ve bilgilendirici. `EvidenceMode::{L2Floor, L2Scaled}` kolu default OFF (byte-özdeş); ölü knob
bırakmamak için takip commit'inde KALDIRILMASI önerilir (owner kararı; bu görevde kaldırılmadı).
A/B kararı verilmedi (kütle hiçbirini ayırt etmiyor). P2 etkileşimi: taban ve P2 zamanda tamamlayıcı (C3p) —
ama P2 tek başına bedeli zaten sınırlıyor; persistence kalibrasyonuna dokunulmadı.

**Ayrı kartlar (görev dışı, ölçüldü):** (a) C4 karışık hücrede ON < OFF (n=10) — parity token açlığı/gate
varyansı (persist §5); (b) kuruluş stall'ı %13–40 (kayıp/karışık hücreler), politikadan bağımsız; (c) 2–5 %
kayıpta L2'nin (r2/r3) tel +12…+23 % bedeli — rsweep bulgusu, `l2_min_persistent_ppm` ve pay-sayımı sorusu;
(d) bootstrap-OFF testi ve server-death flake'i (§0).
