use adw::prelude::WidgetExt;
use adw::{glib, gtk};
use gstreamer::{self as gst, prelude::*};
use learn_swedish::lesson::{Phrase, speech_phrases};
use serde::Deserialize;
use std::cell::RefCell;
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

const VOICES: &[&str] = &["sv_SE-nst-medium", "sv_SE-alma-medium"];
const AZURE_VOICE: &str = "azure-sv-SE-SofieNeural";

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

pub fn transcript_markup(phrases: &[Phrase], active: Option<(usize, usize)>) -> String {
    phrases.iter().enumerate().map(|(phrase_index, phrase)| {
        let swedish = phrase.swedish.split_whitespace().enumerate().map(|(word_index, word)| {
            let escaped = glib::markup_escape_text(word);
            if active == Some((phrase_index, word_index)) {
                format!("<span background=\"#f6d32d\" foreground=\"#1c1c1c\" weight=\"bold\">{escaped}</span>")
            } else {
                escaped.to_string()
            }
        }).collect::<Vec<_>>().join(" ");
        format!("{swedish}\n<span size=\"small\">{}</span>", glib::markup_escape_text(&phrase.english))
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
    completion: Option<Box<dyn FnOnce()>>,
}

pub struct Speech {
    playback: RefCell<Option<Playback>>,
    timer: RefCell<Option<glib::SourceId>>,
    audio_dirs: Vec<PathBuf>,
    #[cfg(test)]
    muted: bool,
}

fn data_dir() -> PathBuf {
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

impl Default for Speech {
    fn default() -> Self {
        Self {
            playback: RefCell::new(None),
            timer: RefCell::new(None),
            audio_dirs: audio_directories(&data_dir()),
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
        let texts: Vec<_> = phrases.iter().map(|phrase| &phrase.swedish).collect();
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

fn publish_azure(parent: &Path, generation: &Path) -> io::Result<()> {
    let link = parent.join(format!("azure-select-{}", glib::uuid_string_random()));
    std::os::unix::fs::symlink(
        generation
            .file_name()
            .expect("A generation directory has a name"),
        &link,
    )?;
    if let Err(error) = fs::rename(&link, parent.join(AZURE_VOICE)) {
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

fn clean_staging(staging: &Path) -> io::Result<()> {
    // Only remove files from this preparation's private staging directory.
    for entry in fs::read_dir(staging)? {
        fs::remove_file(entry?.path())?;
    }
    fs::remove_dir(staging)
}

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

fn validate_audio(staging: &Path, phrases: &[Phrase]) -> Result<(), Box<dyn Error>> {
    for phrase in phrases {
        let path = staging.join(format!("{}.wav", phrase.swedish));
        let mut file = fs::File::open(&path)?;
        let mut header = [0; 12];
        file.read_exact(&mut header)?;
        if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" || file.metadata()?.len() <= 44 {
            return Err(
                io::Error::other(format!("Invalid speech audio: {}", path.display())).into(),
            );
        }
        Timing::load(
            &staging.join(format!("{}.json", phrase.swedish)),
            &phrase.swedish,
        )?;
    }
    Ok(())
}

impl Speech {
    pub fn stop(&self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        if let Some(playback) = self.playback.borrow_mut().take() {
            if let Err(error) = playback.media.set_state(gst::State::Null) {
                Self::report_error(&playback.status, &format!("Could not stop audio: {error}"));
            }
            playback
                .transcript
                .set_markup(&transcript_markup(&playback.phrases, None));
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
        let sink_factory = "pulsesink";
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

    pub fn speak(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
    ) {
        self.start(phrases, status, transcript, None);
    }

    pub fn speak_then(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: impl FnOnce() + 'static,
    ) {
        self.start(phrases, status, transcript, Some(Box::new(completion)));
    }

    fn start(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: Option<Box<dyn FnOnce()>>,
    ) {
        self.stop();
        transcript.set_markup(&transcript_markup(&phrases, None));
        let choice = usize::try_from(glib::random_int_range(
            0,
            i32::try_from(self.audio_dirs.len()).expect("Voice count fits i32"),
        ))
        .expect("Voice choice is nonnegative");
        let audio_dir = &self.audio_dirs[choice];
        transcript.set_tooltip_text(Some(&format!(
            "Speech voice: {}",
            audio_dir
                .file_name()
                .expect("Voice directory has a name")
                .to_string_lossy()
        )));
        let clips = phrases
            .iter()
            .map(|phrase| -> Result<Clip, Box<dyn Error>> {
                let audio = audio_dir.join(format!("{}.wav", phrase.swedish));
                if !audio.is_file() {
                    return Err(io::Error::other(format!(
                        "Speech audio is missing: {}",
                        audio.display()
                    ))
                    .into());
                }
                let timing = Timing::load(
                    &audio_dir.join(format!("{}.json", phrase.swedish)),
                    &phrase.swedish,
                )?;
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
                return;
            }
            Err(error) => {
                Self::report_error(status, &error.to_string());
                return;
            }
        };
        let media = match self.media(&clips[0].audio) {
            Ok(media) => media,
            Err(error) => {
                Self::report_error(status, &error.to_string());
                return;
            }
        };
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
                let next = playback
                    .media
                    .set_state(gst::State::Null)
                    .map_err(|error| -> Box<dyn Error> { error.into() })
                    .and_then(|_| self.media(&clip.audio));
                match next {
                    Ok(media) => playback.media = media,
                    Err(error) => {
                        Self::report_error(&playback.status, &error.to_string());
                        playback
                            .transcript
                            .set_markup(&transcript_markup(&playback.phrases, None));
                        state.take();
                        self.timer.borrow_mut().take();
                        return glib::ControlFlow::Break;
                    }
                }
                playback.loading_since = Instant::now();
                playback.highlighted = None;
                playback
                    .transcript
                    .set_markup(&transcript_markup(&playback.phrases, None));
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
                playback
                    .transcript
                    .set_markup(&transcript_markup(&playback.phrases, highlighted));
                playback.highlighted = highlighted;
            }
            return glib::ControlFlow::Continue;
        }
        if let Err(error) = playback.media.set_state(gst::State::Null) {
            Self::report_error(&playback.status, &format!("Could not stop audio: {error}"));
            completed = false;
        }
        playback
            .transcript
            .set_markup(&transcript_markup(&playback.phrases, None));
        let completion = if completed {
            playback.completion.take()
        } else {
            None
        };
        state.take();
        drop(state);
        self.timer.borrow_mut().take();
        if let Some(completion) = completion {
            completion();
        }
        glib::ControlFlow::Break
    }

    fn report_error(status: &gtk::Label, message: &str) {
        eprintln!("Speech: {message}");
        status.set_label(&format!(
            "{message}\nCheck README's speech setup, GStreamer audio plugins and audio output. Keyboard practice still works."
        ));
    }

    #[cfg(test)]
    pub fn for_test(audio_dirs: Vec<PathBuf>, muted: bool) -> Self {
        assert!(!audio_dirs.is_empty());
        Self {
            playback: RefCell::new(None),
            timer: RefCell::new(None),
            audio_dirs,
            muted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
            },
            Phrase {
                english: "Good job!".into(),
                swedish: "Bra jobbat!".into(),
            },
        ];
        let markup = transcript_markup(&phrases, Some((1, 0)));
        assert!(markup.contains("Less &lt; &amp; more"));
        assert!(markup.contains("Mindre än"));
        assert!(markup.contains("Good job!"));
        assert!(markup.contains("weight=\"bold\">Bra</span> jobbat!"));
        assert_eq!(markup.matches("background=").count(), 1);
        assert!(!transcript_markup(&phrases, None).contains("background="));
    }
}
