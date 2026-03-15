-- SurrealDB 3.0 Schema for rustvideoplatform
-- Run once against your SurrealDB instance to create all tables, fields, and indexes.
--
-- Usage:
--   surreal import \
--     --conn ws://localhost:8000 \
--     --user root --pass root \
--     --ns main --db main \
--     schema.surql

-- ─── Visibility helper function ──────────────────────────────────────────────
-- Returns true if a resource is visible to $viewer.
-- $viewer is the user's login string, or '' for anonymous.
-- Used in WHERE clauses as: fn::visible_to(visibility, restricted_to_group, owner, $viewer)
DEFINE FUNCTION fn::visible_to(
    $visibility: string,
    $rtg: option<string>,
    $owner: string,
    $viewer: string
) {
    RETURN $visibility = 'public'
        OR ($visibility = 'restricted' AND (
            $rtg IN (SELECT VALUE group_id FROM user_group_members WHERE user_login = $viewer)
            OR ($rtg = '__all_registered__' AND string::len($viewer) > 0)
            OR ($rtg = '__subscribers__'
                AND string::len($viewer) > 0
                AND $owner IN (SELECT VALUE target FROM subscriptions WHERE subscriber = $viewer))
        ));
} PERMISSIONS FULL;

-- ─── Users ───────────────────────────────────────────────────────────────────
-- Record ID is the login name: users:alice
DEFINE TABLE users SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.id = id;

DEFINE FIELD name              ON TABLE users TYPE string;
DEFINE FIELD password_hash     ON TABLE users TYPE string;
DEFINE FIELD profile_picture   ON TABLE users TYPE option<string>;
DEFINE FIELD totp_enabled      ON TABLE users TYPE bool    DEFAULT false;
DEFINE FIELD totp_secret       ON TABLE users TYPE option<string>;
DEFINE FIELD created           ON TABLE users TYPE int     DEFAULT time::unix(time::now());

-- ─── Media ───────────────────────────────────────────────────────────────────
-- Record ID is the randomly-generated media ID: media:abc123
DEFINE TABLE media SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD name                 ON TABLE media TYPE string;
DEFINE FIELD description          ON TABLE media TYPE string  DEFAULT '';
DEFINE FIELD owner                ON TABLE media TYPE string;
DEFINE FIELD type                 ON TABLE media TYPE string;
  -- Accepted values: video | audio | picture | document_pdf
DEFINE FIELD visibility           ON TABLE media TYPE string  DEFAULT 'hidden';
  -- Accepted values: public | hidden | restricted
DEFINE FIELD restricted_to_group  ON TABLE media TYPE option<string>;
  -- Accepted values: __all_registered__ | __subscribers__ | <user_group id>
DEFINE FIELD views                ON TABLE media TYPE int     DEFAULT 0;
DEFINE FIELD upload               ON TABLE media TYPE int     DEFAULT time::unix(time::now());
DEFINE FIELD public               ON TABLE media TYPE bool    DEFAULT false;
-- Denormalized reaction counts kept in sync by the media_likes_changed event below.
-- Always prefer reading these fields over counting media_likes rows at query time.
DEFINE FIELD likes_count          ON TABLE media TYPE int     DEFAULT 0;
DEFINE FIELD dislikes_count       ON TABLE media TYPE int     DEFAULT 0;

DEFINE INDEX media_owner_idx      ON TABLE media FIELDS owner;
DEFINE INDEX media_upload_idx     ON TABLE media FIELDS upload;
DEFINE INDEX media_likes_idx      ON TABLE media FIELDS likes_count;

-- ─── Media Likes / Dislikes ──────────────────────────────────────────────────
-- UPSERT media_likes SET media_id=$mid, user_login=$user, reaction='like'
-- UNIQUE index prevents duplicate reactions per user per media item.
DEFINE TABLE media_likes SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD media_id    ON TABLE media_likes TYPE string;
DEFINE FIELD user_login  ON TABLE media_likes TYPE string;
DEFINE FIELD reaction    ON TABLE media_likes TYPE string;
  -- Accepted values: like | dislike

DEFINE INDEX media_likes_media_user ON TABLE media_likes FIELDS media_id, user_login UNIQUE;
DEFINE INDEX media_likes_media_idx  ON TABLE media_likes FIELDS media_id;

-- Keep likes_count / dislikes_count on the parent media record up to date
-- whenever a reaction is inserted, updated, or deleted.
DEFINE EVENT media_likes_changed ON TABLE media_likes
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE"
  THEN {
    LET $mid = IF $event = "DELETE" THEN $before.media_id ELSE $after.media_id END;
    UPDATE type::thing('media', $mid) SET
      likes_count    = array::len((SELECT id FROM media_likes WHERE media_id = $mid AND reaction = 'like')),
      dislikes_count = array::len((SELECT id FROM media_likes WHERE media_id = $mid AND reaction = 'dislike'));
  };

