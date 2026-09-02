# FS-1-RAW-ARTIFACT-POLICY (S5-A)

> **FROZEN.** Raw result-artifact'ları append-only · immutable · asla-overwrite · hemen-checksummed. INVALID-attempt
> + retry-lineage KORUNUR. S5'te HİÇBİR raw-artifact ÜRETİLMEZ (result-bearing-firewall; 0/240).

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / 83787d4 · v1.2 immutable · schema: FS-1-RAW-SCHEMA-v1.json · RULE-14 (immutable attempt history).
raw-namespace : reports/fs-1/raw/PENDING-v1.3-FREEZE (S5-sonrası v1.3-freeze'de FINAL-namespace bağlanır; şu-an YOK).
```

## 1. Artifact kuralları
```text
- append-only : her attempt AYRI dosya (canonical_output_path = <raw-namespace>/<run_id>.json; attempt_index ile).
- immutable : yazıldıktan sonra ASLA overwrite/edit. INVALID-retry yeni attempt_index → yeni dosya (orijinal KORUNUR).
- checksummed : yazımdan-hemen-sonra SHA-256 (artifact-integrity; freeze-lock-provenance-alanları içerir).
- append-only-ledger : primary-run-ledger (hangi run_id koştu) append-only; sessiz-silme/overwrite YOK.
```

## 2. Result-bearing firewall (S5)
```text
S5'te: raw-artifact YAZILMAZ · outcome-field POPULATE-EDİLMEZ · campaign-event-loop İLERLETİLMEZ · run-ID HARCANMAZ.
FINAL raw-namespace v1.3-freeze'de bağlanır; S5'te namespace PENDING (henüz-YOK) → yanlışlıkla-yazım-imkânsız.
gerçek result-bearing yazım YALNIZ S6-onayı + explicit result-bearing-CLI-flag + clean-provenance + empty-namespace ile.
```

## 3. Quarantine (analiz-validator; §21)
```text
INVALID / mismatch (freeze-lock/manifest/prereg/binary/exec-hash/source-hash/trace-lineage/schedule) → quarantine.
quarantine = analiz-DIŞI-tut AMA artifact-SİLME (append-only korunur; RULE-14).
```
