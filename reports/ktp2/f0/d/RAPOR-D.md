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

## 9. TAM SONUÇ — aşama 1 (anlık görüntü 2026-10-30 08:09Z)

Kapsam: `runs/tam/d/out` (6 d_* hücresi) + `runs/tam/b/out`'ta DONE olan B hücreleri (r25 quinn
hücreleri `_quinn` sonekiyle taban profiline birleştirildi). Bekliyor (aşama 2):
`b_r1k_d300_l50_p1000`, `b_r1k_d300_l80_p1000`, `b_r25_d1000_l10_p65`, `b_r25_d1000_l30_p65`,
`b_r25_d300_l10_p65`, `b_r25_d300_l30_p65` (Kama/tcp kolları; quinn kolları geldi) ve D-13
doğrulama hücresi `d_tcp_gso1_r12k_d300_l30` (kuyruk sonrası). Bütün tablolar:
`reports/ktp2/f0/d/tam/TAM-OTOMATIK.md`. Kama sayıları bench üst sınırıdır [D-9: S1 `Reorder` ve Noise
baytları yok]; ürün yolu sayısı = Ajan A. tcp kolları [D-13]: veth TSO açık, kayıplar kısmen ikişerli.

### 9.1 Geçerlilik [ölçüldü]

- 51 ızgara/betimsel hücre + 1 karışık hücre; her kolda bütün koşular rc = 0, "düşen" ve
  "zaman-aşımı" 0. Çıktısı olmayan koşu 0.
- Koşu öncesi kuyruğu boş olmayan 3 koşu (her biri tek paket, 70–86 B; `b_r12k_d1000_l20_p65`
  taban rep1 ve tcp rep8, `b_r1k_d1000_l30_p1000` aday rep1); atılmadı, işaretli.
- Gerçekleşen kayıp profille tutarlı (Kama/quinn ileri ve dönüş koşu medyanları l10/l20/l30/l50/l80
  için 0,10/0,20/0,30/0,49–0,52/0,80–0,86). tcp ileri yönü D-13 nedeniyle düşük görünür
  (r12k l30: 0,26–0,29).
- El sıkışma yok (geçerli 0, ayrı sayıldı): quinn r25 bütün hücrelerde 5/5 (60 s tavan); r12k
  l30_p65 2/30; stres l50 p1000 1/10, l80 p1000 4/10, l80 p65 9/10, r1k_d300_l50_p65 3/10,
  r1k_d300_l80_p65 10/10 (teslim 0); tcp l80 p1000 4/10, p65 3/10. `d_hs_*` quinn_rpl 10/10 (eski 60 s
  kuralı).
