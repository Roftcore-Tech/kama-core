# DR-CS-1b / A2 — v2 KARAR-AMENDMANI [Abkarada-onaylı; BAĞLAYICI]

**Statü:** Abkarada v2-inceleme hükmü (2026-07-27):
> **"A2 TASARIM v2: GO — TASARIM ONAYLANDI. Yeni bir v3 gövdesi gerekmiyor."** Aşağıdaki kararlar dar
> v2 karar-amendmanı olarak eklenir. **"A2 PRODUCTION IMPLEMENTATION: HÂLÂ NO-GO"** — A-1/A-2/A-3
> spec-amendmanları, G-2-tabanlı [CAL]-freeze, mutlak değişiklik-yüzeyi ve faz-bazlı icra-talimatı
> tamamlanmadan üretim koduna dokunulmaz.

Bu belge `dr-cs-1b-a2-receiver-design-v2.md`'nin ÜZERİNE bağlayıcıdır (amendman > gövde; v2-gövdesi
yeniden yazılmaz). Çelişkide BU BELGE kazanır.

---

## K-1 — TOMBSTONE: Model A+B HİBRİT **ONAY** + ek acceptance-pinleri

v2-§4 hibrit sırası (Closing → precompute-sealed-CLOSE-ACK-send-cipher-canlıyken → ağır-state+send-state
zeroize → minimal-tombstone {route + recv-cipher + recv-replay-window + cached-sealed-ACK + grace-deadline}
→ grace-sonu unregister/remove) ONAYLANDI. Gerekçe-pin: retry'ın gerçekten authenticated SESSION_CLOSE
olduğunu anlamak için grace boyunca receive-open yeteneği ZORUNLU; kriptografik doğrulama olmadan
cached-cevap = reflection yüzeyi.

**Tombstone acceptance'a EKLENEN pinler (Abkarada; aynen):**
```text
unauthenticated datagram → cached response YOK
authenticated CLOSE retry → aynı cached ciphertext (byte-for-byte; her retry'da RE-SEAL EDİLMEZ)
yeni cold allocation → typed reject
send cipher grace boyunca YAŞAMAZ
recv cipher yalnız grace için yaşar
grace expiry → route + recv state + cached ACK tamamen düşer
registry cap dolu → fail-closed typed diagnostic
```

## K-2 — NOTICE: per-cold-epoch **ONE-SLOT ONAY** — eş-invariant ŞARTIYLA

Koşul: **"Bir cold epoch en fazla bir cold/bootstrap block sahibi olabilir"** invariant'ı kod+test ile
pinlenecek. Donma-çifti (birlikte donar):
```text
NOTICE_BOUND = 1
MAX_COLD_BLOCKS_PER_EPOCH = 1
```

**Slot-semantiği (Abkarada; aynen):**
```text
slot empty + first valid proof        → notice yaz
slot full  + duplicate/same block     → idempotent no-op
slot full  + FARKLI block             → INVARIANT VIOLATION + DUR-B
```
"Farklı-blok-notice'i sessiz düşürüldü" NORMAL-davranış DEĞİLDİR — böyle bir olay tek-block-per-epoch
varsayımının yanlışlığını gösterir ve bounded-queue kararını yeniden açtırır. (v2-§6'nın "farklı-bloğun-
notice'inin-düşmesi one-slot-bound-SONUCUDUR / pratik-çakışma-beklenmez" dürüst-sınır cümlesi bu kararla
**SUPERSEDE** — düşme değil, violation+DUR-B.) Notice'in terminal-olmadığı, first-valid-proof-wins ve
stale/duplicate-ignore sözleşmesi (v2-§6) aynen geçerli.

