use crate::speech::Speech;
use adw::{glib, gtk, prelude::*};
use learn_swedish::adventure::{Book, Scene};
use learn_swedish::lesson::Phrase;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Reader {
    book: RefCell<Book>,
    speech: Rc<Speech>,
    title: gtk::Label,
    illustration: gtk::DrawingArea,
    scene: Rc<Cell<Scene>>,
    transcript: gtk::Label,
    choices: [gtk::Button; 4],
    entry: gtk::Entry,
    submit: gtk::Button,
    skip: gtk::Button,
    next: gtk::Button,
    status: gtk::Label,
    english: gtk::CheckButton,
    automatic: gtk::CheckButton,
    pause_sentences: gtk::CheckButton,
    replay: gtk::Button,
    speaking: Cell<bool>,
}

impl Reader {
    fn phrases(&self) -> Vec<Phrase> {
        let book = self.book.borrow();
        if let Some(selected) = book.selected {
            let choice = book.page().choices[selected];
            vec![choice.line.phrase(), choice.reply.phrase()]
        } else {
            book.page().phrases()
        }
    }

    fn controls(&self) {
        let book = self.book.borrow();
        let answering = !self.speaking.get() && book.selected.is_none() && !book.finished;
        self.entry.set_sensitive(answering);
        self.submit.set_sensitive(answering);
        for choice in &self.choices {
            choice.set_sensitive(answering);
        }
        self.next
            .set_sensitive(!self.speaking.get() && book.selected.is_some());
        self.skip.set_sensitive(!book.finished);
        self.replay.set_sensitive(!book.finished);
    }

    fn skip(self: &Rc<Self>) {
        if self.book.borrow_mut().skip() {
            self.show();
        }
    }

    fn repeat_sentence(self: &Rc<Self>) -> bool {
        if self.speech.repeat_sentence() {
            return true;
        }
        if self.speech.has_last_sentence() {
            self.speaking.set(true);
            self.controls();
            let weak = Rc::downgrade(self);
            return self.speech.repeat_last_sentence(move |_| {
                if let Some(reader) = weak.upgrade() {
                    reader.speaking.set(false);
                    reader.controls();
                    if reader.entry.is_sensitive() {
                        reader.entry.grab_focus();
                    }
                }
            });
        }
        false
    }

    fn speak(self: &Rc<Self>) {
        self.speech.stop();
        self.speaking.set(true);
        self.controls();
        let weak = Rc::downgrade(self);
        self.speech
            .speak_then(self.phrases(), &self.status, &self.transcript, move |_| {
                if let Some(reader) = weak.upgrade() {
                    reader.speaking.set(false);
                    reader.controls();
                    if reader.entry.is_sensitive() {
                        reader.entry.grab_focus();
                    }
                }
            });
    }

    fn show(self: &Rc<Self>) {
        self.speech.stop();
        self.speech.clear_last_sentence();
        self.entry.set_text("");
        let book = self.book.borrow();
        self.scene.set(book.page().scene);
        self.illustration.queue_draw();
        if book.finished {
            self.title.set_label("Hemma igen · Slut");
            self.speech.show_transcript(
                &[learn_swedish::adventure::Line {
                    actor: learn_swedish::adventure::Actor::Wife,
                    swedish: "Tack själv. Nästa gång får du välja ett nytt äventyr.",
                    english: "Thank you too. Next time you can choose a new adventure.",
                }
                .phrase()],
                &self.transcript,
            );
            self.status
                .set_label("You are home again. Restart the book to try another outing.");
        } else {
            self.title.set_label(book.page().title);
            for (index, (button, choice)) in
                self.choices.iter().zip(&book.page().choices).enumerate()
            {
                let text = if self.english.is_active() {
                    format!(
                        "{}. {}\n{}",
                        index + 1,
                        choice.line.swedish,
                        choice.line.english
                    )
                } else {
                    format!("{}. {}", index + 1, choice.line.swedish)
                };
                button
                    .child()
                    .unwrap()
                    .downcast::<gtk::Label>()
                    .unwrap()
                    .set_label(&text);
            }
            self.next.set_label(
                if book
                    .selected
                    .is_some_and(|i| book.page().choices[i].next.is_none())
                {
                    "Försök igen · Try again"
                } else {
                    "Fortsätt · Continue"
                },
            );
        }
        let finished = book.finished;
        drop(book);
        self.controls();
        if !finished {
            self.speech
                .show_transcript(&self.phrases(), &self.transcript);
            if self.automatic.is_active() {
                self.speak();
            } else {
                self.status.set_label(if self.book.borrow().selected.is_some() {
                    "Read the reply, then use Continue or Try again. Replay reads the exchange aloud."
                } else {
                    "Choose 1–4 or type a full Swedish response. Replay reads this page aloud."
                });
            }
        }
    }

