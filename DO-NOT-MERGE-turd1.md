# ⛔ DO-NOT-MERGE — TUR-D1 (stale-btlbw reconciliation)

**Statü: DONDURULMUŞ-ASKIDA (Abkarada, 2026-07-15). main'e MERGE EDİLMEZ.**

## Neden askıda
TUR-D1-shrink DESEN-1-flood'unu ölçümde KESMEDİ (dal-B/KIRMIZI; scratchpad/turd1-olcum.md):
P6-olay-aralığı (≥srtt) + reseed-bayatlık ikisi de srtt-şişkinliğine bağlı → reset t=66'da geç,
btlbw 4120→1675 (kanal-375'in 4.5×), flood sürdü (srtt-max 53s, infl 19491; osc 39516).

## Kader Y3-ölçümüne bağlı (srtt-decoupling-v1)
- srtt-decoupling (re-clock'lu-W) bayat-max'ı HIZLI-expire ederse → TUR-D1 GEREKSİZ (arşiv).
- Etmezse (51-64s+ yavaş-self-stop) → TUR-D1 aktif-hızlı-demotion-katmanı olarak TAMAMLAR
  (ama P6→rtprop + reseed→en-taze rebase edilmiş YENİDEN-İNŞA; mevcut-form as-is çalışmaz).
- "Arşiv" premisi (~10s-self-stop) çürüdü (yalnız-reddedilen-aday-A'ya aitti); "gerekli" de erken.

## Kod durumu (CLAUDE.md §6 kapanışı)
shrink_apply_event/shrink_reset bu branch'te CANLI (feature-flag-yok). Bu marker DO-NOT-MERGE'i
KİLİTLER → main-temiz kalır, WIP-süpürme-sızma-riski (PR#31-emsali) kapanır. Y3-ölçümü-sonrası:
ya YENİDEN-İNŞA (bu kodu supersede) ya appendix+MUST-NOT (removal).
