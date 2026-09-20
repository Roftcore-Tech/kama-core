#!/usr/bin/env python3
"""veth offload kapat/doğrula — ethtool YOKKEN SIOCETHTOOL ioctl (yalnız stdlib)."""
import fcntl, socket, struct, sys, array
SIOCETHTOOL = 0x8946
GET = {"tso": 0x1e, "gso": 0x23, "gro": 0x2b, "sg": 0x18, "txcsum": 0x16, "rxcsum": 0x14}
SET = {"tso": 0x1f, "gso": 0x24, "gro": 0x2c}
def eth(sock, ifn, cmd, val=0):
    buf = array.array("B", struct.pack("II", cmd, val))
    addr, _ = buf.buffer_info()
    ifr = struct.pack("16sP", ifn.encode(), addr)
    fcntl.ioctl(sock.fileno(), SIOCETHTOOL, ifr)
    return struct.unpack("II", buf.tobytes())[1]
def main():
    ifn = sys.argv[1]; s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    before = {k: eth(s, ifn, c) for k, c in GET.items()}
    for k, c in SET.items(): eth(s, ifn, c, 0)
    after = {k: eth(s, ifn, c) for k, c in GET.items()}
    print(f"{ifn} before={before} after={after}")
    sys.exit(0 if all(after[k] == 0 for k in SET) else 1)
main()
