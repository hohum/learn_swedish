"""Report Azure's timing metadata for one fixed phrase, without saving credentials or audio."""

import io
import json
import wave
from datetime import timedelta
from threading import Condition

import prepare_azure_speech as azure


def diagnose(config: azure.speechsdk.SpeechConfig) -> None:
    text = "Bokstaven D"
    synthesizer = azure.speechsdk.SpeechSynthesizer(speech_config=config, audio_config=None)
    condition = Condition()
    events: list[dict[str, str | int]] = []
    completed: list[str] = []

    def on_boundary(event: azure.speechsdk.SpeechSynthesisWordBoundaryEventArgs) -> None:
        entry = {
            "type": str(event.boundary_type),
            "text": event.text,
            "text_offset": event.text_offset,
            "word_length": event.word_length,
            "start_us": event.audio_offset // 10,
            "duration_us": event.duration // timedelta(microseconds=1),
            "result_id": event.result_id,
        }
        with condition:
            events.append(entry)
            condition.notify_all()

    def on_completed(event: azure.speechsdk.SpeechSynthesisEventArgs) -> None:
        with condition:
            completed.append(event.result.result_id)
            condition.notify_all()

    synthesizer.synthesis_word_boundary.connect(on_boundary)
    synthesizer.synthesis_completed.connect(on_completed)
    try:
        result = synthesizer.speak_text_async(text).get()
        if result.reason != azure.speechsdk.ResultReason.SynthesizingAudioCompleted:
            failure = (
                result.cancellation_details.error_code
                if result.reason == azure.speechsdk.ResultReason.Canceled else result.reason
            )
            raise RuntimeError(f"Azure diagnostic synthesis failed ({failure})")
        with wave.open(io.BytesIO(result.audio_data), "rb") as audio:
            duration_us = audio.getnframes() * 1_000_000 // audio.getframerate()
        # Observe the same ten-second window as preparation, retaining every
        # event type and result ID rather than filtering potentially useful evidence.
        with condition:
            condition.wait_for(lambda: False, timeout=10.0)
            report = {
                "text": text,
                "result_id": result.result_id,
                "completion_result_ids": list(completed),
                "events": list(events),
                "audio_duration_us": duration_us,
            }
        print(json.dumps(report, ensure_ascii=False, indent=2))
    finally:
        synthesizer.synthesis_word_boundary.disconnect_all()
        synthesizer.synthesis_completed.disconnect_all()


def main() -> None:
    print(
        "Diagnostic only: Azure will receive the single phrase 'Bokstaven D'.\n"
        "It takes about ten seconds. No key or audio is saved; the app's library is unchanged.\n"
        "Only the fixed phrase and its timing metadata are included in the report."
    )
    key, region = azure.credentials()
    try:
        diagnose(azure.speech_config(key, region))
    except RuntimeError as error:
        raise RuntimeError(str(error).replace(key, "[redacted]")) from None


if __name__ == "__main__":
    main()
