# F1 uygulama planı (2026-10-30)

**Durum:** öneri. F1 sahip onayıyla başlar. Seçilen yol F0 sonucuna göre kademelidir: önce Aşama 1, ölçüm, sonra karar kapısı.
**Taban:** main `a858f3fc`. 05b bütün olarak alınmaz (#140 taslak kalır); yalnız F1-M6 ve F1-M8 kuralları tek tek taşınır.
**Beklenen etkiler [tahmin]dir;** dayandığı ölçülen sayı yanında yazılıdır. Süreler ajan-gün, ölçüm hariç. Her satır F0 şerit raporundaki ilgili F1 maddesine bağlanır.

## Kurallar

- cargo yalnız orkestratörde. Ajanlar açık dosya yoluyla commit eder. Her iş ayrı dal, `main` yeşil kalır.
- `sender.rs`'e üç ajan dokunuyor: merge sırası M1 → M2 → M3, ajanlar birbirinin dalını taban alır.
- INV-RATE-1 korunur: hız düzeltmeleri kayıp körüdür; kayıp sinyali hızı düşürmez, yükseltmez.
- Her iş kendi birim testini ve mutasyon denetimini getirir. Bench kolu ve `f0_flows` ölçüm kodu aynı kalır.

## Sahip onayı gereken maddeler

1. Aşama 1'in başlaması.
2. **Altın vektör:** F1-M3 INV-RATE-1 altın değerlerini günceller. Bugünkü "tel ve altın vektörler değişmez" kuralı için istisna gerekir.
3. **05b parçaları:** F1-M6 (RTT örnek kuralı) ve F1-M8 (prob devri) tek tek alınır.
4. **Aşama 2:** S1 §4, S1 teyit sinyali, AMP §3 ve `s1.rs` dokunuşları. Bugünkü "s1.rs'e dokunulmaz" kuralı için ayrıca onay gerekir.
5. K-3 ölçütü: ideal ARQ'nun p95'i 14,5 s, ölçüt 12 s [türetilmiş]. Sayı sahip tarafından yeniden belirlenir.

## Aşama 1 — tel ve sözleşme değişmez

### Ajan 1 — client-core / kama-node (dar)

| # | dosya | değişiklik | bağımlılık | ölçü, beklenen etki [tahmin], dayanak | süre |
|---|---|---|---|---|---|
| F1-D1 (B1) | `client-core/runtime.rs:2554-2569` | PARAM_SET yeniden deneme aralığı 200 ms × 2^k; tavan 25 kalır | yok | K-3: r25 teslim 0,167 → 0,67–1,00. Dayanak: PARAM hattın %44–63'ü; ters oynatım [türetilmiş, geri beslemesiz] | 0,5 |
| F1-D2 (A1) | `runtime.rs` `server_task`, `push_delivery`, `fec_ingest` | DATA'da ACK'ten önce `try_reserve`, yer yoksa ACK'siz düşür; FEC'de `RECOVERED` için aynı kural; `delivery_refused` sayacı | yok | K-1, K-10: teslim kanalı düşüşü 32–1630 birim/koşu → 0. `Reorder` atlaması ayrı kalıcı kayıp yolu olarak kalır | 1,5 |
| F1-D3 (A5) | `kama-node/node.rs` | Dinleyiciyi sınırlı kuyrukla düğüm iş parçacığından ayır; tür başına tavan + düşüş sayacı | kama-mobile API notu | K-1 durma terimi +16 s (betimsel) → ~0; TEXT tüketicisi ayrı ve kuyruk taşmıyorsa | 1,5 |
| F1-D4 (C7) | `runtime.rs:3232` `fec_ingest` | Tamamlanan bloğu sınırlı hatırla | yok | CPU; son tarih etkisi beklenmez | 0,5 |

### Ajan 2 — motor hızı (`rate.rs`, `bw.rs`, `sender.rs`)

| # | dosya | değişiklik | bağımlılık | ölçü, beklenen etki [tahmin], dayanak | süre |
|---|---|---|---|---|---|
| F1-M1 (B5) | `rate.rs:101` | `min_cwnd` 4 → 10 × MTU | yok | K-2, K-6: RTT 2 s geç çıkan 286–298 → ≈ 60/600; p95 3,4–3,6 → ≤ 2 s. Tam etki M4'e bağlı | 0,5 |
| F1-M2 (B6) | `sender.rs:4208` | Uygulama sınırlı turu Startup platosu saymamak | yok | K-1: düşük tepe çöküş 6 / 5 → 0. F1-S2 buna bağlı | 1 |
| F1-M3 (C1) | `sender.rs:2361` `on_event_core` | Uygulama sınırlı akışa pacer alt sınırı: son pencere sunulan + yeniden gönderilen × (1 + pay), `cwnd_over_srtt` tavanlı | M1, M2 | K-6, K-2: kilit 185–249/330 → < %5; arq ≤ 800 ms 0,20 → ≈ 0,80 (kilitsiz 0,815, n = 7) | 2 |
| F1-M4 (B4) | `sender.rs` `update_state`, `compute_send_rate`; `bw.rs` | Backlog + düşük kuyruk gecikmesinde RecoverArq'ta ProbeUp kazancı; BtlBw düşüşünü durdurma; işaret qd | M1–M3 | K-1, K-2: r12k l30 kilit %65–77 → %8–11; çöküş 12–14/225 → 0; quinn farkı (32,5 ↔ 8,7 s) daralır | 3 |

### Ajan 3 — motor kurtarma (`tail_recovery.rs`, `sender.rs`)

| # | dosya | değişiklik | bağımlılık | ölçü, beklenen etki [tahmin], dayanak | süre |
|---|---|---|---|---|---|
| F1-M5 (B3) | `tail_recovery.rs:155-169`, `sender.rs:3188` | PTO'ya sabit 25 ms yerine pencereli azami `ack_delay` | yok | K-7: r1k kayıpsız gereksiz yük %35 → ≤ %5; tel/teslim 1,78 → ≈ 1,2. Risk: gerçek kuyruk kaybında kurtarma gecikir | 1 |
| F1-M6 (B7) | `sender.rs` | 05b RTT örnek kuralını tek başına almak (aday `:3449-3460`, `:3609-3620`) | M5 ile birlikte | K-2: r1k 1 KB p95 −8…−12 s (bütün adayın farkı; madde ayrı ölçülmedi) | 0,5 |
| F1-M7 (B2) | `sender.rs` `drain_establish_probe`, `tail_recovery.rs` `base_pto_ns` | Kuruluş probu ve örneksiz PTO için `rtt_floor`; üst kıskaç max(10 s, 2 × srtt) | D1 | K-3: p50 5,7–7,5 s, l20 p95 11,8–14,7 s (kâhin kopya üst sınırı) | 1,5 |
| F1-M8 (A2) | `sender.rs` | Prob bütçesi tükenince teyitsiz birimi kopya kuyruğuna devret (05b, aday `:7164-7165`) | yok | K-4, K-8: kuyruk sonu takılması 4/235 → 0 (aday 0/235; fark gösterilmedi) | 1 |

### Ajan 4 — FEC yolu

| # | dosya | değişiklik | bağımlılık | ölçü, beklenen etki [tahmin], dayanak | süre |
|---|---|---|---|---|---|
| F1-F1 (C2) | `sender.rs:4292-4311`, `:5319`, `:2366-2381` | Son tarihli blok paritesi DATA'dan hemen sonra, ARQ'dan önce; kova tabanı parite başını kapsar | M3 | K-6: rs2 RTT 600 ms %20 0,83 → 0,93–0,98 | 2 |
| F1-F2 (C3) | `client-core/runtime.rs:2503` | Ses oturumunda r'yi ölçülen kayıptan tabloyla seç; §8.5 atlanır | F1 | K-6: parite 0 → plan (K, r); bant 1,5–2,1 × arq | 1 |

### Ajan 5 — ölçüm (Aşama 1 boyunca)

| # | iş | süre |
|---|---|---|
| F1-O1 | Eksik kabul hücreleri: K-5 (bozuk dönüş yolu), K-4 (kesinti), K-9 (iyi ağ); taban ölçümü | 1 |
| F1-O2 | Ürün yolunda son tarih oranı: `f0_flows`'a PTT ≤ D ölçüsü (S2–S4'ün bench boşluğu) | 1 |
| F1-O3 | bench: tcp `t0_unix_ns` kaydı | 0,1 |