-- ─── Comments ────────────────────────────────────────────────────────────────
DEFINE TABLE comments SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD media   ON TABLE comments TYPE string;
DEFINE FIELD author  ON TABLE comments TYPE string;
DEFINE FIELD text    ON TABLE comments TYPE string;
DEFINE FIELD time    ON TABLE comments TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX comments_media_idx ON TABLE comments FIELDS media;
DEFINE INDEX comments_time_idx  ON TABLE comments FIELDS time;

-- ─── Subscriptions ───────────────────────────────────────────────────────────
-- Composite record ID: subscriptions:[$subscriber, $target]
-- UPSERT subscriptions:[$subscriber,$target] SET subscriber=$sub, target=$tgt
DEFINE TABLE subscriptions SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD subscriber ON TABLE subscriptions TYPE string;
DEFINE FIELD target     ON TABLE subscriptions TYPE string;

DEFINE INDEX subscriptions_sub_target ON TABLE subscriptions FIELDS subscriber, target UNIQUE;
DEFINE INDEX subscriptions_subscriber ON TABLE subscriptions FIELDS subscriber;
DEFINE INDEX subscriptions_target     ON TABLE subscriptions FIELDS target;

-- ─── User Groups ─────────────────────────────────────────────────────────────
-- Record ID is a randomly-generated ID: user_groups:xyz789
DEFINE TABLE user_groups SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD name     ON TABLE user_groups TYPE string;
DEFINE FIELD owner    ON TABLE user_groups TYPE string;
DEFINE FIELD created  ON TABLE user_groups TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX user_groups_owner_idx   ON TABLE user_groups FIELDS owner;
DEFINE INDEX user_groups_created_idx ON TABLE user_groups FIELDS created;

-- ─── User Group Members ──────────────────────────────────────────────────────
DEFINE TABLE user_group_members SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD group_id   ON TABLE user_group_members TYPE string;
DEFINE FIELD user_login ON TABLE user_group_members TYPE string;

DEFINE INDEX ugm_group_user ON TABLE user_group_members FIELDS group_id, user_login UNIQUE;
DEFINE INDEX ugm_user_idx   ON TABLE user_group_members FIELDS user_login;
DEFINE INDEX ugm_group_idx  ON TABLE user_group_members FIELDS group_id;

-- ─── Lists ───────────────────────────────────────────────────────────────────
-- Record ID is a randomly-generated ID: lists:abc123
DEFINE TABLE lists SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD name                ON TABLE lists TYPE string;
DEFINE FIELD owner               ON TABLE lists TYPE string;
DEFINE FIELD public              ON TABLE lists TYPE bool   DEFAULT false;
DEFINE FIELD visibility          ON TABLE lists TYPE string DEFAULT 'hidden';
  -- Accepted values: public | hidden | restricted
DEFINE FIELD restricted_to_group ON TABLE lists TYPE option<string>;
DEFINE FIELD created             ON TABLE lists TYPE int    DEFAULT time::unix(time::now());

DEFINE INDEX lists_owner_idx   ON TABLE lists FIELDS owner;
DEFINE INDEX lists_created_idx ON TABLE lists FIELDS created;

-- ─── List Items ──────────────────────────────────────────────────────────────
DEFINE TABLE list_items SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD list_id  ON TABLE list_items TYPE string;
DEFINE FIELD media_id ON TABLE list_items TYPE string;
DEFINE FIELD position ON TABLE list_items TYPE int DEFAULT 0;

DEFINE INDEX list_items_list_pos ON TABLE list_items FIELDS list_id, position;
DEFINE INDEX list_items_list_idx ON TABLE list_items FIELDS list_id;

-- ─── Media Concepts (processing job queue) ───────────────────────────────────
-- Record ID is the upload filename / concept slug.
-- The processor polls: SELECT id, type FROM media_concepts WHERE processed = false
DEFINE TABLE media_concepts SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD name      ON TABLE media_concepts TYPE string;
DEFINE FIELD owner     ON TABLE media_concepts TYPE string;
DEFINE FIELD type      ON TABLE media_concepts TYPE string;
  -- Accepted values: video | audio | picture | document_pdf | vtt_translate
DEFINE FIELD processed ON TABLE media_concepts TYPE bool DEFAULT false;

DEFINE INDEX media_concepts_processed ON TABLE media_concepts FIELDS processed;
DEFINE INDEX media_concepts_owner_idx ON TABLE media_concepts FIELDS owner;

-- ─── WebAuthn / Passkey Credentials ─────────────────────────────────────────
-- Record ID is user-provided credential name.
DEFINE TABLE webauthn_credentials SCHEMAFULL
  PERMISSIONS FOR select, create, update, delete WHERE true;

DEFINE FIELD user_login      ON TABLE webauthn_credentials TYPE string;
DEFINE FIELD credential_name ON TABLE webauthn_credentials TYPE string;
DEFINE FIELD passkey         ON TABLE webauthn_credentials TYPE string;
  -- JSON-serialised webauthn_rs Passkey struct
DEFINE FIELD created         ON TABLE webauthn_credentials TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX webauthn_user_idx ON TABLE webauthn_credentials FIELDS user_login;

-- ─── End of schema ───────────────────────────────────────────────────────────
