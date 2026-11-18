use adw::prelude::*;
use adw::{glib, gtk};
use gstreamer::{self as gst, prelude::*};
use learn_swedish::lesson::Phrase;
#[cfg(unix)]
use learn_swedish::lesson::speech_phrases;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs;
use std::io;
#[cfg(unix)]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

const VOICES: &[&str] = &["sv_SE-nst-medium", "sv_SE-alma-medium"];
const AZURE_VOICE: &str = "azure-sv-SE-SofieNeural";
const ADVENTURE_BANK: &str = "azure-adventure";

#[derive(Debug, Deserialize)]
struct WordTiming {
    word: String,
    start_us: i64,
    end_us: i64,
}

#[derive(Debug, Deserialize)]
struct Timing {
    duration_us: i64,
    words: Vec<WordTiming>,
}

impl Timing {
    fn load(path: &Path, text: &str) -> Result<Self, Box<dyn Error>> {
        let timing: Self = serde_json::from_slice(&fs::read(path)?)?;
        timing.validate(text)?;
        Ok(timing)
    }

    fn validate(&self, text: &str) -> io::Result<()> {
        let words: Vec<_> = text.split_whitespace().collect();
        let mut previous_end = 0;
        if self.duration_us <= 0 || words.len() != self.words.len() {
            return Err(io::Error::other(format!(
                "Invalid word timings for {text:?}"
            )));
        }
        for (expected, timing) in words.iter().zip(&self.words) {
            if *expected != timing.word
                || timing.start_us < previous_end
                || timing.end_us <= timing.start_us
                || timing.end_us > self.duration_us
            {
                return Err(io::Error::other(format!(
                    "Invalid word timings for {text:?}"
                )));
            }
            previous_end = timing.end_us;
        }
        Ok(())
    }

    fn word_at(&self, timestamp: i64) -> Option<usize> {
        self.words
            .iter()
            .position(|word| timestamp >= word.start_us && timestamp < word.end_us)
    }
}

pub fn transcript_markup(
    phrases: &[Phrase],
    active: Option<(usize, usize)>,
    show_english: bool,
) -> String {
    phrases.iter().enumerate().map(|(phrase_index, phrase)| {
        let swedish = phrase.swedish.split_whitespace().enumerate().map(|(word_index, word)| {
            let escaped = glib::markup_escape_text(word);
            if active == Some((phrase_index, word_index)) {
                format!("<span background=\"#f6d32d\" foreground=\"#1c1c1c\" weight=\"bold\">{escaped}</span>")
            } else {
                escaped.to_string()
            }
        }).collect::<Vec<_>>().join(" ");
        let spoken = if let Some(speaker) = phrase.speaker {
            format!("<span size=\"small\" weight=\"bold\">{}</span>\n{swedish}", glib::markup_escape_text(speaker))
        } else {
            swedish
        };
        if show_english && !phrase.english.is_empty() {
            format!("{spoken}\n<span size=\"small\">{}</span>", glib::markup_escape_text(&phrase.english))
        } else {
            spoken
        }
    }).collect::<Vec<_>>().join("\n\n")
}

struct Clip {
    phrase: Phrase,
    audio: PathBuf,
    timing: Timing,
}

struct Playback {
    clips: Vec<Clip>,
    phrases: Vec<Phrase>,
    current: usize,
    media: gst::Pipeline,
    loading_since: Instant,
    highlighted: Option<(usize, usize)>,
    status: gtk::Label,
    transcript: gtk::Label,
    completion: Option<Box<dyn FnOnce(bool)>>,
    reminder: bool,
}

struct Transcript {
    phrases: Vec<Phrase>,
    label: gtk::Label,
    active: Option<(usize, usize)>,
}

pub struct Speech {
    playback: RefCell<Option<Playback>>,
    timer: RefCell<Option<glib::SourceId>>,
    audio_dirs: Vec<PathBuf>,
    show_english: Cell<bool>,
    transcript: RefCell<Option<Transcript>>,
    story_dir: PathBuf,
    #[cfg(test)]
    muted: bool,
}

fn data_dir() -> PathBuf {
    #[cfg(windows)]
    if let Ok(exe) = std::env::current_exe()
        && let Some(bin) = exe.parent()
        && let Some(bundle) = bin.parent()
        && bundle.join("speech").is_dir()
    {
        return bundle.to_path_buf();
    }
    glib::user_data_dir().join("learn-swedish")
}

fn audio_directories(data: &Path) -> Vec<PathBuf> {
    let speech = data.join("speech");
    let azure = speech.join(AZURE_VOICE);
    if azure.exists() || azure.is_symlink() {
        vec![azure]
    } else {
        VOICES.iter().map(|voice| speech.join(voice)).collect()
    }
}

