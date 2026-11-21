//! [HAT-KORUMASI] Geri bildirimi aç profilde gerçek ileri hattın ölçülmesi: ölçüm profilin hat
//! hızını yalnız DÜŞÜRÜR, hiçbir zaman profilin üstüne çıkarmaz.
//!
//! # Sorun
//!
//! Geri bildirimi aç profilde ([`crate::sender::FeedbackStarvedRule`]) gönderici hızı ve hat
//! modelini profilin yukarı yön hızına sert bağlar; uçuş tabanı `hat × (geri bildirim ufku + RTT)`
//! olur (62 B/s dönüşte ≈ 11 s × hat — bütün dosyadan büyük). Profil gerçek hattan iyimserse
//! göndericinin hiçbir sınırı gerçek hatta inmez: darboğaz kuyruğu hat farkı hızında büyür, PTT ve
//! metin kuyruğun arkasında saniyelerce bekler, kuyruk taşınca paketler düşer.
//!
//! # Neden BBR'ın bant tahmini (BtlBw) değil
//!
//! BtlBw teslim hızıdır (ACK'lenen bayt / ACK aralığı). Bu hatta iki nedenle hattı ölçmez:
//! * ileri yönde rastgele kayıp (ürün hücresinde %20) hattı kullanır ama teslim edilmez: teslim
//!   hızı en iyi hâlde hattın `1 − kayıp` katıdır;
//! * ACK'ler 62 B/s dönüşte saniyelerce aralıkla ve kendi serileşmeleri (65–129 B → 1,05–2,08 s)
//!   kadar kayarak gelir; örnek aralığı ACK varış anlarının farkıdır.
//!
//! Ölçüldü (ürün testi, tohum 1): profil 12 500 B/s iken BtlBw gerçek 12 500 B/s hatta 3,9 kB/s,
//! 6 250 B/s hatta 2,8–2,9 kB/s okudu. Tavan olarak kullanılsa dürüst profili üçte bire düşürürdü.
//!
//! # Ölçüm: gönderim cephesinin alıcıya varış gecikmesi
//!
//! Her ACK çerçevesinin en büyük teyitli iletimi `L` (gönderim anı `s`, o ana dek bu motorun hatta
//! verdiği kümülatif tel baytı `c`) alıcıya `r` anında varmıştır. Darboğaz FIFO'dur: `L`'den önce
//! gönderilen her bayt `r`'den önce hattı geçmiş (teslim edilmiş ya da yolda rastgele düşmüş)
//! ya da kuyrukta düşmüştür. Gönderici `r`'yi bilmez ama ACK'in kendi alanlarından tam türetir:
//!
//! ```text
//! τ = ACK varışı − ack_delay − ACK'in dönüş hattında serileşmesi
//!   = r + dönüş yayılımı            (alıcı ACK'i dönüş hattı boşken çıkarır; bkz. aşağı)
//! ```
//!
//! * `ack_delay` alıcının `L`'yi aldıktan sonra ACK'i tuttuğu süredir (bütçe, hatta yol verme);
//! * ACK'in serileşmesi = (ACK datagramı + UDP/IP zarfı) / profilin dönüş hızı; ACK boyu çalışma
//!   zamanından gelir (`Sender::observe_ack_feedback`).
//!
//! İki ACK noktası `j < i` arasında `Δs = s_i − s_j` gönderim süresi, `Δτ = τ_i − τ_j` alıcıdaki
//! varış süresidir. `Δτ − Δs` cephenin kuyrukta beklediği sürenin artışıdır. Artış ölçüm
//! çözünürlüğünü aşıyorsa darboğaz o aralıkta doluydu ve aralıkta gönderilen her bayt hattan
//! geçti: hat hızı `(c_i − c_j) / Δτ`. Rastgele kayıp tahmini düşürmez (düşen bayt hattı kullandı);
//! artış yoksa örnek yoktur (hat en az gönderim hızı kadardır, ama ne kadar fazla olduğu bilinmez).
//!
//! # Gürültünün yönü
//!
//! Dönüş hattında beklemenin (ACK sıkışması) iki ucu vardır:
//! * başlangıç noktası `j`'nin beklemesi `τ_j`'yi büyütür → `Δτ` küçülür → tahmin YÜKSELİR;
//! * bitiş noktası `i`'nin beklemesi tahmini düşürür. Ürün alıcısı bütçeli ACK'i yukarı yön hattı
//!   kendi baytlarından boşalmadan çıkarmaz (`Receiver::set_uplink_yield`); bekleme `ack_delay`'in
//!   içindedir. Kalan sıkışma için tahmin, saklanan bütün başlangıç noktalarına göre aralıkların
//!   EN BÜYÜĞÜdür: tek bir gecikmiş ACK tahmini aşağı çekmez, aralıktaki boşta kalma (hat boş,
//!   gönderici bekliyor) da yalnız o aralığı düşürür.
//! * Kuyrukta düşen bayt aralığın gönderim baytına girer ama hattı kullanmamıştır → tahmin yükselir.
//!   Ölçüldü (ürün testi, gerçek 6 250 B/s, tohum 2): kuyruk taşarken gönderilmiş baytları kapsayan
//!   aralık 10 024 B/s verdi; önceki temiz aralık 6 249 B/s.
//!
//! Yükselten hatalar korumayı kaldırabilir, düşürenler yalnız hızdan yer. Bu yüzden bir örnek
//! hızı hiçbir zaman o an kullanılan tavanın üstüne çıkaramaz (`min(tavan, örnek)`); profil hattı
//! üst sınırdır. Kabul edilen bir örnek, aralığın gönderim hızının yaklaşık %5 altındadır (kuyruk
//! artışı eşiği): gürültü düzeyindeki artış örnek vermez, hız yalnız gerçek kanıtla iner.
//!
//! # Denetimli yeniden yoklama (2026-11-20)
//!
//! #217'de korunan hız oturum boyunca artmıyordu (kalıcı `min`): oturum içinde hızlanan hat
//! yeniden ölçülmüyor, yanlış bir düşük örnek (ör. dönüş hattında gecikme artışı, aşağı) oturum
//! sonuna dek kalıyordu. Artış kanıtı kuyruğun BÜYÜMEMESİDİR: iki ACK noktası arasında gönderici
//! tavana yakın hızda (≥ %90) en az [`VERIFY_MIN_SPAN_NS`] gönderdi ve cephenin kuyruk beklemesi
//! artmadıysa hat en az o gönderim hızındadır. Bu "doğrulama" tavanı bir adım yükseltir (profil
//! hattına dek). Doğrulanan aralık bugünkü tavanla gönderilmiş olmalıdır (ACK gönderimden
//! saniyeler sonra gelir; eski tavanlı aralık yeni bilgi taşımaz). Adım uyarlanır: başarılı
//! yoklamadan sonra iki katına (en çok %100); bir örnek hattı tavanın altında gösterdiğinde en
//! küçük adıma (%6,25) döner: sabit dar hatta her yoklamanın kurduğu kuyruk küçük kalır, hızlanan
//! hatta adım katlanarak toparlanma birkaç ACK aralığına iner. İlk ölçümden önce adım %50'den
//! başlar (temkinli başlangıçtan profile iki doğrulamada).
//!
//! # Temkinli başlangıç
//!
//! Mekanizma: ilk tavan profil hattının [`START_NUM`]/[`START_DEN`] katıdır, hat modeli de bu
//! tavandadır (pacer'ın burst payı tavanı aşamaz) ve tavan doğrulamalarla yükselir.
//!
//! Seçim ölçümle (ürün testi `asym_product_outcome`, release, sırayla):
//! * 2026-11-20, tohum 1: 1/2'de dürüst dosya 15,0 s, 3/4'te 15,8 s ve 23,2 s (sınır 14 s;
//!   önce 8,7 s) → o gün 1/1 seçildi.
//! * 2026-11-21, tohum 1–3: 17/20'de (0,85) dürüst dosya 9,9–11,0 s; gerçek 10 000 hücresinde
//!   başlangıç PTT 2,1–2,35 s → 0,84–1,87 s. Kalan düşüş: 1 s'lik aralıkta %6'lık aşırı yük
//!   doğrulamayı geçti ve tavan profile çıktı; doğrulama sıkılaştırıldı ([`VERIFY_MIN_SPAN_NS`] =
//!   3 s, [`VERIFY_GROWTH_DIV`]). Sonra: dürüst dosya 10,1–10,3 s, 10 000 başlangıç PTT 0,90–0,97 s.
//!   Bedel: dürüst profilde dosya +1,2–2,0 s (sınır içinde).
//! * AMA 17/20 kalıcı simülasyon kapısını (`tests/line_guard_sim.rs`) bozdu: gerçek 6 250 B/s'de
//!   15 s sonrası darboğaz beklemesi 0,41 s (sınır 0,39 s) ve dürüst profilde dosya tohum 1'de
//!   15,7 → 33,3 s (dosyanın son baytları bir ACK aralığı sonrasına kayıyor). SEÇİLEN DEĞER yine
//!   1/1; 17/20 ölçülmüş seçenektir, bedeliyle sahibe gider. Sıkı doğrulama kalır (1/1'de sim
//!   kapısı geçer).
//! * Gerçek hat tavanın altındaysa (ör. 6 250 B/s) kuyruk ilk ölçüme (ikinci ACK, 3,3–7,9 s) dek
//!   büyüyordu (2026-11-21 öncesi; başlangıç bütçesi ve ilk ACK'in penceresi için aşağıya bakın).
//!
//! # Dönüş gecikmesi değişimi
//!
//! `τ` dönüş yayılımını içerir: dönüş hattının gecikmesi `d` kadar artarsa sonraki her cephe `d`
//! kadar "geç" görünür. İki koruma: (1) taban (`τ − s` en küçüğü) son [`BASE_WINDOW`] noktanın
//! en küçüğüdür, tüm oturumunki değil: kalıcı bir dönüş gecikmesi artışı birkaç ACK sonra tabana
//! girer ve kuyruk sayılmaz (kalıcı yanlış boşaltma yok); (2) tek bir gecikme basamağının verdiği
//! yanlış düşük örnek (aralığın %5'inden büyük basamak) yeniden yoklamayla geri alınır.
//!
//! # Kuyruk boşaltma
//!
//! Korumanın devreye girdiği ana dek gönderici profil hızında göndermiştir; gerçek hatta o farkın
//! kuyruğu birikmiştir. Hız yalnız ölçülen hatta inerse kuyruk kalıcı olur ve sonraki her PTT
//! demeti onun arkasında bekler. Bu yüzden her ölçüm noktasında darboğaz kuyruğu da tahmin edilir:
//! `L`'nin hattı terk ettiği andan bu yana hat `oran × süre` bayt boşaltmıştır; `L`'den sonra
//! gönderilen baytların kalanı kuyruktadır. Gönderici bu kuyruğu kendi hat modeline
//! (`TokenBucket::line_raise_backlog`) yazar: yeni datagram, gerçek kuyruk boşalınca çıkar.
//! `L`'nin hattı terk ettiği an `τ − taban`'dır; taban, görülen en küçük `τ − s` (kuyruksuz iki
//! yönlü yayılım; oturumun ilk paketleri boş hatta gider).
//!
//! Bayt sayımı kuyrukta düşmüş baytı da kuyrukta sayar (gönderici hangi baytın düştüğünü bilmez);
//! kuyruk taşmışsa tahmin gerçek kuyruktan büyüktür ve gönderici gereğinden uzun bekler — PTT ve
//! metin de bekler. Ölçüldü (ürün testi, gerçek 6 250 B/s, tohum 2–3): kuyruk ≈ 3 s iken bekleme
//! 5,5–6 s. Bu yüzden yazılan kuyruk ayrıca `L`'nin kendisinin kuyrukta beklediği süreyle
//! (`τ − s − taban`) sınırlıdır: gönderici `L`'den bu yana en az hat hızında gönderdiyse kuyruk o
//! andan küçülmemiştir, bu değer gerçek kuyruğun alt sınırıdır. Sınır, kuyruk `L`'den sonra da
//! büyüdüyse eksik boşaltır; kalan, bir sonraki ölçüm noktasında yine ölçülür ve yazılır.
//!
//! [İŞ-35] Bekleme sınırı yalnız bu cephenin değil, son [`POINT_CAP`] noktanın en büyük cephe
//! beklemesidir (yine `bayt` tahminiyle — gönderilen − boşalan — sınırlı: kuyruk boşaldıysa eski
//! bir tepe boşaltma yazdırmaz). Örnek kuyruğun cepheden sonra da büyüdüğü bir aralıkta tek adımda
//! gelirse (ölçüm boşluğundan dönüşte gönderici tam tavanda) tek cephenin beklemesi gerçek kuyruğun
//! çok altında kalıyordu. Ölçüldü (benzetim `e2`, tohum 1): tek cepheyle yakınsamadan 2 s sonra
//! darboğaz beklemesi 1,73 s; en büyükle iki datagram sınırının içinde.
//!
//! # Süreli yoklama (İŞ-33 C3, 2026-11-21)
//!
//! Doğrulamadan sonra tavan kalıcı yükseliyordu. Hat değişmediyse yükseltilmiş tavan bir ACK
//! aralığı boyunca (5–8,6 s) kuyruk kurar; bu artış örnek eşiğinin (`max(MTU, Δs/20)`) altında
//! kalabildiği için birkaç aralık fark edilmeden birikir. Ölçüldü (kalıcı sim, gerçek 6 250 B/s,
//! tohum 1): tavan 6 636'ya yükseldi, kuyruk iki aralıkta 0,64 s'ye çıktı ve örnek gelmedi; ürün
//! testinde aynı hücrede PTT 1,17 s.
//!
//! Şimdi doğrulama tavanı yalnız kısa bir süre yükseltir (yoklama: hat değişmediyse
//! [`PROBE_QUEUE_NS`] kadar kuyruk kuracak süre, en az sekiz datagram, en çok
//! [`VERIFY_MIN_SPAN_NS`]), sonra hükme
//! dek önceki tavana döner ([`LineGuard::tick`]). Hat değişmediyse yoklamanın kurduğu kuyruk
//! [`PROBE_QUEUE_NS`]'tir (%6,2 adımda ≈ 1,5 s yoklama, ≈ 90 ms kuyruk). Hükmü, cephesi yoklama bittikten sonra
//! gönderilmiş ilk ACK verir: yoklamanın başladığı noktaya göre kuyruk artışı beklenenin (hat
//! değişmediyse yoklamanın kuracağı kuyruk) yarısından küçükse hat yoklama hızındadır, tavan
//! kalıcı yükselir ve adım katlanır; değilse tavan önceki değerde kalır ve ölçülen artış hat
//! modeline yazılır (boşaltma). Örnek eşiğini aşan artış her zamanki gibi örnektir.
//!
//! # Başlangıç bütçesi ve tek ACK'ten ilk ölçüm (İŞ-33, 2026-11-21)
//!
//! İlk ACK'ten önce gönderici gerçek hattı bilmez. Profil iyimserse (gerçek hat beyanın yarısı)
//! profil hızında gönderilen her saniye darboğazda bir saniye kuyruk kurar. Bu yüzden ilk ACK
//! işlenene dek hatta verilen toplam tel baytı (motorun her iletimi ve motor dışı datagramlar)
//!
//! ```text
//! A(t) = max(P × START_BURST_NS, P/2 × t)        P = profil hattı, t = ilk iletimden beri
//! ```
//!
//! ile sınırlıdır ([`LineGuard::start_budget_admits`]). Gerçek hat `P/2` iken kuyruk en çok
//! [`START_BURST_NS`] kadar büyür (ilk patlamanın sonunda) ve `2 × START_BURST_NS`'te boşalır;
//! hat `P/2`'den hızlıysa kuyruk daha küçüktür. Bütçe yalnız öncelikli olmayan iletimleri
//! (dosya, onarım sembolü) bekletir; PTT, metin ve denetim bütçeye sayılır ama beklemez
//! (`Sender`'daki çağrı yerleri).
//!
//! [İŞ-35: bütçe yalnız doğrulamada ya da ölçmeyen alıcıda ilk ACK'le kalkar; diğer sonuçlarda
//! ikinci ölçüme dek sürer, aşağıda "İkinci ölçüm".] İlk ACK alıcının ilk varış penceresinin hızını taşır
//! ([`crate::arrival_rate`]): ilk patlamanın birkaç datagramı. Gönderici pencerenin ilk ve son
//! seq'ini kendi kaydından bulur, pencere süresini `bayt / hız` olarak geri kurar ve yukarıdaki
//! iki nokta tahmincisini bu tek ACK'e uygular ([`LineGuard::on_first_window`]):
//! * kuyruk artışı pencerenin %5'ini aşıyorsa örnek: tavan ölçülen hatta iner (ilk ölçüm ikinci
//!   ACK'ten ≈ 4,6 s yerine ilk ACK'te ≈ 2 s), ama `P/2`'nin altına inmez (İŞ-35);
//! * artış yoksa ve pencere tavanın en az %90'ıyla gönderildiyse hat en az tavan hızındadır
//!   (doğrulama): tavan profilde kalır;
//! * alan sentinel ise (eski alıcı, pencere kapanmadı) ya da pencere kurulamazsa bilgi yoktur:
//!   eski alıcıda (ölçüm aralığı da yok) bütçe yine kalkar, gönderici bugünkü yola döner; ölçen
//!   alıcıda bütçe ikinci ölçüme dek sürer (İŞ-35).
//!
//! Patlama pencereyi kapsamalıdır: pencere ilk datagramdan sonra
//! [`crate::arrival_rate::FIRST_WINDOW_BYTES`] (yarım MTU) alınmış gövde baytıdır;
//! `P × START_BURST_NS` = 6 250 B (P = 12 500 B/s) ≈ beş dosya parçası, %20 kayıpla en az ikisi
//! ≈ %99,3 olasılıkla ulaşır.
//! Pencere patlamadan sonra gönderilen datagramla kapanırsa kuyruk boşalmış olabilir: artış
//! görünmez, sonuç `NoInfo`'dur (bugünkü yol). Profil hattının patlaması pencereden küçükse (dar
//! profil) de böyledir.
//!
//! # İkinci ölçüm: tek örnek kesin kapasite değildir (İŞ-35, 2026-11-22)
//!
//! İlk pencere birkaç datagramdır; tek bir bozulma sonucu değiştirir. Ölçüldü
//! (`tests/start_budget_sim.rs`, gerçek 6 250, beyan 12 500, tohum 1–3, bu kuraldan önce):
//! * alıcının boy sayımı datagram başına +100 B: örnek 6 873–6 894 (+%10), kuyruk 0,87–1,14 s;
//! * açıcının alıcı damgası 40 ms geç: örnek 7 113–8 289, kuyruk 1,17–2,25 s, başlangıç PTT
//!   1,16–1,96 s;
//! * damga 200 ms geç: `Δτ < Δs` ile yanlış doğrulama (tohum 1) ya da sentinel (tohum 2–3); bütçe
//!   kalkar, profil hızı: kuyruk 2,9–3,6 s, 31–56 kuyruk düşmesi;
//! * yeniden sıralama, tohum 3: yanlış DÜŞÜK örnek 2 215 → hat modeli PTT şeridinin kendi tel
//!   hızının (5 iletim × 114 B × 5/s ≈ 2,85 kB/s) altında, PTT 15,9 s;
//! * açıcıdan sonra kuyrukta düşme: örnek 9 372 (tohum 1) ya da yanlış doğrulama (tohum 2–3).
//!
//! Kural ([`LineGuard::settle_start_budget`]):
//! * Bütçe ilk ACK'te yalnız iki durumda kalkar: pencere hattı doğruladı ya da alıcı ölçmüyor (ne
//!   pencere ne ölçüm aralığı: eski alıcı, bugünkü yol). Doğrulama tek ölçüm olduğu hâlde güvenilir:
//!   onu da ikinci ölçüme bekletmek dürüst profilde dosyayı en az ≈ 1,1 s geciktirir (aşağıda
//!   "Sınır"); dürüst bütçe bedeli testinin payı 0,53 s.
//! * Diğer her sonuçta (örnek, bilgi yok, sentinel) geçici evre başlar: bütçe (`P/2`) sürer. Tek
//!   örnek tavanı `P/2`'nin altına indirmez (yanlış düşük örnek öncelikli şeridi aç bırakmaz; hat
//!   gerçekten `P/2`'nin altındaysa bütçe varsayımı zaten bozuktur ve ikinci ölçüm onu ölçer).
//! * Pencerede `Δτ`, `Δs`'den çözünürlüğün (`max(Δs/20, FIRST_WINDOW_MIN_WAIT_NS)`) ötesinde kısa
//!   ise sonuç doğrulama değil "bilgi yok"tur: FIFO'da sabit yayılımla imkânsızdır (açıcının damgası
//!   geç ya da iki ucun boy sayımı farklı); gerçek bekleme bilinmez.
//! * Geçici evreyi sonraki bir ACK'in ölçüm aralığı kapatır (ilk ACK'te kayıtlar budanmaz: ikinci
//!   ACK'in M'si alıcıda baştan, taban 0, başlar):
//!   - kuyruk artışı → örnek: güvenilir (iki bağımsız ölçüm), `P/2`'nin altına da iner;
//!   - artış yok ve gönderici bütçenin hızında gönderdi (aralığın bütçe dolumu başladıktan sonraki
//!     en az 0,5 s'lik kısmında `P/2`'nin en az %90'ı; ilk patlamadan sonraki bekleme kanıt
//!     değildir, hat o sırada boşalır) → hat en az `P/2`: tavan `P/2`'ye iner ve tek ölçümün
//!     iddiası (örneğin hızı; bilgi yoksa profil hattı) tek bir kısa yoklamayla sınanır (hat
//!     `P/2`'deyse kuyruk [`PROBE_QUEUE_NS`], en az iki datagram). Hüküm, ACK ölçüm aralığı
//!     taşıyorsa onun kuyruk artışıyla (dönüş gecikmesi girmez; cephe ölçüsü dönüş gecikmesi
//!     300 → 700 ms artarken dürüst hattı yanlış kesiyordu: `starved_line_sim` c, d3);
//!   - gönderici bütçenin altında kaldıysa (uygulama sınırlı) karar yok, evre sürer (bütçe zaten
//!     bağlamaz).
//!
//! Bedel (aynı düzenek, dosyanın son yeni biriminin anı, önce → sonra): bozulmasız dürüst profil
//! değişmez (5,96 / 5,32 / 5,64 s). Gerçek 10 000: 7,67 / 7,01 / 6,90 → 12,86 / 11,67 / 16,47 s
//! (örnek ikinci ölçüme dek `P/2`'de, yoklamanın hükmü bir sonraki ACK'te; bu düzenekte ACK aralığı
//! sürekli evrede ≈ 8,6 s). Dürüst profilde pencere bozulup bilgi vermezse (boy farkı, damga
//! gecikmesi, sentinel) aynı yol: 5,3–6,1 → 9,5–13,1 s.
//!
//! Sınır (ÇÖZÜLMEDİ): kuyrukta düşen datagram penceredeki artışı gizlerse (`QueueDropAfterOpener`,
//! tohum 2–3) pencere yanlış doğrular. Gözlem dürüst profilde rastgele kaybın açtığı boşlukla
//! aynıdır; gerekçe ve alt sınır `tests/start_budget_sim.rs`
//! (`a_queue_drop_hidden_in_the_first_window_is_not_closed`).
//!
//! # Ölçüm aralığı (İŞ-34, 2026-11-22; sözleşme #175 7067964700)
//!
//! Cephe ölçüsü (`τ − s`) dönüş yayılımını içerir. Gönderici bir aralıkta tavanın altında
//! gönderirken dönüş gecikmesi `Δ` artarsa örnek `gönderim hızı × ds / (ds + Δ)` çıkar ve hat o
//! hıza iner. Ürün testi, dönüş-gecikmesi hücresi, bitmap kolu, tohum 2: dürüst 12 500 hat 7 793'e
//! indi. Bitmap'in sık ACK'leri `ds`'yi kısalttı, böylece 100 ms'lik basamak eşiği aştı.
//!
//! Ölçüm aralığı bu ölçüyü alıcının veri varış süresiyle değiştirir: M = alıcının aldığı seq'ler
//! (`m_first … largest`), `rx_span` = alıcıda ikisinin ilk varışları arası. Kuyruk artışı
//! `g = rx_span − send_span − (ser(last) − ser(first))`; dönüş gecikmesi `g`'ye girmez.
//! Aşağı yönlü örnek yalnız `g` artış eşiğini (`max(MTU serileşmesi, send_span / 20)`) aşınca
//! alınır. Hız `(first, last]` aralığında gönderilen tel baytı / `rx_span`'dir.
//! * Rastgele kayıp alınan baytı düşürür ama `g`'yi büyütmez: örnek yok.
//! * Gönderici kapasitenin altında gönderiyorsa kuyruk büyümez: `g ≈ 0`, örnek yok.
//! * Kuyrukta düşen seq hattı kullanmamıştır ama göndericinin baytına girer. Taşmanın olduğu
//!   aralıkta tahmin gerçek hattın üstünde kalır; sonraki aralıklar düşürür.
//! * Yeniden sıralamada m_first geç gelirse `rx_span` kısalır; daralma küçük görünür (güvenli
//!   taraf: yanlış kesinti yok).
//!
//! **Cephe tahmincisi ne zaman kalır.** Ölçüm aralığı yalnız alıcısı ölçen oturumda gelir (CAPS ile
//! 0x15). İlk aralık gelene dek (eski alıcı, V2 çerçevesi, ilk ACK'ler) cephe tahmincisi bugünkü
//! kuralıyla aşağı yönlü örnek verir: başka ölçü yoktur, gerçek daralmada kuyruğun sınırı odur.
//! **Eski alıcı / V2 oturumunda (hiç ölçüm gelmeyen) davranış İŞ-35 öncesiyle aynıdır:** aralık
//! kipine girilmez, boşluk kuralı kurulmaz, cephe örnekleri bugünkü gibi alınır (gerçek eski
//! ikiliyle uyum; o oturumda dönüş gecikmesi artışının yanlış kesme riski de bugünkü gibi kalır).
//! Bir aralık geldiğinde oturum aralık kipine girer; o andan sonra cephe tahmincisi hiçbir zaman
//! aşağı yönlü örnek vermez (aşağıda "Ölçüm boşluğu").
//! * Yukarı yönlü doğrulama, süreli yoklamanın hükmü ve kuyruk tahmini (taban gecikme) bu adımda
//!   cepheden kalır. Dönüş basamağında yoklama hükmü "sonuçsuz" çıkar ve kuyruk yazmaz (güvenli
//!   taraf).
//!
//! Kayıtlar (`sent`) son iki ACK'in cephelerinin eskisine dek tutulur: bir sonraki ACK'in M'si iki
//! ACK geriye uzanır ([`SENT_CAP`] yine üst sınırdır).
//!
//! # Ölçüm boşluğu (İŞ-35 AÇIK 1, 2026-11-21)
//!
//! İŞ-34 kuralı aralık kipinde üst üste iki ACK ölçümsüz kalınca cephe örneğine dönüyordu. Ölçüm
//! yokluğu cephe tahmininin doğru olduğu anlamına gelmez: dönüş, dönüş gecikmesini ileri daralma
//! sanan yöntemin kendisini geri getirir. Ölçüldü (benzetim `e1`: dürüst 12 500, ürün dönüş
//! çizelgesi, alıcı 5–13 s'de ölçmüyor, göndericiye 3–4 ölçümsüz ACK): hat 11 667 / 10 710 / 9 226'ya
//! indi (tohum 1–3).
//!
//! Değerlendirilen seçenekler:
//! * (i) Ölçülü aralıkları zincirleyip kuyruk düzeyini izlemek (`Q(n) = Q(n−2) + g(n)`; gerçek
//!   alıcıda `m_first(n) ≈ largest(n−2) + 1`): boşlukta `g` yoktur, boşluğu kapatmaz. Ayrıca küçük
//!   FIFO'da düzey gürültülü ölçüldü: 8 paketlik dolu kuyrukta bekleme paket karışımına göre
//!   (8 × 114 … 8 × 1 228 B) 0,15–1,57 s arasında değişir; zincirlenen düzey sabit kuyrukta ≈ ±0,2 s
//!   oynadı. Reddedildi.
//! * (ii) Ölçülü ACK'lerde dönüş gecikmesi değişimini (cephe gecikme değişimi − `g`) ölçüp boşlukta
//!   cephe kanıtını yalnız gözlenen değişimin ötesinde ve art arda kabul etmek: boşluk içindeki
//!   dönüş değişimi gözlenemez (önceki ölçüm sonraki değişimi öngörmez) ve ürün çizelgesi (3 s'de
//!   bir 300 → 400 → 500 → 700 ms) art arda büyüyen cephe gecikmesi üretir — kuyruk büyümesiyle aynı
//!   imza. Reddedildi (akıl yürütme; benzetimle sınanmadı).
//! * (iii) SEÇİLDİ: kapasite iddia etmeyen, kuyruğu sınırlayan boşluk bütçesi.
//!
//! Kural: aralık kipinde üst üste [`UNMEASURED_GAP_ACKS`] (2) ACK ölçümsüz kalınca (tek ölçümsüz ACK
//! olağandır: M tek seq, pencere dışı), o anki gönderim tavanı `R` ile:
//! * öncelikli olmayan iletimler (yeni veri, onarım sembolü ve — yalnız boşlukta — yeniden iletim)
//!   toplam tel baytı `A(t) = max(1 MTU, R/2 × t)` bütçesiyle bekler (başlangıç bütçesinin kovası,
//!   ilk patlama yok); PTT, metin ve denetim bütçeye sayılır ama beklemez;
//! * kurulurken tek seferlik boşaltma: cephenin kuyruk beklemesi (`τ − s − taban`) kadar süre hat
//!   modeline yazılır (`R ×` bekleme). Dönüş gecikmesi artışı da buna girer: dürüst hatta bedel bir
//!   kez o kadar bekleme, hız değil;
//! * `rate_bps` ve gönderim tavanı yazılmaz; ilk ölçülü ACK bütçeyi kaldırır, sonrası olağan aralık
//!   kipidir (daralma varsa ölçülür).
//!
//! Kanıt (benzetim, `tests/starved_line_sim.rs`): `e1` hız ve tavan profilde kaldı; `e2` (8–19 s
//! ölçüm yok, gerçek 6 250) boşlukta darboğaz beklemesi 1,82 s → 0,36–0,61 s (sınır 0,70 s), ölçüm
//! dönünce yakınsama sınır içinde. Ara ölçüm: yeniden iletim bütçe dışındayken (kuyrukta düşen her
//! iletim yeniden iletilir) bekleme 2,60 s kalıyordu.
//! Varsayım ve sınır: gerçek hat `R/2`'den yavaş değilse kuyruk boşluk süresince büyümez; daha
//! yavaşsa `(R/2 − L)/L` hızında büyür (bütçesiz `(R − L)/L`). Boşluğun ilk iki ACK'i boyunca
//! (≈ iki ACK aralığı + dönüş geçişi) koruma yoktur: tek ölçümsüz ACK'in olağan olmasının bedeli.
//! Dürüst hatta bedel: boşluk süresince dosya yarı hızda (benzetim: `e1` dosya 16,62 / 12,60 /
//! 12,71 s, aynı koşu boşluksuz `a` 14,74 / 14,37 / 12,26 s).
//!
//! **Kurulma koşulu (birleşimde bulundu, 2026-11-22).** Bütçe öncelikli trafiği de kovadan düşer.
//! Öncelikli trafik `R/2`'ye yaklaşınca öncelikli olmayan işe kovadan pay kalmaz: ilk pencere
//! benzetiminde (`tests/start_budget_sim.rs`, seyrek ACK, art arda ölçümsüz ACK) hat 6 250
//! ölçülmüşken 5 iletimli PTT (2,85 kB/s) bütçeyi doldurdu, dosya 90 s içinde bitmedi, 2 PTT demeti
//! teslim edilmedi (6 test). O yükte bütçe kuyruğu zaten koruyamaz. Bu yüzden kurulurken son
//! [`GAP_PRIORITY_WINDOW_NS`] (2 s) içindeki öncelikli tel baytı ölçülür. Öncelikli hız `3R/8`'e
//! ulaşıyorsa (öncelikli olmayan işe `R/8`'den az kalır) bütçe kurulmaz ([`LineGuard::gap_skips`]).
//! * Reddedilenler: öncelikli trafiği kovadan hiç düşmemek ve öncelikli olmayan işe her zaman
//!   `R/8` alt pay vermek. İkisi de `e2`'de (boşlukta gerçek daralma) toplamı gerçek hattın üstüne
//!   çıkardı: boşluk beklemesi 1,12–1,36 s (sınır 0,70 s).
//! * Bekletilen öncelikli olmayan yeniden iletim o turun planını keser (`break`, İŞ-35 ilk hali).
//!   `continue` (öncelikli yeniden iletimler beklemeden çıksın) denendi: `e2` tohum 2'de
//!   yakınsamadan sonra PTT 5,96 s (sınır 1 s). Boşlukta dolu FIFO'ya erken giren PTT yeniden
//!   iletimleri düşüyor, kurtarma bir ACK turu daha bekliyor. Bu nedenle `break` kaldı.
//!
//! # Taşma (İŞ-35 AÇIK 3, 2026-11-21)
//!
//! FIFO küçükse (8 paket) daralmada kuyruk hemen dolar ve sabit kalır (`g ≈ 0`); fazlalık kuyrukta
//! düşer. "Kuyruk büyümüyor" tek başına "hat yeterli" demek değildir. Ölçüldü (benzetim `d4`,
//! 12 500 → 6 250, İŞ-34 kuralı): tohum 1'de yakınsama 53,7 s (sınır 17,1 s); iki tahminci de geç
//! ve eksik gördü.
//!
//! Ek kanıt: aralığın teslim kesri `f = alınan tel baytı / gönderilen tel baytı` (`(first, last]`;
//! alınanlar M'den, boylar göndericinin kaydından, [`LineGuard::received_wire`]). Rastgele kayıpta
//! `f ≈ 1 − p`; `S/L` oranında taşmada `f ≈ (L/S)(1 − p)`. Taban `1 − p`, kuyruk kanıtı olmayan ve
//! kuyruğu değişmeyen aralıklardan birikir (sınırlı toplam, tavanda yarılanır: kalıcı kayıp
//! değişimini izler). Sözleşme kuralı gereği düşük teslim tek başına kanıt değildir: taşma örneği
//! yalnız
//! * son [`OVERFLOW_WINDOW_ACKS`] ölçülü ACK'te bir artış örneği varken (kuyruk büyümesi: kayıptan
//!   bağımsız kanıt; hattın tavanın altında olduğunu zaten göstermiştir),
//! * `f` tabanın [`OVERFLOW_EXCESS_NUM`]/[`OVERFLOW_EXCESS_DEN`]'inin altındayken,
//! * kuyruk boşalmıyorken (`g ≥ −eşik`; boşalan kuyrukta darboğaz aralığın bir kısmında boştur) ve
//! * gönderici tavanın en az %90'ıyla gönderirken
//!
//! alınır; hız = (alınan bayt / taban kesri) / `rx_span` (darboğaz meşgulken hattı geçen bayt).
//! Artış görünen aralıkta kullanılmaz: tek aralıkta rastgele kayıp gürültüsü büyüktür (≈ 18 dosya
//! parçası, ≈ ±%12). Ölçüldü (`e2`, tohum 3): artış aralığında taşma tahmini 4 896 verdi, gerçek
//! 6 250; artış örneği kendisi sonraki aralıklarda iner.
//! Ölçülen kesirler (benzetim): rastgele kayıpta 0,62–0,92 (taban ≈ 0,73–0,80), 8 paketlik FIFO
//! taşmasında 0,37–0,49. Kanıt: `d4` yakınsama 15,41–16,24 s (sınır 17,08 s), yakınsamadan 2 s
//! sonra kuyrukta düşme yok; `d1` (%20 rastgele kayıp) ve `d3` (yeniden sıralama) örnek vermiyor.
//! Sınırlar: (1) artış hiç görünmezse (FIFO bir ACK aralığındaki artış eşiğinden küçük) taşma
//! örneği de yoktur; (2) rastgele kayıp oranı tam artış penceresinde yükselirse (ör. karıştırma
//! başlangıcı) hat fazla kesilir, yoklama geri alır; (3) taşma örneği de tavanın üstüne çıkamaz.
//!
//! Determinizm: tamsayı aritmetiği, RNG yok, sınırlı bellek (ADR-17 / SR-AUDIT-001).

