# b2-rtt-hijyen-v1 — Aralıklı-RTT-Kirliliği: Kabul-Kapısı + n-Koruması [TASLAK r2 — ONAY BEKLİYOR]

> **r1→r2 (doğrulama-workflow'u wf_a7ff426a — 2 lens; hesapsız-fix 6. yakalama):** r1'in
> "F-B/F-C kapsanır" iddiası YANLIŞTI: (1) **eşik-altı kör-bant × n-amplifikatör MERDİVENİ** —
> eskalasyonlu kopmalar (5.5s→15s: eşik 5.63'ün altı → breach-yok → örnek girer, n=21 → srtt
> 1.02→5.23 → eşik 15.7 → 15s de girer...) iki-adımda p6b-hasarını stale=0'la geri getirir;
> KÖK: eşiğin srtt-kolu korunan-değişkenden türer + n-amplifikatör eşik-altı örneği reseed'e
> çevirir → **F1'e n-KORUMASI eklendi (§1-B)**. (2) **blackout-bootstrap kill-switch**: ilk-örnek
> zehirse (srtt-None + kopma-bekleyişi) kapı doğuştan-ölü → BEYANLI-KALINTI + vektör (§5-S4).
> (3) S3×§4 iç-çelişkisi giderildi (tek-anlatım: muafiyet YENİ-kod; p5-analizi yeniden — §4).
> (4) RED-yan-etkileri pinlendi (drain_rtprop_qualifying dahil); breach-çıpası tanımlandı.

> **Statü:** D0-TASARIM-NOTU (B2, DV-3-öncesi dar-tur — Abkarada karar-2/3). ONAYDAN SONRA kod.
> **Kapsam-sınırı:** estimator-dar — `sender.rs` ack-yolu (rtt.observe-öncesi kapı) + `rtt.rs`
> (dokunuş gerekirse minimal) + testler + TRAJ-sayacı; **başka her şey = İHLAL, DUR.**
> Zemin: 3-soru teşhis-workflow'u (wf_8b56fc88; bit-exact zincir) + DV-2 p6b trajektorisi.

## 0. HÜKÜM — dört-halkalı bit-exact zincir (p6b 18/20'nin kökü)

1. **Kirli-örnek doğuşu:** kopma-öncesi-gönderilmiş orijinalin (send≈1.80s) SACK-bilgisi iki
   kopuk-fazda bloke → ilk canlı kümülatif-SACK t≈21.43'te NORMAL-inflight-yolundan raw=19.63s
   örneği (≈18.7s'i dönüş-ablukası, ~1s'i gerçek path-RTT). Tombstone-kol-1 DEĞİL (tomb_hit=0),
   dup-eşleme DEĞİL (taze-seq + origin-Karn-kapısı) — sayaç/kodla çürütüldü. NOT: kapı-agnostik
   sınıf — time-loss ateşleseydi kol-1 AYNI örneği üretirdi; zehir örnek-SINIFINDA.
2. **Guard-sessizliği (soru-a):** `apply_ack_delay` yalnız AŞAĞI-yön denetler (corrected ≥
   rtprop → kabul; rtt_rej yalnız düzeltme-reddinde). **KIRILAN VARSAYIM:** "raw kendi monotonic-
   saatimizde ölçüldüğü için güvenilir KANAL-üst-sınırıdır (tek şüpheli girdi peer-beyanlı
   ack_delay)" — aralıklıda raw, kanal-RTT'sini değil send→ack KOPMA-BEKLEYİŞİNİ ölçer.
3. **n=64 amplifikatörü:** kuantum-döngü n=Δt/T=20.15s/254.8ms=79→CAP-64 → TEK örnek srtt'yi
   1.019→19.613s'e taşıdı (bit-exact: (7/8)^64-hesabı log'la µs-özdeş; n=32 uymaz). Kuantum-
   varsayımı "geçen-Δt kanal-zamanıydı" blackout'ta kırılır.
