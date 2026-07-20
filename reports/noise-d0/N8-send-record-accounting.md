# N8 — SEND-RECORD / ACK-ACCOUNTING UYUM-RAPORU [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; kaynak-kod YAZILMADI). noise-wire-D0-A N8 (kama-noise-wire-tdd §12,
> P0-4 YENİ). Yalnız-OKUMA; cargo build/test çalıştırılmadı.
> **Otorite-spec'ler:** `specs/kama-noise-wire-tdd-v2.0.md` §0.5-P0-4 (SealedPacketMeta send-record, satır 147-156)
> · §3 (dört-kimlik + RETRANSMIT nonce-yeni, satır 329-337) · §4 (PN-model, satır 339-390) · §2.57 (RECOVERED
> control-body, satır 263-264). `specs/fec-core-tdd-v3.0.md` §10 (RECOVERED↔BtlBw PATCH-6, satır 304-315) +
> §9 (ÜÇ-FEEDBACK, satır 299-301). **INVARIANT'lar:** CLAUDE.md `INV-RATE-1` + fec-core §10 `RECOVERED≠ACK`.
> **HÜKÜM (özet):** iki-yüksek-değerli-invariant (**INV-RATE-1** + **FEC-recovered≠ACK**) mevcut-muhasebe-kodunda
> **UYUMLU** — loss'un send_rate'i düşürdüğü VEYA recovered-symbol'ün BtlBw'ye kredi verdiği **HİÇBİR yol yok.**
> Ancak P0-4 **SealedPacketMeta** send-record modeli **YOK**: mevcut `InflightPacket` yalnız transport-ordering
> kaydı (seq→OpaqueUnit{id,size}); PN→logical(block_id/esi/message_id) eşleme · dh_epoch · sealed_len · packet_class
> (source/repair/control) · send_generation **hiçbiri yok** → D1-EKSİĞİ. NET: **UYUMLU-ama-ISKELET**.

---

## (1) SPEC NE DİYOR (§0.5-P0-4 · §10 · INV-RATE-1)

### noise-wire §0.5 P0-4 — SEND-SIDE-WIRE-IDENTITY [NORMATİF, satır 147-156]
"receiver-tarafı-OpenedShard-tanımlıydı ama sender-tarafı wire-PN→logical-eşleme-YOKTU. ACK-gelince-engine-bilmeli:
**wire_pn → {packet-class, block_id, ESI, source/repair/control, retransmission-generation, sealed-bytes,
send-timestamp, dh_epoch, deadline}**." Sözleşme-tipleri (satır 151-153, aynen):
```rust
pub struct SealedPacket { pub bytes: Bytes, pub meta: SealedPacketMeta }
pub struct SealedPacketMeta { pub full_wire_pn: u64, pub dh_epoch: u8, pub sealed_len: usize, pub logical_ref: LogicalPacketRef,
                              pub packet_class: PacketClass, pub send_generation: u32 }
```
"Engine/pacer send-record-oluşturur; ACK/replay/FEC-logical-state bu-kayıt-üzerinden. (Bunsuz: ACK-hangi-shard'ı-
teslim-sayar? repair-ACK-BtlBw'ye-kaç-byte? recovered-hangi-retx-iptal? R_sent↔R_acked-ayrımı? RTT-hangi-tx'ten?)"
(satır 155-157). **Kabul-kriteri K-WIRE-SENDREC-1 (§11, satır 490):** "her-sealed-packet SealedPacketMeta-üretir;
ACK→logical-eşleme-deterministik."

### noise-wire §3 — RETRANSMIT + KRİPTO-REPLAY↔LOGICAL-DEDUP [NORMATİF, satır 335-337]
"RETRANSMIT: (block_id,source_esi)-AYNI · wire-PN+nonce-YENİ." Geç-original: "AEAD+outer-replay-geçer → wire-
muhasebe → logical-duplicate → rank-artmaz → app'e-2.teslim-YOK → **RTT/BtlBw yalnız-gerçek-wire-ACK.**" (satır 337).
`send_generation` (P0-4) = bu retransmission-generation sayacı.

