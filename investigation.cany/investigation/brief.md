# Brief

**Question:** Build a desktop microblogging app (Rust/egui) that writes Markdown articles with frontmatter (title, date, tags, abstract) into Content/YYYY/MM/DD/<id>/, shows a tag sidebar, tag search and a scrollable list (title + 3-line abstract), Markdown editing with preview, snippets, JSON transaction logs per day, and button-triggered GitHub sync (pull + push), Hugo-compatible as stage 2.

**Purpose:** Personal microblogging tool for a non-coder, later synced with a Hugo website.

**Targets:**
- none - build from request

**Constraints:**
- Rust + eframe/egui, cargo project in app/
- Change files only in app/
- --screenshot option required

**Done when:** cargo build succeeds, screenshot shows the main view with sidebar tags, search, list and editor/preview; sync and transaction files work.
