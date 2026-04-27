use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;

#[derive(Default)]
struct TestRow {
    file: String,
    test: String,
    result: String,
    ms: u128,
    detail: String,
}

fn json_field(line: &str, key: &str) -> String {
    let needle = format!("\"{key}\":\"");
    let Some(start) = line.find(&needle).map(|idx| idx + needle.len()) else {
        return String::new();
    };
    let mut out = String::new();
    let mut escaped = false;
    for ch in line[start..].chars() {
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            break;
        } else {
            out.push(ch);
        }
    }
    out
}

fn json_number(line: &str, key: &str) -> u128 {
    let needle = format!("\"{key}\":");
    let Some(start) = line.find(&needle).map(|idx| idx + needle.len()) else {
        return 0;
    };
    let digits: String = line[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().unwrap_or(0)
}

fn classify(row: &TestRow) -> &'static str {
    match row.result.as_str() {
        "timeout" => "timeout",
        "panic" | "crash" => "panic/crash",
        "skip" if row.detail.contains("not supported") => "unsupported external/resource",
        "error" if row.detail.contains("void variable") || row.detail.contains("void function") => {
            "load/missing symbol"
        }
        "error" if row.detail.contains("wrong type argument") => "signal/type mismatch",
        "fail" => "assertion failure",
        "error" => "runtime error",
        "skip" => "skip",
        _ => "pass",
    }
}

fn main() -> std::io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("EMACS_TEST_RESULT_PATH").ok())
        .unwrap_or_else(|| "target/emacs-test-results.jsonl".to_string());
    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_file: HashMap<String, BTreeMap<String, usize>> = HashMap::new();
    let mut classes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut slow: Vec<TestRow> = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if !line.starts_with('{') {
            continue;
        }
        let row = TestRow {
            file: json_field(&line, "file"),
            test: json_field(&line, "test"),
            result: json_field(&line, "result"),
            ms: json_number(&line, "ms"),
            detail: json_field(&line, "detail"),
        };
        *counts.entry(row.result.clone()).or_default() += 1;
        *by_file
            .entry(row.file.clone())
            .or_default()
            .entry(row.result.clone())
            .or_default() += 1;
        *classes.entry(classify(&row)).or_default() += 1;
        slow.push(row);
    }

    println!("Results from {path}");
    println!("Totals:");
    for (result, count) in &counts {
        println!("  {result}: {count}");
    }

    println!("\nFailure classes:");
    for (class, count) in classes.iter().filter(|(class, _)| **class != "pass") {
        println!("  {class}: {count}");
    }

    let mut worst: Vec<_> = by_file.into_iter().collect();
    worst.sort_by_key(|(_, counts)| {
        std::cmp::Reverse(
            counts
                .iter()
                .filter(|(result, _)| result.as_str() != "pass")
                .map(|(_, count)| *count)
                .sum::<usize>(),
        )
    });
    println!("\nWorst files:");
    for (file, counts) in worst.into_iter().take(20) {
        let bad: usize = counts
            .iter()
            .filter(|(result, _)| result.as_str() != "pass")
            .map(|(_, count)| *count)
            .sum();
        if bad == 0 {
            continue;
        }
        println!("  {bad}: {file}");
    }

    slow.sort_by_key(|row| std::cmp::Reverse(row.ms));
    println!("\nSlowest tests:");
    for row in slow.into_iter().take(20).filter(|row| row.ms > 0) {
        println!("  {}ms {} :: {}", row.ms, row.file, row.test);
    }
    Ok(())
}
