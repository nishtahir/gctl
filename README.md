# gctl - Ghost Content Management CLI

A command-line interface tool for managing Ghost blog content through the Ghost Admin API.

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

## License

MIT License
