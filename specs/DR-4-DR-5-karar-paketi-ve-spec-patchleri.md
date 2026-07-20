# DR-4 / DR-5 — KARAR PAKETİ + YAPIŞTIRMAYA-HAZIR SPEC-PATCH'LERİ
## [REVİZE — Abkarada tam-kapanış-analiziyle hizalı; FULL-EXECUTION-GO]

```
OTORİTE-SIRASI [DETERMİNİSTİK — agent-çelişki-görürse-BUNU-uygular]:
  1. BU karar-paketi          (DR-4-CLOSED-olana-kadar EN-YÜKSEK-otorite)
  2. kama-noise-wire-tdd-v2.0
  3. fec-core-tdd-v3.0
Bu paket, DR-4-CLOSED-olana-kadar iki-TDD'deki çelişen "FULL-DESIGN-GO / tüm-gate'ler-CLOSED /
D1-implementation-BAŞLAYABİLİR" ifadelerinin ÜZERİNDEDİR. Agent Sprint-4/5-hold'larını KENDİLİĞİNDEN-KALDIRAMAZ.
```

---

## BÖLÜM-1: DR-4 KOŞULLU-KİLİT-KURALI (Abkarada bunu-onaylarsa ekstra-tur-gerekmez)

Claude Code envanteri (`DR-4-envanter.md`) döndüğünde aşağıdaki kural OTOMATİK uygulanır:

**ESAS-KURAL [Blocker-1-düzeltmesi]:** karar "deprecated-D2-ne-yazıyor"a-DEĞİL, **"gerçek-ve-korunması-gereken-
compatibility-BORCU-var-mı"**ya-bağlıdır. Deprecated-appendix = tarihsel-kanıt; TEK-BAŞINA-wire-borcu-YARATMAZ.
Gerçek-borç = {persisted/released-ciphertext-açılmak-zorunda · eski-peer-interop-korunmalı · başka-implementation-
yayımlanmış-vector'a-bağlı · sahadaki-ürün-aynı-ciphertext-contract'ını-tüketiyor}.
```
KOŞUL-A — LE64-OTOMATİK (şunlardan-EN-AZ-BİRİ-kanıtlanırsa):
  (1) legacy-implementation/vector AÇIKÇA-LE64-kullanıyor; VEYA
  (2) GERÇEK-compatibility-borcu-YOK: persisted/released-ciphertext-yok + eski-peer-tüketimi-yok +
      başka-implementation-bağlı-değil + yayımlanmış-korunması-gereken-wire-contract-yok.
  → nonce96 = 0x00000000 ‖ LE64(full_pn) · Sprint-1-golden-re-baseline · DR-4-CLOSED · Sprint-4-GO.
  (D2'nin-BE-yazması, gerçek-borç-yoksa-YALNIZ-TARİHSEL-NOTTUR — LE'yi-engellemez.)

KOŞUL-B — BE64-OTOMATİK (İKİSİ-DE-kanıtlanırsa):
  (1) gerçek-legacy-ciphertext/vector BE64-ile-üretilmiş; VE
  (2) DEVAM-EDEN-compatibility-yükümlülüğü-var (yukarıdaki-borç-listesinden).
  → nonce96 = 0x00000000 ‖ BE64(full_pn) + zorunlu-sapma-beyanı.

KOŞUL-C — MANUEL (kanıt eksik/karışık/çelişkili/geri-türetilemez):
  → mapping-UYGULANMAZ · golden-re-baseline-YAPILMAZ · Sprint-4-BAŞLAMAZ · rapor-Abkarada'ya-döner.
```

---

## BÖLÜM-2: VARYANT-LE — noise-wire-tdd §4.5 [KOŞUL-A'da bu-blok-aynen-spec'e]

