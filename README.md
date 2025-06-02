# fakie – the flaky web proxy

An HTTP proxy for testing web apps under flaky network conditions. Can be configured to drop:

- the first _n_ attempts to send each request;
- the first _n_ attempted responses from each request; and/or
- a percentage of requests and/or responses at random.

No automated tests were used, but also no vibes. Use at your own risk.

A [moymoy.dev](https://moymoy.dev) project.

## Usage

Ensure you have [Rust](https://www.rust-lang.org) installed, then install fakie using cargo:

```shell
cargo install fakie
```

Run `fakie --help` for CLI usage.

## TODO

- [x] drop first request attempt
- [x] configurable listen address
- [x] logging/stdout
- [x] counter reset
- [x] read hostname from CLI
- [x] read IP address from CLI
- [x] read TLS/no TLS from CLI
- [x] configurable drop rate
- [x] chaos mode (random drops)
- [ ] find next available port when socket in use
- [ ] automated tests
- [ ] CI/CD
- [ ] windows
- [ ] execute sub-process and detect open port?
- [ ] time-based filter?
- [ ] path re-writes
