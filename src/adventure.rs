use crate::lesson::{Phrase, Voice};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

mod xml;
use xml::{Element, escape_attr, escape_text};

const DEFAULT_SUBTITLE: &str = "A Swedish choose-your-own-adventure";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Home,
    Woods,
    Shop,
    Road,
    Ikea,
    Train,
    City,
}

impl Scene {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Woods => "woods",
            Self::Shop => "shop",
            Self::Road => "road",
            Self::Ikea => "ikea",
            Self::Train => "train",
            Self::City => "city",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "home" => Ok(Self::Home),
            "woods" => Ok(Self::Woods),
            "shop" => Ok(Self::Shop),
            "road" => Ok(Self::Road),
            "ikea" => Ok(Self::Ikea),
            "train" => Ok(Self::Train),
            "city" => Ok(Self::City),
            other => Err(format!(
                "Unknown scene {other:?}; expected home, woods, shop, road, ikea, train, or city"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorSpec {
    pub id: String,
    pub label: String,
    pub lead: bool,
    pub voices: Vec<Voice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub actor_id: String,
    pub speaker: String,
    pub voice: Voice,
    pub swedish: String,
    pub literal: Option<String>,
    pub english: String,
}

impl Line {
    pub fn phrase(&self) -> Phrase {
        Phrase {
            swedish: self.swedish.clone(),
            literal: self.literal.clone(),
            english: self.english.clone(),
            voice: Some(self.voice),
            speaker: Some(self.speaker.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub line: Line,
    pub reply: Line,
    pub next: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: String,
    pub scene: Scene,
    pub title: Line,
    pub setup: Vec<Line>,
    pub question: Line,
    pub choices: Vec<Choice>,
}

impl Page {
    pub fn phrases(&self) -> Vec<Phrase> {
        std::iter::once(self.title.phrase())
            .chain(self.setup.iter().map(Line::phrase))
            .chain(std::iter::once(self.question.phrase()))
            .chain(self.choices.iter().map(|choice| choice.line.phrase()))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceRotation {
    None,
    ByIndex,
    Random,
}

impl ChoiceRotation {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ByIndex => "by-index",
            Self::Random => "random",
        }
    }

    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim) {
            None | Some("none") => Ok(Self::None),
            Some("by-index") => Ok(Self::ByIndex),
            Some("random") => Ok(Self::Random),
            Some(other) => Err(format!(
                "Invalid choice-rotation {other:?}; expected \"none\", \"by-index\", or \"random\""
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ending {
    pub title: String,
    pub line: Line,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterItem {
    pub id: String,
    pub setup: Line,
    pub say: Line,
    pub reply: Line,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterStop {
    pub after: String,
    pub before: String,
    pub scene: Scene,
    pub title: Line,
    pub question: Line,
    pub encounters: Vec<EncounterItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterRoute {
    pub id: String,
    pub distractors: Vec<Choice>,
    pub stops: Vec<EncounterStop>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterPool {
    pub distractors: Vec<Choice>,
    pub routes: Vec<EncounterRoute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Story {
    pub id: Option<String>,
    pub lang: Option<String>,
    pub title: String,
    pub subtitle: String,
    pub start: String,
    pub outing_hub: Option<String>,
    pub narrator: String,
    pub protagonist: String,
    pub responder: String,
    pub actors: Vec<ActorSpec>,
    pub ending: Ending,
    pub choice_rotation: ChoiceRotation,
    pub pages: Vec<Page>,
    pub encounters: Option<EncounterPool>,
}

fn parse_voice(token: &str) -> Result<Voice, String> {
    match token.trim() {
        "mattias" | "sv-SE-MattiasNeural" => Ok(Voice::Mattias),
        "sofie" | "sv-SE-SofieNeural" => Ok(Voice::Sofie),
        "hillevi" | "sv-SE-HilleviNeural" => Ok(Voice::Hillevi),
        other => Err(format!(
            "Unknown voice {other:?}; expected mattias, sofie, or hillevi"
        )),
    }
}

fn voice_short_name(voice: Voice) -> &'static str {
    match voice {
        Voice::Mattias => "mattias",
        Voice::Sofie => "sofie",
        Voice::Hillevi => "hillevi",
    }
}

impl Story {
    pub fn load_default() -> Result<(PathBuf, Self), String> {
        let path = default_story_path()?;
        let story = Self::from_xml_file(&path)?;
        Ok((path, story))
    }

    pub fn from_xml_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("Could not read story file {}: {error}", path.display()))?;
        Self::from_xml(&content)
            .map_err(|error| format!("Invalid story XML in {}: {error}", path.display()))
    }

    pub fn from_xml(input: &str) -> Result<Self, String> {
        let root = xml::parse(input)?;
        if root.name != "story" {
            return Err(format!("Expected root <story>, found <{}>", root.name));
        }
        root.check_allowed_attrs(&["version", "id", "start", "outing-hub", "xml:lang"])?;
        root.check_allowed_children(&["meta", "cast", "ending", "pages", "encounters"])?;
        let version = root.required_attr("version")?;
        if version != "1.0" {
            return Err(format!(
                "Unsupported story XML version {version:?}; expected \"1.0\""
            ));
        }
        let id = root
            .attr("id")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let lang = root
            .attr("xml:lang")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        let meta = root.required_child("meta")?;
        meta.check_allowed_attrs(&[])?;
        meta.check_allowed_children(&["title", "subtitle"])?;
        let title = meta.required_child("title")?.normalized_text()?;
        if title.is_empty() {
            return Err("<meta><title> must not be empty".into());
        }
        let subtitle = match meta.child("subtitle")? {
            Some(sub) => {
                let text = sub.normalized_text()?;
                if text.is_empty() {
                    DEFAULT_SUBTITLE.to_owned()
                } else {
                    text
                }
            }
            None => DEFAULT_SUBTITLE.to_owned(),
        };

        let cast_el = root.required_child("cast")?;
        cast_el.check_allowed_attrs(&["protagonist", "responder", "narrator"])?;
        cast_el.check_allowed_children(&["actor"])?;
        let mut actors = Vec::new();
        let mut actor_by_id: HashMap<String, ActorSpec> = HashMap::new();
        for actor_el in &cast_el.children {
            actor_el.check_allowed_attrs(&["id", "label", "role", "voices"])?;
            if !actor_el.children.is_empty() || !actor_el.text.trim().is_empty() {
                return Err("<actor> must be empty".into());
            }
            let actor_id = actor_el.required_attr("id")?.to_owned();
            if actor_by_id.contains_key(&actor_id) {
                return Err(format!("Duplicate actor id {actor_id:?} in <cast>"));
            }
            let label = actor_el.required_attr("label")?.to_owned();
            let lead = match actor_el.attr("role").map(str::trim) {
                None | Some("guest") => false,
                Some("lead") => true,
                Some(other) => {
                    return Err(format!(
                        "Invalid role {other:?} on actor {actor_id:?}; expected \"lead\" or \"guest\""
                    ));
                }
            };
            let voices_raw = actor_el.required_attr("voices")?;
            let mut voices = Vec::new();
            for part in voices_raw.split(',') {
                let voice = parse_voice(part)?;
                if !voices.contains(&voice) {
                    voices.push(voice);
                }
            }
            if voices.is_empty() {
                return Err(format!("Actor {actor_id:?} must list at least one voice"));
            }
            let spec = ActorSpec {
                id: actor_id.clone(),
                label,
                lead,
                voices,
            };
            actor_by_id.insert(actor_id, spec.clone());
            actors.push(spec);
        }
        if actors.is_empty() {
            return Err("<cast> must define at least one <actor>".into());
        }

        let resolve_default = |attr_name: &str,
                               fallbacks: &[&str],
                               fallback_index: usize|
         -> Result<String, String> {
            if let Some(explicit) = cast_el
                .attr(attr_name)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if !actor_by_id.contains_key(explicit) {
                    return Err(format!(
                        "<cast {attr_name}={explicit:?}> references an undefined actor"
                    ));
                }
                return Ok(explicit.to_owned());
            }
            for candidate in fallbacks {
                if actor_by_id.contains_key(*candidate) {
                    return Ok((*candidate).to_owned());
                }
            }
            Ok(actors[fallback_index.min(actors.len() - 1)].id.clone())
        };
        let narrator = resolve_default("narrator", &["narrator"], 0)?;
        let protagonist = resolve_default("protagonist", &["protagonist", "you"], 0)?;
        let responder = resolve_default("responder", &["responder"], 1)?;

        let parse_line = |el: &Element,
                          default_actor: Option<&str>,
                          allow_empty_english: bool|
         -> Result<Line, String> {
            el.check_allowed_attrs(&["actor"])?;
            el.check_allowed_children(&["sv", "lit", "en"])?;
            let actor_id = match el.attr("actor").map(str::trim).filter(|s| !s.is_empty()) {
                Some(explicit) => explicit,
                None => default_actor
                    .ok_or_else(|| format!("<{}> requires an actor=\"...\" attribute", el.name))?,
            };
            let spec = actor_by_id
                .get(actor_id)
                .ok_or_else(|| format!("<{}> references unknown actor {actor_id:?}", el.name))?;
            let swedish = el.required_child("sv")?.normalized_text()?;
            if swedish.is_empty() {
                return Err(format!("<{}><sv> must not be empty", el.name));
            }
            let literal = match el.child("lit")? {
                Some(lit_el) => {
                    let lit = lit_el.normalized_text()?;
                    if lit.is_empty() {
                        return Err(format!("<{}><lit> must not be empty when present", el.name));
                    }
                    Some(lit)
                }
                None => None,
            };
            let english = match el.child("en")? {
                Some(en_el) => en_el.normalized_text()?,
                None if allow_empty_english => String::new(),
                None => return Err(format!("<{}> is missing required <en> element", el.name)),
            };
            if !allow_empty_english && english.is_empty() {
                return Err(format!("<{}><en> must not be empty", el.name));
            }
            Ok(Line {
                actor_id: spec.id.clone(),
                speaker: spec.label.clone(),
                voice: spec.voices[0],
                swedish,
                literal,
                english,
            })
        };

        let ending = if let Some(ending_el) = root.child("ending")? {
            ending_el.check_allowed_attrs(&["title"])?;
            ending_el.check_allowed_children(&["line"])?;
            let end_title = ending_el.required_attr("title")?.to_owned();
            let line = parse_line(ending_el.required_child("line")?, Some(&responder), false)?;
            Ending {
                title: end_title,
                line,
            }
        } else {
            let responder_spec = &actor_by_id[&responder];
            Ending {
                title: "Slut · The End".into(),
                line: Line {
                    actor_id: responder_spec.id.clone(),
                    speaker: responder_spec.label.clone(),
                    voice: responder_spec.voices[0],
                    swedish: "Bra jobbat! Berättelsen är slut.".into(),
                    literal: Some("Good worked! Story-the is finished.".into()),
                    english: "Well done! You have reached the end of the story.".into(),
                },
            }
        };

        let pages_el = root.required_child("pages")?;
        pages_el.check_allowed_attrs(&["choice-rotation"])?;
        pages_el.check_allowed_children(&["page"])?;
        let choice_rotation = ChoiceRotation::parse(pages_el.attr("choice-rotation"))?;
        let mut pages = Vec::new();
        for page_el in &pages_el.children {
            page_el.check_allowed_attrs(&["id", "scene"])?;
            page_el.check_allowed_children(&["title", "setup", "question", "choices"])?;
            let page_id = page_el.required_attr("id")?.to_owned();
            let scene = Scene::parse(page_el.required_attr("scene")?)?;
            let page_title = parse_line(page_el.required_child("title")?, Some(&narrator), true)?;
            let mut setup = Vec::new();
            if let Some(setup_el) = page_el.child("setup")? {
                setup_el.check_allowed_attrs(&[])?;
                setup_el.check_allowed_children(&["line"])?;
                for line_el in &setup_el.children {
                    setup.push(parse_line(line_el, None, false)?);
                }
            }
            let question =
                parse_line(page_el.required_child("question")?, Some(&responder), false)?;
            let choices_el = page_el.required_child("choices")?;
            choices_el.check_allowed_attrs(&[])?;
            choices_el.check_allowed_children(&["choice"])?;
            let mut choices = Vec::new();
            for choice_el in &choices_el.children {
                choice_el.check_allowed_attrs(&["next"])?;
                choice_el.check_allowed_children(&["say", "reply"])?;
                let next = choice_el
                    .attr("next")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                if choice_el.attr("next").is_some() && next.is_none() {
                    return Err(format!(
                        "<choice> on page {page_id:?} has an empty next attribute"
                    ));
                }
                let say = parse_line(choice_el.required_child("say")?, Some(&protagonist), false)?;
                let reply =
                    parse_line(choice_el.required_child("reply")?, Some(&responder), false)?;
                choices.push(Choice {
                    line: say,
                    reply,
                    next,
                });
            }
            pages.push(Page {
                id: page_id,
                scene,
                title: page_title,
                setup,
                question,
                choices,
            });
        }
        if pages.is_empty() {
            return Err("<pages> must contain at least one <page>".into());
        }

        let start = root
            .attr("start")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map_or_else(|| pages[0].id.clone(), str::to_owned);
        let outing_hub = root
            .attr("outing-hub")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        let parse_distractors = |container: Option<&Element>| -> Result<Vec<Choice>, String> {
            let Some(el) = container else {
                return Ok(Vec::new());
            };
            el.check_allowed_attrs(&[])?;
            el.check_allowed_children(&["distractor"])?;
            let mut list = Vec::new();
            for item in &el.children {
                item.check_allowed_attrs(&[])?;
                item.check_allowed_children(&["say", "reply"])?;
                let say = parse_line(item.required_child("say")?, Some(&protagonist), false)?;
                let reply = parse_line(item.required_child("reply")?, Some(&responder), false)?;
                list.push(Choice {
                    line: say,
                    reply,
                    next: None,
                });
            }
            Ok(list)
        };

        let encounters = if let Some(enc_el) = root.child("encounters")? {
            enc_el.check_allowed_attrs(&[])?;
            enc_el.check_allowed_children(&["distractors", "route"])?;
            let distractors = parse_distractors(enc_el.child("distractors")?)?;
            let mut routes = Vec::new();
            for route_el in enc_el.children.iter().filter(|c| c.name == "route") {
                route_el.check_allowed_attrs(&["id"])?;
                route_el.check_allowed_children(&["distractors", "stop"])?;
                let route_id = route_el.required_attr("id")?.to_owned();
                let route_distractors = parse_distractors(route_el.child("distractors")?)?;
                let mut stops = Vec::new();
                for stop_el in route_el.children.iter().filter(|c| c.name == "stop") {
                    stop_el.check_allowed_attrs(&["after", "before", "scene"])?;
                    stop_el.check_allowed_children(&["title", "question", "encounter"])?;
                    let after = stop_el.required_attr("after")?.to_owned();
                    let before = stop_el.required_attr("before")?.to_owned();
                    let scene = Scene::parse(stop_el.required_attr("scene")?)?;
                    let stop_title =
                        parse_line(stop_el.required_child("title")?, Some(&narrator), true)?;
                    let question =
                        parse_line(stop_el.required_child("question")?, Some(&responder), false)?;
                    let mut items = Vec::new();
                    for item_el in stop_el.children.iter().filter(|c| c.name == "encounter") {
                        item_el.check_allowed_attrs(&["id"])?;
                        item_el.check_allowed_children(&["setup", "say", "reply"])?;
                        let enc_id = item_el.required_attr("id")?.to_owned();
                        let setup = parse_line(item_el.required_child("setup")?, None, false)?;
                        let say =
                            parse_line(item_el.required_child("say")?, Some(&protagonist), false)?;
                        let reply =
                            parse_line(item_el.required_child("reply")?, Some(&responder), false)?;
                        items.push(EncounterItem {
                            id: enc_id,
                            setup,
                            say,
                            reply,
                        });
                    }
                    stops.push(EncounterStop {
                        after,
                        before,
                        scene,
                        title: stop_title,
                        question,
                        encounters: items,
                    });
                }
                routes.push(EncounterRoute {
                    id: route_id,
                    distractors: route_distractors,
                    stops,
                });
            }
            Some(EncounterPool {
                distractors,
                routes,
            })
        } else {
            None
        };

        let story = Self {
            id,
            lang,
            title,
            subtitle,
            start,
            outing_hub,
            narrator,
            protagonist,
            responder,
            actors,
            ending,
            choice_rotation,
            pages,
            encounters,
        };
        story.validate()?;
        Ok(story)
    }

    fn validate(&self) -> Result<(), String> {
        let mut all_ids = HashSet::new();
        let mut page_map = HashMap::new();
        for page in &self.pages {
            if page.id == "done" {
                return Err("\"done\" is reserved and cannot be used as a <page id>".into());
            }
            if !all_ids.insert(page.id.as_str()) {
                return Err(format!("Duplicate page id {:?}", page.id));
            }
            if !(2..=4).contains(&page.choices.len()) {
                return Err(format!(
                    "Page {:?} has {} choices; expected 2 to 4",
                    page.id,
                    page.choices.len()
                ));
            }
            if !page.choices.iter().any(|choice| choice.next.is_some()) {
                return Err(format!(
                    "Page {:?} must have at least one choice with next=\"...\"",
                    page.id
                ));
            }
            let mut answers = HashSet::new();
            for choice in &page.choices {
                let norm = normalise(&choice.line.swedish);
                if norm.is_empty() || !answers.insert(norm) {
                    return Err(format!(
                        "Page {:?} has duplicate or empty normalized Swedish choices",
                        page.id
                    ));
                }
            }
            page_map.insert(page.id.as_str(), page);
        }

        if !page_map.contains_key(self.start.as_str()) {
            return Err(format!(
                "Story start page {:?} does not exist in <pages>",
                self.start
            ));
        }
        if let Some(hub) = &self.outing_hub
            && !page_map.contains_key(hub.as_str())
        {
            return Err(format!(
                "Story outing-hub page {hub:?} does not exist in <pages>"
            ));
        }
        for page in &self.pages {
            for choice in &page.choices {
                if let Some(next) = &choice.next
                    && next != "done"
                    && !page_map.contains_key(next.as_str())
                {
                    return Err(format!(
                        "Page {:?} links to unknown next page {next:?}",
                        page.id
                    ));
                }
            }
        }

        if let Some(pool) = &self.encounters {
            let mut route_ids = HashSet::new();
            let mut used_links = HashSet::new();
            for route in &pool.routes {
                if !route_ids.insert(route.id.as_str()) {
                    return Err(format!("Duplicate encounter route id {:?}", route.id));
                }
                if route.stops.is_empty() {
                    return Err(format!(
                        "Encounter route {:?} must contain at least one <stop>",
                        route.id
                    ));
                }
                let stop_count = route.stops.len();
                let local_usable = route.distractors.len().min(stop_count);
                if local_usable + pool.distractors.len() < 3 * stop_count {
                    return Err(format!(
                        "Encounter route {:?} needs {} distractors across route and shared pools, found {}",
                        route.id,
                        3 * stop_count,
                        local_usable + pool.distractors.len()
                    ));
                }
                let distractor_norms: HashSet<_> = pool
                    .distractors
                    .iter()
                    .chain(&route.distractors)
                    .map(|d| normalise(&d.line.swedish))
                    .collect();
                if distractor_norms.len() != pool.distractors.len() + route.distractors.len() {
                    return Err(format!(
                        "Duplicate normalized distractor text in route {:?}",
                        route.id
                    ));
                }
                for stop in &route.stops {
                    let Some(after_page) = page_map.get(stop.after.as_str()) else {
                        return Err(format!(
                            "Encounter stop after={:?} references unknown page",
                            stop.after
                        ));
                    };
                    if stop.before != "done" && !page_map.contains_key(stop.before.as_str()) {
                        return Err(format!(
                            "Encounter stop before={:?} references unknown page",
                            stop.before
                        ));
                    }
                    if !after_page
                        .choices
                        .iter()
                        .any(|c| c.next.as_deref() == Some(stop.before.as_str()))
                    {
                        return Err(format!(
                            "Page {:?} has no choice linking to {:?} for encounter stop",
                            stop.after, stop.before
                        ));
                    }
                    if !used_links.insert((stop.after.as_str(), stop.before.as_str())) {
                        return Err(format!(
                            "Duplicate encounter stop between {:?} and {:?}",
                            stop.after, stop.before
                        ));
                    }
                    if stop.encounters.is_empty() {
                        return Err(format!(
                            "Encounter stop between {:?} and {:?} has no <encounter> entries",
                            stop.after, stop.before
                        ));
                    }
                    for enc in &stop.encounters {
                        if enc.id == "done" || !all_ids.insert(enc.id.as_str()) {
                            return Err(format!("Duplicate or reserved encounter id {:?}", enc.id));
                        }
                        if distractor_norms.contains(&normalise(&enc.say.swedish)) {
                            return Err(format!(
                                "Encounter {:?} answer collides with a distractor",
                                enc.id
                            ));
                        }
                    }
                }
            }
        }

        // Verify DAG (no cycles) and that every reachable branch can reach "done".
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum State {
            Visiting,
            Verified,
        }
        fn dfs<'a>(
            id: &'a str,
            pages: &HashMap<&'a str, &'a Page>,
            states: &mut HashMap<&'a str, State>,
        ) -> Result<(), String> {
            if id == "done" {
                return Ok(());
            }
            match states.get(id) {
                Some(State::Verified) => return Ok(()),
                Some(State::Visiting) => {
                    return Err(format!("Story contains a cycle involving page {id:?}"));
                }
                None => {}
            }
            states.insert(id, State::Visiting);
            let page = pages[id];
            for choice in &page.choices {
                if let Some(next) = &choice.next {
                    dfs(next, pages, states)?;
                }
            }
            states.insert(id, State::Verified);
            Ok(())
        }
        let mut states = HashMap::new();
        dfs(&self.start, &page_map, &mut states)?;
        if let Some(hub) = &self.outing_hub {
            dfs(hub, &page_map, &mut states)?;
        }
        Ok(())
    }

    pub fn to_xml(&self) -> String {
        let mut out =
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<story version=\"1.0\"");
        if let Some(lang) = &self.lang {
            write!(out, " xml:lang=\"{}\"", escape_attr(lang)).unwrap();
        }
        if let Some(id) = &self.id {
            write!(out, " id=\"{}\"", escape_attr(id)).unwrap();
        }
        write!(out, " start=\"{}\"", escape_attr(&self.start)).unwrap();
        if let Some(hub) = &self.outing_hub {
            write!(out, " outing-hub=\"{}\"", escape_attr(hub)).unwrap();
        }
        out.push_str(">\n  <meta>\n");
        writeln!(out, "    <title>{}</title>", escape_text(&self.title)).unwrap();
        writeln!(
            out,
            "    <subtitle>{}</subtitle>",
            escape_text(&self.subtitle)
        )
        .unwrap();
        out.push_str("  </meta>\n\n");

        writeln!(
            out,
            "  <cast protagonist=\"{}\" responder=\"{}\" narrator=\"{}\">",
            escape_attr(&self.protagonist),
            escape_attr(&self.responder),
            escape_attr(&self.narrator),
        )
        .unwrap();
        for actor in &self.actors {
            let voices = actor
                .voices
                .iter()
                .map(|v| voice_short_name(*v))
                .collect::<Vec<_>>()
                .join(",");
            writeln!(
                out,
                "    <actor id=\"{}\" label=\"{}\" role=\"{}\" voices=\"{}\"/>",
                escape_attr(&actor.id),
                escape_attr(&actor.label),
                if actor.lead { "lead" } else { "guest" },
                voices,
            )
            .unwrap();
        }
        out.push_str("  </cast>\n\n");

        writeln!(
            out,
            "  <ending title=\"{}\">",
            escape_attr(&self.ending.title)
        )
        .unwrap();
        write_line(&mut out, "    ", "line", &self.ending.line, None);
        out.push_str("  </ending>\n\n");

        if self.choice_rotation == ChoiceRotation::None {
            out.push_str("  <pages>\n");
        } else {
            writeln!(
                out,
                "  <pages choice-rotation=\"{}\">",
                self.choice_rotation.as_str()
            )
            .unwrap();
        }
        for page in &self.pages {
            writeln!(
                out,
                "    <page id=\"{}\" scene=\"{}\">",
                escape_attr(&page.id),
                page.scene.as_str()
            )
            .unwrap();
            write_line(
                &mut out,
                "      ",
                "title",
                &page.title,
                Some(&self.narrator),
            );
            if !page.setup.is_empty() {
                out.push_str("      <setup>\n");
                for line in &page.setup {
                    write_line(&mut out, "        ", "line", line, None);
                }
                out.push_str("      </setup>\n");
            }
            write_line(&mut out, "      ", "question", &page.question, None);
            out.push_str("      <choices>\n");
            for choice in &page.choices {
                if let Some(next) = &choice.next {
                    writeln!(out, "        <choice next=\"{}\">", escape_attr(next)).unwrap();
                } else {
                    out.push_str("        <choice>\n");
                }
                write_line(
                    &mut out,
                    "          ",
                    "say",
                    &choice.line,
                    Some(&self.protagonist),
                );
                write_line(&mut out, "          ", "reply", &choice.reply, None);
                out.push_str("        </choice>\n");
            }
            out.push_str("      </choices>\n    </page>\n");
        }
        out.push_str("  </pages>\n");

        if let Some(pool) = &self.encounters {
            out.push_str("\n  <encounters>\n");
            if !pool.distractors.is_empty() {
                out.push_str("    <distractors>\n");
                for d in &pool.distractors {
                    out.push_str("      <distractor>\n");
                    write_line(
                        &mut out,
                        "        ",
                        "say",
                        &d.line,
                        Some(&self.protagonist),
                    );
                    write_line(&mut out, "        ", "reply", &d.reply, None);
                    out.push_str("      </distractor>\n");
                }
                out.push_str("    </distractors>\n");
            }
            for route in &pool.routes {
                writeln!(out, "    <route id=\"{}\">", escape_attr(&route.id)).unwrap();
                if !route.distractors.is_empty() {
                    out.push_str("      <distractors>\n");
                    for d in &route.distractors {
                        out.push_str("        <distractor>\n");
                        write_line(
                            &mut out,
                            "          ",
                            "say",
                            &d.line,
                            Some(&self.protagonist),
                        );
                        write_line(&mut out, "          ", "reply", &d.reply, None);
                        out.push_str("        </distractor>\n");
                    }
                    out.push_str("      </distractors>\n");
                }
                for stop in &route.stops {
                    writeln!(
                        out,
                        "      <stop after=\"{}\" before=\"{}\" scene=\"{}\">",
                        escape_attr(&stop.after),
                        escape_attr(&stop.before),
                        stop.scene.as_str()
                    )
                    .unwrap();
                    write_line(
                        &mut out,
                        "        ",
                        "title",
                        &stop.title,
                        Some(&self.narrator),
                    );
                    write_line(&mut out, "        ", "question", &stop.question, None);
                    for enc in &stop.encounters {
                        writeln!(out, "        <encounter id=\"{}\">", escape_attr(&enc.id))
                            .unwrap();
                        write_line(&mut out, "          ", "setup", &enc.setup, None);
                        write_line(
                            &mut out,
                            "          ",
                            "say",
                            &enc.say,
                            Some(&self.protagonist),
                        );
                        write_line(&mut out, "          ", "reply", &enc.reply, None);
                        out.push_str("        </encounter>\n");
                    }
                    out.push_str("      </stop>\n");
                }
                out.push_str("    </route>\n");
            }
            out.push_str("  </encounters>\n");
        }

        out.push_str("</story>\n");
        out
    }
}

fn write_line(out: &mut String, indent: &str, tag: &str, line: &Line, omit_actor_if: Option<&str>) {
    if omit_actor_if == Some(line.actor_id.as_str()) {
        writeln!(out, "{indent}<{tag}>").unwrap();
    } else {
        writeln!(
            out,
            "{indent}<{tag} actor=\"{}\">",
            escape_attr(&line.actor_id)
        )
        .unwrap();
    }
    writeln!(out, "{indent}  <sv>{}</sv>", escape_text(&line.swedish)).unwrap();
    if let Some(lit) = &line.literal {
        writeln!(out, "{indent}  <lit>{}</lit>", escape_text(lit)).unwrap();
    }
    if !line.english.is_empty() {
        writeln!(out, "{indent}  <en>{}</en>", escape_text(&line.english)).unwrap();
    }
    writeln!(out, "{indent}</{tag}>").unwrap();
}

pub fn story_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("stories")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        dirs.push(exe_dir.join("stories"));
        if let Some(parent) = exe_dir.parent() {
            dirs.push(parent.join("stories"));
            dirs.push(parent.join("share").join("learn-swedish").join("stories"));
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(xdg).join("learn-swedish").join("stories"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("learn-swedish")
                .join("stories"),
        );
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("learn-swedish").join("stories"));
    }
    dirs.push(PathBuf::from("/usr/share/learn-swedish/stories"));
    dirs
}

pub fn available_story_files() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for dir in story_search_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut xml_files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"))
            })
            .collect();
        xml_files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for path in xml_files {
            let key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen.insert(key) {
                found.push(path);
            }
        }
    }
    found
}

pub fn default_story_path() -> Result<PathBuf, String> {
    if let Some(override_path) = std::env::var_os("LEARN_SWEDISH_STORY") {
        let path = PathBuf::from(override_path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "LEARN_SWEDISH_STORY does not point to an existing file: {}",
            path.display()
        ));
    }
    available_story_files().into_iter().next().ok_or_else(|| {
        "No story XML files found in stories/. Use Open story… to select an XML file.".into()
    })
}

pub struct Book {
    story: Rc<Story>,
    pub pages: Vec<Page>,
    current: usize,
    pub selected: Option<usize>,
    pub finished: bool,
}

pub fn normalise(input: &str) -> String {
    input
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_lowercase()
}

impl Book {
    pub fn load_default(choose: impl FnMut(usize) -> usize) -> Result<(PathBuf, Self), String> {
        let (path, story) = Story::load_default()?;
        Ok((path, Self::from_story(Rc::new(story), choose)))
    }