use std::collections::VecDeque;

use crate::pacer::LineModel;

/// Gönderim kaydı tavanı (SR-AUDIT-001). Tavan dolunca en eski kayıt düşer; cephesi düşmüş bir
/// kayda çarpan ACK örnek vermez (tahmin yalnız zayıflar).
const SENT_CAP: usize = 8192;
/// Saklanan ACK noktası (aralık başlangıcı) sayısı.
const POINT_CAP: usize = 8;
/// Kuyruk artışı en az aralığın bu kesri olmalı (1/20 = %5): daha küçük artış ölçüm
/// çözünürlüğündedir (zamanlayıcı, ACK gecikmesi kuantumu, hattı paylaşan küçük denetim
/// datagramları) ve örnek vermez.
const GROWTH_MIN_DIV: u64 = 20;
/// [YENİDEN-YOKLAMA] Doğrulama aralığının en kısa gönderim süresi (ns).
const VERIFY_MIN_SPAN_NS: u64 = 3_000_000_000;
/// [YENİDEN-YOKLAMA] Doğrulamada kuyruk artışı aralığın bu kesrinden (1/40 = %2,5) ve bir tam
/// datagramın serileşmesinden küçük olmalı. Örnek eşiğinden (%5) sıkı: arada kalan artış ne örnek
/// ne doğrulama verir. Ölçüldü (ürün testi, gerçek 10 000, tavan 10 625, tohum 1): 1 s'lik aralıkta
/// %6'lık aşırı yük çözünürlüğün (1 MTU ≈ 115 ms) altında kaldı, tavan profile çıktı, kuyruk 1,5 s.
const VERIFY_GROWTH_DIV: u64 = 40;
/// [YENİDEN-YOKLAMA] Doğrulamada gönderim hızı tavanın en az bu binde biri olmalı (gönderici
/// uygulama sınırlıyken — ör. yalnız PTT — doğrulama yok).
const VERIFY_MIN_PERMILLE: u64 = 900;
/// [YENİDEN-YOKLAMA] Yoklama adımı (binde): başlangıç, en küçük, en büyük.
const STEP_START_PERMILLE: u64 = 500;
const STEP_MIN_PERMILLE: u64 = 62;
const STEP_MAX_PERMILLE: u64 = 1000;
/// [TEMKİNLİ-BAŞLANGIÇ] İlk tavan = profil hattı × START_NUM / START_DEN.
pub const START_NUM: u64 = 1;
pub const START_DEN: u64 = 1;
/// [DÖNÜŞ-GECİKMESİ] Taban penceresi (ACK noktası).
const BASE_WINDOW: usize = 4;
/// [BAŞLANGIÇ-BÜTÇESİ] İlk ACK'ten önce profil hızında patlama süresi (ns): gerçek hat beyanın
/// yarısıysa kurulan en büyük kuyruk (patlamanın sonunda; `2 × START_BURST_NS`'te boşalır). PTT
/// açıldığında (ürün 0,5 s) kuyruk en çok bu kadardır ve sonraki demetlerde iner: PTT sınırı (1 s) −
/// ileri yayılım (ürün testi 0,3 s) − demetin serileşmesi − zamanlama payı. Patlama ilk varış
/// penceresini (ilk datagram + yarım MTU) %20 kayıpla da kapsamalıdır: 12 500 B/s profilde 6 250 B
/// ≈ beş dosya parçası; pencere patlama dışına ancak beşten en çok biri ulaşırsa taşar (≈ %0,7).
/// Dört parçada (0,4 s) bu olasılık ≈ %2,7'dir (simde tohum 1'in kayıp dizisi). [CAL]
pub const START_BURST_NS: u64 = 500_000_000;
/// [İLK-PENCERE] Pencerenin son datagramının kuyruk beklemesi bu süreyi aşmazsa darboğaz dolu
/// sayılmaz (ns). Zamanlama çözünürlüğü: alıcı varış anını açma/ayrıştırma sonrasında alır, çalışma
/// zamanı döngüsü 1 ms, röle 0,2 ms; iki zaman damgası. Gerçek 10 000 B/s'de tek parçanın beklemesi
/// ≈ 21 ms, gerçek 6 250'de ≈ 86 ms. [CAL]
pub const FIRST_WINDOW_MIN_WAIT_NS: u64 = 5_000_000;
/// [SÜRELİ-YOKLAMA] Hat değişmediyse bir yoklamanın kuracağı kuyruk (ns): yoklama süresi
/// `PROBE_QUEUE_NS × taban / (yoklama − taban)` (adım %6,2'de ≈ 1,3 s; en çok
/// [`VERIFY_MIN_SPAN_NS`]). PTT payının (1 s) gerçek 6 250 B/s'deki yapısal bileşenlerinden — ileri
/// yayılım 0,3 s, önünde serileşen bir parça 0,17 s, kopya yayılımı 0,16 s, demet 0,02 s — sonra
/// kalan ≈ 0,35 s'nin yaklaşık dörtte biri; hükmün eşiği bunun yarısı (40 ms), cephe gecikmesinden
/// göndericinin kendi geçici kuyruğu düşüldükten sonraki zamanlama gürültüsünün (birkaç ms) çok
/// üstünde. Ölçüldü: 3 s'lik yoklama (≈ 0,19 s kuyruk) ürün testinde gerçek 6 250'nin sürekli
/// evresinde PTT'yi 0,97–1,25 s'ye itiyordu. [CAL]
pub const PROBE_QUEUE_NS: u64 = 80_000_000;

