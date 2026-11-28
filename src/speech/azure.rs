use super::Timing;
use adw::glib;
use learn_swedish::adventure::ActorSpec;
use learn_swedish::lesson::{Phrase, Voice};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const WORKER: &str = include_str!("../../scripts/live_azure_speech.py");
const DEFAULT_REGION: &str = "australiaeast";
const F0_WINDOW: Duration = Duration::from_secs(60);
const F0_REQUESTS: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Credentials {
    region: String,
    key: String,
}

#[derive(Default)]
pub struct AzureSpeech {
    credentials: RefCell<Option<Credentials>>,
    saved: RefCell<Option<Credentials>>,
    storage_path: Option<PathBuf>,
    persistent_cache_dir: Option<PathBuf>,
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

    #[cfg(test)]
    pub(super) fn with_worker_and_paths(
        worker: &'static str,
        storage_path: PathBuf,
        cache_dir: PathBuf,
    ) -> Self {
        let mut azure = Self::with_paths(Some(storage_path), Some(cache_dir));
        azure.worker = Some(worker);
        azure
    }

    pub fn load_startup() -> Self {
        let azure = Self::with_paths(
            Some(default_credentials_path()),
            Some(default_tts_cache_dir()),
        );
        if let Ok(key) = std::env::var("SPEECH_KEY").or_else(|_| std::env::var("AZURE_SPEECH_KEY"))
        {
            let _ = azure.configure_session_only(&environment_region(), &key);
        }
        azure
    }

    fn with_paths(storage_path: Option<PathBuf>, persistent_cache_dir: Option<PathBuf>) -> Self {
        let saved = storage_path
            .as_deref()
            .and_then(load_saved_credentials)
            .filter(|creds| validate_region(&creds.region).is_ok() && !creds.key.trim().is_empty());
        Self {
            credentials: RefCell::new(saved.clone()),
            saved: RefCell::new(saved),
            storage_path,
            persistent_cache_dir,
            cache: RefCell::new(None),
            requests: RefCell::new(VecDeque::new()),
            #[cfg(test)]
            worker: None,
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.credentials.borrow().is_some()
    }

    pub(super) fn remembered(&self) -> bool {
        self.saved.borrow().is_some()
    }

    pub(super) fn has_available_key(&self) -> bool {
        self.credentials.borrow().is_some() || self.saved.borrow().is_some()
    }

    pub(super) fn region(&self) -> String {
        if let Some(config) = self.credentials.borrow().as_ref() {
            return config.region.clone();
        }
        if let Some(saved) = self.saved.borrow().as_ref() {
            return saved.region.clone();
        }
        environment_region()
    }

    fn configure_session_only(&self, region: &str, key: &str) -> Result<(), &'static str> {
        let region = validate_region(region)?;
        let key = key.trim();
        if key.is_empty() {
            return Err("Enter the key for your Azure Speech resource.");
        }
        *self.credentials.borrow_mut() = Some(Credentials {
            region,
            key: key.to_owned(),
        });
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn configure(&self, region: &str, key: &str) -> Result<(), &'static str> {
        self.configure_persistent(region, key, self.storage_path.is_some())
    }

