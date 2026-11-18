"""Prepare Azure Sofie recordings and measured word timings for offline playback."""

import getpass
import io
import json
import os
import re
import sys
import time
import warnings
import wave
from dataclasses import dataclass
from datetime import timedelta
from pathlib import Path
from threading import Condition
from typing import TypedDict

import azure.cognitiveservices.speech as speechsdk


@dataclass(frozen=True)
class Boundary:
    offset: int
    length: int
    start_us: int
    end_us: int


class WordTiming(TypedDict):
    word: str
    start_us: int
    end_us: int


class Timing(TypedDict):
    duration_us: int
    words: list[WordTiming]


def word_timings(text: str, boundaries: list[Boundary], duration_us: int) -> list[WordTiming]:
    spans = list(re.finditer(r"\S+", text))
    grouped: list[list[Boundary]] = [[] for _ in spans]
    previous_end = 0
    for boundary in boundaries:
        if (
            boundary.length <= 0
            or boundary.start_us < previous_end
            or boundary.end_us <= boundary.start_us
            or boundary.end_us > duration_us
        ):
            raise ValueError(f"Invalid Azure word boundary for {text!r}")
        matches = [
            index for index, span in enumerate(spans)
            if span.start() <= boundary.offset
            and boundary.offset + boundary.length <= span.end()
        ]
        if len(matches) != 1:
            raise ValueError(f"Cannot map Azure word boundary to {text!r}")
        grouped[matches[0]].append(boundary)
        previous_end = boundary.end_us
    if not spans or any(not group for group in grouped):
        raise ValueError(f"Azure did not provide all word timings for {text!r}")
    words: list[WordTiming] = [
        {"word": span.group(), "start_us": group[0].start_us, "end_us": group[-1].end_us}
        for span, group in zip(spans, grouped, strict=True)
    ]
    if any(left["end_us"] > right["start_us"] for left, right in zip(words, words[1:])):
        raise ValueError(f"Azure word timings are out of text order for {text!r}")
    return words


def synthesize(config: speechsdk.SpeechConfig, text: str) -> tuple[bytes, Timing]:
    synthesizer = speechsdk.SpeechSynthesizer(speech_config=config, audio_config=None)
    condition = Condition()
    boundaries: dict[str, list[Boundary]] = {}
    completed: set[str] = set()

    def on_boundary(event: speechsdk.SpeechSynthesisWordBoundaryEventArgs) -> None:
        # Azure can label a spoken letter such as D as punctuation.
        spoken_punctuation = (
            event.boundary_type == speechsdk.SpeechSynthesisBoundaryType.Punctuation
            and any(character.isalnum() for character in text[
                event.text_offset:event.text_offset + event.word_length
            ])
        )
        if event.boundary_type == speechsdk.SpeechSynthesisBoundaryType.Word or spoken_punctuation:
            start_us = event.audio_offset // 10
            boundary = Boundary(
                event.text_offset,
                event.word_length,
                start_us,
                start_us + event.duration // timedelta(microseconds=1),
            )
            with condition:
                boundaries.setdefault(event.result_id, []).append(boundary)
                condition.notify_all()

    def on_completed(event: speechsdk.SpeechSynthesisEventArgs) -> None:
        with condition:
            completed.add(event.result.result_id)
            condition.notify_all()

    synthesizer.synthesis_word_boundary.connect(on_boundary)
    synthesizer.synthesis_completed.connect(on_completed)
    try:
        result = synthesizer.speak_text_async(text).get()
        if result.reason != speechsdk.ResultReason.SynthesizingAudioCompleted:
            failure = (
                result.cancellation_details.error_code
                if result.reason == speechsdk.ResultReason.Canceled else result.reason
            )
            # Service diagnostics aren't printed: they may contain request credentials.
            raise RuntimeError(
                f"Azure speech failed for {text!r} ({failure}). "
                "Check your key, resource region, network and Azure quota. "
                "The active audio library has not been changed."
            )
        with wave.open(io.BytesIO(result.audio_data), "rb") as audio:
            if audio.getnchannels() != 1 or audio.getsampwidth() != 2 or audio.getframerate() != 24_000:
                raise ValueError(f"Unexpected Azure audio format for {text!r}")
            frames = audio.getnframes()
            if frames <= 0 or len(audio.readframes(frames)) != frames * 2:
                raise ValueError(f"Empty or truncated Azure audio for {text!r}")
            duration_us = frames * 1_000_000 // audio.getframerate()
        def metadata_ready() -> bool:
            current = boundaries.get(result.result_id, [])
            # A completed audio result can precede its callbacks. Hyphenated
            # words can also have several callbacks, so wait for all letters.
            return result.result_id in completed and all(
                any(boundary.offset <= index < boundary.offset + boundary.length for boundary in current)
                for index, character in enumerate(text) if character.isalnum()
            )

        with condition:
            if not condition.wait_for(metadata_ready, timeout=10.0):
                raise RuntimeError(
                    f"Azure did not deliver complete word timings for {text!r} within 10 seconds. "
                    "The active audio library has not been changed."
                )
            current = list(boundaries[result.result_id])
        metadata: Timing = {
            "duration_us": duration_us,
            "words": word_timings(text, current, duration_us),
        }
        return result.audio_data, metadata
    finally:
        synthesizer.synthesis_word_boundary.disconnect_all()
        synthesizer.synthesis_completed.disconnect_all()


