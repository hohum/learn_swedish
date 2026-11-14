import io
import json
import tempfile
import unittest
import wave
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import prepare_speech


class PrepareSpeechTests(unittest.TestCase):
    def test_synthesis_omits_sentence_punctuation_but_preserves_display_and_timing_words(self):
        texts = ["Bra jobbat!", "Försök igen.", "Rätt?", "Snabel-a", "Bokstaven Å"]

        def synthesize(text, *, include_alignments):
            self.assertTrue(include_alignments)
            alignments = []
            for index, word in enumerate(text.split()):
                if index:
                    alignments.append(SimpleNamespace(phoneme=" ", num_samples=10))
                alignments.append(SimpleNamespace(phoneme=word, num_samples=100))
            samples = sum(alignment.num_samples for alignment in alignments)
            yield SimpleNamespace(
                phoneme_alignments=alignments,
                audio_int16_array=[0] * samples,
                audio_int16_bytes=bytes(samples * 2),
                sample_channels=1,
                sample_width=2,
                sample_rate=22_050,
            )

        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with (
                patch.object(prepare_speech.PiperVoice, "load") as load,
                patch("sys.argv", ["prepare_speech.py", "model.onnx", temporary]),
                patch("sys.stdin", io.StringIO(json.dumps(texts))),
                patch("sys.stdout", new_callable=io.StringIO),
            ):
                load.return_value.synthesize.side_effect = synthesize
                prepare_speech.main()
                spoken = [call.args[0] for call in load.return_value.synthesize.call_args_list]
                self.assertEqual(spoken, ["Bra jobbat", "Försök igen", "Rätt", "Snabel-a", "Bokstaven Å"])

            for text in texts:
                with self.subTest(text=text):
                    metadata = json.loads((output / f"{text}.json").read_text(encoding="utf-8"))
                    self.assertEqual([word["word"] for word in metadata["words"]], text.split())
                    with wave.open(str(output / f"{text}.wav"), "rb") as audio:
                        self.assertEqual(
                            metadata["duration_us"],
                            audio.getnframes() * 1_000_000 // audio.getframerate(),
                        )


if __name__ == "__main__":
    unittest.main()
