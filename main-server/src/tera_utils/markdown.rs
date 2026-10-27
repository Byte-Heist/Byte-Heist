use std::cell::OnceCell;

use common::slug::Slug;
use markdown_it::{
    MarkdownIt, Node,
    plugins::{
        cmark::{block::heading::ATXHeading, inline::link::Link},
        extra::syntect::set_theme,
    },
};
use tera::{Kwargs, Map, State, TeraResult, Value};

use super::syntax_highlighting::SYNTECT_THEME;

thread_local! {
    static MARKDOWN: OnceCell<MarkdownIt> = const { OnceCell::new() };
}

fn render_markdown<F>(source: &str, f: F) -> String
where
    F: FnMut(&mut Node, u32),
{
    MARKDOWN.with(|markdown| {
        let parser = markdown.get_or_init(|| {
            let mut parser = markdown_it::MarkdownIt::new();
            markdown_it::plugins::cmark::add(&mut parser);
            markdown_it::plugins::extra::add(&mut parser);

            // todo: Adjust this if we ever need a light mode
            set_theme(&mut parser, SYNTECT_THEME);

            parser
        });

        let mut ast = parser.parse(source);

        let mut headers = vec![];

        // TODO: this is probbly better with a plugin
        ast.walk_mut(|e, _index| {
            if e.is::<Link>() {
                e.attrs.push(("rel", "nofollow noopener".to_owned()));
            }
            if e.is::<ATXHeading>() {
                let slug = format!("{}", Slug(&e.collect_text()));

                e.attrs.push(("id", slug.clone()));
                headers.push(slug);
            }
        });
        ast.walk_mut(f);

        format!("<div class=\"markdown\">{}</div>", ast.render())
    })
}

pub fn markdown_filter(value: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    if kwargs.iter().next().is_some() {
        return Err(tera::Error::message(
            "The markdown function takes no arguments",
        ));
    }
    let text = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Expected type to be a string"))?;
    Ok(Value::safe_string(&render_markdown(text, |_, _| ())))
}

pub fn markdown_with_table_of_contents(
    value: &Value,
    kwargs: Kwargs,
    _: &State,
) -> TeraResult<Value> {
    if kwargs.iter().next().is_some() {
        return Err(tera::Error::message(
            "The markdown function takes no arguments",
        ));
    }
    let text = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Expected type to be a string"))?;

    let mut headings = vec![];

    let content = render_markdown(text, |node, _index| {
        if let Some(e) = node.cast::<ATXHeading>() {
            headings.push((e.level, node.collect_text()))
        }
    });

    let mut map = Map::new();
    map.insert("content".into(), Value::safe_string(&content));
    map.insert(
        "headings".into(),
        Value::from(
            headings
                .into_iter()
                .map(|i| {
                    let mut map = Map::new();
                    map.insert("text".into(), Value::from(i.1));
                    map.insert("level".into(), Value::from(i.0));
                    Value::from(map)
                })
                .collect::<Vec<_>>(),
        ),
    );
    Ok(Value::from(map))
}
