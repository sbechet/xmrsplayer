# XMrsPlayer is a safe no_std soundtracker music player

XMrsPlayer is a library to play real music

The code was initially a simple port of libxm. It's very different today, with rustification as complete as possible and better accuracy in the effects.

**Amiga Module**, **S3M** and **XM** player.

S3M is a work in progress: help welcome.

## About no_std

micromath is used by default in no_std. If you prefer libm, use `cargo build --no-default-features --features=libm --release`.

## About std

if you want to use std feature use `cargo build --no-default-features --features=std --release`

# Example?

```
$ cargo run --no-default-features --features=std,demo --release --example rodio_player -- --help
$ cargo run --no-default-features --features=std,demo --release --example cpal_player -- --help
```
