# FS-1-BULK-SCOPE-BOUNDARY (FINAL BUILD; human scope decision)

> **STATÜ: OUTCOME-BLIND SCOPE-BOUNDARY (frozen-DEĞİL; kayıt).** FS-1 = BULK-ONLY.

## Frozen intended workload (FS-1)
```text
8 application units · 300 B/unit · 2400 total app-bytes · 48 E=64 canonical source-symbols · MULTI-BLOCK execution.
FS-1'e single-message/single-block stratum EKLENMEZ.
```

## Scope-boundary (bağlayıcı sınırlar)
```text
1. TÜM S0-B emission-timing sonuçları (F1/F3 STRUCTURALLY_NON_EFFECTIVE) YALNIZ mevcut 48-symbol BULK multi-block
   workload + mevcut sender-wide DATA-before-parity ordering için geçerlidir.
2. Bunlar single-message/single-block trafiğe GENELLENEMEZ (tek-blok'ta parity kendi-block-DATA'sının ardındadır;
   sender-wide-gate tek-blok'ta trivial → F1/F3 tek-blok davranışı FARKLI olabilir).
3. FS-2, herhangi ürün-geneli FEC-verdict'inden ÖNCE en-az-bir AYRI-preregistered single-message/single-block
   payload-stratum içermelidir.
4. Interleaving FS-1 DIŞI kalır → DR-FEC-INT-1 (sealed DATA-before-parity ordering-amendment).
```

## PROVENANCE
```text
branch/HEAD : d1-sprint5c / (S1 commit) · outcome-blind · kod-etkilemez
FS-PREREG-v1.2 : da4af3c4… immutable · FS-1 0/540
```
