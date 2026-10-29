# KTP2-F0 — Ajan D: rakip karşılaştırması (taslak, tur 1)

Durum: **bu turda koşu yok.** Aşağıdaki her şey [kod] (kaynak okuması, dosya:satır), [çıkarım]
(ölçülmemiş akıl yürütme / fiziksel hesap) ya da yöntemdir. Ölçüm bölümü (§8) koşulardan sonra
doldurulur. Satır numaraları: bench için dal `ktp2/f0-d` (taban `fbe083b2` ise ayrıca yazıldı),
quinn için `quinn-proto 0.11.18` (Cargo.lock, iki ağaçta aynı).

## 1. Kapsam ve kollar

| kol | tanım | teslim tanımı | kurulum |
|---|---|---|---|
| `taban`, `aday` | `bench --transport kama-product`, bayraksız ürün profili (Ajan B `arms_kama.sh`) | gönderende ilk teyit (ACK ya da FEC-RECOVERED), `kama_product.rs` `acked()` | el sıkışma yok; bench `IdentitySeam` (kripto yok, +16 B sahte etiket) |
| `quinn` (BİRİNCİL) | quinn 0.11.11 / quinn-proto 0.11.18, **BBR**, mesaj başına bağımsız **tek-yönlü stream**, idle 300 s, MTU 1452 sabit | eşin **taşıma ACK'i** (`SendStream::stopped` → `Ok(None)`); uygulama yanıtı yok | TLS 1.3 + QUIC el sıkışması, tavan `HS_TIMEOUT_S` (vars. = `DEADLINE_S`), pencere dışında, `setup_ns` |
| `tcp` (BİRİNCİL) | çekirdek TCP, **tek bağlantı**, `TCP_NODELAY`, CC = çekirdek varsayılanı (bu makinede `cubic`; `tcp_cc.txt`) | `snd_una` mesajın kümülatif sonunu geçti (`/proc/self/net/tcp` `tx_queue`, 5 ms örnek) | 3 yönlü el sıkışma, tavan `HS_TIMEOUT_S`, pencere dışında |
| `quinn_rpl` (betimsel) | eski kol: bi-stream + 1 B uygulama yanıtı (önceki kampanyalar, KATALOG §1) | yanıt istemciye varınca | tavan = `--max-deadline-s` (eski kural) |
| `tcp_rpl` (betimsel) | eski kol: 8 B uygulama ACK'i, `--early-exit` | ACK istemciye varınca | tavan 20 s (eski) |
| `quinn_cubic`, `quinn_irtt` (betimsel) | Cubic; BBR + `initial_rtt` = profil RTT'si + IW 12 000 B | `quinn` ile aynı | aynı |

- **TCP tek bağlantıdır**: bütün mesajlar tek bayt akışında, tam sıralı; bir kayıp sonraki bütün
  mesajları bekletir (HOL). Bu, kıyasın konusudur; normalize edilmez.
- quinn'de mesaj başına stream = mesajlar arası sıra YOK (manifest `quinn-tack`: `reliable-messages`,
  `ordering none`). Kama bench kolu da sırasızdır. **Bench'in Kama kolu ürünün S1 adaptörünü
  (oturum-geneli `Reorder`, `client-core/src/transport.rs:361-395`) İÇERMEZ** — alıcı tarafı
  `ServerEndpoint::next_delivery` motorun sırasız teslimidir. Bu yüzden bench sayıları motor
  düzeyidir; ürün uygulama yolunun HOL'u Ajan A'nın ölçümündedir.
- Kıyas-adaleti denetimi: komutun adı `bench fairness --manifests …` (`main.rs`, `Cmd::Fairness`);
  `compare` adında komut yok [kod].

## 2. Bench değişiklikleri (hepsi yalnız açık bayrakla; bayraksız yol eskiyle aynı)