4. **rtprop tek-örnek-reseed + Karn-açlığı kalıcılığı:** 20.15s-boşluk > W=10s → temiz-min
   prune, kirli-örnek TEK-başına min (min-filter bu anda last-value'ya DEJENERE — prune-on-observe
   + push-sonra-buda; **keep-last HİÇ ateşlemedi** — §0.29'daki "keep-last tuttu" ifadem İSABETSİZ,
   düzeltilmiştir). Sonrası tüm teslimler Duplicate-origin → Karn → 98.1s SIFIR-yeni-örnek →
   kendini-düzeltemez. Öz-etkileşim kilidi: kirli-srtt → W 156.9s, DrainRtprop-girişi t=99.88
   (gözlem 100.0 µs-uyum), min_dwell 19.6s → çıkışsız.

Hasar-haritası (ölçülü): Little-cap 516'ya kesildi; conf-eşiği 58.8s → RecoverFwd 96s KÖRDÜ;
dup-aralığı 19.6s → 14-16.5s send-freeze'ler → pencere-ıskalama → 18/20; qd sahte-0; TTL 176.5s.

## 1. FIX (soru-b): **F1 — tek-boğaz kabul-kapısı** (rtt.observe-öncesi; KANAL-TÜREVLİ, yeni-sabit YOK)

