//! Comment command buffer overflow fix.

use super::Module;
use crate::hooks::engine;
use crate::utils::*;

pub struct CommentOverflowFix;
impl Module for CommentOverflowFix {
    fn name(&self) -> &'static str {
        "Comment command buffer overflow fix"
    }

    fn description(&self) -> &'static str {
        "\
Bunnymod XT spams demos with data stored in console command comments. They overflow the command \
buffer upon playback leading to console spam and commands being skipped. In particular, the \
command to play the next demo in `bxt_play_run` can get skipped, which means the demo playback \
interrupts mid-way.

This module strips prefix comments from console commands as they are added to the command buffer, \
preventing the overflow."
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::Cbuf_AddText.is_set(marker)
            || engine::Cbuf_AddFilteredText.is_set(marker)
            || engine::Cbuf_AddTextToBuffer.is_set(marker)
    }
}

pub unsafe fn strip_prefix_comments(text: *const i8) -> *const i8 {
    if text.is_null() {
        return text;
    }

    let mut text: *const u8 = text.cast();

    // If the command starts with //, skip until the end of the comment.
    if *text == b'/' && *(text.offset(1)) == b'/' {
        // Skip past the //.
        text = text.offset(2);

        let mut quote = false;
        while *text != 0 {
            match *text {
                b'"' => quote = !quote,
                // ; and \n terminate the command, but only outside of quotes. Make sure to leave
                // the terminating character.
                b';' | b'\n' if !quote => break,
                _ => (),
            }

            text = text.offset(1);
        }
    }

    text.cast()
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::*;

    #[test]
    fn test_strip_prefix_comments() {
        unsafe {
            let check = |a: &[u8], b: &[u8]| {
                let a = strip_prefix_comments(a.as_ptr().cast());
                assert_eq!(CStr::from_ptr(a), CStr::from_ptr(b.as_ptr().cast()));
            };

            check(b"// blah;echo 3\0", b";echo 3\0");
            check(b"// blah\necho 4\0", b"\necho 4\0");
            check(b"// blah\";\"echo 5\0", b"\0");
            check(b"// blah\";\"echo 6;echo 7\0", b";echo 7\0");
            check(
                b"//blah\";\"echo 6;//echo 7;echo 8\0",
                b";//echo 7;echo 8\0",
            );
            check(b"\0", b"\0");
            check(b"\n\0", b"\n\0");
            check(b"echo hi\0", b"echo hi\0");
            check(b"   echo hi\0", b"   echo hi\0");
        }
    }
}
