use diesel::pg::PgConnection;
use diesel::prelude::*;

use crate::models;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database connection error")]
    ConnectionError(#[from] diesel::result::ConnectionError),

    #[error("database query error")]
    QueryError(#[from] diesel::result::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct DBConnection {
    pg: PgConnection,
}

impl DBConnection {
    pub fn new() -> Result<DBConnection> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        Ok(DBConnection {
            pg: PgConnection::establish(&database_url)?,
        })
    }

    pub fn get_all_papers(&mut self) -> Result<Vec<models::Paper>> {
        use crate::schema::papers::dsl::*;

        let result = papers
            .select(models::Paper::as_select())
            .load(&mut self.pg)?;

        Ok(result)
    }
}
