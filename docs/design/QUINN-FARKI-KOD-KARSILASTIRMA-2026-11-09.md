# Kama ↔ quinn mekanizma karşılaştırması — kod ve ölçüm (2026-11-09)

> **NORMATİF DEĞİL — analiz girdisi.** Uygulama talimatı yalnız `SAHIP-KARARI-v4-EK-A` §9–§10'dur; çelişkide EK-A §10 geçerlidir. (Düzeltme kaydı 2026-11-10: §2a adlandırma, A-6 kapısı, netem kolu, (d) ifadesi §10'a göre düzeltildi.)

Kaynaklar: `kama-core` main `dbea1ef3` (`crates/adaptive-transport/src/*`, motor `a858f3fc` ile aynı) · `quinn-proto 0.11.18` (Cargo.lock sürümü; `quinn-proto/src/*`) · ölçümler: F0 (`reports/ktp2/f0/`), ACIL-INCELEME 2026-11-03, G2 teşhisi 2026-11-09 (OKUMA.md). Her hüküm dosya:satır gösterir. Etiketler: [kod] kaynakta açık · [ölçüldü] koşulmuş veri · [tahmin] henüz ölçülmedi.

---

## §0 Hüküm

**Kama'nın ARQ/ACK/pacing mekanizmaları quinn'den farklı değil — aynı RFC 9002 iskeleti. Fark, sabitlerde ve kapılarda:** Kama'da beş yerde "en kötü köşe" (0,025 kbps, 31 B/s dönüş hattı) için konmuş sabit ya da kapı **her rejimde** çalışıyor ve 12,5 kB/s · 300 ms hattında saniyeler kaybettiriyor; quinn'in karşılığı RTT ya da pencereyle ölçekleniyor. Beş kök neden, ölçülmüş etkisiyle:

| # | Kök neden | Kama [kod] | quinn [kod] | Ölçülen etki |
|---|---|---|---|---|
| 1 | Kayıp ilanı zaman eşiğinin tabanı 31 B/s önseline bağlı | `sender.rs:3402-3412` EAI = 2·birim/rate + **65 B / 31 B/s (= 2,1 s)**; taban = 1,5·EAI (`:3396-3398`, `loss.rs:685`) → **≥ 3,15 s, RTT'den bağımsız** | `connection/mod.rs:1700` loss_delay = max(9/8·max(srtt, latest), 1 ms) → 300 ms'de 0,34 s | ilan→retx p50 4,08 s; kayıp kurtarma 3,7 RTT (ACIL) |
| 2 | Alıcı boşlukta anında ACK üretmiyor; zamanlayıcı SACK'i 8,4 s'de bir | `receiver.rs:255-258` boşlukta `tick_ack(now,false)` (zorlama yok); `:93-94` ACK_CADENCE_FLOOR = 65 B/(0,25·31 B/s) = **8,39 s** | `spaces.rs:720-725` sırasız paket → anında ACK; max_ack_delay 25 ms zamanlayıcı | kuyruk sonunda kayıp sinyali gecikir; 1 ile birleşince kurtarma bekleyişi |
| 3 | Yeniden gönderim DATA'nın arkasında | `sender.rs:4283-4297` DATA önce, retx yalnız her 4. grant'te (`K_ARQ_SHARE=4`, `:1535`); parite en sonda (`:4311`) | `send_buffer.rs:93-113` retransmit aralığı **her zaman önce**, aynı pacing bütçesinde | spec §13 (retx > parite > DATA) ile çelişki; ama **K0r ölçümü retx-first'ü bu hücrede kötü gösterdi** — §2 |
| 4 | Bant probu kayıpla kesiliyor | `sender.rs:3993-3996` bekleyen kayıp → RecoverArq (ProbeUp'ı keser); `:4076` atıflı kayıpta abort; `:4072-4075` başarı = BtlBw > giriş×1,025 | `bbr/mod.rs:160-199` 1,25 fazı **kayıpla iptal olmaz**; kayıp yalnız "hedefe kadar tut" beklemesini kaldırır; başarı testi yok — max-filtre kendisi alır | manşette `probe_up` 0 (K0, K2); yukarı kuvvet yok → (1−p) sarmalı; gönderim/gereken 0,55 |
| 5 | Kova tavanı birim boyutunun altına inebiliyor (parite/retx için; dosya debisini açıklamıyor — §2a) | `sender.rs:2366-2381` cap = clamp(rate·40 ms, front, max(front, 2400)); `pacer.rs:118-119` `size > burst_cap` → **asla gönderilemez** | `pacing.rs:132-139` cap = clamp(window·2 ms/rtt, **10·MTU**, 256·MTU) | 12,5 kB/s'de cap ≈ 500–1062 B; 1058 B parite ve ön sıradaki birim küçükken dosya/retx birimi kovaya sığmaz (ACIL §0 "26,45 kB/s altında parite çıkmıyor") |

Üçü (1, 2, 5) alıcı/pacer tarafında, teli değiştirmiyor, her biri ≤ 20 satır. 4'ün dar hâli 1 satır. 3 ölçümle çelişiyor, önce anlaşılmalı.

**Altı aylık araştırma neden sonuç vermedi:** mekanizma doğru seçilmiş (BBR + RFC 9002), ama sabitler "0,025 kbps'te de çalışsın" diye önsellerle bağlanmış (31 B/s dönüş hattı, 2 datagram/ACK, 1 birimlik kova) ve bu önseller ölçülen hıza göre **gevşemiyor**. Takımın kendi kuralı (ZM-2: "decade-bağımlı sabit yasak") tam olarak bu üç sabitte ihlal edilmiş. quinn'in aynı yerlerde RTT/pencere ölçekli formülleri var; bunlar alınır, 0,025 kbps köşesi için **rejim kapısı** (ölçülen rate < 1 kB/s iken önsel devrede) eklenir.

---

## §1 Mekanizma tablosu

| Mekanizma | quinn 0.11.18 | Kama main | Fark | Hüküm |
|---|---|---|---|---|
| RTT örnekleme | `paths.rs:335-358` RFC 6298; ack_delay yalnız `min+ack_delay ≤ latest` ise düşülür; min_rtt ack_delay'siz | `rtt.rs:142-175` aynı α/β; `rtt.rs:239-248` ack_delay min-rtt koruması aynı semantik; ek: kuantum/yakınsama penceresi (`:125-141`) | Eşdeğer; Kama'da ek pencere mantığı | Koru. Bayat SACK ile srtt şişmesi (K4G-05 bulgusu) ayrı hata, Karn kuralı eksikliği değil: yeniden raporlanan eski kümülatif SACK örnek üretmemeli |
| Kayıp: paket eşiği | `mod.rs:1727` largest_acked ≥ pn+3 | `loss.rs:26-31, :650-657` K=3, adaptif ρ | Eşdeğer | Koru |
| Kayıp: zaman eşiği | `mod.rs:1700` 9/8·RTT, taban 1 ms | `loss.rs:667-703` 9/8·srtt ama taban = cadence_floor ≥ 3,15 s (§0-1) | **Taban** | **Al:** taban = max(1 ms, cadence_ölçülen); 31 B/s önseli yalnız ölçülen rate < 1 kB/s iken (E2a m1 bunu yapıyor mu, kontrol) |
| Zaman eşiği "later-ACK" ön koşulu | `mod.rs:1721` yalnız `< largest_acked` paketler taranır | `loss.rs:692` `seq < la` aynı | Eşdeğer | Koru |
| PTO | `paths.rs:331-333` srtt + max(4·var, 1 ms) + max_ack_delay; `mod.rs:1834-1870` 2^count backoff, 2 prob (`:1683-1689`), çapa = son ack-eliciting gönderim | `tail_recovery.rs:155-174` aynı formül; tek prob; çapa = son ACK ilerlemesi; tavan 3 prob, 30 s; başlangıç 1 s; üründe AÇIK (`client-core/src/lib.rs:91`) | Prob sayısı, çapa, tavan | **Al:** 2 prob; çapa son gönderim; 3 prob tavanı kaldır (RFC'de tavan yok, backoff var). Küçük |
| ACK sıklığı | `spaces.rs:718-721` her 2 ack-eliciting; `:723-725` **sırasız → anında**; `:674-676` max_ack_delay 25 ms | `receiver.rs:76` ACK_FREQ_N=2 aynı; boşlukta zorlama **yok**; zamanlayıcı SACK 8,39 s cadence (`:93`), kuyruk muafiyeti K_TAIL=2 (`:260-271`) | Anında ACK + zamanlayıcı | **Al:** `has_gap()` yeni boşluk → `tick_ack(now,true)`; cadence tabanı ölçülen dönüş hızından, bilinmiyorsa 25 ms + srtt/4; 8,39 s yalnız rate < 1 kB/s |
| Yeniden gönderim sırası | `send_buffer.rs:97` retx önce | `sender.rs:4283-4297` DATA önce, retx 1/4 | Sıra | **Ölç, sonra al:** §2'deki K0r çelişkisi çözülmeden değiştirme |
| Yeniden gönderim tavanı | yok (PTO backoff sınırsız, persistent congestion ayrı) | `arq.rs:70-78` cap 3 → HandToForward; "KALİBRE DEĞİL fixture" | Tavan | **Al:** tavan yok; takılma INT-02'de ölçülmüştü (K4G-05) |
| Teslim hızı örneği | `bbr/bw_estimation.rs` delivered/interval, app-limited işaretli | `bw.rs:98-116` aynı; `bw.rs:233` app-limited örnek yalnız ≥ filtre ise girer | Eşdeğer (BBR) | Koru |
| Startup çıkışı | `bbr/mod.rs:373-391` app-limited turda **sayılmaz**; 3 tur < %25 büyüme; recovery'de çık | `sender.rs:4208-4235` 3 tur Δ<%5; app-limited turlar **sayılıyordu** → (d) bayrağı quinn davranışını getiriyor | (d) = quinn | (d) gösterim setinde (K1 ⊃ (d)); kalıcılık G-CC'de kabul koşullarıyla. Recovery'de çıkış INV-RATE-1 gereği alınmaz (gerekçeli) |
| Startup pacing tabanı | Fiili pace pencereden: `pacing.rs:92-95` token = cwnd·1,25·geçen_RTT, cwnd Startup'ta = ilk pencere (`bbr/mod.rs:84,95`) → ilk pace ≈ 1,25·IW/srtt; BBR'nin `calculate_pacing_rate` (`:284-307`) yalnız metrik (B5, #151) | PreConverge 31 B/s (`:1485`), sonra ilk örnekten 2,885×; kalkış tabanı min_cwnd 4·MTU/srtt (`rate.rs:98-102`) | Taban ve monotonluk | **Al:** IW = 10·MTU (RFC 9002; quinn-Cubic `cubic.rs:266`), Startup'ta pace düşmez. 240 kB quinn-BBR IW'si **alınmaz** |
| Bant probu | `bbr/mod.rs:160-199` 8 fazlı gain döngüsü; 1,25 fazı ≥ 1 min_rtt ve hedef inflight'a kadar (kayıp varsa beklemeden geçer); abort/başarı yok | `sender.rs:4060-4117` giriş: periyot ∧ qd ∧ backlog; çıkış: başarı 1,025×, abort (qd>500‰ ∨ atıflı kayıp), zaman aşımı 4 epoch | Kayıp vetosu + başarı testi | **Al (dar, şimdi):** atıflı-kayıp abort'u kaldır (`:4076`); (e-dar) zaten kesmeyi kaldırdı. **Al (CC-2):** başarı testini kaldır, max-filtre karar versin; gain döngüsü quinn gibi |
| Kayıp → hız | `bbr/mod.rs:128-156, 339-367` recovery_window (conservation/growth) — kayıpta pencere daralır; Startup'ta uygulanmaz | INV-RATE-1: kayıp hızı düşürmez; kayıp yalnız FEC/ARQ'yu besler | Bilinçli fark | **Koru ama tamamla:** INV-RATE-1 rastgele kayıp için doğru; sığ tampon koruması CC-v2'nin üç sinyaliyle (qd, kayıp–kuyruk ilişkisi, prob testi) gelir. quinn-BBR'ın kayıp körlüğü F0'da "denetimsiz" yaptı, bu alınmaz |
| (1−p) telafisi | yok — BBRv1 1,25 döngüsü p ≤ %20'ye kadar BtlBw'yi korur, üstünde çöker | yok; ProbeUp vetosu yüzünden %20'ye bile gelmiyor | Kama daha kötü | Dar düzeltmeyle BBRv1 paritesi (%20); %30 için CC-v2 p̂ telafisi (CC-K3) |
| Pacer | `pacing.rs:92-95` token = pencere·1,25·geçen_RTT; cap `:132-139` clamp(window·2 ms/rtt, 10·MTU, 256·MTU) | `pacer.rs` rate·dt token (alt-bayt kalanlı, doğru); cap `sender.rs:2366-2381` clamp(rate·40 ms, front, max(front,2400)) | Cap tabanı | **Al:** cap tabanı 2·MTU (PreConverge hariç); `size > cap → None` yolu böylece ürün hızlarında ölmez |
| cwnd tavanı | pencere = bayt (inflight ≤ cwnd), `window()` | rate formu min(gain·BtlBw, cwnd/srtt) `rate.rs:143-150` | Eşdeğer (Little) | Koru |
| min_cwnd | `bbr/mod.rs:633-635` 4·MTU | `rate.rs:100` 4·MTU | Aynı | Koru |
| Persistent congestion | `mod.rs:1710-1753` 3·PTO | yok | — | Şimdi alma; CC-2'de değerlendir |
| İlk pencere | BBR 240 kB (`:534`), Cubic 10·MTU (`cubic.rs:266`) | 4·MTU/srtt kalkış + PreConverge | — | 10·MTU (yukarıda) |

Eşdeğer satırlar çoğunlukta: **yeniden icat edilmiş bir ARQ yok**; kopyalanacak şey beş sabit/kapı ve bir sıra kuralı.

---

## §2a Dosya neden 19 s sürüyor — ham veriden (#151, K1 10 koşu) [ölçüldü]

Göndericinin `txb` sayacı ilk 15 kB için medyan **5,2 s** (K0 6,4 s; K1a 5,9 s; sayaç yeniden gönderimi de içerir, alıcı ilerlemesi aynada yok); 12,5 kB/s hatta 1,2 s olmalıydı. Başlangıç gönderimi yavaş (ilk 3 s'de ≈ 2 kB, ilk büyük gönderim ≈ 1,3 s). Startup, bant tahminini ısınma TEXT'lerinden alıyor (bw ≈ 325 B/s, uygulamayla sınırlı) ve pace'i 2,885 × 325 ≈ **937 B/s**'den başlatıyor. 5–6 s'de 25 kB/s'ye taşıyor (Startup kazancı), netem'in 1000 paketlik kuyruğunu dolduruyor (srtt 320 → 900–1 360 ms), RecoverArq'ta cwnd/şişmiş-srtt tavanıyla 8,5–9 kB/s boşaltıyor; 13 s'den sonra ProbeUp 15 kB/s ile hattı dolduruyor. Toplam kayıp: ≈ 5 s rampa + ≈ 5 s boşaltma. Fazlar içinde tel ≥ pacer hedefi; "tel hedefin altında" medyan yanılsaması. **Dünkü kova-kuantizasyon tahmini bu veriyle desteklenmiyor, geri çekildi** (A-3 yine parite/retx için gerekli, dosya debisi için değil).

Düzeltme **A-6**: Startup pace tabanı = IW (10·MTU) / srtt; Startup'ta pace düşmez; ısınmadan öğrenilen uygulamayla-sınırlı bw Startup'ı tohumlamaz. quinn'de aynı etki pencereyle geliyor (`pacing.rs:92`, cwnd = IW 14 520 B → ≈ 48 kB/s ilk pace; Q-bbr-rfc dosya 14,8 s). Kapı (EK-A §10.2): taban yalnız bant tahmini henüz uygulamayla-sınırlı-olmayan bir tam tur görmemişken; "pace düşmez" yalnız bw terimine, cwnd/uçuş tavanı aynen bağlar; etkinleşme/kalkma olayı aynaya yazılır. Beklenen: dosya 19 → ~15 s [tahmin]. Küçük netem kuyruğu **ayrı kol** olarak ölçülür (`q_limit`); mevcut 1000 paket profili ve gösterim profili sonuç alınmadan değişmez (EK-A §10.3). Derin kuyruk Startup taşmasını Kama'da (Drain fazı yok) quinn'den fazla cezalandırıyor [tahmin].

## §2 Ölçümler neden beklentiyle çelişti (G2 teşhisi)

- **(c) PTT'yi iyileştirdi, model tersini bekliyordu.** (c) = Startup'ta cwnd/srtt tavanını min_cwnd'nin üstüne çıkarmak. Model, PTT'nin dosya arkasında beklediğini ve (c)'nin dosyayı hızlandırıp PTT'yi geriye iteceğini varsaydı. Ölçümde PTT p95 8,63 s (K1) ↔ 10,22 s (K2−c): dosya **daha erken bittiği** için PTT daha erken sıraya girdi; şeritsiz ağaçta "dosya bitiş süresi" PTT gecikmesinin baskın terimi. Şeritlerle (T1) bu bağ kopar; o zaman (c)'nin PTT'ye etkisi yeniden ölçülmeli [tahmin].
- **retx_first zararlı çıktı (K0r 16,37 s; 3/10 eksik).** quinn retx-first çalıştırıyor ve bu problemi yaşamıyor. Fark [tahmin]: Kama'da kayıp ilanı ≥ 3,15 s geç (§0-1) ve tavan 3 (`arq.rs:73`) → retx-first açılınca bütçe, çoğu **artık geçerliliğini yitirmiş ya da sahte** (geç ilan edilmiş, bu arada ACK'i gelmek üzere) kayıpların yeniden gönderimine gidiyor; yeni veri geri kalıyor; 3 eksik koşu tavan+HandToForward yolundan. Yani 3. satır tek başına alınamaz; **1 ve 2 düzeltilmeden retx-first ölçülemez**. Kontrol: K0r koşularında `spurious_units_total` / `lost_units` oranı ve `HandToForward` sayısı (loss.rs:559, arq.rs:78).
- **Manşette probe_up = 0 (K0 ve K2)** [ölçüldü, R4]: §0-4. (e-dar) kesmeyi kaldırdı ama giriş hâlâ `backlog ∧ period ∧ qd`; period = 4 epoch, epoch = max(srtt, EAI) ≥ 2,4 s → prob en erken ~10 s'de bir; 60 s koşuda 0–1 prob normal. quinn'de döngü her min_rtt'de ilerler (300 ms). **Al:** periyot = K·srtt, EAI'siz.
- **emisyon/sunulan 4,90 (K1/K2)** [ölçüldü]: payda dosyasız hücrenin 430 B/s sunulanı; pay alice `pr` medyanı. 4,9× = yeni veri + yeniden gönderim + prob; oran bu kadar yüksekse ya yeniden gönderim payı büyük (1 ve 2'nin sonucu) ya da payda yanlış katmanda. Önce ayrıştırılmalı; "hız kilidi açıldı" cümlesi faydalı-teslim/gönderim oranı yazılmadan kullanılmaz.
- **r25'te (d) devreye girmiyor** [ölçüldü]: r25'te oturum hiç app-limited değil (uygulama hattan hızlı) → (d) tanımı gereği pasif. Beklenen; kusur değil.

---

## §3 En hızlı yol (sıralı; ölçülmüş etkiye ve maliyete göre)

**A — şimdi, gösterim ağacına, bayrak arkasında (toplam ≤ 1 ajan-günü; tel değişmez; #151'de A-1…A-5 `yol3/q1-quinn-ack` 507417b7 ve `yol3/q2-quinn-pacer-prob` c8450809'da kodlanmış, ölçülmemiş):**
1. Boşlukta anında ACK: `receiver.rs` `on_datagram`'da `has_gap()` yeni boşluk ürettiyse `tick_ack(now, true)`. ~5 satır. [quinn `spaces.rs:723`]
2. Zamanlayıcı SACK cadence ve kayıp-eşiği tabanındaki 31 B/s önseli **rejim kapılı**: pace ≥ 1 kB/s iken EAI'nin dönüş terimi = SACK_WIRE/pace (simetri varsayımı), cadence = max(25 ms, bu); altında eski önsel. ~15 satır, iki dosya. E2a m1 bunun bir kısmını yapıyorsa birleştirilir.
3. Kova tavanı: `burst_lo = front.max(2*DEFAULT_MTU)` (PreConverge hariç). 2 satır. Parite ve büyük birim artık "asla gönderilemez"e düşmez.
4. ProbeUp atıflı-kayıp abort'u kaldır (`sender.rs:4076` → yalnız qd). 1 satır. Prob periyodu `K·srtt` (EAI'siz). 2 satır.
5. PTO: 2 prob, çapa son ack-eliciting gönderim, 3-prob tavanı kaldır. ~10 satır `tail_recovery.rs`.
6. **A-6** Startup pace tabanı IW/srtt (§2a), bayrak `a6_iw_floor`, kapısı EK-A §10.2. ~10 satır (`grounded_startup_rate` / startup floor, `sender.rs:2351-2361`).
Hepsi K1'in üstüne ayrı bayrakla; 7 kollu ölçümde K1 ↔ K1+A ↔ K1+A−6 olarak ölçülür. Kapı ölçütü değişmez.

**B — dondurmadan sonra (CC-2 kapsamı; `sender.rs` ACK/prob yoluna dokunur):**
6. ProbeUp başarı testini kaldır; quinn gain döngüsü (1,25 → 0,75 → 1×6; faz ≥ min_rtt; kayıpta bekleme yok). Max-filtre tek karar verici.
7. IW = 10·MTU kalıcı; Startup'ta pace monoton. (quinn'de bu pencere üzerinden gelir: `pacing.rs:92`, cwnd = IW.)
8. `arq_retx_cap` kaldır; HandToForward yalnız açık politika kararıyla.
9. retx-first (spec §13) — **1–2 ve 8'den sonra** ölçülerek.
10. (1−p) telafisi (CC-v2, CC-K3 koşullarıyla): %20 üstü rejim.

**C — quinn'den alınmayacaklar (gerekçeli):** 240 kB BBR ilk penceresi (sığ tamponda 18–22×C çökme, F0'da "denetimsiz"); kayıp körlüğü (sığ tampon koruması CC-v2 sinyalleriyle); kayıpta recovery_window daralması (INV-RATE-1 — rastgele kayıp rejiminde ürün kanunu; tıkanıklık rejimi CC-v2'de ayrı sinyalle); rastgele gain faz ofseti (ADR-17 determinizm); f32 aritmetiği (sabit nokta kalır); `min_max.rs` kökeni için atıf.

---

## §4 Açık ölçümler (karar öncesi)

- K0r koşularında sahte-kayıp oranı ve HandToForward sayısı (§2) — A-1/A-2 sonrası retx-first yeniden.
- emisyon/sunulan ayrıştırması: yeni / retx / prob / parite bayt payları.
- A paketiyle manşet (`a_r12k_d300_l30_dosyali`): gönderim/gereken ve **faydalı teslim/gönderim**; probe_up girişi ve kapanışı sayıları.
- Dönüş hattı gerçek hızı: 65 B SACK'lerin ölçülen aralığı → 31 B/s önselinin hangi hücrelerde gerçekten bağlayıcı olduğu.

---

## §5 Tek cümle

Kama'yı quinn'e çevirmek gerekmiyor; Kama'nın beş yerde 0,025 kbps köşesine kilitlenmiş sabitini quinn'in RTT-ölçekli formülleriyle değiştirip kayıpla kesilen probu serbest bırakmak gerekiyor. Üçü bugün alıcı/pacer tarafında yapılabilir; geri kalanı motor dondurulduktan sonra, ölçerek.
