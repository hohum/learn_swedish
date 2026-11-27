# Swedish Keys — Module 2 Story XML Specification (v1.0)

This document defines the XML format read by the **Swedish Keys** Choose-Your-Own-Adventure reader (Module 2). Any UTF-8 `.xml` file conforming to this specification (and companion W3C XML Schema [`stories/story-v1.xsd`](../stories/story-v1.xsd)) can be opened directly in the app via **Open story…** in the adventure header bar, passed on the command line (`learn-swedish path/to/story.xml`), or placed at `stories/en-lagom-aventyrlig-lordag.xml` as the default book — **without recompiling the application**.

---

## 1. Document Encoding & Well-Formedness

- Files MUST be UTF-8 encoded, well-formed XML 1.0 documents beginning with an optional XML declaration (`<?xml version="1.0" encoding="UTF-8"?>`).
- Standard XML predefined entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`) and numeric character references (`&#...;`, `&#x...;`) are supported in text content and attribute values.
- XML comments (`<!-- ... -->`) are permitted anywhere outside tags and are ignored.
- Leading and trailing whitespace inside text elements (`<sv>`, `<lit>`, `<en>`, `<title>`, `<subtitle>`) is trimmed; internal whitespace runs are normalized to single spaces.

---

## 2. Root Element: `<story>`

```xml
<story version="1.0"
       id="en-lagom-aventyrlig-lordag"
       start="home-weather"
       outing-hub="home-plan">
```

### Attributes on `<story>`

| Attribute | Required | Description |
| --- | --- | --- |
| `version` | Yes | Format version. Must be `"1.0"`. |
| `id` | Optional | Machine-readable story identifier (non-empty ASCII/UTF-8 slug). |
| `xml:lang` | Optional | Language tag (typically `"sv"`). |
| `start` | Optional | ID of the starting `<page>`. Defaults to the first `<page>` in `<pages>`. |
| `outing-hub` | Optional | ID of a hub `<page>` (such as a destination chooser). When set, finishing the story shows **Ny utflykt · New outing** to jump directly to this page with newly drawn encounters; when omitted, finishing restarts from `start`. |

### Child Elements of `<story>` (in order or any order, at most once each)

1. `<meta>` *(required)* — window title and optional subtitle.
2. `<cast>` *(required)* — speaking roles and Azure Neural voice assignments.
3. `<ending>` *(optional)* — screen title and final spoken/displayed line when a choice transitions to `next="done"`.
4. `<pages>` *(required)* — the story's core pages.
5. `<encounters>` *(optional)* — randomized encounter stops and distractor pools spliced into routes on each playthrough.

---

## 3. Metadata: `<meta>`

```xml
<meta>
  <title>En lagom äventyrlig lördag</title>
  <subtitle>A Swedish choose-your-own-adventure</subtitle>
</meta>
```

- `<title>` *(required, non-empty)*: Displayed as the window header title.
- `<subtitle>` *(optional)*: Displayed below the header title (defaults to `"A Swedish choose-your-own-adventure"`).

---

## 4. Cast & Voice Synthesis: `<cast>`

`<cast>` maps actor IDs used throughout the story to visible Swedish speaker labels and Azure Neural voices (`mattias`, `sofie`, `hillevi`).

```xml
<cast protagonist="husband" responder="wife" narrator="narrator">
  <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
  <actor id="husband" label="Du" role="lead" voices="mattias"/>
  <actor id="wife" label="Din fru" role="lead" voices="sofie"/>
  <actor id="shop-assistant" label="Butiksmedarbetaren" role="guest" voices="sofie,hillevi"/>
  <actor id="hiker" label="Vandraren" role="guest" voices="mattias,sofie,hillevi"/>
</cast>
```

### Attributes on `<cast>`

| Attribute | Required | Description |
| --- | --- | --- |
| `narrator` | Optional | Default actor ID for `<title>` elements when `actor` is omitted. Defaults to `"narrator"` if defined, else the first actor in `<cast>`. |
| `protagonist` | Optional | Default actor ID for `<say>` elements inside `<choice>` / `<distractor>` / `<encounter>`. Defaults to `"protagonist"` or `"you"` if defined, else the first actor. |
| `responder` | Optional | Default actor ID for `<reply>` and `<question>` elements when `actor` is omitted. Defaults to `"responder"` if defined, else the second actor (or first if only one actor exists). |

### `<actor>` Elements