### fec-core §10 — RECOVERED↔BtlBw [NORMATİF — KARAR-VERİLDİ, PATCH-6, satır 308-312]
> "FEC-ile-reconstruct-edilen-application/source-byte'ları wire_delivered_B / BtlBw / delivered-EAI'ye
> **YENİ-teslim-edilmiş-wire-byte-olarak-EKLENMEZ.** BtlBw YALNIZ-gerçekten-gönderilmiş-ve-network-ACK-ile-teslimi-
> kanıtlanmış-SEALED-wire-byte'lardan-kredi-alır. RECOVERED-feedback yalnız: logical-delivery + retransmission-
> cancellation + retention-release; **ACK-yerine-GEÇMEZ, bandwidth-credit-ÜRETMEZ.**" [UYUM-DOĞRULAMASI: kod-farklı-
> davranıyorsa-KOD-düzeltilir.]

- **BtlBw-ÇİFT-SAYIM-YASAK (satır 307-308):** "1000B-repair→1000B-recovered→delivered=2000B YANLIŞ; wire=1000B."
- **ÜÇ-FEEDBACK (§9, satır 299-301):** network-ACK (RTT/BW/loss) · **recovered-delivery (retransmit-iptal, credit-YOK)** ·
  block-completion (retention-serbest). LATE/DUP: "FEC-recover-sonrası-original: wire-muhasebe + logical-ESI-duplicate
  + app'e-2.teslim-YOK + rank-artmaz."
- **noise-wire §2.57 RECOVERED(0x12) body (satır 263-264):** "block_id · esi_range... (logical-recovery-YALNIZ;
  **ACK/BtlBw-credit-DEĞİL;** retransmit-cancel+retention-release)."
- **K-FEC-CC-1 / K-FEC-ARQ-1 (§ kabul, satır 368-369):** "recovered BtlBw-şişirMEZ (Y3-uyumlu)" · "FEC-recovered-source
  gereksiz-retx-YOK; congestion-loss-gizleMEZ (RECOVERED≠ACK)."

### INV-RATE-1 [CLAUDE.md §4 — İHLAL-ETME]
"loss send_rate'i DÜŞÜRMEZ. Rate = delivered-rate (BtlBw) + queue_delay ile pace edilir; **loss ayrı sayaç, yalnız
FEC/ARQ recovery kolunu besler.**" fec-core §10 `LOSS→CC` (satır 315): "raw-loss-her-zaman-görünür; public-path'te
recovered-source-kaybı-CC-için-KAYIP. **Model-B-özel-mode**" (Alpha-dışı; şu-an kodda-yok — N8-notu §3).

---

## (2) MEVCUT KOD NE YAPIYOR (dosya:satır)

**Muhasebe-yüzeyi tümüyle `crates/adaptive-transport` (transport-motoru) tarafında.** Kripto/PN/sealed-katmanı YOK
(N1-bulgusu: `crypto` crate 3-satır iskelet; wire'da `seq` full-counter'a çözülür, dh_epoch/replay yok). Aşağıdaki
harita P0-4'ün istediği alanları mevcut kayıt-yapılarına karşı eşler.

### 2.a Send-record yapısı — `sender.rs`
- **`InflightPacket` (sender.rs:138-144):** `{ unit: OpaqueUnit, send_ns: u64, bw_sample: SentPacket, retx_count: u8,
  origin: TransmitKind }`. Anahtar = `seq: u64` (`inflight: BTreeMap<u64, InflightPacket>`, ekleme sender.rs:1387-1396).
  Bu **mevcut send-record**'tur.
- **`OpaqueUnit` (sender.rs:74-79):** `{ id: u64, size: u64 }` — doc: "Transport yalnız KİMLİK+BOYUT görür
  (INV-ENGINE-SOC: bytes YOK). `id` çağıranın gerçek payload'una geri-eşlemesi içindir." → **`id` opak, çağıran-tanımlı;
  block_id/esi/message_id DEĞİL.**