pub fn recording_stem(phrase: &Phrase) -> String {
    let stem = phrase.audio_stem();
    #[cfg(windows)]
    // Match Windows tar's replacement of invalid filename characters on import.
    let stem = stem.replace(['<', '>', ':', '"', '/', '\\', '|', '?', '*'], "_");
    stem
}

impl Default for Speech {
    fn default() -> Self {
        Self {
            playback: RefCell::new(None),
            timer: RefCell::new(None),
            audio_dirs: audio_directories(&data_dir()),
            show_english: Cell::new(false),
            transcript: RefCell::new(None),
            story_dir: data_dir().join("speech").join(ADVENTURE_BANK),
            #[cfg(test)]
            muted: false,
        }
    }
}

impl Drop for Speech {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(unix)]
pub fn prepare_azure_speech() -> Result<(), Box<dyn Error>> {
    let data = data_dir();
    let python = data.join("azure-env").join("bin").join("python");
    if !python.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Azure's preparation environment is missing. Follow README's Azure speech setup.",
        )
        .into());
    }
    let parent = data.join("speech");
    fs::create_dir_all(&parent)?;
    let staging = parent.join(format!("azure-generation-{}", glib::uuid_string_random()));
    fs::create_dir(&staging)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        let phrases = speech_phrases();
        let texts = reuse_azure_audio(&parent.join(AZURE_VOICE), &staging, &phrases)?;
        println!(
            "Reusing {} existing Azure recordings.",
            phrases.len() - texts.len()
        );
        if !texts.is_empty() {
            let status = Command::new(python)
                .args([
                    "-B",
                    "-c",
                    include_str!("../scripts/prepare_azure_speech.py"),
                ])
                .arg(&staging)
                .arg(serde_json::to_string(&texts)?)
                .stdin(Stdio::inherit())
                .status()?;
            if !status.success() {
                return Err(io::Error::other(format!(
                    "Azure preparation failed ({status}); the active audio library is unchanged."
                ))
                .into());
            }
        }
        validate_audio(&staging, &phrases)?;
        publish_azure(&parent, &staging)?;
        Ok(())
    })();
    if let Err(error) = result {
        if let Err(cleanup) = clean_staging(&staging) {
            eprintln!(
                "Could not clean Azure staging directory {}: {cleanup}",
                staging.display()
            );
        }
        return Err(error);
    }
    println!("Azure Sofie is ready. Close and relaunch the app to use her recordings offline.");
    Ok(())
}

#[cfg(unix)]
pub fn prepare_adventure_speech() -> Result<(), Box<dyn Error>> {
    let data = data_dir();
    let python = data.join("azure-env").join("bin").join("python");
    if !python.is_file() {
        return Err(io::Error::other(
            "Azure's preparation environment is missing. Follow README's Azure speech setup.",
        )
        .into());
    }
    let parent = data.join("speech");
    fs::create_dir_all(&parent)?;
    let mut caches = Vec::new();
    for entry in fs::read_dir(&parent)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("adventure-generation-")
            && entry.path().is_dir()
        {
            caches.push(entry.path());
        }
    }
    let staging = parent.join(format!(
        "adventure-generation-{}",
        glib::uuid_string_random()
    ));
    fs::create_dir(&staging)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        let phrases = learn_swedish::adventure::speech_phrases();
        let mut jobs = Vec::new();
        for voice in [
            learn_swedish::lesson::Voice::Sofie,
            learn_swedish::lesson::Voice::Mattias,
        ] {
            let selected: Vec<_> = phrases
                .iter()
                .filter(|p| p.voice == Some(voice))
                .cloned()
                .collect();
            let target = staging.join(voice.azure_name());
            fs::create_dir(&target)?;
            let missing = reuse_azure_audio(
                &parent.join(ADVENTURE_BANK).join(voice.azure_name()),
                &target,
                &selected,
            )?;
            let voice_caches: Vec<_> = caches
                .iter()
                .map(|cache| cache.join(voice.azure_name()))
                .collect();
            for text in missing {
                let phrase = selected
                    .iter()
                    .find(|p| p.swedish == text)
                    .expect("Missing phrase came from this voice");
                if !reuse_cached_recording(&voice_caches, &target, phrase)? {
                    jobs.push(serde_json::json!({"text": text, "voice": voice.azure_name(), "stem": phrase.audio_stem()}));
                }
            }
        }
        println!(
            "Story: {} fixed recordings, {} reused, {} to generate.",
            phrases.len(),
            phrases.len() - jobs.len(),
            jobs.len()
        );
        if !jobs.is_empty() {
            // The story manifest is too large for portable command-line limits.
            let manifest = staging.join("jobs.json");
            fs::write(
                &manifest,
                serde_json::to_vec(&serde_json::json!({"jobs": jobs}))?,
            )?;
            let status = Command::new(python)
                .args([
                    "-B",
                    "-c",
                    include_str!("../scripts/prepare_azure_speech.py"),
                ])
                .arg(&staging)
                .arg(format!("@{}", manifest.display()))
                .stdin(Stdio::inherit())
                .status()?;
            fs::remove_file(manifest)?;
            if !status.success() {
                return Err(io::Error::other(format!("Adventure speech preparation failed ({status}); the active story library is unchanged.")).into());
            }
        }
        for voice in [
            learn_swedish::lesson::Voice::Sofie,
            learn_swedish::lesson::Voice::Mattias,
        ] {
            let selected: Vec<_> = phrases
                .iter()
                .filter(|p| p.voice == Some(voice))
                .cloned()
                .collect();
            validate_audio(&staging.join(voice.azure_name()), &selected)?;
        }
        publish_bank(&parent, &staging, ADVENTURE_BANK)?;
        Ok(())
    })();
    if result.is_err() {
        eprintln!(
            "Completed recordings are retained in {}. Run preparation again to reuse them; the active library is unchanged.",
            staging.display()
        );
    }
    result?;
    println!("Adventure voices are ready. Relaunch the app to use them offline.");
    Ok(())
}

