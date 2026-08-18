//! Disposable F-004 prototype.
//!
//! This file deliberately uses only the Rust standard library. It compares
//! three byte-to-Unicode-scalar lookup shapes without selecting a production
//! representation or depending on a parser binding.

use std::hint::black_box;
use std::mem::size_of;
use std::time::Instant;

const CHECKPOINT_STRIDE: usize = 64;
const LOOKUPS: usize = 2_048;
const REPEATS: usize = 3;

#[derive(Clone, Copy)]
enum FixtureKind {
    Ascii,
    Utf8,
    Invalid,
}

#[derive(Clone, Copy)]
enum Locality {
    Sequential,
    Random,
    Hot,
}

impl Locality {
    fn name(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Random => "random",
            Self::Hot => "hot",
        }
    }
}

#[derive(Clone, Copy)]
struct FixtureSpec {
    name: &'static str,
    target_bytes: usize,
    line_bytes: usize,
    kind: FixtureKind,
}

#[derive(Clone)]
struct Fixture {
    bytes: Vec<u8>,
    boundaries: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    byte: usize,
    column: u32,
}

struct FullMap {
    line_starts: Vec<usize>,
    columns: Vec<u32>,
}

struct CheckpointMap {
    line_starts: Vec<usize>,
    checkpoints: Vec<Checkpoint>,
}

struct ScanMap {
    line_starts: Vec<usize>,
}

enum Index {
    Full(FullMap),
    Checkpoint(CheckpointMap),
    Scan(ScanMap),
}

impl Index {
    fn name(&self) -> &'static str {
        match self {
            Self::Full(_) => "full-map",
            Self::Checkpoint(_) => "checkpoint",
            Self::Scan(_) => "line-scan",
        }
    }

    fn estimated_bytes(&self) -> usize {
        match self {
            Self::Full(index) => {
                index.line_starts.len() * size_of::<usize>()
                    + index.columns.len() * size_of::<u32>()
            }
            Self::Checkpoint(index) => {
                index.line_starts.len() * size_of::<usize>()
                    + index.checkpoints.len() * size_of::<Checkpoint>()
            }
            Self::Scan(index) => index.line_starts.len() * size_of::<usize>(),
        }
    }

    fn lookup(&self, bytes: &[u8], offset: usize) -> (usize, u32) {
        match self {
            Self::Full(index) => {
                let line = line_of(&index.line_starts, offset);
                (line, index.columns[offset])
            }
            Self::Checkpoint(index) => {
                let line = line_of(&index.line_starts, offset);
                let checkpoint = upper_bound_checkpoint(&index.checkpoints, offset);
                let (mut cursor, mut column) = (
                    index.checkpoints[checkpoint].byte,
                    index.checkpoints[checkpoint].column,
                );
                while cursor < offset {
                    if bytes[cursor] == b'\n' {
                        column = 0;
                        cursor += 1;
                    } else {
                        cursor += scalar_width(bytes, cursor);
                        column += 1;
                    }
                }
                (line, column)
            }
            Self::Scan(index) => {
                let line = line_of(&index.line_starts, offset);
                let mut cursor = index.line_starts[line];
                let mut column = 0;
                while cursor < offset {
                    if bytes[cursor] == b'\n' {
                        column = 0;
                        cursor += 1;
                    } else {
                        cursor += scalar_width(bytes, cursor);
                        column += 1;
                    }
                }
                (line, column)
            }
        }
    }
}

