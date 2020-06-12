use holochain_zome_types::entry_def::EntryDefId;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::globals::ZomeGlobals;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;

holochain_wasmer_guest::holochain_externs!();

const POST_ID: &str = "post";
const POST_VALIDATIONS: u8 = 8;
struct Post;

impl From<&Post> for EntryDefId {
    fn from(_: &Post) -> Self {
        POST_ID.into()
    }
}

impl From<&Post> for EntryVisibility {
    fn from(_: &Post) -> Self {
        Self::Public
    }
}

impl From<&Post> for CrdtType {
    fn from(_: &Post) -> Self {
        Self
    }
}

impl From<&Post> for RequiredValidations {
    fn from(_: &Post) -> Self {
        POST_VALIDATIONS.into()
    }
}

impl From<&Post> for EntryDef {
    fn from(post: &Post) -> Self {
        Self {
            id: post.into(),
            visibility: post.into(),
            crdt_type: post.into(),
            required_validations: post.into(),
        }
    }
}

const COMMENT_ID: &str = "comment";
const COMMENT_VALIDATIONS: u8 = 3;
struct Comment;

impl From<&Comment> for EntryDefId {
    fn from(_: &Comment) -> Self {
        COMMENT_ID.into()
    }
}

impl From<&Comment> for EntryVisibility {
    fn from(_: &Comment) -> Self {
        Self::Private
    }
}

impl From<&Comment> for CrdtType {
    fn from(_: &Comment) -> Self {
        Self
    }
}

impl From<&Comment> for RequiredValidations {
    fn from(_: &Comment) -> Self {
        COMMENT_VALIDATIONS.into()
    }
}

impl From<&Comment> for EntryDef {
    fn from(comment: &Comment) -> Self {
        Self {
            id: comment.into(),
            visibility: comment.into(),
            crdt_type: comment.into(),
            required_validations: comment.into(),
        }
    }
}

#[no_mangle]
pub extern "C" fn entry_defs(_: RemotePtr) -> RemotePtr {
    let globals: ZomeGlobals = try_result!(host_call!(__globals, ()), "failed to get globals");

    let defs: EntryDefs = vec![
        (&Post).into(),
        (&Comment).into(),
    ].into();

    ret!(GuestOutput::new(try_result!(EntryDefsCallbackResult::Defs(
        globals.zome_name,
        defs,
    ).try_into(), "failed to serialize entry defs return value")));
}
