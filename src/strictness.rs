use serde_json::Value;

pub const FLAGS: &[&str] = &[
    "noImplicitAny",
    "noImplicitThis",
    "strictNullChecks",
    "strictFunctionTypes",
    "strictBindCallApply",
    "strictPropertyInitialization",
    "strictBuiltinIteratorReturn",
    "alwaysStrict",
    "useUnknownInCatchVariables",
    "noUncheckedIndexedAccess",
    "noImplicitOverride",
    "noFallthroughCasesInSwitch",
    "exactOptionalPropertyTypes",
    "noImplicitReturns",
];

pub fn enabled(options: &Value) -> Vec<String> {
    FLAGS
        .iter()
        .filter(|&&flag| options[flag].as_bool() == Some(true))
        .map(|&flag| flag.into())
        .collect()
}

pub fn next_option(enabled: &[String]) -> Option<&'static str> {
    FLAGS
        .iter()
        .copied()
        .find(|flag| !enabled.iter().any(|current| current == flag))
}

pub fn tightening(before: &str, after: &str) -> bool {
    let (Ok(mut old), Ok(mut new)) = (
        serde_json::from_str::<Value>(before),
        serde_json::from_str::<Value>(after),
    ) else {
        return false;
    };
    let (Some(old_options), Some(new_options)) = (
        old.get_mut("compilerOptions")
            .and_then(Value::as_object_mut),
        new.get_mut("compilerOptions")
            .and_then(Value::as_object_mut),
    ) else {
        return false;
    };
    let mut tightened = false;
    for &flag in FLAGS.iter().chain(std::iter::once(&"strict")) {
        let old_value = old_options.remove(flag);
        let new_value = new_options.remove(flag);
        if old_value == new_value {
            continue;
        }
        if new_value != Some(Value::Bool(true))
            || !matches!(old_value, None | Some(Value::Bool(false)))
        {
            return false;
        }
        tightened = true;
    }
    tightened && old == new
}

#[cfg(test)]
#[path = "tests/strictness.rs"]
mod tests;
