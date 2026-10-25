# Method and sources

## Inputs

| Source | Location |
|---|---|
| Retail Lua game code | `common/game_builds/v2.0.1149.0.02-2009-10-26_retail/584A07D1/00000002/extracted/lua/` |
| Retail executable | `default.xex` (PowerPC, image base `0x82000000`) |
| IDA driver | `rev3/tools/ida-1vs100-retail` (persistent idalib process; commands sent as Python snippets) |
| Independent write-up | github.com/Aib0t/write-ups, `massive_ad_client/readme.md` (MAD 3.x on PC and Skate 2 Xbox 360 symbols); compared in [Cross-check with MAD 3.x](crosscheck.md) |
| Independent server | github.com/moio9/NFS-Online-Server, `server/carbon/mad/` (MAD 3.2 server for the retail NFS Carbon PC client, AGPL-3.0); compared in [Cross-check with MAD 3.x](crosscheck.md) |
| Debug build | `common/game_builds/v1.0.808.0_debug_unmodified/1vs100-fre.xex`, the same Massive library (XDK 7978.3) with logging compiled in and partial symbols; IDA driver `rev3/tools/ida` |
| Captured session | `common/game_builds/v1.0.808.0_debug_unmodified/AdClientLog.txt`, the debug client's log of a session against the Massive test servers on 2009-04-13; see [Captured session](capture.md) |

## Process

1. **Anchor on strings.** The Massive library carries debug strings (`XMassiveAdClient::EnterZone`, `CRequestManager::SubmitRequest`, `/adsrv/5/enterZone`, `Adclient Massive Inc./`, …). Every function that references one was identified and named after it.
2. **Walk the library call graph** breadth-first from the request creators, response parsers and transport. This produced a closure of 322 functions in `0x824F0000–0x82510000`, dumped one file per function under `work/asm2/`.
3. **Read the request builders bottom-up.** Each `Create*Request` allocates a `CRequestObject`, the `Write*Body` function fills it, and `sub_824FA478` seals it. Primitive writers (tag, `u16`, `u32`, `u64`, length-prefixed string, prepend vs. append) were decoded first and then used to read every body.
4. **Read the response parsers the same way**, starting with the status byte and section tags, then the sub-tag dispatch tables (`ReadAssetBlock` uses a byte jump table at `0x82084998`).
5. **Trace the value sources** with cross-references to the globals (`qword_82A0EEA8`, `qword_82A0EEB0`, `dword_82A0EF08`, `dword_82A0EF04`, client fields `+0x158/+0x15C`).
6. **Rename in the IDB.** All identified functions were renamed (`massive_*`, `lua_Massive_*`) and the database was saved. The rename table is `work/ida/renames.py`. Helpers are in `work/ida/helpers.py`.
7. **Verify by reconstruction.** The framing, HMAC and body builders were written in Python (`work/massive_envelope.py`) and round-trip tested, which checks the framing is self-consistent.
8. **Carry names over from the debug build.** A FLIRT signature built from the v1.0.808.0 database (`rev3/tools/ida-1vs100-retail/signatures/1vs100-808-debug.sig`) and a function matcher (code hash, string references, call graph, link order, opcode n-grams; result in `808-to-1149-matches.json`) named 8 300 further functions in the retail database. Matcher names carry a function comment with the debug build address and the match method. Existing `massive_*` names were kept.
9. **Check against the capture.** The debug client logs every field it writes and reads, and dumps each request. The dumps were decoded with the tag tables of this book and with `massive-proto`.

## Conventions

- Addresses are XEX virtual addresses (`0x82xxxxxx`). `sub_XXXXXXXX` is the IDA default name; `massive_*` names are the ones given during this work.
- Big-endian is used everywhere (PowerPC). All multi-byte integers on the wire are big-endian.
- `[C]`, `[I]`, `[U]` as defined in the [Introduction](introduction.md).

## Reproducing a function listing

```
cd rev3/tools/ida-1vs100-retail
./ida_exec.sh <<'EOF'
exec(open('/home/dev/project/1vs100/adserver_v5/work/ida/helpers.py').read())
print(dis(0x82501818))
EOF
```
