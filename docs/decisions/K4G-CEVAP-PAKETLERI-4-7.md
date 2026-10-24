# K4G sprinti — sahip karar paketleri CEVAP 4–7 (kayıt) · güncel kapsam CEVAP ~~4–9~~ 4–10 ve üç onay (dosya adı değişmez)

> Bu dosya K4G-05b tasarım notunun (`docs/design/K4G-05b-RETX-FIX.md`) ve K4G-15/K4G-05 PR'larının atıf verdiği **sahip kararlarının** depodaki kaydıdır (CEVAP PAKETİ 7 — EK B7). Metinler değiştirilmeden aktarılmıştır; her bölümün başında kaynağı ve metnin sha256'sı yazılıdır. Karar mercii Abkarada'dır; bu dosya karar üretmez. Kapı ifadeleri yalnız sahibin mesajında geçerlidir; burada geçmeleri gerçekleşmiş kapı anlamına gelmez. CEVAP PAKETİ 8 (2026-10-19) ve CEVAP PAKETİ 9 (2026-10-24) bölümleri 2026-10-24'te K4G-05b rev4 ile eklendi; dosya adı mevcut atıflar için korunur. CEVAP PAKETİ 10 (2026-10-24) ve aynı gün verilen üç onay metni (K4G-05b, K4G-05c, K4G-05d) 2026-10-24'te K4G-05b onayıyla eklendi; ikisinin kaynağı orkestratör transkripsiyonudur (bölüm başında).

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

---

## CEVAP PAKETİ 8 — 2026-10-19

*Kaynak:* dosya `~/Downloads/CEVAP-PAKETI-8-2026-10-19.md` (sahip), bayt-özdeş; sha256 `a3cc8505dc11931938f091a4181dda0202058ec4cde583385d20a3540306c822`

````markdown
# CEVAP PAKETİ 8 — Abkarada → K4G orkestratörü (2026-10-19)

19 Ekim özetin (OZET-2026-10-19.md) ve K4G-05b notunun revizyonu (`k4g/05b-retx-fix` @c4dfd1fd) okundu. Bu mesaj DUR-CI-2, DUR-11..15 ve §7 listendeki K4G'ye düşen kalemleri kapatır. **"K4G-05b ONAYLANDI" ifadesi bu mesajda değil, ayrı mesajdadır** (bugün). "INT-02 GEÇTİ" yine ayrı.

---

## 1. DUR-CI-2 — CEVAP: önce yeniden tetikle; başlamazsa istisna açık

Kök neden tahmini: GitHub Actions harcama limiti/kredisi (12 Ekim'deki kilitle aynı sınıf; 18 Ekim 11:34 UTC'den beri `startup_failure`, workflow dosyaları değişmedi). **Bugün hesaba 50 $ Actions kredisi eklendi.** Sıra:

1. `core-main`'i main'de elle tetikle; #136 ve #135 kontrollerini yeniden koştur (`gh run rerun` ya da "Re-run all jobs"). **Başlarsa:** DUR-CI-2 = (a) kapandı, istisna açılmaz; `core-main` yeşilse plan §4.2 (iii) sağlanmış sayılır, raporda tek satır.
2. **Hâlâ `startup_failure` ise:** bu mesajla **CI-KİLİT İSTİSNASI yeniden açıktır** (plan `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` §4.2): neden GitHub Billing'de teyit edildi — Actions kredisi/limiti 18 Ekim'de tükenmişti, 19 Ekim'de kredi eklendi, hesabın işlemesi bekleniyor. Önce #136, sonra #135 yerel tam kapıyla merge (`k4g-gate.sh` çıktısı + `summary.txt` PR gövdesinde, "CI-KİLİT İSTİSNASI ile merge" satırı). Kilit kalkar kalkmaz `core-main` elle; kırmızıysa ACİL DUR + revert. İstisna kilitle biter.
3. Dakika tasarrufu — **bu kartta workflow değişmez**, K4G-16 / Sprint 2 Y4'e kalem: PR workflow'unda `concurrency` + `cancel-in-progress`; yalnız `docs/**`/`reports/**` değişen PR'larda cargo adımları atlanır (`paths-ignore`); Rust derleme önbelleği. Ayrı küçük PR, "INT-02 GEÇTİ" sonrası.

## 2. K4G-05b notu @c4dfd1fd — inceleme sonucu

CEVAP 7-EK'in bütün maddeleri karşılandı: **A1** (O(1) geri çekildi; A.7 doğrusal büyüme + kesin biçim; AMENDMENT-9 DEMO-01 ön koşulu; D2 kol 3), **A2** (`reports/transport-v1.1/tcell/tools/` depoda, ön-kayıt içeriği), **B1** (imkânsızlık ispatlanamadı → BURST_CAP tabanı üç yolun başını kapsar; PreConverge v1 tabanı beyanlı), **B2** (G ≤ 1, F ≤ D−1, N = D_max = 3 türetimi), **B3** (dizgeler tele/prologue'a/CCR'ye çıkmıyor, satır atıflı), **B4** (56 mutant, ESI + H dahil), **B5** (D2 kol 2, DATA ≤ min_rate/2), **B6** (soğuk başlangıç kol B, N = 64), **B7** (`docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`, tarihli atıflar), **B8** (`lineage` referans; `f3`/`f4` panel koşulur). Ek A 171 satır, sha256 `b68340c5…`; silme kapısı 0/0.

**DUR-11..15 CEVAP** (hepsi notun kalın seçeneği → Ek A sha256 değişmez):

