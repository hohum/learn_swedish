use super::{Actor, Choice, Page, Scene, go, line, no, p, w};
#[cfg(test)]
use crate::lesson::Phrase;

type Text = [&'static str; 3];

struct Stop {
    after: &'static str,
    before: &'static str,
    scene: Scene,
    title: Text,
    question: Text,
    encounters: &'static str,
}

struct Route {
    stops: [Stop; 3],
    distractors: &'static [Choice],
}

const QUESTION: Text = ["Vad säger du?", "What say you?", "What do you say?"];

const ROUTES: &[Route] = &[
    Route {
        distractors: WOODS_DISTRACTORS,
        stops: [
            Stop {
                after: "woods-gate",
                before: "woods-path",
                scene: Scene::Woods,
                title: [
                    "Vid skogskanten",
                    "By forest-edge-the",
                    "At the edge of the woods",
                ],
                question: QUESTION,
                encounters: include_str!("woods/edge.txt"),
            },
            Stop {
                after: "woods-path",
                before: "woods-dog",
                scene: Scene::Woods,
                title: ["Nere vid sjön", "Down by lake-the", "Down by the lake"],
                question: QUESTION,
                encounters: include_str!("woods/lake.txt"),
            },
            Stop {
                after: "woods-toilet",
                before: "woods-rain",
                scene: Scene::Woods,
                title: ["Vid rastplatsen", "At rest-place-the", "At the picnic spot"],
                question: QUESTION,
                encounters: include_str!("woods/picnic.txt"),
            },
        ],
    },
    Route {
        distractors: SHOP_DISTRACTORS,
        stops: [
            Stop {
                after: "shop-list",
                before: "shop-produce",
                scene: Scene::Shop,
                title: [
                    "Något mer på listan",
                    "Something more on list-the",
                    "One more thing on the list",
                ],
                question: [
                    "Vad lägger du i korgen?",
                    "What put you in basket-the?",
                    "What do you put in the basket?",
                ],
                encounters: include_str!("willys/produce.txt"),
            },
            Stop {
                after: "shop-milk",
                before: "shop-bread",
                scene: Scene::Shop,
                title: ["Mellan hyllorna", "Between shelves-the", "In the aisles"],
                question: QUESTION,
                encounters: include_str!("willys/aisles.txt"),
            },
            Stop {
                after: "shop-bread",
                before: "shop-phone",
                scene: Scene::Shop,
                title: ["Innan vi betalar", "Before we pay", "Before we pay"],
                question: QUESTION,
                encounters: include_str!("willys/basket.txt"),
            },
        ],
    },
    Route {
        distractors: IKEA_DISTRACTORS,
        stops: [
            Stop {
                after: "road-police",
                before: "ikea-parking",
                scene: Scene::Road,
                title: ["På väg till IKEA", "On way to IKEA", "On the way to IKEA"],
                question: QUESTION,
                encounters: include_str!("ikea/road.txt"),
            },
            Stop {
                after: "ikea-measure",
                before: "ikea-warehouse",
                scene: Scene::Ikea,
                title: ["I varuhuset", "In goods-house-the", "In the store"],
                question: QUESTION,
                encounters: include_str!("ikea/showroom.txt"),
            },
            Stop {
                after: "ikea-toilet",
                before: "ikea-car",
                scene: Scene::Ikea,
                title: ["På väg ut", "On way out", "On the way out"],
                question: QUESTION,
                encounters: include_str!("ikea/checkout.txt"),
            },
        ],
    },
    Route {
        distractors: TRAIN_DISTRACTORS,
        stops: [
            Stop {
                after: "train-ticket",
                before: "train-delay",
                scene: Scene::Train,
                title: [
                    "I väntan på tåget",
                    "In waiting-the for train-the",
                    "While we wait for the train",
                ],
                question: QUESTION,
                encounters: include_str!("gothenburg/station.txt"),
            },
            Stop {
                after: "train-bus",
                before: "train-arrival",
                scene: Scene::Train,
                title: ["Mot Göteborg", "Toward Gothenburg", "Heading to Gothenburg"],
                question: QUESTION,
                encounters: include_str!("gothenburg/bus.txt"),
            },
            Stop {
                after: "city-toilet",
                before: "train-home",
                scene: Scene::City,
                title: [
                    "En sväng på stan",
                    "A turn in town-the",
                    "A wander around town",
                ],
                question: QUESTION,
                encounters: include_str!("gothenburg/city.txt"),
            },
        ],
    },
];

impl Stop {
    // Each encounter has an ID, a setup with a speaker, an answer, and a reply.
    // Every spoken line lists Swedish, literal English, then natural English.
    fn records(&self) -> impl Iterator<Item = [&'static str; 4]> {
        let mut lines = self.encounters.lines().filter(|line| !line.is_empty());
        std::iter::from_fn(move || {
            let id = lines.next()?;
            Some([
                id,
                lines.next().expect(id),
                lines.next().expect(id),
                lines.next().expect(id),
            ])
        })
    }

    fn page(&self, [id, setup, answer, reply]: [&'static str; 4], [a, b, c]: [Choice; 3]) -> Page {
        let (role, setup) = setup
            .split_once('|')
            .expect("Encounter setup starts with a speaker");
        let actor = match role {
            "n" => Actor::Narrator,
            "w" => Actor::Wife,
            "shop" => Actor::ShopAssistant,
            "walk" => Actor::Hiker,
            "dog" => Actor::DogWalker,
            "station" => Actor::StationStaff,
            "bus" => Actor::BusDriver,
            "cafe" => Actor::CafeStaff,
            "p" => Actor::Passenger,
            "street" => Actor::Passerby,
            _ => panic!("Unknown speaker in {id}: {role}"),
        };
        let choices = [go(triple(answer), self.before, triple(reply)), a, b, c];
        p(
            id,
            self.scene,
            self.title,
            vec![line(actor, triple(setup))],
            w(self.question),
            choices,
        )
    }
}

fn triple(text: &'static str) -> Text {
    let mut parts = text.split('|');
    let result = std::array::from_fn(|_| {
        parts
            .next()
            .expect("Expected Swedish, literal English, and natural English")
    });
    assert!(
        parts.next().is_none() && result.iter().all(|part| !part.is_empty()),
        "Invalid translation: {text}"
    );
    result
}

pub(super) fn insert(
    pages: &mut Vec<Page>,
    previous: &[Page],
    choose: &mut impl FnMut(usize) -> usize,
) {
    for route in ROUTES {
        // Draw without replacement across each outing's three encounters.
        let mut local = route.distractors.to_vec();
        let mut general = DISTRACTORS.to_vec();
        for stop in &route.stops {
            let records: Vec<_> = stop
                .records()
                .filter(|record| !previous.iter().any(|page| page.id == record[0]))
                .collect();
            let record = records[choose(records.len())];
            let choices = [
                local.swap_remove(choose(local.len())),
                general.swap_remove(choose(general.len())),
                general.swap_remove(choose(general.len())),
            ];
            let mut page = stop.page(record, choices);
            page.choices.rotate_left(choose(4));
            let link = pages
                .iter_mut()
                .find(|page| page.id == stop.after)
                .and_then(|page| {
                    page.choices
                        .iter_mut()
                        .find(|choice| choice.next == Some(stop.before))
                })
                .expect("Encounter must be inserted between two linked pages");
            link.next = Some(page.id);
            pages.push(page);
        }
    }
}

#[cfg(test)]
pub(super) fn all_pages() -> impl Iterator<Item = Page> {
    ROUTES
        .iter()
        .flat_map(|route| &route.stops)
        .flat_map(|stop| {
            stop.records()
                .map(|record| stop.page(record, [DISTRACTORS[0], DISTRACTORS[1], DISTRACTORS[2]]))
        })
}

#[cfg(test)]
pub(super) fn distractor_phrases() -> impl Iterator<Item = Phrase> {
    std::iter::once(&DISTRACTORS[..])
        .chain(ROUTES.iter().map(|route| route.distractors))
        .flatten()
        .flat_map(|choice| [choice.line.phrase(), choice.reply.phrase()])
}

const WOODS_DISTRACTORS: &[Choice] = &[
    no(
        [
            "Vi hyr en hiss upp i tallen.",
            "We rent a lift up into pine-the.",
            "Let's rent a lift up the pine tree.",
        ],
        [
            "Tallens porttelefon är också ur funktion.",
            "Pine-the’s door-phone is also out of function.",
            "The pine's intercom is out of order too.",
        ],
    ),
    no(
        [
            "Jag frågar en ekorre om lösenordet.",
            "I ask a squirrel about password-the.",
            "I'll ask a squirrel for the password.",
        ],
        [
            "Den byter det varje gång den gömmer en nöt.",
            "It changes it each time it hides a nut.",
            "It changes it every time it hides a nut.",
        ],
    ),
    no(
        [
            "Jag beställer snö till picknicken.",
            "I order snow for picnic-the.",
            "I'll order snow for the picnic.",
        ],
        [
            "Avbeställ gärna innan den kommer.",
            "Cancel gladly before it comes.",
            "Please cancel that before it arrives.",
        ],
    ),
    no(
        [
            "Finns det hemleverans av hela sjön?",
            "Exists there home-delivery of whole lake-the?",
            "Can we get the whole lake delivered?",
        ],
        [
            "Vi behöver först en större hall.",
            "We need first a bigger hallway.",
            "We'll need a bigger hallway first.",
        ],
    ),
    no(
        [
            "Vi skriver en insändare till mossan.",
            "We write a submitted-letter to moss-the.",
            "Let's write a letter to the moss.",
        ],
        [
            "Mossan läser bara på söndagar.",
            "Moss-the reads only on Sundays.",
            "The moss only reads on Sundays.",
        ],
    ),
    no(
        [
            "Jag vill ha kartan stickad i ull.",
            "I want have map-the knitted in wool.",
            "I want the map knitted in wool.",
        ],
        [
            "Varmt, men svårt att vika ihop.",
            "Warm, but hard to fold together.",
            "Warm, but hard to fold up.",
        ],
    ),
];

const SHOP_DISTRACTORS: &[Choice] = &[
    no(
        [
            "Ett flak läsk till frukost, tack.",
            "One tray soda for breakfast, please.",
            "A case of soft drinks for breakfast, please.",
        ],
        [
            "Jag tänkte mig en frukost man kan tugga.",
            "I imagined myself a breakfast one can chew.",
            "I was picturing a breakfast you can chew.",
        ],
    ),
    no(
        [
            "Kan vi köpa en pall florsocker?",
            "Can we buy one pallet icing-sugar?",
            "Can we buy a pallet of icing sugar?",
        ],
        [
            "Bara om huset följer med på köpet.",
            "Only if house-the comes along with purchase-the.",
            "Only if it comes with a house.",
        ],
    ),
    no(
        [
            "En meter sylt, tack.",
            "One metre jam, please.",
            "A metre of jam, please.",
        ],
        [
            "Jag tror de räknar i burkar här.",
            "I think they count in jars here.",
            "I think they count by the jar here.",
        ],
    ),
    no(
        [
            "Har ni extrapris på tomma kartonger?",
            "Have you [plural] special-price on empty cartons?",
            "Are empty cartons on special offer?",
        ],
        [
            "Vi behöver fortfarande något att äta.",
            "We need still something to eat.",
            "We still need something to eat.",
        ],
    ),
    no(
        [
            "Jag tar en säck isbitar till soppan.",
            "I take a sack ice-cubes for soup-the.",
            "I'll take a sack of ice cubes for the soup.",
        ],
        [
            "Det blir kall gazpacho med väldigt lite smak.",
            "It becomes cold gazpacho with very little taste.",
            "That'll be cold gazpacho with very little flavour.",
        ],
    ),
    no(
        [
            "Vi köper kundvagnen och hyr ut den.",
            "We buy customer-trolley-the and rent out it.",
            "Let's buy the trolley and rent it out.",
        ],
        [
            "Vi får börja med att fylla den själva.",
            "We get begin with filling it ourselves.",
            "Let's start by filling it ourselves.",
        ],
    ),
];

const IKEA_DISTRACTORS: &[Choice] = &[
    no(
        [
            "Kan vi beställa en rondell till hallen?",
            "Can we order a roundabout for hallway-the?",
            "Can we order a roundabout for the hallway?",
        ],
        [
            "Då får vi gå tre varv för att hitta skorna.",
            "Then get we go three laps to find shoes-the.",
            "Then we'd have to do three laps to find our shoes.",
        ],
    ),
    no(
        [
            "Jag parkerar i en bokhylla.",
            "I park in a book-shelf.",
            "I'll park in a bookcase.",
        ],
        [
            "Bilen är en aning för bred.",
            "Car-the is a hint too wide.",
            "The car's a touch too wide.",
        ],
    ),
    no(
        [
            "Vi monterar hyllan på månen.",
            "We assemble shelf-the on moon-the.",
            "Let's assemble the shelf on the moon.",
        ],
        [
            "Det är långt till kundservice därifrån.",
            "It is far to customer-service from-there.",
            "Customer service is a long way from there.",
        ],
    ),
    no(
        [
            "Jag vill ha en soffa till bagageutrymmet.",
            "I want have a sofa for luggage-space-the.",
            "I'd like a sofa for the boot.",
        ],
        [
            "Bagageutrymmet är ingen gäststuga.",
            "Luggage-space-the is no guest-cottage.",
            "The boot isn't a guest room.",
        ],
    ),
    no(
        [
            "Jag köper bara skruven längst till vänster.",
            "I buy only screw-the farthest to left.",
            "I'll only buy the screw on the far left.",
        ],
        [
            "En skruv gör ingen hallmöbel.",
            "One screw makes no hallway-furniture-piece.",
            "One screw doesn't make a piece of hallway furniture.",
        ],
    ),
    no(
        [
            "Jag döper vagnen till Vägvisaren.",
            "I name trolley-the Guide-the.",
            "I'm naming the trolley The Guide.",
        ],
        [
            "Den får inte bestämma vart vi ska.",
            "It may not decide to where we shall.",
            "It doesn't get to decide where we go.",
        ],
    ),
];

const TRAIN_DISTRACTORS: &[Choice] = &[
    no(
        [
            "Jag köper en enkelbiljett till havregrynen.",
            "I buy a single-ticket for oat-flakes-the.",
            "I'll buy a single ticket for the oats.",
        ],
        [
            "Gröten stannar hemma den här gången.",
            "Porridge-the stays home this here time-the.",
            "The porridge is staying home this time.",
        ],
    ),
    no(
        [
            "Kan vi boka en perrong att ha hemma?",
            "Can we book a platform to have at home?",
            "Can we reserve a platform to keep at home?",
        ],
        [
            "Vi får börja med en hallmatta.",
            "We get begin with a hallway-rug.",
            "Let's start with a hallway rug.",
        ],
    ),
    no(
        [
            "Jag tar spårvagnen till måndag.",
            "I take tram-the to Monday.",
            "I'll take the tram to Monday.",
        ],
        [
            "Den linjen går bara i kalendern.",
            "That line-the goes only in calendar-the.",
            "That line only runs in the calendar.",
        ],
    ),
    no(
        [
            "Jag skickar en faktura till motvinden.",
            "I send an invoice to headwind-the.",
            "I'll send the headwind an invoice.",
        ],
        [
            "Vinden betalar alltid för sent.",
            "Wind-the pays always too late.",
            "The wind always pays late.",
        ],
    ),
    no(
        [
            "Jag vill ha fika med platsbiljett.",
            "I want have fika with seat-ticket.",
            "I want my fika with a seat reservation.",
        ],
        [
            "Kanelbullen sitter bra på fatet.",
            "Cinnamon-bun-the sits well on saucer-the.",
            "The cinnamon bun's fine on its plate.",
        ],
    ),
    no(
        [
            "Vi tar med hela stationsklockan hem.",
            "We take along whole station-clock-the home.",
            "Let's take the whole station clock home.",
        ],
        [
            "Då kommer vi aldrig loss från perrongen.",
            "Then come we never loose from platform-the.",
            "Then we'll never get off the platform.",
        ],
    ),
];

// Deliberately off-topic suggestions with their own responses, so they can fit
// different encounters without referring to an item or person who isn't there.
const DISTRACTORS: [Choice; 15] = [
    no(
        [
            "Jag vill prata med kungen.",
            "I want speak with king-the.",
            "I want to speak to the king.",
        ],
        [
            "Han har nog också tagit helg.",
            "He has probably also taken weekend.",
            "He's probably off for the weekend too.",
        ],
    ),
    no(
        [
            "Kan vi spola tillbaka hela dagen?",
            "Can we rewind back whole day-the?",
            "Can we rewind the whole day?",
        ],
        [
            "Du får en chans till här och nu.",
            "You get one chance more here and now.",
            "You get another chance right here, right now.",
        ],
    ),
    no(
        [
            "Jag svarar med en trumvirvel.",
            "I answer with a drum-roll.",
            "I'll answer with a drumroll.",
        ],
        [
            "Snyggt intro. Nu kommer orden.",
            "Nice intro. Now come words-the.",
            "Nice intro. Now for the words.",
        ],
    ),
    no(
        [
            "Jag har bokat en ubåt.",
            "I have booked a submarine.",
            "I've booked a submarine.",
        ],
        [
            "Den detaljen stod inte i kalendern.",
            "That detail-the stood not in calendar-the.",
            "That detail wasn't on the calendar.",
        ],
    ),
    no(
        [
            "Jag byter namn till Bengt.",
            "I change name to Bengt.",
            "I'm changing my name to Bengt.",
        ],
        [
            "Trevligt att träffas. Vad säger Bengt då?",
            "Nice to meet [each other]. What says Bengt then?",
            "Nice to meet you. So what does Bengt say?",
        ],
    ),
    no(
        [
            "Ska jag ringa en cirkus?",
            "Shall I call a circus?",
            "Should I call a circus?",
        ],
        [
            "Vi klarar oss utan akrobater den här gången.",
            "We manage ourselves without acrobats this here time-the.",
            "We'll manage without acrobats this time.",
        ],
    ),
    no(
        [
            "Jag vill tacka min manager.",
            "I want thank my manager.",
            "I'd like to thank my manager.",
        ],
        [
            "Talet kan du spara till prisutdelningen.",
            "Speech-the can you save for prize-giving-the.",
            "You can save the speech for the awards ceremony.",
        ],
    ),
    no(
        [
            "Jag känner mig som en försenad falukorv.",
            "I feel myself like a delayed Falu-sausage.",
            "I feel like a sausage running late.",
        ],
        [
            "Det var oväntat specifikt.",
            "That was unexpectedly specific.",
            "That was unexpectedly specific.",
        ],
    ),
    no(
        [
            "Jag svarar i morgon klockan sju.",
            "I answer tomorrow clock-the seven.",
            "I'll answer tomorrow at seven.",
        ],
        [
            "Så länge har vi inte på oss.",
            "So long have we not on us.",
            "We don't have that much time.",
        ],
    ),
    no(
        [
            "Jag lägger in mitt veto mot lördagar.",
            "I lay in my veto against Saturdays.",
            "I'm vetoing Saturdays.",
        ],
        [
            "Då blir veckan ännu tråkigare.",
            "Then becomes week-the even more boring.",
            "That'll make the week even more boring.",
        ],
    ),
    no(
        [
            "Kan jag svara med en röksignal?",
            "Can I answer with a smoke-signal?",
            "Can I answer with a smoke signal?",
        ],
        [
            "Vi börjar med svenskan i dag.",
            "We begin with Swedish-the today.",
            "Let's start with Swedish today.",
        ],
    ),
    no(
        [
            "Jag har glömt hur man använder ord.",
            "I have forgotten how one uses words.",
            "I've forgotten how to use words.",
        ],
        [
            "Konstigt. De där fungerade utmärkt.",
            "Strange. Those there worked excellently.",
            "Funny. Those worked perfectly.",
        ],
    ),
    no(
        [
            "Allt är månens fel.",
            "Everything is moon-the’s fault.",
            "It's all the moon's fault.",
        ],
        [
            "Månen får yttra sig senare.",
            "Moon-the gets express itself later.",
            "The moon can give its side of the story later.",
        ],
    ),
    no(
        [
            "Jag beställer en applåd.",
            "I order an applause.",
            "I'll order a round of applause.",
        ],
        [
            "Vi tar den efter rätt svar.",
            "We take it after right answer.",
            "Let's save it for after the right answer.",
        ],
    ),
    no(
        [
            "Vet du var min riddarrustning är?",
            "Know you where my knight-armour is?",
            "Do you know where my suit of armour is?",
        ],
        [
            "Den får du leta efter när vi kommer hem.",
            "It get you look for when we come home.",
            "You can look for that when we get home.",
        ],
    ),
];
