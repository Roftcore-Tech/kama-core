# KAPI-1 — v2 Adverse-Network Benchmark Kampanyası: Tasarım Tabloları

**Statü:** TASLAK → Abkarada onayı bekliyor. Otorite: `docs/tasks/v2-benchmark-gorev-r5-kosuya-hazir.md` (r5).
Onay akışı (r5 §A): bu taslağın onayı **calibration** koşularını açar; FINAL onay + KAPI-2 ön-kayıt
**evaluation/headline** koşularını açar. Bu belge kampanyanın TEK tasarım belgesidir (bitiş-dönemi
süreç kuralı 1); tek karşıt inceleme = Abkarada kapı incelemesi.

**ADIM-0 sonucu (2026-08-19):** `origin/main`'de dalda olmayan commit **0** — spec otoritesi ağaçtaki
güncel hal. Kampanyayı etkileyen wire/cert/INIT boyut değişikliği YOK: DÜZLEM-A established-session
(handshake byte'ları zaten bütçe-dışı, r5 D.2); DÜZLEM-B ertelendi (§3.6). ADIM-1 rebrand: `ea82a603`.

**Kaynak-doğrulanmış gerçekler (bu turda elle okundu):**
- Kama FEC = `crates/kama-fec-codec` (`RsSimdCodec`/`RsSimdEngine`), alt-lib `reed-solomon-simd 3.1.0`
  (Cargo.lock pin). Profil kataloğu `profile.rs::PROFILES` (0x01–0x06). INV-FEC-1 sealed
  DATA-before-PARITY per scheduler-turn (`sender.rs`, FS-CAL-2 A3.2 pinleri).
- FEC actuation: `FecActuationMode::{Controller, Manual{enabled}}`, default `Manual{false}`.
  Q18 kalibrasyon kusuru (bankada, pinli) **Controller modunu bu kampanyada yasaklıyor**.
- Tail: `TailRecoveryConfig::validated_v1()` = KAMA-TAIL-V1 (31287614, pin testli). Default OFF.
- Bench harness `crates/bench`: quinn 0.11.11 (Cargo.lock) + Kama adaptörü HAZIR (security-stub,
  DATA 11B header + 16B AEAD-tag-stub; FEC parity emit ETMEZ). quinn adaptörü mesaj-başı `open_bi`
  (= Q-STREAM). Workload şu an yalnız burst. → İş listesi §11.
- netem profilleri p1..p7: `harness/netem-profiles.sh` (+ p6 dinamik `run-bench.sh`); `pin_neigh`
  PERMANENT ön-koşul (r5 J).

---

## 0. Kollar ve FAZ yapısı (kampanya iskeleti — onay konusu D4)

| kol | tanım | durum |
|---|---|---|
| KAMA-ARQ | `Sender::new()` defaults; tail OFF, FEC Manual{false}, startup-pacing ON | adaptör hazır |
| KAMA-TAIL-V1 | + `set_tail_recovery_config(TailRecoveryConfig::validated_v1())` | W4 (küçük bayrak) |
| Q-STREAM | quinn 0.11.11, mesaj-başı `open_bi` (HoL yok; stream-limit raporlanır) | hazır |
| Q-DGRAM-ARQ | quinn DATAGRAM + app-ARQ (FEC'siz semantik-eşlenik) | W1 (yeni adaptör) |
| KAMA-FEC-S *(FAZ-2)* | KAMA-TAIL-V1 + Manual{true}, static profil+r ön-kayıtlı | W9 |
| Q-FEC-ISOLATION *(FAZ-2)* | quinn DATAGRAM + **`kama-fec-codec` aynen** (§1) | W9 |
| Q-FEC-REFERENCE *(FAZ-2)* | Yol-A/B kararına göre (§1.4) | W10 |

**FAZ-1 (önce koşulur):** FEC hiçbir kolda yok → "Kama'nın FEC avantajını QUIC-mimarisine yanlış
atfetme" riski YAPISAL olarak sıfır. İddia tavanı: *recovery/transport-entegrasyon* dili;
"QUIC mimarisine üstünlük" DENMEZ (r5 C.3 zaten FEC-baseline'sız yasaklıyor — yapılmıyor).
FAZ-1 tek başına r5 araştırma sorusunu cevaplar ve zehirli v1 sayılarını resmen değiştirir.
**FAZ-2 (FEC kolları):** conjunctive kural (r5 C.2) ancak burada devreye girer; mimari/entegrasyon
yorumu ancak ISOLATION+REFERENCE aynı yönde geçerse. Gerekçe: Controller yasak (Q18) → static-r
Manual tek savunulabilir Kama-FEC konfigürasyonu; FEC adaptör+doğrulama-kapıları en uzun iş kalemi;
fazlar aynı ön-kayıt çerçevesinde ayrı koşu-partileri olarak kaydedilir.

---

## 1. FEC (r5 KAPI-1 madde 1)

### 1.1 ISOLATION aynı-implementation eşlemesi

| alan | Kama | Q-FEC-ISOLATION |
|---|---|---|
| FEC crate | `kama-fec-codec` (workspace path) | **aynı crate, aynı workspace commit** |
| alt-kütüphane | reed-solomon-simd 3.1.0 (lock) | aynı (tek Cargo.lock) |
| symbol size E | profil kataloğundan (64 veya 1024) | aynı profil sabiti |
| block construction | K_shape source → R repair; sealed DATA-before-PARITY per turn | aynı fonksiyonlar (`encode`), aynı sealing kuralı wrapper'da |
| source/repair mapping | ESI ∈ [0,K) source, [0,R_max) repair | aynı ESI şeması |
| decode trigger | authenticate-before-decode; k_actual yeterli shard'da decode | aynı (`decode` çağrısı aynı eşik) |
| parametre tablosu | ön-kayıtlı profil (0x03: 8+4, E=64 **veya** 0x05: 8+4, E=1024) + static r | birebir aynı sabitler |

### 1.2 Deviation tablosu (yalnız entegrasyon zarfı; tablo-dışı sapma YASAK)

| field | Kama value | Quinn value | why equality impossible | expected methodological effect |
|---|---|---|---|---|
| taşıma çerçevesi | bench DATA: 11B header + 16B tag-stub | QUIC DATAGRAM frame + QUIC paket zarfı + AEAD | iki ayrı protokol zarfı | sabit per-paket byte farkı; IP-düzeyi bütçede görünür, decomposition'da raporlanır |
| shard→datagram eşleme | 1 shard = 1 UDP datagram | 1 shard = 1 DATAGRAM frame (coalescing OFF) | quinn frame'i QUIC paketine sarar | coalescing kapalı → 1:1 korunur; kalan fark zarf-boyu |
| ACK/teslim sinyali | Kama SACK (transport) | app-ACK datagramı (§8 simetri kuralı) | quinn DATAGRAM'da teslim-bildirimi yok | ACK byte'ları iki tarafta eş-politika; bütçeye dahil |
| kayıp-tespiti/ARQ | Kama ARQ (packet-threshold + tail-v1) | app-ARQ (W1 ile aynı kod-yolu) | transport-iç mekanizma vs app | ISOLATION'ın izole ettiği şey TAM BU entegrasyon farkı |

### 1.3 Bağımsız doğrulama kapıları (r5 E — FAZ-2 koşu ön-şartı)

| kapı | test |
|---|---|
| known-answer vectors | `kama-fec-codec` golden'ları (mevcut) + wrapper üstünden aynı vektörler |
| deterministic erasure | k−1/k/k+r kombinasyonları sabit seed'le, iki entegrasyonda byte-eş çıktı |
| decode-boundary | k−1 → fail, k → tam, k+r → tam (üçü de pinli) |
| property/fuzz | rastgele erasure-mask × profil; decode==source |
| çapraz-doğrulama | bağımsız lib (`reed-solomon-erasure`) ile aynı-girdi aynı-çıktı known-answer |
| loss-free regression | %0 kayıpta FEC'li kol ≡ FEC'siz + parity-byte'ları (fazlası yok) |
| wire-budget accounting | parity byte'ları governor sayacında; pcap ↔ iç-trace mutabakat |
| deadline/ARQ-termination | D'de kesilen koşuda FEC state sızıntısı yok; kalan mesaj fail sayılır |

Baseline bug'ı bulunursa etkilenen evaluation partisi yeni commit'le BAŞTAN (cherry-pick yasak).

### 1.4 Q-FEC-REFERENCE: Yol-A vs Yol-B (karar D1)

| kriter | Yol-A: olgun Rust lib (öneri: `raptorq`, RFC 6330) | Yol-B: literatür şeması birebir |
|---|---|---|
| iş-günü | ~3–4 (DATAGRAM sarmalı + kapılar) | ~8–12 (implementasyon + doğrulama) |
| dış-savunulabilirlik | yüksek: bağımsız, olgun, RFC-atfli | düşük: "bizim yazdığımız baseline" itirazı geri gelir |
| eşdeğerlik-zarfı | farklı kod ailesi (fountain) — REFERENCE'ta fark KABUL (r5 C.3) | şemaya bağlı |
| bakım | crates.io pin | bizde |

**Öneri: Yol-A (`raptorq` pin).** ISOLATION'da seçim yok (Kama-eş zorunlu, §1.1).
İş-yükü duvar-saatine §10'da eklendi.

---

## 2. Bütçe (r5 KAPI-1 madde 2)

### 2.1 permitted_utilization (karar D2)

| u | operasyonel anlam | koşu süresi etkisi |
|---|---|---|
| 0.50 | kanalın yarısı rezerv (eş-zamanlı başka trafik varsayımı) — en muhafazakâr | yok (D sabit) |
| **0.75 (öneri)** | tek-uygulama taktik link, koruma payıyla | yok |
| 0.90 | saturasyona yakın; pacing/queue etkileri bütçe sınırına karışır | yok |

Öneri: primary **u=0.75**; {0.50, 0.90} preregistered sensitivity. Koşu süresi D'ye bağlı olduğundan
seçim süreyi değiştirmez; anlam farkı operasyoneldir.

### 2.2 Bütçe biçimleri
- **burst:** `B = C × D × u / 8` one-shot kümülatif hard-cap (yön-başına, kanal-türevli).
- **paced:** `refill = C×u/8 B/s` · `bucket = max(3×path_mtu, 2 s × refill)` (2 s burst-allowance —
  öneri D6; KAPI-2'de pin) · `ceiling = C × W × u / 8` (W = evaluation penceresi).
- **Yön-aktarımı: YOK.** Full-duplex profillerde `B_fwd`/`B_rev` ayrı; kullanılmayan reverse bütçesi
  forward'a aktarılmaz — asimetrik profilleri (p5, h4a) anlamlı kılar. Half/shared profillerde tek
  `B_shared` (Grup-C duplex_model alanına göre).

### 2.3 Packet-atomic governor (W2)
Tek implementasyon iki transport: bench-crate `BudgetGovernor`.
- Kama: adaptörün `send_to` öncesi; quinn: `AsyncUdpSocket` sarmalayıcısı (quinn 0.11 custom-socket API).
- Kural: `B_remaining < ip_len(datagram)` → enjeksiyon YOK (`ip_len = udp_payload + 28`, IPv4).
  Partial-packet/overshoot yok. Governor **asla geciktirmez ve kuyruk tutmaz** (pacing-nötr):
  burst'ta tükeniş kalıcı; paced'te o anki ret sayaca yazılır (`budget_refused_{pkts,bytes}`),
  paket enjekte edilmez — transportun kendi kurtarması kanal-silinmesiyle eşdeğer görür. Bu semantik
  ön-kayda aynen girer; iki transporta özdeş.
- Her hücrede `delivered_unique_payload vs cumulative_injected_ip_bytes` eğrisi (governor sayacından).

### 2.4 Deadline↔byte ortogonalizasyonu (r5 D.1b)
Primary endpoint B(D) bileşiğini olduğu gibi değerlendirir (nedensel ayrım İDDİA EDİLMEZ). İkincil
ZORUNLU eksenler: `fixed-D=120s, B∈{0.5,0.75,0.90}×C·D/8` ve `fixed-B=B(120s,0.75), D∈{60,120,300}`
— yalnız primary ailede (§10 bütçesine dahil).

---

## 3. Düzlem (r5 KAPI-1 madde 3)

### 3.1 A1 barrier protokolü (W6)
1. Session temiz kanalda kurulur (netem yok); quinn handshake + Kama converge (PostConverge'e giriş
   `diag` ile doğrulanır).
2. Uygulama trafiği DURUR; quiescence: in-flight=0 VE son ACK'ten ≥2×RTT sessizlik.
3. ns-mid'e impairment yüklenir; **aktivasyon barrier'ı:** `tc -s qdisc` kural-doğrulaması + ölçüm-dışı
   probe akışının realized-etki göstermesi (ayrı 5-tuple; bütçe/pcap düzlemlerinden dışlanır, portu
   manifestte).
4. Transport-state reset YOK, warm-up YOK.
5. Ortak barrier (harness FIFO senkronu) t=0'da mesajları serbest bırakır. t=0 damgası tek saat (§7).

### 3.2 A2 warm-up kirlenme kuralları (W6)
- warmup_duration = max(10×base_RTT, 10 s); warmup trafiği = aynı sınıf mesaj akışı, ölçüm-dışı.
- Warm-up byte'ları bütçeye GİRMEZ (governor t=0'da kurulur); t=0'da RunOutput sayaçları sıfırlanır
  AMA congestion/estimator/pacer state KORUNUR; warm-up'tan in-flight kalmaz (quiescence §3.1-2 ile
  aynı kural); evaluation-trace başlangıcı iki transport için AYNI (ortak barrier).

### 3.3 14 established-state alanı

| alan | KAMA kolları | quinn kolları |
|---|---|---|
| session_establishment_network | temiz veth (netem yok) | aynı |
| impairment_activation_time | A1: t=−barrier; A2: warm-up başı | aynı |
| warmup_duration | A1: 0; A2: max(10×RTT, 10 s) | aynı |
| warmup_traffic | A2: aynı-sınıf mesaj akışı, ölçüm-dışı | aynı |
| congestion_state_reset | YOK | YOK |
| bandwidth_estimator_reset | YOK (SRC/BtlBw state korunur) | YOK (BBR/cubic state korunur) |
| initial_cwnd | motor §13 floor-pacing başlangıcı (converge sonrası geçerli değil) | quinn default (KAPI-2'de sayısal pin) |
| initial_rtt | ölçülmüş (converge'de) | ölçülmüş (handshake'te) |
| prior_path_history | yalnız temiz-kanal geçmişi | aynı |
| warmup_quiescence_condition | in-flight=0 ∧ ≥2×RTT sessizlik | aynı |
| inflight_at_t0 | 0 | 0 |
| counter_reset_without_state_reset | RunOutput sayaçları t=0 | conn-stats offset t=0 |
| warmup_trace_id | manifest'te ayrı trace-id | aynı |
| evaluation_trace_start_offset | 0 (t=0 ortak barrier) | aynı |

**Kama-özel (A1 estimator-şok):** t=0'da Sender PostConverge'dedir (temiz ağda converge);
PRE_CONVERGE devrede DEĞİL; fate-estimator penceresi temiz-kanal gözlemleriyle doludur (yapı sinyali
gecikmeli oturur — bu ürün-gerçeği raporlanır, maskelenmez); restore_burst/bw-lock başlangıç durumu
repeat-manifestine diag'dan yazılır.

### 3.4 Headline düzlemi (karar D3)
**Öneri: A2 primary.** Gerekçe: r5 dış araştırma sorusu "retains useful message-delivery capability"
= sürdürülen rejim; A2'nin kirlenme kuralları tartışmaya daha kapalı; A1 geçiş-şoku ayrı meşru soru
olarak İKİNCİL raporlanır (her iki düzlem de koşulur, r5 B.2).

### 3.5 Reconnect
DÜZLEM-A'da otomatik reconnect YOK; oturum ölümü → kalan mesajlar deadline'da FAIL +
termination-nedeni (quinn: idle/path-validation hatası; Kama: motor durmaz, ölüm kavramı yok —
tablo dipnotu). DÜZLEM-C bu kampanyada AÇILMAZ (öneri).

### 3.6 DÜZLEM-B (h5 modeled cold-start)
FAZ-2 sonrasına ayrı mini-parti olarak ertelenir (öneri, karar D5): headline'a kaynak olamaz (r5 B.1),
kritik yolda değil; koşulduğunda r5'in aynen-caveat cümlesiyle.

---

## 4. Capture (r5 KAPI-1 madde 4)

### 4.1 Üç-namespace topolojisi (W5)
```
ns-a (endpoint A) ──veth── ns-mid (netem fwd + rev) ──veth── ns-b (endpoint B)
```
netem YALNIZ ns-mid'in iki egress'inde. Linux packet-tap egress'te qdisc-SONRASI çalıştığından,
endpoint-egress capture'ı yapısal olarak pre-impairment yapmanın doğru yolu impairment'ı ORTA
namespace'e taşımaktır; iki-netns faithful harness bu kampanyada üçe genişletilir. `pin_neigh`
her veth çiftinde (ARP-drop ayrı doğrulama, r5 J ön-koşulu).

### 4.2 Üç düzlem — tek capture noktası/düzlem (çift-sayım-yok kanıtı)
| düzlem | nokta | rol |
|---|---|---|
| `sender_injected_ip_bytes` | ns-a/ns-b kendi egress pcap'i (pre-impairment) | **primary bütçe; otoriter toplam** |
| `bottleneck_admitted_ip_bytes` | ns-mid egress pcap (netem sonrası) | tanısal |
| `receiver_observed_ip_bytes` | karşı-uç ingress pcap | çapraz-teyit (`A.tx≈B.rx+drops`) |

Her paket her düzlemde tek interface'te bir kez görülür (forward/reverse ayrı interface); uç-sayaçları
toplam formülüne girmez. Üç byte-seviyesi: udp_payload / ip_wire / l2_observed; dış-terim
"IP-layer observed bytes". Rapor cümlesi (aynen): "The primary byte budget measures IP bytes injected
by the endpoints into the emulated channel, including transmissions subsequently erased by the
impairment model; it is not a measurement of physical RF airtime."

### 4.3 Offload + decomposition
İki uçta `ethtool -K <veth> gro off gso off tso off lro off` + quinn
`enable_segmentation_offload(false)` (komutlar repro-blokta). Decomposition öncelik-kuralı r5 H aynen
(1 handshake/control … 7 unattributed); otoriter-toplam=pcap; semantik=Kama iç-trace
(`TxComponents`) + quinn qlog; reconcile farkı >%1 → açıklama zorunlu.

---

## 5. İstatistik (r5 KAPI-1 madde 5)

### 5.1 Primary-endpoint hiyerarşisi
| alan | değer |
|---|---|
| primary_endpoint | `E(B,D)` = eşit kanal-türevli bütçede, deadline-içi teslim edilen unique-payload-byte ORANI |
| primary_profile_family | Grup-A çekirdek: {p2_loss80, p3_loss95, p5_revpath} (quinn'in ölçülü üç çöküş rejimi — banka #1) |
| primary_workload | paced (survival-messaging; ürün tezi) |
| primary_plane | A2 (öneri, D3) |
| primary_deadline | 120 s (v1 `c120` ile karşılaştırılabilirlik) |
| primary_comparator | FAZ-1: Q-DGRAM-ARQ (semantik-eşlenik). FAZ-2: {Q-FEC-ISOLATION, Q-FEC-REFERENCE} dual-mandatory, conjunctive kural r5 C.2 aynen |
| primary Kama kolu | KAMA-TAIL-V1 (KAMA-ARQ da her hücrede raporlanır — el-devri kuralı) |

Multiplicity: primary = 3 profil × 1 endpoint × 1 karşılaştırma → **Holm (3 test)**. Kalan her şey
"unadjusted exploratory" etiketli.

### 5.2 Tasarım
- **Paired:** aynı seed/trace iki kola; Δsuccess (McNemar / paired-bootstrap), paired RMST-diff,
  paired Δwire-efficiency, paired deadline-delivery-diff. Headline formu: "aynı N trace'te teslim
  farkı +X puan, %95 paired CI [a,b]".
- **Cluster:** n = repeat sayısı; bootstrap repeat-cluster üstünden (mesaj-bazlı örneklem YASAK).
- **Sıfır-teslim-güvenli maliyet:** primary maliyet = `delivered_unique_payload / total_injected_ip_bytes`
  (0-teslimde 0, tanımlı). `bytes/delivered_msg` yalnız iki-taraf-≥1-teslim hücrelerinde ikincil.
- **Güç:** calibration (n=10) discordant-pair-rate → paired-power: hedef %80, min-relevant-effect
  +15pp, α=0.05 (Holm-öncesi). Headline n=30 taban; power hesabı n>30 isterse dış-iddia-kritik
  hücrelerde (p2/p3 paced A2) n=50'ye çıkılır — hesap KAPI-2'ye yazılır.
- **Randomizasyon:** 10-repeat bloklarında 5/5 balanced Kama-önce/Quinn-önce; sıra manifest-seed'inden.

---

## 6. Kanal (r5 KAPI-1 madde 6)

- **Statik profiller:** netem rate/delay/loss (netem iç RNG'si seed'lenemez → per-paket random-loss
  profillerinde trace-eşitliği YOK; adalet paired-seed + realized-log + endogenous kuralıyla sağlanır).
  **Zaman-tabanlı profiller** (p6, h4p, h3 burst-pencereleri): önceden-üretilmiş zaman-pencere planı
  (JSON, commit'li) → ns-mid uygulayıcısı (W7); AYNI plan iki transporta = trace-replay.
  h3a/b/c gemodel: `loss gemodel p,r,1−h,1−k` TAM 4 parametre manifestte.
- **Kör-ayrım Model A (r5 J aynen):** (1) trace-generator sürümü + dağılım-parametreleri commit →
  (2) calibration-set ile ayar → (3) TÜM transport-configleri commit-kilit → (4) evaluation-seed'leri
  **public-randomness'tan** üretilir: drand mainnet, ön-kayıtta ilan edilen GELECEK round R;
  `seed(cell, rep) = SHA-256(drand(R).randomness ‖ "KAMA-V2" ‖ cell_id ‖ rep)`. Calibration seed
  uzayı ayrı namespace; evaluation'da kullanımı YASAK.
- **Realized-channel log (repeat-başına JSON):** configured{rate,loss,delay,jitter} vs
  realized{loss_pct, byte_loss_pct, delay p50/p95/p99, netem_drops, qdisc_drops, transport_drops,
  arp_layer_drops(=0 zorunlu, pin_neigh doğrulaması)}. Sub-kbps hücrelerde realized ŞART.
- **Exogenous eşik (invalidation yalnız bununla):** ARP-drop>0 ∨ netem kural-doğrulama uyuşmazlığı ∨
  |realized−configured| kanal-drop sapması kalibre-edilen toleransın dışı (tolerans calibration'dan
  KAPI-2'ye pinlenir). İki transportun realized-loss FARKI (endogenous) invalidation DEĞİL.

---

## 7. Saat (r5 KAPI-1 madde 7)

| alan | değer |
|---|---|
| clock_source | CLOCK_MONOTONIC, tek host (netns'ler aynı kernel saati) |
| clock_sync_method | N/A — paylaşımlı saat (tercih sırası r5 K'nin 1.'si) |
| max_allowed_offset | 0 (yapısal) |
| max_allowed_drift | 0 (yapısal) |
| timestamp_resolution | ns API; efektif ~µs (user-space damga) |
| timestamp_location | harness user-space: `sent_ns` = APP-OFFER anı, `delivered_ns` = ilk kapsayan teslim-bildirimi — iki transport aynı çerçeve (`RawEvent`) |

**h6 CPU-izolasyon planı:** FAZ-1'de CPU/throughput üstünlük-İDDİASI YOK (Kama AEAD modeled-stub →
r5 L yasağı). Yine de izolasyon uygulanır: endpoint'ler ayrı fiziksel core'lara `taskset`, governor
`performance`, IRQ-affinity ayrık, background-load kontrol; "aynı-host-veth, bağımsız-fiziksel-host
değil" caveat'i rapora aynen. CPU-zamanı + core-util her iki transport için ölçülüp raporlanır
(iddiasız, tanısal).

---

## 8. Semantik + packetization (r5 KAPI-1 madde 8)

### 8.1 Hizmet-semantiği (her hücre raporunda zorunlu tablo)
| boyut | KAMA | Q-STREAM | Q-DGRAM-ARQ / Q-FEC-* |
|---|---|---|---|
| güvenilirlik | ARQ (+FAZ-2 FEC); bütçe/deadline sınırlı | QUIC reliable | app-ARQ (+FEC); aynı sınır |
| sıralama | yok (unit bağımsız) | stream-içi FIFO (mesaj-başı-stream → mesajlar-arası HoL yok) | yok |
| mesaj-sınırları | unit = mesaj | stream = mesaj | datagram = mesaj (≤ zarf; §8.3) |
| deadline-sonrası-iptal | harness D'de keser; geç teslim stale sayılır | aynı | aynı |
| dup-suppression | receiver unit_id dedup | QUIC | app msg_id dedup |
| app-ACK | transport SACK'ten (bench ACK frame) | stream-FIN teyidi | app-ACK datagramı |
| connection-state | yok (motor durmaz) | QUIC conn (idle-timeout pin KAPI-2) | aynı |

ACK alanları: Kama — `transport_ack_unit`=SACK range (FRAME_TYPE_ACK_V2), generation = N-veri-paketi
eşiği + 20 ms timer (`ACK_TICK`, kama.rs; N değeri KAPI-2'de koddan pin), coalescing = SACK range
birleştirme, frequency = eşik/timer bileşimi. quinn — RFC 9000 (her-2-paket + max_ack_delay 25 ms
default; sayısal pin KAPI-2). **App-ACK simetrisi (r5 L):** Q-DGRAM app-ACK'i boyut/aggregation/
delay/retx-politikasında Kama ACK'iyle eşlenik tasarlanır (W1'in tasarım gereksinimi).

### 8.2 MTU/IP alanları (özdeş zarf)
| alan | değer (iki transport) |
|---|---|
| ip_version | IPv4 |
| path_mtu | 1500 (veth; değiştirilmez) |
| max_udp_payload | 1452'ye pin (quinn: transport-config; Kama: mesaj-boyu zaten altında) |
| fragmentation_policy | YOK — mesaj tek datagrama sığar (mesaj planı §9.2 bunu garanti eder) |
| pmtud_policy | kapalı/sabit (quinn MTU discovery off; pin KAPI-2) |
| minimum_frame_size | quinn Initial 1200B-padding gerçeği nötr raporlanır (established'da nadir) |
| message_coalescing_policy | kapalı (GSO off; quinn datagram-coalescing kapalı) |

---

## 9. Profiller (r5 KAPI-1 madde 9)

Tam şema her profil için manifest-üreticide (W8) makine-okur biçimde doldurulur; tablolar özet.
Ortak: ip_version=IPv4 · path_mtu=1500 · qdisc_order = netem(delay,loss)→rate (tek netem kuralı) ·
queue: netem limit 1000 pkt (finite bottleneck; cross-traffic yok) · fwd/rev_RNG independent ·
delay_distribution normal (belirtilmedikçe) · correlation 0 (belirtilmedikçe).

### 9.1 Grup A (v1 mirası, `netem-profiles.sh` değerleriyle)
| id | fwd | rev | delay | duplex |
|---|---|---|---|---|
| p1_loss50 | loss 50% · 100kbit | aynı | — | full |
| p2_loss80 | loss 80% · 200kbit | aynı | — | full |
| p3_loss95 | loss 95% · 100kbit | aynı | — | full |
| p4_jitter | 100kbit · delay 200±100ms normal | aynı | 200±100ms | full |
| p5_revpath | loss 5% · 100kbit | **loss 80%** · 100kbit | — | full |
| p6_blackout | dinamik ON/OFF (zaman-pencere planı, W7) | aynı | — | full |
| p7_bbr | loss 2% · 50kbit | aynı | — | full |

### 9.2 Grup B (sentetik saha-motivasyonlu; dipnot r5 F aynen) + Grup C
| id | parametreler (r5 F aynen) | sınıf |
|---|---|---|
| h1 | 0.20kbps · loss 40% · one-way 1200ms · jitter 400ms normal | III |
| h2 | 8kbps · loss 25% · jitter 300±200ms (finite bottleneck queue) | II |
| h3a/b/c | 250kbit · gemodel, loss {40,50,60} (4-param manifestte) | I |
| h4p | %100-erasure 1s-ON/2s-OFF, üç faz {on,off,uniform_random}-start (trace, W7) | II |
| h4s | 100kbit · loss 85% | II |
| h4a | fwd loss 10% / rev loss 90% | II |
| h6 | 10Mbit · loss 1% · RTT {20,200}ms | I |
| h5 | **DÜZLEM-B — ertelendi (D5)** | — |
| p8/p9/p10 | {1, 0.5, 0.25}kbps × loss {0, 20, 40}% (9 hücre; duplex_model=full — netem çift-yön-ayrı; half-duplex telsiz temsil edilmiyor, rapora yazılır) | III |

### 9.3 Mesaj planları + deadline setleri (kapasite doğrulamalı)
| sınıf | plan | paced aralık | D-seti | kapasite kontrolü (en kötü hücre) |
|---|---|---|---|---|
| I (≥50kbit) | 100×256B | 1 msg/s | {30,60,120,300} | 25.6kB payload ≪ 50kbit×120s=750kB ✓ |
| II (1–50kbit) | 30×64B | 1 msg/5s | {30,60,120,300} | 1.9kB ≪ 8kbit×120s=120kB ✓ (h4s efektif %15: 1.9kB < 100kbit×0.15×120s ✓) |
| III (<1kbit) | 10×32B | 1 msg/30s | {120,300,600} | wire ~10×(32+39+16)B ≈ 7kbit; 0.25kbps×600s=18.75kB ✓; 0.20kbps(h1)×600s=15kB ✓ |

Serialization-lower-bound yalnız analiz metriği (`normalized_completion`); başarı eşiği DEĞİL.

---

## 10. Duvar-saati bütçesi (r5 KAPI-1 madde 10)

Tek makine, hücreler SERİ (CPU-izolasyonu; paralel hücre timing kirletir). Repeat süresi ≈ D + kurulum
~15 s; paced repeat ≈ son-submission + D.

| parti | içerik | kaba hesap | süre |
|---|---|---|---|
| Calibration (taslak-onay sonrası) | primary aile 3 profil × 4 kol × 2 workload × n=10, D=120 | 240 rep × ~200 s | ~13 saat (1 gece) |
| FAZ-1 headline | 3 profil × 4 kol × paced × A2 × n=30 (p2/p3 n=50) | ~440 rep × ~250 s | ~30 saat (2 gece) |
| FAZ-1 sekonder | aynı aile: burst×A2 n=10 · paced×A1 n=10 · fixed-D/fixed-B eksenleri n=10 | ~600 rep × ~220 s | ~37 saat (2–3 gece) |
| FAZ-1 exploratory | Grup-B (h1..h6, 10 hücre) + Grup-C (9) × 4 kol × paced × A2 × n=10 | ~760 rep × ~300–900 s (Sınıf-III uzun) | ~4 gece |
| FAZ-2 implementasyon | W9+W10 + §1.3 kapıları | — | ~5–7 iş-günü |
| FAZ-2 koşu | primary aile + seçili Grup-B, FEC kolları (3 kol) n=30/10 | ~500 rep | ~2–3 gece |

**Toplam: FAZ-1 ≈ 1 hafta (implementasyon W1–W8 ~5–6 iş-günü + ~8 gece koşu, gece koşuları
implementasyonla örtüşür); FAZ-2 ≈ +1.5 hafta.** Rapor+white-paper R3'te.

---

## 11. İş listesi (KAPI-1 "tasarım+implementasyon GO" kapsamı — koşu değil)

| iş | içerik | tahmin |
|---|---|---|
| W1 | Q-DGRAM-ARQ adaptörü (quinn DATAGRAM + app-ARQ + eşlenik app-ACK) | 1 gün |
| W2 | `BudgetGovernor` (packet-atomic; kama send-önü + quinn AsyncUdpSocket sarmalı) | 1 gün |
| W3 | paced workload + per-mesaj deadline metrikleri (r5 G formülleri) | 0.5 gün |
| W4 | KAMA-TAIL-V1 kol bayrağı (`validated_v1()` çağrısı) | 0.1 gün |
| W5 | 3-netns topolojisi + üç-düzlem pcap + realized-log + pin_neigh | 1.5 gün |
| W6 | A1 barrier + A2 warm-up düzeneği | 1 gün |
| W7 | zaman-pencere trace uygulayıcı (p6/h4p/h3-burst) | 0.5 gün |
| W8 | profil-manifest üretici + drand seed türetici + balanced-randomizasyon | 0.5 gün |
| W9 *(FAZ-2)* | bench FEC yolları: KAMA-FEC-S emisyonu + Q-FEC-ISOLATION sarmalı + §1.3 kapıları | 3 gün |
| W10 *(FAZ-2)* | Q-FEC-REFERENCE (Yol-A: raptorq) + kapılar | 3–4 gün |

---

## 12. Abkarada karar noktaları (onayla birlikte)

| # | karar | öneri |
|---|---|---|
| D1 | Q-FEC-REFERENCE yolu | Yol-A (`raptorq` RFC 6330 pin) |
| D2 | permitted_utilization primary | 0.75 |
| D3 | headline düzlemi | A2 (A1 ikincil-raporlu) |
| D4 | FAZ-1/FAZ-2 parti yapısı | onay (FAZ-1 önce; mimari-yorum ancak FAZ-2 conjunctive ile) |
| D5 | DÜZLEM-B (h5) | FAZ-2 sonrası ayrı mini-parti |
| D6 | paced bucket allowance | 2 s × refill (min 3×MTU) |

**Bilinen zayıflıklar (dürüstlük kaydı):** (1) netem per-paket loss seed'lenemez → o profillerde
trace-eşitliği yerine paired-seed+realized-log (r5 J endogenous kuralı bunu kapsıyor). (2) Kama
security-stub (+16B tag) — byte-eş modelleme, kripto-gerçekliği değil; r5 L gereği CPU-iddiası yok.
(3) Q-DGRAM-ARQ'nun app-ARQ'su bizim yazdığımız baseline — ARQ mantığı basit-tutulup (packet-threshold
+ PTO, RFC 9002 izinde) ön-kayıtta aynen dökümante edilir; "knowingly disadvantaging" yasağına tabi.
