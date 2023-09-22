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

    #[allow(dead_code)]
    pub fn get_all_papers(&mut self) -> Result<Vec<models::Paper>> {
        use crate::schema::papers::dsl::*;

        papers
            .select(models::Paper::as_select())
            .load(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn count_papers(&mut self) -> Result<i64> {
        use crate::schema::papers::dsl::*;

        papers
            .count()
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn paper_exists(&mut self, s: &str) -> Result<bool> {
        use crate::schema::papers::dsl::*;

        let count = papers
            .count()
            .filter(submission.eq(s))
            .get_result::<i64>(&mut self.pg)?;

        Ok(count > 0)
    }

    pub fn insert_paper(&mut self, paper: models::NewPaper) -> Result<models::Id> {
        use crate::schema::papers::dsl::*;

        if let Ok(existing_paper_id) = papers
            .select(id)
            .filter(submission.eq(paper.submission))
            .get_result::<models::Id>(&mut self.pg)
        {
            return Ok(existing_paper_id);
        }

        diesel::insert_into(papers)
            .values(&paper)
            .returning(id)
            .on_conflict_do_nothing()
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn insert_author(&mut self, author: models::NewAuthor) -> Result<models::Id> {
        use crate::schema::authors::dsl::*;

        diesel::insert_into(authors)
            .values(&author)
            .returning(id)
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn insert_subject(&mut self, category: models::NewSubject) -> Result<models::Id> {
        use crate::schema::subjects::dsl::*;

        diesel::insert_into(subjects)
            .values(&category)
            .returning(id)
            .get_result(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn set_paper_author(&mut self, paper: models::Id, author: models::Id) -> Result<usize> {
        use crate::schema::paper_author::dsl::*;

        diesel::insert_into(paper_author)
            .values((paper_id.eq(paper), author_id.eq(author)))
            .execute(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn set_paper_category(&mut self, paper: models::Id, category: models::Id) -> Result<usize> {
        use crate::schema::paper_subject::dsl::*;

        diesel::insert_into(paper_subject)
            .values((paper_id.eq(paper), subject_id.eq(category)))
            .execute(&mut self.pg)
            .map_err(|e| e.into())
    }
}
