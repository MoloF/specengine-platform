//! The engine's own words of a reference's file (task spec `work-references`,
//! "The blob store", "Types", "Facts", "The agent copy"; canon
//! `work-references.md`; ADR-0088): the caps and their settings, the fixed
//! table of media types and classes a store keeps, the sniff that judges a
//! file by its first bytes alone, the cheap facts read from its headers, the
//! streaming intake that hashes and sniffs as bytes pass, and the rules of an
//! agent's image. No file is opened here: the caller hands bytes, or a
//! reader over them; no pixel, sample or frame is decoded; no name, extension
//! or sent type decides a type. The core knows media types, never a domain's
//! kinds (ADR-0008).

use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

/// The most bytes of one file (256 MiB); `SPECENGINE_BLOB_FILE_MAX` may lower it.
pub const BLOB_FILE_MAX: u64 = 268_435_456;

/// The most bytes of a project's store (10 GiB); `SPECENGINE_BLOB_QUOTA` may
/// lower it.
pub const BLOB_QUOTA: u64 = 10_737_418_240;

/// The most files of a project's store, which bounds its walk.
pub const BLOB_FILES_MAX: u64 = 20_000;

/// The bytes a store's file system keeps free (5 GiB);
/// `SPECENGINE_BLOB_FLOOR` may raise it.
pub const BLOB_FLOOR: u64 = 5_368_709_120;

/// How many bytes pass between two reads of the free space (16 MiB).
pub const FLOOR_EVERY: u64 = 16_777_216;

/// The one buffer an upload's bytes pass through (64 KiB); a blob is served
/// in pieces of this size too.
pub const BLOB_BUFFER: usize = 65_536;

/// The first bytes kept to sniff a file's type (4 KiB).
pub const SNIFF_BYTES: usize = 4_096;

/// The bytes, after a BOM and whitespace, searched for markup.
pub const MARKUP_BYTES: usize = 512;

/// The most bytes [`facts`] reads of one file (2 MiB).
pub const FACTS_READ_MAX: u64 = 2_097_152;

/// The most bytes of a GLB's JSON chunk [`facts`] parses (1 MiB).
pub const GLB_JSON_MAX: u64 = 1_048_576;

/// How far a JPEG's segments or a WAV's chunks are walked (1 MiB).
pub const HEADER_WALK_MAX: u64 = 1_048_576;

/// The tail of an Ogg file searched for its last page (64 KiB).
pub const OGG_TAIL: u64 = 65_536;

/// An image reaches an agent as it is when its long edge is at most this.
pub const AGENT_EDGE: u64 = 1_568;

/// An image reaches an agent as it is when it is at most this (1 MiB).
pub const AGENT_BYTES: u64 = 1_048_576;

/// A store is swept at most once in this long.
pub const SWEEP_EVERY: Duration = Duration::from_secs(3_600);

/// A file no live reference names is swept once its mtime is this old.
pub const BLOB_GRACE: Duration = Duration::from_secs(86_400);

/// An upload's temporary file is swept once its mtime is this old.
pub const PART_GRACE: Duration = Duration::from_secs(21_600);

/// The most files one sweep collects.
pub const SWEEP_MAX: usize = 512;

/// Uploads a hub (or a local daemon) runs at once.
pub const UPLOADS_PER_HUB: usize = 8;

/// Uploads one login of a hub runs at once.
pub const UPLOADS_PER_LOGIN: usize = 2;

/// A body idle this long is cut.
pub const UPLOAD_IDLE: Duration = Duration::from_secs(60);

/// The three settings, by name.
pub const FILE_MAX_SETTING: &str = "SPECENGINE_BLOB_FILE_MAX";
pub const QUOTA_SETTING: &str = "SPECENGINE_BLOB_QUOTA";
pub const FLOOR_SETTING: &str = "SPECENGINE_BLOB_FLOOR";

/// A class of the table: what a person picks a file as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MediaClass {
    Image,
    Audio,
    Video,
    Model,
    Pdf,
}

impl MediaClass {
    pub const ALL: [Self; 5] = [
        Self::Image,
        Self::Audio,
        Self::Video,
        Self::Model,
        Self::Pdf,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Model => "model",
            Self::Pdf => "pdf",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.as_str() == text)
    }

    /// "What matters here" is required of the class (task spec
    /// `work-references` D10).
    pub const fn needs_matters(self) -> bool {
        !matches!(self, Self::Image)
    }

    /// Served `inline`; `model` and `pdf` as an `attachment`.
    pub const fn inline(self) -> bool {
        matches!(self, Self::Image | Self::Audio | Self::Video)
    }
}

/// A media type of the table (task spec `work-references`, "Types").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Media {
    Png,
    Jpeg,
    Gif,
    Webp,
    Wav,
    Flac,
    Ogg,
    Mpeg,
    Mp4,
    Webm,
    Glb,
    Blender,
    Pdf,
}

/// The warning a `.blend` carries (task spec `work-references` D15).
pub const BLEND_WARNING: &str = "a .blend is only downloaded here; Blender can run the scripts it \
                                 holds: open it with Auto Run Python Scripts off";

