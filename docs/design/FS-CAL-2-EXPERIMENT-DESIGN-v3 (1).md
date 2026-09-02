# FS-CAL-2 v3.0 — Runtime-Gözlemli FEC Activation Subpolicy Kalibrasyonu

**Sürüm:** 3.1 — gain-capture gerekçesi ve cell-label belirsizlik raporlaması eklenmiş birleşik tasarım  
**Yerine geçtiği belgeler:** `FS-CAL-2-EXPERIMENT-DESIGN-v2.1` + `FS-CAL-2 v3.0 AMENDMENT`  
**Durum:** Yalnız araç/grounding aşamasına GO; result-bearing pilot/training §4 kapıları ve freeze tamamlanmadan yasaktır.  
**Ana ürün çıktısı:** Sabit ve doğrulanmış ARQ tabanı üzerinde, runtime-observed loss yapısına göre `FEC_R2` ekleyen veya eklemeyen versioned alt-politika.

---

## 0. Statü, dürüst kapsam ve başarı tanımı

Bu deney **tam fuzzy engine'i kalibre etmez**. Kalibre edilen tek karar:

```text
girdiler : measured_loss_rate + burst_delta + estimator_eligible + support_eligible
çıktı    : sabit ARQ üstüne FEC_R2 ekle / ekleme
aksiyon  : R0 / R2
```

İki girdili ikili kapının fuzzy membership veya açık eşik çifti olarak seri hale getirilmesi bir sunum/uygulama kararıdır; bilimsel karar yüzeyi aynıdır. Denetlenebilirlik için canonical eşik gösterimi zorunludur.

Faz yol haritası:

| Faz | Kalibre edilen çıktı | Durum |
|---|---|---|
| A | FEC activation: sabit ARQ üzerinde R0/R2 | Bu tasarım |
| B | FEC ratio/K/profile | Executable aktüatör ve ayrı veri gerekir |
| C | ARQ aggressiveness | Executable seviye aktüatörü ve joint-action deney gerekir |
| D | send-rate/compression | Ayrı ürün katmanı/aktüatör çalışması gerekir |

Başarı hâlinde izin verilen ifade:

> “Sabitlenmiş ARQ tabanı üzerinde FEC_R2 aktivasyonu, runtime loss-rate ve loss-structure telemetrisiyle, tanımlı bulk/support bölgesinde kalibre edilmiştir.”

Yasak ifadeler:

- “Fuzzy engine tamamen kalibre edildi.”
- “Bütün ARQ/FEC oranları kalibre edildi.”
- Desteklenmeyen workload, structure veya ürün girdilerine genelleme.

Bu sürümün yapısal ilkeleri:

1. ARQ bütün treatment kollarında aktiftir; fuzzy policy ARQ liveness'ı kapatamaz.
2. Yalnız kodda executable olduğu kanıtlanan `R0` ve `FEC_R2` kullanılır.
3. Ürün girdi/çıktı sözleşmesi koddan türetilmeden result-bearing freeze yapılamaz.
4. Training, signal pilot ve holdout seed/namespace olarak ayrıdır; adaptive retuning yoktur.
5. Product actuator, telemetry, data schema ve analysis pipeline synthetic/golden fixture'larda yeşil olmadan sonuç satırı üretilmez.
6. Destek dışı veya gözlemsel olarak çelişkili bölge daima `R0` fallback üretir.

---

## 1. Ürün kararı ve araştırma sorusu

### 1.1 Sabit reliability tabanı

Her treatment kolunda:

```text
ARQ = mevcut doğrulanmış selective ARQ + bounded tail probe ON
```

ARQ ACK, RTT, loss detection, retransmission, pacing ve PTO state'i fuzzy policy tarafından kapatılamaz veya silinemez.

### 1.2 Kalibre edilecek çıktı

```text
R0 = ARQ çalışır; decision block için parity gönderilmez.
R2 = ARQ çalışır; authoritative FEC_R2 profile ile bounded iki-parity assistance uygulanır.
```

`R2`, repo içinde R2A/FS-1'de kullanılan authoritative FEC_R2 implementation kimliğine bağlanacaktır. K, profile ID ve wire değeri §4.1'de kod/test ile çözümlenir ve freeze record'a yazılır; bu tasarım bunları tahmin etmez.

### 1.3 Araştırma sorusu

> Aynı akışta daha önce kesinleşmiş source-packet kaderlerinden runtime'da ölçülen loss-rate ve loss-structure'a bakıldığında, bir sonraki bulk block için çalışan ARQ'nun üstüne R2 parity eklemek hangi gözlenebilir bölgelerde horizon completion'ı en az 15 yüzde puan artırır ve mevcut hard cost cap içinde kalır?

### 1.4 Birincil hipotezler

- **H1:** Yeterli örneklemli ve düşük-burstiness yüksek-loss bölgelerinde `R2`, `R0`'a göre horizon completion'ı en az 15 yüzde puan artırır.
- **H2:** No-loss, tail-only, estimator-unready veya güçlü-burst bölgelerinde policy `R0` döndürür.
- **H3:** Policy-disabled ve `R0` treatment, mevcut tail-capable ARQ ile exact trace/wire eşittir.
- **H4:** `R2` başarısız olsa bile ARQ liveness ve recovery state'i korunur.

---

## 2. Deney birimi, workload ve karar anı

### 2.1 Deney birimi

İstatistiksel deney birimi tek bir bağımsız flow/run'dır. Aynı run içindeki packet'lar bağımsız örnek olarak sayılmaz.

### 2.2 Workload

- Source-symbol/payload/packetization: DR-ARQ-PRODUCT-TAIL-1'de doğrulanan bulk workload ile exact aynı.
- ARQ ve tail-probe config: doğrulanmış product-tail v1 config.
- Decision block FEC profile: §4.1'de authoritative FEC_R2 implementation'a bağlanan profile.
- Deadline ayrı secondary endpoint'tir. Fiziksel olarak olanaksız değildir; fakat önceki workload'da hit oranı yaklaşık sıfırdır. Birincil endpoint horizon completion'dır.

Policy yalnız freeze record'da kimliği verilen bulk workload/profile için eligible olabilir. Product-path interlock:

```text
workload_profile_id != frozen_supported_bulk_profile
  → support_eligible=false
  → requested_r=R0
  → effective_r=R0
  → fallback_reason=UNSUPPORTED_WORKLOAD
```

Bu kontrol yalnız rapor caveat'i değildir; policy çağrısından önce uygulanan ve test edilen mekanik güvenlik kapısıdır.

### 2.3 Warm-up

Her run iki bölümden oluşur:

```text
Warm-up                 : exact 384 original source packets
Estimator window        : son 256 fate-known original source packets
Decision epoch          : warm-up drain tamamlandıktan sonraki ilk yeni block sınırı
Decision-bearing block  : warm-up'tan sonraki ilk authoritative bulk FEC block
```

Warm-up bütün kollarda `R0` ile çalışır. Böylece treatment estimator snapshot'ını etkileyemez.

Warm-up drain timeout:

```text
2 × DR-ARQ-PRODUCT-TAIL-1'de aynı RTT hücresi için kullanılan frozen run horizon
```

Timeout sonunda 256 fate-known packet yoksa `estimator_eligible=false`; run kalibrasyona girer fakat effective action zorunlu `R0` olur. Satır silinmez.

### 2.4 Treatment uygulaması

Treatment yalnız decision-bearing block'a uygulanır. R2 sistematik source packet'ların ardından parity gönderir. R0 ve R2'nin source-packet prefix'i aynı olmak zorundadır.

---

## 3. Prefix-causal loss-structure estimator

### 3.1 Fate state

Her original source packet için üç durum tutulur:

```text
UNKNOWN
SUCCESS
LOSS
```

Finalizasyon:

