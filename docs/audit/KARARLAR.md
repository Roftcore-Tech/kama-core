# Sahip kararı gereken belirsizlikler — biçimsel karar listesi

Taban: ürün hattı `125b9afa`. Satır numaraları o ağaçtadır. Kaynak: bu dizindeki denetim
(`README.md` §3–§8 ve okuyucu raporları).

Amaç: her belirsizliği, sahibin alternatifleri matematiksel olarak modelleyip ispatlayabileceği
biçimde yazmak. Her kararda şunlar var:

- **S** — tek cümlelik soru.
- **K** — bugünkü kodun kuralı, biçimsel olarak, satırlarıyla.
- **O** — otorite metni (aynen) ve kodla ayrıştığı yer.
- **Ö** — kodda bulunan ürün ölçütleri. Ölçüt kodda yoksa öyle yazıldı.
- **A0, A1, …** — seçenekler, uygulanacak kural olarak. A0 her zaman bugünkü koddur.
- **P** — modelin girdileri: sabitler (dosya:satır) ve ölçülmüş ağ değerleri.
- **T** — yalnız koddan ve sabitlerden türettiğim bağıntılar. Bunlar **ölçülmedi**. İspatta
  girdi olarak değil, sınanacak önerme olarak kullanılmalı.

Bir seçenek seçilip ispatlandığında uygulama o kuralın aynısı olacak. Doğrulama: ürünün mevcut
birim testleri ve değişen davranışın tek birim testi. Kural ispatlıysa ve ürün yine tutmuyorsa,
fark uygulamadadır.

Spec metnine dokunulmadı. Spec'i aşan değişiklik sahibin tarihli kararıyla yazılır.

---

## 0. Ortak model parametreleri

### 0.1 Ağ (netem ürün hücresi; ölçülmüş ya da kurulumdan)

| simge | değer | not |
|---|---|---|
| `P` | 12 500 B/s | ileri profil hattı (UDP + IP baytı) |
| `C_f` | 12 500 → 6 250 → 12 500 B/s | gerçek ileri hat; düşüş-toparlanma hücresinde 20 s'de geri döner (`line_guard.rs` 1379) |
| `C_r` | 62 B/s | dönüş hattı |
| `d` | 300 ms | tek yön gecikme (iki yön) |
| `p` | 0,20 | ileri yönde rastgele kayıp |
| `b` | 1 200 B | tbf kovası; IP baytı sayar (stab overhead −14) |
| `B` | 64 datagram | pfifo derinliği |
| `M` | 1 200 B UDP gövdesi, tel 1 228 B | `DEFAULT_MTU` + 28 B zarf |

### 0.2 Geri bildirim (kod)

`receiver.rs` 80–122:

- Dönüş bütçesi `Y·C_r`, `Y = 250‰` → `⌊62·250/1000⌋ = 15 B/s`.
- Tipik ACK 81 B (65 + 4·4). `budgeted_ack_interval = 81 / 15 = 5,40 s`.
- En büyük ACK 129 B. `feedback_horizon = 129/15 + 129/62 = 8,60 + 2,08 = 10,68 s`.
- Ölçülen ACK aralığı (kod yorumu, `line_guard.rs` 1378): 8–9 s.

Profil bayrağı: `link_feedback_starved` `budgeted_ack_interval ≥ 1 s` ise açıktır
(`runtime.rs` 482–489).

### 0.3 Ürün ölçütleri (kodda bulunan)

