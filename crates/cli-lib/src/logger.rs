pub(crate) fn init() -> Result<(), log::SetLoggerError> {
    fern::Dispatch::new()
        .format(|out, message, record| out.finish(format_args!("[{}] {}", record.level(), message)))
        .level(
            std::env::var("SQRUFF_LOG")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(log::LevelFilter::Off),
        )
        .chain(std::io::stderr())
        .apply()
}
