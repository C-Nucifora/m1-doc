//! Renders Markdown files (produced by [`crate::markdown`]) to a self-contained
//! HTML site.  Each `.md` file becomes a `.html` file; intra-doc links are
//! rewritten from `*.md` to `*.html`.  External `http(s)://` links are left
//! untouched.  The only inputs are [`crate::markdown::RenderedFile`] slices and
//! a [`crate::model::DocModel`] (for the sidebar and page title).  No m1-core /
//! m1-typecheck types cross this module boundary.

use crate::diagram::Diagram;
use crate::markdown::RenderedFile;
use crate::model::{AnchoredKind, DocModel};
use std::collections::HashMap;

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
/* #37 interactive relationship graph (force-directed canvas, self-contained) */
.m1-graph{border:1px solid var(--border);border-radius:6px;margin:1rem 0;
          overflow:hidden;background:var(--pre-bg)}
.m1-graph-head{display:flex;justify-content:space-between;align-items:center;
          gap:1rem;padding:.45rem .75rem;border-bottom:1px solid var(--border);
          flex-wrap:wrap}
.m1-graph-title{font-weight:600}
.m1-graph-hint{color:var(--muted);font-size:.8rem;margin-left:auto}
.m1-graph-reset{padding:.2rem .55rem;border:1px solid var(--border);
          border-radius:4px;background:var(--nav-bg);color:var(--fg);
          cursor:pointer;font-size:.8rem}
.m1-graph-stage{position:relative;height:520px;touch-action:none}
.m1-graph-stage canvas{display:block;width:100%;height:100%;cursor:grab}
.m1-graph-tip{position:absolute;pointer-events:none;background:var(--bg);
          color:var(--fg);border:1px solid var(--border);border-radius:4px;
          padding:.3rem .5rem;font-size:.8rem;max-width:24rem;z-index:2;
          box-shadow:0 2px 8px rgba(0,0,0,.3)}
.m1-graph-tip .k{color:var(--muted)}
.m1-graph-legend{display:flex;flex-wrap:wrap;gap:.3rem .9rem;padding:.5rem .75rem;
          border-top:1px solid var(--border);font-size:.8rem}
.m1-graph-legend .lg{display:flex;align-items:center;gap:.35rem;cursor:pointer;
          user-select:none}
.m1-graph-legend .lg.off{opacity:.4}
.m1-graph-legend .dot{width:10px;height:10px;border-radius:50%;flex-shrink:0}
.m1-graph-empty{padding:1rem .75rem;color:var(--muted);font-style:italic}
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

/// Minimal JSON-string escaping for the inline index: returns the *unquoted*
/// inner content (callers supply the surrounding `"`). Uses the shared
/// script-safe escaper so `<`/`>`/`/` can never form a literal `</script>` that
/// would close the embedding element early. See
/// [`crate::escape::escape_json_into`].
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    crate::escape::escape_json_into(&mut out, s, true);
    out
}