/// [ÖLÇÜM-ARALIĞI · İŞ-34 sözleşmesi §2–3] Alıcının ölçtüğü ileri varış aralığı (motor seq
/// uzayında): M'nin en küçük (`first_seq`) ve en büyük (`last_seq`) seq'i ve alıcıda ikisinin ilk
/// varışları arasındaki süre (`rx_span_ns`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardInterval {
    pub first_seq: u64,
    pub last_seq: u64,
    pub rx_span_ns: u64,
    /// [TAŞMA · İŞ-35] M'nin `(first_seq, last_seq]` içindeki alınan seq'lerinin tel baytı
    /// (göndericinin kaydından, [`LineGuard::received_wire`]); `None` = hesaplanamadı.
    pub rx_wire: Option<u64>,
}

/// [TAŞMA · İŞ-35] Teslim kesri tabanın bu oranının altındaysa aşırı kayıp (3/4): tek aralıkta
/// rastgele kaybın (%20) gürültüsü ≈ ±0,09 (≈ 40 datagram); 2:1 taşmada kesir tabanın yarısıdır.
/// Ölçüldü (benzetim, `tests/starved_line_sim.rs`): rastgele kayıpta kesir/taban ≥ 0,9, 8 paketlik
/// FIFO'nun taşmasında 0,50–0,67. [CAL]
const OVERFLOW_EXCESS_NUM: u64 = 3;
const OVERFLOW_EXCESS_DEN: u64 = 4;
/// [TAŞMA] Kuyruk kanıtının (artış örneği) geçerli kaldığı ölçülü ACK sayısı: M iki ACK geriye
/// uzanır; artışın görüldüğü aralıktan sonraki iki aralık geçişi kapsar.
const OVERFLOW_WINDOW_ACKS: u32 = 3;
/// [TAŞMA] Kesrin hesaplandığı en küçük aralık (tam datagram sayısı, tel): küçük aralıkta kesir
/// gürültüdür.
const OVERFLOW_MIN_MTUS: u64 = 8;
/// [TAŞMA] Tabanın bilindiği en küçük birikim ve tavanı (tam datagram, tel); tavanda iki toplam
/// yarılanır (sınırlı, uyarlanır).
const OVERFLOW_FLOOR_MIN_MTUS: u64 = 16;
const OVERFLOW_FLOOR_CAP_MTUS: u64 = 256;

/// [ÖLÇÜM-BOŞLUĞU · İŞ-35] Aralık kipindeki oturumda üst üste bu kadar ACK ölçümsüz kalırsa
/// boşluk kuralı kurulur (modül belgesi, "Ölçüm boşluğu"): cephe örneği DEĞİL, kuyruk sınırlı
/// bütçe. Tek ölçümsüz ACK olağandır (M tek seq, pencere dışı).
const UNMEASURED_GAP_ACKS: u32 = 2;

/// [ÖLÇÜM-BOŞLUĞU] Boşluk bütçesi kurulurken öncelikli trafiğin (PTT, metin, denetim) son bu kadar
/// süredeki gönderim hızı ölçülür; `3R/8`'e ulaşıyorsa (öncelikli olmayan işe `R/8`'den az kalır)
/// bütçe kurulmaz (modül belgesi, "Ölçüm boşluğu"). İki PTT demet aralığının on katı: kopya dalgasını ve yeniden iletimleri ortalar.
const GAP_PRIORITY_WINDOW_NS: u64 = 2_000_000_000;

/// Ölçüm sonucu: korunan hat hızı ve darboğazda tahmin edilen kuyruk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineGuardUpdate {
    /// Korunan hat hızı (B/s, tel baytı — zarf dahil); profil hattını aşmaz.
    pub rate_bps: u64,
    /// Darboğaz kuyruğunda şu an beklediği tahmin edilen kendi baytlarımız (zarf dahil).
    pub backlog_bytes: u64,
    /// [SÜRELİ-YOKLAMA] Mevcut kuyruğun üstüne eklenecek bayt: başarısız yoklamanın fazlası
    /// (`TokenBucket::line_add_backlog`). Kuyruk tahmini birkaç ACK süren kuyruğu tabana katıp
    /// göremeyebilir; yoklamanın kendi fazlası ölçülüdür.
    pub extra_backlog_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct SentRec {
    seq: u64,
    send_ns: u64,
    /// İletimin UDP gövde boyu (ilk varış penceresinin bayt kuralı).
    udp: u64,
    /// Bu iletim dahil hatta verilen kümülatif tel baytı.
    cum: u64,
    /// İletim anındaki gönderim tavanı (B/s, tel).
    pace: u64,
    /// [SÜRELİ-YOKLAMA] İletim anında göndericinin hat modelinde bu iletim dahil kendi baytlarımızın
    /// boşalmasına kalan süre (ns): cephenin kendi geçici kuyruğu (önündeki parça, art arda kopyalar).
    ahead_ns: u64,
    /// [ÖLÇÜM-BOŞLUĞU] Öncelikli şeridin iletimi (PTT, metin, denetim).
    pri: bool,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    tau_ns: u64,
    send_ns: u64,
    cum: u64,
    pace: u64,
    ahead_ns: u64,
}

/// Geri bildirimi aç profilde gerçek ileri hattın tahmincisi (modül belgesi).
#[derive(Debug, Clone)]
pub struct LineGuard {
    /// Profilin yukarı yön hattı (üst sınır).
    line: LineModel,
    /// Profilin dönüş hattı (ACK serileşmesi için).
    return_line: LineModel,
    /// Hatta verilen kümülatif tel baytı (motorun iletimleri + motor dışı datagramlar).
    cum: u64,
    sent: VecDeque<SentRec>,
    points: VecDeque<Point>,
    /// Son ölçüm noktasının cephesi (seq); cephe ilerlemeyen ACK nokta üretmez.
    frontier: Option<u64>,
    /// Son [`BASE_WINDOW`] noktanın `τ − s` değerleri (ns); taban bunların en küçüğü.
    lags: VecDeque<u64>,
    /// Bir sonraki ACK çerçevesinin tel boyu (UDP gövdesi; `note_ack_wire`).
    pending_wire: Option<u64>,
    /// Korunan hat hızı (B/s, tel); `None` = ölçüm hattı profilin altında göstermedi (ya da
    /// yoklama profil hattına döndü).
    rate_bps: Option<u64>,
    /// [TEMKİNLİ-BAŞLANGIÇ · YENİDEN-YOKLAMA] Gönderim tavanı (B/s, tel; ≤ profil hattı).
    pace_bps: u64,
    /// [YENİDEN-YOKLAMA] Sonraki yoklama adımı (binde).
    step_permille: u64,
    /// Kabul edilen örnek sayısı (gözlem).
    samples: u64,
    /// [YENİDEN-YOKLAMA] Doğrulama (tavan yükseltmesi) sayısı (gözlem).
    raises: u64,
    /// [BAŞLANGIÇ-BÜTÇESİ] İlk ACK'e dek bütçe; `None` = bütçe kalktı.
    start: Option<StartBudget>,
    /// [İLK-PENCERE] İlk ACK'in varış penceresinin sonucu (gözlem).
    first_window: Option<FirstWindowOutcome>,
    /// [İLK-PENCERE] Pencerenin ayrıntısı (gözlem): ilk ve son seq, gönderim süresi `Δs`, varış
    /// süresi `Δτ` (ns), aradaki tel baytı.
    first_window_detail: Option<FirstWindowDetail>,
    /// [ÖLÇÜM-ARALIĞI] Oturumda en az bir ölçüm aralığı geldi (aralık kipi).
    interval_mode: bool,
    /// [ÖLÇÜM-ARALIĞI] Aralık kipinde üst üste ölçümsüz ACK sayısı.
    unmeasured_acks: u32,
    /// [ÖLÇÜM-BOŞLUĞU] Boşluk kuralının kurulma sayısı (gözlem).
    gap_arms: u64,
    /// [ÖLÇÜM-BOŞLUĞU] Öncelikli trafik `R/2`'yi doldurduğu için kurulmayan boşluk bütçesi (gözlem).
    gap_skips: u64,
    /// [İKİNCİ-ÖLÇÜM] İlk ACK'e dek ilk varış penceresini açabilecek büyük bir datagram gönderildi.
    bulk_before_first_ack: bool,
    /// [KUYRUK-BOŞALTMA · İŞ-35] Son [`POINT_CAP`] noktanın cephe kuyruk beklemesi (`τ − s − taban`, ns).
    queue_peaks: VecDeque<u64>,
    /// [TAŞMA] Rastgele kayıp tabanı: kuyruk kanıtı olmayan aralıklarda alınan ve gönderilen tel
    /// baytı toplamı (sınırlı; tavanda yarılanır).
    floor_rx: u64,
    floor_sent: u64,
    /// [TAŞMA] Son artış örneğinden bu yana kalan ölçülü ACK hakkı (kuyruk kanıtı penceresi).
    growth_recent: u32,
    /// [TAŞMA] Taşma örneği sayısı (gözlem).
    overflow_samples: u64,
    /// [ÖLÇÜM-BOŞLUĞU] Kurulurken hat modeline yazılacak tek seferlik boşaltma (bayt, tel;
    /// [`Self::take_gap_drain`]).
    gap_drain: u64,
    /// [ÖLÇÜM-ARALIĞI] Son iki ACK'in cephesi: `sent` kayıtları eskisine dek tutulur (bir sonraki
    /// ACK'in M'si iki ACK geriye uzanır).
    recent_fronts: VecDeque<u64>,
    /// [ÖLÇÜM-ARALIĞI] Gözlem: (ölçülen aralık, aralıktan örnek, son kuyruk artışı g ns).
    interval_diag: (u64, u64, Option<i64>),
    /// [SÜRELİ-YOKLAMA] Süren ya da hüküm bekleyen yoklama.
    probe: Option<Probe>,
    /// [SÜRELİ-YOKLAMA] Başarısız yoklama sayısı (gözlem).
    probe_failures: u64,
    /// [İKİNCİ-ÖLÇÜM] Son işlenen ACK geçici evreyi kapattı (`settle_start_budget` bütçeyi kaldırır).
    start_confirmed: bool,
    /// [İKİNCİ-ÖLÇÜM] Geçici evreyi kapatan ölçüm (gözlem).
    start_exit: Option<StartExit>,
}

/// [İKİNCİ-ÖLÇÜM] Başlangıç bütçesini kaldıran kanıt (gözlem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartExit {
    /// İlk ACK: pencere hattı doğruladı ya da alıcı ölçmüyor (eski alıcı; bugünkü yol).
    FirstAck,
    /// Sonraki bir ACK'in ölçümü (aralık ya da cephe) kuyruk artışıyla hattı ölçtü (B/s, tel).
    Sample(u64),
    /// Sonraki bir ACK'in ölçüm aralığı bütçenin hızında gönderilmiş ve kuyruk büyümemiş: hat en
    /// az bütçenin hızında; tavan o hıza iner (B/s, tel).
    Flat(u64),
}

/// [İKİNCİ-ÖLÇÜM] Ölçüm aralığının sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntervalResult {
    /// Kuyruk artışı eşiği aştı: hız (B/s, tel) ve artış `g` (ns).
    Sample(u64, i128),
    /// Kuyruk büyümedi: kanıt hızı ([`LineGuard::flat_tail_rate`], B/s, tel) ve artış `g` (ns).
    Flat(u64, i128),
}

impl IntervalResult {
    fn growth_ns(self) -> i128 {
        match self {
            Self::Sample(_, g) | Self::Flat(_, g) => g,
        }
    }
}

/// [İLK-PENCERE] Pencerenin ayrıntısı (gözlem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstWindowDetail {
    pub first_seq: u64,
    pub last_seq: u64,
    pub ds_ns: u64,
    pub dtau_ns: u64,
    pub wire_bytes: u64,
}

/// [BAŞLANGIÇ-BÜTÇESİ · ÖLÇÜM-BOŞLUĞU] Kuyruk sınırlı bütçe (modül belgesi): ilk ACK'e dek
/// (`gap = false`) ya da aralık kipinde ölçüm boşluğunda (`gap = true`).
#[derive(Debug, Clone, Copy)]
struct StartBudget {
    /// Bütçenin dayandığı hız `R` (B/s, tel): ilk ACK'e dek profil hattı, boşlukta kurulduğu andaki
    /// gönderim tavanı. Dolum `R/2`.
    ref_bps: u64,
    /// [ÖLÇÜM-BOŞLUĞU] Boşluk bütçesi: ilk patlama yok (derinlik bir tam datagram), ilk ölçülü
    /// ACK'te kalkar.
    gap: bool,
    /// İlk iletim anı (ns); `None` = henüz iletim yok (kova dolu).
    first_ns: Option<u64>,
    /// Kovadaki tel baytı (öncelikli iletimler borçlandırabilir: negatif olabilir).
    tokens: i128,
    /// Son dolum anı (ns).
    filled_ns: u64,
    /// Bütçe en az bir kez soruldu: gönderici şerit ayrımıyla uyguladı.
    used: bool,
    /// [İKİNCİ-ÖLÇÜM · İŞ-35] Oturumun ilk ACK'i işlendi ve bütçe kalmadı: geçici evre (ilk ACK'in
    /// tek ölçümü ikinci bağımsız ölçümü bekliyor; modül belgesi).
    provisional: bool,
}

impl StartBudget {
    fn new(ref_bps: u64, gap: bool) -> Self {
        Self {
            ref_bps,
            gap,
            first_ns: None,
            tokens: 0,
            filled_ns: 0,
            used: false,
            provisional: false,
        }
    }
}

/// [SÜRELİ-YOKLAMA] Süren ya da hüküm bekleyen yoklama.
#[derive(Debug, Clone, Copy)]
struct Probe {
    /// Yoklama öncesi tavan (başarısızlıkta dönülen).
    base_pace: u64,
    /// Yoklama tavanı (başarıda kalıcı olan).
    probe_pace: u64,
    /// Yoklamanın bittiği an (ns): o andan sonra tavan hükme dek `base_pace`.
    until_ns: u64,
    /// Hat değişmediyse yoklamanın kuracağı kuyruk (ns).
    expect_ns: u64,
    /// Yoklamanın başladığı ACK noktası.
    reference: Point,
    /// Yoklamanın başladığı an (ns).
    start_ns: u64,
    /// Yoklama süresince gönderilen tel baytı (kanıt; `on_sent` biriktirir).
    sent: u64,
    /// [İKİNCİ-ÖLÇÜM] Başlangıç yoklaması: hüküm ölçüm aralığıyla (varsa).
    by_interval: bool,
}

/// [İLK-PENCERE] İlk ACK'in varış penceresinden çıkan sonuç (gözlem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstWindowOutcome {
    /// Kuyruk büyüdü: tavan ölçülen hatta indi (B/s, tel).
    Sample(u64),
    /// Kuyruk büyümedi, pencere tavana yakın gönderildi: hat en az tavan hızında.
    Verified,
    /// Pencere kuruldu ama karar vermedi (kuyruk büyümedi, gönderim tavanın altında).
    NoInfo,
}

