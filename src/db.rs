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

        papers
            .select(models::Paper::as_select())
            .load(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn add_paper(&mut self, paper: models::NewPaper) -> Result<models::Paper> {
        use crate::schema::papers;

        diesel::insert_into(papers::table)
            .values(&paper)
            .returning(models::Paper::as_returning())
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn add_author(&mut self, author: models::NewAuthor) -> Result<models::Author> {
        use crate::schema::authors;

        diesel::insert_into(authors::table)
            .values(&author)
            .returning(models::Author::as_returning())
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn add_category(&mut self, category: models::NewCategory) -> Result<models::Categories> {
        use crate::schema::categories;

        diesel::insert_into(categories::table)
            .values(&category)
            .returning(models::Categories::as_returning())
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }
}
