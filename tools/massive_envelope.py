#!/usr/bin/env python3
"""
Reference implementation of the MASSIVE ADS (MassiveAdClient3) wire envelope and
field primitives, transcribed from the retail XEX (1vs100 v2.0.1149.0.02, title 584A07D1).

Source functions (XEX addresses):
  sub_824FA478  seal/finish buffer  -> seal()          (type/time/sign framing)
  sub_82505760  HMAC-SHA1           -> hmac_sha1()
  sub_824F9EA8  tag byte writer     -> tag()
  sub_824F9F68  u64 writer          -> u64()
  sub_824FA640  u16-length string   -> str16()
  sub_824F9EE8  u16 writer          -> u16()
  sub_824FA218  player-block writer -> player_blocks()
  sub_82501818  OpenSession body    -> open_session_body()   (0xCB)
  sub_824FF310  PlayerUpdate body   -> player_update_body()  (0xD8)
  sub_82503798  EnterZone body      -> enter_zone_body()     (0xCD)
  sub_82503520  Impression body     -> impression_body()     (0xD3)
  sub_824FEAF8  LocateService body  -> locate_body()         (0xC9, unsealed)

The Rust crate crates/massive-proto supersedes this file. The framing below is confirmed against the code. It has NOT been validated
against live server traffic (the server is no longer reachable and no capture exists).
The self-test only checks internal consistency of the framing.

All integers are big-endian (PowerPC).
"""
import hmac
import hashlib
import struct

# Constant HMAC key (32 ASCII '1' characters), literal at XEX 0x82084xxx
# (also written as a body field, tag 'D', by sub_82501818).
MASSIVE_HMAC_KEY = b"1" * 32

# Envelope type bytes (first argument of sub_824FA478 at each call site)
TYPE_OPEN_SESSION = 0xCB
TYPE_LOCATE_SERVICE = 0xC9
TYPE_CLOSE_SESSION = 0xD1
TYPE_PLAYER_UPDATE = 0xD8
TYPE_ENTER_ZONE = 0xCD
TYPE_IMPRESSION = 0xD3
TYPE_IMPRESSION_UPDATE = 0xD3
TYPE_PLAYER_UPDATE_BLOCK = 0xE4     # block tag inside body (sub_824FA218)

CLIENT_VERSION_STRING = b"XDK Test QFE 1v1009328.3"   # build string in OpenSession body (tag 0x16)
USER_AGENT_PREFIX = b"Adclient Massive Inc./"          # User-Agent prefix (sub_82506308)


def hmac_sha1(key: bytes, msg: bytes) -> bytes:
    """HMAC-SHA1 (sub_82505760 implements standard RFC 2104 ipad/opad)."""
    return hmac.new(key, msg, hashlib.sha1).digest()


# ---- field primitives (append semantics) --------------------------------------------

def tag(t: int) -> bytes:
    return bytes([t & 0xFF])


def u16(v: int) -> bytes:
    return struct.pack(">H", v & 0xFFFF)


def u32(v: int) -> bytes:
    return struct.pack(">I", v & 0xFFFFFFFF)


def u64(v: int) -> bytes:
    return struct.pack(">Q", v & 0xFFFFFFFFFFFFFFFF)


def str16(s: bytes) -> bytes:
    """u16 BE length followed by raw bytes (sub_824FA640, max 2000 bytes)."""
    if len(s) > 2000:
        raise ValueError("string longer than 2000 bytes")
    return u16(len(s)) + s


# ---- sealing ------------------------------------------------------------------------

def seal(body: bytes, msg_type: int, with_time: bool, sign: bool,
         time8: bytes = b"\x00" * 8, key: bytes = MASSIVE_HMAC_KEY) -> bytes:
    """
    Transcription of sub_824FA478(this, type, timeFlag, signFlag).

    Resulting layout:
      05 | type | u32 L | body | [';' | time8] | [1E] | [00 14 | HMAC-SHA1(20)]
    where L = len(body) + (9 if with_time) + (1 if sign) + (22 if sign)  -- i.e. the
    number of bytes that follow the L field.  HMAC covers  type|L|body|[;time]|[1E].
    """
    buf = bytearray(body)
    if with_time:                       # append ';' + 8-byte time
        buf += b";" + time8
    if sign:                            # append 0x1E marker
        buf += b"\x1e"
    length = len(buf) + (22 if sign else 0)
    buf = bytearray(u32(length)) + buf  # prepend L
    buf = bytearray(tag(msg_type)) + buf  # prepend type
    if sign:
        mac = hmac_sha1(key, bytes(buf))  # HMAC over type|L|body|;time|1E
        buf += u16(20) + mac              # append 00 14 + digest
    buf = b"\x05" + bytes(buf)          # prepend 0x05 (message start marker)
    return bytes(buf)


