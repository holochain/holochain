use super::*;

impl GossipDashboard {
    pub fn render<K: Backend>(&self, f: &mut Frame<K>, state: &impl ClientState) {
        let layout = layout::layout(state.nodes().len(), state.num_bases(), f);

        self.local_state.share_mut(|local| {
            let metrics: Vec<_> = state
                .nodes()
                .iter()
                .map(|n| (n.diagnostics.metrics.clone(), n.id.clone()))
                .collect();

            let queue_info: Vec<_> = state
                .nodes()
                .iter()
                .enumerate()
                .map(|(_i, n)| {
                    // tracing::info!("{} {:?}\n{}", i, n.cert, n.diagnostics.fetch_pool.summary());
                    (
                        n.diagnostics.fetch_pool.info(
                            [n.zome.cell_id().dna_hash().to_kitsune()]
                                .into_iter()
                                .collect(),
                        ),
                        n.id.clone(),
                    )
                })
                .collect();

            {
                let activity = metrics
                    .iter()
                    .map(|(metrics, _)| {
                        !metrics.read(|m| state.node_rounds_sorted(m).currents.is_empty())
                    })
                    .enumerate();
                f.render_stateful_widget(
                    widgets::ui_node_list(activity),
                    layout.node_list,
                    &mut local.node_list_state,
                );
            }
            f.render_widget(
                widgets::ui_basis_table(self.refresh_rate * 4, state.link_counts())
                    .block(Block::default().borders(Borders::union(Borders::LEFT, Borders::RIGHT)))
                    // the widths have to be specified here because they are not const
                    // and must be borrowed
                    .widths(&vec![Constraint::Length(3); state.num_bases()]),
                layout.basis_table,
            );

            // {
            //     let sums = todo!("get total throughput");
            //     f.render_widget(
            //         widgets::ui_throughput_summary(sums),
            //         layout.throughput_summary,
            //     );
            // }

            let selected = local.selected_node();
            if selected.is_none() {
                f.render_widget(widgets::ui_keymap(), layout.bottom);
                f.render_widget(
                    widgets::ui_global_stats(self.start_time, state),
                    layout.table_extras,
                );
            }
            let gauges: Vec<_> = queue_info
                .iter()
                .map(|(i, _)| Paragraph::new(i.op_bytes_to_fetch.human_count_bytes().to_string()))
                .collect();

            if let Some(selected) = selected {
                let node = &state.nodes()[selected];
                node.diagnostics.metrics.read(|metrics| {
                    let rounds = state.node_rounds_sorted(metrics);
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
                });

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
