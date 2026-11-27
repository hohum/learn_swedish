"""Synthesize one page with Azure Speech and split it into timed WAV clips."""

import html
import io
import json
import re
import sys
import wave
from dataclasses import dataclass
from datetime import timedelta
from pathlib import Path
from threading import Event


SAMPLE_RATE = 24_000
VOICE_NAMES = {"sv-SE-MattiasNeural", "sv-SE-SofieNeural", "sv-SE-HilleviNeural"}


@dataclass
class Boundary:
    text: str
    start_us: int
    end_us: int


def sentence_end(word):
    return word.rstrip("\"')]\u00bb\u201d\u2019").endswith((".", "!", "?"))


def markup(lines):
    """Return an SSML document and the bookmark IDs for each authored line."""
    parts = ['<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="sv-SE">']
    markers = []
    for index, line in enumerate(lines):
        voice = line["voice"]
        assert voice in VOICE_NAMES
        parts.append(f'<voice name="{voice}">')
        prosody = f'<prosody pitch="{line["pitch"]:+d}%" rate="{line["rate"]:+d}%">'
        words = line["text"].split()
        starts = []
        for word_index, word in enumerate(words):
            if word_index == 0 or sentence_end(words[word_index - 1]):
                if word_index:
                    parts.append("</prosody> ")
                mark = f"line{index}sentence{len(starts)}"
                starts.append(mark)
                parts.extend((f'<bookmark mark="{mark}"/>', prosody))
            else:
                parts.append(" ")
            parts.append(html.escape(word))
        parts.append("</prosody></voice>")
        markers.append(starts)
    parts.append("</speak>")
    return "".join(parts), markers


def letters(text):
    return "".join(character.casefold() for character in text if character.isalnum())


def word_timings(text, boundaries, start_us, duration_us):
    """Group measured words, including compounds Azure splits into several events."""
    timings = []
    words = text.split()
    boundary_index = 0
    previous_end = 0
    for word in words:
        target = letters(word)
        spoken = ""
        start = None
        end = None
        while spoken != target and boundary_index < len(boundaries):
            boundary = boundaries[boundary_index]
            boundary_index += 1
            spoken += letters(boundary.text)
            if not target.startswith(spoken):
                return []
            if start is None:
                start = max(previous_end, boundary.start_us - start_us)
            end = min(duration_us, boundary.end_us - start_us)
        if spoken != target or start is None or end is None or end <= start:
            return []
        timings.append({"word": word, "start_us": start, "end_us": end})
        previous_end = end
    return timings if boundary_index == len(boundaries) else []


def write_clips(destination, lines, markers, audio_data, bookmarks, boundaries):
    with wave.open(io.BytesIO(audio_data), "rb") as audio:
        if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate()) != (1, 2, SAMPLE_RATE):
            raise ValueError("Unexpected Azure audio format.")
        frames = audio.getnframes()
        data = audio.readframes(frames)
        if frames <= 0 or len(data) != frames * 2:
            raise ValueError("Azure returned empty or truncated audio.")

    def to_frame(ticks):
        return ticks * SAMPLE_RATE // 10_000_000

    # Keep the beginning of the first clip and all the natural silence between clips.
    starts = [0] + [to_frame(bookmarks[marks[0]]) for marks in markers[1:]]
    if starts != sorted(set(starts)) or starts[-1] >= frames:
        raise ValueError("Invalid Azure line boundaries.")
    for index, (begin, end) in enumerate(zip(starts, starts[1:] + [frames])):
        start_us = begin * 1_000_000 // SAMPLE_RATE
        duration_us = (end - begin) * 1_000_000 // SAMPLE_RATE
        end_us = end * 1_000_000 // SAMPLE_RATE
        clip_words = [boundary for boundary in boundaries if start_us <= boundary.start_us < end_us]
        sentences = [0]
        for mark in markers[index][1:]:
            if mark in bookmarks:
                offset = bookmarks[mark] // 10 - start_us
                if sentences[-1] < offset < duration_us:
                    sentences.append(offset)
        metadata = {
            "duration_us": duration_us,
            "words": word_timings(lines[index]["text"], clip_words, start_us, duration_us),
            "sentence_starts_us": sentences if len(sentences) == len(markers[index]) else [],
        }
        with wave.open(str(destination / f"{index}.wav"), "wb") as clip:
            clip.setparams((1, 2, SAMPLE_RATE, 0, "NONE", "not compressed"))
            clip.writeframes(data[begin * 2:end * 2])
        (destination / f"{index}.json").write_text(json.dumps(metadata), encoding="utf-8")


def synthesize(speechsdk, request, destination):
    config = speechsdk.SpeechConfig(subscription=request["key"], region=request["region"])
    config.set_speech_synthesis_output_format(speechsdk.SpeechSynthesisOutputFormat.Riff24Khz16BitMonoPcm)
    config.set_property(speechsdk.PropertyId.SpeechServiceResponse_RequestWordBoundary, "true")
    config.set_property(speechsdk.PropertyId.SpeechServiceResponse_SynthesisEventsSyncToAudio, "false")
    synthesizer = speechsdk.SpeechSynthesizer(speech_config=config, audio_config=None)
    ssml, markers = markup(request["lines"])
    completed = Event()
    bookmarks = {}
    boundaries = []

    def on_word(event):
        if event.boundary_type in (
            speechsdk.SpeechSynthesisBoundaryType.Word,
            speechsdk.SpeechSynthesisBoundaryType.Punctuation,
        ) and letters(event.text):
            start_us = event.audio_offset // 10
            end_us = start_us + event.duration // timedelta(microseconds=1)
            if end_us > start_us:
                boundaries.append(Boundary(event.text, start_us, end_us))

    synthesizer.synthesis_word_boundary.connect(on_word)
    synthesizer.bookmark_reached.connect(lambda event: bookmarks.update({event.text: event.audio_offset}))
    synthesizer.synthesis_completed.connect(lambda _event: completed.set())
    result = synthesizer.speak_ssml_async(ssml).get()
    if result.reason != speechsdk.ResultReason.SynthesizingAudioCompleted:
        code = result.cancellation_details.error_code if result.reason == speechsdk.ResultReason.Canceled else None
        return {
            speechsdk.CancellationErrorCode.AuthenticationFailure: "authentication",
            speechsdk.CancellationErrorCode.Forbidden: "authentication",
            speechsdk.CancellationErrorCode.TooManyRequests: "quota",
            speechsdk.CancellationErrorCode.ConnectionFailure: "connection",
            speechsdk.CancellationErrorCode.ServiceTimeout: "connection",
        }.get(code, "service")
    completed.wait(timeout=3)
    write_clips(destination, request["lines"], markers, result.audio_data, bookmarks.copy(), sorted(
        boundaries, key=lambda boundary: boundary.start_us
    ))
    return "ok"


def main():
    # The parent reads only these fixed status codes. SDK diagnostics can contain credentials.
    request = json.load(sys.stdin.buffer)
    try:
        import azure.cognitiveservices.speech as speechsdk
    except (ImportError, OSError):
        print("sdk")
        return
    try:
        result = synthesize(speechsdk, request, Path(sys.argv[1]))
    except (KeyError, ValueError, wave.Error):
        result = "audio"
    except Exception:
        result = "service"
    print(result)


if __name__ == "__main__":
    main()
