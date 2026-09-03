# Kama vs quinn — §4.4 Killer-Chart (v1)

> ⛔ **RETRAKSİYON BANDI (2026-09-02; orijinal metin aşağıda AYNEN korunmuştur):**
> **(1) p5_revpath başlığı (Kama 1,00 vs quinn 0,00) GERİ ÇEKİLMİŞTİR** — sonradan false-loss fırtınasının
> PARAZİT recovery'si olduğu ölçüldü (TUR-1a: parazit sökülünce p5 24/100; kayıt `docs/design/time-model-v1.md`
> §0.7; meşru geri kazanım TUR-7 P5-GATE mührü). Bu hücrenin v1 sayısı hiçbir raporda kullanılamaz.
> **(2)** v1-dönemi mutlak sub-kbps sayıları ARP-pinleme öncesi enstrüman artefaktı taşır (simetrik; kıyas
> yönü ayakta, mutlak değer taşınmaz). **(3)** Bu belgenin kalıcı değeri rejim YAPISIDIR; güncel ön-kayıtlı
> banka `reports/kapi-eval-v1/` (r5 FAZ-1) + `docs/whitepaper/KAMA-TRANSPORT-WHITEPAPER-v0.3.md`'dir.

> **Tez:** Kama, QUIC'le *yarışmaz*; QUIC'in **düştüğü** yerde ayakta kalır ("tactical resilient comms
> layer"). Bu belge onu **ölçer**: aynı netem profilleri, aynı metrik kodu, 5 repeat, elmalar-elmalar.
>
> **Durum:** 35/35 Kama koşusu + 35/35 quinn baseline · Faz D temiz · ölçüm iki ultracode workflow ile
> sağlamlaştırıldı (bir CRITICAL sent_ns frame-bug bulunup düzeltildi — aşağıda). Tarih: 2026-07-11.

---

## TL;DR — QUIC'in öldüğü yerde Kama ayakta

| rejim | profil | **Kama** c(10s) | quinn c(10s) | fark |
|-------|--------|:---:|:---:|:---:|
| **QUIC ayakta** | %2 kayıp / jitter / aralıklı blackout | 0.97–1.00 | 1.00 | **~eş** (−%0–3) |
| **QUIC ÖLÜ** | %50 kayıp | **0.53** | 0.13 | **+0.40 (4×)** |
| | **ACK-yolu %80 çöküşü** | **1.00** | **0.00** | **+1.00 (TOTAL)** |
| | %80 kayıp | **0.44** | 0.00 | **+0.44** |
| | %95 kayıp | **0.15** | 0.00 | **+0.15** |

**Headline:** ACK-yolu çöktüğünde (p5_revpath) **Kama 1.00, quinn 0.00** — QUIC bidirectional handshake +
per-mesaj ACK ister; %80 dönüş-yolu kaybında quinn'in 5 repeat'inin **2'si hiç bağlanamadı** (handshake
fail, udp_tx=0), 3'ü bağlanıp deadline'ı aştı. Kama **connectionless** — handshake yok, forward-blast +
kümülatif SACK, sparse dönüşü tolere eder → her mesaj teslim.
İki cümle: **iyi ağda Kama ~eşit** (fark %0-3, dürüst maliyet); **hostile'da kategorik kazanıyor.**

---

## 1. Metodoloji — elmalar-elmalar

- **Aynı netem profilleri** (§4.4.2, `harness/netem-profiles.sh`, §4.4.9 reproducible), **aynı completion
  tanımı** (`compare.py`'de reimplement + Rust `bench metrics` otoritesiyle cross-check — algoritmik özdeş,
  `metrics.rs` unit-tested; iki dosya setine simetrik uygulanır), **5 repeat**, MSGS=100 × 256B, deadlines {10,30,120}s.
- **Kama:** adaptive-transport engine (Sender §3 / Receiver §4) gerçek UDP'de (`crates/bench/src/kama.rs`).
  Tek adaptif controller → cubic=bbr; cubic koşuldu. quinn: baseline v1 (cubic, `quinn-baseline-v1.md`).
