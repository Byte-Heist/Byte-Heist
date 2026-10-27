use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;
use tera::{Kwargs, State, TeraResult, Value};

use super::escape_html_str;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    file: String,
    #[serde(default)]
    #[allow(unused)]
    name: Option<String>,
    #[serde(default)]
    #[allow(unused)]
    is_entry: bool,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    imports: Vec<String>,
    #[allow(unused)]
    #[serde(default)]
    dynamic_imports: Vec<String>,
}

struct Imports<'a> {
    scripts: Vec<&'a str>,
    styles: Vec<&'a str>,
}

struct ModuleNotFound(String);

impl ManifestEntry {
    fn find_all_imports(mut files: Vec<&str>) -> Result<Imports<'_>, ModuleNotFound> {
        use std::fs::OpenOptions;

        let manifest = VITE_MANIFEST.get_or_init(|| {
            let file = OpenOptions::new()
                .read(true)
                .open("static/target/.vite/manifest.json")
                .expect("Failed to open vite manifest. Did you run `vite build`?");

            serde_json::from_reader(file).unwrap()
        });

        let mut scripts = vec![];
        let mut styles = vec![];
        while let Some(file) = files.pop() {
            let value = manifest
                .get(file)
                .ok_or_else(|| ModuleNotFound(file.to_owned()))?;
            scripts.push(value.file.as_str());
            styles.extend(value.css.iter().map(|k| k.as_str()));

            for import in &value.imports {
                files.push(import);
            }
        }

        Ok(Imports { scripts, styles })
    }
}

static VITE_MANIFEST: OnceLock<HashMap<String, ManifestEntry>> = OnceLock::new();

pub fn load_assets(kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    let scripts = kwargs
        .get::<&Value>("modules")?
        .ok_or_else(|| tera::Error::message("Expected argument \"modules\'"))?;
    let modules = if let Some(arr) = scripts.as_array() {
        arr.iter()
            .map(|k| k.as_str())
            .collect::<Option<Vec<&str>>>()
            .ok_or_else(|| tera::Error::message("Expected modules to be an array of strings"))?
    } else if let Some(k) = scripts.as_str() {
        vec![k]
    } else {
        return Err(tera::Error::message(
            "Expected scripts to be a string or array",
        ));
    };

    if cfg!(debug_assertions) {
        let mut out: String =
            r#"<script type="module" src="http://localhost:5173/static/target/@vite/client"></script>"#
                .to_string();
        for module in modules {
            out.push_str(&format!(
                r#"<script type="module" src="http://localhost:5173/static/target/{}"></script>"#,
                escape_html_str(module)
            ));
        }

        Ok(Value::from(out))
    } else {
        let imports = match ManifestEntry::find_all_imports(modules) {
            Ok(e) => e,
            Err(ModuleNotFound(name)) => {
                return Err(tera::Error::message(format!(
                    "Cannot find module \"{name}\""
                )));
            }
        };
        Ok(Value::from(
            imports
                .scripts
                .into_iter()
                .map(|script| {
                    format!(
                        r#"<script type="module" src="/static/target/{}"></script>"#,
                        escape_html_str(script)
                    )
                })
                .chain(imports.styles.into_iter().map(|style| {
                    format!(
                        r#"<link rel="stylesheet" href="/static/target/{}">"#,
                        escape_html_str(style)
                    )
                }))
                .collect::<String>(),
        ))
    }
}
