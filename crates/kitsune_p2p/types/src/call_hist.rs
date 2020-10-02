//! Quick implementation of something that can be passed around to track async call stack.
//! This is designed to be compiled out of existence for release builds.

// tests at top so line numbers change less : )
#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn call_hist() {
        let h = call_hist!("apple");
        let h = call_hist!(h, "banana");
        let expect1 = "apple:crates/kitsune_p2p/types/src/call_hist.rs:11";
        let expect2 = "banana:crates/kitsune_p2p/types/src/call_hist.rs:12";
        assert_eq!(&format!("[{}][{}]", expect2, expect1), &h.to_string());
        assert_eq!(&format!("[\"{}\", \"{}\"]", expect2, expect1), &format!("{:?}", h));
        assert_eq!(&format!("[\n    \"{}\",\n    \"{}\",\n]", expect2, expect1), &format!("{:#?}", h));
    }
}

#[cfg(debug_assertions)]
mod if_debug {
    const EMPTY: &str = "";
    const STACK_LEN: usize = 16;

    /// Item tracking call history potentially across async / through channels.
    #[derive(Clone, Copy)]
    pub struct CallHist(
        [&'static str; STACK_LEN],
    );

    impl CallHist {
        /// Create a new CallHist instance.
        pub fn new() -> Self {
            Self([EMPTY; STACK_LEN])
        }

        /// Push a new history item.
        #[must_use = "You probably meant to `let h = h.push(\"\")`"]
        pub fn push(&self, item: &'static str) -> Self {
            let mut out: Self = *self;
            out.0.copy_within(0..(STACK_LEN - 1), 1);
            out.0[0] = item;
            out
        }
    }

    impl std::fmt::Debug for CallHist {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut list = f.debug_list();
            for i in &self.0 {
                if i.is_empty() {
                    break;
                }
                list.entry(i);
            }
            if !self.0[STACK_LEN - 1].is_empty() {
                list.entry(&"...");
            }
            list.finish()
        }
    }

    impl std::fmt::Display for CallHist {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for item in &(self.0) {
                if item.is_empty() {
                    break;
                }
                f.write_str("[")?;
                f.write_str(item)?;
                f.write_str("]")?;
            }
            if !self.0[STACK_LEN - 1].is_empty() {
                f.write_str("[...]")?;
            }
            Ok(())
        }
    }

    /// Push a history item onto a CallHist instance, returns the new CallHist.
    /// # Example
    /// ```
    /// # use kitsune_p2p_types::*;
    /// let h = CallHist::new();
    /// let h = call_hist!(h, "test");
    /// ```
    #[macro_export]
    macro_rules! call_hist {
        ($text:literal) => {{
            let h = $crate::CallHist::new();
            $crate::call_hist!(h, $text)
        }};
        ($h:ident, $text:literal) => {{
            const _HIST: &str = concat!($text, ":", file!(), ":", line!());
            $h.push(_HIST)
        }};
    }
}

#[cfg(not(debug_assertions))]
mod if_not_debug {
    /// Stub item for release builds.
    #[derive(Debug, Clone, Copy)]
    pub struct CallHist;

    impl CallHist {
        /// Create a new CallHist instance.
        pub fn new() -> Self {
            Self
        }

        /// Push a new history item.
        #[must_use = "You probably meant to `let h = h.push(\"\")`"]
        pub fn push(&self, _item: &'static str) -> Self {
            Self
        }
    }

    impl std::fmt::Display for CallHist {
        fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            Ok(())
        }
    }

    /// Stub macro is a no-op.
    #[macro_export]
    macro_rules! call_hist {
        ($text:literal) => {{
            $crate::CallHist::new()
        }};
        ($h:ident, $text:literal) => {
            $h
        };
    }
}

#[cfg(debug_assertions)]
pub use if_debug::*;

#[cfg(not(debug_assertions))]
pub use if_not_debug::*;

impl Default for CallHist {
    fn default() -> Self {
        Self::new()
    }
}