| # | Cevap | Not |
|---|---|---|
| DUR-11 | **(a)** | A.7'deki kesin biçim (m + ⌊Δ/RTO_MAX⌋) normatif; "≈ 1000" sahip tahmini olarak etiketli kalır; "≤ 1" rampa sonrası eğim. D2 kol 3 fikstür sayısı PR gövdesine; INT-02 raporu gerçek U ve kayıt sayısını yazar — tahmin orada ölçümle değişir, spec'te değil. |
| DUR-12 | **(a)** | ARQ ve prob yollarındaki aynı delik bu kartta kapanır (+2 kol, +2 mutant = 56). Gerekçe: değişmez "izleyen yol yeniden gönderebilmeli" diyor; iki yolu açık bırakmak değişmezi yarım uygular. |
| DUR-13 | **(a)** | N = D_max = 3; mevcut sabitlerden türetildi (`DEFAULT_DUP_FACTOR` 2, `dup3_factor` 3), yeni sayı yok. Ölçümde sapma → DUR. |
| DUR-14 | **(a)** | Beyan. CEVAP 7'deki "ilk örnek zaten RTO_MAX ile sınırlı" cümlem **yanlıştı**, geri çekildi. Yerine soğuk-başlangıç testinin kol B'si (bayat tohum → N = 64 kabulde yakınsama) geçer; (b) DUR-1 (a) ile çelişir, reddedildi. |
| DUR-15 | kural şimdiden | `dr_cs3_panel` düzeltmeyle kırmızıya düşerse: kırmızı yalnız v1.1'in **kasıtlı** değiştirdiği davranışı (fire-once, tavan) iddia eden satırdansa → iddia güncellenir, DUR-8 biçiminde (test başına paragraf, 2×2 çıktı, mutasyon) + DR-CS-3 kaydına YAMA KAYDI; eşik/amaç başka nedenle değişiyorsa DUR, kod beklemez — o test atlanmadan diğerleri sürer, PR o DUR kapanmadan açılmaz. |

**Onay biçimi:** onay mesajım c4dfd1fd içindir; #136 merge sonrası rebase'in yalnız §0'daki satır ofsetlerini kaydıracağını, spec ve Ek A hedeflerinin değişmediğini kabul ediyorum — rebase sonrası SHA raporda tek satır. Kod, onay mesajı gelince, CI'yi beklemeden başlar (testler önce tabanda kırmızı); PR açılışı #136'nın merge'ünü bekler.

## 3. #135 ERRATUM-2 — kabul

