# SPRINT_BACKLOG — Sprint 2: Hub + Deployment + UI → DEMO-01

> **SUPERSEDED (yalnız süreç satırları) — 2026-11-09:** Bu backlog'un içeriği (kartların ne'si, S4/TDD atıfları, sayılar) geçerli. Aşağıdaki süreç satırları sahibin tarihli kararlarıyla aşıldı; metin silinmedi. Dayanak: `docs/decisions/KARAR-KAYDI-2026-11.md` §7.
> - Künye satırındaki "Durum: TASLAK (… 'PLAN-SPRINT2 ONAYLANDI' mesajıyla kesinleşir)" ve "Süre: 16–30 Ekim 2026": S2-A bu mesajı beklemeden açıldı (EK-1 D-13).
> - "A3 … 'INT-02 GEÇTİ'yi bekler" ve A3 bağımlılığındaki "'INT-02 GEÇTİ' (… A3 onun üstüne rebase'lenir)": A3 INT-02'yi beklemez, dumanı gerçek hub ikilisiyle koşar (EK-1 D-14). "INT-02 GEÇTİ" gösterimin şartı değildir.
> - "Gün N" takvimi (Gün 1–2 … Gün 13–14; `.aar` "hedef Gün 6"; A5 "Gün 4–5"): geçersiz. Geçerli takvim iş emirlerindedir.

Tarih: 2026-10-13 · Karar: Abkarada · Durum: TASLAK (K4G sprinti "INT-02 GEÇTİ" ile kapanınca "PLAN-SPRINT2 ONAYLANDI" mesajıyla kesinleşir) · Süre: 16–30 Ekim 2026

> **Yama 2026-10-14:** hub ile ilgili bütün satırlar `KM-SEAM-TH-001` v1'e (S4; spec çıkarımı `S4-CIKARIM-2026-10-13.md` üzerine yazıldı) göre düzeltildi: mesaj adları spec'ten (`CLIENT_BIND`/`BIND_PROOF`, `STORE_MESSAGE`/`DELIVER`, `ROUTE_MISS`, `HEARTBEAT`), oturum başına tek handle, "1024'te rebind" ve "10 dk ev temizliği" kalktı, `flow` kalktı, medya = ikinci hat, tek soket. Değişen satırlar **[S4]** ile işaretli.
> **Yama 2026-10-16 (4):** INT-02 GATING (2) kırmızı (transport motorunda yeniden-gönderim takılması; K4G-05b düzeltme kartı, K4G oturumu) → A3 ve A6 bağımlılıkları buna bağlandı; **Yaaz ön-başlatma**: A2.2 + A2.3 main'e girince "Y1 ÖN-BAŞLATMA" satırıyla Y1.1 başlar, DONE yine G-S4 GEÇTİ'yi bekler.
> **Yama 2026-10-15 (3b):** Yaaz'ın uygulama soruları → S4 v1.3.1 / TDD v0.6.1: cookie adres ailesi `0x04`/`0x06`; aynı adresten ikinci kimlik son kabulle adresi alır; "cevap ≤ istek" yalnız doğrulanmamış kaynağa; `src_unbound`/`src_mismatch` tanımları.
> **Yama 2026-10-14 (3):** üçüncü tur → S4 v1.3 / TDD v0.6 (Deploy v0.5 kaldı): INIT tekrarı `StoreResp`'le durmaz, doğrulanmış RESPONSE'a kadar sürer; `{UNKNOWN_HANDLE, 0}` = emekli tahsis anahtarı → yeni kriptografik INIT; deneme azami yaşı 1 saat (kurulu oturuma uygulanmaz); alıcı kotası dolunca yalnız gönderen–alıcı ortak hattı değiştirilir; tahsis anahtarı hattın alanı (LRU yok); spec yamaları "§8'deki bütün satırlar" (Y1–Y12). Aşılan ifadeler: "tekrar StoreResp gelince durur", "ilk atama kazanır", "alıcı dolu → hemen HUB_CAPACITY", "Y1–Y11".
> **Yama 2026-10-14 (2):** bağımsız inceleme (iki tur) sonrası S4 v1.2 / TDD v0.5 / Deploy v0.5: A1 artık kapı değil (KR mesajı), "G-S4 GEÇTİ" A2 paketinin sonunda; canlılık dili "pasif sessizlik tek başına düşürmez, kopma ilanı yalnız cevapsız yoklama sonrası"; "koptu" = yeniden bind, oturum devam; K4 yalnız adres değişiminde; hub sınırları kaynak adres başına ve yalnız geçerli cookie sonrası; bind cookie'si `bind_epoch`'a bağlı (nonce halkası yok); medya hattı bağlaması E2E `MEDIA_LINK` (KR-17); Y1/Y3 test listeleri güncel.

---

## BÖLÜM 1 — Herkes için sade özet

### Ne yapıyoruz
Geçen sprintte Rust çekirdeği bitti: şifreleme, transport, mesaj/dosya/PTT motorları, telefon kütüphanesi (`.so`). Ama **iki telefon birbirini bulamıyor**, çünkü aradaki **hub** yok. Bu sprintte hub yazılır, VPS'e konur, Hasan'ın uygulaması gerçek motora bağlanır, medya ekibi aramayı Kama'nın üstüne oturtur. Sprint sonunda **iki Android telefon, İstanbul'daki hub üzerinden, kötü ağda** mesajlaşır, fotoğraf atar, PTT yapar, arama yapar. Buna **DEMO-01** diyoruz; sprintin tek çıkış ölçütü bu.

### Hub nedir, ne değildir
Hub bir **postacı**dır: zarfın üstündeki 4 baytlık adrese (`hub_handle`) bakar, zarfı karşı telefona atar. Zarfı **açamaz** (anahtarı yok), **saklamaz** (mailbox yok), **kimin kiminle konuştuğunu** yalnız adres düzeyinde bilir. Telefon açılınca hub'a "ben buradayım" der (**bind**). Hub **canlılık kararı vermez**: bir telefonun "bağlı" olup olmadığını hiç hesaplamaz; yalnız her telefon için son bind'den gelen **dönüş adresini** tutar ve o adrese iletir **[S4]**. Hub'da veritabanı, TLS, web paneli, kullanıcı yönetimi **yoktur**. Sözleşme: `KM-SEAM-TH-001` (S4; baytlar, kurallar, sabitler yalnız orada).

