use std::path::PathBuf;

use tracing_subscriber::{fmt, fmt::format::FmtSpan, EnvFilter};

#[derive(Debug)]
pub struct DiagnosticsGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

pub fn init(log_dir: Option<PathBuf>) -> DiagnosticsGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,luma_forge_lib=debug"));

    if let Some(log_dir) = log_dir {
        let file_appender = tracing_appender::rolling::daily(log_dir, "luma-forge.log");
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        let subscriber = fmt()
            .with_env_filter(filter)
            .with_writer(writer)
            .with_ansi(false)
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_span_events(FmtSpan::CLOSE)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);

        return DiagnosticsGuard {
            _file_guard: Some(guard),
        };
    }

    let subscriber = fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    DiagnosticsGuard { _file_guard: None }
}
