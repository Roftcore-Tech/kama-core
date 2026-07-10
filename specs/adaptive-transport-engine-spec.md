# SafeRoom — Adaptive Transport Engine Spec (Implementable)

**Belge:** `adaptive-transport-engine-spec.md` v0.1 — Alpha
**Statü:** IMPLEMENTATION SPEC (kod blueprint'i; `transport-layer-tdd.md` mimari/policy otoritesinin altında)
**Bağlam:** `transport-layer-tdd.md` (Architecture & Alpha Policy TDD) *ne* inşa edileceğini dondurdu.
Bu belge *nasıl* — state machine → struct, ACK frame → wire layout, estimator → fonksiyon.
Rust-native (ADR-01), Tokio (ADR-02), reed-solomon-erasure (ADR-16), deterministik fuzzy (ADR-17).

> **Otorite sırası:** `transport-layer-tdd.md` (WHAT/policy) > bu belge (HOW/engine). Çelişki olursa TDD kazanır; bu belge düzeltilir.
> **Sabit disiplini (NORMATİF):** Tunable sabitler `[CAL]` = netem baseline'dan (ADR-18) kalibre edilir, UYDURULMAZ. `[STD:kaynak]` = literatürden alınmış principled başlangıç (ör. BBR default). İkisi de rapora ÖLÇÜLMÜŞ/kaynaklı yazılır (§4.4.0 magic-number yasağı).

---

## 0. Kapsam, Konum ve Rate-Control Ailesi (NORMATİF)

### 0.1 Reliability topolojisi — HOP-BY-HOP

```
Alice ──[reliable transport hop 1]──> Hub ──[reliable transport hop 2]──> Bob
                                       └─ mailbox (store-and-forward, redb; ADR-13)
```
- Transport reliability **hop-scoped**: Alice↔Hub ve Hub↔Bob ayrı reliable hop'lar. Hub, envelope'u mailbox'a yazar → forward.
- E2E kripto (L3 Session Model V3 envelope) **uçtan uca**; transport hop'ları envelope'u OPAK taşır.
- **ACK anlamı:** Alice'in transport ACK'i = "Hub envelope'u aldı (mailbox'a yazıldı)", "Bob okudu" DEĞİL. Bob-teslim/okundu app-katmanı E2E receipt (ayrı, bu belgenin kapsamı dışı).
- Rate controller HER hop'ta ayrı çalışır (endpoint bir sonraki hop'a kendi ölçtüğü bw/loss/rtt ile pace eder).

### 0.2 Transport ⟂ Kripto Ayrımı (NORMATİF — SoC, çekirdek ilke)

Transport engine paket TÜRÜYLE ve KRİPTOYLA İLGİLENMEZ. Görevi tek: opak, **güvenliği zaten sağlanmış** birimleri hedefe ulaştırmak. **AEAD TUTMAZ, anahtar YÖNETMEZ, şifrelemez / çözmez / verify etmez.**

```
GÜVENLİK (ayrı katman)  : Noise handshake + kompakt session design (Session Model V3; TDD §0.1/§3, ccr §35).
                          TÜM AEAD burada yürütülür: inner E2E envelope + control/shard outer-AEAD +
                          code-then-encrypt-authenticate-shards (INV-FEC-1).
TRANSPORT (bu belge)    : opak birimleri PACE / DELIVER / TRACK eder; reliability için METADATA okur
                          (seq / block_id / shard_index — güvenlik katmanı bunları AAD olarak authenticate eder);
                          loss / bw / rtt / jitter ÖLÇER; FEC/ARQ/rate POLİTİKASINI belirler.
```

Sonuçlar (NORMATİF):
```
- seq: transport reliability için OKUR; ama seq güvenlik katmanının AAD'sidir → tampering AEAD verify'ı kırar
       → güvenlik katmanı drop eder. Transport seq'i optimistik kullanır, DOĞRULAMAZ.
- ACK: İÇERİĞİ transport üretir (hangi seq'ler geldi) ama client↔hub Noise-secured kanalın İÇİNDE taşınır
       → forge-koruması Noise'dan gelir, transport'tan DEĞİL. Transport ACK'e AEAD UYGULAMAZ; güvenlik katmanı uygular.
- FEC: POLİTİKA (ne kadar parity, §8.4 fuzzy — ölçülen loss'tan) transport'un;
       EXECUTION (RS-encode + per-shard AEAD, code-then-encrypt-authenticate-shards) GÜVENLİK katmanının.
       Transport parity ORANINI verir; güvenlik katmanı encode+AEAD yapar; transport opak shard'ları pace/track eder.
- "Pipeline D outer-AEAD" (TDD §0.1) control/shard için EXECUTE edilir ama GÜVENLİK katmanında;
       bu belge o AEAD'i transport-owned olarak REDEFINE ETMEZ (yalnız birimi opak taşır).
```

> **INV-ENGINE-SOC:** Transport engine hiçbir AEAD/anahtar tutmaz; kripto = Noise + kompakt (ayrı katman). Transport **type-agnostic + crypto-agnostic**: yalnız opak birim iletir + metadata okur + ölçer + policy verir. Güvenlik birimi zaten güvenli teslim eder; transport "ne olursa olsun ulaştırmaya" çalışır.

### 0.3 Rate-Control ailesi — SRC (SafeRoom Rate Control), model-based

```
SRC = BBR-türevi delivery-rate model çekirdek + FEC/ARQ füzyonu + forward-recovery.
  ÜÇ ORTOGONAL SİNYAL (asla birbirine indirgenmez):
    bandwidth  := BtlBw  = max-filter(delivered_rate)     [ÖLÇÜLÜR; loss'tan TÜRETİLMEZ]
    congestion := queue_delay = srtt − RTprop             [delay-based sinyal; loss DEĞİL]
    loss       := loss_pre_fec / residual_loss (sayaç)    [recovery kolunu besler; RATE'İ DÜŞÜRMEZ]
```
- **INV-RATE-1 (TDD):** loss send_rate'i AZALTMAZ. SRC yalnız `queue_delay` (delay) + `inflight_cap` ile pace eder.
- BBR'ın ölçüm çekirdeği ALINIR; BBR ProbeBW/ProbeRTT gain-cycling state machine ALINMAZ (stabil-path varsayar) → §8 gentle-probe.
- BBR'da olmayan iki kol EKLENİR: (a) aynı state'ten FEC/ARQ arbitrasyonu (§8.4), (b) ack_confidence LOW'da forward-recovery (§11).

