# SafeRoom Core — CLAUDE.md

> Bu dosya Claude Code'un kalıcı proje bağlamıdır. Kod yazmadan önce OKU. Otorite belgeler `docs/specs/` altında; çelişkide **spec kazanır**.

## 1. Proje nedir

SafeRoom = hostile ağlar (yüksek paket kaybı, düşük bant, DPI/throttle, EW jamming) için federe, egemen-IP güvenli haberleşme platformu. Hedef müşteri: Türk savunma sanayii (ASELSAN kanal-partner; Baykar UAV, JÖH/PÖH, AFAD son-kullanıcı). Konum: "tactical resilient comms layer" (Signal'le yarışmaz; onun düştüğü yerde ayakta kalır).

## 2. Bu repo (saferoom-core) kapsamı — SOLO (Abkarada)

Rust workspace (ADR-12). 8 crate:
```
crypto · ccr-codec · adaptive-transport · hub-server · client-core · ra-ca · discovery · admin-server
```
**Bu repoda medya YOK.** Real-time medya (WebRTC fork, SRTP, Lyra) ayrı repo `saferoom-media`'da (Resul+Hasan). İki transport zıt taban: core = recovery-obsesif ("veri ULAŞSIN"); media = latency-obsesif ("geç paket çöp"). Buluşma noktası tek: signaling seam (bkz. `docs/specs/signaling-seam-v0.md`), entegrasyon en sonda.

## 3. Otorite belgeler (donmuş spec'ler — `docs/specs/`)

```
identity-and-trust-model.md            semantik trust otoritesi
ccr-format-spec.md                     byte-level otorite; Session Model V3 §35
pki-ca-ra-hsm-tdd.md                   PKI/CA/RA/HSM; kripto registry §13
revocation-model.md                    3-katman revocation
discovery-architecture.md              VOPRF contact discovery
routing-rendezvous-architecture.md     hub-mesh DHT, hub_handle locality
transport-layer-tdd.md                 transport WHAT/policy otoritesi (§4.4 QUIC benchmark gate)
application-protocol-and-rpc-spec.md   3-plane client↔hub app-RPC
admin-control-plane-tdd.md             Admin↔RA kontratı
tech-stack-adr.md                      ADR-01..19 (teknoloji kararları)
adaptive-transport-engine-spec.md      transport HOW/engine (implementable blueprint)
```
Bunlar KAT-1..5 GO-FREEZE. Değiştirme; uygula.

## 4. Mimari invariant'lar — İHLAL ETME

```
INV-RATE-1        loss send_rate'i DÜŞÜRMEZ. Rate = delivered-rate (BtlBw) + queue_delay ile pace edilir;
                  loss ayrı sayaç, yalnız FEC/ARQ recovery kolunu besler. (recovery-obsesif taban)
INV-FEC-1         CODE-THEN-ENCRYPT-AUTHENTICATE-SHARDS: FEC plaintext block'ta encode → per-shard AEAD →
                  wire'da yalnız ciphertext. Alıcı: AEAD verify SONRA decode (authenticate-before-decode).
                  ASLA "encrypt-then-code / ciphertext üstünde FEC" yazma.
INV-ENGINE-SOC    Transport type-agnostic + crypto-agnostic. AEAD/anahtar TUTMAZ. Kripto = Noise + kompakt
                  (ayrı katman). Transport opak birim iletir + seq/block_id/shard_index metadata okur + ölçer.
FS (forward sec.) ss_e = DH(sender_eph, recipient_static) TAM FS DEĞİL (0-RTT sınırı). Tam FS epoch-1'de,
                  SESSION_INIT_RESPONSE responder ephemeral (ee) katıldıktan sonra. Bunu "forward secrecy" diye
                  ETİKETLEME — "per-INIT ephemeral confidentiality" de.
dest_hash         SESSION_INIT logical olarak recipient_dest_hash'e bağlı ama WIRE'da cleartext TAŞINMAZ
                  (şifreli ClientHint içinde / hub hub_handle'a çevirir). Hot-path SESSION_DATA'da dest_hash YOK.
cert-bound        HubEntryV3 cert-bound (Hub Identity Cert + Hub Noise Cert). Raw hub_noise_x25519_pub
                  authoritative DEĞİL — key Hub Noise Cert'ten türetilir. RAServerEntry de cert-bound.
enrollment        Hub enrollment pubkey-bound CSR PoP (Ed25519 SPKI fingerprint allowlist). IP tek başına
                  authentication factor DEĞİL (BGP hijack/on-path/stale IP mis-issuance'a karşı).
DHT               DHT hub-mesh only; client'lar DHT participant DEĞİL. Client↔Hub server-client RPC.
                  ANNOUNCE validation recursive verify_chain (iterative DEĞİL). PINNED_ROOT_CA app-bundle'dan.
```

## 5. Teknoloji kararları (ADR — `docs/specs/tech-stack-adr.md`)

```
ADR-01  Rust (server + core + transport). unsafe yüzeyi minimum (yalnız libsodium + PKCS#11 + mobil binding).
ADR-02  Tokio async runtime.
ADR-03  Kripto: libsodium (primary) + dalek (cross-check) + custom DER. Primitif YALNIZ pki §13 registry'den.
ADR-05  Wire encoding: custom fixed-width big-endian canonical. VARINT YASAK. ccr-codec tek kaynak crate.
ADR-06  Transport: tek adaptif motor (KCP yok; QUIC/quinn benchmark zorunlu — iyi ağda quinn seçilebilir).
ADR-12  Cargo workspace monorepo.
ADR-13  Hub storage: redb (trait arkasında). RocksDB yalnız Beta benchmark kanıtıyla.
ADR-14  QUIC: quinn (pure-Rust). Fork YOK. QUIC seçilirse Session V3 semantics DEĞİŞMEZ (substrate swap).
ADR-15  Mobil FFI: UniFFI. Manuel JNI/Swift-C YOK. FFI boundary COARSE-GRAINED (paket-başı FFI YASAK).
ADR-16  FEC: reed-solomon-erasure (ErasureCodec trait arkasında). Doğruluk > hız.
ADR-17  Fuzzy controller: custom deterministik. Aynı input → aynı output (golden vector). RNG YOK.
ADR-18  netem/tc benchmark = ZORUNLU KALİTE KAPISI (7 hostile profil vs quinn+BBR, nightly).
ADR-19  Deploy: musl statik-binary + systemd. K8s Beta+. Docker yalnız test/CI.
```

## 6. Kod & branching kuralları

```
- Trunk-based: tek main, HER ZAMAN yeşil. Kısa branch (≤2 gün). Squash merge. main'e direct push YOK.
- API-first (SOLO kritik): crypto+ccr-codec public trait imzalarını ÖNCE indir (typed-error stub), sonra doldur.
- Yarım kod → feature-flag + TYPED-ERROR stub. reachable todo!() YASAK (runtime panik).
    DOĞRU: Err(TransportError::FeatureNotImplemented("arq_selective"))
    YANLIŞ: todo!(...)   ← main'de asla; yalnız private spike branch.
- Conventional commits (feat:/fix:/chore:/docs:/test:/refactor:). Cargo.lock DAİMA commit.
- Deprecated içerik → appendix'e taşı + "MUST NOT IMPLEMENT". Sessizce SİLME.
- Surgical edit: byte-layout / imza değişikliği talimatsız YAPMA. Rewrite yerine cerrahi patch.
```

## 7. CI kapıları (her PR yeşil olmalı)

```
GitHub-hosted (her PR): cargo fmt --check + clippy -D warnings + test --all-features + audit + deny +
                        miri (yalnız crypto, ccr-codec)
Self-hosted (nightly):  netem benchmark (7 profil vs quinn+BBR) — §4.4 gate, regresyon = fail
```

## 8. YAPMA listesi (hızlı yasak referansı)

```
✗ varint (ADR-05 — fixed-width big-endian)
✗ reachable todo!() (typed error kullan)
✗ "encrypt-then-code / ciphertext üstünde FEC" (INV-FEC-1: code-then-encrypt-auth-shards)
✗ ss_e'yi "forward secrecy" diye etiketleme (tam FS epoch-1)
✗ raw hub_noise_x25519_pub'ı authoritative kaynak yapma (cert-bound HubEntryV3)
✗ IP-only hub enrollment (pubkey-bound CSR PoP)
✗ transport'a AEAD/anahtar koyma (INV-ENGINE-SOC — kripto ayrı katman)
✗ loss ile send_rate düşürme (INV-RATE-1)
✗ client'ı DHT'ye sokma (hub-mesh only)
✗ kripto primitifi registry (pki §13) dışından ekleme
✗ ölçülmemiş byte tahmini / "QUIC'i yendik" kanıtsız iddia (§4.4 ölç, varsayma)
✗ genel amaçlı ASN.1 kütüphanesi (custom DER dar alt-küme, ccr §32)
```

## 9. Stil

Precise engineering. Tüm iddialar lab-verifiable. Abartı yok. Türkçe yorum/dokümanda net mühendislik dili. Kod: idiomatic Rust, açık hata tipleri (thiserror), bounded buffer (SR-AUDIT-001: unbounded alloc yok, bounds-before-alloc).

## 10. Bilinen deferred işler

```
PARSER-02  alloc-before-check OOM → bounds-check before alloc, take(MAX 4096).
PARSER-03  time overflow → checked_sub, not_after upper bound year 2100.
KRITIK-02  identity_hash algorithm-prefix → PQC ikinci algoritma EKLENMEDEN önce (şimdi Ed25519 tek, güvenli).
saferoom-contract  transport quinn/custom kararı (§4.4) + signaling API stabilleşince yazılır.
```
