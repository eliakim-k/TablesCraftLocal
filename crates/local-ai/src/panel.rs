use crate::{Answer, Plan, QUESTION_BYTES, SYSTEM_PROMPT, Source};
use zeroize::Zeroize;

// The component is shared by four hosts, so it owns its square, outlined toggle styling.
#[allow(clippy::disallowed_methods, reason = "shared checkbox applies the required square/contrast styling without a circular host-UI dependency")]
fn checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
    ui.scope(|ui| {
        let border = ui.visuals().widgets.noninteractive.fg_stroke.color;
        let fill = if ui.visuals().dark_mode { egui::Color32::from_gray(60) } else { egui::Color32::from_gray(240) };
        let widgets = &mut ui.visuals_mut().widgets;
        for visual in [&mut widgets.noninteractive, &mut widgets.inactive, &mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
            visual.corner_radius = egui::CornerRadius::same(3);
            if visual.bg_stroke.is_empty() {
                visual.bg_stroke = egui::Stroke::new(1.0, border);
            }
        }
        widgets.inactive.bg_fill = fill;
        ui.add(egui::Checkbox::new(checked, label))
    })
    .inner
}

// Paint the icon locally so it does not depend on a platform symbol font.
fn settings_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(106.0, 32.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Settings"));
    let visual = ui.style().interact(&response);
    ui.painter().rect(rect, 6, visual.bg_fill, visual.bg_stroke, egui::StrokeKind::Inside);
    let center = egui::pos2(rect.left() + 17.0, rect.center().y);
    let stroke = egui::Stroke::new(1.5, visual.text_color());
    ui.painter().circle_stroke(center, 5.5, stroke);
    ui.painter().circle_stroke(center, 2.0, stroke);
    for tooth in 0..8 {
        let angle = tooth as f32 * std::f32::consts::TAU / 8.0;
        let direction = egui::vec2(angle.cos(), angle.sin());
        ui.painter().line_segment([center + direction * 5.0, center + direction * 8.0], stroke);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Settings",
        egui::TextStyle::Button.resolve(ui.style()),
        visual.text_color(),
    );
    response.on_hover_text("Local model settings")
}

#[cfg(not(target_arch = "wasm32"))]
use crate::transport::{self, Cancel, Config};

#[cfg(not(target_arch = "wasm32"))]
enum Outcome {
    Models(Result<Vec<String>, String>),
    Source(Result<Source, String>),
    Answer(Result<Answer, String>),
}
#[cfg(not(target_arch = "wasm32"))]
struct Job {
    receiver: std::sync::mpsc::Receiver<Outcome>,
    cancel: Cancel,
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Ephemeral state. Deliberately excluded from each app's persisted settings/recovery.
pub struct Panel {
    pub open: bool,
    enabled: bool,
    local_confirmed: bool,
    port: u16,
    token: String,
    models: Vec<String>,
    model: String,
    pub scope: usize,
    question: String,
    key: Option<String>,
    plan: Option<Plan>,
    answer: Option<Answer>,
    answer_model: String,
    notice: Option<String>,
    selection_source: Option<Source>,
    capture_selection: bool,
    tab: u8,
    settings_open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    job: Option<Job>,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            open: false,
            enabled: false,
            local_confirmed: false,
            port: 1234,
            token: String::new(),
            models: Vec::new(),
            model: String::new(),
            scope: 0,
            question: "Summarize the main points.".into(),
            key: None,
            plan: None,
            answer: None,
            answer_model: String::new(),
            notice: None,
            selection_source: None,
            capture_selection: false,
            tab: 0,
            settings_open: false,
            #[cfg(not(target_arch = "wasm32"))]
            job: None,
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

impl Panel {
    pub fn preview(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
    fn clear_document(&mut self) {
        self.plan = None;
        self.answer = None;
        self.selection_source = None;
        self.capture_selection = false;
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(job) = self.job.take() {
                job.cancel.cancel();
            }
        }
    }
    pub fn busy(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.job.is_some()
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn config(&self) -> Config {
        Config { port: self.port, model: self.model.clone(), token: self.token.clone() }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn(&mut self, ctx: &egui::Context, work: impl FnOnce(Cancel) -> Outcome + Send + 'static) {
        if self.busy() {
            return;
        }
        let (send, receiver) = std::sync::mpsc::channel();
        let cancel = Cancel::default();
        self.job = Some(Job { receiver, cancel: cancel.clone() });
        let ctx = ctx.clone();
        match std::thread::Builder::new().name("craft-local-ai".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(cancel)));
            // A panic carries no document or model content into logs/status.
            if let Ok(result) = result {
                let _ = send.send(result);
            }
            ctx.request_repaint();
        }) {
            Ok(_) => {}
            Err(_) => {
                self.job = None;
                self.notice = Some("Could not start the local AI worker.".into());
            }
        }
    }

