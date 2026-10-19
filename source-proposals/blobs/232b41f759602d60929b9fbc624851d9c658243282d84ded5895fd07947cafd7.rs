pub struct SliderValue {
    pub value: f64,
    pub range: std::ops::RangeInclusive<f64>,
    /// The field's text, such as "50%"; `None` gives the number alone.
    pub text: Option<String>,
}
