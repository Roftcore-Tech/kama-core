#!/usr/bin/env python3
"""INT-02 analizi (plan K4G-05): `int02_run.sh` çıktılarından tohum × kol tabloları ve GATING hükmü.

    python3 scripts/netem/int02_analyze.py <çıktı-dizini> [--md DOSYA] [--json DOSYA] [--criteria DOSYA]
    python3 scripts/netem/int02_analyze.py --selftest

Girdi: `<dizin>/seed-<S>-<kol>.json` (örnek `int02_node`'un satırı + sürücünün `driver` ve
`netem_check` kayıtları). Örnek yalnız GÖZLEM yazar; hüküm YALNIZ burada verilir (tek kaynak).
Çıktı: rapora (`docs/reports/INT-02-RAPORU.md`) girecek Markdown tablolar (stdout ya da `--md`).

HÜKÜM KURALLARI (her tohum ayrı; "temiz" = aşağıdaki ortak koşulların hepsi):

  ÖLÇÜM ARACI (her koşu, kol fark etmez) — netem ateşledi:
    ping: gönderilen > 0, alınan < gönderilen (kayıp görüldü), ort. RTT ≥ RTT_FLOOR_MS;
    qdisc: koşu boyunca düşürülen paket > 0. Sağlanmazsa koşu GEÇERSİZ (hüküm verilmez).
  ORTAK TEMİZLİK (iki düğüm): Application retransmit = 0 (`resent_on_new_session` +
    `resend_refused`), Crypto retransmit = 0 (bayt-özdeş el sıkışma zarfı tekrarı),
    `ctx_mismatch_drops` = `hub_frames_dropped` = `delivery_channel_drops` = 0, dinleyici olay
    düşürmedi (`listener_drops` = 0).
  GATING (1) `gating1`: koşu tamamlandı; alice→bob `count` TEXT tam bir kez teslim (teslim =
    gönderilen = `count`, tekrar / içerik uyuşmazlığı / bilinmeyen 0) ve `count` RECEIPT; + ortak.
  GATING (2) `gating2`: bob→alice için aynı teslim koşulu; + epoch 0→1 AKIŞ İÇİNDE: bob'un ilk
    gönderiminde örneklenen epoch'u 0, bob'un epoch-1 ilk görülüşü ilk gönderimden SONRA, iki
    düğümün son epoch'u 1; + TEK OTURUM: iki düğümde `Ready` = 1, `Closed` = 0, `RenewRequired` = 0,
    canlı oturum = 1, `Closed`/`ConnectAbandoned` düğüm olayı 0; + ortak.
  GATING (6) `gating6`: iki yönde teslim koşulu; iki düğümde tek `Connected`, `Closed` /
    `ConnectAbandoned` yok; + ortak. (Erken metin ve "≤ 2 RTT" el sıkışma ölçütü bu düzenekte
    ölçülmez — rapor beyanı.)
  NEGATİF `negatif` (kurtarma KAPALI): ölçümün kaybı GÖREBİLDİĞİ — netem geçerli, alice→bob
    teslim < gönderilen, iki düğümün bastırılan kurtarma kararı toplamı > 0 ve tele çıkan ARQ
    retransmit baytı 0.
  T4: her GATING kolu için tohum sayısı ≥ 5 ve her tohum GEÇTİ.

Yüzdelikler örnekte EN YAKIN SIRA (nearest-rank) yöntemiyle hesaplanır; burada yeniden hesaplanmaz.

YENİDEN KOŞU ALANLARI (CEVAP PAKETİ 4, şema `int02-node-v2`; hüküm kuralları DEĞİŞMEDİ):
  * `params.window_s` + `window.ended_by` — PENCERE: son gönderimden (ek-kesinti kolunda kesinti
    bitiminden) sonra tamamlanma üst sınırı (v1 dosyalarında `params.drain_s`).
  * GATING (2) özeti: geçen / koşulan tohum ve pencere(ler).
  * ek-kesinti: iki ayrı satır grubu — METİN (`outage.texts_complete`: %100 metin teslimi) ile
    `outage.same_session` (kesinti başından koşu sonuna yeni `Closed` yok, ikinci `Connected` yok,
    epoch değişmemiş) ve ayrı PTT tablosu. "Sahip ölçütü (CEVAP PAKETİ 4)" satırı = metin %100 ∧
    same_session (ölçüm özeti; GATING hükmü değildir).
  * ek-dosya-ptt: `file.outcome` (durum `delivered` / `failed` / `pending`, neden, zaman damgaları).

TAKILI BİRİM ÖZETİ (K4G-05 teşhis; hüküm DEĞİL): her koşu için tek satır — son sondadaki gönderen
kurtarma görüntüsü (`nodes.*.probe.sessions[].recovery`: teyitsiz birim, en eski teyitsiz birimin
emisyon kararı / ARQ retx düzeyi ve tavanı / motor kayıtları / kopya vadesi ve aralığı / son
gönderimden ve son ACK'ten beri, tail-probe turu ve PTO, motor durumu) ve alıcı sıralı teslim durumu
(`reorder`: beklenen birim, arkasında tutulan). Zaman çizelgesi örnek sayısı `timeline.samples`.

M1 / M2 TEŞHİS (CEVAP PAKETİ 6; hüküm DEĞİL; şema `int02-node-v3`): `m_log` ACK çerçevesi ve motor durum
değişikliği günlüklerinden bayat zamanlayıcı SACK tabloları (M1: P / S / N sınıfları, varış ve alıcı
üretim aralıkları, ilerlemesiz diziler, RecoverFwd girişlerinin konumu, eski kayıt doğrulaması) ve
zaman çizelgesinin `motor` alanlarından kesinti öncesi / sırası / sonrası pacer–BtlBw tabloları (M2:
evreler, tabanda oran, toparlanma). Tanımlar docs/reports/INT-02-M1M2-BEKLENTI.md'dedir (ölçümden ÖNCE
yazıldı); eşikler o dosyanın ÖLÇÜT bloğundan okunur (`--criteria`) ve dosyanın sha256'sı çıktı
başlığına ve JSON özetine (`m12.criteria_sha256`) yazılır. Ölçüt dosyası okunamazsa bölüm atlanır
(stderr). `m_log` taşımayan (v1 / v2) dosyalarda bölüm boştur.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
import tempfile
from pathlib import Path

RTT_FLOOR_MS = 100.0
T4_MIN_SEEDS = 5
GATING_ARMS = ("gating1", "gating2", "gating6")
ALL_ARMS = GATING_ARMS + ("negatif", "ek-dosya-ptt", "ek-kesinti")
FILE_RE = re.compile(r"^seed-(\d+)-([a-z0-9-]+)\.json$")
DASH = "—"


# ── okuma ────────────────────────────────────────────────────────────────────────────────────

def load(directory: Path) -> dict[tuple[int, str], dict]:
    runs: dict[tuple[int, str], dict] = {}
    for path in sorted(directory.iterdir()):
        m = FILE_RE.match(path.name)
        if not m:
            continue
        seed, arm = int(m.group(1)), m.group(2)
        try:
            runs[(seed, arm)] = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            runs[(seed, arm)] = {"result": "error", "error": f"okunamadı: {e}"}
    return runs


def get(d, *keys, default=None):
    for k in keys:
        if not isinstance(d, dict) or k not in d:
            return default
        d = d[k]
    return d


def node(rec, name):
    return get(rec, "nodes", name, default={}) or {}


def probe(rec, name):
    return get(rec, "nodes", name, "probe", default={}) or {}


def flow(rec, key):
    return get(rec, "flows", key, default={}) or {}


# ── hüküm ────────────────────────────────────────────────────────────────────────────────────

def netem_check(rec) -> list[str]:
    ping = get(rec, "netem_check", "ping", default={}) or {}
    delta = get(rec, "netem_check", "run_delta", default={}) or {}
    why = []
    sent, received, avg = ping.get("sent"), ping.get("received"), ping.get("rtt_avg_ms")
    if not sent:
        why.append("ping kaydı yok")
    else:
        if received is None or received >= sent:
            why.append("ping kaybı görülmedi")
        if avg is None or avg < RTT_FLOOR_MS:
            why.append(f"ping ort. RTT < {RTT_FLOOR_MS:.0f} ms")
    if not delta.get("dropped"):
        why.append("koşu boyunca qdisc düşürmesi yok")
    return why


def common_clean(rec) -> list[str]:
    why = []
    for name in ("alice", "bob"):
        p = probe(rec, name)
        if not p:
            why.append(f"{name}: sonda yok")
            continue
        for key, label in (("app_retransmit", "app retransmit"),
                           ("crypto_retransmit", "crypto retransmit"),
                           ("ctx_mismatch_drops", "ctx_mismatch_drops"),
                           ("hub_frames_dropped", "hub_frames_dropped"),
                           ("delivery_channel_drops", "delivery_channel_drops")):
            if p.get(key) != 0:
                why.append(f"{name}: {label}={p.get(key)}")
    if rec.get("listener_drops") != 0:
        why.append(f"listener_drops={rec.get('listener_drops')}")
    return why


def flow_complete(rec, key) -> list[str]:
    f, count = flow(rec, key), get(rec, "params", "count")
    why = []
    for field in ("sent", "delivered", "receipts"):
        if f.get(field) != count:
            why.append(f"{key}.{field}={f.get(field)}≠{count}")
    for field in ("duplicate_deliveries", "mismatched_texts", "unknown_deliveries"):
        if f.get(field) != 0:
            why.append(f"{key}.{field}={f.get(field)}")
    return why


def single_session(rec) -> list[str]:
    why = []
    for name in ("alice", "bob"):
        ev, n = get(probe(rec, name), "session_events", default={}) or {}, node(rec, name)
        checks = ((ev.get("ready"), 1, "Ready"), (ev.get("closed"), 0, "Closed"),
                  (ev.get("renew_required"), 0, "RenewRequired"),
                  (probe(rec, name).get("sessions_live"), 1, "canlı oturum"),
                  (get(n, "events", "closed_session"), 0, "Closed olayı"),
                  (get(n, "events", "connect_abandoned"), 0, "ConnectAbandoned"))
        why += [f"{name}: {label}={got}≠{want}" for got, want, label in checks if got != want]
    return why


def epoch_in_flow(rec) -> list[str]:
    f = flow(rec, "bob_to_alice")
    why = []
    if f.get("first_send_epoch") != 0:
        why.append(f"bob ilk gönderim epoch'u={f.get('first_send_epoch')}≠0")
    first_send, epoch1 = f.get("first_send_ms"), get(node(rec, "bob"), "epoch", "first_epoch1_ms")
    if first_send is None or epoch1 is None or epoch1 <= first_send:
        why.append(f"bob epoch-1 ({epoch1}) ilk gönderimden ({first_send}) sonra değil")
    for name in ("alice", "bob"):
        last = get(node(rec, name), "epoch", "last")
        if last != 1:
            why.append(f"{name}: son epoch={last}≠1")
    return why


def connected_once(rec) -> list[str]:
    why = []
    for name in ("alice", "bob"):
        ev = get(node(rec, name), "events", default={}) or {}
        if ev.get("connected") != 1:
            why.append(f"{name}: Connected={ev.get('connected')}≠1")
        if ev.get("closed_session") or ev.get("connect_abandoned"):
            why.append(f"{name}: Closed={ev.get('closed_session')} Abandoned={ev.get('connect_abandoned')}")
    return why


def arq_sum(rec, field):
    total = 0
    for name in ("alice", "bob"):
        value = get(probe(rec, name), "arq_total", field)
        if value is None:
            return None
        total += value
    return total


def verdict(rec, arm) -> tuple[str, list[str]]:
    """('GEÇTİ' | 'KALDI' | 'GEÇERSİZ' | 'KAYIP GÖRÜNDÜ' | 'KAYIP GÖRÜNMEDİ' | 'NON-GATING', gerekçeler)."""
    if rec is None:
        return ("YOK", ["koşu dosyası yok"])
    netem = netem_check(rec)
    if netem:
        return ("GEÇERSİZ", ["netem: " + w for w in netem])
    if arm == "negatif":
        f = flow(rec, "alice_to_bob")
        why = []
        if not (f.get("sent") and f.get("delivered") is not None and f["delivered"] < f["sent"]):
            why.append(f"teslim {f.get('delivered')}/{f.get('sent')} — kayıp görünmedi")
        suppressed, wire = arq_sum(rec, "recovery_suppressed"), arq_sum(rec, "tx_retx_bytes")
        if not suppressed:
            why.append(f"bastırılan kurtarma kararı={suppressed}")
        if wire != 0:
            why.append(f"tele çıkan ARQ baytı={wire}")
        return ("KAYIP GÖRÜNMEDİ", why) if why else ("KAYIP GÖRÜNDÜ", [])
    if arm not in GATING_ARMS:
        return ("NON-GATING", [])
    why = []
    if rec.get("result") != "complete":
        why.append(f"sonuç={rec.get('result')} {rec.get('error') or ''}".strip())
    if arm == "gating1":
        why += flow_complete(rec, "alice_to_bob")
    elif arm == "gating2":
        why += flow_complete(rec, "bob_to_alice") + epoch_in_flow(rec) + single_session(rec)
    else:
        why += flow_complete(rec, "alice_to_bob") + flow_complete(rec, "bob_to_alice")
        why += connected_once(rec)
    why += common_clean(rec)
    return ("KALDI", why) if why else ("GEÇTİ", [])


# ── biçim ────────────────────────────────────────────────────────────────────────────────────

def fmt(value, nd=1):
    if value is None:
        return DASH
    if isinstance(value, bool):
        return "evet" if value else "hayır"
    if isinstance(value, float):
        return f"{value:.{nd}f}"
    return str(value)


def pair(a, b, nd=1):
    return f"{fmt(a, nd)} / {fmt(b, nd)}"


def lat(f, key="latency_ms"):
    s = f.get(key) or {}
    return f"{fmt(s.get('median'), 0)} / {fmt(s.get('p95'), 0)}"


def table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    out += ["| " + " | ".join(str(c) for c in row) + " |" for row in rows]
    return "\n".join(out)


def netem_cell(rec):
    ping = get(rec, "netem_check", "ping", default={}) or {}
    delta = get(rec, "netem_check", "run_delta", default={}) or {}
    loss = None
    if ping.get("sent"):
        loss = 100.0 * (ping["sent"] - (ping.get("received") or 0)) / ping["sent"]
    return (f"ping {fmt(ping.get('received'))}/{fmt(ping.get('sent'))} "
            f"(%{fmt(loss, 0)} kayıp, ort. {fmt(ping.get('rtt_avg_ms'), 0)} ms) · "
            f"qdisc %{fmt(delta.get('loss_pct'), 1)}")


def arq_cell(rec, field="retransmits"):
    return pair(get(probe(rec, "alice"), "arq_total", field), get(probe(rec, "bob"), "arq_total", field), 0)


def zero_cell(rec, key):
    return pair(probe(rec, "alice").get(key), probe(rec, "bob").get(key), 0)


def epoch_cell(rec):
    return pair(get(node(rec, "alice"), "epoch", "epoch1_after_connected_ms"),
                get(node(rec, "bob"), "epoch", "epoch1_after_connected_ms"), 0)


def window_cell(rec):
    """Pencere süresi (s) ve nasıl bittiği (v1 dosyalarında `params.drain_s`)."""
    seconds = get(rec, "params", "window_s")
    if seconds is None:
        seconds = get(rec, "params", "drain_s")
    return f"{fmt(seconds)} · {fmt(get(rec, 'window', 'ended_by'))}"


def mark(rec, name, which):
    return get(rec, "outage", "session", name, which, default={}) or {}


def transition(rec, field):
    """Kesinti başı → koşu sonu değeri, A / B."""
    return " / ".join(f"{fmt(mark(rec, n, 'start').get(field))}→{fmt(mark(rec, n, 'end').get(field))}"
                      for n in ("alice", "bob"))


def file_since_sent(rec, key):
    o = get(rec, "file", "outcome", default={}) or {}
    a, b = o.get("sent_ms"), o.get(key)
    return None if a is None or b is None else b - a


def outage_criterion(rec):
    """CEVAP PAKETİ 4: 60 s kesinti, AYNI OTURUMDA %100 metin teslimi (PTT ayrı). Ölçüm özeti."""
    if netem_check(rec):
        return "GEÇERSİZ"
    o = rec.get("outage") or {}
    ok = o.get("texts_complete") is True and o.get("same_session") is True
    return "KARŞILANDI" if ok else "KARŞILANMADI"


def fmt_s(ms):
    return DASH if ms is None else f"{ms / 1000.0:.1f} s"


def first_session(rec, name):
    sessions = probe(rec, name).get("sessions") or []
    return sessions[0] if sessions else {}


def stuck_cell(rec, name):
    """Bir düğümün gönderen kurtarma görüntüsü (son sonda) — tek hücre."""
    r = first_session(rec, name).get("recovery") or {}
    if not r:
        return "görüntü yok"
    t, u = r.get("timers") or {}, r.get("oldest_unacked")
    parts = [f"teyitsiz {fmt(r.get('unacked_units'))}"]
    if u:
        parts.append(f"en eski u{u.get('unit_id')}: {fmt(u.get('emission_decisions'))} karar, "
                     f"retx {fmt(u.get('max_retx_count'))}/{fmt(t.get('arq_retx_cap'))}, "
                     f"inf {fmt(u.get('inflight_records'))} tomb {fmt(u.get('tombstone_records'))} "
                     f"pl {fmt(u.get('pending_lost_retx'))}, son gönd. {fmt_s(u.get('since_latest_send_ms'))}")
        if u.get("dup_interval_ms") is not None:
            parts.append(f"kopya vade {fmt_s(u.get('dup_due_in_ms'))} aralık {fmt_s(u.get('dup_interval_ms'))}")
    parts.append(f"son ACK {fmt_s(r.get('since_last_ack_ms'))}")
    parts.append(f"tail {fmt(t.get('tail_probe_index'))}/{fmt(t.get('tail_max_probes'))} "
                 f"PTO {fmt_s(t.get('tail_pto_ms'))}")
    parts.append(f"arq‰ {fmt(t.get('arq_aggressiveness_permille'))}")
    parts.append(str(t.get("state", DASH)))
    return " · ".join(parts)


def reorder_cell(rec, name):
    ro = first_session(rec, name).get("reorder") or {}
    if not ro:
        return DASH
    return f"next {fmt(ro.get('next_unit'))} · tutulan {fmt(ro.get('held_units'))}"


def session_diag(rec, name, field):
    sessions = probe(rec, name).get("sessions") or []
    if len(sessions) != 1:
        return None
    return get(sessions[0], "diag", field)


# ── M1 / M2 teşhis ölçümü (CEVAP PAKETİ 6; hüküm DEĞİL) ─────────────────────────────────────
#
# Tanımlar ve eşikler docs/reports/INT-02-M1M2-BEKLENTI.md'dedir (ölçümden ÖNCE yazıldı); eşikler
# o dosyanın ÖLÇÜT bloğundan okunur, burada sabit olarak TEKRARLANMAZ.

CRITERIA_FILE = Path(__file__).resolve().parents[2] / "docs" / "reports" / "INT-02-M1M2-BEKLENTI.md"
CRIT_BEGIN, CRIT_END = "<!-- M12-OLCUT-BASLA -->", "<!-- M12-OLCUT-BITTI -->"
CRIT_TYPES = {
    "ACK_CADENCE_FLOOR_MS": float, "T_SPACING_MS": float, "ARRIVAL_TOL_MS": float,
    "EMISSION_TOL_MS": float, "ACK_DELAY_SATURATED_RAW": int, "MIN_STREAK_FRAMES": int,
    "MIN_RATE_BPS": int, "RECOVERY_FRACTION": float, "OLD_POST_WINDOW_S": float,
    "OLD_S9_BOB_ACK_S": list, "MAX_ROWS": int,
}
# RecoverFwd girişinin ilerlemesiz diziye göre konumu (beklenti kaydı §2).
WHERE = ("dizi öncesi sessizlikte", "bayat SACK'ler sürerken",
         "son bayat SACK'ten sonra, ilerlemeden önce", "dizi dışı")
LEVELS = ("Arq", "Dup2", "Dup3")


def load_criteria(path: Path) -> tuple[dict, str]:
    """Beklenti kaydının ÖLÇÜT bloğu → sözlük, ve dosyanın sha256'sı (bayt)."""
    data = path.read_bytes()
    text = data.decode("utf-8")
    begin, end = text.find(CRIT_BEGIN), text.find(CRIT_END)
    if begin < 0 or end < begin:
        raise ValueError(f"{path}: ÖLÇÜT bloğu ({CRIT_BEGIN} … {CRIT_END}) yok")
    crit: dict = {}
    for raw in text[begin + len(CRIT_BEGIN):end].splitlines():
        line = raw.strip()
        if not line or line.startswith("```") or line.startswith("#"):
            continue
        key, sep, value = line.partition("=")
        key, value = key.strip(), value.strip()
        if not sep or key not in CRIT_TYPES or key in crit:
            raise ValueError(f"{path}: tanınmayan ya da tekrarlı ölçüt satırı {raw!r}")
        kind = CRIT_TYPES[key]
        crit[key] = [float(v) for v in value.split()] if kind is list else kind(value)
    missing = sorted(set(CRIT_TYPES) - set(crit))
    if missing:
        raise ValueError(f"{path}: eksik ölçüt {missing}")
    return crit, hashlib.sha256(data).hexdigest()


