//! from https://github.com/rust-lang/rustfix/blob/master/examples/fix-json.rs#L8

#![allow(unused_imports)]

use anyhow::{Context, Error};
use std::fs::File;
use std::io::Write;
use std::io::{stdin, BufReader, Read};
use std::path::PathBuf;
use std::{collections::HashMap, collections::HashSet, env, fs};

mod types {
    use serde::Deserialize;
    use serde_with::{serde_as, DefaultOnError};
    use std::path::PathBuf;

    #[derive(Debug, Deserialize)]
    #[serde(tag = "reason")]
    #[serde(rename = "kebab-case")]
    pub struct CompilerMessage {
        pub message: rustfix::diagnostics::Diagnostic,
    }
}

fn get_clippy_output() -> Result<PathBuf, Error> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(&[
        "clippy",
        "--target-dir",
        &format!(
            "{}/clippy",
            option_env!("CARGO_TARGET_DIR").unwrap_or("target")
        ),
        "--message-format=json",
        "--",
        "-A",
        "clippy::nursery",
        "-D",
        "clippy::style",
        "-A",
        "clippy::cargo",
        "-A",
        "clippy::pedantic",
        "-A",
        "clippy::restriction",
        "-D",
        "clippy::complexity",
        "-D",
        "clippy::perf",
        "-D",
        "clippy::correctness",
    ]);
    log::debug!("running {:#?}", &cmd);

    let clippy_output_raw = cmd.output()?.stdout;
    let clippy_output_string = String::from_utf8(clippy_output_raw)?;

    // simulate `jq --slurp`
    let sanitized = format!(
        "[\n  {}\n] ",
        clippy_output_string
            .lines()
            .collect::<Vec<_>>()
            .join(",\n  ")
    );

    // TODO: make this path configurable
    let path = PathBuf::from("clippy.json");

    std::fs::File::create(&path)?.write_all(sanitized.as_bytes())?;

    Ok(path)
}

fn main() -> Result<(), Error> {
    env_logger::builder()
        // .filter_level(args.log_level.to_level_filter())
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(None)
        .init();

    let mut iterations = 0;
    let mut suggestions_processed = 0;
    let mut suggestions_processed_successfully = 0;

    loop {
        iterations += 1;
        log::info!("[{}] running clippy...", iterations);

        let json_file = get_clippy_output()?;
        let json = std::fs::read_to_string(json_file)?;

        // we're only interested in the "compiler-message" reason, so we can use a single struct.
        // see: https://stackoverflow.com/questions/67702612/how-to-ignore-unknown-enum-variant-while-deserializing
        use serde::Deserialize;
        use serde_with::{serde_as, DefaultOnError};
        #[serde_as]
        #[derive(serde::Deserialize)]
        pub struct W(
            #[serde_as(as = "Vec<DefaultOnError>")] pub Vec<Option<types::CompilerMessage>>,
        );

        // Convert the JSON string to vec.
        let clippy_lint_elements: Vec<types::CompilerMessage> = serde_json::from_str::<W>(&json)
            .context(format!("failed to parse output:\n{}", json))
            .unwrap()
            .0
            .into_iter()
            .flatten()
            .collect();

        let mut path_suggestions = HashMap::<PathBuf, Vec<rustfix::Suggestion>>::new();

        let mut num_suggestions = 0;

        for m in &clippy_lint_elements {
            let mut messages = vec![&m.message];
            while let Some(message) = messages.pop() {
                messages.extend(message.children.iter());
                for span in &message.spans {
                    if let Some(new_suggestion) = rustfix::collect_suggestions(
                        message,
                        &HashSet::new(),
                        rustfix::Filter::Everything,
                    ) {
                        path_suggestions
                            .entry(PathBuf::from(span.file_name.clone()))
                            .or_insert_with(|| vec![new_suggestion]);
                        num_suggestions += 1;
                    }
                }
            }
        }

        log::info!(
            "[{}]: got {} lints and {} files with {} suggestions",
            iterations,
            clippy_lint_elements.len(),
            path_suggestions.len(),
            num_suggestions,
        );

        if num_suggestions == 0 {
            log::info!("[{}] no more suggestions found, stopping", iterations);
            break;
        }

        let mut new_suggestions_processed = 0;
        let mut new_suggestions_processed_successfully = 0;

        for (source_file, suggestions) in path_suggestions {
            let source = fs::read_to_string(&source_file)?;
            let mut fix = rustfix::CodeFix::new(&source);
            for suggestion in suggestions.iter().rev() {
                new_suggestions_processed += 1;
                if let Err(e) = fix.apply(suggestion) {
                    eprintln!("Failed to apply suggestion to {:?}: {}", &source_file, e);
                } else {
                    new_suggestions_processed_successfully += 1;
                }
            }
            let fixes = fix.finish()?;
            fs::write(source_file, fixes)?;
        }

        log::info!(
            "[{}]: suggestions processed {}, of which successful: {}",
            iterations,
            new_suggestions_processed,
            new_suggestions_processed_successfully
        );

        suggestions_processed += new_suggestions_processed;
        suggestions_processed_successfully += new_suggestions_processed_successfully;
    }

    std::fs::remove_file("clippy.json")?;

    log::info!(
        "{} iterations. suggestions processed {}, of which successful: {}",
        iterations,
        suggestions_processed,
        suggestions_processed_successfully
    );

    Ok(())
}
