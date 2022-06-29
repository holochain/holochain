use std::io::Write;
use std::{io, path::PathBuf};

use holochain_types::prelude::{
    AppBundle, AppManifest, AppManifestCurrentBuilder, AppRoleManifest, DnaBundle, DnaManifest,
    Timestamp,
};
use holochain_types::web_app::{WebAppBundle, WebAppManifest};

fn readline(prompt: Option<&str>) -> io::Result<Option<String>> {
    let mut input = String::new();
    if let Some(prompt) = prompt {
        print!("{} ", prompt);
        io::stdout().flush()?;
    }
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    Ok(if input.is_empty() {
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
    readline(Some(prompt))
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
    let uid = Some(prompt_default(
        "uid:",
        "00000000-0000-0000-0000-000000000000",
    )?);
    let manifest = DnaManifest::current(name, uid, None, Timestamp::now().into(), vec![], vec![]);
    Ok(DnaBundle::new(manifest.try_into()?, vec![], root_dir)?)
}

fn prompt_app_init(root_dir: PathBuf) -> anyhow::Result<AppBundle> {
    let name = prompt_required("name:")?;
    let description = prompt_optional("description:")?;
    let role = AppRoleManifest::sample("sample-role".into());
    let manifest: AppManifest = AppManifestCurrentBuilder::default()
        .name(name)
        .description(description)
        .roles(vec![role])
        .build()
        .unwrap()
        .into();

    Ok(mr_bundle::Bundle::new(manifest, vec![], root_dir)?.into())
}

fn prompt_web_app_init(root_dir: PathBuf) -> anyhow::Result<WebAppBundle> {
    let name = prompt_required("name:")?;

    let manifest = WebAppManifest::current(name);

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

pub async fn init_web_app(target: PathBuf) -> anyhow::Result<()> {
    let bundle = prompt_web_app_init(target.to_owned())?;
    bundle.unpack_yaml(&target, false).await?;
    Ok(())
}

#[cfg(test)]
mod tests {

    // TODO: make these functions able to take an arbitrary stream so that
    //       they can be tested
}
