# FEC ARBİTER + SINIRLI SPEKÜLATİF BOOTSTRAP — v2 ölçümü

**Kod:** `100aa5d2` (arbiter) + denetim düzeltmeleri (bu commit). **Bayrak default KAPALI.**
**Ölçüm ikilisi:** kısa akış 96 msg (n=5) = `100aa5d2` binary'si; saf reorder 96/400 (n=3) =
düzeltilmiş binary (v3) (`spec_min_permille` tabanı reorder davranışını değiştirdiği için yeniden koşuldu;
kayıp ailesinde `reorder_pressure = 0` olduğundan taban etkisizdir — o aile geçerli kalır).
**Tasarım:** owner, 2026-09-05. **Denetim:** 36-ajanlı adversaryal (4 mercek × çürütme), bulgular §6.
Spec amendment YAZILMADI. Q18/fuzzy/ARQ/wire/pacing DEĞİŞMEDİ.

## 1. Ne kuruldu

```
                 kanonik Confirmed ──► fuzzy/Q18 ──┐ (politika açıksa: kalibre politika)
                                      yapı-gate  ──┤  ÖNERİ (artık doğrudan yazmaz)
 Suspected ──► fec_loss_risk_split ──► spekülatif ─┤  L0/L1/L2, tavanlı
                                                    ▼
                                 fec_arbiter: final = max(...)   ← TOPLAMA YOK
                                 (politika açıkken spekülatif ≤ politika tavanı)
```

