# KAMA ↔ TCP kampanyası (kama-vs-tcp-v1) — ÖN-KAYIT ve DONDURMA

Tarih: 2026-09-20 · Dal: `campaign/kama-vs-tcp-v1` · Ürün kaynak commit'i: `bafaefdcc6e1c911cbece28d84b5992b7dbb3227` (main `bafaefdc` + yalnız kampanya dosyaları; `crates/` DEĞİŞMEDİ).
**Bu dosya ve manifestler ölçümden ÖNCE commit'lenir. Bu noktadan sonra hücre, eşik, horizon, N, analiz kuralı DEĞİŞMEZ.**

## 1. Hipotez (sonuçtan ÖNCE yazıldı)

Makas (KAMA lehine) **yüksek kayıp + yüksek RTT + büyük yük** köşesinde açılır; **küçük mesaj / orta kayıpta TCP iyi olabilir**.
(Görevdeki "düşük bant" ekseni süre bütçesi nedeniyle bu kampanyadan çıkarıldı — §6.) Sonuç hipotezi çürütürse AYNEN raporlanır;
KAMA'nın geride olduğu hücreler ayrı başlıkta ve aynı görünürlükte verilir.

## 2. Görev metninden SAPMALAR (sahip kararıyla, koşudan önce)

Görev `reports/AI-HANDOFF-CURRENT.md`'ye (HEAD `b0412708`, 2026-08-04) dayanıyordu; o belge BAYAT:
tail artık ürün runtime'ında sabit AÇIK (`validated_v1`), transport v1 donmuş, ürün yolunda tail'i kapatacak anahtar YOK.
1. **Ana kıyas ürün yolunda:** `kama-v1-full` (donmuş ürün profili: ARQ + tail ON + adaptif FEC 0x05; `bench --transport kama-product`,
   `profile_delta = []` her satırda doğrulanır) ↔ `tcp-std`. Üründe "KAMA-ON" ve "KAMA-FEC" AYNI koldur.
2. **OFF/ON ayrı tabloda:** `abl-off`/`abl-on` = eski `dr_arqtail_product` sürücüsü (çıplak `Sender`; FEC/seam yok) — **bileşen ablasyonu**,
   ürün DEĞİL, TCP ile aynı tabloya GİRMEZ (ADR-hostile-validation-contract §2).
3. **Topoloji 3-netns** (istemci / orta-kutu netem / sunucu; `scripts/netem/kapi3_harness.sh`): netem göndericinin kendi çıkışında
   olsaydı TCP yerel qdisc geri-basıncını (TSQ) görür, UDP görmezdi.
4. **netem seed'lenebilir** (çekirdek 7.0.12 / iproute2 7.2.0; görev "seed'lenemez" varsaymıştı): her satır açık seed taşır.

## 3. Tasarım

