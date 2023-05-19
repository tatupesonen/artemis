-- Add migration script here
CREATE TABLE IF NOT EXISTS feed_entries
(
    id SERIAL NOT NULL,
    title character varying,
    link character varying(2048),
    pub_date timestamp,
    guid character varying,
		feed_id integer,
		UNIQUE (feed_id, guid),
    CONSTRAINT feed_entries_pkey PRIMARY KEY (id),
		CONSTRAINT feed_id FOREIGN KEY (feed_id)
        REFERENCES public.feeds (id) MATCH SIMPLE
        ON UPDATE NO ACTION
        ON DELETE NO ACTION
        NOT VALID
)