//! Sample setup for the screenshots in `doc/` (hidden option `--doc-fixture <dir>`).
//!
//! Builds, inside `<dir>` (used as HOME while taking the screenshots):
//!   blog/hugo.toml, blog/content/…            an existing Hugo blog with a few articles
//!   Documents/hmb/…                           the app's own data folder (filled by a full sync)
//!   Library/Application Support/hmb/config.json
//! plus snippets "from the right-click menu", one local change and one new article from a second
//! computer ("laptop"), so that every step of the guide has something to show.

use crate::store::{self, Article, Config, ImageStyle, Layout, Ontology};
use crate::sync;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn article(cfg: &Config, rel: &str, title: &str, date: &str, tags: &[&str], abs: &str, body: &str) -> Article {
    let d = store::parse_date(date).unwrap();
    let rel_dir = format!("{}{rel}", cfg.root_prefix());
    let mut extra = BTreeMap::new();
    let cat = rel.split('/').next().unwrap_or("").to_string();
    extra.insert("categories".to_string(), serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(cat)]));
    Article {
        id: store::make_id(cfg, &rel_dir, &d),
        rel_dir,
        title: title.into(),
        date: d,
        tags: tags.iter().map(|s| s.to_string()).collect(),
        abstract_: abs.into(),
        body: body.into(),
        draft: false,
        extra,
    }
}

/// A small sketch-like picture for the article with an image.
fn sketch_png(path: &Path) {
    let (w, h) = (640u32, 360u32);
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let t = y as f32 / h as f32;
        let mut c = [
            (219.0 + 20.0 * t) as u8,
            (234.0 - 10.0 * t) as u8,
            254u8,
            255u8,
        ];
        // three "cards" and connecting line, like a whiteboard sketch
        for (i, cx) in [120u32, 320, 520].iter().enumerate() {
            let in_card = x + 80 > *cx && x < cx + 80 && y > 120 && y < 240;
            if in_card {
                c = [255, 255, 255, 255];
                if y < 145 {
                    c = match i {
                        0 => [37, 99, 235, 255],
                        1 => [22, 163, 74, 255],
                        _ => [217, 119, 6, 255],
                    };
                }
            }
        }
        if (178..=182).contains(&y) && ((200..240).contains(&x) || (400..440).contains(&x)) {
            c = [17, 24, 39, 255];
        }
        *p = image::Rgba(c);
    }
    let _ = img.save(path);
}

