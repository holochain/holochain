#![deny(missing_docs)]
#![deny(warnings)]
#![deny(unsafe_code)]
//! Opentelemetry metrics bindings for influxive-child-svc.
//!
//! ## Example
//!
//! ```
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() {
//! #     use std::sync::Arc;
//! use influxive::writer::*;
//!
//! // create an influxive writer
//! let writer = InfluxiveWriter::with_token_auth(
//!     InfluxiveWriterConfig::default(),
//!     "http://127.0.0.1:8086",
//!     "my.bucket",
//!     "my.token",
//! );
//!
//! // register the meter provider
//! opentelemetry_api::global::set_meter_provider(
//!     influxive::otel::InfluxiveMeterProvider::new(
//!         Default::default(),
//!         Arc::new(writer),
//!     )
//! );
//!
//! // create a metric
//! let m = opentelemetry_api::global::meter("my.meter")
//!     .f64_histogram("my.metric")
//!     .init();
//!
//! // make a recording
//! m.record(3.14, &[]);
//! # }
//! ```

use crate::types::*;
use opentelemetry_api::metrics::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

type Erased = Box<dyn Fn() + 'static + Send + Sync>;
struct ErasedMap(Mutex<HashMap<u64, Erased>>);

impl ErasedMap {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(HashMap::new())))
    }

    pub fn push(&self, erased: Erased) -> u64 {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.0.lock().unwrap().insert(id, erased);
        id
    }

    pub fn remove(&self, id: u64) {
        self.0.lock().unwrap().remove(&id);
    }

    pub fn invoke(&self) {
        let mut map = std::mem::take(&mut *self.0.lock().unwrap());
        for (_, cb) in map.iter() {
            cb();
        }
        let mut lock = self.0.lock().unwrap();
        for (id, cb) in lock.drain() {
            map.insert(id, cb);
        }
        std::mem::swap(&mut *lock, &mut map);
    }
}

struct InfluxiveUniMetric<T: 'static + std::fmt::Display + Into<DataType> + Send + Sync> {
    this: std::sync::Weak<Self>,
    influxive: Arc<dyn MetricWriter + 'static + Send + Sync>,
    name: std::borrow::Cow<'static, str>,
    unit: Option<opentelemetry_api::metrics::Unit>,
    attributes: Option<Arc<[opentelemetry_api::KeyValue]>>,
    _p: std::marker::PhantomData<T>,
}

impl<T: 'static + std::fmt::Display + Into<DataType> + Send + Sync> InfluxiveUniMetric<T> {
    pub fn new(
        influxive: Arc<dyn MetricWriter + 'static + Send + Sync>,
        name: std::borrow::Cow<'static, str>,
        // description over and over takes up too much space in the
        // influx database, just ignore it for this application.
        _description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        attributes: Option<Arc<[opentelemetry_api::KeyValue]>>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            influxive,
            name,
            unit,
            attributes,
            _p: std::marker::PhantomData,
        })
    }

    fn report(&self, value: T, attributes: &[opentelemetry_api::KeyValue]) {
        let name = if let Some(unit) = &self.unit {
            format!("{}.{}", &self.name, unit.as_str())
        } else {
            self.name.to_string()
        };

        // otel metrics are largely a single measurement... so
        // just applying them to the generic "value" name in influx.
        let mut metric = Metric::new(std::time::SystemTime::now(), name).with_field("value", value);

        // everything else is a tag? would these be better as fields?
        // some kind of naming convention to pick between the two??
        for kv in attributes {
            metric = metric.with_tag(kv.key.to_string(), kv.value.to_string());
        }

        if let Some(attributes) = &self.attributes {
            for kv in attributes.iter() {
                metric = metric.with_tag(kv.key.to_string(), kv.value.to_string());
            }
        }

        self.influxive.write_metric(metric);
    }
}

impl<T: 'static + std::fmt::Display + Into<DataType> + Send + Sync>
    opentelemetry_api::metrics::SyncCounter<T> for InfluxiveUniMetric<T>
{
    fn add(&self, value: T, attributes: &[opentelemetry_api::KeyValue]) {
        self.report(value, attributes)
    }
}

impl<T: 'static + std::fmt::Display + Into<DataType> + Send + Sync>
    opentelemetry_api::metrics::SyncUpDownCounter<T> for InfluxiveUniMetric<T>
{
    fn add(&self, value: T, attributes: &[opentelemetry_api::KeyValue]) {
        self.report(value, attributes)
    }
}

