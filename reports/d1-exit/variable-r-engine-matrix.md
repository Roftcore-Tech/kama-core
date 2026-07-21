# VARIABLE-R ENGINE-EMISSION MATRİSİ (Commit-H; §15) — preflight'ın-gerçek-emisyon-yolu-tekrarı

> Testler: `kama-engine/tests/controller_actuation.rs` (5-test). Zincir GERÇEK-yol: `controller_decision_
> snapshot` (same-event) → `fec_controller_latch` → planned_r × `enqueue_fec_parity` → `plan_fec_parity`
> (token) → FecParity-decisions → seal (RS-repair-symbol) → receiver `admit_and_decode`. Profil-0x03
> (K=8, Rmax=4). Preflight-raporu-korundu ([variable-r-preflight](variable-r-preflight.md)); bu-ek-rapor.

| satır | k_actual | ratio | requested-r | computed | planned | **enqueued** | **emitted** | received-repair | decode | cleanup | contamination |
|---|---|---|---|---|---|---|---|---|---|---|---|
| A | 8 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | ✅ source-only | ✓ stateless | yok |
| B | 8 | 125 | 1 | 1 | 1 | 1 | **1** | 1 | ✅ 1-kayıp-recovered | ✓ | yok |
| C | 8 | 500 | 4 | 4 | 4 | 4 | **4** | 4 | ✅ 2-kayıp-recovered | ✓ | yok |
| D | 8 | 250 | 2 | 2 | 2 | 2 | **2** | 2 | ✅ | ✓ | yok |
| unrec | 8 | 125 | 1 | 1 | 1 | 1 | 1 | 1 | ✅ typed-FAIL (2-kayıp>r; stuck-yok) | ✓ | yok |
| clamp | 8 | 1000 | **8** | 8 | **4** | 4 | 4 | 4 | ✅ 2-kayıp-recovered | ✓ | yok |
| r>0→r=0 | 8 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | ✅ | ✓ | yok (önceki-clamp-bloktan) |
| partial | **5** | 500 | 3 | 3 | 3 | 3 | **3** | 3 | ✅ 1-kayıp-recovered (**k_actual-formülü**) | ✓ | yok |
| emergency | 8 | 500 | **4-görünür** | 4 | **0** | 0 | 0 | — | — (interlock) | ✓ | yok |

Diag-zinciri-doğrulaması: `FecActuationDiag.enqueued_r_total==emitted_r_total==7` (A+B+C+D) — requested→
planned→enqueued→emitted AYRIŞTIRILABİLİR (kesit-1-arm-collapse-körlüğü-giderildi). Out-of-order/duplicate/
interleaved: preflight-kapsamında-kanıtlı (aynı-receiver-yolu; engine-tekrarı consecutive+interleaved-r'de).

**HÜKÜM: engine-path-matrisi PASS → "wire/profil-değişikliği-GEREKMEZ" NİHAİ-kabul-kanıtı tamam** (preflight+
engine-path ikisi-de-yeşil). Harness-notları (araç-doğrulama-tarihçesi; motor-bulgusu-değil): per-paket-anlık-
ACK app-limited-örnek-üretip btlbw=0-bırakır → BATCH-ACK-deseni (SACK-gerçeğine-uygun) şart; repair-esi-uzayı
0-based (codec `esi<r_code_max`; source-uzayından `is_repair`-ile-ayrık).
