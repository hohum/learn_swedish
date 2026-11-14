#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Alphabet,
    Numbers,
    Symbols,
    All,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub character: char,
    pub english: String,
    pub swedish: String,
    pub keys: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phrase {
    pub english: String,
    pub swedish: String,
}

impl From<Card> for Phrase {
    fn from(card: Card) -> Self {
        Self {
            english: card.english,
            swedish: card.swedish,
        }
    }
}

const CORRECT_PHRASES: &[(&str, &str)] = &[
    ("Good job!", "Bra jobbat!"),
    ("Well done!", "Snyggt gjort!"),
    ("Correct!", "Rätt!"),
    ("Perfect!", "Perfekt!"),
    ("Excellent!", "Utmärkt!"),
    ("Exactly right!", "Helt rätt!"),
    ("Nice!", "Snyggt!"),
    ("Great!", "Toppen!"),
    ("Awesome!", "Grymt!"),
    ("Brilliant!", "Kanon!"),
    ("Spot on!", "Klockrent!"),
    ("You nailed it!", "Där satt den!"),
    ("That's right!", "Precis!"),
    ("Well spotted!", "Bra där!"),
    ("Nicely done!", "Snyggt jobbat!"),
    ("Fantastic!", "Fantastiskt!"),
];

const INCORRECT_PHRASES: &[(&str, &str)] = &[
    ("Try again.", "Försök igen."),
    ("Not quite.", "Inte riktigt."),
    ("That wasn't correct.", "Det var inte rätt."),
    ("Take your time.", "Ta det lugnt."),
    ("You can try again.", "Du kan försöka igen."),
    ("Let's try again.", "Vi provar igen."),
    ("Oops!", "Hoppsan!"),
    ("Oops, try again.", "Oj, försök igen."),
    ("Close!", "Nära!"),
    ("Almost!", "Nästan!"),
    ("Not this time.", "Inte den här gången."),
    ("Have another go.", "Gör ett nytt försök."),
    ("Keep going!", "Kämpa på!"),
    ("No worries.", "Ingen fara."),
    ("Better luck next time.", "Bättre lycka nästa gång."),
    ("Don't give up!", "Ge inte upp!"),
];

const NEXT_PHRASES: &[(&str, &str)] = &[
    ("Here comes the next one.", "Här kommer nästa."),
    ("Let's take the next one.", "Vi tar nästa."),
    ("Time for the next one.", "Dags för nästa."),
    ("Now for the next one.", "Nu kommer nästa."),
    ("Let's move on.", "Nu går vi vidare."),
    ("Ready for the next one?", "Redo för nästa?"),
    ("On to the next one.", "Vidare till nästa."),
    ("Let's try another one.", "Vi provar en till."),
    ("Here's a new one.", "Här kommer en ny."),
    (
        "Let's carry on with the next one.",
        "Vi fortsätter med nästa.",
    ),
];

pub fn next_phrase_count() -> usize {
    NEXT_PHRASES.len()
}

pub fn next_phrase(choice: usize) -> Phrase {
    let (english, swedish) = NEXT_PHRASES[choice % NEXT_PHRASES.len()];
    Phrase {
        english: english.into(),
        swedish: swedish.into(),
    }
}

pub fn feedback_count(correct: bool) -> usize {
    if correct {
        CORRECT_PHRASES.len()
    } else {
        INCORRECT_PHRASES.len()
    }
}

pub fn feedback_phrase(correct: bool, choice: usize) -> Phrase {
    let choices = if correct {
        CORRECT_PHRASES
    } else {
        INCORRECT_PHRASES
    };
    let (english, swedish) = choices[choice % choices.len()];
    Phrase {
        english: english.into(),
        swedish: swedish.into(),
    }
}

pub fn speech_phrases() -> Vec<Phrase> {
    let mut phrases: Vec<Phrase> = cards(Category::All).into_iter().map(Phrase::from).collect();
    phrases.extend(
        CORRECT_PHRASES
            .iter()
            .chain(INCORRECT_PHRASES)
            .chain(NEXT_PHRASES)
            .map(|(english, swedish)| Phrase {
                english: (*english).into(),
                swedish: (*swedish).into(),
            }),
    );
    phrases
}

impl Card {
    pub fn display(&self) -> String {
        match self.character {
            ' ' => "␣".into(),
            c if c.is_alphabetic() => c.to_uppercase().collect(),
            c => c.to_string(),
        }
    }