    pub fn load_file(path: &Path, choose: impl FnMut(usize) -> usize) -> Result<Self, String> {
        let story = Story::from_xml_file(path)?;
        Ok(Self::from_story(Rc::new(story), choose))
    }

    pub fn from_xml(input: &str, choose: impl FnMut(usize) -> usize) -> Result<Self, String> {
        let story = Story::from_xml(input)?;
        Ok(Self::from_story(Rc::new(story), choose))
    }

    pub fn from_story(story: Rc<Story>, mut choose: impl FnMut(usize) -> usize) -> Self {
        Self::build(story, &[], &mut choose)
    }

    fn build(story: Rc<Story>, previous: &[Page], choose: &mut impl FnMut(usize) -> usize) -> Self {
        let mut pages = story.pages.clone();
        for (index, page) in pages.iter_mut().enumerate() {
            let count = page.choices.len();
            match story.choice_rotation {
                ChoiceRotation::None => {}
                ChoiceRotation::ByIndex => page.choices.rotate_left(index % count),
                ChoiceRotation::Random => page.choices.rotate_left(choose(count) % count),
            }
        }
        if let Some(pool) = &story.encounters {
            insert_encounters(&mut pages, pool, previous, choose);
        }
        let current = pages
            .iter()
            .position(|page| page.id == story.start)
            .expect("Validated start page exists");
        Self {
            story,
            pages,
            current,
            selected: None,
            finished: false,
        }
    }