**Aşama 1 toplamı ≈ 18 ajan-gün** (ölçüm dahil ≈ 20); `sender.rs` sıralaması yüzünden takvim ≈ 7–9 iş günü [tahmin].

## Aşama 1 çıkış ölçümü (karar kapısı)

Aynı hücreler, aynı kurallar, N ≥ 30. Hedefler [tahmin]dir, ön-kayıt olarak F1 başında donar.

| ölçü | bugün | hedef | hücre |
|---|---|---|---|
| K-3 teslim@60 s | 0,167 | ≥ 0,67 | `b_r25_*_l0–l20_p65` |
| Gönderen kilidi | r12k l30 %65–77 | ≤ %11 | `b_r12k_*_l30` |
| Gecikme p95 ↔ quinn | 22 hücrede geride | fark en az yarıya | B ızgarası |
| K-6 ≤ 800 ms, rs2 | 0,83 | ≥ 0,93 | `c_r12k_d300_l20_b120` |
| K-7 gereksiz yük | %35 | ≤ %5 | `b_r1k_d300_l0_p1000` |
| Gerileme yok | l30 teslim 1,000; oturum 470/470; %80 teslim 0,04–0,77 | aynı ya da iyi | A, B, stres |
| K-5 bozuk dönüş yolu | ölçülmedi | teslim 1,0 | F1-O1 |

