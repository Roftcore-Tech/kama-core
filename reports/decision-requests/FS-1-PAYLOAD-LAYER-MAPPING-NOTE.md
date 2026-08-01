# FS-1-PAYLOAD-LAYER-MAPPING-NOTE (DR-FEC-WORKLOAD-1)

> **STATÜ: READ-ONLY AUDIT + MAPPING DECISION. Kod-YOK · freeze-YOK · manifest-YOK · execution-YOK.**
> ACCEPTANCE OUTCOME (§18): **RATIFICATION-REQUIRED.** Fragment-FORMAT'ı production-schema'da otoriter;
> operasyonel app→symbol→block eşlemesi HİÇBİR production-path'te wire-EDİLMEMİŞ (yalnız test/example) →
> §14.B MINIMAL EXPLICIT MAPPING PROPOSAL Abkarada-ratifikasyonu gerektirir. **DUR-B DEĞİL** (deficit-domain
> unique, cross-arm equivalence ulaşılabilir). **FRAGMENTATION-DR-REQUIRED DEĞİL** (geçerli-format + test-kural
> var). HARNESS-EXT-1 ratifikasyon-SONRASI devam-edebilir.

## §1. PROVENANCE
```text
branch/HEAD    : d1-sprint5c / adfaa79 (kod-değişikliği-YOK; audit-only)
FS-PREREG-v1.2 : FROZEN da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable)
kabul-bulgular : FS-1-MANIFEST-INCONSISTENCY (5013d5f) · PAYLOAD-SEMANTICS-DUR-B (adfaa79) — kalıcı
audit-yöntem   : read-only repo-audit + paralel-subagent (deficit-domain doğrulandı; frag/prov agent'ları
                 StructuredOutput-cap-fail → bulgular doğrudan-Read ile bağımsız-tamamlandı)
```

## §2. KABUL EDİLEN LAYER-MIX TEŞHİSİ (Abkarada ratified)
```text
300 B  = tarihsel APPLICATION/WORKLOAD birim-boyu (codec-DEĞİL). app_unit_bytes=300, app_units_per_workload=8,
         total_app_bytes=2400. [O1 prereg-deadline + O2 udp_runner unit_bytes ile TUTARLI.]
E=64   = mevcut PRODUCT CODEC source-symbol geometrisi (MANDATORY-BOOTSTRAP-PROFILE 0x03; receiver_host.rs:40).
Bunlar FARKLI-KATMAN; eşit-DEĞİL. Eski "block_bytes = K×payload (8×300=2400)" app-birim-geometrisini
codec-symbol-geometrisine ÇÖKERTTİ (yanlış-katman). E≈300 codec-profili EKLENMEZ; workload E=64/K=8'e RELABEL-edilmez.
```

## §3. GLOSSARY (typed; "payload" TERİMİ YASAK)
```text
APPLICATION            : app_unit_bytes=300 · app_units_per_workload=8 · total_app_bytes=2400
TRANSPORT SOURCE GEOM. : source_fragment_payload_bytes(≤53) · source_fragments_per_app_unit · total_source_fragments ·
                         source_fragment_index (=esi)
CODEC                  : codec_profile_id=0x03 · codec_k=8 · codec_r(=R) · codec_symbol_bytes_E=64 ·
                         app_capacity_per_codec_symbol=53 · padding_bytes_per_symbol · codec_blocks_per_workload
WIRE                   : source_sealed_bytes=101(@E64) · repair_sealed_bytes=101 · framing_bytes=37 ·
                         total_base_source_wire_bytes · proactive_intervention_wire_bytes · total_outcome_wire_bytes
SEMANTIC DOMAINS       : deficit_domain · duplication_domain · FEC_repair_domain · completion_domain
```

