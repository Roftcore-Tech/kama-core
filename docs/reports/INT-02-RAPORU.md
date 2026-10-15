# INT-02 RAPORU — uçtan uca mesaj: Application + Transport + Crypto, gerçek UDP, netem

| Alan | Değer |
|---|---|
| Kart | K4G-05 (L-INT) — `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` v1.1 §3 + Ek 2026-10-11 (3), Ek 2026-10-12 (4), Ek 2026-10-12 (5) |
| Kapsam belgesi | `docs/decisions/PoC-KAPSAM-001.md` (§5 açıklar, K1–K10, EK-2, EK-3) |
| Durum | **İSKELET** — koşu yapılmadı; sayı hücreleri `…`. Koşudan sonra `scripts/netem/int02_analyze.py` çıktısıyla doldurulur. |
| Ön koşullar | "G-IMPL-S3 GEÇTİ" (Abkarada, 2026-10-13) · T5: `sch_netem` yüklü (2026-10-15); orkestratör ölçümü: `unshare -rn` ad alanında `lo` üzerine `netem delay 100ms loss 30% seed 7` → ping RTT ≈ 200 ms, gidiş-dönüş kaybı 40/100 (n = 40) — `lo` netem'i her paketi HER YÖNDE bir kez görür |
| Koşu | tarih: … · commit: … · makine / çekirdek: … · iproute2: 7.2.0 |
| Düzenek | `crates/kama-integration/examples/int02_node.rs` · `crates/kama-integration/tests/int02_loopback.rs` · `scripts/netem/int02_run.sh` · `scripts/netem/int02_analyze.py` |
| Ham çıktı | `results/int02/<tarih>/seed-<S>-<kol>.json` (+ `.err`; `results/` gitignore'da) |

---

## 0. Özet (koşudan sonra doldurulur)

| Ölçüt | Sonuç |
|---|---|
| GATING (1) — iki yönde `delay 200ms 200ms loss 30%`, 100 TEXT + 100 RECEIPT | … / 5 tohum |
| GATING (2) — akış sırasında epoch 0→1, kayıp yok, tek oturum | … / 5 tohum |
| GATING (6) — eşzamanlı connect (glare) | … / 5 tohum |
| Application retransmit = 0 · Crypto retransmit = 0 | … |
| `ctx_mismatch_drops` · `hub_frames_dropped` · `delivery_channel_drops` = 0 | … |
| T4 — sabit tohum, 5 tohum, 5/5 | … |
| recovery OFF negatif kolu — ölçüm kaybı görüyor | … / 5 tohum |
| NON-GATING: dosya + PTT · 60 s kesinti | … |
| Yeniden koşu (CEVAP PAKETİ 4, §10) — GATING (2), 10 tohum × pencere 600 s | … / 10 tohum |
| Yeniden koşu (§10) — ek-kesinti: 60 s kesinti, aynı oturumda %100 metin teslimi | … / 5 tohum |
| Yeniden koşu (§10) — ek-dosya-ptt (D3): dosya sonucu ölçülür ve raporlanır | … |

Bu tablo ölçüm özetidir; kapı ifadesi değildir (son satıra bakın).

---

## 1. Kurulum

### 1.1 Düğümler, anahtarlar, süreç

* **Tek süreç**, iki ürün düğümü: `kama_node::KamaNode::start` (bileşim kökü, K4G-11) — gerçek
  `ClientTransport` (ürün profili `ClientTransportConfig::product`, `KamaV1Profile`), gerçek
  `NoiseWireProtection::with_control` (22 B legacy SESSION_DATA zarfı), `kama-app` Messenger / Ptt /
  FileTransfer / Signaling motorları, `kama-store::RedbStore` (aynı nesne `AppStore` + `CounterStore`),
  OS entropisi, gerçek saatler (Crypto ve Application ayrı). Negatif kolda aynı kurulum
  `KamaNode::start_recovery_off_for_test` ile (§1.4).
* **Anahtar kaynağı: `kama-provision` çıktısı.** Her (tohum, kol) koşusu için ağ ad alanının içinde
  `kama-provision init --out <geçici>/authority --devices alice,bob --direct
  alice=127.0.0.1:40001,bob=127.0.0.1:40002`; her "telefon"a yalnız kendi `device-<ad>.json`'u
  kopyalanır, `root.json` otorite dizininde kalır (D9; sürücü denetler). Test fikstürü KULLANILMAZ.
  Duman testi (`int02_loopback.rs`) aynı `kama-provision` kütüphanesini süreç içinde çağırır.
* İki düğüm de **connect ÖNCESİ** başlatılır; glare kolu hariç bağlantıyı alice başlatır
  (`carrier.rs` başlatma sırası kuralı; §8.2).
* **El sıkışma zarfları** (SSI1/SSR1) süreç-içi röleden (`LoopbackEnvelopeRelay`, DUR-K4G-11 → B)
  **netem DIŞINDA** geçer; veri datagramları (DATA, ACK, PARAM, FEC) loopback UDP'den netem İÇİNDE geçer.
* `KamaNode::start` tokio bağlamından çağrılmaz: sürücü süreci düz iş parçacıklarıdır (plan Ek (5)).

### 1.2 Ağ ve netem profili

| Öğe | Değer |
|---|---|
| Ad alanı | her (tohum, kol) için ayrı `unshare -rn`; `ip link set lo up` |
| Profil (sahibin metni birebir) | `delay 200ms 200ms loss 30%` |
| Anlamı (netem sözdizimi) | 200 ms gecikme **± 200 ms jitter**, %30 kayıp; jitter dağılımı belirtilmedi (çekirdek varsayılanı) |
| Uygulama | `tc qdisc add dev lo root netem delay 200ms 200ms loss 30% seed <S>` — `lo`'ya **BİR kez**; `lo`'da her paket her yönde netem'den bir kez geçer (T5 ölçümü) → profil iki yönde etkin |
| `seed` | netem'in rastgele üretecini sabitler. Paket sırası ve zamanlaması koşudan koşuya değişebildiği için kayıp deseninin bit-bit tekrarı İDDİA EDİLMEZ |
| Veri portları | alice 127.0.0.1:40001 · bob 127.0.0.1:40002 (oturum uçları efemeral port) |
| Tohumlar | … (varsayılan `1 2 3 4 5`) |

### 1.3 Ölçüm aracı doğrulaması (her koşudan ÖNCE)

Sürücü, örneği başlatmadan önce aynı ad alanında `ping -c 20 -i 0.2 -W 2 127.0.0.1` koşar ve
`tc -s qdisc show dev lo` sayaçlarını koşudan önce ve sonra kaydeder (`netem_check`). Koşu, şu
koşulların hepsi sağlanmazsa **GEÇERSİZ** sayılır ve hükme girmez (`int02_analyze.py`): ping kaybı
görüldü (alınan < gönderilen), ping ortalama RTT ≥ 100 ms, koşu boyunca qdisc düşürmesi > 0. Not: ping
paketleri de netem üretecini tüketir.

| Tohum | Kol | ping alınan / gönderilen | ping ort. RTT (ms) | koşu boyunca qdisc kaybı (%) | Geçerli |
|---|---|---|---|---|---|
| … | … | … | … | … | … |

### 1.4 Kollar ve parametreler

| Kol | Akış | GATING |
|---|---|---|
| `gating1` | alice → bob 100 TEXT (100 ms aralık), bob → alice 100 RECEIPT | (1) |
| `gating2` | bob (yanıtlayan) KENDİ `Connected`'ının hemen ardından bob → alice 100 TEXT; yanıtlayanın gönderim epoch'u başlatanın ilk epoch-1 paketi gelene dek 0'dır (noise-wire §4-B3) → 0→1 geçişi akışın içinde | (2) |
| `gating6` | iki düğüm iki iş parçacığından bir engelle (`Barrier`) AYNI ANDA `connect`; sonra iki yönde 100 TEXT + RECEIPT | (6) |
| `negatif` | `gating1` akışı, iki düğümde kurtarma KAPALI | negatif |
| `ek-dosya-ptt` | `gating1` + 1 dosya (150 KiB) + 1 PTT klibi (50 demet × 40 ms, Lyra, 32 B) | NON-GATING |
| `ek-kesinti` | PTT açık; 50 TEXT + 25 demet; **60 s `loss 100%`**; kesinti SIRASINDA 50 TEXT + 25 demet + `ptt_close`; profil geri | NON-GATING |

Ortak: bağlantı üst sınırı 60 s; **pencere** (son gönderimden — ek-kesinti kolunda kesinti
bitiminden — sonra tamamlanma üst sınırı; bütün kollar için tek parametre, örnekte `--window-s`,
sürücüde `--window`) varsayılan 120 s; tamamlanmadan sonra Transport sakinleşmesi (iki düğümde
teyitsiz birim 0) için üst sınır 10 s. ~~(negatif kol 30 s)~~ Negatif kol (kurtarma kapalı)
tamamlanmaz ve pencerenin tamamını bekler; kısa pencereyle ayrı koşulur (`--window 30`).
`--outage-wait-s` yalnız ek-kesinti kolunda `KESINTI_BITTI` işaretinin beklenme üst sınırıdır
(kesinti uygulandıktan sonra, varsayılan 180 s) — pencere DEĞİLDİR.

**recovery OFF kolunun kurulumu:** ürün builder'ı `recovery: false` yapılandırmasını reddeder
(`ClientTransport::new` → `TransportBuildError::RecoveryOff`). Kol, ürün yolunun AYNISIYLA
(`KamaNode::build`, ürün profili, ürün builder'ı) kurulur; düğüm iş parçacığı başlamadan önce
Transport'un test kancası `ClientTransport::set_recovery_for_test(false)` çağrılır (S1 H-APP-RETRY-0
kancası; yalnız `test-util`): ARQ retransmit, §11 kopya ve FEC repair kararları tele çıkmaz, çalışma
zamanı onları `recovery_suppressed`'te sayar. Ürün yolu (`KamaNode::start`) ve ürün davranışı
değişmedi; `TransportCapabilities::reliable` beyanı bu kolda da `true` kalır.

### 1.5 Ölçülenler ve kaynakları

| Ölçü | Kaynak | Erişim |
|---|---|---|
| teslim % | alıcıdaki `NodeEvent::TextReceived` / gönderende `send_text`'in döndürdüğü `msg_id` (eşleme `msg_id` ile; metin içeriği de karşılaştırılır) | ürün olayları |
| alındı % | gönderendeki `NodeEvent::Receipt` | ürün olayları |
| uçtan uca gecikme medyan / 95p | `send_text` çağrısından hemen önceki `Instant` → alıcı düğümün dinleyiciyi `TextReceived` ile çağırdığı an (aynı süreç, aynı monotonik saat); yüzdelikler en yakın sıra (nearest-rank) | düzenek |
| alındı RTT | gönderim → `Receipt` | düzenek |
| epoch geçiş anı | `KamaNode::metrics().connect_state[eş].epoch` (`ClientTransport::session_epoch` ← `NoiseWireProtection::session_send_epoch`), 5 ms'de bir örneklenir | ürün (K4G-11) |
| transport ARQ yeniden gönderim | `ClientLiveDiag::retransmits` (motorun `Retransmit` sınıflı emisyon kararları, tail-probe dahil) + `tail_probes`, `tx_retx_bytes` (tele çıkan) — `ClientTransport::session_live_diags` + `retired_live_diag` | YENİ salt-okunur ayna (client-core) → `KamaNode::probe_for_test` (test-util) |
| Application retransmit (0 olmalı) | `MessengerStats::resent_on_new_session` + `resend_refused` (Application'ın tek yeniden gönderim yolu oturum değişimidir; aynı oturumda zamanlayıcı yok) | `Messenger::stats` → sonda |
| Crypto retransmit (0 olmalı) | süreç-içi röle uç noktası başına, aynı uç noktanın daha önce gönderdiği zarfla AYNI tür ve AYNI baytta el sıkışma zarfı (`EnvelopeCounts::identical_resends`; son 16 zarfa karşı) + INIT / RESPONSE sayıları. T3 (INIT retry) olmadığı için yapısal beklenti 0'dır; sayaç bunu doğrular | YENİ test-util kaydı (kama-node röle kutusu) → sonda |
| `ctx_mismatch_drops`, `hub_frames_dropped` (0 olmalı) | `ClientTransport::ctx_dispatch_stats` | ürün (`NodeMetrics`) + sonda |
| `delivery_channel_drops` (0 olmalı) | `ServerHandle::delivery_dropped` canlı aynası | ürün (`NodeMetrics`) + sonda |
| `not_ready_drops` | `CtxDispatchStats::not_ready_drops` (INV-ACK-DELIVER-1: beklenen ama `Ready` olmayan oturumun ACK'siz düşen datagramı) | sonda |
| Karn / PreConverge | `ClientLiveDiag::{rtt_samples, srtt_us, rtprop_us, left_preconverge_ns}` (oturum ucu başına) | YENİ ayna → sonda |
| tek oturum | düğümün işlediği S1 olayları (`SessionEventCounts`: `Ready`, `Closed`, `RenewRequired`, `ConnectAbandoned`) + canlı oturum sayısı + `Connected` / `Closed` düğüm olayları | sonda + ürün olayları |
| Transport sakinliği | `ClientTransport::outstanding_for_test` = 0 (iki düğüm) | sonda |
| netem doğrulama | `ping`, `tc -s qdisc` | sürücü |

Ürün kodu değişikliği YOKTUR. Eklenenler: client-core'a **salt-okunur** canlı ayna
(`ClientLiveDiag`, `ClientHandle::live_diag`, `ClientTransport::session_live_diags` /
`retired_live_diag`; görev her ana döngü turunun başında yazar, hiçbir karar okumaz —
`ServerHandle::delivery_dropped` emsali) ve kama-node'a `test-util` arkasında sonda + recovery-OFF
kurucusu + röle zarf kaydı.

### 1.6 Koşu komutları

```
cargo build --release -p kama-integration --example int02_node
cargo build --release -p kama-provision
scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms "gating1 gating2 gating6" [--bin <hedef>/release]
scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms negatif --window 30 [--bin <hedef>/release]
scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms "ek-dosya-ptt ek-kesinti" [--bin <hedef>/release]
python3 scripts/netem/int02_analyze.py results/int02/<tarih> --md <tablolar.md> --json <özet.json>
# yeniden koşu (CEVAP PAKETİ 4, §10):
scripts/netem/int02_run.sh --seeds "1 2 3 4 5 6 7 8 9 10" --arms gating2 --window 600 [--bin <hedef>/release]
scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms "ek-kesinti ek-dosya-ptt" --window 600 [--bin <hedef>/release]
```

---

## 2. GATING (1) — iki yönde `delay 200ms 200ms loss 30%`, 100 TEXT + 100 RECEIPT

| tohum | sonuç | teslim % | alındı % | gecikme medyan / 95p (ms) | alındı RTT medyan / 95p (ms) | ARQ retx A / B | tail-probe A / B | app retx A / B | crypto retx A / B | ctx A / B | hub A / B | dcd A / B | not_ready A / B | epoch-1 (Connected'dan, ms) A / B | netem doğrulama | HÜKÜM |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |

(A = alice, B = bob; ARQ retx = o düğümün GÖNDERDİĞİ yöndeki motor kararları — alice: TEXT, bob: RECEIPT.)

## 3. GATING (2) — akış sırasında epoch 0→1, kayıp yok, tek oturum

| tohum | sonuç | teslim % | alındı % | gecikme medyan / 95p (ms) | bob ilk gönderim epoch'u | bob epoch-1 − ilk gönderim (ms) | epoch-0'da gönderim | son epoch A / B | Ready A / B | canlı oturum A / B | ARQ retx A / B | app retx A / B | crypto retx A / B | ctx A / B | netem doğrulama | HÜKÜM |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |

Epoch ölçüsü örneklemedir (`NodeMetrics` her düğüm turunda tazelenir; tur ≤ `NODE_TICK` = 20 ms):
"bob ilk gönderim epoch'u", ilk `send_text`'ten hemen önce okunan değerdir; datagram başına tel epoch'u
gözlenmez.

### 3.1 DUR-K4G-01 (iii) — provisional `NotReady` düşüşleri Karn / PreConverge RTT kestirimini şişiriyor mu

| tohum | not_ready_drops A / B | RTT örneği A / B | PreConverge çıkışı (uç açılışından, ms) A / B | sRTT (ms) A / B | min RTT (ms) A / B | ping ort. RTT (ms) |
|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … |

Yorum (koşudan sonra): …

## 4. GATING (6) — eşzamanlı connect (glare)

| tohum | sonuç | Connected A / B | Closed+Abandoned A / B | A→B teslim/gönderilen · alındı | B→A teslim/gönderilen · alındı | tekrar A→B / B→A | INIT A / B | RESPONSE A / B | Ready A / B | app retx A / B | crypto retx A / B | ctx A / B | netem doğrulama | HÜKÜM |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … | … | … | … | … | … | … |

Beklenen (sahip kararı): her tarafta tek `Connected`; `Closed` / `ConnectAbandoned` yok; iki yönde TEXT
+ RECEIPT tam bir kez — her tohumda. İki INIT'in gerçekten çaprazlanıp çaprazlanmadığı bu düzenekte
denetlenmez (zamanlama belirsiz); INIT / RESPONSE sayıları kayda geçer. **Erken metin GATING (6)'ya
GİRMEZ:** motor düzeyinde #130 ile kanıtlı, ürün API'sinde yok (DUR-K4G-11b). **"≤ 2 RTT" el sıkışma
ölçütü INT-02'de ölçülemez** (zarflar netem dışında); yapısal kanıt #130.

## 5. T4 — sabit tohum, 5 tohum, 5/5

| kol | geçen / koşulan | T4 |
|---|---|---|
| gating1 | … | … |
| gating2 | … | … |
| gating6 | … | … |

## 6. Negatif kol — recovery OFF (ölçümün kaybı GÖREBİLDİĞİNİN kanıtı)

| tohum | sonuç | teslim / gönderilen | alındı % | bastırılan kurtarma kararı A / B | motor retx kararı A / B | tele çıkan retx baytı A / B | netem doğrulama | HÜKÜM |
|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … |

Beklenen biçim: kurtarma kapalıyken kaybolan ilk birim kurtarılmaz; S1 adaptörü oturum başına sıralı
teslim ettiği için (`Reorder`, tutulan birim tavanı aşılana dek boşluk atlanmaz) teslim, ilk kalıcı
boşluğa kadar olan önekle sınırlı kalabilir. Yorum (koşudan sonra): …

## 7. NON-GATING ek koşular

### 7.1 100 TEXT + 1 dosya (150 KiB) + 1 PTT klibi — aynı profil

| tohum | sonuç | TEXT teslim | gecikme medyan / 95p (ms) | dosya bayt | gönderim→alındı (ms) | gönderim→FILE_RECEIPT (ms) | bayt-özdeş | PTT demet alınan/verilen | sıralı+tam | bas→ilk ses (ms) | netem doğrulama |
|---|---|---|---|---|---|---|---|---|---|---|---|
| … | … | … | … | … | … | … | … | … | … | … | … |

### 7.2 Akış ortasında 60 s tam kesinti (`loss 100%`)

Eşgüdüm: örnek `KESINTI_BASLA` işaret dosyasını yaratır; sürücü `tc qdisc change dev lo root netem
loss 100%` uygular, `KESINTI_UYGULANDI` yaratır, 60 s bekler, profili aynı tohumla geri kurar ve
`KESINTI_BITTI` yaratır (anlar sürücü kaydında). Hub mailbox olmadığı için (PoC-KAPSAM-001 EK-2) yük
tamamen gönderen outbox'ında ve transport ARQ'sundadır. Teslim olmazsa NEDEN (ARQ terk süresi, oturum
durumu) buraya yazılır — bu bir DUR değil, ölçümdür.

| tohum | sonuç | TEXT teslim | kesinti (ms) | kesintide gönderilen | bunlardan teslim | kesinti bitişinden teslim medyan / 95p / max (ms) | PTT demet | sıralı+tam | Ready A / B | ARQ retx A / B | netem doğrulama |
|---|---|---|---|---|---|---|---|---|---|---|---|
| … | … | … | … | … | … | … | … | … | … | … | … |

---

## 8. Beyanlı boşluklar

### 8.1 Plan K4G-05 NE 4 listesi (birebir)

1. 2026-10-08 raporundaki "uygulama verisi enjekte edilemez" iddiası sunucu yolunda yanlıştı, K4G-01
   (#114) ve K4G-01b ile kapatıldı, regresyon `server_*` testleri (Ek (3)).
2. Hattaki saldırganın gözlediği geçerli datagramı sahte adresten önce iletmesiyle `max_peers` içinde ek
   yuva açılabilir (K4G-01b §4, kapanış M4).
3. Direct-mode'da hub rölesi / hop-ARQ / HubChannel ACK sınanmadı.
4. INIT retry yok (T3).
5. Ara çerçeveden BatchPayload'a geçiş sonraki sprint.
6. zstd yok.
7. H-PROGRESS real'de sınanmadı.
8. M4 yok → SSI1 alanları (`recipient_dest_hash`, `sender_cert_ref`, `init_counter`) UDP yan kanalında
   düz metin (transport-layer-tdd §1 ve CLAUDE.md §4 dest_hash kuralıyla çelişir, PoC-KAPSAM-001 §5'te
   beyanlı). *(INT-02 düzeneğinde zarflar süreç-içi röleden geçer, tele hiç çıkmaz; beyan hub yolu
   içindir.)*
9. Anahtarların kaynağı: `kama-provision` çıktısı (§1.1); test fikstürü kullanılmadı.
10. Boş batch tel boyutu: ölçülemedi — veri yolu BatchPayload taşımıyor (ara çerçeve, madde 5). **HESAP:**
    `batch.rs:51-52` codec'te 1 B (`EMPTY_BATCH = [submsg_count = 0]`, ccr §35.14 [K3]).

### 8.2 Ek beyanlar

1. **Ek (4) — BestEffort / DeadlineBounded:** BestEffort/DeadlineBounded Reliable gibi taşınır; canlı
   medya transport üzerinden sınanmadı (K4G-10 ertelendi; PoC-KAPSAM-001 EK-3: canlı medya PoC'de medya
   motorunun kendi UDP yolundan gider).
2. **#114 / #116 üç beyanlı sınır (DUR-K4G-01):**
   (i) teslim kanalı taşması (`runtime.rs` ~:2321 `try_send`; bu dalda `push_delivery`) INV-ACK-DELIVER-1'i
   ihlal edebilir → sayaç `delivery_channel_drops`, INT-02'de 0 olmalı (§2–§4 tabloları);
   (ii) `Closed` sonrası ACK'li kuyruktaki birimlerin düşmesi (INT-02 kollarında oturum kapanmaz;
   sınanmadı);
   (iii) provisional `NotReady` düşüşlerinin Karn/PreConverge RTT kestirimini şişirmesi — GATING (2)
   epoch geçişinde ölçüldü (§3.1).
3. **K4G-07 beyanlı istisna 4:** iptal→abandon okunmadan senkron `connect()→Ready(h)` yarışında erken
   metnin "h'ye gönderilmiş" sayılması (canlılık; yanlış Delivered yok; main'de de vardı). INT-02'de
   iptal ve erken metin yolu kullanılmaz; sınanmadı.
4. **K4G-01b §4 — sahte adresten ek yuva:** bkz. §8.1 madde 2 (kapanış M4 / adres doğrulama).
5. **El sıkışma zarfları netem dışında:** SSI1/SSR1 süreç-içi röleden geçer (DUR-K4G-11 → B); kayıp ve
   gecikme yalnız veri datagramlarına uygulandı. İki cihaz arası taşıyıcı = hub Plane 1 rölesi (S4, hub
   sprinti).
6. **Tek süreç:** iki düğüm aynı süreçte; tek monotonik saat (gecikme ölçüsü bunun üzerine kurulu).
   İki telefon arası oturum hub sprintiyle gelir.
7. **`rtt_ms` / `loss_pct` / `bitrate_kbps` `None`:** `NodeMetrics` bu alanları ürün yüzeyinde vermez
   (client-core'da ürün yüzeyli salt-okunur kaynak yok; KATMAN 6 kartı: motor sRTT/kayıp/bitrate
   erişimcileri + overlay, hub sprinti, demo öncesi ZORUNLU). INT-02'nin RTT değerleri test-util
   sondasından (oturum ucu başına sRTT / min RTT) okunur; ürün overlay'i değildir.
8. **Glare / başlatma sırası kuralı:** süreç-içi röle kayıtsız eşe zarf saklamaz; geç başlayan eşte glare
   kuralı iki niyeti de takabilir (`A.dest_hash < B.dest_hash`) ve çözülme ancak erken tarafın bekleyen
   INIT'i kalkınca olur (`carrier.rs` modül belgesi). INT-02 iki düğümü connect ÖNCESİ başlatır ve el
   sıkışma sırasında yeniden başlatmaz.
9. **Erken metin ürün API'sinde yok:** `KamaNode` `connect_with_text` sunmaz; oturumsuz `send_text` →
   `NoSession`. Motor düzeyinde #130 ile kanıtlı; karar sahipte (DUR-K4G-11b).
10. **"≤ 2 RTT" el sıkışma ölçütü ölçülemez:** zarflar netem dışında; yapısal kanıt #130.
11. **INIT retry (T3) yok:** Crypto retransmit'in yapısal beklentisi 0'dır; röle sayacı doğrular (§1.5).
12. **M4 yok → SSI1 alanları düz metin:** bkz. §8.1 madde 8.
13. **Hub / hop-ARQ / HubChannel sınanmadı:** bkz. §8.1 madde 3 (`hub_frames_dropped` yalnız 0 olduğu
    doğrulanır).
14. **H-PROGRESS real'de sınanmadı:** direct-mode yalnız `Measuring` üretir (`Suspended` / `LongOutage`
    tetikleri hub sprintinde).
15. **Loopback + netem saha değildir** (plan K4G-05 TRADE-OFF): tek makine, tek çekirdek zamanlayıcısı,
    `lo` MTU'su; saha radyo kanalının patlamalı kaybı ve bant sınırı bu profilde yoktur.
16. **`seed` ve tekrar:** sabit tohum netem üretecini sabitler; paket zamanlaması koşudan koşuya
    değiştiği için kayıp deseninin bit-bit aynı olduğu iddia edilmez.

## 9. Ölçülemeyenler (tahmin YAPILMADI)

| Kalem | Neden |
|---|---|
| `rtt_ms`, `loss_pct`, `bitrate_kbps` (`NodeMetrics`) | ürün yüzeyinde kaynak yok (§8.2 madde 7) |
| `last_heartbeat_age_s` | heartbeat hub sprintinde (D18) |
| Datagram başına tel epoch'u | Crypto bunu dışarı vermez; epoch düğüm ölçüsünden örneklenir (§3) |
| "≤ 2 RTT" el sıkışma | zarflar netem dışında (§8.2 madde 10) |
| Kapanış boşaltmasındaki ARQ kararları | canlı ayna ana döngüde yazılır; ucu bırakılan oturumun sayaçları bırakıldığı andaki aynadır (alt sınır). Tek oturumlu kollarda ölçü sakinleşmeden sonra ve oturum açıkken alınır |
| Boş batch tel boyutu | BatchPayload veri yolunda yok; değer HESAP (§8.1 madde 10) |

---

## 10. K4G-05b sonrası yeniden koşu (CEVAP PAKETİ 4)

Sahip kararı (CEVAP PAKETİ 4, 2026-10-16), birebir: "INT-02 **yeniden**: 10 tohum × 600 s pencere,
GATING (2) 10/10; ek-kesinti kolu 60 s kesinti 5/5 **aynı oturumda** %100 teslim; dosya kolu (D3)
harness düzeltmesiyle (uzun pencere kesinti/dosya kollarına geçer) ölçülür ve raporlanır."

| Alan | Değer |
|---|---|
| Ön koşul | K4G-05b (motor düzeltmesi, ayrı şerit) merge; dal main'e yeniden tabanlanır |
| Koşu | tarih: … · commit: … · makine / çekirdek: … |
| Şema | `int02-node-v2` (`params.window_s`, `window`, `file.outcome`, `outage.session`, `outage.same_session`) |
| Pencere | son gönderimden (ek-kesinti kolunda kesinti bitiminden) sonra tamamlanma üst sınırı — bütün kollar için tek parametre; bu koşuda 600 s |
| Kollar | `gating2` 10 tohum (`1 … 10`) · `ek-kesinti` 5 tohum (`1 … 5`) · `ek-dosya-ptt` 5 tohum (`1 … 5`) |
| Ham çıktı | `results/int02/<tarih>/` |

Komutlar: §1.6 "yeniden koşu" satırları. Tablolar `int02_analyze.py` çıktısından doldurulur; hüküm
kuralları (§2–§6) değişmedi.

### 10.1 GATING (2) — 10 tohum × pencere 600 s

| tohum | sonuç | pencere (s) · sonu | teslim % | alındı % | gecikme medyan / 95p (ms) | bob ilk gönderim epoch'u | bob epoch-1 − ilk gönderim (ms) | son epoch A / B | Ready A / B | canlı oturum A / B | ARQ retx A / B | app retx A / B | crypto retx A / B | ctx A / B | netem doğrulama | HÜKÜM |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 6 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 7 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 8 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 9 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 10 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |

GATING (2): … / 10 tohum GEÇTİ (ölçüm tablosu; kapı ifadesi değildir).

### 10.2 ek-kesinti — 60 s kesinti, aynı oturumda %100 metin teslimi (PTT ayrı)

`same_session` = kesinti başından koşu sonuna iki düğümde yeni `Closed` yok, ikinci `Connected` yok,
eşle oturumun epoch'u değişmemiş (`outage.session.<düğüm>.{start,end}`); "yalnız kesinti" = aynı ölçüt
kesinti bitişine dek; "tutamaç + S1" = oturum tutamaçları ve S1 `Ready` / `Closed` sayıları da aynı.
Sahip ölçütü = metin %100 ∧ `same_session`; PTT ayrı tablodadır ve ölçüte girmez.

| tohum | sonuç | pencere (s) · sonu | metin teslim/gönderilen | metin %100 | kesinti (ms) | kesintide gönderilenden teslim | kesinti bitişinden teslim medyan / 95p / max (ms) | same_session | yalnız kesinti | tutamaç + S1 | epoch başı→sonu A / B | Connected başı→sonu A / B | Closed başı→sonu A / B | ARQ retx A / B | netem doğrulama | sahip ölçütü |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |

Sahip ölçütü: … / 5 tohum (ölçüm özeti; kapı ifadesi değildir).

PTT (ayrı):

| tohum | demet alınan/verilen | sıralı+tam | PttOpened olayı | PttClosed olayı | bas→ilk ses (ms) | Ready A / B |
|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … |

### 10.3 ek-dosya-ptt (D3) — dosya sonucu, metin ve PTT tam pencere boyunca

Pencere metinler + PTT tamamlanana VE dosya kesin sonuca ulaşana dek sürer (teslim + alındı ya da
gönderen tarafta başarısızlık). Dosya sonucu ayrı alandadır (`file.outcome`: durum `delivered` /
`failed` / `pending`, neden, zaman damgaları). Kol hükmü değişmedi: başarısız dosya kolu tamamlanmamış
sayar.

| tohum | sonuç | pencere (s) · sonu | TEXT teslim | gecikme medyan / 95p (ms) | dosya durumu | neden | gönderim→sonuç (ms) | gönderim→alındı (ms) | gönderim→FILE_RECEIPT (ms) | alıcı başarısızlığı | bayt-özdeş | PTT demet alınan/verilen | sıralı+tam | bas→ilk ses (ms) | netem doğrulama |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 2 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 3 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 4 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |
| 5 | … | … | … | … | … | … | … | … | … | … | … | … | … | … | … |

### 10.4 Takılı birim özeti (teşhis; hüküm değil)

`int02_analyze.py` "Takılı birim özeti" tablosu (son sondadaki gönderen kurtarma görüntüsü ve alıcı
sıralı teslim durumu; zaman çizelgesi `timeline.samples`, 5 s aralık, tavan 256 örnek).

| tohum | kol | sonuç | teyitsiz birim A / B | gönderen A | gönderen B | alıcı reorder A | alıcı reorder B | zaman çizelgesi örneği |
|---|---|---|---|---|---|---|---|---|
| … | … | … | … | … | … | … | … | … |

Yorum (koşudan sonra): …

---

## 11. Sonuç (koşudan sonra doldurulur)

…

Abkarada'nın 'INT-02 GEÇTİ' mesajı bekleniyor.
