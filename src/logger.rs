pub fn init(filepath: &str) -> anyhow::Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Warn)
        .level_for("searxiv", log::LevelFilter::Trace)
        .chain(fern::log_file(filepath)?)
        .apply()?;

    Ok(())
}