def nearest_median(values):
    """En yakın sıra medyanı (örnekteki `summary` ile aynı yöntem)."""
    values = sorted(v for v in values if v is not None)
    return values[max(1, math.ceil(0.5 * len(values))) - 1] if values else None


def m_logs(rec):
    return get(rec, "m_log", "sessions", default=[]) or []


def ack_classes(acks):
    """P (ilerlemeli) / S (bayat) / N (ilerlemesiz, yeni datagramlı) — beklenti kaydı §2."""
    classes, prev_seen = [], None
    for a in acks:
        seen = a.get("seen") if a.get("v2") else None
        if (a.get("new_units") or 0) > 0:
            cls = "P"
        elif seen is not None and prev_seen is not None and seen == prev_seen:
            cls = "S"
        else:
            cls = "N"
        if seen is not None:
            prev_seen = seen
        classes.append(cls)
    return classes


def motor_progress(a):
    """Motorun kendi ilerleme tanımı (tail zamanlayıcısı yeniden kuruldu) — beklenti kaydı §2."""
    before, after = a.get("before") or {}, a.get("after") or {}
    if not a.get("mapped"):
        return "işlenmedi"
    if a.get("tail_rearmed_now"):
        if after.get("tail_probe_index") == 0 and after.get("tail_probes") == before.get("tail_probes"):
            return "ilerleme"
        return "belirsiz"
    return "yok"


