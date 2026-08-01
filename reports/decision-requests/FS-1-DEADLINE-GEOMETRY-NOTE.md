# FS-1-DEADLINE-GEOMETRY-NOTE (DR-FEC-WORKLOAD-1 §11)

> **STATÜ: PROPOSAL — ONAY-BEKLİYOR (frozen-DEĞİL).** §11 gereği v1.3-öncesi arm-independent deadline türevi.
> Hiçbir deadline FREEZE-edilmedi. 305ms empirical-anchor KORUNUR; 2075/5075/570 SUPERSEDED-CANDIDATE;
> önerilen yeni-değerler (§4) DONDURULMADI. Bir-deadline outcome'a-BAĞLI-OLAMAZ; paired-cell arm'lara
> FARKLI-deadline VEREMEZ. → **DEADLINE-COMPARABILITY DUR-B DEĞİL** (arm-independent-deadline türetilebilir).

## 1. ORTAK SOURCE-SERIALIZATION ENVELOPE (arm-independent)
```text
Ratified §4: TÜM-6-arm AYNI 48-symbol canonical-source-dizisini gönderir → base-source-wire kollar-arası ÖZDEŞ.
  total_base_source_sealed_bytes = 48 × fs_sealed_len(64) = 48 × 101 = 4848 B  (arm-INDEPENDENT).
Proaktif-intervention (F0-repair / F3-repair / A1-dup / F4-repair) arm-BAĞIMLI → deadline'a GİRMEZ (§11
  arm-independence şartı). Bu, deadline'ı tüm-arm'lar-için TEK-değer yapar.
```

## 2. ARM-INDEPENDENT DEADLINE FORMÜLÜ (proposal; layer-repaired)
```text
T_deadline(cell) = T_serialize_source(cell) + N_recovery × RTT(cell) + M_fixed
  T_serialize_source(cell) = total_base_source_sealed_bytes / effective_Bps(cell) × 1000 ms
  N_recovery = 2  (birinci-uçuş + BİR bounded-ARQ-recovery-turu; prereg §B0.2'den KORUNUR)
  M_fixed    = 50 ms (prereg §B0.2'den KORUNUR)
KATMAN-ONARIMI: eski "block_bytes = K×payload(300)" (codec-symbol-katmanına-300B) KALDIRILDI. T_serialize artık
  GERÇEK-sealed-source-envelope'dan (48×101=4848B), APP-birim-300B'den DEĞİL. Bu §11/§25(mapping-note) layer-fix.
AÇIK-SORU (onay): effective_Bps = C mi (link-rate) yoksa 0.80×C mi (util-paced offered-rate)? Frozen util=0.80C
  semantics'i hangisini deadline'a-sokar? (Aşağıda ikisi-de gösterildi.)
```

## 3. §11 SORULARINA CEVAP
```text
Q ortak-source-envelope? → 48-symbol / 4848B base-source (§4 tüm-arm ÖZDEŞ). ✓
Q proaktif-intervention deadline'a-girer-mi? → HAYIR (arm-bağımlı; arm-independence için HARİÇ). ✓
Q F3/F4 farklı-block-count nasıl? → block-count intervention'ı (repair-count/timing) etkiler, SOURCE-envelope'u
  DEĞİL → deadline ETKİLENMEZ (arm-independent). ✓
Q configured-rate/RTT nasıl? → T_serialize = source_bytes/effective_Bps; +N_recovery×RTT +M_fixed. (effective_Bps
  util-tanımı onay-bekler.) ✓
Q hangi-deadline empirical-vs-formula? → 305@RTT250 = EMPIRICAL-ANCHOR; §4-tüm-değerler FORMULA-DERIVED-CANDIDATE. ✓
ARM-INDEPENDENCE PROOF: T_deadline yalnız cell'e (RTT, bw, ortak-source-envelope) bağlı; arm'a DEĞİL → 6-arm AYNI-deadline.
```

