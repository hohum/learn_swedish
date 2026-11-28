use crate::speech::Speech;
use adw::{gio, glib, gtk, prelude::*};
use learn_swedish::adventure::{Book, Scene};
use learn_swedish::lesson::Phrase;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

struct Reader {
    book: RefCell<Option<Book>>,
    story_path: RefCell<Option<PathBuf>>,
    speech: Rc<Speech>,
    header_title: adw::WindowTitle,
    prev_page: gtk::Button,
    pages_menu: gtk::MenuButton,
    next_page: gtk::Button,
    #[cfg(test)]
    header_spinner: gtk::Spinner,
    #[cfg(test)]
    loading_spinner: gtk::Spinner,
    title: gtk::Label,
    illustration: gtk::DrawingArea,
    scene: Rc<Cell<Scene>>,
    transcript: gtk::Label,
    text_scroll: gtk::ScrolledWindow,
    choice_scroll: gtk::ScrolledWindow,
    choice_box: gtk::Box,
    choices: [gtk::Button; 8],
    entry: gtk::Entry,
    submit: gtk::Button,
    skip: gtk::Button,
    next: gtk::Button,
    status: gtk::Label,
    english: gtk::CheckButton,
    automatic: gtk::CheckButton,
    pause_sentences: gtk::CheckButton,
    replay: gtk::Button,
    restart: gtk::Button,
    speaking: Cell<bool>,
}

fn random_index(upper: usize) -> usize {
    glib::random_int_range(0, i32::try_from(upper).expect("Story choices fit in i32")) as usize
}

impl Reader {
    fn phrases(&self) -> Vec<Phrase> {
        let borrow = self.book.borrow();
        let Some(book) = borrow.as_ref() else {
            return Vec::new();
        };
        if let Some(selected) = book.selected {
            let choice = &book.page().choices[selected];
            vec![choice.line.phrase(), choice.reply.phrase()]
        } else {
            book.page().phrases()
        }
    }

    fn load_file(self: &Rc<Self>, path: &Path) -> Result<(), String> {
        let book = Book::load_file(path, random_index)?;
        self.apply_book(book, Some(path.to_path_buf()));
        Ok(())
    }

    fn apply_book(self: &Rc<Self>, book: Book, path: Option<PathBuf>) {
        self.speech.stop();
        self.speech.clear_last_sentence();
        self.header_title.set_title(&book.story().title);
        self.header_title.set_subtitle(&book.story().subtitle);
        self.speech.set_cast(&book.story().actors);
        self.story_path.replace(path);
        self.book.replace(Some(book));
        self.show();
    }

    fn controls(&self) {
        let borrow = self.book.borrow();
        let Some(book) = borrow.as_ref() else {
            self.prev_page.set_sensitive(false);
            self.pages_menu.set_sensitive(false);
            self.pages_menu.set_label("Pages");
            self.next_page.set_sensitive(false);
            self.entry.set_sensitive(false);
            self.submit.set_sensitive(false);
            self.skip.set_sensitive(false);
            self.next.set_sensitive(false);
            self.replay.set_sensitive(false);
            self.restart.set_sensitive(false);
            self.entry.set_visible(false);
            self.submit.set_visible(false);
            self.skip.set_visible(false);
            self.choice_scroll.set_visible(false);
            for choice in &self.choices {
                choice.set_visible(false);
                choice.set_sensitive(false);
            }
            return;
        };
        let page_total = book.pages.len();
        self.prev_page.set_sensitive(book.can_go_back());
        self.next_page.set_sensitive(book.can_go_forward());
        self.pages_menu.set_sensitive(true);
        self.pages_menu.set_label(&if book.finished {
            format!("End / {page_total}")
        } else {
            format!("{} / {page_total}", book.current_index() + 1)
        });
        let can_type = !self.speaking.get() && !book.finished;
        let can_pick_choice = !self.speech.is_loading() && !book.finished;
        let choice_count = book.page().choices.len();
        self.entry.set_sensitive(can_type);
        self.entry.set_placeholder_text(Some(&format!(
            "1–{choice_count} or the full Swedish response"
        )));
        self.submit.set_sensitive(can_type);
        self.entry.set_visible(!book.finished);
        self.submit.set_visible(!book.finished);
        self.skip.set_visible(!book.finished);
        self.choice_scroll.set_visible(!book.finished);
        for (index, choice) in self.choices.iter().enumerate() {
            choice.set_visible(index < choice_count);
            choice.set_sensitive(can_pick_choice && index < choice_count);
        }
        self.next
            .set_sensitive(!self.speaking.get() && (book.finished || book.selected.is_some()));
        self.skip.set_sensitive(!book.finished);
        self.replay.set_sensitive(!book.finished);
        self.restart.set_sensitive(true);
    }

