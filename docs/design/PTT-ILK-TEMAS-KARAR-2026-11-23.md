# PTT gerçek ilk temas — bant kararı (Bulut-C, 2026-11-23)

> Tek karar kaydı. Yeni koşu yok; kod değişikliği yok. Bayt sayıları Web'in mevcut izinden
> (#233 gövdesi) ve koddan türetilmiştir; her satırda kaynağı yazılıdır.
> Etiketler: **ölçülmüş** (Web'in ürün fikstürü izi), **hesap** (koddaki birim boylardan),
> **tahmin** (hesap + ölçülmemiş zamanlama varsayımı), **mekanizma kanıtı** (kontrollü girdi, ürün koşusu değil).

## SAHİP KARARI (2026-11-23)

- Desteği bilinmeyen eşe birleşik biçimle başlama önerisi **REDDEDİLDİ** (öneri Ek A'da, UYGULANMAZ).
- Bilinmeyen eş **mevcut uyumlu biçimde** kalır (ayrı güvenilir PTT_OPEN / PTT_CLOSE, tam MEDIA_BUNDLE).
- Birleşik 300 ms biçimi bu PoC'de **yalnız desteği önceden doğrulanmış eşlerde** kullanılır (#233 pilotu, Web).
- **Gerçek ilk temas açık kalır:** 1 388 / 937 B ve 5 666 / 5 250 B, KALDI.
- Bu öneri için kod yazılmaz; Yerel'in netem kuyruğuna koşu eklenmez (Ek A'daki koşu talebi geri çekildi).

## 0. Kapsam ve etiketler

- **Taban:** `cand/asym-combined` @ `86c5602caa18ff592c201e23bd0946d17e43a779` (GitHub'dan doğrulandı).
- **Ölçüm kaynağı:** #233 (`exp/ptt-open-close-coalesce` @ `3a8e09a514fac897f24a8a3bca7123118ea8e769`).
  Gerçek ilk temas satırları 255f53fc ağacındaki ikiliyle ölçüldü; #233'e göre 0d644e42/3a8e09a5 farkı
  yalnız test/belge ve açılış kuralıdır. Hücre: dürüst, tohum 1, anahtarlı kayıp, birer koşu, Alice → Bob,
  IPv4/UDP dahil, bütün PTT datagramları. Lyra 3,2 kbit/s, 300 ms demet = 120 B yük.
- **ACK yolu: bitmap pilot.** #233'ün bütün satırları bitmap ACK yolundandır (0x15/0x16/0x17). **V2 ürün
  varsayılanında** yalın PTT (0x16) ve 0x17 yoktur (`AckFormat::V2`, `client-core/src/runtime.rs:673`);
  orada PTT kopyalı kuralla gider ve V2 izi yoktur (#175 7095127744). Aşağıdaki hesap V2 için geçerli değildir;
  V2'de 1,5 s basışta yalnız ses en az 5 × 194 = **970 B (hesap**, kopyasız alt sınır; V2 izi yok) > 937 B.
- Motor iki uçta cc-port. ACK bütçesi %25. Bant (5 000 bit/s) ve %75 eşiği değişmez.

## 1. Bayt bayt döküm (gerçek ilk temas, basış 0,5 s, yetenek bilinmiyor → bugünkü ayrı kayıtlar)

Koddan birim boylar (`kama-app/src/codec/limits.rs`, `client-core/src/runtime.rs`):

| öğe | B | kaynak |
|---|---|---|
| datagram sabiti (IPv4 20 + UDP 8 + oturum/AEAD + yalın çerçeve 3 B) | 53 | #233 7089464877: 0x16 sonrası açılışlı demet 180 B − kayıt 127 B |
| DATA çerçevesi fazlası (11 B başlık, yalın 3 B'ye göre) | +8 | `PTT_DEADLINE_NS` belgesi; 0x16 gelene dek ses de DATA ile gider |
| AMP kayıt başlığı | 5 | `APP_RECORD_HEADER_BYTES` |
| MEDIA_BUNDLE tam önek (`stream_id 4 · codec 1 · bundle_ms 1 · payload_len 2`) | 8 | `MEDIA_BUNDLE_PREFIX_BYTES` |
| PTT_OPEN gövdesi | 6 | `PTT_OPEN_BODY_BYTES` |
| PTT_CLOSE gövdesi | 4 | `PTT_CLOSE_BODY_BYTES` |
| güvenilir kayıt ileri kopyası | 2 | `PTT_RELIABLE_COPIES` (3 iletim; %20 iid kayıpta kayıt kaybı %0,8) |

Datagram boyları: ses demeti 53 + 5 + 8 + 120 = **186 B** (0x16 öncesi **194 B**); PTT_OPEN 53 + 8 + 5 + 6 =
**72 B**; PTT_CLOSE 53 + 8 + 5 + 4 = **70 B** (güvenilir kayıtlar her zaman DATA çerçeveli).
0x16/0x17 ilk kez ≈ +1,9 s'de gelir; 0,5 s'deki basışın demetleri 0,8 / 1,1 / 1,4 / 1,7 s'de (öncesi) ve
2,0 s'de (sonrası) oluşur → 4 ses datagramı DATA çerçeveli. #233'teki "0x16 öncesi 992 B (7)" = 4 × 194 + 3 × 72.

### 1,5 s basış — 1 388 / 937 B (fazla 451 B)

| sınıf | hesap | B | pay |
|---|---|---|---|
| ses yükü | 5 × 120 | 600 | %43 |
| sesin sabit başlıkları: datagram | 5 × 53 | 265 | |
| sesin sabit başlıkları: AMP tam başlık | 5 × 13 | 65 | |
| sesin sabit başlıkları: 0x16 öncesi DATA | 4 × 8 | 32 | |
| **ses datagramları toplamı** | 4 × 194 + 186 | **962** | #233: 962 (5) ✓ |
| açılış + kapanış, ilk iletim | 72 + 70 | **142** | #233: 142 (2) ✓ |
| kopyalar | 2 × 72 + 2 × 70 | **284** | #233: 284 (4) ✓ |
| yeniden iletim | — | 0 | |
| **toplam** | | **1 388** | 7 402 bit/s |

### 8,4 s basış — 5 666 / 5 250 B (fazla 416 B)

| sınıf | hesap | B |
|---|---|---|
| ses yükü | 28 × 120 | 3 360 |
| datagram sabiti | 28 × 53 | 1 484 |
| AMP tam başlık | 28 × 13 | 364 |
| 0x16 öncesi DATA | 4 × 8 | 32 |
| **ses datagramları** | 4 × 194 + 24 × 186 | **5 240** (#233 ✓) |
| açılış + kapanış | 72 + 70 | 142 |
| kopyalar | 284 | 284 |
| **toplam** | | **5 666** |

## 2. Mevcut tel biçimiyle ne kazanılabilir (eski eş uyumu korunarak) — hesap

Eski eş (birleşik biçimi bilmeyen v1 alıcısı) yalnız tam MEDIA_BUNDLE (13 B) ve ayrı PTT_OPEN/PTT_CLOSE çözer;
0x8A/0x8B/0x8C/0x8D'yi `unknown-msg-version` / `spurt_refused` ile DROP eder; açılışsız demet PTT-R3 ile yetimdir.
Güvenilirlik kuralı 3 iletimdir (`PTT_RELIABLE_COPIES = 2`, %20 iid kayıpta kayıt kaybı %0,8); ACK aralığı bu
hücrede ≈ 5 s olduğundan ARQ bir basış içinde kaydı kurtaramaz.

| kalem | maliyet (1,5 s / 8,4 s) | eski eş uyumuyla kaldırılabilir mi |
|---|---|---|
| ses yükü | 600 / 3 360 | hayır (Lyra 3,2 kbit/s) |
| datagram sabiti 53 B | 265 / 1 484 | hayır (oturum/AEAD/çerçeve) |
| AMP tam başlık 13 B (kompakt 2 B olurdu) | 65 / 364 | hayır: eski eş kompaktı DROP eder |
| 0x16 öncesi DATA +8 B (4 dgram) | 32 / 32 | hayır: yalın çerçeve desteği 0x16'dan önce bilinmez |
| ayrı açılış + kapanış, ilk iletim | 142 / 142 | hayır: eski eşte akışı açan/kapatan tek kayıt |
| kopyalar | **284 / 284** | teknik olarak evet, ama açılış/kapanış güvenilirliği düşer (kayıt kaybı %0,8 → %20; açılış kaybı = eski eşte bütün basış yetim) |

**Daraltılmış hüküm:** güvenilirliği koruyan tasarruf yoktur. Kopyaların maliyeti **284 B**'dir, fakat tamamı
çıkarılsa bile mevcut biçim 1,5 s'de **1 104 B** ile 937 B sınırını aşar (+167 B); 8,4 s'de 5 382 / 5 250 B (+132 B).
Kısa basışta **yalnız ses datagramları bile 962 B**'dir (937 B'nin 25 B üstü); 8,4 s'de 5 240 / 5 250 B ve eski
eşte en ucuz açılış kaydı 72 B olduğundan pay yetmez. Kapanış da bırakılsa: 1 034 / 937 ve 5 312 / 5 250 B.
Mevcut biçimle eski eş uyumlu ilk temas iki satırda da bant içine sığmaz.

### CAPS cevabını beklemek — elendi

Yetenek bildirimi (0x17) en erken ≈ +1,90 s'de gelir (#233, bütün birleşik satırlar). İlk demet 0,80 s'de
oluşur: beklenirse tek yön gecikmesi sıfır sayılsa bile gönderim anında yaşı 1,10 s > 1 s (ilk ses bütçesi).
İkinci demet (1,10 s) tek yön 0,3 s ile ≥ 2,20 s'de varır, yaşı ≥ 1,10 s. 1,5 s basışta en çok 45 / 75 = %60
zamanında (< %75). Karma yol (ilk demetler eski biçim, sonra birleşik) eski açılışı zaten öder:
4 × 194 + 3 × 72 = 992 B > 937 B. İkisi de elendi.

## 3. Sonuç ve devir

- Mevcut biçimle mümkün değil (§2). Bant engelini kaldırabilecek tek ürün kararı (Ek A) sahip tarafından
  reddedildi. Gerçek ilk temas **açık**; hedef 2'nin bu kısmı kırmızı kalır.
- **Web'e devir (#233 pilotu):** ilk ses iki ayrı süreyle raporlanır, biri ötekinin yerine kullanılmaz.
  Doğrulanmış eş, soğuk satır (300 ms / 1,5 s, tohum 1), #233'ün kendi sütun adlarıyla, ölçülmüş:
  (a) **basış başından** ilk sese **1,02 s**; (b) **ilk teslim edilen demetin oluşumundan** teslimine **0,72 s**.
  Aradaki 0,30 s bir demet süresine eşittir (hesap). Web #233'te (b)'nin ve ≤ 1 s zamanında kapısının hangi
  oluşum anını (demetin oluşumu mu, ilk 20 ms çerçevenin oluşumu mu) kullandığını açıkça yazmalıdır; fikstürde
  iki ifade birlikte geçiyor (`asym_product_outcome.rs:3274` "ilk çerçeve oluşumundan", `:3600` "demet başı").
- Gerçek istemcide doğrulanmış eş pilotunun kabulünü Yerel koşar. Bulut-C bu iş için kapandı.

## Ek A — REDDEDİLEN öneri (sahip, 2026-11-23) — UYGULANMAZ / MUST NOT IMPLEMENT

> Tarihçe için korunur. Bu bölümdeki kural yazılmaz, koşusu kuyruğa eklenmez.

**Öneri (reddedildi): yeteneği bilinmeyen eşe birleşik biçimle başla ("iyimser" varsayılan); eş
0x17'siz 0x16 ile "çözmüyorum" derse süren akış bugünkü ayrı kayıtlara döner ve olumsuz bilgi eş başına
saklanır.**

Tel sözleşmesi bu kararla değişmez: 0x8B/0x8C/0x8D, CAPS bit 2 ve 0x17 #233'ün (sahip onaylı deney) telidir;
değişen yalnız "bilinmiyor" durumunun yorumudur (`SpurtCaps::capable` bugün `false` döner). Geri dönüş yolu
#233'te zaten var: `Ptt::on_peer_spurt_capability(false)` süren birleşik akışı `force_reopen` ile ayrı
kayıtlara döndürür (`kama-app/src/ptt.rs:722–745`). Bu kayıt #233 koduna yazmaz; uygulama yazıcısı Web'dir.

### Yetenekli eşte bedel — ölçülmüş, yeni koşu gerekmez

Telde #233'ün "soğuk (0,5 s), yönetilen" satırıyla bayt-aynıdır: iki uçta CAPS, demetler ve 0x17 aynıdır;
fark yalnız Alice'in iç kararının kaynağıdır.

| basış | PTT tel baytı / tavan | bit/s | zamanında çerçeve | ilk ses (oluşumdan / basıştan) | açılış | kapanış |
|---|---|---|---|---|---|---|
| 1,5 s, 300 ms, K = 3 + her 4. | **927 / 937** (en kötü zamanlama 935, pay 2 B — 7089464877) | 4 944 | 60/75 = %80 | 0,72 / 1,02 s | ilk iki kayıpta PttStarted 1,00 s | END; kayıpta sessizlik sonu 1,7 s |
| 8,4 s, 300 ms, K = 3 + her 4. | **4 977 / 5 250** | 4 740 | 360/420 = %85 | 0,72 / 1,02 s | aynı | END düştü, sessizlik sonu 1,79 s |

Tek açılış/kapanış (Bob'da 1/1 başla/bitir) #233 çalışma zamanı kayıp testinde doğrulanmış. 1,5 s satırında
pay 2–10 B'dir; K = 3 (seyrek tekrarsız) en kötü durumda 930 B verir. K seçimi Web'in pilotundadır.

### Eski eşte bedel — tahmin (ölçülmedi)

- Olumsuz bildirim (0x17'siz 0x16) olumlu bildirimle aynı ACK'tir (yalnız tür baytı farklı), beklenen varış
  ≈ +1,9 s. Ondan önce gönderilen birleşik demetler eski eşte DROP olur (panik ve oturum hatası yok, §6 ileri uyum).
- 0,5 s'deki basışta demet 0–3 (60 çerçeve) kaybolur; demet 4'ten (2,0 s) itibaren PTT_OPEN (3 iletim) + tam demet.
  - 1,5 s basış: 15 / 75 = **%20** teslim (**tahmin**, eski eş) (< %75), ilk ses basıştan ≈ 1,5 s + aktarım. Tel: 3 × 188 + 183 + 3 × 72 +
    186 + 3 × 70 = 1 359 B (bugün 1 388 B; eski eş bugün de bant dışıdır).
  - 8,4 s basış: 24 / 28 demet, rastgele kayıptan önce en çok 360 / 420 = %85,7.
- Bedel oturum başına yalnız olumsuz bildirime dek süren ilk basıştır, **olumsuz bilgi saklanırsa**. Bugün `false`
  yalnız öğrenilmiş olumlu kaydı siler; iyimser varsayılanda saklanmazsa eski eş her oturumda ilk ≈ 1,9 s'yi kaybeder.
  Olumsuz kayıt bu kararın parçasıdır (yalnız aynı eş + `SPURT_FORMAT_EPOCH`).

### Karar reddedilirse

Gerçek ilk temas bugünkü gibi kalır: 1 388 / 937 ve 5 666 / 5 250 B, **KALDI**; hedef açık kalır. Kalan tek
yol bandın ilk temasta uygulanmamasıdır; bu bant kararıdır ve bu işin kapsamı dışındadır.

### A.1 Ret koşulları (önceden yazılmıştı)

1. Yetenekli eşle iyimser kuralın herhangi bir satırı #233'ün yönetilen satırından bayt olarak ayrılırsa
   (tel aynı değilse) ya da 937 / 5 250 B veya %75'i aşarsa: ret.
2. Eski eşle olumsuz bildirimden sonra akış yeniden açılmazsa (bildirimden sonra gönderilen demetlerde yetim > 0),
   tek PttStarted/PttEnded bozulursa ya da oturum hatası çıkarsa: ret.
3. Olumsuz bildirim +1,9 s'den belirgin geç gelir ve 8,4 s basışta eski eşte teslim < %75'e düşerse: bedel
   yeniden hesaplanır, karar sahibe geri döner.

### A.2 Eksik ölçü (artık gerekmez)

Eski eş + iyimser kural satırı (Bob `ptt_coalesce = false`, Alice bilinmeyeni yetenekli sayar): olumsuz
bildirimin varış anı ve geri dönüşten sonra teslim edilen çerçeve. Bu ölçü yalnız eski eşin bedelini
değiştirir; yetenekli eşin bayt hesabı (§3 tablo) değişmez. Fikstürde bu kol yoktur (`KAMA_ASYM_PTT_COALESCE`:
`off | neg | known | learned`); koşmak için önce iyimser kural yazılmalıdır. Şimdi koşulmadı.

### A.3 Mekanizma kanıtı (kontrollü girdi, kazı testi — ürün koşusu DEĞİL)

Soru: iyimser kuralda eski eşe giden süren basış, olumsuz bildirimden sonra aynı basış içinde kurtarılıyor mu?

- Ağaç: #233 dal ucu `3a8e09a514fac897f24a8a3bca7123118ea8e769`, ayrı worktree, ayrı `CARGO_TARGET_DIR`.
  #233 dosyalarına yazılmadı; `crates/kama-app/tests/ptt.rs`'nin kopyasına (`bulutc_scratch_iyimser.rs`, izlenmeyen,
  itilmedi) aşağıdaki test eklendi. Test metni Ek B'de.
- Komut: `cargo test --locked -p kama-app --test bulutc_scratch_iyimser bulutc_optimistic -- --nocapture`
- Kurulum: Alice `set_spurt_coalesce(true)` + `set_peer_spurt_provisioned(BOB, true)` (iyimser kuralın tel
  karşılığı); Bob `set_spurt_coalesce(false)` (birleşik biçimi bilmeyen alıcı). Bellek içi bağ, kayıp yok, elle saat.
  Tohum yok (belirlenimli).
- Ham çıktı:

```
BULUTC önce: alice_tel=[MediaBundle, MediaBundle, MediaBundle, MediaBundle] bob_olay=0 spurt_refused=3 yetim=1
BULUTC sonra: alice_tel=[PttOpen, MediaBundle, MediaBundle, PttClose] bob_olay=[PttStarted { peer: PeerRef(<redacted>), stream_id: 0, codec: Lyra, bundle_ms: 20 }, PttAudio { peer: PeerRef(<redacted>), stream_id: 0, payload_len: 3, .. }, PttAudio { peer: PeerRef(<redacted>), stream_id: 0, payload_len: 3, .. }, PttEnded { peer: PeerRef(<redacted>), stream_id: 0 }] spurt_refused=3 yetim=1
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s
```

- Okuma: bildirimden önceki 4 demet Bob'da olay üretmez: 3'ü açılışlı 0x8B (`spurt_refused`), 4.'sü kompakt 0x8A
  (akış yok → PTT-R3 yetim; gerçek v1 alıcısı onu `unknown-msg-version` ile düşürür). `on_peer_spurt_capability(false)`
  sonrası aynı akış güvenilir PTT_OPEN + tam demet + PTT_CLOSE ile sürer; Bob tek PttStarted / iki ses / tek PttEnded
  alır; geri dönüşten sonra yeni düşüş yok. Ret koşulu A.1/2'nin mekanizma kısmı bu düzenekte tetiklenmedi. Yalnız mekanizma kanıtıdır; ürün davranışı iddiası değildir.
- Sınanmayan: olumsuz bildirimin gerçek varış anı, kopyaların ve son tarihlerin gerçek hücredeki etkisi, oturumlar
  arası olumsuz kayıt (bugün yok; §3).

### A.4 Ürün koşusu talebi — GERİ ÇEKİLDİ (sahip kararı; kuyruğa eklenmez)

Yetenekli eş için koşu **gerekmez**: tel #233 yönetilen satırıyla aynıdır (§3). Eski eş bedeli için tek koşu:

- **SHA:** #233 `3a8e09a514fac897f24a8a3bca7123118ea8e769` + iyimser kural commit'i (henüz yok; yazıcı Web,
  `kama-app/src/ptt.rs` `SpurtCaps::capable` "bilinmiyor → evet" + olumsuz kayıt) + fikstür kolu
  (`KAMA_ASYM_PTT_COALESCE` için Alice iyimser / Bob `ptt_coalesce = false`). Koşu bu commit'in tam SHA'sıyla yazılır.
- **Senaryo (tek):** `dürüst-12500`, bitmap pilot yolu, cc-port iki uçta, ACK %25, anahtarlı kayıp, tohum 1,
  300 ms / 120 B Lyra, basış 0,5 s, süre 1,5 s.
- **Beklenen fark (bugünkü gerçek ilk temas satırına göre):** 0x17'siz 0x16 ≈ +1,9 s; Bob'da tek PttStarted /
  tek PttEnded; teslim 15 / 75 (bugün 75 / 75); PTT tel baytı ≈ 1 359 B (bugün 1 388 B); ilk ses basıştan ≈ 1,5 s
  + aktarım (bugün 0,62 s).
- **Ret:** A.1 madde 2 ya da 3.

## Ek B — kazı testi metni (Ek A.3; #233 3a8e09a5 üstünde, itilmedi)

```rust
/// [Bulut-C · kazı testi, itilmez] İyimser kuralın eski eşte geri dönüşü (dar mekanizma kanıtı).
/// Alice "bilinmeyen = yetenekli" sayar (tel olarak `set_peer_spurt_provisioned(true)` ile aynı);
/// Bob birleşik biçimi bilmez (`set_spurt_coalesce(false)`). Olumsuz bildirimden (0x17'siz 0x16)
/// önceki 4 birleşik demet Bob'da DROP; sonra aynı akış güvenilir PTT_OPEN + tam demet + PTT_CLOSE
/// ile sürer, Bob tek başla/bitir ve kalan iki demetin sesini alır, yetim yok.
#[test]
fn bulutc_optimistic_falls_back_on_a_legacy_peer_in_the_same_press() {
    let (mut duo, h) = Duo::connected();
    duo.alice.ptt.set_spurt_coalesce(true);
    duo.bob.ptt.set_spurt_coalesce(false);
    assert!(duo.alice.ptt.set_peer_spurt_provisioned(BOB, true));
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    for k in 0..4 {
        duo.alice.send_bundle(stream, &frame(k)).expect("demet");
    }
    let before = duo.bob.drain();
    let sent_before = duo.sent_on(Side::Alice, h).len();
    println!(
        "BULUTC önce: alice_tel={:?} bob_olay={} spurt_refused={} yetim={}",
        duo.sent_on(Side::Alice, h),
        before.len(),
        duo.bob.ptt.stats().spurt_refused,
        duo.bob.ptt.stats().ptt_orphan_bundles
    );
    assert!(before.is_empty(), "eski eş birleşik demetten olay üretmez");
    // 0, 1, 2: açılışlı 0x8B → spurt_refused; 3: kompakt 0x8A, akış yok → yetim (PTT-R3).
    assert_eq!(duo.bob.ptt.stats().spurt_refused, 3);
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);
    assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, h, false));
    duo.alice.send_bundle(stream, &frame(4)).expect("demet");
    duo.alice.send_bundle(stream, &frame(5)).expect("demet");
    duo.alice.close(stream).expect("kapanır");
    let after_types = duo.sent_on(Side::Alice, h)[sent_before..].to_vec();
    let after = duo.bob.drain();
    println!(
        "BULUTC sonra: alice_tel={:?} bob_olay={:?} spurt_refused={} yetim={}",
        after_types,
        after,
        duo.bob.ptt.stats().spurt_refused,
        duo.bob.ptt.stats().ptt_orphan_bundles
    );
    assert_eq!(
        after_types,
        vec![
            MsgType::PttOpen,
            MsgType::MediaBundle,
            MsgType::MediaBundle,
            MsgType::PttClose
        ]
    );
    assert_eq!(
        after,
        vec![
            started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS),
            audio(ALICE, stream, &frame(4)),
            audio(ALICE, stream, &frame(5)),
            ended(ALICE, stream),
        ]
    );
    // Geri dönüşten sonra yeni düşüş yok: sayaçlar bildirim öncesiyle aynı.
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);
    assert_eq!(duo.bob.ptt.stats().spurt_refused, 3);
}
```