    fn answer(self: &Rc<Self>, input: &str) {
        if self.speaking.get() {
            return;
        }
        let result = self.book.borrow_mut().answer(input);
        match result {
            Ok(_) => self.show(),
            Err(error) => self.status.set_label(error),
        }
    }
}

pub fn open(app: &adw::Application, launcher: &adw::ApplicationWindow, speech: Rc<Speech>) {
    let (window, reader) = build(app, speech);
    window.connect_close_request({
        let launcher = launcher.clone();
        move |_| {
            launcher.present();
            glib::Propagation::Proceed
        }
    });
    launcher.hide();
    window.present();
    reader.show();
}

fn build(app: &adw::Application, speech: Rc<Speech>) -> (adw::ApplicationWindow, Rc<Reader>) {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "En lagom äventyrlig lördag",
        "A Swedish choose-your-own-adventure",
    )));
    let back = gtk::Button::with_label("Meny · Menu");
    back.set_widget_name("activity-menu");
    header.pack_start(&back);
    content.append(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_start(14);
    body.set_margin_end(14);
    body.set_margin_top(8);
    body.set_margin_bottom(10);
    let title = gtk::Label::new(None);
    title.add_css_class("title-3");
    title.set_wrap(true);
    body.append(&title);
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    columns.set_homogeneous(true);
    columns.set_vexpand(true);
    let left = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let scene = Rc::new(Cell::new(Scene::Home));
    let illustration = gtk::DrawingArea::builder().content_height(135).build();
    illustration.set_tooltip_text(Some("Original illustration of the current story location"));
    illustration.set_draw_func({
        let scene = scene.clone();
        move |_, context, width, height| draw_scene(context, width, height, scene.get())
    });
    left.append(&illustration);
    let transcript = gtk::Label::new(None);
    transcript.set_wrap(true);
    transcript.set_xalign(0.0);
    transcript.set_valign(gtk::Align::Start);
    transcript.set_widget_name("adventure-transcript");
    transcript.set_margin_start(8);
    transcript.set_margin_end(8);
    transcript.set_margin_top(8);
    transcript.set_margin_bottom(8);
    let text_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&transcript)
        .build();
    left.append(&text_scroll);
    columns.append(&left);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let choice_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let choices = std::array::from_fn(|_| {
        let label = gtk::Label::new(None);
        label.set_wrap(true);
        label.set_xalign(0.0);
        let button = gtk::Button::builder().child(&label).build();
        choice_box.append(&button);
        button
    });
    let choice_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&choice_box)
        .build();
    right.append(&choice_scroll);
    let entry = gtk::Entry::builder()
        .placeholder_text("1–4 or the full Swedish response")
        .build();
    right.append(&entry);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let submit = gtk::Button::with_label("Svara · Answer");
    let skip = gtk::Button::with_label("Skip");
    skip.set_tooltip_text(Some("Skip the current question without answering (Ctrl+→)"));
    let next = gtk::Button::with_label("Fortsätt · Continue");
    next.add_css_class("suggested-action");
    actions.append(&submit);
    actions.append(&skip);
    actions.append(&next);
    right.append(&actions);
    columns.append(&right);
    body.append(&columns);
    let options = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let english = gtk::CheckButton::with_label("Show English hints");
    let automatic = gtk::CheckButton::with_label("Read aloud");
    automatic.set_active(true);
    let pause_sentences = gtk::CheckButton::with_label("Pause after each sentence");
    pause_sentences.set_active(true);
    let replay = gtk::Button::with_label("Replay");
    let restart = gtk::Button::with_label("Restart book");
    options.append(&english);
    options.append(&automatic);
    options.append(&pause_sentences);
    options.append(&replay);
    options.append(&restart);
    body.append(&options);
    let status = gtk::Label::new(None);
    status.set_wrap(true);
    status.set_xalign(0.0);
    status.add_css_class("dim-label");
    status.set_lines(3);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.set_selectable(true);
    status.connect_label_notify(|label| label.set_tooltip_text(Some(&label.text())));
    body.append(&status);
    let outer = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&body)
        .vexpand(true)
        .build();
    content.append(&outer);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Swedish adventure")
        .default_width(900)
        .default_height(600)
        .content(&content)
        .build();
    let reader = Rc::new(Reader {
        book: RefCell::new(Book::default()),
        speech,
        title,
        illustration,
        scene,
        transcript,
        choices,
        entry,
        submit,
        skip,
        next,
        status,
        english,
        automatic,
        pause_sentences,
        replay,
        speaking: Cell::new(false),
    });
    let playback_keys = gtk::EventControllerKey::new();
    playback_keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    playback_keys.connect_key_pressed(glib::clone!(
        #[weak]
        reader,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, modifiers| {
            let shortcut_modifiers = modifiers
                & (gtk::gdk::ModifierType::CONTROL_MASK
                    | gtk::gdk::ModifierType::ALT_MASK
                    | gtk::gdk::ModifierType::SUPER_MASK);
            let editing_text = reader.entry.is_sensitive() && !reader.entry.text().is_empty();
            if key == gtk::gdk::Key::space && shortcut_modifiers.is_empty() {
                if reader.speech.toggle_pause() {
                    return glib::Propagation::Stop;
                }
            } else if key == gtk::gdk::Key::Left && shortcut_modifiers.is_empty() && !editing_text {
                if reader.repeat_sentence() {
                    return glib::Propagation::Stop;
                }
            } else if key == gtk::gdk::Key::Right
                && shortcut_modifiers == gtk::gdk::ModifierType::CONTROL_MASK
                && !editing_text
                && !reader.book.borrow().finished
            {
                reader.skip();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    window.add_controller(playback_keys);
    back.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    for (index, choice) in reader.choices.iter().enumerate() {
        choice.connect_clicked(glib::clone!(
            #[weak]
            reader,
            move |_| reader.answer(&(index + 1).to_string())
        ));
    }
    reader.entry.connect_activate(glib::clone!(
        #[weak]
        reader,
        move |entry| reader.answer(&entry.text())
    ));
    reader.submit.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| reader.answer(&reader.entry.text())
    ));
    reader.skip.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| reader.skip()
    ));
    reader.pause_sentences.connect_toggled(glib::clone!(
        #[weak]
        reader,
        move |toggle| reader.speech.set_auto_pause(toggle.is_active())
    ));
    reader.next.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| {
            reader.book.borrow_mut().continue_story();
            reader.show();
        }
    ));
    reader.replay.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| reader.speak()
    ));
    restart.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| {
            reader.book.replace(Book::default());
            reader.show();
        }
    ));
    reader.automatic.connect_toggled(glib::clone!(
        #[weak]
        reader,
        move |toggle| {
            if toggle.is_active() && !reader.book.borrow().finished {
                reader.speak();
            } else {
                reader.speech.stop();
                reader
                    .status
                    .set_label("Read aloud off · Replay is still available.");
            }
        }
    ));
    reader.english.connect_toggled(glib::clone!(
        #[weak]
        reader,
        move |toggle| {
            reader.speech.set_show_english(toggle.is_active());
            let book = reader.book.borrow();
            for (index, (button, choice)) in
                reader.choices.iter().zip(&book.page().choices).enumerate()
            {
                let text = if toggle.is_active() {
                    format!(
                        "{}. {}\n{}",
                        index + 1,
                        choice.line.swedish,
                        choice.line.english
                    )
                } else {
                    format!("{}. {}", index + 1, choice.line.swedish)
                };
                button
                    .child()
                    .unwrap()
                    .downcast::<gtk::Label>()
                    .unwrap()
                    .set_label(&text);
            }
        }
    ));
    window.connect_close_request({
        let reader = reader.clone();
        move |_| {
            reader.speech.stop();
            glib::Propagation::Proceed
        }
    });
    (window, reader)
}

