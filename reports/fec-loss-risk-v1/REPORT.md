# FEC-LOSS-RISK v1 — deneysel FEC kayıp-riski tahmincisi

**Tarih:** 2026-09-05 · **Dal:** `dev/product-transport-fusion` · **Bayrak:** default **KAPALI**
**Ham veri:** `runs.tar.gz` · **Analiz:** `ANALYSIS.txt` · **Koşucu:** `scripts/netem/fec_risk_families.sh`

Spec amendment YAZILMADI (talimat gereği). Ağırlık eğrisi `[CAL]`, normatifleştirilmedi.

---

## 1. Değişen dosyalar

| dosya | ne yapıldı |
|---|---|
| `crates/adaptive-transport/src/loss.rs` | `SuspectConfidence` (yaş→güven eğrisi) + `fec_loss_risk_ppm()` + `t_first_suspected/confirmed` damgaları |
| `crates/adaptive-transport/src/sender.rs` | `FecLossRiskConfig` (bayrak), izole çift-karar entegrasyonu, `t_first_risk/parity` damgaları, teşhis accessor'ları |
| `crates/client-core/src/runtime.rs` | `RuntimeConfig::fec_loss_risk` knob + `ClientStats.fate` enstrümanları |
| `crates/bench/src/{transport,kama_product,main,kama,quic,qdgram}.rs` | `--fec-risk` bayrağı + çıktı satırına enstrüman |
| `crates/adaptive-transport/tests/fec_loss_risk.rs` | 8 test (kapalı-kol özdeşliği · eğri · Spurious sıfırlama · **izolasyon**) |
| `crates/client-core/tests/api_surface_v0.rs` | API pin v0.9 (kasıtlı minor-ekleme) |
| `scripts/netem/fec_risk_families.sh`, `fec_risk_analyze.py` | 4 aileli deney koşucusu + çözümleyici |

## 2. Tahmincinin TAM formülü

```
                Σ_events ( units_e × w_e )
fec_loss_risk = ───────────────────────────         w: per-mille
                     sent_window_units

w_e = 1000                       (Confirmed)
w_e = 0                          (Spurious)
w_e = 0                          (Suspected, srtt ölçülmemiş)
w_e = SuspectConfidence(age_e)   (Suspected, srtt ölçülmüş)

  4·age <  srtt  → 200      (age < 0.25 srtt)
  2·age <  srtt  → 400      (age < 0.50 srtt)
    age <  srtt  → 700      (age < 1.00 srtt)
    age >= srtt  → 700      (onay bekliyor; 1000'e ÇIKMAZ)

ppm = Σ(units × w) × 1e6 / (sent_window_units × 1000)      [tamsayı; RNG/float yok, ADR-17]
```

* Payda ve pencere **kanonik metrikle aynı** (`sent_window_units`, rolling `W_loss`). Ayrı/sınırsız
  tarihçe YOK.
* `age ≥ srtt` bandı bilerek 700'de tutuldu: 1000'e çıkarmak onay semantiğini arka kapıdan
  taklit etmek olurdu.
* **Yapısal sınır:** tahminci yalnızca `declare()`'ın ürettiği olayları ağırlıklandırır. Kayıp
  İCAT EDEMEZ. Aile-4 invaryantı bu yüzden şansa değil, inşaya bağlıdır.

## 3. Sinyal nerede tüketiliyor

Tek yer: `sender.rs::fuzzy_evaluate`, bayrak açıkken.

```
kanonik  d  = fuzzy.decide(inputs{loss_ppm = Confirmed oranı})   ← mode, arq, pacing, state
risk     rd = fuzzy.decide(inputs{loss_ppm = risk})              ← YALNIZ iki alan devralınır:
             d.fec_parity_ratio_permille = rd.fec_parity_ratio_permille
             d.no_rule                   = rd.no_rule
```

Ayrıca `loss_for_fec` (= bayrak kapalıyken kanonikle ÖZDEŞ) iki FEC kapısına girer:
`structure_gate_fec(...)` ve `FusionObservation.inputs.loss_ppm` (kalibre politika). İkisi de FEC
karar katmanıdır.

Fuzzy controller'ın kendisi DEĞİŞTİRİLMEDİ. İkinci `decide()` çağrısı, etkinin iki alanla sınırlı
kaldığını **kanıtlanabilir** kılar; tek alanı içeriden dallandırmak kuralların bağlı ağırlıklarını
bozardı.

## 4. CC / ARQ / pacing dokunulmadığının kanıtı

**Mekanik test** (`fec_loss_risk.rs::risk_sinyali_arq_cc_ve_pacing_yollarina_sizmaz`): aynı akış,
tek eksen bayrak; ON kolunda risk sinyalinin üretildiği doğrulandıktan (vakum-test değil) sonra
şunlar **eşitlik** olarak iddia edilir ve geçer:

| iddia | ne korur |
|---|---|
| `off.diag_rate() == on.diag_rate()` | state · btlbw · srtt · **send_rate** · inflight → INV-RATE-1 |
| `off.diag_ledger() == on.diag_ledger()` | kanonik `loss_windowed_ppm`, unloss, spurious |
| `off.diag_confirm_path() == on.diag_confirm_path()` | `T_confirm`, `confirmed_total` → ARQ onay semantiği |
| `off.state() == on.state()` | state-machine geçişleri |

