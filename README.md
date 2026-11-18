# Swedish Keys

A native Rust GTK4/libadwaita app for learning the Swedish PC keyboard on
Ubuntu GNOME and Windows. The interface is English, with Swedish names beside each prompt.
No word associations are used.

The default window is **900 × 600 logical pixels**, designed to sit comfortably
on a 1366 × 768 desktop without maximizing, leaving room for desktop panels.
Practice and input are on the left;
the bilingual follow-along text is on the right, with compact settings below.
The window remains resizable, with scrolling available for smaller work areas
or larger system text/scaling settings.

## Phase 1

- **Level 1:** untimed. **Automatic next** is on by default: a correct answer
  plays feedback and a randomly selected next-card announcement before advancing.
  Turn it off to keep each correct answer on screen until you press **Next**.
  You can also skip a card, replay its Swedish name, or restart the set.
- Practice the 29-letter alphabet (A–Z, Å, Ä, Ö), digits 0–9, or 39 symbols
  and whitespace characters used in Linux administration and programming.
- Every session shuffles the selected set, showing each character once.
  Restarting or switching sets creates a fresh shuffle.
- **Everything** is the default practice set, mixing letters, numbers and symbols.
- Large symbol, Swedish name hints, and Swedish-layout key combinations.
  **Show English hints** is off by default; enable it to reveal the English
  name below the symbol and English translations in **Listen & follow**.
  Toggling it also updates the current transcript without interrupting speech.
- Offline neural Swedish speech from prepared **Azure Sofie** recordings,
  or the alternative Piper **NST** and **Alma** voices.
  A complete Azure library takes precedence over Piper; Sofie is then used
  consistently. With Piper, a voice is picked randomly for each prompt or answer
  sequence. The typed character and its feedback always use the same voice.
  The app speaks both the displayed prompt and what you type, even for wrong
  answers. These have separate checkboxes; **Replay Swedish** always repeats
  the displayed prompt.
- After each answer, one of 16 positive or 16 encouraging retry phrases is
  selected at random, such as **Bra jobbat! / Good job!** or
  **Försök igen. / Try again.** The typed character is spoken first.
  Informal examples include **Grymt! / Awesome!**, **Klockrent! / Spot on!**,
  **Där satt den! / You nailed it!**, and **Hoppsan! / Oops!**
- Everything spoken is shown in Swedish and English in **Listen & follow**.
  Swedish words are highlighted as they are spoken, using Azure's word boundaries
  or Piper's phoneme alignments and the actual audio playback clock.
- Session-only correct, skipped, and mistake counts. Restarting or changing the
  practice set resets them; nothing is saved.

Levels 2–10 (progressively faster practice) and microphone recognition are
**not implemented yet**. No microphone is opened. Practice runs entirely offline;
only Azure preparation sends the fixed Swedish curriculum text to Microsoft.

## Story adventure

At startup, choose **Välj ditt eget äventyr / Choose your own adventure** to open
**En lagom äventyrlig lördag**
("A moderately adventurous Saturday"). You are the husband; your Swedish-speaking
wife helps you find the words. Every outing starts at home and returns home:

- **The woods:** rain gear, muddy paths, a dog walker, unfamiliar berries,
  a toilet stop and a rainy walk home.
- **Willys:** a shopping list, kilos versus individual items, finding the dairy
  aisle, choosing milk, sold-out bread, a lost phone, bags and receipts.
- **IKEA by car:** staying calm in traffic, a police check, parking signs,
  furniture measurements, warehouse directions, toilets and loading safely.
- **Gothenburg by train:** tickets, delays, a cancelled train and replacement
  bus, ordering fika, sharing a café table, a toilet code and the journey home.

Each illustrated page contains Swedish narration, dialogue and a question with
four responses. Click a response, enter **1–4**, or type the complete Swedish
response and press **Enter**. Case and final `.?!` do not matter; Swedish spelling
and accents still do. Wrong choices receive a dry, good-natured reply from your
wife, then **Try again** returns to the same question. **Continue** follows the
chosen route after a successful answer. Destination choices are all valid.
Nothing records microphone audio, and typed answers are never sent to Azure.
Progress is session-only; **Restart book** returns to the beginning.

The silent start menu also offers **Tangentbordsövning / Keyboard practice**.
Choose **Meny / Menu** in either activity to return to the start menu; opening an
activity again starts a fresh session.

