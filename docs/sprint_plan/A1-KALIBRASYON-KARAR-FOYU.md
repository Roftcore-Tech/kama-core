# A1.1–A1.4 — kalibrasyon karar föyü

**Tarih:** 2026-09-23 · **Durum:** A1.4 HESAPLANDI (öneri hazır) · A1.1/A1.2/A1.3 **sahip kararı bekliyor**

Bu föy sprint planının A1 kartlarını kapatmak içindir. Kartların ikisi farklı cinsten:

| kart | cinsi | kim kapatır |
|---|---|---|
| A1.1 `HALF_OPEN_*`, `PER_SOURCE_INIT_RATE`, `PENDING_RESPONSE_TRIAL_CAP` | **politika kararı** — sayının sonucu DoS yüzeyi ve meşru istemci davranışı | sahip |
| A1.2 `N_g`, `T_g` | **politika kararı** — güvenlik penceresi genişliği | sahip |
| A1.3 `KS-8` | **politika kararı** — kullanıcıya görünen bekleme | sahip |
| A1.4 `HANDLE_TOMBSTONE` | **hesap** — spec'te formül var, girdilerden türetilir | ben (aşağıda yapıldı) |

Sayıları ben uydurmam; hesabı ve sınırları getiririm, seçimi sahip yapar. Föy okunup dört başlığa
değer yazıldığında ilgili spec satırları `[KALİBRASYON]` → sayı olarak yamanır (ayrı PR, iki aşamalı onay).

---

## Önce: kartlar arası bağ (ayrı ayrı karar verilemez)

```
KS-8 (A1.3)  ──►  HALF_OPEN_TTL (A1.1)
    Hub, meşru istemci hâlâ denerken kaydı düşürmemeli.
    KISIT:  HALF_OPEN_TTL  ≥  KS-8
```

```
KARAR-1 (idempotent connect, KM-SEAM-AT-001 §52)  ──►  PENDING_RESPONSE_TRIAL_CAP (A1.1)
    Peer başına en fazla BİR pending session var. Dolayısıyla cap'in "per-peer pending"
    bileşeni yapısal olarak 1'dir; kalibre edilecek olan yalnız **sahte-RESPONSE başına
    deneme sayısı** bileşenidir.
```

A1.3'e önce karar verilmeli; A1.1'in TTL'i ondan türer.

---

## A1.1 — KS-DOS-1 dört sabiti

**Otorite:** `specs/ccr-format-spec.md` §35.15. Semantik DONMUŞ (2026-09-19), yalnız sayı eksik.
Normatif boyutlandırma kuralı: **`HALF_OPEN_CAP` saldırgan modeline değil dürüst retry profiline
göre taban alınır**; saldırgan `PER_SOURCE_INIT_RATE` ile sınırlanır.

### `HALF_OPEN_CAP` — per-source eşzamanlı yarım-açık kayıt

Neyi bağlar: dolunca **en eski düşer** (drop-oldest), yeni INIT reddedilmez. Cap küçük seçilirse
**meşru retry'lar meşru kayıtları düşürür** (§35.15'in açık uyarısı).

Tabanı belirleyen: bir istemcinin **aynı anda kaç farklı peer'a** el sıkışabildiği. Kama'nın hedef
kullanımında bu sayı küçük değil — bir tim lideri 30–50 kişilik bir gruba aynı anda bağlanabilir.
İkinci çarpan: "source" bind ÖNCESİ **UDP 4-tuple**'dır ve §35.15 NAT arkasındaki istemcilerin aynı
tuple'ı paylaşabileceğini açıkça kabul eder ("adil olmayan ama güvenli — kota paylaşılır").

| aday | ne olur |
|---|---|
| 32 | 30+ kişilik grup açılışında meşru kayıtlar birbirini düşürmeye başlar |
| **128** | tek istemci için rahat; NAT paylaşımında 2–4 istemcilik pay bırakır |
| 512 | bellek tarafı hâlâ ucuz ama global soft-cap'in anlamı kalmaz |

