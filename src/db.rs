use diesel::pg::PgConnection;
use diesel::prelude::*;

use crate::models::{self, NewPaper, NewSubject};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database connection error")]
    Connection(#[from] diesel::result::ConnectionError),

    #[error("database query error")]
    Query(#[from] diesel::result::Error),
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

    pub fn insert_paper(
        &mut self,
        paper_submission: &str,
        paper_title: &str,
        paper_description: &str,
        paper_body: &str,
    ) -> Result<models::Id> {
        use crate::schema::papers::dsl::*;

        let select_existing = papers
            .select(id)
            .filter(submission.eq(paper_submission))
            .get_result(&mut self.pg)
            .ok();

        match select_existing {
            Some(existing_paper_id) => Ok(existing_paper_id),
            None => diesel::insert_into(papers)
                .values(&NewPaper {
                    submission: paper_submission,
                    title: paper_title,
                    description: paper_description,
                    body: paper_body,
                })
                .returning(id)
                .on_conflict_do_nothing()
                .get_result(&mut self.pg)
                .map_err(|e| e.into()),
        }
    }

    pub fn insert_author(&mut self, author_name: &str) -> Result<models::Id> {
        use crate::schema::authors::dsl::*;

        let select_exisiting = authors
            .select(id)
            .filter(name.eq(name))
            .get_result(&mut self.pg)
            .ok();

        match select_exisiting {
            Some(existing_author_id) => Ok(existing_author_id),
            None => diesel::insert_into(authors)
                .values(&models::NewAuthor { name: author_name })
                .on_conflict(name)
                .do_nothing()
                .returning(id)
                .get_result(&mut self.pg)
                .map_err(|e| e.into()),
        }
    }

    pub fn insert_subject(&mut self, subject_name: &str) -> Result<models::Id> {
        use crate::schema::subjects::dsl::*;

        let select_exisiting = subjects
            .select(id)
            .filter(name.eq(subject_name))
            .get_result(&mut self.pg)
            .ok();

        match select_exisiting {
            Some(existing_subject_id) => Ok(existing_subject_id),
            None => diesel::insert_into(subjects)
                .values(&NewSubject { name: subject_name })
                .on_conflict(name)
                .do_nothing()
                .returning(id)
                .get_result(&mut self.pg)
                .map_err(|e| e.into()),
        }
    }

    pub fn set_paper_author(&mut self, paper: models::Id, author: models::Id) -> Result<usize> {
        use crate::schema::paper_author::dsl::*;

        println!("\n\npapaer author\n\n");
        diesel::insert_into(paper_author)
            .values((paper_id.eq(paper), author_id.eq(author)))
            .execute(&mut self.pg)
            .map_err(|e| e.into())
    }

    pub fn set_paper_category(&mut self, paper: models::Id, category: models::Id) -> Result<usize> {
        use crate::schema::paper_subject::dsl::*;

        println!("\n\npapaer category\n\n");
        diesel::insert_into(paper_subject)
            .values((paper_id.eq(paper), subject_id.eq(category)))
            .execute(&mut self.pg)
            .map_err(|e| e.into())
    }
}
