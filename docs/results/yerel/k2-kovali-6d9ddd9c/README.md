# #262 — mimari kararlarla uyum denetimi (T1) · `6d9ddd9c`

Sahip talimatı #175 7175888074: «Her kural için tam SHA, kod konumu, test sonucu ve DOĞRULANDI / İHLAL / SINANMADI yaz. Açık ihlal varsa somut engeli belirt. Ürün kabulü hükmünü ayrı tut.»

**Bu belge uyum denetimidir; ürün kabulü değildir.** Ürün kabulü hükmü (T2) Web'indir: #262, yorum 7182899149 (`1a5fc80c`) ve son baş için güncellemesi.

## Kimlik

| | |
|---|---|
| Karar kodu | `6d9ddd9cdbd79f37d780221db83b9133cdd39cde` (dal `yerel/cift-yoklama-hedefi`, PR #262 taslak, deney varsayılanı KAPALI) |
| Kırmızı kanıt | `a00bc34c` (yalnız testler; karar kodu `1a5fc80c` ile aynı) ve `474daa44` (yalnız testler; karar kodu `ee168d3d` ile aynı) |
| Kayıt koşusu | `dac42afb77055d9cf99802064d238abf114063f8` (= `6d9ddd9c` + koşu betiğinin satır süzgeci; kod aynı) |
| Ham kayıt | `ham.txt`, sha256 `12926a33b96fc3ae306ee3815e8a1aa055b19a8d3c94dc73f2e36232b06cf939`; `K2BASLIK`: ikili sha256 `4f9b9d0d739cb6760a3c077cf14db5eb4bdaefe46dc192256ea389f0bdae6e91`, rustc 1.96.0, iki koşu bayt-aynı, satırlar sha256 `fae8ea3126b7d8e69819ce152c0b43e1156c633bcf4b79c8172407e4e7003eca` |
| Komutlar | `cargo test --locked --release -p adaptive-transport --lib -- line_guard:: pair_probe_unmeasured_ack_tests --test-threads=1`; `cargo test --locked --release -p adaptive-transport --no-fail-fast`; `bash scripts/ktp2/f0/k2/kos.sh <çıktı>` |
| Sonuçlar | `a00bc34c`: 79 geçti / **8 kırmızı** / 2 ignored (`kirmizi-a00bc34c.txt`). `6d9ddd9c`: 91 geçti / 0 kırmızı / 2 ignored; `--features experimental-cold-start` ile de 91 / 0 / 2; tam paket yalnız taban dalda da kırmızı olan dört test (`yesil-6d9ddd9c.txt`); clippy `-D warnings` ve fmt temiz |
| Denetim | 2 denetçi + 10 şüpheci (`ee168d3d`, salt okuma), ardından iki tur bağımsız çürütme (girdi dizileri + kural tablosu; testleri kendileri koştu). Bağımsız yeniden üretim: Bulut-B (kırmızı / yeşil sayıları), Web ve Bulut-B (kayıt satırları, farklı rustc ile), Bulut-D (ayrı düzenek, #264), Bulut-A (fark ↔ kapsam listesi, 10 / 10 yol) |

## Sahibin dört başlığı

| Başlık | Hüküm | Dayanak |
|---|---|---|
| ACK sayımı — «göndericinin işlediği her ACK kabul sırasını korusun; ölçüm taşımayan veya cephesi ilerlemeyen ACK de sayılır, fakat kapsama kanıtı vermez» | **DOĞRULANDI** | Korumadaki yedi çıkış + göndericideki iki yol (eşlenen seq yok; gönderilmiş seq yok) tek yola toplandı; her biri ayrı testte eski başta kırmızı, şimdi yeşil. Çift sayım yolu yok (Bulut-A + çürütücü). Kapalı döngüde bu yollar hiç oluşmuyor (1 233 satırın hepsi ölçülü): kanıt dar testlerdir. |
| Kapsamayan ACK'te sonuçsuz kapanma — «3. ACK'e kadar yeterli kanıt yoksa deney SONUÇSUZ … 4. ACK bu hükmü değiştiremez» | **DOĞRULANDI** | 2. kapsamayan ACK'te `Inconclusive`, tavan taban, ret yok; ölçümsüz 3. ACK'te de; fazla pacer'a ulaşıyor. |
| Aynı kanıtla ikinci yoklamanın engellenmesi — «sonuçsuz kapanan yoklamanın kendi kuyruğundan gelen aşağı örnek, aynı kanıtla ikinci yoklama başlatamasın» | **DOĞRULANDI** (sonuçsuz, «kaldı», kanıtsız, kayıtlıyken örnekle kapanış) · **İHLAL: «geçti» ile kapanış** | Kalıcı artış yazmadan kapanan her yoklama penceresini saklıyor; pencereye uzanan örnek ret, sonrası hakkı yeniler. «Geçti» kapanışta kural yok: kapalı döngüde (sahte · açık · kova · ‰200 · tohum 2) sahte «geçti» → aşağı örnek → ikinci çift yoklaması. |
| «Tek yoklama, en fazla 6 fiilî datagram ve 7.368 tel baytı. Küçük datagramlar adede girer.» | **DOĞRULANDI** (motorun datagramları) · **İHLAL: telde motor dışı datagramlar** | Pencereden önce sayılmayan yedinci datagram düzeltildi (gerçek `Sender`: 7 / 8 596 B → 6 / 7 368 B; çürütücü 0–12 000 B yabancı kuyruk, süren tutma, boşluk boşaltması, küçük datagram, olay adımı 1–250 ms süpürmesinde hep 6). Kapalı döngüde 35 pencere, en çok 6 datagram / 6 444 B, pencereden hemen önce çıkan 0. Çalışma zamanının PARAM / CAPS datagramları (`cc_line_charge`) pencere sayımına girmiyor: telde 8 / 7 544 B. |

## İhlaller ve somut engeller (kod yazılmadı)

| # | İhlal | Ölçüm | Somut engel |
|---|---|---|---|
| İ1 | «Tek yoklama» kuralı «geçti» ile kapanan yoklamayı bağlamıyor | Kapalı döngü: hat 6 250 sabit, ilk yoklama 4 568 B (taban hızının altında) ile `Passed` → kalıcı 6 995, kuyruk 594 ms → örnek hakkı yeniler → 8. ACK'te ikinci çift yoklaması (6 / 6 444 B, `Inconclusive`) | Yeni kural gerekir (sahibin cümlesi sonuçsuz kapanışı bağlıyor). Kök: parktaki kanıt eşiği (sahte `Passed`). |
| İ2 | Motor dışı datagramlar yoklama penceresinin adet / tel baytı sayımına girmiyor | Gerçek `Sender`, pencere içinde iki PARAM boyu muaf datagram: telde 8 datagram / 7 544 B, kayıt 6 / 7 368 | Yeni muhasebe yolu (karar 8 ile aynı konu: bütün tel baytlarının hat hesabına girmesi). |
| İ3 | Pencere kurulduktan sonra yazılan tutma pencereyi kaydırmıyor; «fiilen gönderilen» kaydı eksik kalıyor | +605 ms'de 1 500 B paylaşılan hat gözlemi: kayıt 5 / 6 140, telde taban aralığından kısa aralıkla 6 datagram / 7 368 B (bütçe aşılmıyor) | Pencereyi sonraki tutmalara göre yeniden hizalamak yeni kural. |
| İ4 | Bütçeyle kapanan pencerede bir datagram fazla boşaltma (gönderici gereksiz susuyor) | Dar model: yazılan 4 298 B, yoklamanın kuyruğu 3 070 B, hat 196,48 ms boş. Kanıt testi `ek9_a4_a_budget_closed_probe_is_drained_without_extra_silence` `#[ignore]`, kırmızı | Düzeltme hazır ama kapalı döngüde 12 hücreyi nedeni izlenmeden değiştiriyor (sonraki olağan yoklamalar `Passed` → `Inconclusive`) ve pencere içinde susma + sonda gönderimde EKSİK boşaltıyor (140 B, 544 B / 87 ms artık kuyruk). Kayıt #175 7179614439. |
| İ5 | Seyrek (uygulama sınırlı) pencerede artık kuyruk | 3 tam datagram 250 ms arayla: yazılan 0 B, sonraki datagram 89,44 ms bekliyor (559 B) | Datagram başına kuyruk izi (yeni yöntem). `d7039c53`'ten beri var. |
| İ6 | «Kaldı» dalında boşaltma iki kez yazılıyor | Kuyruk 3 070 B iken 2 620 + 5 526 B; hat 687,83 ms boş | #258 öncesi kod; PARK. |
| P | EK 6 şartı (değişmemiş hatta hiçbir karışım kalıcı artış yazmaz) | 376 hücrenin 54'ü kalıcı artış yazıyor | PARK (S2 / kanıt eşiği). |

## Sınanmayanlar

- Bilinmeyen ilk uç kolu (kayıt varken erişilemiyor).
- Geçici evrede `Passed` sonrası aynı ACK'te başlangıç yoklaması.
- Pencere açıkken kapanışta saat taşımayan yol (`seen_ns`): çürütücü gerçek `Sender`'da ölçtü (beklenen 1 283 B, yazılan ≈ 1 270–1 283 B); depoda testi yok.
- `experimental-cold-start` derlemesinde boş toplu olay (`Event::Tick`) yolu: olağan yoklamanın süre sonu o yolda `d7039c53`'e göre geç işlenir (`ee168d3d` öncesiyle aynı); hiçbir testte koşulmuyor.
- Bayt sınırı (7 368 B) adetten bağımsız sınanamıyor: bütçe = 6 × 1 228 B ve datagram ≤ 1 228 B; sınır sonuç olarak sağlanıyor.

## Bu turda değişen ve kayıtlı sapmalar

- Pencere başı: ön kayıt «tutma / hedef» → kod «tutma / taban» (`c7b6e187`, ön kaydı yoktu; EK 9'da kaydedildi) → göndericide pacer'ın açılma anı (EK 11; «geri çekmez» birim testi «eşitler» olarak yeniden yazıldı).
- Kayıtlıyken gelen aşağı örneğin hedefi reddetmesi kuralını ön kayıtsız eklemiştim (EK 9'da kaydedildi); hiç datagram göndermemiş yoklama için kaldırıldı (A2).
- Deney anahtarı motor kütüphanesinde süreç ortamından okunuyor (değişmedi; süzgeç saf yardımcıya çıkarıldı ve test edildi).

Tam tablo (61 satır; kural · kod konumu · test · sonuç · hüküm · sınır notu): `kural-tablosu-tablo.md`.
