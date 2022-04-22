// use std::borrow::Cow;

// use hdk_derive::EntryDefRegistration;
// use hdk_derive::ToAppEntryDefName;
// use hdk_derive::ToZomeName;
// use holochain_deterministic_integrity::prelude::*;

// #[derive(Serialize, Debug, Deserialize, SerializedBytes)]
// pub struct Content(pub String);

// #[derive(Serialize, Debug, Deserialize, SerializedBytes)]
// pub struct Post(pub String);

// #[derive(Serialize, Debug, Deserialize, SerializedBytes)]
// pub struct Content2(pub String);

// #[derive(Serialize, Debug, Deserialize, SerializedBytes)]
// pub struct Post2(pub String);

// #[derive(ToAppEntryDefName)]
// pub enum EntryTypes {
//     Msg(Content),
//     UnitMsg,
//     StructMsg {
//         content: Content,
//     },
//     #[entry_def_name(name = "my_post")]
//     Post(Post),
// }

// #[derive(EntryDefRegistration)]
// pub enum EntryTypes2 {
//     Msg(Content),
//     #[entry_def(visibility = "private", required_validations = 1)]
//     UnitMsg,
//     #[entry_def(name = "some_content", visibility = "private")]
//     StructMsg {
//         content: Content,
//     },
//     Post(Post),
// }

// #[hdk_derive::entry_defs_name_registration]
// pub enum EntryTypes3 {
//     #[entry_def(name = "bar")]
//     Msg(Content),
//     #[entry_def(visibility = "private", required_validations = 100)]
//     UnitMsg(Content),
//     #[entry_def(name = "foo", visibility = "private")]
//     StructMsg(Content),
//     Post(Post),
// }

// #[derive(ToZomeName)]
// enum Zomes {
//     IntegrityZome,
//     AnotherIntegrityZome,
// }

// #[zome_names]
// enum Zomes2 {
//     IntegrityZome,
//     AnotherIntegrityZome,
// }
// mod et4 {
//     use super::*;

//     #[hdk_derive::hdk_entry_defs]
//     pub enum EntryTypes4 {
//         #[entry_def(name = "bar")]
//         Msg(Content),
//         Post(Post),
//         #[entry_def(visibility = "private")]
//         Msg2(Content),
//     }
// }
// use et4::EntryTypes4;

// mod et5 {
//     use super::*;

//     #[hdk_derive::hdk_entry_defs]
//     pub enum EntryTypes5 {
//         Msg(Content2),
//         Post(Post2),
//     }
// }
// use et5::EntryTypes5;

// enum LinkTypes {
//     MsgLink,
//     PostLink,
// }

// #[test]
// fn can_get_entry_def_name() {
//     assert_eq!(
//         AppEntryDefName(Cow::Borrowed("hey")),
//         AppEntryDefName(Cow::Owned("hey".to_string()))
//     );
//     assert_eq!(
//         AppEntryDefName(Cow::Borrowed("hey")),
//         AppEntryDefName::new("hey")
//     );

//     assert_eq!(
//         EntryTypes::Msg(Content("foo".to_string())).entry_def_name(),
//         AppEntryDefName::new("msg")
//     );
//     assert_eq!(
//         EntryTypes::UnitMsg.entry_def_name(),
//         AppEntryDefName::new("unit_msg")
//     );
//     assert_eq!(
//         EntryTypes::StructMsg {
//             content: Content("foo".to_string())
//         }
//         .entry_def_name(),
//         AppEntryDefName::new("struct_msg")
//     );
//     assert_eq!(
//         EntryTypes::Post(Post("foo".to_string())).entry_def_name(),
//         AppEntryDefName::new("my_post")
//     );
// }

// #[test]
// fn can_get_entry_def_registration() {
//     assert_eq!(
//         EntryTypes2::ENTRY_DEFS,
//         &[
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("msg"),
//                 visibility: EntryVisibility::default(),
//                 required_validations: RequiredValidations::default(),
//             },
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("unit_msg"),
//                 visibility: EntryVisibility::Private,
//                 required_validations: RequiredValidations(1)
//             },
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("some_content"),
//                 visibility: EntryVisibility::Private,
//                 required_validations: RequiredValidations::default(),
//             },
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("post"),
//                 visibility: EntryVisibility::default(),
//                 required_validations: RequiredValidations::default(),
//             },
//         ]
//     );
// }

