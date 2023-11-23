mod engine;
mod logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init("searxiv.log")?;

    let current_prefix = inquire::ui::Styled::new("~>").with_fg(inquire::ui::Color::DarkRed);
    let config = inquire::ui::RenderConfig::default().with_prompt_prefix(current_prefix);
    inquire::set_global_render_config(config);

    let search = engine::Engine::new().await?;

    loop {
        let query = inquire::Text::new("Query:").prompt()?;

        if query == "exit" {
            break;
        }

        let results = search.query(&query)?;
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
    }

    Ok(())
}