Ek test: `kanonik_kayip_yeniden_tanimlanmadi` — `loss_windowed_ppm` hâlâ YALNIZ `Confirmed` sayar.

Wire formatı değişmedi (yeni codepoint/alan yok). NACK yok. Alıcı-rapor çerçevesi yok.

## 5. AİLE 1 — kayıp, reorder yok: tepki gecikmesi

`t_conf − t_susp` **her rep'te 3149–3153 ms** (kanaldan bağımsız sabitin ta kendisi).
`t_risk == t_susp` **birebir** (ör. 427319295 = 427319295 ns) → risk sinyalinin ek gecikmesi **0**.

%20 kayıp hücresi, ham (n=2, medyan yanıltıcı olurdu):

| kol | t_kuşku | t_risk | t_parity | **tepki gecikmesi** | parity |
|---|---|---|---|---|---|
| OFF rep1 | 435 ms | — | 6261 ms | **5826 ms** | 9 568 B |
| OFF rep2 | 4920 ms | — | — | **PARITY YOK** | 0 B |
| ON rep1 | 427 ms | **427 ms** | 1461 ms | **1034 ms** | 44 160 B |
| ON rep2 | 3140 ms | **3140 ms** | 5341 ms | **2201 ms** | 15 456 B |

%0/2/5/10 kayıpta **iki kol da parity basmadı** (aşağıda §9-b).

## 6. AİLE 2 — reorder, kayıp yok: yanlış-pozitif bedeli

| hücre | risk_ppm tepe | spurious | **parity** | toplam tx |
|---|---|---|---|---|
| reorder %5 OFF / ON | 0 / 0 | 0 / 0 | **0 / 0** | 178 476 / 178 476 |
| reorder %15 OFF / ON | 0 / **81 224** | 24 / 38 | **0 / 0** | 179 046 / 179 810 |
| reorder %30 OFF / ON | 0 / **195 333** | 34 / 31 | **0 / 0** | 181 678 / 180 140 |

**Saf reorder'da fazladan parity = 0.** Byte farkı +%0,4 / −%0,8 (gürültü). Risk sinyali yükseliyor
ama parity üretmiyor — bunun neden böyle olduğu §9-c'de, ve orada bir UYARI var.

## 7. AİLE 3 — kayıp + reorder

| hücre | kol | risk_ppm | parity_B | retx_B | toplam tx | **gecikme** |
|---|---|---|---|---|---|---|
| %5 kayıp + %15 reorder | OFF / ON | 0 / 82 618 | 0 / 0 | 11 902 / 10 544 | 190 444 / 188 995 | PARITY YOK / PARITY YOK |
| %20 kayıp + %15 reorder | OFF / ON | 0 / 198 799 | 11 592 / 1 334 | 45 218 / 43 748 | 235 830 / **223 872** | 9503 ms / **3013 ms** |
| %20 kayıp + %30 reorder | OFF / ON | 0 / 245 975 | 184 / 19 780 | 46 528 / 43 773 | 225 344 / 242 409 | 7059 ms / **2180 ms** |

Tepki ~3× hızlanıyor. Byte etkisi tek yönlü değil: bir hücrede ON daha AZ parity ve daha AZ toplam
byte kullandı, diğerinde daha çok. n=2, aşırı okunmamalı.

## 8. AİLE 4 — bant çökmesi, kayıp yok: **bant ≠ kayıp** invaryantı

| dönüş bandı | teslim | risk_ppm | conf_ppm | parity | hüküm |
|---|---|---|---|---|---|
| 1 mbit | 400/400 | 0 | 0 | 0 | KORUNDU |
| 10 kbit | 182/400 | **0** | 0 | **0** | KORUNDU |
| 1 kbit | 2/400 | 0 | 0 | 0 | KORUNDU |
| 2 kbit | 0/400 | 0 | 0 | 0 | KORUNDU |
| 300 bit | 0/400 | 0 | 0 | 0 | KORUNDU |

Dönüş yolu çökerken risk **hiçbir hücrede** yükselmedi. `31 B/s` tabanı `T_loss` ilan eşiğinde
korundu; ilan olmayınca ağırlıklandıracak olay da olmuyor. **Kısıt:** 1 kbit ve altında teslim
0–2/400'e düştüğü için o hücreler neredeyse boş koşulardır; anlamlı tek düşük-bant hücresi
**10 kbit**'tir (182/400 teslim, risk 0).

## 9. Şaşırtıcı davranışlar

**(a) Sevk edilen kol %20 kayıpta bir rep'te HİÇ parity basmadı.** OFF rep2: 400 mesaj, %20 kayıp,
`t_parity = 0`. Yani mevcut yol sadece yavaş değil, bu yükte **güvenilmez**.

