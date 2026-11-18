use super::{Counts, Evidence, Site, json};
use crate::model::{Error, Result};
use serde_json::Value;

pub(super) fn decode(id: &str, format: Option<&str>, output: &str) -> Result<Evidence> {
    if format.is_none() {
        let value: Value = json(output)?;
        let required = if id == "S15" { "lines" } else { "findings" };
        if value.get(required).is_none() {
            return Err(invalid(required));
        }
        return json(output);
    }
    let value: Value = json(output)?;
    match (id, format) {
        ("S12", Some("knip")) => knip(&value),
        ("S12", Some("machete")) => machete(&value),
        ("S13", Some("eslint")) => eslint(&value, "no-deprecated"),
        ("S15", Some("llvm")) => {
            let runs = list(&value, "data")?;
            if runs.len() != 1 || value["type"] != "llvm.coverage.json.export" {
                return Err(invalid("one LLVM coverage export"));
            }
            coverage(&runs[0]["totals"], "count")
        }
        ("S15", Some("istanbul")) => coverage(&value["total"], "total"),
        ("S24", Some("eslint")) => eslint(&value, "no-floating-promises"),
        ("S25", Some("eslint")) => eslint(&value, "jsx-a11y/"),
        _ => Err(invalid("an evidence format supported for this signal")),
    }
}

fn invalid(field: &str) -> Error {
    Error::new(
        "EVIDENCE_OUTPUT",
        format!("Expected {field} in the evidence output"),
    )
}

fn list<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value[field].as_array().ok_or_else(|| invalid(field))
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field].as_str().ok_or_else(|| invalid(field))
}

fn count(value: &Value, key: &str) -> Result<u64> {
    value[key].as_u64().ok_or_else(|| invalid(key))
}

fn site(path: &str, value: &Value, detail: String) -> Result<Site> {
    let line = value.get("line").map_or(Ok(1), |_| count(value, "line"))?;
    let line = usize::try_from(line).map_err(|_| invalid("line"))?;
    Ok(Site {
        path: path.into(),
        line,
        detail,
    })
}

fn knip(value: &Value) -> Result<Evidence> {
    let mut evidence = Evidence::default();
    for file in list(value, "issues")? {
        let path = string(file, "file")?;
        for group in [
            "files",
            "dependencies",
            "devDependencies",
            "optionalPeerDependencies",
            "exports",
            "types",
            "nsExports",
            "nsTypes",
        ] {
            if file.get(group).is_some() {
                for item in list(file, group)? {
                    evidence.findings.push(site(
                        path,
                        item,
                        format!("{group}: {}", string(item, "name")?),
                    )?);
                }
            }
        }
    }
    Ok(evidence)
}

fn machete(value: &Value) -> Result<Evidence> {
    if value.as_object().is_some_and(|map| map.is_empty()) {
        return Ok(Evidence::default());
    }
    let mut evidence = Evidence::default();
    for item in list(value, "crates")? {
        let path = string(item, "manifest_path")?;
        for name in list(item, "unused")? {
            let name = name
                .as_str()
                .ok_or_else(|| invalid("unused dependency name"))?;
            evidence.findings.push(site(
                path,
                &Value::Null,
                format!("Unused dependency: {name}"),
            )?);
        }
    }
    Ok(evidence)
}

fn coverage(totals: &Value, field: &str) -> Result<Evidence> {
    let counts = |value: &Value| -> Result<Counts> {
        Ok(Counts {
            covered: count(value, "covered")?,
            total: count(value, field)?,
        })
    };
    Ok(Evidence {
        lines: Some(counts(&totals["lines"])?),
        branches: totals.get("branches").map(counts).transpose()?,
        ..Evidence::default()
    })
}

fn eslint(value: &Value, rule_match: &str) -> Result<Evidence> {
    let mut evidence = Evidence::default();
    for file in value
        .as_array()
        .ok_or_else(|| invalid("ESLint file results"))?
    {
        let path = string(file, "filePath")?;
        for message in list(file, "messages")? {
            if message["fatal"].as_bool() == Some(true) {
                return Err(invalid(
                    "a complete ESLint analysis without fatal diagnostics",
                ));
            }
            if let Some(rule) = message["ruleId"].as_str()
                && (rule.starts_with(rule_match) || rule.rsplit('/').next() == Some(rule_match))
            {
                evidence.findings.push(site(
                    path,
                    message,
                    format!("{rule}: {}", string(message, "message")?),
                )?);
            }
        }
    }
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_only_measured_diagnostics_and_coverage() {
        let dead = decode("S12", Some("knip"), r#"{"issues":[{"file":"src/a.ts","exports":[{"name":"unused","line":3}],"unlisted":[{"name":"missing"}]}]}"#).unwrap();
        assert_eq!(
            dead.findings,
            [Site {
                path: "src/a.ts".into(),
                line: 3,
                detail: "exports: unused".into()
            }]
        );
        assert_eq!(
            decode(
                "S12",
                Some("machete"),
                r#"{"crates":[{"manifest_path":"Cargo.toml","unused":["unused"]}]}"#
            )
            .unwrap()
            .findings
            .len(),
            1
        );
        assert!(decode("S12", None, "{}").is_err());
        let cov = decode("S15", Some("llvm"), r#"{"type":"llvm.coverage.json.export","data":[{"totals":{"lines":{"covered":23,"count":25}}}]}"#).unwrap();
        assert_eq!(cov.lines.unwrap().total, 25);
        assert!(cov.branches.is_none());
        let lint = decode("S24", Some("eslint"), r#"[{"filePath":"src/a.ts","messages":[{"ruleId":"@typescript-eslint/no-floating-promises","message":"Handle promise","line":12},{"ruleId":"prefer-const","message":"Use const","line":4}]}]"#).unwrap();
        assert_eq!(lint.findings[0].line, 12);
        assert_eq!(lint.findings.len(), 1);
        assert!(
            decode(
                "S25",
                Some("eslint"),
                r#"[{"filePath":"src/a.ts","messages":[{"fatal":true}]}]"#
            )
            .is_err()
        );
    }
}
