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
codeinjection_head: |    # optional: HTML added to the page's <head>
  <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
codeinjection_foot: {file: _snippets/charts.html}   # or read it from a file
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

**Code injection.** `codeinjection_head` and `codeinjection_foot` set the
post's Ghost code injection. Write the HTML inline (use YAML block scalars,
`|`, for multi-line code) or point at a file with `{file: path}`, so one
snippet can serve many posts. The file is read verbatim, relative to the post,
and follows the same rules as includes: it must be inside the pushed directory
(or `--include-root`), and a `_`-prefixed name keeps it from being pushed as a
post. Leaving a key out keeps whatever the post already has; an empty value
(`codeinjection_head: ""`, or an empty file) clears it. `gctl pull` writes the
code inline, byte-for-byte. This code runs on your live site, so only push
injection from files you trust.

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

## Including other files

Reuse content across posts with an include directive on a line of its own:

```markdown
{{< include "../_snippets/footer.md" >}}
{{< include "../_snippets/hello.rs" lang="rust" >}}
```

- A markdown file is spliced in at that spot as ordinary blocks. Its own front
  matter is ignored, and it can include other files in turn (up to 10 deep;
  cycles are an error).
- With `lang="..."` the file is inserted as a fenced code block instead, read
  verbatim (`lang=""` for no language).
- A `.html`/`.htm` file is inserted as a single HTML card, verbatim. Use this
  for interactive demos and embeds: markup written inline in markdown ends at
  its first blank line, whereas an included file is never split. (Inline HTML
  blocks may be indented up to three spaces.)
- Paths are relative to the file containing the directive.
- Includes can only read files **inside the directory being pushed** (for a
  single file, its folder). This keeps a shared markdown file from pulling
  arbitrary files from your machine into a post; `..` and symlinks that leave
  the directory are refused. Use `--include-root <dir>` to allow a wider
  directory on purpose.
- Name snippet files or folders starting with `_` (`_snippets/`) so `gctl push`
  doesn't publish them as posts.
- The directive only works on its own line, outside code blocks, quotes and
  lists. Elsewhere it is kept as visible text, never silently dropped.

Includes are resolved when you push, so `gctl pull` writes the flattened
content.

## Pulling content

`gctl pull` is the reverse of `push`: it writes the site's posts down as
markdown files with front matter, so you can edit them locally and push the
changes back.

```bash
gctl pull                       # every post into ./content/<slug>.md
gctl pull posts --tag rust      # filter like `post list` (--status, --author, --filter, -n)
gctl pull --dry-run             # show what would be written
```

Each file starts with the post's Ghost `id`, so the next `gctl push` updates
that post. Only posts with Lexical content are pulled; posts still stored as
mobiledoc are skipped and listed as such.

**Existing files are never silently overwritten.** A file is left alone when it
already says what the post says (formatting markdown ignores, such as spacing
or `_` versus `*`, doesn't count, and neither does a front matter field the
file doesn't set). Otherwise it is reported as a `conflict` and the command
exits non-zero; `--force` overwrites it. Files are matched to posts by `id`, so
renaming a pulled file is safe.

**Lossy content.** Some things Ghost can store have no markdown form:
callouts, bookmarks, embeds and file cards, and markdown-card syntax this tool
doesn't parse (footnotes, tables, task lists). Pull approximates them (a
callout becomes a quote, a bookmark a link) and lists the kinds under `lossy:`
in the file's front matter. `gctl push` refuses such a file until you review it
and delete that line, or pass `--force`. The paywall divider survives a round
trip as `<!--members-only-->`.

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
