// @generated automatically by Diesel CLI.

diesel::table! {
    authors (id) {
        id -> Int4,
        name -> Varchar,
    }
}

diesel::table! {
    paper_author (paper_id, author_id) {
        paper_id -> Int4,
        author_id -> Int4,
    }
}

diesel::table! {
    paper_subject (paper_id, subject_id) {
        paper_id -> Int4,
        subject_id -> Int4,
    }
}

diesel::table! {
    papers (id) {
        id -> Int4,
        title -> Varchar,
        description -> Text,
        body -> Text,
    }
}

diesel::table! {
    subjects (id) {
        id -> Int4,
        name -> Varchar,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    authors,
    paper_author,
    paper_subject,
    papers,
    subjects,
);
