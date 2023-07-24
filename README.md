# bxt-rs

Video recording, speedrunning and TAS tools for Half-Life and mods. Current features include:

- Video recording ([video tutorial](https://youtu.be/ZMjhCXA82tU)).
- Interactive editor for TASes—remade and improved version of the BXT TAS editor ([video tutorial](https://youtu.be/zi3pw9iS1sk)).
- Brute-force optimizer for TASes ([video tutorial](https://youtu.be/ECuRruY3XLw)).
- `bxt_hud_scale` to upscale the HUD for high-resolution video recording.
- Commands to play many demos at once (`bxt_play_run`).
- `bxt_force_fov` to override FOV when `default_fov` doesn't work.
- Fixes for command buffer overflow on BXT demos and un-stuck non-determinism for TASes.
- A useful subset of `bxt_tas_log`, including RNG state dumping, when you can't use the one from the original Bunnymod XT.
- `bxt_fade_remove`, `bxt_shake_remove`, `bxt_skybox_remove`, `bxt_novis`, `bxt_wallhack`, `bxt_disable_loading_text`.
- Mostly-working recording real-time gameplay into a `.hltas` script.

For a full list of features, console commands and variables visit the [wiki page](https://github.com/YaLTeR/bxt-rs/wiki/Features).

Started as an experiment to re-architecture Bunnymod XT from scratch with the benefit of hindsight.

## VAC Ban Warning

Do not connect to VAC-secured servers with bxt-rs or you might get VAC banned.

## Usage

You can download the latest development build from [GitHub Actions](https://github.com/YaLTeR/bxt-rs/actions?query=branch%3Amaster). Open the topmost workflow and scroll down for Artifacts.

On Linux, you can use the [`runhl.sh`](runhl.sh) script. Correct the paths at the top of the script if necessary.

On Windows:

1. Download [Bunnymod XT Injector](https://github.com/YaLTeR/BunnymodXT-Injector/releases) 3.2 or newer.
1. Create a new folder with `Injector.exe` and `bxt_rs.dll`.
1. Start Half-Life with the injector: `Injector.exe path\to\Half-Life\hl.exe`. Running HL on its own and running the injector afterwards is not supported.

To run bxt-rs together with the original Bunnymod XT (e.g. for capturing TASes) simply put `BunnymodXT.dll`, `Injector.exe` and `bxt_rs.dll` together into the same folder and start Half-Life with the injector.

### Video Recording

To use video recording you need FFmpeg. On Linux, install it from your package manager. On Windows, download a static FFmpeg build (e.g. [this one](https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full.7z)) and extract `ffmpeg.exe` into the Half-Life folder (the folder that has `hl.exe`).

If the video looks glitched or the game crashes when starting recording (happens on some Windows AMD GPU setups and on some NVIDIA driver versions):

1. Update your GPU driver.
1. If the problem still occurs, try `_bxt_cap_force_fallback 1`.

### Profiling

Start Half-Life with the `BXT_RS_PROFILE` environment variable set to make bxt-rs output a `trace.json` file in the Half-Life folder. This is a Chrome JSON trace file which you can view in [Perfetto](https://ui.perfetto.dev/) or in `chrome://tracing`.

### Debugging

There are environment variables that may assist you in debugging:

- `BXT_RS_VULKAN_DEBUG` will enable the Vulkan validation layers. You need to have 32-bit validation layers installed, otherwise Vulkan will fail to initialize.
- `BXT_RS_VERBOSE` will log more verbose and spammy messages.

## Building

You can uncomment the right line in `.cargo/config` to avoid writing `--target` every time.

### Linux

1. Install 32-bit libc for linking. On Ubuntu that's `libc6-dev-i386`, on Fedora you'll need `glibc-devel.i686`.
1. Install stable Rust (for example, via [rustup](https://rustup.rs/)) with the `i686-unknown-linux-gnu` target.
1. `cargo build --target=i686-unknown-linux-gnu`

You may need to set `CC=cc` environment variable on Fedora and some other distros, otherwise you'll get build errors for SQLite. You can uncomment the corresponding line in `.cargo/config` to set it automatically if you need this.

### Windows

1. Install stable Rust (for example, via [rustup](https://rustup.rs/)) with the `i686-pc-windows-msvc` target.
1. `cargo build --target=i686-pc-windows-msvc`

### Windows (cross-compiling from Linux)

MinGW on Ubuntu currently fails to link Rust code. Debian and Fedora work fine.

1. Install MinGW for the 32-bit target. On Debian that's `gcc-mingw-w64-i686`, on Fedora you'll need `mingw32-gcc` and `mingw32-winpthreads-static`.
1. Install stable Rust (for example, via [rustup](https://rustup.rs/)) with the `i686-pc-windows-gnu` target.
1. `cargo build --target=i686-pc-windows-gnu`

## Documentation

- [Contributing: how to add patterns and new functionality](CONTRIBUTING.md)
- [Design notes](DESIGN.md)

## License

GPL v3 or later.
