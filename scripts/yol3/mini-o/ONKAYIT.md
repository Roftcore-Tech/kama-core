# mini-Ö ön kaydı (KK-Ö-11; sahip v4 EK-A §6 = KK-Ö-EKA-1)

Etiket: *gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel.* Bu belge
koşudan ÖNCE donar; okuma (`oku.py`) bu dosyanın sha256'sına bağlıdır (`ONKAYIT_SHA256`). Koşudan
sonra metin değişmez; düzeltme ayrı erratum dosyasıyla. Tahminler §4'te, gerekçesiyle; sonuç
tahminden saparsa sapma dekompozisyonla yazılır, tahmin geri çekilmez.

## 0. Kimlik

- Tanım (EK-A §6, aynen): mini-Ö "tek entegrasyon SHA'sında (`yol2/goster`), iki ayrı süreçte koşar:
  dosya (150 KB) sürerken TEXT + PTT. Ölçülen ve raporlanan: 1. basıştan ilk demet varışına süre
  (bağlayıcı ≤ 3 s) ve ilk duyulabilir sese süre (raporlanan); 2. oynatma duraklamaları ve biriken
  gecikme; 3. dosya sürerken TEXT teslim süresi (≤ 10 s); 4. dosya bitişi (≤ 30 s); 5. olay/demet
  kaybı ve kuyrukların en yüksek doluluğu (T1 sayaçları)." Aynı süreçte iki düğüm bileşen testidir,
  kabul değildir; bu koşu iki ayrı süreçtir (`f0_flows`, F0 3-netns düzeni).
- Ağaç: koşu anındaki `origin/yol2/goster` ucu. Bu ön kayıt `9e157992` ucuna göre yazıldı (o1-e2a +
  T1 şeritleri + düzenek; `--goster on` = S1 şeritleri `LaneConfig::default()` W = 5 + PTT oynatma
  tamponu). Koşu kaydına: ağaç SHA'sı, `9e157992..<SHA>` farkının commit listesi, iki ikilinin
  sha256'sı, `SIRA.txt` ve bu dosyanın sha256'sı.
- Gösterim kolu: `K1` (sahip v4 EK-A §2 "Gösterim seti = K1": (c) + (d) + DrainRtprop + (e-dar);
  retx_first kapalı), iki uçta `--e1-arm K1`. Kol parametredir (`gen_cells.py --kol`); A paketi
  ağaca girer ve orkestratör başka kol seçerse hücreler yeniden üretilir, kol adı ve SIRA sha256'sı
  kayda girer, bu ön kaydın tanımları (§3) aynen geçerlidir, §4 tahminleri yalnız K1 içindir.