**Karar:** `HALF_OPEN_CAP = ______`

### `HALF_OPEN_TTL`

Kısıt: **≥ KS-8** (yukarıdaki bağ). Üst sınırı belirleyen yok — kayıt maliyeti düşük.
Öneri biçimi: `KS-8 + bir retry aralığı pay`.

**Karar:** `HALF_OPEN_TTL = ______`

### `PER_SOURCE_INIT_RATE`

Alt sınırı belirleyen: **%90 kayıpta** meşru istemcinin cached INIT'i kaç kez göndereceği.
KS-8 penceresi boyunca üstel backoff'lu retry sayısı kadar olmalı, artı burst payı.
Aşımda INIT **sessizce düşer** (reject cevabı yok — amplifikasyon vermez), yani fazla sıkı bir
değerin bedeli doğrudan meşru kullanıcının bağlanamamasıdır.

**Karar:** `PER_SOURCE_INIT_RATE = ______ INIT/s (burst ______)`

### `PENDING_RESPONSE_TRIAL_CAP`

Yalnız **amplifikasyon** knob'u (yukarıdaki bağ gereği per-peer bileşeni = 1).
Maliyet modeli: sahte RESPONSE başına `cap × 2` X25519. Deneme sırası en-yeni-önce.

> **ÖLÇÜLMEMİŞ:** X25519'un bu makinedeki gerçek maliyeti henüz ölçülemez — Rust kripto
> bağımlılıkları A5.0'da eklenecek. Cap'i seçerken "cap × 2 × t_X25519" bütçesi A5.0'dan sonra
> ÖLÇÜLÜP bu satır güncellenmeli. Şimdilik yalnız mertebe: cap 8 ⇒ 16 skaler çarpım/sahte paket.

**Karar:** `PENDING_RESPONSE_TRIAL_CAP = ______`

---

## A1.2 — KS-GRACE-1 (`N_g`, `T_g`)

**Otorite:** `specs/kama-noise-wire-tdd-v2.0.md` **§4 "NONCE/PN/ANTI-REPLAY", madde 5**
(satır 495–505). *Sprint planının A1.2 kartı "§5" diyor — orada §5 diye bir yer yok; KS-GRACE-1
§4'ün 5. maddesidir. Plandaki işaretçi düzeltilmeli.* Semantik donmuş (2026-09-19):
epoch-0 grace kapanır = (**AEAD-geçerli** epoch-1 paketi sayısı `N_g`) VEYA (`T_g` mutlak monotonic
süre) — hangisi önce. `T_g` **implicit confirmation ANINDAN** başlar, RESPONSE gönderiminden değil.
Kapanınca epoch-0 recv state DISCARD; **yeniden açılmaz**.

Tasarım notu (kararı etkiler): eski formül `max(4×sRTT, 64 paket)` idi ve **sRTT bağımlılığı
bilerek KALDIRILDI** — kripto kabul penceresinin transport estimator'ının kalitesine bağlanması
istenmiyor. Yani `T_g` **sabit** bir süre olmalı, RTT'nin katı değil.

- `N_g` için tarihsel çıpa: eski formüldeki **64 paket**.
- `T_g` için alt sınır: hostile zarfta epoch geçişi anında yolda olabilecek son epoch-0 paketinin
  varış ufku. Üst sınır: pencere ne kadar uzunsa ele geçirilmiş epoch-0 anahtarının kullanım ömrü
  o kadar uzun.

**Karar:** `N_g = ______ paket` · `T_g = ______`

---

## A1.3 — KS-8 (`connect()` deadline)

