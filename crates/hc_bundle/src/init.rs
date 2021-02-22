use std::io::Write;
use std::{io, path::PathBuf};

use holochain_types::prelude::{
    AppBundle, AppManifest, AppManifestCurrentBuilder, AppSlotManifest, DnaBundle, DnaManifest,
};

fn readline(prompt: Option<&str>) -> io::Result<Option<String>> {
    let mut input = String::new();
    if let Some(prompt) = prompt {
        print!("{} ", prompt);
        io::stdout().flush()?;
    }
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    Ok(if input == "" {
        None
    } else {
        Some(input.to_owned())
    })
}

fn prompt_default<S: Into<String>>(prompt: &str, default: S) -> io::Result<String> {
    let default = default.into();
    let prompt = format!("{} ({})", prompt, default);
    Ok(readline(Some(&prompt))?.unwrap_or(default))
}

fn prompt_optional(prompt: &str) -> io::Result<Option<String>> {
    Ok(readline(Some(prompt))?)
}

fn prompt_required(prompt: &str) -> io::Result<String> {
    loop {
        if let Some(line) = readline(Some(prompt))? {
            return Ok(line);
        }
    }
}

fn prompt_dna_init(root_dir: PathBuf) -> anyhow::Result<DnaBundle> {
    let name = prompt_required("name:")?;
    let uuid = Some(prompt_default(
        "uuid:",
        "00000000-0000-0000-0000-000000000000",
    )?);
    let manifest = DnaManifest::current(name, uuid, None, vec![]);
    Ok(DnaBundle::new(manifest, vec![], root_dir)?)
}

fn prompt_app_init(root_dir: PathBuf) -> anyhow::Result<AppBundle> {
    let name = prompt_required("name:")?;
    let description = prompt_optional("description:")?;
    let slot = AppSlotManifest::sample("sample-slot".into());
    let manifest: AppManifest = AppManifestCurrentBuilder::default()
        .name(name)
        .description(description)
        .slots(vec![slot])
        .build()
        .unwrap()
        .into();
    Ok(mr_bundle::Bundle::new(manifest, vec![], root_dir)?.into())
}

pub async fn init_dna(target: PathBuf) -> anyhow::Result<()> {
    let bundle = prompt_dna_init(target.to_owned())?;
    bundle.unpack_yaml(&target, false).await?;
    Ok(())
}

pub async fn init_app(target: PathBuf) -> anyhow::Result<()> {
    let bundle = prompt_app_init(target.to_owned())?;
    bundle.unpack_yaml(&target, false).await?;
    Ok(())
}

#[cfg(test)]
mod tests {

    // TODO: make these functions able to take an arbitrary stream so that
    //       they can be tested

    // use super::*;

    // #[tokio::test]
    // async fn can_init_dna() {
    //     let tmpdir = tempdir::TempDir::new("hc_bundle").unwrap();
    //     init_dna(tmpdir.path().join("app")).await.unwrap();
    //     init_dna(tmpdir.path().join("app/n")).await.unwrap();
    // }
}
