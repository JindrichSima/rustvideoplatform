-- SurrealDB Schema for Rust Video Platform
-- Uses graph edges for relationships and native SurrealDB features

-- ============================================================
-- TABLES
-- ============================================================

-- Users: record ID is the login string (e.g. users:john)
DEFINE TABLE users SCHEMAFULL;
DEFINE FIELD name ON users TYPE string;
DEFINE FIELD password_hash ON users TYPE string;
DEFINE FIELD profile_picture ON users TYPE option<string>;
DEFINE FIELD channel_picture ON users TYPE option<string>;

-- Media: record ID is a 10-char random string (e.g. media:abc1234567)
DEFINE TABLE media SCHEMAFULL;
DEFINE FIELD name ON media TYPE string;
DEFINE FIELD description ON media TYPE option<object>;
DEFINE FIELD upload ON media TYPE int DEFAULT time::unix(time::now());
DEFINE FIELD owner ON media TYPE string;
DEFINE FIELD views ON media TYPE int DEFAULT 0;
DEFINE FIELD visibility ON media TYPE string DEFAULT 'hidden';
DEFINE FIELD restricted_to_group ON media TYPE option<string>;
DEFINE FIELD type ON media TYPE string;

DEFINE INDEX media_owner ON media FIELDS owner;
DEFINE INDEX media_visibility ON media FIELDS visibility;

-- Comments: record ID is a random string
DEFINE TABLE comments SCHEMAFULL;
DEFINE FIELD media ON comments TYPE string;
DEFINE FIELD user ON comments TYPE string;
DEFINE FIELD text ON comments TYPE object;
DEFINE FIELD time ON comments TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX comments_media ON comments FIELDS media;

-- Media concepts (unprocessed uploads)
DEFINE TABLE media_concepts SCHEMAFULL;
DEFINE FIELD name ON media_concepts TYPE string;
DEFINE FIELD owner ON media_concepts TYPE string;
DEFINE FIELD processed ON media_concepts TYPE bool DEFAULT false;
DEFINE FIELD type ON media_concepts TYPE string;

DEFINE INDEX concepts_owner ON media_concepts FIELDS owner;

-- Lists (playlists)
DEFINE TABLE lists SCHEMAFULL;
DEFINE FIELD name ON lists TYPE string;
DEFINE FIELD owner ON lists TYPE string;
DEFINE FIELD visibility ON lists TYPE string DEFAULT 'hidden';
DEFINE FIELD restricted_to_group ON lists TYPE option<string>;
DEFINE FIELD created ON lists TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX lists_owner ON lists FIELDS owner;

-- User groups for restricted access
DEFINE TABLE user_groups SCHEMAFULL;
DEFINE FIELD name ON user_groups TYPE string;
DEFINE FIELD owner ON user_groups TYPE string;
DEFINE FIELD created ON user_groups TYPE int DEFAULT time::unix(time::now());

DEFINE INDEX groups_owner ON user_groups FIELDS owner;

-- ============================================================
-- GRAPH EDGES (Relations)
-- ============================================================

-- Subscriptions: users -> subscribes -> users
DEFINE TABLE subscribes SCHEMAFULL TYPE RELATION FROM users TO users;

-- Group membership: user_groups -> has_member -> users
DEFINE TABLE has_member SCHEMAFULL TYPE RELATION FROM user_groups TO users;

-- List items: lists -> list_contains -> media (with position and timestamp)
DEFINE TABLE list_contains SCHEMAFULL TYPE RELATION FROM lists TO media;
DEFINE FIELD position ON list_contains TYPE int DEFAULT 0;
DEFINE FIELD added ON list_contains TYPE int DEFAULT time::unix(time::now());

-- Reactions: users -> reacts -> media (with reaction type)
DEFINE TABLE reacts SCHEMAFULL TYPE RELATION FROM users TO media;
DEFINE FIELD reaction ON reacts TYPE string;

-- ============================================================
-- Migration notes:
-- - PostgreSQL `subscriptions` table -> `subscribes` graph edge
-- - PostgreSQL `user_group_members` table -> `has_member` graph edge
-- - PostgreSQL `list_items` table -> `list_contains` graph edge
-- - PostgreSQL `media_likes` table -> `reacts` graph edge
-- - `public` boolean column removed (was legacy, replaced by `visibility`)
-- - Record IDs encode table + key (e.g. media:abc123 instead of id='abc123')
-- - Graph traversals replace JOINs (e.g. ->subscribes->users, <-reacts)
-- ============================================================
