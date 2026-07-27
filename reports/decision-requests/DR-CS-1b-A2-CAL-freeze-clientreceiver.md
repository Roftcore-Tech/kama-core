# DR-CS-1b / A2 — [CAL] TÜRETİM + SAYISAL FREEZE — ClientReceiver v1 [TASLAK — DUR-A]

**Statü:** K-6 ADIM-6 taslağı. Abkarada adım-5 girdileriyle yetkilendirildi ("[CAL] taslağını ve bütün
türetim tablosunu rapor et; henüz production implementasyona başlama" — DUR-A). **Bu belge Abkarada
onayıyla MÜHÜRLENİR**; onay öncesi taslak-bütünlük damgası §9'da. Koşu-sonucu görülerek HİÇBİR değer
GEVŞETİLEMEZ; model-doğrulama yalnız SIKILAŞTIRABİLİR.

**Rol-kapsamı:** `TARGET_DEPLOYMENT_ROLE = ClientReceiver`. Bu değerler **HubReceiver'a TAŞINMAZ**;
Test/CI değerleri production-sabiti SAYILMAZ (Abkarada adım-5 normatif-yorumu).

---

## 1. GİRDİLER (Abkarada, K-6 adım-5 — AYNEN)

```text
PROCESS_MEMORY_BUDGET                        = 256 MiB   (A2 kaynak-planlama envelope'u; OS-garantisi değil)
EXPECTED_CONCURRENT_AUTHENTICATED_SESSIONS   = 32        (kapasite-planlama; güvenlik-sınırı DEĞİL)
MAXIMUM_COLD_STATE_MEMORY_PERCENTAGE         = 6.25%
EXPECTED_CONCURRENT_RX_COLD_BLOCKS_PER_SESSION = 4       (genel RxColdLedgerBlock planlama girdisi)
→ GLOBAL_COLD_MEMORY_BUDGET = 256 MiB × 6.25% = 16 MiB   (HARD process-wide security cap)
```

Kod/spec-gerçekleri (türetimin ikinci girdi-seti; bu turda doğrulandı):
```text
PROFILES-allowlist (kama-fec-codec/src/profile.rs): max(K_shape + R_code_max) = 16+8 = 24 (0x04/0x06)
                                                    max E = 1024 (0x05/0x06; sealed 26+11+1024 = 1061B)
MAX_PREPARAM_SHARDS = 256 (kama-engine/src/preparam_hold.rs:13 — fiziksel hold-kapasitesi)
recv-yön ReplayWindow = 2 KiB/epoch-yönü (pn.rs: 2^14/64 × u64)
K-2 alt-küme-modeli ONAYLI: BootstrapColdBlock ⊂ RxColdLedgerBlock
```

## 2. RETAINED-MEMORY MODELİ (yalnız-payload-hesabı YASAK — Abkarada şartı)

Model-sabitleri (KONSERVATİF; §8 model-doğrulama kapısıyla ölçüme bağlanır — yalnız SIKILAŞIR):

| Model-sabiti | Değer | İçerik |
|---|---|---|
| `SHARD_RETAINED_COST(E=1024)` | **1536 B** | symbol-buffer allocator-sınıfı 1024×1.25=1280 (Vec-capacity + malloc-chunk-header + size-class-yuvarlaması) + `H_SHARD=256` (OpenedTransportShard DTO ≈48 + LogicalShardHeader ≈16 + hold-VecDeque-slot ≈48 + per-shard (block_id,esi)-dedup-girdisi ≈48 + pay) |
| `SHARD_RETAINED_COST(E=64)` | 336 B | 64×1.25=80 + 256 (bilgi-amaçlı; CAP'ler worst-case E=1024'ten türetilir) |
| `BLOCK_META_COST` | 512 B | blok-girdisi + ESI-set-başlığı + deadline-slotu + progress-sayaçları + profil-ref |
| `SESSION_COLD_FIXED` | 1024 B | ParamState(~40) + defter-girdisi + deadline-çarkı + ledger-sayaçları + pay |

Kapsam-notu: ControlDedupWindow-bitmap (512 B/yön) ve tombstone (~4.5 KiB/kayıt: recv-ReplayWindow 2-epoch
4 KiB + recv-cipher + ~51B cached-ACK + girdi) COLD-defterine DEĞİL, plateau-global-bound'una (§7) sayılır
(session-lifecycle-state; cold-state değil). Decoder-scratch cold-RETENTION'a girmez (decode-anlık;
reusable-bounded-pool fec-core-§9) — plateau-ölçümü yine de yakalar (RSS-ekseni).