```markdown
## 4.5 AEAD-NONCE96 [NORMATİF — KİLİTLİ, DR-4] (ChaCha20-Poly1305, per-direction per-dh_epoch CipherState)

fn nonce96(full_pn: u64) -> [u8; 12]:
    nonce = [0u8; 12]
    nonce[0..4]  = 0x00 0x00 0x00 0x00          # 32-bit zero prefix
    nonce[4..12] = full_pn.to_le_bytes()        # LE64 — Noise-Framework-ChaChaPoly-KANONİK-mapping
    return nonce

ÖRNEK-BYTE'LAR (elle-yazılmış — golden bunları-DOĞRUDAN-assert-eder):
    full_pn=0     → 00 00 00 00 · 00 00 00 00 00 00 00 00
    full_pn=1     → 00 00 00 00 · 01 00 00 00 00 00 00 00
    full_pn=2^32  → 00 00 00 00 · 00 00 00 00 01 00 00 00
    full_pn=2^63  → 00 00 00 00 · 00 00 00 00 00 00 00 80

NOT: wire'daki pn32/seq16 BIG-ENDIAN-taşınır (§2.6 — network-byte-order); nonce96 ise CIPHER-PRIMITIVE-girdisidir
ve Noise-ChaChaPoly-kanonik-LE64-kullanır. İKİ-FARKLI-SERIALIZATION-SÖZLEŞMESİ — karıştırılmaz.

GEREKÇELİ-AYRIMLAR (test-edilebilir-INVARIANT'lar — spec-cümlesi-değil-test):
  INV-NONCE-DIR-1  : direction nonce'a-KODLANMAZ çünkü Split() per-direction-AYRI-key-üretir.
                     BİRİNCİL-TEST: deterministic-KDF/Split-vector → k_A→B != k_B→A (key-derivation-seviyesi).
                     EK-REGRESSION: aynı-{nonce,AAD,plaintext}-iki-yön-farklı-ciphertext (ispat-değil-regression).
  INV-NONCE-EPOCH-1: dh_epoch nonce'a-KODLANMAZ çünkü her-epoch-AYRI-cipher-key/state.
                     BİRİNCİL-TEST: k_epoch0 != k_epoch1 (KDF-vector) + epoch1-counter=0-assert + epoch0/epoch1-
                     AYNI-key-NESNESİNİ-paylaşamaz (tip/lifecycle-assert).
  INV-NONCE-REUSE-1: aynı-key-altında-aynı-nonce96-İKİNCİ-KEZ-KULLANILAMAZ.
                     TEST: counter-monoton + no-reuse-guard + wrap/cap-öncesi-SESSION_RENEW_REQUIRED + restart=
                     yeni-session/yeni-key (full-PN-persist-YOK).

Seçili-mapping protocol-version-V1-için-IMMUTABLE.
GOLDEN-ZORUNLU-ASSERT-SETİ: reconstructed-full_pn · exact-nonce96-bytes · exact-AAD · exact-ciphertext · exact-tag ·
RustCrypto==libsodium-IETF-eşitliği (K-WIRE-NONCE96-1).
```

---

## BÖLÜM-3: VARYANT-BE — noise-wire-tdd §4.5 [KOŞUL-B'de bu-blok-aynen-spec'e]

