# mini-Ö v2 ön kaydı (sahip kararı 2026-11-10 madde 3, 4, 5; v1 = ONKAYIT.md)

Etiket: *gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel; sığ tamponlu
ağ gösterim kapsamı dışı.* Bu belge koşudan ÖNCE donar; okuma (`oku.py --surum v2` ve
`--surum abl`) bu dosyanın sha256'sına bağlıdır (`ONKAYIT_V2_SHA256`). Koşudan sonra metin
değişmez; düzeltme ayrı erratum dosyasıyla. Tahminler §5'te, gerekçesiyle; sonuç tahminden saparsa
sapma dekompozisyonla yazılır, tahmin geri çekilmez. v1 ön kaydı (ONKAYIT.md, sha256 `8a96ca0a…7a8c`)
değişmez; bu belge onun yerine geçmez, v2 koşusunun ön kaydıdır.

## 0. v1'den farklar

| konu | v1 (ONKAYIT.md) | v2 (bu belge) | dayanak |
|---|---|---|---|
| kol | K1 (`--e1-arm K1`) | gösterim seti = K1 + A-1…A-6 + şeritler (gece `K1A6s`); A hücrelerinde f0_flows yalnız `--goster on` (E1/E2a bayrakları kama-node `goster.rs`'ten), b hücrelerinde bench `--e1-arm K1A6` | sahip madde 3 |
| r25 N | 10 | **20** (kol ve taban) | sahip madde 4 |
| R3-d1000 | yok | `b_r12k_d1000_l0_p1000` × {K1A6, K0}, N = 10 (§3.11) | sahip madde 3 |
| T1 sayaçları | "bu koşuda eksik" (v1 §3.7) | **zorunlu**: her A koşusunda iki uçta ts `t1` (§3.13) | sahip madde 4 |
| geçerlilik | — | uygulanan bayrak seti başlıkta ve manifest'te denetlenir (§3.12) | KK-Ö-41 |
| yeşil | v1 §3.8 | v1 §3.8 ∧ R3-d1000 (§4) | sahip madde 3 |
| ablasyon | — | tetik ve karar kuralı (§6), hücreleri hazır (SIRA-V2'de değil) | sahip madde 3, 4 |
| etiket | … N < 30 betimsel | + "sığ tamponlu ağ gösterim kapsamı dışı" | sahip madde 5 |

Değişmeyen: v1 §2 (kaynaklar, katmanlar), §3.1–§3.10 tanımları (§3.4 bağlayıcı dört ölçüt AYNEN),
§3.8'in ilk üç koşulu, §6 sapma/erratum kuralları.

## 1. Kimlik

- Tanım: v1 §0 aynen (EK-A §6; iki ayrı süreç, `f0_flows`, F0 3-netns düzeni).
- Ağaç: koşu anındaki `origin/yol2/goster` ucu. Bu ön kayıt `3beab598` ucuna göre yazıldı:
  `yol2/goster-a6` e332991b merge (5c291d4f) + gösterim profili (54f9c01d) + f0_flows tek bayrak
  kaynağı (4674cb7d) + bench ablasyon kolları (c2668343) + v2/abl hücreleri + okuma. Koşu kaydına:
  ağaç SHA'sı, `3beab598..<SHA>` commit listesi, iki ikilinin sha256'sı, `SIRA-V2.txt` ve bu
  dosyanın sha256'sı.
- Gösterim seti (bayrak bayrak; kama-node `goster.rs` `GOSTER_E1`, `GOSTER_E2A`; bench
  `e1arm.rs` `K1A6` aynı değerler):

| katman | bayrak | değer |
|---|---|---|
| E1 | (c) `e1_startup_cwnd` · (d) `e1_applimited_plateau` · DrainRtprop `e1_drainrt_gate` · (e-dar) `e1_probe_keep` | açık |
| E1 | `retx_first` | kapalı |
| E1 | A-3 `a3_burst_floor` · A-4 `a4_probe_srtt` · A-6 `a6_iw_floor` ("max" okuması, sahip madde 3) | açık |
| E2a | m1 `tt_rfc` (g = 1 ms) · m8 `rtt_rule_v11` · m6 `iw_idle_hold` · `pto_probes = 2` | açık |
| E2a | A-1 `gap_ack` · A-2 `return_prior_gated` · A-5 `pto_rfc` | açık |
| FEC | `runtime.fec = None` (f0_flows) / `--fec-profile 0` (bench) | kapalı |
| şerit / tampon (yalnız A) | W = 5 (`LaneConfig::default`) · T = 2,0 s | açık |

- Başlık etiketleri: `yol3.e1_set` = `c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor`,
  `yol3.e2a_set` = `tt_rfc(g=1ms)+rtt_rule_v11+iw_idle_hold+pto_probes=2+gap_ack+return_prior_gated+pto_rfc`.
- Ürün varsayılanları (`KamaV1Profile`, `RuntimeConfig::default`) değişmez; bayraklar yalnız
  gösterim profilinde ve bench kolunda açıktır.

## 2. Hücreler, koşu, süre

Üretici `scripts/yol3/mini-o/gen_cells.py --set v2`, dosyalar `scripts/yol3/mini-o/cells-v2/`, sıra
`scripts/yol3/mini-o/SIRA-V2.txt`. 7 dosya, 90 iş.

| sıra | hücre dosyası | kaynak (geometri aynen) | kol | N | okuma |
|---|---|---|---|---|---|
| 1 | `a_r12k_d150_l20_foto150k__K1A6s` | `scripts/yol3/cells/a/` (Y10) | gösterim seti (f0_flows `--goster on`) | 10 | bağlayıcı dört ölçüt; T, W |
| 2 | `b_r25_d300_l20_p65__K1A6` | `scripts/ktp2/f0/cells/b/` | bench `--e1-arm K1A6` | 20 | R3-r25 |
| 3 | `b_r25_d300_l20_p65__K0` | aynı | K0 | 20 | R3-r25 tabanı |
| 4 | `b_r12k_d1000_l0_p1000__K1A6` | `scripts/ktp2/f0/cells/b/` | bench `--e1-arm K1A6` | 10 | R3-d1000 |
| 5 | `b_r12k_d1000_l0_p1000__K0` | aynı | K0 | 10 | R3-d1000 tabanı |
| 6 | `a_r12k_d300_l30_dosyali__K1A6s` | `scripts/ktp2/f0/cells/a/` | gösterim seti | 10 | G-K8 kötü ağ (raporlanan); W |
| 7 | `a_r12k_d300_l0_dosyali__K1A6s` | `scripts/yol3/cells/a/` | gösterim seti | 10 | G-K8 iyi ağ (raporlanan); W |

- A hücrelerinde her uçta yalnız `--goster on` (`--e1-arm`, `--fec-off`, `--playout-ms`, `--lane-w`
  YOK): E1, E2a, FEC, W ve T `goster.rs`'ten. Zaman çizelgesi v1 §1 ile aynı (F0 COMMON).
- b hücrelerinde (bench `kama-product`): `--fec-profile 0 --e1-arm <kol>`, iki uçta aynı. **bench'te
  şerit ve oynatma tamponu yoktur** (beyanlı): R3 satırları motor bayraklarını (E1 + E2a) ölçer.
- Taban K0 aynı ikili (`BIN_GOSTER_BENCH`), aynı kök (aynı `f0_grid` koşusu), aynı FEC durumu.
- Koşu (orkestratör; D-18): `python3 -I scripts/yol3/mini-o/gen_cells.py --set v2 --komut`.
  `f0_grid.sh -j 11`, yedi dosya paralel. İkililer `yol2/goster` release (orkestratör derler).
- Süre tahmini [türetilmiş]: r25 koşusu 2–2,5 dk (20 koşu seri ≈ 40–50 dk), d300 l30 2,5–6 dk
  (10 koşu ≈ 35–60 dk), Y10 ≈ 2 dk, d1000 ≈ 1,5–3 dk → duvar süresi ≈ 45–60 dk.
- Sessiz yeniden koşu yok; düşen koşu düşük kalır (rc ≠ 0 koşu başarısız sayılır, N'de kalır).
- Ön koşu (orkestratör, ölçümden önce): bir A hücresinde iki ucun `hdr` satırında
  `yol3` = `{"e1_arm":"K1A6","e1_edar":true,"e1_set":"<§1>","fec_off":true,"e2a_set":"<§1>",
  "goster":true,"lanes_w":5,"playout_ms":2000,"flag_src":"goster.rs"}`; iki ucun `ts` satırlarında
  `t1` nesnesi (null değil); alice `ts` satırında bob oturumunun (`s[]`, `peer` = bob) `a6` nesnesi
  (A-6 deney kaydı: `arm_ms`, `arms`, `r0`, `rel_ms`, `why`, `up`, …); bir b hücresinde istemci
  `manifest.json` `profile_delta`'da `e1_flags …a6_iw_floor: true…` ve `e2a_flags …pto_rfc: true…`
  satırları (K0'da ikisi de yok).

## 3. Tanımlar (bağlayıcı)

**3.1–3.10** v1 ONKAYIT §3.1–§3.10 AYNEN; "gösterim kolu" A hücrelerinde `K1A6s`, r25 ve d1000
hücrelerinde `K1A6` okunur. Ek notlar (tanım değişikliği değil):
- §3.4 (koşu başarılı, yalnız Y10) ve §3.2 (oynatma kesintisiz: tampon açık → doğrudan duraklama
  = 0 ∧ basışın bütün demetleri oynatıldı, 40 ms tolerans) bağlayıcı kalır; T1 `playout_stalls`
  sayacı raporlanandır (KK-Ö-40).
- §3.6 R3-r25: kol medyanı teslim@60 ≥ taban medyanı − 1 ∧ kol medyanı gönderim ≤ taban × 1,10;
  N = 20, medyan `statistics.median` (çift N'de iki orta değerin ortalaması; v1 aracıyla aynı).
  Veri eksikse "veri yok" (geçmiş sayılmaz). Gönderim = istemci tel baytı (`tx_total_B`, analiz_b).
- §3.7'nin T1 satırı yerine §3.13.

**3.11 R3-d1000 l0** (CC §4.2 R3, aynen): kol (`K1A6`) teyit p95 medyanı ≤ taban (`K0`) teyit p95
medyanı × 1,10. Teyit p95 = `scripts/ktp2/f0/b/analiz_b.py` `analyze_run` `conf_p95_s` (koşu başına),
medyan koşular üstünden. Veri eksikse "veri yok" (geçmiş sayılmaz).

**3.12 Geçerlilik — uygulanan bayrak seti** (KK-Ö-41): A hücrelerinin her koşusunda alice ve bob
`hdr.yol3` §2 son maddesindeki değerlerin AYNISI; b hücrelerinin her koşusunda istemci
`manifest.json` `profile_delta`'nın `e1_flags` / `e2a_flags` satırları kolun beklenen setiyle alan alan
aynı (K0: iki satır da yok; manifest yoksa "doğrulanamadı", uyuşmazlık sayılmaz). Uyuşmayan koşu
okumada listelenir; uyuşmayan koşu varsa yeşil satırı "GEÇERSİZ KOŞU" etiketiyle yazılır.

**3.13 T1 sayaçları** (zorunlu; madde 5'in ikinci yarısı): her A koşusunda iki ucun `ts` satırlarında
`t1` nesnesi bulunur. Raporlanan (eşik yok): tampon tepe (`bob.po.peak`, T1'in kendi en yüksek
doluluk sayacı), T1 duraklama Σ ve ms (`bob.po.st`, `po.stms`), atılan / geç elenen / taşma
(`po.disc`, `po.late`, `po.ov`), şerit ret Σ (`alice.ln.ref[*]`), şerit düşen / süresi dolan
(`ln.drp`, `ln.exp`), şerit kuyruğu ve dosya penceresi tepe (`alice.bl[0]`, `bl[1]`; ≈ 200 ms
örnekleme, alt sınır). Eksik koşu "T1 EKSİK" listesine girer ve yeşil satırı "GEÇERSİZ KOŞU"
etiketiyle yazılır.

## 4. "Yeşil" (v2)

**Yeşil ⟺ Y10'da ≥ 8/10 başarılı koşu (§3.4) ∧ gösterim setinin bütün A hücrelerinde ACK'li birim
kaybı = 0 (§3.5) ∧ R3-r25 geçer (§3.6) ∧ R3-d1000 geçer (§3.11).** "veri yok" geçmez. Okuma ayrıca
geçerliliği (§3.12, §3.13) yazar; geçersiz koşuda yeşil satırı hesaplanır ama "GEÇERSİZ KOŞU"
etiketlidir. Dondurma şartı olarak kullanımı v1 §3.8 ile aynı (sahip satırı).

## 5. Beklenti kaydı (tahmin ÖNCE; gerekçe)

Öncüller: (G) gece ölçümü 2026-11-10, `K1A6s` kolu (`yol2/goster-a6` e332991b; `--e1-arm K1A6 --goster
on`; bayraklar bu setle alan alan aynı) — aynı hücre geometrisi, bu ön kaydın tanımlarıyla
(`oku.py --surum v2`) yeniden okundu; o ikilide T1 oynatma tamponu v1'di (erken verme vardı) ve
`t1` yazılmıyordu. (V1) mini-Ö v1, K1 + şeritler + tampon v2 (`yol2/goster` 1ee9ee68).

| ölçüt | tahmin | gerekçe |
|---|---|---|
| Y10 ilk demet varışı | medyan 0,8–1,2 s; en kötü 1,5–3,5 s; ≤ 3 s 9–10/10 | G 0,98 [3,37], 9/10; V1 1,10 [1,99] |
| Y10 bütün demetler varış p95 | medyan 1,2–1,8 s | G 1,43 [2,50] |
| Y10 ilk duyulabilir ses | medyan 2,9–3,3 s (ilk demet + T + ≤ 20 ms) | T = 2,0 s; tampon v2 erken vermez; G 3,00 [5,38], V1 3,12 [4,00] |
| Y10 D p95 → T önerisi | D p95 1,3–2,5 s → T 1,6–2,5 s (nokta 2,0 s) | G 1,67 → 1,97 s; V1 2,77 → 2,50 s |
| Y10 doğrudan duraklama | 15 s basışında duraklamasız 9–10/10; biriken gecikme en çok ≤ 2,2 s | G 0 duraklama; V1 1 duraklama (761 ms) |
| Y10 TEXT dosya sürerken | en büyük medyan 1,5–4,5 s, en kötü ≤ 6 s; ≤ 10 s 10/10 | G 1,97 [2,60]; V1 4,04 [5,50] |
| Y10 dosya | medyan 20–25 s, en kötü 25–30 s; ≤ 30 s 9–10/10 | G 22,0 [27,5], 10/10 |
| Y10 başarılı koşu | 8–10/10 (nokta 9) | G 9/10 (kalan koşu: ilk demet 3,37 s) |
| ACK'li birim kaybı → W | 0 → W = 5; belirsiz koşu 0–3 (yalnız d300 l30) | G Y10 + l30 0, belirsiz 0; V1 0 (l30'da 6 belirsiz) |
| olay/demet kaybı | 0 (dinleyici düşüşü, O/C/P eksik, yinelenen, ayrışamayan, sırasız) | G, V1 |
| kuyruk en yüksek (Y10) | alice `out` 15–30; bob Reorder tutulan 25–40 | G 22 / 31; V1 14 / 36 |
| T1 (Y10) | tampon tepe 10–14 demet; T1 duraklama Σ 0–10 (10 koşu, bütün akışlar); atılan / geç / taşma 0; şerit düşen / süresi dolan 0; dosya penceresi tepe 5; şerit kuyruğu tepe 1–10; şerit ret Σ 0–500 (öncül yok, ilk kayıt) | T / 200 ms = 10 demet; W = 5 |
| T1 varlığı (§3.13) | 30 A koşusunun hepsinde iki uçta var | f0_flows ts testi `ts_line_writes_t1_counters_additively` |
| geçerlilik (§3.12) | başlık 30/30 (iki uç), manifest 60/60 | f0_flows testi `goster_on_writes_exactly_the_kama_node_goster_profile`; bench `ablation_arms_drop_exactly_one_a_flag` |
| R3-r25 teslim@60 | K1A6 6, K0 6 | G 6 / 6; V1 6 / 6 |
| R3-r25 gönderim | K1A6 2 400–2 700 B, K0 2 000–2 650 B; oran 0,90–1,25; nokta tahmin oran 1,02 → **geçer (zayıf, olasılık ≈ yarı)** | G 2 546 / 2 610 B (0,975, geçti); V1 K1 2 786 / K0 1 996 B (1,40, kaldı); K0'ın iki koşusu çelişkili |
| r25 bütün mesajlar 60 s içinde (betimsel) | K1A6 13–17/20, K0 16–19/20 | G 7/10, 9/10 |
| R3-d1000 | K1A6 teyit p95 medyanı 3,0–4,5 s; K0 4,8–5,8 s; oran 0,55–0,85 → **geçer** | G 3,44 / 5,45 s (0,63) |
| yeşil (v2) | nokta tahmin **yeşil** (zayıf: tek belirsiz kalem R3-r25) | yukarıdakiler |
| ablasyon tetiği | r25 ≈ yarı olasılıkla; d1000 tetiklenmez | R3 satırları |
| ablasyon (r25 tetiklenirse) | K1A6 aynı kökte de kalır ≈ yarı olasılıkla; düşen bayrak en olası A-5 `pto_rfc`, sonra A-1 `gap_ack`; A-6 değil | gönderim istemci baytıdır (veri + retx + dup + ctrl); `pto_rfc` PTO tur tavanını kaldırır, r25'te (RTT ≈ 0,6–0,7 s, uygulamayla sınırlı akış) kuyruk probu sık; G'de K1A ↔ K1A6 r25 2 616 ↔ 2 546 B (A-6 etkisiz) |
| d300 l30 ilk demet varışı | medyan 1,2–2,0 s; p95 2,5–3,5 s (B1 ilk demet ≤ 3 s sınırda) | G 1,57 [3,06], p95 2,76 |
| d300 l30 ilk duyulabilir ses | p95 4,0–5,5 s ("≤ 3 s + T" etiketi) | G 4,78 |
| d300 l30 TEXT | havuz p95 3–6 s (B2 ≤ 10 s geçer); 60 s'de %100 9–10/10 | G 3,95 s, 10/10 |
| d300 l30 dosya (524 700 B) | medyan 130–180 s; teslim 9–10/10 | G 147,4 [176,4] s, 10/10 |
| d300 l30 duraklama | D p95 3–5 s; doğrudan duraklama 5–10 aralık (10 koşu) | G 4,17 s; 7 aralık / 6 027 ms |
| d300 l0 ilk demet / ilk ses | ilk demet medyan 0,5–0,9 s (B1 iyi ≤ 1,5 s ilk demette geçer); ilk ses 2,5–2,9 s ("≤ 1,5 s + T") | V1 0,62 [0,82] / 2,64 s; kayıp yok |
| d300 l0 TEXT | havuz p95 0,5–1,5 s (B2 iyi ≤ 2 s geçer) | V1 1,04 s |
| d300 l0 dosya | medyan 85–100 s | pencereyle sınırlı (BDP 7 500 B > W · S = 5 065 B); A bayrakları pencereyi değiştirmez; V1 93,9 s |
| callback vekili | TEXT iz R → dinleyici p95 ≤ 0,2 ms; PTT kalıntısı p95 15–20 ms | G 0,1 / 17,5 ms; V1 0,1 / 19,8 ms |

## 6. Ablasyon (sahip madde 3 ve 4)

**6.1 Tetik.** v2 okuması (`oku.py --surum v2`, bu ön kayda bağlı) R3-d1000 için "kalır" ⇒
`SIRA-ABL-d1000.txt`; R3-r25 için "kalır" ⇒ `SIRA-ABL-r25.txt`. "veri yok" tetik değildir (koşu kaydı
incelenir; sessiz yeniden koşu yok). Madde 4 "iki koşu yine tutmazsa" = mini-Ö v1 (R3-r25 kaldı) ve
bu v2 koşusu; v2'de de kalırsa ablasyon (KK-Ö-43).

**6.2 Koşu.** Tetiklenen her hücre kendi kökünde, v2 ikilileriyle; N = tetikleyen v2 hücresininki
(r25 20, d1000 10). Üretici `gen_cells.py --set abl` (`cells-abl/`), komut `--set abl --komut`. Kollar
(bench `--e1-arm`, iki uçta aynı, `--fec-profile 0`):

| kol | bench kolu | gösterim setinden fark |
|---|---|---|
| K0 | K0 | taban (bayraklar kapalı) |
| K1A6 | K1A6 | yok — aynı kökte yeniden üretim kontrolü (KK-Ö-42) |
| K1A6mA1 | K1A6mA1 | − A-1 `gap_ack` |
| K1A6mA2 | K1A6mA2 | − A-2 `return_prior_gated` |
| K1A6mA3 | K1A6mA3 | − A-3 `a3_burst_floor` |
| K1A6mA4 | K1A6mA4 | − A-4 `a4_probe_srtt` |
| K1A6mA5 | K1A6mA5 | − A-5 `pto_rfc` (`pto_probes = 2` kalır; KK-Ö-44) |
| K1A6mA6 | K1A | − A-6 `a6_iw_floor` (KK-Ö-45) |

**6.3 Karar** (`oku.py --surum abl --hucre r25|d1000`). R3(X) = X'in aynı kökteki K0'a göre R3'ü
(§3.6 / §3.11 aynen).
- (a) K0 ya da K1A6 verisi yoksa karar yok ("veri yok").
- (b) R3(K1A6) geçer ⇒ ihlal bu kökte yeniden üretilmedi ⇒ **hiçbir bayrak düşmez**; set değişmez;
  okuma "R3 ölçüsü kararsız" satırını sahibe verir.
- (c) R3(K1A6) kalır: aday kümesi C = {A-i : R3(K1A6mAi) geçer}.
  - |C| = 0 ⇒ tek bayrak açıklamıyor ⇒ bu kuralla bayrak düşmez; sahibe (bayraklar arası etkileşim
    olası). Varsayılan: set değişmez, "R3 kalır" satırı gösterim kaydına girer.
  - |C| = 1 ⇒ **o bayrak düşer**.
  - |C| ≥ 2 (beraberlik) ⇒ marjı en iyi olan **tek** bayrak düşer. Marj: d1000'de kol / K0 teyit p95
    medyanı oranı (küçük iyi); r25'te gönderim medyanı oranı (küçük iyi), eşitse teslim@60 farkı
    (büyük iyi). Oranlar 3 ondalığa yuvarlanır. Yine eşitse numarası büyük A bayrağı düşer
    (A-6 > A-5 > A-4 > A-3 > A-2 > A-1; deterministik).
- (d) İki hücre de tetiklenir ve farklı bayrak seçerse ikisi de düşer; aynı bayraksa bir kez.
- (e) Düşen bayrak `goster.rs`'te kapanır (sınıf M; Paz 15 20:00 öncesi) ve bench/f0_flows kol
  tablosunda yeni set ayrı kol adıyla tanımlanır. Yeni setle Y10 ve tetikleyen R3 hücresi yeniden
  koşulur; o koşu bu ön kaydın kapsamı dışındadır (ayrı ön kayıt ya da erratum).
- N < 30: karar bayrak seçimi kuralıdır; "bu bayrak R3'ü bozuyor" nedensellik iddiası değildir.

## 7. KARAR-KAYDI (bu ön kayda ait; biçim KK · karar · dayanak · geri alma)

- **KK-Ö-40 · §3.2 bağlayıcı kalır; T1 `playout_stalls` raporlanan.** v1 KK-Ö-35'in geri alma satırı
  ("T1 sayacı kayda girerse o sayaç bağlayıcı olur") v2'de uygulanmaz: sayaç düğüm başına
  birikimlidir ve bütün akışları (15 s ve 60 s basışı, karşı örnek basışı) sayar
  (`kama-app/src/playout.rs` `PlayoutStats`); 15 s basışına ayrılamaz ve 40 ms toleranslı tanımla
  aynı nicelik değildir. Orkestratör satırı: "bağlayıcı ölçütler v1 §3.4 aynen". · dayanak: iş emri
  ENT-DEMO madde 6; playout.rs · geri alma: erratum; T1'e basış başına sayaç eklenirse o sayaç.
- **KK-Ö-41 · Bayrak kaynağı tek; geçerlilik denetimi.** A hücrelerinde `--goster on` ∧ `--e1-arm`
  yok ⇒ f0_flows E1/E2a bayraklarını `kama_node::goster`'dan yazar, başlık `flag_src = goster.rs`;
  `--e1-arm` verilirse kol tablosu (`flag_src = e1-arm`). Başlığın `e1_set` / `e2a_set` alanları
  düğüme uygulanan bayraklardan yazılır. Gece `K1A6s` (`--e1-arm K1A6 --goster on`) ile bu set alan
  alan aynıdır (f0_flows testi). · dayanak: sahip madde 3; iş emri madde 3 · geri alma: —.
- **KK-Ö-42 · Ablasyon kökünde K1A6.** İş emrindeki liste (D−A1…D−A6 + K0) aynı kökte gösterim
  setiyle genişletildi: bütün çıkarma kolları R3'ü geçerse ihlalin o kökte yeniden üretilip
  üretilmediği ancak böyle ayrılır (§6.3 b). · dayanak: ölçüm disiplini (aynı kök, aynı ikili) ·
  geri alma: SIRA-ABL'lerden K1A6 satırı çıkar, §6.3 (b) düşer, (c) K1A6'nın v2 sonucuyla uygulanır.
- **KK-Ö-43 · Madde 4 okuması.** "İki koşu yine tutmazsa" = v1 (K1, kaldı) + v2 (gösterim seti). Diğer
  okuma (v2'de iki ayrı N = 20 koşusu) orkestratör/sahip satırıdır; o okumada §6.1 ikinci koşudan
  sonra uygulanır. · dayanak: sahip madde 4 · geri alma: erratum.
- **KK-Ö-44 · A-5 ablasyonu yalnız `pto_rfc`.** `adaptive-transport` loss.rs belgesi "A-5 = `pto_rfc`
  + `pto_probes = 2`" der; `pto_probes = 2` M1 E2a setindedir (v4 III.1 "PTO prob sayısı 2") ve
  gösterim setine A paketinden önce girmiştir ⇒ A-5 çıkınca PTO 2 kalır. · dayanak: v4 III.1; gece
  K1A6 tanımı · geri alma: bench'e `pto_rfc` + `pto_probes = 1` kolu, hücre yeniden üretimi.
- **KK-Ö-45 · K1A6 − A-6 = K1A.** Yeni bench kolu açılmadı; hücre kol adı `K1A6mA6`, bench
  `--e1-arm K1A` (bench testi: K1A = K1A6 − `a6_iw_floor`). · dayanak: bench `e1arm.rs` · geri alma: —.

## 8. Sapma ve erratum

v1 §6 aynen: koşu öncesi değişiklikte bu dosya değişir ve sha256 yeniden yazılır (`oku.py`
`ONKAYIT_V2_SHA256`); koşu sonrası bu dosya değişmez, `ONKAYIT-V2-ERRATUM-<n>.md`; sessiz yeniden
koşu yok; ölçüm aracı hatasında önce araç düzeltilir ve eski koşu yeniden okunur (kural değişmez).
