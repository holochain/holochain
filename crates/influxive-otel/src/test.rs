// TODO: to be restored with updated implementation

// use super::*;
// use influxive_child_svc::*;
//
// #[tokio::test(flavor = "multi_thread")]
// async fn observable_report_interval() {
//     let tmp = tempfile::tempdir().unwrap();
//
//     let i = Arc::new(
//         InfluxiveChildSvc::new(
//             InfluxiveChildSvcConfig::default()
//                 .with_database_path(Some(tmp.path().into()))
//                 .with_metric_write(
//                     InfluxiveWriterConfig::default()
//                         .with_batch_duration(std::time::Duration::from_millis(5)),
//                 ),
//         )
//         .await
//         .unwrap(),
//     );
//
//     let meter_provider = InfluxiveMeterProvider::new(
//         InfluxiveMeterProviderConfig::default()
//             .with_observable_report_interval(Some(std::time::Duration::from_millis(5))),
//         i.clone(),
//     );
//     opentelemetry_api::global::set_meter_provider(meter_provider.clone());
//
//     let metric = opentelemetry_api::global::meter("test")
//         .u64_counter("m_obs_cnt_u64_r")
//         .init();
//
//     metric.add(1, &[]);
//
//     tokio::time::sleep(std::time::Duration::from_millis(200)).await;
//
//     for _ in 0..5 {
//         tokio::time::sleep(std::time::Duration::from_millis(10)).await;
//         metric.add(1, &[]);
//     }
//
//     tokio::time::sleep(std::time::Duration::from_millis(200)).await;
//
//     let result = i
//         .query(
//             r#"from(bucket: "influxive")
// |> range(start: -15m, stop: now())
// "#,
//         )
//         .await
//         .unwrap();
//
//     println!("{result}");
//
//     let result_count = result.matches("m_obs_cnt_u64_a").count();
//     assert!(
//         result_count >= 5,
//         "expected result_count >= 5, got: {result_count}"
//     );
//
//     i.shutdown();
//     drop(i);
// }
//
// #[tokio::test(flavor = "multi_thread")]
// async fn sanity() {
//     use opentelemetry_api::metrics::MeterProvider;
//
//     let tmp = tempfile::tempdir().unwrap();
//
//     let i = Arc::new(
//         InfluxiveChildSvc::new(
//             InfluxiveChildSvcConfig::default()
//                 .with_database_path(Some(tmp.path().into()))
//                 .with_metric_write(
//                     InfluxiveWriterConfig::default()
//                         .with_batch_duration(std::time::Duration::from_millis(5)),
//                 ),
//         )
//         .await
//         .unwrap(),
//     );
//
//     println!("{}", i.get_host());
//
//     i.ping().await.unwrap();
//
//     let meter_provider = InfluxiveMeterProvider::new(
//         InfluxiveMeterProviderConfig {
//             observable_report_interval: None,
//         },
//         i.clone(),
//     );
//     opentelemetry_api::global::set_meter_provider(meter_provider.clone());
//
//     let meter = opentelemetry_api::global::meter_provider().versioned_meter(
//         "my_metrics",
//         None::<&'static str>,
//         None::<&'static str>,
//         Some(vec![opentelemetry_api::KeyValue::new(
//             "test-metric-attribute-key",
//             "test-metric-attribute-value",
//         )]),
//     );
//
//     // -- f64 -- //
//
//     let m_cnt_f64 = meter.f64_counter("m_cnt_f64").init();
//     let m_hist_f64 = meter
//         .f64_histogram("m_hist_f64")
//         .with_unit(opentelemetry_api::metrics::Unit::new("s"))
//         .init();
//     // let (m_obs_cnt_f64_a, _) = meter
//     //     .f64_observable_counter_atomic("m_obs_cnt_f64_a", 0.0)
//     //     .init();
//     let m_obs_cnt_f64_r = Arc::new(meter.f64_observable_counter("m_obs_cnt_f64_r").init());
//     let m_obs_cnt_f64_r2 = m_obs_cnt_f64_r.clone();
//     meter
//         .register_callback(&[m_obs_cnt_f64_r], move |o| {
//             o.observe_f64(&*m_obs_cnt_f64_r2, 1.1, &[]);
//         })
//         .unwrap();
//     // let (m_obs_g_f64_a, _) = meter
//     //     .f64_observable_gauge_atomic("m_obs_g_f64_a", 0.0)
//     //     .init();
//     let m_obs_g_f64_r = Arc::new(meter.f64_observable_gauge("m_obs_g_f64_r").init());
//     let m_obs_g_f64_r2 = m_obs_g_f64_r.clone();
//     meter
//         .register_callback(&[m_obs_g_f64_r], move |o| {
//             o.observe_f64(&*m_obs_g_f64_r2, 1.1, &[]);
//         })
//         .unwrap();
//     let m_obs_ud_f64_r = Arc::new(
//         meter
//             .f64_observable_up_down_counter("m_obs_ud_f64_r")
//             .init(),
//     );
//     // let (m_obs_ud_f64_a, _) = meter
//     //     .f64_observable_up_down_counter_atomic("m_obs_ud_f64_a", 0.0)
//     //     .init();
//     let m_obs_ud_f64_r2 = m_obs_ud_f64_r.clone();
//     meter
//         .register_callback(&[m_obs_ud_f64_r], move |o| {
//             o.observe_f64(&*m_obs_ud_f64_r2, -1.1, &[]);
//         })
//         .unwrap();
//     let m_ud_f64 = meter.f64_up_down_counter("m_ud_f64").init();
//
//     // -- i64 -- //
//
//     let m_hist_i64 = meter.i64_histogram("m_hist_i64").init();
//     // let (m_obs_g_i64_a, _) =
//     //     meter.i64_observable_gauge_atomic("m_obs_g_i64_a", 0).init();
//     let m_obs_g_i64_r = Arc::new(meter.i64_observable_gauge("m_obs_g_i64_r").init());
//     let m_obs_g_i64_r2 = m_obs_g_i64_r.clone();
//     meter
//         .register_callback(&[m_obs_g_i64_r], move |o| {
//             o.observe_i64(&*m_obs_g_i64_r2, -1, &[]);
//         })
//         .unwrap();
//     // let (m_obs_ud_i64_a, _) = meter
//     //     .i64_observable_up_down_counter_atomic("m_obs_ud_i64_a", 0)
//     //     .init();
//     let m_obs_ud_i64_r = Arc::new(
//         meter
//             .i64_observable_up_down_counter("m_obs_ud_i64_r")
//             .init(),
//     );
//     let m_obs_ud_i64_r2 = m_obs_ud_i64_r.clone();
//     meter
//         .register_callback(&[m_obs_ud_i64_r], move |o| {
//             o.observe_i64(&*m_obs_ud_i64_r2, -1, &[]);
//         })
//         .unwrap();
//     let m_ud_i64 = meter.i64_up_down_counter("m_ud_i64").init();
//
//     // -- u64 -- /
//
//     let m_cnt_u64 = meter.u64_counter("m_cnt_u64").init();
//     let m_hist_u64 = meter.u64_histogram("m_hist_u64").init();
//     // let (m_obs_cnt_u64_a, _) = meter
//     //     .u64_observable_counter_atomic("m_obs_cnt_u64_a", 0)
//     //     .init();
//     let m_obs_cnt_u64_r = Arc::new(meter.u64_observable_counter("m_obs_cnt_u64_r").init());
//     let m_obs_cnt_u64_r2 = m_obs_cnt_u64_r.clone();
//     meter
//         .register_callback(&[m_obs_cnt_u64_r], move |o| {
//             o.observe_u64(&*m_obs_cnt_u64_r2, 1, &[])
//         })
//         .unwrap();
//     // let (m_obs_g_u64_a, _) =
//     //     meter.u64_observable_gauge_atomic("m_obs_g_u64_a", 0).init();
//     let m_obs_g_u64_r = Arc::new(meter.u64_observable_gauge("m_obs_g_u64_r").init());
//     let m_obs_g_u64_r2 = m_obs_g_u64_r.clone();
//     meter
//         .register_callback(&[m_obs_g_u64_r], move |o| {
//             o.observe_u64(&*m_obs_g_u64_r2, 1, &[])
//         })
//         .unwrap();
//
//     for _ in 0..12 {
//         tokio::time::sleep(std::time::Duration::from_millis(1)).await;
//
//         m_cnt_f64.add(1.1, &[]);
//         m_hist_f64.record(1.1, &[]);
//         // m_obs_cnt_f64_a.add(1.1);
//         // m_obs_g_f64_a.set(-1.1);
//         // m_obs_ud_f64_a.add(-1.1);
//         m_ud_f64.add(-1.1, &[]);
//
//         m_hist_i64.record(-1, &[]);
//         // m_obs_g_i64_a.set(-1);
//         // m_obs_ud_i64_a.add(-1);
//         m_ud_i64.add(-1, &[]);
//
//         m_cnt_u64.add(1, &[]);
//         m_hist_u64.record(1, &[]);
//         // m_obs_cnt_u64_a.add(1);
//         // m_obs_g_u64_a.set(1);
//
//         // trigger reporting of observable metrics
//         meter_provider.report();
//     }
//
//     tokio::time::sleep(std::time::Duration::from_secs(2)).await;
//
//     let result = i
//         .query(
//             r#"from(bucket: "influxive")
// |> range(start: -15m, stop: now())
// "#,
//         )
//         .await
//         .unwrap();
//
//     println!("{result}");
//
//     assert_eq!(12, result.matches("m_cnt_f64").count());
//     assert_eq!(12, result.matches("m_hist_f64").count());
//     assert_eq!(12, result.matches("m_obs_cnt_f64_a").count());
//     assert_eq!(12, result.matches("m_obs_cnt_f64_r").count());
//     assert_eq!(12, result.matches("m_obs_g_f64_a").count());
//     assert_eq!(12, result.matches("m_obs_g_f64_r").count());
//     assert_eq!(12, result.matches("m_obs_ud_f64_a").count());
//     assert_eq!(12, result.matches("m_obs_ud_f64_r").count());
//     assert_eq!(12, result.matches("m_ud_f64").count());
//
//     assert_eq!(12, result.matches("m_hist_i64").count());
//     assert_eq!(12, result.matches("m_obs_g_i64_a").count());
//     assert_eq!(12, result.matches("m_obs_g_i64_r").count());
//     assert_eq!(12, result.matches("m_obs_ud_i64_a").count());
//     assert_eq!(12, result.matches("m_obs_ud_i64_r").count());
//     assert_eq!(12, result.matches("m_ud_i64").count());
//
//     assert_eq!(12, result.matches("m_cnt_u64").count());
//     assert_eq!(12, result.matches("m_hist_u64").count());
//     assert_eq!(12, result.matches("m_obs_cnt_u64_a").count());
//     assert_eq!(12, result.matches("m_obs_cnt_u64_r").count());
//     assert_eq!(12, result.matches("m_obs_g_u64_a").count());
//     assert_eq!(12, result.matches("m_obs_g_u64_r").count());
//
//     println!("about to shutdown influxive-child-svc");
//     i.shutdown();
//
//     println!("about to drop influxive-child-svc");
//     drop(i);
//
//     println!("about to close tempfile::tempdir");
//     // okay if this fails on windows...
//     let _ = tmp.close();
//
//     println!("test complete");
// }