impl Media {
    pub const ALL: [Self; 13] = [
        Self::Png,
        Self::Jpeg,
        Self::Gif,
        Self::Webp,
        Self::Wav,
        Self::Flac,
        Self::Ogg,
        Self::Mpeg,
        Self::Mp4,
        Self::Webm,
        Self::Glb,
        Self::Blender,
        Self::Pdf,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Wav => "audio/wav",
            Self::Flac => "audio/flac",
            Self::Ogg => "audio/ogg",
            Self::Mpeg => "audio/mpeg",
            Self::Mp4 => "video/mp4",
            Self::Webm => "video/webm",
            Self::Glb => "model/gltf-binary",
            Self::Blender => "application/x-blender",
            Self::Pdf => "application/pdf",
        }
    }

    /// The media type stored as `text`, exactly.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|media| media.as_str() == text)
    }

    pub const fn class(self) -> MediaClass {
        match self {
            Self::Png | Self::Jpeg | Self::Gif | Self::Webp => MediaClass::Image,
            Self::Wav | Self::Flac | Self::Ogg | Self::Mpeg => MediaClass::Audio,
            Self::Mp4 | Self::Webm => MediaClass::Video,
            Self::Glb | Self::Blender => MediaClass::Model,
            Self::Pdf => MediaClass::Pdf,
        }
    }

    pub const fn inline(self) -> bool {
        self.class().inline()
    }

    /// The warning line of the type: a `.blend`'s alone.
    pub const fn warning(self) -> Option<&'static str> {
        match self {
            Self::Blender => Some(BLEND_WARNING),
            _ => None,
        }
    }

    /// A type an agent copy may be.
    pub const fn copyable(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::Webp)
    }
}

/// Why a file is refused, for the daemon's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobRefusalKind {
    /// A cap, the quota, the file count, the floor: 413, exit 1.
    TooLarge,
    /// The type, or outside `allow`: 415, exit 1.
    Type,
    /// A body shorter or longer than declared: 400.
    Body,
}

/// A file refused, its line `refused: …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRefusal {
    pub kind: BlobRefusalKind,
    pub line: String,
}

impl BlobRefusal {
    fn new(kind: BlobRefusalKind, line: String) -> Self {
        Self { kind, line }
    }
}

/// `bytes` in binary units, one decimal, rounded to the nearest
/// (`300.0 MiB`; 2 202 009 bytes `2.1 MiB`); under 1 KiB in bytes. A value
/// that rounds to 1024 of a unit is written in the next.
pub fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1_024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1_024.0;
    let mut unit = 0;
    while (value * 10.0).round() / 10.0 >= 1_024.0 && unit + 1 < UNITS.len() {
        value /= 1_024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// `bytes` as [`size_text`] writes them, but rounded down, so what remains
/// under a floor never reads as the floor itself.
fn size_text_down(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1_024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1_024.0;
    let mut unit = 0;
    while value >= 1_024.0 && unit + 1 < UNITS.len() {
        value /= 1_024.0;
        unit += 1;
    }
    format!("{:.1} {}", (value * 10.0).floor() / 10.0, UNITS[unit])
}

/// A cap in binary units: whole when it is (`256 MiB`), else one decimal.
pub fn cap_text(bytes: u64) -> String {
    for (shift, unit) in [(40u32, "TiB"), (30, "GiB"), (20, "MiB"), (10, "KiB")] {
        let one = 1u64 << shift;
        if bytes >= one && bytes.is_multiple_of(one) {
            return format!("{} {unit}", bytes / one);
        }
    }
    size_text(bytes)
}

/// `n` with a space between each group of three digits (`20 000`).
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(digit);
    }
    out
}

/// The caps a store judges, the settings applied (they narrow; nothing
/// widens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobCaps {
    pub file_max: u64,
    pub quota: u64,
    pub floor: u64,
}

impl Default for BlobCaps {
    fn default() -> Self {
        Self {
            file_max: BLOB_FILE_MAX,
            quota: BLOB_QUOTA,
            floor: BLOB_FLOOR,
        }
    }
}

