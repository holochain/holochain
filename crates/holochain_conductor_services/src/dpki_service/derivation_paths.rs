pub type DerivationPath = Box<[u32]>;
pub type LairTag = String;

pub fn derivation_path_for_dpki_instance(index: u32, tag: &LairTag) -> (DerivationPath, LairTag) {
    let path = [index].into();
    let tag = format!("{tag}.{index}");
    (path, tag)
}
