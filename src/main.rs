mod scraper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const PAPERS_PER_PAGE: usize = 25;
    const BASE_URL: &str = "https://arxiv.org";
    let start_url: String = format!("{}/search/advanced?advanced=&terms-0-operator=AND&terms-0-term=&terms-0-field=title&classification-computer_science=y&classification-physics_archives=all&classification-include_cross_list=include&date-filter_by=all_dates&date-year=&date-from_date=&date-to_date=&date-date_type=submitted_date&abstracts=show&size={}&order=-announced_date_first", BASE_URL, PAPERS_PER_PAGE);

    let scraper = scraper::Scraper::new();

    const MAX_PAGES: usize = 2;

    let mut pages = Vec::new();

    let mut current_url = start_url;
    for _ in 0..MAX_PAGES {
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

    const OUTPUT: &str = "./out.json";
    std::fs::write(OUTPUT, data)?;

    Ok(())
}
