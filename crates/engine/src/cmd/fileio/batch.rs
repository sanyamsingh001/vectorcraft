//! `command.batch`: several commands as one undo step (in each document they edit).

use serde_json::{Value, json};

use super::super::*;
use crate::{DocState, EngineError};

pub(super) fn batch(s: &mut Session, p: &Value) -> Result<Value> {
    let mut cmds = p.get("commands").and_then(Value::as_array).ok_or_else(|| bad("command.batch", "missing commands"))?.clone();
    let label = str_param(p, "label").unwrap_or("Batch").to_string();
    // An empty batch owns no edits and must leave a live preview usable.
    if cmds.is_empty() {
        s.journal.push(("command.batch".into(), p.clone()));
        return Ok(json!({ "results": [] }));
    }
    // What an error rolls back besides the documents: session-level state a step may change
    // (paint defaults, drawing mode, clipboard, the Untitled-N count).
    let saved = (s.paint.clone(), s.fill_active, s.draw_mode, s.draw_inside, s.clipboard.clone(), s.untitled_counter);
    let (start, active): (Vec<u64>, _) = (s.docs.iter().map(|d| d.uid).collect(), s.active);
    // An idle Artboard tool has no preview to checkpoint, but deletion/reorder can change its
    // transient active index. Restore its options on rejection even when a tab reset the tool.
    let idle_artboard = (!s.in_tool_actions && s.lender.is_none() && s.tool_id() == "artboard" && !s.tool_busy()).then(|| s.tool.options());
    // Keep the original tool instance untouched while fallible steps run. Closing or reverting a
    // document deactivates the installed tool, so a DocState snapshot cannot restore its gesture.
    let before = s
        .active()
        .filter(|st| {
            !s.in_tool_actions
                && s.lender.is_none()
                && (!s.tool_busy() || matches!(s.tool_id(), "pen" | "artboard"))
                && st.interaction.as_ref().is_some_and(|it| it.preview.is_some())
        })
        .cloned();
    let checkpoint = if let Some(doc) = before {
        let journal = s.journal.len();
        let pending = s.pending_paint.clone();
        let solid = s.last_solid;
        let gradient = s.last_gradient.clone();
        let colors = s.recent_colors.clone();
        s.commit_interaction()?;
        let mut fresh = s.make_tool(s.tool_id());
        // Keep transient options too, including the Artboard tool's active index.
        if let Some(options) = s.tool.options().as_object() {
            for (key, value) in options {
                fresh.set_option(key, value);
            }
        }
        let tool = std::mem::replace(&mut s.tool, fresh);
        // Hosted ruler state lives outside the ToolBox; park the original option too.
        let ruler_guide = s.ruler_guide.take();
        Some(PreviewBeforeBatch { doc, tool, ruler_guide, journal, pending, solid, gradient, colors })
    } else {
        None
    };
    s.batch_stash = Some(vec![]);
    let r = run_steps(s, &label, &mut cmds);
    let stash = s.batch_stash.take().unwrap_or_default();
    let results = match r {
        Ok(results) => results,
        Err(e) => {
            let replaced = checkpoint.as_ref().is_some_and(|p| stash.iter().any(|st| st.uid == p.doc.uid));
            roll_back(s, &start, stash);
            s.active = active;
            (s.paint, s.fill_active, s.draw_mode, s.draw_inside, s.clipboard, s.untitled_counter) = saved;
            s.pending_paint = None;
            if let Some(p) = checkpoint {
                if let Some(st) = s.docs.iter_mut().find(|st| st.uid == p.doc.uid) {
                    let revision = st.revision;
                    *st = p.doc;
                    // Replaced documents must invalidate caches, as existing batch rollback does.
                    if replaced {
                        st.revision = revision;
                    }
                }
                s.tool = p.tool;
                s.ruler_guide = p.ruler_guide;
                s.journal.truncate(p.journal);
                s.pending_paint = p.pending;
                s.last_solid = p.solid;
                s.last_gradient = p.gradient;
                s.recent_colors = p.colors;
            } else if let Some(options) = idle_artboard {
                let mut tool = s.make_tool("artboard");
                if let Some(options) = options.as_object() {
                    for (key, value) in options {
                        tool.set_option(key, value);
                    }
                }
                s.tool = tool;
                s.keep_tool_settings();
            }
            return Err(e);
        }
    };
    // Keep an inert original ruler option until its release; a late Drag must not create anew.
    if let Some(mut g) = checkpoint.and_then(|p| p.ruler_guide) {
        g.finish_for_command();
        s.ruler_guide = Some(g);
    }
    // Each document the steps edited keeps them as one undo step.
    s.remember_pending_paint();
    for st in &mut s.docs {
        if let Some(it) = st.interaction.take() {
            st.keep_interaction(it);
        }
    }
    // The batch journals itself, whichever document it ends in.
    let mut entry = p.clone();
    if let Some(o) = entry.as_object_mut() {
        o.insert("commands".into(), Value::Array(cmds));
    }
    s.journal.push(("command.batch".into(), entry));
    Ok(json!({ "results": results }))
}

/// A live panel, Pen or Artboard preview and its original tool, before the batch owns any edit.
struct PreviewBeforeBatch {
    doc: DocState,
    tool: Box<dyn vectorcraft_tools::Tool>,
    ruler_guide: Option<vectorcraft_tools::rulerguide::NewGuide>,
    journal: usize,
    pending: Option<vectorcraft_color::Paint>,
    solid: vectorcraft_color::Color,
    gradient: vectorcraft_color::GradientPaint,
    colors: Vec<vectorcraft_color::Color>,
}

/// Run the steps → their results. Each runs inside an interaction of the document active when it
/// starts (one for all the steps in that document), so a step may open, switch or close documents.
fn run_steps(s: &mut Session, label: &str, cmds: &mut [Value]) -> Result<Vec<Value>> {
    let mut results = Vec::with_capacity(cmds.len());
    for c in cmds {
        let id = c.get("command").and_then(Value::as_str).unwrap_or("").to_string();
        if id == "command.batch" {
            return Err(bad("command.batch", "batches cannot nest"));
        }
        if s.active().is_some() {
            s.begin_interaction(label)?;
        }
        let params = c.get("params").cloned().unwrap_or(json!({}));
        let (v, noted) =
            s.execute_step(&id, &params).map_err(|e| EngineError::Other(format!("batch step {} (`{id}`) failed: {e}", results.len())))?;
        // The step's journal entry records what it took from the preferences or the clock (a
        // save's date, a new document's), as a command of its own would.
        if noted != params
            && let Some(c) = c.as_object_mut()
        {
            c.insert("params".into(), noted);
        }
        results.push(v);
    }
    Ok(results)
}

/// The documents as they were before the batch (`start`: their uids in order): the ones it opened
/// close, the ones it closed or replaced (`stash`) come back, and each one's interaction is undone.
fn roll_back(s: &mut Session, start: &[u64], mut stash: Vec<DocState>) {
    let mut open = std::mem::take(&mut s.docs);
    for &uid in start {
        // The newest revision the document reached, so the restored one redraws.
        let newest = open.iter().chain(&stash).filter(|d| d.uid == uid).map(|d| d.revision).max().unwrap_or_default();
        // Its state before the batch: the first one a step closed or replaced, else the open one.
        let st = match stash.iter().position(|d| d.uid == uid) {
            Some(i) => Some(stash.remove(i)),
            None => open.iter().position(|d| d.uid == uid).map(|i| open.remove(i)),
        };
        if let Some(mut st) = st {
            st.undo_interaction();
            st.revision = newest.saturating_add(1);
            s.docs.push(st);
        }
    }
}
