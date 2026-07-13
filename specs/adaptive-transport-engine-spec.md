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
  0x12 CTRL_PATH_PROBE          # [K4] AEAD-içi `subtype` ile versiyonlu: cold-start ölçüm varyantı (§11.1); mevcut semantik korunur
  0x13 CTRL_REKEY_SIGNAL
  0x14 ACK_FRAME_V2             # [K3/R2] cold-start ölçüm eki (§2.2); V1 0x10 korunur
  0x20 FEC_SHARD_HEADER
(SESSION_* class'ları hub_handle demux + E2E envelope tipinden ayrışır; transport-control frame_type güvenlik katmanı outer-AEAD-çözümünden SONRA görünür — çözümü GÜVENLİK katmanı yapar, §0.2.)
# [K-final] Cold-start probe/echo (§11.1) dış çerçevesi = hub_handle(4B)+ciphertext+tag → CTRL sınıfı-İÇİ ÖZDEŞ
# (frame_type/subtype AEAD-içi → tür-ele-verme YOK). SESSION_DATA'dan seq16(2B)+inner-E2E ile ayrışır →
# sınıf-ele-verme (TÜR değil); §10.1 "frame_type sızar" gerilimiyle tutarlı, tam indistinguishability Beta+.
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
                                # [TUR-1x] V1 EMIT EDİLMEZ (yalnız decode-uyumluluk): µs u16 tavanı
                                # 65.5ms sub-kbps ACK-hold'unda anında doyar → §6 düzeltmesi çalışamaz
                                # (§16.2 satırı). EMIT = V2 (§2.2, 256µs).
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
- ack_delay = now − largest_seq_recv_time. [TUR-1x] V2 encoding (NORMATİF, D-4/1 Seçenek-A):
  `encoded = min(floor(delay_us/256), 65535)` (FLOOR — receiver kendi delay'ini ŞİŞİRMEZ, decoded ≤
  gerçek → sender path-RTT'si fazla küçülmez); `encoded==65535` → satürasyon audit-sayacı
  (`ack_delay_saturated`, beklenen 0: tavan 16.78s > ACK_CADENCE_FLOOR 8.39s).
- Reverse-path impaired (ack_confidence LOW) → ACK decimation (§11): ACK_FREQ artar, ACK-only budget düşer.
```

### 2.2 Extended SACK — ACK_FRAME_V2 (NORMATİF — cold-start ölçüm eki) [K3/R2/revize-D0 §4.1]

> V1 (0x10) KORUNUR. V2 geriye-uyumlu — yeni frame_type **0x14** (0x11 DEĞİL: CTRL_MTU_PROBE ile
> çakışırdı, R2). V2 ölçüm eki (`seen_count`+`arrival_rate`) cold-start'ta yön-ayrımlı loss + ilk bw ipucu verir.

```
ACK_FRAME_V2 {
  frame_type    : u8       # 0x14   (§1.1 tabloda 0x14; 0x14-0x1F boştu, çakışma YOK)
  ack_epoch     : u8
  largest_seq   : u16_be
  ack_delay     : u16_be   # [TUR-1x, D-4/1 Seçenek-A] birim 256µs (tavan 65535×256µs ≈ 16.78s >
                           #   cadence-floor 8.39s). Floor-encoding + satürasyon-sayacı §2.1'de.
                           #   µs-birim YALNIZ V1'dedir (V1 emit edilmez, decode-uyumluluk).
  block0_len    : u16_be
  range_count   : u8       # ≤ MAX_ACK_RANGES (16)
  ranges[range_count] { gap:u16_be; len:u16_be }
  # ── V2 ölçüm eki ──
  seen_count    : u32_be   # görülen toplam DATA (birim+kopya) → sender kendi gönderdiğiyle kıyaslar:
                           #   seen≪sent = forward-loss; seen≈sent ama SACK gelmiyor = return-loss (p5 ayrımı)
  arrival_rate  : u16_be   # receiver-gözlemli pencere-oranı B/s (ilk bw örneği). Sentinel 0xFFFF =
                           #   "ölçüm YOK ya da üst-bantta — cold-start ipucu olarak KULLANMA".
                           #   [TUR-1x zeyli] Mevcut artımda receiver datagram BOYUTU görmez → daima
                           #   sentinel emit edilir; gerçek pencere-oranı K3 tüketicisiyle bağlanır.
}
# 15B sabit + 4B×range_count. Noise üstü, authenticated (§0.2 outer-AEAD güvenlik katmanı).
# arrival_rate pencere-oranı (packet-pair DEĞİL — cold-start'ta gürültüye dayanıklı yeterli).
```

---

## 3. Sender State Machine (NORMATİF)

```
Durumlar:
  IDLE            → gönderilecek yok; timer'lar pasif
  PRE_CONVERGE    → cold-start: motor HİÇ ölçüm yokken; en-kötü-köşe önsel pacing + geometrik kopya (§3.3-B)   [K-KEYSTONE]
  STARTUP         → BtlBw hızlı keşif (pacing_gain yüksek), path doldur
  STEADY          → pace = BtlBw; periyodik BW/RTprop probe
  RECOVER_ARQ     → residual loss var + ack_confidence OK → selective retransmit (§9)
  RECOVER_FWD     → ack_confidence LOW → forward redundancy (§11), ACK-bağımsız
  DRAIN_RTPROP    → RTprop probe: inflight düşür, min-RTT ölç (§8.3)

Geçişler (event-driven, deterministik):
  IDLE   --data-hazır-->                 PRE_CONVERGE                                          [K-KEYSTONE §3.3]
  PRE_CONVERGE --ilk geçerli SACK/echo (RTT+loss kilidi §3.3-D)--> STARTUP                     [R1/R14]
  STARTUP--BtlBw plato (Δ<ε, N round)--> STEADY
  ACTIVE --ack_confidence LOW----------->  RECOVER_FWD  --ackconf≥MED--> STEADY   [AMENDMENT-1]
  ACTIVE --residual_loss>0 ∧ ackconf≥MED-> RECOVER_ARQ  --temiz-------> STEADY   [AMENDMENT-1]
  STEADY --rtprop_probe_due------------->  DRAIN_RTPROP --min-rtt ölçüldü--> STEADY
  (any)  --idle > IDLE_TIMEOUT---------->  IDLE
  ── ACTIVE = {STARTUP,STEADY,RECOVER_ARQ,RECOVER_FWD,DRAIN_RTPROP} (IDLE hariç tüm aktif fazlar).
     PRE_CONVERGE ACTIVE'e DAHİL DEĞİL: recovery (FWD/ARQ) + emergency + loss-ilanı §3.3-C'de DONDURULUR.
     Değerlendirme önceliği (her event): IDLE(iş yok) > PRE_CONVERGE(ölçüm-yok §3.3) > RECOVER_FWD(ackconf
     LOW) > RECOVER_ARQ(pending loss) > faz-içi(STARTUP plato / STEADY / DRAIN_RTPROP).
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
    AppData(bytes)   -> enqueue(tx_queue); if state==IDLE: state=PRE_CONVERGE   # §3.3 [K-KEYSTONE]; PRE_CONVERGE→STARTUP ilk geçerli SACK/echo'da (§3.3-D)
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
arm_pto_timer()                    # §12 — PRE_CONVERGE'de ATLANIR (§3.3-C/5: srtt BOŞ → PTO tanımsız; geometrik kopya §3.3-B yerini alır)
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

### 3.3 PRE_CONVERGE — Cold-Start (NORMATİF)

> **[K-KEYSTONE / revize-D0 §2.1-2.4]** Motor HİÇ ölçüm yokken (cold-start) bu durumdadır.
> İlke: **KAPI YOK (ama FREN VAR), TERMOMETRE VAR** — veri t=0'da çıkar; ölçüm gönderilenin yanında
> toplanır. Eski "sabit INITIAL_RTT-seed + sabit-timeout" cold-start modeli CANCELLED (Ek-B).

**Giriş/çıkış:**
```
IDLE --AppData--> PRE_CONVERGE            # t=0, ölçüm yok; ilk birim beklemeden gider
PRE_CONVERGE --ilk geçerli SACK/echo--> STARTUP   # RTT+loss kilidi açılır (§3.3-D; echo de açar — §11.1(iv))
```

**(A) Davranış önseli — fuzzy en-kötü-köşe (NORMATİF cümle):** [K5/§2.1]
> *"PRE_CONVERGE'de motor fuzzy en-kötü-köşe önseliyle DAVRANIR; bu önsel hiçbir estimator'a örnek
> olarak yazılmaz, ilk geçerli ölçümle koşulsuz tahliye edilir ve prob takvimi önselin geçerliliğinden
> bağımsız sürer."*
- `bw = PRE_CONVERGE_FLOOR_BW` (§16 [CAL]; operatör `bandwidth_class` yalnız ÖNSELİ ezer, ölçümü ASLA),
  `loss = extrem`. Pacing bu tabana kurulur → zarf-içi (≥ taban) hiçbir kanalda self-congestion inşa
  gereği imkânsız. Estimator'lar (BtlBw/srtt/loss) cold-start'ta BOŞ.
- **[R8] Pacing floor:** PRE_CONVERGE'de send_rate clamp floor'u = `PRE_CONVERGE_FLOOR_BW`; `min_rate_bps`
  fixture (=10000 B/s=80 kbps) **MUST-NOT-APPLY** (Ek-B). Yoksa 1 kbps kanal 64× floodlanır (self-congestion).
  [K-D2] **BURST clamp da ZORUNLU** (yalnız rate DEĞİL): token-bucket DEFAULT_BURST_CAP (2×MTU) t0'da tüm
  kuyruğu tek hamlede wire'a çıkarır → floor'u baypas eder (self-congestion + §3.3-B kopyalar temiz kanalda
  spurious ateşler; burst tüm wire-emit'leri t0'a çökertir → [K-D2] copy-clock spread'i etkisiz kalır).
  PRE_CONVERGE'de burst tek-birime clamp'lenir → ilk birim t=0'da gider, kalanı floor-refill yayar.
- **[R1/R2/R3-ADIM6] Pacer kbit-domain genellemesi (NORMATİF — AŞAMA-1 denetim bulgusu):** Pacer aritmetiği
  80 kbps altına inemiyordu (dar-pencere ADIM-6 D2'nin 10-msg burst'ü maskeledi; 100-msg sub-kbps açığa
  çıkardı). ÜÇ kök-fix:
  - **[R1] Token refill SUB-BYTE precision:** `rate·dt_ns/1e9` per-tick tamsayı-truncation'ı rate<1000 B/s'te
    (31 B/s @ 1ms = 0.031→0) her tick 0 üretip pacer'ı KİLİTLİYORDU (PreConverge no-ack STALL kök nedeni: 1
    birim gönderip durur). Fix: kesir-kalan sonraki refill'e taşınır → floor gerçekten token üretir.
  - **[R2] min_rate kanal-türevli:** MUTLAK 80 kbps min_rate SADECE PreConverge'de kapatılmıştı; Startup'a
    çıkınca <80 kbps kanalı 80× overdrive ediyordu (self-congestion çöküşü). min_rate artık TÜM state'lerde
    kanal-hayatta-kalma floor'u (=`PRE_CONVERGE_FLOOR_BW`); efektif rate BtlBw'yi izler.
    **CANCELLED:** `min_rate_bps=10_000` mutlak sabiti (MUST-NOT: <80 kbps kanalda floor olarak uygulama).
  - **[R3] restore-burst kanal-türevli:** PreConverge çıkış burst'ü + micro-burst cap artık `clamp(rate·
    BURST_WINDOW, 1 birim, ~2×MTU)` — bant-kör 2×MTU=2400B sabiti yerine. sub-kbps'te 1-birim (backlog-spike
    yok), hızlı kanalda micro-burst. **CANCELLED:** `restore_burst`'ün sabit 2400B dump'ı (MUST-NOT: convergence
    sonrası bant-kör 2×MTU grant).
  Zarf: [R8]'in "self-congestion imkânsız" iddiası artık 4-decade'de (0.25k/1k/100k/1M) gerçekten tutar —
  önceden yalnız ≥80 kbps + küçük iş-yükünde tutuyordu. (Kalibrasyon: §16.1 pacer satırları.)

**(B) Veri — programlı geometrik kopya takvimi (forward-redundancy sürücüsü):** [R1/§2.2]
```
İlk birim t=0'da gider; ACK beklenmez. Kopyalar geometrik: t=0, k_base·RTT_floor, ×COPY_MULTIPLIER, ...
  (taban/çarpan serialization fiziğinden — §16 [CAL]; UYDURULMAZ). Tetik: "henüz hiçbir ölçüm yok".
Stop: birimin kopyaları o birim Noise-authenticated SACK ile teyit edilince DURUR (RX_DUP_SUPPRESS §4).
  Akışın İLK SACK'i cadence-exempt (§12 ACK_CADENCE'i atlar, HEMEN emit) → ilk SACK ~RTT_floor'da →
  k_base·RTT_floor > RTT_floor temiz kanalda ilk kopyayı ateşlemeden iptal → tx_wire/altsınır ≈ 1×.
