# DR-CS-1 TUR-A1 — H1-DESEN DENETİMİ (F-A1-3)

**Tarih:** 2026-07-26 · **Faz:** F-A1-3 (DUR-B'den bağımsız) · **Yöntem:** `rg` aday-toplama + SEMANTİK-inceleme.
**Desen:** BUG-H1 sınıfı = `X=$(func)` içinde arka-plan-işi (`cmd &`) STDOUT'u yönlendirilmemiş → `$()` komut-
ikamesi, arka-plan-işi ÇIKANA-KADAR bloklar (rx/tx-örtüşmez). **Ayırıcı-kriter:** background-job `$()` İÇİNDE mi
+ stdout `$()`-borusuna mı bağlı.

---

## 1. ADAY TARAMASI (tüm scripts/*.sh)
14 script tarandı. Üç aday-sınıfı: (1) `$( ... & ... )` satır-içi, (2) `X=$(runrx/rx ...)`, (3) `cmd & VAR=$!`.

## 2. BULGU + SINIFLANDIRMA (her satır tam-bir-sınıf)

| dosya:satır | desen | sınıf | gerekçe |
|---|---|---|---|
| run_dr_cs1_wire.sh:27 | `runrx(){ faithful_rx ... >/dev/null 2>&1 & echo $!; }` | **SAFE** | stdout YÖNLENDİRİLMİŞ → `$(runrx)` bloklamaz (a6d199b-fix) |
| run_dr_cs1_wire.sh:32/43/59 | `RX=$(runrx ...)` | **SAFE** | runrx stdout-redirect'li; `$()` yalnız `echo $!` yakalar |
| run_dr_cs1_w2.sh:27/35 | `runrx(){...>/dev/null 2>&1 &...}` + `RX=$(runrx ...)` | **SAFE** | aynı fix; W-2B A1-harness'ı |
| run_wallclock_pins.sh:38-40 | `faithful_rx ... 2>/dev/null & local RX=$!` | **SAFE** | DOĞRUDAN-background (`cmd & RX=$!`); `$()` İÇİNDE-DEĞİL → ikame-borusu-yok → blok-yok |
| run_rootless_netns.sh:34-35 | `$BIN --role rx ... & RX=$!` | **SAFE** | doğrudan-bg; `$()`-yok |
| run_rootless_contended.sh:26 | `$BIN --role rx ... & RX=$!` | **SAFE** | doğrudan-bg; `$()`-yok |
| run_policy_validation.sh:61-62 | `$SUDO ip netns exec ... $BIN --role rx ... & RXPID=$!` | **SAFE** | doğrudan-bg; `$()`-yok |
| `$(uname -r)` · `$(tc -V 2>&1)` · `$(ping ... 2>&1 \| grep)` | ADAY-1 | **SAFE** | `2>&1` = REDIRECT (background-job DEĞİL) |
| `$(awk -F, 'NR>1 && $2==0 ...')` | ADAY-1 | **SAFE** | `&&` = awk-mantıksal-AND (single-quote içinde; shell-bg-DEĞİL) |

## 3. NEDENSELLİK-NOTU (neden doğrudan-bg SAFE)
`cmd & VAR=$!` deseninde `&` işi arka-plana atar, kabuk HEMEN devam eder; `cmd`'in stdout'u kabuğun-stdout'una
(log/terminal) gider — bir `$()` ikame-borusuna DEĞİL. BUG-H1 YALNIZ `$()`-ikamesi arka-plan-işin stdout-fd'sini
(borusunu) tuttuğunda oluşur (rx-ömrü-boyunca EOF-beklenir). Doğrudan-bg'de böyle bir boru YOKTUR → blok-yok.
Tek gerçek-BUG-H1 örneği (runrx `$(...)` içinde, stdout-yönlendirmesiz) `a6d199b`'de DÜZELTİLDİ.

## 4. DUR-B TETİK KONTROLÜ (talimat §3)
Tetik YALNIZ: mevcut-A1-harness VEYA yaşayan-status-board-kanıtı VEYA mevcut-closure-hüküm-taşıyan-script
etkileniyorsa. **Etkilenen CURRENT-EVIDENCE-SUSPECT: YOK** (tüm bulgular SAFE). run_dr_cs1_wire/w2 (current-A1/
closure) FIXED-SAFE; eski policy/wallclock/rootless script'leri doğrudan-bg-SAFE.

## HÜKÜM
> **H1 current-evidence audit: CLEAN.**

HISTORICAL-not: tek-tarihsel-BUG-H1 (runrx-substitution-block) `a6d199b`-öncesi vardı; düzeltildi + izole-
tekrar-üretimle-kanıtlandı (F4-closure §3.1). CURRENT-EVIDENCE-SUSPECT sıfır → A1 H1-tarafından BLOKLANMAZ.
