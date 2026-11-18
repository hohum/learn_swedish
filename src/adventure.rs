use crate::lesson::{Phrase, Voice};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Narrator,
    Husband,
    Wife,
    ShopAssistant,
    Police,
    DogWalker,
    Conductor,
    BusDriver,
    CafeStaff,
    Passenger,
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
            Self::Conductor => "Konduktören",
            Self::BusDriver => "Busschauffören",
            Self::CafeStaff => "Kafébiträdet",
            Self::Passenger => "Medresenären",
        }
    }

    pub fn voice(self) -> Voice {
        match self {
            Self::Wife | Self::ShopAssistant | Self::Police | Self::BusDriver | Self::CafeStaff => {
                Voice::Sofie
            }
            Self::Narrator
            | Self::Husband
            | Self::DogWalker
            | Self::Conductor
            | Self::Passenger => Voice::Mattias,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub actor: Actor,
    pub swedish: &'static str,
    pub english: &'static str,
}

impl Line {
    pub fn phrase(self) -> Phrase {
        Phrase {
            swedish: self.swedish.into(),
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
    pub title: &'static str,
    pub setup: Vec<Line>,
    pub question: Line,
    pub choices: [Choice; 4],
}

impl Page {
    pub fn phrases(&self) -> Vec<Phrase> {
        std::iter::once(line(Actor::Narrator, self.title, "").phrase())
            .chain(self.setup.iter().map(|line| line.phrase()))
            .chain(std::iter::once(self.question.phrase()))
            .chain(self.choices.iter().map(|choice| choice.line.phrase()))
            .collect()
    }
}

fn line(actor: Actor, swedish: &'static str, english: &'static str) -> Line {
    Line {
        actor,
        swedish,
        english,
    }
}

fn n(sv: &'static str, en: &'static str) -> Line {
    line(Actor::Narrator, sv, en)
}

fn w(sv: &'static str, en: &'static str) -> Line {
    line(Actor::Wife, sv, en)
}

fn go(
    sv: &'static str,
    en: &'static str,
    next: &'static str,
    reply: &'static str,
    translation: &'static str,
) -> Choice {
    Choice {
        line: line(Actor::Husband, sv, en),
        reply: w(reply, translation),
        next: Some(next),
    }
}

fn no(
    sv: &'static str,
    en: &'static str,
    reply: &'static str,
    translation: &'static str,
) -> Choice {
    Choice {
        line: line(Actor::Husband, sv, en),
        reply: w(reply, translation),
        next: None,
    }
}

fn p(
    id: &'static str,
    scene: Scene,
    title: &'static str,
    setup: Vec<Line>,
    question: Line,
    choices: [Choice; 4],
) -> Page {
    Page {
        id,
        scene,
        title,
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
            "En lagom äventyrlig lördag",
            vec![
                n(
                    "Det är lördag. Du och din fru står i köket och planerar dagen.",
                    "It is Saturday. You and your wife are in the kitchen planning the day.",
                ),
                w(
                    "Du väljer utflykt i dag. Jag hjälper dig med svenskan, men du får prata.",
                    "You choose today's outing. I'll help with Swedish, but you do the talking.",
                ),
                w(
                    "Det är åtta grader och det ska regna i eftermiddag.",
                    "It is eight degrees and it is going to rain this afternoon.",
                ),
            ],
            w("Vad behöver du ta på dig?", "What do you need to wear?"),
            [
                go(
                    "Jag tar en tröja och en regnjacka.",
                    "I'll wear a jumper and a rain jacket.",
                    "home-keys",
                    "Bra. Svenskt väder uppskattar en reservplan.",
                    "Good. Swedish weather appreciates a backup plan.",
                ),
                no(
                    "Jag tar en tunn skjorta. Det är väl varmt?",
                    "I'll wear a thin shirt. It's warm, isn't it?",
                    "Åtta grader. Värmen verkar ha tagit ledigt.",
                    "Eight degrees. The warmth seems to have taken the day off.",
                ),
                no(
                    "Jag behöver ingen jacka. Jag går fort.",
                    "I don't need a jacket. I'll walk fast.",
                    "Regnet går också ganska fort.",
                    "The rain moves fairly quickly too.",
                ),
                no(
                    "Jag tar på mig ett paraply.",
                    "I'll put on an umbrella.",
                    "Du kan ta med ett paraply. Att ta på sig det blir ett annat äventyr.",
                    "You can bring an umbrella. Wearing it would be another adventure.",
                ),
            ],
        ),
        p(
            "home-keys",
            Home,
            "Nycklarna",
            vec![
                n(
                    "Jackan är på, men nyckelskålen är tom.",
                    "Your jacket is on, but the key bowl is empty.",
                ),
                w(
                    "Känn efter i fickan på jackan du hade i går.",
                    "Check the pocket of the jacket you wore yesterday.",
                ),
            ],
            w("Var är nycklarna?", "Where are the keys?"),
            [
                go(
                    "Här är de! De ligger i jackfickan.",
                    "Here they are! They're in the jacket pocket.",
                    "home-plan",
                    "Precis. Nycklar brukar trivas där man lämnade dem.",
                    "Exactly. Keys tend to like being where you left them.",
                ),
                no(
                    "De ligger säkert i skålen. Vi går nu.",
                    "They're surely in the bowl. Let's go now.",
                    "Skålen är tom. Den har redan blivit förhörd.",
                    "The bowl is empty. It's already been questioned.",
                ),
                no(
                    "Vi behöver inga nycklar när vi går ut.",
                    "We don't need keys when we go out.",
                    "Det stämmer. Det är när vi ska in igen som det blir intressant.",
                    "True. It gets interesting when we want to come back in.",
                ),
                no(
                    "Jag letar i kylskåpet först.",
                    "I'll look in the fridge first.",
                    "Gör det. Fråga mjölken om den har sett något.",
                    "Go ahead. Ask the milk whether it has seen anything.",
                ),
            ],
        ),
        p(
            "home-plan",
            Home,
            "Vart ska vi?",
            vec![
                n(
                    "Nycklarna är hittade. Telefonen är laddad och ytterdörren väntar.",
                    "The keys are found. Your phone is charged and the front door awaits.",
                ),
                w(
                    "Vi har gott om tid. Välj det du är sugen på.",
                    "We have plenty of time. Choose what you fancy.",
                ),
            ],
            w(
                "Vad vill du göra i dag?",
                "What would you like to do today?",
            ),
            [
                go(
                    "Jag vill ta en promenad i skogen.",
                    "I'd like to go for a walk in the woods.",
                    "woods-gate",
                    "Då tar vi med vatten och något att äta.",
                    "Then let's bring water and something to eat.",
                ),
                go(
                    "Vi går till Willys och handlar.",
                    "Let's walk to Willys and do some shopping.",
                    "shop-list",
                    "Bra idé. Tygkassen ligger vid dörren.",
                    "Good idea. The reusable bag is by the door.",
                ),
                go(
                    "Vi kör till IKEA och tittar på en hylla.",
                    "Let's drive to IKEA and look at a shelf.",
                    "road-calm",
                    "Då tar vi körkortet, måttbandet och lite tålamod.",
                    "Then let's bring your driving licence, the tape measure and a little patience.",
                ),
                go(
                    "Jag vill ta tåget till Göteborg.",
                    "I'd like to take the train to Gothenburg.",
                    "train-ticket",
                    "Då kollar vi biljetter och väder innan vi går.",
                    "Then let's check tickets and the weather before we leave.",
                ),
            ],
        ),
        p(
            "woods-gate",
            Woods,
            "Vid skogsbrynet",
            vec![
                n(
                    "Ni går hemifrån med vatten och smörgåsar. Vid skogen stannar din fru.",
                    "You leave home with water and sandwiches. At the woods your wife stops.",
                ),
                w(
                    "Det har regnat i natt. Stigen kan vara lerig.",
                    "It rained last night. The path might be muddy.",
                ),
            ],
            w("Vilka skor passar bäst?", "Which shoes are most suitable?"),
            [
                go(
                    "Jag tar mina vattentåliga promenadskor.",
                    "I'll wear my water-resistant walking shoes.",
                    "woods-path",
                    "Bra. Leran behöver inte följa med hem i strumporna.",
                    "Good. The mud doesn't need to come home in your socks.",
                ),
                no(
                    "Mina tofflor passar nog bra.",
                    "My slippers should be fine.",
                    "Ja, om skogen har börjat med inneskor.",
                    "Yes, if the forest has introduced indoor shoes.",
                ),
                no(
                    "Jag tar mina tunna tygskor.",
                    "I'll wear my thin canvas shoes.",
                    "Då får fötterna också uppleva vädret. Vi väljer något vattentåligt.",
                    "Then your feet get to experience the weather too. Let's choose something water-resistant.",
                ),
                no(
                    "Skorna kan vänta hemma.",
                    "The shoes can wait at home.",
                    "Omtänksamt. Men det är du som ska gå.",
                    "Thoughtful. But you're the one doing the walking.",
                ),
            ],
        ),
        p(
            "woods-path",
            Woods,
            "Två stigar",
            vec![
                n(
                    "Stigen delar sig. En skylt pekar mot sjön. Den andra vägen är avstängd.",
                    "The path forks. A sign points towards the lake. The other way is closed.",
                ),
                w(
                    "Avstängd betyder att vi inte ska gå där.",
                    "Closed means we shouldn't go that way.",
                ),
            ],
            w("Vilken väg tar vi?", "Which way shall we go?"),
            [
                go(
                    "Vi följer skylten mot sjön.",
                    "Let's follow the sign towards the lake.",
                    "woods-dog",
                    "Ja. Sjön har inte satt upp några protester.",
                    "Yes. The lake hasn't lodged any objections.",
                ),
                no(
                    "Vi tar den avstängda vägen.",
                    "Let's take the closed path.",
                    "Skylten verkar ha en annan uppfattning.",
                    "The sign seems to have a different opinion.",
                ),
                no(
                    "Avstängd betyder väl att vägen är öppen?",
                    "Closed means the path is open, doesn't it?",
                    "Nästan tvärtom. Skylten är inte särskilt förhandlingsvillig.",
                    "Almost the opposite. The sign isn't particularly willing to negotiate.",
                ),
                no(
                    "Vi väntar tills stigen bestämmer sig.",
                    "Let's wait until the path makes up its mind.",
                    "Då kan det bli en ganska lång lördag.",
                    "Then Saturday could be quite long.",
                ),
            ],
        ),
        p(
            "woods-dog",
            Woods,
            "En ny bekantskap",
            vec![
                n(
                    "En man kommer gående med en liten hund i koppel.",
                    "A man approaches with a small dog on a lead.",
                ),
                line(
                    DogWalker,
                    "God morgon! Vilken fin dag det blev ändå.",
                    "Good morning! It turned into a lovely day after all.",
                ),
            ],
            w("Hur svarar du vänligt?", "How do you reply politely?"),
            [
                go(
                    "God morgon! Ja, verkligen. Vilken fin hund!",
                    "Good morning! Yes, indeed. What a lovely dog!",
                    "woods-berries",
                    "Precis. Ett hej räcker långt här ute.",
                    "Exactly. A hello goes a long way out here.",
                ),
                no(
                    "God natt! Ja, verkligen.",
                    "Good night! Yes, indeed.",
                    "God morgon passar bättre. Vi har ju precis lämnat frukosten.",
                    "Good morning fits better. We've only just left breakfast behind.",
                ),
                no(
                    "Jag beställde faktiskt solsken.",
                    "I actually ordered sunshine.",
                    "Då får du reklamera hos väderprognosen.",
                    "Then you'll have to complain to the weather forecast.",
                ),
                no(
                    "Du står på min stig.",
                    "You're standing on my path.",
                    "Din stig? Då har jag missat ett ganska stort bostadsköp.",
                    "Your path? Then I've missed a rather large property purchase.",
                ),
            ],
        ),
        p(
            "woods-berries",
            Woods,
            "Bär vid stigen",
            vec![
                n(
                    "Du ser röda bär på en buske. Ingen av er vet vad det är för sort.",
                    "You spot red berries on a bush. Neither of you knows what kind they are.",
                ),
                w("Alla bär går inte att äta.", "Not all berries are edible."),
            ],
            w(
                "Vad gör vi med bär vi inte känner igen?",
                "What do we do with berries we don't recognise?",
            ),
            [
                go(
                    "Vi låter dem vara.",
                    "We leave them alone.",
                    "woods-toilet",
                    "Klokt. Färgen är ingen innehållsförteckning.",
                    "Wise. The colour isn't an ingredients list.",
                ),
                no(
                    "Jag smakar först och frågar sedan.",
                    "I'll taste first and ask afterwards.",
                    "Vi tar frågorna först. Magen behöver ingen överraskning.",
                    "Let's do the questions first. Your stomach doesn't need a surprise.",
                ),
                no(
                    "Rött betyder säkert jordgubbe.",
                    "Red must mean strawberry.",
                    "Det gör det inte ens på godishyllan.",
                    "Not even on the sweet shelf.",
                ),
                no(
                    "Jag litar på att fåglarna har kontrollerat dem.",
                    "I trust the birds have checked them.",
                    "Fåglarna har tyvärr ingen kundtjänst.",
                    "Unfortunately the birds don't have a customer service desk.",
                ),
            ],
        ),
        p(
            "woods-toilet",
            Woods,
            "En praktisk fråga",
            vec![
                n(
                    "Vid rastplatsen finns en liten servicebyggnad. Du behöver gå på toaletten.",
                    "There is a small service building at the picnic area. You need the toilet.",
                ),
                line(
                    DogWalker,
                    "Letar ni efter något?",
                    "Are you looking for something?",
                ),
            ],
            w(
                "Hur frågar du efter toaletten?",
                "How do you ask where the toilet is?",
            ),
            [
                go(
                    "Ursäkta, var finns toaletten?",
                    "Excuse me, where is the toilet?",
                    "woods-rain",
                    "Bra. Den ligger bakom byggnaden, säger han.",
                    "Good. It's behind the building, he says.",
                ),
                no(
                    "Ursäkta, när öppnar toaletten?",
                    "Excuse me, when does the toilet open?",
                    "Vi behöver veta var den är. Klockan visar inte vägen.",
                    "We need to know where it is. The clock doesn't show the way.",
                ),
                no(
                    "Jag vill köpa en toalett.",
                    "I'd like to buy a toilet.",
                    "Vi kan fråga, men hemtransporten blir besvärlig.",
                    "We can ask, but getting it home will be awkward.",
                ),
                no(
                    "Är sjön ledig?",
                    "Is the lake available?",
                    "Vi håller oss till toaletten.",
                    "Let's stick to the toilet.",
                ),
            ],
        ),
        p(
            "woods-rain",
            Woods,
            "Regnet kommer",
            vec![
                n(
                    "När ni har ätit era smörgåsar börjar det regna. Hemvägen är tydligt skyltad.",
                    "After you've eaten your sandwiches, it starts raining. The way home is clearly signposted.",
                ),
                w(
                    "Vi kan ta på oss regnjackorna och gå hem.",
                    "We can put on our rain jackets and walk home.",
                ),
            ],
            w("Vad säger du?", "What do you say?"),
            [
                go(
                    "Ja, vi går hem innan vi blir genomblöta.",
                    "Yes, let's go home before we're soaked.",
                    "home-return",
                    "Lagom äventyr. Nu längtar jag efter te.",
                    "Just enough adventure. Now I'm looking forward to tea.",
                ),
                no(
                    "Jag väntar här tills sommaren kommer.",
                    "I'll wait here until summer arrives.",
                    "Jag skickar en julhälsning om det dröjer.",
                    "I'll send a Christmas greeting if it takes a while.",
                ),
                no(
                    "Regnjackan får stanna i väskan så att den inte blir våt.",
                    "The rain jacket can stay in the bag so it doesn't get wet.",
                    "Det är snällt mot jackan, men den har faktiskt ett jobb.",
                    "That's kind to the jacket, but it does actually have a job.",
                ),
                no(
                    "Vi ber träden att stänga av regnet.",
                    "Let's ask the trees to turn off the rain.",
                    "De verkar redan vara upptagna med att bli blöta.",
                    "They already seem busy getting wet.",
                ),
            ],
        ),
        p(
            "shop-list",
            Shop,
            "På väg till Willys",
            vec![
                n(
                    "Ni går till Willys med tygkassen. Din fru läser inköpslistan.",
                    "You walk to Willys with the reusable bag. Your wife reads the shopping list.",
                ),
                w(
                    "Vi behöver mjölk, bröd, potatis och bananer.",
                    "We need milk, bread, potatoes and bananas.",
                ),
            ],
            w(
                "Vad ska vi köpa till frukosten?",
                "What shall we buy for breakfast?",
            ),
            [
                go(
                    "Vi köper mjölk, bröd och bananer.",
                    "Let's buy milk, bread and bananas.",
                    "shop-produce",
                    "Bra. Potatisen kan få vänta till middagen.",
                    "Good. The potatoes can wait until dinner.",
                ),
                no(
                    "Fyra diskborstar och en lök.",
                    "Four washing-up brushes and an onion.",
                    "Frukosten blir ren, men lite ensidig.",
                    "Breakfast will be clean, but a little limited.",
                ),
                no(
                    "Bara godis. Det går fort.",
                    "Just sweets. It's quick.",
                    "Sant. Och sedan blir förmiddagen väldigt lång.",
                    "True. And then the morning will be very long.",
                ),
                no(
                    "Vi köper själva inköpslistan.",
                    "Let's buy the shopping list itself.",
                    "Den är redan betald med min handstil.",
                    "It's already been paid for with my handwriting.",
                ),
            ],
        ),
        p(
            "shop-produce",
            Shop,
            "Bland frukt och grönt",
            vec![
                n(
                    "Bananerna kostar per kilo. Du vill bara ha några stycken.",
                    "The bananas are priced per kilo. You only want a few.",
                ),
                w(
                    "Vi behöver inte hela lådan.",
                    "We don't need the whole box.",
                ),
            ],
            w("Hur mycket tar du?", "How much do you take?"),
            [
                go(
                    "Jag tar fyra bananer.",
                    "I'll take four bananas.",
                    "shop-aisle",
                    "Utmärkt. Vi öppnar ingen djurpark i dag.",
                    "Excellent. We're not opening a zoo today.",
                ),
                no(
                    "Jag tar fyra kilo bananer.",
                    "I'll take four kilos of bananas.",
                    "Fyra stycken räcker. Kilo och stycken ger ganska olika frukostplaner.",
                    "Four individual bananas will do. Kilos and individual items make rather different breakfast plans.",
                ),
                no(
                    "Jag vill ha en liter bananer.",
                    "I'd like a litre of bananas.",
                    "Då får vi nog börja med en mixer.",
                    "Then we'll probably need to start with a blender.",
                ),
                no(
                    "Jag tar alla. De ser ensamma ut.",
                    "I'll take them all. They look lonely.",
                    "Fint tänkt. Men fruktskålen är inte så social.",
                    "A lovely thought. But the fruit bowl isn't that sociable.",
                ),
            ],
        ),
        p(
            "shop-aisle",
            Shop,
            "Var finns mjölken?",
            vec![
                n(
                    "Du hittar pastan, men ingen mjölk. En kvinnlig medarbetare fyller på en hylla.",
                    "You find pasta but no milk. A female employee is restocking a shelf.",
                ),
                line(
                    ShopAssistant,
                    "Hej! Behöver du hjälp?",
                    "Hi! Do you need help?",
                ),
            ],
            line(
                ShopAssistant,
                "Vad letar du efter?",
                "What are you looking for?",
            ),
            [
                go(
                    "Ursäkta, var finns mjölken?",
                    "Excuse me, where is the milk?",
                    "shop-milk",
                    "Bra frågat. Hon visar oss till mejeriavdelningen.",
                    "Well asked. She shows us to the dairy section.",
                ),
                no(
                    "Var bor korna?",
                    "Where do the cows live?",
                    "Lite tidigare i leveranskedjan.",
                    "A little earlier in the supply chain.",
                ),
                no(
                    "Jag letar efter utgången till kylskåpet.",
                    "I'm looking for the exit to the fridge.",
                    "Vi kan börja med mjölken och arbeta oss vidare.",
                    "We can start with the milk and work from there.",
                ),
                no(
                    "Ursäkta, vad kostar mjölken?",
                    "Excuse me, how much is the milk?",
                    "Priset kan vi fråga om sedan. Först måste vi hitta mjölken.",
                    "We can ask about the price later. First we need to find the milk.",
                ),
            ],
        ),
        p(
            "shop-milk",
            Shop,
            "Rätt förpackning",
            vec![
                n(
                    "Vid kylen pekar din fru på en förpackning.",
                    "At the fridge your wife points to a carton.",
                ),
                w(
                    "Vi brukar köpa en liter mellanmjölk.",
                    "We usually buy a litre of semi-skimmed milk.",
                ),
            ],
            w(
                "Vilken förpackning väljer du?",
                "Which carton do you choose?",
            ),
            [
                go(
                    "En liter mellanmjölk.",
                    "One litre of semi-skimmed milk.",
                    "shop-bread",
                    "Precis. Mjölkhyllan är inget personlighetstest.",
                    "Exactly. The milk shelf isn't a personality test.",
                ),
                no(
                    "En burk gräddfil.",
                    "A tub of sour cream.",
                    "Den är bra till annat. Kaffet kanske blir lite förvånat.",
                    "It's good for other things. The coffee might be a little surprised.",
                ),
                no(
                    "En liter vispgrädde.",
                    "A litre of whipping cream.",
                    "Det blir ett festligt kaffe, men vi skulle ha mellanmjölk.",
                    "That would make festive coffee, but we wanted semi-skimmed milk.",
                ),
                no(
                    "Jag tar hyllan. Då får vi alla sorter.",
                    "I'll take the shelf. Then we'll get every kind.",
                    "Kassen är rymlig, men inte kommunalt lager.",
                    "The bag is roomy, but it isn't a municipal warehouse.",
                ),
            ],
        ),
        p(
            "shop-bread",
            Shop,
            "Brödet är slut",
            vec![
                n(
                    "Det vanliga brödet är slut. Du frågar medarbetaren om det finns mer.",
                    "Your usual bread is sold out. You ask the employee whether there is more.",
                ),
                line(
                    ShopAssistant,
                    "Tyvärr, det kommer mer i morgon. Vill du prova det här?",
                    "Unfortunately, more arrives tomorrow. Would you like to try this one?",
                ),
            ],
            w(
                "Hur svarar du om ett annat bröd går bra?",
                "How do you reply if another bread is fine?",
            ),
            [
                go(
                    "Ja tack, det går bra med det här.",
                    "Yes please, this one will be fine.",
                    "shop-phone",
                    "Bra. Lördagen är räddad utan krismöte.",
                    "Good. Saturday is saved without an emergency meeting.",
                ),
                no(
                    "Då väntar jag här till i morgon.",
                    "Then I'll wait here until tomorrow.",
                    "Jag lämnar smörgåspåsen som sovsäck.",
                    "I'll leave the sandwich bag as a sleeping bag.",
                ),
                no(
                    "Kan du baka ett medan jag väntar?",
                    "Can you bake one while I wait?",
                    "Du överskattar nog vad som ingår i hyllpåfyllningen.",
                    "You're probably overestimating what's included in restocking shelves.",
                ),
                no(
                    "Jag tar en limpa potatis i stället.",
                    "I'll take a loaf of potatoes instead.",
                    "Det blir en ganska originell smörgås.",
                    "That will be quite an original sandwich.",
                ),
            ],
        ),
        p(
            "shop-phone",
            Shop,
            "Telefonen är borta",
            vec![
                n(
                    "Vid kassan hittar du inte telefonen. Du minns att du lade den bredvid bananerna.",
                    "At the checkout you can't find your phone. You remember putting it beside the bananas.",
                ),
                w("Vi frågar personalen först.", "Let's ask the staff first."),
            ],
            w("Vad säger du?", "What do you say?"),
            [
                go(
                    "Jag har tappat bort min telefon. Har någon lämnat in den?",
                    "I've lost my phone. Has anyone handed it in?",
                    "shop-checkout",
                    "Precis. Den ligger i kundservice. Bananerna klarade sig själva.",
                    "Exactly. It's at customer service. The bananas managed on their own.",
                ),
                no(
                    "Jag vill lämna tillbaka min telefon.",
                    "I'd like to return my phone.",
                    "Först måste vi hitta den. Tappat bort är något annat än lämna tillbaka.",
                    "First we need to find it. Lost is different from returning something.",
                ),
                no(
                    "Ge mig alla telefoner ni har.",
                    "Give me every phone you have.",
                    "Vi söker en telefon, inte ett nytt abonnemang på problem.",
                    "We're looking for one phone, not a new subscription to trouble.",
                ),
                no(
                    "Telefonen har nog flyttat till fruktavdelningen.",
                    "The phone has probably moved to the fruit section.",
                    "Då får vi hjälpa den att flytta hem igen.",
                    "Then we'll have to help it move home again.",
                ),
            ],
        ),
        p(
            "shop-checkout",
            Shop,
            "I kassan",
            vec![
                n(
                    "Telefonen är tillbaka. Du lägger varorna på bandet och visar tygkassen.",
                    "Your phone is back. You put the groceries on the belt and show your reusable bag.",
                ),
                line(ShopAssistant, "Behöver du en påse?", "Do you need a bag?"),
            ],
            w(
                "Du har redan en kasse. Vad svarar du?",
                "You already have a bag. What do you reply?",
            ),
            [
                go(
                    "Nej tack, jag har med mig en kasse.",
                    "No thank you, I've brought a bag.",
                    "shop-receipt",
                    "Bra. Den har väntat på sin stora stund.",
                    "Good. It's been waiting for its big moment.",
                ),
                no(
                    "Ja tack, jag behöver en ny kasse.",
                    "Yes please, I need a new bag.",
                    "Du kan säga så, men kassen du håller i handen ser lite besviken ut.",
                    "You can say that, but the bag in your hand looks a little disappointed.",
                ),
                no(
                    "Nej, jag bär mjölken i fickan.",
                    "No, I'll carry the milk in my pocket.",
                    "Det går bäst så länge förpackningen håller med.",
                    "That works best as long as the carton agrees.",
                ),
                no(
                    "Ingår en kundvagn att ta hem?",
                    "Is a trolley to take home included?",
                    "Bara om du också tänker flytta hit.",
                    "Only if you're also planning to move in.",
                ),
            ],
        ),
        p(
            "shop-receipt",
            Shop,
            "Kvitto och hemväg",
            vec![
                line(
                    ShopAssistant,
                    "Vill du ha kvittot?",
                    "Would you like the receipt?",
                ),
                n(
                    "Du vill kunna kontrollera köpet hemma.",
                    "You'd like to check the purchase at home.",
                ),
            ],
            w(
                "Vad säger du innan vi går hem?",
                "What do you say before we go home?",
            ),
            [
                go(
                    "Ja tack. Tack så mycket, hej då!",
                    "Yes please. Thank you very much, goodbye!",
                    "home-return",
                    "Precis. Nu går vi hem innan mjölken får egna planer.",
                    "Exactly. Now let's go home before the milk makes plans of its own.",
                ),
                no(
                    "Nej tack, jag behöver inget kvitto.",
                    "No thank you, I don't need a receipt.",
                    "Det är artigt. Men du ville ju kontrollera köpet hemma.",
                    "That's polite. But you did want to check the purchase at home.",
                ),
                no(
                    "Kan jag få kvittot på bananskal?",
                    "Can I have the receipt on a banana skin?",
                    "Det formatet stöds nog inte ännu.",
                    "That format probably isn't supported yet.",
                ),
                no(
                    "Jag tar kvittorullen.",
                    "I'll take the receipt roll.",
                    "Vi behöver bara vår egen lilla del av historien.",
                    "We only need our own little part of the story.",
                ),
            ],
        ),
        p(
            "road-calm",
            Road,
            "Bakom ratten",
            vec![
                n(
                    "Ni kör mot IKEA. En bil bakom tutar trots att ljuset är rött. Du blir irriterad.",
                    "You're driving towards IKEA. A car behind honks even though the light is red. You feel irritated.",
                ),
                w(
                    "Vi har ingen brådska. Håll avståndet och kör lugnt.",
                    "We're in no hurry. Keep your distance and drive calmly.",
                ),
            ],
            w("Vad säger du?", "What do you say?"),
            [
                go(
                    "Jag tar det lugnt och väntar på grönt.",
                    "I'll stay calm and wait for green.",
                    "road-police",
                    "Bra. Vi ska köpa en hylla, inte vinna en tävling.",
                    "Good. We're buying a shelf, not winning a race.",
                ),
                no(
                    "Jag tutar tillbaka tills det blir grönt.",
                    "I'll honk back until it turns green.",
                    "Trafikljuset tar tyvärr inte emot musikönskemål.",
                    "Unfortunately the traffic light doesn't take song requests.",
                ),
                no(
                    "Jag kör mot rött så slipper han vänta.",
                    "I'll drive through red so he doesn't have to wait.",
                    "Nej. Hans brådska ändrar inte trafikreglerna.",
                    "No. His hurry doesn't change the traffic rules.",
                ),
                no(
                    "Jag stannar här resten av dagen.",
                    "I'll stay here for the rest of the day.",
                    "Lugnt är bra. Att bosätta sig i korsningen är lite väl lugnt.",
                    "Calm is good. Moving into the junction is a little too calm.",
                ),
            ],
        ),
        p(
            "road-police",
            Road,
            "En poliskontroll",
            vec![
                n(
                    "En polis visar er till en säker kontrollplats. Du stannar enligt hennes anvisningar.",
                    "A police officer directs you to a safe checkpoint. You stop as she directs.",
                ),
                line(
                    Police,
                    "God dag. Kan jag få se ditt körkort?",
                    "Good afternoon. May I see your driving licence?",
                ),
            ],
            w("Vad svarar du?", "What do you reply?"),
            [
                go(
                    "Självklart. Här är mitt körkort.",
                    "Of course. Here is my driving licence.",
                    "ikea-parking",
                    "Bra. Kontrollen är snart klar, och vi kan köra vidare.",
                    "Good. The check is soon over, and we can continue.",
                ),
                no(
                    "Går det bra med IKEA-kortet?",
                    "Will my IKEA card do?",
                    "Det hjälper mer vid bokhyllor än vid vägkontroller.",
                    "It's more useful for bookshelves than roadside checks.",
                ),
                no(
                    "Här är mitt bankkort.",
                    "Here is my bank card.",
                    "Nästan rätt storlek på kortet. Men hon frågade efter körkortet.",
                    "Almost the right size of card. But she asked for your driving licence.",
                ),
                no(
                    "Körkortet körde nog hem.",
                    "The licence probably drove home.",
                    "Då får det förklara hur det nådde pedalerna.",
                    "Then it'll have to explain how it reached the pedals.",
                ),
            ],
        ),
        p(
            "ikea-parking",
            Ikea,
            "Här får du inte parkera",
            vec![
                n(
                    "Vid IKEA ser du en tom plats, men skylten säger lastzon.",
                    "At IKEA you see an empty space, but the sign says loading zone.",
                ),
                w(
                    "Här får vi inte parkera. Vi behöver en vanlig parkeringsplats.",
                    "We can't park here. We need an ordinary parking space.",
                ),
            ],
            w("Vad gör du?", "What do you do?"),
            [
                go(
                    "Jag letar efter en tillåten parkeringsplats.",
                    "I'll look for a space where parking is permitted.",
                    "ikea-entry",
                    "Precis. Tom plats betyder inte automatiskt ledig plats.",
                    "Exactly. An empty space doesn't automatically mean an available space.",
                ),
                no(
                    "Jag parkerar bara i en timme.",
                    "I'll only park for an hour.",
                    "Skylten verkar inte ha något undantag för ordet bara.",
                    "The sign doesn't seem to make an exception for the word only.",
                ),
                no(
                    "Jag lägger en servett över skylten.",
                    "I'll put a napkin over the sign.",
                    "Då har vi fortfarande parkerat fel, fast med dukning.",
                    "Then we're still parked incorrectly, but with table linen.",
                ),
                no(
                    "Lastzon betyder väl vanlig parkering?",
                    "Loading zone means ordinary parking, doesn't it?",
                    "Inte riktigt. Här ska varor kunna lastas, inte våra lördagsplaner.",
                    "Not quite. Goods need to be loaded here, not our Saturday plans.",
                ),
            ],
        ),
        p(
            "ikea-entry",
            Ikea,
            "En liten hylla",
            vec![
                n(
                    "Ni parkerar rätt och går in. Ni behöver en smal hylla till hallen.",
                    "You park correctly and go in. You need a narrow shelf for the hallway.",
                ),
                line(
                    ShopAssistant,
                    "Hej! Vad kan jag hjälpa er med?",
                    "Hi! What can I help you with?",
                ),
            ],
            w(
                "Hur förklarar du vad vi söker?",
                "How do you explain what we're looking for?",
            ),
            [
                go(
                    "Vi letar efter en smal hylla till hallen.",
                    "We're looking for a narrow shelf for the hallway.",
                    "ikea-measure",
                    "Bra. Hon visar oss en modell som kanske passar.",
                    "Good. She shows us a model that might fit.",
                ),
                no(
                    "Vi letar efter ett smalt bord till köket.",
                    "We're looking for a narrow table for the kitchen.",
                    "Smal var rätt. Möbeln och rummet tog en liten omväg.",
                    "Narrow was right. The furniture and the room took a little detour.",
                ),
                no(
                    "Jag vill ha en hylla till min hylla.",
                    "I'd like a shelf for my shelf.",
                    "Vi börjar med en. Sedan ser vi om den behöver sällskap.",
                    "Let's start with one. Then we'll see whether it needs company.",
                ),
                no(
                    "Vi behöver Sveriges största soffa.",
                    "We need Sweden's largest sofa.",
                    "Till vår hall? Då får brevbäraren gå genom fönstret.",
                    "For our hallway? Then the postman will have to use the window.",
                ),
            ],
        ),
        p(
            "ikea-measure",
            Ikea,
            "Måttbandet",
            vec![
                n(
                    "Hyllan är sextio centimeter bred. Utrymmet hemma är bara femtio centimeter.",
                    "The shelf is sixty centimetres wide. The space at home is only fifty.",
                ),
                w(
                    "Vi tog ju med måtten. Den här är för bred.",
                    "We did bring the measurements. This one is too wide.",
                ),
            ],
            w(
                "Vad säger du till medarbetaren?",
                "What do you say to the employee?",
            ),
            [
                go(
                    "Har ni en smalare hylla?",
                    "Do you have a narrower shelf?",
                    "ikea-warehouse",
                    "Precis. Hon hittar en modell som passar.",
                    "Exactly. She finds a model that fits.",
                ),
                no(
                    "Vi mäter igen tills den passar.",
                    "Let's measure again until it fits.",
                    "Måttbandet är ganska envist på den punkten.",
                    "The tape measure is rather stubborn on that point.",
                ),
                no(
                    "Sextio är väl mindre än femtio?",
                    "Sixty is less than fifty, isn't it?",
                    "Tvärtom. Siffrorna måste få plats innan hyllan får det.",
                    "The other way round. The numbers need to fit before the shelf will.",
                ),
                no(
                    "Vi behöver en bredare ytterdörr och en ny hall.",
                    "We need a wider front door and a new hallway.",
                    "Det blev snabbt en dyr hylla.",
                    "That quickly became an expensive shelf.",
                ),
            ],
        ),
        p(
            "ikea-warehouse",
            Ikea,
            "Gång och fack",
            vec![
                line(
                    ShopAssistant,
                    "Paketet finns på gång tjugofem, fack fyra.",
                    "The package is in aisle twenty-five, bay four.",
                ),
                w(
                    "Säg gärna numren igen så att vi minns dem.",
                    "Say the numbers again so we remember them.",
                ),
            ],
            w("Vart ska vi gå?", "Where should we go?"),
            [
                go(
                    "Till gång tjugofem, fack fyra.",
                    "To aisle twenty-five, bay four.",
                    "ikea-toilet",
                    "Rätt. Nu låter vi nästan som om vi arbetar här.",
                    "Correct. Now we almost sound as if we work here.",
                ),
                no(
                    "Till gång fyra, fack tjugofem.",
                    "To aisle four, bay twenty-five.",
                    "Samma siffror, en helt annan promenad.",
                    "The same numbers, a completely different walk.",
                ),
                no(
                    "Till fackföreningen.",
                    "To the trade union.",
                    "Viktig verksamhet, men hyllan står nog inte där.",
                    "Important work, but the shelf probably isn't there.",
                ),
                no(
                    "Till gång femton, fack fyra.",
                    "To aisle fifteen, bay four.",
                    "Femtio, femton, tjugofem. Siffrorna försöker ta en egen genväg. Vi ska till tjugofem.",
                    "Fifty, fifteen, twenty-five. The numbers are trying their own shortcut. We need twenty-five.",
                ),
            ],
        ),
        p(
            "ikea-toilet",
            Ikea,
            "En kort paus",
            vec![
                n(
                    "Ni har hittat paketet. Nu behöver du gå på toaletten innan ni betalar.",
                    "You've found the package. Now you need the toilet before you pay.",
                ),
                line(
                    ShopAssistant,
                    "Toaletterna ligger till vänster om restaurangen.",
                    "The toilets are to the left of the restaurant.",
                ),
            ],
            w("Var ligger toaletterna?", "Where are the toilets?"),
            [
                go(
                    "Till vänster om restaurangen.",
                    "To the left of the restaurant.",
                    "ikea-car",
                    "Ja. Jag väntar här med vagnen.",
                    "Yes. I'll wait here with the trolley.",
                ),
                no(
                    "Till vänster om parkeringen.",
                    "To the left of the car park.",
                    "Vänster var rätt, men restaurangen flyttade visst ut i översättningen.",
                    "Left was right, but the restaurant seems to have moved outside in the translation.",
                ),
                no(
                    "Till höger om parkeringen.",
                    "To the right of the car park.",
                    "Vänster och höger är tyvärr inte utbytbara här.",
                    "Unfortunately left and right aren't interchangeable here.",
                ),
                no(
                    "Vi måste bygga dem först.",
                    "We have to assemble them first.",
                    "Den insexnyckeln tänker jag inte leta efter.",
                    "I'm not going to look for that Allen key.",
                ),
            ],
        ),
        p(
            "ikea-car",
            Ikea,
            "Får paketet plats?",
            vec![
                n(
                    "Efter betalningen mäter ni paketet och bagageutrymmet. Paketet ryms när baksätet är fällt.",
                    "After paying you measure the package and the boot. It fits with the rear seat folded.",
                ),
                w(
                    "Vi följer bilens anvisningar och säkrar lasten innan vi kör.",
                    "We'll follow the car's instructions and secure the load before driving.",
                ),
            ],
            w(
                "Vad gör vi innan vi åker hem?",
                "What do we do before we drive home?",
            ),
            [
                go(
                    "Vi lastar ordentligt och säkrar paketet.",
                    "We'll load it properly and secure the package.",
                    "home-return",
                    "Bra. Hyllan behöver ingen egen rundtur i bilen.",
                    "Good. The shelf doesn't need its own tour of the car.",
                ),
                no(
                    "Jag håller paketet med ena handen och kör med den andra.",
                    "I'll hold the package with one hand and drive with the other.",
                    "Nej. Vi behöver en säker lösning, inte ett jongleringsnummer.",
                    "No. We need a safe solution, not a juggling act.",
                ),
                no(
                    "Vi lägger det löst över framsätena.",
                    "We'll lay it loose across the front seats.",
                    "En lös hylla är ingen bra medpassagerare.",
                    "A loose shelf isn't a good passenger.",
                ),
                no(
                    "Vi ber hyllan att spänna fast sig.",
                    "We'll ask the shelf to fasten its seat belt.",
                    "Den har knappt lärt sig att stå än.",
                    "It has barely learned to stand yet.",
                ),
            ],
        ),
        p(
            "train-ticket",
            Train,
            "Biljetter till Göteborg",
            vec![
                n(
                    "Ni går till stationen. I biljettappen väljer du Göteborg och två vuxna.",
                    "You walk to the station. In the ticket app you select Gothenburg and two adults.",
                ),
                w(
                    "Kontrollera datum och avgång innan du betalar.",
                    "Check the date and departure before paying.",
                ),
            ],
            w("Vilka biljetter behöver vi?", "Which tickets do we need?"),
            [
                go(
                    "Två vuxenbiljetter till Göteborg i dag.",
                    "Two adult tickets to Gothenburg today.",
                    "train-delay",
                    "Precis. Båda ska med, även om bara en av oss pratar mest.",
                    "Exactly. We're both going, even if only one of us does most of the talking.",
                ),
                no(
                    "Två barnbiljetter till Göteborg.",
                    "Two children's tickets to Gothenburg.",
                    "Jag uppskattar komplimangen. Men vi behöver vuxenbiljetter.",
                    "I appreciate the compliment. But we need adult tickets.",
                ),
                no(
                    "Två vuxenbiljetter till Göteborg i morgon.",
                    "Two adult tickets to Gothenburg tomorrow.",
                    "Nästan. Men lördagsutflykten vill gärna börja i dag.",
                    "Almost. But the Saturday outing would rather start today.",
                ),
                no(
                    "En biljett till varje sko.",
                    "One ticket for each shoe.",
                    "Då reser skorna bättre än vi.",
                    "Then the shoes will travel better than we do.",
                ),
            ],
        ),
        p(
            "train-delay",
            Train,
            "Försenat",
            vec![
                n(
                    "På tavlan står det att tåget är tjugo minuter försenat. En man bredvid er suckar.",
                    "The board says the train is twenty minutes late. A man beside you sighs.",
                ),
                line(
                    Passenger,
                    "Man hinner visst bli kaffesugen igen.",
                    "Apparently there's time to fancy another coffee.",
                ),
            ],
            w(
                "Vad betyder informationen på tavlan?",
                "What does the information on the board mean?",
            ),
            [
                go(
                    "Tåget kommer tjugo minuter senare.",
                    "The train will arrive twenty minutes later.",
                    "train-change",
                    "Ja. Vi håller koll på nya besked.",
                    "Yes. Let's watch for updates.",
                ),
                no(
                    "Resan tar bara tjugo minuter.",
                    "The journey only takes twenty minutes.",
                    "Det vore trevligt, men tavlan är inte riktigt så generös.",
                    "That would be nice, but the board isn't quite that generous.",
                ),
                no(
                    "Tåget avgår tjugo minuter tidigare.",
                    "The train leaves twenty minutes earlier.",
                    "Senare, inte tidigare. Järnvägen ger oss tid, den tar inte tillbaka gårdagen.",
                    "Later, not earlier. The railway is giving us time, not taking back yesterday.",
                ),
                no(
                    "Kaffet är tjugo minuter gratis.",
                    "The coffee is free for twenty minutes.",
                    "Det erbjudandet finns tyvärr bara i din översättning.",
                    "Unfortunately that offer only exists in your translation.",
                ),
            ],
        ),
        p(
            "train-change",
            Train,
            "Tåget är inställt",
            vec![
                n(
                    "Ett nytt besked kommer: tåget är inställt på grund av ett fordonsfel.",
                    "An update arrives: the train is cancelled because of a fault with the train.",
                ),
                line(
                    Conductor,
                    "En ersättningsbuss till Göteborg avgår från hållplats C. Biljetten gäller på bussen.",
                    "A replacement bus to Gothenburg leaves from stop C. Your ticket is valid on the bus.",
                ),
            ],
            w("Vad gör vi nu?", "What do we do now?"),
            [
                go(
                    "Vi går till hållplats C och tar ersättningsbussen.",
                    "Let's go to stop C and take the replacement bus.",
                    "train-bus",
                    "Bra. I dag får ordet tåg lite fler hjul än vanligt.",
                    "Good. Today the word train gets a few more wheels than usual.",
                ),
                no(
                    "Vi väntar inne i det inställda tåget.",
                    "We'll wait inside the cancelled train.",
                    "Först måste du hitta tåget som inte kommer.",
                    "First you'll have to find the train that isn't coming.",
                ),
                no(
                    "Vi köper en ny biljett utan att fråga.",
                    "Let's buy a new ticket without asking.",
                    "Den vi har gäller redan. Plånboken tackar för att vi lyssnade.",
                    "The one we have is already valid. Your wallet thanks us for listening.",
                ),
                no(
                    "Vi går till hållplats B.",
                    "Let's go to stop B.",
                    "C, inte B. Bara en bokstav bort, men bussen kan vara ganska noga.",
                    "C, not B. Only one letter away, but the bus can be rather particular.",
                ),
            ],
        ),
        p(
            "train-bus",
            Train,
            "Rätt buss",
            vec![
                n(
                    "Vid hållplats C står två bussar. Du vill kontrollera att ni kliver på rätt.",
                    "There are two buses at stop C. You want to check you're boarding the right one.",
                ),
                line(
                    BusDriver,
                    "Hej! Visa gärna biljetterna.",
                    "Hi! Please show your tickets.",
                ),
            ],
            w("Vad frågar du chauffören?", "What do you ask the driver?"),
            [
                go(
                    "Går den här bussen till Göteborg?",
                    "Does this bus go to Gothenburg?",
                    "train-arrival",
                    "Precis. Hon säger ja och kontrollerar biljetterna.",
                    "Exactly. She says yes and checks the tickets.",
                ),
                no(
                    "Kan du köra oss hem till köket?",
                    "Can you drive us home to the kitchen?",
                    "Vi har biljett till Göteborg, inte till kaffebryggaren.",
                    "We have tickets to Gothenburg, not to the coffee maker.",
                ),
                no(
                    "Är det här ett tåg med gummihjul?",
                    "Is this a train with rubber tyres?",
                    "Nästan rätt känsla, men fråga efter destinationen.",
                    "Almost the right idea, but ask about the destination.",
                ),
                no(
                    "Kommer bussen från Göteborg?",
                    "Does the bus come from Gothenburg?",
                    "Vi vill veta vart den går, inte hur dess morgon har varit.",
                    "We want to know where it's going, not how its morning has been.",
                ),
            ],
        ),
        p(
            "train-arrival",
            City,
            "Framme i Göteborg",
            vec![
                n(
                    "Bussen kommer fram till Göteborg. Ni går till ett kafé nära stationen.",
                    "The bus arrives in Gothenburg. You walk to a café near the station.",
                ),
                w(
                    "Det blev en annan resa än vi tänkte, men nu är vi här.",
                    "It wasn't the journey we planned, but now we're here.",
                ),
                line(
                    CafeStaff,
                    "Hej! Vad får det lov att vara?",
                    "Hi! What can I get you?",
                ),
            ],
            w("Beställ fika till oss båda.", "Order fika for both of us."),
            [
                go(
                    "Två kaffe och två kanelbullar, tack.",
                    "Two coffees and two cinnamon buns, please.",
                    "city-table",
                    "Bra. En fungerande svensk reservplan.",
                    "Good. A working Swedish backup plan.",
                ),
                no(
                    "En kaffe och en kanelbulle, tack.",
                    "One coffee and one cinnamon bun, please.",
                    "Jag är också här. Två kaffe och två bullar, om jag får rösta.",
                    "I'm here too. Two coffees and two buns, if I get a vote.",
                ),
                no(
                    "Jag vill ha Göteborg på en tallrik.",
                    "I'd like Gothenburg on a plate.",
                    "Vi börjar med en bulle. Den ryms bättre.",
                    "Let's start with a bun. It'll fit better.",
                ),
                no(
                    "En kanelbulle till kaffet och kaffe till kanelbullen och ingen kopp.",
                    "A cinnamon bun for the coffee and coffee for the bun and no cup.",
                    "Jag tror beställningen behöver en liten paus.",
                    "I think the order needs a little break.",
                ),
            ],
        ),
        p(
            "city-table",
            City,
            "Är det ledigt här?",
            vec![
                n(
                    "Kaféet är fullt. Vid ett bord sitter en man med två tomma stolar bredvid sig.",
                    "The café is busy. A man sits at a table with two empty chairs beside him.",
                ),
                w(
                    "Vi frågar innan vi sätter oss.",
                    "Let's ask before we sit down.",
                ),
            ],
            w("Vad säger du?", "What do you say?"),
            [
                go(
                    "Ursäkta, är de här platserna lediga?",
                    "Excuse me, are these seats free?",
                    "city-toilet",
                    "Ja, säger han. Ibland räcker en liten fråga.",
                    "Yes, he says. Sometimes a small question is enough.",
                ),
                no(
                    "Kan du flytta hem så att vi får plats?",
                    "Can you move home so there's room for us?",
                    "Vi behöver två stolar, inte hela hans eftermiddag.",
                    "We need two chairs, not his whole afternoon.",
                ),
                no(
                    "Jag bokade den här stolen i går i en dröm.",
                    "I booked this chair in a dream yesterday.",
                    "Tyvärr tar de nog inte emot drömbokningar.",
                    "Unfortunately they probably don't accept dream bookings.",
                ),
                no(
                    "Är de här platserna upptagna? Tack, då sätter vi oss.",
                    "Are these seats taken? Thanks, we'll sit down then.",
                    "Upptagna betyder att någon använder dem. Vi väntar på svaret först.",
                    "Taken means somebody is using them. Let's wait for the answer first.",
                ),
            ],
        ),
        p(
            "city-toilet",
            City,
            "En kod till dörren",
            vec![
                n(
                    "Efter kaffet behöver du toaletten. Dörren har ett kodlås.",
                    "After coffee you need the toilet. The door has a code lock.",
                ),
                line(
                    CafeStaff,
                    "Koden står längst ner på kvittot.",
                    "The code is at the very bottom of the receipt.",
                ),
            ],
            w("Var hittar du koden?", "Where do you find the code?"),
            [
                go(
                    "Längst ner på kvittot.",
                    "At the very bottom of the receipt.",
                    "train-home",
                    "Precis. Ännu ett skäl att spara kvittot.",
                    "Exactly. Another reason to keep the receipt.",
                ),
                no(
                    "På undersidan av kanelbullen.",
                    "On the underside of the cinnamon bun.",
                    "Den informationen har du redan ätit upp.",
                    "You've already eaten that information.",
                ),
                no(
                    "Högst upp på kvittot.",
                    "At the very top of the receipt.",
                    "Längst ner. Kvittot har tydligen fått både vind och källare.",
                    "At the bottom. Apparently the receipt has acquired both an attic and a cellar.",
                ),
                no(
                    "Jag frågar kaffekoppen om lösenordet.",
                    "I'll ask the coffee cup for the password.",
                    "Den är rätt tom på svar just nu.",
                    "It's rather empty of answers right now.",
                ),
            ],
        ),
        p(
            "train-home",
            Train,
            "Hemresan",
            vec![
                n(
                    "Ni promenerar tillbaka till stationen. Tåget ni tänkte ta har just gått.",
                    "You walk back to the station. The train you planned to catch has just left.",
                ),
                line(
                    Conductor,
                    "Nästa avgång går om en halvtimme. Kontrollera att era biljetter gäller.",
                    "The next departure is in half an hour. Check that your tickets are valid.",
                ),
            ],
            w(
                "Vad frågar du innan vi går till perrongen?",
                "What do you ask before we go to the platform?",
            ),
            [
                go(
                    "Gäller våra biljetter på nästa tåg?",
                    "Are our tickets valid on the next train?",
                    "home-return",
                    "Bra. Konduktören hjälper oss att ordna rätt biljetter och hitta rätt spår.",
                    "Good. The conductor helps us sort out the right tickets and find the right platform.",
                ),
                no(
                    "Kan du be tåget att backa?",
                    "Can you ask the train to reverse?",
                    "Det är nog enklare att vänta än att ändra hela järnvägen.",
                    "It's probably easier to wait than to rearrange the entire railway.",
                ),
                no(
                    "Kan vi sitta på tavlan tills nästa avgång?",
                    "Can we sit on the departure board until the next train?",
                    "Det finns bänkar med betydligt bättre sittkomfort.",
                    "There are benches with considerably better seating comfort.",
                ),
                no(
                    "Kan jag få tillbaka pengarna för kaffet?",
                    "Can I get a refund for the coffee?",
                    "En tydlig fråga. Men konduktören ansvarar inte för fikat.",
                    "A clear question. But the conductor isn't responsible for the fika.",
                ),
            ],
        ),
        p(
            "home-return",
            Home,
            "Hemma vid dörren",
            vec![
                n(
                    "Efter utflykten står ni vid ytterdörren igen. Du känner efter i fickorna.",
                    "After the outing you're at the front door again. You check your pockets.",
                ),
                w(
                    "Nycklarna lade du i jackfickan innan vi gick.",
                    "You put the keys in your jacket pocket before we left.",
                ),
            ],
            w(
                "Vad säger du när du hittar dem?",
                "What do you say when you find them?",
            ),
            [
                go(
                    "Här är nycklarna. Jag låser upp.",
                    "Here are the keys. I'll unlock the door.",
                    "home-evening",
                    "Skönt. Huset står kvar och vi kommer in också.",
                    "Lovely. The house is still here, and we can get inside too.",
                ),
                no(
                    "Jag tror att dörren behöver en biljett.",
                    "I think the door needs a ticket.",
                    "Den har redan en egen plats.",
                    "It already has a place of its own.",
                ),
                no(
                    "Vi väntar på nästa ersättningsdörr.",
                    "Let's wait for the next replacement door.",
                    "Den här verkar gå enligt tidtabell.",
                    "This one seems to be running on time.",
                ),
                no(
                    "Nycklar är nog bara en rekommendation.",
                    "Keys are probably just a recommendation.",
                    "Låset har ganska bestämda åsikter om det.",
                    "The lock has rather firm opinions about that.",
                ),
            ],
        ),
        p(
            "home-evening",
            Home,
            "Dagens sista fråga",
            vec![
                n(
                    "Ni är hemma igen. Jackorna hänger i hallen och vattenkokaren börjar susa.",
                    "You're home again. The jackets hang in the hallway and the kettle starts to hum.",
                ),
                w(
                    "Du klarade dig bra. Man behöver inte säga allt perfekt för att bli förstådd.",
                    "You did well. You don't need to say everything perfectly to be understood.",
                ),
            ],
            w(
                "Vad vill du säga om dagen?",
                "What would you like to say about the day?",
            ),
            [
                go(
                    "Det var en fin dag. Tack för hjälpen med svenskan.",
                    "It was a lovely day. Thanks for helping me with Swedish.",
                    "done",
                    "Tack själv. Nästa gång får du välja ett nytt äventyr.",
                    "Thank you too. Next time you can choose a new adventure.",
                ),
                no(
                    "Jag har lärt mig att aldrig lämna köket.",
                    "I've learned never to leave the kitchen.",
                    "Det vore synd. Vi har ju bara börjat.",
                    "That would be a shame. We've only just started.",
                ),
                no(
                    "Nu talar jag bättre svenska än alla svenskar.",
                    "Now I speak better Swedish than all Swedes.",
                    "Skönt. Då får du förklara skattedeklarationen nästa gång.",
                    "Lovely. Then you can explain the tax return next time.",
                ),
                no(
                    "Kan vi köpa en ny lördag med kvittot?",
                    "Can we buy a new Saturday with the receipt?",
                    "Vi får en till nästa vecka. Utan kvitto.",
                    "We'll get another next week. No receipt needed.",
                ),
            ],
        ),
    ];
    for (index, page) in pages.iter_mut().enumerate() {
        page.choices.rotate_left(index % 4);
    }
    pages
}

pub fn speech_phrases() -> Vec<Phrase> {
    let mut phrases = Vec::new();
    for page in pages() {
        phrases.extend(page.phrases());
        phrases.extend(page.choices.iter().map(|choice| choice.reply.phrase()));
    }
    phrases.sort_by(|a, b| (a.voice, &a.swedish).cmp(&(b.voice, &b.swedish)));
    phrases.dedup_by(|a, b| a.voice == b.voice && a.swedish == b.swedish);
    phrases
}

pub struct Book {
    pub pages: Vec<Page>,
    current: usize,
    pub selected: Option<usize>,
    pub finished: bool,
}

impl Default for Book {
    fn default() -> Self {
        Self {
            pages: pages(),
            current: 0,
            selected: None,
            finished: false,
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn every_outing_is_reachable_and_every_route_returns_home_without_cycles() {
        let pages = pages();
        let by_id: HashMap<_, _> = pages.iter().map(|p| (p.id, p)).collect();
        assert_eq!(by_id.len(), pages.len());
        fn walk<'a>(
            id: &'a str,
            pages: &HashMap<&str, &'a Page>,
            path: &mut Vec<&'a str>,
            seen: &mut HashSet<&'a str>,
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
        let mut seen = HashSet::new();
        walk("home-weather", &by_id, &mut Vec::new(), &mut seen);
        assert_eq!(seen.len(), pages.len());
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
    fn all_story_audio_has_a_role_appropriate_voice_and_unique_safe_filename() {
        let phrases = speech_phrases();
        let keys: HashSet<_> = phrases.iter().map(|p| (p.voice, p.audio_stem())).collect();
        assert_eq!(keys.len(), phrases.len());
        for phrase in &phrases {
            assert!(phrase.voice.is_some() && phrase.speaker.is_some());
            assert_eq!(phrase.audio_stem().len(), 16);
        }
        assert_eq!(Actor::Wife.voice(), Voice::Sofie);
        assert_eq!(Actor::Husband.voice(), Voice::Mattias);
        assert_eq!(Actor::Police.voice(), Voice::Sofie);
        assert_eq!(Actor::Conductor.voice(), Voice::Mattias);
    }
}
