# SafeRoom — Teknoloji Stack ADR (Architecture Decision Record)

**Belge:** `tech-stack-adr.md` v1.0 — Alpha
**Statü:** ADR-01..ADR-19 KABUL (TÜMÜ KİLİTLİ — Temmuz 2026, insan onayıyla)
**Tarih:** Temmuz 2026
**Bağlam:** 9 çekirdek spec belgesi FREEZE durumunda (aşağıda liste). Bu ADR spec'leri DEĞİŞTİRMEZ —
spec'lerin (WHAT) implementasyon katmanına (HOW) bağlanmasındaki teknoloji kararlarını kaydeder.
Tüm per-component TDD'ler bu ADR'ye referans verir.

---

## 0. Freeze'deki 9 Spec Belgesi (bu ADR'nin bağlandığı otoriteler)

```
1. identity-and-trust-model.md          (semantik trust otoritesi)
2. ccr-format-spec.md                   (byte-level otorite; Session Model V3 §35)
3. pki-ca-ra-hsm-tdd.md                 (PKI/CA/RA/HSM; kripto registry §13)
4. revocation-model.md                  (3-katman revocation)
5. discovery-architecture.md            (VOPRF contact discovery)
6. routing-rendezvous-architecture.md   (hub-mesh DHT, hub_handle locality)
7. transport-layer-tdd.md               (adaptif transport doktrini; KCP çıkarıldı)
8. application-protocol-and-rpc-spec.md (3-plane client↔hub app-RPC)
9. admin-control-plane-tdd.md           (Admin↔RA kontratı)
```

Kural: ADR ile spec çelişirse **spec kazanır**; ADR düzeltilir.

---

## 1. KABUL EDİLMİŞ KARARLAR (kilitli)

### ADR-01 — Ana dil: Rust (server-side + core + transport)

**Statü:** KABUL
**Karar:** Aşağıdaki tüm bileşenler Rust ile yazılır:
```
hub-server              (data plane, DHT, mailbox, hub_handle tablosu)
client-core             (kripto, session state machine, transport, offline queue — paylaşılan çekirdek)
adaptive-transport      (ortak modül/crate: hub-server + client-core her iki uçta)
ra-ca-service           (RA 3-gate: license + admin allowlist + device approval,
                         Intermediate auto-sign pipeline, CSR işleme)
discovery-server        (VOPRF eval, indeksleme, rate-limit)
ccr-codec               (canonical encode/decode/verify — tek kaynak crate)
```
**Gerekçe:**
- Memory-safety: SR-AUDIT-001'in kritik bulguları (heap corruption/RCE path'i FEC matrix indexing'de,
  SPSC data race) tam olarak C/C++ sınıfı hatalar. Rust bu sınıfı derleme zamanında kapatır.
- KCP'nin çıkarılma gerekçesinin yarısı C++/FFI mismatch idi (transport-layer-tdd.md §4) — aynı mantık
  tüm stack'e uygulanır: **unsafe FFI yüzeyi minimumda tutulur** (yalnız libsodium + PKCS#11 + mobil binding).
- Savunma sanayi denetim profili: memory-safe dil, deterministik davranış, supply-chain'i denetlenebilir
  (Cargo.lock + vendoring).
