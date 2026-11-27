import io
import json
import re
import tempfile
import threading
import unittest
import warnings
import wave
from datetime import timedelta
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import prepare_azure_speech as azure
import diagnose_azure_speech as diagnostic


def silent_wav(frames=24_000):
    buffer = io.BytesIO()
    with wave.open(buffer, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(24_000)
        audio.writeframes(bytes(frames * 2))
    return buffer.getvalue()


class PrepareAzureSpeechTests(unittest.TestCase):
    def test_recorded_d_events_keep_spoken_punctuation_label_but_ignore_actual_punctuation_and_sentences(self):
        for text in ["Bokstaven D", "Bokstaven D."]:
            with (
                self.subTest(text=text),
                tempfile.TemporaryDirectory() as temporary,
                patch.object(azure.speechsdk, "SpeechConfig"),
                patch.object(azure.speechsdk, "SpeechSynthesizer") as factory,
                patch.object(azure.Condition, "wait_for", side_effect=lambda predicate, timeout: predicate()),
                patch("sys.stdout", new_callable=io.StringIO),
            ):
                synthesizer = factory.return_value
                result = SimpleNamespace(
                    result_id="recorded-d-result",
                    reason=azure.speechsdk.ResultReason.SynthesizingAudioCompleted,
                    audio_data=silent_wav(53_700),
                )

                def synthesize():
                    callback = synthesizer.synthesis_word_boundary.connect.call_args.args[0]
                    for kind, offset, length, start_us, duration_us in [
                        (azure.speechsdk.SpeechSynthesisBoundaryType.Word, 0, 9, 62_500, 825_000),
                        (azure.speechsdk.SpeechSynthesisBoundaryType.Punctuation, 10, 1, 1_025_000, 512_500),
                        (azure.speechsdk.SpeechSynthesisBoundaryType.Sentence, 0, len(text), 62_500, 1_475_000),
                    ]:
                        callback(SimpleNamespace(
                            result_id=result.result_id,
                            boundary_type=kind,
                            text_offset=offset,
                            word_length=length,
                            audio_offset=start_us * 10,
                            duration=timedelta(microseconds=duration_us),
                        ))
                    if text.endswith("."):
                        callback(SimpleNamespace(
                            result_id=result.result_id,
                            boundary_type=azure.speechsdk.SpeechSynthesisBoundaryType.Punctuation,
                            text_offset=11,
                            word_length=1,
                            audio_offset=16_250_000,
                            duration=timedelta(milliseconds=125),
                        ))
                    synthesizer.synthesis_completed.connect.call_args.args[0](SimpleNamespace(result=result))
                    return result

                synthesizer.speak_text_async.return_value.get.side_effect = synthesize
                azure.prepare(Path(temporary), [text], "not-a-real-key", "australiaeast")
                metadata = json.loads((Path(temporary) / f"{text}.json").read_text(encoding="utf-8"))
                self.assertEqual(metadata, {
                    "duration_us": 2_237_500,
                    "words": [
                        {"word": "Bokstaven", "start_us": 62_500, "end_us": 887_500},
                        {"word": text.split()[1], "start_us": 1_025_000, "end_us": 1_537_500},
                    ],
                })

    def test_diagnostic_reports_raw_metadata_without_key_or_library_writes(self):
        with (
            patch.object(azure.speechsdk, "SpeechSynthesizer") as factory,
            patch.object(azure.Condition, "wait_for", return_value=False),
            patch("sys.stdout", new_callable=io.StringIO) as stdout,
        ):
            synthesizer = factory.return_value
            result = SimpleNamespace(
                result_id="diagnostic-utterance",
                reason=azure.speechsdk.ResultReason.SynthesizingAudioCompleted,
                audio_data=silent_wav(),
            )

            def synthesize():
                synthesizer.synthesis_word_boundary.connect.call_args.args[0](SimpleNamespace(
                    result_id="diagnostic-utterance",
                    boundary_type=azure.speechsdk.SpeechSynthesisBoundaryType.Word,
                    text="D",
                    text_offset=10,
                    word_length=0,
                    audio_offset=3_000_000,
                    duration=timedelta(milliseconds=80),
                ))
                synthesizer.synthesis_completed.connect.call_args.args[0](SimpleNamespace(result=result))
                return result

            synthesizer.speak_text_async.return_value.get.side_effect = synthesize
            diagnostic.diagnose(SimpleNamespace(subscription="not-a-real-key"))
            report = json.loads(stdout.getvalue())
            self.assertEqual(report["text"], "Bokstaven D")
            self.assertEqual(report["audio_duration_us"], 1_000_000)
            self.assertEqual(report["completion_result_ids"], ["diagnostic-utterance"])
            self.assertEqual(report["events"][0]["word_length"], 0)
            self.assertNotIn("not-a-real-key", stdout.getvalue())

    def test_waits_for_late_matching_boundaries_even_after_completion_and_inside_a_hyphenated_word(self):
        for text in ["Bokstaven D", "Snabel-a"]:
            timers = []
            with (
                self.subTest(text=text),
                tempfile.TemporaryDirectory() as temporary,
                patch.object(azure.speechsdk, "SpeechConfig"),
                patch.object(azure.speechsdk, "SpeechSynthesizer") as factory,
                patch("sys.stdout", new_callable=io.StringIO),
            ):
                synthesizer = factory.return_value
                result = SimpleNamespace(
                    result_id="current-utterance",
                    reason=azure.speechsdk.ResultReason.SynthesizingAudioCompleted,
                    audio_data=silent_wav(),
                )
                first, last = list(re.finditer(r"\w+", text))

                def synthesize():
                    callback = synthesizer.synthesis_word_boundary.connect.call_args.args[0]

                    def boundary(result_id, match, start):
                        return SimpleNamespace(
                            result_id=result_id,
                            boundary_type=azure.speechsdk.SpeechSynthesisBoundaryType.Word,
                            text_offset=match.start(),
                            word_length=len(match.group()),
                            audio_offset=start * 10,
                            duration=timedelta(milliseconds=80),
                        )

                    callback(boundary("current-utterance", first, 100_000))
                    completed = synthesizer.synthesis_completed.connect.call_args
                    completed.args[0](SimpleNamespace(result=result))

                    def deliver_late_metadata():
                        callback(boundary("another-utterance", last, 200_000))
                        callback(boundary("current-utterance", last, 300_000))

                    timer = threading.Timer(0.05, deliver_late_metadata)
                    timers.append(timer)
                    timer.start()
                    return result

                synthesizer.speak_text_async.return_value.get.side_effect = synthesize
                try:
                    azure.prepare(Path(temporary), [text], "not-a-real-key", "australiaeast")
                    metadata = json.loads((Path(temporary) / f"{text}.json").read_text(encoding="utf-8"))
                    expected = (
                        [{"word": text, "start_us": 100_000, "end_us": 380_000}]
                        if "-" in text else [
                            {"word": "Bokstaven", "start_us": 100_000, "end_us": 180_000},
                            {"word": "D", "start_us": 300_000, "end_us": 380_000},
                        ]
                    )
                    self.assertEqual(metadata["words"], expected)
                finally:
                    for timer in timers:
                        timer.join()

    def test_preparation_preserves_text_and_uses_measured_boundaries_including_split_hyphens(self):
        texts = ["Försök igen.", "Snyggt gjort!", "Siffran fem", "Siffran sju", "Snabel-a"]
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.object(azure.speechsdk, "SpeechConfig") as config,
            patch.object(azure.speechsdk, "SpeechSynthesizer") as factory,
            patch.object(azure.time, "sleep") as sleep,
            patch("sys.stdout", new_callable=io.StringIO),
        ):
            synthesizer = factory.return_value

            def synthesize(text):
                callback = synthesizer.synthesis_word_boundary.connect.call_args.args[0]
                for index, match in enumerate(re.finditer(r"\w+", text), 1):
                    callback(SimpleNamespace(
                        result_id="current-utterance",
                        boundary_type=azure.speechsdk.SpeechSynthesisBoundaryType.Word,
                        text_offset=match.start(),
                        word_length=len(match.group()),
                        audio_offset=index * 1_000_000,
                        duration=timedelta(milliseconds=80),
                    ))
                for punctuation in re.finditer(r"[^\w\s]", text):
                    callback(SimpleNamespace(
                        boundary_type=azure.speechsdk.SpeechSynthesisBoundaryType.Punctuation,
                        text_offset=punctuation.start(),
                        word_length=len(punctuation.group()),
                    ))
                result = SimpleNamespace(
                    result_id="current-utterance",
                    reason=azure.speechsdk.ResultReason.SynthesizingAudioCompleted,
                    audio_data=silent_wav(),
                )
                synthesizer.synthesis_completed.connect.call_args.args[0](SimpleNamespace(result=result))
                return result

            synthesizer.speak_text_async.return_value.get.side_effect = (
                lambda: synthesize(synthesizer.speak_text_async.call_args.args[0])
            )
            output = Path(temporary)
            azure.prepare(output, texts, "not-a-real-key", "australiaeast")
            self.assertEqual(config.return_value.speech_synthesis_voice_name, "sv-SE-SofieNeural")
            self.assertIsNone(factory.call_args.kwargs["audio_config"])
            self.assertEqual([call.args[0] for call in synthesizer.speak_text_async.call_args_list], texts)
            self.assertEqual([call.args[0] for call in sleep.call_args_list], [3.1] * (len(texts) - 1))
            for text in texts:
                metadata = json.loads((output / f"{text}.json").read_text(encoding="utf-8"))
                self.assertEqual(metadata["duration_us"], 1_000_000)
                self.assertEqual([word["word"] for word in metadata["words"]], text.split())
                with wave.open(str(output / f"{text}.wav"), "rb") as audio:
                    self.assertEqual(audio.getnframes(), 24_000)
            metadata = json.loads((output / "Snabel-a.json").read_text(encoding="utf-8"))
            self.assertEqual(metadata["words"], [
                {"word": "Snabel-a", "start_us": 100_000, "end_us": 280_000}
            ])
            metadata = json.loads((output / "Försök igen..json").read_text(encoding="utf-8"))
            self.assertEqual(metadata["words"][1], {
                "word": "igen.", "start_us": 200_000, "end_us": 280_000,
            })

    def test_missing_overlapping_out_of_order_or_out_of_range_timings_fail(self):
        first = azure.Boundary(0, 6, 100_000, 180_000)
        second = azure.Boundary(7, 4, 200_000, 280_000)
        for boundaries in [
            [],
            [first],
            [first, azure.Boundary(7, 4, 170_000, 280_000)],
            [first, azure.Boundary(7, 4, 200_000, 1_100_000)],
            [azure.Boundary(0, 20, 100_000, 180_000), second],
            [azure.Boundary(7, 4, 100_000, 180_000), azure.Boundary(0, 6, 200_000, 280_000)],
        ]:
            with self.subTest(boundaries=boundaries), self.assertRaises(ValueError):
                azure.word_timings("Försök igen.", boundaries, 1_000_000)

    def test_service_cancellation_does_not_save_audio_or_expose_diagnostics(self):
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.object(azure.speechsdk, "SpeechConfig"),
            patch.object(azure.speechsdk, "SpeechSynthesizer") as factory,
        ):
            factory.return_value.speak_text_async.return_value.get.return_value = SimpleNamespace(
                reason=azure.speechsdk.ResultReason.Canceled,
                cancellation_details=SimpleNamespace(
                    error_code=azure.speechsdk.CancellationErrorCode.AuthenticationFailure,
                    error_details="private-diagnostics",
                ),
            )
            with self.assertRaisesRegex(RuntimeError, "Check your key") as caught:
                azure.prepare(Path(temporary), ["Siffran fem"], "not-a-real-key", "australiaeast")
            self.assertNotIn("private-diagnostics", str(caught.exception))
            self.assertIn("AuthenticationFailure", str(caught.exception))
            self.assertEqual(list(Path(temporary).iterdir()), [])

    def test_key_prompt_is_hidden_local_and_refuses_nonterminal_or_echo_fallback(self):
        with (
            patch("sys.argv", ["prepare_azure_speech.py", "unused", '["Siffran fem"]']),
            patch("sys.stdin.isatty", return_value=False),
            patch.object(azure.getpass, "getpass") as prompt,
        ):
            with self.assertRaisesRegex(RuntimeError, "interactively"):
                azure.main()
            prompt.assert_not_called()
        with (
            patch("sys.argv", ["prepare_azure_speech.py", "unused", '["Siffran fem"]']),
            patch("sys.stdin.isatty", return_value=True),
            patch.dict("os.environ", {"SPEECH_REGION": "australiaeast"}),
            patch.object(azure.getpass, "getpass", return_value="not-a-real-key") as prompt,
            patch.object(azure, "prepare") as prepare,
            patch("sys.stdout", new_callable=io.StringIO) as stdout,
        ):
            azure.main()
            prepare.assert_called_once_with(Path("unused"), ["Siffran fem"], "not-a-real-key", "australiaeast")
            self.assertNotIn("not-a-real-key", stdout.getvalue())
            prompt.side_effect = lambda _: warnings.warn("No hidden input", azure.getpass.GetPassWarning)
            with self.assertRaises(azure.getpass.GetPassWarning):
                azure.main()

    def test_missing_callbacks_time_out_without_writing_incomplete_audio(self):
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.object(azure.speechsdk, "SpeechConfig"),
            patch.object(azure.speechsdk, "SpeechSynthesizer") as factory,
            patch.object(azure.Condition, "wait_for", return_value=False),
        ):
            synthesizer = factory.return_value
            synthesizer.speak_text_async.return_value.get.return_value = SimpleNamespace(
                result_id="current-utterance",
                reason=azure.speechsdk.ResultReason.SynthesizingAudioCompleted,
                audio_data=silent_wav(),
            )
            with self.assertRaisesRegex(RuntimeError, "complete word timings.*Bokstaven D"):
                azure.prepare(Path(temporary), ["Bokstaven D"], "not-a-real-key", "australiaeast")
            self.assertEqual(list(Path(temporary).iterdir()), [])
            synthesizer.synthesis_word_boundary.disconnect_all.assert_called_once()
            synthesizer.synthesis_completed.disconnect_all.assert_called_once()

    def test_invalid_region_is_not_echoed_and_key_prompt_is_not_shown(self):
        invalid_region = "not-a-region-code"
        with (
            patch("sys.argv", ["prepare_azure_speech.py", "unused", '["Siffran fem"]']),
            patch("sys.stdin.isatty", return_value=True),
            patch.dict("os.environ", {"SPEECH_REGION": invalid_region}),
            patch.object(azure.getpass, "getpass") as prompt,
            patch("sys.stdout", new_callable=io.StringIO) as stdout,
        ):
            with self.assertRaisesRegex(ValueError, "SPEECH_REGION") as caught:
                azure.main()
            self.assertNotIn(invalid_region, str(caught.exception) + stdout.getvalue())
            prompt.assert_not_called()


if __name__ == "__main__":
    unittest.main()