impl LineGuard {
    /// Profilin ileri (`line`) ve dönüş (`return_line`) hatlarıyla boş tahminci; ilk gönderim
    /// tavanı `start_pace_bps` (B/s, tel; profil hattıyla sınırlı). Ürün yolu
    /// [`Self::conservative_start`] kullanır.
    #[must_use]
    pub fn new(line: LineModel, return_line: LineModel, start_pace_bps: u64) -> Self {
        Self {
            line,
            return_line,
            cum: 0,
            sent: VecDeque::new(),
            points: VecDeque::new(),
            frontier: None,
            lags: VecDeque::new(),
            pending_wire: None,
            rate_bps: None,
            pace_bps: start_pace_bps.clamp(1, line.rate_bps.max(1)),
            step_permille: STEP_START_PERMILLE,
            samples: 0,
            raises: 0,
            start: Some(StartBudget::new(line.rate_bps, false)),
            first_window: None,
            first_window_detail: None,
            interval_mode: false,
            unmeasured_acks: 0,
            gap_arms: 0,
            gap_skips: 0,
            bulk_before_first_ack: false,
            gap_drain: 0,
            queue_peaks: VecDeque::new(),
            floor_rx: 0,
            floor_sent: 0,
            growth_recent: 0,
            overflow_samples: 0,
            recent_fronts: VecDeque::new(),
            interval_diag: (0, 0, None),
            probe: None,
            probe_failures: 0,
            start_confirmed: false,
            start_exit: None,
        }
    }

    /// [TEMKİNLİ-BAŞLANGIÇ] Ürün yolunun ilk tavanı: profil hattı × [`START_NUM`] / [`START_DEN`].
    #[must_use]
    pub fn conservative_start(line: LineModel) -> u64 {
        line.rate_bps * START_NUM / START_DEN
    }

    /// Motorun bir iletimi hatta verildi (`udp_bytes` = UDP gövdesi; zarf burada eklenir).
    pub fn on_sent(&mut self, seq: u64, now_ns: u64, udp_bytes: u64) {
        self.on_sent_ahead(seq, now_ns, udp_bytes, 0);
    }

    /// [`Self::on_sent`] ve iletim anındaki hat modeli kuyruğu (`ahead_ns`: bu iletim dahil kendi
    /// baytlarımızın modelde boşalmasına kalan süre). Yoklama hükmü cephe gecikmesinden bunu düşer:
    /// hüküm yalnız modelin bilmediği (gerçek hattın yavaşlığından gelen) kuyruğu görür.
    pub fn on_sent_ahead(&mut self, seq: u64, now_ns: u64, udp_bytes: u64, ahead_ns: u64) {
        self.on_sent_classed(seq, now_ns, udp_bytes, ahead_ns, false);
    }

    /// [ÖLÇÜM-BOŞLUĞU] [`Self::on_sent_ahead`] ve iletimin öncelikli şeride (PTT, metin, denetim)
    /// ait olup olmadığı. Bütün iletimler kovadan düşülür (bütün trafik kuyruğu doldurur); öncelikli
    /// işaret boşluk bütçesinin kurulma koşulunda okunur ([`GAP_PRIORITY_WINDOW_NS`]).
    pub fn on_sent_classed(
        &mut self,
        seq: u64,
        now_ns: u64,
        udp_bytes: u64,
        ahead_ns: u64,
        priority: bool,
    ) {
        let wire = udp_bytes.saturating_add(self.line.datagram_overhead);
        self.cum = self.cum.saturating_add(wire);
        if let Some(p) = self.probe.as_mut() {
            if now_ns >= p.start_ns && now_ns < p.until_ns {
                p.sent = p.sent.saturating_add(wire);
            }
        }
        if self
            .start
            .is_some_and(|b| !b.provisional && !b.gap && self.first_window.is_none())
            && udp_bytes >= crate::arrival_rate::FIRST_WINDOW_BYTES
        {
            self.bulk_before_first_ack = true;
        }
        if let Some(mut b) = self.start {
            if b.first_ns.is_none() {
                b.first_ns = Some(now_ns);
                b.tokens = Self::start_depth(&b, self.line.datagram_overhead);
                b.filled_ns = now_ns;
            }
            b.tokens = self.start_tokens_at(&b, now_ns) - i128::from(wire);
            b.filled_ns = b.filled_ns.max(now_ns);
            self.start = Some(b);
        }
        if self.sent.len() >= SENT_CAP {
            self.sent.pop_front();
        }
        self.sent.push_back(SentRec {
            seq,
            send_ns: now_ns,
            udp: udp_bytes,
            cum: self.cum,
            pace: self.pace_bps,
            ahead_ns,
            pri: priority,
        });
    }

    /// Motorun kararı olmadan bu uçtan aynı hatta çıkan datagram (ör. PARAM): hattı kullanır,
    /// sonraki iletimlerin kümülatif baytına girer.
    pub fn on_unsequenced(&mut self, udp_bytes: u64) {
        let wire = udp_bytes.saturating_add(self.line.datagram_overhead);
        self.cum = self.cum.saturating_add(wire);
        if let Some(b) = self.start.as_mut() {
            if b.first_ns.is_some() {
                b.tokens -= i128::from(wire);
            }
        }
    }

    /// Bir sonraki ACK çerçevesini taşıyan datagramın tel boyu (UDP gövdesi).
    pub fn note_ack_wire(&mut self, udp_bytes: u64) {
        self.pending_wire = Some(udp_bytes);
    }

    /// Korunan hat hızı (B/s, tel); `None` = koruma devrede değil.
    #[must_use]
    pub fn rate_bps(&self) -> Option<u64> {
        self.rate_bps
    }

    /// Kabul edilen örnek sayısı (gözlem).
    #[must_use]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// [TEMKİNLİ-BAŞLANGIÇ · YENİDEN-YOKLAMA] Gönderim tavanı (B/s, tel; ≤ profil hattı).
    #[must_use]
    pub fn pace_bps(&self) -> u64 {
        self.pace_bps
    }

    /// [YENİDEN-YOKLAMA] Doğrulama sayısı (gözlem).
    #[must_use]
    pub fn raises(&self) -> u64 {
        self.raises
    }

    /// [SÜRELİ-YOKLAMA] Başarısız yoklama sayısı (gözlem).
    #[must_use]
    pub fn probe_failures(&self) -> u64 {
        self.probe_failures
    }

    /// [SÜRELİ-YOKLAMA] Yoklamanın süresi dolduysa tavan hükme dek yoklama öncesi değerine döner.
    /// Tavan değiştiyse `true` (gönderici hat modelini yeni tavana alır).
    pub fn tick(&mut self, now_ns: u64) -> bool {
        match self.probe {
            Some(p) if now_ns >= p.until_ns && self.pace_bps == p.probe_pace => {
                self.pace_bps = p.base_pace;
                true
            }
            _ => false,
        }
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Bütçe hâlâ geçerli mi (ilk ACK işlenmedi).
    #[must_use]
    pub fn start_budget_active(&self) -> bool {
        self.start.is_some()
    }

    /// [ÖLÇÜM-BOŞLUĞU] Boşluk bütçesi kurulu mu (modül belgesi, "Ölçüm boşluğu").
    #[must_use]
    pub fn gap_budget_active(&self) -> bool {
        self.start.is_some_and(|b| b.gap)
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] `udp_bytes` gövdeli bir iletim şimdi bütçeye sığar mı (modül belgesi).
    /// Bütçe kalktıysa daima `true`. Sorulması bütçeyi "uygulandı" işaretler.
    pub fn start_budget_admits(&mut self, now_ns: u64, udp_bytes: u64) -> bool {
        let Some(mut b) = self.start else {
            return true;
        };
        b.used = true;
        self.start = Some(b);
        let tokens = if b.first_ns.is_some() {
            self.start_tokens_at(&b, now_ns)
        } else {
            Self::start_depth(&b, self.line.datagram_overhead)
        };
        tokens >= i128::from(udp_bytes.saturating_add(self.line.datagram_overhead))
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Kova derinliği (tel baytı): `R × START_BURST_NS`, en az bir tam datagram
    /// (dar profilde ilk iletim de sığar; saat ilk iletimle başlar). [ÖLÇÜM-BOŞLUĞU] Boşluk
    /// bütçesinde yalnız bir tam datagram (ilk patlama yok).
    fn start_depth(b: &StartBudget, overhead: u64) -> i128 {
        let mtu = i128::from(crate::rate::DEFAULT_MTU.saturating_add(overhead));
        if b.gap {
            return mtu;
        }
        let p = i128::from(b.ref_bps);
        let burst = p * i128::from(START_BURST_NS) / 1_000_000_000;
        burst.max(mtu)
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] `now_ns` anındaki kova: P/2 hızında dolum, derinlikte tavan. Dolum ilk
    /// iletimden `2 × derinlik / P` sonra başlar: meşgul gönderimde toplam sınır
    /// `max(derinlik, P/2 × t)` olur (patlamanın kuyruğu gerçek hat beyanın yarısıyken o ana dek
    /// boşalır); boşta geçen süre kovayı derinliğin üstüne dolduramaz (boşta birikme patlama olmaz).
    fn start_tokens_at(&self, b: &StartBudget, now_ns: u64) -> i128 {
        let depth = Self::start_depth(b, self.line.datagram_overhead);
        let Some(first) = b.first_ns else {
            return depth;
        };
        let p = i128::from(b.ref_bps).max(1);
        let cross_ns = u64::try_from(depth * 2_000_000_000 / p).unwrap_or(u64::MAX);
        let from = b.filled_ns.max(first.saturating_add(cross_ns));
        let refill = if now_ns > from {
            p * i128::from(now_ns - from) / 2_000_000_000
        } else {
            0
        };
        (b.tokens + refill).min(depth)
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Bütçeyi kaldırır (ilk ACK işlendi). Bütçe uygulandıysa ilk ACK'in noktası
    /// aralık başlangıcı olmaktan çıkar: cephesi bütçenin yarı hız evresinde gönderilmişti ve
    /// darboğaz o evrede boşta kalabilir; böyle bir aralığın `bayt / Δτ` tahmini hattı düşük okur.
    /// Ölçüldü: kalıcı sim, gerçek 6 250 B/s, tohum 3, eski alıcı: 5 779 B/s; ürün testi, dönüş
    /// gecikmesi değişken hücre (dürüst hat), tohum 3: ilk ACK'in penceresi doğrulama verdikten sonra
    /// bütçe evresini kapsayan aralık 6 185 B/s'lik yanlış örnek verdi. İlk pencerenin sonucu
    /// (örnek ya da doğrulama) tavanda kalır; taban için gecikme örneği kalır. Sonraki iki nokta
    /// tahmini bütçesiz gönderilmiş aralıktan gelir.
    pub fn end_start_budget(&mut self) {
        // [ÖLÇÜM-BOŞLUĞU] Boşluk bütçesi burada kalkmaz (ilk ölçülü ACK'te kalkar).
        if self.start.is_some_and(|b| b.gap) {
            return;
        }
        if self.start.take().is_some_and(|b| b.used) {
            self.points.clear();
        }
    }

    /// [ÖLÇÜM-BOŞLUĞU] Boşluk kuralının kurulma sayısı (gözlem).
    #[must_use]
    pub fn gap_arms(&self) -> u64 {
        self.gap_arms
    }

    /// [ÖLÇÜM-BOŞLUĞU] Öncelikli trafik `R/2`'yi doldurduğu için kurulmayan boşluk bütçesi sayısı.
    #[must_use]
    pub fn gap_skips(&self) -> u64 {
        self.gap_skips
    }

    /// [ÖLÇÜM-BOŞLUĞU] Boşluk kuralı kurulurken hat modeline yazılacak tek seferlik boşaltma
    /// (bayt, tel; toplam kuyruk olarak — `TokenBucket::line_raise_backlog`). Okununca sıfırlanır.
    pub fn take_gap_drain(&mut self) -> u64 {
        std::mem::take(&mut self.gap_drain)
    }

    /// [İKİNCİ-ÖLÇÜM · İŞ-35] Bir ACK işlendikten sonra (`on_ack_measured`'dan sonra) bütçenin
    /// kalkıp kalkmayacağı (modül belgesi, "İkinci ölçüm"):
    /// * oturumun ilk ACK'inde: pencere hattı doğruladıysa ya da alıcı ölçmüyorsa (ne pencere ne
    ///   ölçüm aralığı; eski alıcı) bütçe kalkar; değilse geçici evre başlar;
    /// * geçici evrede: sonraki bir ACK'in ölçümü (örnek ya da bütçe hızında düz aralık) bütçeyi
    ///   kaldırır.
    pub fn settle_start_budget(&mut self) {
        // [ÖLÇÜM-BOŞLUĞU] Boşluk bütçesi ilk ölçülü ACK'te kalkar, geçici evreye girmez.
        let Some(mut b) = self.start.filter(|b| !b.gap) else {
            return;
        };
        if !b.provisional {
            if self.first_ack_trusted() {
                self.start_exit = Some(StartExit::FirstAck);
                self.end_start_budget();
            } else {
                b.provisional = true;
                self.start = Some(b);
            }
        } else if std::mem::take(&mut self.start_confirmed) {
            self.end_start_budget();
        }
    }

    /// [İKİNCİ-ÖLÇÜM] Bütçeyi kaldıran kanıt (gözlem; `None` = bütçe sürüyor ya da hiç ACK yok).
    #[must_use]
    pub fn start_exit(&self) -> Option<StartExit> {
        self.start_exit
    }

    /// [İKİNCİ-ÖLÇÜM] Bütçenin garanti ettiği hız (B/s, tel): `P/2`. İkinci ölçümden önce tek bir
    /// ölçüm tavanı bunun altına indiremez.
    fn start_floor_bps(&self) -> u64 {
        (self.line.rate_bps / 2).max(1)
    }

    /// [İKİNCİ-ÖLÇÜM] İlk ACK'in sonucu bütçeyi kaldırır mı: pencere hattı doğruladı, alıcı
    /// ölçmüyor (ne pencere ne ölçüm aralığı: eski alıcı, bugünkü yol) ya da ilk ACK'e dek hiç
    /// pencere açacak büyük datagram gönderilmedi (uygulama sınırlı başlangıç, ör. önce yalnız PTT):
    /// ölçüm denenmedi, güvenilmeyecek tek bir ölçüm yok. Bu sonuncusunda hat bugünkü yoldaki gibi
    /// ölçülmemiştir; sonraki ACK'lerin ölçüm aralığı ölçer. Ölçüldü (`tests/starved_line_sim.rs`,
    /// seyrek ACK, dosya PTT'den sonra): geçici evrede dosya 22,30 s, bu kuralla 12,19 s.
    fn first_ack_trusted(&self) -> bool {
        self.first_window == Some(FirstWindowOutcome::Verified)
            || (self.first_window.is_none() && (!self.interval_mode || !self.bulk_before_first_ack))
    }

    /// [İKİNCİ-ÖLÇÜM] İlk ACK işleniyor (bütçe var, geçici evre başlamadı).
    fn on_first_ack(&self) -> bool {
        self.start.is_some_and(|b| !b.provisional && !b.gap)
    }

    /// [İKİNCİ-ÖLÇÜM] Geçici evrede (ilk ACK işlendi, bütçe sürüyor).
    fn in_provisional(&self) -> bool {
        self.start.is_some_and(|b| b.provisional && !b.gap)
    }

    /// [İLK-PENCERE] İlk ACK'in varış penceresinin sonucu (gözlem; `None` = pencere işlenmedi).
    #[must_use]
    pub fn first_window(&self) -> Option<FirstWindowOutcome> {
        self.first_window
    }

    /// [İLK-PENCERE] Pencerenin ayrıntısı (gözlem).
    #[must_use]
    pub fn first_window_detail(&self) -> Option<FirstWindowDetail> {
        self.first_window_detail
    }

    /// [İLK-PENCERE] Oturumun ilk ACK'i işlenirken: `acked` çerçevenin teyit ettiği seq'ler,
    /// `arrival_rate` çerçevenin §2.2 alanı (alıcının ilk varış penceresinin gövde hızı, B/s).
    /// Pencere göndericinin kaydından kurulur ([`crate::arrival_rate::window_end`]); kuyruk artışı
    /// pencerenin %5'ini aşarsa tavan ölçülen hatta iner. Bütçe kalktıysa, alan sentinel ise ya da
    /// pencere kurulamazsa etkisiz (`None`). Yalnız bir kez çalışır.
    pub fn on_first_window(
        &mut self,
        acked: &[u64],
        arrival_rate: u16,
    ) -> Option<FirstWindowOutcome> {
        if !self.on_first_ack()
            || self.first_window.is_some()
            || arrival_rate == crate::ack::ARRIVAL_RATE_SENTINEL
            || arrival_rate == 0
        {
            return None;
        }
        let rec_of = |seq: u64| {
            self.sent
                .binary_search_by_key(&seq, |r| r.seq)
                .ok()
                .map(|i| self.sent[i])
        };
        let mut seqs: Vec<u64> = acked.to_vec();
        seqs.sort_unstable();
        seqs.dedup();
        // Alıcının saydığı her seq göndericinin kaydında olmalı; olmayan pencereyi bozar.
        let mut sized = Vec::with_capacity(seqs.len());
        for s in seqs {
            sized.push((s, rec_of(s)?.udp));
        }
        let (first, last, rx_bytes) = crate::arrival_rate::window_end(sized)?;
        let (rf, rl) = (rec_of(first)?, rec_of(last)?);
        let ds = rl.send_ns.checked_sub(rf.send_ns).filter(|&d| d > 0)?;
        let dtau =
            u64::try_from(u128::from(rx_bytes) * 1_000_000_000 / u128::from(arrival_rate)).ok()?;
        let wire = rl.cum.saturating_sub(rf.cum);
        self.first_window_detail = Some(FirstWindowDetail {
            first_seq: first,
            last_seq: last,
            ds_ns: ds,
            dtau_ns: dtau,
            wire_bytes: wire,
        });
        // Darboğaz pencere boyunca doluysa (son datagram kuyrukta bekledi) hat = tel baytı / Δτ.
        // Son datagramın beklemesi `Δτ − Δs − (ser(son) − ser(ilk))` (ilk datagram oturumun boş
        // hattında gitti); serileşme tahminin kendisiyle: iki datagramın boy farkı kuyruk artışı
        // gibi görünmesin.
        let est = u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(dtau.max(1)))
            .unwrap_or(u64::MAX)
            .max(1);
        let ser = |udp: u64| {
            i128::from(udp.saturating_add(self.line.datagram_overhead)) * 1_000_000_000
                / i128::from(est)
        };
        let wait = i128::from(dtau) - i128::from(ds) - (ser(rl.udp) - ser(rf.udp));
        let tol = i128::from((ds / GROWTH_MIN_DIV).max(FIRST_WINDOW_MIN_WAIT_NS));
        let busy = wait > tol;
        // [İKİNCİ-ÖLÇÜM] FIFO'da yayılım sabitken varış süresi gönderim süresinden kısa olamaz
        // (`wait ≥ 0`). Çözünürlüğün ötesinde kısa pencere bir damga hatasıdır (ör. açıcının
        // damgası geç): gerçek bekleme bilinmez, doğrulama yok.
        let compressed = wait < -tol;
        let sent_bps =
            u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(ds)).unwrap_or(u64::MAX);
        let at_pace =
            sent_bps.saturating_mul(1000) >= self.pace_bps.saturating_mul(VERIFY_MIN_PERMILLE);
        // Örnek yalnız tavana yakın gönderilmiş, darboğazı doldurmuş ve tavanın en az %5 altını
        // gösteren pencereden (iki nokta tahmincisinin kabul eşiği); gönderici uygulama sınırlıyken
        // (ör. önce küçük metin, sonra parça) bilgi yok.
        let outcome = if !at_pace || compressed {
            FirstWindowOutcome::NoInfo
        } else if busy
            && est.saturating_mul(GROWTH_MIN_DIV) < self.pace_bps.saturating_mul(GROWTH_MIN_DIV - 1)
        {
            let cap = self.pace_bps.min(self.line.rate_bps);
            let est = est.clamp(1, cap);
            // [İKİNCİ-ÖLÇÜM] Tek örnek tavanı bütçenin garanti ettiği hızın (P/2) altına indirmez:
            // yanlış düşük örnek öncelikli şeridi aç bırakmasın. Bütçe ikinci ölçüme dek sürer.
            let held = est.max(self.start_floor_bps().min(cap));
            self.rate_bps = Some(held);
            self.pace_bps = held;
            self.samples = self.samples.saturating_add(1);
            self.step_permille = STEP_MIN_PERMILLE;
            FirstWindowOutcome::Sample(est)
        } else {
            FirstWindowOutcome::Verified
        };
        self.first_window = Some(outcome);
        Some(outcome)
    }

    /// Bir ACK çerçevesi işlendi: `largest_seq` çerçevenin en büyük teyitli iletimi, `ack_delay_ns`
    /// çerçevenin bildirdiği tutma süresi, `saturated` tutma süresi kodlama tavanında mı.
    /// Koruma devredeyse güncel hız ve kuyruk tahmini döner.
    pub fn on_ack(
        &mut self,
        now_ns: u64,
        ack_delay_ns: u64,
        saturated: bool,
        largest_seq: u64,
    ) -> Option<LineGuardUpdate> {
        self.on_ack_measured(now_ns, ack_delay_ns, saturated, largest_seq, None)
    }

    /// [ÖLÇÜM-ARALIĞI] Aralığın aşağı yönlü örneği: kuyruk artışı
    /// `g = rx_span − send_span − (ser(last) − ser(first))` artış eşiğini aşarsa hız
    /// `(first, last] aralığında gönderilen tel baytı / rx_span`. Kayıtta olmayan seq, sıfır gönderim
    /// süresi ya da artış yoksa örnek yok.
    fn interval_sample(&mut self, iv: ForwardInterval) -> Option<IntervalResult> {
        let rec_of = |seq: u64| {
            self.sent
                .binary_search_by_key(&seq, |r| r.seq)
                .ok()
                .map(|i| self.sent[i])
        };
        let (Some(first), Some(last)) = (rec_of(iv.first_seq), rec_of(iv.last_seq)) else {
            return None;
        };
        let send_span = last.send_ns.checked_sub(first.send_ns).filter(|&d| d > 0)?;
        self.interval_diag.0 = self.interval_diag.0.saturating_add(1);
        // Serileşme farkı güncel hat tahminiyle (yoksa profil hattı): iki ucun boy farkı kuyruk
        // artışı sayılmasın. Gerçek hat tahminden yavaşsa fark küçük çıkar, `g` büyür (kuyruğu
        // fark etme yönünde).
        let rate = self.rate_bps.unwrap_or(self.line.rate_bps).max(1);
        let ser = |udp: u64| {
            i128::from(udp.saturating_add(self.line.datagram_overhead)) * 1_000_000_000
                / i128::from(rate)
        };
        let g =
            i128::from(iv.rx_span_ns) - i128::from(send_span) - (ser(last.udp) - ser(first.udp));
        self.interval_diag.2 = Some(i64::try_from(g).unwrap_or(i64::MAX));
        let mtu_ns = ser(crate::rate::DEFAULT_MTU);
        let threshold = mtu_ns.max(i128::from(send_span / GROWTH_MIN_DIV));
        let wire = last.cum.saturating_sub(first.cum);
        if g > threshold {
            // [TAŞMA] Artış örneği kuyruk kanıtıdır: sonraki aralıklarda taşma örneği alınabilir.
            // Artış görünen aralıkta taşma tahmini kullanılmaz: tek aralıktaki rastgele kayıp
            // gürültüsünü taşır (≈ ±%12) ve artış sürerken sonraki aralıklar zaten iner.
            self.growth_recent = OVERFLOW_WINDOW_ACKS;
            let est =
                u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(iv.rx_span_ns.max(1)))
                    .unwrap_or(u64::MAX);
            self.interval_diag.1 = self.interval_diag.1.saturating_add(1);
            return Some(IntervalResult::Sample(est, g));
        }
        // [TAŞMA] Kuyruk büyümüyor: teslim kesri rastgele kayıp tabanına göre aşırı mı (modül
        // belgesi, "Taşma"). Taban bu ölçümden önceki değerdir.
        let overflow = self.overflow_estimate(iv, wire, send_span, g, threshold);
        self.growth_recent = self.growth_recent.saturating_sub(1);
        if let Some(est) = overflow {
            self.interval_diag.1 = self.interval_diag.1.saturating_add(1);
            return Some(IntervalResult::Sample(est, g));
        }
        // [İKİNCİ-ÖLÇÜM] Kuyruğu büyümeyen, taşma da göstermeyen aralık: düz aralık kanıtı.
        Some(IntervalResult::Flat(self.flat_tail_rate(first, last), g))
    }