fn draw_scene(cr: &gtk::cairo::Context, width: i32, height: i32, scene: Scene) {
    cr.save().expect("Save illustration context");
    cr.scale(f64::from(width) / 440.0, f64::from(height) / 150.0);
    cr.set_source_rgb(0.77, 0.88, 0.93);
    cr.paint().expect("Paint sky");
    cr.set_source_rgb(0.95, 0.78, 0.30);
    cr.arc(380.0, 27.0, 15.0, 0.0, std::f64::consts::TAU);
    cr.fill().expect("Paint sun");
    cr.set_source_rgb(0.42, 0.61, 0.43);
    cr.rectangle(0.0, 104.0, 440.0, 46.0);
    cr.fill().expect("Paint ground");
    let rect = |x, y, w, h, color: (f64, f64, f64)| {
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.rectangle(x, y, w, h);
        cr.fill().expect("Paint scene detail");
    };
    match scene {
        Scene::Home => {
            rect(90.0, 60.0, 150.0, 68.0, (0.65, 0.23, 0.21));
            cr.set_source_rgb(0.25, 0.27, 0.31);
            cr.move_to(70.0, 63.0);
            cr.line_to(165.0, 13.0);
            cr.line_to(260.0, 63.0);
            cr.close_path();
            cr.fill().expect("Paint roof");
            rect(145.0, 83.0, 30.0, 45.0, (0.92, 0.80, 0.49));
            rect(106.0, 77.0, 24.0, 24.0, (0.85, 0.94, 0.96));
            rect(197.0, 77.0, 24.0, 24.0, (0.85, 0.94, 0.96));
        }

        Scene::Woods => {
            rect(0.0, 112.0, 440.0, 17.0, (0.48, 0.72, 0.81));
            for x in [45.0, 100.0, 175.0, 280.0, 340.0] {
                rect(x - 4.0, 65.0, 8.0, 65.0, (0.42, 0.29, 0.17));
                cr.set_source_rgb(0.15, 0.38, 0.28);
                cr.move_to(x, 13.0);
                cr.line_to(x - 30.0, 95.0);
                cr.line_to(x + 30.0, 95.0);
                cr.close_path();
                cr.fill().expect("Paint pine");
            }
        }
        Scene::Shop | Scene::Ikea | Scene::City => {
            let color = if scene == Scene::Shop {
                (0.67, 0.21, 0.25)
            } else if scene == Scene::Ikea {
                (0.18, 0.35, 0.56)
            } else {
                (0.73, 0.55, 0.37)
            };
            rect(55.0, 36.0, 260.0, 90.0, color);
            rect(67.0, 48.0, 235.0, 15.0, (0.95, 0.86, 0.57));
            for x in [75.0, 130.0, 245.0] {
                rect(x, 77.0, 40.0, 40.0, (0.79, 0.90, 0.92));
            }
            rect(191.0, 75.0, 35.0, 51.0, (0.25, 0.32, 0.38));
            if scene == Scene::City {
                rect(20.0, 100.0, 20.0, 30.0, (0.47, 0.32, 0.25));
            }
        }
        Scene::Road => {
            rect(0.0, 100.0, 440.0, 50.0, (0.31, 0.35, 0.38));
            for x in [0.0, 90.0, 180.0, 270.0, 360.0] {
                rect(x, 130.0, 45.0, 4.0, (0.96, 0.91, 0.71));
            }
            rect(110.0, 82.0, 130.0, 35.0, (0.65, 0.25, 0.22));
            rect(135.0, 60.0, 70.0, 25.0, (0.65, 0.25, 0.22));
            rect(143.0, 65.0, 52.0, 17.0, (0.79, 0.91, 0.94));
            for x in [135.0, 218.0] {
                cr.set_source_rgb(0.16, 0.19, 0.22);
                cr.arc(x, 116.0, 13.0, 0.0, std::f64::consts::TAU);
                cr.fill().expect("Paint wheel");
            }
        }
        Scene::Train => {
            rect(0.0, 127.0, 440.0, 5.0, (0.28, 0.31, 0.32));
            rect(40.0, 49.0, 300.0, 67.0, (0.86, 0.89, 0.88));
            rect(40.0, 99.0, 300.0, 11.0, (0.35, 0.49, 0.55));
            for x in [58.0, 112.0, 166.0, 220.0, 274.0] {
                rect(x, 62.0, 37.0, 27.0, (0.22, 0.36, 0.44));
            }
            for x in [82.0, 150.0, 245.0, 308.0] {
                cr.set_source_rgb(0.21, 0.26, 0.29);
                cr.arc(x, 119.0, 9.0, 0.0, std::f64::consts::TAU);
                cr.fill().expect("Paint train wheel");
            }
        }
    }
    for (x, coat) in [(365.0, (0.74, 0.35, 0.24)), (399.0, (0.26, 0.39, 0.54))] {
        cr.set_source_rgb(0.85, 0.67, 0.49);
        cr.arc(x, 79.0, 8.0, 0.0, std::f64::consts::TAU);
        cr.fill().expect("Paint character");
        rect(x - 8.0, 89.0, 16.0, 28.0, coat);
        rect(x - 6.0, 117.0, 4.0, 17.0, (0.23, 0.26, 0.30));
        rect(x + 2.0, 117.0, 4.0, 17.0, (0.23, 0.26, 0.30));
    }
    cr.restore().expect("Restore illustration context");
}

