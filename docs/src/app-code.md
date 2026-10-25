# Appendix C: Reference code

The maintained implementation is the Rust crate `crates/massive-proto`, with the field server `crates/massived`. The Python reference below is kept for comparison; the Rust tests check that both produce the same bytes.

The Python reference is in `work/massive_envelope.py` (included below). It implements the sealing, the body primitives, the request bodies, the response framing and the MAC checks. Running it executes a self-test:

```text
$ python3 work/massive_envelope.py
massive_envelope self-test OK (framing is internally consistent; not validated against live traffic)
```

The self-test checks:

- a sealed message of type `0xCB` round-trips and its MAC verifies;
- a tampered message fails the MAC;
- LocateService round-trips unsealed;
- the player block has the exact bytes from `massive_WritePlayerUpdateBlocks`;
- the token derivation (MD5 hex of the MAC string);
- heartbeat, statistics and impression messages round-trip;
- a signed response parses and its MAC verifies, and a corrupted one does not.

The self-test does **not** prove the layout matches a real server.

## massive_envelope.py

```python
{{#include ../../work/massive_envelope.py}}
```
