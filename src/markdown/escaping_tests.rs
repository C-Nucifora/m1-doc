use super::*;
use crate::model::*;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

#[test]
fn every_metadata_kind_remains_literal_in_markdown() {
    let mut values = Vec::new();
    let mut field = |name: &str| {
        let value = format!(
            "{name}: <img src=x onerror=bad> &lt; | ` `` [x](injected) ![x](injected) *x* \"\r\nnext"
        );
        values.push(value.clone());
        value
    };
    let symbol_path = field("symbol");
    let enum_name = field("enum");
    let model = DocModel {
        title: field("title"),
        target_hardware: Some(field("hardware")),
        groups: vec![GroupDoc {
            path: "Root".into(),
            symbols: vec![SymbolDoc {
                path: symbol_path.clone(),
                anchor: "symbol".into(),
                type_label: field("type"),
                quantity: Some(field("quantity")),
                unit: Some(field("unit")),
                base_unit: Some(field("base")),
                security: Some(field("security")),
                tags: vec![field("tag")],
                classname: Some(field("class")),
                enum_ref: Some(enum_name.clone()),
                ..Default::default()
            }],
            functions: vec![FunctionDoc {
                path: field("function"),
                anchor: "function".into(),
                inputs: vec![(field("input"), field("input-type"))],
                return_type: Some(field("return")),
                annotations: vec![AnnotationDoc {
                    kind: field("annotation"),
                    args: vec![field("argument")],
                }],
                source_path: Some(field("source-path")),
                ..Default::default()
            }],
            tables: vec![TableDoc {
                path: field("table"),
                anchor: "table".into(),
                axes: vec![TableAxisDoc {
                    size: 2,
                    unit: Some(field("axis")),
                }],
                output_unit: Some(field("output")),
                ..Default::default()
            }],
            objects: vec![ObjectDoc {
                path: field("object"),
                anchor: "object".into(),
                class: Some(field("object-class")),
                members: vec![field("member")],
                ..Default::default()
            }],
            can_messages: vec![CanMessageDoc {
                path: field("message"),
                anchor: "message".into(),
                signals: vec![CanSignalDoc {
                    path: field("signal"),
                    anchor: "signal".into(),
                    unit: Some(field("signal-unit")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            references: vec![
                ReferenceDoc {
                    path: field("reference"),
                    anchor: "reference".into(),
                    target_raw: field("target"),
                    ..Default::default()
                },
                ReferenceDoc {
                    path: field("resolved-reference"),
                    anchor: "resolved-reference".into(),
                    target_resolved: Some(symbol_path),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        enums: vec![EnumDoc {
            name: enum_name,
            anchor: "enum".into(),
            default: Some(field("enum-default")),
            members: vec![EnumMemberDoc {
                name: field("enum-member"),
                value: 0,
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let original = model.clone();
    let original_json = crate::json::render(&model);
    let files = render(&model);
    let mut visible = String::new();
    let mut table_widths = Vec::new();
    for file in &files {
        let mut width = 0;
        let mut cells = 0;
        for event in Parser::new_ext(&file.body, Options::ENABLE_TABLES) {
            match event {
                Event::Text(s) | Event::Code(s) => visible.push_str(&s),
                Event::Start(Tag::Image { .. }) => panic!("injected image in {}", file.path),
                Event::Start(Tag::Link { dest_url, .. }) => {
                    assert_ne!(dest_url.as_ref(), "injected")
                }
                Event::Html(s) | Event::InlineHtml(s) => assert!(!s.contains("<img"), "{s}"),
                Event::Start(Tag::Table(columns)) => {
                    width = columns.len();
                    table_widths.push(width);
                }
                Event::Start(Tag::TableHead | Tag::TableRow) => cells = 0,
                Event::Start(Tag::TableCell) => cells += 1,
                Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                    assert_eq!(cells, width, "{}", file.body)
                }
                _ => {}
            }
        }
    }
    assert_eq!(table_widths, [9, 5, 2, 2]);
    for value in values {
        assert!(
            visible.contains(&value.replace("\r\n", " ")),
            "lost or interpreted {value:?}"
        );
    }
    assert_eq!(model, original);
    assert_eq!(crate::json::render(&model), original_json);
}

#[test]
fn source_destinations_cannot_end_the_link() {
    let base = "https://example.test/a%20b/(x)?q=&lt;&other=é) [bad](injected)";
    let expected =
        "https://example.test/a%20b/%28x%29?q=&lt;&other=%C3%A9%29%20%5Bbad%5D%28injected%29/";
    let link = helpers::SourceLinker {
        base: Some(base.into()),
        m1prj_path: Some("Project [a].m1prj"),
    };
    let md = format!(
        "{}\n{}",
        helpers::source_line("Script [a].m1scr", Some(base)),
        link.suffix(Some(u32::MAX))
    );
    let links: Vec<_> = Parser::new(&md)
        .filter_map(|e| match e {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        links,
        [
            format!("{expected}Script%20%5Ba%5D.m1scr"),
            format!("{expected}Project%20%5Ba%5D.m1prj#L4294967296")
        ]
    );
}

#[test]
fn graph_control_text_and_long_source_fences_stay_separate() {
    let group = "Root--> <img src=x> ` #35; &lt;";
    let from = format!("{group}.Update");
    let to = format!("{group}.Value");
    let source = "``````\n<!--m1-graph:group:1:526f6f74-->\n```mermaid\ngraph LR\n```\n</code></pre><img src=x>";
    let model = DocModel {
        groups: vec![GroupDoc {
            path: group.into(),
            symbols: vec![SymbolDoc {
                path: to.clone(),
                anchor: "value".into(),
                ..Default::default()
            }],
            functions: vec![FunctionDoc {
                path: from.clone(),
                anchor: "update".into(),
                source_text: Some(source.into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        graph: ProjectGraph {
            edges: vec![GraphEdge {
                from,
                to,
                kind: EdgeKind::Write,
            }],
        },
        ..Default::default()
    };
    let md = render_with(
        &model,
        &RenderOptions {
            include_source: true,
            ..Default::default()
        },
    );
    let page = &md[1];
    let mut language = String::new();
    let mut source_body = String::new();
    let mut mermaid_blocks = 0;
    for event in Parser::new(&page.body) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                language = lang.into_string();
                if language == "mermaid" {
                    mermaid_blocks += 1;
                }
            }
            Event::End(TagEnd::CodeBlock) => language.clear(),
            Event::Text(s) if language == "m1" => source_body.push_str(&s),
            _ => {}
        }
    }
    assert_eq!(source_body, format!("{source}\n"));
    assert_eq!(mermaid_blocks, 1);
    let html = crate::html::render(&md, &model);
    let page = &html[1].body;
    assert_eq!(page.matches("<figure class=\"m1-graph\">").count(), 1);
    assert!(page.contains("&lt;!--m1-graph:group:1:526f6f74--&gt;"));
    assert!(!page.contains("<img"));
    assert!(page.contains("<code class=\"language-m1\">"));
}

#[test]
fn block_markers_in_group_names_remain_breadcrumbs_and_headings() {
    for name in [
        "~~~",
        "~~~ info",
        "~~~~",
        "~~gone~~",
        "***",
        "---",
        "___",
        "+ item",
        "# heading",
        "> quote",
        "[x](bad)",
        "<div>",
        "    indented",
    ] {
        let model = DocModel {
            groups: vec![GroupDoc {
                path: name.into(),
                symbols: vec![SymbolDoc {
                    path: format!("{name}.Speed"),
                    anchor: "speed".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let md = render(&model);
        let page = &md[1];
        let mut tables = 0;
        let mut heading = false;
        let mut title = String::new();
        for event in Parser::new_ext(
            &page.body,
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
        ) {
            match event {
                Event::Start(
                    Tag::CodeBlock(_) | Tag::Strikethrough | Tag::Link { .. } | Tag::BlockQuote(_),
                ) => panic!("interpreted group {name:?}: {page}", page = page.body),
                Event::Start(Tag::Table(_)) => tables += 1,
                Event::Start(Tag::Heading {
                    level: pulldown_cmark::HeadingLevel::H1,
                    ..
                }) => heading = true,
                Event::End(TagEnd::Heading(_)) => heading = false,
                Event::Text(text) if heading => title.push_str(&text),
                _ => {}
            }
        }
        assert_eq!(tables, 1, "{name}");
        assert_eq!(title, name);
        let html = crate::html::render(&md, &model);
        assert!(html[1].body.contains("<table>"));
        assert!(html[1].body.contains("id=\"speed\""));
        assert!(!html[1].body.contains("<pre>"));
    }
}