- `SUCCESS`: receiver ACK evidence original source'un herhangi bir recovery action gerekmeden teslim edildiğini doğrular.
- `LOSS`: receiver missing/recovered-source evidence veya sender loss/recovery lineage original source'un recovery gerektirdiğini doğrular.
- Reordering guard bitmeden packet final değildir.
- Guard içinde gelen late ACK, geçici loss declaration'ı `SUCCESS` olarak düzeltebilir; spurious-loss metriği ayrıca artar.
- Aynı logical source ID ikinci kez pencereye eklenemez.
- Terminal packet ancak bounded tail probe/recovery veya run horizon tarafından fate-known hâle geldiğinde pencereye girer.
- Horizon sonunda hâlâ ayırt edilemeyen packet `UNKNOWN` kalır ve estimator denominator'ına girmez.

Reordering guard süresi bu sürümde aşağıdaki ürün PTO tabanına sabitlenir:

```text
fate_reordering_guard_ns =
  clamp(srtt_ns + 4*rttvar_ns + max_ack_delay_ns,
        min_pto_ns,
        max_pto_ns)
```

Guard dolmadan hiçbir packet `LOSS` olarak kesinleştirilemez. Formül, product-tail doğrulamasındaki bounded PTO tabanıyla aynı authoritative sembol ve runtime değerlerden beslenir; exact kaynak bağları §4.2 qualification kaydında doğrulanır.

Estimator'ın product code'daki exact ACK/loss/recovery event kaynakları §4.2'de sembol ve test adıyla ground edilir.

### 3.2 Pencere ve sayaçlar

Logical source sırasına göre son 256 fate-known bit tutulur:

```text
0 = SUCCESS
1 = LOSS
```

Sayaçlar:

```text
N
lost
n00
n01
n10
n11
```

Transition yalnız ardışık logical source ID'lerin ikisi de fate-known ise sayılır.

### 3.3 Formüller

```text
loss_rate = lost / N

p11 = (n11 + 1) / (n10 + n11 + 2)
p01 = (n01 + 1) / (n00 + n01 + 2)

burst_delta = p11 - p01
```

Laplace smoothing nedeniyle `p=0` bölme hatası oluşmaz. Raw sayaçlar daima kaydedilir; yalnız türetilmiş oran kaydetmek yasaktır.

### 3.4 Eligibility

```text
N == 256
n10 + n11 >= 8
n00 + n01 >= 32
warmup_complete == true
telemetry_stale == false
flow_converged == true
```

`N=256`, `n10+n11>=8` ve `n00+n01>=32` eşikleri FS-CAL-2 v3 için pinlenmiştir. Bu eşiklerden veya reordering-guard formülünden herhangi birinin değişmesi yeni bir tasarım/kalibrasyon sürümü ve S0 + §4.2b yeniden-yeterlilik koşusu gerektirir; result gördükten sonra sessiz ayar yapılamaz.

Koşullardan biri sağlanmazsa:

```text
estimator_eligible = false
requested_r = R0
effective_r = R0
fallback_reason = exact reason code
```

### 3.4b Runtime destek bölgesi — configured label kullanımı yasak

`configured_structure`, configured RTT veya impairment etiketi runtime karar özelliği değildir. Policy yalnız gözlenen snapshot üzerinden destek kontrolü yapar.

Training tamamlandıktan sonra, holdout açılmadan önce canonical support bitmap üretilir:

```text
loss_bin_width_ppm  = 50_000
burst_bin_width_ppm = 100_000

loss_bin  = floor(measured_loss_ppm / loss_bin_width_ppm)
burst_bin = floor((clamp(burst_delta_ppm, -200_000, 900_000) + 200_000)
                  / burst_bin_width_ppm)
```

Bir bin yalnız aşağıdakiler birlikte sağlanırsa `SUPPORTED` olur:

```text
bin + 8 komşusunda >= 100 bağımsız eligible training pairing_group
ve >= 2 farklı training cell_id
ve workload_profile_id frozen bulk profile ile exact eşit
```

Bin genişlikleri training `{5,10,20,30}%` ile holdout `{7.5,15,25}%` loss seviyelerini ve training `{0.15,0.35}` ile holdout `{0.25,0.45}` intermediate-burst seviyelerini yalnız komşu-bin interpolasyonu üzerinden bağlayacak şekilde pre-outcome sabitlenmiştir.

Support bitmap, kaynak training dataset hash'iyle policy artifact'a gömülür ve holdout'tan önce freeze edilir. Runtime davranışı:

```text
estimator_eligible == false → R0 / ESTIMATOR_INELIGIBLE
support bitmap dışında      → R0 / OUT_OF_SUPPORT
unsupported workload        → R0 / UNSUPPORTED_WORKLOAD
```

Bu predicate yeni structure ailesini ismen tanıdığını iddia etmez; yalnız eğitimde gözlenmiş runtime-feature desteğini uygular. Support tanımı sonuçlara bakılarak holdout sonrası genişletilemez.

### 3.5 Estimator yeterlilik testi — S0

Product experiment öncesi sentetik fate-bit traces:

```text
loss rate p       = {0.05, 0.10, 0.20, 0.30}
structure         = {IID, MARKOV-BD-b15, MARKOV-BD-b35, GE-L3, GE-L10}
trace length      = 384
traces/profile    = 500
total traces      = 4 × 5 × 500 = 10,000
```

Gilbert–Elliott tanımı:

```text
Good state drop probability = 0
Bad state drop probability  = 1
P(Bad→Good) = 1/L
P(Good→Bad) = p × P(Bad→Good) / (1-p)
```

Split run/trace seviyesinde, seed-hash ile `%70 train / %30 validation` yapılır. Packet seviyesinde split yasaktır.

S0 PASS:

- p ∈ {0.10,0.20,0.30} için IID-vs-GE-L3 ve IID-vs-GE-L10 ayrı ayrı bootstrap `%95 AUC lower bound ≥ 0.90`.
- MARKOV-BD profillerinde measured burst-delta median'ı configured `b` değerinden mutlak `0.05`'ten fazla sapmaz ve sıralama `IID < b15 < b35 < GE-L3 < GE-L10` her p seviyesinde korunur.
- Aynı profillerde estimator eligibility ≥ `%95`.
- p=0.05 eligibility düşerse p=0.05 ve altı support bitmap'e alınmaz ve R0 fallback olur; `<`/`<=` sınırı S0 sonucu değil bu mekanik kuralla sabittir.
- Reordering, delayed ACK, ACK loss ve terminal-tail test vektörlerinde future leakage ve double-count yoktur.

S0 düşerse result-bearing training başlatılmaz; farklı feature/estimator tasarımı gerekir.

---

## 4. Instrument qualification — result öncesi zorunlu kapılar

### 4.0 Ürün sözleşmesi matrisi — bloklayıcı

Kalibrasyon kapsamı koddan türetilir. Agent, authoritative `FusionInputs`, `FusionOutputs`, safety config ve actuator call-sites öğelerini enumerate ederek aşağıdaki iki tabloyu doldurur; boş/varsayımsal satırla freeze yasaktır.

Girdi tablosu zorunlu alanları:

| Alan | İçerik |
|---|---|
| `input_symbol` | Exact Rust type/field/variant |
| `source_file_sha256` | Kaynak kimliği |
| `runtime_producer` | Değeri üreten authoritative sembol |
| `role` | `VARIED`, `FIXED`, `SAFETY_ONLY`, `UNSUPPORTED` |
| `design_factor_or_value` | Faktör seviyeleri veya exact sabit değer |
| `fallback_if_unavailable` | Exact reason code |

Çıktı tablosu zorunlu alanları:

| Alan | İçerik |
|---|---|
| `output_symbol` | Exact output field/variant |
| `actuator_symbol` | Wire davranışını değiştiren authoritative call-site |
| `actuator_state` | `VARIABLE`, `DEGENERATE`, `ABSENT` |
| `treatment_arm` | Bu tasarımdaki karşı-olgusal arm veya `NONE` |
| `phase` | A/B/C/D |
| `wire_proof_test` | Actuation test adı |