impl<T: 'static + std::fmt::Display + Into<DataType> + Send + Sync>
    opentelemetry_api::metrics::SyncHistogram<T> for InfluxiveUniMetric<T>
{
    fn record(&self, value: T, attributes: &[opentelemetry_api::KeyValue]) {
        self.report(value, attributes)
    }
}

impl<T: 'static + std::fmt::Display + Into<DataType> + Send + Sync>
    opentelemetry_api::metrics::AsyncInstrument<T> for InfluxiveUniMetric<T>
{
    fn observe(&self, measurement: T, attributes: &[opentelemetry_api::KeyValue]) {
        self.report(measurement, attributes)
    }

    fn as_any(&self) -> Arc<dyn std::any::Any> {
        // this unwrap *should* be safe... so long as no one calls
        // Arc::into_inner() ever, which shouldn't be possible
        // because we're using trait objects everywhere??
        self.this.upgrade().unwrap()
    }
}

struct InfluxiveInstrumentProvider(
    Arc<dyn MetricWriter + 'static + Send + Sync>,
    Option<Arc<[opentelemetry_api::KeyValue]>>,
    Arc<ErasedMap>,
);

macro_rules! obs_body {
    ($s:ident, $t:ident, $n:ident, $d:ident, $u:ident, $c:ident,) => {{
        let g = $t::new(InfluxiveUniMetric::new(
            $s.0.clone(),
            $n,
            $d,
            $u,
            $s.1.clone(),
        ));

        let g2 = g.clone();
        $s.2.push(Box::new(move || {
            for cb in $c.iter() {
                cb(&g2);
            }
        }));

        Ok(g)
    }};
}