impl BlobCaps {
    /// The caps with the three settings `lookup` reads applied, unset or
    /// empty meaning the default; any other value is its line (exit 2 at a
    /// start): `SPECENGINE_BLOB_QUOTA: 20000000000 is over 10 GiB: it may
    /// only lower the cap`.
    pub fn from_settings(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let mut caps = Self::default();
        let read = |name: &str| -> Result<Option<u64>, String> {
            let Some(written) = lookup(name) else {
                return Ok(None);
            };
            if written.is_empty() {
                return Ok(None);
            }
            if !written.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "{name}: `{written}` is not a number of bytes (decimal digits)"
                ));
            }
            written
                .parse::<u64>()
                .map(Some)
                .map_err(|_| format!("{name}: {written} is too large"))
        };
        if let Some(value) = read(FILE_MAX_SETTING)? {
            if value == 0 {
                return Err(format!("{FILE_MAX_SETTING}: 0 is no cap: give 1 or more"));
            }
            if value > BLOB_FILE_MAX {
                return Err(format!(
                    "{FILE_MAX_SETTING}: {value} is over {}: it may only lower the cap",
                    cap_text(BLOB_FILE_MAX)
                ));
            }
            caps.file_max = value;
        }
        if let Some(value) = read(QUOTA_SETTING)? {
            if value == 0 {
                return Err(format!("{QUOTA_SETTING}: 0 is no cap: give 1 or more"));
            }
            if value > BLOB_QUOTA {
                return Err(format!(
                    "{QUOTA_SETTING}: {value} is over {}: it may only lower the cap",
                    cap_text(BLOB_QUOTA)
                ));
            }
            caps.quota = value;
        }
        if let Some(value) = read(FLOOR_SETTING)? {
            if value < BLOB_FLOOR {
                return Err(format!(
                    "{FLOOR_SETTING}: {value} is under {}: it may only raise the floor",
                    cap_text(BLOB_FLOOR)
                ));
            }
            caps.floor = value;
        }
        Ok(caps)
    }

    /// The process's settings.
    pub fn from_process() -> Result<Self, String> {
        Self::from_settings(|name| std::env::var(name).ok())
    }

    /// `name` of `size` bytes is over the file cap: its refusal.
    pub fn file_refusal(&self, name: &str, size: u64) -> Option<BlobRefusal> {
        (size > self.file_max).then(|| {
            BlobRefusal::new(
                BlobRefusalKind::TooLarge,
                format!(
                    "refused: {name} is {}; a file here is at most {}",
                    size_text(size),
                    cap_text(self.file_max)
                ),
            )
        })
    }

    /// `stored` bytes and `incoming` more pass the quota: its refusal.
    pub fn quota_refusal(&self, stored: u64, incoming: u64) -> Option<BlobRefusal> {
        self.quota_refusal_beside(stored, 0, incoming)
    }

    /// [`Self::quota_refusal`] beside `in_flight` bytes the project's other
    /// uploads declared: judged on all three, the line naming what is
    /// stored alone.
    pub fn quota_refusal_beside(
        &self,
        stored: u64,
        in_flight: u64,
        incoming: u64,
    ) -> Option<BlobRefusal> {
        let total = stored.saturating_add(in_flight).saturating_add(incoming);
        (total > self.quota).then(|| {
            BlobRefusal::new(
                BlobRefusalKind::TooLarge,
                format!(
                    "refused: this project's files would pass {} ({} stored); delete some first",
                    cap_text(self.quota),
                    size_text(stored)
                ),
            )
        })
    }

    /// `available` free bytes, `still` bytes still to come: under the floor,
    /// its refusal.
    pub fn floor_refusal(&self, available: u64, still: u64) -> Option<BlobRefusal> {
        let remain = available.saturating_sub(still);
        (remain < self.floor).then(|| {
            BlobRefusal::new(
                BlobRefusalKind::TooLarge,
                format!(
                    "refused: the store keeps {} free; {} would remain",
                    cap_text(self.floor),
                    size_text_down(remain)
                ),
            )
        })
    }
}

/// A store already holding `files` files refuses new bytes: its refusal.
pub fn files_refusal(files: u64) -> Option<BlobRefusal> {
    (files >= BLOB_FILES_MAX).then(|| {
        BlobRefusal::new(
            BlobRefusalKind::TooLarge,
            format!(
                "refused: this project stores {} files; delete some first",
                grouped(BLOB_FILES_MAX)
            ),
        )
    })
}

