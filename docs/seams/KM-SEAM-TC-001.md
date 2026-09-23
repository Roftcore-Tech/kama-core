# KM-SEAM-TC-001 — Transport ↔ Crypto Seam Sözleşmesi (S3)

> **YAMA KAYDI — 2026-09-20:** §6 H-RETRY Bob tarafıyla genişletildi (tekrarlanan SSI1 → mevcut session + byte-identical cached SSR1; noise-wire §0.7-B, ccr INV-RESP-KEY-1).


**Belge:** `docs/seams/KM-SEAM-TC-001.md` v0.1 · **Tarih:** 2026-09-18
**İmzacı:** Abkarada (iki taraf da onun; Transport donmuş, Crypto S2'de yazılacak)
**Statü: TASARIM GO ✅ (K1–K9 + KARAR-1 kapalı) · SEAM-FROZEN HOLD 🟡** — kalan: KS-DOS-1 + KS-GRACE-1 sabitleri, §2 vektörler, §6 harness, §7 stub.
**Türetildiği kaynaklar (icat yok):** engine-spec §0.2 [AMENDMENT-5] · transport-tdd §0 INV-NEIGHBOR-1 / INV-PLAINTEXT-NOT-SEMANTICS, §7, INV-FEC-1 · kod: `seam.rs:22-46` (SealArgs), `runtime.rs:956-1008/1178-1184/1957` (seal/open çağrıları), `legacy.rs:126` (hub_handle), `protector.rs:236-247` (epoch anahtar seçimi), `wire.rs:15-30` (interim çerçeve) · noise-wire §2.5/§4/§5 · ccr §35.2.1/§35.5/§35.6/§35.8/§35.9/§35.15 · identity §15.5/§15.6
**Yazım:** her karar **K** (karar) · **G** (gerekçe, kaynak atfıyla) · **R** (reddedilen alternatif ve maliyeti) · **S** (sınır).

---

## 0. Sınır

```
Transport (kama-core)                                   Crypto
  RS/ARQ/pacing/batch/segment, hub-RPC istemcisi   ──►   tel kenarı seal/open + oturum durum makinesi
  plaintext birimler; tip bilmez; anahtar tutmaz   ◄──   SessionHandle basar; Application'ı görmez
```
İki arayüz ailesi, tek seam: **PacketProtection** (veri yolu, her datagram) ve **SessionControl** (kontrol, typed komut). Crypto'nun tek üst komşusu Transport'tur (INV-NEIGHBOR-1).

---

## 1. Arayüz / şema

### 1.1 PacketProtection

```rust
pub trait PacketProtection {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError>;
    fn open(&mut self, wire: &[u8]) -> Option<Opened>;            // None = sessiz drop
    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError>; // KESİN (üst sınır değil);
                                                                                //  pacer-admission için; noise-wire P0-3 buraya taşındı
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits; // handle GEREKTİRMEZ; sınıfın tüm profilleri
                                                                                //  (legacy/extended) için güvenli MİNİMUM — S1 capabilities()
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits;              // aktif session'ın gerçek profili — S1 v2 capabilities(session)
                                                                                //  [2026-09-19] Transport kripto format matematiğini tekrar ETMEZ
}
pub enum   ProtectionClass  { Session, HubChannel }
pub struct ProtectionLimits { pub wire_overhead_bytes: u32, pub max_plaintext_bytes: u32 }  // Session: legacy 18(+4 hub_handle)/extended 26; HubChannel: hop
pub enum   ProtectionContext { Session(SessionHandle), HubChannel }
pub struct SealedUnit      { pub bytes: Bytes, pub meta: SealedPacketMeta }              // [2026-09-19] noise-wire P0-4 ile hizalandı
pub struct SealedPacketMeta { pub full_wire_pn: u64, pub dh_epoch: u8, pub sealed_len: u32 } // YALNIZ Crypto'ya ait alanlar;
                                                                                          //  logical_ref TRANSPORT'un kendi tablosunda (K4)
pub struct Opened { pub ctx: ProtectionContext, pub frame: Bytes, pub meta: SealedPacketMeta }  // frame = transport'un verdiği plaintext çerçeve
```

**K1 — `ProtectionContext` çağıranın verdiği tek bağlamdır; Crypto çerçeve içeriğine bakarak anahtar SEÇMEZ.**
`Session(h)` → o session'ın E2E cipher state'i (yön + `dh_epoch`'a göre `send_cipher[dh_epoch & 1]`, protector.rs:245). `HubChannel` → client↔hub hop anahtarı (identity §11/§14 Noise IK/KK split).
**G:** Transport birimi hangi kapıdan aldığını bilir (app `send(h, …)` / kendi kontrol çerçevesi / Plane 1 RPC) — bu **yapısal** bilgidir, semantik değil (INV-PLAINTEXT-NOT-SEMANTICS). Kod bugün tür iletmiyor (SealArgs'ta ctx yok; runtime.rs:1089-1137 tek seal yolu) → **S3'ün getirdiği tek API değişikliği budur.**
**R:** *Crypto'nun ilk byte'tan tür çıkarması* (bugün open sonrası :1186 `plain.first()`) — reddedildi: Crypto transport çerçevesini parse etmiş olur (LAW-3 simetriği), Plane 1/2 ayrımı kodda karşılıksız kalır.
**S:** `HubChannel`'a giden her şey (engine ACK_FRAME, HANDLE_ASSIGN, Plane 1 RPC) ayrı PN uzayı ve ayrı anahtar; `Session(h)`'ye giden her şey (SESSION_DATA birimleri + o session'ın FEC shard'ları + E2E kontrol inner_kind 0x10–0x1A) **tek** E2E cipher state ve **tek** PN uzayı (noise-wire §2.5 orijinal hükmü). PoC'de `HubChannel` no-op (IdentitySeam); `Session` gerçek.

**K2 — PN sayacını Crypto tutar; `SealArgs.wire_pn` kaldırılır.**
**G:** Kod bugün PN'i transport'tan alıyor ve yalnız monotonluğu reddediyor (seam.rs:13); transport `tx_pn` PeerSlot yaşamına bağlı **sıfırlanıyor** (runtime.rs:2274; idle 105 s sonrası yeni slot → tx_pn=0) ve `wrapping_add` ile **dönüyor** (:1008). Gerçek seam'de bu, aynı anahtar altında nonce tekrarı = felaket sınıfı (noise-wire §2.6 NO-REUSE). Sayaç Crypto'da olunca transport'un her reset'i, slot değişimi ve wrap'i nonce güvenliğinden **yapısal olarak** kopar.
**R:** *(b) çağıran verir, seam monoton + slot değişiminde rekey zorlar* — reddedildi: "slot değişti" bilgisi transport-iç bir olaydır; seam'e sızdırmak LAW-8'i ihlal eder ve unutulması sessizce nonce reuse üretir. Maliyet farkı sıfır (sayaç zaten seam'de tutulabilir).
**S:** Transport PN'i **üretmez**; `seal()` dönüşündeki `SealedPacketMeta.full_wire_pn`'i (Crypto'nun atadığı tam sayaç) kendi ARQ/ACK tablosuna kaydeder — ACK'te gelen full-wire-PN aralığını (noise-wire §2.57) bu tabloyla logical birime eşler; `logical_ref`'i Transport tutar, Crypto'dan istemez (K4). Alımda `Opened.meta.full_wire_pn` aynı amaçla verilir. Transport wire'daki `seq16/pn32`'yi ayrıca okuyabilir (engine-spec §0.2 "optimistik okur, doğrulamaz") ama kimlik kaynağı `meta`'dır. Cap: `SEND_PN_CAP` (protector.rs:249) dolunca `RenewRequired`. Restart = yeni session, PN persist edilmez (noise-wire restart politikası).

**K3 — `hub_handle`'ı Crypto yazar; Transport çerçevesinde bu alan YOKTUR.**
**G:** Kod: `LegacySessionProtector::seal` `[hub_handle 4][seq16 2][ct][tag16]` (legacy.rs:126), AAD dışı; extended'da `h_ext` (session.rs:42,74). Donmuş transport DATA/FEC başlıklarında hub_handle yok (wire.rs:20). Session→HubHandle eşlemesi Crypto'nun session state'inde (HandleAssign, §2.55 hub-channel).
**R:** *Transport'a taşımak* — reddedildi: donmuş wire formatı değişir (v1.x).
**S:** `HubHandle` ≠ `SessionHandle` (ccr §35.7 AMENDMENT-6). Hub, `hub_handle` + cleartext `seq16/pn32` ile **açmadan** yönlendirir ve hop-ARQ tutar; hiçbir hub E2E'yi açmaz.

**K4 — Transport'un seal'e verdiği plaintext çerçeve = Transport'un kendi birim çerçevesidir; Crypto onu opak mühürler.**
**G:** INV-FEC-1: RS plaintext'te (transport, runtime.rs:1408), seal shard başına (seam), alıcı `open → reconstruct` (:1184 → :2061). Çerçevenin içi (block_id/esi/k, SourcePrefix, batch yapısı) Cryptex olarak ciphertext **içinde** kalır.
**KARAR-1 — canonical birim çerçevesi = (a) ccr §35.6 `BatchPayload` [KARARA BAĞLANDI 2026-09-18, Abkarada].** Transport batching'i AMP record'larını `BatchSubMessage` olarak paketler; interim `[type u8][seq u16][unit_id u64]` 11 B çerçeve (wire.rs:15-30) **emekliye ayrılır** (transport v1.x kalemi; donmuş engine davranışı değişmez, yalnız birim çerçevesi). Gerekçe: ccr disiplini (varint yok, implicit length) ve 11 B `unit_id`'nin hot-path'te lüks olması. Reddedilen (b): interim'i canonical ilan etmek — 11 B/birim ve iki çerçeve tanımı.
**S:** Hangisi seçilirse seçilsin Crypto **parse etmez**; yalnız `SealedWireUnit` üretir. `seq16` (legacy) / `pn32` (extended) Crypto'nun yazdığı dış başlıktır, transport çerçevesinin parçası değildir.

**K5 — `open()` başarısızlığı sessizdir; state değişmez.**
**G:** identity §15.5 P0-6: AEAD/replay fail → `replay_precheck` state'i ilerletmez; `replay_commit` yalnız success. Transport'a "neden düştü" verilmez — verilirse demux/replay oracle'ı ve transport'un güvenlik kararına bağlanması (LAW-4).
**S:** Transport `None` görür; kayıp gibi davranır (ARQ). Bu, sahte shard'ın **availability** etkisidir, bütünlük değil (PoC tehdit varsayımı).

### 1.2 SessionControl

```rust
pub trait SessionControl {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult;
    fn poll_event(&mut self) -> Option<SessionEvent>;
}
pub enum SessionCommand {
    Initiate      { peer: PeerRef, early_unit: Option<Bytes> },   // early_unit OPAK (ccr §35.3 AMENDMENT-6)
    AcceptInit    { wire: Bytes },                                 // STORE_MESSAGE/DELIVER'dan gelen SSI1
    AcceptResponse{ wire: Bytes },                                 // SSR1 — candidate seçimi İÇERİDE
    Renew         { session: SessionHandle },
    Close         { session: SessionHandle },
}
pub enum SessionResult {
    OutboundEnvelope { envelope_type: u8, bytes: Bytes },          // transport Plane 1 ile yollar
    Established      { session: SessionHandle },
    EstablishedAndReply { session: SessionHandle, envelope_type: u8, bytes: Bytes },
    NoOutput,
    Failed,                                                         // opak (§3)
}
pub enum SessionEvent { Ready(SessionHandle), RenewRequired(SessionHandle), Closed(SessionHandle, CloseReason) }
```

**K6 — Komut typed'dır, zarf payload'ı opaktır.**
**G:** Transport komutun *türünü* bilmek zorunda (Initiate mi AcceptResponse mu — DELIVER `envelope_type`'ından gelir, app-rpc §2); zarfın *içini* bilmemeli. "session_control(opak zarf)" ifadesi yanlıştı: komut opak değil, payload opak.
**S:** Transport `OutboundEnvelope`'u Plane 1 `STORE_MESSAGE` ile hub'a yollar (hop anahtarıyla mühürlenir, K1); Application hiçbirini görmez, yalnız `Ready(h)` alır (INV-NEIGHBOR-1).

**K7 — KS-DEMUX-1 Crypto'nun içinde kapanır: `AcceptResponse` candidate'ı kendi pending kümesinden cap'li dener.**
**G:** SSR1 wire'ında oturum ayırt edici yok (ccr §35.5); Transport ve Application candidate bilmez ve bilmemeli (bilseler handshake/transcript/key bilgisi sızar). Crypto pending INIT'leri zaten tutuyor (`init_ephemeral_sk` ≤7 gün, T-18c). Deneme sırası: **en yeni pending önce**; her deneme = 2×X25519 + 1 AEAD; toplam ≤ `PENDING_RESPONSE_TRIAL_CAP`. Sonuç ayırt edilemez (`Failed` tek tip — §3): yanlış candidate ile bozuk paket aynı görünür.
**R:** *`resp_hint` wire alanı* — LAW-7 (yasak). *Taşıyıcı bağlamı `pending_ref`* — hub/mailbox state'i gerektirir, DELIVER payload'ı tanımsız [OPEN app-rpc §6.11]; kapanırsa **optimizasyon** olarak eklenir, cap'li deneme yine fallback kalır. *Application'ın seçmesi* — INV-NEIGHBOR-1 ihlali.
**S:** PoC direct-mode dahil geçerli (INIT retry yeni ephemeral üretir → ≥2 pending mümkün). `PENDING_RESPONSE_TRIAL_CAP` ⬛ **KARAR (KS-DOS-1)** — bu belge değer vermez; cap yoksa sahte RESPONSE başına N×X25519 amplifikasyonu (ccr §35.15).

**K8 — `SessionHandle`'ı Crypto basar (`Established`/`Ready`); yaşam döngüsü invariantları transport-tdd §0 INV-NEIGHBOR-1'dedir.** Transport handle'ı `Session(h)` ctx'inde geri verir, yorumlamaz. `Renew` → yeni session, yeni handle; eski handle `Closed` sonrası geçersiz. Yenilemede transport eski handle'ın outstanding ARQ state'ini kapatır (H5/H10 — hop-local ARQ, recovery domain = FlowHandle).

**K9 — Zaman sahipliği.** Transport: teslim/yeniden-iletim deadline'ları, `Initiate` çıktısının Plane 1 ile Reliable gönderimi. Crypto: candidate expiry, key lifetime, replay retirement, epoch grace (`N_g` paket VEYA `T_g` süre — ⬛ KARAR KS-GRACE-1, sRTT'siz), session cap. Crypto **ağ retransmisyon timer'ı tutmaz**; retry = cached byte-for-byte (INV-INIT-KEY-1), Crypto aynı `OutboundEnvelope`'u yeniden **üretmez**, Transport cache'ler.

---

## 2. Byte sözleşmesi ve vektörler
Bu seam kendi wire'ını tanımlamaz. Çıktı formatları: legacy 22 B (ccr §35.6) · extended 26 B (noise-wire §2.5) · SSI1/SSR1 (ccr §35.3/§35.5). KDF/AAD/nonce: ccr §35.2.1/§35.9/§35.8. **Vektörler:** ccr §35.13 CRYPTO-P0-FREEZE seti (V-KDF-1..7, SSI1/SSR1/SSD1 exact+negatif, replay-poisoning, epoch geçişi) + **seam-özel**: `ProtectionContext::Session` vs `HubChannel` aynı çerçeve → farklı ciphertext (anahtar ayrımı) · PN sayacının transport reset/slot-değişimi/wrap'te **değişmediği** · `open()` fail sonrası state byte-özdeş. Üçüncü kaynak Python `hashlib.blake2s`+`hmac`. **Üretilmedi → HOLD.**

## 3. Hata semantiği
`open() → None` (sessiz), `command() → Failed` (opak, alt-sınıfsız — identity §15.6-6, K7 oracle). `SealError` yalnız yapısal (`FrameTooLarge`, `UnknownSession`, `CapReached`) — kripto nedeni taşımaz. Hiçbir hata state ilerletmez (P0-6/P0-13).

## 4. Zamanlama
K9. Ek: `open()` ve `seal()` senkron, bloklamaz; seam I/O yapmaz.

## 5. Sürümleme
`protocol_version`/`cipher_suite_id` (ccr §35.2); `SealArgs` v1 → v2 (ctx eklendi, wire_pn kaldırıldı — **API değişikliği, wire değişikliği DEĞİL**); çerçeve canonical'ı KARAR-1 ile v1.

## 6. Conformance harness
| Test | Kanıtladığı |
|---|---|
| H-CTX | Aynı çerçeve, `Session` vs `HubChannel` → farklı anahtar/PN uzayı; `HubChannel` PoC'de no-op byte-identity |
| H-PN-OWN | Transport PeerSlot reset / idle emeklilik / `tx_pn` wrap senaryolarında seam PN'i monoton devam eder; `seal()` dönüşündeki `meta.full_wire_pn` ardışık ve Transport'un kaydettiğiyle byte-özdeş |
| H-FEC-ORDER | Sahte shard → `open()` None → decoder'a girmez; k geçerli shard ile reconstruct başarılı (INV-FEC-1) |
| H-DEMUX | ≥2 pending; doğru candidate herhangi sırada; cap aşımı → `Failed`; yanlış candidate ile bozuk paket ayırt edilemez |
| H-RETRY | **Alice:** `Initiate` çıktısı transport cache'inden byte-identical yeniden gönderilir; ikinci `Initiate` = farklı ephemeral · **Bob:** aynı SSI1 tekrar gelirse (duplicate `AcceptInit`) mevcut session'a eşlenir ve **byte-identical cached SSR1** döner; yeni responder ephemeral / ikinci session YOK (noise-wire §0.7-B, ccr INV-RESP-KEY-1) |
| H-NO-MUTATE | Her fail yolunda replay/pending/key state byte-for-byte aynı |
| H-HANDLE | `Closed` sonrası `Session(h)` ile seal → `UnknownSession`; `HubHandle` ≠ `SessionHandle` tip düzeyinde |
| H-SPLIT | Application'dan Crypto'ya doğrudan çağrı yolu YOK (statik bağımlılık kontrolü, INV-NEIGHBOR-1) |

## 7. Karşı-taraf stub'ları
`MockCrypto` (Transport için): deterministik seal/open (vektör-uyumlu veya identity), `command()` sabit `Established`, cap'li `AcceptResponse`. `MockTransport` (Crypto için): kayıp/sırasız/duplicate enjeksiyonu, PeerSlot reset simülasyonu, `OutboundEnvelope`'u cache'leyip yeniden gönderme.

## 8. Açıklar (bu seam'i HOLD tutanlar)
| # | Kalem | Sahip |
|---|---|---|
| ~~KARAR-1~~ | KAPANDI → (a) ccr §35.6 BatchPayload; interim çerçeve emekli (transport v1.x) | — |
| ⬛ KS-DOS-1 | `PENDING_RESPONSE_TRIAL_CAP` + `HALF_OPEN_*` + `PER_SOURCE_INIT_RATE` değerleri | Abkarada |
| ⬛ KS-GRACE-1 | `N_g` / `T_g` | Abkarada |
| — | §2 vektörler + §6 harness + §7 stub | Abkarada (üretim), Hasan (koşum) |
| ~~[OPEN]~~ | DELIVER/MailboxItem `envelope_type` eklendi (app-rpc AMENDMENT-7) → Transport dispatch anahtarı; `pending_ref` optimizasyonu ayrı [OPEN] | Hayri + hub |
| [OPEN] | `HubChannel` gerçek anahtar/PN (M4; PoC no-op) | Abkarada |