---

## 1. Packet Classes (NORMATİF)

Wire'daki her datagram tek bir class'a aittir. Class, ilk byte'tan (ya da hub_handle demux'undan sonra frame_type'tan) ayrışır.

```
CLASS               taşır                                       güvenlik (AYRI katman*)  ref
──────────────────────────────────────────────────────────────────────────────────────────
SESSION_INIT        zero-RTT E2E bootstrap (ss_e+ss_s)          inner E2E                §3,§1
SESSION_INIT_RESP   responder ephemeral (epoch-0→1 FS-upgrade)  inner E2E                §1
SESSION_DATA        hot-path: hub_handle+seq+ct+tag (22B+pl)    inner E2E tag            §2
SIGNED_CONTROL      SESSION_DATA + Ed25519 64B (non-repud.)     inner + Ed25519          §1
─── transport-control (client↔hub hop; güvenlik katmanı outer-AEAD uygular, §0.2) ──────────
ACK_FRAME           SACK feedback (§2)                          hop outer-AEAD*          §12 TDD
CTRL_FRAME          MTU probe / path-probe / rekey-signal       hop outer-AEAD*          §11 TDD
FEC_SHARD           block data/parity shard                     per-shard AEAD*          §7 TDD, INV-FEC-1
```
> **\* Tüm güvenlik (AEAD) NOISE+KOMPAKT katmanda yürütülür (§0.2); transport bu birimleri OPAK görür — tuttuğu/uyguladığı AEAD YOK.** Bu tablo "güvenlik nereden geliyor"u gösterir, transport'un ne yaptığını değil.

- SESSION_DATA hot-path'e ACK/FEC header, transport-prefix, imza EKLENMEZ (TDD §2 INV-WIRE). ACK ayrı CLASS.
- FEC_SHARD yalnız block/shard framing MEVCUTSA (segment-block pipeline); tek-datagram'da FEC yok (TDD §7).
- Transport için hepsi opak "birim"dir; transport yalnız seq/block_id/shard_index metadata'sını okur (§0.2).

### 1.1 Wire class tag (hub_handle sonrası)

```
transport-control frame ilk byte = frame_type:
  0x10 ACK_FRAME
  0x11 CTRL_MTU_PROBE
  0x12 CTRL_PATH_PROBE
  0x13 CTRL_REKEY_SIGNAL
  0x20 FEC_SHARD_HEADER
(SESSION_* class'ları hub_handle demux + E2E envelope tipinden ayrışır; transport-control frame_type güvenlik katmanı outer-AEAD-çözümünden SONRA görünür — çözümü GÜVENLİK katmanı yapar, §0.2.)
```

---

## 2. ACK / SACK Frame Format (NORMATİF — fixed-width, ccr §35 varint YASAK)

SafeRoom `seq` = transport hop seq, **u16** (TDD §2 SESSION_DATA seq 2B). ACK bu seq uzayında çalışır.

```
ACK_FRAME {                     # içeriği transport üretir; client↔hub Noise-secured kanalda taşınır (güv. katmanı, §0.2)
  frame_type    : u8            # 0x10
  ack_epoch     : u8            # rekey/counter-wrap epoch (largest_seq bu epoch'ta)
  largest_seq   : u16_be        # alınan en yüksek seq (bu epoch)
  ack_delay     : u16_be        # µs; largest_seq alımı → bu ACK gönderimi (RTT düzeltmesi, §6)
  block0_len    : u16_be        # en üstteki ardışık-alınan blok uzunluğu (largest dahil)
  range_count   : u8            # EK gap+blok çifti sayısı (0 = yalnız cumulative/block0)
  ranges[range_count] {
    gap         : u16_be        # bir önceki bloğun altındaki KAYIP seq sayısı
    len         : u16_be        # ardışık-alınan blok uzunluğu
  }
}
# Boyut: 9B sabit + 4B × range_count. Varint YOK (fixed-width big-endian, ccr §35).
```

**Anlam (descending — QUIC ACK yapısına benzer, fixed-width):**
```
Block 0 (acked): [largest_seq − block0_len + 1 .. largest_seq]
i≥1 için:
  gap_i kadar seq KAYIP (henüz alınmadı)
  Block i (acked): gap+len ile bir alt bloğu tanımlar
Rekonstrüksiyon:
  cursor = largest_seq − block0_len + 1
  her range: cursor -= gap_i;  block_i = [cursor − len_i + 1 .. cursor];  cursor -= len_i
```

**seq-wrap (u16) rekonstrüksiyonu (NORMATİF):**
```
Wire seq 16-bit; full_counter 64-bit sender'da monoton. Rekey wrap'ten ÖNCE (TDD: counter wrap→rekey);
ack_epoch rekey sınırını disambiguate eder. Decoder full seq'i "expected next"e en yakın aday ile çözer:
  candidate = (expected_high_bits << 16) | wire_seq
  |candidate − expected| minimize edecek high-bits seçilir (QUIC packet-number decode analog).
Farklı ack_epoch → farklı key domain; cross-epoch ACK reddedilir.
```

### 2.1 ACK üretim kuralları (receiver)
```
- ACK-tetik: max(ACK_FREQ_N alınan DATA, ACK_TIMER_T) hangisi önce (§12).   [CAL]
- range_count ≤ MAX_ACK_RANGES; taşarsa en yeni (en yüksek seq) bloklar öncelik.   [CAL: MAX_ACK_RANGES ~16]
- ack_delay = now − largest_seq_recv_time (µs).
- Reverse-path impaired (ack_confidence LOW) → ACK decimation (§11): ACK_FREQ artar, ACK-only budget düşer.
```

---

## 3. Sender State Machine (NORMATİF)

