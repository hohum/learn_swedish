use super::Timing;
use adw::glib;
use learn_swedish::adventure::Actor;
use learn_swedish::lesson::{Phrase, Voice};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const WORKER: &str = include_str!("../../scripts/live_azure_speech.py");
const DEFAULT_REGION: &str = "australiaeast";
const F0_WINDOW: Duration = Duration::from_secs(60);
const F0_REQUESTS: usize = 20;

struct Credentials {
    region: String,
    key: String,
}

#[derive(Default)]
pub struct AzureSpeech {
    credentials: RefCell<Option<Credentials>>,
    cache: RefCell<Option<Cache>>,
    requests: RefCell<VecDeque<Instant>>,
    #[cfg(test)]
    worker: Option<&'static str>,
}

impl AzureSpeech {
    #[cfg(test)]
    pub(super) fn with_worker(worker: &'static str) -> Self {
        Self {
            worker: Some(worker),
            ..Self::default()
        }
    }

    pub fn from_env() -> Self {
        let azure = Self::default();
        if let Ok(key) = std::env::var("SPEECH_KEY").or_else(|_| std::env::var("AZURE_SPEECH_KEY"))
        {
            let _ = azure.configure(&environment_region(), &key);
        }
        azure
    }

    pub(super) fn enabled(&self) -> bool {
        self.credentials.borrow().is_some()
    }

    pub(super) fn region(&self) -> String {
        self.credentials
            .borrow()
            .as_ref()
            .map_or_else(environment_region, |config| config.region.clone())
    }

    pub(super) fn configure(&self, region: &str, key: &str) -> Result<(), &'static str> {
        let region = region.trim().to_ascii_lowercase();
        if !(2..=28).contains(&region.len())
            || !region.as_bytes()[0].is_ascii_lowercase()
            || !region
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err("Enter your resource region, for example australiaeast or swedencentral.");
        }
        let mut config = self.credentials.borrow_mut();
        let key = key.trim();
        let key = if key.is_empty() {
            config.as_ref().map_or("", |config| config.key.as_str())
        } else {
            key
        };
        if key.is_empty() {
            return Err("Enter the key for your Azure Speech resource.");
        }
        *config = Some(Credentials {
            region,
            key: key.to_owned(),
        });
        Ok(())
    }

    pub(super) fn disable(&self) {
        self.credentials.take();
    }

    pub(super) fn cached(&self, utterances: &[Utterance]) -> Option<Vec<Recording>> {
        let cache = self.cache.borrow();
        let cache = cache.as_ref()?;
        utterances
            .iter()
            .map(|key| cache.recordings.get(key).cloned())
            .collect()
    }

    pub(super) fn request(&self, utterances: &[Utterance]) -> Result<Request, String> {
        let config = self.credentials.borrow();
        let config = config
            .as_ref()
            .ok_or("Enter your Azure Speech key first.")?;
        let mut requests = self.requests.borrow_mut();
        let now = Instant::now();
        while requests
            .front()
            .is_some_and(|then| now.duration_since(*then) >= F0_WINDOW)
        {
            requests.pop_front();
        }
        if requests.len() >= F0_REQUESTS {
            let seconds = (F0_WINDOW - now.duration_since(requests[0])).as_secs() + 1;
            return Err(format!(
                "Azure F0 request limit reached. Try Replay in {seconds} seconds."
            ));
        }
        let mut cache = self.cache.borrow_mut();
        if cache.is_none() {
            *cache =
                Some(Cache::new().map_err(|_| "Could not create a temporary Azure audio folder.")?);
        }
        let cache = cache.as_ref().expect("Azure cache exists");
        let mut seen = HashSet::new();
        let missing: Vec<_> = utterances
            .iter()
            .filter(|line| !cache.recordings.contains_key(*line) && seen.insert(*line))
            .cloned()
            .collect();
        let folder = CacheFolder::in_directory(&cache.root.0)
            .map_err(|_| "Could not create a temporary Azure audio folder.")?;
        let lines: Vec<_> = missing
            .iter()
            .map(|line| {
                serde_json::json!({
                    "text": line.text,
                    "voice": line.style.voice.azure_name(),
                    "pitch": line.style.pitch,
                    "rate": line.style.rate,
                })
            })
            .collect();
        let stdin = serde_json::json!({"region": config.region, "key": config.key, "lines": lines})
            .to_string();
        let python = python_executable();
        #[cfg(test)]
        let worker = self.worker.unwrap_or(WORKER);
        #[cfg(not(test))]
        let worker = WORKER;
        let mut command = Command::new(python);
        command
            .args(["-B", "-X", "utf8", "-c", worker])
            .arg(&folder.0)
            .env_remove("SPEECH_KEY")
            .env_remove("AZURE_SPEECH_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut process = Worker(command.spawn().map_err(|_| sdk_help())?);
        requests.push_back(now);
        let mut input = process
            .0
            .stdin
            .take()
            .expect("Azure worker has a stdin pipe");
        std::thread::Builder::new()
            .name("azure-speech-input".into())
            .spawn(move || {
                let _ = input.write_all(stdin.as_bytes());
            })
            .map_err(|_| "Could not send the page to the Azure Speech worker.")?;
        Ok(Request {
            process,
            started: Instant::now(),
            folder,
            missing,
        })
    }

    pub(super) fn save(&self, request: Request) -> Result<(), String> {
        let recordings: Vec<_> = request
            .missing
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let audio = request.folder.0.join(format!("{index}.wav"));
                let timing = Timing::load_online(&audio.with_extension("json"), &line.text)?;
                if !audio.is_file() {
                    return Err(io::Error::other("Missing Azure WAV").into());
                }
                Ok((line.clone(), Recording { audio, timing }))
            })
            .collect::<Result<_, Box<dyn std::error::Error>>>()
            .map_err(|_| "Azure did not return usable speech for this page. Try Replay.")?;
        let mut cache = self.cache.borrow_mut();
        let cache = cache
            .as_mut()
            .expect("The audio cache owns request folders");
        cache.recordings.extend(recordings);
        cache.folders.push(request.folder);
        Ok(())
    }
}

