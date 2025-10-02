use crate::cli::Args;
use crate::components::common::show_message;
use crate::event::ScreenEvent;
use chrono::{DateTime, Utc};
use holo_hash::AgentPubKey;
use kitsune2_api::AgentInfoSigned;
use kitsune2_core::Ed25519Verifier;
use ratatui::{prelude::*, widgets::*};
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use std::time::Instant;

fn get_agents() -> &'static RwLock<Vec<Arc<AgentInfoSigned>>> {
    static AGENTS: OnceLock<RwLock<Vec<Arc<AgentInfoSigned>>>> = OnceLock::new();

    AGENTS.get_or_init(|| RwLock::new(vec![]))
}

fn get_last_refresh_at() -> &'static RwLock<Option<Instant>> {
    static LAST_REFRESH_AT: OnceLock<RwLock<Option<Instant>>> = OnceLock::new();

    LAST_REFRESH_AT.get_or_init(|| RwLock::new(None))
}

fn get_selected() -> &'static RwLock<usize> {
    static SELECTED: OnceLock<RwLock<usize>> = OnceLock::new();

    SELECTED.get_or_init(|| RwLock::new(0))
}

pub struct BootstrapWidget {
    args: Arc<Args>,
    events: Vec<ScreenEvent>,
}

impl BootstrapWidget {
    pub fn new(args: Arc<Args>, events: Vec<ScreenEvent>) -> Self {
        Self { args, events }
    }
}

impl Widget for BootstrapWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bootstrap_url = match &self.args.bootstrap_url {
            Some(b) => b,
            None => {
                show_message("No bootstrap URL configured, to use this screen please re-run the terminal with `--boostrap-url <my-url> --dna-hash <dna-hash-base64>`", area, buf);
                return;
            }
        };

        let dna_hash = match &self.args.dna_hash {
            Some(d) => d.clone(),
            None => {
                show_message("No DNA hash configured, to use this screen please re-run the terminal with `--boostrap-url <my-url> --dna-hash <dna-hash-base64>`", area, buf);
                return;
            }
        };

        let mut refresh = false;

        for event in self.events {
            match event {
                ScreenEvent::Refresh => {
                    // Assume the refresh is permitted and clear it if not
                    refresh = true;

                    let mut last_refresh = get_last_refresh_at().write().unwrap();
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
                ScreenEvent::NavDown => {
                    let mut selected = get_selected().write().unwrap();
                    let agents = get_agents().read().unwrap();

                    if *selected < agents.len() - 1 {
                        *selected += 1;
                    }
                }
                ScreenEvent::NavUp => {
                    let mut selected = get_selected().write().unwrap();

                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
            }
        }

        if refresh {
            *get_selected().write().unwrap() = 0;

            let result = kitsune2_bootstrap_client::blocking_get(
                bootstrap_url.clone(),
                dna_hash.to_k2_space(),
                Arc::new(Ed25519Verifier),
            );
            match result {
                Ok(agents) => {
                    *get_agents().write().unwrap() = agents;
                }
                Err(e) => {
                    show_message(format!("Error fetching agents - {e:?}").as_str(), area, buf);
                    return;
                }
            };
        }

        let agents = get_agents()
            .read()
            .expect("Should have been able to read agents");

        let screen_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(screen_layout[0]);

        let list_items: Vec<ListItem> = agents
            .iter()
            .map(|a| ListItem::new(format!("{:?}", AgentPubKey::from_k2_agent(&a.agent))))
            .collect();

        let list = List::new(list_items)
            .block(Block::default().title(" Agents ").borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_symbol(">> ");

        let selected = *get_selected().read().unwrap();
        let selected = if !agents.is_empty() && selected < agents.len() {
            let detail_line = List::new(vec![
                ListItem::new(format!(
                    "agent       : {:?}",
                    AgentPubKey::from_k2_agent(&agents[selected].agent)
                )),
                ListItem::new(format!("storage arc : {:?}", agents[selected].storage_arc)),
                ListItem::new(format!("url list    : {:?}", agents[selected].url)),
                ListItem::new(format!(
                    "signed at   : {:?}",
                    DateTime::<Utc>::from_timestamp(
                        agents[selected].created_at.as_micros() / 1000,
                        0
                    )
                    .unwrap_or_default()
                )),
                ListItem::new(format!(
                    "expires at  : {:?}",
                    DateTime::<Utc>::from_timestamp(
                        agents[selected].expires_at.as_micros() / 1000,
                        0
                    )
                    .unwrap_or_default()
                )),
            ])
            .block(Block::default().title(" Detail ").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));

            Widget::render(detail_line, content_layout[1], buf);

            Some(selected)
        } else {
            None
        };

        StatefulWidget::render(
            list,
            content_layout[0],
            buf,
            &mut ListState::default().with_selected(selected),
        );

        let timeout_remaining = 10
            - (*get_last_refresh_at().read().unwrap())
                .map(|i| i.elapsed().as_secs())
                .unwrap_or(10) as i64;
        let refresh_timeout_message = if timeout_remaining > 0 {
            format!("{timeout_remaining}s")
        } else {
            "ready".to_string()
        };

        let menu_line = Line::from(vec![
            Span::styled(
                "r",
                Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            Span::raw(format!(": refresh ({refresh_timeout_message})")),
        ]);

        Widget::render(
            Paragraph::new(Text::from(vec![menu_line])),
            screen_layout[1],
            buf,
        );
    }
}
