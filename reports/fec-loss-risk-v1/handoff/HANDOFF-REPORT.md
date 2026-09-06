# HANDOFF AİLESİ — spekülatif → kanonik devir teslim

**Soru (owner):** ilk `Confirmed` geldiğinde spekülatif kolun otoritesi değişince uzun akışta
recovery "cliff"i oluşuyor mu? **İnvaryant:** kanonik sinyalin gelmesi FEC kurtarmasını
kötüleştiremez.
**Kod:** `d5b81514` (c809069b + salt-gözlem zaman-serisi enstrümanı). **Beklenti kaydı:**
`EXPECTATION.md` — koşudan ÖNCE yazıldı. **Konfig:** RTT 200 ms · 1 mbit · reorder yok ·
msgs {96, 400, 1000} × kayıp {5, 10, 20} % × {OFF, ON} × n=3 · 50 ms örnekleme.

## Referans — MDS K=8 blok çözülebilirliği (hesap, ölçüm değil)

| kayıp | r=1 | r=2 | r=4 | Δ(r2→r1) |
|---|---|---|---|---|
| 5 % | 0.929 | 0.988 | 1.000 | −0.060 |
| 10 % | 0.775 | 0.930 | 0.996 | −0.155 |
| 20 % | 0.436 | 0.678 | 0.927 | −0.242 |

Klampın öngörülen bedeli: 250→125 ‰ (r=2→r=1) demotion, %10'da −15.5 pp, %20'de −24 pp blok
çözülebilirliği.

## 1. Baseline — klamp VAR (`d5b81514` + örnekleyici düzeltmesi `836f4f67`), n=3

Pencereler t_conf'a göre: pre = [−1 s, 0), post1 = [0, +1 s), post3 = [+1 s, +3 s). Medyan.
`fin` = arbiter hükmü (‰); `del` = ≤2×RTT teslim oranı (o pencerede SUNULAN mesajlar).

| hücre | fin pre→p1→p3 | Δ_p1 | del pre → p1 → p3 | Δdel_p1 | hüküm |
|---|---|---|---|---|---|
| 400 · 5 % · OFF | 0 → 0 → 0 | 0 | 0.76 → 0.78 → 0.80 | +0.02 | düz |
| 400 · 5 % · **ON** | **250 → 125 → 125** | **−125** | 0.98 → 0.98 → 0.93 | 0.00 | **CLIFF** |
| 400 · 10 % · OFF | 0 → 0 → 0 | 0 | 0.54 → 0.64 → 0.58 | +0.10 | düz |
| 400 · 10 % · **ON** | **250 → 125 → 125** | **−125** | 0.98 → 0.80 → 0.79 | **−0.18** | **CLIFF + teslim ↓** |
| 400 · 20 % · OFF | 0 → 0 → 81 | 0 | 0.30 → 0.29 → 0.39 | −0.01 | düz |
| 400 · 20 % · **ON** | **250 → 125 → 125** | **−125** | 0.66 → 0.44 → 0.46 | **−0.22** | **CLIFF + teslim ↓** |
| 1000 · 5 % · OFF | 0 → 0 → 0 | 0 | 0.70 → 0.84 → 0.76 | +0.14 | düz |
| 1000 · 5 % · **ON** | **250 → 125 → 125** | **−125** | 0.98 → 0.96 → 0.98 | −0.02 | **CLIFF** |
| 1000 · 10 % · OFF | 0 → 0 → 0 | 0 | 0.66 → 0.62 → 0.57 | −0.04 | düz |
| 1000 · 10 % · **ON** | **250 → 125 → 125** | **−125** | 0.92 → 0.80 → 0.82 | **−0.12** | **CLIFF + teslim ↓** |
| 1000 · 20 % · OFF | 0 → 0 → 0 | 0 | 0.32 → 0.38 → 0.39 | +0.06 | düz |
| 1000 · 20 % · **ON** | **250 → 125 → 125** | **−125** | 0.66 → 0.48 → 0.54 | **−0.18** | **CLIFF + teslim ↓** |

