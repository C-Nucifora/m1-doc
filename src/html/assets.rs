//! The inline, self-contained page assets: the full CSS (`STYLE`) and the
//! vanilla-JS behaviour bundle (`SCRIPT`). Both are embedded verbatim in every
//! page by [`super::render`] so the site needs no external stylesheet, font,
//! script, or CDN (#33). Pure data — no rendering logic lives here.

// ---------------------------------------------------------------------------
// Internal CSS
// ---------------------------------------------------------------------------
//
// All styling is inline (no external stylesheet, font, or CDN) so the site is
// self-contained and works from `file://` and GitHub Pages alike (#33). Colours
// are expressed through CSS custom properties; dark mode flips the variables
// via `prefers-color-scheme` and an explicit `[data-theme]` override the toggle
// sets (persisted in localStorage by the inline script).

pub(super) const STYLE: &str = r#"
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
// Inline behaviour (#31 search, #33 polish, #34 filters)
// ---------------------------------------------------------------------------
//
// One vanilla-JS module, inlined in every page. No external script, no build
// step, CSP-friendly (a single inline <script>). It is defensive: every feature
// is guarded on the element existing, so the index page (no filter panel) and a
// group page (no enums) both run the same script without error.

pub(super) const SCRIPT: &str = r##"
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
// ---- client-side search over the shared index ----
function initSearch(){
  var box=document.getElementById("search-box");
  var results=document.getElementById("search-results");
  if(!box||!results)return;
  // The index is one shared sibling file (search-index.js) loaded before this
  // script, so it is parsed once per project rather than inlined per page.
  var index=window.__M1_SEARCH_INDEX__||[];
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