def multiple_of(delta, base, tol):
    if delta is None or base <= 0:
        return None
    k = round(delta / base)
    return k if k >= 1 and abs(delta - k * base) <= tol else None


def interval_class(delta, crit, tol):
    """`taban×k` (ACK_CADENCE_FLOOR katı) / `kota×j` (T_SPACING katı) / `diğer`."""
    k = multiple_of(delta, crit["ACK_CADENCE_FLOOR_MS"], tol)
    if k:
        return f"taban×{k}"
    j = multiple_of(delta, crit["T_SPACING_MS"], tol)
    return f"kota×{j}" if j else "diğer"


def time_in_state(states, lo, hi, name):
    """`[lo, hi]` aralığında `name` durumunda geçen süre (ms), durum günlüğünden."""
    total = 0.0
    for k, s in enumerate(states):
        if s.get("to") != name:
            continue
        a = s["t_ms"]
        b = states[k + 1]["t_ms"] if k + 1 < len(states) else math.inf
        x, y = max(a, lo), min(b, hi)
        if y > x:
            total += y - x
    return total


def timeline_motor(rec, side, session=None, peer=None):
    """Zaman çizelgesinden bir ucun örnekleri (oturum ya da eş adıyla seçilir):
    `(t_ms, oturum, motor, kurtarma görüntüsü)`."""
    rows = []
    for sample in get(rec, "timeline", "samples", default=[]) or []:
        for s in (sample.get(side) or {}).get("sessions") or []:
            if not s.get("motor"):
                continue
            if (session is not None and s.get("session") == session) or (
                    session is None and s.get("peer") == peer):
                rows.append((sample.get("t_ms"), s.get("session"), s["motor"], s.get("recovery") or {}))
                break
    return [r for r in rows if r[0] is not None]


def streak_window(st):
    """RecoverFwd girişinin diziye atfedildiği aralık: önceki P'den diziyi kapatan P'ye (açıksa sonsuz)."""
    lo = st["prev_P_ms"] if st["prev_P_ms"] is not None else st["start_ms"]
    return lo, (st["end_ms"] if st["closed"] else math.inf)


def m1_session(rec, log, crit):
    """Bir gönderen ucunun ACK günlüğü → sınıflar, aralıklar, diziler, RecoverFwd girişleri."""
    acks = [a for a in (log.get("acks") or []) if a.get("t_ms") is not None]
    states = [s for s in (log.get("states") or []) if s.get("t_ms") is not None]
    classes = ack_classes(acks)
    sat = crit["ACK_DELAY_SATURATED_RAW"]
    arrivals, emissions = [], []
    for k in range(1, len(acks)):
        a, b = acks[k - 1], acks[k]
        if classes[k] != "S" or b.get("i") != (a.get("i") or 0) + 1:
            continue
        delta = b["t_ms"] - a["t_ms"]
        arrivals.append({"t_ms": b["t_ms"], "delta_ms": delta,
                         "class": interval_class(delta, crit, crit["ARRIVAL_TOL_MS"])})
        if (classes[k - 1] == "S" and a.get("largest_pn") is not None
                and a.get("largest_pn") == b.get("largest_pn")
                and (a.get("ack_delay_raw") or 0) < sat and (b.get("ack_delay_raw") or 0) < sat):
            d = b["ack_delay_ms"] - a["ack_delay_ms"]
            emissions.append({"t_ms": b["t_ms"], "delta_ms": d,
                              "class": interval_class(d, crit, crit["EMISSION_TOL_MS"])})
    streaks, k = [], 0
    while k < len(acks):
        if classes[k] == "P":
            k += 1
            continue
        j = k
        while j < len(acks) and classes[j] != "P":
            j += 1
        if j - k >= crit["MIN_STREAK_FRAMES"]:
            frames = range(k, j)
            s_frames = [m for m in frames if classes[m] == "S"]
            closing = acks[j]["t_ms"] if j < len(acks) else None
            start = acks[k]["t_ms"]
            end = closing if closing is not None else acks[j - 1]["t_ms"]
            streaks.append({
                "start_ms": start, "end_ms": end, "closed": closing is not None,
                "duration_ms": end - start, "frames": j - k, "S": len(s_frames),
                "N": (j - k) - len(s_frames),
                "last_S_ms": acks[s_frames[-1]]["t_ms"] if s_frames else None,
                "prev_P_ms": acks[k - 1]["t_ms"] if k > 0 else None,
                "last_ack_now": sum(1 for m in frames if acks[m].get("last_ack_now")),
                "motor_progress": sum(1 for m in frames if motor_progress(acks[m]) == "ilerleme"),
                "recover_fwd_ms": time_in_state(states, start, end, "RecoverFwd"),
            })
        k = j
    entries = []
    for s in states:
        if s.get("to") != "RecoverFwd" or s.get("from") == "RecoverFwd":
            continue
        t, where = s["t_ms"], WHERE[3]
        for st in streaks:
            lo, hi = streak_window(st)
            if not lo <= t <= hi:
                continue
            if t < st["start_ms"]:
                where = WHERE[0]
            elif st["last_S_ms"] is not None and t <= st["last_S_ms"]:
                where = WHERE[1]
            else:
                where = WHERE[2]
            break
        level = s.get("emergency")
        path = "+".join(name for name, on in (("seviye", level not in (None, "Arq")),
                                              ("füzyon", bool(s.get("fusion_emergency_mode")))) if on)
        entries.append({"t_ms": t, "from": s.get("from"), "where": where, "path": path or DASH,
                        "silence_ms": (t - s["last_ack_ms"]) if s.get("last_ack_ms") is not None else None,
                        "ack_confidence_permille": s.get("ack_confidence_permille"), "emergency": level,
                        "fusion_emergency_mode": s.get("fusion_emergency_mode")})
    series = timeline_motor(rec, log.get("side"), session=log.get("session"))
    for st in streaks:
        lo, hi = streak_window(st)
        st["recover_fwd_entries"] = sum(1 for e in entries if lo <= e["t_ms"] <= hi)
        inside = [m for t, _, m, _ in series if st["start_ms"] <= t <= st["end_ms"]]
        confs = [m.get("ack_confidence_permille") for m in inside if m.get("ack_confidence_permille") is not None]
        levels = [m.get("emergency") for m in inside if m.get("emergency") in LEVELS]
        st["min_ack_confidence_permille"] = min(confs) if confs else None
        st["max_emergency"] = max(levels, key=LEVELS.index) if levels else None
    s_frames = [acks[m] for m, c in enumerate(classes) if c == "S"]
    logged = log.get("acks_logged") if log.get("acks_logged") is not None else len(acks)
    return {
        "side": log.get("side"), "peer": log.get("peer"), "session": log.get("session"),
        "logged": logged, "missed": log.get("acks_missed"), "dropped": log.get("acks_dropped"),
        "unread": (log.get("acks_ring_total") or 0) - logged - (log.get("acks_missed") or 0)
        - (log.get("acks_dropped") or 0),
        "states_missed": log.get("states_missed"),
        "P": classes.count("P"), "S": classes.count("S"), "N": classes.count("N"),
        "S_mapped": sum(1 for a in s_frames if a.get("mapped")),
        "S_last_ack_now": sum(1 for a in s_frames if a.get("last_ack_now")),
        "S_motor_progress": sum(1 for a in s_frames if motor_progress(a) == "ilerleme"),
        "S_inflight_shrunk": sum(1 for a in s_frames if (a.get("after") or {}).get("inflight", 0)
                                 < (a.get("before") or {}).get("inflight", 0)),
        "arrivals": arrivals, "emissions": emissions, "streaks": streaks, "recover_fwd": entries,
    }


def class_hist(items):
    hist = {"taban×1": 0, "taban×2": 0, "taban×≥3": 0, "kota": 0, "diğer": 0}
    for it in items:
        c = it["class"]
        if c.startswith("taban×"):
            n = int(c.split("×")[1])
            hist["taban×1" if n == 1 else "taban×2" if n == 2 else "taban×≥3"] += 1
        elif c.startswith("kota×"):
            hist["kota"] += 1
        else:
            hist["diğer"] += 1
    return hist