`fec_loss_risk_split` (tek geçiş, ek depolama yok): `instant` (yaş-ağırlıklı) · `persistent`
(yalnız ≥ 0.5×srtt yaşamış Suspected + Confirmed) · `reorder_pressure` = Spurious / tüm olay.
Spekülatif: L0 instant<10k → 0 · L1 → 125‰ (K=8: 1 repair) · L2 persistent≥10k → 250‰ ·
deadline/srtt ≤ 2500 → L1 de 250‰ · kanonik pencerede görünür → tavan 125‰ · reorder indirimi
tavanı en çok yarıya indirir, `spec_min_permille = 63` altına ASLA inmez (K=8'de ≥1 repair).
Spekülatif kol **fuzzy'den geçmez**; 6f9cb6db'deki çift-decide kaldırıldı.
`[CAL]` olan HER sayı ölçümle süpürülecek — bu rapor kalibrasyon girdisidir, sevk kararı değil.

## 2. Kısa akış (96 msg, RTT 200 ms, 1 mbit, reorder yok, n=5) — sonuç metriği

**Birincil uç nokta = 2×RTT (400 ms) içinde teslim oranı** (FEC-MARGINAL-R'nin uç noktası;
denetim "parity varlığı değil KURTARMA ölç" dedi). Birim = REP, medyan [min, max].

| kayıp | OFF | **v2 ON** | Δ | eski ON (6f9cb6db, gate←risk, r=4) | wire Δ |
|---|---|---|---|---|---|
| 0 % | 0.979 | 0.979 | 0 | 0.979 | **+0.00 %** |
| 2 % | 0.875 | 0.906 | +3.1 pp | 0.906 | +8.3 % |
| 5 % | 0.729 | **0.885** | **+15.6 pp** | 0.740 | +16.3 % |
| 10 % | 0.521 | **0.833** | **+31.2 pp** | 0.552 | +14.1 % |
| 15 % | 0.427 | 0.583 [0.000, 0.615] | +15.6 pp | 0.438 | +17.5 % |
| 20 % | 0.292 | **0.510** | **+21.8 pp** | 0.375 | +16.2 % |
| 25 % | 0.000 [0, 0.219] | **0.406** [0.000, 0.448] | **+40.6 pp** | 0.229 | +10.0 % |

Okuma: sınırlı spekülatif bootstrap, 2×RTT deadline'da 5–25 % kayıpta **+15…+41 pp** teslim
kazandırır; eski gate yolu aynı hücrelerde +0…+10 pp vermişti (parity geç geliyordu). Bedel
+8…+17 % wire. `min = 0.000` aykırıları kuruluş-varyansı koşularıdır (§5).

Zamanlama (aynı aile): `t_risk − t_kuşku = 0.000 ms` (n=30) · `t_onay − t_kuşku` 3148–3887 ms
(n=3; ARQ değişmedi) · `risk < akış_sonu < onay` **30/30** · ilk parity medyan **758–956 ms**
(6f9cb6db'de 1141–1560 ms) · parity>0: 2%→3/5 · 5%→3/5 · 10%→5/5 · 15%→3/5 · 20%→5/5 · 25%→3/5.

Arbiter histogramı: kayıplı hücrelerde parity'yi **yalnız spekülatif kol** üretti (SpecBoot 91–657,
SpecPersist 5 222–12 905; ConfFuzzy/StructGate/ConfPolicy **0**) — kanonik yol kısa akışta hiç
ateşlemez, eski ölçümde de fuzzy hiç ateşlememişti (§6 erratum).

## 3. Temiz yol (kapı D) — birebir

0 %: risk 0/5, parity 0/5, `lvl = [14407, 0, 0]`, arbiter tümü `None`, toplam tx rep-başına
OFF = ON ({42 918 ×4, 42 867 ×1}). Tahminci kayıp icat edemez (yalnız `declare()` olaylarını
ağırlıklandırır).

## 4. Saf reorder (kayıp YOK) — kapı 4 "wire amplifikasyonu < bütçe"

v3 = `spec_min_permille = 63` tabanı SONRASI (indirim artık vetoya dönmüyor), n=3. Kayıp yok →
her parity byte'ı saf bedeldir. Birincil uç nokta yine ≤2×RTT teslim (OFF vs ON medyan).

| yük | reorder | wire Δ | parity B (med) | L1 / L2 ateşleme | pers_peak | ≤2×RTT OFF → ON |
|---|---|---|---|---|---|---|
| 96 | 0 % | **+0.00 %** | 0 | 0 / 0 | 0 | 0.979 → 0.979 |
| 96 | 5 % | **+0.00 %** | 0 | 0 / 0 | 0 | 0.979 → 0.885 (parity YOK; gürültü) |
| 96 | 15 % | **+3.59 %** | 184 | 304 / 3 | 11 363 | 0.979 → 0.771 |
| 96 | 30 % | +0.90 % | 276 | 341 / 75 | 74 626 | 0.760 → 0.812 |
| 400 | 0 % | +0.00 % | 0 | 0 / 0 | 0 | 0.995 → 0.995 |
| 400 | 5 % | +0.00 % | 0 | 0 / 0 | 0 | 0.960 → 0.960 |
| 400 | 15 % | +0.05 % | 184 | 261 / 37 | 15 267 | 0.973 → **0.983** |
| 400 | 30 % | +0.07 % | 368 | 354 / 156 | 57 971 | 0.960 → 0.950 |

**Kapı 4 hükmü:** 400 msg'de ≤ +0.07 % (rahat); 96 msg'de 15 % reorder'da **+3.59 %** — owner'ın
kavramsal "%1–2 bütçe"sinin ÜSTÜNDE. Taban öncesi (100aa5d2) aynı hücre +0.82 %'ydi; o düşüklük
tasarlanan ihtiyattan değil yuvarlama-vetosundan geliyordu. Yani "indirim veto değil" ilkesi
kısa akışta ölçülebilir bir bedel taşır; bütçe SAYISI kalibrasyon görevinde belirlenmeli.

**Persistence bulgusu (kritik):** saf reorder'da `pers_peak` 11–75 k ppm ve L2 ateşliyor
(3–156 kez) — **0.5×srtt eşiği bu hücrede reorder'ı AYIRT ETMİYOR** (ölçülen Suspected yaşları
91–131 ms, eşik 100–110 ms). Ön-itiraz (ii)'nin ampirik doğrulaması: tek hücreye kalibrasyon
netem `delay 100ms` sabitine kalibrasyondur. Kalibrasyon görevine devredildi (kanal-türevli eşik:
`max(0.5×srtt, ρ×ölçülen_spurious_yaşı)`).

**"Sürpriz" çözüldü — gürültü, sistematik değil:** 15 % reorder'da ≤2×RTT teslim üç gözlem verdi:
v2 ro400 0.970→0.873, v3 ro96 0.979→0.771, v3 ro400 0.973→**0.983**. Aynı ailede 5 % ON
(SIFIR parity, ÖZDEŞ byte) −9 pp gösterdi → metriğin n=3 gürültü tabanı ≈ ±10 pp. Düşük teslimli
ON koşuları retx aykırılarıyla korele (ro96 15 % ON rep3 retx = 3 222 B vs kol medyanı ~700 B);
parity 184–276 B hiçbir şeyi açıklamaz. n ≥ 5 olmadan yön iddia edilemez.

