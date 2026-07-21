# Sprint-5c KAPANIŞ — send-record + FEC-emisyon + N8 + RETRANSMIT (cerrahi + saf-doğrulama)

> **HÜKÜM: Sprint-5c KAPANIR.** İki çıkış-şartı da yeşil: (1) N8-yeniden-doğrulama emisyon-AÇIKKEN, (2) RETRANSMIT-
> invariant. Üç cerrahi-DR (S5-1/2/3) mühürlü-hot-path'i bozmadan FEC-emisyon-yolunu açtı; iki saf-doğrulama-adımı
> invariant'ların açıkken-de-tutulduğunu ölçtü. Dal: `d1-sprint5c` (main-üstünde; `pre-surgical-baseline` tag = dönüş-noktası).

## 1. CERRAHİ-DR ÖZETLERİ (her biri ayrı-commit, {A+B+C}-yeşil)

| DR | commit | Ne yaptı | Mühürlü-komşuluk |
|---|---|---|---|
| **DR-S5-1** | `9057c89` | `TransmitKind::FecParity` KOŞULLU-emit (`fec_enabled`-guard, default-OFF → hot-path byte-değişmez) + `TransmitDecision.send_record: Option<SendRecordHint>` + `plan_fec_parity` (drain_new_data-SONRASI = DATA-ÖNCE korunur; mevcut `pacer.try_send`) | DATA-ÖNCE sıralaması + TokenBucket-internals dokunulmadı |
| **DR-S5-2** | `a960558` | `RxKind::FecShard` genişliği → schema §2.6 (`block_id:u32/esi:u16/k_actual:u8/is_repair`; r-alanı-kalktı) + admission-köprüsü (INV-FEC-1; forged-erişemez) + N8-receiver-check (BtlBw-credit-yolu-açmaz) | receiver BtlBw-kredi-yolu dokunulmadı |
| **DR-S5-3** | `132cef1` | `sent_logical`(seq→SrLogicalRef) + `confirmed_logical`(BOUNDED VecDeque cap=4096) + on_ack_received-loop **tek-satır** SALT-LOGICAL teyit (bw_sampler/btlbw/delivered'e DEĞMEZ) + `take_confirmed_logical` | bw.rs W_BTLBW/W_qd/delivered-EAI/credit_delivered 5-blok dokunulmadı; N8-spot BtlBw bit-özdeş |

**Non-regression çekirdeği (§D KİLİTLİ {A+B+C}):** A=exact-test-count, B=desen1_sim 5-aralık-pin (RNG-nondeterminizmine-rağmen-tutan-invariant), C=FEC-off yapısal (guard-OFF no-emit). desen1 exact-değer bit-comparison GEÇERSİZ (HashSet-nondeterminizm; `baseline-pre-5c.md` §D). **DR-S5-0 (HashSet→BTreeSet) REDDEDİLDİ** (sim-harness = pin-doğrulayıcısı; deterministik-yapmak kanıtı-bozar).

## 2. ÇIKIŞ-ŞARTI 1/2 — N8-YENİDEN-DOĞRULAMA (emisyon-AÇIK) · commit `a08c150`

Detay: [`N8-reverify-emisyon-acik.md`](N8-reverify-emisyon-acik.md). **HÜKÜM: D0-N8 uyumu emisyon-AÇIKKEN de KORUNUYOR.**

- **INV-RATE-1** (`adaptive-transport/tests/n8_reverify_emit.rs`): FEC-emisyon-AÇIK canlı-sender; loss=0 vs loss≈%14 (aynı
  kanal-BW) → **send_rate & btlbw BİT-ÖZDEŞ (240000)**; loss-arm loss_pkt=63(vs4)/acked=82(vs99) → kayıp+emisyon gerçek.
  Loss'un send_rate'e etkisi SIFIR. (N8-spot'un canlı-pipeline + emisyon-AÇIK + gerçek-kayıp versiyonu.)
- **recovered≠ACK** (`kama-integration/tests/pipeline.rs::n8_recovered_...`): source{1,3}-kayıp+repair → recovered (app-bytes
  tam); recovered WIRE-teslim DEĞİL (kredi-tabanı={0,2,4}); `RECOVERED.codepoint 0x12 ≠ ACK 0x10`; `control_produces_bandwidth_credit(RECOVERED)=false`.
- **SINIR-BULGUSU:** sender-RECOVERED→`confirmed_logical` wiring DR-S5-3 §5'te ERTELENDİ → recovered≠BtlBw'nin **güvenlik-
  yarısı KANITLI** (yapısal+runtime), muhasebe-yarısı gelecekte-bağlanacak. → **K2** (aşağıda).

## 3. ÇIKIŞ-ŞARTI 2/2 — RETRANSMIT-INVARIANT (emisyon-AÇIK) · saf-doğrulama

**Konum:** `kama-integration/tests/pipeline.rs` — `retransmit_new_pn_new_nonce_new_ct_reuse_blocked` +
`retransmit_logical_single_delivery_via_admission_dedup`. **HÜKÜM: retransmit = YENİ full_pn + YENİ nonce96 + YENİ
ciphertext; eski-PN ASLA yeniden-seal-edilmez.**

| Assert | Kanıt |
|---|---|
| **(a)** yeni-monoton-PN + reuse yapısal-imkansız | Nonce-reuse guard `last_sealed_pn[ei]` (per-instance+per-epoch-parite): aynı/geri-PN 2.-seal → **TYPED-ERROR** `SealInput("non-monotonic wire_pn (nonce-reuse guard)")`, ciphertext ÜRETİLMEZ (panik değil). İlk-seal Ok guard'ı ARM eder (vacuity-korumalı). + `SEND_PN_CAP=1<<62` → `PnCapReached` (asla-wrap → nonce asla-tekrar). |
| **(b)** yeni nonce96 | `nonce96(100) ≠ nonce96(101)` (LE64-mapping; `nonce = [4B0 ‖ LE64(full_pn)]`). |
| **(c)** yeni ciphertext | `ct(orig) ≠ ct(retx)` (ct = `dg[10..len-16]`; OUTER_HDR/TAG dışlanır → header-farkı değil KEYSTREAM-farkı; aynı-plaintext-farklı-nonce → farklı-ct). |
| **(d)** logical-dedup → tek app-teslim | İki-kopya (aynı block_id/esi, farklı-PN) receiver'da İKİ-de open olur (farklı full_pn); app-teslim TEK: codec dup-esi'yi **FAIL-CLOSED** reddeder (`FecError::DuplicateEsi(0)` codec-sınırında doğrudan-assert) → distinct-by-esi admission CALLER-sözleşmesi; tek-kopya kontrastı → Ok. |
| **(e)** send-record retransmit-görünürlüğü | İki-emisyon AYRI `SealedPacketMeta` (`full_wire_pn` per-emisyon ayrımı). **BULGU:** `send_generation` transport-core'da retransmit'te ARTMAZ (sabit-0; wire-katmanı SADIK-plumb'lar); retransmit-diskriminatörü `full_wire_pn`, sender-içi sayaç `retx_count`. → **K2**. |

**TEK MEŞRU İSTİSNA (yeniden-yazılmadı, REFERANS):** SESSION_INIT byte-for-byte cached retransmit (InitFlight anahtar-
TUTMAZ → re-seal imkansız; aynı-PN/nonce/ct MEŞRU). Sprint-4d: `kama-noise-wire/tests/sprint4d_init_flight.rs::retransmit_is_byte_for_byte_cached`.

**Doğrulama-metodolojisi (ultracode):** İki paralel-workflow — (1) understand: 5-assert API-yüzeyi haritalandı (4/5 agent;
nonce-reuse-guard/nonce96/replay-window/dedup/send_gen keşfi naif-planı düzeltti); (2) adversarial: 5-çürütücü testi
ÇÜRÜTMEYE çalıştı → **0 blocker**. ÇÜRÜT-3'ün "note"'u (InnerParse-collapse variant'ı gizler) codec-sınırında doğrudan
`DuplicateEsi(0)`-assert'iyle kapatıldı.

## 4. NON-REGRESSION {A+B+C} — 5c-kapanış son-durumu

| Gate | pre-5c | 5c-kapanış | Δ |
|---|---|---|---|
| kama-noise-wire | 90 | **90** | 0 |
| kama-fec-codec | 25 | **25** | 0 |
| kama-transport-schema | 9 | **9** | 0 |
| kama-integration | 9 | **12** | +3 (N8-Test2 + 2 RETRANSMIT) |
| adaptive-transport | 118 | **126** | +8 (DR-S5-1/2/3 + N8-Test1; mevcut-118 AYNEN) |
| B: desen1_sim 5-aralık-pin | PASS | **PASS** | — |
| C: FEC-off yapısal | PASS | **PASS** | — |
| fmt/clippy (default+all-features) | temiz | **temiz** | — |

**KESİN-YASAK dokunulmadı** (adversarial ÇÜRÜT-4 doğruladı): bw.rs delivered-EAI/last_sample_bw · send_rate-loss-modeli ·
ZAMAN-MODELİ · SACK-V2 · TokenBucket · PreConverge-CC · qd-canlanma · W_BTLBW. 5c'nin TÜM src-dokunuşları DR-S5-1/2/3
(onaylı-cerrahi); N8+RETRANSMIT yalnız tests/. netem-pinleri (P5/4114→308/qd-52.2s/Y3) nightly-§4.4 (yerel-değil).

## 5. İKİ KAYIT (5e-öncesi zorunlu takip)

### K1 — SPURIOUS-GAP OVERHEAD → 5e-M3/M4 ölçüm-maddesi ("FEC-on temiz-link retransmit-vergisi")
Emisyon-AÇIK, temiz-linkte (loss=0) bile az-sayıda gap-detection görüldü (N8-Test1: loss0=4) — FEC-parity DATA'dan
ayrı-pace edildiği için geçici inflight-seq boşluğu. send_rate'i etkilemedi (INV-RATE-1 korundu) AMA **spurious-retransmit
vergisi** doğuruyor (gereksiz-dup wire-bütçesi). **5e-M3/M4 ölçüm-maddesi:** loss-arm'da GERÇEK-kayıp vs SPURIOUS-gap
ayrımını netem'de ölç; "FEC-on temiz-link retransmit-vergisi" nicele. Bu bir INV-ihlali DEĞİL, verimlilik-kalemi.

### K2 — DR-S5-4 ÖN-KAYDI (sender-RECOVERED → retransmit-cancel + retention-release wiring) · 5e/M1'den ÖNCE ZORUNLU
DR-S5-3 §5 + N8-SINIR-BULGUSU + RETRANSMIT-(e)-BULGUSU birlikte: **sender-tarafı RECOVERED-control işleme BAĞLI DEĞİL.**
Şu-an recovered → `confirmed_logical`/retention-release/retransmit-cancel YOLU YOK; `send_generation` retransmit-diskriminatörü
olarak KULLANILMIYOR (sabit-0). Sonuç: FEC-arm'ın ANA-avantajı (recovered → gereksiz-retransmit-iptali + retention-erken-
serbest) **KAPALI.** **DR-S5-4 (ön-kayıt):** sender RECOVERED-control'ü işlesin → (i) ilgili logical-paketleri `confirmed_logical`'a
(logical-teslim) yazsın AMA BtlBw'ye DEĞMESİN (N8-güvenlik korunur), (ii) bekleyen ARQ-retransmit'i İPTAL etsin, (iii) retention'ı
erken-serbest bıraksın. **Zorunluluk:** DR-S5-4 bağlanMADAN 5e/M1 (FEC-arm vs quinn karşılaştırması) koşulursa FEC-arm
ana-avantajı-kapalı ölçülür → **karşılaştırma GEÇERSİZ.** Sıra: DR-S5-4 (cerrahi; §2-protokol) → SONRA 5e/M1.

## 6. KALAN → 5d/5e (kullanıcı 5d-açılışını verecek)

- **5d:** PARAM-bootstrap + admission.
- **5e:** M1–M5 ölçümleri (+RsSimdEngine-kararı) → D1-EXIT paketi. **ÖN-KOŞUL:** DR-S5-4 (K2) 5e/M1'den ÖNCE; K1 ölçüm-maddesi M3/M4'te.
