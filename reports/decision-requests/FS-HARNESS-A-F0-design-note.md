# FS-HARNESS-A — F0 CLOSED-LOOP DESIGN NOTE (koddan-önce; onay-bekler)

**Statü:** Implementasyon-ÖNCESİ mühürleme (Abkarada). Her madde gerçek-koda referanslı (module:line).
KRİTİK-yeni-bulgu §0'da (crate-placement).

## 0. CRATE PLACEMENT (yeni; bağlayıcı)
`adaptive-transport` **FEC-codec-BAĞIMSIZ** (INV-ENGINE-SOC; Cargo.toml'da kama-fec-codec YOK). Dolayısıyla
**F0 closed-loop `fs_harness`(adaptive-transport)'ta YAŞAYAMAZ** — gerçek-decoder erişimi yok. F0 (+ real-
decode A0/A1/F0 karşılaştırması) **`kama-integration`**'da kurulur: hem gerçek `Sender` (dev-dep adaptive-
transport) hem gerçek-decoder (`kama-fec-codec::RsSimdCodec` + `kama-engine::admit_and_decode`) hem gerçek
`ReceiverHost` orada birlikte. A0/A1 primitifleri (TraceChannel/BottleneckChannel/A1EarlyEmitter/RedundancyCost/
a1_duplicate_target_set) `adaptive-transport::{fs_trace,fs_harness}`'ten dep+feature ile yeniden-kullanılır.

## 1. Amaç
F0 = current block-close FEC + **fixed configured R** + existing ARQ substrate. **Historical control.**
DEĞİL: adaptive-FEC · controller-selected-R · early-emission-FEC.

