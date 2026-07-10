# quinn Baseline v1 — §4.4 Ölçüm Sonuçları

> **Amaç:** Kama adaptif transport'un yeneceği referans. Bu, "iyi ağın QUIC'i" (quinn 0.11.11,
> pure-Rust, en-iyi-mevcut §4.4.1) 7 hostile profilde ne yapıyor sorusunun **ölçülmüş** cevabı.
> ADIM-3 (engine loop) hedefini ve ADIM-5 (Kama vs quinn killer-chart) referansını buradan alır.
>
> **Durum:** 70/70 koşu tamam · Faz D empirik sanity temiz · tüm iddialar lab-verifiable.
> Tarih: 2026-07-10 · quinn 0.11.11 · 5 repeat/hücre · deadlines {10, 30, 120}s.

---

## TL;DR — QUIC'in operasyonel haritası

QUIC üç net rejimde yaşıyor:

| Rejim | Koşul | completion(120s) | Kama için anlam |
|-------|-------|:---:|---|
| ① **AYAKTA** | ≤%2 kayıp, jitter, aralıklı blackout | **1.00** | Eşit kal (§4.4.5 dürüstlük) |
| ② **SALLANIYOR** | %50 kayıp | **0.15** (cubic) / bimodal (bbr) | **Kazan** |
| ③ **ÖLÜ** | ≥%80 kayıp, ACK-yolu çöküşü @ kısa deadline | **0.00** | **Kategorik kazan** |

**En kritik bulgu:** ACK-yolu bozulunca (p5_revpath) QUIC 10s/30s deadline'da **tam sıfır** teslim
ediyor; %50 kayıpta sonuç bir **yazı-tura** (bazı koşu 1.00, bazısı 0.00). Bu iki nokta Kama'nın
determinist forward-recovery tezinin doğrudan hedefi.

---

## 1. Metodoloji

### 1.1 Ölçülen
- **Birincil (§4.4.3):** deadline-bounded completion — `latency ≤ deadline` olan mesaj oranı,
  deadline ∈ {10, 30, 120}s. Her hücre 100 mesaj × 256 B, 5 tekrar (ayrı seed).
- **İkincil:** handshake başarı oranı, first-delivery, p50/p95 latency, delivered oranı,
  bytes/delivered-msg, ACK/dönüş-yolu overhead.
- **Varyans (§4.4.9):** 5 tekrar → medyan + IQR + min/max. Tek koşu YASAK — aşağıdaki bimodallik
  tam da bu yüzden.

### 1.2 Ölçüm aracının doğrulanması — "ölçmeden önce aracı doğrula"

Baseline'a güvenmeden önce araç doğrulandı; süreçte **3 ölçüm-bozucu bug** bulunup düzeltildi
(hiçbiri derlemede yakalanmaz, hepsi sessizce yanlış sayı üretirdi):

