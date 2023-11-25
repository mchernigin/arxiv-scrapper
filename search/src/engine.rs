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

pub struct SearchEngine {
    schema: tantivy::schema::Schema,
    _index: Index,
    searcher: Searcher,
    query_parser: QueryParser,
}

impl SearchEngine {
    pub async fn new(db: &Arc<Mutex<DBConnection>>) -> anyhow::Result<Self> {
        let options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("searxiv-main")
                .set_index_option(IndexRecordOption::WithFreqs),
        );

        let mut schema_builder = Schema::builder();
        let id = schema_builder.add_u64_field("id", STORED);
        let url = schema_builder.add_text_field("url", STORED);
        let title = schema_builder.add_text_field("title", options.clone().set_stored());
        let description = schema_builder.add_text_field("description", options.clone());
        let body = schema_builder.add_text_field("body", options.clone());
        let schema = schema_builder.build();

        let directory = MmapDirectory::open(std::env::current_dir()?.join("tantivy-index"))?;
        let index = Index::create(
            directory,
            schema.clone(),
            tantivy::IndexSettings {
                sort_by_field: None,
                docstore_compression: Compressor::Zstd(tantivy::store::ZstdCompressor {
                    compression_level: Some(5),
                }),
                docstore_compress_dedicated_thread: true,
                docstore_blocksize: 100_000, // TODO: figure out not random value
            },
        )?;

        let stop_words = StopWordFilter::new(tantivy::tokenizer::Language::English).unwrap();
        let tokenizer = tantivy::tokenizer::TextAnalyzer::builder(
            tantivy::tokenizer::SimpleTokenizer::default(),
        )
        .filter(tantivy::tokenizer::LowerCaser)
        .filter(stop_words)
        .filter(tantivy::tokenizer::Stemmer::new(
            tantivy::tokenizer::Language::English,
        ))
        .build();
        index.tokenizers().register("searxiv-main", tokenizer);

        let mut index_writer = index.writer(100_000_000)?;

        {
            let mut db = db.lock().await;
            let papers = db.get_all_papers()?;

            for paper in papers {
                index_writer.add_document(doc!(
                    id => paper.id as u64,
                    url => paper.url,
                    title => paper.title,
                    description => paper.description,
                    body => paper.body,
                ))?;
            }
        }

        index_writer.commit()?;

        let reader = index.reader()?;
        let searcher = reader.searcher();
        let mut query_parser = QueryParser::for_index(&index, vec![title, description, body]);
        query_parser.set_field_fuzzy(title, true, 1, true);
        query_parser.set_field_fuzzy(description, true, 1, true);

        Ok(Self {
            schema,
            _index: index,
            searcher,
            query_parser,
        })
    }

    pub fn query(&self, query: &str) -> anyhow::Result<Vec<(Score, DocAddress)>> {
        let query = self.query_parser.parse_query(query)?;

        Ok(self.searcher.search(&query, &TopDocs::with_limit(10))?)
    }

    pub fn get_doc_id(&self, doc_address: DocAddress) -> Option<u64> {
        let retrieved_doc = self.searcher.doc(doc_address).ok()?;
        let id_field = self.schema.get_field("id").ok()?;
        retrieved_doc.get_first(id_field)?.as_u64()
    }
}