96 msg: onay akış bitmeden gelmedi (kontrol; bir rep'te 3575 ms'de geldi, akış 3860 ms).

**Beklenti kaydıyla karşılaştırma:** (1) `t_conf ≈ t_susp + 3.15 s` ✓ (3581–3719 ms). (2) t_conf'ta
250→125 ‰, ANINDA ✓ — tam −125, 6/6 hücre. (3) %5/%10'da gate hiç açılmadı ✓; %20'de +1…+3 s
penceresinde açılmaya başladı (kısmi 168–185; OFF'ta bile p3=81) ✓. (5) teslim düşüşü %10–20'de
−0.12…−0.22 ✓ (beklenti −5…−15 pp; ölçülen daha büyük). %5'te teslim düşmedi (r=1 %5'te 0.929 —
yeterli). **Hiçbir sapma yok; owner'ın "handoff cliff" hipotezi DOĞRU.**

**Mekanizma, 50 ms çözünürlükte** (m400_l5 ON rep1): `t=3585 conf=0 spec=250` → `t=3736 conf=1069
spec=125` — persistent risk 44 444 → 44 919 (DEĞİŞMEDİ). Demotion kanıta değil, bir onayın var
olmasına tepki.

**Ek gözlem:** ON kolu handoff ÖNCESİ teslimde OFF'u 20–40 pp geçiyor (0.92–0.98 vs 0.54–0.76);
handoff sonrası hâlâ önde ama farkın 10–22 pp'sini kaybediyor. Yani cliff, spekülatif kolun
kazancının bir kısmını sebepsiz geri veriyor.

## 2. Düzeltme sonrası — "devir ancak devralan öneriyorsa"

Klamp koşulu `loss_windowed_ppm() > 0` → `canonical_prop > 0` (kanonik lane'in fiili önerisi,
`max(confirmed, structure)`). Yalnız ON kolları yeniden ölçüldü (OFF kolu spekülatif koldan
etkilenmez; baseline OFF verisi geçerli). n=3.

| hücre | fin pre→p1→p3 | Δ_p1 | del pre → p1 → p3 | Δdel_p1 | hüküm |
|---|---|---|---|---|---|
| 400 · 5 % · ON | 250 → 250 → 250 | 0 | 1.00 → 1.00 → 0.99 | 0.00 | **düz** |
| 400 · 10 % · ON | 250 → 250 → 250 | 0 | 0.94 → 0.94 → 0.92 | 0.00 | **düz** |
| 400 · 20 % · ON | 250 → 250 → 250 | 0 | 0.72 → 0.70 → 0.64 | −0.02 | **düz** |
| 1000 · 5 % · ON | 250 → 250 → 250 | 0 | 1.00 → 1.00 → 0.99 | 0.00 | **düz** |
| 1000 · 10 % · ON | 250 → 250 → 250 | 0 | 0.94 → 0.96 → 0.92 | +0.02 | **düz** |
| 1000 · 20 % · ON | 250 → 250 → 250 | 0 | 0.70 → 0.70 → 0.69 | 0.00 | **düz** |

**İnvaryant SAĞLANDI:** 6/6 hücrede kanonik sinyalin gelmesi parity'yi ve teslimi değiştirmedi.
Zaman serisi (m400_l10 ON rep1, t_conf = 3578): `conf` 0 → 3 333 → 18 719 ppm yükselirken
`spec = final = 250`, `cprop = sprop = 0`. Handoff SONRASI teslim baseline'a göre: %10 0.79–0.82 →
**0.92–0.96**, %20 0.46–0.54 → **0.64–0.69**.

Birim testi: `handoff_devralan_yoksa_spekulatif_dusmez` (burst desen → gate kapalı → kanonik
önermez; onay sonrası L2 önerisi düşmemeli; vakum-olmama şartlı).

## 3. Kalan (bu görevde DÜZELTİLMEDİ — kalibrasyon görevine)