**(b) Erken sinyal tek başına parity ÜRETMİYOR.** %2/5/10 kayıpta risk 19k/48k/88k ppm'e çıktı,
iki kol da 0 parity bastı. Zamanlama kusuru ile EŞİK kusuru AYRI: tahminci birincisini çözer,
Q18 dizleri (fuzzy `fec>0` için ≳200 200 ppm, K=8'de `r≥1` için ≳212 600 ppm) ikincisini
çözmez. Bu iki kusur birlikte düzeltilmeden "%10 kayıpta FEC" olmaz.

**(c) UYARI — reorder güvenliği tahmincinin KENDİNDEN gelmiyor.** Aile 2'de risk 195 333 ppm'e
çıktı ve yine de parity 0 kaldı. Sıfırı üreten şey tahmincinin ihtiyatı değil, aşağı akıştaki
dizlerin yüksekliğidir. Dizler ileride düşürülürse **saf reorder parity üretmeye başlar** ve
Aile-2 sonucu geçersizleşir. Bu bağımlılık kayda geçirilmiştir; dizler değişirse Aile 2 YENİDEN
koşulmalıdır.

**(d) Kollar ölçüm boyunca nedensel olarak ayrışıyor.** ON kolu daha çok parity gönderince trafik
deseni değişiyor, bu da ölçülen kayıp oranını etkiliyor (Aile 1 %20: conf_ppm OFF 79 500 vs
ON 130 750). Bu bir ölçüm hatası değil, kolun aşağı akış sonucudur — ama "aynı kanal" varsayımıyla
okunmamalıdır.

## 10. Kabul ölçütleri karşısında durum

| # | ölçüt | durum |
|---|---|---|
| 1 | risk, onaydan belirgin ÖNCE görünüyor | **KARŞILANDI** — 3149–3153 ms erken; `t_risk == t_susp` |
| 2 | gerçek kayıpta ilk parity belirgin erkene kayıyor | **KARŞILANDI** — 5826→1034 ms; "parity yok"→2201 ms; A3: 9503→3013, 7059→2180 |
| 3 | saf reorder aşırı parity üretmiyor | **KARŞILANDI** (ama §9-c uyarısıyla) |
| 4 | düşük dönüş bandı kayıp metriğini yükseltmiyor | **KARŞILANDI** — yapısal; tek anlamlı hücre 10 kbit |
| 5 | ARQ semantiği değişmedi | **KARŞILANDI** — `diag_confirm_path` eşitliği |
| 6 | CC/pacing değişmedi | **KARŞILANDI** — `diag_rate` eşitliği |
| 7 | wire değişmedi | **KARŞILANDI** — yeni codepoint/alan yok |
| 8 | bayrak kapalıyken mevcut davranış ve testler geçiyor | **KARŞILANDI** — 56 suite yeşil, fmt 0, clippy 0 |

## 11. Ölçülmeyenler

* n = 2 rep/hücre. Bu bir TEŞHİS koşusudur, PR-doğrulama kampanyası değildir.
* Tek RTT (200 ms), tek bant (1 mbit), tek yük (256 B × 400 mesaj), tek profil (0x03/K=8).
* Ağırlık eğrisi (200/400/700) **kalibre edilmedi** — sadece ilk deneysel şekil.
* Kısa akış (96 mesaj) bu ailelerde koşulmadı; tahmincinin asıl hedefi orasıdır.
* Kalibre politika kolu (`--fec-mode policy`) bu ailelerde koşulmadı; aileler adaptif kolla koştu.


---

# ERRATUM — 2026-09-05 (adversaryal denetim sonrası)

**§9-b / §9-c "≈200 200 ppm Q18 dizi" cümleleri bu bant rejiminde YANLIŞ.** Denetim (fusion.rs
formül replikasyonu + 106 koşunun yeniden ayrıştırılması): ölçülen 1 mbit hücresinde BtlBw ≤ 50 kB/s
→ fuzzy'nin `bw.med ≈ 0` → R3 kuralı ölü → **hiçbir ölçülen koşuda fuzzy `fec > 0` üretmedi.**
Parity üreten her koşuda oran tam **500‰ = fec_med = YAPI-GATE** (100 000 ppm önkoşulu).
Yani: "risk 195k'ya çıktı ama parity üretmedi çünkü dizler yüksek" cümlesi doğru mekanizmayı
yanlış katmana atfediyor — engel fuzzy dizi değil, gate'in 100k önkoşulu + fuzzy'nin bant üyeliği.

**§9-a "sevk yolu güvenilmez"** stall ile karışık: ilk-kuşku zamanı bimodal (≈425 ms
packet-threshold vs ≈3 134 ms = 1.5×65 B/31 B·s⁻¹ cadence tabanı) + aykırılar (3 706/4 920/
6 999 ms); parity basmayan OFF koşuları bu stall sınıfındandır. Gecikme/tepki iddialarında
`t_susp > 1 s` koşuları ayrı raporlanmalıdır.

**"reorder 195k vs loss 137k"** tepe-vs-plato ve yükler-arası kıyastı; ham tepeler 100 000 /
290 666 (n=2), akış sonunda `risk_last = 0`. Tepe ayırt edici istatistik değildir.
