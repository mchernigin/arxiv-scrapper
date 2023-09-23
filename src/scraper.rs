use crate::config;
use crate::db;

type Url = String;

pub struct Scraper<'a> {
    client: reqwest::Client,
    config: config::Config,
    db: &'a mut db::DBConnection,
    last_request: std::time::Instant,
    burst_count: u8,
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

impl<'a> Scraper<'a> {
    pub fn new(config: config::Config, db: &'a mut db::DBConnection) -> Scraper<'a> {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Googlebot")
                .build()
                .unwrap(),
            config,
            db,
            last_request: std::time::Instant::now(),
            burst_count: 0,
        }
    }

    async fn get(&mut self, url: Url) -> reqwest::Result<reqwest::Response> {
        const BURST_SIZE: u8 = 4;
        let now = std::time::Instant::now();

        let since_last_request = now - self.last_request;
        if self.burst_count >= BURST_SIZE {
            if since_last_request < std::time::Duration::from_secs(1) {
                std::thread::sleep(std::time::Duration::from_secs(1) - since_last_request);
            }
            self.burst_count = 0;
        }
        self.last_request = now;
        self.burst_count += 1;

        self.client.get(url).send().await
    }

    async fn get_dom(&mut self, url: Url) -> Result<scraper::Html> {
        let home_page = self.get(url).await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        Ok(dom)
    }

    async fn download_pdf(&mut self, url: Url) -> Result<bytes::Bytes> {
        let download_progress = self.config.progress_bars.add(
            indicatif::ProgressBar::new(self.config.max_pages as u64).with_style(
                indicatif::ProgressStyle::with_template(
                    "[{elapsed_precise:.dim}] [{bar:50.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("##."),
            ),
        );
        download_progress.enable_steady_tick(std::time::Duration::from_millis(100));

        let response = self.get(url).await?;
        let content = response.bytes().await?;

        Ok(content)
    }

    pub async fn scrape_paper(&mut self, url: Url) -> Result<()> {
        let dom = self.get_dom(url.clone()).await?;
        let submission = extract_submission_from_url(&url);

        let title = &select_title(&dom).await;
        let description = &select_description(&dom).await;

        let pdf_url = url.replace("abs", "pdf");
        let pdf = self.download_pdf(pdf_url).await?;
        let body = &body_from_pdf(&pdf).await;

        let paper_id = self.db.insert_paper(submission, title, description, body)?;

        let authors = select_authors(&dom).await?;
        _ = authors
            .iter()
            .map(|name| self.db.insert_author(name))
            .collect::<db::Result<Vec<_>>>()?
            .into_iter()
            .map(|author_id| self.db.set_paper_author(paper_id, author_id));

        let subjects = select_subjects(&dom).await?;
        _ = subjects
            .iter()
            .map(|name| self.db.insert_subject(name))
            .collect::<db::Result<Vec<_>>>()?
            .into_iter()
            .map(|subject_id| self.db.set_paper_category(paper_id, subject_id));

        Ok(())
    }

    pub async fn scrape_page(&mut self, url: Url) -> Result<Option<String>> {
        let home_page = self.get(url).await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        let paper_link_selector = scraper::Selector::parse(".list-title > a").unwrap();
        let paper_links = dom
            .select(&paper_link_selector)
            .map(|l| l.value().attr("href").unwrap().to_string())
            .collect::<Vec<Url>>();

        let papers_progress = self.config.progress_bars.add(
            indicatif::ProgressBar::new(paper_links.len() as u64).with_style(
                indicatif::ProgressStyle::with_template(
                    "[{elapsed_precise:.dim}] [{bar:50.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("##."),
            ),
        );
        papers_progress.enable_steady_tick(std::time::Duration::from_millis(100));

        let mut papers = Vec::new();
        for paper_link in paper_links {
            let export_link = paper_link.replace("arxiv.org", "export.arxiv.org");
            let submission = extract_submission_from_url(&export_link);
            if !self.db.paper_exists(submission)? {
                papers.push(self.scrape_paper(export_link).await?);
            }
            papers_progress.inc(1);
        }

        let next_page_selector = scraper::Selector::parse("a.pagination-next").unwrap();
        let mut next_page_url = None;
        if let Some(next_page_href) = dom.select(&next_page_selector).next() {
            let mut next_page = "https://arxiv.org".to_string();
            let next_page_href = next_page_href.value().attr("href").unwrap();
            next_page.push_str(next_page_href);

            next_page_url = Some(next_page);
        }

        Ok(next_page_url)
    }
}

fn extract_submission_from_url(url: &Url) -> &str {
    url.split('/').last().unwrap()
}

async fn select_title(dom: &scraper::Html) -> String {
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

async fn select_description(dom: &scraper::Html) -> String {
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

async fn select_authors(dom: &scraper::Html) -> Result<Vec<String>> {
    let authors_selector = scraper::Selector::parse(".authors > a").unwrap();
    let authors_elements = dom.select(&authors_selector).collect::<Vec<_>>();
    let authors = authors_elements
        .iter()
        .map(|a| a.text().collect::<String>())
        .collect::<Vec<_>>();

    Ok(authors)
}

async fn select_subjects(dom: &scraper::Html) -> Result<Vec<String>> {
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

async fn body_from_pdf(pdf: &bytes::Bytes) -> String {
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
