# HARNESS-EXT-1 §8 GATE — PAYLOAD-SEMANTICS AMBIGUITY — DUR-B

> **STATÜ: STOPPED AT §8 MANDATORY GATE.** "300-B payload" mapping repository-authority tarafından
> UNIQUE-belirlenmiyor → HARNESS-EXT-1 directive §8 ("This gate is mandatory ... STOP BEFORE IMPLEMENTING
> IT; classify PAYLOAD-SEMANTICS AMBIGUITY — DUR-B; report the competing interpretations") + §23 ("ambiguity
> → DUR-B rather than choosing the convenient interpretation") + §25.A uyarınca DUR-B.
>
> **HİÇBİR HARNESS-EXT-1 KODU YAZILMADI** (F1/F3/F4 arm-YOK · IID-plumbing-YOK · manifest-YOK · coverage-v1.2-YOK
> · analysis-spec-YOK · prereg-v1.3-YOK). Frozen mekanizma DOKUNULMADI. **0 FS-1 result-bearing run.**
> FS-PREREG-v1.2 (da4af3c4…) değiştirilmedi. Uygun-yorum SEÇİLMEDİ (directive §23 yasağı).

## 0. PROVENANCE
```text
branch / HEAD     : d1-sprint5c / 5013d5f (FS-1-MANIFEST-INCONSISTENCY sonrası; kod-değişikliği-YOK)
FS-PREREG-v1.2    : FROZEN sha256 = da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable)
coverage authority: FS-COVERAGE-PLAN-v1.1.md (2648370135…) — payload={300B} sabit (§1)
harness commit    : cd336e9 (F0=profile-0x03 E=64/K=8; F3-fixture=profile-0x02 E=64/K=4)
```

## 1. GATE SORUSU (§8)
"payload = 300 B" için ZORUNLU tam-eşleme gerekir:
`300B logical-workload → source-shard-count → E/symbol-size → padding → block/profile → sealed-wire`.
Bu eşleme YALNIZ (a) protocol/profile-semantics, (b) frozen-prereg/coverage, veya (c) mevcut-product-path'ten
gelebilir — **YENİ sharding/padding-kuralı İCAT EDİLEMEZ.** Aşağıda üç-otorite ÇELİŞİYOR.

## 2. DANIŞILAN OTORİTELER (cite)
```text
[O1] Frozen prereg deadline-formülü (FS-PREREG-v1.1 §B0.2, v1.2 §D'de KORUNUR):
     "block_bytes = K×payload + planned-parity-bytes"; "C-P2 (K=8, payload=300B ... (2400+r·300)/125000)".
     → payload=300B PER-SHARD (source VE parity 300B/shard); block = K×300 = 2400B. IMA: E ≈ 300/shard.
[O2] Mevcut product-path udp_runner (crates/adaptive-transport/src/udp_runner.rs:17,75,85):
     UNIT=1200; unit_bytes ∈ {65,300,1200} (comment "W-3 65/300/1200B"); w_units=8; total_workload=w_units×unit_bytes.
     DR-CS-1 scriptleri: --unit-bytes {65,300,1200} --w-units 8. → payload=300B = PER-DATA-UNIT wire-boyu, K=8.
     (O1 ile tutarlı: block = 8×300 = 2400B.)
[O3] Product-path MANDATORY codec profili (crates/kama-integration/src/receiver_host.rs:40-46):
     BOOTSTRAP_PROFILE_ID=0x03 = K=8/R=4/**E=64**; "E=64 bootstrap ≈53B app → 75s"; "app-payload ≤300B → 75s sınıfı".
     → 300B app-payload E=64 bootstrap-profil SINIFINDA (≈53B app/symbol); block symbol-uzayı = K×E = 8×64 = 512B.
[O4] Codec profil-kataloğu (crates/kama-fec-codec/src/profile.rs:56-97): E ∈ {64, 1024} YALNIZ.
     **E=300 (veya E≈263 sealed-300) profili YOK.** sealed-shard = fs_sealed_len(E) = E+37 → E=64→101B, E=1024→1061B.
[O5] Mevcut FS-harness (frozen cd336e9): run_f0 = profile-0x03 (E=64/K=8); F3-fixture = profile-0x02 (E=64/K=4).
     → tüm-gerçek-codec-arm'ları E=64 kullanır (300B DEĞİL).
```

## 3. ÇELİŞKİ (repo-authority UNIQUE-değil)
```text
O1/O2 (deadline+udp_runner): payload=300B PER-SHARD → block = K×300 = 2400B → E ≈ 300/shard GEREKİR.
O3/O4/O5 (mandatory-profil+codec+harness): gerçek-codec E=64 (mandatory bootstrap); block = K×64 = 512B symbol-uzayı.
    2400B (O1) ≠ 512B (O3/O5). E≈300 profili YOK (O4) → O1 gerçek-codec ile REALIZE-EDİLEMEZ.
Yani: FROZEN deadline-formülü (300B/shard) ⊥ MANDATORY bootstrap-profil (E=64) ⊥ codec-katalog (E=300-YOK).
İkisi-de "frozen authority" (prereg v1.1→v1.2 §D korunmuş) AMA birbiriyle ve gerçek-codec ile TUTARSIZ.
```

## 4. RAKİP YORUMLAR + SONUÇLARI (hiçbiri UNIQUE-değil; hiçbiri seçilmedi)
```text
YORUM-A "per-shard 300B" (O1/O2):
  E ≈ 300/shard; block = K×300 = 2400B; sealed ≈ 337B/shard.
  SONUÇ: gerçek-codec F0/F1/F3/F4 ÇALIŞAMAZ (E=300 profili YOK; §8 "real RsSimdCodec" ihlali). A0/A1 (opak-unit)
         300B çalışır AMA FEC-arm'ları çalışmaz → 6-arm cross-arm comparability KIRILIR.

YORUM-B "300B app-payload → E=64 bootstrap chunk" (O3/O5):
  E=64 (mandatory); 300B app → ceil(300/53)=6 symbol VEYA K=8'e 37.5B/symbol yayıp pad — CHUNKING/PADDING KURALI
  REPO'DA TANIMSIZ. block symbol-uzayı = 512B; sealed = K×101 = 808B.
  SONUÇ: O1 deadline-formülü (block=2400) ile ÇELİŞİR; chunking-kuralı icat-gerektirir (§8 YASAK).

YORUM-C "300B → E=1024 pad" (O4 büyük-profil):
  300B tek E=1024-symbol'e pad; K=8 → block = 8×1024; sealed = 8×1061 = 8488B.
  SONUÇ: O1 (2400) ile ÇELİŞİR; O3 (300B → E=64 SINIFI, E=1024 DEĞİL) ile ÇELİŞİR; %71-pad israfı. Desteklenmez.

CROSS-ARM ORTAK-GEOMETRİ SORUNU (hepsini keser): FS-1'in 6-arm paired-kıyasları (A1-A0, F0-A0, F1-F0, F3-F0,
  FEC-vs-A1-wire-fair) TÜM arm'ların AYNI block-geometrisinde koşmasını ister. A0/A1 opak-unit (300B olabilir),
  F0/F1/F3/F4 gerçek-codec (E=64|1024, 300 DEĞİL). "payload=300B"te 6-arm'ın PAYLAŞTIĞI, codec-realize-edilebilir
  TEK geometri repo-authority'de YOK.
```

## 5. CLASSIFICATION
```text
PAYLOAD-SEMANTICS AMBIGUITY — DUR-B  (HARNESS-EXT-1 §8 mandatory-gate + §25.A "300-B payload semantics
cannot be grounded"). Kök: frozen-prereg-deadline (300B/shard) ile mandatory-bootstrap-codec-profili (E=64) +
codec-katalog (E=300-yok) UNIQUE-tutarlı-eşleme VERMİYOR; 300B→E=64 chunking-kuralı da tanımsız. §23 gereği
uygun-yorum SEÇİLMEDİ.
```

## 6. NE YAPILMADI (disiplin)
```text
✗ HARNESS-EXT-1 4-deliverable'ının HİÇBİRİ yazılmadı (F1/F3/F4 closed-loop-arm · IID-plumbing · cell-grid ·
   manifest · executability-preflight). ✗ coverage-v1.2 / analysis-spec / F1-note / prereg-v1.3 YOK.
✗ Payload-eşlemesi İCAT-edilmedi (YORUM-A/B/C hiçbiri seçilmedi). ✗ Frozen mekanizma (Sender/loss/codec/harness)
   DOKUNULMADI. ✗ FS-PREREG-v1.2 değiştirilmedi. ✗ 0 FS-1 result-bearing run.
```

## 7. ABKARADA-KARARI GEREKENLER (çözüm; UYGULANMADI)
```text
Aşağıdakilerden BİRİNİ authoritative-olarak belirle (yeni-coverage/prereg-amendment ile):
1. "payload=300B" gerçek-codec-arm'ları için TAM eşleme: shard-count · E · padding · profile · sealed-wire.
   (a) E=64 bootstrap + 300B-app'ın K=8'e chunking/padding-kuralını AÇIKÇA tanımla (O3'ü otorite-yap; O1
       deadline-formülünü 300B/shard yerine gerçek-sealed[E=64]-ile-tutarlı yeniden-ifade et), VEYA
   (b) yeni E≈300 (veya E=256) codec-profili ekle (bu KOD-değişikliği; ayrı-DR; freeze-dışı), VEYA
   (c) FS-1 payload'ını mevcut-mandatory-geometriye (E=64/K=8, ≈53B-app veya 512B-block) yeniden-ifade et +
       coverage-grid'deki "300B" etiketini bu geometriyle-tutarlı düzelt.
2. Seçilen eşleme 6-arm'ın HEPSİNCE paylaşılabilir + codec-realize-edilebilir olmalı (cross-arm comparability).
3. Deadline-formülü (O1) seçilen-sealed-geometriyle yeniden-türetilmeli (300B/shard varsayımı gerçek-sealed ile
   uyumlanmalı) — aksi frozen-deadline gerçek-runs'a yanlış-uygulanır.
Karar sonrası HARNESS-EXT-1 (F1/F3/F4 + IID + cell-grid + manifest + executability + coverage-v1.2 + v1.3) o
authoritative-geometri üstünde inşa edilir.
```

## 8. STOP
HARNESS-EXT-1 §8 mandatory-gate'te DURULDU. Abkarada'nın 300B-payload-semantiğini authoritative-çözmesi bekleniyor.
Çözüm gelene dek: kod-YOK · manifest-YOK · freeze-YOK · execution-YOK · yorum-seçimi-YOK.
```text
"If an ambiguity exists: DUR-B rather than choosing the convenient interpretation." (HARNESS-EXT-1 §23)
```
