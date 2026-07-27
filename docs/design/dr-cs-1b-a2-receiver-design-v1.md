# DR-CS-1b / A2 — ÜRÜN-ALICI PATIENCE + RESOURCE ENTEGRASYONU — TASARIM v1 [ARŞİV — v2 YÜRÜRLÜKTE]

> **ÜST-NOT (2026-07-27):** Abkarada v1-incelemesi: *"A2 DESIGN DIRECTION: GO / A2 PRODUCTION
> IMPLEMENTATION v1: NO-GO — 6 bağlayıcı düzeltme sonrası v2 gerekli."* Altı blocker + ek düzeltmeler
> `dr-cs-1b-a2-receiver-design-v2.md`'de işlendi; **v2 geçerli belgedir.** Bu v1, v2'nin açıkça revize
> ettiği bölümlerde **MUST NOT IMPLEMENT** statüsündedir — tam liste (v2-verify-turuyla genişletildi):
> D-1 host-crate · P-1/P-2-patience-semantiği · **P-4-session-idle-anchor-koşulu** ·
> **§3.1-hizalama-tablosu (eski cap-adları)** · §3.2-cap-matematiği · **§3.3-global-cap-formülü
> (N_SESSION×RX_COLD_BLOCK_CAP×U_oversub — v2-§3 min()-formu geçerli)** · §4.3-iade-sırası ·
> D-5-dedup-session-scoping · §6.2-notice-API'si · **§7.1-F-C-bound-cümlesi (session-scoped-dedup-tabanlı
> — v2-§5 fixed-window'la yeniden türetilir)** · **§7.2-hücre-cap-değerleri (cap=256/cap=2 — reddedilen
> sayı; v2-§9 [CAL]-placeholder'ları geçerli)** · **§8.1-[CAL]-tablosu (v2-§9 yerine geçer)** ·
> T_ECHO_MIN-sabiti · **§9-iskeletindeki "ColdRxHost" adı ve "dedup-session-scoping" kalemleri
> (v2-§1/§5 ile okunur)**. v2'nin referansla-yürürlükte tuttuğu bölümler v1-metninden okunur;
> §4.1/§3.4'ün "blok+byte+shard" üç-boyut sözlüğü v2-§3 dört-eksen eşlemesiyle okunur.
> Karar-izlenebilirliği için silinmez.

**Statü:** TASARIM-TASLAĞI — Abkarada incelemesi + bağlayıcı icra-talimatı bekliyor (DUR-B).
**Yetki tabanı:** Abkarada A1-mühür kararı (2026-07-26): *"A2 TASARIM VE BAĞLAYICI TALİMAT HAZIRLIĞI: GO. A2
PRODUCTION IMPLEMENTATION: HENÜZ BAŞLAMA."* Bu belge o kararın 9-maddelik zorunlu sırasının 1–8. maddelerini
işler; madde-9 (bağlayıcı icra-talimatı) için §9'da yalnız İSKELET önerilir.
**Bu belgenin ürettiği kod değişikliği: SIFIR.** Üretim koduna, bağlayıcı talimat + pre-registration freeze
tamamlanmadan dokunulmaz.

> **BAĞLAYICI ÇERÇEVE (devralınan):** DR-CS-1 bir cold-start LIVENESS/bootstrap DR'ıdır; ARQ tüm kollarda
> aktiftir; "ARQ FEC'i yendi / FEC ARQ'dan iyi" türü ifadeler YASAKTIR. A1 SEALED — yeniden açılmaz.
> A1-hükmü: `DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL — SCOPE-LIMITED CHARACTERIZATION COMPLETE`.
> 1200B-yasak-iddialar (decisive/liveness-solved/recovery-impossible/FEC-ARQ-hükmü) A2'de de geçerlidir.

---

## 0. KAPSAM, YÖNTEM VE ENVANTER TABANI

Bu tasarım, 5-ajanlı paralel read-only envanter + completeness-critic turu (workflow `wf_76dac9e2`) ve şu
normatif kaynakların doğrudan okunmasıyla yazıldı; sonrasında **6-mercekli adversarial-verify turundan**
(workflow `wf_4bb590ef`: normatif/kod-sadakat/invariant/kapsam/süreç/tamlık; 3 REFUTED + 12 WEAKENED +
27 HOLDS) geçirildi ve TÜM refuted/weakened bulgular bu v1'e işlendi (shard-boyutu rezervasyon-deliği §3.2;
g5-pin-amendman sınıflandırması §5.2-c/§8.2/§9; noise-wire-§8 normatif-gerilim §3.4; P-4 probe-DoS aynası
§2.2; P-6 PreParamHold-TTL §2.2; feature-flag propagasyonu §9-F0; ZM-2 sabit-kol §8.1; DUR-B-4a/4b bölünmesi):

- `specs/fec-core-tdd-v3.0.md` §9 (RETENTION + ZEROIZE P0-9 + memory-cap-test), §13 (DoS-SINIRI cap-ailesi) — **iki-seviye cap'in normatif üst-kaynağı**
- `specs/kama-noise-wire-tdd-v2.0.md` §2.56 (amendman-sınırı: yeni codepoint/subtype/alan = DUR-B), §2.57 (control_id dedup + CS_PROBE/CS_ECHO stateless-zorunluluğu + tombstone/grace), §3 (dört-kimlik), §5 (dh_epoch/renewal), §8 (PARAM_SET/B2 + max_preparam_shards), §10 (DoS kripto-tarafı)
- `specs/adaptive-transport-engine-spec.md` §16.2 (ZM-1..ZM-4 saat-sınıfı kuralları)
- `reports/decision-requests/DR-CS-1-minimal-bootstrap.md` (EK4-A1 patience max-formülü; INV-CS-RX-RESOURCE; [CAL]-tablo + hash-freeze emsali)
- `reports/decision-requests/DR-CS-1-F0-findings.md` TESPİT-(ii)/(viii) (bounded/unbounded envanteri; profil-türevi patience tablosu; DUR-A karar-2: S-B floor-profillerde ZORUNLU)
- `docs/roadmap/kama-transport-roadmap-status-board.md` A2-bloğu (iki-seviye cap şartları; kabul kriteri; çıkış hükmü)
- W-3-65 B1-sonucu (`reports/d1-exit/DR-CS-1-A1-W365-B1-result.md`) + F4-closure DI-10 bölümü

### 0.1 BLOCKER — Abkarada'dan temin edilmeden kapatılamayan girdiler