    fn skip(self: &Rc<Self>) {
        let skipped = self.book.borrow_mut().as_mut().is_some_and(Book::skip);
        if skipped {
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
        let phrases = self.phrases();
        if phrases.is_empty() {
            return;
        }
        self.speech.stop();
        self.speaking.set(true);
        self.controls();
        let weak = Rc::downgrade(self);
        self.speech
            .speak_then(phrases, &self.status, &self.transcript, move |_| {
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
        self.render_view(true, true);
    }

    fn render_view(self: &Rc<Self>, reset_choice_scroll: bool, auto_speak: bool) {
        self.speech.stop();
        self.speech.clear_last_sentence();
        self.speaking.set(false);
        self.entry.set_text("");
        self.text_scroll.vadjustment().set_value(0.0);
        if reset_choice_scroll {
            self.choice_scroll.vadjustment().set_value(0.0);
        }
        let borrow = self.book.borrow();
        let Some(book) = borrow.as_ref() else {
            drop(borrow);
            self.scene.set(Scene::Home);
            self.illustration.queue_draw();
            self.header_title.set_title("Story Reader");
            self.header_title
                .set_subtitle("Choose a story from Stories");
            self.title
                .set_label("Ingen berättelse inläst · No story loaded");
            self.transcript.set_text(
                "Use Stories in the header bar to pick a bundled story or open an XML file.",
            );
            self.controls();
            self.status
                .set_label("No story XML loaded. Click Stories above to pick a story.");
            return;
        };
        self.header_title.set_title(&book.story().title);
        self.header_title.set_subtitle(&book.story().subtitle);
        self.scene.set(book.page().scene);
        self.illustration.queue_draw();
        let choice_count = book.page().choices.len();
        let selected_choice = book.selected;
        let selected_advances =
            selected_choice.is_some_and(|i| book.page().choices[i].next.is_some());
        if book.finished {
            let ending = &book.story().ending;
            self.title.set_label(&ending.title);
            self.speech
                .show_transcript(&[ending.line.phrase()], &self.transcript);
            if book.has_outing_hub() {
                self.next.set_label("Ny utflykt · New outing");
                self.status.set_label(
                    "You are home again. Choose New outing for another destination, or Stories for another book.",
                );
            } else {
                self.next.set_label("Börja om · Play again");
                self.status.set_label(
                    "Story complete. Choose Play again to restart, or Stories for another book.",
                );
            }
        } else {
            self.title.set_label(&book.page().title.swedish);
            self.next
                .set_label(if selected_choice.is_some() && !selected_advances {
                    "Försök igen · Try again"
                } else {
                    "Fortsätt · Continue"
                });
        }
        let finished = book.finished;
        drop(borrow);
        self.show_choices();
        self.controls();
        if !finished {
            self.speech
                .show_transcript(&self.phrases(), &self.transcript);
            if auto_speak && self.automatic.is_active() && self.speech.azure_enabled() {
                self.speak();
            } else if self.speech.azure_enabled() {
                self.status.set_label(&if selected_choice.is_some() {
                    if selected_advances {
                        "Click Continue to move on, or pick any other option to change your answer.".into()
                    } else {
                        "Pick any other option directly to change your answer, or click Try again to view the full page.".into()
                    }
                } else {
                    format!(
                        "Choose 1–{choice_count} or type a full Swedish response. Replay reads this page aloud."
                    )
                });
            } else {
                self.status.set_label(&if selected_choice.is_some() {
                    if selected_advances {
                        "Click Continue to move on, or pick any other option to change your answer. Open Azure Speech above for live audio.".into()
                    } else {
                        "Pick any other option directly to change your answer, or click Try again to view the full page.".into()
                    }
                } else {
                    format!(
                        "Choose 1–{choice_count} or type a full Swedish response. Open Azure Speech above for live audio."
                    )
                });
            }
        }
    }

    fn show_choices(&self) {
        let borrow = self.book.borrow();
        let Some(book) = borrow.as_ref() else {
            for button in &self.choices {
                button.set_visible(false);
                button.remove_css_class("selected-choice");
            }
            return;
        };
        let page_choices = &book.page().choices;
        let selected = book.selected;
        for (index, button) in self.choices.iter().enumerate() {
            let Some(choice) = page_choices.get(index) else {
                button.set_visible(false);
                button.remove_css_class("selected-choice");
                continue;
            };
            button.set_visible(true);
            let is_selected = selected == Some(index);
            if is_selected {
                button.add_css_class("selected-choice");
            } else {
                button.remove_css_class("selected-choice");
            }
            let label = button.child().unwrap().downcast::<gtk::Label>().unwrap();
            let prefix = if is_selected { "✓ " } else { "" };
            let text = format!("{prefix}{}. {}", index + 1, choice.line.swedish);
            if self.english.is_active() {
                let mut markup = glib::markup_escape_text(&text).to_string();
                if let Some(literal) = &choice.line.literal {
                    markup.push_str(&format!(
                        "\n<span size=\"small\" style=\"italic\">{}</span>",
                        glib::markup_escape_text(literal)
                    ));
                }
                if !choice.line.english.is_empty() {
                    markup.push_str(&format!(
                        "\n<span size=\"small\">{}</span>",
                        glib::markup_escape_text(&choice.line.english)
                    ));
                }
                label.set_markup(&markup);
            } else {
                label.set_text(&text);
            }
        }
    }

    fn answer(self: &Rc<Self>, input: &str) {
        if self.speech.is_loading() {
            return;
        }
        if self.speaking.get() {
            self.speech.stop();
            self.speaking.set(false);
        }
        let result = {
            let mut borrow = self.book.borrow_mut();
            let Some(book) = borrow.as_mut() else {
                return;
            };
            book.answer(input)
        };
        match result {
            Ok(_) => self.render_view(false, true),
            Err(error) => self.status.set_label(&error),
        }
    }

    fn update_speech_source(self: &Rc<Self>) {
        self.speech.stop();
        let has_active_page = self
            .book
            .borrow()
            .as_ref()
            .is_some_and(|book| !book.finished);
        if self.automatic.is_active() && self.speech.azure_enabled() && has_active_page {
            self.speak();
        } else {
            self.status.set_label(if self.speech.azure_enabled() {
                "Live Azure Speech is enabled. Use Replay to hear this page."
            } else {
                "Live Azure Speech is off. Story continues in text mode."
            });
        }
    }

    fn follow_choice(&self, active_phrase: Option<usize>) {
        let active_choice = {
            let borrow = self.book.borrow();
            if let Some(book) = borrow.as_ref()
                && book.selected.is_none()
                && !book.finished
            {
                let first_choice_phrase = 2 + book.page().setup.len();
                active_phrase
                    .and_then(|idx| idx.checked_sub(first_choice_phrase))
                    .filter(|&c| c < book.page().choices.len())
            } else {
                None
            }
        };
        for (index, button) in self.choices.iter().enumerate() {
            if Some(index) == active_choice {
                button.add_css_class("speaking-choice");
            } else {
                button.remove_css_class("speaking-choice");
            }
        }
        if let Some(choice_index) = active_choice {
            let button = &self.choices[choice_index];
            if !Self::scroll_choice_into_view(button, &self.choice_box, &self.choice_scroll) {
                let weak_btn = button.downgrade();
                let weak_box = self.choice_box.downgrade();
                let weak_scroll = self.choice_scroll.downgrade();
                glib::idle_add_local_once(move || {
                    if let (Some(btn), Some(cbox), Some(cscroll)) = (
                        weak_btn.upgrade(),
                        weak_box.upgrade(),
                        weak_scroll.upgrade(),
                    ) {
                        let _ = Self::scroll_choice_into_view(&btn, &cbox, &cscroll);
                    }
                });
            }
        }
    }

    fn scroll_choice_into_view(
        button: &gtk::Button,
        choice_box: &gtk::Box,
        choice_scroll: &gtk::ScrolledWindow,
    ) -> bool {
        let Some(bounds) = button.compute_bounds(choice_box) else {
            return false;
        };
        let adjustment = choice_scroll.vadjustment();
        let page_size = adjustment.page_size();
        let upper = adjustment.upper();
        let top = f64::from(bounds.y());
        let bottom = f64::from(bounds.y() + bounds.height());
        if page_size <= 1.0 || upper + 2.0 < bottom {
            return false;
        }
        if upper <= page_size + 1.0 {
            return true;
        }
        let max_scroll = (upper - page_size).max(0.0);
        let current = adjustment.value();
        if top < current + 8.0 {
            adjustment.set_value((top - 8.0).clamp(0.0, max_scroll));
        } else if bottom + 8.0 > current + page_size {
            adjustment.set_value((bottom - page_size + 12.0).clamp(0.0, max_scroll));
        }
        true
    }
}

pub fn open(
    app: &adw::Application,
    launcher: &adw::ApplicationWindow,
    speech: Rc<Speech>,
    story_file: Option<&Path>,
) {
    let (window, reader, startup_error) = build_with_story(app, speech, story_file);
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
    if let Some(error) = startup_error {
        reader.status.set_label(&error);
    }
}

fn build_with_story(
    app: &adw::Application,
    speech: Rc<Speech>,
    story_file: Option<&Path>,
) -> (adw::ApplicationWindow, Rc<Reader>, Option<String>) {
    let loaded = match story_file {
        Some(path) => Book::load_file(path, random_index).map(|book| (path.to_path_buf(), book)),
        None => Book::load_default(random_index),
    };
    let (initial_path, initial_book, startup_error) = match loaded {
        Ok((path, book)) => (Some(path), Some(book), None),
        Err(error) => (None, None, Some(error)),
    };
    let (window, reader) = build_ui(app, speech, initial_book, initial_path);
    (window, reader, startup_error)
}

#[cfg(test)]
fn build(app: &adw::Application, speech: Rc<Speech>) -> (adw::ApplicationWindow, Rc<Reader>) {
    let (window, reader, startup_error) = build_with_story(app, speech, None);
    assert!(startup_error.is_none(), "{startup_error:?}");
    (window, reader)
}

fn build_ui(
    app: &adw::Application,
    speech: Rc<Speech>,
    initial_book: Option<Book>,
    initial_path: Option<PathBuf>,
) -> (adw::ApplicationWindow, Rc<Reader>) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "button.speaking-choice { outline: 2px solid #e5a50a; background-color: alpha(#f6d32d, 0.18); }
         button.selected-choice { outline: 2px solid #3584e4; background-color: alpha(#3584e4, 0.14); }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    if let Some(book) = &initial_book {
        speech.set_cast(&book.story().actors);
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    let header_title = match &initial_book {
        Some(book) => adw::WindowTitle::new(&book.story().title, &book.story().subtitle),
        None => adw::WindowTitle::new("Story Reader", "Open a Story XML file"),
    };
    header.set_title_widget(Some(&header_title));
    let back = gtk::Button::with_label("Meny · Menu");
    back.set_widget_name("activity-menu");
    header.pack_start(&back);
    let stories_list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    stories_list.set_margin_top(6);
    stories_list.set_margin_bottom(6);
    stories_list.set_margin_start(6);
    stories_list.set_margin_end(6);
    let stories_popover = gtk::Popover::builder().child(&stories_list).build();
    let stories_menu = gtk::MenuButton::builder()
        .label("Stories")
        .popover(&stories_popover)
        .tooltip_text("Switch between available stories")
        .build();
    stories_menu.set_widget_name("stories-menu");
    header.pack_start(&stories_menu);

    let nav_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    nav_box.add_css_class("linked");
    let prev_page = gtk::Button::with_label("‹");
    prev_page.set_widget_name("nav-prev");
    prev_page.set_tooltip_text(Some("Previous page"));
    let pages_panel = gtk::Box::new(gtk::Orientation::Vertical, 6);
    pages_panel.set_margin_top(8);
    pages_panel.set_margin_bottom(8);
    pages_panel.set_margin_start(8);
    pages_panel.set_margin_end(8);
    let pages_search = gtk::SearchEntry::builder()
        .placeholder_text("Search title, scene, or ID…")
        .build();
    let pages_variants = gtk::CheckButton::with_label("Show all pool & encounter variants");
    let pages_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let pages_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(370)
        .min_content_height(310)
        .max_content_height(360)
        .propagate_natural_height(true)
        .child(&pages_list)
        .build();
    pages_panel.append(&pages_search);
    pages_panel.append(&pages_variants);
    pages_panel.append(&pages_scroll);
    let pages_popover = gtk::Popover::builder().child(&pages_panel).build();
    let pages_menu = gtk::MenuButton::builder()
        .label("Pages")
        .popover(&pages_popover)
        .tooltip_text("Page navigator — jump to any page")
        .build();
    pages_menu.set_widget_name("pages-menu");
    let next_page = gtk::Button::with_label("›");
    next_page.set_widget_name("nav-next");
    next_page.set_tooltip_text(Some("Next page"));
    nav_box.append(&prev_page);
    nav_box.append(&pages_menu);
    nav_box.append(&next_page);
    header.pack_start(&nav_box);
    let azure = gtk::Button::with_label("Azure Speech");
    azure.set_widget_name("azure-settings");
    azure.set_tooltip_text(Some("Set up live story audio"));
    header.pack_end(&azure);
    let header_spinner = gtk::Spinner::new();
    header_spinner.set_widget_name("azure-header-spinner");
    header_spinner.set_tooltip_text(Some("Loading Swedish audio from Azure…"));
    header_spinner.set_visible(false);
    header.pack_end(&header_spinner);
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
        .min_content_height(110)
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
        .min_content_height(110)
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
    english.set_tooltip_text(Some(
        "Show literal English in italics, followed by natural English",
    ));
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
    let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let loading_spinner = gtk::Spinner::new();
    loading_spinner.set_widget_name("azure-loading-spinner");
    loading_spinner.set_valign(gtk::Align::Center);
    loading_spinner.set_visible(false);
    let status = gtk::Label::new(None);
    status.set_wrap(true);
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.add_css_class("dim-label");
    status.set_lines(3);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.set_selectable(true);
    status.connect_label_notify(|label| label.set_tooltip_text(Some(&label.text())));
    status_row.append(&loading_spinner);
    status_row.append(&status);
    body.append(&status_row);
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
    speech.register_spinner(&header_spinner);
    speech.register_spinner(&loading_spinner);
    let reader = Rc::new(Reader {
        book: RefCell::new(initial_book),
        story_path: RefCell::new(initial_path),
        speech,
        header_title,
        prev_page: prev_page.clone(),
        pages_menu,
        next_page: next_page.clone(),
        #[cfg(test)]
        header_spinner,
        #[cfg(test)]
        loading_spinner,
        title,
        illustration,
        scene,
        transcript,
        text_scroll,
        choice_scroll,
        choice_box,
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
        restart: restart.clone(),
        speaking: Cell::new(false),
    });
    reader.speech.set_phrase_listener({
        let weak = Rc::downgrade(&reader);
        move |active_phrase| {
            if let Some(reader) = weak.upgrade() {
                reader.follow_choice(active_phrase);
            }
        }
    });
    stories_popover.connect_show(glib::clone!(
        #[weak]
        reader,
        #[weak]
        window,
        #[weak]
        stories_list,
        move |popover| populate_stories_popover(popover, &stories_list, &reader, &window)
    ));
    prev_page.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| {
            let moved = reader.book.borrow_mut().as_mut().is_some_and(Book::go_back);
            if moved {
                reader.show();
            }
        }
    ));
    next_page.connect_clicked(glib::clone!(
        #[weak]
        reader,
        move |_| {
            let moved = reader
                .book
                .borrow_mut()
                .as_mut()
                .is_some_and(Book::go_forward);
            if moved {
                reader.show();
            }
        }
    ));
    pages_popover.connect_show(glib::clone!(
        #[weak]
        reader,
        #[weak]
        pages_list,
        #[weak]
        pages_search,
        #[weak]
        pages_variants,
        move |popover| {
            populate_pages_popover(
                popover,
                &pages_list,
                &pages_search.text(),
                pages_variants.is_active(),
                &reader,
            );
            pages_search.grab_focus();
        }
    ));
    pages_search.connect_search_changed(glib::clone!(
        #[weak]
        reader,
        #[weak]
        pages_popover,
        #[weak]
        pages_list,
        #[weak]
        pages_variants,
        move |search| {
            populate_pages_popover(
                &pages_popover,
                &pages_list,
                &search.text(),
                pages_variants.is_active(),
                &reader,
            );
        }
    ));
    pages_variants.connect_toggled(glib::clone!(
        #[weak]
        reader,
        #[weak]
        pages_popover,
        #[weak]
        pages_list,
        #[weak]
        pages_search,
        move |toggle| {
            populate_pages_popover(
                &pages_popover,
                &pages_list,
                &pages_search.text(),
                toggle.is_active(),
                &reader,
            );
        }
    ));
    azure.connect_clicked(glib::clone!(
        #[weak]
        reader,
        #[weak]
        window,
        move |_| show_azure_settings(&reader, &window)
    ));
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
                && reader.book.borrow().as_ref().is_some_and(|b| !b.finished)
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
            enum Action {
                None,
                AdvancePage,
                RetryCurrentPage,
            }
            let action = {
                let mut borrow = reader.book.borrow_mut();
                if let Some(book) = borrow.as_mut() {
                    if book.finished {
                        book.new_outing(random_index);
                        reader.speech.recast_guests();
                        Action::AdvancePage
                    } else {
                        let was_wrong = book
                            .selected
                            .is_some_and(|idx| book.page().choices[idx].next.is_none());
                        if was_wrong {
                            book.clear_selection();
                            Action::RetryCurrentPage
                        } else {
                            book.continue_story();
                            Action::AdvancePage
                        }
                    }
                } else {
                    Action::None
                }
            };
            match action {
                Action::AdvancePage => reader.show(),
                Action::RetryCurrentPage => reader.render_view(false, false),
                Action::None => {}
            }
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
            let restarted = {
                let mut borrow = reader.book.borrow_mut();
                if let Some(book) = borrow.as_mut() {
                    book.restart(random_index);
                    true
                } else {
                    false
                }
            };
            if restarted {
                reader.speech.recast_guests();
                reader.show();
            }
        }
    ));
    reader.automatic.connect_toggled(glib::clone!(
        #[weak]
        reader,
        move |toggle| {
            let has_active_page = reader
                .book
                .borrow()
                .as_ref()
                .is_some_and(|book| !book.finished);
            if toggle.is_active() && has_active_page {
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
            reader.show_choices();
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

fn populate_pages_popover(
    popover: &gtk::Popover,
    list: &gtk::Box,
    query: &str,
    include_variants: bool,
    reader: &Rc<Reader>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let (entries, current_id) = {
        let borrow = reader.book.borrow();
        let Some(book) = borrow.as_ref() else {
            list.append(&gtk::Label::new(Some("No story loaded.")));
            return;
        };
        let current = (!book.finished).then(|| book.page().id.clone());
        (book.navigator_entries(include_variants), current)
    };
    let needle = query.trim().to_lowercase();
    let mut shown = 0usize;
    for entry in entries {
        let scene_name = entry.scene.as_str();
        if !needle.is_empty()
            && !entry.title_sv.to_lowercase().contains(&needle)
            && !entry.title_en.to_lowercase().contains(&needle)
            && !entry.id.to_lowercase().contains(&needle)
            && !scene_name.contains(&needle)
        {
            continue;
        }
        let is_current = current_id.as_deref() == Some(entry.id.as_str());
        let marker = if is_current {
            "✓ "
        } else if entry.visited {
            "• "
        } else {
            "   "
        };
        let number = entry
            .active_index
            .map_or_else(|| "var".into(), |idx| (idx + 1).to_string());
        let subtitle = if entry.title_en.is_empty() {
            entry.id.clone()
        } else {
            format!("{} · {}", entry.title_en, entry.id)
        };
        let markup = format!(
            "<b>{marker}{number}. {}</b> <span size=\"small\" alpha=\"70%\">[{scene_name}]</span>\n<span size=\"small\" alpha=\"75%\">{}</span>",
            glib::markup_escape_text(&entry.title_sv),
            glib::markup_escape_text(&subtitle)
        );
        let label = gtk::Label::new(None);
        label.set_markup(&markup);
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_max_width_chars(46);
        let button = gtk::Button::builder().child(&label).build();
        button.add_css_class("flat");
        let target_id = entry.id;
        button.connect_clicked(glib::clone!(
            #[weak]
            popover,
            #[weak]
            reader,
            move |_| {
                popover.popdown();
                let jumped = reader
                    .book
                    .borrow_mut()
                    .as_mut()
                    .is_some_and(|book| book.jump_to_id(&target_id, random_index));
                if jumped {
                    reader.show();
                }
            }
        ));
        list.append(&button);
        shown += 1;
    }
    if shown == 0 {
        let empty = gtk::Label::new(Some("No matching pages."));
        empty.add_css_class("dim-label");
        list.append(&empty);
    }
}

fn populate_stories_popover(
    popover: &gtk::Popover,
    list: &gtk::Box,
    reader: &Rc<Reader>,
    window: &adw::ApplicationWindow,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let active_canonical = reader
        .story_path
        .borrow()
        .as_ref()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()));
    for entry in learn_swedish::adventure::discover_catalog() {
        let is_active = active_canonical.as_ref().is_some_and(|active| {
            let candidate =
                std::fs::canonicalize(&entry.path).unwrap_or_else(|_| entry.path.clone());
            active == &candidate
        });
        let prefix = if is_active { "✓ " } else { "   " };
        let markup = format!(
            "<b>{}{}</b>\n<span size=\"small\" alpha=\"75%\">{}</span>",
            prefix,
            glib::markup_escape_text(&entry.title),
            glib::markup_escape_text(&entry.subtitle)
        );
        let label = gtk::Label::new(None);
        label.set_markup(&markup);
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_max_width_chars(42);
        let button = gtk::Button::builder().child(&label).build();
        button.add_css_class("flat");
        let path = entry.path;
        button.connect_clicked(glib::clone!(
            #[weak]
            popover,
            #[weak]
            reader,
            move |_| {
                popover.popdown();
                if let Err(error) = reader.load_file(&path) {
                    reader.status.set_label(&error);
                }
            }
        ));
        list.append(&button);
    }
    list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let browse_label = gtk::Label::new(Some("Open external XML file…"));
    browse_label.set_xalign(0.0);
    let browse = gtk::Button::builder().child(&browse_label).build();
    browse.add_css_class("flat");
    browse.connect_clicked(glib::clone!(
        #[weak]
        popover,
        #[weak]
        reader,
        #[weak]
        window,
        move |_| {
            popover.popdown();
            prompt_open_story(&reader, &window);
        }
    ));
    list.append(&browse);
}

fn prompt_open_story(reader: &Rc<Reader>, window: &adw::ApplicationWindow) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Story XML files (*.xml)"));
    filter.add_pattern("*.xml");
    let dialog = gtk::FileChooserNative::new(
        Some("Open Story XML"),
        Some(window),
        gtk::FileChooserAction::Open,
        Some("Open"),
        Some("Cancel"),
    );
    dialog.set_modal(true);
    dialog.add_filter(&filter);
    if let Some(folder) = reader
        .story_path
        .borrow()
        .as_ref()
        .and_then(|path| path.parent())
        .filter(|dir| dir.is_dir())
    {
        let _ = dialog.set_current_folder(Some(&gio::File::for_path(folder)));
    }
    let keep_alive = Rc::new(RefCell::new(Some(dialog.clone())));
    dialog.connect_response(glib::clone!(
        #[weak]
        reader,
        move |dialog, response| {
            let _held = keep_alive.borrow_mut().take();
            let selected = (response == gtk::ResponseType::Accept)
                .then(|| dialog.file())
                .flatten();
            dialog.destroy();
            let Some(file) = selected else {
                if response == gtk::ResponseType::Accept {
                    reader.status.set_label("No story XML file was selected.");
                }
                return;
            };
            let Some(path) = file.path() else {
                reader
                    .status
                    .set_label("Selected story XML does not have a local file path.");
                return;
            };
            if let Err(error) = reader.load_file(&path) {
                reader.status.set_label(&error);
            }
        }
    ));
    dialog.show();
}

fn show_azure_settings(reader: &Rc<Reader>, window: &adw::ApplicationWindow) {
    let live = reader.speech.azure_enabled();
    let remembered = reader.speech.azure_remembered();
    let has_key = reader.speech.azure_has_key();
    let dialog = gtk::Window::builder()
        .title("Live Azure Speech")
        .transient_for(window)
        .modal(true)
        .resizable(false)
        .default_width(430)
        .build();
    let fields = gtk::Box::new(gtk::Orientation::Vertical, 8);
    fields.set_margin_top(16);
    fields.set_margin_bottom(16);
    fields.set_margin_start(16);
    fields.set_margin_end(16);
    let explanation = gtk::Label::new(Some(
        "Azure receives the authored Swedish lines you play. Remembered keys are saved locally and enabled automatically at startup.",
    ));
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);
    fields.append(&explanation);
    let region = gtk::Entry::builder()
        .placeholder_text("Azure resource region")
        .text(reader.speech.azure_region())
        .build();
    let key = gtk::PasswordEntry::builder()
        .placeholder_text(if has_key {
            "Leave blank to keep saved/current key"
        } else {
            "Azure Speech F0 / resource key"
        })
        .build();
    let region_label = gtk::Label::new(Some("Resource region"));
    let key_label = gtk::Label::new(Some("Speech key"));
    for label in [&region_label, &key_label] {
        label.set_xalign(0.0);
    }
    fields.append(&region_label);
    fields.append(&region);
    fields.append(&key_label);
    fields.append(&key);

    let remember_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let remember = gtk::CheckButton::with_label("Remember key on this computer");
    remember.set_active(remembered || !live);
    remember.set_hexpand(true);
    remember_row.append(&remember);
    if remembered {
        let forget = gtk::Button::with_label("Forget saved key");
        forget.connect_clicked(glib::clone!(
            #[weak]
            reader,
            #[weak]
            dialog,
            move |_| {
                reader.speech.stop();
                reader.speech.forget_azure();
                dialog.close();
                reader
                    .status
                    .set_label("Saved Azure Speech key forgotten. Story continues in text mode.");
            }
        ));
        remember_row.append(&forget);
    }
    fields.append(&remember_row);

    let cached_clips = reader.speech.azure_cached_clips();
    let cache_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let cache_label = gtk::Label::new(Some(&format!("Cached TTS clips: {cached_clips}")));
    cache_label.set_xalign(0.0);
    cache_label.set_hexpand(true);
    cache_label.add_css_class("dim-label");
    let clear_cache = gtk::Button::with_label("Clear TTS cache");
    clear_cache.set_sensitive(cached_clips > 0);
    clear_cache.connect_clicked(glib::clone!(
        #[weak]
        reader,
        #[weak]
        cache_label,
        #[weak]
        clear_cache,
        move |_| {
            reader.speech.clear_azure_cache();
            cache_label.set_label("Cached TTS clips: 0");
            clear_cache.set_sensitive(false);
        }
    ));
    cache_row.append(&cache_label);
    cache_row.append(&clear_cache);
    fields.append(&cache_row);

    let error = gtk::Label::new(None);
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.add_css_class("error");
    error.set_visible(false);
    fields.append(&error);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    actions.append(&cancel);
    if live {
        let disable = gtk::Button::with_label("Turn off speech");
        actions.append(&disable);
        disable.connect_clicked(glib::clone!(
            #[weak]
            reader,
            #[weak]
            dialog,
            move |_| {
                reader.speech.disable_azure();
                dialog.close();
                reader.update_speech_source();
            }
        ));
    }
    let connect = gtk::Button::with_label("Use live speech");
    connect.add_css_class("suggested-action");
    actions.append(&connect);
    fields.append(&actions);
    dialog.set_child(Some(&fields));
    dialog.set_default_widget(Some(&connect));
    cancel.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| dialog.close()
    ));
    connect.connect_clicked(glib::clone!(
        #[weak]
        reader,
        #[weak]
        dialog,
        move |_| {
            if let Err(message) = reader.speech.configure_azure_persistent(
                &region.text(),
                &key.text(),
                remember.is_active(),
            ) {
                error.set_label(message);
                error.set_visible(true);
                return;
            }
            key.set_text("");
            dialog.close();
            reader.update_speech_source();
        }
    ));
    dialog.present();
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
pub fn test_flow(app: &adw::Application) {
    use std::collections::HashSet;
    use std::fs;
    use std::time::{Duration, Instant};

    const MOCK_WORKER: &str = r#"
import json, sys, time, wave
from pathlib import Path
output = Path(sys.argv[1])
request = json.load(sys.stdin.buffer)
assert len(sys.argv) == 2 and request["key"] == "test-key"
time.sleep(0.15)
for i, line in enumerate(request["lines"]):
    with wave.open(str(output / f"{i}.wav"), "wb") as wav:
        wav.setparams((1, 2, 24000, 0, "NONE", "not compressed"))
        wav.writeframes(bytes(2 * 12000))
    words = line["text"].split()
    timings = [
        {"word": word, "start_us": n * 500000 // len(words), "end_us": (n + 1) * 500000 // len(words)}
        for n, word in enumerate(words)
    ]
    starts = [0] + [
        timings[idx]["start_us"]
        for idx in range(1, len(words))
        if words[idx - 1].rstrip("\"'.)]»”’") != words[idx - 1] and words[idx - 1].rstrip("\"')]»”’").endswith((".", "!", "?"))
    ]
    metadata = {"duration_us": 500000, "sentence_starts_us": starts, "words": timings}
    (output / f"{i}.json").write_text(json.dumps(metadata), encoding="utf-8")
print("ok")
"#;

    // 1. Verify runtime discovery loads external XML from stories/ and fits within 900×600.
    assert!(!learn_swedish::adventure::discover_catalog().is_empty());
    let (default_win, default_reader) = build(app, Rc::new(Speech::for_live_test(MOCK_WORKER)));
    assert!(default_reader.story_path.borrow().is_some());
    assert!(default_reader.book.borrow().is_some());
    default_win.present();
    default_reader.show();
    let context = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    while (default_win.width() == 0 || default_win.height() == 0) && Instant::now() < deadline {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    let settle = Instant::now() + Duration::from_millis(100);
    while Instant::now() < settle {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        default_win.width() > 0
            && default_win.height() > 0
            && default_win.width() <= 900
            && default_win.height() <= 600,
        "Story window size invalid or exceeded 900×600: {}×{}",
        default_win.width(),
        default_win.height()
    );
    for (name, widget) in [
        ("entry", default_reader.entry.upcast_ref::<gtk::Widget>()),
        ("skip", default_reader.skip.upcast_ref()),
        ("next", default_reader.next.upcast_ref()),
        ("automatic", default_reader.automatic.upcast_ref()),
        (
            "pause_sentences",
            default_reader.pause_sentences.upcast_ref(),
        ),
        ("english", default_reader.english.upcast_ref()),
        ("illustration", default_reader.illustration.upcast_ref()),
    ] {
        let bounds = widget.compute_bounds(&default_win).unwrap();
        assert!(
            bounds.y() >= 0.0 && bounds.y() + bounds.height() <= default_win.height() as f32,
            "Widget {name} out of bounds: {bounds:?} in {}×{}",
            default_win.width(),
            default_win.height()
        );
    }
    default_win.close();

    // 2. Verify unloaded state when no XML file is loaded yet, then load a story XML file at runtime.
    let missing_file = std::env::temp_dir().join("nonexistent-story-xml-test.xml");
    let (window, reader, startup_error) = build_with_story(
        app,
        Rc::new(Speech::for_live_test(MOCK_WORKER)),
        Some(&missing_file),
    );
    assert!(startup_error.is_some());
    assert!(reader.book.borrow().is_none());
    window.present();
    reader.show();
    assert!(!reader.entry.is_sensitive());
    assert!(!reader.restart.is_sensitive());

    let primary_story_file = std::env::temp_dir().join(format!(
        "swedish-reader-flow-{}.xml",
        glib::uuid_string_random()
    ));
    fs::write(
        &primary_story_file,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<story version="1.0" id="reader-flow" start="step-one" outing-hub="step-hub">
  <meta>
    <title> stationen &amp; torget </title>
    <subtitle>Loaded from XML</subtitle>
  </meta>
  <cast protagonist="you" responder="companion" narrator="narrator">
    <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
    <actor id="you" label="Du" role="lead" voices="mattias"/>
    <actor id="companion" label="Vännen" role="lead" voices="sofie"/>
  </cast>
  <ending title="Resan klar · Slut">
    <line actor="companion">
      <sv>Vi ses nästa gång!</sv>
      <lit>We see-each-other next time!</lit>
      <en>See you next time!</en>
    </line>
  </ending>
  <pages>
    <page id="step-one" scene="home">
      <title>
        <sv>Första kapitlet</sv>
        <lit>First chapter-the</lit>
        <en>Chapter One</en>
      </title>
      <setup>
        <line actor="narrator">
          <sv>Klockan är nio. Ni står i hallen med ryggsäcken.</sv>
          <lit>Clock-the is nine. You stand in hall-the with backpack-the.</lit>
          <en>It is nine o'clock. You are standing in the hall with the backpack.</en>
        </line>
      </setup>
      <question>
        <sv>Har du biljetterna?</sv>
        <lit>Have you tickets-the?</lit>
        <en>Do you have the tickets?</en>
      </question>
      <choices>
        <choice next="step-two">
          <say>
            <sv>Ja, de ligger i fickan.</sv>
            <lit>Yes, they lie in pocket-the.</lit>
            <en>Yes, they are in my pocket.</en>
          </say>
          <reply>
            <sv>Toppen, då går vi.</sv>
            <lit>Great, then go we.</lit>
            <en>Great, let's go then.</en>
          </reply>
        </choice>
        <choice>
          <say>
            <sv>Jag åt upp dem till frukost.</sv>
            <lit>I ate up them for breakfast.</lit>
            <en>I ate them for breakfast.</en>
          </say>
          <reply>
            <sv>Papper mättar väldigt dåligt.</sv>
            <lit>Paper fills very poorly.</lit>
            <en>Paper is not very filling.</en>
          </reply>
        </choice>
        <choice>
          <say>
            <sv>Vi tar luftballongen i stället.</sv>
            <lit>We take hot-air-balloon-the instead.</lit>
            <en>Let's take the hot-air balloon instead.</en>
          </say>
          <reply>
            <sv>Det blåser lite för mycket i dag.</sv>
            <lit>It blows little too much in day.</lit>
            <en>It is a bit too windy today.</en>
          </reply>
        </choice>
        <choice>
          <say>
            <sv>Hunden lånade dem.</sv>
            <lit>Dog-the borrowed them.</lit>
            <en>The dog borrowed them.</en>
          </say>
          <reply>
            <sv>Vi har ingen hund.</sv>
            <lit>We have no dog.</lit>
            <en>We don't have a dog.</en>
          </reply>
        </choice>
      </choices>
    </page>
    <page id="step-two" scene="train">
      <title><sv>Vid spåret</sv><lit>By track-the</lit><en>By the platform</en></title>
      <question>
        <sv>Vilken vagn ska vi ta?</sv>
        <lit>Which carriage shall we take?</lit>
        <en>Which carriage should we take?</en>
      </question>
      <choices>
        <choice next="step-hub">
          <say><sv>Vagn tre här framme.</sv><lit>Carriage three here in-front.</lit><en>Carriage three up ahead.</en></say>
          <reply><sv>Där finns lediga platser.</sv><lit>There exist free seats.</lit><en>There are free seats there.</en></reply>
        </choice>
        <choice>
          <say><sv>Vi sitter på taket.</sv><lit>We sit on roof-the.</lit><en>Let's sit on the roof.</en></say>
          <reply><sv>Kontaktledningen säger nej.</sv><lit>Overhead-wire-the says no.</lit><en>The overhead wire says no.</en></reply>
        </choice>
      </choices>
    </page>
    <page id="step-hub" scene="city">
      <title><sv>Framme på torget</sv><lit>Arrived on square-the</lit><en>At the square</en></title>
      <question>
        <sv>Vart går vi nu?</sv>
        <lit>Where-to go we now?</lit>
        <en>Where shall we go now?</en>
      </question>
      <choices>
        <choice next="step-market">
          <say><sv>Vi kollar bokhandeln.</sv><lit>We check bookshop-the.</lit><en>Let's check out the bookshop.</en></say>
          <reply><sv>Bra idé!</sv><lit>Good idea!</lit><en>Good idea!</en></reply>
        </choice>
        <choice next="done">
          <say><sv>Vi åker hem direkt.</sv><lit>We travel home directly.</lit><en>Let's head straight home.</en></say>
          <reply><sv>Hem ljuva hem.</sv><lit>Home sweet home.</lit><en>Home sweet home.</en></reply>
        </choice>
      </choices>
    </page>
    <page id="step-market" scene="shop">
      <title><sv>I bokhandeln</sv><lit>In bookshop-the</lit><en>In the bookshop</en></title>
      <question>
        <sv>Hittade du en bok?</sv>
        <lit>Found you a book?</lit>
        <en>Did you find a book?</en>
      </question>
      <choices>
        <choice next="done">
          <say><sv>Ja, en svensk roman.</sv><lit>Yes, a Swedish novel.</lit><en>Yes, a Swedish novel.</en></say>
          <reply><sv>Perfekt kvällsläsning.</sv><lit>Perfect evening-reading.</lit><en>Perfect evening reading.</en></reply>
        </choice>
        <choice>
          <say><sv>Jag köpte bokhyllan.</sv><lit>I bought bookshelf-the.</lit><en>I bought the bookshelf.</en></say>
          <reply><sv>Den tillhör butiken.</sv><lit>It belongs-to shop-the.</lit><en>That belongs to the shop.</en></reply>
        </choice>
      </choices>
    </page>
  </pages>
</story>"#,
    )
    .unwrap();
    reader.load_file(&primary_story_file).unwrap();
    assert!(reader.pause_sentences.is_active());
    assert!(reader.automatic.is_active());
    assert!(!reader.speaking.get());
    assert!(reader.entry.is_sensitive());
    assert!(reader.status.text().contains("Azure Speech"));
    reader.replay.emit_clicked();
    assert!(!reader.speaking.get());
    assert!(reader.entry.is_sensitive());
    assert!(reader.status.text().contains("Azure Speech"));

    reader.automatic.set_active(false);
    assert!(!reader.transcript.text().contains("It is nine o'clock"));
    reader.english.set_active(true);
    assert!(reader.transcript.text().contains(
        "Klockan är nio. Ni står i hallen med ryggsäcken.\n\
         Clock-the is nine. You stand in hall-the with backpack-the.\n\
         It is nine o'clock. You are standing in the hall with the backpack."
    ));
    let first_choice = reader.choices[0]
        .child()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert_eq!(first_choice.text().lines().count(), 3);
    reader.english.set_active(false);
    assert_eq!(first_choice.text().lines().count(), 1);

    let window_keys = (0..window.observe_controllers().n_items())
        .filter_map(|index| window.observe_controllers().item(index))
        .filter_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
        .find(|controller| controller.propagation_phase() == gtk::PropagationPhase::Capture)
        .unwrap();
    let wait_until_paused = |reader: &Reader| {
        let deadline = Instant::now() + Duration::from_secs(6);
        let mut highlighted = false;
        while !reader.speech.is_paused() {
            assert!(
                Instant::now() < deadline,
                "Story speech did not pause: {}",
                reader.status.text()
            );
            context.iteration(false);
            highlighted |= reader.transcript.label().contains("background=");
            std::thread::sleep(Duration::from_millis(5));
        }
        highlighted
    };

    reader
        .speech
        .configure_azure("australiaeast", "test-key")
        .unwrap();
    assert!(!reader.loading_spinner.is_visible());
    assert!(!reader.loading_spinner.is_spinning());
    reader.speak();
    assert!(
        reader
            .status
            .text()
            .contains("Getting Swedish audio from Azure")
    );
    assert!(reader.loading_spinner.is_visible());
    assert!(reader.loading_spinner.is_spinning());
    assert!(reader.header_spinner.is_visible());
    assert!(reader.header_spinner.is_spinning());
    assert!(!reader.entry.is_sensitive());
    assert!(reader.skip.is_sensitive());
    assert!(wait_until_paused(&reader));
    assert!(!reader.loading_spinner.is_visible());
    assert!(!reader.loading_spinner.is_spinning());
    assert!(!reader.header_spinner.is_visible());
    assert!(!reader.header_spinner.is_spinning());
    assert_eq!(
        reader.transcript.tooltip_text().as_deref(),
        Some("Speech voice: sv-SE-MattiasNeural · Azure live")
    );
    assert!(reader.status.text().contains("Paused after sentence"));

    // Replay must use the session cache immediately, without launching another worker.
    reader.speech.stop();
    reader.speak();
    assert!(!reader.loading_spinner.is_spinning());
    assert!(
        reader
            .status
            .text()
            .starts_with("Speaking Swedish: Första kapitlet")
    );
    wait_until_paused(&reader);
    assert!(window_keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::space,
            &0_u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    assert!(reader.status.text().contains("Klockan är nio."));
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
        voices.contains("Speech voice: sv-SE-SofieNeural · Azure live")
            && voices.contains("Speech voice: sv-SE-MattiasNeural · Azure live")
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
        .as_ref()
        .unwrap()
        .page()
        .choices
        .iter()
        .position(|choice| choice.next.is_none())
        .unwrap();
    reader.choices[wrong].emit_clicked();
    assert!(reader.next.is_sensitive());
    assert!(
        reader.entry.is_sensitive(),
        "Entry and choice buttons stay unlocked so the user can change their answer directly"
    );
    assert!(reader.choices[wrong].has_css_class("selected-choice"));
    assert!(reader.transcript.text().contains("Vännen"));
    // Clicking Try again restores the full page silently without re-speaking the whole page.
    reader.next.emit_clicked();
    assert_eq!(reader.book.borrow().as_ref().unwrap().page().id, "step-one");
    assert!(!reader.speaking.get());
    // Changing answer directly from wrong -> right without clicking Try again:
    reader.choices[wrong].emit_clicked();
    let right = reader
        .book
        .borrow()
        .as_ref()
        .unwrap()
        .page()
        .choices
        .iter()
        .find(|choice| choice.next.is_some())
        .unwrap()
        .line
        .swedish
        .clone();
    reader.entry.set_text(&right);
    reader.entry.emit_activate();
    assert_eq!(reader.next.label().as_deref(), Some("Fortsätt · Continue"));
    reader.next.emit_clicked();
    assert_eq!(reader.book.borrow().as_ref().unwrap().page().id, "step-two");
    // Page navigator Back & Forward buttons jump cleanly between pages:
    assert!(reader.prev_page.is_sensitive());
    reader.prev_page.emit_clicked();
    assert_eq!(reader.book.borrow().as_ref().unwrap().page().id, "step-one");
    assert!(reader.next_page.is_sensitive());
    reader.next_page.emit_clicked();
    assert_eq!(reader.book.borrow().as_ref().unwrap().page().id, "step-two");
    reader.speak();
    assert!(
        reader
            .status
            .text()
            .contains("Getting Swedish audio from Azure")
    );
    assert!(reader.loading_spinner.is_spinning());
    reader.skip.emit_clicked();
    assert!(!reader.loading_spinner.is_spinning());
    assert!(!reader.loading_spinner.is_visible());
    assert_eq!(reader.book.borrow().as_ref().unwrap().page().id, "step-hub");
    assert!(reader.entry.is_sensitive());
    reader.speech.disable_azure();
    reader.automatic.set_active(true);
    reader.show();
    assert!(
        !reader.speaking.get(),
        "Disabling live speech must keep story controls unlocked in text mode"
    );
    assert!(reader.entry.is_sensitive());
    reader.automatic.set_active(false);
    reader.answer("Vi kollar bokhandeln.");
    reader.next.emit_clicked();
    assert_eq!(
        reader.book.borrow().as_ref().unwrap().page().id,
        "step-market"
    );
    while !reader.book.borrow().as_ref().unwrap().finished {
        reader.skip.emit_clicked();
    }
    assert_eq!(
        reader.next.label().as_deref(),
        Some("Ny utflykt · New outing")
    );
    reader.next.emit_clicked();
    assert_eq!(reader.book.borrow().as_ref().unwrap().page().id, "step-hub");
    let _ = fs::remove_file(&primary_story_file);

    // 3. Verify switching to another external custom XML file at runtime and rejecting broken XML.
    let custom_file = std::env::temp_dir().join(format!(
        "swedish-custom-story-{}.xml",
        glib::uuid_string_random()
    ));
    fs::write(
        &custom_file,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<story version="1.0" id="custom-cafe">
  <meta>
    <title>Kafébesöket</title>
    <subtitle>External XML test</subtitle>
  </meta>
  <cast protagonist="you" responder="staff" narrator="narrator">
    <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
    <actor id="you" label="Du" role="lead" voices="mattias"/>
    <actor id="staff" label="Kafépersonalen" role="guest" voices="hillevi"/>
  </cast>
  <ending title="Mätt och belåten">
    <line actor="staff">
      <sv>Välkommen åter!</sv>
      <en>Welcome back!</en>
    </line>
  </ending>
  <pages>
    <page id="start" scene="city">
      <title><sv>Vid disken</sv><en>At the counter</en></title>
      <question><sv>En kaffe?</sv><en>A coffee?</en></question>
      <choices>
        <choice next="done">
          <say><sv>Ja tack!</sv><en>Yes please!</en></say>
          <reply><sv>Varsågod.</sv><en>Here you go.</en></reply>
        </choice>
        <choice>
          <say><sv>En skruvmejsel, tack.</sv><en>A screwdriver, please.</en></say>
          <reply><sv>Fel butik!</sv><en>Wrong shop!</en></reply>
        </choice>
      </choices>
    </page>
  </pages>
</story>"#,
    )
    .unwrap();
    reader.load_file(&custom_file).unwrap();
    assert_eq!(reader.header_title.title().as_str(), "Kafébesöket");
    assert_eq!(reader.title.text().as_str(), "Vid disken");
    assert!(reader.choices[0].is_visible() && reader.choices[1].is_visible());
    assert!(!reader.choices[2].is_visible() && !reader.choices[3].is_visible());
    fs::write(&custom_file, "<story version=\"1.0\"><broken>").unwrap();
    assert!(reader.load_file(&custom_file).is_err());
    assert_eq!(reader.title.text().as_str(), "Vid disken");
    let _ = fs::remove_file(&custom_file);
    reader.answer("Ja tack!");
    reader.next.emit_clicked();
    assert!(reader.book.borrow().as_ref().unwrap().finished);
    assert_eq!(reader.title.text().as_str(), "Mätt och belåten");
    assert_eq!(
        reader.next.label().as_deref(),
        Some("Börja om · Play again")
    );
    window.close();

    // 4. Verify remembered Azure key auto-enable, persistent TTS disk cache across instances,
    //    and follow-along auto-scrolling for both transcript and choices.
    let persist_dir = std::env::temp_dir().join(format!(
        "swedish-persist-scroll-{}",
        glib::uuid_string_random()
    ));
    let creds_file = persist_dir.join("azure-speech.json");
    let cache_dir = persist_dir.join("tts-v1");
    let scroll_story_file = persist_dir.join("long-scroll.xml");
    fs::create_dir_all(&persist_dir).unwrap();
    fs::write(
        &scroll_story_file,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<story version="1.0" id="scroll-test">
  <meta>
    <title>Lång sida</title>
    <subtitle>Auto-scroll &amp; cache test</subtitle>
  </meta>
  <cast protagonist="you" responder="wife" narrator="narrator">
    <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
    <actor id="you" label="Du" role="lead" voices="mattias"/>
    <actor id="wife" label="Frun" role="lead" voices="sofie"/>
  </cast>
  <ending title="Slut">
    <line actor="wife"><sv>Klart!</sv><en>Done!</en></line>
  </ending>
  <pages>
    <page id="long" scene="home">
      <title><sv>En väldigt lång morgon i köket</sv><lit>A very long morning in kitchen-the</lit><en>A very long morning in the kitchen</en></title>
      <setup>
        <line actor="narrator"><sv>Första stycket handlar om kaffebryggaren som puttrar långsamt medan regnet smattrar mot köksfönstret hela morgonen.</sv><lit>First paragraph-the deals about coffee-brewer-the that simmers slowly while rain-the patters against kitchen-window-the whole morning-the.</lit><en>The first paragraph is about the coffee maker burbling slowly while the rain patters against the kitchen window all morning.</en></line>
        <line actor="wife"><sv>Andra stycket har ännu fler svenska meningar så att texten garanterat blir högre än fönstret på vänster sida.</sv><lit>Second paragraph-the has still more Swedish sentences so that text-the guaranteed becomes higher than window-the on left side.</lit><en>The second paragraph has even more Swedish sentences so the text is guaranteed to be taller than the left-hand pane.</en></line>
        <line actor="narrator"><sv>Tredje stycket fortsätter genom hallen förbi skohyllan, paraplystället, tvättkorgen och tre tunga matkassar vid dörren.</sv><lit>Third paragraph-the continues through hall-the past shoe-rack-the, umbrella-stand-the, laundry-basket-the and three heavy grocery-bags by door-the.</lit><en>The third paragraph continues through the hallway past the shoe rack, umbrella stand, laundry basket, and three heavy grocery bags by the door.</en></line>
        <line actor="wife"><sv>Fjärde stycket lägger till ännu mer dialog om nycklar, vantar, halsdukar, mössor och biljetter innan frågan ens kommer.</sv><lit>Fourth paragraph-the adds to still more dialogue about keys, mittens, scarves, hats and tickets before question-the even comes.</lit><en>The fourth paragraph adds still more dialogue about keys, mittens, scarves, hats, and tickets before the question even arrives.</en></line>
      </setup>
      <question><sv>Vad vill du göra med hela den här lördagen nu när kaffet äntligen är klart?</sv><lit>What want you do with whole this Saturday-the now when coffee-the finally is ready?</lit><en>What do you want to do with this whole Saturday now that the coffee is finally ready?</en></question>
      <choices>
        <choice next="done">
          <say><sv>Alternativ ett är en lång mening om att gå ut i skogen trots regnet och plocka svamp i korgen.</sv><lit>Option one is a long sentence about to go out in forest-the despite rain-the and pick mushrooms in basket-the.</lit><en>Option one is a long sentence about heading into the woods despite the rain to pick mushrooms in the basket.</en></say>
          <reply><sv>Bra val!</sv><en>Good choice!</en></reply>
        </choice>
        <choice>
          <say><sv>Alternativ två är en lång mening om att stanna inne vid köksbordet med påtår och kanelbullar.</sv><lit>Option two is a long sentence about to stay inside by kitchen-table-the with refill and cinnamon-buns.</lit><en>Option two is a long sentence about staying inside at the kitchen table with a refill and cinnamon buns.</en></say>
          <reply><sv>Mysigt men vi behöver frisk luft.</sv><en>Cosy, though we need fresh air.</en></reply>
        </choice>
        <choice>
          <say><sv>Alternativ tre är en lång mening om att köra till varuhuset och mäta tre bokhyllor på lagret.</sv><lit>Option three is a long sentence about to drive to department-store-the and measure three bookshelves in warehouse-the.</lit><en>Option three is a long sentence about driving to the store and measuring three bookshelves in the warehouse.</en></say>
          <reply><sv>Inte utan måttband.</sv><en>Not without a tape measure.</en></reply>
        </choice>
        <choice>
          <say><sv>Alternativ fyra är en lång mening om att gå ner i tvättstugan och boka en tid på den blå skärmen.</sv><lit>Option four is a long sentence about to go down in laundry-room-the and book a slot on the blue screen-the.</lit><en>Option four is a long sentence about heading down to the laundry room and booking a slot on the blue screen.</en></say>
          <reply><sv>Inte i dag!</sv><en>Not today!</en></reply>
        </choice>
        <choice>
          <say><sv>Alternativ fem är en lång mening om att ta tåget in till centrum och dela ett bord på kaféet.</sv><lit>Option five is a long sentence about to take train-the in to centre and share a table at café-the.</lit><en>Option five is a long sentence about taking the train into town and sharing a table at the café.</en></say>
          <reply><sv>Nästa helg kanske.</sv><en>Maybe next weekend.</en></reply>
        </choice>
        <choice>
          <say><sv>Alternativ sex är en lång mening längst ner i listan så att högerpanelen rullar ner automatiskt.</sv><lit>Option six is a long sentence furthest down in list-the so that right-panel-the scrolls down automatically.</lit><en>Option six is a long sentence at the very bottom of the list so the right-hand panel scrolls down automatically.</en></say>
          <reply><sv>Där nere var den!</sv><en>Down there it was!</en></reply>
        </choice>
      </choices>
    </page>
  </pages>
</story>"#,
    )
    .unwrap();

    let speech1 = Rc::new(Speech::for_live_test_with_paths(
        MOCK_WORKER,
        creds_file.clone(),
        cache_dir.clone(),
    ));
    speech1
        .configure_azure_persistent("australiaeast", "test-key", true)
        .unwrap();
    assert!(creds_file.is_file());
    let (win1, rdr1, err1) = build_with_story(app, speech1, Some(&scroll_story_file));
    assert!(err1.is_none());
    rdr1.automatic.set_active(false);
    rdr1.pause_sentences.set_active(false);
    rdr1.english.set_active(true);
    win1.present();
    rdr1.show();
    let settle = Instant::now() + Duration::from_millis(200);
    while Instant::now() < settle {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(rdr1.text_scroll.vadjustment().value(), 0.0);
    assert_eq!(rdr1.choice_scroll.vadjustment().value(), 0.0);
    rdr1.speak();
    let mut max_text_scroll = 0.0_f64;
    let mut max_choice_scroll = 0.0_f64;
    let mut saw_speaking_choice = false;
    let mut saw_literal_underscore = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    while rdr1.speaking.get() {
        assert!(Instant::now() < deadline, "{}", rdr1.status.text());
        context.iteration(false);
        max_text_scroll = max_text_scroll.max(rdr1.text_scroll.vadjustment().value());
        max_choice_scroll = max_choice_scroll.max(rdr1.choice_scroll.vadjustment().value());
        saw_speaking_choice |= rdr1
            .choices
            .iter()
            .any(|btn| btn.has_css_class("speaking-choice"));
        saw_literal_underscore |= rdr1.transcript.label().contains("underline=\"single\"");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        saw_literal_underscore,
        "Follow-along with English hints must underscore the literal English equivalent"
    );
    assert!(
        max_text_scroll > 0.0,
        "Transcript did not auto-scroll: upper={}, page_size={}",
        rdr1.text_scroll.vadjustment().upper(),
        rdr1.text_scroll.vadjustment().page_size()
    );
    assert!(
        max_choice_scroll > 0.0,
        "Choices did not auto-scroll: upper={}, page_size={}",
        rdr1.choice_scroll.vadjustment().upper(),
        rdr1.choice_scroll.vadjustment().page_size()
    );
    assert!(saw_speaking_choice);
    assert!(
        !rdr1
            .choices
            .iter()
            .any(|btn| btn.has_css_class("speaking-choice"))
    );
    assert!(rdr1.speech.azure_cached_clips() >= 12);
    win1.close();

    // New Speech instance with a worker that fails if invoked: must auto-enable from remembered
    // credentials AND play immediately from the persistent TTS disk cache without calling Python.
    const FAIL_IF_CALLED_WORKER: &str = "import sys; sys.exit(1)";
    let speech2 = Rc::new(Speech::for_live_test_with_paths(
        FAIL_IF_CALLED_WORKER,
        creds_file,
        cache_dir,
    ));
    assert!(
        speech2.azure_enabled() && speech2.azure_remembered(),
        "Remembered Azure key must default to enabled on startup"
    );
    let (win2, rdr2, err2) = build_with_story(app, speech2, Some(&scroll_story_file));
    assert!(err2.is_none());
    win2.present();
    rdr2.show();
    assert!(
        !rdr2.loading_spinner.is_spinning(),
        "Persistent TTS cache hit must start immediately without loading spinner"
    );
    assert!(
        rdr2.status.text().starts_with("Speaking Swedish:"),
        "Expected immediate playback from persistent TTS cache, got: {}",
        rdr2.status.text()
    );
    rdr2.speech.stop();
    win2.close();
    let _ = fs::remove_dir_all(&persist_dir);
}
