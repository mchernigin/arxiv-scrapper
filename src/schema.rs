// @generated automatically by Diesel CLI.

diesel::table! {
    authors (id) {
        id -> Int4,
        name -> Varchar,
    }
}

diesel::table! {
    categories (id) {
        id -> Int4,
        name -> Varchar,
    }
}

diesel::table! {
    paper_author (paper_id) {
        paper_id -> Int4,
        author_id -> Int4,
    }
}

diesel::table! {
    paper_category (paper_id) {
        paper_id -> Int4,
        category_id -> Int4,
    }
}

diesel::table! {
    papers (id) {
        id -> Int4,
        title -> Varchar,
        body -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    authors,
    categories,
    paper_author,
    paper_category,
    papers,
);