Kapı kuralları:

- Kodda bulunan her input/output exact bir kez görünür.
- `VARIED` alan §6/§8 faktörlerinde bulunur.
- `FIXED` alan exact değer ve invalidation condition taşır.
- `UNSUPPORTED` alan §14 ve runtime fallback tablosunda bulunur.
- `burst_delta` mevcut input type'ta yoksa ek alan, producer, freshness ve serialization yolu §4.2 kapsamında test edilir.
- `VARIABLE` actuator olup treatment arm'ı bulunmayan çıktı gerekçesiz bırakılamaz.

Çıktı artifact'ı: `FS-CAL-2-PRODUCT-CONTRACT-v3.json`. Bu artifact schema-valid ve hash-frozen olmadan result-bearing manifest üretilmez.

### 4.0b Joint-action space — bloklayıcı

```text
Eğer controller-erişilebilir executable ARQ ayarı sayısı == 1:
  joint action = {frozen ARQ} × {R0,R2}
  mevcut treatment matrisi yeterli

Eğer sayı > 1:
  veri görülmeden:
    A) ARQ setting treatment faktörüne eklenir ve bütün satır hesabı yeniden türetilir; veya
    B) calibration-v1 için tek ARQ setting sözleşmeyle sabitlenir.
  Sessizce ilk/default setting seçmek yasaktır.
```

Zorunlu makine-okunur sözleşme:

```text
arq_contract_id
arq_setting_symbol
arq_setting_value
arq_source_sha256
invalid_if_arq_setting_changes = true
```

Product config veya actuator bu sözleşmeden farklı ARQ setting kullanırsa policy yüklenmez ve `R0` fallback olur.

### 4.0c Utility/cost sözleşmesi — bloklayıcı

§7'deki `R2_PREFERRED` etiketi ancak authoritative ürün sınırlarıyla üretilebilir. Freeze record şu değerleri exact kaynak sembolüyle taşır:

```text
max_fec_bytes_per_block
max_combined_recovery_overhead_bytes
max_fec_wire_ratio_ppm
minimum_completion_gain_pp = 15
primary_horizon_definition
```

Bu değerlerden biri yalnız deney için icat ediliyorsa açıkça `EXPERIMENTAL_BOUND` işaretlenir ve ürün iddiası vermez. Hiçbir cost bound grounded değilse training başlamaz; çünkü deney overhead–yarar bölgesi değil yalnız completion bölgesi üretir.

### 4.1 FEC_R2 grounding

Freeze record aşağıdakileri repo kanıtıyla doldurur:

| Alan | Zorunlu kanıt |
|---|---|
| Authoritative R2 source symbol/type | source path + symbol |
| Codec/profile identity | exact enum/constant |
| K ve R | runtime assertion |
| Serializer fields | schema path |
| Parity emission path | integration-test trace |
| Decode/reassembly path | golden test |
| Maximum FEC bytes | `FusionSafetyConfig` field/value |
| Combined overhead cap | safety field/value |

`R1/R4` bu tasarımda yoktur. Grounding sonucu R2 product path'te executable değilse tasarım `IMPLEMENTATION-BLOCKED` olur; sahte harness arm kullanılmaz.

### 4.2 Runtime estimator grounding

Her fate transition için exact product event source, timestamp ve correction path tabloya yazılır. Sentetik event replay ile `N/lost/n00/n01/n10/n11` expected vector exact eşitliği aranır.

Grounding tablosu en az ACK-success, missing/recovered-source, sender loss/recovery lineage, late-ACK correction, terminal resolution, `srtt_ns`, `rttvar_ns`, `max_ack_delay_ns`, `min_pto_ns` ve `max_pto_ns` için authoritative source symbol ile exact runtime değerini içerir. Reordering guard ve eligibility eşikleri test adlarıyla pinlenir.

### 4.2b Product-path eligibility pre-check — signal pilot/training öncesi, bloklayıcı

S0 sentetik fate-bit trace'i üzerinde çalışır; orada her paketin kaderi tanım gereği bilinir. Ürün yolunda fate çözünürlüğü ACK kanıtına bağlıdır ve uç hücrelerde 384 warm-up paketinden 256 fate-known çıkmayabilir.

Result-bearing training'den önce yalnız estimator telemetrisi üreten şu diagnostic pre-check çalıştırılır:

```text
P1: structure=IID,    p=30%, RTT=1000 ms, arm=STATIC_R0, N=40
P2: structure=GE-L10, p=20%, RTT=1000 ms, arm=STATIC_R0, N=40
Toplam: 80 diagnostic run
```

PASS kriteri iki hücre için ayrı ayrı:

```text
estimator_eligible oranı >= 90%
runtime_features_present = 100%
schema/invariant failure = 0
```

Kriter düşerse warm-up uzunluğu, pencere boyutu veya eligibility eşikleri yeniden değerlendirilir; result-bearing training başlamaz. Her değişiklik yeni tasarım sürümü gerektirir ve S0 ile bu pre-check yeniden çalıştırılır. Bu 80 diagnostic run training/holdout sonucuna, parameter fit'e veya result-bearing satır toplamına girmez.

### 4.3 Product actuator proof

Result run öncesi tek-flow golden tests:

- R0 parity bytes = 0.
- R0 trace digest = mevcut tail-capable ARQ trace digest.
- R2 parity bytes > 0 ve pinned profile'a exact eşit.
- R2 source prefix digest = R0 source prefix digest.
- FEC decode success exact reassembly üretir.
- FEC decode failure ARQ recovery'yi engellemez.
- Pacing/congestion gate bypass edilmez.
- Hard FEC ve combined-overhead caps aşılamaz.

### 4.4 Schema gate

Her satırda §10'daki bütün alanlar bulunur. `runtime_features_present != 1`, missing/extra field veya invalid enum satırı yazılmadan reddedilir. Unknown-field reject zorunludur.

`DATA-PROCESSABLE-PASS` preflight zorunludur: synthetic STATIC_R0/STATIC_R2/CALIBRATED_POLICY kayıtları runner'dan gerçek typed schema ile çıkar, analysis binary tarafından ingest edilir, fate vector'dan feature'lar yeniden hesaplanır, PairRecord/HoldoutTriple kurulur ve §11.8'deki 12 artifact iki kez byte-identical üretilir. Elle hazırlanmış analysis-only fixture bu end-to-end preflight'ın yerine geçmez.

§4.0, §4.0b, §4.0c, §4.1, §4.2, §4.2b, §4.3 ve §4.4 kapıları PASS olmadan result-bearing manifest/freeze/run yoktur.

---

## 5. Treatment ve pairing

### 5.1 Training arms

```text
STATIC_R0
STATIC_R2
```

### 5.2 Holdout arms

```text
STATIC_R0
STATIC_R2
CALIBRATED_POLICY
```

Policy arm tek başına hiçbir holdout hücresinde koşulmaz. R0 ve R2 karşı-olguları her holdout hücresinde bulunur.

### 5.3 Deterministik training impairment

Her run için üç ayrı hash stream:

```text
InitialSource
Parity
ArqRetransmission
```

```text
seed = SHA256("FSFC2-TRAIN" || cell_id || repetition || stream_class)
```

R0/R2 pairing group aynı InitialSource stream'ini kullanır. Ek parity veya retransmit packet'ı source-loss sırasını kaydıramaz.

Signal pilot aynı generator'ı kullanır fakat ayrık domain zorunludur:

```text
seed = SHA256("FSFC2-PILOT-V3" || cell_id || repetition || stream_class)
```

Pilot seed/run-id hiçbir training veya holdout manifest'ında bulunamaz.

### 5.4 Holdout pairing

