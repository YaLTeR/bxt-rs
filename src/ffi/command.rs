#![allow(unused)]

use std::os::raw::*;

#[repr(C)]
#[derive(Debug)]
pub struct cmd_function_s {
    pub next: *mut cmd_function_s,
    pub name: *const c_char,
    pub function: extern "C" fn(),
    pub flags: c_int,
}
