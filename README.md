# gctl - Ghost Content Management CLI

A command-line interface tool for managing Ghost blog content through the Ghost Admin API.

## Features

- Post management (create, read, update, delete)
- Environment-based configuration
- Error handling with custom error types
- Support for Ghost Admin API v5.0
- JWT authentication

## Installation

```bash
cargo install gctl
```

## Configuration

The CLI can be configured using environment variables or command-line arguments:

- `GHOST_URL`: Your Ghost blog URL (e.g., `https://yourblog.ghost.io`)
- `GHOST_ADMIN_KEY`: Your Ghost Admin API key

You can set these in a `.env` file:

```env
GHOST_URL=https://yourblog.ghost.io
GHOST_ADMIN_KEY=your_admin_key
```

Or pass them as command-line arguments:

```bash
gctl --url https://yourblog.ghost.io --key your_admin_key [command]
```

## Usage

### List Posts
```bash
gctl post list
```

### Get Post Details
```bash
gctl post get <post-id>
```

### Create Post
```bash
gctl post create "Post Title" "Post Content"
```

### Update Post
```bash
gctl post update <post-id> --title "New Title" --content "New Content"
```

### Delete Post
```bash
gctl post delete <post-id>
```

## Ghost Admin API Key

To get your Ghost Admin API key:

1. Log in to your Ghost Admin panel
2. Go to Settings → Integrations
3. Click "Add custom integration"
4. Give it a name (e.g., "gctl")
5. Copy the Admin API Key

The Admin API Key format is: `{key-id}:{key-secret}`

## Development

### Building from Source

```bash
git clone https://github.com/yourusername/gctl
cd gctl
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Project Structure

- `gctl-bin/`: CLI application
- `gctl-ghost/`: Ghost API client library
  - `src/api.rs`: Main API client implementation
  - `src/posts.rs`: Post management functionality
  - `src/error.rs`: Error types
  - `src/options.rs`: Configuration options

## Publishing markdown

`gctl push` (alias of `gctl publish`) works like a static site generator's
content directory: write posts as markdown files with YAML front matter, then
push them to update Ghost.

```
content/
  hello-world.md
  my-series/
    part-one.md
    index.md          # slug comes from the directory name
```

```markdown
---
title: "Hello, World"
slug: hello-world        # optional, defaults to the file name
status: published        # draft | published | scheduled; or `draft: false`
date: 2024-01-31         # optional; a future date schedules the post
tags: [rust, wasm]
description: A short summary
image: https://example.com/cover.png
---

Body in markdown...
```

```bash
gctl push                    # pushes ./content
gctl push posts/hello.md     # a single file
gctl push --dry-run          # show what would be created/updated, change nothing
```

**Status.** It comes from the front matter: `status:` if present, otherwise
`draft: true|false`. With neither, a *new* post is created as a Ghost draft, so
nothing goes live by accident, and an *existing* post keeps whatever status it
has. Drafts are pushed too, which means setting a published post's front matter
back to `draft: true` unpublishes it.

**Identity.** The first push writes the post's Ghost `id` into the file's
front matter (nothing else in the file changes). Later pushes update that post
by id, so you can freely rename the file or change `slug:` without creating a
duplicate. A file without an id is matched by slug, and adopts the post it
finds. Commit the id along with the file.

**Source of truth.** The files win: a push overwrites the post's title, slug,
content, tags and other front-matter fields in Ghost, including edits made in
the Ghost editor. Removing a file does not delete its post.

The markdown is parsed with a [pest](https://pest.rs) grammar and compiled to
Ghost's Lexical format (headings, paragraphs, bold/italic/strikethrough/code,
links, images, nested lists, quotes, code blocks, rules and raw HTML blocks).
Files that start with `_` or `.` are skipped.

Not supported yet: tables, footnotes, task lists, setext headings, shortcodes,
and uploading local images (use absolute image URLs for now). Tables are best
written as an HTML block.

## License

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
