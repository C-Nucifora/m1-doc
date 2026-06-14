//! Renders Markdown files (produced by [`crate::markdown`]) to a self-contained
//! HTML site.  Each `.md` file becomes a `.html` file; intra-doc links are
//! rewritten from `*.md` to `*.html`.  External `http(s)://` links are left
//! untouched.  The only inputs are [`crate::markdown::RenderedFile`] slices and
//! a [`crate::model::DocModel`] (for the sidebar and page title).  No m1-core /
//! m1-typecheck types cross this module boundary.

use crate::markdown::RenderedFile;
use crate::model::{DocModel, SymbolDocKind};

// ---------------------------------------------------------------------------
// Internal CSS
// ---------------------------------------------------------------------------
//
// All styling is inline (no external stylesheet, font, or CDN) so the site is
// self-contained and works from `file://` and GitHub Pages alike (#33). Colours
// are expressed through CSS custom properties; dark mode flips the variables
// via `prefers-color-scheme` and an explicit `[data-theme]` override the toggle
// sets (persisted in localStorage by the inline script).

const STYLE: &str = r#"
:root{
  --bg:#fff;--fg:#1a1a1a;--muted:#666;--link:#0055cc;
  --nav-bg:#f4f4f4;--border:#ddd;--th-bg:#f0f0f0;--row-alt:#fafafa;
  --code-bg:#eef;--pre-bg:#f6f6f6;--accent:#0055cc;
  --kw:#a626a4;--str:#50a14f;--num:#986801;--com:#a0a1a7;--fn:#4078f2;
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --bg:#1a1b1e;--fg:#e6e6e6;--muted:#9aa0a6;--link:#6aa9ff;
    --nav-bg:#222428;--border:#3a3d42;--th-bg:#2a2c30;--row-alt:#212327;
    --code-bg:#2a2c30;--pre-bg:#202225;--accent:#6aa9ff;
    --kw:#c678dd;--str:#98c379;--num:#d19a66;--com:#7f848e;--fn:#61afef;
  }
}
:root[data-theme="dark"]{
  --bg:#1a1b1e;--fg:#e6e6e6;--muted:#9aa0a6;--link:#6aa9ff;
  --nav-bg:#222428;--border:#3a3d42;--th-bg:#2a2c30;--row-alt:#212327;
  --code-bg:#2a2c30;--pre-bg:#202225;--accent:#6aa9ff;
  --kw:#c678dd;--str:#98c379;--num:#d19a66;--com:#7f848e;--fn:#61afef;
}
*,*::before,*::after{box-sizing:border-box}
body{margin:0;font-family:system-ui,sans-serif;font-size:16px;line-height:1.6;
     color:var(--fg);background:var(--bg);display:flex;min-height:100vh}
nav{width:260px;min-width:260px;background:var(--nav-bg);
    border-right:1px solid var(--border);padding:1rem;
    position:sticky;top:0;height:100vh;overflow-y:auto}
nav h2{font-size:.85rem;text-transform:uppercase;letter-spacing:.06em;
        color:var(--muted);margin:1rem 0 .5rem}
nav>a{display:block;font-size:.9rem;color:var(--link);text-decoration:none;
      padding:.15rem 0}
nav a:hover{text-decoration:underline}
nav ul{list-style:none;margin:.25rem 0;padding-left:.9rem}
nav>ul{padding-left:0}
nav li{font-size:.9rem;position:relative}
nav li a{color:var(--link);text-decoration:none}
/* Collapsible tree: a toggle caret precedes a node that has children. */
.tw{cursor:pointer;display:inline-block;width:1rem;color:var(--muted);
    user-select:none;text-align:center}
.tw::before{content:"▸"}
li.open>.tw::before{content:"▾"}
li.has-children>ul{display:none}
li.has-children.open>ul{display:block}
nav a.active{font-weight:700;text-decoration:underline}
main{flex:1;padding:1rem 3rem 3rem;max-width:60rem;min-width:0}
h1{font-size:1.8rem;border-bottom:2px solid var(--border);padding-bottom:.4rem}
h2{font-size:1.3rem;margin-top:2rem}
h3{font-size:1.1rem}
table{border-collapse:collapse;width:100%;margin:1rem 0}
th,td{border:1px solid var(--border);padding:.4rem .7rem;text-align:left}
th{background:var(--th-bg);font-weight:600}
tr:nth-child(even) td{background:var(--row-alt)}
a{color:var(--link)}
code{background:var(--code-bg);padding:.1em .3em;border-radius:3px;font-size:.9em}
pre code{background:none;padding:0}
pre{background:var(--pre-bg);padding:1rem;border-radius:4px;overflow-x:auto}
/* Permalink anchors revealed on heading hover. */
.permalink{margin-left:.4rem;color:var(--muted);text-decoration:none;
           opacity:0;font-weight:400}
h1:hover .permalink,h2:hover .permalink,h3:hover .permalink{opacity:1}
/* Sticky toolbar: breadcrumb sits in the page; this bar holds the controls. */
.toolbar{position:sticky;top:0;z-index:5;background:var(--bg);
         border-bottom:1px solid var(--border);padding:.5rem 0;margin-bottom:1rem;
         display:flex;gap:.5rem;align-items:center;flex-wrap:wrap}
