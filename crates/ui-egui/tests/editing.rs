use gridcraft_engine::{Session, core::CellRef};
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

fn app(services: Services) -> SheetApp {
    let mut session = Session::new();
    session.new_workbook();
    SheetApp::new(session, services)
}
fn shown(a: &SheetApp, row: u32, col: u32) -> String {
    let d = a.session.active().unwrap();
    gridcraft_engine::display::cell_text(&d.wb, d.wb.active().unwrap(), CellRef::new(row, col))
}

#[test]
fn external_source_formatting_and_destination_formatting_are_distinct_and_undoable() {
    let mut a = app(Services {
        paste_text: Some(Box::new(|| Some("42".into()))),
        paste_html: Some(Box::new(|| Some("<table><tr><td style='font-weight:bold;font-size:18pt;color:#123456'>42</td></tr></table>".into()))),
        ..Default::default()
    });
    a.run("selection.set", json!({"cell":"B2"})).unwrap();
    a.run("home.italic", json!({"on":true})).unwrap();
    let before = a.session.active().unwrap().wb.clone();
    a.run("edit.pasteSpecial", json!({"what":"all"})).unwrap();
    let d = a.session.active().unwrap();
    let c = d.wb.active().unwrap().cell(CellRef::new(1, 1)).unwrap();
    let f = &d.wb.styles.get(c.style).font;
    assert!(f.bold);
    assert_eq!(f.size, 18.0);
    assert_eq!(shown(&a, 1, 1), "42");
    a.run("edit.undo", json!({})).unwrap();
    assert_eq!(a.session.active().unwrap().wb, before);
    a.run("edit.pasteSpecial", json!({"what":"formulas"})).unwrap();
    let d = a.session.active().unwrap();
    let c = d.wb.active().unwrap().cell(CellRef::new(1, 1)).unwrap();
    let f = &d.wb.styles.get(c.style).font;
    assert!(f.italic);
    assert!(!f.bold);
    assert_eq!(shown(&a, 1, 1), "42");
}

#[test]
fn newer_external_copy_wins_over_internal_and_formulas_keep_destination_references() {
    use std::{cell::RefCell, rc::Rc};
    let clipboard = Rc::new(RefCell::new("old external".to_string()));
    let reader = clipboard.clone();
    let mut a = app(Services { paste_text: Some(Box::new(move || Some(reader.borrow().clone()))), ..Default::default() });
    a.run("cell.set", json!({"cell":"A1","input":"STALE"})).unwrap();
    a.run("edit.copy", json!({"range":"A1"})).unwrap();
    *clipboard.borrow_mut() = "=B1+1".into();
    a.run("cell.set", json!({"cell":"B1","input":"5"})).unwrap();
    a.run("selection.set", json!({"cell":"D1"})).unwrap();
    a.run("edit.pasteSpecial", json!({"what":"formulas"})).unwrap();
    assert_eq!(shown(&a, 0, 3), "6");
    let d = a.session.active().unwrap();
    assert_eq!(d.wb.active().unwrap().cell(CellRef::new(0, 3)).unwrap().formula.as_ref().unwrap().text, "B1+1");
}

#[test]
fn values_transpose_and_format_only_preserve_expected_content() {
    let mut a = app(Default::default());
    a.run("cell.set", json!({"cell":"A1","input":"=1+2"})).unwrap();
    a.run("cell.set", json!({"cell":"B1","input":"9"})).unwrap();
    a.run("home.bold", json!({"range":"A1:B1","on":true})).unwrap();
    a.run("edit.copy", json!({"range":"A1:B1"})).unwrap();
    a.run("selection.set", json!({"cell":"D1"})).unwrap();
    a.run("edit.pasteSpecial", json!({"what":"values","transpose":true})).unwrap();
    assert_eq!(shown(&a, 0, 3), "3");
    assert_eq!(shown(&a, 1, 3), "9");
    assert!(a.session.active().unwrap().wb.active().unwrap().cell(CellRef::new(0, 3)).unwrap().formula.is_none());
    a.run("edit.undo", json!({})).unwrap();
    assert_eq!(shown(&a, 0, 3), "");
    a.run("cell.set", json!({"cell":"D1","input":"KEEP"})).unwrap();
    a.run("selection.set", json!({"cell":"D1"})).unwrap();
    a.run("edit.pasteSpecial", json!({"what":"formats"})).unwrap();
    assert_eq!(shown(&a, 0, 3), "KEEP");
}

#[test]
fn cut_is_not_resurrected_by_paste_special() {
    let mut a = app(Services { paste_text: Some(Box::new(|| Some("MOVE".into()))), ..Default::default() });
    a.run("cell.set", json!({"cell":"A1","input":"MOVE"})).unwrap();
    let ctx = egui::Context::default();
    a.copy_to_clipboard(&ctx, "edit.cut");
    a.run("selection.set", json!({"cell":"B1"})).unwrap();
    a.paste_from_clipboard();
    a.run("selection.set", json!({"cell":"C1"})).unwrap();
    assert!(a.run("edit.pasteSpecial", json!({"what":"all"})).is_err());
    assert_eq!(shown(&a, 0, 2), "");
}

