//! Metrics handling for HTTP server.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Global handle to the Prometheus recorder so the HTTP server can render the
/// metrics endpoint without needing the handle passed through the CLI layer.
static METRICS_HANDLE: std::sync::OnceLock<PrometheusHandle> = std::sync::OnceLock::new();

/// Installs a Prometheus metrics recorder and returns a handle for rendering scrape output.
///
/// The handle is also stashed in a global so `serve()` can retrieve it later.
pub fn install_metrics_recorder(
) -> Result<PrometheusHandle, metrics_exporter_prometheus::BuildError> {
    let handle = PrometheusBuilder::new()
        .add_global_label("service", "padagonia")
        .install_recorder()?;
    let _ = METRICS_HANDLE.set(handle.clone());
    Ok(handle)
}

pub fn get_metrics_handle() -> Option<PrometheusHandle> {
    METRICS_HANDLE.get().cloned()
}