.toolbar input[type=search]{flex:1;min-width:8rem;padding:.35rem .5rem;
         border:1px solid var(--border);border-radius:4px;background:var(--bg);
         color:var(--fg)}
.btn{padding:.35rem .6rem;border:1px solid var(--border);border-radius:4px;
     background:var(--nav-bg);color:var(--fg);cursor:pointer;font-size:.9rem}
#search-results{list-style:none;margin:.25rem 0 0;padding:0;
     border:1px solid var(--border);border-radius:4px;max-height:18rem;
     overflow-y:auto;background:var(--bg)}
#search-results:empty{display:none}
#search-results li{padding:0}
#search-results a{display:block;padding:.35rem .6rem;text-decoration:none;
     border-bottom:1px solid var(--border)}
#search-results a:hover{background:var(--nav-bg)}
#search-results .kind{color:var(--muted);font-size:.8rem;margin-left:.4rem}
/* Filter panel (security + tags) — a disclosure on group pages. */
.filters{border:1px solid var(--border);border-radius:4px;padding:.5rem .75rem;
     margin:0 0 1rem}
.filters summary{cursor:pointer;font-weight:600}
.filters label{display:inline-block;margin:.25rem .75rem .25rem 0;font-size:.9rem}
.toc{border:1px solid var(--border);border-radius:4px;padding:.5rem .75rem;
     margin:0 0 1.5rem}
.toc summary{cursor:pointer;font-weight:600}
.toc ul{margin:.4rem 0 0;padding-left:1.1rem}
.toc a{text-decoration:none}
tr.filtered{display:none}
/* M1 syntax highlighting tokens (inline, no highlight.js). */
.m1-kw{color:var(--kw)}
.m1-str{color:var(--str)}
.m1-num{color:var(--num)}
.m1-com{color:var(--com);font-style:italic}
.m1-fn{color:var(--fn)}
@media (max-width:760px){
  body{flex-direction:column}
  nav{width:100%;min-width:0;height:auto;position:static;
      border-right:none;border-bottom:1px solid var(--border)}
  nav.collapsed ul,nav.collapsed>a{display:none}
  main{padding:1rem 1.25rem 2rem}
}
"#;

// ---------------------------------------------------------------------------
// Search index (#31)
// ---------------------------------------------------------------------------
//
// A flat JSON array over every documented entity, emitted inline in each page
// so the client-side search needs no network and works from `file://`. Each
// record carries the deep-link target `<group>.html#<anchor>` (reusing the
// anchors `assign_anchors` already made page-unique), the entity kind, the
// owning group, and a short unit/quantity hint. The order is deterministic: we
// walk groups (already sorted) and, within each, members in their sorted model
// order.

/// One search record. `p` = full path, `k` = kind label, `g` = owning group,
/// `u` = unit/quantity hint (may be empty), `h` = `<group>.html#<anchor>` href.
/// Short field names keep the inline JSON compact on large projects.
struct SearchEntry {
    path: String,
    kind: &'static str,
    group: String,
    hint: String,
    href: String,
}

/// Minimal JSON-string escaping for the inline index: the characters that would
/// break a `"…"` literal or an inline `<script>` block. We deliberately escape
/// `<`/`/` (as `<` / `\/`) so the JSON can never contain a literal
/// `</script>` that would close the embedding element early.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '/' => out.push_str("\\/"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The page filename for a group path (`Root.Engine` → `Root.Engine.html`).
fn group_html(path: &str) -> String {
    format!("{path}.html")
}

/// A human label for a symbol kind, used in search results and the index.
fn symbol_kind_label(kind: SymbolDocKind) -> &'static str {
    match kind {
        SymbolDocKind::Channel => "channel",
        SymbolDocKind::Parameter => "parameter",
        SymbolDocKind::Constant => "constant",
    }
}

/// Collect every documented entity into search records, in deterministic order.
fn build_search_entries(model: &DocModel) -> Vec<SearchEntry> {
    let mut out = Vec::new();
    for g in &model.groups {
        let page = group_html(&g.path);
        for s in &g.symbols {
            // Prefer the display unit, fall back to the quantity, else empty.
            let hint = s
                .unit
                .clone()
                .or_else(|| s.quantity.clone())
                .unwrap_or_default();
            out.push(SearchEntry {
                path: s.path.clone(),
                kind: symbol_kind_label(s.kind),
                group: g.path.clone(),
                hint,
                href: format!("{page}#{}", s.anchor),
            });
        }
        for f in &g.functions {
            out.push(SearchEntry {
                path: f.path.clone(),
                kind: "function",
                group: g.path.clone(),
                hint: f.return_type.clone().unwrap_or_default(),
                href: format!("{page}#{}", f.anchor),
            });
        }
        for t in &g.tables {
            out.push(SearchEntry {
                path: t.path.clone(),
                kind: "table",
                group: g.path.clone(),
                hint: t.output_unit.clone().unwrap_or_default(),
                href: format!("{page}#{}", t.anchor),
            });
        }
        for o in &g.objects {
            out.push(SearchEntry {
                path: o.path.clone(),
                kind: "object",
                group: g.path.clone(),
                hint: o.class.clone().unwrap_or_default(),
                href: format!("{page}#{}", o.anchor),
            });
        }
        for m in &g.can_messages {
            out.push(SearchEntry {
                path: m.path.clone(),
                kind: "CAN message",
                group: g.path.clone(),
                hint: String::new(),
                href: format!("{page}#{}", m.anchor),
            });
            for sig in &m.signals {
                out.push(SearchEntry {
                    path: sig.path.clone(),
                    kind: "CAN signal",
                    group: g.path.clone(),
                    hint: sig.unit.clone().unwrap_or_default(),
                    href: format!("{page}#{}", sig.anchor),
                });
            }
        }
    }
    // Enums live on the shared reference page; link to their entry there.
    for e in &model.enums {
        out.push(SearchEntry {
            path: e.name.clone(),
            kind: "enum",
            group: "Enums".to_string(),
            hint: String::new(),
            href: format!("enums.html#{}", e.anchor),
        });
    }
    out
}

