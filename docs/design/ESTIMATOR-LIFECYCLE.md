# ESTIMATOR-LIFECYCLE — `advance()` bağlama tasarımı

**Kapsam:** yalnız fate-estimator yaşam döngüsü. `burst_delta` karar taşımaz; FEC/fuzzy/pacing
semantiği değişmez. Prereg-A, overlay, threshold ve bandwidth politikası kapsam dışı.
**Problem kaydı:** `S-ANSWERS-STRUCT-GATE.md` — `advance()`'ın sıfır ürün çağıranı var; kayıplar
`provisional`'da kalıyor, ürün penceresi yalnız başarılardan oluşuyor, `n10+n11` yapısal 0,
`eligible()` üründe sağlanamaz, `burst_delta` dejenere.

---

## 1. Amaçlanan semantik — kaynaktan

Modül sözleşmesi (`fate_estimator.rs:1-15`, alıntı):

> Per original source packet a three-state fate is tracked: `UNKNOWN` → (`SUCCESS` | `LOSS`). …
> a packet may NOT be finalized `LOSS` before the reordering guard elapses (§3.1). A late
> original-ACK inside the guard corrects the provisional loss to `SUCCESS` … An ACK arriving
> after `LOSS` finalization cannot mutate the window (immutable once fate-known).

`advance()` (`fate_estimator.rs:215-228`):

> Advance virtual time: finalize every provisional loss whose guard has elapsed (`expiry ≤ now`).
> Deterministic (id order).

`drop_unresolved_as_unknown()` (`:231-234`): **horizon/karar-epoch kesimi** — "call AFTER a final
`advance` at the cutoff instant". Bu bir epoch kavramı; ürün katmanında bir horizon olayı **yok**
(bağlantı yaşam döngüsü = `PRODUCT-RUNTIME-INTEGRATION-PENDING` madde 3). **Bağlanmadı — kapsam dışı.**

Yaşam döngüsünün üç bacağı ve bağlanma durumu (bu düzeltme öncesi):

```
on_loss_evidence  →  provisional            sender.rs:2619  BAĞLI (originals-only, 359ca287)
on_original_ack   →  finalized(SUCCESS)     sender.rs:2274  BAĞLI (originals-only)
advance           →  provisional→finalized  ÜRÜN ÇAĞIRANI YOK   ← bu tasarımın konusu
snapshot          →  yalnız finalized       diag_loss_structure (tek okuyucu, pinli)
```

## 2. Aday çağrı noktaları — tamamı

| # | nokta | hüküm |
|---|---|---|
| A | `on_event` başı (public giriş, cfg ayrımından önce) | **RED** — `RecoveredControl` üzerinde de advance koşardı; o olay belgeli olarak tam saat-izolasyonlu ("pacer-saatine … **saat-ilerletme-yok** — N8-bit-özdeşliğin yapısal-tabanı", `sender.rs:1509-1510`) ve `on_event_core`'a hiç girmiyor. İzolasyonu korumak için estimator saati de orada ilerlememeli |
| B | `on_ack_received` içi, beslemeden önce | **RED** — yalnız ACK yolunu kapsar. Kayıplardan sonra susan bir akış (tam da düzeltilen dejenere durum) hiçbir zaman finalize edemez; timer olayları bu noktadan geçmez |
| C | kayıp-ilanı bölgesi | **RED** — yalnız YENİ kayıp geldiğinde koşar; aynı susma boşluğu. Ayrıca ACK olaylarında advance, besleme **sonrasına** düşerdi (bkz. §3 sıralama) |
| D | `snapshot()`/diag içinde tembel | **RED** — `snapshot` `&self` ve bilinçli yan-etkisiz ("owned copy — later events can never mutate a taken snapshot"). Gözlem durumu mutasyona uğratamaz; okuyucuya bağlı sıralama belirsizliği doğar |
| E | yalnız `TimerFired` | **RED** — ACK-anı finalizasyonu bir timer periyodu kayar; `now ≥ expiry` ACK'i guard-içi düzeltme gibi işlenir, guard sessizce uzar. A'nın kapsadığı her şeyi F kapsıyor |
| F | **`on_event_core` başı, motor saatiyle birlikte** | **SEÇİLDİ** |

## 3. F'nin olay-sıralaması gerekçesi

1. **Motorun kendi saati orada ilerliyor.** `on_event_core` girişinde `pacer.refill(dt)` ve
   `last_now_ns = now_ns` (`sender.rs:1524-1526`). Estimator'ın sanal zamanı motor saatiyle aynı
   anda, aynı kapsamda ilerler.
2. **Kapsam tam.** Çekirdek iki build'in ortak omurgası (flag-OFF doğrudan; flag-ON `on_event_batch`
   `for ev in inbound` ile çekirdeğe girer, `sender.rs:1705`) ve AppData/Ack/Timer/CsProbeEcho'yu
   işler. **`TimerFired` buradan geçer** ⇒ kayıplardan sonra susan akış PTO tick'lerinde finalize
   olur — düzeltilen dejenere durumun tam kendisi.
3. **İzolasyon korunur.** `RecoveredControl` flag-OFF'ta çekirdekten önce erken döner; sözleşmesi
   saat ilerletmemek. F bunu aynen korur.
4. **Guard sadakati (asıl sıralama argümanı).** `advance(now)` bu olayın kanıtı beslenmeden **önce**
   koşar. `now ≥ expiry` olan bir ACK için guard, kanıttan kesin önce dolmuştur → modül sözleşmesine
   göre kader expiry anında mühürlüdür → ACK **geç**tir (`late_ack_corrections`), advance-önce bunu
   birebir gerçekler. Besleme-önce olsaydı expiry-sonrası ACK'ler kaderi yeniden yazar, guard
   sessizce "expiry'den sonraki ilk olaya kadar" uzardı.
