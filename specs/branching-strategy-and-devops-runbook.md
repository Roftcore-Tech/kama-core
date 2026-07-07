# SafeRoom — Branching Stratejisi & DevOps Kurulum Runbook

**Belge:** `branching-strategy-and-devops-runbook.md` v1.0
**Kime:** Yaaz (DevOps Lead) + Abkarada (core) + Resul (media)
**Amaç:** Geliştirmeye BU AKŞAM başlamak. Bölüm A-D = strateji (policy); **Bölüm E = adım-adım executable kurulum**; Bölüm F = bu akşamki sıralı checklist.
**Referans:** ADR-01 (Rust), ADR-12 (workspace monorepo), ADR-18 (netem gate), ADR-19 (musl+systemd deploy), ADR-08 (HSM/PKCS#11).

---

## Bölüm A — Ekip & Repo Modeli

### A.1 Görev dağılımı (kilitli)

```
Abkarada  → saferoom-core TAMAMI (solo): ccr-codec, crypto, adaptive-transport (reliability),
            hub-server, client-core, ra-ca, discovery, admin-server
Resul     → saferoom-media: WebRTC fork transport (real-time; geç veri çöp; özel FEC/ARQ)
Hasan     → Lyra codec'i WebRTC'ye entegre + API tasarımı (media repo'da)
Yaaz      → DevOps Lead (her iki repo, CI/CD, runner, harness)
Hayri     → rebranding, logo, isimlendirme, UI/UX
Meriç     → UI/UX, branding
```

**İki transport, iki zıt taban:** core-transport = *recovery-obsesif* ("veri ULAŞSIN", deadline-bounded ama ulaşma birincil). media-transport = *latency-obsesif* ("geç paket at gitsin"). Bu yüzden iki ayrı düzlem, iki ayrı repo.

### A.2 Repo topolojisi

```
saferoom-core     (Abkarada, SOLO)      Cargo workspace (ADR-12), private
saferoom-media    (Resul + Hasan)       WebRTC fork + Rust FFI, private
[saferoom-contract → ERTELENDİ]         transport quinn/custom kararı (§4.4) + signaling API
                                        stabilleşince eklenir. Şimdi tanımlamak erken optimizasyon.
```

**Neden 2 repo (tek değil):** WebRTC fork Cargo workspace'ine **giremez** — gn/ninja/depot_tools + milyonlarca satır C++. Aynı repoda olsalar core CI'ı (~10 dk) fork build'i (saatler) yüzünden felç olur. Ayrı repo = ayrı CI hızı, ayrı bağımlılık dünyası.

**Neden contract IMPLEMENTATION ertelendi (ama seam DÜŞÜNCESİ değil):** transport'un quinn mi custom mu olacağı benchmark'la (§4.4) belirlenene kadar seam API'yi dondurmak = belirsiz şeyin üstüne kontrat = iki kez iş. AMA **sıfır temas da riskli** — media aylarca ayrı gidip signaling'e bağlanınca lifecycle uyuşmazlığı çıkar (core signaling'i şöyle taşıyor / media SDP-ICE'ı farklı varsaymış / app bağlayamıyor). Çözüm: contract repo/API ertelensin, ama **`docs/signaling-seam-v0.md`** ŞİMDİ açılsın (API dondurmaz; yalnız minimal seam varsayımını yazar — §E.7, ayrı dosya teslim edildi). Bu fazda repo'lar bağımsız çalışır; cross-repo entegrasyon TESTİ yok ama seam VARSAYIMI hizalı.

### A.3 İki repo ilerideki entegrasyon — BİRLEŞME DEĞİL, BAĞLANMA

> Kritik zihinsel model: iki repo **hiçbir zaman tek repo'ya kaynatılMAZ.** Ayrı kalırlar; çalışma zamanında **bağlanırlar** (kütüphane linki gibi — tıpkı bir uygulamanın `libsodium`'un kaynağını kopyalamayıp derlenmiş çıktısına link vermesi gibi). Kod tabanları ayrı; media, core'un **çıktısını** tüketir.

