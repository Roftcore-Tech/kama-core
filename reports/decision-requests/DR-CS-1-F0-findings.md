# DR-CS-1 — F0 TESPİTLERİ + DUR-A KARARLARI

> Bkz. `DR-CS-1-minimal-bootstrap.md` (normatif tasarım). Bu rapor F0'ın 8 tespiti + DUR-A
> kararlarıdır (DUR-A = rapor-et-ve-devam). Tarih: 2026-07-24 · dayanak-commit: `7880e84`.

## TESPİT (i) — Spec-statü + probe_seq genişliği + ECHO_WIRE_MAX

**SPEC-STATÜ: DRAFT-EXISTS.** `CTRL_PATH_PROBE_v2` probe/echo TAM-taslaklı:
`specs/adaptive-transport-engine-spec.md` §11.1 (satır 799-843; "Cold-Start Probe/Echo — NORMATİF"
başlıklı ama doküman-statüsü IMPLEMENTATION-DRAFT satır-1133; byte'lar "ONAY BEKLİYOR" —
`docs/design/pre-converge-v1.md:11`). Kod: SIFIR (hiç implement edilmemiş).

- **Probe layout (§11.1):** hub_handle u32(4) + frame_type=0x12(1) + subtype(1) + probe_seq
  u16_be(2) + pad(amp≤1) + tag(16). **probe_seq genişliği: u16_be** (öneri: KORU — echo-korelasyon-
  id'dir, PN değil; replay outer-pn32-window + control_id-dedup'ta).
- **⚠️ 0x10-0x18 ÇELİŞKİSİ: VAR → F2B kapı-(i) = DUR-B.** Otoritatif-KİLİTLİ registry
  (`kama-noise-wire-tdd-v2.0.md` §2.56 + `kama-transport-schema/src/inner_kind.rs`; runtime-assert
  `pipeline.rs:502-505`): `0x10 ACK · 0x11 SACK · 0x12 RECOVERED · 0x13 BLOCK_COMPLETE · 0x14
  PARAM_SET · 0x15 PARAM_ACK · 0x16 BLOCK_ABORT · 0x17 SESSION_CLOSE · 0x18 SESSION_RENEW_REQUIRED`
  — **0x10-0x18 TAM-DOLU; engine-spec'in 0x12=CTRL_PATH_PROBE'u RECOVERED ile ÇAKIŞIYOR** (0x11/
  0x13/0x14 de çakışık). K-WIRE-CTRL-1: tüm-kontrol-frame'ler §2.56-allowlist'ten; "yeni-tip =
  spec-değişikliği". **Çözüm-yolu (onay-gerektirir):** probe/echo'ya 0x19+ reserved-alanından yeni
  inner_kind çifti (allowlist-genişletme = DR-seviyesi spec-değişikliği; §5 "spec-edit"-yasağı
  nedeniyle bu turda YAPILAMAZ).
- **ECHO_WIRE_MAX = 43B (TÜRETİLDİ — uydurma değil):** kilitli sealed-control yolu üzerinden:
  EXTENDED-outer 26B (10B header + 16B tag; 101B@E=64-golden `26+(11+E)` `protector.rs:337`) +
  control-inner-prefix 6B (`inner_version 1 + inner_kind 1 + control_id 4`; `inner.rs:14-15`) +
  echo-body 11B (subtype 1 + probe_seq 2 + seen_count 4 + arrival_gap 4) = **43B** (subtype-düşerse
  42B). Engine-spec'in 32B'si BAYAT (11B eksik sayım). Amp≤1 pad-hedefi: probe_wire ≥ 43B →
  [CAL] PROBE_WIRE_MAX=44B TUTARLI. `B_COLD_ECHO_TX_BUDGET = 4 × 43B = 172B`.
- **Replay:** sealed-control-yolu otomatik sağlar (pn32-window 2^14 + control_id-dedup §2.57);
  engine-§11.1'in kendi zarfı PN'siz (tek-başına under-specified) → F2B'de noise-wire-yolu-şart.

## TESPİT (ii) — Alıcı-terk envanteri

**RECEIVER-PATIENCE-LOCUS: HARNESS-ONLY.**
- Harness rx (`udp_runner.rs`): deadline (caller; exit-2) + **8s-idle-fallback** (:121; yorum "5s"
  BAYAT) + FINISH-marker≥3 (:124). Terk-sonrası pending-blok ledger'a `-1` yazılır.
