# KAPI-MANİFESTİ — Aralık kapıları (YOL PLANI v2 §9 + EK-1 D-19/D-20/D-07 + SAHİP v3/v4 + CC §5.3)

| alan | değer |
|---|---|
| Belge | KAPI-MANIFESTI v1 (Aralık). Yazan: K4G orkestratörü, Pzt 9 Kas 2026 (G0). |
| Durum | **TASLAK** → **DONDU** Çar 11 Kas (G2), G2 teşhis koşusunun ilk işi başlamadan önce, **en geç 12:00** (KK-20). |
| Depo yolu | `reports/yol-v2/KAPI-MANIFESTI.md` (v2 Ö-d). Bayt-özdeş kopya; docs PR; `reports/**` hosted CI'ı tetiklemez (`.github/workflows/ci.yml:23-26`). |
| Bağlar | Aralık kapıları (Ö0–Ö5, Ö-CC), model elemesi (eleme; bağlayıcı değil), G-CC, INV-RATE-1 kilit testi eşikleri, D-18 kayıtlı koşu listesi. |
| **Bağlamaz** | **GÖSTERİM-α'yı bağlamaz** (v4 III: "iddia kapısı değildir, çalışır-durum gösterimidir"). G2 teşhisi (ayrı teşhis manifesti, M2, **Sal 10 Kas 20:00** donar), mini-Ö, L/A yolu provaları bu manifestin dışındadır (§8). |
| Otorite | SAHİP v4 (2026-11-09) > SAHİP v3 (2026-11-04) > EK-1 (D-01…D-35) > CC belgesi (sha256 `df696a083329457b9907b6dff6d57f07081386f103504f1e61b8dbbac083e401`; CC-K1…K11, TD-1…3 imzalı) > YOL PLANI v2 rev2 > bu manifest. |
| Tarihler | Ö gecelerinin tarihleri bu manifestte yoktur: orkestratörün **G5 (Pzt 16 Kas)** yeniden hesabındadır (v4 Bölüm V başlığı: "[tahmin; orkestratör G5'te yeniden hesaplar, sahip tarih vaat etmez]"). Sabit [tahmin] tarihler v4 Bölüm V'tekilerdir (§6). v2'nin G numaraları geçersizdir. |
| Yasak sözler | "kanıtlandı", "QUIC'i yendik", "eşdeğer", "performans değişmedi". Kapı sözleri ("… GEÇTİ") yalnız sahibin ayrı mesajında. |

dondu: 2026-11-11 01:05 Europe/Istanbul
sha256: ba60a7276297f5999b8b26acbe1661697ec87e9417be8b735a841af9a133d16a

---

## 1. Gömülü sahip kararları (alıntı; lafzidir)

**D-19 (EK-1, imzalı):** "İki kip: **Kip-M** (G-E2a, G-T1, G-E2b, G-K1, G-FEC B1–B3, G-K6, G-K8, G-HB): commit'li betik + sha256'lı mekanik manifest + tek satır beklenti + deterministik testler; bağımsız yeniden hesap yok. **Kip-T** (G-T; G-FEC B4 = 'FEC ARQ turunu kaldırır' cümlesi): HVC + BM tam: ön-kayıt → kuru koşu → dondurma → tek koşu → donmuş analiz → bağımsız yeniden hesap. Ortak: kurallar G2'de donar; ölçülemeyen = KALDI." · "Kapı yapısı: ≤ 5 bağlayıcı ölçüt + DT + koruma ailesi; kapı düzeyi güç ≥ 0,80 (ölçüt başına ≥ 0,8^(1/m)); tutmazsa N = 60, eşik bir IQR uzağa ya da raporlanan." · "KÖTÜLEŞME: α_Ö = 0,02, program geneli ≤ 0,10, iki aşamalı doğrulama (aşama-2 bir sonraki planlı gecede, p < 0,05); bootstrap/RNG yok." · "e_c tablosu: teslim −0,05; p95 +%25; `wr_util` +0,10; tel/teslim +%5; spurious/retx +0,02; srtt/rtprop +0,3." · "Eşikler: … K-3 teslim@60 ≥ 0,67, p95 ≤ 1,5 × ideal ARQ (22,5 / 25,65 s) …; K-6 PTT-yalnız ≥ 0,80 ve FEC farkı ≥ +0,09, ürün yolu ≥ 0,69 ve farkı ≥ +0,05; K8 (G-K8); G-K1 B4 BK payı ≥ 0,125." · "Rakipler: quinn-reliable-stream (BBR-240k, BBR-RFC-IW …, Cubic), qdgram ayrı tablo, tcp-default birincil."

**D-19 değişikliği (v3 Bölüm I):** "kanıt zinciri adım 5 (A0/A1/A2/A3₁/A3₂ × 30 × 8 hücre, Kip-T) Ö4'ten **Ö5'e**; G-FEC B4 Ö5'te. Ö4 yalnız G-FEC B1–B3, G-K6, G-K8 (canlı)."

**D-20 (EK-1; çerçeve imzalı, G21 hükmü hariç):** "Sınıflar hedef banttan … HB-düşük r25, HB-orta r1k, HB-yüksek r12k, RTT 0,6/2 s, kayıp %0–30; P_blackout; stres (l50, l80, p2, p3) ayrı rapor, ürün kararını yönlendirmez … Tier-1/2 yalnız sıcak-sıcak … Tier-1 quinn kollarına karşı: her kolda UCB ≤ 0,10 ∧ Kama LCB ≥ 0,90; tcp-default ayrı sütun ve ayrı hüküm … Tier-2: mesaj D ∈ {2, 5, 10 s}; canlı PTT D_VO = OWD_nom + {0,4; 0,8} s …; hüküm rakip UCB < 0,90 − δ (δ = 0,05) ∧ Kama LCB ≥ 0,90 … Hüküm 'PoC-ön hükmü' etiketi taşır."

**D-20 eki (v3 Bölüm I):** "G-T'ye **'ilk temas / soğuk başlangıç'** iddia sınıfı: iki uç daha önce hiç konuşmamış, oturum yok; ölçü = ilk mesajın teslim süresi ve kurulum baytı; hücreler r25 ve r1k × d300/d1000 × l0–l30; rakip quinn soğuk (TLS el sıkışması; 0-RTT bileti YOK çünkü ilk temas). Kama kolu bu sınıfta **gerçek el sıkışmayla** (IdentitySeam değil; f0_flows yolu). Tier-1 'sıcak-sıcak' kuralı bu sınıfa uygulanmaz; sınıf ayrı raporlanır ve GO cümlesi kurabilir ('ilk temasta')."

**v4 Y11:** "Aralık G-T 'ilk temas' sınıfı: zarflar netem'li röleden ya da gerçek hub'dan; yoksa sınıf 'ölçülemedi'."

**D-07 (v3 değişikliğiyle):** "E_VO varsayılanı 66 B DEĞİL, **128 B** (Lyra 3,2 kbps × 200 ms = 80 B yük; 80 + 8 + 5 + 2 + 11 = 106 → 128). … G-K6 ve kanıt 1a/3/5 hücreleri 80 B demetle (`--ptt-bytes 80`); 40 B yalnız F0 kıyası için ikincil kol. **R_code_max = 16** (K = 2); R'yi kayıp ve bütçe seçer (D-06 tablosu %30'a dek R3; stres için %50 → R6, %80 → R16, yalnız hat payı varsa). Gerekçe: %80 kayıpta K2/R16 bloğu tek turda ≈ 0,90 çözülür, R3'te ≈ 0,26."

**D-07'ye ek (v4 I.3):** "R16 yalnız PTT-yalnız oturumlarda (r12k'da D-04 paylarına sığmıyor). 80 B demet 0x01 vekiline sığmaz (53 B tavan); gösterimde FEC kapalı olduğundan etkisiz, Aralık'ta E_VO 128 çözer."

**D-12 (v3 Bölüm I):** "(A); x = 0,05. GÖSTERİM-α (Bölüm III) DEMO-01'den ayrı bir teslimattır, D-12'nin kapıları gösterimi bağlamaz."

**D-16 (EK-1):** "(B) tek yönlü KÖTÜLEŞME: M1 medyan farkı ≤ −0,05, M2 gecikme oranı > 1,25; kesin MW; N = 20; iyileşme raporlanır, durdurmaz … α D-19'un program kuralından (α_Ö = 0,02, iki aşamalı)."

**Stratejik nokta C (v3 Bölüm I):** "betimsel 'mesaj-FEC' kolu (sınıf ayrımı kapalı, E2a + E1 açık, 0x05 `manual_r = 2`) Ö5'e ek olarak **Ö2 gecesinde de** koşar: manşet ve `b_r1k_d1000_l20_p1000`, N = 10. Yeni ürün kodu gerektirmediği iş emrinde doğrulanır. (B) kararı Ö5 verisiyle, DEMO sonrası."

**DEMO erken uyarısı (v3 Bölüm I + v4 I.3 düzeltmesi):** v3: "G-K8'in beş ölçüsü Ö1 ve Ö2'de `a_r12k_d300_l30_dosyali` ve `a_r12k_d300_l0_dosyali`'da 'raporlanan' olarak hesaplanır (aynı koşular, ek hücre yok)." → v4: "'ek hücre yok' yanlıştı; `a_r12k_d300_l30_dosyali` ve `a_r12k_d300_l0_dosyali` Ö2'ye **ek hücre** olarak yazılır."

