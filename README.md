# XMrsPlayer is a safe soundtracker music player

XMrsPlayer was a safe portage of libxm to play real music

the original [code](https://github.com/Artefact2/libxm) uses the WTFPL Version2 license. [XMrsPlayer](https://github.com/sbechet/xmrsplayer) uses the MIT License with consent from the Romain Dal Maso original author. The code was initially a simple port of libxm. It's very different today, with rustification as complete as possible and better accuracy in the effects.

Help welcome.

# Example?

```
$ cargo run --release --features demo --example rodio_player
$ cargo run --release --features demo --example rodio_player -- --help
$ cargo run --release --features demo --example cpal_player
$ cargo run --release --features demo --example cpal_player -- --help
```

