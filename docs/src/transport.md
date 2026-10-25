# Transport

## HTTP request template [C]

Built by `sub_82506308` (`massive_BuildHTTPRequest`) using the format string at `0x82084BE0`:

```text
POST <path> HTTP/1.1\r\n
Host:<host>:<port>\r\n
Content-Length:<n>\r\n
Content-Type:application/massive\r\n
User-Agent:Adclient Massive Inc./XDK Test QFE 1v1009328.3\r\n
\r\n
<body bytes, n of them>
```

- `Host` is built as `"%s:%d"` from the server name and port (`sub_824B0720`, string at `0x82084C40`).
- The User-Agent is `Adclient Massive Inc./` followed by the build string `XDK Test QFE 1v1009328.3` (string at `0x82081B0C`).
- `POST` is used for every request except the asset download, which uses `GET` (selected when the request type is `0x54`).
- The path comes from the request object's `+0x08` virtual function ([Requests](requests.md)).

## Socket layer [C]

`CMassiveSocket` (constructor `0x82506DD0`) owns the connection. The strings `Transfer-Encoding: chunked` (`0x82084C64`) and `Content-Length:` (`0x82084C48`) show the response reader handles both fixed-length and chunked bodies. The response status line is parsed with `%256s %d` and `%256s %d %256[ a-zA-Z0-9]` (`0x82084C58`, `0x82084C80`).

## Transport security [I]

No TLS code was found in the transport path. The request is built as plain text and written to a socket. The transport therefore appears to be plaintext HTTP. This has not been confirmed on the wire.

## Retry and transaction handling [U]

`TransactionHTTP` (`0x825061F0`) is the transaction object. Its timeout and retry constants were not identified.

## Response reading

Response bodies are parsed by the response parsers described in [Responses](responses.md). The transport delivers the body as a byte buffer that the `CRequestObject` owns. The buffer is then parsed in place by removing consumed bytes from the front and the trailer.
