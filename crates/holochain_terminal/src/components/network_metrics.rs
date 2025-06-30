use crate::cli::Args;
use crate::client::AppClient;
use crate::components::common::show_message;
use crate::event::ScreenEvent;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use holo_hash::{AgentPubKey, DnaHash};
use holochain_types::network::Kitsune2NetworkMetrics;
use holochain_util::tokio_helper::block_on;
use once_cell::sync::Lazy;
use ratatui::{prelude::*, widgets::*};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

type NetworkInfoParams = (AgentPubKey, Vec<(String, DnaHash)>);

static NETWORK_METRICS_PARAMS: Lazy<Arc<RwLock<Option<NetworkInfoParams>>>> =
    Lazy::new(|| Arc::new(RwLock::new(None)));

fn get_selected() -> &'static RwLock<usize> {
    static SELECTED: OnceLock<RwLock<usize>> = OnceLock::new();

    SELECTED.get_or_init(|| RwLock::new(0))
}

fn get_last_refresh_at() -> &'static RwLock<Option<Instant>> {
    static LAST_REFRESH_AT: OnceLock<RwLock<Option<Instant>>> = OnceLock::new();

    LAST_REFRESH_AT.get_or_init(|| RwLock::new(None))
}

fn get_network_metrics() -> &'static RwLock<HashMap<DnaHash, Kitsune2NetworkMetrics>> {
    static NETWORK_INFO: OnceLock<RwLock<HashMap<DnaHash, Kitsune2NetworkMetrics>>> =
        OnceLock::new();

    NETWORK_INFO.get_or_init(|| RwLock::new(HashMap::with_capacity(0)))
}

pub struct NetworkMetricsWidget {
    args: Arc<Args>,
    app_client: Option<Arc<Mutex<AppClient>>>,
    events: Vec<ScreenEvent>,
}

impl NetworkMetricsWidget {
    pub fn new(
        args: Arc<Args>,
        app_client: Option<Arc<Mutex<AppClient>>>,
        events: Vec<ScreenEvent>,
    ) -> Self {
        Self {
            args,
            app_client,
            events,
        }
    }
}