fn main() {
    let specs = [
        FixtureSpec {
            name: "ascii-short",
            target_bytes: 256 * 1024,
            line_bytes: 80,
            kind: FixtureKind::Ascii,
        },
        FixtureSpec {
            name: "utf8-short",
            target_bytes: 256 * 1024,
            line_bytes: 80,
            kind: FixtureKind::Utf8,
        },
        FixtureSpec {
            name: "invalid-short",
            target_bytes: 256 * 1024,
            line_bytes: 80,
            kind: FixtureKind::Invalid,
        },
        FixtureSpec {
            name: "ascii-long",
            target_bytes: 256 * 1024,
            line_bytes: 4_096,
            kind: FixtureKind::Ascii,
        },
        FixtureSpec {
            name: "utf8-long",
            target_bytes: 256 * 1024,
            line_bytes: 4_096,
            kind: FixtureKind::Utf8,
        },
        FixtureSpec {
            name: "invalid-long",
            target_bytes: 256 * 1024,
            line_bytes: 4_096,
            kind: FixtureKind::Invalid,
        },
    ];

    println!(
        "fixture\tbytes\tboundaries\tshape\tlocality\tbuild_us\tlookup_us\testimated_bytes\tchecksum"
    );

    for spec in specs {
        let fixture = make_fixture(spec);
        let full = timed_build(|| Index::Full(build_full(&fixture.bytes)));
        let checkpoint =
            timed_build(|| Index::Checkpoint(build_checkpoint(&fixture.bytes, CHECKPOINT_STRIDE)));
        let scan = timed_build(|| Index::Scan(build_scan(&fixture.bytes)));

        assert_equivalent(&fixture, [&full.1, &checkpoint.1, &scan.1]);

        for (build_us, index) in [full, checkpoint, scan] {
            for locality in [Locality::Sequential, Locality::Random, Locality::Hot] {
                let offsets = choose_offsets(&fixture.boundaries, locality);
                let started = Instant::now();
                let mut checksum = 0_u64;
                for _ in 0..REPEATS {
                    for &offset in &offsets {
                        let (line, column) = index.lookup(&fixture.bytes, black_box(offset));
                        checksum = checksum
                            .wrapping_add((line as u64).wrapping_mul(1_000_003))
                            .wrapping_add(column as u64);
                    }
                }
                let lookup_us = started.elapsed().as_micros();
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    spec.name,
                    fixture.bytes.len(),
                    fixture.boundaries.len(),
                    index.name(),
                    locality.name(),
                    build_us,
                    lookup_us,
                    index.estimated_bytes(),
                    checksum
                );
            }
        }
    }
}

fn timed_build(build: impl FnOnce() -> Index) -> (u128, Index) {
    let started = Instant::now();
    let index = build();
    (started.elapsed().as_micros(), index)
}

fn make_fixture(spec: FixtureSpec) -> Fixture {
    let mut bytes = Vec::with_capacity(spec.target_bytes);
    let mut line_bytes = 0;
    let mut token = 0;
    while bytes.len() < spec.target_bytes {
        if line_bytes >= spec.line_bytes {
            bytes.push(b'\n');
            line_bytes = 0;
            continue;
        }

        let value: &[u8] = match spec.kind {
            FixtureKind::Ascii => b"a",
            FixtureKind::Utf8 => match token % 4 {
                0 => "a".as_bytes(),
                1 => "é".as_bytes(),
                2 => "日".as_bytes(),
                _ => "😀".as_bytes(),
            },
            FixtureKind::Invalid => match token % 4 {
                0 => b"a",
                1 => &[0xff],
                2 => &[0xfe],
                _ => &[0x80],
            },
        };
        token += 1;
        if line_bytes + value.len() > spec.line_bytes {
            bytes.push(b'\n');
            line_bytes = 0;
        } else {
            bytes.extend_from_slice(value);
            line_bytes += value.len();
        }
    }
    let boundaries = scalar_boundaries(&bytes);
    Fixture { bytes, boundaries }
}

fn line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = vec![0];
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte == b'\n' {
            starts.push(offset + 1);
        }
    }
    starts
}

fn build_full(bytes: &[u8]) -> FullMap {
    let line_starts = line_starts(bytes);
    let mut columns = vec![0_u32; bytes.len() + 1];
    let mut cursor = 0;
    let mut column = 0_u32;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\n' {
            columns[cursor] = column;
            cursor += 1;
            column = 0;
            columns[cursor] = 0;
            continue;
        }
        let width = scalar_width(bytes, cursor);
        let end = (cursor + width).min(bytes.len());
        for value in &mut columns[cursor..end] {
            *value = column;
        }
        column += 1;
        columns[end] = column;
        cursor = end;
    }
    FullMap {
        line_starts,
        columns,
    }
}

