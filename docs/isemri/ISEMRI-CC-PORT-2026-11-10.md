YOL3-CC BAŞLA

# ISEMRI-CC-PORT — Kama taşıma motorunun hız/tıkanıklık denetimini quinn'in BBR + RFC 9002 mekanizmalarıyla değiştirmek

> **Depo sürümü** (2026-11-10; KK-45 madde 1 emsali, §0a): yazan satırındaki kişi adı çıkarıldı, kredi tavanının değeri iki satırda (§0 tablosu, §8 rapor kalıbı) yer tutucuyla değiştirildi; başka satır değişmedi. `~/.cargo/registry/...` kişisel makine yolu değil, araç yolu olarak kaldı. Bayt-özdeş özgün (v2) depo dışında saklanır: sha256 `2a800f420da86852c30114921c86ffbb2a8771472ae224acab62b7a960efbdea`. Bu belge kayıt içindir; uygulanması sahibin ayrı oturumu başlatmasıyla ("YOL3-CC BAŞLA").

Yazan: sahip, Sal 10 Kas 2026, **v2** (dış gözden geçirme sonrası altı düzeltme: kayıp filtresi deney olarak ayrıldı, CC-M0 muhasebe tablosu, düşük bant deney olarak ayrıldı, varsayılan motor değişimi Ö-CC sonrasına, motor başına test matrisi, kabul ölçütleri tanımlandı). Bu iş emri **kendi başına yeterlidir**: sohbeti ve depo dışındaki belgeleri görmezsin; gereken her karar ve ölçüm burada. Depodaki belgeleri klondan okursun. Çelişkide sıra: **bu iş emri > `docs/design/QUINN-FARKI-KOD-KARSILASTIRMA-2026-11-09.md` (analiz girdisi, normatif değil) > mevcut kod yorumları.** Spec'lerle (`specs/*`) çelişen her nokta §7'deki sahip kararıyla çözülür; spec metni bu iş emrinin söylediği kadarıyla, üstü çizili + tarihli YAMA KAYDI ile değişir.

---

## 0. Kimlik

