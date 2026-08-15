# D0 ONAY + REVİZYON TALİMATI — pre-converge-v1.md

> **Kimden:** Abkarada. **Statü:** D0 KOŞULLU ONAY — aşağıdaki revizyonlar D0'a işlenir,
> revize diff bana gösterilir (ham-D0 → revize-D0 farkı ayrıca görünür olacak; hangi değişikliğin
> karardan hangisinin review'dan geldiği izlenebilir kalsın), ONAYIMDAN SONRA D1 başlar.
> **İş sırası değişmedi:** revize-D0 onayı → D1 spec patch → D2+ kod. Otorite sırası aynen.

---

## 1. §9 açık maddelerine kararlar

### Karar 1 — B-maddesi: **B-opt1 + keyed-token/silent-responder** (birleşik model)

B-opt1 (akış-bağlı) doğru ama tek başına eksik: on-path bir gözlemci, gördüğü akışın 5-tuple'ını
spoof'layıp akış-içi probe enjekte ederek echo alabilir → "Kama endpoint" teyidi. Bu yüzden iki
savunma BİRLEŞİR:

- **Akış-bağlılık (B-opt1):** bağımsız/kör tarama probe'una cevap YOK.
- **Proof-of-knowledge token:** PROBE payload'ına, `hub_noise_x25519_pub`'dan türetilmiş anahtarla
  üretilen kısa keyed tag (truncated MAC, ~16B öneri — boyutu D0-revizyonunda gerekçelendir) girer.
  Responder tag'i doğrular; **geçersiz/eksik tag'e — akış içinde bile — TEK BYTE cevap yok**
  (silent-responder; Shadowsocks-sonrası obfs4/V2Ray dersinin uygulanması). Tarayıcıya port ölü
  görünür.
- **SoC koruması (zorunlu kurgu):** token'ı transport ÜRETMEZ. Güvenlik katmanı probe-token'ı
  türetip transport'a **opak blob** olarak verir; transport için kimliksiz byte'lar. INV-ENGINE-SOC
  paragrafı D0'a bu cümleyle girer. Anahtar türetimi (HKDF etiketi vb.) güvenlik-katmanı spec'inin
  konusu — D0'da yalnız arayüz tanımlanır ("opak N-byte token, güvenlik katmanından"), türetim
  detayı D1'de ccr/identity tarafına cross-ref edilir.
- **Replay:** tag'e kaba freshness (probe_seq bağlama; gerekirse geniş zaman penceresi) — echo
  idempotent olduğundan replay zararı düşük, pencere ucuz sigorta. D0'a bir cümle.
- **Amplification yeniden hesap:** token'la probe başı büyür (~11+16=27B) → echo'yu (17B) zaten
  domine eder; pad formülü aynen kalır, örnek sayılar güncellenir.
- **B-opt2** (DATA-ayırt-edilemez şekillendirme) Beta+ notuna; **B-opt3** reddedildi (probe kalıyor).

### Karar 2 — C-maddesi: **minimal ACK-cadence D2 co-requisite'i — EVET**

TV-COLDSTART-1 D2'de kalır. Sınır: D2'deki hal MİNİMAL (tek formül: `ACK_CADENCE ≥
sack_wire/(Y × return_bw_önseli)`, arrival-rate ile hızlanma); FULL adaptif decimation
(reverse-path-impaired N↑ eskalasyonu, per-profil budget) ADIM-6 ana gövdesinde. Dilim şişmez.

### Karar 3 — §4.1: **pencere-oranı ONAY; V2 frame_type 0x11 ONAY**

Geriye-uyumlu versioning (0x10 korunur) doğru. `arrival_rate` u16 B/s tavanı 65535 B/s ≈ 524 kbps —
üst-bant akışlarında doyar; D0-revizyonunda alan semantiği "doygunlukta 0xFFFF = 'ölçüm üst-bantta,
cold-start ipucu olarak anlamsız'" olarak tanımlansın YA DA birim log-ölçekli/kademeli kodlansın —
iki seçenek yaz, birini öner. (Cold-start amaçlı yeterlilik ölçüt; hassasiyet değil.)

### Karar 4 — §4.2 probe byte-layout: **yapı ONAY, token alanıyla REVİZE**

`flag + probe_seq + nonce(8B) + token(~16B, opak) + pad(türetilmiş)`. Pad-türetme kuralı
(echo'dan otomatik ≥) aynen. ECHO formatı değişmez (token yansıtılmaz). Revize taslak
ONAY BEKLİYOR işaretiyle bana tekrar gelir — byte-layout kuralım değişmedi.

### Karar 5 — [CAL] aralıkları: **formüller ONAY; sayılar PLACEHOLDER**

`k_base` 1.3-1.5, `Y` 0.20-0.30, `Z` 0.10-0.20 başlangıç hipotezi olarak tabloda kalır ama
statüleri açıkça "[CAL] — placeholder, netem lowband baseline'ından kalibre edilecek" olur.
Golden vektörlere yalnız ÖLÇÜLMÜŞ değer işlenir (mevcut kural). Kalibrasyon-domaini sütunu her
satırda dolu olacak — "tüm bw" yazan satırlar için formülün 4 decade boyunca geçerliliği
kalibrasyonda ayrıca doğrulanır (0.25k, 1k, 100k, 1M noktaları).

### Karar 6 — A-maddesi: **RETRY_BUDGET deadline-türevi ONAY — iki ek koşulla**

D0 §3.A'nın kurgusu ("mesajı bırakmak değil alternatif yola taşımak", "deadline dolmadıkça mesaj
düşmez") doğru. Eklenecek iki cümle:
1. **PATH_DEAD = yol-vazgeçişi, teslim-vazgeçişi DEĞİL** — failover'a eskalasyon; mesajın
   kuyruktan düşmesi yalnız deadline'la olur. (D0'da imada var, normatif cümle olsun.)
2. **Tek-yol durumu:** failover edecek alternatif hub YOKSA PATH_DEAD davranışı = geometrik
   kuyruğun ucunda düşük-tempolu devam; TAM SESSİZLİK YASAK (blackout-survival vaadi;
   TV-COLDSTART-5 bu durumu da kapsasın — "tek-yol + echo'suz" varyantı eklensin).

---

## 2. D0'a eklenecek yeni bölümler (kararlardan bağımsız, önceki turların borçları)

### 2a. Alpha DPI tehdit-modeli paragrafı (yeni alt-bölüm, spec'e D1'de taşınacak)

Üç maddelik SINIRLI iddia — aynen:
1. Wire'da plaintext invariant/magic yok; Noise sonrası payload'lar ciphertext-random.
2. Aktif taramayla keşfedilemezlik: silent-responder (Karar 1) — geçersiz probe'a sıfır cevap.
3. Tam trafik-ayırt-edilemezlik (boy/zamanlama şekillendirme, cover traffic) **Beta+ yol
   haritası** — Alpha İDDİA ETMEZ.
Bağlam cümlesi: QUIC/KCP'nin DPI'da düştüğü yer (1) ve (2) ihlalleri (QUIC Initial tanınabilir,
KCP header çıplak); Alpha bu ikisini kapatır, üçüncüyü vaat etmez. "DPI-dirençli" sıfatı
pazarlama metinlerinde bu üç maddeye referanssız KULLANILMAZ (no-exaggeration kuralı).

### 2b. Backpressure bağlantısı (§2.2'ye bir paragraf)

"Uygulama sunumu sınırsız; wire çıkışı PACER-TEK-KAPI (token bucket §13); kuyruk sınırı §14
(inflight/memory cap + bounded pools). PRE_CONVERGE'de pacer taban-tempoda olduğundan, uygulama
2M birim sunsa bile wire'a taban-bütçe dışında tek byte çıkmaz; kuyruk dolunca backpressure
uygulamaya döner." — "cold-start'ta app floodlarsa ne olur" sorusunun spec cevabı bu.

### 2c. Kapı-yok/fren-var ayrımı (§1'e bir cümle)

"Kapı yok ≠ fren yok: hiçbir şey akışı BLOKLAMAZ (kapı), ama her şey taban-pacing'le ÇIKAR (fren).
QUIC'in hatası freni kapıyla karıştırmaktı."

---

## 3. Revizyon sonrası süreç

1. Revize-D0 diff'i bana (ham→revize farkı görünür; review-kaynaklı değişiklikler işaretli).
2. Onayım → D1 spec patch'leri (D0 §6 haritası + bu talimatın ekleri; eski model CANCELLED
   appendix'e, sessiz silme yok).
3. D2+ dilimleri aynen (D0 §7 TV seti + Karar 6'nın TV-COLDSTART-5 varyantı + Karar 1'in
   negatif-testleri: geçersiz-token'a sessizlik testi, akış-dışı probe'a sessizlik testi,
   amplification ≤ 1 testi).
4. Her dilim: tek değişken + teşhis dekompozisyonu + aynı dekompozisyonla retest — değişmedi.

## Ek A — Karar 1'in gerekçesi (bu tartışmayı görmedin; karar tarihçesi kayda)

Keyed-token/silent-responder modeli, D0 yazılırken sana iletilmemiş bir tasarım turunda doğdu.
Tarihçe ve gerekçe (D0-revizyonunun B bölümüne özet olarak işlensin):

1. **İki tehdit sınıfı ayrıdır ve farklı savunma ister:**
   - *Pasif DPI* (trafiği izleyip sınıflandırma): savunması protokol-geneli iştir — plaintext
     magic yokluğu, ciphertext-random payload, Beta+ şekillendirme. Probe'un varlığı/yokluğu bu
     cephede marjinal.
   - *Aktif tarama* (trafik gözlemi gerektirmez): düşman IP uzayını probe'layarak cevap veren
     endpoint'leri İHBAR ETTİRİR. Emsal: GFW'nin Shadowsocks sunucularını pasif DPI'la değil,
     aktif probing'le yakalaması — cevap veren her sunucu kendini ele verdi. Sonraki nesil
     (obfs4/V2Ray ailesi) standart cevabı: proof-of-knowledge'sız isteğe SESSİZLİK.
2. **Senin B-opt1'in (akış-bağlı) kör taramayı çözer ama on-path saldırganı çözmez:** akışı
   gören gözlemci 5-tuple'ı spoof'layıp akış-içi probe enjekte eder, echo alır → "Kama endpoint"
   teyidi. Token bunu kapatır (saldırgan tag üretemez). İki savunma bu yüzden BİRLEŞİK.
3. **Reddedilen alternatifler (kayda):** (a) probe'u tamamen atmak / default-kapalı — dar kanalda
   öğrenme-hızı avantajı (≈50B probe vs 256B+50B veri birimi) kaybedilir; ayrıca DPI direnci
   kazandırmaz, wire'ın geri kalanı zaten sınıflandırma yüzeyi. (b) B-opt2 tek başına —
   şekillendirme fingerprint'i azaltır ama aktif-taramaya cevap vermeye devam eder.
4. **Anahtar zaten dağıtımda:** KAT-4 patch'i sonrası her meşru client HubListRecord'dan
   `hub_noise_x25519_pub`'ı bilir → proof-of-knowledge için yeni dağıtım mekanizması GEREKMEZ.
   Token türetimi güvenlik katmanında; transport'a opak (INV-ENGINE-SOC).
5. **Metafor (nota girmesin, sana çerçeve):** açık zil → herkes çalar (D0-öncesi tasarım);
   zili sökmek → işlevi kaybedersin (reddedildi); parolalı zil → bilene tam hizmet, bilmeyene
   zilin varlığı bile görünmez (karar).

## 4. Bu talimatta YASAK olanlar
- Kararları "iyileştirerek" genişletmek — kapsam bu belgede yazandır.
- Token türetim detayını transport spec'ine gömmek (yalnız opak arayüz; türetim güvenlik-katmanı
  tarafında, cross-ref).
- Onaysız byte finalize etmek (Karar 4 revize taslağı dahil — tekrar bana gelir).
- v1 killer-chart belgesine dokunmak (D maddesi/addendum ayrı iş, bu görevin dışında).