def m2_run(rec, side, peer, crit):
    """Kesinti öncesi / sırası / sonrası pacer ve BtlBw (beklenti kaydı §3)."""
    o = rec.get("outage") or {}
    applied, restored = o.get("applied_ms"), o.get("restored_ms")
    rows = timeline_motor(rec, side, peer=peer)
    out = {"side": side, "peer": peer, "samples": len(rows),
           "sessions": sorted({r[1] for r in rows if r[1]})}
    if applied is None or restored is None or not rows:
        out["error"] = "kesinti anları ya da zaman çizelgesi motor örneği yok"
        return out
    start = rec.get("connect_requested_ms")
    phases = {
        "önce": [r for r in rows if (start is None or r[0] >= start) and r[0] < applied
                 and r[2].get("state") != "PreConverge"],
        "sırası": [r for r in rows if applied <= r[0] <= restored],
        "sonrası": [r for r in rows if r[0] > restored],
    }

    def rate(rs, key):
        if len(rs) < 2 or rs[-1][0] <= rs[0][0]:
            return None
        a, b = rs[0][2].get(key), rs[-1][2].get(key)
        return None if a is None or b is None else (b - a) / ((rs[-1][0] - rs[0][0]) / 1000.0)

    def stats(rs):
        pacer = [r[2].get("pacer_rate_bps") for r in rs if r[2].get("pacer_rate_bps") is not None]
        bw = [r[2].get("btlbw_bps") for r in rs if r[2].get("btlbw_bps") is not None]
        states: dict = {}
        for r in rs:
            states[r[2].get("state")] = states.get(r[2].get("state"), 0) + 1
        return {"n": len(rs),
                "pacer": {"median": nearest_median(pacer), "min": min(pacer, default=None),
                          "max": max(pacer, default=None)},
                "btlbw": {"median": nearest_median(bw), "min": min(bw, default=None),
                          "max": max(bw, default=None)},
                "floor_fraction": (sum(1 for p in pacer if p <= crit["MIN_RATE_BPS"]) / len(pacer))
                if pacer else None,
                "datagrams_per_s": rate(rs, "tx_datagrams"), "bytes_per_s": rate(rs, "tx_bytes"),
                "states": dict(sorted(states.items(), key=lambda kv: -kv[1]))}

    out["phases"] = {name: stats(rs) for name, rs in phases.items()}
    post = phases["sonrası"]
    for key in ("pacer_rate_bps", "btlbw_bps"):
        pre_med = nearest_median([r[2].get(key) for r in phases["önce"]])
        res = {"pre_median": pre_med, "threshold": None, "first_s": None, "sustained_s": None}
        if pre_med is None:
            res["error"] = "ölçülemedi — kesinti öncesi PreConverge dışı örnek yok"
        else:
            thr = crit["RECOVERY_FRACTION"] * pre_med
            res["threshold"] = thr
            ok = [(r[2].get(key) or 0) >= thr for r in post]
            first = next((i for i, v in enumerate(ok) if v), None)
            sustained = next((i for i in range(len(ok)) if all(ok[i:])), None)
            res["first_s"] = None if first is None else (post[first][0] - restored) / 1000.0
            res["sustained_s"] = None if sustained is None else (post[sustained][0] - restored) / 1000.0
        out["recovery_" + key] = res
    window_ms = crit["OLD_POST_WINDOW_S"] * 1000.0
    base = [r for r in rows if r[0] <= restored]
    within = [r for r in post if r[0] <= restored + window_ms]
    out["old_window"] = None
    if base and within:
        b, e = base[-1], within[-1]
        dt = (e[0] - b[0]) / 1000.0
        dgr = (e[2].get("tx_datagrams") or 0) - (b[2].get("tx_datagrams") or 0)
        byt = (e[2].get("tx_bytes") or 0) - (b[2].get("tx_bytes") or 0)
        out["old_window"] = {"span_s": dt, "datagrams": dgr, "bytes": byt,
                             "bytes_per_s": byt / dt if dt > 0 else None}
    return out


def m12_render(runs, seeds, crit, crit_sha):
    """M1 / M2 tabloları (Markdown satırları) ve özet; `m_log` taşıyan koşu yoksa boş."""
    md: list[str] = []
    summary: dict = {"criteria_sha256": crit_sha, "m1": [], "m2": []}
    cap = crit["MAX_ROWS"]
    m1_rows, streak_rows, entry_rows, s9_bob = [], [], [], None
    for arm in ALL_ARMS:
        for s in seeds:
            rec = runs.get((s, arm))
            if rec is None:
                continue
            for log in m_logs(rec):
                m = m1_session(rec, log, crit)
                m["seed"], m["arm"] = s, arm
                summary["m1"].append(m)
                if arm == "gating2" and s == 9 and m["side"] == "bob":
                    s9_bob = m
                end = f"{m['side']}→{m['peer']}"
                hist = class_hist(m["arrivals"])
                where = [sum(1 for e in m["recover_fwd"] if e["where"] == w) for w in WHERE]
                longest = max((st["duration_ms"] for st in m["streaks"]), default=None)
                m1_rows.append([
                    s, arm, end,
                    f"{fmt(m['logged'])} / {fmt(m['missed'])} / {fmt(m['dropped'])} / {fmt(m['unread'])}",
                    f"{m['P']} / {m['S']} / {m['N']}",
                    f"{m['S_mapped']} / {m['S_last_ack_now']} / {m['S_motor_progress']} / {m['S_inflight_shrunk']}",
                    f"{len(m['arrivals'])} · {fmt_s(nearest_median([a['delta_ms'] for a in m['arrivals']]))}",
                    " / ".join(str(v) for v in hist.values()),
                    f"{len(m['emissions'])} · " + (", ".join(f"{e['delta_ms'] / 1000.0:.3f} ({e['class']})"
                                                           for e in m["emissions"][:6]) or DASH),
                    f"{len(m['streaks'])} · {fmt_s(longest)}",
                    " / ".join(str(v) for v in where)])
                for st in m["streaks"]:
                    share = 100.0 * st["recover_fwd_ms"] / st["duration_ms"] if st["duration_ms"] > 0 else None
                    streak_rows.append((st["duration_ms"], [
                        s, arm, end, fmt_s(st["start_ms"]), fmt_s(st["duration_ms"]),
                        "evet" if st["closed"] else "açık", f"{st['S']} / {st['N']}",
                        fmt_s(None if st["last_S_ms"] is None else st["last_S_ms"] - st["start_ms"]),
                        fmt_s(None if st["prev_P_ms"] is None else st["start_ms"] - st["prev_P_ms"]),
                        f"{st['last_ack_now']}/{st['frames']}", str(st["motor_progress"]), fmt(share, 0),
                        str(st["recover_fwd_entries"]), fmt(st["min_ack_confidence_permille"]),
                        fmt(st["max_emergency"])]))
                for e in m["recover_fwd"]:
                    entry_rows.append([s, arm, end, fmt_s(e["t_ms"]), fmt(e["from"]), fmt_s(e["silence_ms"]),
                                       fmt(e["ack_confidence_permille"]), fmt(e["emergency"]),
                                       fmt(e["fusion_emergency_mode"]), e["path"], e["where"]])
    if not m1_rows:
        return md, summary
    md += ["## M1 / M2 teşhis ölçümü (CEVAP PAKETİ 6 — hüküm değil)",
           f"Ölçüt ve beklenti kaydı `docs/reports/INT-02-M1M2-BEKLENTI.md` · sha256 `{crit_sha}` · "
           f"taban {crit['ACK_CADENCE_FLOOR_MS']:.3f} ms · kota aralığı {crit['T_SPACING_MS']:.3f} ms · "
           f"varış toleransı {crit['ARRIVAL_TOL_MS']:.0f} ms · üretim toleransı {crit['EMISSION_TOL_MS']:.0f} ms.",
           "",
           "### M1 — ACK çerçeveleri (gönderen ucu)",
           "P = ilerlemeli (yeni birim teyidi) · S = bayat (alıcı iki ACK arasında yeni datagram görmedi) · "
           "N = ilerlemesiz, yeni datagramlı. Kaçan = iki sonda arasında halkadan düşen; okunmayan = son "
           "sondadan sonra gelen.",
           table(["tohum", "kol", "uç", "kayıt / kaçan / tavan / okunmayan", "P / S / N",
                  "S: motora giden / son-ACK tazeleyen / motor ilerleme / uçuş küçülen",
                  "S varış aralığı n · medyan", "taban×1 / ×2 / ×≥3 / kota / diğer",
                  "alıcı üretim aralığı n · ilk değerler (s)", "dizi · en uzun",
                  "RecoverFwd girişi: öncesi / sürerken / sonra / dışı"], m1_rows), ""]
    streak_rows.sort(key=lambda x: -x[0])
    md += [f"### M1 — ilerlemesiz diziler (en uzun {cap}; tam liste JSON özetinde)",
           table(["tohum", "kol", "uç", "baş", "süre", "kapandı", "S / N", "son S (baştan)",
                  "önceki P'den", "son-ACK tazeleyen / çerçeve", "motor ilerleme", "RecoverFwd payı %",
                  "RecoverFwd girişi", "en düşük ack_conf ‰ (çizelge)", "en yüksek acil seviye (çizelge)"],
                 [r for _, r in streak_rows[:cap]]), ""]
    if entry_rows:
        md += [f"### M1 — RecoverFwd girişleri (ilk {cap}; tam liste JSON özetinde)",
               table(["tohum", "kol", "uç", "an", "önceki durum", "son ACK'ten", "ack_conf ‰", "seviye",
                      "füzyon kipi", "yol", "konum"], entry_rows[:cap]), ""]
    old = crit["OLD_S9_BOB_ACK_S"]
    old_items = [{"delta_ms": (b - a) * 1000.0,
                  "class": interval_class((b - a) * 1000.0, crit, crit["ARRIVAL_TOL_MS"])}
                 for a, b in zip(old, old[1:])]
    lines = [["eski kayıt (gating2 s9 bob)", ", ".join(f"{i['delta_ms'] / 1000.0:.1f}" for i in old_items),
              ", ".join(i["class"] for i in old_items), DASH]]
    if s9_bob is not None:
        rf = sum(st["recover_fwd_ms"] for st in s9_bob["streaks"])
        dur = sum(st["duration_ms"] for st in s9_bob["streaks"])
        lines.append([f"bu koşu (gating2 s9 bob, ilk {cap} S aralığı)",
                      ", ".join(f"{a['delta_ms'] / 1000.0:.1f}" for a in s9_bob["arrivals"][:cap]) or DASH,
                      ", ".join(a["class"] for a in s9_bob["arrivals"][:cap]) or DASH,
                      fmt(100.0 * rf / dur if dur > 0 else None, 0)])
    summary["old_record"] = {"intervals": old_items}
    md += ["### M1 — eski kayıt doğrulaması (gating2 s9, bob)",
           table(["kaynak", "S varış aralıkları (s)", "sınıf", "dizilerde RecoverFwd payı %"], lines), ""]
    m2_rows, rec_rows = [], []
    for s in seeds:
        rec = runs.get((s, "ek-kesinti"))
        if rec is None:
            continue
        for side, peer in (("alice", "bob"), ("bob", "alice")):
            m = m2_run(rec, side, peer, crit)
            m["seed"] = s
            summary["m2"].append(m)
            if m.get("error"):
                m2_rows.append([s, f"{side}→{peer}", m["error"]] + [DASH] * 7)
                continue
            for name, ph in m["phases"].items():
                p, b = ph["pacer"], ph["btlbw"]
                m2_rows.append([s, f"{side}→{peer}", name, ph["n"],
                                f"{fmt(p['median'])} / {fmt(p['min'])} / {fmt(p['max'])}",
                                f"{fmt(b['median'])} / {fmt(b['min'])} / {fmt(b['max'])}",
                                fmt(None if ph["floor_fraction"] is None else 100.0 * ph["floor_fraction"], 0),
                                fmt(ph["datagrams_per_s"], 2), fmt(ph["bytes_per_s"], 1),
                                ", ".join(f"{k}:{v}" for k, v in list(ph["states"].items())[:3])])
            rp, rb, ow = m["recovery_pacer_rate_bps"], m["recovery_btlbw_bps"], m["old_window"] or {}
            rec_rows.append([s, f"{side}→{peer}", fmt(rp.get("pre_median")), fmt(rp.get("threshold"), 0),
                             fmt(rp.get("first_s")), fmt(rp.get("sustained_s")),
                             fmt(rb.get("first_s")), fmt(rb.get("sustained_s")),
                             f"{fmt(ow.get('datagrams'))} · {fmt(ow.get('bytes'))} B · "
                             f"{fmt(ow.get('bytes_per_s'), 1)} B/s ({fmt(ow.get('span_s'), 0)} s)",
                             rp.get("error") or rb.get("error") or DASH])
    if m2_rows:
        md += ["### M2 — kesinti öncesi / sırası / sonrası pacer ve BtlBw (ek-kesinti)",
               f"Taban = pacer ≤ {crit['MIN_RATE_BPS']} B/s. Medyan en yakın sıra; datagram/s ve bayt/s "
               "evrenin ilk ve son örneği arasında (tel datagramı, bütün sınıflar).",
               table(["tohum", "uç", "evre", "örnek", "pacer medyan / min / max (B/s)",
                      "BtlBw medyan / min / max (B/s)", "tabanda %", "datagram/s", "bayt/s",
                      "durum (ilk 3)"], m2_rows), "",
               f"### M2 — toparlanma (eşik = {crit['RECOVERY_FRACTION']} × kesinti öncesi medyan; süre "
               "kesinti bitiminden, s)",
               table(["tohum", "uç", "pacer öncesi medyan (B/s)", "eşik (B/s)", "pacer ilk dönüş",
                      "pacer kalıcı dönüş", "BtlBw ilk dönüş", "BtlBw kalıcı dönüş",
                      f"kesinti sonrası ilk {crit['OLD_POST_WINDOW_S']:.0f} s: datagram · bayt · B/s", "not"],
                     rec_rows), ""]
    return md, summary


