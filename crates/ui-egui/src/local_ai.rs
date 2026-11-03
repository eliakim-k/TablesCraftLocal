use crate::SheetApp;
use craft_local_ai::{Source, transport::Cancel};
use gridcraft_engine::{DocState, display};

fn key(app: &SheetApp) -> Option<String> {
    app.session.active().map(|d| format!("{}:{}", d.uid, d.revision))
}
pub fn show(app: &mut SheetApp, ui: &mut egui::Ui) {
    if let Some(scope) = app.local_ai.show(ui, key(app), &["Selected cells / object", "Visible sheet", "Selected cells + formulas"]) {
        let work = snapshot(app, scope);
        app.local_ai.prepare(ui.ctx(), work);
    }
}
pub(crate) fn selection_menu(app: &mut SheetApp, ui: &mut egui::Ui) -> bool {
    if ui.button("Ask local AI").clicked() {
        let work = snapshot(app, 0);
        app.local_ai.open_selection(ui.ctx(), key(app), work);
        return true;
    }
    false
}
fn snapshot(app: &SheetApp, scope: usize) -> impl FnOnce(Cancel) -> Result<Source, String> + Send + 'static {
    let snapshot = app.session.active().cloned();
    let object = app.selected_chart;
    move |cancel| {
        let st = snapshot.ok_or("Open a workbook first.")?;
        if scope == 0
            && let Some(id) = object
        {
            let sh = st.wb.active().ok_or("No active sheet.")?;
            if sh.visibility != gridcraft_engine::model::Visibility::Visible {
                return Err("Hidden sheets are excluded.".into());
            }
            let mut source = Source::new(format!("{} / selected object text; no linked cells or pixels", sh.name));
            if let Some(shape) = sh.shapes.iter().find(|s| s.id == id) {
                if !matches!(shape.kind, gridcraft_engine::model::ShapeKind::Ink | gridcraft_engine::model::ShapeKind::Icon) {
                    source.push("Selected shape", &shape.text);
                }
            } else if let Some(chart) = sh.charts.iter().find(|s| s.id == id) {
                for (label, text) in [("Chart title", &chart.title), ("X axis", &chart.x_title), ("Y axis", &chart.y_title)] {
                    if let Some(text) = text {
                        source.push(label, text);
                    }
                }
            } else if let Some(image) = sh.images.iter().find(|s| s.id == id) {
                source.push("Selected image description (not pixels)", &image.alt);
            }
            return Ok(source);
        }
        extract(&st, scope, &cancel)
    }
}

