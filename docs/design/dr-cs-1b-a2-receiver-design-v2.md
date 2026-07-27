# DR-CS-1b / A2 — ÜRÜN-ALICI PATIENCE + RESOURCE — TASARIM v2 [6-BLOCKER REVİZYONU; ONAY BEKLİYOR]

**Statü:** v1-incelemesinin (Abkarada) hükmü uyarınca hazırlanan revizyon — icra-talimatı ÖNCESİ son tasarım
turu. **Üretim-kodu değişikliği: SIFIR** (docs-only). v1 (`dr-cs-1b-a2-receiver-design-v1.md`) ARŞİV
statüsünde üst-notla korunur; **değişmeyen bölümleri referansla yürürlüktedir** — bu belge yalnız kalan
deltayı işler (roadmap §7 tasarım-döngüsü kuralı).
**Doğrulama:** v2, 4-mercekli adversarial-verify turundan geçirildi (wf_19addf9b: karar-sadakat /
cap-matematik / spec-kod-uyum / v1↔v2-iç-tutarlılık; 4 REFUTED + 14 WEAKENED + 12 HOLDS — TÜMÜ işlendi:
v1-üst-not süpersesyon-listesi genişletildi; eski-ad→yeni-eksen eşleme-tablosu + fec-core-§13-yeniden-
hizalama + MAX_PREPARAM_SHARDS-invariantı §3'e; iade-sırası adım-atamaları (2a/2b) + PersistentReplayGuard-
(5) + 51B-düzeltmesi §4'e; window-boyut-koşullu "yeni-iş-kaybı" + reordered-fresh sınıfı §5'e; Abkarada-
listesi-aslına-sadık + one-slot-öneri-ayrımı + release-taraf-düzeltmesi §6'ya; REPLAY_PAIR_CAP/RECOVERED-
bütçe/plateau-şerhi §9'a; A-3-aday-ayrımı + statü-beyanı §10'a).

> **Devralınan bağlayıcı çerçeve:** A1 SEALED; 1200B-yasak-iddialar geçerli; "ARQ-vs-FEC" dili yasak;
> üretim koduna bağlayıcı-talimat + pre-registration-freeze tamamlanmadan dokunulmaz.

---

## 0. ABKARADA v1-KARAR KAYDI (bağlayıcı; bu belgenin varlık nedeni)

> **"A2 DESIGN DIRECTION: GO. A2 PRODUCTION IMPLEMENTATION v1: NO-GO — 6 bağlayıcı düzeltme sonrası v2
> gerekli."** · Son-hüküm: *"A2 DESIGN v1: STRONG FOUNDATION — CONDITIONAL GO FOR v2. A2 PRODUCTION
> IMPLEMENTATION: NO-GO UNTIL v2 CLOSES THE SIX BLOCKERS."*

| Karar | Hüküm (Abkarada) | v2-bölümü |
|---|---|---|
| D-1 composition root | **REVİZE:** integration host + engine pure state | §1 |
| D-2 patience | **GO:** absolute, non-renewable deadline | §2 |
| D-3 typed reject | **GO:** isimli spec-amendmanı + sessiz yol erişilemez | §3 (cap-matematiği REVİZE) |
| D-4 lifecycle owner | **GO:** integration owner; tombstone sırası revize | §4 |
| D-5 stateless probe/echo | **GO:** fakat genel dedup fixed-window ZORUNLU | §5 |
| D-6 post-timeout notice | **CONDITIONAL GO:** exact-once/bounded identity sözleşmesi | §6 |

G-durumları: **G-1 ÇÖZÜLDÜ** (on-bir madde teslim edildi → §8 aynen + fark-taraması) · **G-2** = yalnız
NUMERİK-freeze-blocker'ı (mimari-blocker değil; gereken 4 girdi §9'da) · **G-3** = blocker DEĞİL
(dependency-note olarak kalır; A2 epizot-alanına dokunmaz — v1-§2.5 sınırı yürürlükte).

En kritik iki v1-hatası (Abkarada): **session-scoped HashSet'in hâlâ unbounded olması** (→ §5) ve
**block/shard/byte cap matematiğinin tutarsızlığı** (→ §3). Bunlar kapanmadan memory-plateau iddiası
kurulamazdı.