/// Collect every documented entity into search records, in deterministic order.
///
/// Built from the model's single [`DocModel::anchored_entities`] walk — the same
/// traversal the relationship-graph node-href map uses — so the search index and
/// the graph deep links can never again cover different subsets of the anchored
/// kinds. The search index keeps every kind (it indexes enums too); the graph
/// map drops enums. See `node_hrefs`.
fn build_search_entries(model: &DocModel) -> Vec<SearchEntry> {
    model
        .anchored_entities()
        .into_iter()
        .map(|e| SearchEntry {
            path: e.path.to_string(),
            kind: e.kind.label(),
            group: e.group.to_string(),
            hint: e.hint.to_string(),
            href: e.href(),
        })
        .collect()
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
// ---- #37 interactive relationship graph (force-directed, no library/CDN) ----
function cssVar(name,fb){var v=getComputedStyle(document.documentElement)
  .getPropertyValue(name).trim();return v||fb;}
function initGraphs(){
  var figs=document.querySelectorAll("figure.m1-graph");
  for(var i=0;i<figs.length;i++){try{buildGraph(figs[i]);}catch(e){}}
}
function buildGraph(fig){
  var dataEl=fig.querySelector(".m1-graph-data");if(!dataEl)return;
  var data=JSON.parse(dataEl.textContent);
  var nodes=data.nodes||[],edges=data.edges||[];
  var canvas=fig.querySelector("canvas");if(!canvas||!nodes.length)return;
  var stage=fig.querySelector(".m1-graph-stage"),tip=fig.querySelector(".m1-graph-tip");
  var ctx=canvas.getContext("2d");
  var byId={};nodes.forEach(function(n){byId[n.id]=n;});
  edges.forEach(function(e){e.a=byId[e.from];e.b=byId[e.to];});
  var nbr={};nodes.forEach(function(n){nbr[n.id]={};});
  edges.forEach(function(e){if(e.a&&e.b){nbr[e.a.id][e.b.id]=1;nbr[e.b.id][e.a.id]=1;}});
  var hidden={};
  var EC={call:cssVar("--fn","#4078f2"),read:cssVar("--str","#50a14f"),
          write:cssVar("--num","#986801"),reference:cssVar("--muted","#888")};
  function theme(){return{bg:cssVar("--pre-bg","#0f0f1a"),fg:cssVar("--fg","#e0e0e0"),
          muted:cssVar("--muted","#888"),border:cssVar("--border","#333")};}
  var N=nodes.length;
  nodes.forEach(function(n,i){var a=i/N*6.2832;n.x=Math.cos(a)*150+(i%7);
    n.y=Math.sin(a)*150+(i%5);n.vx=0;n.vy=0;n.r=6+Math.min(20,(n.degree||0)*2.2);});
  var view={s:1,ox:0,oy:0},W=0,H=0,DPR=window.devicePixelRatio||1;
  function resize(){W=stage.clientWidth;H=stage.clientHeight;
    canvas.width=W*DPR;canvas.height=H*DPR;canvas.style.width=W+"px";
    canvas.style.height=H+"px";}
  var alpha=1,running=false,dragNode=null,panning=false,last={x:0,y:0},moved=false;
  var hover=null,sel=null;
  function vis(n){return !hidden[n.community];}
  function tick(){
    var a=nodes.filter(vis),i,j;
    for(i=0;i<a.length;i++){var p=a[i];for(j=i+1;j<a.length;j++){var q=a[j];
      var dx=p.x-q.x,dy=p.y-q.y,d2=dx*dx+dy*dy+0.01,d=Math.sqrt(d2),f=1800/d2;
      var fx=dx/d*f,fy=dy/d*f;p.vx+=fx;p.vy+=fy;q.vx-=fx;q.vy-=fy;}}
    edges.forEach(function(e){if(!e.a||!e.b||!vis(e.a)||!vis(e.b))return;
      var dx=e.b.x-e.a.x,dy=e.b.y-e.a.y,d=Math.sqrt(dx*dx+dy*dy)+0.01;
      var f=(d-90)*0.04,fx=dx/d*f,fy=dy/d*f;
      e.a.vx+=fx;e.a.vy+=fy;e.b.vx-=fx;e.b.vy-=fy;});
    a.forEach(function(n){if(n===dragNode)return;n.vx+=-n.x*0.015;n.vy+=-n.y*0.015;
      n.vx*=0.82;n.vy*=0.82;n.x+=n.vx*alpha;n.y+=n.vy*alpha;});
    alpha*=0.985;
  }
  function fit(){var a=nodes.filter(vis);if(!a.length)return;
    var x0=1e9,y0=1e9,x1=-1e9,y1=-1e9;a.forEach(function(n){x0=Math.min(x0,n.x-n.r);
      y0=Math.min(y0,n.y-n.r);x1=Math.max(x1,n.x+n.r);y1=Math.max(y1,n.y+n.r);});
    var gw=x1-x0||1,gh=y1-y0||1,s=Math.min(W/(gw+60),H/(gh+60),2.2);
    view.s=s;view.ox=W/2-(x0+x1)/2*s;view.oy=H/2-(y0+y1)/2*s;}
  function S(n){return{x:n.x*view.s+view.ox,y:n.y*view.s+view.oy};}
  function world(px,py){return{x:(px-view.ox)/view.s,y:(py-view.oy)/view.s};}
  function arrow(p,q,col,rr){var dx=q.x-p.x,dy=q.y-p.y,d=Math.sqrt(dx*dx+dy*dy)||1;
    var ux=dx/d,uy=dy/d,tx=q.x-ux*(rr+1),ty=q.y-uy*(rr+1),k=6;ctx.fillStyle=col;
    ctx.beginPath();ctx.moveTo(tx,ty);
    ctx.lineTo(tx-ux*k-uy*k*0.6,ty-uy*k+ux*k*0.6);
    ctx.lineTo(tx-ux*k+uy*k*0.6,ty-uy*k-ux*k*0.6);ctx.closePath();ctx.fill();}
  function draw(){
    var T=theme(),f=sel||hover;
    ctx.setTransform(DPR,0,0,DPR,0,0);ctx.fillStyle=T.bg;ctx.fillRect(0,0,W,H);
    edges.forEach(function(e){if(!e.a||!e.b||!vis(e.a)||!vis(e.b))return;
      var p=S(e.a),q=S(e.b),on=f?(e.a===f||e.b===f):true;ctx.globalAlpha=on?0.8:0.1;
      ctx.strokeStyle=EC[e.kind]||T.muted;ctx.lineWidth=e.kind==="write"?2.2:1.3;
      ctx.setLineDash(e.kind==="read"?[5,4]:e.kind==="reference"?[2,3]:[]);
      ctx.beginPath();ctx.moveTo(p.x,p.y);
      ctx.quadraticCurveTo((p.x+q.x)/2,(p.y+q.y)/2,q.x,q.y);ctx.stroke();
      if(on)arrow(p,q,EC[e.kind]||T.muted,e.b.r*view.s);});
    ctx.setLineDash([]);ctx.globalAlpha=1;
    nodes.forEach(function(n){if(!vis(n))return;var p=S(n),r=n.r*view.s;
      var dim=f&&!(n===f||nbr[f.id][n.id]);ctx.globalAlpha=dim?0.18:1;
      ctx.beginPath();ctx.arc(p.x,p.y,r,0,6.2832);ctx.fillStyle=n.color;ctx.fill();
      ctx.lineWidth=n===f?2.5:(n.primary?1.5:1);
      ctx.strokeStyle=n===f?T.fg:(n.primary?T.fg:T.border);
      if(!n.primary&&n!==f)ctx.globalAlpha=dim?0.12:0.55;ctx.stroke();
      ctx.globalAlpha=dim?0.25:1;
      if(view.s>0.55||n===hover||n===sel){ctx.fillStyle=T.fg;
        ctx.font="11px ui-monospace,monospace";ctx.textAlign="center";
        ctx.textBaseline="top";ctx.fillText(n.label,p.x,p.y+r+2);}});
    ctx.globalAlpha=1;
  }
  function frame(){if(alpha>0.02){tick();tick();}draw();
    if(alpha>0.02)requestAnimationFrame(frame);else running=false;}
  function start(){if(!running){running=true;requestAnimationFrame(frame);}}
  function heat(v){alpha=Math.max(alpha,v||0.5);start();}
  function pick(px,py){var best=null,bd=1e9;nodes.forEach(function(n){if(!vis(n))return;
    var p=S(n),dx=p.x-px,dy=p.y-py,d=dx*dx+dy*dy,rr=n.r*view.s+4;
    if(d<rr*rr&&d<bd){bd=d;best=n;}});return best;}
  function rel(ev){var b=canvas.getBoundingClientRect();
    return{x:ev.clientX-b.left,y:ev.clientY-b.top};}
  canvas.addEventListener("mousedown",function(ev){var m=rel(ev),n=pick(m.x,m.y);
    moved=false;last=m;if(n)dragNode=n;else panning=true;});
  window.addEventListener("mousemove",function(ev){
    if(dragNode){var m=rel(ev),w=world(m.x,m.y);dragNode.x=w.x;dragNode.y=w.y;
      moved=true;heat(0.25);return;}
    if(panning){var m2=rel(ev);view.ox+=m2.x-last.x;view.oy+=m2.y-last.y;last=m2;
      moved=true;draw();return;}
    if(ev.target!==canvas){return;}
    var m3=rel(ev),h=pick(m3.x,m3.y);
    if(h!==hover){hover=h;if(!running)draw();}
    if(h){tip.hidden=false;tip.innerHTML="<b>"+escTok(h.id)+"</b><br><span class='k'>"
      +escTok(h.community)+" · degree "+h.degree+"</span>";
      var tx=m3.x+14;if(tx+tip.offsetWidth>W)tx=m3.x-tip.offsetWidth-14;
      tip.style.left=tx+"px";tip.style.top=(m3.y+14)+"px";canvas.style.cursor="pointer";}
    else{tip.hidden=true;canvas.style.cursor="";}});
  window.addEventListener("mouseup",function(){
    if(dragNode&&!moved){if(dragNode.href){window.location.href=dragNode.href;}
      else{sel=sel===dragNode?null:dragNode;draw();}}
    else if(panning&&!moved){sel=null;draw();}
    dragNode=null;panning=false;});
  canvas.addEventListener("wheel",function(ev){ev.preventDefault();var m=rel(ev),
    w=world(m.x,m.y),k=ev.deltaY<0?1.12:0.89;view.s*=k;
    view.ox=m.x-w.x*view.s;view.oy=m.y-w.y*view.s;if(!running)draw();},{passive:false});
  var legend=fig.querySelector(".m1-graph-legend");
  if(legend&&data.communities){data.communities.forEach(function(c){
    var el=document.createElement("span");el.className="lg";
    el.innerHTML="<span class='dot' style='background:"+c.color+"'></span>"+escTok(c.name);
    el.addEventListener("click",function(){hidden[c.name]=!hidden[c.name];
      el.classList.toggle("off",!!hidden[c.name]);heat(0.6);});
    legend.appendChild(el);});}
  var rb=fig.querySelector(".m1-graph-reset");
  if(rb)rb.addEventListener("click",function(){fit();heat(0.6);});
  // Repaint on theme change so canvas colours track light/dark.
  new MutationObserver(function(){if(!running)draw();}).observe(
    document.documentElement,{attributes:true,attributeFilter:["data-theme"]});
  if("ResizeObserver"in window)
    new ResizeObserver(function(){resize();if(!running){fit();draw();}}).observe(stage);
  resize();fit();start();
}
function init(){
  initNav();initButtons();initPermalinks();initToc();
  initSearch();initFilters();highlightM1();initGraphs();
}
if(document.readyState!=="loading"){init();}
else{document.addEventListener("DOMContentLoaded",init);}
})();
"##;