- **PTT gecikmesi ≤ 1 s.** Bileşenleri (`line_guard.rs` 418, 431–436): ileri yayılım 0,3 s; önde
  serileşen bir parça 0,17 s (6 250'de); kopya yayılımı 0,16 s; demet 0,02 s. Kalan pay ≈ 0,35 s.
- **PTT bandı ≤ 4,5 kbit/s/yön.** Sahip, 2026-11-22.
- **İkinci dosya ≤ 14 s.** Düşüş-toparlanma hücresi (`line_guard.rs` 1379–1380).

Hedef 1, 3 ve 4'ün tam metni #175 koordinatör tablosunda (7092136652). Bu belge yalnız koddaki üç
sınırı aktarıyor. Ölçütleri sahip düzeltir.

---

## Karar 1 — Geri bildirimi aç profilde pacing ve uçuşun kontrolcüsü (S-01, S-02, S-25, S-26, S-91)

**S.** `FeedbackStarvedRule` açıkken pacing hızını, uçuş tavanını ve burst'ü hangi kural belirler?

**K (bugün).**

```
pace(t)   = cc_feedback_cap = P_cap · min(1, L(t)/P)                    sender.rs 3183–3185, 2782–2799
            L(t) = LineGuard.pace_bps                                    line_guard.rs 860
            BBR pacing_rate = pacing_gain × BtlBw hesaplanır, OKUNMAZ    sender.rs 3162–3166 (üzerine yazılır)
flight(t) = max( BBR.window_without_recovery ,                          sender.rs 2761–2780
                 cc_feedback_cap · (feedback_horizon + rtprop) )
burst     = clamp( cc.window() · 2 ms / srtt , 10·M , 256·M )           sender.rs 3155, 3187
            cc.window() recovery'de min(cwnd, recovery_window)           cc/bbr.rs 596–605   (Y-01)
loss      → BBR.on_congestion_event (durum değişir; uçuşu bağlamaz:
            max'ın ikinci terimi tabandır)                               sender.rs 4811–4815
          → LineGuard.overflow_estimate (pace'i düşürebilir)             Karar 2
yukarı    → LineGuard yoklaması (begin_probe/probe_to)                   line_guard.rs 1855–1901
          → on_pair_rate (doğrudan, kalıcı)                              Karar 3
```

Sonuç: aynı yolda iki tahminci var. LineGuard hız ve yukarı yön kararını tek başına verir. BBR
ProbeBW/ProbeRTT çalışır ama hiçbir karar yolunda okunmaz (Y-02). ProbeRTT penceresi tabanın
altında kaldığı için onun da etkisi yok.

**O.**

- INV-RATE-1 2026-11-11: «loss PACING HIZINI düşürmez (pace = BBR pacing_gain × BtlBw; BtlBw
  loss'tan türetilmez)».
- 2026-11-16 profil istisnası: «recovery_window uçuş tavanını SINIRLAMAZ; tavan en az hat ×
  (geri bildirim ufku + RTT), pacing profil hattıyla sert sınırlı (ölçüm aşamaz). Loss yine
  ölçülür, FEC/ARQ'yu ve BBR durumunu besler.»
- engine-spec §7.4 INV-ENGINE-BW: «bandwidth DAİMA delivered-rate'ten (BtlBw); loss'tan ASLA
  türetilmez.»
- engine-spec başlık YAMA 2026-11-21 §2.2 LineGuard ölçümünü kısmen tanımlıyor: ilk pencere,
  aralık ve iki nokta tahmini aşağı, süreli yoklama yukarı. Aşağıdakiler tanımsız:
  - ölçünün pacing'in kaynağı olması,
  - taşma kuralı,
  - çift alt sınırıyla açma,
  - boşluk bütçesi,
  - çift kipi.

Ayrışma: otorite `pace = min(gain·BtlBw, P_cap)` diyor, kod `pace = P_cap·min(1, L/P)`.

**Ö.** PTT ≤ 1 s; ikinci dosya ≤ 14 s; düşüşte kuyruk PTT payını aşmasın (≈ 0,35 s).

**Seçenekler.**

- **A0 (bugün) + YAMA.** Kural yukarıdaki gibi kalır; INV-RATE-1'e "profil açıkken pace =
  LineGuard" yazılır.
- **A1 (Yön A: tek kontrolcü LineGuard).**
  - `pace = P_cap·min(1, L/P)`.
  - `flight = cap·(H + rtprop)`; BBR `cwnd` max'tan çıkar.
  - `burst = clamp(flight·2 ms/srtt, 10·M, 256·M)`; recovery'siz pencereden.
  - cc-port yalnız kayıp tespiti, ACK muhasebesi ve RTT için kalır. BBR durum makinesi bu
    profilde kapalı.
  - INV-RATE-1'e YAMA gerekir.
- **A2 (Yön B: tek kontrolcü BBR, LineGuard ölçer).**
  - `pace = min(P_cap, gain(phase) · max(BtlBw, L_meas))`.
  - `L_meas`, LineGuard'ın aşağı yönlü örneğidir (kuyruk artışı). BBR'a dış bant girdisi olarak
    verilir.
  - LineGuard yoklaması, başlangıç bütçesi, çift açma ve çift kipi silinir. Yukarı yönü BBR
    ProbeBW kazancı (1,25) sınar.
  - Metne en yakın seçenek; istisna yalnız "ölçüm kaynağı" için gerekir.
- **A3 (metnin harfi).** `pace = min(gain·BtlBw, P_cap)`, LineGuard yok.

**P.**

- Ölçülmüş: bu hatta BtlBw `≈ 0,31·C_f`.
  - 12,5 kB/s hatta 3,9 kB/s; 6,25 kB/s hatta 2,8–2,9 kB/s (`sender.rs` 3176–3178).
  - A3 benzeri okumada hız 2,8 → 1,5 → 1,1 kB/s, dosya 60 s'de bitmedi (3178–3181).
- Sabitler: `PROBE_QUEUE_NS = 80 ms` (437), `STEP` 500/62/1000‰ (408–410),
  `VERIFY_MIN_SPAN = 3 s` (398), `VERIFY_GROWTH_DIV = 40` (403), `VERIFY_MIN_PERMILLE = 900`
  (406).
- `H = 10,68 s`, `T_ack ≈ 5,4` (bütçe) ile 8–9 s (ölçülen).

**T (türetilmiş, ölçülmedi).**

- **T1.1 — A3 neden çöker.** BBR'ın teslim örneği `delivered / interval`.
  - Seyrek ACK'te interval `≥ T_ack`. Bayt kayıp kesriyle `(1−p)` küçülür.
  - `BtlBw ≤ (1−p)·pace`. Pace `gain·BtlBw` ise ProbeBW ortalaması `ḡ = 1`.
  - O zaman `BtlBw_{n+1} ≤ (1−p)·BtlBw_n`: geometrik iniş.
  - Ölçülen 2,8 → 1,5 → 1,1 dizisi bununla tutarlı. Oran 0,54 ve 0,73; `(1−p) = 0,8`'den sert,
    ek bir terim var.
  - Önerme: `p > 0` ve teslim örneği kaybı saydığı sürece A3'ün sabit noktası 0'dır.
- **T1.2 — A2 için gereken koşul.** BtlBw düzeltilmeden A2'de de aynı iniş olur, çünkü
  `max(BtlBw, L_meas)`'ta `L_meas` yalnız aşağı yönlüdür.
  - A2'nin işlemesi için: ya teslim örneği kayıpla bölünmeli (`delivered/(1−p̂)`), ya `L_meas`
    tutulmalı.
  - İlki §7.4 «loss'tan ASLA türetilmez» kuralıyla çelişir. Bu yüzden A2 de ayrı bir YAMA ister.
- **T1.3 — Uçuş tabanı hiç bağlamıyor.** Pace `c` iken uçuştaki bayt `≈ c·(T + s_r + RTT + q)`.
  - Taban `c·(H + rtprop)`. `H = T_max + s_r`.
  - Taban bağlar ⇔ çevrim `> H + rtprop`.
  - Kapalı döngüde ölçüldü: uçuş kapısı reddi 0, her hücrede (`6d9ddd9c`, benzetim).
  - Sonuç: S-02 ("hat" = profil mi, ölçülen mi) bu hücrelerde davranışa yansımaz; metin uyumu
    sorusudur.
- **T1.4 — Y-01 dar hatta etkisiz.**
  - `cc.window() ≤ 138 kB` iken `cc.window()·2 ms / srtt ≤ 138 kB · 2 ms / 0,6 s ≈ 460 B`. Bu, `10·M = 12 280 B`'nin altında.
  - Burst her zaman `10·M` tabanında; recovery penceresi sonucu değiştirmez.
  - Geniş profilde (`window·2 ms / srtt > 10·M`) değiştirir.
  - Not: `10·M` burst 12,5 kB/s hatta ≈ 1 s'lik hattır. Kovada (`b = 1 200 B`) karşılanmayan
    kısmı pfifo'da kuyruk olur.

**Bağımlılık.** Karar 2 ve 3 bu kararın içindedir. A2 seçilirse ikisi de silinir. A0 ya da A1
seçilirse ayrı ayrı karar gerekir.

---

## Karar 2 — Taşma kuralı: kayıp kesrinden hız düşürme (S-03)

**S.** `overflow_estimate` kayıp kesrinden pace'i düşürebilir mi?

**K.** `line_guard.rs` 1248–1305; tüketim 1228–1233, 1757–1778.

```
Tanım: f = rx_wire / wire  (aralık (first,last]; tel baytı)
       taban φ = Σfloor_rx / Σfloor_sent   (kuyruk kanıtı yok ve |g| ≤ eşik olan aralıklar; 16..256 M; tavanda yarılanır)
Tetik: wire ≥ 8·M  ∧  Σfloor_sent ≥ 16·M
     ∧ f < (3/4)·φ                         OVERFLOW_EXCESS 3/4 (456–457)
     ∧ growth_recent > 0                   son 3 ölçülü ACK'te artış örneği (460, 1219)
     ∧ sent_bps ≥ 0,9·pace                 (1282–1283)
Büyüklük: pair_bound var  → pair_bound < min(pace, P) ise pace := pair_bound, yoksa örnek yok
          pair_bound yok  → g ≥ −eşik ise pace := (rx / φ) / rx_span     ← kayıptan türeyen hız
Uygulama: pace := clamp(est, 1, min(pace, P))   (1763–1764)
```

**O.** INV-RATE-1 (yukarıda); §7.4 INV-ENGINE-BW. Kuralın kendisi: spec'te yok (YAMA 2026-11-21
§2.2'de de yok).

**Seçenekler.**

- **A0.** Bugünkü kural + INV-RATE-1'e tarihli istisna: «kuyruk kanıtlı ve tabanın 3/4'ünün
  altındaki teslim kesri tıkanıklık sayılır».
- **A1.** Tetik aynı. Büyüklük yalnız `pair_bound` olabilir. Alt sınır yoksa örnek yok:
  kayıptan türeyen hız kalkar.
- **A2.** Taşma kuralı tamamen kalkar. Aşağı yön yalnız kuyruk artışı örneğiyle (1215–1224) ve
  cephe örneğiyle olur.

**P.** `p = 0,2` · `B = 64` · `b = 1 200 B` · 8 paketlik FIFO benzetimi · kod yorumu: rastgele
kayıpta `f/φ ≥ 0,9`; 8'lik FIFO taşmasında 0,50–0,67 (454–455).

**T.**

- **T2.1 — Yanlış tetik olasılığı** (kuyruk kanıtı ve tam hız şartı verilmişken).
  - `n` datagramlık aralıkta `X ~ Bin(n, 1−p)`. Tetik `X/n < 0,75·0,8 = 0,6`.
  - `n = 8` (en küçük aralık): `P(X ≤ 4) = 0,0563`.
  - `n = 40`: `P(X ≤ 23) = 9,9·10⁻⁴` (kesin binom).
  - Gerçek oran, kuyruk kanıtının olasılığıyla çarpılır.
- **T2.2 — A2 ne zaman yeterli.** Artış örneği, kuyruk bir ölçüm aralığı boyunca büyürse
  görünür.
  - Hız `c > C_f` iken kuyruk `B·M / (c − C_f)` sürede dolar. Sonra `g ≈ 0`, yalnız düşme olur.
  - A2 yeterli ⇔ dolum süresi `≥` bir ölçüm aralığı (≈ `T_ack`).
  - `B = 64`, `c = 12 500`, `C_f = 6 250`: dolum 78,6 kB / 6 250 = 12,6 s > 8,5 s → artış
    görünür.
  - 8'lik FIFO'da 1,57 s < `T_ack` → görünmez. Taşma kuralı bu durum için yazılmış.
  - Karar girdisi: hedef hücrelerin kuyruk derinliği. Ürün hücresi `B = 64`.

---

## Karar 3 — `on_pair_rate`: tek çiftle doğrudan ve kalıcı açma (S-04)

Durum: kilitli. #262 park, `receiver.rs` alarm aralığının tek yazıcısı Web. Karar Web PR'ı ve
netem koşusundan sonra verilir. Model girdileri şimdiden hazır.

**K.** `line_guard.rs` 1381–1407.

```
Ön koşul: pair_allowed (yalnız bu ACK aralık kipinde ve aşağı yönlü örnek vermediyse; 1815–1817)
          ∧ pace < P
w = pair_udp · (M+28)/M                          (1411–1418)
floor = max(pace, refuted_bps)
w ≤ 1,05·floor → yok
w ≥ 0,95·P → pace := P, koruma kalkar (rate_bps := None); değilse pace := w
probe := None; refuted := None                    — yoklama ve hüküm YOK, tek ölçüm kalıcı
```

Pacer çift kipi: `PAIR_PERIOD_NS = 500 ms` (`pacer.rs` 41).

**Seçenekler.**

- **A0.** Doğrudan kalıcı açma.
- **A1 (K2′ / #262).** Alt sınır yalnız `up = w` hedefli bir yoklama kurar. Kalıcılık
  yoklamanın hükmüne bağlıdır.
- **A2.** Silinir; yukarı yön yalnız süreli yoklamayla bulunur.
- **A3 (b tasarımı).** Hedef = profil hattı. Kanıt, alarm sıfırlamasını atlatan tren kanıtıdır
  (alıcıda; Web).

**P (ölçülmüş / kurulumdan).**

- Sıkışma eşiği `0,9·(b_udp+28)/P`: 77,33 ms (1 074 B IP), 78,19 ms (1 086 B).
- Tam kovada çift aralığı 75,84 ms (12 500), 151,68 ms (6 250). Boş kovada 85,92 / 171,84 ms.
- 12 500'de gerçek gönderici çiftlerinin 56/57'si sıkışmış.
- Yükseliş kanıtı yalnız dönüşte ≥ 3 datagramlık duran kuyruktan gelmiş.
- Yoklama bütçesi 6 datagram / 7 368 B.
- b tasarımının yanlış geçişi için kovada ≥ 2 datagram kredi gerekir (2 148–2 456 B). Kova
  `b = 1 200 B` → Q2 sağlanıyor.
- `n` datagramlık trende `p = 0,2` iken ardışık iki datagramın birlikte ulaştığı en az iki çift
  olasılığı: `n = 4` 0,614 · `n = 5` 0,799 · `n = 6` 0,868.
- Hedefin 2 katı tren boyu: `n ≤ 1 + 2/(1 − taban/hedef) = 5`.
- Kapalı döngü (`6d9ddd9c`, benzetim):

  | kol | sonuç |
  |---|---|
  | açık | 2/6 GEÇTİ (4'ü `ack_kapsamadi`) |
  | kapalı | 3/6 |
  | sahte kol | 1/6 yanlış kalıcı yükselme |

  Kaynak: `docs/results/yerel/k2-kovali-6d9ddd9c/` (park dalında).
- Bayt ölçeği kayması: alıcı sayımı datagram başına +100 B iken alt sınır 6 863 (gerçek 6 250)
  çıkıyor (1367–1369).

**T.**

- **T3.1 — Yoklamayla toparlanmanın alt sınırı.** Bugünkü adım takvimi şöyle: örnekten sonra
  62‰, her geçişte ×2, en çok 1000‰ (1728, 1778).
  - `6 250 → 12 500` (oran 2) için çarpanlar 1,062 · 1,124 · 1,248 · 1,496. Toplam çarpan 2,23,
    yani **4 hüküm**.
  - Her adım en az iki ACK ister:
    1. hüküm ACK'i,
    2. yeni tavanda ≥ 3 s gönderilmiş "doğrulanmış" en yeni aralık (1612–1621; hüküm aralığı
       doğrulama sayılmaz, 1754).
  - `t_rec ≥ 4·2·T_ack` → 43 s (`T = 5,4`) ile 68 s (`T = 8,5`).
  - Ölçülen 38–42 s'de 7 086–8 376'da kalma (1379–1380) bununla tutarlı.
  - Önerme: hüküm başına ≥ 2 ACK gerektiren her yoklama düzeni, `T_ack ≥ 5,4 s` iken 14 s
    ölçütünü tutamaz. Yukarı yön ≤ 1–2 ACK'te çözülmeli.
- **T3.2 — Tek adımda profile yoklamanın bedeli.** Başarısız yoklamanın kurduğu kuyruk
  (probe_to 1876–1889):
  - `E = max(Q, 8·M_w·(up−c) / (up·c))`, `Q = 80 ms`.
  - `c = 6 250`, `up = P = 12 500` için `E = 8·1 228 / 12 500 = 0,786 s`.
  - Bu, PTT payını (≈ 0,35 s) aşar.
  - En küçük-datagram tabanı 8 yerine 2 olursa (`begin_start_probe`, 1868): `E = 0,196 s`.
- **T3.3 — Çift kanıtının varlık koşulu.** Hat tavandan hızlıyken (`c < C_f`) kova `b`, iki çift
  arasında dolar: 500 ms'de 6 250 B ≫ 1 200 B.
  - Çiftin ikincisi kova kredisiyle geçer. Aralık hattı değil gönderici aralığını ölçer, sıkışmış
    sayılır.
  - Önerme: `c < C_f` ve `b ≥ M` iken çift ölçüsü hattı ancak duran kuyrukla verir. Duran kuyruk
    ancak `c > C_f` iken oluşur.
  - Yani toparlanmayı çift ölçüsüyle bulmak için gönderim, profil tarafına yükseltilmiş bir
    yoklama treni olmalı. Bu, A1 ve A3'ün ortak gereği.

---

## Karar 4 — Sunucu yuva emekliliği (S-05)

**S.** 105 s sessiz kalan yuvanın emekliye ayrılması INV-NO-IDLE-TEARDOWN ile nasıl uyumlu olur?

**K.** `client-core/src/runtime.rs` 5879–5891, 6007–6020, 6221–6259; istemci 3750–3752,
4417–4422, 4455.

- Sunucu `idle_hold = 105 s` sessiz yuvayı emekliye ayırır; oturum açık kalır.
- Geç DATA yeni yuva açar. PARAM bağı, CAPS/bitmap seçimi, dedup penceresi ve FEC blokları
  sıfırlanır.
- İstemci `param_confirmed` / `caps_confirmed` sonrasında bunları bir daha göndermez.
- Sonuç:
  - onarımlar ön-PARAM bekletmesinde kalır (≤ 64, çözülmez),
  - ACK 0x14'e döner (0x15 ölçüm aralığı ve çift ölçüsü kaybolur).
- Kod okuması; koşuyla doğrulanmadı.

**O.** transport-layer-tdd §0 INV-NO-IDLE-TEARDOWN: «Bir oturum ya da bekleyen niyet, ZAMAN
GEÇTİĞİ İÇİN sonlandırılmaz». noise-wire §8: PARAM bağı oturum başınadır. 105 s spec'te yok
(yalnız ZM-3 [CAL]-aday).

**Seçenekler.**

- **A1.** Yuva yalnız oturum kapanınca emekliye çıkar (bellek tavanı oturum sayısına bağlanır).
- **A2.** PARAM/CAPS/dedup/FEC durumu yuvaya değil oturuma bağlanır. Yuva düşer, durum kalır.
- **A3.** Yeni yuvada sunucu "bağ yok" bildirir; istemci PARAM/CAPS'i yeniden kurar. Bu, tel kod
  noktası ister (Karar 9).

**İspat nesnesi.** Canlılık önermesi: «açık oturumda her onarım sonlu sürede çözülür ya da tipli
hata verir». A0 bunu ihlal eder: sessizlik > 105 s ise, sonra gelen ilk onarım. Bellek önermesi:
A1'de yuva sayısı ≤ açık oturum sayısı.

---

## Karar 5 — CC-PORT YAMA'sında listelenmeyen port farkları (S-08, S-09, S-17..S-21, A6-10)

Her biri ikili bir karar: **Y** = YAMA'ya alınır (kod kalır), **G** = spec'e geri çekilir (kod
değişir).

| kalem | kod kuralı | spec kuralı | not / türetilmiş |
|---|---|---|---|
| S-08 E2a v1.1 | `ack_delay` doygunsa ya da düzeltme reddedilip `ack_delay ≥ rtprop` ise RTT örneği atlanır (`sender.rs` 4180–4202, 2723) | §6 [TUR-1x] «RAW sample işlenir, örnek ATLANMAZ» | `ACK_DELAY_ENC_MAX = 65 535` birim (`receiver.rs` 80). `T_ack ≈ 8,5 s` iken doygunluk birim çözünürlüğüne bağlı. G'de aç profilde RTT örneği `ack_delay` ile şişer. |
| S-09 PreConverge PTO | Merdiven en eski birimi taşımıyorsa PTO kurulur (U14 canlılık; 4465–4507) | §3.3-C(5) «PTO ARM EDİLMEZ»; §12 NOT | G'de ilk ACK kaybolursa en eski birimin canlılığı yalnız merdivene kalır. |
| S-17 retx tavanı | Port kurucusu tavanı kaldırır (`RetxConfig::rfc9002_cc_port`, 2721) | §9.1 ARQ_RETX_CAP [CAL] → FEC/forward ya da connection-fail | G'de `p = 0,2` iken bir birimin `k` denemede kaybolma olasılığı `0,2^k`; tavan `K` ise `P(fail) = 0,2^K`. |
| S-18 ReorderTracker | Ömür boyu en büyük; paket eşiği `max(3, 2·max_reorder)` (`arq.rs` 154–185; `sender.rs` 4095) | YAMA (4) «paket eşiği 3»; §9.3 `max(K, ρ·recent_max_reorder)` («recent» tanımsız) | İki otorite çakışıyor. Tek 500'lük sırasızlık eşiği kalıcı 1000 yapar. |
| A6-10 PTO tavanı | Etkin tavan 30 s (`tail_recovery.rs` 89–99, 186–190) | YAMA (5) «PTO tavansız» | ESLEME satır 38'de U15 adayı olarak beyanlı. |
| S-19 Duplicate tombstone | Duplicate kökenli kayıp ilanı kayıt bırakmaz; birim motordan düşebilir (13826–13875) | §5 [TUR-2b] kayıt SAKLANIR; ZEYL-2 | Recovery-obsesif taban (CLAUDE.md §2). İki motoru değiştirir. |
| S-20 ack_confidence | `θ = max(3·srtt, 2·EAI)` (9341–9344; `K_CONF_EAI = 2`, 1982) | §5.1 `θ = 3·srtt` | Aç profilde `3·srtt ≈ 1,8–6 s < T_ack` → G'de her ACK arası "sessizlik" sayılır. Kodda `2·EAI = 10,8 s` (`EAI = 5,4`). |
| S-21 INV-RATE-1 golden | Kilit testi varsayılan olmayan yolda koşuyor (11168–11278) | — | Karar 1'e bağlı: hangi kuralın kilitleneceği. |

---

## Karar 6 — DR-EST-1 kuruluş kopyası (S-11)

**K.** `sender.rs` 8579–8644, 1920–1925, 2147–2155, 3519–3534.

- Kopya token'sızdır (pacer ve uçuş penceresi dışında). Varsayılan açık.
- Aralık 500 ms'den 8 s'ye, duvar saati sabitleriyle katlanır.

**O.** engine-spec §3.3-B: «pacer'ı baypas eden gönderim yolu YOK (DATA/retransmit/kopya tek
token bucket'ı paylaşır)». §16.2 ZM-2: duvar saati sabiti 4 decade doğrulaması ister. DR-EST-1
otorite belgelerinde yok.

**Seçenekler.** A0 (muaf) + YAMA · A1: kopya pacer kovasından geçer.

**T.** Pacer dışı bayt üst sınırı: kopya başına `M_w`. `t ≤ 8 s` için aralıklar 0,5 · 1 · 2 · 4 ·
8 s.

- Kopya sayısı `≈ 5`, hat dışı bayt ≈ 5·1 228 = 6,1 kB. 12 500'de 0,49 s'lik hat.
- Bunun 1 200 B'yi aşan kısmı kuyruk olur.
- A1'de kuruluş kopyası PTT ile aynı kovayı paylaşır.

---

## Karar 7 — Spekülatif FEC kolu, Q18 yapı kapısı ve sabit kalibrasyonu (S-12, S-31, S-25)

**K.**

- FEC arbiter: kanonik fuzzy oranının yanında iki kol var (`sender.rs` 336–448, 4877–5032,
  5152–5234; `loss.rs` 94–108):
  - spekülatif Suspected kolu `bounded_v1` (ürün varsayılanı; L0/L1/L2, persist ufku, reorder
    indirimi, `spec_min` tabanı),
  - Q18 yapı kapısı 100 000 ppm (kodun kendi belgesi: «unsanctioned magic constant»).
- Kolların birleşimi: `r = max(r_fuzzy, r_spec, r_q18)`.
- `eligible()` okunmuyor.
- fuzzy/histerezis fixture'ları "KALİBRE DEĞİL" işaretli ve ürün yolunda (`fusion.rs` 316–374,
  503–548; `emergency.rs` 91–126).
- LineGuard'ın 20'den fazla sabiti benzetimden seçilmiş (§0 ve Karar 1–3).

**O.** engine-spec §5 `loss_pre_fec = Confirmed_units / pencere`; §8.4 kanonik kural tablosu;
başlık «Tunable sabitler `[CAL]` = netem baseline'dan (ADR-18) kalibre edilir, UYDURULMAZ».

**Seçenekler.**

- A0 + YAMA (kollar spec'e).
- A1: spekülatif kol ve Q18 ürün varsayılanından çıkar; yalnız §8.4 fuzzy kalır.
- A2: kollar kalır, sabitler netem kalibrasyonuna bağlanır (ADR-18 kampanyası; ayrı iş).

**Model için.** Arbiter `max()` olduğundan parite oranı kolların en büyüğüdür. Bir kolun yanlış
pozitifi parite bant bedelidir: `Δbayt = (r − r_fuzzy)·k·M`.

---

## Karar 8 — KS-PN-1 ve zamanlayıcıların takvimi (S-15, S-16)

**K.**

- Hop-ACK'siz ardışık PN tavanı yok (`runtime.rs` 3386–3431 `seal_send!`; `sender.rs` emisyon
  yolları).
- `Timer` yalnız Pto/RtpropProbe. IDLE_TIMER, PATH_DEAD_TIMER, RETRY_BUDGET ve
  COPY_MAX_INTERVAL yok (1052–1059, 1830–1840).
- `dup_queue` tavansız; `dup_spacing = 20 ms`.

**O.**

- transport-layer-tdd §12 [AMENDMENT-9] KS-PN-1: «hop-ACK almadan 1024 PN tüketilince yeni
  Session-ctx emisyonu DURUR», «SPEC-ONLY … AYRI bir iş kalemidir».
- engine-spec §12 zamanlayıcılar.
- §16.1 `COPY_MAX_INTERVAL = deadline/8`.

**S.** Ne zaman ve hangi profil kimliğiyle (`transport-v1.x`) yapılacak? Bu bir takvim kararı.
Davranış spec'te tanımlı.

**T.** Tam hızda PN tüketimi `P_cap/M ≈ 10,2 datagram/s`. `K = 1024` dolma süresi 100 s (12 500)
ve 201 s (6 250). Hop-ACK aralığı geri bildirim ufkuysa (≤ 10,68 s) aralık başına ≤ 109 PN
tüketilir. Tavan yalnız ≥ 100 s'lik kesintide bağlar ve BENCH-01 blackout metriğini doğrudan
etkiler.

---

## Karar 9 — Tel kod noktası tahsisi (S-24, S-51, S-58) — politika

Bu karar matematiksel değil. Tahsis gereken kod noktaları:

- bitmap ACK 0x15–0x17,
- CAPS 0x08,
- yalın PTT 0x09,
- v0 interim aile (0x00–0x09, 0x10, 0x14–0x17; noise-wire §2.56 kayıt defteriyle anlamları
  çakışıyor),
- 0x1D,
- S4 Y1–Y12,
- G-S4 kapısının durumu,
- şerit etiketli `message_id`.

Seçenekler: kayıt defterine ekle · v0 aileyi S2 değişimine kadar "interim" diye beyan et.

## Karar 10 — ADR sapmaları (S-47, S-48, S-52, S-55, S-56) — politika

| ADR | durum |
|---|---|
| ADR-03 | yalnız dalek; libsodium yok |
| ADR-05 | `ccr-codec` boş (3 satır); kanonik kodek dağınık |
| ADR-16 | `reed-solomon-simd` |
| ADR-02/13/19 (hub-server) | tokio/redb Storage trait/sağlık uç noktası |
| CLAUDE.md §2 | crate düzeni ≠ 14 crate'lik gerçek ağaç; `crypto` crate'i yok |

Her biri için: ADR'ye uy · ADR'yi tarihli kararla değiştir.

## Karar 11 — Güvenlik açıklarının takvimi (S-42..S-45, S-57) — politika

- HubChannel hop anahtarı M4 yok.
- Hub çerçevelerinde açık metin `dest_hash`.
- Kalıcı INIT tekrar koruması yok (noise-wire §0.7).
- `s3::Opened` mühürü.
- Provisioning'de sertifika imzası doğrulanmıyor.

Davranış spec'te tanımlı. Karar sıradır.

---

## Karar 12 — Dosya penceresi tabanı ve dar uplink'te FEC (S-36, S-33)

### 12a — Pencere tabanı (S-36)

**K.**

- Şerit katmanı: `W_lane = U · budgeted_ack_interval = 12 500 · 5,40 = 67 500 B` (`runtime.rs`
  541–556; `lanes.rs` 282–297).
- Motor tabanı: `W_eng = P_cap · (H + rtprop) ≈ 12 215 · (10,68 + 0,6) ≈ 138 kB`
  (`sender.rs` 2775–2780).
- Kayıp birim teyide dek şerit penceresinde yer tutar.

**O.** 2026-11-16 istisnası «tavan en az hat × (geri bildirim ufku + RTT)». İstisnanın şerit
katmanını kapsayıp kapsamadığı yazılı değil.

**Seçenekler.** A0: iki ayrı taban · A1: şerit tabanı motorunkiyle tek kaynaktan
(`U·(H + rtprop)`).

**T12.1 — Pencere sınırlı hız.**

- ACK `k` alıcıdan `r_k` anında çıkar, `a_k = r_k + s_r + d` anında gelir. `r_k − d − q` öncesi
  gönderilenleri teyit eder.
- `a_{k+1}`'den hemen önce teyitsiz bayt: `U · (T + s_r + 2d + q)`.
- Tam hız için `W ≥ U·(T + s_r + RTT + q)`. Bu, motor tabanının formülüdür
  (`H = T_max + s_r`).
- Şerit penceresiyle hız `≤ W_lane / (T + s_r + RTT + q)`, kayıpsız ve `q = 0` iken:

  | `T` | `s_r` | `RTT` | hız | hattın oranı |
  |---|---|---|---|---|
  | 8,5 s (ölçülen) | 1,3 s | 0,6 s | 6,5 kB/s | ≈ %52 |
  | 5,4 s (bütçe) | 1,3 s | 0,6 s | 9,2 kB/s | ≈ %74 |

- `p = 0,2`'de kayıp birimlerin işgali bunu daha da düşürür.
- Önerme: `T + s_r + RTT > budgeted_ack_interval` iken A0'da dosya hızı hattın altında kalır.
  Aç profilde bu eşitsizlik her zaman doğrudur (`s_r + RTT > 0`).

### 12b — Dar uplink'te FEC (S-33)

**K.** `runtime.rs` 3223–3233, 780–847.

- Parite sembolünün uplink'te serileşmesi > `PARITY_SER_LIMIT_NS = 2 s` (843) ise FEC hiç
  kurulmaz: `fec = None`, PARAM çıkmaz, sayaç yok.
- Bu durumda 500‰ ACK'siz FEC kuralı (`fec_feedback_starved_permille`, 495–499) da devre dışı
  kalır.

**O.** 2026-11-16 kararı: «PTT dışı şeritlere profilin en yüksek FEC oranı». fec-core §11
«sub-kbps ≠ E=1024» yalnız yön verir, eşik vermez.

**T.** Sembol serileşmesi `M_w/U`. `U = 62 B/s` iken 19,8 s → FEC kapalı. Eşik
`U < M_w / 2 s = 614 B/s`.

500‰ kuralı uygulanırsa goodput şu oranda düşer: `1/(1 + 0,5) = 2/3`. Kurtarma kazancı
`P(blok kurtarılır | p)`. `k = 8`, `r = 4`, `p = 0,2` için:

- FEC'li: `P(Bin(12, 0,8) ≥ 8) = 0,927`.
- FEC'siz, 8 birimin tümü ilk denemede: `0,8⁸ = 0,168`.

**Seçenekler.**

- A0: dar uplink'te FEC yok (sessiz) + sayaç.
- A1: 500‰ kuralı eşikten önce gelir.
- A2: eşik `srtt` ya da ACK ufkundan türetilir.

---

## Karar 13 — Ürün metrikleri ve benchmark (S-61, S-65..S-67) — politika

- §4.4 mekanizma metrikleri.
- BENCH-01 ikili blackout survival (bugün yalnız `p6_blackout` tamamlanma vekili).
- kama-node RTT/kayıp/bitrate alanlarının tanımı.
- `LinkChanged` olayı.
- Medya API'si (seam).

Tanım kararı: hangi ölçünün hangi ürün iddiasını taşıyacağı.

---

## Özet: matematiksel modele uygun olanlar

| karar | ispat nesnesi | ana girdi |
|---|---|---|
| 1 | pace/flight kuralının sabit noktası ve yakınsaması (`p = 0,2`, seyrek ACK) | §0.1–0.2, T1.1–T1.3 |
| 2 | taşma tetiğinin yanlış pozitif olasılığı; kuyruk derinliği koşulu | T2.1–T2.2 |
| 3 | toparlanma süresi alt sınırı; çift kanıtının varlık koşulu; başarısız yoklama kuyruk bedeli | T3.1–T3.3 |
| 4 | canlılık önermesi (onarım çözülür) | durum makinesi |
| 5 | kalem başına: örnek atlama, PTO, tavan, eşik — her biri tek formül | tablo |
| 6 | pacer dışı bayt üst sınırı | T6 |
| 8 | PN tavanının bağladığı kesinti süresi | T8 |
| 12 | pencere sınırlı hız; FEC eşiği ve 500‰ kuralının bant/kurtarma dengesi | T12.1, 12b |

Karar 9, 10, 11 ve 13 politika ve takvim kararıdır; ispat nesnesi yok.
