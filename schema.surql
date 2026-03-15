-- SurrealDB 3.0 Schema for rustvideoplatform
-- Apply with: surreal import --conn ws://localhost:8000 --user root --pass root --ns main --db main schema.surql

-- ─── Users ───────────────────────────────────────────────────────────────────
DEFINE TABLE users SCHEMAFULL;
DEFINE FIELD name              ON users TYPE string;
DEFINE FIELD password_hash     ON users TYPE string;
DEFINE FIELD profile_picture   ON users TYPE option<string>;
DEFINE FIELD totp_enabled      ON users TYPE bool DEFAULT false;
DEFINE FIELD totp_secret       ON users TYPE option<string>;
DEFINE FIELD created           ON users TYPE int DEFAULT time::unix(time::now());

-- ─── Media ───────────────────────────────────────────────────────────────────
DEFINE TABLE media SCHEMAFULL;
DEFINE FIELD name                 ON media TYPE string;
DEFINE FIELD description          ON media TYPE string DEFAULT '';
DEFINE FIELD owner                ON media TYPE string;
DEFINE FIELD type                 ON media TYPE string;    -- video | audio | picture | document_pdf
DEFINE FIELD visibility           ON media TYPE string DEFAULT 'hidden';  -- public | hidden | restricted
DEFINE FIELD restricted_to_group  ON media TYPE option<string>;
DEFINE FIELD views                ON media TYPE int DEFAULT 0;
DEFINE FIELD upload               ON media TYPE int DEFAULT time::unix(time::now());
DEFINE FIELD public               ON media TYPE bool DEFAULT false;

DEFINE INDEX media_owner_idx   ON media FIELDS owner;
DEFINE INDEX media_upload_idx  ON media FIELDS upload;

-- ─── Media Likes / Dislikes ──────────────────────────────────────────────────
DEFINE TABLE media_likes SCHEMAFULL;
DEFINE FIELD media_id    ON media_likes TYPE string;
DEFINE FIELD user_login  ON media_likes TYPE string;
DEFINE FIELD reaction    ON media_likes TYPE string;   -- like | dislike

DEFINE INDEX media_likes_media_user ON media_likes FIELDS media_id, user_login UNIQUE;

-- ─── Comments ────────────────────────────────────────────────────────────────
DEFINE TABLE comments SCHEMAFULL;
DEFINE FIELD media   ON comments TYPE string;
DEFINE FIELD author  ON comments TYPE string;
DEFINE FIELD text    ON comments TYPE string;
DEFINE FIELD time    ON comments TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX comments_media_idx ON comments FIELDS media;

-- ─── Subscriptions ───────────────────────────────────────────────────────────
DEFINE TABLE subscriptions SCHEMAFULL;
DEFINE FIELD subscriber ON subscriptions TYPE string;
DEFINE FIELD target     ON subscriptions TYPE string;

DEFINE INDEX subscriptions_sub_target ON subscriptions FIELDS subscriber, target UNIQUE;
DEFINE INDEX subscriptions_target_idx ON subscriptions FIELDS target;

-- ─── User Groups ─────────────────────────────────────────────────────────────
DEFINE TABLE user_groups SCHEMAFULL;
DEFINE FIELD name     ON user_groups TYPE string;
DEFINE FIELD owner    ON user_groups TYPE string;
DEFINE FIELD created  ON user_groups TYPE int DEFAULT time::unix(time::now());

DEFINE TABLE user_group_members SCHEMAFULL;
DEFINE FIELD group_id   ON user_group_members TYPE string;
DEFINE FIELD user_login ON user_group_members TYPE string;

DEFINE INDEX ugm_group_user ON user_group_members FIELDS group_id, user_login UNIQUE;
DEFINE INDEX ugm_user_idx   ON user_group_members FIELDS user_login;

-- ─── Lists ───────────────────────────────────────────────────────────────────
DEFINE TABLE lists SCHEMAFULL;
DEFINE FIELD name                ON lists TYPE string;
DEFINE FIELD owner               ON lists TYPE string;
DEFINE FIELD public              ON lists TYPE bool DEFAULT false;
DEFINE FIELD visibility          ON lists TYPE string DEFAULT 'hidden';
DEFINE FIELD restricted_to_group ON lists TYPE option<string>;
DEFINE FIELD created             ON lists TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX lists_owner_idx   ON lists FIELDS owner;
DEFINE INDEX lists_created_idx ON lists FIELDS created;

DEFINE TABLE list_items SCHEMAFULL;
DEFINE FIELD list_id  ON list_items TYPE string;
DEFINE FIELD media_id ON list_items TYPE string;
DEFINE FIELD position ON list_items TYPE int DEFAULT 0;

DEFINE INDEX list_items_list_idx ON list_items FIELDS list_id;

-- ─── Media Concepts (processing queue) ───────────────────────────────────────
DEFINE TABLE media_concepts SCHEMAFULL;
DEFINE FIELD name      ON media_concepts TYPE string;
DEFINE FIELD owner     ON media_concepts TYPE string;
DEFINE FIELD type      ON media_concepts TYPE string;  -- video | audio | picture | document_pdf | vtt_translate
DEFINE FIELD processed ON media_concepts TYPE bool DEFAULT false;

DEFINE INDEX media_concepts_processed ON media_concepts FIELDS processed;

-- ─── WebAuthn Credentials ────────────────────────────────────────────────────
DEFINE TABLE webauthn_credentials SCHEMAFULL;
DEFINE FIELD user_login      ON webauthn_credentials TYPE string;
DEFINE FIELD credential_name ON webauthn_credentials TYPE string;
DEFINE FIELD passkey         ON webauthn_credentials TYPE string;  -- JSON-serialised Passkey
DEFINE FIELD created         ON webauthn_credentials TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX webauthn_user_idx ON webauthn_credentials FIELDS user_login;

-- ─── Session Tokens ──────────────────────────────────────────────────────────
-- Sessions are stored in Redis; no SurrealDB table needed.
