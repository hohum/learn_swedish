import io
import json
import tempfile
import unittest
import wave
from pathlib import Path
from xml.etree import ElementTree

from live_azure_speech import Boundary, markup, write_clips


class LiveAzureSpeechTests(unittest.TestCase):
    def test_ssml_has_three_voices_and_bookmarks_outside_prosody(self):
        ssml, marks = markup([
            {"voice": "sv-SE-MattiasNeural", "text": "God dag. Hej!", "pitch": 0, "rate": 0},
            {"voice": "sv-SE-SofieNeural", "text": "Vind & väder.", "pitch": -5, "rate": 4},
            {"voice": "sv-SE-HilleviNeural", "text": "Fika?", "pitch": 4, "rate": -7},
        ])
        document = ElementTree.fromstring(ssml)
        ns = {"s": "http://www.w3.org/2001/10/synthesis"}
        self.assertEqual([voice.attrib["name"] for voice in document], [
            "sv-SE-MattiasNeural", "sv-SE-SofieNeural", "sv-SE-HilleviNeural",
        ])
        self.assertEqual(marks, [["line0sentence0", "line0sentence1"], ["line1sentence0"], ["line2sentence0"]])
        self.assertEqual([b.attrib["mark"] for b in document.findall("s:voice/s:bookmark", ns)], [
            mark for group in marks for mark in group
        ])
        sofie = document[1].find("s:prosody", ns)
        self.assertEqual(sofie.attrib, {"pitch": "-5%", "rate": "+4%"})
        self.assertEqual(sofie.text, "Vind & väder.")

    def test_multi_voice_audio_splits_with_measured_words_and_sentence_starts(self):
        audio = io.BytesIO()
        with wave.open(audio, "wb") as wav:
            wav.setparams((1, 2, 24_000, 0, "NONE", "not compressed"))
            wav.writeframes(bytes(24_000 * 2 * 2))
        lines = [{"text": "Hej. Kristi-himmel!"}, {"text": "Tack."}, {"text": "Hej då!"}]
        markers = [["a", "b"], ["c"], ["d", "e"]]
        bookmarks = {"a": 0, "b": 4_000_000, "c": 10_000_000, "d": 15_000_000, "e": 18_000_000}
        boundaries = [
            Boundary("Hej", 10_000, 250_000), Boundary("Kristi", 410_000, 650_000),
            Boundary("himmel", 660_000, 870_000), Boundary("Tack", 1_010_000, 1_300_000),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp)
            write_clips(destination, lines, markers, audio.getvalue(), bookmarks, boundaries)
            clips = [json.loads((destination / f"{i}.json").read_text()) for i in range(3)]
            self.assertEqual(clips[0]["duration_us"], 1_000_000)
            self.assertEqual(clips[0]["sentence_starts_us"], [0, 400_000])
            self.assertEqual(clips[0]["words"][1], {
                "word": "Kristi-himmel!", "start_us": 410_000, "end_us": 870_000,
            })
            self.assertEqual(clips[1]["words"][0]["start_us"], 10_000)
            self.assertEqual(clips[2]["words"], [])
            self.assertEqual(clips[2]["sentence_starts_us"], [0, 300_000])
            with wave.open(str(destination / "1.wav"), "rb") as clip:
                self.assertEqual(clip.getnframes(), 12_000)

    def test_standalone_punctuation_keeps_measured_word_timings(self):
        audio = io.BytesIO()
        with wave.open(audio, "wb") as wav:
            wav.setparams((1, 2, 24_000, 0, "NONE", "not compressed"))
            wav.writeframes(bytes(24_000 * 2))
        lines = [{"text": "LÅG TEMP — TORKAR"}]
        markers = [["a"]]
        bookmarks = {"a": 0}
        boundaries = [
            Boundary("LÅG", 10_000, 220_000),
            Boundary("TEMP", 240_000, 480_000),
            Boundary("TORKAR", 560_000, 900_000),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp)
            write_clips(destination, lines, markers, audio.getvalue(), bookmarks, boundaries)
            clip = json.loads((destination / "0.json").read_text(encoding="utf-8"))
            self.assertEqual([w["word"] for w in clip["words"]], ["LÅG", "TEMP", "—", "TORKAR"])
            self.assertEqual(clip["words"][0], {"word": "LÅG", "start_us": 10_000, "end_us": 220_000})
            self.assertEqual(clip["words"][2], {"word": "—", "start_us": 480_000, "end_us": 480_001})
            self.assertEqual(clip["words"][3], {"word": "TORKAR", "start_us": 560_000, "end_us": 900_000})

    def test_numbers_and_normalized_tokens_preserve_anchored_word_timings(self):
        audio = io.BytesIO()
        with wave.open(audio, "wb") as wav:
            wav.setparams((1, 2, 24_000, 0, "NONE", "not compressed"))
            wav.writeframes(bytes(24_000 * 2 * 2))
        lines = [{"text": "Vi väljer NORMAL 40 i tvättstugan."}]
        markers = [["a"]]
        bookmarks = {"a": 0}
        # Azure speaks "40" as "fyrtio" and "i" with 1-us duration
        boundaries = [
            Boundary("Vi", 20_000, 150_000),
            Boundary("väljer", 170_000, 420_000),
            Boundary("NORMAL", 450_000, 780_000),
            Boundary("fyrtio", 800_000, 1_150_000),
            Boundary("i", 1_180_000, 1_180_001),
            Boundary("tvättstugan", 1_260_000, 1_850_000),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp)
            write_clips(destination, lines, markers, audio.getvalue(), bookmarks, boundaries)
            clip = json.loads((destination / "0.json").read_text(encoding="utf-8"))
            self.assertEqual(
                [w["word"] for w in clip["words"]],
                ["Vi", "väljer", "NORMAL", "40", "i", "tvättstugan."],
            )
            self.assertEqual(clip["words"][2], {"word": "NORMAL", "start_us": 450_000, "end_us": 780_000})
            self.assertEqual(clip["words"][3], {"word": "40", "start_us": 780_000, "end_us": 1_180_000})
            self.assertEqual(clip["words"][5]["start_us"], 1_260_000)


if __name__ == "__main__":
    unittest.main()
