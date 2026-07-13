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

**KARAR (Abkarada): (a) KAPSAM-KİLİTLİ** — yalnız tail/tek-paket timer-SACK muafiyeti (R1'in
tail-simetriği); tam adaptif ACK-decimation §3.3-E-FULL'e (kampanya-öncesi borç). Adaptivite-şartı:
muafiyet-gecikmesi MUTLAK-ms OLAMAZ → formül: sessizlik ≥ K_TAIL×son-varış-aralığı (kanal-ölçülü,
boyutsuz K_TAIL=2 [CAL {1,2,4}]; fast'ta QUIC-max_ack_delay-sınıfı davranış formülden doğar;
sub-kbps'te aralık≥cadence → devreye girmez). (b) RED (EAI-bölme+maske), (c) RED (3b yarım kalır).

**p5-KAYMA TAHMİNİ (talimat; ölçümden ÖNCE):** tail-muafiyeti p5'te (100kbit; varış-aralığı
23-45ms → tail-SACK ~46-90ms'de) SACK-EMİSYON sayısını artırır → %80-kayıplı dönüş-yolunda
hayatta-kalma ÇEKİLİŞ-sayısı artar → **dağılım YUKARI kayar tahmini**: açlık-uç {11-24}→{15-35}
bandına, şanslı-frekans ↑ (%50-67 sınıfı kalır ya da artar); temiz-1k +0/+1 (son-birim ACK'i erken).
Bu iyileşme BU turda ölçülür ki 3c'ye YANLIŞ atfedilmesin. Dönüş-bütçesi riski düşük (65B×birkaç
ek SACK ≪ %25-bütçe).

## 0.18 TUR-3b commit-C FİNAL (rev2) — teşhis-kollu ikinci-iterasyon + kapı GEÇİLDİ

**İlk-C b3-matrisi KIRMIZI:** p5 {0,10,10,56,100-yavaş,100-yavaş} (açlık-uç 0 < 11 İHLAL).
p5-kayma tahminim ("dağılım yukarı") bileşik-commit için YANLIŞTI. **İki tek-değişken teşhis-kolu**
(working-tree toggle): yalnız-taban {12,100@c10=.99,100@.91} NORMAL; yalnız-tail {5,8,11} DÜŞÜK →
birincil şüpheli TAIL-yolu: her tail-SACK record_ack_emit'le CADENCE-saatini sıfırlıyor →
garantili-ritim kümülatif-SACK'ler erteleniyor → %80-kayıplı dönüşte hayatta-kalan SACK azalıyor
(r5=0 bileşik-zincir). **rev2 (5ada7a8):** tail-SACK KENDİ ritim-sayacıyla (spam-korumalı) saf-EK
emisyon; cadence/freq sayaçlarına DOKUNMAZ.

**rev2 SONUÇ — kapı GEÇİLDİ:** p5×6 = **{11, 22, 92, 99, 100, 100}** (üst-uç 4/6; sıfır YOK;
açlık {11,22} ≥ önceki ✓). TARİHİ koşu r4: **100 birim SAF-DATA (retx=0 dup=0, oran 1.00×) → 92
teslim** — tail-SACK'lerin taşıdığı en temiz p5 akışı (recovery-trafiği olmadan). p4 da en-temiz
hali: 100/100, c10=1.00, **oran 1.00×** (liveness-dup 283→0 — tail-SACK gereksizleştirdi).
Temiz-1k 43 (B'nin belgeli-değeri; 3c-hipotezi duruyor). 0.25k korundu (monoton, 20/20).
Not: c10-varyansı şanslı-koşularda büyüdü (başlangıç-SACK şansı; c120 teslimleri yüksek) —
3c/ProbeUp sonrası yeniden okunur.

## 0.19 TUR-3c m1 ÖLÇÜM-KAYDI — dönüş-kolu ✓, tırmanış-kolu YANLIŞLANDI (hedef miskalibre)

**Mekanik (hepsi kontratça):** giriş t=91 (periyot+qd-histerezis+backlog ✓), gain uygulandı
(rate 102→127 ✓), kuyruk-kanıtı qd 1.23→2.49s ✓, **dürüst-abort** qd_ratio 814‰>500‰ ✓
(probe=0/1/0/0). btlbw 102'de KALDI — ve kalması DOĞRU:

**Dekompozisyon — 3b-devri hipotezin "kanal 125" değeri WIRE/PAYLOAD karışımıydı:** 1kbit =
125 B/s WIRE; birim = 256B payload + 50B başlık (28 UDP/IP + 22 kama) = 306B wire → payload-tavanı
= 125×256/306 ≈ **104.6 B/s**. btlbw=102 = tavanın %97.5'i → app-limited-kilit 3b-drain'de ZATEN
kalkmıştı; ProbeUp'ın bulacağı headroom fiziksel olarak yoktu. 120s teorik tavan = **49 birim**;
50-53 hedefi matematiksel imkânsızdı (miskalibrasyon 125/104.6 oranından geliyordu).

**Sonuç: temiz-1k 43 → 45** (49 tavanın %92'si; kalan = cold-start ~4s + kesim-anı uçuştaki ~4
birim). +2'nin mekanizması: probe'un kurduğu ayakta-kuyruk sonraki DrainRtprop dip'lerinde
(rate 76 < 104.6) wire'ı DOLU tutuyor — eski 43'ün "son-30s kaybı"nın gerçek yüzü boş-kuyruklu
DrainRtprop idle'ıymış (bkz. maske-soyma #4'ün devamı; kilit değil, faz-dip'i).

**Hüküm:** m1 doğrulama-şartının ≥45 kolu SAĞLANDI; tırmanış-kolu hedef-miskalibrasyonla düştü
(ProbeUp-kusuru DEĞİL — genişletme yok, talimatça). ProbeUp temiz-kanalda görevini "headroom yok"
ölçümüyle yaptı; asıl sınavı m2/p5 (açlık-modu btlbw-floor-kilidi, §3.6'nın hedefi).

## 0.20 TUR-3c m2 ÖLÇÜM-KAYDI — mekanizma-kanıtı GELDİ (btlbw-floor kaçışı); spektrum üst-uçta örneklendi

**p5×6 = {100, 100, 100, 100, 100(c10=1.00), 81}** — açlık-bandı ({11,22} sınıfı) bugün HİÇ
örneklenmedi (kernel-RNG çekilişi; P≈(2/3)⁶≈0.09, seyrek ama meşru). Bant-kayması tahmini
({11,22}→{25-60}) bu yüzden TEST EDİLEMEDİ — yerine daha güçlüsü geldi:

**p5r6 = m2 mekanizma-kanıtı (trajektori):** btlbw **31-kilidinde 68s** (t=8.5→76.5; Startup
rate=89 iken bile floor'da — ACK-açlığı tek-kümülatif-SACK örnekleri, §3.6'daki birincil-kilit).
DrainRtprop→Steady sonrası ProbeUp t=78.5 GİRDİ → btlbw **78→191→463→591 (7.6×)** → SUCCESS-çıkış
t≈102 (probe=1/0/0/0) → teslim 21→**81** (saf-data, retx=0 dup=0). Beklenti-kaydının "ProbeUp
btlbw'yi 31-kilidinden RAMP'ler → yapısal kaçış" cümlesi birebir ölçüldü.

**Dürüst-atıflama:** Diğer 5 koşu (hepsi 100/100) probe-NÖTR — Steady'ye hiç ulaşmadılar
(PreConverge/RecoverArq-baskın; giriş Steady-şartlı, dal-2 sınırı aynen). 5×100'ün kaynağı 3c
DEĞİL — 3b-rev2 tabanının bugünkü şanslı çekilişleri (sender-yolu deterministik: r1/r3/r4/r5
tx=29432 birebir; varyans yalnız hangi-SACK-hayatta-kalır'dan).

**Şart-b (c10-varyans) DOĞRULANMADI:** c10 = {0,.01,0,.01,1.00,.01} — hâlâ iki-uçlu. İlk-SACK-şansı
yapısal kalıyor (hızlı-mod koşularında probe hiç koşmadı → etkileyemezdi). Tahmindeki dürüst-dal
aynen: **P5-GATE 3c'yle KAPANMAZ** — dar-IQR kabulü TUR-4 (fuzzy/arq + dönüş-yolu) sonrasına.

**p4-smoke ✓:** 100/100, c10=1.00, saf-data 1.00×, probe-nötr (hızlı kanalda kuyruk anında
boşalır → backlog-kapısı girişi doğru engelliyor: talep zaten karşılanıyorken prob gereksiz).

## 0.21 TUR-3c SONRASI KARARLAR (Abkarada, 2026-07-13) + byte-cap bağımlılık-raporu

1. **PR #30 MERGED** (squash d0d14da). Üç beyanlı-sapma meşru bulundu.
2. **temiz-1k = TAVANDA-KAPALI VAKA:** 49-payload-tavan dekompozisyonuyla dosyalandı. Bundan
   sonra temiz-1k "≥45 korunur" REGRESYON-KAPISI'dır, ilerleme-metriği DEĞİL (fiziksel tavan).
   İlerleme-metriği p5/lowband/aralıklı-profillere kayar. m1-yanlışlanması motor hatası değil
   ÖLÇÜT hatasıydı (wire/payload karışımı — önceki turların beklenti-hatası).
3. **byte-cap bağımlılık-raporu (talimatla):** ProbeUp gain-mekaniği byte-cap'e BAĞLI DEĞİL —
   kuyruksuz kanalda Little-kolu tavanı = 2×btlbw×(rtprop/srtt) ≈ 2×btlbw > 1.25×btlbw (gain
   çarpmıyor). ANCAK prob kuyruk kurdukça (srtt>1.6×rtprop) Little-kolu probu 1.25×'in ALTINA
   kırpabilirdi — bugün bunu MIN_CWND=4×MTU maskesi engelliyor (sub-kbps'te cwnd=4800B ≫ 2×BDP;
   m1'de ölçüldü: srtt=5.86s, rtprop=3.06s iken rate=127 kırpılmadı çünkü cap=4800/5.86=819).
   → SONUÇ: byte-cap/Little faz-davranışı işi MIN_CWND-kalibrasyonuyla AYNI mekanizma-ailesi →
   **SON fazına devir** (D-4/2 yanına); TUR-4-öncesi ayrı commit GEREKMEZ. ProbeUp-stabilizasyonu
   byte-cap'i beklemez; MIN_CWND kalibre edilirken ProbeUp×Little etkileşimi ölçüm-noktası olur
   (§3.7 Little-borcu satırıyla birleşik).
4. **TUR-4 GO** (fuzzy): bw log-ölçek + sıfır-ağırlık fallback (arq_light) + membership-invariant
   + qd rtprop-göreli üyelik. Hedef: p5-determinizmi (dar-IQR — P5-GATE son-mührü). P5-GATE bu
   fazda dürüst-açık: ProbeUp açlık-KAÇIŞINI çözdü (p5r6 7.6×), p5-DETERMİNİZMİNİ çözmedi.
5. **Gerçek-dünya doğrulama üç-eksen planı → `docs/design/dv-plan-v2.md`** (DV-1 bant-decade'leri,
   DV-2 aralıklılık, DV-3 rejim-geçişi; sıra TUR-4 SONRASI; kampanya-öncesi kapanır). Açık-uyarı
   aynen kayda: DV-2/DV-3 kapanana kadar "gerçek-dünyaya hazır" İDDİA EDİLMEZ (yalnız
   "sürekli-bant kanıtlı" denir).

## 0.22 TUR-4 TASARIM+WORKFLOW KAYDI (fuzzy; ölçüm-bölümü ölçümden sonra eklenecek)

**Kapsam (onaylı):** bw log-ölçek + sıfır-ağırlık fallback (arq_light) + membership-invariant.
**Beklenti-kaydı:** scratchpad `tur4-prediction.md` — İKİ ölçüm-öncesi dürüst-revizyonla:

**Ön-bulgu (kod-doğrulama, ölçümden önce):** arq.plan() yalnız arq==0'da baskılar; payda-0 tek
arq=0 üreticisi; bugünkü p5 lppm'leri (0-77k) loss.low=1000 bölgesinde → R1 zaten ateşliyor →
**ölçülmüş BREAKS-dejenerasyonu ESKİ-ERA'dandı** (lifetime-ratchet lppm; TUR-2a pencereleme onu
seyrekleştirmiş). Fallback = yapısal-sigorta; dar-IQR bu kapsamdan BEKLENMEZ (dürüst hedef-revizyonu).

**Workflow-doğrulaması (wf_04b67f3d, 3 lens, 17 bulgu / 5 HIGH) — tasarıma işlenenler:**
1. HIGH: önerilen log-eşikler (1k/4k/50k/100k) bw.low bandını 37× daraltıp sr1-throttle/
   emergency/compress'i BEYANSIZ yeniden yazıyordu → **RE-DOMAINING ONLY karara bağlandı:**
   kesme-noktaları (50k/100k/150k/200k) korunur, decade-ayrıştırma kalibrasyonu DV-1'e (kanıtla).
2. HIGH: naif mantissa (x−2^e)·256>>e, e≥56'da u64 taşırır (release non-monoton / debug panik =
   ADR-17 ihlali) → taşmasız formül: e≥8: (x>>(e−8))&0xFF; e<8: (x<<(8−e))&0xFF (EXACT).
   x=0 guard (ilog2(0) panik). Monotonluk kanıtlı; e>8'de strict-değil (~2-4‰ merdiven, dokümante).
3. HIGH: fallback çıktısı R1-baskın çıktıyla BAYT-ÖZDEŞ → gözlemlenemez → no_rule bayrağı +
   diag_fuzzy_no_rule + TRAJ fnr= eklendi (kanıt-sayacı ilkesi).
4. MED: covers_domain KESİN > (trap kenarları dışlayıcı; ≥ dokunma-noktasında tek-nokta
   zero-weight deliği bırakır) + eşikler AYNI const fn'den (offline-float hardcode yasak).
5. Beyanlar: arq==0 fec-dominant baskılama-dalı (§9) yapısal-erişilmez (arq consequent'leri
   ≥arq_light); fec_zero fallback'i kapsama-deliklerinin kalıcı-politikası (ARQ-ekseni muhafazakâr,
   FEC-ekseni statüko — DV-2'de yeniden okunur).

**Sonuç (implementasyon):** kapılar 30 suite ✓ clippy 0 ✓; **golden'lar BİT-ÖZDEŞ geçti** —
kesme-noktası-koruma o kadar temiz ki davranış-sınıfı yeniden-üretim bile gerekmedi (175k
rampa-değeri log-domain'de de tam 500). Commit 37f8a46.

### §0.22-ÖLÇÜM (tam-matris, TUR-4)

**p5×6 = {17, 29, 62, 100, 100, 100}** (önceki-era {11,22,92,99,100,100}). Smoke-kuralı: açlık-uç
17≥11 ∧ 29≥22 ✓; şanslı-uç 100≥100 ✓ (3/6). **Dar-IQR GELMEDİ** — ölçüm-öncesi dürüst-tahmin
aynen: fuzzy bu spektrumda karar-yolunu değiştirmiyor; determinizm-kilitleri fuzzy-DIŞI.

**fnr-KEŞFİ (tahminden sapma → dekompozisyon):** fnr=102 İKİ hızlı-mod koşuda (r2 c10=0.97,
r5 c10=0.95), t≈19.2'de tek-patlama (iki koşuda BİREBİR aynı — deterministik desen); diğer 4
koşu + p4 + 1k + 0.25k'da fnr=0. Dekompozisyon: pencereli-lppm 20k→41k→125k tırmanıp örnekler-
arası ANLIK ≥300k'yı aştı — **W_loss payda-çürümesi artefaktı** (pencere-dışı decay gönderim-
paydasını Confirmed-olaylardan hızlı düşürüyor → oran spike → 1s içinde 0'a döner). Ön-bulgum
"lppm 0-77k bandı payda-0'a girmez" SON-DEĞER okumasıydı; geçişler GİRİYOR. Davranışsal etki
bugün SIFIR: patlama-anı state=RecoverFwd ∧ pend=0 → arq tüketilmiyor (lens-bulgusuyla tutarlı)
→ çıktılar eski-kodla bayt-özdeş (spektrum-eşleşmesi bunu doğrular). Sayaç tam işini yaptı:
görünmez bölge artık ölçülüyor. DV-2'de (aralıklı-rejim kopma-dönüşleri) bu bölgeye pend>0 ∧
RecoverArq ile girilirse fallback İLK KEZ davranışsal-canlı olur — izlenecek.

**Kapılar:** temiz-1k **45** ✓ (regresyon-kapısı ≥45; karar-2 çerçevesi); 0.25k **bit-özdeş**
(sıfır-tick=4, flap=0, son-btlbw=27); p4 100/100 saf-data 1.00× c10=1.00. Kesme-noktası-koruma
kanıtı: tüm profiller eski-davranışla uyumlu, golden'lar bit-özdeş.

**TUR-4 teslimi (dürüst):** yapısal-sigorta (arq=0 artık İMKÂNSIZ + gözlemlenebilir) +
log-domain altyapısı (DV-1 kalibrasyonuna hazır) + invariant-katmanı + fnr-keşfi.
**P5-GATE karar-noktası (Abkarada'ya):** dar-IQR fuzzy'den gelmedi ve gelemezdi (ölçüldü);
kalan determinizm-kilitleri: (i) ilk-SACK-şansı — dönüş-yolu işi (§3.3-E-FULL ailesi),
(ii) conf-LOW RecoverFwd dilimi (derin-açlık {17,29} sınıfı). P5-GATE dar-IQR kabulü bu iki
işten sonrasına kalır; öneri: §3.3-E-FULL zaten kampanya-öncesi borç — P5-GATE'i ona bağla.

## 0.23 AYRIŞTIRMA-KAYDI: ilk-SACK-şansı kilidi (TEŞHİS — fix YOK, karar Abkarada'da)

**Yöntem:** 3 bağımsız lens (receiver-cadence / sender-kader-zinciri / trajektori-kanıtı,
wf_b0b46caa) + elle çapraz-hakemlik. Mekanizmada üçü BİRLEŞTİ; fix-etiketi sentezi aşağıda.

### Mekanizma (kod-zeminli, ölçümle doğrulanmış)
İki-fazlı ÇİFT-KARARLI dönüş-yolu açlık döngüsü; "ilk-SACK-şansı" rafine hali:
**KAFES-KANITI:** p5×6 ilk-teslim zamanları {≈0.03, ≈0.03, 8.5, 25.5, 25.5, 34.0}s =
0.03 + k×8.39s, k∈{0,0,1,3,3,4} — ACK_CADENCE_FLOOR (receiver.rs:91-92; RETURN_BW_PRIOR=31
statik, :85) VE PreConverge pacing-periyodu (267B/31B/s≈8.6s) İKİSİ de deneme-yoğunluğunu
~1/8.4s'e kilitliyor. p=0.2 hayatta-kalmayla geometrik: P(k=0)=0.2, gözlenen 2/6 ✓.
- **Faz-1 (piyango):** İlk SACK cadence-muaf (receiver.rs:220) ama muafiyet EMİSYONLA tükenir
  (teslimle değil; first_ack_sent set-only :270-273, HİÇ sıfırlanmaz) → ölürse denemeler
  8.39s-kafesine düşer.
- **Faz-2 (tohum-bimodalitesi — asıl amplifikatör):** bw-örneği interval = deneme-GECİKMESİ
  olur → tohum = deliv×267/t ≈ kendi-pacing-tabanı ≤31 B/s (6/6 koşuda aritmetik doğrulandı)
  vs deneme-1-hayatta → 267/RTT ≈ 10.000 B/s. **325× tohum-uçurumu, tek 65B SACK'in kaderiyle.**
  Açlık-tohumu DAİRESEL-ölçüm: motor kanalı değil kendi açlığını ölçüyor.
- **Faz-3 (kendini-kilitleme):** rate=btlbw-pacing → varış-oranı → SACK-arzı (freq ACK_FREQ_N=2
  varış-saatli; tail 2×gap varış-saatli; tek şans-bağımsız taban 8.39s-cadence) → ×0.2 →
  hayatta-kalan ~1/20-40s → bw-örnekleri 31-279 → rate düşük kalır. İki dal arası arz-oranı
  ≈120× = bistabilite. Kader-penceresi: [t≈30ms; t≈35s]; final teslim = ENTEGRE hayatta-kalma
  (r4: en geç çıkış 34s ama 12 sağ-SACK → 62).
- Alternatifler sayısal DIŞLANDI: fnr=0 (açlık-koşularında), emisyon-durması yok (pacing-hızında
  monoton), false-loss fırtınası yok (lppm≤55k, unloss=0 — TUR-2 fix'leri tutuyor).

### Soru-1: dar-fix mi §3.3-E-FULL mü? — CEVAP: İKİSİ DE SAF HALİYLE DEĞİL; üç-bacaklı orta-paket
- **E-FULL-TAM (ölçülen return_bw→cadence) P5-GATE-anahtarı DEĞİL:** (i) açlık-havzasında
  BOOTSTRAP-DAİRESEL — sağ-çıkan dönüş-trafiği yokken return_bw ölçülemez, önsel kalır, kilit
  sürer; (ii) impaired-dalı (N↑=daha az ACK) kayıplı-ama-geniş dönüşte TERS-YÖN; (iii) kilit
  t≈0'da, ölçüm tanımsızken kurulur. E-FULL'ün gerçek faydası AYRI problem: çıkış-SONRASI
  sürünme + hızlı-kanal cadence-dürüstlüğü (400× arz) → kampanya-öncesi borç OLARAK KALIR.
- **Eski dar-fix listesi de yetmez:** tail-simetriği ZATEN yapıldı (3b-rev2) ve bimodaliteyi
  çözmedi (ölçüldü); muafiyet-genişletme tek başına kafesi değiştirmez.
- **Mekanizmanın talep ettiği ÜÇ BACAK (mekanizma-düzeyi; tasarım ayrı tur):**
  1. **Dönüş-yönü deneme-yoğunluğu:** handshake/yeniden-başlama döneminde SACK-TEKRARI
     (aynı kümülatif-SACK'in R dekorele kopyası; R=4→P(hızlı-tohum) 0.2→0.59, R=8→0.83 —
     EN BÜYÜK TEK KALDIRAÇ) + muafiyetin sessizlik-epizodu-başına yeniden-kurulması
     (first_ack_sent → epizot-tabanlı; DV-2 yeniden-başlamasını da AYNI hamle çözer).
     Maliyet sınırlı: SACK=65B; INV-RATE-1'in dönüş-simetriği ruhu.
  2. **§3.3-E MİNİMAL-TABAN (spec'te ZATEN DONUK cümle, engine-spec ~:355-359):** cadence'in
     DATA-varış-oranıyla hızlanması — FULL'ün ölçülen-return_bw kısmı DEĞİL; donmuş minimal
     kısmın uygulanması.
  3. **Sender tohum-hijyeni:** gap-sonrası ilk-SACK bw-örneğinin dairesel-ölçüm sınıfı
     (interval=deneme-gecikmesi → kendi-tabanını ölçme) — örnek-geçerlilik kapısı ailesi;
     yanına plato-yanılgısı (izole-SACK örnekleriyle 3-olayda "yakınsadı") ve
     supersede-sansürü (açlıkta ACK'lenen byte'ların örneksiz kalması) sender-taraf eşlikçileri.
- Sıra-imasi: bacak-1+2 receiver-taraf tek-tur olabilir; bacak-3 estimator-turu (ZM-kurallı).

### Soru-2: DV-2 köküyle aynı mı? — EVET, mekanizma-ailesi AYNI (üç lens hemfikir)
Ortak çekirdek: seyrek-varış rejiminde SACK-yeniden-deneme arzının statik-önsel cadence'e
kilitlenmesi + tek-atımlık/emisyon-tüketimli muafiyet. DV-2'ye ÖZGÜ ağırlaştırıcılar (kod-zeminli):
(a) kopmada PreConverge'e YENİDEN GİRİLMEZ (sender.rs:429-436 — srtt varsa Idle→Startup, bayat
estimator'la); (b) first_ack_sent sıfırlanmaz → yeniden-başlama muafiyetsiz; (c) last_gap
zehirlenmesi (receiver.rs:150-152: ilk varış gap=kopma-süresi yazar → tail-eşiği 2×kopma —
tail tam gerektiği pencerede ölü); (d) cadence-faz rehinesi (sessizlikte timer yeni-bilgi-şartsız
emit edip last_ack_ns'i sıfırlar → dönüş anında kalan-faz rastgele).
**AMA p5'in piyango-ekseni (dönüş-kaybı) DV-2'nin temiz-pencereli kopmasında YOK → P5-GATE
yeniden-kanıtı fix-turu birleşse bile p5'in KENDİSİNDE koşulmalı.**

### Yan-bulgular (kayda; ayrı işler)
- **Dup-force SACK yolu ÖLÜ-YOL** (iki lens bağımsız buldu): sender her yeniden-emisyona TAZE
  seq atar (sender.rs:1267-1268) → receiver engine-kopyasını "duplicate" göremez →
  RX_DUP_SUPPRESS erişilmez; receiver'ın "SACK'im öldü" kanıt-kanalı YOK (spec §3.3-B copy-stop
  referansı mevcut kablolamada kopuk). Bacak-1 tasarımının girdisi.
- r4-dersi: kader-metriği çıkış-zamanı değil entegre-hayatta-kalma — fix-turu ölçüm-tasarımına girer.

## 0.24 TUR-5 ÖLÇÜM-KAYDI (bacak-1+2, R-matrisi) — KAFES ÖLDÜ; dal-B DOĞRULANDI; temiz-1k KIRMIZI-DUR

**Seçim-serisi (p5×6 × R∈{4,8}):** R=4 {33,100×5}, med-ilk-teslim 9.49s (K-1a ✗);
R=8 {14,30,100×4}, med-ilk **2.14s** (K-1a ✓) → **çok-kriterli seçim R=8**.
**Doğrulama-serisi (R=8):** {55,63,100×4}, med-ilk 8.445s — literal-eşiği (8.4s) **45ms aştı**
(dürüst-beyan; 12-koşu-birleşik 5.30s ✓; kapı-hükmü Abkarada'da). **K-1b 12-koşu KAPISI ✓:**
>16.8s = 3 ≤ 4 (kesin-binom %95-üst). **K-4 ✓ tüm koşularda** (kredi-invariant canlı;
tx-oran ≤1.21). K-2 ✗ (min 14) → K-3 ✗.

**KAFES-ÖLÜMÜ (K-1a mekanik-kanıt, alt-kapı):** 18 koşunun TÜM ilk-teslimleri kota/kafes
TAKVİM-noktalarında ({0.03-0.04, 2.14, 4.24, 6.35, 8.44, 10.54, 14.73-74, 23.14, 39.95, ...}
— r3-aritmetiğinin parmak-izi, nokta-nokta); eski k×8.39s-kümelenmesi YOK; medyan 25.5s →
2.1-8.4s bandı (3-12×). r4_3'ün 14.74s'i (kota-4 + kafes-1 = 5. deneme) takvimi tek-koşuda kanıtlar.

**Dal-B DOĞRULANDI (r3 §4 şablonu):**
```
Bacak-1 alt-kapısı: ACCEPTED   (kafes-ölümü + K-1b + K-4; K-1a doğrulama-serisi 45ms sınır-notuyla)
P5-GATE: DEFERRED — bacak-3 kanıtı gerekli
Tam mühür: yalnız K-2 ∧ K-3 sonrası
```
Trajektori-kanıtı ({14,30,33,55,63} koşuları): kurtuluş TAKVİMDE geldi ama tohum =
267/t_kurtuluş formülüyle floor-yarı-kanal bandında (18-63 B/s; v8_5: 4.24s-ERKEN kurtuluşta
bile tohum 63 → 63/100) VE **YENİ-BİLEŞEN: RecoverArq-YAPIŞMASI** — %80 dönüş-kaybında pending
sürekli-yenileniyor → faz-önceliği Steady/ProbeUp'a HİÇ düşmüyor (probe=0/0/0/0; r4_6 41-birim,
r8_3 38-birim emit) → ProbeUp-köprüsü yapısal-erişilmez. Bacak-3 teşhis-girdisi: tohum-hijyeni
+ RecoverArq-gölgeleme İKİLİSİ. Kuyruk-çekilişleri {14, v8_6-56.7s} ≈ binom-beklentisi (2/18).

**Smoke:** p4 ✓ 100/100 saf-data 1.00× (jitter-kuyruğu 3 meşru-epizot; invariant ✓).
**temiz-1k 44/100 — KIRMIZI (kapı ≥45): DUR-noktası.** Dekompozisyon: dönüş-bütçesi DEĞİL
(27 SACK × 65B / 120s ≈ 14.6 B/s ≪ 125 kapasite) — zengin erken-SACK-arzı Startup-PLATO
tespitini 24.5→37.5s'e uzattı (+13s × gain 2.885 → overfill-kuyruğu büyüdü) → Drain 120s'e
dek çıkamadı → 50-emit/44-teslim (6 uçuşta; eski 49/45-4-uçuşta). KÖK spec §16.2'de ZATEN
işaretli yarım-fix: `startup_plateau` sayacı ack-OLAY-tabanlı (gereken-sınıf "RTT-round
delivered-anchor", TUR-A satırı) — arz-zenginleşmesi olay-sayacını kaydırdı. Tavan-çerçevesi:
49-tavanda 44 = %90 (önceki %92); kayıp cold-start-fazında, mekanizma-dışı.
**Fix-seçenekleri (karar Abkarada'nın):** (a) kapı-revizyonu (44 kabul; startup_plateau
yarım-fix'i zaten SON-listesinde); (b) startup_plateau delivered-anchor fix'i AYRI-tur
(spec-satırı hazır); (c) kota'yı dar-banda kısıtlama (mekanizmayı sulandırır — ÖNERİLMEZ).

**İKİNCİ KIRMIZI — 0.25k SEÇİLEN-R(=8) İLE 13/20:** R=4-kolu 20/20 + özet-metrikler ESKİYLE
ÖZDEŞ (4/0/27, tx=5660); R=8-kolu 13/20, btlbw-son 27→13, sıfır-tick 4→25 (tx aynı — sender-
emisyon değişmedi; bozulan ACK-fiziği). Dekompozisyon: +7 kota-kopyası @T_SPACING=2.1s ×65B =
31 B/s ≈ dar-dönüş kanalının TAMAMI ~14.7s → SACK-kuyruklanması → ack-delay/estimator
zehirlenmesi 300s'e yayılıyor. r3'ün "dar-dönüşte doğal-seyrekleşme" gerekçesi 0.25k'da YANLIŞ
(T_SPACING = tam-kanal-serialization = doyum); r1-lens'in kota-bütçe uyarısının R=8-ölçekli
geri-dönüşü. **R-SEÇİM ÇIKMAZI:** R=8 K-1a ✓ ama K-5 ✗; R=4 K-5 ✓ ama K-1a ✗ (9.49s, tek-seri)
— çok-kriterli kural ("kota-yükünü artıran kol seçilmez") iki kolu da tek-başına bırakmıyor.
Fix-uzayı (karar Abkarada'nın): (α) T_SPACING'in kanal-türevli tabanı — örn.
max(T_SPACING, kendi-varış-gap'i sınıfı bir taban): dar-dönüşte kota yayılır, p5'te 2.1s kalır
(YENİ-TASARIM, onaysız girilmez); (β) R=4 seçimi (K-1a farkı 1.1s, 6-koşu-örneklem gürültüsü
payı; 12-koşu R=4-verisi yok); (γ) her ikisi + startup_plateau-fix'i ayrı-tur paketlemesi.

### §0.24-KARARLAR (Abkarada, 2026-07-14 — çift-KIRMIZI DUR'unun çözümü)

1. **β ÖNCE:** R=4'e 6 ek koşu → 12-koşu K-1a (per-koşu P(>8.4s)=0.8⁴≈%41 → n=6 medyan-kapısı
   mekanizma-mükemmelken ~%30-48 yanlış-eler — PATCH-2 dersinin K-1a'ya dönüşü). 12-koşu-medyanı
   ≤8.4s → R=4 seçilir (K-5 zaten ✓) → PR. Medyan ≥9s → mekanizma-bilgisi (per-deneme-sağkalım
   <0.2) → DUR, α-tasarım-sorusu Abkarada'ya.
2. **α ŞİMDİ DEĞİL** + sensing-ön-şartı kayda: receiver varış-ritminden p5-açlık(sender-throttle)
   ile 0.25k'yı(serialization) AYIRT EDEMEZ (ikisi de gap≈8.6-9.9s) → varış-türevli T_SPACING
   p5-kotasını da yayar → K-1a yapısal bozulur. Gerçek çözüm E-FULL ölçülen-return_bw (§3.7
   borcu — "statik-önsel-türevi" dürüst-etiketi tam bu gün içindi). **γ RED** (tek-değişken ihlali).
3. **temiz-1k 44 = BELGELİ-REGRESYON** (3b-commit-B emsali), ŞARTLA: startup_plateau
   delivered-anchor turu HEMEN sıradaki iş (süresiz-erteleme YOK — regresyon-kapısının anlamı
   erimesin). β geçerse PR: R=4 + 1k-44-belgeli + plateau-turu-sıradaki taahhüdü.
4. **RecoverArq-yapışması = bacak-3'ün İKİNCİ-BAŞI:** ProbeUp-köprüsü p5'te yapısal-gölgede.
   Bacak-3 tasarım-notu İKİ-başlı: tohum-hijyeni (estimator) + RecoverArq-gölgelemesi
   (faz-önceliği). İkisi sender-taraf, ayrı-tur, D0-notu-önce.
5. Ekleme-2 geri-çekilmesi KABUL; count=0-deliği DV-2 açık-riski olarak kalır.
Sıra: β → (geçerse) PR → plateau-turu → bacak-3 D0-notu → DV-1.

**Süreç-kaydı:** Ekleme-2 (kota-yanış şartı) implementasyon-öncesi çelişki-tespitiyle GERİ-
ÇEKİLDİ (K-1b retry-takvimini bloklardı — not §2 revizyonu; count=0-deliği DV-2 açık-riski
aynen). Bacak-2 yapısal-gölgeleme dürüst-bulgusu: K_ARR=2'de accel-ritmi freq/tail'le örtüşük;
ayrık-katkı dar-pencerede (ölçüldü: accel=0-4/koşu).

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
| **Statik return-prior → ProbeUp-periyodu tüketicisi** | TUR-3c: giriş-periyodu 4×ack_epoch; EAI'nin statik önseli (31 B/s worst-corner) hızlı kanalda epoch'u ~2.1s'e mıhlar → probe-periyodu ≥8.4s'e sabitlenir (kanal-türevi değil, önsel-türevi) | §3.3-E-FULL (ölçülen return_bw) gelince EAI daralır → periyot kanal-zamanlı olur; E-FULL kampanya-öncesi borcun YENİ tüketicisi olarak kaydedildi | AÇIK |
| **W_loss payda-çürümesi lppm-spike artefaktı** | TUR-4 fnr-keşfi: pencere-dışı decay paydayı olaylardan hızlı düşürüyor → pencereli-lppm anlık ≥300k spike (gerçek kayıp %30 DEĞİL) → fuzzy loss.low=0 bölgesine ~1s'lik sahte-giriş | Bugün zararsız (fallback + fnr-sayacı ile görünür; patlama-anı arq-tüketilmeyen state'te). DV-2 aralıklı-rejimde pend>0∧RecoverArq ile çakışırsa fallback davranışsal-canlı olur → orada yeniden oku; gerekirse payda-tabanı (min-sends) fix'i AYRI karar | AÇIK |
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