- **`TransmitDecision` (sender.rs:113-122):** `{ kind, seq: u64, unit_id: u64, size: u64 }` — seq doc: "full-counter;
  wire'da u16, §2" (sender.rs:116-117). `next_seq` monoton (sender.rs:1367-1368: `let seq = self.next_seq; self.next_seq += 1;`).
- **`TransmitKind` (sender.rs:126-135):** `Data | Retransmit | Duplicate | FecParity`. **`FecParity` (satır 134):**
  "POLİTİKA; transport üretmez (güvenlik katmanı encode eder). **Emit EDİLMEZ.**" — grep teyidi: `FecParity` sender.rs'te
  YALNIZ enum-tanımında (satır 134) görünür; **hiçbir yerde `track_sent`/schedule edilmez.**
- **`track_sent` (sender.rs:1360-1398):** send-record oluşturucu. `seq` atar, `bw_sampler.on_sent` (sender.rs:1381-1383),
  `loss.on_sent` **yalnız `origin==Data` için** (sender.rs:1384-1385: `units = u64::from(origin == TransmitKind::Data)`),
  `inflight_bytes += size`, `inflight.insert`.

### 2.b P0-4 alan-eşlemesi (madde-madde)
| P0-4 istenen | Mevcut kod | Durum |
|---|---|---|
| (a) **PN→logical (block_id/esi/message_id)** | `seq → OpaqueUnit{id,size}` (sender.rs:74-79,138-144); `id` opak-çağıran-token, logical-alan-ayrışması YOK | **YOK** (yalnız opak-id) |
| (b) **send-timestamp** | `send_ns: u64` (InflightPacket sender.rs:140; `SentPacket.sent_ns` bw.rs:29; LostRec.send_ns sender.rs:155; TombstoneRec.send_ns sender.rs:165) | **VAR** |
| (c) **sealed-byte size** | `OpaqueUnit.size` = wire-boyut (pacing/inflight); `SentPacket.size` bw.rs:26 | **KISMEN** (wire-size var; `sealed_len()` sözleşmesi/EXACT-formül yok — N7-konusu) |
| (d) **retx-generation** | `retx_count: u8` (InflightPacket sender.rs:142; LostRec sender.rs:152; TombstoneRec sender.rs:167) | **KISMEN** (retx-sayacı var; P0-4 `send_generation:u32` wire-generation değil, per-record) |
| (e) **packet-class (repair/source/control)** | `origin: TransmitKind{Data,Retransmit,Duplicate,FecParity}` (sender.rs:143) = transport-emisyon-türü; **FEC source/repair/control ayrımı YOK** | **YOK** (emisyon-türü ≠ PacketClass) |
| (f) **dh_epoch** | Yok; `AckFrame.ack_epoch:u8` (ack.rs:51) = "rekey/counter-wrap epoch" — kripto dh_epoch(0/1) DEĞİL (ack.rs:15-16) | **YOK** |
| (g) **deadline** | `LostRec.deadline_ns` (sender.rs:151) yalnız-kayıp-kaydında; send-record'da yok | **KISMEN** |

### 2.c ACK ve loss TERMINAL-STATE'leri
- **ACK-terminal (sender.rs:542-586):** yeni-acked seq için `inflight.remove(&seq)` (sender.rs:542) → `inflight_bytes -=`;
  eşleşen kayıt yoksa `tombstones.remove(&seq)` (sender.rs:552 — geç-ACK) → yoksa Expired/Unknown (örneksiz).
- **Loss-terminal (loss.rs):** `on_ack` packet-threshold kaybı → `declare(...Suspected)` (loss.rs:322-326); `detect_time_losses`
  time-threshold (loss.rs:360-363). **`LedgerState` (loss.rs:114-122): `Suspected → Confirmed | Spurious`**; pencere-dışı → Expired
  (loss.rs:224-225). UN-LOSS: kayıp-ilan seq'e ACK → `Spurious` (loss.rs:295-304). Kayıp birim → `pending_lost` (LostRec sender.rs:148-156)
  → retransmit VEYA unloss-iptal (sender.rs:596-619).
- **Tombstone (sender.rs:163-170):** kayıp-ilanı-sonrası SAKLANAN kayıt; geç-ACK eşleşmesi (sender.rs:552-585). Bounded
  `TOMBSTONE_MAX=4096` (sender.rs:260).
