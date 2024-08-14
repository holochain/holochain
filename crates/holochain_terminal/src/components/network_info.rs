use crate::cli::Args;
use crate::client::AppClient;
use crate::components::common::show_message;
use crate::event::ScreenEvent;
use anyhow::anyhow;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_conductor_api::NetworkInfo;
use holochain_util::tokio_helper::block_on;
use kitsune_p2p_types::dependencies::tokio;
use once_cell::sync::Lazy;
use ratatui::{prelude::*, widgets::*};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;

type NetworkInfoParams = (AgentPubKey, Vec<(String, DnaHash)>);

static NETWORK_INFO_PARAMS: Lazy<Arc<RwLock<Option<NetworkInfoParams>>>> =
    Lazy::new(|| Arc::new(RwLock::new(None)));
static SELECTED: Lazy<RwLock<usize>> = Lazy::new(|| RwLock::new(0));

pub struct NetworkInfoWidget {
    args: Arc<Args>,
    app_client: Option<Arc<Mutex<AppClient>>>,
    events: Vec<ScreenEvent>,
}

impl NetworkInfoWidget {
    pub fn new(args: Arc<Args>, app_client: Option<Arc<Mutex<AppClient>>>, events: Vec<ScreenEvent>) -> Self {
        Self { args, app_client, events }
    }
}

impl Widget for NetworkInfoWidget {
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

        if NETWORK_INFO_PARAMS.read().unwrap().is_none() {
            *NETWORK_INFO_PARAMS.write().unwrap() =
                match get_network_info_params(app_client.clone(), app_id.clone()) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        show_message(format!("{:?}", e).as_str(), area, buf);
                        return;
                    }
                };
        }

        let network_infos = match get_network_info(app_client) {
            Ok(network_infos) => network_infos,
            Err(e) => {
                show_message(format!("{:?}", e).as_str(), area, buf);
                return;
            }
        };

        for event in self.events {
            match event {
                ScreenEvent::NavDown => {
                    let mut selected = SELECTED.write().unwrap();
                    if *selected < network_infos.len() - 1 {
                        *selected += 1;
                    }
                }
                ScreenEvent::NavUp => {
                    let mut selected = SELECTED.write().unwrap();

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

        let list_items: Vec<ListItem> = NETWORK_INFO_PARAMS
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
                    .title(format!(" Network info for {} ", app_id))
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White))
            .highlight_symbol(">> ");

        let selected = *SELECTED.read().unwrap();
        let selected = if !network_infos.is_empty() && selected < network_infos.len() {
            let detail_line = List::new(vec![
                ListItem::new(format!(
                    "peers             : {:?}",
                    network_infos[selected].current_number_of_peers,
                )),
                ListItem::new(format!(
                    "total peers       : {:?}",
                    network_infos[selected].total_network_peers
                )),
                ListItem::new(format!(
                    "total bytes       : {:?}",
                    network_infos[selected].bytes_since_last_time_queried
                )),
                ListItem::new(format!(
                    "num ops to fetch  : {:?}",
                    network_infos[selected].fetch_pool_info.num_ops_to_fetch
                )),
                ListItem::new(format!(
                    "op bytes to fetch : {:?}",
                    network_infos[selected].fetch_pool_info.op_bytes_to_fetch
                )),
            ])
                .block(Block::default().title(" Info ").borders(Borders::ALL))
                .style(Style::default().fg(Color::White));

            Widget::render(detail_line, content_layout[1], buf);

            Some(selected)
        } else {
            None
        };

        StatefulWidget::render(list, content_layout[0], buf, &mut ListState::default().with_selected(selected));
    }
}

fn get_network_info_params(
    app_client: Arc<Mutex<AppClient>>,
    app_id: String,
) -> anyhow::Result<(AgentPubKey, Vec<(String, DnaHash)>)> {
    match block_on(
        async {
            app_client
                .lock()
                .await
                .discover_network_info_params(app_id)
                .await
        },
        Duration::from_secs(10),
    ) {
        Ok(Ok(p)) => Ok(p),
        Ok(Err(e)) => Err(anyhow!("Error fetching network info params - {:?}", e)),
        Err(_) => Err(anyhow!("Timeout while fetching network info params")),
    }
}

fn get_network_info(app_client: Arc<Mutex<AppClient>>) -> anyhow::Result<Vec<NetworkInfo>> {
    let (agent, named_dna_hashes) = NETWORK_INFO_PARAMS.read().unwrap().clone().unwrap();
    match block_on(
        async {
            app_client
                .lock()
                .await
                .network_info(
                    agent,
                    named_dna_hashes.into_iter().map(|(_, h)| h).collect(),
                )
                .await
        },
        Duration::from_secs(10),
    ) {
        Ok(Ok(network_infos)) => Ok(network_infos),
        Ok(Err(e)) => Err(anyhow!("Failed to fetch network infos - {:?}", e)),
        Err(_) => Err(anyhow!("Timeout while fetching network infos")),
    }
}
