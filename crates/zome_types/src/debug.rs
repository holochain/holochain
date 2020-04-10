use holochain_serialized_bytes::prelude::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
pub struct DebugMsg {
    module_path: String,
    file: String,
    line: u32,
    msg: String,
}

impl DebugMsg {
    pub fn new(module_path: &str, file: &str, line: u32, msg: &str) -> Self {
        Self {
            module_path: module_path.to_owned(),
            file: file.to_owned(),
            line,
            msg: msg.to_owned(),
        }
    }

    pub fn msg(&self) -> &str {
        &self.msg
    }

    pub fn module_path(&self) -> &str {
        &self.module_path
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn line(&self) -> u32 {
        self.line
    }
}

#[macro_export]
macro_rules! debug_msg {
    ( $msg:expr ) => {
        debug_msg!("{}", $msg);
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        $crate::debug::DebugMsg::new(module_path!(), file!(), line!(), &format!($msg, $($tail),*))
    }};
}