| bayrak / komut | yer | ne |
|---|---|---|
| `--rival-delivery transport-ack` (serve+client) | `rival.rs`, `quic.rs` `send_one_tack` (:657), `tcp.rs` `run_client_tack` (:350) | taşıma-ACK teslimi (yukarıda) |
| `--handshake-timeout-s` | `quic.rs:302`, `tcp.rs:121` | el sıkışma/bağlanma tavanı; 0 = eski (quinn `max_deadline`, tcp 20 s) |
| `--quinn-initial-rtt-ms`, `--quinn-iw-bytes`, `--quinn-keepalive-ms` | `quic.rs` `transport_config` | quinn varsayılanlarının açık değiştirilmesi |
| `--rival-stats` | `quic.rs` `print_quinn_stats` (:677) | `QUINN-STATS` satırı: rtt, cwnd, congestion_events, lost_packets/bytes, udp ve frame sayaçları |
| `serve --rx-log P` (İSTEK-C1/D2) | `rxlog.rs`; kama-product `server_loop`, tcp ve quinn sunucuları | alıcı damgası, şema `ktp2-c-rx-v1` (`{"u","t","n"}`), 2^20 tavan, atomik yazım; quinn/tcp'de yalnız bu bayrakla SIGTERM yakalanır |
| `client --payload-id` (D2) | `quic.rs` | quinn yükünün ilk 8 baytı = mesaj kimliği (alıcı eşlemesi) |
| `client --early-exit` (D1) | `tcp.rs` | eski tcp kolu bütün ACK'ler gelince biter; teslim olayları değişmez |
| `client --dup-copies N` (C2) | `kama_product.rs` `client_loop` | her ölçüm mesajı N ayrı birim; teslim = ilk kopya; birim = mesaj×N+kopya |
| `KP-CLIENT-OFFER` satırı (D4) | `kama_product.rs` | tx/rx datagram, `unaccepted`, QueueFull yeniden deneme sayısı ve beklemesi (ayrı satır: `KP-CLIENT` biçimi korunur) |
| `mixed-serve` / `mixed-client` | `mixed.rs` | karışık akış kıyası (§6) |

## 3. Koddan bulgular (ölçüm · dosya/işlev · kök neden · en dar düzeltme)

**D-1 Teslim tanımı rakiplere fazladan ileri-yön paketi yüklüyor.** Ölçüm: yok [çıkarım, aşağıda
fiziksel]. Dosya: `quic.rs` (taban `fbe083b2`) `run_server_loop` :516-518 (1 B yanıt) + `send_one`
:536-541; `tcp.rs` :82-86 (8 B ACK). Kök neden: uygulama yanıtı ack-eliciting'dir; istemci bunu
da ACK'lemek zorundadır → mesaj başına ileri yönde ek bir saf ACK (quinn ≈ 35 B UDP + 42 B L2 ≈
77 B; TCP 52 B + 14 = 66 B). 25 B/s'de 65 B mesajın kendisi ≈ 137–143 B tel (≈ 5,5 s); ek ACK
≈ 2,6–3,1 s, yani ileri yön tel yükünün ≈ %46–56'sı. Kama'nın taşıma ACK'i ACK'lenmez. Düzeltme:
`--rival-delivery transport-ack` (uygulandı; birincil kollar bunu kullanır). Eski kol `*_rpl` olarak
betimsel tutuldu; fark ölçülecek (`d_hs_r25_*`).

**D-2 El sıkışma tavanları kollar arasında farklıydı.** Dosya: `quic.rs` (taban) :256
`timeout(cfg.max_deadline, connecting)`; `tcp.rs` (taban) :103 sabit 20 s. Kök neden: iki ayrı
yazım. Etki: aynı hücrede quinn 60 s, TCP 20 s bekliyordu. Düzeltme: `--handshake-timeout-s`
(uygulandı; `rakip_arms` ikisine aynı değeri verir).

**D-3 Eski TCP kolu gönderen beklemesini gecikmeden düşüyordu.** Dosya: `tcp.rs` (taban) :153
(`sent_ns` yazma anında, önceki `write_all` bittikten sonra), :160 (`sleep(interval)` yazmadan
SONRA → plan kayar). Kök neden: çekirdek gönderim tamponu (`tcp_wmem` 16 KiB başlangıç) dolunca
`write_all` bekler; sonraki mesajların damgası geç konur. 1 KB × ≥ 16 mesajda, kayıp/RTO
duraklamasında tetiklenir [çıkarım]. Düzeltme: transport-ack kolunda sunum anı = plan
(`sent_ns = i × aralık`), uygulandı.

**D-4 Eski TCP kolu sabit süre bekliyordu ve pencereyi 60 s'de kesiyordu.** Dosya: `tcp.rs`
(taban) :164 `min(max_deadline, 60 s)`. Kök neden: erken bitiş yok; `D > 60 s`'de son mesajın
gözlem penceresi kısalır. Düzeltme: `--early-exit` (İSTEK-D1, eski kol) ve transport-ack kolunda
mesaj başına tam pencere + erken bitiş (uygulandı).

