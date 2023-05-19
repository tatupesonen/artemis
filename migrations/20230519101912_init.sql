-- Add migration script here
CREATE TABLE feeds
(
		id SERIAL not null,
		name character varying(2048) NOT NULL UNIQUE,
    url character varying(2048) NOT NULL UNIQUE,
    PRIMARY KEY (id)
);
