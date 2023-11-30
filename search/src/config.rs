use std::path::PathBuf;

use etcetera::{app_strategy, AppStrategy, AppStrategyArgs};

use lazy_static::lazy_static;

lazy_static! {
    static ref STRATEGY: app_strategy::Xdg = app_strategy::Xdg::new(AppStrategyArgs {
        top_level_domain: "dev".to_string(),
        author: "mchernigin".to_string(),
        app_name: "searxiv".to_string(),
    })
    .unwrap();
    pub static ref CONFIG: Config = {
        let config_file = get_config_dir().join("searxiv.toml");
        if let Ok(config_contents) = std::fs::read_to_string(&config_file) {
            toml::from_str(&config_contents).unwrap_or_default()
        } else {
            let config = Config::default();
            if std::fs::create_dir_all(get_config_dir()).is_ok() {
                drop(std::fs::write(
                    config_file,
                    toml::to_string_pretty(&config).unwrap(),
                ));
            };
            config
        }
    };
    pub static ref SYMSPELL: symspell::SymSpell<symspell::AsciiStringStrategy> = {
        let mut spell = symspell::SymSpell::default();
        spell.load_dictionary("./search/dictionaries/LScD.txt", 0, 1, " ");
        spell.load_bigram_dictionary(
            "./search/dictionaries/frequency_bigramdictionary_en_243_342.txt",
            0,
            2,
            " ",
        );

        spell
    };
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
    pub cli_specific: CliConfig,
    pub server_specific: ServerConfig,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct CliConfig {
    pub prune: bool,
    pub max_results: usize,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct ServerConfig {
    pub max_results: usize,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            database_url: "postgres://postgres:password@localhost/arxiv".to_string(),
            index_zstd_compression_level: None,
            index_writer_memory_budget: 100_000_000,
            index_docstore_blocksize: 100_000, // TODO: figure out not random value
            cli_specific: CliConfig {
                prune: false,
                max_results: 10,
            },
            server_specific: ServerConfig {
                max_results: 10,
                port: 3000,
            },
        }
    }
}