#[test]
fn right_click_paste_menu_performs_the_chosen_action() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut a = app(Default::default());
    a.run("cell.set", json!({"cell":"A1","input":"=6*7"})).unwrap();
    a.run("edit.copy", json!({"range":"A1"})).unwrap();
    a.run("selection.set", json!({"cell":"B1"})).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1440.0, 1000.0)).wgpu().build_ui_state(
        |ui, a: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            a.logic(&ctx);
            a.ui(ui);
        },
        a,
    );
    h.run_steps(8);
    h.state_mut().grid.context_menu = Some(egui::pos2(400.0, 350.0));
    h.run_steps(3);
    h.get_by_label_contains("Paste options").hover();
    h.run_steps(5);
    if let Ok(path) = std::env::var("CRAFT_AI_APP_IMAGE") {
        h.render().unwrap().save(path).unwrap();
    }
    h.get_by_label("Values only").click();
    h.run_steps(3);
    assert_eq!(shown(h.state(), 0, 1), "42");
    assert!(h.state().session.active().unwrap().wb.active().unwrap().cell(CellRef::new(0, 1)).unwrap().formula.is_none());
}

#[test]
fn typed_paste_filters_text_numbers_dates_and_formulas_without_overwriting_formats() {
    for (what, expected_col) in [("textOnly", 0), ("numbersOnly", 1), ("datesOnly", 2), ("formulasOnly", 3)] {
        let mut a = app(Default::default());
        for (cell, input) in [("A1", "word"), ("B1", "12"), ("C1", "45292"), ("D1", "=1+2")] {
            a.run("cell.set", json!({"cell":cell,"input":input})).unwrap();
        }
        a.run("home.numberFormat", json!({"range":"C1","code":"yyyy-mm-dd"})).unwrap();
        a.run("edit.copy", json!({"range":"A1:D1"})).unwrap();
        a.run("selection.set", json!({"cell":"A3"})).unwrap();
        a.run("edit.pasteSpecial", json!({"what":what})).unwrap();
        for col in 0..4 {
            assert_eq!(!shown(&a, 2, col).is_empty(), col == expected_col, "{what}, {col}");
        }
        a.run("edit.undo", json!({})).unwrap();
        assert_eq!(shown(&a, 2, expected_col), "");
    }
}

#[test]
fn mismatched_and_hostile_html_cannot_apply_styles_or_mutate_before_rejection() {
    let mut a = app(Services {
        paste_text: Some(Box::new(|| Some("SAFE".into()))),
        paste_html: Some(Box::new(|| {
            Some("<script>fetch('https://invalid.example')</script><table><td style='font-weight:bold;background-image:url(file:///secret)'>OTHER</td></table>".into())
        })),
        ..Default::default()
    });
    a.run("edit.pasteSpecial", json!({"what":"all"})).unwrap();
    let d = a.session.active().unwrap();
    let c = d.wb.active().unwrap().cell(CellRef::new(0, 0)).unwrap();
    assert!(!d.wb.styles.get(c.style).font.bold);
    assert_eq!(shown(&a, 0, 0), "SAFE");
}

#[test]
fn html_colspan_amplification_falls_back_to_safe_plain_text() {
    let mut a = app(Services {
        paste_text: Some(Box::new(|| Some("SAFE".into()))),
        paste_html: Some(Box::new(|| {
            Some(format!("<table>{}</table>", "<tr><td colspan='16384' style='font-weight:bold'>SAFE</td></tr>".repeat(8)))
        })),
        ..Default::default()
    });
    a.run("edit.pasteSpecial", json!({"what":"all"})).unwrap();
    assert_eq!(shown(&a, 0, 0), "SAFE");
    let d = a.session.active().unwrap();
    let c = d.wb.active().unwrap().cell(CellRef::new(0, 0)).unwrap();
    assert!(!d.wb.styles.get(c.style).font.bold);
}

#[test]
fn simple_excel_clipboard_classes_keep_formatting_and_inline_properties_win() {
    let mut a = app(Services {
        paste_text: Some(Box::new(|| Some("42".into()))),
        paste_html: Some(Box::new(|| {
            Some("<style>.xl65{font-weight:bold;font-size:18pt;color:#123456}td.xl65{font-style:italic}@import url(https://invalid.example);</style><table><td class='xl65' style='font-size:20pt'>42</td></table>".into())
        })),
        ..Default::default()
    });
    a.run("edit.pasteSpecial", json!({"what":"all"})).unwrap();
    let d = a.session.active().unwrap();
    let c = d.wb.active().unwrap().cell(CellRef::new(0, 0)).unwrap();
    let font = &d.wb.styles.get(c.style).font;
    assert!(font.bold && font.italic);
    assert_eq!(font.size, 20.0);
}

#[test]
fn same_words_copied_again_with_new_formatting_replace_the_previous_snapshot() {
    use std::{cell::RefCell, rc::Rc};
    let rich = Rc::new(RefCell::new("<table><td style='font-weight:bold'>42</td></table>".to_string()));
    let reader = rich.clone();
    let mut a = app(Services {
        paste_text: Some(Box::new(|| Some("42".into()))),
        paste_html: Some(Box::new(move || Some(reader.borrow().clone()))),
        ..Default::default()
    });
    a.run("edit.pasteSpecial", json!({"what":"all"})).unwrap();
    *rich.borrow_mut() = "<table><td style='font-style:italic'>42</td></table>".into();
    a.run("selection.set", json!({"cell":"B1"})).unwrap();
    a.run("edit.pasteSpecial", json!({"what":"all"})).unwrap();
    let d = a.session.active().unwrap();
    let c = d.wb.active().unwrap().cell(CellRef::new(0, 1)).unwrap();
    let font = &d.wb.styles.get(c.style).font;
    assert!(font.italic);
    assert!(!font.bold);
}