```markdown
## 4.5 AEAD-NONCE96 [NORMATİF — KİLİTLİ, DR-4] (ChaCha20-Poly1305, per-direction per-dh_epoch CipherState)

fn nonce96(full_pn: u64) -> [u8; 12]:
    nonce = [0u8; 12]
    nonce[0..4]  = 0x00 0x00 0x00 0x00          # 32-bit zero prefix
    nonce[4..12] = full_pn.to_be_bytes()        # BE64 — KAMA-SPECIFIC mapping
    return nonce

⚠ SAPMA-BEYANI [ZORUNLU-GÖRÜNÜRLÜK]: Bu-mapping Noise-Framework-ChaChaPoly-kanonik-LE64-mapping'inden BİLİNÇLİ-
OLARAK-FARKLIDIR. Sebep: mevcut-Kama-legacy-wire-compatibility (envanter-kanıtı: DR-4-envanter.md §<ref> —
<legacy-vector/D2-satır-referansı-buraya>). Standart-Noise-ChaChaPoly-CipherState-nonce-mapping'iyle CIPHERTEXT-LEVEL-interop-beklenmez; standart-Noise-
implementation ancak-Kama-specific-nonce-adapter/cipher-implementation-ile-bu-profile-uyabilir. (Kama-zaten-özel-
framing/protokol-kullanır — snow-ile-doğrudan-interop-yalnız-nonce'tan-kaynaklanmaz.) Cross-implementation her-
zaman-bu-spec'in-vector'larına-pinlenir.

ÖRNEK-BYTE'LAR (elle-yazılmış — golden bunları-DOĞRUDAN-assert-eder):
    full_pn=0     → 00 00 00 00 · 00 00 00 00 00 00 00 00
    full_pn=1     → 00 00 00 00 · 00 00 00 00 00 00 00 01
    full_pn=2^32  → 00 00 00 00 · 00 00 00 01 00 00 00 00
    full_pn=2^63  → 00 00 00 00 · 80 00 00 00 00 00 00 00

[VARYANT-LE'deki GEREKÇELİ-AYRIMLAR (INV-NONCE-DIR-1 / INV-NONCE-EPOCH-1 / INV-NONCE-REUSE-1), IMMUTABLE-kuralı
ve GOLDEN-ZORUNLU-ASSERT-SETİ AYNEN-geçerli — buraya-kopyalanır.]
```

---

## BÖLÜM-4: DR-5 — ADR-03 YENİ-METNİ [KOŞULSUZ — hemen-işlenebilir]

```markdown
# ADR-03 — AEAD Backend Seçimi [GÜNCELLENDİ: 2026-07, DR-5]

KARAR: Production-backend = RustCrypto `chacha20poly1305` (IETF-varyantı).
       Independent-test-oracle = libsodium IETF-ChaCha20-Poly1305 (YALNIZ-CI/test; production-dependency-DEĞİL).
       Runtime-backend-switching: YOK. Generic-CryptoProvider-abstraction: YOK
       (Kama = kapalı-tek-profil-güvenlik-yaklaşımı).

GEREKÇE: küçük-FFI/deployment-yüzeyi · saf-Rust-toolchain-bütünlüğü · kolay-cross-compilation · sade-build-zinciri ·
audit-edilebilir-supply-chain. (NOT: "forbid-unsafe-bozulur"-gerekçesi-DEĞİL — güvenli-wrapper-FFI çağıran-crate'in-
unsafe'i-sayılmaz; asıl-maliyet-native-lib/ABI/packaging-karmaşıklığıdır.)

ZORUNLU-UYGULAMA-KISITLARI: production-type-AÇIKÇA-`ChaCha20Poly1305` (XChaCha20Poly1305-YANLIŞLIKLA-KULLANILMAZ —
tip-assert) · exact-version AGENT-UYDURMAZ → Cargo.lock/cargo-metadata'dan-alınır · checksum-registry/lockfile-
metadata'dan · license+SBOM-kayıtlı · libsodium test-only/ayrı-CI-job (production-dependency-graph'ına-GİRMEZ) ·
differential-test-failure = HARD-FAIL · runtime-backend-seçimi-YOK · public-CryptoProvider-abstraction-YOK ·
RFC-8439-resmi-vector'ları + random-{key,nonce,AAD,pt}-corpus-eşitliği.

ESKİ-KARAR (libsodium-primary): SUPERSEDED — erken-dönem-varsayımı; proje-saf-Rust'a-evrildi.
```

---