**Reddedilen:** C++ (memory-safety), Go (GC duraklamaları hostile-lowband pacing hassasiyetini bozar;
kripto ekosistemi FFI'ya dayanır), Java/Kotlin server-side (GC + footprint).

### ADR-02 — Async runtime: Tokio

**Statü:** KABUL
**Karar:** Tüm Rust servisleri Tokio üzerinde (transport-layer-tdd.md §4 zaten "Rust-native, Tokio" der).
**Gerekçe:** UDP socket ekosistemi, timer/pacing altyapısı, olgunluk, quinn/tonic uyumu.

### ADR-03 — Kripto kütüphanesi: libsodium (primary) + dalek ailesi + custom DER

**Statü:** KABUL (geçmiş oturum "KARAR 4: libsodium + custom DER"; GnuTLS/mbedTLS reddedildi)
**Karar:**
```
libsodium (FFI, tek onaylı C bağımlılığı): Ed25519, X25519, ChaCha20-Poly1305, BLAKE2s,
                                            ristretto255 grup aritmetiği (VOPRF)
dalek ailesi (ed25519-dalek / x25519-dalek / curve25519-dalek): pure-Rust yol; libsodium ile
                                            cross-check testleri (aynı vektörler iki implementasyonda)
custom DER encoder/decoder:                 X.509 projection için (ccr §32) — genel amaçlı ASN.1
                                            kütüphanesi YOK (saldırı yüzeyi + parser differential riski)
HKDF-BLAKE2s, SHA-256:                      pki-ca-ra-hsm-tdd.md §13 registry'ye sadık
```
**Gerekçe:** libsodium audit geçmişi + constant-time garantiler; dalek pure-Rust cross-check imkânı;
DER'in yalnızca dar bir alt kümesi gerekli (CCR ≡ X.509 differential fuzz, ccr §32).
**Kural:** Kripto primitifi registry dışından EKLENMEZ (pki §13 tek otorite).
**⚠ AEAD-BACKEND GÜNCELLENDİ (DR-5, aşağıdaki amendment):** ChaCha20-Poly1305'in **production-backend'i artık
RustCrypto `chacha20poly1305`** (libsodium DEĞİL); libsodium yalnız test-oracle. Ed25519/X25519/BLAKE2s/ristretto
kararları DEĞİŞMEDİ.

### ADR-03 AMENDMENT — AEAD Backend Seçimi [GÜNCELLENDİ: 2026-07, DR-5]

**KARAR:** Production-backend = RustCrypto `chacha20poly1305` v0.10.1 (IETF-varyantı;
checksum `10cd79432192d1c0f4e1a0fef9527696cc039165d729fb41b3f4f4f354c2dc35`, Cargo.lock-teyitli).
Independent-test-oracle = libsodium IETF-ChaCha20-Poly1305 (YALNIZ-CI/test; production-dependency-DEĞİL).
Runtime-backend-switching: YOK. Generic-CryptoProvider-abstraction: YOK (Kama = kapalı-tek-profil-güvenlik-yaklaşımı).

**GEREKÇE:** küçük-FFI/deployment-yüzeyi · saf-Rust-toolchain-bütünlüğü · kolay-cross-compilation · sade-build-zinciri ·
audit-edilebilir-supply-chain. (NOT: "forbid-unsafe-bozulur"-gerekçesi-DEĞİL — güvenli-wrapper-FFI çağıran-crate'in-
unsafe'i-sayılmaz; asıl-maliyet-native-lib/ABI/packaging-karmaşıklığıdır.)

**ZORUNLU-UYGULAMA-KISITLARI:** production-type-AÇIKÇA-`ChaCha20Poly1305` (XChaCha20Poly1305-YANLIŞLIKLA-KULLANILMAZ —
tip-assert) · exact-version Cargo.lock/cargo-metadata'dan (uydurma-YOK) · checksum-registry/lockfile-metadata'dan ·
license+SBOM-kayıtlı · libsodium test-only/ayrı-CI-job (production-dependency-graph'ına-GİRMEZ) ·
differential-test-failure = HARD-FAIL · runtime-backend-seçimi-YOK · public-CryptoProvider-abstraction-YOK ·
RFC-8439-resmi-vector'ları + random-{key,nonce,AAD,pt}-corpus-eşitliği.

**ESKİ-KARAR (libsodium-primary AEAD):** SUPERSEDED — erken-dönem-varsayımı; proje-saf-Rust'a-evrildi.

### ADR-03 AMENDMENT-2 — Hash Backend (BLAKE2s-256 / param_hash) [GÜNCELLENDİ: 2026-07, DR-S5-5]

**KARAR:** `param_hash = BLAKE2s-256(DS ‖ fields)` (noise-wire-tdd §8) üretim-backend'i = RustCrypto `blake2` v0.10
(`Blake2s256`). Üretim-home = `kama-noise-wire` (kripto-katmanı). Schema kripto-free KALIR (yalnız canonical-byte-üretir);
adaptive-transport crypto-agnostik KALIR (INV-ENGINE-SOC).

**GEREKÇE:** BLAKE2s-256 pki §13 registry'de (cipher_suite 0x0001). ADR-03-üst-kararı BLAKE2s'i libsodium'a atamıştı
AMA libsodium `crypto_generichash` = BLAKE2b ≠ BLAKE2s (256-bit BLAKE2s'i doğrudan expose-etmez) → libsodium-BLAKE2s-yolu
uygulanamaz. DR-5 emsali (production-crypto = RustCrypto; libsodium yalnız test-oracle) + golden-vektör zaten RustCrypto
`blake2` ile üretiliyor → drift-gate kesintisiz.

**ZORUNLU-UYGULAMA-KISITLARI:** üretim-tip AÇIKÇA `blake2::Blake2s256` (256-bit; BLAKE2b/uzunluk-varyantı-karıştırılmaz) ·
exact-version + checksum Cargo.lock'tan · golden-eşitlik-gate (`bind_transport_parameters` == `golden_param_blake2s_hash`
byte-özdeş) · runtime-backend-switching YOK · public CryptoProvider-abstraction YOK. AEAD-backend (DR-5, RustCrypto
chacha20poly1305) DEĞİŞMEZ. Gelecek Noise HKDF-BLAKE2s aynı-backend'e yönelir (tutarlılık; bu-amendment onu implemente-etmez).

### ADR-04 — VOPRF: ristretto255-SHA512 (RFC 9497), libsodium ristretto üzerinde

**Statü:** KABUL (discovery-architecture.md ile aynı; suite 0x0001)
**Karar:** Discovery Server VOPRF eval = RFC 9497 VOPRF mode (0x01), ristretto255-SHA512.
P-256 profili Beta FIPS için rezerve (kod yolu şimdi açılmaz).

### ADR-05 — Wire encoding: custom fixed-width big-endian canonical (varint YASAK)

**Statü:** KABUL (ccr-format-spec.md otorite)
**Karar:** Tüm imzalı/canonical yapılar ccr-format-spec.md'nin fixed-width big-endian kurallarıyla
encode edilir. `ccr-codec` crate tek kaynak: hub-server, client-core, ra-ca-service, discovery-server
aynı crate'i kullanır. Protobuf/JSON canonical yapılarda KULLANILMAZ.
**Not:** Protobuf yalnız ADR-07'deki mTLS/gRPC düzleminde (canonical byte'lar opak blob olarak taşınır,
ccr §2.3).

### ADR-06 — Transport: tek adaptif transport motoru (KCP yok; QUIC benchmark zorunlu)

**Statü:** KABUL (transport-layer-tdd.md freeze)
**Karar:**
```
adaptive-transport crate (Rust-native, C/C++ FFI YOK):
  - ARQ + FEC mekanizmalarını İÇERİR; fuzzy controller ölçülen bw/loss/jitter/queue/
    delivered-rate-confidence/path-health'e göre ARQ/FEC/hybrid/none SEÇER
  - ne ARQ-default ne FEC-default; application-role-blind (SoC)
  - hub-server + client-core ortak kullanır (iki uçta aynı motor)
QUIC: forklanmaz; iyi ağda (hub↔hub / RA) as-is kullanılabilir; HER testte zorunlu benchmark —
  bir rejimde QUIC daha iyiyse o rejimde QUIC seçilir.
```

### ADR-07 — Kontrol düzlemi substrate: mTLS + gRPC (RA↔CA, Admin↔RA)

**Statü:** KABUL (admin-control-plane-tdd.md + pki-ca-ra-hsm-tdd.md)
**Karar:** RA↔CA ve Admin Server↔RA hatları izole mTLS/gRPC. Canonical CCR byte'ları bu kanalda
opak blob taşınır (ccr §2.3). Client bu düzleme ASLA doğrudan bağlanmaz (hub proxy, app-RPC).

### ADR-08 — HSM: YubiHSM2 / PKCS#11 (Alpha) → TÜBİTAK DİRAK (Beta hedefi)

**Statü:** KABUL (pki-ca-ra-hsm-tdd.md; DİRAK Ed25519 desteği Beta açık-gap olarak izlenir)
**Karar:** Root (offline ceremony), Intermediate, License Authority, Discovery OPRF server_key —
hepsi PKCS#11 soyutlaması arkasında. Kod HSM markasına değil PKCS#11 arayüzüne yazılır.

### ADR-09 — Mobil istemci: Rust client-core + platform kabukları

**Statü:** KABUL (mimari; binding yöntemi ADR-15'te kilitli: UniFFI)
**Karar:**
```
client-core (Rust)     : kripto, session, transport, storage mantığı — TEK implementasyon
Android kabuğu (Kotlin): UI + platform servisleri (StrongBox keystore, push, lifecycle)
iOS kabuğu (Swift)     : UI + platform servisleri (Secure Enclave/Keychain, push, lifecycle)
```
**Gerekçe:** Kripto/protokol İKİ KEZ yazılmaz (parser/protokol divergence = güvenlik açığı sınıfı);
platform kabukları yalnız UI/OS-entegrasyon.

### ADR-10 — Parser interop testi: Rust canonical + Java/C++ differential fuzzing

**Statü:** KABUL (ccr §32)
**Karar:** `ccr-codec` (Rust) referans implementasyon. Java ve C++ minimal doğrulayıcılar YALNIZCA
differential-fuzz hedefi olarak yazılır (CCR ≡ X.509 projection dahil). libFuzzer/cargo-fuzz altyapısı.

### ADR-11 — Admin panel: web UI (React) + Rust backend

**Statü:** KABUL (backend dili ADR-01/ADR-11 ile kapandı: Rust — tek dil, ccr-codec/mTLS yeniden kullanım)
**Karar:** Admin Server backend Rust (tonic gRPC istemcisi olarak RA'ya bağlanır, ADR-07);
panel React SPA; panel↔backend arası standart HTTPS/JSON (canonical yapı taşımaz — draft'lar backend'de
canonical'a çevrilir, admin-control-plane-tdd.md draft→RA-gate→Intermediate-sign akışı).

### ADR-12 — Repo yapısı: Cargo workspace monorepo

**Statü:** KABUL
**Karar:**
```
saferoom/
  crates/
    ccr-codec/            # canonical encode/decode/verify (tek kaynak)
    crypto/               # libsodium FFI + dalek cross-check + registry
    adaptive-transport/   # ARQ+FEC+fuzzy motor (ortak)
    hub-server/
    client-core/
    ra-ca-service/
    discovery-server/
    admin-server/
  bindings/
    android/  ios/        # ADR-15 yöntemiyle
  interop/
    java-verifier/  cpp-verifier/   # ADR-10 differential hedefleri
  testvectors/            # ccr §30 gerçek byte dosyaları + conformance
```
Supply-chain: Cargo.lock commit + `cargo vendor` + `cargo audit`/`cargo deny` CI zorunlu.

---

## 2. KAPANAN KARARLAR (Temmuz 2026 insan onayı — ek şartlar NORMATİF)

### ADR-13 — Hub storage engine: redb — KABUL

```
KARAR: redb (pure-Rust, MVCC). Gerekçe: Alpha'da esas risk throughput değil — C++ FFI +
  operasyonel karmaşıklık + crash-consistency bug'ları. RocksDB "minimum unsafe/FFI yüzeyi"
  doktrinine (ADR-01/§4) istisna açar. Mailbox + DHT state + hub_handle tablosu Alpha
  ölçeğinde redb sınırlarının çok altında.
RocksDB: YALNIZCA Beta'da ölçüm redb yetmez derse (benchmark kanıtıyla) fallback.

NORMATİF — Storage trait ZORUNLU (redb'ye kod GÖMÜLMEZ; takas trait üzerinden):
  put_mailbox_item / get_mailbox_range / delete_after_ack
  put_routing_record / get_routing_record_set
  put_ahs / get_ahs
  put_handle_binding / expire_handle_binding
```

### ADR-14 — QUIC kütüphanesi: quinn — KABUL

```
KARAR: quinn (pure-Rust, Tokio-native). quiche GEREKSİZ (C FFI). QUIC fork hâlâ NO-GO.
  quinn = benchmark harness + hub↔hub/RA iyi-ağ rejimi. QUIC SafeRoom'un hostile ana
  transport'u DEĞİL — kıyas referansı + iyi-ağ yolu.
```

### ADR-15 — Mobil FFI binding: UniFFI — KABUL

```
KARAR: UniFFI. Manuel JNI/Swift-C NO-GO (UniFFI hard-block çıkarmadıkça). Gerekçe: iki
  platformda elle FFI = lifetime/memory/copy bug sınıfı — SafeRoom'daki en tehlikeli
  risklerden biri; UniFFI bu sınıfı yok eder.

NORMATİF — FFI boundary COARSE-GRAINED (packet-level FFI YASAK):
  Transport hot loop Rust İÇİNDE kalır; Kotlin/Swift yalnız coarse API çağırır.
  OLUR : send_message(...) / fetch_mailbox(...) / start_session(...) /
         process_incoming_datagram(...)
  OLMAZ: on_each_packet_cross_ffi(...)  ← paket başına FFI crossing = perf + bug riski.
```

### ADR-16 — Reed-Solomon FEC crate: reed-solomon-erasure — KABUL

```
KARAR: reed-solomon-erasure (pure-Rust). Doktrin: hız değil DOĞRULUK — FEC path'i fuzzy
  tarafından her zaman aktif edilmeyecek; ilk hedef doğru/test-edilebilir/fuzzlanabilir
  erasure coding. reed-solomon-simd Beta'da benchmark ile değerlendirilebilir.

NORMATİF — ErasureCodec trait ZORUNLU (SIMD crate'e geçiş trait üzerinden):
  trait ErasureCodec {
    encode(k, r, shards) -> parity
    reconstruct(shards) -> plaintext
  }
```

### ADR-17 — Fuzzy controller: custom deterministik motor — KABUL

```
KARAR: custom küçük fuzzy motoru (adaptive-transport içinde). Genel amaçlı fuzzy crate =
  gereksiz supply-chain riski; kural tabanı zaten transport-layer-tdd.md §5'te sabit
  (inputs: loss/jitter/bandwidth/queue_delay → outputs: fec_ratio/arq_aggressiveness/
  send_rate/compress).

NORMATİF — Determinizm ZORUNLU:
  Aynı input snapshot → aynı output. Golden test vector seti üretilecek (conformance).
  Aksi halde transport debug edilemez hale gelir.
```

### ADR-18 — Ağ simülasyon/test harness: netem/tc — KABUL (ZORUNLU KALİTE KAPISI)

```
KARAR: Linux netem/tc + container'lı iki-uç harness. Bu "açık karar" değil ZORUNLU KALİTE
  KAPISI statüsünde: SafeRoom'un iddiası "QUIC'in çöktüğü hostile-lowband rejimde daha düşük
  bantta ayakta kalmak" — bunu göstermeden custom transport SAVUNULAMAZ (§4.1).

NORMATİF — Minimum test profilleri (her profilde SafeRoom adaptive transport vs QUIC/quinn
YAN YANA, nightly CI):
  - 0.025 kbps / yüksek gecikme / düşük loss
  - 0.1 kbps / %50 loss
  - 0.2 kbps / %80 loss
  - yüksek jitter / düşük bant
  - reverse-path impaired ACK (dönüş yolu bozuk)
  - queue delay inflation (bufferbloat)
  - intermittent blackout (aralıklı tam kesinti)
```

### ADR-19 — Sunucu dağıtımı: statik tek-binary + systemd — KABUL

```
KARAR: statik-link (musl) tek-binary + systemd (Alpha default). Container OPSİYONEL.
  Kubernetes Beta+ only — savunma/kurumsal ortamda K8s dayatmak deployment friction;
  kurum altyapıları heterojen, tek binary + systemd en taşınabilir yol (hub + admin-server
  kurumda koşuyor).

NORMATİF — Her servis şunları sağlar:
  --config /etc/saferoom/<service>.toml
  systemd hardening (sandbox direktifleri)
  journald + append-only audit export
  healthcheck endpoint
  metrics endpoint
```

---

## 3. Karar Matrisi Özeti (bileşen → stack)

| Bileşen | Dil/Runtime | Kilit bağımlılıklar | İlgili spec |
|---|---|---|---|
| hub-server | Rust/Tokio | adaptive-transport, ccr-codec, redb (trait arkasında), quinn | routing, app-protocol, transport |
| client-core | Rust (mobil: Kotlin/Swift kabuk) | adaptive-transport, ccr-codec, libsodium/dalek | ccr §35, identity, transport |
| adaptive-transport | Rust/Tokio (ortak crate) | reed-solomon-erasure (trait), deterministik custom fuzzy | transport-layer-tdd |
| ccr-codec | Rust (referans) + Java/C++ fuzz hedefleri | custom DER | ccr-format-spec |
| ra-ca-service | Rust/Tokio | tonic/mTLS, PKCS#11, ccr-codec | pki-ca-ra-hsm, revocation |
| discovery-server | Rust/Tokio | libsodium ristretto (VOPRF), ccr-codec | discovery-architecture |
| admin-server | Rust backend + React UI | tonic (RA istemcisi) | admin-control-plane |
| HSM katmanı | PKCS#11 soyutlaması | YubiHSM2→DİRAK | pki-ca-ra-hsm |

---

## 4. Bağımlılık Politikası (NORMATİF)

```
- Onaylı C FFI yüzeyi: libsodium + PKCS#11 modülü. Başka C/C++ YOK (redb pure-Rust; RocksDB yalnız Beta benchmark kanıtıyla istisna olabilir).
- Kripto primitifi kaynağı: yalnız pki §13 registry. Yeni primitif = spec değişikliği gerektirir.
- cargo audit + cargo deny (lisans + advisory) CI'da zorunlu; Cargo.lock commit; vendoring release'te.
- unsafe bloklar: yalnız FFI sınırı crate'lerinde (crypto, bindings); clippy + miri CI.
- Canonical yapı üreten/parse eden TEK crate: ccr-codec (kopya parser YASAK — divergence riski).
```

---

## 5. Sonraki Belgeler (bu ADR'ye referans verecek)

```
1. srs.md                    — ölçülebilir gereksinimler (spec'lerden toplama)
2. hub-server-tdd.md         — PoC kritik yol
3. client-core-tdd.md        — PoC kritik yol
4. adaptive-transport-tdd.md — iki TDD'nin ortak kritik modülü (ayrı belge önerilir)
5. discovery-server-tdd.md
6. ra-ca-service-tdd.md / admin-server-tdd.md
7. test-vectors + conformance suite (ccr §30 gerçek byte dosyaları)
8. security-test-plan.md     — SR-AUDIT-001 128+ bulgu kapatma eşlemesi
```
