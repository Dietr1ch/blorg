use std::path::Path;

use indoc::indoc;
use orgize::Org;
use orgize::export::HtmlEscape;
use orgize::export::HtmlExport;
use orgize::export::{Container, Event, Traverser, from_fn_with_ctx};
use slugify::slugify;

const HTML_HEADING_LEVELS: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];

pub struct RssConfig {
    root_address: String,
}

impl RssConfig {
    pub fn new(root_address: String) -> Self {
        Self { root_address }
    }
}

pub fn to_rss_item(config: &RssConfig, doc: &Org, file_rel_path: &Path) -> Option<rss::Item> {
    let mut item = rss::ItemBuilder::default();

    if let Some(properties) = doc.document().properties() {
        if properties.get("skip_feed").is_some() {
            return None;
        }

        if let Some(title) = properties.get("title") {
            item.title(title.to_string());
        }
        if let Some(description) = properties.get("description") {
            item.description(description.to_string());
        }
        if let Some(publication_date) = properties.get("publication_date") {
            item.pub_date(publication_date.to_string());
        }
        let mut path = file_rel_path.to_path_buf();
        path.set_extension("");
        item.link(Some(format!("{}/{}", config.root_address, path.display())));
    }

    Some(item.build())
}

pub fn to_html(doc: Org, tags: &[String], file_rel_path: &Path) -> Result<String, std::io::Error> {
    let mut html_export = HtmlExport::default();
    let file_name = file_rel_path.file_name().unwrap().to_str().unwrap();
    let file_stem = file_name.trim_end_matches(".org");

    assert!(file_rel_path.is_relative());

    let mut base_depth = 0i8;
    if let Some(properties) = doc.document().properties()
        && let Some(base_depth_token) = properties.get("base_depth")
    {
        base_depth = base_depth_token.as_ref().parse::<i8>().unwrap_or(0);
    }

    let mut handler = from_fn_with_ctx(|event, ctx| {
        match event {
            Event::Enter(Container::Document(_doc)) => {
                // Add title if present
                if let Some(title) = doc.title() {
                    // Parse title as Org document
                    let title = Org::parse(title);
                    let title = title.first_node::<orgize::ast::Paragraph>().unwrap();

                    use orgize::rowan::ast::AstNode;
                    let mut html = HtmlExport::default();
                    html.render(title.syntax());
                    let title_html = html.finish();
                    // Drop surrounding <p>...</p>
                    let title_html = &title_html[3..title_html.len() - 3 - 1];

                    let depth = base_depth;
                    let heading = HTML_HEADING_LEVELS[(depth - 1).clamp(0, 5) as usize];

                    if tags.is_empty() {
                        // <H*>$TITLE</H*>
                        html_export.push_str(format!(r#"<{heading}>{title_html}</{heading}>"#));
                    } else {
                        // <hgroup>
                        //   <H*>$TITLE</H*>
                        //   <p>Tags: <dd-tag>TAG</dd-tag>...</p>
                        // </hgroup>
                        html_export.push_str(format!(
                            r#"<hgroup><{heading}>{title_html}</{heading}><p>Tags:"#
                        ));
                        for t in tags {
                            html_export.push_str(format!(r#" <dd-tag>{t}</dd-tag>"#));
                        }
                        html_export.push_str(r#"</p></hgroup>"#);
                    }
                }
            }

            Event::Enter(Container::Link(link)) => {
                let path = link.path();
                let mut path: &str = path.trim_start_matches("file:");
                log::debug!("Linking to: {path:?}");

                let local_link_prefix = format!("./{file_stem}/");

                // Handle local links
                let is_local_org_link = path.ends_with(".org");
                let is_local_link = path.starts_with(&local_link_prefix);
                if is_local_link {
                    path = path.strip_prefix(&local_link_prefix).unwrap();

                    if path.ends_with(".org") {
                        path = path.strip_suffix(".org").unwrap();
                    }
                }

                let target = HtmlEscape(&path);

                if link.is_image() {
                    html_export.push_str(format!(r#"<img src="{}">"#, target));
                    return ctx.skip();
                }

                html_export.push_str(if is_local_link {
                    if is_local_org_link {
                        format!(
                            indoc! {r###"
                                <a hx-get="{0}/_.html"
                                  preload
                                  hx-target="#content"
                                  hx-push-url="{0}/"
                                  hx-history-target="{0}/"
                                  aria-controls="content"
                                  href="{0}" />
                        "###},
                            target
                        )
                    } else {
                        // Not a local .org link
                        format!(
                            indoc! {r###"
                                <a target="blank"
                                  preload
                                  href="{0}" />
                        "###},
                            target
                        )
                    }
                } else {
                    format!(
                        indoc! {r###"
                                <a href="{}"
                                  preload
                                  target="_blank" />
                        "###},
                        target
                    )
                });

                if !link.has_description() {
                    html_export.push_str(format!("{}</a>", target.0.trim()));
                    ctx.skip();
                }
            }

            Event::Enter(Container::SourceBlock(block)) => {
                if let Some(language) = block.language() {
                    html_export.push_str(format!(
                        r#"<pre><code class="language-{}">"#,
                        HtmlEscape(&language)
                    ));
                } else {
                    html_export.push_str("<pre><code>");
                }
            }
            Event::Leave(Container::SourceBlock(_block)) => {
                html_export.push_str("</code></pre>");
            }

            Event::Enter(Container::FixedWidth(fixed)) => {
                html_export.push_str(format!(
                    r#"<pre><samp class="org_result">{}</samp></pre>"#,
                    HtmlEscape(fixed.value()),
                ));
                ctx.skip();
            }

            Event::Enter(Container::ExportBlock(e)) => {
                // Don't enter this block
                ctx.skip();

                // TODO: Check that the export type is "html"
                html_export.push_str(e.value());
            }
            Event::Enter(Container::PropertyDrawer(properties)) => {
                ctx.skip();

                for (k, v) in properties.iter() {
                    match (k.as_ref(), v.as_ref()) {
                        ("ID", id) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:id\" content=\"{}\" />",
                                id,
                            ));
                        }
                        ("published_time", published_time) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:published_time\" content=\"{}\" />",
                                published_time,
                            ));
                        }
                        ("modified_time", modified_time) => {
                            html_export.push_str(format!(
                                "<meta property=\"article:modified_time\" content=\"{}\" />",
                                modified_time,
                            ));
                        }
                        (k, v) => {
                            log::debug!("Ignoring property {}:{}", k, v);
                        }
                    }
                }
            }

            Event::Enter(Container::Headline(headline)) => {
                let depth = (headline.level() as i8) + base_depth;
                let heading = HTML_HEADING_LEVELS[(depth - 1).clamp(0, 5) as usize];
                let title = headline.title().map(|e| e.to_string()).collect::<String>();
                let slug = slugify!(&title);

                if title.starts_with(".") {
                    base_depth -= 1;

                    html_export.push_str(format!(
                        "<div class=\"s{} {}\">",
                        headline.level(),
                        title.replace(".", " ").trim()
                    ));
                } else {
                    // <section id="$SLUG(TITLE)">
                    html_export.push_str(format!(
                        "<section id=\"{}\" class=\"s{}\">",
                        slug,
                        headline.level(),
                    ));
                    //   <hgroup>
                    //   <$HEADING>
                    // TODO: Add hgroup support
                    // html_export.push_str(format!("<hgroup>"));
                    html_export.push_str(format!("<{heading}>"));
                    //     <a href="SLUG($TITLE)" />
                    html_export.push_str(format!("<a href=\"#{0}\">", slug));
                    if headline.is_todo() {
                        html_export.push_str("<span class=\"org_todo\">TODO</span> ");
                    }
                    if headline.is_done() {
                        html_export.push_str("<span class=\"org_todo_done\">DONE</span> ");
                    }
                    //     $HEADLINE.title
                    for elem in headline.title() {
                        html_export.element(elem, ctx);
                    }

                    //     </a>
                    html_export.push_str("</a>");
                    //   </$HEADING>
                    //   </hgroup>
                    html_export.push_str(format!("</{heading}>"));
                    // TODO: Add hgroup support
                    // html_export.push_str(format!("</hgroup>"));
                }
            }
            Event::Leave(Container::Headline(headline)) => {
                let title = headline.title().map(|e| e.to_string()).collect::<String>();
                if title.starts_with(".") {
                    base_depth += 1;
                    html_export.push_str("</div>");
                } else {
                    // </section>
                    html_export.push_str("</section>");
                }
            }

            _ => {
                log::debug!("Default handling for {:?}...", event);
                html_export.event(event, ctx);
            }
        }
    });

    doc.traverse(&mut handler);

    Ok(html_export.finish())
}
