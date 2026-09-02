# FS-1-RETRY-AND-INVALIDITY-POLICY (S5-A)

> **FROZEN.** 240 primary bilimsel-satır. VALID_COMPLETE/VALID_CENSORED = FINAL (asla-bilimsel-kolaylık-için-rerun).
> YALNIZ INVALID retry-edilebilir (aynı-RunCfg/lineage/pairing; attempt_index++; max 3 = primary+2). append-only.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / 83787d4 · v1.2 immutable da4af3c4…034 · FS-1 0/240
authority : FS-PREREG-v1.2 §A1.9 (INVALID ⟂ INCOMPLETE) + §B0.3 censoring + RULE-14 (immutable attempt history).
```

## 1. Primary satırlar + finality
```text
primary bilimsel-satır : 240 (12 cell × 4 arm × 5 rep).
VALID_COMPLETE VEYA VALID_CENSORED attempt = FINAL. Bilimsel-kolaylık-için ASLA rerun-EDİLMEZ.
YALNIZ INVALID attempt retry-edilebilir.
```

## 2. INVALID retry kuralı
```text
invalid-satır için:
  - orijinal-artifact append-only KORU (asla-overwrite).
  - AYNI bilimsel RunCfg · AYNI trace-lineage · AYNI pairing-identity.
  - attempt_index++ · max-toplam-attempt = 3 (primary + 2 retry).
retry YAPMAZ: yeni-repetition · replacement-seed · trace-target-değişimi · impairment-değişimi · deadline/horizon-değişimi.
```

## 3. Retry-YASAK sebepleri (§6)
```text
ASLA-retry: deadline-miss · censoring · tail-stall · FEC-failure · yüksek-byte-cost · beklenmedik-arm-sırası ·
  elverişsiz-IID-realizasyonu. (Bunlar GEÇERLİ bilimsel-outcome; retry = outcome-güdümlü-manipülasyon YASAK.)
```

## 4. Tüm-attempt INVALID kalırsa
```text
satır UNRESOLVED kalır. sessiz-dışlama-YOK · replacement-lineage-YOK · AÇIKÇA-raporla (availability explicit).
```

## 5. RULE-14 (immutable attempt history)
```text
INVALID-artifact + valid-censored-artifact + superseded-invalid-attempt + retry-lineage HEPSİ KORUNUR (append-only;
asla-sessiz-silme). Analiz-validator quarantine-eder (§21) ama artifact SİLİNMEZ.
```