#[cfg(unix)]
fn reuse_cached_recording(
    caches: &[PathBuf],
    target: &Path,
    phrase: &Phrase,
) -> Result<bool, Box<dyn Error>> {
    for cache in caches {
        let audio = cache.join(format!("{}.wav", phrase.audio_stem()));
        let timing = audio.with_extension("json");
        if audio.exists() || timing.exists() {
            if let Err(error) = validate_audio(cache, std::slice::from_ref(phrase)) {
                eprintln!(
                    "Ignoring incomplete or invalid cached story recording {}: {error}. It will be regenerated if no valid copy exists.",
                    audio.display()
                );
                continue;
            }
            fs::copy(audio, target.join(format!("{}.wav", phrase.audio_stem())))?;
            fs::copy(timing, target.join(format!("{}.json", phrase.audio_stem())))?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(unix)]
fn reuse_azure_audio(
    current: &Path,
    staging: &Path,
    phrases: &[Phrase],
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut missing = Vec::new();
    for phrase in phrases {
        let audio = current.join(format!("{}.wav", phrase.audio_stem()));
        let timing = audio.with_extension("json");
        if audio.exists() || timing.exists() {
            validate_audio(current, std::slice::from_ref(phrase))?;
            fs::copy(audio, staging.join(format!("{}.wav", phrase.audio_stem())))?;
            fs::copy(
                timing,
                staging.join(format!("{}.json", phrase.audio_stem())),
            )?;
        } else {
            missing.push(phrase.swedish.clone());
        }
    }
    Ok(missing)
}

#[cfg(unix)]
fn publish_azure(parent: &Path, generation: &Path) -> io::Result<()> {
    publish_bank(parent, generation, AZURE_VOICE)
}

#[cfg(unix)]
fn publish_bank(parent: &Path, generation: &Path, name: &str) -> io::Result<()> {
    let link = parent.join(format!("azure-select-{}", glib::uuid_string_random()));
    std::os::unix::fs::symlink(
        generation
            .file_name()
            .expect("A generation directory has a name"),
        &link,
    )?;
    if let Err(error) = fs::rename(&link, parent.join(name)) {
        if let Err(cleanup) = fs::remove_file(&link) {
            eprintln!(
                "Could not remove Azure selection link {}: {cleanup}",
                link.display()
            );
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(unix)]
fn clean_staging(staging: &Path) -> io::Result<()> {
    // Only remove files from this preparation's private staging directory.
    for entry in fs::read_dir(staging)? {
        fs::remove_file(entry?.path())?;
    }
    fs::remove_dir(staging)
}

#[cfg(unix)]
pub fn prepare_speech() -> Result<(), Box<dyn Error>> {
    let data = data_dir();
    let python = data.join("piper-env").join("bin").join("python");
    if !python.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Piper or its Swedish model is missing from {}. Follow README's speech setup.",
                data.display()
            ),
        )
        .into());
    }
    for voice in VOICES {
        let model = data.join("voices").join(format!("{voice}.onnx"));
        if !model.is_file() || !model.with_extension("onnx.json").is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Piper model {voice} is missing from {}. Follow README's speech setup.",
                    data.display()
                ),
            )
            .into());
        }
    }
    for voice in VOICES {
        let model = data.join("voices").join(format!("{voice}.onnx"));
        let destination = data.join("speech").join(voice);
        prepare_voice(&python, &model, &destination)?;
    }
    Ok(())
}