## §4. TARİHSEL 300-B SOYU
```text
[O1] FS-PREREG-v1.1 §B0.2 (v1.2 §D korunmuş): payload=300B PER-SHARD; block_bytes=K×payload=8×300=2400 + r·300.
[O2] udp_runner (adaptive-transport/src/udp_runner.rs:17,75,85): UNIT=1200; unit_bytes∈{65,300,1200} "W-3";
     w_units=8; total_workload=w_units×unit_bytes. DR-CS-1 scriptleri --unit-bytes {65,300,1200} --w-units 8.
→ 300B = tarihsel per-app-unit iş-yükü-boyu; 8-birim; 2400B toplam. (KATMAN: APPLICATION.)
```

## §5. CODEC E=64 OTORİTESİ
```text
receiver_host.rs:40 BOOTSTRAP_PROFILE_ID=0x03 = K=8/R=4/E=64 (MANDATORY; §8-noise-wire B2). profile.rs:70-74.
E-katalog: {64,1024} YALNIZ (profile.rs:56-97) — E=300 YOK. sealed = fs_sealed_len(E)=E+37 → 101@E64, 1061@E1024.
"app-payload ≤300B → 75s sınıfı" (receiver_host.rs:46) = RETENTION-COST timeout-sınıfı (RX_COLD_ABANDON), SHARDING-DEĞİL.
```

## §6. SegmentBlock AUDIT
```text
Framing::SegmentBlock / SegmentBlock TİPİ REPO'DA YOK. (rg "SegmentBlock" → yalnız emergency.rs/tv_emergency.rs
alakasız-eşleşmeler; fragmentation-tipi DEĞİL.) → SegmentBlock mapping-otoritesi DEĞİL.
```

## §7. PRODUCTION REACHABILITY (fragmenter)
```text
serialize_symbol / SourcePrefix-construction TÜM çağıranları test/ VEYA example/ (production src/ çağıran = SIFIR):
  kama-engine/tests/{controller_actuation,fuzzy_e2e,common/mod} · kama-integration/{examples/a2_rx_netem_runner,
  tests/{a2_c1b,a2_f5,a2_f2,pipeline}} · kama-transport-schema/tests/golden_v0 · kama-fec-codec/tests/{golden,known_zero}.
→ PRODUCTION FRAGMENTER YOK. Fragment-FORMAT'ı production-schema'da (prefix.rs) tanımlı AMA app→symbol operasyonel-
  kuralı yalnız test-harness'larında (PAYLOAD_CAP=53; chunk_offset=i×53). reachability: fragment-FORMAT=production-schema;
  fragment-RULE=test-only.
```

## §8. TAM app→fragment→codec→wire YOLU (schema-tanımlı; fragmenter test-only)
```text
app_bytes → SourcePrefix(11B: message_id4+chunk_offset4+actual_payload_len2+source_flags1) + app_payload[≤53] +
  canonical_zero_pad[E−11−len] = E-byte SYMBOL (prefix.rs) → RsSimdCodec.encode(K source → R repair) →
  per-shard AEAD-seal (INV-FEC-1 code-then-encrypt-auth-shards; padded-symbol coded+sealed → PADDING AUTHENTICATED) →
  sealed-datagram (E+37=101B) → transport-channel. Ters-yol: AEAD-verify → decode → SourcePrefix::parse
  (kama-engine/src/admission.rs; actual_payload_len+chunk_offset okunur) → reassembly.
```

## §9. EKSİK SEMANTİK GEÇİŞLER (ratifikasyon-gerektiren)
```text
[M1] app-unit-başına-fragmentasyon mı workload-başına-contiguous mı? (300B→6 symbol/unit VS 2400B→46 symbol total)
[M2] final-symbol zero-pad + son-partial-block K_actual davranışı (48→6-tam-blok VS 46→5×8+1×6).
[M3] deficit-domain FRAGMENTED-geometride: d SYMBOL mü, d APP-UNIT(=d×6 symbol) mi? (§17 altında; authority-default
     esi/symbol-level AMA relative-loss-büyüklüğü tarihsel-1:1'den SAPAR → açık-ratifikasyon).
[M4] completion_domain: tüm 2400 app-byte reassemble+deliver (ilk-K'=4-blok-decode DEĞİL — §F3).
[M5] codec_r (R) her-blok başına mı workload-toplamı mı; F0/A1 matched-budget bu granülariteyle.
```