impl opentelemetry_api::metrics::InstrumentProvider for InfluxiveInstrumentProvider {
    fn u64_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::Counter<u64>> {
        Ok(opentelemetry_api::metrics::Counter::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn f64_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::Counter<f64>> {
        Ok(opentelemetry_api::metrics::Counter::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn u64_observable_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<u64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableCounter<u64>>
    {
        obs_body!(
            self,
            ObservableCounter,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn f64_observable_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<f64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableCounter<f64>>
    {
        obs_body!(
            self,
            ObservableCounter,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn i64_up_down_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::UpDownCounter<i64>> {
        Ok(opentelemetry_api::metrics::UpDownCounter::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn f64_up_down_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::UpDownCounter<f64>> {
        Ok(opentelemetry_api::metrics::UpDownCounter::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn i64_observable_up_down_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<i64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableUpDownCounter<i64>>
    {
        obs_body!(
            self,
            ObservableUpDownCounter,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn f64_observable_up_down_counter(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<f64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableUpDownCounter<f64>>
    {
        obs_body!(
            self,
            ObservableUpDownCounter,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn u64_observable_gauge(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<u64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableGauge<u64>> {
        obs_body!(
            self,
            ObservableGauge,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn i64_observable_gauge(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<i64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableGauge<i64>> {
        obs_body!(
            self,
            ObservableGauge,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn f64_observable_gauge(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
        callback_list: Vec<opentelemetry_api::metrics::Callback<f64>>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::ObservableGauge<f64>> {
        obs_body!(
            self,
            ObservableGauge,
            name,
            description,
            unit,
            callback_list,
        )
    }

    fn f64_histogram(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::Histogram<f64>> {
        Ok(opentelemetry_api::metrics::Histogram::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn u64_histogram(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::Histogram<u64>> {
        Ok(opentelemetry_api::metrics::Histogram::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn i64_histogram(
        &self,
        name: std::borrow::Cow<'static, str>,
        description: Option<std::borrow::Cow<'static, str>>,
        unit: Option<opentelemetry_api::metrics::Unit>,
    ) -> opentelemetry_api::metrics::Result<opentelemetry_api::metrics::Histogram<i64>> {
        Ok(opentelemetry_api::metrics::Histogram::new(
            InfluxiveUniMetric::new(self.0.clone(), name, description, unit, self.1.clone()),
        ))
    }

    fn register_callback(
        &self,
        _instruments: &[Arc<dyn std::any::Any>],
        callback: Box<dyn Fn(&dyn opentelemetry_api::metrics::Observer) + Send + Sync>,
    ) -> opentelemetry_api::metrics::Result<Box<dyn opentelemetry_api::metrics::CallbackRegistration>>
    {
        struct O;
        impl opentelemetry_api::metrics::Observer for O {
            fn observe_f64(
                &self,
                inst: &dyn opentelemetry_api::metrics::AsyncInstrument<f64>,
                measurement: f64,
                attrs: &[opentelemetry_api::KeyValue],
            ) {
                inst.observe(measurement, attrs);
            }

            fn observe_u64(
                &self,
                inst: &dyn opentelemetry_api::metrics::AsyncInstrument<u64>,
                measurement: u64,
                attrs: &[opentelemetry_api::KeyValue],
            ) {
                inst.observe(measurement, attrs);
            }

            fn observe_i64(
                &self,
                inst: &dyn opentelemetry_api::metrics::AsyncInstrument<i64>,
                measurement: i64,
                attrs: &[opentelemetry_api::KeyValue],
            ) {
                inst.observe(measurement, attrs);
            }
        }

        let id = self.2.push(Box::new(move || callback(&O)));

        struct Unregister(u64, Arc<ErasedMap>);

        impl opentelemetry_api::metrics::CallbackRegistration for Unregister {
            fn unregister(&mut self) -> opentelemetry_api::metrics::Result<()> {
                self.1.remove(self.0);
                Ok(())
            }
        }

        Ok(Box::new(Unregister(id, self.2.clone())))
    }
}

/// Influxive InfluxDB Meter Provider Configuration.
#[non_exhaustive]
pub struct InfluxiveMeterProviderConfig {
    /// Reporting interval for observable metrics.
    /// Set to `None` to disable periodic reporting
    /// (you'll need to call [InfluxiveMeterProvider::report] manually).
    /// Defaults to 30 seconds.
    pub observable_report_interval: Option<std::time::Duration>,
}

impl Default for InfluxiveMeterProviderConfig {
    fn default() -> Self {
        Self {
            observable_report_interval: Some(std::time::Duration::from_secs(30)),
        }
    }
}

impl InfluxiveMeterProviderConfig {
    /// Apply [InfluxiveMeterProviderConfig::observable_report_interval].
    pub fn with_observable_report_interval(
        mut self,
        observable_report_interval: Option<std::time::Duration>,
    ) -> Self {
        self.observable_report_interval = observable_report_interval;
        self
    }
}

/// Influxive InfluxDB Opentelemetry Meter Provider.
#[derive(Clone)]
pub struct InfluxiveMeterProvider(
    Arc<dyn MetricWriter + 'static + Send + Sync>,
    Arc<ErasedMap>,
);

impl InfluxiveMeterProvider {
    /// Construct a new InfluxiveMeterProvider instance with a given
    /// "Influxive" InfluxiveDB child process connector.
    pub fn new(
        config: InfluxiveMeterProviderConfig,
        influxive: Arc<dyn MetricWriter + 'static + Send + Sync>,
    ) -> Self {
        let strong = ErasedMap::new();

        if let Some(interval) = config.observable_report_interval {
            let weak = Arc::downgrade(&strong);
            tokio::task::spawn(async move {
                let mut interval = tokio::time::interval(interval);
                loop {
                    interval.tick().await;
                    if let Some(strong) = weak.upgrade() {
                        strong.invoke();
                    } else {
                        break;
                    }
                }
            });
        }

        Self(influxive, strong)
    }

    /// Manually report all observable metrics.
    pub fn report(&self) {
        self.1.invoke();
    }
}

impl opentelemetry_api::metrics::MeterProvider for InfluxiveMeterProvider {
    fn versioned_meter(
        &self,
        _name: impl Into<std::borrow::Cow<'static, str>>,
        _version: Option<impl Into<std::borrow::Cow<'static, str>>>,
        _schema_url: Option<impl Into<std::borrow::Cow<'static, str>>>,
        attributes: Option<Vec<opentelemetry_api::KeyValue>>,
    ) -> opentelemetry_api::metrics::Meter {
        let attributes: Option<Arc<[opentelemetry_api::KeyValue]>> =
            attributes.map(|a| a.into_boxed_slice().into());
        opentelemetry_api::metrics::Meter::new(Arc::new(InfluxiveInstrumentProvider(
            self.0.clone(),
            attributes,
            self.1.clone(),
        )))
    }
}

#[cfg(test)]
mod test;
