# Massive ad protocol, revision 5 (1 vs 100, Xbox 360)

[![ci](https://github.com/1vs100/mads_server_v5/actions/workflows/ci.yml/badge.svg)](https://github.com/1vs100/mads_server_v5/actions/workflows/ci.yml)
[![docs](https://img.shields.io/badge/docs-docs.1vs100.fans-blue?logo=readthedocs&logoColor=white)](https://docs.1vs100.fans/mads_server_v5/)
![Static Badge](https://img.shields.io/badge/ai-slop-orange)

| Path | Content |
|---|---|
| `docs/` | protocol description (`mdbook build docs`) |
| `docs/src/status.md` | reverse engineering status: known, uncertain, to do |
| `crates/massive-proto/` | codec library: envelope, MAC, schemas with confidence, typed messages |
| `crates/massived/` | field server, capture decoder and client probe |

```text
cargo test --workspace
cargo run -p massived -- check-config -c crates/massived/massived.example.toml --advertise-host 127.0.0.1 --advertise-port 1002
cargo run -p massived -- serve -c crates/massived/massived.example.toml --listen 127.0.0.1:1002 --advertise-host 127.0.0.1
```

Changes to the protocol knowledge go to the book chapter, `status.md` and
the schema in `crates/massive-proto/src/schema.rs` together.
