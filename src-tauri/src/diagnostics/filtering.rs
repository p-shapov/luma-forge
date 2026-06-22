use logforth::filter::FilterResult;
use logforth::record::{FilterCriteria, Level, LevelFilter};

#[derive(Debug)]
pub(super) struct AppLogFilter;

impl logforth::Filter for AppLogFilter {
    fn enabled(
        &self,
        criteria: &FilterCriteria,
        _: &[Box<dyn logforth::Diagnostic>],
    ) -> FilterResult {
        if !LevelFilter::MoreSevereEqual(Level::Info).test(criteria.level()) {
            return FilterResult::Reject;
        }

        if is_app_log_target(criteria.target()) {
            FilterResult::Neutral
        } else {
            FilterResult::Reject
        }
    }
}

fn is_app_log_target(target: &str) -> bool {
    matches!(target, "luma_forge" | "luma_forge_lib")
        || target.starts_with("luma_forge::")
        || target.starts_with("luma_forge_lib::")
}