```
Durumlar:
  IDLE            → gönderilecek yok; timer'lar pasif
  STARTUP         → BtlBw hızlı keşif (pacing_gain yüksek), path doldur
  STEADY          → pace = BtlBw; periyodik BW/RTprop probe
  RECOVER_ARQ     → residual loss var + ack_confidence OK → selective retransmit (§9)
  RECOVER_FWD     → ack_confidence LOW → forward redundancy (§11), ACK-bağımsız
  DRAIN_RTPROP    → RTprop probe: inflight düşür, min-RTT ölç (§8.3)

Geçişler (event-driven, deterministik):
  IDLE   --data-hazır-->                 STARTUP
  STARTUP--BtlBw plato (Δ<ε, N round)--> STEADY
  ACTIVE --ack_confidence LOW----------->  RECOVER_FWD  --ackconf≥MED--> STEADY   [AMENDMENT-1]
  ACTIVE --residual_loss>0 ∧ ackconf≥MED-> RECOVER_ARQ  --temiz-------> STEADY   [AMENDMENT-1]
  STEADY --rtprop_probe_due------------->  DRAIN_RTPROP --min-rtt ölçüldü--> STEADY
  (any)  --idle > IDLE_TIMEOUT---------->  IDLE
  ── ACTIVE = {STARTUP,STEADY,RECOVER_ARQ,RECOVER_FWD,DRAIN_RTPROP} (IDLE hariç tüm aktif fazlar).
     Değerlendirme önceliği (her event): IDLE(iş yok) > RECOVER_FWD(ackconf LOW) > RECOVER_ARQ(pending
     loss) > faz-içi(STARTUP plato / STEADY / DRAIN_RTPROP).
```

> **[AMENDMENT-1] (impl-driven, gerekçeli — kod-spec tutarlılığı):** Recovery (FWD/ARQ) yalnız STEADY'den
> DEĞİL, her ACTIVE fazdan erişilir. **Gerekçe:** ACK-yolu çökerse (p5_revpath baseline — quinn c(10s)=0.00)
> STARTUP BtlBw platosuna ULAŞAMAZ (ACK yok → btlbw büyümez) → STEADY erişilemez → RECOVER_FWD KİLİTLENİRDİ;
> ayrıca loss STARTUP sırasında olabilir (yaygın) → retransmit tıkanmamalı. forward-recovery zaten
> ACK-BAĞIMSIZ (§11) — faza bağlı olmaması, recovery-obsesif taban ("veri ULAŞSIN") ile tutarlı. Öncelik
> RECOVER_FWD > RECOVER_ARQ: ackconf LOW iken ARQ futile (ACK dönmüyor), forward-recovery devralır.

**Sender ana döngü (her event):**
```
on_event(e):
  match e:
    PacketSent(pkt)  -> §3.1 OnPacketSent
    AckReceived(ack) -> §3.2 OnAckReceived
    TimerFired(t)    -> §12 timer handler
    AppData(bytes)   -> enqueue(tx_queue); if state==IDLE: state=STARTUP
  ClassifyPath()        # §5
  (fec_ratio, arq_aggr, send_rate, compress) = FuzzyEvaluate()   # §8.4, §15
  ClampRecoveryBudget()  # §8.5
  UpdatePacingRate(send_rate)  # §13
  ScheduleTransmit()     # pacing token bucket'a göre DATA / retransmit / FEC / duplication
```

### 3.1 OnPacketSent(pkt)
```
pkt.send_time = now
pkt.seq       = next_seq++        # u16 wire, full_counter 64-bit
pkt.size      = wire_bytes
inflight_bytes += pkt.size
append(sent_packets, pkt)          # RTT/loss/delivered muhasebesi için
mark_app_limited_if(tx_queue boş ∧ inflight < cwnd)   # §7.3 app-limited işareti
arm_pto_timer()                    # §12
```

### 3.2 OnAckReceived(ack)
```
newly_acked = resolve_acked_ranges(ack, sent_packets)   # §2 rekonstrüksiyon
for p in newly_acked:
    inflight_bytes -= p.size
UpdateRTT(ack, newly_acked)             # §6
UpdateDeliveredRate(newly_acked, ack)   # §7
DetectLoss(ack)                         # §5 (loss = SİNYAL DEĞİL rate için; recovery için)
free(acked packets)
reset_pto_timer()
```

---

## 4. Receiver State Machine (NORMATİF)

```
Durumlar:
  RX_NORMAL       → in-order/near-order teslim; windowed ACK
  RX_REORDER      → gap var, reordering_threshold içinde → bekle (spurious-rtx önle, §9.3)
  RX_FEC_DECODE   → block/shard framing; eksik data shard'ları parity'den reconstruct (§10)
  RX_DUP_SUPPRESS → duplicate/retransmit geldi, zaten teslim → drop + ACK teyit

Akış:
  on_datagram(d):
    # GÜVENLİK katmanı (Noise+kompakt, §0.2) d'yi çözer/verify eder; transport VERIFIED opak birim + metadata alır.
    # (verify FAIL → güvenlik katmanı silent drop, TDD §15; transport bunu hiç görmez.)
    if SESSION_DATA: inner-E2E teslim path (E2E verify güv. katmanında)
    if FEC_SHARD:
        # per-shard AEAD verify (code-then-encrypt-authenticate-shards) GÜVENLİK katmanında yapılır;
        # unauthenticated shard FEC decoder'a ASLA girmez (INV-FEC-1, authenticate-before-decode).
        # transport yalnız AUTHENTICATED plaintext shard + (block_id/shard_index/k/r) metadata'sını alır:
        buffer shard; if block reconstructable: RX_FEC_DECODE → reconstruct → teslim
    update rx_bitmap (alınan seq'ler — metadata)
    if gap ∧ within reordering_threshold: RX_REORDER (bekle)
    if gap ∧ beyond threshold: mark missing → ACK'te SACK range
    schedule_ack()   # §2.1, §12
    duplicate → RX_DUP_SUPPRESS: teslim etme, yalnız ACK (§9.4)
```

