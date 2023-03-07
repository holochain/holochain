use super::*;

pub enum InputCmd {
    Quit,
    ClearBuffer,
    ExchangePeers,
    AddNode(usize),
    RemoveNode(usize),
    AddEntry(usize),
    AwakenGossip,
}

impl GossipDashboard {
    #[allow(clippy::single_match)]
    pub fn input<S: ClientState>(&self, state: RwShare<S>) -> Option<InputCmd> {
        if event::poll(self.refresh_rate).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    KeyCode::Char('q') => {
                        return Some(InputCmd::Quit);
                    }
                    KeyCode::Char('c') => {
                        return Some(InputCmd::ClearBuffer);
                    }
                    KeyCode::Char('x') => {
                        if let Some(node) = self.local_state.share_ref(|s| s.selected_node()) {
                            return Some(InputCmd::RemoveNode(node));
                        }
                    }
                    KeyCode::Char('n') => {
                        if let Some(node) = self.local_state.share_ref(|s| s.selected_node()) {
                            return Some(InputCmd::AddNode(node));
                        }
                    }
                    KeyCode::Char('e') => {
                        if let Some(node) = self.local_state.share_ref(|s| s.selected_node()) {
                            return Some(InputCmd::AddEntry(node));
                        }
                    }
                    KeyCode::Char('g') => {
                        return Some(InputCmd::AwakenGossip);
                    }
                    KeyCode::Enter => self.local_state.share_mut(|s| match s.focus {
                        Focus::Empty => {
                            if let Some(n) = s.selected_node() {
                                s.focus = Focus::Node(n);
                                s.round_table_state.select(Some(0));
                            }
                        }
                        Focus::Node(node) => {
                            if let Some(round) = s.round_table_state.selected() {
                                let diffs = state.share_mut(|state| {
                                    let node = &state.nodes()[node];
                                    let metrics = node.diagnostics.metrics.read();
                                    let histories = state.node_rounds_sorted(&metrics);
                                    histories.round_regions(round).clone()
                                });
                                if let Some((our_diff, their_diff)) = diffs {
                                    s.focus = Focus::Round {
                                        node,
                                        round: RoundInfo {
                                            our_diff,
                                            their_diff,
                                        },
                                        ours: false,
                                    };
                                }
                            } else {
                                panic!("round table must have selection");
                            }
                        }
                        _ => {}
                    }),
                    KeyCode::Esc => self.local_state.share_mut(|s| match s.focus {
                        Focus::Node(_) => {
                            s.focus = Focus::Empty;
                            s.round_table_state.select(None);
                        }
                        Focus::Round { node, .. } => {
                            s.focus = Focus::Node(node);
                            s.region_table_state.select(None);
                        }
                        _ => {}
                    }),
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.local_state.share_mut(|s| match s.focus {
                            Focus::Empty => {
                                s.node_selector(-1, state.share_ref(|state| state.nodes().len()))
                            }
                            Focus::Node(_) => s.round_selector(-1),
                            Focus::Round { .. } => s.region_selector(-1),
                        })
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.local_state.share_mut(|s| match s.focus {
                            Focus::Empty => {
                                s.node_selector(1, state.share_ref(|state| state.nodes().len()))
                            }
                            Focus::Node(_) => s.round_selector(1),
                            Focus::Round { .. } => s.region_selector(1),
                        })
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.local_state.share_mut(|s| match s.focus {
                            Focus::Round { ref mut ours, .. } => *ours = false,
                            _ => (),
                        })
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.local_state.share_mut(|s| match s.focus {
                            Focus::Round { ref mut ours, .. } => *ours = true,
                            _ => (),
                        })
                    }
                    KeyCode::Char('0') => self
                        .local_state
                        .share_mut(|s| s.filter_zeroes = !s.filter_zeroes),
                    _ => {}
                }
            }
        };
        None
    }
}
