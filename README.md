SIP server or testing tool written in Rust.

### Features:
* Passwordless registration
* Subscriptions (200 OK and NOTIFY without content)
* Regular calls (B2BUA and Proxy):
    * INVITE (call, hold, resume)
    * CANCEL
    * BYE
    * REFER (transfer)

### Usage:
```
cargo run <ip> <port>
```

### Limitations
It uses [libsip] that isn't yet RFC3261-compliant. Open issues in [libsip] if the server shows any parsing errors.

It accepts connections via TCP and UDP.

Suggestions are really appreciated.

[libsip]: https://github.com/ByteHeathen/libsip

### Documentation
[Message flow](message_flow.md)