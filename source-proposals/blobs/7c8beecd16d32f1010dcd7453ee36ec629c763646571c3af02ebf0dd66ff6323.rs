impl SliderValue {
    /// `value` on a rail over `range`, without a text.
    pub fn number(value: f64, range: std::ops::RangeInclusive<f64>) -> Self {
        Self { value, range, text: None }
    }

    /// `value` on a rail over `range` as [`plain_field`] shows it with `suffix` and at most
    /// `decimals` places: the number it shows (`value` rounded) and its text.
    pub fn field(value: f64, range: std::ops::RangeInclusive<f64>, suffix: &str, decimals: usize) -> Self {
        let number = field_text(value, "", decimals);
        Self { value: number.parse().unwrap_or(value), range, text: Some(format!("{number}{suffix}")) }
    }

    /// `value` on a rail over `range`, with `text`, for a field that writes its own text (a length
    /// in the document's unit, an egui `DragValue`).
    pub fn text(value: f64, range: std::ops::RangeInclusive<f64>, text: String) -> Self {
        Self { value, range, text: Some(text) }
    }
}
