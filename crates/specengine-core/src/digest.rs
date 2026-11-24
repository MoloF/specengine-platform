//! Streaming SHA-256 and SHA-1 (FIPS 180-4), fed in pieces, never holding
//! the input (`docs/canon/work-references-promotion-surfaces.md`
//! "Propose and apply", step 8): a promoted file's SHA-256 names its LFS
//! object and the v1 pointer the apply writes itself (no git-lfs process
//! runs), and its git object ID (`blob <size>` NUL and the bytes, under the
//! repository's object format) checks the blob a plain home commits. No
//! dependency of the workspace computes either; both are integrity checks
//! of bytes this process streamed, never a security boundary.

/// SHA-256's round constants.
const K256: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// SHA-256's initial state.
const H256: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// SHA-1's initial state.
const H1: [u32; 5] = [
    0x6745_2301,
    0xefcd_ab89,
    0x98ba_dcfe,
    0x1032_5476,
    0xc3d2_e1f0,
];

/// The block buffer both hashers share: 64-byte blocks, the length in bits
/// appended big-endian at the end.
#[derive(Clone)]
struct Blocks {
    buffer: [u8; 64],
    filled: usize,
    length: u64,
}

impl Blocks {
    const fn new() -> Self {
        Self {
            buffer: [0; 64],
            filled: 0,
            length: 0,
        }
    }

    /// Feeds `bytes`, handing each full block to `block`.
    fn update(&mut self, mut bytes: &[u8], mut block: impl FnMut(&[u8; 64])) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        if self.filled > 0 {
            let take = (64 - self.filled).min(bytes.len());
            self.buffer[self.filled..self.filled + take].copy_from_slice(&bytes[..take]);
            self.filled += take;
            bytes = &bytes[take..];
            if self.filled < 64 {
                return;
            }
            block(&self.buffer);
            self.filled = 0;
        }
        let (whole, rest) = bytes.as_chunks::<64>();
        for chunk in whole {
            block(chunk);
        }
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.filled = rest.len();
    }

    /// The padding: `0x80`, zeros, the length in bits.
    fn finish(mut self, mut block: impl FnMut(&[u8; 64])) {
        let bits = self.length.wrapping_mul(8);
        self.buffer[self.filled] = 0x80;
        self.filled += 1;
        if self.filled > 56 {
            self.buffer[self.filled..].fill(0);
            block(&self.buffer);
            self.filled = 0;
        }
        self.buffer[self.filled..56].fill(0);
        self.buffer[56..].copy_from_slice(&bits.to_be_bytes());
        block(&self.buffer);
    }
}

/// A streaming SHA-256.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    blocks: Blocks,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub const fn new() -> Self {
        Self {
            state: H256,
            blocks: Blocks::new(),
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let state = &mut self.state;
        self.blocks.update(bytes, |block| compress256(state, block));
    }

    /// The digest, lower-case hex.
    pub fn hex(mut self) -> String {
        let state = &mut self.state;
        self.blocks.finish(|block| compress256(state, block));
        hex_words(&self.state)
    }
}

fn compress256(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (at, word) in block.as_chunks::<4>().0.iter().enumerate() {
        w[at] = u32::from_be_bytes(*word);
    }
    for at in 16..64 {
        let s0 = w[at - 15].rotate_right(7) ^ w[at - 15].rotate_right(18) ^ (w[at - 15] >> 3);
        let s1 = w[at - 2].rotate_right(17) ^ w[at - 2].rotate_right(19) ^ (w[at - 2] >> 10);
        w[at] = w[at - 16]
            .wrapping_add(s0)
            .wrapping_add(w[at - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for at in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K256[at])
            .wrapping_add(w[at]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (word, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(add);
    }
}

/// A streaming SHA-1.
#[derive(Clone)]
pub struct Sha1 {
    state: [u32; 5],
    blocks: Blocks,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    pub const fn new() -> Self {
        Self {
            state: H1,
            blocks: Blocks::new(),
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let state = &mut self.state;
        self.blocks.update(bytes, |block| compress1(state, block));
    }

    /// The digest, lower-case hex.
    pub fn hex(mut self) -> String {
        let state = &mut self.state;
        self.blocks.finish(|block| compress1(state, block));
        hex_words(&self.state)
    }
}

fn compress1(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (at, word) in block.as_chunks::<4>().0.iter().enumerate() {
        w[at] = u32::from_be_bytes(*word);
    }
    for at in 16..80 {
        w[at] = (w[at - 3] ^ w[at - 8] ^ w[at - 14] ^ w[at - 16]).rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (at, word) in w.iter().enumerate() {
        let (f, k) = match at {
            0..=19 => ((b & c) | (!b & d), 0x5a82_7999),
            20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
            _ => (b ^ c ^ d, 0xca62_c1d6),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(*word);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }
    for (word, add) in state.iter_mut().zip([a, b, c, d, e]) {
        *word = word.wrapping_add(add);
    }
}

fn hex_words(words: &[u32]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(words.len() * 8);
    for word in words {
        let _ = write!(out, "{word:08x}");
    }
    out
}

/// A git object ID of a blob being streamed, under the repository's object
/// format: `blob <size>` NUL, then the bytes.
#[derive(Clone)]
pub enum BlobId {
    Sha1(Sha1),
    Sha256(Sha256),
}

impl BlobId {
    /// A blob of `size` bytes under `format` (`sha1` or `sha256`, as `git
    /// rev-parse --show-object-format` prints it); `None` for another.
    pub fn new(format: &str, size: u64) -> Option<Self> {
        let header = format!("blob {size}\0");
        match format {
            "sha1" => {
                let mut hasher = Sha1::new();
                hasher.update(header.as_bytes());
                Some(Self::Sha1(hasher))
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(header.as_bytes());
                Some(Self::Sha256(hasher))
            }
            _ => None,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha1(hasher) => hasher.update(bytes),
            Self::Sha256(hasher) => hasher.update(bytes),
        }
    }

    /// The object ID, lower-case hex.
    pub fn hex(self) -> String {
        match self {
            Self::Sha1(hasher) => hasher.hex(),
            Self::Sha256(hasher) => hasher.hex(),
        }
    }
}
