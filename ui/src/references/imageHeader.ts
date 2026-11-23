// A picked image's size read from its first bytes, never by decoding it (docs/features/work-references.md
// "Facts", "The agent copy"; ADR-0086: a tab decoding a huge image at full size is a crash): the same
// rules as the core's `blob::facts`. PNG: `IHDR` width and height (u32 BE at 16 and 20). JPEG: the
// segments walked by their lengths from offset 2, never a byte scan, so an EXIF thumbnail inside
// `APP1` is skipped; the first `SOF0`-`SOF15` but `C4`, `C8` and `CC` gives height and width (u16 BE
// at +5 and +7); `SOS` or 1 MiB first: none. GIF: the logical screen (u16 LE at 6 and 8). WebP:
// `VP8 ` (14 bits at 26 and 28), `VP8L` (14 bits + 1 each from 21), `VP8X` (24 bits + 1 each at 24
// and 27). A JPEG's EXIF orientation (tag 0x0112 in `APP1`'s first IFD) says whether the browser,
// which draws it upright, turns it a quarter: its copy is then drawn with the edges swapped.

/** How far a JPEG's segments are walked, the core's `HEADER_WALK_MAX`. */
const WALK_MAX = 1_048_576;

/** The bytes read of a file to find its size: the walk and a frame header past its end. */
export const HEADER_READ = WALK_MAX + 16;

/** An image's stored size, and whether its EXIF orientation turns it a quarter (5 to 8). */
export interface ImageHeader {
  width: number;
  height: number;
  turned: boolean;
}

function u16be(bytes: Uint8Array, at: number): number | null {
  return at + 2 <= bytes.length ? ((bytes[at] ?? 0) << 8) | (bytes[at + 1] ?? 0) : null;
}

function u16le(bytes: Uint8Array, at: number): number | null {
  return at + 2 <= bytes.length ? ((bytes[at + 1] ?? 0) << 8) | (bytes[at] ?? 0) : null;
}

function u24le(bytes: Uint8Array, at: number): number | null {
  return at + 3 <= bytes.length ? ((bytes[at + 2] ?? 0) << 16) | ((bytes[at + 1] ?? 0) << 8) | (bytes[at] ?? 0) : null;
}

function u32(bytes: Uint8Array, at: number, little: boolean): number | null {
  if (at + 4 > bytes.length) {
    return null;
  }
  return new DataView(bytes.buffer, bytes.byteOffset + at, 4).getUint32(0, little);
}

function startsWith(bytes: Uint8Array, at: number, magic: readonly number[]): boolean {
  return magic.every((byte, index) => bytes[at + index] === byte);
}

function ascii(word: string): number[] {
  return Array.from(word, (char) => char.charCodeAt(0));
}

/** Whether an `APP1` segment's EXIF orientation turns the image a quarter (5 to 8); false when unreadable. */
function exifTurned(bytes: Uint8Array, start: number, end: number): boolean {
  // `Exif\0\0`, the identifier an EXIF `APP1` starts with.
  if (!startsWith(bytes, start, [0x45, 0x78, 0x69, 0x66, 0, 0])) {
    return false;
  }
  const tiff = start + 6;
  const little = startsWith(bytes, tiff, ascii("II"));
  if (!little && !startsWith(bytes, tiff, ascii("MM"))) {
    return false;
  }
  const read16 = (at: number) => (at + 2 > end ? null : little ? u16le(bytes, at) : u16be(bytes, at));
  const offset = u32(bytes, tiff + 4, little);
  if (offset === null) {
    return false;
  }
  const ifd = tiff + offset;
  const count = read16(ifd) ?? 0;
  for (let index = 0; index < count; index += 1) {
    const entry = ifd + 2 + index * 12;
    if (entry + 12 > end) {
      break;
    }
    // Orientation, typed SHORT (3): its value the entry's first two bytes of value.
    if (read16(entry) === 0x0112 && read16(entry + 2) === 3) {
      const value = read16(entry + 8);
      return value !== null && value >= 5 && value <= 8;
    }
  }
  return false;
}

