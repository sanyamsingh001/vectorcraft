fn field_text(value: f64, suffix: &str, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, value);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    format!("{s}{suffix}")
}