pub fn build(dir: &Path) -> Result<(), String> {
    let _ = fs::remove_dir_all(dir);
    let blog = dir.join("blog");
    let app_data = dir.join("Documents/hmb");
    fs::create_dir_all(blog.join("content")).map_err(|e| e.to_string())?;
    fs::write(
        blog.join("hugo.toml"),
        "baseURL = 'https://example.org/'\nlanguageCode = 'de-de'\ntitle = 'Mein Blog'\n",
    )
    .map_err(|e| e.to_string())?;

    // 1) the existing Hugo blog
    let blog_cfg = Config {
        data_dir: blog.clone(),
        content_subdir: "content".into(),
        layout: Layout::Sections,
        image_style: ImageStyle::Figure,
        client: "laptop".into(),
        ..Default::default()
    };
    let posts = [
        article(&blog_cfg, "notizen/wochenmarkt", "Wochenmarkt am Samstag", "2026-11-21 10:30", &["alltag"],
            "Äpfel, Kürbis und frisches Brot. Der Stand mit dem Bergkäse hatte wieder die längste Schlange.",
            "Nächste Woche gibt es dort auch Maronen.\n"),
        article(&blog_cfg, "technik/hugo/page-bundles", "Page Bundles in Hugo", "2026-11-19 21:15", &["hugo", "web"],
            "Jeder Artikel bekommt einen eigenen Ordner mit index.md – Bilder liegen direkt daneben. Das hält alles zusammen.",
            "## Aufbau\n\n```\ncontent/technik/hugo/page-bundles/\n├── index.md\n└── bild.png\n```\n"),
        article(&blog_cfg, "technik/rust-egui", "Rust und egui ausprobiert", "2026-11-17 20:05", &["rust", "programmieren"],
            "Ein kleines Fenster mit Seitenleiste, Liste und Editor. Erstaunlich wenig Code für eine Oberfläche, die sich schnell anfühlt.",
            "```rust\nfn main() {\n    println!(\"Hallo\");\n}\n```\n"),
        article(&blog_cfg, "lesen/gute-tags", "Lesenotiz: Gute Tags", "2026-11-14 07:45", &["lesen", "notizen"],
            "Tags sind frei vergebbar und lassen sich später ordnen – \"rust\" unter \"programmieren\", \"hugo\" unter \"web\".",
            "Weniger, aber besser.\n"),
    ];
    for p in &posts {
        store::save_article(&blog_cfg, p)?;
    }
    let mut onto = Ontology::default();
    onto.parents.insert("rust".into(), "programmieren".into());
    onto.parents.insert("hugo".into(), "web".into());
    onto.save(&blog)?;

    // 2) the app's own data folder, configured for that blog
    let mut cfg = Config {
        data_dir: app_data.clone(),
        content_subdir: "content".into(),
        layout: Layout::Sections,
        default_section: "notizen".into(),
        image_style: ImageStyle::Figure,
        image_width: 600,
        sync_dir: blog.join("content"),
        client: "mac".into(),
        ..Default::default()
    };
    fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
    let r = sync::full_sync(&cfg)?;
    cfg.last_tx = r.last_tx;
    cfg.last_tx_files = r.last_tx_files;
    cfg.last_sync = Some("2026-11-22T18:00:00+01:00".into());

    // 3) snippets collected with the right-click menu
    let meta = cfg.meta_dir();
    let snips = [
        ("2026-11-23T08:12:00+01:00", "Hugo baut die Seite in Millisekunden – auch mit Hunderten Artikeln."),
        ("2026-11-23T08:40:00+01:00", "> Schreiben ist Denken auf Papier."),
        ("2026-11-23T09:05:00+01:00", "Page Bundles: Bilder liegen direkt neben index.md."),
    ];
    let all: Vec<store::Snippet> = snips
        .iter()
        .rev()
        .map(|(c, t)| store::Snippet { created: c.to_string(), text: t.to_string(), source: "Rechtsklick".into() })
        .collect();
    store::save_snippets(&meta, &all)?;

    // 4) a new article written in the app, with a picture and a snippet
    let mut a = article(&cfg, "notizen/skizze-vom-workshop", "Skizze vom Workshop", "2026-11-23 09:20", &["notizen", "web"],
        "Drei Karten, zwei Pfeile: So sah der Ablauf am Ende aus. Das Foto der Tafel liegt direkt beim Artikel.",
        "## Ergebnis\n\n{{< figure src=\"skizze.png\" width=\"600\" alt=\"skizze\" >}}\n\nPage Bundles: Bilder liegen direkt neben index.md.\n\n- **Planen** – was soll rein?\n- **Schreiben** – kurz und in Markdown\n- **Veröffentlichen** – synchronisieren, pushen\n");
    a.extra.insert("author".into(), serde_yaml::Value::String("Autorin".into()));
    store::save_article(&cfg, &a)?;
    sketch_png(&a.dir(&cfg.data_dir).join("skizze.png"));
    store::log_tx(&cfg, &a, "created")?;

    // 5) meanwhile a second computer added an article to the blog and wrote a transaction file
    let from_laptop = article(&blog_cfg, "lesen/buchtipp", "Buchtipp für den Winter", "2026-11-23 07:50", &["lesen"],
        "Ein dickes Buch, eine Kanne Tee und ein freier Sonntag – mehr braucht es nicht.",
        "Leseprobe folgt.\n");
    store::save_article(&blog_cfg, &from_laptop)?;
    let entry = store::TxEntry {
        time: "2026-11-23T07:55:00+01:00".into(),
        id: from_laptop.id.clone(),
        action: "created".into(),
        path: blog_cfg.content_rel(&from_laptop.rel_dir),
        title: from_laptop.title.clone(),
        client: "laptop".into(),
    };
    store::write_tx_file(&blog, "laptop", &[entry])?;

    // settings of the app (HOME = dir while taking screenshots)
    let cfg_path = dir.join("Library/Application Support/hmb/config.json");
    fs::create_dir_all(cfg_path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&cfg_path, serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("Beispiel aufgebaut in {}", dir.display());
    Ok(())
}
