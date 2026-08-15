# ADIM-6 / Görev: PRE_CONVERGE Cold-Start Tasarım Notu + Spec Patch + Dilimli Implementasyon

> **Statü:** Onaylı görev tanımı (Abkarada onayı: 2026-07-11).
> **İş sırası ZORUNLU:** (1) tasarım notu → (2) Abkarada diff onayı → (3) spec patch'leri →
> (4) dilimli implementasyon. **Kod, notun önüne geçemez.**
> **Otorite sırası:** transport-layer-tdd.md (WHAT/policy) > adaptive-transport-engine-spec.md
> (HOW/engine) > bu görev belgesi > kod.

---

## 0. Bağlam — neden bu görev var

kbps testleri iki katmanlı kök-neden ortaya çıkardı (ikisi de doğrulandı):

- **Forward:** INITIAL_RTT=100ms + time-threshold loss detection, kbps'te ilk ACK fiziksel olarak
  dönemeden (~1-2s) kayıp ilan ediyor → spurious retransmit fırtınası → temiz 1kbps kanalda 3× byte.
- **Return:** ACK_TICK=20ms → SACK seli (~650 B/s) 125 B/s dönüş borusunu tıkıyor → teyit ~19s gecikiyor.
- **Sistemik:** INITIAL_RTT, ACK_TICK, min_rate, RTPROP_TICK, loss/emergency eşikleri — hepsi
  kbit-domain'e hardcoded; hiçbiri bandwidth-aware değil. Ürün hedefi 0.25 kbps – Mbps (4 decade).

Bu görev, cold-start (motorun hiç ölçümü yokken) davranışını **tasarım seviyesinde** çözer.
Bandwidth-aware ölçekleme yasalarının geri kalanı (ADIM-6 ana gövdesi) ayrı dilimlerdir; bu belge
onlarla çakışan yerlerde referans verir, kapsamı genişletmez.

## 1. Temel ilke — KAPI YOK, TERMOMETRE VAR

Veri akışını hiçbir ölçüm, el sıkışma veya probe BLOKLAMAZ. Motor t=0'da göndermeye başlar;
ölçüm gönderilenin yanında toplanır. QUIC'in p5 ölümünün nedeni handshake'in boyutu değil
**kapı** olmasıydı (baseline: hs=%40-60, udp_tx=0 koşuları). Bu ilke her tasarım kararının
turnusoludur: bir bileşen veri akışını bekletiyorsa tasarım yanlıştır.

## 2. Onaylı mimari — dört bileşen

### 2.1 Davranış önseli: fuzzy en-kötü-köşe
- Motor açılışta fuzzy matrisinin en-kötü senaryosu gibi **davranır**: bw = taban, loss = extrem.
- Pacing bu tabana kurulur → zarf-içi hiçbir kanalda self-congestion inşa gereği imkânsız.
- **KURAL (spec cümlesi olarak girecek):** "PRE_CONVERGE'de motor, fuzzy en-kötü-köşe önseliyle
  DAVRANIR; bu önsel hiçbir estimator'a örnek olarak yazılmaz, ilk geçerli ölçümle koşulsuz
  tahliye edilir ve prob takvimi önselin geçerliliğinden bağımsız sürer."
- Gerekçe (nota girsin): dominance argümanı — kanal dipteyse davranış doğru; kanal iyiyse ilk
  cevap hızlı döner, önsel saniyeler içinde ölür; yanılma cezası kendi kendini sınırlar.
  Bilinçli takas olarak yazılır: yüksek-bant senaryoda ilk-echo süresince suboptimal hız kabul
  edilir, karşılığında self-congestion imkânsızdır.

### 2.2 Veri: programlı geometrik kopya takvimi
- İlk veri birimi t=0'da gider; ACK beklenmez. Kopyalar geometrik takvimle: t=0, ~1s, 2s, 4s,
  8s, 16s… Taban ve çarpan **[CAL]** — serialization fiziğinden türetilir, UYDURULMAZ.
- "Kayıp mı yavaş mı?" sorusu cold-start'ta CEVAPLANMAZ; takvim iki hipotezi birden kapsar.
- RX_DUP_SUPPRESS fazla kopyaları semantik olarak zararsız kılar (mevcut mekanizma).
- Bu, RECOVER_FWD felsefesinin cold-start'a uzantısıdır; tetikleyici "ack_confidence LOW" değil
  "henüz hiçbir ölçüm yok"tur.

