use clap::Parser;

mod config;
mod db;
mod models;
mod schema;
mod scraper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::parse();

    let mut db = db::DBConnection::new()?;
    let papers = db.get_all_papers()?;
    println!("{:#?}", papers);
    let mut scraper = scraper::Scraper::new(cfg.clone(), db);

    let start_url: String = format!(
        "https://arxiv.org/search/advanced?advanced=&terms-0-operator=AND&terms-0-term=&terms-0-field=title&classification-computer_science=y&classification-physics_archives=all&classification-include_cross_list=include&date-filter_by=all_dates&date-year=&date-from_date=&date-to_date=&date-date_type=submitted_date&abstracts=show&size={}&order=announced_date_first",
        cfg.papers_per_page
    );

    let pages_progress = cfg
        .progress_bars
        .add(
            indicatif::ProgressBar::new(cfg.max_pages as u64).with_style(
                indicatif::ProgressStyle::with_template(
                    "[{elapsed_precise:.dim}] [{bar:50.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("##."),
            ),
        )
        .with_finish(indicatif::ProgressFinish::Abandon);
    pages_progress.enable_steady_tick(std::time::Duration::from_millis(100));

    let mut current_url = start_url;
    for _ in 0..cfg.max_pages {
        pages_progress.inc(0);
        let next_page_url = scraper.scrape_page(current_url.to_string()).await?;

        if let Some(next_page_url) = next_page_url {
            current_url = next_page_url;
        } else {
            break;
        }
        pages_progress.inc(1);
    }

    Ok(())
}