def speech_config(key: str, region: str) -> speechsdk.SpeechConfig:
    config = speechsdk.SpeechConfig(subscription=key, region=region)
    config.speech_synthesis_voice_name = "sv-SE-SofieNeural"
    config.set_speech_synthesis_output_format(
        speechsdk.SpeechSynthesisOutputFormat.Riff24Khz16BitMonoPcm
    )
    config.set_property(speechsdk.PropertyId.SpeechServiceResponse_RequestWordBoundary, "true")
    config.set_property(speechsdk.PropertyId.SpeechServiceResponse_SynthesisEventsSyncToAudio, "false")
    return config


def prepare(output: Path, texts: list[str], key: str, region: str) -> None:
    config = speech_config(key, region)
    for index, text in enumerate(texts, 1):
        if index > 1:
            # F0 allows 20 requests per minute; leave headroom between requests.
            time.sleep(3.1)
        audio, metadata = synthesize(config, text)
        (output / f"{text}.wav").write_bytes(audio)
        (output / f"{text}.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
        print(f"Prepared {index}/{len(texts)}: {text}", flush=True)

def validate_jobs(payload: object) -> list[dict[str, str]]:
    if not isinstance(payload, dict) or set(payload) != {"jobs"}:
        raise ValueError("Expected fixed story recording jobs")
    jobs = payload["jobs"]
    if not isinstance(jobs, list) or not jobs:
        raise ValueError("Expected a nonempty list of story recording jobs")
    seen = set()
    for job in jobs:
        if (
            not isinstance(job, dict)
            or set(job) != {"text", "voice", "stem"}
            or not isinstance(job["text"], str) or not job["text"].strip()
            or job["voice"] not in ("sv-SE-SofieNeural", "sv-SE-MattiasNeural")
            or not isinstance(job["stem"], str)
            or not re.fullmatch(r"[0-9a-f]{16}", job["stem"])
        ):
            raise ValueError("Invalid story voice, text or recording filename")
        identity = (job["voice"], job["stem"])
        if identity in seen:
            raise ValueError("Duplicate story recording filename")
        seen.add(identity)
    return jobs


def prepare_jobs(output: Path, jobs: list[dict[str, str]], key: str, region: str) -> None:
    config = speech_config(key, region)
    for index, job in enumerate(jobs, 1):
        if index > 1:
            time.sleep(3.1)
        config.speech_synthesis_voice_name = job["voice"]
        audio, metadata = synthesize(config, job["text"])
        folder = output / job["voice"]
        folder.mkdir(exist_ok=True)
        (folder / f'{job["stem"]}.wav').write_bytes(audio)
        (folder / f'{job["stem"]}.json').write_text(
            json.dumps(metadata, ensure_ascii=False), encoding="utf-8"
        )
        print(f'Prepared {index}/{len(jobs)} ({job["voice"]}): {job["text"]}', flush=True)


def credentials() -> tuple[str, str]:
    if not sys.stdin.isatty():
        raise RuntimeError(
            "Run this command interactively in an Ubuntu terminal. "
            "An API key must be entered at a hidden local prompt, not supplied in a command or chat."
        )
    region = os.environ.get("SPEECH_REGION", "australiaeast").strip()
    if not re.fullmatch(r"[a-z]{2,24}[0-9]?", region):
        raise ValueError("SPEECH_REGION must be a region code such as australiaeast, not an API key")
    print(f"Azure resource region: {region} (set SPEECH_REGION to change it)")
    with warnings.catch_warnings():
        warnings.simplefilter("error", getpass.GetPassWarning)
        key = getpass.getpass("Azure Speech KEY 1 (hidden; paste then Enter): ").strip()
    if not key:
        raise ValueError("An Azure Speech key is required")
    return key, region


def main() -> None:
    output = Path(sys.argv[1])
    argument = sys.argv[2]
    payload = json.loads(Path(argument[1:]).read_text(encoding="utf-8") if argument.startswith("@") else argument)
    jobs = validate_jobs(payload) if isinstance(payload, dict) else None
    texts = [job["text"] for job in jobs] if jobs else payload
    if not isinstance(texts, list) or not texts or not all(isinstance(text, str) and text.strip() for text in texts):
        raise ValueError("Expected a nonempty JSON array of Swedish phrases")
    print(
        f"Azure will receive {len(texts)} fixed Swedish phrases "
        f"({sum(map(len, texts))} characters) to record with "
        f"{'Sofie and Mattias' if jobs else 'Sofie'}.\n"
        "No typed answers or microphone audio are sent. Your key is not saved."
    )
    key, region = credentials()
    try:
        if jobs:
            prepare_jobs(output, jobs, key, region)
        else:
            prepare(output, texts, key, region)
    except RuntimeError as error:
        raise RuntimeError(str(error).replace(key, "[redacted]")) from None


if __name__ == "__main__":
    main()
