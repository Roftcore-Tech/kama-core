# P4 — Transport-API-v0 Dondurma + `kama-contract` Konumlandırması

**Tarih:** 2026-08-26
**Faz:** A-hattı son kalemi (P0 burst-fix ✅ · P1 runtime ✅ · P2 default-flip ✅ · P3 sertleştirme ✅ · **P4 API-freeze**)
**Statü:** transport-katmanı ürün yüzeyi DONDURULDU (transport-API-v0). `kama-contract` crate'i + signaling KESİN imzası spec gereği ERTELENDİ.
**Otorite:** `specs/signaling-seam-v0.md` §5, `specs/adaptive-transport-engine-spec.md` §0.2 (INV-ENGINE-SOC), `specs/tech-stack-adr.md` ADR-15, CLAUDE.md §10.

---

## 0. Tek cümle + kapsam kararı

P4, ürün ekibinin (client-core tüketicileri + güvenlik-düzlemi) üstüne inşa edeceği **kriptodan
bağımsız transport-katmanı public yüzeyini** `transport-API-v0` adı altında dondurur ve
yürütülebilir bir derleme-kapısıyla drift'e karşı kilitler.

**P4 NE YAPAR:**
- Ürün-runtime yüzeyini (`client-core::runtime`), güvenlik-dikişi sözleşmesini
  (`SecuritySeam`), wire-v0 codec'i ve sevk edilen `ClientSession` yüzeyini dondurur.
- Bu yüzeyi bir imza-pin testiyle (`tests/api_surface_v0.rs`) yürütülebilir kılar.
- UniFFI **taslağını** (ADR-15 coarse-grained; bağlayıcı DEĞİL) kaydeder.

**P4 NE YAPMAZ (spec gereği — aşağıda §1):**
- `kama-contract` crate'i **oluşturmaz/yayımlamaz**.
- `send_signaling / on_signaling` KESİN imzasını, `max_blob_size / ordering / retry` KESİN
  değerlerini dondurmaz.
- Substrat (quinn/custom, §4.4) kararını kilitlemez.

---

## 1. Spec gerilimi ve çözümü (neden kapsam daraldı)

Roadmap `BITIRME-YOL-HARITASI-2026-08-18.md` P4 satırı (172) ve çıkış-kriteri #6 (154) "`kama-contract`
yayımlanmış" diyordu. **Bu, dondurulmuş spec ile çelişir:**

> `specs/signaling-seam-v0.md` satır 7: *"BU DOSYA API'Yİ DONDURMAZ. Contract repo/imza, transport
> quinn/custom kararı (§4.4 benchmark) sonrasına ertelendi."*
>
> §5 (satır 60-68): *"transport kararı (§4.4 quinn/custom) + signaling API stabil → kama-contract:
> send_signaling/on_signaling KESİN imzası (C ABI / .proto) … v0 → contract'ın DÜŞÜNCE taslağı;
> contract → v0'ın dondurulmuş hali."*
>
> CLAUDE.md §10: *"kama-contract  transport quinn/custom kararı (§4.4) + signaling API stabilleşince
> yazılır."*

İki koşul da bugün SAĞLANMADI:
- **§4.4 substrat kararı verilmedi.** r5 FAZ-1 p2-üstünlüğünü (+38pp) kanıtladı ama bu, "iyi ağda
  quinn mi custom mu" substrat kararı DEĞİL (ADR-06/§4.4 daha büyük, benchmark-tam bir kapı).
- **signaling API stabil değil** — açıkça v0/ASSUMPTIONS.

"Çelişkide spec kazanır" (CLAUDE.md başlık) → **`kama-contract`'ı şimdi yayımlamak spec ihlalidir.**
Roadmap satır 154/172, transport-katmanının kriptodan-bağımsız yüzeyini (bugün stabil, §4.4'e bağlı
DEĞİL) `kama-contract`'ın signaling-tarafıyla (§4.4-gate'li) karıştırmıştı. **Ayrım net:**