    pub fn story(&self) -> &Story {
        &self.story
    }

    pub fn has_outing_hub(&self) -> bool {
        self.story.outing_hub.is_some()
    }

    pub fn restart(&mut self, mut choose: impl FnMut(usize) -> usize) {
        *self = Self::build(self.story.clone(), &self.pages, &mut choose);
    }

    pub fn new_outing(&mut self, choose: impl FnMut(usize) -> usize) {
        self.restart(choose);
        if let Some(hub) = self.story.outing_hub.clone() {
            self.follow(&hub);
        }
    }

    pub fn page(&self) -> &Page {
        &self.pages[self.current]
    }

    pub fn answer(&mut self, input: &str) -> Result<Choice, String> {
        if self.finished || self.selected.is_some() {
            return Err("Continue or try again before answering another question.".into());
        }
        let max_choice = self.page().choices.len();
        let normalized = normalise(input);
        let choice = normalized
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=max_choice).contains(n))
            .map(|n| n - 1)
            .or_else(|| {
                self.page()
                    .choices
                    .iter()
                    .position(|choice| normalise(&choice.line.swedish) == normalized)
            })
            .ok_or_else(|| {
                format!(
                    "Enter 1–{max_choice}, click an option, or type one of the complete Swedish responses."
                )
            })?;
        self.selected = Some(choice);
        Ok(self.page().choices[choice].clone())
    }

    pub fn continue_story(&mut self) {
        let next = self
            .selected
            .take()
            .and_then(|selected| self.page().choices[selected].next.clone());
        if let Some(next) = next {
            self.follow(&next);
        }
    }

    pub fn skip(&mut self) -> bool {
        if self.finished {
            return false;
        }
        let selected_next = self
            .selected
            .take()
            .and_then(|selected| self.page().choices[selected].next.clone());
        if let Some(next) = selected_next {
            self.follow(&next);
            return true;
        }
        let fallback_next = self
            .page()
            .choices
            .iter()
            .find_map(|choice| choice.next.clone());
        if let Some(next) = fallback_next {
            self.follow(&next);
            return true;
        }
        false
    }

    fn follow(&mut self, next: &str) {
        if next == "done" {
            self.finished = true;
        } else {
            self.current = self
                .pages
                .iter()
                .position(|page| page.id == next)
                .expect("Story links are validated when loading XML");
        }
    }
}

