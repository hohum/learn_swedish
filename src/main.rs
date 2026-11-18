mod speech;
mod story_ui;

use adw::gtk::{self, glib};
use adw::prelude::*;
use learn_swedish::lesson::{
    Answer, Category, Lesson, Phrase, feedback_count, feedback_phrase, next_phrase,
    next_phrase_count, spoken_input, wait_phrase, wait_phrase_count,
};
use speech::Speech;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Practice {
    lesson: RefCell<Lesson>,
    category: gtk::DropDown,
    symbol: gtk::Label,
    english: gtk::Label,
    swedish: gtk::Label,
    keys: gtk::Label,
    progress: gtk::Label,
    entry: gtk::Entry,
    feedback: gtk::Label,
    next: gtk::Button,
    replay: gtk::Button,
    automatic: gtk::CheckButton,
    speak_typed: gtk::CheckButton,
    auto_next: gtk::CheckButton,
    show_english: gtk::CheckButton,
    next_timer: RefCell<Option<glib::SourceId>>,
    speech_status: gtk::Label,
    follow_along: gtk::Label,
    listening_heading: gtk::Label,
    speaking: Cell<bool>,
    speech: Rc<Speech>,
}

impl Practice {
    fn selected_category(&self) -> Category {
        match self.category.selected() {
            0 => Category::Alphabet,
            1 => Category::Numbers,
            2 => Category::Symbols,
            3 => Category::All,
            _ => unreachable!("The category dropdown has exactly four entries"),
        }
    }

    fn restart(self: &Rc<Self>) {
        self.lesson
            .replace(Self::shuffled_lesson(self.selected_category()));
        self.show_card();
    }

    fn shuffled_lesson(category: Category) -> Lesson {
        Lesson::shuffled(category, |upper| {
            usize::try_from(glib::random_int_range(
                0,
                i32::try_from(upper).expect("Practice sets fit in i32"),
            ))
            .expect("Shuffle choices are nonnegative")
        })
    }

    fn show_card(self: &Rc<Self>) {
        self.cancel_next_timer();
        self.speech.stop();
        self.speech.show_transcript(&[], &self.follow_along);
        self.entry.set_text("");
        let lesson = self.lesson.borrow();
        self.entry.set_sensitive(lesson.current().is_some());
        self.entry.set_editable(true);
        self.replay.set_sensitive(lesson.current().is_some());
        if let Some(card) = lesson.current() {
            self.symbol.set_label(&card.display());
            self.english.set_label(&card.english);
            self.swedish.set_label(&card.swedish);
            self.keys
                .set_label(&format!("Swedish keyboard: {}", card.keys));
            self.progress.set_label(&format!(
                "{} / {} · {} correct · {} mistakes",
                lesson.position() + 1,
                lesson.total(),
                lesson.correct(),
                lesson.mistakes()
            ));
            self.feedback
                .set_label("Type the symbol. There is no time limit.");
            self.next
                .set_label(if lesson.position() + 1 == lesson.total() {
                    "Finish / Skip"
                } else {
                    "Next / Skip"
                });
        } else {
            self.symbol.set_label("✓");
            self.english.set_label("Practice complete");
            self.swedish.set_label("Övningen är klar");
            self.keys
                .set_label("Restart, or choose another practice set.");
            let summary = format!(
                "{} correct · {} skipped · {} mistakes",
                lesson.correct(),
                lesson.total() - lesson.correct(),
                lesson.mistakes()
            );
            self.progress.set_label(&summary);
            self.feedback
                .set_label("Well done. Take as much practice as you need.");
            self.next.set_label("Practice again");
        }
        drop(lesson);
        self.entry.grab_focus();
        if self.automatic.is_active() {
            self.speak_current();
        } else {
            self.speech_status
                .set_label("Automatic speech off · Replay is still available");
        }
    }

    fn speak_current(self: &Rc<Self>) {
        self.cancel_next_timer();
        let phrase = self.lesson.borrow().current().cloned().map(Phrase::from);
        if let Some(phrase) = phrase {
            self.play(vec![phrase], false);
        } else {
            self.speech_status.set_label("Practice complete");
        }
    }

    fn play(self: &Rc<Self>, phrases: Vec<Phrase>, advance: bool) {
        self.speech.stop();
        self.speaking.set(true);
        self.entry.set_editable(false);
        let weak = Rc::downgrade(self);
        self.speech.speak_then(
            phrases,
            &self.speech_status,
            &self.follow_along,
            move |completed| {
                if let Some(practice) = weak.upgrade() {
                    practice.speaking.set(false);
                    practice
                        .entry
                        .set_editable(!practice.lesson.borrow().answered());
                    if completed && advance {
                        practice.advance_after_answer();
                    }
                }
            },
        );
    }

    fn request_wait(&self) {
        if self.speaking.get() {
            let choice = usize::try_from(glib::random_int_range(
                0,
                i32::try_from(wait_phrase_count()).expect("Wait phrase count fits i32"),
            ))
            .expect("Wait phrase choice is nonnegative");
            self.speech.interrupt_with(wait_phrase(choice));
        }
    }

    fn cancel_next_timer(&self) {
        if let Some(timer) = self.next_timer.borrow_mut().take() {
            timer.remove();
        }
    }

    fn advance_after_answer(self: &Rc<Self>) {
        if self.auto_next.is_active() && self.lesson.borrow().answered() {
            self.lesson.borrow_mut().advance();
            self.show_card();
        }
    }