## 4. CANDIDATE DEĞERLER (DONDURULMADI; onay-sonrası v1.3)
```text
C=1Mbps=125000 Bps; T_serialize @C = 4848/125000×1000 = 38.8ms; @0.80C(100000 Bps) = 48.5ms. (N_rec=2, M=50)
| cell RTT | T_ser@C | T_dl@C | T_ser@0.80C | T_dl@0.80C | eski-formül | statü |
| 50ms     | 38.8    | 188.8  | 48.5        | 198.5      | —           | CANDIDATE (yeni) |
| 250ms    | 38.8    | 588.8  | 48.5        | 598.5      | 570 (d2/4)  | CANDIDATE; 305-anchor-AYRI (§5) |
| 1000ms   | 38.8    | 2088.8 | 48.5        | 2098.5     | 2075        | CANDIDATE; 2075 SUPERSEDED |
| 2500ms   | 38.8    | 5088.8 | 48.5        | 5098.5     | 5075        | CANDIDATE; 5075 SUPERSEDED (C-P2) |
Mapping-note-önerisi 2099/5099/599 ≈ @0.80C-satırı; **DONDURULMADI** (§11 "do NOT yet freeze"). Hepsi CANDIDATE.
```

## 5. 305ms / RTT250 EMPIRICAL ANCHOR + GEOMETRİ-CAVEAT
```text
305ms = EMPIRICAL WALL-CLOCK anchor (FS-wallclock-fec-utility.md; faithful two-netns netem; ölçülen-percentile
  293-296<305<315-328). KORUNUR (§11). Layer-mix-formül-hatası bu ölçümü GEÇERSİZ-KILMAZ.
⚠ GEOMETRİ-RECONCILIATION (onay-item): empirical-305 vs yeni-candidate-598.5 (RTT250) BÜYÜK-SAPAR. Neden: tarihsel
  wall-clock-runs muhtemelen YENİ-48-symbol/4848B-geometride-DEĞİLDİ (o geometride completion ~590ms-beklenirdi,
  ölçülen ~290ms) → tarihsel-geometri DAHA-KÜÇÜK (muhtemelen tek-blok/8-symbol). Sonuç: **305-anchor YENİ-48-symbol-
  FS-1-completion'larıyla DOĞRUDAN-KIYASLANAMAYABİLİR** (farklı-source-envelope). v1.3 açıkça: (a) 305'i tarihsel-
  geometri-etiketiyle-korur AMA yeni-FS-1-deadline'ı yeni-geometriden-türetir, VEYA (b) tarihsel-geometriyi yeni-FS-1'e
  eşleştirir. Bu RECONCILIATION Abkarada-onayı gerektirir (§13 historical-geometry-provenance). **305 yeni-formülü
  KANITLAMAK için KULLANILMAZ** (§13).
```

## 6. SUPERSESSION (dokümantasyon; sessiz-üzerine-yazma-YOK)
```text
2075 / 5075 / 570 ms  → SUPERSEDED-CANDIDATE / v1.3-için-OTORİTER-DEĞİL (K×300 layer-mixed).
2099 / 5099 / 599 ms  → önerildi (mapping-note §26) AMA DONDURULMADI; bu-nota'daki @0.80C-candidate'larla-uyumlu.
305 ms @RTT250        → EMPIRICAL HISTORICAL ANCHOR (geometri-caveat §5); v1.3 açıkça-yeniden-sınıflandırana-dek sabit.
```

## 7. ONAY-GEREKTİREN ITEM'LER (Abkarada)
```text
1. effective_Bps = C mi 0.80×C mi (util-tanımı deadline'a).  2. N_recovery=2 / M_fixed=50 yeni-geometride-korunur-mu.
3. 305-anchor vs yeni-48-symbol-geometri RECONCILIATION (§5).  4. candidate-deadline-tablosunun (§4) v1.3-adayı-olarak-onayı.
Onay-SONRASI bu değerler v1.3'te dondurulur (§13 freeze-order). Onay-ÖNCESİ freeze-YOK.
```
```text
"A deadline may not depend on the observed outcome; a paired cell may not give different deadlines to different
arms merely because one arm emitted more intervention traffic." (§11)
```