At least one `<actor>` MUST be defined, and all `id` values MUST be unique.

| Attribute | Required | Description |
| --- | --- | --- |
| `id` | Yes | Unique identifier referenced by `actor="..."` on lines, titles, questions, choices, and replies. |
| `label` | Yes | Display label shown before spoken lines in the transcript (e.g. `"Din fru"`, `"Du"`, `"Berättaren"`). |
| `role` | Optional | `"lead"` or `"guest"` (default `"guest"`). `"lead"` roles always speak at natural prosody (`pitch="0%"`, `rate="0%"`) using the first voice in `voices`. `"guest"` roles are dynamically cast per outing from `voices` (preferring least-used voices) with a consistent subtle pitch/rate preset so multiple characters remain audibly distinct. |
| `voices` | Yes | Comma-separated list of one or more supported Swedish neural voice short names (`mattias`, `sofie`, `hillevi`) or full Azure voice names (`sv-SE-MattiasNeural`, `sv-SE-SofieNeural`, `sv-SE-HilleviNeural`). |

---

## 5. Multilingual Text Blocks (`<sv>`, `<lit>`, `<en>`)

Every spoken or displayed story utterance (`<title>`, `<line>`, `<question>`, `<setup>`, `<say>`, `<reply>`) uses up to three child elements:

```xml
<line actor="wife">
  <sv>Det är åtta grader och det ska regna i eftermiddag.</sv>
  <lit>It is eight degrees and it shall rain this afternoon.</lit>
  <en>It is eight degrees and it is going to rain this afternoon.</en>
</line>
```

- `<sv>` *(required, non-empty)*: Swedish text spoken by TTS and displayed at all times.
- `<lit>` *(optional)*: Word-for-word literal English gloss shown in italics when **Show English hints** is enabled (for example, `rain-jacket-the` for `regnjackan`). If omitted, only `<sv>` and `<en>` are rendered.
- `<en>` *(required on dialogue, questions, `<say>`, and `<reply>`; optional on `<title>`)*: Natural idiomatic English translation shown when **Show English hints** is enabled.

---

## 6. Ending Block: `<ending>` (Optional)

Defines what is displayed when the reader follows a choice with `next="done"`.

```xml
<ending title="Hemma igen · Slut">
  <line actor="wife">
    <sv>Tack själv. Nästa gång får du välja ett nytt äventyr.</sv>
    <lit>Thanks yourself. Next time get you choose a new adventure.</lit>
    <en>Thank you too. Next time you can choose a new adventure.</en>
  </line>
</ending>
```

If `<ending>` is omitted, the reader uses `"Slut · The End"` and displays a default completion message.

---

## 7. Scenes (`scene` Attribute)

Each `<page>` and encounter `<stop>` specifies one of the built-in vector scene illustrations via `scene="..."`:

| Value | Illustration |
| --- | --- |
| `home` | Red Swedish house with yellow door and garden |
| `woods` | Pine forest and lake |
| `shop` | Supermarket storefront (`Willys` style) |
| `road` | Roadway with car and lane markings |
| `ikea` | Blue-and-yellow furniture store |
| `train` | Railway track and passenger train |
| `city` | City street / café |

---

## 8. Pages & Branching: `<pages>` and `<page>`

```xml
<pages choice-rotation="by-index">
  <page id="home-weather" scene="home">
    <title>
      <sv>En lagom äventyrlig lördag</sv>
      <lit>A just-right adventurous Saturday</lit>
      <en>A moderately adventurous Saturday</en>
    </title>
    <setup>
      <line actor="narrator">
        <sv>Det är lördag.</sv>
        <lit>It is Saturday.</lit>
        <en>It is Saturday.</en>
      </line>
    </setup>
    <question actor="wife">
      <sv>Vad behöver du ta på dig?</sv>
      <lit>What need you put on yourself?</lit>
      <en>What do you need to wear?</en>
    </question>
    <choices>
      <choice next="home-keys">
        <say>
          <sv>Jag tar en tröja och en regnjacka.</sv>
          <lit>I take a jumper and a rain-jacket.</lit>
          <en>I'll wear a jumper and a rain jacket.</en>
        </say>
        <reply actor="wife">
          <sv>Bra. Svenskt väder uppskattar en reservplan.</sv>
          <lit>Good. Swedish weather appreciates a backup-plan.</lit>
          <en>Good. Swedish weather appreciates a backup plan.</en>
        </reply>
      </choice>
      <choice>
        <say>
          <sv>Jag tar på mig ett paraply.</sv>
          <lit>I put on myself an umbrella.</lit>
          <en>I'll put on an umbrella.</en>
        </say>
        <reply actor="wife">
          <sv>Du kan ta med ett paraply.</sv>
          <lit>You can take along an umbrella.</lit>
          <en>You can bring an umbrella.</en>
        </reply>
      </choice>
    </choices>
  </page>
</pages>
```