    fn answer(self: &Rc<Self>, input: &str) {
        if self.speaking.get() {
            self.request_wait();
            return;
        }
        let result = self.lesson.borrow_mut().submit(input);
        if !matches!(result, Answer::Correct | Answer::Incorrect) {
            return;
        }
        match result {
            Answer::Correct => {
                self.entry.set_editable(false);
                self.feedback.set_label(if self.auto_next.is_active() {
                    "Correct! / Rätt! · Moving on after feedback."
                } else {
                    "Correct! / Rätt! · Press Next when you are ready."
                });
                let lesson = self.lesson.borrow();
                self.next
                    .set_label(if lesson.position() + 1 == lesson.total() {
                        "Finish"
                    } else {
                        "Next"
                    });
            }
            Answer::Incorrect => {
                self.feedback
                    .set_label("Not quite. / Försök igen. · Try the same symbol again.");
            }
            Answer::AlreadyCorrect | Answer::Finished => {}
        }
        let mut phrases = Vec::new();
        if let Some(phrase) = spoken_input(input) {
            phrases.push(phrase);
        } else {
            self.feedback.set_label(
                "Not quite. / Försök igen.\nNo spoken name for this input. Enter one supported character at a time.",
            );
        }
        let choice = usize::try_from(glib::random_int_range(
            0,
            i32::try_from(feedback_count(result == Answer::Correct))
                .expect("Feedback count fits i32"),
        ))
        .expect("Feedback choices are nonnegative");
        phrases.push(feedback_phrase(result == Answer::Correct, choice));
        let advance = result == Answer::Correct && self.auto_next.is_active();
        if advance {
            let lesson = self.lesson.borrow();
            if lesson.position() + 1 < lesson.total() {
                let choice = usize::try_from(glib::random_int_range(
                    0,
                    i32::try_from(next_phrase_count()).expect("Next phrase count fits i32"),
                ))
                .expect("Next phrase choice is nonnegative");
                phrases.push(next_phrase(choice));
            }
        }
        self.speech.stop();
        self.speech.show_transcript(&phrases, &self.follow_along);
        if self.speak_typed.is_active() {
            self.play(phrases, advance);
        } else if advance {
            let weak = Rc::downgrade(self);
            let timer =
                glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || {
                    if let Some(practice) = weak.upgrade() {
                        practice.next_timer.borrow_mut().take();
                        practice.advance_after_answer();
                    }
                });
            self.next_timer.replace(Some(timer));
        }
        let lesson = self.lesson.borrow();
        if lesson.current().is_some() {
            self.progress.set_label(&format!(
                "{} / {} · {} correct · {} mistakes",
                lesson.position() + 1,
                lesson.total(),
                lesson.correct(),
                lesson.mistakes()
            ));
        }
    }
}

fn main() -> glib::ExitCode {
    let preparation = match std::env::args_os().nth(1).as_deref() {
        Some(arg) if arg == "--prepare-speech" => Some(speech::prepare_speech()),
        Some(arg) if arg == "--prepare-azure-speech" => Some(speech::prepare_azure_speech()),
        Some(arg) if arg == "--prepare-adventure-speech" => {
            Some(speech::prepare_adventure_speech())
        }
        _ => None,
    };
    if let Some(result) = preparation {
        return match result {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Speech preparation failed: {error}");
                glib::ExitCode::FAILURE
            }
        };
    }
    let app = adw::Application::builder()
        .application_id("io.github.hohum.LearnSwedish")
        .build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        build_launcher(app, Rc::new(|| Rc::new(Speech::default()))).present();
    });
    app.run()
}

fn build_launcher(
    app: &adw::Application,
    new_speech: Rc<dyn Fn() -> Rc<Speech>>,
) -> adw::ApplicationWindow {
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Svenska / Swedish",
        "Välj en övning · Choose an activity",
    )));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header);
    let choices = gtk::Box::new(gtk::Orientation::Vertical, 16);
    choices.set_halign(gtk::Align::Center);
    choices.set_valign(gtk::Align::Center);
    choices.set_vexpand(true);
    choices.set_margin_top(24);
    choices.set_margin_bottom(24);
    choices.set_margin_start(24);
    choices.set_margin_end(24);
    let heading = label(
        "Vad vill du öva?\nWhat would you like to practise?",
        "title-2",
    );
    choices.append(&heading);
    let keyboard = gtk::Button::with_label("Tangentbordsövning\nKeyboard practice");
    keyboard.set_widget_name("launcher-keyboard");
    let story = gtk::Button::with_label("Välj ditt eget äventyr\nChoose your own adventure");
    story.set_widget_name("launcher-story");
    keyboard.set_hexpand(true);
    story.set_hexpand(true);
    choices.append(&keyboard);
    choices.append(&story);
    content.append(&choices);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Svenska · Swedish")
        .default_width(900)
        .default_height(600)
        .content(&content)
        .build();
    let keyboard_speech = new_speech.clone();
    keyboard.connect_clicked(glib::clone!(
        #[weak]
        app,
        #[weak]
        window,
        move |_| {
            let (practice_window, practice) = build_window(&app, keyboard_speech());
            practice_window.connect_close_request({
                let launcher = window.clone();
                move |_| {
                    launcher.present();
                    glib::Propagation::Proceed
                }
            });
            window.hide();
            practice_window.present();
            practice.show_card();
        }
    ));
    story.connect_clicked(glib::clone!(
        #[weak]
        app,
        #[weak]
        window,
        move |_| story_ui::open(&app, &window, new_speech())
    ));
    window
}

