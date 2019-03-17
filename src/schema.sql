CREATE TABLE image (
    image_id INTEGER PRIMARY KEY,
    location_id INTEGER,
    image_name TEXT NOT NULL,
    image_hash TEXT NOT NULL,
    image_width INTEGER NOT NULL,
    image_height INTEGER NOT NULL,
    image_type TEXT NOT NULL,
    image_caption TEXT NOT NULL DEFAULT "",
    image_description TEXT NOT NULL DEFAULT "",
    image_rating INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_name ON image (image_name);

CREATE TABLE image_comment (
    comment_id INTEGER PRIMARY KEY,
    image_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    comment_timestamp TIMESTAMP NOT NULL,
    comment_text TEXT NOT NULL
);

CREATE TABLE image_person (
    image_id INTEGER,
    person_id INTEGER,
    PRIMARY KEY (image_id, person_id)
);

CREATE TABLE image_tag (
    image_id INTEGER,
    tag_id INTEGER,
    PRIMARY KEY (image_id, tag_id)
);

CREATE TABLE person (
    person_id INTEGER PRIMARY KEY,
    person_name TEXT NOT NULL
);

CREATE TABLE tag (
    tag_id INTEGER PRIMARY KEY,
    tag_name TEXT NOT NULL
);

CREATE TABLE location (
    location_id INTEGER PRIMARY KEY,
    location_name TEXT NOT NULL
);

CREATE TABLE user (
    user_id INTEGER PRIMARY KEY,
    user_name TEXT NOT NULL,
    user_password TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_user_name ON user (user_name);
