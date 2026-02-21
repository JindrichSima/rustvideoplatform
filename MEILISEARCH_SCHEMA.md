# Meilisearch Index Schema for Media Search

This document describes the Meilisearch index configuration required by the search module.
The indexer module should use this as a reference when populating the Meilisearch index.

## Configuration

Add these fields to `config.json`:

```json
{
    "meilisearch_url": "http://localhost:7700",
    "meilisearch_key": "your-master-key-here"
}
```

`meilisearch_key` is optional (can be `null` or omitted) for development environments.

## Index Name

**`media`**

## Primary Key

**`id`** (the media ID string)

## Document Schema

Each document in the index must have the following fields:

| Field       | Type    | Description                                     |
|-------------|---------|------------------------------------------------|
| `id`        | String  | Media ID (primary key)                          |
| `name`      | String  | Media title/name                                |
| `owner`     | String  | Creator's login username                        |
| `views`     | Integer | View count                                      |
| `likes`     | Integer | Like count                                      |
| `dislikes`  | Integer | Dislike count                                   |
| `type`      | String  | Media type: `"video"`, `"audio"`, or `"picture"`|
| `upload`    | Integer | Upload timestamp (Unix epoch seconds)           |
| `public`    | Boolean | Whether the media is publicly visible           |

### Example Document

```json
{
    "id": "abc1234xyz",
    "name": "My Cool Video",
    "owner": "johndoe",
    "views": 1500,
    "likes": 42,
    "dislikes": 3,
    "type": "video",
    "upload": 1708531200,
    "public": true
}
```

## Required Index Settings

The indexer must configure these settings on the `media` index:

### Searchable Attributes

```json
["name", "owner"]
```

`name` should be the primary searchable field, `owner` allows searching by creator.

### Filterable Attributes

```json
["public", "type", "upload", "views", "likes"]
```

These are used by the search filters:
- `public` - Always filtered to `true` in search queries
- `type` - Filter by media type (video/audio/picture)
- `upload` - Filter by upload date range (timestamp comparisons)
- `views` - Used for sorting by most viewed
- `likes` - Used for sorting by most liked

### Sortable Attributes

```json
["upload", "views", "likes"]
```

Used for sort options:
- `upload:desc` - Newest first
- `upload:asc` - Oldest first
- `views:desc` - Most viewed
- `likes:desc` - Most liked

### Ranking Rules (recommended)

```json
["words", "typo", "proximity", "attribute", "sort", "exactness"]
```

This is the default Meilisearch ranking, which provides good relevance out of the box.

### Typo Tolerance

Meilisearch has typo tolerance enabled by default. No extra configuration needed.

## Indexing Strategy

The indexer should:

1. **Initial sync**: Fetch all records from the `media` PostgreSQL table and bulk-index them
2. **Incremental updates**: When media is created/updated/deleted, update the corresponding Meilisearch document
3. **Only index public-relevant fields**: The `description` field (jsonb) is not indexed since it contains rich text delta format - it can be added later if plain text extraction is implemented

### SQL Query for Full Sync

```sql
SELECT id, name, owner, views, likes, dislikes, type, upload, public
FROM media;
```

### Rust Struct for Serialization

```rust
#[derive(Serialize, Deserialize)]
struct MeiliMedia {
    id: String,
    name: String,
    owner: String,
    views: i64,
    likes: i64,
    dislikes: i64,
    r#type: String,
    upload: i64,
    public: bool,
}
```

## Index Setup Code Example

```rust
use meilisearch_sdk::client::Client;

async fn setup_media_index(client: &Client) {
    let media = client.index("media");

    // Set primary key
    media.set_primary_key("id").await.unwrap();

    // Configure searchable attributes
    media.set_searchable_attributes(["name", "owner"]).await.unwrap();

    // Configure filterable attributes
    media.set_filterable_attributes(["public", "type", "upload", "views", "likes"]).await.unwrap();

    // Configure sortable attributes
    media.set_sortable_attributes(["upload", "views", "likes"]).await.unwrap();
}
```