## §10. EXACT app_capacity_per_E64_symbol
```text
app_capacity_per_codec_symbol = E − SOURCE_PREFIX_LEN = 64 − 11 = **53 B** (schema-const; kama-transport-schema/
src/lib.rs:6-8 "prefix + payload + zero_pad == E"; PAYLOAD_CAP=53 test-const kama-engine/tests/common/mod.rs,
pipeline.rs). "64" VARSAYILMADI — SOURCE_PREFIX_LEN(11) çıkarıldı.
```

## §11. WORKED — bir 300-B app-unit
```text
app_unit_bytes=300 · app_capacity=53 → source_fragments_per_app_unit = ceil(300/53) = 6 symbol.
  symbol 1..5: actual_payload_len=53 (chunk_offset 0,53,106,159,212); symbol 6: actual_payload_len=35 (offset 265)
  + zero_pad = 64−11−35 = 18B. Toplam app 5×53+35 = 300 ✓. symbol-space 6×64 = 384B; sealed 6×101 = 606B.
```

## §12/13/14/15. WORKED — 8×300-B WORKLOAD (per-unit-fragmentation okuması; [M1] ratifikasyona-bağlı)
```text
total_app_bytes                = 8 × 300 = 2400
total_source_fragments(symbols)= 8 × 6   = 48        [M1 per-unit] · (workload-contiguous [M1-alt]: ceil(2400/53)=46)
total_source_symbol_bytes(E)   = 48 × 64 = 3072  (= prefix 48×11=528 + app 2400 + zero_pad 144)
codec_blocks_per_workload      = 48 / K=8 = 6 blok (per-unit: 6 tam-blok, block-pad-YOK) · (46 → 5×8+1×6 K_actual=6)
padding_bytes                  = 144 (symbol-zero-pad; per-unit → block-pad 0)
```
- **§16 A0 source geometry:** 48 InitialSource sealed-datagram (101B/each) sırayla; proaktif-redundancy-YOK.
- **§17 A1 duplication granularity:** duplication_domain = **source codec-symbol / sealed initial-source datagram**
  (deficit-domain ile AYNI granülarite; §22). A1 blok-total-R SEÇİLİ symbol'leri birer-kez kopyalar (301B-app-unit-bütünü
  DEĞİL). matched-wire: A1-dup-sealed == F0-repair-sealed (101B/symbol).
- **§18 F0 geometry:** 6 blok × K=8; her-blok gerçek RsSimdCodec R-repair (E=64/profile-0x03). repair-sealed 101B.
- **§19 F1 geometry:** = F0 (full-K early-logical-close; DATA-first altında wire-time-ilerlemez → STRUCTURALLY_NON_EFFECTIVE).
- **§20 F3 equal-workload:** K'=4 partial-block. **INVARIANT: total_app_bytes(F3)=2400=total_app_bytes(F0)=A0.**
  48-symbol AYNI dizisi K'=4 bloklara bölünür → 12 blok × K'=4 (block-count-artar; workload-AZALMAZ). completion =
  tüm-48-symbol reassemble (ilk-K'=4-decode DEĞİL). [§F3-eşit-workload; tek-K'=4-blok = workload-parçası → YANLIŞ.]
- **§21 F4 variable-K:** f4_select_k∈{1,2,3} → partition-K değişir (block-count/padding/repair-grup değişir) AMA
  offered_app_bytes=2400 + source-fragment-dizisi + completion SABİT. Her-K için 48-symbol farklı-partition.
```

## §22. DEFICIT-DOMAIN KARARI
```text
MEVCUT-authority UNIQUE (subagent-doğrulı): deficit = bir (block_id, esi) InitialSource sealed-datagram/source-symbol
  (fs_trace.rs:72-73,236-252; wire-ordinal YASAK fs_trace.rs:3-5). Mevcut-1:1-geometride (num_units==K==8,
  unit_size==E==64) 1-deficit == 1-symbol == 1-app-unit == 1-sealed-datagram.
