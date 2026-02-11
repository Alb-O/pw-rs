# pw page

The `pw page` subcommand provides various methods for extracting content from web pages.

## Extract Page Content

```bash
pw page text https://example.com -s "article"           # text content
pw page html https://example.com -s "article"           # HTML content
pw page eval https://example.com "document.title"       # run JavaScript
```

## Extract Readable Content (articles, docs)

Use `pw page read` to extract the main content from a page, automatically removing ads, navigation, sidebars, and other clutter:

```bash
pw page read https://example.com                        # markdown (default)
pw page read https://example.com -o text                # plain text
pw page read https://example.com -o html                # cleaned HTML
pw page read https://example.com -m                     # include metadata
pw -f text page read https://example.com                # output content directly (not JSON)
```

This is ideal for reading articles, documentation, or any page where you want the content without the noise.

## Get Full Page Context (snapshot)

Use `pw page snapshot` to get a comprehensive page model in one call - URL, title, interactive elements, and visible text:

```bash
pw page snapshot https://example.com              # full page model
pw page snapshot --text-only                      # skip elements (faster)
pw page snapshot --full                           # include all text (not just visible)
pw page snapshot --max-text-length 10000          # increase text limit
```

This is ideal for AI agents that need full page context without multiple round-trips. The output includes:

- Page URL and title
- Viewport dimensions
- All interactive elements (buttons, links, inputs) with stable selectors
- Visible text content

## Page Commands (batch mode)

When using batch mode (`pw run`), the following page commands are available:

- `page.text` - args: `url`, `selector`
- `page.html` - args: `url`, `selector`
- `page.eval` - args: `url`, `expression`
- `page.elements` - args: `url`, `wait`, `timeout_ms`
- `page.snapshot` - args: `url`, `text_only`, `full`, `max_text_length`
- `page.console` - args: `url`, `timeout_ms`
- `page.read` - args: `url`, `output_format`, `metadata`
- `page.coords` - args: `url`, `selector`
- `page.coords_all` - args: `url`, `selector`
