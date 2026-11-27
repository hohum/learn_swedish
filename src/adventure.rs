use crate::lesson::{Phrase, Voice};

mod encounters;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Narrator,
    Husband,
    Wife,
    ShopAssistant,
    Police,
    DogWalker,
    Hiker,
    Conductor,
    StationStaff,
    BusDriver,
    CafeStaff,
    Passenger,
    Passerby,
}

impl Actor {
    pub fn name(self) -> &'static str {
        match self {
            Self::Narrator => "Berättaren",
            Self::Husband => "Du",
            Self::Wife => "Din fru",
            Self::ShopAssistant => "Butiksmedarbetaren",
            Self::Police => "Polisen",
            Self::DogWalker => "Hundägaren",
            Self::Hiker => "Vandraren",
            Self::Conductor => "Konduktören",
            Self::StationStaff => "Stationsvärden",
            Self::BusDriver => "Busschauffören",
            Self::CafeStaff => "Kafébiträdet",
            Self::Passenger => "Medresenären",
            Self::Passerby => "Förbipasserande",
        }
    }

    pub fn voice(self) -> Voice {
        match self {
            Self::Wife
            | Self::ShopAssistant
            | Self::Police
            | Self::BusDriver
            | Self::CafeStaff
            | Self::StationStaff => Voice::Sofie,
            Self::Narrator
            | Self::Husband
            | Self::DogWalker
            | Self::Hiker
            | Self::Conductor
            | Self::Passenger
            | Self::Passerby => Voice::Mattias,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub actor: Actor,
    pub swedish: &'static str,
    pub literal: &'static str,
    pub english: &'static str,
}

impl Line {
    pub fn phrase(self) -> Phrase {
        Phrase {
            swedish: self.swedish.into(),
            literal: Some(self.literal),
            english: self.english.into(),
            voice: Some(self.actor.voice()),
            speaker: Some(self.actor.name()),
        }
    }
}

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

#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub line: Line,
    pub reply: Line,
    pub next: Option<&'static str>,
}

pub struct Page {
    pub id: &'static str,
    pub scene: Scene,
    pub title: Line,
    pub setup: Vec<Line>,
    pub question: Line,
    pub choices: [Choice; 4],
}

impl Page {
    pub fn phrases(&self) -> Vec<Phrase> {
        std::iter::once(self.title.phrase())
            .chain(self.setup.iter().map(|line| line.phrase()))
            .chain(std::iter::once(self.question.phrase()))
            .chain(self.choices.iter().map(|choice| choice.line.phrase()))
            .collect()
    }
}

