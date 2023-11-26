use utoipa::OpenApi;
use utoipa_rapidoc::RapiDoc;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::CONFIG;

#[derive(utoipa::OpenApi)]
#[openapi(
        paths(searxiv::root, searxiv::search),
        components(
            schemas(searxiv::PaperInfo)
        ),
        tags(
            (name = "searxiv", description = "Search through pages in arxiv.org")
        )
    )]
struct ApiDoc;

pub async fn run_server() -> anyhow::Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::INFO)
        .init();

    let db = std::sync::Arc::new(tokio::sync::Mutex::new(
        arxiv_shared::db::DBConnection::new(&CONFIG.database_url)?,
    ));
    let engine = Mutex::new(crate::engine::SearchEngine::new(&db).await?);

    let store = Arc::new(searxiv::Store { engine, db });
    let app = axum::Router::new()
        .route("/", axum::routing::get(searxiv::root))
        .route("/search", axum::routing::get(searxiv::search))
        .merge(RapiDoc::with_openapi("/api-docs/openapi.json", ApiDoc::openapi()).path("/docs"))
        .with_state(store);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], CONFIG.server_specific.port));
    tracing::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

mod searxiv {
    use axum::{
        extract::{Query, State},
        Json,
    };
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use arxiv_shared::db::DBConnection;

    use crate::config::CONFIG;

    pub(super) struct Store {
        pub(crate) engine: Mutex<crate::engine::SearchEngine>,
        pub(crate) db: Arc<Mutex<DBConnection>>,
    }

    #[utoipa::path(
        get,
        path = "/",
        responses(
            (status = 200, description = "Say hi!", body = String)
        )
    )]
    pub(super) async fn root(State(_): State<Arc<Store>>) -> &'static str {
        "Hi!"
    }

    /// Paper info
    #[derive(serde::Serialize, utoipa::ToSchema)]
    pub(super) struct PaperInfo {
        /// Title of the paper
        title: String,
        /// Description or absract of the paper
        description: String,
        /// Url to the paper on arxiv.org
        url: String,
    }

    #[derive(serde::Deserialize, utoipa::IntoParams)]
    pub(super) struct SearchQuery {
        query: String,
    }

    #[utoipa::path(
        get,
        path = "/search",
        params(
            SearchQuery
        ),
        responses(
            (status = 200, description = "Search for papers", body = [PaperInfo])
        )
    )]
    pub(super) async fn search(
        State(state): State<Arc<Store>>,
        query: Query<SearchQuery>,
    ) -> Json<Vec<PaperInfo>> {
        let results = state
            .engine
            .lock()
            .await
            .query(query.query.clone(), CONFIG.server_specific.max_results)
            .unwrap();

        let mut papers = Vec::new();

        for (_score, doc_address) in results {
            let doc_id = state.engine.lock().await.get_doc_id(doc_address).unwrap();
            let mut db = state.db.lock().await;
            let paper = db.get_paper(doc_id as i32).unwrap();
            papers.push(PaperInfo {
                title: paper.title,
                description: paper.description,
                url: paper.url,
            })
        }
        Json(papers)
    }
}