- **İstek/yanıt modeli (simetrik):** her iki transport da mesaj-başı teslim teyidi kullanır — quinn her
  mesaj için ayrı bi-stream + server 1-byte app-ACK; Kama tüm birimleri yollayıp server'ın periyodik
  KÜMÜLATİF SACK'ine dayanır (tek sağ kalan ACK tüm rx-set'i teyit eder). p5'te bu Kama'nın mimari
  avantajı; ama gap'in bir kısmı bench'in quinn'i mesaj-başı 100 bağımsız dönüş-yolu ACK'ine zorlamasından
  (coalesced-ACK bir QUIC app daha az dönüş paketi isterdi — yine de p5 handshake-fail'lerinde hiç bağlanamaz).
- **Ölçüm iki workflow ile sağlamlaştırıldı:** (1) contract/gap scope, (2) 5-boyut adversarial review — 18 ham
  bulgu → 4 confirmed (hepsi aynı kök, 4 bağımsız boyut). **CRITICAL bulgu:** `sent_ns` paced wire-emit'te
  damgalanıyordu → Kama'nın pacing dwell'i latency'den dışlanıyor → completion **sessizce şişiyordu**
  (delivered_ns frame-bug'ının sent-tarafı ikizi). **Düzeltildi:** app-offer'da damgala (quinn'in zero-point'i).
  Bu belge fix SONRASI honest sayılarla; regression guard testi eklendi.

## 2. Ana sonuç — completion medyan + IQR (5 repeat)

| profil | koşul | K c10 | Q c10 | K c120 | Q c120 | **K IQR(120)** | **Q IQR(120)** |
|--------|-------|:---:|:---:|:---:|:---:|:---:|:---:|
| p7_bbr | %2 kayıp | 0.98 | 1.00 | 0.98 | 1.00 | [0.96,0.99] | [1.0,1.0] |
| p4_jitter | jitter | 1.00 | 1.00 | 1.00 | 1.00 | [1.0,1.0] | [1.0,1.0] |
| p6_blackout | aralıklı | 0.97 | 1.00 | 0.98 | 1.00 | [0.97,0.98] | [1.0,1.0] |
| **p1_loss50** | %50 kayıp | **0.53** | 0.13 | **0.85** | 0.15 | **[0.78,0.89]** | [0.04,0.18] |
| **p5_revpath** | ACK %80 | **1.00** | 0.00 | **1.00** | 0.27 | **[1.0,1.0]** | [0.00,0.34] |
| **p2_loss80** | %80 kayıp | **0.44** | 0.00 | **0.48** | 0.00 | **[0.44,0.52]** | [0.0,0.0] |
| **p3_loss95** | %95 kayıp | **0.15** | 0.00 | **0.15** | 0.00 | **[0.14,0.16]** | [0.0,0.0] |

## 3. Üç rejim

**① QUIC AYAKTA — Kama ~eşit (dürüst: %0-3 geride).** p4_jitter eş (1.00); p7_bbr Kama 0.98, p6_blackout
0.98 (quinn 1.00). Kama iyi ağda quinn'i **yenmez** — recovery overhead + temkinli pacing küçük bir bedel.
Tez bunu gerektirmez (§4.4.5 dürüstlük): "QUIC'le yarışma, düştüğü yerde ayakta kal."

**② QUIC ÖLÜ — Kama kategorik kazanıyor.** %50-95 kayıp + ACK-çöküşü: quinn ~0, Kama 0.15–1.00.

## 4. 🎯 Headline — p5_revpath (ACK-yolu %80 çöküşü)

```
Kama:  c(10s)=1.00  c(120s)=1.00   IQR [1.00,1.00]  (c120'de 5/5 kusursuz; c10 medyan 1.00, bir repeat 0.95)
quinn: c(10s)=0.00  c(120s)=0.27   IQR [0.00,0.34]  (yazı-tura)
```
Baseline'ın işaret ettiği tam nokta — **mekanizma ham veriden:** quinn'in 5 revpath repeat'inin **2'si
handshake'i KURAMADI** (udp_tx=0; %80 dönüş-yolu kaybında bidirectional handshake tamamlanamadı → hiç veri
gönderilmedi); bağlanan 3'ü deadline'ı çok aştı (r1: 34 mesaj ~86s; r4: 20/100 mesaj 10s içinde). Kama
**connectionless** (handshake yok) + forward yol %5 (veri varıyor) + **ACK-bağımsız duplication (§11)** +
**kümülatif SACK** (sağ kalan seyrek ACK tüm rx-set'i teyit eder) → sparse ACK'le bile medyan **1.00**.
Spec'teki `ack_confidence LOW → EMERGENCY_FORWARD_REDUNDANCY` + connectionless bootstrap'ın ölçülmüş zaferi.

## 5. Determinizm kazancı — dar IQR vs yazı-tura

Kama yalnız daha yüksek medyan değil, **öngörülebilir**:
- p5_revpath: Kama **[1.00,1.00]** (5 repeat kusursuz) vs quinn **[0.00,0.34]** (yazı-tura).
- p1_loss50: Kama **[0.78,0.89]** (dar, tutarlı) vs quinn **[0.04,0.18]** (düşük + değişken, baseline'da bimodal).
- p2_loss80: Kama **[0.44,0.52]** vs quinn **[0.0,0.0]**.

Baseline'da quinn'in %50 kayıpta "bağlan-ve-hepsini-ver YA DA tümüyle-başarısız" bimodalliğini
göstermiştik. **Kama o yazı-turayı öngörülebilir teslime çeviriyor** — ADIM-3 hedefi ("dar IQR") tuttu.

## 6. Bedel — dürüst (bytes/delivered-msg)

Kama teslimi bant genişliği + latency ile takas ediyor. **Ama sürpriz (ölçülmüş):** quinn'in kaybolan
byte'ları sayıldığında **Kama çoğu profilde daha UCUZ** — quinn az teslim ederken çok byte harcar.
(`bench metrics` semantiği: udp_tx / delivered, 5-repeat medyan.)

| profil | Kama bytes/msg | quinn bytes/msg | not |
|--------|:---:|:---:|-----|
| p7_bbr | 272 | 389 | Kama ~%30 daha ucuz |
| p4_jitter | 267 | 374 | Kama daha ucuz |
| p6_blackout | 1283 | 415 | **Kama daha pahalı** (kesinti boyunca duplication) |
| p1_loss50 | 897 | 4299 | Kama ~4.8× ucuz (quinn %15 için 4.3KB harcar) |
| p5_revpath | 558 | 1036 | Kama daha ucuz |
| p2_loss80 | 1711 | 5396 | Kama ~3× ucuz |
| p3_loss95 | 7135 | — (0 teslim) | Kama ~7KB/msg; quinn hiç teslim etmez |

Mutlak olarak Kama byte'ları loss'la büyüyor (%95'te ~7KB/teslim-msg forward-recovery). Latency de yüksek:
**p50 hostile'da ~1.4–4.9s** (p5_revpath≈1.4 · p1_loss50≈2.5 · p2_loss80≈2.6 · p3_loss95≈4.9s [byte-maliyeti
en yüksek profil]; dwell + recovery dahil). **Bilinçli:** "geç + pahalı ama ULAŞAN" > "hızlı ama ulaşMAYAN".
Yalnız p6_blackout'ta Kama quinn'den pahalı. Latency-obsesif medya ayrı repo (`saferoom-media`).

## 7. Faz D empirik sanity — TEMİZ ✓
- Kama delivered latency'leri **değişken + sıfır-olmayan** (p5: 87–10486ms, p2: 40–12467ms) → frame doğru.
- delivered oranları tutarlı (p5 %100, p2 %51, p7 %97) → completion gerçek, artefakt değil.
- sent_ns app-offer penceresinde (span ~0.1ms) → dwell latency'e DAHİL (honest).

## 8. Caveat'ler (overclaim yok)
1. **Tek CC (Kama):** Kama tek adaptif controller; cubic koşuldu (cubic=bbr). quinn cubic (baseline'ın
   dayanıklı tarafı) ile karşılaştırıldı.
2. **5 repeat:** medyan+IQR robust ama küçük örnek; magnitude'lar (1.00 vs 0.00) gürültü değil kategorik.
3. **İyi ağda %0-3 geri:** Kama iyi ağda quinn'i yenmiyor — bu tez DEĞİL (§4.4.5). Recovery overhead küçük.
4. **Bedel:** yüksek bytes/msg + latency hostile'da. Bant çok kısıtlıysa (p7 50kbit) küçük completion kaybı.
5. **Loopback temsili değil:** bottleneck-yok loopback'te Kama flood eder; tüm bu ölçümler rate-limited netem'de.
6. **Identity security-stub:** bench'te kripto yok (INV-ENGINE-SOC); gerçek Noise+AEAD katmanı byte/latency'yi
   biraz artırır ama rejim yapısını değiştirmez.

## 9. Ne kanıtlandı
> **QUIC'in kategorik öldüğü iki rejimde — ACK-yolu çöküşü (1.00 vs 0.00) ve ≥%80 kayıp (0.15–0.48 vs 0.00)
> — Kama teslim ediyor; ve %50 kayıpta quinn'in yazı-turasını (0.13, bimodal) 4× daha yüksek + dar-IQR
> öngörülebilir teslime (0.53) çeviriyor. İyi ağda ~eşit kalarak.** Tez ölçüldü: Kama, QUIC'in düştüğü
> yerde ayakta kalan tactical resilient comms layer.

## 10. Tekrarla
```bash
docker compose -f harness/docker-compose.yml up -d --build
bash harness/run-kama-matrix.sh        # Kama: 7 profil × 5 repeat (cubic)
python3 harness/compare.py             # Kama vs quinn baseline → tablo + results/kama-vs-quinn.json
```
Ham: `results/kama_*.jsonl` + `results/*_cubic_r*.jsonl` (quinn baseline). Metrik: `bench metrics` (Rust otorite).
