# Massive ad protocol, revision 5 (1 vs 100, Xbox 360)

| Path | Content |
|---|---|
| `docs/` | protocol description (`mdbook build docs`) |
| `docs/src/status.md` | reverse engineering status: known, uncertain, to do |
| `crates/massive-proto/` | codec library: envelope, MAC, schemas with confidence, typed messages |
| `crates/massived/` | field server, capture decoder and client probe |

```text
cargo test --workspace
cargo run -p massived -- check-config -c crates/massived/massived.example.toml
cargo run -p massived -- serve -c crates/massived/massived.example.toml
```

Changes to the protocol knowledge go to the book chapter, `status.md` and
the schema in `crates/massive-proto/src/schema.rs` together.