```text
in-process: SHA256("FSFC2-HOLDOUT-INPROC-V3" || cell_id || repetition || stream_class)
netns     : SHA256("FSFC2-HOLDOUT-NETNS-V3"  || cell_id || repetition || arm)
```

Netns'te üç arm aynı netem config/seed lineage'ını kullanır. Ek parity packet'ları netem RNG tüketimini değiştirebildiği için packet-exact pairing iddia edilmez. Analiz run/seed-clustered ve arm-level karşılaştırma kullanır. Source prefix'in aynı olduğu packet capture digest ile ayrıca doğrulanır; sağlanamazsa paired test yerine önceden belirlenmiş unpaired/clustered yöntem kullanılır.

---

## 6. Training faktörleri ve exact matris

### 6.0 Exact intermediate-burst generator ve frozen signal pilot

GE-L3/L10 yalnız yüksek burst-delta kümeleri üretir. Orta yapı seviyeleri aşağıdaki doğrudan binary Markov fate generator ile zorunlu olarak eklenir:

```text
MARKOV-BD(p,b)
P(loss_t=1 | loss_{t-1}=1) = p + b*(1-p)
P(loss_t=1 | loss_{t-1}=0) = p*(1-b)

PPM_SCALE = 1_000_000
p11_ppm = p_ppm + floor(b_ppm*(PPM_SCALE-p_ppm)/PPM_SCALE)
p01_ppm = floor(p_ppm*(PPM_SCALE-b_ppm)/PPM_SCALE)
```

Bu tanımın stationary loss rate'i `p`, teorik `burst_delta` değeri `b`'dir. İlk state stationary Bernoulli(p) draw ile seçilir. Çarpım ara değeri `u128`/checked integer kullanır. RNG stream-class ve fixed-point arithmetic freeze record'da pinlenir; float implementation yasaktır. Golden vectors transition counts ve empirical p/b toleranslarını doğrular.

Full training'den önce ayrı namespace/seed domain'de frozen signal pilot:

```text
Rejimler : IID-20, IID-30, MARKOV-BD(p=20%,b=0.35), GE-L10-p20
RTT      : {50,1000} ms
Arms     : {STATIC_R0,STATIC_R2}
N        : 50
Toplam   : 4 × 2 × 2 × 50 = 800 paired result-bearing diagnostic run
```

Pilot calibration fit'e, support bitmap'e veya holdout'a girmez. Devam kapısı veri görülmeden donar:

```text
schema/invariant/actuator failure = 0
runtime_features_present = 100%
pairing + pre-treatment snapshot equality = 100%
>=1 IID hücrede point delta_completion(R2-R0) >= 15 pp
>=1 structured-loss hücrede point delta_completion(R2-R0) < 15 pp
   ve mean total_bidirectional_wire_bytes(R0) <= R2
```

Kapı düşerse 14.800-run training başlamaz; pilot sonucuna bakarak eşik veya faktör seviyesi değiştirilmez. Yeni tasarım sürümü gerekir.

### 6.1 Main training — RTT 250 ms

| Rejim | Seviyeler | Base condition |
|---|---|---:|
| IID | p={5%,10%,20%,30%} | 4 |
| Markov intermediate | p={10%,20%,30%}, b={0.15,0.35} | 6 |
| GE short | p={5%,10%,20%,30%}, L=3 | 4 |
| GE long | p={5%,10%,20%,30%}, L=10 | 4 |
| Terminal tail | length={1,4}; warm-up no-loss | 2 |
| Mixed IID+tail | p=20%, tail=1 | 1 |
| Mixed GE+tail | p=20%, L=3, tail=1 | 1 |
| No loss | p=0 | 1 |
| **Toplam** |  | **23** |

```text
23 conditions × 2 arms × N=200 = 9,200 runs
```

### 6.2 RTT interaction training

Conditions:

```text
IID-20
MARKOV-BD-p20-b15
MARKOV-BD-p20-b35
GE-20-L3
GE-20-L10
TAIL-1
MIXED-IID-20-TAIL1
```

RTT:

```text
{50 ms, 1000 ms}
```

```text
7 conditions × 2 RTT × 2 arms × N=200 = 5,600 runs
```

### 6.3 Training toplamı

```text
9,200 + 5,600 = 14,800 runs
```

N=200 gerekçesi:

- Worst-case Wilson `%95` yarı-genişlik yaklaşık `%6.9`.
- Paired McNemar power `%80`, δ=15 yüzde puan için pdisc≤0.50 bölgesinde yaklaşık N=173 gerekir.
- N=200, frozen minimum effect `δ=15 yüzde puan` için muhafazakâr üst tam sayıdır.

Bu v3 tasarım `%5` etkileri çözmeyi iddia etmez.

---

## 7. Training label ve calibration objective

### 7.1 Endpoint sırası

1. Safety/invariant
2. Horizon completion
3. Existing hard FEC/combined cost caps
4. Total bidirectional wire bytes
5. RMST
6. Conditional latency
7. Deadline-hit ayrı secondary

### 7.2 Cell label

Analysis ingest önce her `pairing_group` için R0 ve R2 satırını birleştirerek tek `PairRecord` üretir. Pair kabul şartları:

```text
exactly one STATIC_R0 + exactly one STATIC_R2
same cell_id / repetition / InitialSource seed / workload / ARQ contract
same observation_epoch
same pre-treatment estimator counters and feature_snapshot_sha256
action uygulanmadan önce snapshot alınmış
```

Şartlardan biri düşerse satırlar analizden sessizce çıkarılmaz; dataset conformance FAIL olur.

Her cell için pairing-group seviyesinde `10,000` bootstrap resample ve exact McNemar raporlanır. Packet veya tek-arm row seviyesinde bağımsızlık varsayılmaz.

```text
delta_completion = Pcomplete(R2) - Pcomplete(R0)
```

`R2_PREFERRED` yalnız:

```text
lower_95_CI(delta_completion) > 0
point_estimate(delta_completion) >= 0.15
all safety/invariant gates pass
hard FEC and combined cost caps pass
R2 realized wire metrics authoritative cost contract içinde
```

Diğer bütün durumlar `R0_PREFERRED` olur. Bu sürüm latency-only kazanç için parity aktive etmez.

#### 7.2b Cell-label belirsizliği — reporting-only, retune gerekçesi değil

37 training condition içindeki per-cell label'lar sonlu örnekleme bağlıdır; tek bir bootstrap realization'ı bilimsel gerçek gibi sunulmaz. Her cell için zorunlu alanlar:

```text
preferred_action
delta_completion_point
delta_completion_ci_low/high
p_bootstrap_r2_preferred
mcnemar_exact_p
discordant_r2_only
discordant_r0_only
label_boundary_flag
```

`p_bootstrap_r2_preferred`, 10.000 pairing-group bootstrap replikasyonunun §7.2'deki bütün `R2_PREFERRED` koşullarını birlikte sağladığı orandır. `label_boundary_flag=true` iff `0.10 < p_bootstrap_r2_preferred < 0.90`. Bu flag verdict'i tek başına değiştirmez; §9.0/K3–K6 stability kapıları karar haritasına etkisini sınar. Boundary cell sayısı, kimlikleri ve LOCO/bootstrap altında aksiyon değişimleri `IDENTIFIABILITY.json` ile final raporda açıklanır. Çoklu cell p-value'ları headline kanıtı değildir; nihai genelleme ayrık holdout'a dayanır.

### 7.3 Calibration policy formu

Calibrated FEC-activation alt-politikası yalnız:

```text
loss_rate
burst_delta
estimator_eligible
support_eligible
workload_supported
```

kullanır.

Bandwidth `FusionSafetyConfig` içindeki hard budget guard'dır. Jitter, queue-delay ve ACK-confidence bu sürümde FEC activation membership'i değildir ve `UNCALIBRATED-v1` olarak raporlanır.