fn environment_region() -> String {
    std::env::var("SPEECH_REGION")
        .or_else(|_| std::env::var("AZURE_SPEECH_REGION"))
        .unwrap_or_else(|_| DEFAULT_REGION.to_owned())
}

fn python_executable() -> PathBuf {
    if let Some(executable) = std::env::var_os("AZURE_SPEECH_PYTHON") {
        return executable.into();
    }
    let data = glib::user_data_dir().join("learn-swedish");
    #[cfg(windows)]
    let candidates = [
        data.join("azure-env/Scripts/python.exe"),
        std::env::var_os("LOCALAPPDATA")
            .map(|path| PathBuf::from(path).join("learn-swedish"))
            .unwrap_or(data)
            .join("azure-env/Scripts/python.exe"),
    ];
    #[cfg(not(windows))]
    let candidates = [data.join("azure-env/bin/python")];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "py" } else { "python3" }))
}

fn sdk_help() -> &'static str {
    "Azure Speech needs the Python Speech SDK. See README’s one-time live speech setup."
}

fn worker_error(code: &str) -> &'static str {
    match code.trim() {
        "sdk" => sdk_help(),
        "authentication" => "Azure rejected the key or region. Check the Azure Speech settings.",
        "quota" => "Azure is limiting requests. Wait a minute and try Replay.",
        "connection" => "Could not reach Azure Speech. Check your internet connection and region.",
        "audio" => "Azure did not return usable speech for this page. Try Replay.",
        _ => "Azure Speech could not read this page. Try Replay.",
    }
}

struct CacheFolder(PathBuf);

