-- Your SQL goes here
CREATE TABLE papers (
  id SERIAL PRIMARY KEY,
  title VARCHAR NOT NULL,
  body TEXT NOT NULL
);


CREATE TABLE authors (
  id SERIAL PRIMARY KEY,
  name VARCHAR NOT NULL
);

CREATE TABLE paper_author (
  paper_id INTEGER PRIMARY KEY REFERENCES papers (id),
  author_id INTEGER REFERENCES papers (id) NOT NULL
);


CREATE TABLE categories (
  id SERIAL PRIMARY KEY,
  name VARCHAR NOT NULL
);

CREATE TABLE paper_category (
  paper_id INTEGER PRIMARY KEY REFERENCES papers (id),
  category_id INTEGER REFERENCES papers (id) NOT NULL
);

