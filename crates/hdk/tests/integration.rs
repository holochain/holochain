// use hdk::prelude::*;
// #[hdk_entry_helper]
// pub struct Content(pub String);

// #[hdk_entry_helper]
// pub struct Post(pub String);

// #[hdk_derive::hdk_entry_defs]
// enum EntryTypes {
//     #[entry_def(name = "bar")]
//     Msg(Content),
//     Post(Post),
// }

// #[hdk_derive::hdk_entry_zomes]
// enum EntryZomes {
//     IntegrityZome(EntryTypes),
//     AnotherIntegrityZome(EntryTypes),
// }

// fn zome_needs_traits<T, E>(t: T) -> Result<(), WasmError>
// where
//     T: ToAppEntryDefName,
//     T: ToZomeName,
//     Entry: TryFrom<T, Error = E>,
//     WasmError: From<E>,
// {
//     let _n = t.entry_def_name();
//     let _n = t.zome_name();
//     let _e: Entry = t.try_into()?;
//     Ok(())
// }

// #[test]
// fn test_entry_zome_traits() {
//     let _ = zome_needs_traits(EntryZomes::IntegrityZome(EntryTypes::Msg(Content(
//         "foo".to_string(),
//     ))));
//     let _ = zome_needs_traits(&EntryZomes::IntegrityZome(EntryTypes::Msg(Content(
//         "foo".to_string(),
//     ))));
//     let _ = zome_needs_traits(EntryZomes::IntegrityZome(EntryTypes::Post(Post(
//         "foo".to_string(),
//     ))));
//     let _ = zome_needs_traits(&EntryZomes::IntegrityZome(EntryTypes::Post(Post(
//         "foo".to_string(),
//     ))));
//     let _ = zome_needs_traits(EntryZomes::AnotherIntegrityZome(EntryTypes::Msg(Content(
//         "foo".to_string(),
//     ))));
// }

// #[test]
// fn test_entry_zomes_names() {
//     assert_eq!(
//         EntryZomes::IntegrityZome(EntryTypes::Msg(Content("foo".to_string(),))).zome_name(),
//         ZomeName::new("integrity_zome")
//     );
//     assert_eq!(
//         EntryZomes::AnotherIntegrityZome(EntryTypes::Msg(Content("foo".to_string(),))).zome_name(),
//         ZomeName::new("another_integrity_zome")
//     );
// }