| katman | §4.4/signaling gate? | P4'te? |
|---|---|---|
| Transport-katmanı ürün API'si (opak-birim runtime + SecuritySeam sınırı) | HAYIR — kriptodan/substrattan bağımsız | ✅ DONDURULDU |
| Signaling `send_signaling/on_signaling` KESİN imzası + değerleri | EVET | ⏸️ ERTELENDİ (spec §5) |
| `kama-contract` crate'i (versioned, iki-repo pin) | EVET | ⏸️ ERTELENDİ (§10) |

Bu belge signaling-seam-v0'ın §5'teki terfi-koşulunu değiştirmez; yalnız onu tekrar teyit eder ve
roadmap satır 154/172'yi düzeltir (bkz. §7).

---

## 2. DONDURULMUŞ YÜZEY — `transport-API-v0`

Aşağıdaki her öğe `crates/client-core/tests/api_surface_v0.rs` içinde yürütülebilir pinlidir; bir
isim/alan/varyant/imza sessizce değişirse test hedefi DERLENMEZ → CI RED.

### 2.1 Ürün-runtime (`client_core::runtime`)

**Uç kurulumları (coarse — ADR-15 hizası):**
```
ClientEndpoint::connect(SocketAddr, RuntimeConfig, Arc<dyn SecuritySeam>)
    -> impl Future<Output = Result<ClientHandle, RuntimeError>>
ServerEndpoint::bind(SocketAddr, RuntimeConfig, Arc<dyn SecuritySeam>)
    -> impl Future<Output = Result<ServerHandle, RuntimeError>>
```

**Tutamaç operasyonları (coarse; hot-loop Rust içinde kalır):**
```
ClientHandle::send(Vec<u8>) -> Result<(), RuntimeError>     // non-blocking, bounded backpressure
ClientHandle::acked() -> impl Future<Output = Option<u64>>  // ilk-ACK olay akışı
ClientHandle::shutdown() -> impl Future<Output = ClientStats>
ServerHandle::local_addr() -> SocketAddr
ServerHandle::next_delivery() -> impl Future<Output = Option<Delivery>>
ServerHandle::shutdown() -> impl Future<Output = ServerStats>
```

**`RuntimeConfig` — TAM alan kümesi (Default = üretim-makul):**
`max_inflight_units`(4096) · `offer_queue`(1024) · `delivery_queue`(1024) · `max_unit_bytes`(1200) ·
`max_peers`(64) · `quota_r`(4) · `drain_cap`(3s) · `idle_hold`(105s).

**`RuntimeError` — TAM varyant kümesi (reachable `todo!()` YASAK; her sınır typed reddeder):**
`QueueFull` · `UnitTooLarge{got,cap}` · `Closed` · `Io(std::io::Error)`.

**`ClientStats` (10 alan) · `ServerStats` (7 alan) · `Delivery{peer,unit_id,payload}`** — gözlem
yüzeyi; alan kümeleri pinli. Kontrol-düzlemi dal-noktaları (`unknown_frames`) sessiz yutulmaz, sayılır.

### 2.2 `SecuritySeam` sözleşmesi (`client_core::seam`) — OPAK SINIR

INV-ENGINE-SOC'un çalışma-zamanı sınırı. Güvenlik-düzlemi (workstream B, Sprint-6) gerçek Noise/AEAD'i
**bu aynı üç metoda** takar; çalışma-zamanı döngüsü DEĞİŞMEZ.
```
trait SecuritySeam: Send + Sync + 'static {
    fn seal(&self, plain: Vec<u8>) -> Vec<u8>;        // dışarı: şifrele+etiketle
    fn open(&self, wire: &[u8]) -> Option<Vec<u8>>;   // içeri: doğrula; None = sessiz düşür
    fn overhead(&self) -> usize;                       // pacing bütçesi gerçek wire boyunu kullansın
}
```
Bugün `IdentitySeam` (stub, +`IDENTITY_TAG`=16 B byte-eşdeğer sahte tag). **GÜVENLİK İDDİASI YOK.**