### `<pages>` Attributes

- `choice-rotation` *(optional)*:
  - `"none"` *(default)*: Keeps `<choice>` elements in the exact order written in XML.
  - `"by-index"`: Rotates each page's choices left by `page_index mod choice_count` so correct answers authored first do not always appear as option `1`.
  - `"random"`: Randomly rotates choice order on each playthrough.

### `<page>` Structure & Rules

- `id` *(required)*: Unique page identifier. Reserved value `"done"` MUST NOT be used as a `<page id>`.
- `scene` *(required)*: One of `home`, `woods`, `shop`, `road`, `ikea`, `train`, `city`.
- `<title>` *(required)*: Page heading (spoken first when reading the page aloud; `actor` defaults to `cast/@narrator`).
- `<setup>` *(optional)*: Zero or more `<line actor="...">` elements setting the scene before the question.
- `<question>` *(required)*: The prompt asking the player how to respond (`actor` defaults to `cast/@responder`).
- `<choices>` *(required)*: Contains **2 to 4** `<choice>` elements:
  - Optional `next` attribute on `<choice>`:
    - `next="<page-id>"`: Valid choice that advances to `<page-id>` when **Continue** is clicked.
    - `next="done"`: Valid choice that completes the story when **Continue** is clicked.
    - Omitted `next`: Incorrect / humorous option; shows `<reply>` and prompts **Försök igen · Try again** on the same page.
  - `<say>` *(required)*: The player's spoken response (`actor` defaults to `cast/@protagonist`).
  - `<reply>` *(required)*: The companion/interlocutor's reply (`actor` defaults to `cast/@responder`).

---

## 9. Optional Randomized Encounters: `<encounters>`

Stories that want fresh replayability on each run can define `<encounters>`. On every new playthrough (and **New outing** / **Restart book**), each `<stop>` selects one `<encounter>` not shown in the immediately preceding run and combines its `<say>`/`<reply>` with wrong choices drawn without replacement from the route's `<distractors>` and shared `<distractors>`, splicing the generated 4-choice page between `after` and `before`.

```xml
<encounters>
  <distractors>
    <!-- Shared off-topic wrong choices usable at any stop -->
    <distractor>
      <say>
        <sv>Jag vill prata med kungen.</sv>
        <lit>I want speak with king-the.</lit>
        <en>I want to speak to the king.</en>
      </say>
      <reply>
        <sv>Han har nog också tagit helg.</sv>
        <lit>He has probably also taken weekend.</lit>
        <en>He's probably off for the weekend too.</en>
      </reply>
    </distractor>
  </distractors>

  <route id="woods">
    <distractors>
      <!-- Route-specific wrong choices (optional; 1 drawn per stop when available) -->
      <distractor>
        <say>
          <sv>Vi hyr en hiss upp i tallen.</sv>
          <lit>We rent a lift up into pine-the.</lit>
          <en>Let's rent a lift up the pine tree.</en>
        </say>
        <reply>
          <sv>Tallens porttelefon är också ur funktion.</sv>
          <lit>Pine-the’s door-phone is also out of function.</lit>
          <en>The pine's intercom is out of order too.</en>
        </reply>
      </distractor>
    </distractors>

    <stop after="woods-gate" before="woods-path" scene="woods">
      <title>
        <sv>Vid skogskanten</sv>
        <lit>By forest-edge-the</lit>
        <en>At the edge of the woods</en>
      </title>
      <question actor="wife">
        <sv>Vad säger du?</sv>
        <lit>What say you?</lit>
        <en>What do you say?</en>
      </question>
      <encounter id="woods-map">
        <setup actor="narrator">
          <sv>På kartan står du här, men kartan sitter upp och ner.</sv>
          <lit>On map-the stands you are here, but map-the sits up and down.</lit>
          <en>The map says You are here, but someone hung it upside down.</en>
        </setup>
        <say>
          <sv>Vi får vända på kartan i huvudet.</sv>
          <lit>We get turn on map-the in head-the.</lit>
          <en>We'll have to turn the map around in our heads.</en>
        </say>
        <reply actor="wife">
          <sv>För en gångs skull är det kartan som gått vilse.</sv>
          <lit>For one time’s sake is it map-the that gone astray.</lit>
          <en>For once it's the map that's got lost.</en>
        </reply>
      </encounter>
    </stop>
  </route>
</encounters>
```

