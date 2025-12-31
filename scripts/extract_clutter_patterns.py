#!/usr/bin/env python3
"""
Extract clutter patterns from Defuddle's constants.ts into a JSON manifest.

Usage:
    python scripts/extract_clutter_patterns.py tmp/defuddle/src/constants.ts > crates/pw-cli/src/clutter.json
"""

import re
import json
import sys
from pathlib import Path


def extract_array(name: str, content: str) -> list[str]:
    """Extract a JS array from TypeScript content."""
    # Match patterns like: export const NAME = [ ... ]; or [ ... ].join(',');
    pattern = rf"export const {name}\s*=\s*\[(.*?)\](?:\.join\(['\"],?['\"]\))?;"
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        # Try Set pattern: new Set([ ... ])
        pattern = rf"export const {name}\s*=\s*new Set\(\[(.*?)\]\)"
        match = re.search(pattern, content, re.DOTALL)
    if not match:
        return []

    array_content = match.group(1)
    
    # Check if it's a single-line array (comma-separated on one line)
    if "\n" not in array_content.strip() or array_content.count("'") > 4:
        # Single-line format: 'item1', 'item2', 'item3'
        items = re.findall(r"'([^']+)'", array_content)
        if not items:
            items = re.findall(r'"([^"]+)"', array_content)
        return items
    
    # Multi-line format: split by lines and extract complete string literals
    items = []
    for line in array_content.split("\n"):
        line = line.strip()
        # Skip comments and empty lines
        if not line or line.startswith("//"):
            continue
        # Match single or double quoted strings that span the whole value
        # Handle strings like '[role="article"]' or "simple"
        m = re.match(r"^['\"](.+?)['\"],?\s*(?://.*)?$", line)
        if m:
            items.append(m.group(1))
    return items


def categorize_partial_selectors(selectors: list[str]) -> dict[str, list[str]]:
    """Group partial selectors into categories for readability."""
    categories = {
        "ads": [],
        "navigation": [],
        "header_footer": [],
        "sidebar": [],
        "social": [],
        "comments": [],
        "auth": [],
        "newsletter": [],
        "article_meta": [],
        "post_meta": [],
        "author": [],
        "related": [],
        "misc": [],
    }

    # Keywords for categorization
    category_keywords = {
        "ads": ["ad", "advert", "promo", "sponsor", "banner"],
        "navigation": ["nav", "menu", "breadcrumb", "pagination", "skip", "jump"],
        "header_footer": ["header", "footer", "copyright", "masthead", "topbar"],
        "sidebar": ["sidebar", "widget", "aside", "rail"],
        "social": ["social", "share", "facebook", "twitter", "instagram", "rss"],
        "comments": ["comment", "disqus", "discuss", "feedback", "response"],
        "auth": ["login", "sign", "register", "access-wall", "paywall", "gated"],
        "newsletter": ["newsletter", "subscribe", "signup", "email", "donate"],
        "article_meta": ["article-", "article_", "article__"],
        "post_meta": ["post-", "post_", "entry-", "byline", "dateline", "timestamp", "pub"],
        "author": ["author", "bio", "avatar", "profile", "contributor"],
        "related": ["related", "recommend", "more-", "read-next", "keep-reading", "popular", "trending", "recent"],
    }

    for selector in selectors:
        sel_lower = selector.lower()
        categorized = False

        for category, keywords in category_keywords.items():
            if any(kw in sel_lower for kw in keywords):
                categories[category].append(selector)
                categorized = True
                break

        if not categorized:
            categories["misc"].append(selector)

    # Remove empty categories
    return {k: v for k, v in categories.items() if v}


def extract_js_array_simple(name: str, content: str) -> list[str]:
    """Extract a simple JS array (const name = [...]) from content."""
    pattern = rf"const {name}\s*=\s*\[(.*?)\];"
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        return []
    
    array_content = match.group(1)
    items = []
    for line in array_content.split("\n"):
        line = line.strip()
        if not line or line.startswith("//"):
            continue
        m = re.match(r"^['\"](.+?)['\"],?\s*(?://.*)?$", line)
        if m:
            items.append(m.group(1))
    return items


def main():
    if len(sys.argv) < 2:
        # Default path
        input_path = Path("tmp/defuddle/src/constants.ts")
    else:
        input_path = Path(sys.argv[1])

    if not input_path.exists():
        print(f"Error: {input_path} not found", file=sys.stderr)
        sys.exit(1)

    content = input_path.read_text()

    # Also try to read scoring.ts for additional patterns
    scoring_path = input_path.parent / "scoring.ts"
    scoring_content = ""
    if scoring_path.exists():
        scoring_content = scoring_path.read_text()

    # Extract all arrays
    partial_selectors = extract_array("PARTIAL_SELECTORS", content)

    manifest = {
        "$comment": "Web clutter patterns extracted from Defuddle (https://github.com/kepano/defuddle)",
        "content_selectors": {
            "$comment": "Selectors for finding main content, in priority order",
            "selectors": extract_array("ENTRY_POINT_ELEMENTS", content),
        },
        "remove": {
            "$comment": "Elements to remove from the page",
            "exact_selectors": extract_array("EXACT_SELECTORS", content),
            "partial_patterns": {
                "$comment": "Substring patterns matched against class/id/data-* attributes (case-insensitive)",
                "check_attributes": extract_array("TEST_ATTRIBUTES", content),
                "patterns": categorize_partial_selectors(partial_selectors),
            },
        },
        "preserve": {
            "$comment": "Elements to preserve during content extraction",
            "block_elements": extract_array("BLOCK_ELEMENTS", content),
            "preserve_elements": extract_array("PRESERVE_ELEMENTS", content),
            "inline_elements": extract_array("INLINE_ELEMENTS", content),
            "allowed_empty": extract_array("ALLOWED_EMPTY_ELEMENTS", content),
            "allowed_attributes": extract_array("ALLOWED_ATTRIBUTES", content),
        },
        "footnotes": {
            "$comment": "Selectors for identifying footnotes and citations",
            "inline_references": extract_array("FOOTNOTE_INLINE_REFERENCES", content),
            "list_selectors": extract_array("FOOTNOTE_LIST_SELECTORS", content),
        },
        "scoring": {
            "$comment": "Patterns used for content scoring (from scoring.ts)",
            "content_indicators": extract_js_array_simple("contentIndicators", scoring_content),
            "navigation_indicators": extract_js_array_simple("navigationIndicators", scoring_content),
            "non_content_patterns": extract_js_array_simple("nonContentPatterns", scoring_content),
        },
    }

    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