---

## 1. BLOCKER-1 — COMPOSITION ROOT: `kama-integration::ReceiverHost` + `kama-engine::ColdRxState`

v1'in birincil önerisi (`kama-engine::ColdRxHost`) REDDEDİLDİ: engine gerçek composition-root olursa
noise-wire/session/ürün-lifecycle'ının dolaylı sahibi olur → SoC ihlali. Revize ayrım:

```text
kama-engine::ColdRxState              — SAF STATE COMPONENT (lifecycle-sahibi DEĞİL):
  ParamState · PreParamHold · RecoveredMap · session-cold-cap ledger'ı ·
  saf admission/deadline state-machine · dışarıdan enjekte edilen `now`
  (engine SAAT TUTMAZ — v1-§2.3 ZM-kaydıyla tutarlı; noise-wire/session tiplerini BİLMEZ)

kama-integration::ReceiverHost        — GERÇEK COMPOSITION ROOT (sahiplik):
  SessionManager · HandleRouter · noise-wire open/demux · clock-sampling ·
  ColdRxState çağrıları · transport Receiver · lifecycle/cleanup (§4) ·
  RECEIVER-tarafı notification-emisyonu (0x12 RECOVERED, bütçeli — §6)
  [sender-notice-yüzeyi take_post_timeout_notice AYRI: sender-host'ta yaşar — §6 release-düzeltmesi]
```

Yeni crate AÇILMAZ (Abkarada: bu aşamada gereksiz; `kama-integration` bu iş için var, şu an stub —
üretimleştirilir). v1-§1.3 hedef call-flow bu sahiplik ayrımıyla okunur; akışın kendisi (demux → cold-gate →
Receiver → delivery) DEĞİŞMEZ.

---

## 2. BLOCKER-2 — PATIENCE: MUTLAK, NON-RENEWABLE PER-BLOCK DEADLINE

v1-P-1/P-2 arasındaki yorum-belirsizliği (mutlak-deadline mi, shard-başına-uzayan-idle mı) DoS-açığı
üretebilirdi. **Bağlayıcı semantik (Abkarada metni aynen):**

```text
block_deadline = first_authenticated_shard_time + derived_patience     (RX_COLD_ABANDON, EK4-A1)

Sonraki shard'lar:
- block progress'i günceller
- deadline'ı ASLA resetlemez
- deadline'ı ASLA uzatmaz
```

**P-2 REVİZE:** "patience'ı besler" ifadesi KALDIRILDI. Shard-varışının patience'a TEK etkisi progress-
muhasebesidir; deadline first-shard-anchor'dan mutlaktır.

**P-4 REVİZE (session idle hold — ayrı mekanizma):**
```text
yalnız canlı block yokken
authenticated DATA arrival
→ session idle anchor yenilenebilir
```
Probe (0x19) ve genel control-trafiği NE block-deadline'ı NE session-idle-anchor'ı etkiler (v1-P-2/P-4
probe-DoS-aynası yürürlükte). SESSION_COUNT_CAP korunur.

P-3 (evict+zeroize+typed-diag+iade), P-5 (decode→retention-devri), P-6 (PreParamHold-TTL'i per-block-
deadline kapsar; `ParamNeverArrived`) v1'den AYNEN yürürlükte — P-6'daki deadline artık non-renewable
formdadır (pre-PARAM blok da first-shard-anchor'dan mutlak süre yaşar).

---

## 3. BLOCKER-3 — CAP HİYERARŞİSİ: BAĞIMSIZ TÜRETİMLİ DÖRT CAP + GERÇEK BELLEK FORMÜLÜ

**RED (Abkarada):** `RX_COLD_BLOCK_CAP=256` — `MAX_PREPARAM_SHARDS=256` SHARD-emsalidir, eşzamanlı-açık-BLOK
sayısına taşınamaz (farklı kaynak eksenleri). Ayrıca v1'de shard-cap(256) ile byte-formülü(256-blok×K=2048-
shard-varsayımı) ÇELİŞİYORDU. Revize aile (Abkarada hiyerarşisi aynen):