// ---------------------------------------------------------------------------
// Relationship-graph widgets (#37)
// ---------------------------------------------------------------------------
//
// The Markdown renderer drops a sentinel comment + a ` ```mermaid ` fallback
// block for each graph (so the canonical `.md` renders a diagram on GitHub).
// Here, in the HTML, we swap that pair for the interactive force-directed
// widget: a `<canvas>` plus an inline JSON payload the page's `buildGraph`
// renders with no library and no network. The diagram is regenerated from the
// model's graph using the sentinel's `mode:depth:group`, so the two outputs
// always agree.

/// Map every graph-eligible documented entity's path to its page link
/// (`<group>.html#<anchor>`), so a graph node can deep-link to where it is
/// documented. Built once from the model's shared [`DocModel::anchored_entities`]
/// walk (so it can never drift from the search index again) and filtered to the
/// kinds that can appear as graph nodes — i.e. everything anchored on a group
/// page (symbols, functions, tables, objects, CAN messages and signals,
/// references); enums live on the shared reference page and are not graph nodes.
fn node_hrefs(model: &DocModel) -> HashMap<String, String> {
    model
        .anchored_entities()
        .into_iter()
        .filter(|e| e.kind != AnchoredKind::Enum)
        .map(|e| (e.path.to_string(), e.href()))
        .collect()
}

