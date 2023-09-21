use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenvy::dotenv;
use std::env;

use crate::models::{self, Paper};

pub struct DBConnection {
    pg: PgConnection,
}

impl DBConnection {
    pub fn new() -> DBConnection {
        dotenv().ok();

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        DBConnection {
            pg: PgConnection::establish(&database_url)
                .unwrap_or_else(|_| panic!("Error connecting to {}", database_url)),
        }
    }

    pub fn get_all_papers(&mut self) -> Vec<models::Paper> {
        use crate::schema::papers::dsl::*;

        papers
            .select(Paper::as_select())
            .load(&mut self.pg)
            .expect("Error loading posts")
    }
}
