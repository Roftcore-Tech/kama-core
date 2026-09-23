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
