#[derive(clap::Parser, Debug, Clone)]
#[command(version)]
pub struct Config {
    /// Set start page to scraping
    #[arg(short, long, value_name = "PAGE", default_value_t = 0)]
    pub start_page: usize,

    /// Set maximum number of pages to scrape
    #[arg(short = 'n', long, value_name = "PAGE", default_value_t = 1)]
    pub max_pages: usize,

    /// Number of papers on page
    #[arg(short, long, value_name = "PAPERS", default_value_t = 25)]
    pub papers_per_page: usize,

    /// Data directory
    #[arg(short, long, value_name = "FILE", default_value = "data")]
    pub data_dir: std::path::PathBuf,
}