### 2.3 wire-v0 codec (`client_core::wire`)
```
FRAME_DATA: u8 = 0x00   HDR_DATA: usize = 11        // fixed-width BE, VARINT YASAK (ADR-05)
encode_data(seq: u16, unit_id: u64, payload: &[u8]) -> Vec<u8>
decode_data(buf: &[u8]) -> Option<(u16, u64, &[u8])>
```
Tag BURADA DEĞİL — güvenlik dikişinin işi (SoC).

### 2.4 Sevk yüzeyi (`client_core::ClientSession`)
`ClientSession::new()` (G2: tail-ON `validated_v1` kurulu) · `on_event` · `admit` · `sender` /
`sender_mut`. `new()` sevk default'unu dondurur (P2/G2 kararı).

### 2.5 Re-export edilen motor tipleri (yol/isim pinli)
`Event · OpaqueUnit · Sender · TransmitDecision · TransmitKind · TailRecoveryConfig` (adaptive-transport) ·
`admit_and_decode · EngineError` (kama-engine) · `OpenedTransportShard` (kama-noise-wire). Bunların
derin imzaları kendi crate testlerinde; P4 yalnız re-export yolunun kararlılığını pinler.

---

## 3. Sürümleme ve kararlılık garantisi

- **Ad:** `transport-API-v0`. Semver niyeti: bu yüzeyde alan/varyant EKLEME minor; SİLME/İMZA-DEĞİŞİMİ
  major. Major'da pin testi kasıtlı güncellenir + bu belge revize edilir (sessiz drift YASAK).
- Dondurulmuş **davranış-invariant'ları** (yüzeyle birlikte bağlayıcı): opak-birim taşıma
  (INV-ENGINE-SOC), typed-refuse (SR-AUDIT-001, reachable `todo!()` yok), loss→rate düşürmez
  (INV-RATE-1), bounded-buffer (P3).
- **v0 → v1 tetikleyicisi:** §4.4 substrat kararı VEYA güvenlik-düzlemi (Sprint-6) gerçek seam'i
  takınca; ikisi de yüzey imzasını değiştirMEDEN takılacak şekilde tasarlandı (seam trait sabit).

---

## 4. Yürütülebilir freeze kapısı (KALICI-KURAL-3)

`crates/client-core/tests/api_surface_v0.rs` bir DERLEME kapısıdır (çalışma doğrulaması değil):
- `RuntimeConfig`/`ClientStats`/`ServerStats`/`Delivery` → eksiksiz destructure (alan ekle/sil = RED).
- `RuntimeError` → eksiksiz match, `_` yok (varyant ekle/sil = RED).
- Tüm metot imzaları → dondurulmuş tiplere bağlanır (imza kayarsa RED).
- Gövde koşulsuz `return` ile ASLA çalışmaz; yalnız tip-denetlenir (UDP bind yok).

Gerekçe (KALICI-KURAL-3): hash-kimliği ≠ sözleşme-uygunluğu → sözleşme yürütülebilir olmalı. Bu kapı
`cargo test`/`clippy --tests -D warnings` altında yeşil; drift'i derleme-zamanında yakalar.

---

## 5. UniFFI TASLAĞI (ADR-15 — coarse-grained; BAĞLAYICI DEĞİL)

ADR-15 NORMATİF: FFI boundary **coarse-grained**; `on_each_packet_cross_ffi` YASAK. Transport hot-loop
Rust içinde kalır; Kotlin/Swift yalnız coarse API çağırır. Aşağıdaki `.udl` taslağı §2.1 runtime
yüzeyinin FFI izdüşümüdür — mobil binding fazında (Sprint-B+) kalibre edilir, ŞİMDİ dondurulMAZ:

