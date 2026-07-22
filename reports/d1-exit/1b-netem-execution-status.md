# AD — Gerçek-Netem-Koşusu: TYPED-EXACT-BLOCKER [sahte-sonuç-ÜRETİLMEDİ]

Sertleştirilmiş-harness (AC) GERÇEKTEN-koşuldu; yetenek-tespiti-aşamasında-typed-blocker'la-çıktı
(exit-4). Gerçek-çıktı (NETEM-BLOCKER.txt, aynen):
```
missing_capability=root/CAP_NET_ADMIN
failed_command=sudo -n true
exit_status=1
stderr=sudo: a password is required
runner_readiness=READY (loopback-smoke-yeşil; bkz. udp_runner_smoke.rs)
remaining_manual_action=self-hosted-nightly'de (root/CAP_NET_ADMIN) bu-script'i NETEM_OUT ile koş
```
Runner-hazırlık-kanıtı: udp_runner_smoke (3-politika-loopback; waste=0; parity-canlı-UDP'de) — "netem-
yalnız-capability-nedeniyle-bloklu"-ifadesi-artık-DOĞRU (runner-eksiği-kapandı). Sim↔netem-yön-tablosu
İSKELET-halinde-bekliyor (Z4-raporu); netem-koşusu-self-hosted-nightly'de → tablo-doldurulup-karar-
kuralı (§14) uygulanacak. NETEM'SİZ-PRODUCTION-DEFAULT-SEÇİLMEZ-kuralı-uygulanıyor.
