# DR-CS-1b / A2 — [CAL] TÜRETİM + SAYISAL FREEZE — ClientReceiver r2 [SEALED — SCOPE-BOUNDED]

> **MÜHÜR KAYDI (Abkarada, 2026-07-27):** *"K-6 ADIM-6: GO — CLIENTRECEIVER [CAL] r2 SCOPE-BOUNDED OLARAK
> MÜHÜRLENDİ."* Damga kabul: sha256=5b298244722eafe12611d027cae1c980763e69c744ee01562c69f10a59738318
> @2026-07-27T20:52:39Z. **İSTİSNA:** CONTROL_DEDUP_WINDOW=4096 MÜHÜRLENMEDİ — CANDIDATE; dört hard-değer
> (MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION · MAX_CONTROL_ID_BURST · MAX_CONTROL_REORDER_ALLOWANCE ·
> MAX_RETRY_REORDER_HORIZON) TÜM logical-control üreticileri üzerinden türetilip normatif-donduktan sonra
> required_window-hesabıyla ayrı **CAL-10 addendum** hazırlanır ve implementation-ÖNCESİ onaya sunulur.
> İcra-talimatında fixed-window-implementasyonundan önce **F-CID-0 kapısı**: producer-inventory →
> hard-rate/burst/reorder-derivation → CAL-10-freeze → fixed-window-impl. C-8: ACCEPTED FOR
> RESOURCE-BOUNDEDNESS ONLY (fairness/availability HubReceiver-devri; zorunlu-diag korunur).
> Mühür-sonrası değişiklik = yeni-versiyon + yeniden-onay; GEVŞEME YASAK. Production-impl NO-GO.

**Statü:** K-6 ADIM-6 taslağı, **r2** — Abkarada DUR-B kararının üç zorunlu türetim-düzeltmesi işlendi:
(1) toplam-envelope modeli (16 MiB = cold-15 + side-1; "16+1-üstüne" REDDEDİLDİ), (2) reachable-shard-cap
(TOTAL=192; "C-6/320>256" hükmü KALDIRILDI), (3) dedup-window HARD-bound-kanıtı yokken **CANDIDATE**.
C-8 artık-riski **yalnız resource-boundedness için, kapsam-sınırlı KABUL** + zorunlu-diagnostik seti (§4-C-8).
**Bu belge Abkarada onayıyla MÜHÜRLENİR**; r2-bütünlük damgası §9'da. Koşu-sonucu görülerek HİÇBİR değer
GEVŞETİLEMEZ; model-doğrulama yalnız SIKILAŞTIRABİLİR.

**Rol-kapsamı:** `TARGET_DEPLOYMENT_ROLE = ClientReceiver`. Değerler **HubReceiver'a TAŞINMAZ**; Test/CI
değerleri production-sabiti SAYILMAZ. Per-peer/multi-federation fairness ve çok-hostile-session-availability
bu freeze'le DOĞRULANMIŞ SAYILMAZ → HubReceiver rol-freeze'ine devredilir (Abkarada C-8 kararı).

---

## 1. GİRDİLER (Abkarada, K-6 adım-5 + adım-6-DUR-B — AYNEN)

```text
PROCESS_MEMORY_BUDGET                        = 256 MiB   (A2 kaynak-planlama envelope'u; OS-garantisi değil)
EXPECTED_CONCURRENT_AUTHENTICATED_SESSIONS   = 32        (kapasite-planlama; güvenlik-sınırı DEĞİL)
MAXIMUM_COLD_STATE_MEMORY_PERCENTAGE         = 6.25%
EXPECTED_CONCURRENT_RX_COLD_BLOCKS_PER_SESSION = 4       (genel RxColdLedgerBlock planlama girdisi)

TOPLAM-ENVELOPE MODELİ (DUR-B düzeltme-1; "cold+side-üstüne" REDDEDİLDİ):
TOTAL_A2_RX_MEMORY_BUDGET          = 256 MiB × 6.25% = 16 MiB    (A2-ilişkili TÜM bounded-state)
SIDE_STATE_RESERVE                 = 1 MiB                        (dedup + tombstone + session-taban + pay)
GLOBAL_COLD_RETAINED_MEMORY_BUDGET = 16 − 1 = 15 MiB              (HARD cold-retained security cap)
PLATEAU-TOPLAM-INVARIANTI          : cold-retained + bounded-side-state ≤ 16 MiB
```