**Karar:** hedefler tutarsa Aşama 2 (sözleşme, bu motor). Tutmazsa, ya da sahip sözleşme değişikliklerinin toplamını KTP-2'den ayırt edilemez bulursa, KTP-2 F1'e geçilir. F1-M3 ölçümü KTP-2 için de girdi olur: §3.5 aynı örnekleyici ve bant filtresini devralıyor [çıkarım; V-25]. F1-M4'ün RecoverArq kuralı ise taslağa geçmediği için KTP-2'de yeniden yazılır.

## Aşama 2 — sözleşme (ayrı sahip onayıyla)

### Ajan 6 — S1, kama-app, adaptör

| # | dosya | değişiklik | bağımlılık | ölçü, beklenen etki [tahmin], dayanak | süre |
|---|---|---|---|---|---|
| F1-S1 (A6 + C6) | `client-core/transport.rs`, kama-app, S1 §4 | Reliable birimleri sırasız teslim; kama-app akış başına sıralar; PTT sıra beklemesini atlar. Motor değişmez | sahip | K-1: dosya kaynaklı sıralama payı (%98–99,8, taban) → ~0. K-6: rs2 sıralı → sırasız (0,71 → 0,95; 0,38 → 0,83). Akış içi bekleme kalır | 4 |
| F1-S2 (A4) | `file_transfer.rs` `pump`, kama-contracts S1 | Oturum teyit sinyali + dosya uçuş penceresi W = 8 | sahip, M3, M4 | K-1: ek terim ≤ W × 1088 B / hat hızı (0,7 / 8,7 s). Hız düzelmezse 24–43 s. Risk: W ≈ BDP, dosya verimi düşer | 2 |
| F1-S3 (A3) | `file_transfer.rs:1541`, `specs/application-message-protocol.md:91` | `FILE_RECEIPT_TTL` son ilerlemeden başlar (AMP §3 ek) | S2 | Yanlış `ReceiptTimeout` 280/280 → bob'un aldığı koşularda 0 | 0,5 |

### Ajan 7 — canlı ses

| # | dosya | değişiklik | bağımlılık | ölçü, beklenen etki [tahmin], dayanak | süre |
|---|---|---|---|---|---|
| F1-S4 (C4) | kama-node, client-core, S1 | PTT için ayrı oturum/uç ya da akış başına FEC; profil 0x01 (K = 2, E = 64) | F2, S1 | K-6: onarım 1058 → 98 B; rs2 ↔ rs4 +0,03…+0,08 | 3 |
| F1-S5 (C5) | `ptt.rs:627`, `transport.rs:1272`, `sender.rs` | PTT `DeadlineBounded`; süresi geçen birim kuyruk ve ARQ'dan silinir | sahip (K4G-08), M3 | K-6, K-7: son tarih sonrası yeniden gönderim 6,7–26 B/çerçeve → ≈ 0 | 2 |

**Aşama 2 toplamı ≈ 11,5 ajan-gün;** takvim ≈ 6–9 iş günü [tahmin]. İki aşama ≈ 30 ajan-gün, 13–18 iş günü. Karşılaştırma: KTP-2 taslağı §8 F1–F3 11–15 iş günü (tahmin; taslak denetiminde 16 hata var, `TASLAK-NOTLARI.md`).

## Plan dışı

- **İki FIFO:** F1-S1 olmadan etkisiz; ayrı iş değil.
- **A-3 yeniden kayıp vadesi:** F1-M4 ölçümünden sonra değerlendirilir.
- **B-K3, B-K5:** engelleyici değil / ölçülemedi.
- **05b'nin kalanı, 05c, 05d A1, 05f, K4G-06:** duruyor, dokunulmaz.
