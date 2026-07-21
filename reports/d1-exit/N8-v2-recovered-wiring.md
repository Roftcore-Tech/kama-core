# N8-YENİDEN-DOĞRULAMA-v2 — RECOVERED-wiring AÇIKKEN (5d-çıkış-şartı) — K2-TAM-KAPANIŞ

> **HÜKÜM: N8-güvenlik-yarısı KORUNUYOR + muhasebe-yarısı ARTIK-BAĞLI.** DR-S5-4 recovered-wiring'i açtıktan
> sonra: recovered → `confirmed_logical` defterine GİRER (S5-3'te-ertelenen muhasebe-yarısı KAPANDI) AMA
> BtlBw/delivered-EAI'ye BİT-EKLEMEZ (güvenlik-yarısı hâlâ kanıtlı). **K2 tamamen kapandı → M1 artık
> ADİL koşulabilir (FEC-arm ana-avantajı — kurtardığını-retransmit-etmeme — AÇIK).**

## 1. İki-katman kanıt

**(a) Sender-birim (DR-S5-4-commit'i, sender.rs):** `dr_s5_4_n8_recovered_does_not_change_btlbw` —
aynı-senaryo (warm-up + kuyruk-sonu-kayıp; pending_lost dolu, iki-koşu aynı-ön-durum-assert'li)
RECOVERED **fires-vs-noop** → **BtlBw BİT-ÖZDEŞ**; fires'ta `diag_recovered_cancels>0` + defter-doldu.
Yapısal-taban: `on_event` SAF-LOGICAL erken-dönüş (RECOVERED pacer-saati/fuzzy/rate/schedule'a HİÇ girmez)
+ `on_recovered_control` bw.rs yasak-6-bloğuna SIFIR-satır.

**(b) Canlı-pipeline (bu-adım, `kama-engine/tests/n8_v2.rs` — dış-API-only):** tam-zincir:
```
sender FEC-parity-emisyonu (send_record → RecoveredMap.register, emit-anı GERÇEK-karardan)
  → parity-kayıp (ACK'lenmez; gap-detection → pending_lost)
  → receiver GERÇEK-FEC-decode-recovery (admit_and_decode; source{1,3}-kayıp+repair → app-bytes TAM)
  → Control::Recovered seal→open→decode_control (WIRE-roundtrip doğrulandı)
  → engine resolve (block_id/esi→unit_id — ÇEVRİM-ENGINE'DE, DR-S5-4-şart-2)
  → sender Event::RecoveredControl
```

**Ölçüm (deterministik):**

| Koşu | btlbw | recovered_cancels | Recovered-marker'lı-defter | wire-roundtrip |
|---|---|---|---|---|
| noop | **240000** | 0 | 0 | ✓ |
| fires | **240000** | **1** | **2** | ✓ |

- **Güvenlik-yarısı:** btlbw **BİT-ÖZDEŞ (240000==240000)** — recovered-wiring-AÇIKKEN kredi-yolu kapalı.
- **Muhasebe-yarısı (ARTIK-BAĞLI):** fires'ta `confirmed_logical` 2 `ConfirmSource::Recovered`-girişi
  (noop'ta 0) — S5-3-N8-raporundaki "muhasebe-yarısı-ertelenmiş" sınır-bulgusu KAPANDI.
- **Retransmit-cancel (M1-sinyali):** `diag_recovered_cancels` fires=1 / noop=0 — pending-retransmit
  RECOVERED'la iptal edildi (FEC ana-avantajı canlı-kanıt).

## 2. M1-erişilebilirlik (DR-S5-4-şart-3)

- `Sender::diag_recovered_cancels()` **public** — M1'de FEC-arm-metriği ("kurtardığını-retransmit-etmiyor"un sayısı).
- `take_confirmed_logical() → Vec<(seq, SrLogicalRef, ConfirmSource)>` — Recovered-vs-WireAck-teslim ayrımı M1'de sayılabilir.
- `RecoveredMap.resolve` unresolved-sayar (sessiz-atlamaz) — kayıp-eşleme teşhisi.

## 3. Sınır-notları (dürüst-kapsam)

- Canlı-pipeline'da cancels=1 (2-değil): iki-parity'den-biri RECOVERED-anından-önce meşru-retransmit-edilmişti
  (pacer-token drenajı) — gerçek-dünya-yarışının aynısı; kalan-pending İPTAL edildi. Sender-birim-testi (a)
  cancel-mekanizmasını pending-doluyken tam-genişlikte kanıtlar.
- RecoveredMap kayıtları emit-anı GERÇEK `TransmitDecision.send_record`'undan (sentetik-değil).
- Kayıtsız-recovered (sent_logical'da-olmayan) → defter-NO-OP + cancel-saymaz (bilinmeyen-logical
  teyit-edilmez; `dr_s5_4_recovered_cancels_and_ledger` kanıtlı).

## 4. Non-regression {A+B+C} (bu-adım)

| Gate | Önce | Sonra |
|---|---|---|
| adaptive-transport | 128 | **128** (değişmedi) |
| kama-engine | 14 | **15** (+1 n8_v2) |
| nw/fec/schema/integ | 94/25/9/12 | **aynen** |
| B: desen1 5-pin · C: FEC-off | PASS | **PASS** |
| fmt/clippy | temiz | **temiz** |

**HÜKÜM: N8 (INV-RATE-1-yanı + recovered≠ACK/BtlBw) RECOVERED-wiring-AÇIKKEN korunuyor; muhasebe bağlı; K2 kapandı. 5d-çıkış-şartı SAĞLANDI.**