Kural formu:

```text
IF estimator_eligible
AND support_eligible
AND workload_supported
AND loss IS high
AND burstiness IS low
THEN request R2
ELSE request R0
```

Membership arama uzayı:

```text
loss_a, loss_b       : 0..300000 ppm, step 5000, loss_a < loss_b
burst_a, burst_b     : -0.20..0.80, step 0.025, burst_a < burst_b
activation threshold : {0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90}
```

Inference:

```text
SCALE = 1_000_000

mu_high_loss(x;a,b) =
  0                         if x <= a
  SCALE                     if x >= b
  floor((x-a)*SCALE/(b-a))  otherwise

mu_nonburst(y;a,b) =
  SCALE                     if y <= a
  0                         if y >= b
  floor((b-y)*SCALE/(b-a))  otherwise

activation = min(mu_high_loss, mu_nonburst)

request R2 iff:
  estimator_eligible && support_eligible && workload_supported
  && safety caps allow R2
  && activation >= activation_threshold*SCALE
else R0
```

Bütün hesap unsigned/signed checked integer arithmetic kullanır; boundary equality yukarıdaki dallarla sabittir. Overflow, negative burst, exact-threshold ve ±1 unit golden testleri zorunludur.

Parameter seçimi, leave-one-base-condition-out validation'da lexicographic:

1. Harmful R2 false-positive cell sayısını minimize et.
2. Eşitse oracle completion-gain capture'ı maksimize et.
3. Eşitse excess parity/wire regret'i minimize et.
4. Eşitse daha dar R2 activation region seç.

Her aday politika training `PairRecord` tablosunda karşı-olgusal replay ile değerlendirilir: policy R0 seçerse pair'in gerçek R0 sonucu, R2 seçerse gerçek R2 sonucu kullanılır. Böylece candidate utility tahminî model çıktısından değil, aynı pairing-group içindeki ölçülmüş aksiyon sonucundan gelir. Cell label yalnız harmful/preferred bölge sınırını belirler; completion, wire ve RMST toplamları pair replay'den hesaplanır.

Her lexicographic kademeden sonra kalan aday sayısı ve canonical aday-kümesi SHA-256 değeri raporlanır. Identifiability §9.0/K6, **dördüncü bilimsel kademe sonrasındaki ve deterministik bağ-kırıcı öncesindeki** bütün adayların karar haritasını sınar. K6 PASS olduktan sonra birden fazla aday kalırsa bilimsel anlam taşımayan deterministik son bağ-kırıcı uygulanır:

```text
lexicographically smallest tuple:
(loss_a, loss_b, burst_a, burst_b, activation_threshold)
```

Calibration artifact zorunlu olarak şunları içerir:

```text
candidate_count_after_tier_1
candidate_count_after_tier_2
candidate_count_after_tier_3
candidate_count_after_tier_4
final_tie_count
candidate_set_sha256
selected_tuple
```

Final parameters bütün training üzerinde yeniden fit edilir ve holdout açılmadan freeze edilir. Bağ-kırıcı sonuçlara bakılarak değiştirilemez.

---

## 8. Ayrık holdout matrisi

Training'de olmayan loss-rate seviyeleri kullanılır. Holdout sonuçları calibration'a geri beslenmez.

### 8.1 In-process product-path holdout

| Rejim | Seviyeler | Base condition |
|---|---|---:|
| IID | p={7.5%,15%,25%}, RTT={50,1000} | 6 |
| Markov intermediate | p={15%,25%}, b={0.25,0.45}, RTT={50,1000} | 8 |
| GE short | p={7.5%,15%,25%}, L=3, RTT={50,1000} | 6 |
| GE long | p={7.5%,15%,25%}, L=10, RTT={50,1000} | 6 |
| Terminal tail | length={1,4}, RTT={50,1000} | 4 |
| Mixed IID+tail | p=15%, tail=1, RTT={50,1000} | 2 |
| Mixed GE+tail | p=15%, L=3, tail=1, RTT={50,1000} | 2 |
| No loss | RTT={50,1000} | 2 |
| **Toplam** |  | **36** |

Arms:

```text
STATIC_R0
STATIC_R2
CALIBRATED_POLICY
```

```text
36 conditions × 3 arms × N=200 = 21,600 runs
```

### 8.2 Faithful two-netns/netem holdout subset

Exact 14 cells:

```text
1  IID-7.5-RTT50
2  IID-15-RTT1000
3  IID-25-RTT1000
4  GE-15-L3-RTT50
5  GE-15-L10-RTT1000
6  GE-25-L3-RTT1000
7  GE-25-L10-RTT50
8  TAIL1-RTT50
9  TAIL4-RTT1000
10 MIXED-IID15-TAIL1-RTT1000
11 MIXED-GE15-L3-TAIL1-RTT50
12 NOLOSS-RTT50
13 MARKOV-BD-p15-b25-RTT50
14 MARKOV-BD-p25-b45-RTT1000
```

Her cell:

```text
3 arms × N=50
```

```text
14 × 3 × 50 = 2,100 netns runs
```

Netns N=50, full calibration hassasiyeti değil environment-direction/actuator doğrulaması içindir; worst-case Wilson `%95` yarı-genişliği yaklaşık `%13.4` olduğundan küçük performans farkı iddia edilmez. Statistical policy verdict'in birincil kaynağı N=200 in-process holdout'tur. Netns'te her cell'de static karşı-olgular bulunduğu için policy seçimi doğrulanabilir. Netns RTT seviyeleri `{50,1000}` ile in-process holdout desteğinin içindedir; yalnız netns'e özgü RTT seviyesi üzerinden ekstrapolasyon yapılmaz. Markov-BD netns hücrelerinde packet capture'dan gözlenen transition matrix kaydedilir; configured `p,b` yalnız metadata'dır ve fidelity PASS gözlenen değerlerden hesaplanır.

### 8.3 Toplam result-bearing satır

```text
Frozen signal pilot      800
Training              14,800
In-process holdout    21,600
Netns holdout          2,100
TOTAL                 39,300
```

S0'nun 10.000 sentetik estimator trace'i, §4.2b'nin 80 estimator-only diagnostic run'ı ve outcome-blind runtime benchmark result-bearing toplamına dahil değildir. Joint-action Kapı-2 birden fazla ARQ treatment'ı gerektirirse bu sayı geçersiz olur ve manifest generator tarafından yeniden türetilmeden freeze yapılamaz.

---

## 9. Holdout değerlendirmesi ve verdict

### 9.0 Training gate — holdout'tan önce, bloklayıcı

Training analizi, support bitmap ve candidate fit tamamlandığında aşağıdaki kriterlerin tümü geçmeden holdout açılmaz.