**D-5 quinn BBR ilk penceresi 240 000 B.** Dosya: quinn-proto `congestion/bbr/mod.rs:534,648`
(`200 × 1200`); Cubic `congestion/cubic.rs:266` → 12 000 B. Bench bunu değiştirmiyordu
(`quic.rs` taban :183-185). Etki [çıkarım]: düşük bantta ilk turda bütün yük darboğaz kuyruğuna
dökülür (pacing ilk RTT örneğine dek `pencere / 333 ms`); netem `limit 1000` paket içinde kaldığı
sürece teslimi değil kuyruk gecikmesini etkiler. Düzeltme: `--quinn-iw-bytes` (betimsel
`quinn_irtt` 12 000 B). Birincil kol quinn varsayılanında bırakıldı (en güçlü "mevcut" yapılandırma
= kütüphanenin kendisi).

**D-6 quinn ilk RTT 333 ms → düşük bant/yüksek RTT'de sahte el sıkışma yoklamaları.** Dosya:
quinn-proto `config/transport.rs:383` (`initial_rtt 333 ms`), `connection/paths.rs:331`
(PTO = srtt + 4·rttvar = 999 ms), `connection/mod.rs:1679-1690` (PTO başına **2** yoklama),
`:789-790` + `lib.rs:330` (istemcinin Initial içeren her datagramı ≥ 1200 B'a doldurulur). Kök
neden: ilk turda PTO, gerçek tur süresinden (serileştirme dahil) kısa. Etki §4'te. Düzeltme
(betimsel kol): `--quinn-initial-rtt-ms`. Kama'ya eşdeğer bir ön bilgi verilmediği için birincil
kolda kullanılmadı.

**D-7 Hücre koşucusu koşular arasında netem kuyruğunu boşaltmıyor.** Dosya:
`scripts/ktp2/f0/f0_cell.sh` `run_one` (netem yalnız hücre başında, `kapi3_netem` `tc qdisc
replace`). Kök neden: bir koşunun kuyrukta kalan paketleri sonraki kolun koşusunun önüne geçer.
25 B/s'de quinn tek başına ≈ 14 × 1242 B ≈ 695 s ileri kuyruk bırakabilir (§4) [çıkarım]. Ayrıca
sayaç farkları (realized pre/post) sonraki koşuya kayar. En dar düzeltme: İSTEK-O-D1 (§9).
Analiz betiği koşu öncesi `backlog/qlen > 0` olan koşuyu "kirli" işaretler.

**D-8 quinn manifest anlamı mesaj başına stream modelini yanlış adlandırıyor.** Dosya: `manifest.rs`
(taban) :264-270 `"quinn"` → `reliable-ordered-stream / total`; ama `quic.rs:536` her mesajı ayrı
stream'de taşır → mesajlar arası sıra yok. Etki: Kama↔quinn `fairness` çıktısında `ordering`
farkı yanlış alarmı. Düzeltme: eski girdiye dokunulmadı (varsayılan çıktı değişmesin); yeni
`quinn-tack` girdisi doğru anlamla eklendi.

**D-9 Bench Kama kolunda kurulum maliyeti yapısal olarak sıfır.** Dosya: `kama_product.rs:17-20`
(`IdentitySeam`). Kök neden: bench'te Noise yok. Ürünün gerçek SESSION_INIT baytları bu kıyasta
ÖLÇÜLMEZ; quinn'in TLS 1.3 el sıkışması gerçektir (rcgen P-256 öz-imzalı sertifika). Kurulum
kıyası bu sınırla raporlanır.

**D-10 TCP için "en güçlü CC" bu makinede yok.** `/proc/sys/net/ipv4/tcp_available_congestion_control`
= `reno cubic`; `tcp_bbr.ko.zst` var ama yüklü değil. Yüklemek sahibin ayrıcalıklı işi (`modprobe
tcp_bbr`). TCP kolu çekirdek cubic'tir; `rival_wrap.sh` yürürlükteki CC'yi her koşuya yazar.

**D-11 quinn `max_ack_delay` 25 ms, ayarlanamıyor.** quinn-proto `transport_parameters.rs:60`;
değiştirmek ACK-frequency uzantısı ister. 25 B/s'de her saf ACK ≈ 77 B (≈ 3,1 s dönüş hattı).
Ayar yapılmadı; yük `QUINN-STATS frame_tx.acks` ve netem dönüş sayacıyla raporlanır.

**D-12 Eski TCP kolu gate/warmup/bütçe bayraklarını yok sayar.** `tcp.rs` (taban) — F0 hücreleri
bunları kullanmıyor; yalnız beyan.

## 4. Fiziksel hesap: el sıkışma ve ilk teslim [çıkarım]

Varsayımlar: kayıpsız, jitter yok, netem `rate` L2 dahil (Eth 14 + IP 20 + UDP 8 = 42 B),
istemci Initial = 1200 B UDP yükü → **1242 B tel**; sunucunun ilk uçuşu 1 datagram ≥ 1242 B
(ServerHello + EE + P-256 sertifika + CV + Fin ≈ 0,8 KB, 1200 B'a doldurulur); istemcinin ikinci
uçuşu (Initial ACK + Handshake Finished) yine 1200 B'a doldurulur. PTO ilk tur 0,999 s, her tur
iki katı, tur başına 2 datagram. TCP: SYN/SYN-ACK 74 B tel, SYN RTO 1 s.

| profil | QUIC 1242 B serileştirme | QUIC bağlantı (istemci) | sahte PTO yoklaması (datagram) | ilk 1-RTT veriden önce ileri kuyruk | TCP SYN | TCP bağlantı | sahte SYN | TCP ilk veriden önce kuyruk |
|---|---|---|---|---|---|---|---|---|
| r25 d300 | 49,7 s | ≈ 100,0 s | 12 | ≈ 695 s | 2,96 s | ≈ 6,5 s | 2 | ≈ 8,9 s |
| r25 d1000 | 49,7 s | ≈ 101,4 s | 12 | ≈ 695 s | 2,96 s | ≈ 7,9 s | 3 | ≈ 11,8 s |
| r1k d300 | 1,24 s | ≈ 3,1 s | 4 | ≈ 7,5 s | 0,07 s | ≈ 0,75 s | 0 | — |
| r1k d1000 | 1,24 s | ≈ 4,5 s | 4 | ≈ 7,5 s | 0,07 s | ≈ 2,15 s | 1 | ≈ 0,15 s |
| r12k d300 | 0,10 s | ≈ 0,8 s | 0 | ≈ 0,2 s | 0,006 s | ≈ 0,61 s | 0 | — |
| r12k d1000 | 0,10 s | ≈ 2,2 s | 2 | ≈ 0,4 s | 0,006 s | ≈ 2,0 s | 1 | — |

Sorudaki sayı: **25 B/s'de tek 1200 B Initial ≈ 49,7 s** (L2 dahil; L2 hariç 48 s); 31 B/s'de
≈ 40 s (taslak Ek A.4'teki "≈ 39 s" L2'siz değerdir). QUIC'in bağlanabilmesi için en az iki
doldurulmuş datagram iki yönde seri geçmeli → 25 B/s'de **≈ 100 s alt sınır**; 60 s pencereli
hiçbir 25 B/s hücresinde quinn mesaj teslim edemez (iki görünümde de). Uzun tavanla bağlansa bile
kuyruktaki ≈ 14 doldurulmuş datagram ilk veriyi ≈ 700 s geciktirir. `setup_ns` bunun yalnız
bağlantıya kadarki kısmını gösterir; kuyruk taşması **steady-state görünümünde ilk mesaj
gecikmesine** yansır (analiz `ilk gecikme` sütunu). Kama bench kolu ilk paketi hemen yollar
(65 B → 140 B tel ≈ 5,6 s + 0,3 s).

Mesaj başına tel boyu (65 B yük, kayıpsız tek iletim): Kama 65 + 33 + 42 = 140 B; quinn ≈ 65 + ≈ 30
(kısa başlık 1 + DCID 8 + PN 1–2 + STREAM başlığı ≈ 4 + etiket 16) + 42 ≈ 137 B; TCP 65 + 12
(bench çerçevesi) + 32 (TCP+zaman damgası) + 20 + 14 = 143 B [çıkarım; netem sayaçlarıyla
doğrulanacak].

## 5. Profil başına beklenen davranış [çıkarım — önceden kayıt]

| profil sınıfı | quinn (BBR, uni-stream, tack) | tcp (tek bağlantı, cubic) |
|---|---|---|
| r25, her kayıp | bağlantı ≥ 100 s → completion(60 s) = 0 (ss ve cold); `connection-failure` | bağlanır (≈ 6,5–8 s); l0'da teslim; l10–l20'de RTO geri çekilmesi (srtt serileştirmeyle şişik ≥ 6 s → RTO ≥ 10 s, ardışık kayıpta 2×) + HOL → K-3 ölçütü (p95 ≤ 12 s) aşılır |
| r1k, l0 | el sıkışma ≈ 3–4,5 s + ≈ 4 s kuyruk taşması; ss'de ilk mesajlar ≈ 4–5 s geç, sonra hat hızında | ≈ 0,75–2,2 s bağlantı; hat hızında |
| r1k, l10–l30 | BBR kayıpta hızı düşürmez (`on_congestion_event` yalnız sayar, bbr/mod.rs:468-476); BDP ≈ 0,6–2 KB < asgari pencere → hız sınırı hat; kurtarma PTO/RACK ile ≥ 1 RTT; mesajlar arası HOL yok | asgari pencere de BDP'nin üstünde → CC değil RTO ve HOL belirleyici; l20–l30'da completion(60 s) düşüşü quinn'den önce beklenir |
| r12k d300 l10–l30 | BBR hat hızına yakın; Cubic (betimsel) asgari pencere 2 × 1452 B / 0,6 s ≈ 4,8 kB/s | çekirdek cubic: kayıpta cwnd 2 MSS'e iner → ≈ 4,8 kB/s tavan + HOL |
| r12k d1000 l10–l30 | BBR etkilenmez; Cubic ≈ 1,45 kB/s tavan → %41 yükte kuyruk büyür (`d_cc_*`) | ≈ 1,45 kB/s tavan; yükün altında kalır → gecikme birikir |
| stres l50/l80 | el sıkışma: tur başına (istemcinin 2 yoklamasından en az biri) × (sunucu yanıtı) ≈ 0,75·0,5 (l50), 0,36·0,2 (l80); 60 s'de ≈ 5–6 PTO turu → l80'de bağlanma olasılığı ≈ %35 mertebesi | SYN küçük ama 6 deneme/127 s; HOL ve RTO geri çekilmesi baskın |

Karışık akış (r12k_d300_l30) öngörüsü: **tcp** dosyalı kolda PTT/TEXT dosya baytlarının arkasında
kalır (tek akış + çekirdek tamponu) → K-1 GEÇMEZ. **quinn** akış başına stream + öncelik → gönderen
tarafında dosya PTT/TEXT'i bekletmez; PTT akışı kendi içinde sıralı olduğu için kayıpta akış-içi
HOL kalır (≈ %70 son-tarih oranı, 0,4 s). **Kama (motor)** yeni-veri kuyruğu tek FIFO (iş emri
§3 A; Ajan A) ve bench dosyayı t0'da sunduğu için PTT/TEXT dosyanın ≈ 192 KB'ının arkasında bekler
→ K-1 taşıma düzeyinde GEÇMEZ beklenir; bu hücre Kama'nın KAYBEDEBİLECEĞİ hücre olarak önceden
kayıtlıdır.

## 6. Karışık akış kıyası — kapsam

`bench mixed-client/mixed-serve` (`mixed.rs`): dosya (1000 B parça, hepsi t0'da) + TEXT (65 B,
1 s) + PTT (39 B = 30 B kodlayıcı + 9 B başlık, 200 ms demet) eşzamanlı. Teslim **alıcıda**
damgalanır (CLOCK_REALTIME; iki süreç aynı makine). Sunum anı plandır. Dosyalı/dosyasız çift →
K-1 taşıma uyarlaması: TEXT p95(dosyalı) ≤ p95(dosyasız) + 2 × 1514 B / hız + 1 RTT; PTT son tarih
oranı ≥ 0,95 × dosyasız. Kapsam dışı: S1 adaptörü sıralaması, dosya tüketicisinin durması, kodlayıcı/
titreşim tamponu (PTT oranı tek-yön taşıma gecikmesidir, ağızdan kulağa değildir), FEC'in PTT'ye
özel kullanımı (Ajan C).

## 7. Analiz yöntemi (betikler hazır, sentetik veriyle denendi)

- `scripts/ktp2/f0/d/rakip_analiz.py KÖK… [--bench BIN] [--md X --json Y]`: hücre × kol;
  completion(10/30/60 s) medyan [IQR] (min–maks), steady-state ve cold-flow (setup_ns her mesaja
  eklenir: paced yükte geç mesajlar için kötümser üst sınır); sansürlü gecikme kuantilleri
  (teslim edilmeyen = ∞); kurulum; netem sayaçlarından ileri+dönüş tel baytı / teslim edilen mesaj
  (dört kolda simetrik; TCP çekirdek yükü dahil); kirli-kuyruk işareti; TCP RetransSegs, quinn
  lost_packets, Kama retx.
- Kazanma/kaybetme kuralı (önceden yazılı): aynı hücrede completion(60 s) medyanlarının %95
  dağılımdan-bağımsız GA'ları (sıra istatistiği, binom; rastgelelik yok) örtüşmüyorsa KAZANILDI /
  KAYBEDİLDİ, örtüşüyorsa "ayrışmadı"; kol başına N < 30 → betimsel, hüküm yok.
- K-2: (hız, gecikme, yük, kol) başına completion(60 s) medyanının 0,90 altına düştüğü ilk kayıp;
  Kama ↔ quinn kesişimleri karşılaştırılır (iki görünümde).
- `bench fairness` (rep1 manifestleri) + Python ayna denetimi (yük, mesaj sayısı, pencere, aralık,
  bozulma, kurulum sınıfı).
- `scripts/ktp2/f0/d/karisik_analiz.py KÖK…`: karışık hücre (§6).

## 8. Ölçüm sonuçları

### 8.1 Duman (duman-2, N = 1 / hücre — BETİMSEL, hüküm yok)

Kök `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-2/out`; ikililer taban `2cd9f68d`
(sha256 `3311d8e0…`), aday `bf18dc35` (`c893e840…`); `F0_REPS=1`, j = 12 (diğer şeritlerle
eşzamanlı). Analiz: `rakip_analiz.py` / `karisik_analiz.py` (bu dal).

Düzenek denetimi [ölçüldü]: 6 hücre, 32/32 koşu rc = 0; koşu öncesi netem kuyruğu dolu (kirli)
koşu 0 (İSTEK-O-D1 etkin); `bench fairness` hiçbir çiftte engelleyici fark bulmadı; anlam farkları
(teslim tanımı, sıra) beyan edildi (`non-normalized`, beklenen). Alıcı damgası (`rx_log.jsonl`)
kama, tcp, quinn'de yazıldı; tcp için t0 `t_start + setup_ns` yaklaşıklamasıyla (≈ 10 ms erken).

| hücre (N=1) | taban (Kama) | quinn (BBR, tack) | tcp (tack) | betimsel kollar |
|---|---|---|---|---|
| d_hs_r25_d300_l0 (65 B × 6, 15 s) | 1/6 teslim (60 s); tel 4208 B: data 552, retx 357, **dup 2024, ctrl 1275**; `state=PreConverge`, srtt 0, `pace=31 B/s` > hat 25 B/s; koşu sonu ileri kuyruk 32 paket / 3769 B | bağlantı **107,5 s** (öngörü ≈ 100 s), 0/6 | bağlantı 6,45 s (öngörü 6,5 s), 6/6; p95 gönderen 32,6 s / alıcı 29,6 s | quinn_rpl: 60 s el sıkışma tavanı, 0/6 · tcp_rpl 6/6 |
| d_hs_r25_d300_l20 | 1/6 (60 s) | bağlantı 113 s, 0/6 | 3/6 (ilk üç mesaj 92 s'de birlikte: HOL; alıcı ilk ikisini ≈ 46 s'de almış, gönderen ACK'i geç görmüş) | tcp_rpl 5/6 |
| d_irtt_r1k_d1000_l10 (1000 B × 20, 5 s) | 20/20; p95 15,4 s | 20/20; p95 11,3 s; kurulum 4,9 s | 20/20; p95 11,3 s | quinn_irtt kurulum **18,6 s** (initial_rtt 2 s → PTO 6 s; kayıp el sıkışma paketi 6 s+ bekletir), p95 7,6 s |
| d_cc_r12k_d300_l30 (1000 B × 150, 200 ms) | 150/150; p95 11,5 s | 150/150; p95 3,4 s | 0,667 (60 s) | quinn_cubic 1,0; p95 37,4 s |
| d_cc_r12k_d1000_l10 | **0,793**; ss@30 0,013; BtlBw kestirimi 2981 B/s (hat 12 500), `pace=2981 B/s` < sunulan ≈ 5 kB/s → gönderen kuyruğu | 150/150; p95 5,0 s | 150/150; p95 6,8 s | quinn_cubic 1,0; p95 45,5 s |

Karışık akış `d_mix_r12k_d300_l30` (N=1): üç taşıma uçtan uca çalıştı (372/372 öğe kama ve quinn'de
alındı). TEXT p95 dosyalı/dosyasız: taban 18,7/8,7 s · aday 24,7/4,1 s · quinn 12,3/8,8 s · tcp ∞/15,4 s
(tcp dosyalıda TEXT@10 s = 0,033, dosya %63,5). PTT@0,4 s dosyasız kollarda bile 0,07–0,53: bu
profilde netem'in sıra koruyan jitter'ı tek-yön gecikmeyi profil değerinin üstüne taşıyor (Ajan B
İSTEK-O1) → 0,4 s son tarihi fiziksel olarak dar; analiz 1,2 s'yi de raporlayacak.

Önceden kayıtlı öngörülerden sapmalar (beklenti-kaydı; N = 1, yön bilgisi):
- **Tutan:** quinn 25 B/s bağlantısı ≈ 100 s (ölçülen 107/113 s), quinn r25'te 0 teslim; TCP
  r25 bağlantısı ≈ 6,5 s; tcp karışık akışta HOL.
- **Tutmayan:** (a) "Cubic asgari pencerede çöker" öngörüsü r12k_d1000_l10'da TCP için tutmadı
  (150/150, p95 6,8 s); quinn_cubic tamamladı ama p95 45 s. (b) quinn_irtt'in el sıkışmayı
  hızlandıracağı öngörüsü tersine döndü (kurulum 4,9 → 18,6 s): uzun initial_rtt kayıpta PTO'yu da
  uzatıyor. (c) Kama'nın 25 B/s'de kayıpsız profilde bile çoğu mesajı teslim edemeyeceği ve
  r12k_d1000_l10'da BtlBw'yi hattın ≈ %24'ü kestireceği ÖNGÖRÜLMEMİŞTİ.

Şerit-dışı bulgu adayları (ölçüm N = 1; kök neden Ajan B'nin alanı — yalnız iletiyorum):
- **K-r25:** 25 B/s'de Kama'nın asgari pacing hızı (31 B/s, `KP-ACTIVATION est pace`) hat
  hızının üstünde; RTT örneği olmadan PreConverge'den çıkılmıyor (srtt 0) ve çoğaltma + kontrol
  baytı veri baytının ≈ 6 katı. Kuyruk sınırsız büyüyor.
- **K-r12k-d1000:** RTT 2 s, %10 kayıpta BtlBw kestirimi hattın ≈ %24'ü; pacing sunulan yükün
  altında kalıyor, gecikme 30 s+.

### 8.2 Tam set — ara not 1: `d_mix_r12k_d300_l30` (N = 30 / kol, 2026-10-29 20:22→23:29Z)

Kök `~/kama-k4g-work/ktp2-f0/runs/tam/d/out/d_mix_r12k_d300_l30`; ikililer taban `2cd9f68d`
(`3311d8e0…`), aday `bf18dc35` (`c893e840…`); hücre dosyası sha256 `5ecf9764…` (bu daldaki dosya).

**Geçerlilik [ölçüldü]:**
- 8 kol × 30 koşu, hepsi rc = 0; her koşuda olay var (tx 372 / 180 öğe); alıcı günlüğünde kimlik
  hatası 0, mükerrer 0, plandan önce alım 0; tcp alım sırası yazma sırasıyla aynı (ihlal 0; quinn'de
  dosya-sırası ihlalleri stream'ler arası eşzamanlı günlükten, beklenen).
- Koşu öncesi netem: 240 koşunun 239'unda sayaçlar sıfır; `tcp_nofile rep2`'de ileri qdisc'te
  `drops = 1`, `bytes = packets = backlog = 0` (kuyruk boş; sıfırlamayla `pre` arasında düşen tek
  başıboş paket) — zararsız.
- Gerçekleşen kayıp (profil %30): Kama ve quinn kollarında ileri 0,291–0,307, dönüş 0,297–0,310
  (havuz; binom %95 aralıklarıyla tutarlı). tcp dönüş 0,308–0,309. **tcp_mix ileri 0,256 ± 0,018**
  — sayaç yapıtı: aşağıdaki D-13.
- quinn el sıkışma tavanına takılan: `quinn_nofile rep26` (60 s, `connected=false`) — geçerli 0,
  ayrı sayıldı; diğer 59 quinn koşusu bağlandı (kurulum medyanı 1,76–1,80 s).
- tcp `snd_una` kaydı (B ızgarasındaki 460 tcp-tack koşusu): `poll_fail = 0`, `polls > 0` her koşuda.
  Karışık akışta tcp teslimi alıcıda damgalanır (snd_una kullanılmaz).

**D-13 (yeni, ölçüldü) — tcp ileri yönde TSO çift-segment skb'leri.** Ölçüm: B ızgarasında r12k
hücrelerinde netem ileri birimi / (OutSegs + RetransSegs) = 0,95–0,98 ve ölçülen tcp ileri kaybı
profilden düşük (l20: 0,171–0,200; l30: 0,259–0,285); r1k'da 1,00 ve kayıp ≈ p; Kama/quinn her
yerde ≈ p. Dosya/işlev: `scripts/netem/kapi3_harness.sh` `kapi3_setup` (offload kapatma ethtool'a
bağlı; bu makinede `offload=ethtool-missing`). Kök neden: veth TSO açık → TCP 12,5 kB/s'de bir kısım
skb'yi 2 segment yollar; netem kaybı skb başına çeker (`drops` skb sayar, `packets` gso_segs sayar).
Nicelik [çıkarım, sayaçlardan]: l30 r12k'da skb'lerin ≈ %22'si çift segment (oran 0,945 öngörü ↔
0,952 ölçülen). Segment başına marjinal kayıp yine p; ancak segmentlerin ≈ %37'si çift halinde
birlikte kaybolur (kayıp olayları korelasyonlu). Etki yalnız tcp kollarında; yönü ölçülmedi. En dar
düzeltme: `kapi3_setup`'ta `ip link set dev va0 gso_max_segs 1` (ve vb0), ya da ethtool kurulumu.

**İlk gözlem (N = 30, karışık akış, taşıma düzeyi). Kama sayıları bench üst sınırıdır [D-9: S1
Reorder ve Noise kripto baytları yok].** Hücre medyanı [IQR] koşu başına; K-1 havuzlanmış.

| kol | TEXT p95 dosyalı | TEXT p95 dosyasız | Δ (dosya HOL) | PTT@0,8 s dosyalı / dosyasız | dosya bitiş s | K-1 |
|---|---|---|---|---|---|---|
| taban [D-9] | 24,5 s [20,3–29,1] | 4,9 s [3,7–7,6] | +19,6 s | 0,133 / 0,623 | 30,3 [26,0–37,8] | GEÇMEDİ |
| aday [D-9] | 24,2 s [20,1–28,2] | 4,3 s [3,1–7,2] | +19,9 s | 0,080 / 0,387 | 27,4 [23,6–32,1] | GEÇMEDİ |
| quinn | 12,8 s [11,5–14,0] | 2,3 s [1,9–4,9] | +10,5 s | 0,080 / 0,273 | 19,2 [18,3–28,4] | GEÇMEDİ |
| tcp | ∞ (TEXT@10 s 0,033) | 36,4 s [11,0–∞] | — | 0,007 / 0,073 | bitmedi (dosyanın %34'ü) | GEÇMEDİ |

- Dört kol da K-1'i (taşıma düzeyi uyarlaması) GEÇMEDİ. Dosya kaynaklı TEXT gecikmesi Kama'da
  quinn'in ≈ 2 katı; tcp'de dosya TEXT'i pencere boyunca bekletiyor (tek bayt akışı).
- Kama dosyasız PTT@0,8 s'de quinn'den yüksek (taban 0,623, aday 0,387 ↔ 0,273); dosyalıda ikisi de
  ≈ 0,08–0,13.
- Mekanizma [çıkarım, ölçülmedi]: Kama'da dosya t0'da tek FIFO sunum kuyruğuna giriyor; TEXT p95 ≈
  dosya süresi (192 000 B / ≈ 6,5 kB/s ≈ 30 s). quinn'de gönderen önceliği var ama dosya verisi darboğaz
  kuyruğunda (BBR ilk pencere 240 000 B) bekliyor; öncelik kuyruktaki paketin önüne geçemez.
- İleri tel (medyan): taban 236,7 KB, aday 237,1 KB, quinn 229,7 KB (uygulama yükü 199,8 KB); tcp
  71,1 KB (tamamlanmadı).

## 9. İSTEK'ler

- **İSTEK-O-D1 (f0_cell.sh, orkestratör):** `run_one` içinde, `kapi3_realized "$d" pre`'den önce,
  trace yoksa netem'i sıfırla: `kapi3_netem none none; kapi3_netem "$FWD" "$REV"` (kuyrukta kalan
  önceki koşu paketleri düşer, sayaçlar sıfırlanır). Gerekçe D-7. Analiz sayaç sıfırlanmasını
  tanır.
- **İSTEK-O-D2 (KATALOG §1):** `quinn` satırı → `rakip_arms` birincil (BBR, mesaj başına uni-stream,
  taşıma-ACK teslimi, `HS_TIMEOUT_S`); `tcp` satırı → transport-ack (`snd_una`). Eski tanımlar
  `quinn_rpl` / `tcp_rpl` betimsel.
- **İSTEK-B-D (Ajan B, `arms_kama.sh`):** dört kolda aynı alıcı damgası için Kama sunucusuna
  `KAMA_SRV_EXTRA="--rx-log @D@/rx_log.jsonl"` (şema ve dosya adı rakip kollarla aynı).
- Not (B/C): `rx_log.jsonl` birim kimliği: kama = `unit_id` (warmup önde; `--dup-copies N` ile
  mesaj×N+kopya), tcp = çerçeve `msg_id`, quinn = yükün ilk 8 baytı (`--payload-id`; yoksa
  `u64::MAX`). Karışık akış kendi günlüğünü (`mixed_rx.jsonl`, akış alanlı) yazar.