def render(runs, seeds, crit=None, crit_sha=None) -> tuple[str, dict]:
    def rec(seed, arm):
        return runs.get((seed, arm))

    md: list[str] = []
    summary: dict = {"seeds": seeds, "arms": {}}
    for arm in ALL_ARMS:
        verdicts = {s: verdict(rec(s, arm), arm) for s in seeds if rec(s, arm) is not None}
        if verdicts:
            summary["arms"][arm] = {str(s): {"verdict": v, "reasons": r} for s, (v, r) in verdicts.items()}

    # GATING (1)
    rows = []
    for s in seeds:
        r = rec(s, "gating1")
        if r is None:
            continue
        f = flow(r, "alice_to_bob")
        rows.append([s, r.get("result"), fmt(f.get("delivered_pct")), fmt(f.get("receipt_pct")),
                     lat(f), lat(f, "receipt_rtt_ms"), arq_cell(r), arq_cell(r, "tail_probes"),
                     zero_cell(r, "app_retransmit"), zero_cell(r, "crypto_retransmit"),
                     zero_cell(r, "ctx_mismatch_drops"), zero_cell(r, "hub_frames_dropped"),
                     zero_cell(r, "delivery_channel_drops"), zero_cell(r, "not_ready_drops"),
                     epoch_cell(r), netem_cell(r), verdict(r, "gating1")[0]])
    if rows:
        md += ["### GATING (1) — alice → bob 100 TEXT + 100 RECEIPT",
               table(["tohum", "sonuç", "teslim %", "alındı %", "gecikme medyan / 95p (ms)",
                      "alındı RTT medyan / 95p (ms)", "ARQ retx A / B", "tail-probe A / B",
                      "app retx A / B", "crypto retx A / B", "ctx A / B", "hub A / B", "dcd A / B",
                      "not_ready A / B", "epoch-1 (Connected'dan, ms) A / B", "netem doğrulama",
                      "HÜKÜM"], rows), ""]

    # GATING (2)
    rows, karn = [], []
    for s in seeds:
        r = rec(s, "gating2")
        if r is None:
            continue
        f = flow(r, "bob_to_alice")
        e1, fs = get(node(r, "bob"), "epoch", "first_epoch1_ms"), f.get("first_send_ms")
        gap = (e1 - fs) if (e1 is not None and fs is not None) else None
        ev = [get(probe(r, n), "session_events", "ready") for n in ("alice", "bob")]
        rows.append([s, r.get("result"), window_cell(r), fmt(f.get("delivered_pct")), fmt(f.get("receipt_pct")), lat(f),
                     fmt(f.get("first_send_epoch")), fmt(gap, 0), fmt(f.get("sends_at_epoch0")),
                     pair(get(node(r, "alice"), "epoch", "last"), get(node(r, "bob"), "epoch", "last"), 0),
                     pair(ev[0], ev[1], 0),
                     pair(probe(r, "alice").get("sessions_live"), probe(r, "bob").get("sessions_live"), 0),
                     arq_cell(r), zero_cell(r, "app_retransmit"), zero_cell(r, "crypto_retransmit"),
                     zero_cell(r, "ctx_mismatch_drops"), netem_cell(r), verdict(r, "gating2")[0]])
        karn.append([s, zero_cell(r, "not_ready_drops"),
                     pair(session_diag(r, "alice", "rtt_samples"), session_diag(r, "bob", "rtt_samples"), 0),
                     pair(session_diag(r, "alice", "left_preconverge_ms"),
                          session_diag(r, "bob", "left_preconverge_ms"), 0),
                     pair(session_diag(r, "alice", "srtt_ms"), session_diag(r, "bob", "srtt_ms"), 0),
                     pair(session_diag(r, "alice", "min_rtt_ms"), session_diag(r, "bob", "min_rtt_ms"), 0),
                     fmt(get(r, "netem_check", "ping", "rtt_avg_ms"), 0)])
    if rows:
        g2 = [verdict(rec(s, "gating2"), "gating2")[0] for s in seeds if rec(s, "gating2") is not None]
        windows = sorted({str(get(rec(s, "gating2"), "params", "window_s")) for s in seeds
                          if rec(s, "gating2") is not None})
        summary["gating2"] = {"passed": sum(v == "GEÇTİ" for v in g2), "run": len(g2), "windows_s": windows}
        md += ["### GATING (2) — bob (yanıtlayan) → alice; epoch 0→1 akış içinde, tek oturum",
               f"**GATING (2): {summary['gating2']['passed']}/{len(g2)} tohum GEÇTİ** · pencere "
               f"{', '.join(windows)} s (ölçüm tablosu; kapı ifadesi değildir).",
               table(["tohum", "sonuç", "pencere (s) · sonu", "teslim %", "alındı %", "gecikme medyan / 95p (ms)",
                      "bob ilk gönderim epoch'u", "bob epoch-1 − ilk gönderim (ms)",
                      "epoch-0'da gönderim", "son epoch A / B", "Ready A / B", "canlı oturum A / B",
                      "ARQ retx A / B", "app retx A / B", "crypto retx A / B", "ctx A / B",
                      "netem doğrulama", "HÜKÜM"], rows), "",
               "#### DUR-K4G-01 (iii) — NotReady düşüşleri ve Karn / PreConverge (GATING (2))",
               table(["tohum", "not_ready_drops A / B", "RTT örneği A / B",
                      "PreConverge çıkışı (uç açılışından, ms) A / B", "sRTT (ms) A / B",
                      "min RTT (ms) A / B", "ping ort. RTT (ms)"], karn), ""]

    # GATING (6)
    rows = []
    for s in seeds:
        r = rec(s, "gating6")
        if r is None:
            continue
        ab, ba = flow(r, "alice_to_bob"), flow(r, "bob_to_alice")
        conn = [get(node(r, n), "events", "connected") for n in ("alice", "bob")]
        closed = [(get(node(r, n), "events", "closed_session") or 0) + (get(node(r, n), "events", "connect_abandoned") or 0)
                  for n in ("alice", "bob")]
        crypto = [get(probe(r, n), "crypto", default={}) or {} for n in ("alice", "bob")]
        rows.append([s, r.get("result"), pair(conn[0], conn[1], 0), pair(closed[0], closed[1], 0),
                     f"{fmt(ab.get('delivered'))}/{fmt(ab.get('sent'))} · {fmt(ab.get('receipts'))}",
                     f"{fmt(ba.get('delivered'))}/{fmt(ba.get('sent'))} · {fmt(ba.get('receipts'))}",
                     pair(ab.get("duplicate_deliveries"), ba.get("duplicate_deliveries"), 0),
                     pair(crypto[0].get("inits_sent"), crypto[1].get("inits_sent"), 0),
                     pair(crypto[0].get("responses_sent"), crypto[1].get("responses_sent"), 0),
                     pair(get(probe(r, "alice"), "session_events", "ready"),
                          get(probe(r, "bob"), "session_events", "ready"), 0),
                     zero_cell(r, "app_retransmit"), zero_cell(r, "crypto_retransmit"),
                     zero_cell(r, "ctx_mismatch_drops"), netem_cell(r), verdict(r, "gating6")[0]])
    if rows:
        md += ["### GATING (6) — eşzamanlı connect (glare), iki yönde TEXT + RECEIPT",
               table(["tohum", "sonuç", "Connected A / B", "Closed+Abandoned A / B",
                      "A→B teslim/gönderilen · alındı", "B→A teslim/gönderilen · alındı",
                      "tekrar A→B / B→A", "INIT A / B", "RESPONSE A / B", "Ready A / B",
                      "app retx A / B", "crypto retx A / B", "ctx A / B", "netem doğrulama",
                      "HÜKÜM"], rows), ""]

    # T4
    t4_rows, t4_pass = [], True
    for arm in GATING_ARMS:
        vs = [verdict(rec(s, arm), arm)[0] for s in seeds if rec(s, arm) is not None]
        passed = sum(v == "GEÇTİ" for v in vs)
        ok = len(vs) >= T4_MIN_SEEDS and passed == len(vs)
        t4_pass &= ok
        t4_rows.append([arm, f"{passed}/{len(vs)}", "GEÇTİ" if ok else "KALDI"])
    summary["t4"] = {"pass": t4_pass, "rows": t4_rows}
    md += [f"### T4 — sabit tohum, en az {T4_MIN_SEEDS} tohum, hepsi GEÇTİ",
           table(["kol", "geçen / koşulan", "T4"], t4_rows), "",
           f"**T4 (ölçüm tablosu): {'GEÇTİ' if t4_pass else 'KALDI'}** — kapı ifadesi değildir.", ""]

    # NEGATİF
    rows = []
    for s in seeds:
        r = rec(s, "negatif")
        if r is None:
            continue
        f = flow(r, "alice_to_bob")
        rows.append([s, r.get("result"), f"{fmt(f.get('delivered'))}/{fmt(f.get('sent'))}",
                     fmt(f.get("receipt_pct")), arq_cell(r, "recovery_suppressed"),
                     arq_cell(r, "retransmits"), arq_cell(r, "tx_retx_bytes"),
                     netem_cell(r), verdict(r, "negatif")[0]])
    if rows:
        md += ["### Negatif kol — recovery OFF (ölçümün kaybı görebildiğinin kanıtı)",
               table(["tohum", "sonuç", "teslim / gönderilen", "alındı %",
                      "bastırılan kurtarma kararı A / B", "motor retx kararı A / B",
                      "tele çıkan retx baytı A / B", "netem doğrulama", "HÜKÜM"], rows), ""]

    # NON-GATING: dosya + PTT
    rows = []
    for s in seeds:
        r = rec(s, "ek-dosya-ptt")
        if r is None:
            continue
        f, fl, pt = flow(r, "alice_to_bob"), r.get("file") or {}, r.get("ptt") or {}
        oc = fl.get("outcome") or {}
        rows.append([s, r.get("result"), window_cell(r), f"{fmt(f.get('delivered'))}/{fmt(f.get('sent'))}", lat(f),
                     fmt(fl.get("bytes")), fmt(oc.get("status")), fmt(oc.get("reason")),
                     fmt(file_since_sent(r, "at_ms"), 0),
                     fmt(fl.get("send_to_received_ms"), 0),
                     fmt(fl.get("send_to_receipt_ms"), 0), fmt(file_since_sent(r, "sender_failed_ms"), 0),
                     fmt(oc.get("receiver_failed_reason")), fmt(fl.get("bytes_match")),
                     f"{fmt(pt.get('received'))}/{fmt(pt.get('pushed'))}", fmt(pt.get("in_order_complete")),
                     fmt(pt.get("press_to_first_audio_ms"), 0), netem_cell(r)])
        summary.setdefault("ek_dosya_ptt", {})[str(s)] = {
            "status": oc.get("status"), "reason": oc.get("reason"),
            "texts_delivered": f.get("delivered"), "texts_sent": f.get("sent"),
            "ptt_in_order_complete": pt.get("in_order_complete"), "window_ended_by": get(r, "window", "ended_by")}
    if rows:
        md += ["### NON-GATING — 100 TEXT + 1 dosya + 1 PTT klibi (aynı profil)",
               "Dosya sonucu ayrı alandadır (`file.outcome`); kol hükmü değişmedi: başarısız dosya kolu "
               "tamamlanmamış sayar. Metin ve PTT tam pencere boyunca ölçülür.",
               table(["tohum", "sonuç", "pencere (s) · sonu", "TEXT teslim", "gecikme medyan / 95p (ms)",
                      "dosya bayt", "dosya durumu", "neden", "gönderim→sonuç (ms)",
                      "gönderim→alındı (ms)", "gönderim→FILE_RECEIPT (ms)",
                      "gönderim→gönderen başarısızlığı (ms)", "alıcı başarısızlığı", "bayt-özdeş",
                      "PTT demet alınan/verilen", "sıralı+tam", "bas→ilk ses (ms)",
                      "netem doğrulama"], rows), ""]

    # NON-GATING: 60 s kesinti
    rows, ptt_rows = [], []
    for s in seeds:
        r = rec(s, "ek-kesinti")
        if r is None:
            continue
        f, o, pt = flow(r, "alice_to_bob"), r.get("outage") or {}, r.get("ptt") or {}
        after = o.get("delivery_after_restore_ms") or {}
        rows.append([s, r.get("result"), window_cell(r), f"{fmt(f.get('delivered'))}/{fmt(f.get('sent'))}",
                     fmt(o.get("texts_complete")), fmt(o.get("duration_ms"), 0),
                     f"{fmt(o.get('texts_delivered_of_during'))}/{fmt(o.get('texts_sent_during'))}",
                     f"{fmt(after.get('median'), 0)} / {fmt(after.get('p95'), 0)} / {fmt(after.get('max'), 0)}",
                     fmt(o.get("same_session")), fmt(o.get("same_session_outage_only")),
                     fmt(o.get("same_session_handles")), transition(r, "epoch"),
                     transition(r, "connected_events"), transition(r, "closed_events"),
                     arq_cell(r), netem_cell(r), outage_criterion(r)])
        ptt_rows.append([s, f"{fmt(pt.get('received'))}/{fmt(pt.get('pushed'))}",
                         fmt(pt.get("in_order_complete")), fmt(pt.get("peer_opened_events")),
                         fmt(pt.get("peer_closed_events")), fmt(pt.get("press_to_first_audio_ms"), 0),
                         pair(get(probe(r, "alice"), "session_events", "ready"),
                              get(probe(r, "bob"), "session_events", "ready"), 0)])
        summary.setdefault("ek_kesinti", {})[str(s)] = {
            "texts_complete": o.get("texts_complete"), "same_session": o.get("same_session"),
            "same_session_outage_only": o.get("same_session_outage_only"),
            "same_session_handles": o.get("same_session_handles"),
            "ptt_in_order_complete": pt.get("in_order_complete"), "criterion": outage_criterion(r)}
    if rows:
        met = sum(v["criterion"] == "KARŞILANDI" for v in summary["ek_kesinti"].values())
        md += ["### NON-GATING — akış ortasında 60 s tam kesinti (`loss 100%`)",
               f"**Sahip ölçütü (CEVAP PAKETİ 4 — 60 s kesinti, aynı oturumda %100 metin teslimi): "
               f"{met}/{len(rows)} tohum** (metin %100 ∧ same_session; PTT ayrı tabloda, ölçüte girmez; "
               "ölçüm özeti, kapı ifadesi değildir).",
               "#### Metin ve aynı oturum",
               "`same_session` = kesinti başından koşu sonuna iki düğümde yeni `Closed` yok, ikinci "
               "`Connected` yok, epoch değişmemiş; 'yalnız kesinti' = aynı ölçüt kesinti bitişine dek; "
               "'tutamaç + S1' = oturum tutamaçları ve S1 Ready/Closed sayıları da aynı.",
               table(["tohum", "sonuç", "pencere (s) · sonu", "metin teslim/gönderilen", "metin %100",
                      "kesinti (ms)", "kesintide gönderilenden teslim",
                      "kesinti bitişinden teslim medyan / 95p / max (ms)", "same_session",
                      "yalnız kesinti", "tutamaç + S1", "epoch başı→sonu A / B",
                      "Connected başı→sonu A / B", "Closed başı→sonu A / B", "ARQ retx A / B",
                      "netem doğrulama", "sahip ölçütü"], rows), "",
               "#### PTT (ayrı)",
               table(["tohum", "demet alınan/verilen", "sıralı+tam", "PttOpened olayı",
                      "PttClosed olayı", "bas→ilk ses (ms)", "Ready A / B"], ptt_rows), ""]

    # TAKILI BİRİM ÖZETİ (teşhis; hüküm değil) — koşu başına tek satır
    rows = []
    for arm in ALL_ARMS:
        for s in seeds:
            r = rec(s, arm)
            if r is None:
                continue
            samples = get(r, "timeline", "samples", default=[]) or []
            rows.append([s, arm, r.get("result"),
                         pair(probe(r, "alice").get("outstanding_units"), probe(r, "bob").get("outstanding_units"), 0),
                         stuck_cell(r, "alice"), stuck_cell(r, "bob"),
                         reorder_cell(r, "alice"), reorder_cell(r, "bob"),
                         f"{len(samples)} (düşen {fmt(get(r, 'timeline', 'dropped'))})"])
            summary.setdefault("stuck", []).append({
                "seed": s, "arm": arm, "result": r.get("result"),
                "alice": first_session(r, "alice").get("recovery"),
                "bob": first_session(r, "bob").get("recovery"),
                "reorder": {"alice": first_session(r, "alice").get("reorder"),
                            "bob": first_session(r, "bob").get("reorder")}})
    if rows:
        md += ["### Takılı birim özeti (K4G-05 teşhis; hüküm değil — son sonda)",
               "A = alice, B = bob. Gönderen hücresi o düğümün GÖNDERDİĞİ yöndür; `reorder` o düğümün "
               "ALDIĞI yönde sıralı teslimin beklediği birim ve arkasında tutulanlardır.",
               table(["tohum", "kol", "sonuç", "teyitsiz birim A / B", "gönderen A", "gönderen B",
                      "alıcı reorder A", "alıcı reorder B", "zaman çizelgesi örneği"], rows), ""]

    # [CEVAP PAKETİ 6] M1 / M2 teşhis ölçümü (ölçüt dosyası verildiyse ve koşuda `m_log` varsa)
    if crit is not None:
        m12_md, summary["m12"] = m12_render(runs, seeds, crit, crit_sha)
        md += m12_md

    # gerekçeler
    reasons = []
    for arm, per_seed in summary["arms"].items():
        for s, v in per_seed.items():
            if v["reasons"]:
                reasons.append(f"- `{arm}` tohum {s}: {v['verdict']} — " + "; ".join(v["reasons"]))
    if reasons:
        md += ["### Hüküm gerekçeleri (GEÇTİ olmayan koşular)", *reasons, ""]
    return "\n".join(md), summary