- **Değerlendirme:** de-facto **per-seq (per-transmission)** terminal-makinesi VAR
  (`Inflight→{Acked | Lost(Suspected→Confirmed) | Spurious | Tombstone→{late-ACK | Expired}}`). Ancak **per-class
  (source/repair/control) DEĞİL** ve logical(block_id,esi) terminal-state YOK — P0-4/§2.57 kapsamı eksik.

### 2.d RECOVERED-etkileşimi + BtlBw-kredi yolu (KRİTİK-DOĞRULAMA)
- **BtlBw'ye kredi YALNIZ gerçek-ACK'ten:** `btlbw.update(sample, ...)` iki-çağrı — (i) normal-ACK (sender.rs:545-546:
  `let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample); self.btlbw.update(...)`), (ii) tombstone kol-1 geç-ACK
  (sender.rs:560-561; tek-gönderim, retx_count==0 → gerçek-wire-teslim). `on_ack` (bw.rs:98-116) `delivered += pkt.size`
  yalnız **ACK'lenen gerçek-wire-paketten.**
- **`credit_delivered` (bw.rs:143-147, çağrı sender.rs:574):** superseded-retransmit'in **geç-ACK'i** — doc
  (sender.rs:569-573): "byte'lar wire'ı **gerçekten geçti**, sampler kümülatifine girer." Bu **gerçek-wire-ACK'tir**
  (retx kopyası wire'ı geçti+ACK'lendi), FEC-recovered-symbol DEĞİL → INV uyumlu. `btlbw.update` ÇAĞIRMAZ (rate-örneği
  üretmez, muhafazakâr Karn).
- **FEC-recovered ingest yolu YOK:** grep `recovered`/`credit` (crate-geneli) → sadece: `bw.rs:143 credit_delivered`
  (yukarıda, gerçek-wire), `sender.rs:574 credit_delivered`, `diag_delivered_credited` (sender.rs:431,575,1548 — teşhis-sayacı).
  **Hiçbir `on_recovered`/`recovered_source`→`bw_sampler`/`btlbw` çağrısı yok.** RECOVERED(0x12) control-frame ingest'i
  (§2.57) kod-tabanında YOK.
- **ADLANDIRMA-ÇAKIŞMASI (BULGU, ihlal-DEĞİL):** `diag_bw_recovered` (sender.rs:359,563,1545) = **tombstone-kol-1'den
  KURTARILAN BW-örneği** teşhis-sayacı; FEC-RECOVERED ile ALAKASIZ (gerçek-wire geç-ACK). D1'de FEC-RECOVERED
  yolu yazılırken isim-karışıklığı riski → §5-notu.
- **`on_unrecovered` (loss.rs:75,369-371):** `unrecovered_units += units` → **yalnız `residual_loss_ppm`** (loss.rs:377-379);
  BtlBw'ye/send_rate'e DEĞMEZ.

### 2.e INV-RATE-1 yolu (loss → send_rate?)
- **send_rate girdileri loss-KÖR:** `rate.rs::evaluate` (rate.rs:137-155) `send_rate = min(pacing_rate, cwnd/srtt)`;
  `effective_send_rate` (rate.rs:164-177) dört-girdi (pacing_gain, gain, BtlBw, cwnd/srtt) — doc (rate.rs:160-162):
  "Dört girdi de loss-kör → INV-RATE-1 uçtan uca." srtt loss-içermez (loss.rs:1: "loss = RECOVERY sinyali, RATE sinyali DEĞİL").
- **fusion send_rate_gain loss-KÖR:** fusion.rs:366-371 `sr1 = bw.low.min(qd.high)`; loss GİRDİ DEĞİL (modül-doc fusion.rs:9-10:
  "loss girdisi DEĞİLDİR → loss send_rate'e giremez"). loss yalnız fec/arq-kural-ağırlığında (fusion.rs:335-346, R1-R4) +
  emergency-tetiğinde (fusion.rs:374).
