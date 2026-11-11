# CC-PORT — Eşleme Belgesi (CC-M0 çıktısı; ISEMRI-CC-PORT v3 §5)

Tarih: 11 Kas 2026 (gece çalışması). Kaynak: quinn-proto **0.11.18** (`congestion/bbr/*.rs`,
`connection/pacing.rs`, `connection/mod.rs`, `connection/spaces.rs`); lisans Apache-2.0 / MIT.
Hedef: `crates/adaptive-transport/src/cc/` + `sender.rs`/`receiver.rs` bağlama noktaları.
Bu belge NORMATİF DEĞİLDİR; kodun ne yaptığını ve neden sapıldığını kaydeder. Ölçüm (Ö-CC) ayrıdır.

**Motor seçimi çalışma zamanındadır**: `Sender::new()` / `Receiver::new()` = bugünkü motor
(bit-eşit; 458 test, 0 kırmızı); `Sender::with_cc_port()` / `Receiver::with_cc_port()` = port.
Cargo özelliği bilinçli olarak yoktur: CI'nın `--all-features`'ı motoru değiştiremez ve iki motor
aynı süreçte eşli koşar (`tests/cc_port_sim.rs`).

## (a) Fonksiyon eşlemesi

| quinn (dosya:satır, 0.11.18) | Kama'da yerine geçtiği / bağlandığı yer | veri yapısı farkı | uyarlama |
|---|---|---|---|
| `bbr/min_max.rs` MinMax | `cc/min_max.rs` | yok | birebir (quinn'in kendi testi dahil) |
| `bbr/bw_estimation.rs` BandwidthEstimation | `cc/bw_estimation.rs` | `Instant`→`u64 ns` | bayt muhasebesi kullanılır; **tahmin dış örnekleyiciden** (U6) |
| `bbr/mod.rs:26-61` Bbr durumu | `cc/bbr.rs` `Bbr` | `f32` kazanç → ‰ tamsayı | U2 |
| `:105-126` enter_startup / enter_probe_bw | aynı adlar | RNG ofseti yok | U3 (ofset 0) |
| `:128-157` update_recovery_state | aynı | paket numarası = Kama `seq` (iletim başına tekil) | — |
| `:159-200` update_gain_cycle_phase | aynı | — | — |
| `:202-211` maybe_exit_startup_or_drain | aynı | — | — |
| `:213-262` ProbeRTT | aynı | — | U8 (ilk örnek saati başlatır) |
| `:264-282` get_target_cwnd / probe_rtt_cwnd | aynı | µs → ns | — |
| `:284-307` calculate_pacing_rate | aynı | — | çıktı pacer'a bağlı (U7) |
| `:309-337` calculate_cwnd | aynı | `cwnd_gain < target as f32` birimsiz karşılaştırma olduğu gibi | — |
| `:339-369` calculate_recovery_window | aynı | — | referans davranış (kayıp filtresi deneyi bu PR'da YOK) |
| `:372-391` check_if_full_bw_reached | aynı | — | — |
| `:395-398` on_sent | `sender.rs::track_sent` → `cc.on_sent(now, size, seq)` | — | — |
| `:400-414` on_ack | `sender.rs::on_ack_received` (uçuş + tombstone kol-1/kol-2 kayıtları) | app_limited = gönderim anı damgası (`bw.rs::SentPacket`) | RTT min = `rtt.rtprop()` |
| `:416-466` on_end_acks | aynı yerde çerçeve sonunda | `largest_acked` = `loss.largest_acked()`; `in_flight` = `inflight_bytes` | app_limited ∨ "gönderilecek iş yok" |
| `:468-476` on_congestion_event | `sender.rs::queue_losses` (kayıp ilan edilen uçuş baytı) | — | — |
| `:485-492` window | `cc_window()` → `cc_fits(size)` kapısı (`try_send_pre_parity`, parite yolu) | **Kama'da uçuş kapısı yoktu** (yalnız hız) | yeni kapı |
| `pacing.rs:116-139` optimal_capacity | `cc/mod.rs::PacerParams::from_window` → `pacer.set_burst_cap` | — | — |
| `pacing.rs:92-95` 1,25·window/srtt | `PacerParams.rate_bps` — **yalnız bant tahmini yokken** | — | U7 |
| `connection/mod.rs:1700` 9/8·max(srtt, latest) | `E2aFlags::tt_rfc` (`with_cc_port` açar) + `cc_on()` | — | mevcut E2a m1 |
| `connection/mod.rs:1834-1870` PTO, backoff | `TailRecoveryConfig::rfc9002_cc_port()` (tavansız) + E2a `pto_probes = 2` | — | — |
| `poll_transmit` loss_probes cwnd'yi atlar | `try_send_probe` (pencere kapısı yok) | — | — |
| `send_buffer.rs:93-113` retx önce | `schedule_transmit`: `retx_first ∨ cc_on`; durum koşulu `e1_probe_keep ∨ cc_on` | — | — |
| `send_buffer.rs` tavansız retx | `RetxConfig::rfc9002_cc_port()` (`arq_retx_cap = u8::MAX`) | Kama: 3 sonra ileri-kopyaya devir | tavan kalktı |
| `spaces.rs:718-725` immediate_ack_pending | `receiver.rs::on_datagram` `out_of_order` → `tick_ack(force)` | — | — |
| `spaces.rs:674-676` max_ack_delay 25 ms | `receiver.rs::on_ack_timer` (cc_port kolu) | cadence 8,39 s ve kota yok | — |
| `cubic.rs:266` ilk pencere | `cc::rfc9002_initial_window(MTU)` = 10·MTU | BBR'ın 200·MTU'su ALINMADI | — |

**Beyanlı uyarlamalar (U1–U8)** — her biri `cc/bbr.rs` başlığında ve kodda satır atıflı:
U1 saat ns · U2 ‰ kazanç · U3 RNG yok · U4 RttEstimator yerine min RTT girdisi · U5 IW 10·MTU ·
**U6** bant tahmini Kama'nın BBR §7.1 paket-başına örnekleyicisinden (`bw.rs`); gerekçe: quinn'in
`min(send_rate, ack_rate)` tahmini çerçeve başına 2 paketlik ACK'te hızı ½ ölçer, ACK-toplanma
terimi cwnd'yi sınırsız büyütür (ölçüldü, `CC_TRACE`: Drain'de cwnd 21 k→76 k, 161 kuyruk düşmesi) ·
**U7** pacer hızı = BBR draft §4.2 `pacing_gain × BtlBw` (quinn'de yalnız metrik; pencere pacer'ı
Drain/ProbeBW kazançlarını yok sayar — ölçüldü: Drain hiç boşaltmaz, 60 düşme) ·
**U8** ProbeRTT saati ilk min_rtt örneğinde başlar (quinn ilk ACK'te ProbeRTT'ye giriyor; 1 s kayıp).
U6–U8 quinn'den değil BBR draft'ından alınmıştır; "quinn'e sadakat" değil "BBR'a sadakat"tir ve
Ö-CC'de kapalı/açık karşılaştırılabilir.
**U9** Startup'ta paket koruması (Linux BBR v1; `bbr.rs` başlığı, PR #179).
**U10–U13 (2026-11-11, r25 düşük bant kökü; PR #181)** — hepsi RFC 9002'ye *yaklaştırma*, ek sapma değil:
**U10** PTO probu hedefi uçuşta KALIR (`sender.rs` `emit_tail_probe` / `e2a_emit_second_probe`,
`!cc_on()` dalı eski yol). Eski yol hedefi `K_TOMB × T_loss(INITIAL_RTT)` = 8 × 9/8 × 100 ms ≈ 0,9 s TTL'li tombstone'a
emekli ediyordu; RTT örneği olmayan yüksek gecikmeli hatta ACK süresi dolmuş kayda düşüyor → RTT örneği
yok, teyit yok, her PTO turu yeni kopya (r25 sim: ilk RTT örneği 85 s'de). RFC 9002 §6.2.4 / quinn:
PTO kayıp ilanı değildir, `in_flight` değişmez · **U11** `rfc9002_cc_port` taban PTO tavanı yok
(`max_pto_ns = u64::MAX`; RFC 9002 §6.2.1 üst sınır tanımlamaz, quinn'de yok; eski 10 s tavanı srtt
> 10 s'de ACK gelmeden prob) · **U12** RTT örneği yokken tur başına TEK prob (§6.2.4 "SHOULD send
two" → "MUST send at least one"): serileştirme bilinmezken ikinci kopya kuyruğu kendi eliyle doldurur.
**U13** kayıp ilan edilen birimin DAHA TAZE kopyası (PTO probu) uçuştaysa yeniden kuyruk yok
(`queue_losses`, `cc_on`): kopya zaten yeniden iletimdir; quinn/RFC 9002 çerçeve düzeyinde
tekilleştirmez (çift gönderim). Kopya da kaybolursa kendi ilanı yeniden kuyruklar.
Sim r25 (25 B/s iki yön, 300 ms, %20, 60 × 65 B / 10 s, 10 tohum medyanı): port ileti p50 95 → 17
(U10+U11) → 11,4 (U12) → 8,5 s (U13), retx 87 → 28. Sim'in `Legacy` kolu (`Sender::new()`, kuyruk
kurtarması KAPALI, IW 0) 5,4 s / 17 — bu kol ürünün geri alma profili DEĞİLDİR; ürün eski profili
(`validated_v1` + IW 2·MTU) sim'de 106,8 s (H2 bağımsız incelemesi, PR #185; aynı kök: RTT örneksiz prob +
tombstone/Karn), bench netem'de ise 25–28 s → sim bu kolu abartır, doğru taban bench K0 / gerçek yığın.
Öbür hücreler bayt düzeyinde aynı (kesinti p50 84,8 → 84,3; U13 kesinti seed1 retx 83 → 82).

## (b) Paket/birim muhasebesi — koddan cevaplar

| olay | cevap (kod) |
|---|---|
| bir birimin birden fazla iletimi | Kama her iletime **taze `seq`** verir (`track_sent`: `next_seq += 1`), `inflight` seq-anahtarlı. Dolayısıyla quinn'in paket numarası = `seq`; uçuş kaydı **iletim başına**. `cc.on_sent` her iletimde, `cc.on_ack` yalnız ilk kez onaylanan iletim kaydında (uçuş ya da tombstone) çağrılır. |
| geç veya tekrarlanan SACK | `AckInfo.acked_seqs` çağıranda tekilleştirilir (`udp_runner`, sim: `acked_seen`); gönderici tarafında `inflight.remove`/`tombstones.remove` tek kez başarılı olur → teslim baytı ikinci kez sayılmaz. Un-loss (kayıp ilan edilmiş seq'in ACK'i) `cc.on_ack`'a **girmez** (quinn: kayıp ilan edilen paket `sent_packets`'tan düşer). |
| FEC ile kurtarma | `RecoveredControl` motor çekirdeğine girmez (N8); CC'ye de girmez. Teslim hızı yalnız wire-ACK'ten. Kurtarılan birimin orijinal iletimi kayıp sayılmaya devam eder (quinn eşdeğeri yok; beyanlı). [K4G-05b-R, 2026-11-11] RECOVERED birimi teyit eder: uçuştaki iletimi aşağıdaki satırdaki gibi kararsız işaretlenir (uçuş baytı ve CC girdisi DEĞİŞMEZ; kaybı yine `on_congestion_event`'tir), yalnız yeniden gönderilmez. |
| teyitli birimin diğer iletimleri [K4G-05b-R, 2026-11-11] | Birim bir iletiminin ACK'iyle ya da RECOVERED ile teyit edilince uçuştaki DİĞER iletimleri **uçuşta kalır** ve kararsız işaretlenir (`InflightPacket::confirmed_release_ns`): ACK'leri `cc.on_ack`'a, kayıp ilanları `on_congestion_event`'e öncekiyle aynı girer; yalnız yeniden kuyruğa, kopyaya ve prob hedefine girmezler (quinn: verisi başka paketle onaylanmış paket `sent_packets`'ta kalır, ACK'te `on_packet_acked`, kayıpta `on_congestion_event`; kaybında o akış verisi yeniden gönderilmez). **Beyanlı uyarlama:** ACK'i de kayıp ilanı da teyitten `K_TOMB × T_loss` içinde gelmeyen kararsız iletim kayıp ilan edilmeden uçuştan düşer, tıkanıklık olayı yoktur. Bu başlıca sessiz kanalda (daha yeni ACK'lenmiş seq yok) ve PreConverge'ün kayıp-ilanı yasağında olur. Daha yeni seq ACK'lenmişken dedektör genellikle önce ilan eder; istisna dar bir penceredir: paket eşiği dolmamış (`la − seq` < reorder eşiği) VE zaman eşiği teyitten bu yana en az `K_TOMB` kat büyümüş (bırakma anında geçen süre en az `K_TOMB × T_loss(teyit anı)`'dır; eşik bu kadar ancak kuyruk birikimiyle, tek bir geç `latest_rtt` örneğiyle ya da ACK aralığının büyümesiyle büyür). O pencerede gerçekten kaybolan iletim tıkanıklık olayı doğurmaz; dedektör seq'i sonra yine ilan edebilir (yalnız kayıp metriği, uçuş kaydı olmadığından CC'ye girmez). Bırakma anı EK4-A2 uyanma adayıdır (`next_wakeup_ns`). quinn'de PTO probu (PING) sonraki ACK'le kaybı ilan ettirirdi; Kama'da sessizlik kayıp değildir (TUR-1a) ve teyitli birime prob kararı üretilmez. Sonuç TTL'ini doldurmuş tombstone ile aynıdır (sonraki ACK'i bilinmeyen seq). |
| parite ve prob | Parite `track_sent` (origin `FecParity`) → `cc.on_sent`; ACK'lenirse `cc.on_ack`; uçuş penceresine tabi (`cc_fits` parite yolunda). PTO probu `track_sent` ile uçuşa girer ama **pencereden muaf** (`try_send_probe`). Tombstone'a emekli edilen orijinal uçuştan düşer, kaybı ilan edilmez (quinn PTO ile aynı). [K4G-05b-R] Prob hedefi en eski teyitsiz KAYNAK birim kaydıdır: parite (`unit.id` = ESI) ve kararsız iletim hedef olamaz; uygun kayıt yoksa prob yok. |
| ACK üretimi | Alıcı: her 2 ack-eliciting datagram (`ACK_FREQ_N`) ya da sırasız varış → anında; 25 ms zamanlayıcı. SACK'in kendisi uçuşa girmez (tek yönlü motor; dönüş yolu ayrı). ACK'e ACK yok. |

## Test matrisi

| koşu | komut | sonuç (11 Kas gece) |
|---|---|---|
| legacy + port, tüm crate | `cargo test -p adaptive-transport` | 458 geçti, 0 kırmızı |
| sim kapısı (iki motor eşli) | `cargo test -p adaptive-transport --test cc_port_sim -- --nocapture` | 6/6 |
| keşif matrisi | `CC_MATRIX=1 … matrix_explore` | tablo aşağıda |
| bağımlı crate'ler | `cargo test -p kama-engine -p client-core` | 232 geçti |
| clippy / fmt | `--all-targets -D warnings`, `fmt --check` | temiz |

Sim (150 kB dosya, 1200 B birim, 10 seed; sayılar **sim**, netem değil):

| hücre | legacy teslim · p50 · gönderim/yük | port teslim · p50 · gönderim/yük |
|---|---|---|
| q6 12,5 kB/s 300 ms %0 | 10/10 · 13,2 s · 1,08 | 10/10 · 12,3 s · 1,38 (48 kuyruk düşmesi — Startup taşması) |
| q6 12,5k 300 ms %20 | 7/10 · 16,4 s | 10/10 · 15,8 s · 1,54 |
| q6 12,5k 300 ms %30 | 6/10 · 33,5 s · 1,43 | 10/10 · 19,3 s · 1,68 |
| q6 12,5k 1000 ms %20 | 7/10 · 22,1 s | 10/10 · 18,4 s · 1,50 |
| q6 3 kB/s 600 ms %25 | 6/10 · 75,8 s | 10/10 · 71,6 s (taban 50 s) |
| q6 50 kB/s 100 ms %10 | 10/10 · 6,5 s (max 45 s) | 10/10 · 3,5 s (max 3,6 s) |
| q1000 12,5k 300 ms %20 | 7/10 · 17,4 s | 10/10 · 15,6 s · 1,23 |
| q1000 12,5k 300 ms %30 | 7/10 · 29,6 s | 10/10 · 18,7 s · 1,42 |
| FEC: 2 parite / 8 birim | 8/30 tele çıktı | 30/30 |
| PTT 6×400 B q6 %20 | 0,64 s | 0,34 s |

Legacy'nin sim'deki ölümleri: kalkışta (PreConverge + 31 B/s) ve kuyrukta (124/125 — PTO kapalı,
retx tavanı 3 → ileri-kopya kuyruğu 40 s). Sim legacy'nin netem'deki 91 s çöküşünü aynen
üretmiyor (13 s) — harness netem değil; Ö-CC gerçek hücreyi koşar.

## Bilinen açık noktalar (PR-1 kapsamı dışı, Ö-CC'ye not)
- Startup taşması sığ kuyrukta kayıpsız hücrede 48 düşme (%38 ek gönderim) — BBR v1 doğası; draft'ta
  Startup çıkışı kayıpla da tetiklenir (kodda var: `in_recovery` → full bw). 
- Kayıp filtresi deneyi (`cc_loss_filter`) ve düşük bant kipi bu PR'da YOK (v3 §3'e uygun: deney).
- `RecoverFwd` ileri kopya mekanizması (fuzzy acil durum) port altında da çalışır; CC dışı.
- Legacy pin testleri `Sender::new()` ile koştuğundan dokunulmadı; port için ortak-doğruluk
  testleri `tests/cc_port_sim.rs`'te; CC-M5'te genişletilir.
