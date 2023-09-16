use indicatif::ProgressIterator;

type Url = String;

#[derive(Debug, serde::Serialize)]
pub struct Page {
    pub papers: Vec<Paper>,
    pub next_page_url: Option<Url>,
}

#[derive(Debug, serde::Serialize)]
pub struct Paper {
    title: String,
    authors: Vec<String>,
    description: String,
    subjects: Vec<String>,
}

#[derive(Debug)]
pub struct Scraper {
    client: reqwest::Client,
}

impl Scraper {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Googlebot")
                .build()
                .unwrap(),
        }
    }

    async fn get_dom(&self, url: Url) -> reqwest::Result<scraper::Html> {
        let home_page = self.client.get(url).send().await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        Ok(dom)
    }

    pub async fn scrape_paper(&self, url: Url) -> reqwest::Result<Paper> {
        let dom = self.get_dom(url).await?;

        let title_selector = scraper::Selector::parse("h1.title").unwrap();
        let title_element = dom.select(&title_selector).next().unwrap();
        let title = title_element.text().last().unwrap().to_string();

        let authors_selector = scraper::Selector::parse(".authors > a").unwrap();
        let authors_elements = dom.select(&authors_selector).collect::<Vec<_>>();
        let authors = authors_elements
            .iter()
            .map(|a| a.text().collect::<String>())
            .collect::<Vec<_>>();

        let description_selector = scraper::Selector::parse("blockquote.abstract").unwrap();
        let description_element = dom.select(&description_selector).next().unwrap();
        let description = description_element
            .text()
            .collect::<String>()
            .trim()
            .trim_start_matches("Abstract:")
            .trim_start()
            .replace("\n", " ")
            .to_string();

        let subjects_selector = scraper::Selector::parse("td.subjects").unwrap();
        let subjects_element = dom.select(&subjects_selector).next().unwrap();
        let subjects = subjects_element
            .text()
            .collect::<String>()
            .split(';')
            .map(|x| x.trim().to_string())
            .collect();

        Ok(Paper {
            title,
            authors,
            description,
            subjects,
        })
    }

    pub async fn scrape_page(&self, url: Url) -> reqwest::Result<Page> {
        let home_page = self.client.get(url).send().await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        let paper_link_selector = scraper::Selector::parse(".list-title > a").unwrap();
        let paper_links = dom
            .select(&paper_link_selector)
            .map(|l| l.value().attr("href").unwrap().to_string())
            .collect::<Vec<Url>>();

        let papers_bar_style = indicatif::ProgressStyle::with_template(
            "[{elapsed_precise:.dim}] [{bar:50.cyan/blue}] {pos}/{len} ({eta})",
        )
        .unwrap()
        .progress_chars("##.");

        let mut papers = Vec::new();
        for paper_link in paper_links
            .into_iter()
            .progress_with_style(papers_bar_style)
            .with_finish(indicatif::ProgressFinish::Abandon)
        {
            papers.push(self.scrape_paper(paper_link).await?);
        }

        let next_page_selector = scraper::Selector::parse("a.pagination-next").unwrap();
        let mut next_page_url = None;
        if let Some(next_page_href) = dom.select(&next_page_selector).next() {
            let mut next_page = "https://arxiv.org".to_string();
            let next_page_href = next_page_href.value().attr("href").unwrap().to_string();
            next_page.push_str(&next_page_href);

            next_page_url = Some(next_page);
        }

        Ok(Page {
            papers,
            next_page_url,
        })
    }
}