## 2. Sender authority wiring (gerçek call-path)
```text
AppData → schedule_transmit → drain_new_data (K source; block'a doldurulur)
  → [SEMANTIC POINT = block-close: bloğun K-source'u drain-edildi]
  → s.set_manual_fec_enabled(true)                         [sender.rs:2336 — F0 PURITY: Manual, fixed]
  → let a = s.open_fec_block_authority()                    [sender.rs:2322 → FecBlockAuthority{instance,gen,mode}]
  → s.close_fec_block_manual(a, block_id, manual_r, parity_size)  [sender.rs:2393 → esi 0..manual_r push_parity_authorized]
  → pending_fec (VecDeque)                                  [sender.rs:847/2375]
  → plan_fec_parity (schedule_transmit; drain-SONRASI)      [sender.rs:2753 — DATA-before-parity MÜHÜRLÜ sender.rs:2287]
  → TransmitDecision{kind=FecParity, seq, unit_id(=parity), size} 
  → BottleneckChannel (§4)
```
- **authority block_id:** ölçülen tek-blok (block_id=0; harness tek-message/block).
- **kaç kez:** blok başına **BİR** open+close (block-close'da). Tekrar-açma YOK.
- **configured R:** YALNIZ prereg-arm-config'ten (`manual_r` sabit); Controller/PathTiming/fuzzy-DEĞİL.
- **block-close event:** bloğun K-source'unun drain'i (harness: K AppData emisyonu tamamlandığında).
- **parity eligibility:** `plan_fec_parity` (drain-sonrası; sealed ordering korunur).
- **PURITY (mühürlü):** configured-R-only · PathTimingState-TÜKETMEZ · fuzzy/controller-R-YOK · future-loss-YOK.
  (Controller-yolu `fec_controller_latch` sender.rs:2446 F0'da **KULLANILMAZ**.)

## 3. TraceChannel role mapping
```text
InitialSource(attempt=0)  : yeni-source (Data)          → ExactDeficit D YALNIZ bunu düşürür
RetransmitSource(attempt≥1): ARQ retransmit             → bağımsız redundancy-policy (PassAll)
FecRepair(repair_index=esi): FecParity                  → bağımsız redundancy-policy (PassAll)
```
D = tüm-arm'larda AYNI initial-erasure-set (source_id keyed). Retransmit/parity D'den bağımsız değerlendirilir
(mevcut fs_trace `RedundancyPolicy` — attempt-aware, e7b8503).

## 4. Physical egress
F0 parity: **AYNI BottleneckChannel** (`offer`) → serialization + link_free-kuyruğu + prop; realized-sealed-
wire-accounting'e girer. **Special/unmetered parity-lane YOK.** (fs_harness bottleneck-drops-consume-budget
pini parity'ye de uygulanır.)

## 5. Receiver / decode modeli — **GERÇEK CODEC (fake-count YASAK)**
```text
received InitialSource/Retransmit/Duplicate(source) → distinct-source-set (dedup: source_id; dup-2.-kopya IGNORE)
received FecParity                                   → repair-set (repair_index ayrı-namespace)
when (distinct_source + known_zero) ≥ K_shape:
  kama-fec-codec RsSimdCodec::reconstruct(profile, &[ReceivedSymbol], known_zero_count)  [codec.rs:68 — GERÇEK reed-solomon-simd]
    → Vec<RecoveredSource>                            → eksik-source BYTE'ları reconstruct
  → reconstructed-source normal-receive-state'e yazılır (delivered-set) → ordinary delivery
```
**YASAK:** `received_count ≥ K → complete` (fake). Zorunlu: gerçek `reconstruct` (RS-decode) — aksi yalnız-
scheduling-test-edilmiş-olur, codec+transport-closed-loop-DEĞİL.
- **dedup:** source_id-keyed delivered-set; aynı-source'un duplicate/retransmit-2.-kopyası IGNORE.
- **parity identity:** FecRepair-role + repair_index (source-namespace'ten ayrı).
- **reconstructed → receive-state:** delivered-set'e yazılır (source-byte'ları RecoveredSource'tan).
- **decode → ARQ-state kapanışı:** FEC-reconstruct edilen source için **RECOVERED-mekanizması** (DR-S5-4;
  on_recovered_control → pending_lost.retain-cancel, sender.rs:2642) veya ACK-tabanlı-kapanış (reconstructed-
  seq ACK → Sender delivered-görür). **Karar:** RECOVERED (product-faithful; ACK yerine — source wire'da
  ALINMADI, decode-edildi). Residual-ARQ: reconstruct-EDİLMEYEN source normal-retransmit.

## 6. Hybrid semantics (ARQ kapatılmaz)
F0 = **ARQ + proactive-FEC**. `r < deficit`: FEC tek-başına-tamamlamak-zorunda-DEĞİL → residual-ARQ normal-
çalışır (kayıp-source retransmit ile gelir). `r ≥ deficit`: parity zamanında-ulaşırsa FEC-complete olabilir.
**Harness FEC-lehine ARQ'yu KAPATMAZ** (F0_EXACT_D2_R1 pini bunu kanıtlar).

## 7. Timestamp mapping
```text
generic (redundancy_kind=FEC_REPAIR):  t_redundancy_{eligible,enqueued,wire,arrival}
legacy (birebir aynı):                 t_parity_{eligible,enqueued,wire,arrival}
  eligible = close_fec_block_manual/plan_fec_parity anı · enqueued = pending_fec push · wire = channel.offer
  arrival = channel Some(arrival) (drop→None)
t_source_loss = deterministic-TraceChannel INITIAL-source DROP-anı (ACK-tahmini DEĞİL)
t_arq_{eligible,wire,arrival} = Sender Retransmit-decision/channel (ARQ-yolu)
t_block_complete = TÜM-K-source delivered (original|retransmit|dup|FEC-reconstruct) — **reconstruct/delivery-SONRASI**
```
BlockTiming.record_redundancy(FecRepair,...) generic+legacy'yi birlikte doldurur (fs_harness; test-pinli).

## 8. Wire accounting (A1 ile aynı domain)
```text
per-block: configured_r · planned_r · enqueued_r · wire_r
           redundancy_payload_bytes · redundancy_framing_bytes · realized_sealed_wire_bytes · redundancy_packet_count
```
**C-P4:** configured-R-equality yalnız-metadata; **gerçek-fairness = realized_sealed_wire_bytes** (A1 vs F0);
frozen-tolerance-dışı → **BUDGET_MISMATCH — NO CODING CAUSAL EVIDENCE**.

## 9. Required F0 sanity pins
```text
F0_LOOPBACK       : loss-yok → normal-delivery; parity-varlığı substrate'i-bozmaz
F0_EXACT_D1_R1    : 1 initial-source-erased + 1 parity → GERÇEK-decoder reconstruct → FEC-completion
F0_EXACT_D2_R1    : yetersiz-FEC → residual-ARQ-gerekir → completion yine-ARQ-ile (ARQ-kapatılmadı KANITI)
F0_EXACT_D2_R2    : yeterli-parity → decode-çalışır (repair zamanında-ulaşırsa)
F0_PURITY         : configured-R PathTimingState'ten-BAĞIMSIZ; controller/fuzzy-etkisi-YOK
F0_EGRESS         : repair gerçek-BottleneckChannel-serialization-tüketir
F0_ACCOUNTING     : payload/framing/sealed-bytes tutarlı
F0_FLAG_OFF       : current product-behavior byte-özdeş (harness experimental-flag-arkası)
```

## 10. Expected invariants (yeşil-kalmalı)
A1/A2 regressions · flag-OFF · resource-bounds · **DATA-before-parity ordering** · no-production-default-activation.

## 11. Known risks (açık-liste)
```text
- authority yanlış-semantic-point'te (block-close-DEĞİL) açılır
- synthetic-decoder gerçek-codec'i yanlışlıkla-değiştirir (fake-count) → EN-TEHLİKELİ
- parity physical-bottleneck'i bypass-eder (free-lane)
- ARQ harness-tarafından-kapatılır (F0=ARQ-disabled) → F0_EXACT_D2_R1 yakalar
- R yanlışlıkla-adaptive (Controller/PathTiming sızar)
- duplicate/parity accounting-domain'leri ayrışır
- t_block_complete reconstruct/delivery-ÖNCESİ alınır (erken-completion)
```

---
**EN KRİTİK ŞART (Abkarada):** receiver'da **gerçek `kama-fec-codec::reconstruct`** kullanılır; `7-source+1-parity
→ 8-symbol → complete` gibi fake-rule YASAK. Ve **F0_EXACT_D2_R1** ile ARQ'nun residual-recovery'ye devam-ettiği
kanıtlanır (harness farkında-olmadan "FEC-arm = ARQ-disabled" olamaz).