```udl
// TASLAK — kama_transport.udl (non-binding sketch, ADR-15 coarse boundary)
namespace kama_transport {};

dictionary RuntimeConfig {
    u32 max_inflight_units;   u32 offer_queue;   u32 delivery_queue;
    u32 max_unit_bytes;       u32 max_peers;     u32 quota_r;
    u64 drain_cap_ms;         u64 idle_hold_ms;
};

[Error]
enum RuntimeError { "QueueFull", "UnitTooLarge", "Closed", "Io" };

interface ClientEndpoint {
    // coarse: start_session / send_message — paket-başı geçiş YOK
    [Async, Throws=RuntimeError]
    constructor(string server_addr, RuntimeConfig cfg /* , SecuritySeam via callback-iface */);
    [Throws=RuntimeError] void send(bytes payload);   // process_outgoing (bounded)
    [Async] u64? acked();
    [Async] ClientStats shutdown();
};
// SecuritySeam → UniFFI callback-interface olarak dışa; güvenlik-düzlemi implement eder.
// Substrat/kripto kararı bağlanınca imzalar KESİNLEŞİR (kama-contract).
```
Not: `SecuritySeam` FFI tarafında callback-interface olur; `process_incoming_datagram(...)` coarse
girişi (ADR-15 OLUR listesi) server tarafında `next_delivery` akışına eşlenir.

---

## 6. ERTELENEN — `kama-contract` + signaling KESİN imzası

`specs/signaling-seam-v0.md` §5 terfi-koşulu (DEĞİŞTİRİLMEDİ):
```
§4.4 (quinn/custom substrat) + signaling API stabil → kama-contract:
  - send_signaling(peer_id, opaque_blob) / on_signaling(peer_id, opaque_blob) KESİN imzası (C ABI/.proto)
  - max_blob_size / ordering(garanti-yok) / retry(at-least-once, duplicate-mümkün) KESİN değerleri
  - versioned; kama-core ve kama-media iki-taraflı pin'ler.
```
Bugünkü durum: `send_signaling/on_signaling` **kavramı** hizalı (v0 §2), ama imza/değer DONDURULMADI.
Transport'un opak taşıma sözleşmesi (§2.2 SecuritySeam + §2.1 runtime) signaling blob'unu da taşıyacak
kapasitededir (opak byte); signaling-contract yalnız media↔core lifecycle değerlerini kilitler, transport
mekanizmasını değil. **Bu erteleme transport-grade'i GATE ETMEZ** (roadmap workstream A/B ayrımı).

---

## 7. Roadmap düzeltmesi (satır 154/172)

`BITIRME-YOL-HARITASI-2026-08-18.md`:
- Çıkış-kriteri #6 "`kama-contract` yayımlanmış" → **düzeltilir:** "transport-API-v0 dondurulmuş +
  yürütülebilir pin; `kama-contract` crate'i spec §5/§10 gereği §4.4+signaling'e ertelendi."
- P4 satırı (172) "kama-contract (opak seam)" → **düzeltilir:** "transport-API-v0 freeze + SecuritySeam
  opak sözleşmesi + UniFFI taslağı; signaling KESİN imzası ERTELENDİ."

Düzeltme roadmap dosyasına da işlenir (bu commit).

---

## 8. Kapanış

**A-hattı (transport product-grade) KAPANDI:** P0 ✅ P1 ✅ P2 ✅ P3 ✅ **P4 ✅**. Transport-katmanı,
kriptodan bağımsız, dondurulmuş ve yürütülebilir-pinli bir ürün yüzeyi sunar. Kalan `kama-contract`
(signaling KESİN imzası) spec gereği §4.4 substrat kararı + Sprint-6 güvenlik-düzlemine bağlıdır —
bunlar transport-grade'i gate etmeyen, ayrı-workstream/gelecek kalemlerdir.

**Çıkış kriterleri (§145 roadmap) durumu:**
| # | kriter | durum |
|---|---|---|
| 1 | Gerçek Noise + Split | ⏸️ workstream B (Sprint-6) — transport'u gate etmez |
| 2 | Üretim UDP runtime + netem-yeşil | ✅ P1 |
| 3 | Defaults kanıtla pinli | ✅ P2/G2 (tail-ON) + OFF gerekçeleri belgeli |
| 4 | Burst-erasure regresyon kapısı | ✅ P0 |
| 5 | CI (fmt/clippy/test/miri/deny/audit/fuzz-smoke) + bounded-alloc | ✅ P3 |
| 6 | `kama-contract` API dondurma | ✅ transport-tarafı (transport-API-v0); signaling-tarafı §5 ertelendi |