impl Widget for NetworkMetricsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let app_id = match &self.args.app_id {
            Some(b) => b.clone(),
            None => {
                show_message("No app ID configured, to use this screen please re-run the terminal with `--admin-url <my-url> --app-id <my-app-id>`", area, buf);
                return;
            }
        };

        let app_client = match self.app_client.clone() {
            Some(b) => b,
            None => {
                show_message("No admin URL configured, to use this screen please re-run the terminal with `--admin-url <my-url> --app-id <my-app-id>`", area, buf);
                return;
            }
        };

        if NETWORK_METRICS_PARAMS.read().unwrap().is_none() {
            *NETWORK_METRICS_PARAMS.write().unwrap() =
                match get_network_metrics_params(app_client.clone(), app_id.clone()) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        show_message(format!("{:?}", e).as_str(), area, buf);
                        return;
                    }
                };
        }

        {
            let mut last_refreshed = get_last_refresh_at().write().unwrap();
            let mut do_refresh = false;
            if let Some(lr) = *last_refreshed {
                if lr.elapsed() > Duration::from_secs(10) {
                    *last_refreshed = Some(Instant::now());
                    do_refresh = true;
                }
            } else {
                *last_refreshed = Some(Instant::now());
                do_refresh = true;
            }

            if do_refresh {
                match fetch_network_metrics(app_client) {
                    Ok(metrics) => {
                        *get_network_metrics().write().unwrap() = metrics;
                    }
                    Err(e) => {
                        show_message(format!("{:?}", e).as_str(), area, buf);
                        return;
                    }
                }
            };
        }

        let network_metrics_params = NETWORK_METRICS_PARAMS.read().unwrap();
        let network_metrics_params_value = network_metrics_params
            .as_ref()
            .expect("Should have network metrics params");

        for event in self.events {
            match event {
                ScreenEvent::NavDown => {
                    let mut selected = get_selected().write().unwrap();
                    if *selected < network_metrics_params_value.1.len() - 1 {
                        *selected += 1;
                    }
                }
                ScreenEvent::NavUp => {
                    let mut selected = get_selected().write().unwrap();

                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                _ => {
                    // Ignored
                }
            }
        }

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let list_items: Vec<ListItem> = NETWORK_METRICS_PARAMS
            .read()
            .unwrap()
            .clone()
            .unwrap()
            .1
            .into_iter()
            .map(|(name, dna_hash)| ListItem::new(format!("{} - {:?}", name, dna_hash)))
            .collect();

        let list = List::new(list_items)
            .block(
                Block::default()
                    .title(format!(
                        " Network info for {} (age: {}s)",
                        app_id,
                        get_last_refresh_at()
                            .read()
                            .unwrap()
                            .expect("Should be able to get refresh time")
                            .elapsed()
                            .as_secs() as u32
                    ))
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_symbol(">> ");

        let selected = *get_selected().read().unwrap();

        let selected = if !network_metrics_params_value.1.is_empty()
            && selected < network_metrics_params_value.1.len()
        {
            let dna_hash = &network_metrics_params_value.1[selected].1;

            let network_metrics = get_network_metrics().read().unwrap();
            let Some(metrics) = network_metrics.get(dna_hash) else {
                return;
            };

            let detail_line = List::new(vec![
                // Fetch
                ListItem::new(format!(
                    "Pending fetch requests : {:?}",
                    metrics.fetch_state_summary.pending_requests.len(),
                )),
                // Gossip
                ListItem::new(format!(
                    "Gossip peers           : {:?}",
                    metrics.gossip_state_summary.peer_meta.len(),
                )),
                ListItem::new(format!(
                    "Last gossip round      : {:?}",
                    metrics
                        .gossip_state_summary
                        .peer_meta
                        .iter()
                        .filter_map(|meta| meta.1.last_gossip_timestamp)
                        .max()
                        .map(|ts| { DateTime::<Utc>::from_timestamp(ts.as_micros() / 1000, 0) }),
                )),
                ListItem::new(format!(
                    "Peer behaviour errors  : {:?}",
                    metrics
                        .gossip_state_summary
                        .peer_meta
                        .iter()
                        .filter_map(|meta| meta.1.peer_behavior_errors)
                        .sum::<u32>(),
                )),
                ListItem::new(format!(
                    "Local errors           : {:?}",
                    metrics
                        .gossip_state_summary
                        .peer_meta
                        .iter()
                        .filter_map(|meta| meta.1.local_errors)
                        .sum::<u32>(),
                )),
                ListItem::new(format!(
                    "Peer busy              : {:?}",
                    metrics
                        .gossip_state_summary
                        .peer_meta
                        .iter()
                        .filter_map(|meta| meta.1.peer_busy)
                        .sum::<u32>(),
                )),
                ListItem::new(format!(
                    "Completed rounds       : {:?}",
                    metrics
                        .gossip_state_summary
                        .peer_meta
                        .iter()
                        .filter_map(|meta| meta.1.completed_rounds)
                        .sum::<u32>(),
                )),
                ListItem::new(format!(
                    "Peer timeouts          : {:?}",
                    metrics
                        .gossip_state_summary
                        .peer_meta
                        .iter()
                        .filter_map(|meta| meta.1.peer_timeouts)
                        .sum::<u32>(),
                )),
            ])
            .block(Block::default().title(" Info ").borders(Borders::ALL))
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
    }
}

fn get_network_metrics_params(
    app_client: Arc<Mutex<AppClient>>,
    app_id: String,
) -> anyhow::Result<(AgentPubKey, Vec<(String, DnaHash)>)> {
    match block_on(
        async {
            app_client
                .lock()
                .await
                .discover_network_metrics_params(app_id)
                .await
        },
        Duration::from_secs(10),
    ) {
        Ok(Ok(p)) => Ok(p),
        Ok(Err(e)) => Err(anyhow!("Error fetching network metrics params - {:?}", e)),
        Err(_) => Err(anyhow!("Timeout while fetching network metrics params")),
    }
}

fn fetch_network_metrics(
    app_client: Arc<Mutex<AppClient>>,
) -> anyhow::Result<HashMap<DnaHash, Kitsune2NetworkMetrics>> {
    match block_on(
        async { app_client.lock().await.network_metrics().await },
        Duration::from_secs(10),
    ) {
        Ok(Ok(metrics)) => Ok(metrics),
        Ok(Err(e)) => Err(anyhow!("Failed to fetch network metrics - {:?}", e)),
        Err(_) => Err(anyhow!("Timeout while fetching network metrics")),
    }
}