Belge düzeltmesi doğru ve kanıtlı; CI açılınca (ya da §1 istisnasıyla, #136'dan sonra) merge. Ek kalem **K4G-16'ya:** §4'teki desen ("alan yok" → 0 okundu, "vakum sayaç") daha önce de görüldü (D0 kaynak denetimi, `first_probe_tick`). Analiz betiklerine "alan yoksa HATA, sıfır değil" kuralı ve manifest şeması ↔ `c.err` alan eşleşmesi denetimi — ayrı küçük PR, INT-02 sonrası.

## 4. §7 listendeki diğer kalemler — K4G'ye düşenler

- **Hasan'a release bağlantısı:** benden gidiyor. README-HASAN'daki iki eksik pratik not (loopback'te iki düğüm `connect`'ten önce başlatılır; manifest INTERNET izni) → K4G-16'da README'ye iki satır; release yeniden yayımlanmaz, release notuna bir satır.
- **Dedup tablosu bağlı değil → yeniden başlatmada aynı mesaj iki kez görünebilir** (`kama-app/src/messaging.rs:120`): demo riski. KATMAN 6'ya kalem; Sprint 2'de Hayri'ye küçük kart (benden). K4G kodu dokunmaz.
- **KAPSAM K3 "0-RTT ilk mesaj" ↔ kod (ertelendi):** K4G-16 belge PR'ında YAMA KAYDI (üstü çiz, "0-RTT kodda yok, ertelendi — K4G kararı"). **G-IMPL-S3 "BEKLİYOR"**, **CLAUDE.md "8 crate"**, S1/S3 "SEAM-FROZEN HOLD" aynı PR'da.
- **"İki katman" netleştirme:** CAD-18'deki PKI bağı kastediliyor (Noise anahtar sahipliği + sertifika zinciriyle kimlik); iki AEAD değil. Metin değişmez.
- **22 B:** küçültme yok, DR açılmaz. PTT başına asıl kaldıraç 11 B ara çerçeve (BatchPayload) ve `bundle_ms` — Sprint 2 dışı.
- **Medya anahtarı (a/b/c), PTT ses yolu sahipliği, KM-SEAM-AM, arka plan (wake lock/Doze), O-20:** Sprint 2 kararları, bu oturumun işi değil; Resul/Hasan'la ben kapatırım, sonuç S2-A'ya gider.
- **Yerel checkout:** `Desktop/kama-core`'u `git pull --ff-only` ile güncelle (izin verildi; izlenmeyen dosyalara dokunma).

## 5. Sıra (bugün)

CI yeniden tetikleme (§1) → ayrı mesajımla **"K4G-05b ONAYLANDI"** → kod (tabanda kırmızı testler önce) → #136 merge → rebase + SHA → #135 merge → PR → mutasyon → T-hücre (gece) → INT-02 → rapor → benden **"INT-02 GEÇTİ"** → K4G-16.

Rapor: CI durumu tek satır (başladı / başlamadı + ekran görüntüsündeki hata metni); onaydan sonra gün sonu tek rapor.
````

---

## CEVAP PAKETİ 9 — 2026-10-24

*Kaynak:* dosya `~/Downloads/CEVAP-PAKETI-9-2026-10-24.md` (sahip), bayt-özdeş; sha256 `8d0ce2af168d1b5eb28eccc2cd2c837f9d7e9a524a7c42f386fb4add681f85bb`

````markdown
# CEVAP PAKETİ 9 — Abkarada → K4G orkestratörü (2026-10-24, v2) — bağımsız inceleme + senin 10 doğrulayıcın sonrası plan revizyonu

**Girdi:** `Kama_Adversarial_Inceleme_2026-10-23.md` (incelemeci; kaynak okuması, cargo yok) + senin bağımsız doğrulaman (10 iddia: 9 DOĞRU, DEVIR KISMEN; `verify-wf_f84f4d0e.json`). Doğrulama kabul; daraltmaların da kabul. Bu mesaj DUR-16..19'u cevaplar, iki yeni kart açar, notun rev4 içeriğini verir, INT-02 kapısını günceller. **Kapı ifadesi bu mesajda yok.**

**Çalışma kuralı:** her düzeltme önce **tabanda kırmızı testle** (P0). Doğrulayıcı "DOĞRU" dediği için değil, test kırmızı olduğu için kod yazılır. Yanlışlanan madde nota "yanlışlandı: <test>" diye girer.

---

## 1. DUR-16..19 CEVAP

| # | Cevap |
|---|---|
| **DUR-16** (R1/R2/R3 + tablo sızıntısı kapsamı) | **(b)** — iki ayrı kart: **K4G-05c** alıcı kabul bütünlüğü (R1 + R2 + sızıntı; `client-core`), **K4G-05d** bekleyen-receipt kuyruğu (R3; `kama-app`). Kartlar §3'te. "Reliable düzeldi" üçü merge olmadan söylenmez. **"INT-02 GEÇTİ" üçünü de bekler** ve INT-02'ye R2'yi gerçekten tetikleyen kol eklenir (§5) — "INT-02 düzeltmesiz de geçer" olasılığı kapıyı boşaltmasın. |
| **DUR-17** (not revizyonu) | **Evet.** Beş düzeltmen + §2'deki rev4 maddeleri işlenir, dala push, bana tek satır SHA + Ek A sha256. **Onay rev4 SHA'sına yazılır.** 19 Ekim onay metni oturuma yapıştırıldıysa rev4'le yenilenir; yapıştırılmadıysa ilk onay rev4'e. Kod rev4 onayından önce yazılmaz (değişmedi). |
| **DUR-18** (R2 statüsü) | **K4G-05c'de kapatılır; beyanlı sınır olmaktan çıkar.** Gerekçe: düşen birimin arkasındaki teslimlerin 4096 birime dek beklediği yeni bulgu + gerçek tetiğin düğüm takılması olması (Kotlin dinleyicisi, disk — Android arka planında gerçekçi). DUR-K4G-01 (i) kararı geri çekildi: plan:532 satırına YAMA KAYDI ("2026-10-24: beyanlı sınır kaldırıldı → K4G-05c"); `runtime_e2e.rs:288`'deki davranışı sabitleyen test DUR-8 biçiminde (paragraf + 2×2) düzeltilir. INT-02 `delivery_channel_drops == 0` şartı kalır, artık **yapısal** sağlanır. |
| **DUR-19** (#136) | (i) `INT-02-RAPORU.md:127` cümlesi merge öncesi düzelir: "ürün koduna salt-okunur canlı ayna ve teşhis yazıcısı eklendi; motor kararları değişmedi; maliyeti ölçülmedi". (ii) Teşhis yazıcısı ve `ClientLiveDiag` tur-başı yazımı **çalışma zamanı bayrağının** arkasına (`RuntimeConfig` alanı, varsayılan **kapalı**; tek derleme, feature değil — INT-02 ve ürün aynı ikiliyi koşsun); INT-02 düzeneği açar. (iii) Birleşik adayda **bir** karşılaştırma koşusu bayrak açık/kapalı (gating2, 2 tohum) raporda. #136 bu üç değişiklikle merge; "davranış değişikliği yok" iddiası bayrak kapalıyken geçerli. |
| **A3 sırası** | **Değişti** (S2-A'ya ayrıca yazıldı): A3'ün birinci parçası (yeni dosya `hub_link.rs`, sahte hub conformance) şimdi; `runtime.rs`/`transport.rs` entegrasyonu **K4G-05c merge'ünden sonra**; A3 DONE ve INT-03 yine "INT-02 GEÇTİ" sonrası. Senin dosyaların A şeridine kapalı (çakışma haritası aşağıda). |

**Numaralandırma:** DUR-16..19 seninkiler; benim yeni kararlarım **DUR-20** (K_MAP) ve **DUR-21** (PTT/dosya) — §2'de.

## 2. K4G-05b rev4 — nota girecekler (senin beş maddenle birlikte)

| Konu | Rev4 metni / kuralı |
|---|---|
| **D1 fikstürü** (senin iki ön koşul) | Test PreConverge **dışında** (önce RTT kurulur); kurban birim taze bir ACK'ten hemen sonra, **en yüksek seq** ile gönderilir ve ondan sonra ACK'lenen paket yoktur; **yeni uygulama verisi yok** (kusur oturum düzeyinde: başka birimin ACK'i prob bütçesini yeniler — notta böyle yazılır); her yeniden emisyonda runtime'da **payload mevcut** iddiası. |
| **INV-NO-ORPHAN üç kabul koşulu** | A.7 okumasına: birim kayıtlı · payload mevcut · hizmet verilebilir bütçede yeniden seçilebilir. `confirmed_units` dışında hiçbir yol payload'u silmez (test). |
| **ESI** | Madde 1 bütün yolları kapsar (ACK/tombstone/un-loss eşleşmesi, **prob hedefi**, RECOVERED); iç tipler `UnitId` / `Seq` / `Fec{block, esi}`. **Yalnız-parite kaydının akıbeti tanımlanır:** parite Reliable birim değildir, sahiplenilmez; kaydı kendi ACK'i, kayıp ilanı ya da süre dolumuyla kapanır; ARQ/prob'a girmez; runtime tablolarına girmez. Test: parite ESI = `UnitId` 0..3, uçuş + tombstone, en eski kayıt parite. |
| **BURST_CAP (senin BURST)** | A.4 **tur başı taban kuralı**: `sender.rs:2322` tabanı = max(tx başı, ARQ planının ilk kaydı, bekleyen prob hedefi, ilk vadeli kopya kaydı) — **ilk kırpmadan önce**; token üretilmez, birikime izin; seçim ve uyanma aynı başları kullanır. Yol girişinde yükseltme cümlesi çıkar. Test: doğrulayıcının modeli (37 B / 42 B / 1050 B, 600 s) tabanda kırmızı. |
| **Sınıflar arası pay (DUR-9 a → b-lite)** | Vadesi gelmiş kopya kuyruğu başı en geç her `K_ARQ_SHARE`'inci turda DATA'dan **önce** hizmet alır (RECOVER_ARQ'daki mekanizma kopya kuyruğuna genişler; **yeni sayı yok**, ayrı kova yok). A.4 "sınıflar arası sıra DEĞİŞMEDİ" cümlesi bu tek istisnayla. Test: pace'i dolduran DATA + eski kopya → ≤ K_ARQ_SHARE tur. Gecikme sözü yok. |
| **RTT (senin RTT)** | KABUL KURALI rev4: örnek, çerçevenin **en büyük yeni-onaylı seq'inin kendi kaydından** (a); ilk kez onay (b); kaydın kökeni Data / Retransmit / Duplicate olabilir, zaman **o iletimin** gönderim anı (her iletim taze seq alır → tek anlamlı; CEVAP 5'teki Karn dışlamam gereksizdi, kalkar); FEC parite örnek vermez; (d) doygun `ack_delay` yok; (e) guard. **srtt-yok istisnası (DUR-1 a) kalkar**, DUR-14 düşer. **Tazelik ölçütü:** son kabul edilen örneğin yaşı ≤ max(10 × srtt, RTO_MAX); aşılırsa `rtt_stale` sayacı (INT-02 raporlar). Testler: yol RTT 0.4 → 2 s kalıcı artış, kopyalı trafik → yakınsama; kopya/FEC ağırlıklı 600 s → örnek yaşı sınırda; soğuk başlangıç A/B/C kilitlenmez (kilitlenirse DUR). Teslim / BW / delivered-kredi RTT kararından bağımsız (test iddia eder). m12'deki 60.5 s'nin `srtt` alanı olduğu tek satırla teyit. A.1/A.3 değişir. |
| **Sıfırlama kapsamı** | İlk ilerlemeli ACK vadeleri tabana döndürür; emisyon pace + BURST_CAP'e tabi (INV-RATE-1) — test: tur başına emisyon ≤ bütçe, sıra HOL. |
| **Devir zamanı (senin DEVIR)** | Normatif: devir **bütçenin tükendiği turda, hemen**; A.7'nin "en geç sonraki prob vadesi" cümlesi çıkar. "≤ 60 s" **uygunluk** sınırı olarak, zamanlayıcı başlangıç anı ve varsayımlarıyla yeniden türetilir; gerçek emisyon token'a bağlı, D1 testi uygunluk ve emisyonu **ayrı** raporlar. |
| **Kaynak / tarihçe (DUR-20)** | CEVAP 7-EK A1'deki "AMENDMENT-9 DEMO-01 ön koşulu" kararım **geri çekildi** (K = 1024 hop-ACK'siz PN tavanı; hub fallback ister; bellek sınırı değil — doğrulandı). Yerine (madde 11): birim başına iletim eşlemesi ve zamanlama kaydı ≤ **K_MAP**, en eski budanır; **geç-ACK ufku = K_MAP × RTO_MAX** beyanlı; ufuk dışı geç ACK → Expired/Unknown: birimi **teyit etmez**, birim sahipli kalır, yeniden gider, RTT vermez. Kopya işi ≤ 3 (var); payload ≤ store (var); alıcı tampon → 05c; **teyit sonrası bayat motor kararının runtime tablolarına eklenmesi** (`runtime.rs:1469-1470` sızıntısı) → kaynağı 05b madde 2 (`confirmed_units`: teyitli birim için karar üretilmez), bekçisi 05c (runtime teyitli/olmayan birim için kayıt eklemez; mevcut sızıntı temizlenir). A.7 "doğrusal büyür" → "sınırlı; ufuk K_MAP × RTO_MAX". **DUR-20 CEVAP: K_MAP = 8** (ufuk 240 s = 8 × RTO_MAX; türetim notta: desteklenen azami RTT + bir tam RTO_MAX; sapma gerekçesi varsa DUR). |
| **Gözlem** | DUR-19 (ii)/(iii). |

## 3. Yeni kartlar (ayrı notlar ≤ 1 sayfa + yama metni bana; kod "K4G-05c ONAYLANDI" / "K4G-05d ONAYLANDI" bekler)

**K4G-05c — Alıcı kabul bütünlüğü (R1 + R2 + sızıntı)**
- Neden: Alıcı tutamadığı veriye ACK veriyor (kanal dolu) ve arkasındaki teslimler 4096 birime dek bekliyor; tampon dolunca eksik birimi atlıyor; teyit sonrası bayat kayıtlar runtime tablolarında birikiyor. Üçü de gönderen payload'u sildikten sonra sessiz kayıp ya da oturum ömrü boyunca büyüme.
- Ne: (R2) **tek kabul kararı** — oturum gelen-kutusunda yer ayır + payload'u tut + dedup'u ilerlet + ACK'e uygun işaretle; dördü birlikte ya da hiçbiri; yer yoksa o PN **ACK'lenmez** (SACK boşluğu; gönderen yeniden gönderir), dedup ilerlemez, sayaç. `Receiver`'a kabul kararı girdisi (iç API; tel değişmez). Bloklayan `await` yok; gelen kutusu oturum başına sınırlı, **sıradaki eksik birim için yer ayrılı**; FEC RECOVERED ve parçalı mesaj aynı kural. (R1) `Reorder` eksik Reliable birimi **asla atlamaz**; `held > cap` gönderen ihlalidir → en yeni pencere-dışı birim ACK'siz düşer + `reorder_skip` sayacı. Gönderen: yeni birim kabulü **birim kimliği mesafesiyle** sınırlı (en yeni − en eski onaysız < alıcı cap − pay) → `Backpressure`; cap ürün sabiti (mevcut 4096), pay = kopya işi sınırı (3) × … notta türetilir. (Sızıntı) runtime, teyitli ya da bilinmeyen birim için `seq_to_unit`/`unit_seqs` kaydı eklemez; teyitte temizler; mevcut sızıntı için test (oturum boyu tablo boyutu ≤ sınır). Eski testler (`transport.rs:1323-1338` atlama; `runtime_e2e.rs:288` düşürme) DUR-8 biçiminde beyanlı.
- Nereye: `crates/client-core/src/{transport.rs, runtime.rs}`, `crates/adaptive-transport/src/receiver.rs` (kabul girdisi); otoriter belgeye YAMA KAYDI (hangisi, notta); `s1.rs` dokunulmaz; tel/codec/tv_* değişmez.
- Bağımlılık: K4G-05b rev4 PR merge; P0 testleri tabanda kırmızı.
- Done: R1/R2/sızıntı regresyonları yeşil; INT-02 `ek-tuketici-durak` kolu: teslim %100, `delivery_channel_drops = 0`, `reorder_skip = 0`, dedup yanlış ilerlemedi, kontrol yolu çalıştı; mevcut testler + tv_* yeşil; mutasyon kabul kararı dalları.

**K4G-05d — Bekleyen RECEIPT kuyruğu (R3) [+ isteğe bağlı DUR-21]**
- Neden: `send_receipt` tek deneme; Backpressure'da receipt unutuluyor; canlı oturum 7 güne dek sürebildiği için "teslim edildi" o süre gelmiyor (yanlış "teslim edildi" yok). Tetik: aynı eşe büyük dosya giderken kuyruğun dolması — demoda gerçekçi.
- Ne: `(peer, msg_id)` ile tekilleştirilen **sınırlı** bekleyen-receipt kuyruğu; ilk başarılı transport kabulüne dek kapasite açıldığında yeniden sunulur; `Ok` sonrası tekrar transport'un. İkinci ağ ARQ'su **değildir** (zamanlayıcı yok). Sınır = outbox eş başına sınırı (256; yeni sayı yok). "Tek deneme" açıklaması ve testleri beyanlı güncellenir; `flush_receipts` süre dolumu kopyalanmaz. **DUR-21 CEVAP: madde karttan ÇIKAR** — PTT açıkken dosya parça sınırı için sayı yok; karma trafik INT-02'de ölçülür, karar Sprint 3.
- Nereye: `crates/kama-app/src/messaging.rs` (+ test); AMP/S1 dokunulmaz.
- Bağımlılık: yok (05b/05c'den bağımsız).
- Done: Backpressure → kapasite → yeni oturum olmadan teslim durumu tamamlanır; kuyruk sınırlı; INT-02 sonunda bekleyen receipt 0; mevcut testler yeşil.

## 4. P0 — kanıt ve taban (1 gün; kod düzeltmesi yok)

Tabanda kırmızı beklenenler: D1 (rev4 fikstürü) · ESI (0..3, uçuş + tombstone, en eski parite; prob pariteyi hedefler) · R1 (cap 3; `1,2,3,4,0`) · R2 (tüketici durur → kanal dolar → payload düşer, PN ACK'lenir, dedup ilerler; arkası bekler) · R3 (Backpressure → kayıt yok) · BURST (37/42/1050, 600 s) · sınıflar arası (doygun DATA + eski kopya) · sızıntı (teyit sonrası tablo büyür). Tabanda **yeşil** beklenen (rev4'te de yeşil kalmalı): RTT yol değişimi 0.4 → 2 s. Yalnız rev4'te: sıfırlama patlaması, tazelik. Değişecek eski testler listesi (atlama, düşürme, DR-CS-1 üçlüsü) nedenleriyle PR öncesi bana. **Rapor: P0 tablosu tek mesaj.**

## 5. INT-02 kapısı (ölçüt eklendi; kaldırılan yok)

- Değişmeyen: gating2 10 × 600 s **10/10**; 60 s kesinti **5/5** aynı oturum; dosya kolu ölçülür; `delivery_channel_drops = 0`, app/crypto retx = 0, ctx/hub = 0.
- **Yeni kol `ek-tuketici-durak`:** alıcı dinleyicisi 30 s durur (kanal dolar), yeniden başlar; 50 TEXT akarken → teslim %100, `delivery_channel_drops = 0`, `reorder_skip = 0`, dedup doğru, kontrol yolu (ACK/heartbeat) duraklamada çalışmış — **3 tohum, 3/3**. R2'yi tetikleyen tek kol budur; 05c olmadan kırmızı beklenir.
- Yeni hüküm: koşu sonunda bekleyen receipt = 0; `reorder_skip = 0`; `rtt_stale` raporlanır (hüküm değil).
- `ek-dosya-ptt`: "bas→ilk ses" dosya altında raporlanır (ölçüt değil).
- Zaman çizelgesine: tarihçe tabloları boyutu, en eski onaysız birimin yaşı/sahibi/son gerçek gönderimi, token reddi nedeni, bekleyen receipt, RTT örnek sayısı/yaşı. Bayrak açık/kapalı karşılaştırma (DUR-19 iii).
- "INT-02 GEÇTİ" = 05b rev4 + 05c + 05d merge **ve** yukarıdakiler.

## 6. Sıra, tahmin, çakışma

P0 (1) → rev4 notu + onay → **05b** (2–3) → **05c** (2–3) → **05d** (0,5) → madde 11 / A.7 / §16.2 (1) → mutasyon + kapılar (1) → T-hücre gece → INT-02 → rapor → "INT-02 GEÇTİ". **≈ 9–10 iş günü → 5–6 Kasım**; senin yeniden hesabın gelince bu satır güncellenir. Her kart ayrı PR.

Çakışma haritası (A şeridi dokunmaz): `crates/adaptive-transport/**`, `crates/client-core/src/{runtime.rs, transport.rs}` (05c bitene dek), `crates/client-core/tests/transport_v1_freeze.rs`, `crates/kama-app/src/messaging.rs`, `crates/bench/**`, `.github/workflows/nightly.yml`. A şeridi yeni dosyalarda (`hub_link.rs`, `s4.rs`, `ref/hub/**`, `vectors/hub/**`, `s4_*`).

Çevre: `CARGO_BUILD_JOBS=2`, tek cargo, worktree başına target, ölçüm çıktısı `/tmp` dışında. CI başlamadıysa CEVAP 8 §1 (2).

Rapor: P0 tablosu → rev4 SHA + Ek A sha256 tek satır → 05c/05d notları → sonra gün sonu tek rapor.
````

---

## CEVAP PAKETİ 10 — 2026-10-24

*Kaynak:* oturuma yapıştırılan metin, 2026-10-24; orkestratör transkripsiyonu `~/kama-k4g-work/decisions/C10.md`, bayt-özdeş; sha256 `cd84bab11ead1b4a8a426d685a486d3dd75f3a24905c9b972eec081af0c2e6d4`

````markdown
# CEVAP PAKETİ 10 — Abkarada → K4G orkestratörü (2026-10-24) — P0 sonucu, rev4/05c/05d notları, DUR-22..31, DUR-05c-*, DUR-05d-*, bağımlılık PR'ı

P0 kabul: 12 testin 11'i tabanda beklenen nedenle kırmızı, RTT yol değişimi yeşil, yanlışlanan madde yok (`k4g/p0-taban` @df7c9e62). Üç not alındı: rev4 @ea2e57b2 (Ek A `27171e1c…`, 227 satır), 05c @18e6de95 (Ek A `efee869f…`), 05d @b9d83396 (Ek A `f3f62563…`). **Onaylar bu mesajda değil; üç ayrı mesajla, bunun hemen ardından geliyor.** CEVAP 8'in oturuma ulaşmamış olması benim hatam; bugünden itibaren her karar paketi dosya olarak gelir, sen alındığını tek satırla yazarsın.

---

## 1. P0 mesajındaki kararlar

| # | Cevap |
|---|---|
| **DUR-22** | Kabul: kırmızı test tek başına main'e girmez; her test kendi düzeltme PR'ıyla (05b / 05c / 05d) yeşil olarak girer, `#[ignore]` yok. `k4g/p0-taban` dalı kanıt olarak kalır (PR açılmaz); PR gövdeleri P0 koşusunun log yoluna ve bu dala atıf verir. |
| **DUR-23** | (a) kabul: motorda kopya kuyruğu sahipliğini okuyan salt-okur erişimci (05b; davranışsız). (b) kabul: runtime tablo boyutları için `#[cfg(any(test, feature = "test-util"))]` sayaç kancası (05c), notta önerildiği biçimde. İkisi de ürün yolunda karar okumaz. |
| **DUR-24** | Onay: sınıflar arası pay **her durumda** (PRE_CONVERGE dışı), tur = `schedule_transmit` çağrısı. RECOVER_ARQ sayacı yalnız o durumun ARQ payı için kalır; kopya kuyruğu payı ondan bağımsız sayılır. PAY testi bu okumayla yeşil olmalı. |
| **DUR-25** | Kabul: ESI birim testleri statik-r (Manual) kolunu kullanır (belirlenimli). Ürün kipi eşdeğerliği iddia edilmez; notta tek cümle. INT-02 FEC açık koşusunda parite ACK sayaçları raporlanır (hüküm değil). |
| **DUR-26** | Kabul: devir ACK sessizliğine dayanmaz; ana yol "bütçe tükendiği turda hemen". Sessizlik kontrol kolu, gerçek alıcının ≈ 8.4 s'lik bayat SACK tekrarını içerecek şekilde yeniden tanımlanır (notta). |
| **DUR-27** | **Evet.** Üç RustSec uyarısı için ayrı bağımlılık PR'ı (yalnız yama sürümleri: rustls-webpki 0.103.16, webpki-root-certs ≥ 1.0.10, tokio ≥ 1.53.3; yeni bağımlılık yok). CI-KİLİT İSTİSNASI bu PR'ı ve kilit sürdüğü sürece sıradaki **her** PR'ı kapsar (koşullar aynı: yerel tam kapı çıktısı PR'da, "CI-KİLİT İSTİSNASI ile merge" satırı, kilit kalkınca `core-main` elle, kırmızıysa ACİL DUR + revert). tokio ürün bağımlılığı olduğu için tam kapı + tv_* + entegrasyon testleri yeşil şart; `Cargo.lock` farkı yalnız bu üç satır. Sıra: bağımlılık PR'ı → #136 → #135. |

## 2. rev4'ün açtığı DUR-28..31

| # | Cevap |
|---|---|
| **DUR-28** (K_MAP türetimi) | K_MAP = 8 kalır. Türetim ("desteklenen azami RTT + bir tam RTO_MAX") 8'den **büyük** değer veriyorsa notta göster, DUR; küçük veriyorsa 8 kalır (ufuk 240 s). Ufuk dışı geç ACK kuralı değişmez. |
| **DUR-29** (RTT (a) okuması) | **Dar okuma:** `ack_delay` çerçevenin en büyük PN'ine aittir (`receiver.rs:405`); örnek yalnız **o kayıt** bu çerçeveyle ilk kez onaylanıyorsa alınır. En büyük PN zaten onaylıysa ya da bilinmiyorsa örnek **yok** — daha küçük yeni-onaylı kayda geri düşülmez. Tazelik ölçütü bu yüzden var. |
| **DUR-30** (madde 11 / A.7 / §16.2 adımının yeri) | Ayrı PR değil: motor tarafı K_MAP budaması + A.7 + §16.2 satırı **05b PR'ında** (Ek A zaten orada); runtime tarafı (birim başına O(1) kayıt, `take_confirmed_units` ile silme) **05c PR'ında**. C9 §6'daki ayrı adım kalkar. |
| **DUR-31** (PAY karışık boy) | Kabul: kopya kuyruğu başı için **token rezervasyonu** — baş token'a yetişemediğinde eksik kısmı rezerve edilir, DATA rezervin altına inemez, baş gidince rezerv kalkar; token üretilmez, rezerv ≤ bir kayıt boyu. A.7 (iii) buna göre; test: 37 B DATA sürekli + 1050 B kopya, karışık boy → kopya ≤ K_ARQ_SHARE turda. |

## 3. K4G-05c notunun kararları

| # | Cevap |
|---|---|
| **DUR-05c-1** | **(a′)**: çağrı = kabul (`receiver.rs` yalnız doc) **+** engine-spec §2:216'ya tek satır YAMA KAYDI ("`seen_count` = alıcı motorunun **kabul ettiği** DATA; kabul kararı istemci runtime'ındadır, v1.1 · K4G-05c"). Yetki bu satırla sınırlı. Beyanlı sapma bırakmayalım. |
| **DUR-05c-2** | **(a)** pay = 0 (notun türetimi). Kart metnindeki "3c" yer tutucuydu. |
| **DUR-05c-3** | Kabul, önerilen tarifle: durak (30 s) boyunca eşzamanlı dosya ≥ 1 025 birim (≈ 1,04 MB); profil 30 s'de taşımalı; **taban kolu** (05c'siz aday, 1 tohum) `delivery_channel_drops > 0` gösterir; geçerlilik: `full_refusals > 0`. Hüküm: teslim %100, `delivery_channel_drops = 0`, `reorder_skip = 0`, dedup doğru, kontrol yolu çalıştı — 3 tohum 3/3. Profil taşıyamıyorsa dosya boyutu değil **kanal kapasitesi test ayarı** değiştirilmez — DUR. |
| **DUR-05c-4** | **(a)** spec dokunulmaz; gerekçe notta kalır. |
| **DUR-05c-5** | **(a)** A.3 — plan:392 ve :532 ikisi de. |
| **DUR-05c-6** | **(a)** — `NodeMetrics`/UniFFI değişmez. **K4G-05e açıldı** (§5), not gerekmez. |
| **DUR-05c-8** | **(a)** R = min(`max_peers`, C − 1) = 64/1024; yalnız sıra başı garantisi; adalet sözü yok (beyan). |
| **DUR-05c-9** | **(a)** A.4. |
| **DUR-05c-10** | **(a)** `runtime.rs:3334-3372` ve `kama-node node/tests.rs:415-456` DUR-8 biçimiyle (paragraf + 2×2) değişir. |
| **DUR-05c-11** | **(a)** 05b'nin `kama-v1.1-full` beyanına 05c satırı; T-hücre birleşik adayda. |
| **DUR-23 (b)** | Kabul (bkz. §1). |
| **DUR-05c-12** | **(a)** protokol ihlali: ACK + düşür + `frag_chunk_dropped`; değişmez "geçerli birim" için; beyan notta ve INT-02 raporunda. |
| **DUR-05c-13** | **(a)**: bekçi yazılır; kanıt 3439fccd + kanca kırmızısı; 05c tabanında yeşilse nota "yanlışlandı (05c tabanında)". |

## 4. K4G-05d notunun kararları

| # | Cevap |
|---|---|
| **DUR-05d-1** | En eski düşer + `receipts_evicted` (öneri). Sınır 256 (eş başına outbox sınırıyla aynı; 2 × 256 reddedildi). |
| **DUR-05d-2** | Kabul: üç public sayaç + erişimci; RECEIPT ertelemesi INT-02 `app_retransmit`'e girmez (INT-02 raporunda ayrı sütun: `receipts_deferred`, koşu sonunda `receipts_pending = 0`). |
| **DUR-05d-3** | Kabul: yalnız `Backpressure` geçici. |
| **DUR-05d-4** | Kabul: oturum değişiminde kuyruk düşer (yeni oturumda TEXT yeniden gelir, RECEIPT yeniden üretilir). |

## 5. K4G-05e — INT-02 düzeneği: yeni kollar ve sayaçlar (harness; kod bu mesajla serbest)

- Neden: 05c/05d'nin sayaçları ve `ek-tuketici-durak` kolu olmadan INT-02 yeni kusurları ne tetikler ne ölçer.
- Ne: `kama-node/src/probe.rs` `NodeProbe` (test-util) + `node.rs` `probe()` — `delivery_channel_drops`, `full_refusals`, `reorder_skip`, `receipts_pending/deferred/evicted`, runtime tablo boyutları (DUR-23 b kancası); `int02_node.rs --arm ek-tuketici-durak` (DUR-05c-3 tarifi); `int02_run.sh` / `int02_analyze.py` hükümleri (C9 §5); `live_diag` açık; gözlem açık/kapalı karşılaştırma kolu.
- Nereye: `crates/kama-node/src/{probe.rs, node.rs}` (test-util), `crates/kama-integration/examples/int02_node.rs`, `scripts/netem/`, `docs/reports/INT-02-RAPORU.md` §10 (yeni tablolar).
- Bağımlılık: #136 merge; sayaç adları 05c/05d notlarındaki gibi (PR'lar sıralı: 05c → 05d → 05e; 05e'nin kodu onların sayaçlarını bekler).
- Done: yeni kol tabanda (05c'siz) kırmızı, 05c'li adayda 3/3; analiz betiği self-test; ürün kodu değişmez (test-util dışında).

## 6. Sıra (bugünden)

Bağımlılık PR'ı → #136 → #135 (yerel kapı, istisna) → onaylar (3 ayrı mesaj, bugün) → **05b** kod (P0 testleri yeşile; DR-CS-1 üçlüsü, Karn testleri, freeze pin'i beyanlı) → **05c** → **05d** → **05e** → mutasyon + kapılar → T-hücre gece → INT-02 (C9 §5 + DUR-05c-3) → rapor → benden "INT-02 GEÇTİ". Takvimin (29 Ekim tatili dahil, "INT-02 GEÇTİ" ≈ 10 Kasım) benimkinin yerine geçer; onay 27 değil **bugün** geldiği için bir iş günü kazanılabilir — sen hesapla.

Rapor: bağımlılık PR'ı + #136 + #135 merge → tek satır; 05b PR açılınca tek satır; gün sonu tek rapor.
````

---

## Üç onay (K4G-05b, K4G-05c, K4G-05d) — 2026-10-24

*Kaynak:* oturuma yapıştırılan metin, 2026-10-24; orkestratör transkripsiyonu `~/kama-k4g-work/decisions/C10-ONAYLAR.md`, bayt-özdeş; sha256 `0361420b29e61b8743c46c67733b30def5e52c183a852a1b41d7dd6eca73df97`

````markdown
# Üç onay — K4G oturumuna ÜÇ AYRI MESAJ olarak, CEVAP PAKETİ 10'dan sonra

(Orkestratör notu: sahip bu üç bloğu CEVAP PAKETİ 10 ile aynı mesajda, "Şimdi bu cevap ve onaylarla başla" talimatıyla verdi — 2026-10-24.)

Her blok tek mesaj. Araya başka metin koyma.

---

K4G-05b ONAYLANDI — `docs/design/K4G-05b-RETX-FIX.md` rev4 @ ea2e57b2b57e77b2c50b0370b69c402341890d9c (dal `k4g/05b-retx-fix`), Ek A 227 satır sha256 `27171e1c9658d959d6b98b49d5af2af9b44f74fa66ec8b35e5d996b73bac5c77`. DUR-22..31 cevapları CEVAP PAKETİ 10'da. `<ONAY-TARİHİ>` = 2026-10-24. Kod başlar: P0 testleri bu PR'da yeşile döner; DR-CS-1 üçlüsü, Karn testleri ve freeze pin'i DUR-8 biçiminde beyanlı; mevcut testler + tv_* değişmeden yeşil; 2×2 çıktılı; mutasyon hedefli. PR, bağımlılık PR'ı ve #136 merge olmadan açılmaz. "INT-02 GEÇTİ" ayrı.

---

K4G-05c ONAYLANDI — `docs/design/K4G-05c-ALICI-KABUL.md` rev2 @ 18e6de95e361195bb2e5bf297b04426c84024df3 (dal `k4g/05c-alici-kabul`), Ek A sha256 `efee869ff8aa2377140337e043191f509c143dc1a4f4332aa0fc3dc9ab9dab15`. DUR-05c-1 (a′), -2 (a), -3 kabul (tarif CEVAP 10 §3), -4 (a), -5 (a), -6 (a) + K4G-05e, -8 (a), -9 (a), -10 (a), -11 (a), -12 (a), -13 (a), DUR-23 (b) kabul. Kod 05b rev4 merge'ünden sonra başlar; P0 R1/R2/takılma/sızıntı testleri bu PR'da yeşile döner; `transport.rs:1323-1338`, `runtime_e2e.rs:255-305`, `runtime.rs:3334-3372`, `node/tests.rs:415-456` beyanlı. "INT-02 GEÇTİ" ayrı.

---

K4G-05d ONAYLANDI — `docs/design/K4G-05d-RECEIPT.md` rev2 @ b9d83396b2a9004c2a92e1f84408bdc3847caf6a (dal `k4g/05d-receipt`), Ek A sha256 `f3f62563f21f8eee…` (tam değer PR gövdesinde). DUR-05d-1 en eski düşer + sayaç, -2 kabul, -3 kabul, -4 düşür. Kod şimdi başlayabilir (05b/05c'den bağımsız); P0 R3 testleri bu PR'da yeşile döner; "tek deneme" açıklaması ve testleri beyanlı. "INT-02 GEÇTİ" ayrı.
````