- quinn UDP GSO etkisi ihmal edilebilir [çıkarım, sayaç oranından: quinn drops/(packets+drops) ≈ p
  bütün kayıplı hücrelerde; GSO olsaydı `packets` gso_segs saydığı için p'nin altında kalırdı].

### 9.2 Sonuç (S4) — ızgara, N = 30 / kol (stres N = 10, betimsel)

Yöntem: kol başına koşu-başı değerlerin medyanı; fark = medyan farkı, %95 aralık = koşu birimli
yüzdelik bootstrap (B = 2000, hücre|kol|kol|ölçü'den sabit tohum). Teslim oranı hükmü ön-kayıtlı
kuraldır (iki medyan GA'sı örtüşmüyorsa). Gecikme p95 = koşu-başı p95, D'de sansürlü. D = hücrenin
penceresi (1000 B → 60 s; 65 B → r1k/r12k 30 s, r25 60 s). Tam tablo: TAM-OTOMATIK §0 ve §3.

| karşılaştırma (36 birincil hücre: r12k/r1k × d300/d1000 × l0–l30 × 65/1000 B + r25 l0/l20) | teslim@D | gecikme p95 |
|---|---|---|
| taban [D-9] ↔ quinn | 32 ayrışmadı, 4 betimsel (r25: quinn N=5) | quinn düşük 22, Kama düşük 2 (r1k_*_l0_p65: −2,6 / −4,2 s), fark gösterilmedi 12 |
| aday [D-9] ↔ quinn | 32 ayrışmadı | quinn düşük 19, Kama düşük 3 (r1k_*_l0_p65, r1k_d300_l30_p1000 −11,4 s), fark gösterilmedi 14 |
| taban [D-9] ↔ tcp [D-13] | Kama üstün 6 (bütün l30 r12k + r1k_p65 l30), tcp üstün 4 (r25 l0/l20 × 2 gecikme), ayrışmadı 26 | Kama düşük 15 (l20–l30 ağırlıklı), tcp düşük 14 (l0–l10 ve r25), fark gösterilmedi 7 |
| aday ↔ taban | 36 ayrışmadı | aday düşük 6 (r12k_d300_l30_p1000 −3,9 s; r1k p1000 beş hücre −0,4…−12,7 s), 30 fark gösterilmedi |

En büyük gecikme farkları (Kama aleyhine, quinn'e karşı): r12k_d1000_l30_p1000 p95 taban 32,5 s /
aday 26,2 s ↔ quinn 8,7 s; r12k_d1000_l20_p1000 16,4 / 12,5 ↔ 5,8 s; r1k_d300_l20_p1000 22,8 / 11,7
↔ 11,6 s. Tel baytı / teslim edilen mesaj (netem, iki yön): r12k p1000 Kama 1130–1192 ↔ quinn
1200–1271 ↔ tcp 1112–1245 B; r1k p65 Kama 195–238 ↔ quinn 304–390 ↔ tcp 134–216 B.

### 9.3 K-2 kesişimi ve yüksek kayıp (§5.4)

- Birincil aralıkta (l0–l30) teslim@D medyanı Kama (taban, aday) ve quinn için hiçbir hız/gecikme/
  yük grubunda 0,90'ın altına düşmedi (`>l30`, üç görünümde de: gönderen / cold / alıcı). tcp her
  r12k ve r1k grubunda l30'da düştü (0,42–0,86). Ölçülen aralıkta Kama ile QUIC'in kesişim
  noktası arasında **fark gösterilmedi**; K-2'nin "QUIC'inkinden ileride" ölçütü bu aralıkta
  ayırt edici değil.
- r25 (65 B): Kama 0,17 (l0 ve l20, iki kol) → kesişim l0; quinn 0,00 (bağlanamadı); tcp 1,00 (l0, l20).
- Stres (N = 10, betimsel): r12k_d300 l80: taban 0,04 (1000 B) / 0,58 (65 B), aday 0,40 / 0,77,
  quinn 0,00 / 0,00, tcp 0,00 / 0,00. r1k_d300_l80_p65: taban 0,57, aday 0,68, quinn 0,00, tcp 0,00.
  l50: r12k p1000 taban 0,46 ↔ aday 0,99 ↔ quinn 1,00 ↔ tcp 0,05; r12k p65 Kama 1,00 ↔ quinn 1,00;
  r1k p65 taban 0,88, aday 0,99, quinn 0,80, tcp 0,01.
- **Korunması gereken avantaj (ölçülen hücrelerde):** %80 iki yönlü kayıpta Kama mesaj teslim etti,
  quinn ve tcp etmedi (quinn'in büyük kısmı bağlanamadı); %30'da tcp'ye karşı teslim oranı üstünlüğü
  (6 hücre). %50'de taban 1000 B'de quinn'in gerisinde, aday değil.

### 9.4 K-1 (karışık akış, taşıma düzeyi; N = 30)

| kol | TEXT p50 dosyalı | TEXT p95 dosyalı | Δ TEXT p95 (dosyalı − dosyasız) [%95] | PTT ≤0,8 s dosyalı / dosyasız | dosya bitiş s | tel/faydalı (dosyalı) |
|---|---|---|---|---|---|---|
| taban [D-9] | 9,97 | 24,50 | +19,60 [+15,64, +22,76] | 0,133 / 0,623 | 30,3 | 1,31 |
| aday [D-9] | 10,81 | 24,15 | +19,85 [+16,39, +22,76] | 0,080 / 0,387 | 27,4 | 1,31 |
| quinn | 3,22 | 12,80 | +10,46 [+9,39, +11,29] | 0,080 / 0,273 | 19,2 | 1,27 |
| tcp [D-13] | ∞ | ∞ | +23,57 [+0,00, +46,71] (60 s sansür) | 0,007 / 0,073 | ∞ (dosyanın %34'ü) | 1,18 |

- K-1 dört kolda da GEÇMEDİ. Dosyalı Kama − quinn: TEXT p95 +11,70 [+7,92, +14,71] s, dosya bitiş
  +11,09 [+6,92, +14,75] s, PTT ≤0,8 s +0,05 [−0,03, +0,12] (fark gösterilmedi). aday − taban: üç
  ölçüde de fark gösterilmedi.
- Ürün yolu (A, aynı profil, FARKLI iş yükü: 524,7 kB dosya, seyrek TEXT): TEXT p95 dosyalı taban
  107,1 s, aday 73,1 s; dosyasız 13,5 / 10,7 s; PTT ≤0,8 s dosyalı %0. Bench Kama sayıları bunun
  üst sınırıdır.

### 9.5 quinn el sıkışma

- r25 (B hücreleri, tavan 60 s): bağlanamayan oran 5/5 her hücrede (%95 Clopper–Pearson [0,48, 1,00]).
- r25 (`d_hs_*`, tavan 240 s, N = 10): 0/10 bağlanamadı; kurulum l0 100,0 s [99,9–100,0]
  (99,9–106,1), l20 100,1 s [100,0–112,3] (99,8–139,3); yine de 60 s içinde teslim 0 (PTO yoklama
  kuyruğu). Ön-kayıt "≈ 100 s" ile sapma +0…+12 %.
- r1k: 3,1 s (d300) / 4,5 s (d1000); r12k: 0,8 s / 2,3 s (l0 medyanları, §4 öngörüsü 3,1/4,5/0,8/2,2 s).

### 9.6 Bulgular (dört alan)

1. **D-T1 · r25'te Kama kendi trafiğiyle hattı dolduruyor.**
   - Ölçüm: b_r25_d300_l0_p65 ve b_r25_d1000_l0_p65 taban ve aday teslim@60 s = 0,167, 30/30 koşuda (alıcı tek-yön aynı); tcp 1,00 (N=30).
   - İleri tel ≈ 3665 B/koşu: hattın pencere boyu kapasitesi kadar. Döküm, taban/aday: data 552, retx 357/828, dup 2208/368, ctrl 1275/1275 B.
   - Dosya: `adaptive-transport/src/sender.rs:1485` `PRE_CONVERGE_FLOOR_BW = 31` (a858f3fc; aday :1672), kullanım :4243; kuruluş probu ve PARAM_SET: Ajan B B-K2, B-K8, B-K13.
   - Kök neden [ölçüldü: tel dökümü; çıkarım: sıra]: 31 B/s taban > 25 B/s hat, PARAM_SET + kopya + yeniden gönderim veri birimlerinin önünde netem kuyruğunda.
   - En dar düzeltme: B-K8 (taban ≤ ölçülen teslim hızı) + B-K13 (PARAM_SET aralığı) + B-K2 (kuruluş probu jetonlu).
2. **D-T2 · Kararlı durumda quinn'e karşı gecikme.**
   - Ölçüm: 36 birincil hücrenin 22'sinde quinn p95'i düşük. Örnek: r12k_d1000_l30_p1000 taban 32,5 / aday 26,2 ↔ quinn 8,7 s (Δ taban +23,8 s, aralık 0'ı dışlıyor). Teslim@D farkı yok.
   - Dosya: B'nin atfı `bw.rs:220-245`, `rate.rs:164-180`, `sender.rs:4239-4268` (B-K14 / B-C9).
   - Kök neden [çıkarım, B]: pacer sunulan hıza kilitleniyor, yeniden gönderim bandı ayrılmıyor.
   - En dar düzeltme: B'nin önerisi (Steady'de kurtarma bandı / kazanç). 05b kısmen: aday 6 hücrede tabandan düşük.
3. **D-T3 · Karışık akışta dosya TEXT'i bekletiyor (taşıma düzeyi).**
   - Ölçüm: §9.4. Kama dosyalı TEXT p95 +19,6 s; quinn'den +11,7 s fazla.
   - Dosya: motor yeni-veri FIFO'su (iş emri §3 A; Ajan A A-9) ve `mixed.rs` sunum sırası.
   - Kök neden [çıkarım]: tek FIFO, öncelik yok; Kama'nın dosya bitişi quinn'den 11 s uzun (teslim hızı).
   - En dar düzeltme: gönderen tarafında sınıf kuyrukları (A'nın F1 maddesi). quinn ölçümü gösteriyor ki yalnız gönderen önceliği K-1'i geçirmiyor (+10,5 s, darboğaz kuyruğu) [çıkarım].
4. **D-T4 · %80 kayıpta teslim.**
   - Ölçüm: stres hücrelerinde Kama > 0, quinn ve tcp = 0 (§9.3, betimsel N=10).
   - Dosya: el sıkışmasız kuruluş (`kama_product.rs` IdentitySeam yolu, D-9) ve kayıpta hızı düşürmeyen pacer (INV-RATE-1).
   - Kök neden [ölçüldü: quinn el sıkışma 4/10–10/10 başarısız].
   - En dar düzeltme: yok. Korunacak davranış; KTP-2 §3.5 "loss hızı düşürmez" ve el sıkışmasız akış korunmalı. Gerçek Noise SESSION_INIT maliyeti ölçülmedi (D-9).
5. **D-T5 · quinn 65 B r1k'da Kama'dan yavaş ve iki kat tel harcıyor.**
   - Ölçüm: r1k_d300_l0_p65 p95 quinn 5,6 ↔ taban 1,4 s; tel/mesaj 390 ↔ 195 B (N=30).
   - Dosya: `quic.rs` mesaj başına uni-stream (rakip yapısı).
   - Kök neden [çıkarım]: stream başına çerçeve + ACK/MAX_STREAMS yükü 1 kB/s'de belirgin.
   - En dar düzeltme: Kama tarafında yok. Kazanılan hücre olarak yazıldı.
6. **D-13 (tcp TSO), D-T6 (quinn_irtt kurulumu 4,6 s ↔ 4,3 s, betimsel, ön-kayıt sapması), D-1…D-12 (bench):** §3 ve §8.

Ön-kayıt sapmaları:
- "Cubic çöker" öngörüsü tcp için r12k_d1000_l10'da tutmadı (`d_cc`: tcp p95 8,0 s, tamam); r12k_d300_l30'da tuttu (tcp 0,43).
- Kama'nın r25'te 1/6'da kalacağı öngörülmemişti (B-K8 bulgusu).
- "Kama karışık akışta K-1'i taşıma düzeyinde geçmez" öngörüsü tuttu.

### 9.7 Sınıflandırma (ISEMRI §5)

| davranış | (i) 05b düzeltiyor mu (aday ölçümü) | (ii) dar mı / çekirdek mi | (iii) KTP-2 0.3 | (iv) korunacak avantaj |
|---|---|---|---|---|
| D-T1 r25 hat doldurma | hayır (0,167 her iki kol) | dar: sabit/pacer tabanı, PARAM_SET ve kuruluş probu kuralları; mimari değişmez | §3.5 hız, §3.9 düşük bant; K-3 (TASLAK-NOTLARI B'nin İSTEK-E1'i: 96 B → 134–140 B) | — |
| D-T2 Steady gecikme ↔ quinn | kısmen (6/36 hücrede aday düşük; en büyük farkta 32,5 → 26,2 s) | dar (pacer/kurtarma bandı, B) | §3.5, §3.6 | teslim@D eşit |
| D-T3 karışık akışta dosya HOL | hayır (aday − taban fark gösterilmedi) | gönderen sınıfları dar yapılabilir; alıcı sıralaması (A-2) ve darboğaz kuyruğu payı çekirdek/akış modeli ister [çıkarım] | §3.2 akışlar, §3.6 zamanlayıcı, §3.3 | — |
| D-T4 %80'de teslim | aday daha yüksek (0,40 ↔ 0,04 r12k p1000; betimsel) | — | §3.5, §3.10 | evet (ölçülen stres hücreleri) |
| tcp l30 HOL | — (rakip) | — | — | evet: l30'da 6 hücrede tcp'ye karşı teslim üstünlüğü |

### 9.8 F1 girdisi (D şeridinin gözüyle; uygulama B/A şeritlerinde)

| # | dosya / crate | değişiklik | bağımlılık | ölçü ve beklenen etki [tahmin] | süre |
|---|---|---|---|---|---|
| 1 | adaptive-transport `sender.rs:1485`, client-core `runtime.rs` (PARAM_SET), kuruluş probu | taban hızı ≤ ölçülen teslim; PARAM_SET seyrek; kuruluş probu jetonlu (B-K8/K13/K2) | B şeridi | K-3: r25 teslim@60 s 0,167 → ≥ 0,83. Dayanak: ileri tel 3665 B'nin 1275 B'ı ctrl, 2208 B'ı dup (taban); 6 × 140 B = 840 B, pencerede ≈ 1500 B kapasite kalıyor | 1–1,5 ajan-gün |
| 2 | adaptive-transport `rate.rs` / `bw.rs` (B'nin atfı) | Steady'de kurtarma bandı | 1 | K-2 p95: r12k_d1000_l30_p1000'de 32,5 s → quinn'in 8,7 s'sine doğru. Dayanak: teslim@D eşit, fark kuyrukta | 2 ajan-gün |
| 3 | client-core runtime sunum katmanı | sınıf başına sunum kuyruğu (PTT > TEXT > dosya) | A'nın planı | K-1 TEXT p95 dosyalı 24,5 s → ≤ quinn'in 12,8 s'si. Tek başına K-1'i geçirmez (quinn +10,5 s) | 2 ajan-gün |
| 4 | bench (ölçüm) | `--rx-log` bütün kollarda varsayılan hücre şablonu; tcp t0_unix kaydı | — | ölçüm doğruluğu (tcp tek-yön yaklaşıklaması kalkar) | 0,25 |

### 9.9 TABAN girdisi — S4 "Kama aynı koşullarda QUIC'in ve TCP'nin neresinde?"

- Ölçülen 36 birincil hücrede (12,5 kB/s ve 1 kB/s; RTT 0,6/2 s; kayıp %0–30; 65 B ve 1000 B; N = 30) bench Kama kolu (üst sınır; ürün yolu değil) ile quinn (BBR, mesaj başına stream, taşıma ACK'i) arasında pencere içi teslim oranında fark gösterilmedi; ikisi de 1,00.
- Gecikmede quinn 22 hücrede önde. En büyük fark 12,5 kB/s, RTT 2 s, %30, 1000 B'de: p95 32,5 ↔ 8,7 s. Kama 2 hücrede önde (1 kB/s, 65 B, kayıpsız).
- 05b adayı teslimde fark yaratmadı, gecikmede 6 hücrede tabandan düşük.
- tcp (tek bağlantı; D-13) %30'da 6 hücrede pencere içi teslimde Kama'nın gerisinde (0,42–0,85 ↔ 1,00). %0–10'da gecikmede önde.
- 25 B/s'de Kama 6 mesajın 1'ini teslim etti (iki kol, 30/30 koşu). Kendi kontrol, kopya ve yeniden gönderim trafiği hattı dolduruyor. tcp 6/6, quinn 0/6: QUIC el sıkışması ≈ 100 s, 60 s tavanda 5/5 bağlanamadı.
- K-2: birincil aralıkta Kama ile QUIC'in %90 kesişimi arasında fark gösterilmedi (ikisi de > %30); tcp %30'da kesişti.
- Stres %80'de (N = 10, betimsel) Kama 0,04–0,77 teslim etti, quinn ve tcp 0. Bu, korunması gereken ölçülmüş avantajdır.
- Karışık akışta (12,5 kB/s, %30) dört taşıma da K-1'i geçmedi. Dosya Kama'da TEXT p95'i 19,6 s, quinn'de 10,5 s uzattı.

### 9.10 Yeniden üretim

```
python3 -I scripts/ktp2/f0/d/tam_analiz.py ~/kama-k4g-work/ktp2-f0/runs/tam/d/out \
  ~/kama-k4g-work/ktp2-f0/runs/tam/b/out --out reports/ktp2/f0/d/tam \
  --product-note reports/ktp2/f0/d/tam/urun_yolu_A.md
python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <geçici> <a_r12k_d300_l30_{dosyasiz,dosyali} kökleri>   # ürün yolu satırı
```
sha256 (`reports/ktp2/f0/d/tam/SHA256SUMS`):
`5e480fcd…` TAM-OTOMATIK.md · `fc2b318a…` kosular.csv · `feb9b4c3…` karisik_kosular.csv.
İkililer: bench taban 2cd9f68d (3311d8e0…), aday bf18dc35 (c893e840…). Ham koşu dizinleri git'te değil.
