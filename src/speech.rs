use adw::prelude::*;
use adw::{glib, gtk};
use gstreamer::{self as gst, prelude::*};
#[cfg(unix)]
use learn_swedish::lesson::speech_phrases;
use learn_swedish::lesson::{Phrase, Voice};
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

mod azure;
pub use azure::AzureSpeech;

const VOICES: &[&str] = &["sv_SE-nst-medium", "sv_SE-alma-medium"];
const AZURE_VOICE: &str = "azure-sv-SE-SofieNeural";

#[derive(Clone, Debug, Deserialize)]
struct WordTiming {
    word: String,
    start_us: i64,
    end_us: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct Timing {
    duration_us: i64,
    words: Vec<WordTiming>,
    #[serde(default)]
    sentence_starts_us: Vec<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sentence {
    start_us: i64,
    end_us: i64,
}

fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', ')', ']', '»', '”', '’'])
        .ends_with(['.', '!', '?'])
}

impl Timing {
    fn load(path: &Path, text: &str) -> Result<Self, Box<dyn Error>> {
        let timing: Self = serde_json::from_slice(&fs::read(path)?)?;
        timing.validate(text)?;
        Ok(timing)
    }

    fn load_online(path: &Path, text: &str) -> Result<Self, Box<dyn Error>> {
        let mut timing: Self = serde_json::from_slice(&fs::read(path)?)?;
        let starts = &timing.sentence_starts_us;
        if timing.duration_us <= 0
            || starts.first().is_some_and(|start| *start != 0)
            || starts.windows(2).any(|pair| pair[0] >= pair[1])
            || starts
                .last()
                .is_some_and(|start| *start >= timing.duration_us)
        {
            return Err(io::Error::other("Invalid Azure sentence timings").into());
        }
        if timing.words.is_empty() {
            timing.populate_estimated_words(text);
        }
        if !timing.words.is_empty() {
            timing.validate(text)?;
        }
        Ok(timing)
    }