**AÇIK-TANIM ŞERHİ (verify-bulgusu; K-6-adım-5/6'dan ÖNCE Abkarada-tanımı gerekli):** K-2'deki
"cold/bootstrap block" sınıfının §3 RX_COLD_*-defter-bloklarıyla ilişkisi üç belgenin hiçbirinde tanımlı
değil. İki okuma: (a) **alt-küme** — MAX_COLD_BLOCKS_PER_EPOCH=1 yalnız bootstrap-epoch'unun cold-bloğunu
sınırlar, defter genel pre-PARAM/cold blokları yönetmeye devam eder (K-5 "expected concurrent cold
blocks/session" girdisi ve OPEN_BLOCK_CAP-türetimi anlamlı kalır); (b) **özdeş** — invariant eşzamanlı-
cold-blok sayısını yapısal-1'e indirir, K-5-dördüncü-girdisi ve OPEN_BLOCK_CAP-türetimi boşalır.
Pinlenecek blok-sınıfı [CAL]-freeze'den önce netleşmeli; tasarımın ÖNERİSİ (a)-alt-küme okumasıdır
(onay bekler — G-2 sorusuyla birlikte).

## K-3 — A-3 (`control_id` MONOTONLUĞU): **İSİMLİ SPEC-AMENDMANI ONAYLANDI**

Kod-seviyesi gelenek olarak bırakılamaz — replay/dedup semantiğini, reordered-fresh ayrımını, peer-
interoperability'yi ve window-boyutlandırmasını belirleyen PROTOKOL KONTRATIDIR.
Hedef: noise-wire §2.57. Amendman-metni (Abkarada; aynen):
```text
control_id MUST be monotonically allocated per session and direction.
Retries MUST retain the original control_id.
A control_id MUST NOT be reused within the same session/direction.
```
Wrap-semantiği: **control_id wrap olmadan önce session renewal zorunlu**; bu turda modular-wrap
karşılaştırması EKLENMEZ (Abkarada; gerekçe-şerhi editoryal: wrap'a ulaşmadan renewal daha sade).

**Ek teşhis (Abkarada):** fixed-window altına düşen ID için yalnız `DuplicateAckOnly` yazmak yetmez —
ayrıca `diag = ControlIdBelowDedupWindow` çıkarılır; window-sizing varsayımı bozulursa sessizce
"duplicate" sanılmaz (v2-§5 reordered-fresh tespitinin teşhis-kancası).

## K-4 — PREPARAM KAPASİTESİ: **REVİZE** — `RX_PREPARAM_SHARD_CAP` alt-kapısı

v2-§3'ün `TOTAL_SHARD_CAP ≤ MAX_PREPARAM_SHARDS` global-bağı FAZLA-KISITLAYICI (tüm cold-state'i
PreParamHold kapasitesine kilitler; post-PARAM açık-bloklar gereksiz 256-sınırına girer). Revize:

```text
RX_PREPARAM_SHARD_CAP_PER_SESSION = min( RX_COLD_TOTAL_SHARD_CAP_PER_SESSION,
                                         MAX_PREPARAM_SHARDS )

Pre-PARAM admission İKİ cap'i birden kontrol eder:
  preparam_shards   < RX_PREPARAM_SHARD_CAP
  total_cold_shards < RX_COLD_TOTAL_SHARD_CAP
```

Bu **beşinci global kaynak-ekseni DEĞİL** — PreParamHold'un FİZİKSEL admission ALT-KAPISIDIR; dört-cap
hiyerarşisi (v2-§3) korunur. Sonuçlar: PreParamHold sessiz-drop dalına ulaşılamaz · TOTAL_SHARD_CAP
256'ya kilitlenmez · post-PARAM state'e ileride daha yüksek bütçe verilebilir. `MAX_PREPARAM_SHARDS`
artırılacaksa engine-fiziksel-kapasitesi ve bu preparam-cap AYNI isimli dilimde birlikte yükseltilir.
(v2-§3'ün MAX_PREPARAM-invariant-BLOĞUNUN TAMAMI bu formla SUPERSEDE edilir — "TOTAL_SHARD_CAP ≤
hold-kapasitesi" cümlesi VE "cap 256-üstünde donacaksa eş-yükseltilir" tetik-cümlesi dahil; K-4-rejiminde
TOTAL cap 256-üstüne MAX_PREPARAM-yükseltmesiz çıkabilir, 257.-shard'ı hold'dan min()-alt-kapısı keser;
eş-yükseltme-kuralı yalnız MAX_PREPARAM_SHARDS-artışına bağlıdır.)

## K-5 — G-2: girdiler **deployment role/tier BAŞINA**

Numerik-freeze bloke kalır; değerler tek evrensel ürün-sabiti olarak SEÇİLMEZ. Roller:
```text
Client receiver · Hub receiver · Test/CI receiver
```
İlk implementasyonun hedeflediği role için asgari girdiler:
```text
process memory budget
expected concurrent authenticated sessions
maximum cold-state memory percentage
expected concurrent cold blocks/session
```
Güvenlik sınırı = `GLOBAL_COLD_MEMORY_BUDGET`; session-hedefi yalnız kapasite-planlamasıdır.
**Sayılar gelmeden DONAMAZ:** per-session retained-byte cap · total shard cap · open block cap ·
global byte cap · session count cap · tombstone registry cap · plateau bound.

## K-6 — SÜREÇ SIRASI (donduruldu; Abkarada aynen)

```text
1. v2 karar-amendmanı            (BU BELGE)
2. A-1 spec amendmanı            (noise-wire §8 — aşım-politikası)
3. A-2 spec amendmanı            (noise-wire §2.57 — probe/echo dedup-bypass notu)
4. A-3 spec amendmanı            (noise-wire §2.57 — control_id monotonluk)
5. G-2 deployment girdileri      (Abkarada)
6. [CAL] formülleri + sayısal freeze
7. MUTLAK-SINIR: izin verilen crate/dosya yüzeyi
8. faz-bazlı production icra talimatı
9. implementation
10. pre-registered plateau + product receiver matrisi
```

## K-8 — NOTICE SAHİPLİĞİ DÜZELTMESİ (Abkarada adım-7/8-kararı; BAĞLAYICI)

`PostTimeoutNotice` **SENDER-tarafı state'idir**:
```text
kama-engine::ColdRxState : notice-slot TUTMAZ · PostTimeoutNotice TUTMAZ · sender-cold_epoch-state TUTMAZ
adaptive-transport::Sender: current-cold_epoch · BootstrapColdBlock-identity · PostTimeoutNotice-one-slot ·
                            NOTICE_BOUND=1 · MAX_COLD_BLOCKS_PER_EPOCH=1 ·
                            MAX_ACTIVE_COLD_EPOCHS_PER_SESSION_DIRECTION=1
```
State YALNIZ F4.2'de, `sender.rs` DUR-B-3 diliminde doğar. ReceiverHost yalnız RECOVERED-kontrol-emisyonunu
yönetir; sender-application-notice-state'ini SAHİPLENMEZ. (MUTLAK-SINIR-§2.2 ve talimat-F0.3'teki
"notice-slot (1/1/1)" ifadeleri kaldırıldı.)

## K-9 — GLOBAL LEDGER SAHİPLİĞİ DÜZELTMESİ (Abkarada adım-7/8-kararı; BAĞLAYICI)

```text
kama-engine::ColdRxState      : SESSION-LOCAL admission/resource state
kama-integration::ReceiverHost: PROCESS-WIDE/GLOBAL resource state SAHİBİ:
  GLOBAL_COLD_RETAINED_MEMORY_BUDGET · GLOBAL_COLD_BYTE_CAP · SESSION_COUNT_CAP ·
  TOMBSTONE_REGISTRY_CAP · global-reservation-ledger
İki-seviyeli admission ATOMİK: global-reserve + session-reserve → ikisi-de-başarılıysa commit →
herhangi-biri-reddederse HİÇBİR ledger değişmez.
Engine SAF ledger-TİPLERİNİ sağlayabilir; global-bütçenin LIFECYCLE-sahibi OLAMAZ.
```

## K-10 — ADIM-7/8 DİĞER KARAR-KAYITLARI (Abkarada; aynen)

```text
MUTLAK-SINIR v1               : CONDITIONAL GO   (K-8/K-9-işlenince)
A-4 protector-amendmanı       : GO               (7-normatif-postcondition; retired-state YAPISAL;
                                                  spec-A-4 §0.9-sonuna işlendi)
Faz-bazlı icra-talimatı v1    : CONDITIONAL GO   (K-8/K-9-işlenince)
F-CID-0 / RECOVERED           : NORMATİF-BOUND + F4.5 ONAYLI (F4.1 öne-ALINMAZ; CAL-10 konservatif:
                                RECOVERED-rate/burst ≤ pre-registered-SACK-return-supply-allocation;
                                F4.1'de merkezi ControlIdIssueGate'e bağlanır; yeni-logical=yeni-id+
                                rate/burst-tüketimi, retry=aynı-id-rate-tüketmez-ama-horizon-hesabında)
PARAM_SET_RETRY               : DUR-B-3-DIŞI; varsayılan A2-production-kapsamı-DIŞI. F-CID-0 yalnız
                                READ-ONLY-envanter (cadence/deadline/max-retry/new-id/retry-same-id/
                                hard-horizon); hard-bound-varsa değer-CAL-10'a; yoksa/patience-uyumsuzsa
                                → isimli "DUR-B-PARAM — PARAM_SET RETRY COORDINATION" (param_state.rs/
                                param_bootstrap.rs sessizce AÇILMAZ)
SONRAKİ-KAPI                  : bu-kararlar 4-belgeye işlenip commit → A2-F0 PRODUCTION GO;
                                sonraki-fazlar DUR-A/★onay/DUR-B kapılarıyla; merge+default-flip yasak
```

## K-7 — NİHAİ DURUM TABLOSU (Abkarada; aynen)

```text
A2 design v1               : ARCHIVED / MUST-NOT-IMPLEMENT in revised sections
A2 design v2               : GO — APPROVED WITH DECISION AMENDMENT
Tombstone hybrid           : GO
Notice one-slot            : GO — only with one-block-per-cold-epoch invariant
A-3 monotonic control_id   : GO — REQUIRED NAMED SPEC AMENDMENT
PreParam capacity          : REVISE — add RX_PREPARAM_SHARD_CAP sub-gate
G-2 numeric values         : OPEN — freeze blocker only
Production implementation  : NO-GO
A1                         : SEALED — remains closed
```

---

**v2-gövdesine etkisi (amendman > gövde; yeniden-yazım YOK):** §3 MAX_PREPARAM-invariant-BLOĞU (eş-yükseltme-
tetik-cümlesi dahil) → K-4 supersede · §4 acceptance-pinleri → K-1 genişletir · §6 slot-semantiği → K-2
(farklı-blok = DUR-B; **§6'nın "farklı-blok-düşmesi=one-slot-bound-sonucu/pratik-çakışma-beklenmez"
dürüst-sınır-cümlesi SUPERSEDE**) + MAX_COLD_BLOCKS_PER_EPOCH-invariantı eklenir · §10 A-3 "ADAY" → K-3
ONAYLI (üç-amendman) · §9 [CAL]-tablosu → K-4-satırı (RX_PREPARAM_SHARD_CAP) + K-2-çifti
(NOTICE_BOUND/MAX_COLD_BLOCKS) eklenir, K-5 rol-başına-girdi şerhi düşülür.
