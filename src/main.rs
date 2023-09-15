use indicatif::ProgressIterator;

mod scraper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const START_URL: &str = "https://arxiv.org/search/advanced?advanced=&terms-0-operator=AND&terms-0-term=&terms-0-field=title&classification-computer_science=y&classification-physics_archives=all&classification-include_cross_list=include&date-filter_by=all_dates&date-year=&date-from_date=&date-to_date=&date-date_type=submitted_date&abstracts=show&size=100&order=-announced_date_first";

    let scraper = scraper::Scraper::new();
    let paper_links = scraper.scrape_page(START_URL.to_string()).await?;

    println!("Scrapping page 1");
    let papers_bar_style = indicatif::ProgressStyle::with_template(
        "[{elapsed_precise:.dim}] [{bar:50.cyan/blue}] {pos}/{len} ({eta})",
    )
    .unwrap()
    .progress_chars("##.");

    for paper_link in paper_links
        .into_iter()
        .progress_with_style(papers_bar_style)
        .with_finish(indicatif::ProgressFinish::Abandon)
    {
        println!("{:#?}", scraper.scrape_paper(paper_link).await?);
    }

    Ok(())
}
