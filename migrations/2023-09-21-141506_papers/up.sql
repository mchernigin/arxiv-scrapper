-- Your SQL goes here
CREATE TABLE papers (
  id SERIAL PRIMARY KEY,
  url VARCHAR NOT NULL UNIQUE,
  title VARCHAR NOT NULL,
  description TEXT NOT NULL,
  body TEXT NOT NULL
);


CREATE TABLE authors (
  id SERIAL PRIMARY KEY,
  name VARCHAR NOT NULL UNIQUE
);

CREATE TABLE paper_author (
  paper_id INTEGER REFERENCES papers (id),
  author_id INTEGER REFERENCES authors (id),
  PRIMARY KEY (paper_id, author_id)
);


CREATE TABLE subjects (
  id SERIAL PRIMARY KEY,
  name VARCHAR NOT NULL UNIQUE
);

CREATE TABLE paper_subject (
  paper_id INTEGER REFERENCES papers (id),
  subject_id INTEGER REFERENCES subjects (id),
  PRIMARY KEY (paper_id, subject_id)
);

