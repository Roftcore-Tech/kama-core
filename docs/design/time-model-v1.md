# time-model-v1 (r2-ONAYLI) — Motorun Zaman-Modelinin Sub-kbps'e Taşınması (TASARIM NOTU, D0-tarzı)

> **Statü: r2 ONAYLANDI (Abkarada, 2026-07-12) → TAM-GO.** Dört karar: **(1)** p5 = seçenek (a)
> belgeli-regresyon, main'e merge YOK, taşıyıcı TUR-4; (b) RED (tek-değişken kırar, 4-kök kompozisyonu
> maskeler) — ŞART: §3.5 "bilinen-açık kabul-kapısı" izlemesi. **(2)** not r2 tam-GO, TUR-1x'ten devam.
> **(3)** §3 [CAL] başlangıçları ONAYLI (placeholder, golden'a girmez) + tombstone MAX=4096 4-decade
> bellek-doğrulaması eklendi (D-1/4). **(4)** TUR-1x aynı PR, sıra 1a→1x→1b, **ÜÇ ayrı ölçüm-noktası**;
> 1x-sonrası ara-ölçüm (srtt ~8.3s→gerçek-path-RTT) KAYDEDİLMEDEN 1b başlamaz. Her commit:
> beklenti-kaydı-önce + tek-değişken + p4/p5-smoke (p5 "bilinen-düşük" istisnasıyla).
> Otorite: transport-tdd (WHAT) > engine-spec (HOW; §16.2 ZM-1..4) > bu not > kod.

## 0. Kök teşhis (r1'den, GO): 22 BREAKS ≠ 22 bug — İKİ paylaşılan yanlış-model

**(a) Estimator saat-modeli:** pencere/dwell/round'lar kanal-zamanı yerine wall-clock/event-count
ölçüyor (W_BTLBW/W_RTPROP=10s; DrainRtprop dwell=0 event [verify: AYNI çağrıda çıkıyor]; emergency
ladder=çağrı-sayısı; deescalate_dwell=1s<srtt; dup_spacing=20ms≪serialization; ProbeUp UNREACHABLE).

**(b) Loss-modeli:** loss OLAY (monoton, geri-dönüşsüz, anında-infaz) modellenmiş; sub-kbps fiziğinde
HİPOTEZ (SACK-cadence gecikmesinden ayırt edilemez; geç kanıtla ÇÜRÜMELİ). K-1 kompozisyonu (ölçüldü):
false-loss → eviction → ACK-sansürü → estimator açlığı (btlbw 88→4) → fuzzy zehirlenmesi → arq=0 →
pending drain edilmez → RecoverArq mıhlı.

## 0.5 Temel-kanıt tablosu (r1 + re-verify işlenmiş; tamamı ÖLÇÜLDÜ/VERIFIED)

| bulgu | kanıt | statü |
|---|---|---|
| Temiz kanalda %100 time-threshold false-loss (27/27; packet=0) | diag_loss atıflama | ÖLÇÜLDÜ |
| btlbw 88→4 çöküşü (gerçek ~90-94 B/s iken); nedeni eviction-sansürü | TRAJ; TUR-1a'da kayboldu | ÖLÇÜLDÜ (2 yönlü) |
| arq=0 → pending drain edilmez (retx=0, pend monoton) | TRAJ + tx-dekompozisyon | ÖLÇÜLDÜ |
| W_BTLBW/W_RTPROP=10s wall-clock ("~10 RTT" değil) | bw.rs:12/rtt.rs:16 | VERIFIED |
| ack_delay u16µs=65.5ms tavan → RTT ~8.3s şişer | receiver.rs:76 + rtt akışı | VERIFIED (2 grup) |
| DrainRtprop N=0 — AYNI on_event'te çıkar, ProbeDown gain hiç uygulanmaz | sender update_state `_` | VERIFIED (CORRECTED-worse) |
| ProbeUp UNREACHABLE — Steady kalıcı app-limited kilit | compute_send_rate match | VERIFIED (yeni kaçak) |
| ack_confidence anchor `.or()` ≠ spec `max()`; idle-resume bayat → anında RecoverFwd | sender.rs:756 | VERIFIED (CONFIRMED) |
| fuzzy mutlak üyelikler → kural-ağırlığı-0 → arq=0 | fusion.rs + ölçüm | VERIFIED + ÖLÇÜLDÜ |
| UN-LOSS yolu yok; loss_ppm monoton ratchet | loss.rs | VERIFIED |
| **p5 100/100'ün false-loss-fırtınası PARAZİT recovery'si olduğu** | TUR-1a smoke: p5 24/100, dup 93673→0 | **ÖLÇÜLDÜ (yeni, §0.7)** |

## 0.7 TUR-1a ölçüm kaydı (commit 546927c; talimat-GO ile koşuldu)

**Beklenti-kaydı (önceden):** loss_time 27→0; RecoverArq kaybolur; teslim 29→↑; iki dal: (a) Startup
ramp tamamlar → btlbw ~110-125, (b) premature-plateau kilitler → btlbw ~88-100, tam ramp TUR-3c'ye.
**Gerçekleşen:** `loss_time 27→2` (t<108s SIFIR; kalan 2 = kanıtlı-ama-cadence → tam TUR-1b hedefi),
teslim **29→43/100**, dup=retx=0 (1.00×), 88→4 çöküşü KAYBOLDU (88→102 stabil), **dal (b)**: premature-
plateau t≈27'de Steady kilitledi, btlbw ~102 app-limited (kayıtlı bağımlılık — başarısızlık DEĞİL).
p4 100/100 ✓ (oran 1.08→1.01 iyileşme — precondition jitter false-loss'unu da temizledi). p2 1→3/100 ✓.
**p5 100→24/100 REGRESYON (tahmin ✗, dekompoze):** eski 100/100, false-loss fırtınasının parazit
recovery'siydi (pending'i fırtına dolduruyor → RecoverFwd 93673B dup). Precondition parazit motoru
söktü; meşru yollar taşımıyor: (i) kanıtlı sınıflandırma çalışıyor AMA fuzzy arq=0 (D-3 kökü) →
pending drain edilmiyor; (ii) p5'te SACK'lerin %20'si geçiyor → conf yüksek → RecoverFwd'e girilmiyor.
**Sonuç: p5, D-3 fallback'ine (TUR-4) + TUR-B'ye bağımlı — D-3 "temizlik" değil, p5'in TAŞIYICISI.**
→ Abkarada karar noktası §3/5.

## 0.8 TUR-1x ölçüm kaydı (ZORUNLU ara-ölçüm; talimat #4 — 1b bu kayıt olmadan başlamaz)

**Beklenti-kaydı ölçümden ÖNCE yazıldı** (scratchpad tur1x-prediction.md; baseline-sonrası zeyl dahil).

**Baseline (546927c, temiz 1kbps):** srtt "8.3s plato" TAHMİNİ YANLIŞTI → gerçek: **3.0→30.8s monoton
büyüme, KUYRUK-domine** (infl/rate ≈ 48-62s sender-side netem kuyruk = gerçek kanal-zamanı). Hold
bileşeni yalnız timer-SACK örneklerinde (kanıt: t≈90 tek kümülatif SACK srtt'yi 14→20.8s sıçrattı;
freq-SACK'te hold≈0). Kullanıcının "~8.3s→path-RTT" hedefi bu dekompozisyonla okundu: hedef = hold'un
örneklerden çıkması.

**ÖLÇÜM-ARACI ARTEFAKTI (yeni, measurement-tool-first 4. yakalama):** after-kollarında tekrarlanan
loss_pkt=4-8 sapması seq-düzeyi yer-gerçeğiyle köklendi: seq 37-38 wire'da GERÇEKTEN kayıp (client
SND, server RX yok) ama netem dropped=0, Udp Snd/RcvbufErrors=0 → düşüş **kernel ARP/neighbor
katmanında** (ARP probe'ları da netem kuyruğuna girer → çözümleme timeout → arp_queue sessiz drop;
neigh girdileri STALE gözlendi). **Fix: kalıcı neighbor-pin, harness/netem-profiles.sh `pin_neigh`**
— pin sonrası 2 koşu sıfır gap/sıfır loss. SONUÇ: (i) motor TÜM koşularda DOĞRU davranmış (gerçek
kaybı kanıtla sınıflandırdı — TUR-1a mekanizması sahada doğrulandı); (ii) ölçüm-1'in "kalan 2
loss_time (kanıtlı-ama-cadence)" kalıntısı büyük olasılıkla AYNI artefakt = gerçek kayıp →
TUR-1b'nin "2→0" hedef metriği pin'li ortamda ZATEN 0 (1b'nin değeri 0.25k/koruma-katmanına kayar);
(iii) pin'siz TÜM geçmiş sub-kbps ölçümler (quinn baseline dahil) bu artefaktı içerir — simetrik
olduğundan karşılaştırma bozulmaz ama kampanya pin'li koşulmalı (r5'e not).

**Guard-revizyonu (ölçümle):** ilk uygulanan mutlak `MIN_RTT_FLOOR=1ms` bench-loopback'te ÇÖKTÜ
(gerçek RTT sub-ms → tüm örnekler red → srtt kurulamadı → PreConverge kilidi) = kendi ZM-2 ihlalim,
testle yakalandı → **RFC 9002 §5.3 gözlenen-min guard'ıyla değiştirildi**: düzeltme yalnız sonuç
rtprop'un altına inmiyorsa; aksi hâlde RAW işlenir, örnek ATLANMAZ (sample-skip = K-1 sınıfı açlık).
**AFTER (V2 0x14 15B gövde §2.2-konform + 256µs + floor-encoding + satürasyon-sayacı + min-rtt guard):**
temiz 1kbps: **teslim 41→45/100** (tahmin bandı 43-47 ✓), loss_pkt=0 ✓, loss_time=0 ✓ (tahminden iyi),
dup=retx=0 ✓, btlbw 88→102 stabil-plato ✓ (app-limited, TUR-3c bağımlılığı kayıtlı), rtt_rej=0 ✓,
satürasyon=0 ✓. **Kodlama sadakati kanıtı:** adly_max 1.43s DECODED+SUBTRACTED (eski tavan 65.5ms —
21×büyük hold artık görünür/düşülür); hold-sıçrama sınıfı örnekler kayboldu; srtt saf kuyruk-fiziği
(kuyruk boşalırken DÜŞÜYOR — r1 t=100-115: 23.6→20.3s).

**Smoke:** p4 100/100, oran 1.01× ✓ (kararlı, 4 ölçümde). **p5 — İKİ AŞAMALI BULGU:**
(i) *sample-skip'li ilk guard'la* {13,5,13}: sunulan birim 28→15 (tx 7924→4245 deterministik) —
büyük-ack_delay'li örnekler SKIP → estimator açlığı derinleşti.
(ii) *RFC-9002 guard'la (final, örnek atlanmaz)* **BIMODAL {100, 13, 4}**: şanslı-mod erken-SACK
örnekleri sağlıklı → **100/100, oran 1.12×, c10=0.93, data=100 birim + retx=1698B + dup=1698B —
MEŞRU TAŞIYICI UÇTAN UCA İLK KEZ ÇALIŞTI** (parazit 4.31×/93673B'ye karşı temiz 1.12×); açlık-modu
(r3 TRAJ): ilk SACK t≈40 (hold 7.77s V2-decoded ✓, srtt=31ms gerçek-path ✓ — 1x DOĞRU), ama tek
kümülatif-SACK'ten türeyen bw-örneği btlbw'yi 31 B/s'e FLOOR-KİLİTLİYOR → RecoverFwd'de 120s
floor-crawl (15 birim, dup bile akmıyor — rate-açlığı). **Kökler sıradaki turların tam kendisi:**
ACK-starved kümülatif-SACK bw-örneği semantiği (TUR-2/Karn-ledger), floor'dan yukarı yol yok
(TUR-3c ProbeUp — app-limited kilit sınıfı), fuzzy (TUR-4), MIN_CWND (D-4/2). P5-GATE: durum
"meşru-mekanizma-varlığı KANITLANDI, stabilite yok (bimodal)" — kabul-kriteri stabiliteyi de içerir.

## 0.9 TUR-1b ölçüm kaydı (ölçüm-noktası 3/3)

**Beklenti-kaydı önce** (scratchpad tur1b-prediction.md). Tek değişken: cadence-farkında taban
`T_loss = max(9/8·srtt, min_delay, K_c×EAI)`, K_c=1.5, EAI sender-yerel deterministik (D-1/2 formülü).
**Gerçekleşen = tahmin, tam isabet:** temiz 1kbps teslim 45 DEĞİŞMEDİ ✓, loss=0/0 KALDI ✓ (eşik
yalnız gevşer; değeri KORUMA — erken-pencerede taban≈11.2s vs 9/8·srtt≈3.4s → 3× marj; artefakt-
sonrası kalan cadence-riski sınıfının yapısal kapanışı). p4 100/100 + 1.01× DEĞİŞMEDİ ✓ (kayıpsız →
eşik hiç tetiklenmez). p5 bimodal KALDI {10 açlık | 100 şanslı} ✓; şanslı-modda c10=0.99,
retx 1698→849 DÜŞTÜ (daha az erken-tetik ✓ tahmin), dup 5094 (liveness taşıdı), oran 1.21×.
Saat-sınıfı beyanı (ZM-3): EAI kanal-türevi + worst-corner return-önseli [CAL] — §16.2/§16.1'e girdi.
NOT: ölçüm-1'deki "kalan 2 kanıtlı-ama-cadence loss" hedefi ARP-artefakt bulgusuyla (§0.8) yeniden
yorumlandı — pin'li ortamda zaten 0'dı; 1b'nin ölçülebilir birincil değeri 0.25k/TUR-C'de görünecek.

## 0.10 TUR-2a ölçüm kaydı (W_loss event-ledger + un-loss; yeni PR)

**Beklenti-kaydı önce** (scratchpad tur2a-prediction.md). Abkarada'nın sorusuna tahminim:
"un-loss beslenince btlbw-örnek-sansürü azalır mı?" → **HAYIR (2a'da yapısal imkânsız — BW-örneği
SentPacket'in korunmasını gerektirir = 2b tombstone işi; açlık-modunda pend≈0 zaten loss-yolu değil)".

**Gerçekleşen — tümü tahmin bandında:**
- Temiz 1kbps: teslim 45, loss=0/0, unloss=0, lppm=0 — HİÇBİR metrik değişmedi ✓ (ledger temiz
  kanalda boş). p4 100/100 + 1.01× ✓.
- **p5 (n=6, iki-mod AYRI izleme — talimat kapısı):** {7, 11, 12, 30, 100, 100}.
  *Açlık-modu* {7-12}: önceki taban {4-13} bandında ✓ (tx=4245 birebir — "2a dokunmaz" tahmini ✓;
  btlbw-sansürü AZALMADI, tahmin doğrulandı → 2b'nin gerekçesi ölçümle sabit).
  *Şanslı-mod* 2/6 (~%33, tarihsel ~%40 tutarlı): r4 100/100 c10=1.00 (liveness-taşıyıcılı,
  dup=5943B); **r6 100/100 c10=0.97 retx=849 — windowed-ppm ARQ'yu ÖLDÜRMEDİ** (riziko-yolu
  ölçümle çürüdü); btlbw şanslı-modda 8.3-10.3 kB/s'e tırmandı. Kapı: şanslı ≥ önceki ✓,
  açlık ≥ önceki ✓. Yeni gözlem: r5=30/100 ARA-MOD (31 birim — kısmi kaçış; bimodal aslında
  süreklilik-spektrumu, kökü aynı).
- **Ledger sahada:** r6'da canlı lppm=30000 (3 gerçek Confirmed / pencere-gönderim) → fuzzy İLK KEZ
  gerçek pencere-loss sinyali aldı + ARQ tam draini; r4'te olaylar pencere-dışı decay → lppm 0'a
  DÖNDÜ = **monoton-ratchet ölümü SAHADA** (K-1 fuzzy-zehirlenmesi kökü kapandı).
- unloss=0 tüm p5 koşularında (kayıplar GERÇEKti — %5 forward loss; spurious yok). Un-loss yolu
  birim-vektörlerle kanıtlı (G5 + sender-testi: geç-ACK → pending-iptal + note_spurious + kol-1 RTT).
- Adaptif packet-threshold (§9.3) kablolandı: besleme (note_spurious) + tüketim (k=max(K,ρ×reorder)).

## 0.11 TUR-2b ölçüm kaydı (active-flight/tombstone; D-1'in son parçası, D-1→D-2 köprüsü)

**Beklenti-kaydı önce** (scratchpad tur2b-prediction.md), Abkarada'nın istediği #7-ayrımıyla:
tombstone btlbw-ÖRNEĞİNİ serbest bırakır ama app-limited kilidini TAM AÇMAZ (o ProbeUp/TUR-3c) —
2b = "örnek üretiliyor + kısmi gevşeme", "tam ramp" DEĞİL; "kilit tam açılmadı" 2b-başarısızlığı
olarak OKUNMAZ. İKİNCİ keskinleştirme (2a-ölçüm gerçeğinden): tombstone-hit geç-ACK'in FİİLEN
gelmesine bağlı; p5'te unloss=0 ölçüldü → **p5-açlık-modunda hit≈0, btlbw~31 KALIR, tx=4245
DEĞİŞMEZ, {7-12} KAYMAZ tahmini** (kilit kökü kümülatif-SACK bw-örnek aralığı + ProbeUp-yokluğu);
mekanizmanın yaşadığı yer = ilan-sonrası-geç-SACK rejimleri → **p2 mekanizma-kolu olarak eklendi**
(tahmin: p2'de tombstone-hit>0 + bw_recovered>0; teslim 3→3-15 mütevazı — p2 taşıyıcısı TUR-B).

**Gerçekleşen (matris: 1k + p4 + p5×9 + p2×2, ARP-pin'li):**
- Temiz-1k 45/100 0/0 ✓, p4 100/100 1.01× ✓ — değişmedi (tomb=0: kayıp yok → kayıt yok).
- **p5 ×9: {6,6,12,12,12,12,14,14,14} — tamamı açlık-ucu, tahminle birebir:** tx=4245 birebir
  (r9: 8207 ara-gönderim), btlbw~31-86 kaldı, tombstone-hit=0 (kayıplar GERÇEK — geç-ACK yok).
  Keskinleştirilmiş tahmin ("hit≈0, band kaymaz") ✓. Açlık-uç ≥ önceki ✓ (band {6-14} ⊇ {4-13}
  örtüşür, taban 6>4). **Şanslı-uç: 0/9 çekiliş — DEĞERLENDİRİLEMEDİ** (P(0/9|~%29-33)≈%3-5
  şanssızlık-kuyruğu). Birincil savunma YAPISAL: 9 koşunun tümünde hit=0 → 2b'nin sıcak-yol
  eklentileri (insert/prune/hit-check) karar-yolunu 2a'yla BİT-ÖZDEŞ bırakır → şanslı-mod
  frekansı 2b'yle değişemez; 0/9 = netem-çekiliş varyansı. TUR-3a retest'i spektrumu yeniden
  örnekleyecek (kalan-belirsizlik oraya devredildi, gizlenmedi).
- **p2 mekanizma-kolu: tahmin ✗ — DEKOMPOZE (yapısal içgörü):** tombstone'lar OLUŞTU (5, 9 kayıt)
  ama hit=0, bw_recovered=0; teslim {2,3} (önceki 3 bandı). Neden: hit'in kaynağı "yanlış-ilan-
  sonrası-geç-ACK" sınıfı; **TUR-1a kanıt-şartı + TUR-1b cadence-tabanı bu sınıfı yapısal olarak
  KURUTTU** — kendi eşik-düzeltmelerimiz tombstone'un tetiklenme-alanını daralttı. Tombstone artık
  throughput-sürücüsü değil: (i) nadir-olay doğruluk-sigortası (reorder/aşırı-geç-SACK rejimleri),
  (ii) **TUR-2c Karn-kurallarının önkoşul-altyapısı** (kayıt olmadan kol-1/kol-2 ayrımı imkânsız),
  (iii) K-1 sansür-kolunun yapısal kapanışı (kayıt artık DÜŞMÜYOR; kurtarma-yolu birim-testlerle
  kanıtlı: geç-ACK → BW-örneği + RTT + plato-round beslemesi; expiry → örneksiz).
- Ledger canlı-değer kanıtı sürüyor: p5-r7 lppm=71428, p2-r1 lppm=200000 (pencere-içi gerçek oran).

## 0.12 TUR-2c ölçüm kaydı (Karn RTT/BW kuralları; D-1 KAPANIŞI)

**Beklenti-kaydı önce** (scratchpad tur2c-prediction.md), iki talimat-şartıyla:
**[ŞART-a] NÖTR-ZİNCİR ÇERÇEVESİ:** 2a+2b+2c üç tur üst üste throughput-NÖTR — doğru ve beklenen;
D-1 "temiz zemin" ayağıdır, hiçbiri tek başına teslim kazanımı vermez, kümülatif K-1 kapanışıdır
(eşik[1a+1b]+ratchet[2a]+sansür[2b]+Karn[2c]). İlk teslim-kazanımı TUR-3c; D-1 turları 3c'nin
MÜHİMMATI. "Üç tur ilerlemedik mi?" okuması yanlıştır.
**[ŞART-b]** şanslı-mod 0/9 borcu §3.7'ye girdi (TUR-3a'da doğrula/çürüt).
**Tahmin:** her kol değişmez; 2c-tetikleyicisi (retx-sonrası-geç-orijinal-ACK) bu kollarda ≈0
ateşlenir (açlıkta retx≈0; şanslıda kayıplar gerçek) → davranış-kanıtı birim-vektörlerde.
**2c'nin kapattığı gerçek delik:** 2b'de tombstone.retx_count İLAN-anında donuyordu → retx-sonrası
geç-orijinal-ACK yanlışlıkla kol-1'e düşer (sahte-büyük RTT + yanlış-atıflı rate-örneği) —
superseded-işareti bunu kapattı; kol-2 artık delivered-KREDİSİ verir (örneksiz, `credit_delivered`).

**REVIEW-WORKFLOW (ultracode, commit-öncesi):** 3-mercek düşman-inceleme 12 bulgu üretti;
doğrulayıcılar session-limit'e düştüğünden özet-verdikt GEÇERSİZDİ (refuted_count=12 artefakt) —
journal'dan ham bulgular alınıp elle yargılandı: **2 gerçek kod-fix** (1: superseded-işaret yalnız
ARQ-kolundaydı — FWD-dup pending-drain + HandToForward yolları da yeniden-emisyondur, tekdüze
`mark_tombstone_superseded` yapıldı; 2: kol-2 kanonik spurious-teyidi `note_spurious`u beslemiyordu
— §9.3 kablosu tamamlandı), **2 doc-zeyl** (wire-byte vs birim-teslim çift-sayım semantiği; kredi
delta-etkisi = stok-BBR eşdeğeri — spec §5 ZEYL-2/3), **1 bayat** (spec-güncellenmedi — workflow
başladıktan sonra işlenmişti), **1 borç-detayı** (§3.7 packets_in_flight: üç no-decrement yolu),
**2 yeni vektör** (karn çift-ACK sıralaması; tv_bw kredi-çıpası).

**Gerçekleşen (1k + p4 + p5×6, ARP-pin'li):**
- Temiz-1k 45/100 0/0 ✓, p4 100/100 1.01× ✓ — nötr-tur (şart-a çerçevesi) tahminle birebir.
- 2c-tetikleyicisi ≈0 tahmini ✓: tombstone-hit=0 tüm kollarda (kayıplar gerçek; tombstone'lar
  oluştu: 1/2/5 kayıt). Davranış-kanıtı birim-vektörlerde (kol-2: RTT değişmez + rate-örneği yok +
  kredi + reorder-toleransı yükselir; çift-ACK sıralaması; kredi-çıpası).
- **p5×6: {11,12,12,20,21,100} — ŞANSLI-MOD GERİ DÖNDÜ** (r3: 100/100, c10=1.00, oran 1.07×,
  retx=1415B + dup=566B meşru taşıyıcı; lppm=50000 canlı). §3.7 borcunun "2b öldürdü mü" kolu
  gözlemle çürüdü; frekans-sorusu (≈%30 bandı mı) 3a'nın büyük-n retest'ine kaldı. Ara-modlar
  (20, 21) süreklilik-spektrumu okumasını pekiştiriyor. Açlık-uç {11-12} ≥ önceki taban ✓.
**D-1 KAPANDI** (1a→1b→2a→2b→2c): kayıp artık kanıt-şartlı, cadence-farkında, pencere-türetimli,
geri-alınabilir, kayıt-korumalı ve Karn-kurallı bir HİPOTEZ. Sırada D-2: TUR-3a (estimator
pencereleri — §3.7 borçlarının ikisi de masasında).

## 0.13 TUR-3a ölçüm kaydı (D-2/1+2: pencereler + keep-last; ÜÇ AYRI ölçüm — talimat şart-b)

**Beklenti-kaydı önce** (scratchpad tur3a-prediction.md; şart-a ara-çerçevesi başta).

**(i) Şanslı-mod frekansı büyük-n [BORÇ REVİZE-KAPANDI]:** taze n=12 (D-1-final kod) = 1/12; havuz
post-2b 2/27 ≈ %7.4 vs pre-2b 4/11 ≈ %36 (Fisher p≈0.05, post-hoc). Beklenti-kaydının öngördüğü
git-bisect-ölçüm dalı koşuldu: **AYRIŞTIRICI = 2a-binary'si (b68cd2d) BUGÜN p5×8 → 1/8 (%12.5)** —
kendi tarihsel %33'üne değil bugünkü-era havuzuna oturdu. **SONUÇ: kod-nedeni DIŞLANDI (yapısal
bit-özdeşlik savunması DOĞRULANDI); frekans ORTAM/ZAMAN-duyarlı** (bugünkü-era bandı 3/35 ≈ %8.6).
Çıkarımlar: (1) şanslı-uç kapı-kıyasları AYNI-ERA çekilişleriyle yapılmalı; (2) P5-GATE kriteri
etkilenmez (hedef spektrumun dar-IQR'a çöküşü — frekans değil).

**(est) Pencereler + keep-last [KISMİ ✓ + KIRMIZI-SAPMA, dekompoze]:**
- **0.25k süreklilik ✓ TAM İSABET:** baseline btlbw HER örneği izliyordu (tek-örnek-pencere;
  5 düşüş-adımı, 18-27 salınım); commit-A sonrası **düşüş-adımı 0** (monoton 21→27) — gerçek
  max-filter davranışı geldi. Keep-last vektörü: valid=false güncellemede btlbw→0 flap'i öldü.
- **Temiz-1k ✗ 45→34/100 (tahmin-bandı 45±3 DIŞI) — DOĞRUDAN ATIFLI:** TRAJ+qd-diag: rtprop artık
  DÜRÜST (3.056s'e sabit — propagation-sınıfı; eskiden 10s-pencere şişkin-RTT'ye tazeleniyordu) →
  qd = srtt−rtprop GERÇEK kuyruğu gösteriyor (1.6→10.3→4.3s) → **mutlak-eşikli fuzzy qd (D-3'ün
  BİLİNEN kırığı) her değeri maksimal-HIGH okuyor → send_rate_gain 0.5× KALICI** (rate=51=btlbw/2
  mıhlı; kuyruk boşalıp qd 4.3s'e inse bile — 4.3s ≈ bu bantta FİZİKSEL-TABAN qd). Eski davranışta
  İKİ kırık birbirini maskeliyordu (çöp-rtprop → qd≈0 → eşik hiç tetiklenmiyordu) — D-1'deki
  parazit-iptali hikâyesinin D-2/D-3 tekrarı. Verim tersine İYİLEŞTİ: deliv/sent 34/36=%94
  (eski 45/59=%76), oran 0.60→0.36×; ama kampanya-metriği (completion@deadline) geriledi:
  kanal ~45 taşıyabilirken 36 sunuldu (0.5×-gaz kanal-altı).
→ **KARAR (Abkarada, 2026-07-13): (b) qd_ratio minimal-önalım — kapsam-kilidiyle.** (a) RED:
gerilemiş baseline üç tur taşınır, sonraki turlar kirli zemine oturur. (c) RED — kritik gerekçe:
W_RTPROP'u ertelemek şişkin-rtprop MASKESİNİ GERİ TAKMAK; kirli-saat D-2/D-3'ün sonraki tüm
turlarını kirletir (W_BTLBW ack_epoch, ProbeUp süre-ölçümü hepsi rtprop'a bakar) — #4-düzeltmesinin
(kirli-saat üstüne kurma) tam tersi olurdu. KAPSAM-KİLİDİ: yalnız qd kolu (qd_ratio =
queue_delay×1000/rtprop; rtprop=None→0+sayaç); D-3'ün kalanı (bw log-ölçek, sıfır-ağırlık fallback)
TUR-4'te. Kural: "rtprop-fix'inin yaşaması için gereken MİNİMUM fuzzy-değişikliği = qd_ratio,
o kadar." Bu çiftlenmiş-değişken, iki-değişken değil. DOĞRULAMA-ŞARTI: temiz-1k ≥45'e dönmeli;
dönmezse ayrı teşhis, genişletme YOK, Abkarada'ya.

**qd_ratio SONUÇ (commit-A2) — DOĞRULAMA-ŞARTI SAĞLANMADI, β-dalı (teşhisle Abkarada'ya):**
- qd_ratio MEKANİZMASI ÇALIŞTI: erken-faz boğulması kalktı (Startup 15→19 birim/20s), temiz-1k
  **34→42** (+8); 0.25k süreklilik KORUNDU (düşüş-adımı 0, monoton 21→27, 20/20); p4 100/100 ✓;
  p5×6 aynı-era {8,11,12,15,100,100} = 2/6 şanslı ✓ (commit-A 2/6 ile tutarlı).
- AMA 42 < 45: rate=51 (=102×0.5) kilidi t=40+ sürüyor. TEŞHİS (TRAJ qd-diag ile doğrudan):
  (1) **srtt-EWMA gecikmesi**: kuyruk boşalmışken (infl=267B ≈ 2s) srtt hâlâ 17.9s — α=1/8 ×
  seyrek-örnek → srtt dakikalarca şişkin → qd=srtt−rtprop sahte-yüksek → ratio 4.85 "HIGH" mıhlı.
  qd'nin saati srtt-LAG'ıyla kirli — ZM-ailesine yeni üye (EWMA yakınsama-hızı örnek-SAYISI-sınıfı,
  kanal-zamanı değil; §16.2'ye aday satır). (2) **Startup aşırı-doldurması**: 2.89× gain 20s'de
  kuyruğu 3.4kB'a (kanalın ~27s'i) şişiriyor; Startup-exit'te DRAIN fazı yok (BBR'da var) → koşunun
  kalanı drain'le geçiyor. İkisi de 3b/3c bölgesi (DrainRtprop sözleşmesi + faz-geçişleri).

**(ii) packets_in_flight [commit-B — KAPANDI ✓]:** ÖNCE-kanıtı: p5q_r3'te pif=23 vs gerçek-uçuş≈14
(**+9 sızıntı** = TTL-prune'lu 8 tombstone + dup-kayıp). Üç yol kapatıldı (`forget_in_flight`:
TTL-prune, kapasite-evict, Duplicate-kökenli kayıp). SONRA-kanıtı: p5cb_r1 pif=2 = uçuş(1)+tomb(1)
BİREBİR; p5cb_r2 pif=11 = uçuş(11) BİREBİR. Temiz-1k 42 değişmedi (davranış-nötr; pipe-boş
yeniden-çıpalama artık kayıplı koşuda da erişilebilir). §3.7 borç satırı KAPANDI.

**KARAR (α) + ÜÇ ŞART (Abkarada, 2026-07-13):** 42 = commit-A/A2'nin dürüst ara-değeri, kabul;
(β: srtt-anlık-fix 3a-içi) RED — srtt-EWMA fix'i motorun her yerine dokunan KÖK, kendi izolasyonunu
hak eder. **ŞART-1:** temiz-1k ≥45 dönüşü = **TUR-3b DOĞRULAMA-ŞARTI** (açık-kapı; "42 geçici-kabul,
İZLENEN" — kaybolmaz). **ŞART-2:** srtt-EWMA bulgusu KÖK olarak kayıtlı: (a) §16.2'ye
ZM-estimator-katmanı satırı girdi; (b) 3b-kararı: "srtt yakınsaması KANAL-ZAMANLI olmalı";
(c) TARAMA SONUCU — srtt-lag'ını miras alan **9 tüketici**: loss-eşiği 9/8·srtt · ack_epoch
(→W_BTLBW+W_loss) · W_RTPROP · tombstone-TTL · jitter-normalize (rttvar/srtt) · SRC Little-kolu
cwnd/srtt (42-kilidinin İKİNCİ yarısı) · liveness-dup aralığı · ack_confidence 3×srtt ölçeği ·
qd=srtt−rtprop. (PTO: engine-içi henüz yok — spec-düzeyi, gelecek.) → "srtt-fix bir tur mu tek
satır mı" CEVABI: dokuz-tüketicili KÖK = kendi turu. **ŞART-3:** Startup-exit DRAIN-fazı-yokluğu
ayrı bulgu (ProbeUp-unreachable'ın kardeşi — BBR Startup→Drain→ProbeBW'nin Drain ayağı da eksik);
3b sözleşmesi ÜÇLÜ: giriş + çıkış + drain, hepsi ZM-saat-sınıfıyla.

**3b-YAPI ÖNERİSİ (Abkarada'nın sorusuna; karar onda):** tarama "her yere taşıyor" dediği için
**3a.5 AYRI ÖN-TUR** öneriyorum: 3a.5 = srtt-kanal-zamanlı yakınsama (tek-değişken; kendi
beklenti-kaydı + tam kol-matrisi; ölçüm-altyapısı-önce ilkesinin — 1x'in — birebir analoğu: 3b'nin
dwell/drain ölçümleri srtt-SAATİYLE yapılacak, kirli saatle sözleşme ölçülmez, #4 dersi). Sonra
3b = DrainRtprop-sözleşmesi + Startup-exit-Drain (iki commit, tek PR; doğrulama-şartı temiz-1k ≥45).
Alternatif (üç-commit'li tek 3b: srtt + Drain + DrainRtprop) daha az PR ama üç büyük-değişken tek
turda — önermiyorum.

## 0.15 TUR-3b TASARIM-KAYDI (Drain state-makinesi; ultracode-niyetli — haftalık subagent-limiti
nedeniyle İNLİNE derin-analiz + beyan, 3a.5-emsali)

**p5-çerçevesi (talimat, beklenti-kaydına):** {11,24,100,100,100,100} P5-GATE HAREKETİ, GEÇİŞİ DEĞİL —
hâlâ bimodal (IQR 11→100); srtt-fix şanslı-mod FREKANSINI artırdı, açlık-modunu ÖLDÜRMEDİ.
Açlık-modunun yapısal ölümü = ProbeUp (3c). "srtt p5'i çözdü" yanlış-umudu YASAK; {11,24} aksinin kanıtı.

**(i) Startup-exit DRAIN fazı (BBR-analog; β-bulgusu doğrudan girdi):**
- Yeni state `Drain`: Startup-plato → Drain → Steady (eski: direkt Steady → overfill Steady'ye taşınıyordu).
- Gain = Startup'ın tersi: 1000/2885 [STD:BBR drain=1/high_gain]. PacingPhase::Drain + Gain::DRAIN.
- ÇIKIŞ (byte-karşılaştırma — saat-sınıfsız, ZM-temiz): `inflight_bytes ≤ BDP(btlbw×rtprop)`
  → Steady; YEDEK: elapsed ≥ K_drain_max×ack_epoch [CAL {2,3,4} başlangıç 3] → Steady (liveness;
  ACK-açlığında sonsuz-Drain yok). Emergency/pending öncelikleri Drain'i kesebilir (mevcut sıra).
- ZM-beyanı: dwell kanal-zamanlı (ack_epoch); tetik byte-eşiği (saat-bağımsız).
**(ii) DrainRtprop SÖZLEŞMESİ (D-2/3 onaylı formülle):**
- GİRİŞ: RtpropProbe-tick + Steady + **rtprop-penceresi-doluşu** (min-örneğin yaşı ≥ W_rtprop/2
  [CAL kesir]; 200ms-sabit-tick tetikçiliği biter — pencere tazeliği kanal-saatli).
- ÇIKIŞ-İNVARİANTI (onaylı): `(elapsed ≥ min_dwell ∧ qualifying_min_rtt_örneği) ∨ elapsed ≥ max_dwell`;
  min_dwell=1×ack_epoch, max_dwell=3×ack_epoch [CAL]. VERIFIED N=0 bulgusu (aynı-event çıkış +
  ProbeDown-gain hiç uygulanmıyor) kapanır.
**(iii) Tüketici-8 EAI-tabanı (3a.5-devri):** sessizlik-eşiği = max(3×srtt_t, K_CONF_EAI×EAI);
K_CONF_EAI=2 [CAL {1,2,3}] — srtt_t hızlı-düşüşünün sahte-LOW-conf/erken-RecoverFwd riski kapanır.
**İZLENEN-BORÇ (yeni, §3.7'ye):** Little-kolu fixture-min_cwnd'iyle MASKELİ (3a.5-α'da bağlamadı;
yarı-2 öngörüm aşırı-temkindi) — MIN_CWND kalibrasyonu (SON'da) gelince kol yeniden görünebilir.

**Beklenti-kaydı (ölçümden önce; talimat: 3b-tırmanışı vs 3c-kalanı AYRIMI):**
- commit-A (ii): temiz-1k DEĞİŞMEZ (45; sözleşme-doğruluğu — birim-vektörler taşır; DrainRtprop
  artık gerçek dwell yaşar, ProbeDown-gain fiilen uygulanır).
- commit-B (i): **3b-tırmanışı = overfill kesilir** → kuyruk-israfı düşer → temiz-1k 45→46-49 bandı;
  **3c'ye kalan = app-limited ramp**: btlbw 102'de kilitli kalır (plato app-limited), kanal-tavanı
  (~125B/s → 50-53 birim) ProbeUp'sız VURULAMAZ. "3b tavana varmadı" okuması YANLIŞ olur — beklenen.
- commit-C (iii): temiz-1k nötr; p5-açlıkta ÇİFT-YÖNLÜ risk (geç-RecoverFwd → daha az spekülatif dup:
  açlık-teslimi düşebilir DE yükselebilir DE — dürüst belirsizlik, iki-mod ayrı ölçülür).
- Doğrulama: temiz-1k ≥45 KORUNUR ∧ TIRMANIR; smoke p4/p5×6 aynı-era; tam-matris PR-öncesi.

## 0.16 TUR-3b ÖLÇÜM-KAYDI — commit-A ✓; commit-B'de YAPISAL-BULGU, DUR-noktası

**commit-A (9051164, DrainRtprop sözleşmesi):** temiz-1k 45 DEĞİŞMEDİ ✓ (tahmin). N=0 öldü;
birim-vektörler: giriş-reddi/dwell/invariant. Test-yazımında 3a.5-β'nın canlı-kanıtı: qualifying-ack
büyük örneği srtt'yi kuantum-döngüyle taşıyıp min_dwell'i büyüttü (dürüst estimator hareketi).

**commit-B (da44cf0 + 2×BDP ayarı; Startup-Drain):** temiz-1k 45→43 — beklenti (46-49) YANLIŞTI ve
sapmanın dekompozisyonu TURUN ASIL DERSİ: **ayakta-kuyruk varken kanal ZATEN %100 doluydu — drain
teslimi ARTIRMAZ, kuyruğu öldürür.** Teslim-kaybı SONDAN: kuyruk ölünce (t≈90; son-faz srtt=3.09s≈
rtprop, qd=0 — CENNET-zemin) Steady rate=102 < kanal-125 → app-limited-kilit son ~30s'de kanalı AÇ
bırakıyor (≈−2.6 birim). **ESKİ 45, ayakta-kuyruğun app-limited-kilidi MASKELEMESİYLE geliyordu** —
3b maskeyi soydu (D-1/D-2 maske-soyma serisinin devamı), açığa çıkan hırsız = 3c'nin tam hedefi
(btlbw 102→~125 ramp). Drain-hedef ayarı (BDP→2×BDP) süreyi değiştirmedi (fizik aynı). Latency-
faydası BU bench-metriğinde görünmez (sent_ns=app-offer; hepsi t0'da → pacing-dwell domine) —
dürüst kayıt: drain'in ölçülebilir kazanımı dürüst-zemin + bench-dışı sınıflar (geç-katılan trafik,
blackout-taşıma riski).

**→ KARAR (Abkarada, 2026-07-13): (a) commit-B KALIR** — belgeli −2 (43), 3c kuyruk-maskesiz
temiz-zeminde. (b) RED: sahte-kuyruk-zemin 3c'yi kirletir + Drain 3c'yle iki-değişken (şişkin-
rtprop-maskesi reddiyle aynı sınıf). (c) RED: yarım-maske, tam-maskeden beter. **Maske-soyma
serisinin 4.'sü olarak kayıtlı:** D-1 (parazit 100→24) → 3a (rtprop → 34) → 3a.5-β (overfill) →
3b (kuyruk-drain → app-limited-kilit). Desen: maske soyulur → sayı geçici düşer → altındaki kilit
çıkar → kilit ZATEN sıradaki turun konusu. 3b, 3c'nin ne yapacağını ÖLÇEREK gösterdi.

**3c-DEVİR (onaylı, HİPOTEZ-formülasyonuyla — 3c beklenti-kaydına aynen):** "app-limited-kilit
ProbeUp ile btlbw 102→~125 ramp → temiz-1k 43 hem ≥45'e DÖNER hem tavana (50-53) TIRMANIR.
DÖNMEZSE kilit ProbeUp-dışı bileşen taşıyor → ayrı teşhis, ProbeUp'ı GENİŞLETME." Kilidin SEBEBİ
ölçüldü (rate=102<125, son-faz temiz); ProbeUp'ın ÇÖZÜMÜ henüz kanıtlanmadı — mantık sağlam,
ölçüm 3c'de.

**Yan-gözlem (kayıt, talimat):** commit-A test-yazımında 3a.5-β canlı göründü — büyük qualifying-ack
(12.7s) srtt'yi kuantum-döngüyle taşıyıp min_dwell'i büyüttü = srtt-türev-fix'inin doğru çalıştığının
EK kanıtı.

## 0.17 TUR-3b commit-C DENEMESİ — 5. MASKE-SOYMA, geri çekildi, karar-noktası

commit-C (tüketici-8 EAI-tabanı) yazıldı, kapıda KIRILDI ve kırılma ÖĞRETİCİ: bench-loopback
9/10 — tek-kalan-son-paketin teslimi meğer **liveness-dup → DupSuppress-force-ACK** gizli-yoluna
dayanıyormuş: receiver'ın STATİK 8.39s cadence'i (§16.2'de zaten SUSPECT: "ACK_CADENCE_FLOOR
statik-prior") tek-paket-tail'in timer-SACK'ini deadline-dışına atıyor; eski 3×srtt-eşiği (fast'ta
ms'ler) conf'u anında düşürüp RecoverFwd-dup'ı ateşliyor, dup force-ACK tetikliyordu. EAI-tabanı
(2×EAI≈4.2s; LOW-eşiği ≈14s) bu gizli-yolu kapattı → tail teslim edilemedi. **MASKE-SOYMA #5:**
tail-ACK-zamanlılığı liveness-emergency'ye (ağır mekanizma) dayanıyormuş — gerçek borç receiver
statik-cadence (tek-paket-tail için timely-ACK; QUIC max_ack_delay≈25ms sınıfı).
AYRICA yeni iz: loopback tx=1550B = 17 data-emisyonu / 10 birim (7 fazla-data — retx/dup DEĞİL;
kaynağı belirsiz, AYRI teşhis ister).

**→ commit-C working-tree'den GERİ ÇEKİLDİ** (A+B kapıları yeşil; C commit'lenmedi).
**KARAR-NOKTASI (Abkarada):** tüketici-8 tabanı nasıl gelmeli?
(a) C'yi receiver tail-ACK fix'iyle EŞLEŞTİR (son-paket/tek-paket timer-SACK'i cadence-muaf ya da
    kısa-gecikmeli — QUIC-max_ack_delay sınıfı; §3.3-E-FULL'ün öne alınmış dar-parçası) → çift
    taraf birlikte: liveness ağır-yol olmaktan çıkar, conf-tabanı güvenle gelir [ÖNERİM — kök-fix];
(b) C'nin tabanını yalnız EAI'nin KANAL-koluna bağla (statik-prior'suz) → fast'ta taban≈0 (eski
    davranış), sub-kbps'te koruma kalır; ama EAI-semantiği bölünür + statik-cadence borcu maskeli
    kalır;
(c) C'yi 3c-sonrasına ertele (liveness-sözleşmesi 3b'de eksik kaldı — dürüst kayıt).
17-data izi: karar ne olursa olsun AYRI teşhis-maddesi (yeni borç-satırı §3.7'ye).

## 1. KARARLAR

### D-1 — Loss-modeli yeniden-tanımı (K-1 kökü; İLK) [düzeltme #1, #2, #3 işlendi]

1. **[#1] İKİ MEKANİZMA AYRI — classification vs liveness (TUR-1a'da uygulandı):**
   - *Loss classification:* higher-ACK kanıtı olmadan time-threshold KESİN loss YAZMAZ (metrik artmaz).
   - *Liveness recovery:* **"Saf sessizlik loss metriğini ARTIRMAZ; fakat liveness amacıyla
     probe/retransmit tetikleyebilir"** — §5.1 ack_confidence → RECOVER_FWD spekülatif geometrik kopya
     (taban=srtt, ×2, ACK'le iptal; Duplicate sayılır, loss sayılmaz). Gerçek tail-loss böyle yaşar.
2. **Cadence-farkında eşik (TUR-1b):** `T_loss = max(9/8·srtt, K_c × beklenen_ACK_aralığı + marj)`;
   `beklenen_ACK_aralığı = ACK_FREQ_N × unit_wire / send_rate_fiili + sack_dönüş_serialization`
   (sender-yerel, deterministik). K_c=1.5 başlangıç, matris {1.25, 1.5, 2.0}.
3. **[#3] UN-LOSS + retransmission-ambiguity — İKİ KOL:**
   - *ACK retransmit gönderilmeden ÖNCE geldi:* spurious → pending-retransmit iptal + loss-olayı
     geri-al (ledger, aşağıda) + **orijinal RTT örneği GEÇERLİ** (tek gönderim, belirsizlik yok) +
     BW örneği normal.
   - *ACK retransmit gönderildikten SONRA geldi (Karn ambiguity):* teslimat BİR KEZ muhasebe +
     pending temizle + spurious/ambiguous sayacı + **RTT örneği ALINMAZ** (generation wire'da ayırt
     edilemiyor: ilk-ts→sahte-büyük, retx-ts→sahte-küçük). **BW:** delivered byte'ları sampler'ın
     kümülatif `delivered`'ına GİRER (teslimat gerçek) ama bu ACK'ten **rate-örneği ÜRETİLMEZ**
     (interval hangi gönderime ait bilinmiyor) — `sample_skipped_ambiguous++`.
   - *[#3] W_loss EVENT-LEDGER (integer-decrement DEĞİL):* seq-başına durum
     `Suspected → Confirmed | Spurious | Expired`; `loss_ppm` pencere-içi mevcut event-state'lerinden
     TÜRETİLİR (Confirmed/pencere-içi-toplam). Monoton ratchet yapısal biter; spurious→
     `ReorderTracker.note_spurious` beslenir (adaptif reorder_thresh nihayet kablolanır).
4. **[#2] ACTIVE-FLIGHT vs TOMBSTONE AYRIMI ("inflight'tan silinmez" DEĞİL):**
   - *Active flight accounting:* `loss_suspected` olan paket `bytes_in_flight`/cwnd muhasebesinden
     DÜŞÜLEBİLİR (gönderim durmaz).
   - *Retained transmission record (tombstone):* geç-ACK eşleştirme için AYRI bounded kayıt:
     `zaman-sınırı = min(mesaj deadline'ı, TOMBSTONE_TTL=K_tomb×T_loss, K_tomb=8 [CAL])` ·
     `max-kayıt = TOMBSTONE_MAX=4096 [CAL]` · *eviction:* en-eski-TTL-önce · *eviction etkisi:*
     `tombstone_evicted_total++` (audit sayacı; evict edilen seq'in geç ACK'i artık `Expired/Unknown`
     — RTT/BW örneği üretmez, teslimat-damgası bench-katmanında zaten bağımsız). SR-AUDIT-001 bounded.
   - *4-decade bellek/doluş doğrulaması (talimat, 2026-07-12):* steady-state kayıt ≈ pkt_rate ×
     TTL(=K_tomb×T_loss=9×srtt). **0.25kbps:** ~0.1 pkt/s × 9×~12s ≈ **11 kayıt** (4096'nın %0.3'ü —
     eviction hiç tetiklenmez). **1kbps:** ~0.4×9×4s ≈ 15. **100kbps:** ~10×9×0.2s ≈ 18.
     **1Mbps %heavy-loss:** ~1000×9×0.05s ≈ 450. Tavan ancak ≳100k pkt/s'te (≈Gbps küçük-paket)
     dolar → eviction+audit devreye girer (tasarlanan davranış). Bellek tavanı: 4096 × (~24B kayıt +
     ~64B map-overhead) ≈ **≤352 KiB** worst-case, sabit. SONUÇ: MAX=4096 4-decade'de güvenli. ✓
5. **W_loss kayan pencere:** ledger penceresi `W_loss = K_wl × ack_epoch` [CAL] (spec §5'in borcu).

**Beklenti-kaydı (bağımlılıklar AÇIK, #7):** TUR-1b sonrası temiz-kanal loss_time 2→0; TUR-2 sonrası
kayıplı kanalda estimator sağlığı (btlbw/srtt gerçek-kayıp altında stabil) + spurious-oran ölçülür
(r5 confusion-matrix ön-provası). **Bunlar btlbw'yi 125'e ÇIKARMAZ** — ramp TUR-3c'ye (ProbeUp),
p5-tırmanışı TUR-4+TUR-B'ye bağlı. Ara-retest'ler bu bağımlılıkla OKUNUR.

### D-2 — Estimator zaman-referansı [K_w AYRILDI → K_bw + K_rtprop; ack-epoch tanımı]

1. **Pencereler (karar, talimattan):** `ack_epoch = max(corrected_srtt, expected_ack_interval)`;
   `W_BTLBW = max(10s, K_bw × ack_epoch)`; `W_RTPROP = max(10s, K_rtprop × corrected_srtt)`.
   Başlangıç K_bw=8, K_rtprop=8; kalibrasyon matrisi K∈{6,8,10} (merkez 8). Gerekçe: bant-örneği
   geliş-ritmi (ACK-epoch) ≠ min-RTT yenilenme ihtiyacı (RTT). `corrected_srtt` = ack_delay-düzeltilmiş
   srtt — **TUR-1x'e bağımlı** (kirli-saat, §2 sıra gerekçesi). Hiçbir placeholder golden'a girmez;
   ölçülen değer girer.
2. **Keep-last:** max/min-filter son örneği ASLA expire etmez → btlbw=0/rtprop=None flap'i yapısal biter.
3. **[#5] DrainRtprop (ProbeRTT) giriş/çıkış SÖZLEŞMESİ:** giriş = RTPROP_WINDOW-doluşu (200ms sabit
   tick DEĞİL); çıkış invariantı KESİN:
   `exit = (elapsed ≥ min_dwell ∧ qualifying_min_rtt_sample_seen) ∨ elapsed ≥ max_dwell`
   (örnek hemen gelse tek-event'te çıkmaz; hiç gelmese sonsuza takılmaz). Faz en az bir GERÇEK
   gönderim fırsatı boyunca uygulanır (yalnız wall-clock değil). min_dwell=1 ack_epoch,
   max_dwell=3 ack_epoch [CAL].
4. **[#5] ProbeUp SÖZLEŞMESİ (yapısal; eksikti):**
   - *Giriş:* backlog'da veri VAR ∧ recovery/emergency AKTİF DEĞİL; periyot RTT-round-türevi
     (event-count DEĞİL — ZM-1).
   - *Gain:* 1.25× (Startup 2.89× re-entry RED — Steady bağlamında agresif).
   - `probe_success = delivered_since_entry ≥ probe_volume ∧ btlbw_sample > old_btlbw × (1+ε)`
   - `probe_abort = qd_ratio > limit ∨ confirmed_loss > limit`
   - `probe_timeout = elapsed ≥ max_probe_dwell`
   - **"Başarısız probe" ≠ "kapasite artmadı"** (abort/timeout ayrı raporlanır, KARIŞTIRILMAZ).
   - `compute_send_rate`'te ProbeUp AÇIK ayrı match-kolu — `_`'ya BIRAKILMAZ.
   - **Startup-3ms-hatası tekrarlanmaz kanıtı:** her ProbeUp/Drain eşiği §16.2 ZM-tablosuna
     saat-sınıfıyla girer.
5. **Emergency dwell'ler:** `deescalate_dwell = max(1s, k_d×srtt)`; escalate adım-arası RTT-türevi taban.
6. **anchor:** spec `max(last_ack, first_send)` + aktif-dönem re-base (Idle→aktif geçişinde).
7. **plateau:** ölçüm-enablerı statüsünde (546927c'de commit'li); delivered-anchor + re-entry
   sayaç-sıfırlama revisiti K-1+pencere fix'leri sonrası.

### D-3 — Fuzzy üyelik normalizasyonu [#6 karar VERİLDİ + p5-kanıtı]

1. **qd:** rtprop-göreli `qd_ratio = queue_delay/rtprop`. **`rtprop=None` veya 0 davranışı (tanım):**
   `qd_ratio = 0` (kanıt yok → boğma yok) + `qd_no_evidence_counter++`; keep-last (D-2/2) None'u
   nadirleştirir.
2. **bw:** log-ölçek (decade-tabanlı) üyelik; sub-kbps kendi decade'i.
3. **[#6] Sıfır-ağırlık fallback — SEÇİLDİ:** `sum(rule_weights) ≤ ε` ise: son-geçerli karar
   **EN FAZLA 1 decision-window** korunur → sonra **`arq_light` güvenli-default** →
   `fuzzy_no_rule_counter++`. (Sınırsız last-valid RED — bayat arq_heavy'yi kalıcılaştırır.)
   **Membership invariant testi:** her geçerli input-kombinasyonunda `sum(rule_weights) > ε`
   (fallback güvenlik-ağı, normal-yol DEĞİL) — property-test + TV-FUZZY'ye girer.
4. loss girdisi D-1/5 ledger-penceresinden.
**p5-kanıtı (§0.7):** bu karar p5'in taşıyıcısı — arq=0 kilidi kalkmadan p5 meşru-recovery'yle taşınamaz.

### D-4 — Wire/spec-seviye [AÇIK KARARLAR işlendi; grep-kanıtı aşağıda]

**Grep-kanıtı (iddia değil, satır):**
```
$ grep -n -i "ack_delay\|ACK_FRAME\|0x10" specs/ccr-format-spec.md
2447:  candidates = [ base | seq16, (base - 0x10000) | seq16, (base + 0x10000) | seq16 ]   ← alakasız (seq16 wrap)
$ grep -n -i "ack_delay\|ACK_FRAME" specs/transport-layer-tdd.md
(boş — sıfır eşleşme)
```
→ ack_delay/ACK_FRAME **KAT-donmuş belgelerde YOK**; layout engine-spec §2 (HOW). 0x14/V2 zaten bu
döngünün D1-amend'i — KAT kaydı TETİKLENMİYOR (yeni frame, donmuş sete dahil değil).

1. **ack_delay = SEÇENEK A (karar):** u16 kalır, birim 256µs, tavan 16.78s. Şartlar (talimattan, hepsi
   uygulanacak): **yalnız-V2(0x14) semantiği** (V1 0x10 µs-birimle decode-uyumlu kalır) ·
   **floor encoding** `encoded = min(floor(actual_us/256), 65535)`, `decoded = encoded×256µs`
   (receiver delay'i ŞİŞİRMEZ → path-RTT fazla küçülmez) · **saturation-sayacı** (encoded==65535) ·
   **RTT-subtraction guard** (raw−decoded < min_rtt_floor → sample invalid, §7.3 `ack_delay_untrusted`;
   underflow yasak) · sack_wire=65B ve §16.1 türetimleri DEĞİŞMEZ.
2. **MIN_CWND (karar, ACK-progress alt sınırıyla):**
   `MIN_CWND = max(K_bdp × BDP, min(ACK_FREQ_N, 2) × unit_wire)`, `BDP = btlbw × rtprop`, K_bdp=2
   [CAL-placeholder]. Gerekçe: salt 1-birim tabanı ACK_FREQ_N=2'yle her paketi timer-SACK'e düşürür
   (ACK-hold'u yeniden üretir). 4-decade domain doğrulaması (0.25k/1k/100k/1M). **Kalibrasyonu sıranın
   SONUNDA** (aşağıda).

## 2. TUR SIRASI (REVİZE [#4]: ack_delay ölçüm-ALTYAPISIDIR, D-2'den önce)

```
TUR-1a  later-ACK + liveness ayrımı            [KOŞULDU: 546927c; ölçüm-1 §0.7]
TUR-1x  ack_delay Seçenek-A (V2) + corrected-RTT saati     ← D-2 pencereleri KİRLİ SAAT üstüne kurulmasın
TUR-1b  cadence-farkında eşik (K_c)            [tek PR TUR-1a ile, commit-2; ölçüm-2 zorunlu]
TUR-2a  W_loss event-ledger + un-loss (spurious→note_spurious kablosu)   [KOŞULDU: ölçüm §0.10]
TUR-2b  active-flight/tombstone ayrımı (parametreli, bounded)   [KOŞULDU: ölçüm §0.11]
TUR-2c  Karn-ambiguity RTT/BW kuralları   [KOŞULDU: ölçüm §0.12; D-1 KAPANDI]
TUR-3a  estimator pencereleri (K_bw×ack_epoch, K_rtprop×corrected_srtt) + keep-last
TUR-3b  DrainRtprop giriş/çıkış sözleşmesi (window-derived giriş + exit-invariant)
TUR-3c  ProbeUp state + sözleşme + açık match-kolu
TUR-4   fuzzy normalize + sıfır-ağırlık fallback (arq_light)      ← p5'in taşıyıcısı (§0.7)
        [KAPSAM DARALDI, talimat 2026-07-13: qd kolu (D-3/1 qd_ratio) TUR-3a'ya önalındı —
         kalan: bw log-ölçek (D-3/2) + sıfır-ağırlık fallback (D-3/3) + membership-invariant
         property-testi + üyelik-eşiği kalibrasyonları (qd_ratio placeholder'ları DAHİL)]
TUR-B   redundancy/kopya bütçesi (p2/p3 taşıma)
TUR-C   0.25k residual (overdrive + cadence)
SON     plateau-revisit (delivered-anchor) · MIN_CWND-kalibrasyonu · emergency-dwell'ler
```
Her tur: TEK-DEĞİŞKEN + beklenti-kaydı-önce + aynı-dekompozisyon-retest + p4/p5 smoke kapısı.
Beklenti-kayıtları BİRBİRİNE bağımlılıklarıyla yazılır (#7): hangi metrik hangi turda AÇILIR —
teslim-tavanı: 1a→43 (ölçüldü) · 1x-1b→~45-50 (loss 2→0 + temiz saat) · 3c→fizik-tavanı ~53 ·
p5→TUR-4'te geri · p2/p3-tırmanış→TUR-4+B.

## 3. Açık noktalar — KARARA BAĞLANDI (Abkarada, 2026-07-12)

1. **p5 sıralama → (a) KABUL:** PR-içi belgeli-regresyon, main'e merge YOK, meşru taşıyıcı TUR-4.
   (b) RED — tek-değişken disiplinini kırar + p5'in 4-kök kompozisyonunu maskeler.
2. TOMBSTONE_MAX=4096, K_tomb=8, K_wl → **ONAYLI** (placeholder; golden'a girmez) + 4-decade
   bellek-doğrulaması D-1/4'e işlendi (0.25kbps doluş ≈ 11 kayıt — tavan güvenli).
3. ProbeUp başlangıçları → **ONAYLI** (placeholder).
4. TUR-1x PR yapısı → **aynı PR, 1a→1x→1b, ÜÇ ayrı ölçüm-noktası.** 1x-sonrası ZORUNLU ara-ölçüm:
   ölçülen srtt ack_delay-A ile ~8.3s'ten gerçek-path-RTT'ye indi mi — **bu kaydedilmeden 1b
   BAŞLAMAZ** (1b cadence-eşiği temiz-saate muhtaç, düzeltme #4'ün kökü).

## 3.5 Bilinen-açık kabul-kapıları (KAYBOLMAZ-listesi; talimat 2026-07-12)

| kapı | durum | taşıyıcı | kabul-kriteri |
|---|---|---|---|
| **P5-GATE** | AÇIK — parazit söküldü (§0.7); karakterizasyon GÜNCELLENDİ (talimat 2026-07-13): "bimodal" DEĞİL, **yüksek-varyans süreklilik-SPEKTRUMU** (n=6 TUR-2a: {7,11,12,30,100,100}; r5=30 ara-mod). Şanslı uçta meşru taşıyıcı uçtan uca ✓ (dup-taşıyıcılı r4 c10=1.00; ARQ-taşıyıcılı r6 c10=0.97); açlık ucu btlbw floor-kilidi (bw-örnek semantiği + ProbeUp-yokluğu) | TUR-2b (tombstone) + TUR-3a (pencereler) + TUR-3c (ProbeUp) + TUR-4 (fuzzy) + TUR-B | **Kabul-kriteri (netleştirildi): spektrumun DAR-IQR'a ÇÖKÜŞÜ** (meşru mekanizmayla, yüksek uçta) — TUR-3c/4'ün gerçek testi bu. Her ara-smoke'ta p5 "yüksek-varyans" istisnasıyla: şanslı-uç ≥ önceki ∧ açlık-uç ≥ önceki, spektrum ayrı izlenir. |

## 3.7 İZLENEN-BORÇLAR (talimat 2026-07-13: açık listede tut, kapatınca işaretle)

| borç | doğuş | doğrulama/çürütme noktası | durum |
|---|---|---|---|
| **ŞANSLI-MOD-FREKANSI** | TUR-2b ölçümü: p5 şanslı-mod 0/9 çekiliş (P≈%3-5 kuyruk). Yapısal savunma ("2b sıcak-yolu hit=0 iken karar-yolu 2a-bit-özdeş → frekans değişemez → netem-varyansı") bir **ÇIKARIM, ölçüm değil** | **TUR-3a retest'i** spektrumu yeniden örnekler: şanslı-mod ~%30 bandına dönerse borç KAPANIR; yine seyrekse yapısal-savunma ÇÜRÜR → şanslı-mod-frekansı bağımsız sorun olarak açılır | AÇIK |
| **17-data-emisyonu izi (loopback)** | commit-C teşhisi sırasında görüldü: 10 birim / 17 data-send (tx=1550B; retx=0 dup=0) — fazla-emisyonların kaynağı belirsiz | AYRI teşhis (muhtemelen PreConverge/emisyon-yolu); kampanya-metriklerine oran-etkisi olabilir | AÇIK |
| **Little-kolu min_cwnd-maskesi** | 3a.5-α ölçümü: cwnd/srtt kolu fixture min_cwnd'i sayesinde BAĞLAMADI (yarı-2 öngörüm aşırı-temkin) — kol fiilen uykuda | MIN_CWND kalibrasyonu (sıra-SONU, D-4/2): kalibre min_cwnd küçülürse kol uyanır → srtt_t-paydalı davranış O ZAMAN ölçülür | AÇIK |
| packets_in_flight kayıp-sızıntısı (sampler) | TUR-2c muhasebe-incelemesi + review-workflow'u: kayıp-ilan edilen paket sampler'ın packets_in_flight'ını düşürmüyor; hit'siz üç yol HİÇ decrement etmez: (a) tombstone TTL-prune, (b) kapasite-eviction, (c) Duplicate-kökenli kayıp (tombstonesuz) → her gerçek-kayıp kalıcı +1 sızıntı → pipe-boş tespiti (first_sent/delivered_time yeniden-çıpalama) kayıplı koşuda bir daha tetiklenmez | TUR-3a (estimator pencereleri) — sampler semantiği zaten o turun masasında | AÇIK |

## 3.6 Abkarada'ya YENİ karar noktası (TUR-1x final ölçümünden)

§0.8-final: p5 açlık-modunun birincil kilidi **btlbw floor-kilidi** (ACK-starved tek-kümülatif-SACK
bw-örneği + ProbeUp-yokluğu) — TUR-2/TUR-3c'nin tam hedefi; sıra zaten oraya gidiyor. İlk-guard'lı
ara-bulgudaki "MIN_CWND öne çekilsin mi" sorusu final guard'la ZAYIFLADI (sunulan-birim kolu ikincil).
Önerim: **sıra korunur** (1b → 2a/2b/2c → 3a/3b/3c → 4); MIN_CWND SON'da kalır.

**KARAR (Abkarada, 2026-07-13): sıra KORUNUR — MIN_CWND SON'da.** Gerekçe: açlık-modu kökü
btlbw-semantiği + ProbeUp; MIN_CWND onların semptomu olabilir; erken çekme = tek-değişken kırar.
Aynı talimattan ek kararlar: TUR-1 PR merge edildi (#23, squash `2379b0b`; p5 belgeli-regresyon
karar-a + §3.5 izleme); ARP ön-koşulu r5 §J'ye işlendi; ZM-2 aktif-koruma örneği spec §16.2'ye
iliştirildi; **TUR-2a GO** (yeni PR; beklenti-kaydı: açlık-modu btlbw-kilidine etkisi ölçülecek;
smoke'ta iki p5 modu AYRI izlenir: şanslı ≥ önceki, açlık ≥ önceki).