**D-02 Aşama 2 (v3 Bölüm I + v4 I.3):** v3: "quinn farkı denetimi … kural · quinn (dosya:satır) · Kama (dosya:satır) · Kama'nın sapmasının yazılı gerekçesi var mı (spec/ADR/DR + ölçüm) · öneri {quinn'i al / gerekçeli koru} … quinn kodu Apache-2.0/MIT atfıyla **taşınır**, quinn-proto ürün crate'ine bağımlılık olarak eklenmez. Gain-cycling'in 'reddedilmiş' statüsü (engine :87, :671) bu denetime tabidir. BBR sabitlerinin (BtlBw penceresi 10 s, ProbeRTT 10 s/200 ms, ilk pencere) RTT 2 s ve 25 B/s'de ne yaptığı ayrı sütun; r25 koruması bağlayıcı. Teşhis kolu 'quinn-eşdeğer' Ö2'ye eklenir" *(sahibin kol adı; bu manifestte "quinn-kural kolu" denir, KK-29)*. v4: "'quinn-proto 0.11.11' → **0.11.18** (Cargo.lock'ta quinn 0.11.11 ve quinn-proto 0.11.18 ikisi de var). 'İki kol da geçerse quinn-eşdeğer seçilir' kuralı **kalkar** (CC-K8): quinn kuralı ancak doyuran yük ve sığ tampon koruma hücrelerini geçerse aday olur; ilk pencere büyütme EVO §9.1 gereği ayrı karar. Harfiyen taşınamayanlar (kurtarma penceresinin kayıpla daralması → INV-RATE-1; rastgele gain ofseti → determinizm; `min_max.rs` BSD kökeni; ADR-06/14 fork yasağı) tabloda 'gerekçeli koru / yeniden yaz' sütununa; gain-cycling reddi değişecekse spec'e tarihli EK satırı. **Raporlarda quinn-BBR ve quinn-Cubic birlikte; 'quinn-BBR F0'da fiilen denetimsiz (240 kB pencere, Startup'tan çıkmıyor)' cümlesi her kıyas tablosunun altına.**"

**CC-K8, CC-K10 (v4 Bölüm I erken imza):** CC-K8 yukarıda. CC-K10: "Model elemesi ve G-CC ölçütleri (§5.3) KAPI-MANİFESTİ'ne girer; yeni senaryo eşikleri G7'de." *(CC belgesinin G numaraları geçersiz; v4 Bölüm V: "yeni senaryo eşikleri CC-0 başında ön kayıtla".)*

**v4 I.2:** "Kilit testi cümlesi ('%30 IID kayıp + bekleyen iş → send_rate/gereken ≥ 0,9; sığ tamponlu tıkanıklıkta send/C ≤ 1,1') kanunun test hâlidir; model elemesi bu cümleyi değiştiremez, yalnız eşik sayıları KAPI-MANİFESTİ'nde ön kayıtla sabitlenir."

**v4 Bölüm V eki:** "Model elemesi ve G-CC ölçütleri CC §5.3'teki gibi; **I.1 (3)'ün PTT-yalnız şartı** ve **toplu akış l30** her iki listeye eklenir; yeni senaryo eşikleri CC-0 başında ön kayıtla."

**CC §6.4 (CC-K7 ile imzalı metin değişiklikleri, ilgili maddeler):** (5) "v2 §5.2 okuma kuralının sw95 şartı (medyan ≤ 1,2 s ya da 9. sıra ≤ 2,0 s) → G2'de raporlanan; G-CC'de bağlayıcı." (7) "v2 §5.2 deterministik testi '%30 IID + backlog → ProbeUp başarıyla kapanır' → beklenen sonucu 'kalır'." (9) "v2 §5.2 supersession listesi (8023, 7968, diag_contract:153, taban testleri) → gösterimde supersession yok (bayraklar varsayılan kapalı); varsayılan açılışta 7968 + liveness sözleşmesi; 8023 yalnız O8'de."

**v4 II orta madde:** "D-18 ölçüm listesine mini-Ö, G2 teşhisi ve CC §4.4 ölçümleri eklenir (CC-K7)."

---

## 2. Ortak kurallar (v2 §9.1; D-19)

