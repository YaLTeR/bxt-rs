use libc::c_void;

use crate::ffi::edict::entvars_s;

pub unsafe fn get_entvars(cbaseentity: *mut c_void) -> *mut entvars_s {
    return *(cbaseentity.offset(4) as *mut *mut entvars_s);
}