- `bench/kama.rs serve_loop`: SONSUZ + peer-map eviction'sız (unbounded).
- **Ürün-alıcı (`receiver.rs`): HİÇBİR terk/patience/eviction YOK** — saf ACK-pacing/codec; rx-set
  `BTreeSet<u64>` **unbounded**. `kama-engine`: `admit_and_decode` stateless; `MAX_PREPARAM_SHARDS=
  256` (count-cap, timer'sız); `RECOVERED_MAP_CAP=4096`. noise-wire session-store TTL "Sprint-4"e
  ertelenmiş.
- Ürün per-blok shard-assembler HENÜZ-YOK (gelecek-host: `PreParamHold`-şablonu).
- **RX_COLD_*_CAP kaynakları:** MAX_PREPARAM_SHARDS=256 (alıcı-taraf-şablon) + BOUNDED-4096-ailesi
  (LEDGER_MAX/RECOVERED_MAP_CAP/TOMBSTONE_MAX/CONFIRMED_LOGICAL_CAP); byte-cap = block_cap ×
  k_shape × symbol_size.

## TESPİT (iii) — PTO-geriye-dönük kod-tespiti

**RETRO-PTO: ALREADY-IN-CODE** — (d)-yetkisi YENİ-kod-değil-kilit-açma. Tek-donduran state-gate
`sender.rs:1177`; loss-detector inflight'ı orijinal `send_ns` ile tutar → srtt-kilidi açılınca
(bootstrap-SRTT) bir `Timer::Pto` mevcut `detect_time_losses` yolundan geçmiş-outstanding'i
değerlendirir. **Caveat (rapora-bağlayıcı):** `loss.rs:356` yalnız `seq < largest_acked` aday —
largest_acked-üstü tail mevcut-RecoverFwd-liveness-yoluna kalır (ARQ-kural-değişikliği YOK; (d)
sınırı-içi; DI-1'de davranış-belgelenir).

## TESPİT (iv) — Git-history prior-mechanism

**NO-PRIOR-MECHANISM** → DUR-B-tetiklenmedi. Kodda hiçbir cold-start probe/keepalive/self-wakeup
mekanizması var-olup-kaldırılmamış. Tek-yakın-şey: CTRL_PATH_PROBE_v2 TASARIMI (pre-converge-v1
§11.1; hiç-onaylanmadı/hiç-implement-edilmedi; main'den-çıkarılışı WIP-doc-hijyeni PR#35).
False-positive'ler (BBR ProbeUp/RtpropProbe/Pto = congestion-probing, farklı-şey) listelendi.

## TESPİT (v) — PreConverge yeniden-giriş + wire-görünürlük

**RE-ENTRY-IN-INSTANCE: IMPOSSIBLE** (srtt `Option` yalnız-construction'da-None; reset-yolu-YOK;
`sender.rs:946-953` "PreConverge'e geri DÖNÜLMEZ"). Yeniden-giriş ⇒ YENİ-Sender-instance = session
(yeniden-)kurulumu. **WIRE-DISTINGUISHABILITY: DISTINGUISHABLE-VIA(h_ext)** — yeni-session = yeni
hub-assigned h_ext (session_attempt_id + KALICI-MONOTON init_counter'a bağlı; stale-h_ext →
UnknownHandle-drop). → DUR-B-tetiklenmedi. **INV-CS-3 onaylı-yeniden-giriş-listesi = YALNIZ
session-(yeniden-)kurulumu.**

## TESPİT (vi) — T_W1_BOUND PTO-formül çıkarımı

Time-threshold: birim `t_s`'te-gönderilmiş; kayıp-ilanı `now − t_s ≥ (9/8)·srtt` VE taban
`cadence_loss_floor = (3/2)·EAI` (detect_time_losses(now, srtt, cadence_loss_floor);
`sender.rs:1178-1181, 1214-1216`). → `PTO_after_bootstrap_bound = max((9/8)·srtt_bootstrap,
(3/2)·EAI)` + Suspected→Confirmed-yolu ARQ-kuyruğuna. W-1'de `srtt_bootstrap` = kalibrasyon-
koşusundan (dondurulmuş); EAI = kanal-türevi (31 B/s-floor'da ≈ (65B×8)/31+... ≈ büyük — W-1
normal-profil RTT100'de koşulacağı için EAI-küçük).

## TESPİT (vii) — B4-feasibility (probe emisyon-yolu)

- Sender'da MEVCUT sender-side control-frame-yolu YOK (`SrPacketClass::Control` hiç-construct-
  edilmiyor); probe = yeni cfg-gated `TransmitKind`-varyantı + `schedule_transmit`'e YENİ-pass.
- **Ekleme-noktası:** `plan_fec_parity`(:1682)/`budget_settle`(:1685) SONRASI (out-öncesi) —
  DATA-önce-mühürlü-sıralama DOKUNULMAZ.
- **PROBE-TOKEN-FEASIBILITY: AFFORDABLE** (saturated-backlog @ 31 B/s floor): initial-burst 1×1200B
  → tokens=0; refill-31B/s; probe-fire anlarında tokens = **93B@3s / 235B@9s / 563B@21s** → 44B
  try_send-GEÇER. t=45s: 1200B-DATA-front-yeni-affordable-çakışması — gerçek-10ms-tick-driver'da
  DATA ~38.7s'te-çıkar → t=45s'te ~195B-token → probe-GEÇER (event-only-köşede ≤1.42s-regen;
  PACER_ERR=250ms-aşımı-köşesi F2A'da-ölçülür). `set_burst_cap`-clamp'i 44-93B-bakiyeyi-DÜŞÜRMEZ
  (cap=1200-sabit).
- **BATCH-REFACTOR-RISK: LOW** — tek-output-seam `sender.rs:982`; temiz-dikiş: :934-980=core
  (dispatch+fuzzy+state+pacer-prep), :982=schedule; 1:1 event→core+schedule korunur (pacer-refill-
  kadansı-bozulmaz).
- **Driver-envanteri (production/test-dışı):** `src/udp_runner.rs` 4-site (:344 AppData / :363
  Pto-10ms-tick / :402 Ack / :430 RecoveredControl) + `crates/bench/src/kama.rs` drive()-fan
  (:168/:217/:221/:244). examples'ta-yok. Testler-hariç (işaretli).

## TESPİT (viii) — EK4-A1 profil-türevi RX-bound tablosu (DONDURULDU)

Formül: `RX_COLD_ABANDON = max(75, T_elig + serial@31B/s + FDB + 2)`. `usable_rate` cold-state'te
= PreConverge-pacer-floor **31 B/s** (link-hızından-bağımsız; compute_send_rate→31). T_elig =
(3/2)(size+65)/31. Not: serial-terimi-konservatif (token-birikimi eligibility-beklemesiyle-örtüşür;
formül-normatif-olduğundan-toplanır).

| profil (FDB tek-yön) | birim | T_elig | serial | FDB | marj | B-dal | **RX_COLD_ABANDON** | 75s? |
|---|---|---|---|---|---|---|---|---|
| rtt100 (0.05s) | 65B | 6.3 | 2.1 | 0.05 | 2 | 10.4 | **75.0** | EVET |
| rtt100 | 300B | 17.7 | 9.7 | 0.05 | 2 | 29.4 | **75.0** | EVET |
| rtt100 | 1200B | 61.2 | 38.7 | 0.05 | 2 | 102.0 | **102.0** | **HAYIR → S-A(102s)/S-B** |
| rtt250 (0.125s) | 1200B | 61.2 | 38.7 | 0.13 | 2 | 102.0 | **102.0** | HAYIR |
| rtt1000 (0.5s) | 1200B | 61.2 | 38.7 | 0.5 | 2 | 102.4 | **102.4** | HAYIR |
| rtt2500 (1.25s) | 1200B | 61.2 | 38.7 | 1.25 | 2 | 103.2 | **103.2** | HAYIR |
| rtt5000-max (2.5s) | 1200B | 61.2 | 38.7 | 2.5 | 2 | 104.4 | **104.4** | HAYIR |
| (65/300B tüm-profiller) | | | | | | ≤31.8 | **75.0** | EVET |

**Sonuç: max-birim-1200B'de 75s-sabiti HİÇBİR-profilde-geçerli-DEĞİL** (102.0-104.4s) → W-3-1200B-
hücresi profile-derived-patience (≥104.4s tavan; harness **105s** kullanır) veya S-B. 65/300B → 75s.

---

## DUR-A KARARLARI (rapor-et-ve-devam)

1. **M6 kapsamı = Seçenek-2 (harness + spec-sözleşme).** Gerekçe: tespit-(ii) — ürün-alıcı
   patience-host'u YOK (per-blok-assembler inşa-edilmemiş). Sözleşme (EK4-A1-formül + S-B-şartı +
   INV-CS-RX-RESOURCE-cap'leri) DR-dokümanına yazılır; harness-rx implement-eder. → Hüküm-tavanı:
   **"SENDER-SIDE ... VALIDATED — PRODUCT RECEIVER PATIENCE CONTRACT UNIMPLEMENTED"**.
2. **S-A/S-B = S-A profil-türevi** (harness: 1200B→105s, 65/300B→75s); **S-B ürün-alıcı-için
   floor-profillerde ZORUNLU-işaretli** (viii-tablosu: 1200B'de 75s hiçbir-profilde-geçerli-değil).
3. **Yeniden-giriş-listesi:** yalnız session-(yeniden-)kurulumu (tespit-(v): in-instance-imkansız;
   h_ext+init_counter-ile-wire-distinguishable).
4. **F2B-STATÜSÜ: DUR-B (bekle-onay).** Tespit-(i) 0x10-0x18-çelişkisi (0x12 CTRL_PATH_PROBE ↔
   RECOVERED). Çözüm-önerisi (onaya-sunulur): probe/echo'ya **0x19+ reserved-alanından yeni-
   inner_kind-çifti** §2.56-allowlist-genişletmesi (K-WIRE-CTRL-1-prosedürü) + §11.1-byte-layout'un
   sealed-control-yoluna-uyarlanması (ECHO_WIRE_MAX=43B-aritmetiğiyle) + engine-spec-32B-düzeltmesi.
   **F0/F1/F2A ilerler** (talimat-yetkisi); F2B/F3/F4-tam-matrisi onay-sonrası.