def unseal(msg: bytes, key: bytes = MASSIVE_HMAC_KEY, expect_time: bool = True,
           expect_sign: bool = True):
    """
    Inverse of seal(); returns (type, body, time8, mac_ok).
    L counts every byte after the L field, including the signature trailer
    (00 14 + 20-byte MAC) when signed.
    """
    if msg[0:1] != b"\x05":
        raise ValueError("missing 0x05 start marker")
    msg_type = msg[1]
    length = struct.unpack(">I", msg[2:6])[0]
    if len(msg) < 6 + length:
        raise ValueError("truncated: L=%d have=%d" % (length, len(msg) - 6))
    inner = msg[6:6 + length]          # body | [;time] | [1E] | [0014 | MAC]
    mac_ok = None
    if expect_sign:
        if len(inner) < 22 + 1:
            raise ValueError("signed message too short")
        trailer = inner[-22:]
        mac = trailer[2:]
        covered = msg[1:6 + length - 22]   # type|L|body|;time|1E  (HMAC input)
        mac_ok = (trailer[:2] == u16(20)) and hmac_sha1(key, covered) == mac
        inner = inner[:-22]
        if inner[-1:] != b"\x1e":
            raise ValueError("missing 0x1E sign marker")
        inner = inner[:-1]
    time8 = None
    body = inner
    if expect_time:
        if len(inner) < 9 or inner[-9:-8] != b";":
            raise ValueError("missing ';' time marker")
        time8 = inner[-8:]
        body = inner[:-9]
    return msg_type, body, time8, mac_ok


# ---- body builders -------------------------------------------------------------------

def player_blocks(players):
    """
    sub_824FA218 WritePlayerUpdateBlocks.  `players` is a list of up to 4 16-byte IDs.
    Each occupied slot i writes:
      E4 | 00 00 00 15 | 'W'(57) | i | 'A'(41) | 00 10 | <16-byte id>
    The loop stops at the first empty slot.
    """
    out = b""
    for i, pid in enumerate(players[:4]):
        if pid is None:
            break
        if len(pid) != 16:
            raise ValueError("player id must be 16 bytes")
        out += tag(0xE4) + u32(0x15) + b"W" + tag(i) + b"A" + u16(0x10) + pid
    return out


def open_session_body(a: bytes, b: bytes, token: bytes, players=(), flag_token=True):
    """
    sub_82501818.  Field order (append):
      '='(3D) str16(a) '>'(3E) str16(b) '<'(3C) 01 | 18 00 | 1B 01 |
      'D'(44) str16('1'*32) | [1D str16(token)] | 16 str16(build) | player blocks
    Sealed with type 0xCB, time=1, sign=1.
    `a`/`b` are the SKU name and version; `token` is device_token().
    """
    body = b"="
    body += str16(a) + b">" + str16(b)
    body += b"<" + b"\x01" + b"\x18" + b"\x00" + b"\x1b" + b"\x01"
    body += b"D" + str16(MASSIVE_HMAC_KEY)
    if flag_token and token:
        body += b"\x1d" + str16(token)
    body += b"\x16" + str16(CLIENT_VERSION_STRING)
    body += player_blocks(list(players))
    return seal(body, TYPE_OPEN_SESSION, with_time=True, sign=True)


def player_update_body(session_id: int, session_token: int, players=()):
    """
    sub_824FF310.  Prepends are used, so the final order is:
      '*'(2A) u64(session_token) '+'(2B) u64(session_id) | player blocks
    where session_token = qword_82A0EEB0 and session_id = qword_82A0EEA8 (globals).
    Sealed with type 0xD8, time=1, sign=1.
    """
    body = b"\x2a" + u64(session_token) + b"\x2b" + u64(session_id) + player_blocks(list(players))
    return seal(body, TYPE_PLAYER_UPDATE, with_time=True, sign=True)


def enter_zone_body(zone_name: bytes, session_id: int, session_token: int, players=(),
                    peer_public=None, peer_private=None, peer_port=None):
    """
    sub_82503798.  Appended order:
      'G'(47) str16(zone) '*'(2A) u64(token) '+'(2B) u64(session) | player blocks
      [10 u32 (sub_824F9F28)] [11 u32] [12 u16 (sub_824F9EE8)]  (all three or none)
    Sealed with type 0xCD, time=1, sign=1.
    """
    body = b"G" + str16(zone_name) + b"\x2a" + u64(session_token) + b"\x2b" + u64(session_id)
    body += player_blocks(list(players))
    if peer_public and peer_private and peer_port:
        body += b"\x10" + u32(peer_public) + b"\x11" + u32(peer_private) + b"\x12" + u16(peer_port)
    return seal(body, TYPE_ENTER_ZONE, with_time=True, sign=True)


