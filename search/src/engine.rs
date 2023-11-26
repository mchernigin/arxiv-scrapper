use arxiv_shared::db::DBConnection;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::IndexRecordOption;
use tantivy::schema::*;
use tantivy::store::Compressor;
use tantivy::tokenizer::StopWordFilter;
use tantivy::{doc, Searcher};
use tantivy::{DocAddress, Index, Score};

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{CONFIG, SYMSPELL};

const TOKENIZER_MAIN: &str = "searxiv-main";

pub struct SearchEngine {
    schema: tantivy::schema::Schema,
    searcher: Searcher,
    query_parser: QueryParser,
}

impl SearchEngine {
    pub async fn new(db: &Arc<Mutex<DBConnection>>) -> anyhow::Result<Self> {
        let options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER_MAIN)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );

        let mut schema_builder = Schema::builder();
        let _id = schema_builder.add_u64_field("id", STORED);
        let _url = schema_builder.add_text_field("url", STORED);
        let title = schema_builder.add_text_field("title", options.clone().set_stored());
        let description = schema_builder.add_text_field("description", options.clone());
        let body = schema_builder.add_text_field("body", options.clone());
        let schema = schema_builder.build();

        let index = create_index(&schema, db).await?;

        let reader = index.reader()?;
        let searcher = reader.searcher();
        let mut query_parser = QueryParser::for_index(&index, vec![title, description, body]);
        query_parser.set_field_fuzzy(title, true, 1, true);
        query_parser.set_field_fuzzy(description, true, 1, true);

        Ok(Self {
            schema,
            searcher,
            query_parser,
        })
    }

    pub fn query(&self, query: String, limit: usize) -> anyhow::Result<Vec<(Score, DocAddress)>> {
        // NOTE: you can uncomment this... do it... but don't blame me later
        // let query = spellcheck_query(query);
        // let query = add_synonyms(query, 0);
        let query = self.query_parser.parse_query(&query)?;
        Ok(self.searcher.search(&query, &TopDocs::with_limit(limit))?)
    }

    pub fn get_doc_id(&self, doc_address: DocAddress) -> Option<u64> {
        let retrieved_doc = self.searcher.doc(doc_address).ok()?;
        let id_field = self.schema.get_field("id").ok()?;
        retrieved_doc.get_first(id_field)?.as_u64()
    }
}

async fn create_index(schema: &Schema, db: &Arc<Mutex<DBConnection>>) -> anyhow::Result<Index> {
    let index_dir = crate::config::get_cache_dir().join("index");
    let index_already_exists = index_dir.exists();
    let index = if index_already_exists {
        tracing::info!("Index dir {index_dir:?} alreay exist: opening existing index");
        Index::open_in_dir(index_dir)?
    } else {
        tracing::info!("Index dir {index_dir:?} does not exist: creating an index");
        std::fs::create_dir_all(index_dir.clone())?;
        Index::create(
            MmapDirectory::open(index_dir.clone())?,
            schema.clone(),
            tantivy::IndexSettings {
                sort_by_field: None,
                docstore_compression: Compressor::Zstd(tantivy::store::ZstdCompressor {
                    compression_level: CONFIG.index_zstd_compression_level,
                }),
                docstore_compress_dedicated_thread: true,
                docstore_blocksize: CONFIG.index_docstore_blocksize,
            },
        )?
    };

    index
        .tokenizers()
        .register(TOKENIZER_MAIN, create_tokenizer());

    if !index_already_exists {
        let mut index_writer = index.writer(CONFIG.index_writer_memory_budget)?;
        let mut db = db.lock().await;
        let papers = db.get_all_papers()?;

        for paper in papers {
            index_writer.add_document(doc!(
                schema.get_field("id")? => paper.id as u64,
                schema.get_field("url")? => paper.url,
                schema.get_field("title")? => paper.title,
                schema.get_field("description")? => paper.description,
                schema.get_field("body")? => paper.body,
            ))?;
        }
        index_writer.commit()?;
    }

    Ok(index)
}

fn create_tokenizer() -> tantivy::tokenizer::TextAnalyzer {
    let stop_words = StopWordFilter::new(tantivy::tokenizer::Language::English).unwrap();
    tantivy::tokenizer::TextAnalyzer::builder(tantivy::tokenizer::SimpleTokenizer::default())
        .filter(tantivy::tokenizer::LowerCaser)
        .filter(stop_words)
        .filter(tantivy::tokenizer::Stemmer::new(
            tantivy::tokenizer::Language::English,
        ))
        .build()
}

#[allow(dead_code)]
fn spellcheck_query(query: String) -> String {
    let words = query.split(' ').collect::<Vec<_>>();

    let mut words_witch_correction = Vec::new();

    for word in words.into_iter() {
        words_witch_correction.push(word.to_string());
        if let Some(suggestion) = SYMSPELL.lookup(word, symspell::Verbosity::Top, 2).first() {
            if suggestion.distance > 0 {
                words_witch_correction.push(suggestion.term.to_string());
            }
        }
    }

    words_witch_correction.join(" ")
}

#[allow(dead_code)]
fn add_synonyms(query: String, n: usize) -> String {
    let query_words = query
        .split(' ')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    let mut query_words_with_synonyms = query_words.clone();
    for word in query_words {
        let mut synonyms = thesaurus::synonyms(&word)
            .into_iter()
            .take(n)
            .collect::<Vec<_>>();
        query_words_with_synonyms.append(&mut synonyms);
    }

    query_words_with_synonyms.join(" ")
}
