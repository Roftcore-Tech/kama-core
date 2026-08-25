# BİTİRME YOL HARİTASI — 2026-08-18

**Amaç:** projeyi hızla ve istendiği şekilde bitirmek. Bu belge üç şeyi taşır: sürecin dürüst
teşhisi, elimizdekilerin güncel envanteri, ve çıktı-odaklı kısa bitirme planı.
Kaynaklar bu turda **tek tek elle okundu** (agent taraması değil): `AI-HANDOFF-CURRENT.md`,
`kama-transport-roadmap-status-board.md`, `v2-benchmark-gorev-r5-kosuya-hazir.md`,
`ADIM-6-coldstart-gorev.md`, `EVIDENCE-LEDGER-2026-08-15.md` (+erratum), bench crate,
udp_runner, netem scriptleri, git geçmişi.

---

## 1. SÜREÇ TEŞHİSİ — 6 hafta nereye gitti

**Ham sayılar (git, 354 commit, 7 Tem–18 Ağu):**

| tür | commit | oran |
|---|---:|---:|
| docs | 153 | **%43** |
| feat | 114 | %32 |
| test | 51 | %14 |
| fix | 19 | %5 |

Rework maliyeti: **12** corrigendum/errata commit'i · **11** supersession/withdrawal ·
**15** corrigendum/errata dosyası · **84** dosyada SUPERSEDED/WITHDRAWN/INVALIDATED işareti.

**Fazlar:**

| dönem | iş | teslimat |
|---|---|---|
| 7–16 Tem | temel + quinn baseline | quinn 70/70 referansı ✅ |
| 21–28 Tem | FS-1 + ürün cold-start (A2) | FS-1 v1.3.2 ✅ · CLOSURE-1B/2 ✅ |
| 30 Tem–4 Ağu | R1/R2A + ürün tail doğrulaması | **GO** (2290 satır) ✅ + el-devri |
| **12–15 Ağu** | **DR-CS-3/M2 doğrulama zinciri** | **sapma** — aşağıda |
| 15–18 Ağu | rota düzeltme + füzyon/Q18/estimator | 6 gerçek bulgu ✅ |

**Kök neden — tek cümle:** 4 Ağustos el-devri §I'da sonraki iş açıkça yazılıydı
(*"Comparison harness arms: KAMA-OFF, KAMA-TAIL-V1/ON, Quinn, TCP, KCP"*), ama sonraki iki hafta
cold-start **enstrüman doğrulama zincirine** gitti. Son haftada (12–18 Ağu) 52 commit'in yalnız
**7'si** ürün koduna dokundu; 25'i docs, 17'si test. DR-CS-3 tek başına 12+ addendum/audit/contract
dosyası üretti; 5 atomik karttan 3'ü BLOCKED döndü; `/tmp`'ye kaybedilen bir komparatör 3 kurtarma
kartına mal oldu.

**Korunacak disiplin:** corrigendum kültürü gerçek hataları yakaladı (p5 paraziti, R2A bütçe
geri-çekmesi, Q18, S-0.7 asimetrisi, V1 flag-ON kırılması). **Bırakılacak:** çok-katmanlı enstrüman
doğrulama zincirleri (kart-başına-addendum, denetim-denetimi), kampanya-başına 5+ meta-belge.
Bundan sonra: kampanya başına **tek** belge + **tek** karşıt inceleme; sonuç üreten araç sonuçla
aynı commit'te; her yeşil kilometre taşı anında commit.

---

## 2. ELİMİZDEKİLER — güncel envanter (habersiz olduğumuz sonuçlar dahil)

