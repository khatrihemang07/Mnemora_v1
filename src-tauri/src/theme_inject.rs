//! Builds the script that sets theme attributes before first paint (no flash of wrong theme).

use std::collections::BTreeMap;

use mnemora_core::env::Environment;
use serde_json::{Value, json};

fn pick(values: &BTreeMap<String, Value>, key: &str, fallback: &str) -> String {
    values
        .get(key)
        .and_then(Value::as_str)
        .map_or_else(|| fallback.to_owned(), str::to_owned)
}

/// Window initialization script. It sets `data-theme` (resolved; `system` is decided in JS via
/// `matchMedia`), `data-theme-pref` (what the user chose), `data-density`, `data-text-size`,
/// `data-reduce-motion` and `data-env` on `<html>`. Values are embedded as JSON, never
/// string-concatenated, so a stored value can't break out of the script.
pub fn script(env: Environment, values: &BTreeMap<String, Value>) -> String {
    let prefs = json!({
        "theme": pick(values, "core.theme", "system"),
        "density": pick(values, "core.density", "comfortable"),
        "textSize": pick(values, "core.text_size", "m"),
        "reduceMotion": values.get("core.reduce_motion").and_then(Value::as_bool).unwrap_or(false),
        "env": env.as_str(),
    });
    format!(
        r#"(function () {{
  var p = {prefs};
  function apply() {{
    var d = document.documentElement;
    var resolved = p.theme;
    if (p.theme === "system") {{
      resolved = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }}
    d.dataset.theme = resolved;
    d.dataset.themePref = p.theme;
    d.dataset.density = p.density;
    d.dataset.textSize = p.textSize;
    d.dataset.reduceMotion = String(p.reduceMotion);
    d.dataset.env = p.env;
  }}
  try {{
    if (document.documentElement) {{ apply(); return; }}
    // The document element may not exist yet at injection time.
    new MutationObserver(function (_, obs) {{
      if (document.documentElement) {{ obs.disconnect(); apply(); }}
    }}).observe(document, {{ childList: true }});
  }} catch (e) {{}}
}})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    }

    #[test]
    fn embeds_saved_preferences_and_environment() {
        let js = script(
            Environment::Sandbox,
            &values(&[
                ("core.theme", json!("oled")),
                ("core.density", json!("compact")),
                ("core.reduce_motion", json!(true)),
            ]),
        );
        assert!(js.contains(r#""theme":"oled""#), "{js}");
        assert!(js.contains(r#""density":"compact""#));
        assert!(js.contains(r#""reduceMotion":true"#));
        assert!(js.contains(r#""env":"sandbox""#));
        assert!(js.contains("matchMedia"), "system is resolved in JS");
    }

    #[test]
    fn missing_values_use_defaults() {
        let js = script(Environment::Production, &BTreeMap::new());
        assert!(js.contains(r#""theme":"system""#));
        assert!(js.contains(r#""density":"comfortable""#));
        assert!(js.contains(r#""env":"production""#));
    }

    #[test]
    fn hostile_values_stay_inside_a_json_string() {
        let js = script(
            Environment::Production,
            &values(&[("core.theme", json!("\"};alert(1);//"))]),
        );
        assert!(js.contains(r#""theme":"\"};alert(1);//""#), "{js}");
    }
}