- **Kipler:** Kip-M / Kip-T yukarıdaki gibi. Kip-T'nin bağımsız yeniden hesabı: "ajan-A dışında yürütücü: ajan-Ö'nün yalnız Python adımı; sayılar 1e-9, hükümler eşit; ayrışma = KALDI + enstrüman denetimi" (v2 §9.1).
- **Ortak:** kurallar bu manifestin dondurulmasıyla donar; sonuç görüldükten sonra kural/hücre/N değişmez; ölçülemeyen ya da koşulamayan ölçüt = **DEĞERLENDİRİLEMEDİ = KALDI**.
- **Kapı yapısı:** ≤ 5 bağlayıcı ölçüt (yalnız o işin hareket ettirdiği nicelik) + DT (kırmızı → kapı kalır) + koruma ailesi (KÖTÜLEŞME testleri ve bugün geçen mutlak korumalar) + raporlananlar.
- **Ölçü:** mekanizma kapılarında alıcı tek yön, uygulama teslim noktasında: ileti başına t_app_rx − t_offer; teslim edilmeyen = +∞; koşu p95 = ⌈0,95·n⌉'inci sıra; hücre değeri = koşu p95'lerinin medyanı; gönderen teyidi ikincil sütun. G-T'nin ölçüsü son tarihli tamamlanma ve HVC sonuç sınıfları.
- **Kollar ve ikili:** gece başına tek entegrasyon ağacı (`yol2/oN`); kollar bayrakla seçilir; ağaç SHA'sı kaydedilir. **Birleştirme penceresi:** Ö_N dondurmasından kapılı birleşmeye kadar ölçülen ikiliye giren yollara yalnız `yol2/oN`'de bulunan PR'lar girer; kapılı PR ertesi sabah ilk sırada birleşir; muafiyet bit-eşitliği kanıtlı davranış-nötr PR'lar (manifestte adıyla); aksi durumda kapı yeniden koşulur.
- **İstatistik (bootstrap ve RNG yok):** N = 30 tek yanlı %95 üst = 20. sıra, alt = 11. sıra; N = 10 üst = 9. sıra; N = 60 üst = 37. sıra. Oranlar Clopper–Pearson (20/20 → alt 0,861; 28/30 → 0,805; 30/30 → 0,905; 0/600 → üst %0,498). **Eşik ölçütü:** medyanın tek yanlı %95 sınırı eşiği geçer. **Fark ölçütü:** tek yanlı kesin Mann–Whitney (bağ düzeltmeli); Holm. **Marjlı karşılaştırma:** Moses tek yanlı %95 kayma sınırı (30 × 30'da k = 339, 10 × 10'da k = 28); oranlarda LCB₉₇,₅(A) − UCB₉₇,₅(B) (Clopper–Pearson, Bonferroni birleştirmeli).
- **KÖTÜLEŞME:** hücre c'de nokta etkisi > m_c ∧ tek yanlı kesin MW (oranlarda Fisher) p < α_Ö/k; α_Ö = 0,02 (program geneli ≤ 0,10); iki aşamalı (aşama-2 bir sonraki planlı Ö gecesinde aynı N; p < 0,05 ve etki > m_c). Aşama-1 işaretinde birleşme koşullu sürer; aşama-2 doğrularsa bayrak tek satırlık PR'la kapanır, sahibe tek satır. netem-7 (R-5) tek aile kuralı bu (D-16).
- **e_c:** teslim oranı −0,05; p95 +%25; `wr_util` +0,10; tel/teslim +%5; spurious/retx +0,02; srtt/rtprop +0,3. m_c, e_c'de güç ≥ 0,8 olacak biçimde (EK-OC).
- **Kapı düzeyi işletim karakteristiği:** m bağlayıcı ölçütlü kapıda P(hepsi geçer) ≥ 0,80 ⇒ ölçüt başına güç ≥ 0,8^(1/m) (m = 3: 0,928; 4: 0,946; 5: 0,956). Tutmazsa **koşudan önce** N = 60, eşik bir IQR uzağa ya da raporlanan (EK-OC). F0'da olmayan ölçüler Ö0 verisiyle; o da yoksa ilk Ö'de betimsel.
- **Eşik yerleşimi:** referans değerin kendisi eşik olmaz; en az bir dağılım payı (F0 IQR) uzakta. **Boşluk denetimi:** bugün kalan → bağlayıcı; bugün geçen → koruma; yanlış KALIR; ulaşılabilirlik ve güç.
- **N:** kapı/iddia 30 (OC'ye göre 60); netem-7 taraması 20 (KÖTÜLEŞME taraması; BM:279-280 yazılı gerekçesi manifestte); G-T'de netem-7 N = 30.
- **Kurulum:** mekanizma kapılarında "sıcak"; G-T Tier-1/2'de sıcak-sıcak (`--establish-gate`); soğuk ikincil; "ilk temas" sınıfı ayrı (§5.11). **Eşli tasarım** varsayılan değil (istisna: R3 kıyasları, aşağıdaki satır).
- **R3 kıyasları — aynı kökte eşleştirilmiş koşu (sahip 2026-11-10 akşam; KK-47).** Sahip metni (lafzi): "R3 kıyasları bundan sonra aynı kökte eşleştirilmiş koşu." Kapsam: kol ↔ taban K0 R3 koruma kıyaslarının hepsi (CC §4.2 R3; gösterim seti R3 hücreleri `b_r12k_d1000_l0_p1000` ve `b_r25_d300_l20_p65`; CC-0, Ö-CC ve sonraki Ö'lerdeki R3 satırları). Orkestratör okuması: kol ve K0 aynı kökte, aynı ikiliyle; tekrar i'de K0_i ve kol_i koşu kuyruğunda yan yana (aynı dalgada koşar), sırası dönüşümlü (tek i'de K0 önce, çift i'de kol önce); eşleşme koşu kaydına yazılır. Karar kuralı değişmez (R3 medyan oranı ve eşikleri); eşli oranların medyanı raporlanan sütun. Gerekçe [ölçüldü]: K0'ın r25 gönderim medyanı dört kökte 1 996 / 2 292 / 2 610 / 2 835 B; kökler arası oynama %10 payından geniş (reports/yol3/2026-11-10/BULGULAR.md §10). Düzenek işi: `f0_grid.sh` bugün hücre-hücre sıralıyor, netem tohumu yok; eşli sıra ilk R3 koşusundan önce eklenir (orkestratör). Önceden donmuş belgeler (ONKAYIT-V2, E1-TESHIS-G2) değişmez.

## 3. Taban, kollar, rakipler

- **Taban (KK-22):** "önceki main" ve "bütün yeni bayraklar kapalı" kolları, sahibin **"GÖSTERİM MOTORU DONDU <SHA>"** mesajındaki SHA'dan türeyen ağaçla tanımlanır; SHA her Ö'nün mekanik koşu manifestine yazılır (bu manifeste değil). F0 "bugün" sütunları `a858f3fc` tabanının ölçümüdür (`a858f3fc..b9d09d91` arasında `crates/` farkı yok).
- **Kip-M kolları:** her kural kendi bayrağının arkasında; geçersiz bayrak birleşimi açılışta tipli hata; bayrak kafesi ve ölçülen geri alma birleşimleri Ö manifestinde.
- **Rakipler (KK-29):** `quinn-BBR` (= quinn-reliable-stream BBR, 240 kB ilk pencere; F0 kolu), `quinn-BBR-RFC-IW`, `quinn-Cubic`; `qdgram` ayrı tablo; `tcp-default` ayrı sütun ve ayrı hüküm. **quinn içeren her tabloda `quinn-BBR` ve `quinn-Cubic` yan yana** yazılır ve tablonun altına aynen şu dipnot konur:

> **Dipnot Q:** quinn-BBR F0'da fiilen denetimsiz (240 kB pencere, Startup'tan çıkmıyor).

## 4. Koruma ailesi (v2 §9.2)

| kod | hücre | ölçü | not |
|---|---|---|---|
| R-1 | `b_r1k_d300_l20_p65`, `b_r1k_d1000_l20_p65` | alıcı p95 | Kama bugün geride |
| R-2 | `b_r25_d300_l20_p65` | teslim@60 s, p95, `wr_util` | `wr_util` 0,59–0,95 |
| R-3 | `b_r12k_d300_l80_p1000`, `b_r1k_d300_l80_p1000` (N = 30) | teslim@60 s | önde olunan eksen |
| R-4 | T0 `b_r12k_d300_l0_p1000`, `b_r1k_d300_l0_p1000` | göreli tel/teslim ve p95 + mutlak: iki yön tel/teslim ≤ quinn-240k × 1,05 (bugün 1,131 ↔ 1,265) | Dipnot Q |
| R-5 | netem-7 p1–p7 (N = 20) | §2 KÖTÜLEŞME | ADR-18 |
| R-6 | p5_revpath | teslim | bugün 1,000 |
| R-7 | p6_blackout + BENCH-01 | teslim; ikili sağkalım | CLAUDE.md §10 |
| R-8 | `a_rinf_d300_l30_tasma_dosya` | spurious, yanlış-confirm | Ö1'de G-E2a B5; sonra KÖTÜLEŞME |
| R-9 | `a_r12k_d300_l30_dosyali`, `a_r1k_d300_l30_dosyali`, manşet | srtt/rtprop p95 ≤ 2 (mutlak) | kalıcı kuyruk yok |
| R-10 | B-32'nin 60 s eksik koşu sayısı | Fisher | Ö3'te F0'nun en kötü 4 hücresi, Ö5'te tam |
| R-11 | fec-core mühürlü pin seti | K-FEC-OFF-1 + `desen1_sim` 5-pin her FEC PR'ında (CI); netem: P5{100×12, S5}, DESEN-1 4114→308, qd-52,2 s, DV-decade, Y3 — FEC PR'ı içeren Ö'de bir kez; E1'de DESEN-1 + qd + Y3 | — |

## 5. Kapılar

### 5.1 G-Ö0 — Ö0 (taban; Kip-M)
(i) nötrlük: motor kancalarında sahte saatli bit-eşitliği, runtime kancalarında (b) sınıfı (diff denetimi + birim testleri + Ö0-a betimsel netem); (ii) netem nötrlüğü betimsel; (iii) 1a negatif kontrolü 20/20 via = ARQ; (iv) düşürme sayısı = `open_failures` artışı; (v) ≥ 1 MB izde düşen kayıt 0, her düşürülen birim için zincir eksiksiz, yeni olay alanları dolu. Kalırsa: (ii) → enstrüman denetimi; diğerleri → Ş-Ö düzeltir, kısmi yeniden koşu. Ö0-a/Ö0-b hücreleri v2 §4 Ö0 tablosundaki gibi; E1 teşhisi satırı v4 ile G2 teşhisine taşındı (bu manifestin dışında, §8).

### 5.2 G-E2a — Ö1 (Kip-M)
Hücreler: K-7; seyrek r1k `b_r1k_d300_l30_p1000`, `b_r1k_d1000_l30_p1000`; 1a (0x05); `a_rinf_d300_l30_tasma_dosya`; r25; R-1, R-4…R-7, R-11; T0/T1a/T6; INT-02 kolları (erken rapor).

| # | bağlayıcı ölçüt | bugün | kural |
|---|---|---|---|
| B1 | K-7: (retx + PTO-prob + RecoverFwd-kopya baytı) / DATA, medyanın UCB'si ≤ 0,10; PreConverge geometrik kopyaları ayrı sütun | 0,526 (+0,100 kopya) | m3, m4, m8, m9 |
| B2 | seyrek r1k fazla bekleyiş: zaman yoluyla ilan edilen kayıplarda Suspected − (o seq'ten büyük ilk ACK'in varışı), koşu p50 medyanının UCB'si ≤ 0,1·srtt + 1 ms | ≈ 1,7–2,5 s (d1000 3,2–3,8 s) | m1 |
| B3 | kuyruk sonu: Ö1'in bütün Kama koşularında (≥ 600) havuzlanmış takılma oranının CP üst sınırı ≤ %0,5 | 4/235 | m9, E2a-K |
| B4 | 1a perm, manual_r ∈ {1,2}: C-ZAMAN 20/20 ∧ "burst_cap < parite başı" nedenli TokenDenied = 0 | Ö0-b | m6, m7 |
| B5 | R-8: spurious/retx ≤ max(0,05; Ö0-a + 0,02) ∧ yanlış-confirm/confirm ≤ max(0,02; Ö0-a + 0,01) | Ö0-a | m1, m2 |

- **DT:** C5.md:17 değişmezi (600 s); kara-delik testi (DUR-41/42); ESI testleri (E2a-K); PreConverge'de PTO arm = 0; tamamlanmış bloğa olaydan 1 tick sonra parite emisyonu 0; r25'te anında ACK emisyon sayısı ≤ bayrak kapalı; zaman eşiği ve onay ufku formülleri; D-35 seçilirse srtt = 100 s'de ilk PTO > srtt. **Koruma:** R-1, R-2, R-4, R-5, R-6, R-7, R-11. **Raporlanan:** manşet kanıt → retx p50; kuyruksuz anlarda kanıt → retx; Suspected → Confirmed; `fec_zero_floor` kolunda parite; K-7 tel/teslim; INT-02 GATING (2). **LOO kolları** (B1, B2, B4, B5 hücrelerinde): {tam, tam−tt_rfc, tam−pto_v2, tam−kova, tam−devir, tam−rtt_kurali}.
- **DEMO erken uyarısı Ö1'de yok (KK-23):** iki hücre Ö1 listesinde değil; satır "Ö1'de koşulmuyor — raporlanamadı".

### 5.3 G-T1 (yalnız hız) — Ö2 (Kip-M)
Hücreler: manşet; `b_r12k_d1000_l0_p1000`; `a_r12k_d300_l30_dosyasiz` (**80 B demet**, KK-24); R-9; R-11 (DESEN-1, qd, Y3); etkilenen R; T-hücreleri.

| # | bağlayıcı ölçüt | bugün |
|---|---|---|
| B1 | manşet sender_wait_p95 medyanının UCB'si ≤ 2,0 s | 20,06 / 19,51 s |
| B2 | `b_r12k_d1000_l0_p1000` sw95 UCB ≤ 2,0 s (quinn alıcı p95 1,41 s; Dipnot Q) | 3,40 / 3,80 s |
| B3 | manşet srtt/rtprop p95 medyanının UCB'si ≤ 2,0 (koruma) | Ö-d |
| B4 | A dosyasızda basış pencerelerinde (başı → sonu + 1 s) yeni DATA emisyon baytı / pencere süresi, medyanın LCB'si ≥ 640 B/s | 293–419 B/s |
| B5 | manşette PreConverge tuzağı ≤ 1/30 | 3/30 |

- **Koruma:** R-9, R-list, R-11; manşet PreConverge çıkışı p50'de KÖTÜLEŞME yok (bugün 2,41 s). **Raporlanan:** manşet alıcı p95; `b_r12k_d300_l30_p1000` alıcı p95; gönderim/gereken; ProbeUp başarı/iptal; r1k_d1000 PreConverge çıkışı (24,94 / 3,70 s); B4 hücresinde **yeni-veri emisyonu/sunulan** (v4 Y2'nin oranı; raporlanan).
- **DT (v2 §5.2, CC §6.4 madde 7 ile):** ProbeUp sırasında retx sürer; **"%30 IID + backlog → ProbeUp başarıyla kapanır" testinin beklenen sonucu "kalır"**; tıkanıklık kaybında qd iptali, srtt/rtprop ≤ 2; p = 0'da (e) bit-aynı; uygulamayla sınırlı PTT temposu platoya sayılmaz; DrainRtprop girişi.
- **Bağladığı (KK-26):** E1 bayraklarının (gösterim seti) varsayılan açılışı ⇔ G-T1 geçer ∧ sahibin varsayılan açılış kararı (v4 Bölüm V: Sal 8 Ara, G-CC okumasından sonra). **Supersession sırası (CC §6.4 madde 9):** gösterimde yok; varsayılan açılışta `probe_up_timeout_and_preempt_bookkeeping` (sender.rs:7968) + `tests/ek4_a2_liveness.rs` sözleşmesi; kilit-golden `sender.rs:8023` yalnız O8 seçilirse (CC-2). Hash'li kayıt test düzenlemesinden önce.