```text
K1 YÖN ÇEŞİTLİLİĞİ
   >=1 R2_PREFERRED ve >=1 R0_PREFERRED training cell.

K2 ÜRÜN-YOLU AYIRT EDİLEBİLİRLİK
   Aynı configured loss seviyelerinde IID vs {MARKOV-BD-b35,GE-L3,GE-L10}
   ayrımı, her pair'den tek pre-treatment burst_delta snapshot ile yapılır.
   Stratified pairing-group bootstrap B=10.000;
   her karşılaştırmada percentile %95 AUC lower bound >=0.90.
   MARKOV-BD-b15 bir sert sınıflandırma hedefi değildir; her loss seviyesinde
   median sıralaması IID < b15 < b35 korunmalı ve b15-overlap bölgeleri
   K5 stability/support sonucuna göre R0 fallback olmalıdır.
   Ineligible pair AUC'ye girmez; eligibility oranı ayrıca raporlanır.

K3 LEAVE-ONE-CONDITION-OUT KARAR KARARLILIĞI
   Her base condition çıkarılıp fit tekrarlanır. Karar haritası canonical grid'de:
     loss_ppm  = 0..300.000 step 5.000
     burst_ppm = -200.000..900.000 step 25.000
   Yalnız §3.4b support bitmap içindeki noktalar karşılaştırılır.
   Bütün LOCO fit'ler full-training haritasıyla exact aynı olmalıdır.

K4 GÖZLEMSEL ALIAS ÇAKIŞMASI
   Aynı runtime support bin'ine düşen fakat RTT veya başka FIXED/UNSUPPORTED
   eksende ayrılan cell'ler çelişkili preferred_action üretemez.
   Çelişki varsa gözlenmeyen eksen causal karar girdisidir:
   input'a eklenmeden veya bölge tamamen unsupported yapılmadan GO yoktur.

K5 BOOTSTRAP POLICY STABILITY
   Pairing-group'lar cell içinde stratified resample edilir; B=1.000 full refit.
   Her supported canonical grid noktasında bootstrap fit'lerin >=%90'ı
   full-training aksiyonunu seçmelidir.

K6 TIED-CANDIDATE REGION IDENTITY
   §7.3 scientific tier-4 sonrası, deterministic tuple tie-break öncesi kalan
   bütün adaylar supported canonical grid'de aynı R0/R2 haritasını üretmelidir.

K7 DESTEK VE AKTİVASYON
   En az bir R2_PREFERRED cell'in eligible snapshot'larının >=%90'ı support
   bitmap içinde olmalı; aksi hâlde öğrenilen pozitif bölge üründe kullanılamaz.

K8 DATA/CONTRACT
   runtime_features_present=100%; pair completeness=100%; contract/schema/
   snapshot equality failure=0; cost contract grounded.
```

Herhangi biri düşerse `CALIBRATION-NO-GO (IDENTIFIABILITY)`; 21.600 in-process + 2.100 netns holdout satırı üretilmez. Rapor düşen kriteri, hücre/bin/grid noktasını, en yüksek `delta_completion` hücresini, `%95` CI'ını ve δ=15pp'ye kalan farkı gösterir. Mevcut training üzerinde eşik/faktör/policy yeniden ayarlanmaz.

### 9.1 Harmful false positive

```text
Policy effective action = R2
AND static holdout cell label = R0_PREFERRED
```

GO için harmful false-positive cell sayısı `0` olmalıdır.

In-process triple-pair'de ayrıca gerçek `CALIBRATED_POLICY` aksiyonu, aynı snapshot üzerinde policy artifact'ın offline inference sonucuyla exact eşit olmalıdır. Policy R0 seçtiyse ölçülen policy outcome/trace seçilmiş static-R0 davranışıyla; R2 seçtiyse static-R2 davranışıyla semantic digest eşitliği göstermelidir. Bu actuator/replay conformance kapısıdır; netns'te RNG packet-exact değilse outcome digest yerine effective-action + wire-actuation doğrulanır.

### 9.2 Gain capture

Yalnız static holdout sonucunda `R2_PREFERRED` olan hücrelerde:

```text
gain_capture =
  SUM(Pcomplete(policy) - Pcomplete(R0))
  /
  SUM(Pcomplete(R2) - Pcomplete(R0))
```

GO:

```text
gain_capture >= 0.50
policy at least one supported holdout cell'de R2 seçer
```

`0.50` eşiği ürün SLO'su veya doğa sabiti değildir; pre-outcome dondurulan `EXPERIMENTAL_ACCEPTANCE_BOUND` değeridir. Gerekçesi:

- `harmful FP = 0` değişmez güvenlik kapısıdır.
- Policy, desteklenen ve static-R2 tarafından erişilebilir completion kazancının en az yarısını yakalamıyorsa güvenli olsa bile ürün açısından aşırı muhafazakâr/yetersizdir.
- `%100` capture zorunlu değildir; support/OOD ve stability fallback'larının kasıtlı olarak kazancın bir bölümünü bırakması beklenir.
- Eşik yalnız `GO` ile `CALIBRATION-LIMITED` ayrımını yapar; safety failure'ı örtemez ve `NO-GO` koşullarını gevşetmez.

Sensitivity-only rapor:

```text
observed_gain_capture
passes_acceptance_bound_0_25
passes_acceptance_bound_0_50   # tek verdict bound'u
passes_acceptance_bound_0_75
```

`0.25/0.75` değerleri alternatif verdict üretmez; yalnız sonucun kabul eşiğine duyarlılığını gösterir. Holdout görüldükten sonra `0.50` değiştirilemez.

### 9.3 Safety/data gates

- `inv_fail=0`
- ARQ liveness failure `0`
- Hard cost-cap violation `0`
- R0 trace digest baseline ARQ ile exact
- No-loss policy R2 activation `0`
- Unsupported/ineligible R2 activation `0`
- FEC failure ARQ recovery'yi engellemez
- `runtime_features_present=1` satırların `%100`'ünde
- Missing/extra/duplicate/skip satır `0`
- Her karar raw estimator sayaçlarına geri izlenebilir
- Static R2 gerçek parity wire bytes üretir
- Policy effective action wire behavior ile eşleşir

### 9.4 Verdict

```text
FEC-ACTIVATION-CALIBRATION-GO:
  safety/data gates pass
  harmful FP = 0
  gain_capture >= 0.50
  netns actuator/direction checks pass

CALIBRATION-LIMITED:
  safety/data gates pass
  harmful FP = 0
  fakat gain_capture < 0.50 veya netns direction sınırlı

CALIBRATION-NO-GO:
  harmful FP > 0
  veya reliability/invariant/cost-cap failure
  veya runtime telemetry/actuator conformance failure
```

Hiçbir verdict default activation yetkisi vermez. Başarılı policy versioned experimental/opt-in kalır.

---

## 10. Raw veri sözleşmesi — analiz için yeterlilik

Authoritative typed schema `FS-CAL-2-RAW-SCHEMA-v3.json` olarak üretilir. Her alan için type/width, enum, nullability, unit, source symbol, validation rule ve analysis-use bulunur. `additionalProperties=false`; missing/extra/invalid enum satır atomic write öncesi reddedilir.

Phase-dependent alanlar (`configured_markov_b_ppm`, `calibration_version`, `policy_artifact_sha256` gibi) için key daima bulunur; yalnız schema'nın ilgili phase için açıkça izin verdiği canonical `null` kabul edilir. Sessiz field omission yoktur.

Canonical ham format: run başına bir canonical JSON record + SHA-256 sidecar. Key order, integer formatting, UTF-8/LF ve null representation sabittir; bilimsel artifact'larda wall-clock timestamp yoktur.

Her satırda zorunlu:

```text
identity:
  run_id
  cell_id
  pairing_group
  arm
  repetition_index
  seed
  environment
  experiment_phase
  seed_domain
  decision_block_id
  workload_profile_id
  arq_contract_id
  fec_profile_id

configured ground truth — runtime policy input'u değildir:
  configured_structure
  configured_loss_ppm
  configured_markov_b_ppm
  configured_burst_length
  configured_tail_length
  configured_rtt_ms

observed at decision epoch:
  observation_epoch_ns
  estimator_window
  estimator_resolved
  estimator_lost
  n00
  n01
  n10
  n11
  measured_loss_ppm
  p11_ppm
  p01_ppm
  burst_delta_ppm
  estimator_eligible
  warmup_complete
  flow_converged
  telemetry_stale
  runtime_features_present
  feature_snapshot_sha256
  estimator_window_source_ids[256]
  estimator_window_fates[256]       # enum SUCCESS/LOSS; yalnız fate-known
  late_ack_corrections
  spurious_loss_corrections
  observed_warmup_initial_source_sent
  observed_warmup_initial_source_lost
  observed_warmup_max_loss_run
  observed_warmup_mean_loss_run_milli
  observed_srtt_ns
  observed_rttvar_ns

decision/action:
  requested_redundancy
  effective_redundancy
  policy_activation_score
  calibration_version
  safety_result
  fallback_reason
  workload_supported
  support_eligible
  support_loss_bin
  support_burst_bin
  support_bitmap_sha256
  arq_setting_applied
  action_applied_at_ns

outcome:
  base_source_bytes
  parity_bytes
  arq_retransmit_bytes
  control_bytes
  total_bidirectional_wire_bytes
  probe_sent
  retransmits
  fec_decoded_sources
  fec_decode_success
  completion_status
  completion_ns
  censor_time_ns
  horizon_ns
  deadline_hit
  reassembly_equal
  inv_fail

provenance:
  source_commit_40hex
  binary_sha256
  manifest_sha256
  execution_order_sha256
  product_contract_sha256
  policy_artifact_sha256
  schema_version
  raw_schema_sha256
  toolchain
```

