# bxt-rs

A few speedrunning and TAS-related tools for Half-Life and mods. Current features:
- Video recording (a successor to [hl-capture](https://github.com/YaLTeR/hl-capture)).
- Commands to play many demos at once (`bxt_play_run`).
- A useful subset of `bxt_taslog` when you can't use the one from the original Bunnymod XT.
- `bxt_fade_remove`.

Started as an experiment to re-architecture Bunnymod XT from scratch with the benefit of hindsight.

## Usage

On Linux, load it into HL like you would load regular BXT.

On Windows, rename `bxt-rs.dll` to `BunnymodXT.dll` and start HL using the injector (`Injector.exe path\to\Half-Life\hl.exe`). Running HL on its own and running the injector afterwards is not supported.

To run the bxt-compatibility build together with the original Bunnymod XT (e.g. for capturing TASes):
1. Create a new folder, copy the original `BunnymodXT.dll` and `Injector.exe` there.
1. Load Half-Life with bxt-rs as described above.
1. While in the main menu, run the injector in the new folder to inject the original Bunnymod XT.

## Building

### Linux

1. Install 32-bit libc for linking. On Ubuntu that's `libc6-dev-i386`.
1. Install stable Rust (for example, via [rustup](https://rustup.rs/)) with the `i686-unknown-linux-gnu` target.
1. `cargo build --target=i686-unknown-linux-gnu`

### Windows

1. Install stable Rust (for example, via [rustup](https://rustup.rs/)) with the `i686-pc-windows-msvc` target.
1. `cargo build --target=i686-pc-windows-msvc`

### Windows (cross-compiling from Linux)

1. Install MinGW for the 32-bit target. On Ubuntu that's `gcc-mingw-w64-i686`.
1. Install stable Rust (for example, via [rustup](https://rustup.rs/)) with the `i686-pc-windows-gnu` target.
1. `cargo build --target=i686-pc-windows-gnu`

## Documentation

- [Contributing: how to add patterns and new functionality](CONTRIBUTING.md)
- [Design notes](design.md)

## License

Currently GPL v3 or later.