5. **Aynı-olay güvenliği.** Bu olayda beslenen yeni kanıt `expiry = now + guard > now` alır ⇒ aynı
   olayın advance'i onu vaktinden önce finalize **edemez**.
6. **Monotonluk gerekmez.** `advance` `expiry ≤ now` filtreler; eski bir damga yalnız daha az
   finalize eder, durum bozulmaz.

## 4. Sınır (SR-AUDIT-001) — yaşam döngüsünün parçası

`finalized` yalnız insert görüyordu (`:200`, `:226`), hiç evict yok. Yaşam döngüsünü bağlamak onu
üründe süresiz büyütür; sınırsız tahsis yasak. Sınır: her insert sonrası `len > FATE_WINDOW` iken
`pop_first()` (en düşük id'ler atılır).

**Denklik kanıtı:** `snapshot()` zaten yalnız en yüksek `FATE_WINDOW` id'yi okur
(`skip = len − 256`, `:259-262`); atılan küme tam olarak o pencerenin altındaki id'ler ⇒ erişilebilir
her durumda `snapshot(sınırlı) ≡ snapshot(sınırsız)`. Atılmış bir id için geç kanıt pencerenin altına
yeniden girer ve bir sonraki sınır kontrolünde yine atılır — pencere değişmez. Tek fark:
`late_ack_corrections`/`spurious` sayaçları atılmış id'ler için eksik sayabilir
(yalnız gözlemlenebilirlik; belgelendi).

## 5. Zorunlu üç doğrulama

| # | doğrulama | nasıl |
|---|---|---|
| V1 | gerçek kayıplı akışta `n10+n11 > 0` | Sender-seviyesi: kayıplı akış + guard'ı aşan timer'lar → `diag_loss_structure` |
| V2 | deterministik fate dizisinde doğru geçiş sayaçları | gerçek yaşam döngüsü (kanıt+guard+advance) ile el-hesaplı dizi → **tam** `(n00,n01,n10,n11)`; spurious/late dalları; sınır |
| V3 | legacy controller neutrality | yapısal pin (kullanım kümesi = {2 yazma, advance, 1 snapshot okuma}) + golden regresyon (`fuzzy_e2e`, `q18_check`, lib, client-core) |

V3'ün dürüst sınırı: süreç-içi sayaçsal karşı-olgu kurulamaz (toggle yok, setter yasak). Kanıt,
yapısal pin (hiçbir karar yolu estimator'ı okumuyor) + golden pinlerin yeşil kalmasıdır.

## 6. Bilinen sınırlar — karşıt denetimin bulguları (kayıt, bu turda düzeltilmedi)

Bağımsız denetim bir BLOCKING (V1'in flag-ON sürüşü — düzeltildi: erken-ACK yakınsaması, iki
build'de de yeşil) ve aşağıdaki kayıt-zorunlu bulguları verdi:

### 6.1 In-guard düzeltme bacağı ürün yolunda ERİŞİLEMEZ (MAJOR — takip işi)

Kayıp ilanında seq `inflight`'tan **çıkar** (`sender.rs:2609` üzerinden `queue_losses`);
`on_original_ack` beslemesi ise yalnız `inflight.remove` ACK dalında (`:2280`). İlan edilmiş bir
kaybın geç ACK'i **tombstone** (`:2299-2331`) veya **UN-LOSS** (`:2343-2358`) yollarına düşer —
ikisi de estimator'ı beslemez. Sonuç:

* §3.1'in in-guard spurious düzeltmesi ürün yolunda **ölü koddur**; `spurious_loss_corrections`
  üründe hep 0 okur.
* `advance()` bağlandığı için, ürünün sonradan **çürüttüğü** kayıplar (un-loss/tombstone-hit) dahil
  her ilan edilen kayıp guard sonunda LOSS olarak mühürlenir. Reordering guard'ı saf gecikmeye
  indirger; `burst_delta`, guard'ın korumak için var olduğu reordering rejimlerinde kayıp yapısını
  **fazla sayar**.
* **Bağlayıcı sonuç:** herhangi bir kalibrasyon `burst_delta`'yı tüketmeden önce bu besleme-sadakati
  boşluğu kapatılmalıdır (doğal düzeltme: tombstone-hit/un-loss yollarından `on_original_ack`
  beslemek, yalnız Data orijinalleri). Bu, S-0.7 ile aynı sınıf bir sadakat işidir ve **ayrı
  yetkilendirme** ister.

### 6.2 `fate_known()` artık FATE_WINDOW'da doyar

Sınır nedeniyle `fate_known()` = `min(kümülatif, 256)`. Ürün tüketicisi yok; untracked FS-CAL-2
spine'ı onu kümülatif zaman serisi olarak kaydediyor (`fs_cal2_spine.rs:766` `>=` karşılaştırması
doygunlukta da doğru) — FS-CAL-2 akışına not düşüldü.

### 6.3 Sender-seviyesi guard-süresi pini yok (rezidüel)

Çağrı noktasında yanlış bir `advance(u64::MAX)` üç doğrulamayı da geçerdi (aynı-olay kanıtı yine
erken finalize olamaz; guard süresini pinleyen Sender-seviyesi test yok). V2 bunu estimator
seviyesinde pinliyor; Sender seviyesinde pinlemek 6.1 kapandığında anlamlı olur.
