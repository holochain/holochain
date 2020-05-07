use holochain_types::header::AppEntryType;

pub struct ValidationPackageInvocation<'a> {
    zome_name: &'a str,
    app_entry_type: &'a AppEntryType,
}