#[cfg(unix)]
fn prepare_voice(python: &Path, model: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    let staging = destination.join(format!("prepare-{}", glib::uuid_string_random()));
    fs::create_dir(&staging)?;
    let result = generate_audio(python, model, &staging, destination);
    let cleanup = clean_staging(&staging);
    if let Err(error) = &cleanup {
        eprintln!(
            "Could not clean speech staging directory {}: {error}",
            staging.display()
        );
    }
    result?;
    cleanup?;
    println!("Piper speech is ready in {}", destination.display());
    Ok(())
}

#[cfg(unix)]
fn generate_audio(
    python: &Path,
    model: &Path,
    staging: &Path,
    destination: &Path,
) -> Result<(), Box<dyn Error>> {
    let phrases = speech_phrases();
    let mut child = Command::new(python)
        .args(["-c", include_str!("../scripts/prepare_speech.py")])
        .arg(model)
        .arg(staging)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let texts: Vec<_> = phrases.iter().map(|phrase| &phrase.swedish).collect();
    let input = serde_json::to_vec(&texts)?;
    let write_result = child
        .stdin
        .take()
        .expect("Piped stdin must be available")
        .write_all(&input);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Piper generation failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    write_result?;
    validate_audio(staging, &phrases)?;
    for phrase in &phrases {
        for extension in ["wav", "json"] {
            let filename = format!("{}.{extension}", phrase.swedish);
            fs::rename(staging.join(&filename), destination.join(&filename))?;
        }
    }
    println!(
        "Prepared {} bilingual phrases with word timings.",
        phrases.len()
    );
    Ok(())
}

#[cfg(unix)]
fn validate_audio(staging: &Path, phrases: &[Phrase]) -> Result<(), Box<dyn Error>> {
    for phrase in phrases {
        let path = staging.join(format!("{}.wav", phrase.audio_stem()));
        let mut file = fs::File::open(&path)?;
        let mut header = [0; 12];
        file.read_exact(&mut header)?;
        if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" || file.metadata()?.len() <= 44 {
            return Err(
                io::Error::other(format!("Invalid speech audio: {}", path.display())).into(),
            );
        }
        Timing::load(
            &staging.join(format!("{}.json", phrase.audio_stem())),
            &phrase.swedish,
        )?;
    }
    Ok(())
}

impl Speech {
    pub fn set_show_english(&self, show: bool) {
        self.show_english.set(show);
        if let Some(transcript) = self.transcript.borrow().as_ref() {
            transcript.label.set_markup(&transcript_markup(
                &transcript.phrases,
                transcript.active,
                show,
            ));
        }
    }

    pub fn show_transcript(&self, phrases: &[Phrase], label: &gtk::Label) {
        self.render_transcript(phrases, label, None);
    }

