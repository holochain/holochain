use crate::cli::Args;
use crate::components::common::show_message;
use crate::event::ScreenEvent;
use chrono::{DateTime, Utc};
use holo_hash::AgentPubKey;
use holochain_util::tokio_helper::block_on;
use kitsune_p2p_bin_data::KitsuneAgent;
use kitsune_p2p_bin_data::{KitsuneBinType, KitsuneSpace};
use kitsune_p2p_bootstrap_client::{random, BootstrapNet};
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::{RandomLimit, RandomQuery};
use once_cell::sync::Lazy;
use ratatui::{prelude::*, widgets::*};
use std::sync::{Arc, RwLock};
use std::time::Instant;

static NETWORK_TYPE: Lazy<RwLock<BootstrapNet>> = Lazy::new(|| RwLock::new(BootstrapNet::Tx5));
static AGENTS: Lazy<RwLock<Vec<AgentInfoSigned>>> = Lazy::new(|| RwLock::new(vec![]));
static LAST_REFRESH_AT: Lazy<RwLock<Option<Instant>>> = Lazy::new(|| RwLock::new(None));
static SELECTED: Lazy<RwLock<usize>> = Lazy::new(|| RwLock::new(0));

pub fn render_bootstrap_widget<B: Backend>(
    args: &Args,
    events: Vec<ScreenEvent>,
    frame: &mut Frame<B>,
    rect: Rect,
) {
    let bootstrap_url = match &args.bootstrap_url {
        Some(b) => b,
        None => {
            show_message("No bootstrap URL configured, to use this screen please re-run the terminal with `--boostrap-url <my-url> --dna-hash <dna-hash-base64>`", frame, rect);
            return;
        }
    };

    let dna_hash = match &args.dna_hash {
        Some(d) => d.clone(),
        None => {
            show_message("No DNA hash configured, to use this screen please re-run the terminal with `--boostrap-url <my-url> --dna-hash <dna-hash-base64>`", frame, rect);
            return;
        }
    };

    let mut refresh = false;
    let mut switch_network = false;

    for event in events {
        match event {
            ScreenEvent::Refresh => {
                // Assume the refresh is permitted and clear it if not
                refresh = true;

                let mut last_refresh = LAST_REFRESH_AT.write().unwrap();
                if let Some(lr) = last_refresh.as_ref() {
                    if lr.elapsed().as_millis() < 10000 {
                        refresh = false;
                    } else {
                        // Permitting refresh, set up a new timer
                        *last_refresh = Some(Instant::now());
                    }
                } else {
                    // First refresh, set up a timer
                    *last_refresh = Some(Instant::now());
                }
            }
            ScreenEvent::SwitchNetwork => {
                switch_network = true;
                refresh = true; // Always refresh when switching network
                *LAST_REFRESH_AT.write().unwrap() = Some(Instant::now()); // Reset the refresh timer
            }
            ScreenEvent::NavDown => {
                let mut selected = SELECTED.write().unwrap();
                let agents = AGENTS.read().unwrap();

                if *selected < agents.len() - 1 {
                    *selected += 1;
                }
            }
            ScreenEvent::NavUp => {
                let mut selected = SELECTED.write().unwrap();

                if *selected > 0 {
                    *selected -= 1;
                }
            }
        }
    }

    if switch_network {
        let mut network_type = NETWORK_TYPE
            .write()
            .expect("Should have been able to read network type");

        let new_net = match *network_type {
            BootstrapNet::Tx2 => BootstrapNet::Tx5,
            BootstrapNet::Tx5 => BootstrapNet::Tx2,
        };

        *network_type = new_net;
    }

    if refresh {
        *SELECTED.write().unwrap() = 0;

        match block_on(
            async {
                let network_type = { *NETWORK_TYPE.read().unwrap() };
                random(
                    Some(bootstrap_url.into()),
                    RandomQuery {
                        // TODO This conversion is defined but it's in holochain_p2p which shouldn't be a dep of this crate.
                        space: Arc::new(KitsuneSpace::new(dna_hash.get_raw_36().to_vec())),
                        limit: RandomLimit(30),
                    },
                    network_type,
                )
                .await
            },
            std::time::Duration::from_secs(10),
        ) {
            Ok(Ok(agents)) => {
                *AGENTS.write().unwrap() = agents;
            }
            Ok(Err(e)) => {
                show_message(
                    format!("Error fetching agents - {:?}", e).as_str(),
                    frame,
                    rect,
                );
                return;
            }
            Err(_) => {
                show_message("Timeout while fetching agents", frame, rect);
                return;
            }
        };
    }

    let agents = AGENTS.read().expect("Should have been able to read agents");

    let screen_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(rect);

    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(screen_layout[0]);

    let list_items: Vec<ListItem> = agents
        .iter()
        .map(|a| ListItem::new(format!("{:?}", kitsune_agent_to_pub_key(a.agent.clone()))))
        .collect();

    let list = List::new(list_items)
        .block(Block::default().title(" Agents ").borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_symbol(">> ");

    let selected = *SELECTED.read().unwrap();
    let selected = if !agents.is_empty() && selected < agents.len() {
        let detail_line = List::new(vec![
            ListItem::new(format!(
                "agent       : {:?}",
                kitsune_agent_to_pub_key(agents[selected].agent.clone())
            )),
            ListItem::new(format!("storage arc : {:?}", agents[selected].storage_arc)),
            ListItem::new(format!("url list    : {:?}", agents[selected].url_list)),
            ListItem::new(format!(
                "signed at   : {:?}",
                DateTime::<Utc>::from_timestamp((agents[selected].signed_at_ms / 1000) as i64, 0)
                    .unwrap_or_default()
            )),
            ListItem::new(format!(
                "expires at  : {:?}",
                DateTime::<Utc>::from_timestamp((agents[selected].expires_at_ms / 1000) as i64, 0)
                    .unwrap_or_default()
            )),
        ])
        .block(Block::default().title(" Detail ").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

        frame.render_widget(detail_line, content_layout[1]);

        Some(selected)
    } else {
        None
    };

    frame.render_stateful_widget(
        list,
        content_layout[0],
        &mut ListState::default().with_selected(selected),
    );

    let timeout_remaining = 10
        - (*LAST_REFRESH_AT.read().unwrap())
            .map(|i| i.elapsed().as_secs())
            .unwrap_or(10) as i64;
    let refresh_timeout_message = if timeout_remaining > 0 {
        format!("{}s", timeout_remaining)
    } else {
        "ready".to_string()
    };

    let menu_line = Line::from(vec![
        Span::styled(
            "r",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::raw(format!(": refresh ({})", refresh_timeout_message)),
        Span::raw("    "),
        Span::styled(
            "n",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::raw(format!(
            ": network type ({:?})",
            NETWORK_TYPE
                .read()
                .expect("Should have been able to read network type")
        )),
    ]);

    frame.render_widget(
        Paragraph::new(Text::from(vec![menu_line])),
        screen_layout[1],
    );
}

fn kitsune_agent_to_pub_key(agent: Arc<KitsuneAgent>) -> AgentPubKey {
    AgentPubKey::from_raw_36((*agent).clone().into())
}
