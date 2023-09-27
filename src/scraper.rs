use crate::config;
use crate::db;

use futures_util::StreamExt;
use std::sync::{Arc, Mutex};

type Url = String;

pub struct Scraper {
    client: reqwest::Client,
    config: config::Config,
    db: Arc<Mutex<db::DBConnection>>,
    last_request: Arc<Mutex<std::time::Instant>>,
    burst_count: Arc<Mutex<u8>>,
    pub progress_bars: indicatif::MultiProgress,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("network error")]
    NetworkError(#[from] reqwest::Error),

    #[error("file error")]
    FileError(#[from] std::io::Error),

    #[error("database error")]
    DatabaseError(#[from] db::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Scraper {
    pub fn new(config: config::Config) -> Result<Scraper> {
        let client = reqwest::Client::builder()
            .user_agent("Googlebot")
            .build()
            .unwrap();
        let db = Arc::new(Mutex::new(db::DBConnection::new()?));
        let last_request = Arc::new(Mutex::new(std::time::Instant::now()));
        let burst_count = Arc::new(Mutex::new(0));
        let progress_bars = indicatif::MultiProgress::new();
        progress_bars.set_move_cursor(true);

        Ok(Self {
            client,
            config,
            db,
            last_request,
            burst_count,
            progress_bars,
        })
    }

    pub fn get_total_papers(&self) -> Result<i64> {
        self.db.lock().unwrap().count_papers().map_err(|e| e.into())
    }

    async fn get(&self, url: Url) -> reqwest::Result<reqwest::Response> {
        const BURST_SIZE: u8 = 4;
        let mut burst_count = self.burst_count.lock().unwrap();
        let mut last_request = self.last_request.lock().unwrap();

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
            let response = self.client.get(&url).send().await;
            if response.is_err() {
                // std::thread::sleep(backoff); // TODO
                backoff *= 2;
                continue;
            }

            return response;
        }
    }

    async fn get_dom(&self, url: Url) -> Result<scraper::Html> {
        let response = self.get(url).await?;
        let body = response.text().await?;
        let dom = scraper::Html::parse_document(&body);

        Ok(dom)
    }

    async fn download_pdf(&self, url: Url) -> Result<bytes::Bytes> {
        let response = self.get(url).await?;
        let total_size = response.content_length().unwrap_or(std::u64::MAX);

        let download_progress = self.progress_bars.add(
            indicatif::ProgressBar::new(total_size).with_style(
                indicatif::ProgressStyle::with_template(
                    "{bar:50.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
                )
                .unwrap(),
            ),
        );
        download_progress.enable_steady_tick(std::time::Duration::from_millis(100));

        let mut stream = response.bytes_stream();
        let mut downloaded = 0;
        let mut chunks = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
            download_progress.set_position(new);
            chunks.push(chunk);
        }

        Ok(bytes::Bytes::from(chunks.concat()))
    }

    pub async fn scrape_paper(&self, url: Url) -> Result<()> {
        let dom = self.get_dom(url.clone()).await?;
        let submission = extract_submission_from_url(&url);

        let title = &select_title(&dom);
        let description = &select_description(&dom);

        let pdf_url = url.replace("abs", "pdf");
        let pdf = self.download_pdf(pdf_url).await?;
        let body = &body_from_pdf(&pdf);

        let mut db = self.db.lock().unwrap();

        let paper_id = db.insert_paper(submission, title, description, body)?;

        let authors = select_authors(&dom)?;
        _ = authors
            .iter()
            .map(|name| db.insert_author(name))
            .collect::<db::Result<Vec<_>>>()?
            .into_iter()
            .map(|author_id| db.set_paper_author(paper_id, author_id));

        let subjects = select_subjects(&dom)?;
        _ = subjects
            .iter()
            .map(|name| db.insert_subject(name))
            .collect::<db::Result<Vec<_>>>()?
            .into_iter()
            .map(|subject_id| db.set_paper_category(paper_id, subject_id));

        Ok(())
    }

    pub async fn scrape_page(&mut self, url: Url) -> Result<(Vec<Url>, Option<String>)> {
        let home_page = self.get(url).await?;
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
            "{} Scrappping pages...",
            console::style("[1/2]").bold().dim()
        ));

        let mut paper_links = Vec::new();
        let mut current_url = start_url;
        for _ in 0..self.config.max_pages {
            let (page_paper_links, next_page_url) =
                self.scrape_page(current_url.to_string()).await?;
            paper_links.extend(page_paper_links.into_iter());

            if let Some(next_page_url) = next_page_url {
                current_url = next_page_url;
            } else {
                break;
            }
            pages_progress.inc(1);
        }

        let mut download_count = 0;
        let mut paper_futures = Vec::new();
        for paper_link in paper_links {
            let export_link = paper_link.replace("arxiv.org", "export.arxiv.org");
            let submission = extract_submission_from_url(&export_link);
            if !!!self.db.lock().unwrap().paper_exists(submission)? {
                download_count += 1;
                paper_futures.push(self.scrape_paper(export_link));
            }
        }

        drop(pages_progress);

        println!(
            "{} Downloading {download_count} pages...",
            console::style("[2/2]").bold().dim()
        );

        futures::future::try_join_all(paper_futures).await?;

        Ok(())
    }
}

fn extract_submission_from_url(url: &Url) -> &str {
    url.split('/').last().unwrap()
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

fn body_from_pdf(pdf: &bytes::Bytes) -> String {
    let mut body = String::new();
    if let Ok(document) = lopdf::Document::load_mem(pdf) {
        let pages = document.get_pages();
        for (i, _) in pages.iter().enumerate() {
            let page_number = (i + 1) as u32;
            let page_body = document.extract_text(&[page_number]);
            body.push_str(&page_body.unwrap_or_default());
        }
    }

    fix_line_breaks(body)
}

fn fix_line_breaks(text: String) -> String {
    let rg = regex::Regex::new(r"(\w)- (\w)").unwrap();
    rg.replace_all(&text, "$1$2").to_string()
}