def impression_body(field_0x50: int, field_0x52: int):
    """
    sub_82503520.  '!'(21) u16(field_0x50) ')'(29) u16(field_0x52).
    Per-impression records are appended by sub_82502C50 (not traced in detail).
    Sealed with type 0xD3, time=1, sign=1.
    """
    body = b"!" + u16(field_0x50) + b")" + u16(field_0x52)
    return seal(body, TYPE_IMPRESSION, with_time=True, sign=True)


def locate_body(a: bytes, b: bytes, token: bytes):
    """
    sub_824FEAF8 (LocateService).  '='(3D) str16(a) '>'(3E) str16(b)
    [1D str16(token) when the device-id source is present], unsealed (no time, no signature), type 0xC9.
    """
    body = b"=" + str16(a) + b">" + str16(b) + b"\x1d" + str16(token)   # no '<' tag in this writer
    return seal(body, TYPE_LOCATE_SERVICE, with_time=False, sign=False)


# ---- HTTP framing (sub_825061f0 / sub_82506308) ------------------------------------

def http_request(method: str, path: str, host: str, body: bytes, user_agent_suffix: bytes = b"XDK Test QFE 1v1009328.3") -> bytes:
    """
    Reproduces the request template:
      'POST ' <path> ' HTTP/1.1\\r\\nHost:%s\\r\\nContent-Length:%d\\r\\n'
      'Content-Type:application/massive\\r\\nUser-Agent:%s%s\\r\\n\\r\\n' <body>
    GET (asset download) uses 'GET ' with the asset URL (sub_82506308 checks request type 0x54).
    """
    head = (
        f"{method} {path} HTTP/1.1\r\n"
        f"Host:{host}\r\n"
        f"Content-Length:{len(body)}\r\n"
        f"Content-Type:application/massive\r\n"
        f"User-Agent:{USER_AGENT_PREFIX.decode()}{user_agent_suffix.decode()}\r\n\r\n"
    ).encode("latin-1")
    return head + body


# ---- session identity (sub_824FC9F8 / sub_82501EF8) -------------------------------

def console_mac_string(mac: bytes) -> bytes:
    """sub_824FC9F8: 6-byte MAC -> '%02X:%02X:%02X:%02X:%02X:%02X' (17 chars + NUL in an 18-byte buffer)."""
    if len(mac) != 6:
        raise ValueError("MAC must be 6 bytes")
    return ":".join("%02X" % b for b in mac).encode("ascii")


def device_token(mac: bytes) -> bytes:
    """sub_824F3A90 + sub_82501EF8: lowercase hex of MD5(mac string). The 33-byte buffer
    holds a NUL, but sub_824FA640 writes strnlen() bytes, so 32 characters go on the wire."""
    return hashlib.md5(console_mac_string(mac)).hexdigest().encode("ascii")


# ---- additional request bodies -----------------------------------------------------

def heartbeat_body(session_id: int, session_token: int):
    """sub_825074E0 (sealed type 0xD6, time+sign). Prepend-built: 2A QB0 2B QA8."""
    body = b"\x2a" + u64(session_token) + b"\x2b" + u64(session_id)
    return seal(body, 0xD6, with_time=True, sign=True)


def stats_body(session_id: int, session_token: int, c_x: int, c_y: int, c_z: int):
    """sub_82507800 (sealed type 0xDA, time+sign). Append-built:
    2B QA8 2A QB0 'X' u16 'Y' u16 '\\' u16 (counters from global struct dword_82A0EF04 +0x60/+0x62/+0x64).
    Field meaning of the three counters is not known [U]."""
    body = b"\x2b" + u64(session_id) + b"\x2a" + u64(session_token)
    body += b"X" + u16(c_x) + b"Y" + u16(c_y) + b"\\" + u16(c_z)
    return seal(body, 0xDA, with_time=True, sign=True)


def impression_record(ie_name: bytes) -> bytes:
    """sub_824FA6F0: E6 | u32 (len+3) | '=' | str16(name)."""
    inner = b"=" + str16(ie_name)
    return b"\xe6" + u32(len(inner)) + inner


