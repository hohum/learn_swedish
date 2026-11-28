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
from threading import Condition
import time


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


def exact_word_timings(words, boundaries, start_us, duration_us):
    """Fast path when Azure word boundaries match source words and compounds in order."""
    timings = []
    boundary_index = 0
    previous_end = 0
    for index, word in enumerate(words):
        target = letters(word)
        if not target:
            if previous_end >= duration_us:
                return None
            next_limit = (
                max(previous_end + 1, boundaries[boundary_index].start_us - start_us)
                if boundary_index < len(boundaries)
                else duration_us
            )
            end = min(duration_us, previous_end + 1, next_limit)
            if end <= previous_end:
                return None
            timings.append({"word": word, "start_us": previous_end, "end_us": end})
            previous_end = end
            continue
        spoken = ""
        start = None
        end = None
        while spoken != target and boundary_index < len(boundaries):
            boundary = boundaries[boundary_index]
            boundary_index += 1
            spoken += letters(boundary.text)
            if not target.startswith(spoken):
                return None
            if start is None:
                start = max(previous_end, boundary.start_us - start_us)
            end = min(duration_us, boundary.end_us - start_us)
        if spoken != target or start is None or end is None:
            return None
        if end <= start:
            # Short words can arrive from Azure with duration == 0; give them a usable window.
            next_start = (
                max(start + 1, boundaries[boundary_index].start_us - start_us)
                if boundary_index < len(boundaries)
                else duration_us
            )
            remaining_words = len(words) - index - 1
            max_end = max(start + 1, duration_us - remaining_words)
            end = min(max_end, next_start, start + 120_000)
        if end <= start or end > duration_us:
            return None
        timings.append({"word": word, "start_us": start, "end_us": end})
        previous_end = end
    return timings if boundary_index == len(boundaries) else None


def anchored_word_timings(words, boundaries, start_us, duration_us):
    """Align matching measured words via LCS and interpolate numbers/symbols in between."""
    if not words or not boundaries or duration_us < len(words):
        return []
    word_norms = [letters(w) for w in words]
    # Build candidate boundary groups (single boundary or 2-3 adjacent compound parts).
    rel_bounds = [
        (
            letters(b.text),
            max(0, min(duration_us - 1, b.start_us - start_us)),
            max(1, min(duration_us, b.end_us - start_us)),
        )
        for b in boundaries
    ]
    n_words = len(words)
    n_bounds = len(rel_bounds)
    # DP for maximum anchored words: dp[i][j] = best match count for words[i:] and rel_bounds[j:]
    dp = [[0] * (n_bounds + 1) for _ in range(n_words + 1)]
    choice = [[None] * (n_bounds + 1) for _ in range(n_words + 1)]
    for i in range(n_words - 1, -1, -1):
        target = word_norms[i]
        for j in range(n_bounds - 1, -1, -1):
            best = dp[i + 1][j]
            act = ("skip_w", 0)
            if dp[i][j + 1] > best:
                best = dp[i][j + 1]
                act = ("skip_b", 0)
            if target:
                combined = ""
                for span in range(1, min(4, n_bounds - j + 1)):
                    combined += rel_bounds[j + span - 1][0]
                    if combined == target:
                        cand = 1 + dp[i + 1][j + span]
                        if cand >= best:
                            best = cand
                            act = ("match", span)
                        break
                    if not target.startswith(combined):
                        break
            dp[i][j] = best
            choice[i][j] = act

    anchors = {}
    i, j = 0, 0
    while i < n_words and j < n_bounds:
        act, span = choice[i][j]
        if act == "match":
            b_start = rel_bounds[j][1]
            b_end = max(b_start + 1, rel_bounds[j + span - 1][2])
            anchors[i] = (b_start, b_end)
            i += 1
            j += span
        elif act == "skip_w":
            i += 1
        else:
            j += 1

    # Enforce room before/after every anchor for unanchored words (at least 1 us per word).
    cleaned = {}
    prev_idx = -1
    prev_end = 0
    for idx in sorted(anchors):
        gap_words = idx - prev_idx - 1
        min_start = prev_end + gap_words
        max_end = duration_us - (n_words - 1 - idx)
        s, e = anchors[idx]
        s = max(min_start, s)
        e = min(max_end, max(s + 1, e))
        if s < max_end and e > s:
            cleaned[idx] = (s, e)
            prev_idx = idx
            prev_end = e

    # Fill all spans (anchored and interpolated gaps).
    result = [None] * n_words
    anchor_indices = sorted(cleaned)
    segments = []
    cursor = 0
    for a_idx in anchor_indices:
        if cursor < a_idx:
            win_start = cleaned[anchor_indices[anchor_indices.index(a_idx) - 1]][1] if cursor > 0 else 0
            win_end = cleaned[a_idx][0]
            segments.append((cursor, a_idx, win_start, win_end))
        s, e = cleaned[a_idx]
        result[a_idx] = {"word": words[a_idx], "start_us": s, "end_us": e}
        cursor = a_idx + 1
    if cursor < n_words:
        win_start = cleaned[anchor_indices[-1]][1] if anchor_indices else 0
        segments.append((cursor, n_words, win_start, duration_us))

    for lo, hi, win_start, win_end in segments:
        count = hi - lo
        avail = win_end - win_start
        if avail < count:
            return []
        weights = [max(1, len(letters(words[k]))) for k in range(lo, hi)]
        total_w = sum(weights)
        pos = win_start
        acc_w = 0
        for offset, k in enumerate(range(lo, hi)):
            acc_w += weights[offset]
            remaining = count - offset - 1
            if offset + 1 == count:
                end = win_end
            else:
                target_end = win_start + (avail * acc_w) // total_w
                end = max(pos + 1, min(win_end - remaining, target_end))
            result[k] = {"word": words[k], "start_us": pos, "end_us": end}
            pos = end

    return result