    fn populate_estimated_words(&mut self, text: &str) {
        let raw_words: Vec<&str> = text.split_whitespace().collect();
        let Ok(total_count) = i64::try_from(raw_words.len()) else {
            return;
        };
        if raw_words.is_empty() || self.duration_us < total_count {
            return;
        }
        let sentences = self.sentences();
        let mut word_groups: Vec<Vec<&str>> = Vec::new();
        let mut current_group = Vec::new();
        for (idx, &word) in raw_words.iter().enumerate() {
            current_group.push(word);
            if idx + 1 == raw_words.len() || ends_sentence(word) {
                word_groups.push(std::mem::take(&mut current_group));
            }
        }
        let distribute =
            |group: &[&str], win_start: i64, win_end: i64, out: &mut Vec<WordTiming>| -> bool {
                let count = group.len();
                let Ok(count_i64) = i64::try_from(count) else {
                    return false;
                };
                let avail = win_end - win_start;
                if avail < count_i64 {
                    return false;
                }
                let weights: Vec<i64> = group
                    .iter()
                    .map(|w| i64::try_from(w.chars().count().max(1)).unwrap_or(1))
                    .collect();
                let total_weight: i64 = weights.iter().sum::<i64>().max(1);
                let mut pos = win_start;
                let mut acc = 0_i64;
                for (offset, &word) in group.iter().enumerate() {
                    acc += weights[offset];
                    let remaining = i64::try_from(count - offset - 1).unwrap_or(0);
                    let end = if offset + 1 == count {
                        win_end
                    } else {
                        let target = win_start + (avail * acc) / total_weight;
                        target.max(pos + 1).min(win_end - remaining)
                    };
                    out.push(WordTiming {
                        word: word.to_owned(),
                        start_us: pos,
                        end_us: end,
                    });
                    pos = end;
                }
                true
            };

        let mut generated = Vec::with_capacity(raw_words.len());
        let mut matched_sentences = word_groups.len() == sentences.len();
        if matched_sentences {
            for (group, sentence) in word_groups.iter().zip(&sentences) {
                if !distribute(group, sentence.start_us, sentence.end_us, &mut generated) {
                    matched_sentences = false;
                    break;
                }
            }
        }
        if !matched_sentences {
            generated.clear();
            if !distribute(&raw_words, 0, self.duration_us, &mut generated) {
                return;
            }
        }
        self.words = generated;
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

    fn word_at_continuous(&self, timestamp: i64) -> Option<usize> {
        if let Some(exact) = self.word_at(timestamp) {
            return Some(exact);
        }
        for (index, word) in self.words.iter().enumerate() {
            if timestamp >= word.end_us {
                let next_start = self
                    .words
                    .get(index + 1)
                    .map_or(self.duration_us, |next| next.start_us);
                if timestamp < next_start && timestamp - word.end_us <= 250_000 {
                    return Some(index);
                }
            }
        }
        None
    }

    fn sentences(&self) -> Vec<Sentence> {
        if !self.sentence_starts_us.is_empty() {
            return self
                .sentence_starts_us
                .iter()
                .enumerate()
                .map(|(index, start)| Sentence {
                    start_us: *start,
                    end_us: self
                        .sentence_starts_us
                        .get(index + 1)
                        .copied()
                        .unwrap_or(self.duration_us),
                })
                .collect();
        }
        if self.words.is_empty() {
            return vec![Sentence {
                start_us: 0,
                end_us: self.duration_us,
            }];
        }
        let mut sentences = Vec::new();
        let mut start_word = 0;
        for (index, word) in self.words.iter().enumerate() {
            let last_word = index + 1 == self.words.len();
            if last_word || ends_sentence(&word.word) {
                let start_us = if start_word == 0 {
                    0
                } else {
                    self.words[start_word].start_us
                };
                let end_us = if last_word {
                    self.duration_us
                } else {
                    word.end_us
                };
                sentences.push(Sentence { start_us, end_us });
                start_word = index + 1;
            }
        }
        sentences
    }
}

struct PhraseSpan {
    start: usize,
    end: usize,
    words: Vec<(usize, usize)>,
}

fn transcript_spans(phrases: &[Phrase], show_english: bool) -> Vec<PhraseSpan> {
    let mut cursor = 0;
    let mut spans = Vec::with_capacity(phrases.len());
    for (index, phrase) in phrases.iter().enumerate() {
        if index > 0 {
            cursor += 2; // "\n\n"
        }
        let start = cursor;
        if let Some(speaker) = &phrase.speaker {
            cursor += speaker.len() + " · ".len();
        }
        let mut words = Vec::new();
        for (word_index, word) in phrase.swedish.split_whitespace().enumerate() {
            if word_index > 0 {
                cursor += 1; // " "
            }
            let word_start = cursor;
            cursor += word.len();
            words.push((word_start, cursor));
        }
        if show_english {
            if let Some(literal) = &phrase.literal {
                cursor += 1 + literal.len();
            }
            if !phrase.english.is_empty() {
                cursor += 1 + phrase.english.len();
            }
        }
        spans.push(PhraseSpan {
            start,
            end: cursor,
            words,
        });
    }
    spans
}

fn clean_word_token(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn is_two_word_swedish_idiom(first: &str, second: &str) -> bool {
    let a = clean_word_token(first);
    let b = clean_word_token(second);
    matches!(
        (a.as_str(), b.as_str()),
        ("i", "dag")
            | ("i", "går")
            | ("i", "morgon")
            | ("i", "kväll")
            | ("i", "natt")
            | ("i", "stället")
            | ("in", "i")
            | ("ut", "ur")
            | ("ett", "annat")
            | ("en", "annan")
            | ("någon", "gång")
            | ("nån", "gång")
            | ("var", "sin")
            | ("var", "sitt")
            | ("allt", "ihop")
            | ("över", "allt")
            | ("till", "exempel")
            | ("fram", "för")
    )
}

fn is_two_word_english_gloss(sv_word: &str, lit_first: &str, lit_second: &str) -> bool {
    let sv = clean_word_token(sv_word);
    let l1 = clean_word_token(lit_first);
    let l2 = clean_word_token(lit_second);
    if l1 == "to" && sv != "att" && sv != "till" && !l2.is_empty() {
        return true;
    }
    matches!(
        (sv.as_str(), l1.as_str(), l2.as_str()),
        ("ledigt", "time", "off") | ("ledig", "time", "off") | ("slut", "run", "out")
    )
}

type ClausePair = ((usize, usize), (usize, usize));

fn split_matching_clauses<'a>(
    sv_words: &[&'a str],
    lit_words: &[&'a str],
    pred: impl Fn(&str) -> bool,
) -> Option<Vec<ClausePair>> {
    let sv_breaks: Vec<usize> = sv_words
        .iter()
        .enumerate()
        .filter_map(|(i, w)| (i + 1 == sv_words.len() || pred(w)).then_some(i + 1))
        .collect();
    let lit_breaks: Vec<usize> = lit_words
        .iter()
        .enumerate()
        .filter_map(|(i, w)| (i + 1 == lit_words.len() || pred(w)).then_some(i + 1))
        .collect();
    if sv_breaks.len() != lit_breaks.len() || sv_breaks.len() <= 1 {
        return None;
    }
    let mut out = Vec::with_capacity(sv_breaks.len());
    let mut s_prev = 0;
    let mut l_prev = 0;
    for (s_end, l_end) in sv_breaks.into_iter().zip(lit_breaks) {
        out.push(((s_prev, s_end), (l_prev, l_end)));
        s_prev = s_end;
        l_prev = l_end;
    }
    Some(out)
}

pub fn literal_word_span(
    swedish: &str,
    literal: &str,
    swedish_word_index: usize,
) -> Option<(usize, usize)> {
    let sv_all: Vec<&str> = swedish.split_whitespace().collect();
    let lit_all: Vec<&str> = literal.split_whitespace().collect();
    if swedish_word_index >= sv_all.len() || lit_all.is_empty() {
        return None;
    }
    if sv_all.len() == lit_all.len() {
        return Some((swedish_word_index, swedish_word_index + 1));
    }
    let sentence_pairs = split_matching_clauses(&sv_all, &lit_all, ends_sentence)
        .unwrap_or_else(|| vec![((0, sv_all.len()), (0, lit_all.len()))]);
    let ((s_sent_lo, s_sent_hi), (l_sent_lo, l_sent_hi)) = sentence_pairs
        .into_iter()
        .find(|((s_lo, s_hi), _)| (*s_lo..*s_hi).contains(&swedish_word_index))?;

    let clause_pred = |w: &str| {
        w.trim_end_matches(['"', '\'', ')', ']', '»', '”', '’'])
            .ends_with([',', ';', ':', '—', '–'])
    };
    let sv_sent = &sv_all[s_sent_lo..s_sent_hi];
    let lit_sent = &lit_all[l_sent_lo..l_sent_hi];
    let local_sent_idx = swedish_word_index - s_sent_lo;
    let ((c_sv_lo, c_sv_hi), (c_lit_lo, c_lit_hi)) =
        if let Some(clauses) = split_matching_clauses(sv_sent, lit_sent, clause_pred) {
            clauses
                .into_iter()
                .find(|((lo, hi), _)| (*lo..*hi).contains(&local_sent_idx))?
        } else {
            ((0, sv_sent.len()), (0, lit_sent.len()))
        };

    let sv_clause = &sv_sent[c_sv_lo..c_sv_hi];
    let lit_clause = &lit_sent[c_lit_lo..c_lit_hi];
    let target = local_sent_idx - c_sv_lo;
    let lit_base = l_sent_lo + c_lit_lo;
    let s_len = sv_clause.len();
    let l_len = lit_clause.len();
    if s_len == l_len {
        return Some((lit_base + target, lit_base + target + 1));
    }

    let mut mapping = vec![(0, 1); s_len];
    let mut s = 0;
    let mut l = 0;
    while s < s_len && l < l_len {
        let rem_s = s_len - s;
        let rem_l = l_len - l;
        if rem_s == 1 {
            mapping[s] = (l, l_len);
            break;
        }
        if rem_l == 1 {
            for item in &mut mapping[s..s_len] {
                *item = (l, l + 1);
            }
            break;
        }
        if rem_s == rem_l {
            for offset in 0..rem_s {
                mapping[s + offset] = (l + offset, l + offset + 1);
            }
            break;
        }
        if rem_s > rem_l && is_two_word_swedish_idiom(sv_clause[s], sv_clause[s + 1]) {
            mapping[s] = (l, l + 1);
            mapping[s + 1] = (l, l + 1);
            s += 2;
            l += 1;
            continue;
        }
        if rem_l > rem_s
            && is_two_word_english_gloss(sv_clause[s], lit_clause[l], lit_clause[l + 1])
        {
            mapping[s] = (l, l + 2);
            s += 1;
            l += 2;
            continue;
        }
        let later_idiom = (rem_s > rem_l
            && (s + 1..s_len - 1)
                .any(|k| is_two_word_swedish_idiom(sv_clause[k], sv_clause[k + 1])))
            || (rem_l > rem_s
                && (s + 1..s_len).any(|k| {
                    let est_l = l + (k - s);
                    est_l + 1 < l_len
                        && is_two_word_english_gloss(
                            sv_clause[k],
                            lit_clause[est_l],
                            lit_clause[est_l + 1],
                        )
                }));
        if later_idiom {
            mapping[s] = (l, l + 1);
            s += 1;
            l += 1;
        } else {
            for idx in 0..rem_s {
                let start = l + (idx * rem_l) / rem_s;
                let end = (l + ((idx + 1) * rem_l).div_ceil(rem_s))
                    .max(start + 1)
                    .min(l_len);
                mapping[s + idx] = (start.min(l_len - 1), end);
            }
            break;
        }
    }
    let (lo, hi) = mapping[target];
    Some((lit_base + lo, lit_base + hi))
}

pub fn transcript_markup(
    phrases: &[Phrase],
    active: Option<(usize, usize)>,
    show_english: bool,
) -> String {
    phrases
        .iter()
        .enumerate()
        .map(|(phrase_index, phrase)| {
            let swedish = phrase
                .swedish
                .split_whitespace()
                .enumerate()
                .map(|(word_index, word)| {
                    let escaped = glib::markup_escape_text(word);
                    if active == Some((phrase_index, word_index)) {
                        format!(
                            "<span background=\"#f6d32d\" foreground=\"#1c1c1c\" weight=\"bold\">{escaped}</span>"
                        )
                    } else {
                        escaped.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            let spoken = if let Some(speaker) = &phrase.speaker {
                format!(
                    "<span size=\"small\" weight=\"bold\">{}</span> · {swedish}",
                    glib::markup_escape_text(speaker)
                )
            } else {
                swedish
            };
            let mut text = spoken;
            if show_english {
                use std::fmt::Write;
                if let Some(literal) = &phrase.literal {
                    let lit_active = active
                        .filter(|(p_idx, _)| *p_idx == phrase_index)
                        .and_then(|(_, w_idx)| literal_word_span(&phrase.swedish, literal, w_idx));
                    let lit_markup = literal
                        .split_whitespace()
                        .enumerate()
                        .map(|(lit_idx, word)| {
                            let escaped = glib::markup_escape_text(word);
                            if lit_active.is_some_and(|(lo, hi)| (lo..hi).contains(&lit_idx)) {
                                format!("<span underline=\"single\" weight=\"bold\">{escaped}</span>")
                            } else {
                                escaped.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    write!(text, "\n<span size=\"small\" style=\"italic\">{lit_markup}</span>")
                        .unwrap();
                }
                if !phrase.english.is_empty() {
                    write!(
                        text,
                        "\n<span size=\"small\">{}</span>",
                        glib::markup_escape_text(&phrase.english)
                    )
                    .unwrap();
                }
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Clone)]
struct Clip {
    phrase: Phrase,
    phrase_index: usize,
    audio: PathBuf,
    live_voice: Option<Voice>,
    timing: Timing,
    sentences: Vec<Sentence>,
}

type Completion = Option<Box<dyn FnOnce(bool)>>;

struct PendingSpeech {
    request: azure::Request,
    poll: glib::SourceId,
    completion: Completion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseState {
    Playing,
    Manual,
    AtSentenceEnd,
    AtClipEnd,
}

struct Playback {
    clips: Vec<Clip>,
    phrases: Vec<Phrase>,
    current: usize,
    sentence: usize,
    pause_state: PauseState,
    awaiting_seek_below: Option<(i64, Instant)>,
    media: gst::Pipeline,
    loading_since: Instant,
    highlighted: Option<(usize, usize)>,
    approx_scroll_word: Option<(usize, usize)>,
    status: gtk::Label,
    transcript: gtk::Label,
    completion: Completion,
    reminder: bool,
}

struct LastSentence {
    phrases: Vec<Phrase>,
    clip: Clip,
    sentence: usize,
    status: gtk::Label,
    transcript: gtk::Label,
}

struct Transcript {
    phrases: Vec<Phrase>,
    label: gtk::Label,
    active: Option<(usize, usize)>,
}

type PhraseListener = Box<dyn Fn(Option<usize>)>;

pub struct Speech {
    playback: RefCell<Option<Playback>>,
    pending: RefCell<Option<PendingSpeech>>,
    timer: RefCell<Option<glib::SourceId>>,
    audio_dirs: Vec<PathBuf>,
    show_english: Cell<bool>,
    auto_pause: Cell<bool>,
    transcript: RefCell<Option<Transcript>>,
    last_sentence: RefCell<Option<LastSentence>>,
    spinners: RefCell<Vec<gtk::Spinner>>,
    on_phrase: RefCell<Option<PhraseListener>>,
    azure: Rc<AzureSpeech>,
    cast: RefCell<azure::Cast>,
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
        Self::with_azure(Rc::new(AzureSpeech::default()))
    }
}

impl Speech {
    pub fn with_azure(azure: Rc<AzureSpeech>) -> Self {
        Self {
            playback: RefCell::new(None),
            pending: RefCell::new(None),
            timer: RefCell::new(None),
            audio_dirs: audio_directories(&data_dir()),
            show_english: Cell::new(false),
            auto_pause: Cell::new(true),
            transcript: RefCell::new(None),
            last_sentence: RefCell::new(None),
            spinners: RefCell::new(Vec::new()),
            on_phrase: RefCell::new(None),
            azure,
            cast: RefCell::default(),
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
    pub fn register_spinner(&self, spinner: &gtk::Spinner) {
        let loading = self.is_loading();
        spinner.set_visible(loading);
        if loading {
            spinner.start();
        } else {
            spinner.stop();
        }
        self.spinners.borrow_mut().push(spinner.clone());
    }

    pub fn set_phrase_listener(&self, listener: impl Fn(Option<usize>) + 'static) {
        self.on_phrase.replace(Some(Box::new(listener)));
    }

    pub fn is_loading(&self) -> bool {
        self.pending.borrow().is_some()
    }

    fn set_loading(&self, loading: bool) {
        for spinner in self.spinners.borrow().iter() {
            spinner.set_visible(loading);
            if loading {
                spinner.start();
            } else {
                spinner.stop();
            }
        }
    }

    fn notify_phrase(&self, phrase_index: Option<usize>) {
        if let Some(listener) = self.on_phrase.borrow().as_ref() {
            listener(phrase_index);
        }
    }

    pub fn azure_enabled(&self) -> bool {
        self.azure.enabled()
    }

    pub fn azure_remembered(&self) -> bool {
        self.azure.remembered()
    }

    pub fn azure_has_key(&self) -> bool {
        self.azure.has_available_key()
    }

    pub fn azure_region(&self) -> String {
        self.azure.region()
    }

    #[cfg(test)]
    pub fn configure_azure(&self, region: &str, key: &str) -> Result<(), &'static str> {
        self.azure.configure(region, key)
    }

    pub fn configure_azure_persistent(
        &self,
        region: &str,
        key: &str,
        remember: bool,
    ) -> Result<(), &'static str> {
        self.azure.configure_persistent(region, key, remember)
    }

    pub fn disable_azure(&self) {
        self.azure.disable();
    }

    pub fn forget_azure(&self) {
        self.azure.forget();
    }

    pub fn azure_cached_clips(&self) -> usize {
        self.azure.cached_clip_count()
    }

    pub fn clear_azure_cache(&self) {
        self.azure.clear_cache();
    }

    pub fn set_cast(&self, actors: &[learn_swedish::adventure::ActorSpec]) {
        self.cast.borrow_mut().set_specs(actors.to_vec());
    }

    pub fn recast_guests(&self) {
        self.cast.borrow_mut().recast_guests();
    }

    pub fn set_show_english(&self, show: bool) {
        self.show_english.set(show);
        let active_clip_phrase = self
            .playback
            .borrow()
            .as_ref()
            .map(|pb| pb.clips[pb.current].phrase_index);
        if let Some(transcript) = self.transcript.borrow().as_ref() {
            transcript.label.set_markup(&transcript_markup(
                &transcript.phrases,
                transcript.active,
                show,
            ));
            if let Some((phrase_index, word_index)) = transcript.active {
                Self::scroll_transcript_to(
                    &transcript.phrases,
                    &transcript.label,
                    show,
                    phrase_index,
                    Some(word_index),
                );
            } else if let Some(phrase_index) = active_clip_phrase {
                Self::scroll_transcript_to(
                    &transcript.phrases,
                    &transcript.label,
                    show,
                    phrase_index,
                    None,
                );
            }
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
        let show_english = self.show_english.get();
        label.set_markup(&transcript_markup(phrases, active, show_english));
        self.transcript.replace(Some(Transcript {
            phrases: phrases.to_vec(),
            label: label.clone(),
            active,
        }));
        if let Some((phrase_index, word_index)) = active {
            Self::scroll_transcript_to(
                phrases,
                label,
                show_english,
                phrase_index,
                Some(word_index),
            );
        }
    }

    fn scroll_transcript_to(
        phrases: &[Phrase],
        label: &gtk::Label,
        show_english: bool,
        phrase_index: usize,
        word_index: Option<usize>,
    ) {
        if label.widget_name() != "adventure-transcript" || phrase_index >= phrases.len() {
            return;
        }
        let spans = transcript_spans(phrases, show_english);
        let span = &spans[phrase_index];
        let (focus_start, focus_end) = match word_index.and_then(|w| span.words.get(w).copied()) {
            Some(word_span) => word_span,
            None => span
                .words
                .first()
                .map_or((span.start, span.end), |first| (span.start, first.1)),
        };
        let phrase_end = span.end;
        if !Self::apply_transcript_scroll(label, focus_start, focus_end, phrase_end) {
            let weak_label = label.downgrade();
            glib::idle_add_local_once(move || {
                if let Some(label) = weak_label.upgrade() {
                    let _ =
                        Self::apply_transcript_scroll(&label, focus_start, focus_end, phrase_end);
                }
            });
        }
    }

    fn apply_transcript_scroll(
        label: &gtk::Label,
        focus_start: usize,
        focus_end: usize,
        phrase_end: usize,
    ) -> bool {
        let Some(scroll) = label
            .ancestor(gtk::ScrolledWindow::static_type())
            .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
        else {
            return true;
        };
        let adjustment = scroll.vadjustment();
        let page_size = adjustment.page_size();
        let upper = adjustment.upper();
        let layout = label.layout();
        let text_len = layout.text().len();
        if text_len == 0 {
            return true;
        }
        let f_start = i32::try_from(focus_start.min(text_len)).unwrap_or(i32::MAX);
        let f_end = i32::try_from(focus_end.min(text_len)).unwrap_or(i32::MAX);
        let p_end = i32::try_from(phrase_end.min(text_len)).unwrap_or(i32::MAX);
        let start_rect = layout.index_to_pos(f_start);
        let end_rect = layout.index_to_pos(f_end);
        let phrase_rect = layout.index_to_pos(p_end);
        let top_margin = f64::from(label.margin_top());
        let scale = f64::from(gtk::pango::SCALE);
        let word_top = top_margin + f64::from(start_rect.y()) / scale;
        let word_bottom = top_margin + f64::from(end_rect.y() + end_rect.height()) / scale;
        let phrase_bottom = top_margin + f64::from(phrase_rect.y() + phrase_rect.height()) / scale;

        if page_size <= 1.0 || upper + 2.0 < phrase_bottom {
            return false;
        }
        if upper <= page_size + 1.0 {
            if adjustment.value() > 0.0 {
                adjustment.set_value(0.0);
            }
            return true;
        }
        let top_pad = (page_size * 0.15).clamp(14.0, 44.0);
        let bottom_pad = (page_size * 0.28).clamp(32.0, 88.0);
        let visible_phrase_bottom = phrase_bottom.min(word_top + page_size - top_pad - 12.0);
        let desired_bottom = visible_phrase_bottom.max(word_bottom + bottom_pad);
        let current = adjustment.value();
        let max_scroll = (upper - page_size).max(0.0);
        if word_top < current + top_pad {
            adjustment.set_value((word_top - top_pad).clamp(0.0, max_scroll));
        } else if desired_bottom > current + page_size {
            adjustment.set_value((desired_bottom - page_size + 12.0).clamp(0.0, max_scroll));
        }
        true
    }

    pub fn set_auto_pause(self: &Rc<Self>, enabled: bool) {
        self.auto_pause.set(enabled);
        let resume_auto_pause = self.playback.borrow().as_ref().is_some_and(|playback| {
            matches!(
                playback.pause_state,
                PauseState::AtSentenceEnd | PauseState::AtClipEnd
            )
        });
        if !enabled && resume_auto_pause {
            self.toggle_pause();
        }
    }

    pub fn is_active(&self) -> bool {
        self.playback.borrow().is_some()
    }

    #[cfg(test)]
    pub fn is_paused(&self) -> bool {
        self.playback
            .borrow()
            .as_ref()
            .is_some_and(|playback| playback.pause_state != PauseState::Playing)
    }

    pub fn has_last_sentence(&self) -> bool {
        self.pending.borrow().is_none() && self.last_sentence.borrow().is_some()
    }

    pub fn clear_last_sentence(&self) {
        self.last_sentence.borrow_mut().take();
    }

    fn stop_timer(&self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
    }

    fn start_timer(self: &Rc<Self>) {
        self.stop_timer();
        let weak = Rc::downgrade(self);
        let timer = glib::timeout_add_local(Duration::from_millis(15), move || {
            weak.upgrade()
                .map_or(glib::ControlFlow::Break, |speech| speech.tick())
        });
        self.timer.replace(Some(timer));
    }

    pub fn stop(&self) {
        let pending = self.pending.borrow_mut().take();
        self.set_loading(false);
        if let Some(pending) = pending {
            pending.poll.remove();
            drop(pending.request);
            if let Some(completion) = pending.completion {
                completion(false);
            }
        }
        self.stop_timer();
        let playback = self.playback.borrow_mut().take();
        if let Some(mut playback) = playback {
            if let Err(error) = playback.media.set_state(gst::State::Null) {
                Self::report_error(&playback.status, &format!("Could not stop audio: {error}"));
            }
            self.show_transcript(&playback.phrases, &playback.transcript);
            self.notify_phrase(None);
            if let Some(completion) = playback.completion.take() {
                completion(false);
            }
        } else {
            self.notify_phrase(None);
        }
    }

    fn fail_playback(&self, mut state: std::cell::RefMut<'_, Option<Playback>>, message: &str) {
        let Some(mut playback) = state.take() else {
            return;
        };
        drop(state);
        self.stop_timer();
        let _ = playback.media.set_state(gst::State::Null);
        Self::report_error(&playback.status, message);
        self.show_transcript(&playback.phrases, &playback.transcript);
        self.notify_phrase(None);
        if let Some(completion) = playback.completion.take() {
            completion(false);
        }
    }

    fn media(&self, path: &Path) -> Result<gst::Pipeline, Box<dyn Error>> {
        self.media_at(path, 0)
    }

    fn media_at(&self, path: &Path, start_us: i64) -> Result<gst::Pipeline, Box<dyn Error>> {
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
        let start_result: Result<(), Box<dyn Error>> = if start_us > 0 {
            (|| {
                media.set_state(gst::State::Paused)?;
                let (state_result, _, _) = media.state(gst::ClockTime::from_seconds(2));
                state_result?;
                let position = gst::ClockTime::from_useconds(u64::try_from(start_us).unwrap_or(0));
                media.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, position)?;
                media.set_state(gst::State::Playing)?;
                Ok(())
            })()
        } else {
            media
                .set_state(gst::State::Playing)
                .map(|_| ())
                .map_err(Into::into)
        };
        if let Err(error) = start_result {
            if let Err(stop_error) = media.set_state(gst::State::Null) {
                eprintln!("Could not stop failed audio pipeline: {stop_error}");
            }
            return Err(error);
        }
        Ok(media)
    }

    fn seek_and_play(&self, playback: &mut Playback, start_us: i64) -> Result<(), Box<dyn Error>> {
        let position = gst::ClockTime::from_useconds(u64::try_from(start_us).unwrap_or(0));
        let seeked = playback
            .media
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, position)
            .is_ok();
        if seeked {
            playback.media.set_state(gst::State::Playing)?;
        } else {
            let _ = playback.media.set_state(gst::State::Null);
            let audio = playback.clips[playback.current].audio.clone();
            playback.media = self.media_at(&audio, start_us)?;
        }
        Ok(())
    }

    pub fn toggle_pause(self: &Rc<Self>) -> bool {
        let mut state = self.playback.borrow_mut();
        let Some(playback) = state.as_mut() else {
            return false;
        };
        match playback.pause_state {
            PauseState::Playing => {
                if let Err(error) = playback.media.set_state(gst::State::Paused) {
                    self.fail_playback(state, &format!("Could not pause audio: {error}"));
                    return true;
                }
                playback.pause_state = PauseState::Manual;
                playback
                    .status
                    .set_label("Paused · Space to resume · ← to repeat");
                drop(state);
                self.stop_timer();
                true
            }
            PauseState::Manual => {
                if let Err(error) = playback.media.set_state(gst::State::Playing) {
                    self.fail_playback(state, &format!("Could not resume audio: {error}"));
                    return true;
                }
                playback.pause_state = PauseState::Playing;
                playback.loading_since = Instant::now();
                playback.status.set_label(&format!(
                    "Speaking Swedish: {}",
                    playback.clips[playback.current].phrase.swedish
                ));
                drop(state);
                self.start_timer();
                true
            }
            PauseState::AtSentenceEnd => {
                playback.sentence += 1;
                let clip = &playback.clips[playback.current];
                let sentence = clip.sentences[playback.sentence];
                let first_word = clip
                    .timing
                    .words
                    .iter()
                    .position(|w| w.start_us >= sentence.start_us);
                let phrase_index = clip.phrase_index;
                let awaiting = (playback.sentence + 1 < clip.sentences.len())
                    .then(|| (sentence.end_us, Instant::now()));
                if let Err(error) = self.seek_and_play(playback, sentence.start_us) {
                    self.fail_playback(state, &format!("Could not resume audio: {error}"));
                    return true;
                }
                playback.pause_state = PauseState::Playing;
                playback.awaiting_seek_below = awaiting;
                playback.loading_since = Instant::now();
                playback.highlighted = None;
                playback.approx_scroll_word = None;
                playback.status.set_label(&format!(
                    "Speaking Swedish: {}",
                    playback.clips[playback.current].phrase.swedish
                ));
                let phrases = playback.phrases.clone();
                let transcript = playback.transcript.clone();
                drop(state);
                Self::scroll_transcript_to(
                    &phrases,
                    &transcript,
                    self.show_english.get(),
                    phrase_index,
                    first_word,
                );
                self.notify_phrase(Some(phrase_index));
                self.start_timer();
                true
            }
            PauseState::AtClipEnd => {
                playback.current += 1;
                playback.sentence = 0;
                let clip = &playback.clips[playback.current];
                let phrase_index = clip.phrase_index;
                Self::voice_tooltip(&playback.transcript, clip);
                let next = playback
                    .media
                    .set_state(gst::State::Null)
                    .map_err(|error| -> Box<dyn Error> { error.into() })
                    .and_then(|_| self.media(&clip.audio));
                match next {
                    Ok(media) => playback.media = media,
                    Err(error) => {
                        let message = error.to_string();
                        self.fail_playback(state, &message);
                        return true;
                    }
                }
                playback.pause_state = PauseState::Playing;
                playback.awaiting_seek_below = None;
                playback.loading_since = Instant::now();
                playback.highlighted = None;
                playback.approx_scroll_word = None;
                self.show_transcript(&playback.phrases, &playback.transcript);
                playback
                    .status
                    .set_label(&format!("Speaking Swedish: {}", clip.phrase.swedish));
                let phrases = playback.phrases.clone();
                let transcript = playback.transcript.clone();
                drop(state);
                Self::scroll_transcript_to(
                    &phrases,
                    &transcript,
                    self.show_english.get(),
                    phrase_index,
                    None,
                );
                self.notify_phrase(Some(phrase_index));
                self.start_timer();
                true
            }
        }
    }

    pub fn repeat_sentence(self: &Rc<Self>) -> bool {
        let mut state = self.playback.borrow_mut();
        let Some(playback) = state.as_mut() else {
            return false;
        };
        let clip = &playback.clips[playback.current];
        let sentence = clip.sentences[playback.sentence];
        let first_word = clip
            .timing
            .words
            .iter()
            .position(|w| w.start_us >= sentence.start_us);
        let phrase_index = clip.phrase_index;
        let awaiting = (playback.sentence + 1 < clip.sentences.len())
            .then(|| (sentence.end_us, Instant::now()));
        let result = if playback.pause_state == PauseState::AtClipEnd {
            let _ = playback.media.set_state(gst::State::Null);
            let audio = clip.audio.clone();
            self.media_at(&audio, sentence.start_us).map(|media| {
                playback.media = media;
            })
        } else {
            self.seek_and_play(playback, sentence.start_us)
        };
        if let Err(error) = result {
            let message = format!("Could not repeat sentence: {error}");
            self.fail_playback(state, &message);
            return true;
        }
        let clip = &playback.clips[playback.current];
        Self::voice_tooltip(&playback.transcript, clip);
        playback.pause_state = PauseState::Playing;
        playback.awaiting_seek_below = awaiting;
        playback.loading_since = Instant::now();
        playback.highlighted = None;
        playback.approx_scroll_word = None;
        self.show_transcript(&playback.phrases, &playback.transcript);
        playback
            .status
            .set_label(&format!("Speaking Swedish: {}", clip.phrase.swedish));
        let phrases = playback.phrases.clone();
        let transcript = playback.transcript.clone();
        drop(state);
        Self::scroll_transcript_to(
            &phrases,
            &transcript,
            self.show_english.get(),
            phrase_index,
            first_word,
        );
        self.notify_phrase(Some(phrase_index));
        self.start_timer();
        true
    }

    pub fn repeat_last_sentence(self: &Rc<Self>, completion: impl FnOnce(bool) + 'static) -> bool {
        let Some(last) = self.last_sentence.borrow().as_ref().map(|last| {
            (
                last.phrases.clone(),
                last.clip.clone(),
                last.sentence,
                last.status.clone(),
                last.transcript.clone(),
            )
        }) else {
            return false;
        };
        let (phrases, clip, sentence_index, status, transcript) = last;
        let sentence = clip.sentences[sentence_index];
        let first_word = clip
            .timing
            .words
            .iter()
            .position(|w| w.start_us >= sentence.start_us);
        let phrase_index = clip.phrase_index;
        self.stop();
        self.show_transcript(&phrases, &transcript);
        let media = match self.media_at(&clip.audio, sentence.start_us) {
            Ok(media) => media,
            Err(error) => {
                Self::report_error(&status, &error.to_string());
                completion(false);
                return true;
            }
        };
        Self::voice_tooltip(&transcript, &clip);
        status.set_label(&format!("Speaking Swedish: {}", clip.phrase.swedish));
        Self::scroll_transcript_to(
            &phrases,
            &transcript,
            self.show_english.get(),
            phrase_index,
            first_word,
        );
        self.playback.replace(Some(Playback {
            clips: vec![clip],
            phrases,
            current: 0,
            sentence: sentence_index,
            pause_state: PauseState::Playing,
            awaiting_seek_below: None,
            media,
            loading_since: Instant::now(),
            highlighted: None,
            approx_scroll_word: None,
            status,
            transcript,
            completion: Some(Box::new(completion)),
            reminder: false,
        }));
        self.notify_phrase(Some(phrase_index));
        self.start_timer();
        true
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
        self.stop_timer();
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
        let resume_from = if playback.pause_state == PauseState::AtClipEnd {
            (playback.current + 1).min(playback.clips.len().saturating_sub(1))
        } else {
            playback.current
        };
        let audio_dir = playback.clips[resume_from]
            .audio
            .parent()
            .expect("Audio clips have a voice directory")
            .to_path_buf();
        let phrases = std::iter::once(phrase)
            .chain(
                playback.clips[resume_from..]
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
        completion: Completion,
        audio_dir: Option<PathBuf>,
    ) {
        self.stop();
        self.show_transcript(&phrases, transcript);
        let story = !phrases.is_empty()
            && phrases
                .iter()
                .all(|phrase| phrase.voice.is_some() && phrase.speaker.is_some());
        if story {
            self.start_online(phrases, status, transcript, completion);
        } else {
            self.start_offline(phrases, status, transcript, completion, audio_dir);
        }
    }

    fn start_online(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: Completion,
    ) {
        if !self.azure_enabled() {
            status.set_label("Open Azure Speech in the header to set up live read-aloud.");
            if let Some(completion) = completion {
                completion(false);
            }
            return;
        }
        let mut cast = self.cast.borrow_mut();
        let utterances: Vec<_> = phrases
            .iter()
            .map(|phrase| cast.utterance(phrase))
            .collect();
        drop(cast);
        if let Some(recordings) = self.azure.cached(&utterances) {
            self.play_online(
                phrases,
                &utterances,
                recordings,
                status,
                transcript,
                completion,
            );
            return;
        }
        let request = match self.azure.request(&utterances) {
            Ok(request) => request,
            Err(error) => {
                Self::report_online_error(status, &error);
                if let Some(completion) = completion {
                    completion(false);
                }
                return;
            }
        };
        status.set_label("Getting Swedish audio from Azure…");
        let weak = Rc::downgrade(self);
        let status = status.clone();
        let transcript = transcript.clone();
        let mut phrases = Some(phrases);
        let poll = glib::timeout_add_local(Duration::from_millis(30), move || {
            let Some(speech) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let mut state = speech.pending.borrow_mut();
            let Some(pending) = state.as_mut() else {
                return glib::ControlFlow::Break;
            };
            let Some(result) = pending.request.poll() else {
                return glib::ControlFlow::Continue;
            };
            let pending = state.take().expect("Polling an active Azure request");
            drop(state);
            speech.set_loading(false);
            let phrases = phrases.take().expect("The request completes only once");
            let result = match result {
                Ok(()) => speech.azure.save(pending.request).and_then(|()| {
                    speech
                        .azure
                        .cached(&utterances)
                        .ok_or_else(|| "Azure did not return all the requested recordings.".into())
                }),
                Err(error) => {
                    drop(pending.request);
                    Err(error)
                }
            };
            match result {
                Ok(recordings) => speech.play_online(
                    phrases,
                    &utterances,
                    recordings,
                    &status,
                    &transcript,
                    pending.completion,
                ),
                Err(error) => {
                    Self::report_online_error(&status, &error);
                    if let Some(completion) = pending.completion {
                        completion(false);
                    }
                }
            }
            glib::ControlFlow::Break
        });
        self.pending.replace(Some(PendingSpeech {
            request,
            poll,
            completion,
        }));
        self.set_loading(true);
    }

    fn play_online(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        utterances: &[azure::Utterance],
        recordings: Vec<azure::Recording>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: Completion,
    ) {
        let clips = phrases
            .iter()
            .zip(utterances)
            .zip(recordings)
            .enumerate()
            .map(|(phrase_index, ((phrase, utterance), recording))| Clip {
                phrase: phrase.clone(),
                phrase_index,
                audio: recording.audio,
                live_voice: Some(utterance.voice()),
                sentences: recording.timing.sentences(),
                timing: recording.timing,
            });
        let clips = clips.collect();
        self.play_clips(phrases, clips, status, transcript, completion);
    }

    fn start_offline(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: Completion,
        audio_dir: Option<PathBuf>,
    ) {
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
            .enumerate()
            .map(|(phrase_index, phrase)| -> Result<Clip, Box<dyn Error>> {
                let stem = recording_stem(phrase);
                let audio = audio_dir.join(format!("{stem}.wav"));
                if !audio.is_file() {
                    return Err(io::Error::other(format!(
                        "Speech audio is missing: {}",
                        audio.display()
                    ))
                    .into());
                }
                let timing =
                    Timing::load(&audio_dir.join(format!("{stem}.json")), &phrase.swedish)?;
                let sentences = timing.sentences();
                Ok(Clip {
                    phrase: phrase.clone(),
                    phrase_index,
                    audio,
                    live_voice: None,
                    timing,
                    sentences,
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let clips = match clips {
            Ok(clips) => clips,
            Err(error) => {
                Self::report_error(status, &error.to_string());
                if let Some(completion) = completion {
                    completion(false);
                }
                return;
            }
        };
        self.play_clips(phrases, clips, status, transcript, completion);
    }

    fn play_clips(
        self: &Rc<Self>,
        phrases: Vec<Phrase>,
        clips: Vec<Clip>,
        status: &gtk::Label,
        transcript: &gtk::Label,
        completion: Completion,
    ) {
        if clips.is_empty() {
            Self::report_error(status, "No phrases were supplied for playback");
            if let Some(completion) = completion {
                completion(false);
            }
            return;
        }
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
        let first_phrase_index = clips[0].phrase_index;
        Self::scroll_transcript_to(
            &phrases,
            transcript,
            self.show_english.get(),
            first_phrase_index,
            None,
        );
        self.playback.replace(Some(Playback {
            clips,
            phrases,
            current: 0,
            sentence: 0,
            pause_state: PauseState::Playing,
            awaiting_seek_below: None,
            media,
            loading_since: Instant::now(),
            highlighted: None,
            approx_scroll_word: None,
            status: status.clone(),
            transcript: transcript.clone(),
            completion,
            reminder: false,
        }));
        self.notify_phrase(Some(first_phrase_index));
        self.start_timer();
    }

    fn tick(&self) -> glib::ControlFlow {
        let mut state = self.playback.borrow_mut();
        let Some(playback) = state.as_mut() else {
            self.timer.borrow_mut().take();
            return glib::ControlFlow::Break;
        };
        if playback.pause_state != PauseState::Playing {
            self.timer.borrow_mut().take();
            return glib::ControlFlow::Break;
        }
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
            let last_sentence = playback.clips[playback.current]
                .sentences
                .len()
                .saturating_sub(1);
            playback.sentence = last_sentence;
            if playback.current + 1 < playback.clips.len() {
                if self.auto_pause.get() && !(playback.reminder && playback.current == 0) {
                    if let Err(error) = playback.media.set_state(gst::State::Null) {
                        Self::report_error(
                            &playback.status,
                            &format!("Could not pause audio: {error}"),
                        );
                    } else {
                        playback.pause_state = PauseState::AtClipEnd;
                        playback.awaiting_seek_below = None;
                        playback.highlighted = None;
                        self.show_transcript(&playback.phrases, &playback.transcript);
                        playback
                            .status
                            .set_label("Paused after sentence · Space to continue · ← to repeat");
                        self.timer.borrow_mut().take();
                        return glib::ControlFlow::Break;
                    }
                } else {
                    playback.current += 1;
                    playback.sentence = 0;
                    let clip = &playback.clips[playback.current];
                    let phrase_index = clip.phrase_index;
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
                            self.notify_phrase(None);
                            if let Some(completion) = completion {
                                completion(false);
                            }
                            return glib::ControlFlow::Break;
                        }
                    }
                    playback.awaiting_seek_below = None;
                    playback.loading_since = Instant::now();
                    playback.highlighted = None;
                    playback.approx_scroll_word = None;
                    self.show_transcript(&playback.phrases, &playback.transcript);
                    playback
                        .status
                        .set_label(&format!("Speaking Swedish: {}", clip.phrase.swedish));
                    let phrases = playback.phrases.clone();
                    let transcript = playback.transcript.clone();
                    drop(state);
                    Self::scroll_transcript_to(
                        &phrases,
                        &transcript,
                        self.show_english.get(),
                        phrase_index,
                        None,
                    );
                    self.notify_phrase(Some(phrase_index));
                    return glib::ControlFlow::Continue;
                }
            } else {
                let last_clip = playback.clips[playback.current].clone();
                self.last_sentence.replace(Some(LastSentence {
                    phrases: playback.phrases.clone(),
                    clip: last_clip,
                    sentence: last_sentence,
                    status: playback.status.clone(),
                    transcript: playback.transcript.clone(),
                }));
                playback
                    .status
                    .set_label("Swedish speech ready · Replay to hear the prompt again");
                completed = true;
            }
        } else {
            if let Some(position) = timestamp {
                let pos_us = i64::try_from(position.useconds()).expect("Speech duration fits i64");
                let mut ignore_boundary = false;
                if let Some((limit, since)) = playback.awaiting_seek_below {
                    if pos_us < limit || since.elapsed() > Duration::from_millis(250) {
                        playback.awaiting_seek_below = None;
                    } else {
                        ignore_boundary = true;
                    }
                }
                let clip = &playback.clips[playback.current];
                if !ignore_boundary && playback.sentence + 1 < clip.sentences.len() {
                    if self.auto_pause.get() {
                        if pos_us >= clip.sentences[playback.sentence].end_us {
                            if let Err(error) = playback.media.set_state(gst::State::Paused) {
                                Self::report_error(
                                    &playback.status,
                                    &format!("Could not pause audio: {error}"),
                                );
                            } else {
                                playback.pause_state = PauseState::AtSentenceEnd;
                                playback.highlighted = None;
                                self.show_transcript(&playback.phrases, &playback.transcript);
                                playback.status.set_label(
                                    "Paused after sentence · Space to continue · ← to repeat",
                                );
                                self.timer.borrow_mut().take();
                                return glib::ControlFlow::Break;
                            }
                        }
                    } else {
                        while playback.sentence + 1 < clip.sentences.len()
                            && pos_us >= clip.sentences[playback.sentence + 1].start_us
                        {
                            playback.sentence += 1;
                        }
                    }
                }
                let highlighted = if ignore_boundary {
                    None
                } else if clip.live_voice.is_some() {
                    clip.timing
                        .word_at_continuous(pos_us)
                        .map(|word| (clip.phrase_index, word))
                } else {
                    clip.timing
                        .word_at(pos_us)
                        .map(|word| (clip.phrase_index, word))
                };
                if playback.highlighted != highlighted {
                    self.render_transcript(&playback.phrases, &playback.transcript, highlighted);
                    playback.highlighted = highlighted;
                } else if !ignore_boundary
                    && clip.timing.words.is_empty()
                    && clip.timing.duration_us > 0
                {
                    let word_count = clip.phrase.swedish.split_whitespace().count();
                    if word_count > 0 {
                        let clamped = pos_us.clamp(0, clip.timing.duration_us) as u64;
                        let total = clip.timing.duration_us as u64;
                        let approx = usize::try_from((clamped * word_count as u64) / total)
                            .unwrap_or(0)
                            .min(word_count - 1);
                        let target = Some((clip.phrase_index, approx));
                        if playback.approx_scroll_word != target {
                            playback.approx_scroll_word = target;
                            Self::scroll_transcript_to(
                                &playback.phrases,
                                &playback.transcript,
                                self.show_english.get(),
                                clip.phrase_index,
                                Some(approx),
                            );
                        }
                    }
                }
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
        self.notify_phrase(None);
        if let Some(completion) = completion {
            completion(completed);
        }
        glib::ControlFlow::Break
    }

    fn report_online_error(status: &gtk::Label, message: &str) {
        eprintln!("Azure Speech: {message}");
        status.set_label(&format!("{message} Text mode still works."));
    }

    fn report_error(status: &gtk::Label, message: &str) {
        eprintln!("Speech: {message}");
        let setup = if cfg!(windows) {
            "See README for the optional speech download and check Windows audio output."
        } else {
            "Check README's speech setup, GStreamer audio plugins and audio output."
        };
        status.set_label(&format!("{message}\n{setup} Text practice still works."));
    }

    fn voice_tooltip(label: &gtk::Label, clip: &Clip) {
        if let Some(voice) = clip.live_voice {
            label.set_tooltip_text(Some(&format!(
                "Speech voice: {} · Azure live",
                voice.azure_name()
            )));
            return;
        }
        let voice = clip
            .audio
            .parent()
            .and_then(Path::file_name)
            .expect("Audio clips have a voice directory");
        label.set_tooltip_text(Some(&format!("Speech voice: {}", voice.to_string_lossy())));
    }

    #[cfg(test)]
    pub fn for_live_test(worker: &'static str) -> Self {
        let mut speech = Self::for_test(
            vec![glib::tmp_dir().join(glib::uuid_string_random().as_str())],
            true,
        );
        speech.azure = Rc::new(AzureSpeech::with_worker(worker));
        speech
    }

    #[cfg(test)]
    pub fn for_live_test_with_paths(
        worker: &'static str,
        storage_path: PathBuf,
        cache_dir: PathBuf,
    ) -> Self {
        let mut speech = Self::for_test(
            vec![glib::tmp_dir().join(glib::uuid_string_random().as_str())],
            true,
        );
        speech.azure = Rc::new(AzureSpeech::with_worker_and_paths(
            worker,
            storage_path,
            cache_dir,
        ));
        speech
    }

    #[cfg(test)]
    pub fn for_test(audio_dirs: Vec<PathBuf>, muted: bool) -> Self {
        assert!(!audio_dirs.is_empty());
        Self {
            playback: RefCell::new(None),
            pending: RefCell::new(None),
            timer: RefCell::new(None),
            audio_dirs,
            show_english: Cell::new(false),
            auto_pause: Cell::new(true),
            transcript: RefCell::new(None),
            last_sentence: RefCell::new(None),
            spinners: RefCell::new(Vec::new()),
            on_phrase: RefCell::new(None),
            azure: Rc::new(AzureSpeech::default()),
            cast: RefCell::default(),
            muted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    #[ignore = "Requires the optional keyboard voice bank and a Windows audio device"]
    fn portable_windows_banks_play_with_bundled_audio_plugins() {
        let data = data_dir();
        let keyboard = speech_clip_path(&data, AZURE_VOICE, "Bokstaven Å");
        let speech = Speech::default();
        for phrase in learn_swedish::lesson::speech_phrases() {
            let audio = speech_clip_path(&data, AZURE_VOICE, &recording_stem(&phrase));
            assert!(audio.is_file(), "Missing {}", audio.display());
            Timing::load(&audio.with_extension("json"), &phrase.swedish).unwrap();
        }
        let timing = Timing::load(&keyboard.with_extension("json"), "Bokstaven Å").unwrap();
        assert_eq!(timing.word_at(timing.words[0].start_us), Some(0));
        let pipeline = speech.media(&keyboard).unwrap();
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
            keyboard.display()
        );
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
            literal: None,
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
        fs::write(current.join("Bra jobbat!.json"), "{}").unwrap();
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
            sentence_starts_us: Vec::new(),
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
                literal: Some("Literal < & >".into()),
                swedish: "Mindre än".into(),
                voice: None,
                speaker: None,
            },
            Phrase {
                english: "Good job!".into(),
                literal: None,
                swedish: "Bra jobbat!".into(),
                voice: None,
                speaker: None,
            },
        ];
        let markup = transcript_markup(&phrases, Some((1, 0)), true);
        assert!(markup.starts_with(concat!(
            "Mindre än\n",
            "<span size=\"small\" style=\"italic\">Literal &lt; &amp; &gt;</span>\n",
            "<span size=\"small\">Less &lt; &amp; more</span>",
        )));
        assert!(markup.contains("Good job!"));
        assert!(markup.contains("weight=\"bold\">Bra</span> jobbat!"));
        assert_eq!(markup.matches("background=").count(), 1);
        assert!(!transcript_markup(&phrases, None, true).contains("background="));
        let swedish_only = transcript_markup(&phrases, Some((1, 0)), false);
        assert!(!swedish_only.contains("Good job!"));
        assert!(!swedish_only.contains("Less"));
        assert!(!swedish_only.contains("Literal"));
        assert!(swedish_only.contains("weight=\"bold\">Bra</span> jobbat!"));
    }

    #[test]
    fn literal_english_equivalent_underscores_matching_words_and_idioms() {
        assert_eq!(
            literal_word_span("Du väljer utflykt i dag.", "You choose outing today.", 3),
            Some((3, 4))
        );
        assert_eq!(
            literal_word_span("Du väljer utflykt i dag.", "You choose outing today.", 4),
            Some((3, 4))
        );
        assert_eq!(
            literal_word_span(
                "Åtta grader. Värmen verkar ha tagit ledigt.",
                "Eight degrees. Warmth-the seems to have taken time off.",
                4
            ),
            Some((4, 6))
        );
        assert_eq!(
            literal_word_span(
                "Åtta grader. Värmen verkar ha tagit ledigt.",
                "Eight degrees. Warmth-the seems to have taken time off.",
                6
            ),
            Some((7, 9))
        );
        let phrase = Phrase {
            swedish: "Första kapitlet".into(),
            literal: Some("First chapter-the".into()),
            english: "Chapter One".into(),
            voice: None,
            speaker: None,
        };
        let markup = transcript_markup(std::slice::from_ref(&phrase), Some((0, 1)), true);
        assert!(markup.contains("weight=\"bold\">kapitlet</span>"));
        assert!(markup.contains("<span underline=\"single\" weight=\"bold\">chapter-the</span>"));
    }

    #[test]
    fn sentence_splitting_uses_terminal_punctuation_and_clip_end() {
        let timing = Timing {
            duration_us: 1_500_000,
            sentence_starts_us: Vec::new(),
            words: vec![
                WordTiming {
                    word: "Det".into(),
                    start_us: 50_000,
                    end_us: 200_000,
                },
                WordTiming {
                    word: "regnar!".into(),
                    start_us: 220_000,
                    end_us: 500_000,
                },
                WordTiming {
                    word: "Går".into(),
                    start_us: 700_000,
                    end_us: 900_000,
                },
                WordTiming {
                    word: "vi?".into(),
                    start_us: 950_000,
                    end_us: 1_300_000,
                },
            ],
        };
        assert_eq!(
            timing.sentences(),
            vec![
                Sentence {
                    start_us: 0,
                    end_us: 500_000,
                },
                Sentence {
                    start_us: 700_000,
                    end_us: 1_500_000,
                },
            ]
        );
        assert_eq!(
            timings().sentences(),
            vec![Sentence {
                start_us: 0,
                end_us: 1_000_000,
            }]
        );
    }
}