fn build_window(
    app: &adw::Application,
    speech: Rc<Speech>,
) -> (adw::ApplicationWindow, Rc<Practice>) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".practice-symbol { font-size: 64pt; font-weight: 700; }
         .practice-hint { font-size: 11pt; }",
    );
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default()
            .expect("GTK must have a display after application activation"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Swedish Keys",
        "Level 1 · Untimed practice",
    )));
    content.append(&header);
    let menu = gtk::Button::with_label("Meny · Menu");
    menu.set_widget_name("activity-menu");
    header.pack_start(&menu);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_top(10);
    body.set_margin_bottom(10);
    body.set_margin_start(16);
    body.set_margin_end(16);
    let category = gtk::DropDown::from_strings(&[
        "Alphabet · A–Z, Å, Ä, Ö",
        "Numbers · 0–9",
        "Symbols · Linux & programming",
        "Everything · Random order",
    ]);
    category.set_tooltip_text(Some("Changing the practice set starts a new session"));
    category.set_selected(3);
    let chooser = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    chooser.append(&gtk::Label::new(Some("Practice set")));
    category.set_hexpand(true);
    chooser.append(&category);
    let progress = label("", "dim-label");
    chooser.append(&progress);
    body.append(&chooser);

    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    columns.set_homogeneous(true);
    columns.set_vexpand(true);
    let practice_panel = gtk::Box::new(gtk::Orientation::Vertical, 6);
    columns.append(&practice_panel);
    let symbol = label("", "practice-symbol");
    symbol.set_vexpand(true);
    practice_panel.append(&symbol);
    let english = label("", "practice-hint");
    english.set_visible(false);
    let swedish = label("", "practice-hint");
    let keys = label("", "dim-label");
    practice_panel.append(&english);
    practice_panel.append(&swedish);
    practice_panel.append(&keys);

    let entry = gtk::Entry::builder()
        .placeholder_text("Type the displayed symbol here")
        .halign(gtk::Align::Center)
        .width_chars(28)
        .enable_undo(false)
        .input_purpose(gtk::InputPurpose::FreeForm)
        .build();
    entry.set_tooltip_text(Some(
        "Letters accept either case. For dead-key symbols, follow the hint: Space or a double press.",
    ));
    practice_panel.append(&entry);
    let feedback = label("", "");
    practice_panel.append(&feedback);
    let follow_along = label("", "practice-hint");

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::Center);
    let replay = gtk::Button::with_label("Replay Swedish");
    let next = gtk::Button::with_label("Next / Skip");
    next.add_css_class("suggested-action");
    let restart = gtk::Button::with_label("Restart");
    buttons.append(&replay);
    buttons.append(&next);
    buttons.append(&restart);
    practice_panel.append(&buttons);

    let listening_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    listening_panel.set_margin_top(12);
    listening_panel.set_margin_bottom(12);
    listening_panel.set_margin_start(12);
    listening_panel.set_margin_end(12);
    let listening_heading = label("Listen & follow · Svenska", "heading");
    listening_panel.append(&listening_heading);
    follow_along.set_vexpand(true);
    listening_panel.append(&follow_along);
    let speech_status = label("", "dim-label");
    listening_panel.append(&speech_status);
    let listening_frame = gtk::Frame::new(None);
    listening_frame.add_css_class("card");
    listening_frame.set_child(Some(&listening_panel));
    columns.append(&listening_frame);
    body.append(&columns);

    let automatic = gtk::CheckButton::with_label("Speak each new symbol in Swedish");
    automatic.set_active(true);
    automatic.set_halign(gtk::Align::Start);
    let speak_typed = gtk::CheckButton::with_label("Speak my answers and feedback in Swedish");
    speak_typed.set_active(true);
    speak_typed.set_halign(gtk::Align::Start);
    let auto_next = gtk::CheckButton::with_label("Automatic next");
    auto_next.set_active(true);
    auto_next.set_halign(gtk::Align::Start);
    let show_english = gtk::CheckButton::with_label("Show English hints");
    show_english.set_halign(gtk::Align::Start);
    let options = gtk::Grid::builder()
        .row_spacing(0)
        .column_spacing(16)
        .build();
    options.attach(&automatic, 0, 0, 1, 1);
    options.attach(&speak_typed, 0, 1, 1, 1);
    options.attach(&auto_next, 1, 0, 1, 1);
    options.attach(&show_english, 1, 1, 1, 1);
    options.set_halign(gtk::Align::Center);
    body.append(&options);
    body.append(&label(
        "Use Ubuntu’s Swedish input source · AltGr = Right Alt · No time limit",
        "dim-label",
    ));
    let clamp = adw::Clamp::builder().maximum_size(960).child(&body).build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();
    content.append(&scroll);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Swedish Keys")
        .default_width(900)
        .default_height(600)
        .content(&content)
        .build();
    let practice = Rc::new(Practice {
        lesson: RefCell::new(Practice::shuffled_lesson(Category::All)),
        category,
        symbol,
        english,
        swedish,
        keys,
        progress,
        entry,
        feedback,
        next,
        replay,
        automatic,
        speak_typed,
        auto_next,
        show_english,
        next_timer: RefCell::new(None),
        speech_status,
        follow_along,
        listening_heading,
        speaking: Cell::new(false),
        speech,
    });
    menu.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));

    practice.show_english.connect_toggled(glib::clone!(
        #[weak]
        practice,
        move |show_english| {
            practice.english.set_visible(show_english.is_active());
            practice.speech.set_show_english(show_english.is_active());
            practice
                .listening_heading
                .set_label(if show_english.is_active() {
                    "Listen & follow · Svenska / English"
                } else {
                    "Listen & follow · Svenska"
                });
            practice.entry.grab_focus();
        }
    ));
    let input_guard = gtk::EventControllerKey::new();
    input_guard.set_propagation_phase(gtk::PropagationPhase::Capture);
    input_guard.connect_key_pressed(glib::clone!(
        #[weak]
        practice,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, modifiers| {
            let editing_key = key.to_unicode().is_some_and(|c| !c.is_control())
                || key.name().is_some_and(|name| name.starts_with("dead_"))
                || matches!(key, gtk::gdk::Key::BackSpace | gtk::gdk::Key::Delete)
                || (key == gtk::gdk::Key::v
                    && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK));
            if practice.speaking.get() && editing_key {
                practice.request_wait();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    practice.entry.add_controller(input_guard);
    practice.entry.connect_changed(glib::clone!(
        #[weak]
        practice,
        move |entry| {
            let input = entry.text();
            if !input.is_empty() {
                // Entry's input method commits dead keys and AltGr before validation.
                practice.answer(&input);
                entry.select_region(0, -1);
            }
        }
    ));
    practice.next.connect_clicked(glib::clone!(
        #[weak]
        practice,
        move |_| {
            if practice.lesson.borrow().current().is_none() {
                practice.restart();
            } else {
                practice.lesson.borrow_mut().advance();
                practice.show_card();
            }
        }
    ));
    restart.connect_clicked(glib::clone!(
        #[weak]
        practice,
        move |_| practice.restart()
    ));
    practice.category.connect_selected_notify(glib::clone!(
        #[weak]
        practice,
        move |_| practice.restart()
    ));
    practice.replay.connect_clicked(glib::clone!(
        #[weak]
        practice,
        move |_| {
            practice.speak_current();
            practice.entry.grab_focus();
        }
    ));
    practice.automatic.connect_toggled(glib::clone!(
        #[weak]
        practice,
        move |automatic| {
            if automatic.is_active() {
                practice.speak_current();
            } else {
                practice.cancel_next_timer();
                practice.speech.stop();
                practice
                    .speech_status
                    .set_label("Automatic speech off · Replay is still available");
            }
            practice.entry.grab_focus();
        }
    ));
    practice.speak_typed.connect_toggled(glib::clone!(
        #[weak]
        practice,
        move |speak_typed| {
            if !speak_typed.is_active() {
                practice.cancel_next_timer();
                practice.speech.stop();
                practice.speech_status.set_label("Typed speech off");
            }
            practice.entry.grab_focus();
        }
    ));
    practice.auto_next.connect_toggled(glib::clone!(
        #[weak]
        practice,
        move |auto_next| {
            if !auto_next.is_active() {
                practice.cancel_next_timer();
                practice.speech.stop();
                if practice.lesson.borrow().answered() {
                    practice
                        .feedback
                        .set_label("Correct! / Rätt! · Press Next when you are ready.");
                }
            }
            practice.entry.grab_focus();
        }
    ));
    window.connect_close_request({
        let practice = practice.clone();
        move |_| {
            practice.cancel_next_timer();
            practice.speech.stop();
            glib::Propagation::Proceed
        }
    });
    (window, practice)
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn finish_playback(practice: &Practice) -> (Vec<String>, Vec<String>) {
        let context = glib::MainContext::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut spoken = Vec::new();
        let mut highlighted = Vec::new();
        while practice
            .speech_status
            .text()
            .starts_with("Speaking Swedish:")
        {
            assert!(
                Instant::now() < deadline,
                "Speech playback did not finish: {}",
                practice.speech_status.text()
            );
            let status = practice.speech_status.text().to_string();
            if spoken.last() != Some(&status) {
                spoken.push(status);
            }
            if practice.follow_along.label().contains("background=") {
                highlighted.push(practice.follow_along.label().to_string());
            }
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            practice
                .speech_status
                .text()
                .contains("Swedish speech ready"),
            "{}",
            practice.speech_status.text()
        );
        assert!(!practice.follow_along.label().contains("background="));
        (spoken, highlighted)
    }

    fn wait_for_advance(practice: &Practice, position: usize) -> Vec<String> {
        let context = glib::MainContext::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut spoken = Vec::new();
        while practice.lesson.borrow().position() == position {
            assert!(
                Instant::now() < deadline,
                "Automatic next did not advance: {}",
                practice.speech_status.text()
            );
            let status = practice.speech_status.text().to_string();
            if status.starts_with("Speaking Swedish:") && spoken.last() != Some(&status) {
                spoken.push(status);
            }
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(practice.lesson.borrow().position(), position + 1);
        spoken
    }

    fn silent_wav() -> Vec<u8> {
        let data_size = 22_050_u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + data_size).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16_u32.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(22_050_u32.to_le_bytes());
        wav.extend(44_100_u32.to_le_bytes());
        wav.extend(2_u16.to_le_bytes());
        wav.extend(16_u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(data_size.to_le_bytes());
        wav.resize(44 + usize::try_from(data_size).unwrap(), 0);
        wav
    }

    fn ordered_set(practice: &Rc<Practice>, selected: u32) {
        practice.category.set_selected(selected);
        practice
            .lesson
            .replace(Lesson::new(practice.selected_category()));
        practice.show_card();
    }

    fn replace_selected_input(entry: &gtk::Entry, input: &str) {
        assert!(entry.selection_bounds().is_some());
        entry.delete_selection();
        let mut position = entry.position();
        entry.insert_text(input, &mut position);
    }

    fn assert_compact_layout(window: &adw::ApplicationWindow, practice: &Rc<Practice>) {
        let card = learn_swedish::lesson::cards(Category::Symbols)
            .into_iter()
            .max_by_key(|card| card.keys.len())
            .unwrap();
        practice.symbol.set_label(&card.display());
        practice.english.set_label(&card.english);
        practice.swedish.set_label(&card.swedish);
        practice
            .keys
            .set_label(&format!("Swedish keyboard: {}", card.keys));
        practice
            .feedback
            .set_label("Correct! / Rätt! · Moving on after feedback.");
        practice.follow_along.set_markup(&speech::transcript_markup(
            &[
                Phrase::from(card),
                feedback_phrase(true, 14),
                next_phrase(9),
            ],
            Some((2, 2)),
            true,
        ));
        practice
            .speech_status
            .set_label("Swedish speech ready · Replay to hear the prompt again");
        let context = glib::MainContext::default();
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            window.width() <= 900 && window.height() <= 600,
            "Window exceeded the 768-pixel-screen layout size: {} × {}",
            window.width(),
            window.height()
        );
        let scroll = practice
            .entry
            .ancestor(gtk::ScrolledWindow::static_type())
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let adjustment = scroll.vadjustment();
        assert!(
            adjustment.upper() <= adjustment.page_size() + 1.0,
            "The full feedback layout needs scrolling: content {}, viewport {}",
            adjustment.upper(),
            adjustment.page_size()
        );
        for widget in [
            practice.entry.upcast_ref::<gtk::Widget>(),
            practice.replay.upcast_ref(),
            practice.next.upcast_ref(),
            practice.keys.upcast_ref(),
            practice.follow_along.upcast_ref(),
            practice.automatic.upcast_ref(),
            practice.speak_typed.upcast_ref(),
            practice.auto_next.upcast_ref(),
            practice.show_english.upcast_ref(),
        ] {
            let bounds = widget.compute_bounds(window).unwrap();
            assert!(
                bounds.x() >= 0.0
                    && bounds.x() + bounds.width() <= window.width() as f32
                    && bounds.y() >= 0.0
                    && bounds.y() + bounds.height() <= window.height() as f32,
                "A practice control is outside the visible window"
            );
        }
        practice.show_card();
    }

    fn find_button(widget: &gtk::Widget, name: &str) -> Option<gtk::Button> {
        if widget.widget_name() == name {
            return widget.clone().downcast::<gtk::Button>().ok();
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(button) = find_button(&current, name) {
                return Some(button);
            }
            child = current.next_sibling();
        }
        None
    }

    fn assert_launcher_flow(app: &adw::Application, voice_dirs: Vec<PathBuf>) {
        let created = Rc::new(Cell::new(0));
        let count = created.clone();
        let launcher = build_launcher(
            app,
            Rc::new(move || {
                count.set(count.get() + 1);
                Rc::new(Speech::for_test(voice_dirs.clone(), true))
            }),
        );
        launcher.present();
        assert_eq!(
            created.get(),
            0,
            "The launcher must not create or start speech"
        );
        let keyboard = find_button(launcher.upcast_ref(), "launcher-keyboard").unwrap();
        let story = find_button(launcher.upcast_ref(), "launcher-story").unwrap();
        assert_eq!(
            keyboard.label().as_deref(),
            Some("Tangentbordsövning\nKeyboard practice")
        );
        assert_eq!(
            story.label().as_deref(),
            Some("Välj ditt eget äventyr\nChoose your own adventure")
        );
        let context = glib::MainContext::default();
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(launcher.is_visible());
        assert!(launcher.width() <= 900 && launcher.height() <= 600);
        for button in [&keyboard, &story] {
            let bounds = button.compute_bounds(&launcher).unwrap();
            assert!(
                bounds.y() >= 0.0 && bounds.y() + bounds.height() <= launcher.height() as f32,
                "Launcher option bounds: {bounds:?}, window height: {}",
                launcher.height()
            );
        }
        for (button, title, expected_count) in [
            (&keyboard, "Swedish Keys", 1),
            (&story, "Swedish adventure", 2),
            (&keyboard, "Swedish Keys", 3),
        ] {
            button.emit_clicked();
            assert!(!launcher.is_visible());
            assert_eq!(created.get(), expected_count);
            let activity = app
                .windows()
                .into_iter()
                .find(|window| window.is_visible() && window.title().as_deref() == Some(title))
                .expect("Selected activity must be visible");
            let menu = find_button(activity.upcast_ref(), "activity-menu").unwrap();
            menu.emit_clicked();
            assert!(
                launcher.is_visible(),
                "Closing an activity must restore the launcher"
            );
            assert!(!activity.is_visible());
        }
        launcher.close();
    }

    #[test]
    #[ignore = "Requires a graphical display: xvfb-run -a cargo test --bin learn-swedish -- --ignored"]
    fn native_practice_flow() {
        adw::init().unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.hohum.LearnSwedish.Tests")
            .flags(adw::gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(adw::gio::Cancellable::NONE).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "swedish-speech-test-{}",
            glib::uuid_string_random()
        ));
        fs::create_dir(&directory).unwrap();
        let voice_dirs: Vec<_> = ["nst", "alma"]
            .iter()
            .map(|voice| directory.join(voice))
            .collect();
        for (voice_index, voice_dir) in voice_dirs.iter().enumerate() {
            fs::create_dir(voice_dir).unwrap();
            for phrase in learn_swedish::lesson::speech_phrases() {
                fs::write(
                    voice_dir.join(format!("{}.wav", phrase.swedish)),
                    silent_wav(),
                )
                .unwrap();
                let words: Vec<_> = phrase.swedish.split_whitespace().collect();
                let count = words.len();
                let duration = if voice_index == 0 { 500_000 } else { 350_000 };
                let timings: Vec<_> = words.iter().enumerate().map(|(index, word)| {
                serde_json::json!({"word": word, "start_us": index * duration / count, "end_us": (index + 1) * duration / count})
            }).collect();
                fs::write(
                    voice_dir.join(format!("{}.json", phrase.swedish)),
                    serde_json::to_vec(
                        &serde_json::json!({"duration_us": 500_000, "words": timings}),
                    )
                    .unwrap(),
                )
                .unwrap();
            }
        }
        let speech = Rc::new(Speech::for_test(voice_dirs.clone(), true));
        let (window, practice) = build_window(&app, speech);
        assert_eq!(practice.selected_category(), Category::All);
        assert!(!practice.show_english.is_active());
        assert!(!practice.english.is_visible());
        assert!(practice.auto_next.is_active());
        practice.auto_next.set_active(false);
        practice.automatic.set_active(false);
        window.present();
        practice.show_card();
        let context = glib::MainContext::default();
        while context.pending() {
            context.iteration(false);
        }
        assert!(window.is_visible());
        let character = practice.lesson.borrow().current().unwrap().character;
        practice.show_english.set_active(true);
        assert!(practice.english.is_visible());
        assert_eq!(
            practice.lesson.borrow().current().unwrap().character,
            character
        );
        assert_compact_layout(&window, &practice);
        practice.show_english.set_active(false);
        practice.next.emit_clicked();
        assert!(!practice.english.is_visible());
        practice.restart();
        assert!(!practice.english.is_visible());
        practice.play(vec![spoken_input("'").unwrap()], false);
        assert!(!practice.entry.is_editable());
        assert!(
            !practice
                .follow_along
                .text()
                .contains("Single quote / apostrophe")
        );
        let (spoken, _) = finish_playback(&practice);
        assert_eq!(spoken.len(), 1);
        assert!(practice.entry.is_editable());
        practice.show_english.set_active(true);
        assert!(
            practice
                .follow_along
                .text()
                .contains("Single quote / apostrophe")
        );
        practice.show_english.set_active(false);
        assert!(
            !practice
                .follow_along
                .text()
                .contains("Single quote / apostrophe")
        );
        let mut visited = std::collections::HashSet::new();
        let total = practice.lesson.borrow().total();
        for _ in 0..total {
            let character = practice.lesson.borrow().current().unwrap().character;
            assert!(
                visited.insert(character),
                "A shuffled session repeated a character"
            );
            assert_eq!(
                practice.symbol.text(),
                practice.lesson.borrow().current().unwrap().display()
            );
            practice.next.emit_clicked();
        }
        assert_eq!(
            visited.len(),
            learn_swedish::lesson::cards(Category::All).len()
        );
        assert!(practice.lesson.borrow().current().is_none());
        practice.next.emit_clicked();
        assert!(practice.lesson.borrow().current().is_some());
        assert_eq!(practice.lesson.borrow().position(), 0);
        // Keep the remaining widget checks independent of random card ordering.
        practice.show_english.set_active(true);
        ordered_set(&practice, 0);
        assert_eq!(practice.symbol.text(), "A");
        practice.entry.set_text("z");
        assert_eq!(practice.entry.text(), "z");
        assert_eq!(practice.entry.selection_bounds(), Some((0, 1)));
        assert!(practice.feedback.text().contains("Not quite"));
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Bokstaven Z"
        );
        assert!(practice.follow_along.text().contains("Letter Z"));
        practice.show_english.set_active(false);
        assert!(!practice.follow_along.text().contains("Letter Z"));
        assert!(practice.follow_along.text().contains("Bokstaven Z"));
        practice.show_english.set_active(true);
        assert!(practice.follow_along.text().contains("Letter Z"));
        let voice = practice.follow_along.tooltip_text().unwrap();
        let (spoken, highlighted) = finish_playback(&practice);
        assert_eq!(
            practice.follow_along.tooltip_text().as_ref(),
            Some(&voice),
            "{spoken:?}"
        );
        assert_eq!(spoken.len(), 2, "{spoken:?}");
        assert_eq!(spoken[0], "Speaking Swedish: Bokstaven Z");
        assert!(
            highlighted
                .iter()
                .any(|markup| markup.contains("weight=\"bold\">Bokstaven</span> Z"))
        );
        assert!(
            highlighted
                .iter()
                .any(|markup| markup.contains("Bokstaven <span")
                    && markup.contains("weight=\"bold\">Z</span>"))
        );
        let incorrect_feedback: Vec<_> = (0..feedback_count(false))
            .map(|index| feedback_phrase(false, index))
            .collect();
        assert!(incorrect_feedback.iter().any(|phrase| {
            spoken[1] == format!("Speaking Swedish: {}", phrase.swedish)
                && practice.follow_along.text().contains(&phrase.english)
        }));
        let mistakes = practice.lesson.borrow().mistakes();
        practice.entry.delete_selection();
        assert!(practice.entry.text().is_empty());
        assert_eq!(practice.lesson.borrow().mistakes(), mistakes);
        let mut position = 0;
        practice.entry.insert_text("å", &mut position);
        assert_eq!(practice.entry.text(), "å");
        assert_eq!(practice.entry.selection_bounds(), Some((0, 1)));
        assert_eq!(practice.lesson.borrow().mistakes(), mistakes + 1);
        assert!(!practice.entry.is_editable());
        let status = practice.speech_status.text();
        practice.answer("a");
        let reminder = practice.speech_status.text();
        assert_ne!(reminder, status);
        assert!(
            (0..wait_phrase_count()).any(
                |index| reminder == format!("Speaking Swedish: {}", wait_phrase(index).swedish)
            )
        );
        let controllers = practice.entry.observe_controllers();
        let guard = (0..controllers.n_items())
            .filter_map(|index| controllers.item(index))
            .filter_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
            .find(|controller| controller.propagation_phase() == gtk::PropagationPhase::Capture)
            .unwrap();
        assert!(guard.emit_by_name::<bool>(
            "key-pressed",
            &[&gtk::gdk::Key::a, &0_u32, &gtk::gdk::ModifierType::empty()],
        ));
        assert_eq!(practice.entry.text(), "å");
        assert_eq!(practice.lesson.borrow().mistakes(), mistakes + 1);
        assert!(!practice.lesson.borrow().answered());
        assert_eq!(practice.speech_status.text(), reminder);
        let (spoken, _) = finish_playback(&practice);
        assert_eq!(
            spoken.len(),
            3,
            "The reminder must be followed by the interrupted word and its feedback"
        );
        assert_eq!(spoken[0], reminder);
        assert_eq!(spoken[1], "Speaking Swedish: Bokstaven Å");
        assert!(practice.entry.is_editable());
        replace_selected_input(&practice.entry, "a");
        assert_eq!(practice.entry.text(), "a");
        assert!(practice.feedback.text().contains("Correct!"));
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Bokstaven A"
        );
        let (spoken, _) = finish_playback(&practice);
        assert_eq!(spoken.len(), 2);
        assert!((0..feedback_count(true)).any(|index| spoken[1]
            == format!("Speaking Swedish: {}", feedback_phrase(true, index).swedish)));
        assert!(!practice.entry.is_editable());
        assert_eq!(practice.symbol.text(), "A");
        practice.next.emit_clicked();
        assert_eq!(practice.symbol.text(), "B");
        assert!(practice.entry.text().is_empty());
        assert!(practice.entry.is_editable());
        practice.speak_typed.set_active(false);
        practice.entry.set_text("x");
        assert_eq!(practice.speech_status.text(), "Typed speech off");
        assert!(practice.follow_along.text().contains("Letter X"));
        assert!(!practice.follow_along.label().contains("background="));
        practice.speak_typed.set_active(true);
        replace_selected_input(&practice.entry, "B");
        assert_eq!(practice.entry.text(), "B");
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Bokstaven B"
        );
        finish_playback(&practice);

        ordered_set(&practice, 2);
        assert!(practice.entry.text().is_empty());
        assert_eq!(practice.symbol.text(), "/");
        practice.entry.set_text("\\");
        assert!(practice.feedback.text().contains("Not quite"));
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Omvänt snedstreck"
        );
        finish_playback(&practice);
        assert!(practice.follow_along.text().contains("Backslash"));
        replace_selected_input(&practice.entry, "/");
        assert_eq!(practice.entry.text(), "/");
        assert!(practice.feedback.text().contains("Correct!"));
        finish_playback(&practice);
        practice.next.emit_clicked();
        assert_eq!(practice.symbol.text(), "\\");
        practice.next.emit_clicked();
        practice.next.emit_clicked();
        assert_eq!(practice.symbol.text(), "~");
        practice.entry.set_text("~");
        assert_eq!(practice.entry.text(), "~");
        assert!(practice.feedback.text().contains("Correct!"));
        finish_playback(&practice);

        ordered_set(&practice, 1);
        let total = practice.lesson.borrow().total();
        for _ in 0..total {
            practice.next.emit_clicked();
        }
        assert!(!practice.entry.is_sensitive());
        assert!(!practice.replay.is_sensitive());
        assert!(practice.progress.text().contains("10 skipped"));
        practice.next.emit_clicked();
        assert!(practice.entry.is_sensitive());
        assert!(practice.entry.text().is_empty());
        ordered_set(&practice, 1);
        assert_eq!(practice.symbol.text(), "0");
        practice.entry.set_text("5");
        assert_eq!(practice.entry.text(), "5");
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Siffran fem"
        );
        finish_playback(&practice);

        ordered_set(&practice, 0);
        practice.entry.set_text("é");
        assert!(practice.feedback.text().contains("No spoken name"));
        finish_playback(&practice);
        practice.speak_typed.set_active(false);
        practice.automatic.set_active(true);
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Bokstaven A"
        );
        finish_playback(&practice);
        practice.replay.emit_clicked();
        ordered_set(&practice, 2);
        assert!(practice.follow_along.text().contains("Forward slash"));
        assert!(!practice.follow_along.text().contains("Letter A"));
        finish_playback(&practice);
        practice.automatic.set_active(false);
        practice.speak_typed.set_active(true);
        ordered_set(&practice, 1);
        practice.auto_next.set_active(true);
        practice.entry.set_text("5");
        let (spoken, _) = finish_playback(&practice);
        assert_eq!(spoken.len(), 2);
        assert_eq!(practice.lesson.borrow().position(), 0);
        assert_eq!(practice.entry.text(), "5");
        replace_selected_input(&practice.entry, "0");
        assert_eq!(practice.entry.text(), "0");
        assert_eq!(practice.lesson.borrow().position(), 0);
        let spoken = wait_for_advance(&practice, 0);
        assert_eq!(spoken.len(), 3);
        assert_eq!(spoken[0], "Speaking Swedish: Siffran noll");
        assert!((0..feedback_count(true)).any(|index| spoken[1]
            == format!("Speaking Swedish: {}", feedback_phrase(true, index).swedish)));
        assert!(
            (0..next_phrase_count())
                .any(|index| spoken[2]
                    == format!("Speaking Swedish: {}", next_phrase(index).swedish))
        );
        assert_eq!(practice.symbol.text(), "1");
        assert!(practice.entry.text().is_empty());

        practice.automatic.set_active(true);
        assert!(!practice.entry.is_editable());
        finish_playback(&practice);
        practice.entry.set_text("1");
        wait_for_advance(&practice, 1);
        assert_eq!(practice.symbol.text(), "2");
        assert_eq!(
            practice.speech_status.text(),
            "Speaking Swedish: Siffran två"
        );
        finish_playback(&practice);
        practice.automatic.set_active(false);
        ordered_set(&practice, 1);
        practice.next.emit_clicked();
        practice.entry.set_text("1");
        practice.auto_next.set_active(false);
        practice.replay.emit_clicked();
        finish_playback(&practice);
        assert_eq!(practice.lesson.borrow().position(), 1);
        assert_eq!(practice.entry.text(), "1");

        practice.auto_next.set_active(true);
        practice.next.emit_clicked();
        practice.entry.set_text("2");
        practice.next.emit_clicked();
        practice.replay.emit_clicked();
        finish_playback(&practice);
        assert_eq!(practice.lesson.borrow().position(), 3);

        practice.speak_typed.set_active(false);
        practice.entry.set_text("3");
        assert!(practice.next_timer.borrow().is_some());
        practice.auto_next.set_active(false);
        assert!(practice.next_timer.borrow().is_none());
        assert_eq!(practice.lesson.borrow().position(), 3);
        practice.next.emit_clicked();
        practice.auto_next.set_active(true);
        practice.entry.set_text("4");
        assert_eq!(practice.entry.text(), "4");
        wait_for_advance(&practice, 4);
        assert!(practice.entry.text().is_empty());

        practice.speak_typed.set_active(true);
        ordered_set(&practice, 1);
        for _ in 0..9 {
            practice.next.emit_clicked();
        }
        practice.entry.set_text("9");
        let spoken = wait_for_advance(&practice, 9);
        assert_eq!(
            spoken.len(),
            2,
            "The last card must not announce another card"
        );
        assert!(practice.lesson.borrow().current().is_none());
        assert!(!practice.entry.is_sensitive());
        practice.auto_next.set_active(false);

        ordered_set(&practice, 1);
        practice.auto_next.set_active(true);
        practice.entry.set_text("0");
        let deadline = Instant::now() + Duration::from_secs(3);
        while practice.speech_status.text() == "Speaking Swedish: Siffran noll" {
            assert!(Instant::now() < deadline);
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
        let interrupted = practice.speech_status.text();
        let voice = practice.follow_along.tooltip_text();
        practice.request_wait();
        let reminder = practice.speech_status.text();
        practice.request_wait();
        assert_eq!(practice.speech_status.text(), reminder);
        assert!(!practice.entry.is_editable());
        assert_eq!(practice.lesson.borrow().position(), 0);
        assert_eq!(practice.follow_along.tooltip_text(), voice);
        let spoken = wait_for_advance(&practice, 0);
        assert_eq!(spoken.len(), 3, "{spoken:?}");
        assert_eq!(spoken[0], reminder);
        assert_eq!(
            spoken[1], interrupted,
            "Restart the interrupted feedback, not the earlier number"
        );
        assert!(
            (0..next_phrase_count())
                .any(|index| spoken[2]
                    == format!("Speaking Swedish: {}", next_phrase(index).swedish))
        );
        assert!(practice.entry.is_editable());
        assert!(practice.entry.text().is_empty());
        practice.auto_next.set_active(false);

        ordered_set(&practice, 0);
        for voice_dir in &voice_dirs {
            fs::remove_file(voice_dir.join("Bokstaven A.wav")).unwrap();
        }
        practice.speak_typed.set_active(true);
        practice.auto_next.set_active(true);
        practice.entry.set_text("a");
        assert!(
            practice
                .speech_status
                .text()
                .contains("Speech audio is missing")
        );
        assert!(practice.feedback.text().contains("Correct!"));
        assert_eq!(
            practice.lesson.borrow().position(),
            0,
            "Speech failure must not advance"
        );
        assert!(practice.next_timer.borrow().is_none());
        ordered_set(&practice, 1);
        practice.entry.set_text("a");
        assert!(
            practice
                .speech_status
                .text()
                .contains("Speech audio is missing")
        );
        assert!(!practice.speaking.get());
        assert!(
            practice.entry.is_editable(),
            "Speech failure must release wrong-answer input"
        );
        practice.auto_next.set_active(false);
        if std::env::var_os("LEARN_SWEDISH_TEST_REAL_AUDIO").is_some() {
            for voice in ["sv_SE-nst-medium", "sv_SE-alma-medium"] {
                let real_speech = Rc::new(Speech::for_test(
                    vec![
                        glib::user_data_dir()
                            .join("learn-swedish")
                            .join("speech")
                            .join(voice),
                    ],
                    false,
                ));
                real_speech.speak_then(
                    vec![spoken_input("å").unwrap(), feedback_phrase(true, 15)],
                    &practice.speech_status,
                    &practice.follow_along,
                    |_| {},
                );
                let (spoken, highlighted) = finish_playback(&practice);
                assert_eq!(
                    spoken,
                    [
                        "Speaking Swedish: Bokstaven Å",
                        "Speaking Swedish: Fantastiskt!"
                    ]
                );
                assert!(!highlighted.is_empty());
                assert_eq!(
                    practice.follow_along.tooltip_text().unwrap(),
                    format!("Speech voice: {voice}")
                );
            }
        }
        crate::story_ui::test_flow(&app, &silent_wav());
        window.hide();
        assert_launcher_flow(&app, voice_dirs.clone());
        window.close();
        for voice_dir in &voice_dirs {
            for entry in fs::read_dir(voice_dir).unwrap() {
                fs::remove_file(entry.unwrap().path()).unwrap();
            }
            fs::remove_dir(voice_dir).unwrap();
        }
        fs::remove_dir(&directory).unwrap();
    }
}