| eksen | değer |
|---|---|
| kayıp | IID {10,20,30,40,50} %, **iki yönde simetrik** |
| RTT | low/med/high = OWD 25/125/500 ms her yön (DR-ARQ-PRODUCT-TAIL-1 sınıfları) |
| iş yükü | small = 48×101 B (4 848 B; o harness'ın donmuş yükü) · large = 101×995 B (100 495 B; 995 B = ürün v1 mesaj tavanı, fragmenter KAPALI) · burst sunum |
| kollar (ana) | `kama-v1-full`, `tcp-std` (Linux varsayılanı CUBIC, TCP_NODELAY, TLS YOK, sysctl değişikliği YOK) |
| ölçüm tanımı | gönderim = uygulamanın SUNUM anı; teslim = uygulama-düzeyi teslim teyidinin/ACK'inin **gönderene varışı** (iki kolda aynı katman) |
| WARM | uç kurulu + 8 eş-boy ısıtma mesajı (tavan aralık×8+30 s, iki kolda aynı sabit), SONRA t0. `warmup_residue` satırda raporlanır |
| COLD (ikincil) | ısıtma yok; TCP'de t0 `connect()`'ten ÖNCE, Kama'da el sıkışma yok (uç kurulur kurulmaz t0). Yalnız 4 manşet hücre, N=30 |
| horizon | H = ⌈10 s + 10·RTT⌉ → 11 / 13 / 20 s (a-priori formül) |
| deadline | D = 3×LB, LB = serialize + 1.5·RTT (TDD §3 formülü, değişmedi; rate yok → serialize 0) → 225 / 1125 / 4500 ms |
| N | hücre×kol başına 30; **manşet hücreler N=100**: 30%/high/large · 30%/med/large · 50%/low/large · 10%/low/small |
| replika | tüm kampanya ayrık seed alanıyla İKİ KEZ (R1, R2; seed kesişimi 0) |
| seed | `sha256("KVT1|replika|hücre|yük|mod|rep|yön")[:8] mod 2^62` — kollar arasında ORTAK (eşleşmiş), yönler/replikalar arasında ayrık |
| sıra | hücre içinde kollar ardışık ve dönüşümlü; kol sırası (rep + hücre_indeksi) kadar döner (ilk-koşan dengesi 725/725) |
| kontrol | bozulmasız hücre, kol başına N=100 (ön-uçuş kapısı §5'te ayrıca koşuldu) |
| ablasyon | `abl-off`/`abl-on`, small, RTT low, 5 kayıp düzeyi, N=30 |
| satır | replika başına 3 500 (control 600 · main 2 360 · cold 240 · ablation 300) → toplam **7 000** |

Fail-closed: hücre başında yalnız-gecikme netem + ping kapısı (RTT 2·OWD bandı); her satırda `tc -j` dökümü beklenen
gecikme/kayıp/seed ile SAYISAL karşılaştırılır; tutmazsa satır `NETEM-SKIP` olarak LOGLANIR (sessiz düşürme yok).
Offload: TSO/GSO/GRO dört veth ucunda ioctl ile kapatılıp GERİ OKUNUR; kapatılamazsa yürütücü DURUR.
Satır başına: netem qdisc sayaç farkları (iki yön), `ip -s link` veth bayt farkı (tel maliyeti; her kolda aynı yöntem), loadavg, netem limit.

## 4. Analiz kuralları (donmuş — `baselines/campaign/kvt_analyze.py`)

Oranlar Wilson %95 aralığıyla; **"ÖNDE" yalnız iki kolun aralıkları ayrışıyorsa**, aksi hâlde "ayırt edilemedi" (metrik: horizon-complete, R1+R2 havuz).
Horizon-complete ve deadline-complete AYRI sütunlar. `status≠OK` satır paydada kalır ve başarısız sayılır. `loadavg1 > 2.0` satır işaretlenir;
duyarlılık tablosu işaretlileri dışlar. R1↔R2 uyumu kol×hücre bazında raporlanır. Ablasyon ve COLD ayrı tablolardadır.

## 5. Ön-uçuş sonuçları (kanıt: `preflight/`)

| kapı | sonuç |
|---|---|
| Offload | TSO/GSO 1→0, GRO 0 (dört uçta geri-okuma doğrulandı; `smoke_control_run.log` TOPO-UP satırı). `ethtool` kurulu değil → SIOCETHTOOL ioctl |
| Kanal kalibrasyonu | 16/16 hücre KABUL (3 000 paket/hücre): gerçekleşen kayıp iki yönde binom %99 aralığında, RTT p50 hedefin ±%5'inde; probe ↔ tc sayaç kaybı 4 ondalıkta aynı (`calibration.csv`). İlk koşu betik hatası (tc sayaçları birikimli — fark alınmıyordu) nedeniyle GEÇERSİZ sayılıp yeniden koşuldu |
| TCP sağlama-1 (Mathis) | 2 MB akış: oran 0.71–1.79; **1 satır bant dışı (10.3; %1 kayıp, RTT 250)** → araştırıldı: akış ~11 RTT = slow-start rejimi; aynı hücre 8 MB ile 1.05 / 1.20 (`tcp_sanity_long_flow.csv`). 2 MB ≈ 1 470 paket → GSO süper-paketi netem'den GEÇMİYOR |
| TCP sağlama-2 (nc) | OpenBSD netcat, 3 hücre: adapter süresi nc'nin 0.73–1.59 katı (7 eşleşmiş koşu); adapter sistematik olarak YAVAŞ DEĞİL (`tcp_sanity.csv`). iperf3 kurulu değil — kurulmadı |
| Kontrol dumanı | 6 kol×yük kombinasyonu **100/100** horizon ve deadline; drop 0; yük-işaretli satır 0 (`smoke_control_result.csv`) |
| Paralellik | 4 eşzamanlı işçi: kontrol hücresinde süre kayması Kama +%0.7, TCP +%3.9 (≈ 20 µs). Kayıplı hücre kıyası **SONUÇSUZ** (iki seri koşu aynı seed = aynı gerçekleşme; n=15) — raporun Tehditler bölümüne girer. Adalet: kollar AYNI işçi içinde dönüşümlü → olası yük etkisi iki kola eşit düşer |

## 6. Süre bütçesi ve KESME kararları (dondurmadan ÖNCE)

Pilot (48 satır; kampanyaya GİRMEZ) satır-başı süreyi verdi: ısıtma yüksek kayıp/RTT'de satırı 12–36 s'ye çıkarıyor. Seri tahmin iki replika için
≈ 25 saat. Görevdeki öncelik sırası uygulandı: (3) **bant ekseni ÇIKARILDI**, (2) **mevcut çekirdek grid (burst/terminal-tail) ÇIKARILDI**;
ek olarak ablasyon RTT high'tan, manşet N=100 50%/high/large'dan (yerine 50%/low/large) vazgeçildi. Kalan: 4 işçi × replika başına ≈ 3.1 saat → **≈ 6.2 saat**.

## 7. Donmuş kimlikler (sha256)

| nesne | sha256 |
|---|---|
| `manifest-R1.csv` | `875b369f8726eddd547e80117cfae9d35318e229c4b44af65b669529df53d2c5` |
| `manifest-R2.csv` | `5e22c2ab3cfda5182cd5b86281a780ada3a54bde1c6d9194e000d9256a93a9d9` |
| `target/release/bench` (kama-v1-full kolu) | `7a4e2d5c5e5966b3f4ef862919b7b0a7f2ca6e48d3aeb9b1b9a6dcda32f492d3` |
| `baselines/target/release/tcp_adapter` | `46d07cbb0c7029ae01ceb6727c16980be0396c0a3742eac0300c2a13ec71db38` |
| `baselines/target/release/udp_probe` | `859591e736c893dfbb778d6125cc0fbdb524fbbf1f3d4258409606b5977a7876` |
| `target/release/examples/dr_arqtail_product` | `4a94cde68fc9910672d566b408d51c19d9251bc18d12b27d9a0fcaed11d93a53` |
| `baselines/campaign/kvt_analyze.py` (analyzer) | `46ae83ee137e376f97432e4156a50bbbc105994c4867034f3b878649edbc3d1c` |
| `baselines/campaign/kvt_exec.py` | `2c0b084904020f0cca7c4df745d8d857698d2f63417399e891131a8b6aeaa986` |
| `baselines/campaign/kvt_manifest.py` | `b09a02b8fbca1275320fcf5f82355e3d9dda51eee65eeae18c4b3bebd10e9f27` |
| `baselines/campaign/kvt_row.py` | `2352d251e00646d70e998cfc20938102eaba04b952021a8689105afabd120ca8` |
| `scripts/netem/kapi3_harness.sh` | `10f187b21d09131a69213c01f3c7062377334c910d59bd07f1169f5310f7e57a` |
| `Cargo.lock` | `ff13d80bd64b88eddb800154454708dd1bd3d37d28210cea9635f5e6719e4e16` |

Ortam: `rustc 1.96.0 (ac68faa20 2026-05-25)` · çekirdek `7.0.12-arch1-1` · `tc utility, iproute2-7.2.0, libbpf 1.7.0` · Python 3.14.7 ·
sysctl: tcp_congestion_control=cubic, tcp_retries2=15, tcp_syn_retries=6,
tcp_rmem="4096 131072 33554432", tcp_wmem="4096 16384 4194304" · netem limit 1000 (varsayılan).
