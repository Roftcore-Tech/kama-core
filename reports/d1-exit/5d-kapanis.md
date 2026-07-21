# Sprint-5d KAPANIŞ — PARAM-bootstrap + admission + DR-S5-4 (RECOVERED-wiring) — K2-TAM-KAPANIŞ

> **HÜKÜM: Sprint-5d KAPANIR.** 5d-1 (PARAM-bootstrap B2) + 5d-2 (decoder-admission INV-FEC-1) +
> 5d-3 (DR-S5-4 RECOVERED-wiring) + 5d-çıkış (N8-v2) hepsi yeşil. **K2 tamamen kapandı → 5e/M1
> artık ADİL koşulabilir** (FEC-arm ana-avantajı açık). Dal: `d1-sprint5c`.

## 1. Yapısal-kararlar (Abkarada; 5d-açılış-analizi)

- **Yeni `kama-engine` lib-crate** — eksik SoC-düğümü kapandı: noise-wire+fec-codec+schema'ya birlikte-bağlı
  İLK üretim-crate'i (adaptive-transport yalnız dev-dep → transport-core crypto-agnostik KALIR, INV-ENGINE-SOC).
- **DR-S5-5 (ONAYLI):** param_hash BLAKE2s-256 üretim-backend = RustCrypto `blake2` (ADR-03 AMENDMENT-2;
  libsodium-generichash=BLAKE2b≠BLAKE2s latent-uyumsuzluğu → DR-5 RustCrypto-emsali; golden-drift-gate kesintisiz).

## 2. 5d-1 — PARAM-bootstrap (B2) · commit `bd03eae`

- `plan_param_bootstrap` → PARAM_SET (param_hash-bind) + `start_fec_immediately` (proaktif; **PARAM_ACK-beklemeden**).
- **Nedensel-B2-kanıt:** PARAM_SET atomik-seal + **confirmed==unconfirmed FEC-emisyonu bit-özdeş**
  (PARAM_ACK = confirmation-only, gate-DEĞİL; regresyon-guard). Bob-offline'ken FEC akar.
- `ParamState` (receiver): authenticated-PARAM_SET → hash-verify + profile-eşleşme → bind; **mismatch → FAIL-CLOSED**
  (bound=None, admission kapalı). `PreParamHold`: bounded (MAX=256, en-eski-drop+audit; SR-AUDIT-001).

## 3. 5d-2 — decoder-admission köprüsü (INV-FEC-1 production-kapanışı) · commit `bd03eae`

- `admit_and_decode(Vec<OpenedTransportShard>, bound_profile_id, k_actual)` — `receiver_open_decode`
  test-harness'ının ÜRETİM-halefi. **TYPE-GATE:** forge open()-atlayamaz → `OpenedTransportShard` üretemez →
  köprüye ULAŞAMAZ (AEAD + mühürlü-witness + trybuild). **Profil-gate:** shard.profile_id≠bound → fail-closed.
- **N8-firewall:** yalnız app-bytes döner; BtlBw/delivered/credit'e SIFIR-referans.

## 4. 5d-3 — DR-S5-4 RECOVERED-wiring (CERRAHİ; SEÇENEK-A + 3-şart) · commit `9dbd46e`

- `Event::RecoveredControl` (additive) + **on_event SAF-LOGICAL erken-dönüş** (pacer-saati/fuzzy/rate/schedule'a
  girmez) + `on_recovered_control`: (i) retransmit-CANCEL (`pending_lost.retain`; pacer-attempt hiç-başlamaz)
  (ii) retention-RELEASE (`dup_queue.retain`; acked-cancel birebir-paraleli) (iii) `ConfirmSource::Recovered`-
  marker'lı defter. **bw.rs yasak-6-blok'a SIFIR-satır.**
- **Şart-1 N8-bit-özdeşlik:** `dr_s5_4_n8_recovered_does_not_change_btlbw` — fires-vs-noop BtlBw BİT-ÖZDEŞ
  (vacuity-korumalı: pending-dolu + aynı-ön-durum-assert).
- **Şart-2 çevrim-ENGINE'de:** `kama-engine/recovered_map.rs` — (block_id,esi)→unit_id emit-anı-defteri
  (BOUNDED 4096) + `Control::Recovered`-çözücü (BTreeMap::range DoS-güvenli; unresolved-sayılır).
- **Şart-3 M1-erişilebilir:** `diag_recovered_cancels()` public + `take_confirmed_logical` 3-tuple
  (Recovered-vs-WireAck ayrımı). send_generation-semantiği DOKUNULMADI (SEÇENEK-A; B=ileride-alt-DR).

## 5. 5d-ÇIKIŞ — N8-yeniden-doğrulama-v2 · [`N8-v2-recovered-wiring.md`](N8-v2-recovered-wiring.md)

Canlı-pipeline (dış-API-only, tam-zincir: emisyon→kayıp→gerçek-decode-recovery→RECOVERED-wire-roundtrip→
engine-resolve→sender): **btlbw fires==noop BİT-ÖZDEŞ (240000)** + recovered-defter fires=2/noop=0
(**muhasebe-yarısı BAĞLI** — S5-3-sınır-bulgusu kapandı) + cancels fires=1/noop=0 (retransmit-suppress canlı).
**K2-TAM-KAPANIŞ: M1 adil.**

## 6. Non-regression {A+B+C} — 5d-kapanış son-durumu

| Gate | 5c-kapanış | 5d-kapanış | Δ |
|---|---|---|---|
| kama-noise-wire | 90 | **94** | +4 (param_bind) |
| kama-fec-codec | 25 | **25** | 0 |
| kama-transport-schema | 9 | **9** | 0 |
| kama-integration | 12 | **12** | 0 |
| adaptive-transport | 126 | **128** | +2 (DR-S5-4; mevcut AYNEN) |
| kama-engine | — | **15** | yeni-crate (5d-1/2: 10 + map: 4 + n8_v2: 1) |
| B: desen1 5-pin · C: FEC-off | PASS | **PASS** | — |
| fmt/clippy (all-features) | temiz | **temiz** | — |

**KESİN-YASAK dokunulmadı:** bw.rs/send_rate/delivered-EAI/W_BTLBW/ZAMAN-MODELİ/SACK-V2/TokenBucket/
PreConverge/qd — 5d'nin tek-sender-dokunuşu DR-S5-4 (onaylı-cerrahi; additive Event+ayrı-metod; N8-bit-özdeş-kanıtlı).
Doğrulama-metodolojisi (ultracode): 5d-açılış understand-workflow (5-agent; yapısal-boşluk-tespiti) + 5d-1/2
adversarial-workflow (0-blocker; b2-vacuity-note nedensel-testle kapatıldı).

## 7. KALAN → 5e (kullanıcı açılışı verecek)

- **5e:** M1–M5 ölçümleri + RsSimdEngine-kararı → D1-EXIT paketi. **M1 ÖN-KOŞULU SAĞLANDI** (K2-kapalı;
  `diag_recovered_cancels` + ConfirmSource FEC-arm-metriği hazır).
- **K1-hatırlatma (5c-kapanıştan):** FEC-on temiz-link spurious-gap-vergisi → 5e-M3/M4'te netem'de ölçülecek.
- Açık-uçlar: sender-RECOVERED source-shard emisyon-kaydı (şu-an sent_logical yalnız-FEC-parity; DATA-source
  send-record engine-genişlemesi 5e-entegrasyonda) · send_generation-per-logical (SEÇENEK-B, gerekirse alt-DR).