fn build_checkpoint(bytes: &[u8], stride: usize) -> CheckpointMap {
    let line_starts = line_starts(bytes);
    let mut checkpoints = Vec::new();
    let mut line_start = 0;
    let mut cursor = 0;
    let mut column = 0_u32;
    let mut last_checkpoint = 0;
    while cursor < bytes.len() {
        if cursor == line_start {
            checkpoints.push(Checkpoint {
                byte: cursor,
                column,
            });
            last_checkpoint = cursor;
        } else if cursor - last_checkpoint >= stride {
            checkpoints.push(Checkpoint {
                byte: cursor,
                column,
            });
            last_checkpoint = cursor;
        }

        if bytes[cursor] == b'\n' {
            cursor += 1;
            line_start = cursor;
            column = 0;
            last_checkpoint = cursor;
            continue;
        }
        cursor += scalar_width(bytes, cursor);
        column += 1;
    }
    checkpoints.push(Checkpoint {
        byte: bytes.len(),
        column,
    });
    CheckpointMap {
        line_starts,
        checkpoints,
    }
}

fn build_scan(bytes: &[u8]) -> ScanMap {
    ScanMap {
        line_starts: line_starts(bytes),
    }
}

fn scalar_boundaries(bytes: &[u8]) -> Vec<usize> {
    let mut boundaries = vec![0];
    let mut cursor = 0;
    while cursor < bytes.len() {
        cursor += if bytes[cursor] == b'\n' {
            1
        } else {
            scalar_width(bytes, cursor)
        };
        boundaries.push(cursor);
    }
    boundaries
}

fn choose_offsets(boundaries: &[usize], locality: Locality) -> Vec<usize> {
    let usable = boundaries.len().saturating_sub(1).max(1);
    (0..LOOKUPS)
        .map(|index| match locality {
            Locality::Sequential => boundaries[index % usable],
            Locality::Hot => boundaries[index % usable.min(32)],
            Locality::Random => {
                let mut value = (index as u64).wrapping_add(0x9e37_79b9);
                value ^= value << 7;
                value ^= value >> 9;
                boundaries[(value as usize) % usable]
            }
        })
        .collect()
}

fn assert_equivalent(fixture: &Fixture, indexes: [&Index; 3]) {
    for &offset in &fixture.boundaries {
        let expected = indexes[0].lookup(&fixture.bytes, offset);
        for index in &indexes[1..] {
            assert_eq!(expected, index.lookup(&fixture.bytes, offset));
        }
    }
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    let mut low = 0;
    let mut high = starts.len();
    while low + 1 < high {
        let middle = (low + high) / 2;
        if starts[middle] <= offset {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

fn upper_bound_checkpoint(checkpoints: &[Checkpoint], offset: usize) -> usize {
    let mut low = 0;
    let mut high = checkpoints.len();
    while low + 1 < high {
        let middle = (low + high) / 2;
        if checkpoints[middle].byte <= offset {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

fn scalar_width(bytes: &[u8], offset: usize) -> usize {
    if let Some(width) = valid_width(bytes, offset) {
        return width;
    }
    let mut end = offset + 1;
    while end < bytes.len() && valid_width(bytes, end).is_none() {
        end += 1;
    }
    end - offset
}

fn valid_width(bytes: &[u8], offset: usize) -> Option<usize> {
    let first = *bytes.get(offset)?;
    let (width, minimum, maximum) = match first {
        0x00..=0x7f => return Some(1),
        0xc2..=0xdf => (2, 0x80, 0x7ff),
        0xe0..=0xef => (3, 0x800, 0xffff),
        0xf0..=0xf4 => (4, 0x1_0000, 0x10_ffff),
        _ => return None,
    };
    if offset + width > bytes.len() {
        return None;
    }
    let mut codepoint = (first & (0x7f >> width)) as u32;
    for byte in &bytes[offset + 1..offset + width] {
        if byte & 0xc0 != 0x80 {
            return None;
        }
        codepoint = (codepoint << 6) | (byte & 0x3f) as u32;
    }
    if (minimum..=maximum).contains(&codepoint) && !(0xd800..=0xdfff).contains(&codepoint) {
        Some(width)
    } else {
        None
    }
}