/// `text` is a BLAKE3 as a store names it: 64 lower-case hex digits.
pub fn is_b3(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// What a type's first bytes are refused as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Html,
    Svg,
    Xml,
    /// An archive, named (`zstd`).
    Archive(&'static str),
    Program,
    Unknown,
}

impl Refused {
    /// The line naming `name`.
    pub fn line(self, name: &str) -> String {
        match self {
            Self::Html => format!("refused: {name}: this file is HTML, which is never stored here"),
            Self::Svg => format!("refused: {name}: this file is SVG, which is never stored here"),
            Self::Xml => format!("refused: {name}: this file is XML, which is never stored here"),
            Self::Archive(archive) => {
                let blend = matches!(archive, "zstd" | "gzip")
                    && name.to_ascii_lowercase().ends_with(".blend");
                format!(
                    "refused: {name}: an archive ({archive}) is never stored here{}",
                    if blend {
                        "; save the .blend uncompressed"
                    } else {
                        ""
                    }
                )
            }
            Self::Program => format!("refused: {name}: a program is never stored here"),
            Self::Unknown => format!(
                "refused: {name}: not a type stored here (images PNG, JPEG, GIF, WebP; audio \
                 WAV, FLAC, Ogg, MP3; video MP4, WebM; models GLB, .blend; PDF)"
            ),
        }
    }
}

/// The type of a file whose first bytes are `head` (at most
/// [`SNIFF_BYTES`], fewer only for a shorter file), or why none: the table's
/// magic first, then the archives' and programs', then markup after a BOM
/// and whitespace; anything else is no type stored here.
pub fn sniff(head: &[u8]) -> Result<Media, Refused> {
    if let Some(media) = stored_magic(head) {
        return Ok(media);
    }
    if let Some(refused) = refused_magic(head) {
        return Err(refused);
    }
    if let Some(refused) = markup(head) {
        return Err(refused);
    }
    Err(Refused::Unknown)
}

fn at(head: &[u8], offset: usize, magic: &[u8]) -> bool {
    head.get(offset..offset + magic.len()) == Some(magic)
}

fn stored_magic(head: &[u8]) -> Option<Media> {
    if at(head, 0, b"\x89PNG\r\n\x1a\n") && at(head, 12, b"IHDR") {
        return Some(Media::Png);
    }
    if at(head, 0, &[0xFF, 0xD8, 0xFF]) {
        return Some(Media::Jpeg);
    }
    if at(head, 0, b"GIF87a") || at(head, 0, b"GIF89a") {
        return Some(Media::Gif);
    }
    if at(head, 0, b"RIFF") {
        if at(head, 8, b"WEBP")
            && (at(head, 12, b"VP8 ") || at(head, 12, b"VP8L") || at(head, 12, b"VP8X"))
        {
            return Some(Media::Webp);
        }
        if at(head, 8, b"WAVE") {
            return Some(Media::Wav);
        }
    }
    if at(head, 0, b"fLaC") {
        return Some(Media::Flac);
    }
    if at(head, 0, b"OggS") && ogg_first_packet(head).is_some() {
        return Some(Media::Ogg);
    }
    if mpeg_audio(head) {
        return Some(Media::Mpeg);
    }
    if at(head, 4, b"ftyp") {
        const BRANDS: [&[u8; 4]; 10] = [
            b"isom", b"iso2", b"iso4", b"iso5", b"iso6", b"mp41", b"mp42", b"avc1", b"M4V ",
            b"dash",
        ];
        if BRANDS.iter().any(|brand| at(head, 8, *brand)) {
            return Some(Media::Mp4);
        }
    }
    if at(head, 0, &[0x1A, 0x45, 0xDF, 0xA3]) && webm_doctype(head) {
        return Some(Media::Webm);
    }
    if at(head, 0, b"glTF") && at(head, 4, &2u32.to_le_bytes()) {
        return Some(Media::Glb);
    }
    if at(head, 0, b"BLENDER") {
        return Some(Media::Blender);
    }
    if at(head, 0, b"%PDF-") {
        return Some(Media::Pdf);
    }
    None
}

/// An Ogg file's first packet: Vorbis's identification header or Opus's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OggCodec {
    Vorbis,
    Opus,
}

/// The codec of the first page's first packet.
fn ogg_first_packet(head: &[u8]) -> Option<OggCodec> {
    let segments = usize::from(*head.get(26)?);
    let data = 27 + segments;
    if at(head, data, b"\x01vorbis") {
        Some(OggCodec::Vorbis)
    } else if at(head, data, b"OpusHead") {
        Some(OggCodec::Opus)
    } else {
        None
    }
}

/// `ID3` with a major version 2 to 4, or an MPEG-1/2 layer III frame header
/// at 0.
fn mpeg_audio(head: &[u8]) -> bool {
    if at(head, 0, b"ID3") {
        return head.get(3).is_some_and(|major| (2..=4).contains(major));
    }
    let (Some(&first), Some(&second), Some(&third)) = (head.first(), head.get(1), head.get(2))
    else {
        return false;
    };
    let sync = first == 0xFF && second & 0xE0 == 0xE0;
    let version = (second >> 3) & 0b11;
    let layer = (second >> 1) & 0b11;
    let bitrate = third >> 4;
    let rate = (third >> 2) & 0b11;
    sync && matches!(version, 0b11 | 0b10)
        && layer == 0b01
        && bitrate != 0
        && bitrate != 15
        && rate != 3
}

/// The EBML DocType `webm` within the first 64 bytes: the DocType element
/// (`42 82`), its one-byte size, then `webm`.
fn webm_doctype(head: &[u8]) -> bool {
    let window = &head[..head.len().min(64)];
    window.windows(2).enumerate().any(|(index, pair)| {
        pair == [0x42, 0x82]
            && window
                .get(index + 3..index + 7)
                .is_some_and(|doctype| doctype == b"webm")
    })
}

fn refused_magic(head: &[u8]) -> Option<Refused> {
    const ARCHIVES: [(&[u8], &str); 9] = [
        (b"PK\x03\x04", "zip"),
        (b"PK\x05\x06", "zip"),
        (b"PK\x07\x08", "zip"),
        (&[0x1F, 0x8B], "gzip"),
        (&[0x28, 0xB5, 0x2F, 0xFD], "zstd"),
        (&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00], "xz"),
        (b"BZh", "bzip2"),
        (&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], "7z"),
        (b"Rar!\x1a\x07", "rar"),
    ];
    for (magic, name) in ARCHIVES {
        if at(head, 0, magic) {
            return Some(Refused::Archive(name));
        }
    }
    if at(head, 257, b"ustar") {
        return Some(Refused::Archive("tar"));
    }
    const PROGRAMS: [&[u8]; 8] = [
        &[0x7F, 0x45, 0x4C, 0x46],
        b"MZ",
        &[0xFE, 0xED, 0xFA, 0xCE],
        &[0xFE, 0xED, 0xFA, 0xCF],
        &[0xCF, 0xFA, 0xED, 0xFE],
        &[0xCE, 0xFA, 0xED, 0xFE],
        &[0xCA, 0xFE, 0xBA, 0xBE],
        b"#!",
    ];
    PROGRAMS
        .iter()
        .any(|magic| at(head, 0, magic))
        .then_some(Refused::Program)
}

