use std::str::FromStr;
use std::sync::OnceLock;

use axum::response::{IntoResponse, Response};
use common::AchievementType;
use common::langs::LANGS;
use sqlx::types::time::OffsetDateTime;
use tera::{Kwargs, State, Tera, TeraResult, Value};
use tower_sessions::cookie::time::macros::format_description;

use crate::tera_utils::markdown::{markdown_filter, markdown_with_table_of_contents};
use crate::tera_utils::syntax_highlighting::syntax_highlight;

use super::{render_html_error, vite::load_assets};

static TERA: OnceLock<TeraResult<Tera>> = OnceLock::new();

pub enum GetTerraError {
    Initalizing(&'static tera::Error),
}

impl IntoResponse for GetTerraError {
    fn into_response(self) -> Response {
        match self {
            GetTerraError::Initalizing(error) => {
                render_html_error("Error initializing Tera", error)
            }
        }
    }
}

pub fn get_tera() -> Result<&'static Tera, GetTerraError> {
    let value = TERA.get_or_init(|| {
        let mut tera = Tera::new();
        tera.load_from_glob("templates/**/*")?;
        tera.autoescape_on(vec![".html.jinja", ".xml.jinja", ".html", ".xml"]);
        tera.register_function("languages", get_langs);
        tera.register_function("modules", load_assets);
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter(
            "markdown_with_table_of_contents",
            markdown_with_table_of_contents,
        );
        tera.register_filter("prepend_linebreak", prepend_line_break);
        tera.register_filter("format_number", format_number);
        tera.register_filter("format_date", format_date);
        tera.register_test("empty", empty);
        tera.register_filter("syntax_highlight", syntax_highlight);
        tera.register_filter("slugify", slugify);
        tera.register_filter("urlencode_strict", urlencode_strict);
        tera.register_filter("get_achievement_icon", get_achievement_icon);
        tera.register_filter("get_achievement_name", get_achievement_name);
        tera.register_filter("get_achievement_description", get_achievement_description);
        tera.register_filter("un_camelcase", un_camelcase);
        tera.register_filter("language_display_name", get_lang_name);
        tera.register_filter("language_syntax", get_lang_syntax);
        Ok(tera)
    });

    let tera = match value {
        Ok(tera) => tera,
        Err(e) => {
            return Err(GetTerraError::Initalizing(e));
        }
    };

    Ok(tera)
}

fn get_lang_name(e: &str, _: Kwargs, _: &State) -> String {
    LANGS
        .get(e)
        .map(|e| e.display_name)
        .unwrap_or(e)
        .to_string()
}

fn get_lang_syntax(e: &str, _: Kwargs, _: &State) -> String {
    LANGS
        .get(e)
        .map(|e| e.syntax_name)
        .unwrap_or("text")
        .to_string()
}

fn get_achievement_icon(e: &str, _: Kwargs, _: &State) -> String {
    AchievementType::from_str(e)
        .map(|k| k.get_icon())
        .unwrap_or_default()
}

fn get_achievement_name(e: &str, _: Kwargs, _: &State) -> String {
    AchievementType::from_str(e)
        .map(|k| k.get_achievement_name())
        .unwrap_or_default()
        .to_owned()
}

fn get_achievement_description(e: &str, _: Kwargs, _: &State) -> String {
    AchievementType::from_str(e)
        .map(|k| k.get_achievement_description())
        .unwrap_or_default()
        .to_owned()
}

fn un_camelcase(e: &str, _: Kwargs, _: &State) -> String {
    e.chars()
        .flat_map(|k| k.is_ascii_uppercase().then_some(' ').into_iter().chain([k]))
        .collect::<String>()
}

fn slugify(value: &Value, _: Kwargs, _: &State) -> TeraResult<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Expected a string"))?;
    Ok(Value::from(common::slug::Slug(s).to_string()))
}

fn urlencode_strict(value: &Value, _: Kwargs, _: &State) -> TeraResult<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::message("Expected a string"))?;
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    Ok(Value::from(out))
}

fn get_langs(kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    if kwargs.iter().next().is_some() {
        return Err(tera::Error::message(
            "Get langs function takes no arguments",
        ));
    }
    Value::try_from_serializable(&LANGS)
}

fn format_number(value: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    if kwargs.iter().next().is_some() {
        return Err(tera::Error::message(
            "The format string filter takes no parameters",
        ));
    }

    match value.as_i64() {
        Some(number) => Ok(Value::from(format_number_with_thousands_seperators(number))),
        None => Err(tera::Error::message(format!(
            "Expected a number, got {value}"
        ))),
    }
}

fn format_number_with_thousands_seperators(num: i64) -> String {
    use num_format::{Locale, ToFormattedString};

    num.to_formatted_string(&Locale::en)
}

fn format_date(value: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    let include_day = kwargs.get::<bool>("include_day")?.unwrap_or(true);
    if kwargs
        .iter()
        .any(|(key, _)| key.as_str() != Some("include_day"))
    {
        return Err(tera::Error::message(
            "The format string filter takes one paramter: include day",
        ));
    }
    let date: OffsetDateTime =
        serde_json::from_value(serde_json::to_value(value).map_err(|e| tera::Error::message(e))?)
            .map_err(|e| tera::Error::message(e))?;

    let offset = date - OffsetDateTime::now_utc();
    let (relative_time_prefix, relative_time_postfix) = if offset.is_positive() {
        ("in ", "")
    } else {
        ("", " ago")
    };
    let offset = offset.abs();

    Ok(Value::from(if offset.whole_weeks() > 12 {
        date.format(if include_day {
            format_description!("[day] [month repr:long] [year]")
        } else {
            format_description!("[month repr:long] [year]")
        })
        .map_err(|_e| tera::Error::message("unknown"))?
    } else if offset.whole_hours() == 0 {
        format!("{relative_time_prefix}a few minutes{relative_time_postfix}")
    } else if offset.whole_weeks() != 0 {
        format!(
            "{relative_time_prefix}{} weeks, {} days{relative_time_postfix}",
            offset.whole_weeks(),
            offset.whole_days() % 7
        )
    } else {
        format!(
            "{relative_time_prefix}{} days, {} hours{relative_time_postfix}",
            offset.whole_days(),
            offset.whole_hours() % 24
        )
    }))
}

fn prepend_line_break(value: &Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    if kwargs.iter().next().is_some() {
        return Err(tera::Error::message(
            "The format string filter takes no parameters",
        ));
    }
    match value.as_str() {
        Some(s) => Ok(Value::from(format!("\n{s}"))),
        None => Err(tera::Error::message(
            "prepend_line_break: Expected a string",
        )),
    }
}

fn empty(value: &Value, _: Kwargs, _: &State) -> bool {
    if let Some(s) = value.as_str() {
        s.is_empty()
    } else if let Some(values) = value.as_array() {
        values.is_empty()
    } else if let Some(map) = value.as_map() {
        map.is_empty()
    } else {
        false
    }
}
