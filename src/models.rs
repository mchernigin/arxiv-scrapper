use diesel::prelude::*;

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::papers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Paper {
    pub id: i32,
    pub title: String,
    pub body: String,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::authors)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Author {
    pub id: i32,
    pub name: String,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::categories)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Categories {
    pub id: i32,
    pub name: String,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::paper_author)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PaperAuthor {
    pub paper_id: i32,
    pub author_id: i32,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::paper_category)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PaperCategory {
    pub paper_id: i32,
    pub category_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::papers)]
pub struct NewPaper<'a> {
    pub title: &'a str,
    pub body: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::authors)]
pub struct NewAuthor<'a> {
    pub name: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::categories)]
pub struct NewCategory<'a> {
    pub name: &'a str,
}