---

## 10. Structural Validation Rules

When loading any XML file, the story engine validates all of the following before replacing the active story:

1. **Version & metadata:** `<story version="1.0">` and non-empty `<meta><title>`.
2. **Valid cast references:** Every explicit or defaulted `actor` ID on any line, title, question, choice, distractor, or encounter MUST exist in `<cast>`, and every voice in `voices` MUST be one of `mattias`, `sofie`, or `hillevi`.
3. **Unique IDs:** Every `<page id>` and `<encounter id>` MUST be non-empty, unique across the entire document, and not equal to `"done"`.
4. **Choice rules:** Every `<page>` MUST have 2 to 4 choices, at least one choice with `next` set, and distinct normalized `<say><sv>` strings (case-insensitive, ignoring trailing `.`, `!`, `?`) so typed Swedish answers are unambiguous.
5. **Graph integrity:**
   - `start` (and `outing-hub`, if specified) MUST reference an existing `<page id>`.
   - Every `next` target on `<choice>` MUST be either `"done"` or an existing `<page id>`.
   - Every reachable path from `start` MUST be acyclic (no infinite page loops) and able to reach `"done"`.
6. **Encounter pool integrity (when `<encounters>` is present):**
   - Every `<stop>` MUST reference an existing `<page id="after">` that has a choice with `next="before"`, contain at least 1 `<encounter>`, and the route + shared `<distractors>` pools MUST supply enough distractors (`3 × stops` total per route) without running dry.

---

## 11. Complete Minimal Example

Save the following as `fika.xml` and open it via **Open story…** (or `learn-swedish fika.xml`):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<story version="1.0" id="kort-fika" start="cafe-order">
  <meta>
    <title>En kort fika</title>
    <subtitle>A two-page custom story example</subtitle>
  </meta>
  <cast protagonist="you" responder="barista" narrator="narrator">
    <actor id="narrator" label="Berättaren" role="lead" voices="mattias"/>
    <actor id="you" label="Du" role="lead" voices="mattias"/>
    <actor id="barista" label="Kafébiträdet" role="guest" voices="sofie,hillevi"/>
  </cast>
  <ending title="Smaklig fika · Slut">
    <line actor="barista">
      <sv>Varsågod! Hoppas det smakar.</sv>
      <lit>There-you-go! Hope it tastes [good].</lit>
      <en>Here you go! Enjoy.</en>
    </line>
  </ending>
  <pages>
    <page id="cafe-order" scene="city">
      <title>
        <sv>Framme vid disken</sv>
        <lit>At-the-front by counter-the</lit>
        <en>At the counter</en>
      </title>
      <setup>
        <line actor="narrator">
          <sv>Det doftar nybakade kanelbullar i kaféet.</sv>
          <lit>It smells newly-baked cinnamon-buns in café-the.</lit>
          <en>The café smells of freshly baked cinnamon buns.</en>
        </line>
      </setup>
      <question actor="barista">
        <sv>Hej! Vad får det lov att vara?</sv>
        <lit>Hi! What may it permission to be?</lit>
        <en>Hi! What can I get for you?</en>
      </question>
      <choices>
        <choice next="done">
          <say>
            <sv>En kaffe och en kanelbulle, tack.</sv>
            <lit>One coffee and one cinnamon-bun, please.</lit>
            <en>A coffee and a cinnamon bun, please.</en>
          </say>
          <reply actor="barista">
            <sv>Det ordnar jag direkt.</sv>
            <lit>That arrange I directly.</lit>
            <en>I'll get that right away.</en>
          </reply>
        </choice>
        <choice>
          <say>
            <sv>En vinterjacka med mjölk, tack.</sv>
            <lit>One winter-jacket with milk, please.</lit>
            <en>A winter jacket with milk, please.</en>
          </say>
          <reply actor="barista">
            <sv>Vi säljer mest kaffe och bullar här.</sv>
            <lit>We sell mostly coffee and buns here.</lit>
            <en>We mostly sell coffee and buns here.</en>
          </reply>
        </choice>
      </choices>
    </page>
  </pages>
</story>
```
