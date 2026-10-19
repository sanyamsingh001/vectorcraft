    pub fn smooth_anchor(&mut self, i: usize) -> bool {
        let n = self.anchors.len();
        let prev = if i > 0 {
            Some(i - 1)
        } else if self.closed {
            n.checked_sub(1)
        } else {
            None
        };
        let next = if i + 1 < n {
            Some(i + 1)
        } else if self.closed {
            Some(0)
        } else {
            None
        };
        let at = |j: Option<usize>| j.and_then(|j| self.anchors.get(j)).map(|a| a.p);
        let (prev, next) = (at(prev), at(next));
        let Some(a) = self.anchors.get_mut(i) else { return false };
        if a.kind == AnchorKind::Smooth && a.has_in() && a.has_out() {
            return true;
        }
        let (pp, nn) = (prev.unwrap_or(a.p), next.unwrap_or(a.p));
        let dir = nn - pp;
        let l = dir.hypot();
        if l <= 1e-9 {
            return false;
        }
        let u = dir / l;
        a.h_in = a.p - u * (a.p.distance(pp) / 3.0);
        a.h_out = a.p + u * (a.p.distance(nn) / 3.0);
        a.kind = AnchorKind::Smooth;
        true
    }