### 2.3 Ölçüm: zengin SACK (ana kanal) + 33B probe (hızlandırıcı)
**SACK genişletmesi (ONAY MADDESİ — wire değişikliği, taslak format Abkarada onayına gider):**
- `seen_count`: receiver'ın gördüğü birim/kopya sayısı → yön-ayrımlı kayıp. Tek dönen SACK'ten:
  seq eşleşmesi → RTT; seen_count → forward-loss mı return-loss mu (p5 ayrımı; tepkiler zıt);
  geliş oranı → ilk bw ipucu.
- Receiver-gözlemli geliş oranı alanı (packet-pair aralığı veya pencere-oranı — nota iki seçenek
  yazılır, biri önerilir).

**Probe/echo birimi (ONAY MADDESİ — yeni wire birimi):**
- ~33B: `flag + probe_seq + kısa nonce` payload (+28B IPv4/UDP). Amaç: dar kanalda ucuz öğrenme
  (0.25 kbps'te 256B veri ~8s, 33B probe ~1s serialization → aynı bütçeyle ~8× deneme).
- Verinin aralarına geometrik takvimle serpiştirilir; açılışta mini-burst (ilk saniyede 3-5 adet;
  sayı [CAL]).
- **Statü cümlesi (spec'e aynen):** "Probe hızlandırıcıdır; hiçbir şeyi bloklamaz, receiver'da
  state açmaz, başarısızlığının sonucu yoktur. Yokluğunda sistem yalnız daha yavaş öğrenir."
- Echo: stateless, seq-echo + seen-count + geliş-aralığı; **echo ≤ probe boyutu (amplification
  faktörü ≤ 1)**; responder rate-limit'li (unauthenticated DoS yüzeyi). Katman: Noise'un ALTINDA,
  kimliksiz ölçüm birimi — INV-ENGINE-SOC korunur; bu konum kararı nota açıkça yazılır.

### 2.4 Kademeli tahliye — sinyal başına ayrı kilit
- İlk geçerli echo/SACK → **RTT + loss kilidi açılır** (gerçek örneklerle); ARQ RTT-temposunda.
- **bw kilidi AYRI:** taban varsayımı, ölçülmüş bw örnekleri birikene kadar sürer. Tek echo
  "bant var" der, "ne kadar" DEMEZ → FEC/redundancy agresifliği bw-kanıtıyla kademeli açılır
  (MIN_RATE dersinin kurumsallaşması: bilmediğin banta yüklenme).
- **PRE_CONVERGE yasakları:** loss ilanı YOK; emergency duplication YOK; sessizlikten estimator
  beslemek YOK (sessizlik davranış seçtirir, veri üretmez).
- Hiç cevap gelmezse: sessizliğe düşülmez — geometrik kuyruğun ucunda düşük tempolu süresiz
  devam; F/persistence politikasına bağlanır (deadline-güdümlü, sayı-tavansız).

### 2.5 Tamamlayıcı: operatör önseli
- Config alanı `bandwidth_class`: cold-start tabanını ezer. **Yalnız önseli değiştirir, ölçümü
  ASLA ezemez** — ilk gerçek örnekte çöpe.

## 3. Kapsam dışı — kayda geçecekler
- **Fountain-tarzı init → Beta+ notu.** Gerekçe aynen: tek-paket mesajlarda kopya takvimine
  dejenere olur; gerçek değeri çok-paketli yüklerde (bulk/geniş early_data); INV-FEC-1 katman
  sınırına dokunur, ayrı tasarım turu ister. (Tarihsel not: ölçüsüz bulk-first TCP'de israf
  gerekçesiyle terk edildi; buradaki takvim taban-bütçeli ve geometrik-seyrekleşen olduğundan
  aynı sınıf değildir — bu ayrım nota yazılsın.)
- **Timestamp-tabanlı tek yönlü ölçüm → ELENDİ:** saat senkronu hostile modelde varsayılamaz.

## 4. Spec etkisi
- State machine'e **PRE_CONVERGE** durumu (giriş/çıkış/yasaklar/kademeli tahliye) — §3 + §12,
  numaralı karar.
- §2 SACK formatına iki alan (onay sonrası); probe/echo birimi yeni bölüm.
- Eski INITIAL_RTT / sabit-timeout modeli **CANCELLED** işaretiyle appendix'e — sessiz silme YOK.
- §16 [CAL] tablosuna yeni sabitler + **kalibrasyon-domaini alanı** (hangi kanal aralığında
  geçerli — MIN_RATE dersinin spec karşılığı).
- Fuzzy-önsel kuralı (2.1'deki cümle) + probe statü cümlesi (2.3'teki) spec'e aynen.

## 5. İş akışı ve dilimler
1. **D0 — Tasarım notu** (docs/design/pre-converge-v1.md): yukarıdaki mimari + wire taslakları
   (SACK alanları + probe/echo byte-layout ÖNERİ olarak, "ONAY BEKLİYOR" işaretli) + [CAL]
   sabitlerinin türetme formülleri. → **Abkarada diff onayı. Onaysız 2. adıma geçilmez.**
2. **D1 — Spec patch'leri** (onaylı nota birebir; cerrahi, byte-layout yalnız onaylı haliyle).
3. **D2 — PRE_CONVERGE state + geometrik veri takvimi** (probe'suz; SACK eski formatta).
   TV-COLDSTART-1: temiz 1kbps, 10/10 teslim.
4. **D3 — SACK genişletmesi + kademeli tahliye.** TV-COLDSTART-2: %95 loss'ta tek SACK'ten
   RTT+yön-ayrımlı loss çıkarımı; bw kilidinin AÇILMADIĞI doğrulanır.
5. **D4 — Probe/echo + mini-burst + rate-limit.** TV-COLDSTART-3: hızlı kanal (100kbit+%90 loss)
   ilk saniyelerde yakalanır; TV-COLDSTART-4: 0.25kbps'te probe bütçesi kanal payını aşmaz.
6. **D5 — operatör önseli + F/persistence bağlantısı.** TV-COLDSTART-5: echo'suz koşuda motor
   asla tam sessizliğe düşmez.
- Her dilim: TEK değişken + öncesinde teşhis dekompozisyonu + sonrasında AYNI dekompozisyonla
  retest. "Düzeldi" hükmünü diff değil ölçüm verir.

## 6. Kabul kriterleri (ölçülür, uydurulmaz)
- Temiz 1kbps: 10/10 teslim; tx/payload ≤ hesaplanan-alt-sınır × ~1.15 (alt sınırı header
  fiziğinden HESAPLA, sabit yazma).
- 0.5 / 0.25 kbps: fizik-türevi deadline'larla (k × ideal-kanal-süresi; k değerleri nota)
  pozitif completion.
- Dönüş-yolu (SACK+echo) byte'ı: kanal payının ölçülü üst sınırında (oran [CAL], nota).
- **Üst-bant regresyon YOK:** v1 matrisinin p4/p5 hücreleri yeniden koşulur, 1.00 korunur;
  floor-contaminated hücreler (p7 vb.) ADIM-6 ana gövdesiyle birlikte v2'de yeniden ölçülür.
- Sessizlikten-çıkarım-yok kuralının testi: PRE_CONVERGE'de N timeout sonrası estimator
  state'lerinin BOŞ kaldığını doğrulayan test.

## 7. Yasaklar
- Probe'u veya herhangi bir ölçümü veri akışının ÖNÜNE kapı yapmak.
- Sessizlikten estimator beslemek; önseli ölçüm gibi yazmak.
- Onaysız wire/byte-layout değişikliği (SACK alanları, probe formatı dahil).
- Spec'te olmayan davranışı kod yorumuyla meşrulaştırmak — karar önce spec'e.
- Sayı-tavanlı retry/duplication cap eklemek (persistence politikası deadline-güdümlü; F maddesi).
- Uydurma sabit: her yeni sabit [CAL] (türetme formülü + kalibrasyon domaini ile) veya [STD]
  (kaynak ile).
- Bu görevden bağımsız modüllerde "hazır elim değmişken" düzeltmesi — kapsam sabittir.
