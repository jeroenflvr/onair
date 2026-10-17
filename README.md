# ON AIR

A macOS utility that turns an LED board on while Microsoft Teams is using the
microphone, then turns it off after the microphone has been idle for three
seconds.

## Configure

Set the LED board endpoint in `.env`:

```sh
ONAIR_URL=http://your-led-board/power
```

The Teams process names, polling interval, and off delay can be configured at
the top of `src/main.rs`.

## Run

Requires Rust and macOS:

```sh
cargo run --release
```

The bridge sends `POST` requests to the configured LED URL with a JSON body of
`{"state": true}` while Teams is using the microphone and `{"state": false}`
when it switches off.