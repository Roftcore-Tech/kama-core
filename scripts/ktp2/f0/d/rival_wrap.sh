#!/usr/bin/env bash
# [KTP2-F0 / Ajan D] İstemci sarmalayıcı (salt-gözlem). Komutu AYNEN koşar ve rc'sini döner;
# öncesinde/sonrasında istemci ağ ad alanının sayaçlarını kaydeder:
#   snmp_pre/post.txt     /proc/self/net/snmp   (Tcp: OutSegs RetransSegs …, Udp: …)
#   netstat_pre/post.txt  /proc/self/net/netstat (TcpExt: TCPTimeouts TCPLossProbes …)
#   tcp_cc.txt            bu ad alanında yürürlükteki TCP tıkanıklık denetimi
# Netns hücre boyunca koşular arasında paylaşılır ama koşular ardışıktır → fark koşuya aittir.
# Zaman aşımında (timeout SIGTERM) "post" dosyaları yazılmayabilir; analiz bunu "yok" sayar.
# Kullanım: rival_wrap.sh KOŞU_DİZİNİ KOMUT [ARG…]
set -u
d="${1:?kullanım: rival_wrap.sh KOŞU_DİZİNİ KOMUT [ARG…]}"; shift
cat /proc/self/net/snmp > "$d/snmp_pre.txt" 2>/dev/null || true
cat /proc/self/net/netstat > "$d/netstat_pre.txt" 2>/dev/null || true
cat /proc/sys/net/ipv4/tcp_congestion_control > "$d/tcp_cc.txt" 2>/dev/null || echo unknown > "$d/tcp_cc.txt"
"$@"
rc=$?
cat /proc/self/net/snmp > "$d/snmp_post.txt" 2>/dev/null || true
cat /proc/self/net/netstat > "$d/netstat_post.txt" 2>/dev/null || true
exit "$rc"