```
BULUŞMA NOKTASI = TEK: signaling.
  Bir sesli arama kurulurken:
    1. Alice → Bob: "seni aramak istiyorum" + SDP offer  ┐
    2. ICE candidate değişimi                            ├─ HEPSİ core'dan geçer (core signaling taşır)
    3. parametre anlaşması                               ┘
         ↓ anlaşma bitince
    4. ses/video akışı başlar  ─────────────────────────── media (WebRTC fork) devralır
  Yani core "teli döşer", media "akışı taşır". Arayüz ince: send_signaling(peer, sdp_blob) / on_signaling(blob).

TEKNİK BAĞLAMA (muhtemel yol):
  - core kendini kütüphane olarak derler (Rust → .so/.a veya UniFFI binding, ADR-15).
  - media o çıktıya link verir, send_signaling()'i çağırır (RingRTC'nin Signal app'ine bağlanması gibi).
  - ASIL birleşme ÜÇÜNCÜ yerde olur: mobil UYGULAMA (app) katmanı HEM core HEM media kütüphanesini
    import edip orkestra eder. Birleşme noktası core/media repo'su DEĞİL, onları tüketen app'tir.
    (Hayri/Meriç UI tarafı buraya oturur; app repo'su entegrasyon fazında kurulur.)

ZAMAN ÇİZELGESİ (bağlama EN SONDA — şimdi DEĞİL):
  ŞİMDİ (aylar)     : iki repo SIFIR temas; sen core, Resul media — birbirinizi beklemezsiniz.
  transport kararı  : §4.4 benchmark quinn/custom → signaling API stabilleşir → saferoom-contract tanımlanır.
  entegrasyon fazı  : app katmanında core+media kütüphaneleri import + signaling seam → uçtan uca arama.
```

**Özet:** Ağır bir "merge" operasyonu YOK. İki repo ayrı kalır; media core'un binary çıktısına link verir; asıl orkestrasyon app katmanındadır; ve bu **en sonda**, transport kararı netleşince olur. Şimdi bu bağlamayı düşünmene gerek yok — core'a odaklan.

---

## Bölüm B — Branching Stratejisi

### B.1 Model: trunk-based (git-flow DEĞİL)

Google/Meta gerçeği: tek trunk, kısa ömürlü branch, sürekli yeşil main. git-flow (develop/release/hotfix) zamanlı release için tasarlandı; sürekli AR-GE'ye yanlış ve solo geliştirici için ölüm.

```
- Tek main branch. HER ZAMAN yeşil + release-edilebilir.
- Kısa ömürlü feature branch: saatler – MAX 2 gün. Uzun branch = merge cehennemi.
- main'e DIRECT PUSH KAPALI → PR + CI yeşil zorunlu (branch protection).
- Merge = squash (temiz history).
- Yarım modül → feature-flag + **typed-error stub** arkasında main'de (reachable `todo!()` YOK — §B.4; uzun branch DEĞİL).
```

### B.2 Branch isimlendirme

```
feat/<alan>-<kısa-açıklama>     feat/adaptive-transport-arq, feat/ccr-hubentry-v3
fix/<alan>-<kısa-açıklama>      fix/ccr-parser-oom
chore/<açıklama>                chore/ci-add-miri
docs/<açıklama>                 docs/engine-spec-fec
```

### B.3 Commit convention (Conventional Commits)

```
feat: adaptive-transport selective ARQ (SR-09)
fix: ccr HubEntryV3 SAN match bounds-check
chore: add cargo-deny to CI
docs: engine spec §7 delivered-rate estimator
test: TV-BW golden vectors
refactor: extract ErasureCodec trait
```
Her commit atomik + küçük. Cargo.lock DAİMA commit.