**Show English hints** is off by default and controls both the story transcript
and response translations. **Read aloud** reads the page, its question and all
four options, then reads your selected response and the reply. Responses are
locked during speech; turn off Read aloud to stop playback and read at your own
pace. Replay repeats the current page or feedback. Highlighting uses measured
word timings and automatically follows the current passage in its scroll area.
Male roles use **Azure Mattias**; female roles, including your wife, use
**Azure Sofie**. The narrator is male. Several characters share each voice,
but their Swedish role labels identify who is speaking.

The original location illustrations are drawn as scalable vector scenes by the
app: home, forest, supermarket, road, furniture store, railway and city café.
They need no downloads, network access or external image models.

### Prepare the adventure voices

The keyboard voice bank does **not** contain the story dialogue. First complete
the Azure Python environment setup below, close the app, and run in PowerShell
from the project directory:

```powershell
wsl -d Ubuntu -- cargo run --locked -- --prepare-adventure-speech
```

In an Ubuntu terminal, use `cargo run --locked -- --prepare-adventure-speech`;
an installed binary from an updated build accepts
`learn-swedish --prepare-adventure-speech`.
Enter the key only at the hidden terminal prompt. The command displays how many
fixed recordings it will reuse or generate. A full story requires many more
recordings than keyboard practice; allow **tens of minutes** at F0's request
limit and check your quota/pricing before starting. Only the authored Swedish
story is sent, with the selected prebuilt voice for each line. No credentials
are stored. Generation remains below 20 requests per minute.

Story audio is kept separately under
`~/.local/share/learn-swedish/speech/azure-adventure`, with one directory per
voice, and never replaces your keyboard recordings. Updates reuse validated
story recordings and publish the new bank only after every requested recording
and its timings pass validation. If preparation fails, completed recordings are
retained and reused on the next attempt; incomplete cache entries are reported
and regenerated. The v0.2.0 optional speech download contains both the keyboard
and adventure banks. Until preparation or installation is complete,
the book still works as a text adventure; missing audio is reported visibly.
Generated story audio is also subject to the Azure terms described below.

## Download and run on Windows 11

