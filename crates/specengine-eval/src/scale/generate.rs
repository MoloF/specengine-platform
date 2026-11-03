//! The seeded synthetic corpus of `scale` (`docs/features/scale-eval.md`
//! "Acceptance criteria", SE-1 to SE-4): a pure function of the seed and
//! the size, written afterwards.
//!
//! [`generate`] builds every file in memory from two xorshift64\* streams
//! seeded through splitmix64, so seed 0 works too: the READMEs from the
//! seed alone, so that one seed gives them byte-equal at every size, and
//! the part files and the samples from the seed and the size. No clock,
//! no hash-map order and no environment enter it: one seed and one size
//! give one byte sequence. [`write`] puts the files under a directory.
//!
//! The text is a closed vocabulary: syllable words (one of `j k q v x z`,
//! then one of `a e i o u y`), `P-` IDs, area and group names, one date,
//! digit runs and the config's and the front-matter's keys. It names no
//! domain and no project.

use std::collections::BTreeSet;
use std::fs;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_store::B3Stream;

/// `specengine.toml` of every tree, byte for byte (SE-2).
const CONFIG: &str = "[project]
slug = \"synthetic\"

[paths]
roots = [\"README.md\", \"areas\"]
tier0 = \"README.md\"
tier1_name = \"README.md\"

[ids]
P = { kind = \"node\", width = 6 }
";
/// The config's file name at the tree's root.
const CONFIG_FILE: &str = "specengine.toml";
/// The Tier 0 file (`[paths] tier0`).
const TIER0_FILE: &str = "README.md";
/// Every Tier 1 file's name (`[paths] tier1_name`).
const TIER1_NAME: &str = "README.md";
/// The directory of the areas (a `[paths]` root).
const AREAS_DIR: &str = "areas";
/// Areas, each with its Tier 1 README.
const AREAS: usize = 16;
/// The documents of the first four part files.
const HUBS: usize = 4;
/// Files at most in one group directory.
const GROUP_FILES: usize = 100;
/// Sections of every part file but the last.
const SECTIONS: RangeInclusive<usize> = 2..=6;
/// `depends_on` items of a document, and inline mentions of a section.
const LINKS: RangeInclusive<usize> = 0..=2;
/// Incoming links at most of a non-hub node.
const IN_DEGREE_MAX: u32 = 16;
/// Redraws of a draw landing on a full node before it is dropped.
const REDRAWS: usize = 8;
/// One part document in this many is shipped (Tier 3).
const SHIPPED_ONE_IN: usize = 5;
/// The part files that each name one hub in turn.
const HUB_TURNS: RangeInclusive<usize> = 5..=8;
/// After [`HUB_TURNS`], one part file in this many names a hub.
const HUB_NAMED_ONE_IN: usize = 10;
/// Sections holding one rare word.
const RARE_SECTIONS: usize = 8;
/// Syllables of a rare word.
const RARE_SYLLABLES: usize = 5;
/// Syllables of a common word.
const COMMON_SYLLABLES: RangeInclusive<usize> = 1..=3;
/// Words of a title.
const TITLE_WORDS: RangeInclusive<usize> = 2..=4;
/// Words of a document's paragraph.
const DOCUMENT_WORDS: RangeInclusive<usize> = 20..=60;
/// Words of a section's paragraph.
const SECTION_WORDS: RangeInclusive<usize> = 15..=50;
/// Words of a sentence.
const SENTENCE_WORDS: RangeInclusive<usize> = 6..=14;
/// Words of the Tier 0 paragraph.
const TIER0_WORDS: RangeInclusive<usize> = 40..=80;
/// Words of a Tier 1 paragraph.
const TIER1_WORDS: RangeInclusive<usize> = 60..=120;
/// A syllable's first letter.
const CONSONANTS: &[u8; 6] = b"jkqvxz";
/// A syllable's second letter.
const VOWELS: &[u8; 6] = b"aeiouy";
/// Syllables in all: the digits of a rare word's code.
const SYLLABLES: usize = CONSONANTS.len() * VOWELS.len();
/// Every date the tree holds (`reviewed`, `shipped`).
const DATE: &str = "2026-01-01";
/// The W targets of a size.
const W_TARGETS: usize = 20;
/// The search queries of a size.
const QUERIES: usize = 3;

/// One size's tree, in memory.
pub struct Synthetic {
    /// Every file: its root-relative `/` path and bytes, in path byte order.
    pub files: Vec<(String, Vec<u8>)>,
    pub corpus: CorpusCounts,
    pub samples: Samples,
    /// Bytes of the Tier 0 README.
    pub tier0_bytes: usize,
    /// Bytes of each area's Tier 1 README, by area.
    pub tier1_bytes: Vec<usize>,
}

/// What a tree holds (`corpus` of the result). `files` and `bytes` count
/// its Markdown documents, as the index walks them (the config is not one).
#[derive(Debug, Clone, Serialize)]
pub struct CorpusCounts {
    pub files: usize,
    /// The part nodes plus the Tier 0 and the 16 Tier 1 documents.
    pub nodes: usize,
    pub bytes: usize,
    /// `depends_on` items and inline mentions, the hubs' included.
    pub links: usize,
    /// Shipped part files.
    pub tier3_files: usize,
    /// The most incoming links of a non-hub node.
    pub degree_max: u32,
    pub hubs: usize,
    pub hub_degree: MinMax,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MinMax {
    pub min: u32,
    pub max: u32,
}

/// What the size's stream draws after the tree.
pub struct Samples {
    /// Live non-hub part nodes, without replacement; the first three are
    /// the timed targets.
    pub targets: Vec<Target>,
    /// Rare words, each held by at least one live section.
    pub queries: Vec<String>,
    /// The hub documents in file order; the first is the measured hub.
    pub hubs: Vec<Target>,
    /// Each hub's incoming links, in the order of [`Self::hubs`].
    pub hub_degrees: Vec<u32>,
}

/// A node read by ID, and the area whose Tier 1 its W_after counts.
pub struct Target {
    pub id: String,
    pub area: usize,
}

impl Synthetic {
    /// `b3:` over each file's path, NUL, bytes, NUL, in path byte order.
    pub fn digest(&self) -> String {
        let mut stream = B3Stream::new();
        for (path, bytes) in &self.files {
            stream.update(path.as_bytes());
            stream.update(&[0]);
            stream.update(bytes);
            stream.update(&[0]);
        }
        stream.finish()
    }
}

/// Writes every file of `synthetic` under `tree`, the directories created
/// as needed. An error names the file or directory.
pub fn write(synthetic: &Synthetic, tree: &Path) -> Result<(), String> {
    let mut made: BTreeSet<PathBuf> = BTreeSet::new();
    for (path, bytes) in &synthetic.files {
        let file = path
            .split('/')
            .fold(tree.to_path_buf(), |dir, part| dir.join(part));
        if let Some(parent) = file.parent()
            && !made.contains(parent)
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            made.insert(parent.to_path_buf());
        }
        fs::write(&file, bytes)
            .map_err(|error| format!("cannot write {}: {error}", file.display()))?;
    }
    Ok(())
}

/// The tree of `nodes` part nodes for `seed`, with its samples.
pub fn generate(seed: u64, nodes: usize) -> Synthetic {
    let readmes = Readmes::draw(&mut Rng::seeded(&[seed]));
    let mut rng = Rng::seeded(&[seed, nodes as u64]);
    let plan = Plan::lay_out(&mut rng, nodes);
    let links = Links::draw(&mut rng, &plan);

    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(plan.files.len() + AREAS + 2);
    files.push((CONFIG_FILE.to_owned(), CONFIG.as_bytes().to_vec()));
    let tier0_bytes = readmes.tier0.len();
    let tier1_bytes: Vec<usize> = readmes.tier1.iter().map(String::len).collect();
    files.push((TIER0_FILE.to_owned(), readmes.tier0.into_bytes()));
    for (area, text) in readmes.tier1.into_iter().enumerate() {
        files.push((tier1_path(area), text.into_bytes()));
    }
    let mut sections = 0;
    let mut rare_live: Vec<usize> = Vec::new();
    for (k, file) in plan.files.iter().enumerate() {
        let text = part_file(&mut rng, &plan, &links, k, &mut sections, &mut rare_live);
        files.push((part_path(k, file.first), text.into_bytes()));
    }
    let samples = Samples::draw(&mut rng, &plan, &links, &rare_live);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let non_hub = (0..nodes).filter(|&node| !plan.is_hub(node));
    let hub_degree = MinMax {
        min: samples.hub_degrees.iter().copied().min().unwrap_or(0),
        max: samples.hub_degrees.iter().copied().max().unwrap_or(0),
    };
    let corpus = CorpusCounts {
        files: files.len() - 1,
        nodes: nodes + 1 + AREAS,
        bytes: files
            .iter()
            .filter(|(path, _)| path != CONFIG_FILE)
            .map(|(_, bytes)| bytes.len())
            .sum(),
        links: links.count(),
        tier3_files: plan.files.iter().filter(|file| file.shipped).count(),
        degree_max: non_hub.map(|node| links.incoming[node]).max().unwrap_or(0),
        hubs: HUBS,
        hub_degree,
    };
    Synthetic {
        files,
        corpus,
        samples,
        tier0_bytes,
        tier1_bytes,
    }
}

// ---------------------------------------------------------------------------
// The streams.

/// xorshift64\* (the pattern of the core's and the CLI's random corpora),
/// its state seeded through splitmix64.
struct Rng(u64);

impl Rng {
    /// The stream of `words`, folded through splitmix64; never the zero
    /// state, which xorshift cannot leave.
    fn seeded(words: &[u64]) -> Self {
        let mut state = GOLDEN;
        for &word in words {
            state = splitmix64(state ^ word);
        }
        Self(if state == 0 { GOLDEN } else { state })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `0..n`; 0 when `n` is 0.
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        // The remainder is below `n`, so it fits.
        (self.next() % n as u64) as usize
    }

    /// Uniform in `range`.
    fn within(&mut self, range: &RangeInclusive<usize>) -> usize {
        range.start() + self.below(range.end() - range.start() + 1)
    }
}

/// The golden-ratio increment of splitmix64.
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(GOLDEN);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

// ---------------------------------------------------------------------------
// The layout and the links.

/// One part file: the node index of its document, its sections after it.
struct FilePlan {
    first: usize,
    sections: usize,
    shipped: bool,
}

/// Every part file, and each node's file.
struct Plan {
    files: Vec<FilePlan>,
    /// By node index (the ID's number minus one).
    file_of: Vec<usize>,
}

impl Plan {
    /// Files of one document and 2..=6 sections until `nodes` are placed,
    /// the last taking what remains; then the status of every non-hub file.
    fn lay_out(rng: &mut Rng, nodes: usize) -> Self {
        let mut files = Vec::new();
        let mut file_of = Vec::with_capacity(nodes);
        while file_of.len() < nodes {
            let first = file_of.len();
            let count = (1 + rng.within(&SECTIONS)).min(nodes - first);
            file_of.extend(std::iter::repeat_n(files.len(), count));
            files.push(FilePlan {
                first,
                sections: count - 1,
                shipped: false,
            });
        }
        for file in files.iter_mut().skip(HUBS) {
            file.shipped = rng.below(SHIPPED_ONE_IN) == 0;
        }
        Self { files, file_of }
    }

    fn is_hub(&self, node: usize) -> bool {
        self.files.iter().take(HUBS).any(|file| file.first == node)
    }

    fn is_live(&self, node: usize) -> bool {
        !self.files[self.file_of[node]].shipped
    }
}

/// The tree's links, all drawn before any text.
struct Links {
    /// By file: its document's `depends_on`, earlier non-hub documents.
    depends: Vec<Vec<usize>>,
    /// By node: the nodes its section paragraph mentions (none for a
    /// document).
    mentions: Vec<Vec<usize>>,
    /// By file: the hub its document paragraph names.
    hub_named: Vec<Option<usize>>,
    /// By node: links landing on it.
    incoming: Vec<u32>,
}

impl Links {
    fn draw(rng: &mut Rng, plan: &Plan) -> Self {
        let nodes = plan.file_of.len();
        let mut incoming = vec![0_u32; nodes];

        let mut depends = Vec::with_capacity(plan.files.len());
        for k in 0..plan.files.len() {
            let count = rng.within(&LINKS);
            // The non-hub files before this one: `HUBS..k`.
            let earlier = k.saturating_sub(HUBS);
            let mut list: Vec<usize> = Vec::new();
            for _ in 0..count {
                if earlier == 0 {
                    break;
                }
                let drawn = draw_capped(
                    rng,
                    &incoming,
                    |rng| plan.files[HUBS + rng.below(earlier)].first,
                    |node| list.contains(&node),
                );
                if let Some(node) = drawn {
                    incoming[node] += 1;
                    list.push(node);
                }
            }
            depends.push(list);
        }

        let non_hub: Vec<usize> = (0..nodes).filter(|&node| !plan.is_hub(node)).collect();
        let mut mentions: Vec<Vec<usize>> = vec![Vec::new(); nodes];
        for file in &plan.files {
            let first_section = file.first + 1;
            let slots = &mut mentions[first_section..first_section + file.sections];
            for (offset, slot) in slots.iter_mut().enumerate() {
                let section = first_section + offset;
                let count = rng.within(&LINKS);
                let mut list: Vec<usize> = Vec::new();
                for _ in 0..count {
                    let drawn = draw_capped(
                        rng,
                        &incoming,
                        |rng| non_hub[rng.below(non_hub.len())],
                        |node| node == section || list.contains(&node),
                    );
                    if let Some(node) = drawn {
                        incoming[node] += 1;
                        list.push(node);
                    }
                }
                *slot = list;
            }
        }

        let mut hub_named = Vec::with_capacity(plan.files.len());
        for k in 0..plan.files.len() {
            let hub = if HUB_TURNS.contains(&k) {
                Some(k - HUB_TURNS.start())
            } else if k > *HUB_TURNS.end() && rng.below(HUB_NAMED_ONE_IN) == 0 {
                Some(rng.below(HUBS))
            } else {
                None
            };
            if let Some(hub) = hub {
                incoming[plan.files[hub].first] += 1;
            }
            hub_named.push(hub);
        }
        Self {
            depends,
            mentions,
            hub_named,
            incoming,
        }
    }

    fn count(&self) -> usize {
        self.depends.iter().map(Vec::len).sum::<usize>()
            + self.mentions.iter().map(Vec::len).sum::<usize>()
            + self.hub_named.iter().flatten().count()
    }
}

/// One link's end: `pick` drawn again, up to [`REDRAWS`] times, while it
/// lands on a full node or on one `taken` refuses (the node itself, or one
/// already in the same list); then dropped.
fn draw_capped(
    rng: &mut Rng,
    incoming: &[u32],
    mut pick: impl FnMut(&mut Rng) -> usize,
    taken: impl Fn(usize) -> bool,
) -> Option<usize> {
    for _ in 0..=REDRAWS {
        let node = pick(rng);
        if incoming[node] < IN_DEGREE_MAX && !taken(node) {
            return Some(node);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The samples.

impl Samples {
    fn draw(rng: &mut Rng, plan: &Plan, links: &Links, rare_live: &[usize]) -> Self {
        let nodes = plan.file_of.len();
        let live_non_hub: Vec<usize> = (0..nodes)
            .filter(|&node| !plan.is_hub(node) && plan.is_live(node))
            .collect();
        let targets = draw_distinct(rng, &live_non_hub, W_TARGETS)
            .into_iter()
            .map(|node| target(plan, node))
            .collect();
        let queries = draw_distinct(rng, rare_live, QUERIES)
            .into_iter()
            .map(rare_word)
            .collect();
        let hub_nodes: Vec<usize> = plan.files.iter().take(HUBS).map(|f| f.first).collect();
        Self {
            targets,
            queries,
            hubs: hub_nodes.iter().map(|&node| target(plan, node)).collect(),
            hub_degrees: hub_nodes.iter().map(|&node| links.incoming[node]).collect(),
        }
    }
}

fn target(plan: &Plan, node: usize) -> Target {
    Target {
        id: id_of(node),
        area: plan.file_of[node] % AREAS,
    }
}

/// `count` of `pool`, without replacement (all of it when shorter).
fn draw_distinct(rng: &mut Rng, pool: &[usize], count: usize) -> Vec<usize> {
    let mut pool = pool.to_vec();
    let mut drawn = Vec::with_capacity(count.min(pool.len()));
    while drawn.len() < count && !pool.is_empty() {
        let at = rng.below(pool.len());
        drawn.push(pool.swap_remove(at));
    }
    drawn
}

// ---------------------------------------------------------------------------
// The text.

/// `P-` and the six-digit number of node index `node`.
fn id_of(node: usize) -> String {
    format!("P-{:06}", node + 1)
}

fn tier1_path(area: usize) -> String {
    format!("{AREAS_DIR}/a{area:02}/{TIER1_NAME}")
}

/// Part file `k`: area `k mod 16`, group `floor(k / 16) / 100`, named by
/// its document's ID.
fn part_path(k: usize, first: usize) -> String {
    let area = k % AREAS;
    let group = k / AREAS / GROUP_FILES;
    format!("{AREAS_DIR}/a{area:02}/g{group:04}/{}.md", id_of(first))
}

/// The READMEs, drawn from the seed's own stream.
struct Readmes {
    tier0: String,
    tier1: Vec<String>,
}

impl Readmes {
    fn draw(rng: &mut Rng) -> Self {
        let title0 = title(rng);
        let paragraph0 = paragraph(rng, &TIER0_WORDS, &[]);
        let titles: Vec<String> = (0..AREAS).map(|_| title(rng)).collect();
        let mut tier0 = format!(
            "---\nclass: canon\ntier: 0\nowner: owner\nreviewed: {DATE}\n---\n\n# {title0}\n\n{paragraph0}\n\n"
        );
        for (area, title) in titles.iter().enumerate() {
            tier0.push_str(&format!("- [{title}]({})\n", tier1_path(area)));
        }
        let tier1 = titles
            .iter()
            .enumerate()
            .map(|(area, title)| {
                let paragraph = paragraph(rng, &TIER1_WORDS, &[]);
                format!(
                    "---\nclass: canon\ntier: 1\nowner: owner\nreviewed: {DATE}\nscope: [a{area:02}]\n---\n\n# {title}\n\n{paragraph}\n"
                )
            })
            .collect();
        Self { tier0, tier1 }
    }
}

/// Part file `k`: its front-matter, its document's H1 and paragraph (the
/// hub it names), then each section's heading and paragraph (its mentions
/// and its rare word). `sections` counts the sections written so far, in
/// ID order; `rare_live` gathers each rare word a live section holds.
fn part_file(
    rng: &mut Rng,
    plan: &Plan,
    links: &Links,
    k: usize,
    sections: &mut usize,
    rare_live: &mut Vec<usize>,
) -> String {
    let file = &plan.files[k];
    let mut text = format!("---\nclass: spec\nid: {}\n", id_of(file.first));
    if file.shipped {
        text.push_str(&format!("status: shipped\nshipped: {DATE}\n"));
    } else {
        text.push_str("status: draft\n");
    }
    text.push_str(&format!("scope: [a{:02}]\n", k % AREAS));
    if !links.depends[k].is_empty() {
        let ids: Vec<String> = links.depends[k].iter().map(|&node| id_of(node)).collect();
        text.push_str(&format!("links:\n  depends_on: [{}]\n", ids.join(", ")));
    }
    let heading = title(rng);
    let hub: Vec<String> = links.hub_named[k]
        .map(|hub| id_of(plan.files[hub].first))
        .into_iter()
        .collect();
    let body = paragraph(rng, &DOCUMENT_WORDS, &hub);
    text.push_str(&format!("---\n\n# {heading}\n\n{body}\n"));
    for section in file.first + 1..=file.first + file.sections {
        let rare = *sections / RARE_SECTIONS;
        *sections += 1;
        if !file.shipped && rare_live.last() != Some(&rare) {
            rare_live.push(rare);
        }
        let mut specials: Vec<String> = links.mentions[section]
            .iter()
            .map(|&node| id_of(node))
            .collect();
        specials.push(rare_word(rare));
        let heading = title(rng);
        let body = paragraph(rng, &SECTION_WORDS, &specials);
        text.push_str(&format!(
            "\n## {heading} {{#{}}}\n\n{body}\n",
            id_of(section)
        ));
    }
    text
}

/// One syllable, appended.
fn syllable(rng: &mut Rng, word: &mut String) {
    let digit = rng.below(SYLLABLES);
    push_syllable(digit, word);
}

/// Syllable `digit` (0..36): consonant `digit / 6`, vowel `digit % 6`.
fn push_syllable(digit: usize, word: &mut String) {
    word.push(char::from(CONSONANTS[digit / VOWELS.len()]));
    word.push(char::from(VOWELS[digit % VOWELS.len()]));
}

/// 1 to 3 syllables.
fn common_word(rng: &mut Rng) -> String {
    let mut word = String::new();
    for _ in 0..rng.within(&COMMON_SYLLABLES) {
        syllable(rng, &mut word);
    }
    word
}

/// Rare word `number`: its base-36 code in exactly five syllables, most
/// significant first, one word per number. Longer than any common word,
/// so no common word holds one.
fn rare_word(number: usize) -> String {
    let mut digits = [0_usize; RARE_SYLLABLES];
    let mut rest = number;
    for digit in digits.iter_mut().rev() {
        *digit = rest % SYLLABLES;
        rest /= SYLLABLES;
    }
    let mut word = String::with_capacity(2 * RARE_SYLLABLES);
    for digit in digits {
        push_syllable(digit, &mut word);
    }
    word
}

/// 2 to 4 common words, the first capitalised.
fn title(rng: &mut Rng) -> String {
    let words: Vec<String> = (0..rng.within(&TITLE_WORDS))
        .map(|_| common_word(rng))
        .collect();
    let mut title = words.join(" ");
    capitalise(&mut title);
    title
}

/// A paragraph of `words` common words, `specials` (IDs, a rare word) in
/// place of as many of them at distinct drawn positions, cut into
/// sentences of 6 to 14 words, each capitalised and ending with `.`.
fn paragraph(rng: &mut Rng, words: &RangeInclusive<usize>, specials: &[String]) -> String {
    let count = rng.within(words);
    let mut tokens: Vec<String> = (0..count).map(|_| common_word(rng)).collect();
    let mut free: Vec<usize> = (0..count).collect();
    for special in specials {
        if free.is_empty() {
            break;
        }
        let at = free.swap_remove(rng.below(free.len()));
        tokens[at].clone_from(special);
    }
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0;
    while start < count {
        let left = count - start;
        let length = if left <= *SENTENCE_WORDS.end() {
            left
        } else {
            // Leave at least one whole sentence behind.
            let longest = (*SENTENCE_WORDS.end()).min(left - SENTENCE_WORDS.start());
            rng.within(&(*SENTENCE_WORDS.start()..=longest))
        };
        let mut sentence = tokens[start..start + length].join(" ");
        capitalise(&mut sentence);
        sentence.push('.');
        sentences.push(sentence);
        start += length;
    }
    sentences.join(" ")
}

/// The first letter upper-cased (a word starting with a syllable; an ID
/// already starts with `P`).
fn capitalise(text: &mut String) {
    if let Some(first) = text.get(..1)
        && first.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        let upper = first.to_ascii_uppercase();
        text.replace_range(..1, &upper);
    }
}