Sayı-tavanı YOK: aralık COPY_MAX_INTERVAL'e kadar büyür, deadline'a dek düşük tempoda sürer (§12 no_ack(T)).
[K-D2] Kopya-takvimi t₀'ı = birimin PACER'DAN WIRE'A ÇIKTIĞI an (ScheduleTransmit'te pacer.try_send başarısı),
  sunum/enqueue anı DEĞİL → copy_due = wire_emit + k_base·RTT_floor. Gerekçe: RTT_floor "birim yalnız yolda"
  varsayar; burst kuyruğu bunu bozar → saat wire-emit'e bağlanınca serialization/kuyruk gecikmesi otomatik
  hesaba girer (§2b pacer-tek-kapı ilkesinin kopya takvimine simetrik uygulanması). Kopyalar da AYNI pacer'dan
  geçer (§13); pacer'ı baypas eden gönderim yolu YOK (DATA/retransmit/kopya tek token bucket'ı paylaşır).
```

**(C) PRE_CONVERGE yasakları (NORMATİF — mekanizma seviyesinde):**
```
(1) loss ilanı YOK — MEKANİZMA-AGNOSTİK: §5'in TÜM loss-ilan tetikleri DEĞERLENDİRİLMEZ:
    §5(a) PACKET_THRESHOLD (≥K=3 yüksek-seq ACK) + §5(b) TIME_THRESHOLD (9/8×srtt). (PTO ayrıca (5).)
    "loss İLANI yok" tetik-ayrımı yapmaz → ilk geçerli SACK (kilit) öncesi hiçbir birim "kayıp" sayılmaz.
