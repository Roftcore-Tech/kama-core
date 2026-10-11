# KM-SEAM-TH-001 — Transport ↔ Hub dikişi (S4): arayüz taslak notu

**Belge:** `docs/seams/KM-SEAM-TH-001-TASLAK.md` · **Tarih:** 2026-10-11 · **Kart:** K4G-13 (NON-GATING, belge-yalnız) · **Karar sahibi:** Abkarada

Durum: **ARAYÜZ TASLAĞI — G-S4 öncesi, davranış kararı içermez**

Yalnız plan K4G-13 NE (a)–(d): yeni karar, tip, kod, yeni sayı ve spec yaması yok; S4 tipleri (`s4.rs`) G-S4 ile gelir
(plan :344). Atıf `belge §bölüm :satır`, satırlar `b7da69c4`'e göre. **plan** `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` ·
**PoC** `docs/decisions/PoC-KAPSAM-001.md` · **DRC** `docs/decisions/DR-CONNECT-1.md` · `specs/` altında **ccr**
`ccr-format-spec.md` · **rpc** `application-protocol-and-rpc-spec.md` · **ttdd** `transport-layer-tdd.md` · **eng**
`adaptive-transport-engine-spec.md` · **nw** `kama-noise-wire-tdd-v2.0.md`.

## (a) Mesaj listesi ve yönü

Ad, alan ve yön kaynaktaki gibidir; PoC'de hangi alanın kullanılacağı G-S4'tedir (PoC §3 :40: zincir/iptal doğrulaması,
çoklu hub ve failover dışarıda). Plane 1 yönü: rpc §4 (:139, :154).

| Mesaj | Yön | Düzlem · bağlam | Kaynak |
|---|---|---|---|
| `CLIENT_BIND` (`0x01`) | istek istemci → hub · yanıt hub → istemci | Plane 1 · `HubChannel` | rpc §4 :141, §5 :168–204 |
| `SESSION_REBIND` (`0x08`): `{link_ref bytes16; reason u8}` → `{status u16_be; new_handle u32_be}` | istek istemci → hub · yanıt hub → istemci | Plane 1 · `HubChannel` | rpc §4 :148, §6.8 :370–375 |
| `LINK_ROUTE_MISS` — telde `ROUTE_MISS` (`0x84`) `{link_ref bytes16}`; istemci sınırlı `SESSION_REBIND` başlatır, kurtarma = `SESSION_INIT_REF` rebind | hub → istemci | Plane 1 · `HubChannel` | rpc §4 :159, §6.11 :413–414, §6.11.1 :426–444; ccr §35.16 :3166–3174, INV-RESUME-1 :3191 |
| `HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C` — **rezerve**, uygulama hub sprintinde; gövde 0 B ya da `echo_token u32_be`, ACK aynı uzunlukta ve token'ı aynen yansıtır | `0x1B` istemci → hub · `0x1C` hub → istemci | transport-control `frame_type` · `HubChannel` | eng §1.1 :125–128; ttdd §10 [AMENDMENT-10] :781–794; DRC tamamlama 2 :31; plan D18 :105 |
| Röle datagramı: `hub_handle` (4 B, açık, L2) + opak yük (L3 E2E zarfı) | istemci → hub → eş istemci, iki yönde | Plane 2 · E2E zarf; hub yalnız `hub_handle`'ı okur, yükü **açmaz** | ttdd §0 :79–80, §2 :139–151; rpc §2 :72–77, §6.2 :244–252; PoC §2 "Tek hub" :28 |

Notlar (kaynaktaki durum; karar değil):
- `HubChannel` = istemci↔hub hop bağlamı, ayrı PN uzayı ("engine ACK, HANDLE_ASSIGN, Plane 1 RPC, hop heartbeat":
  `kama-contracts/src/s3.rs:61–62`; nw §2.5 :280–283). PoC'de hop anahtarı no-op, hop'ta kimlik doğrulama yok (PoC §3 :40,
  §5 :79). `on_hub_frame()` "PoC'de yalnız düşür + say", Session durumuna (ARQ deposu, PN penceresi, oturum tablosu)
  erişemez; hub sprintinde gerçek handler'lar eklenirken bu imza korunur (plan D1 :88; K4G-01 :126, TRADE-OFF :134).
- Hub saf röledir; store-and-forward PoC'de yoktur (PoC §2 "Tek hub" EK-2 :28, EK-2 madde 2 :99).
- Kart listesinde satırı olmayan `HubChannel` çerçeveleri: nw §2.55 `HANDLE_ASSIGN` / `HANDLE_ASSIGN_ACK` (WIRE-FROZEN
  :292–307; S3 `AssignHandle`, KM-SEAM-TC-001 :100) ve engine (hop) ACK (nw :281); `hub_handle` ataması rpc §6.1
  `assigned_handle`'da da geçer (:226).

## (b) Zaten karar verilmiş kurallar

Hepsi bugün yalnız karar/spec metnidir; uygulama hub sprintinde (plan §5 :457; D15 :102; D18 :105).

1. **K5** (PoC §4 :52): hub'da eş kimliği `hub_handle`, `SocketAddr` değil. Dönüş adresi = o handle için son bind/INIT'in
   geldiği adres; veri datagramıyla **güncellenmez** (hijack yüzeyi); bilinmeyen handle → sessiz drop; heartbeat yalnız mevcut
   PeerSlot'u yeniler, slot yaratmaz (aynı kural: ttdd §10 [AMENDMENT-10] (a) REFRESH-ONLY :795–797).
