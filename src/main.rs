use clap::Parser;

mod config;
mod scraper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::parse();

    const BASE_URL: &str = "https://arxiv.org";
    let start_url: String = format!(
        "{}/search/advanced?advanced=&terms-0-operator=AND&terms-0-term=&terms-0-field=title&classification-computer_science=y&classification-physics_archives=all&classification-include_cross_list=include&date-filter_by=all_dates&date-year=&date-from_date=&date-to_date=&date-date_type=submitted_date&abstracts=show&size={}&order=-announced_date_first",
        BASE_URL,
        cfg.papers_per_page
    );

    let scraper = scraper::Scraper::new(cfg.clone());

    let mut pages = Vec::new();
    let mut current_url = start_url;
    for _ in 0..cfg.max_pages {
        let page = scraper.scrape_page(current_url.to_string()).await?;
        let next_page_url = page.next_page_url.clone();
        pages.push(page);

        if let Some(next_page_url) = next_page_url {
            current_url = next_page_url;
        } else {
            break;
        }
    }

    let data = serde_json::to_string_pretty::<Vec<scraper::Page>>(&pages)?;

    let mut data_file = cfg.data_dir;
    data_file.push("out.json");
    std::fs::write(data_file, data)?;

    Ok(())
}
