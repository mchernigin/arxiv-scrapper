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
    pub fn new(db_url: &str) -> Result<DBConnection> {
        Ok(DBConnection {
            pg: PgConnection::establish(db_url)?,
        })
    }

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

    pub fn get_paper(&mut self, desired_id: i32) -> Option<models::Paper> {
        use crate::schema::papers::dsl::*;

        papers
            .filter(id.eq(desired_id))
            .get_result(&mut self.pg)
            .ok()
    }

    pub fn get_paper_authors(&mut self, desired_paper_id: i32) -> Result<Vec<models::Author>> {
        use crate::schema::authors::dsl::*;
        use crate::schema::paper_author::dsl::*;

        let paper_authors: Vec<models::PaperAuthor> = paper_author
            .filter(paper_id.eq(desired_paper_id))
            .get_results(&mut self.pg)?;

        let authors_ids = paper_authors
            .into_iter()
            .map(|pa| pa.author_id)
            .collect::<Vec<_>>();

        let r = authors
            .filter(id.eq_any(authors_ids))
            .get_results(&mut self.pg)?;

        Ok(r)
    }

    pub fn paper_exists(&mut self, s: &str) -> Result<bool> {
        use crate::schema::papers::dsl::*;

        let count = papers
            .count()
            .filter(url.eq(s))
            .get_result::<i64>(&mut self.pg)?;

        Ok(count > 0)
    }

    pub fn insert_paper(
        &mut self,
        paper_url: &str,
        paper_title: &str,
        paper_description: &str,
        paper_body: &str,
    ) -> Result<models::Id> {
        use crate::schema::papers::dsl::*;

        let select_existing = papers
            .select(id)
            .filter(url.eq(paper_url))
            .get_result(&mut self.pg)
            .ok();

        match select_existing {
            Some(existing_paper_id) => {
                log::trace!("DB: paper already exists {paper_url:?}");
                Ok(existing_paper_id)
            }
            None => {
                log::trace!("DB: inserting new paper {paper_url:?}");
                diesel::insert_into(papers)
                    .values(&NewPaper {
                        url: paper_url,
                        title: paper_title,
                        description: paper_description,
                        body: paper_body,
                    })
                    .returning(id)
                    .on_conflict_do_nothing()
                    .get_result(&mut self.pg)
                    .map_err(|e| e.into())
            }
        }
    }

    pub fn insert_author(&mut self, author_name: &str) -> Result<models::Id> {
        use crate::schema::authors::dsl::*;

        let select_exisiting = authors
            .select(id)
            .filter(name.eq(author_name))
            .get_result(&mut self.pg)
            .ok();

        match select_exisiting {
            Some(existing_author_id) => {
                log::trace!("DB: author already exists {author_name:?}");
                Ok(existing_author_id)
            }
            None => {
                log::trace!("DB: inserting new author {author_name:?}");
                diesel::insert_into(authors)
                    .values(&models::NewAuthor { name: author_name })
                    .on_conflict(name)
                    .do_nothing()
                    .returning(id)
                    .get_result(&mut self.pg)
                    .map_err(|e| e.into())
            }
        }
    }

    pub fn insert_subject(&mut self, subject_name: &str) -> Result<models::Id> {
        use crate::schema::subjects::dsl::*;

        let select_existing = subjects
            .select(id)
            .filter(name.eq(subject_name))
            .get_result(&mut self.pg)
            .ok();

        match select_existing {
            Some(existing_subject_id) => {
                log::trace!("DB: subject already exists {subject_name:?}");
                Ok(existing_subject_id)
            }
            None => {
                log::trace!("DB: inserting new subject {subject_name:?}");
                diesel::insert_into(subjects)
                    .values(&NewSubject { name: subject_name })
                    .on_conflict(name)
                    .do_nothing()
                    .returning(id)
                    .get_result(&mut self.pg)
                    .map_err(|e| e.into())
            }
        }
    }

    pub fn set_paper_author(&mut self, paper: models::Id, author: models::Id) -> Result<()> {
        use crate::schema::paper_author::dsl::*;

        let select_existing: Option<(models::Id, models::Id)> = paper_author
            .select((paper_id, author_id))
            .filter(paper_id.eq(paper).and(author_id.eq(author)))
            .get_result(&mut self.pg)
            .ok();

        if select_existing.is_none() {
            diesel::insert_into(paper_author)
                .values((paper_id.eq(paper), author_id.eq(author)))
                .execute(&mut self.pg)?;
        }

        Ok(())
    }

    pub fn set_paper_category(&mut self, paper: models::Id, subject: models::Id) -> Result<()> {
        use crate::schema::paper_subject::dsl::*;

        let select_existing: Option<(models::Id, models::Id)> = paper_subject
            .select((paper_id, subject_id))
            .filter(paper_id.eq(paper).and(subject_id.eq(subject)))
            .get_result(&mut self.pg)
            .ok();

        if select_existing.is_none() {
            diesel::insert_into(paper_subject)
                .values((paper_id.eq(paper), subject_id.eq(subject)))
                .execute(&mut self.pg)?;
        }

        Ok(())
    }
}