- **budget clamp:** `send_rate_final = clamp(fuzzy_rate,[min_rate,cap])` (budget.rs:52-54); MIN_RATE floor rate-düşüklüğünden,
  **loss'tan DEĞİL** (budget.rs:50, emergency.rs:4-6: "MIN_RATE floor loss'tan tetiklenmez").

---

## (3) HÜKÜM: UYUMLU / UYUMSUZ / YOK

| Invariant / P0-4 bileşeni | Bulgu | Hüküm |
|---|---|---|
| **INV-RATE-1** (loss send_rate'i düşürmez) | send_rate = f(BtlBw, rtprop, srtt, gain) — rate.rs:137-177; fusion send_rate_gain = f(bw,qd) loss-hariç fusion.rs:366-371; loss yalnız fec/arq/residual (loss.rs, fusion.rs:335-346); budget-clamp loss-bağımsız budget.rs:50-54 | **UYUMLU** |
| **FEC-recovered ≠ ACK** (recovered BtlBw'ye kredi vermez) | BtlBw-kredi YALNIZ `on_ack` gerçek-wire-ACK'ten (sender.rs:545-546,560-561; bw.rs:98-116); `credit_delivered` gerçek-wire geç-ACK (bw.rs:143-147); **recovered-symbol → BtlBw yolu YOK** (grep-teyidi); RECOVERED control ingest YOK | **UYUMLU** (ihlal-eden-yol yok; FEC-recovered-yolu henüz mevcut-değil) |
| **P0-4 PN→logical map** (block_id/esi/message_id) | yalnız `seq→OpaqueUnit{id}` opak (sender.rs:74-79,138-144) | **YOK** → D1 |
| **P0-4 dh_epoch** | yok; ack_epoch = transport-wrap ≠ dh_epoch (ack.rs:15-16,51) | **YOK** → D1 |
| **P0-4 packet_class (source/repair/control)** | `origin:TransmitKind` emisyon-türü, FEC-class ayrımı yok (sender.rs:126-135,143) | **YOK** → D1 |
| **P0-4 sealed_len / send_generation** | wire-size var (kısmen); EXACT-sealed_len + send_generation:u32 yok | **KISMEN/YOK** → D1 |
| **P0-4 send-timestamp / retx-generation / deadline** | send_ns VAR; retx_count VAR (kısmi); deadline yalnız-kayıp-kaydı | **KISMEN** |

**NET HÜKÜM: UYUMLU-ama-ISKELET.**
1. **İki-yüksek-değerli-invariant UYUMLU (HIGH-severity ihlal YOK).** `INV-RATE-1`: loss'un send_rate'i düşürdüğü
   hiçbir kod-yolu yok (rate/fusion/budget üçü de loss-kör; loss yalnız recovery/emergency koluna gider).
   `FEC-recovered≠ACK`: BtlBw'yi krediyle besleyen tek yol gerçek-network-ACK (`bw_sampler.on_ack`→`btlbw.update`);
   recovered-symbol'ü ACK-yerine sayan hiçbir yol **yok** — çünkü FEC-recovered ingest katmanı henüz kod-olarak mevcut
   değil. `credit_delivered` (bw.rs:143) bir istisna GİBİ görünür ama **gerçek-wire geç-ACK'tir** (retx kopyası wire'ı
   geçti+ACK'lendi), recovered-symbol değil → uyumlu.
2. **P0-4 SealedPacketMeta send-record modeli YOK.** Mevcut `InflightPacket` transport-ordering kaydıdır: PN→logical
   eşleme, dh_epoch, packet_class, sealed_len-sözleşmesi, send_generation **yok.** ACK→logical-shard eşlemesi (§0.5-soru:
   "ACK-hangi-shard'ı-teslim-sayar?") deterministik-DEĞİL çünkü `OpaqueUnit.id` opak-token, block_id/esi değil. Bu bir
   invariant-ihlali değil, **eksik-katman** → tümüyle D1-EKSİĞİ.
3. **Terminal-state makinesi var ama per-seq (per-transmission).** Acked/Lost(Suspected→Confirmed)/Spurious/Tombstone/
   Expired zinciri mevcut (loss.rs:114-122, sender.rs:542-619) — sağlam. Ama **per-class** (source/repair/control) ve
   logical(block_id,esi) terminal-state (§2.57 RECOVERED/BLOCK_COMPLETE etkileşimi) yok → D1.

**DR-ADAYI:** yok. (fec-core §10 `LOSS→CC Model-B-özel-mode`, satır 315, INV-RATE-1 ile çelişmez: Alpha-dışı, kodda-yok;
Alpha'da loss yalnız recovery-kolunu besler — kod bununla tutarlı. Spec-hatası tespit edilmedi.)

---

## (4) KOD-DÜZELTME/YAZIM PLANI (D1) — SealedPacketMeta-tabanlı muhasebe (spec DEĞİŞMEZ)

> Bu bir "UYUMSUZ-düzeltme" DEĞİL, **eksik-katman-yazımı**dır: mevcut `InflightPacket`/`bw_sampler`/`loss`-ledger
> muhasebesi INV-RATE-1 + recovered≠ACK açısından KIRILMAYACAK (dokunulmazlık fec-core §0.8-3: bw.rs-delivered-EAI,
> sender.rs-w_epoch/W_BTLBW, send_rate-loss-modeli DEĞİŞTİRİLEMEZ). Aşağıdaki E'ler bu-çekirdeğin ÜSTÜNE SealedPacketMeta
> alanlarını + logical-eşlemeyi + FEC-class terminal-state'ini EKLER.

- **E1 — `SealedPacketMeta` send-record (P0-4, §0.5 satır 151-153):** `InflightPacket`'i (sender.rs:138-144) SealedPacketMeta
  ile zenginleştir/yanına-koy: `{ full_wire_pn:u64, dh_epoch:u8, sealed_len:usize, logical_ref:LogicalPacketRef,
  packet_class:PacketClass, send_generation:u32 }`. Mevcut `seq`→`full_wire_pn` (aynı monoton-counter, sender.rs:1367);
  `send_ns`→timestamp (korunur); `retx_count`→`send_generation`'a taşınır. Tip **kama-transport-schema** crate'inde
  (schema-crate henüz sadece Cargo.toml — N1-bulgusu; K-SCHEMA-1).
- **E2 — `LogicalPacketRef` + PN→logical map (§3 dört-kimlik, satır 331-333):** `OpaqueUnit.id` opak-token'ı YETMEZ;
  logical_ref = `{ block_id:u32, esi:u16, message_id:u32 }` (FEC) veya `control_id:u32` (control) taşımalı. ACK gelince
  `full_wire_pn → logical_ref` deterministik-çözülür (§0.5: "ACK-hangi-shard'ı-teslim-sayar?"). SECURE-WIRE-IDENTITY =
  `(session_id, direction, dh_epoch, full_pn)` ≠ FEC-LOGICAL-IDENTITY `(session, block_id, ESI)` (K-ID-SEPARATION-1).
- **E3 — `PacketClass` enum (P0-4 (e); §2.56 inner_kind):** `{ Source, Repair, Control(kind) }` — transport `origin:
  TransmitKind` (sender.rs:126-135) emisyon-türüdür (Data/Retransmit/Duplicate), FEC-class DEĞİL; ikisi ORTOGONAL.
  ACK-muhasebesi "repair-ACK-BtlBw'ye-kaç-byte" (§0.5) için class-ayrımı şart. **KRİTİK-KISIT:** class-ayrımı BtlBw-kredi
  KURALINI DEĞİŞTİRMEZ — source-VE-repair sealed-wire-byte'ı ACK'lenince BtlBw-kredi alır (gerçek-wire-teslim); recovered-
  SYMBOL almaz (§10 PATCH-6). Bu ayrım yalnız telemetry (§10 SAYAÇLAR: acked_source/acked_repair) + retention/ARQ-cancel için.
- **E4 — dh_epoch alanı (P0-4 (f); §4 satır 342-347):** send-record'a `dh_epoch:u8`; ACK-muhasebesi epoch-scoped
  (§2.57 `acked_dh_epoch`). Mevcut `ack_epoch:u8` (ack.rs:51) transport-wrap → kripto-dh_epoch'a **YENİDEN-yorumlanMAZ**,
  ayrı-alan eklenir (N1-D1-3 ile ortak).
- **E5 — RECOVERED(0x12) feedback-yolu (§2.57 satır 263-264; fec-core §9 satır 300-301) — YENİ:** RECOVERED-control
  ingest'i → **yalnız** {logical-delivery-işaretle + `pending_lost`/dup-queue retransmit-CANCEL + retention-release}.
  **ASLA** `bw_sampler.on_ack`/`credit_delivered`/`btlbw.update` çağırMAZ (recovered≠ACK; K-FEC-CC-1/ARQ-1). Tip-düzeyi-
  koruma önerisi: RECOVERED-handler'ın imzası BtlBw-sampler'a erişemesin (compile-time SoC).
- **E6 — Per-class terminal-state makinesi (§9 ÜÇ-FEEDBACK, satır 299-301):** mevcut per-seq zinciri (loss.rs:114-122,
  sender.rs:542-619) KORUNUR; üstüne logical(block_id,esi) state: `{ InFlight → Acked(wire) | Recovered(logical) |
  Lost→Retx | Unrecoverable }`. LATE/DUP kuralı (§9 satır 301): FEC-recover-sonrası-original geç-ACK → **wire-muhasebe VAR**
  (BtlBw-kredi, gerçek-wire) + logical-ESI-duplicate (rank-artmaz, app'e-2.teslim-yok). Bu §3-satır-337 ile birebir;
  mevcut tombstone/late-ACK mantığı (sender.rs:552-585) bu davranışın transport-yarısını zaten yapıyor.
- **E7 — `send_generation` retransmit-nonce-bağı (§3 satır 335, K-RETX-1):** `retx_count:u8` → `send_generation:u32`;
  her retransmit yeni-full-PN+yeni-nonce (aynı-logical, sender `next_seq++` bunu yapısal sağlıyor sender.rs:1367).
  send-record generation'ı "recovered-hangi-retx-iptal" (§0.5) için logical_ref+generation ile çözer.
- **E8 — `sealed_len()` sözleşmesi (P0-3, N7-ortak):** `OpaqueUnit.size` (sender.rs:78) wire-size'dır ama EXACT-sealed_len
  sözleşmesi (`seal(p).len()==sealed_len(p)`) yok; pacer-reserve==send-byte doğrulaması (K-WIRE-SIZE-1) N7 ile birlikte.

---

## (5) D1'DE YAZILACAK EKSİKLER (kesin liste)

- **[D1-1] SealedPacketMeta send-record (KRİTİK, P0-4):** `{full_wire_pn, dh_epoch, sealed_len, logical_ref, packet_class,
  send_generation}` (§0.5 satır 151-153). Mevcut `InflightPacket` (sender.rs:138-144) yalnız transport-ordering; altı-alanın
  dördü (logical_ref, dh_epoch, packet_class, sealed_len-sözleşmesi) + send_generation:u32 YOK. **K-WIRE-SENDREC-1.**
- **[D1-2] PN→logical eşleme + LogicalPacketRef:** `OpaqueUnit.id` opak → `{block_id:u32, esi:u16, message_id:u32}` /
  `control_id:u32`. ACK→logical deterministik-çözüm; K-ID-SEPARATION-1 (kripto-replay-key ≠ logical-dedup-key).
- **[D1-3] PacketClass (source/repair/control) muhasebe-ayrımı:** telemetry-sayaçları (§10 satır 305:
  sent_new_source/retrans_source/repair/control · acked_source/repair · lost_source/repair_pre_fec · recovered_source ·
  unrecoverable_source · wire_delivered_B · app_delivered_B). Mevcut `loss.rs` yalnız tek-akış sent/lost/unrecovered-unit;
  class-ayrımı yok. **BtlBw-kredi-kuralı korunur** (source+repair wire-ACK'i kredi alır; recovered-symbol almaz).
- **[D1-4] RECOVERED(0x12) feedback ingest (§2.57/fec-core §9) — credit-YOK zorunlu:** logical-delivery + retransmit-cancel
  + retention-release; BtlBw/delivered-EAI/wire_delivered_B'ye DOKUNMAZ (§10 PATCH-6). Şu-an control-frame ingest'i yok
  (yalnız receiver.rs:39-48 RxKind::FecShard tip-yüzeyi, N3-tutarsızlık-bulgusu). K-FEC-CC-1/K-FEC-ARQ-1 forged-RECOVERED testi.
- **[D1-5] dh_epoch-scoped ACK-muhasebe (§4/§2.57):** `acked_dh_epoch` (ack.rs `ack_epoch:u8`'i yeniden-kullanMA; ayrı-alan).
  epoch-0/1 aynı-PN-değeri belirsizliği çözülür. N1-D1-3 ile ortak.
- **[D1-6] Per-class/logical terminal-state (§9 ÜÇ-FEEDBACK):** mevcut per-seq makinesi (Suspected/Confirmed/Spurious/
  Tombstone/Expired — loss.rs:114-122) KORUNUR; logical(block_id,esi) katmanı + BLOCK_COMPLETE(0x13) retention-release eklenir.
- **[D1-7] ÇİFT-SAYIM-guard testi (§10 satır 307):** "repair-byte + recovered-source-byte iki-kez sayılmaz"
  (1000B-repair→1000B-recovered→delivered=2000B YANLIŞ; wire=1000B) — golden/property test. Şu-an FEC-yolu-ölü olduğu için
  vektör yazılamıyor; D1-FEC-emisyonu açılınca zorunlu.
- **[D1-8] Adlandırma-temizliği (kod-hijyen, ihlal-değil):** `diag_bw_recovered` (sender.rs:359,563,1545) = BW-sample-
  from-tombstone; D1'de FEC-RECOVERED yolu gelince isim-çakışması riski → yeniden-adlandır (`diag_bw_from_tombstone` vb.).

### Kama-Noise-Wire N8 uyum durumu — özet tablo
| P0-4 / §10 bileşeni | Kodda | Nerede |
|---|---|---|
| INV-RATE-1 (loss send_rate'i düşürmez) | ✅ | rate.rs:137-177 · fusion.rs:366-371 · budget.rs:50-54 · loss.rs:1,56 |
| FEC-recovered ≠ ACK (recovered BtlBw-kredi vermez) | ✅ (ihlal-yok) | bw.rs:98-116,143-147 · sender.rs:545-546,560-561,574 (yalnız gerçek-wire-ACK) |
| send-timestamp | ✅ | sender.rs:140 · bw.rs:29 |
| wire-size (kısmi sealed-byte) | ◐ | sender.rs:78 · bw.rs:26 (EXACT sealed_len yok) |
| retx-generation | ◐ | retx_count:u8 sender.rs:142 (send_generation:u32 değil) |
| per-seq terminal-state | ✅ | loss.rs:114-122 · sender.rs:542-619 |
| PN→logical (block_id/esi/message_id) | ⛔ D1 | yalnız OpaqueUnit.id opak (sender.rs:74-79) |
| dh_epoch | ⛔ D1 | ack_epoch transport-wrap ≠ dh_epoch (ack.rs:15-16,51) |
| PacketClass (source/repair/control) | ⛔ D1 | origin:TransmitKind emisyon-türü (sender.rs:126-135) |
| SealedPacketMeta send-record | ⛔ D1 | InflightPacket transport-ordering (sender.rs:138-144) |
| RECOVERED(0x12) feedback (credit-YOK) | ⛔ D1 | ingest yok; FecParity emit-yok (sender.rs:134) |

**D0-ENTRY-KAPISI (N8 açısından):** iki-invariant UYUMLU → **engel yok** (HIGH-severity ihlal tespit edilmedi).
Belge-sonu D0-B-gate "SealedPacketMeta = PN→logical deterministic mapping = CLOSED" **tasarım-olarak** kapalı; **kod-olarak**
henüz yok → tümü D1-implementation işi (STATÜ-MERDİVENİ D1-IMPLEMENTATION-GO şartı, N8-tarafı).