### Canlılık: pasif sessizlik tek başına düşürmez, kopma ilanı yalnız cevapsız yoklama sonrası
Telefon, bağının koptuğunu **olaylardan** öğrenir; süreler yalnız "ne zaman yoklayayım"ı söyler, hiçbir süre tek başına "koptu" demez. Sırayla:
1. **İşletim sistemi olayı** — ağ değişti / arayüz düştü (`LinkChanged`) → anında yeniden bind (+ K4: adres değişti, yeni INIT).
2. **Gönderim hatası** — soket `send` hata verdi (arayüz yok, ICMP unreachable) → anında yoklama.
3. **ARQ** — gönderilmiş bir birim RTO yedeklemesi içinde ACK almadı (≈ 3 s, bağa göre) → "şüpheli"; hemen 3 yoklama (1 s arayla); cevap yoksa "koptu" → **yeniden bind; oturumlar devam eder** (hub hatları korur). Trafik varken kopma **saniyeler içinde** anlaşılır.
4. **Keepalive** (eski adıyla heartbeat) — yalnız **trafik yokken**: 8 s (ön plan) / 25 s (arka plan) sessizlikte tek küçük datagram. Amacı canlılık **ölçmek değil**, NAT'taki deliği açık tutmak ki karşı taraf bize ulaşabilsin. Cevabı gelmezse yine 3. maddedeki yoklama patlaması; "3 kaçırma / 6 kaçırma" sayımı **yoktur**.
Yeni INIT (yeni oturum) yalnız iki olayla: `LinkChanged` (K4) ya da hub'dan `ROUTE_MISS`. 60 saniyelik tam kesinti bile **aynı oturumla** toparlar: yeniden bind, aynı handle, kaldığı yerden (S4 C7).
Hub tarafında da aynı ilke **[S4]**: dönüş adresi yalnız **doğrulanmış bind** ile değişir; yeni bind adresi taşır, hatları korur; uç kaydı hiç silinmez (dizinle sınırlı), hat yalnız tablo doluyken en-uzun-boşta tahliyesiyle düşer — hiçbir zamanlayıcı tablo değiştirmez. Hub, bilmediği/emekli bir handle'a veri gelince bağlı göndericiye **ROUTE_MISS** döner — bu da olaydır, saat değil; telefon o handle'ı bırakıp yeni INIT atar (hub yeniden başladıysa bu yol).

### Güven modeli — PKI yok, ama güvenlik pazarlığı da yok
PKI kurmuyoruz; onun yerine **bir dizin dağıtıyoruz**. `kama-provision` aracı laptopta çalışır, her cihaz için anahtarlar üretir ve herkesin herkesi tanıdığı bir liste yazar. Üç ilişki:

| İlişki | Ne doğrulanır | Nasıl | Ne garantiler |
|---|---|---|---|
| **Telefon ↔ telefon** (asıl güvenlik) | Karşı taraf listedeki kişi mi | Mevcut el sıkışma: iki tarafın **statik X25519** anahtarları (listeden) + geçici anahtarlar. Listede olmayan anahtarla oturum **kurulamaz**. İmza yok, sertifika zinciri yok; liste güven çapasıdır. | Uçtan uca gizlilik ve kimlik. Hub ve yoldaki herkes kördür. |
| **Telefon → hub** (telefon kendini hub'a kanıtlar) **[S4]** | Bağlanan cihaz listede mi | **Ed25519 meydan okuma** (`CLIENT_BIND` → cookie → `BIND_PROOF`): hub durumsuz bir sayı (cookie) yollar, telefon **sertifikasındaki cihaz Ed25519 anahtarıyla** imzalar, hub `hub.json`'daki sertifikadan doğrular. Bir bağlanmada bir imza (64 B). Spec'in Plane 0 Noise IK'sinin PoC karşılığıdır; M4'te Noise IK'ye döner. | Listede olmayan cihaz hub'a bağlanamaz, hub'ın varlığını bile öğrenmez → DoS yüzeyi kapanır. Hub'da telefonlara ait **gizli anahtar yok**; VPS ele geçse telefon sırrı sızmaz. |
| **Hub → telefon** (telefon hub'ı tanır) **[S4]** | Doğru hub mu | Hub'ın Ed25519 açık anahtarı her telefonun dosyasında (`hub.ed25519_pub`, provisioning v1); hub `BIND_PROOF` yanıtını telefonun nonce'uyla imzalar. | Sahte hub telefonu kandırıp kara deliğe çekemez (gizlilik zaten E2E'dir, bu yalnız erişilebilirlik içindir). |

Bind'den sonraki hub mesajları (heartbeat, `STORE_MESSAGE`/`DELIVER`, `ROUTE_MISS`) **imzasızdır**: yoldaki bir saldırgan bunları taklit ederek gereksiz yeniden bağlanma üretebilir ama **hiçbir içeriği okuyamaz, değiştiremez, dönüş adresini kaçıramaz**. Bu, PoC'nin beyanlı sınırıdır (hop anahtarı = M4, PoC sonrası; S4 §5). "Güvenli mi?" sorusunun demo cevabı tek cümle: **"Listede olmayan hiç kimse ne bir telefonla ne de hub'la konuşabilir; hub hiçbir şeyi okuyamaz."**

### Kim ne yapıyor
| Kişi | İş | Görev kodları |
|---|---|---|
| **Abkarada** | S4 sözleşmesi `KM-SEAM-TH-001` (G-S4), `client-core` hub istemcisi, DoS tavanı, ölçüm erişimcileri, INT-03 | A1–A6 |
| **Yaaz** | Hub sunucusu (Rust, saf röle) — sözleşme S4 v1.3.1, tasarım+testler `KM-HUB-TDD-001.md` v0.6.1; VPS + Docker + CI + APK/provisioning hattı — `KM-DEPLOY-TDD-001.md` v0.5 | Y1–Y6 |
| **Hasan** | Uygulamayı Rust motoruna bağlamak, ses yolu (PTT/arama baytları), arka plan servisi, demo ekranları, overlay | H1–H5 |
| **Resul** | Kendi programı (KM-PLAN-001). Bu backlog'da görevi yok; Kama'nın ona verdiği/ondan beklediği arayüz Bölüm 2 sonundaki notta | — |
| **Hayri** | AMP bakımı, uygulama seviyesi testler | P1–P3 |
| **Meriç** | Demo ekranlarının tasarımı (H3 ile) | — |
| Claude Code | Her Rust görevinde icra; kararlar sahibin | — |

### Sıra ve kapılar
```
Gün 1–2   A1 S4 kararları (KR mesajı) ──► A2.2 + A2.3 main'de ──► "Y1 ÖN-BAŞLATMA" (Yaaz hub kodu başlar)
          ──► A2 FREEZE paketi ──► "G-S4 GEÇTİ" (Abkarada)   ← Y1 DONE / A3 bunu bekler; Y3, A4, H1 beklemez
(K4G)     INT-02 GATING (2) kırmızı → K4G-05b motor düzeltmesi → INT-02 yeniden → "INT-02 GEÇTİ"   ← A3 ve A6 (60 s kesinti) bunu bekler
Gün 2–5   Y1 hub ∥ A3 istemci ∥ A4 DoS ∥ A5 ölçüm ∥ Y3 VPS
Gün 5–6   A6 INT-03 (laptop ↔ VPS, kötü ağ) ──► "INT-03 GEÇTİ"
Gün 6–12  H1–H5 uygulama ∥ (medya: Resul'un programı) ∥ Y5 APK ∥ P1–P2
Gün 13–14 DEMO-01 provası ──► "DEMO-01 GEÇTİ"
```
Kapılar yalnız Abkarada'nın birebir mesajıyla geçer. Kurallar geçen sprintle aynı: görev başına PR, kart dışı iş yok, belirsizlikte DUR (sor), yeni bağımlılık yok (hub için std UDP yeter), spec'te silme yok (üstü çiz + YAMA KAYDI).

### Sözlük
- **hub_handle** **[S4]**: her veri datagramının ilk 4 baytı; hub oturum kurulurken (INIT kabulünde) verir, iki telefon **aynı** değeri kullanır, hub değiştirmez. Telde açık, içerik şifreli.
- **bind** **[S4]**: telefonun hub'a kaydolması (`CLIENT_BIND` → cookie → `BIND_PROOF` → imzalı yanıt; 2 RTT, iki yönlü imza). Ağ değişince yeniden yapılır; yeni bind adresi taşır, oturumları düşürmez.
- **zarf (envelope)** **[S4]**: oturum kurma mesajı (INIT/RESPONSE) ya da medya hattı açılışı; `STORE_MESSAGE` ile hub'a, `DELIVER` ile karşıya. Hub zarfı alıcı bağlıysa iletir, değilse `PEER_UNBOUND` döner; gönderen tekrar dener (T3, aynı `request_id`).
- **keepalive (heartbeat)**: trafik yokken NAT deliğini açık tutan küçük datagram; ön planda 8 s, arka planda 25 s sessizlikten sonra. Canlılık ölçmez; cevabı gelmezse yoklama patlaması (3 × 1 s) tetiklenir.
- **yoklama patlaması**: "şüpheli" anında 1 s arayla 3 keepalive; hiçbiri cevaplanmazsa "koptu" → yeniden bind; oturumlar ve handle'lar **kalır**.
- **T3**: oturum kurma mesajının tekrar kadansı: 1 s, 2 s, 4 s … en çok 30 s. Hub'ın `OK` yanıtı tekrarı **durdurmaz** (o yalnız "hat tahsis edildi" demektir); tekrar doğrulanmış RESPONSE gelince, niyet iptal edilince ya da deneme sonlandırılınca durur. Bir deneme en çok 1 saat yaşar; sonra yeni deneme (kurulu oturumlar etkilenmez).
- **K4**: ağ (IP) **değişince** (`LinkChanged`) eski bağ düşer, yeniden bind + yeniden oturum; teslim edilmemiş mesajlar yeniden gider. Yalnız adres değişiminde; kesinti sonrası aynı adresten toparlanma K4 değildir.
- **netem**: Linux'ta gecikme/kayıp simülasyonu (`delay 200ms loss 30%`).
- **INT-03**: iki makine + gerçek hub + netem testi. **DEMO-01**: iki telefon + hub + kötü ağ.

### Sayılar (Abkarada kararı; "G-S4 GEÇTİ" ile kesinleşir)
| Konu | Değer |
|---|---|
| Hub mesajları **[S4]** | Kontrol datagramı = ilk 4 bayt `0x00000000` + çerçeve tipi + gövde + 16 sıfır bayt (telefonun mevcut HubChannel alım yolu). Çerçeveler: `HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C` · `PLANE1_RPC 0x1D` içinde spec'in `AppRpcFrame`'i: **`CLIENT_BIND` 0x01 → cookie → `BIND_PROOF` 0x0B** · **`STORE_MESSAGE` 0x05 → `DELIVER` 0x81** (INIT / RESPONSE / medya hattı) · **`ROUTE_MISS` 0x84**. Veri: `[hub_handle][yük]`, hub yükü okumaz, değiştirmez. Baytlar yalnız `KM-SEAM-TH-001` §2'de. |
| Bind doğrulama | Ed25519 meydan okuma, iki yönlü, 2 RTT, hub tarafı durumsuz (cookie); cihaz anahtarı = sertifikadaki Ed25519 |
| Canlılık | Pasif sessizlik tek başına düşürmez. Yoklamayı tetikleyen: OS olayı / gönderim hatası / ARQ (RTO yedeklemesi ≈ 3 s) / cevapsız heartbeat → yoklama patlaması 3 × 1 s → cevap yoksa "koptu" = yeniden bind, oturum devam. Sayım yok. Yeni INIT yalnız `LinkChanged` ya da `ROUTE_MISS`. |
| Heartbeat | Yalnız sessizlikte: 8 s ön plan / 25 s arka plan / doze OS; gövde boş ya da 4 B token; cevapsız → yoklama patlaması; hub yalnız bağlı adrese cevap verir |
| Hub'da uç/hat **[S4]** | Dönüş adresi yalnız doğrulanmış bind ile; yeni bind adresi taşır, hatları korur; uç kaydı silinmez; hat yalnız tablo doluyken en-uzun-boşta tahliyesiyle düşer; emekli handle 7 gün yeniden verilmez; bilinmeyen handle'a veri → bağlı göndericiye `ROUTE_MISS` (yabancıya sessiz) → telefon yeni INIT atar |
| Handle ömrü **[S4]** | Oturum ömrü boyunca sabit; yenileme = yeni oturum = yeni INIT = yeni handle (7 günlük yaş ya da PN tavanı). "1024'te rebind" diye bir şey **yok** (K = 1024 hop-ACK'siz PN tavanıdır, spec-only, istemci işi) |
| T3 **[S4]** | 1 s × 2 … ≤ 30 s; aynı `request_id` ile bayt-özdeş tekrar; `StoreResp OK` tekrarı **durdurmaz**, doğrulanmış RESPONSE durdurur; `{UNKNOWN_HANDLE, 0}` → deneme biter, yeni kriptografik INIT + yeni `request_id`; deneme azami yaşı 1 saat (`ATTEMPT_MAX_AGE`); `PEER_UNBOUND`/`HUB_CAPACITY` sonrası ≥ 5 s |
| Hub sınırları **[S4]** | `CLIENT_BIND` 1. tur sınırsız (durumsuz); `BIND_PROOF`: önce cookie, sonra bütçe (küresel 2000/s), sonra imza; yanlış cookie durum yaratmaz; başarısızlık yalnız geçerli cookie sonrası, **kaynak adres** başına 10/60 s, tablo 4096 girdi (kimlik de adres taklidi de Alice'i kilitleyemez); `STORE_MESSAGE` uç başına 10/s; yeni hat tahsisi uç başına 1/s; `ROUTE_MISS` uç başına 1/s; datagram ≤ 1280 B; en çok 8192 hat, uç başına 64 (alıcı doluysa yalnız gönderen–alıcı **ortak** en-uzun-boşta hat değiştirilir, ortak hat yoksa `HUB_CAPACITY`; üçüncü tarafın hattı düşürülmez), 65 536 tombstone (sığmayan emeklilik → `HUB_CAPACITY`, erken silme yok; başarısız tahsis hatları/sayaçları/tombstone'u değiştirmez); tahsis anahtarı hattın alanı, emekli olunca tombstone'a taşınır, emekli anahtara `{UNKNOWN_HANDLE, 0}`; veri hattına tavan yok (beyan); kuyruk yok |
| DoS (kripto tarafı) | eş başına en çok 4 yarım oturum (en eski düşer); kaynak başına INIT 1/s, patlama 5 |
| Medya **[S4]** | Hub için medya = `STORE_MESSAGE{envelope_type = RELAY_OPEN}` ile açılan sıradan ikinci hat (`flow` yok); hattın eşe **bağlanması hub mesajıyla değil, uçtan uca** (E2E, güvenilir `MEDIA_LINK{OPEN, h}` → `ACK`; medya ACK'ten sonra akar; iki taraf aynı anda açarsa küçük `dest_hash` kazanır); telefon tarafında **tek soket**, medya handle'ları kriptodan önce ayrılır; medya motoru yalnız `mediaSend`/`MediaPacket` görür. Medya **anahtarı**: EK-3 "oturumdan türetilir" der, bu backlog "signaling içinde 32 B rastgele" demişti — **Abkarada + Resul kararı** (A3 başlamadan) |

---

## BÖLÜM 2 — Görev kartları

Her kart: **Neden** (bu olmazsa ne olur) · **Ne** (tam olarak ne yapılacak) · **Nereye** (dosya/dizin) · **Bağımlılık** · **Done** (kabul ölçütü). Kartta yazmayan şey yapılmaz; gerekiyorsa DUR.

### A — Abkarada (karar + client-core + kripto)

**A1 — S4 kararları (KR mesajı)**
- Neden: Hub ile telefonun anlaşması yazılı olmadan Yaaz ve ben paralel yazamayız; geçen sprint bunu S1/S3 ile öğrendik.
- Ne **[S4]**: Spec çıkarımı yapıldı (`S4-CIKARIM-2026-10-13.md`, PR #132), `KM-SEAM-TH-001` v1 onun üstüne yazıldı, bağımsız incelemenin üç turu (9 + 6 + 4 bulgu) v1.3'e işlendi. Karar = §0'daki **KR-1…19** (her satırda önerilen değer var): "KR-n: EVET / HAYIR — neden / DEĞİŞTİ — değer".
- Nereye: karar mesajı (`G-S4-KARAR-MESAJI.md` şablonu) S2-A orkestratörüne; sonucu A2'ye girdi.
- Bağımlılık: yok — "INT-02 GEÇTİ"yi **beklemez** (A2'nin ön-başlatması buna bağlı değil; karar mesajında açıkça yazılı).
- Done: **"S4 KARARLARI" mesajı yazıldı** (KR-1…19 her biri cevaplı, tarihli). Bu kapı değildir; "G-S4 GEÇTİ" A2'nin sonunda verilir.

**A2 — S4 FREEZE paketi: belge, tipler, vektörler, harness, provisioning v1, yamalar (Claude Code) → "G-S4 GEÇTİ"**
- Neden: Hub ve istemci aynı baytları konuşmalı; vektörler iki tarafı aynı anda test eder. FROZEN damgası paketin **sonucudur**, girdisi değil.
- Ne **[S4]**: (1) `KM-SEAM-TH-001` v1.3.1 repoya (KR kararları işlenmiş); `KM-HUB-TDD-001` v0.6.1 ve `KM-DEPLOY-TDD-001` v0.5 yanına; TASLAK'a SUPERSEDED bandı. (2) `kama-contracts::s4` = S4 §2 tipleri + fixed-width codec (sıfır bağımlılık; `ErrorPayload`, `DELIVER.sender_dest_hash`, `RELAY_OPEN` gövdesi dahil). (3) Python üretici `ref/hub/gen_vectors.py` → `vectors/hub/` (S4 §7 listesi; §2.3 tablosunun her satırı). (4) Harness `crates/kama-integration/tests/s4_*` (sahte hub ↔ gerçek istemci, gerçek hub ↔ sahte istemci; S4 §7 kapanış vakaları K1–K11, kimlik kilitleme denemeleri, epoch tekrarı dahil; el sıkışma vakalarında başarı = iki uçta aynı handle + doğrulanmış oturum + iki yönde veri) + `scripts/ci/hub-conformance.sh`. (5) **kama-provision v1**: `--hub` verilince hub Ed25519 çifti, cihaz dosyasına `hub.ed25519_pub`, ayrıca `hub.json`; `--probe` ile eş listelerine girmeyen `probe` cihazı. (6) Spec yamaları: S4 §8'deki **bütün satırlar** (Y1–Y12; Y11 A2.1'de tamamlanır; Y12 = `MEDIA_LINK` uygulama mesaj protokolü belgesi + codepoint kararı, A2.6'da) — üstü çiz, YAMA KAYDI, word_coverage.
- Nereye: `docs/seams/KM-SEAM-TH-001.md`, `docs/hub/`, `crates/kama-contracts/src/s4.rs`, `ref/hub/`, `vectors/hub/`, `crates/kama-integration/tests/s4_*`, `crates/kama-provision`, `docs/provisioning/PROVISIONING-FORMAT-v1.md`, `specs/*` (yamalar).
- Bağımlılık: A1 (KR mesajı). INT-02'yi beklemez.
- Done: paket main'de (A2.1–A2.7 merge); `s4.rs` derleniyor; vektörler Python'dan; `--target mock` harness yeşil (real hedef Y2 ile); provisioning v1 testli; S4 §8'deki bütün satırlar main'de → Abkarada paketi inceler ve **"G-S4 GEÇTİ"** yazar → belge FROZEN damgası alır (ayrı tek satırlık PR). İcra: `ISEMRI-S2-A.md` kartları A2.1–A2.7.

**A3 — Telefon tarafı hub istemcisi**
- Neden: Telefon hub'a bağlanmayı, kopunca toparlanmayı, oturum zarfını hub üzerinden yollamayı bilmiyor.
- Ne **[S4]**: `client-core`'a gerçek zarf taşıyıcısı = hub istemcisi (`EnvelopeLink`'in UDP + S4 uygulaması; S4 §4 C1–C10): **tek UDP soketi** (bind, kontrol, veri, heartbeat, medya); PoC-BIND (`CLIENT_BIND` → cookie → `BIND_PROOF`, hub imzası doğrulanır); INIT = `STORE_MESSAGE{0, INIT}` ve **T3 tekrarı aynı `request_id` ile bayt-özdeş, doğrulanmış RESPONSE gelene kadar** (`StoreResp OK` tekrarı durdurmaz; yalnız `AssignHandle`); `{UNKNOWN_HANDLE, 0}` → deneme biter, niyet sürüyorsa **yeni kriptografik INIT + yeni `request_id`**; çelişkili handle → hata geçişi + yeni deneme ("ilk atama kazanır" yok); deneme azami yaşı 1 saat (kurulu oturuma uygulanmaz); yanıtlayan tekrar INIT'e yeni oturum açmaz, RESPONSE'u yeniden gönderir; `RELAY_OPEN` tekrarı yalnız hub yanıtını bekler; RESPONSE = `STORE_MESSAGE{h, RESPONSE}`; `DELIVER` → `IncomingInit`/`Response` olayları; `ROUTE_MISS` → handle bırakılır, sınırlı yeniden INIT; **olay güdümlü canlılık** (Bölüm 1): OS olayı ve soket hatası → anında yeniden bind; ARQ'da RTO yedeklemesi dolan birim → şüpheli → yoklama patlaması → koptu; heartbeat yalnız sessizlikte (8/25 s), cevapsızsa aynı patlama; **hiçbir yerde kaçırma sayacı yok**; "1024'te rebind" YOK. **"Koptu" = yeniden bind, oturumlar devam eder** (aynı handle; 60 s kesinti dahil). K4 akışı **yalnız `LinkChanged`'de** (adres değişti → yeniden bind → aktif eşlere yeni INIT → eski oturum Superseded → teslim edilmemişler yeniden); `ROUTE_MISS` → yalnız o oturum için yeni INIT. HubChannel çerçeveleri `on_hub_frame()`'de, oturum durumuna erişimsiz imza korunur (D1). `kama-node`: `connect` hub üzerinden; `metrics()`'te `last_heartbeat_age_s` dolar (hub'dan gelen son datagramın yaşı — her tür). **Medya taşıyıcısı Rust'ta, aynı soketten:** `kama-node` eşle `RELAY_OPEN` ile ikinci hat açar ve handle'ı eşe **E2E `MEDIA_LINK{OPEN, h}`** ile bildirir, eş `ACK` döner (hub'ın `DELIVER{RELAY_OPEN}`'ı bağlama yaratmaz; S4 C10, KR-17; `MEDIA_LINK` uygulama mesaj protokolüne eklenir — tür kaydı `s1.rs`'teyse DUR); medya handle'ları kriptoya girmeden ayrılır (handle → bağlam, bağlam-önce); API `media_open(peer)`/`media_close(peer)`, olaylar `MediaLinkOpened/Closed`, `media_send(peer, bytes)` (başına handle, tek datagram, ARQ yok) ve `MediaPacket { peer, bytes }`. Medya motoru hub'ı hiç görmez. Medya **anahtarı** kararı (EK-3 türetme ↔ signaling içinde rastgele) A3 başlamadan Abkarada + Resul.
- Nereye: `crates/client-core/src/{transport.rs, runtime.rs, hub_link.rs(yeni)}`, `crates/kama-node`.
- Bağımlılık: A2; "INT-02 GEÇTİ" (= K4G-05b motor düzeltmesi main'de; A3 onun üstüne rebase'lenir).
- Done: S4 conformance telefon tarafı (sahte hub) yeşil; T3/heartbeat/yoklama/ROUTE_MISS/K4 testleri sahte saatle (özellikle: trafik varken kopma ≤ RTO yedeklemesi içinde anlaşılıyor; sessizlikte ≤ heartbeat aralığı + 4 s; **pasif sessizlik tek başına oturumu düşürmez, kopma ilanı yalnız cevapsız yoklama sonrası**; 60 s kesinti → aynı handle, yeni INIT yok); medya hattı `MEDIA_LINK` OPEN/ACK/CLOSE + glare testleri; geçen sprintin güvenlik testleri değişmeden yeşil; INT-02 hâlâ geçiyor.

**A4 — DoS tavanı (KS-DOS-1) — bu sprintin ilk kripto işi**
- Neden: Listedeki kötü niyetli bir telefon yarım oturum fırtınasıyla karşı tarafın belleğini ve bandını tüketebilir; bant yokken bu teorik değil.
- Ne: Yanıtlayanda eş başına en çok 4 yarım (provisional) oturum, en eski düşer; aynı INIT baytları tekrar gelirse yeni yuva açmaz, mevcutu yeniler; kaynak başına INIT hızı 1/s, patlama 5, aşım sessiz drop + sayaç. S3 arayüzüne dokunmaz (iç politika). ccr §35.15'e sabitler YAMA KAYDI ile.
- Nereye: `crates/kama-noise-wire/src/session_control.rs`, `protector.rs:651` yorumu, `specs/ccr-format-spec.md` §35.15.
- Bağımlılık: yok (G-S4'ü beklemez).
- Done: testler: 5. yarım oturum en eskiyi düşürür; aynı INIT yeni yuva açmaz; hız aşımı drop; dürüst %90 kayıp tekrar profili sınırı tüketmez; kani harness'ı değişmeden yeşil. (INT-02'ye bağlı değil; `crates/adaptive-transport`'a dokunmaz.)

**A5 — Ölçüm erişimcileri (overlay için)**
- Neden: Demoda ekranda RTT/kayıp/bitrate görünmezse "Kama daha iyi" iddiası havada kalır; bugün bu üçü `None`.
- Ne: `client-core`'da salt-okunur erişimciler: sRTT, son 10 s kayıp oranı (yeniden gönderim / gönderim), son 1 s bitrate (tx/rx); `kama-node.metrics()` dolar; UniFFI'ye geçer. Motor davranışı değişmez.
- Nereye: `crates/client-core` (yalnız okuma; `api_surface_v0` pin'i kasıtlı güncellenir), `crates/kama-node`, `crates/kama-mobile`.
- Bağımlılık: A3.
- Done: INT-03 raporunda gerçek sayılar; sahte saatle kestirim testi.

**A6 — INT-03 ("INT-03 GEÇTİ")**
- Neden: INT-02 tek makinedeydi; hub ve gerçek ağ ilk kez burada.
- Ne **[S4]**: Düğüm A laptop (netem `delay 200ms loss 30%`), düğüm B VPS'te ikinci konteyner ya da ikinci laptop, hub VPS'te. Beş ölçüm: (1) 100 TEXT + 100 RECEIPT, 5 tohum; (2) epoch 0→1 geçişi; (3) K4: A'nın adresi değişir → yeniden bind → mesajlar teslim; (4) 60 s tam kesinti → yoklama sonrası yeniden bind, **aynı oturum, aynı handle**, yeni INIT **yok** (S4 C7); tüm mesajlar + PTT klibi devam; (5) hub yeniden başlatılır → iki düğüm Deploy TDD §6 süre kuralı içinde yeniden bind olur, ilk veride `ROUTE_MISS` alır, oturumlar yeniden kurulur; **süreler ölçülür, uydurulmaz**. `ctx_mismatch_drops == 0`, `delivery_channel_drops == 0`. Rapor beyanlı boşluklarla (hop anahtarı yok, BestEffort yok, hattaki saldırgan ek yuva, hub yeniden başlayınca oturumlar yeniden).
- Nereye: `crates/kama-integration/examples/int03_node.rs`, `docs/reports/INT-03-RAPORU.md`.
- Bağımlılık: A3, A4, Y1, Y3 ve **K4G-05b** (transport yeniden-gönderim düzeltmesi; (4) 60 s kesinti ölçümü onsuz geçemez — INT-02'de ölçüldü).
- Done: 5/5 tohum, beş ölçüm yeşil; rapor main'de; "INT-03 GEÇTİ".

### Y — Yaaz (hub + deployment)

**Y1 — Hub sunucusu (`hub-server`), üç parça — sözleşme `KM-SEAM-TH-001` (S4 v1.3.1), tasarım ve testler `KM-HUB-TDD-001.md` v0.6.1; aşağıdaki özet onların yerine geçmez** **[S4]**
- Neden: Hub olmadan iki telefon birbirini bulamaz; demo yok.
- Önce oku: S4 §1 (on cümle), §2 (tel; §2.3 mesaj tablosu, §2.4(c) geçerli birleşimler), §3 (H1–H12 kuralları), §6 (sabitler); sonra TDD §2 (tablolar), §3 (işleyici sözde kodu), §7 (önce yazılacak testler). Toplam ~6 sayfa. Kodlama sırası TDD §7'deki test sırasıdır: testi yaz → kırmızı gör → yeşile çevir.
- Ne (Y1.1 — config + bind + heartbeat; TDD §3 `on_bind1/on_bind2/on_heartbeat`, §6; testler T-B1–13, T-H1–3, T-C1–3): `hub.json` oku (okunamazsa `config unreadable`, çıkış 1); tek UDP soketi; `CLIENT_BIND` → durumsuz cookie yanıtı (cookie ucun `bind_epoch`'una bağlı) → `BIND_PROOF`: **sıra kayıp-ACK tekrarı → cookie → adres kilidi + bütçe → imza** (`ed25519_strict::verify_strict`); yanlış cookie hiçbir durum yaratmaz, başarısızlık yalnız geçerli cookie sonrası ve **kaynak adres** başına (kimlik de adres taklidi de kilitleyemez); her kabul `bind_epoch`'u ilerletir → eski kanıt adresi geri taşıyamaz; hub yalnız son kabul edilen kanıtı saklar (kayıp ACK → aynı yanıt) → imzalı yanıt; aynı kimlik yeni adresten → adres taşınır, hatlar korunur; aynı adresten ikinci kimlik → son kabul adresi alır, önceki uç adressiz kalır (hatları korunur); cookie adres ailesi `0x04`/`0x06`; heartbeat → aynı uzunlukta ACK, yalnız bağlı adrese. Hub canlılık **hesaplamaz**.
- Ne (Y1.2 — zarf rölesi; TDD §3 `on_store`; testler T-S1–15): `STORE_MESSAGE{0, INIT|RELAY_OPEN}` → `{OK, h}` gönderene + `DELIVER{h, gönderen dest_hash, tip, zarf}` alıcıya (alıcı bağlı değilse `PEER_UNBOUND`); tahsis anahtarı (gönderen + `request_id` + alıcı + tip + zarf özeti) **hattın alanıdır**: anahtar aktif hatta → aynı h ve **yeniden teslim** (kaç işlem geçerse geçsin, LRU yok); anahtar tombstone'da → `{UNKNOWN_HANDLE, 0}` (tahsis yok, DELIVER yok); hiçbir yerde → yeni hat; aynı `request_id` farklı alıcı → **yeni** hat; `STORE_MESSAGE{h, RESPONSE}` → alıcı hattın karşı ucu olmalı → `DELIVER`; geçersiz (handle, tip) birleşimi → `BAD_REQUEST` (`ErrorPayload`). Zarfın içini **açma**.
- Ne (Y1.3 — veri iletimi, ROUTE_MISS, sınırlar, tombstone, log; TDD §3 `on_data/allocate/evict/retire`, §4, §5; testler T-D1–5, T-M1–9, T-R1–4, T-L1): `[h][yük]` → kaynak hattın ucuysa karşı uca **bayt-özdeş**; değilse sessiz drop; bilinmeyen h → yalnız bağlı kaynağa `ROUTE_MISS` (hız sınırlı); token bucket sınırları S4 §6 adlarıyla; yeni hat için **sekiz adımlı emeklilik planlayıcısı** (S4 §2.6): alıcı kotası dolu → yalnız gönderen–alıcı ortak en-uzun-boşta hat (yoksa `HUB_CAPACITY`), gönderen kotası → kendi kalan en-uzun-boşta hattı, küresel tavan → en-uzun-boşta; plan küme; iki uç + küresel sınır + tombstone kapasitesi + aday handle denetimleri geçmeden **hiçbir şey değişmez** (garanti dışı yalnız hız bütçesi ve aday sayacı); 7 gün tombstone (`handle + anahtar`); 8 tahsis çakışması `HUB_CAPACITY` (üstüne yazma yok; sayaç her denemede ilerler); 10 s'de bir `hub stats`; `probe` alt komutu.
- Nereye: `crates/hub-server/src/{main.rs, config.rs, wire.rs, bind.rs, table.rs, relay.rs, rate.rs, stats.rs, probe.rs}` (TDD §5); bağımlılık `kama-contracts::s4` + `kama-noise-wire` + `ed25519-dalek` + `blake2` + `sha2` + serde/serde_json — hepsi Cargo.lock'ta, **yeni crate eklenmez**. Saat ve rastgelelik dışarıdan verilir.
- Bağımlılık: A2.2 + A2.3 main'de → Abkarada'nın "Y1 ÖN-BAŞLATMA: s4.rs <SHA>, vectors/hub <SHA>" satırı (A2 paketinin tamamını beklemez). DONE için "G-S4 GEÇTİ" şart; ön-başlatma sonrası bayt değişirse "S4 BAYT DEĞİŞTİ" satırıyla rebase.
- Done: TDD §7'deki testlerin **hepsi** yeşil; vektörler Python'dan; S4 conformance hub tarafı yeşil; `layer_check` yeşil; S4 H1–H12'nin her biri bir testle kilitli (PR gövdesinde kural → test adı tablosu).

**Y2 — Hub conformance'ı CI'da**
- Neden: Hub ile telefon ayrı kişilerce yazılıyor; aynı vektörler ikisini bağlar.
- Ne: A2'nin harness'ını `ci.yml`'e hub real-hedef adımı olarak ekle; kapılar diğer crate'lerle aynı (fmt, clippy, test, deny, layer_check, vektör).
- Nereye: `.github/workflows/ci.yml` (yalnız yeni adım; yapı değişmez).
- Bağımlılık: Y1.1.
- Done: PR kontrolleri hub'ı kapsıyor.

**Y3 — VPS + Docker + hub ayakta — spec: `KM-DEPLOY-TDD-001.md` §3–§6**
- Neden: Demo İstanbul'daki gerçek hub'la.
- Ne: Deploy TDD §3 (sunucu), §4 (imaj: statik, root değil), §5 (compose + systemd + Makefile), §6 runbook adımları 6.1–6.7 birebir.
- Nereye: `deploy/{Dockerfile, compose.yml, Makefile, kama-hub.service}`, `docs/deploy/HUB-DEPLOY.md` (= TDD §6 kopyası).
- Bağımlılık: Y1.1 (bind + keepalive imaj için yeter).
- Done: Deploy TDD T-DEP-0, T-DEP-1, T-DEP-2, T-DEP-5, T-DEP-6 (gerçek başlangıç + probe ile), T-DEP-6b, T-DEP-9 (gerçek VPS'te) yeşil; `hub.json` sahibi 65532 / 0400; yaşam döngüsü yalnız systemd'de; `hub-server probe` (probe cihazıyla) laptop'tan `bind ok … heartbeat 3/3` alıyor; belge main'de. **[S4]**

**Y4 — CI: hub imajı — spec: `KM-DEPLOY-TDD-001.md` §7**
- Ne: PR kapıları hub-server'ı kapsar (mevcut `ci.yml` adımları); `hub-image.yml` yalnız main merge'inde imaj derler ve `ghcr.io/<org>/kama-hub:<kısa-sha>` push eder; otomatik deploy **yok**; Actions dakikası raporu adımı.
- Nereye: `.github/workflows/{ci.yml (yalnız adım ekleme), hub-image.yml}`.
- Bağımlılık: Y1.1, Y3.
- Done: Main'deki son SHA'nın imajı registry'de; `make deploy TAG=<sha>` ile VPS'te çalışıyor (T-DEP-3, T-DEP-4).

**Y5 — APK/AAR hattı + telefonlara dağıtım — spec: `KM-DEPLOY-TDD-001.md` §8**
- Neden: Demoda iki telefona 15 dakikada tekrarlanabilir kurulum lazım.
- Ne: AAR üretimi (`.so` arm64 16 KB hizalı + Kotlin), yan yükleme imzalı APK (`versionName = <tarih>-<sha>`), `kama-provision init --devices alice,bob --probe --hub <vps>:47000` (v1, A2), telefona aktarım (adb), içe aktarma; belge `PROVISION-DEMO.md` (= TDD §8). **[S4]**
- Nereye: `docs/deploy/PROVISION-DEMO.md`; Hasan'ın reposunda AAR tüketimi (H1).
- Bağımlılık: H1 (uygulama), A5 (son `.so`).
- Done: T-DEP-8 — sıfırdan iki telefon 15 dk, hub üzerinden TEXT; belge main'de.

**Y6 — (isteğe bağlı) Kötü ağ profilleri**
- Ne: Laptop'ta tek komutla netem profilleri: iyi / kötü (200 ms, %30) / kesinti (60 s %100). Provada kullanılır.
- Nereye: `scripts/netem/`.

### H — Hasan (Android)

Hasan'ın bu sprintteki işi tek cümle: **telefondaki uygulamayı Rust motoruna bağlamak ve demo ekranlarını bitirmek.** Signaling içeriği, arama kurulumu, codec, görüntü Hasan'ın işi değil (Resul'un programı). Rust motoru Hasan'a bir `.aar` olarak gelir; içinde `KamaMobile` adında tek bir sınıf vardır. Hasan'ın kendi facade belgesi (`ui-api v0.3`) ve `FakeKamaEngine`'i olduğu gibi kalır; yapılacak şey `RealKamaEngine`'in içini bu sınıfla doldurmaktır.

`KamaMobile`'ın Hasan'a verdiği şey (hepsi Abkarada'nın A3/A5 işi ile gelir, `README-HASAN.md`'de örnekli):
- `start(configPath, dataDir, listener)` — motoru başlatır. **Arka plan iş parçacığından** çağrılır (coroutine/main thread'den değil).
- `listener.onEvent(event)` — motordan gelen her şey tek yerden: bağlantı durumu, gelen mesaj, teslim alındısı, dosya ilerlemesi/geldi, PTT başladı/parça geldi/bitti, signaling baytı geldi, medya paketi geldi, ölçümler.
- Çağrılar: `connect(peer)`, `sendText(peer, text)`, `sendFile(peer, path)`, `cancelFile(h)`, `pttOpen(peer, codec, bundleMs)`, `pttPush(h, bytes)`, `pttClose(h)`, `sendSignaling(peer, bytes)`, `mediaSend(peer, bytes)`, `onPlatformSignal(signal)`, `drainBackground()`, `metrics()`, `stop()`.

**H1 — Gerçek motoru bağla**
- Neden: Uygulama bugün sahte motorla çalışıyor; iki telefon gerçek motorla konuşmalı.
- Ne: `.aar`'ı projeye al. `RealKamaEngine`'de: `start()`'ı arka plan iş parçacığında çağır; `onEvent`'ten gelenleri mevcut `ConnectionMapper` ile StateFlow'a çevir. Facade'ın şu çağrılarını PoC'de **etkisiz** bırak (gövde boş, hata değil): push kaydı, trust-failure yeniden deneme, rehber eşitleme, cihaz listesi (yalnız bu cihaz). `Enrollment` = provisioning dosyasını içe aktar → `Ready`. `Contacts` = dosyadaki liste. `background.drain()` = `drainBackground()`.
- Nereye: Hasan'ın Android reposu (RealKamaEngine, ConnectionMapper).
- Bağımlılık: `.aar` (Abkarada, A3 sonrası, hedef Gün 6).
- Done: Sahte ve gerçek motor aynı UI testlerini geçiyor; iki telefon hub üzerinden metin alışverişi yapıyor.

**H2 — Ses yolu: PTT ve arama için baytları taşı**
- Neden: Motor ses üretmez, yalnız bayt taşır; mikrofon/hoparlör ve Lyra kodlayıcı Hasan'ın ses katmanında zaten var.
- Ne: PTT: basınca `pttOpen(peer, "lyra", bundleMs)` → mikrofon → Lyra kodla → `pttPush(h, bytes)`; bırakınca `pttClose(h)`. Alıcıda `PttBundle` olayı → Lyra çöz → çal; parça gelmezse **durur, bozuk ses üretmez**, gelince devam eder ("bekliyor" göstergesi). Arama için iki **geçiş** işlevi: medya motorundan gelen signaling baytını `sendSignaling(peer, bytes)`'e ver, `SignalingReceived` olayını medya motoruna ver; medya paketleri için aynısı `mediaSend` / `MediaPacket` ile. İçeriğe bakılmaz; ne gönderileceği Resul'un programı.
- Nereye: Hasan'ın ses katmanı (AAudio ADM, Lyra sarmalayıcı) + RealKamaEngine.
- Bağımlılık: H1.
- Done: İki telefon arasında PTT: bas → karşıda ses ≤ 1,5 s (Wi-Fi); kötü ağda ses bozulmuyor, duruyor. Arama baytları iki yönde geçiyor (içeriği Resul test eder).

**H3 — Arka planda canlı kal, ağ değişimini bildir**
- Neden: Demoda uygulama arkada çalışırken karşılaştırma yapılacak (rakiplerin bağlantısı zaten açık); ağ değişince motor bunu hemen bilmeli.
- Ne: `specialUse` tipinde kalıcı foreground service (APK yan yüklü, Play beyanı yok); `ConnectivityManager` ağ değişimi → `onPlatformSignal(LinkChanged)`; uygulama öne/arkaya geçince `onPlatformSignal(Foreground/Background)` (motor keepalive aralığını buna göre seçer). Ekran kilitli ve Doze'da 30 dk bağ kopmamalı.
- Nereye: Hasan'ın servis katmanı.
- Bağımlılık: H1.
- Done: 30 dk arkada bağ kesintisiz (overlay/hub logu); Wi-Fi→mobil geçişinde 10 s içinde yeniden bağlı.

**H4 — Demo ekranları (Meriç tasarlar, Hasan yapar)**
- Neden: Demo beş işi gösterecek, başka hiçbir ekran gerekmiyor.
- Ne: (1) İçe aktarma: dosya seç → uygulama özel dizinine kopyala → `start`. (2) Kişiler: dosyadaki liste, bağlantı durumu renkli. (3) Sohbet: metin ("gönderiliyor" → "teslim edildi"), fotoğraf (yeniden kodlama ≈150 KB, EXIF/GPS atılır, ilerleme çubuğu). (4) PTT düğmesi. (5) Arama düğmesi (medya motorunun ekranı/API'si). Güvenlik bayrakları: `FLAG_SECURE`, `allowAudioPlaybackCapture="false"`, ses tipi `VOICE_COMMUNICATION`. Ayarlar, profil, bildirim tercihleri, okundu bilgisi **yok**.
- Nereye: Hasan'ın UI modülleri; tasarım Meriç'ten.
- Bağımlılık: H1, H2.
- Done: Beş iş iki telefonda el ile geçiliyor (Wi-Fi).

**H5 — Ölçüm overlay'i**
- Neden: Demoda "Kama daha iyi" iddiası ekranda sayıyla görünmeli ve kayda geçmeli.
- Ne: Ekranın köşesinde yarı saydam kutu: RTT, kayıp %, bitrate, PTT bas→ilk ses (ms), oturum durumu, hub'dan gelen son paketin yaşı. Hepsi `metrics()`'ten; saniyede bir yenilenir; aynı satırlar CSV'ye yazılır (zaman damgalı). "Sıcak" göstergesi: oturum onaylı + hub'a bağlı + son 25 s'de hub'dan paket gelmiş → yeşil.
- Nereye: Hasan'ın UI modülü.
- Bağımlılık: H1; `rtt/loss/bitrate` alanları Abkarada'nın A5'iyle dolar (Gün 4–5), o zamana kadar "—" gösterilir.
- Done: DEMO-01 provasının CSV dosyası.

### Medya ekibine arayüz notu (görev değil — Resul'un programı KM-PLAN-001'dir)

Medya motoru **hub'ı bilmez, Kama transport'unu kullanmaz, hiçbir Kama belgesini okumak zorunda değildir.** Kama ona iki bayt kanalı verir, ikisi de Hasan'ın köprüsünden geçer:
1. **Signaling kanalı** — `sendSignaling(peer, bytes)` / `SignalingReceived`: uçtan uca şifreli, güvenilir, sıralı; içerik tamamen medya motorunun (protobuf; arama teklifi/yanıtı/bitişi; `call_id` + sıra numarası içeride olmalı, Kama bu kanalda tekrar-eleme yapmaz). **Medya anahtarının kaynağı açık karar [S4]:** (a) 32 B rastgele anahtar bu mesajın içinde gider, Kama türetmez; ya da (b) PoC-KAPSAM EK-3'ün dediği gibi oturumdan türetilir ve Kama bir `exportMediaKey` benzeri API verir. Abkarada + Resul, A3 başlamadan.
2. **Medya paket kanalı** — `mediaSend(peer, bytes)` / `MediaPacket`: her çağrı tek bir UDP paketi olur (≤ 1200 B); ARQ yok, sıralama yok, geç gelen paket medya motorunun jitter tamponunda düşer. Rust tarafı bu paketleri hub üzerinden **aynı soketten, ayrı bir hatla** taşır (`RELAY_OPEN`); medya motoru bunu görmez. **[S4]**
Kama'nın medyadan beklediği tek şey: paketleri kendi anahtarıyla şifrelemesi (el sıkışma yok → 0-RTT) ve kurulum/codec/bundle/görüntü kararlarını kendi programında vermesi.

### P — Hayri (application)

**P1 — WS-B devir + AMP bakımı**
- Ne: AMP başlık/statü güncel; 0x0B FILE_RECEIPT ve 0x0C ret vakası tutarlı; §3 kuralları (PTT-R0…R5, dosya zamanlayıcıları, signaling tekrar sunumu) tek listede.
- Nereye: `specs/application-message-protocol.md`, `vectors/amp/`.
- Done: word_coverage yeşil; BL'de WS-B sahibi Hayri (SHA ile).

**P2 — Uygulama seviyesi INT-04 (nightly)**
- Neden: Geçen sprintte motorlar yalnız birim testlerle ve INT-02'de TEXT ile sınandı; dosya/PTT/signaling uçtan uca gece testine girmeli.
- Ne: İki `KamaNode` süreç-içi röleyle, netem altında: TEXT + dosya + PTT + signaling; oturum değişimi; 60 s kesinti. Nightly'e eklenir.
- Nereye: `crates/kama-integration/tests/int04_app.rs`, `.github/workflows/nightly.yml` (yalnız adım ekleme).
- Done: Nightly'de yeşil koşuyor.

**P3 — (isteğe bağlı) Temizlik**
- Ne: `EntropySource` tek tanım; `cargo doc` uyarıları; BL/KATMAN 6 hijyeni.

---

## BÖLÜM 3 — Bu sprinte girmeyenler
BestEffort/BatchPayload · hop anahtarı (M4) · INIT'e gömülü 0-RTT · zstd · uzun mesaj bölme · okundu bilgisi · mesaj geçmişi kalıcılığı · Keystore/TEE · PKI/RA/enrollment/discovery/DHT · çoklu hub · mailbox · push/SMS. Sprint 3 = saha hazırlığı (ısınma protokolü, ölçüm raporu şablonu, demo senaryosu, "sorulursa" cevapları).