    pub(super) fn configure_persistent(
        &self,
        region: &str,
        key: &str,
        remember: bool,
    ) -> Result<(), &'static str> {
        let region = validate_region(region)?;
        let mut config = self.credentials.borrow_mut();
        let saved_borrow = self.saved.borrow();
        let trimmed = key.trim();
        let resolved_key = if !trimmed.is_empty() {
            trimmed.to_owned()
        } else if let Some(active) = config.as_ref() {
            active.key.clone()
        } else if let Some(saved) = saved_borrow.as_ref() {
            saved.key.clone()
        } else {
            String::new()
        };
        drop(saved_borrow);
        if resolved_key.is_empty() {
            return Err("Enter the key for your Azure Speech resource.");
        }
        let next = Credentials {
            region,
            key: resolved_key,
        };
        if remember {
            if let Some(path) = &self.storage_path {
                save_credentials_file(path, &next).map_err(
                    |_| "Could not save the Azure Speech key to your user config folder.",
                )?;
            }
            *self.saved.borrow_mut() = Some(next.clone());
        } else {
            if let Some(path) = &self.storage_path {
                let _ = fs::remove_file(path);
            }
            *self.saved.borrow_mut() = None;
        }
        *config = Some(next);
        Ok(())
    }

    pub(super) fn disable(&self) {
        self.credentials.take();
    }

    pub(super) fn forget(&self) {
        self.credentials.take();
        self.saved.take();
        if let Some(path) = &self.storage_path {
            let _ = fs::remove_file(path);
        }
    }

    pub(super) fn cached_clip_count(&self) -> usize {
        if let Some(dir) = &self.persistent_cache_dir {
            count_wav_files(dir)
        } else {
            self.cache
                .borrow()
                .as_ref()
                .map_or(0, |cache| cache.recordings.len())
        }
    }

    pub(super) fn clear_cache(&self) {
        if let Some(cache) = self.cache.borrow_mut().as_mut() {
            cache.recordings.clear();
            cache.folders.clear();
        }
        if let Some(dir) = &self.persistent_cache_dir
            && dir.is_dir()
        {
            let _ = fs::remove_dir_all(dir);
        }
    }

    fn ensure_cache(&self) -> Result<(), &'static str> {
        let mut cache = self.cache.borrow_mut();
        if cache.is_none() {
            *cache =
                Some(Cache::new().map_err(|_| "Could not create a temporary Azure audio folder.")?);
        }
        Ok(())
    }

    fn lookup_persistent(&self, line: &Utterance) -> Option<Recording> {
        let root = self.persistent_cache_dir.as_ref()?;
        let hash = line.cache_key();
        let shard = root.join(&hash[..2]);
        let audio = shard.join(format!("{hash}.wav"));
        let json = shard.join(format!("{hash}.json"));
        if !is_valid_wav_file(&audio) {
            return None;
        }
        let timing = Timing::load_online(&json, &line.text).ok()?;
        Some(Recording { audio, timing })
    }

    fn hydrate_from_disk(&self, utterances: &[Utterance]) {
        if self.persistent_cache_dir.is_none() || self.ensure_cache().is_err() {
            return;
        }
        let mut cache = self.cache.borrow_mut();
        let Some(cache) = cache.as_mut() else {
            return;
        };
        for line in utterances {
            if let Some(existing) = cache.recordings.get(line)
                && existing.audio.is_file()
            {
                continue;
            }
            if let Some(recording) = self.lookup_persistent(line) {
                cache.recordings.insert(line.clone(), recording);
            } else {
                cache.recordings.remove(line);
            }
        }
    }

    pub(super) fn cached(&self, utterances: &[Utterance]) -> Option<Vec<Recording>> {
        self.hydrate_from_disk(utterances);
        let cache = self.cache.borrow();
        let cache = cache.as_ref()?;
        utterances
            .iter()
            .map(|key| {
                cache
                    .recordings
                    .get(key)
                    .filter(|rec| rec.audio.is_file())
                    .cloned()
            })
            .collect()
    }

    pub(super) fn request(&self, utterances: &[Utterance]) -> Result<Request, String> {
        let config = self.credentials.borrow();
        let config = config
            .as_ref()
            .ok_or("Enter your Azure Speech key first.")?;
        self.hydrate_from_disk(utterances);
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
        self.ensure_cache()?;
        let cache_borrow = self.cache.borrow();
        let cache = cache_borrow.as_ref().expect("Azure cache exists");
        let mut seen = HashSet::new();
        let missing: Vec<_> = utterances
            .iter()
            .filter(|line| {
                !cache
                    .recordings
                    .get(*line)
                    .is_some_and(|rec| rec.audio.is_file())
                    && seen.insert(*line)
            })
            .cloned()
            .collect();
        let folder = CacheFolder::in_directory(&cache.root.0)
            .map_err(|_| "Could not create a temporary Azure audio folder.")?;
        drop(cache_borrow);
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
        let mut recordings = Vec::with_capacity(request.missing.len());
        let mut keep_temp_folder = false;
        for (index, line) in request.missing.iter().enumerate() {
            let staged_audio = request.folder.0.join(format!("{index}.wav"));
            let staged_json = staged_audio.with_extension("json");
            let timing = Timing::load_online(&staged_json, &line.text)
                .map_err(|_| "Azure did not return usable speech for this page. Try Replay.")?;
            if !is_valid_wav_file(&staged_audio) {
                return Err("Azure did not return usable speech for this page. Try Replay.".into());
            }
            let final_audio = if let Some(root) = &self.persistent_cache_dir {
                persist_clip_to_disk(root, line, &staged_audio, &staged_json).unwrap_or_else(|_| {
                    keep_temp_folder = true;
                    staged_audio.clone()
                })
            } else {
                keep_temp_folder = true;
                staged_audio
            };
            recordings.push((
                line.clone(),
                Recording {
                    audio: final_audio,
                    timing,
                },
            ));
        }
        let mut cache = self.cache.borrow_mut();
        let cache = cache
            .as_mut()
            .expect("The audio cache owns request folders");
        cache.recordings.extend(recordings);
        if keep_temp_folder {
            cache.folders.push(request.folder);
        }
        Ok(())
    }
}