Download `learn-swedish-0.3.0-windows-x64.zip` from
[GitHub Releases](https://github.com/hohum/learn_swedish/releases/latest).
Right-click it, choose **Extract All**, then open `bin\learn-swedish.exe`
inside the extracted folder. Keep the extracted files together. No WSL,
administrator access, Rust or separate GTK install is needed. Add the Swedish
keyboard in Windows language settings and use **Windows+Space** to select it.
The portable ZIP is not code-signed.

To add optional offline voices, download the existing
[v0.2.0 speech archive](https://github.com/hohum/learn_swedish/releases/download/v0.2.0/learn-swedish-speech-0.2.0.tar.gz).
It is the same keyboard and story content; no Azure account or regeneration is
needed. See the Azure terms note under Ubuntu below. Close the app, open
PowerShell in the extracted app folder (the folder containing `bin`), and run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install_windows_speech.ps1 -Archive "$HOME\Downloads\learn-swedish-speech-0.2.0.tar.gz"
```

Use the actual path to the archive if you saved it elsewhere. The script checks
the archive's SHA-256 and installs both banks into the portable folder without
symlinks. `-ExecutionPolicy Bypass` only applies to that PowerShell process.
Speech preparation still runs on Ubuntu, not in the Windows app.

### Build the Windows ZIP

Run these commands in **MSYS2 UCRT64** on Windows:

```sh
pacman -S --needed mingw-w64-ucrt-x86_64-{rust,gcc,pkgconf,libadwaita,gst-plugins-base,gst-plugins-good} zip
bash scripts/build_windows.sh
```

The ZIP is created in `dist`. `THIRD-PARTY.txt` lists its bundled packages.

## Download and install on Ubuntu 26.04

Download the latest `.deb` from [GitHub Releases](https://github.com/hohum/learn_swedish/releases/latest).
The prebuilt downloads target **Ubuntu 26.04, Intel/AMD 64-bit**, not older
Ubuntu releases or ARM devices.

In the directory containing the download:

```sh
sudo apt install ./learn-swedish_0.3.0_ubuntu26.04_amd64.deb
learn-swedish
```

The package installs runtime dependencies and adds **Swedish Keys** to the app
launcher. No Rust compiler is needed. A `.tar.gz` containing the standalone
binary is also available; it requires the same native libraries as the `.deb`.
Verify downloads with `sha256sum --ignore-missing -c SHA256SUMS` after downloading
your chosen files and their checksum file.

**Speech recordings are a separate optional download**, not bundled in the
app packages. Download [learn-swedish-speech-0.2.0.tar.gz](https://github.com/hohum/learn_swedish/releases/download/v0.2.0/learn-swedish-speech-0.2.0.tar.gz) for
the keyboard and adventure recordings and their word timings.
Close the app, then install the recordings from your download directory:

```sh
data="${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish"
mkdir -p "$data"
tar -xzf learn-swedish-speech-0.2.0.tar.gz -C "$data"
```

Relaunch the app to use the Sofie and Mattias synthetic voices offline. No Azure
account, key, SDK or network connection is needed for playback. Installing the
archive selects the downloaded banks; previously prepared banks are retained.

The app's MIT source license does not
grant rights to Microsoft-generated recordings or Piper models. Microsoft's
[Azure terms](https://www.microsoft.com/licensing/terms/productoffering/MicrosoftAzure/MCA)
expressly grant prebuilt-neural TTS output use rights to paid-tier customers;
the redistribution rights for these F0-generated recordings are not confirmed.
Their availability here is not a grant of rights or confirmation of Microsoft's
permission. Downloads contain no API keys, Python SDKs or voice models.
Confirm the terms applicable to your subscription before redistributing audio.

For private use on another computer, copy your existing
`~/.local/share/learn-swedish/speech` directory into the same location on that
computer, preserving its relative symlinks. No Azure key or SDK needs to be
copied. Close and relaunch the app after copying. If you use `XDG_DATA_HOME`,
use its `learn-swedish/speech` directory instead.

Alternatively, follow **Speech setup** below on the destination computer. For
the installed `.deb`, the one-time Azure commands are:

```sh
data="${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish"
python3 -m venv "$data/azure-env"
"$data/azure-env/bin/python" -m pip install -r /usr/share/learn-swedish/requirements-azure.txt
learn-swedish --prepare-azure-speech
```

The region defaults to `australiaeast`; set `SPEECH_REGION` to your own resource's
region if different. The API key is requested only at a hidden local terminal
prompt. For the tar download, run `./learn-swedish` and use the bundled
`scripts/requirements-azure.txt` instead. For Piper setup with either prebuilt
download, replace `cargo run --locked -- --prepare-speech` below with
`learn-swedish --prepare-speech` or `./learn-swedish --prepare-speech`.

## Build and run on Ubuntu

Install the native libraries, audio plugins, speech setup tools, and Rust build tools:

```sh
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgstreamer1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good python3-venv cargo rustc
rustc --version
```

Rust **1.92 or newer** is required. If your Ubuntu release packages an older
compiler, install the current stable toolchain through [rustup](https://rustup.rs/).
The bindings use GTK4 and libadwaita without opting into newer native APIs.

Complete the one-time **Speech setup** below, then from the project directory:

```sh
cargo run --locked
```

In **Settings → Keyboard → Input Sources**, add **Swedish** and select it using
GNOME's input-source switcher. Use the standard Swedish layout, not a Dvorak,
Apple, or “no dead keys” variant. The app does not change your system layout.

Click the input box if it loses focus, then type the displayed character.
Letters accept either case; punctuation must match exactly. Correct input
stays visible until the next card. With **Automatic next** on, the app waits for
the full answer/feedback/announcement sequence before advancing. Wrong input
also stays visible and is selected so that typing again replaces it; Backspace
or Delete can clear it without counting another attempt. Input is locked while
the app speaks a prompt, answer or feedback, so an early correction cannot cut
off the explanation. Early keystrokes are discarded, not saved for the next
card. If you type while it is speaking, speech pauses for one random Swedish
reminder: **Vänta lite**, **Ett ögonblick**, **Vänta en sekund**,
**Vänta lite grann** or **Bara en sekund**. Repeated keys do not queue more
reminders during the reminder itself. Afterwards, the interrupted phrase
restarts from the beginning in the same voice, followed by any remaining
feedback and next-card announcement. Input stays locked throughout.
Replay, Next and settings can still cancel speech; playback errors
also release the input lock. Moving to the next
card clears the input box. **Next / Skip** advances without a correct
answer and counts that card as skipped. **Finish** shows the session summary.
**Practice again** starts over.

### Swedish keyboard details

- `AltGr` means **Right Alt**, not Ctrl+Alt.
- `/` is **Shift+7**; `\` is **AltGr++** (Right Alt and the plus key).
- `|` is **AltGr+<** (the key between Left Shift and Z).
- `{ }` are **AltGr+7 / AltGr+0**; `[ ]` are **AltGr+8 / AltGr+9**.
- `~`, `^` and backtick use **dead keys**: press the hinted combination,
  release it, then press **Space** to commit the symbol.
- For the spacing acute accent `´` or diaeresis `¨`, press its key **twice**.
  On Ubuntu's Compose table, following these keys with Space produces an
  apostrophe or double quote instead.
- The visible `␣` prompt means press **Space**; it is not the literal `␣` character.

Input is checked as committed text rather than physical keycodes so that GTK's
input method handles Swedish characters, AltGr and dead-key composition.
This also means pasted text can answer a card; this is a learning aid, not a
typing examination. The app cannot verify which hardware layout you are using.
Tab and Enter remain available for normal GUI navigation.

### Speech setup

Speech setup records the fixed curriculum and feedback phrases once.
The Rust app plays the prepared WAV files through an audio-only GStreamer pipeline,
giving immediate feedback without loading a neural model for each key. On
GNOME and WSLg, its PulseAudio output also works through PipeWire's PulseAudio
compatibility service. Hover over the follow-along text to see the selected voice.

#### Recommended: Azure Sofie

Create an Azure Speech resource, preferably on the **Free F0** tier, and open
its **Keys and Endpoint** page. The resource region does not determine the
language: this setup explicitly uses **sv-SE-SofieNeural**.
Azure is only used during preparation; no cloud SDK, key or connection is
needed for subsequent GUI playback.

Run these commands in an **interactive Ubuntu terminal**, from the project directory:

```sh
data="${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish"
python3 -m venv "$data/azure-env"
"$data/azure-env/bin/python" -m pip install -r scripts/requirements-azure.txt
cargo run --locked -- --prepare-azure-speech
```

Close the app before preparing or updating its library. The resource region
defaults to **australiaeast** and is displayed, not requested at an input prompt.
For another region, run `export SPEECH_REGION=swedencentral` (using your resource's
region code) before the preparation command. This variable is only for the region,
never a key.
The only input prompt is for **KEY 1**: paste it there, then press Enter.
Nothing is echoed.
The key is used only in the preparation process's memory, not saved in files,
environment variables, command arguments or shell history. Do not paste it into
chat, code, screenshots or a shell command. Preparation refuses to fall back to
visible key input if a hidden terminal prompt is unavailable.

The command explains the fixed text that will be sent to Microsoft and prints
progress for each recording. It paces requests below F0's 20-per-minute limit;
allow several minutes for a fresh library of 125 phrases. Existing validated
Azure recordings are reused, so updating the previous 120-phrase library sends
only the five new wait phrases. The active bank is replaced only after the
complete updated library passes validation. Do not use the same resource for
other synthesis simultaneously. Check [Azure pricing](https://azure.microsoft.com/pricing/details/speech/)
and [quotas](https://learn.microsoft.com/azure/ai-services/speech-service/speech-services-quotas-and-limits)
for your tier. Requests can count against your allowance even if preparation
later fails.

Audio and measured word boundaries are validated before an atomic library
switch. Network, authentication, quota or alignment errors stop preparation
without changing the active library; no estimated word timings or silent Piper
fallback are used. Azure's normal punctuation is retained, independently of
the Piper punctuation workaround below. Hyphenated display words combine
their actual Azure sub-word intervals.
The audio result can arrive before Azure finishes delivering timing callbacks.
Preparation waits for completion and matching word events, including all parts
of hyphenated words, for up to ten seconds per phrase. Each utterance has its own
synthesizer, and callbacks are matched by result ID so delayed events cannot
contaminate another recording. Missing metadata fails explicitly, without
publishing partial recordings.
Azure can classify a spoken letter such as **D** as a punctuation event while
still providing its measured start and duration. Such events are retained when
their source-text span contains a letter or digit. Actual punctuation and
sentence-level events are ignored, so punctuation pauses are not highlighted
and sentence durations cannot replace word timings.

If one phrase repeatedly fails timing validation, inspect the actual service
events without regenerating the library:

```sh
"${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish/azure-env/bin/python" \
  -B scripts/diagnose_azure_speech.py
```

This sends only **Bokstaven D**, uses the same hidden key prompt, and observes
callbacks for ten seconds. Its JSON report contains the fixed text, word event
types, offsets, durations and synthesis result IDs, but no key or request headers.
No audio is saved and the app's active library is unchanged. Share only the JSON
report when troubleshooting, never the key.

Successful preparation selects **Sofie only**, not a random mixture with Piper.
Close and relaunch the GUI after preparation:

```sh
cargo run --locked
```

Generated libraries live under
`$XDG_DATA_HOME/learn-swedish/speech/azure-generation-*`, normally inside
`~/.local/share/learn-swedish`. The `speech/azure-sv-SE-SofieNeural` symlink selects
the complete active generation. Previous successful generations are retained.
A missing or damaged file in a selected Azure library produces a visible error,
not an unexpected switch to Piper. Re-run preparation when the curriculum changes.

#### Alternative: local Piper generation

Piper generation needs no Azure account, but its Swedish pronunciation has known
limitations even with natural-sounding voices. The original **sv_SE-nst-medium** voice is joined by
**sv_SE-alma-medium**, a Swedish female voice. There is no eSpeak fallback.
These voices are used only when no Azure library is selected.

Run these commands in Ubuntu, from the project directory:

```sh
data="${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish"
python3 -m venv "$data/piper-env"
"$data/piper-env/bin/python" -m pip install "piper-tts[alignment]==1.8.0"
mkdir -p "$data/voices"
"$data/piper-env/bin/python" -m piper.download_voices \
  --data-dir "$data/voices" sv_SE-nst-medium sv_SE-alma-medium
cargo run --locked -- --prepare-speech
```

Only package installation and downloading the two voice models need internet.
Generation and all subsequent playback are offline. Run `--prepare-speech`
again if the curriculum changes. A failed generation leaves previously prepared
audio unchanged; generated files are validated before replacing existing ones.
Models, the isolated Python environment and prepared audio live under
`$XDG_DATA_HOME/learn-swedish` (normally `~/.local/share/learn-swedish`),
not in the source tree. Python is only used for preparation; the GUI is Rust.
Setup patches the model in memory to expose phoneme durations; the downloaded
models remain unchanged. Each voice has its own audio directory and timings,
so changing voices never reuses the other voice's word boundaries. Each WAV has a JSON file containing measured word
intervals. Missing alignments, mismatched words and invalid intervals fail setup
explicitly rather than using estimated timing.
Sentence-ending punctuation is omitted from Piper's synthesis input to avoid
the voice's audible trailing-vowel artifact. Written punctuation, audio filenames
and highlighted words retain the original text; internal hyphens are unchanged.

#### Practice and playback

The app speaks the Swedish name (for example, “Bokstaven A” or “Snedstreck”),
not an associated word. If the prompt is A and you type B, it says “Bokstaven B”.
Uppercase/lowercase letters use the same name; digits and supported symbols are
also spoken, regardless of which practice set is selected. Unsupported characters
or multi-character pastes show a message instead of speaking an invented name.
Dead-key combinations are spoken only after committing the completed character.
After a correct answer, input remains locked until the next card.

**Speak each new symbol in Swedish** controls prompts;
**Speak my answers and feedback in Swedish** controls typed-character speech
followed by the randomly selected answer feedback. Both default to on.
Replay, typing another character, Next, switching sets, turning off a speech
checkbox and closing the window stop the entire current sequence. The bilingual
transcript remains readable after playback, with highlighting cleared.

**Automatic next** defaults to on. Correct answers play the typed character,
positive feedback, then one of ten next-card announcements (for example,
**Här kommer nästa. / Here comes the next one.** or
**Dags för nästa. / Time for the next one.**). All three use the same randomly
selected voice, with translations and synchronized highlighting. The next card
is shown only after playback finishes; its prompt is then spoken if enabled.
The last card finishes the session without announcing a nonexistent next card.
Wrong answers never advance automatically.

If answer speech is off, automatic advancement waits one second so your typed
answer and the written feedback are briefly visible; nothing is spoken.
Turning off automatic advancement, pressing Replay/Next, restarting, changing
sets, changing a speech toggle or closing the window cancels pending automatic
advancement. Turning the checkbox on after an already-completed answer does not
retroactively advance it. If audio fails, the card stays in place and the error
remains visible; use **Next** manually.

The ten next-card announcements are:

| Swedish | English |
| --- | --- |
| Här kommer nästa. | Here comes the next one. |
| Vi tar nästa. | Let's take the next one. |
| Dags för nästa. | Time for the next one. |
| Nu kommer nästa. | Now for the next one. |
| Nu går vi vidare. | Let's move on. |
| Redo för nästa? | Ready for the next one? |
| Vidare till nästa. | On to the next one. |
| Vi provar en till. | Let's try another one. |
| Här kommer en ny. | Here's a new one. |
| Vi fortsätter med nästa. | Let's carry on with the next one. |

Playback is asynchronous; rapid typing replaces the previous answer sequence
instead of building up a backlog. English translations remain visible while only
the spoken Swedish words are highlighted. Silence between words is not
highlighted. Missing audio/timing files, unavailable plugins, audio errors and
playback startup timeouts are shown in the window and logged to stderr;
keyboard practice remains usable. Check Ubuntu's output device and volume if
playback reports success but you cannot hear it.

Piper is an external [GPL-3.0-or-later engine](https://github.com/OHF-Voice/piper1-gpl).
The selected voice's [model card](https://huggingface.co/rhasspy/piper-voices/blob/main/sv/sv_SE/nst/medium/MODEL_CARD)
identifies NST's training dataset as CC0.
[Alma](https://huggingface.co/rhasspy/piper-voices/blob/main/sv/sv_SE/alma/medium/MODEL_CARD)
is by Daniel Nylander (yeagersthlm); its model weights are licensed
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Alma is used here
without modifying the downloaded model. No engine or model is bundled in this project.

## Development

- [src/lesson.rs](src/lesson.rs): curriculum and manual lesson state, with no GUI dependencies.
- [src/main.rs](src/main.rs): native interface and input handling.
- [src/speech.rs](src/speech.rs): one-time Azure/Piper preparation and asynchronous WAV playback.
- [scripts/prepare_speech.py](scripts/prepare_speech.py): full-phrase neural audio and measured word timings; embedded into the Rust preparation command.
- [scripts/prepare_azure_speech.py](scripts/prepare_azure_speech.py): hidden local credential prompt, Azure Sofie recordings and measured word boundaries; embedded into the Rust Azure preparation command.
- [scripts/build_release.py](scripts/build_release.py): Ubuntu 26.04 `.deb`, tar archive and checksums from an explicit file allowlist, without generated speech or credentials.

On Ubuntu 26.04 x86_64, build release downloads with `python3 -B scripts/build_release.py`.
Artifacts are written to the ignored `dist` directory. Generated recordings
remain in the user's data directory and are never read by the release packager.

```sh
# Core behavior, without requiring GTK development libraries or a display:
cargo test --locked --no-default-features

# Speech preparation regression test (using the speech-setup environment):
"${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish/piper-env/bin/python" \
  -B -m unittest discover -s scripts -p 'test_prepare_speech.py'

# Azure preparation tests: mocked synthesis, no key or cloud calls:
"${XDG_DATA_HOME:-$HOME/.local/share}/learn-swedish/azure-env/bin/python" \
  -B -m unittest discover -s scripts -p 'test_prepare_azure_speech.py'

# Full build and default tests:
cargo build --locked
cargo test --locked

# Native widget flow, using a virtual display:
sudo apt install xvfb
GSK_RENDERER=cairo xvfb-run -a -s '-screen 0 1366x768x24' \
  cargo test --locked --bin learn-swedish -- --ignored

# Optional: also play two real prepared phrases through your audio device:
LEARN_SWEDISH_TEST_REAL_AUDIO=1 GSK_RENDERER=cairo \
  xvfb-run -a cargo test --locked --bin learn-swedish -- --ignored

# If these Cargo subcommands are not installed with your Ubuntu Rust packages:
sudo apt install rustfmt rust-clippy
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

The virtual-display test plays silent WAV fixtures through GStreamer's
clock-synchronized fake audio sink (no model or audio device needed). It checks
typed-character/feedback ordering, word-by-word highlights, bilingual text,
correct/incorrect phrase selection, independent speech toggles, unavailable
audio, immediate cancellation, manual progression, switching sets, committed
punctuation, completion and restart. Real Swedish-layout dead-key
composition, audible pronunciation, and the final GNOME desktop experience
also need checking interactively on the target machine.