fn insert_encounters(
    pages: &mut Vec<Page>,
    pool: &EncounterPool,
    previous: &[Page],
    choose: &mut impl FnMut(usize) -> usize,
) {
    for route in &pool.routes {
        let mut local = route.distractors.clone();
        let mut general = pool.distractors.clone();
        for stop in &route.stops {
            let candidates: Vec<_> = stop
                .encounters
                .iter()
                .filter(|enc| !previous.iter().any(|page| page.id == enc.id))
                .collect();
            let available: Vec<_> = if candidates.is_empty() {
                stop.encounters.iter().collect()
            } else {
                candidates
            };
            let chosen = available[choose(available.len()) % available.len()];
            let mut choices = vec![Choice {
                line: chosen.say.clone(),
                reply: chosen.reply.clone(),
                next: Some(stop.before.clone()),
            }];
            if !local.is_empty() {
                let idx = choose(local.len()) % local.len();
                choices.push(local.swap_remove(idx));
            }
            while choices.len() < 4 {
                if !general.is_empty() {
                    let idx = choose(general.len()) % general.len();
                    choices.push(general.swap_remove(idx));
                } else {
                    let idx = choose(local.len()) % local.len();
                    choices.push(local.swap_remove(idx));
                }
            }
            let count = choices.len();
            choices.rotate_left(choose(count) % count);
            let link = pages
                .iter_mut()
                .find(|page| page.id == stop.after)
                .and_then(|page| {
                    page.choices
                        .iter_mut()
                        .find(|choice| choice.next.as_deref() == Some(stop.before.as_str()))
                })
                .expect("Validated encounter stop must match an existing link");
            link.next = Some(chosen.id.clone());
            pages.push(Page {
                id: chosen.id.clone(),
                scene: stop.scene,
                title: stop.title.clone(),
                setup: vec![chosen.setup.clone()],
                question: stop.question.clone(),
                choices,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGINE_FIXTURE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<story version="1.0" id="fixture-outing" start="intro" outing-hub="hub">
  <meta>
    <title>Testberättelsen</title>
    <subtitle>Generic engine fixture</subtitle>
  </meta>
  <cast protagonist="you" responder="companion" narrator="narrator">
    <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
    <actor id="you" label="Du" role="lead" voices="mattias"/>
    <actor id="companion" label="Sällskapet" role="lead" voices="sofie"/>
    <actor id="clerk" label="Expediten" role="guest" voices="hillevi,sofie"/>
  </cast>
  <ending title="Tillbaka hemma · Slut">
    <line actor="companion">
      <sv>Tack för i dag!</sv>
      <lit>Thanks for in day!</lit>
      <en>Thanks for today!</en>
    </line>
  </ending>
  <pages choice-rotation="by-index">
    <page id="intro" scene="home">
      <title><sv>Morgon</sv><lit>Morning</lit><en>Morning</en></title>
      <setup>
        <line actor="narrator">
          <sv>Solen skiner över köksbordet.</sv>
          <lit>Sun-the shines over kitchen-table-the.</lit>
          <en>The sun shines across the kitchen table.</en>
        </line>
      </setup>
      <question>
        <sv>Är du redo att gå ut?</sv>
        <lit>Are you ready to go out?</lit>
        <en>Are you ready to head out?</en>
      </question>
      <choices>
        <choice next="hub">
          <say><sv>Ja, nu går vi.</sv><lit>Yes, now go we.</lit><en>Yes, let's go.</en></say>
          <reply><sv>Perfekt!</sv><lit>Perfect!</lit><en>Perfect!</en></reply>
        </choice>
        <choice>
          <say><sv>Jag tar på mig skidorna i köket.</sv><lit>I put on me skis-the in kitchen-the.</lit><en>I'll put my skis on in the kitchen.</en></say>
          <reply><sv>Golvet tackar nej.</sv><lit>Floor-the thanks no.</lit><en>The floor says no thank you.</en></reply>
        </choice>
        <choice>
          <say><sv>Vi ringer efter en helikopter.</sv><lit>We phone after a helicopter.</lit><en>Let's call for a helicopter.</en></say>
          <reply><sv>Balkongen är lite trång.</sv><lit>Balcony-the is little narrow.</lit><en>The balcony is a bit cramped.</en></reply>
        </choice>
        <choice>
          <say><sv>Jag flyttar ut i kylskåpet.</sv><lit>I move out into fridge-the.</lit><en>I'm moving into the fridge.</en></say>
          <reply><sv>Där bor redan mjölken.</sv><lit>There lives already milk-the.</lit><en>The milk already lives there.</en></reply>
        </choice>
      </choices>
    </page>
    <page id="hub" scene="home">
      <title><sv>Val av väg</sv><lit>Choice of way</lit><en>Choosing a direction</en></title>
      <question>
        <sv>Vart vill du gå?</sv>
        <lit>Where-to want you go?</lit>
        <en>Where would you like to go?</en>
      </question>
      <choices>
        <choice next="park-entry">
          <say><sv>Vi går till parken.</sv><lit>We go to park-the.</lit><en>Let's go to the park.</en></say>
          <reply><sv>Skönt med frisk luft.</sv><lit>Nice with fresh air.</lit><en>Fresh air sounds nice.</en></reply>
        </choice>
        <choice next="bakery-door">
          <say><sv>Vi går till bageriet.</sv><lit>We go to bakery-the.</lit><en>Let's go to the bakery.</en></say>
          <reply><sv>Gott med nybakat bröd.</sv><lit>Good with newly-baked bread.</lit><en>Fresh bread sounds great.</en></reply>
        </choice>
      </choices>
    </page>
    <page id="park-entry" scene="woods">
      <title><sv>I parken</sv><lit>In park-the</lit><en>In the park</en></title>
      <question>
        <sv>Ska vi ta bänken vid vattnet?</sv>
        <lit>Shall we take bench-the by water-the?</lit>
        <en>Shall we take the bench by the water?</en>
      </question>
      <choices>
        <choice next="home-finish">
          <say><sv>Ja, där är det lugnt.</sv><lit>Yes, there is it calm.</lit><en>Yes, it's peaceful there.</en></say>
          <reply><sv>Vi sätter oss en stund.</sv><lit>We seat ourselves a while.</lit><en>Let's sit down for a while.</en></reply>
        </choice>
        <choice>
          <say><sv>Vi simmar hem med kläderna på.</sv><lit>We swim home with clothes-the on.</lit><en>Let's swim home with our clothes on.</en></say>
          <reply><sv>Vattnet är fyra grader.</sv><lit>Water-the is four degrees.</lit><en>The water is four degrees.</en></reply>
        </choice>
      </choices>
    </page>
    <page id="bakery-door" scene="shop">
      <title><sv>På bageriet</sv><lit>At bakery-the</lit><en>At the bakery</en></title>
      <question actor="clerk">
        <sv>Vad får det lov att vara?</sv>
        <lit>What may it permission to be?</lit>
        <en>What can I get for you?</en>
      </question>
      <choices>
        <choice next="home-finish">
          <say><sv>Ett surdegsbröd, tack.</sv><lit>One sourdough-bread, please.</lit><en>One sourdough loaf, please.</en></say>
          <reply actor="clerk"><sv>Varsågod!</sv><lit>There-you-go!</lit><en>Here you go!</en></reply>
        </choice>
        <choice>
          <say><sv>Tio vinterdäck, tack.</sv><lit>Ten winter-tyres, please.</lit><en>Ten winter tyres, please.</en></say>
          <reply actor="clerk"><sv>De är väldigt svårätna.</sv><lit>They are very hard-eaten.</lit><en>Those are very chewy.</en></reply>
        </choice>
      </choices>
    </page>
    <page id="home-finish" scene="home">
      <title><sv>Kväll</sv><lit>Evening</lit><en>Evening</en></title>
      <question>
        <sv>Vill du ha te nu?</sv>
        <lit>Want you have tea now?</lit>
        <en>Would you like some tea now?</en>
      </question>
      <choices>
        <choice next="done">
          <say><sv>Ja tack, gärna.</sv><lit>Yes thanks, gladly.</lit><en>Yes please, I'd love some.</en></say>
          <reply><sv>Jag sätter på vattenkokaren.</sv><lit>I set on kettle-the.</lit><en>I'll put the kettle on.</en></reply>
        </choice>
        <choice>
          <say><sv>Nej, jag dricker bara regnvatten.</sv><lit>No, I drink only rainwater.</lit><en>No, I only drink rainwater.</en></say>
          <reply><sv>Vi håller oss till tekoppen.</sv><lit>We keep ourselves to teacup-the.</lit><en>Let's stick to the teacup.</en></reply>
        </choice>
      </choices>
    </page>
  </pages>
  <encounters>
    <distractors>
      <distractor>
        <say><sv>Jag beställer en ubåt.</sv><lit>I order a submarine.</lit><en>I'd like to order a submarine.</en></say>
        <reply><sv>Fel disk.</sv><lit>Wrong counter.</lit><en>Wrong counter.</en></reply>
      </distractor>
      <distractor>
        <say><sv>Kan vi låna en giraff?</sv><lit>Can we borrow a giraffe?</lit><en>Can we borrow a giraffe?</en></say>
        <reply><sv>Taket är för lågt.</sv><lit>Ceiling-the is too low.</lit><en>The ceiling is too low.</en></reply>
      </distractor>
    </distractors>
    <route id="bakery">
      <distractors>
        <distractor>
          <say><sv>Har ni bröd av betong?</sv><lit>Have you bread of concrete?</lit><en>Do you have concrete bread?</en></say>
          <reply><sv>Inte på lördagar.</sv><lit>Not on Saturdays.</lit><en>Not on Saturdays.</en></reply>
        </distractor>
      </distractors>
      <stop after="bakery-door" before="home-finish" scene="shop">
        <title><sv>Vid kassan</sv><lit>By checkout-the</lit><en>At the till</en></title>
        <question><sv>Vad säger du?</sv><lit>What say you?</lit><en>What do you say?</en></question>
        <encounter id="enc-card">
          <setup actor="narrator"><sv>Kortläsaren piper till.</sv><lit>Card-reader-the beeps to.</lit><en>The card reader beeps.</en></setup>
          <say><sv>Jag blippar kortet.</sv><lit>I tap card-the.</lit><en>I'll tap my card.</en></say>
          <reply><sv>Smidigt!</sv><lit>Smooth!</lit><en>Easy!</en></reply>
        </encounter>
        <encounter id="enc-bag">
          <setup actor="narrator"><sv>Brödet är fortfarande varmt.</sv><lit>Bread-the is still warm.</lit><en>The loaf is still warm.</en></setup>
          <say><sv>Vi tar en papperspåse också.</sv><lit>We take a paper-bag also.</lit><en>Let's take a paper bag too.</en></say>
          <reply><sv>Bra tänkt.</sv><lit>Good thought.</lit><en>Good thinking.</en></reply>
        </encounter>
      </stop>
    </route>
  </encounters>
</story>"#;

    fn verify_story_graph_and_round_trip(story: &Story) {
        let exported = story.to_xml();
        let reparsed = Story::from_xml(&exported).unwrap();
        assert_eq!(&reparsed, story);

        let mut phrases = vec![story.ending.line.phrase()];
        for page in &story.pages {
            phrases.extend(page.phrases());
            phrases.extend(page.choices.iter().map(|c| c.reply.phrase()));
        }
        if let Some(pool) = &story.encounters {
            for d in pool
                .distractors
                .iter()
                .chain(pool.routes.iter().flat_map(|r| &r.distractors))
            {
                phrases.push(d.line.phrase());
                phrases.push(d.reply.phrase());
            }
            for stop in pool.routes.iter().flat_map(|r| &r.stops) {
                phrases.push(stop.title.phrase());
                phrases.push(stop.question.phrase());
                for enc in &stop.encounters {
                    phrases.push(enc.setup.phrase());
                    phrases.push(enc.say.phrase());
                    phrases.push(enc.reply.phrase());
                }
            }
        }
        for phrase in &phrases {
            assert!(phrase.voice.is_some() && phrase.speaker.is_some());
            assert!(!phrase.swedish.is_empty());
        }

        fn walk<'a>(
            id: &'a str,
            pages: &HashMap<&'a str, &'a Page>,
            path: &mut Vec<&'a str>,
            seen: &mut HashSet<&'a str>,
        ) {
            if id == "done" {
                return;
            }
            assert!(!path.contains(&id), "Story cycle: {path:?} -> {id}");
            let page = pages.get(id).expect("Broken story link");
            seen.insert(page.id.as_str());
            path.push(page.id.as_str());
            assert!(page.choices.iter().any(|c| c.next.is_some()));
            let answers: HashSet<_> = page
                .choices
                .iter()
                .map(|c| normalise(&c.line.swedish))
                .collect();
            assert_eq!(answers.len(), page.choices.len());
            for choice in &page.choices {
                assert!(!choice.line.english.is_empty() && !choice.reply.swedish.is_empty());
                if let Some(next) = &choice.next {
                    walk(next, pages, path, seen);
                }
            }
            path.pop();
        }

        let rc = Rc::new(story.clone());
        let max_pool = story
            .encounters
            .as_ref()
            .and_then(|pool| {
                pool.routes
                    .iter()
                    .flat_map(|r| &r.stops)
                    .map(|s| s.encounters.len())
                    .max()
            })
            .unwrap_or(1)
            .max(1);
        let mut all_seen = HashSet::new();
        for index in 0..max_pool {
            let book = Book::from_story(rc.clone(), |upper| index % upper);
            let by_id: HashMap<_, _> = book
                .pages
                .iter()
                .map(|page| (page.id.as_str(), page))
                .collect();
            assert_eq!(by_id.len(), book.pages.len());
            let mut seen = HashSet::new();
            walk(&story.start, &by_id, &mut Vec::new(), &mut seen);
            assert_eq!(seen.len(), book.pages.len());
            all_seen.extend(seen.into_iter().map(str::to_owned));
        }
        let expected_ids: HashSet<String> = story
            .pages
            .iter()
            .map(|p| p.id.clone())
            .chain(
                story
                    .encounters
                    .iter()
                    .flat_map(|pool| &pool.routes)
                    .flat_map(|r| &r.stops)
                    .flat_map(|s| &s.encounters)
                    .map(|e| e.id.clone()),
            )
            .collect();
        assert_eq!(all_seen, expected_ids);
    }

    #[test]
    fn external_story_files_in_stories_directory_parse_validate_and_round_trip() {
        let files = available_story_files();
        assert!(
            !files.is_empty(),
            "Expected at least one .xml story file in stories/"
        );
        for path in files {
            let story = Story::from_xml_file(&path).unwrap();
            verify_story_graph_and_round_trip(&story);
        }
    }

    #[test]
    fn typed_and_numbered_answers_retry_without_moving_and_then_follow_the_selected_branch() {
        let mut book = Book::from_xml(ENGINE_FIXTURE_XML, |_| 0).unwrap();
        let wrong = book
            .page()
            .choices
            .iter()
            .position(|c| c.next.is_none())
            .unwrap();
        assert!(book.answer("0").is_err());
        assert!(book.answer("5").is_err());
        assert!(book.answer("a partial answer").is_err());
        assert!(
            book.answer(&(wrong + 1).to_string())
                .unwrap()
                .next
                .is_none()
        );
        assert!(book.answer("1").is_err());
        book.continue_story();
        assert_eq!(book.page().id, "intro");
        let right = book
            .page()
            .choices
            .iter()
            .find(|c| c.next.is_some())
            .unwrap()
            .line
            .swedish
            .to_uppercase();
        assert_eq!(book.answer(&right).unwrap().next.as_deref(), Some("hub"));
        book.continue_story();
        assert_eq!(book.page().id, "hub");
    }

    #[test]
    fn encounters_hold_steady_on_retry_and_change_for_the_next_outing() {
        let mut book = Book::from_xml(ENGINE_FIXTURE_XML, |_| 0).unwrap();
        let take_bakery = |book: &mut Book| {
            let choice = book
                .page()
                .choices
                .iter()
                .find(|choice| choice.next.as_deref() == Some("bakery-door"))
                .unwrap()
                .line
                .swedish
                .clone();
            book.answer(&choice).unwrap();
            book.continue_story();
            assert!(book.skip());
        };
        assert!(book.skip());
        assert_eq!(book.page().id, "hub");
        take_bakery(&mut book);
        let first_encounter = book.page().id.clone();
        let wrong = book
            .page()
            .choices
            .iter()
            .find(|choice| choice.next.is_none())
            .unwrap()
            .line
            .swedish
            .clone();
        book.answer(&wrong).unwrap();
        book.continue_story();
        assert_eq!(book.page().id, first_encounter);
        assert!(book.skip());
        assert_eq!(book.page().id, "home-finish");
        while book.skip() {}
        assert!(book.finished);
        book.new_outing(|_| 0);
        assert_eq!(book.page().id, "hub");
        take_bakery(&mut book);
        assert_ne!(book.page().id, first_encounter);
        book.restart(|_| 0);
        assert_eq!(book.page().id, "intro");
    }

    #[test]
    fn skipping_advances_unanswered_and_wrong_pages_until_the_book_finishes() {
        let mut book = Book::from_xml(ENGINE_FIXTURE_XML, |_| 0).unwrap();
        assert_eq!(book.page().id, "intro");
        let wrong = book
            .page()
            .choices
            .iter()
            .position(|choice| choice.next.is_none())
            .unwrap();
        book.answer(&(wrong + 1).to_string()).unwrap();
        assert!(book.selected.is_some());
        assert!(book.skip());
        assert_eq!(book.page().id, "hub");
        assert!(book.selected.is_none());
        for _ in 0..book.pages.len() {
            if !book.skip() {
                break;
            }
        }
        assert!(book.finished);
        assert!(!book.skip());
    }

    #[test]
    fn custom_xml_stories_load_without_recompiling_and_reject_invalid_graphs() {
        let custom_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<story version="1.0" id="kort-fika">
  <!-- Minimal two-choice story without literal glosses or encounter pools -->
  <meta>
    <title>En kort fika &amp; kaffe</title>
    <subtitle>Custom XML story</subtitle>
  </meta>
  <cast protagonist="you" responder="barista" narrator="narrator">
    <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
    <actor id="you" label="Du" role="lead" voices="mattias"/>
    <actor id="barista" label="Baristan" role="guest" voices="hillevi,sofie"/>
  </cast>
  <pages>
    <page id="counter" scene="city">
      <title><sv>Framme vid disken</sv></title>
      <setup>
        <line actor="narrator">
          <sv>Det doftar kanelbullar &amp; kaffe.</sv>
          <en>It smells of cinnamon buns &amp; coffee.</en>
        </line>
      </setup>
      <question>
        <sv>Vad vill du beställa?</sv>
        <en>What would you like to order?</en>
      </question>
      <choices>
        <choice next="done">
          <say><sv>En kaffe, tack.</sv><en>One coffee, please.</en></say>
          <reply><sv>Varsågod!</sv><en>Here you go!</en></reply>
        </choice>
        <choice>
          <say><sv>En cykel, tack.</sv><en>One bicycle, please.</en></say>
          <reply><sv>Vi säljer bara fika.</sv><en>We only sell fika.</en></reply>
        </choice>
      </choices>
    </page>
  </pages>
</story>"#;
        let story = Story::from_xml(custom_xml).unwrap();
        assert_eq!(story.title, "En kort fika & kaffe");
        assert!(story.outing_hub.is_none());
        assert_eq!(
            Story::from_xml(&story.to_xml()).unwrap().title,
            "En kort fika & kaffe"
        );

        let spec_doc = std::fs::read_to_string(Path::new("docs/story-xml-spec.md")).unwrap();
        let spec_example = spec_doc
            .split("## 11. Complete Minimal Example")
            .nth(1)
            .and_then(|section| section.split("```xml").nth(1))
            .and_then(|code| code.split("```").next())
            .expect("story-xml-spec.md must contain minimal XML example");
        let parsed_example = Story::from_xml(spec_example.trim()).unwrap();
        assert_eq!(parsed_example.title, "En kort fika");
        let mut book = Book::from_story(Rc::new(story), |_| 0);
        assert_eq!(book.page().choices.len(), 2);
        assert!(book.answer("3").unwrap_err().contains("Enter 1–2"));
        assert!(book.answer("2").unwrap().next.is_none());
        book.continue_story();
        assert!(!book.finished);
        assert_eq!(
            book.answer("en kaffe, tack").unwrap().next.as_deref(),
            Some("done")
        );
        book.continue_story();
        assert!(book.finished);
        book.new_outing(|_| 0);
        assert!(!book.finished);
        assert_eq!(book.page().id, "counter");

        let cycle_xml = custom_xml.replace("next=\"done\"", "next=\"counter\"");
        assert!(Story::from_xml(&cycle_xml).unwrap_err().contains("cycle"));

        let broken_xml = custom_xml.replace("next=\"done\"", "next=\"missing-page\"");
        assert!(
            Story::from_xml(&broken_xml)
                .unwrap_err()
                .contains("unknown next page")
        );

        let bad_actor_xml = custom_xml.replace("actor=\"narrator\"", "actor=\"ghost\"");
        assert!(
            Story::from_xml(&bad_actor_xml)
                .unwrap_err()
                .contains("unknown actor")
        );
    }
}