```text
RX_COLD_SHARDS_PER_BLOCK_CAP          = profil K_shape + izinli-R üst-sınırı (wire-maximum türevi)
RX_COLD_TOTAL_SHARD_CAP_PER_SESSION   = bellek-bütçesi türevi
RX_COLD_OPEN_BLOCK_CAP_PER_SESSION    = concurrency/workload türevi
RX_COLD_RETAINED_BYTE_CAP_PER_SESSION = gerçek retained-memory hesabı
```

**Gerçek bellek formülü** — byte-hesabına YALNIZ payload girmez:
```text
retained_bytes(shard) = shard-buffer-CAPACITY (payload değil; Vec-capacity)
                      + allocator-overhead + blok-metadata payı
                      + ESI/dedup-state payı + decoder-scratch payı + retained-repair/source-kopyaları
```

**Tutarlılık-invariantı (v1-hatasının kapanışı):** dört cap BİRBİRİNDEN türetilmez; rezervasyon-defteri
(v1-§4.1, kalıcı-retention-öncesi) hangi eksen önce dolarsa ONU typed-reject eder. Byte-cap artık
blok×K çarpımından DEĞİL, doğrudan bellek-bütçesinden gelir.

**Eski-ad → yeni-eksen eşlemesi (v1-referanslı bölümler BU sözlükle okunur; verify-düzeltmesi):**

| v1-adı / defter-boyutu | v2-ekseni | Enforcement-noktası |
|---|---|---|
| "blok" (üç-boyut) / RX_COLD_BLOCK_CAP | RX_COLD_OPEN_BLOCK_CAP_PER_SESSION | defter (yeni-blok-slot rezervi) |
| "byte" / RX_COLD_BYTE_CAP | RX_COLD_RETAINED_BYTE_CAP_PER_SESSION | defter (retained-byte rezervi) |
| "shard" / RX_COLD_SHARD_CAP | RX_COLD_TOTAL_SHARD_CAP_PER_SESSION | defter (shard rezervi) |
| — (yeni) | RX_COLD_SHARDS_PER_BLOCK_CAP | **per-blok admission ÖN-KAPISI** (rezerv-anında blok-sayaç kontrolü; defter-ekseni değil) |

`CapExceeded{kind}` enum'u DÖRT-eksene genişletilir (Block|Byte|Shard|ShardsPerBlock) — icra-talimatı
maddesi. **fec-core-§13 hizalaması yeni-adlarla (v1-§3.1-tablosunun yerine geçer):** max_open_blocks →
OPEN_BLOCK_CAP · max_block_bytes/per-peer-quota → RETAINED_BYTE_CAP · **per-peer-quota → TOTAL_SHARD_CAP**
· max_K/max_symbol_size → SHARDS_PER_BLOCK_CAP · global-memory-cap → GLOBAL_COLD_MEMORY_BUDGET.

**MAX_PREPARAM_SHARDS bağlayıcı-invariantı (verify-düzeltmesi; "alt-sınır" terim-tersliği giderildi):**
`MAX_PREPARAM_SHARDS=256` engine-hold'un FİZİKSEL KAPASİTESİDİR (üst-sınır-kısıtı):
```text
RX_COLD_TOTAL_SHARD_CAP_PER_SESSION ≤ engine-hold-fiziksel-kapasitesi  (İHLAL EDİLEMEZ)
Cap 256 üstünde donacaksa MAX_PREPARAM_SHARDS AYNI dilimde eş-yükseltilir — aksi halde defter 257.
shard'ı admit eder, hold sessiz-drop yapar → "hold-drop-dalı ölü-kod-pinli" hükmü ve G-1-madde-8 bozulur.
```