// #[test]
// fn test_entry_def_full_defs() {
//     assert_eq!(
//         EntryTypes3::ENTRY_DEFS,
//         &[
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("bar"),
//                 visibility: EntryVisibility::default(),
//                 required_validations: RequiredValidations::default(),
//             },
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("unit_msg"),
//                 visibility: EntryVisibility::Private,
//                 required_validations: RequiredValidations(100)
//             },
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("foo"),
//                 visibility: EntryVisibility::Private,
//                 required_validations: RequiredValidations::default(),
//             },
//             AppEntryDef {
//                 name: AppEntryDefName::from_str("post"),
//                 visibility: EntryVisibility::default(),
//                 required_validations: RequiredValidations::default(),
//             },
//         ]
//     );
// }

// #[test]
// fn test_entry_def_full_names() {
//     assert_eq!(
//         EntryTypes3::Msg(Content("foo".to_string())).entry_def_name(),
//         AppEntryDefName::new("bar")
//     );
//     assert_eq!(
//         EntryTypes3::UnitMsg(Content("foo".to_string())).entry_def_name(),
//         AppEntryDefName::new("unit_msg")
//     );
//     assert_eq!(
//         EntryTypes3::StructMsg(Content("foo".to_string())).entry_def_name(),
//         AppEntryDefName::new("foo")
//     );
//     assert_eq!(
//         EntryTypes3::Post(Post("foo".to_string())).entry_def_name(),
//         AppEntryDefName::new("post")
//     );
// }

// #[test]
// fn test_entry_def_full_unit() {
//     assert_eq!(
//         EntryTypes3::Msg(Content("foo".to_string())).to_unit(),
//         UnitEntryTypes3::Msg,
//     );
//     assert_eq!(
//         EntryTypes3::UnitMsg(Content("foo".to_string())).to_unit(),
//         UnitEntryTypes3::UnitMsg,
//     );
//     assert_eq!(
//         EntryTypes3::StructMsg(Content("foo".to_string())).to_unit(),
//         <EntryTypes3 as UnitEnum>::Unit::StructMsg,
//     );
//     assert_eq!(
//         EntryTypes3::Post(Post("foo".to_string())).to_unit(),
//         <EntryTypes3 as UnitEnum>::Unit::Post,
//     );
// }

// #[test]
// fn test_entry_def_full_index() {
//     assert_eq!(EntryTypes3::Msg(Content("foo".to_string())).index(), 0);
//     assert_eq!(EntryTypes3::UnitMsg(Content("foo".to_string())).index(), 1);
//     assert_eq!(
//         EntryTypes3::StructMsg(Content("foo".to_string())).index(),
//         2
//     );
//     assert_eq!(EntryTypes3::Post(Post("foo".to_string())).index(), 3);
// }

// fn needs_traits<T, E>(t: T) -> Result<(), WasmError>
// where
//     T: ToAppEntryDefName + EntryDefRegistration,
//     Entry: TryFrom<T, Error = E>,
//     WasmError: From<E>,
// {
//     let _n = t.entry_def_name();
//     let _ = &T::ENTRY_DEFS[0];
//     let _e: Entry = t.try_into()?;
//     Ok(())
// }

// #[test]
// fn test_entry_defs_traits() {
//     let _ = needs_traits(EntryTypes4::Msg(Content("foo".to_string())));
//     let _ = needs_traits(&EntryTypes4::Msg(Content("foo".to_string())));
//     let _ = needs_traits(EntryTypes4::Post(Post("foo".to_string())));
//     let _ = needs_traits(&EntryTypes4::Post(Post("foo".to_string())));
// }

// #[test]
// fn test_entry_types_entry_defs() {
//     assert_eq!(
//         EntryTypes4::entry_def(EntryTypes4::Post),
//         EntryDef {
//             id: EntryDefId::App(AppEntryDefName::new("post")),
//             visibility: EntryVisibility::Public,
//             required_validations: RequiredValidations(5),
//             required_validation_type: Default::default(),
//         }
//     );
//     assert_eq!(
//         EntryTypes4::entry_def(EntryTypes4::Msg),
//         EntryDef {
//             id: EntryDefId::App(AppEntryDefName::new("bar")),
//             visibility: EntryVisibility::Public,
//             required_validations: Default::default(),
//             required_validation_type: Default::default(),
//         }
//     );
//     assert_eq!(
//         EntryTypes4::entry_def(EntryTypes4::Msg2),
//         EntryDef {
//             id: EntryDefId::App(AppEntryDefName::new("msg2")),
//             visibility: EntryVisibility::Private,
//             required_validations: Default::default(),
//             required_validation_type: Default::default(),
//         }
//     );
// }