* **Aşırı-atış:** %20'de yapı-gate onaylı oran ≥100k'a ulaşınca 500 ‰ (r=4) açacak (baseline
  400/20 OFF p3 = 81, ON p3 168–185 kısmi başlangıç); düzeltilmiş koşularda +3 s içinde
  görünmedi (fin_p3 = 250). r=4 kampanyada r=2'ye eşit kurtarma/2× byte ölçülmüştü — gate
  seviyesi (`fec_med = 500`) kalibre değil; owner'ın r-sweep / Pareto maddesi.
* Sebep etiketi seviyeyi söyler, miktarı değil (`SpecPersist` klamp altında da görünürdü) —
  analizde `spec_prop` okunmalı.
* n=3; tek RTT/bant; reorder yok. Persistence eşiği, payda/evidence-mass, L1/L2 r-sweep,
  çok-RTT reorder matrisi, remaining_recovery_rounds — owner'ın sırasıyla, ayrı görevler.

## 4. Kapılar

fmt 0 · clippy -D warnings 0 (bench test hedefi dahil) · 3-crate suite: 55 ok + **1 flaky**:
`runtime_e2e::client_survives_server_death_mid_run` tam suite'te bugün 3 koşunun 2'sinde düştü,
izole ×3+×3 tekrarda 6/6 geçti. Sebep test yarışı: sunucu ölümünden sonra sunulan 10 offer,
shutdown kuyruğu işlemeden `unaccepted` sayılınca `undelivered = 0` (ürün semantiği tipli ve
tutarlı; testin deterministik bariyeri yok). Bu görevin diff'i `None`'da inert; düzeltme ayrı
iş (owner'a). Gizlenmedi. Test ADLARI doğrulandı (kaybolan test yok).

## 5. Hüküm

Owner'ın hipotezi doğruydu: klamp, kısa akışta görünmeyen ama uzun akışta 3.15 s'de gerçek bir
uçurum yaratıyordu (%10–20'de −12…−22 pp teslim). Düzeltme ("otorite ancak devralan biri varsa
devredilir") uçurumu kaldırdı; invaryant ölçüldü. **Mimari dondurulabilir; sıradaki iş kalibrasyon.**

## Düzeltme tasarımı (veri gelmeden yazıldı; uygulama ölçüm SONRASI)

**Kusurun kökü:** `speculative_propose` içinde `if loss_windowed_ppm() > 0 { cap = min(cap, bootstrap_cap) }`
— olgunluk = "pencerede en az bir Confirmed var". Bu, kanıtın *değişmesine* değil, bir onayın *var olmasına*
tepki verir. Kanonik kol bu bant rejiminde hiçbir şey ÖNERMEZKEN (fuzzy 0, gate ≥100k'a ulaşmıyor)
spekülatif kol yine de düşürülür → devir teslim alıcısı olmayan bir devir.

**Kural (owner semantiği, doğru uygulanmış):** *otorite ancak devralan biri VARSA devredilir.*
Spekülatif tavan, kanonik lane fiilen bir öneri ürettiğinde (`confirmed_prop > 0 ∨ structure_prop > 0`)
`bootstrap_cap`'e iner; aksi hâlde kanıt (persistent risk) neyi destekliyorsa o kalır. `max()` zaten
kanonik öneri daha büyükse onu seçer; klamp yalnız "kanonik küçük ama var" durumunda spekülatifi
frenler — "kanonik hiç yok" durumunda değil.

**Alternatif (daha basit):** klampı tamamen kaldır; `max()` tek başına devir teslimi tanımlar.
Ölçüm iki alternatifi de ayırt etmez (her ikisi de bu ailede aynı davranır); tercih tasarım sorusudur.

**Enstrüman notu:** `reason_idx = SpecPersist` klamp SONRASINDA da görünür — sebep seviyeyi söyler,
miktarı değil. Devir-teslim analizinde `spec_prop` değerine bakılmalı, sebebe değil.