`estimator_window_source_ids/fates` sayesinde `N/lost/n00/n01/n10/n11`, loss-rate ve burst-delta analiz sırasında sıfırdan yeniden hesaplanır. Recomputed değer serialized özetle exact eşleşmezse dataset conformance FAIL olur. Bu alanlar treatment uygulanmadan önce snapshot edilir ve arm'lar arasında `feature_snapshot_sha256` exact eşitliği zorunludur.

Configured ground-truth alanlarının `FusionPolicy` call path'ine erişimi compile/test seviyesinde yasaklanır. Configured alanlar yalnız stratification/fidelity analizinde kullanılabilir; policy inference feature allowlist'i `{measured_loss_ppm, burst_delta_ppm, estimator_eligible, support_eligible, workload_supported}` ile compile-time/test seviyesinde sınırlandırılır.

Namespace'ler fiziksel olarak ayrıdır:

```text
reports/fs-cal-2/v3/pilot/raw/
reports/fs-cal-2/v3/training/raw/
reports/fs-cal-2/v3/holdout-inproc/raw/
reports/fs-cal-2/v3/holdout-netns/raw/
```

Training reader holdout namespace'lerini açamaz. Her namespace STARTED/COMPLETE ledger, expected manifest count, per-record checksum ve dataset-concat hash taşır. Raw dizin immutable/read-only olur; analiz yalnız ayrı `analysis/` namespace'ine yazar.

---

## 11. Deterministik veri işleme ve analiz pipeline'ı

### 11.1 Pipeline freeze ve fixture-first

Analysis binary gerçek outcome açılmadan önce synthetic/golden fixture'larla tamamlanır. Fixture'lar en az şunları kapsar:

- Strict schema accept/reject, missing/extra/invalid enum.
- Feature counters'ın fate vector'dan exact recomputation'ı.
- Complete/incomplete/duplicate pair ve triple rejection.
- Pre-treatment snapshot mismatch/leakage rejection.
- Valid, invalid ve censored outcome.
- R0/R2 cost accounting ve hard-cap rejection.
- Support bitmap inside/outside/boundary davranışı.
- Candidate replay, tie-set ve deterministic tuple seçimi.
- K2–K8 PASS/FAIL örnekleri.
- Bütün final artifact header/schema/traceability kontrolleri.

Synthetic suite iki bağımsız kez çalıştırıldığında bütün scientific artifact'lar byte-identical olmalıdır. Release binary hash'i, source commit, toolchain, Cargo.lock, schema, fixture-suite ve exact command pre-run freeze record'a girer.

### 11.2 Ingest ve conformance

Pipeline sırası:

```text
1. freeze/provenance/dataset hashes doğrula
2. manifest expected run-id setini yükle
3. raw record'ları strict typed schema ile parse et
4. missing/extra/duplicate/sidecar mismatch ara
5. fate window'dan estimator features yeniden hesapla
6. serialized/recomputed snapshot exact karşılaştır
7. configured-feature leakage allowlist testini doğrula
8. deterministic execution-order ile sırala
```

Herhangi bir hata satır düşürme üretmez; namespace bütün olarak `DATA-CONFORMANCE-NO-GO` olur.

### 11.3 PairRecord ve HoldoutTriple

Training/pilot için:

```text
PairRecord = {shared pre-treatment snapshot,
              R0 measured outcome,
              R2 measured outcome,
              two run_ids}
```

In-process holdout için:

```text
HoldoutTriple = {shared snapshot,
                 STATIC_R0 outcome,
                 STATIC_R2 outcome,
                 CALIBRATED_POLICY actual outcome/action,
                 three run_ids}
```

Her derived satır raw `run_id`, `cell_id`, `pairing_group`, repetition ve provenance'a geri izlenebilir. Pair/triple eksikliği veya shared snapshot eşitsizliği FAIL'dir.

### 11.4 Cell ve survival analizi

- Completion proportion: Wilson `%95` CI.
- R0/R2 paired completion: exact McNemar + pairing-group bootstrap.
- Bootstrap: cell içinde pairing-group stratified, `B=10.000`, seed `20260805`; packet/arm-row resampling yasaktır.
- Time-to-completion: Kaplan–Meier + RMST; RMST tau exact run horizon'dır.
- Censored satır latency değildir; conditional latency ayrı secondary tablodur.
- Wire/cost: paired bootstrap; `base/parity/ARQ/control/total` ayrı ve toplam olarak raporlanır.
- Deadline-hit yalnız secondary endpoint; horizon completion ile birleştirilmez.
- Netns packet-exact pairing kanıtlanamazsa önceden tanımlı seed-clustered/unpaired bootstrap kullanılır ve `UNPAIRED_ENVIRONMENT` etiketi taşır.
- Cell-label residual-risk tablosu preferred action, delta/CI, bootstrap label probability, discordant counts, boundary flag ve LOCO/K5 action-stability sonuçlarını birlikte verir; yalnız point label raporlamak yasaktır.

### 11.5 Counterfactual policy fit

Her candidate policy, training PairRecord üzerinde offline replay edilir:

```text
candidate(snapshot)=R0 → PairRecord.R0 measured outcome
candidate(snapshot)=R2 → PairRecord.R2 measured outcome
```

Hesaplanan metrikler: harmful false positives, completion gain capture, cost-cap violations, excess parity/wire regret, RMST regret ve supported activation coverage. Candidate selection yalnız §7.3 sırasını kullanır. Candidate score karşılaştırmaları exact integer/rational değerlerle yapılır; formatted float karar girdisi değildir.

### 11.6 Identifiability ve policy artifact

§9.0 K1–K8 training üzerinde çalıştırılır. PASS olursa artifact:

```text
FecActivationCalibrationV1_R0R2_LossBurst.json
  schema_version
  training_dataset_sha256
  product_contract_sha256
  arq_contract_id
  workload_profile_id
  selected thresholds/membership
  support bitmap + sha256
  safety caps
  feature allowlist
  fallback table
  identifiability results
  invalidation conditions
```

Policy artifact ve holdout binary holdout namespace açılmadan freeze edilir.

### 11.7 Holdout işleme

Holdout'ta fit/threshold/support bitmap değişikliği yasaktır. Pipeline:

1. Static R0/R2 ile holdout cell label'larını ölçer.
2. Actual policy action'ını frozen artifact'tan offline yeniden hesaplar.
3. Offline action ile product-path actual action exact eşitliğini doğrular.
4. In-process ortamda actual policy semantic outcome digest'ini seçilmiş static arm ile karşılaştırır.
5. Harmful FP, primary `gain_capture>=0.50`, sensitivity-only `{0.25,0.75}`, coverage, fallback ve safety verdict'lerini üretir.
6. Netns'te actuator/wire/fidelity yönünü ayrı raporlar.

Holdout sonucu hiçbir biçimde parameter fit'e geri verilmez. FAIL sonrası aynı holdout üzerinde retune + rerun yasaktır; yeni policy sürümü yeni holdout seed domain gerektirir.

