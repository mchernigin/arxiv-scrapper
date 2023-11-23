mod engine;
mod logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init("searxiv.log")?;

    let db = std::sync::Arc::new(tokio::sync::Mutex::new(
        arxiv_shared::db::DBConnection::new()?,
    ));

    println!("Building index...");
    let search = engine::SearchEngine::new(&db).await?;

    loop {
        let query = inquire::Text::new("Query:").prompt()?;

        if query == "exit" {
            break;
        }

        let start = std::time::Instant::now();
        let results = search.query(&query)?;
        let duration = start.elapsed();

        for (idx, &(_score, doc_address)) in results.iter().enumerate() {
            let doc_id = search.get_doc_id(doc_address).unwrap();
            let mut db = db.lock().await;
            let paper = db.get_paper(doc_id as i32).unwrap();
            println!("{:2}. {} ({})", idx + 1, paper.title, paper.url);
        }
        println!("This search took: {:?}", duration);
    }

    Ok(())
}