# ── öz-test ──────────────────────────────────────────────────────────────────────────────────

def _synthetic(arm: str, seed: int, *, lossy_negative=True, broken=False) -> dict:
    count = 100
    ping = {"sent": 20, "received": 9, "rtt_avg_ms": 401.0}
    probe_ = {"app_retransmit": 0, "crypto_retransmit": 0, "ctx_mismatch_drops": 0,
              "hub_frames_dropped": 0, "delivery_channel_drops": 0, "not_ready_drops": 0,
              "sessions_live": 1, "session_events": {"ready": 1, "closed": 0, "renew_required": 0},
              "arq_total": {"retransmits": 40, "tail_probes": 3, "recovery_suppressed": 0,
                            "tx_retx_bytes": 4000},
              "crypto": {"inits_sent": 1, "responses_sent": 0},
              "outstanding_units": 0,
              "sessions": [{"diag": {"rtt_samples": 90, "srtt_ms": 410.0, "min_rtt_ms": 20.0,
                                     "left_preconverge_ms": 800.0},
                            "recovery": {"unacked_units": 1, "since_last_ack_ms": 400.0,
                                         "timers": {"state": "RecoverFwd", "arq_retx_cap": 3,
                                                    "tail_probe_index": 3, "tail_max_probes": 3,
                                                    "tail_pto_ms": 30000.0, "arq_aggressiveness_permille": 500},
                                         "oldest_unacked": {"unit_id": 17, "emission_decisions": 7,
                                                            "max_retx_count": 3, "inflight_records": 0,
                                                            "tombstone_records": 2, "pending_lost_retx": None,
                                                            "since_latest_send_ms": 12000.0,
                                                            "dup_due_in_ms": 41000.0, "dup_interval_ms": 51200.0}},
                            "reorder": {"next_unit": 17, "held_units": 31, "first_held_unit": 18}}]}
    full = {"sent": count, "delivered": count, "receipts": count, "duplicate_deliveries": 0,
            "mismatched_texts": 0, "unknown_deliveries": 0, "delivered_pct": 100.0, "receipt_pct": 100.0,
            "first_send_epoch": 0, "first_send_ms": 50.0,
            "latency_ms": {"median": 420.0, "p95": 1900.0}, "receipt_rtt_ms": {"median": 900.0, "p95": 3100.0}}
    empty = {"sent": 0, "delivered": 0, "receipts": 0, "duplicate_deliveries": 0, "mismatched_texts": 0,
             "unknown_deliveries": 0}
    rec = {"arm": arm, "seed": seed, "result": "complete", "params": {"count": count}, "listener_drops": 0,
           "netem_check": {"ping": ping, "run_delta": {"dropped": 900, "loss_pct": 30.1}},
           "flows": {"alice_to_bob": dict(full), "bob_to_alice": dict(empty)},
           "nodes": {n: {"events": {"connected": 1, "closed_session": 0, "connect_abandoned": 0},
                         "epoch": {"last": 1, "first_epoch1_ms": 300.0, "epoch1_after_connected_ms": 250.0},
                         "probe": json.loads(json.dumps(probe_))} for n in ("alice", "bob")}}
    if arm in ("gating2", "gating6"):
        rec["flows"]["bob_to_alice"] = dict(full)
    if arm == "gating2":
        rec["flows"]["alice_to_bob"] = dict(empty)
    if arm == "negatif":
        rec["result"] = "incomplete"
        rec["flows"]["alice_to_bob"].update(delivered=3 if lossy_negative else count, receipts=2)
        for n in ("alice", "bob"):
            rec["nodes"][n]["probe"]["arq_total"].update(recovery_suppressed=50, tx_retx_bytes=0)
    if broken:
        rec["nodes"]["bob"]["probe"]["crypto_retransmit"] = 1
    # [CEVAP PAKETİ 4] v2 alanları.
    rec["params"]["window_s"] = 600
    rec["window"] = {"s": 600, "anchor": "outage_end" if arm == "ek-kesinti" else "last_send",
                     "ended_by": "complete" if rec["result"] == "complete" else "deadline"}
    if arm == "ek-kesinti":
        mark = {"epoch": 1, "connected_events": 1, "closed_events": 0, "s1_ready": 1, "s1_closed": 0,
                "session_ids": ["SessionHandle(3)"]}
        rec["outage"] = {"duration_ms": 60010.0, "texts_sent_during": 50, "texts_delivered_of_during": 50,
                         "delivery_after_restore_ms": {"median": 9000.0, "p95": 31000.0, "max": 40000.0},
                         "texts_sent": count, "texts_delivered": count, "texts_complete": True,
                         "session": {n: {"start": dict(mark), "restore": dict(mark), "end": dict(mark)}
                                     for n in ("alice", "bob")},
                         "same_session": True, "same_session_outage_only": True, "same_session_handles": True}
        rec["ptt"] = {"received": 50, "pushed": 50, "in_order_complete": True, "peer_opened_events": 1,
                      "peer_closed_events": 1, "press_to_first_audio_ms": 900.0}
    if arm == "ek-dosya-ptt":
        rec["file"] = {"bytes": 153600, "send_to_received_ms": 21000.0, "send_to_receipt_ms": 22000.0,
                       "bytes_match": True,
                       "outcome": {"status": "delivered", "reason": None, "sent_ms": 11000.0,
                                   "at_ms": 33000.0, "sender_failed_ms": None, "receiver_failed_reason": None}}
        rec["ptt"] = {"received": 50, "pushed": 50, "in_order_complete": True, "press_to_first_audio_ms": 700.0}
    return rec