impl CacheFolder {
    fn in_directory(parent: &Path) -> io::Result<Self> {
        let path = parent.join(format!("learn-swedish-{}", glib::uuid_string_random()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for CacheFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Cache {
    root: CacheFolder,
    folders: Vec<CacheFolder>,
    recordings: HashMap<Utterance, Recording>,
}

impl Cache {
    fn new() -> io::Result<Self> {
        Ok(Self {
            root: CacheFolder::in_directory(&glib::tmp_dir())?,
            folders: Vec::new(),
            recordings: HashMap::new(),
        })
    }
}

pub(super) struct Request {
    process: Worker,
    started: Instant,
    folder: CacheFolder,
    missing: Vec<Utterance>,
}

impl Request {
    pub(super) fn poll(&mut self) -> Option<Result<(), String>> {
        match self.process.0.try_wait() {
            Ok(None) if self.started.elapsed() < Duration::from_secs(25) => return None,
            Ok(None) => {
                return Some(Err(
                    "Azure Speech took too long to respond. Try Replay.".into()
                ));
            }
            Ok(Some(status)) if status.success() => {}
            _ => return Some(Err(worker_error("").into())),
        }
        let mut code = String::new();
        if let Some(output) = self.process.0.stdout.take() {
            let _ = output.take(128).read_to_string(&mut code);
        }
        Some(if code.trim() == "ok" {
            Ok(())
        } else {
            Err(worker_error(&code).into())
        })
    }
}

struct Worker(Child);

impl Drop for Worker {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[derive(Clone)]
pub(super) struct Recording {
    pub(super) audio: PathBuf,
    pub(super) timing: Timing,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct Utterance {
    text: String,
    style: Style,
}

impl Utterance {
    pub(super) fn voice(&self) -> Voice {
        self.style.voice
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Style {
    voice: Voice,
    pitch: i8,
    rate: i8,
}

#[derive(Default)]
pub(super) struct Cast {
    guests: HashMap<&'static str, Style>,
}

impl Cast {
    pub(super) fn utterance(&mut self, phrase: &Phrase) -> Utterance {
        Utterance {
            text: phrase.swedish.clone(),
            style: self.style(phrase.speaker.expect("Story phrases have speakers")),
        }
    }

    fn style(&mut self, speaker: &'static str) -> Style {
        if speaker == Actor::Narrator.name() || speaker == Actor::Husband.name() {
            return Style {
                voice: Voice::Mattias,
                pitch: 0,
                rate: 0,
            };
        }
        if speaker == Actor::Wife.name() {
            return Style {
                voice: Voice::Sofie,
                pitch: 0,
                rate: 0,
            };
        }
        if let Some(style) = self.guests.get(speaker) {
            return *style;
        }
        let voices: &[Voice] = if [Actor::Police, Actor::ShopAssistant, Actor::BusDriver]
            .iter()
            .any(|actor| actor.name() == speaker)
        {
            &[Voice::Sofie, Voice::Hillevi]
        } else if [Actor::Conductor, Actor::DogWalker, Actor::Passenger]
            .iter()
            .any(|actor| actor.name() == speaker)
        {
            &[Voice::Mattias]
        } else {
            &[Voice::Sofie, Voice::Mattias, Voice::Hillevi]
        };
        // Pick randomly among the least-used guest voices. Prefer a female voice on
        // an otherwise equal choice because Mattias already reads two lead roles.
        let rank = |voice| {
            let count = self
                .guests
                .values()
                .filter(|style| style.voice == voice)
                .count();
            (count, voice == Voice::Mattias)
        };
        let best = voices.iter().copied().map(rank).min().unwrap();
        let candidates: Vec<_> = voices
            .iter()
            .copied()
            .filter(|voice| rank(*voice) == best)
            .collect();
        let voice = candidates[random_index(candidates.len())];
        let presets = [(-5, -7), (4, 4), (-3, 3), (6, -5), (2, -10), (-6, 5)];
        let presets = presets.map(|(pitch, rate)| Style { voice, pitch, rate });
        let available: Vec<_> = presets
            .into_iter()
            .filter(|style| !self.guests.values().any(|other| other == style))
            .collect();
        let available = if available.is_empty() {
            &presets[..]
        } else {
            &available
        };
        let style = available[random_index(available.len())];
        self.guests.insert(speaker, style);
        style
    }
}

fn random_index(upper: usize) -> usize {
    glib::random_int_range(0, upper as i32) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_cast_is_fixed_and_guest_sofie_has_her_own_delivery() {
        let mut cast = Cast::default();
        let husband = cast.style(Actor::Husband.name());
        let wife = cast.style(Actor::Wife.name());
        assert_eq!(husband, cast.style(Actor::Narrator.name()));
        assert_eq!(husband.voice, Voice::Mattias);
        assert_eq!(
            wife,
            Style {
                voice: Voice::Sofie,
                pitch: 0,
                rate: 0
            }
        );
        let shop = cast.style(Actor::ShopAssistant.name());
        let driver = cast.style(Actor::BusDriver.name());
        assert!([shop.voice, driver.voice].contains(&Voice::Hillevi));
        let sofie = [shop, driver]
            .into_iter()
            .find(|style| style.voice == Voice::Sofie)
            .unwrap();
        assert_ne!(sofie.pitch, wife.pitch);
        assert_ne!(sofie.rate, wife.rate);
        assert_eq!(driver, cast.style(Actor::BusDriver.name()));
    }

    #[test]
    fn the_local_f0_limit_is_checked_before_starting_a_worker() {
        let azure = AzureSpeech::default();
        assert!(
            azure
                .configure("https://example.invalid", "test-key")
                .is_err()
        );
        azure.configure("AustraliaEast", "test-key").unwrap();
        assert_eq!(azure.region(), "australiaeast");
        azure
            .requests
            .borrow_mut()
            .extend([Instant::now(); F0_REQUESTS]);
        assert!(matches!(azure.request(&[]), Err(error) if error.contains("F0 request limit")));
    }
}