/// Markup in the first [`MARKUP_BYTES`] after a UTF-8 BOM and ASCII
/// whitespace, any case: SVG over HTML over XML.
fn markup(head: &[u8]) -> Option<Refused> {
    let mut rest = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    while let Some((&first, tail)) = rest.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        rest = tail;
    }
    let window: Vec<u8> = rest[..rest.len().min(MARKUP_BYTES)].to_ascii_lowercase();
    let holds = |token: &[u8]| window.windows(token.len()).any(|part| part == token);
    if holds(b"<svg") {
        return Some(Refused::Svg);
    }
    const HTML: [&[u8]; 6] = [
        b"<!doctype html",
        b"<html",
        b"<head",
        b"<body",
        b"<script",
        b"<iframe",
    ];
    if HTML.iter().any(|token| holds(token)) {
        return Some(Refused::Html);
    }
    holds(b"<?xml").then_some(Refused::Xml)
}

/// A file's facts: each an integer or `null` (task spec `work-references`,
/// "Facts").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub duration_ms: Option<u64>,
    pub triangles: Option<u64>,
}

/// The bytes a read of [`Budget`] takes ahead of what it was asked, so a
/// walk of small headers (a JPEG's segments, its fill bytes one at a time)
/// reads the file in windows, never byte by byte.
const READ_AHEAD: usize = 8_192;

/// A reader of one file under [`FACTS_READ_MAX`]: every byte read counted,
/// a read past it refused, so no file is read whole; reads go through one
/// window of at least [`READ_AHEAD`] bytes, the buffered reader's part.
struct Budget<R> {
    inner: R,
    read: u64,
    size: u64,
    /// The bytes last read, and where they start in the file.
    window: Vec<u8>,
    window_at: u64,
}

impl<R: Read + Seek> Budget<R> {
    fn new(inner: R, size: u64) -> Self {
        Self {
            inner,
            read: 0,
            size,
            window: Vec::new(),
            window_at: 0,
        }
    }

    /// `len` bytes at `offset`, exactly; `None` past the end or the budget.
    /// Served from the window when it holds them; else a new window is read
    /// at `offset`, `len` bytes or [`READ_AHEAD`] when more, within the file
    /// and what the budget leaves.
    fn bytes(&mut self, offset: u64, len: usize) -> Option<Vec<u8>> {
        let len64 = len as u64;
        let end = offset.checked_add(len64)?;
        if end > self.size {
            return None;
        }
        let window_end = self.window_at + self.window.len() as u64;
        if offset >= self.window_at && end <= window_end {
            let from = usize::try_from(offset - self.window_at).ok()?;
            return Some(self.window[from..from + len].to_vec());
        }
        let left = FACTS_READ_MAX.checked_sub(self.read)?;
        if len64 > left {
            return None;
        }
        let want = (len.max(READ_AHEAD) as u64)
            .min(self.size - offset)
            .min(left);
        let want = usize::try_from(want).ok()?;
        self.inner.seek(SeekFrom::Start(offset)).ok()?;
        let mut buffer = vec![0; want];
        self.inner.read_exact(&mut buffer).ok()?;
        self.read += want as u64;
        self.window = buffer;
        self.window_at = offset;
        Some(self.window[..len].to_vec())
    }

    /// At most `len` bytes at `offset`, fewer at the end.
    fn upto(&mut self, offset: u64, len: usize) -> Option<Vec<u8>> {
        let len = len.min(usize::try_from(self.size.saturating_sub(offset)).unwrap_or(usize::MAX));
        self.bytes(offset, len)
    }
}

fn u16_be(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u16::from_be_bytes(
        bytes.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u16_le(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u16::from_le_bytes(
        bytes.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u32_be(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u32::from_be_bytes(
        bytes.get(at..at + 4)?.try_into().ok()?,
    )))
}

fn u32_le(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u32::from_le_bytes(
        bytes.get(at..at + 4)?.try_into().ok()?,
    )))
}

fn u24_le(bytes: &[u8], at: usize) -> Option<u64> {
    let part = bytes.get(at..at + 3)?;
    Some(u64::from(part[0]) | u64::from(part[1]) << 8 | u64::from(part[2]) << 16)
}

