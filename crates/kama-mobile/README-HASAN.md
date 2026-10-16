# kama-mobile — Hasan için teslim notu (K4G-15, `poc-uniffi-v0`)

**Paket.** `jniLibs/arm64-v8a/libkama_mobile.so` · `kotlin/uniffi/kama_mobile/kama_mobile.kt` (UniFFI 0.32.2 üretimi, paket `uniffi.kama_mobile`) · `KM-SEAM-AK-001.md` (ui-api v0.3 ↔ Rust eşlemesi, satır satır) · `PROVISIONING-FORMAT-v0.md` · `device-alice.json` (örnek; `kama-provision` ile paketleme anında üretildi — `root.json` pakette yoktur, D9).

**Ekleme.** `.so` → `src/main/jniLibs/arm64-v8a/`; `.kt` → kaynak ağacı (elle değiştirme; her sürümde yeniden üretilir). Bağımlılık yalnız JNA 5.x: `implementation("net.java.dev.jna:jna:<5.x>@aar")`; kütüphaneyi JNA yükler (`System.loadLibrary` gerekmez). R8 açıksa: `-keep class com.sun.jna.** { *; }` ve `-keep class uniffi.kama_mobile.** { *; }`. Yalnız arm64-v8a, minSdk 28, 16 KB sayfa hizalı.

**Başlatma.** Provisioning dosyasını `noBackupFilesDir`'e yaz; sonra ana iş parçacığı DIŞINDA bir JVM iş parçacığından (disk ve soket açar):

```kotlin
val node = KamaMobile.start(configPath, dataDir, listener) // dataDir: uygulamaya özel, yoksa oluşturulur
```

Kapanış: `node.shutdown()`, sonra `node.close()`. Kapanıştan sonra her çağrı `NodeException.Stopped` atar; aynı `dataDir` ile yeniden `start` edilebilir.

**Olay akışı.** `EngineListener.onEvent(ev)` düğümün kendi iş parçacığında çağrılır: hızlı dön (olayı `Channel`/`StateFlow`'a koy). İçinden yanıt bekleyen çağrı `NodeException.ReentrantCall` alır; attığın istisna yutulur, düğüm etkilenmez. `connect(peer)` → `ConnectProgress` → `Connected` · `sendText` → karşıda `TextReceived`, sende `Receipt(msgId)` (= teslim edildi) · `sendFile(peer, path)` (yerel dosya yolu, `content://` değil) → `FileProgress(SENT, DELIVERED)`, karşıda `FileReceived(path)` · `pttOpen`/`pttPush`/`pttClose` → karşıda `PttOpened` → `PttBundle`… → `PttClosed` · `sendSignaling` → `SignalingReceived` · saniyede bir `Metrics`. Hatalar tiplidir: `NodeException` (`NodeError` aynası; ör. `Busy`, `TooLarge`, `Ptt(PttError.Transport(TransportError.BACKPRESSURE))` → bekle ve aynı girdiyi yeniden ver).

**Zorunlu notlar** (plan K4G-15 Ek (5) ve DUR-K4G-11b):

1. Bu sürümde iki telefon arası oturum YOK — el sıkışma zarfı taşıyıcısı hub Plane 1 rölesidir (hub sprinti); bu `.so` API entegrasyonu ve süreç-içi loopback (iki `KamaNode` aynı süreçte) içindir
2. `start()` async/tokio bağlamından çağrılmaz; JVM iş parçacığından çağrılır
3. `metrics()`'te `rtt_ms`/`loss_pct`/`bitrate_kbps` bu sürümde `None`; overlay alanları hub sprintinde dolar
4. Sinyalleşme (`sendSignaling`) alındısız ve dedupsuz: içerik kendi `call_id`/sıra numarasını taşımalı; oturum değişiminde 60 s içindeki bloblar yeniden sunulabilir
5. `pttOpen(peer, codec, bundleMs)` — codec ve demet süresi çağıran verir
6. Arayüz "Bağlandı" olayını (`EngineEvent.Connected`) bekler; oturumsuz `sendText` → `NodeException.Messenger(MessengerError.NoSession)` davranışı kalır (DUR-K4G-11b)

**Ayrıca.** Kotlin'de alan adları camelCase'dir (`rttMs`, `lossPct`, `bitrateKbps`). Örnek `device-alice.json`'da alice'in veri adresi `127.0.0.1:47001`; `bob` eşi dosyada var ama onun cihaz dosyası pakette yok — telefonda `connect("bob")` bekler, `sendText("bob", …)` `NoSession` alır. `EngineEvent`'in Kotlin `toString()`'i metni, ses yükünü ve blob'u içerir: olayı loglama.