    /// [TAŞMA · İŞ-35] Aralığın teslim kesri `f = alınan tel baytı / gönderilen tel baytı`
    /// (`(first, last]`). Taban (rastgele kayıp) kuyruk kanıtı olmayan aralıklardan birikir.
    /// Yalnız artış örneği vermeyen aralıkta çağrılır. Taşma örneği yalnız (1) `f` tabanın
    /// [`OVERFLOW_EXCESS_NUM`]/[`OVERFLOW_EXCESS_DEN`]'inin altında, (2) kuyruk kanıtı — son
    /// [`OVERFLOW_WINDOW_ACKS`] ölçülü ACK'te artış örneği —, (3) kuyruk boşalmıyor
    /// (`g ≥ −eşik`) ve (4) gönderici tavanın en az %90'ıyla gönderdiyse: hız = (alınan bayt / taban kesri) / `rx_span` (hattı geçen baytın tahmini).
    /// Düşük teslim tek başına kanıt değildir (rastgele kayıp): (2) olmadan örnek yok.
    fn overflow_estimate(
        &mut self,
        iv: ForwardInterval,
        wire: u64,
        send_span: u64,
        g: i128,
        threshold: i128,
    ) -> Option<u64> {
        let rx = iv.rx_wire?;
        let mtu_wire = crate::rate::DEFAULT_MTU + self.line.datagram_overhead;
        if wire < OVERFLOW_MIN_MTUS * mtu_wire || rx > wire {
            return None;
        }
        let floor_known =
            self.floor_sent >= OVERFLOW_FLOOR_MIN_MTUS * mtu_wire && self.floor_rx > 0;
        // f < taban × NUM/DEN  ⇔  rx × floor_sent × DEN < floor_rx × wire × NUM
        let excess = floor_known
            && u128::from(rx) * u128::from(self.floor_sent) * u128::from(OVERFLOW_EXCESS_DEN)
                < u128::from(self.floor_rx) * u128::from(wire) * u128::from(OVERFLOW_EXCESS_NUM);
        // Taban: kuyruk kanıtı olmayan, kuyruğu boşalmayan aralık (rastgele kayıp değişirse taban
        // izler; kanıtlı taşma evresi tabana girmez).
        let queue_evidence = self.growth_recent > 0;
        if !queue_evidence && g.abs() <= threshold {
            self.floor_rx = self.floor_rx.saturating_add(rx);
            self.floor_sent = self.floor_sent.saturating_add(wire);
            if self.floor_sent > OVERFLOW_FLOOR_CAP_MTUS * mtu_wire {
                self.floor_rx /= 2;
                self.floor_sent /= 2;
            }
        }
        let sent_bps =
            u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(send_span.max(1)))
                .unwrap_or(u64::MAX);
        let at_pace =
            sent_bps.saturating_mul(1000) >= self.pace_bps.saturating_mul(VERIFY_MIN_PERMILLE);
        if !(excess && queue_evidence && g >= -threshold && at_pace) {
            return None;
        }
        let passed = u128::from(rx) * u128::from(self.floor_sent) / u128::from(self.floor_rx);
        let est = u64::try_from(passed * 1_000_000_000 / u128::from(iv.rx_span_ns.max(1)))
            .unwrap_or(u64::MAX);
        self.overflow_samples = self.overflow_samples.saturating_add(1);
        Some(est)
    }

    /// [İKİNCİ-ÖLÇÜM] Kuyruğu büyümeyen bir aralığın kanıt hızı (B/s, tel; bir tam datagramlık
    /// tanecik payıyla): bütçe sürerken aralığın bütçe dolumunun başladığı andan sonraki kısmı
    /// (ilk patlamadan sonraki bekleme kanıt değildir: hat o sırada boşalır), en az
    /// [`START_BURST_NS`] sürmeli; değilse 0.
    fn flat_tail_rate(&self, first: SentRec, last: SentRec) -> u64 {
        let cross = self.start.map_or(0, |b| {
            b.first_ns.map_or(0, |t0| {
                let p = i128::from(b.ref_bps).max(1);
                let depth = Self::start_depth(&b, self.line.datagram_overhead);
                t0.saturating_add(u64::try_from(depth * 2_000_000_000 / p).unwrap_or(u64::MAX))
            })
        });
        let from = first.send_ns.max(cross);
        let base = self
            .sent
            .iter()
            .take_while(|r| r.seq <= last.seq)
            .filter(|r| r.seq >= first.seq && r.send_ns <= from)
            .last()
            .copied()
            .unwrap_or(first);
        let span = last.send_ns.saturating_sub(base.send_ns);
        if span < START_BURST_NS {
            return 0;
        }
        let mtu_wire = crate::rate::DEFAULT_MTU + self.line.datagram_overhead;
        let bytes = last.cum.saturating_sub(base.cum).saturating_add(mtu_wire);
        u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(span)).unwrap_or(u64::MAX)
    }

    /// [TAŞMA · İŞ-35] Taşma örneği sayısı (gözlem).
    #[must_use]
    pub fn overflow_samples(&self) -> u64 {
        self.overflow_samples
    }

    /// [TAŞMA · İŞ-35] M'nin `(first, last]` içinde alınan seq'lerinin tel baytı (göndericinin
    /// kaydından; zarf dahil). Kayıtta olmayan seq varsa `None`.
    #[must_use]
    pub fn received_wire(&self, first: u64, last: u64, received: &[u64]) -> Option<u64> {
        let mut sum = 0u64;
        for &q in received.iter().filter(|&&q| q > first && q <= last) {
            let i = self.sent.binary_search_by_key(&q, |r| r.seq).ok()?;
            sum = sum.saturating_add(self.sent[i].udp.saturating_add(self.line.datagram_overhead));
        }
        Some(sum)
    }

    /// [ÖLÇÜM-ARALIĞI] Gözlem: (ölçülen aralık sayısı, aralıktan aşağı yönlü örnek sayısı, son
    /// aralığın kuyruk artışı `g`, ns).
    #[must_use]
    pub fn interval_diag(&self) -> (u64, u64, Option<i64>) {
        self.interval_diag
    }

    /// [ÖLÇÜM-ARALIĞI] [`Self::on_ack`] ve çerçevenin ölçüm aralığı (`None` = bu ACK'te ölçüm yok).
    /// Ölçüm varken aşağı yönlü örneği yalnız aralığın kuyruk artışı verir; cephe tahmincisinin
    /// kuralı modül belgesindedir ("Ölçüm aralığı").
    pub fn on_ack_measured(
        &mut self,
        now_ns: u64,
        ack_delay_ns: u64,
        saturated: bool,
        largest_seq: u64,
        interval: Option<ForwardInterval>,
    ) -> Option<LineGuardUpdate> {
        // Aralık örneği cephe zamanlamasından bağımsızdır; kayıtlar budanmadan önce.
        let interval_result = match interval {
            Some(iv) => {
                self.interval_mode = true;
                self.unmeasured_acks = 0;
                // [ÖLÇÜM-BOŞLUĞU] Ölçüm döndü: boşluk bütçesi kalkar.
                if self.start.is_some_and(|b| b.gap) {
                    self.start = None;
                }
                self.interval_sample(iv)
            }
            None => {
                if self.interval_mode {
                    self.unmeasured_acks = self.unmeasured_acks.saturating_add(1);
                }
                None
            }
        };
        let interval_best = match interval_result {
            Some(IntervalResult::Sample(est, _)) => Some(est),
            _ => None,
        };
        // [ÖLÇÜM-BOŞLUĞU] Aralık kipinde cephe hiçbir zaman aşağı yönlü örnek vermez; uzun ölçüm
        // boşluğunda kapasite iddia etmeyen boşluk bütçesi kurulur (modül belgesi).
        let fronts_decide = !self.interval_mode;
        let arm_gap = self.interval_mode
            && interval.is_none()
            && self.unmeasured_acks >= UNMEASURED_GAP_ACKS
            && self.start.is_none();
        // Öncelikli trafik varsayılan hattın (`R/2`) neredeyse tamamını dolduruyorsa bütçe kuyruğu
        // koruyamaz, yalnız dosyayı ve yeniden iletimi aç bırakır (ör. 6 250 ölçülmüşken 5 iletimli
        // PTT 2,85 kB/s → dosyaya 275 B/s): öncelikli olmayan işe kovadan en az `R/8` kalmıyorsa
        // (öncelikli ≥ 3R/8) kurulmaz.
        let arm_gap = arm_gap && {
            let since = now_ns.saturating_sub(GAP_PRIORITY_WINDOW_NS);
            let pri_wire: u64 = self
                .sent
                .iter()
                .filter(|r| r.pri && r.send_ns >= since)
                .map(|r| r.udp.saturating_add(self.line.datagram_overhead))
                .sum();
            // öncelikli < 3R/8: öncelikli olmayan işe kovadan en az R/8 kalır.
            let feasible = u128::from(pri_wire) * 8 * 1_000_000_000
                < u128::from(self.pace_bps) * 3 * u128::from(GAP_PRIORITY_WINDOW_NS);
            if !feasible {
                self.gap_skips = self.gap_skips.saturating_add(1);
            }
            feasible
        };
        if arm_gap {
            self.start = Some(StartBudget::new(self.pace_bps, true));
            self.gap_arms = self.gap_arms.saturating_add(1);
        }
        let wire = self.pending_wire.take()?;
        if saturated || self.frontier.is_some_and(|f| largest_seq <= f) {
            return None;
        }
        let i = self
            .sent
            .binary_search_by_key(&largest_seq, |r| r.seq)
            .ok()?;
        let rec = self.sent[i];
        let ser_ns = ns_for(
            wire.saturating_add(self.return_line.datagram_overhead),
            self.return_line.rate_bps,
        )?;
        let tau = now_ns.checked_sub(ack_delay_ns)?.checked_sub(ser_ns)?;
        let lag = tau.checked_sub(rec.send_ns)?;
        self.frontier = Some(largest_seq);
        if self.lags.len() >= BASE_WINDOW {
            self.lags.pop_front();
        }
        self.lags.push_back(lag);
        let base = self.lags.iter().copied().min().unwrap_or(lag);
        if self.queue_peaks.len() >= POINT_CAP {
            self.queue_peaks.pop_front();
        }
        self.queue_peaks.push_back(lag.saturating_sub(base));
        // [ÖLÇÜM-BOŞLUĞU] Kurulurken tek seferlik boşaltma: cephenin kuyruk beklemesi kadar süre
        // (dönüş gecikmesi artışı da buna girer: bedeli bir kez o kadar bekleme, hız değil).
        if arm_gap {
            self.gap_drain = u64::try_from(
                u128::from(self.pace_bps) * u128::from(lag.saturating_sub(base)) / 1_000_000_000,
            )
            .unwrap_or(u64::MAX);
        }
        // [SÜRELİ-YOKLAMA] Hüküm ACK'i: yoklama süresince gönderilen tel baytı (kanıt). Canlı sayaç
        // ve budanmamış kayıtlar; büyüğü (kayıtlar ara ACK'lerle budanmış olabilir, gönderim de
        // yoklamadan önce kaydedilmiş olabilir).
        let wire_between = |from: u64, to: u64| {
            self.sent
                .iter()
                .filter(|r| r.send_ns >= from && r.send_ns < to)
                .map(|r| r.udp.saturating_add(self.line.datagram_overhead))
                .sum::<u64>()
        };
        let probe_sent = self
            .probe
            .filter(|p| rec.send_ns >= p.until_ns)
            .map(|p| wire_between(p.start_ns, p.until_ns).max(p.sent));
        // Yoklama bitişinden hüküm cephesine dek gönderilen (cephe dahil): gönderici bu arada
        // yoklama öncesi tavanın altında kaldıysa kuyruk boşalmış olabilir ve artış görünmez.
        let probe_after = self
            .probe
            .filter(|p| rec.send_ns >= p.until_ns)
            .map(|p| wire_between(p.until_ns, rec.send_ns.saturating_add(1)));
        // [ÖLÇÜM-ARALIĞI] Kayıtlar son iki ACK'in cephelerinin eskisine dek tutulur: bir sonraki
        // ACK'in ölçüm aralığı (M) iki ACK geriye uzanır. Cephe tahmincisi yalnız bu cepheden
        // sonrasını okur.
        if self.recent_fronts.len() >= 2 {
            self.recent_fronts.pop_front();
        }
        self.recent_fronts.push_back(largest_seq);
        // [İKİNCİ-ÖLÇÜM] Geçici evreye girecek ilk ACK'te kayıtlar budanmaz: bir sonraki ACK'in M'si
        // alıcıda en baştan (taban 0) başlar ve ikinci ölçüm odur ([`SENT_CAP`] yine üst sınır).
        // Gerçek alıcı ilk iki bitmap ACK'inde M'nin tabanını 0 alır (`receiver.rs`
        // `bitmap_base`): iki cephe birikmeden kayıt budanmaz. Geçici evrede (ilk ACK'in tek ölçümü
        // ikinci ölçümü bekliyor) de budanmaz.
        let floor =
            if self.recent_fronts.len() < 2 || (self.on_first_ack() && !self.first_ack_trusted()) {
                0
            } else {
                self.recent_fronts.front().copied().unwrap_or(largest_seq)
            };
        let keep_from = self.sent.partition_point(|r| r.seq < floor).min(i);
        self.sent.drain(..keep_from);

        let current = self.rate_bps.unwrap_or(self.line.rate_bps);
        let mtu_ns = ns_for(
            crate::rate::DEFAULT_MTU + self.line.datagram_overhead,
            current,
        )
        .unwrap_or(0);
        let mut best: Option<u64> = None;
        let mut verified = false;
        let newest = self.points.len().saturating_sub(1);
        for (k, p) in self.points.iter().enumerate() {
            let (Some(ds), Some(dtau)) = (
                rec.send_ns.checked_sub(p.send_ns),
                tau.checked_sub(p.tau_ns),
            ) else {
                continue;
            };
            if ds == 0 {
                continue;
            }
            let growth = dtau.saturating_sub(ds);
            let bytes = rec.cum.saturating_sub(p.cum);
            if growth <= mtu_ns.max(ds / GROWTH_MIN_DIV) {
                // [YENİDEN-YOKLAMA] Yalnız en yeni aralık: tavana yakın gönderildi, kuyruk büyümedi.
                // Aralık bugünkü tavanla gönderilmiş olmalı: ACK gönderimden saniyeler sonra gelir,
                // eski (düşük tavanlı) aralığın doğrulaması yeni bilgi taşımaz.
                let interval_pace = p.pace.min(rec.pace);
                if k == newest
                    && ds >= VERIFY_MIN_SPAN_NS
                    && interval_pace >= self.pace_bps
                    && growth <= (ds / VERIFY_GROWTH_DIV).min(mtu_ns)
                {
                    let sent_bps =
                        u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(ds))
                            .unwrap_or(u64::MAX);
                    verified = sent_bps.saturating_mul(1000)
                        >= self.pace_bps.saturating_mul(VERIFY_MIN_PERMILLE);
                }
                continue;
            }
            let est = u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(dtau))
                .unwrap_or(u64::MAX);
            best = Some(best.map_or(est, |b| b.max(est)));
        }
        // [ÖLÇÜM-ARALIĞI] Aralık kipinde aşağı yönlü örneği aralık verir; cephe örneği yalnız
        // ölçüm yokken ve geri dönüş koşulunda (modül belgesi).
        let best = if fronts_decide { best } else { interval_best };
        if self.points.len() >= POINT_CAP {
            self.points.pop_front();
        }
        let point = Point {
            tau_ns: tau,
            send_ns: rec.send_ns,
            cum: rec.cum,
            pace: rec.pace,
            ahead_ns: rec.ahead_ns,
        };
        self.points.push_back(point);
        // [SÜRELİ-YOKLAMA] Hüküm: cephe yoklama bittikten sonra gönderildi.
        let mut probe_drain_bytes = 0u64;
        // Sonuçsuz hüküm: bu ACK kuyruk yazmaz (iki ölçü ayrıştı; biri dönüş gecikmesi basamağı
        // olabilir).
        let mut inconclusive = false;
        if let Some(p) = self.probe.filter(|p| rec.send_ns >= p.until_ns) {
            self.probe = None;
            // Hüküm iki ölçüyle (göndericinin kendi geçici kuyruğu — hat modelinde cephenin önünde
            // serileşen kendi baytlarımız; 6 250 B/s'de bir parça 172 ms — iki cepheden de düşülür):
            // * mutlak: hüküm cephesinin kuyruğu = gecikme − taban (son noktaların en küçüğü).
            //   Yoklamadan önce duran, sonra boşalan kuyruğa dayanıklı (kalıcı sim, gerçek 6 250,
            //   tohum 1: referansta 0,29 s kuyruk yoklamanın izini maskeliyordu); yakın bir dönüş
            //   gecikmesi basamağını kuyruk sanır.
            // * fark: hüküm cephesi − referans cephesi. Basamağa dayanıklı (iki cephe de
            //   basamaktan sonra), referansta duran kuyrukta izi maskeler.
            // İkisi de yoklamanın beklenen izinin yarısının altında → başarı; ikisi de üstünde →
            // başarısızlık; ayrışırsa sonuç yok (tavan geri döner, kuyruk yazılmaz).
            let corr = |tau: u64, send: u64, ahead: u64| {
                i128::from(tau) - i128::from(send) - i128::from(ahead)
            };
            let new_corr = corr(tau, rec.send_ns, rec.ahead_ns);
            let abs_q = u64::try_from((new_corr - i128::from(base)).max(0)).unwrap_or(u64::MAX);
            let growth = u64::try_from(
                (new_corr
                    - corr(
                        p.reference.tau_ns,
                        p.reference.send_ns,
                        p.reference.ahead_ns,
                    ))
                .max(0),
            )
            .unwrap_or(u64::MAX);
            // Kanıt: yoklama süresince yoklama tavanının en az %90'ı gönderildi (uygulama sınırlı
            // gönderici yoklamayı sınamamıştır; hüküm yok, tavan önceki değerinde).
            // Datagram tanecikliği: süre içine bir tam datagram eksik düşebilir (kanıt payı).
            let mtu_wire = crate::rate::DEFAULT_MTU + self.line.datagram_overhead;
            let sent = probe_sent.unwrap_or(0).saturating_add(mtu_wire);
            let span = p.until_ns.saturating_sub(p.start_ns).max(1);
            let sent_bps = u64::try_from(u128::from(sent) * 1_000_000_000 / u128::from(span))
                .unwrap_or(u64::MAX);
            // Yoklamadan sonra da kanıt: hüküm cephesine dek yoklama öncesi tavanın en az %90'ı
            // gönderildi (bir datagramlık aralıktan kısa süre boşalma fırsatı değildir).
            let after_span = rec.send_ns.saturating_sub(p.until_ns);
            let mtu_span = (crate::rate::DEFAULT_MTU + self.line.datagram_overhead)
                .saturating_mul(1_000_000_000)
                / p.base_pace.max(1);
            let after_ok = after_span < mtu_span
                || u128::from(probe_after.unwrap_or(0).saturating_add(mtu_wire))
                    * 1_000_000_000
                    * 1000
                    >= u128::from(p.base_pace)
                        * u128::from(after_span)
                        * u128::from(VERIFY_MIN_PERMILLE);
            let evidence = after_ok
                && sent_bps.saturating_mul(1000)
                    >= p.probe_pace.saturating_mul(VERIFY_MIN_PERMILLE);
            let half = p.expect_ns / 2;
            // [İKİNCİ-ÖLÇÜM] Başlangıç yoklamasının hükmü, ACK ölçüm aralığı taşıyorsa onun kuyruk
            // artışıyla (alıcının veri varış süresi; dönüş gecikmesi girmez). M iki ACK geriye
            // uzanır: yoklamadan önce başlar. Geri bildirimi aç profilde yoklama dürüst hatta da
            // (ilk pencere bilgi vermediyse) yapılır; cephe ölçüsü orada dönüş gecikmesi
            // basamağını kuyruk sayıp hattı yanlış keserdi (ölçüldü: starved_line_sim c ve d3,
            // dönüş gecikmesi 300 → 700 ms).
            let (passed, failed) = match interval_result.filter(|_| p.by_interval) {
                Some(r) => {
                    let g = u64::try_from(r.growth_ns().max(0)).unwrap_or(u64::MAX);
                    (g <= half, g > half)
                }
                None => (
                    abs_q <= half && growth <= half,
                    abs_q > half && growth > half,
                ),
            };
            if best.is_none() && (!evidence || !(passed || failed)) {
                self.pace_bps = p.base_pace;
                verified = false;
                inconclusive = evidence;
            } else if best.is_none() {
                if passed {
                    self.pace_bps = p.probe_pace;
                    self.rate_bps = self
                        .rate_bps
                        .filter(|_| self.pace_bps < self.line.rate_bps)
                        .map(|r| r.max(self.pace_bps));
                    self.raises = self.raises.saturating_add(1);
                    self.step_permille = (self.step_permille * 2).min(STEP_MAX_PERMILLE);
                } else {
                    // Hat yoklama tavanının altında, yoklama öncesi tavan kadar: ölçülen hız o
                    // tavandır (koruma ölçülmemiş başladıysa da), sonraki yoklama en küçük adımla.
                    self.pace_bps = p.base_pace;
                    self.rate_bps = Some(self.rate_bps.map_or(p.base_pace, |r| r.min(p.base_pace)));
                    self.step_permille = STEP_MIN_PERMILLE;
                    self.probe_failures = self.probe_failures.saturating_add(1);
                    // Yoklamanın kuyruğa eklediği: ölçülen artış (referans ile hüküm noktası
                    // arası) ve yoklama öncesi tavanın üstünde gönderilen bayt; büyüğü. Dönüş
                    // gecikmesi basamağı yanlış bir başarısızlık verirse bedel artış kadar tek
                    // seferlik bir beklemedir.
                    let base_bytes =
                        u64::try_from(u128::from(p.base_pace) * u128::from(span) / 1_000_000_000)
                            .unwrap_or(u64::MAX);
                    let growth_bytes =
                        u64::try_from(u128::from(p.base_pace) * u128::from(growth) / 1_000_000_000)
                            .unwrap_or(u64::MAX);
                    probe_drain_bytes = sent.saturating_sub(base_bytes).max(growth_bytes);
                }
                // Hüküm aralığı yeni bir doğrulama değildir.
                verified = false;
            }
        }
        if let Some(est) = best {
            self.probe = None;
            // Örnek tavanı aşamaz: kuyrukta düşen bayt aralığı şişirir, hat tavandan hızlıysa
            // kuyruk büyümezdi.
            let cap = self.pace_bps.min(self.line.rate_bps);
            let mut est = est.clamp(1, cap);
            if self.on_first_ack() {
                // [İKİNCİ-ÖLÇÜM] İlk ACK'in tek ölçümü bütçenin hızının altına indirmez.
                est = est.max(self.start_floor_bps().min(cap));
            } else if self.in_provisional() {
                // [İKİNCİ-ÖLÇÜM] Geçici evrede ikinci ölçüm: güvenilir, bütçe kalkar.
                self.start_confirmed = true;
                self.start_exit = Some(StartExit::Sample(est));
            }
            self.rate_bps = Some(est);
            self.pace_bps = est;
            self.samples = self.samples.saturating_add(1);
            // Ölçülmüş hatta yoklama en küçük adımdan yeniden başlar: sabit dar hatta her yoklama
            // en çok adım × ACK aralığı kadar kuyruk kurar.
            self.step_permille = STEP_MIN_PERMILLE;
        } else if verified && self.probe.is_none() && self.pace_bps < self.line.rate_bps {
            self.begin_probe(now_ns, point);
        }
        // [İKİNCİ-ÖLÇÜM] Geçici evrede örnek yoksa: ölçüm aralığı bütçenin hızında (en az %90)
        // gönderilmiş ve kuyruk büyümemişse hat en az o hızdadır. Tek ölçümün (ilk pencere ya da
        // ilk ACK'in aralığı) gösterdiği daha yüksek hız doğrulanmamıştır: tavan bütçenin hızına
        // iner, yukarısı tek bir başlangıç yoklamasıyla sınanır. Gönderici bütçenin altında
        // kaldıysa (uygulama sınırlı) karar yok, evre sürer.
        if best.is_none() && self.in_provisional() {
            let floor = self.start_floor_bps();
            if let Some(IntervalResult::Flat(sent, _)) = interval_result {
                if sent.saturating_mul(1000) >= floor.saturating_mul(VERIFY_MIN_PERMILLE) {
                    // Tek ölçümün iddiası (örneğin hızı; bilgi yoksa profil hattı) tek bir kısa
                    // yoklamayla sınanır: tavan bütçenin hızına iner, yoklama iddia edilen hıza
                    // çıkar. Gönderici bütçeyle sınırlıydı (kanıt): yoklama süresince gönderecek işi
                    // var. Süre: hat bütçenin hızındaysa kuyruk [`PROBE_QUEUE_NS`]; en az iki tam
                    // datagram yoklama hızında (datagram tanecikliğinden büyük iz). Hüküm ölçüm
                    // aralığının kuyruk artışıyla (yoksa cepheyle): başarıda tavan iddiada kalır,
                    // başarısızlıkta hat bütçenin hızında ölçülür ve fazlası boşaltılır.
                    let up = self.pace_bps.min(self.line.rate_bps);
                    self.probe = None;
                    self.pace_bps = up.min(floor);
                    self.rate_bps = self.rate_bps.map(|r| r.min(self.pace_bps));
                    self.step_permille = STEP_MIN_PERMILLE;
                    self.start_confirmed = true;
                    self.start_exit = Some(StartExit::Flat(self.pace_bps));
                    if up > self.pace_bps {
                        self.begin_start_probe(now_ns, point, up);
                    }
                }
            }
        }
        let rate = self.rate_bps?;
        // [SÜRELİ-YOKLAMA] Yoklama sürerken kuyruk yazılmaz: yoklama hattı sınar, boşaltma onunla
        // çelişir; gerçek bir kuyruk varsa hüküm ACK'i onu boşaltır.
        if self.probe.is_some() || inconclusive {
            return None;
        }
        // `L` hattı `τ − taban` anında terk etti; o andan bu yana hat `rate × süre` boşalttı.
        let since_ns = now_ns.saturating_sub(tau).saturating_add(base);
        let drained = u64::try_from(u128::from(rate) * u128::from(since_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        let by_bytes = self.cum.saturating_sub(rec.cum).saturating_sub(drained);
        // `L`'nin kuyrukta beklediği süre kadar hat baytı (gerçek kuyruğun alt sınırı).
        let queued_ns = self
            .queue_peaks
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .max(lag.saturating_sub(base));
        let by_delay = u64::try_from(u128::from(rate) * u128::from(queued_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        // [SÜRELİ-YOKLAMA] Başarısız yoklamanın fazla baytı boşaltılır. Kuyruk tahmini (`by_bytes`,
        // `by_delay`) bunu göremeyebilir: taban son noktaların en küçük gecikmesidir ve birkaç ACK
        // süren bir kuyruğu dönüş gecikmesi artışı sayar.
        Some(LineGuardUpdate {
            rate_bps: rate,
            backlog_bytes: by_bytes.min(by_delay),
            extra_backlog_bytes: probe_drain_bytes,
        })
    }
}

impl LineGuard {
    /// [SÜRELİ-YOKLAMA] Tavanı adım kadar (`step_permille`) yükselten yoklamayı başlatır. Yükseltme,
    /// hat değişmediyse [`PROBE_QUEUE_NS`] kadar kuyruk kuracak süre (en çok
    /// [`VERIFY_MIN_SPAN_NS`]) sürer; kalıcılığı hükme bağlı. `point` yoklamanın başladığı ACK
    /// noktası.
    fn begin_probe(&mut self, now_ns: u64, point: Point) {
        let up = (self.pace_bps.saturating_mul(1000 + self.step_permille) / 1000)
            .clamp(self.pace_bps + 1, self.line.rate_bps);
        // En az sekiz tam datagram yoklama hızında: kanıtın bir datagramlık payı uygulama sınırlı
        // gönderimi (yoklama öncesi tavan, %50 adımda yoklamanın 2/3'ü) ayırt etsin.
        self.probe_to(now_ns, point, up, 8);
    }

    /// [İKİNCİ-ÖLÇÜM] Geçici evrenin sonunda tek ölçümün iddiasını (`up`) sınayan yoklama. Gönderici
    /// bütçeyle sınırlı olduğu kanıtlandığından uygulama sınırlılığını ayırma payı gerekmez; süre
    /// yalnız datagram tanecikliğinden büyük iz için en az iki tam datagram.
    fn begin_start_probe(&mut self, now_ns: u64, point: Point, up: u64) {
        let up = up.clamp(self.pace_bps + 1, self.line.rate_bps);
        self.probe_to(now_ns, point, up, 2);
        if let Some(p) = self.probe.as_mut() {
            p.by_interval = true;
        }
    }

    /// Yoklama: tavan `up`'a çıkar; süre hat değişmediyse [`PROBE_QUEUE_NS`] kadar kuyruk kuracak
    /// süre, en az `min_datagrams` tam datagram yoklama hızında, en çok [`VERIFY_MIN_SPAN_NS`].
    fn probe_to(&mut self, now_ns: u64, point: Point, up: u64, min_datagrams: u64) {
        let min_span = (min_datagrams * (crate::rate::DEFAULT_MTU + self.line.datagram_overhead))
            .saturating_mul(1_000_000_000)
            / up.max(1);
        let span_ns = u64::try_from(
            u128::from(PROBE_QUEUE_NS) * u128::from(self.pace_bps.max(1))
                / u128::from((up - self.pace_bps).max(1)),
        )
        .unwrap_or(u64::MAX)
        .clamp(min_span.min(VERIFY_MIN_SPAN_NS), VERIFY_MIN_SPAN_NS);
        let expect_ns = u64::try_from(
            u128::from(up - self.pace_bps) * u128::from(span_ns) / u128::from(self.pace_bps.max(1)),
        )
        .unwrap_or(u64::MAX);
        self.probe = Some(Probe {
            base_pace: self.pace_bps,
            probe_pace: up,
            until_ns: now_ns.saturating_add(span_ns),
            expect_ns,
            reference: point,
            start_ns: now_ns,
            sent: 0,
            by_interval: false,
        });
        self.pace_bps = up;
    }
}

/// `bytes` baytın `rate_bps` hızında serileşmesi (ns); hız 0 ise `None`.
fn ns_for(bytes: u64, rate_bps: u64) -> Option<u64> {
    if rate_bps == 0 {
        return None;
    }
    u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(rate_bps)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;
    const SEC: u64 = 1_000_000_000;
    const PROFILE: u64 = 12_500;
    const RET: u64 = 62;
    const OVH: u64 = 28;

    /// Birim testleri göndericiyi profil hızında sürer: ilk tavan profil hattı.
    fn guard() -> LineGuard {
        LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            PROFILE,
        )
    }

    /// Profil hızında `n` × 1 172 B gövde (1 200 B tel) gönderir, gerçek hat `real` B/s, ileri
    /// yayılım 300 ms; her iletimin alıcıya varış anını döner.
    fn send_into(g: &mut LineGuard, real: u64, n: u64) -> Vec<u64> {
        let mut free = 0u64;
        let mut arrivals = Vec::new();
        for k in 0..n {
            let s = k * 1200 * SEC / PROFILE;
            g.on_sent(k, s, 1172);
            let done = free.max(s) + 1200 * SEC / real;
            free = done;
            arrivals.push(done + 300 * MS);
        }
        arrivals
    }

    /// Alıcı `seq`'i aldığında hemen `wire` B'lik ACK çıkarır; ACK 300 ms + serileşmeyle gelir.
    fn ack(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        wire: u64,
    ) -> (u64, Option<LineGuardUpdate>) {
        let at = arrivals[seq as usize] + 300 * MS + (wire + OVH) * SEC / RET;
        g.note_ack_wire(wire);
        (at, g.on_ack(at, 0, false, seq))
    }

    #[test]
    fn honest_line_gives_no_sample_even_with_unequal_ack_sizes() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 60);
        // 37 B ve 101 B: ACK serileşmesi 1,05 s ↔ 2,08 s. Düzeltmesiz bu 1 s'lik sahte kuyruk
        // artışı olurdu.
        for (seq, wire) in [(1, 37), (25, 101), (50, 55)] {
            let (_, u) = ack(&mut g, &arr, seq, wire);
            assert_eq!(u, None, "dürüst hatta koruma devreye girmez");
        }
        assert_eq!(g.rate_bps(), None);
    }

    /// [ÖLÇÜM-ARALIĞI] ACK dönüş hattında `extra_ns` geç gelir. `first` verilirse çerçeve ölçüm
    /// aralığını taşır (alıcıdaki varış süresi `arrivals[seq] − arrivals[first]`).
    fn ack_m(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        extra_ns: u64,
        first: Option<u64>,
    ) -> Option<LineGuardUpdate> {
        let wire = 55;
        let at = arrivals[seq as usize] + 300 * MS + extra_ns + (wire + OVH) * SEC / RET;
        g.note_ack_wire(wire);
        let iv = first.map(|f| ForwardInterval {
            first_seq: f,
            last_seq: seq,
            rx_span_ns: arrivals[seq as usize] - arrivals[f as usize],
            rx_wire: None,
        });
        g.on_ack_measured(at, 0, false, seq, iv)
    }

    /// [ÖLÇÜM-ARALIĞI · ÖLÇÜM-BOŞLUĞU] Dürüst hat, dönüş gecikmesi 300 ms adımlarla artıyor. Ölçüm
    /// aralığı varken cephe tahmincisi örnek vermez (basamak kuyruk sayılmaz). Aralık kipinde üst
    /// üste iki ACK ölçümsüz kalınca da cephe örneği alınmaz (İŞ-35 öncesi kural burada keserdi);
    /// yerine kapasite iddia etmeyen boşluk bütçesi kurulur, ilk ölçülü ACK'te kalkar. Hız ve tavan
    /// değişmez. Karşı-olgu: ölçüm aralığı hiç gelmeyen koruma (eski alıcı) basamağı keser
    /// (bugünkü davranış, değişmedi).
    /// [ÖLÇÜM-BOŞLUĞU] Öncelikli trafik (PTT) son iki saniyede tavanın yarısından fazla gönderdiyse
    /// boşluk bütçesi kurulmaz: o yükte kuyruğu koruyamaz, yalnız öncelikli olmayan işi (dosya,
    /// yeniden iletim) aç bırakır. Karşı-olgu (iletimler öncelikli diye işaretlenmeden, birleşimdeki
    /// ilk hesap): bütçe kurulur ve öncelikli trafik kovayı sürekli eksiye çektiği için öncelikli
    /// olmayan iş 2 s boyunca hiç izin almaz.
    #[test]
    fn gap_budget_is_not_armed_when_priority_fills_half_the_line() {
        let run = |mark: bool| {
            let mut g = guard();
            let arr = send_into(&mut g, PROFILE, 100);
            let _ = ack_m(&mut g, &arr, 10, 0, Some(1));
            g.end_start_budget();
            let _ = ack_m(&mut g, &arr, 30, 0, Some(10));
            // İkinci ölçümsüz ACK'ten (seq 70, ≈ 8,6 s) önceki 2 s: her 16 ms'de 114 B öncelikli
            // iletim (7 125 B/s > R/2 = 6 250).
            let t_ack70 = arr[70] + 300 * MS + (55 + OVH) * SEC / RET;
            let mut seq = 1_000u64;
            let mut t = t_ack70 - 2 * SEC;
            while t < t_ack70 {
                g.on_sent_classed(seq, t, 86, 0, mark);
                seq += 1;
                t += 16 * MS;
            }
            let _ = ack_m(&mut g, &arr, 50, 0, None);
            let _ = ack_m(&mut g, &arr, 70, 0, None);
            // Sonraki 2 s: öncelikli trafik sürer, her 1 ms öncelikli olmayan 1 172 B denemesi.
            let mut admitted = 0u64;
            for k in 0..2_000u64 {
                let now = t_ack70 + k * MS;
                if k % 16 == 0 {
                    g.on_sent_classed(seq, now, 86, 0, mark);
                    seq += 1;
                }
                if g.start_budget_admits(now, 1172) {
                    g.on_sent_classed(seq, now, 1172, 0, false);
                    seq += 1;
                    admitted += 1200;
                }
            }
            (g.gap_budget_active(), g.gap_skips(), admitted)
        };
        let (armed, skips, admitted) = run(true);
        assert!(
            !armed && skips == 1,
            "kurulmadı: armed={armed} skips={skips}"
        );
        assert!(
            admitted >= 12 * 1200,
            "öncelikli olmayan iş ilerler: {admitted} B"
        );
        let (armed, _, admitted) = run(false);
        assert!(armed, "karşı-olgu: bütçe kuruldu");
        assert!(
            admitted <= 1200,
            "karşı-olgu: öncelikli olmayan iş aç kalır ({admitted} B)"
        );
    }

    #[test]
    fn interval_mode_never_takes_a_front_sample_and_arms_the_gap_budget() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 100);
        assert_eq!(ack_m(&mut g, &arr, 10, 0, Some(1)), None);
        // Gönderici ilk ACK'ten sonra başlangıç bütçesini kaldırır (`Sender`).
        g.end_start_budget();
        assert_eq!(ack_m(&mut g, &arr, 30, 300 * MS, Some(10)), None, "basamak");
        assert_eq!(g.rate_bps(), None);
        let (checks, samples, last_g) = g.interval_diag();
        assert_eq!((checks, samples), (2, 0));
        assert!(
            last_g.is_some_and(|x| x.abs() < 1_000_000),
            "g ≈ 0: {last_g:?}"
        );
        // Aralık kipinde tek ölçümsüz ACK: cephe örneği yok, bütçe yok.
        let _ = ack_m(&mut g, &arr, 50, 600 * MS, None);
        assert_eq!(g.rate_bps(), None, "tek ölçümsüz ACK");
        assert!(!g.gap_budget_active());
        // İkinci ölçümsüz ACK: cephe örneği yine yok; boşluk bütçesi kurulur ve tek seferlik
        // boşaltma cephenin beklemesi (≈ basamak) kadardır.
        let _ = ack_m(&mut g, &arr, 70, 900 * MS, None);
        assert_eq!(g.rate_bps(), None, "iki ölçümsüz ACK'te cephe örneği yok");
        assert_eq!(g.pace_bps(), PROFILE, "tavan değişmez");
        assert!(g.gap_budget_active(), "boşluk bütçesi kuruldu");
        assert_eq!(g.gap_arms(), 1);
        let drain = g.take_gap_drain();
        assert!(
            drain > 0 && drain <= PROFILE * 900 / 1000,
            "boşaltma {drain}"
        );
        // Boşluk bütçesi: tavanın yarısı hızında, ilk patlama yok.
        let t0 = 100 * 1200 * SEC / PROFILE;
        g.on_sent(100, t0, 1172);
        assert!(!g.start_budget_admits(t0, 1172), "patlama yok");
        assert!(g.start_budget_admits(t0 + 2 * 1200 * SEC / PROFILE * 2, 1172));
        // Ölçüm döner: bütçe kalkar.
        let _ = ack_m(&mut g, &arr, 90, 900 * MS, Some(71));
        assert!(!g.gap_budget_active(), "ölçülü ACK bütçeyi kaldırır");
        assert_eq!(g.rate_bps(), None);
        // Karşı-olgu: ölçüm aralığı hiç gelmeyen koruma basamağı ikinci ACK'te keser.
        let mut old = guard();
        let arr = send_into(&mut old, PROFILE, 80);
        let _ = ack_m(&mut old, &arr, 10, 0, None);
        let _ = ack_m(&mut old, &arr, 30, 300 * MS, None);
        assert!(old.rate_bps().is_some_and(|r| r < PROFILE * 19 / 20));
        assert!(!old.gap_budget_active(), "eski alıcıda boşluk kuralı yok");
    }

    /// [TAŞMA] Ölçüm aralığı açıkça verilir: alıcıdaki süre `rx_span_ns`, alınan tel baytı `rx_wire`.
    fn ack_iv(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        first: u64,
        rx_span_ns: u64,
        rx_wire: u64,
    ) -> Option<LineGuardUpdate> {
        let wire = 55;
        let at = arrivals[seq as usize] + 300 * MS + (wire + OVH) * SEC / RET;
        g.note_ack_wire(wire);
        let iv = ForwardInterval {
            first_seq: first,
            last_seq: seq,
            rx_span_ns,
            rx_wire: Some(rx_wire),
        };
        g.on_ack_measured(at, 0, false, seq, Some(iv))
    }

    /// [TAŞMA] Düşük teslim tek başına örnek vermez; artış örneğinden sonraki aşırı kayıp verir.
    /// Taban üç temiz aralıkta %80 teslim. Sonra %40 teslimli, kuyruğu değişmeyen aralık: kuyruk
    /// kanıtı yok → örnek yok (rastgele kayıp artışı olabilir). Artış örneğinden (alıcıdaki süre
    /// gönderimin 1,5 katı → 8 333) sonra aynı %40'lık aralık taşma örneği verir: hattı geçen bayt
    /// = alınan / taban kesri.
    #[test]
    fn overflow_needs_queue_evidence_and_excess_loss() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 200);
        let span = |a: u64, b: u64| arr[b as usize] - arr[a as usize];
        let wire = |a: u64, b: u64| (b - a) * 1200;
        let _ = ack_iv(&mut g, &arr, 10, 1, span(1, 10), wire(1, 10) * 8 / 10);
        g.end_start_budget();
        let _ = ack_iv(&mut g, &arr, 40, 11, span(11, 40), wire(11, 40) * 8 / 10);
        let _ = ack_iv(&mut g, &arr, 70, 41, span(41, 70), wire(41, 70) * 8 / 10);
        assert_eq!(g.rate_bps(), None, "temiz aralıklar");
        let _ = ack_iv(&mut g, &arr, 100, 71, span(71, 100), wire(71, 100) * 4 / 10);
        assert_eq!(
            g.rate_bps(),
            None,
            "kuyruk kanıtsız düşük teslim örnek değil"
        );
        assert_eq!(g.overflow_samples(), 0);
        let _ = ack_iv(
            &mut g,
            &arr,
            130,
            101,
            span(101, 130) * 3 / 2,
            wire(101, 130) * 8 / 10,
        );
        let grown = g.rate_bps().expect("artış örneği");
        assert!(grown.abs_diff(8_333) <= 100, "artış örneği {grown}");
        let _ = ack_iv(
            &mut g,
            &arr,
            160,
            131,
            span(131, 160),
            wire(131, 160) * 4 / 10,
        );
        let r = g.rate_bps().expect("taşma örneği");
        assert_eq!(g.overflow_samples(), 1);
        // Taban kuyruk kanıtsız aralıkların hepsidir (kalıcı kayıp değişimini izler): üç %80'lik
        // ve bir %40'lık aralık → (0,8 × 67 + 0,4 × 29) / 96 ≈ 0,679 (güvenli yön: tahmin yükselir).
        let floor_rx = wire(1, 10) * 8 / 10
            + wire(11, 40) * 8 / 10
            + wire(41, 70) * 8 / 10
            + wire(71, 100) * 4 / 10;
        let floor_sent = wire(1, 10) + wire(11, 40) + wire(41, 70) + wire(71, 100);
        let expect = wire(131, 160) * 4 / 10 * floor_sent / floor_rx * SEC / span(131, 160);
        assert!(
            r.abs_diff(expect) * 100 <= expect,
            "taşma örneği {r} (beklenen {expect})"
        );
    }

    /// [ÖLÇÜM-ARALIĞI] Gerçek hat yarıya düşük: aralık kuyruk artışını görür ve hattı ±%2 ölçer.
    /// M iki ACK geriye uzanır: `sent` kayıtları iki cephe geriye dek tutulur (M'nin başı bulunur).
    #[test]
    fn interval_measures_a_slow_line_from_records_two_fronts_back() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 80);
        assert!(ack_m(&mut g, &arr, 10, 0, None).is_none());
        let _ = ack_m(&mut g, &arr, 20, 0, Some(10));
        let before = g.interval_diag().0;
        let u = ack_m(&mut g, &arr, 40, 0, Some(10)).expect("örnek");
        assert_eq!(g.interval_diag().0, before + 1, "M'nin başı (10) kayıtta");
        assert!(
            u.rate_bps.abs_diff(6_250) * 50 <= 6_250,
            "tahmin {}",
            u.rate_bps
        );
    }

    #[test]
    fn slow_line_is_measured_within_one_percent_and_never_above_profile() {
        for real in [6_250u64, 10_000] {
            let mut g = guard();
            let arr = send_into(&mut g, real, 60);
            let _ = ack(&mut g, &arr, 1, 37);
            let (_, u) = ack(&mut g, &arr, 20, 101);
            let u = u.expect("kuyruk büyüdü → ölçüm");
            assert!(
                u.rate_bps.abs_diff(real) * 100 <= real,
                "gerçek {real} B/s, tahmin {} B/s",
                u.rate_bps
            );
            assert!(u.rate_bps <= PROFILE);
        }
    }

    /// Yazılan kuyruk gerçek kuyruğu aşmaz ve cephenin kendi beklediği kuyruktan az değildir;
    /// kuyrukta düşen bayt kuyruk sayılmaz.
    #[test]
    fn backlog_estimate_never_exceeds_the_bottleneck_queue() {
        for dropped in [0u64, 20_000] {
            let real = 6_250u64;
            let mut g = guard();
            let mut free = 0u64;
            let mut arr = Vec::new();
            for k in 0..60u64 {
                let s = k * 1200 * SEC / PROFILE;
                if k == 25 {
                    g.on_unsequenced(dropped);
                }
                g.on_sent(k, s, 1172);
                free = free.max(s) + 1200 * SEC / real;
                arr.push(free + 300 * MS);
            }
            let _ = ack(&mut g, &arr, 0, 37);
            let (at, u) = ack(&mut g, &arr, 20, 101);
            let u = u.expect("ölçüm");
            // Gerçek kuyruk `at` anında: gönderilmiş ama hattı henüz terk etmemiş iletimler.
            let sent_by = (0..60u64)
                .filter(|k| k * 1200 * SEC / PROFILE <= at)
                .count() as u64;
            let left_by = arr.iter().filter(|&&a| a - 300 * MS <= at).count() as u64;
            let queue = (sent_by - left_by) * 1200;
            // Cephe (seq 20) kuyrukta `varış − 300 ms − gönderim − kendi serileşmesi` bekledi.
            let own_wait = arr[20] - 300 * MS - 20 * 1200 * SEC / PROFILE - 1200 * SEC / real;
            let floor = real * own_wait / SEC;
            assert!(
                u.backlog_bytes <= queue && u.backlog_bytes + 1200 >= floor,
                "düşen {dropped} B: kuyruk {queue} B, cephenin kuyruğu {floor} B, yazılan {} B",
                u.backlog_bytes
            );
        }
    }

    /// Kuyruk taşarken gönderilmiş baytları kapsayan aralık (gönderim baytı hattın taşıdığından
    /// fazla) korunan hızı yükseltemez.
    #[test]
    fn inflated_span_cannot_raise_the_guard() {
        let real = 6_250u64;
        let mut g = guard();
        let mut free = 0u64;
        let mut arr = Vec::new();
        for k in 0..60u64 {
            let s = k * 1200 * SEC / PROFILE;
            if k == 25 {
                // 20 000 B gönderildi ama kuyrukta düştü: hattı kullanmadı.
                g.on_unsequenced(20_000);
            }
            g.on_sent(k, s, 1172);
            free = free.max(s) + 1200 * SEC / real;
            arr.push(free + 300 * MS);
        }
        let _ = ack(&mut g, &arr, 1, 37);
        let (_, u) = ack(&mut g, &arr, 20, 101);
        let first = u.expect("temiz aralık").rate_bps;
        assert!(first.abs_diff(real) * 100 <= real, "temiz ölçüm {first}");
        // 20 → 50 aralığı düşen baytı içerir: tek başına ≈ 9,7 kB/s verirdi.
        let (_, later) = ack(&mut g, &arr, 50, 101);
        assert_eq!(
            later.map(|u| u.rate_bps),
            Some(first),
            "şişkin aralık hızı yükseltmez"
        );
    }

    #[test]
    fn stale_or_saturated_acks_give_no_point() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 60);
        let _ = ack(&mut g, &arr, 10, 37);
        // Cephe ilerlemedi.
        let (_, u) = ack(&mut g, &arr, 10, 37);
        assert_eq!(u, None);
        // Tutma süresi kodlama tavanında: alıcının gerçek tutma süresi bilinmez.
        g.note_ack_wire(37);
        assert_eq!(g.on_ack(60 * SEC, 0, true, 30), None);
        // ACK boyu bildirilmeden gelen çerçeve nokta üretmez.
        assert_eq!(g.on_ack(60 * SEC, 0, false, 40), None);
        assert_eq!(g.rate_bps(), None);
    }

    /// Gönderim `send_rate` B/s ile (1 200 B tel), gerçek hat `real`, ileri yayılım 300 ms;
    /// her iletimin alıcıya varış anı.
    #[allow(dead_code)]
    fn send_at(g: &mut LineGuard, send_rate: u64, real: u64, n: u64) -> Vec<u64> {
        let mut free = 0u64;
        let mut arrivals = Vec::new();
        for k in 0..n {
            let s = k * 1200 * SEC / send_rate;
            g.on_sent(k, s, 1172);
            let done = free.max(s) + 1200 * SEC / real;
            free = done;
            arrivals.push(done + 300 * MS);
        }
        arrivals
    }

    /// `seq`'in ACK'i; dönüş yayılımı 300 ms + `extra`.
    fn ack_extra(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        extra: u64,
    ) -> Option<LineGuardUpdate> {
        let at = arrivals[seq as usize] + 300 * MS + extra + (101 + OVH) * SEC / RET;
        g.note_ack_wire(101);
        g.on_ack(at, 0, false, seq)
    }

    /// Nedensel sürücü: gönderici her an korumanın tavanında (ya da uygulama tavanı `app_cap`
    /// altında) 1 172 B gövde gönderir; tavanı `tick` ve ACK'ler değiştirir. Gerçek ileri hat
    /// `real`, ileri yayılım 300 ms. Alıcı her `ack_every` sürede o ana dek varan en büyük seq için
    /// 37 B'lik ACK çıkarır (`ack_delay` = çıkış − o seq'in varışı); ACK 300 ms + serileşmeyle gelir.
    /// Dönen: her ACK'ten sonra `(an, tavan, güncelleme)` ve her iletimin `(an, kuyruk beklemesi)`.
    #[allow(clippy::type_complexity)]
    fn drive(
        g: &mut LineGuard,
        real: u64,
        app_cap: Option<u64>,
        t_end: u64,
        ack_every: u64,
    ) -> (Vec<(u64, u64, Option<LineGuardUpdate>)>, Vec<(u64, u64)>) {
        let mut acks_out = Vec::new();
        let mut waits = Vec::new();
        let (mut free, mut next_send, mut seq, mut now) = (0u64, 0u64, 0u64, 0u64);
        let mut in_flight: VecDeque<(u64, u64)> = VecDeque::new();
        let mut last_arrived: Option<(u64, u64)> = None;
        let mut acks: VecDeque<(u64, u64, u64)> = VecDeque::new();
        let mut next_ack = ack_every;
        while now < t_end {
            g.tick(now);
            if now >= next_send {
                g.on_sent(seq, now, 1172);
                let start = free.max(now);
                waits.push((now, start - now));
                free = start + 1200 * SEC / real;
                in_flight.push_back((free + 300 * MS, seq));
                seq += 1;
                let rate = app_cap.map_or(g.pace_bps(), |c| c.min(g.pace_bps())).max(1);
                next_send = now + 1200 * SEC / rate;
            }
            while in_flight.front().is_some_and(|&(t, _)| t <= now) {
                let (t, q) = in_flight.pop_front().expect("var");
                last_arrived = Some((t, q));
            }
            if now >= next_ack {
                if let Some((t, q)) = last_arrived {
                    acks.push_back((now + 300 * MS + (37 + OVH) * SEC / RET, now - t, q));
                }
                next_ack += ack_every;
            }
            while acks.front().is_some_and(|&(at, _, _)| at <= now) {
                let (_, delay, q) = acks.pop_front().expect("var");
                g.note_ack_wire(37);
                let u = g.on_ack(now, delay, false, q);
                // Ürün göndericisi kuyruğu hat modeline yazar: yeni iletim o kuyruk boşalınca çıkar.
                if let Some(u) = u {
                    let rate = g.pace_bps().max(1);
                    next_send = next_send.max(now + u.backlog_bytes * SEC / rate)
                        + u.extra_backlog_bytes * SEC / rate;
                }
                acks_out.push((now, g.pace_bps(), u));
            }
            now += MS;
        }
        (acks_out, waits)
    }

    /// [YENİDEN-YOKLAMA · SÜRELİ-YOKLAMA] Doğrulanmamış başlangıçtan (yarı profil) dürüst hatta:
    /// her yoklama hattı sınar, hüküm başarılıdır ve tavan kalıcı yükselir; tavan hiçbir zaman
    /// başlangıcın altına inmez ve profil hattına ulaşınca koruma kalkar.
    #[test]
    fn verified_interval_raises_the_ceiling_up_to_the_profile() {
        let mut g = LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            PROFILE / 2,
        );
        g.end_start_budget();
        assert_eq!(g.pace_bps(), PROFILE / 2);
        let (acks, _) = drive(&mut g, PROFILE, None, 90 * SEC, 5 * SEC);
        assert!(
            acks.iter().all(|&(_, pace, _)| pace >= PROFILE / 2),
            "tavan düşmez"
        );
        assert!(g.raises() >= 1, "doğrulama yok");
        assert_eq!(g.probe_failures(), 0, "dürüst hatta yoklama başarısız");
        assert_eq!(g.pace_bps(), PROFILE);
        assert_eq!(g.rate_bps(), None, "dürüst hatta ölçülen hız yok");
    }

    /// [SÜRELİ-YOKLAMA] Sabit dar hatta (gerçek 6 250 B/s): yoklamalar başarısız, tavan ölçülen
    /// hatta kalır ve yoklamanın kuyruğu boşaltılır. Kuyruk ilk ölçümden sonra en çok bir yoklamanın
    /// kuracağı kadar büyür: [`PROBE_QUEUE_NS`] + iki tam datagramın
    /// serileşmesi (FIFO çözünürlüğü). Eski kural (kalıcı yükseltme) kuyruğu fark edilmeden birkaç
    /// aralık büyütüyordu.
    #[test]
    fn failed_probes_on_a_steady_narrow_line_keep_the_queue_bounded() {
        // Koruma gerçek hattın hızında, ölçülmemiş başlar (kuyruksuz); ilk yoklama başlangıç
        // adımıyla (%50) yapılır ve başarısız olur, sonrakiler en küçük adımla.
        let mut g = LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            6_250,
        );
        g.end_start_budget();
        let (acks, waits) = drive(&mut g, 6_250, None, 150 * SEC, 5 * SEC);
        assert!(g.probe_failures() >= 2, "yoklama yok ya da hep başarılı");
        assert_eq!(g.raises(), 0, "dar hatta kalıcı yükseltme");
        assert_eq!(g.rate_bps(), Some(6_250), "ölçülen hat");
        // İlk (büyük adımlı) yoklamanın boşaltılmasından sonra.
        let settle = acks
            .iter()
            .find(|&&(_, _, u)| u.is_some_and(|u| u.backlog_bytes > 0))
            .map(|&(at, _, _)| at + 10 * SEC)
            .expect("başarısız yoklama boşaltıldı");
        let max_wait = waits
            .iter()
            .filter(|&&(t, _)| t >= settle)
            .map(|&(_, w)| w)
            .max()
            .unwrap_or(0);
        let bound = PROBE_QUEUE_NS + 2 * 1200 * SEC / 6_250;
        assert!(max_wait <= bound, "kuyruk {max_wait} ns (sınır {bound} ns)");
    }

    /// [SÜRELİ-YOKLAMA] Uygulama sınırlı gönderici (yoklama tavanında gönderemiyor): yoklamanın
    /// kanıtı yok, hüküm verilmez; tavan kalıcı yükselmez ve yoklama başarısız sayılmaz.
    #[test]
    fn an_app_limited_probe_is_inconclusive() {
        let mut g = LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            PROFILE / 2,
        );
        g.end_start_budget();
        let (_, _) = drive(&mut g, PROFILE, Some(PROFILE / 2), 60 * SEC, 5 * SEC);
        assert_eq!(g.raises(), 0, "kanıtsız yükseltme");
        assert_eq!(g.probe_failures(), 0, "kanıtsız başarısızlık");
        assert!(g.pace_bps() <= PROFILE / 2 * (1000 + STEP_START_PERMILLE) / 1000);
    }

    /// [DÖNÜŞ-GECİKMESİ] Dönüş gecikmesinin tek basamağı (400 ms) yanlış düşük örnek verebilir; sonraki
    /// sabit gecikmeli aralıklar onu yeniden yoklamayla geri alır ve kalıcı kuyruk yazılmaz.
    #[test]
    fn return_delay_step_does_not_lock_a_low_rate() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 2000);
        let _ = ack_extra(&mut g, &arr, 0, 0);
        let _ = ack_extra(&mut g, &arr, 40, 0);
        // Basamak: bundan sonraki her ACK 400 ms geç döner.
        let _ = ack_extra(&mut g, &arr, 80, 400 * MS);
        let mut backlog_late = 0;
        for seq in [200u64, 400, 600, 800, 1000, 1200, 1400] {
            if let Some(u) = ack_extra(&mut g, &arr, seq, 400 * MS) {
                backlog_late = u.backlog_bytes + u.extra_backlog_bytes;
            }
        }
        assert_eq!(
            g.rate_bps(),
            None,
            "dönüş gecikmesi ileri hattın daralması kaldı"
        );
        assert_eq!(g.pace_bps(), PROFILE);
        assert_eq!(backlog_late, 0, "kalıcı yanlış kuyruk");
    }

    /// [YENİDEN-YOKLAMA] Ölçülen dar hat oturum içinde hızlanınca tavan yeniden yükselir.
    #[test]
    fn measured_narrow_line_recovers_when_the_line_speeds_up() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 60);
        let _ = ack(&mut g, &arr, 1, 37);
        let _ = ack(&mut g, &arr, 20, 101);
        let low = g.rate_bps().expect("dar hat ölçüldü");
        assert!(low.abs_diff(6_250) * 100 <= 6_250 * 2);
        // Hat hızlandı: gönderici tavanda (≈ 6 250) gönderir, kuyruk büyümez.
        let mut g2 = g.clone();
        let base = arr[59] + SEC;
        let mut free = 0u64;
        let mut arr2 = Vec::new();
        for k in 0..600u64 {
            let s = base + k * 1200 * SEC / g2.pace_bps().max(1);
            g2.on_sent(1000 + k, s, 1172);
            let done = free.max(s) + 1200 * SEC / PROFILE;
            free = done;
            arr2.push(done + 300 * MS);
        }
        for k in [0u64, 100, 200, 300, 400, 500, 599] {
            let at = arr2[k as usize] + 300 * MS + (101 + OVH) * SEC / RET;
            g2.note_ack_wire(101);
            let _ = g2.on_ack(at, 0, false, 1000 + k);
        }
        assert!(
            g2.pace_bps() > low,
            "tavan yükselmedi: {} ≤ {low}",
            g2.pace_bps()
        );
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] İlk ACK'e dek meşgul gönderimde toplam sınır
    /// `max(P × START_BURST_NS, P/2 × t)`: patlama 6 250 B tel (beş 1 200 B datagram), dolum
    /// `2 × START_BURST_NS`'te başlar; bütçe kalkınca sınır yok.
    #[test]
    fn start_budget_is_a_burst_then_half_the_profile_line() {
        let mut g = guard();
        assert!(g.start_budget_active());
        let mut sent = 0u64;
        while g.start_budget_admits(0, 1172) {
            g.on_sent(sent, 0, 1172);
            sent += 1;
        }
        assert_eq!(sent, 5, "patlama 6 250 B'ye beş 1 200 B datagram sığar");
        // 1 s (= 2 × START_BURST_NS): dolum yeni başlıyor, 1 200 B'lik yer yok.
        assert!(!g.start_budget_admits(SEC, 1172));
        // 1,2 s: 0,2 s × 6 250 B/s = 1 250 B dolum + 250 B artık → bir datagram daha.
        assert!(g.start_budget_admits(1200 * MS, 1172));
        g.end_start_budget();
        assert!(!g.start_budget_active());
        assert!(g.start_budget_admits(0, 1_000_000));
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Saat ilk iletimle başlar (oturum açılışından değil) ve boşta geçen süre
    /// kovayı derinliğin üstüne dolduramaz: uzun bir sessizlikten sonra da patlama en çok
    /// derinlik kadardır.
    #[test]
    fn start_budget_clock_starts_at_the_first_send_and_idle_time_does_not_accumulate() {
        let mut g = guard();
        // İlk iletim 5 s'de: kova dolu (beş datagram), saat buradan.
        let t0 = 5 * SEC;
        let mut sent = 0u64;
        while g.start_budget_admits(t0, 1172) {
            g.on_sent(sent, t0, 1172);
            sent += 1;
        }
        assert_eq!(sent, 5);
        // 30 s sessizlik: kova en çok derinliğe dolar → yine en çok beş datagram.
        let t1 = t0 + 30 * SEC;
        let mut burst = 0u64;
        while g.start_budget_admits(t1, 1172) {
            g.on_sent(sent + burst, t1, 1172);
            burst += 1;
        }
        assert_eq!(burst, 5, "boşta biriken pay patlama olmaz");
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Patlaması bir datagramdan küçük dar profil (P × START_BURST_NS < 1 228
    /// B) de ilk datagramı gönderir: kova en az bir tam datagramdır, saat ilk iletimle başlar.
    #[test]
    fn start_budget_never_deadlocks_a_narrow_profile() {
        let mut g = LineGuard::new(
            LineModel {
                rate_bps: 2_000,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            2_000,
        );
        assert!(g.start_budget_admits(0, 1172), "ilk datagram sığmalı");
        g.on_sent(0, 0, 1172);
        assert!(!g.start_budget_admits(0, 1172));
        // Dolum 2 × 1 228 / 2 000 ≈ 1,23 s'de başlar; 1 000 B/s ile ≈ 1,2 s sonra yeni datagram.
        assert!(g.start_budget_admits(3 * SEC, 1172));
    }

    /// Alıcının ilk varış penceresinin gövde hızı (`arrival_rate::FirstArrivalWindow` kuralı):
    /// `seqs` artan sırada alınan seq'ler, `arrivals` her seq'in varış anı.
    fn window_field(arrivals: &[u64], seqs: &[u64]) -> u16 {
        let mut w = crate::arrival_rate::FirstArrivalWindow::default();
        for &q in seqs {
            w.on_arrival(q, 1172, arrivals[q as usize]);
        }
        w.rate_field()
    }

    /// [İLK-PENCERE] Profil hızında gönderilen ilk patlama gerçek 6 250 B/s hatta: ilk ACK'in
    /// penceresi hattı ölçer (pencere ilk datagram + bir datagram; tahmin ±%1), tavan ölçülen hatta
    /// iner. Aradaki datagram kayıpsa da (seq 1 düştü) tahmin aynı: gönderici düşen datagramın
    /// baytını kendi kaydından ekler (rastgele kayıp darboğazdan sonra; hattı kullandı).
    #[test]
    fn first_window_measures_a_slow_line_in_one_ack() {
        for acked in [vec![0u64, 1, 2, 3], vec![0, 2, 3]] {
            let mut g = guard();
            let arr = send_into(&mut g, 6_250, 6);
            let field = window_field(&arr, &acked);
            let outcome = g.on_first_window(&acked, field).expect("pencere kuruldu");
            let FirstWindowOutcome::Sample(est) = outcome else {
                panic!("örnek bekleniyordu: {outcome:?} (acked {acked:?})");
            };
            assert!(
                est.abs_diff(6_250) * 100 <= 6_250,
                "tahmin {est} (acked {acked:?})"
            );
            // [İKİNCİ-ÖLÇÜM] Tek örnek tavanı bütçenin hızının (P/2) altına indirmez.
            assert_eq!(g.pace_bps(), est.max(PROFILE / 2));
            assert_eq!(g.rate_bps(), Some(est.max(PROFILE / 2)));
            assert_eq!(g.samples(), 1);
            // Yalnız bir kez: ikinci çağrı etkisiz.
            assert_eq!(g.on_first_window(&acked, field), None);
        }
    }

    /// [İLK-PENCERE] Dürüst hatta pencere kuyruk artışı göstermez ve tavanda gönderilmiştir:
    /// doğrulama, tavan profilde, koruma devrede değil.
    #[test]
    fn first_window_verifies_an_honest_line() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 6);
        let acked = [0u64, 1, 2];
        let field = window_field(&arr, &acked);
        assert_eq!(
            g.on_first_window(&acked, field),
            Some(FirstWindowOutcome::Verified)
        );
        assert_eq!(g.pace_bps(), PROFILE);
        assert_eq!(g.rate_bps(), None);
        assert_eq!(g.samples(), 0);
    }

    /// [İLK-PENCERE] Önce küçük bir metin (100 B gövde), 150 ms sonra dosya parçası (dürüst hat):
    /// pencere tavanın çok altında gönderilmiştir, bilgi yok; tavan değişmez (inceleme bulgusu:
    /// önceki kural burada hattı %62 düşük okurdu).
    #[test]
    fn first_window_after_a_small_text_then_a_chunk_gives_no_info() {
        let mut g = guard();
        g.on_sent(0, 0, 100);
        g.on_sent(1, 150 * MS, 1060);
        g.on_sent(2, 150 * MS + 1088 * SEC / PROFILE, 1060);
        // Dürüst hat: her datagram kendi serileşmesi + 300 ms.
        let arr = [
            128 * SEC / PROFILE + 300 * MS,
            150 * MS + 1088 * SEC / PROFILE + 300 * MS,
            150 * MS + 2 * 1088 * SEC / PROFILE + 300 * MS,
        ];
        let mut w = crate::arrival_rate::FirstArrivalWindow::default();
        w.on_arrival(0, 100, arr[0]);
        w.on_arrival(1, 1060, arr[1]);
        w.on_arrival(2, 1060, arr[2]);
        let field = w.rate_field();
        let out = g.on_first_window(&[0, 1, 2], field);
        assert_ne!(
            out.map(|o| matches!(o, FirstWindowOutcome::Sample(_))),
            Some(true)
        );
        assert_eq!(g.pace_bps(), PROFILE);
        assert_eq!(g.rate_bps(), None);
    }

    /// [İLK-PENCERE] Dürüst hatta küçük bir dosya başlığı ve arkasından profil hızında parçalar:
    /// pencere ilk parçayla açılır (başlık atlanır), boy farkı kuyruk artışı sayılmaz, doğrulama.
    #[test]
    fn first_window_skips_a_small_leading_datagram_on_an_honest_line() {
        let mut g = guard();
        g.on_sent(0, 0, 128);
        let t1 = 156 * SEC / PROFILE;
        g.on_sent(1, t1, 1046);
        g.on_sent(2, t1 + 1074 * SEC / PROFILE, 1046);
        let arr = [
            156 * SEC / PROFILE + 300 * MS,
            t1 + 1074 * SEC / PROFILE + 300 * MS,
            t1 + 2 * 1074 * SEC / PROFILE + 300 * MS,
        ];
        let mut w = crate::arrival_rate::FirstArrivalWindow::default();
        for (q, (b, t)) in [(128u64, arr[0]), (1046, arr[1]), (1046, arr[2])]
            .iter()
            .enumerate()
        {
            w.on_arrival(q as u64, *b, *t);
        }
        let field = w.rate_field();
        assert_eq!(
            g.on_first_window(&[0, 1, 2], field),
            Some(FirstWindowOutcome::Verified)
        );
        assert_eq!(g.rate_bps(), None);
    }

    /// [İLK-PENCERE] Ölçüm yoksa (sentinel: eski alıcı, pencere kapanmadı) ya da bütçe kalktıysa
    /// etkisiz; pencere uygulama sınırlıyken (gönderim tavanın %90'ının altında) karar yok.
    #[test]
    fn first_window_without_a_usable_measurement_is_inert() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 6);
        assert_eq!(
            g.on_first_window(&[0, 1, 2], crate::ack::ARRIVAL_RATE_SENTINEL),
            None
        );
        g.end_start_budget();
        let field = window_field(&arr, &[0, 1, 2]);
        assert_eq!(g.on_first_window(&[0, 1, 2], field), None);
        assert_eq!(g.pace_bps(), PROFILE);

        // Uygulama sınırlı: iki datagram 0,5 s arayla (dürüst hat, kuyruk yok; pencere bekletme
        // tavanı içinde kapanır).
        let mut g = guard();
        g.on_sent(0, 0, 1172);
        g.on_sent(1, 500 * MS, 1172);
        let arr = [300 * MS + 96 * MS, 500 * MS + 300 * MS + 96 * MS];
        let field = window_field(&arr, &[0, 1]);
        assert_eq!(
            g.on_first_window(&[0, 1], field),
            Some(FirstWindowOutcome::NoInfo)
        );
        assert_eq!(g.pace_bps(), PROFILE);
    }
}