/// The facts of a file of `media` and `size` bytes read through `reader`
/// (one function for every caller, so equal bytes give equal facts): at
/// most [`FACTS_READ_MAX`] bytes read; any defect, a truncated header
/// among them, leaves its fact `null`.
pub fn facts<R: Read + Seek>(media: Media, reader: R, size: u64) -> Facts {
    let mut source = Budget::new(reader, size);
    let mut facts = Facts::default();
    match media {
        Media::Png => {
            if let Some(head) = source.bytes(0, 24) {
                facts.width = u32_be(&head, 16);
                facts.height = u32_be(&head, 20);
            }
        }
        Media::Jpeg => {
            if let Some((width, height)) = jpeg_size(&mut source) {
                facts.width = Some(width);
                facts.height = Some(height);
            }
        }
        Media::Gif => {
            if let Some(head) = source.bytes(0, 10) {
                facts.width = u16_le(&head, 6);
                facts.height = u16_le(&head, 8);
            }
        }
        Media::Webp => {
            if let Some((width, height)) = webp_size(&mut source) {
                facts.width = Some(width);
                facts.height = Some(height);
            }
        }
        Media::Wav => facts.duration_ms = wav_duration(&mut source),
        Media::Flac => facts.duration_ms = flac_duration(&mut source),
        Media::Ogg => facts.duration_ms = ogg_duration(&mut source),
        Media::Glb => facts.triangles = glb_triangles(&mut source),
        Media::Mpeg | Media::Mp4 | Media::Webm | Media::Blender | Media::Pdf => {}
    }
    facts
}

/// A JPEG's frame size: the segments walked by their lengths from offset 2,
/// never a byte scan, so an EXIF thumbnail inside `APP1` is skipped; the
/// first `SOF0`-`SOF15` but `C4`, `C8` and `CC`; `SOS` or 1 MiB first: none.
fn jpeg_size<R: Read + Seek>(source: &mut Budget<R>) -> Option<(u64, u64)> {
    let mut at = 2u64;
    while at < HEADER_WALK_MAX {
        let marker = source.bytes(at, 2)?;
        if marker[0] != 0xFF {
            return None;
        }
        match marker[1] {
            // Fill bytes before a marker.
            0xFF => {
                at += 1;
                continue;
            }
            // Markers with no length.
            0x01 | 0xD0..=0xD8 => {
                at += 2;
                continue;
            }
            0xDA | 0xD9 => return None,
            0xC0..=0xCF if !matches!(marker[1], 0xC4 | 0xC8 | 0xCC) => {
                let frame = source.bytes(at, 9)?;
                let height = u16_be(&frame, 5)?;
                let width = u16_be(&frame, 7)?;
                return Some((width, height));
            }
            _ => {
                let length = source.bytes(at + 2, 2)?;
                let length = u16_be(&length, 0)?;
                if length < 2 {
                    return None;
                }
                at += 2 + length;
            }
        }
    }
    None
}

/// A WebP's size from its first chunk: `VP8 `, `VP8L` or `VP8X`.
fn webp_size<R: Read + Seek>(source: &mut Budget<R>) -> Option<(u64, u64)> {
    let head = source.bytes(0, 30)?;
    match head.get(12..16)? {
        b"VP8 " => Some((u16_le(&head, 26)? & 0x3FFF, u16_le(&head, 28)? & 0x3FFF)),
        b"VP8L" => {
            let bits = u32_le(&head, 21)?;
            Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
        }
        b"VP8X" => Some((u24_le(&head, 24)? + 1, u24_le(&head, 27)? + 1)),
        _ => None,
    }
}

/// A WAV's duration: the chunks walked within 1 MiB, `data`'s size × 1000
/// by `fmt `'s byte rate, rounded down.
fn wav_duration<R: Read + Seek>(source: &mut Budget<R>) -> Option<u64> {
    let mut at = 12u64;
    let mut byte_rate = None;
    let mut data = None;
    while at < HEADER_WALK_MAX && (byte_rate.is_none() || data.is_none()) {
        let header = source.bytes(at, 8)?;
        let size = u32_le(&header, 4)?;
        match &header[..4] {
            b"fmt " => {
                let format = source.bytes(at + 8, 12)?;
                byte_rate = u32_le(&format, 8);
            }
            b"data" => data = Some(size),
            _ => {}
        }
        at = at + 8 + size + (size & 1);
    }
    let (byte_rate, data) = (byte_rate?, data?);
    if byte_rate == 0 {
        return None;
    }
    Some(data.saturating_mul(1_000) / byte_rate)
}

/// A FLAC's duration from `STREAMINFO`: total samples × 1000 by the
/// sample rate; 0 samples none.
fn flac_duration<R: Read + Seek>(source: &mut Budget<R>) -> Option<u64> {
    let head = source.bytes(0, 42)?;
    // The first block is STREAMINFO (type 0), its data at 8.
    if head[4] & 0x7F != 0 {
        return None;
    }
    let packed = u64::from_be_bytes(head.get(18..26)?.try_into().ok()?);
    let rate = packed >> 44;
    let samples = packed & ((1u64 << 36) - 1);
    if rate == 0 || samples == 0 {
        return None;
    }
    Some(samples.saturating_mul(1_000) / rate)
}