## 5. Kuruluş varyansı (arbiter DEĞİL)

25 % ON 3/5: rep2/rep4'te ilk kuşku **3 705 / 4 919 ms** (akış 6.8 / 7.7 s) — 60 blok çoktan
kapanmış, `fec_blocks = 0`; erken kurulan 3 rep'te 45/60 blok r=2 aldı (`latch 89/89`).
Aynı sınıf eski %20 OFF rep2'de de var (4 920 ms). İlk-kuşku bimodal: ≈425 ms (packet-threshold)
vs ≈3 134 ms (cadence tabanı 1.5×65 B/31 B·s⁻¹) + aykırılar. Gecikme iddialarında `t_susp > 1 s`
koşuları ayrı sayılmalı.

## 6. Denetim (36 ajan) — ne değişti, ne kaldı

**Koda alındı:** (1) indirim K=8'de yuvarlamayla VETO → `spec_min_permille = 63`; (2) olgunluk
ömür-boyu `diag_` sayacı (lifetime-ratchet sınıfı) → pencereli `loss_windowed_ppm() > 0`;
(3) politika AÇIKKEN spekülatif kolun TAVANI politikadır — `hard_max` (FecAssist/ColdStart/
LowConfidence/NoDeadline) ya da 0 (DeadlineTooLoose/UnsupportedRegion/Stale/…): lane'ler arası
`max()` politikanın "asla daha agresif değil" sözleşmesini bozuyordu; (4) izolasyon testi
AKTÜASYONSUZ olarak pinlendi (profil yok → parity emit edilmez; netem'de ON/OFF hız eşitliği
iddia edilemez — parity inflight'a girer, FEC'in doğal bedeli); (5) politika-açık byte-özdeşlik pini.

**Uygulanmadı — veri karşı:** "reorder_pressure yalnız L2'de" önerisi: 30 % reorder'da L1
indirimi (370 ateşleme) maliyeti sıfırlayan mekanizmaydı; L1'den kaldırmak saf reorder maliyetini
artırır. Yapısal itiraz (baskı ilk L1 ateşlemesinde her zaman 0) DOĞRU ve kabul: ilk L1 indirimsizdir.

**Erratum (eski raporlara işlendi):** "≈200 200 ppm Q18 dizi" bu bant rejiminde yanlış — fuzzy
`bw.med ≈ 0` (btlbw ≤ 50 kB/s) → R3 ölü → hiçbir ölçülen koşuda fuzzy fec>0; tüm eski parity
gate'in 500‰'siydi. "reorder 195k vs loss 137k" tepe-vs-plato, yükler-arası; tepe ayırt edici değil.

**Kalibrasyon görevine devredilen (uygulanmadı):** persistence eşiği = max(0.5×srtt,
ρ×ölçülen_spurious_yaşı) (kanal-türevli; srtt yerine rtprop ya da ilan-anı srtt'si); spekülatif
payda tabanı (sent_window ≥ 32; küçük-payda tepeleri 195k/290k); r=1 hiç ölçülmedi (v1/v2 yalnız
r0/r2/r4) — L1 = 250‰ alternatifi; L1 tabanı 10k → +8 % wire @ 2 % kayıp (+3 pp fayda) — taban
ölçülü bölgeye çekilmeli; `recovery_budget` kalan-tur (`(deadline − yaş)/srtt − 1`) olmalı, statik
oran değil; spec §8.5 kompozisyonu MIN'dir — arbiter yalnız bayrak arkasında, amendment D0'da.

## 7. Ham veri

`arbiter/`: `loss96-v2.*`, `loss96-6f9cb6db.completion.txt`, `reorder96-v3.*`, `reorder400-v3.*`,
`runs-*.tar.gz` (tüm c.err/out.jsonl). Çözümleyiciler: `scripts/netem/fec_risk_shortflow_analyze.py`,
`scripts/netem/fec_risk_completion.py`.

## 8. Kapılar

fmt 0 · clippy -D warnings 0 · adaptive-transport + client-core + bench: **56 suite ok, 0 FAILED**
(fec_loss_risk 17/17 dahil). `cargo test --workspace` 1 FAILED: `crates/kama-integration/tests/fs_cal2_pins.rs`
— **UNTRACKED owner WIP** (`??`), `bcap == SEALED` ile ESKİ burst-clamp kilidini pinliyor; o kusur
bu oturumun başında shipped `initial_window` ile düzeltildi; bu görevden bağımsız, dosyaya dokunulmadı.