### B.4 SOLO-CORE için kritik disiplin: API-FIRST

Tek başına 8 crate → kendi kendinin bottleneck'i olma riski. Foundation crate'lerini (`crypto`, `ccr-codec`) **API-first** yaz:

```
1. Public trait imzalarını + tipleri main'e ÖNCE indir (içerik typed-error stub olsa bile — reachable `todo!()` YOK).
2. İmzaları DONDUR (semver). Diğer crate'ler (hub-server, client-core) stub API'ye karşı DERLENİR.
3. İç implementasyonu stable imza arkasında doldur → tek noktadan tıkanma YOK.
```

**Feature flag + STUB kuralı** (yarım kodu main'de yeşil tutmak — ama RUNTIME-SAFE):

> **NORMATİF — reachable `todo!()` YASAK.** `todo!()` derlenir ama runtime'da PANİK eder. `cargo test --all --all-features` feature açıkken yarım kodu compile+reachable path'e sokar → `todo!()` paniği CI'ı/uygulamayı düşürür. Kural: **public API stub OLABİLİR, ama erişilebilir path'te `todo!()` OLMAZ** — stub **typed error** döner. `todo!()` yalnız private spike/deneme branch'inde kalır, main'e GELMEZ.

```toml
# crates/adaptive-transport/Cargo.toml
[features]
default = []
arq_selective     = []
fec_reed_solomon  = []
emergency_forward = []
```
```rust
// TransportError'da açık bir "henüz implemente değil" varyantı:
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("feature not implemented: {0}")]
    FeatureNotImplemented(&'static str),
    // ... diğer hatalar
}

// DOĞRU stub — panik YOK, açık typed error:
pub fn selective_retransmit(&mut self, _sack: &AckFrame) -> Result<(), TransportError> {
    Err(TransportError::FeatureNotImplemented("arq_selective"))   // SR-09
}

#[cfg(feature = "arq_selective")]
pub fn selective_retransmit_impl(&mut self, sack: &AckFrame) -> Result<(), TransportError> {
    /* gerçek implementasyon; feature açılınca public fn buna delege eder */
    Ok(())
}

// YANLIŞ (main'de OLMAZ): pub fn ... { todo!("...") }   ← reachable panic
```
main her zaman derlenir/yeşil kalır; yarım fonksiyon çağrılırsa **panik değil, açık hata** döner (test edilebilir, debug edilebilir). Feature olgunlaşınca stub gerçek impl'e delege eder.

### B.5 Versiyonlama / tag

```
Her milestone: git tag -a v0.1.0-alpha -m "..."
Semver: 0.MINOR.PATCH-alpha (Alpha boyunca 0.x).
```

### B.6 saferoom-media branching (Resul + Hasan)

```
main + kısa feature branch. 2 kişi → BİRBİRLERİNİ review eder (PR approval).
WebRTC upstream PIN'li (§E.4); SafeRoom patch'leri patches/ altında ayrı tutulur.
```

---

## Bölüm C — CI/CD Pipeline

### C.1 Core CI (GitHub-hosted; her PR) — `.github/workflows/ci.yml`

```yaml
name: core-ci
on:
  push: { branches: [main] }
  pull_request:
jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: "clippy, rustfmt" }
      - uses: Swatinem/rust-cache@v2        # derleme cache (hız)
      - run: cargo fmt --all -- --check                                  # format kapısı
      - run: cargo clippy --all-targets --all-features -- -D warnings    # lint = HATA
      - run: cargo test --all --all-features                            # unit + conformance vektörleri
      - run: cargo build --release --all
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-audit cargo-deny --locked
      - run: cargo audit          # CVE taraması
      - run: cargo deny check      # lisans + advisory (ADR §4)
  miri:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with: { components: miri }
      - run: cargo miri test -p crypto -p ccr-codec    # unsafe/FFI sınırı UB kontrolü
```

### C.2 Nightly transport benchmark (SELF-HOSTED = senin makinen) — `.github/workflows/bench-nightly.yml`

netem/tc **privileged** ister → GitHub-hosted'da kısıtlı → senin makinende self-hosted runner.

```yaml
name: transport-bench-nightly
on:
  schedule: [{ cron: "0 2 * * *" }]     # her gece 02:00
  workflow_dispatch:                     # elle tetik
jobs:
  netem-bench:
    runs-on: [self-hosted, linux, bench]   # senin makinen (§E.3 etiketi)
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release -p bench
      - run: sudo ./harness/run-netem-profiles.sh   # 7 hostile profil (§4.4.2)
      - run: cargo run --release -p bench -- --vs quinn-bbr --profiles all --out results/
      - run: python3 scripts/gen_completion_chart.py results/   # iki-eğri grafik (§4.4.11)
      - uses: actions/upload-artifact@v4                        # ham veri + grafik
        with: { name: bench-results, path: results/ }
      # regresyon (bir profilde düşüş) → exit 1 → build FAIL (§4.4 gate)
```

### C.3 Media CI (ayrı; Resul + Yaaz)

```
Ayrı workflow — fork build AĞIR (depot_tools cache + gn gen + ninja = uzun).
  PR'da: incremental build + media unit test (signaling'siz, loopback).
  Nightly: full clean build.
Senin core CI'ından TAMAMEN bağımsız; birbirini bloklamaz.
```

### C.4 Branch protection (main)

GitHub → repo → Settings → Branches → Add rule → Branch name pattern: `main`
```
☑ Require a pull request before merging
☑ Require status checks to pass:  quality · security · miri
☑ Require branches to be up to date before merging
☑ Do not allow bypassing the above settings
(Squash merge: Settings → General → Pull Requests → yalnız "Allow squash merging" açık)
```
`gh` ile (opsiyonel, tek komut):
```bash
gh api -X PUT repos/OWNER/saferoom-core/branches/main/protection \
  -f "required_status_checks[strict]=true" \
  -f "required_status_checks[contexts][]=quality" \
  -f "required_status_checks[contexts][]=security" \
  -f "required_status_checks[contexts][]=miri" \
  -F "enforce_admins=true" \
  -F "required_pull_request_reviews[required_approving_review_count]=0" \
  -F "restrictions="
```
(Solo core → required review = 0; ama CI zorunlu. crypto crate'e kendi kuralın: 24 saat soğuma + kendi diff-review'un.)

---

## Bölüm D — Docker & Test Harness

Docker'ı **deployment için DEĞİL** (ADR-19: production = musl statik-binary + systemd), iki yerde kullan:

### D.1 netem iki-uç test harness (ŞART — §4.4.9 reproducibility)

```yaml
# harness/docker-compose.yml
version: "3.8"
services:
  hub:
    build: { context: ., dockerfile: harness/Dockerfile }
    cap_add: [NET_ADMIN]          # tc/netem için
    command: ./target/release/hub-server --config /etc/saferoom/hub.toml
  client:
    build: { context: ., dockerfile: harness/Dockerfile }
    cap_add: [NET_ADMIN]
    depends_on: [hub]
    command: ./harness/run-client-bench.sh
```
```dockerfile
# harness/Dockerfile
FROM rust:1.83-slim
RUN apt-get update && apt-get install -y iproute2 iperf3 python3   # tc netem, ölçüm
WORKDIR /app
COPY . .
RUN cargo build --release -p hub-server -p bench
```
```bash
# harness/run-netem-profiles.sh — 7 hostile profil (§4.4.2)
set -e
IF=eth0
apply() { tc qdisc replace dev $IF root netem "$@"; }
# P1: loss %50 / düşük bant
apply loss 50% rate 100kbit
# P2: loss %80
apply loss 80% rate 200kbit
# P3: yüksek jitter / düşük bant
apply delay 200ms 100ms distribution normal rate 100kbit
# P4: reverse-path impaired (dönüş yolu) — ikinci qdisc / ayrı interface
# P5: intermittent blackout — döngüde qdisc ekle/kaldır
# ... her profil için bench koş, sonucu results/'a yaz
```

### D.2 CI toolchain image (reproducibility)
```
Pinned Rust + lib sürümleri tek image → herkes aynı derleme ortamı.
rust-toolchain.toml zaten sürümü pinler; image bunu paketler.
```

---

## Bölüm E — BU AKŞAM: Adım-Adım Executable Kurulum

> Ön koşul: `git`, `gh` (GitHub CLI, `gh auth login`), `rustup`, `docker`. Sırayı takip et.

### E.1 — saferoom-core repo (Abkarada) ⏱️ ~20 dk

```bash
# 1. Workspace iskeleti
mkdir saferoom-core && cd saferoom-core
git init && git branch -M main

# 2. Workspace Cargo.toml
cat > Cargo.toml <<'EOF'
[workspace]
resolver = "2"
members = [
  "crates/crypto",
  "crates/ccr-codec",
  "crates/adaptive-transport",
  "crates/hub-server",
  "crates/client-core",
  "crates/ra-ca",
  "crates/discovery",
  "crates/admin-server",
  "crates/bench",
]

[workspace.package]
edition = "2021"
license = "Proprietary"
version = "0.1.0"
EOF

# 3. Toolchain pin (reproducibility)
cat > rust-toolchain.toml <<'EOF'
[toolchain]
channel = "1.83.0"
components = ["clippy", "rustfmt"]
EOF

# 4. cargo-deny config
cat > deny.toml <<'EOF'
[advisories]
yanked = "deny"
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "Unicode-DFS-2016"]
[bans]
multiple-versions = "warn"
EOF

# 5. .gitignore
cat > .gitignore <<'EOF'
/target
**/*.rs.bk
Cargo.lock.orig
/results
EOF

# 6. Crate stub'ları (API-first: her biri lib, foundation'lar önce)
for c in crypto ccr-codec adaptive-transport hub-server client-core ra-ca discovery admin-server; do
  cargo new --lib "crates/$c" --name "$(echo $c | tr - _)"
done
cargo new --bin crates/bench --name bench

# 7. İlk derleme (yeşil taban)
cargo build --all

# 8. İlk commit
git add -A && git commit -m "chore: workspace skeleton + toolchain/deny/gitignore"

# 9. GitHub'a (PRIVATE — self-hosted runner güvenliği)
gh repo create saferoom-core --private --source=. --remote=origin --push
```

### E.2 — Core CI workflow ⏱️ ~10 dk

```bash
mkdir -p .github/workflows
# Bölüm C.1'deki ci.yml içeriğini buraya yaz:
cat > .github/workflows/ci.yml <<'EOF'
<Bölüm C.1 içeriği>
EOF
git checkout -b chore/ci-setup
git add .github && git commit -m "chore: add core CI (fmt/clippy/test/audit/deny/miri)"
git push -u origin chore/ci-setup
gh pr create --fill                 # CI'ın yeşil olduğunu gör → merge
gh pr merge --squash --delete-branch
```
Sonra branch protection'ı kur (Bölüm C.4 — UI veya `gh api` komutu).

### E.3 — Self-hosted runner (senin makinen) ⏱️ ~15 dk

GitHub → saferoom-core → Settings → Actions → Runners → **New self-hosted runner** → Linux.
GitHub sana TAM komutları verir (token gömülü); şuna benzer:
```bash
mkdir actions-runner && cd actions-runner
curl -o actions-runner-linux-x64.tar.gz -L <GITHUB'IN_VERDİĞİ_URL>
tar xzf actions-runner-linux-x64.tar.gz
./config.sh --url https://github.com/OWNER/saferoom-core --token <TOKEN> \
            --labels self-hosted,linux,bench --name bench-box
# Servis olarak (arka planda sürekli):
sudo ./svc.sh install
sudo ./svc.sh start
```
Doğrula: Settings → Runners'da "bench-box • Idle" görünmeli. Nightly workflow (`runs-on: [self-hosted, linux, bench]`) artık senin makinende koşar.

> **NORMATİF — Self-hosted runner hardening (ZORUNLU; runner = ciddi saldırı yüzeyi):**
```
□ YALNIZ private repo (fork PR makinende kod çalıştıramaz — public'te asla).
□ Repo-scoped runner (org-wide DEĞİL) — yalnız saferoom-core/bench repo'suna bağlı.
□ Normal PR CI self-hosted'da KOŞMAZ — yalnız nightly + workflow_dispatch bench.
  (bench-nightly.yml zaten schedule/dispatch; quality/security/miri GitHub-hosted'da kalır.)
□ Ayrı Linux user (ör. `saferoom-runner`) veya ayrı VM/container — ana kullanıcıdan izole.
□ sudo YALNIZ netem script için, NOPASSWD allowlist ile (tüm sudo DEĞİL):
    # /etc/sudoers.d/saferoom-runner
    saferoom-runner ALL=(root) NOPASSWD: /opt/saferoom/harness/run-netem-profiles.sh
□ GitHub token/secrets MİNİMUM — bench için gereken dışında secret verme.
□ Runner labels: self-hosted, linux, bench (workflow bu üçüyle eşleşir; yanlış job düşmez).
□ Runner makinesi ağ-izole tercih (bench trafiği dış ağa sızmasın).
```
Kısaca: privileged netem gücü yalnız gece bench'e, ayrı user altında, dar sudo ile; PR CI'ı asla bu makinede koşmaz.

### E.4 — saferoom-media + WebRTC FORK (Resul; henüz yapılmadı) ⏱️ ~30 dk kurulum + uzun ilk build

WebRTC'yi olduğu gibi commit'lemeyiz (milyonlarca satır). **RingRTC modeli:** sürümü PIN'le, depot_tools ile fetch et, patch'leri üstte tut.

```bash
# 1. depot_tools (WebRTC'nin build araç zinciri)
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
export PATH="$PWD/depot_tools:$PATH"    # kalıcı: ~/.bashrc'ye ekle

# 2. WebRTC checkout (BELİRLENEN sürüm)
mkdir webrtc-checkout && cd webrtc-checkout
fetch --nohooks webrtc
gclient sync
cd src
# Belirlediğiniz milestone'a geç (branch-heads/XXXX = seçilen sürüm):
git checkout -b saferoom-fork branch-heads/<PINNED_MILESTONE>
gclient sync -D          # o sürümün bağımlılıklarını çek

# 3. İlk build testi (gn + ninja) — bu UZUN sürer (ilk sefer saatler)
gn gen out/Default
ninja -C out/Default
```

```bash
# 4. saferoom-media repo iskeleti (thin — WebRTC'yi PIN'le, tree'yi commit ETME)
cd ../.. && mkdir saferoom-media && cd saferoom-media && git init && git branch -M main
echo "branch-heads/<PINNED_MILESTONE>" > webrtc.version   # TEK doğruluk kaynağı
mkdir -p patches src bin
cat > bin/fetch-webrtc.sh <<'EOF'
#!/usr/bin/env bash
# depot_tools ile webrtc.version'daki sürümü çek + patches/ uygula
set -e
REV="$(cat webrtc.version)"
# fetch + checkout $REV + for p in patches/*.patch: git apply
EOF
chmod +x bin/fetch-webrtc.sh
cat > .gitignore <<'EOF'
/webrtc-checkout
/out
/target
EOF
git add -A && git commit -m "chore: media skeleton + pinned webrtc.version (RingRTC-style)"
gh repo create saferoom-media --private --source=. --remote=origin --push
```

```
saferoom-media/
  webrtc.version        # PIN'li sürüm (commit'li, tek kaynak)
  patches/              # SafeRoom özel FEC/ARQ/codec patch'leri (Resul + Hasan)
  src/                  # Rust FFI wrapper (RingRTC modeli)
  bin/fetch-webrtc.sh   # depot_tools fetch + checkout + apply patches
```
Böylece repo İNCE kalır; WebRTC pin'li sürümden fetch edilir. Hasan Lyra'yı `patches/` + `src/` API'sinde geliştirir.

### E.5 — netem harness iskeleti (Yaaz) ⏱️ ~20 dk

```bash
cd saferoom-core
mkdir -p harness scripts
# Bölüm D.1 dosyalarını yaz: harness/docker-compose.yml, harness/Dockerfile, harness/run-netem-profiles.sh
chmod +x harness/run-netem-profiles.sh
# Lokal test:
docker compose -f harness/docker-compose.yml build
# (bench crate + profiller olgunlaşınca `docker compose up` iki-uç ölçüm)
git checkout -b chore/netem-harness
git add harness scripts && git commit -m "chore: netem two-endpoint harness skeleton"
git push -u origin chore/netem-harness && gh pr create --fill
```

### E.6 — İlk gerçek feature akışı (örnek; workflow'u kanıtla) ⏱️ ~10 dk

```bash
# API-first: crypto public trait imzalarını indir (typed-error stub — reachable todo!() YOK)
git checkout -b feat/crypto-api
# crates/crypto/src/lib.rs:
#   pub trait Aead { fn seal(...) -> Result<Vec<u8>, CryptoError>; ... }
#   stub gövde: Err(CryptoError::NotImplemented("aead_seal"))  ← panik değil
cargo build --all         # yeşil (stub'lar derlenir)
git add -A && git commit -m "feat: crypto public trait API (Aead/Kdf/Dh typed-error stubs, SR-01)"
git push -u origin feat/crypto-api
gh pr create --fill       # CI yeşil → merge
gh pr merge --squash --delete-branch
```
Bu akıştan sonra tüm geliştirme aynı döngü: kısa branch → CI yeşil → squash merge.

### E.7 — signaling-seam-v0 doc (contract ertelendi ama seam düşüncesi DEĞİL) ⏱️ ~10 dk

Contract repo/API ertelendi (§A.2) ama **sıfır temas riskli** — media aylarca ayrı gidip signaling seam'e bağlanınca lifecycle uyuşmazlığı çıkar. Çözüm: API dondurmayan, yalnız **minimal seam varsayımını** yazan bir doc'u ŞİMDİ aç (ayrı `docs/signaling-seam-v0.md` dosyası; runbook'la birlikte teslim edildi).

```bash
cd saferoom-core && mkdir -p docs
cp <teslim>/signaling-seam-v0.md docs/     # aşağıda üretilen dosya
git checkout -b docs/signaling-seam-v0
git add docs && git commit -m "docs: signaling seam v0 (minimal assumptions, API NOT frozen)"
git push -u origin docs/signaling-seam-v0 && gh pr create --fill
```
Media ekibi bu doc'u okur → SDP/ICE lifecycle varsayımını core'unkiyle hizalar. Contract implementation transport kararı (§4.4) sonrası; ama seam düşüncesi kayıt altında.

---

## Bölüm F — Bu Akşamki Sıralı Checklist

> Gerçekçi ayrım: WebRTC "ilk build başlat" bile ortam sorunlarıyla saatlerce sürebilir. Aşağıdaki **Must-tonight** dar tutuldu ki ekip ilk gece "plan tuttu" hissiyle çıksın; gerisi yarına kayabilir (borç değil, plan).

```
MUST TONIGHT (dar, kesin biter)                     KİM        SÜRE
────────────────────────────────────────────────────────────────────
 1  saferoom-core workspace + push (E.1)            Abkarada   20 dk
 2  Basic core CI + PR merge (E.2)                   Abkarada   10 dk
 3  Branch protection (C.4)                          Abkarada   5 dk
 4  saferoom-media repo SKELETON (E.4/4)             Resul      15 dk
 5  WebRTC version PIN (webrtc.version) (E.4/2)      Resul      5 dk
────────────────────────────────────────────────────────────────────
 → Sonuç: iki repo ayakta, core CI yeşil, branch protection aktif, media iskeleti + sürüm pin'li.

CAN SLIP TO TOMORROW (uzun/kırılgan — yarın engelsiz)  KİM        SÜRE
────────────────────────────────────────────────────────────────────
 6  Self-hosted runner + HARDENING (E.3)             Abkarada   30 dk
 7  netem harness iskeleti (E.5)                     Yaaz       20 dk
 8  WebRTC FULL build validation (E.4/3)             Resul      saatler
 9  signaling-seam-v0 doc (E.7)                       Abkarada   10 dk
10  İlk feature PR: crypto API (E.6)                 Abkarada   10 dk
────────────────────────────────────────────────────────────────────
İpucu: 8 (WebRTC build) uzun → BU AKŞAM komutu başlat (arka planda gece koşsun), doğrulaması yarın.
```

### Yaaz toplantı gündemi (sıra)
```
1. İki repo + branch protection kararını onayla (A.2, B.1)
2. Core CI workflow'u birlikte yaz + ilk PR'ı yeşil gör (E.1-E.2)
3. Self-hosted runner'ı Abkarada'nın makinesine bağla (E.3)
4. Resul'a WebRTC fork komutlarını ver, ilk build'i başlat (E.4)
5. netem harness'ı Yaaz devral (E.5) — yarın olgunlaştır
```

---

## Ek — Kararların Gerekçe Özeti (hızlı referans)

```
2 repo (tek değil)        : WebRTC fork Cargo workspace'ine girmez (gn/ninja); CI hızı ayrışması.
contract ertelendi        : transport quinn/custom kararı (§4.4) öncesi seam API dondurmak = 2× iş.
trunk-based (git-flow yok) : sürekli AR-GE + solo geliştirici; kısa branch + yeşil main.
API-first (solo kritik)    : foundation trait'leri önce → kendi kendinin bottleneck'i olma.
feature-flag + typed stub  : yarım modül main'de yeşil; reachable todo!() YOK (panik değil, typed error); uzun branch yok.
self-hosted nightly        : netem/tc privileged → senin makinen; ucuz CI GitHub-hosted.
Docker = test+CI, deploy≠  : ADR-19 production musl+systemd; Docker yalnız netem harness + toolchain.
WebRTC pin + patches       : RingRTC modeli; tree commit etme, sürüm pin'le + fetch + patch.
private repo               : self-hosted runner güvenliği (fork PR kod çalıştırmasın).
```

**Statü:** BU AKŞAM UYGULANMAYA HAZIR. Bölüm E komutları sırayla çalıştırılır; `<PINNED_MILESTONE>` = seçilen WebRTC sürümü, `OWNER` = GitHub org/kullanıcı.

**Freeze durumu:** GO (Conditional GO'nun 3 patch'i uygulandı — (1) `docs/signaling-seam-v0.md` açıldı, contract düşüncesi ertelenmedi; (2) reachable `todo!()` → typed `NotImplemented` error; (3) self-hosted runner hardening normatif). Operational GO for initial setup + long-term DevOps freeze GO.

> **Küçük not (sonra optimize):** `cargo audit`/`cargo deny` her CI run'ında install ediliyor (çalışır ama yavaş). İleride: bunları önceden kurulu bir CI toolchain image'ına al veya `Swatinem/rust-cache` ile binary cache. Şimdilik doğruluk > hız; olduğu gibi bırak.
