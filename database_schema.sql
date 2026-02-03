CREATE TABLE public.media (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	description text NOT NULL,
	upload int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	"owner" varchar NOT NULL,
	likes int8 DEFAULT 0 NOT NULL,
	dislikes int8 DEFAULT 0 NOT NULL,
	"views" int8 DEFAULT 0 NOT NULL,
	public bool DEFAULT false NOT NULL,
	"type" varchar NOT NULL,
	CONSTRAINT videos_pk PRIMARY KEY (id)
);
CREATE TABLE public."comments" (
	id bigserial NOT NULL,
	media varchar NOT NULL,
	"user" varchar NOT NULL,
	"text" text NOT NULL,
	"time" int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	CONSTRAINT comments_pk PRIMARY KEY (id)
);
CREATE TABLE public.users (
	login varchar(40) NOT NULL,
	name varchar(100) NOT NULL,
	password_hash varchar NOT NULL,
	profile_picture varchar,
	channel_picture varchar,
	CONSTRAINT users_pk PRIMARY KEY (login)
);
CREATE TABLE public.subscriptions (
	subscriber varchar(40) NOT NULL,
	target varchar(40) NOT NULL
);
CREATE TABLE public.media_concepts (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	"owner" varchar NOT NULL,
	processed bool DEFAULT false NOT NULL,
	"type" varchar NOT NULL,
	CONSTRAINT media_concepts_pk PRIMARY KEY (id)
);

CREATE TABLE public.playlists (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	description text NOT NULL,
	"owner" varchar NOT NULL,
	created_at int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	public bool DEFAULT true NOT NULL,
	CONSTRAINT playlists_pk PRIMARY KEY (id)
);

CREATE TABLE public.playlist_items (
	id bigserial NOT NULL,
	playlist_id varchar NOT NULL,
	media_id varchar NOT NULL,
	item_order int8 DEFAULT 0 NOT NULL,
	added_at int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	CONSTRAINT playlist_items_pk PRIMARY KEY (id),
	CONSTRAINT playlist_items_playlist_fk FOREIGN KEY (playlist_id) REFERENCES public.playlists(id) ON DELETE CASCADE,
	CONSTRAINT playlist_items_media_fk FOREIGN KEY (media_id) REFERENCES public.media(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX playlist_items_unique_media_per_playlist ON public.playlist_items(playlist_id, media_id);