FRAGMENTED-geometride (48 symbol ≠ 8 app-unit) AYRIŞIR → RATIFIKASYON-ITEM [M3]: "deficit=1" = 1-SOURCE-SYMBOL mü,
  1-APP-UNIT(=6 symbol) mi? Authority-consistent-DEFAULT = SOURCE-SYMBOL (esi-level; mevcut-domain'i korur) AMA bu,
  tarihsel-1:1-runs'a göre "deficit=1"in RELATIVE-KAYIP-büyüklüğünü değiştirir (1/48 vs 1/8) → açık-ratifikasyon
  ŞART. **DUR-B DEĞİL** (authority esi/symbol-level'e UNIQUE-işaret-eder; fragmented'e taşıma belgelenmiş-seçim).
```

## §23. CROSS-ARM PRE-INTERVENTION EQUIVALENCE
```text
ULAŞILABİLİR (DUR-B DEĞİL): tüm-6-arm AYNI canonical fragmented source-symbol dizisini alırsa (48 symbol, esi 0..47,
  sealed 101B/each, ExactDeficit+PassAll AYNI). Mevcut-harness'ta A0/A1(opak-unit) ve F0(codec-symbol) num_units==K
  ∧ unit_size==E preconditionıyla ZATEN 1:1-geometri paylaşır (subagent-doğrulı). HARNESS-EXT-1 bu birliği
  fragmented-48-symbol'e taşımalı: A0/A1/F0/F1/F3/F4 HEPSİ AYNI 48-symbol source-dizisini; yalnız intervention
  (none/dup/parity/partition) farklı. Load-bearing-precondition: fragmentation-kuralı TÜM-arm'lar-için-AYNI uygulanmalı
  (A0/A1 native-300B-packet iken FEC-arm E=64-symbol OLURSA → packetization/ACK-cadence/loss-granularity confounder →
  CROSS-ARM SOURCE-GEOMETRY GAP; bunu ratified-mapping ÖNLEMELİ).
```

## §24. WIRE-ACCOUNTING TABLE (8×300B workload; per-unit [M1]; R örnek=2/blok)
```text
total_app_bytes                    = 2400
total_source_symbol_bytes(E)       = 48 × 64  = 3072
total_padding_bytes                = 144
total_base_source_sealed_bytes     = 48 × 101 = 4848      (framing 48×37 = 1776)
F0 repair_sealed  (R=2/blok × 6)   = 12 × 101 = 1212      (intervention_wire; proactive)
F3 repair_sealed  (K'=4 → 12 blok; R=2/blok × 12) = 24 × 101 = 2424   [F3 block-count↑ → intervention↑; ratify R-map]
A1 matched-dup_sealed (=F0 R-eşdeğeri) = 12 × 101 = 1212  (matched-wire; C-P4)
total_outcome_wire = base_source + intervention + arq_retransmit + forward-recovery  (§8 iki-defter AYRI)
```
NOT: "300B → 337B sealed" YAZILMADI (300 codec-E DEĞİL; §11 wire-uyarısı). fs_sealed_len(E)=E+37 YALNIZ codec-symbol'e.

## §25. ESKİ DEADLINE-FORMÜLÜ TEŞHİSİ
```text
"block_bytes = K × payload" (8×300=2400) → payload=300B'yi CODEC-symbol-katmanında kullandı (300B/shard) → YANLIŞ-KATMAN.
Gerçek-codec E=64 → block source-sealed = K×101 = 808B/blok (2400 DEĞİL). Formül-otoritesi KALKAR.
```

## §26. YENİ DEADLINE TÜREVİ (ÖNERİ; ratifikasyona-bağlı)
```text
Kavramsal-ayrışma KORUNUR: T_deadline = T_serialize + N_recovery×RTT + M_fixed. T_serialize gerçek-sealed-geometriden:
  T_serialize = (total_base_source_sealed + planned_proactive_intervention_sealed + framing/control) / bw_Bps × 1000 ms.
Örnek (workload=4848B source + R=2 → 1212B repair = 6060B; bw=1Mbps=125000 Bps):
  T_serialize_new ≈ 6060/125000×1000 ≈ 48.5 ms   (ESKİ 2400/125000 ≈ 19.2 ms'nin ~2.5×'i — fragmentation-overhead).
old-vs-new (N_recovery=2, M_fixed=50):
  C-P2 RTT=1000 : OLD 2075ms  → NEW ≈ 48.5 + 2000 + 50 ≈ 2099ms   (CANDIDATE-SUPERSEDED)
  C-P2 RTT=2500 : OLD 5075ms  → NEW ≈ 48.5 + 5000 + 50 ≈ 5099ms   (CANDIDATE-SUPERSEDED)
  C-P5 d2/d4    : OLD 570ms   → NEW ≈ 48.5 + 2×250 + 50 ≈ 599ms    (CANDIDATE-SUPERSEDED)
bit/byte: bw_Bps = bw_bps/8; T = bytes/bw_Bps×1000 ms. (RTT-dominant rejimlerde Δ küçük; düşük-RTT'de belirgin.)
```

## §27. 2075/5075/570 MUAMELESİ
```text
Üçü-de FORMÜL-TÜREVİ (K×300 layer-mixed) → **CANDIDATE-SUPERSEDED** (§26 yeni-türev ratified olana dek). Tarihsel
  dokümantasyon SESSİZCE-ÜZERİNE-YAZILMAZ (v1.1 immutable; v1.3'te açık-supersede).
```

## §28. 305ms / RTT250 PROVENANCE VERDICT
```text
EMPIRICAL WALL-CLOCK (formül-DEĞİL). Kaynak: reports/d1-exit/FS-wallclock-fec-utility.md — FAITHFUL two-netns
  rootless netem (measured-RTT rtt250=250.09ms; 48-koşu/0-fail). 305ms = pre-declared-deadline, ÖLÇÜLEN-percentile-
  aralığından: T_fec_p99≈293-296 < 305 < T_arq_p01≈315-328 (Phase-C). Phase-D/E MarginalFecBeneficial@RTT250 ÖLÇÜLDÜ
  (r<d 333/393 vs r≥d 287/292; Δ−46ms). FS-CORRIGENDUM-deadline.md deficit=4'ü recompute-wall-clock ile KISMEN-geri-çekti.
VERDICT: **EMPIRICAL HISTORICAL ANCHOR.** Layer-mixing-formül-hatası bu ölçülen-sonucu GEÇERSİZ-KILMAZ. 305ms yeni-
  geometri-formülünü KANITLAMAK için KULLANILMAZ (§13); v1.3 açıkça-yeniden-sınıflandırana dek sabit-empirik-çapa.
  CAVEAT: bu wall-clock-runs'ın exact-source-geometrisi (unit_bytes/E) v1.3-çapası taşınırken kaydedilmeli (faithful
  udp_runner-harness; K×300-formülünden bağımsız).
```

## §29. ÖNERİLEN MAPPING AUTHORITY
```text
§14.B PARTIAL PRODUCT AUTHORITY → MINIMAL EXPLICIT MAPPING PROPOSAL (documentation-only; ratifikasyon-öncesi-kod-YOK).
Otoriter-olan: fragment-FORMAT (prefix.rs production-schema) + app_capacity=53 + sealed=101 + deficit=esi/symbol-level.
Ratifiye-edilmesi-gereken-EKSİKLER: [M1] app-unit-vs-workload fragmentation · [M2] final-symbol/partial-block padding ·
  [M3] deficit-domain-under-fragmentation (symbol-default) · [M4] completion=tüm-app-byte · [M5] R-granularity ·
  §23 cross-arm-tek-fragmentation-kuralı. Öneri-DEFAULT'ları: per-unit-fragmentation ([M1]); son-symbol-zero-pad +
  per-unit → block-pad-yok ([M2]); deficit=source-symbol ([M3]); completion=48-symbol-reassemble ([M4]); R-per-block ([M5]).
```

## §30. MINIMAL MAPPING PROPOSAL — RATIFIKASYON GEREKİR mi?
```text
EVET. Yukarıdaki [M1]–[M5] + cross-arm-tek-kural Abkarada tarafından açık-ratifiye-edilmeli. Bunlar authority-consistent
  DEFAULT'lara sahip (schema+test-kanıtlı) AMA production'da wire-edilmemiş → kod-öncesi-ratifikasyon ŞART (§14.B).
```

## §31. DR-TRANSPORT-FRAGMENTATION-1 GEREKİR mi?
```text
HAYIR (bu-turda). Geçerli fragment-FORMAT (production-schema) + test-kanıtlı-kural var → §14.C (no-viable-mapping)
  koşulu SAĞLANMIYOR. Minimal-mapping (§29/30) yeterli. NOT: ratified-mapping bir PRODUCTION-fragmenter gerektirirse
  (yalnız-harness-fragmenter DEĞİL, ürün-yolu da) → o AYRI bir ürün-DR olabilir; FS-1-harness-fragmenter için gerekli-DEĞİL.
```

## §32. HARNESS-EXT-1 DEVAM-EDEBİLİR mi?
```text
RATIFIKASYON-SONRASI EVET. §29/30 minimal-mapping ([M1]–[M5]+cross-arm) Abkarada-onayından SONRA HARNESS-EXT-1
  (F1/F3/F4 closed-loop-arm + IID-plumbing + fragmented-cell-grid + manifest + 540/540-executability + coverage-v1.2 +
  analysis-spec + prereg-v1.3) o authoritative-fragmented-geometri üstünde inşa-edilir. Ratifikasyon-ÖNCESİ kod-YOK.
```

## §33. GOVERNANCE RULES (kalıcı; kaydedildi)
```text
RULE-1 EXECUTABILITY: count-match GEREK-ama-YETER-DEĞİL; her-manifest-satırı per-row executability-proof olmadan
  hiçbir result-bearing-coverage-plan FREEZE-edilemez. result-bearing-ARM = closed-loop ∧ cell-grid-parameterized ∧
  dispatcher-registered ∧ canonical-result-producing ∧ instrument-gate-passing ∧ manifest-dry-run-katılan.
RULE-2 GROUNDING-CHAIN: her-frozen-deney-parametresi ŞUNLARI-tanımlamalı: semantic-layer · unit · owning-repo-authority ·
  wire-davranışına-transformation-chain. Layer-ambiguous-parametre FREEZE-edilemez. ("payload" gibi layer-belirsiz-ad YASAK.)
RULE-3 PRE-INTERVENTION-EQUIVALENCE: paired-causal-arm'lar AYNI canonical-source-workload + pre-intervention-packetization
  paylaşmalı. App-byte-eşitliği TEK-BAŞINA YETMEZ (packetization/fragment-count/source-sealed-size/ACK-loss-granularity
  farklıysa confounder).
```

## §18 ACCEPTANCE OUTCOME
```text
RATIFICATION-REQUIRED. Ürün-yönü net (fragment-FORMAT production-schema'da otoriter; app_capacity=53; deficit=esi/symbol
  unique; cross-arm-equivalence ulaşılabilir) AMA operasyonel-fragmentasyon-kuralı ([M1]–[M5]+cross-arm) production'da
  wire-edilmemiş → minimal-explicit-mapping Abkarada-ratifikasyonu ŞART (kod-öncesi). DUR-B/FRAGMENTATION-DR/FULL-AUTHORITY
  DEĞİL.
```

## STOP
FS-1-PAYLOAD-LAYER-MAPPING-NOTE tamam. Kod-YOK · manifest-YOK · analysis-spec/coverage-v1.2/prereg-v1.3 freeze-YOK ·
FS-1-execution-YOK. Abkarada'nın §29/30 minimal-mapping'i ratifiye-etmesi bekleniyor; SONRA HARNESS-EXT-1 devam.