(2) emergency duplication YOK (§11 escalate DONDURULUR; TDD §10 "kör escalate ETME").
(3) sessizlikten estimator besleme YOK (sessizlik DAVRANIŞ seçer — geometrik tempo — VERİ üretmez).
(4) [R14] ack_confidence (§5.1) DEĞERLENDİRİLMEZ — FULL (1000) sabitlenir; TÜM tüketicileri
    (§3 ACTIVE→RECOVER_FWD, §8.4/§15 fuzzy, §11 emergency, §12 decimation) ilk gerçek dönüş örneğine
    kadar DONDURULUR. Gerekçe: threshold=ACK_SILENCE_MULT×srtt ≈300ms ≪ RTT_floor → çöküş değil
    taban-bw serialization gecikmesi → yanlış-pozitif.
(5) PTO ARM EDİLMEZ (§3.1 arm_pto_timer ATLANIR) — srtt BOŞ (§3.3-A) → §12 PTO formülü tanımsız
    (Ek-B srtt-seed CANCELLED). Retransmit/probe TEK sürücüsü §3.3-B geometrik kopya (RTT_floor-tabanlı,
    srtt-bağımsız); yoksa PTO ≈ min_pto RTT_floor'dan önce ateşler → geometrik kopyayla çift retransmit
    (Ek-B spurious-storm'unu geri getirir). STARTUP'ta (ilk SACK sonrası) PTO normal armanır.
```

**(D) Kademeli tahliye — sinyal başına AYRI kilit:**
```
İlk geçerli Noise-authenticated SACK/echo → RTT + loss kilidi açılır (gerçek örneklerle) → STARTUP.
bw kilidi AYRI: ölçülmüş bw örnekleri BİRİKENE kadar taban varsayımı sürer (tek örnek "bant VAR" der,
  "ne kadar" DEMEZ) → FEC/redundancy agresifliği bw-kanıtıyla kademeli açılır (min_rate dersi; §8.5
  budget clamp bw-kilidine bağlı).
```

**(E) Return-path ACK cadence — cold-start (firm co-requisite):** [K2/§3.C]
```
Receiver de cold-start yapar: dönüş bw'sini bilmez → taban-önselle başlar
  ACK_CADENCE ≥ sack_wire / (RETURN_BUDGET_FRACTION × return_bw_önseli)   # §12
DATA arrival-rate'iyle hızlanır. İLK SACK cadence-exempt (yukarıda B). FULL adaptif decimation
(reverse-path-impaired N↑ eskalasyonu) ADIM-6 ana gövdesi — burada yalnız minimal taban.
```

**(F) Ölçüm birimi (probe/echo) — hızlandırıcı, opsiyonel:** [K-KEYSTONE/§2.3b]
Probe/echo = authenticated hop-control frame (§11.1, `CTRL_PATH_PROBE` genişletmesi). **SoC-guard:**
probe güvenlik katmanının birimidir; transport yalnız içeriği (probe_seq/ölçüm) üretir, güvenlik
dikişinden opak geçer — AEAD'i güvenlik katmanı uygular (INV-ENGINE-SOC; QUIC-tipi bütünleşmeye
YAKLAŞTIRMAZ, keystone yalnız zarfı değiştirdi, katman sınırını değil). *"Probe hızlandırıcıdır; hiçbir
şeyi bloklamaz, receiver'da YENİ state açmaz, yokluğu VEYA geçersizliği otoriter sonuç doğurmaz."*

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
  (b) TIME_THRESHOLD:   send_time + max(TIME_THRESH × srtt, min_delay, K_c × EAI) < now
                        ∧ LATER-ACK KANITI (seq < largest_acked) VAR.                    [TUR-1a/1b, D-1]
                        TIME_THRESH=[STD:9/8]; min_delay=[CAL] (reordering tail).
      [TUR-1a / D-1/1] KANIT-ŞARTI (RFC 9002 §6.1'in atlanmış yarısı): higher-ACK kanıtı olmadan
      time-threshold KESİN loss YAZMAZ. Saf sessizlik loss metriğini ARTIRMAZ; fakat liveness
      amacıyla probe/retransmit tetikleyebilir (§5.1 ackconf → RECOVER_FWD spekülatif kopya —
      Duplicate sayılır, loss sayılmaz). Kanıtsız-eşik K-1 kompozisyonunun köküydü (ölçüldü: 27/27).
      [TUR-1b / D-1/2] CADENCE-FARKINDA TABAN (ZM-4): K_c × EAI;
      EAI = ACK_FREQ_N × ort_inflight_birim / send_rate_fiili + sack_wire/return_bw_önseli.
      Sender-yerel + deterministik; K_c=1.5 [CAL matris {1.25, 1.5, 2.0}]; return_bw_önseli
      §3.3-E-FULL'de ölçülen return_bw ile değişir [CAL]. "SACK'i henüz gelmedi" ≠ "kayıp".
[TUR-2a / D-1/3, D-1/5] W_loss OLAY-LEDGER'ı (NORMATİF): kayıp bir OLAY değil HİPOTEZdir.
  Her ilan seq-başına kayıt: Suspected → Confirmed (un-loss ufku kanıtsız geçti — ufuk =
  cadence-taban, geç-SACK'in makul varış penceresi) | Spurious (geç-ACK geldi → GERİ-ALINDI:
  pending-retransmit İPTAL + ReorderTracker.note_spurious beslenir + packet-threshold
  k = max(K, ρ×recent_max_reorder) TÜKETİLİR (§9.3 kablolandı) + retx_count==0 ise orijinal
  RTT örneği GEÇERLİ [kol-1]); pencere-dışına yaşlanan olay düşer (Expired → doğal decay).
  Ledger + pencere-paydası bounded: LEDGER_MAX=4096, en-eski-önce eviction + audit (SR-AUDIT-001).
[TUR-2b / D-1/4] ACTIVE-FLIGHT / TOMBSTONE AYRIMI (NORMATİF): kayıp-ilanı "inflight'tan silinir"
  DEĞİLDİR — İKİ ayrı kavram: (a) *active-flight muhasebesi*: şüpheli paket bytes_in_flight/cwnd'den
  DÜŞER (gönderim durmaz); (b) *retained transmission record (TOMBSTONE)*: gönderim kaydı
  {unit, send_ns, bw_sample, retx_count, origin} SAKLANIR — geç-ACK eşleştirme için.
  TTL = K_tomb×T_loss(ilan-anı) [zeyl: onaylı `min(mesaj-deadline,…)` kolu motorda uygulanamaz —
  per-unit deadline engine'e taşınmıyor]; tavan TOMBSTONE_MAX, en-eski-önce eviction +
  `tombstone_evicted` audit. Geç-ACK çarparsa: kol-1 (retx_count==0) → BW-örneği NORMAL + Data'ysa
  RTT (sansür biter — K-1 sansür-kolu kapanışı); kol-2 (retx'li, Karn-ambiguity) → örnek YOK +
  `ambiguous_skipped` sayacı (tam kurallar TUR-2c). Süresi/tavanı dolan kaydın geç-ACK'i →
  Expired/Unknown (örneksiz; teslim-damgası app/bench-katmanında bağımsız).
loss_pre_fec (fuzzy girdisi) = loss_windowed_ppm = Confirmed_units / pencere-içi-gönderim;
  W_loss = K_wl × ack_epoch; ack_epoch = max(corrected_srtt, EAI).  [TUR-2a — ömür-boyu
  monoton ratchet CANCELLED (K-1 fuzzy-zehirlenmesinin köküydü); ömür-boyu sayaç yalnız diag]
residual_loss = unrecovered_units / sent_units                  (FEC/ARQ SONRASI)
→ loss_rate fuzzy girdisi = fuzzify(loss_windowed_ppm)  (LOW/MED/HIGH, §15)
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
  [R14] PRE_CONVERGE (§3.3) → ack_confidence = 1000 (FULL) SABİT; aşağıdaki hesap DEĞERLENDİRİLMEZ.
        Gerekçe: threshold=ACK_SILENCE_MULT×srtt ≈300ms ≪ RTT_floor (1.4s@1kbps) → cold-start sessizliği
        reverse-path çöküşü DEĞİL taban-bw serialization gecikmesi → yanlış-pozitif emergency'yi keser.
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
       [TUR-1x] min-rtt guard (RFC 9002 §5.3 semantiği, NORMATİF): düzeltme yalnız
       `raw − ack_delay ≥ gözlenen_min_rtt (rtprop)` ise uygulanır; underflow ya da min-rtt-altı
       → RAW sample işlenir, örnek ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı YASAK).
       Bozuk/kötü ack_delay path-RTT'yi gözlenen-min altına çökertEMEZ. Saat-sınıfı: gözlenen-min
       (kanal-türevi) — mutlak wall-clock taban (1ms) 4-decade'de GEÇERSİZ çıktı (loopback gerçek
       RTT sub-ms; bench loopback ÖLÇÜMÜYLE yanlışlandı) → CANCELLED, ZM-2 örneği.

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
  → [R8 işaret levhası] PRE_CONVERGE'de MIN_RATE floor UYGULANMAZ (MUST-NOT-APPLY) → cold-start pacing
    floor = PRE_CONVERGE_FLOOR_BW (§3.3-A / §16.1 / Ek-B). (Kural burada da işaretli — uzakta yazılıp
    sessizce çiğnenmesin; R8 dersi.)
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

## 11.1 Cold-Start Probe/Echo — CTRL_PATH_PROBE genişletmesi (NORMATİF) [K4/K-KEYSTONE/R3/revize-D0 §4.2]

> Hızlandırıcı ölçüm birimi. **Noise İÇİNDE** (authenticated hop-control frame — INV-CRYPTO-1 / §1
> taksonomisi; below-Noise DEĞİL, Ek-B). `CTRL_PATH_PROBE (0x12)` versiyonlu genişletmesi — mevcut 0x12
> semantiği KORUNUR (spec'te 0x12 gövdesi hiç tanımlı değildi; ilk tanım burada, `subtype` ayrımıyla).
> **INV-ENGINE-SOC:** AEAD'i güvenlik katmanı uygular; transport opak metadata (probe_seq/seen_count/
> arrival_gap) okur, anahtar TUTMAZ. Yalnız KURULMUŞ akış içinde üretilir (Noise transport-anahtarı zaten
> var → +16B tag, yeni handshake YOK, INIT-öncesi pencere sorunu YOK).

```
CTRL_PATH_PROBE_v2 (probe) {           # sender→receiver, kurulmuş akış içinde
  [L2] hub_handle : u32_be             # demux (cleartext L2, SESSION_DATA ile aynı)
  [outer-AEAD zarfı içinde:]
    frame_type    : u8                 # 0x12 (§1.1: outer-AEAD sonrası İLK byte = frame_type; ACK/ACK_V2 ile aynı konvansiyon)
    subtype       : u8                 # cold-start probe alt-tipi (0x12-içi versiyonlama)
    probe_seq     : u16_be             # echo eşleştirme
    pad           : u8[N]              # amp≤1: probe_wire ≥ echo_wire olacak N (zarf-İÇİ)
  [AEAD tag : 16B]
}
CTRL_PATH_PROBE_v2 (echo) {            # receiver→sender, stateless (akışın MEVCUT demux/SACK state'i)
  [L2] hub_handle : u32_be
  [outer-AEAD zarfı içinde:]
    frame_type    : u8                 # 0x12
    subtype       : u8                 # echo alt-tipi
    probe_seq     : u16_be             # yansıtılan
    seen_count    : u32_be             # şu ana kadar görülen DATA
    arrival_gap   : u32_be             # [R3] son iki DATA/probe arası µs — u32 (u16=65ms kbps'te taşardı)
  [AEAD tag : 16B]
}
# [R4-fix] echo AEAD-gövdesi=12B (frame_type1+subtype1+probe_seq2+seen_count4+arrival_gap4) (+handle 4 +tag 16 = 32B wire).
# pad, echo'yu (32B wire) domine → amplification ≤ 1. nonce/token YOK: AEAD anti-forge/replay + proof-of-knowledge'ı kapsar.
```

**Davranış (NORMATİF):**
```
(i)   silent-drop — geçersiz/AEAD-çözülemeyen probe'a TEK BYTE cevap yok (aktif-tarama savunması AEAD-doğrulaması).
(ii)  amplification ≤ 1 — pad zarf-içi, echo wire-boyutunu domine eder (reflection kazanç sağlamaz).
(iii) responder rate-limit [CAL] — garbage'a AEAD-decrypt ucuz ama bedava değil (DoS taban).
(iv)  otorite: echo AUTHENTICATED → RTT/loss kilidini AÇABİLİR (§3.3-D); bw kilidi yine AYRI (birikim gerek).
      echo/arrival_gap tek örnek "bant VAR" der, "ne kadar" DEMEZ.
Statü: "Probe hızlandırıcıdır; hiçbir şeyi bloklamaz, receiver'da YENİ state açmaz, yokluğu VEYA
       geçersizliği otoriter sonuç doğurmaz. Yokluğunda sistem yalnız daha yavaş öğrenir."
```

---

## 12. Timers (NORMATİF)

```
ACK_DELAY_TIMER (receiver):
  max_ack_delay içinde ACK gönder; ama ACK_FREQ_N DATA alınınca hemen (hangisi önce).   [CAL]
  reverse-path impaired → ACK decimation: N ↑, budget ↓ (§11, §2.1).
  [K2/§3.3-E] BANDWIDTH-AWARE cadence (cold-start + genel): ACK_CADENCE ≥ sack_wire/(RETURN_BUDGET_FRACTION
    × return_bw) — dönüş kıt iken SACK seyrekleşir (kbps'te 20ms sabit → SACK-seli/self-congestion; §16).
  [R1] Akışın İLK SACK'i cadence-exempt: ilk DATA gelişinde ACK_CADENCE'i ATLAR, HEMEN emit edilir
    (yalnız sonraki SACK'ler rate-limitli) → PRE_CONVERGE geometrik ilk-kopya iptali (§3.3-B).

PTO (Probe Timeout — retransmit/probe tetik; RFC 9002-türevi ama loss≠congestion):
  PTO = srtt + max(RTTVAR_MULT × rttvar, min_pto) + max_ack_delay    RTTVAR_MULT=[STD:4]; min_pto=[CAL]
  PTO firing → probe/retransmit; PTO backoff exponential (2^n) ardışık firing'de.
  NOT: PTO send_rate'i DÜŞÜRMEZ (loss-decoupling); yalnız recovery tetikler.
  NOT: PRE_CONVERGE'de hem time-threshold loss-ilanı (§5) hem PTO ARM DONDURULUR (§3.3-C/1,5); srtt BOŞ
    olduğundan PTO tanımsız — retransmit/probe yerine §3.3-B geometrik kopya (RTT_floor-tabanlı) geçer.

RTPROP_PROBE_TIMER: RTPROP_WINDOW (§8.3) dolunca DRAIN_RTPROP.
IDLE_TIMER: idle > IDLE_TIMEOUT → IDLE (session korunur; TDD blackout-survival).   IDLE_TIMEOUT=[CAL]
  [R4] ROLÜ DEĞİŞMEZ: IDLE_TIMER yalnız IDLE'a geçirir (session korunur) — failover TETİKLEMEZ.
RETRY_BUDGET: [K6/R4] SABİT-SAYI DEĞİL, DEADLINE-TÜREVİ (deadline/COPY_MAX_INTERVAL) — sayı-tavanı YOK,
  zaman-tavanı var; geometrik kopya deadline'la sınırlı (bounded store-and-forward).   [CAL]
PATH_DEAD_TIMER: [R4] no_ack(T) → PATH_DEAD; T = IDLE_TIMEOUT'tan AYRI [CAL]. silence=now−max(last_ack,
  first_send) (data-pending iken monoton artar → erişilebilirlik ispatlı; §5.1 active). İki sınır:
  geometrik-takvim-span < T < deadline − failover_marjı (alt: kopyalar şans bulmadan ateşlenmesin; üst:
  failover'ın davranma süresi kalsın). PATH_DEAD = YOL-vazgeçişi (failover'a eskalasyon), TESLİM-vazgeçişi
  DEĞİL — mesaj kuyruktan yalnız deadline'la düşer. Tek-yol (alternatif hub yok) → geometrik kuyruk düşük
  tempoda sürer, TAM SESSİZLİK YASAK (blackout-survival). Failover sorumluluğu RETRY_BUDGET'tan buraya taşındı.
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
  TV-COLDSTART: [K5/K6/R1/R8/R14/revize-D0 §7] PRE_CONVERGE (§3.3) conformance:
    -1 (D2): temiz 1 kbps → 10/10 teslim; tx_wire ≤ 1.15·altsınır (altsınır=N·(50+payload)); worst-phase:
             her birim boyutunda (64B@floor dahil) ilk SACK k_base·RTT_floor'dan ÖNCE → spurious ilk-kopya YOK;
             [R14] ilk gerçek SACK öncesi state≠RECOVER_FWD ∧ emergency=OFF ∧ decimation=OFF;
             [R8] hayatta kalan min_rate=80kbps floor'u bu çift kapı (tx_wire≤1.15× + 10/10) mekanik yakalar.
    -2 (D3): %95 loss'ta tek SACK'ten RTT + yön-ayrımlı loss (seen_count); bw kilidi AÇILMAZ (§3.3-D).
    -3 (D4): hızlı kanal (100kbit+%90 loss) ilk saniyelerde yakalanır (probe hızlandırma).
    -4 (D4): 0.25 kbps'te probe bütçesi Z·forward_bw'yi AŞMAZ; amp≤1; geçersiz-probe'a sessizlik.
    -5 (D5): echo'suz koşuda motor asla tam sessizliğe düşmez; [K6] "tek-yol + echo'suz" (failover yok) varyantı.

Format: ccr §30 tarzı gerçek byte / sayısal dosyalar; Rust referans ≡ değer.
Tunable'lar [CAL] → baseline ölçümünden sonra vektörlere GERÇEK değerle işlenir (placeholder değil).
```

### 16.1 PRE_CONVERGE [CAL] sabitleri — türetme + kalibrasyon-domaini (NORMATİF) [K5/revize-D0 §5]

> Her sabit [CAL] (formül+domain) ya da [STD] (kaynak). Sayılar PLACEHOLDER — netem lowband baseline'ından
> kalibre; "tüm bw" satırlarının 4-decade geçerliliği ayrıca doğrulanır (0.25k/1k/100k/1M). Fizik: TDD §13
> SESSION_DATA=50B+payload. 1 kbps=125 B/s; 0.25 kbps=31.25 B/s.

| sabit | formül (türetme) | kalibrasyon-domaini | PLACEHOLDER |
|---|---|---|---|
| `PRE_CONVERGE_FLOOR_BW` | ürün floor; operatör `bandwidth_class` yalnız önseli ezer | önsel, tüm bw | 31.25 B/s |
| cold-start pacing floor [R8] | = PRE_CONVERGE_FLOOR_BW; `min_rate_bps`=10000 PRE_CONVERGE'de MUST-NOT-APPLY (Ek-B) | PRE_CONVERGE | 31.25 B/s |
| `min_rate_bps` [R2-ADIM6] | kanal-hayatta-kalma floor = `PRE_CONVERGE_FLOOR_BW`, TÜM state; rate→0 önle ∧ overdrive-yok. **CANCELLED:** 10000 (80 kbps) mutlak | tüm bw (4-decade) | 31 B/s |
| `BURST_WINDOW` [R3-ADIM6] | burst_cap = clamp(rate·BURST_WINDOW, 1 birim, ~2×MTU); bant-kör 2×MTU dump CANCELLED | tüm bw (4-decade) | 40 ms |
| refill precision [R1-ADIM6] | sub-byte kesir-kalan taşınır; per-tick tamsayı-truncation (rate<1000 B/s'te stall) YASAK | tüm bw | ns-ölçek integer |
| `sack_wire` [R15] | 28(UDP/IPv4)+22(handle4+seq2+tag16)+ACK_FRAME_V2 gövde(15+4·range), 0 range | tüm bw | 65B |
| `ACK_DELAY_UNIT` [TUR-1x] | V2 ack_delay birimi; tavan 65535×birim ≈ 16.78s > ACK_CADENCE floor 8.39s (satürasyon beklenmez, sayaçla denetlenir) | tüm bw (4-decade: floor-encoding hızlı kanalda ≤256µs yuvarlama = ihmal) | 256 µs [STD-karar D-4/1] |
| min-rtt guard [TUR-1x] | `raw−ack_delay ≥ rtprop` değilse düzeltme REDDedilir, RAW işlenir (örnek atlanmaz; §6/§7.3, RFC 9002 §5.3). **CANCELLED:** mutlak `MIN_RTT_FLOOR=1ms` — loopback sub-ms gerçek RTT'de tüm örnekleri reddedip estimator açlığı yaptı (ZM-2 ihlali, bench-loopback ölçümü) | tüm bw (4-decade: gözlenen-min kanal-türevi) | — (sabitsiz) |
| `K_c` cadence-eşik çarpanı [TUR-1b] | loss time-threshold tabanı = K_c×EAI (§5b); EAI = ACK_FREQ_N×ort_birim/send_rate + sack_wire/return_bw_önseli(31 B/s [CAL], §3.3-E-FULL ölçümle değişir) | tüm bw (EAI kanal-türevi; önsel-kolu worst-corner sabit — hızlı kanalda eşiği ~3.1s'e taşır, deadline≫ olduğundan kabul; ölçülen return_bw gelince daralır) | 1.5 [CAL matris {1.25, 1.5, 2.0}] |
| `K_wl` W_loss pencere çarpanı [TUR-2a] | W_loss = K_wl×ack_epoch, ack_epoch = max(srtt, EAI) (§5 ledger; D-1/5) | tüm bw (kanal-türevi) | 8 [CAL, K_bw/K_rtprop merkeziyle hizalı] |
| ledger confirm-ufku [TUR-2a] | Suspected→Confirmed = cadence-taban (K_c×EAI) kanıtsız geçince — geç-SACK'in makul varış penceresi; saat-sınıfı = eşikle AYNI (ZM-4) | tüm bw | = K_c×EAI (ayrı sabit YOK) |
| `LEDGER_MAX` [TUR-2a] | olay-ledger + pencere-paydası kayıt tavanı; en-eski-önce eviction + audit sayacı (SR-AUDIT-001; tombstone MAX ailesi) | tüm bw (bellek-tavanı sabit) | 4096 |
| `K_TOMB` tombstone TTL çarpanı [TUR-2b] | TTL = K_tomb×T_loss(ilan-anı); T_loss = max(9/8·srtt, cadence-taban) — eşikle AYNI saat (ZM-4). Onaylı formülün `min(mesaj-deadline,…)` kolu motorda UYGULANAMAZ (per-unit deadline engine'e taşınmıyor; bench/app-katmanı) → TTL tek-kollu ZEYL | tüm bw (kanal-türevi) | 8 [CAL] |
| `TOMBSTONE_MAX` [TUR-2b] | geç-ACK eşleştirme kayıt tavanı; en-eski-önce eviction + `tombstone_evicted` audit (evict edilenin geç-ACK'i Expired/Unknown — örneksiz) | tüm bw (4-decade doluş: 0.25k≈11, 1M-heavy≈450 ≪ 4096; bellek ≤~350KiB) | 4096 [CAL] |
| `RTT_floor(unit)` | (unit_wire+sack_wire)/floor_bw + PROP_MARGIN | pace edilen taban | 256B@0.25k≈11.9s; 64B@1k≈1.43s |
| `k_base` (COPY_BASE_FACTOR) [R1] | >1; ilk-SACK cadence-exempt → RTT_floor ilk-SACK modeli. Fallback (exempt değilse): ≥1+ACK_CADENCE/RTT_floor | tüm bw | 1.3-1.5 |
| `COPY_MULTIPLIER` | geometrik backoff (RFC 6298 analoğu) | tüm bw | 2 [STD] |
| `COPY_MAX_INTERVAL` | deadline/MIN_COPIES_IN_DEADLINE | deadline-türevi | deadline/8 |
| `T_PATH_DEAD` [R4] | geometrik-span < T < deadline−failover_marjı; IDLE_TIMEOUT'tan AYRI; silence=now−max(last_ack,first_send) | tüm bw | — |
| `RETURN_BUDGET_FRACTION` (Y) | SACK+echo dönüş ≤ Y·return_bw | tüm bw | 0.20-0.30 |
| `ACK_CADENCE(return_bw)` [R15] | sack_wire/(Y·return_bw); ilk SACK cadence-exempt | tüm bw | 0.25k≈8.3s; 1k≈2.1s; fast~20ms |
| `PROBE_BUDGET_FRACTION` (Z) | probe forward ≤ Z·forward_bw | tüm bw | 0.10-0.20 |
| `PROBE_PAYLOAD_SIZE` (probe_wire) | probe_baş+pad, pad=echo_gövde−probe_baş+margin → amp≤1 (AEAD içinde) | tüm bw | ~50B wire (echo 32B'yi domine) |
| `MINI_BURST_COUNT` [R-mb] | min(TARGET, floor(Z·forward_bw·1s/probe_wire)) | tüm bw | fast:3-5; 0.25k:0 (amortize ~10-20s) |
| `PROBE_RATE_LIMIT` | responder kaynak-başına echo/s (DoS taban) | responder | — |

### 16.2 Zaman-modeli haritası — saat-sınıfı envanteri (NORMATİF) [TUR-A]

> **Neden var:** Motor 50-200 kbit'te yazıldı; orada pace-tick ≈ RTT-round (ikisi ms mertebesi) →
> saat-sınıfı karışıklıkları hiç patlamadı. Sub-kbps'te srtt 3-12 s, ACK cadence 2.1-8.4 s, birim
> serialization 2.3-9.1 s iken tick 1 ms — sınıflar 1000× ayrışır ve her gizli karışıklık ayrı patlar.
> Sınıfın ölçülmüş örnekleri: (1) INITIAL_RTT wall-clock [D2], (2) server ACK_TICK=20ms SACK-seli [D2],
> (3) startup_plateau round'u pace-tick sayması → STARTUP 3 ms'de biter, motor app-limited hızına
> kilitlenir [TUR-A trajektori], (4) W_BTLBW=10s wall-clock ("~10 RTT" yorumuna rağmen) → srtt=3s'te
> ~3 RTT → btlbw 88→4 çöküşü [TUR-A ölçüm]. Envanter: 94 mekanizma tarandı (74 + 20 verify-avcısı eki):
> **22 BREAKS / 35 SUSPECT / 17 SAFE**.

**Normatif kurallar (yeni kod dahil, İHLAL EDİLEMEZ):**
```
ZM-1  Olay-sayısı (on_event/tick çağrı adedi) ASLA zaman-vekili olamaz. "Round" = RTT-round
      (BBR delivered-anchoring) ya da açık bw-örneği; tick-frekansı değişince semantik DEĞİŞMEMELİ.
ZM-2  Wall-clock mutlak sabit ancak 4-decade (0.25k/1k/100k/1M) doğrulamasıyla girer; RTT/kanal-zamanı
      ölçen her eşik srtt/rtprop/serialization-türevi olmak ZORUNDA (ms sabiti değil).
ZM-3  Zaman ölçen her YENİ mekanizma bu tabloya saat-sınıfıyla eklenir; sınıfsız eşik CI-red gerekçesidir.
ZM-4  Karşılaştırmanın İKİ YANI aynı saati kullanmalı (örn. loss-threshold srtt-türevi iken elapsed'in
      içindeki receiver ACK-hold'u threshold'un saati GÖRMÜYORSA karşılaştırma geçersiz — aşağıda K-1).
```

> **ZM-2 AKTİF-KORUMA ÖRNEĞİ (ilk kanıt — TUR-1x, 2026-07-13; talimatla kayda geçirildi):** min-rtt
> guard'ın ilk tasarımı mutlak `MIN_RTT_FLOOR=1ms` wall-clock tabanıydı. ZM-2'nin şart koştuğu
> 4-decade doğrulaması (bench-loopback testi) tabanı ANINDA yanlışladı: loopback/LAN gerçek RTT
> sub-ms → TÜM örnekler reddedildi → estimator açlığı → PreConverge kilidi. Taban İPTAL edilip
> gözlenen-min (rtprop, kanal-türevi) guard'ıyla değiştirildi (§6, RFC 9002 §5.3). Bu, haritanın
> pasif envanter değil AKTİF koruma olduğunun ilk kanıtıdır: kural, hatayı üretim yerine testte
> yakaladı.

**Sınıf envanteri (BREAKS + kritik SUSPECT; tam liste denetim kaydında):**

| mekanizma | kod | fiili sınıf | sub-kbps davranış | gereken sınıf |
|---|---|---|---|---|
| time-threshold loss 9/8·srtt | loss.rs + sender on_timer | ~~rtt-türevi eşik ↔ ack-hold'lu wall-clock elapsed~~ → **FIXED [TUR-1a+1b]**: later-ACK kanıt-şartı + cadence-farkında taban max(9/8·srtt, K_c×EAI) | (eski) temiz kanalda SİSTEMATİK false loss (K-1, 27/27 ölçüldü) | ✓ eşik ≥ beklenen-ACK-cadence (§5b; K_c=1.5 [CAL]) |
| startup_plateau round | sender.rs | ~~event-count~~ → bw-örneği (TUR-A ölçüm değişikliği) | 3 ms'de STEADY → app-limited kilit (ölçüldü) | RTT-round (delivered-anchor); re-entry'de sayaç sıfırla |
| W_BTLBW=10s | bw.rs:12 | wall-clock ("~10 RTT" değil) | 0.8-3.3 RTT → btlbw 88→4 çöküşü (ölçüldü); 0.25k'da pencere ACK arası boşalır → btlbw=0 | max(10s, k·srtt) veya örnek-sayısı |
| W_RTPROP=10s | rtt.rs:16 | wall-clock | 1-4 örnek; 10s+ SACK boşluğunda tek örneğe düşer → rtprop=şişkin RTT | max(10s, k·srtt) |
| ack_delay wire u16 µs (65.5ms tavan) | receiver.rs/ack.rs | ~~wall-clock tavan~~ → **FIXED [TUR-1x]**: V2 0x14 256µs-birim (tavan 16.78s), floor-encoding + satürasyon-sayacı + MIN_RTT_FLOOR guard | (eski) 8.4s ACK-hold 65.5ms'e doyardı → RTT örneği ~8.3s şişerdi; ölçüm-2: 1.43s hold decoded+subtracted, rtt_rej=0 | ✓ saniye-yetenekli encoding (D-4/1 Seçenek-A) |
| ACK_CADENCE_FLOOR (RETURN_BW_PRIOR=31 const) | receiver.rs:89 | derleme-zamanı prior (statik 8.39s) | 1kbps'te 4× fazla throttle (2.1s yeterli); K-1'i besler | ölçülen return_bw'den adaptif (§3.3-E FULL) |
| DrainRtprop dwell | sender update_state `_=>Steady` | event-count(1) | probe 1 ms yaşar → rtprop HİÇ yenilenmez | ölçüm-kapılı çıkış (≥1 RTT / min-rtt örneği) |
| emergency escalate adımı | emergency.rs update() | event-count (çağrı başına) | 1ms pompada Arq→Dup3 2 ms'de (merdiven fiilen binary) | adım-başına RTT-türevi dwell |
| deescalate_dwell=1s | emergency.rs:95 | wall-clock | 1s < 1 srtt → tek ACK'te koruma tam iner (flap) | k·srtt veya ≥1 sağlıklı round |
| dup_spacing=20ms | emergency/sender | wall-clock | 20ms ≪ serialization → kopyalar wire'da bitişik (dekorelasyon amacı ölür) | k·birim-serialization |
| ack_confidence anchor `.or()` | sender.rs | — (semantik) | spec `max(last_ack, first_send)` DEĞİL → idle-resume'da bayat anchor → anında RecoverFwd | spec formülü + aktif-dönem re-base |
| fuzzy qd üyelikleri 50-400ms | fusion.rs | wall-clock mutlak | sub-kbps'te qd doğal olarak saniyeler → qd.high≡1 → rate yarıya | rtprop-göreli üyelik |
| fuzzy bw üyelikleri 50-200 kB/s | fusion.rs | mutlak | 31-125 B/s'te bw.low≡1; loss.low=0 ile TÜM kural ağırlıkları 0 → arq=0, pending_lost SONSUZA DEK drain edilmez (ölçüldü: retx=0, pend monoton) | log-ölçek/kanal-göreli kalibrasyon + sıfır-ağırlık fallback |
| loss_pre_fec_ppm ömür-boyu | loss.rs | ~~monoton ratchet (W_loss YOK)~~ → **FIXED [TUR-2a]**: W_loss olay-ledger'ı (`Suspected→Confirmed\|Spurious`, pencere-dışı Expired) + UN-LOSS (geç-ACK → Spurious: pending-retransmit iptal + `note_spurious` kablosu + kol-1 orijinal-RTT) + fuzzy girdisi `loss_windowed_ppm = Confirmed/pencere-gönderim` | (eski) false-loss kalıcı loss.high → fuzzy zehirlenirdi; ölçüm: p5-r4'te olaylar pencere-dışı decay (lppm→0), p5-r6'da canlı lppm=3% + ARQ tam draini | ✓ W_loss = K_wl×ack_epoch; confirm = cadence-taban (kanal-türevi, ZM-2) |
| MIN_CWND=4800B | rate.rs:93 | byte-mutlak | BDP'nin 3-48× → inflight-cap kolu hiç seçilmez | k×BDP tabanı |
| geometrik kopya deadline'sızlığı | sender dup_queue | deadline-türevi AMA tavansız | ölü kanalda sonsuz reschedule (COPY_MAX_INTERVAL/RETRY_BUDGET kodda yok) | §12 deadline tavanı DupRec'e |
| SAFE (doğru sınıf, örnek): pacer refill+kalan [R1], BURST_WINDOW [R3], PC geometrik takvim (RTT_floor-türevi), ACK_SILENCE=3·srtt, K=3 packet-threshold, ilk-SACK cadence-exempt | — | — | — | — |

**K-1 — Bilinen ölümcül KOMPOZİSYON (TUR-A ölçüm, atıflama sayaçlı):** temiz 1 kbps'te 27/27 loss-ilanı
time-threshold (packet-threshold=0). Zincir: eşik=9/8·srtt≈3.38s < SACK-varış-fiziği (2·serialization+
cadence-hold 4.6-8.4s) → her birim "kayıp" → `queue_losses` seq'i inflight'tan DÜŞÜRÜR → gerçek (geç)
ACK artık RTT/BW örneği ÜRETEMEZ (censor) → srtt düşük-yanlı kalır + W_BTLBW boşalır (btlbw 88→4) +
loss_ppm doyar → fuzzy arq=0 → pending_lost drain edilmez → state RecoverArq'a MIHLANIR (Startup
re-entry yolu da yok). Tek false-loss tüm ölçüm katmanını zehirler. Fix-sınıfı: eşik ack-cadence-farkında
+ un-loss yolu + eviction yerine geç-ACK'e açık muhasebe.

**K-1 KAPANIŞ DURUMU (TUR-1a→2b, hepsi ölçülü):** eşik-kolu ✓ (TUR-1a kanıt-şartı + TUR-1b
cadence-taban); ratchet/un-loss kolu ✓ (TUR-2a ledger — §5); **sansür-kolu ✓ (TUR-2b tombstone):**
`queue_losses` artık kaydı DÜŞÜRMEZ — active-flight muhasebesinden düşer (gönderim durmaz) ama
gönderim-kaydı (bw_sample dahil) TOMBSTONE'da yaşar; geç-ACK kol-1'de (retx'siz) BW+RTT örneği
KURTARILIR (§5 D-1/4). Kalan kollar: fuzzy arq=0 dejenerasyonu (TUR-4) + Startup re-entry/ProbeUp
(TUR-3c).

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

## Ek-B — CANCELLED / MUST-NOT-IMPLEMENT (PRE_CONVERGE devri, revize-D0 D1) [K-KEYSTONE/R8/deprecate]

> Sessiz silme YOK — eski modeller gerekçesiyle burada kalır (izlenebilirlik). §3.3 PRE_CONVERGE bunları supersede eder.

```
[CANCELLED] Sabit INITIAL_RTT-seed cold-start modeli:
  Neydi: ilk RTT örneği gelene kadar srtt = INITIAL_RTT (sender.rs ≈100ms) + time-threshold loss detection.
  Neden CANCELLED: kbps'te ilk ACK ~1-2s uzakta ama 112ms'de "kayıp" ilan → spurious retransmit fırtınası
    (temiz 1 kbps'te 3× byte, ölçüldü). MUST NOT IMPLEMENT — yerine §3.3 en-kötü-köşe önsel + PRE_CONVERGE'de
    loss-ilanı DONDURULMUŞ (§3.3-C/1) + geometrik kopya (§3.3-B).

[MUST-NOT-APPLY (PRE_CONVERGE)] min_rate_bps=10000 fixture (budget.rs):
  Neydi: send_rate clamp floor'u 10000 B/s (=80 kbps), her state'te uygulanıyordu.
  Kural: PRE_CONVERGE'de floor = PRE_CONVERGE_FLOOR_BW (§16.1); min_rate=10000 MUST-NOT-APPLY (1 kbps
    kanalı 64× floodlar → self-congestion). NOT: steady-state minimal-reliable floor'un bandwidth-aware
    hale gelmesi (ölçülen bw'nin fraksiyonu, 4-decade) = ADIM-6 ana gövde kapsamı; burada BLANKET-deprecate DEĞİL.

[CANCELLED] below-Noise unauthenticated probe (revize-D0 keystone-öncesi / task §2.3 "Noise ALTINDA"):
  Neden CANCELLED: INV-CRYPTO-1 / §1 taksonomisi (unauthenticated transport-emitted class YOK) + §2a
    plaintext-magic + on-path echo-forge ihlali. Yerine §11.1 Noise-İÇİ authenticated CTRL_PATH_PROBE.
```

---

**Statü:** IMPLEMENTATION DRAFT — bölüm iskeletleri + formüller + wire layout + state machine tam; `[CAL]` sabitleri netem baseline'dan (ADR-18) kalibre edilecek, ardından golden vektörlere (§16) işlenecek. Bu belge tamamlanınca `Transport Protocol Specification v1.0`'a terfi eder. **PRE_CONVERGE cold-start (§3.3/§11.1/§16.1/Ek-B) revize-D0'dan D1 patch'iyle eklendi (docs/design/pre-converge-v1.md).**