| # | Eksik girdi | Neden blocker | Tasarımın şimdiki tavrı |
|---|---|---|---|
| G-1 | **"durum-analizi §12 on-bir maddesi"nin tam metni** — repoda YOK (tek referans roadmap:74; minimal-bootstrap "EK1 (11 madde) referansla-yürürlükte" der ama madde metinleri repoda değil) | A2 kapsamının ilk enumerasyon kalemi; metinsiz kapsam-tamlığı iddia edilemez | Tasarım INV-CS-RX-RESOURCE + roadmap-A2-bloğu + DI-10 üzerinden yazıldı; §12-maddeleri geldiğinde **fark-taraması** yapılıp v2'ye işlenecek |
| G-2 | **N_SESSION_TARGET** (gerçek eşzamanlı-session hedefi) — hiçbir spec'te sayısal değil | Process-wide cap'in [CAL] donması roadmap:85-86 gereği "gerçek eşzamanlı-session bütçesiyle" yapılmak zorunda | §3.3'te formül parametrik bırakıldı; [CAL]-freeze N_SESSION_TARGET gelmeden YAPILMAZ |
| G-3 | **tur-s-sensing sıralama/bağımlılık kararı** — `tur-s-sensing-v1.md` yalnız `tur-s-sensing` branch'inde (ONAY BEKLİYOR); "kısa-kopmalar receiver-epizot-dedektörü alanında" hükmü A2 patience-alanına komşu | Epizot-alanı çift-sahiplik riski | §2.5'te sınır çizildi: A2 receiver-patience **kaynak-yaşam-döngüsü** işidir, kanal-durumu-sensing DEĞİL; epizot-dedektörüne dokunulmaz |

---

## 1. MADDE-1 — ENVANTER + CALL-FLOW HARİTASI

### 1.1 Kritik yapısal gerçek: ürün kompozisyon-kökü YOK

Madde-1 "mevcut product receiver yolunun haritası" ister; envanterin bulgusu: **haritalanacak bütünleşik bir
ürün yolu henüz mevcut değil.** Kanıt zinciri:

- `kama-integration/src/lib.rs` = 9 satır (test-harness beyanı); `hub-server`/`client-core` = 3'er satır stub.
- Harness `run_rx` (udp_runner.rs:175-631) transport `Receiver`'ı HİÇ instantiate etmez — kendi acked-set/50ms-SACK muhasebesini yürütür.
- `EngineError::ParamNotConfirmed` ÖLÜ-VARYANT (tanımlı, hiç raise edilmiyor); `ParamState.is_confirmed → {PreParamHold | admit_and_decode}` yönlendirme kablosu üretimde yok (yalnız test-konvansiyon).
- `InboundControlDedup.admit()` üretim kodundan hiç çağrılmıyor (yalnız testler).
- Üretimde `RecoveredByForwardRecoveryControl` taşıyıcı tipi yok (yalnız harness-ledger string'i, udp_runner.rs:552).

**Sonuç:** Madde-1 çıktısı iki katmandır: (a) mevcut PARÇALARIN envanteri (aşağıda), (b) **HEDEF call-flow**
(§1.3) — mevcut değil, inşa edilecek mimari olarak etiketlenir.

### 1.2 Mevcut parçaların envanteri (bounded/unbounded — SR-AUDIT-001 tabanı)

**BOUNDED (mevcut, emsal-değerli):**

| Yapı | Sınır | Politika | Emsal-notu |
|---|---|---|---|
| `PreParamHold` (kama-engine) | MAX_PREPARAM_SHARDS=256 | **sessiz** en-eski-drop + `dropped` audit-sayacı | Cap-değeri emsal; eviction-politikası A2-şartının TERSİ (bkz. §3.4) |
| `RecoveredMap` (kama-engine) | RECOVERED_MAP_CAP=4096 | sessiz en-eski-evict + `evicted` sayacı | resolve() `BTreeMap::range` DoS-güvenli |
| `InboundControlDedup.close_tombstones` | tombstone_cap | cap-doluda **insert-reddi** (eviction değil) | |
| `PairState.recent` (restart.rs) | recent_cap | min-eviction O(n) | |
| `ReplayWindow.bits` | sabit 2KB/yön (window=2^14) | — | session-başına ~4KB sabit |
| wire-parse cap'leri (noise-wire control.rs) | RECOVERED_MAX_ESI_RANGES=16, ACK_MAX_RANGES=32 | **parse-time TYPED-reject** (`WireError::ControlCountExceedsCap`) | **A2 "typed diagnostic" şartının repo-içi gerçek emsali** |
| `cold_admit_new_block` (harness, DI-10) | cap>0 → open_incomplete≥cap'te YENİ-blok reddi | slot-öncesi karar; eviction YOK; `cs_rxcap_diag` typed-diag | f4_di10 test-pinli; A2 ürün-varyantının çekirdeği |
| `InitFlight` (noise-wire) | çift-sınır: sayaç+toplam-süre; saturating backoff | typed-outcome {Send, Exhausted, Confirmed} | **bounded-retry + typed-outcome şablonu** |
| probe→echo yolu (harness) | 43B sabit stack-buffer; sıfır map-insert; T_ECHO_MIN=500ms | O(1); g10c/g10d pinli | Ürün-alıcının devralacağı model |

**UNBOUNDED (A2 cap/yaşam-döngüsü adayları):**

| Yapı | Büyüme ekseni | Katman | A2-kapsam kararı (bkz. §8) |
|---|---|---|---|
| `InboundControlDedup.seen: HashSet<u32>` | control_id sayısı (g10c: 1024-probe → seen=1024) | noise-wire | **A2-İÇİ** (cold-yol dedup'u; TTL/renewal-sınırı §4.3) |
| `SessionManager.by_handle` | session sayısı (cap yok; remove() çağıranı tanımsız) | noise-wire | **A2-İÇİ** (iade-zincirinin kökü) |
| `HandleRouter.classes` | handle sayısı (unregister API'si YOK) | noise-wire | **A2-İÇİ** (iade-zinciri) |
| `PersistentReplayGuard.per_pair` | cihaz-çifti (`or_default` insert — doğrulanmamış-kaynak şişme riski) | noise-wire | **A2-İÇİ** (cap + çağrı-sırası sözleşmesi) |
| `InitReceiver.seen` | INIT-identity | noise-wire | **A2-İÇİ** (cap) |
| `receiver.rs received: BTreeSet<u64>` + build_ack tam-set kopyası | distinct seq (oturum-ömrü) | adaptive-transport | **A2-DIŞI → ayrı-DR** (warm-path; cold-state değil; §8 kapsam-sınırı) |
| harness `run_rx` blok-map'leri (src_have, block_complete, …) | blok sayısı (tamamlanan bloklar kalıcı) | harness | Harness-kabul; ürün-alıcıda karşılığı retention-sınırlarıyla doğar (§3.2) |

**Mevcut zaman-davranışı:** transport `Receiver`'da give-up/patience KAVRAMI YOK (yalnız SACK-ritmi:
FLOOR≈8.39s, K_TAIL=2, QUOTA_R=4, T_SPACING≈2.1s); tek terk-mantığı harness `idle_abandon=105s(ON)/8s(OFF)`;
sender-tarafı `CS_GIVEUP_NS=60s` → `CsTerminal::TimedOut`. `ack_epoch` alanı pasif (hiç artmıyor; dh_epoch/renewal
§5-noise-wire'a bağlanacak — A2-dışı, kayıt düşüldü).

### 1.3 HEDEF call-flow (inşa edilecek; mevcut-değil)

```text
UDP datagram
  → HandleRouter.classify(h_ext)                      [noise-wire; fixed-cost, alloc-öncesi]
  → SessionManager.open()  → AEAD-verify              [forged burada ölür; §10-DoS fixed-cost]
  → OpenedTransportShard (sealed-tip; tek-üretici open())
  → INNER demux (inner_kind):
      0x19 CTRL_CS_PROBE  → STATELESS-ECHO yolu       [admit()'e GİRMEZ — §5; O(1), rate-limit]
      0x10/0x11/0x12/…    → InboundControlDedup.admit(control_id) → control-işleme
      0x01/0x02 FEC shard → COLD-GATE:
          ParamState.is_confirmed?
            ├─ HAYIR → [CAP-REZERVASYON §3] → PreParamHold(bounded)   ← RX-COLD-STATE
            └─ EVET  → admit_and_decode (TYPE-GATE + profil-gate + INV-FEC-1)
  → Receiver.on_datagram (seq-muhasebe, SACK-ritmi, epizot-dedektörü)  [mevcut; DEĞİŞMEZ]
  → app-delivery + [RECOVERED-emisyon (0x12) + notification-contract §6]

PATIENCE-HOST (yeni; kompozisyon-kökünde):  per-block deadline + session-hold (S-B) — §2
İADE-ZİNCİRİ (yeni): session close/timeout → tek-remove → tüm katman-state'leri + global-bütçe iadesi — §4
```

**D-1 (DUR-B karar noktası) — kompozisyon-kökü nerede doğar:**
- **Öneri (birincil):** `kama-engine`'e yeni modül `rx_host.rs` — ColdRxHost: {ParamState, PreParamHold,
  RecoveredMap, iki-seviye-cap-defteri, patience-saat-girdisi} sahipliği + `kama-integration`'da
  noise-wire↔engine↔transport kompozisyon-testi. Gerekçe: cold-state'in mevcut üç parçası zaten engine'de;
  INV-ENGINE-SOC korunur (AEAD/anahtar noise-wire'da kalır; transport type-agnostik kalır).
- Alternatif: yeni crate (`kama-rx-host`). Maliyet: workspace-genişleme; fayda: bağımlılık-izolasyonu.
- **Karar Abkarada'nın** (ADR-etkisi olabilir).

---

## 2. MADDE-2 — PATIENCE FORMÜLÜ + PROFİL TABLOSU

### 2.1 Normatif formül (EK4-A1 — AYNEN devralınır)

```text
RX_COLD_ABANDON = max( T_COLD_GIVEUP + RTT_MAX_SUPPORTED + RECOVERY_ALLOWANCE,
                       T_NEXT_RECOVERY_ELIGIBLE + MAX_RECOVERY_SERIALIZATION
                       + FORWARD_DELAY_BOUND + RECEIVER_PROCESSING_MARGIN )
```

Dondurulmuş profil-türevi tablo (F0-TESPİT-(viii); f3_m6 test-kontratı `dr_cs1_acceptance.rs:894-914`):
65/300B tüm-profiller → 75.0s yeterli; **1200B hiçbir profilde 75s'e sığmaz** (102.0–104.4s) → S-A tavan
105s (harness'ta derili) **veya S-B**. F0-DUR-A-karar-2: **S-B (session-hold + hard-cap) floor-profillerde
ZORUNLU** — ürün-alıcı tasarımı bunu birincil kabul eder.

### 2.2 D-2 — Anchor/reset semantiği (mevcut belgelerde TANIMSIZ; bu tasarım tanımlar)

EK4-A1 formülü mutlak-bound tanımlar ama SIFIR-NOKTASI hiçbir belgede yok; harness ise idle-gap ölçer
(`last_rx.elapsed() > idle_abandon`) ve **probe-varışı last_rx'i besliyor** (udp_runner.rs:282) — probe-only
akış patience'ı süresiz canlı tutar (DoS-yüzeyi). Ürün-alıcı için öneri:

```text
P-1  PATIENCE = PER-BLOCK DEADLINE (idle-gap DEĞİL). Anchor: blok-ilk-shard-varışı (block_first).
     deadline(block) = block_first + RX_COLD_ABANDON(profil, max_unit_size).
P-2  Patience'ı BESLEYEN olaylar: yalnız o bloğa ait AUTHENTICATED DATA/shard varışı (Data/Retransmit/
     Parity/Duplicate). PROBE (0x19) ve genel control-trafiği patience'ı BESLEMEZ.
     (Probe-only-akış bir bloğu süresiz canlı TUTAMAZ.)
P-3  Deadline dolunca: blok-state EVICT + zeroize (§4.2) + TYPED diagnostic (sessiz değil) +
     global-bütçe iadesi. Bu bir TEŞHİS+KAYNAK-hükmüdür; kanal-durumu hükmü DEĞİLDİR (§2.5).
P-4  SESSION-HOLD (S-B): session-seviyesi ayrı sabır — session'da HİÇBİR canlı blok yoksa VE HİÇBİR
     authenticated **DATA/shard**-varışı yoksa SESSION_IDLE_HOLD [CAL] sonra iade-zinciri (§4).
     **P-2 aynası: probe (0x19 — üründe AEAD-sealed olsa da) ve genel control-trafiği session-hold'u da
     BESLEMEZ** — aksi halde probe-only akış session'ı süresiz canlı tutar ve harness'taki DoS-deseni
     (last_rx-besleme) session-seviyesinde geri döner (skeptik-bulgusu). Session-count üst-sınırı
     SESSION_COUNT_CAP [CAL] (N_SESSION_TARGET-türevi) §8.1-tablosuna eklendi.
     Floor-profillerde (sub-kbps) S-B ZORUNLU: per-block deadline profil-türevi uzar; hard-cap her durumda kalır.
P-5  Kısmi-teslim ANINDA (blok decode edildi) blok-state serbest kalır (retention §3.2'ye devrolur).
P-6  PREPARAM-HOLD-TTL: PARAM_SET kalıcı-kayıpsa (§8-noise-wire: "deadline→retry-durur, emission-SÜRER" —
     senaryo gerçek) PreParamHold süresiz yaşayamaz. Pre-PARAM shard'lar header.block_id üzerinden AYNI
     per-block-deadline'a (P-1, block_first-anchor) bağlanır; deadline dolan pre-PARAM blok EVICT+zeroize+
     typed-diag (`ParamNeverArrived` sınıfı). Ayrı bir mutlak PREPARAM_HOLD_TTL sabiti GEREKMEZ
     (per-block-deadline kapsar); [CAL]-tabloda not.
```

### 2.3 ZM-uyumu (engine-spec §16.2 — İHLAL EDİLEMEZ)

- **ZM-2:** RX_COLD_ABANDON zaten profil-türevi (serialization + T_elig türevli) → uyumlu. SESSION_IDLE_HOLD
  ve T_ECHO_MIN üretim-değerleri wall-clock sabit girecekse **4-decade (0.25k/1k/100k/1M) doğrulaması ZORUNLU**;
  aksi halde profil-türevi türetilir.
- **ZM-3:** Patience-timer + plateau-test-timer'ları **§16.2 saat-sınıfı tablosuna kayıt ZORUNLU** (sınıfsız
  eşik = CI-red). Saat-sınıfı: receiver-monotonic (CLOCK_MONOTONIC), kompozisyon-kökünde örneklenir;
  kama-engine saat TUTMAZ (envanter: engine'de sıfır zaman-kavramı — SoC korunur; saat DEĞER-olarak enjekte edilir).
- **ZM-4:** deadline-karşılaştırmasının iki yanı aynı saat (block_first ve now aynı monotonic domain).

### 2.4 Profil tablosu (freeze-adayı; [CAL] — §8 tablosunda)

| Profil (usable_rate) | 65B | 300B | 1200B | S-B zorunlu? |
|---|---|---|---|---|
| rtt100..rtt5000, ≥31 B/s floor | 75.0s | 75.0s | 102.0–104.4s → **105s tavan** | floor-profillerde EVET |

(Değerler F0-(viii) donmuş tablosundan; ürün-freeze'i W-koşularından ÖNCE hash+timestamp ile — §8.)

### 2.5 Sınır beyanı — patience ≠ sensing

Receiver-patience **kaynak-yaşam-döngüsü** hükmüdür (bu bloğa/oturuma ayrılan belleği ne zaman bırakırım);
**kanal-durumu hükmü değildir** (kopma/kalıcı-kesinti tespiti tur-s-sensing alanı; kısa-kopmalar receiver-
epizot-dedektörü — `open_episode`, return-supply-r3 — alanında). A2, epizot-dedektörüne ve frozen-W'ye
DOKUNMAZ; patience-eviction bir kanal-teşhisi ÜRETMEZ. (G-3 sıralama kararı Abkarada'da.)

---

## 3. MADDE-3 — İKİ-SEVİYE RESOURCE CAP

### 3.1 Normatif hizalama (critic-BLOCKER-1 çözümü)

İki-seviye cap AD-HOC bir A2 icadı değil; `fec-core-tdd-v3.0 §13` [NORMATİF] cap-ailesinin cold-state
instantiation'ıdır:

| fec-core §13 ailesi | A2 cold-state karşılığı |
|---|---|
| max_open_blocks | RX_COLD_BLOCK_CAP (per-session) |
| max_block_bytes × per-peer-quota | RX_COLD_BYTE_CAP (per-session) |
| global-memory-cap | GLOBAL_COLD_BLOCK_CAP / GLOBAL_COLD_BYTE_CAP (process-wide) |
| idle_timeout / retention_ttl | RX_COLD_ABANDON (§2) / retention-sınırları (§3.2) |
| duplicate-ESI-rejection · strict-length-validation · profile-allowlist | mevcut (admit_and_decode + wire-parse) |

"Auth'lu-peer-de-compromised-olabilir" (§13) → cap'ler AEAD-SONRASI da uygulanır; AEAD-authenticity
resource-trust DEĞİL (noise-wire §10 ile tutarlı).

### 3.2 Seviye-1: per-session / per-flow

```text
RX_COLD_BLOCK_CAP  = 256                    [CAL] (kama-engine MAX_PREPARAM_SHARDS precedent)
RX_COLD_BYTE_CAP   = RX_COLD_BLOCK_CAP × K_shape × E     — PER-PROFİL FORMÜL (harness-örneği
                     256×8×1200 ≈ 2.46MB yalnız o profil-değerinin instantiation'ıdır; [CAL]-freeze
                     per-profil yapılır, tek mutlak-sayı olarak DEĞİL)
RX_COLD_SHARD_CAP  = MAX_PREPARAM_SHARDS (=256) — ÜÇÜNCÜ boyut (skeptik-REFUTED-düzeltmesi): blok+byte
                     rezervasyonu TEK BAŞINA PreParamHold'u korumaz — tek-blok >256-shard seli blok=1
                     ve byte≪cap ile rezervasyonu GEÇER, hold 257. shard'da sessiz-drop yapardı. Defter
                     session-başına tutulan-shard-sayısını da rezerve eder; aşım → typed
                     CapExceeded{kind: Shard}. Pre-PARAM per-blok (block_id,esi)-dedup da bu katmanda
                     yapılır (duplicate-ESI-rejection'ın cold-yol öncüsü; post-PARAM'ı admit_and_decode
                     zaten yapar).
Kapsanan state: PreParamHold-shard'ları + açık-incomplete-blok slotları + blok-metadata.
Tamamlanan-blok dedup-kayıtları: retention-sınırlarına bağlanır (fec-core §9: max_retained_blocks ·
retention_ttl · per-peer-fair-share) — harness'taki "tamamlanan-blok kalıcı-büyüme" ürününe TAŞINMAZ.
admit_and_decode girdi-Vec bound'u fonksiyon-içi ENFORCE EDİLMEZ (sözleşmesel kalır) — üst-sınır bu
rezervasyon-defterinden gelir (drain ≤ RX_COLD_SHARD_CAP); bu bağ artık AÇIKÇA kurulmuştur.
```

### 3.3 Seviye-2: process-wide

```text
GLOBAL_COLD_BLOCK_CAP = N_SESSION_TARGET × RX_COLD_BLOCK_CAP × U_oversub   [CAL; U_oversub ≤ 1
                        oversubscription katsayısı — her session aynı anda cap'ini doldurmaz]
GLOBAL_COLD_BYTE_CAP  = aynı yapı, byte cinsinden.
```
**N_SESSION_TARGET tanımsız (G-2-BLOCKER)** → bu iki [CAL] değeri Abkarada girdisi gelmeden DONMAZ.
Global-cap doluyken yeni-session cold-state açamaz → typed-reject (fail-closed; sessiz-degrade YOK).

### 3.4 Cap-aşımı davranışı: typed diagnostic; sessiz eviction YOK

- Yeni RX-COLD yapısında: cap-aşımı → `CapExceeded { scope: Session|Global, kind: Block|Byte|Shard, cap, requested }`
  sınıfı TYPED hata/diag (emsal: `WireError::ControlCountExceedsCap`) + sayaç. Mevcut bloklar EVICT EDİLMEZ
  (f4_di10 semantiği: red = yeni-slot-yok).
- **D-3 (karar; NORMATİF-ÇELİŞKİ ADLANDIRILDI):** `PreParamHold`/`RecoveredMap`'in kendi API'si bu dilimde
  **RETROFIT EDİLMEZ**; sessiz-drop'a erişim, rezervasyon-defterinin ÜÇ boyutu (blok+byte+**shard**, §3.2)
  hold'un ÖNÜNDE typed-reject üretmesiyle kapatılır (shard-boyutu olmadan bu iddia ÇÜRÜKTÜ — skeptik-REFUTED
  düzeltmesi işlendi). **Ancak dikkat:** noise-wire-tdd §8 [NORMATİF] pre-param hold aşım-politikasını açıkça
  *"max_preparam_shards, küçük; aşımda-en-eski-drop"* yazar; A2-şartı (roadmap:81 + INV-CS-RX-RESOURCE
  "sessiz eviction YOK") ile §8-metni arasında GERÇEK bir normatif gerilim vardır: typed-reject-yeni,
  aşım altında tutulan kümeyi değiştirir (en-eski-korunur ↔ en-yeni-drop). Bu tasarım typed-reject-yeni'yi
  önerir (INV-CS-RX-RESOURCE-hizası) ve **DUR-B-1 karar-listesine isimli-amendman kalemi** olarak koyar:
  §8'in aşım-politikası cümlesine dar açıklama-notu (davranış: rezervasyon-katmanı reddeder; hold'un kendi
  drop-dalı ölü-kod-pinli kalır). Abkarada tersini seçerse (§8-politikası korunur) tasarım v2'ye işlenir.
  Not (skeptik-tamlık): `RecoveredMap` aynı-anahtar re-register'da order-pozisyonu TAZELENMEZ (taze-emisyon
  orijinal-yaşta evict edilebilir) — bu davranış bilinçli-kabul; düzeltme ihtiyacı doğarsa retrofit-DR kapsamı.

---

## 4. MADDE-4 — BOUNDS-BEFORE-ALLOC + BÜTÇE-İADE YAŞAM DÖNGÜSÜ

### 4.1 Rezervasyon deseni (repoda emsali yok — yeni desen tanımı)

```text
admit-cold-shard(session, blok, shard, bytes):
  1. REZERVE  : session-defteri + global-defter ATOMİK kontrol-ve-ayır (üç boyut: blok+byte+shard;
                iki-seviye birden; biri reddederse HİÇBİRİ ayrılmaz) — KALICI-RETENTION'dan ÖNCE.
                (Kesinleştirme — skeptik-düzeltmesi: AEAD-open'ın transient-alloc'u rezervasyondan
                ÖNCE gerçekleşir ve tek-datagram-MTU ile üstten sınırlıdır; bounds-before-alloc'un
                datagram-parse katmanındaki mevcut garantisidir. Rezervasyonun koruduğu şey
                KALICI cold-state retention'ıdır.)
  2. COMMIT   : retention başarılı → rezervasyon kalıcılaşır
  3. RELEASE  : blok terminal-durumda (DELIVERED / deadline-EVICT / session-close) → defter iadesi
Tek-thread receiver-poll varsayımı altında "atomik" = tek-kritik-bölge (lock-free gereksinimi YOK;
mevcut motor tek-iş-parçacıklı event-batch modeli).
Typed-outcome: {Admitted, RejectedSessionCap, RejectedGlobalCap} (InitFlight typed-outcome şablonu).
```

### 4.2 Zeroize yükümlülüğü (fec-core §9 P0-9 — critic-BLOCKER-2 çözümü)

Hassas = source + repair symbol-buffer'ları + decoder-scratch + kopyalar. Politika: terminal-state-zeroize ·
**eviction/timeout-path-zeroize** (patience-EVICT §2.2-P-3 dahil) · panic/error-path-zeroize ·
reusable-bounded-pool · memory-cap-test. **PreParamHold bugün drop'ta zeroize ETMİYOR** — ColdRxHost iade
yolunda shard-byte'ları `Zeroizing`-eşdeğeriyle sarılır (engine-içi dar dokunuş; icra-talimatında ayrı madde).

### 4.3 İade-zinciri: tek yaşam-döngüsü sahibi

Bugün kopuk: `SessionManager.remove()` çağıranı tanımsız; `InboundControlDedup` session'a bağlı DEĞİL;
`HandleRouter` unregister'sız; `PersistentReplayGuard.per_pair` or_default ile büyür. Öneri:

```text
D-4: İADE-SAHİBİ = kompozisyon-kökü (ColdRxHost'un üstündeki session-yöneticisi katman).
close/timeout(h_ext) sırası:
  (1) SessionManager.remove(h_ext)      → protector/cipher zeroize-on-drop (mevcut)
  (2) tombstone/grace (§2.57)           → SESSION_CLOSE-retry-ACK için bounded-süre; süre [CAL],
                                          patience-parametresine bağlanır (grace ≤ SESSION_IDLE_HOLD)
  (3) ColdRxHost.release(session)       → PreParamHold-drain-discard + blok-slotları + REZERVASYON-İADE
                                          (global-defter geri-alır) + zeroize (§4.2)
  (4) HandleRouter.unregister(h_ext)    → YENİ API (bugün yok; icra-diliminde eklenir)
  (5) dedup-state iadesi                → InboundControlDedup session-scoped'a taşınır (D-5):
                                          session-başına instance → remove'la birlikte düşer.
                                          control_id "wrap-öncesi-session-renewal" (§2.57) ile tutarlı:
                                          renewal = yeni session = yeni dedup-instance (TTL-icadına gerek yok).
  (6) PersistentReplayGuard             → KALIR (restart-güvenliği kalıcı-state; per_pair'e cihaz-çifti
                                          cap'i [CAL] + "admit yalnız cert-verify-SONRASI" çağrı-sırası
                                          sözleşmesi eklenir; cap-doluda yeni-çift typed-reject fail-closed)
```

Not: `close_tombstones` cap-dolu politikasının seen-TTL etkileşimi (envanter açık-sorusu) D-5 ile çözülür:
seen session-scoped olunca CLOSE-retry-ACK'i tombstone'dan gelir; tombstone-cap [CAL] korunur.

---

## 5. MADDE-5 — PROBE/ECHO STATELESS + `InboundControlDedup.admit()` YASAĞI

### 5.1 Normatif zemin ve mevcut çelişki

Spec zaten kararı vermiş: §2.57 — *"Echo: alıcı-flow-state'inden idempotent-stateless; per-probe-uzun-ömürlü-
allocation-YOK; rate-limit T_ECHO_MIN/akış"* ve CTRL_CS_PROBE/ECHO → RETRY-EDİLMEZ; replay-koruması
"EXTENDED-outer-pn32-window + control_id-dedup OTOMATİK". Test-katmanında çelişki: `g5` probe'u admit()'e
fiilen sokup "otomatik-kapsam" der; `g10c` aynı dosyada bunun unbounded-büyüme anti-pattern olduğunu pinler.

### 5.2 D-5 çözümü (öneri)

```text
(a) NORMATİF yorum önerisi: g10c KAZANIR — ANCAK dürüst kayıt (skeptik-bulgusu): §2.57:308-310 probe/echo
    replay-korumasını İKİ katman sayar ("EXTENDED-outer-pn32-window + control_id-dedup OTOMATİK");
    admit()-bypass ikinci katmanı probe/echo için fiilen kaldırır → spec-CÜMLESİ ile önerilen davranış
    ayrışır. Yön roadmap:82-şartı + §2.57 stateless-echo zorunluluğu + g10c-kanıtıyla desteklidir; yine de
    bu ayrışma İSİMLİ dar açıklama-notu/amendman olarak DUR-B-1 paketine girer (sessizce yorumlanmaz).
    Güvenlik-tabanı yeterli kalır: outer-pn-window kripto-replay'ı zaten keser; echo idempotent-stateless
    olduğundan logical-dedup'un önlediği hiçbir side-effect yoktur.
(b) Uygulama şekli: CALLER-DISPATCH (demux 0x19/0x1A'yı admit()-öncesi stateless-yola ayırır) +
    admit()-içi debug_assert (CsProbe|CsEcho görürse test-yakalar). admit() İMZASI DEĞİŞMEZ; wire'a alan
    EKLENMEZ → §2.56 amendman-sınırına (yeni codepoint/subtype/alan = DUR-B) DOKUNULMAZ.
(c) g5 düzeltmesi — SINIFLANDIRMA DÜZELTİLDİ (skeptik-REFUTED): g5, `dr_cs1_wire` 12'lik A1-PİN-SETİNİN
    ÜYESİDİR; "A1-pinlerine dokunmaz" denemez. Değişikliği DUR-B-4a'da **A1-pin-seti İSİMLİ-AMENDMANI**
    olarak onaylanır; §9 faz-kapısı koşulu buna göre yazılmıştır ("g5: DUR-B-4a-amendmanlı hali baz;
    kalan 11 dokunulmamış"). A1-hükmü etkilenmez (g5 hüküm-taşıyıcı değil; probe-admit satırları
    "otomatik-kapsam" yan-iddiasıydı).
(d) T_ECHO_MIN üretim-değeri [CAL] (harness 500ms; wire-testleri değeri pinlemez) + per-flow limit;
    engine-spec PROBE_RATE_LIMIT satırıyla (responder kaynak-başına echo/s) hizalanır.
(e) Echo-yolu allocation-yasağı ürün-kabulünde AYNEN pinlenir (g10c/g10d ürün-varyantı: sabit-buffer,
    sıfır-map-insert, O(1) working-set).
```

---

## 6. MADDE-6 — POST-TIMEOUT RECOVERED NOTIFICATION CONTRACT

### 6.1 Boşluğun kesin tanımı (B1-amendman sonucu)

65B-sınıfı rejimde heal §11 forward-recovery ile ACK-BAĞIMSIZ tamamlanır (W-3-65 B1: 3/3
`RecoveredByForwardRecoveryControl`; PostTimeoutRecovery ATEŞLEMEZ — o yalnız DATA-ACK-clocked 300/1200B
rejimi). `TimedOut` terminali almış ürün-katmanı, bloğun sonradan kurtarıldığını ÖĞRENEMEZ. Üretimde taşıyıcı
tip SIFIR — sözleşme sıfırdan tanımlanacak.

### 6.2 D-6 sözleşme önerisi (SINIF-AYRIMLI)

```text
Taşıyıcı: SENDER-tarafı yeni pull-API  take_post_timeout_notice() -> Option<PostTimeoutNotice>
  PostTimeoutNotice { cause: DataAckClocked | RecoveredControl, block_id, time }
Tetik-1 (mevcut yol):  TimedOut-SONRASI DATA-ACK  → cause=DataAckClocked
                       (bugünkü CsTerminal::PostTimeoutRecovery'nin bilgisini AYNI olayda taşır)
Tetik-2 (YENİ yol)  :  TimedOut-SONRASI Event::RecoveredControl (0x12) o epoch'un cold-bloğunu
                       kapsıyorsa → cause=RecoveredControl   ← 65B-sınıfının öğrenme yolu
KORUNAN INVARIANT'LAR:
  - Epoch-başına-1-terminal DEĞİŞMEZ: notice TERMINAL DEĞİL, terminal-SONRASI bilgi-kanalı
    (CsTerminal enum'una varyant EKLENMEZ; ikinci-terminal üretilmez).
  - N8: notice BtlBw/delivered/credit-yoluna Bİ'T eklemez (RecoveredControl işleme SAF-LOGICAL kalır;
    fires-vs-noop bit-özdeşlik testi ürün-kabulüne eklenir).
  - INV-CS-5: tümü experimental-cold-start flag-arkası; flag-OFF byte-özdeş.
Receiver-tarafı emisyon: RECOVERED (0x12) üretim-kadansı SACK return-supply bütçesini AŞMAZ
  (kota/T_SPACING ile aynı defter; harness 50ms+take(100) modeli [CAL]-parametrize edilir);
  control_id ataması §2.57 (session+direction-scoped; RECOVERED retry-edilmez, en-güncel-durum taşır).
```

**Dikkat:** Tetik-2 `sender.rs` mekanizma-yoluna DAR bir dokunuştur (RecoveredControl-işleyicisinde
post-timeout-durum kontrolü + notice-defteri). Bu, A2'nin üretim-dokunuşlu ilk dilimlerinden biridir ve icra-
talimatında **ayrı-fazlı, N8-bit-özdeşlik + flag-OFF-özdeşlik kanıt-şartlı** yazılmalıdır.

---

## 7. MADDE-7 — MEMORY-PLATEAU + W-3-65 ÜRÜN-ALICI ACCEPTANCE MATRİSİ

### 7.1 Plateau metodolojisi (tanımsızdı; pre-registered tanım önerisi)

Normatif çengel: fec-core §9 "memory-cap-test". Üç flood-sınıfı (üç ayrı unbounded-yüzeye karşılık):

```text
F-A probe-flood        : N_flood=10^5 farklı-probe → echo-yolu working-set SABİT (g10c ürün-varyantı);
                         patience beslenmez (P-2 doğrulaması)
F-B cold-DATA-blok-flood: PARAM-yokken çok-bloklu authenticated-shard seli → rezervasyon-defteri cap'te
                         doyar; typed-reject sayılır; bellek üst-sınırı formül-bound'u AŞMAZ
F-C control-flood      : farklı-control_id seli → session-scoped dedup; session-cap × tombstone-cap bound'u
Ölçüm: birincil = allocator-stat/defter-sayaçları (deterministik; formül-türevi üst-sınıra karşı assert);
       ikincil = RSS-örneklemesi (plateau-eğrisi: t∈[T/2,T]'de eğim≈0).
Kabul: working_set ≤ BOUND(formül: cap'lerden hesaplanan) her t'de  ∧  plateau (artış-yok).
Süre/parametreler [CAL] — koşudan ÖNCE hash+timestamp freeze (§8).
```

### 7.2 W-3-65 ürün-alıcı rerun matrisi (B1-amendmanlı; roadmap-çelişki düzeltmesi şart)

Roadmap:61-63 hâlâ orijinal zinciri (`…→DATA-ACK→PostTimeoutRecovery`) listeler — B1 bunu 65B için geçersiz
kıldı. **A2 pre-registration'ı B1-amendmanlı sınıf-ayrımıyla donar** (aksi halde A1'deki pre-reg/gerçeklik
çelişkisi tekrarlanır); roadmap satırına düzeltme SUPERSEDED-by-B1 üst-not deseniyle, DUR-B-4b yetki-yolu
üzerinden (§8.2 — status-board güncelleme-yetkisi kısıtına dikkat).

| Hücre | Beklenen zincir (pre-reg) |
|---|---|
| 65B/w8, restore=61, ürün-alıcı, cap=256 | TimedOut@60 (tek) → restore@61 → §11-forward-recovery → `RecoveredByForwardRecoveryControl` → **PostTimeoutNotice{cause=RecoveredControl}** → PostTimeoutRecovery-YOK · app_dup=0 · busy-loop-YOK |
| 65B/w8, aynı, cap-stres varyantı (cap=2) | Yukarısı + cap-reddi typed-diag gözlenir; tamamlanma cap-içi bloklarla |
| 300B/w8 kontrol-hücresi | TimedOut@60 → restore@61 → PTO-retransmit → DATA-ACK → **PostTimeoutNotice{cause=DataAckClocked}** (PostTimeoutRecovery-sınıfı korunur) |
| Plateau F-A/F-B/F-C | §7.1 kabulleri |
| DI-10 ürün-varyantları | f4_di10/g10c/g10d ürün-alıcı üstünde (cap-admission, stateless-echo, idempotent-regen) |

3 tekrar/hücre; enstrüman-zinciri (faithful-acceptance → overlap-gate → binary-provenance → arg-paritesi)
A1-disiplini AYNEN. 1200B uzun-horizon (DECISIVE-vs-NEUTRAL çözümü) **A2-kapsamı DIŞI** (A1-devir notu;
ayrı karar).

---

## 8. MADDE-8 — [CAL] TABLOSU + DUR-B KAPILARI + HÜKÜM FORMATLARI (sonuç-görülmeden dondurulacaklar)

### 8.1 [CAL] tablosu (freeze: W-koşularından ÖNCE, hash+timestamp — minimal-bootstrap emsali)

| Sabit | Başlangıç-değeri | Kaynak | Donma-önkoşulu |
|---|---|---|---|
| RX_COLD_BLOCK_CAP | 256 | MAX_PREPARAM_SHARDS precedent | — |
| RX_COLD_BYTE_CAP | blok_cap × K_shape × E (per-profil) | fec-core §13 | profil-seti |
| RX_COLD_SHARD_CAP | 256 (=MAX_PREPARAM_SHARDS; üçüncü boyut) | §3.2 | — |
| GLOBAL_COLD_BLOCK/BYTE_CAP | N_SESSION_TARGET × per-session × U_oversub | §3.3 | **G-2 (Abkarada)** |
| SESSION_COUNT_CAP | N_SESSION_TARGET-türevi | §2.2-P-4 | **G-2 (Abkarada)** |
| RX_COLD_ABANDON | EK4-A1 formül; 65/300B→75s, 1200B→105s-tavan / S-B | F0-(viii) donmuş | profil-tablo-freeze + **sabit-kol (75.0s = T_COLD_GIVEUP+RTT_MAX+ALLOWANCE) için ZM-2 4-decade doğrulaması** (65/300B'de bağlayıcı-kol profil-değişmez → fiilen wall-clock-sabit; skeptik-bulgusu) |
| PREPARAM-HOLD-TTL | ayrı-sabit YOK — per-block-deadline kapsar (P-6) | §2.2-P-6 | — |
| PARAM_SET_RETRY_CADENCE/DEADLINE | önerilecek; kısıt: retry-horizon ≥ receiver-patience-penceresi (RX_COLD_ABANDON ile koordine — aksi halde receiver PARAM-beklerken sender retry'ı erken keser) | noise-wire §8 state-machine (değerler spec'te sayısal-tanımsız); sender-tarafı | ZM-2 |
| SESSION_IDLE_HOLD (S-B) | önerilecek; profil-türevi | §2.2-P-4 | ZM-2 4-decade |
| TOMBSTONE_GRACE | ≤ SESSION_IDLE_HOLD | §4.3-(2) | aynı |
| T_ECHO_MIN (ürün) | 500ms başlangıç | harness | ZM-2 4-decade |
| REPLAY_PAIR_CAP | önerilecek | §4.3-(6) | — |
| RECOVERED-emisyon bütçesi | SACK-return-supply-defterinden | §6.2 | kota-parametreleri |
| Plateau N_flood/süre/BOUND | §7.1 | — | formül-türetim |

### 8.2 DUR-B kapıları (icra-öncesi/icra-içi onay noktaları)

```text
DUR-B-1  Bu tasarımın bütünü + D-1..D-6 kararları + G-1/G-2/G-3 girdileri + İKİ isimli-amendman kalemi:
         (i) noise-wire §8 "aşımda-en-eski-drop" → typed-reject-yeni geçişi (D-3-gerilimi, §3.4),
         (ii) §2.57:308-310 probe/echo "control_id-dedup OTOMATİK" cümlesine dar açıklama-notu (§5.2-a)
         (icra-öncesi; ZORUNLU)
DUR-B-2  İcra-talimatındaki MUTLAK-SINIR yeniden-yazımı: hangi crate/dosya değişebilir
         (A1'in "production=SIFIR"ı A2'de geçersiz — yeni sınır açıkça yazılmalı)
DUR-B-3  sender.rs dar-dokunuşu (Tetik-2, §6.2) — ayrı-fazlı onay + N8/flag-OFF kanıt-planı
DUR-B-4a g5 test-düzeltmesi — A1-PİN-SETİ İSİMLİ-AMENDMANI (dr_cs1_wire-12'nin üyesi; §5.2-c)
DUR-B-4b roadmap:61-63 zincir-düzeltmesi — SUPERSEDED-by-B1 ÜST-NOT deseniyle (orijinal metin silinmez);
         DİKKAT: status-board güncelleme-yetkisi "yalnız DUR-SON hükümleriyle" (roadmap:3-4/213) —
         bu düzeltme ya A2-DUR-SON hükmüne bağlanır ya Abkarada açık-yetki verir (skeptik-bulgusu)
DUR-B-5  Pre-registration freeze (beklenen-zincirler + [CAL] + plateau-parametreleri; hash+timestamp)
         — W-koşularından ÖNCE; koşu-sonrası değişiklik YASAK (A1-§5-disiplini)
İcra-içi: pre-reg/gerçeklik çelişkisi → DUR-B (A1-emsali); mühürlü-bölge dokunuşu → isimli-amendman.
```

### 8.3 Hüküm formatları (önceden sabit; üçüncü/bulanık sonuç YASAK)

```text
TAM   : END-TO-END PRODUCT COLD-START FIX VALIDATED            (roadmap:88 sabit-string)
KISMİ : PRODUCT COLD-START PARTIAL — [EXACT-eksik-hücre-listesi]
RED   : PRODUCT COLD-START NOT VALIDATED — [EXACT-fail-nedeni]
Default-flip HER DURUMDA AYRI (coordinated rollout + min-peer-version + federation-policy + §4.4-benchmark).
```

---

## 9. MADDE-9 — İCRA-TALİMATI İSKELETİ (öneri; bağlayıcı talimatı Abkarada yazar)

```text
A2-F0  Kompozisyon-kökü + tip-iskeleti  : ColdRxHost (D-1-kararlı yerde) + typed-error stub'lar +
       feature-flag (default-OFF; flag-OFF byte-özdeş pini İLK-commit'te) + ZM-3 saat-sınıfı kaydı
       + FLAG-PROPAGASYONU (skeptik-MAJOR-düzeltmesi): `experimental-cold-start` bugün YALNIZ
       adaptive-transport'ta tanımlı — kama-engine + kama-noise-wire Cargo.toml'larına feature eklenir
       (workspace-propagation); davranış-değiştiren dokunuşlar (PersistentReplayGuard cap/typed-reject,
       dedup-session-scoping) cfg-gate edilir; SALT-ADDITIVE API'ler (HandleRouter.unregister — flag-OFF'ta
       çağrılmaz) ayrıca sınıflandırılır; her iki crate için flag-OFF-özdeşlik kanıt-yöntemi (test-pin
       + derlenmiş-özdeşlik nerede mümkünse) icra-talimatında yazılır (F4-emsali: tüm-eklemeler cfg-gated)
A2-F1  İki-seviye cap + rezervasyon + iade-zinciri (§3+§4) + zeroize + HandleRouter.unregister +
       dedup-session-scoping — birim + adversarial cap-testleri
A2-F2  Patience (P-1..P-5) + S-B session-hold — f3_m6-formül-kontratı ürün-varyantı
A2-F3  Notification-contract: receiver-emisyon-bütçesi + sender-Tetik-2 (DUR-B-3'ten sonra) +
       N8-bit-özdeşlik + tek-terminal pinleri
A2-F4  Plateau (F-A/B/C) + W-3-65 ürün-rerun matrisi (§7.2) — DUR-B-5-freeze'den SONRA koşulur
DUR-SON: kabul-matrisi + hüküm (§8.3 formatlarından biri) + status-board güncellemesi
Her faz: fmt/clippy(-D)/test yeşil + flag-OFF-özdeşlik + ilgili A1-pinleri: dr_cs1_acceptance 22 +
cold_start_liveness 4 DOKUNULMAMIŞ; dr_cs1_wire 12 → **g5 DUR-B-4a-amendmanlı hali baz alınır, kalan 11
dokunulmamış** (g5 pin-setinin üyesidir; "dokunulmamış-12" koşulu DUR-B-4a sonrası tanımsal olarak
karşılanamazdı — skeptik-REFUTED düzeltmesi).
```

## 10. AÇIK SORULAR (konsolide; envanter+critic+skeptik'ten devralınan, tasarımın cevaplamadıkları)

1. G-1/G-2/G-3 (§0.1 — Abkarada girdisi).
2. D-1 host-crate kararı (§1.3).
3. PreParamHold/RecoveredMap sessiz-eviction retrofit'i — bu dilimde YAPILMAZ önerisi (§3.4-D-3) +
   noise-wire-§8 isimli-amendman yönü (typed-reject-yeni vs §8-en-eski-drop-korunur) onaylanıyor mu?
4. Warm-path unbounded'lar (`received: BTreeSet`, build_ack-kopyası) A2-DIŞI/ayrı-DR sınırı (§1.2) onaylanıyor mu?
5. RECOVERED-emisyonunun SACK-bütçesiyle defter-paylaşım oranı (§6.2) — [CAL] başlangıcı ne olsun?
6. 1200B uzun-horizon follow-up'ının zamanlaması (A2-dışı bırakıldı, §7.2).
7. PARAM_SET_RETRY kadans/deadline değerleri (spec'te sayısal-tanımsız; §8.1-satırı; sender-tarafı dokunuş
   DUR-B-3-ailesine girer mi yoksa ayrı dilim mi?).
8. `ack_epoch` pasif-alanı + dh_epoch/renewal tetik-katmanı — A2-DIŞI bırakıldı (§1.2 kaydı); Sprint-4/§5
   sahipliği onaylanıyor mu?
9. DUR-B-4b yetki-yolu: roadmap-düzeltmesi A2-DUR-SON'a mı bağlanır, açık-yetkiyle mi yapılır (§8.2)?