| # | soru | durum | sayı | kaynak |
|---|---|---|---|---|
| 1 | quinn baseline (3 rejim) | **PROVEN** | 70/70; p2/p3/p5 c120=0.00–0.27 | `docs/results/quinn-baseline-v1.md` |
| 2 | Kama-vs-quinn v1 | **ZEHİRLİ** | p5 1.00 → false-loss paraziti; fix sonrası 24/100 | `time-model-v1.md` §0.7 |
| 3 | Ürün tail probe | **GO** | OFF 8/360 (%2.2) → ON 329/360 (%91.4); tek tuning hücresi L10-med | `dr-arq-product-tail-1/FINAL-CHECKPOINT.md` |
| 4 | Coding vs blind-dup | **PROVEN** | 868–0 eşleşmiş bütçede | R2A §C3 |
| 5 | FEC'in kuyruk avantajı | **PROVEN** | çoğu ARQ kuyruk-körlüğüydü (C2); füzyon hücresi doğrudan gösterdi | R2A §C2 + `fs-fusion-cell-1` |
| 6 | Füzyon hücresi (tail+r2) | **PROVEN** | kuyrukta coding=0 katkı; IID'de 0/12→9/12; **hiç zarar yok** (tail_only=0) | `fs-fusion-cell-1/FINAL-REPORT.md` |
| 7 | IID'de ARQ katkısı | **PROVEN** | coding ARQ harcamasını %7–20 düşürüyor | `ARQ-CONTRIBUTION.csv` |
| 8 | Kayıp ORANI FEC'i kapılayamaz | **PROVEN** | Q17 holdout zararlı yanlış-pozitif; aynı 200k ppm'de yapıya göre zıt cevap | `fs-fusion-cal-1/FINAL-REPORT.md` §5 |
| 9 | Q18 kalibrasyon kusuru | **PROVEN+pinli** | %20 kayıpta parity=0; diz 250k'da | `q18_check.rs` + `Q18-CALIBRATION-DEFECT.md` |
| 10 | LOSS_MAX skaler olamaz | **PROVEN** | onset 3-eksenli yüzey; sembol kaynakta yok | `S-ANSWERS-STRUCT-GATE.md` |
| 11 | Estimator yapıyı ayırıyor | **PROVEN** ⚠uncommitted | FATE2ARM: IID +600 vs GE-L3 +567k, 20/20, boşluk ~433k ppm | `ESTIMATOR-LIFECYCLE.md` §7 |
| 12 | Cold-start ürün durumu | **PARTIAL** | tek eksik hücre (iii) = gerçek Noise (Sprint-6 mühürlü) | status-board + `dr-cs-1b-a2-dur-son.md` |
| 13 | FEC emisyon zamanlaması | **PARTIAL** | parity-at-block-close yüksek RTT'de avantajı siliyor; rtt250 pozitif | `d1-exit/FS-wallclock-fec-utility.md` |
| 14 | Deadline | **PROVEN-negatif** | hiçbir kol sıkı deadline'ı tutmuyor; ürün SLO'su yok | R2A corrigendum §1 |
| 15 | Bench harness | **HAZIR** | quinn+kama adaptörleri, metrics, CLI komple | `crates/bench/` |
| 16 | **r5 benchmark görevi** | **HAZIR ama untracked** | Abkarada'nın kendi otoriter kampanya belgesi, kapılarıyla | `docs/tasks/v2-benchmark-gorev-r5-kosuya-hazir.md` |

**"Farkında olmadıklarımız" listesi:** #16 (koşuya-hazır benchmark görevi untracked duruyordu) ·
#15 (bench crate'in kama adaptörünün de bitmiş olduğu) · #2'nin kesin zehirlenme kaydı ·
el-devri §I'ın zaten yazılmış bitirme sırası · udp_runner'ın feature-gated bench yolu olduğu
(ürün UDP döngüsü DEĞİL).

---

## 3. YOL HARİTASI — çıktı odaklı, 5 adım

Hedef sıralaması el-devri §I + senin r5 belgen. İkisi aynı yere işaret ediyor: **benchmark → rapor
→ white paper.** Default-flip kararları zaten §4.4-benchmark-sonrası'na bağlı (status-board satır 52).

### R0 — Bankayı güvene al *(bugün, ~1 saat)*
Uncommitted S-0.7 + FATE2ARM işini commit+push et (SIGNAL-SEPARATES sonucu şu an kaybolabilir
durumda). r5 görevini, ADIM-6'yı ve struct-gate tasarım belgelerini track et.
**Çıktı:** temiz working tree, banka tam.

### R1 — Tail-v1'i adlandır ve dondur *(yarım gün)*
Doğrulanmış ON konfigürasyonunu (cap=3, 10ms/10s/30s) üründe adlandırılmış sabit yap
(`set_tail_recovery_config` yolu; default flip YOK). El-devri kuralı: karşılaştırmadan önce
post-hoc tuning YOK — L10-med tuning'i product-v2 backlog'unda kalır.
**Çıktı:** benchmark'ın KAMA-TAIL-V1/ON kolu çağrılabilir.

