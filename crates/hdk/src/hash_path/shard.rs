// /// the number/depth of nested anchors that we use to avoid hotspots
// struct ShardFactor;
// impl Default for ShardFactor {
//     fn default() -> Self {
//         Self(0)
//     }
// }

// /// the start of the path e.g. users/...
// struct AnchorType;
// /// the end of the path e.g. .../thedavidmeister
// struct AnchorText;
//
// impl From<(AnchorType, AnchorText, ShardFactor)> for Anchor {
//  // ...
// }
//
// impl From<(AnchorType, AnchorText)> for Anchor {
//   fn from(t: (AnchorType, AnchorText)) -> Self {
//       let (type, text) = t;
//       (ShardFactor::default(), type, text).into()
//   }
// }
//
// // in wasm
// struct User(String)
// impl From<&User> for ShardFactor {
//     fn from(user: &User) -> Self {
//         Self::from(3)
//     }
// }
// impl From<&User> for AnchorType {
//     fn from(user: &User) -> Self {
//         Self::from("users")
//     }
// }
// impl From<&User> for AnchorText {
//     fn from(user: &User) -> Self {
//         Self::from(user.0)
//     }
// }
//
// let user = User::from("thedavidmeister");
// Anchor::from((ShardFactor::from(&user), AnchorType::from(&user), AnchorText::from(&user)));
// // "root/users/t/h/e/thedavidmeister" -> vec!["root".as_bytes(), "users".as_bytes(), "t".as_bytes() , ...]

// impl TryFrom<Anchor> for String {
//     type Error = core::str::Utf8Error;
//     fn try_from(anchor: Anchor) -> Result<Self, Self::Error> {
//         let string_components: Result<Vec<&str>, core::str::Utf8Error> = anchor.0.iter().map(|c| core::str::from_utf8(&c.0)).collect();
//         Ok(string_components?.join(DELIMITER).to_string())
//     }
// }
//
// impl TryFrom<&Anchor> for Entry {
//     type Error = SerializedBytesError;
//     fn try_from(anchor: &Anchor) -> Result<Self, Self::Error> {
//         Ok(Self::App(anchor.try_into()?))
//     }
// }
//
// impl TryFrom<&Anchor> for EntryHashInput {
//     type Error = SerializedBytesError;
//     fn try_from(anchor: &Anchor) -> Result<Self, Self::Error> {
//         Ok(Self(anchor.try_into()?))
//     }
// }
