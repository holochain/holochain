use std::io::Write;
use std::{io, path::PathBuf};

use holochain_types::prelude::{DnaBundle, DnaManifest, DnaResult};

fn readline<'a>(prompt: Option<&'a str>) -> io::Result<Option<String>> {
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

fn prompt_default<'a, S: Into<String>>(prompt: &'a str, default: S) -> io::Result<String> {
    let default = default.into();
    let prompt = format!("{} ({})", prompt, default);
    Ok(readline(Some(&prompt))?.unwrap_or(default))
}

fn prompt_optional<'a>(prompt: &'a str) -> io::Result<Option<String>> {
    Ok(readline(Some(prompt))?)
}

fn prompt_required<'a>(prompt: &'a str) -> io::Result<String> {
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
    let manifest = DnaManifest::new(name, uuid, None, vec![]);
    Ok(DnaBundle::new(manifest, vec![], root_dir)?)
}

pub async fn init_dna(target: PathBuf) -> anyhow::Result<()> {
    let dir = target
        .parent()
        .expect("Did not specify directory")
        .to_owned();
    let bundle = prompt_dna_init(target)?;
    bundle.unpack_yaml(&dir, false).await?;
    Ok(())
}