fn extract(st: &DocState, scope: usize, cancel: &Cancel) -> Result<Source, String> {
    let wb = &st.wb;
    let sh = wb.active().ok_or("No active sheet.")?;
    if sh.visibility != gridcraft_engine::model::Visibility::Visible {
        return Err("Hidden sheets are excluded. Choose a visible sheet.".into());
    }
    let mut source = Source::new(format!(
        "{} / {} snapshot; hidden rows and columns excluded",
        sh.name,
        if scope == 1 { "visible sheet" } else { "selected cells" }
    ));
    let mut count = 0;
    // Sparse iteration also bounds a whole-column selection without walking a million blanks.
    let mut coordinates: std::collections::BTreeSet<_> = sh
        .cells
        .iter()
        .take_while(|_| !cancel.is_cancelled())
        .map(|(at, _)| at)
        .filter(|at| scope == 1 || st.selection.contains(*at))
        .take(20_001)
        .collect();
    coordinates
        .extend(sh.spill.keys().copied().take_while(|_| !cancel.is_cancelled()).filter(|at| scope == 1 || st.selection.contains(*at)).take(20_001));
    if coordinates.len() > 20_000 {
        source.partial = true;
    }
    for at in coordinates {
        if cancel.is_cancelled() {
            return Err("Preview cancelled.".into());
        }
        if scope != 1 && !st.selection.contains(at) {
            continue;
        }
        let (height, width) = (sh.row_height(at.row), sh.col_width(at.col));
        if !height.is_finite() || !width.is_finite() || height <= 0.0 || width <= 0.0 || sh.merge_at(at).is_some_and(|range| range.start != at) {
            continue;
        }
        count += 1;
        if count > 20_000 {
            source.partial = true;
            break;
        }
        let mut text = display::value_text(wb, sh, at);
        if scope == 2 && !display::formula_hidden(wb, sh, at) {
            if let Some(f) = sh.cell(at).and_then(|cell| cell.formula.as_ref()) {
                text.push_str(&format!("\nFormula: ={}", f.text));
            }
        }
        if !source.push(&format!("{}!{}", sh.name, at.a1()), &text) {
            break;
        }
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_menu_freezes_cells_before_focus_changes() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut session = gridcraft_engine::Session::new();
        session.execute("file.new", serde_json::json!({})).unwrap();
        session.execute("cell.set", serde_json::json!({"cell":"A1","value":"PUBLIC"})).unwrap();
        session.execute("cell.set", serde_json::json!({"cell":"B1","value":"SECRET"})).unwrap();
        let app = SheetApp::new(session, crate::Services::default());
        let mut h = Harness::builder().build_ui_state(
            |ui, app: &mut SheetApp| {
                if !app.local_ai.open {
                    selection_menu(app, ui);
                } else {
                    show(app, ui);
                }
            },
            app,
        );
        h.run_steps(3);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Ask local AI").click();
        h.run_steps(2);
        h.state_mut().session.active_mut().unwrap().selection = gridcraft_engine::Selection::range(gridcraft_engine::core::RangeRef::new(
            gridcraft_engine::core::CellRef::new(0, 1),
            gridcraft_engine::core::CellRef::new(0, 1),
        ));
        for _ in 0..100 {
            h.run_steps(1);
            if h.state().local_ai.preview().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let plan = h.state().local_ai.preview().unwrap();
        assert_eq!(plan.excerpts.len(), 1);
        assert_eq!(plan.excerpts[0].text, "PUBLIC");
    }

    #[test]
    fn local_ai_excludes_merge_covered_and_zero_size_cells() {
        let mut session = gridcraft_engine::Session::new();
        session.execute("file.new", serde_json::json!({})).unwrap();
        for (cell, value) in [("A1", "PUBLIC"), ("B1", "MERGED SECRET"), ("C1", "ZERO COLUMN SECRET"), ("A2", "ZERO ROW SECRET")] {
            session.execute("cell.set", serde_json::json!({"cell":cell,"value":value})).unwrap();
        }
        let st = session.active_mut().unwrap();
        let wb = std::sync::Arc::make_mut(&mut st.wb);
        let sh = std::sync::Arc::make_mut(&mut wb.sheets[0]);
        sh.merges.push(gridcraft_engine::core::RangeRef::new(gridcraft_engine::core::CellRef::new(0, 0), gridcraft_engine::core::CellRef::new(0, 1)));
        sh.cols.entry(2).or_default().size = Some(0.0);
        sh.rows.entry(1).or_default().size = Some(0.0);
        let source = extract(st, 1, &Cancel::default()).unwrap();
        assert_eq!(source.excerpts.len(), 1);
        assert_eq!(source.excerpts[0].text, "PUBLIC");
    }
    #[test]
    fn local_ai_real_grid_window_previews_selected_cell_without_editing() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut session = gridcraft_engine::Session::new();
        session.execute("file.new", serde_json::json!({})).unwrap();
        session.execute("cell.set", serde_json::json!({"cell":"A1","value":"The project deadline is 31 December 2026."})).unwrap();
        let before = session.active().unwrap().wb.clone();
        let mut app = SheetApp::new(session, crate::Services::default());
        app.local_ai.open = true;
        let mut h = Harness::builder().with_size(egui::vec2(1440.0, 1000.0)).wgpu().build_ui_state(
            |ui, app: &mut SheetApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
        h.run_steps(8);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Preview").click();
        for _ in 0..100 {
            h.run_steps(1);
            if h.state().local_ai.preview().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            h.state()
                .local_ai
                .preview()
                .expect("cell preview")
                .excerpts
                .iter()
                .any(|e| e.location.ends_with("!A1") && e.text.contains("31 December 2026"))
        );
        assert_eq!(h.state().session.active().unwrap().wb, before);
        if let Ok(path) = std::env::var("CRAFT_AI_APP_IMAGE") {
            h.render().unwrap().save(path).unwrap();
        }
    }
    #[test]
    fn local_ai_selection_excludes_neighbors_and_hidden_rows() {
        let mut session = gridcraft_engine::Session::new();
        session.execute("file.new", serde_json::json!({})).unwrap();
        for (cell, text) in [("A1", "PUBLIC"), ("A2", "SECRET"), ("B1", "NEIGHBOR")] {
            session.execute("cell.set", serde_json::json!({"cell":cell,"value":text})).unwrap();
        }
        let st = session.active_mut().unwrap();
        st.selection = gridcraft_engine::Selection::range(gridcraft_engine::core::RangeRef::new(
            gridcraft_engine::core::CellRef::new(0, 0),
            gridcraft_engine::core::CellRef::new(1, 0),
        ));
        std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(&mut st.wb).sheets[0]).rows.entry(1).or_default().hidden = true;
        let source = extract(st, 0, &Cancel::default()).unwrap();
        assert_eq!(source.excerpts.len(), 1);
        assert_eq!(source.excerpts[0].text, "PUBLIC");
    }

    #[test]
    fn local_ai_protected_hidden_formulas_remain_excluded_in_formula_scope() {
        let mut session = gridcraft_engine::Session::new();
        session.execute("file.new", serde_json::json!({})).unwrap();
        session.execute("cell.set", serde_json::json!({"cell":"A1","value":"=1+2"})).unwrap();
        let st = session.active_mut().unwrap();
        let wb = std::sync::Arc::make_mut(&mut st.wb);
        let at = gridcraft_engine::core::CellRef::new(0, 0);
        let style = wb.styles.derive(wb.sheets[0].style_id(at), |style| style.protection.hidden = true);
        let sh = std::sync::Arc::make_mut(&mut wb.sheets[0]);
        let mut cell = sh.cell(at).cloned().unwrap();
        cell.style = style;
        sh.set_cell(at, cell);
        sh.protection = Some(Default::default());
        let source = extract(st, 2, &Cancel::default()).unwrap();
        assert_eq!(source.excerpts.len(), 1);
        assert_eq!(source.excerpts[0].text, "3");
    }
}
