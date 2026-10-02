//! BLAKE3 (hash mode), copied from the CLI's `tests/common/bundle.rs`,
//! which writes it from the specification: `bundle_hash` is checked
//! against a hash no crate under test computed.

// BLAKE3, hash mode, written from the specification (one chunk = 1 024
// bytes, blocks of 64, the left subtree the largest power of two of
// chunks, the root flag on the last compression).

const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];
const PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const CHUNK_START: u32 = 1;
const CHUNK_END: u32 = 2;
const PARENT: u32 = 4;
const ROOT: u32 = 8;
const BLOCK_LEN: usize = 64;
const CHUNK_LEN: usize = 1024;

#[allow(clippy::too_many_arguments)]
fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(x);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(y);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn compress(cv: &[u32; 8], block: &[u32; 16], counter: u64, len: u32, flags: u32) -> [u32; 16] {
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        len,
        flags,
    ];
    let mut m = *block;
    for index in 0..7 {
        round(&mut state, &m);
        if index < 6 {
            let mut permuted = [0u32; 16];
            for (slot, &from) in permuted.iter_mut().zip(PERMUTATION.iter()) {
                *slot = m[from];
            }
            m = permuted;
        }
    }
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= cv[i];
    }
    state
}

fn words(block: &[u8]) -> [u32; 16] {
    let mut padded = [0u8; BLOCK_LEN];
    padded[..block.len()].copy_from_slice(block);
    let mut out = [0u32; 16];
    for (i, word) in out.iter_mut().enumerate() {
        *word = u32::from_le_bytes(padded[i * 4..i * 4 + 4].try_into().unwrap());
    }
    out
}

fn first8(state: [u32; 16]) -> [u32; 8] {
    state[..8].try_into().unwrap()
}

fn chunk_cv(chunk: &[u8], counter: u64, root: bool) -> [u32; 8] {
    let blocks = chunk.len().div_ceil(BLOCK_LEN).max(1);
    let mut cv = IV;
    for index in 0..blocks {
        let block = &chunk
            [(index * BLOCK_LEN).min(chunk.len())..((index + 1) * BLOCK_LEN).min(chunk.len())];
        let mut flags = 0;
        if index == 0 {
            flags |= CHUNK_START;
        }
        if index + 1 == blocks {
            flags |= CHUNK_END;
            if root {
                flags |= ROOT;
            }
        }
        cv = first8(compress(
            &cv,
            &words(block),
            counter,
            block.len() as u32,
            flags,
        ));
    }
    cv
}

fn subtree_cv(input: &[u8], counter: u64, root: bool) -> [u32; 8] {
    if input.len() <= CHUNK_LEN {
        return chunk_cv(input, counter, root);
    }
    let chunks = input.len().div_ceil(CHUNK_LEN);
    let left_chunks = 1usize << (usize::BITS - 1 - (chunks - 1).leading_zeros());
    let split = left_chunks * CHUNK_LEN;
    let left = subtree_cv(&input[..split], counter, false);
    let right = subtree_cv(&input[split..], counter + left_chunks as u64, false);
    let mut block = [0u32; 16];
    block[..8].copy_from_slice(&left);
    block[8..].copy_from_slice(&right);
    let flags = PARENT | if root { ROOT } else { 0 };
    first8(compress(&IV, &block, 0, BLOCK_LEN as u32, flags))
}

/// The lower-case hex BLAKE3 (32 bytes) of `input`.
pub fn blake3_hex(input: &[u8]) -> String {
    subtree_cv(input, 0, true)
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
