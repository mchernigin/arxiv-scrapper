mod scraper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const PAPERS_PER_PAGE: usize = 25;
    let start_url: String = format!("https://arxiv.org/search/advanced?advanced=&terms-0-operator=AND&terms-0-term=&terms-0-field=title&classification-computer_science=y&classification-physics_archives=all&classification-include_cross_list=include&date-filter_by=all_dates&date-year=&date-from_date=&date-to_date=&date-date_type=submitted_date&abstracts=show&size={}&order=-announced_date_first", PAPERS_PER_PAGE);

    let scraper = scraper::Scraper::new();
    _ = scraper.scrape_page(start_url.to_string()).await?;

    Ok(())
}