```
breach_ns: uçuş-penceresi-bazlı sessizlik-damgası — eşik-üstü ack-boşluğunun bittiği an
  (ÖLÇÜM last_ack_ns-güncellemesinden ÖNCE alınır — sender.rs:495 tuzağı, teşhis-notu)
eşik = max(3×srtt, K_CONF_EAI×EAI)      ← MEVCUT conf-eşiği ölçeğinin yeniden-kullanımı
                                            (sender.rs:1394-1396; tamamı kanal-türevi; K_CONF_EAI=2 mevcut)
RED-koşulu (İKİ-KOŞUL birlikte):
  örneğin uçuş-penceresi [send_ns, now] eşik-üstü-sessizlikle örtüşüyor (send_ns < breach_ns)
  ∧ raw > eşik
→ örnek HİÇBİR filtreye beslenmez (srtt-EWMA / rttvar / rtprop-min / qd-min) VE
  **drain_rtprop_qualifying'i SET ETMEZ** (kirli-örnek DrainRtprop çıkış-invariantını
  sağlayamaz — r2-pin) VE diag_ack_delay/rtt_rejected muhasebesi kapı-ÖNCESİ işler (TRAJ-
  karşılaştırılabilirlik); yalnız diag_rtt_stale + TRAJ "stale=".
breach-çıpası (r2-pin): sessizlik-referansı = max(last_ack_ns, first_send_ns) (conf-anchor
  kalıbı, sender.rs:1387-sınıfı); last_ack_ns yalnız yeni-acked SACK'lerde ilerler (boş-ack
  erken-dönüşü) — salt-kümülatif akışta gap-şişmesi iki-koşulla emilir (dürüst-raw eşik-altı).
```
- **Ayırt-edilebilirlik (sürekli-20s-RTT vs kopma-bekleyişi):** sürekli-uzun-RTT kanalda ack'ler
  pipeline'la ~EAI-ritminde akar → boşluk ≤ eşik → örnekler girer, dürüstlük bozulmaz; ilk-temas
  bootstrap'ı ilk-örnek-kuralıyla muaf. **Rejim-yükseliş deadlock'u YOK:** klasik "raw>k×srtt"
  tek-koşul kilidi iki-koşulla kırılır — yükselişte ilk örnek(ler) reddedilir ama akış oturunca
  sessizlik-koşulu düşer → sonraki örnekler girer, EWMA tırmanır (kalıntı: birkaç-örneklik lag,
  yönlü-güvenli — kanıt-şartı+cadence-taban false-loss'u frenler).
- **Kümülatif-SACK-sarkması tuzağı kapalı:** sessizlik per-ACK değil uçuş-penceresi-bazlı
  (eski-seq'in ack'i sessizlik-sonrası İKİNCİ SACK'e sarkarsa per-ACK-sessizlik≈0 olur ve zehir
  kaçardı — breach-damgası bunu keser).
- **§1-B — n-KORUMASI (r2; merdiven-kırıcı):** kuantum-döngü varsayımı "örnekler-arası Δt
  kanal-aktif zamandı" — kesintide kırılır ve eşik-ALTI kör-banttaki örneği fiilen reseed'e
  çevirir (r1-HIGH). KURAL (kanal-türevli, mevcut-sabitlerle, yeni-sabit YOK):
  `örnekler-arası gap > rtprop  →  n = 1` (Δt kanal-kanıtı değil → tek-EWMA-adımı).
  Merdiven-aritmetiği: 5.5s-kopma artık srtt'yi 1.02→1.58s yapar (eşik 4.7→6.3s) — adım-başı
  ×1.1-1.3; p6b-hasarına ulaşmak düzinelerce ardışık-eskalasyon ister (pratikte ölü). Sürekli-
  kanalda etkisiz (örnek-arası ≈ EAI ≤ rtprop-sınıfı decade'lerde n zaten küçük; 0.25k'da
  örnek-arası ≈ rtprop sınırında — davranış-sınıfı smoke'ta izlenir). qd_min de aynı kabul-
  kapısının arkasında (tek-boğaz) → r1-HIGH'ın qd-bonus-hasarı da kapanır.
- rtprop tek-örnek-reseed'e ek-koruma GEREKMEZ (kapı+n-koruması birlikte: eşik-üstü kesilir,
  eşik-altı n=1'le sınırlı); keep-last dokunulmaz.

## 2. RED-edilen alternatifler (soru-b devamı; sayısal)

- **F2 (yalnız rtprop-koruması): RED —** 8/9 tüketici kirli kalır (conf 58.8s-körlüğü, freeze'ler
  sürer) VE ters-etki: dürüst-rtprop + kirli-srtt → Little 4800/19.61=**245 B/s < btlbw 258**
  (kirli-BDP'nin bugünkü kısmi-telafisi bozulur — mevcuttan KÖTÜ).
- **F3 (srtt-korunur, rtprop-kirli): RED —** qd≡0 kalıcı-körlük, Little-tavanı silinir,
  T=rtprop/K_CONV=4.9s → kuantum-döngü ölür (ZM-1 nüksü); srtt-clock-v1 tablo-6 RED-gerekçesinin
  aynısı.

## 3. Dokuz-tüketici etkileşimi (soru-c): **F1'de değişen formül 0/9**

Tek-boğaz (rtt.observe = EWMA+rtprop-min+qd-min'in tek besleme-noktası) sayesinde dokuz tüketici
(pacing/Little · loss-eşiği · conf-eşiği · EAI/epoch→W'ler · W_RTPROP · tombstone-TTL · jitter ·
dup/tail-cadence · qd) + ProbeUp-periyot/DrainRtprop/plato-round FORMÜL-DEĞİŞMEDEN dürüst-saatte
kalır. Kabul-edilen kalıntılar (beyanlı): (i) meşru rejim-yükselişinde birkaç-örneklik lag;
(ii) pencere-İÇİ örnek-arzı sıfırlığı (p6b'de tüm pencere-teslimleri dup → Karn) bu fix'in görevi
DEĞİL — o, B1/kopma-körlüğü paketinin kapsamı (harita-notu); fix zehri keser, açlığı çözmez.

## 4. Ölçüm-planı + beklenti-şablonu (kod-GO sonrası, ölçümden önce yazılacak)

- **Birim-vektörler:** p6b-zinciri (kirli-örnek kesilir; srtt/rtprop 1.02s kalır); rejim-yükseliş
  deadlock-yok (1→20s: lag-sonrası EWMA tırmanır); kümülatif-SACK-sarkması breach-testi;
  sürekli-uzun-RTT muafiyeti; **(r2-ek) ESKALASYONLU-KOPMA merdiven-vektörü** (5.5s→15s dizisi:
  n-korumasıyla srtt-tırmanışı sınırlı, p6b-hasarı OLUŞMAZ assert'i); **(r2-ek) blackout-
  bootstrap vektörü** (S4-kalıntısı pinlenir: davranış belgelenir, KIRMIZI değil-beyanlı);
  n-koruması sınır-vektörü (gap ≈ rtprop tam-sınırı, tamsayı-semantiği pinli).
- **DV-2-seti yeniden (ana-sınav p6b):** İKİ-DALLI: (dal-A) srtt-dürüst → freeze'ler kalkar +
  conf-eşiği 6.26s → kopmalarda RecoverFwd zamanında → 18/20 → ≥19/20 bandı; (dal-B) teslim
  aynı-kalır ama sayaç/trajektori zehir-kesimini kanıtlar (davranış-iyileşmesi B1'e bağlıysa
  dürüst-kayıt — fix yine değerli: DV-3-ölçümünü temizler, ASIL amaç bu). h4/c90/count0/p6a3
  sınıf-aynı beklenir.
- **Sürekli-smoke:** temiz-1k(=45) + 0.25k(özet-özdeş; n-koruması sınır-kontrolü: örnek-arası ≈
  rtprop → davranış-sınıfı izlenir) — kapı sürekli-kanalda tetiklenmez-tahmini (0.25k: boşluk
  8.4s < eşik ✓). **p5-analizi (r2 — tek-anlatım, S3-çelişkisi giderildi):** İLK p5-kurtuluş-
  örneği muaf girer (srtt-None bypass — YENİ-kod, mevcut-davranış DEĞİL; r1-atfı yanlıştı) →
  srtt=tohum-raw (2-15s sınıfı). İKİNCİ+ kurtuluş-örnekleri (SACK-abluka-bekleyişli) kapıda
  KESİLEBİLİR → srtt tohum-değerinde DONAR. Bu, bugünkü davranıştan İYİLEŞME-yönlü (bugün o
  örnekler srtt'yi ŞİŞİRİYOR; donma < şişme — saat-türevleri küçülür) ama yeni örnek-açlığı-
  kanalı: dürüst-26ms'ye iniş yine imkânsız (o zaten bugün de imkânsızdı — Karn+abluka).
  p5×2-smoke dal-yapılı: (dal-A) sınıf-aynı/iyileşme; (dal-B) beklenmedik-kayma → dekompoze.
  PreConverge-kilidi tarif-düzeltmesi (r1-hatam): muafiyet olmasaydı srtt None-KALIR (INITIAL_RTT
  değil) → PreConverge-çıkışı kilitlenirdi (sender.rs srtt.is_some()-şartı) — muafiyet bu yüzden
  ZORUNLU-bileşen (tasarımsal, tesadüf değil).
- TRAJ: `stale=` sayacı.

## 5. Açık sorular (onayla birlikte)

- **S1:** Duplicate-kopyanın KENDİ send_ns'iyle RTT-örneği (teşhis F-D — taze-seq belirsizlik-siz;
  örnekleme-açlığı kanalını açar ve p6b'de zehir-sonrası kendini-düzeltmeyi sağlar) — bu tura
  GİRMELİ Mİ? Önerim: GİRMESİN (tek-değişken; açlık B1-kapsamı) ama B1-paketinin masasına kayıt.
- **S2:** RED-edilen örnek salt-sayaç mı, ayrı "bekleyiş-istatistiği" mi (ileride DV-3 kanal-
  değişti-tespitine girdi olabilir)? Önerim: bu turda salt-sayaç; istatistik DV-3-tasarım-sorusu.
- **S3 (r2-güncel):** ilk-örnek-muafiyeti YENİ-kod olarak yazılır (srtt-None bypass) — p5-tohumu
  yaşar; İKİNCİ+ örnek-analizi §4'te (tek-anlatım).
- **S4 (r2 — BEYANLI-KALINTI, karar):** blackout-bootstrap kill-switch — İLK örnek zehirse
  (kopma ilk-ack'ten önce başlarsa; p6b'de 0.2s farkla ıskalandı) muafiyet zehri tohumlar →
  eşik 60s → kapı o oturumda ölü; rtprop ilk-dürüst-örnekle öz-onarır ama srtt Karn-açlığında
  saplanabilir. NADİR-desen + n-koruması-yarı-ilaçsız. Önerim: KABUL+beyan (dar-tur disiplini;
  birim-vektörle davranış pinlenir; DV-3-izleme maddesi) — alternatif (ikinci-örnek-rebase
  mekanizması) yeni-tasarım, bu tura girmez. Karar Abkarada'nın.
