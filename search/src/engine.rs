use arxiv_shared::db;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::store::Compressor;
use tantivy::{doc, Searcher};
use tantivy::{DocAddress, Index, Score};

use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Engine {
    schema: tantivy::schema::Schema,
    index: Index,
    searcher: Searcher,
    query_parser: QueryParser,
}

impl Engine {
    pub async fn new() -> anyhow::Result<Self> {
        let mut schema_builder = Schema::builder();
        let id = schema_builder.add_u64_field("id", STORED);
        let url = schema_builder.add_text_field("url", STORED);
        let title = schema_builder.add_text_field("title", TEXT | STORED);
        let description = schema_builder.add_text_field("description", TEXT | STORED);
        let body = schema_builder.add_text_field("body", TEXT);
        let schema = schema_builder.build();

        let directory = MmapDirectory::open(std::env::current_dir()?.join("tantivy-index"))?;
        let index = Index::create(
            directory,
            schema.clone(),
            tantivy::IndexSettings {
                sort_by_field: None,
                docstore_compression: Compressor::Zstd(tantivy::store::ZstdCompressor {
                    compression_level: None,
                }),
                docstore_compress_dedicated_thread: true,
                docstore_blocksize: 100_000, // TODO: figure out not random value
            },
        )?;

        let mut index_writer = index.writer(100_000_000)?;

        // NOTE: there is no need for Mutex for now, but I probably would want to build index in
        // parallel later
        let db = Arc::new(Mutex::new(db::DBConnection::new()?));
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
        let query_parser = QueryParser::for_index(&index, vec![title, description, body]);

        Ok(Self {
            schema,
            index,
            searcher,
            query_parser,
        })
    }

    pub fn query(&self, query: &str) -> anyhow::Result<Vec<(Score, DocAddress)>> {
        let query = self.query_parser.parse_query(query)?;

        Ok(self.searcher.search(&query, &TopDocs::with_limit(10))?)
    }

    pub fn get_doc(&self, doc_address: DocAddress) -> anyhow::Result<NamedFieldDocument> {
        Ok(self.schema.to_named_doc(&self.searcher.doc(doc_address)?))
    }
}