/// Render the search index as a compact inline JSON array. Deterministic order.
fn search_index_json(model: &DocModel) -> String {
    let mut json = String::from("[");
    for (i, e) in build_search_entries(model).iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"p\":\"{}\",\"k\":\"{}\",\"g\":\"{}\",\"u\":\"{}\",\"h\":\"{}\"}}",
            json_escape(&e.path),
            json_escape(e.kind),
            json_escape(&e.group),
            json_escape(&e.hint),
            json_escape(&e.href),
        ));
    }
    json.push(']');
    json
}

// ---------------------------------------------------------------------------
// Inline behaviour (#31 search, #33 polish, #34 filters)
// ---------------------------------------------------------------------------
//
// One vanilla-JS module, inlined in every page. No external script, no build
// step, CSP-friendly (a single inline <script>). It is defensive: every feature
// is guarded on the element existing, so the index page (no filter panel) and a
// group page (no enums) both run the same script without error.

const SCRIPT: &str = r##"
(function(){
"use strict";
// ---- dark-mode toggle, persisted in localStorage ----
function applyTheme(t){
  if(t==="dark"||t==="light"){document.documentElement.setAttribute("data-theme",t);}
  else{document.documentElement.removeAttribute("data-theme");}
}
try{var saved=localStorage.getItem("m1doc-theme");if(saved){applyTheme(saved);}}catch(e){}
function toggleTheme(){
  var cur=document.documentElement.getAttribute("data-theme");
  var mq=window.matchMedia&&window.matchMedia("(prefers-color-scheme:dark)").matches;
  var next=(cur?cur==="dark":mq)?"light":"dark";
  applyTheme(next);
  try{localStorage.setItem("m1doc-theme",next);}catch(e){}
}
// ---- collapsible nav tree ----
function initNav(){
  var lis=document.querySelectorAll("nav li");
  lis.forEach(function(li){
    if(li.querySelector(":scope > ul")){
      li.classList.add("has-children");
      var tw=document.createElement("span");
      tw.className="tw";
      tw.addEventListener("click",function(){li.classList.toggle("open");});
      li.insertBefore(tw,li.firstChild);
    }
  });
  // Highlight the active page and expand its ancestors.
  var here=location.pathname.split("/").pop()||"index.html";
  var active=document.querySelector('nav a[href="'+here+'"]');
  if(active){
    active.classList.add("active");
    var p=active.parentElement;
    while(p&&p.tagName!=="NAV"){
      if(p.tagName==="LI"){p.classList.add("open");}
      p=p.parentElement;
    }
  }
}
// ---- permalink anchors on headings ----
function initPermalinks(){
  document.querySelectorAll("main h1[id],main h2[id],main h3[id]").forEach(addLink);
  // Headings without an id but wrapping an <a id> (our symbol/function anchors).
  document.querySelectorAll("main h2,main h3").forEach(function(h){
    if(h.id)return;
    var a=h.querySelector("a[id]");
    if(a){h.id=a.id;addLink(h);}
  });
}
function addLink(h){
  if(h.querySelector(".permalink"))return;
  var a=document.createElement("a");
  a.className="permalink";a.href="#"+h.id;a.textContent="¶";
  a.title="Permalink";
  a.addEventListener("click",function(ev){
    if(navigator.clipboard){
      ev.preventDefault();
      var url=location.href.split("#")[0]+"#"+h.id;
      navigator.clipboard.writeText(url).catch(function(){});
      location.hash=h.id;
    }
  });
  h.appendChild(a);
}
// ---- in-page table of contents ----
function initToc(){
  var slot=document.getElementById("toc-slot");
  if(!slot)return;
  var heads=document.querySelectorAll("main h2[id],main h3[id]");
  if(heads.length<2){return;}
  var det=document.createElement("details");det.className="toc";det.open=true;
  var sum=document.createElement("summary");sum.textContent="On this page";
  det.appendChild(sum);
  var ul=document.createElement("ul");
  heads.forEach(function(h){
    var li=document.createElement("li");
    if(h.tagName==="H3"){li.style.marginLeft="1rem";}
    var a=document.createElement("a");a.href="#"+h.id;
    a.textContent=(h.textContent||"").replace(/¶$/,"").trim();
    li.appendChild(a);ul.appendChild(li);
  });
  det.appendChild(ul);slot.appendChild(det);
}
// ---- client-side search over the inline index ----
function initSearch(){
  var box=document.getElementById("search-box");
  var results=document.getElementById("search-results");
  var dataEl=document.getElementById("search-index");
  if(!box||!results||!dataEl)return;
  var index=[];
  try{index=JSON.parse(dataEl.textContent||"[]");}catch(e){index=[];}
  function esc(s){return (s||"").replace(/[&<>]/g,function(c){
    return c==="&"?"&amp;":c==="<"?"&lt;":"&gt;";});}
  function render(q){
    results.innerHTML="";
    q=q.trim().toLowerCase();
    if(!q)return;
    var hits=[];
    for(var i=0;i<index.length&&hits.length<50;i++){
      var e=index[i];
      var hay=(e.p+" "+e.g+" "+e.u).toLowerCase();
      if(hay.indexOf(q)!==-1){hits.push(e);}
    }
    hits.forEach(function(e){
      var li=document.createElement("li");
      var a=document.createElement("a");
      a.href=e.h;
      a.innerHTML=esc(e.p)+'<span class="kind">'+esc(e.k)+
        (e.u?" · "+esc(e.u):"")+'</span>';
      li.appendChild(a);results.appendChild(li);
    });
  }
  box.addEventListener("input",function(){render(box.value);});
}
// ---- security / tag row filters ----
function initFilters(){
  var panel=document.getElementById("filters");
  if(!panel)return;
  function apply(){
    var secOn={},tagOn={},anySec=false,anyTag=false;
    panel.querySelectorAll("input[data-sec]").forEach(function(c){
      if(c.checked)secOn[c.getAttribute("data-sec")]=true;else anySec=true;});
    panel.querySelectorAll("input[data-tag]").forEach(function(c){
      if(c.checked)tagOn[c.getAttribute("data-tag")]=true;else anyTag=true;});
    document.querySelectorAll("main table tr").forEach(function(tr){
      var a=tr.querySelector("a.m1-row-anchor");
      if(!a)return;
      var sec=a.getAttribute("data-security");
      var tags=(a.getAttribute("data-tags")||"").split(/\s+/).filter(Boolean);
      var okSec=!anySec|| (sec!=null&&secOn[sec]);
      var okTag=!anyTag|| tags.some(function(t){return tagOn[t];});
      if(okSec&&okTag){tr.classList.remove("filtered");}
      else{tr.classList.add("filtered");}
    });
  }
  panel.addEventListener("change",apply);
}
// ---- lightweight M1 syntax highlighting ----
var M1_KW=["if","else","when","is","expand","to","local","return","and","or",
  "not","true","false","In","Out","Parent","Root","Library","This"];
function highlightM1(){
  document.querySelectorAll("pre code.language-m1,pre code.language-M1").forEach(function(code){
    var src=code.textContent;
    var html="";var i=0;var n=src.length;
    function isIdent(c){return /[A-Za-z0-9_.]/.test(c);}
    while(i<n){
      var c=src[i];
      if(c==="/"&&src[i+1]==="/"){
        var j=src.indexOf("\n",i);if(j<0)j=n;
        html+='<span class="m1-com">'+escTok(src.slice(i,j))+'</span>';i=j;
      }else if(c==='"'){
        var j2=i+1;while(j2<n&&src[j2]!=='"'){if(src[j2]==="\\")j2++;j2++;}
        j2=Math.min(j2+1,n);
        html+='<span class="m1-str">'+escTok(src.slice(i,j2))+'</span>';i=j2;
      }else if(/[0-9]/.test(c)){
        var j3=i;while(j3<n&&/[0-9a-fA-FxX.]/.test(src[j3]))j3++;
        html+='<span class="m1-num">'+escTok(src.slice(i,j3))+'</span>';i=j3;
      }else if(/[A-Za-z_]/.test(c)){
        var j4=i;while(j4<n&&isIdent(src[j4]))j4++;
        var word=src.slice(i,j4);
        // A bare keyword (no dot) is a keyword; an identifier followed by "("
        // reads as a function/method call.
        var k=j4;while(k<n&&/\s/.test(src[k]))k++;
        if(M1_KW.indexOf(word)!==-1){
          html+='<span class="m1-kw">'+escTok(word)+'</span>';
        }else if(src[k]==="("){
          html+='<span class="m1-fn">'+escTok(word)+'</span>';
        }else{html+=escTok(word);}
        i=j4;
      }else{html+=escTok(c);i++;}
    }
    code.innerHTML=html;
  });
}
function escTok(s){return s.replace(/[&<>]/g,function(c){
  return c==="&"?"&amp;":c==="<"?"&lt;":"&gt;";});}
// ---- wire up the menu / theme buttons ----
function initButtons(){
  var t=document.getElementById("theme-toggle");
  if(t)t.addEventListener("click",toggleTheme);
  var m=document.getElementById("menu-toggle");
  if(m)m.addEventListener("click",function(){
    var nav=document.querySelector("nav");if(nav)nav.classList.toggle("collapsed");});
}
function init(){
  initNav();initButtons();initPermalinks();initToc();
  initSearch();initFilters();highlightM1();
}
if(document.readyState!=="loading"){init();}
else{document.addEventListener("DOMContentLoaded",init);}
})();
"##;

