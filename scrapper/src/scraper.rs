use crate::config;
use arxiv_shared::db;

use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

type Url = String;

pub struct Scraper {
    client: reqwest::Client,
    config: config::Config,
    db: Arc<Mutex<db::DBConnection>>,
    last_request: Arc<Mutex<std::time::Instant>>,
    burst_count: Arc<Mutex<u8>>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("network error")]
    Network(#[from] reqwest::Error),

    #[error("file error")]
    File(#[from] std::io::Error),

    #[error("database error")]
    Database(#[from] db::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

type SharedProgress = Arc<Mutex<indicatif::ProgressBar>>;

impl Scraper {
    pub fn new(config: config::Config) -> Result<Scraper> {
        let client = reqwest::Client::builder()
            .user_agent("Googlebot")
            .build()
            .unwrap();
        let db = Arc::new(Mutex::new(db::DBConnection::new()?));
        let last_request = Arc::new(Mutex::new(std::time::Instant::now()));
        let burst_count = Arc::new(Mutex::new(0));

        Ok(Self {
            client,
            config,
            db,
            last_request,
            burst_count,
        })
    }

    pub async fn get_total_papers(&self) -> Result<i64> {
        self.db.lock().await.count_papers().map_err(|e| e.into())
    }

    async fn get(&self, url: &Url) -> reqwest::Result<reqwest::Response> {
        const BURST_SIZE: u8 = 4;
        let mut burst_count = self.burst_count.lock().await;
        let mut last_request = self.last_request.lock().await;

        let now = std::time::Instant::now();

        let since_last_request = now - *last_request;
        if *burst_count >= BURST_SIZE {
            if since_last_request < std::time::Duration::from_secs(1) {
                std::thread::sleep(std::time::Duration::from_secs(1) - since_last_request);
            }
            *burst_count = 0;
        }
        *last_request = now;
        *burst_count += 1;

        drop(burst_count);
        drop(last_request);

        let mut backoff = std::time::Duration::from_secs(1);
        loop {
            log::trace!("Reqwest: GET {url:?}");
            let response = self.client.get(url).send().await;
            if response.is_err() {
                // std::thread::sleep(backoff); // TODO
                backoff *= 2;
                continue;
            }

            return response;
        }
    }

    async fn get_dom(&self, url: Url) -> Result<scraper::Html> {
        let response = self.get(&url).await?;
        let body = response.text().await?;
        let dom = scraper::Html::parse_document(&body);

        Ok(dom)
    }

    async fn download_pdf(&self, url: Url) -> Result<glib::Bytes> {
        let response = self.get(&url).await?;
        Ok(glib::Bytes::from_owned(response.bytes().await?))
    }

    pub async fn scrape_paper(&self, url: Url, sp: &SharedProgress) -> Result<()> {
        let dom = self.get_dom(url.clone()).await?;

        let title = &select_title(&dom);
        let description = &select_description(&dom);

        let pdf_url = url.replace("abs", "pdf");
        let pdf_bytes = self.download_pdf(pdf_url).await?;
        let body = &body_from_pdf(&pdf_bytes);

        if body.is_empty() {
            log::warn!("PDF: empty body {url:?}")
        }

        let mut db = self.db.lock().await;

        // TODO: https://stackoverflow.com/questions/75939019/transactions-in-rust-diesel
        let paper_id = db.insert_paper(&url, title, description, body)?;

        let authors = select_authors(&dom)?;
        _ = authors
            .iter()
            .map(|name| db.insert_author(name))
            .collect::<db::Result<Vec<_>>>()?
            .into_iter()
            .map(|author_id| db.set_paper_author(paper_id, author_id))
            .collect::<db::Result<Vec<_>>>()?;

        let subjects = select_subjects(&dom)?;
        _ = subjects
            .iter()
            .map(|name| db.insert_subject(name))
            .collect::<db::Result<Vec<_>>>()?
            .into_iter()
            .map(|subject_id| db.set_paper_category(paper_id, subject_id))
            .collect::<db::Result<Vec<_>>>()?;

        sp.lock().await.inc(1);

        Ok(())
    }

    pub async fn scrape_page(&mut self, url: Url) -> Result<(Vec<Url>, Option<String>)> {
        let home_page = self.get(&url).await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        let paper_link_selector = scraper::Selector::parse(".list-title > a").unwrap();
        let paper_links = dom
            .select(&paper_link_selector)
            .map(|l| l.value().attr("href").unwrap().to_string())
            .collect::<Vec<Url>>();

        let next_page_selector = scraper::Selector::parse("a.pagination-next").unwrap();
        let mut next_page_url = None;
        if let Some(next_page_href) = dom.select(&next_page_selector).next() {
            let mut next_page = "https://arxiv.org".to_string();
            let next_page_href = next_page_href.value().attr("href").unwrap();
            next_page.push_str(next_page_href);

            next_page_url = Some(next_page);
        }

        Ok((paper_links, next_page_url))
    }

    pub async fn scrape(&mut self, start_url: Url) -> Result<()> {
        let pages_progress = indicatif::ProgressBar::new(self.config.max_pages as u64)
            .with_style(
                indicatif::ProgressStyle::with_template(
                    "{elapsed_precise:.dim} {bar:50.cyan/blue} {pos}/{len}",
                )
                .unwrap(),
            )
            .with_message("Scrapping pages...");
        pages_progress.enable_steady_tick(std::time::Duration::from_millis(100));

        pages_progress.println(format!(
            "{} Searching for new papers...",
            console::style("[1/2]").bold().dim()
        ));

        let mut paper_urls = Vec::new();
        let mut current_url = start_url;
        for _ in 0..self.config.max_pages {
            let (page_paper_urls, next_page_url) =
                self.scrape_page(current_url.to_string()).await?;
            paper_urls.extend(page_paper_urls.into_iter());

            if let Some(next_page_url) = next_page_url {
                current_url = next_page_url;
            } else {
                break;
            }
            pages_progress.inc(1);
        }

        let mut paper_urls_to_download = Vec::new();
        for paper_url in paper_urls {
            let export_url = paper_url.replace("arxiv.org", "export.arxiv.org");
            if !self.db.lock().await.paper_exists(&export_url)? {
                paper_urls_to_download.push(export_url);
            }
        }

        drop(pages_progress);

        println!(
            "{} Scrapping {} papers...",
            console::style("[2/2]").bold().dim(),
            paper_urls_to_download.len()
        );

        let total_progress = indicatif::ProgressBar::new(paper_urls_to_download.len() as u64)
            .with_style(
                indicatif::ProgressStyle::with_template(
                    "{elapsed_precise:.dim} {bar:50.cyan/blue} {pos}/{len}",
                )
                .unwrap(),
            );
        total_progress.enable_steady_tick(std::time::Duration::from_millis(100));

        let amtp = Arc::new(Mutex::new(total_progress));

        let paper_futures = paper_urls_to_download
            .into_iter()
            .map(|url| self.scrape_paper(url, &amtp));
        let stream = futures::stream::iter(paper_futures)
            .buffer_unordered(25)
            .collect::<Vec<_>>();

        stream.await.into_iter().collect::<Result<Vec<_>>>()?;

        Ok(())
    }
}

fn select_title(dom: &scraper::Html) -> String {
    let title_selector = scraper::Selector::parse("h1.title").unwrap();
    dom.select(&title_selector)
        .next()
        .map(|el| {
            el.text()
                .collect::<String>()
                .trim()
                .trim_start_matches("Title:")
                .trim_start()
                .to_string()
        })
        .unwrap_or_default()
}

fn select_description(dom: &scraper::Html) -> String {
    let description_selector = scraper::Selector::parse("blockquote.abstract").unwrap();
    dom.select(&description_selector)
        .next()
        .map(|el| {
            el.text()
                .collect::<String>()
                .trim()
                .trim_start_matches("Abstract:")
                .trim_start()
                .replace('\n', " ")
                .to_string()
        })
        .unwrap_or_default()
}

fn select_authors(dom: &scraper::Html) -> Result<Vec<String>> {
    let authors_selector = scraper::Selector::parse(".authors > a").unwrap();
    let authors_elements = dom.select(&authors_selector).collect::<Vec<_>>();
    let authors = authors_elements
        .iter()
        .map(|a| a.text().collect::<String>())
        .collect::<Vec<_>>();

    Ok(authors)
}

fn select_subjects(dom: &scraper::Html) -> Result<Vec<String>> {
    let subjects_selector = scraper::Selector::parse("td.subjects").unwrap();
    let subjects = dom
        .select(&subjects_selector)
        .next()
        .map(|s| {
            s.text()
                .collect::<String>()
                .split(';')
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(subjects)
}

fn body_from_pdf(bytes: &glib::Bytes) -> String {
    let mut body = String::new();
    if let Ok(pdf) = poppler::Document::from_bytes(bytes, None) {
        let n = pdf.n_pages();
        for i in 0..n {
            if let Some(text) = pdf.page(i).and_then(|page| page.text()) {
                body.push_str(text.as_str());
                body.push(' ');
            }
        }
    }

    fix_line_breaks(body)
}

fn fix_line_breaks(text: String) -> String {
    let rg = regex::Regex::new(r"(\w)-\n(\w)").unwrap(); // TODO: handle spaces
    rg.replace_all(&text, "$1$2").to_string()
}