1. **`delivered_ns` frame bug** (PR #12): mesaj-başı süre mutlak-an yerine kaydediliyordu →
   completion eğrisi düz → QUIC hatalı olarak "kusursuz" görünürdü. Düzeltildi + loopback guard testi.
2. **Server sıralı stream (HoL) + ALPN eksik + idle 30s** (PR #12, ultracode review, 8 bulgu):
   ölçülen QUIC değil bench-app darboğazı olurdu / tüm handshake fail / eğri 30s'de kesilirdi.
3. **Handshake bounded değil** (PR #13): %80 kayıpta handshake 2m17s asılı kaldı (300s idle'a kadar).
   `connect()` + `wait_idle()` deadline'la sınırlandı; bağlanamama = geçerli **0-completion** sonucu
   (§4.4.3 handshake metriği), agregaya dahil. Test: `handshake_timeout_is_bounded`.

Doğrulama kanıtı: araç **üç seviyeyi de** doğru ölçtü — 1.00 (kayıpsız), 0.15 (ara, %50), 0.00
(handshake fail, %95). Faz D: tüm latency'ler değişken, gradation monoton. Stuck-at-0/1 değil.

### 1.3 Reproducibility (§4.4.9)
```
image   sha256:7a30f6f2326b5e95106561c1fbc89e66d96a7ef5080d95fd23e3a1f868181a6b
netem   sha256:293bb306d37df83d0f8897c80aba61aa4082ec50e69a05d4c7117790e67b0290
quinn   0.11.11   ·   repeats 5   ·   deadlines 10,30,120s   ·   100 msg × 256 B
```
Profiller (per-container egress netem, `specs/transport-layer-tdd.md §4.4.2`):
`p1_loss50` %50+100kbit · `p2_loss80` %80+200kbit · `p3_loss95` %95+100kbit ·
`p4_jitter` 200±100ms+100kbit · `p5_revpath` fwd %5 / **ACK %80** (asimetrik) ·
`p6_blackout` 2s kesinti/8s normal (dinamik) · `p7_bbr` %2+50kbit.

---

## 2. Ana sonuç — completion vs deadline (medyan, 5 repeat)

| profil | koşul | cc | c(10s) | c(30s) | c(120s) | IQR(120s) | hs% | deliv% |
|--------|-------|-----|:---:|:---:|:---:|:---:|:---:|:---:|
| p7_bbr | %2 kayıp | cubic | **1.00** | 1.00 | 1.00 | [1.0,1.0] | 100 | 100 |
| p7_bbr | | bbr | **1.00** | 1.00 | 1.00 | [1.0,1.0] | 100 | 100 |
| p4_jitter | 200±100ms | cubic | **1.00** | 1.00 | 1.00 | [1.0,1.0] | 100 | 100 |
| p4_jitter | | bbr | **1.00** | 1.00 | 1.00 | [1.0,1.0] | 100 | 100 |
| p6_blackout | aralıklı | cubic | **1.00** | 1.00 | 1.00 | [1.0,1.0] | 100 | 100 |
| p6_blackout | | bbr | **1.00** | 1.00 | 1.00 | [1.0,1.0] | 100 | 100 |
| p1_loss50 | %50 kayıp | cubic | 0.13 | 0.15 | **0.15** | [0.04,0.18] | 100 | 27.6 |
| p1_loss50 | | bbr | 0.00 | 0.00 | **0.00** | **[0.0,1.0]** | 100 | 40.0 |
| p5_revpath | ACK %80 | cubic | 0.00 | 0.00 | **0.27** | [0.0,0.34] | 60 | 22.2 |
| p5_revpath | | bbr | 0.00 | 0.00 | **0.00** | **[0.0,1.0]** | 40 | 40.0 |
| p2_loss80 | %80 kayıp | cubic | 0.00 | 0.00 | **0.00** | [0.0,0.0] | 20 | 1.6 |
| p2_loss80 | | bbr | 0.00 | 0.00 | **0.00** | [0.0,0.0] | 60 | 2.4 |
| p3_loss95 | %95 kayıp | cubic | 0.00 | 0.00 | **0.00** | [0.0,0.0] | **0** | 0.0 |
| p3_loss95 | | bbr | 0.00 | 0.00 | **0.00** | [0.0,0.0] | **0** | 0.0 |

`hs%` = handshake başarı oranı (bağlantı kurulan koşu / 5). `deliv%` = agregat teslim (delivered/500).

---

## 3. Üç rejim — detay

### ① AYAKTA (completion 1.00): p7_bbr, p4_jitter, p6_blackout
QUIC iyi ağda, saf jitter'da ve *aralıklı* blackout'ta her mesajı teslim ediyor. Latency değişken
(700ms–6s tipik). **Kama bu bölgede QUIC'i yenmek zorunda değil — eşit kalması yeter** (§4.4.5).

### ② SALLANIYOR (%50 kayıp): kısmi + yüksek varyans
- **cubic:** medyan %15 (c120), delivered %27.6. Hızlı ilk-teslim (241ms) ama uzun kuyruk (p95 48s).
- **bbr:** medyan %0 ama **bimodal** (§5). Geç ilk-teslim (10s).
- cubic > bbr: konservatif CC %50 kayıpta daha tutarlı kısmi teslim veriyor.

### ③ ÖLÜ (completion 0.00): p2_loss80, p3_loss95, p5_revpath@kısa-deadline
- **%80 kayıp:** bağlantı ara sıra kuruluyor (hs %20-60) ama teslim ~%0 (1-2%). Bağlanmak ≠ teslim.
- **%95 kayıp:** **handshake %0** — QUIC bağlantıyı *kuramıyor bile*. Tam ölü.
- **p5_revpath:** aşağıda ayrı başlık — asıl hedef.

---

## 4. 🎯 Killer bulgu — p5_revpath (ACK-yolu %80 kayıp)

Forward yolu neredeyse temiz (%5), ama **dönüş/ACK yolu %80 bozuk**. QUIC ACK-clocked: ACK'ler
kaybolunca congestion control aç kalır, RTT tahmini bozulur.

```
cubic:  c(10s)=0.00   c(30s)=0.00   c(120s)=0.27    hs=60%   p95 latency=86s
bbr:    c(10s)=0.00   c(30s)=0.00   c(120s)=0.00    hs=40%   p95 latency=62s
```

- **Kısa deadline'da (10s/30s) tam SIFIR.** Teslim olan mesajlar bile 120s'e varan (p95 86s!) gecikmeyle geliyor.
- **Handshake bile yazı-tura** (hs %40-60): dönüş yolu bozukken bağlantı kurmak çoğu zaman başarısız.
- per-repeat c(120s): cubic `[0.0, 0.34, 0.27, 0.0, 0.5]`, bbr `[0.0, 0.0, 0.0, 1.0, 1.0]`.

Bu, `specs`'teki **ack_confidence çöküşü → EMERGENCY_FORWARD_REDUNDANCY** senaryosunun ölçülmüş
kanıtı. Kama'nın forward-redundancy'si (ACK'e bağımlı olmayan proaktif FEC) burada QUIC'i
**kategorik** yener — QUIC'in 0.00'ına karşı Kama'nın kısa-deadline'da pozitif completion'ı.

---

## 5. Varyans & bimodallik — "neden ≥5 repeat + IQR" haklı çıktı

Tek koşu ölçseydik yanıltıcı olurdu. Per-repeat c(120s):

| profil / cc | per-repeat completion | medyan | okuma |
|---|---|:---:|---|
| p1_loss50 bbr | **[1.0, 0.0, 0.0, 0.0, 1.0]** | 0.00 | **temiz yazı-tura** — %40 tam, %60 sıfır |
| p5_revpath bbr | **[0.0, 0.0, 0.0, 1.0, 1.0]** | 0.00 | bağlanırsa hepsi, bağlanamazsa hiç |
| p1_loss50 cubic | [1.0, 0.04, 0.18, 0.15, 0.01] | 0.15 | dağınık kısmi, bir koşu 1.0 |
| p5_revpath cubic | [0.0, 0.34, 0.27, 0.0, 0.5] | 0.27 | 3/5 bağlandı, kısmi teslim |

**Çıkarım:** %50 kayıp ve ACK-çöküşü rejiminde QUIC per-koşu davranışı çoğu zaman "bağlan-ve-hepsini-
ver YA DA tümüyle-başarısız" — medyan bunu gizler, IQR açığa çıkarır. Bu determinizm eksikliği,
Kama'nın satış argümanının çekirdeği: **QUIC bir kumar, Kama öngörülebilir**.

---

## 6. Handshake başarı oranı — yeni boyut (§4.4.3)

completion'dan ayrı, "QUIC bağlantıyı kurabildi mi":

```
%2 / jitter / blackout / %50 kayıp :  hs 100%   (bağlantı sağlam)
%80 kayıp                          :  hs 20-60% (kırılgan)
ACK-yolu %80                       :  hs 40-60% (dönüş yolu bağlantıyı da vuruyor)
%95 kayıp                          :  hs 0%     (bağlantı İMKÂNSIZ)
```
%95 kayıpta QUIC teslim etmemekten öte **el sıkışamıyor bile** — Kama'nın 0-RTT + FEC'li
SESSION_INIT'inin fark yaratacağı en uç nokta.

---

## 7. cubic vs bbr

| | cubic | bbr | not |
|---|---|---|---|
| %50 kayıp (c120) | 0.15 medyan | bimodal (0/1) | cubic daha tutarlı |
| %50 first-delivery | 241ms | 10062ms | cubic çok daha hızlı ilk teslim |
| %80 kayıp hs% | 20 | 60 | bbr daha agresif bağlanıyor |
| ACK-yolu (c120) | 0.27 | 0.00 | cubic ACK-çöküşünü daha iyi kaldırıyor |

**Sonuç:** iyi ağda ikisi de 1.00. Hostile'da **cubic (default) genelde daha dayanıklı** — bbr'nin
"Experimental! Use at your own risk" etiketi (quinn kaynağı) sonuçlarla tutarlı. Baseline referansı
olarak **cubic** alınmalı; bbr yalnız cross-check.

---

## 8. Latency profili (delivered mesajlar, ms)

| profil/cc | first | p50 | p95 | max |
|---|---:|---:|---:|---:|
| p7_bbr cubic | 511 | 3300 | 5395 | 6375 |
| p4_jitter cubic | 718 | 2236 | 3315 | 3374 |
| p6_blackout cubic | 0 | 2 | 28901 | 28902 |
| p1_loss50 cubic | 241 | 5393 | 48428 | 48428 |
| p5_revpath cubic | 387 | 21657 | 86473 | 86473 |

p6_blackout'ta çoğu mesaj anında (p50 2ms) ama kuyrukta biri blackout boyunca ~29s bekliyor.
p5/p1'de p95 onlarca saniye — QUIC'in retransmit-until-deadline davranışı.

---

## 9. Faz D empirik sanity — TEMİZ ✓
- Tüm delivered latency'ler **sıfır-olmayan + değişken** → frame doğru (delivered_ns bug'ı yok).
- Gradation **monoton**: 1.00 (p7/p4) → 0.15 (p1_loss50) → 0.00 (p2/p3). Loss↑ ⇒ completion↓, fiziksel.
- Düz-eğri / netem-uygulanmama şüphesi yok. Araç güvenilir.

## 10. Caveat'ler (overclaim yok)
1. **p6_blackout=1.00 yanıltıcı okunmamalı.** Bu *aralıklı* blackout (2s/8s); idle-timeout 300s ≫ 2s
   → kolay hayatta kalınır. **Sürekli** blackout survival ölçülmedi (`CLAUDE.md §10 BENCH-01`).
   "quinn blackout-proof" DENEMEZ.
2. Mutlak sayılar **bu makine + bu netem profilleri**ne özgü; rejim yapısı (①②③) taşınabilir, absolut
   completion donanıma göre kayabilir.
3. bbr "Experimental" — production karşılaştırması cubic üzerinden yapılmalı.

## 11. ADIM-3 hedefleri (kantitatif — Kama engine bunu yenmeli)

| Cephe | QUIC baseline | Kama hedefi |
|---|---|---|
| **p5_revpath c(10s)** | 0.00 | **> 0** (kategorik kazanç — ACK'e bağımsız FEC) |
| **p5_revpath c(120s)** | 0.27 (cubic) | **> 0.27**, tercihen ≫ |
| **p1_loss50 c(10s)** | 0.13 (cubic) | **> 0.13**, determinist (IQR dar) |
| **p2_loss80 c(120s)** | 0.00 | **> 0** |
| **p3_loss95 handshake** | %0 | **bağlantı kur** (0-RTT + FEC init) |
| **p7/p4/p6 (iyi ağ)** | 1.00 | **1.00'ı KORU** (kaybetme) |

Ana metrik: **determinizm**. Kama'nın kazancı yalnız daha yüksek medyan değil, **dar IQR** —
QUIC'in yazı-turasını öngörülebilir teslime çevirmek.

## 12. Nasıl tekrarlanır
```bash
sudo modprobe sch_netem
docker compose -f harness/docker-compose.yml up -d --build
bash harness/run-bench.sh                 # 7×2×5, ~82 dk (bounded)
python3 harness/report.py                 # konsolide + Faz D
python3 harness/analyze.py                # zengin metrik → results/analysis.json
```
Ham veri `results/*.jsonl` (gitignore); özet `results/*_summary.txt`; log `results/matrix-run.log`.