### 5.4 Ö2 ek kolları (kapı değil; betimsel/raporlanan)
1. **FEC erken uyarısı** (v2 §9.3 sonu): PTT-yalnız, `runtime_override` 0x01, K = 2, 200 ms demet; E2a (kova, r = 0 reddi) ve E1 açık; 1b ve 3 C-ZAMAN ile. **Demet 40 B, etiket "40 B vekil (0x01 tavanı 53 B)"** (KK-24; v4 I.3). Sonuç Ö4 öncesi E3 önceliklerini yönlendirir.
2. **Stratejik C mesaj-FEC kolu** (KK-25): sınıf ayrımı kapalı (şeritler devre dışı, tek sınıf), E2a + E1 açık (E1 = gösterim seti bayrakları; v4 I.3 terim eşlemesi; `cc_v2` o gecede birleşmişse ayrı satır), FEC 0x05 `manual_r = 2`; hücreler manşet ve `b_r1k_d1000_l20_p1000`; N = 10; betimsel. Ö5'te de koşar.
3. **DEMO erken uyarısı ek hücreleri** (v4 I.3; KK-23): `a_r12k_d300_l30_dosyali` ve `a_r12k_d300_l0_dosyali` **ek hücre**; G-K8'in beş ölçüsü (§5.9 B1–B5) **raporlanan**; B4 (60 s kesinti) bu hücrelerde tanımsız → "ölçülemedi". N = G-K8'in N'i (30).
4. **quinn-kural kolu** (v3 D-02 Aşama 2; CC-K8): Ö2'ye eklenir; **aday olması için** Ö-CC'nin doyuran yük (toplu akış l30) ve sığ tampon (ayrı tbf/bfifo) koruma hücrelerinde §6.2'nin koruma ölçütlerini geçmesi gerekir; "iki kol da geçerse quinn kolu seçilir" kuralı **yoktur**; ilk pencere büyütme EVO §9.1 gereği ayrı karar.

### 5.5 G-E2b (zamanlayıcı; yalnız `retx_first` varsayılanını bağlar) — Ö3 (Kip-M)

| # | bağlayıcı ölçüt | bugün |
|---|---|---|
| B1 | manşet kanıt → retx p50 medyanının UCB'si ≤ 1,0 s | 4,08 s (aday 0,51) |
| B2 | manşet alıcı p95 UCB ≤ 1,5 × quinn-240k = 10,74 s (quinn 7,16; ideal ARQ 6,58) | 31,28 / 24,60 s |
| B3 | sunum penceresi sonunda (son sunum + 1 RTT_nom) teslim edilmiş bayt / sunulan bayt, manşet ve `b_r12k_d300_l30_p1000`: Kama − quinn-240k Moses alt sınırı ≥ −0,05 | Ö-d |

- B2 ve B3'ün tablolarında quinn-Cubic yan sütun + Dipnot Q (eşik quinn-240k'ya bağlı kalır). **DT:** aç-kalmama; PTT-yalnız oturumda BtlBw azalmaz; iptal yolu; store-miss; ESI testleri. **Koruma:** R-list, R-10 alt kümesi, R-11 (P5-GATE {100×12}, TUR-7, DESEN-1). Kalırsa `retx_first` kapalı kalır; G-K1 ve DEMO etkilenmez.

### 5.6 G-K1 (akış izolasyonu; DEMO'yu bağlar) — Ö3 (Kip-M)
Hücreler: `a_r12k_d300_l30_{dosyali, dosyasiz, dosyali_dur}`, `a_r12k_d300_l30_dosyasiz_pttsiz`, taşma hücreleri, sürekli TEXT yükü, FILE_RECEIPT düşürme; r1k A hücreleri raporlanır.