def word_timings(text, boundaries, start_us, duration_us):
    """Group measured words, falling back to anchored interpolation when tokens normalize."""
    if not boundaries:
        return []
    words = text.split()
    exact = exact_word_timings(words, boundaries, start_us, duration_us)
    if exact is not None:
        return exact
    return anchored_word_timings(words, boundaries, start_us, duration_us)


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
    raw_starts_us = [0] + [bookmarks[marks[0]] // 10 for marks in markers[1:]] + [
        frames * 1_000_000 // SAMPLE_RATE
    ]
    for index, (begin, end) in enumerate(zip(starts, starts[1:] + [frames])):
        start_us = begin * 1_000_000 // SAMPLE_RATE
        duration_us = (end - begin) * 1_000_000 // SAMPLE_RATE
        raw_begin_us = raw_starts_us[index]
        raw_end_us = raw_starts_us[index + 1]
        clip_words = [
            boundary
            for boundary in boundaries
            if raw_begin_us <= boundary.start_us < raw_end_us
        ]
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
    condition = Condition()
    done = False
    last_event_at = 0.0
    bookmarks = {}
    boundaries = []
    expected_line_marks = [marks[0] for marks in markers[1:]]

    def on_word(event):
        nonlocal last_event_at
        if event.boundary_type in (
            speechsdk.SpeechSynthesisBoundaryType.Word,
            speechsdk.SpeechSynthesisBoundaryType.Punctuation,
        ) and letters(event.text):
            start_us = event.audio_offset // 10
            dur_us = max(1, event.duration // timedelta(microseconds=1))
            with condition:
                boundaries.append(Boundary(event.text, start_us, start_us + dur_us))
                last_event_at = time.monotonic()
                condition.notify_all()

    def on_bookmark(event):
        nonlocal last_event_at
        with condition:
            bookmarks[event.text] = event.audio_offset
            last_event_at = time.monotonic()
            condition.notify_all()

    def on_completed(_event):
        nonlocal done, last_event_at
        with condition:
            done = True
            last_event_at = time.monotonic()
            condition.notify_all()

    synthesizer.synthesis_word_boundary.connect(on_word)
    synthesizer.bookmark_reached.connect(on_bookmark)
    synthesizer.synthesis_completed.connect(on_completed)
    try:
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
        deadline = time.monotonic() + 3.0
        with condition:
            while time.monotonic() < deadline:
                marks_ready = all(m in bookmarks for m in expected_line_marks)
                settled = done and marks_ready and (time.monotonic() - last_event_at >= 0.04)
                if settled:
                    break
                condition.wait(timeout=0.02)
            snapshot_bookmarks = bookmarks.copy()
            snapshot_boundaries = sorted(boundaries, key=lambda boundary: boundary.start_us)
    finally:
        synthesizer.synthesis_word_boundary.disconnect_all()
        synthesizer.bookmark_reached.disconnect_all()
        synthesizer.synthesis_completed.disconnect_all()
    write_clips(
        destination,
        request["lines"],
        markers,
        result.audio_data,
        snapshot_bookmarks,
        snapshot_boundaries,
    )
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