### 11.8 Zorunlu analiz çıktıları

```text
01 INTEGRITY.json
02 DATA-CONFORMANCE.json
03 PAIR-RECORDS.csv
04 CELL-RESULTS.csv
05 SUPPORT-BITMAP.json
06 CANDIDATE-SUMMARY.json
07 IDENTIFIABILITY.json
08 CALIBRATION-POLICY.json
09 HOLDOUT-RESULTS.csv
10 NETNS-FIDELITY.json
11 FINAL-REPORT.md
12 FINAL-CHECKPOINT.md
```

CSV/JSON satırları raw run-id'lere traceable olur. Her artifact SHA-256 sidecar ve analysis ledger kaydı taşır. Bütün sayılar measured fact, frozen-rule classification veya limitation olarak etiketlenir.

---

## 12. Freeze ve yürütme sırası

### 12.0 Outcome-blind runtime ve paralellik ölçümü

Takvim tahmin edilmez; outcome açmadan ölçülür:

```text
M1: p=30%, RTT=1000, STATIC_R2 diagnostic, N=20
    ayrı seed/namespace
    runner yalnız wall_time_ms + peak_rss_bytes döndürür
    scientific stdout/stderr/raw artifact mekanik olarak kapalı
    completion/cost/latency/action değerleri persist/log edilmez

M2: aynı synthetic trace 2 ve 8 process ile
    event_log_sha256 exact aynı olmalı
```

M1/M2 training, pilot veya holdout'a girmez. M2 düşerse kampanya seri çalışır; bilimsel tasarım değişmez. Netns her hâlde seri kalır.

### 12.1 Üç yetkilendirme bloğu

```text
YETKİ-1 — ARAÇ/GROUNDING (result-bearing = 0)
  §4.0 product contract + §4.0b joint action + §4.0c cost contract
  estimator + FEC_R2 actuator + R0/R2 golden proof
  typed raw schema + manifest generators + deterministic analysis pipeline
  synthetic/golden suite ×2 + hosted CI
  §12.0 M1/M2 + S0 10.000 synthetic + §4.2b 80 estimator-only diagnostic
  PRE-RUN CHECKPOINT ve DUR

YETKİ-2 — SIGNAL PILOT + TRAINING
  pilot/training source, schema, manifests ve analysis binary birlikte freeze
  800-run frozen signal pilot
  pilot gate FAIL → DUR; retune/rerun yok
  pilot gate PASS → ara onay olmadan 14.800 training run
  counterfactual fit + support bitmap + §9.0 K1–K8
  identifiability FAIL → DUR; holdout açılmaz
  PASS → policy/identifiability CHECKPOINT ve DUR

YETKİ-3 — HOLDOUT/HÜKÜM
  frozen policy + frozen holdout manifest doğrula
  21.600 in-process holdout
  2.100 faithful netns holdout
  12 analysis artifact + final verdict + handoff
```

Code ve araçlar çalışır hâle gelmeden aspirational manifest freeze edilmez. Yetki bloğu içinde mekanik mühendislik ayrıntıları ara onay gerektirmez. Bilimsel DUR yalnız tanımlı kapı FAIL'lerinde olur. Holdout raw namespace training/fit process'i tarafından dosya sistemi seviyesinde açılamaz.

---

## 13. Makine ve git güvenliği

- Lokal Cargo `CARGO_BUILD_JOBS=2` veya `-j 2`.
- Aynı anda tek Cargo process.
- Full workspace/release/all-features/miri/deny/audit yalnız hosted CI.
- Compilation ve netns campaign aynı anda çalışmaz.
- Her run atomic write + ledger ile kaydedilir.
- Interrupted infrastructure attempt INVALIDATED olarak korunur; farklı source/freeze kimliği olmadan sonuçlar karıştırılmaz.
- Kullanıcının mevcut WIP dosyaları stage/modify/clean edilmez.

---

## 14. Kapsam sınırı

Bu deney başarılı olursa kalibre edilmiş sayılacak:

```text
ARQ üstüne R2 FEC activation subpolicy
inputs: measured loss-rate + burst-delta + estimator/support/workload eligibility
valid only under frozen arq_contract_id and bulk workload_profile_id
```

Kalibre edilmiş sayılmayacak:

```text
Jitter membership
Queue-delay membership
ACK-confidence membership
ARQ aggressiveness
Send-rate/compression output
R1/R4 redundancy
Default activation
K varyasyonu
Quinn/TCP/KCP superiority
```

Yapı ekstrapolasyonu test edilmez: training ve holdout `IID`, `MARKOV-BD`, `GE-L3`, `GE-L10`, tail ve mixed ailelerini kullanır; holdout görülmemiş loss-rate ve intermediate-burst seviyeleriyle interpolasyon ile environment fidelity'yi sınar. Ürün runtime'da yapı etiketi kullanmaz; support bitmap dışında kalan gözlenen feature bölgesi `R0/OUT_OF_SUPPORT` üretir. Aynı feature bölgesinde gözlenmeyen eksene bağlı action çelişkisi §9.0/K4 ile NO-GO'dur.

In-process sonuç kernel/netem/NIC sonucu değildir. Netns N=50 yalnız actuator, direction, boundedness ve fidelity kanıtıdır. Küçük mesaj/workload, jitter/queue/ACK-confidence üyelikleri ve değişken ARQ ayarı bu artifact'ın geçerlilik alanı dışındadır ve mekanik fallback/invalidation taşır.

Residual statistical risk: 37 training condition'ın cell label'ları sonlu N=200 örneklemden üretilir. K3–K6 ve ayrık holdout bu riski sınırlar fakat sıfırlamaz; boundary-label dağılımı ve stability sonuçları final raporun zorunlu limitation bölümüdür.

Bu ayrım final rapor ve ürün config adında açıkça görünmelidir. Başarılı artifact adı örneği:

```text
FecActivationCalibrationV1_R0R2_LossBurst
```

---

## 15. GO sınırı ve “veri işlenebilir mi?” hükmü

Bu belgenin onayı yalnız `YETKİ-1` için GO anlamına gelir. Signal pilot, training ve holdout kendi freeze/checkpoint koşulları sağlanmadan yetkili değildir.

Veri **tasarım olarak işlenebilir hâldedir**, çünkü:

- Typed strict raw schema bütün feature/action/outcome/provenance alanlarını tanımlar.
- Ham fate snapshot saklandığı için estimator feature'ları bağımsız yeniden hesaplanabilir.
- R0/R2 karşı-olguları PairRecord'a, holdout üçlüsü HoldoutTriple'a deterministik bağlanabilir.
- Censoring, completion, deadline ve latency birbirine karıştırılmaz.
- Candidate policy gerçek ölçülmüş karşı-olgular üzerinde replay edilir.
- Support/OOD, alias, identifiability ve actuator conformance ayrı kapılardır.
- Raw → derived → policy → holdout artifact zinciri run-id seviyesinde izlenebilir.

Fakat veri ancak §4.4 `DATA-PROCESSABLE-PASS` ve §11.1 deterministic pipeline freeze'i gerçek kod üzerinde PASS olduktan sonra **fiilen işlenebilir** kabul edilir. Şema/pipeline implementation'ı eksikse deney koşulmaz; “sonra analiz ederiz” yaklaşımı yasaktır.

Result-bearing nihai satır bütçesi, yalnız joint-action space `1 × {R0,R2}` ise:

```text
signal pilot             800
training              14.800
in-process holdout    21.600
netns holdout          2.100
TOTAL                 39.300
```

Bu toplam replikasyon gücü kadar feature/action coverage içerir: 37 training condition, intermediate-burst treatment'ları, üç RTT desteği, static karşı-olgular ve actual policy arm. Tam fuzzy engine Faz B/C/D çıktıları için yeterli değildir; yalnız bu belgede tanımlı Faz-A artifact'ını üretir.
