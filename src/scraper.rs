#[derive(Debug)]
pub struct Paper {
    title: String,
    authors: Vec<String>,
    description: String,
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

    async fn get_dom(&self, url: String) -> reqwest::Result<scraper::Html> {
        let home_page = self.client.get(url).send().await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        Ok(dom)
    }

    pub async fn scrape_paper(&self, url: String) -> reqwest::Result<Paper> {
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

        Ok(Paper {
            title,
            authors,
            description,
        })
    }

    pub async fn scrape_page(&self, url: &str) -> reqwest::Result<Vec<String>> {
        let home_page = self.client.get(url).send().await?;
        let body = home_page.text().await?;
        let dom = scraper::Html::parse_document(&body);

        let paper_link_selector = scraper::Selector::parse(".list-title > a").unwrap();
        let paper_links = dom
            .select(&paper_link_selector)
            .map(|l| l.value().attr("href").unwrap().to_string())
            .collect();

        Ok(paper_links)
    }
}
