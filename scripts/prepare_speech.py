"""Prepare full-phrase Piper audio with model-derived word timings."""

import json
import sys
import wave
from dataclasses import asdict, dataclass
from pathlib import Path

from piper import PiperVoice
from piper.voice import AudioChunk


@dataclass
class WordTiming:
    word: str
    start_us: int
    end_us: int


def word_timings(text: str, chunk: AudioChunk) -> list[WordTiming]:
    if not chunk.phoneme_alignments:
        raise ValueError(f"Piper did not provide phoneme alignments for {text!r}")
    spans: list[tuple[int, int]] = []
    cursor = 0
    start: int | None = None
    end = 0
    for alignment in chunk.phoneme_alignments:
        samples = int(alignment.num_samples)
        if samples < 0:
            raise ValueError(f"Negative phoneme duration for {text!r}")
        if alignment.phoneme in {"^", "$", " ", ".", "!", "?", ",", ";", ":"}:
            if start is not None:
                spans.append((start, end))
                start = None
        elif samples:
            if start is None:
                start = cursor
            end = cursor + samples
        cursor += samples
    if start is not None:
        spans.append((start, end))
    words = text.split()
    if len(spans) != len(words):
        raise ValueError(
            f"Cannot align {text!r}: {len(words)} written words, {len(spans)} spoken groups"
        )
    if cursor != len(chunk.audio_int16_array):
        raise ValueError(f"Phoneme durations do not cover the audio for {text!r}")
    return [
        WordTiming(word, start * 1_000_000 // chunk.sample_rate, end * 1_000_000 // chunk.sample_rate)
        for word, (start, end) in zip(words, spans, strict=True)
    ]


def main() -> None:
    model, output = map(Path, sys.argv[1:])
    texts = json.load(sys.stdin)
    if not isinstance(texts, list) or not all(isinstance(text, str) for text in texts):
        raise ValueError("Expected a JSON array of Swedish phrases")
    voice = PiperVoice.load(str(model), include_alignments=True)
    for text in texts:
        chunks = list(voice.synthesize(text.rstrip(".!?"), include_alignments=True))
        if len(chunks) != 1:
            raise ValueError(f"Expected a single audio chunk for {text!r}")
        chunk = chunks[0]
        timings = word_timings(text, chunk)
        with wave.open(str(output / f"{text}.wav"), "wb") as audio:
            audio.setnchannels(chunk.sample_channels)
            audio.setsampwidth(chunk.sample_width)
            audio.setframerate(chunk.sample_rate)
            audio.writeframes(chunk.audio_int16_bytes)
        metadata = {
            "duration_us": len(chunk.audio_int16_array) * 1_000_000 // chunk.sample_rate,
            "words": [asdict(timing) for timing in timings],
        }
        (output / f"{text}.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
        print(f"Prepared {text}", flush=True)


if __name__ == "__main__":
    main()
