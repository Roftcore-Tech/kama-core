# FS-COVERAGE-PLAN-v1.1 — B0 AMENDMENT (DR-FEC-FS-1 v5 §4.7)

**Statü:** FS-COVERAGE-PLAN-v1.0 (sha `91b33e1e`) **BASE**; bu v1.1 yalnız §4.7 + §5-nedensel-düzeltmelerin
koşu-sayısı sonuçlarını işler + F2-DUR-B'nin FS-1 etkisini kaydeder. v1.0'ın diğer-tüm-hücreleri/sampling-
kuralları aynen. Çelişki-sırası: v2 > prereg-v1.1 > **bu coverage-v1.1** > v5 > denetim.

## 1. FS-1 SCREENING — F2-DUR-B ETKİSİ

```text
v1.0: 18 cells × 7 arms × 5 = 630   (arms: A0,A1-EARLY,F0,F1,F2,F3,F4)
F2 → DUR-B (sealed DATA-before-parity ordering çelişkisi; FS-SEAL-COLLISION-MAPS.md) → F2 DROPPED THIS TOUR
v1.1: 18 cells × 6 arms × 5 = 540   (arms: A0,A1-EARLY,F0,F1,F3,F4)
Tasarruf (90 koşu) BAŞKA-FAZA SESSİZCE AKTARILMAZ (§7.5). F2 PENDING-DECISION (Abkarada).
```

## 2. CONFIRMATORY — §5 NEDENSEL DÜZELTME KOŞU-SAYILARI

```text
C-P1: exact-deterministic-deficit paired-trace (IID-ifadesi KALKTI). arms A0,A1-EARLY(r=d),F0(r=d).
      3 deficit × 3 arm × 10 = 90 koşu (DEĞİŞMEZ — arm-sayısı 3).
C-P2: arms {A0,A1-EARLY,F0,F1,F3} = 5 (v1.0'da 4'tü). 2 RTT × 2 deficit × 5 arm × 10 = 200 koşu (160 DEĞİL).
C-P3: 2 revblk × 4 arm × 10 = 80 (DEĞİŞMEZ).
C-P4: 2 RTT × 3 deficit × 3 arm × 10 = 180 (DEĞİŞMEZ).
C-P5: 3 deficit × 4 arm × 10 = 120 (DEĞİŞMEZ).
CONFIRMATORY TOPLAM: 630 → 670   (≤1200 tavan ✓)
near-equivalence → 30-seed genişleme yalnız frozen-rule ile (§7.2/v1.0).
```

## 3. TUR-TOPLAM (bu tur; result-bearing)
```text
FS-1        : 540 (F2-dropped)  [F2 geri-gelirse 630]
Confirmatory: 670
Oracle      : 0 (measured-arm; yeni-koşu-yok)
TUR-TOPLAM  : 1210 result-bearing run   (grand-ceiling 6000 ✓; FS-2/FS-3-bütçesi HARCANMAZ)
```

## 4. ELENEN-ARM × CONFIRMATORY ÇARPIŞMA (Ş3; v5 §13)
```text
Frozen-confirmatory-hücrenin zorunlu-bir-arm'ı FS-1'de mekanik-nedenle-elenirse → hücre KOŞULMAZ →
EVIDENCE-GAP kaydı (O3/K4-bar girdisi). Arm-İKAMESİ YASAK.
```

## PROVENANCE
```text
base   : FS-COVERAGE-PLAN-v1.0 sha256=91b33e1e2dee5d87f9809e986ea4b154f246c66c30cf3a52fb9cefa9a46cb8bb (SUPERSEDED-BY-v1.1)
sha256: 2648370135bae58b6b9a9f8981e0d0da08c67ac3a9b2bbeb831f93d7ae0a4fc5
tarih  : 2026-07-31T14:23:52Z
prereg : FS-PREREG-v1.1.md (ayrı-hash)
```