2. **K4** (PoC §4 :50): IP değişimi = link kaybı. `PlatformSignal.LinkChanged` → hop bağı düşer → her aktif eş için
   `SESSION_INIT_REF` + `early_unit` (0-RTT, tek datagram) → alıcıda provisional supersession (DRC H6) → teslim edilmemişler
   `msg_id` ile yeniden; "Ölçülür, iddia edilmez"; gerçek path migration M4'te. Akış hub sprintinde (plan D15 :102). Bugün
   `early_unit` INIT'e gömülmez, oturum `Ready` olunca ilk `send()` ile gider; gömülü 0-RTT sonraki sprint (KM-SEAM-AT-001 YAMA
   KAYDI 2026-10-08 :9; plan :461).
3. **Heartbeat kadansı: ön plan 8 s · arka plan 25 s · doze OS takvimi** (DRC :31 "KS-HB-1 = 8 s / 25 s / OS"; ccr §35.15
   :3151–3153; PoC §2 "Süreç ömrü" :29; plan D18 :105 "değişmez"). Kadans = hub-kanalı prob backoff'unun bağlama göre tavanı
   (ttdd §10 [AMENDMENT-10] madde 2 :763, :801); aynı bloğun 3–6. maddeleri de karardır: link yokken 0 emisyon, anında prob,
   RNG'siz zorunlu jitter, sinyal yokken ön plan kadansı (:764–778).
4. **K = 1024 → `SESSION_REBIND`** (DRC H5 :15; ccr §35.15 KS-PN-1 :3143–3144; PoC §2 "Süreç ömrü" :29): Session bağlamında
   hop-ACK almadan 1024 PN tüketilince yeni Session emisyonu durur; sırayla (1) `SESSION_REBIND` → (2) floor kadansta hub-kanalı
   prob'u → (3) hop-ACK gelince ARQ kaldığı yerden devam; oturum kapanmaz (ttdd §12 [AMENDMENT-9] :930–934). Heartbeat Session
   PN'i tüketmez, K'ye sayılmaz (ttdd :790–791).

## (c) AÇIK — bu notta karar yok (G-S4 / hub sprinti; plan D18 :105)

| # | Başlık | Kaynaktaki durum |
|---|---|---|
| A1 | Kaçırılan heartbeat eşiği; `LinkSuspect` / `LinkLost` geçişleri | Heartbeat'e özgü eşik ve iki durum spec'lerde yok (adlar yalnız plan :105, :344). En yakın metin: ttdd §9 tanısı `no_ack(T) → PATH_DEAD` (:748; T `[CAL]`, eng :940); tanı durumu sonlandırma nedeni değil (ttdd §0 :62). KS-UI-1 `LongOutage` yalnız UI, "protokol parametresi DEĞİL" (ccr :3154–3155; KM-SEAM-AT-001 §4 :146). |
| A2 | `hub_handle` TTL | TTL'in değeri ve tanımı yok. İlgili: ccr §35.16 "TTL expiry" tetiği (:3169); ccr §35.7 KS-HANDLE-1 — handle ömrü oturuma bağlı, yeniden kullanım tombstone'lu (:2736). |
| A3 | Handle başına kuyruk / hız | Değer ve tanım yok. İlgili: ccr §35.7 bilinmeyen handle için "cheap-drop / rate-limit" (:2749); rpc §12 yalnız Plane 1 yöntem kancaları (:641–649). |
| A4 | Hata satırları | S4 hata tablosu yok; rpc §11 taksonomisi var (:575–637). Sınır noktaları: `ROUTE_MISS` ile sessiz drop'un ayrımı (K5 sessiz drop, PoC :52; ccr ikisini birlikte anar, :2749, :2906); rpc §6.11.1 sınırı tükenince tek hub'da ne olacağı (failover PoC dışı, PoC §3 :40). |
| A5 | (b)4'teki `hop-ACK`'in PoC tek-hub kipinde kimden geldiği; `HubChannel` kısıtıyla ilişkisi | Yazılı değil. İlgili: eng §0.1 hop-kapsamlı güvenilirlik (:37, :39); PoC §2 "Tek hub" (:28) saf röle, alıcı çevrimdışıyken mesaj gönderenin transport ARQ'sunda bekler; engine ACK `HubChannel`'da (s3.rs :61–62), `on_hub_frame` ARQ deposuna erişemez (plan :126, :134). |

## (d) Hub sprintinde yamalanacak spec cümleleri (bu not yama yapmaz)

| # | Cümle | Çelişki | Depodaki durum |
|---|---|---|---|
| Y1 | ccr §35.16: "client keepalive YOK (state hub'da; miss → SESSION_INIT_REF rebind, sürekli ping değil)" | DRC H4 / tamamlama 2: hop heartbeat `0x1B`/`0x1C` vardır (:14, :31) | **Zaten üstü çizili** (2026-10-04, DRC H4): :3175, yerine yazılan not :3176–3179, YAMA KAYDI :13. Plan :343'teki "yama hub sprintinde" bu kaydı anmıyor. |
| Y2 | ttdd §2 "Alan gerekçeleri", `hub_handle`: "NAT-robust." … "TLS connection_id taşımaz (TCP 5-tuple)" (:155–158; plan bu yeri "84–96" diye anar) | K4: adres değişimi = link kaybı; dönüş yolu güncelleme kuralı ve path validation yok (PoC :50) | Üstü çizilmemiş. |
