# XMrsPlayer is a safe soundtracker music player

XMrsPlayer is a library to play real music

The code was initially a simple port of libxm. It's very different today, with rustification as complete as possible and better accuracy in the effects.

**Amiga Module** and **XM** player.

Help welcome.

# Example?

```
$ cargo run --release --features demo --example rodio_player
$ cargo run --release --features demo --example rodio_player -- --help
$ cargo run --release --features demo --example cpal_player
$ cargo run --release --features demo --example cpal_player -- --help
```