/// An Ogg file's duration: the identification header's rate (Vorbis) or
/// 48 000 with Opus's pre-skip, the granule position of the last page in
/// the last 64 KiB.
fn ogg_duration<R: Read + Seek>(source: &mut Budget<R>) -> Option<u64> {
    let head = source.upto(0, 512)?;
    let segments = usize::from(*head.get(26)?);
    let packet = 27 + segments;
    let (rate, pre_skip) = match ogg_first_packet(&head)? {
        OggCodec::Vorbis => (u32_le(&head, packet + 12)?, 0),
        OggCodec::Opus => (48_000, u16_le(&head, packet + 10)?),
    };
    if rate == 0 {
        return None;
    }
    let start = source.size.saturating_sub(OGG_TAIL);
    let tail = source.upto(start, OGG_TAIL as usize)?;
    let granule = tail
        .windows(4)
        .enumerate()
        .rev()
        .filter(|(_, part)| *part == b"OggS")
        .find_map(|(index, _)| {
            if *tail.get(index + 4)? != 0 {
                return None;
            }
            let granule = i64::from_le_bytes(tail.get(index + 6..index + 14)?.try_into().ok()?);
            (granule >= 0).then_some(granule as u64)
        })?;
    Some(granule.checked_sub(pre_skip)?.saturating_mul(1_000) / rate)
}

/// A GLB's triangles: its JSON chunk (at most [`GLB_JSON_MAX`]) parsed; per
/// mesh, once, each primitive of mode 4 (or none) adds its `indices`
/// accessor's count / 3, else its `POSITION`'s; modes 5 and 6 add count −
/// 2; others nothing; saturating. Any defect: none.
fn glb_triangles<R: Read + Seek>(source: &mut Budget<R>) -> Option<u64> {
    let header = source.bytes(0, 20)?;
    if &header[..4] != b"glTF" || u32_le(&header, 4)? != 2 || &header[16..20] != b"JSON" {
        return None;
    }
    let length = u32_le(&header, 12)?;
    if length > GLB_JSON_MAX {
        return None;
    }
    let json = source.bytes(20, usize::try_from(length).ok()?)?;
    let document: serde_json::Value = serde_json::from_slice(&json).ok()?;
    let accessors = document.get("accessors").and_then(|value| value.as_array());
    let count = |index: &serde_json::Value| -> Option<u64> {
        let index = usize::try_from(index.as_u64()?).ok()?;
        accessors?.get(index)?.get("count")?.as_u64()
    };
    let mut triangles = 0u64;
    for mesh in document.get("meshes")?.as_array()? {
        for primitive in mesh.get("primitives")?.as_array()? {
            let mode = match primitive.get("mode") {
                None => 4,
                Some(mode) => mode.as_u64()?,
            };
            let counted = match primitive.get("indices") {
                Some(indices) => count(indices)?,
                None => count(primitive.get("attributes")?.get("POSITION")?)?,
            };
            let added = match mode {
                4 => counted / 3,
                5 | 6 => counted.saturating_sub(2),
                _ => 0,
            };
            triangles = triangles.saturating_add(added);
        }
    }
    Some(triangles)
}

/// The image is agent-ready: its width and height known, its long edge at
/// most [`AGENT_EDGE`], its size at most [`AGENT_BYTES`].
pub fn agent_ready(media: Media, facts: &Facts, size: u64) -> bool {
    media.class() == MediaClass::Image
        && matches!(
            (facts.width, facts.height),
            (Some(width), Some(height)) if width.max(height) <= AGENT_EDGE
        )
        && size <= AGENT_BYTES
}

/// One file as a reference names it: its type, facts and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Judged {
    pub media: Media,
    pub facts: Facts,
    pub size: u64,
}

/// Why `copy` is no agent copy of `original` (task spec `work-references`,
/// "The agent copy"), its line `agent_b3: …`; `None`: it is one. Its
/// aspect is the image's within 1 % or within one pixel an edge of the image
/// scaled to it. An original whose size is unknown has no aspect to
/// compare: a copy within the limits passes.
pub fn agent_copy_problem(original: &Judged, copy: &Judged) -> Option<String> {
    if original.media.class() != MediaClass::Image {
        return Some(format!(
            "agent_b3: a {} file takes no agent copy; only an image does",
            original.media.class().as_str()
        ));
    }
    if agent_ready(original.media, &original.facts, original.size) {
        return Some(
            "agent_b3: the image reaches agents as it is: it takes no agent copy".to_owned(),
        );
    }
    if !copy.media.copyable() {
        return Some(format!(
            "agent_b3: the copy is {}: an agent copy is JPEG, PNG or WebP",
            copy.media.as_str()
        ));
    }
    if !agent_ready(copy.media, &copy.facts, copy.size) {
        let dims = match (copy.facts.width, copy.facts.height) {
            (Some(width), Some(height)) => format!("{width}x{height}"),
            _ => "of unknown size".to_owned(),
        };
        return Some(format!(
            "agent_b3: the copy is {dims}, {}: an agent copy is at most {} px on its long edge \
             and {}",
            size_text(copy.size),
            grouped(AGENT_EDGE),
            cap_text(AGENT_BYTES)
        ));
    }
    let (Some(width), Some(height)) = (original.facts.width, original.facts.height) else {
        return None;
    };
    let (Some(copy_width), Some(copy_height)) = (copy.facts.width, copy.facts.height) else {
        return None;
    };
    // The image's aspect kept (as it is, or transposed by an EXIF
    // rotation): within 1 %; or, for a full-size copy alone (its long edge
    // the image's scaled to `AGENT_EDGE`, or the image's own when shorter),
    // each edge within one pixel of the image scaled by its long edge: the
    // whole pixels a browser rounds a strip's short edge to (4096x64 to
    // 1568x25 is 2.4 % off, and one pixel). So no tiny copy passes for any
    // image.
    let full_size = copy_width.max(copy_height) == width.max(height).min(AGENT_EDGE);
    let within = |width: u64, height: u64| {
        let (width, height) = (u128::from(width), u128::from(height));
        let (copy_width, copy_height) = (u128::from(copy_width), u128::from(copy_height));
        let off = (copy_width * height).abs_diff(copy_height * width);
        off * 100 <= width * copy_height || (full_size && off <= width.max(height))
    };
    if within(width, height) || within(height, width) {
        None
    } else {
        Some(format!(
            "agent_b3: the copy is {copy_width}x{copy_height}, the image {width}x{height}: a copy \
             keeps the image's aspect"
        ))
    }
}

