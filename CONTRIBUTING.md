# Contributing to bxt-rs

- [Adding support for more GoldSrc versions](#adding-support-for-more-goldsrc-versions)
  - [Checking what pointers some module needs](#checking-what-pointers-some-module-needs)
  - [Pointers without patterns](#pointers-without-patterns)
  - [Pointers with patterns](#pointers-with-patterns)
- [Modules](#modules)
  - [Adding a new module](#adding-a-new-module)
  - [Adding a console variable](#adding-a-console-variable)
  - [Adding a console command](#adding-a-console-command)
- [Hooking a new engine function](#hooking-a-new-engine-function)

## Adding support for more GoldSrc versions

[`src/hooks/engine.rs`](src/hooks/engine.rs) contains function patterns and offsets. All pointers that bxt-rs finds and uses are listed at the top of the file. Every pattern has instructions on how to find it.

### Checking what pointers some module needs

Every module has a list of pointers it needs to be enabled. To find it, do a global search for the module name from `bxt_module_list`, e.g. `Multiple demo playback`. You will find it in a file under `src/modules/`, in this case [`src/modules/demo_playback.rs`](src/modules/demo_playback.rs).

```rust
impl Module for DemoPlayback {
    fn name(&self) -> &'static str {
        "Multiple demo playback"
    }

    fn description(&self) -> &'static str {
        "Playing multiple demos at once."
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

For finding patterns I suggest [Ghidra](https://ghidra-sre.org/) and the [makesig.py](https://github.com/YaLTeR/ghidra_scripts/blob/master/makesig.py) script.

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

![](https://user-images.githubusercontent.com/1794388/108740828-23484a00-7547-11eb-9204-35a410d0336c.png)

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

## Modules

### Adding a new module

1. Create a file `src/modules/useful_functionality.rs`:

   ```rust
   //! Useful functionality.

   use super::Module;
   use crate::utils::*;
   
   pub struct UsefulFunctionality;
   impl Module for UsefulFunctionality {
       fn name(&self) -> &'static str {
           "Useful functionality"
       }

       fn description(&self) -> &'static str {
           "Doing useful things."
       }

       fn is_enabled(&self, _marker: MainThreadMarker) -> bool {
           true
       }
   }
   ```

1. Open `src/modules/mod.rs`, add the module declaration at the top:

   ```rust
   pub mod useful_functionality;
   ```

   Add the module to the array of all modules at the bottom:

   ```rust
   pub static MODULES: &[&dyn Module] = &[
       // ...
       &useful_functionality::UsefulFunctionality,
   ];
   ```

Now you can build bxt-rs and find your new module in `bxt_module_list`:

![](https://user-images.githubusercontent.com/1794388/127631714-b79c436b-422a-43b4-b93c-fe5ec6ffb411.png)

### Adding a console variable

1. Import CVar things:

   ```rust
   use crate::modules::cvars::{self, CVar};
   ```

1. Add a CVar:

   ```rust
   static BXT_ENABLE_THING: CVar = CVar::new(b"bxt_enable_thing\0", b"0\0");
   ```

   The second argument is the default value.

   Note the `\0` in the end. It is required; if you forget it for any active CVar, `cargo test` will complain.

1. Add it to the module's list of CVars:

   ```rust
   impl Module for UsefulFunctionality {
       // ...
   
       fn cvars(&self) -> &'static [&'static CVar] {
           static CVARS: &[&CVar] = &[&BXT_ENABLE_THING];
           CVARS
       }
   }
   ```

1. Add the `CVars` module to the `is_enabled()` check:

   ```rust
   fn is_enabled(&self, marker: MainThreadMarker) -> bool {
       cvars::CVars.is_enabled(marker)
   }
   ```

Now you can build bxt-rs and find your new console variable:

![](https://user-images.githubusercontent.com/1794388/127633097-cecf73bf-2d16-4d7b-be84-795382ead2f8.png)

### Adding a console command

1. Import command things:

   ```rust
   use crate::{
       handler,
       hooks::engine::con_print,
       modules::commands::{self, Command},
   }
   ```

1. Add a command:

   ```rust
   static BXT_DO_THING: Command = Command::new(
       b"bxt_do_thing\0",
       handler!(
           "Usage: bxt_do_thing\n \
             Does a thing.\n",
           do_thing as fn(_)
       ),
   );
   
   fn do_thing(marker: MainThreadMarker) {
       con_print(marker, "Thing done!\n");
   }
   ```

   Usage is printed when the number or types of arguments given to the command from the console is wrong.

   Note the `\0` in the end. It is required; if you forget it for any active CVar, `cargo test` will complain.

1. Add it to the module's list of commands:

   ```rust
   impl Module for UsefulFunctionality {
       // ...
   
       fn commands(&self) -> &'static [&'static Command] {
           static COMMANDS: &[&Command] = &[&BXT_DO_THING];
           COMMANDS
       }
   }
   ```

1. Add the `Commands` module to the `is_enabled()` check:

   ```rust
   fn is_enabled(&self, marker: MainThreadMarker) -> bool {
       commands::Commands.is_enabled(marker)
   }
   ```

Now you can build bxt-rs and find your new console command:

![](https://user-images.githubusercontent.com/1794388/127668685-f266160b-64a1-4ea0-94f1-5601d1c2900b.png)

Commands can accept a string argument or an argument of any type that can be parsed from a string. Just add the argument to the handler function and to the type cast inside the `handler! {}` macro:

```rust
static BXT_DO_THING: Command = Command::new(
    b"bxt_do_thing\0",
    handler!(
        "Usage: bxt_do_thing <N>\n \
          Does a thing N times.\n",
        do_thing as fn(_, _)
    ),
);

fn do_thing(marker: MainThreadMarker, times: usize) {
    for _ in 0..times {
        con_print(marker, "Thing done!\n");
    }
}
```

Now the command can be invoked with an argument:

![](https://user-images.githubusercontent.com/1794388/127669008-b6999242-5521-473b-872e-a6865d266438.png)

Commands can also have multiple handlers with different argument count or types:

```rust
static BXT_DO_THING: Command = Command::new(
    b"bxt_do_thing\0",
    handler!(
        "Usage: bxt_do_thing [argument]\n \
          Does a thing, maybe with an argument.\n",
        do_thing as fn(_),
        do_thing_with_argument as fn(_, _)
    ),
);

fn do_thing(marker: MainThreadMarker) {
    con_print(marker, "No argument!\n");
}

fn do_thing_with_argument(marker: MainThreadMarker, argument: String) {
    con_print(marker, &format!("Got an argument: {}\n", argument));
}
```

This command accepts no arguments or one string argument:

![](https://user-images.githubusercontent.com/1794388/127669511-558227e5-07c3-46ab-a751-7a969f4dbf7c.png)

## Hooking a new engine function

1. Find the function you want to hook in Ghidra. Refer to [Pointers with patterns](#pointers-with-patterns).
1. Open [`src/hooks/engine.rs`](src/hooks/engine.rs).
1. Add a function pointer variable alongside the ones at the top:

   ```rust
   pub static SomeFunction: Pointer<unsafe extern "C" fn(*mut c_void) -> c_int> = Pointer::empty_patterns(
       b"SomeFunction\0",
       // To find, search for this. Navigate there. The function you're looking at is SomeFunction.
       Patterns(&[
           // 1337
           pattern!(11 22 33 ?? ?? 44 55),
       ]),
       my_SomeFunction as _,
   );
   ```

   These things should match what you see in Ghidra:

   - the calling convention (`extern "C"`)
   - argument types (`*mut c_void`)
   - return type (`c_int`)

   The name (`SomeFunction`) should match the exported symbol name, it's used to get the function pointer on Linux. If there's no name or finding the pointer through it is not needed, feel free to come up with your own name which doesn't match any existing symbol.

   The function may have no patterns if it's Linux-only or if you're getting the pointer some other way.

   Note that the variables are kept in sorted order by name manually.

1. Add the new pointer to the `POINTERS` array:

   ```rust
   static POINTERS: &[&dyn PointerTrait] = &[
       // ...
       &SomeFunction,
   ];
   ```

   Note that the variables are kept in sorted order by name manually.

1. Ctrl-F `find_pointers`, in this function you can add code that sets your new pointer using some other pointer's value (if you're not using patterns, or as an alternative finding method). You can also set other pointers based on your pointer. Check other code in the function and do the same. Note there are two `find_pointers` functions, one for Linux and one for Windows.

1. Navigate down to `pub mod exported {`, there you should add the hook function:

   ```rust
   #[export_name = "SomeFunction"]
   pub unsafe extern "C" fn my_SomeFunction(some_argument: *mut c_void) -> c_int {
       abort_on_panic(move || {
           // Most GoldSrc functions are main game thread-only.
           let marker = MainThreadMarker::new();
   
           // Do something before the original function is called.
   
           let rv = SomeFunction.get(marker)(some_argument);
   
           // Do something after the original function is called.
   
           rv
       })
   }
   ```

   Once again, the calling convention, the argument types and the return type should match what you see in Ghidra. `export_name` is used on Linux and should match the raw, mangled function name, which is also visible in Ghidra. For most functions it'll look the same as the regular name, however for mangled (usually C++) functions it'll look different, for example `_Z18Sys_VID_FlipScreenv`.