    /// Hosts call this only after the user presses Preview. PDF extraction uses its own
    /// frozen renderer snapshot on this worker; cancellation is checked between pages.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn prepare(&mut self, ctx: &egui::Context, work: impl FnOnce(Cancel) -> Result<Source, String> + Send + 'static) {
        self.plan = None;
        self.answer = None;
        self.notice = None;
        self.spawn(ctx, move |cancel| Outcome::Source(work(cancel)));
    }

    /// Capture a context-menu selection locally. Only the separate Send action uses the model.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_selection(&mut self, ctx: &egui::Context, key: Option<String>, work: impl FnOnce(Cancel) -> Result<Source, String> + Send + 'static) {
        self.clear_document();
        self.key = key;
        self.open = true;
        self.scope = 0;
        self.tab = 0;
        self.capture_selection = true;
        self.prepare(ctx, work);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll(&mut self) {
        let result = self.job.as_ref().and_then(|job| match job.receiver.try_recv() {
            Ok(out) => Some(Ok(out)),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err("The local AI worker stopped unexpectedly.".to_string())),
        });
        if let Some(result) = result {
            let cancelled = self.job.as_ref().is_some_and(|job| job.cancel.is_cancelled());
            self.job = None;
            if cancelled {
                return;
            }
            match result {
                Ok(Outcome::Models(Ok(models))) => {
                    if !models.contains(&self.model) {
                        self.model = models.first().cloned().unwrap_or_default();
                    }
                    self.models = models;
                    if !self.models.is_empty() {
                        self.settings_open = false;
                    }
                    self.notice = self.models.is_empty().then(|| "No models found. Load a chat model in LM Studio.".into());
                }
                Ok(Outcome::Source(Ok(source))) => {
                    if self.capture_selection {
                        self.selection_source = Some(source.clone());
                    }
                    match Plan::new(source, &self.question) {
                        Ok(plan) => {
                            self.plan = Some(plan);
                            self.tab = 0;
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
                Ok(Outcome::Answer(Ok(answer))) => {
                    self.answer = Some(answer);
                    self.tab = 1;
                }
                Ok(Outcome::Models(Err(e)) | Outcome::Source(Err(e)) | Outcome::Answer(Err(e))) | Err(e) => self.notice = Some(e),
            }
        }
    }

    /// Place the launcher in the host's existing chrome without adding a new panel.
    pub fn launcher(&mut self, ui: &mut egui::Ui, label: &str) {
        if ui.small_button(label).clicked() {
            self.open = !self.open;
            if !self.open {
                self.clear_document();
            }
        }
    }

    /// Persistent actions above a scrollable preview/answer; nothing sends automatically.
    pub fn show(&mut self, ui: &mut egui::Ui, key: Option<String>, scopes: &[&str]) -> Option<usize> {
        if self.key != key {
            self.clear_document();
            self.key = key;
            self.notice = None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.poll();
        let ctx = ui.ctx().clone();
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut prepare = None;
        egui::Window::new("Ask local AI")
            .id(egui::Id::new("craft-local-ai-window"))
            .open(&mut open)
            .default_pos(egui::pos2(70.0, 60.0))
            .default_size([620.0, 600.0])
            .frame(egui::Frame::window(ui.style()).corner_radius(12).inner_margin(16))
            .min_width(400.0)
            .show(&ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 9.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
                let widgets = &mut ui.visuals_mut().widgets;
                for visual in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
                    visual.corner_radius = egui::CornerRadius::same(6);
                }
                let connected = self.enabled && self.local_confirmed && self.models.contains(&self.model);
                let mut settings_open = self.settings_open;
                ui.horizontal(|ui| {
                    let color = if connected { ui.visuals().selection.stroke.color } else { ui.visuals().weak_text_color() };
                    ui.colored_label(color, if connected { format!("Local · {}", self.model) } else { "Set up a local model in Settings".into() });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let response = settings_button(ui);
                        if response.clicked() { settings_open = !settings_open; }
                        egui::Popup::from_response(&response)
                            .open_bool(&mut settings_open)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .width(360.0)
                            .show(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                                ui.strong("Local model");
                        let before = self.enabled;
                        checkbox(ui, &mut self.enabled, "Enable local AI for this session");
                        if before && !self.enabled {
                            self.clear_document(); self.models.clear(); self.model.clear(); self.token.zeroize(); self.local_confirmed = false;
                        }
                        let busy = self.busy();
                        ui.add_enabled_ui(self.enabled && !busy, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("LM Studio · 127.0.0.1:");
                                if ui.add(egui::DragValue::new(&mut self.port).range(1..=65535).speed(1)).changed() { self.models.clear(); self.model.clear(); }
                            });
                            checkbox(ui, &mut self.local_confirmed, "On-device model; remote routes disabled");
                            ui.horizontal(|ui| {
                                #[cfg(not(target_arch = "wasm32"))]
                                if ui.add_enabled(self.local_confirmed, egui::Button::new("List local models")).clicked() {
                                    self.notice = None;
                                    let config = self.config();
                                    self.spawn(&ctx, move |cancel| Outcome::Models(transport::models(config, cancel)));
                                }
                                egui::ComboBox::from_id_salt("craft-local-ai-model").selected_text(if self.model.is_empty() { "Choose model" } else { &self.model }).show_ui(ui, |ui| {
                                    for model in &self.models { ui.selectable_value(&mut self.model, model.clone(), model); }
                                });
                            });
                            ui.collapsing("API token (optional)", |ui| {
                                if ui.add(egui::TextEdit::singleline(&mut self.token).password(true).char_limit(4096).hint_text("Stored only for this session")).changed() { self.models.clear(); self.model.clear(); }
                            });
                        });

                            });
                    });
                });
                self.settings_open = settings_open;
                ui.separator();
                let busy = self.busy();
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Content");
                        egui::ComboBox::from_id_salt("craft-local-ai-scope").selected_text(scopes.get(self.scope).copied().unwrap_or("Selection")).show_ui(ui, |ui| {
                            for (i, name) in scopes.iter().enumerate() {
                                if ui.selectable_value(&mut self.scope, i, *name).changed() {
                                    self.plan = None; self.answer = None; self.notice = None;
                                }
                            }
                        });
                    });
                    ui.label("Ask a question");
                    if ui.add(egui::TextEdit::multiline(&mut self.question).desired_rows(2).desired_width(f32::INFINITY).char_limit(QUESTION_BYTES).hint_text("Summarize, explain or compare…")).changed() {
                        self.plan = None; self.answer = None; self.notice = None;
                    }
                });
                let question_valid = !self.question.trim().is_empty() && self.question.trim().len() <= QUESTION_BYTES;
                let connected = self.enabled && self.local_confirmed && self.models.contains(&self.model);
                let ready = connected && self.plan.is_some() && !self.busy();
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.key.is_some() && question_valid && !self.busy(), egui::Button::new("Preview").min_size(egui::vec2(90.0, 30.0))).clicked() {
                        self.answer = None; self.notice = None; self.tab = 0;
                        if self.scope == 0 && let Some(source) = self.selection_source.clone() {
                            match Plan::new(source, &self.question) { Ok(plan) => self.plan = Some(plan), Err(e) => self.notice = Some(e) }
                        } else {
                            self.selection_source = None; self.capture_selection = false; prepare = Some(self.scope);
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if ui.add_enabled(ready, egui::Button::new("Send to local AI").fill(ui.visuals().selection.bg_fill).min_size(egui::vec2(145.0, 30.0))).clicked()
                        && let Some(plan) = self.plan.clone()
                    {
                        let config = self.config(); self.answer_model = self.model.clone(); self.answer = None; self.notice = None; self.tab = 1;
                        self.spawn(&ctx, move |cancel| Outcome::Answer(transport::ask(config, plan, cancel)));
                    }
                    if self.busy() {
                        ui.spinner();
                        if ui.button("Cancel").clicked() { self.clear_document(); self.notice = Some("Cancelled.".into()); }
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    } else if ui.small_button("Clear").clicked() { self.clear_document(); self.question.clear(); self.notice = None; }
                });
                let status = if self.busy() { "Working locally…" } else if self.plan.is_none() { "Preview the content before sending." } else if !self.enabled || !self.local_confirmed { "Connect to your model in Settings." } else if !connected { "Choose a model in Settings." } else { "Ready to send. Only this preview will be sent." };
                ui.small(status);
                if let Some(notice) = &self.notice { ui.colored_label(ui.visuals().warn_fg_color, notice); }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, 0, "Sources");
                    ui.selectable_value(&mut self.tab, 1, "Answer");
                    if self.tab == 1 && let Some(answer) = &self.answer && ui.small_button("Copy answer").clicked() { ctx.copy_text(answer.text.clone()); }
                });
                egui::ScrollArea::vertical().id_salt("craft-ai-content").max_height(ui.available_height().clamp(100.0, 320.0)).show(ui, |ui| {
                    if self.tab == 0 {
                        if let Some(plan) = &self.plan {
                            ui.strong("Content to send");
                            ui.small(&plan.description);
                            if self.capture_selection { ui.small("Selection captured from the right-click menu."); }
                            if plan.partial { ui.small("Partial selection. Review the excerpts below."); }
                            for excerpt in &plan.excerpts {
                                ui.add_space(6.0);
                                ui.small(format!("[{}] {}", excerpt.id, excerpt.location));
                                ui.add(egui::Label::new(&excerpt.text).selectable(true).wrap());
                            }
                            ui.collapsing("Request details", |ui| {
                                let mut text = plan.user_message.as_str();
                                ui.add(egui::TextEdit::multiline(&mut text).desired_rows(8).desired_width(f32::INFINITY));
                                ui.collapsing("System instruction", |ui| { ui.label(SYSTEM_PROMPT); });
                            });
                        } else { ui.label("Your selected content will appear here."); }
                    } else if let Some(answer) = &self.answer {
                        ui.small(format!("{} · Verify important claims against the sources.", self.answer_model));
                        if let Some(notice) = &answer.citation_notice { ui.colored_label(ui.visuals().warn_fg_color, notice); }
                        if answer.truncated { ui.small("Output limit reached. Try a narrower question."); }
                        answer.formatted.show(ui);
                    } else { ui.label(if self.busy() { "Waiting for the local model…" } else { "Your answer will appear here after you send." }); }
                    ui.add_space(12.0);
                    ui.collapsing("Privacy & help", |ui| {
                        ui.small("Text only; images need readable text or OCR. No cloud fallback or document edits. Tokens and conversations stay in memory. LM Studio has its own logging and routing settings.");
                    });
                });
            });
        if !open {
            self.clear_document();
            self.question.clear();
            self.settings_open = false;
        }
        self.open = open;
        prepare
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    fn preview() -> Panel {
        let mut source = Source::new("Synthetic document / page 1");
        source.push("Page 1", "The project deadline is 31 December 2026. The approved budget is 42,000 euros.");
        let mut panel = Panel::default();
        panel.open = true;
        panel.key = Some("doc:1".into());
        panel.question = "When is the deadline?".into();
        panel.plan = Some(Plan::new(source, &panel.question).unwrap());
        panel
    }

    #[test]
    fn closed_panel_preserves_the_editors_available_space() {
        let mut h = Harness::builder().build_ui_state(
            |ui, panel: &mut Panel| {
                let before = ui.available_rect_before_wrap();
                panel.show(ui, Some("doc:1".into()), &["Selected text"]);
                assert_eq!(ui.available_rect_before_wrap(), before);
            },
            Panel::default(),
        );
        h.run_steps(3);
    }

    #[test]
    fn defaults_make_no_network_requests_and_keep_the_connection_off() {
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).build_ui_state(
            |ui, panel: &mut Panel| {
                panel.launcher(ui, "Ask local AI");
                panel.show(ui, Some("doc:1".into()), &["Selected text", "Current page"]);
            },
            Panel::default(),
        );
        h.run_steps(3);
        assert!(!h.state().enabled);
        assert!(!h.state().busy());
        assert!(h.state().models.is_empty());
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Ask local AI").click();
        h.run_steps(3);
        assert!(h.state().open);
        assert!(!h.state().busy());
    }

    #[test]
    fn settings_controls_are_hidden_until_the_gear_button_is_opened() {
        let mut panel = preview();
        panel.settings_open = false;
        let mut h = Harness::builder().with_size(egui::vec2(900.0, 760.0)).wgpu().build_ui_state(
            |ui, p: &mut Panel| {
                p.show(ui, Some("doc:1".into()), &["Selected text"]);
            },
            panel,
        );
        h.run_steps(4);
        assert!(h.query_by_label("Enable local AI for this session").is_none());
        let send = h.get_by_role_and_label(egui::accesskit::Role::Button, "Send to local AI").rect();
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Settings").click();
        h.run_steps(3);
        h.get_by_label("Enable local AI for this session");
        if let Ok(path) = std::env::var("CRAFT_AI_TEST_IMAGE") {
            h.render().unwrap().save(path.replace(".png", "-settings.png")).unwrap();
        }
        let after = h.get_by_role_and_label(egui::accesskit::Role::Button, "Send to local AI").rect();
        assert!((after.min.y - send.min.y).abs() < 1.0, "Settings must not displace the send controls");
        assert!(after.max.y < 760.0);
        h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() });
        h.run_steps(3);
        assert!(h.query_by_label("Enable local AI for this session").is_none());
        assert!(h.state().preview().is_some());
        assert!(!h.state().busy());
    }

    #[test]
    fn closing_the_panel_discards_document_text_and_answer() {
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).build_ui_state(
            |ui, panel: &mut Panel| {
                panel.launcher(ui, "Ask local AI");
                panel.show(ui, Some("doc:1".into()), &["Selected text", "Current page"]);
            },
            preview(),
        );
        h.run_steps(3);
        assert!(h.state().plan.is_some());
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Ask local AI").click();
        h.run_steps(3);
        assert!(!h.state().open);
        assert!(h.state().plan.is_none());
        assert!(h.state().answer.is_none());
    }

    #[test]
    fn changed_document_discards_preview_and_late_worker_completion() {
        let mut panel = preview();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (send, receiver) = std::sync::mpsc::channel();
            panel.job = Some(Job { receiver, cancel: Cancel::default() });
            send.send(Outcome::Source(Ok(Source::new("previous document")))).unwrap();
        }
        let mut h = Harness::builder().build_ui_state(
            |ui, panel: &mut Panel| {
                panel.show(ui, Some("other-doc:2".into()), &["Selected text"]);
            },
            panel,
        );
        h.run_steps(3);
        assert!(h.state().plan.is_none());
        assert!(h.state().answer.is_none());
        assert!(!h.state().busy());
    }

    #[test]
    fn selection_snapshot_survives_question_edits_but_not_document_edits() {
        let ctx = egui::Context::default();
        let mut panel = preview();
        panel.open_selection(&ctx, Some("doc:1".into()), |_| {
            let mut source = Source::new("Captured selection");
            source.push("Selection", "ONLY THE SELECTED TEXT");
            Ok(source)
        });
        for _ in 0..100 {
            panel.poll();
            if !panel.busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(panel.preview().unwrap().excerpts[0].text.contains("ONLY THE SELECTED TEXT"));
        assert!(!panel.enabled);
        panel.question = "Explain this".into();
        panel.plan = None;
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).build_ui_state(
            |ui, p: &mut Panel| {
                p.show(ui, Some("doc:1".into()), &["Selected text"]);
            },
            panel,
        );
        h.run_steps(3);
        assert!(h.get_by_role_and_label(egui::accesskit::Role::Button, "Send to local AI").accesskit_node().is_disabled());
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Preview").click();
        h.run_steps(3);
        assert!(h.state().preview().unwrap().user_message.contains("ONLY THE SELECTED TEXT"));
        assert!(h.state().preview().unwrap().user_message.contains("Explain this"));
        h.state_mut().clear_document();
        assert!(h.state().selection_source.is_none());
    }

    #[test]
    fn visible_send_button_sends_only_preview_and_displays_answer() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration, Instant};
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                if let Ok((socket, _)) = listener.accept() {
                    break socket;
                }
                assert!(Instant::now() < deadline, "Send never reached the local server");
                std::thread::sleep(Duration::from_millis(5));
            };
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let n = socket.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&request[..end]);
                    let length = header
                        .lines()
                        .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").and_then(|s| s.trim().parse::<usize>().ok()))
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            tx.send(String::from_utf8(request).unwrap()).unwrap();
            let body = r#"{"choices":[{"message":{"content":"The deadline is 31 December 2026. [S1]"},"finish_reason":"stop"}]}"#;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let mut panel = preview();
        panel.enabled = true;
        panel.local_confirmed = true;
        panel.models = vec!["synthetic-model".into()];
        panel.model = "synthetic-model".into();
        panel.port = port;
        panel.settings_open = false;
        let expected = panel.preview().unwrap().user_message.clone();
        let mut h = Harness::builder().with_size(egui::vec2(1000., 800.)).build_ui_state(
            |ui, p: &mut Panel| {
                p.show(ui, Some("doc:1".into()), &["Selected text"]);
            },
            panel,
        );
        h.run_steps(3);
        assert!(rx.try_recv().is_err(), "Preview must not send anything");
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Send to local AI").click();
        h.run_steps(2);
        assert_eq!(h.state().tab, 1);
        for _ in 0..400 {
            h.run_steps(1);
            if h.state().answer.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(h.state().answer.is_some(), "No visible answer: notice={:?}, busy={}", h.state().notice(), h.state().busy());
        assert_eq!(h.state().answer.as_ref().expect("visible answer").text, "The deadline is 31 December 2026. [S1]");
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Copy answer");
        let request = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(request.starts_with("POST /v1/chat/completions "));
        let body: serde_json::Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["messages"][1]["content"], expected);
        assert!(body.get("tools").is_none());
        server.join().unwrap();
    }

    #[test]
    fn render_local_ai_preview() {
        for dark in [true, false] {
            let mut panel = preview();
            panel.enabled = true;
            panel.local_confirmed = true;
            panel.models = vec!["gpt-oss-20b".into()];
            panel.model = "gpt-oss-20b".into();
            panel.settings_open = false;
            let mut h = Harness::builder().with_size(egui::vec2(1000.0, 800.0)).wgpu().build_ui_state(
                move |ui, p: &mut Panel| {
                    ui.ctx().set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                    p.show(ui, Some("doc:1".into()), &["Selected text", "Current page"]);
                },
                panel,
            );
            if let Ok(path) = std::env::var("CRAFT_AI_TEST_BOLD_FONT") {
                let mut fonts = egui::FontDefinitions::default();
                let data = std::fs::read(path).expect("fixture bold font");
                fonts.font_data.insert("fixture-bold".into(), std::sync::Arc::new(egui::FontData::from_owned(data)));
                let mut family = vec!["fixture-bold".into()];
                family.extend(fonts.families[&egui::FontFamily::Proportional].clone());
                fonts.families.insert(egui::FontFamily::Name("fixture-bold".into()), family);
                h.ctx.set_fonts(fonts);
                h.ctx.all_styles_mut(|style| {
                    style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::new(20.0, egui::FontFamily::Name("fixture-bold".into())));
                });
            }
            h.run_steps(4);
            if let Ok(path) = std::env::var("CRAFT_AI_TEST_IMAGE") {
                h.render().unwrap().save(path.replace(".png", if dark { "-dark.png" } else { "-light.png" })).unwrap();
                let answer=Some(crate::parse_answer(serde_json::json!({"choices":[{"message":{"content":"# Project summary ✅\n\nThe **deadline** is *31 December 2026*. [S1]\n\n- <u>Owner</u>: studio team\n- Review the `budget` before approval 😊\n\n| Action | Status |\n|---|---|\n| Final review | Pending |"},"finish_reason":"stop"}]}),h.state().plan.as_ref().unwrap()).unwrap());
                h.state_mut().answer = answer;
                h.state_mut().answer_model = "gpt-oss-20b".into();
                h.state_mut().tab = 1;
                h.run_steps(3);
                h.render().unwrap().save(path.replace(".png", if dark { "-answer-dark.png" } else { "-answer-light.png" })).unwrap();
            }
            assert!(!h.state().busy());
        }
    }
}
