use clap::Parser;

mod scraper;

#[derive(clap::Parser)]
#[command(version)]
struct Cli {
    /// Set start page to scraping
    #[arg(short, long, value_name = "PAGE", default_value_t = 0)]
    start_page: usize,

    /// Set maximum number of pages to scrape
    #[arg(short, long, value_name = "PAGE", default_value_t = 5)]
    max_pages: usize,

    /// Number of papers on page
    #[arg(short, long, value_name = "PAPERS", default_value_t = 25)]
    papers_per_page: usize,

    /// Output file
    #[arg(short, long, value_name = "FILE", default_value = "out.json")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    const BASE_URL: &str = "https://arxiv.org";
    let start_url: String = format!(
        "{}/search/advanced?advanced=&terms-0-operator=AND&terms-0-term=&terms-0-field=title&classification-computer_science=y&classification-physics_archives=all&classification-include_cross_list=include&date-filter_by=all_dates&date-year=&date-from_date=&date-to_date=&date-date_type=submitted_date&abstracts=show&size={}&order=-announced_date_first",
        BASE_URL,
        cli.papers_per_page
    );

    let scraper = scraper::Scraper::new();

    let mut pages = Vec::new();
    let mut current_url = start_url;
    for _ in 0..cli.max_pages {
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

    std::fs::write(cli.output, data)?;

    Ok(())
}
