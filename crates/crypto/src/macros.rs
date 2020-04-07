/// make invoking ffi functions more readable
macro_rules! raw_ptr_void {
    ($name: ident) => {
        $name.as_mut_ptr() as *mut libc::c_void
    };
}

/// make invoking ffi functions more readable
macro_rules! raw_ptr_char {
    ($name: ident) => {
        $name.as_mut_ptr() as *mut libc::c_uchar
    };
}

/// make invoking ffi functions more readable
macro_rules! raw_ptr_char_immut {
    ($name: ident) => {
        $name.as_ptr() as *const libc::c_uchar
    };
}

/*
/// make invoking ffi functions more readable
macro_rules! raw_ptr_ichar_immut {
    ($name: ident) => {
        $name.as_ptr() as *const libc::c_char
    };
}
*/
