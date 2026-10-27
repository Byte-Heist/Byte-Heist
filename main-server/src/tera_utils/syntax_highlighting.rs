use std::cell::OnceCell;

use syntect::html::highlighted_html_for_string;
use tera::{Kwargs, State, TeraResult, Value};

pub const SYNTECT_THEME: &str = "base16-eighties.dark";

thread_local! {
    static THEME: OnceCell<(syntect::highlighting::Theme, syntect::parsing::SyntaxSet)> = const { OnceCell::new() };
}

pub fn syntax_highlight(value: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    if kwargs.iter().count() != 1 {
        return Err(tera::Error::message(
            "The syntaxHighlight expects exactly one argument",
        ));
    }
    let lang = kwargs
        .get::<&str>("lang")?
        .ok_or_else(|| tera::Error::message("The syntaxHighlight expects one argument: lang"))?;

    let code = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Expected input of kind string"))?;

    THEME.with(|theme| -> TeraResult<Value> {
        let (theme, syntax_set) = theme.get_or_init(|| {
            let themes = syntect::highlighting::ThemeSet::load_defaults();
            let syntax_set = syntect::parsing::SyntaxSet::load_defaults_newlines();

            (themes.themes[SYNTECT_THEME].clone(), syntax_set)
        });

        let mut html = highlighted_html_for_string(
            code,
            syntax_set,
            syntax_set.find_syntax_by_token(lang).ok_or_else(|| {
                tera::Error::message(format!("Unknown syntax highlighting language: {lang}"))
            })?,
            theme,
        )
        .map_err(|e| tera::Error::message(format!("error: {e:?}")))?;

        html.insert_str(4, " class=\"code-pre\"");

        Ok(Value::safe_string(&html))
    })
}
