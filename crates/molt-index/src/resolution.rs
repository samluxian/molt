//! Conservative file-level resolution for static JS/TS relative imports.
//! Does not interpret tsconfig paths, package exports, bundler plugins or runtime resolution.
use std::collections::HashSet;

pub(crate) struct Resolved {
    pub status: &'static str,
    pub target: Option<String>,
}

fn normalize(importer: &str, specifier: &str) -> Option<String> {
    let base = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut segments: Vec<&str> = if base.is_empty() {
        Vec::new()
    } else {
        base.split('/').collect()
    };
    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            part => segments.push(part),
        }
    }
    Some(segments.join("/"))
}

pub(crate) fn resolve(importer: &str, specifier: &str, paths: &HashSet<String>) -> Resolved {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return Resolved {
            status: "non_relative",
            target: None,
        };
    }
    if specifier.contains(['?', '#', '\\', '\0']) {
        return Resolved {
            status: "unsupported_specifier",
            target: None,
        };
    }
    let Some(base) = normalize(importer, specifier) else {
        return Resolved {
            status: "outside_root",
            target: None,
        };
    };
    if paths.contains(&base) {
        return Resolved {
            status: "resolved",
            target: Some(base),
        };
    }
    // JS emitted specifiers in TS sources may refer to .ts/.tsx (or .mts/.cts) source files.
    if let Some(stem) = base.strip_suffix(".js") {
        return candidates(&[format!("{stem}.ts"), format!("{stem}.tsx")], paths);
    }
    if let Some(stem) = base.strip_suffix(".mjs") {
        return candidates(&[format!("{stem}.mts")], paths);
    }
    if let Some(stem) = base.strip_suffix(".cjs") {
        return candidates(&[format!("{stem}.cts")], paths);
    }
    let extensions = ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"];
    if base
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
    {
        return Resolved {
            status: "unsupported_specifier",
            target: None,
        };
    }
    let direct: Vec<String> = extensions
        .iter()
        .map(|ext| format!("{base}.{ext}"))
        .collect();
    let resolved = candidates(&direct, paths);
    if resolved.status != "missing" {
        return resolved;
    }
    let index: Vec<String> = extensions
        .iter()
        .map(|ext| format!("{base}/index.{ext}"))
        .collect();
    candidates(&index, paths)
}

fn candidates(options: &[String], paths: &HashSet<String>) -> Resolved {
    let mut found = options
        .iter()
        .filter(|candidate| paths.contains(*candidate));
    let Some(first) = found.next() else {
        return Resolved {
            status: "missing",
            target: None,
        };
    };
    if found.next().is_some() {
        return Resolved {
            status: "ambiguous",
            target: None,
        };
    }
    Resolved {
        status: "resolved",
        target: Some(first.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_relative_specifiers_without_guessing() {
        let paths = [
            "app/index.tsx",
            "app/card.ts",
            "app/shared/index.ts",
            "app/x.mts",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            resolve("app/index.tsx", "./card", &paths).target.as_deref(),
            Some("app/card.ts")
        );
        assert_eq!(
            resolve("app/index.tsx", "./card.js", &paths)
                .target
                .as_deref(),
            Some("app/card.ts")
        );
        assert_eq!(
            resolve("app/index.tsx", "./shared", &paths)
                .target
                .as_deref(),
            Some("app/shared/index.ts")
        );
        assert_eq!(
            resolve("app/index.tsx", "./x.mjs", &paths)
                .target
                .as_deref(),
            Some("app/x.mts")
        );
        assert_eq!(
            resolve("app/index.tsx", "../../secret", &paths).status,
            "outside_root"
        );
        assert_eq!(
            resolve("app/index.tsx", "@/card", &paths).status,
            "non_relative"
        );
        assert_eq!(
            resolve("app/index.tsx", "./missing", &paths).status,
            "missing"
        );
        assert_eq!(
            resolve("app/index.tsx", "./card?raw", &paths).status,
            "unsupported_specifier"
        );
    }

    #[test]
    fn ambiguous_candidates_are_not_arbitrarily_selected() {
        let paths = ["a.ts", "a.js"].into_iter().map(str::to_owned).collect();
        assert_eq!(resolve("entry.ts", "./a", &paths).status, "ambiguous");
    }
}