- Teslim sırası: hop transport in-order teslim GARANTİ ETMEZ zorunlu değil (E2E envelope seq'i uygulama sıralar); ama block reconstruction için shard toplama gerekir. Reorder toleransı §9.3.

---

## 5. Loss Detection (NORMATİF — loss = recovery sinyali, RATE sinyali DEĞİL)

QUIC RFC 9002 packet-threshold(3)+time-threshold(9/8) **loss-based CC** içindir; biz loss'u rate'e bağlamayız (INV-RATE-1). Loss'u yalnız **recovery** (ARQ/FEC) ve **residual ölçüm** için tespit ederiz.

```
Packet kayıp SAYILIR (recovery için) eğer:
  (a) PACKET_THRESHOLD: daha yüksek seq'li ≥ K paket ACK'lendi (reordering toleransı).   K=[STD:BBR/QUIC 3]
  (b) TIME_THRESHOLD:   send_time + max(TIME_THRESH × srtt, min_delay) < now.
                        TIME_THRESH=[STD:9/8]; min_delay=[CAL] (reordering tail).
loss_pre_fec = lost_original_units / sent_original_units        (pencere W_loss)   [CAL: W_loss]
residual_loss = unrecovered_units / sent_units                  (FEC/ARQ SONRASI)
→ loss_rate fuzzy girdisi = fuzzify(loss_pre_fec)  (LOW/MED/HIGH, §15)
→ NOT: bu değerler send_rate'i düşürmez; fec_parity_ratio ve arq_aggressiveness'ı besler.
```

**Reordering threshold (adaptif, §9.3):** `reorder_thresh = max(K, ρ × recent_max_reorder)` — yüksek jitter'da spurious retransmit önler.  ρ=[CAL].

### 5.1 ack_confidence — reverse-path canlılık sinyali (NORMATİF)

`ack_confidence` §3 (state geçişleri) · §8.4/§15 (fuzzy girdi) · §11 (forward-recovery tetiği) tarafından
TÜKETİLİR ve §8.1 SrcState'te tutulur; bu alt-bölüm hesabını KİLİTLER (önceden tanımsızdı — [AMENDMENT-2]).

```
ANLAM: reverse (ACK/dönüş) yolunun canlılığı, per-mille. YÜKSEK = ACK'ler bekleneni sağlıyor;
       DÜŞÜK = ACK SESSİZLİĞİ (dönüş yolu bozuk/çökmüş → ARQ'ya güvenilemez → forward-recovery).

TANIM (sessizlik-tabanlı, NORMATİF):
  active = (inflight ∨ pending_recovery ∨ tx_queue) boş DEĞİL      # bekleyen teyit var mı
  active DEĞİLSE → ack_confidence = 1000 (FULL)                    # outstanding iş yok, sessizlik anlamsız
  aksi:
    silence   = now − max(last_ack_time, first_send_time)          # hiç ACK yoksa ilk gönderimden
    threshold = ACK_SILENCE_MULT × srtt                            # [CAL: ACK_SILENCE_MULT ~3]
    silence ≤ threshold → 1000
    silence > threshold → clamp(threshold·1000 / silence, 0..1000) # monoton azalan
```

**NEDEN oran (sent/received) DEĞİL:** yeni gönderilmiş paket "düşük güven" DEĞİLDİR — henüz ACK'lenme
zamanı gelmemiştir; yalnız SESSİZLİK reverse-path çöküşüdür. Naif sent/received oranı taze gönderimi
yanlışlıkla LOW'a çeker → STARTUP'ta anında sahte-emergency. Sessizlik modeli bunu ayırır (last_ack
gelince silence sıfırlanır). Somut `ACK_SILENCE_MULT` [CAL] netem baseline'dan; mekanizma [STD].

> **[AMENDMENT-2] (impl-driven, gerekçeli — kod-spec tutarlılığı):** Sinyal §8.1 state alanı +
> §3/§8.4/§11 tüketicisiydi ama HESABI hiçbir yerde tanımlı değildi. İmplementasyonda somutlaşınca
> (naif oran → sessizlik modeli düzeltmesi) semantiğiyle spec'e eklendi. `residual_loss` ile birlikte
> §11 `health = f(ack_confidence↑, residual_loss↓)`'i besler.

---

## 6. RTT / Jitter / Queue-Delay Estimator (NORMATİF — formüller)

```
Girdi: her acked paket için rtt_sample = ack_recv_time − pkt.send_time − ack.ack_delay
       (ack_delay çıkarılır; negatifse ack_delay untrusted → sample invalid, §7.3)

srtt / rttvar (RFC 6298 türevi, EWMA):
  ilk sample: srtt = rtt_sample;            rttvar = rtt_sample / 2
  sonraki:    rttvar = (1−β)·rttvar + β·|srtt − rtt_sample|      β=[STD:1/4]
              srtt   = (1−α)·srtt   + α·rtt_sample                α=[STD:1/8]

RTprop (propagation, min-filter — BBR-türevi):
  RTprop = min(rtt_sample) over window W_rtprop     W_rtprop=[STD:BBR 10s]
  (pencere kayan; DRAIN_RTPROP probe ile tazelenir, §8.3)

queue_delay (congestion sinyali — delay-based):
  queue_delay = srtt − RTprop            (≥0; buffer birikimi = congestion)
  → fuzzify(queue_delay) LOW/MED/HIGH (§15). BU congestion sinyalidir, loss DEĞİL.

jitter / reordering score:
  jitter_score     = rttvar_normalized = rttvar / max(srtt, ε)
  reordering_score = EWMA(out_of_order_arrivals / total_arrivals)     [CAL: EWMA α_reorder]
  → fuzzify(max(jitter_score, reordering_score)) → jitter girdisi
```

---

## 7. Delivered-Rate Bandwidth Estimator (NORMATİF — "ana yetenek"; BBR-türevi)

Bu bölüm belgenin çekirdeği: **bandwidth loss'tan türetilmez, delivered-rate'ten ÖLÇÜLÜR.**

### 7.1 Rate sample (paket-bazlı, BBR delivery-rate)
```
Her paket gönderilirken sakla:
  pkt.delivered      = total_delivered_bytes (o an)
  pkt.delivered_time = now
  pkt.first_sent_time, pkt.is_app_limited

ACK gelince (paket p için):
  total_delivered_bytes += p.size
  delivered_now = total_delivered_bytes
  ack_elapsed   = now − p.delivered_time            # ACK-tarafı geçen süre
  send_elapsed  = p.send_time − p.first_sent_time    # gönderim-tarafı geçen süre
  interval      = max(ack_elapsed, send_elapsed)     # BBR: iki taraftan büyüğü (ACK-compression'a dayanıklı)
  delivered     = delivered_now − p.delivered
  if interval < MIN_RATE_INTERVAL: sample GEÇERSİZ (çok kısa)    MIN_RATE_INTERVAL=[STD:BBR ~1ms/1RTT]
  bw_sample = delivered / interval
```

### 7.2 BtlBw (bottleneck bandwidth, max-filter)
```
BtlBw = max(bw_sample) over window W_btlbw     W_btlbw=[STD:BBR 6-10 RTT]
  (max-filter: gerçek bottleneck'i yakalar; app-limited/geçersiz sample'lar BtlBw'i DÜŞÜRMEZ)
bw_est için raporlanan = BtlBw × estimator_confidence_gate  (düşük confidence'ta muhafazakâr, §7.3)
```

### 7.3 Sample geçerlilik (NORMATİF — estimator'ı ŞİŞİRME/çökertme koruması; TDD §5.1 validity)
```
bw_sample GEÇERSİZ (BtlBw'e KATILMAZ) eğer:
  app_limited            : tx_queue boştu (inflight < cwnd) → gönderim hızı bw'yi yansıtmaz
  ack_compression_score > ACK_COMP_THRESH : ACK'ler toplu geldi → interval yanıltıcı (send_elapsed devreye)
  reverse_path_impaired  : ack_confidence LOW → ACK zamanlaması güvenilmez
  ack_delay_untrusted    : rtt_sample negatif / ack_delay > srtt anomali

estimator_confidence (EWMA, 0..1):   [CAL]
  geçerli sample → confidence ↑ ;  geçersiz/anomali → confidence ↓
  düşük confidence → SRC muhafazakâr pace (BtlBw'e tam güvenme), fuzzy "bilinmiyor" moduna kayar

app_limited işareti:
  paket gönderilirken (tx_queue boş ∧ inflight < cwnd) → pkt.is_app_limited=true
  bu paketten türeyen bw_sample BtlBw max-filter'a GİRMEZ (yalnızca "≥ mevcut BtlBw" ise günceller)

ack_compression_score:
  W penceresinde: beklenen_ack_spacing (paket pacing'inden) vs gerçek_ack_spacing oranı;
  gerçek ≪ beklenen → compression yüksek. score = clamp(1 − actual_spacing/expected_spacing).   [CAL]
```

### 7.4 "ana yetenek" invariant'ı
```
INV-ENGINE-BW: bandwidth DAİMA delivered-rate'ten (BtlBw); loss'tan ASLA türetilmez.
INV-ENGINE-SEP: bandwidth (BtlBw) ⟂ congestion (queue_delay) ⟂ loss (loss_pre_fec) — üç bağımsız ölçüm.
INV-ENGINE-RATE: loss send_rate'i düşürmez (INV-RATE-1); yalnız queue_delay + inflight_cap pace eder.
```

---

## 8. Congestion / Rate Control Algorithm — SRC (NORMATİF)

BBR ölçüm çekirdeği + hostile-uygun gentle probe + delay-based backoff (loss DEĞİL).

### 8.1 State
```
struct SrcState {
  btlbw, rtprop, srtt, rttvar, queue_delay,
  delivered_rate, estimator_confidence,
  send_rate, pacing_gain, cwnd_gain,
  inflight_bytes, cwnd_bytes, tx_queue_delay,
  loss_pre_fec, residual_loss, ack_confidence,
  state: SenderState, cycle_idx, rtprop_stamp,
}
```

### 8.2 Temel denklemler
```
BDP        = BtlBw × RTprop
cwnd_bytes = max(cwnd_gain × BDP, MIN_CWND)          cwnd_gain=[STD:BBR 2]; MIN_CWND=[CAL] (≥4×MTU)
send_rate  = pacing_gain × BtlBw                      (§13 pacing)
inflight cap: inflight_bytes ≤ cwnd_bytes            (üstündeyse gönderme, kuyrukta beklet)
```

### 8.3 Probe döngüsü (gentle — BBR ProbeBW gain-cycle DEĞİL)
```
STEADY'de:
  pacing_gain = 1.0 varsayılan.
  BW-probe: her PROBE_BW_PERIOD'da kısa süre pacing_gain = 1.0+g_up (ör. 1.25) → BtlBw taze;
            ardından pacing_gain = 1.0−g_dn ile biriken kuyruğu boşalt.   g_up,g_dn=[CAL]
  (BBR'ın 8-fazlı pace_gain cycle'ı ALINMADI — hostile path'te agresif probe zararlı; tek up/down yeter.)

DRAIN_RTPROP (RTprop tazeleme):
  RTprop_stamp > RTPROP_WINDOW eskiyse: cwnd_gain'i kısa süre düşür (inflight drain) → min rtt_sample ölç → RTprop güncelle.
  RTPROP_WINDOW=[STD:BBR 10s]; drain süresi ≥ 1 RTT.
```

### 8.4 FEC/ARQ arbitrasyonu (FÜZYON — aynı state, §15 fuzzy)
```
Girdi:  loss_rate, jitter, bandwidth, queue_delay, ack_confidence   (§15 fuzzified)
Çıktı:  fec_parity_ratio, arq_aggressiveness, send_rate_gain_hint, compress
Kanonik kurallar (TDD §5.3 ile birebir):
  loss LOW                         → fec=0,    arq=light
  loss HIGH ∧ bw HIGH ∧ jitter LOW → fec=high, arq=moderate           (RS erasure, RTT-beklemeden recovery)
  loss MED  ∧ bw MED               → fec=med,  arq=med
  loss LOW  ∧ jitter HIGH          → fec=0,    arq=reorder-tolerant (RTO↑, fast-rtx↓)
  bw LOW ∧ queue_delay HIGH        → send_rate↓ (congestion; fec CAP)  [delay-driven, loss DEĞİL]
  loss HIGH ∧ bw LOW ∧ ackconf LOW → EMERGENCY_FORWARD_REDUNDANCY (§11)
```

### 8.5 Budget clamp (NORMATİF — §5.5 TDD)
```
available_redundancy_budget = f(BtlBw, mevcut tx ihtiyacı)   [CAL]
parity_final = min(fuzzy_parity, budget_cap, profile_cap)
  → düşük bantta yüksek loss bile parity'yi patlatamaz (dar bandı FEC israf etmez).
send_rate_final = clamp(fuzzy_send_rate, [MIN_RATE, min(BtlBw_based_cap, profile_cap)])   MIN_RATE=[CAL]
```

---

## 9. ARQ Selective Retransmit Algorithm (NORMATİF)

```
RECOVER_ARQ state'inde (residual_loss>0 ∧ ack_confidence≥MED):
  missing = rx tarafından SACK'te bildirilen gap seq'leri (§2)
  for seq in missing (öncelik: en eski deadline'a yakın):
     if arq_mode_active (fuzzy arq_aggressiveness > 0):
        retransmit(seq) — pacing token bucket'tan (§13); inflight cap'e tabi
     if fec_dominant (fuzzy fec yüksek ∧ ackconf düşük): retransmit BASKILA — residual FEC ile kapanacak
  spurious önleme: §9.3
```

### 9.1 Retransmit budget
```
retransmit'ler DATA ile aynı pacing bütçesini paylaşır (ayrı queue, deadline-öncelikli).
ARQ_RETX_CAP: bir seq için max retransmit sayısı; aşılırsa FEC/forward'a devret veya connection-fail.   [CAL]
```

### 9.2 RTO/PTO ile ilişki: §12.

### 9.3 Spurious retransmit detection
```
reorder_thresh = max(K, ρ × recent_max_reorder)     (§5)
retransmit'ten SONRA orijinal ACK gelirse → spurious işaretle → reorder_thresh ↑ (adaptif) → gelecekte bekle.
DSACK-benzeri: rx duplicate aldığını ACK'te bildirir (§9.4) → sender spurious teyidi.
```

### 9.4 Duplicate suppression
```
rx zaten-teslim seq alırsa: teslim ETME (RX_DUP_SUPPRESS); ACK'i teyit et (cumulative ilerlet).
Bu, spurious retransmit'i sender'a bildirir + gereksiz uygulama teslimi önler.
```

---

## 10. FEC Block Format & Parity Selection (NORMATİF — INV-FEC-1 uyumlu)

> **SoC (§0.2):** FEC **POLİTİKA** (ne kadar parity → k,r; §8.4 fuzzy, ölçülen loss'tan) TRANSPORT'un. FEC **EXECUTION** (RS-encode + per-shard AEAD, code-then-encrypt-authenticate-shards) GÜVENLİK katmanının (Noise+kompakt). Transport parity ORANINI verir; güvenlik katmanı encode+AEAD yapar; transport opak shard'ları pace/deliver/track eder + framing metadata'sını (block_id/shard_index/k/r) reliability için okur. Aşağıdaki kripto sırası GÜVENLİK katmanının davranışıdır — transport onu EXECUTE ETMEZ, referanstır.

**Kripto sırası (KİLİTLİ — TDD INV-FEC-1, §4.4.7; GÜVENLİK katmanı yürütür): CODE-THEN-ENCRYPT-AND-AUTHENTICATE-SHARDS.**
```
Gönderen (güv. katmanı): block_plaintext → RS-erasure encode (k data + r parity) → (k+r) PLAINTEXT shard →
          HER shard AYRI AEAD encrypt+authenticate → wire'da yalnız ciphertext shard.
Alıcı (güv. katmanı):    shard al → ÖNCE per-shard AEAD verify/decrypt → YALNIZCA authenticated plaintext shard
          FEC decoder'a girer → eksik data shard'ları parity'den reconstruct.
          unauthenticated/tampered shard decoder'a ASLA girmez (authenticate-before-decode).
Transport: yukarıdakini ÇAĞIRMAZ; yalnız opak shard'ları taşır + metadata okur.
```

### 10.1 FEC_SHARD wire (fixed-width)
```
FEC_SHARD {
  # --- framing metadata (TRANSPORT okur — reliability/reconstruction; güv. katmanı AAD olarak authenticate eder) ---
  frame_type   : u8        # 0x20
  block_id     : u32_be    # blok kimliği (hop-scoped, wrap→rekey)
  shard_index  : u8        # 0..k+r−1
  k            : u8        # data shard sayısı (bu blok)
  r            : u8        # parity shard sayısı
  shard_len    : u16_be    # bu shard'ın payload uzunluğu
  # --- güvenlik katmanı (TRANSPORT görmez/yönetmez — opak; §0.2, INV-FEC-1) ---
  ciphertext   : shard_len bytes    # per-shard AEAD ciphertext  [GÜVENLİK katmanı]
  aead_tag     : 16 bytes           # per-shard AEAD tag         [GÜVENLİK katmanı]
}
# Kripto yapı (ct+tag) ccr §35/TDD §0.1'e aittir; bu belge onu transport-owned REDEFINE ETMEZ.
# reed-solomon-erasure (ADR-16); ErasureCodec trait arkasında (SIMD Beta geçişi trait ile) — encode/decode güv. katmanında.
# IMPL NOTU (INV-ENGINE-SOC tip-kilidi): transport kodunda ct+tag AYRI alan DEĞİL, TEK OPAK BLOB
#   (opaque_secured = ct||tag) olarak taşınır. Ayrı `aead_tag` alanı transport struct'ında BULUNMAZ →
#   tag'e dokunmak DERLENEMEZ; SoC yorum değil, TİP garantisi olur.
```

> **⚠️ AUDIT-FINDING METADATA-LEAK-01 (Alpha'da KABUL, Beta/Beta+'ta MİNİMİZE — kayıtlı karar):**
> FEC_SHARD framing metadata (`frame_type · block_id · shard_index · k · r · shard_len`) wire'da **PLAINTEXT** — yol üzerindeki gözlemciye (pratikte hub'a) açıktır. Kısmen kaçınılmaz (hub reconstruction/forward için framing'i bilmek zorunda; TCP seq'in açık olması gibi) ve **Alpha varsayımıyla tutarlı** (sealed sender Alpha'da YOK; hub metadata görür — federated model kabulü; cover traffic Beta+).
> **Bilinen sızıntılar (Alpha'da bilinçli kabul):**
> - `k, r` → **parity oranı = kayıp/emergency koşulunu sızdırır.** Global passive observer (DPI/EW) "bu bağlantı ne zaman EMERGENCY_FORWARD_REDUNDANCY'ye girdi"i r/k'dan okuyabilir. **En hassas alan.**
> - `block_id` → aynı bloğun shard'ları **linkable** (bir mesajın parçaları gruplanır) → trafik analizi sinyali.
> - `frame_type` (0x20) → trafik **sınıfı** sızar; tek-opak trafik indistinguishability hedefiyle (audio codec PT=96 kararı) gerilim yaratır.
> - `shard_len` → uzunluk sızıntısı (E2E'de kısmen kaçınılmaz).
>
> **Beta/Beta+ minimizasyon yönü (BURADA ÇÖZÜLMEZ, unutulmasın diye kayıtlı):** (a) `k,r` her shard'da DEĞİL, **blok-başına bir kez** (hub ilk shard'da öğrenir + cache) → r/k sızıntı yüzeyi daralır; (b) `frame_type` ayrımını SESSION_DATA ile tek trafik sınıfında gizle (indistinguishability); (c) cover traffic + padding (Beta+). Alpha'da §10.1 framing-açık implemente edilir; benchmark geciktirilmez.

### 10.2 Parity seçimi (k, r) — fuzzy-driven
```
r = round(fec_parity_ratio × k)     (§8.4 fuzzy çıktısı; §8.5 budget-clamp'li)
k = block'taki data shard sayısı (payload / shard_size; shard_size=[CAL], MTU-uyumlu)
kısıt: k + r ≤ RS_MAX_SHARDS (GF(2^8) → 255).   loss profiline göre r/k oranı §8.4.
tek-datagram / tiny payload: RS uygulanamaz → duplication (§11) doğru araç.
```

---

## 11. Emergency Forward Redundancy Algorithm (NORMATİF — ACK-bağımsız; TDD §5.6)

```
Tetik (fuzzy): loss HIGH ∧ bandwidth LOW ∧ ack_confidence LOW
  Gerekçe: %80 çift-yönlü loss → P(data∧ACK)=0.2×0.2≈%4 → ARQ ~25× round-trip'e sürünür.
           Bu köşede kurtarma ACK dönüş yoluna BAĞIMSIZ olmalı.

Davranış (payload tipine göre):
  tek-datagram / tiny (block/shard yok):
     → forward duplication: aynı payload D× gönder (D=2..3), ACK beklemeden.   D=[CAL]
  segment-block / multi-shard:
     → minimal parity shards (low-rate forward FEC), r/k düşük ama ACK-bağımsız.
  compress = ON (dar bantta bytes azalt).
  send_rate = minimal-reliable (MIN_RATE clamp; §8.5) — path'i boğma.

Geri çekilme (NORMATİF):
  path düzelirse (ack_confidence ↑ / residual_loss ↓) fuzzy redundancy'yi AZALTIR:
     3× → 2× → ARQ'ya iniş (delivered-rate confidence yükseldikçe).
  → EMERGENCY kalıcı DEĞİL; fuzzy tarafından ölçümle geri alınır.

GEÇİŞ KARARLILIĞI — RETREAT HYSTERESIS (NORMATİF, KİLİTLİ — asimetrik):
  Sorun: fuzzy girdiler SÜREKLİ (trapezoidal), emergency LEVEL DISCRETE (Dup3/Dup2/ARQ). Hostile
  rejimde (%50-95 loss) ACK seyrek → ack_confidence örneklem varyansı YÜKSEK → tek-eşikli sistem
  eşik etrafında 3×↔2× SALINIR → kıt bantta duplication israfı + duplication→ölçüm→mode pozitif
  geri-besleme riski. Çözüm iki katman:
  (1) ASİMETRİK Schmitt-trigger eşikleri: escalate (redundancy ARTIR) health < X;
      de-escalate (AZALT) health > Y; Y > X. Bant [X,Y] gürültüyü soğurur (bant içinde geçiş YOK).
      health = f(ack_confidence↑, residual_loss↓) per-mille.
  (2) ASİMETRİK dwell (recovery-obsesif taban gereği — yönler eş maliyetli DEĞİL):
      escalate dwell   = 0 / çok kısa → kötüleşmede ANINDA koru (geç escalate = korumasız kayıp =
                          "veri ULAŞSIN" tezinin ihlali; Schmitt bandı escalate'i gürültüden zaten korur).
      de-escalate dwell = min_dwell [CAL] → iyileşmede TEMKİNLİ (osilasyonun asıl kaynağı erken
                          de-escalate→re-escalate döngüsü; dwell bunu zamanla ayırır, duplication'ın
                          ölçüm etkisi sindirilene dek mode kilitli).
  Marjlar (X, Y, min_dwell) = [CAL] fixture (netem baseline'da kalibre; mekanizma [STD:Schmitt/kontrol-teorisi]).
duplication/parity yine available_redundancy_budget ile clamp (§8.5).
```

---

## 12. Timers (NORMATİF)

```
ACK_DELAY_TIMER (receiver):
  max_ack_delay içinde ACK gönder; ama ACK_FREQ_N DATA alınınca hemen (hangisi önce).   [CAL]
  reverse-path impaired → ACK decimation: N ↑, budget ↓ (§11, §2.1).

PTO (Probe Timeout — retransmit/probe tetik; RFC 9002-türevi ama loss≠congestion):
  PTO = srtt + max(RTTVAR_MULT × rttvar, min_pto) + max_ack_delay    RTTVAR_MULT=[STD:4]; min_pto=[CAL]
  PTO firing → probe/retransmit; PTO backoff exponential (2^n) ardışık firing'de.
  NOT: PTO send_rate'i DÜŞÜRMEZ (loss-decoupling); yalnız recovery tetikler.

RTPROP_PROBE_TIMER: RTPROP_WINDOW (§8.3) dolunca DRAIN_RTPROP.
IDLE_TIMER: idle > IDLE_TIMEOUT → IDLE (session korunur; TDD blackout-survival).   IDLE_TIMEOUT=[CAL]
RETRY_BUDGET: bağlantı başına toplam retransmit/probe üst sınırı → aşılırsa fail-closed / hub-failover.   [CAL]
```

---

## 13. Pacing / Token Bucket (NORMATİF)

```
pace_rate = send_rate_final (§8.5)     # bytes/sec
Token bucket:
  refill: her Δt'de tokens += pace_rate × Δt
  cap:    tokens ≤ BURST_CAP           BURST_CAP=[CAL] (≈ 1-2 × MTU; micro-burst sınırı)
  gönder: paket ancak tokens ≥ pkt.size ise; değilse pacing kuyruğunda bekle (Tokio timer)
inter-packet gap = pkt.size / pace_rate    (düzgün pacing; ACK-clock değil, rate-clock — BBR pacing)
priority: retransmit(deadline) > FEC parity > yeni DATA > ACK-only (ACK-only ayrı küçük budget)
```

---

## 14. Inflight Cap / Memory Cap (NORMATİF)

```
inflight_bytes ≤ cwnd_bytes (§8.2) — congestion/hafıza koruması.
Buffer pool (bounded — SR-AUDIT-001):
  tx_unacked_pool:   ≤ TX_POOL_MAX bytes (retransmit için tutulan);  aşılırsa app back-pressure.   [CAL]
  rx_reorder_pool:   ≤ RX_POOL_MAX;   fazla out-of-order → en eski drop (SACK ile kurtarılır).      [CAL]
  fec_shard_pool:    blok başına ≤ (k+r) shard; blok tamamlanınca/timeout'ta serbest.
  MAX_SKIP (E2E ratchet skipped-key, TDD): ayrı sınır; bounded.
Tüm pool'lar sabit-üst-sınır (unbounded alloc YOK; PARSER-02 bounds-before-alloc).
```

---

## 15. Fuzzy Membership Functions (NORMATİF — DETERMINISTIK, ADR-17)

```
Girdiler (§5.1 TDD): loss_rate, jitter, bandwidth, queue_delay, ack_confidence
Her biri 3 membership: LOW / MED / HIGH — trapezoidal (deterministik, süreklilik → cliff/osilasyon yok).

Örnek (loss_rate; eşikler [CAL: netem baseline]):
  μ_LOW(x)  = trap(x; 0, 0, a1, a2)
  μ_MED(x)  = trap(x; a1, a2, b1, b2)
  μ_HIGH(x) = trap(x; b1, b2, ∞, ∞)
  (a1,a2,b1,b2 netem'de kalibre; UYDURULMAZ.)

Defuzzification: kurallar (§8.4) → ağırlıklı-ortalama (centroid) → fec_parity_ratio, arq_aggressiveness,
                 send_rate_gain, compress.

DETERMINIZM (ADR-17, ZORUNLU):
  aynı input snapshot (loss,jitter,bw,qdelay,ackconf) → aynı output. RNG YOK.
  golden test vector seti (§16) her build'de doğrulanır; sapma = build fail.
```

---

## 16. Deterministic Test Vectors (NORMATİF — conformance)

```
Amaç: engine'in her deterministik parçasını golden vektörle kilitle (implementer drift + regresyon).

VEKTÖR SETLERİ:
  TV-ACK:    (rx_bitmap) → beklenen ACK_FRAME byte'ları (fixed-width, §2). Wrap senaryoları dahil.
  TV-SEQ:    (expected, wire_seq) → çözülen full_counter (§2 wrap decode).
  TV-RTT:    (rtt_sample dizisi) → beklenen srtt/rttvar/RTprop/queue_delay (§6).
  TV-BW:     (send/ack olay dizisi + app_limited/compression bayrakları) → beklenen bw_sample geçerliliği +
             BtlBw (§7). "ana yetenek" kanıtı: loss enjekte edilir, BtlBw DEĞİŞMEZ (bandwidth loss'tan bağımsız).
  TV-LOSS:   (ack dizisi) → beklenen lost set (packet/time threshold, §5) + loss_pre_fec/residual.
  TV-FEC:    (block, k, r, silinen shard'lar) → reconstruct doğru; tampered shard → decoder RED (INV-FEC-1).
  TV-FUZZY:  (input snapshot) → beklenen (fec_ratio, arq_aggr, send_rate, compress) (ADR-17 determinizm).
  TV-EMERGENCY: (health trace) → beklenen level geçişleri (§11 hysteresis): Schmitt bant-içi gürültüde
             SIFIR flip; escalate ANINDA (dwell'siz); de-escalate geçişleri ≥ min_dwell aralıklı;
             framing-ayrımlı RecoveryAction (Tiny→Duplication D×+spacing, SegmentBlock→MinimalParity).
  TV-SRC:    (senaryo trace) → beklenen send_rate/cwnd trajektorisi (loss send_rate'i düşürMEDİĞİ dahil).

Format: ccr §30 tarzı gerçek byte / sayısal dosyalar; Rust referans ≡ değer.
Tunable'lar [CAL] → baseline ölçümünden sonra vektörlere GERÇEK değerle işlenir (placeholder değil).
```

---

## 17. QUIC Comparison Harness (NORMATİF — §4.4 TDD'ye bağlı)

```
Bu belge engine'i üretir; KABUL kapısı transport-layer-tdd.md §4.4'tedir (tekrar tanımlanmaz, REFERANS).
Harness girdileri (engine'in ölçülen çıktıları → §4.4 metrikleri):
  - deadline-bounded completion, blackout survival, operational floor, handshake/first-delivery (§4.4.3 birincil)
  - time-to-first-delivery, p50/p95 (deadline-altı), bytes/msg, ACK overhead, energy (§4.4.3 secondary-but-required)
  - estimator accuracy (§7 BtlBw vs netem ground-truth) + loss/congestion confusion matrix (§4.4.4 mekanizma-kanıtı)
  - FEC efficiency (§10), CPU/byte (§4.4.6), güvenlik-pariteliği (§4.4.7 code-then-encrypt-auth-shards)
Karşı taraf: best-available production-grade QUIC (en güçlü mevcut CC), §4.4.1 kurulum.
GO/NO-GO: §4.4.10 üç-tier (Tier-1/2/3) + guardrail'ler. Bu belge o kapıyı BESLER, yeniden tanımlamaz.
```

---

## Ek — Cross-Doc Tutarlılık (NORMATİF)

```
transport-layer-tdd.md   : WHAT/policy otoritesi (§0.1 kripto, §2 wire, §4.4 benchmark, §5 fuzzy, INV-*).
ccr-format-spec.md §35   : Session Model V3 canonical byte'lar (SESSION_INIT/DATA); ACK/FEC_SHARD hop-transport (E2E DEĞİL).
identity §11 / §0.1      : client↔hub Noise IK/KK GÜVENLİK katmanında; ACK/control/shard AEAD'i GÜVENLİK katmanı yürütür (transport tutmaz, §0.2); hot-path thin.
ADR-01/02/16/17/18       : Rust/Tokio; reed-solomon-erasure + ErasureCodec trait; deterministik fuzzy; netem gate.
Bu belge byte-layout'ta ccr §35'i EZMEZ; ACK/FEC_SHARD hop-transport frame'leridir (E2E envelope'un dışında, §0.2).
```

**Statü:** IMPLEMENTATION DRAFT — bölüm iskeletleri + formüller + wire layout + state machine tam; `[CAL]` sabitleri netem baseline'dan (ADR-18) kalibre edilecek, ardından golden vektörlere (§16) işlenecek. Bu belge tamamlanınca `Transport Protocol Specification v1.0`'a terfi eder.