/// Rebuild the diagram a sentinel refers to. `mode` is `group` (seed on the
/// group's direct members) or `subtree` (the whole `--graph` subsystem).
fn diagram_for(model: &DocModel, mode: &str, group: &str, depth: usize) -> Diagram {
    match mode {
        "subtree" => Diagram::subsystem(&model.graph, group, depth),
        _ => {
            let members: Vec<&str> = model
                .groups
                .iter()
                .find(|g| g.path == group)
                .map(|g| {
                    g.symbols
                        .iter()
                        .map(|s| s.path.as_str())
                        .chain(g.functions.iter().map(|f| f.path.as_str()))
                        .chain(g.references.iter().map(|r| r.path.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            Diagram::for_group(&model.graph, &members, group, depth)
        }
    }
}

/// The `<figure>` markup for one diagram: the canvas stage, the legend slot, and
/// the inline JSON the renderer consumes. An edge-less diagram degrades to a
/// note rather than an empty canvas.
fn graph_figure(diagram: &Diagram, hrefs: &HashMap<String, String>) -> String {
    if diagram.is_empty() {
        return format!(
            "<figure class=\"m1-graph\"><div class=\"m1-graph-empty\">No documented \
relationships{}.</div></figure>",
            if diagram.title.is_empty() {
                String::new()
            } else {
                format!(" for {}", html_escape(&diagram.title))
            }
        );
    }
    let json = diagram.to_json(|p| hrefs.get(p).cloned());
    format!(
        "<figure class=\"m1-graph\">\
<div class=\"m1-graph-head\">\
<span class=\"m1-graph-title\">{title}</span>\
<span class=\"m1-graph-hint\">drag · scroll to zoom · click a node to open its page</span>\
<button class=\"m1-graph-reset\" type=\"button\">Fit</button>\
</div>\
<div class=\"m1-graph-stage\"><canvas></canvas><div class=\"m1-graph-tip\" hidden></div></div>\
<div class=\"m1-graph-legend\"></div>\
<script type=\"application/json\" class=\"m1-graph-data\">{json}</script>\
</figure>",
        title = html_escape(&diagram.title),
    )
}

/// Append `s` to `out` with HTML escaping. `& < >` are always escaped (the set
/// that can inject markup in any context); when `attr` is set the
/// attribute-delimiting `"` is additionally escaped so the result is safe inside
/// a double-quoted attribute value.
///
/// This is the single implementation behind both [`html_escape`] (text context)
/// and [`attr_escape`] (attribute context) — the two contexts differ only by the
/// `"` flag, so they can never drift apart on the common `& < >` set the way two
/// hand-rolled bodies could (mirrors the JSON escaper consolidation in
/// [`crate::escape`]).
///
/// Note: spaces are deliberately left verbatim. The attribute callers escape
/// group-path hrefs, which must keep spaces literal to match the on-disk page
/// filename (`<group path>.html`).
fn html_escape_into(out: &mut String, s: &str, attr: bool) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

/// Minimal HTML-text escaping for figure titles. Escapes `& < >`, leaving `"`
/// (and spaces) verbatim — for text contexts, not attribute values.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    html_escape_into(&mut out, s, false);
    out
}

/// Replace every `<!--m1-graph:mode:depth:group-->` sentinel (and the Mermaid
/// `<pre>` block pulldown-cmark rendered right after it) with the interactive
/// graph figure. Other content is untouched.
fn swap_graphs(html: &str, model: &DocModel, hrefs: &HashMap<String, String>) -> String {
    const OPEN: &str = "<!--m1-graph:";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(OPEN) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + OPEN.len()..];
        let Some(close) = after.find("-->") else {
            // Malformed — emit verbatim and stop scanning.
            out.push_str(&rest[pos..]);
            return out;
        };
        let spec = &after[..close];
        // mode:depth:group  (group may contain ':'? paths use '.', so splitn is safe)
        let mut it = spec.splitn(3, ':');
        let mode = it.next().unwrap_or("group");
        let depth: usize = it.next().and_then(|d| d.parse().ok()).unwrap_or(1);
        let group = it.next().unwrap_or("");
        // Advance past the comment, then past the trailing Mermaid <pre> block.
        let mut tail = &after[close + 3..];
        if let Some(ps) = tail.find("<pre>")
            && let Some(pe) = tail[ps..].find("</pre>")
        {
            tail = &tail[ps + pe + "</pre>".len()..];
        }
        let diagram = diagram_for(model, mode, group, depth);
        out.push_str(&graph_figure(&diagram, hrefs));
        rest = tail;
    }
    out.push_str(rest);
    out
}

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
    // M1 names may contain spaces and markup-significant characters, so escape
    // both the href (attribute context) and the visible label (text context).
    // `attr_escape` deliberately leaves spaces verbatim so the href matches the
    // on-disk page filename (`<group path>.html` keeps spaces literal too).
    nav.push_str(&format!(
        "<li><a href=\"{}.html\">{}</a>",
        attr_escape(&g.path),
        html_escape(label)
    ));
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

/// Escape a string for use inside a double-quoted HTML attribute. Escapes the
/// markup-significant `& < >` plus the attribute-delimiting `"`; spaces are left
/// verbatim so an href matches the on-disk page filename. Thin wrapper over
/// [`html_escape_into`] with `attr = true`.
fn attr_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    html_escape_into(&mut out, s, true);
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
    // Node → page-link map for the interactive relationship graphs (#37).
    let graph_hrefs = node_hrefs(model);
    markdown_files
        .iter()
        .map(|f| {
            // 1. Convert Markdown → HTML fragment (tables enabled).
            let mut fragment = String::new();
            let parser =
                pulldown_cmark::Parser::new_ext(&f.body, pulldown_cmark::Options::ENABLE_TABLES);
            pulldown_cmark::html::push_html(&mut fragment, parser);

            // 2. Swap relationship-graph sentinels (+ their Mermaid fallback)
            //    for the interactive force-directed widget (#37).
            let fragment = swap_graphs(&fragment, model, &graph_hrefs);

            // 3. Rewrite intra-doc .md links → .html links.
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
                title = html_escape(&model.title),
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
            m1prj_path: None,
        }
    }

    fn render_html(model: &DocModel) -> Vec<RenderedFile> {
        let md_files = crate::markdown::render(model);
        render(&md_files, model)
    }

    /// #37: a group with relationships renders the interactive force-graph
    /// widget (canvas + inline JSON), and the Mermaid fallback is swapped out —
    /// the HTML draws the diagram itself, with no library or CDN.
    #[test]
    fn relationships_become_self_contained_interactive_widget() {
        use crate::model::{EdgeKind, FunctionDoc, GraphEdge, ProjectGraph};
        let mut model = demo_model();
        model.groups[0].functions.push(FunctionDoc {
            path: "Root.Engine.Update".into(),
            anchor: "root-engine-update".into(),
            ..Default::default()
        });
        model.graph = ProjectGraph {
            edges: vec![GraphEdge {
                from: "Root.Engine.Update".into(),
                to: "Root.Engine.Speed".into(),
                kind: EdgeKind::Read,
            }],
        };
        let files = render_html(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("<figure class=\"m1-graph\">"),
            "interactive graph figure missing"
        );
        assert!(page.body.contains("<canvas>"));
        assert!(page.body.contains("class=\"m1-graph-data\""));
        // A node links to its documentation page (deep-link, .html rewritten).
        assert!(page.body.contains("Root.Engine.html#root-engine-update"));
        // The Mermaid fallback was replaced; the HTML needs no Mermaid runtime.
        assert!(
            !page.body.contains("language-mermaid"),
            "Mermaid block should be swapped for the widget"
        );
        // Self-contained: the widget pulls nothing from the network.
        assert!(!page.body.contains("unpkg.com") && !page.body.contains("cdn"));
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

    // The sidebar nav is hand-built raw HTML, so a group/component name with
    // markup-significant characters (`&`, `<`, `>`, `"`) must be escaped in both
    // the visible label and the href. M1 names permit spaces and are not
    // restricted to alphanumerics, so this is a real corpus shape, not a
    // synthetic one. Without escaping, `Root.A & B` would emit a raw `&`,
    // producing a malformed entity reference and invalid HTML.
    #[test]
    fn nav_escapes_markup_in_component_names() {
        let mut model = demo_model();
        model.groups[0].path = "Root.A & B <x> \"q\"".into();
        // The single demo symbol's path is irrelevant to the nav; keep it valid.
        model.groups[0].symbols[0].path = "Root.A & B <x> \"q\".Speed".into();
        let files = render_html(&model);
        let nav = &files[0].body;
        let nav = &nav
            [nav.find("<nav>").expect("nav missing")..nav.find("</nav>").expect("nav end missing")];

        // `&` is escaped to `&amp;`, not left raw (the raw form would be a
        // malformed entity reference).
        assert!(
            nav.contains("&amp;"),
            "nav should escape '&' to '&amp;'; got:\n{nav}"
        );
        assert!(
            !nav.contains("A & B"),
            "nav must not contain a raw, unescaped '&'; got:\n{nav}"
        );
        // `<` / `>` / `"` from the name must not survive as literal markup.
        assert!(
            nav.contains("&lt;x&gt;"),
            "nav should escape '<x>' to '&lt;x&gt;'; got:\n{nav}"
        );
        assert!(
            nav.contains("&quot;q&quot;"),
            "nav should escape '\"q\"' to '&quot;q&quot;'; got:\n{nav}"
        );
        // The href path is escaped consistently with the on-disk filename
        // (`<group path>.html`, which keeps spaces verbatim), so escaping `&<>"`
        // but not spaces keeps the link pointing at the actual file.
        assert!(
            nav.contains("href=\"Root.A &amp; B &lt;x&gt; &quot;q&quot;.html\""),
            "nav href should be attribute-escaped to match the page filename; got:\n{nav}"
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

    use crate::model::{EnumDoc, EnumMemberDoc, FunctionDoc, TableDoc};

    fn rich_model() -> DocModel {
        DocModel {
            title: "UQR-EV".into(),
            target_hardware: None,
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                members: vec![
                    EnumMemberDoc {
                        name: "Off".into(),
                        value: 0,
                    },
                    EnumMemberDoc {
                        name: "On".into(),
                        value: 1,
                    },
                ],
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
            m1prj_path: None,
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

    // The graph node-href map and the search index are both built from the one
    // `DocModel::anchored_entities` walk, so they cannot drift on which anchored
    // kinds carry a deep link (the historical bug: `node_hrefs` covered only
    // symbols/functions/references, silently missing any table/object/CAN node).
    // Every graph-eligible kind must now be deep-linked, with the same href the
    // search index uses; enums (not graph nodes) are excluded.
    #[test]
    fn node_hrefs_cover_every_graph_eligible_kind() {
        use crate::model::{CanMessageDoc, CanSignalDoc, ObjectDoc, ReferenceDoc};
        let mut model = rich_model();
        let eng = model
            .groups
            .iter_mut()
            .find(|g| g.path == "Root.Engine")
            .unwrap();
        eng.objects.push(ObjectDoc {
            path: "Root.Engine.Sensor".into(),
            anchor: "root-engine-sensor".into(),
            ..Default::default()
        });
        eng.references.push(ReferenceDoc {
            path: "Root.Engine.Alias".into(),
            anchor: "root-engine-alias".into(),
            ..Default::default()
        });
        eng.can_messages.push(CanMessageDoc {
            path: "Root.Engine.Frame".into(),
            anchor: "root-engine-frame".into(),
            signals: vec![CanSignalDoc {
                path: "Root.Engine.Frame.Rpm".into(),
                anchor: "root-engine-frame-rpm".into(),
                ..Default::default()
            }],
            ..Default::default()
        });

        let hrefs = node_hrefs(&model);

        // Symbols, functions, tables, objects, CAN messages + signals and
        // references all resolve to their <group>.html#<anchor> page link.
        assert_eq!(
            hrefs.get("Root.Engine.Speed").map(String::as_str),
            Some("Root.Engine.html#root-engine-speed")
        );
        assert_eq!(
            hrefs.get("Root.Engine.Update").map(String::as_str),
            Some("Root.Engine.html#root-engine-update")
        );
        assert_eq!(
            hrefs.get("Root.Engine.Map").map(String::as_str),
            Some("Root.Engine.html#root-engine-map"),
            "a table node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Sensor").map(String::as_str),
            Some("Root.Engine.html#root-engine-sensor"),
            "an object node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Frame").map(String::as_str),
            Some("Root.Engine.html#root-engine-frame"),
            "a CAN message node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Frame.Rpm").map(String::as_str),
            Some("Root.Engine.html#root-engine-frame-rpm"),
            "a CAN signal node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Alias").map(String::as_str),
            Some("Root.Engine.html#root-engine-alias")
        );

        // Enums live on the shared reference page and are not graph nodes, so the
        // node-href map (unlike the search index) omits them.
        assert!(
            !hrefs.contains_key("Switch"),
            "enums are not graph nodes and must not be in the node-href map"
        );

        // The node-href and search-index deep links agree for every shared key.
        for e in build_search_entries(&model) {
            if let Some(h) = hrefs.get(&e.path) {
                assert_eq!(
                    *h, e.href,
                    "search index and node-href map disagree on {}",
                    e.path
                );
            }
        }
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
                graph: None,
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

    // The page title is user-controlled (the `--title` flag, or the project's
    // parent directory name). HTML metacharacters in it must be escaped before
    // they reach the raw `<title>...</title>` in the head, the same way every
    // other text node in the shell is escaped — otherwise a title like
    // `A <b> & "C"` produces malformed/unescaped HTML on every generated page.
    #[test]
    fn title_with_html_metacharacters_is_escaped_in_head() {
        let mut model = demo_model();
        model.title = "A <b> & \"C\"".into();
        let files = render_html(&model);
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.html")
            .expect("Root.Engine.html missing");
        assert!(
            page.body.contains("<title>A &lt;b&gt; &amp;"),
            "title metacharacters must be escaped in <title>; got head:\n{}",
            &page.body[..page.body.len().min(400)]
        );
        // The raw, unescaped tag must NOT leak into the head.
        assert!(
            !page.body.contains("<title>A <b>"),
            "raw unescaped <b> leaked into the page <title>"
        );
    }

    // The two public escapers share one implementation (a single `attr` flag),
    // so they can never drift apart on the common `& < >` set the way two
    // hand-rolled bodies could. This test pins that contract.
    #[test]
    fn text_and_attr_escapers_agree_except_on_double_quote() {
        let sample = "a & b <c> \"d\" e";

        // Both contexts always escape the markup-significant `& < >`.
        for esc in [html_escape(sample), attr_escape(sample)] {
            assert!(esc.contains("&amp;"), "'&' not escaped in: {esc}");
            assert!(esc.contains("&lt;c&gt;"), "'<c>' not escaped in: {esc}");
            assert!(!esc.contains(" & "), "raw '&' survived in: {esc}");
        }

        // The *only* difference is the attribute-delimiting double quote:
        // text context leaves it verbatim, attribute context escapes it.
        assert!(
            html_escape(sample).contains('"'),
            "text escaper must leave '\"' verbatim"
        );
        assert!(
            !html_escape(sample).contains("&quot;"),
            "text escaper must not escape '\"'"
        );
        assert!(
            attr_escape(sample).contains("&quot;"),
            "attribute escaper must escape '\"' to '&quot;'"
        );
        assert!(
            !attr_escape(sample).contains('"'),
            "attribute escaper must leave no raw '\"'"
        );

        // Apart from the `"` handling the two outputs are identical — proving
        // the single shared body. Replacing the escaped quote in the attr form
        // with a raw quote reconstructs the text form exactly.
        assert_eq!(
            attr_escape(sample).replace("&quot;", "\""),
            html_escape(sample),
            "the escapers must differ only by '\"'-handling"
        );
    }

    // Load-bearing: `attr_escape` must leave spaces verbatim so an href matches
    // the on-disk page filename (`<group path>.html` keeps spaces literal).
    #[test]
    fn attr_escape_leaves_spaces_verbatim() {
        assert_eq!(attr_escape("Root.A B"), "Root.A B");
    }

    // The shared lower-level routine appends to a caller-supplied buffer and
    // selects the attribute hardening via the `attr` flag.
    #[test]
    fn html_escape_into_appends_and_honours_attr_flag() {
        let mut out = String::from("pre:");
        html_escape_into(&mut out, "x \"y\" <z>", false);
        assert_eq!(out, "pre:x \"y\" &lt;z&gt;");

        let mut out = String::new();
        html_escape_into(&mut out, "x \"y\" <z>", true);
        assert_eq!(out, "x &quot;y&quot; &lt;z&gt;");
    }
}
