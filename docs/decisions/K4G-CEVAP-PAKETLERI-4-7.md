# K4G sprinti — sahip karar paketleri CEVAP 4–7 (kayıt) · güncel kapsam CEVAP ~~4–9~~ ~~4–10 ve üç onay~~ 4–13 (+ 11-EK) ve üç onay (dosya adı değişmez)

> Bu dosya K4G-05b tasarım notunun (`docs/design/K4G-05b-RETX-FIX.md`) ve K4G-15/K4G-05 PR'larının atıf verdiği **sahip kararlarının** depodaki kaydıdır (CEVAP PAKETİ 7 — EK B7). Metinler değiştirilmeden aktarılmıştır; her bölümün başında kaynağı ve metnin sha256'sı yazılıdır. Karar mercii Abkarada'dır; bu dosya karar üretmez. Kapı ifadeleri yalnız sahibin mesajında geçerlidir; burada geçmeleri gerçekleşmiş kapı anlamına gelmez. CEVAP PAKETİ 8 (2026-10-19) ve CEVAP PAKETİ 9 (2026-10-24) bölümleri 2026-10-24'te K4G-05b rev4 ile eklendi; dosya adı mevcut atıflar için korunur. CEVAP PAKETİ 10 (2026-10-24) ve aynı gün verilen üç onay metni (K4G-05b, K4G-05c, K4G-05d) 2026-10-24'te K4G-05b onayıyla eklendi; ikisinin kaynağı orkestratör transkripsiyonudur (bölüm başında). CEVAP PAKETİ 11, 11-EK, 12 ve 13 (2026-10-24) bölümleri 2026-10-24'te K4G-05b rev5 ile eklendi; çelişkide öncelik 13 > 12 > 11-EK > 11 > 10 (CEVAP 13). CEVAP 11 üç onay mesajını geri aldı; CEVAP 12 §1'e göre K4G-05b onayı rev5 kapsamıyla geçerlidir — onay bölümü kayıt olarak yerinde kalır.

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

---

## CEVAP PAKETİ 11 — 2026-10-24

*Kaynak:* dosya `~/kama-k4g-work/decisions/C11.md` (orkestratörün ilettiği karar paketi, 2026-10-24), bayt-özdeş; sha256 `301ae7223bb8a359b97b2df7d483ce215253a582c623586f41ef41f71954379e`

````markdown
# CEVAP PAKETİ 11 — Abkarada → K4G orkestratörü (2026-10-24, akşam) — CEVAP 10'un düzeltilmesi; üç onay geri alındı / verilmedi

