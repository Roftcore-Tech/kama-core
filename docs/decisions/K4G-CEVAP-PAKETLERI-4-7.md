# K4G sprinti — sahip karar paketleri CEVAP 4–7 (kayıt)

> Bu dosya K4G-05b tasarım notunun (`docs/design/K4G-05b-RETX-FIX.md`) ve K4G-15/K4G-05 PR'larının atıf verdiği **sahip kararlarının** depodaki kaydıdır (CEVAP PAKETİ 7 — EK B7). Metinler değiştirilmeden aktarılmıştır; her bölümün başında kaynağı ve metnin sha256'sı yazılıdır. Karar mercii Abkarada'dır; bu dosya karar üretmez. Kapı ifadeleri yalnız sahibin mesajında geçerlidir; burada geçmeleri gerçekleşmiş kapı anlamına gelmez.

---

## CEVAP PAKETİ 4 — 2026-10-16

*Kaynak:* dosya `~/Downloads/CEVAP-PAKETI-4-2026-10-16.md` (sahip), bayt-özdeş; sha256 `126d678266d2fcf621e4ca42963fd92fe82513fd7fa6781e636d0973fa8ce2d4`

````markdown
# CEVAP PAKETİ 4 — Abkarada → K4G orkestratörü (2026-10-16)

Bu metin K4G sprintinin orkestratörüne (mevcut Claude Code oturumu) verilir. Aşağıdaki cevaplar DUR-K4G-05 (güncel), DUR-K4G-11b, K4G-15 ortam onayı ve "S4 KR listesi" içindir. Kapı ifadeleri yalnız bu metinde açıkça yazıldığı yerde ve birebir geçerlidir.

---

**T5 TAMAM** — `sch_netem` yüklü (lsmod kanıtı 2026-10-15); ifade burada birebir verildi, kayıt bu mesajdır.

**DUR-K4G-11b CEVAP: (b)** — arayüz "Bağlandı" olayını bekler; oturumsuz `send_text` → `NoSession` davranışı kalır. K4G-15 README'ye bir satır, KATMAN 6'ya bir kalem. Demo ölçümü iki taraf bağlıyken yapılır (K1); bağlantı sırasında ikinci mesaj kuralı bu sprintte açılmaz.

