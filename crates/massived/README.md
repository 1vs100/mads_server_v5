# massived

Field server for the Massive ad client protocol, revision 5, as used by
*1 vs 100* (Xbox 360). It answers the retail client, logs the observations
listed in the reverse engineering status (`massive-book/src/status.md`), and
captures raw exchanges for offline analysis.

The protocol codec is the `massive-proto` crate in this workspace.

## Commands

```text
massived serve -c massived.toml --listen 0.0.0.0:1002 --advertise-host ads.example.net   serve the protocol
massived check-config -c massived.toml --advertise-host ads.example.net --advertise-port 1002   validate configuration and asset files
massived probe --target HOST:PORT         run the retail request sequence against a server
massived decode --capture capture.jsonl   decode a capture
massived decode --hex 05d70000...         decode one message or HTTP exchange
massived decode --file message.bin        decode raw bytes
```

Global options: `--log-format text|json`, `--log-filter <directive>` (else
`RUST_LOG`, else `massived=info`), `--log-file <path>` for an additional JSON
log.

`massived=debug` adds the decoded field tree of every request, the raw HTTP
headers and response timing.

## Configuration

See `massived.example.toml`. Relative paths are resolved against the
directory of the configuration file. All content is checked at start-up with
the client's own rules: media types that select no object class, asset
records that would make `CreateAsset` report an error, and references to
undefined assets are rejected.

The file holds content and protocol settings only. Addresses are arguments, each also read from the environment:

- `--listen` (`MASSIVED_LISTEN`): addresses to accept connections on, comma separated.
- `--advertise-host` (`MASSIVED_ADVERTISE_HOST`): host name or IPv4 address the client is told to use for all three services and in asset URLs.
- `--advertise-port` (`MASSIVED_ADVERTISE_PORT`): defaults to the first `--listen` port.

The client obtains the server address through the game's XLSP lookup on the tag `MASSIVE`. That lookup is outside this program; point it at one of the `listen` addresses.

Every OpenSession response lists the zones of `content.announce_zones` in field `0x47`, followed by any configured zone missing from it. The default is the 14 zones of the retail `data.lua` (`ZonesPT`, `ZonesEP`). EnterZone for an announced zone without content returns an empty zone and logs `unknown_zone`.

## Events

Events are tracing events whose message is the event name. Fields are
structured; with `--log-format json` they appear under `fields`.

| Event | Level | Fields | Use |
|---|---|---|---|
| `locate_service` | info | `sku_name`, `sku_version`, `device_token` | first contact |
| `open_session` | info | `session_id`, `massive_player_id`, `build`, `players`, `client_time` | session start |
| `request` | info | `message_type`, `session_id`, `client_time`, `since_previous_ms`, `zone` | cadence of periodic requests per type |
| `client_time` | info | `session_id`, `ticks`, `elapsed_ms`, `ticks_per_second` | unit of the request time field |
| `enter_zone` | info | `zone`, `maos`, `orders`, `bandwidth`, `players` | zone entry and the bandwidth fields `10`, `11`, `12` |
| `asset_offered` | info | `zone`, `order_id`, `asset_id`, `url`, `bytes` | one per asset in an EnterZone response |
| `asset_request` | info | `asset_id`, `bytes`, `target`, `target_form`, `host`, `user_agent`, `range` | URL form the client uses; `range` shows a resumed download |
| `asset_sent`, `asset_send_failed` | info, warn | `target`, `status`, `body_len`, `elapsed_ms`, `error` | whether a GET response reached the client, and how long it took |
| `impression_update` | info | `records`, `value_21`, `value_29`, `names` | meaning of the impression counters |
| `statistics` | info | `cache_lookups`, `cache_hits`, `cache_mismatches` | cache look-ups, hits and mismatches |
| `player_update`, `close_session` | info | | lifecycle |
| `connection_reused` | info | `request_number` | client connection reuse |
| `mac_mismatch` | warn | `expected`, `found` | key or coverage differs |
| `key_string_differs`, `open_session_settings_differ` | warn, info | | OpenSession constants differ from this build |
| `unknown_field` | warn | `tag`, `offset`, `bytes` | block tag the schema does not know |
| `decode_failed` | warn | `error`, `bytes` | request the codec cannot decode |
| `unknown_session`, `player_id_mismatch` | warn | | session values not issued by this server |
| `unknown_zone`, `asset_not_found` | warn | `asset_not_found`: `target`, `host`, `user_agent`, `range` | content missing for a client request |
| `response_fails_client_rules` | error | `reason`, `bytes` | the server built a response the client would reject |
| `fields`, `http_request`, `http_response` | debug | | field tree, raw headers, timing |

## Capture format

One JSON object per line: `time_ms`, `connection`, `peer`, `direction`
(`in` from the client, `out` to it) and `bytes`, the HTTP message as
lowercase hex.

## Probe

`probe` sends LocateService, OpenSession, PlayerUpdate, EnterZone, the
downloads of every asset, ImpressionUpdate, Heartbeat, Statistics and
CloseSession. Each response is verified with the retail key and decoded with
the client's acceptance rules; downloads are checked for size and SHA-1. It
stops at the first step the retail client would fail.