/// What an upload's bytes were, once all passed [`Intake`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// 64 lower-case hex.
    pub b3: String,
    pub size: u64,
    pub media: Media,
}

/// An upload's bytes as they pass (task spec `work-references`, "The blob
/// store" steps 1 and 2): hashed by an incremental BLAKE3, the first
/// [`SNIFF_BYTES`] kept and sniffed as soon as they are in, so a refused type
/// is refused before the rest is read; the file cap judged on the bytes
/// received. Nothing holds the body.
pub struct Intake {
    name: String,
    file_max: u64,
    allow: Option<crate::reference::ReferencesConfig>,
    hasher: blake3::Hasher,
    size: u64,
    head: Vec<u8>,
    media: Option<Media>,
}

impl Intake {
    /// An intake of the file `name` (its refusal lines' word), its cap and
    /// a project's `allow`.
    pub fn new(
        name: &str,
        file_max: u64,
        allow: Option<crate::reference::ReferencesConfig>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            file_max,
            allow,
            hasher: blake3::Hasher::new(),
            size: 0,
            head: Vec::with_capacity(SNIFF_BYTES),
            media: None,
        }
    }

    /// The bytes received so far.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The type, once sniffed.
    pub fn media(&self) -> Option<Media> {
        self.media
    }

    /// `chunk` taken: hashed, counted against the cap, its first bytes kept
    /// and sniffed once [`SNIFF_BYTES`] are in.
    pub fn take(&mut self, chunk: &[u8]) -> Result<(), BlobRefusal> {
        self.size = self.size.saturating_add(chunk.len() as u64);
        if let Some(refusal) = self.over_cap() {
            return Err(refusal);
        }
        self.hasher.update(chunk);
        if self.media.is_none() {
            let wanted = SNIFF_BYTES - self.head.len();
            self.head
                .extend_from_slice(&chunk[..chunk.len().min(wanted)]);
            if self.head.len() == SNIFF_BYTES {
                self.judge_type()?;
            }
        }
        Ok(())
    }

    fn over_cap(&self) -> Option<BlobRefusal> {
        (self.size > self.file_max).then(|| {
            BlobRefusal::new(
                BlobRefusalKind::TooLarge,
                format!(
                    "refused: {} is over {}; a file here is at most {}",
                    self.name,
                    size_text(self.file_max),
                    cap_text(self.file_max)
                ),
            )
        })
    }

    fn judge_type(&mut self) -> Result<(), BlobRefusal> {
        let media = sniff(&self.head)
            .map_err(|refused| BlobRefusal::new(BlobRefusalKind::Type, refused.line(&self.name)))?;
        if let Some(allow) = &self.allow
            && !allow.allows(media)
        {
            return Err(BlobRefusal::new(
                BlobRefusalKind::Type,
                format!(
                    "refused: {}: {} is not allowed in this project ([references] allow)",
                    self.name,
                    media.as_str()
                ),
            ));
        }
        self.media = Some(media);
        self.head = Vec::new();
        Ok(())
    }

    /// The bytes are all in: a file shorter than [`SNIFF_BYTES`] sniffed
    /// now; an empty one refused as no type.
    pub fn finish(mut self) -> Result<Received, BlobRefusal> {
        if self.media.is_none() {
            self.judge_type()?;
        }
        let media = self.media.ok_or_else(|| {
            BlobRefusal::new(BlobRefusalKind::Type, Refused::Unknown.line(&self.name))
        })?;
        Ok(Received {
            b3: self.hasher.finalize().to_hex().to_string(),
            size: self.size,
            media,
        })
    }
}

/// An incremental BLAKE3 of a stored file's bytes, as a store names them:
/// what a copy out of the store checks against (`spec ref get --out`).
#[derive(Debug, Clone, Default)]
pub struct BlobHasher(blake3::Hasher);

impl BlobHasher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// 64 lower-case hex.
    pub fn finalize(&self) -> String {
        self.0.finalize().to_hex().to_string()
    }
}
