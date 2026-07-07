# SafeRoom — Signaling Seam v0 (Minimal Assumptions)

**Belge:** `docs/signaling-seam-v0.md` v0
**Repo:** `saferoom-core` (signaling'i core taşır) — media ekibi (Resul/Hasan) referans alır.
**Amaç:** `saferoom-core` (reliable transport) ile `saferoom-media` (WebRTC fork, real-time) arasındaki buluşma noktasının **minimal varsayımını** kayıt altına almak.

> **BU DOSYA API'Yİ DONDURMAZ.** Contract repo/imza, transport quinn/custom kararı (§4.4 benchmark) sonrasına ertelendi. Bu v0 yalnız iki ekibin **aynı zihinsel modeli** paylaşmasını sağlar → aylar sonra bağlanınca lifecycle uyuşmazlığı çıkmasın. Değerler değişebilir; niyet sabit.

---

## 0. Neden bu doc (sıfır temas riski)

İki repo bu fazda bağımsız ilerler. Ama tamamen sıfır temas şu riski doğurur:
```
core : "signaling blob'larını ŞÖYLE taşıyorum (ordering, retry, boyut...)"
media: "ben SDP/ICE exchange'i FARKLI lifecycle ile varsaydım"
app  : ikisini bağlayamıyor → entegrasyon fazında sürpriz yeniden-tasarım
```
v0, bu uyuşmazlığı **şimdi** — kod yazılmadan — önler.

## 1. Rol ayrımı (engine spec INV-ENGINE-SOC ile hizalı)

```
core  : signaling BLOB'unu OPAK, güvenilir (deadline-bounded) teslim eder.
        İçeriğini (SDP/ICE) OKUMAZ, parse ETMEZ — sadece taşır (crypto-agnostic + type-agnostic).
media : SDP offer/answer + ICE candidate ÜRETİR/TÜKETİR; core'un signaling kanalını çağırır.
app   : ikisini orkestra eder (entegrasyon fazı; §A.3 runbook).
```

## 2. Minimal seam yüzeyi (imza DEĞİL, kavram)

```
send_signaling(peer_id, opaque_blob)   → core'a: "bu blob'u bu peer'a ulaştır"
on_signaling(peer_id, opaque_blob)     → core'dan: "bu peer'dan bu blob geldi"
```
`opaque_blob` = SDP/ICE payload; core için anlamı YOK, byte dizisi. media üretir, media yorumlar.

## 3. Karara bağlanacak varsayımlar (v0 — değişebilir, ama şimdi hizala)

| Konu | v0 varsayımı (öneri) | Not |
|---|---|---|
| **max_blob_size** | tek signaling mesajı ≤ birkaç KB (SDP+ICE tipik) | core MTU/fragment davranışı §11 TDD; büyük ICE bundle fragment |
| **ordering guarantee** | seam **ordering GARANTİ ETMEZ** varsay | media kendi seq/state machine'ini tutmalı; core hop-reliable ama app-ordering media'nın |
| **retry / duplicate** | at-least-once; **duplicate mümkün** varsay | media idempotent işlemeli (aynı ICE candidate iki kez gelebilir) |
| **delivery semantiği** | deadline-bounded reliable (core "ulaşsın") | ama "ulaştı" = hub/peer aldı; media app-ACK'ini kendi tutar |
| **call_id / session_id** | **media** sahibi; core opak taşır | core signaling'i bir session'a bağlamaz; media call lifecycle'ı yönetir |
| **peer_id** | core'un dest_hash/identity'sine map | core routing'i çözer; media peer_id'yi core kimliğine bağlar |
| **çift-yönlü** | offer→answer→ICE trickle; her yön ayrı send_signaling | trickle-ICE: birden çok küçük blob akışı |
| **SDP/ICE opaklığı** | core ASLA okumaz/parse etmez | INV-ENGINE-SOC; ihlal = SoC kırılması |

## 4. Kırmızı çizgiler (v0'da bile ihlal edilmez)

```
- core signaling blob İÇERİĞİNİ okumaz/parse etmez (opak; INV-ENGINE-SOC).
- media, core'un ordering/retry GARANTİSİNE güvenmez → kendi state machine'i.
- call/session lifecycle MEDIA'nın; core stateless-opak taşıyıcı.
- SDP/ICE formatı bu doc'ta TANIMLANMAZ (WebRTC standardı; media'nın işi).
```

## 5. Ne zaman contract'a terfi eder

```
transport kararı (§4.4 quinn/custom) + signaling API stabil → saferoom-contract:
  - send_signaling/on_signaling KESİN imzası (C ABI / .proto)
  - max_blob_size, ordering, retry KESİN değerleri (bu tablodan kalibre)
  - versioned; iki repo pin'ler.
v0 → contract'ın DÜŞÜNCE taslağı; contract → v0'ın dondurulmuş hali.
```

**Statü:** ASSUMPTIONS v0 — API DONDURULMADI. İki ekip bunu okur, seam varsayımını hizalar; kesin kontrat transport kararı sonrası.