**Otorite:** `docs/seems/KM-SEAM-AT-001.md` §104 *(yol PR #67 ile `docs/seams/` olacak)*.

Kapsam daraltması (donmuş, kararı kolaylaştırır): bu deadline **yalnız online-direct** yola
uygulanır. **Mailbox/async yolunda `connect` deadline'ı YOKTUR** — orada pending, mailbox TTL'iyle
(7 gün) yaşar ve T-18c'nin "RESPONSE 7 güne kadar gelebilir" kuralı korunur.

Yani bu sayı bir güvenlik sınırı değil, **karşı taraf çevrimiçiyken kullanıcının ne kadar
bekleyeceği**. Dolunca `ConnectFailed(ticket)`, pending kripto state kapanır (ephemeral zeroize),
`early_unit` teslim edilmemiş sayılır ve uygulama tarafından yeniden verilmelidir.

Dikkat: hostile zarfta (%50 kayıp, 1000 ms RTT) bir el sıkışmanın tamamlanması saniyeler sürer;
kısa bir KS-8 **hostile ağda ürünü çalışmaz gösterir**. Bu, Kama'nın varlık sebebine doğrudan dokunur.

**Karar:** `KS-8 = ______`

---

## A1.4 — `HANDLE_TOMBSTONE` (HESAPLANDI)

**Otorite:** `specs/ccr-format-spec.md` §35.7 KS-HANDLE-1. Formül (NORMATİF):

```
HANDLE_TOMBSTONE ≥ max( hop-ARQ azami RTO × retry ufku,
                        REPLAY_WINDOW_SIZE'ın süre karşılığı,     ← D-GAP-1 ile buraya bağlandı
                        mailbox teslim ufku )
```

### Girdi 1 — hop-ARQ azami RTO × retry ufku

`arq_retx_cap = 3` (`crates/adaptive-transport/src/arq.rs:73`). **Ama bu değer kodda
`KALİBRE DEĞİL fixture` diye işaretli ve `KamaV1Profile`'a pinlenmiş DEĞİL** — ürün sabiti olarak
alınamaz. `adaptive-transport` içinde bir RTO tavanı bulamadım; belgelenmiş tek RTO profili bench
tarafında: `rto₀ = clamp(2×RTT, 100 ms, 10 s)`, her yeniden-gönderimde ×2, **10 s tavan**
(`crates/bench/src/qdgram.rs:5`).

Cömert üst sınır: 3 deneme × 10 s tavan = **≈ 30 s**. Mertebesi saniyeler; dakikaları aşmıyor.

### Girdi 2 — `REPLAY_WINDOW_SIZE`'ın süre karşılığı

`REPLAY_WINDOW_SIZE = 1024` (ccr §35.8). Eski-paket kuralı:
`full_counter + REPLAY_WINDOW_SIZE <= highest` → reddet.

Bir datagram, **gönderen o yönde 1024 paket daha gönderene kadar** kabul edilebilir. Yani süre
karşılığı = `1024 / (o yöndeki paket hızı)`:

| hız | süre karşılığı |
|---|---|
| 100 paket/s | ≈ 10 s |
| 1 paket/s | ≈ 17 dk |
| 0.1 paket/s | ≈ 2.8 saat |

> **BULGU — bu girdi tek başına bir süreye ÇEVRİLMİYOR (D-GAP-1 yan etkisi).**
> `REPLAY_WINDOW_SIZE` bir **paket-sayısı** ufkudur; süreye çevrilmesi bir **asgari hız** varsayımı
> gerektirir ve spec'te böyle bir taban yok. Dahası tombstone'un konusu **invalidate edilmiş** bir
> handle'dır: oturum yıkıldığı için sayaç hiç ilerlemez, dolayısıyla pencere sayaçla hiç kapanmaz.
> D-GAP-1'den önce bu girdi `MAX_REORDER_DISTANCE` idi ve aynı birim sorunu oradaydı; D-GAP-1 onu
> `REPLAY_WINDOW_SIZE`'a bağladı ama **birim uyuşmazlığını çözmedi**. Pratikte Girdi 3 baskın
> olduğu için sonuç değişmiyor; yine de bu, D-GAP-1 paketinden artakalan açık bir kalemdir
> (öneri: yeni OQ, ya asgari-hız tabanı ya da ağ paket ömrü üst sınırı).

### Girdi 3 — mailbox teslim ufku

**7 gün** = 604 800 s (`specs/application-protocol-and-rpc-spec.md:458` "Bob offline → mailbox store
(TTL 7 gün)"; identity CAD-45 timestamp penceresi de max 7 gün).

### Sonuç

```
max( ≈30 s , [birimsiz — Girdi 2] , 604 800 s )  =  604 800 s
```

**ÖNERİ: `HANDLE_TOMBSTONE = 7 gün`** — Girdi 3 baskın; Girdi 1 ondan dört mertebe küçük.
§35.7 ayrıca kısa tutmayı aktif olarak önermiyor ("namespace 2³² olduğu için tombstone'un bellek
maliyeti düşüktür, kısa tutmanın gerekçesi yoktur").

### Bedeli: namespace basıncı (kontrol edildi)

Tombstone uzadıkça aynı anda "canlı + mezar taşlı" handle sayısı artar; `hub_handle` 4 byte
(2³² = 4 294 967 296) ve tahsis PRF-türevli, çakışmada `allocation_attempt++`,
`MAX_HANDLE_ALLOC_ATTEMPTS = 8`.

Doluluk `≈ R × T` (R = saniyedeki yeni bacak, T = tombstone). %25 doluluk sınırı alınırsa
(o noktada 8 denemenin hepsinin çakışma olasılığı `0.25⁸ ≈ 1.5×10⁻⁵`):

```
R × 604 800  ≤  0.25 × 2³²  = 1 073 741 824
R            ≤  ≈ 1 775 yeni bacak/saniye   (hub_handle_epoch başına)
```

Yani 7 günlük tombstone, **hub başına ~1775 yeni bacak/s**'ye kadar epoch rollover'a zorlamadan
çalışır. Alpha hedefleri için fazlasıyla yeterli. Bu sınırın aşıldığı bir dağıtımda çözüm
tombstone'u kısaltmak değil, §35.7'nin zaten öngördüğü **8B handle profili**dir.

**Karar (onay):** `HANDLE_TOMBSTONE = 7 gün` → onaylanırsa §35.7'deki `[KALİBRASYON]` bu değerle
ve yukarıdaki hesapla yamanır.

---

## Bu föyün ürettiği iki açık kalem

1. **`arq_retx_cap` ürün sabiti değil.** Kodda `KALİBRE DEĞİL fixture`, `KamaV1Profile`'a pinli
   değil. Transport v1 donmuş ilan edildi ama bu sabit dondurulmuş profilin dışında kaldı.
   A1.4 için sonucu değiştirmiyor (Girdi 3 baskın), ama transport-v1 sözleşmesinde bir boşluk.
2. **D-GAP-1 artığı: KS-HANDLE-1 Girdi 2'nin birimi.** Paket-sayısı ufku süreye çevrilmiyor;
   asgari hız tabanı veya ağ paket ömrü üst sınırı gerekiyor (yukarıdaki BULGU).

---

# KANIT PAKETİ — KS-8 / KS-HANDLE-1 / KS-PN-1 (2026-09-24)

**Karar yok.** Sahibin 5 kalemlik isteği: yalnız kod/spec'ten sayı + `dosya:satır`. Ölçüm değil, okuma;
her satır repodan doğrulanabilir. Kod tarafı `main` @ `35142f57` öncesi ağaç (Rust'a dokunulmadı).

## K1 — Hub kanalı kurulumu

| soru | cevap | kaynak |
|---|---|---|
| Ayrı bir kuruluş paketi var mı? | **YOK.** Kuruluş, ilk DATA'nın §3.3-B kopyalarıyla sürülür; kopya kuyruğu boşsa probe hiç kurulmaz | `sender.rs:5871` ("kaynak yok — ilk DATA henüz kopya planlamadı") |
| Establish-probe ilk vade | **500 ms** | `sender.rs:1434` `ESTABLISH_PROBE_INITIAL_NS` |
| Backoff | **×2** her emisyonda | `sender.rs:5911-5914` `saturating_mul(2)` |
| Tavan | **8 s** | `sender.rs:1435` `ESTABLISH_PROBE_CAP_NS` |
| Kopyanın kendi yeniden-zamanlaması | ×2 geometrik (`PC_COPY_MULTIPLIER`) | `sender.rs:1503`, `:5900-5907` |
| Yalnız hangi durumda çalışır | `SenderState::PreConverge` | `sender.rs:5866`, `:2583` |
| **İlk paketten "başarısız" ilanına azami süre** | **TANIMSIZ — pes etme YOK.** Probe yalnız (a) durum PreConverge'den çıkınca (teyit geldi) veya (b) `set_establish_probe(false)` ile söner. `EstablishFailed`/`abandon`/deneme-tavanı: transport ve client-core'da **sıfır eşleşme** | `sender.rs:5866-5869`, `:5919-5923`; `grep -rn "EstablishFailed\|abandon\|give_up\|max_establish"` → boş |
| RTO/PTO tablosu (ürün = `validated_v1`) | base PTO = `clamp(srtt + 4·rttvar + 25 ms, 10 ms, 10 s)`; RTT örneği yoksa **1 s** (Karn); tur PTO = base·2^i, **30 s** mutlak tavan; **3** probe tavanı | `tail_recovery.rs:64-82` (`validated_v1`), `:152-172` (hesap); ürün pinleri `runtime.rs:190`, `:695`, `:892` |
| `Default` PTO config'i (cap 2, 1 ms / 60 s / 120 s) | **ürün DEĞİL** — "hiçbir kampanya tarafından sürülmedi" | `tail_recovery.rs:36-44` |
| İdle emeklilik 105 s bu yolda geçerli mi? | **HAYIR — karşı tarafın işi.** 105 s, ALICI/server döngüsünde `PeerSlot` emekliliğidir (K5); istemcinin kuruluş yolunda idle-terk yok (`runtime.rs`'de idle/abandon yalnız server-loop satırlarında). `udp_runner.rs:253`'teki 105 s **bench harness'i** (`experimental-cold-start` feature), ürün değil | `runtime.rs:100`, `:246`, `:2182-2188`; `udp_runner.rs:250-255` |
| `SESSION_IDLE_HOLD` spec değeri | **YOK** — `[CAL]`-aday olarak listelenmiş, sayı verilmemiş; kod yorumu "SESSION_IDLE_HOLD hizası 105 s" diyor | `adaptive-transport-engine-spec.md:1091`; `runtime.rs:100` |

## K2 — Plane 1 RPC zaman aşımı

| soru | cevap | kaynak |
|---|---|---|
| `request_id` başına timeout var mı? | **YOK.** `request_id` semantiği: korelasyon + idempotency anahtarı + session-local monoton; süre yok | `application-protocol-and-rpc-spec.md:112-121` |
| Spec'te "timeout" kelimesi | **0 geçiş** | `grep -c -i timeout` → 0 |
| Kodda Plane 1 RPC istemcisi | **YOK** — `STORE_MESSAGE` hiçbir `.rs` dosyasında geçmiyor | `grep -rln STORE_MESSAGE crates/` → boş |
| STORE_MESSAGE yanıtını ne bekletiyor? | **Tanımlı değil** (ne spec ne kod) | — |
| "Plane 1 `Reliable` deadline'ı" (S1 §4'ün dayandığı şey) | **Yapısal olarak yok.** `DeliveryPolicy { Reliable, DeadlineBounded { deadline: Instant }, BestEffort }` — deadline alanı yalnız `DeadlineBounded`'da; `Reliable`'ın deadline'ı tanım gereği yoktur. K9 da aynı: "Initiate çıktısının Plane 1 ile Reliable gönderimi" | `KM-SEAM-AT-001.md:37`; `:104` (yanlış cümle); `KM-SEAM-TC-001.md:109` (K9) |

## K3 — STORE_MESSAGE yanıtı: doğrudan teslim ↔ mailbox ayrımı

| soru | cevap | kaynak |
|---|---|---|
| Yanıt alanları | `status: u16` (**yalnız** `0x0001 ACCEPTED_QUEUED` \| `0x03xx route miss` \| `0x07xx revoked/stale`) + `assigned_handle: u32` | `application-protocol-and-rpc-spec.md:222-225`; registry `:577-578` |
| Ayrımı taşıyor mu? | **HAYIR.** Tek kabul kodu `ACCEPTED_QUEUED`; "doğrudan teslim edildi" kodu yok | aynı |
| Hub ayrımı biliyor mu? | **EVET, ama yanıt anından SONRA ve BAŞKA hub'da.** Akış: (1) Hub A `ACCEPTED_QUEUED` döner → (2) mesh RESOLVE + FORWARD → (3) **Hub B** karar verir: "Bob offline → mailbox store (TTL 7 gün) / Bob online → server-initiated push". Karar app-level presence'a dayanır (`PRESENCE_KEEPALIVE`) | `:444-446` (adım 1), `:450-452` (adım 2), `:456-461` (adım 3), `:390` (presence) |
| Çok-hub yolunda senkron alan mümkün mü? | **Yapısal olarak hayır:** Hub A yanıtı, Hub B'nin kararından ÖNCE gider | `:446` vs `:457` sırası |
| Mevcut en yakın sinyal | `DELIVERY_STATUS (0x82)` — **OPSİYONEL / best-effort**, server-initiated, asenkron. `state: 0x01 queued · 0x02 delivered_to_hub_b · 0x03 fetched_by_recipient`. `0x03` ≈ "Bob çevrimiçi ve aldı". Non-repudiation sağlamaz | `:155`, `:404-406`, `:470` |
| Sonuç | Alan **yok**; ayrım için ya `DELIVERY_STATUS`'un INIT için normatif yapılması ya da yeni bir asenkron alan gerekir — ikisi de amendment. Senkron yanıt alanı çok-hub'da imkânsız | — |

## K4 — Session başına azami uçuş penceresi (datagram) — KS-PN-1 için

| soru | cevap | kaynak |
|---|---|---|
| Formül | `cwnd = max(cwnd_gain × BDP, min_cwnd)`, `cwnd_gain = 2`; `inflight ≤ cwnd` | `rate.rs:16`, `:111`, `:145`, `:173`; engine-spec `:643-645` |
| Alt sınır | `min_cwnd = 4 × DEFAULT_MTU = 4 × 1200 = 4 800 B` → **≈ 4 datagram** @ 1013 B | `rate.rs:18`, `:101` |
| **Üst sınır (byte)** | **YOK.** `cwnd` BDP ile orantılı, tavan sabiti yok. §14'ün `TX_POOL_MAX` `[CAL]`'ı spec'te değersiz, kodda karşılığı yok | engine-spec `:947-955`; `grep TX_POOL_MAX crates/` → boş |
| İlk uçuş (PreConverge) | `initial_window_bytes = 2 400 B` → **2 datagram** | `product_profile_v1.rs:62-65` |
| Zarfın en geniş köşesinde türetilmiş değer | h6 ailesi 10 Mbit (T0 satırı) @ 200 ms RTT (T4): BDP = 1.25 MB/s × 0.2 s = 250 kB → `cwnd` = 500 kB → **≈ 494 datagram** @ 1013 B. *(Türetme; T4 satırı bant yazmıyor, h6 = 10 Mbit varsayımı T0'dan.)* | `HOSTILE-ENVELOPE.md:12`, `:18` |
| Zarfın dar köşesinde | 8 kbit @ ~200 ms (T3a): BDP = 200 B → `min_cwnd` devreye girer → **≈ 4 datagram** | `HOSTILE-ENVELOPE.md:17` |
| "Tipik cwnd yüzlerce datagram" varsayımı | zarfın **yalnız üst köşesinde** doğru (~494); hostile köşelerde tek haneli. Kod-türevi bir **tavan** yok — KS-PN-1 için K, cwnd'den değil bağımsız bir sınırdan gelmek zorunda | yukarıdaki iki satır |

## K5 — PeerSlot idle emekliliği ve slotu düşüren olaylar — KS-HANDLE-1 için

| soru | cevap | kaynak |
|---|---|---|
| Nerede | client-core'un **server** runtime'ı (`hub-server` crate 3 satırlık iskelet; ürün alıcı döngüsü burada) | `runtime.rs:1640`, `:2120`; `crates/hub-server/src/lib.rs` (3 satır) |
| Slot ne zaman açılır | Bilinmeyen peer'dan **AEAD-açılabilen** ilk datagramda; tablo doluysa (`max_peers = 64`) yeni peer **sessizce düşer** (`dropped_new_peers`) | `runtime.rs:2258-2278`, `:243` |
| `last_seen_ns` neyle yenilenir | Her başarıyla açılan datagramda (tür fark etmez) | `runtime.rs:2281` |
| **İdle emeklilik süresi** | **105 s** (`idle_hold`); kontrol her **1 s** (`PATIENCE_TICK`) | `runtime.rs:246`, `:66`, `:2149`, `:2182-2188` |
| **Slotu düşüren BAŞKA olay var mı?** | **YOK.** `peers` tablosuna yazan yalnız iki yer: `insert` (`:2265`) ve idle `retain` (`:2187`). Teardown / Closed / rebind / ROUTE_MISS hiçbir slotu düşürmez. Modül belgesi bunu tasarım olarak yazıyor: "ASLA sessiz-evict edilmez, boşalma idle-emeklilikle" | `runtime.rs:15`, `:2184-2185`; `grep "peers\.\(remove\|clear\)"` → boş |
| 105 s'nin gerekçesi | "SESSION_IDLE_HOLD hizası"; spec'te değer yok (K1 son satır) | `runtime.rs:100` |

## Çıkarım (karar değil — yalnız türetmeyi etkileyen yapısal gerçekler)

1. **"KS-8 = (1+3)'ün toplamının üstünde" formülünün 1. terimi sonlu değil.** Transport hub-kanalı kuruluşunda hiç pes etmiyor (K1). Üst sınır olarak alınabilecek tek transport sayısı karşı tarafın 105 s idle emekliliğidir — ama o, gönderenin değil alıcının unutma süresidir.
2. **3. terim tanımsız:** Plane 1 RPC'de timeout yok ve istemci kodda yok (K2). "Plane 1 Reliable deadline" diye bir şey yapısal olarak yok — S1 §4'teki cümle düzeltilmeli (sahibin tespiti kodla doğrulandı).
3. **"Hub'ın cevabı ayrımı yapsın" fikri senkron yanıtla kurulamaz** (K3): karar Hub B'de, yanıt Hub A'dan ve daha önce gidiyor. Asenkron `DELIVERY_STATUS` var ama opsiyonel/best-effort.
4. **KS-PN-1 için K'nin cwnd'den türetilecek bir tavanı yok** (K4); zarfın üst köşesinde ~494, dar köşesinde ~4.
5. **KS-HANDLE-1 için 105 s tek ve yalnız idle sinyalidir** (K5); slotu düşüren başka olay olmadığından tombstone'un "ARQ state ölmüş" koşulu (a) pratikte "105 s sessizlik" demektir.
