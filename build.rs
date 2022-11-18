extern crate gl_generator;

use std::env;
use std::fs::File;
use std::path::Path;

use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};

fn main() {
    let dest = env::var("OUT_DIR").unwrap();
    let mut file = File::create(Path::new(&dest).join("bindings.rs")).unwrap();

    Registry::new(
        Api::Gl,
        (4, 6), // Extensions below were introduced in OpenGL 4.6.
        Profile::Compatibility,
        Fallbacks::All,
        [
            "GL_EXT_memory_object",
            "GL_EXT_memory_object_fd",
            "GL_EXT_memory_object_win32",
            "GL_EXT_semaphore",
            "GL_EXT_semaphore_fd",
            "GL_EXT_semaphore_win32",
        ],
    )
    .write_bindings(StructGenerator, &mut file)
    .unwrap();

    // Don't re-run the build script on bxt-rs changes.
    println!("cargo:rerun-if-changed=build.rs");
}