// ---------------------------------------------------------------------------
// Link rewriting
// ---------------------------------------------------------------------------

/// Rewrite relative `*.md` hrefs to `*.html`.  Operates on the raw HTML
/// string produced by pulldown-cmark.  Only touches `href="…"` attributes
/// whose values end with `.md` and do **not** start with `http://` or
/// `https://`.
fn rewrite_md_links(html: &str) -> String {
    // We scan byte-by-byte for the pattern  href="…"  to keep the
    // implementation simple and dependency-free.
    let needle = "href=\"";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(needle) {
        // Emit everything up to and including `href="`
        out.push_str(&rest[..pos + needle.len()]);
        rest = &rest[pos + needle.len()..];
        // Find the closing quote.
        if let Some(end) = rest.find('"') {
            let href = &rest[..end];
            if !href.starts_with("http://") && !href.starts_with("https://") {
                // Split off any fragment (#…) or query (?…) that follows the
                // path component so we can check the path extension alone.
                let (path, suffix) = if let Some(i) = href.find(['#', '?']) {
                    (&href[..i], &href[i..])
                } else {
                    (href, "")
                };
                if let Some(stem) = path.strip_suffix(".md") {
                    // Replace the trailing `.md` with `.html`, then reattach
                    // the fragment/query string unchanged.
                    out.push_str(stem);
                    out.push_str(".html");
                    out.push_str(suffix);
                } else {
                    out.push_str(href);
                }
            } else {
                out.push_str(href);
            }
            out.push('"');
            rest = &rest[end + 1..];
        }
        // If no closing quote found the rest of the string is copied below.
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

fn build_nav(model: &DocModel) -> String {
    use std::collections::BTreeMap;
    let by_path: BTreeMap<&str, &crate::model::GroupDoc> =
        model.groups.iter().map(|g| (g.path.as_str(), g)).collect();

    let mut nav = String::from("<nav><h2>Navigation</h2>");
    nav.push_str("<a href=\"index.html\">Index</a>");
    if !model.enums.is_empty() {
        nav.push_str("<a href=\"enums.html\">Enums</a>");
    }
    nav.push_str("<ul>");
    // Start the tree from the forest roots (groups whose parent is not itself a
    // documented group); descend recursively. The interactive collapse widget
    // is the HTML-polish issue (#33); this is the nested structure it needs.
    for g in &model.groups {
        let parent = match g.path.rfind('.') {
            Some(i) => &g.path[..i],
            None => "",
        };
        if parent.is_empty() || !by_path.contains_key(parent) {
            push_nav_node(&mut nav, g, &by_path);
        }
    }
    nav.push_str("</ul></nav>");
    nav
}

/// Append one `<li>` for a group node and, recursively, a nested `<ul>` for its
/// children — reflecting the real group hierarchy in the sidebar.
fn push_nav_node(
    nav: &mut String,
    g: &crate::model::GroupDoc,
    by_path: &std::collections::BTreeMap<&str, &crate::model::GroupDoc>,
) {
    let label = g.path.rsplit('.').next().unwrap_or(&g.path);
    nav.push_str(&format!("<li><a href=\"{}.html\">{}</a>", g.path, label));
    if !g.children.is_empty() {
        nav.push_str("<ul>");
        for child in &g.children {
            if let Some(cg) = by_path.get(child.as_str()) {
                push_nav_node(nav, cg, by_path);
            }
        }
        nav.push_str("</ul>");
    }
    nav.push_str("</li>");
}

// ---------------------------------------------------------------------------
// Toolbar, search, filters, legend (#31 / #33 / #34)
// ---------------------------------------------------------------------------

/// Escape a string for use inside a double-quoted HTML attribute. Only the
/// characters that can break out of `"…"` or inject markup are touched.
fn attr_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The sticky toolbar: a menu button (narrow screens), the live search box and
/// its results list, a theme toggle, and an empty slot the script fills with a
/// per-page table of contents. The search box is wired to the inline index by
/// the script; with JS off it is an inert text box (the index is also a plain
/// list reachable by browsing), so the site degrades rather than breaks.
fn build_toolbar() -> String {
    let mut t = String::from("<div class=\"toolbar\">");
    t.push_str("<button id=\"menu-toggle\" class=\"btn\" title=\"Toggle navigation\">☰</button>");
    t.push_str(
        "<input id=\"search-box\" type=\"search\" placeholder=\"Search symbols, functions, tables…\" autocomplete=\"off\">",
    );
    t.push_str("<button id=\"theme-toggle\" class=\"btn\" title=\"Toggle dark mode\">◐</button>");
    t.push_str("</div>");
    t.push_str("<ul id=\"search-results\"></ul>");
    t.push_str("<div id=\"toc-slot\"></div>");
    t
}

/// The security/tag filter panel for a group page: a checkbox per security
/// level and per tag present in the project, each ticking the matching rows
/// visible (#34). Returns an empty string when the project declares neither, so
/// untagged/security-free projects get no empty panel. A short legend explains
/// what the controls do, complementing the per-level legend on the index.
fn build_filters(model: &DocModel) -> String {
    let levels = model.security_levels();
    let tags = model.tags();
    if levels.is_empty() && tags.is_empty() {
        return String::new();
    }
    let mut f =
        String::from("<details id=\"filters\" class=\"filters\"><summary>Filter rows</summary>");
    if !levels.is_empty() {
        f.push_str("<div><strong>Security</strong> ");
        for level in &levels {
            let esc = attr_escape(level);
            f.push_str(&format!(
                "<label><input type=\"checkbox\" data-sec=\"{esc}\"> {esc}</label>"
            ));
        }
        f.push_str("</div>");
    }
    if !tags.is_empty() {
        f.push_str("<div><strong>Tags</strong> ");
        for tag in &tags {
            let esc = attr_escape(tag);
            f.push_str(&format!(
                "<label><input type=\"checkbox\" data-tag=\"{esc}\"> {esc}</label>"
            ));
        }
        f.push_str("</div>");
    }
    f.push_str(
        "<div><small>Tick levels/tags to show only matching rows; all unticked shows everything.</small></div>",
    );
    f.push_str("</details>");
    f
}

/// The inline search-index script element. The index JSON sits in a
/// non-executing `<script type="application/json">` so the browser does not try
/// to run it; the behaviour script reads it by id. Self-contained — no fetch.
fn build_search_index_el(model: &DocModel) -> String {
    format!(
        "<script id=\"search-index\" type=\"application/json\">{}</script>",
        search_index_json(model)
    )
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a slice of Markdown [`RenderedFile`]s to HTML [`RenderedFile`]s.
///
/// For each input file:
/// - renders the Markdown body to an HTML fragment (tables enabled),
/// - wraps it in a minimal self-contained page with inline CSS and a sidebar,
/// - rewrites relative `*.md` hrefs to `*.html`,
/// - changes the output path from `*.md` to `*.html`.
pub fn render(markdown_files: &[RenderedFile], model: &DocModel) -> Vec<RenderedFile> {
    let nav = build_nav(model);
    let toolbar = build_toolbar();
    let filters = build_filters(model);
    let search_index = build_search_index_el(model);
    markdown_files
        .iter()
        .map(|f| {
            // 1. Convert Markdown → HTML fragment (tables enabled).
            let mut fragment = String::new();
            let parser =
                pulldown_cmark::Parser::new_ext(&f.body, pulldown_cmark::Options::ENABLE_TABLES);
            pulldown_cmark::html::push_html(&mut fragment, parser);

            // 2. Rewrite intra-doc .md links → .html links.
            let fragment = rewrite_md_links(&fragment);

            // The row filter only belongs on a group page (one with filterable
            // rows). The landing/enums/tag-index pages have no `.m1-row-anchor`
            // rows, so the panel would filter nothing — omit it there.
            let is_group_page =
                f.path != "index.md" && f.path != "enums.md" && !f.path.starts_with("tag.");
            let filter_panel = if is_group_page { filters.as_str() } else { "" };

            // 3. Wrap in full page. The toolbar (search + theme + menu + TOC
            // slot) sits at the top of <main>; the inline search index and the
            // behaviour script are appended before </body>. Everything is
            // inline — no external asset, so the page is self-contained (#31/#33).
            let page = format!(
                "<!doctype html>\
<html lang=\"en\">\
<head>\
<meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>{title}</title>\
<style>{style}</style>\
</head>\
<body>\
{nav}\
<main>{toolbar}{filter_panel}{fragment}</main>\
{search_index}\
<script>{script}</script>\
</body>\
</html>",
                title = model.title,
                style = STYLE,
                script = SCRIPT,
            );

            // 4. Output path: swap .md → .html.
            let out_path = if f.path.ends_with(".md") {
                format!("{}.html", &f.path[..f.path.len() - 3])
            } else {
                format!("{}.html", f.path)
            };

            RenderedFile {
                path: out_path,
                body: page,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DocModel, GroupDoc, SymbolDoc, SymbolDocKind};

    fn demo_model() -> DocModel {
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    unit: Some("rpm".into()),
                    security: None,
                    ..Default::default()
                }],
                functions: vec![],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
        }
    }

    fn render_html(model: &DocModel) -> Vec<RenderedFile> {
        let md_files = crate::markdown::render(model);
        render(&md_files, model)
    }

    // #24: the symbol's deterministic anchor id survives from Markdown into the
    // HTML, so `Root.Engine.html#root-engine-speed` resolves to its row.
    #[test]
    fn symbol_anchor_id_survives_into_html() {
        let files = render_html(&demo_model());
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.html")
            .expect("Root.Engine.html missing");
        assert!(
            page.body.contains("id=\"root-engine-speed\""),
            "expected the symbol anchor id in the HTML; got:\n{}",
            &page.body[..page.body.len().min(800)]
        );
    }

    // (a) Group page contains <table and the channel data.
    #[test]
    fn group_page_has_table_and_channel() {
        let files = render_html(&demo_model());
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.html")
            .expect("Root.Engine.html missing");
        assert!(
            page.body.contains("<table"),
            "expected <table in group page; got:\n{}",
            &page.body[..page.body.len().min(500)]
        );
        assert!(
            page.body.contains("Root.Engine.Speed"),
            "expected channel name in group page; got:\n{}",
            &page.body[..page.body.len().min(500)]
        );
    }

    // (b) index.html contains a <nav> with href="Root.Engine.html".
    #[test]
    fn index_nav_has_html_link() {
        let files = render_html(&demo_model());
        let index = files
            .iter()
            .find(|f| f.path == "index.html")
            .expect("index.html missing");
        assert!(
            index.body.contains("<nav"),
            "expected <nav in index.html; got:\n{}",
            &index.body[..index.body.len().min(500)]
        );
        assert!(
            index.body.contains("href=\"Root.Engine.html\""),
            "expected href=\"Root.Engine.html\" in nav; got:\n{}",
            &index.body[..index.body.len().min(1000)]
        );
    }

    // (c) External http links are NOT rewritten.
    #[test]
    fn external_links_not_rewritten() {
        let html = r#"<a href="https://example.com/doc.md">ext</a>"#;
        let out = rewrite_md_links(html);
        assert_eq!(
            out, html,
            "external .md link must not be rewritten; got:\n{out}"
        );
    }

    // (c-extra) Relative .md links ARE rewritten.
    #[test]
    fn relative_md_links_are_rewritten() {
        let html = r#"<a href="Root.Engine.md">Engine</a>"#;
        let out = rewrite_md_links(html);
        assert!(
            out.contains("href=\"Root.Engine.html\""),
            "expected .md→.html rewrite; got:\n{out}"
        );
    }

    // (e) .md links with a fragment are rewritten; fragment is preserved.
    #[test]
    fn md_link_with_fragment_is_rewritten() {
        let html = r#"<a href="Root.Engine.md#section">Engine</a>"#;
        let out = rewrite_md_links(html);
        assert!(
            out.contains("href=\"Root.Engine.html#section\""),
            "expected .md#section→.html#section rewrite; got:\n{out}"
        );
    }

    // (f) .md links with a query string are rewritten; query is preserved.
    #[test]
    fn md_link_with_query_is_rewritten() {
        let html = r#"<a href="Root.Engine.md?v=1">Engine</a>"#;
        let out = rewrite_md_links(html);
        assert!(
            out.contains("href=\"Root.Engine.html?v=1\""),
            "expected .md?v=1→.html?v=1 rewrite; got:\n{out}"
        );
    }

    // (d) Every output path ends in .html.
    #[test]
    fn all_output_paths_end_in_html() {
        let files = render_html(&demo_model());
        for f in &files {
            assert!(
                f.path.ends_with(".html"),
                "expected .html path, got: {}",
                f.path
            );
        }
    }

    // ---- richer fixture for #31 / #33 / #34 ----

    use crate::model::{EnumDoc, FunctionDoc, TableDoc};

    fn rich_model() -> DocModel {
        DocModel {
            title: "UQR-EV".into(),
            target_hardware: None,
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                members: vec!["Off".into(), "On".into()],
                default: Some("Off".into()),
                open: false,
            }],
            groups: vec![
                GroupDoc {
                    path: "Root".into(),
                    references: vec![],
                    children: vec!["Root.Engine".into()],
                    ..Default::default()
                },
                GroupDoc {
                    path: "Root.Engine".into(),
                    symbols: vec![
                        SymbolDoc {
                            path: "Root.Engine.Speed".into(),
                            anchor: "root-engine-speed".into(),
                            kind: SymbolDocKind::Channel,
                            type_label: "f32".into(),
                            unit: Some("rpm".into()),
                            security: Some("Tune".into()),
                            tags: vec!["engine".into()],
                            ..Default::default()
                        },
                        SymbolDoc {
                            path: "Root.Engine.Gain".into(),
                            anchor: "root-engine-gain".into(),
                            kind: SymbolDocKind::Parameter,
                            type_label: "u16".into(),
                            security: Some("Calibration".into()),
                            tags: vec!["fuel".into()],
                            ..Default::default()
                        },
                    ],
                    functions: vec![FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        anchor: "root-engine-update".into(),
                        source_text: Some("Out = In.Speed * 2; // double it\n".into()),
                        ..Default::default()
                    }],
                    tables: vec![TableDoc {
                        path: "Root.Engine.Map".into(),
                        anchor: "root-engine-map".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            graph: crate::model::ProjectGraph::default(),
        }
    }

    // #31: the inline search index covers a known symbol with a resolvable
    // deep-link, and is embedded (no fetch) as application/json.
    #[test]
    fn search_index_contains_symbol_with_resolvable_anchor() {
        let json = search_index_json(&rich_model());
        // The channel is present with its group-page anchor href.
        assert!(
            json.contains("Root.Engine.Speed")
                && json.contains("Root.Engine.html#root-engine-speed"),
            "search index missing symbol/anchor; got:\n{json}"
        );
        // Functions, tables and enums are indexed too.
        assert!(json.contains("Root.Engine.Update"), "function missing");
        assert!(json.contains("Root.Engine.Map"), "table missing");
        assert!(
            json.contains("enums.html#switch"),
            "enum entry missing; got:\n{json}"
        );
    }

    #[test]
    fn search_index_is_embedded_inline_and_wired() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        assert!(
            index.body.contains("id=\"search-index\"")
                && index.body.contains("type=\"application/json\""),
            "inline search index element missing"
        );
        assert!(
            index.body.contains("id=\"search-box\""),
            "search box missing from the shell"
        );
    }

    #[test]
    fn search_index_order_is_deterministic() {
        let a = search_index_json(&rich_model());
        let b = search_index_json(&rich_model());
        assert_eq!(a, b, "search index must be byte-identical across runs");
    }

    // #33: the nav is a nested <ul> tree (the collapse JS toggles it), the page
    // carries the theme toggle, a TOC slot, and the M1-highlightable code class.
    #[test]
    fn nav_is_a_nested_tree() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        // Root has Engine as a child → a nested <ul> inside the <li>.
        assert!(
            index
                .body
                .contains("<li><a href=\"Root.html\">Root</a><ul>"),
            "expected a nested nav tree; got nav around Root:\n{}",
            &index.body[..index.body.len().min(1200)]
        );
    }

    #[test]
    fn shell_has_theme_toggle_and_toc_slot() {
        let files = render_html(&rich_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("id=\"theme-toggle\""),
            "theme toggle missing"
        );
        assert!(page.body.contains("id=\"toc-slot\""), "TOC slot missing");
        assert!(
            page.body.contains("id=\"menu-toggle\""),
            "menu toggle missing"
        );
        // Dark mode follows prefers-color-scheme in the inline CSS.
        assert!(
            page.body.contains("prefers-color-scheme:dark"),
            "dark-mode media query missing from inline CSS"
        );
    }

    #[test]
    fn m1_source_block_is_highlightable() {
        // With an embedded ```m1 block, pulldown-cmark emits language-m1; the
        // inline highlighter keys off that class.
        use crate::markdown::{RenderOptions, render_with};
        let mut model = rich_model();
        // Force source embedding on the function.
        for g in &mut model.groups {
            for f in &mut g.functions {
                f.source_path = Some("Engine/Update.m1scr".into());
            }
        }
        let md = render_with(
            &model,
            &RenderOptions {
                source_base: None,
                include_source: true,
            },
        );
        let html = render(&md, &model);
        let page = html.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("language-m1"),
            "embedded source must carry the language-m1 class for highlighting"
        );
        assert!(
            page.body.contains("m1-kw") && page.body.contains("M1_KW"),
            "the inline highlighter script/CSS for M1 must be present"
        );
    }

    // #34: a security legend appears on the index; a filter panel with the
    // project's levels and tags appears on a group page; rows carry the filter
    // metadata.
    #[test]
    fn index_has_security_legend() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        assert!(
            index.body.contains("Security levels"),
            "security legend missing from index"
        );
        assert!(
            index.body.contains("Tune") && index.body.contains("Calibration"),
            "legend must name each level present"
        );
    }

    #[test]
    fn group_page_has_filter_panel_and_row_metadata() {
        let files = render_html(&rich_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("id=\"filters\""),
            "filter panel missing from group page"
        );
        assert!(
            page.body.contains("data-sec=\"Tune\"")
                && page.body.contains("data-sec=\"Calibration\""),
            "security filter checkboxes missing"
        );
        assert!(
            page.body.contains("data-tag=\"engine\"") && page.body.contains("data-tag=\"fuel\""),
            "tag filter checkboxes missing"
        );
        // Rows carry the filter metadata the script reads.
        assert!(
            page.body.contains("data-security=\"Tune\"")
                && page.body.contains("data-tags=\"engine\""),
            "row filter metadata missing; got:\n{}",
            &page.body[..page.body.len().min(2400)]
        );
    }

    #[test]
    fn index_page_has_no_filter_panel() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        assert!(
            !index.body.contains("id=\"filters\""),
            "the landing page has no filterable rows; it must not carry the panel"
        );
    }

    // Self-containment: no external asset URLs anywhere in any page (#33). We
    // only forbid asset-bearing schemes; an issue/source link in body text is
    // fine, but the shell (CSS/JS/index) must not reach the network.
    #[test]
    fn shell_has_no_external_asset_references() {
        let files = render_html(&rich_model());
        for f in &files {
            for needle in [
                "src=\"http",
                "href=\"http",
                "@import",
                "url(http",
                "cdn.",
                "googleapis",
                "unpkg",
                "jsdelivr",
            ] {
                assert!(
                    !f.body.contains(needle),
                    "page {} reaches an external asset ({needle})",
                    f.path
                );
            }
        }
    }

    // Determinism: rendering twice yields byte-identical pages (#33 guardrail).
    #[test]
    fn html_output_is_deterministic() {
        let a = render_html(&rich_model());
        let b = render_html(&rich_model());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.path, y.path);
            assert_eq!(x.body, y.body, "page {} differs across runs", x.path);
        }
    }

    #[test]
    fn permalink_and_filter_css_are_inline() {
        let files = render_html(&rich_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains(".permalink") && page.body.contains("tr.filtered"),
            "permalink / filter CSS must be inline in the shell"
        );
    }
}