- FEC: kapalı (v4 Y1 "Gösterim düğümünde `runtime.fec = None`"; f0_flows `--fec-off on`, bench
  `--fec-profile 0`). E2a bayrakları K1 kolunda kapalı (`pto_probes = 1`; v4 III.1 "PTO prob sayısı
  2" bu kolda YOK — bkz. §5 KK-Ö-38).

## 1. Hücreler, koşu, süre

Üretici `scripts/yol3/mini-o/gen_cells.py` (varsayılanlar: `--kol K1 --taban K0 --playout-ms 2000
--lane-w 5`), dosyalar `scripts/yol3/mini-o/cells/`, sıra `scripts/yol3/mini-o/SIRA.txt`. N = 10.

| sıra | hücre dosyası | kaynak (geometri aynen) | kol | okuma |
|---|---|---|---|---|
| 1 | `a_r12k_d150_l20_foto150k__K1` | `scripts/yol3/cells/a/` (Y10: iki yön 150 ms, %20, 100 kbit, 150 480 B, 80 B demet, jitter yok) | K1 | bağlayıcı dört ölçüt; T, W |
| 2 | `b_r25_d300_l20_p65__K1` | `scripts/ktp2/f0/cells/b/` | K1 | R3-r25 (K-3) |
| 3 | `b_r25_d300_l20_p65__K0` | aynı | K0 | R3-r25 tabanı (aynı ikili, aynı FEC durumu) |
| 4 | `a_r12k_d300_l30_dosyali__K1` | `scripts/ktp2/f0/cells/a/` (524 700 B, 40 B demet) | K1 | G-K8 kötü ağ (raporlanan); W |
| 5 | `a_r12k_d300_l0_dosyali__K1` | `scripts/yol3/cells/a/` (524 700 B, 40 B demet) | K1 | G-K8 iyi ağ (raporlanan); W |

- A hücrelerinde her uçta: `--e1-arm K1 --fec-off on --goster on --playout-ms 2000`. Zaman çizelgesi
  F0 COMMON: ısınma 3 TEXT (0/0,5/1,0 s; sınıf `W`, sayılmaz), dosya t0 + 5 s, 40 TEXT t0 + 6 s'den
  2,5 s arayla (sınıf `T`), 10 sinyal, basışlar t0 + 15 s ve t0 + 60 s (25 demet × 200 ms), karşı
  örnek basışı (akış 90) ve 5 karşı TEXT (sınıf `K`) motorun yeni veri kuyruğu boş ∧ teyitsiz ≥ 3
  iken bir kez.
- B hücrelerinde (bench `kama-product`): `--fec-profile 0 --e1-arm <kol>`. **bench'te şerit ve
  oynatma tamponu yoktur** (beyanlı): r25 satırı motor bayraklarını (K1 ↔ K0) ölçer.
- Koşu (orkestratör; D-18; ajan ve cargo durur, KK-Ö-EKA-5): `python3 -I
  scripts/yol3/mini-o/gen_cells.py --komut` çıktısı. `f0_grid.sh -j 11`, beş dosya paralel.
- Süre tahmini [türetilmiş]: Y10 koşusu ≈ 115 s (2026-11-09 şeritli koşu), r25 ≈ 120–150 s (G2),
  d300 l30 ≈ 2,5–5 dk (F0 şeritsiz 105–233 s; şeritli dosya pencereyle sınırlı → daha uzun), d300 l0
  ≈ 1,5–2,5 dk. Dosya başına 10 koşu seri; en uzun dosya (d300 l30) ≈ 35–60 dk = toplam duvar süresi.
- Sessiz yeniden koşu yok; düşen koşu düşük kalır (rc ≠ 0 koşu başarısız sayılır, N'de kalır).

## 2. Kaynak ve katmanlar

Okuma `scripts/yol3/mini-o/oku.py` (yalnız stdlib; `python3 -I`). Öğe anları F0 analizöründen
(`scripts/ktp2/f0/a/f0a_analyze.py` `analyze_run`, değişmeden):
- `t_send` alice uygulama çağrısı; `t_open` = basışın `ptt_open` çağrısı (alice `send` `c = "O"`).
- **varış** `r` = `t_released`: bob iz "R" (Transport'un birimi Application'a verdiği an; **oynatma
  tamponu ÖNCESİ**). Birim eşlenemezse `t_rx` üst sınır olarak kullanılır ve "eşlenmemiş" sayılır.
- **oynatma** `p` = `t_rx`: bob `PttBundle` dinleyici olayı (tampon açıkken oynatma anı).
- Teslim edilmeyen öğe ∞ (sansür). Yüzdelik nearest-rank (`f0a_tam.rank` ile aynı).
- B hücresi: `scripts/ktp2/f0/b/analiz_b.py` `analyze_run`; teslim@60 = `deliv_D × n_msgs`,
  gönderim = `tx_total_B`.
- Etiket: değerler [ölçüldü]; T önerisi, kıyaslar ve G-K8 eşik karşılaştırması [türetilmiş].

## 3. Tanımlar (bağlayıcı)

**3.1 Basış ölçüleri** (basış başına; bağlayıcı basış = t0 + 15 s basışı, akış 0; t0 + 60 s basışı
ve karşı örnek basışı raporlanır):
- ilk demet varışı = r₀ − t_open (demet 0; ölçüt ≤ 3 s, EK-A §6 madde 1).
- bütün demetler varış p95 = p95ᵢ(rᵢ − t_send,ᵢ) (EK-A §10.4; raporlanan).
- ilk duyulabilir ses = p₀ − t_open (raporlanan; dinleyicinin duyduğu gecikme "≤ 3 s + T" etiketiyle,
  EK-A §3.4).
- durma D = maxᵢ(rᵢ − r₀ − i · 200 ms) (KK-Ö-11, aynen; varış anlarından, tampondan bağımsız).
  Teslim edilmeyen demet varsa D = ∞.
- doğrudan duraklama = ardışık oynatma aralığı pᵢ − pᵢ₋₁ > 200 ms + 40 ms olan aralık sayısı; süresi
  Σ(pᵢ − pᵢ₋₁ − 200 ms). 40 ms = 2 × düğüm turu (kama-node `NODE_TICK` = 20 ms): olay anları tur
  kuantalıdır, daha küçük durma bu düzenekte ayırt edilmez (KK-Ö-35).
- biriken gecikme: Lᵢ = pᵢ − t_send,ᵢ; son = L_son − L₀, en çok = maxᵢ Lᵢ − L₀ (EK-A §6 madde 2).
- sırasız oynatma = pᵢ < pᵢ₋₁ olan aralık sayısı.

**3.2 Oynatma kesintisiz** (15 s basışı): tampon açık koşuda (koşu başlığı `yol3.playout_ms` dolu)
**doğrudan duraklama = 0 ∧ basışın bütün demetleri oynatıldı**; tampon kapalı koşuda analitik
D ≤ 2,0 s (KK-Ö-11 analitik tanımı; yalnız eski veri). Analitik D ≤ T ayrıca raporlanır.

**3.3 T önerisi** (Paz yeniden koşusu için): Y10 × gösterim kolu, 15 s basışlarının D değerleri
(n ≤ 10, ∞ dahil); T = p95(D) + 0,3 s, [1,5; 2,5] s'ye kırpılır; D yoksa 2,0 s (KK-Ö-11, v4 III.1).
n = 10'da p95 = en büyük değer. İki basış birlikte (15 s + 60 s) aynı formülle raporlanır.

**3.4 Koşu başarılı** (yalnız Y10 hücresi) ⟺ rc = 0 ∧ ilk demet varışı ≤ 3,0 s ∧ TEXT ≤ 10,0 s ∧
dosya ≤ 30,0 s ∧ oynatma kesintisiz (§3.2), hepsi 15 s basışı ve aynı koşu için.
- TEXT = alice `send_file` çağrısı ile bob dosya tamam anı (alınmadıysa alice koşu sonu) arasında
  gönderilen `T` sınıfı TEXT'lerin send → bob rx en büyüğü (∞ = alınmadı); p95 ayrıca. Pencerede hiç
  TEXT yoksa ölçüt **sağlanmamış** sayılır (boş geçiş yok). `W` (ısınma) sayılmaz; `K` (karşı
  örnek) raporlanır, ölçüte girmez (KK-Ö-34).
- dosya = alice `send_file` çağrısı → bob dosya tamam (`rx` `c = "F"`); alınmadıysa ∞.

**3.5 W teyidi** (gösterim kolunun bütün A hücreleri, bütün koşular): ACK'li birim kaybı =
bob `delivery_channel_drops` (son ts `dcd`) + Reorder atlaması (`units_skipped_below_next`) +
alice teyitli ∧ bob teslimsiz [süzülmeyen birim (`units_never_drained`), yalnız alice'in son ts
`out` = 0 ise; değilse "belirsiz" diye listelenir, sayılmaz (KK-Ö-36)] + uygulama düzeyi: alice
alındısı var ∧ bob `rx` yok TEXT + alice `file_delivered` ∧ bob dosya alınmadı/eşleşmedi. Toplam
0 ⇒ W = 5 kalır; > 0 ⇒ Paz koşusunda W = 4 (T1 KK-T1-09).

**3.6 K-3 (R3-r25)**, CC §4.2 R3 aynen: kol medyanı teslim@60 ≥ taban medyanı − 1 ∧ kol medyanı
gönderim ≤ taban × 1,10. Veri eksikse "veri yok" (geçmiş sayılmaz).

**3.7 Madde 5 — olay/demet kaybı ve kuyruk doluluğu** (raporlanan; eşik yok):
- olay/demet kaybı: dinleyici kuyruğu düşüşü (`listener_drops`, iki uç), iz düşüşlü koşu, alice
  gönderdiği ↔ bob aldığı `O` (PttStarted) / `C` (PttEnded) / `P` (demet) farkı, yinelenen, ayrışamayan.
- kuyrukların en yüksek doluluğu: ts satırlarından (≈ 200 ms örnekleme) alice teyitsiz birim (`out`),
  motor kuyrukları (`rec.txq`, `rec.un`, `rec.inf`, `rec.pl`) ve bob Reorder tutulan (`reo.h`).
  Örnekleme tepeyi kaçırabilir (alt sınır).
- **T1 sayaçları (envanter, `9e157992`):** T1 kodunda VAR ama bu düzenekte KAYITLI DEĞİL —
  `client-core/src/lanes.rs` `LaneStats` {accepted, given, refused, lane_expired, lane_dropped,
  give_failed, file_acked, acks_drained} ve `ClientTransport::lane_backlog` (anlık şerit + dosya
  penceresi doluluğu); `kama-app/src/playout.rs` `PlayoutStats` {playout_stalls, playout_stall_ms,
  playout_overflow, bundles_released}; `ptt.rs` `PttStats`. kama-node ölçüm sondası (`NodeProbe`)
  bunları taşımıyor, f0_flows yazmıyor. T1 kodunda YOK: şerit kuyruklarının ve oynatma tamponunun
  **en yüksek doluluk** sayacı (yalnız anlık değer var). Okuma ts satırında `t1` alanı görürse
  okur; görmezse sütun "eksik" yazar. Bu koşuda beklenen: **eksik** (KK-Ö-37).

**3.8 "Yeşil"** (KK-Ö-11, aynen; dondurma şartı Paz yeniden koşusunda): Y10'da ≥ 8/10 başarılı koşu
∧ gösterim kolunun bütün A hücrelerinde ACK'li birim kaybı = 0 ∧ R3-r25 geçer. G4 koşusunda aynı
satır hesaplanır, "G4 durumu" diye raporlanır.

**3.9 G-K8** (d300 hücreleri; raporlanan, KAPI-MANİFESTİ §5.9 eşikleri; N = 10 → UCB yok):
B1 basış → ilk ses p95 (iki basış birlikte) ≤ 3 s (l30) / ≤ 1,5 s (l0) — ilk demet varışı p95 de
yazılır; tampon T = 2,0 s ilk sese eklenir, bu yüzden "≤ eşik + T" etiketi. B2 her koşuda `T`
TEXT'lerinin %100'ü 60 s içinde (koşu sayısı) ∧ havuzlanmış `T` TEXT p95 ≤ 10 s (l30) / ≤ 2 s (l0).
B3 150 KB ölçütü yalnız Y10'da (d300 dosyası 524 700 B; bitiş raporlanır). B4 (60 s kesinti) bu
hücrelerde yok → ölçülemedi. B5 bozuk (bob `unparsed`) ve sırasız oynatılan demet = 0; D p95.

**3.10 EK-A §3.7 — callback + tur süresi** (raporlanan): düğüm turu süresi aynada yok; vekiller —
TEXT için bob dinleyici anı − iz "R" (aynı tur içi işleme + dinleyici çağrısı), PTT için
p₀ − (r₀ + T) (tampon dışı kalıntı: tur kuantası + çağrı). Kotlin tüketimi bu düzenekte yok →
**ölçülemedi**.

## 4. Beklenti kaydı (tahmin ÖNCE; gerekçe)

Öncül: 2026-11-09 şeritli koşu (ağaç `yol2/goster-a` 56313c73 = `yol2/goster` 9e157992 + Q1/Q2
bayrakları, K1 kolunda kapalı; Y10, `--goster on`, W = 5, T = 2,0 s, FEC kapalı, N = 10), bu okuma
aracıyla yeniden okundu. K1 öncülü: başarılı 5/10; ilk demet medyan 1,71 [en kötü 2,40] s; ilk ses
3,72 [4,41] s; D p95 4,07 s; duraklamasız 8/10; TEXT en büyük medyan 4,10 [9,04] s; dosya medyan
30,30 [40,65] s, ≤ 30 s 5/10; ACK'li kayıp 0; olay kaybı 0.

| ölçüt | tahmin | gerekçe |
|---|---|---|
| Y10 ilk demet varışı | medyan 1,5–2,0 s; ≤ 3 s 9–10/10 | öncül; varış alıcının tek sıralı Reorder'ında önceki ≤ W dosya biriminin kurtarılmasını bekler, %20 kayıpta ≈ 1 kurtarma |
| Y10 bütün demetler varış p95 | medyan 1,5–2,5 s | öncül (2026-11-09 okuması "PTT varış p95 med 1,94") |
| Y10 ilk duyulabilir ses | ilk demet + T + 10–20 ms; medyan 3,6–4,0 s | T = 2,0 s; tur kuantası 20 ms |
| Y10 D p95 → T önerisi | D p95 2–5 s → **T = 2,5 s** (kırpma) | öncül D p95 4,07 s; kırpma üst sınırı |
| Y10 doğrudan duraklama | 15 s basışında duraklamasız 7–9/10; biriken gecikme medyan ≈ 0, en kötü 0,3–1,5 s | öncül 8/10; T = 2,0 < D'nin kuyruğu |
| Y10 TEXT dosya sürerken | ≤ 10 s 9–10/10; en büyük medyan 3–5 s, en kötü 6–10 s | öncül 10/10, en kötü 9,04 s (eşiğe yakın) |
| Y10 dosya | medyan 28–33 s; ≤ 30 s 3–7/10 | öncül 5/10; şeritsiz K1 19,2 s → şeritli 30,3 s (W = 5 tel hızını sınırlıyor, BULGULAR §2b çıkarımı) |
| Y10 başarılı koşu | 3–7/10; bağlayıcı kalan ölçüt büyük olasılıkla dosya | yukarıdakilerin birleşimi |
| "Yeşil" G4 durumu | **yeşil değil** (Y10 < 8/10) | dosya ölçütü |
| ACK'li birim kaybı → W | 0 → W = 5 | öncül K1 10 koşu 0 |
| olay/demet kaybı | 0 (dinleyici düşüşü, O/C/P eksik, yinelenen, ayrışamayan, sırasız) | öncül |
| kuyruk en yüksek (Y10) | alice `out` ≤ 30; bob Reorder tutulan ≤ 40 | öncül 23 / 27 |
| T1 sayaçları | eksik | §3.7 envanteri |
| r25 teslim@60 / gönderim | K1 6 / ≈ K0 ± %5; K0 6 / 2 000–2 150 B → **R3-r25 geçer** | G2: K0 (6; 2 075 B), K2 (6; 2 123,5 B); K1 = K2 − retx_first; r25'te (d) devreye girmiyor (EK-A §2) |
| d300 l30 ilk demet varışı | medyan 2–4 s; p95 4–8 s → B1 (≤ 3 s) KALIR | öncül yok (şeritli d300 koşmadı). RTT ≈ 600 ms, %30 iki yön: Reorder önündeki dosya biriminin kurtarılması ≈ 1–3 RTT |
| d300 l30 TEXT | havuz p95 3–8 s (B2 ≤ 10 s geçer); 60 s'de %100 7–10/10 | TEXT şeridi dosyanın önünde; aynı Reorder bekleyişi |
| d300 l30 dosya (524 700 B) | medyan 150–250 s; 0–3/10 pencerede (300 s) bitmeyebilir | BDP 7 500 B > W · S = 5 065 B (lanes.rs `file_window_covers_bdp` = yanlış) → dosya pencereyle sınırlı |
| d300 l30 duraklama | D p95 3–8 s; duraklamalı basış çok | varış dağılımının kuyruğu T'yi aşar |
| d300 l0 ilk demet / ilk ses | ilk demet medyan 0,6–1,2 s (B1 iyi ≤ 1,5 s ilk demette geçer); ilk ses ≥ 2,6 s → "≤ 1,5 s" ilk seste yapısal olarak KALIR, "≤ 1,5 s + T" etiketiyle | kayıp yok; OWD 300 ms + netem kuyruğu (≤ W birim) |
| d300 l0 TEXT | havuz p95 0,5–1,5 s (B2 iyi ≤ 2 s geçer) | kayıp yok |
| d300 l0 dosya | medyan 70–110 s | pencereyle sınırlı ≈ 5 065 B / srtt |
| d300 l0 duraklama | 0 (D < T) | kayıp yok |
| callback vekili | TEXT R → dinleyici p95 < 1 ms, en çok < 250 ms; PTT kalıntısı 10–20 ms | öncül 0,1 ms / ≤ 206 ms; 10–19 ms |

## 5. KARAR-KAYDI (bu ön kayda ait; biçim KK · karar · dayanak · geri alma)

- **KK-Ö-33 · G4'te tampon AÇIK (T = 2,0 s).** KK-Ö-11 "Tampon G4'te kapalı (varış ölçülür)" yerine
  tampon açık koşulur: EK-A §6 madde 1 "ilk duyulabilir ses" ve madde 2 "oynatma duraklamaları ve
  biriken gecikme" ölçülen olarak istenir (EK-A §3.4 "ayrıca ölçülür"); varış bob iz "R"den tampon
  öncesi ölçüldüğünden D ve T önerisi aynı koşudan türer; `yol2/goster` f0_flows `--goster on`
  tamponu her zaman açar (şeritli ama tamponsuz kip yok). · dayanak: EK-A §6, §3.4 (otorite sırası
  v4 EK-A > KK satırı) · geri alma: f0_flows'a tampon kapalı seçeneği (KK-Ö-13 kapsamı) + üreticiye
  seçenek; o zaman §3.2 analitik tanımı bağlayıcı olur.
- **KK-Ö-34 · TEXT ölçütü yalnız `T` sınıfı.** "dosya aktarımı sürerken gönderilen TEXT'ler" = `T`
  sınıfı (sahne TEXT'inin analoğu); `K` karşı örnek TEXT'leri raporlanır, ölçüte girmez; pencerede
  TEXT yoksa ölçüt sağlanmamış. · dayanak: KK-Ö-11 (karşı örnek basışı hariç tutma deseni);
  deney-istatistiği boş geçiş kuralı · geri alma: `K` dahil satır okumada ayrıca yazılır.
- **KK-Ö-35 · Doğrudan duraklama toleransı 40 ms** (2 × düğüm turu). T1'in `playout_stalls` sayacı
  kayıtlı olmadığından (§3.7) doğrudan ölçü olay zaman çizgisinden alınır; tur kuantası altı durma
  ayırt edilmez. · dayanak: kama-node `NODE_TICK` 20 ms; playout.rs takvim ızgarası · geri alma: T1
  sayacı kayda girerse (`t1` alanı) o sayaç bağlayıcı olur, bu tanım raporlanan.
- **KK-Ö-36 · "Belirsiz" ACK'li kayıp sayılmaz, listelenir.** alice koşu sonunda teyitsiz birim > 0
  iken süzülmeyen birimlerin hangisinin teyitli olduğu bu izden ayrılamaz. · dayanak: F17; f0a
  `units_never_drained` · geri alma: okuma bu koşuları W kararından önce tek tek inceler.
- **KK-Ö-37 · T1 sayaçları bu koşuda "eksik".** Kayıt için kama-node ölçüm sondası (`NodeProbe`,
  test-util) ve f0_flows `ts` satırına eklemeli `t1` alanı gerekir; bu Ö bölgesinde (düzenek), T1
  koduna dokunmaz, T1 tiplerine bağlı commit'tir (KK-Ö-27). En yüksek doluluk sayacı T1 kodunda yok:
  kayıt eklense de örneklenmiş tepe olur. · dayanak: EK-A §6 madde 5; iş emri "T1'in bölgesine kod
  yazma" · geri alma: —.
- **KK-Ö-38 · PTO prob 2 K1 kolunda yok.** v4 III.1 "PTO prob sayısı 2" E2a setindedir (`K1a`);
  sahip EK-A §2 gösterim setini K1 olarak kabul etti ve 2026-11-09 ölçümleri K1 = `pto_probes = 1`
  ile. Kol parametresi bunu ayırır; seçim orkestratör/sahip satırıdır, bu ön kayıt seçmez. ·
  dayanak: v4 III.1; EK-A §2; PRODUCT-PROFILE `transport-v1.x-goster` `e2a_flags` satırı ("bkz.
  M1") · geri alma: `--kol` ile hücre yeniden üretimi.

## 6. Sapma ve erratum

- Koşu öncesi değişiklik: bu dosya değişir, sha256 yeniden yazılır (`oku.py` `ONKAYIT_SHA256`).
- Koşu sonrası: bu dosya değişmez; `ONKAYIT-ERRATUM-<n>.md` (neden, etkilenen satır, yeni okuma).
- Sessiz yeniden koşu yok. Ölçüm aracı hatası bulunursa önce araç düzeltilir ve eski koşu yeniden
  okunur (kural değişmez); kural değişikliği erratumdur.
