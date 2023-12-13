use arxiv_shared::db::DBConnection;
use nalgebra::{DVector, RealField};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::IndexRecordOption;
use tantivy::schema::*;
use tantivy::store::Compressor;
use tantivy::tokenizer::StopWordFilter;
use tantivy::{doc, DocAddress, Index, Score, Searcher};

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{CONFIG, MODEL, SYMSPELL, SYNONYMS};

const TOKENIZER_MAIN: &str = "searxiv-main";

pub struct SearchEngine {
    schema: tantivy::schema::Schema,
    searcher: Searcher,
    index: Index,
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
        let _embediding = schema_builder.add_bytes_field("embedding", STORED);
        let title = schema_builder.add_text_field("title", options.clone().set_stored());
        let authors = schema_builder.add_text_field("authors", options.clone().set_stored());
        let description = schema_builder.add_text_field("description", options.clone());
        let body = schema_builder.add_text_field("body", options.clone());
        let schema = schema_builder.build();

        let index = create_index(&schema, db).await?;

        let reader = index.reader()?;
        let searcher = reader.searcher();
        let mut query_parser =
            QueryParser::for_index(&index, vec![title, authors, description, body]);

        query_parser.set_field_fuzzy(title, true, 1, true);
        query_parser.set_field_fuzzy(description, true, 1, true);

        query_parser.set_field_boost(title, 3.0);
        query_parser.set_field_boost(authors, 1.0);
        query_parser.set_field_boost(description, 1.0);
        query_parser.set_field_boost(body, 0.1);

        Ok(Self {
            schema,
            searcher,
            index,
            query_parser,
        })
    }

    pub async fn query(
        &self,
        query: String,
        _limit: usize,
    ) -> anyhow::Result<Vec<(Score, DocAddress)>> {
        // NOTE: we get query in double quotes if it contains more than 1 word
        let query = query.trim_matches('"').to_string();

        let query = spellcheck_query(query);
        let query = add_synonyms(query, 2);
        log::info!("Executing query {query:?}");

        let search_query = self.query_parser.parse_query(&query)?;
        let search_results = self
            .searcher
            .search(&search_query, &TopDocs::with_limit(100))?;

        self.bert_filter(query, search_results).await
    }

    pub async fn bert_filter(
        &self,
        query: String,
        top: Vec<(f32, DocAddress)>,
    ) -> anyhow::Result<Vec<(f32, DocAddress)>> {
        let model = &MODEL.lock().await;

        let query_embedding = model.encode(&[&query])?.first().unwrap().to_owned();
        let query_embedding = &DVector::from_vec(query_embedding);

        let mut new_top = top
            .into_iter()
            .map(|(score, doc_id)| -> anyhow::Result<(f32, DocAddress)> {
                let doc = self.searcher.doc(doc_id)?;
                let embdeding_bytes = doc
                    .get_first(self.schema.get_field("embedding")?)
                    .unwrap()
                    .as_bytes()
                    .unwrap();
                let paper_embedding = bincode::deserialize(embdeding_bytes)?;

                let similarity =
                    cosine_similarity(query_embedding, &DVector::from_vec(paper_embedding));

                Ok((score * similarity, doc_id))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        new_top.sort_by(|(score1, _), (score2, _)| score2.partial_cmp(score1).unwrap());

        let first_n = new_top.into_iter().take(CONFIG.max_results).collect();

        Ok(first_n)
    }

    pub fn get_doc_id(&self, doc_address: DocAddress) -> Option<u64> {
        let retrieved_doc = self.searcher.doc(doc_address).ok()?;
        let id_field = self.schema.get_field("id").ok()?;
        retrieved_doc.get_first(id_field)?.as_u64()
    }

    pub fn get_index_size(&self) -> anyhow::Result<u32> {
        let segments = self.index.searchable_segment_metas()?;
        Ok(segments
            .iter()
            .fold(0u32, |sum, segment| sum + segment.num_docs()))
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
        let model = &MODEL.lock().await;
        let mut db = db.lock().await;
        let papers = db.get_all_papers().await?;

        for paper in papers {
            let authors = db
                .get_paper_authors(paper.id)
                .await?
                .into_iter()
                .map(|a| a.name)
                .collect::<Vec<_>>()
                .join(" ");

            let title_and_abstract = format!("{}. {}", paper.title, paper.description);
            let sentences = title_and_abstract.split(". ").collect::<Vec<_>>();
            let output = model.encode(&sentences)?;
            let embedding = output.first().unwrap().to_owned();
            let embedding_bytes = bincode::serialize(&embedding).unwrap();

            index_writer.add_document(doc!(
                schema.get_field("id")? => paper.id as u64,
                schema.get_field("url")? => paper.url,
                schema.get_field("embedding")? => embedding_bytes,
                schema.get_field("title")? => paper.title,
                schema.get_field("authors")? => authors,
                schema.get_field("description")? => paper.description,
                schema.get_field("body")? => paper.body,
            ))?;
        }
        index_writer.commit()?;
    }

    Ok(index)
}

fn cosine_similarity<T: RealField>(a: &DVector<T>, b: &DVector<T>) -> T {
    let norm_a = a.norm();
    let norm_b = b.norm();
    let dot_product = a.dot(b);
    dot_product / (norm_a * norm_b)
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
    const CORRECTED_WORD_WEIGHT: f32 = 0.5;

    let mut words_witch_correction = Vec::new();

    query.split(' ').for_each(|word| {
        words_witch_correction.push(word.to_string());
        if let Some(suggestion) = SYMSPELL
            .lookup(word, symspell::Verbosity::Closest, 1)
            .first()
        {
            if suggestion.distance > 0 {
                log::info!("Correcting {word:?} to {:?}", suggestion.term);
                words_witch_correction
                    .push(format!("{}^{}", suggestion.term, CORRECTED_WORD_WEIGHT));
            }
        }
    });

    words_witch_correction.join(" ")
}

#[allow(dead_code)]
fn add_synonyms(query: String, n: usize) -> String {
    const SYNONYM_WEIGHT: f32 = 0.1;

    let mut query_words_with_synonyms = Vec::new();

    query.split(' ').for_each(|word| {
        query_words_with_synonyms.push(word.to_string());
        if let Some(found_synonyms) = SYNONYMS.get(word) {
            let needed_synonyms = found_synonyms.iter().take(n).collect::<Vec<_>>();
            log::info!("Found synonyms for {word:?}: {:?}", needed_synonyms);
            for synonym in needed_synonyms {
                query_words_with_synonyms.push(format!("{}^{}", synonym, SYNONYM_WEIGHT));
            }
        }
    });

    query_words_with_synonyms.join(" ")
}