**K4G-15: (1)** — ortam onayı: cargo-ndk + Android NDK + `aarch64-linux-android` hedefi + `uniffi` (D11'de onaylı), **kullanıcı dizinine, sudo'suz**. `.so` + Kotlin bağlayıcıları + `poc-uniffi-v0` release'i (private repo). K4G-05'le paralel yürür; K4G-15 MOBILE-HANDOFF kapısıdır, CORE-CLOSE'u bloklamaz.

**DUR-K4G-05 CEVAP: (a) — motor düzeltilir.** (c) reddedildi: Reliable teslim sözünün kalıcı bozulması beyanlı sınır olamaz; K8 demo ölçütü ("60 s kesinti sonrası aynı oturumda %100 teslim") ve Sprint 2'nin INT-03'ü bu motora bağlı. (b) tek başına yetmez; rapor yine yazılır ama kapanış düzeltmeyi bekler.

Yeni kart **K4G-05b — transport yeniden-gönderim yolu düzeltmesi (v1.1)**. Önce **tasarım notu** (`docs/design/K4G-05b-RETX-FIX.md`, ≤ 2 sayfa), ben onaylayınca kod. Notun karşılaması gerekenler:

1. **Terk edilmeyen birim (D1):** yeniden gönderim tavanına (`RetxConfig` cap 3) ulaşan Reliable birim **her zaman** kopya/ileri (duplicate) yoluna planlanır ve ACK gelene ya da oturum bitene kadar geri çekilmeli aralıkla **yeniden gönderilmeye devam eder**; "cap aşıldı ama kopya kuyruğu boş" durumu yapısal olarak imkânsız olmalı (yeniden-planlama yalnız PRE_CONVERGE kopyalarına özgü olamaz; `sender.rs:6095-6103` yolu genelleşir). `RetxConfig::fixture`'ın "KALİBRE DEĞİL" değeri ürün yolunda kalacaksa Reliable davranış ondan **bağımsız** olmalı (cap = yolu değiştirir, asla düşürmez); kalibrasyon ayrı iş.
2. **Sıranın başı önce (D2):** kopya kuyruğu en eski onaysız birime öncelik verir; kesinti sonrası token açlığı (dup_queue 169–332) sıranın başını bekletemez. Kesinti sonrası ilk taze ACK'te RTO geri çekilmesi sıfırlanır; `srtt`'nin 60 s'ye şişmesi için kök neden (yeniden gönderilen birimden RTT örneklemesi — Karn) notta açıklanır ve kapatılır; RTO üst sınırı transport TDD'deki değerle, yeni sayı uydurulmaz.
3. **Kapsam ve sınırlar:** yalnız `crates/adaptive-transport` yeniden-gönderim/kopya planlaması (`sender.rs`, `arq.rs`); **tel değişmez, hiçbir vektör bozulmaz**, S1/S3/noise-wire dokunulmaz; FEC/kalibrasyon kampanyası (DR-1x) **yeniden açılmaz**. Transport v1 FROZEN'dır: değişiklik **v1.1** olarak transport TDD'ye YAMA KAYDI ile girer (üstü çiz, silme); bu yazılı onay = bu mesaj. Sayfa dışı bir bulgu çıkarsa DUR.
4. **Kanıt:** (i) D1 ve D2'yi sahte saatle **belirlenimli** yeniden üreten iki regresyon testi (önce kırmızı, sonra yeşil); (ii) mevcut transport testleri + `tv_*` altın vektörleri değişmeden yeşil; (iii) mutasyon: cap dalı ve kopya planlaması; (iv) INT-02 **yeniden**: 10 tohum × 600 s pencere, GATING (2) 10/10; ek-kesinti kolu 60 s kesinti 5/5 **aynı oturumda** %100 teslim; dosya kolu (D3) harness düzeltmesiyle (uzun pencere kesinti/dosya kollarına geçer) ölçülür ve raporlanır.
5. **Sıra:** tasarım notu → onayım → K4G-05b PR (tek kart, tek şerit) → INT-02 kampanyası → rapor → benden "INT-02 GEÇTİ" (ayrı mesaj). K4G-16 kapanış belgeleri bunu bekler; "CORE-CLOSE kapandı" bu mesajla söylenmez.

**S4 KR listesi (B grubu):** bu oturumda karar yok. S4 kararları `KM-SEAM-TH-001.md` v1.3.1 §0 KR-1…19 ile verildi ve **ayrı** bir Claude Code oturumu (Sprint 2 A şeridi, "S2-A") icra ediyor; listendeki numaralar analizin numaralarıdır, S4'ünkiler değil — liste aşıldı. K4G kapsamında S4 işi yok; `docs/seams/KM-SEAM-TH-001*`, `docs/hub/`, `docs/deploy/`, `docs/plans/SPRINT-2-*` dosyalarına dokunma. S2-A oturumu `crates/adaptive-transport/**`'a dokunmaz (K4G-05b'nin dosyaları); çakışma yok.

Rapor: tasarım notu hazır olunca tek mesaj; INT-02 yeniden koşusu bitince gün raporu biçiminde.
````

---

## CEVAP PAKETİ 5 — 2026-10-16

*Kaynak:* sahibin K4G oturumuna yapıştırdığı metin, birebir (oturum kaydından çıkarıldı); sha256 `f12ecbae0aa54f28746bc9cb9339d7649b2ca1398f10a0c7d6a11cc43fb2e759`

````markdown
# CEVAP PAKETİ 5 — Abkarada → K4G orkestratörü (2026-10-16, 15:20 raporuna)

PR #133 merge'ü (rustls advisory, #128 politikası) kabul. Aşağıdakiler K4G-15'in iki DUR adayı ve K4G-05b tasarım notunun karar isteyeceği dört bulgu içindir; not gelmeden yetki sınırı belli olsun diye şimdi veriyorum. "K4G-05b ONAYLANDI" ifadesi **bu mesajda yok**; notu okuyunca ayrı mesajla.

---

## K4G-15

**DUR-K4G-15-A CEVAP: kalsın.** `kama-mobile`'ın `kama-node` dışında `kama-app` ve `kama-contracts`'a doğrudan bağımlılığı bu sürümde kabul; **koşul:** C9 katman kuralı bu bağımlılığı **yalnız tip dönüşümüne** sınırlar (davranış çağrısı yok — `kama-mobile` bir motor fonksiyonunu doğrudan çağırıyorsa DUR), `layer_check`'te denetlenir ve K4G-16 KATMAN 6'ya kalem: "M-sonrası: `kama-node` facade'ı gereken tipleri yeniden ihraç eder, `kama-mobile` yalnız `kama-node`'a bağlanır." PR gövdesinde hangi tiplerin hangi crate'ten geldiği listelenir.

**DUR-K4G-15-B CEVAP: evet, `device-bob.json` da zip'e girer.** `root.json` **girmez** (değişmez kural). README'ye tek satır: "İki cihaz dosyası aynı pakette yalnız süreç-içi loopback testi içindir; gerçek kurulumda her telefon yalnız kendi dosyasını alır (Sprint 2 provisioning v1 + Deploy TDD §8)." Plan metnindeki "yalnız alice" cümlesine YAMA KAYDI (üstü çiz).

Sıra değişmedi: inceleme → düzeltme → tam kapı → PR → `.so` + Kotlin paketleme (betik gerçekten koşmuş olmalı; "koşulmadı" ile DONE yok) → merge → `poc-uniffi-v0` release (private). Release bağlantısı gün raporuna.

## K4G-05b — tasarım notu için yetki ve kararlar

**Mekanizma düzeltmen kabul:** D1 "cap 3 → HandToForward" değil, **öksüz kuyruk-probu kopyası**. Notta kuralı mekanizmayla değil **değişmezle** yaz ve testle: *"Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir."* D1 ve D2 regresyon testleri bu değişmezi doğrudan sınar (sahte saat; 600 s'de "0 yeniden gönderim" → kırmızı).

**1. RTT örnekleyici (srtt şişmesi).** Teşhisin kabul: kesinti öncesi gönderilmiş orijinal, kümülatif SACK ile yeniden raporlanınca bayat örnek alınıyor; kuantum döngüsü (n = 64) tam ağırlık veriyor. Kural: RTT örneği yalnız **ilk kez onaylanan ve son gönderimi tek anlamlı** birimlerden (onaylanan kayıt, çerçevenin **en yeni gönderimine** ait olmalı; birden çok gönderimi olan birimden örnek alınmaz); bayat örnek alınmaz, kesinti sonrası ilk taze örnek döngüyü yeniden başlatır. **Yetki:** engine spec **§5/§6 (RTT örnekleme) ve §13 (öncelik)** bu değişiklik kadarıyla **YAMA KAYDI ile yamanabilir** (üstü çiz, silme yok, `word_coverage`; "v1.1" etiketi); bu yazılı yetki = bu mesaj. Yama metni notta aynen yer alır, ben görmeden spec'e girmez.

**2. RTO tavanı.** Yeni sayı yok: koddaki `validated_v1` 30 s değeri transport TDD'ye **normatif sabit** olarak YAMA KAYDI ile yazılır ("koddan belgeye"); değiştirilecekse ayrı karar. Kesinti sonrası ilk taze ACK'te geri çekilme sıfırlanır (notta nerede).

**3. Donma ADR'si kapsamı.** ADR'yi atlamıyoruz, iki parçaya bölüyoruz:
- **Bu kartta:** `kama-v1-full` kimlik değişikliği → `kama-v1.1-full` (client-core profil kimliği + freeze testleri; `api_surface`/freeze pin'leri kasıtlı güncellenir, PR'da beyan). Dosya kapsamı buna göre **genişler**: `crates/adaptive-transport/**` + kimliğin yaşadığı client-core/freeze test dosyaları (notta liste; başka dosya yok).
- **T-hücre yeniden doğrulaması:** kayıp/kesinti ile ilgili hücreler **bu kartta** koşar (notta hangi hücreler, süresi); tam set **DEMO-01'den önce** gece koşusuyla; aradaki fark KATMAN 6'ya "beyanlı: T-hücre tam seti bekliyor" kalemi. Notta tam setin süresini yaz; 1 günün altındaysa bu kartta koşsun.

**4. Not biçimi:** ≤ 2 sayfa; değişmez + mekanizma + dokunulan dosyalar listesi + spec yama metinleri (engine §5/§6/§13, transport TDD RTO sabiti, v1.1 kaydı) + test planı (D1/D2 regresyon, mutasyon, tv_* vektörleri değişmez, T-hücre alt kümesi, INT-02 10 × 600 s + 60 s kesinti 5/5 aynı oturum + dosya kolu) + süre tahmini. Tel ve altın vektörler **değişmez**; değişiyorsa DUR, kart değil.

Kod, benden **"K4G-05b ONAYLANDI"** gelmeden yazılmaz (değişmedi).

## Diğer

- **S2-A:** doğru gözlem — henüz açılmadı; bugün açıyorum, ayrı checkout. `main` dışında ortak dosya yok; S2-A `crates/adaptive-transport/**`'a ve senin kimlik/freeze dosyalarına dokunmaz. Sen `docs/seams/KM-SEAM-TH-001*`, `docs/hub/`, `docs/deploy/`, `docs/plans/SPRINT-2-*`'e dokunma.
- **Hayri `ws-b/amp-int04-entropy`:** gözlem kabul; bu oturumun işi değil. Hayri'ye PR açmasını ben söylerim (Sprint 2 P1).
- INT-02 düzenek düzeltmesi (ed2841fe, D3) kabul; kampanya K4G-05b merge sonrası.

Rapor: tasarım notu tek mesaj; K4G-15 PR'ı açılınca tek satır (PR#, release bağlantısı DONE'da).
````

---

## CEVAP PAKETİ 6 — 2026-10-18

*Kaynak:* sahibin K4G oturumuna yapıştırdığı metin, birebir (oturum kaydından çıkarıldı); sha256 `e405f7f6ed946f66772e8e17dae635747dab5762598ecc15dece34b4a46b451c`

````markdown
CEVAP PAKETİ 6 — 2026-10-18
DUR-K4G-15-C CEVAP: (a) — SignalingId::get ve LimitsError için Display tip dönüşümüdür, motor çağrısı değil; C9 listesine açıkça yazılır.
DUR-K4G-15-D CEVAP: (a) — PoC’de facade bağlantı kaynaklı hataları kendisi yeniden gönderir, düzelmeyenler PENDING’de kalır; plan D6 ile ui-api INV-5 çelişkisi KATMAN 6'ya kalem (“UI ‘yeniden gönder’ sunmaz; Beta’da karar”).
DUR-K4G-15-E CEVAP: (a) — belge + test ile kabul; açık kapanış kancası KATMAN 6'ya.
K4G-05 dalı: 2 commit’i push et, yalnız dal, PR açılmaz.
K4G-05b notu için ön karar: kama-v1-full kimliğinin geçtiği bench, nightly.yml ve Faz-5 betikleri yalnız kimlik adı değişikliği kadarıyla dosya kapsamına girer (davranış değişikliği yok); FEC parite ESI deliği kapsam içi düzeltilir ve ayrı regresyon testi alır; bayat zamanlayıcı SACK’leri ve 28 B/s hız tabanı notta ölçümle gelsin, karar o zaman. Not geldiğinde ayrı mesajla “K4G-05b ONAYLANDI” ya da düzeltme.
Paylaşılabilir sayfa gerekmez; gün raporu yeter.
````

---

## CEVAP PAKETİ 7 — 2026-10-18

*Kaynak:* sahibin K4G oturumuna yapıştırdığı metin, birebir (oturum kaydından çıkarıldı); sha256 `df140db7c98e6bd9d759453d4aaaa68e3968b97a71a11380cc08f4c3c17051f6`

````markdown
# CEVAP PAKETİ 7 — Abkarada → K4G orkestratörü (2026-10-18; K4G-15 DONE + K4G-05b tasarım notu DUR-1…10)

**K4G-15 / MOBILE-HANDOFF kabul:** PR #134 (main 3439fccd), release `poc-uniffi-v0` (`kama-mobile-3439fccd.zip`, sha256 108a1ba2…, LOAD segmentleri 0x4000 hizalı). Bağlantı Hasan'a benden gidiyor. "CORE-CLOSE kapandı" **denmez**.

Aşağıdaki cevaplar `docs/design/K4G-05b-RETX-FIX.md` (dal `k4g/05b-retx-fix` @d6d60be8) içindir. **"K4G-05b ONAYLANDI" bu mesajda yoktur.** Sıra: bu cevaplar nota işlenir (Ek B'deki seçenekler kapanır, Ek A yama metinleri buna göre güncellenir), yeni commit push edilir, SHA bana yazılır → notu okurum → ayrı mesajla onay ya da düzeltme.

---

## Tasarım değişmezleri — kabul, iki koşul

Nottaki değişmezler olduğu gibi kabul: tek kalıcı geometrik kopya sahibi; tavan yolu değiştirir, hiçbir zaman düşürmez; sahiplendirme her olayda, RecoverFwd'e dayanmaz; kopya kuyruğunda en eski onaysız önce, aralık ≤ RTO_MAX 30 s; kesinti sonrası ilk ilerlemeli ACK geri çekilmeyi sıfırlar.

1. **CEVAP 5'teki sahipsizlik cümlesi notta birebir yer alır** ("Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden gönderebilmelidir…") ve D1/D2 regresyon testleri bu cümleyi sınar: sahte saat, **düzeltme öncesi kodda kırmızı** (600 s'de 0 yeniden gönderim), düzeltme sonrası yeşil. **D2 testi düzeltme öncesi kodda kırmızı yapılamıyorsa** mekanizma gösterilememiş demektir → D2 koda girmez, DUR.
2. **Bellek değişmezi (DUR-2 ile):** kesinti ne kadar sürerse sürsün sender belleği **pencereyle sınırlı** kalır; yeniden gönderim sayısıyla büyüyen hiçbir kayıt tutulmaz (birim başına O(1) durum: son gönderim zamanı/sırası, deneme sayısı, sahip; deneme listesi yok). Beyan "yavaş büyür" değil, "pencereyle sınırlı" olur.

## DUR cevapları

| # | Cevap | Koşul |
|---|---|---|
| **DUR-6** | **(a), bir düzeltmeyle.** Faz-5 betikleri ve v1 kanıt bankası **bayt-özdeş kalır**; `kama-v1-full` adı orada değişmez. Gerekçen doğru: CEVAP 5/6'daki kimlik kararı client-core profil kimliği + freeze testleri + ileriye dönük gece koşusu (`bench/nightly.yml`) içindi, v1 kanıt betiklerini kapsamamalıydı. **"Ağaç dışı kopya" depo dışı demekse hayır:** v1.1 koşusu **depoda**, yeni dizin/dosyalarla (v1 dosyalarına dokunulmadan; depoda ayrı dizin kastediliyorsa zaten budur). **Ön-kayıt dosyası** (hangi hücreler, eşikler, kabul ölçütü, koşulacak commit SHA) koşudan **önce** commit'lenir; sonuçlar ona karşı raporlanır. Kanıt bankası iki etiketli: v1 (dokunulmadı), v1.1 (yeni). |
| **DUR-8** | **(a), dört koşulla.** `dr_cs1_acceptance`'ın 3 testi yalnız hata sayesinde yeşilse fikstür düzeltmesi meşrudur; "mevcut test değişmez" kuralının istisnası **kanıtla** verilir: (1) PR gövdesinde test başına bir paragraf — neden yeşildi, düzeltmeyle neden kırmızıya düştü, fikstürde tam ne değişti, **iddia (assertion) ve eşik aynı kaldı**; (2) **2×2 matris** PR'da çıktılarıyla: eski kod + eski fikstür yeşil (bugün), yeni kod + eski fikstür **kırmızı**, yeni kod + yeni fikstür yeşil, **eski kod + yeni fikstür de yeşil** — sonuncusu kırmızıysa fikstür testin amacını değiştirmiştir (test artık düzeltmeyi sınıyor, CS-1'i değil) → (a) değil, DUR; (3) mutasyon koşusu bu üç testin **hâlâ mutant öldürdüğünü** gösterir; (4) DR-CS-1 belgesine YAMA KAYDI: "v1.1'de kabul fikstürleri düzeltildi; neden: teyit edilmiş birimi yeniden planlayan v1 davranışına dayanıyordu". |
| DUR-1 | **(a)** srtt yokken ilk örnek için v1 seçimi aynen; ilk örnek alınır alınmaz yeni kural. Ek: soğuk başlangıç + %30 kayıp altında srtt'nin N örnekte yakınsadığını gösteren regresyon testi (N notta); ilk örnek zaten RTO_MAX ile sınırlı. |
| DUR-2 | **(a)** v1.1'de beyan + tavan AMENDMENT-9 kartına; yukarıdaki bellek değişmezi **bu kartta** uygulanır. |
| DUR-3 | **(b)** Yetki genişler: engine spec **§16.2 saat-sınıfı tablosuna bir satır** (kopya aralığı tavanı + sahipsizlik süresi; YAMA KAYDI; yalnız o satır, yama metni notta). Uzun pencereli hücreler DEMO-01 öncesi gece koşusunda; KATMAN 6'ya kalem. |
| DUR-4 | **(b)** Kapsam dışı belgelere (DR-ARQ-TDD, `docs/transport-v1/…`) tek satırlık **AMENDMENT-12 işaretçisi** (YAMA KAYDI biçiminde, silme yok); asıl düzeltme INT-02 sonrası ayrı doküman PR'ı. |
| DUR-5 | **(a)** Ayrı **ERRATUM PR'ı**: "recover_fwd_entries = 0 (330/330)" → ham veride 15/330 (T1a 2, T1b 7, T6 6); ham veri dosya yolları ERRATUM'da; donmuş belgeye ERRATUM bandı (metin silinmez, üstü çizilir). PHASE5 "Aşama A 5 sa 20 dk" etiketi aynı PR'da düzeltilir (tam kampanya süresi). Tasarımı değiştirmez: sahiplendirme zaten RecoverFwd'e dayanmıyor. |
| DUR-7 | **(a)** Önce `k4g/05-int02` (D3 düzenek + M1/M2 ölçüm aracı) **ayrı PR**, onay beklemeden açılabilir: gövdede "davranış değişikliği yok, yalnız gözlem; `ClientLiveDiag` aynası maliyeti ölçülmedi" beyanı; tv_* vektörleri değişmedi; tam kapı yeşil → merge → `k4g/05b-retx-fix` rebase. |
| DUR-9 | **(a)** "Sıranın başı önce" yalnız kopya kuyruğu içinde; sınıflar arası öncelik bu kartta değişmez. **Koşul:** D2 regresyon testi **eşzamanlı yeni trafik varken** koşar (kopya kuyruğu tokensiz kalıp sıranın başını bekletmemeli — D2'nin mekanizması bu). Dosya kolu INT-02'de ölçer. |
| DUR-10 | **(a)** Gövde ~2 sayfa + aynı dosyada Ek A (yama metinleri birebir, spec'e aynen girecek) + Ek B (DUR'lar, bu cevaplarla kapatılmış). |

## Ölçüm notları (§5b) — kabul

- 28 B/s hız tabanı yeniden üretilmedi → bu kartta **hız işi yok**; INT-02 raporu ölçer, görünürse ayrı kart.
- "60 s kesinti düzeltme öncesi 2/2; önceki kırmızılar kısmen 120 s pencere artefaktı" → D2 düzeltmesi yukarıdaki koşulla (testi düzeltme öncesi kodda kırmızı) yapılır; INT-02 raporunda önceki kırmızının artefakt payı açıkça yazılır.
- T-hücre tam seti 5 sa 40 dk < 1 gün → CEVAP 5 gereği **bu kartta**, gece.

## Süre ve sıra

Tahmin 4,5–5 iş günü **kabul**; kısaltmak için kalite kesilmez. Paralel yürüyebilenler: DUR-7 PR'ı şimdi; INT-02 kampanya hazırlığı kod bitmeden; T-hücre gece koşusu mutasyonla aynı gece. Mutasyon hedefli (tavan dalı, sahiplendirme, kopya kuyruğu sırası, RTT örnekleyici, ESI deliği) ama atlanmaz.

Sıra: DUR-7 PR → cevapların nota işlenmesi + push + SHA → **"K4G-05b ONAYLANDI"** (benden, ayrı) → kod + D1/D2 regresyon → mutasyon → tam kapı → PR → T-hücre → INT-02 (10 × 600 s + 5 × (60 s kesinti + dosya), aynı oturum) → rapor → **"INT-02 GEÇTİ"** (benden) → K4G-16.

S2-A oturumu benim tarafımda; bugün açılıyor. Yaaz'a S4 v1.3.1 + hub TDD v0.6.1 + deploy TDD v0.5 + backlog bugün gidiyor; Y3/Y4 iskeleti başlar. Hayri'nin dalı bu oturumun işi değil.
````

---

## CEVAP PAKETİ 7 — EK — 2026-10-18

*Kaynak:* sahibin K4G oturumuna yapıştırdığı metin, birebir (oturum kaydından çıkarıldı); sha256 `d5315b24c3c6924bbaad2fa41b1ad896054cad3e724c3b612bae1055ae6df683`

````markdown
# CEVAP PAKETİ 7 — EK: K4G-05b tasarım notu incelemesi (Abkarada → K4G orkestratörü, 2026-10-18)

Not (`docs/design/K4G-05b-RETX-FIX.md`, `k4g/05b-retx-fix` @d6d60be8, taban d46ec628) tam okundu: §0–§9, Ek A (A.1–A.8), Ek B. CEVAP 5'in şartları karşılanıyor: değişmez cümlesi §1 ve A.7'de birebir; D1/D2 testleri değişmezi sahte saatle doğrudan sınıyor ve tabanda kırmızı (kontrol kolu dahil); RTT kabul kuralı CEVAP 5 kuralının üzerine (d)/(e) ile sıkılaştırılmış, kuantum döngüsü değişmiyor; RTO_MAX koddan belgeye, yeni sayı yok; kimlik değişikliği kasıtlı ve beyanlı; tel/codec/tv_* değişmiyor; silme kapısı 0/0; T-hücre tam seti 5 sa 40 dk < 1 gün → bu kartta; INT-02 10/10 + 5/5 aynı oturum + dosya kolu.

**"K4G-05b ONAYLANDI" bu mesajda da yok.** CEVAP PAKETİ 7'nin DUR cevapları (A bölümündeki iki düzeltmeyle) Ek B'ye işlenir, A.8 koşulsuz olur, B1–B8 nota girer → push → SHA bana → onay bugün.

---

## A. CEVAP PAKETİ 7'ye iki düzeltme (tam notu okuyunca)

**A1 — Bellek değişmezi (DUR-2), geri çekildi.** CEVAP 7'deki "birim başına O(1) durum, deneme listesi yok" şartı **kalkıyor**: ACK seq'e göre geldiği için iletim kayıtları (`loss.rs:210`) gerekli, O(1) istemek ACK anlambilimini değiştirmek olur; bu kart değil. Karar **DUR-2 (a)**, üç ekle: (1) A.7'deki beyan sayısal kalır ve **doğrultuyu yazar**: v1'de aralık tavansız olduğu için büyüme logaritmikti, v1.1 RTO_MAX ile **doğrusal** (birim başına 30 s'de ≤ 1 kayıt; 60 s INT-02 kesintisi ≤ ≈ 1000) — canlılık için bilinçli takas; (2) **AMENDMENT-9 (K = 1024) uygulama kartı** Sprint 2 backlog'una, **DEMO-01 ön koşulu** ("saha öncesi" değil), sahibi bu oturum, INT-02 GEÇTİ sonrası; (3) D2 testine dar bir ek iddia: 600 s sahte-saat kesintisinde kayıt sayısı beyandaki doğrusal sınırı aşmaz.

**A2 — DUR-6, netleşti.** "Ağaç dışı kopya" depo dışı demekmiş → **hayır, kopyalar depo içinde**: `reports/transport-v1.1/tcell/tools/` altında, yalnız etiket + ROOT farkıyla; orijinal ve kopya sha256'ları ön-kayıtta. Faz-5 betikleri bayt-özdeş. `reports/transport-v1.1/tcell/*` izni **verildi**. Ön-kayda: koşulan ağacın `Cargo.lock` SHA'sı, çekirdek 7.2.8-8-omarchy-bore (T-hücre + INT-02 aynı çekirdekte), rakip kolların aynı ortamda yeniden koştuğu.

## B. Onay öncesi nota girecekler

| # | Yer | Bulgu | İstenen |
|---|---|---|---|
| **B1** | A.4 §13, §3 madde 5 | "Birim BURST_CAP'ten büyükse tur sonraki birime geçer" istisnası: bu birim **ne zaman ve hangi yoldan** gönderilir? Her turda atlanan sıra başı kalıcı sahipsizdir → INV-NO-ORPHAN ihlali. | Ya imkânsızlık gösterilir (kayıt ≤ MTU ≤ BURST_CAP her zaman; satır atıfları) ve istisna metinden çıkar, ya da delik kapanır: burst-cap alt sınırı tx_queue başı gibi (:2310) **kopya kuyruğu başını da** kapsar. Notta hangisi, testle. |
| **B2** | §3 madde 3/5, A.7, "dup_queue ≤ 3/birim" | D2 kuyruk şişmesiydi (94→261, 104→335). Birim başına kuyrukta hangi yollar giriş tutabilir (ARQ yeniden gönderim / fire-once / geometrik), kuyruğa eklemede **dedupe kuralı**, "3"ün türetimi yazılı değil. | Bir paragraf: birim başına giriş ≤ N, yol başına ≤ 1, eklemede dedupe; N mevcut sabitten (`emergency.rs:121` kaynağıyla). A.7 "birim başına ≤ 1 geometrik kopya kaydı" ile tutarlı. |
| **B3** | §4 kimlik | `kama-v1-full`→`kama-v1.1-full`, `transport-v1`→`transport-v1.1` dizgeleri **tele, Noise prologue'a, CCR'a veya herhangi bir el sıkışma alanına çıkmıyor** beyanı yok. | Tek satır beyan, satır atıfıyla. Çıkıyorsa kart değil, DUR. |
| **B4** | §6 mutasyon | Hedef listesinde ESI deliği (madde 1) ve `adopt_unowned` H eşiği yok. | İkisi eklenir; 38 sayısı güncellenir. |
| **B5** | §6 D2 testi | Tek kol: saf kesinti-sonrası drenaj. | İkinci kol: pacer hızının **altında** eşzamanlı yeni DATA varken sıra başı yine önce ve sınırlı sürede. Sınıflar arası doygunluk (DUR-9 b) bu testin konusu değil — notta yazılı. |
| **B6** | Ek B DUR-1 | (a)'nın artığı: > 16.78 s tutulan ilk ACK bayat tohum verebilir. | Soğuk-başlangıç regresyon testine bu durum eklenir: bayat tohum sonrası srtt N örnekte toparlanır (N notta; sınır RTO_MAX). |
| **B7** | A.1, A.3, A.4, A.5, A.7 | "Sahip kararı CEVAP PAKETİ 5" atıfı tarihsiz ve depodan çözülmüyor. | Atıf tarihli ("CEVAP PAKETİ 5, 2026-10-16") ve karar metinleri depoda (`docs/decisions/` ya da CEVAP 4–6'nın işlendiği yer; yoksa bu PR'da eklenir, yol PR gövdesinde). |
| **B8** | Ek B DUR-4 | `kama-integration/tests/common/dr_cs3_panel.rs:996-1001` "F3/F4 hâlâ fire-once": atıf mı, iddia mı? | İddiaysa düzeltmeyle kırmızıya düşer → DUR-8 gibi beyanlı, 2×2 matrisle; atıfsa DUR-4 (b) işaretçisi yeter. Notta hangisi. |

## C. Olduğu gibi kabul

§1 okuma (sessizlik = canlılık kopyası, INV-001 korunur) · §3 madde 1–10 (B1/B2 dışında) · §3 sınırlar (ilk yeniden gönderim ≤ 60 s prob açık / H; H kanal-türevi, RTO_MAX'la sınırlanmaz) · §4 dosya listesi (api_surface pin güncellemesi yok, beyanla) · §5 Ek A kapı denetimi · §5b ölçüm sonuçları ve önerisi (hız işi yok; değişmez RecoverFwd'den bağımsız) · §6 kanıt planı (tabanda kırmızı, değilse DUR; mevcut testler + tv_* değişmeden yeşil, kırmızı = DUR; T-hücre tam set bu kartta, KATMAN 6 kalemi yok) · §7 süre · §8 sıra + DUR-7 (a) önce · Ek A metinleri (B1/B7 düzeltmeleriyle) · DUR-3 (b) → A.8 koşulsuz · DUR-4 (b) · DUR-5 (a) + PHASE5 "Aşama A 5 sa 20 dk" etiketi aynı ERRATUM PR'ında · DUR-8 (a) CEVAP 7'nin dört koşuluyla (2×2 matris; eski kod + yeni fikstür kırmızıysa nedeni DUR olarak gelir) · DUR-9 (a) · DUR-10 (a).

## D. KATMAN 6 / Sprint 2 backlog'una (bu kart değil)

- Ürün yolu `RetxConfig::fixture` (cap 3, **kalibre değil**) ile çalışıyor → adlandırma + kalibrasyon kartı (DR-1x kampanyasıyla birlikte).
- HEAD bench aracı v1 arşiv manifestlerini `canonical_kama=false` sayacak → iki kimliği tanıyan araç (M-sonrası).
- AMENDMENT-9 K = 1024 uygulaması — **DEMO-01 ön koşulu** (A1).
- DUR-4 bayat atıflar doküman PR'ı (INT-02 GEÇTİ sonrası; tam liste K4G-05b PR gövdesinde).
- ZM-2 4-decade: uzun pencereli hücreler (0.25k/1k/100k/1M, D 30 s'ye ulaşan, N ≥ 30) ön-kayıtlı gece kartı, DEMO-01 öncesi.
- DUR-9 (b) sınıflar arası HOL yuvası — INT-02 dosya kolu açlık gösterirse, ayrı §13 kartı.

## E. Sıra

1. `k4g/05-int02` PR'ı **şimdi** (onay gerekmez; "davranış değişikliği yok, yalnız gözlem; `ClientLiveDiag` aynası maliyeti ölçülmedi" beyanı; tam kapı yeşil) → merge → `k4g/05b-retx-fix` rebase.
2. Not revizyonu: Ek B kapanır (CEVAP 7 + bu EK), A.8 koşulsuz, B1–B8 işlenir, `<ONAY-TARİHİ>` boş kalır → push → SHA + Ek A yeni sha256 bana tek satır.
3. **"K4G-05b ONAYLANDI"** benden, ayrı mesaj → §8 sırası → **"INT-02 GEÇTİ"** benden → K4G-16.
````