Kod/spec-gerçekleri (türetimin ikinci girdi-seti; doğrulanmış):
```text
PROFILES-allowlist (kama-fec-codec/src/profile.rs): max(K_shape + R_code_max) = 16+8 = 24 (0x04/0x06)
                                                    max E = 1024 (0x05/0x06; sealed 26+11+1024 = 1061B)
MAX_PREPARAM_SHARDS = 256 (kama-engine/src/preparam_hold.rs:13 — fiziksel hold-kapasitesi)
recv-yön ReplayWindow = 2 KiB/epoch-yönü (pn.rs)
K-2 alt-küme-modeli ONAYLI: BootstrapColdBlock ⊂ RxColdLedgerBlock
```

## 2. RETAINED-MEMORY MODELİ (yalnız-payload-hesabı YASAK)

Model-sabitleri (KONSERVATİF; §8 model-doğrulama kapısıyla ölçüme bağlanır — yalnız SIKILAŞIR):

| Model-sabiti | Değer | İçerik |
|---|---|---|
| `SHARD_RETAINED_COST(E=1024)` | **1536 B** | symbol-buffer allocator-sınıfı 1024×1.25=1280 (Vec-capacity + malloc-chunk-header + size-class) + `H_SHARD=256` (OpenedTransportShard DTO ≈48 + LogicalShardHeader ≈16 + hold-slot ≈48 + per-shard (block_id,esi)-dedup-girdisi ≈48 + pay) |
| `SHARD_RETAINED_COST(E=64)` | 336 B | 64×1.25=80 + 256 (bilgi-amaçlı; CAP'ler worst-case E=1024'ten) |
| `BLOCK_META_COST` | 512 B | blok-girdisi + ESI-set-başlığı + deadline-slotu + progress-sayaçları + profil-ref |
| `SESSION_COLD_FIXED` | 1024 B | ParamState(~40) + defter-girdisi + deadline-çarkı + ledger-sayaçları + pay |

**Kapsam (DUR-B düzeltme-1):** ControlDedupWindow-bitmap (512 B/yön), tombstone (~4.5 KiB/kayıt) ve
session-taban (~4 KiB/protector) COLD-defterine değil **SIDE_STATE_RESERVE=1 MiB** içine sayılır — yani
%6.25-envelope'un İÇİNDE (üstüne-toplama modeli reddedildi). Decoder-scratch cold-RETENTION'a girmez
(decode-anlık; reusable-bounded-pool fec-core-§9) — plateau RSS-ekseni yine de yakalar.