## BÖLÜM-4.5: DÖRT-ZORUNLU-TEST-KATMANI [K-WIRE-NONCE96-1'in-katman-ayrımı]
```
KATMAN-1  RFC-8439-vector'ları        → primitive-doğruluğu (full_pn→nonce96-mapping'ini-TEST-ETMEZ)
KATMAN-2  nonce-mapping-vector'ları   → full_pn→EXACT-nonce96-bytes (0, 1, 2^32, 2^63 zorunlu)
KATMAN-3  RustCrypto==libsodium-diff  → implementation-independence (aynı-{key,nonce,AAD,pt}→ciphertext+tag-byte-eşit)
KATMAN-4  full-wire-golden            → reconstructed-full_pn + nonce96 + AAD + pt + ct + tag + outer/inner-framing
Self-generated-golden-TEK-ORACLE-SAYILMAZ (yalnız-davranış-sabitliği-kanıtlar, doğruluğu-DEĞİL).
```

## BÖLÜM-4.6: AGENT-DETERMİNİSTİK-GÖREV-SINIRI + İLK-ÇIKTI-FORMATI
İlk-aşamada-YALNIZ: (1)legacy-nonce-inventory (2)D2-exact-historical-evidence (3)ciphertext/vector-analysis
(4)compatibility-debt-determination (5)LE/BE/INCONCLUSIVE-classification (6)DR-5-ADR-update (7)oracle-altyapısı.
**YASAK:** kanıtsız-endian-seçmek · Sprint-4'e-erken-başlamak · Sprint-5'e-başlamak · main'e-atomik-olmayan-merge ·
runtime-crypto-provider · golden'ı-tek-oracle-saymak · eski-artefakt-bulunamayınca-VARSAYIM-yapmak.
**İLK-ÇIKTI (`reports/decision-requests/DR-4-envanter.md`):** gerçek-legacy-AEAD-impl-var-mı? · crate/kod-yolu ·
nonce-mapping-exact-bytes · D2-exact-metni · eski-vector'lar · vector-classification ∈ {PROVES_LE64, PROVES_BE64,
AMBIGUOUS, NO_VECTOR} · compatibility-debt ∈ {YES, NO, INCONCLUSIVE} · recommendation ∈ {RECOMMEND_LE64,
RECOMMEND_BE64, INCONCLUSIVE} · changed-files · tests · risks. **SON-SATIR-ZORUNLU:**
"CHECKPOINT: Awaiting/Applied Abkarada DR-4 conditional lock. Sprint-4 status explicitly reported. Sprint-5 remains HOLD."
KOŞUL-A/B-KESİN-oluşuyorsa-otomatik-kilit-uygulanır; KOŞUL-C'de-agent-DURUR.

## BÖLÜM-5: PR/SPRINT STATÜLERİ [özet — checkpoint-analiziyle-hizalı]

```
PR #42  : şartlı-HOLD → DR-4-spec-patch'i-main'e-girmeden-TEK-BAŞINA-merge-EDİLMEZ; iki-değişiklik {aynı-PR}
          VEYA {merge-order-zorunlu-stacked-zincir}-içinde-ardışık-kesintisiz-main'e-girer (normatif-nonce-boşluğu-
          main'de-BIRAKILMAZ).
PR #43  : draft/unmergeable (DR-4 + N1-legacy-interop kapanana-dek). İş-adı: "D1-preflight/implementation-POC".
Sprint-4: DR-4-kapanışı-sonrası-GO (session/HANDLE_ASSIGN/dh_epoch-machine/INIT-flight/restart).
Sprint-5: HOLD (ek-şartlar: N1-legacy-interop-kapanışı + PR#43-cross-crate-entegrasyon-testleri + DR-4-spec-
          patch + independent-nonce-oracle + Sprint-4-session/state-testleri — mühürlü/paylaşılan-engine-koduna-
          dokunduğu-için-daha-yüksek-gate).
RsSimdEngine: kalıcılaştırma M4/M5-allocation-ölçümüne-bağlı (count/bytes/peak/p50-p95-p99/churn/stale-fuzz/one-shot).
```
