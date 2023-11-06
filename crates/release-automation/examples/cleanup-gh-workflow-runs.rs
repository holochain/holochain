use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result as Fallible};
use octocrab::{
    models::{workflows::Run, RunId},
    Octocrab,
};

static OWNER: &str = "holochain";
static REPO: &str = "holochain";

static WORKFLOWS_KEEP_NAMES: &[&str] = &[
    "release holochain",
    "SSH session",
    "trigger source updates",
    "pages-build-deployment",
    "autorebase",
    "autoupdate",
];

static WORKFLOWS_KEEP: &[(u64, &str)] = &[(25453391, "SSH session")];

fn get_octocrab_instance() -> Fallible<Octocrab> {
    let token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| {
            let hosts = gh_config::Hosts::load()?;
            match hosts.get(gh_config::GITHUB_COM) {
                Some(host) => Ok(host.oauth_token.clone()),
                _ => bail!("Token not found."),
            }
        })
        .expect("GITHUB_TOKEN env variable or a previous `gh login` is required");

    octocrab::Octocrab::builder()
        .personal_token(token)
        .build()
        .map_err(Into::into)
}

async fn delete_workflow_run(
    instance: &Octocrab,
    owner: impl AsRef<str>,
    repo: impl AsRef<str>,
    run_id: RunId,
) -> Fallible<()> {
    let route = format!(
        "repos/{owner}/{repo}/actions/runs/{run_id}",
        owner = owner.as_ref(),
        repo = repo.as_ref(),
        run_id = run_id,
    );
    let url = instance.absolute_url(route)?;
    octocrab::map_github_error(instance._delete(url, None::<&()>).await?)
        .await
        .map(drop)
        .map_err(Into::into)
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Fallible<()> {
    let instance = get_octocrab_instance()?;

    let workflows_keep = instance
        .workflows(OWNER, REPO)
        .list()
        .send()
        .await?
        .into_iter()
        .filter_map(|workflow| {
            let result = if WORKFLOWS_KEEP_NAMES.contains(&workflow.name.as_str()) {
                Some((workflow.id.0, workflow.name.clone()))
            } else {
                None
            };

            println!(
                "workflow: id: {}, name: {} -> {:?}",
                &workflow.id, &workflow.name, &result
            );

            result
        })
        .chain(
            WORKFLOWS_KEEP
                .iter()
                .map(|(id, name)| (*id, name.to_string())),
        )
        .collect::<HashMap<_, _>>();

    let workflows_keep_ids = workflows_keep.keys().cloned().collect::<HashSet<_>>();

    let (tx, mut rx) = tokio::sync::watch::channel::<Vec<Run>>(Default::default());

    tokio::spawn({
        let instance = get_octocrab_instance()?;

        async move {
            let mut cntr = 0u8;
            while rx.changed().await.is_ok() {
                let runs = rx.borrow().to_owned();
                let runs_filtered = runs
                    .iter()
                    .cloned()
                    .filter(|run| !workflows_keep_ids.contains(&run.workflow_id.0))
                    .collect::<Vec<_>>();

                println!(
                    "page: {}, relevant items: {}/{} ...",
                    cntr,
                    runs_filtered.len(),
                    runs.len()
                );

                cntr += 1;

                for run in runs_filtered {
                    print!(
                        "removing run (name: {} ({}), id: {})",
                        run.name, run.workflow_id, run.id
                    );

                    let result = delete_workflow_run(&instance, OWNER, REPO, run.id).await;

                    match result {
                        Ok(_) => println!(" success!"),
                        Err(e) => println!(" error removing {}: {e:?}", run.id),
                    };
                }
            }
        }
    });

    {
        let mut page = instance
            .workflows(OWNER, REPO)
            .list_all_runs()
            // Send the request
            .per_page(100)
            .page(0u8)
            .send()
            .await?;

        loop {
            tx.send(page.take_items())?;

            if let Some(next_page) = instance.get_page(&page.next).await? {
                page = next_page;
            } else {
                println!("no more pages, exiting...");
                break;
            };
        }
    };

    Ok(())
}
