    pub fn pin_gradients(&mut self) {
        if self.appearance.has_unplaced_gradient()
            && let Some(b) = self.geometric_bounds()
        {
            self.appearance.pin_gradients(b);
        }
    }
    /// The paint behind the Fill (or Stroke) proxy, the map from its space to the document and
    /// the box (in that space) an unplaced gradient fits. `item` (the Appearance panel's active
    /// item) stands in for the topmost fill or stroke when it is of the proxy's kind. Otherwise
    /// type objects paint their runs (the first run speaks for all) in text space, fitted to the
    /// layout bounds; everything else paints its fill or stroke in the document, fitted to the
    /// geometric bounds. Strokes fit the box grown by half their weight.
