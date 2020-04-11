use holochain_serialized_bytes::prelude::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
pub struct DebugMsg {
    // TODO: Consider either replacing with `Cow<'static, str>` or with
    // `&'a str` and using `#[serde(borrow)]`
    module_path: String,
    // TODO: Consider either replacing with `Cow<'static, str>` or with
    // `&'a str` and using `#[serde(borrow)]`
    file: String,
    line: u32,
    msg: String,
}

impl DebugMsg {
    pub fn new(module_path: String, file: String, line: u32, msg: String) -> Self {
        Self {
            module_path,
            file,
            line,
            msg,
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
        $crate::debug::DebugMsg::new(module_path!().to_string(), file!().to_string(), line!(), format!($msg, $($tail),*))
    }};
}
