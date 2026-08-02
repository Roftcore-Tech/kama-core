# S5-EXECUTION-BASELINE-GAP (S5-C → STOP; readiness-incomplete before one-way freeze)

> **STOP before FS-1-FREEZE-LOCK-v1.3.** S5-A + S5-B TAMAM (authority-audits + coverage-v1.2 + analysis/oracle/
> structural-null/F4 frozen-artifacts + validators; gate-yeşil). S5-C: reproducible SOURCE-provenance kuruldu AMA
> **reproducible EXECUTION-BINARY kurulamaz** — one-way freeze-lock (§31) `execution_binary_sha256` gerektirir; §36
> "binary provenance not reproducible → DUR-B" + §30 "readiness incomplete → freeze-lock YARATMA, STOP, eksik-otorite-
> raporla". İrreversible freeze non-existent/non-reproducible binary üstüne MİNTLENMEZ. **Abkarada kararı bekleniyor.**

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / a0e6308 (S5-B) · v1.2 immutable da4af3c4…034 · FS-1 0/240 · v1.3-freeze YOK.
S5 ilerleme : S5-A ✓ (4b276f6) · S5-B ✓ (a0e6308) · S5-C SOURCE-provenance ✓ / EXECUTION-BINARY ✗ → STOP.
```

## 1. Ne TAMAM (reproducible)
```text
- SOURCE/toolchain provenance: commit a0e6308 · Cargo.lock sha256 47893d24…4 · rustc/cargo 1.96.0 (pinned
  rust-toolchain.toml) · target x86_64-unknown-linux-gnu · feature experimental-fs-harness · build-command bilinir.
  (FS-1-EXECUTION-BASELINE-v1.json) — bu, binary'yi DETERMINISTIK-tanımlar.
- Runtime-kod: manifest-generator + validator + executability-mode + raw-schema + analysis-skeleton HEPSİ TAMAM.
```

## 2. Ne EKSİK (blocking)
```text
(a) FS-1 RESULT-BEARING RUNNER YOK: manifest → 240-row → run_spine(result-bearing) → raw-record yazan binary MEVCUT-DEĞİL.
    fs_runner "result-bearing-DEĞİL / FS-1 AYRI (v1.2-freeze-sonrası)". result-bearing CLI-flag (§27) YOK.
    → §23 "all runtime code complete before baseline" SAĞLANMADI (result_bearing_runner: false).
(b) REPRODUCIBLE BINARY SHA-256 kurulamaz: deterministic-build infra (ADR-19 musl-static + --remap-path-prefix +
    reproducible-flags) FS-1 için kurulmadı → binary-hash environment'a-bağlı (benim-host ≠ CI ≠ S6-verify) →
    "reproducibly-verifiable" DEĞİL. §36 STOP-condition: "binary provenance is not reproducible".
⇒ §31 freeze-lock `execution_binary_sha256` alanı DOLDURULAMAZ (gerçek+reproducible). §30 readiness-item-G (frozen-
  binary + reproducible) INCOMPLETE → §30 "DO NOT CREATE FREEZE LOCK; STOP; report exact missing authority".
```

## 3. Neden şimdi STOP (irreversibility)
```text
Freeze-lock ONE-WAY (§31). Non-existent/non-reproducible binary-hash üstüne mint edilirse: (i) §31 ihlali; (ii) §36
DUR-B-condition; (iii) S6 (§33 "verify exact binary hash") doğrulayamaz → freeze GEÇERSİZ. İrreversible-flawed-freeze
üretmektense §30/§36'nın emrettiği gibi STOP + Abkarada-kararı doğru-yol. (S3-A ve S4-A DUR-B'leriyle aynı disiplin.)
S5-D (candidate→final) / S5-E (prereg) / S5-F (lock) BAŞLATILMADI — hepsi frozen-baseline'a bağlı.
```

## 4. Abkarada kararı (SEÇENEKLER)
```text
OPT-1 (BUILD-RUNNER + DETERMINISTIC-BUILD): FS-1 result-bearing runner (result-bearing-flag'li; default EXECUTABILITY_ONLY)
  + ADR-19 musl-static-reproducible-build (--remap-path-prefix, pinned) kur → reproducible execution_binary_sha256 üret →
  SONRA S5-C-baseline + S5-D/E/F freeze. (En-sağlam; ama yeni-runtime-component + build-infra işi.)
OPT-2 (SOURCE-PROVENANCE-FREEZE): freeze-lock'ta execution_binary_sha256 yerine SOURCE-provenance (commit + Cargo.lock-sha
  + pinned-toolchain + build-command) bağla; gerçek-binary S6-build'de üretilir + hash S6-lock-amendment ile eklenir.
  §31'i gevşetir (execution_binary_sha256 → deferred-to-S6) → Abkarada-onayı ŞART (directive-sapması).
OPT-3 (RUNNER-NOW, HASH-BEST-EFFORT): runner'ı-kur + non-reproducible-binary-hash'i "environment-specific, S6-rebuild-verify"
  etiketiyle lock'a-koy. §36 "not reproducible → DUR-B" ile ÇELİŞİR → ÖNERİLMEZ.
ÖNERİ: OPT-1 (doğru, sağlam) VEYA OPT-2 (hızlı, directive-sapma-onayı-ile). Her-iki-durumda binary-approach Abkarada'nın.
```

## 5. STOP
```text
S5-C SOURCE-provenance ✓ / EXECUTION-BINARY ✗ → readiness-incomplete → FS-1-FREEZE-LOCK YARATILMADI. S5-D/E/F YOK.
kod/spec/frozen-matris DEĞİŞMEDİ (S5-A/B artifact'ları committed); sender/loss zero-diff; FS-1 0/240; v1.2 immutable.
Sonraki: Abkarada execution-binary-approach kararı (OPT-1/2). Karar-sonrası S5-C-tamamla → S5-D/E/F → S6.
```