def _m12_ack(i, t_ms, *, new_units=0, seen=12, largest=40, raw=None, mapped=3):
    """Sentetik `m_log` ACK kaydı (V2)."""
    view = {"state": "Steady", "inflight": 2, "pending_lost": 0, "tombstones": 1, "dup_queue": 1,
            "tail_probes": 0, "tail_armed_ms": None, "tail_probe_index": 0, "last_ack_ms": t_ms}
    return {"i": i, "t_ms": t_ms, "v2": True, "seen": seen, "largest_pn": largest,
            "ack_delay_raw": 0 if raw is None else raw, "ack_delay_ms": 0.0 if raw is None else raw * 0.256,
            "pns": 4, "mapped": mapped, "new_units": new_units, "tx_during": 0, "last_ack_now": True,
            "tail_rearmed_now": False, "before": dict(view), "after": dict(view)}


def _m12_state(i, t_ms, frm, to, *, emergency="Arq", fusion=False, conf=1000, last_ack=None):
    return {"i": i, "t_ms": t_ms, "from": frm, "to": to, "motor_transitions": i, "emergency": emergency,
            "fusion_emergency_mode": fusion, "ack_confidence_permille": conf, "last_ack_ms": last_ack}


def _selftest_m12() -> int:
    """[CEVAP PAKETİ 6] M1 / M2: ölçüt dosyası okunur; sentetik günlükte sınıflar, aralıklar, dizi,
    RecoverFwd konumları, eski kayıt sınıfları ve kesinti sonrası toparlanma beklenen değerleri verir."""
    failed = 0
    try:
        crit, sha = load_criteria(CRITERIA_FILE)
        ok = (len(sha) == 64 and abs(crit["ACK_CADENCE_FLOOR_MS"] - 8387.096774) < 1e-6
              and len(crit["OLD_S9_BOB_ACK_S"]) == 8 and crit["MIN_RATE_BPS"] == 31)
    except (OSError, ValueError) as e:
        print(f"  FAIL  ölçüt dosyası: {e}")
        return 1
    failed += not ok
    print(f"  {'PASS' if ok else 'FAIL'}  ölçüt dosyası okundu (sha256 {sha[:12]}…)")
    # gating2 s9: bob ucu — 2 P, 3 S (biri dönüş yolunda kayıp sonrası), 1 N, kapatan P.
    acks = [_m12_ack(1, 1000.0, new_units=2, seen=10), _m12_ack(2, 2000.0, new_units=1),
            _m12_ack(3, 10400.0, raw=30000), _m12_ack(4, 18790.0, raw=62802),
            _m12_ack(5, 35570.0, raw=65535), _m12_ack(6, 43570.0, seen=13),
            _m12_ack(7, 50000.0, new_units=1, seen=14)]
    states = [_m12_state(1, 500.0, None, "Startup"), _m12_state(2, 1500.0, "Startup", "Steady"),
              _m12_state(3, 30000.0, "Steady", "RecoverFwd", emergency="Dup2", conf=250, last_ack=18790.0),
              _m12_state(4, 40000.0, "RecoverFwd", "RecoverArq"),
              _m12_state(5, 45000.0, "RecoverArq", "RecoverFwd", fusion=True, conf=600, last_ack=43570.0),
              _m12_state(6, 51000.0, "RecoverFwd", "Steady")]
    g2 = _synthetic("gating2", 9)
    g2["m_log"] = {"enabled": True, "sessions": [{
        "side": "bob", "peer": "alice", "session": "SessionHandle(7)", "acks_ring_total": 7,
        "acks_logged": 7, "acks_missed": 0, "acks_dropped": 0, "acks": acks,
        "states_missed": 0, "states": states}]}

    def bob_sample(t_ms, conf, level):
        return {"t_ms": t_ms, "bob": {"sessions": [{
            "peer": "alice", "session": "SessionHandle(7)",
            "motor": {"ack_confidence_permille": conf, "emergency": level}}]}}

    g2["timeline"] = {"samples": [bob_sample(20000.0, 500, "Arq"), bob_sample(31000.0, 250, "Dup2")]}
    # ek-kesinti s1: alice → bob; kesinti 10–70 s; sonrası 130 s tabanda, sonra 600 B/s.
    k = _synthetic("ek-kesinti", 1)
    k["connect_requested_ms"] = 0.0
    k["outage"].update(applied_ms=10000.0, restored_ms=70000.0)
    samples = []
    for t in range(1000, 300001, 1000):
        if t < 3000:
            state, pacer, bw = "PreConverge", 31, 0
        elif t <= 70000:
            state, pacer, bw = ("Steady" if t < 10000 else "RecoverFwd"), 1000, 900
        elif t <= 200000:
            state, pacer, bw = "Steady", 31, 20
        else:
            state, pacer, bw = "Steady", 600, 600
        samples.append({"t_ms": float(t), "alice": {"sessions": [{
            "peer": "bob", "session": "SessionHandle(3)",
            "motor": {"state": state, "pacer_rate_bps": pacer, "btlbw_bps": bw,
                      "tx_datagrams": t // 1000, "tx_bytes": 70 * (t // 1000)}}]}})
    k["timeline"] = {"samples": samples}
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "seed-9-gating2.json").write_text(json.dumps(g2), encoding="utf-8")
        (d / "seed-1-ek-kesinti.json").write_text(json.dumps(k), encoding="utf-8")
        md, summary = render(load(d), [1, 9], crit, sha)
    m1 = [m for m in summary["m12"]["m1"] if m["side"] == "bob"]
    m = m1[0] if m1 else {}
    st = (m.get("streaks") or [{}])[0]
    entries = m.get("recover_fwd") or []
    ok = (len(m1) == 1 and (m["P"], m["S"], m["N"]) == (3, 3, 1)
          and [a["class"] for a in m["arrivals"]] == ["taban×1", "taban×1", "taban×2"]
          and [e["class"] for e in m["emissions"]] == ["taban×1"]
          and len(m["streaks"]) == 1 and st["duration_ms"] == 39600.0 and st["closed"]
          and (st["S"], st["N"]) == (3, 1) and st["recover_fwd_ms"] == 15000.0
          and st["recover_fwd_entries"] == 2 and st["min_ack_confidence_permille"] == 250
          and st["max_emergency"] == "Dup2"
          and [e["where"] for e in entries] == [WHERE[1], WHERE[2]]
          and [e["path"] for e in entries] == ["seviye", "füzyon"] and entries[0]["silence_ms"] == 11210.0
          and "### M1 — ACK çerçeveleri (gönderen ucu)" in md and f"sha256 `{sha}`" in md
          and "taban×1, taban×2, taban×1, taban×2, taban×1, taban×1, taban×3" in md
          and "| 3 / 3 / 1 |" in md and "| 2 / 1 / 0 / 0 / 0 |" in md)
    failed += not ok
    print(f"  {'PASS' if ok else 'FAIL'}  M1: P/S/N, varış ve üretim aralıkları, dizi, RecoverFwd konumu, eski kayıt")
    m2 = [x for x in summary["m12"]["m2"] if x["side"] == "alice"]
    x = m2[0] if m2 else {}
    rp, rb = x.get("recovery_pacer_rate_bps") or {}, x.get("recovery_btlbw_bps") or {}
    ow, ph = x.get("old_window") or {}, x.get("phases") or {}
    ok = (len(m2) == 1 and ph["önce"]["n"] == 7 and ph["önce"]["pacer"]["median"] == 1000
          and ph["sırası"]["btlbw"]["min"] == 900 and ph["sonrası"]["pacer"]["min"] == 31
          and round(ph["sonrası"]["floor_fraction"], 3) == round(130 / 230, 3)
          and rp["threshold"] == 500.0 and rp["first_s"] == 131.0 and rp["sustained_s"] == 131.0
          and rb["first_s"] == 131.0 and (ow["datagrams"], ow["bytes"], ow["span_s"]) == (125, 8750, 125.0)
          and "| 1 | alice→bob | 1000 | 500 | 131.0 | 131.0 | 131.0 | 131.0 | 125 · 8750 B · 70.0 B/s (125 s) | — |"
          in md
          and any(y["side"] == "bob" and y.get("error") for y in summary["m12"]["m2"]))
    failed += not ok
    print(f"  {'PASS' if ok else 'FAIL'}  M2: evreler, tabanda oran, toparlanma (ilk / kalıcı), eski çıkarım penceresi")
    return failed


def selftest() -> int:
    cases = [
        ("gating1 temiz → GEÇTİ", _synthetic("gating1", 1), "gating1", "GEÇTİ"),
        ("gating2 temiz → GEÇTİ", _synthetic("gating2", 1), "gating2", "GEÇTİ"),
        ("gating6 temiz → GEÇTİ", _synthetic("gating6", 1), "gating6", "GEÇTİ"),
        ("crypto retransmit → KALDI", _synthetic("gating1", 1, broken=True), "gating1", "KALDI"),
        ("negatif kayıplı → KAYIP GÖRÜNDÜ", _synthetic("negatif", 1), "negatif", "KAYIP GÖRÜNDÜ"),
        ("negatif kayıpsız → KAYIP GÖRÜNMEDİ", _synthetic("negatif", 1, lossy_negative=False), "negatif",
         "KAYIP GÖRÜNMEDİ"),
    ]
    no_netem = _synthetic("gating1", 1)
    no_netem["netem_check"]["ping"].update(received=20, rtt_avg_ms=0.05)
    cases.append(("netem ateşlemedi → GEÇERSİZ", no_netem, "gating1", "GEÇERSİZ"))
    epoch_before = _synthetic("gating2", 1)
    epoch_before["flows"]["bob_to_alice"]["first_send_epoch"] = 1
    cases.append(("gating2 ilk gönderim epoch-1 → KALDI", epoch_before, "gating2", "KALDI"))
    two_sessions = _synthetic("gating2", 1)
    two_sessions["nodes"]["alice"]["probe"]["session_events"]["ready"] = 2
    cases.append(("gating2 iki oturum → KALDI", two_sessions, "gating2", "KALDI"))
    dup = _synthetic("gating6", 1)
    dup["flows"]["bob_to_alice"]["duplicate_deliveries"] = 1
    cases.append(("gating6 çift teslim → KALDI", dup, "gating6", "KALDI"))
    failed = 0
    for name, rec, arm, want in cases:
        got = verdict(rec, arm)[0]
        ok = got == want
        failed += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {got}")
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        for seed in range(1, 6):
            for arm in ("gating1", "gating2", "gating6", "negatif"):
                (d / f"seed-{seed}-{arm}.json").write_text(json.dumps(_synthetic(arm, seed)), encoding="utf-8")
        md, summary = render(load(d), list(range(1, 6)))
        ok = (summary["t4"]["pass"] and "### T4" in md and "DUR-K4G-01 (iii)" in md
              and "### Takılı birim özeti" in md and "en eski u17: 7 karar, retx 3/3" in md
              and "next 17 · tutulan 31" in md)
        failed += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  5 tohum × 4 kol sentetik → T4 GEÇTİ ve tablolar")
    # [CEVAP PAKETİ 4] Yeniden koşu biçimi: 10 tohum GATING (2) pencere 600 s; ek-kesinti 5 tohum (biri
    # oturum değiştirmiş); ek-dosya-ptt 5 tohum (biri gönderende başarısız); v1 `drain_s` uyumu.
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        for seed in range(1, 11):
            (d / f"seed-{seed}-gating2.json").write_text(json.dumps(_synthetic("gating2", seed)), encoding="utf-8")
        for seed in range(1, 6):
            k = _synthetic("ek-kesinti", seed)
            if seed == 3:
                k["outage"]["same_session"] = False
                k["outage"]["session"]["bob"]["end"]["connected_events"] = 2
            (d / f"seed-{seed}-ek-kesinti.json").write_text(json.dumps(k), encoding="utf-8")
            f = _synthetic("ek-dosya-ptt", seed)
            if seed == 2:
                f["result"] = "incomplete"
                f["window"]["ended_by"] = "settled"
                f["file"]["outcome"].update(status="failed", reason="Outgoing(ReceiptTimeout)",
                                            sender_failed_ms=91000.0, at_ms=91000.0)
            (d / f"seed-{seed}-ek-dosya-ptt.json").write_text(json.dumps(f), encoding="utf-8")
        md, summary = render(load(d), list(range(1, 11)))
        old = _synthetic("gating1", 1)
        old["params"] = {"count": 100, "drain_s": 120}
        ok = ("**GATING (2): 10/10 tohum GEÇTİ** · pencere 600 s" in md
              and "60 s kesinti, aynı oturumda %100 metin teslimi): 4/5 tohum**" in md
              and "#### PTT (ayrı)" in md and "1→2" in md
              and "| failed | Outgoing(ReceiptTimeout) |" in md
              and summary["ek_dosya_ptt"]["2"]["status"] == "failed"
              and summary["ek_kesinti"]["3"]["criterion"] == "KARŞILANMADI"
              and window_cell(old).startswith("120 · "))
        failed += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  yeniden koşu biçimi (10×GATING (2), ek-kesinti, ek-dosya-ptt, v1 uyumu)")
    failed += _selftest_m12()
    print("int02_analyze öz-test:", "YEŞİL" if not failed else f"KIRMIZI ({failed})")
    return 0 if not failed else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("directory", nargs="?", type=Path)
    ap.add_argument("--md", type=Path, help="Markdown çıktısı (varsayılan: stdout)")
    ap.add_argument("--json", type=Path, help="hüküm özeti (JSON)")
    ap.add_argument("--criteria", type=Path, default=CRITERIA_FILE,
                    help="M1/M2 ölçüt ve beklenti kaydı (varsayılan: docs/reports/INT-02-M1M2-BEKLENTI.md)")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    if args.directory is None or not args.directory.is_dir():
        ap.error("çıktı dizini gerekli (int02_run.sh --out)")
    runs = load(args.directory)
    if not runs:
        print(f"{args.directory}: seed-<S>-<kol>.json yok", file=sys.stderr)
        return 2
    seeds = sorted({s for s, _ in runs})
    crit, crit_sha = None, None
    try:
        crit, crit_sha = load_criteria(args.criteria)
    except (OSError, ValueError) as e:
        print(f"M1/M2 bölümü atlandı: {e}", file=sys.stderr)
    md, summary = render(runs, seeds, crit, crit_sha)
    header = (f"<!-- int02_analyze.py · kaynak: {args.directory} · tohumlar: {' '.join(map(str, seeds))}"
              + (f" · M1/M2 ölçütü {args.criteria.name} sha256 {crit_sha}" if crit_sha else "") + " -->\n")
    if args.md:
        args.md.write_text(header + md + "\n", encoding="utf-8")
    else:
        print(header + md)
    if args.json:
        args.json.write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
