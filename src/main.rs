fn main() {
    let settings = padagonia::app_config::Settings::load().expect("failed to load configuration");
    padagonia::app_config::init_tracing(settings.log_level());
    let _metrics_handle =
        padagonia::server::install_metrics_recorder().expect("failed to install metrics recorder");

    tracing::info!(
        listen_addr = %settings.listen_addr(),
        data_dir = %settings.data_dir().display(),
        "PADAGONIA starting"
    );

    padagonia::cli::run();
}