def impression_request(session_id: int, session_token: int, names, players=(), count_0x50=None, count_0x52=None):
    """sub_82502C50 (records + player blocks) followed by sub_82503520 ('!' u16, ')' u16), sealed 0xD3."""
    body = b"\x2a" + u64(session_token) + b"\x2b" + u64(session_id)
    for n in names:
        body += impression_record(n)
    body += player_blocks(list(players))
    body += b"!" + u16(count_0x50 or 0) + b")" + u16(count_0x52 or 0)
    return seal(body, TYPE_IMPRESSION, with_time=True, sign=True)


# ---- response framing (sub_824FA0B8 / sub_824FA108 / sub_824F9300) -----------------

def parse_response(resp: bytes, key: bytes = MASSIVE_HMAC_KEY):
    """
    Response framing as implemented by the parsers (sub_825016D8, sub_82504728, ...):
      05            protocol version, must be 0x05 (sub_824FA0B8 removes it before parsing)
      type          request type + 1 (CA, CC, CE, D2, D4, D7, D9, DB)
      u32 BE L      bytes that follow, including the 22-byte trailer when signed
      ...tags...
      1E u16(20) MAC  signature trailer; MAC = HMAC-SHA1 over everything before the 00 14 bytes
    Returns (section_tag, inner_bytes_after_L_without_trailer, mac_ok).
    """
    if resp[0] != 0x05:
        raise ValueError("version byte is not 0x05")
    sec = resp[1]
    length = struct.unpack(">I", resp[2:6])[0]
    inner = resp[6:6 + length]
    # signature trailer is the last 22 bytes of the counted region, preceded by 0x1E
    if len(inner) < 23 or inner[-23:-22] != b"\x1e":
        return sec, inner, None
    mac = inner[-20:]
    covered = resp[1:6 + length - 22]        # type | L | ... | 1E  (version byte excluded)
    mac_ok = hmac_sha1(key, covered) == mac and inner[-22:-20] == u16(20)
    return sec, inner[:-22], mac_ok


# ---- self test ----------------------------------------------------------------------

def _selftest():
    time8 = bytes(range(1, 9))
    body = b"hello-massive"
    # signed + timed round trip
    msg = seal(body, TYPE_OPEN_SESSION, with_time=True, sign=True, time8=time8)
    t, b, tm, mac_ok = unseal(msg)
    assert t == TYPE_OPEN_SESSION, t
    assert b == body, b
    assert tm == time8, tm
    assert mac_ok is True, "HMAC did not verify"
    # unsigned, untimed (locate) round trip
    msg2 = seal(b"locate", TYPE_LOCATE_SERVICE, with_time=False, sign=False)
    assert msg2[0] == 0x05 and msg2[1] == TYPE_LOCATE_SERVICE
    assert struct.unpack(">I", msg2[2:6])[0] == len(b"locate")
    assert msg2[6:] == b"locate"
    # tampering must break the MAC
    bad = bytearray(msg); bad[10] ^= 0xFF
    assert unseal(bytes(bad))[3] is False
    # player block layout
    pb = player_blocks([b"A" * 16])
    assert pb == b"\xe4\x00\x00\x00\x15W\x00A\x00\x10" + b"A" * 16, pb.hex()
    # identity token: MD5 hex of MAC string, no NUL on the wire
    assert device_token(bytes.fromhex("001122aabbcc")) == hashlib.md5(b"00:11:22:AA:BB:CC").hexdigest().encode()
    hb = heartbeat_body(1, 2)
    assert unseal(hb)[0] == 0xD6 and unseal(hb)[3] is True
    st = stats_body(1, 2, 3, 4, 5)
    assert unseal(st)[0] == 0xDA and unseal(st)[3] is True
    ir = impression_request(1, 2, [b"Video15_1"])
    assert unseal(ir)[0] == TYPE_IMPRESSION and unseal(ir)[3] is True
    # response round-trip: build a signed response (version 05, type CC, L, body, 1E 0014 MAC)
    inner_no_sig = b"\x47\x01\x02\x03"     # arbitrary tagged content
    L = len(inner_no_sig) + 1 + 22              # counts body | 1E | 0014 | MAC
    covered = b"\xcc" + u32(L) + inner_no_sig + b"\x1e"
    resp = b"\x05" + covered + u16(20) + hmac_sha1(MASSIVE_HMAC_KEY, covered)
    sec, payload, ok = parse_response(resp)
    assert sec == 0xCC and ok is True and payload == inner_no_sig + b"\x1e", (sec, payload, ok)
    bad = bytearray(resp); bad[8] ^= 1
    assert parse_response(bytes(bad))[2] is False
    print("massive_envelope self-test OK (framing is internally consistent; not validated against live traffic)")


if __name__ == "__main__":
    _selftest()