### R2 — r5 BENCHMARK KAMPANYASI *(ana iş, 3–5 gün)*
Senin r5 belgenin kapılarıyla: ADIM-0 spec-sync → (rebrand mekanik PR) → KAPI-1 tasarım
(adaptörler HAZIR: `crates/bench`; netem: `scripts/netem/` + `faithful_harness.sh`) →
KAPI-2 ön-kayıt → koşu → rapor. Kollar: KAMA-OFF · KAMA-TAIL-V1/ON · quinn rolleri
(Q-FEC-ISOLATION/REFERENCE dahil). Zehirli v1 sayıları bu kampanyayla resmen değişir.
FEC bu kampanyada **Manual/OFF** kalır (r5'in FEC-aynı-implementation kilidi; Q18 kusuru
Controller modunu zaten yasaklıyor).
**Çıktı:** hakem-dayanıklı karşılaştırma raporu — projenin dış teslimatı.

### R3 — White paper + pano güncellemesi *(2 gün)*
r5 §C iddia-dili kilitleriyle; status-board (26 Tem'de kalmış) ve el-devri (4 Ağu) tek geçişte
güncellenir. Envanterdeki 16 satır kanıt tabanı.
**Çıktı:** DCP/ASELSAN'a giden belge seti.

### R4 — Sprint-6 + ürün runtime *(benchmark sonrası)*
Noise handshake (cold-start hücre-iii'ü kapatır) → PRODUCT-RUNTIME items 2–9 (UDP döngüsü,
lifecycle, drain, process-level e2e) → default-flip kararları (artık benchmark kanıtı var) →
FEC yapı-kapısı kalibrasyonu (estimator ayrımı bankada; Prereg-A enstrümantasyon dalıyla).
**Çıktı:** sevk edilebilir ürün.

**Toplam tahmin: benchmark raporu + white paper taslağına ~7–9 iş günü.**

### Süreç kuralları (bitiş dönemi için bağlayıcı)
1. Kampanya başına TEK belge + TEK karşıt inceleme. Addendum zinciri yok.
2. Sonuç üreten araç, sonuçla aynı commit'te. `/tmp`'de kanıt tutulmaz.
3. Her yeşil kilometre taşı ANINDA commit+push.
4. Bankadaki soruyu yeniden ölçme (el-devri §I: FS-1/R2A/product grid re-run YASAK).
5. Makine güvenliği aynen: `-j2`, tek cargo, ağır kapılar hosted CI'da.
6. Ön-kayıt yalnız result-bearing koşular için; enstrüman işi için değil.

---

# REVİZYON 2026-08-24 — ÜRÜN-GRADE PLANI (kanıt fazı kapandı; inşa fazı)

## Neredeyiz (hedefe göre)

**Hedef (Abkarada):** transport bir ARAÇ; ürün onun üstüne inşa edilecek → **product-grade ready**.

| katman | durum |
|---|---|
| Motor mekanizmaları (ARQ/pacing/rate/tail/FEC-codec/estimator) | ✅ bileşen düzeyinde doğrulanmış |
| **Kanıt fazı (r5 FAZ-1)** | ✅ **KAPANDI** — p2 KAMA-SUPERIOR +38pp (Holm), rapor `d677ab6b`, white-paper v0.1 |
| Exploratory / u0.9 dolgusu / FAZ-2 / sub-kbps | ⏸️ BACKLOG (iddia-gücü işi, ürün-grade için gerekli DEĞİL) |
| **Ürün-grade eksikleri** | ❌ aşağıdaki P0–P5 |

**Bugünkü kritik bulgu:** exploratory h3 (250 kbit/s, GE-burst %40-60) → Kama %86-97 vs Quinn %100.
Kama **patlamalı kayıpta rakibin gerisinde.** Mekanizma hipotezi: bankadaki bilinen tuning açığı
(`DR-ARQ-PRODUCT-TAIL-1` "terminal-tail L10-med 0/30": sabit cap-3 probe, ardışık kayıpta
packet-threshold ARQ devreye girmeden tükeniyor) tam burst imzası. Bu artık ürün P0'ı.

## Ürün-grade READY tanımı (çıkış kriterleri — hepsi ölçülebilir)

1. Gerçek Noise handshake uçtan uca + Split-türevi yön anahtarları (cold-start hücre-iii kapanır).
2. Üretim UDP çalışma-zamanı: bağlantı yaşam-döngüsü, drain, shutdown/hata/kaynak-sınırı; süreç-düzeyi
   e2e testi netem altında yeşil (PRODUCT-RUNTIME 2–9).
3. Varsayılanlar kanıtla karara bağlanıp pinlenmiş (tail, startup-pacing, bootstrap, FEC).
4. Burst-erasure regresyon kapısı yeşil (P0 düzeltmesi; r5 M: motor fix = yeni commit, kampanya re-run YOK).
5. CI: fmt/clippy/test(default+all-features)/miri/deny/audit + fuzz-smoke yeşil; reachable `todo!()` sıfır;
   SR-AUDIT-001 bounded-alloc; PARSER-02/03 kapalı.
6. `kama-contract` yayımlanmış: API yüzeyi ürün ekibi (client/media/signaling) için dondurulmuş.

## Sıralı iş (solo; 1 sprint ≈ 1 hafta)

| # | iş | içerik | çıkış kriteri | sprint |
|---|---|---|---|---|
| **P0** | Burst-erasure kurtarma | GE-burst'ta teşhis (mevcut fs harness + tail probe diag); adaptif cap/backoff (product-v2 backlog kalemi) veya threshold-ARQ tetik düzeltmesi; regresyon kapısı = h3-benzeri netem hücresinde kama-tail ≥ q-dgram | kapı yeşil; pin testli | 1 |
| **P1** | Sprint-6 Noise | `crypto` crate (3 satır→gerçek): libsodium X25519/ChaCha20-Poly1305 (pki §13 registry), Noise pattern (spec), Split → yön anahtarları; `kama-noise-wire` ile birleşir; CLOSURE-1B'nin TEST-ONLY anahtarı gerçek türevle değişir | cold-start hücre-iii PASS; A2 closure gerçek anahtarla tekrar | 1,5–2 |
| **P2** | Runtime kabuğu | PRODUCT-RUNTIME 2–9: UDP loop, lifecycle, Sender↔ReceiverControlCore wiring, drain, e2e, shutdown/resource | süreç-düzeyi e2e netem'de yeşil; kaynak sınırı testli | 1,5 |
| **P3** | Default-flip paketi | tail-v1 ON (kanıt: random-loss nötr, kuyrukta GO, P0 sonrası), startup-pacing (EK4-A2 kararı), bootstrap OFF kalır, FEC: OFF (structure-gate kalibrasyonu ürün-v2) | pinlenmiş defaults + değişim gerekçesi tek belge | 0,5 |
| **P4** | Sertleştirme | PARSER-02/03, cargo-fuzz (ccr-codec, ack, fec-codec, noise-wire), bounded-buffer denetimi, CI'ya fuzz-smoke | CI yeşil, fuzz 0 crash (N saat) | 1 |
| **P5** | kama-contract + API dondurma | transport API + signaling seam sözleşmesi; UniFFI yüzeyi taslağı (ADR-15) | belge + ürün ekibi onayı | 0,5 |
| | **TOPLAM** | | | **~6 sprint** |

Sıra gerekçesi: P0 ürünün en görünür zayıflığı (ölçüldü); P1 tek başına "PARTIAL" etiketini kapatır ve
en belirsiz kalem (erken başlamalı); P2 P1'e bağımlı (anahtarlar); P3 P0+P2 sonrası anlamlı;
P4/P5 paralel-doküman/CI işi, koşu gerektirmez.

## BACKLOG (ürün-grade için gerekli DEĞİL; iddia-gücü/eksiksizlik)

- Exploratory Grup-B tamamlama: 204 rep-kol (`manifest-exploratory.jsonl` resume; ~15 h) + u=0.9 karantina
  dolgusu 14 koşu (`--only u0.9`). Makine stabilken tek gece.
- FAZ-2 FEC kolları (KAMA-FEC-S, Q-FEC-ISOLATION `kama-fec-codec`, Q-FEC-REFERENCE `raptorq`) + E kapıları
  — "QUIC mimarisine üstünlük" cümlesinin tek yolu; ~1,5 sprint; iş-değeri: pazarlama/hakem, ürün değil.
- Sub-kbps süpürmesi (h1 + Grup-C 9 hücre; sınıf-III kuralları yavaş).
- BENCH-01 blackout-survival ikili metriği (claim-gate).
- FEC yapı-kapısı kalibrasyonu (Prereg-A + estimator'ü tüketen kural) — ürün-v2.
- ADR-16 ↔ gerçek FEC crate uyuşmazlığı notu (reed-solomon-erasure yazıyor, kod reed-solomon-simd).

## Süreç kuralı (uzamayı kesmek için)
Her P-kalemi: tek belge (varsa) + tek regresyon kapısı + anında commit. Yeni kampanya YOK; ölçüm yalnız
kapı-testi düzeyinde (dakikalar). Kanıt-fazı bitti; docs/feat oranı ters çevrilir.
