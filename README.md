# bxt-rs

An experiment to re-architecture Bunnymod XT from scratch with the benefit of hindsight. I may or may not replace part of BXT with this if I like it enough.

## Aims

The main goal is to make things simpler and less convoluted than in the original BXT and in general keep things simple where reasonable. The second important goal is correctness and safety, enforced by Rust's type system and appropriate runtime checks.

Another area I want to explore is better handling of disabling functionality when some patterns could not be found.

The original BXT design failed at making this feasible so I ended up ignoring some failures in pattern finding, and mass-disabling functionality by zeroing out the `Cbuf_Execute` pointer for other failures.

There are different approaches here; I'll try to choose one which makes tackling the problem feasible while being sufficiently simple.

## Design Notes

The huge `HwDLL` and other classes—collections of hooked functions and pointers—are replaced with regular functions and global variables in separate files. This removes a few layers of indirection.

The actual BXT functionality is put into small modules. The modules can have console variables and commands. Note that there's no module hooking system; modules export plain functions which are called directly from hooks—the same as in BXT if functionality was split into separate functions instead of intertwining all code in the hooks.

Modules have an `is_enabled()` function which can check if the required pointers were found or if other required modules were enabled. For example, the `bxt_fade_remove` module checks for the `V_FadeAlpha` engine function and for the `CVars` module.

All console commands and variables are initially registered in the engine, and then commands and variables for disabled modules are de-registered. For example, `bxt_timer_autostop` will be available in the main menu (making it settable from `userconfig.cfg`) and then upon loading a map for the first time it will either remain or disappear if the required pointers are missing.

All global variables are accessed through a `MainThreadMarker` which is a dummy type representing "being on the main game thread". It's created manually in the hooked functions (known to always be called from the main thread) and passed down. This is used instead of `Mutex`es and such because in GoldSource pretty much everything happens on a single thread so I don't want to pay the `Mutex` cost (besides, a main thread marker system would still be needed for safe calls to game functions).

Found function and variable pointers are stored in global `Cell`s which means it's not possible to misuse references to them. For larger or non-`Copy` global state, main-thread-accessible `RefCell` will ensure that references are not misused (no multiple exclusive references to the global state at once).

The `Pointer` struct represents a function or variable pointer to be found by name, by pattern or by offset from another pointer. All relevant information (symbol name, patterns, hook function) is stored directly in the `Pointer` and initialized all at once. `Pointer` also stores the index of the pattern which the pointer was found with, if any, and provides helper methods to derive other pointers by offset or by relative call instruction (from function pointers).

The `CVars` module provides a safe zero-cost console variable abstraction. Only variables stored as globals can be registered in the engine to ensure their address doesn't change (as the engine stores a pointer to each registered variable). This allows to skip any allocations that would otherwise be necessary to keep the address stable.

The `Commands` module provides safe console command helpers. A console command handler function can be wrapped with a `handler!` macro to have console command arguments automatically parsed and passed into the function with a usage string printed on argument count mismatch or parsing failure.

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

## Usage

On Linux, load it into HL like you would load regular BXT.

On Windows, rename `bxt-rs.dll` to `BunnymodXT.dll` and start HL using the injector (`Injector.exe path\to\Half-Life\hl.exe`). Running HL on its own and running the injector afterwards is not supported.

## License

Currently GPL v3 or later, but I might change this to MIT if I end up integrating it into BXT.