**Retained-shard sınıf-beyanı (DUR-B düzeltme-2 önkoşulu):** cold-defterdeki TÜM shard'lar AÇIK
RxColdLedgerBlock'lara aittir. Tamamlanan-blok buffer'ları cold-defterde YAŞAMAZ (v2-§3: retention-devri
fec-core-§9 sınırlarına — ayrı muhasebe). Açık-blok-modeli-dışı in-ledger retained-shard sınıfı
TANIMLANMAMIŞTIR → reachable-shard-cap blok-şekliyle sınırlıdır (§3-#6). Gelecek dilim böyle bir sınıf
tanımlarsa TOTAL yeniden-türetilir (yeni DUR-B).

## 3. TÜRETİM TABLOSU (kaynak-girdi · formül · yuvarlama · güvenlik-gerekçesi · model)

| # | Sabit | DEĞER | Kaynak-girdi | Türetim-formülü | Yuvarlama | Güvenlik-gerekçesi | Retained-memory modeli |
|---|---|---|---|---|---|---|---|
| 0 | `TOTAL_A2_RX_MEMORY_BUDGET` | **16 MiB** | PROCESS×COLD-% | 256 MiB × 6.25% | tam (=) | A2-ilişkili TÜM bounded-state'in envelope'u (plateau-toplam-invariantı) | cold(#1) + side(#0b) ayrışımı |
| 0b | `SIDE_STATE_RESERVE` | **1 MiB** | Abkarada DUR-B | sabit-rezerv | tam (=) | dedup+tombstone+session-taban envelope-İÇİ | C-7: 608 KiB ≤ 1 MiB |
| 1 | `GLOBAL_COLD_RETAINED_MEMORY_BUDGET` | **15 MiB** | #0 − #0b | 16 − 1 | tam (=) | HARD cold-retained cap; tüm cold-rezervasyonların birincil sınırı | tüketiciler #2/#3/#6 |
| 2 | `RX_COLD_RETAINED_BYTE_CAP_PER_SESSION` | **480 KiB** (491 520 B) | #1 + N-session | 15 MiB / 32 (eşit-pay) | tam (=) | fail-closed eşit-pay **N≤32 zarfında** (C-8); tek-session tüm-bütçeyi işgal edemez | C-3-tavanı: 300 032 B ≤ cap |
| 3 | `GLOBAL_COLD_BYTE_CAP` | **15 MiB** | #1, #2 | min(15 MiB, 32 × 480 KiB) = min(15,15) | tam (=) | Abkarada min()-formu; defter GLOBAL-eksende önce-dolan-reddeder | #2-modelinin N-katı; C-8-bağlayıcı |
| 4 | `RX_COLD_OPEN_BLOCK_CAP_PER_SESSION` | **8** | EXPECTED_COLD_BLOCKS=4 | 4 × headroom-2 (kapasite-türetimi; §5) | headroom-sabit-2 | RxColdLedgerBlock eşzamanlılık-sınırı; ötesi typed-reject | BLOCK_META=512 B/blok → 4 096 B rezerv |
| 5 | `RX_COLD_SHARDS_PER_BLOCK_CAP` | **24** | PROFILES-allowlist | max(K_shape + R_code_max) = 16+8 | tam (=) | geçerli-ESI-uzayı ≤ K+R; allowlist-dışı kimlik rezerv alamaz | blok-başı worst 24×1536 = 36 864 B |
| 5b | `MAX_SHARDS_BY_BLOCK_SHAPE` | **192** | #4 × #5 | 8 × 24 (DUR-B düzeltme-2 formülü) | tam (=) | REACHABLE-üst-sınır: açık-blok-modeli dışı in-ledger shard-sınıfı YOK (§2-beyanı) | 192×1536 = 288 KiB |
| 6 | `RX_COLD_TOTAL_SHARD_CAP_PER_SESSION` | **192** | #2-model + #5b | min( floor((491 520−1 024−4 096)/1536)=**316**, 192 ) = **192** | min (bellek-kolu AŞAĞI 316.6→316) | **bağlayıcı-eksen = BLOK-ŞEKLİ** (allowlist-türevi); bellek-kolu 316 yalnız-türetim-kaydı; allowlist genişlerse yeniden-türetim | model-kolu: SHARD_COST(1024)=1536 B (§8-ölçüm-kapılı) |
| 7 | `RX_PREPARAM_SHARD_CAP_PER_SESSION` | **192** | #6 + kod-gerçeği | min(192, MAX_PREPARAM_SHARDS=256) = 192 | tam (min) | K-4-alt-kapı: hold-fiziksel-kapasitesi (256) > cap (192) → sessiz-drop-dalı erişilemez (trivyal) | #6-modeli (ek-girdi yok) |
| 8 | `SESSION_COUNT_CAP` | **64** | N-session=32 | 32 × headroom-2 (kapasite-türetimi; §5) | headroom-sabit-2 | session-taban-state sınırı; ötesi typed-reject fail-closed | ~4 KiB/protector → 256 KiB (#0b-içi) |
| 9 | `TOMBSTONE_REGISTRY_CAP` | **64** | #8 | = SESSION_COUNT_CAP (worst: tümü closing) | tam (=) | K-1-pin: registry-dolu → fail-closed typed-diag | ~4.5 KiB/kayıt → 288 KiB (#0b-içi) |
| 10 | `CONTROL_DEDUP_WINDOW` | **4096 id — CANDIDATE [MÜHÜRLENMEZ]** | emisyon-kadansı + patience | Freeze-formülü (DUR-B düzeltme-3): WINDOW ≥ MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION × MAX_RETRY_REORDER_HORIZON + MAX_CONTROL_ID_BURST + MAX_CONTROL_REORDER_ALLOWANCE | YUKARI (fail-safe) | **HARD-issuance-bound kod/spec'te ENFORCE EDİLMİYOR** (20 id/s = gözlem-sınıfı, kanıtlı-max değil) → 4096 CANDIDATE kalır; mühür-önkoşulu = emisyon-yoluna MAX_ISSUE_RATE+MAX_BURST enforce-sabitleri (icra-talimatı maddesi; ACK/SACK-split + RECOVERED + TÜM logical-control üreticileri dahil) | bitmap 512 B/yön → 64 KiB (#0b-içi) |
| 11 | `NOTICE_BOUND` | **1** | K-2 | sabit | — | exact-once one-slot | tek-slot (SESSION_COLD_FIXED-payı) |
| 12 | `MAX_COLD_BLOCKS_PER_EPOCH` | **1** | K-2 | sabit | — | yalnız BootstrapColdBlock (alt-küme); ihlal = VIOLATION + DUR-B | sayı-invariantı |
| 13 | `MAX_ACTIVE_COLD_EPOCHS_PER_SESSION_DIRECTION` | **1** | adım-5-§4 | sabit | — | epoch-map yapısal-imkansız (HashMap YASAK; tek-CURRENT-slot); stale-proof → IGNORE + typed-diag | tek-CURRENT-slot |

Üçlü (11/12/13) BİRLİKTE donar, kod+test-pinlenir. **Mühür-kapsamı:** #0..#9 + #11-13 mühürlenebilir;
**#10 CANDIDATE** (önkoşul karşılanınca ayrı-mühür).

## 4. TUTARLILIK KONTROLLERİ (r2 — yeniden-hesaplandı)

```text
C-1  #5b = #6 = 192                                → bağlayıcı-eksen blok-şekli; bellek-kolu (316) gevşek
C-2  192 × 1536 = 294 912 B = 288 KiB ≤ #2 (480 KiB) → tam-doluluk byte-cap'e sığar
C-3  192×1536 + 8×512 + 1024 = 300 032 B ≈ 293 KiB ≤ 480 KiB (marj 191 488 B ≈ 187 KiB)
C-4  32 × #2 = 15 MiB = #1                          → eşit-pay/global tutarlı (min() dejenere-değil)
C-5  #7 = 192 < MAX_PREPARAM_SHARDS = 256           → hold-sessiz-drop erişilemez (K-4-alt-kapı)
C-6  (KALDIRILDI — Abkarada DUR-B: K-4'ün amacı numerik ">256" değil, TOTAL'in PreParam-sabitinden
     BAĞIMSIZ türetilmesidir; r2'de türetim bağımsız [bellek+şekil-kolları], sonuç 192 < 256.)
C-7  side-state: dedup 64 KiB + tombstone 288 KiB + session-taban 256 KiB = 608 KiB ≤ #0b (1 MiB)
C-8  #8 (64) × #2 (480 KiB) = 30 MiB > #1 (15 MiB)  → GLOBAL-eksen BAĞLAYICI; eşit-pay yalnız N≤32.
     ABKARADA-HÜKMÜ: "CLIENTRECEIVER C-8 RESIDUAL RISK: ACCEPTED FOR RESOURCE-BOUNDEDNESS ONLY."
     ZORUNLU-DIAGNOSTIC alanları (icra-talimatı maddesi):
       GlobalColdBudgetExceeded · SessionColdBudgetExceeded · ColdAdmissionRejectedByGlobalBudget ·
       active_sessions_at_global_exhaustion · distinct_authenticated_peers_at_global_exhaustion
     Per-peer/multi-federation-fairness + çok-hostile-session-availability DOĞRULANMIŞ-SAYILMAZ →
     HubReceiver rol-freeze'ine devir.
C-9  #1 + #0b = 15 + 1 = 16 MiB = #0                → plateau-toplam-invariantı yapısal-tutarlı
```

## 5. YUVARLAMA POLİTİKASI

Hard-security-cap'lerde **AŞAĞI / fail-closed** (bellek-kolu 316.6→316; bağlayıcı min-kolu tam-değer 192).
Kapasite uğruna budget YUKARI yuvarlanmaz. #10-YUKARI yalnız pencere (fail-safe; CANDIDATE-statüsünde).
Headroom-2 (#4/#8) yuvarlama değil kapasite-TÜRETİMİDİR; bellek-etkisi C-7/C-8'de ayrıca doğrulanır.

## 6. FREEZE-DIŞI KALANLAR

```text
CONTROL_DEDUP_WINDOW                 → CANDIDATE (mühür-önkoşulu: hard-issuance-bound enforce; §3-#10)
HubReceiver tüm-değerleri            → AYRI rol-kararı (+ C-8 fairness/availability devri)
RX_COLD_ABANDON profil-tablosu       → F0-(viii)-donmuş; profil-türevi (rol-değil)
T_ECHO_MIN(profile) / SESSION_IDLE_HOLD / TOMBSTONE_GRACE / PARAM_SET_RETRY → profil-türevi + ZM-2;
                                       icra-talimatı-fazında donar
```

## 7. PLATEAU ÜST-SINIRLARI (pre-registered; r2)

```text
BOUND_COLD_GLOBAL   = 15 MiB   : cold-defter-toplamı her-t'de ≤; defter-sayaç == model-tahmini-assert
BOUND_COLD_SESSION  = 480 KiB  : per-session defter her-t'de ≤
BOUND_SIDE_STATE    = 1 MiB    : dedup + tombstone + session-taban + pay (envelope-İÇİ rezerv)
TOPLAM-INVARIANT    : cold-retained + bounded-side-state ≤ 16 MiB  (her-t'de; RSS-ekseni + defter-ekseni)
Flood-kabulleri (v2-§7.1 F-A/F-B/F-C):
  F-A probe-flood 10^5   : echo-yolu working-set SABİT; cold-defter-delta=0; patience-beslenmez (P-2)
  F-B cold-blok-flood    : defter ≤ cap'ler; cap-ötesi typed-reject-sayacı > 0; C-8-diag-alanları raporlanır;
                           RSS-plateau (eğim≈0 @ t∈[T/2,T])
  F-C control-flood 10^5 : dedup-bitmap sabit-512B/yön (büyüme=0); tombstone ≤ 64;
                           ControlIdBelowDedupWindow sayacı raporlanır
Süre/tekrar-parametreleri icra-talimatı pre-registration'ında donar (bound'lar değişmez).
```

## 8. MODEL-DOĞRULAMA KAPISI

A2-F1-kabulüne ZORUNLU: memory-model-testi gerçek `SHARD_RETAINED_COST(1024)` ölçer (allocator-farkındalıklı).
```text
ölçülen ≤ 1536 B  → model GEÇERLİ; cap'ler AYNEN
ölçülen > 1536 B  → DUR-B: bellek-kolu AŞAĞI yeniden-türetilir (bağlayıcı-eksen blok-şekli olduğundan
                    #6=192 ancak bellek-kolu < 192'ye düşerse değişir). GEVŞEME YÖNÜ YOK.
```

## 9. r2-BÜTÜNLÜK DAMGASI

**Kanonik hash-prosedürü (KESİN; verify-düzeltmesi — r1-prosedürü belirsizdi):** UTF-8/LF dosyanın TAMAMI,
yalnız `^sha256: ` ile başlayan satır silinerek, sondaki newline dahil:
```text
grep -v '^sha256: ' <bu-dosya> | sha256sum
```
Damga-sırası: (1) tarih-satırı DOLDURULUR → (2) hash yukarıdaki komutla hesaplanır → (3) sha256-satırına
işlenir. (r1-hatası: hash tarih-doldurulmadan hesaplanmıştı → committed-gövdeden yeniden-üretilemez.)

**r1-arşiv-şerhi (dürüst-kayıt):** r1-metin-damgası `658e75d1…` committed-gövdeden YENİDEN-ÜRETİLEMEZ
(yukarıdaki sıra-hatası; sayısal-içerik bağımsız-doğrulandı, etkilenmedi). Doğrulanabilir r1-çapası:
git-blob `cc6ac3acc7943d5a14a5ca452df99b597581df90` (= `git rev-parse
5c9ca4a:reports/decision-requests/DR-CS-1b-A2-CAL-freeze-clientreceiver.md`). r1 DUR-B-düzeltmeleriyle
SUPERSEDED.

```text
sha256: 5b298244722eafe12611d027cae1c980763e69c744ee01562c69f10a59738318
tarih : 2026-07-27T20:52:39Z
```
