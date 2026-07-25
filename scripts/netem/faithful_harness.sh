#!/usr/bin/env bash
# Commit-FQ — FAITHFUL TWO-ENDPOINT WALL-CLOCK netem harness (rootless; docker-daemon-GEREKMEZ).
#
# Rootless tek-namespace-veth netem GECİKME-UYGULAMAZ (kanıt: prior ping RTT≈0.02ms). Bu harness İKİ
# AYRI network-namespace kullanır → cross-namespace veth-forwarding egress-qdisc'i SADAKATLE uygular
# (kanıt: RTT-acceptance-pin §aşağıda). Yöntem: `unshare -rnm` (parent-netns A + CAP_NET_ADMIN in userns)
# → child-netns B `unshare -n sleep BIG` (PID-referanslı) → `ip link set v1 netns $CHILD` (veth-ucu-B'ye)
# → `nsenter -t $CHILD -n` ile B-tarafı yapılandır/çalıştır. /var/run/netns-mount GEREKMEZ (PID-referans).
#
# Kullanım: bu dosya `source` edilir; `faithful_run FWD_NETEM REV_NETEM RX_CMD TX_CMD [BLACKOUT_SPEC]`.
# FWD_NETEM/REV_NETEM = tam `tc qdisc ... netem <spec>` argümanı (ör. "delay 1250ms loss 10%").
# RX_CMD child-netns'te (nsenter) koşar; TX_CMD parent-netns'te. IP'ler: tx=10.9.0.1, rx=10.9.0.2.
#
# Bu script SADECE unshare-INNER gövdesini tanımlar; çağıran `unshare -rnm bash -c "... source ...; faithful_run ..."`.

# Parent-netns içinde iki-uçlu topoloji kur; $CHILD (rx-netns-pid) global bırak.
faithful_setup() { # yalnız unshare -rnm İÇİNDE çağrılır
  ip link add v0 type veth peer name v1
  unshare -n sleep 100000 & CHILD=$!
  # child-netns hazır olana kadar bekle (/proc/$CHILD/ns/net)
  for _ in $(seq 1 50); do [ -e /proc/$CHILD/ns/net ] && break; sleep 0.05; done
  ip link set v1 netns "$CHILD"
  ip addr add 10.9.0.1/24 dev v0; ip link set v0 up; ip link set lo up
  nsenter -t "$CHILD" -n ip addr add 10.9.0.2/24 dev v1
  nsenter -t "$CHILD" -n ip link set v1 up
  nsenter -t "$CHILD" -n ip link set lo up
  # STATİK-ARP (her-iki-yön): reverse-blackout ARP-reply'i-düşürünce forward-teslimin-ARP'a-bağımlı
  # OLMAMASI için (DI-2B/W-3 blackout senaryoları; aksi netem-loss ARP'ı-kilitler → forward-de-durur).
  local m0 m1
  m0=$(ip link show v0 | awk '/link\/ether/{print $2}')
  m1=$(nsenter -t "$CHILD" -n ip link show v1 | awk '/link\/ether/{print $2}')
  ip neigh replace 10.9.0.2 lladdr "$m1" dev v0 nud permanent
  nsenter -t "$CHILD" -n ip neigh replace 10.9.0.1 lladdr "$m0" dev v1 nud permanent
  export CHILD
}

faithful_teardown() {
  [ -n "${CHILD:-}" ] && kill "$CHILD" 2>/dev/null || true
  ip link del v0 2>/dev/null || true   # netns yok-olunca zaten temizlenir; yine de dene
}

# Forward = parent v0-egress (tx→rx). Reverse = child v1-egress (rx→tx).
faithful_netem() { # $1=fwd-spec $2=rev-spec
  tc qdisc replace dev v0 root netem $1
  nsenter -t "$CHILD" -n tc qdisc replace dev v1 root netem $2
}

# rx-tarafı komutu child-netns'te koştur.
faithful_rx() { nsenter -t "$CHILD" -n "$@"; }
