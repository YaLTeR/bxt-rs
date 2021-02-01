# Contributing to bxt-rs

## Adding support for more GoldSrc versions

[`src/hooks/engine.rs`](src/hooks/engine.rs) contains function patterns and offsets. All pointers that bxt-rs finds and uses are listed at the top of the file. Every pattern has instructions on how to find it.

### Checking what pointers some module needs

Every module has a list of pointers it needs to be enabled. To find it, do a global search for the module name from `bxt_module_list`, e.g. `Multiple demo playback`. You will find it in a file under `src/modules/`, in this case [`src/modules/demo_playback.rs`](src/modules/demo_playback.rs).

```rust
impl Module for DemoPlayback {
    fn name(&self) -> &'static str {
        "Multiple demo playback"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_PLAY_RUN];
        &COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::cls_demos.is_set(marker)
            && engine::com_gamedir.is_set(marker)
            && engine::Cbuf_InsertText.is_set(marker)
            && engine::Host_NextDemo.is_set(marker)
    }
}
```

The `is_enabled` function lists all pointers that the module needs. The next sections describe how to find them.

### Pointers without patterns

Some pointers look like this.

```rust
pub static Cbuf_InsertText: Pointer<unsafe extern "C" fn(*const c_char)> =
    Pointer::empty(b"Cbuf_InsertText\0");
```

This means they are set using some other pointer. Ctrl-F for `Cbuf_InsertText` to find how it is set.

```rust
let ptr = &Host_NextDemo;
match ptr.pattern_index(marker) {
    // 6153
    Some(0) => {
        Cbuf_InsertText.set(marker, ptr.by_relative_call(marker, 140));
        cls_demos.set(marker, ptr.by_offset(marker, 11));
    }
    _ => (),
}
```

This means that you need to find a pattern for `Host_NextDemo`. The next section shows how to do that.

### Pointers with patterns

For finding patterns I suggest [Ghidra](https://ghidra-sre.org/) and the [makesig.py](https://github.com/nosoop/ghidra_scripts/blob/master/makesig.py) script.

For example, let's say you want to add a new pattern for `CL_GameDir_f`.

```rust
pub static CL_GameDir_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_GameDir_f\0",
    // To find, search for "gamedir is ".
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 83 F8 02 74 ?? 68 ?? ?? ?? ?? 68),
    ]),
    null_mut(),
);
```

Open your engine's `hw.dll` in Ghidra and search for the string `gamedir is `.

![](https://user-images.githubusercontent.com/1794388/106464200-60ba3a00-64a9-11eb-81ce-1ad40fb2a38a.png)

If the comment doesn't say otherwise, you should get a single match, which will be used in a single function. Select the match that it found and show references to its address.

![](https://user-images.githubusercontent.com/1794388/106464332-919a6f00-64a9-11eb-984d-896a6b3c31bb.png)

Go to the function. You can rename it if you want. Put the cursor somewhere inside the function and run `makesig.py` from the script manager.

![](https://user-images.githubusercontent.com/1794388/106464766-256c3b00-64aa-11eb-8a92-28ed30daf3f6.png)

Make signature at the start of the function. If the signature has trailing `??`, don't copy them.

![](https://user-images.githubusercontent.com/1794388/106464932-62d0c880-64aa-11eb-84ad-c6788a4ee099.png)

Add the signature to the list.

```rust
pub static CL_GameDir_f: Pointer<unsafe extern "C" fn()> = Pointer::empty_patterns(
    b"CL_GameDir_f\0",
    // To find, search for "gamedir is ".
    Patterns(&[
        // 6153
        pattern!(E8 ?? ?? ?? ?? 83 F8 02 74 ?? 68 ?? ?? ?? ?? 68),
        // Some other engine
        pattern!(Signature that you copied),
    ]),
    null_mut(),
);
```

Next, Ctrl-F `CL_GameDir_f` to see if it's used for other pointers down in the file. In this case it is.

```rust
let ptr = &CL_GameDir_f;
match ptr.pattern_index(marker) {
    // 6153
    Some(0) => com_gamedir.set(marker, ptr.by_offset(marker, 11)),
    _ => (),
}
```

You probably want to update this part so these other pointers also get set.

```rust
let ptr = &CL_GameDir_f;
match ptr.pattern_index(marker) {
    // 6153
    // This 0 is the zero-based pattern index. This is the first pattern, so the index is 0.
    Some(0) => com_gamedir.set(marker, ptr.by_offset(marker, 11)),
    // Some other engine
    // The pattern we added is second, so the index is 1.
    Some(1) => com_gamedir.set(marker, ptr.by_offset(marker, offset for this pattern)),
    _ => (),
}
```

Now build bxt-rs and see if it successfully finds the function.