    fn render_transcript(
        &self,
        phrases: &[Phrase],
        label: &gtk::Label,
        active: Option<(usize, usize)>,
    ) {
        label.set_markup(&transcript_markup(phrases, active, self.show_english.get()));
        self.transcript.replace(Some(Transcript {
            phrases: phrases.to_vec(),
            label: label.clone(),
            active,
        }));
        if label.widget_name() == "adventure-transcript"
            && let Some((phrase_index, word_index)) = active
        {
            let plain = |phrase: &Phrase| {
                let mut text = String::new();
                if let Some(speaker) = phrase.speaker {
                    text.push_str(speaker);
                    text.push('\n');
                }
                text.push_str(
                    &phrase
                        .swedish
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                if self.show_english.get() && !phrase.english.is_empty() {
                    text.push('\n');
                    text.push_str(&phrase.english);
                }
                text
            };
            let before: usize = phrases[..phrase_index]
                .iter()
                .map(|p| plain(p).len() + 2)
                .sum();
            let speaker = phrases[phrase_index]
                .speaker
                .map_or(0, |name| name.len() + 1);
            let words: usize = phrases[phrase_index]
                .swedish
                .split_whitespace()
                .take(word_index)
                .map(|word| word.len() + 1)
                .sum();
            let offset = i32::try_from(before + speaker + words)
                .expect("A story page fits Pango's text index");
            let rect = label.layout().index_to_pos(offset);
            if let Some(scroll) = label
                .ancestor(gtk::ScrolledWindow::static_type())
                .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
            {
                let adjustment = scroll.vadjustment();
                let y = f64::from(rect.y()) / f64::from(gtk::pango::SCALE);
                let height = f64::from(rect.height()) / f64::from(gtk::pango::SCALE);
                if y < adjustment.value()
                    || y + height > adjustment.value() + adjustment.page_size()
                {
                    adjustment.set_value(
                        (y - 12.0)
                            .clamp(0.0, (adjustment.upper() - adjustment.page_size()).max(0.0)),
                    );
                }
            }
        }
    }

    pub fn stop(&self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        let playback = self.playback.borrow_mut().take();
        if let Some(mut playback) = playback {
            if let Err(error) = playback.media.set_state(gst::State::Null) {
                Self::report_error(&playback.status, &format!("Could not stop audio: {error}"));
            }
            self.show_transcript(&playback.phrases, &playback.transcript);
            if let Some(completion) = playback.completion.take() {
                completion(false);
            }
        }
    }

    fn media(&self, path: &Path) -> Result<gst::Pipeline, Box<dyn Error>> {
        gst::init()?;
        let media = gst::Pipeline::new();
        let source = gst::ElementFactory::make("filesrc")
            .property(
                "location",
                path.to_str()
                    .ok_or_else(|| io::Error::other("Audio path is not valid UTF-8"))?,
            )
            .build()?;
        let parser = gst::ElementFactory::make("wavparse").build()?;
        let convert = gst::ElementFactory::make("audioconvert").build()?;
        let resample = gst::ElementFactory::make("audioresample").build()?;
        #[cfg(not(windows))]
        let sink_factory = "pulsesink";
        #[cfg(windows)]
        let sink_factory = "directsoundsink";
        #[cfg(test)]
        let sink_factory = if self.muted { "fakesink" } else { sink_factory };
        let sink = gst::ElementFactory::make(sink_factory)
            .property("sync", true)
            .build()?;
        media.add_many([&source, &parser, &convert, &resample, &sink])?;
        gst::Element::link_many([&source, &parser, &convert, &resample, &sink])?;
        if let Err(error) = media.set_state(gst::State::Playing) {
            if let Err(stop_error) = media.set_state(gst::State::Null) {
                eprintln!("Could not stop failed audio pipeline: {stop_error}");
            }
            return Err(error.into());
        }
        Ok(media)
    }

    pub fn speak_then(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: impl FnOnce(bool) + 'static,
    ) {
        self.start(
            phrases,
            status,
            transcript,
            Some(Box::new(completion)),
            None,
        );
    }

    pub fn interrupt_with(self: &Rc<Self>, phrase: Phrase) {
        let mut state = self.playback.borrow_mut();
        let Some(playback) = state.as_ref() else {
            return;
        };
        if playback.reminder && playback.current == 0 {
            return;
        }
        let mut playback = state.take().expect("Playback was checked above");
        drop(state);
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        if let Err(error) = playback.media.set_state(gst::State::Null) {
            Self::report_error(
                &playback.status,
                &format!("Could not interrupt audio: {error}"),
            );
            if let Some(completion) = playback.completion.take() {
                completion(false);
            }
            return;
        }
        let audio_dir = playback.clips[playback.current]
            .audio
            .parent()
            .expect("Audio clips have a voice directory")
            .to_path_buf();
        let phrases = std::iter::once(phrase)
            .chain(
                playback.clips[playback.current..]
                    .iter()
                    .map(|clip| clip.phrase.clone()),
            )
            .collect();
        self.start(
            phrases,
            &playback.status,
            &playback.transcript,
            playback.completion.take(),
            Some(audio_dir),
        );
        if let Some(playback) = self.playback.borrow_mut().as_mut() {
            playback.reminder = true;
        }
    }

    fn start(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: Option<Box<dyn FnOnce(bool)>>,
        audio_dir: Option<PathBuf>,
    ) {
        self.stop();
        self.show_transcript(&phrases, transcript);
        let audio_dir = audio_dir.unwrap_or_else(|| {
            let choice = usize::try_from(glib::random_int_range(
                0,
                i32::try_from(self.audio_dirs.len()).expect("Voice count fits i32"),
            ))
            .expect("Voice choice is nonnegative");
            self.audio_dirs[choice].clone()
        });
        let clips = phrases
            .iter()
            .map(|phrase| -> Result<Clip, Box<dyn Error>> {
                let directory = phrase.voice.map_or_else(
                    || audio_dir.clone(),
                    |voice| self.story_dir.join(voice.azure_name()),
                );
                let stem = recording_stem(phrase);
                let audio = directory.join(format!("{stem}.wav"));
                if !audio.is_file() {
                    return Err(io::Error::other(format!(
                        "Speech audio is missing: {}",
                        audio.display()
                    ))
                    .into());
                }
                let timing =
                    Timing::load(&directory.join(format!("{stem}.json")), &phrase.swedish)?;
                Ok(Clip {
                    phrase: phrase.clone(),
                    audio,
                    timing,
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let clips = match clips {
            Ok(clips) if !clips.is_empty() => clips,
            Ok(_) => {
                Self::report_error(status, "No phrases were supplied for playback");
                if let Some(completion) = completion {
                    completion(false);
                }
                return;
            }
            Err(error) => {
                Self::report_error(status, &error.to_string());
                if let Some(completion) = completion {
                    completion(false);
                }
                return;
            }
        };
        let media = match self.media(&clips[0].audio) {
            Ok(media) => media,
            Err(error) => {
                Self::report_error(status, &error.to_string());
                if let Some(completion) = completion {
                    completion(false);
                }
                return;
            }
        };
        Self::voice_tooltip(transcript, &clips[0]);
        status.set_label(&format!("Speaking Swedish: {}", clips[0].phrase.swedish));
        self.playback.replace(Some(Playback {
            clips,
            phrases,
            current: 0,
            media,
            loading_since: Instant::now(),
            highlighted: None,
            status: status.clone(),
            transcript: transcript.clone(),
            completion,
            reminder: false,
        }));
        let weak = Rc::downgrade(self);
        let timer = glib::timeout_add_local(Duration::from_millis(25), move || {
            weak.upgrade()
                .map_or(glib::ControlFlow::Break, |speech| speech.tick())
        });
        self.timer.replace(Some(timer));
    }

    fn tick(&self) -> glib::ControlFlow {
        let mut state = self.playback.borrow_mut();
        let Some(playback) = state.as_mut() else {
            self.timer.borrow_mut().take();
            return glib::ControlFlow::Break;
        };
        let message = playback
            .media
            .bus()
            .expect("A pipeline has a bus")
            .pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos]);
        let ended = message
            .as_ref()
            .is_some_and(|message| matches!(message.view(), gst::MessageView::Eos(_)));
        let timestamp = playback.media.query_position::<gst::ClockTime>();
        let failure = if let Some(gst::MessageView::Error(error)) =
            message.as_ref().map(|message| message.view())
        {
            Some(format!("{} ({:?})", error.error(), error.debug()))
        } else if timestamp.is_none() && playback.loading_since.elapsed() > Duration::from_secs(15)
        {
            Some("Audio did not become ready within 15 seconds".into())
        } else {
            None
        };
        let mut completed = false;
        if let Some(message) = failure {
            Self::report_error(&playback.status, &message);
        } else if ended {
            playback.current += 1;
            if let Some(clip) = playback.clips.get(playback.current) {
                Self::voice_tooltip(&playback.transcript, clip);
                let next = playback
                    .media
                    .set_state(gst::State::Null)
                    .map_err(|error| -> Box<dyn Error> { error.into() })
                    .and_then(|_| self.media(&clip.audio));
                match next {
                    Ok(media) => playback.media = media,
                    Err(error) => {
                        Self::report_error(&playback.status, &error.to_string());
                        self.show_transcript(&playback.phrases, &playback.transcript);
                        let completion = playback.completion.take();
                        state.take();
                        drop(state);
                        self.timer.borrow_mut().take();
                        if let Some(completion) = completion {
                            completion(false);
                        }
                        return glib::ControlFlow::Break;
                    }
                }
                playback.loading_since = Instant::now();
                playback.highlighted = None;
                self.show_transcript(&playback.phrases, &playback.transcript);
                playback
                    .status
                    .set_label(&format!("Speaking Swedish: {}", clip.phrase.swedish));
                return glib::ControlFlow::Continue;
            }
            playback
                .status
                .set_label("Swedish speech ready · Replay to hear the prompt again");
            completed = true;
        } else {
            let highlighted = timestamp
                .and_then(|position| {
                    playback.clips[playback.current].timing.word_at(
                        i64::try_from(position.useconds()).expect("Speech duration fits i64"),
                    )
                })
                .map(|word| (playback.current, word));
            if playback.highlighted != highlighted {
                self.render_transcript(&playback.phrases, &playback.transcript, highlighted);
                playback.highlighted = highlighted;
            }
            return glib::ControlFlow::Continue;
        }
        if let Err(error) = playback.media.set_state(gst::State::Null) {
            Self::report_error(&playback.status, &format!("Could not stop audio: {error}"));
            completed = false;
        }
        self.show_transcript(&playback.phrases, &playback.transcript);
        let completion = playback.completion.take();
        state.take();
        drop(state);
        self.timer.borrow_mut().take();
        if let Some(completion) = completion {
            completion(completed);
        }
        glib::ControlFlow::Break
    }

    fn report_error(status: &gtk::Label, message: &str) {
        eprintln!("Speech: {message}");
        #[cfg(windows)]
        let setup = "See README for the optional speech download and check Windows audio output.";
        #[cfg(not(windows))]
        let setup = if message.contains(ADVENTURE_BANK) {
            "Prepare story voices: cargo run --locked -- --prepare-adventure-speech."
        } else {
            "Check README's speech setup, GStreamer audio plugins and audio output."
        };
        status.set_label(&format!("{message}\n{setup} Text practice still works."));
    }

    fn voice_tooltip(label: &gtk::Label, clip: &Clip) {
        let voice = clip
            .audio
            .parent()
            .and_then(Path::file_name)
            .expect("Audio clips have a voice directory");
        label.set_tooltip_text(Some(&format!("Speech voice: {}", voice.to_string_lossy())));
    }

    #[cfg(test)]
    pub fn for_story_test(directory: PathBuf) -> Self {
        let mut speech = Self::for_test(vec![directory.clone()], true);
        speech.story_dir = directory;
        speech
    }

    #[cfg(test)]
    pub fn for_test(audio_dirs: Vec<PathBuf>, muted: bool) -> Self {
        assert!(!audio_dirs.is_empty());
        Self {
            playback: RefCell::new(None),
            timer: RefCell::new(None),
            audio_dirs,
            show_english: Cell::new(false),
            transcript: RefCell::new(None),
            story_dir: data_dir().join("speech").join(ADVENTURE_BANK),
            muted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    #[ignore = "Requires the optional voice banks and a Windows audio device"]
    fn portable_windows_banks_play_with_bundled_audio_plugins() {
        let data = data_dir();
        let keyboard = speech_clip_path(&data, AZURE_VOICE, "Bokstaven Å");
        let speech = Speech::default();
        let story = learn_swedish::adventure::speech_phrases();
        let mut clips = vec![(keyboard, "Bokstaven Å".to_owned())];
        for phrase in learn_swedish::lesson::speech_phrases() {
            let audio = speech_clip_path(&data, AZURE_VOICE, &recording_stem(&phrase));
            assert!(audio.is_file(), "Missing {}", audio.display());
            Timing::load(&audio.with_extension("json"), &phrase.swedish).unwrap();
        }
        for voice in ["sv-SE-SofieNeural", "sv-SE-MattiasNeural"] {
            let phrase = story
                .iter()
                .find(|phrase| phrase.voice.map(|voice| voice.azure_name()) == Some(voice))
                .expect("Story includes both voices");
            let bank = format!("{ADVENTURE_BANK}/{voice}");
            clips.push((
                speech_clip_path(&data, &bank, &phrase.audio_stem()),
                phrase.swedish.clone(),
            ));
        }
        for (audio, text) in clips {
            let timing = Timing::load(&audio.with_extension("json"), &text).unwrap();
            assert_eq!(timing.word_at(timing.words[0].start_us), Some(0));
            let pipeline = speech.media(&audio).unwrap();
            let message = pipeline.bus().unwrap().timed_pop_filtered(
                gst::ClockTime::from_seconds(10),
                &[gst::MessageType::Error, gst::MessageType::Eos],
            );
            pipeline.set_state(gst::State::Null).unwrap();
            assert!(
                matches!(
                    message.as_ref().map(|message| message.view()),
                    Some(gst::MessageView::Eos(_))
                ),
                "Playback failed for {}: {message:?}",
                audio.display()
            );
        }
    }

    #[cfg(windows)]
    fn speech_clip_path(data: &Path, bank: &str, stem: &str) -> PathBuf {
        data.join("speech").join(bank).join(format!("{stem}.wav"))
    }

    #[test]
    #[cfg(unix)]
    fn azure_update_reuses_valid_recordings_and_requests_only_missing_phrases() {
        let data =
            std::env::temp_dir().join(format!("swedish-reuse-test-{}", glib::uuid_string_random()));
        let current = data.join("current");
        let staging = data.join("staging");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir(&staging).unwrap();
        let phrase = Phrase {
            english: "Good job!".into(),
            swedish: "Bra jobbat!".into(),
            voice: None,
            speaker: None,
        };
        let mut wav = b"RIFF\0\0\0\0WAVE".to_vec();
        wav.resize(45, 0);
        fs::write(current.join("Bra jobbat!.wav"), &wav).unwrap();
        let json = r#"{"duration_us":1000000,"words":[{"word":"Bra","start_us":100000,"end_us":400000},{"word":"jobbat!","start_us":450000,"end_us":900000}]}"#;
        fs::write(current.join("Bra jobbat!.json"), json).unwrap();
        let missing = learn_swedish::lesson::wait_phrase(0);
        assert_eq!(
            reuse_azure_audio(&current, &staging, &[phrase.clone(), missing.clone()]).unwrap(),
            vec![missing.swedish]
        );
        assert_eq!(fs::read(staging.join("Bra jobbat!.wav")).unwrap(), wav);
        assert_eq!(
            fs::read_to_string(staging.join("Bra jobbat!.json")).unwrap(),
            json
        );
        assert!(reuse_cached_recording(std::slice::from_ref(&current), &staging, &phrase).unwrap());
        fs::write(current.join("Bra jobbat!.json"), "{}").unwrap();
        assert!(
            !reuse_cached_recording(std::slice::from_ref(&current), &staging, &phrase).unwrap()
        );
        assert!(reuse_azure_audio(&current, &staging, &[phrase]).is_err());
        clean_staging(&current).unwrap();
        clean_staging(&staging).unwrap();
        fs::remove_dir(data).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn azure_library_is_selected_only_when_published_and_replaces_the_bank_atomically() {
        let data =
            std::env::temp_dir().join(format!("swedish-azure-test-{}", glib::uuid_string_random()));
        let parent = data.join("speech");
        fs::create_dir_all(&parent).unwrap();
        let first = parent.join("first-generation");
        fs::create_dir(&first).unwrap();
        fs::write(first.join("bank"), "first").unwrap();
        assert_eq!(audio_directories(&data).len(), 2);
        assert!(validate_audio(&first, &speech_phrases()).is_err());
        assert_eq!(audio_directories(&data).len(), 2);
        publish_azure(&parent, &first).unwrap();
        assert_eq!(audio_directories(&data), vec![parent.join(AZURE_VOICE)]);
        assert_eq!(
            fs::read_to_string(parent.join(AZURE_VOICE).join("bank")).unwrap(),
            "first"
        );

        let second = parent.join("second-generation");
        fs::create_dir(&second).unwrap();
        fs::write(second.join("bank"), "second").unwrap();
        publish_azure(&parent, &second).unwrap();
        assert_eq!(
            fs::read_to_string(parent.join(AZURE_VOICE).join("bank")).unwrap(),
            "second"
        );
        assert_eq!(fs::read_to_string(first.join("bank")).unwrap(), "first");
        fs::remove_file(parent.join(AZURE_VOICE)).unwrap();
        clean_staging(&first).unwrap();
        clean_staging(&second).unwrap();
        fs::remove_dir(parent).unwrap();
        fs::remove_dir(data).unwrap();
    }

    fn timings() -> Timing {
        Timing {
            duration_us: 1_000_000,
            words: vec![
                WordTiming {
                    word: "Bra".into(),
                    start_us: 100_000,
                    end_us: 400_000,
                },
                WordTiming {
                    word: "jobbat!".into(),
                    start_us: 450_000,
                    end_us: 900_000,
                },
            ],
        }
    }

    #[test]
    fn highlighting_uses_word_boundaries_including_silence() {
        let timing = timings();
        assert!(timing.validate("Bra jobbat!").is_ok());
        for (time, word) in [
            (0, None),
            (100_000, Some(0)),
            (399_999, Some(0)),
            (400_000, None),
            (450_000, Some(1)),
            (900_000, None),
        ] {
            assert_eq!(timing.word_at(time), word);
        }
        let mut invalid = timings();
        invalid.words[1].start_us = 200_000;
        assert!(invalid.validate("Bra jobbat!").is_err());
        assert!(timings().validate("Snyggt gjort!").is_err());
        let mut invalid = timings();
        invalid.words[1].end_us = 1_100_000;
        assert!(invalid.validate("Bra jobbat!").is_err());
    }

    #[test]
    fn transcript_keeps_both_languages_and_escapes_markup() {
        let phrases = vec![
            Phrase {
                english: "Less < & more".into(),
                swedish: "Mindre än".into(),
                voice: None,
                speaker: None,
            },
            Phrase {
                english: "Good job!".into(),
                swedish: "Bra jobbat!".into(),
                voice: None,
                speaker: None,
            },
        ];
        let markup = transcript_markup(&phrases, Some((1, 0)), true);
        assert!(markup.contains("Less &lt; &amp; more"));
        assert!(markup.contains("Mindre än"));
        assert!(markup.contains("Good job!"));
        assert!(markup.contains("weight=\"bold\">Bra</span> jobbat!"));
        assert_eq!(markup.matches("background=").count(), 1);
        assert!(!transcript_markup(&phrases, None, true).contains("background="));
        let swedish_only = transcript_markup(&phrases, Some((1, 0)), false);
        assert!(!swedish_only.contains("Good job!"));
        assert!(!swedish_only.contains("Less"));
        assert!(swedish_only.contains("weight=\"bold\">Bra</span> jobbat!"));
    }
}