/** A JPEG's frame size, its segments walked by their lengths, and its EXIF orientation met on the way. */
function jpegHeader(bytes: Uint8Array): ImageHeader | null {
  let at = 2;
  let turned = false;
  while (at < WALK_MAX) {
    if (at + 2 > bytes.length || bytes[at] !== 0xff) {
      return null;
    }
    const marker = bytes[at + 1] ?? 0;
    if (marker === 0xff) {
      at += 1;
      continue;
    }
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd8)) {
      at += 2;
      continue;
    }
    if (marker === 0xda || marker === 0xd9) {
      return null;
    }
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      const height = u16be(bytes, at + 5);
      const width = u16be(bytes, at + 7);
      return width === null || height === null ? null : { width, height, turned };
    }
    const length = u16be(bytes, at + 2);
    if (length === null || length < 2) {
      return null;
    }
    if (marker === 0xe1) {
      turned = turned || exifTurned(bytes, at + 4, Math.min(at + 2 + length, bytes.length));
    }
    at += 2 + length;
  }
  return null;
}

/** A WebP's size from its first chunk. */
function webpHeader(bytes: Uint8Array): ImageHeader | null {
  if (startsWith(bytes, 12, ascii("VP8 "))) {
    const width = u16le(bytes, 26);
    const height = u16le(bytes, 28);
    return width === null || height === null ? null : { width: width & 0x3fff, height: height & 0x3fff, turned: false };
  }
  if (startsWith(bytes, 12, ascii("VP8L"))) {
    const bits = u32(bytes, 21, true);
    return bits === null ? null : { width: (bits & 0x3fff) + 1, height: ((bits >>> 14) & 0x3fff) + 1, turned: false };
  }
  if (startsWith(bytes, 12, ascii("VP8X"))) {
    const width = u24le(bytes, 24);
    const height = u24le(bytes, 27);
    return width === null || height === null ? null : { width: width + 1, height: height + 1, turned: false };
  }
  return null;
}

/** Whether the first bytes are a PNG's, a JPEG's, a GIF's or a WebP's: a file a copy may be made of. */
export function startsAsImage(bytes: Uint8Array): boolean {
  return (
    startsWith(bytes, 0, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) ||
    startsWith(bytes, 0, [0xff, 0xd8, 0xff]) ||
    startsWith(bytes, 0, ascii("GIF87a")) ||
    startsWith(bytes, 0, ascii("GIF89a")) ||
    (startsWith(bytes, 0, ascii("RIFF")) && startsWith(bytes, 8, ascii("WEBP")))
  );
}

/** The size the first bytes of a PNG, a JPEG, a GIF or a WebP give, sniffed by their magic; null for anything else, or a size of zero. */
export function headerOf(bytes: Uint8Array): ImageHeader | null {
  let header: ImageHeader | null = null;
  if (startsWith(bytes, 0, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
    const width = u32(bytes, 16, false);
    const height = u32(bytes, 20, false);
    header = width === null || height === null ? null : { width, height, turned: false };
  } else if (startsWith(bytes, 0, [0xff, 0xd8, 0xff])) {
    header = jpegHeader(bytes);
  } else if (startsWith(bytes, 0, ascii("GIF87a")) || startsWith(bytes, 0, ascii("GIF89a"))) {
    const width = u16le(bytes, 6);
    const height = u16le(bytes, 8);
    header = width === null || height === null ? null : { width, height, turned: false };
  } else if (startsWith(bytes, 0, ascii("RIFF")) && startsWith(bytes, 8, ascii("WEBP"))) {
    header = webpHeader(bytes);
  }
  return header === null || header.width === 0 || header.height === 0 ? null : header;
}

/** A picked file's first HEADER_READ bytes, all a size is read from; nothing decoded. */
export async function readHead(file: Blob): Promise<Uint8Array> {
  return new Uint8Array(await file.slice(0, HEADER_READ).arrayBuffer());
}
