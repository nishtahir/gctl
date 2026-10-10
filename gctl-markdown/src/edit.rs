/// Sets the `id` key in a file's YAML front matter, leaving everything else
/// (comments, key order, line endings) untouched.
///
/// An existing top-level `id:` line is replaced; otherwise one is inserted as
/// the first key. A file without front matter gets a new block.
pub fn set_front_matter_id(source: &str, id: &str) -> String {
    let (bom, rest) = match source.strip_prefix('\u{feff}') {
        Some(rest) => ("\u{feff}", rest),
        None => ("", source),
    };
    let newline = if rest.contains("\r\n") { "\r\n" } else { "\n" };
    // Quoted so ids made only of digits are never read back as numbers.
    let id_line = format!("id: \"{id}\"{newline}");

    let mut lines: Vec<&str> = rest.split_inclusive('\n').collect();
    let is_fence = |line: &str| line.trim_end() == "---";

    let close = match lines.first() {
        Some(first) if is_fence(first) => {
            lines[1..].iter().position(|l| is_fence(l)).map(|i| i + 1)
        }
        _ => None,
    };

    let Some(close) = close else {
        return format!("{bom}---{newline}{id_line}---{newline}{newline}{rest}");
    };

    let existing = lines[1..close].iter().position(|line| {
        line.strip_prefix("id:")
            .is_some_and(|value| value.is_empty() || value.starts_with([' ', '\t', '\r', '\n']))
    });

    let mut out = String::from(bom);
    match existing {
        Some(offset) => {
            let at = offset + 1;
            lines[at] = &id_line;
        }
        None => lines.insert(1, &id_line),
    }
    for line in lines {
        out.push_str(line);
    }
    out
}
