# DR-CONNECT-1 — `connect()` davranışı: "kalıcı niyet, sınırlı deneme"

**Karar sahibi:** Abkarada · **Tarih:** 2026-10-03 · **Bu belge İNDEKSTİR:** normatif metin spec'lerde yaşar; burada yalnız hüküm + işlendiği yer durur.

**Sorun:** S1 `connect(peer)` için bir deadline sabiti (KS-8) öngörüyordu. Kanıt bunun kurulamayacağını gösterdi: transport hub kanalı kuruluşunda pes etmiyor; "Plane 1 `Reliable` deadline'ı" yapısal olarak yok; STORE_MESSAGE yanıtı "doğrudan teslim / mailbox" ayrımını senkron taşıyamıyor. **Karar:** NİYET zamanla ölmez; DENEME maliyeti sınırlanır; sonlandırma yalnız olay-tetiklidir.

| # | Hüküm (özet) | Durum | Normatif metin |
|---|---|---|---|
| H1 | **INV-NO-IDLE-TEARDOWN:** oturum / bekleyen niyet zaman geçtiği için sonlandırılmaz | işlendi | transport-layer-tdd §0 · identity §16 |
| H2 | Sonlandırma nedenleri kapalı liste: (a) cancel · (b) karşı tarafın close'u · (c) anahtar hijyeni · (d) kalıcı yerel hata | işlendi | transport-layer-tdd §0 · identity §16 · KM-SEAM-AT-001 §4 |
| H3 | Prob maliyeti sınırlı: tavan bağlama göre 8 s / 25 s / OS · link yokken 0 emisyon · anında prob · zorunlu jitter | işlendi (SPEC-ONLY, altı beyanlı) | transport-layer-tdd §10 [AMENDMENT-10] |
| H4 | Hop heartbeat = prob = NAT keepalive; mevcut CS_PROBE/ECHO; KS-HB-1 | **UYGULANMADI — sahip kararı bekliyor** (aşağıda) | — |
| H5 | **KS-PN-1 KAPANDI: K = 1024**; tavanda REBIND → floor prob → hop-ACK'le devam | işlendi | transport-layer-tdd §12 [AMENDMENT-9] · engine-spec §3.3-B/§11/§11.1 · ccr §35.15 |
| H6 | Peer-restart supersession: bilinen eşten yeni INIT → `Ready(h') → Closed(h, Superseded)` | işlendi | identity §16 · KM-SEAM-AT-001 §1 (K7), §6 |
| H7 | DELIVER → mailbox fallback normatif; silme yalnız ACK_RANGE | işlendi | app-rpc §6.11 |
| H8 | SESSION_MAX_AGE → RenewRequired → cutover; timeout değil, anahtar hijyeni; **KS-AGE-1 AÇIK** (öneri 7 gün) | işlendi (değer açık) | identity §16 · ccr §35.15 · KM-SEAM-AT-001 §4 |
| H9 | S1 API: `ConnectFailed` kalktı; `cancel(ticket)`, `ConnectAbandoned(ticket, reason)` (opak değil), `ConnectProgress(ticket, phase)` | işlendi — **Hayri imzası bekliyor** | KM-SEAM-AT-001 §1, §3, §4, §6, §7, §8 |
| H10 | **KS-8 KAPANDI: deadline yoktur**, sayı yok | işlendi | KM-SEAM-AT-001 §4 · ccr §35.15 |
| H11 | T-18c: 7 günde ephemeral silinir, niyet korunur, yeni INIT | işlendi | identity §21 T-18c, §16 · KM-SEAM-AT-001 §4 |
| H12 | Ürün notu: Google servissiz dağıtımda arka plan erişilebilirliği foreground service ister | işlendi | docs/PARK-LISTESI.md P-3 |
| H13 | OQ-14 AÇIK kalır (hop heartbeat E2E değildir) | işlendi | identity §28 OQ-14 |
| — | **KS-HANDLE-1 KAPANDI:** `HANDLE_TOMBSTONE` = S&F mailbox TTL (bugün 7 gün) | işlendi (K2) | ccr §35.7 |

**Kanıt (K1–K5), ayrıntı `docs/sprint_plan/A1-KALIBRASYON-KARAR-FOYU.md` → KANIT PAKETİ 2:**
K1 çerçeve var (ürün kabuğu 0x06/0x07; noise-wire 0x19/0x1A), 44/43 B mühürlü; hub her açılan datagramda PeerSlot'u yeniliyor — ama üç normatif çakışma var (aşağıda) · K2 handle oturum ömürlü, mailbox'tan ESKİ handle'la çıkıyor; gerçek hub repoda yok · K3 jitter yok, transport'ta RNG yok · K4 hub kodu yok; fallback yalnız ima ediliyordu · K5 link/power sinyali ve UniFFI bağlaması yok.

**H4 neden uygulanmadı (K1):** "mevcut çerçeve, yeni codepoint yok" iddiası çerçeve *yerleşimi* için doğru, *kuralları* için değil. (1) `probe_seq` u16 ve yeniden kullanılamaz, wrap → yeni prob YOK (INV-CS-7, noise-wire §2.57): 8 s kadansta ≈6 gün, 25 s'de ≈19 günde heartbeat kalıcı olarak susar — INV-NO-IDLE-TEARDOWN ile çelişir. (2) Prob bugün bütçeli bir merdivendir (4 atış 3/9/21/45 s, 200 B sert bütçe INV-CS-2; "RETRY-EDİLMEZ"), sürekli yayıcı yok ve ürün varsayılanında kapalı. (3) ccr §35.16 "client keepalive YOK" normatif (aynı ifade transport-layer-tdd §4.4.3 blackout metriğinde "no-keepalive" olarak geçiyor). Ayrıca kodda HubChannel ctx yok (prob veriyle aynı PN uzayında mühürleniyor) ve bilinmeyen eşten gelen ilk geçerli datagram PeerSlot YARATIYOR ("state yaratmaz" bugün doğru değil; `max_peers = 64` ile sınırlı). **Gereken karar:** heartbeat için wrap'e izin veren ayrı `subtype` mi, HubChannel-epoch başına sıfırlanan sayaç mı, yoksa yeni codepoint mi; ve §35.16 satırının akıbeti.

**Açık kalanlar:** KS-AGE-1 değeri · H4 + KS-HB-1 · `LongOutage` eşiğinin tanımı · alıcıda Renew ↔ peer-restart ayrımı (ikisi de "bilinen eşten yeni INIT") · eşzamanlı INIT (glare) · S3'te `CloseReason::Superseded` (KM-SEAM-TC-001'e işlenmedi) · jitter kaynağı (motor determinizmi) · S1 için Hayri imzası · OQ-14.