**Cap-matematiği-DIŞI üç sözleşme (v1-§3.2'den yeni-adlarla yürürlük beyanı):** (a) pre-PARAM per-blok
(block_id,esi)-dedup bu katmanda; (b) tamamlanan-blok kayıtları retention-sınırlarına devrolur (fec-core
§9); (c) admit_and_decode girdi-bound'u sözleşmesel: drain ≤ TOTAL_SHARD_CAP (per-blok: SHARDS_PER_BLOCK_CAP).

**Global seviye (Abkarada modeli aynen):**
```text
GLOBAL_COLD_MEMORY_BUDGET = deployment/operator configuration        ← GERÇEK güvenlik sınırı
GLOBAL_COLD_BYTE_CAP      = min( configured_memory_budget,
                                 N_SESSION_TARGET × per_session_cap )
```
`N_SESSION_TARGET` kapasite-PLANLAMA girdisidir; güvenlik sınırı process-memory-budget'tır. Sayılar
uydurulmaz: [CAL]-türetim G-2 girdilerini bekler (§9).

D-3'ün typed-reject + noise-wire-§8 isimli-amendman kararı (v1-§3.4) AYNEN yürürlükte (Abkarada: GO).

---

## 4. BLOCKER-4 — TOMBSTONE/CLEANUP SIRASI (REVİZE)

**Bağlayıcı invariant (Abkarada):**
```text
grace boyunca retry karşılanabiliyorsa router/session routing bilgisi erken silinemez
```
v1-sırası (önce `SessionManager.remove`, sonra tombstone) bu invariant'ı İHLAL EDİYORDU: remove sonrası
gelen SESSION_CLOSE-retry'ını doğrulayacak/demux-edecek state kalmıyordu.

**Model analizi:** Retry'a cevap için asgari iki yetenek gerekir: (a) DEMUX (h_ext-router-kaydı yaşamalı),
(b) retry'ı AUTHENTICATE etmek (inner_kind şifreli — open olmadan paketin CLOSE-retry olduğu BİLİNEMEZ →
recv-cipher + replay-window grace boyunca zorunlu; open'sız-cevap = unauthenticated-tetiklemeye-yanıt =
reflection riski). Model B'nin gerçek katkısı SEND tarafıdır: cevap her seferinde YENİDEN SEAL EDİLMEZ.

