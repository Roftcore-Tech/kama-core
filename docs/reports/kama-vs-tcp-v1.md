# KAMA ↔ TCP adil karşılaştırma kampanyası (kama-vs-tcp-v1) — RAPOR

Tarih: 2026-09-20 · Dal: `campaign/kama-vs-tcp-v1` · Ön-kayıt/dondurma commit'i: `dbbf5958`
([reports/kama-vs-tcp-v1/FREEZE.md](../../reports/kama-vs-tcp-v1/FREEZE.md)) · Tekrar paketi: [baselines/REPRODUCE.md](../../baselines/REPRODUCE.md)

> Her sayı commit'li artefakttan yeniden türetilebilir: ham satırlar `reports/kama-vs-tcp-v1/raw/result-R{1,2}-w{0..3}.csv`,
> özetler `CELL-RESULTS.csv`, grafik verisi `GRAPH.csv`, tablolar `TABLES.md`, muhasebe `ACCOUNTING.md`
> (hepsi `baselines/campaign/kvt_analyze.py` çıktısı; analizör sha256 FREEZE §7'de donmuş).
> Satır-başı ham koşu dizinleri (out.jsonl, c.err/s.err, sayaç anlık görüntüleri; 7 000 dizin) repo DIŞINDA arşivlidir:
> `campaign-v1-runs.tar.gz`, 12.9 MB, sha256 `cdb2770f4c8aa1d6c78fa48b426088688c01ed479e04a340defcd7ae6f44df31` (sahibin makinesinde `/home/ryuzaki/kama-kvt-results/`).

## 1. Ne ölçüldü

Aynı rootless 3-netns/netem düzeneğinde (istemci / orta-kutu netem / sunucu), aynı iş yükü ve aynı ağ profilleriyle:

| kol | ne | not |
|---|---|---|
| `kama-v1-full` | **donmuş ürün** (Transport v1: ARQ + tail recovery `validated_v1` + adaptif FEC 0x05), `bench --transport kama-product` | her satırda `profile_id = kama-v1-full ∧ profile_delta = []` doğrulandı |
| `tcp-std` | Linux TCP (CUBIC varsayılanları), `TCP_NODELAY`, **TLS yok**, sysctl değişikliği yok; `baselines/tcp_adapter` (yalnız `std`) | TCP lehine muhafazakâr |
| `abl-off` / `abl-on` | eski `dr_arqtail_product` sürücüsü (çıplak `Sender`), tail kapalı/açık | **bileşen ablasyonu — ürün DEĞİL, TCP ile aynı tabloda DEĞİL** |

Ölçüm tanımı iki ana kolda aynı katmandadır: gönderim = uygulamanın mesajı SUNDUĞU an; teslim = uygulama-düzeyi teslim
teyidinin/ACK'inin **gönderene varışı**. WARM: uç kurulur + 8 eş-boy ısıtma mesajı, SONRA t0 (kurulum pencerenin dışında).

Eksenler: IID kayıp {10,20,30,40,50} % (iki yönde simetrik) × RTT {50, 250, 1000 ms} × iş yükü {small 48×101 B = 4 848 B,
large 101×995 B = 100 495 B; burst sunum}. Horizon 11/13/20 s, deadline 3×LB = 225/1125/4500 ms (a-priori formüller; FREEZE §3).
N = 30/kol/hücre/replika (4 manşet hücrede 100); tüm kampanya ayrık seed alanıyla iki kez (R1, R2) → **7 000 satır**.

## 2. Hipotez (sonuçtan ÖNCE, FREEZE §1) ve hüküm kuralı

*Makas yüksek kayıp + yüksek RTT + büyük yükte açılır; küçük mesaj / orta kayıpta TCP iyi olabilir.*
Hüküm kuralı (donmuş): oranlar Wilson %95 aralığıyla; **"ÖNDE" yalnız iki kolun aralıkları ayrışıyorsa**, aksi hâlde
**"ayırt edilemedi"**. Metrik = horizon-complete oranı, R1+R2 havuz. Horizon-complete ve deadline-complete AYRI sütunlardır.

## 3. Görev metninden sapmalar (sahip kararıyla, koşudan ÖNCE)

Görevin dayandığı `reports/AI-HANDOFF-CURRENT.md` (HEAD `b0412708`, 2026-08-04) bayattı: tail artık ürün runtime'ında sabit AÇIK,
transport v1 donmuş, ürün yolunda tail'i kapatacak anahtar yok. Bu yüzden: (1) ana kıyas ürün yolunda `kama-v1-full` ↔ `tcp-std`
(üründe "KAMA-ON" ve "KAMA-FEC" aynı koldur); (2) KAMA-OFF/ON yalnız ayrı bileşen-ablasyon tablosunda; (3) topoloji 3-netns
(netem göndericinin kendi çıkışında olsaydı TCP yerel qdisc geri-basıncını görür, UDP görmezdi); (4) netem bu çekirdekte
seed'lenebilir → her satır açık seed taşır. Süre bütçesi nedeniyle dondurmadan ÖNCE çıkarılanlar: bant (`rate`) ekseni, eski
çekirdek grid (burst/terminal-tail), ablasyonun RTT-high kısmı (FREEZE §6).

## 4. Düzenek geçerliliği (Faz 0.5 — kanıt `reports/kama-vs-tcp-v1/preflight/`)

| kapı | sonuç |
|---|---|
| Offload | TSO/GSO 1→0, GRO 0; dört veth ucunda geri-okuma ile doğrulandı (`ethtool` kurulu değil → SIOCETHTOOL ioctl) |
| Kanal kalibrasyonu | **16/16 hücre kabul** (3 000 paket/hücre): gerçekleşen kayıp iki yönde binom %99 aralığında, RTT p50 hedefin ±%5'inde; probe ↔ tc sayaç kaybı 4 ondalıkta aynı |
| TCP sağlama-1 (Mathis) | 2 MB akış: ölçülen/Mathis = 0.71–1.79; bant dışı tek satır (10.3; %1, RTT 250) araştırıldı → ≈ 11 RTT'lik slow-start rejimi; aynı hücre 8 MB ile 1.05 / 1.20 |
| TCP sağlama-2 (`nc`) | adapter süresi OpenBSD netcat'in 0.73–1.59 katı (7 eşleşmiş koşu): TCP kolu engelli DEĞİL. `iperf3` kurulu değil, kurulmadı |
| Kontrol dumanı | 6 kol×yük kombinasyonu 100/100 (horizon ve deadline), drop 0 |
| Kendi hatam | ilk kalibrasyon koşusu betik hatası (tc sayaçları `replace`te sıfırlanmıyor; fark alınmıyordu) nedeniyle GEÇERSİZ sayıldı ve yeniden koşuldu |

## 5. Satır muhasebesi

| replika | beklenen | üretilen | eksik | fazla | yinelenen | NETEM-SKIP | status≠OK | yük-işaretli |
|---|---|---|---|---|---|---|---|---|
| R1 | 3 500 | 3 500 | 0 | 0 | 0 | 0 | 1 | 23 |
| R2 | 3 500 | 3 500 | 0 | 0 | 0 | 0 | 0 | 46 |

Yürütücü loglarında (`raw/run-*.log`) NETEM-SKIP / ROW-ERROR / SETUP-FAILED / SERVER-NOT-READY / NETEM-VERIFY-FAIL / kapı-FAIL sayısı: **0**.
Tek `status≠OK` satır: `R1-main-iid50-high-large-warm-tcp-std-r004` = `ERROR-NO-RESULT` — istemci süreci harness tavanına (horizon + 65 s = 85 s)
takıldı (`rc=124`) ve çıktı yazamadan sonlandırıldı. Adapter'ın üst sınır hesabı ≤ 70 s'dir; **neden belirlenemedi**. Satır paydada kaldı ve
başarısız sayıldı; o hücrede TCP zaten 0/60 olduğu için hiçbir hükmü etkilemez. Süre: 14:14 → 22:11 (7 sa 57 dk; ön tahmin 6.2 sa idi).

## 6. Sonuçlar — özet

**Ana tablo (WARM, 30 hücre, horizon-complete, R1+R2 havuz): `kama-v1-full` 13 hücrede ÖNDE · 17 hücrede ayırt edilemedi · `tcp-std` 0 hücrede önde.**

Hipotezin iki yarısı da desteklendi:

* **Makas büyük yük + yüksek kayıpta açılıyor.** large yükte Kama'nın önde olduğu hücreler: 20%/high (46/60 ↔ 17/60), 30%/low (49/60 ↔ 24/60),
  30%/med (105/200 ↔ 15/200), 30%/high (50/200 ↔ 0/200), 40%/low (41/60 ↔ 0/60), 40%/med (13/60 ↔ 0/60), 50%/low (65/200 ↔ 0/200).
  TCP large yükü ≥ %40 kayıpta HİÇBİR hücrede horizon içinde tamamlayamadı (0/500).
* **Küçük yük / ≤ %30 kayıpta fark yok:** small yükte %10–30 kayıptaki 9 hücrenin tamamı "ayırt edilemedi". small yükte fark %40'ta başlıyor
  (40%/low 54/60 ↔ 37/60, 40%/med 47/60 ↔ 32/60, 40%/high 37/60 ↔ 21/60) ve %50'de üç RTT sınıfında da sürüyor.
* **Hipotezin "yüksek RTT" bileşeni KISMEN:** makas RTT high'ta daha düşük kayıpta (%20) açılıyor; fakat ≥ %40 kayıp + RTT 1000 ms + 100 KB
  köşesinde **iki kol da 0/60** — 20 s horizon içinde Kama da tamamlayamıyor. "En sert köşede Kama kazanır" iddiası bu veriyle YAPILAMAZ.

**COLD (kurulum pencerenin içinde; 4 manşet hücre, N=60/kol):** 3 hücrede Kama önde (30%/med/large 42/60 ↔ 8/60; 30%/high/large 19/60 ↔ 1/60;
50%/low/large 18/60 ↔ 0/60), 10%/low/small'da ayırt edilemedi (60/60 ↔ 60/60).

**R1 ↔ R2 uyumu:** 84 kol×hücre çiftinin **0** tanesinde replikaların Wilson aralıkları ayrışıyor. **Yük duyarlılığı:** işaretli satır 69/7 000;
dışlanınca hükmü değişen ana hücre **0/30**. **Kontrol (bozulmasız):** tüm kollar N/N (tablo aşağıda).

**Bileşen ablasyonu (çıplak Sender, tail OFF ↔ ON; ürün DEĞİL):** ON her kayıp düzeyinde daha yüksek (56→59, 43→57, 43→54, 31→47, 30→38 /60);
aralıklar %20 ve %40'ta ayrışıyor (**abl-on ÖNDE** 2/5), diğer üçünde ayırt edilemedi. Tamamlanma süresi p90'ı OFF'ta ≈ 3.3–4.7 s, ON'da 0.35–2.8 s
(kuyruk kısalıyor; betimsel).

## 7. KAMA'nın GERİDE olduğu yerler (aynı görünürlükte)

Horizon-complete metriğinde Kama'nın istatistiksel olarak geride olduğu hücre **yok**. Ancak:

1. **Bozulmasız ağda TCP çok daha hızlı.** Kontrol hücresi tamamlanma süresi p50: 100 KB için TCP **1.0 ms** ↔ Kama **66.1 ms**;
   4.8 KB için TCP **0.5 ms** ↔ Kama **16.8 ms**. (Kama pacing + 20 ms ACK tick; iyi ağda TCP'nin yerine geçmez.)
2. **Deadline metriğinde TCP bir hücrede ÖNDE:** 20%/med/small — deadline-complete Kama 5/60 [0.04,0.18] ↔ TCP 17/60 [0.19,0.41] (aralıklar ayrışıyor).
   Aynı yönde, aralıkları ÖRTÜŞEN eğilim: 20%/low/small 6↔12, 20%/high/small 8↔17, 10%/high/small 35↔41, 10%/low/small 126↔141 (/200).
   (COLD'da tersine bir hücre var: 10%/low/small Kama 39/60 ↔ TCP 18/60 — TCP el sıkışması pencerenin içinde.)
3. **Düşük kayıp + küçük yükte tamamlanma süresi TCP lehine** (iki kol da ≥ %90 tamamlıyor; p50): 20%/med/small 1 967 ↔ 1 407 ms (Kama +%40),
   10%/med/small 1 215 ↔ 1 074 ms (+%13), 10%/high/small 4 121 ↔ 4 002 ms (+%3), 10%/low/small 206 ↔ 201 ms (+%3).
4. **%20 kayıp + büyük yük + RTT low/med'de eğilim TCP lehine** (aralıklar örtüşüyor → hüküm yok): 52/60 ↔ 59/60 ve 42/60 ↔ 47/60.
5. **Küçük yükte tel maliyeti Kama'da daha yüksek** (post-hoc ek tablo, §7.1): tamamlanan koşularda tel/yük oranı Kama 2.94–5.79× ↔ TCP 2.58–3.60× (large yükte Kama 1.36–2.56× ↔ TCP 1.36–1.73×; TCP yüksek kayıpta tamamlayamadığı için üst uç karşılaştırılamaz).
6. **Deadline (3×LB) sütunu büyük yükte iki kolda da 0.** Ölçülen üstünlük HORIZON tamamlanmasıdır; bir gerçek-zaman/deadline iddiası DEĞİLDİR.

### 7.1 Tel maliyeti


Tel baytı = istemci veth arayüzünde (va0) koşu boyunca tx+rx bayt farkı; ısıtma trafiği DAHİL (iki kolda aynı kural).

| kayıp | RTT | yük | Kama tamamlananlarda tel/yük (n) | TCP tamamlananlarda tel/yük (n) | Kama Σtel/Σteslim-yük (tüm) | TCP Σtel/Σteslim-yük (tüm) |
|---|---|---|---|---|---|---|
| 10% | low | large | 1.40× (60) | 1.36× (60) | 1.45× | 1.36× |
| 10% | low | small | 2.96× (200) | 2.58× (200) | 3.35× | 2.64× |
| 10% | med | large | 1.37× (58) | 1.38× (60) | 1.40× | 1.38× |
| 10% | med | small | 2.94× (60) | 2.63× (60) | 2.98× | 2.72× |
| 10% | high | large | 1.36× (55) | 1.36× (54) | 1.38× | 1.38× |
| 10% | high | small | 3.27× (60) | 2.69× (60) | 3.34× | 2.76× |
| 20% | low | large | 1.68× (52) | 1.53× (59) | 1.65× | 1.54× |
| 20% | low | small | 4.42× (60) | 2.87× (60) | 4.09× | 2.92× |
| 20% | med | large | 1.58× (42) | 1.53× (47) | 1.60× | 1.58× |
| 20% | med | small | 3.23× (60) | 2.61× (58) | 3.31× | 2.74× |
| 20% | high | large | 1.54× (46) | 1.48× (17) | 1.59× | 1.69× |
| 20% | high | small | 3.66× (59) | 2.81× (53) | 3.83× | 2.99× |
| 30% | low | large | 1.86× (49) | 1.73× (24) | 1.90× | 1.94× |
| 30% | low | small | 4.65× (59) | 2.92× (53) | 4.41× | 3.30× |
| 30% | med | large | 1.80× (105) | 1.63× (15) | 1.85× | 2.19× |
| 30% | med | small | 3.70× (59) | 2.85× (51) | 3.74× | 3.33× |
| 30% | high | large | 1.73× (50) | — (0) | 1.90× | 2.82× |
| 30% | high | small | 4.16× (52) | 3.00× (46) | 4.64× | 3.53× |
| 40% | low | large | 2.14× (41) | — (0) | 2.19× | 3.11× |
| 40% | low | small | 4.96× (54) | 3.07× (37) | 5.19× | 4.18× |
| 40% | med | large | 2.03× (13) | — (0) | 2.24× | 3.80× |
| 40% | med | small | 4.21× (47) | 3.13× (32) | 4.44× | 4.53× |
| 40% | high | large | — (0) | — (0) | 2.42× | 6.42× |
| 40% | high | small | 4.99× (37) | 2.59× (21) | 5.25× | 5.17× |
| 50% | low | large | 2.50× (65) | — (0) | 2.69× | 6.83× |
| 50% | low | small | 5.79× (39) | 3.53× (11) | 6.22× | 10.28× |
| 50% | med | large | 2.56× (1) | — (0) | 2.93× | 13.21× |
| 50% | med | small | 5.02× (29) | 3.60× (7) | 5.77× | 10.59× |
| 50% | high | large | — (0) | — (0) | 3.63× | 32.98× |
| 50% | high | small | 5.57× (21) | 2.80× (4) | 6.23× | 16.14× |

## 7.2 Tam tablolar (donmuş analizör çıktısı — `reports/kama-vs-tcp-v1/TABLES.md` ile birebir)

### ANA TABLO — WARM, ürün (kama-v1-full) ↔ tcp-std, R1+R2 havuz

| kayıp | RTT | yük | kama-v1-full horizon x/N [Wilson95] | tcp-std horizon x/N [Wilson95] | kama-v1-full deadline | tcp-std deadline | kama-v1-full t_tam p50/p90 ms | tcp-std t_tam p50/p90 ms | kama-v1-full tel B p50 | tcp-std tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 10% | low | large | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 484.278 / 755.715 | 818.788 / 1655.801 | 140346.0 | 136213.0 | ayırt edilemedi |
| 10% | low | small | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 126/200 [0.56,0.69] | 141/200 [0.64,0.76] | 206.138 / 346.638 | 200.460 / 458.467 | 14346.5 | 12519.5 | ayırt edilemedi |
| 10% | med | large | 58/60 [0.89,0.99] | 60/60 [0.94,1.00] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 2511.862 / 4579.258 | 3824.758 / 6547.316 | 137231.0 | 138905.0 | ayırt edilemedi |
| 10% | med | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 27/60 [0.33,0.58] | 31/60 [0.39,0.64] | 1215.362 / 2119.872 | 1073.917 / 2043.250 | 14232.0 | 12752.5 | ayırt edilemedi |
| 10% | high | large | 55/60 [0.82,0.96] | 54/60 [0.80,0.95] | 0/60 [0.00,0.06] | 1/60 [0.00,0.09] | 8878.124 / 11899.291 | 10003.544 / 15961.348 | 136388.0 | 137178.0 | ayırt edilemedi |
| 10% | high | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 35/60 [0.46,0.70] | 41/60 [0.56,0.79] | 4120.827 / 6350.045 | 4001.601 / 6834.738 | 15830.5 | 13045.0 | ayırt edilemedi |
| 20% | low | large | 52/60 [0.76,0.93] | 59/60 [0.91,1.00] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 719.307 / 2883.537 | 2549.060 / 6409.807 | 159928.0 | 153447.5 | ayırt edilemedi |
| 20% | low | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 6/60 [0.05,0.20] | 12/60 [0.12,0.32] | 300.483 / 881.638 | 479.581 / 1069.864 | 21408.5 | 13907.5 | ayırt edilemedi |
| 20% | med | large | 42/60 [0.57,0.80] | 47/60 [0.66,0.87] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 3400.062 / 9151.303 | 8893.930 / 12486.875 | 153018.5 | 152877.5 | ayırt edilemedi |
| 20% | med | small | 60/60 [0.94,1.00] | 58/60 [0.89,0.99] | 5/60 [0.04,0.18] | 17/60 [0.19,0.41] | 1967.120 / 3802.741 | 1406.683 / 3573.762 | 15651.5 | 12673.0 | ayırt edilemedi |
| 20% | high | large | 46/60 [0.65,0.86] | 17/60 [0.19,0.41] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 12720.506 / 17640.824 | 15343.747 / 19764.348 | 151966.0 | 141579.0 | **kama-v1-full ÖNDE** |
| 20% | high | small | 59/60 [0.91,1.00] | 53/60 [0.78,0.94] | 8/60 [0.07,0.24] | 17/60 [0.19,0.41] | 6559.982 / 10200.272 | 6254.575 / 13326.234 | 17742.0 | 13615.5 | ayırt edilemedi |
| 30% | low | large | 49/60 [0.70,0.89] | 24/60 [0.29,0.53] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 1418.384 / 7749.176 | 6660.924 / 10495.180 | 184310.5 | 163587.5 | **kama-v1-full ÖNDE** |
| 30% | low | small | 59/60 [0.91,1.00] | 53/60 [0.78,0.94] | 2/60 [0.01,0.11] | 5/60 [0.04,0.18] | 659.346 / 3059.496 | 1319.053 / 4377.797 | 22519.5 | 14282.5 | ayırt edilemedi |
| 30% | med | large | 105/200 [0.46,0.59] | 15/200 [0.05,0.12] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 4980.592 / 9340.141 | 10368.172 / 11838.770 | 170486.5 | 109396.0 | **kama-v1-full ÖNDE** |
| 30% | med | small | 59/60 [0.91,1.00] | 51/60 [0.74,0.92] | 2/60 [0.01,0.11] | 2/60 [0.01,0.11] | 2677.684 / 6899.518 | 3135.176 / 8381.641 | 17997.5 | 13717.0 | ayırt edilemedi |
| 30% | high | large | 50/200 [0.20,0.31] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 17060.226 / 19518.338 |  /  | 135354.5 | 72204.0 | **kama-v1-full ÖNDE** |
| 30% | high | small | 52/60 [0.76,0.93] | 46/60 [0.65,0.86] | 2/60 [0.01,0.11] | 6/60 [0.05,0.20] | 8440.327 / 15846.310 | 8299.651 / 14851.237 | 20006.0 | 14459.5 | ayırt edilemedi |
| 40% | low | large | 41/60 [0.56,0.79] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 3361.449 / 8420.033 |  /  | 205310.5 | 67798.5 | **kama-v1-full ÖNDE** |
| 40% | low | small | 54/60 [0.80,0.95] | 37/60 [0.49,0.73] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 1280.244 / 4759.697 | 1574.921 / 8921.129 | 23788.0 | 13658.5 | **kama-v1-full ÖNDE** |
| 40% | med | large | 13/60 [0.13,0.34] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 9187.803 / 11540.681 |  /  | 106803.5 | 53450.0 | **kama-v1-full ÖNDE** |
| 40% | med | small | 47/60 [0.66,0.87] | 32/60 [0.41,0.65] | 0/60 [0.00,0.06] | 1/60 [0.00,0.09] | 5884.560 / 11020.213 | 6207.010 / 12095.877 | 20509.5 | 14922.5 | **kama-v1-full ÖNDE** |
| 40% | high | large | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] |  /  |  /  | 73277.0 | 40161.5 | ayırt edilemedi |
| 40% | high | small | 37/60 [0.49,0.73] | 21/60 [0.24,0.48] | 1/60 [0.00,0.09] | 0/60 [0.00,0.06] | 12472.058 / 17279.939 | 13151.791 / 17135.024 | 22809.5 | 12393.0 | **kama-v1-full ÖNDE** |
| 50% | low | large | 65/200 [0.26,0.39] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 5784.680 / 9082.034 |  /  | 141389.5 | 39769.0 | **kama-v1-full ÖNDE** |
| 50% | low | small | 39/60 [0.52,0.76] | 11/60 [0.11,0.30] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 3080.663 / 7580.045 | 2341.226 / 3911.916 | 26079.0 | 12753.0 | **kama-v1-full ÖNDE** |
| 50% | med | large | 1/60 [0.00,0.09] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 7825.727 / 7825.727 |  /  | 44555.0 | 29580.5 | ayırt edilemedi |
| 50% | med | small | 29/60 [0.36,0.61] | 7/60 [0.06,0.22] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 8640.160 / 12047.540 | 8466.757 / 11066.063 | 23453.5 | 12631.5 | **kama-v1-full ÖNDE** |
| 50% | high | large | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] |  /  |  /  | 45432.0 | 21316.5 | ayırt edilemedi |
| 50% | high | small | 21/60 [0.24,0.48] | 4/60 [0.03,0.16] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 14780.681 / 19161.126 | 16704.027 / 18963.321 | 26361.0 | 10449.5 | **kama-v1-full ÖNDE** |

Özet: **kama-v1-full ÖNDE**: 13 · ayırt edilemedi: 17

#### KAMA'nın GERİDE olduğu hücreler (horizon, aralıklar ayrışıyor)

- (yok)

#### Tamamlanma SÜRESİNDE KAMA'nın geride olduğu hücreler (iki kol da ≥ %90 horizon-complete; p50 kıyas, aralık testi YOK — betimsel)

- control · kayıp 0% · RTT none · large: kama p50 66.1 ms  vs  tcp p50 1.0 ms
- control · kayıp 0% · RTT none · small: kama p50 16.8 ms  vs  tcp p50 0.5 ms
- main · kayıp 10% · RTT high · small: kama p50 4120.8 ms  vs  tcp p50 4001.6 ms
- main · kayıp 10% · RTT low · small: kama p50 206.1 ms  vs  tcp p50 200.5 ms
- main · kayıp 10% · RTT med · small: kama p50 1215.4 ms  vs  tcp p50 1073.9 ms
- main · kayıp 20% · RTT med · small: kama p50 1967.1 ms  vs  tcp p50 1406.7 ms

### KONTROL (bozulmasız) — her kol N/N tamamlamalı

| kayıp | RTT | yük | kama-v1-full horizon x/N [Wilson95] | tcp-std horizon x/N [Wilson95] | kama-v1-full deadline | tcp-std deadline | kama-v1-full t_tam p50/p90 ms | tcp-std t_tam p50/p90 ms | kama-v1-full tel B p50 | tcp-std tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0% | none | large | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 66.138 / 76.084 | 0.965 / 1.293 | 120349.0 | 127679.0 | ayırt edilemedi |
| 0% | none | small | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 16.848 / 19.184 | 0.512 / 0.619 | 11748.0 | 15834.0 | ayırt edilemedi |

Özet: ayırt edilemedi: 2

### COLD senaryo (kurulum pencerenin İÇİNDE) — manşet hücreler

| kayıp | RTT | yük | kama-v1-full horizon x/N [Wilson95] | tcp-std horizon x/N [Wilson95] | kama-v1-full deadline | tcp-std deadline | kama-v1-full t_tam p50/p90 ms | tcp-std t_tam p50/p90 ms | kama-v1-full tel B p50 | tcp-std tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 10% | low | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 39/60 [0.52,0.76] | 18/60 [0.20,0.43] | 185.970 / 326.600 | 267.637 / 1238.921 | 11376.0 | 9835.5 | ayırt edilemedi |
| 30% | med | large | 42/60 [0.57,0.80] | 8/60 [0.07,0.24] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 6126.415 / 10235.689 | 10911.818 / 12266.118 | 157054.5 | 122036.5 | **kama-v1-full ÖNDE** |
| 30% | high | large | 19/60 [0.21,0.44] | 1/60 [0.00,0.09] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 18480.633 / 19653.218 | 15596.356 / 15596.356 | 160712.5 | 89670.5 | **kama-v1-full ÖNDE** |
| 50% | low | large | 18/60 [0.20,0.43] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 7546.105 / 10518.845 |  /  | 192938.5 | 33537.5 | **kama-v1-full ÖNDE** |

Özet: **kama-v1-full ÖNDE**: 3 · ayırt edilemedi: 1

### BİLEŞEN ABLASYONU — çıplak Sender sürücüsü, tail OFF ↔ ON (ürün DEĞİL; TCP ile kıyaslanmaz)

| kayıp | RTT | yük | abl-off horizon x/N [Wilson95] | abl-on horizon x/N [Wilson95] | abl-off deadline | abl-on deadline | abl-off t_tam p50/p90 ms | abl-on t_tam p50/p90 ms | abl-off tel B p50 | abl-on tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 10% | low | small | 56/60 [0.84,0.97] | 59/60 [0.91,1.00] | 35/60 [0.46,0.70] | 37/60 [0.49,0.73] | 204.000 / 3300.000 | 212.000 / 345.000 | 36523.0 | 33153.5 | ayırt edilemedi |
| 20% | low | small | 43/60 [0.59,0.81] | 57/60 [0.86,0.98] | 7/60 [0.06,0.22] | 10/60 [0.09,0.28] | 320.000 / 3484.000 | 363.000 / 811.000 | 223091.0 | 78034.0 | **abl-on ÖNDE** |
| 30% | low | small | 43/60 [0.59,0.81] | 54/60 [0.80,0.95] | 10/60 [0.09,0.28] | 9/60 [0.08,0.26] | 347.000 / 3437.000 | 395.000 / 1186.000 | 119691.0 | 88712.0 | ayırt edilemedi |
| 40% | low | small | 31/60 [0.39,0.64] | 47/60 [0.66,0.87] | 1/60 [0.00,0.09] | 2/60 [0.01,0.11] | 566.000 / 4637.000 | 633.000 / 1687.000 | 342094.0 | 108155.0 | **abl-on ÖNDE** |
| 50% | low | small | 30/60 [0.38,0.62] | 38/60 [0.51,0.74] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 834.000 / 4676.000 | 763.000 / 2814.000 | 272218.5 | 115366.0 | ayırt edilemedi |

Özet: **abl-on ÖNDE**: 2 · ayırt edilemedi: 3

### KONTROL (ablasyon kolları)

| kayıp | RTT | yük | abl-off horizon x/N [Wilson95] | abl-on horizon x/N [Wilson95] | abl-off deadline | abl-on deadline | abl-off t_tam p50/p90 ms | abl-on t_tam p50/p90 ms | abl-off tel B p50 | abl-on tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0% | none | small | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 42.000 / 49.000 | 43.000 / 50.000 | 20256.0 | 20151.0 | ayırt edilemedi |

Özet: ayırt edilemedi: 1

### R1 ↔ R2 UYUMU (kol × hücre; horizon oranı)

| grup | kayıp | RTT | yük | mod | kol | R1 x/N | R2 x/N | aralıklar | 
|---|---|---|---|---|---|---|---|---|
| ablation | 10% | low | small | warm | abl-off | 28/30 | 28/30 | örtüşüyor |
| ablation | 10% | low | small | warm | abl-on | 30/30 | 29/30 | örtüşüyor |
| ablation | 20% | low | small | warm | abl-off | 25/30 | 18/30 | örtüşüyor |
| ablation | 20% | low | small | warm | abl-on | 30/30 | 27/30 | örtüşüyor |
| ablation | 30% | low | small | warm | abl-off | 21/30 | 22/30 | örtüşüyor |
| ablation | 30% | low | small | warm | abl-on | 28/30 | 26/30 | örtüşüyor |
| ablation | 40% | low | small | warm | abl-off | 17/30 | 14/30 | örtüşüyor |
| ablation | 40% | low | small | warm | abl-on | 24/30 | 23/30 | örtüşüyor |
| ablation | 50% | low | small | warm | abl-off | 11/30 | 19/30 | örtüşüyor |
| ablation | 50% | low | small | warm | abl-on | 17/30 | 21/30 | örtüşüyor |
| cold | 10% | low | small | cold | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| cold | 10% | low | small | cold | tcp-std | 30/30 | 30/30 | örtüşüyor |
| cold | 30% | high | large | cold | kama-v1-full | 11/30 | 8/30 | örtüşüyor |
| cold | 30% | high | large | cold | tcp-std | 0/30 | 1/30 | örtüşüyor |
| cold | 30% | med | large | cold | kama-v1-full | 22/30 | 20/30 | örtüşüyor |
| cold | 30% | med | large | cold | tcp-std | 6/30 | 2/30 | örtüşüyor |
| cold | 50% | low | large | cold | kama-v1-full | 10/30 | 8/30 | örtüşüyor |
| cold | 50% | low | large | cold | tcp-std | 0/30 | 0/30 | örtüşüyor |
| control | 0% | none | large | warm | kama-v1-full | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | large | warm | tcp-std | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | abl-off | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | abl-on | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | kama-v1-full | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | tcp-std | 100/100 | 100/100 | örtüşüyor |
| main | 10% | high | large | warm | kama-v1-full | 27/30 | 28/30 | örtüşüyor |
| main | 10% | high | large | warm | tcp-std | 26/30 | 28/30 | örtüşüyor |
| main | 10% | high | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 10% | high | small | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 10% | low | large | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 10% | low | large | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 10% | low | small | warm | kama-v1-full | 100/100 | 100/100 | örtüşüyor |
| main | 10% | low | small | warm | tcp-std | 100/100 | 100/100 | örtüşüyor |
| main | 10% | med | large | warm | kama-v1-full | 29/30 | 29/30 | örtüşüyor |
| main | 10% | med | large | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 10% | med | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 10% | med | small | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 20% | high | large | warm | kama-v1-full | 25/30 | 21/30 | örtüşüyor |
| main | 20% | high | large | warm | tcp-std | 4/30 | 13/30 | örtüşüyor |
| main | 20% | high | small | warm | kama-v1-full | 29/30 | 30/30 | örtüşüyor |
| main | 20% | high | small | warm | tcp-std | 25/30 | 28/30 | örtüşüyor |
| main | 20% | low | large | warm | kama-v1-full | 25/30 | 27/30 | örtüşüyor |
| main | 20% | low | large | warm | tcp-std | 30/30 | 29/30 | örtüşüyor |
| main | 20% | low | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 20% | low | small | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 20% | med | large | warm | kama-v1-full | 19/30 | 23/30 | örtüşüyor |
| main | 20% | med | large | warm | tcp-std | 23/30 | 24/30 | örtüşüyor |
| main | 20% | med | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 20% | med | small | warm | tcp-std | 29/30 | 29/30 | örtüşüyor |
| main | 30% | high | large | warm | kama-v1-full | 27/100 | 23/100 | örtüşüyor |
| main | 30% | high | large | warm | tcp-std | 0/100 | 0/100 | örtüşüyor |
| main | 30% | high | small | warm | kama-v1-full | 28/30 | 24/30 | örtüşüyor |
| main | 30% | high | small | warm | tcp-std | 24/30 | 22/30 | örtüşüyor |
| main | 30% | low | large | warm | kama-v1-full | 25/30 | 24/30 | örtüşüyor |
| main | 30% | low | large | warm | tcp-std | 11/30 | 13/30 | örtüşüyor |
| main | 30% | low | small | warm | kama-v1-full | 30/30 | 29/30 | örtüşüyor |
| main | 30% | low | small | warm | tcp-std | 27/30 | 26/30 | örtüşüyor |
| main | 30% | med | large | warm | kama-v1-full | 52/100 | 53/100 | örtüşüyor |
| main | 30% | med | large | warm | tcp-std | 3/100 | 12/100 | örtüşüyor |
| main | 30% | med | small | warm | kama-v1-full | 29/30 | 30/30 | örtüşüyor |
| main | 30% | med | small | warm | tcp-std | 26/30 | 25/30 | örtüşüyor |
| main | 40% | high | large | warm | kama-v1-full | 0/30 | 0/30 | örtüşüyor |
| main | 40% | high | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 40% | high | small | warm | kama-v1-full | 18/30 | 19/30 | örtüşüyor |
| main | 40% | high | small | warm | tcp-std | 13/30 | 8/30 | örtüşüyor |
| main | 40% | low | large | warm | kama-v1-full | 18/30 | 23/30 | örtüşüyor |
| main | 40% | low | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 40% | low | small | warm | kama-v1-full | 26/30 | 28/30 | örtüşüyor |
| main | 40% | low | small | warm | tcp-std | 18/30 | 19/30 | örtüşüyor |
| main | 40% | med | large | warm | kama-v1-full | 7/30 | 6/30 | örtüşüyor |
| main | 40% | med | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 40% | med | small | warm | kama-v1-full | 24/30 | 23/30 | örtüşüyor |
| main | 40% | med | small | warm | tcp-std | 18/30 | 14/30 | örtüşüyor |
| main | 50% | high | large | warm | kama-v1-full | 0/30 | 0/30 | örtüşüyor |
| main | 50% | high | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 50% | high | small | warm | kama-v1-full | 12/30 | 9/30 | örtüşüyor |
| main | 50% | high | small | warm | tcp-std | 2/30 | 2/30 | örtüşüyor |
| main | 50% | low | large | warm | kama-v1-full | 35/100 | 30/100 | örtüşüyor |
| main | 50% | low | large | warm | tcp-std | 0/100 | 0/100 | örtüşüyor |
| main | 50% | low | small | warm | kama-v1-full | 20/30 | 19/30 | örtüşüyor |
| main | 50% | low | small | warm | tcp-std | 6/30 | 5/30 | örtüşüyor |
| main | 50% | med | large | warm | kama-v1-full | 1/30 | 0/30 | örtüşüyor |
| main | 50% | med | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 50% | med | small | warm | kama-v1-full | 14/30 | 15/30 | örtüşüyor |
| main | 50% | med | small | warm | tcp-std | 3/30 | 4/30 | örtüşüyor |

Özet: 84 kol×hücre çiftinin 0 tanesinde R1 ve R2 Wilson aralıkları ayrışıyor (beklenen yanlış-pozitif ≈ %5 altı).

### YÜK İŞARETİ DUYARLILIĞI (loadavg1 > 2.0 satırlar DIŞLANARAK ana tablo hükümleri)

İşaretli satır: 69/7000. İşaretliler dışlanınca hükmü DEĞİŞEN ana hücre: 0/30.

## 8. Sınırlar

* **Emüle ağ.** Tüm bozulma Linux `netem` ile üretildi (orta-kutu netns'inde). Gerçek radyo/uydu/hücresel bağ DEĞİL.
* **RF / jammer verisi YOK.** Kayıp modeli IID (bağımsız, sabit olasılıklı) ve iki yönde simetriktir. Burst (Gilbert–Elliott),
  terminal-tail ve yönlü-asimetrik kayıp bu kampanyada KOŞULMADI (süre bütçesi; FREEZE §6). Bunlar için mevcut kanıt:
  DR-ARQ-PRODUCT-TAIL-1 (çıplak Sender) ve Faz-5 T2a/T2b/T6 (ürün, N=30).
* **TCP kolu TLS'siz.** `tcp-std` = çıplak TCP (CUBIC, `TCP_NODELAY`). Gerçek bir uygulama TLS el sıkışması ve kayıt
  ek yükü taşırdı → bu seçim TCP **lehine** muhafazakârdır. Kama kolu da PoC `IdentitySeam` (+16 B etiket, gerçek AEAD yok) ile koştu.
* **Bant ekseni yok.** `rate` sınırı uygulanmadı; hipotezin "düşük bant" bileşeni bu kampanyada SINANMADI.
* **COLD senaryo** yalnız 4 manşet hücrede, N=30/replika koşuldu; Kama'da el sıkışma olmadığı için Kama-COLD = "ısıtmasız ilk uçuş",
  TCP-COLD = `connect()` pencerenin içinde.
* **FEC kolu durumu.** Ayrı bir "KAMA-FEC" kolu YOKTUR: donmuş ürün profili (`kama-v1-full`) adaptif FEC'i (profil 0x05) zaten
  içerir; ürün yolunda FEC'i ya da tail'i tek başına kapatan anahtar yoktur (kapatmak `kama-custom` üretir, ürün sonucu sayılmaz).
  FEC'in bu sonuçlara NEDENSEL katkısı bu kampanyayla ayrıştırılamaz (bkz. KNOWN-LIMITATIONS LIM-002).
* **OFF/ON ablasyonu ürün DEĞİLDİR.** `abl-off`/`abl-on` eski `dr_arqtail_product` sürücüsüdür (çıplak `Sender`; FEC yok, seam yok,
  harness'a özel çerçeve ve SACK üreten alıcı). Yalnız "tail probe açık/kapalı" bileşen farkını gösterir; TCP ile kıyaslanamaz.
* **N ve güven aralıkları.** Hücre×kol başına N=30/replika (havuzda 60), manşet hücrelerde 100/replika (havuzda 200). Wilson %95 aralığı
  N=60'ta en geniş hâlinde ≈ ±0.12'dir: 0.12'den küçük oran farkları bu kampanyayla AYIRT EDİLEMEZ. 30 ana hücrede çoklu karşılaştırma
  düzeltmesi YAPILMADI; "önde" hükümleri hücre-bazında okunmalıdır.
* **Horizon kısa ve a-prioridir** (11/13/20 s). "Horizon içinde tamamlanmadı" ≠ "asla tamamlanmaz". Deadline (3×LB) rate'siz ağda çok sıkıdır
  (225/1125/4500 ms); deadline sütunu bir ürün SLO'su DEĞİLDİR (ürün SLO'su yoktur).

## 9. Geçerlilik tehditleri

1. **Emülasyon ≠ saha.** netem kuyruk/zamanlama davranışı gerçek NIC/RF zincirini temsil etmez; çekirdek-içi TCP ile kullanıcı-alanı
   Kama aynı makinede, aynı CPU'da koştu.
2. **IID kayıp ≠ RF kaybı.** Sahadaki kayıp çoğunlukla ilişkilidir (burst, sönümlenme, jammer görev döngüsü). IID sonuçları RF'e genellenemez.
3. **Tek makine, tek çekirdek sürümü** (7.0.12, CUBIC varsayılanları). Başka çekirdek/CC (BBR) farklı sonuç verebilir.
4. **Paralel işçiler.** Kampanya 4 eşzamanlı işçiyle koştu. Ön-uçuşta kontrol hücresinde süre kayması ≤ +%4 (TCP'de ≈ 20 µs) ölçüldü;
   **kayıplı hücrelerde paralellik etkisi doğrulanamadı** (iki seri koşu aynı seed'i paylaştığı için bağımsız örnek değildi, n=15).
   Azaltıcı: kollar aynı işçi içinde dönüşümlü koştu → olası yük etkisi iki kola eşit düşer; `loadavg1 > 2.0` satırlar işaretlendi ve
   duyarlılık analizi verildi.
5. **Tekrarlanabilirlik istatistikseldir.** netem bu çekirdekte seed'lenebilir (görev metni tersini varsaymıştı) ve her satırın seed'i
   manifesttedir; ancak protokol zamanlaması kayıp dizisinin hangi pakete düştüğünü değiştirir → yeniden koşu bit-bit aynı olmaz.
   Aynı seed iki kolda ORTAKTIR (eşleşmiş tasarım); R1/R2 ayrık seed alanlarıdır.
6. **Isıtma artığı.** Yüksek kayıpta 8 ısıtma mesajının tamamı 30 s tavanında teyit edilemeyebilir (`warmup_residue`); artık trafik
   ölçüm penceresine taşar. Kural iki kolda aynıdır ve satır başına raporlanır, ama etkisi kollar arasında eşit olmak zorunda değildir
   (TCP'de artık baytlar akışın ÖNÜNDE sıralıdır → head-of-line; Kama'da mesaj-granülü).
7. **Anlam farkı normalize edilmedi.** TCP güvenilir + SIRALI; Kama v1 güvenilir, mesaj-granülü (sıra garantisi yok). Sıra gerektiren
   bir uygulama Kama üstünde ek iş yapardı.
8. **Offload.** TSO/GSO/GRO kapalı koşuldu (doğrulandı). Faz-5 kampanyası offload AÇIKKEN koşmuştu (`ethtool-missing`); iki kampanyanın
   TCP sayıları bu yüzden doğrudan karşılaştırılmamalıdır.
9. **Harness zaman aşımı.** İstemci süreci harness tavanını (horizon + 65 s) aşarsa satır `ERROR-NO-RESULT` olur, paydada kalır ve
   başarısız sayılır (muhasebe bölümünde listelenir).

## 10. Yapılmadı

* **QUIC / Quinn kolu** — bu raporda YOK (Faz 5; ayrı manifest eki gerektirir).
* **KCP kolu** — yok.
* **Bant (`rate`) ekseni**, **burst / terminal-tail çekirdek grid'i** — süre bütçesi nedeniyle dondurmadan ÖNCE çıkarıldı.
* **BBR / farklı TCP ayarı, TLS'li TCP** — yok.
* **Gerçek RF / jammer / saha ölçümü** — yok.
* **Grafik** — çizilmedi; veri `reports/kama-vs-tcp-v1/GRAPH.csv` (şema: `loss_pct, rtt_class, workload, arm, completion_rate, …`).