**Girdi:** bağımsız incelemecinin rev4/05c/05d notlarına ve CEVAP 10 + onay metinlerine verdiği iki cevap (ekte, veri olarak; karar bu mesaj). İncelemeci haklı: CEVAP 10 üç yerde kusuru kapatmak yerine başka koşula taşıyan kararlar verdi ve onay metinleri kararlarla tutarsızdı (eski SHA'ya onay + davranışı değiştiren paket). **Bu mesaj CEVAP 10'un aşağıda sayılan maddelerini geçersiz kılar; sayılmayan her şey CEVAP 10'daki gibi kalır.** `ONAY-3-MESAJ` yapıştırıldıysa üçü de **GERİ ALINDI**; yapıştırılmadıysa verilmemiş sayılır. Onaylar rev5 SHA'larına, tam hash'le, tek seferde.

Önce **durum satırı** (tek mesaj, uzun rapor değil): hangi değişiklik yalnız yerelde, hangisi commit, PR açıldı mı, merge var mı. Bağımlılık PR'ı, #136, #135 ve P0 dalı **devam eder**; hiçbir dal silinmez.

---

## 1. Geçersiz kılınan kararlar → yeni karar

| CEVAP 10'daki | Yeni karar | Ayırt edici test |
|---|---|---|
| **DUR-30** — K_MAP budaması motorda (05b), runtime uyarlaması 05c'de; arada "beyanlı ara durum" | **Ara durum yok.** 05b'de `K_MAP` sabiti ve `take_confirmed_units()` **tanımlanır, budama KAPALI** (davranış yok). Budama, runtime'ın ACK-tabanlı payload silmesinin kaldırılması, `take_confirmed_units()` tüketimi ve birim başına O(1) kayıt **aynı merge edilebilir değişiklikte** — 05c'de — açılır. Teyidin otoritesi motor, payload saklama sorumluluğu runtime; tek karar yolu. rev4 B.2.2/B.2.6'daki "05b aşamasında kırmızı beklenen gerçek-runtime testi" kaldırılır; o test 05c'de **yeşil** şarttır. | `late_ack_after_prune_keeps_payload` (gerçek runtime): K_MAP aşıldıktan sonra budanmış seq'e geç ACK → birim teyit edilmez, payload yerinde, yeniden gönderim payload'la çıkar. 05c'de yeşil; 05b'de budama kapalı olduğu için uygulanmaz. |
| **DUR-28 / A.7** — "geç-ACK ufku = K_MAP × RTO_MAX = 240 s" | **K_MAP = 8 bir ADET sınırıdır; süre garantisi değildir.** "240 s" ve "ufuk = K_MAP × RTO_MAX" ifadeleri kaldırılır (ilk kopyalar 0.4 → 25.6 s aralıklarla çıkar; ARQ ve problar da kayıt tüketir). Garanti şöyle yazılır: "Geç ACK, seq'i birimin son K_MAP iletimi arasındaysa tanınır; tanınmayan geç ACK veri kaybı değil, fazladan bir yeniden gönderimdir — payload sahipliği (05c) buna dayanır." Gerçek gönderim dizilerinde tutulan kayıt yaşı **ölçülür** ve INT-02 raporunda yazılır. | D2 kol 3'e: 600 s kesintide birim başına kayıt ≤ 8 ve tanınmayan geç ACK sonrası yeniden gönderim payload'la. |
| **DUR-31** — "kopya ≤ K_ARQ_SHARE turda gönderilir" | Fiziksel olarak sağlanamaz (1064 B/s'de 1050 B için ≈ 1 s token birikimi gerekir). **İki ayrı ölçüt:** (i) **Seçilme:** gönderilmeye uygun kopya başı en geç K_ARQ_SHARE `schedule_transmit` çağrısında hizmet hakkı kazanır. (ii) **Gönderilme:** hak kazandıktan sonra, rezervasyon **bütün sınıflara** (DATA, ARQ, prob) karşı geçerliyken ≤ boy / pace_rate + bir tur içinde çıkar; rezerv ≤ bir kayıt boyu, aynı anda tek rezerv, gönderimde temizlenir; token üretilmez. ARQ/prob rezervi tüketemez (açık). | (a) 37 B DATA + 1050 B kopya, 1064 B/s: seçilme ≤ K_ARQ_SHARE çağrı, gönderilme ≤ 1050/1064 s + 1 tur. (b) Aynı düzen + eşzamanlı ARQ ve prob trafiği: kopya yine sınır içinde; rezerv gönderim sonrası 0. (c) Soğuk başlangıç ve token kıtlığı (DUR-24 koşulu). |
| **DUR-24** ifadesi | **"PRE_CONVERGE hariç bütün durumlarda"** (PRE_CONVERGE'de §3.3-B sırası ve takvimi değişmez — zaten A.4'te). Önceki kapsam daraltılmadı; netleştirildi. | DUR-31 (c). |
| **DUR-25** — ürün kipi paritesi "yalnız raporlanır" | **Zorunlu test:** Controller kipinde, PreConverge sonrası kayıplı kanalda paritenin **gerçekten üretildiği** gösterilir (parite sayacı > 0 fikstür ön koşulu; 0 ise test geçersiz, yeşil sayılmaz) ve ESI düzeltmesi bu kolda doğrulanır. Statik-r kolu kusur kanıtı olarak kalır. | `esi_controller_mode_parity_does_not_confirm_source`. |
| **DUR-05c-12** — parçalı mesaj: "ACK + düşür" | **Geri çekildi.** Geçersiz girdi (bozuk FINAL, çelişen parça sayısı — koşullar tek tek listelenir) → **ACK YOK**, parçalı mesaj açık hata olayıyla iptal (`FragmentInvalid{peer, msg_id, neden}` → uygulama görür), sayaç. Geçerli ama yerel limiti aşan mesaj: PoC'de iki uç aynı derlemedir, limit uyumsuzluğu **ihlaldir**; yine ACK yok + açık hata; sessiz düşürme hiçbir dalda yok. | Her geçersizlik koşulu için ayrı test: ACK'lenmedi + hata olayı üretildi + payload düşürülmedi (tombstone'a). |
| **DUR-05c-13** ifadesi | 05c tabanında yeşil çıkan sızıntı bekçisi "yanlışlandı" DEĞİL, **"05b (`confirmed_units`) ile giderildi"** diye yazılır; kanıt 3439fccd + kanca kırmızısı. | — |
| **DUR-05d-1** — taşmada en eski düşer | **Geri çekildi: canlı RECEIPT taşma nedeniyle SİLİNMEZ** (değişmez). Mekanizma (05d rev3): (1) bekleyen-receipt kuyruğu eş başına 256, tekilleştirilmiş, **düşürme yok**; (2) kuyruk doluyken `Messenger` o eşin **oturum gelen-kutusunu** (05c'nin oturum başına kutusu) tüketmeyi durdurur → kutu dolar → 05c kabulü o oturumun yeni DATA'sını ACK'lemez → karşı taraf yeniden gönderir; transport ACK/heartbeat etkilenmez; aynı eşin PTT/sinyalleşmesi geçici olarak bekler (beyanlı, sınırlı). Diğer eşler etkilenmez. (3) **Boşluk payı:** `FileTransfer` uçuştaki parça sayısını transport sınırının altında tutar: ≤ sınır − H; **DUR-32:** H = 512 (256 TEXT + 256 RECEIPT; mevcut sabitlerden) — gerçek sınır sabitiyle doğrula, farklıysa türetimle DUR. Böylece RECEIPT'in Backpressure görmesi olağan yolda imkânsız, taşma yalnız ihlalde. | (a) `flush_resends` ve dosya sürekli kapasite tüketirken 300 TEXT → her RECEIPT aynı oturumda kabul edildi, kuyruk ≤ 256, düşen 0. (b) Eş 257+ RECEIPT yükümlülüğü yaratırsa: kutu tüketimi durdu, yeni DATA ACK'lenmedi, kuyruk boşalınca sürdü, kayıp 0. (c) Kontrol yolu: durma sırasında heartbeat/ACK çalıştı. |
| **DUR-05d-4** — oturum değişiminde kuyruk düşer | Kalır, **kanıt şartıyla:** adopt/retire/yeniden-connect sıralamalarının en az üç farklı dizilişinde (RECEIPT kuyrukta iken oturum kapanır; `adopt` ile `retire` ters sırada; iki ardışık değişim) yeni oturumda TEXT yeniden gelir ve RECEIPT yeniden üretilir, hiçbir TEXT "teslim edildi"siz kalmaz. Kanıt gelmezse kuyruk `(peer, msg_id)` ile yeni tutamaca **taşınır**. | `receipt_queue_survives_session_change_{a,b,c}`. |
| **DUR-27** ifadeleri | tokio 1.52.3 → 1.53.3 **minor** sürüm geçişidir; "yalnız yama sürümleri" yanlış. `Cargo.lock` farkı "yalnız bu üç güncellemenin gerektirdiği değişiklikler (sürüm, checksum, zorunlu bağımlılıklar); ilgisiz güncelleme yok". Tam kapı **merge edilecek gerçek adayda** (bağımlılık PR'ı main üstüne, #136 onun üstüne, #135 onun üstüne) koşar; eski SHA'daki 24/25 sayılmaz; `cargo deny` istisnayla **affedilmez**, yeşil olmalı. Karar (evet) değişmedi. | Her merge için o adayın `summary.txt` PR gövdesinde. |
| **ONAY-3-MESAJ** | Geri alındı / verilmedi. Kararlar notlara işlenir → rev5 SHA'ları + **tam** Ek A hash'leri → tek onay turu (üç ayrı mesaj, aynı gün). | — |

## 2. Eklenen testler (05c)

- **FEC dolu kanalda:** tüketici sonlu süre durur; kaynak iletimleri kaybolur, yeterli onarım sembolü ulaşır, kurtarılan birim kabul edilemez (yer yok) → tüketici açılınca teslim **tamamlanır** (ARQ yoluyla ya da sınırlı saklamayla; hangisi, notta). Tamamlanmıyorsa DUR.
- **Oturum geçişinde rezerv:** eski ve yeni oturum birlikte varken (adopt öncesi/sonrası) rezerv hesabı (R = min(max_peers, C − 1)) aşılmaz; sıra başı garantisi korunur.
- `seen_count` spec yaması (DUR-05c-1 a′) **aynı PR'da** (değişmedi).

## 3. Süreç — bundan sonra DUR açılmayacaklar

Rev5 onaylarından sonra **uygulama tercihleri için DUR açılmaz** (ad, dosya düzeni, test fikstürü ayrıntısı, yardımcı fonksiyon). DUR yalnız: değişmez/sözleşme metni, yeni sayı, tel/vektör, FROZEN dosya, beyanlı sınır eklenmesi, bir testin amacının/eşiğinin değişmesi. Gerisini yap, PR gövdesinde yaz.

## 4. İstenen çıktı (tek mesaj, bu sırayla)

1. Durum satırı (yerel / commit / PR / merge).
2. **Tek tablo:** eski karar → düzeltilmiş karar → etkilenen dosya/PR → ayırt edici test (bu mesajın §1–§2'si notlara işlenmiş hâliyle).
3. rev5 SHA'ları (05b, 05c, 05d) + **tam** Ek A sha256'ları + 05d için tam hash.
4. Yeni süre tahmini (05d artık 0,5 gün değil; 05c'ye iki test eklendi).

Takvim: yol v2'deki 9–10 Kasım / 18–20 Kasım **planlama tahminidir**, taahhüt değil; rev5 ve ilk PR'dan sonra yeniden yazılır. Bağımlılık PR'ı → #136 → #135 bu mesajı beklemez.
````

---

## CEVAP PAKETİ 11 — EK — 2026-10-24

*Kaynak:* dosya `~/kama-k4g-work/decisions/C11-EK.md` (orkestratörün ilettiği karar paketi, 2026-10-24), bayt-özdeş; sha256 `da21ced9e818868d9e06df8ba19814800c7fa67d4fb9c7500c4a3b18cf6387ee`

````markdown
# CEVAP PAKETİ 11 — EK (2026-10-24) — CEVAP 11 ile birlikte verilir; çelişkide EK geçerli

İncelemecinin CEVAP 11'e üçüncü cevabı işlendi. Beş düzeltme; CEVAP 11'in geri kalanı aynen.

## 1. RECEIPT (05d rev3) — mekanizma değil, değişmez + test hedefi onaylanıyor

CEVAP 11'deki "gelen-kutusunu durdur + H = 512" **emir değil, aday**. Gerekçe: iki eş aynı anda kuyruk-dolu duruma girerse ikisi de kutuyu tüketmez, kutudaki RECEIPT'ler işlenmez, 05c kabulü iki yönde de ACK'i keser → karşılıklı bekleme. Transport ACK'lerinin çalışması uygulama RECEIPT'inin ilerlemesini garanti etmez.

**Değişmez (onaylı):** (i) canlı RECEIPT hiçbir koşulda silinmez; (ii) RECEIPT için kapasite **bütün üreticilere** (dosya parçası, TEXT yeniden sunumu, PTT demeti, sinyalleşme) karşı korunur; (iii) iki yön aynı anda dolduğunda ilerleme sürer — karşılıklı bekleme yok; (iv) DATA kabulü duraklarken gerekli kontrol mesajları işlenir.

**Mekanizma orkestratörün tasarımıdır** (notta, kanıtla). Öneri, emir değil: rezerv `client-core`'a AMP bilgisi sızdırmadan, `kama-node`'da tek "uygulama gönderim bütçesi" ile — RECEIPT dışı üreticiler eş başına bekleyen bütçenin ≤ (sınır − H)'sini kullanır, H yalnız RECEIPT'e; birim türleri (mesaj / transport birimi / kuyruk girdisi) tek birimde eşlenir. **H = 512 aday**, doğrulanır; türetim notta.

**Zorunlu testler:** (a) iki yön aynı anda: her iki uçta dosya + TEXT yeniden sunumu + PTT sürerken 256 TEXT karşılıklı → bütün RECEIPT'ler aynı oturumda kabul, kayıp 0, kilit yok, kontrol yolu (ACK, S1 oturum olayları — PoC'de transport heartbeat **yok**, heartbeat şart koşulmaz) çalıştı; (b) tek yön doygunluk (CEVAP 11 (a)), ama gönderenin 256 outbox sınırı **aşılmadan** — 300 TEXT zaman içinde kabul edilir; 256'yı aşan sunum ayrı bir **protokol ihlali** testidir; (c) rezerv ihlali denemesi: dosya motoru H'yi tüketemez.

## 2. Geçersiz parçalı mesaj (DUR-05c-12) — iki uçta terminal durum

CEVAP 11'deki "ACK YOK" düzeltiliyor: ACK'siz geçersiz birim gönderende sonsuz yeniden gönderim ve sıralı akışta baş-blokajı yaratır. Yeni kural: transport **ACK'ler** (bayt eksiksiz ulaştı, uygulama katmanına teslim edildi); uygulama katmanı mesajı **açık hatayla reddeder** (`FragmentInvalid{peer, msg_id, neden}` olayı + sayaç), payload **sınırlı temizlikle** düşer (tombstone saklama yok). Değişmezin okuması: "ACK'lenmiş birim uygulama katmanına teslim edilir; uygulama geçersiz mesajı açık hatayla reddedebilir — sessiz düşürme yok." Geçerli-ama-limit-üstü sınıfı PoC'de yoktur (aynı derleme); gerçekleşirse ihlal sayılır ve aynı yolu izler.

**Gönderen tarafı terminal durumu notta tanımlanır (tel değişmez):** dosya → mevcut 60 s zaman aşımıyla `Failed`; TEXT/sinyalleşme → RECEIPT gelmez, mevcut oturum-yenileme kuralıyla yeniden sunulur; aynı mesaj ikinci kez de reddedilirse gönderen tarafında kalıcı `Failed` (mevcut sayaçlarla; yeni AMP mesajı yok). Mevcut oturum kapatma/hata mekanizmasının bunu karşılayıp karşılamadığı notta incelenir; karşılamıyorsa DUR, yeni tel değil.

## 3. 05b/05c geçişi — bütün kayıt-unutma yolları

Kabul şartı genişledi: motorun kaydı unutabildiği **her** yol — K_MAP budaması, tombstone süre dolumu, kapasite tahliyesi, başka varsa — için geç ACK, motor ile runtime arasında payload sahipliği ayrışması yaratmaz. **P0-ek (yarım gün):** bu yolların envanteri, satır atıflı. Envanterde v1'de **bugün** var olan bir yol çıkarsa (tombstone TTL büyük olasılıkla), ayrışma main'de zaten var demektir → runtime uyarlaması (payload yalnız `take_confirmed_units()` ile silinir) **05b'ye alınır**; "budama 05b'de kapalı" bunu değiştirmez. Test her yol için ayrı.

## 4. K_MAP ve rezervasyon sözleşmeleri — ifade düzeltmeleri (notlara aynen)

| Eski | Yeni |
|---|---|
| "Son K_MAP iletimindeyse geç ACK tanınır" | "…tanınır — o kayıt TTL/tahliyeyle daha önce silinmemişse; tanıma garantisi bütün silme yollarıyla birlikte tanımlıdır (§3)" |
| "Tanınmayan geç ACK fazladan bir yeniden gönderimdir" | "…ek yeniden gönderimlere yol açabilir; payload sahipliği korunur" |
| "Gönderilme ≤ boy / pace_rate + bir tur" | "sabit pace_rate, çalışan zamanlayıcı ve yeterli BURST kapasitesi koşuluyla; değişken hızda biriken token üzerinden hesaplanır (test iki kipte)" |
| "Rezerv gönderimde temizlenir" | "…gönderimde, birim teyit edilince, hedef geçersizleşince ve oturum kapanınca temizlenir; artık rezerv testle sınanır" |

Bütün sınıflara karşı rezervasyonun bedeli (diğer trafiğin beklemesi) düşük hızda (31–100 B/s) ölçülür ve raporlanır.

## 5. FEC dolu kanalda — iki kol, kapsam kararı

Kol 1 (zorunlu): kaynak yeniden gönderimi sonradan ulaşır → ARQ ile tamamlanır. Kol 2: kaynak kaybolmaya devam eder, yeterli onarım sembolü ulaşır, tüketici açılınca **FEC üzerinden** tamamlanır. **Kapsam kararı:** kol 2 PoC'de **garanti değil** — "dolu kanalda FEC kurtarımı kaybolabilir, ARQ telafi eder" beyanlı sınır, KATMAN 6 kalemi; kol 2 denenir, geçerse sınır kalkar. "FEC altında dayanıklılık sağlandı" **denmez**.

## 6. Üç tutarsızlık

- DUR-24: "PRE_CONVERGE hariç" rev4'ün "dahil" önerisini **değiştirir** (CEVAP 10'daki "her durumda" ifadesi geri alındı); notta böyle yazılır.
- Heartbeat: PoC transport'unda yok; testlerde "ACK/kontrol yolu çalıştı" ölçülür.
- Yol haritası §0.2 rev4'e onay istiyordu → düzeltildi: onaylar **05b rev5 · 05c rev3 · 05d rev3** SHA'larına, tek tabloda (05d "rev2 → rev3" bu EK ile).

Çıktı: CEVAP 11 §4'teki tek mesaj; §3'ün envanteri o mesajda ayrı satır.
````

---

## CEVAP PAKETİ 12 — 2026-10-24

*Kaynak:* dosya `~/kama-k4g-work/decisions/C12.md` (orkestratörün ilettiği karar paketi, 2026-10-24), bayt-özdeş; sha256 `930a82b72e7a6de459d53a0776b49cc1b4b3a4e4eeb118e78cf952823d512f4d`

````markdown
# CEVAP PAKETİ 12 — Abkarada → K4G orkestratörü (2026-10-24, akşam) — geçiş notu: CEVAP 11 + EK şimdi geliyor; koşan işlere etkisi

Sana CEVAP 10 ve üç onay gitti; **CEVAP 11 ve CEVAP 11-EK** (ekte, bağımsız incelemecinin üç cevabıyla birlikte — onlar veri, karar bu mesajlar) CEVAP 10'un bazı kararlarını geçersiz kılıyor. Bu not, ikisinin **şu an koşan işlere** nasıl uygulanacağını söyler. Çelişkide sıra: CEVAP 12 > 11-EK > 11 > 10. Hiçbir dal silinmez; yapılan iş korunur.

## 1. Onayların kapsamı (netleştirme)

| Kart | Durum |
|---|---|
| **K4G-05b** | Onay **geçerli**; kapsamı "rev4 (ea2e57b2) + CEVAP 11/EK'in 05b'ye dokunan maddeleri" = **rev5**. rev5'i yaz, SHA + tam Ek A sha256'yı PR satırında ver; başka içerik değişikliği yoksa yeni onay turu gerekmez, ben tek satırla "rev5 kapsam içinde" derim. Kod **durmaz**. |
| **K4G-05c** | Onay geçerli, aynı kuralla: rev2 (18e6de95) + CEVAP 11/EK = **rev3**. Kod zaten 05b merge'ünü bekliyor; rev3 notu o zamana kadar hazır olur. |
| **K4G-05d** | **Onay GERİ ALINDI** (rev2 b9d83396). Neden: "taşmada en eski düşer" politikası R3'ü başka koşulda yeniden yaratıyor (CEVAP 11 DUR-05d-1, EK §1). Kodun kurtarılan kısmı: kuyruk, `(peer, msg_id)` tekilleştirme, poll başında yeniden sunum, ilk Backpressure'da durma, sayaçlar, T5 ve iki yeni birim testi. **Çıkan kısım:** `receipts_evicted` düşürme yolu ve onu sabitleyen test. Şerit kodlamayı durdurur, **rev3 notunu** yazar (EK §1: değişmez + iki yönlü doygunluk testi; mekanizma senin tasarımın, kanıtla; H = 512 aday). rev3 gelince yeni onay. |
| **K4G-05e** | Değişmedi. |

## 2. Koşan işlere etkisi

| İş | Yap |
|---|---|
| **#137 merge (2a0e2133), #136 ve #135 kapıları** | Devam. Kilit dosyası tercihin (tokio'da yalnız sürüm + checksum, `--locked` kabul) **uygun**; PR gövdesinde CEVAP 11 DUR-27 ifadesiyle: "yalnız bu güncellemelerin gerektirdiği değişiklikler; ilgisiz güncelleme yok; tokio minor geçiş". |
| **05b spec şeridi (46b6cadf), Ek A `ab425e57…`** | DUR-24 değişikliği **uygun**; nota ve A.4'e açıkça yaz: "PRE_CONVERGE hariç" rev4'ün "dahil" önerisini **değiştirir** (CEVAP 10'daki "her durumda" geri alındı; EK §6). rev5'e girecek diğer Ek A değişiklikleri: K_MAP ufuk cümleleri (EK §4), DUR-31 iki ölçüt, DUR-05c-12 yeniden yazımı 05c'de. |
| **05b kod şeridi — sekiz adım** | Önce **bildir**: hangi adım K_MAP budamasını açıyor, hangi adım runtime'a dokunuyor? Kural (CEVAP 11 DUR-30): budama 05b'de **kapalı** (sabit + `take_confirmed_units()` tanımlı, davranış yok). **EK §3 envanteri hemen** (yarım gün, şeride paralel): motorun kaydı unutabildiği her yol (tombstone TTL, tahliye, başka). v1'de bugün var olan bir yol çıkarsa runtime payload-silme uyarlaması **05b'ye alınır**; çıkmazsa 05c'de budamayla birlikte. |
| **DUR-31 okuması** (şeride ilettiğin) | Düzelt: "arada DATA gönderilmez" değil — **DATA rezervin altına inemez**, rezervin üstündeki token'ı kullanabilir; ARQ ve prob da rezervi tüketemez; rezerv ≤ bir kayıt, tek rezerv, temizlenme: gönderim / teyit / hedef geçersiz / oturum kapanışı (EK §4). Test iki ölçütlü: seçilme ≤ K_ARQ_SHARE çağrı; gönderilme ≤ boy / pace_rate + bir tur (sabit hız, çalışan zamanlayıcı, yeterli BURST; değişken hızda biriken token). |
| **DUR-24, DUR-26 okumaları** | Uygun. |
| **9 kırmızı** | Sınıflandırman doğru; kural sabit: **gerileme → kod düzelir; test değişmez.** Mevcut testlerde (dr_cs1 ikilisi dahil) değişiklik yalnız DUR-8 biçimiyle (paragraf + 2×2 çıktı + mutasyon) ve yalnız v1.1'in **kasıtlı** davranış değişikliği kanıtlanınca; "zamanlama kaydı" tek başına gerekçe değil. Yeni testlerin fikstür/beklenti hataları (two_units_handover, saturated_ack_delay durum beklentisi) senin kararın — PR gövdesinde yaz, DUR açma. `h_anchor_stale_ack_vs_progress`'teki "ilerleme ile bayat ACK aynı anda" → ilerleme **öncelikli** (sessizlik sayacı ilerlemeyle sıfırlanır); notta tek cümle. |
| **`#[ignore]` testi 05c'ye** | Uygun. |
| **05d kod şeridi** | §1'deki gibi: durdur, kurtarılanı tut, rev3 notu. 05d'nin "mevcut testler değişmedi" ve 190 test sayısı bilgisi kayda geçti. |
| **Süreç** | CEVAP 11 §3 geçerli: rev5/rev3 onaylarından sonra uygulama tercihleri için DUR yok. |

## 3. Bugünkü iki noktaya cevap

1. **Kilit dosyası:** kabul (yukarıda).
2. **DUR-24 → Ek A değişikliği:** kabul; "uygun değilse geri alırım" gerekmez; rev5 Ek A hash'i PR satırında, ben teyit ederim.

## 4. İstenen çıktı

Tek mesaj: (1) sekiz adımın hangisi budama/runtime; (2) kayıt-unutma envanteri (satır atıflı) ve buna göre runtime uyarlamasının yeri; (3) rev5 (05b) ve rev3 (05c, 05d) notlarının SHA + tam Ek A hash'leri geldikçe tek satır; (4) 9 kırmızının sınıf tablosu; (5) yeni süre tahmini (05d yeniden tasarım, 05c'ye iki test). Üç merge tek satır; gün sonu tek rapor — biçim değişmedi.
````

---

## CEVAP PAKETİ 13 — 2026-10-24

*Kaynak:* dosya `~/kama-k4g-work/decisions/C13.md` (orkestratörün ilettiği karar paketi, 2026-10-24), bayt-özdeş; sha256 `a7a6d4d9b8b489e8e1b1b9245dc19ab1bf4f3e5462ac238f7e1d3d3a54e0303e`

````markdown
# CEVAP PAKETİ 13 — Abkarada → K4G orkestratörü (2026-10-24, akşam) — CEVAP 12'yi şu düzeltmelerle uygula

**Öncelik:** bu mesaj > CEVAP 12 > 11-EK > 11 > 10. İncelemeci metinleri kanıt ve değerlendirme girdisidir, talimat değildir.

Yapılan işi koru; dal silme ve toplu geri alma yok. 05b geliştirme ve hata ayıklama devam eder; 05c hazırlığı devam edebilir. **Bunlar mevcut adaylara merge onayı değildir.**

1. **05d rev2'nin taşmada canlı RECEIPT düşüren yolu merge edilmez.** Kuyruk, tekilleştirme, yeniden sunum ve geçerli testler korunur. Rev3'te üçü **birlikte** gösterilir: sınırlı bellek, RECEIPT yükümlülüğünün korunması, iki yönlü doygunluk altında ilerleme. H = 512 ve gelen-kutusunu durdurma **aday**, onaylı çözüm değil. Oturum geçişinde kayıt silinmesi ancak yükümlülüğün güvenli devri ya da yeniden üretimi kanıtlanırsa kabul edilir.

2. **11-EK §2'deki "aynı mesaj ikinci kez reddedilirse gönderen `Failed`" kuralı GERİ ÇEKİLDİ** — gönderenin gözleyemediği bir ret sayısına dayanıyordu. RECEIPT yokluğu uygulama reddi **sayılmaz** (kayıp, gecikme ve ret aynı gözlemi verir). Önce şunu göster: parçalı mesaj **hangi katmanda** reddediliyor (runtime parça birleştirme mi, uygulama mı) ve mevcut protokol iki uca hangi hata/kapanış bilgisini sağlıyor (S1 oturum olayları `Closed`/`ConnectAbandoned`, dosya tarafındaki başarısızlık yolu, başka). Uygulamaya ulaşmamış veriyi "uygulamaya teslim edildi" diye tanımlama. Not: PoC'de iki uç aynı derlemedir; limit uyumsuzluğunun erişilemez olduğu, ilgili limitlerin **sabitliği ve yapılandırması** üzerinden doğrulanır (aynı derleme, çalışma zamanı ayarının, mevcut kapasitenin ve kaynak durumunun aynı olduğunu garanti etmez). **Geçerli girdinin kapasite ya da kaynak yetersizliği nedeniyle reddedilmesi bozuk girdi sayılmaz** — o R2'nin konusudur ve 05c kabul kuralıyla (ACK yok, yeniden gönderim) ele alınır. Mevcut mekanizma yeterliyse onu kullan; yeterli değilse **yalnız bu madde** için somut seçenekleri ve etkilerini getir (tel değişikliği de bir seçenek olarak yazılır, kararı benim); bağımsız işler devam eder.

3. **Kayıt-unutma envanteri:** her yol için aynı geç ACK'in motorda ve runtime'da verdiği sonucu **karşılaştır**; silme yolunun varlığı tek başına kusur kanıtı değildir. Ayrışma gösterilen her yol için runtime payload-silme uyarlaması **05b'ye alınır**. **05b merge koşulu:** adayın etkin bütün yollarında payload sahipliği tutarlı; yeniden gönderimde store-miss yok (test). K_MAP budaması ancak runtime ile uyumlu değişiklikte açılır.

4. **P0 motor 7/7** "bu senaryolar geçti" diye kaydedilir; "motor tamamlandı" denmez. Dokuz kırmızı için tablo: test adı · taban/aday SHA · kök neden · kod mu test mi değişecek · korunan davranış. Fikstür hatası düzeltilebilir; testin hedeflediği davranış ya da kabul eşiği sessizce değişmez. `ResolvedByDataAck → TimedOut`, kasıtlı sözleşme değişikliği gösterilmedikçe **gerileme şüphesidir**.

5. **`#[ignore]` yok.** Test 05c'ye taşınıyorsa test ettiği davranışın neden 05c'de olduğunu yaz; test o PR'ın kapısında **aktif** olsun. Testi taşıyarak 05b'de etkin bir kusuru kapsam dışına çıkarma.

6. **#137 sırf kilit dosyası elle düzenlendiği için geri alınmaz.** Desteklenen hedeflerde doğrulama tamamlanır: `--locked` kabulünün yanına çözümleme/derleme kanıtı — `x86_64-unknown-linux-gnu` (kapı) ve **`aarch64-linux-android`** (`kama-mobile`, K4G-15 derleme betiğiyle, `--locked`); eksikse tamamla, PR gövdesine ek satır. Gerçek bir gerileme bulunursa düzeltme ya da gerekçeli geri alma değerlendirilir (bana tek satır). #136 ve #135, merge edilecek **gerçek adaylarında** tam kapı yeşilse mevcut yetkiyle ilerler.

7. **FEC ikinci kolu** PoC garantisi dışında kalır; raporda açıkça. Tek başarılı koşu genel sınırı kaldırmaz. Birinci kolun ARQ ile toparlanması zorunlu.

**Sonraki çıktı (tek mesaj):** güncel dal/PR/merge durumu · dokuz kırmızının tablosu · kayıt-unutma karşılaştırma sonuçları ve runtime uyarlamasının yeri · parçalı mesaj ret katmanı analizi · 05d rev3 önerisi · yeni süre tahmini. Rutin uygulama tercihleri için onay isteme; kalan sözleşme sorunları yalnız ilgili işi durdursun.
````
