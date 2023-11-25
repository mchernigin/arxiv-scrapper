use clap::Parser;

mod config;
mod scraper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::parse();

    dotenvy::dotenv()?;

    let mut scraper = scraper::Scraper::new(cfg.clone())?;

    let start_url: String = format!(
        "https://arxiv.org/search/advanced?\
        advanced=&\
        terms-0-operator=AND&\
        terms-0-term=&\
        terms-0-field=title&\
        classification-computer_science=y&\
        classification-physics_archives=all&\
        classification-include_cross_list=include&\
        date-filter_by=all_dates&\
        date-year=&\
        date-from_date=&\
        date-to_date=&\
        date-date_type=submitted_date&\
        abstracts=show&\
        size={}&\
        order=-announced_date_first&\
        start={}",
        cfg.papers_per_page,
        cfg.start_page * cfg.papers_per_page
    );

    scraper.scrape(start_url).await?;

    println!(
        "Done: total number of papers in database: {}",
        scraper.get_total_papers().await?
    );

    Ok(())
}
