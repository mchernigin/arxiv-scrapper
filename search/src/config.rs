use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

use figment::{
    providers::{Env, Format, Toml},
    Error, Figment, Metadata, Profile, Provider,
};

use etcetera::{app_strategy, AppStrategy, AppStrategyArgs};
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType,
};

use lazy_static::lazy_static;

lazy_static! {
    static ref STRATEGY: app_strategy::Xdg = app_strategy::Xdg::new(AppStrategyArgs {
        top_level_domain: "dev".to_string(),
        author: "mchernigin".to_string(),
        app_name: "searxiv".to_string(),
    })
    .unwrap();
    pub static ref CONFIG: Config = Figment::from(Config::default())
        .merge(Toml::file(get_config_dir().join("searxiv.toml").to_str().unwrap()))
        .merge(Env::prefixed("SEARXIV_"))
        .extract()
        .unwrap();
    pub static ref SYMSPELL: symspell::SymSpell<symspell::AsciiStringStrategy> = {
        let mut spell = symspell::SymSpell::default();
        // TODO: store dictionaries in XDG_DATA_HOME and download them if there is none
        spell.load_dictionary(&format!("{}/LScD.txt", &CONFIG.dictionaries_path), 0, 1, " ");
        spell.load_bigram_dictionary(
            &format!("{}/FrequencyBigramdictionary.txt", &CONFIG.dictionaries_path),
            0,
            2,
            " ",
        );

        spell
    };
    pub static ref SYNONYMS: HashMap<String, Vec<String>> = {
        let mut synonyms = HashMap::new();
        println!("{}", format!("{}/WordnetSynonyms.txt", &CONFIG.dictionaries_path));
        let mut csv_reader =
            csv::Reader::from_path(&format!("{}/WordnetSynonyms.txt", &CONFIG.dictionaries_path)).unwrap();
        for result in csv_reader.records() {
            let record = result.unwrap();
            let word = record.get(0).unwrap();
            let word_synonyms = record.get(1).unwrap().split(';').map(|s| s.to_string()).collect();

            synonyms.insert(word.to_string(), word_synonyms);
        }

        synonyms
    };
    pub static ref MODEL: Arc<Mutex<SentenceEmbeddingsModel>> =
        Arc::new(Mutex::new(SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL12V2)
        .create_model().unwrap()));
}

pub fn get_cache_dir() -> PathBuf {
    STRATEGY.cache_dir()
}

pub fn get_config_dir() -> PathBuf {
    STRATEGY.config_dir()
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Config {
    pub database_url: String,
    pub index_zstd_compression_level: Option<i32>,
    pub index_docstore_blocksize: usize,
    pub index_writer_memory_budget: usize,
    pub max_results: usize,
    pub dictionaries_path: String,
    pub cli_specific: CliConfig,
    pub server_specific: ServerConfig,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct CliConfig {
    pub prune: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct ServerConfig {
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            database_url: "postgres://postgres:password@localhost/arxiv".to_string(),
            index_zstd_compression_level: None,
            index_writer_memory_budget: 100_000_000,
            index_docstore_blocksize: 100_000, // TODO: figure out not random value
            max_results: 10,
            dictionaries_path: "./search/dictionaries".to_string(),
            cli_specific: CliConfig { prune: false },
            server_specific: ServerConfig { port: 1818 },
        }
    }
}

use figment::value::{Dict, Map};

impl Provider for Config {
    fn metadata(&self) -> Metadata {
        Metadata::named("Searxiv Config")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        figment::providers::Serialized::defaults(Config::default()).data()
    }
}
