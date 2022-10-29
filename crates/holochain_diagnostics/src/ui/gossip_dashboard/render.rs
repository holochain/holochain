use super::*;

impl GossipDashboard {
    pub fn render<K: Backend>(&self, f: &mut Frame<K>, state: &impl ClientState) {
        let layout = layout::layout(state.nodes().len(), state.num_bases(), f);

        self.local_state.share_mut(|local| {
            let metrics: Vec<_> = state
                .nodes()
                .iter()
                .map(|n| {
                    (
                        n.diagnostics.metrics.read(),
                        n.zome.cell_id().agent_pubkey().clone(),
                    )
                })
                .collect();
            let activity = metrics
                .iter()
                .map(|(metrics, agent)| {
                    !state.node_rounds_sorted(metrics, agent).currents.is_empty()
                })
                .enumerate();
            f.render_stateful_widget(
                widgets::ui_node_list(activity),
                layout.node_list,
                &mut local.node_list_state,
            );
            f.render_widget(
                widgets::ui_basis_table(self.refresh_rate * 4, state.link_counts())
                    .block(Block::default().borders(Borders::union(Borders::LEFT, Borders::RIGHT)))
                    // the widths have to be specified here because they are not const
                    // and must be borrowed
                    .widths(&vec![Constraint::Length(3); state.num_bases()]),
                layout.basis_table,
            );
            let selected = local.selected_node();
            if selected.is_none() {
                f.render_widget(widgets::ui_keymap(), layout.bottom);
                f.render_widget(
                    widgets::ui_global_stats(self.start_time, state),
                    layout.table_extras,
                );
            }
            let gauges: Vec<_> = metrics
                .iter()
                .map(|(m, _)| ui_gossip_progress_gauge(m.incoming_gossip_progress()))
                .collect();

            if let Some(selected) = selected {
                // node.conductor.get_agent_infos(Some(node.zome.cell_id().clone()))
                let node = &state.nodes()[selected];
                let agent = node.agent();
                let metrics = &node.diagnostics.metrics.read();
                let rounds = state.node_rounds_sorted(metrics, &agent);
                for (i, gauge) in gauges.into_iter().enumerate() {
                    f.render_widget(gauge, layout.gauges[i]);
                }
                f.render_widget(
                    gossip_round_table(&GossipRoundTableState {
                        rounds: &rounds,
                        start_time: self.start_time,
                        current_time: state.time(),
                        filter_zeroes: local.filter_zeroes,
                        table_state: &local.round_table_state,
                    }),
                    layout.bottom,
                );

                if let Focus::Round { round, ours, .. } = &local.focus {
                    let table = gossip_region_table(&GossipRegionTableState {
                        regions: if *ours {
                            &round.our_diff
                        } else {
                            &round.their_diff
                        },
                    })
                    .block(Block::default().borders(Borders::all()));
                    f.render_widget(Clear, layout.modal);
                    f.render_widget(
                        Tabs::new(vec!["ours".into(), "theirs".into()]),
                        layout.modal,
                    );
                    f.render_stateful_widget(table, layout.modal, &mut local.region_table_state);
                }
            }

            let focus_status = match local.focus {
                Focus::Empty => "e",
                Focus::Node(_) => "n",
                Focus::Round { .. } => "r",
            };

            let zero_filter_status = if local.filter_zeroes { "(0)" } else { "   " };
            let (t, style) = local
                .done_time
                .map(|t| {
                    (
                        t.duration_since(self.start_time),
                        Style::default().add_modifier(Modifier::REVERSED),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        state.time().duration_since(self.start_time),
                        Style::default(),
                    )
                });
            let t_widget = Paragraph::new(format!(
                "{} {}  T={:<.2?}",
                focus_status, zero_filter_status, t
            ))
            .style(style);
            f.render_widget(t_widget, layout.time);
        });
    }
}
