//! Code location markers and comments.
//!
//! Use like:
//!
//! ```ignore
//! comment!("right before we start the transfer");
//! comment!("right before we start the transfer", barrier);
//! mark!("dma_kickoff", "right before we start the transfer");
//! mark!("dma_kickoff", "right before we start the transfer", barrier);
//! ```
//!
//! Each use adds one record to the non-loaded ELF section `.code_labels`:
//!
//!   u32   address (final VMA of the next instruction)
//!   [u8]  id, NUL-terminated      (empty for comment!)
//!   [u8]  comment, NUL-terminated
//!   padded to 4-byte alignment
//!
//! Without `barrier` the asm block is set to `nomem`, so the compiler may move
//! memory accesses across it. With `barrier` it is a full compiler memory
//! barrier. It's not a hardware barrier though, so no instructions are
//! emitted because of it.

#[doc(hidden)]
pub mod __marks {
    /// Ids must match `[A-Za-z_][A-Za-z0-9_.]*`
    pub const fn valid_id(s: &str) -> bool {
        let b = s.as_bytes();
        if b.is_empty() {
            return false;
        }
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let ok = c.is_ascii_alphabetic()
                || c == b'_'
                || (i > 0 && (c.is_ascii_digit() || c == b'.'));
            if !ok {
                return false;
            }
            i += 1;
        }
        true
    }

    /// Comments are spliced into a `.asciz "..."` directive, so we need to reject
    /// anything that would break or be reinterpreted by the assembler.
    pub const fn valid_comment(s: &str) -> bool {
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'"' | b'\\' | b'\n' | b'\r' | 0 => return false,
                _ => {},
            }
            i += 1;
        }
        true
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __mark_emit {
    ($id:literal, $comment:literal, [$($opt:ident)?]) => {{
        const {
            assert!(
                $crate::marks::__marks::valid_comment($comment),
                "comments must not contain `\"`, `\\`, newlines or NUL"
            );
        }
        unsafe {
            ::core::arch::asm!(
                "1:",
                ".pushsection .code_labels, \"\", %progbits",
                ".balign 4",
                ".word 1b", // becomes an R_ARM_ABS32 relocation against the text section
                concat!(".asciz \"", $id, "\""),
                concat!(".asciz \"", $comment, "\""),
                ".popsection",
                options($($opt,)? nostack, preserves_flags),
            )
        }
    }};
}

/// `mark!("id", "comment")` or `mark!("id", "comment", barrier)`
#[macro_export]
macro_rules! mark {
    ($id:literal, $comment:literal, barrier $(,)?) => {{
        const {
            assert!(
                $crate::marks::__marks::valid_id($id),
                "mark! id must match [A-Za-z_][A-Za-z0-9_.]*"
            );
        }
        $crate::__mark_emit!($id, $comment, [])
    }};
    ($id:literal, $comment:literal $(,)?) => {{
        const {
            assert!(
                $crate::marks::__marks::valid_id($id),
                "mark! id must match [A-Za-z_][A-Za-z0-9_.]*"
            );
        }
        $crate::__mark_emit!($id, $comment, [nomem])
    }};
}

/// `comment!("comment")` or `comment!("comment", barrier)`
#[macro_export]
macro_rules! comment {
    ($comment:literal, barrier $(,)?) => {
        $crate::__mark_emit!("", $comment, [])
    };
    ($comment:literal $(,)?) => {
        $crate::__mark_emit!("", $comment, [nomem])
    };
}