| # | bağlayıcı ölçüt | bugün |
|---|---|---|
| B1 | TEXT p95, `_dosyali` − `_dosyasiz` Moses üst sınırı ≤ 2·S_maks/r + 1 RTT_nom = 0,78 s (S_maks = 1130 B) | 113,6 / 73,9 ↔ 13,5 / 10,7 s |
| B2 | ACK'li ∧ teslimsiz (WireAck ∨ Recovered) = 0 ∧ skipped = 0 ∧ late_dropped = 0, bütün A ve taşma koşularında | 15/470 koşu |
| B3 | `_dosyali_dur`'da yalnız dosya tüketicisi 30 s durur, dosyada bir birim kayıp; TEXT p95 farkı B1 sınırında; alıcı belleği ≤ manifest tavanı; tüketici açılınca dosya 30/30 tamamlanır; boşa giden retx baytı ≤ türetilen sınır (D-26) | Ö-d |
| B4 | dosya aç kalmaz: sürekli TEXT yükünde BK hizmet payı medyanının LCB'si ≥ 0,125 ∧ dosya 30/30 tamamlanır | Ö-d |
| B5 | PTT (ara eşleme VO_REL, FEC'siz): demet teslim p95, `_dosyali` − `_dosyasiz` Moses üst sınırı ≤ S_maks/r + 1 RTT_nom = 0,69 s | Ö-d |

- **DT:** RECEIPT karşı örneği; FILE_BEGIN kaybı; RECOVERED öneki; VO/CTRL Reorder dışı; sınıf başına kanallar. **Koruma:** R-list, R-10 alt kümesi. **Raporlanan:** r1k farkları; PTT → TEXT (`_dosyasiz` ↔ `_dosyasiz_pttsiz`); darboğaz bekleyişi p95; TEXT mutlak (G-K8'de bağlayıcı).

### 5.7 G-FEC — B1–B3 Ö4 (Kip-M); **B4 Ö5** (Kip-T; D-19 değişikliği)
- B1: 1a Ö4 sürümü (VO profili): perm C-ZAMAN 20/20; first C-FEC ∧ C-ZAMAN 20/20.
- B2: 1b: C-ZAMAN ≥ 28/30, payda 30 koşu.
- B3: kanıt 3: havuzlanmış C-ZAMAN ≥ 119/120 ∧ hiçbir konumda ≥ 2/20 kayıp.
- B4 (Ö5): adım 5 kararı (A0/A1/A2/A3₁/A3₂ × 30 × 8 hücre).
- Demet **80 B**, VO profili (E_VO 128 B; D-07); R_code_max 16, **R16 yalnız PTT-yalnız oturumlarda** (v4 I.3). **Koruma:** R-11 tam set; R-list. **Raporlanan:** A1 − A0, A2 − A0, C-FEC; parite kuyruğu son tarih sırası; dosya sırasında hedefli PTT düşürmesi (adım 4). Rapor kuralı: CC-v2 kaynaklı ve E3 + D-07 kaynaklı sonuçlar ayrı satırda (v4 I.1(2)).

### 5.8 G-K6 (canlı PTT) — Ö4 (Kip-M)
Hücreler: f0_flows PTT-yalnız VO `r12k_d300_l20_b120`, `r12k_d300_l30_b120` ve A `a_r12k_d300_l30_{dosyasiz, dosyali, dosyali_dur}`, hepsinde aynı ikilide FEC açık ↔ kapalı; **80 B demet** (D-07); mixed.rs quinn PTT ve bench C betimsel (Dipnot Q). Ölçü: T2-800.
- B1: PTT-yalnız l20: VO payı LCB ≥ 0,80 ∧ açık − kapalı Moses alt sınırı ≥ +0,09.
- B2: A `_dosyasiz` l30: VO payı LCB ≥ 0,69 ∧ açık − kapalı ≥ +0,05.
- B3: dosyaya takılmaz: `_dosyali` ve `_dosyali_dur` − `_dosyasiz` Moses alt sınırı ≥ −0,05; yalnız B2 geçerse değerlendirilir, geçmezse KALDI.
- B4: oynatılan geç çerçeve = 0 ∧ perm'de düşürülen demet son tarihte gönderende iptal (sayaç) ∧ ACK'li ∧ teslimsiz = 0.
- **Raporlanan:** r1k ("desteklenmeyen profil"); quinn PTT kolları; darboğaz bekleyişi.

### 5.9 G-K8 (DEMO ön kapısı; kipe göre) — güvenilir kip Ö3 + INT-02 son koşusu gecesi, canlı kip Ö4 (Kip-M)
DEMO hangi kipte yapılacaksa o kipin G-K8'i geçmiş olmalı. "Kötü ağ" vekili `a_r12k_d300_l30_*`; "iyi ağ" `a_r12k_d300_l0_dosyali`; 30 koşu.
- B1: PTT bas → ilk ses p95 UCB ≤ 3 s (kötü) ∧ ≤ 1,5 s (iyi).
- B2: her koşuda TEXT %100 60 s içinde ∧ TEXT p95 UCB ≤ 10 s (kötü) ∧ ≤ 2 s (iyi).
- B3: 150 KB fotoğraf ≤ 30 s, 30/30.
- B4: 60 s kesinti: INT-02 `ek-kesinti` 5/5 tohum, aynı oturum, metin %100.
- B5: güvenilir kipte bozuk ya da sırasız oynatılan çerçeve = 0 (durma p95 raporlanır); canlı kipte geç çerçeve oynatılmaz ∧ oynatılmayan demet payı ≤ **x = 0,05** (D-12, v3).
- INT-03 aynı ölçütleri hub üzerinden yineler.

### 5.10 G-HB (hedef bant ürün kapısı) — Ö5 (Kip-M)
- B1 (K-3): r25 %20 d300 ve d1000: teslim@60 s LCB ≥ 0,67 ∧ p95 UCB ≤ 1,5 × ideal ARQ (22,5 / 25,65 s).
- B2 (K-4): 60 s tam kesinti, iki yön, akışlar dolu: aynı oturum, ilk teslim ≤ 30 s + 2 RTT, BENCH-01 sağkalım 30/30 (CP alt 0,905).
- B3 (K-5): p5_revpath teslim 1,0, N = 30 (koruma).
- **Koruma:** R-3, R-10 (tam B-32), T0–T7 tam set, ve "bütün yeni bayraklar kapalı" koluna göre toplam KÖTÜLEŞME yok.

### 5.11 G-T (iddia kapısı) — Ö5 (Kip-T; transport-tdd §4.4.10 + HVC + BM)
- (0) Sınıflar: D-20 (yukarıda); stres ayrı rapor.
- (a) Kurulum adilliği: Tier-1/2 yalnız sıcak-sıcak; soğuk ikincil.
- (b) Tier-1: her quinn kolunda (BBR-240k, BBR-RFC-IW, Cubic) tamamlanma medyanının UCB'si ≤ 0,10 ∧ Kama LCB ≥ 0,90; D ≤ 60 s; tcp-default ayrı sütun, ayrı hüküm. Fiziksel tavan sınıfı (ρ_ARQ > 1, p3_loss95) betimsel.
- (c) Tier-2: mesaj D ∈ {2, 5, 10 s}; canlı PTT D_VO = OWD_nom + {0,4; 0,8} s (f0_flows ↔ mixed.rs, aynı JSON plan; rakip quinn reliable-stream ve qdgram, NON-NORMALIZED damgası zorunlu); kesinti sağkalımı BENCH-01. Hüküm: rakip UCB < 0,90 − δ (δ = 0,05) ∧ Kama LCB ≥ 0,90.
- (d) Tier-3: tel/teslim (`mid_qdisc`, iki yön) raporlanır; tek başına GO değil.
- (e) Guardrail'ler G1 (tahminci doğruluğu + kayıp/tıkanıklık karışıklık matrisi), G2 (§4.4.7 + trafik analizi yan kanalı + BindSigner), G3; ölçülmeyen varsa "guardrail'siz ön hüküm", GO cümlesi yok.
- (f) BM uyumu: §16 stall üçlüsü, §17 manifest şeması, §18 `bench fairness`, §15 N ≥ 30, HVC'nin 15 hüküm sınıfı.
- (g) Hüküm: GO/NO-GO hedef bant sınıfı başına; "QUIC'ten iyi" cümlesi yalnız Tier-1/2 hücrelerinde; "PoC-ön hükmü" etiketi. Her tablo Dipnot Q.
- **(h) İlk temas sınıfı (D-20 eki + v4 Y11):** ölçü ilk mesajın teslim süresi ve kurulum baytı; hücreler r25 ve r1k × d300/d1000 × l0–l30; rakip quinn soğuk (0-RTT bileti yok); Kama kolu gerçek el sıkışmayla. **Zarflar netem'li röleden ya da gerçek hub'dan geçmelidir; geçmiyorsa sınıf "ölçülemedi".** F0 düzeneğinde zarflar Unix datagram soketlerinden netem dışında geçer (`origin/ktp2/f0-taban:crates/kama-integration/examples/f0_flows.rs:9-12`, `SockLink` :627-716) → bu düzenekle sınıf "ölçülemedi"; netem'li röle Ö-c'de tasarlanır ve KARAR-KAYDI ile yazılır. Tier-1 sıcak-sıcak kuralı bu sınıfa uygulanmaz; sınıf ayrı raporlanır.

---

## 6. Sıkışıklık denetleyicisi (Aralık CC zinciri)

**Aralık ölçüm listesi = §6.6 (CC-PORT; sahip 2026-11-11, KK-49).** §6.2 yalnız 18 Kas gösterim motorunun kaydı; §6.1 ve §6.3–6.5 metni değişmedi.

Sabit [tahmin] tarihler (v4 Bölüm V): CC-0 **19–20 Kas** (Ö) · CC §4.4 ölçümleri **19–24 Kas geceleri** (Ö) · model elemesi **Cum 20 Kas** (bulut) / **Sal 24 Kas** (yerel) · CC-K12 **Pzt 23 / Çar 25 Kas** (sahip) · CC-2 **24 Kas – 3 Ara** (M2) · Ö-CC **3–4 Ara** · G-CC okuması **Pzt 7 Ara** · varsayılan açılış kararı **Sal 8 Ara** (sahip).

### 6.1 Model elemesi (CC-1; eleme, bağlayıcı değil) — CC §5.3 + v4 Bölüm V eki
Varyantlar: {(d) var, yok} × {P_TABAN_MAX 0,2, 0,3} × {overlay dar, tam} (8). **I.1'in üç koşulu `docs/design/CC-v2.md`'de yazılmadan eleme koşulmaz** (v4 I.1). Model N = 30 tohum; hücre değeri medyan; sw95 kuralının model karşılığı medyan ≤ 1,2 s ya da 20. sıra ≤ 2,0 s; "O0" = CC §5.3 lafzıyla bugün (`pcfix`'siz), "O0p" raporlanan (KK-27). En az bir varyant şunları sağlamalı [tahmin ölçütleri]:

| # | ölçüt | eşik |
|---|---|---|
| ME-1 | S1 (manşet) gönderim/gereken | ≥ 0,9 |
| ME-2 | S1L (uzun akış) gönderim/gereken | ≥ 0,9 |
| ME-3 | **toplu akış l30** (gönderecek veri hep var; d1000 ve d300) goodput/C | **EK-CC** (yön 0,74·C → ≥ 0,9·C) |
| ME-4 | S5/S6: gönderim/C | ≤ 1,1 |
| ME-5 | S5/S6: tıkanıklık kaybı | ≤ O0 + 5 puan; Startup dönemi kaybı ([0, 20) s) ayrı raporlanır |
| ME-6 | S5d (derin tampon) qd p95 | ≤ O0 + 1 s |
| ME-7 | S3 (gösterim analoğu) PTT p95 | ≤ gösterim seti × 1,10 |
| ME-8 | S9 (aynı darboğazda iki Kama akışı) pay | 0,4–0,6 |
| ME-9 | S10 policer | **EK-CC** |
| ME-10 | **PTT-yalnız** oturumda yeni-veri emisyonu/sunulan (v4 I.1(3)); (d)'siz varyant sağlamıyorsa elenir | ≥ 0,95 |
| ME-11…15 | yeni senaryolar: Kama + Cubic/CBR sığ tampon (rakibin kaybı) · sığ tampon + %20 dönüş kaybı · r1k sığ tampon · kapasite basamağı C → 0,3C (BtlBw_yol iniyor mu) · 10–30 s kesinti (kör emisyon) | **EK-CC** |

- Not (CC §5.3): S5/S6 F0'a kalibre edilemiyor. Bağlayıcı karar G-CC'de.
- **Kalkış izni** (CC §3.8; CC-K9): üç aday (i) sönen tek seferlik 10×MTU/srtt, (ii) paket çifti, (iii) BDP ölçekli MIN_CWND; **r25 ve r1k koruması üç aday için de bağlayıcı** — eşikler **EK-CC**; sabit 10×MTU reddedildi, yalnız referans satırı.
- Uçuş kapısı duyarlılık kolu raporlanan (KK-35).

### 6.2 G-CC — Ö-CC (bağlayıcı, ölçüm; Kip-M) — CC §5.3 + v4 Bölüm V eki
> **Sahip 2026-11-11 (madde 3; KK-49):** Aralık ölçümünde bu bölümün cc_v2 kolları yerine **CC-PORT (§6.6)** koşar. Bu bölüm yalnız 18 Kasım gösterim motorunun (K1 + A paketi + şeritler) kaydı olarak kalır; Aralık listesinde koşulmaz. Metin aşağıda değişmeden duruyor.

Hücreler (v4 V): F0 B ızgarası r12k/r1k; toplu akış l30; ayrı tbf/bfifo sığ tampon; iki akış; çapraz trafik; rakipler quinn-BBR, quinn-Cubic, RFC ilk pencereli quinn (Dipnot Q); PTT-yalnız hücre `a_r12k_d300_l30_dosyasiz` (80 B). Kollar: K0 = "bütün yeni bayraklar kapalı" (§3 taban), `cc_v2` açık, gösterim seti. N = 30; istatistik §2.

| # | bağlayıcı ölçüt | eşik |
|---|---|---|
| GCC-1 | manşette gönderim/gereken | ≥ 0,9 (medyan LCB) |
| GCC-2 | manşet sw95 v2 kuralı (kalkış izni CC-2'de olduğu için bağlayıcı) | sahip metni (CC §5.3): "medyan ≤ 1,2 s ya da 9. sıra ≤ 2,0 s"; "9. sıra" N = 10'un tek yanlı %95 üst sınırıdır, G-CC N = 30'da aynı sınır **20. sıra** okunur (KK-27; 1,2 s ve 2,0 s aynen) |
| GCC-3 | toplu akış l30 goodput/C | EK-CC eşiği |
| GCC-4 | sığ tampon (ayrı tbf/bfifo): gönderim/C ≤ 1,1 ∧ tıkanıklık kaybı ≤ K0 + 5 puan | — |
| GCC-5 | derin tampon qd p95 | ≤ K0 + 1 s |
| GCC-6 | çapraz trafik: rakibin kaybı | ≤ K0 + 5 puan |
| GCC-7 | PTT-yalnız yeni-veri emisyonu/sunulan (v4 V eki) | ≥ 0,95 |

- **Koruma:** T0 ve K-3 (r25 ≥ 600 s dahil) gerilemiyor (§2 KÖTÜLEŞME); P5-GATE, DESEN-1, qd-52,2 s, DV ve Y3 pinleri yeşil (R-11). Bu iki satır CC §5.3 G-CC listesinin 7. ve 8. maddeleridir: **G-CC'nin geçmesi için ikisi de sağlanır** (biri kalırsa G-CC kalır).
- G-CC = CC §5.3'ün sekiz maddesi + v4 Bölüm V eki (PTT-yalnız) = dokuz madde: yedi eşikli ölçüt (GCC-1…GCC-7) + iki koruma maddesi, hepsi bağlayıcı. D-19'un "≤ 5 bağlayıcı ölçüt" kuralı ile çelişki: sahibin CC §5.3 listesi + v4 eki aynen korunur, kapı düzeyi güç (§2 OC) yedi eşikli ölçüt için EK-OC'de hesaplanır (KK-27).
- **Bağladığı:** `cc_v2` ve (KK-26) E1 bayraklarının varsayılan açılış kararı (sahip, Sal 8 Ara); CLAUDE.md §4 / INVARIANTS / golden supersession'ları G-CC geçince (sahip, Sal 8 Ara; v4 Bölüm V — CC belgesindeki "G21" geçersiz numara).

### 6.3 INV-RATE-1 kilit testi (v4 I.2; ön kayıt)
- Eşikler (bu manifestle sabit): **%30 IID kayıp + bekleyen iş → send_rate/gereken ≥ 0,9; sığ tamponlu tıkanıklıkta send/C ≤ 1,1.**
- Test koşulları (süre, ölçüm penceresi, tampon boyu, tohum, sahte saat) **EK-INV**'de, CC-2'nin ilk commit'inden önce (§7). Test kapalı döngüdür (CC §3.7); deterministik; eski kodda kırmızı, yeni kodda yeşil (v2 §11.7).
- O8 elenirse (CC-K4 yedeği) eşikler aynı kalır; kapalı döngü cümlesi beyanlı sınır olur (v4 I.2), kilit testinin kapalı döngü yarısı "beyanlı sınır" etiketiyle raporlanan.

### 6.4 CC §4.4 beyanlı sınır ölçümleri (raporlanan; kapı değil) — 19–24 Kas geceleri
- Çapraz trafik: aynı netem darboğazında UDP CBR ya da TCP Cubic; K0 ↔ gösterim seti; Kama tel hızı ve rakibin kaybı.
- r25 ≥ 600 s (ya da "boşta kaldıktan sonra patlama"); K0 ↔ gösterim seti.
- Kesinti (p6_blackout) ya da kapasite basamağı C → 0,3C.
- Bayraklar yalnız gösterim düğümünde açık olduğu için gösterimi bağlamaz; G-CC'de r25 ≥ 600 s bağlayıcıdır (GCC koruma).

### 6.5 D-02 Aşama 2 quinn farkı tablosu (kapı değil) — orkestratör, 19–23 Kas
Sütunlar: kural · quinn-proto **0.11.18** (dosya:satır) · Kama (dosya:satır) · yazılı gerekçe (spec/ADR/DR + ölçüm) · öneri {quinn'i al / gerekçeli koru / yeniden yaz} · RTT 2 s ve 25 B/s davranışı (r25 koruması bağlayıcı). Harfiyen taşınamayanlar "gerekçeli koru / yeniden yaz" sütununda (§1). quinn kodu Apache-2.0/MIT atfıyla taşınır, bağımlılık eklenmez. Dipnot Q.


### 6.6 Ö-CC — CC-PORT kolları (ISEMRI-CC-PORT v2 §5; sahip 2026-11-10 akşam; KK-48) — **Aralık ölçüm listesi** (sahip 2026-11-11; KK-49)
Kaynak: `ISEMRI-CC-PORT-2026-11-10.md` v2, özgün sha256 `2a800f420da86852c30114921c86ffbb2a8771472ae224acab62b7a960efbdea` (depo sürümü `docs/isemri/ISEMRI-CC-PORT-2026-11-10.md`, sha256 `377d5eaeb6d82fd7ed83e7498e447276d51535ec24c0cf4ea4a9a84f84305b27`; fark yalnız kişi adı ve kredi değeri, §0a). Belge: "Hücreler KAPI-MANİFESTİ'ne bu iş emriyle girer."

**Düzeltme 2026-11-11 (sahip "CC-PORT DURUM", Çar 11 Kas 00:45, madde 2–3 ve 5; KK-49).** CC-PORT'un gerçek hâli v2/v3 belgesinden farklıdır ve bu bölüm ona göre düzeltildi; belge metni (v2) değişmez. 2026-11-10 akşam sürümündeki cargo özelliği kolları (`cc_port`, `cc_port,cc_loss_filter`, `cc_port,cc_lowband`) **geçersizdir**: bu özellikler yoktur ve olmayacaktır; belgedeki `compile_error!` konusu kapandı (sahip). Önceki metin orkestratör yedeğinde, satır satır.

- **Motor seçimi çalışma zamanında, tek ikili:** `Sender::new()` / `Receiver::new()` = eski motor (bit-eşit); `Sender::with_cc_port()` + `Receiver::with_cc_port()` = port (quinn-proto 0.11.18 BBR + RFC 9002). Eşleme ve paket/birim muhasebesi: `docs/design/CC-PORT-ESLEME.md` (normatif değil). Sim kapısı (ölçüm değil): `cargo test -p adaptive-transport --test cc_port_sim -- --nocapture` (iki motor aynı süreçte, eşli; a2c46563'te 6/6).
- **Ağaç:** PR-1 = #170, main `23bffb7f` (son aday başı `a2c46563`; ağaçlar bayt-aynı). `a2c46563` içindeki `loss.rs` düzeltmesi (zaman eşiği yüklemi `>` → `>=`, vade anında ilan) eski motoru da kapsar (KK-49). Her koşu kaydında ikilinin sha256'sı.
- **Taşıma:** ölçüm bench **`kama`** taşımasında koşar; `kama-product` (ClientSession) portu henüz kurmuyor (sahip, 2026-11-11). bench `kama` tek akış taşır (N birim × yük; hepsi t0'da ya da sabit aralıkla) → karma yüklü hücrelerin (CP-1, CP-2) bench karşılığı **dosya akışıdır**; TEXT, PTT ve karşı trafik bu taşımada yoktur [orkestratör okuması]. Şerit ve oynatma tamponu gerektiren CP-6, kama-product portu kurana dek koşulamaz.
- **Kollar:**

| kol | seçim (bench; tek ikili) | etiket | rol | durum |
|---|---|---|---|---|
| legacy = K0 | istemci `--kama-arm tail-v1`; sunucu bayraksız (eski motor, tail v1) | `kama-tail-v1` | taban | koşar |
| referans port | istemci `--kama-arm cc-port` **ve** sunucu `--cc-port` (ikisi birlikte; biri eksikse koşu geçersiz) | `kama-cc-port` | kabul ölçütleri bu kola uygulanır | koşar |
| port + kayıp filtresi | CC-M4'te ayrı kurucu olarak gelecek | — | ayrı kabul yok; referansın yanında raporlanan; INV-RATE-1 kararı sahipte (belge §7) | **M4 sonrası; koşulmaz** |

  Ek kollar (kapı kolu değil): CP-1'de ölçek için `quinn-BBR-RFC-IW` (§3; Dipnot Q) — bench `quinn` taşımasında aynı dosya akışıyla, alıcı damgası `--rx-log` [orkestratör okuması]; CP-7'de düşük bant kolu (CC-M4'te ayrı kurucu) — **M4 sonrası; koşulmaz**.
- **Eşleştirme (KK-47; sahip teyidi 2026-11-10 akşam):** her hücrede tekrar i'de kollar koşu kuyruğunda yan yana (aynı netem ad alanı), sıra tekrardan tekrara değişir; eşli oran kol_i / K0_i, medyanı raporda. Kabul sütununda "eşli medyan" yazan satırda eşik eşli oranların medyanına uygulanır.
- **Ölçü (sahip teyidi 2026-11-11 madde 5):** tamamlanma = **alıcıda uygulamaya teslim anı** − sunuş (§2 Ölçü; teslim edilmeyen +∞); koşu pX = ⌈X·n⌉'inci sıra; hücre değeri = koşu değerlerinin medyanı (⌈0,5·n⌉'inci); gönderen teyidi (ACK) ikincil sütun. bench `kama` sunucusunda alıcı damgası (`--rx-log`) yoktur ve iki kolun ACK politikası farklıdır (eski motor 20 ms tik + kota; port sırasız varışta anında ACK + 25 ms) → ACK tabanlı tamamlanma kollar arasında eşit değildir. Alıcı teslimi, sunucu arayüzünde birimin **ilk varışından** okunur (bench `kama` sunucusu her DATA'yı varışta işler; yeniden sıralama tamponu yok); enstrüman Ö-CC'den önce depoya girer ve sha256'sı EK-CC'de [orkestratör okuması].
- **Hücreler — ağ alanları ayrı (sahip notu 2026-11-10 akşam: "ileri/dönüş hızı, gecikme ve kayıp ayrı alanlarla; farklı hücre yanlışlıkla koruma testi yapılmasın"):**

| # | hücre (kayıtlı ad) | ileri yön: hız · gecikme · kayıp · tampon | dönüş yönü | yük (F0 hücresi → bench `kama` karşılığı) | metrik | referans port için kabul | N |
|---|---|---|---|---|---|---|---|
| CP-1 | 6 paketlik kuyruk: `a_r12k_d150_l20_foto150k` + netem `limit 6` (gece hücresi `scripts/yol3/tesis/cells-a6/a_r12k_d150_l20_foto150k__*_qlim.cell`) | 12 500 B/s (100 kbit/s) · 150 ms, jitter yok · %20 · `limit 6` | ileri ile aynı | F0: dosya 150 480 B (5 s'de), TEXT 40 × 64 B, PTT 25 × 80 B, karşı trafik → bench: dosya **132 × 1140 B = 150 480 B**, t0'da; ısınma 3 birim (ölçüm dışı) [orkestratör okuması] | dosya tamamlanma p50; dosya teslim sayısı | p50 ≤ 1,5 × quinn-BBR-RFC-IW aynı hücrede; 10/10 | 10 |
| CP-2 | manşet `a_r12k_d300_l30_dosyali` | 12 500 B/s · 300 ms ± 60 ms (normal) · %30 · netem varsayılanı (1000 paket) | ileri ile aynı | F0: dosya 524 700 B, TEXT 40 × 64 B, PTT demeti **80 B** (F0 hücresinde 40 B; KK-24) → bench: dosya akışı; birim boyu EK-CC | gönderim/gereken = gönderilen bayt / hattın o sürede taşıyabileceği bayt | ≥ 0,9 | EK-CC |
| CP-3 | manşet, CP-2 ile aynı koşu | — | — | — | faydalı/gönderim = alıcıda ilk kez teslim edilen yük baytı / tele yazılan toplam bayt (başlık, parite, tekrar dahil), tüm koşu | eşik EK-CC'de (aşağıdaki satır); belge: "ön kayıt adayı, bağlayıcı değil" | CP-2 ile aynı |
| CP-4 | r25 = `b_r25_d300_l20_p65` | 25 B/s (200 bit/s) · 300 ms ± 60 ms (normal) · **%20** · netem varsayılanı | ileri ile aynı (simetrik) | 6 ileti × 65 B, 18 s aralık, son tarih 60 s (bench: `--msgs 6 --payload 65 --submit-interval-ms 18000`) | ileti tamamlanma p50 **ve** teslim sayısı | p50 ≤ 1,10 × K0 (eşli medyan); teslim sayısı ≥ K0 | EK-CC (mini-Ö R3'te 20) |
| CP-5 | d1000 l0 = `b_r12k_d1000_l0_p1000` | 12 500 B/s · 1000 ms ± 200 ms (normal) · %0 · netem varsayılanı | ileri ile aynı | 100 ileti × 1000 B, 285 ms aralık, son tarih 60 s | ileti tamamlanma p95 | ≤ 1,1 × K0 | EK-CC (mini-Ö R3'te 10) |
| CP-6 | şeritli PTT: `a_r12k_d150_l20_foto150k` (gösterim hücresi, derin kuyruk) | 12 500 B/s · 150 ms · %20 · netem varsayılanı | ileri ile aynı | CP-1'in F0 yükü; K1 profili = S1 şeritleri + oynatma tamponu açık (sahip teyidi 2026-11-11) | ilk demet gecikmesi | ≤ 3 s, 10/10 | 10 — **kama-product portu kurana dek koşulamaz** |
| CP-7 | 0,025 kbps: **hücre tanımı yok** — 3,125 B/s mı 31 B/s mi, hangi yön: CC-M0 eşleme belgesi (belge §3, §7) → hücre EK-CC'de | EK-CC | EK-CC | EK-CC | teslim var/yok; ilk TEXT süresi | yalnız raporlanır (deney kipi) | düşük bant kolu **M4 sonrası; koşulmaz** |

  - CP-4 notu: belgede "r25 (l25, d300)" yazıyor; kayıtlı r25 hücresinin kaybı **%20**'dir ve "l25" hiçbir kayıtlı hücrede yok. Koruma hücresi yukarıdaki alanlarla tanımlıdır; alanları farklı bir hücre bu satırın yerine geçmez.
  - CP-6 notu: "K1 profili" = S1 şeritleri + oynatma tamponu açık; motor kolu tablodaki gibi (sahip teyidi 2026-11-11 madde 5).
  - Bench karşılığı notu: bench yükü F0 yükünün yalnız dosya akışıdır; CP-1'in kabul satırı (quinn'e oran) aynı bench yükündeki quinn koluyla okunur. F0 karma yükündeki gece ölçümleri (`reports/yol3/2026-11-10`) bu satırların ölçüsü değildir.
- **İlk bakış (sahip 2026-11-11 madde 4; kapı koşusu DEĞİL, raporlanan):** CP-1, K0 ↔ `kama-cc-port`, eşli, N = 10, aynı kök, sıra değişken, `a2c46563`. Bu bölüm o koşunun sonucu görülmeden yazıldı (KK-49).
- **Faydalı/gönderim eşiği (CP-3; sahip notu 2026-11-10 akşam):** belgedeki "%30 kayıpta fizik üst sınırı 0,70'tir (1 − kayıp)" kesin hüküm sayılmaz: bağımsız %30 kayıp ve yalnız yük baytı sayımı varsayımında **beklenen** sınırdır; sonlu koşuda gerçekleşen kayıp %30'dan sapar, mutlak sınır değildir. Eşik **aday kolların sonucu görülmeden** taban ölçümüyle belirlenir ve donar: legacy kolunun CP-2 hücresinde ayrı taban koşusu (aynı ağaç, Ö-CC'den önce) → eşik EK-CC'ye → donma → Ö-CC.
- **Kabul:** referans port CP-1, CP-2, CP-4, CP-5, CP-6'yı sağlar; CP-3 belge gereği bağlayıcı değil (EK-CC eşiğiyle kıyas raporlanır); CP-7 raporlanan. CP-6 kama-product portu kurulmadan ölçülemez: belge "Ortam yüzünden koşamayan zorunlu kontrol uygun ortamda tamamlanır; 'ortam' etiketi geçiş sonucu değildir" (§2: ölçülemeyen = KALDI).
- **Bağladığı:** PR-2 (varsayılan motor değişimi) için GO = Ö-CC'nin PR-1'in son aday SHA'sında geçmesi; sahip dizgisi "CC-PORT VARSAYILAN <SHA>" (belge §7). INV-RATE-1 bu iş emriyle değişmez.
- **§6.2 ile ilişki (sahip 2026-11-11 madde 3; 2026-11-10'daki açık soru kapandı):** CC-PORT, Aralık ölçümünde §6.2 cc_v2 kollarının **yerine geçer**. §6.2 yalnız 18 Kasım gösterim motorunun (K1 + A paketi + şeritler) kaydı olarak kalır, Aralık listesinde koşulmaz. **§6.6 Aralık listesidir.**

---

## 7. Ön kayıt EK yuvaları (KK-21; ekle-yalnız)

Bu manifest donduktan sonra **değişmez**. Eksik sayılar ayrı EK dosyalarıyla, koşudan **önce** sabitlenir. Her EK: kendi kanonik sha256 satırı (§9 prosedürü), `ana-manifest:` satırında bu manifestin 64 hex sha256'sı, yazan, tarih-saat. Yazan **orkestratör**; sahibe tek satır bildirim (sahip imzası gerektirmez; v4 I.2 "ön kayıtla sabitlenir").

| EK | içerik | son an | depo yolu |
|---|---|---|---|
| EK-CC | ME-3, ME-9, ME-11…15 eşikleri; kalkış izni r25 ve r1k koruma eşikleri; GCC-3 eşiği; yeni senaryoların hücre tanımları (CC-1'in `CC-v2.md` §8'i ile aynı); **§6.6:** CP satırlarının N'si, CP-3 faydalı/gönderim eşiği (legacy taban koşusundan, adaylar görülmeden), CP-7 hücre tanımı (CC-M0 sonrası) | **Per 19 Kas 09:00** (hedef; CC-1 eleme yedeği için tavan aynı gün 12:00, aşağıdaki satır; CC-0'ın ilk koşusundan ve CC-1 eleme koşusundan önce; v4 V "CC-0 başında") | `reports/yol-v2/KAPI-MANIFESTI-EK-CC.md` |
| EK-INV | INV-RATE-1 kilit testinin koşulları (§6.3) | CC-2'nin ilk commit'inden önce (Sal 24 Kas) | `reports/yol-v2/KAPI-MANIFESTI-EK-INV.md` |
| EK-OC | işletim karakteristiği: m_c, N = 60'a çıkan ölçütler, kaydırılan eşikler, raporlanana inenler (Ö0 verisiyle) | her Ö'nün ilk koşusundan önce | `reports/yol-v2/KAPI-MANIFESTI-EK-OC-<Ö>.md` |
| Ö manifesti | mekanik koşu manifesti: DONDU SHA, ağaç SHA'sı, kollar, hücreler, N, ikili sha256, CPU yükü | her Ö gecesinden önce | `reports/yol-v2/<Ö>/MANIFEST.md` |

EK-CC Per 19 Kas 12:00'ye dek yoksa: CC-1 elemesi yalnız sayısal eşiği yazılı ölçütlerle koşar; EK-CC'ye bağlı satırlar "raporlanan; ön kayıtlı eşik yok" etiketini taşır (ISEMRI-YOL3-CC1 §8).

## 8. Gösterim ölçümleriyle ilişki (KK-20)
- **Teşhis manifesti (G2, Çar 11 Kas):** CC §4.2 kollar/hücreler/S/R1/R3/R4/ön testler aynen; **M2'nin iş emrinde, ayrı dosya; Sal 10 Kas 20:00'de sha256 ile donar.** Bu manifest yalnız onun 64 hex sha256'sını ve donma saatini kayda geçirir (ilk G2 raporunda): `teşhis-manifesti: <yol> <sha256> <tarih-saat>`.
- mini-Ö (G4) ve L/A provaları bu manifestin kapısı değildir; III.1 ölçütleri gösterim içindir.
- **D-18 kayıtlı koşular (KK-28):** Ö0–Ö5, LOO kolları, FEC erken uyarısı, Ö2 ek kolları (§5.4), INT-02 kolları, A3 paneli, Ö1′/Ö2′/Ö3′, KÖTÜLEŞME aşama-2 yeniden koşuları, TAMPON-FEC/TAMPON-2, **mini-Ö, G2 teşhisi, CC §4.4 ölçümleri** (v4 II), **CC-0 ölçümleri ve Ö-CC** (bu manifestte kayıtlı). Bunları orkestratör başlatır; diğer netem ve ayrıcalıklı komutlar sahipte (D-18). Ölçüm sırasında yerel ajan ve cargo durur (v2 §11.5).

## 9. sha256 doldurma prosedürü (dondurma anında; orkestratör)

Kanonik satır: dosyada **tek** bir satır `sha256: ` ile başlar. Özet, o satır **çıkarılarak** dosyanın geri kalanından hesaplanır. Sıra: tarih → özet → denetim → commit.
```bash
F=KAPI-MANIFESTI.md
grep -c '^sha256: ' "$F"                                  # 1 olmalı
sed -i "s/^dondu: .*/dondu: $(TZ=Europe/Istanbul date '+%Y-%m-%d %H:%M') Europe\/Istanbul/" "$F"
H=$(grep -v '^sha256: ' "$F" | sha256sum | cut -d' ' -f1)
echo "$H" | grep -Eq '^[0-9a-f]{64}$' && echo "64 hex"
sed -i "s/^sha256: .*/sha256: $H/" "$F"
test "$(grep -v '^sha256: ' "$F" | sha256sum | cut -d' ' -f1)" = "$(sed -n 's/^sha256: //p' "$F")" && echo SHA-TUTARLI
# depoya bayt-özdeş kopya, açık yol, docs PR
cp -p "$F" <worktree>/reports/yol-v2/KAPI-MANIFESTI.md
git -C <worktree> add -- reports/yol-v2/KAPI-MANIFESTI.md
git -C <worktree> diff --cached --name-only
```
- PR gövdesine ve G2 günlük raporuna tam 64 hex özet yazılır (kısaltılmaz). Her Ö manifesti ve her EK bu özeti `ana-manifest:` satırında taşır.
- Denetim (herkes için): `grep -v '^sha256: ' reports/yol-v2/KAPI-MANIFESTI.md | sha256sum` çıktısı dosyadaki değere eşittir. Dosyanın kendi beyan ettiği özet ile hesaplanan eşit değilse manifest geçersizdir; orkestratör aynı gün düzeltir ve sahibe tek satır yazar.
- EK dosyaları aynı prosedürle (kendi `sha256: ` satırı).

## 10. Dondurma sonrası değişiklik
Donmuş metin değişmez. Hata bulunursa: yeni sürüm (`KAPI-MANIFESTI-v1.1.md`), tarihli, eski sürüm silinmez, değişiklik ve gerekçe satır satır; yalnız koşusu henüz yapılmamış kapılar için ve **sonuç görülmeden**. Sonuç görüldükten sonra kural, hücre, N değişmez (§2). Sahibin sayıları ve ölçütleri hiçbir sürümde değişmez.

## 11. KARAR-KAYDI satırları (ayrıntı: `KARAR-KAYDI-DEFTERI.md`)
- **KK-20** dondurma anı (Çar 11 Kas ≤ 12:00) ve yeri; teşhis manifesti ayrı (M2, Sal 10 Kas 20:00).
- **KK-21** EK yuvaları (ekle-yalnız; yazan orkestratör; EK-CC Per 19 Kas 09:00).
- **KK-22** taban = DONDU SHA ağacı; F0 "bugün" = `a858f3fc`.
- **KK-23** DEMO erken uyarısı ek hücreleri yalnız Ö2; Ö1 "koşulmuyor"; B4 "ölçülemedi".
- **KK-24** 80 B kapı hücrelerinde (G-T1 B4 hücresi dahil); 0x01 vekil kolları 40 B, etiketli.
- **KK-25** stratejik C kolu yeni ürün kodu gerektirmiyor (doğrulama); "E1 açık" = gösterim seti.
- **KK-26** E1 bayraklarının varsayılan açılışı = G-T1 ∧ sahibin 8 Ara kararı; supersession sırası.
- **KK-27** model istatistiği ve sw95'in N = 30 karşılığı; G-CC dokuz madde (yedi eşikli ölçüt + iki koruma, hepsi bağlayıcı), OC yedi eşikli ölçüt için.
- **KK-28** D-18 kayıtlı koşu listesi (CC-0 ve Ö-CC dahil).
- **KK-29** quinn kol adları; "birlikte" = her quinn tablosunda quinn-BBR ve quinn-Cubic yan yana + Dipnot Q.
- **KK-47** R3 kıyasları aynı kökte eşleştirilmiş koşu (sahip 2026-11-10 akşam; §2 satırı).
- **KK-48** Ö-CC CC-PORT kolları (legacy / referans port / port+filtre) ve CP-1…CP-7 hücreleri (§6.6; ISEMRI-CC-PORT v2 §5).
- **KK-49** CC-PORT gerçek hâli (cargo özelliği yok; çalışma zamanı seçimi; kollar K0 `tail-v1` ↔ `kama-cc-port`; filtre ve düşük bant kolları M4 sonrası), bench `kama` taşıması, §6.6 = Aralık listesi, §6.2 yalnız 18 Kas kaydı (sahip 2026-11-11).