## 3. TÜRETİM TABLOSU (her değer: kaynak-girdi · formül · yuvarlama-yönü · güvenlik-gerekçesi · model)

| # | Sabit | DEĞER | Kaynak-girdi | Türetim-formülü | Yuvarlama | Güvenlik-gerekçesi | Retained-memory modeli |
|---|---|---|---|---|---|---|---|
| 1 | `GLOBAL_COLD_MEMORY_BUDGET` | **16 MiB** | PROCESS_MEMORY_BUDGET × COLD-% | 256 MiB × 6.25% | tam (=) | HARD process-wide cap; tüm cold-rezervasyonların birincil sınırı | yüzde-tanımı (model-girdisi yok; tüketiciler #2/#3/#6) |
| 2 | `RX_COLD_RETAINED_BYTE_CAP_PER_SESSION` | **512 KiB** (524 288 B) | #1 + N-session | 16 MiB / 32 (eşit-pay) | tam (=) | fail-closed eşit-pay **N≤32 zarfında** (C-8: N>32'de global-eksen bağlayıcı); tek-session tüm-bütçeyi işgal edemez | §2-sabitleriyle C-3-tavanı: 496 640 B ≤ cap |
| 3 | `GLOBAL_COLD_BYTE_CAP` | **16 MiB** | #1, #2 | min(16 MiB, 32 × 512 KiB) = min(16,16) | tam (=) | Abkarada min()-formu (K-5/adım-5); defter GLOBAL-eksende önce-dolan-reddeder | #2-modelinin N-katı; C-8-ekseninde bağlayıcı-sınır |
| 4 | `RX_COLD_OPEN_BLOCK_CAP_PER_SESSION` | **8** | EXPECTED_COLD_BLOCKS=4 | 4 × headroom-2 (kapasite-türetimi; §5-notu) | headroom-sabit-2 (büyütülmez) | RxColdLedgerBlock eşzamanlılık-sınırı; reorder/burst-toleransı; ötesi typed-reject | BLOCK_META_COST=512 B/blok → #6-rezervi 8×512=4 096 B |
| 5 | `RX_COLD_SHARDS_PER_BLOCK_CAP` | **24** | PROFILES-allowlist | max(K_shape + R_code_max) = 16+8 | tam (=; allowlist-türevi) | geçerli-ESI-uzayı zaten ≤ K+R; cap bunun defter-aynası — allowlist-dışı shard-kimliği rezerv alamaz | blok-başı worst-shard-yükü 24×1536 = 36 864 B ≈ 36 KiB (C-2-girdisi) |
| 6 | `RX_COLD_TOTAL_SHARD_CAP_PER_SESSION` | **320** | #2 + model | floor((524 288 − SESSION_COLD_FIXED − #4×BLOCK_META) / SHARD_COST(1024)) = 519 168/1536 = **338 (tam-bölünme)** → **320** | **AŞAĞI** (338→320; %5.3 model-hata-payı) | bellek-bütçesi-türevi (Abkarada-hiyerarşisi); worst-case-E türetimi = küçük-E'de otomatik-fazla-güvenli | **birincil model-tüketicisi:** SHARD_RETAINED_COST(1024)=1536 B (§2; §8-ölçüm-kapılı) |
| 7 | `RX_PREPARAM_SHARD_CAP_PER_SESSION` | **256** | #6 + kod-gerçeği | min(320, MAX_PREPARAM_SHARDS=256) | tam (min) | K-4/adım-5-teyit: PreParamHold fiziksel-admission-alt-kapısı; hold-sessiz-drop-dalı ölü-kod-pinli | #6-modeli + kod-sabiti (min; ek-model-girdisi yok) |
| 8 | `SESSION_COUNT_CAP` | **64** | N-session=32 | 32 × headroom-2 (kapasite-türetimi; §5-notu) | headroom-sabit-2 | session-taban-state'i sınırlar; ötesi typed-reject fail-closed; cold-bütçeyi #1/#3 korur (C-8) | session-taban ~4 KiB/protector → 64×4 KiB = 256 KiB (yan-state; §7-BOUND_SIDE) |
| 9 | `TOMBSTONE_REGISTRY_CAP` | **64** | #8 | = SESSION_COUNT_CAP (worst-case: tümü aynı-anda closing) | tam (=) | K-1-pin "registry-cap-dolu → fail-closed typed-diag" | ~4.5 KiB/kayıt (§2-kapsam-notu) → 288 KiB (§7-BOUND_SIDE) |
| 10 | `CONTROL_DEDUP_WINDOW` | **4096 id** (bitmap 512 B/yön) | emisyon-kadansı + patience | ~20 id/s × 105 s = 2100 → next-pow2 | **YUKARI** (fail-safe: büyük-pencere = az yanlış-duplicate; maliyet sabit-bounded) | A-3-boyut-koşulu (retry-horizon + reorder + burst-id-tüketimi); `ControlIdBelowDedupWindow` varsayım-ihlalini görünür kılar | bitmap sabit 512 B/yön → 64×2×512 = 64 KiB (§7-BOUND_SIDE) |
| 11 | `NOTICE_BOUND` | **1** | K-2 | sabit | — | exact-once one-slot (Abkarada-onaylı) | tek-slot; SESSION_COLD_FIXED-payı içinde |
| 12 | `MAX_COLD_BLOCKS_PER_EPOCH` | **1** | K-2 | sabit | — | yalnız BootstrapColdBlock-sınıfı (alt-küme); ihlal = INVARIANT VIOLATION + DUR-B | sayı-invariantı (state-modeli #11-slotu) |
| 13 | `MAX_ACTIVE_COLD_EPOCHS_PER_SESSION_DIRECTION` | **1** | adım-5-§4 | sabit | — | epoch-map yapısal-imkansız (HashMap<cold_epoch,·> YASAK; tek-CURRENT-slot); stale-proof → IGNORE + typed-diag | tek-CURRENT-slot; SESSION_COLD_FIXED-payı içinde |

Üçlü donma-çifti (11/12/13) BİRLİKTE donar ve kod+test-pinlenir (Abkarada adım-5-§4).

## 4. TUTARLILIK KONTROLLERİ (türetim-içi; freeze-öncesi doğrulandı)

```text
C-1  #4 × #5 = 8×24 = 192 ≤ #6 (320)             → blok-şekilli-doluluk shard-cap'e sığar (bağımsız
                                                    eksenler; ilk-dolan-reddeder — çelişki değil)
C-2  192 shard × 1536 B = 288 KiB ≤ #2 (512 KiB)  → blok-şekilli-doluluk byte-cap'e sığar
C-3  #6 × 1536 + #4×512 + 1024 = 491 520 + 4 096 + 1 024 = 496 640 B = **485 KiB** ≤ 512 KiB
     → model-tavanı per-session-cap içinde (marj 27 648 B = 27 KiB)
C-4  32 × #2 = 16 MiB = #1                        → eşit-pay/global tutarlı (min()-formu dejenere-değil)
C-5  #7 = 256 ≤ MAX_PREPARAM_SHARDS = 256         → hold-sessiz-drop erişilemez (K-4)
C-6  #6 (320) > 256                               → K-4-amacı gerçekleşti: TOTAL 256'ya kilitlenmedi;
                                                    post-PARAM cold-state 256-üstü bütçe kullanabilir
C-7  yan-state: dedup 64-sess×2-yön×512B = 64 KiB + tombstone 288 KiB + session-taban 256 KiB
     → cold-bütçe DIŞI, plateau-global-bound İÇİ (§7)
C-8  #8 (64) × #2 (512 KiB) = 32 MiB > #1 (16 MiB) → GLOBAL-eksen BAĞLAYICI: eşit-pay garantisi yalnız
     N≤32 zarfında; N>32'de global-defter önce-dolan-reddeder (fail-closed; bounded KALIR). Dürüst-sınır:
     32 hostile-session 16 MiB'ı doldurursa 33.-64. meşru-oturumlar cold-budget'sız kalır (typed-reject;
     servis-aşındırma ekseni) — per-peer-fairness politikası HubReceiver-rol-kararına devredilir
     (ClientReceiver'da peer-sayısı düşük; kabul-edilen-artık-risk).
```

## 5. YUVARLAMA POLİTİKASI (Abkarada adım-5-§6 aynen)

Hard-security-cap'lerde yuvarlama **AŞAĞI / fail-closed** (örn. #6: 338→320). Kapasite hedefi uğruna
memory-budget YUKARI yuvarlanmaz. Tek YUKARI-yuvarlama #10-pencere (gerekçe: büyük-pencere fail-safe
yönlüdür ve bellek-maliyeti sabit-bounded'dır; yanlış yön = pencereyi küçültmek).
**Headroom-2 ayrımı (verify-netleştirmesi):** #4/#8'deki ×2-headroom YUVARLAMA DEĞİL, kapasite-TÜRETİMİDİR
(planlama-girdisi × sabit-katsayı); yuvarlama-politikası türetim-SONRASI sayıya uygulanır. Bu türetimlerin
bellek-etkisi C-7/C-8'de bütçeye karşı ayrıca doğrulanmıştır.

## 6. FREEZE-DIŞI KALANLAR (bu belge donDURMAZ)

```text
HubReceiver tüm-değerleri            → AYRI rol-kararı (Abkarada adım-5: taşınmaz)
RX_COLD_ABANDON profil-tablosu       → F0-(viii)-donmuş formül; rol-değil profil-türevi (v2-§9 şartları)
T_ECHO_MIN(profile) / SESSION_IDLE_HOLD / TOMBSTONE_GRACE / PARAM_SET_RETRY → profil-türevi + ZM-2
                                       4-decade; icra-talimatı-fazında donar
```

## 7. PLATEAU ÜST-SINIRLARI (pre-registered; koşu-öncesi)

```text
BOUND_COLD_GLOBAL   = 16 MiB   : cold-defter-toplamı her-t'de ≤; ayrıca defter-sayaç == model-tahmini-assert
BOUND_COLD_SESSION  = 512 KiB  : per-session defter her-t'de ≤
BOUND_SIDE_STATE    = 1 MiB    : dedup(64 KiB) + tombstone(288 KiB) + session-taban(256 KiB) + pay
                                 → toplam-yan-state tavanı (RSS-ekseninde cold-bound'a eklenir)
Flood-kabulleri (v2-§7.1 F-A/F-B/F-C):
  F-A probe-flood 10^5   : echo-yolu working-set SABİT; cold-defter-delta=0; patience-beslenmez (P-2)
  F-B cold-blok-flood    : defter ≤ cap'ler; cap-ötesi typed-reject-sayacı > 0; RSS-plateau (eğim≈0 @ t∈[T/2,T])
  F-C control-flood 10^5 : dedup-bitmap sabit-512B/yön (büyüme=0); tombstone ≤ 64; ControlIdBelowDedupWindow
                           sayacı raporlanır
Süre/tekrar-parametreleri icra-talimatı pre-registration'ında donar (bu bound'lar değişmez).
```

## 8. MODEL-DOĞRULAMA KAPISI (freeze-bütünlüğünün ölçüm-bağı)

A2-F1-kabulüne ZORUNLU madde: memory-model-testi gerçek `SHARD_RETAINED_COST(1024)`'ü ölçer
(allocator-farkındalıklı; Vec-capacity + gerçek-DTO-boyutları).
```text
ölçülen ≤ 1536 B  → model GEÇERLİ; cap'ler AYNEN
ölçülen > 1536 B  → DUR-B: cap'ler AŞAĞI yeniden-türetilir (formüller sabit; yalnız model-sabiti büyür,
                    #6 küçülür). GEVŞETME YÖNÜ YOK.
```

## 9. TASLAK-BÜTÜNLÜK DAMGASI

Bu taslağın sha256'sı ve zaman-damgası commit-mesajı + aşağıya işlenir; **Abkarada-onayında bu içerik
üzerinden FREEZE mühürlenir** (onay-sonrası değişiklik = yeni-versiyon + yeniden-onay).
```text
sha256: 658e75d15fad92332cdc8dd4558bdae82be5920799ba6b4acc0db533e692fdd3  (§9-sha256-satırı hariç gövde)
tarih : 2026-07-27T20:03:28Z
```
