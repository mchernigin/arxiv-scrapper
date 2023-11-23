mod engine;
mod logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init("searxiv.log")?;

    let search = engine::Engine::new().await?;

    let results = search.query("hello")?;
    for (idx, &(_score, doc_address)) in results.iter().enumerate() {
        println!(
            "{:2}. {} ({_score})",
            idx + 1,
            search
                .get_doc(doc_address)?
                .0
                .get("title")
                .unwrap()
                .last()
                .unwrap()
                .as_text()
                .unwrap()
        );
    }

    Ok(())
}