**Öneri: MODEL A+B HİBRİT** (karar Abkarada'ya — DUR-B; adım-atamaları verify-düzeltmesiyle kesinleştirildi):
```text
(1)  lifecycle = Closing       : yeni cold-allocation YASAK (typed-reject)
(2a) ReceiverHost              : sealed CLOSE-ACK'i PRECOMPUTE eder — send-cipher HENÜZ CANLI
                                 (byte-for-byte cached; sonrası re-seal YOK — InitFlight cached-datagram
                                 emsali; nonce-reuse yapısal-engelli; sahibi HOST, ColdRxState DEĞİL)
(2b) teardown (HOST sorumluluğu): noise-wire send-cipher/PN-send-state düşer (zeroize-on-drop) +
                                 ColdRxState.release (ağır cold-state: hold/blok/rezervasyon iade +
                                 ZEROIZE fec-core-§9) + ControlDedupWindow düşer
                                 (CLOSE-retry'lar bundan sonra dedup'a DEĞİL tombstone'a düşer —
                                 cached-cevap zaten idempotent)
(3)  minimal-tombstone kurulur : SessionManager-kaydı tombstone-kaydına DÖNÜŞÜR:
                                 { h_ext-router-kaydı KORUNUR · recv-cipher + recv-ReplayWindow
                                   (open-yeteneği) · (2a)-cached sealed CLOSE-ACK ·
                                   grace_deadline (TOMBSTONE_GRACE [CAL]) }
(4)  grace dolunca             : tombstone-kaydı düşer (SessionManager.remove-eşdeğeri) +
                                 HandleRouter.unregister
(5)  PersistentReplayGuard     : KALIR (kalıcı restart-güvenlik-state'i; iade-zincirine GİRMEZ).
                                 v1-§4.3-(6) sözleşmesi AYNEN yürürlükte: REPLAY_PAIR_CAP [CAL] +
                                 "admit yalnız cert-verify-SONRASI" çağrı-sırası + cap-doluda yeni-çift
                                 typed-reject fail-closed. (G-1-madde-10'un ikinci yüzeyi — §8-taraması.)
```
Not: ColdRxState hiçbir noise-wire tipine dokunmaz (§1-tanımı; INV-ENGINE-SOC) — cipher-teardown HOST'un
işidir; (4)-parantezindeki eski "(2)'de düşmüştü" ifadesinin taraf-karışıklığı giderildi.
Bellek-karakteri: tombstone ≈ recv-yön ReplayWindow(2KB) + recv-cipher + **~51B tek-range-ACK cached-cevap**
(26B-outer + 6B-inner-prefix + 19B ACK-body; "43B" yanlış-emsaldi — o CTRL_CS_ECHO sabiti) — session-başına
küçük, GRACE-süreli, TOMBSTONE_REGISTRY_CAP [CAL] ile üstten sınırlı. §2.57 tombstone/grace metniyle uyumlu
("session-state-silinse-de gelen-CLOSE-retry'larına-ACK"; buradaki "silinme" = AĞIR-state; minimal-set
sözleşmesi bu tasarımın kesinleştirmesidir).

---

## 5. BLOCKER-5 — BOUNDED FIXED-WINDOW CONTROL DEDUP (session-scoped-HashSet RED)

Abkarada: probe/echo `admit()`-bypass **ONAY** · stateless-echo **ONAY** · `seen`-HashSet'i yalnız
session-scoped yapmak **RED** (uzun-ömürlü tek session içinde hâlâ unbounded; "wrap-öncesi-renewal" pratik
bellek-sınırı değil) · **bounded fixed-window dedup ZORUNLU.**

**Revize yapı (outer ReplayWindow emsali — pn.rs sabit-bitmap modeli):**
```text
ControlDedupWindow (per-session, per-direction):
  highest_control_id : u32          (high-water)
  bitmap             : sabit-boyut  (CONTROL_DEDUP_WINDOW [CAL]; modular-index — pn.rs deseni)
  Karar:
    id >  highest                → Fresh; high-water ilerler; bitmap işaretlenir
    highest-W < id ≤ highest     → bitmap: işaretli→DuplicateAckOnly · değil→Fresh+işaretle
    id ≤ highest-W (window-dışı) → FAIL-SAFE: DuplicateAckOnly (idempotent-ACK yeniden-üret; side-effect YOK)
Boyut: CONTROL_DEDUP_WINDOW ≥ TÜM in-flight control'ların id-uzayı-SPANI [CAL]:
       retry-horizon + reorder-derinliği + burst-id-tüketimi (split_for_send her parçaya YENİ id verir —
       control.rs:191-236; ACK/SACK/RECOVERED kadansı da id tüketir).
```

**"Yeni-iş-kaybı" iddiası KOŞULLU (verify-düzeltmesi — mutlak değil):** güvence yalnız
`(monotonluk) ∧ (CONTROL_DEDUP_WINDOW ≥ in-flight-id-span)` birlikte sağlanınca geçerlidir. Monotonluk tek
başına YETMEZ: uçuştaki FRESH bir control (örn. BLOCK_ABORT id=N), araya giren id-tüketimi high-water'ı
N+W'nin üstüne taşırsa gecikmiş-varışta window-altına düşer → DuplicateAckOnly → side-effect sessiz kaybolur;
retry aynı-id taşıdığından (§2.57) tekrar-denemeler de düşer. Taksonomiye üçüncü sınıf eklendi:
window-dışı-düşük-id ∈ {çok-eski-retry, replay, **reordered-fresh**} — üçüncüsü yalnız yukarıdaki
boyut-koşulu ihlal edilirse mümkündür; bu yüzden CONTROL_DEDUP_WINDOW donma-önkoşulu §9'da
"retry-parametreleri + id-tüketim-kadansı + out-of-order-toleransı"na genişletildi.

**Önkoşul (kod-gerçeği; bu turda doğrulandı):** üretimde control_id-ATAYICISI YOK (caller-verir;
`inner.rs`/`protector.rs` id'yi parametre alır) ve §2.57 "yeni-logical-control'da-YENİ" der ama MONOTONLUK
yazmaz. Fixed-window/high-water modeli **sender-side monoton control_id atama disiplinini** gerektirir
(block_id/message_id "monoton-u32 NO-REUSE" MADDE-14 emsali). → **ÜÇÜNCÜ dar spec-notu adayı** (isimli-
amendman listesi, §10): §2.57'ye "control_id session+direction-scoped MONOTON atanır (retry aynı-id)"
kesinleştirmesi. Wire'a alan eklenmez; §2.56-sınırına dokunulmaz.

CLOSE-retry-ACK'i artık `seen`'den değil TOMBSTONE'dan gelir (§4) → v1'in "tombstone-cap × seen-TTL
etkileşimi" açık-sorusu KAPANIR. Probe/echo bu pencereye HİÇ girmez (bypass yürürlükte).

---

## 6. BLOCKER-6 — `PostTimeoutNotice`: EXACT-ONCE + BOUNDED (bağlayıcı sözleşme)

Konsept GO; v1-API'si PARTIAL idi. **Bağlayıcı sözleşme (Abkarada listesi AYNEN) + bir seçim (ayrık-işaretli):**

```text
epoch/block başına en fazla BİR notice
notice terminal DEĞİL                       (tek-terminal-invariantı; CsTerminal'e varyant EKLENMEZ)
bounded one-slot VEYA bounded queue         (Abkarada — İKİ seçenek)
stale session/epoch/block → IGNORE
duplicate RECOVERED → idempotent NO-OP      (aynı-kimlik-dublikatı için exact-once gereği)
DataAckClocked ↔ RecoveredControl yarışı → FIRST-VALID-PROOF WINS (tek notice; cause=ilk-geçerli-kanıt)
session close → notice-state RELEASE
caller-poll-etmezse: bound üstten sınırlı — bellek büyümez
```

**ÖNERİ (onay bekler — §11-1b):** iki seçenekten **per-cold-epoch ONE-SLOT** (slot-içeriği en-erken-geçerli-
kanıtı korur). Dürüst-sınır: dolu-slotta FARKLI bir bloğun notice'inin düşmesi exact-once GEREĞİ DEĞİL,
one-slot-bound SONUCUDUR — cold-start bootstrap'ta epoch-başına tek-cold-blok olduğundan pratik-çakışma
beklenmez; çok-bloklu genelleme gerekirse bounded-queue'ya geçiş [CAL]-kararıdır.
**Release-tarafı düzeltmesi (verify):** notice SENDER-tarafı pull-API'dir → notice-state release'i
SENDER-tarafı session-close/terminal-temizliğine aittir (CsTerminal-alanı emsali); alıcı-tarafı iade-zinciri
(§4) İLGİSİZDİR — v1-§6.2'nin "§4-(2)-kapsamında" ataması taraf-karıştırmaydı, kaldırıldı. §1'deki
"application-notification" kalemi de ikiye ayrılır: receiver-emisyon-bütçesi (0x12; ReceiverHost) ↔
sender-notice-yüzeyi (take_post_timeout_notice; sender-host).

**Kimlik alanları (Abkarada listesi aynen):**
```text
PostTimeoutNotice { session_id / dh_epoch · cold_epoch · block_id · cause · proof_time }
```
N8 (BtlBw'ye bit-eklemez; fires-vs-noop bit-özdeşlik testi) + flag-arkası (INV-CS-5) + tek-terminal
v1-§6.2'den AYNEN. Sender-tarafı Tetik-2 dar-dokunuşu DUR-B-3 kapısında (değişmedi).

---

## 7. EK BAĞLAYICI DÜZELTMELER

**T_ECHO_MIN sabit-500ms olarak DONAMAZ** (43B echo 0.25kbps'te ~1.4s serialization — sabit tüm
decade'lerde doğru olamaz). Ürün formu (Abkarada aynen):
```text
T_ECHO_MIN(profile) = max( security_min_interval,
                           echo_serialization_time(profile) + processing_margin )
```
500ms yalnız yüksek-bant profillerinde geçerli olabilecek başlangıç-örneğidir; [CAL] per-profil; ZM-2.

**Global cap min()-formu** §3'e işlendi (GLOBAL_COLD_MEMORY_BUDGET = operator-config birincil sınır).

---

## 8. G-1 — ON BİR MADDE (Abkarada temini; AYNEN) + FARK-TARAMASI

Bağlayıcı on-bir madde (repo-kaydı bu belgeyle yapılmış olur):

```text
 1. Profile-derived RX_COLD_ABANDON
 2. 75 saniyenin yalnız bound ≤75s profillerde kullanılması
 3. Düşük bant/1200B için ≈105s veya türetilmiş patience
 4. Per-session cold block cap
 5. Per-session cold byte cap
 6. Bounds-before-allocation
 7. Session close/timeout cleanup
 8. Typed resource diagnostic; sessiz eviction yok
 9. Stateless probe echo
10. Duplicate/replay state'in bounded tutulması
11. Probe/echo'nun unbounded InboundControlDedup.admit() yoluna girmemesi
```

Fark-taraması (madde → karşılayan bölüm → v1-durumu):

| # | v2-bölüm | v1-durumu |
|---|---|---|
| 1 | v1-§2.1 (EK4-A1) | ✓ kapsıyordu |
| 2 | v1-§2.1/2.4 (F0-(viii) tablosu) | ✓ |
| 3 | v1-§2.1 (105s-tavan / S-B) | ✓ |
| 4 | §3 (revize-adla: OPEN_BLOCK_CAP_PER_SESSION) | ✓ ama cap-matematiği TUTARSIZDI → v2-düzeltti |
| 5 | §3 (RETAINED_BYTE_CAP; gerçek-bellek formülü) | ✓ ama yalnız-payload sayıyordu → v2-düzeltti |
| 6 | v1-§4.1 (kalıcı-retention-öncesi kesinleştirmesiyle) | ✓ |
| 7 | §4 (tombstone-invariant'lı revize-sıra) | ✓ ama sıra invariant-İHLALLİYDİ → v2-düzeltti |
| 8 | v1-§3.4 + noise-wire-§8 isimli-amendman | ✓ |
| 9 | v1-§5 (g10c/g10d modeli) | ✓ |
| 10 | **§5 (fixed-window dedup)** + **§4-(5) PersistentReplayGuard** (REPLAY_PAIR_CAP + cert-verify-sonrası-çağrı-sözleşmesi — replay-state'in İKİNCİ yüzeyi) | ✗ **v1-EKSİKTİ** (session-scoping yetmezdi) → v2-kapattı |
| 11 | v1-§5 (caller-dispatch bypass) | ✓ |

Sonuç: **11/11 v2'de kapsandı**; v1'in gerçek boşluğu madde-10'du (Abkarada-tespiti doğrulandı).

---

## 9. REVİZE [CAL] TABLOSU (v1-§8.1'in yerine geçer)

| Sabit | Başlangıç/form | Donma-önkoşulu |
|---|---|---|
| RX_COLD_SHARDS_PER_BLOCK_CAP | K_shape + izinli-R (profil/wire-türevi) | profil-seti |
| RX_COLD_TOTAL_SHARD_CAP_PER_SESSION | bellek-bütçesi-türevi | **G-2 girdileri** |
| RX_COLD_OPEN_BLOCK_CAP_PER_SESSION | concurrency/workload-türevi | **G-2 girdileri** |
| RX_COLD_RETAINED_BYTE_CAP_PER_SESSION | gerçek-retained-memory formülü (§3) | **G-2 girdileri** |
| GLOBAL_COLD_MEMORY_BUDGET | operator/deployment config (birincil sınır) | **G-2 girdileri** |
| GLOBAL_COLD_BYTE_CAP | min(budget, N_SESSION_TARGET × per-session) | **G-2 girdileri** |
| SESSION_COUNT_CAP | N_SESSION_TARGET-türevi | **G-2 girdileri** |
| RX_COLD_ABANDON | EK4-A1; sabit-kol ZM-2-4-decade şerhli (v1) | profil-tablo-freeze |
| SESSION_IDLE_HOLD (S-B) | profil-türevi | ZM-2 4-decade |
| TOMBSTONE_GRACE · TOMBSTONE_REGISTRY_CAP | ≤ SESSION_IDLE_HOLD · önerilecek | aynı |
| CONTROL_DEDUP_WINDOW | ≥ in-flight-id-SPANI (§5) | **retry-parametreleri + id-tüketim-kadansı + out-of-order-toleransı** |
| REPLAY_PAIR_CAP | önerilecek (PersistentReplayGuard.per_pair; §4-(5)) | — |
| RECOVERED-emisyon bütçesi | SACK-return-supply-defterinden (v1-§6.2) | kota-parametreleri |
| T_ECHO_MIN(profile) | max(security_min, serialization+margin) — §7 | ZM-2; profil-seti |
| PARAM_SET_RETRY_CADENCE/DEADLINE | retry-horizon ≥ receiver-patience (v1) | ZM-2 |
| NOTICE-bound (öneri: per-cold-epoch one-slot; §6/§11-1b) | sabit-1 | Abkarada-seçimi |
| Plateau N_flood/süre/BOUND | v1-§7.1 — **ŞERH: F-C-bound'u v2-§5-fixed-window + §4-tombstone modeliyle YENİDEN türetilir; v1'in "session-scoped-dedup; session-cap×tombstone-cap" cümlesi GEÇERSİZ (CONTROL_DEDUP_WINDOW-bitmap + TOMBSTONE_REGISTRY_CAP türevi). §7.2-hücre-cap-değerleri de [CAL]-placeholder (OPEN_BLOCK_CAP=G-2-türevi; "cap=256/cap=2" reddedilen-sayı, freeze'e giremez)** | formül-türetim |

**G-2 NUMERİK-FREEZE GİRDİLERİ (Abkarada'dan istenen; sayı UYDURULMAZ):**
```text
deployment tier · process memory budget · expected concurrent authenticated sessions ·
maximum allowed cold-state memory percentage
```

---

## 10. SONRAKİ ADIMLAR (Abkarada süreç-kararı uyarınca; sıra bağlayıcı)

```text
(1) v2-onayı (bu belge)                                             ← DUR-B
(2) İSİMLİ SPEC-AMENDMANLARI — Abkarada-kararı İKİ-amendmandı:
    A-1  noise-wire §8  : pre-param hold "aşımda-en-eski-drop" → rezervasyon-katmanı-typed-reject
    A-2  noise-wire §2.57: probe/echo "control_id-dedup OTOMATİK" cümlesine açıklama-notu (bypass)
    ADAY (karar-listesi-DIŞI; ONAY BEKLER — §11-2):
    A-3  noise-wire §2.57: control_id MONOTON-atama kesinleştirmesi (§5-önkoşulu; v2-bulgusu —
         Abkarada'nın iki-amendman kararını genişletir, onaysız listeye GİRMEZ)
(3) [CAL]-freeze (G-2 girdileri geldikten sonra; hash+timestamp; koşu-öncesi)
(4) MUTLAK-SINIR yeniden-yazımı: değişmesine izin verilen crate/dosya yüzeyi
(5) Faz-bazlı production icra-talimatı (v1-§9 iskeleti; ReceiverHost=kama-integration + v1-iskeletindeki
    "ColdRxHost" adı ve "dedup-session-scoping" kalemleri v2-§1/§5 ile okunarak)
```
**Statü-beyanı (verify-düzeltmesi):** v1-§8.2 DUR-B kapı-seti YÜRÜRLÜKTE — DUR-B-1-(i)/(ii) = A-1/A-2
(sayısal-uyum: v1-metnindeki "İKİ kalem" A-1/A-2'dir; A-3 aday-statüsünde AYRIK); DUR-B-2/3/4a/4b/5
değişmedi. Production implementation: **v2 altı-blocker'ı kapatana ve (2)-(5) donana kadar NO-GO**
(Abkarada son-hüküm).

## 11. AÇIK SORULAR (v2-sonrası kalanlar)

1. §4 Model A+B-hibrit tombstone önerisi — onay/revizyon?
1b. §6 NOTICE-bound seçimi: Abkarada'nın iki seçeneğinden (one-slot / bounded-queue) tasarım
    per-cold-epoch ONE-SLOT önerdi — onay?
2. A-3 (control_id-monotonluk) amendman-yolu uygun mu; yoksa atayıcı-disiplin yalnız kod-seviyesi kural mı?
3. RX_COLD_TOTAL_SHARD_CAP ile MAX_PREPARAM_SHARDS=256 fiziksel-kapasitesinin ilişkisi (hold-kapasitesi
   [CAL]-türetimle revize edilebilir mi — §3).
4. v1-açık-soruları 5/7/8 (RECOVERED-bütçe oranı · PARAM_SET-kadans değerleri · ack_epoch/dh_epoch katmanı)
   — hâlâ açık; icra-talimatı öncesi karar gerekmez (ilgili fazda DUR-B).
5. G-2 dört girdisi (§9) — [CAL]-freeze önkoşulu.