#[cfg(test)]
pub fn test_flow(app: &adw::Application, wav: &[u8]) {
    use std::collections::HashSet;
    use std::fs;
    use std::time::{Duration, Instant};
    let directory =
        std::env::temp_dir().join(format!("swedish-book-test-{}", glib::uuid_string_random()));
    fs::create_dir(&directory).unwrap();
    let book = Book::default();
    let mut phrases = book.page().phrases();
    phrases.extend(
        book.page()
            .choices
            .iter()
            .map(|choice| choice.reply.phrase()),
    );
    for phrase in &phrases {
        let folder = directory.join(phrase.voice.unwrap().azure_name());
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(format!("{}.wav", phrase.audio_stem())), wav).unwrap();
        let words: Vec<_> = phrase.swedish.split_whitespace().collect();
        let timings: Vec<_> = words.iter().enumerate().map(|(index, word)| {
            serde_json::json!({"word": word, "start_us": index * 500_000 / words.len(), "end_us": (index + 1) * 500_000 / words.len()})
        }).collect();
        fs::write(
            folder.join(format!("{}.json", phrase.audio_stem())),
            serde_json::to_vec(&serde_json::json!({"duration_us": 500_000, "words": timings}))
                .unwrap(),
        )
        .unwrap();
    }
    let (window, reader) = build(app, Rc::new(Speech::for_story_test(directory.clone())));
    assert!(reader.pause_sentences.is_active());
    reader.automatic.set_active(false);
    window.present();
    reader.show();
    let context = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    while (window.width() == 0 || window.height() == 0) && Instant::now() < deadline {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    let settle = Instant::now() + Duration::from_millis(100);
    while Instant::now() < settle {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        window.width() > 0
            && window.height() > 0
            && window.width() <= 900
            && window.height() <= 600,
        "Story window size invalid or exceeded 900×600: {}×{}",
        window.width(),
        window.height()
    );
    for (name, widget) in [
        ("entry", reader.entry.upcast_ref::<gtk::Widget>()),
        ("skip", reader.skip.upcast_ref()),
        ("next", reader.next.upcast_ref()),
        ("automatic", reader.automatic.upcast_ref()),
        ("pause_sentences", reader.pause_sentences.upcast_ref()),
        ("english", reader.english.upcast_ref()),
        ("illustration", reader.illustration.upcast_ref()),
    ] {
        let bounds = widget.compute_bounds(&window).unwrap();
        assert!(
            bounds.y() >= 0.0 && bounds.y() + bounds.height() <= window.height() as f32,
            "Widget {name} out of bounds: {bounds:?} in {}×{}",
            window.width(),
            window.height()
        );
    }
    assert!(!reader.transcript.text().contains("It is Saturday"));
    reader.english.set_active(true);
    assert!(reader.transcript.text().contains("It is Saturday"));
    reader.english.set_active(false);
    let window_keys = (0..window.observe_controllers().n_items())
        .filter_map(|index| window.observe_controllers().item(index))
        .filter_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
        .find(|controller| controller.propagation_phase() == gtk::PropagationPhase::Capture)
        .unwrap();
    let wait_until_paused = |reader: &Reader| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !reader.speech.is_paused() {
            assert!(
                Instant::now() < deadline,
                "Story speech did not pause: {}",
                reader.status.text()
            );
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    reader.speak();
    assert!(!reader.entry.is_sensitive());
    assert!(reader.skip.is_sensitive());
    wait_until_paused(&reader);
    assert!(reader.status.text().contains("Paused after sentence"));
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::space,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(reader.status.text().contains("Det är lördag."));
    wait_until_paused(&reader);
    assert!(reader.status.text().contains("Paused after sentence"));
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Left,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(!reader.speech.is_paused());
    wait_until_paused(&reader);
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::space,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(!reader.speech.is_paused());
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::space,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(reader.speech.is_paused());
    assert!(reader.status.text().starts_with("Paused ·"));
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::space,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(!reader.speech.is_paused());
    wait_until_paused(&reader);
    reader.pause_sentences.set_active(false);
    assert!(!reader.speech.is_paused());
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut voices = HashSet::new();
    let mut highlighted = false;
    while reader.speaking.get() {
        assert!(Instant::now() < deadline, "{}", reader.status.text());
        if let Some(voice) = reader.transcript.tooltip_text() {
            voices.insert(voice.to_string());
        }
        highlighted |= reader.transcript.label().contains("background=");
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        reader.status.text().starts_with("Swedish speech ready"),
        "{}",
        reader.status.text()
    );
    assert!(highlighted);
    assert!(
        voices.contains("Speech voice: sv-SE-SofieNeural")
            && voices.contains("Speech voice: sv-SE-MattiasNeural")
    );
    assert!(reader.entry.is_sensitive());
    assert!(!window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::space,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    reader.entry.set_text("hej");
    assert!(!window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Left,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    reader.entry.set_text("");
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Left,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(reader.speaking.get());
    let deadline = Instant::now() + Duration::from_secs(5);
    while reader.speaking.get() {
        assert!(Instant::now() < deadline, "{}", reader.status.text());
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    reader.entry.set_text("0");
    reader.entry.emit_activate();
    assert!(reader.status.text().contains("Enter 1"));
    let wrong = reader
        .book
        .borrow()
        .page()
        .choices
        .iter()
        .position(|choice| choice.next.is_none())
        .unwrap();
    reader.choices[wrong].emit_clicked();
    assert!(reader.next.is_sensitive());
    assert!(!reader.entry.is_sensitive());
    assert!(reader.transcript.text().contains("Din fru"));
    reader.next.emit_clicked();
    assert_eq!(reader.book.borrow().page().id, "home-weather");
    let right = reader
        .book
        .borrow()
        .page()
        .choices
        .iter()
        .find(|choice| choice.next.is_some())
        .unwrap()
        .line
        .swedish
        .to_string();
    reader.entry.set_text(&right);
    reader.entry.emit_activate();
    reader.next.emit_clicked();
    assert_eq!(reader.book.borrow().page().id, "home-keys");
    reader.skip.emit_clicked();
    assert_eq!(reader.book.borrow().page().id, "home-plan");
    reader.automatic.set_active(true);
    assert!(
        !reader.speaking.get(),
        "Missing audio must release the controls"
    );
    assert!(reader.entry.is_sensitive());
    assert!(reader.status.text().contains("Speech audio is missing"));
    window.close();
    for voice in fs::read_dir(&directory).unwrap() {
        let folder = voice.unwrap().path();
        for file in fs::read_dir(&folder).unwrap() {
            fs::remove_file(file.unwrap().path()).unwrap();
        }
        fs::remove_dir(folder).unwrap();
    }
    fs::remove_dir(directory).unwrap();
}