| alan | değer |
|---|---|
| İş emri | ISEMRI-CC-PORT (ajan-CC) |
| Başlatma dizgisi | sahibin mesajında birebir **"YOL3-CC BAŞLA"** |
| Çalışma yeri | **bulut** Claude Code oturumu (kendi konteynerin, kendi `cargo`'n; netem/tc/adb yok). Ölçüm bu oturumda **yapılmaz**; ölçüm yerel kadroda (Ö) koşar. |
| Model | Opus |
| Kredi tavanı | iş emrinin özgününde (depoya yazılmaz, §0a); her raporda kümülatif değer |
| Depo | `https://github.com/Roftcore-Tech/kama-core` (**herkese açık** — PR, rapor, commit metnine kişi adı, makine yolu, bakiye, müşteri adı yazılmaz) |
| Taban | oturum açılışındaki `origin/main` (≥ d2fcca96). Motor `crates/adaptive-transport/**` |
| Toolchain | 1.96.0 + `--locked` |
| Dal | tek dal **`cc/port`** (main'den). Alt dallar yok. Saatlik commit + push. main her gün dala merge; rebase/force-push yok |
| Merge | **sen merge etmezsin.** PR'ı sahip birleştirir ("MERGE <PR#>"); kapılar §8 |
| quinn kaynağı | `quinn-proto 0.11.18` (Cargo.lock'taki sürüm). Kaynak `~/.cargo/registry/src/*/quinn-proto-0.11.18/`; yoksa `cargo fetch` sonrası aynı yol. Lisans Apache-2.0 / MIT: alınan her parçanın başına dosya:satır + lisans atfı yazılır. **quinn ürün bağımlılığı olarak eklenmez**; kod taşınır |

Klon ve hazırlık:
```bash
git clone https://github.com/Roftcore-Tech/kama-core.git kama-core && cd kama-core
git fetch origin && git checkout -b cc/port origin/main
rustc --version && cargo metadata --locked --format-version 1 >/dev/null
cargo build --locked -p adaptive-transport
ls ~/.cargo/registry/src/*/quinn-proto-0.11.18/src/congestion/bbr/ || cargo fetch
```

---

## 1. Neden (ölçülmüş; `reports/yol3/2026-11-09`, `reports/yol3/2026-11-10`, `reports/ktp2/f0/ACIL-INCELEME-2026-11-03.md`)

Kama'nın hız/faz makinesi BBR'ı yeniden yazıyor ve beş yerde ölçülebilir biçimde kötü çalışıyor. Hepsi aynı kökten: 0,025 kbps köşesi için konmuş sabitler (31 B/s dönüş hattı önseli) her rejimde çalışıyor, prob kayıpla kesiliyor, kayıp hiçbir koşulda hızı etkilemiyor, Drain fiilen yok.

| # | belirti [ölçüldü] | kök [kod] |
|---|---|---|
| 1 | Kayıp ilanı ≥ 3,15 s, RTT'den bağımsız (quinn: 0,34 s @ 300 ms). İlan→retx p50 4,08 s | `sender.rs:3402-3412` EAI = 2·birim/rate + 65 B/31 B/s; taban 1,5·EAI (`loss.rs:685`) |
| 2 | Alıcı boşlukta anında ACK üretmiyor; zamanlayıcı SACK 8,39 s | `receiver.rs:255-258`, `:93` |
| 3 | Manşette (%30 kayıp, 12,5 kB/s) gönderim/gereken 0,55; prob sayısı 0 | `sender.rs:3993-3996` bekleyen kayıp probu keser; `:4076` atıflı kayıpta abort; `:4072-4075` başarı testi 1,025× |
| 4 | **6 paketlik kuyrukta çöküş: dosya 91 s, 10 koşunun 4'ünde teslim yok (quinn-bbr-rfc 16,4 s)** | Startup 2,885 kazancıyla taşıyor; Drain çıkış koşulu uçuş < 2·BDP (sığ kuyrukta zaten sağlanıyor, boşaltma olmuyor); kayıp (kuyruk taşması dahil) uçuş miktarını hiç daraltmıyor; `tail_recovery.rs` 3 prob sonra pes |
| 5 | 26,45 kB/s altında parite tele çıkmıyor | `sender.rs:2366-2381` kova tavanı front'a düşüyor; `pacer.rs:118` `size > cap → None` |

Hız üstünde üç ayrı otorite var: faz makinesi (`sender.rs` ~3985-4330), fuzzy throttle (`FusionDecision.send_rate_gain_permille` → `rate.rs:170-175`), cwnd/srtt tavanı (`rate.rs:143-150`). quinn'de bir tane var. Gösterim motoru (`yol2/goster`: K1 + A-1…A-6 + şeritler) derin kuyruk profilinde ölçütleri sağlıyor ve **dondurulmuştur; bu iş ona dokunmaz.** Bu iş, Aralık PoC ölçümünün (Ö-CC) motorudur.

## 2. Amaç

Kama motorunun hız/tıkanıklık denetimini **düzeltmek değil, değiştirmek**: quinn-proto 0.11.18'in BBR durum makinesi (Startup / Drain / ProbeBW gain döngüsü / ProbeRTT), `recovery_window`, teslim hızı tahmini (max-filtre, app-limited işaretleme), pencere tabanlı pacer ve RFC 9002 kayıp tespiti / PTO / ACK kuralları Kama'nın `adaptive-transport` motorunda **tek otorite** olur — önce `cc_port` bayrağı altında (PR-1, bu iş emri), ölçüm geçince varsayılan olarak (PR-2, ayrı). Kama'dan kalanlar yalnız §3'te sayılan, gerekçesi yazılı parçalardır. Yeni mekanizma icat edilmez; quinn'de ne varsa o, Kama'nın birim/çerçeve modeline uyarlanarak. Kama'ya özgü iki uyarlama (kayıp filtresi, düşük bant) **deney bayrağıdır**, portun parçası değildir.

## 3. Ne kalır, ne gider

**Kalır (Kama'ya özgü, gerekçeli):**
- Birim modeli, mühürlü tel, sınıf şeritleri (`client-core/src/lanes.rs`), oynatma tamponu — CC'nin dışında, dokunulmaz.
- FEC bütçesi ve profil kararı (`budget.rs`, `fusion.rs`'in FEC/ARQ çıktıları) — CC'nin dışında; CC yalnız "teslim hızı" ve "uçuş tavanı" verir, FEC bütçesi bu değerlerden hesaplanır (parite baytı gönderim sayımına girer). **`FusionDecision.send_rate_gain_permille` hız yolundan ÇIKAR** (`rate.rs:170-175` çarpanı kalkar; alan korunur, 1000 sabit, yorumla "CC-PORT ile pasif").
- Determinizm (ADR-17): quinn'in rastgele gain-döngüsü faz ofseti alınmaz, faz 0'dan başlar; `f32` kullanılmaz, sabit nokta (‰) kalır.
- Düşük bant hedefi korunur ama **bu portun parçası değil, ayrı deneydir** (`cc/lowband.rs`, bayrak `cc_lowband`, varsayılan kapalı; M4). Sebep: dönüş yönü (ACK) kapasitesi ileri yön teslim hızından türetilemez (asimetrik hat); quinn'de de böyle bir ölçüm yok. Kipte hangi büyüklüğün kullanılacağı CC-M0'da çözülür: belgelerde iki ayrı sayı dolaşıyor — **0,025 kbps = 3,125 B/s** ve **31 B/s ≈ 0,25 kbps** — eşleme belgesi her birinin hangi yönü (ileri/dönüş) ve hangi spec satırını anlattığını yazar; ikisi birbirinin yerine kullanılmaz. Varsayılan motor bu kip olmadan, quinn kurallarıyla koşar; 0,025 kbps hücresi Ö-CC'de ayrı ölçülür.
- E2a alt kümesi (m1 zaman eşiği, m8 RTT kuralı, m6 ilk pencere yarışı, PTO 2 prob; main'de `a8721540`) ve A paketi bayrakları (`yol3/q1`, `yol3/q2`; gösterimde açık) — bunlar bu portun **öncülleri**; `cc_port` açıkken bu bayraklar anlamsızlaşır (davranış quinn'inki olur), legacy'de olduğu gibi kalır. Hiçbir bayrak PR-1'de kaldırılmaz.

**Gider (yerine quinn):**
- `sender.rs` faz makinesi (PreConverge/Startup/Drain/Steady/ProbeUp/RecoverArq/RecoverFwd/DrainRtprop; ~3985-4330) → BBR `Mode` + `RecoveryState` (`congestion/bbr/mod.rs`).
- ProbeUp giriş/çıkış/başarı/abort mantığı (`:4060-4139`) → gain döngüsü (`bbr/mod.rs:160-199`), başarı testi yok.
- Startup plato kuralı (`:4208-4235`) → `check_if_full_bw_reached` (`bbr/mod.rs:373-391`; app-limited turlar sayılmaz, 3 tur < %25).
- Kalkış: PreConverge 31 B/s tabanı ve `grounded_startup_rate` → ilk pencere **10·MTU** (RFC 9002; quinn-Cubic `cubic.rs:266`), pencere tabanlı pace (`pacing.rs:92-95`: token = cwnd·1,25·geçen_RTT). **240 kB BBR ilk penceresi ALINMAZ.**
- Kayıp zaman eşiği tabanı (`loss.rs:667-703` cadence_floor) → `max(9/8·max(srtt, latest_rtt), 1 ms)` (`connection/mod.rs:1700`); paket eşiği 3 kalır.
- PTO: çapa son ack-eliciting gönderim, 2 prob, tavan yok (üstel backoff), `tail_recovery.rs`'in 3-prob tavanı ve 30 s tavanı kalkar (`connection/mod.rs:1834-1870`).
- ACK üretimi: her 2 ack-eliciting datagram **veya** sırasız paket → anında ACK; zamanlayıcı max_ack_delay 25 ms (`spaces.rs:718-725`, `:674-676`); 8,39 s cadence kalkar (düşük bant kipi hariç).
- Kova tavanı: `clamp(cwnd·2 ms/rtt, 10·MTU, 256·MTU)` (`pacing.rs:132-139`); `size > cap → None` yolu ürün hızlarında imkânsız olur.
- Yeniden gönderim sırası: retx önce (`send_buffer.rs:93-113`; spec §13 ile aynı), `K_ARQ_SHARE` kalkar.
- `arq_retx_cap` (3) ve `HandToForward` yolu: tavan kalkar; RecoverFwd (ileri kopya) yalnız FEC politikasının işi olarak `budget.rs` tarafında kalır, CC durumu olmaktan çıkar.
- cwnd: BBR'daki gibi **bayt** cinsinden uçuş tavanı (`inflight ≤ cwnd`), `rate.rs:143-150`'nin rate-formu kalkar. `min_cwnd` 4·MTU kalır.

**Kayıp ↔ hız — iki davranış, aynı adaptör üstünde, ayrı ayrı ölçülür:**
- **Referans (varsayılan):** quinn'in kayıp olayı işleme davranışı **olduğu gibi**: her kayıp olayı `on_congestion_event` → `recovery_window` (`bbr/mod.rs:128-156, 339-367`). Portun "quinn'i taşıma" iddiası bu davranışla ölçülür; buna kapı konmaz.
- **Deney (bayrak `cc_loss_filter`, varsayılan kapalı):** Kama'nın gecikme koşullu kayıp filtresi: `on_congestion_event` yalnız `srtt − min_rtt ≥ max(QD_EŞİĞİ, 0,25·min_rtt)` iken çağrılır (QD_EŞİĞİ = 50 ms, aday). Tek fonksiyon `loss_class.rs::is_congestive(sample) -> bool`, birim testli. **Bu filtre ürün kuralı DEĞİLDİR:** `srtt − min_rtt` kaybın nedenini tek başına belirlemez (kısa kuyruk taşması yumuşatılmış RTT'ye yansımayabilir; yol gecikmesi değişimi kuyruk gibi görünebilir). Birim testinde üç etiketin beklenen boolean'a dönmesi, gerçek ağda sınıflandırmanın doğru olduğunu kanıtlamaz. Ö-CC'de referansla **aynı hücrelerde** yan yana ölçülür; sahip ancak ölçümden sonra INV-RATE-1'i değiştirir (§7).

## 4. Yapı

Yeni dosyalar (`crates/adaptive-transport/src/`): `cc/mod.rs` (trait `CongestionController { on_sent, on_ack, on_loss, window, pacing_window, mode }`), `cc/bbr.rs` (quinn portu; atıflı), `cc/bw_filter.rs` (`min_max.rs` portu; Google BSD atfı korunur), `cc/loss_class.rs` (§3 deney filtresi), `cc/lowband.rs` (düşük bant deneyi).

**Bayrak yönü — varsayılan ESKİ motor:** cargo özelliği **`cc_port`** (varsayılan **kapalı**). Kapalıyken `sender.rs`'teki mevcut faz makinesi bit-eşit çalışır (main'in davranışı değişmez). Açıkken trait üzerinden `cc/bbr.rs` çalışır. Alt deneyler `cc_loss_filter` ve `cc_lowband` yalnız `cc_port` ile birlikte anlamlı. Mevcut kod silinmez, `#[cfg(not(feature = "cc_port"))]` arkasına alınır. **Böylece `cc/` modülü main'e eski davranışı koruyarak girebilir; varsayılan motorun değişmesi ayrı PR'dır ve ancak Ö-CC aynı son aday SHA üzerinde geçtikten sonra olur (§5, §7).**

**Test matrisi (her raporda motor adıyla):**
| koşu | komut | ne kanıtlar |
|---|---|---|
| legacy | `cargo test -p adaptive-transport` | mevcut golden/`validated_v1` bit-eşit |
| port | `cargo test -p adaptive-transport --features cc_port` | yeni motor; faz geçişleri, RFC 9002 testleri |
| port+filtre | `… --features cc_port,cc_loss_filter` | `loss_class` birim testleri |
| ortak doğruluk | `tests/common_correctness.rs`, iki motorla da | güvenilir teslim, yük sahipliği, sıralama, SACK tekrarı — motordan bağımsız doğruluk |

`--all-features` iki motoru birlikte açamaz (özellikler `compile_error!` ile dışlanır); **`--all-features` tek başına port kanıtı sayılmaz.** Her test raporu çalıştırdığı motoru yazar. Gösterim ağacı etkilenmez (ayrı dal).

Tel değişmez. `unit_id`, SACK biçimi, PARAM_SET, kripto, S1/S3 seam'leri değişmez. `kama-contracts/src/{s1,s3,handles}.rs`, `kama-noise-wire/**` dokunulmaz.

## 5. Sıra ve kilometre taşları

| # | iş | DONE |
|---|---|---|
| CC-M0 | okuma + eşleme belgesi `docs/design/CC-PORT-ESLEME.md`, **üç bölüm:** (a) fonksiyon eşlemesi — quinn fonksiyonu → Kama'da yerine geçeceği fonksiyon → veri yapısı farkı → uyarlama notu; (b) **paket/birim muhasebesi** — aşağıdaki tablo; (c) referans BBR ile deney filtresinin ayrımı + motor başına test matrisi (§4). Düşük bant iki sayısının (§3) yön ve kaynağı da burada | belge PR'da (docs); sahip okur, "CC-M0 TAMAM" der, kod başlar |
| CC-M1 | `cc/` modülü + trait + `cc_port` bayrağı; `sender.rs` CC çağrılarını trait'e yönlendirir; bayrak kapalıyken bit-eşit | legacy koşusu yeşil (mevcut tüm testler), port koşusu derlenir |
| CC-M2 | BBR portu: teslim hızı, Startup/Drain/ProbeBW/ProbeRTT, recovery_window (referans davranış), pencere pacer, IW 10·MTU; fuzzy throttle pasif; muhasebe (b) kodda | birim testleri: her faz geçişi için deterministik senaryo; muhasebe tablosunun her satırı için bir test |
| CC-M3 | RFC 9002 kayıp/PTO/ACK: zaman eşiği, PTO 2 prob backoff, anında ACK, 25 ms zamanlayıcı, retx önce, kova tavanı; `loss_class` deney bayrağı | birim + E2a testleri (m1/m8/m6) port koşusunda yeşil; `loss_class` üç vaka (rastgele / kuyruk / karışık) filtre koşusunda |
| CC-M4 | FEC bütçesi arayüzü (`budget.rs` CC'den `delivery_rate`/`cwnd` okur) + düşük bant deneyi (`cc_lowband`) | `fec_loss_risk` ve FEC bütçe testleri iki motorla yeşil; düşük bant birim senaryosu deney koşusunda |
| CC-M5 | **PR-1 "cc/ modülü, varsayılan eski motor":** spec'e yalnız "CC-PORT deneysel, bayrak arkasında" notu; INV değişikliği YOK; EVO/INVARIANTS beyanları; PR "hazır" | yerel kapı (§8) + hosted CI; sahip MERGE. **Varsayılan motor değişimi (PR-2) ve INV-RATE-1 yaması bu iş emrinin dışında, Ö-CC aynı SHA'da geçince** |

**CC-M0 (b) muhasebe tablosu — her satır eşleme belgesinde cevaplanır, M2'de birim testine bağlanır:**
| olay | açıklanması gereken |
|---|---|
| bir birimin birden fazla iletimi | hangi iletim ACK'lendi sayılır; hangi uçuş kaydı kapanır (quinn: paket numarası benzersiz; Kama: `unit_id` tekrar iletimde aynı — uçuş kaydı iletim başına mı, birim başına mı?) |
| geç veya tekrarlanan SACK | teslim edilen bayt ikinci kez sayılmadan nasıl işlenir (teslim hızı örnekleyicisi yalnız ilk ACK'i sayar) |
| FEC ile kurtarma | kaynak birimin tamamlanması ≠ paketin teslimi; teslim hızı hangi olaydan beslenir; kurtarılan birim "kayıp" sayılır mı |
| parite ve prob paketleri | uçuş miktarına, teslim hızına ve kayıp hesabına nasıl girer (bayt sayımı, ack-eliciting mi) |
| ACK üretimi | hangi kayıt ACK tetikler; ACK'e ACK döngüsü nasıl önlenir; SACK'in kendisi uçuşa girer mi |
Bunlar yanlış bağlanırsa doğru BBR kodu da yanlış bant ve pencere üretir; CC-M0 onayı bu tabloya bağlıdır.

**Sıra (eşzamanlılık yüzünden M2 M1'den önce):** CC-M0 Çar 11 → **CC-M2 önce, bağımsız modül olarak** (`cc/` altında, `sender.rs`'e bağlanmadan, kendi birim testleriyle; Per 12 – Cmt 14) → CC-M3'ün alıcı/ACK tarafı ve `loss_class` (Paz 15, yine `cc/` içinde) → **Pzt 16 12:00'den sonra** CC-M1 bağlama (`sender.rs` trait'e yönlendirme + legacy bayrağı, Pzt 16) → M3'ün gönderici tarafı (Sal 17) → M4–M5 (Çar 18). Pzt 16 12:00'ye kadar `sender.rs`, `loss.rs`, `receiver.rs`, `pacer.rs`, `rate.rs`'e **tek satır** dokunulmaz (§9). **Dal Paz 15 20:00 dondurmasından bağımsızdır** (gösterim ağacına dokunmuyor); main'e giriş sahibin MERGE'iyle, en erken Pzt 16. **Ölçüm (Ö-CC)** yerel kadroda, bu iş emrinin dışında; PR-1'in son aday SHA'sı üzerinde, `cc_port` açık/kapalı ve `cc_loss_filter` açık/kapalı olarak **üç kol** (legacy = K0, referans port, port+filtre), her hücre eşleştirilmiş koşu (aynı kökte, sırası değişen). Hücreler ve ölçütler (metrik adı açık):
| hücre | metrik | referans port için kabul |
|---|---|---|
| 6 paketlik kuyruk, dosya 150 kB | tamamlanma süresi p50; teslim sayısı | ≤ 1,5 × quinn-bbr-rfc aynı hücrede; 10/10 |
| manşet `a_r12k_d300_l30_dosyali` | gönderim/gereken (gönderilen bayt / hattın o sürede taşıyabileceği bayt) | ≥ 0,9 |
| manşet, aynı koşu | faydalı/gönderim = uygulama katmanında ilk kez teslim edilen yük baytı / tele yazılan toplam bayt (başlık, parite, tekrar iletim dahil), tüm koşu | **ön kayıt adayı, bağlayıcı değil**: %30 kayıpta fizik üst sınırı 0,70'tir (1 − kayıp), FEC ve başlıkla gerçekçi değer daha düşük; K0'ın aynı hücredeki değeri ölçülür, eşik ona göre Ö-CC'de konur |
| r25 (l25, d300) | tamamlanma süresi p50 **ve** teslim sayısı | p50 ≤ 1,10 × K0 (eşli medyan); teslim sayısı ≥ K0 |
| d1000 l0 | tamamlanma süresi p95 | ≤ 1,1 × K0 |
| şeritli PTT (K1 profili) | ilk demet gecikmesi | ≤ 3 s, 10/10 |
| 0,025 kbps hücresi | teslim var/yok; ilk TEXT süresi | yalnız raporlanır (deney kipi) |
Filtre kolu için ayrı kabul yok: aynı hücrelerde referansın yanında raporlanır; sahip sonuca göre INV-RATE-1'e karar verir. Ortam yüzünden koşamayan zorunlu kontrol uygun ortamda tamamlanır; "ortam" etiketi geçiş sonucu değildir. Hücreler KAPI-MANİFESTİ'ne bu iş emriyle girer.

## 6. Yasaklar

- Yeni mekanizma icat etmek, quinn'de olmayan sabit/eşik eklemek (§3 kayıp sınıflandırma ve düşük bant kipi dışında; ikisi de sahip kararı).
- `yol2/goster`, `yol3/*` dallarına dokunmak; gösterim profil satırını, `transport-v1.x-goster` kimliğini değiştirmek.
- Tel, vektör, S1/S3, kripto, `kama-app`, `client-core` değişikliği. FEC kodek/bütçe algoritmasını değiştirmek (yalnız girdi arayüzü).
- quinn'i bağımlılık olarak eklemek; yeni harici crate; `f32`; RNG.
- "QUIC'i yendik", "kanıtlandı", "eşdeğer" demek; koşmamış teste "geçti" demek.
- Ölçüm koşturmak (netem yok).
- Erişilebilir `todo!()`; silinen veya üstü çizilen spec satırı (PR-1'de yalnız YAMA ADAYI notu).
- `cc_port`'u varsayılan yapmak, `Cargo.toml` `default` özelliklerine eklemek, mevcut CI işlerini değiştirmek. Kayıp filtresini veya düşük bant kipini referans porta karıştırmak.
- Test raporunda motoru yazmadan "geçti" demek.

## 7. Sahip kararları (imzalı; spec yamaları bu metinle)

- **INV-RATE-1 bu iş emriyle DEĞİŞMEZ.** Mevcut metin kalır; `cc_port` motoru bayrak arkasında olduğu için main'in varsayılan davranışı INV-RATE-1'i ihlal etmez. Referans port (kayıpta `recovery_window`) INV-RATE-1 ile çelişir — bu çelişki spec'e şu notla yazılır: "YAMA ADAYI 2026-11-10 CC-PORT: `cc_port` bayrağı altında quinn-proto 0.11.18 referans kayıp davranışı deneniyor; Ö-CC ölçümü sonrası sahip kararıyla INV-RATE-1 ya korunur ya şu adayla değişir: '<§3 deney filtresinin metni>'." Üstü çizili satır YOK; yalnız not.
- Engine spec §5/§6/§13'teki faz makinesi, ProbeUp başarı testi, K_ARQ_SHARE, EAI/31 B/s önseli ve 8,39 s ACK cadence hükümleri: **PR-1'de dokunulmaz**; her birine "YAMA ADAYI 2026-11-10 CC-PORT: bayrak altında yerine quinn <dosya:satır> deneniyor" notu. Üstü çizme PR-2'de (varsayılan değişince).
- ADR-06/ADR-14 "quinn fork YOK" korunur: fork değil, lisanslı kod taşıma; her dosyada atıf. ADR-17 determinizm korunur.
- Fuzzy denetleyici (ADR-17) `cc_port` altında hız yolundan çıkar; legacy'de olduğu gibi kalır; FEC/ARQ politika çıktıları iki motorda da korunur.
- 0,025 kbps hedefi korunur; önsel sabitle değil, ölçülü deney kipiyle (§3). İki sayının yön/kaynak ayrımı CC-M0'da.
- Gösterim (18 Kasım) bu işten etkilenmez. **Varsayılan motor değişimi (PR-2) için GO ancak Ö-CC'nin PR-1'in son aday SHA'sında geçmesiyle; sahip dizgisi "CC-PORT VARSAYILAN <SHA>".**

## 8. Kapı ve rapor

Son aday kapısı: `scripts/ci/local-gate.sh --ref cc/port` (konteynerde root/IPv6/deny kaynaklı kırmızılar "ortam" diye ayrı yazılır; bench crate'i IPv6 yüzünden koşmaz) + §4 test matrisinin dört koşusu ayrı ayrı (legacy / port / port+filtre / ortak doğruluk iki motorla) + `cargo test -p kama-engine` + `cargo test -p client-core` (bayraksız; legacy). Hosted CI PR'da koşar (11 iş); CI'ya `cc_port` koşusu ayrı iş olarak eklenir (`.github/workflows/ci.yml`'de yeni iş; mevcut işler değişmez).

Rapor (≤ 40 satır, PR yorumu ve her tur sonu):
```
RAPOR CC — <tarih saat> — taban origin/main <SHA> — dal cc/port @ <SHA|YEREL>
KİLOMETRE: CC-M0 <…> · M1 · M2 · M3 · M4 · M5
ALINAN quinn PARÇALARI: <dosya:satır → Kama dosya:satır> (her tur yeni eklenenler)
KALAN KAMA PARÇALARI (gerekçeli): <liste>
TEST: <komut → sonuç>   KOŞMAYAN: <komut — neden>
SPEC YAMASI: <dosya § → YAMA ADAYI notu> (PR-1'de üstü çizme yok)
KARAR-KAYDI (yeni): <satır|yok>
KREDİ: <x> / <tavan> oturum-saat (yalnız raporda, PR gövdesinde değil)
RİSK: <tek satır>
SÜRDÜRME: <sıradaki ilk komut>
```
Boşlukta DUR açılmaz; `KARAR-KAYDI:` satırı yazılır, iş sürer. Sahibin cevabı gereken tek nokta CC-M0 tablosudur ("CC-M0 TAMAM").

## 9. Eşzamanlılık

Aynı depoda: gösterim ağacı `yol2/goster` (dondurulmuş; dokunma), hub istemcisi oturumları HA/HB1/HB2 (`client-core/src/hub/`, `kama-node`, `kama-provision`, `kama-contracts/src/s4.rs`; dokunma), yerel M1/M2/T1/Ö (`adaptive-transport` L/P bölgeleri — **Pzt 16 12:00'den sonra bu bölgelerde yalnız sen çalışırsın**; o saate dek `sender.rs`/`loss.rs`/`receiver.rs`/`pacer.rs`/`rate.rs`'e dokunma; `cc/` yeni dosyalarında, `tests/` altında kendi test dosyalarında ve eşleme belgesinde çalış. Pzt 16 12:00'de sahip "YOL: …" kararını verdikten sonra önce `git merge origin/main`, sonra bağlama). Her gün `git merge origin/main`. `Cargo.lock` yalnız kendi değişikliğinin doğurduğu satırlar. Çatışmada diğer tarafın satırı silinmez, PR'a `ÇATIŞMA: <dosya>` yazılır.