fn validate_region(region: &str) -> Result<String, &'static str> {
    let region = region.trim().to_ascii_lowercase();
    if !(2..=28).contains(&region.len())
        || !region.as_bytes()[0].is_ascii_lowercase()
        || !region
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err("Enter your resource region, for example australiaeast or swedencentral.");
    }
    Ok(region)
}

fn default_credentials_path() -> PathBuf {
    glib::user_config_dir()
        .join("learn-swedish")
        .join("azure-speech.json")
}

fn default_tts_cache_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(glib::user_data_dir)
            .join("learn-swedish")
            .join("tts-cache")
            .join("v2")
    }
    #[cfg(not(windows))]
    {
        glib::user_cache_dir().join("learn-swedish").join("tts-v2")
    }
}

fn load_saved_credentials(path: &Path) -> Option<Credentials> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_credentials_file(path: &Path, creds: &Credentials) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid credentials path"))?;
    #[cfg(unix)]
    {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(parent)?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(parent)?;

    let json = serde_json::to_vec_pretty(creds).map_err(io::Error::other)?;
    let temp = parent.join(format!("azure-speech-{}.tmp", glib::uuid_string_random()));
    let write_res = (|| -> io::Result<()> {
        #[cfg(unix)]
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        #[cfg(not(unix))]
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&json)?;
        file.sync_all()
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    if let Err(err) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    Ok(())
}

fn is_valid_wav_file(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    if meta.len() <= 44 {
        return false;
    }
    let mut header = [0_u8; 12];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    &header[..4] == b"RIFF" && &header[8..12] == b"WAVE"
}

fn persist_clip_to_disk(
    root: &Path,
    line: &Utterance,
    staged_audio: &Path,
    staged_json: &Path,
) -> io::Result<PathBuf> {
    let hash = line.cache_key();
    let shard = root.join(&hash[..2]);
    fs::create_dir_all(&shard)?;
    let final_wav = shard.join(format!("{hash}.wav"));
    let final_json = shard.join(format!("{hash}.json"));
    let nonce = glib::uuid_string_random();
    let tmp_wav = shard.join(format!("{hash}.{nonce}.tmp.wav"));
    let tmp_json = shard.join(format!("{hash}.{nonce}.tmp.json"));
    fs::copy(staged_audio, &tmp_wav)?;
    if let Err(err) = fs::copy(staged_json, &tmp_json) {
        let _ = fs::remove_file(&tmp_wav);
        return Err(err);
    }
    if let Err(err) = fs::rename(&tmp_json, &final_json) {
        let _ = fs::remove_file(&tmp_wav);
        let _ = fs::remove_file(&tmp_json);
        return Err(err);
    }
    if let Err(err) = fs::rename(&tmp_wav, &final_wav) {
        let _ = fs::remove_file(&tmp_wav);
        let _ = fs::remove_file(&final_json);
        return Err(err);
    }
    Ok(final_wav)
}

fn count_wav_files(dir: &Path) -> usize {
    let Ok(shards) = fs::read_dir(dir) else {
        return 0;
    };
    shards
        .flatten()
        .filter_map(|entry| fs::read_dir(entry.path()).ok())
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "wav"))
        .count()
}

fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0_u32; 64];
        for (i, &word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (state, delta) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(delta);
        }
    }
    let mut out = String::with_capacity(64);
    for word in h {
        let _ = write!(out, "{word:08x}");
    }
    out
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

    fn cache_key(&self) -> String {
        let payload = format!(
            "learn-swedish-tts-v2\0{}\0{}\0{}\0{}",
            self.style.voice.azure_name(),
            self.style.pitch,
            self.style.rate,
            self.text
        );
        sha256_hex(payload.as_bytes())
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
    specs: Vec<ActorSpec>,
    guests: HashMap<String, Style>,
}

impl Cast {
    pub(super) fn set_specs(&mut self, specs: Vec<ActorSpec>) {
        self.specs = specs;
        self.guests.clear();
    }

    pub(super) fn recast_guests(&mut self) {
        self.guests.clear();
    }

    pub(super) fn utterance(&mut self, phrase: &Phrase) -> Utterance {
        Utterance {
            text: phrase.swedish.clone(),
            style: self.style(
                phrase
                    .speaker
                    .as_deref()
                    .expect("Story phrases have speakers"),
            ),
        }
    }

    fn style(&mut self, speaker: &str) -> Style {
        let spec = self
            .specs
            .iter()
            .find(|spec| spec.label == speaker || spec.id == speaker);
        if let Some(spec) = spec
            && spec.lead
        {
            return Style {
                voice: spec.voices[0],
                pitch: 0,
                rate: 0,
            };
        }
        if let Some(style) = self.guests.get(speaker) {
            return *style;
        }
        let voices: Vec<Voice> = spec.map_or_else(
            || vec![Voice::Sofie, Voice::Mattias, Voice::Hillevi],
            |spec| spec.voices.clone(),
        );
        // Pick randomly among the least-used guest voices, deprioritizing any voice
        // that already carries multiple lead roles.
        let rank = |voice: Voice| {
            let guest_count = self
                .guests
                .values()
                .filter(|style| style.voice == voice)
                .count();
            let lead_count = self
                .specs
                .iter()
                .filter(|spec| spec.lead && spec.voices.first() == Some(&voice))
                .count();
            (
                guest_count,
                lead_count > 1 || (self.specs.is_empty() && voice == Voice::Mattias),
            )
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
        self.guests.insert(speaker.to_owned(), style);
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
        cast.set_specs(vec![
            ActorSpec {
                id: "narrator".into(),
                label: "Narrator".into(),
                lead: true,
                voices: vec![Voice::Mattias],
            },
            ActorSpec {
                id: "protagonist".into(),
                label: "Protagonist".into(),
                lead: true,
                voices: vec![Voice::Mattias],
            },
            ActorSpec {
                id: "companion".into(),
                label: "Companion".into(),
                lead: true,
                voices: vec![Voice::Sofie],
            },
            ActorSpec {
                id: "guest_one".into(),
                label: "Guest One".into(),
                lead: false,
                voices: vec![Voice::Hillevi, Voice::Sofie, Voice::Mattias],
            },
            ActorSpec {
                id: "guest_two".into(),
                label: "Guest Two".into(),
                lead: false,
                voices: vec![Voice::Hillevi, Voice::Sofie, Voice::Mattias],
            },
        ]);
        let protagonist = cast.style("Protagonist");
        let companion = cast.style("Companion");
        assert_eq!(protagonist, cast.style("Narrator"));
        assert_eq!(protagonist.voice, Voice::Mattias);
        assert_eq!(
            companion,
            Style {
                voice: Voice::Sofie,
                pitch: 0,
                rate: 0
            }
        );
        let first_guest = cast.style("Guest One");
        let second_guest = cast.style("Guest Two");
        assert!([first_guest.voice, second_guest.voice].contains(&Voice::Hillevi));
        let sofie = [first_guest, second_guest]
            .into_iter()
            .find(|style| style.voice == Voice::Sofie)
            .unwrap();
        assert_ne!(sofie.pitch, companion.pitch);
        assert_ne!(sofie.rate, companion.rate);
        assert_eq!(second_guest, cast.style("Guest Two"));
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

    #[test]
    fn sha256_matches_standard_test_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn remembered_credentials_persist_and_default_to_enabled_on_reload() {
        let dir = std::env::temp_dir().join(format!(
            "learn-swedish-creds-test-{}",
            glib::uuid_string_random()
        ));
        let creds_path = dir.join("config").join("azure-speech.json");
        let cache_dir = dir.join("cache");

        let first = AzureSpeech::with_paths(Some(creds_path.clone()), Some(cache_dir.clone()));
        assert!(!first.enabled());
        assert!(!first.remembered());

        first
            .configure_persistent("SwedenCentral", "f0-secret-key", true)
            .unwrap();
        assert!(first.enabled());
        assert!(first.remembered());
        assert_eq!(first.region(), "swedencentral");
        assert!(creds_path.is_file());

        // Reloading from disk defaults to enabled automatically.
        let reloaded = AzureSpeech::with_paths(Some(creds_path.clone()), Some(cache_dir.clone()));
        assert!(reloaded.enabled());
        assert!(reloaded.remembered());
        assert!(reloaded.has_available_key());
        assert_eq!(reloaded.region(), "swedencentral");

        // Turning off speech for the session keeps the saved key available for re-enable/next launch.
        reloaded.disable();
        assert!(!reloaded.enabled());
        assert!(reloaded.remembered());
        assert!(reloaded.has_available_key());
        reloaded
            .configure_persistent("australiaeast", "", true)
            .unwrap();
        assert!(reloaded.enabled());
        assert_eq!(reloaded.region(), "australiaeast");

        // Forgetting removes the saved file from disk and disables speech.
        reloaded.forget();
        assert!(!reloaded.enabled());
        assert!(!reloaded.remembered());
        assert!(!creds_path.exists());

        let after_forget = AzureSpeech::with_paths(Some(creds_path), Some(cache_dir));
        assert!(!after_forget.enabled());
        assert!(!after_forget.remembered());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn persistent_tts_cache_reuses_audio_across_instances_and_clears_cleanly() {
        let dir = std::env::temp_dir().join(format!(
            "learn-swedish-tts-cache-test-{}",
            glib::uuid_string_random()
        ));
        let creds_path = dir.join("azure-speech.json");
        let cache_dir = dir.join("tts-v1");
        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();

        let utterance = Utterance {
            text: "Hej och välkommen!".into(),
            style: Style {
                voice: Voice::Sofie,
                pitch: 0,
                rate: 0,
            },
        };
        let mut wav = b"RIFF\x24\x00\x00\x00WAVEfmt ".to_vec();
        wav.resize(64, 0);
        let staged_wav = staging.join("0.wav");
        let staged_json = staging.join("0.json");
        fs::write(&staged_wav, &wav).unwrap();
        fs::write(
            &staged_json,
            r#"{"duration_us":600000,"sentence_starts_us":[0],"words":[{"word":"Hej","start_us":10000,"end_us":180000},{"word":"och","start_us":200000,"end_us":320000},{"word":"välkommen!","start_us":340000,"end_us":580000}]}"#,
        )
        .unwrap();
        persist_clip_to_disk(&cache_dir, &utterance, &staged_wav, &staged_json).unwrap();

        let fresh = AzureSpeech::with_paths(Some(creds_path), Some(cache_dir));
        assert_eq!(fresh.cached_clip_count(), 1);
        let hit = fresh
            .cached(std::slice::from_ref(&utterance))
            .expect("Persistent disk cache should hit in a new session");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].timing.words.len(), 3);

        fresh.clear_cache();
        assert_eq!(fresh.cached_clip_count(), 0);
        assert!(fresh.cached(std::slice::from_ref(&utterance)).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
