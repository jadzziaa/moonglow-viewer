//! The gallery's page: one self-contained `index.html` (no network, no
//! scripts beyond a search filter), light or dark with the system.

use crate::Entry;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const STYLE: &str = r#"
:root { --bg: #f6f6f4; --card: #ffffff; --text: #1d1d1b; --weak: #6b6b66; --line: #deded8; --err: #b3261e; }
@media (prefers-color-scheme: dark) {
  :root { --bg: #1b1d21; --card: #25282d; --text: #e4e4e0; --weak: #9a9a94; --line: #34373d; --err: #f2b8b5; }
}
* { box-sizing: border-box; }
body { margin: 0; padding: 16px; background: var(--bg); color: var(--text);
  font: 14px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; }
header { display: flex; flex-wrap: wrap; gap: 12px; align-items: baseline; margin-bottom: 16px; }
h1 { font-size: 20px; margin: 0; }
.count { color: var(--weak); }
input { flex: 1 1 200px; max-width: 360px; padding: 6px 10px; border: 1px solid var(--line);
  border-radius: 6px; background: var(--card); color: var(--text); font: inherit; }
.grid { display: grid; gap: 12px; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); }
figure { margin: 0; background: var(--card); border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
figure img { display: block; width: 100%; height: auto; aspect-ratio: 1; object-fit: contain; background: #2a2e35; }
.missing { display: flex; align-items: center; justify-content: center; aspect-ratio: 1; color: var(--err);
  padding: 8px; text-align: center; font-size: 12px; }
figcaption { padding: 6px 8px; font-size: 12px; overflow-wrap: anywhere; }
figcaption .meta { color: var(--weak); }
"#;

const SCRIPT: &str = r#"
const q = document.getElementById('q');
const cards = Array.from(document.querySelectorAll('figure'));
const count = document.getElementById('count');
q.addEventListener('input', () => {
  const t = q.value.toLowerCase();
  let n = 0;
  for (const c of cards) { const ok = c.dataset.k.includes(t); c.hidden = !ok; if (ok) n++; }
  count.textContent = n + ' shown';
});
"#;

/// The page for a gallery's entries.
pub fn index_html(title: &str, entries: &[Entry]) -> String {
    let mut cards = String::new();
    for e in entries {
        let meta = match (&e.info["classification"], &e.info["faces"]) {
            (serde_json::Value::String(c), serde_json::Value::Number(f)) => {
                format!("{c} · {f} faces")
            }
            _ => String::new(),
        };
        let key =
            format!("{} {} {}", e.item.id, e.item.label, e.item.group.as_deref().unwrap_or(""))
                .to_lowercase();
        let picture = match (&e.image, &e.error) {
            (Some(img), _) => format!(
                r#"<img src="{}" alt="{}" loading="lazy" width="512" height="512">"#,
                escape(img),
                escape(&e.item.label)
            ),
            (None, Some(err)) => format!(r#"<div class="missing">{}</div>"#, escape(err)),
            (None, None) => r#"<div class="missing">no picture</div>"#.to_string(),
        };
        cards.push_str(&format!(
            "<figure data-k=\"{}\">{picture}<figcaption>{}<br><span class=\"meta\">{}</span></figcaption></figure>\n",
            escape(&key),
            escape(&e.item.label),
            escape(&meta),
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{t}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<header><h1>{t}</h1>\
<span class=\"count\" id=\"count\">{n} pictures</span>\
<input id=\"q\" type=\"search\" placeholder=\"Filter by name\" aria-label=\"Filter by name\"></header>\n\
<main class=\"grid\">\n{cards}</main>\n<script>{SCRIPT}</script>\n</body>\n</html>\n",
        t = escape(title),
        n = entries.len(),
    )
}