const fn line(actor: Actor, [swedish, literal, english]: [&'static str; 3]) -> Line {
    Line {
        actor,
        swedish,
        literal,
        english,
    }
}

const fn n(text: [&'static str; 3]) -> Line {
    line(Actor::Narrator, text)
}

const fn w(text: [&'static str; 3]) -> Line {
    line(Actor::Wife, text)
}

const FAREWELL_TEXT: [&str; 3] = [
    "Tack själv. Nästa gång får du välja ett nytt äventyr.",
    "Thanks yourself. Next time get you choose a new adventure.",
    "Thank you too. Next time you can choose a new adventure.",
];
pub const FAREWELL: Line = w(FAREWELL_TEXT);

const fn go(answer: [&'static str; 3], next: &'static str, reply: [&'static str; 3]) -> Choice {
    Choice {
        line: line(Actor::Husband, answer),
        reply: w(reply),
        next: Some(next),
    }
}

const fn no(answer: [&'static str; 3], reply: [&'static str; 3]) -> Choice {
    Choice {
        line: line(Actor::Husband, answer),
        reply: w(reply),
        next: None,
    }
}

fn p(
    id: &'static str,
    scene: Scene,
    title: [&'static str; 3],
    setup: Vec<Line>,
    question: Line,
    choices: [Choice; 4],
) -> Page {
    Page {
        id,
        scene,
        title: n(title),
        setup,
        question,
        choices,
    }
}

pub fn pages() -> Vec<Page> {
    use Actor::*;
    use Scene::*;
    let mut pages = vec![
        p(
            "home-weather",
            Home,
            [
                "En lagom äventyrlig lördag",
                "A just-right adventurous Saturday",
                "A moderately adventurous Saturday",
            ],
            vec![
                n([
                    "Det är lördag. Du och din fru står i köket och planerar dagen.",
                    "It is Saturday. You and your wife stand in kitchen-the and plan day-the.",
                    "It is Saturday. You and your wife are in the kitchen planning the day.",
                ]),
                w([
                    "Du väljer utflykt i dag. Jag hjälper dig med svenskan, men du får prata.",
                    "You choose outing today. I help you with Swedish-the, but you get speak.",
                    "You choose today's outing. I'll help with Swedish, but you do the talking.",
                ]),
                w([
                    "Det är åtta grader och det ska regna i eftermiddag.",
                    "It is eight degrees and it shall rain this afternoon.",
                    "It is eight degrees and it is going to rain this afternoon.",
                ]),
            ],
            w([
                "Vad behöver du ta på dig?",
                "What need you put on yourself?",
                "What do you need to wear?",
            ]),
            [
                go(
                    [
                        "Jag tar en tröja och en regnjacka.",
                        "I take a jumper and a rain-jacket.",
                        "I'll wear a jumper and a rain jacket.",
                    ],
                    "home-keys",
                    [
                        "Bra. Svenskt väder uppskattar en reservplan.",
                        "Good. Swedish weather appreciates a backup-plan.",
                        "Good. Swedish weather appreciates a backup plan.",
                    ],
                ),
                no(
                    [
                        "Jag tar en tunn skjorta. Det är väl varmt?",
                        "I take a thin shirt. It is surely warm?",
                        "I'll wear a thin shirt. It's warm, isn't it?",
                    ],
                    [
                        "Åtta grader. Värmen verkar ha tagit ledigt.",
                        "Eight degrees. Warmth-the seems to have taken time off.",
                        "Eight degrees. The warmth seems to have taken the day off.",
                    ],
                ),
                no(
                    [
                        "Jag behöver ingen jacka. Jag går fort.",
                        "I need no jacket. I go fast.",
                        "I don't need a jacket. I'll walk fast.",
                    ],
                    [
                        "Regnet går också ganska fort.",
                        "Rain-the goes also fairly fast.",
                        "The rain moves fairly quickly too.",
                    ],
                ),
                no(
                    [
                        "Jag tar på mig ett paraply.",
                        "I put on myself an umbrella.",
                        "I'll put on an umbrella.",
                    ],
                    [
                        "Du kan ta med ett paraply. Att ta på sig det blir ett annat äventyr.",
                        "You can take along an umbrella. To put on oneself it becomes another adventure.",
                        "You can bring an umbrella. Wearing it would be another adventure.",
                    ],
                ),
            ],
        ),
        p(
            "home-keys",
            Home,
            ["Nycklarna", "Keys-the", "The keys"],
            vec![
                n([
                    "Jackan är på, men nyckelskålen är tom.",
                    "Jacket-the is on, but key-bowl-the is empty.",
                    "Your jacket is on, but the key bowl is empty.",
                ]),
                w([
                    "Känn efter i fickan på jackan du hade i går.",
                    "Feel around in pocket-the on jacket-the you had yesterday.",
                    "Check the pocket of the jacket you wore yesterday.",
                ]),
            ],
            w([
                "Var är nycklarna?",
                "Where are keys-the?",
                "Where are the keys?",
            ]),
            [
                go(
                    [
                        "Här är de! De ligger i jackfickan.",
                        "Here are they! They lie in jacket-pocket-the.",
                        "Here they are! They're in the jacket pocket.",
                    ],
                    "home-plan",
                    [
                        "Precis. Nycklar brukar trivas där man lämnade dem.",
                        "Exactly. Keys tend to thrive where one left them.",
                        "Exactly. Keys tend to like being where you left them.",
                    ],
                ),
                no(
                    [
                        "De ligger säkert i skålen. Vi går nu.",
                        "They lie surely in bowl-the. We go now.",
                        "They're surely in the bowl. Let's go now.",
                    ],
                    [
                        "Skålen är tom. Den har redan blivit förhörd.",
                        "Bowl-the is empty. It has already been interrogated.",
                        "The bowl is empty. It's already been questioned.",
                    ],
                ),
                no(
                    [
                        "Vi behöver inga nycklar när vi går ut.",
                        "We need no keys when we go out.",
                        "We don't need keys when we go out.",
                    ],
                    [
                        "Det stämmer. Det är när vi ska in igen som det blir intressant.",
                        "That is correct. It is when we shall in again that it becomes interesting.",
                        "True. It gets interesting when we want to come back in.",
                    ],
                ),
                no(
                    [
                        "Jag letar i kylskåpet först.",
                        "I look in refrigerator-the first.",
                        "I'll look in the fridge first.",
                    ],
                    [
                        "Gör det. Fråga mjölken om den har sett något.",
                        "Do that. Ask milk-the if it has seen anything.",
                        "Go ahead. Ask the milk whether it has seen anything.",
                    ],
                ),
            ],
        ),
        p(
            "home-plan",
            Home,
            [
                "Vart ska vi?",
                "To where shall we [go]?",
                "Where are we going?",
            ],
            vec![
                n([
                    "Nycklarna är hittade. Telefonen är laddad och ytterdörren väntar.",
                    "Keys-the are found. Phone-the is charged and outer-door-the waits.",
                    "The keys are found. Your phone is charged and the front door awaits.",
                ]),
                w([
                    "Vi har gott om tid. Välj det du är sugen på.",
                    "We have plenty of time. Choose that which you are keen on.",
                    "We have plenty of time. Choose what you fancy.",
                ]),
            ],
            w([
                "Vad vill du göra i dag?",
                "What want you do today?",
                "What would you like to do today?",
            ]),
            [
                go(
                    [
                        "Jag vill ta en promenad i skogen.",
                        "I want take a walk in forest-the.",
                        "I'd like to go for a walk in the woods.",
                    ],
                    "woods-gate",
                    [
                        "Då tar vi med vatten och något att äta.",
                        "Then take we along water and something to eat.",
                        "Then let's bring water and something to eat.",
                    ],
                ),
                go(
                    [
                        "Vi går till Willys och handlar.",
                        "We go to Willys and shop.",
                        "Let's walk to Willys and do some shopping.",
                    ],
                    "shop-list",
                    [
                        "Bra idé. Tygkassen ligger vid dörren.",
                        "Good idea. Cloth-bag-the lies by door-the.",
                        "Good idea. The reusable bag is by the door.",
                    ],
                ),
                go(
                    [
                        "Vi kör till IKEA och tittar på en hylla.",
                        "We drive to IKEA and look at a shelf.",
                        "Let's drive to IKEA and look at a shelf.",
                    ],
                    "road-calm",
                    [
                        "Då tar vi körkortet, måttbandet och lite tålamod.",
                        "Then take we driving-licence-the, measuring-tape-the and a little patience.",
                        "Then let's bring your driving licence, the tape measure and a little patience.",
                    ],
                ),
                go(
                    [
                        "Jag vill ta tåget till Göteborg.",
                        "I want take train-the to Gothenburg.",
                        "I'd like to take the train to Gothenburg.",
                    ],
                    "train-ticket",
                    [
                        "Då kollar vi biljetter och väder innan vi går.",
                        "Then check we tickets and weather before we go.",
                        "Then let's check tickets and the weather before we leave.",
                    ],
                ),
            ],
        ),
        p(
            "woods-gate",
            Woods,
            [
                "Vid skogsbrynet",
                "At forest-edge-the",
                "At the edge of the woods",
            ],
            vec![
                n([
                    "Ni går hemifrån med vatten och smörgåsar. Vid skogen stannar din fru.",
                    "You [plural] go from home with water and sandwiches. By forest-the stops your wife.",
                    "You leave home with water and sandwiches. At the woods your wife stops.",
                ]),
                w([
                    "Det har regnat i natt. Stigen kan vara lerig.",
                    "It has rained last night. Path-the can be muddy.",
                    "It rained last night. The path might be muddy.",
                ]),
            ],
            w([
                "Vilka skor passar bäst?",
                "Which shoes fit best?",
                "Which shoes are most suitable?",
            ]),
            [
                go(
                    [
                        "Jag tar mina vattentåliga promenadskor.",
                        "I take my water-resistant walking-shoes.",
                        "I'll wear my water-resistant walking shoes.",
                    ],
                    "woods-path",
                    [
                        "Bra. Leran behöver inte följa med hem i strumporna.",
                        "Good. Mud-the needs not come along home in socks-the.",
                        "Good. The mud doesn't need to come home in your socks.",
                    ],
                ),
                no(
                    [
                        "Mina tofflor passar nog bra.",
                        "My slippers fit probably well.",
                        "My slippers should be fine.",
                    ],
                    [
                        "Ja, om skogen har börjat med inneskor.",
                        "Yes, if forest-the has started with indoor-shoes.",
                        "Yes, if the forest has introduced indoor shoes.",
                    ],
                ),
                no(
                    [
                        "Jag tar mina tunna tygskor.",
                        "I take my thin fabric-shoes.",
                        "I'll wear my thin canvas shoes.",
                    ],
                    [
                        "Då får fötterna också uppleva vädret. Vi väljer något vattentåligt.",
                        "Then get feet-the also experience weather-the. We choose something water-resistant.",
                        "Then your feet get to experience the weather too. Let's choose something water-resistant.",
                    ],
                ),
                no(
                    [
                        "Skorna kan vänta hemma.",
                        "Shoes-the can wait at home.",
                        "The shoes can wait at home.",
                    ],
                    [
                        "Omtänksamt. Men det är du som ska gå.",
                        "Thoughtful. But it is you who shall walk.",
                        "Thoughtful. But you're the one doing the walking.",
                    ],
                ),
            ],
        ),
        p(
            "woods-path",
            Woods,
            ["Två stigar", "Two paths", "Two paths"],
            vec![
                n([
                    "Stigen delar sig. En skylt pekar mot sjön. Den andra vägen är avstängd.",
                    "Path-the divides itself. A sign points toward lake-the. The other way-the is closed off.",
                    "The path forks. A sign points towards the lake. The other way is closed.",
                ]),
                w([
                    "Avstängd betyder att vi inte ska gå där.",
                    "Closed off means that we not shall go there.",
                    "Closed means we shouldn't go that way.",
                ]),
            ],
            w([
                "Vilken väg tar vi?",
                "Which way take we?",
                "Which way shall we go?",
            ]),
            [
                go(
                    [
                        "Vi följer skylten mot sjön.",
                        "We follow sign-the toward lake-the.",
                        "Let's follow the sign towards the lake.",
                    ],
                    "woods-dog",
                    [
                        "Ja. Sjön har inte satt upp några protester.",
                        "Yes. Lake-the has not put up any protests.",
                        "Yes. The lake hasn't lodged any objections.",
                    ],
                ),
                no(
                    [
                        "Vi tar den avstängda vägen.",
                        "We take the closed-off way-the.",
                        "Let's take the closed path.",
                    ],
                    [
                        "Skylten verkar ha en annan uppfattning.",
                        "Sign-the seems to have another opinion.",
                        "The sign seems to have a different opinion.",
                    ],
                ),
                no(
                    [
                        "Avstängd betyder väl att vägen är öppen?",
                        "Closed off means surely that way-the is open?",
                        "Closed means the path is open, doesn't it?",
                    ],
                    [
                        "Nästan tvärtom. Skylten är inte särskilt förhandlingsvillig.",
                        "Almost opposite. Sign-the is not particularly negotiation-willing.",
                        "Almost the opposite. The sign isn't particularly willing to negotiate.",
                    ],
                ),
                no(
                    [
                        "Vi väntar tills stigen bestämmer sig.",
                        "We wait until path-the decides itself.",
                        "Let's wait until the path makes up its mind.",
                    ],
                    [
                        "Då kan det bli en ganska lång lördag.",
                        "Then can it become a fairly long Saturday.",
                        "Then Saturday could be quite long.",
                    ],
                ),
            ],
        ),
        p(
            "woods-dog",
            Woods,
            [
                "En ny bekantskap",
                "A new acquaintance",
                "A new acquaintance",
            ],
            vec![
                n([
                    "En man kommer gående med en liten hund i koppel.",
                    "A man comes walking with a small dog in leash.",
                    "A man approaches with a small dog on a lead.",
                ]),
                line(
                    DogWalker,
                    [
                        "God morgon! Vilken fin dag det blev ändå.",
                        "Good morning! What a fine day it became after all.",
                        "Good morning! It turned into a lovely day after all.",
                    ],
                ),
            ],
            w([
                "Hur svarar du vänligt?",
                "How answer you kindly?",
                "How do you reply politely?",
            ]),
            [
                go(
                    [
                        "God morgon! Ja, verkligen. Vilken fin hund!",
                        "Good morning! Yes, really. What a fine dog!",
                        "Good morning! Yes, indeed. What a lovely dog!",
                    ],
                    "woods-berries",
                    [
                        "Precis. Ett hej räcker långt här ute.",
                        "Exactly. A hello reaches far out here.",
                        "Exactly. A hello goes a long way out here.",
                    ],
                ),
                no(
                    [
                        "God natt! Ja, verkligen.",
                        "Good night! Yes, really.",
                        "Good night! Yes, indeed.",
                    ],
                    [
                        "God morgon passar bättre. Vi har ju precis lämnat frukosten.",
                        "Good morning fits better. We have, as you know, just left breakfast-the.",
                        "Good morning fits better. We've only just left breakfast behind.",
                    ],
                ),
                no(
                    [
                        "Jag beställde faktiskt solsken.",
                        "I ordered actually sunshine.",
                        "I actually ordered sunshine.",
                    ],
                    [
                        "Då får du reklamera hos väderprognosen.",
                        "Then get you make a complaint to weather-forecast-the.",
                        "Then you'll have to complain to the weather forecast.",
                    ],
                ),
                no(
                    [
                        "Du står på min stig.",
                        "You stand on my path.",
                        "You're standing on my path.",
                    ],
                    [
                        "Din stig? Då har jag missat ett ganska stort bostadsköp.",
                        "Your path? Then have I missed a fairly big home-purchase.",
                        "Your path? Then I've missed a rather large property purchase.",
                    ],
                ),
            ],
        ),
        p(
            "woods-berries",
            Woods,
            [
                "Bär vid stigen",
                "Berries by path-the",
                "Berries by the path",
            ],
            vec![
                n([
                    "Du ser röda bär på en buske. Ingen av er vet vad det är för sort.",
                    "You see red berries on a bush. None of you knows what it is for kind.",
                    "You spot red berries on a bush. Neither of you knows what kind they are.",
                ]),
                w([
                    "Alla bär går inte att äta.",
                    "All berries go not to eat.",
                    "Not all berries are edible.",
                ]),
            ],
            w([
                "Vad gör vi med bär vi inte känner igen?",
                "What do we with berries we not recognize?",
                "What do we do with berries we don't recognise?",
            ]),
            [
                go(
                    [
                        "Vi låter dem vara.",
                        "We let them be.",
                        "We leave them alone.",
                    ],
                    "woods-toilet",
                    [
                        "Klokt. Färgen är ingen innehållsförteckning.",
                        "Wise. Colour-the is no contents-list.",
                        "Wise. The colour isn't an ingredients list.",
                    ],
                ),
                no(
                    [
                        "Jag smakar först och frågar sedan.",
                        "I taste first and ask then.",
                        "I'll taste first and ask afterwards.",
                    ],
                    [
                        "Vi tar frågorna först. Magen behöver ingen överraskning.",
                        "We take questions-the first. Stomach-the needs no surprise.",
                        "Let's do the questions first. Your stomach doesn't need a surprise.",
                    ],
                ),
                no(
                    [
                        "Rött betyder säkert jordgubbe.",
                        "Red means surely strawberry.",
                        "Red must mean strawberry.",
                    ],
                    [
                        "Det gör det inte ens på godishyllan.",
                        "That does it not even on sweets-shelf-the.",
                        "Not even on the sweet shelf.",
                    ],
                ),
                no(
                    [
                        "Jag litar på att fåglarna har kontrollerat dem.",
                        "I trust in that birds-the have checked them.",
                        "I trust the birds have checked them.",
                    ],
                    [
                        "Fåglarna har tyvärr ingen kundtjänst.",
                        "Birds-the have unfortunately no customer-service.",
                        "Unfortunately the birds don't have a customer service desk.",
                    ],
                ),
            ],
        ),
        p(
            "woods-toilet",
            Woods,
            [
                "En praktisk fråga",
                "A practical question",
                "A practical question",
            ],
            vec![
                n([
                    "Vid rastplatsen finns en liten servicebyggnad. Du behöver gå på toaletten.",
                    "At rest-place-the exists a small service-building. You need go on toilet-the.",
                    "There is a small service building at the picnic area. You need the toilet.",
                ]),
                line(
                    DogWalker,
                    [
                        "Letar ni efter något?",
                        "Look you for something?",
                        "Are you looking for something?",
                    ],
                ),
            ],
            w([
                "Hur frågar du efter toaletten?",
                "How ask you for toilet-the?",
                "How do you ask where the toilet is?",
            ]),
            [
                go(
                    [
                        "Ursäkta, var finns toaletten?",
                        "Excuse [me], where exists toilet-the?",
                        "Excuse me, where is the toilet?",
                    ],
                    "woods-rain",
                    [
                        "Bra. Den ligger bakom byggnaden, säger han.",
                        "Good. It lies behind building-the, says he.",
                        "Good. It's behind the building, he says.",
                    ],
                ),
                no(
                    [
                        "Ursäkta, när öppnar toaletten?",
                        "Excuse [me], when opens toilet-the?",
                        "Excuse me, when does the toilet open?",
                    ],
                    [
                        "Vi behöver veta var den är. Klockan visar inte vägen.",
                        "We need know where it is. Clock-the shows not way-the.",
                        "We need to know where it is. The clock doesn't show the way.",
                    ],
                ),
                no(
                    [
                        "Jag vill köpa en toalett.",
                        "I want buy a toilet.",
                        "I'd like to buy a toilet.",
                    ],
                    [
                        "Vi kan fråga, men hemtransporten blir besvärlig.",
                        "We can ask, but transport-home-the becomes troublesome.",
                        "We can ask, but getting it home will be awkward.",
                    ],
                ),
                no(
                    [
                        "Är sjön ledig?",
                        "Is lake-the free?",
                        "Is the lake available?",
                    ],
                    [
                        "Vi håller oss till toaletten.",
                        "We keep ourselves to toilet-the.",
                        "Let's stick to the toilet.",
                    ],
                ),
            ],
        ),
        p(
            "woods-rain",
            Woods,
            ["Regnet kommer", "Rain-the comes", "Here comes the rain"],
            vec![
                n([
                    "När ni har ätit era smörgåsar börjar det regna. Hemvägen är tydligt skyltad.",
                    "When you [plural] have eaten your sandwiches begins it to rain. Way-home-the is clearly signposted.",
                    "After you've eaten your sandwiches, it starts raining. The way home is clearly signposted.",
                ]),
                w([
                    "Vi kan ta på oss regnjackorna och gå hem.",
                    "We can put on ourselves rain-jackets-the and go home.",
                    "We can put on our rain jackets and walk home.",
                ]),
            ],
            w(["Vad säger du?", "What say you?", "What do you say?"]),
            [
                go(
                    [
                        "Ja, vi går hem innan vi blir genomblöta.",
                        "Yes, we go home before we become soaked-through.",
                        "Yes, let's go home before we're soaked.",
                    ],
                    "home-return",
                    [
                        "Lagom äventyr. Nu längtar jag efter te.",
                        "Just-right adventure. Now long I for tea.",
                        "Just enough adventure. Now I'm looking forward to tea.",
                    ],
                ),
                no(
                    [
                        "Jag väntar här tills sommaren kommer.",
                        "I wait here until summer-the comes.",
                        "I'll wait here until summer arrives.",
                    ],
                    [
                        "Jag skickar en julhälsning om det dröjer.",
                        "I send a Christmas-greeting if it takes a while.",
                        "I'll send a Christmas greeting if it takes a while.",
                    ],
                ),
                no(
                    [
                        "Regnjackan får stanna i väskan så att den inte blir våt.",
                        "Rain-jacket-the gets stay in bag-the so that it not becomes wet.",
                        "The rain jacket can stay in the bag so it doesn't get wet.",
                    ],
                    [
                        "Det är snällt mot jackan, men den har faktiskt ett jobb.",
                        "It is kind toward jacket-the, but it has actually a job.",
                        "That's kind to the jacket, but it does actually have a job.",
                    ],
                ),
                no(
                    [
                        "Vi ber träden att stänga av regnet.",
                        "We ask trees-the to shut off rain-the.",
                        "Let's ask the trees to turn off the rain.",
                    ],
                    [
                        "De verkar redan vara upptagna med att bli blöta.",
                        "They seem already to be occupied with becoming wet.",
                        "They already seem busy getting wet.",
                    ],
                ),
            ],
        ),
        p(
            "shop-list",
            Shop,
            [
                "På väg till Willys",
                "On way to Willys",
                "On the way to Willys",
            ],
            vec![
                n([
                    "Ni går till Willys med tygkassen. Din fru läser inköpslistan.",
                    "You [plural] go to Willys with cloth-bag-the. Your wife reads shopping-list-the.",
                    "You walk to Willys with the reusable bag. Your wife reads the shopping list.",
                ]),
                w([
                    "Vi behöver mjölk, bröd, potatis och bananer.",
                    "We need milk, bread, potatoes and bananas.",
                    "We need milk, bread, potatoes and bananas.",
                ]),
            ],
            w([
                "Vad ska vi köpa till frukosten?",
                "What shall we buy for breakfast-the?",
                "What shall we buy for breakfast?",
            ]),
            [
                go(
                    [
                        "Vi köper mjölk, bröd och bananer.",
                        "We buy milk, bread and bananas.",
                        "Let's buy milk, bread and bananas.",
                    ],
                    "shop-produce",
                    [
                        "Bra. Potatisen kan få vänta till middagen.",
                        "Good. Potatoes-the can get wait until dinner-the.",
                        "Good. The potatoes can wait until dinner.",
                    ],
                ),
                no(
                    [
                        "Fyra diskborstar och en lök.",
                        "Four dish-brushes and an onion.",
                        "Four washing-up brushes and an onion.",
                    ],
                    [
                        "Frukosten blir ren, men lite ensidig.",
                        "Breakfast-the becomes clean, but a little one-sided.",
                        "Breakfast will be clean, but a little limited.",
                    ],
                ),
                no(
                    [
                        "Bara godis. Det går fort.",
                        "Only sweets. It goes fast.",
                        "Just sweets. It's quick.",
                    ],
                    [
                        "Sant. Och sedan blir förmiddagen väldigt lång.",
                        "True. And then becomes before-noon-the very long.",
                        "True. And then the morning will be very long.",
                    ],
                ),
                no(
                    [
                        "Vi köper själva inköpslistan.",
                        "We buy shopping-list-the itself.",
                        "Let's buy the shopping list itself.",
                    ],
                    [
                        "Den är redan betald med min handstil.",
                        "It is already paid for with my handwriting.",
                        "It's already been paid for with my handwriting.",
                    ],
                ),
            ],
        ),
        p(
            "shop-produce",
            Shop,
            [
                "Bland frukt och grönt",
                "Among fruit and greens",
                "In the produce aisle",
            ],
            vec![
                n([
                    "Bananerna kostar per kilo. Du vill bara ha några stycken.",
                    "Bananas-the cost per kilo. You want only have a few pieces.",
                    "The bananas are priced per kilo. You only want a few.",
                ]),
                w([
                    "Vi behöver inte hela lådan.",
                    "We need not whole box-the.",
                    "We don't need the whole box.",
                ]),
            ],
            w([
                "Hur mycket tar du?",
                "How much take you?",
                "How much do you take?",
            ]),
            [
                go(
                    [
                        "Jag tar fyra bananer.",
                        "I take four bananas.",
                        "I'll take four bananas.",
                    ],
                    "shop-aisle",
                    [
                        "Utmärkt. Vi öppnar ingen djurpark i dag.",
                        "Excellent. We open no zoo today.",
                        "Excellent. We're not opening a zoo today.",
                    ],
                ),
                no(
                    [
                        "Jag tar fyra kilo bananer.",
                        "I take four kilos bananas.",
                        "I'll take four kilos of bananas.",
                    ],
                    [
                        "Fyra stycken räcker. Kilo och stycken ger ganska olika frukostplaner.",
                        "Four pieces are enough. Kilos and pieces give fairly different breakfast-plans.",
                        "Four individual bananas will do. Kilos and individual items make rather different breakfast plans.",
                    ],
                ),
                no(
                    [
                        "Jag vill ha en liter bananer.",
                        "I want have one litre bananas.",
                        "I'd like a litre of bananas.",
                    ],
                    [
                        "Då får vi nog börja med en mixer.",
                        "Then get we probably begin with a blender.",
                        "Then we'll probably need to start with a blender.",
                    ],
                ),
                no(
                    [
                        "Jag tar alla. De ser ensamma ut.",
                        "I take all. They look lonely.",
                        "I'll take them all. They look lonely.",
                    ],
                    [
                        "Fint tänkt. Men fruktskålen är inte så social.",
                        "Nicely thought. But fruit-bowl-the is not so social.",
                        "A lovely thought. But the fruit bowl isn't that sociable.",
                    ],
                ),
            ],
        ),
        p(
            "shop-aisle",
            Shop,
            [
                "Var finns mjölken?",
                "Where exists milk-the?",
                "Where is the milk?",
            ],
            vec![
                n([
                    "Du hittar pastan, men ingen mjölk. En kvinnlig medarbetare fyller på en hylla.",
                    "You find pasta-the, but no milk. A female co-worker fills up a shelf.",
                    "You find pasta but no milk. A female employee is restocking a shelf.",
                ]),
                line(
                    ShopAssistant,
                    [
                        "Hej! Behöver du hjälp?",
                        "Hi! Need you help?",
                        "Hi! Do you need help?",
                    ],
                ),
            ],
            line(
                ShopAssistant,
                [
                    "Vad letar du efter?",
                    "What look you for?",
                    "What are you looking for?",
                ],
            ),
            [
                go(
                    [
                        "Ursäkta, var finns mjölken?",
                        "Excuse [me], where exists milk-the?",
                        "Excuse me, where is the milk?",
                    ],
                    "shop-milk",
                    [
                        "Bra frågat. Hon visar oss till mejeriavdelningen.",
                        "Good asked. She shows us to dairy-department-the.",
                        "Well asked. She shows us to the dairy section.",
                    ],
                ),
                no(
                    [
                        "Var bor korna?",
                        "Where live cows-the?",
                        "Where do the cows live?",
                    ],
                    [
                        "Lite tidigare i leveranskedjan.",
                        "A little earlier in delivery-chain-the.",
                        "A little earlier in the supply chain.",
                    ],
                ),
                no(
                    [
                        "Jag letar efter utgången till kylskåpet.",
                        "I look for exit-the to refrigerator-the.",
                        "I'm looking for the exit to the fridge.",
                    ],
                    [
                        "Vi kan börja med mjölken och arbeta oss vidare.",
                        "We can begin with milk-the and work ourselves onward.",
                        "We can start with the milk and work from there.",
                    ],
                ),
                no(
                    [
                        "Ursäkta, vad kostar mjölken?",
                        "Excuse [me], what costs milk-the?",
                        "Excuse me, how much is the milk?",
                    ],
                    [
                        "Priset kan vi fråga om sedan. Först måste vi hitta mjölken.",
                        "Price-the can we ask about later. First must we find milk-the.",
                        "We can ask about the price later. First we need to find the milk.",
                    ],
                ),
            ],
        ),
        p(
            "shop-milk",
            Shop,
            ["Rätt förpackning", "Right package", "The right carton"],
            vec![
                n([
                    "Vid kylen pekar din fru på en förpackning.",
                    "By fridge-the points your wife at a package.",
                    "At the fridge your wife points to a carton.",
                ]),
                w([
                    "Vi brukar köpa en liter mellanmjölk.",
                    "We usually buy a litre medium-milk.",
                    "We usually buy a litre of semi-skimmed milk.",
                ]),
            ],
            w([
                "Vilken förpackning väljer du?",
                "Which package choose you?",
                "Which carton do you choose?",
            ]),
            [
                go(
                    [
                        "En liter mellanmjölk.",
                        "A litre medium-milk.",
                        "One litre of semi-skimmed milk.",
                    ],
                    "shop-bread",
                    [
                        "Precis. Mjölkhyllan är inget personlighetstest.",
                        "Exactly. Milk-shelf-the is no personality-test.",
                        "Exactly. The milk shelf isn't a personality test.",
                    ],
                ),
                no(
                    [
                        "En burk gräddfil.",
                        "A tub sour-cream.",
                        "A tub of sour cream.",
                    ],
                    [
                        "Den är bra till annat. Kaffet kanske blir lite förvånat.",
                        "It is good for other things. Coffee-the maybe becomes a little surprised.",
                        "It's good for other things. The coffee might be a little surprised.",
                    ],
                ),
                no(
                    [
                        "En liter vispgrädde.",
                        "A litre whipping-cream.",
                        "A litre of whipping cream.",
                    ],
                    [
                        "Det blir ett festligt kaffe, men vi skulle ha mellanmjölk.",
                        "It becomes a festive coffee, but we were to have medium-milk.",
                        "That would make festive coffee, but we wanted semi-skimmed milk.",
                    ],
                ),
                no(
                    [
                        "Jag tar hyllan. Då får vi alla sorter.",
                        "I take shelf-the. Then get we all kinds.",
                        "I'll take the shelf. Then we'll get every kind.",
                    ],
                    [
                        "Kassen är rymlig, men inte kommunalt lager.",
                        "Bag-the is roomy, but not a municipal warehouse.",
                        "The bag is roomy, but it isn't a municipal warehouse.",
                    ],
                ),
            ],
        ),
        p(
            "shop-bread",
            Shop,
            [
                "Brödet är slut",
                "Bread-the is at an end",
                "The bread is sold out",
            ],
            vec![
                n([
                    "Det vanliga brödet är slut. Du frågar medarbetaren om det finns mer.",
                    "The usual bread-the is at an end. You ask co-worker-the if there exists more.",
                    "Your usual bread is sold out. You ask the employee whether there is more.",
                ]),
                line(
                    ShopAssistant,
                    [
                        "Tyvärr, det kommer mer i morgon. Vill du prova det här?",
                        "Unfortunately, there comes more tomorrow. Want you try this here?",
                        "Unfortunately, more arrives tomorrow. Would you like to try this one?",
                    ],
                ),
            ],
            w([
                "Hur svarar du om ett annat bröd går bra?",
                "How answer you if another bread goes fine?",
                "How do you reply if another bread is fine?",
            ]),
            [
                go(
                    [
                        "Ja tack, det går bra med det här.",
                        "Yes please, it goes fine with this here.",
                        "Yes please, this one will be fine.",
                    ],
                    "shop-phone",
                    [
                        "Bra. Lördagen är räddad utan krismöte.",
                        "Good. Saturday-the is saved without crisis-meeting.",
                        "Good. Saturday is saved without an emergency meeting.",
                    ],
                ),
                no(
                    [
                        "Då väntar jag här till i morgon.",
                        "Then wait I here until tomorrow.",
                        "Then I'll wait here until tomorrow.",
                    ],
                    [
                        "Jag lämnar smörgåspåsen som sovsäck.",
                        "I leave sandwich-bag-the as sleeping-bag.",
                        "I'll leave the sandwich bag as a sleeping bag.",
                    ],
                ),
                no(
                    [
                        "Kan du baka ett medan jag väntar?",
                        "Can you bake one while I wait?",
                        "Can you bake one while I wait?",
                    ],
                    [
                        "Du överskattar nog vad som ingår i hyllpåfyllningen.",
                        "You overestimate probably what is included in shelf-refilling-the.",
                        "You're probably overestimating what's included in restocking shelves.",
                    ],
                ),
                no(
                    [
                        "Jag tar en limpa potatis i stället.",
                        "I take a loaf potatoes instead.",
                        "I'll take a loaf of potatoes instead.",
                    ],
                    [
                        "Det blir en ganska originell smörgås.",
                        "It becomes a fairly original sandwich.",
                        "That will be quite an original sandwich.",
                    ],
                ),
            ],
        ),
        p(
            "shop-phone",
            Shop,
            [
                "Telefonen är borta",
                "Phone-the is gone",
                "The phone is missing",
            ],
            vec![
                n([
                    "Vid kassan hittar du inte telefonen. Du minns att du lade den bredvid bananerna.",
                    "At checkout-the find you not phone-the. You remember that you laid it beside bananas-the.",
                    "At the checkout you can't find your phone. You remember putting it beside the bananas.",
                ]),
                w([
                    "Vi frågar personalen först.",
                    "We ask staff-the first.",
                    "Let's ask the staff first.",
                ]),
            ],
            w(["Vad säger du?", "What say you?", "What do you say?"]),
            [
                go(
                    [
                        "Jag har tappat bort min telefon. Har någon lämnat in den?",
                        "I have lost away my phone. Has anyone handed in it?",
                        "I've lost my phone. Has anyone handed it in?",
                    ],
                    "shop-checkout",
                    [
                        "Precis. Den ligger i kundservice. Bananerna klarade sig själva.",
                        "Exactly. It lies in customer service. Bananas-the managed themselves.",
                        "Exactly. It's at customer service. The bananas managed on their own.",
                    ],
                ),
                no(
                    [
                        "Jag vill lämna tillbaka min telefon.",
                        "I want hand back my phone.",
                        "I'd like to return my phone.",
                    ],
                    [
                        "Först måste vi hitta den. Tappat bort är något annat än lämna tillbaka.",
                        "First must we find it. Lost away is something other than hand back.",
                        "First we need to find it. Lost is different from returning something.",
                    ],
                ),
                no(
                    [
                        "Ge mig alla telefoner ni har.",
                        "Give me all phones you have.",
                        "Give me every phone you have.",
                    ],
                    [
                        "Vi söker en telefon, inte ett nytt abonnemang på problem.",
                        "We seek one phone, not a new subscription to problems.",
                        "We're looking for one phone, not a new subscription to trouble.",
                    ],
                ),
                no(
                    [
                        "Telefonen har nog flyttat till fruktavdelningen.",
                        "Phone-the has probably moved to fruit-department-the.",
                        "The phone has probably moved to the fruit section.",
                    ],
                    [
                        "Då får vi hjälpa den att flytta hem igen.",
                        "Then get we help it to move home again.",
                        "Then we'll have to help it move home again.",
                    ],
                ),
            ],
        ),
        p(
            "shop-checkout",
            Shop,
            ["I kassan", "At checkout-the", "At the checkout"],
            vec![
                n([
                    "Telefonen är tillbaka. Du lägger varorna på bandet och visar tygkassen.",
                    "Phone-the is back. You put goods-the on belt-the and show cloth-bag-the.",
                    "Your phone is back. You put the groceries on the belt and show your reusable bag.",
                ]),
                line(
                    ShopAssistant,
                    [
                        "Behöver du en påse?",
                        "Need you a bag?",
                        "Do you need a bag?",
                    ],
                ),
            ],
            w([
                "Du har redan en kasse. Vad svarar du?",
                "You have already a carrier bag. What answer you?",
                "You already have a bag. What do you reply?",
            ]),
            [
                go(
                    [
                        "Nej tack, jag har med mig en kasse.",
                        "No thanks, I have with me a carrier bag.",
                        "No thank you, I've brought a bag.",
                    ],
                    "shop-receipt",
                    [
                        "Bra. Den har väntat på sin stora stund.",
                        "Good. It has waited for its big moment.",
                        "Good. It's been waiting for its big moment.",
                    ],
                ),
                no(
                    [
                        "Ja tack, jag behöver en ny kasse.",
                        "Yes please, I need a new carrier bag.",
                        "Yes please, I need a new bag.",
                    ],
                    [
                        "Du kan säga så, men kassen du håller i handen ser lite besviken ut.",
                        "You can say so, but bag-the you hold in hand-the looks a little disappointed.",
                        "You can say that, but the bag in your hand looks a little disappointed.",
                    ],
                ),
                no(
                    [
                        "Nej, jag bär mjölken i fickan.",
                        "No, I carry milk-the in pocket-the.",
                        "No, I'll carry the milk in my pocket.",
                    ],
                    [
                        "Det går bäst så länge förpackningen håller med.",
                        "That goes best as long as carton-the agrees.",
                        "That works best as long as the carton agrees.",
                    ],
                ),
                no(
                    [
                        "Ingår en kundvagn att ta hem?",
                        "Is included a customer-trolley to take home?",
                        "Is a trolley to take home included?",
                    ],
                    [
                        "Bara om du också tänker flytta hit.",
                        "Only if you also intend to move here.",
                        "Only if you're also planning to move in.",
                    ],
                ),
            ],
        ),
        p(
            "shop-receipt",
            Shop,
            [
                "Kvitto och hemväg",
                "Receipt and way home",
                "The receipt and the way home",
            ],
            vec![
                line(
                    ShopAssistant,
                    [
                        "Vill du ha kvittot?",
                        "Want you have receipt-the?",
                        "Would you like the receipt?",
                    ],
                ),
                n([
                    "Du vill kunna kontrollera köpet hemma.",
                    "You want be able to check purchase-the at home.",
                    "You'd like to check the purchase at home.",
                ]),
            ],
            w([
                "Vad säger du innan vi går hem?",
                "What say you before we go home?",
                "What do you say before we go home?",
            ]),
            [
                go(
                    [
                        "Ja tack. Tack så mycket, hej då!",
                        "Yes please. Thanks so much, goodbye!",
                        "Yes please. Thank you very much, goodbye!",
                    ],
                    "home-return",
                    [
                        "Precis. Nu går vi hem innan mjölken får egna planer.",
                        "Exactly. Now go we home before milk-the gets its own plans.",
                        "Exactly. Now let's go home before the milk makes plans of its own.",
                    ],
                ),
                no(
                    [
                        "Nej tack, jag behöver inget kvitto.",
                        "No thanks, I need no receipt.",
                        "No thank you, I don't need a receipt.",
                    ],
                    [
                        "Det är artigt. Men du ville ju kontrollera köpet hemma.",
                        "That is polite. But you wanted, as you know, to check purchase-the at home.",
                        "That's polite. But you did want to check the purchase at home.",
                    ],
                ),
                no(
                    [
                        "Kan jag få kvittot på bananskal?",
                        "Can I get receipt-the on banana-peel?",
                        "Can I have the receipt on a banana skin?",
                    ],
                    [
                        "Det formatet stöds nog inte ännu.",
                        "That format-the is supported probably not yet.",
                        "That format probably isn't supported yet.",
                    ],
                ),
                no(
                    [
                        "Jag tar kvittorullen.",
                        "I take receipt-roll-the.",
                        "I'll take the receipt roll.",
                    ],
                    [
                        "Vi behöver bara vår egen lilla del av historien.",
                        "We need only our own little piece of history-the.",
                        "We only need our own little part of the story.",
                    ],
                ),
            ],
        ),
        p(
            "road-calm",
            Road,
            [
                "Bakom ratten",
                "Behind steering-wheel-the",
                "Behind the wheel",
            ],
            vec![
                n([
                    "Ni kör mot IKEA. En bil bakom tutar trots att ljuset är rött. Du blir irriterad.",
                    "You [plural] drive toward IKEA. A car behind honks despite that light-the is red. You become irritated.",
                    "You're driving towards IKEA. A car behind honks even though the light is red. You feel irritated.",
                ]),
                w([
                    "Vi har ingen brådska. Håll avståndet och kör lugnt.",
                    "We have no hurry. Keep distance-the and drive calmly.",
                    "We're in no hurry. Keep your distance and drive calmly.",
                ]),
            ],
            w(["Vad säger du?", "What say you?", "What do you say?"]),
            [
                go(
                    [
                        "Jag tar det lugnt och väntar på grönt.",
                        "I take it calmly and wait for green.",
                        "I'll stay calm and wait for green.",
                    ],
                    "road-police",
                    [
                        "Bra. Vi ska köpa en hylla, inte vinna en tävling.",
                        "Good. We shall buy a shelf, not win a contest.",
                        "Good. We're buying a shelf, not winning a race.",
                    ],
                ),
                no(
                    [
                        "Jag tutar tillbaka tills det blir grönt.",
                        "I honk back until it becomes green.",
                        "I'll honk back until it turns green.",
                    ],
                    [
                        "Trafikljuset tar tyvärr inte emot musikönskemål.",
                        "Traffic-light-the takes unfortunately not against music-requests.",
                        "Unfortunately the traffic light doesn't take song requests.",
                    ],
                ),
                no(
                    [
                        "Jag kör mot rött så slipper han vänta.",
                        "I drive against red so escapes he waiting.",
                        "I'll drive through red so he doesn't have to wait.",
                    ],
                    [
                        "Nej. Hans brådska ändrar inte trafikreglerna.",
                        "No. His hurry changes not traffic-rules-the.",
                        "No. His hurry doesn't change the traffic rules.",
                    ],
                ),
                no(
                    [
                        "Jag stannar här resten av dagen.",
                        "I stay here rest-the of day-the.",
                        "I'll stay here for the rest of the day.",
                    ],
                    [
                        "Lugnt är bra. Att bosätta sig i korsningen är lite väl lugnt.",
                        "Calm is good. To settle oneself in intersection-the is a little too calm.",
                        "Calm is good. Moving into the junction is a little too calm.",
                    ],
                ),
            ],
        ),
        p(
            "road-police",
            Road,
            ["En poliskontroll", "A police-check", "A police check"],
            vec![
                n([
                    "En polis visar er till en säker kontrollplats. Du stannar enligt hennes anvisningar.",
                    "A police officer shows you [plural] to a safe inspection-place. You stop according to her instructions.",
                    "A police officer directs you to a safe checkpoint. You stop as she directs.",
                ]),
                line(
                    Police,
                    [
                        "God dag. Kan jag få se ditt körkort?",
                        "Good day. Can I get see your driving-licence?",
                        "Good afternoon. May I see your driving licence?",
                    ],
                ),
            ],
            w(["Vad svarar du?", "What answer you?", "What do you reply?"]),
            [
                go(
                    [
                        "Självklart. Här är mitt körkort.",
                        "Self-evidently. Here is my driving-licence.",
                        "Of course. Here is my driving licence.",
                    ],
                    "ikea-parking",
                    [
                        "Bra. Kontrollen är snart klar, och vi kan köra vidare.",
                        "Good. Check-the is soon done, and we can drive onward.",
                        "Good. The check is soon over, and we can continue.",
                    ],
                ),
                no(
                    [
                        "Går det bra med IKEA-kortet?",
                        "Goes it fine with IKEA-card-the?",
                        "Will my IKEA card do?",
                    ],
                    [
                        "Det hjälper mer vid bokhyllor än vid vägkontroller.",
                        "It helps more by book-shelves than at road-checks.",
                        "It's more useful for bookshelves than roadside checks.",
                    ],
                ),
                no(
                    [
                        "Här är mitt bankkort.",
                        "Here is my bank-card.",
                        "Here is my bank card.",
                    ],
                    [
                        "Nästan rätt storlek på kortet. Men hon frågade efter körkortet.",
                        "Almost right size on card-the. But she asked for driving-licence-the.",
                        "Almost the right size of card. But she asked for your driving licence.",
                    ],
                ),
                no(
                    [
                        "Körkortet körde nog hem.",
                        "Driving-licence-the drove probably home.",
                        "The licence probably drove home.",
                    ],
                    [
                        "Då får det förklara hur det nådde pedalerna.",
                        "Then gets it explain how it reached pedals-the.",
                        "Then it'll have to explain how it reached the pedals.",
                    ],
                ),
            ],
        ),
        p(
            "ikea-parking",
            Ikea,
            [
                "Här får du inte parkera",
                "Here may you not park",
                "No parking here",
            ],
            vec![
                n([
                    "Vid IKEA ser du en tom plats, men skylten säger lastzon.",
                    "At IKEA see you an empty place, but sign-the says loading-zone.",
                    "At IKEA you see an empty space, but the sign says loading zone.",
                ]),
                w([
                    "Här får vi inte parkera. Vi behöver en vanlig parkeringsplats.",
                    "Here may we not park. We need an ordinary parking-place.",
                    "We can't park here. We need an ordinary parking space.",
                ]),
            ],
            w(["Vad gör du?", "What do you?", "What do you do?"]),
            [
                go(
                    [
                        "Jag letar efter en tillåten parkeringsplats.",
                        "I look for an allowed parking-place.",
                        "I'll look for a space where parking is permitted.",
                    ],
                    "ikea-entry",
                    [
                        "Precis. Tom plats betyder inte automatiskt ledig plats.",
                        "Exactly. Empty place means not automatically available place.",
                        "Exactly. An empty space doesn't automatically mean an available space.",
                    ],
                ),
                no(
                    [
                        "Jag parkerar bara i en timme.",
                        "I park only for an hour.",
                        "I'll only park for an hour.",
                    ],
                    [
                        "Skylten verkar inte ha något undantag för ordet bara.",
                        "Sign-the seems not to have any exception for word-the only.",
                        "The sign doesn't seem to make an exception for the word only.",
                    ],
                ),
                no(
                    [
                        "Jag lägger en servett över skylten.",
                        "I lay a napkin over sign-the.",
                        "I'll put a napkin over the sign.",
                    ],
                    [
                        "Då har vi fortfarande parkerat fel, fast med dukning.",
                        "Then have we still parked wrong, though with table-setting.",
                        "Then we're still parked incorrectly, but with table linen.",
                    ],
                ),
                no(
                    [
                        "Lastzon betyder väl vanlig parkering?",
                        "Loading-zone means surely ordinary parking?",
                        "Loading zone means ordinary parking, doesn't it?",
                    ],
                    [
                        "Inte riktigt. Här ska varor kunna lastas, inte våra lördagsplaner.",
                        "Not quite. Here shall goods be able to be loaded, not our Saturday-plans.",
                        "Not quite. Goods need to be loaded here, not our Saturday plans.",
                    ],
                ),
            ],
        ),
        p(
            "ikea-entry",
            Ikea,
            ["En liten hylla", "A little shelf", "A small shelf"],
            vec![
                n([
                    "Ni parkerar rätt och går in. Ni behöver en smal hylla till hallen.",
                    "You [plural] park right and go in. You need a narrow shelf for hallway-the.",
                    "You park correctly and go in. You need a narrow shelf for the hallway.",
                ]),
                line(
                    ShopAssistant,
                    [
                        "Hej! Vad kan jag hjälpa er med?",
                        "Hi! What can I help you [plural] with?",
                        "Hi! What can I help you with?",
                    ],
                ),
            ],
            w([
                "Hur förklarar du vad vi söker?",
                "How explain you what we look for?",
                "How do you explain what we're looking for?",
            ]),
            [
                go(
                    [
                        "Vi letar efter en smal hylla till hallen.",
                        "We look for a narrow shelf for hallway-the.",
                        "We're looking for a narrow shelf for the hallway.",
                    ],
                    "ikea-measure",
                    [
                        "Bra. Hon visar oss en modell som kanske passar.",
                        "Good. She shows us a model that maybe fits.",
                        "Good. She shows us a model that might fit.",
                    ],
                ),
                no(
                    [
                        "Vi letar efter ett smalt bord till köket.",
                        "We look for a narrow table for kitchen-the.",
                        "We're looking for a narrow table for the kitchen.",
                    ],
                    [
                        "Smal var rätt. Möbeln och rummet tog en liten omväg.",
                        "Narrow was right. Furniture-piece-the and room-the took a little detour.",
                        "Narrow was right. The furniture and the room took a little detour.",
                    ],
                ),
                no(
                    [
                        "Jag vill ha en hylla till min hylla.",
                        "I want have a shelf for my shelf.",
                        "I'd like a shelf for my shelf.",
                    ],
                    [
                        "Vi börjar med en. Sedan ser vi om den behöver sällskap.",
                        "We begin with one. Then see we if it needs company.",
                        "Let's start with one. Then we'll see whether it needs company.",
                    ],
                ),
                no(
                    [
                        "Vi behöver Sveriges största soffa.",
                        "We need Sweden’s biggest sofa.",
                        "We need Sweden's largest sofa.",
                    ],
                    [
                        "Till vår hall? Då får brevbäraren gå genom fönstret.",
                        "For our hallway? Then gets letter-carrier-the go through window-the.",
                        "For our hallway? Then the postman will have to use the window.",
                    ],
                ),
            ],
        ),
        p(
            "ikea-measure",
            Ikea,
            ["Måttbandet", "Measuring-tape-the", "The tape measure"],
            vec![
                n([
                    "Hyllan är sextio centimeter bred. Utrymmet hemma är bara femtio centimeter.",
                    "Shelf-the is sixty centimetres wide. Space-the at home is only fifty centimetres.",
                    "The shelf is sixty centimetres wide. The space at home is only fifty.",
                ]),
                w([
                    "Vi tog ju med måtten. Den här är för bred.",
                    "We took, as you know, along measurements-the. This here is too wide.",
                    "We did bring the measurements. This one is too wide.",
                ]),
            ],
            w([
                "Vad säger du till medarbetaren?",
                "What say you to co-worker-the?",
                "What do you say to the employee?",
            ]),
            [
                go(
                    [
                        "Har ni en smalare hylla?",
                        "Have you [plural] a narrower shelf?",
                        "Do you have a narrower shelf?",
                    ],
                    "ikea-warehouse",
                    [
                        "Precis. Hon hittar en modell som passar.",
                        "Exactly. She finds a model that fits.",
                        "Exactly. She finds a model that fits.",
                    ],
                ),
                no(
                    [
                        "Vi mäter igen tills den passar.",
                        "We measure again until it fits.",
                        "Let's measure again until it fits.",
                    ],
                    [
                        "Måttbandet är ganska envist på den punkten.",
                        "Measuring-tape-the is fairly stubborn on that point-the.",
                        "The tape measure is rather stubborn on that point.",
                    ],
                ),
                no(
                    [
                        "Sextio är väl mindre än femtio?",
                        "Sixty is surely less than fifty?",
                        "Sixty is less than fifty, isn't it?",
                    ],
                    [
                        "Tvärtom. Siffrorna måste få plats innan hyllan får det.",
                        "Opposite. Numbers-the must get space before shelf-the gets it.",
                        "The other way round. The numbers need to fit before the shelf will.",
                    ],
                ),
                no(
                    [
                        "Vi behöver en bredare ytterdörr och en ny hall.",
                        "We need a wider outer-door and a new hallway.",
                        "We need a wider front door and a new hallway.",
                    ],
                    [
                        "Det blev snabbt en dyr hylla.",
                        "It became quickly an expensive shelf.",
                        "That quickly became an expensive shelf.",
                    ],
                ),
            ],
        ),
        p(
            "ikea-warehouse",
            Ikea,
            ["Gång och fack", "Aisle and compartment", "Aisle and bin"],
            vec![
                line(
                    ShopAssistant,
                    [
                        "Paketet finns på gång tjugofem, fack fyra.",
                        "Package-the exists in aisle twenty-five, compartment four.",
                        "The package is in aisle twenty-five, bay four.",
                    ],
                ),
                w([
                    "Säg gärna numren igen så att vi minns dem.",
                    "Say gladly numbers-the again so that we remember them.",
                    "Say the numbers again so we remember them.",
                ]),
            ],
            w([
                "Vart ska vi gå?",
                "To where shall we go?",
                "Where should we go?",
            ]),
            [
                go(
                    [
                        "Till gång tjugofem, fack fyra.",
                        "To aisle twenty-five, compartment four.",
                        "To aisle twenty-five, bay four.",
                    ],
                    "ikea-toilet",
                    [
                        "Rätt. Nu låter vi nästan som om vi arbetar här.",
                        "Right. Now sound we almost as if we work here.",
                        "Correct. Now we almost sound as if we work here.",
                    ],
                ),
                no(
                    [
                        "Till gång fyra, fack tjugofem.",
                        "To aisle four, compartment twenty-five.",
                        "To aisle four, bay twenty-five.",
                    ],
                    [
                        "Samma siffror, en helt annan promenad.",
                        "Same numbers, a completely different walk.",
                        "The same numbers, a completely different walk.",
                    ],
                ),
                no(
                    [
                        "Till fackföreningen.",
                        "To trade-union-the.",
                        "To the trade union.",
                    ],
                    [
                        "Viktig verksamhet, men hyllan står nog inte där.",
                        "Important activity, but shelf-the stands probably not there.",
                        "Important work, but the shelf probably isn't there.",
                    ],
                ),
                no(
                    [
                        "Till gång femton, fack fyra.",
                        "To aisle fifteen, compartment four.",
                        "To aisle fifteen, bay four.",
                    ],
                    [
                        "Femtio, femton, tjugofem. Siffrorna försöker ta en egen genväg. Vi ska till tjugofem.",
                        "Fifty, fifteen, twenty-five. Numbers-the try to take their own shortcut. We shall [go] to twenty-five.",
                        "Fifty, fifteen, twenty-five. The numbers are trying their own shortcut. We need twenty-five.",
                    ],
                ),
            ],
        ),
        p(
            "ikea-toilet",
            Ikea,
            ["En kort paus", "A short pause", "A quick break"],
            vec![
                n([
                    "Ni har hittat paketet. Nu behöver du gå på toaletten innan ni betalar.",
                    "You [plural] have found package-the. Now need you go on toilet-the before you pay.",
                    "You've found the package. Now you need the toilet before you pay.",
                ]),
                line(
                    ShopAssistant,
                    [
                        "Toaletterna ligger till vänster om restaurangen.",
                        "Toilets-the lie to left of restaurant-the.",
                        "The toilets are to the left of the restaurant.",
                    ],
                ),
            ],
            w([
                "Var ligger toaletterna?",
                "Where lie toilets-the?",
                "Where are the toilets?",
            ]),
            [
                go(
                    [
                        "Till vänster om restaurangen.",
                        "To left of restaurant-the.",
                        "To the left of the restaurant.",
                    ],
                    "ikea-car",
                    [
                        "Ja. Jag väntar här med vagnen.",
                        "Yes. I wait here with trolley-the.",
                        "Yes. I'll wait here with the trolley.",
                    ],
                ),
                no(
                    [
                        "Till vänster om parkeringen.",
                        "To left of parking-lot-the.",
                        "To the left of the car park.",
                    ],
                    [
                        "Vänster var rätt, men restaurangen flyttade visst ut i översättningen.",
                        "Left was right, but restaurant-the apparently moved out in translation-the.",
                        "Left was right, but the restaurant seems to have moved outside in the translation.",
                    ],
                ),
                no(
                    [
                        "Till höger om parkeringen.",
                        "To right of parking-lot-the.",
                        "To the right of the car park.",
                    ],
                    [
                        "Vänster och höger är tyvärr inte utbytbara här.",
                        "Left and right are unfortunately not interchangeable here.",
                        "Unfortunately left and right aren't interchangeable here.",
                    ],
                ),
                no(
                    [
                        "Vi måste bygga dem först.",
                        "We must build them first.",
                        "We have to assemble them first.",
                    ],
                    [
                        "Den insexnyckeln tänker jag inte leta efter.",
                        "That hex-key-the intend I not look for.",
                        "I'm not going to look for that Allen key.",
                    ],
                ),
            ],
        ),
        p(
            "ikea-car",
            Ikea,
            [
                "Får paketet plats?",
                "Gets package-the room?",
                "Will the package fit?",
            ],
            vec![
                n([
                    "Efter betalningen mäter ni paketet och bagageutrymmet. Paketet ryms när baksätet är fällt.",
                    "After payment-the measure you [plural] package-the and luggage-space-the. Package-the fits when back-seat-the is folded.",
                    "After paying you measure the package and the boot. It fits with the rear seat folded.",
                ]),
                w([
                    "Vi följer bilens anvisningar och säkrar lasten innan vi kör.",
                    "We follow car-the’s instructions and secure load-the before we drive.",
                    "We'll follow the car's instructions and secure the load before driving.",
                ]),
            ],
            w([
                "Vad gör vi innan vi åker hem?",
                "What do we before we drive home?",
                "What do we do before we drive home?",
            ]),
            [
                go(
                    [
                        "Vi lastar ordentligt och säkrar paketet.",
                        "We load properly and secure package-the.",
                        "We'll load it properly and secure the package.",
                    ],
                    "home-return",
                    [
                        "Bra. Hyllan behöver ingen egen rundtur i bilen.",
                        "Good. Shelf-the needs no tour of its own in car-the.",
                        "Good. The shelf doesn't need its own tour of the car.",
                    ],
                ),
                no(
                    [
                        "Jag håller paketet med ena handen och kör med den andra.",
                        "I hold package-the with one hand-the and drive with the other.",
                        "I'll hold the package with one hand and drive with the other.",
                    ],
                    [
                        "Nej. Vi behöver en säker lösning, inte ett jongleringsnummer.",
                        "No. We need a safe solution, not a juggling-act.",
                        "No. We need a safe solution, not a juggling act.",
                    ],
                ),
                no(
                    [
                        "Vi lägger det löst över framsätena.",
                        "We lay it loose across front-seats-the.",
                        "We'll lay it loose across the front seats.",
                    ],
                    [
                        "En lös hylla är ingen bra medpassagerare.",
                        "A loose shelf is no fellow-passenger.",
                        "A loose shelf isn't a good passenger.",
                    ],
                ),
                no(
                    [
                        "Vi ber hyllan att spänna fast sig.",
                        "We ask shelf-the to strap fast itself.",
                        "We'll ask the shelf to fasten its seat belt.",
                    ],
                    [
                        "Den har knappt lärt sig att stå än.",
                        "It has barely learned itself to stand yet.",
                        "It has barely learned to stand yet.",
                    ],
                ),
            ],
        ),
        p(
            "train-ticket",
            Train,
            [
                "Biljetter till Göteborg",
                "Tickets to Gothenburg",
                "Tickets to Gothenburg",
            ],
            vec![
                n([
                    "Ni går till stationen. I biljettappen väljer du Göteborg och två vuxna.",
                    "You [plural] go to station-the. In ticket-app-the choose you Gothenburg and two adults.",
                    "You walk to the station. In the ticket app you select Gothenburg and two adults.",
                ]),
                w([
                    "Kontrollera datum och avgång innan du betalar.",
                    "Check date and departure before you pay.",
                    "Check the date and departure before paying.",
                ]),
            ],
            w([
                "Vilka biljetter behöver vi?",
                "Which tickets need we?",
                "Which tickets do we need?",
            ]),
            [
                go(
                    [
                        "Två vuxenbiljetter till Göteborg i dag.",
                        "Two adult-tickets to Gothenburg today.",
                        "Two adult tickets to Gothenburg today.",
                    ],
                    "train-delay",
                    [
                        "Precis. Båda ska med, även om bara en av oss pratar mest.",
                        "Exactly. Both shall come along, even if only one of us talks most.",
                        "Exactly. We're both going, even if only one of us does most of the talking.",
                    ],
                ),
                no(
                    [
                        "Två barnbiljetter till Göteborg.",
                        "Two child-tickets to Gothenburg.",
                        "Two children's tickets to Gothenburg.",
                    ],
                    [
                        "Jag uppskattar komplimangen. Men vi behöver vuxenbiljetter.",
                        "I appreciate compliment-the. But we need adult-tickets.",
                        "I appreciate the compliment. But we need adult tickets.",
                    ],
                ),
                no(
                    [
                        "Två vuxenbiljetter till Göteborg i morgon.",
                        "Two adult-tickets to Gothenburg tomorrow.",
                        "Two adult tickets to Gothenburg tomorrow.",
                    ],
                    [
                        "Nästan. Men lördagsutflykten vill gärna börja i dag.",
                        "Almost. But Saturday-outing-the wants gladly to begin today.",
                        "Almost. But the Saturday outing would rather start today.",
                    ],
                ),
                no(
                    [
                        "En biljett till varje sko.",
                        "One ticket to each shoe.",
                        "One ticket for each shoe.",
                    ],
                    [
                        "Då reser skorna bättre än vi.",
                        "Then travel shoes-the better than we.",
                        "Then the shoes will travel better than we do.",
                    ],
                ),
            ],
        ),
        p(
            "train-delay",
            Train,
            ["Försenat", "Delayed", "Running late"],
            vec![
                n([
                    "På tavlan står det att tåget är tjugo minuter försenat. En man bredvid er suckar.",
                    "On board-the stands it that train-the is twenty minutes delayed. A man beside you [plural] sighs.",
                    "The board says the train is twenty minutes late. A man beside you sighs.",
                ]),
                line(
                    Passenger,
                    [
                        "Man hinner visst bli kaffesugen igen.",
                        "One has time apparently to become coffee-keen again.",
                        "Apparently there's time to fancy another coffee.",
                    ],
                ),
            ],
            w([
                "Vad betyder informationen på tavlan?",
                "What means information-the on board-the?",
                "What does the information on the board mean?",
            ]),
            [
                go(
                    [
                        "Tåget kommer tjugo minuter senare.",
                        "Train-the comes twenty minutes later.",
                        "The train will arrive twenty minutes later.",
                    ],
                    "train-change",
                    [
                        "Ja. Vi håller koll på nya besked.",
                        "Yes. We keep watch on new announcements.",
                        "Yes. Let's watch for updates.",
                    ],
                ),
                no(
                    [
                        "Resan tar bara tjugo minuter.",
                        "Journey-the takes only twenty minutes.",
                        "The journey only takes twenty minutes.",
                    ],
                    [
                        "Det vore trevligt, men tavlan är inte riktigt så generös.",
                        "That would be nice, but board-the is not really so generous.",
                        "That would be nice, but the board isn't quite that generous.",
                    ],
                ),
                no(
                    [
                        "Tåget avgår tjugo minuter tidigare.",
                        "Train-the departs twenty minutes earlier.",
                        "The train leaves twenty minutes earlier.",
                    ],
                    [
                        "Senare, inte tidigare. Järnvägen ger oss tid, den tar inte tillbaka gårdagen.",
                        "Later, not earlier. Railway-the gives us time, it takes not back yesterday-the.",
                        "Later, not earlier. The railway is giving us time, not taking back yesterday.",
                    ],
                ),
                no(
                    [
                        "Kaffet är tjugo minuter gratis.",
                        "Coffee-the is twenty minutes free.",
                        "The coffee is free for twenty minutes.",
                    ],
                    [
                        "Det erbjudandet finns tyvärr bara i din översättning.",
                        "That offer-the exists unfortunately only in your translation.",
                        "Unfortunately that offer only exists in your translation.",
                    ],
                ),
            ],
        ),
        p(
            "train-change",
            Train,
            [
                "Tåget är inställt",
                "Train-the is cancelled",
                "The train is cancelled",
            ],
            vec![
                n([
                    "Ett nytt besked kommer: tåget är inställt på grund av ett fordonsfel.",
                    "A new announcement comes: train-the is cancelled because of a vehicle-fault.",
                    "An update arrives: the train is cancelled because of a fault with the train.",
                ]),
                line(
                    Conductor,
                    [
                        "En ersättningsbuss till Göteborg avgår från hållplats C. Biljetten gäller på bussen.",
                        "A replacement-bus to Gothenburg departs from stop C. Ticket-the applies on bus-the.",
                        "A replacement bus to Gothenburg leaves from stop C. Your ticket is valid on the bus.",
                    ],
                ),
            ],
            w(["Vad gör vi nu?", "What do we now?", "What do we do now?"]),
            [
                go(
                    [
                        "Vi går till hållplats C och tar ersättningsbussen.",
                        "We go to stop C and take replacement-bus-the.",
                        "Let's go to stop C and take the replacement bus.",
                    ],
                    "train-bus",
                    [
                        "Bra. I dag får ordet tåg lite fler hjul än vanligt.",
                        "Good. Today gets word-the train a few more wheels than usual.",
                        "Good. Today the word train gets a few more wheels than usual.",
                    ],
                ),
                no(
                    [
                        "Vi väntar inne i det inställda tåget.",
                        "We wait inside in the cancelled train-the.",
                        "We'll wait inside the cancelled train.",
                    ],
                    [
                        "Först måste du hitta tåget som inte kommer.",
                        "First must you find train-the that not comes.",
                        "First you'll have to find the train that isn't coming.",
                    ],
                ),
                no(
                    [
                        "Vi köper en ny biljett utan att fråga.",
                        "We buy a new ticket without asking.",
                        "Let's buy a new ticket without asking.",
                    ],
                    [
                        "Den vi har gäller redan. Plånboken tackar för att vi lyssnade.",
                        "The one we have applies already. Wallet-the gives thanks for that we listened.",
                        "The one we have is already valid. Your wallet thanks us for listening.",
                    ],
                ),
                no(
                    [
                        "Vi går till hållplats B.",
                        "We go to stop B.",
                        "Let's go to stop B.",
                    ],
                    [
                        "C, inte B. Bara en bokstav bort, men bussen kan vara ganska noga.",
                        "C, not B. Only one letter away, but bus-the can be fairly particular.",
                        "C, not B. Only one letter away, but the bus can be rather particular.",
                    ],
                ),
            ],
        ),
        p(
            "train-bus",
            Train,
            ["Rätt buss", "Right bus", "The right bus"],
            vec![
                n([
                    "Vid hållplats C står två bussar. Du vill kontrollera att ni kliver på rätt.",
                    "At stop C stand two buses. You want check that you [plural] step on right.",
                    "There are two buses at stop C. You want to check you're boarding the right one.",
                ]),
                line(
                    BusDriver,
                    [
                        "Hej! Visa gärna biljetterna.",
                        "Hi! Show gladly tickets-the.",
                        "Hi! Please show your tickets.",
                    ],
                ),
            ],
            w([
                "Vad frågar du chauffören?",
                "What ask you driver-the?",
                "What do you ask the driver?",
            ]),
            [
                go(
                    [
                        "Går den här bussen till Göteborg?",
                        "Goes this here bus-the to Gothenburg?",
                        "Does this bus go to Gothenburg?",
                    ],
                    "train-arrival",
                    [
                        "Precis. Hon säger ja och kontrollerar biljetterna.",
                        "Exactly. She says yes and checks tickets-the.",
                        "Exactly. She says yes and checks the tickets.",
                    ],
                ),
                no(
                    [
                        "Kan du köra oss hem till köket?",
                        "Can you drive us home to kitchen-the?",
                        "Can you drive us home to the kitchen?",
                    ],
                    [
                        "Vi har biljett till Göteborg, inte till kaffebryggaren.",
                        "We have ticket to Gothenburg, not to coffee-maker-the.",
                        "We have tickets to Gothenburg, not to the coffee maker.",
                    ],
                ),
                no(
                    [
                        "Är det här ett tåg med gummihjul?",
                        "Is this here a train with rubber-wheels?",
                        "Is this a train with rubber tyres?",
                    ],
                    [
                        "Nästan rätt känsla, men fråga efter destinationen.",
                        "Almost right feeling, but ask for destination-the.",
                        "Almost the right idea, but ask about the destination.",
                    ],
                ),
                no(
                    [
                        "Kommer bussen från Göteborg?",
                        "Comes bus-the from Gothenburg?",
                        "Does the bus come from Gothenburg?",
                    ],
                    [
                        "Vi vill veta vart den går, inte hur dess morgon har varit.",
                        "We want know to where it goes, not how its morning has been.",
                        "We want to know where it's going, not how its morning has been.",
                    ],
                ),
            ],
        ),
        p(
            "train-arrival",
            City,
            [
                "Framme i Göteborg",
                "Arrived in Gothenburg",
                "Arrival in Gothenburg",
            ],
            vec![
                n([
                    "Bussen kommer fram till Göteborg. Ni går till ett kafé nära stationen.",
                    "Bus-the comes forth to Gothenburg. You [plural] go to a café near station-the.",
                    "The bus arrives in Gothenburg. You walk to a café near the station.",
                ]),
                w([
                    "Det blev en annan resa än vi tänkte, men nu är vi här.",
                    "It became a different trip than we thought, but now are we here.",
                    "It wasn't the journey we planned, but now we're here.",
                ]),
                line(
                    CafeStaff,
                    [
                        "Hej! Vad får det lov att vara?",
                        "Hi! What gets it permission to be?",
                        "Hi! What can I get you?",
                    ],
                ),
            ],
            w([
                "Beställ fika till oss båda.",
                "Order fika for us both.",
                "Order fika for both of us.",
            ]),
            [
                go(
                    [
                        "Två kaffe och två kanelbullar, tack.",
                        "Two coffees and two cinnamon-buns, please.",
                        "Two coffees and two cinnamon buns, please.",
                    ],
                    "city-table",
                    [
                        "Bra. En fungerande svensk reservplan.",
                        "Good. A functioning Swedish backup-plan.",
                        "Good. A working Swedish backup plan.",
                    ],
                ),
                no(
                    [
                        "En kaffe och en kanelbulle, tack.",
                        "One coffee and one cinnamon-bun, please.",
                        "One coffee and one cinnamon bun, please.",
                    ],
                    [
                        "Jag är också här. Två kaffe och två bullar, om jag får rösta.",
                        "I am also here. Two coffees and two buns, if I get vote.",
                        "I'm here too. Two coffees and two buns, if I get a vote.",
                    ],
                ),
                no(
                    [
                        "Jag vill ha Göteborg på en tallrik.",
                        "I want have Gothenburg on a plate.",
                        "I'd like Gothenburg on a plate.",
                    ],
                    [
                        "Vi börjar med en bulle. Den ryms bättre.",
                        "We begin with a bun. It fits better.",
                        "Let's start with a bun. It'll fit better.",
                    ],
                ),
                no(
                    [
                        "En kanelbulle till kaffet och kaffe till kanelbullen och ingen kopp.",
                        "A cinnamon-bun with coffee-the and coffee with cinnamon-bun-the and no cup.",
                        "A cinnamon bun for the coffee and coffee for the bun and no cup.",
                    ],
                    [
                        "Jag tror beställningen behöver en liten paus.",
                        "I think order-the needs a little pause.",
                        "I think the order needs a little break.",
                    ],
                ),
            ],
        ),
        p(
            "city-table",
            City,
            [
                "Är det ledigt här?",
                "Is it free here?",
                "Are these seats free?",
            ],
            vec![
                n([
                    "Kaféet är fullt. Vid ett bord sitter en man med två tomma stolar bredvid sig.",
                    "Café-the is full. At a table sits a man with two empty chairs beside himself.",
                    "The café is busy. A man sits at a table with two empty chairs beside him.",
                ]),
                w([
                    "Vi frågar innan vi sätter oss.",
                    "We ask before we sit ourselves.",
                    "Let's ask before we sit down.",
                ]),
            ],
            w(["Vad säger du?", "What say you?", "What do you say?"]),
            [
                go(
                    [
                        "Ursäkta, är de här platserna lediga?",
                        "Excuse [me], are these here places-the free?",
                        "Excuse me, are these seats free?",
                    ],
                    "city-toilet",
                    [
                        "Ja, säger han. Ibland räcker en liten fråga.",
                        "Yes, says he. Sometimes suffices a little question.",
                        "Yes, he says. Sometimes a small question is enough.",
                    ],
                ),
                no(
                    [
                        "Kan du flytta hem så att vi får plats?",
                        "Can you move home so that we get room?",
                        "Can you move home so there's room for us?",
                    ],
                    [
                        "Vi behöver två stolar, inte hela hans eftermiddag.",
                        "We need two chairs, not whole his afternoon.",
                        "We need two chairs, not his whole afternoon.",
                    ],
                ),
                no(
                    [
                        "Jag bokade den här stolen i går i en dröm.",
                        "I booked this here chair-the yesterday in a dream.",
                        "I booked this chair in a dream yesterday.",
                    ],
                    [
                        "Tyvärr tar de nog inte emot drömbokningar.",
                        "Unfortunately take they probably not against dream-bookings.",
                        "Unfortunately they probably don't accept dream bookings.",
                    ],
                ),
                no(
                    [
                        "Är de här platserna upptagna? Tack, då sätter vi oss.",
                        "Are these here places-the occupied? Thanks, then sit we ourselves.",
                        "Are these seats taken? Thanks, we'll sit down then.",
                    ],
                    [
                        "Upptagna betyder att någon använder dem. Vi väntar på svaret först.",
                        "Occupied means that someone uses them. We wait for answer-the first.",
                        "Taken means somebody is using them. Let's wait for the answer first.",
                    ],
                ),
            ],
        ),
        p(
            "city-toilet",
            City,
            [
                "En kod till dörren",
                "A code to door-the",
                "A code for the door",
            ],
            vec![
                n([
                    "Efter kaffet behöver du toaletten. Dörren har ett kodlås.",
                    "After coffee-the need you toilet-the. Door-the has a code-lock.",
                    "After coffee you need the toilet. The door has a code lock.",
                ]),
                line(
                    CafeStaff,
                    [
                        "Koden står längst ner på kvittot.",
                        "Code-the stands farthest down on receipt-the.",
                        "The code is at the very bottom of the receipt.",
                    ],
                ),
            ],
            w([
                "Var hittar du koden?",
                "Where find you code-the?",
                "Where do you find the code?",
            ]),
            [
                go(
                    [
                        "Längst ner på kvittot.",
                        "Farthest down on receipt-the.",
                        "At the very bottom of the receipt.",
                    ],
                    "train-home",
                    [
                        "Precis. Ännu ett skäl att spara kvittot.",
                        "Exactly. Yet another reason to save receipt-the.",
                        "Exactly. Another reason to keep the receipt.",
                    ],
                ),
                no(
                    [
                        "På undersidan av kanelbullen.",
                        "On underside-the of cinnamon-bun-the.",
                        "On the underside of the cinnamon bun.",
                    ],
                    [
                        "Den informationen har du redan ätit upp.",
                        "That information-the have you already eaten up.",
                        "You've already eaten that information.",
                    ],
                ),
                no(
                    [
                        "Högst upp på kvittot.",
                        "Highest up on receipt-the.",
                        "At the very top of the receipt.",
                    ],
                    [
                        "Längst ner. Kvittot har tydligen fått både vind och källare.",
                        "Farthest down. Receipt-the has apparently got both attic and cellar.",
                        "At the bottom. Apparently the receipt has acquired both an attic and a cellar.",
                    ],
                ),
                no(
                    [
                        "Jag frågar kaffekoppen om lösenordet.",
                        "I ask coffee-cup-the about password-the.",
                        "I'll ask the coffee cup for the password.",
                    ],
                    [
                        "Den är rätt tom på svar just nu.",
                        "It is rather empty of answers just now.",
                        "It's rather empty of answers right now.",
                    ],
                ),
            ],
        ),
        p(
            "train-home",
            Train,
            ["Hemresan", "Home-journey-the", "The journey home"],
            vec![
                n([
                    "Ni promenerar tillbaka till stationen. Tåget ni tänkte ta har just gått.",
                    "You [plural] walk back to station-the. Train-the you intended take has just gone.",
                    "You walk back to the station. The train you planned to catch has just left.",
                ]),
                line(
                    Conductor,
                    [
                        "Nästa avgång går om en halvtimme. Kontrollera att era biljetter gäller.",
                        "Next departure goes in half an hour. Check that your tickets apply.",
                        "The next departure is in half an hour. Check that your tickets are valid.",
                    ],
                ),
            ],
            w([
                "Vad frågar du innan vi går till perrongen?",
                "What ask you before we go to platform-the?",
                "What do you ask before we go to the platform?",
            ]),
            [
                go(
                    [
                        "Gäller våra biljetter på nästa tåg?",
                        "Apply our tickets on next train?",
                        "Are our tickets valid on the next train?",
                    ],
                    "home-return",
                    [
                        "Bra. Konduktören hjälper oss att ordna rätt biljetter och hitta rätt spår.",
                        "Good. Conductor-the helps us to arrange right tickets and find right track.",
                        "Good. The conductor helps us sort out the right tickets and find the right platform.",
                    ],
                ),
                no(
                    [
                        "Kan du be tåget att backa?",
                        "Can you ask train-the to reverse?",
                        "Can you ask the train to reverse?",
                    ],
                    [
                        "Det är nog enklare att vänta än att ändra hela järnvägen.",
                        "It is probably simpler to wait than to change whole railway-the.",
                        "It's probably easier to wait than to rearrange the entire railway.",
                    ],
                ),
                no(
                    [
                        "Kan vi sitta på tavlan tills nästa avgång?",
                        "Can we sit on board-the until next departure?",
                        "Can we sit on the departure board until the next train?",
                    ],
                    [
                        "Det finns bänkar med betydligt bättre sittkomfort.",
                        "There exist benches with considerably better sitting-comfort.",
                        "There are benches with considerably better seating comfort.",
                    ],
                ),
                no(
                    [
                        "Kan jag få tillbaka pengarna för kaffet?",
                        "Can I get back money-the for coffee-the?",
                        "Can I get a refund for the coffee?",
                    ],
                    [
                        "En tydlig fråga. Men konduktören ansvarar inte för fikat.",
                        "A clear question. But conductor-the is responsible not for fika-the.",
                        "A clear question. But the conductor isn't responsible for the fika.",
                    ],
                ),
            ],
        ),
        p(
            "home-return",
            Home,
            [
                "Hemma vid dörren",
                "Home at door-the",
                "Back at the front door",
            ],
            vec![
                n([
                    "Efter utflykten står ni vid ytterdörren igen. Du känner efter i fickorna.",
                    "After outing-the stand you [plural] at outer-door-the again. You feel around in pockets-the.",
                    "After the outing you're at the front door again. You check your pockets.",
                ]),
                w([
                    "Nycklarna lade du i jackfickan innan vi gick.",
                    "Keys-the laid you in jacket-pocket-the before we went.",
                    "You put the keys in your jacket pocket before we left.",
                ]),
            ],
            w([
                "Vad säger du när du hittar dem?",
                "What say you when you find them?",
                "What do you say when you find them?",
            ]),
            [
                go(
                    [
                        "Här är nycklarna. Jag låser upp.",
                        "Here are keys-the. I unlock up.",
                        "Here are the keys. I'll unlock the door.",
                    ],
                    "home-evening",
                    [
                        "Skönt. Huset står kvar och vi kommer in också.",
                        "Lovely. House-the stands still and we come in too.",
                        "Lovely. The house is still here, and we can get inside too.",
                    ],
                ),
                no(
                    [
                        "Jag tror att dörren behöver en biljett.",
                        "I think that door-the needs a ticket.",
                        "I think the door needs a ticket.",
                    ],
                    [
                        "Den har redan en egen plats.",
                        "It has already its own place.",
                        "It already has a place of its own.",
                    ],
                ),
                no(
                    [
                        "Vi väntar på nästa ersättningsdörr.",
                        "We wait for next replacement-door.",
                        "Let's wait for the next replacement door.",
                    ],
                    [
                        "Den här verkar gå enligt tidtabell.",
                        "This here seems to go according to timetable.",
                        "This one seems to be running on time.",
                    ],
                ),
                no(
                    [
                        "Nycklar är nog bara en rekommendation.",
                        "Keys are probably only a recommendation.",
                        "Keys are probably just a recommendation.",
                    ],
                    [
                        "Låset har ganska bestämda åsikter om det.",
                        "Lock-the has fairly firm opinions about it.",
                        "The lock has rather firm opinions about that.",
                    ],
                ),
            ],
        ),
        p(
            "home-evening",
            Home,
            [
                "Dagens sista fråga",
                "Day-the’s last question",
                "The last question of the day",
            ],
            vec![
                n([
                    "Ni är hemma igen. Jackorna hänger i hallen och vattenkokaren börjar susa.",
                    "You [plural] are home again. Jackets-the hang in hallway-the and water-boiler-the begins to hum.",
                    "You're home again. The jackets hang in the hallway and the kettle starts to hum.",
                ]),
                w([
                    "Du klarade dig bra. Man behöver inte säga allt perfekt för att bli förstådd.",
                    "You managed yourself well. One needs not say everything perfectly to become understood.",
                    "You did well. You don't need to say everything perfectly to be understood.",
                ]),
            ],
            w([
                "Vad vill du säga om dagen?",
                "What want you say about day-the?",
                "What would you like to say about the day?",
            ]),
            [
                go(
                    [
                        "Det var en fin dag. Tack för hjälpen med svenskan.",
                        "It was a fine day. Thanks for help-the with Swedish-the.",
                        "It was a lovely day. Thanks for helping me with Swedish.",
                    ],
                    "done",
                    FAREWELL_TEXT,
                ),
                no(
                    [
                        "Jag har lärt mig att aldrig lämna köket.",
                        "I have learned myself to never leave kitchen-the.",
                        "I've learned never to leave the kitchen.",
                    ],
                    [
                        "Det vore synd. Vi har ju bara börjat.",
                        "That would be a shame. We have after all only begun.",
                        "That would be a shame. We've only just started.",
                    ],
                ),
                no(
                    [
                        "Nu talar jag bättre svenska än alla svenskar.",
                        "Now speak I better Swedish than all Swedes.",
                        "Now I speak better Swedish than all Swedes.",
                    ],
                    [
                        "Skönt. Då får du förklara skattedeklarationen nästa gång.",
                        "Lovely. Then get you explain tax-declaration-the next time.",
                        "Lovely. Then you can explain the tax return next time.",
                    ],
                ),
                no(
                    [
                        "Kan vi köpa en ny lördag med kvittot?",
                        "Can we buy a new Saturday with receipt-the?",
                        "Can we buy a new Saturday with the receipt?",
                    ],
                    [
                        "Vi får en till nästa vecka. Utan kvitto.",
                        "We get one more next week. Without receipt.",
                        "We'll get another next week. No receipt needed.",
                    ],
                ),
            ],
        ),
    ];
    for (index, page) in pages.iter_mut().enumerate() {
        page.choices.rotate_left(index % 4);
    }
    pages
}

pub struct Book {
    pub pages: Vec<Page>,
    current: usize,
    pub selected: Option<usize>,
    pub finished: bool,
}

impl Default for Book {
    fn default() -> Self {
        Self::randomized(|_| 0)
    }
}

fn normalise(input: &str) -> String {
    input
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_lowercase()
}

impl Book {
    pub fn randomized(choose: impl FnMut(usize) -> usize) -> Self {
        Self::with_encounters(choose, &[])
    }

    fn with_encounters(mut choose: impl FnMut(usize) -> usize, previous: &[Page]) -> Self {
        let mut pages = pages();
        encounters::insert(&mut pages, previous, &mut choose);
        Self {
            pages,
            current: 0,
            selected: None,
            finished: false,
        }
    }

    pub fn restart(&mut self, choose: impl FnMut(usize) -> usize) {
        *self = Self::with_encounters(choose, &self.pages);
    }

    pub fn new_outing(&mut self, choose: impl FnMut(usize) -> usize) {
        self.restart(choose);
        self.follow("home-plan");
    }

    pub fn page(&self) -> &Page {
        &self.pages[self.current]
    }

    pub fn answer(&mut self, input: &str) -> Result<Choice, &'static str> {
        if self.finished || self.selected.is_some() {
            return Err("Continue or try again before answering another question.");
        }
        let input = normalise(input);
        let choice = input
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=4).contains(n))
            .map(|n| n - 1)
            .or_else(|| {
                self.page()
                    .choices
                    .iter()
                    .position(|choice| normalise(choice.line.swedish) == input)
            })
            .ok_or("Enter 1–4, click an option, or type one of the complete Swedish responses.")?;
        self.selected = Some(choice);
        Ok(self.page().choices[choice])
    }

    pub fn continue_story(&mut self) {
        if let Some(selected) = self.selected.take()
            && let Some(next) = self.page().choices[selected].next
        {
            self.follow(next);
        }
    }

    pub fn skip(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if let Some(selected) = self.selected.take()
            && let Some(next) = self.page().choices[selected].next
        {
            self.follow(next);
            return true;
        }
        if let Some(next) = self.page().choices.iter().find_map(|choice| choice.next) {
            self.follow(next);
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
                .expect("Story links are validated by tests");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn every_outing_is_reachable_and_every_route_returns_home_without_cycles() {
        fn walk(
            id: &'static str,
            pages: &HashMap<&'static str, &Page>,
            path: &mut Vec<&'static str>,
            seen: &mut HashSet<&'static str>,
        ) {
            if id == "done" {
                assert!(path.contains(&"home-return") && path.contains(&"home-evening"));
                return;
            }
            assert!(!path.contains(&id), "Story cycle: {path:?} -> {id}");
            let page = pages.get(id).expect("Broken story link");
            seen.insert(page.id);
            path.push(page.id);
            assert!(page.choices.iter().any(|c| c.next.is_some()));
            let answers: HashSet<_> = page
                .choices
                .iter()
                .map(|c| normalise(c.line.swedish))
                .collect();
            assert_eq!(answers.len(), 4);
            for choice in &page.choices {
                assert!(!choice.line.english.is_empty() && !choice.reply.swedish.is_empty());
                if let Some(next) = choice.next {
                    walk(next, pages, path, seen);
                }
            }
            path.pop();
        }
        let mut all_seen = HashSet::new();
        for index in 0..25 {
            let book = Book::randomized(|upper| index % upper);
            let by_id: HashMap<_, _> = book.pages.iter().map(|page| (page.id, page)).collect();
            assert_eq!(by_id.len(), book.pages.len());
            let mut seen = HashSet::new();
            walk("home-weather", &by_id, &mut Vec::new(), &mut seen);
            assert_eq!(seen.len(), book.pages.len());
            all_seen.extend(seen);
        }
        let all_ids: HashSet<_> = pages()
            .into_iter()
            .chain(encounters::all_pages())
            .map(|page| page.id)
            .collect();
        assert_eq!(all_ids.len(), 335);
        assert_eq!(all_seen, all_ids);
    }

    #[test]
    fn typed_and_numbered_answers_retry_without_moving_and_then_follow_the_selected_branch() {
        let mut book = Book::default();
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
        assert_eq!(book.page().id, "home-weather");
        let right = book
            .page()
            .choices
            .iter()
            .find(|c| c.next.is_some())
            .unwrap()
            .line
            .swedish
            .to_uppercase();
        assert_eq!(book.answer(&right).unwrap().next, Some("home-keys"));
        book.continue_story();
        assert_eq!(book.page().id, "home-keys");
    }

    #[test]
    fn the_book_can_be_completed_via_each_of_the_four_destinations() {
        for destination in ["woods-gate", "shop-list", "road-calm", "train-ticket"] {
            let mut book = Book::default();
            let mut visited = Vec::new();
            for _ in 0..book.pages.len() {
                if book.finished {
                    break;
                }
                visited.push(book.page().id);
                let choice = book
                    .page()
                    .choices
                    .iter()
                    .position(|choice| {
                        if book.page().id == "home-plan" {
                            choice.next == Some(destination)
                        } else {
                            choice.next.is_some()
                        }
                    })
                    .unwrap();
                book.answer(&(choice + 1).to_string()).unwrap();
                book.continue_story();
            }
            assert!(book.finished, "{destination}: {visited:?}");
            assert!(visited.contains(&destination));
            assert_eq!(book.page().id, "home-evening");
        }
    }

    #[test]
    fn all_story_lines_have_speakers_roles_and_three_line_translations() {
        let mut phrases = Vec::new();
        for page in pages().into_iter().chain(encounters::all_pages()) {
            phrases.extend(page.phrases());
            phrases.extend(page.choices.iter().map(|choice| choice.reply.phrase()));
        }
        phrases.extend(encounters::distractor_phrases());
        for phrase in &phrases {
            assert!(phrase.voice.is_some() && phrase.speaker.is_some());
            assert!(!phrase.swedish.is_empty());
            assert!(phrase.literal.is_some_and(|literal| !literal.is_empty()));
            assert!(!phrase.english.is_empty());
        }
        assert_eq!(Actor::Wife.voice(), Voice::Sofie);
        assert_eq!(Actor::Husband.voice(), Voice::Mattias);
        assert_eq!(Actor::Police.voice(), Voice::Sofie);
        assert_eq!(Actor::Conductor.voice(), Voice::Mattias);
    }

    #[test]
    fn willys_encounters_hold_steady_on_retry_and_change_for_the_next_outing() {
        let mut book = Book::default();
        let take_willys_route = |book: &mut Book| {
            let choice = book
                .page()
                .choices
                .iter()
                .find(|choice| choice.next == Some("shop-list"))
                .unwrap();
            book.answer(choice.line.swedish).unwrap();
            book.continue_story();
            assert!(book.skip());
        };
        assert!(book.skip() && book.skip());
        take_willys_route(&mut book);
        let first_item = book.page().id;
        let wrong = book
            .page()
            .choices
            .iter()
            .find(|choice| choice.next.is_none())
            .unwrap();
        book.answer(wrong.line.swedish).unwrap();
        book.continue_story();
        assert_eq!(book.page().id, first_item);
        assert!(book.skip());
        assert_eq!(book.page().id, "shop-produce");
        while book.skip() {}
        assert!(book.finished);
        book.new_outing(|_| 0);
        assert_eq!(book.page().id, "home-plan");
        take_willys_route(&mut book);
        assert_ne!(book.page().id, first_item);
        book.restart(|_| 0);
        assert_eq!(book.page().id, "home-weather");
    }

    #[test]
    fn skipping_advances_unanswered_and_wrong_pages_until_the_book_finishes() {
        let mut book = Book::default();
        assert_eq!(book.page().id, "home-weather");
        assert!(book.skip());
        assert_eq!(book.page().id, "home-keys");
        assert!(book.selected.is_none());
        let wrong = book
            .page()
            .choices
            .iter()
            .position(|choice| choice.next.is_none())
            .unwrap();
        book.answer(&(wrong + 1).to_string()).unwrap();
        assert!(book.selected.is_some());
        assert!(book.skip());
        assert_eq!(book.page().id, "home-plan");
        assert!(book.selected.is_none());
        for _ in 0..book.pages.len() {
            if !book.skip() {
                break;
            }
        }
        assert!(book.finished);
        assert!(!book.skip());
    }
}