    fn accepts(&self, input: &str) -> bool {
        let mut chars = input.chars();
        let Some(character) = chars.next() else {
            return false;
        };
        chars.next().is_none()
            && if self.character.is_alphabetic() {
                character.to_lowercase().eq(self.character.to_lowercase())
            } else {
                character == self.character
            }
    }
}

pub fn cards(category: Category) -> Vec<Card> {
    let mut cards = Vec::new();
    if matches!(category, Category::Alphabet | Category::All) {
        for character in "abcdefghijklmnopqrstuvwxyzåäö".chars() {
            let upper: String = character.to_uppercase().collect();
            cards.push(Card {
                character,
                english: format!("Letter {upper}"),
                swedish: format!("Bokstaven {upper}"),
                keys: format!("{upper} key (either case)"),
            });
        }
    }
    if matches!(category, Category::Numbers | Category::All) {
        for (character, name) in ('0'..='9').zip([
            "noll", "ett", "två", "tre", "fyra", "fem", "sex", "sju", "åtta", "nio",
        ]) {
            cards.push(Card {
                character,
                english: format!("Number {character}"),
                swedish: format!("Siffran {name}"),
                keys: format!("{character} key"),
            });
        }
    }
    if matches!(category, Category::Symbols | Category::All) {
        for (character, english, swedish, keys) in [
            ('/', "Forward slash", "Snedstreck", "Shift + 7"),
            ('\\', "Backslash", "Omvänt snedstreck", "AltGr + +"),
            ('|', "Vertical bar / pipe", "Lodstreck", "AltGr + <"),
            ('~', "Tilde", "Tilde", "AltGr + ¨, then Space"),
            (
                '`',
                "Backtick / grave accent",
                "Grav accent",
                "Shift + ´, then Space",
            ),
            (
                '^',
                "Caret / circumflex",
                "Cirkumflex",
                "Shift + ¨, then Space",
            ),
            ('-', "Hyphen / minus", "Bindestreck", "- key (right of .)"),
            ('_', "Underscore", "Understreck", "Shift + -"),
            ('.', "Period / dot", "Punkt", ". key"),
            (',', "Comma", "Komma", ", key"),
            (':', "Colon", "Kolon", "Shift + ."),
            (';', "Semicolon", "Semikolon", "Shift + ,"),
            ('!', "Exclamation mark", "Utropstecken", "Shift + 1"),
            ('?', "Question mark", "Frågetecken", "Shift + +"),
            (
                '\'',
                "Single quote / apostrophe",
                "Apostrof",
                "' key (right of Ä)",
            ),
            ('"', "Double quote", "Citattecken", "Shift + 2"),
            ('(', "Left parenthesis", "Vänsterparentes", "Shift + 8"),
            (')', "Right parenthesis", "Högerparentes", "Shift + 9"),
            (
                '[',
                "Left square bracket",
                "Vänster hakparentes",
                "AltGr + 8",
            ),
            (
                ']',
                "Right square bracket",
                "Höger hakparentes",
                "AltGr + 9",
            ),
            ('{', "Left brace", "Vänster klammerparentes", "AltGr + 7"),
            ('}', "Right brace", "Höger klammerparentes", "AltGr + 0"),
            ('<', "Less-than sign", "Mindre än", "< key (left of Z)"),
            ('>', "Greater-than sign", "Större än", "Shift + <"),
            ('=', "Equals sign", "Likhetstecken", "Shift + 0"),
            ('+', "Plus sign", "Plustecken", "+ key (right of 0)"),
            ('*', "Asterisk", "Asterisk", "Shift + '"),
            ('&', "Ampersand", "Et-tecken", "Shift + 6"),
            ('%', "Percent sign", "Procenttecken", "Shift + 5"),
            ('$', "Dollar sign", "Dollartecken", "AltGr + 4"),
            ('#', "Hash / number sign", "Nummertecken", "Shift + 3"),
            ('@', "At sign", "Snabel-a", "AltGr + 2"),
            (' ', "Space", "Mellanslag", "Space"),
            ('§', "Section sign", "Paragraftecken", "§ key (left of 1)"),
            ('¤', "Currency sign", "Valutatecken", "Shift + 4"),
            ('€', "Euro sign", "Eurotecken", "AltGr + E"),
            ('£', "Pound sign", "Pundtecken", "AltGr + 3"),
            ('´', "Acute accent", "Akut accent", "´ key twice"),
            ('¨', "Diaeresis", "Trema", "¨ key (right of Å) twice"),
        ] {
            cards.push(Card {
                character,
                english: english.into(),
                swedish: swedish.into(),
                keys: keys.into(),
            });
        }
    }
    cards
}

