//! Average exactly two picked anchors and join them in one history entry.
//! Coordinates and handles are already in document space.
use std::collections::BTreeSet;

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_geom::{AnchorKind, PathData, Point, SubPath};

use super::path_mut;
use crate::cmd::bad;
use crate::{Result, Session};

const COMMAND: &str = "path.averageJoin";
const PICK_TWO: &str = "select exactly two open-path endpoints with Direct Selection";

#[derive(Clone, Copy)]
struct Pick {
    id: NodeId,
    subpath: usize,
    anchor: usize,
}

fn picks(s: &Session) -> Result<[Pick; 2]> {
    let st = s.doc()?;
    if !st.selection.guides.is_empty() || !st.selection.slices.is_empty() {
        return Err(bad(COMMAND, PICK_TWO));
    }
    let mut picks = Vec::with_capacity(2);
    // A whole object or a segment is not a request to choose its nearest endpoints.
    for &id in &st.selection.objects {
        if st.selection.segments.contains_key(&id) {
            return Err(bad(COMMAND, PICK_TWO));
        }
        // Toggling an object's last anchor can leave an empty entry: do not silently skip it.
        let anchors = st.selection.anchors.get(&id).filter(|anchors| !anchors.is_empty()).ok_or_else(|| bad(COMMAND, PICK_TWO))?;
        for &(subpath, anchor) in anchors {
            if picks.len() == 2 {
                return Err(bad(COMMAND, PICK_TWO));
            }
            picks.push(Pick { id, subpath, anchor });
        }
    }
    let [a, b] = picks.as_slice() else { return Err(bad(COMMAND, PICK_TWO)) };
    for pick in [a, b] {
        let sp = st.doc.node(pick.id).and_then(|n| n.path_data()).and_then(|p| p.subpaths.get(pick.subpath));
        if !sp.is_some_and(|sp| !sp.closed && sp.anchors.len() >= 2 && (pick.anchor == 0 || pick.anchor == sp.anchors.len() - 1)) {
            return Err(bad(COMMAND, PICK_TWO));
        }
        if a.id == b.id && a.subpath == b.subpath && sp.is_some_and(|sp| sp.anchors.len() <= 2) {
            return Err(bad(COMMAND, "joining must leave at least two anchors"));
        }
    }
    Ok([*a, *b])
}

pub(super) fn enabled(s: &Session) -> std::result::Result<(), String> {
    picks(s).map(|_| ()).map_err(|e| e.to_string())
}

fn path(s: &Session, id: NodeId) -> Result<PathData> {
    s.doc()?.doc.node(id).and_then(|n| n.path_data()).cloned().ok_or_else(|| bad(COMMAND, "selected object is not a path"))
}

fn subpath(path: &PathData, pick: Pick) -> Result<SubPath> {
    path.subpaths
        .get(pick.subpath)
        .filter(|sp| sp.anchors.get(pick.anchor).is_some())
        .cloned()
        .ok_or_else(|| bad(COMMAND, "selected anchor no longer exists"))
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::new(a.x / 2.0 + b.x / 2.0, a.y / 2.0 + b.y / 2.0)
}

/// Close an open path at the averaged endpoints, retaining its incoming and outgoing handles.
fn close(sp: &mut SubPath) -> Result<()> {
    let first = sp.anchors.first().copied().ok_or_else(|| bad(COMMAND, "empty subpath"))?;
    let mut last = sp.anchors.last().copied().ok_or_else(|| bad(COMMAND, "empty subpath"))?;
    let center = midpoint(first.p, last.p);
    last.translate(center - last.p);
    let first = sp.anchors.first_mut().ok_or_else(|| bad(COMMAND, "empty subpath"))?;
    first.translate(center - first.p);
    first.h_in = last.h_in;
    first.kind = AnchorKind::Corner;
    sp.anchors.pop();
    sp.closed = true;
    Ok(())
}

pub(super) fn run(s: &mut Session, params: &Value) -> Result<Value> {
    let smooth = match params.get("kind") {
        None => false,
        Some(Value::String(kind)) if kind == "corner" => false,
        Some(Value::String(kind)) if kind == "smooth" => true,
        _ => return Err(bad(COMMAND, "kind must be corner or smooth")),
    };
    let [a, b] = picks(s)?;
    let mut left = path(s, a.id)?;
    let mut right = path(s, b.id)?;
    let mut x = subpath(&left, a)?;
    let mut y = subpath(&right, b)?;
    let same_subpath = a.id == b.id && a.subpath == b.subpath;
    let joined;
    if same_subpath {
        close(&mut x)?;
        joined = 0;
    } else {
        let endpoint = |sp: &SubPath, i: usize| !sp.closed && sp.anchors.len() >= 2 && (i == 0 || i == sp.anchors.len() - 1);
        if !endpoint(&x, a.anchor) || !endpoint(&y, b.anchor) {
            return Err(bad(COMMAND, "joining different subpaths requires two open endpoints"));
        }
        if a.anchor == 0 {
            x.reverse();
        }
        if b.anchor != 0 {
            y.reverse();
        }
        let last = x.anchors.last_mut().ok_or_else(|| bad(COMMAND, "empty first subpath"))?;
        let first = y.anchors.first_mut().ok_or_else(|| bad(COMMAND, "empty second subpath"))?;
        let center = midpoint(last.p, first.p);
        last.translate(center - last.p);
        first.translate(center - first.p);
        last.h_out = first.h_out;
        last.kind = AnchorKind::Corner;
        joined = x.anchors.len() - 1;
        x.anchors.extend(y.anchors.into_iter().skip(1));
    }
    // Use the app's existing Smooth convention, rather than inventing a target-app algorithm.
    if smooth && !x.smooth_anchor(joined) {
        return Err(bad(COMMAND, "the joined anchor has no distinct neighbors to define a smooth direction"));
    }
    *left.subpaths.get_mut(a.subpath).ok_or_else(|| bad(COMMAND, "missing first subpath"))? = x;
    let mut selected_subpath = a.subpath;
    if !same_subpath {
        if a.id == b.id {
            left.subpaths.remove(b.subpath); // validated by subpath() before any mutation
            if b.subpath < selected_subpath {
                selected_subpath -= 1;
            }
        } else {
            right.subpaths.remove(b.subpath);
        }
    }
    s.edit("Average and Join", |d, sel| {
        *path_mut(d, a.id)? = left;
        if a.id != b.id {
            if right.subpaths.is_empty() {
                d.remove(b.id)?;
            } else {
                d.node_mut(b.id).ok_or(crate::EngineError::NoNode(b.id))?.pin_gradients();
                *path_mut(d, b.id)? = right;
            }
        }
        sel.set([a.id]);
        sel.anchors.insert(a.id, BTreeSet::from([(selected_subpath, joined)]));
        Ok(())
    })?;
    Ok(json!({"id": a.id.0, "subpath": selected_subpath, "anchor": joined}))
}