pub fn spoken_input(input: &str) -> Option<Phrase> {
    cards(Category::All)
        .into_iter()
        .find(|card| card.accepts(input))
        .map(Phrase::from)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Correct,
    Incorrect,
    AlreadyCorrect,
    Finished,
}

pub struct Lesson {
    cards: Vec<Card>,
    position: usize,
    answered: bool,
    correct: usize,
    mistakes: usize,
}

impl Lesson {
    pub fn new(category: Category) -> Self {
        Self {
            cards: cards(category),
            position: 0,
            answered: false,
            correct: 0,
            mistakes: 0,
        }
    }

    pub fn shuffled(category: Category, mut choose: impl FnMut(usize) -> usize) -> Self {
        let mut lesson = Self::new(category);
        for index in (1..lesson.cards.len()).rev() {
            let selected = choose(index + 1);
            assert!(
                selected <= index,
                "Shuffle choice must be below its exclusive upper bound"
            );
            lesson.cards.swap(index, selected);
        }
        lesson
    }

    pub fn current(&self) -> Option<&Card> {
        self.cards.get(self.position)
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn total(&self) -> usize {
        self.cards.len()
    }

    pub fn correct(&self) -> usize {
        self.correct
    }

    pub fn mistakes(&self) -> usize {
        self.mistakes
    }

    pub fn answered(&self) -> bool {
        self.answered
    }

    pub fn submit(&mut self, input: &str) -> Answer {
        let Some(card) = self.current() else {
            return Answer::Finished;
        };
        if self.answered {
            return Answer::AlreadyCorrect;
        }
        if card.accepts(input) {
            self.answered = true;
            self.correct += 1;
            Answer::Correct
        } else {
            self.mistakes += 1;
            Answer::Incorrect
        }
    }

    pub fn advance(&mut self) {
        if self.current().is_some() {
            self.position += 1;
            self.answered = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn curriculum_covers_swedish_alphabet_digits_and_shell_punctuation_once() {
        let alphabet: String = cards(Category::Alphabet)
            .iter()
            .map(|c| c.character)
            .collect();
        assert_eq!(alphabet, "abcdefghijklmnopqrstuvwxyzåäö");
        let all = cards(Category::All);
        let characters: HashSet<_> = all.iter().map(|c| c.character).collect();
        assert_eq!(characters.len(), all.len());
        for c in ' '..='~' {
            if !c.is_ascii_uppercase() {
                assert!(characters.contains(&c), "Missing {c:?}");
            }
        }
        assert!(
            all.iter()
                .all(|c| !c.english.is_empty() && !c.swedish.is_empty() && !c.keys.is_empty())
        );
    }

    #[test]
    fn accepts_either_letter_case_but_not_different_swedish_letters() {
        for (expected, matching, wrong) in [
            ('a', "A", "å"),
            ('å', "Å", "a"),
            ('ä', "Ä", "ö"),
            ('ö', "Ö", "o"),
        ] {
            let card = cards(Category::Alphabet)
                .into_iter()
                .find(|c| c.character == expected)
                .unwrap();
            assert!(card.accepts(matching));
            assert!(!card.accepts(wrong));
            assert!(!card.accepts(""));
            assert!(!card.accepts(&format!("{expected}{expected}")));
        }
    }

    #[test]
    fn requires_exact_symbols_including_space_and_committed_dead_keys() {
        let symbols = cards(Category::Symbols);
        for character in [' ', '\\', '~', '^', '`', '´', '¨'] {
            let card = symbols.iter().find(|c| c.character == character).unwrap();
            assert!(card.accepts(&character.to_string()));
            assert!(!card.accepts("x"));
        }
        let slash = symbols.iter().find(|c| c.character == '/').unwrap();
        assert!(!slash.accepts("\\"));
        assert!(!slash.accepts("//"));
    }

    #[test]
    fn spoken_input_names_the_entered_character_instead_of_the_prompt() {
        for (input, expected) in [
            ("b", "Bokstaven B"),
            ("B", "Bokstaven B"),
            ("Å", "Bokstaven Å"),
            ("ä", "Bokstaven Ä"),
            ("Ö", "Bokstaven Ö"),
            ("5", "Siffran fem"),
            ("\\", "Omvänt snedstreck"),
            ("/", "Snedstreck"),
            (" ", "Mellanslag"),
            ("´", "Akut accent"),
        ] {
            assert_eq!(
                spoken_input(input).map(|phrase| phrase.swedish).as_deref(),
                Some(expected)
            );
        }
        for input in ["", "ab", "a b", "é", "\n"] {
            assert_eq!(spoken_input(input), None);
        }
    }

    #[test]
    fn feedback_varies_by_result_and_all_spoken_phrases_have_translations() {
        let all = speech_phrases();
        for correct in [true, false] {
            let choices: HashSet<_> = (0..feedback_count(correct))
                .map(|index| {
                    let phrase = feedback_phrase(correct, index);
                    assert!(all.contains(&phrase));
                    assert!(!phrase.english.is_empty());
                    phrase.swedish
                })
                .collect();
            assert_eq!(choices.len(), 16);
        }
        let next: HashSet<_> = (0..next_phrase_count())
            .map(|index| {
                let phrase = next_phrase(index);
                assert!(all.contains(&phrase));
                assert!(!phrase.english.is_empty());
                phrase.swedish
            })
            .collect();
        assert_eq!(next.len(), 10);
        assert_eq!(next_phrase(next_phrase_count()), next_phrase(0));
        assert_ne!(feedback_phrase(true, 0), feedback_phrase(false, 0));
        for correct in [true, false] {
            assert_eq!(
                feedback_phrase(correct, feedback_count(correct)),
                feedback_phrase(correct, 0)
            );
        }
    }

    #[test]
    fn correct_answer_waits_for_manual_next_and_counts_only_once() {
        let mut lesson = Lesson::new(Category::Alphabet);
        assert_eq!(lesson.submit("b"), Answer::Incorrect);
        assert_eq!(lesson.position(), 0);
        assert_eq!(lesson.submit("a"), Answer::Correct);
        assert_eq!(lesson.submit("A"), Answer::AlreadyCorrect);
        assert_eq!(lesson.correct(), 1);
        assert_eq!(lesson.mistakes(), 1);
        assert_eq!(lesson.current().unwrap().character, 'a');
        lesson.advance();
        assert_eq!(lesson.current().unwrap().character, 'b');
        assert!(!lesson.answered());
    }

    #[test]
    fn shuffled_sessions_visit_each_card_once_and_keep_manual_progression() {
        for category in [
            Category::Alphabet,
            Category::Numbers,
            Category::Symbols,
            Category::All,
        ] {
            let ordered: Vec<_> = cards(category).iter().map(|card| card.character).collect();
            let mut bounds = Vec::new();
            let mut lesson = Lesson::shuffled(category, |upper| {
                bounds.push(upper);
                0
            });
            assert_eq!(bounds, (2..=ordered.len()).rev().collect::<Vec<_>>());
            let mut visited = Vec::new();
            while let Some(card) = lesson.current() {
                let character = card.character;
                visited.push(character);
                assert_eq!(lesson.submit(&character.to_string()), Answer::Correct);
                assert_eq!(lesson.current().unwrap().character, character);
                lesson.advance();
            }
            assert_ne!(visited, ordered);
            assert_eq!(visited.len(), ordered.len());
            assert_eq!(
                visited.iter().collect::<HashSet<_>>(),
                ordered.iter().collect::<HashSet<_>>()
            );
            assert_eq!(lesson.correct(), ordered.len());
        }
    }

    #[test]
    fn skipping_and_finishing_do_not_wrap_or_add_correct_answers() {
        let mut lesson = Lesson::new(Category::Numbers);
        lesson.advance();
        assert_eq!(lesson.submit("1"), Answer::Correct);
        for _ in 0..lesson.total() + 2 {
            lesson.advance();
        }
        assert!(lesson.current().is_none());
        assert_eq!(lesson.position(), lesson.total());
        assert_eq!(lesson.correct(), 1);
        assert_eq!(lesson.submit("0"), Answer::Finished);
        let restarted = Lesson::new(Category::Numbers);
        assert_eq!(restarted.current().unwrap().character, '0');
        assert_eq!(restarted.correct(), 0);
        assert_eq!(restarted.mistakes(), 0);
    }
}
