//! The Part 10 reader: the preamble, the file meta group (always explicit VR
//! little endian), then the data set in its transfer syntax, up to Pixel
//! Data.

use std::io::{self, Read};

use crate::error::DicomError;
use crate::header::{Dataset, Element, Header, Value};
use crate::tag::{implicit_vr, tags, Tag, Vr};
use crate::{transfer_syntax, MAX_DEPTH, MAX_HEADER_BYTES, MAX_INFLATED_BYTES, MAX_VALUE_BYTES};

const UNDEFINED: u32 = 0xFFFF_FFFF;

/// What each element and item counts against [`MAX_HEADER_BYTES`] besides
/// its value: about what it costs in memory, so a file of empty elements
/// is bounded as one of large values is.
const ELEMENT_COST: u64 = 128;

/// Why the inflated data set stopped: marked so it reads as a DICOM error,
/// not an I/O one.
#[derive(Debug)]
enum InflateError {
    TooLarge,
    Corrupt,
}

impl std::fmt::Display for InflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InflateError::TooLarge => "the deflated data set inflates past its limit",
            InflateError::Corrupt => "the deflated data set does not inflate",
        })
    }
}

impl std::error::Error for InflateError {}

/// An error of the file under a deflated data set, marked so that
/// [`Inflating`] can tell it from the decoder's own and hand it back as it
/// was.
#[derive(Debug)]
struct SourceError(io::Error);

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for SourceError {}

/// The file under a deflated data set, its errors marked.
struct Source<R: Read>(R);

impl<R: Read> Read for Source<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0
            .read(buf)
            .map_err(|e| io::Error::new(e.kind(), SourceError(e)))
    }
}

/// A deflated data set, inflated up to [`MAX_INFLATED_BYTES`].
struct Inflating<R: Read> {
    inner: flate2::read::DeflateDecoder<Source<R>>,
    out: u64,
}

impl<R: Read> Inflating<R> {
    fn new(file: R) -> Self {
        Inflating {
            inner: flate2::read::DeflateDecoder::new(Source(file)),
            out: 0,
        }
    }
}

impl<R: Read> Read for Inflating<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.out >= MAX_INFLATED_BYTES {
            return Err(io::Error::other(InflateError::TooLarge));
        }
        let room = (MAX_INFLATED_BYTES - self.out).min(buf.len() as u64) as usize;
        let n = match self.inner.read(&mut buf[..room]) {
            Ok(n) => n,
            // The file's own error, as it was: a failing disc is I/O, not
            // a malformed file.
            Err(e) if e.get_ref().is_some_and(|i| i.is::<SourceError>()) => {
                return Err(e
                    .into_inner()
                    .and_then(|i| i.downcast::<SourceError>().ok())
                    .map_or_else(|| io::ErrorKind::Other.into(), |s| s.0));
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => return Err(e),
            // The stream stops before its end: the data set ends there, and
            // reads as a plain file cut at the same place does.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => 0,
            Err(_) => return Err(io::Error::other(InflateError::Corrupt)),
        };
        self.out += n as u64;
        Ok(n)
    }
}

/// An I/O error as a DICOM error: the inflater's own are a limit or a
/// malformed file.
fn io_error(e: io::Error, offset: u64) -> DicomError {
    match e.get_ref().and_then(|i| i.downcast_ref::<InflateError>()) {
        Some(InflateError::TooLarge) => DicomError::Limit {
            offset,
            what: "a deflated data set inflating past 256 MiB",
        },
        Some(InflateError::Corrupt) => {
            malformed(offset, "a deflated data set that does not inflate")
        }
        None => DicomError::Io(e),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Syntax {
    pub explicit: bool,
    pub big_endian: bool,
}

pub(crate) const EXPLICIT_LE: Syntax = Syntax {
    explicit: true,
    big_endian: false,
};
const IMPLICIT_LE: Syntax = Syntax {
    explicit: false,
    big_endian: false,
};

/// A reader that counts the bytes it has given out and can take some back.
struct Input<'a> {
    inner: Box<dyn Read + 'a>,
    back: Vec<u8>,
    back_at: usize,
    pos: u64,
}

impl<'a> Input<'a> {
    fn new(inner: impl Read + 'a) -> Self {
        Input {
            inner: Box::new(inner),
            back: Vec::new(),
            back_at: 0,
            pos: 0,
        }
    }

    /// Up to `buf.len()` bytes; fewer only at the end of the input.
    fn fill(&mut self, buf: &mut [u8]) -> Result<usize, DicomError> {
        let mut got = 0;
        while got < buf.len() {
            match self.read(&mut buf[got..]) {
                Ok(0) => break,
                Ok(n) => got += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error(e, self.pos)),
            }
        }
        Ok(got)
    }

    fn exact(&mut self, buf: &mut [u8], began: u64) -> Result<(), DicomError> {
        if self.fill(buf)? < buf.len() {
            return Err(DicomError::Truncated { offset: began });
        }
        Ok(())
    }

    fn unread(&mut self, bytes: &[u8]) {
        if self.back_at >= bytes.len() {
            // What was taken back was the end of what was handed out.
            self.back_at -= bytes.len();
            self.back[self.back_at..self.back_at + bytes.len()].copy_from_slice(bytes);
        } else if self.back_at == self.back.len() {
            // Nothing pushed back is left: reuse the buffer.
            self.back.clear();
            self.back.extend_from_slice(bytes);
            self.back_at = 0;
        } else {
            let mut back = bytes.to_vec();
            back.extend_from_slice(&self.back[self.back_at..]);
            self.back = back;
            self.back_at = 0;
        }
        self.pos -= bytes.len() as u64;
    }

    fn at_end(&mut self) -> Result<bool, DicomError> {
        let mut one = [0u8; 1];
        if self.fill(&mut one)? == 0 {
            return Ok(true);
        }
        self.unread(&one);
        Ok(false)
    }

    fn u16(&mut self, be: bool, began: u64) -> Result<u16, DicomError> {
        let mut b = [0u8; 2];
        self.exact(&mut b, began)?;
        Ok(if be {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    }

    fn u32(&mut self, be: bool, began: u64) -> Result<u32, DicomError> {
        let mut b = [0u8; 4];
        self.exact(&mut b, began)?;
        Ok(if be {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    }

    fn skip(&mut self, n: u64, began: u64) -> Result<(), DicomError> {
        let copied = io::copy(&mut self.by_ref().take(n), &mut io::sink())
            .map_err(|e| io_error(e, began))?;
        if copied < n {
            return Err(DicomError::Truncated { offset: began });
        }
        Ok(())
    }

    /// `n` bytes, read without trusting `n` for an allocation.
    fn value_bytes(&mut self, n: u64, began: u64) -> Result<Vec<u8>, DicomError> {
        let mut value = Vec::new();
        self.by_ref()
            .take(n)
            .read_to_end(&mut value)
            .map_err(|e| io_error(e, began))?;
        if (value.len() as u64) < n {
            return Err(DicomError::Truncated { offset: began });
        }
        Ok(value)
    }

    /// The unread input, pushed-back bytes first.
    fn rest(self) -> Box<dyn Read + 'a> {
        let back = self.back[self.back_at..].to_vec();
        Box::new(io::Cursor::new(back).chain(self.inner))
    }
}

impl Read for Input<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = if self.back_at < self.back.len() {
            let n = buf.len().min(self.back.len() - self.back_at);
            buf[..n].copy_from_slice(&self.back[self.back_at..self.back_at + n]);
            self.back_at += n;
            n
        } else {
            self.inner.read(buf)?
        };
        self.pos += n as u64;
        Ok(n)
    }
}

enum End {
    /// The end of the input.
    Input,
    /// A byte offset: a defined-length item.
    At(u64),
    /// An Item Delimitation: an undefined-length item.
    Delimiter,
}

struct Parser<'a> {
    input: Input<'a>,
    retained: u64,
}

fn malformed(offset: u64, what: impl Into<String>) -> DicomError {
    DicomError::Malformed {
        offset,
        what: what.into(),
    }
}

impl<'a> Parser<'a> {
    /// Count `n` bytes against [`MAX_HEADER_BYTES`].
    fn charge(&mut self, n: u64, offset: u64) -> Result<(), DicomError> {
        self.retained = self.retained.saturating_add(n);
        if self.retained > MAX_HEADER_BYTES {
            return Err(DicomError::Limit {
                offset,
                what: "a header holding over 64 MiB before Pixel Data",
            });
        }
        Ok(())
    }

    fn tag(&mut self, syntax: Syntax, began: u64) -> Result<Tag, DicomError> {
        let group = self.input.u16(syntax.big_endian, began)?;
        let element = self.input.u16(syntax.big_endian, began)?;
        Ok(Tag(group, element))
    }

    fn vr_and_length(
        &mut self,
        tag: Tag,
        syntax: Syntax,
        began: u64,
    ) -> Result<(Vr, u32), DicomError> {
        if !syntax.explicit {
            return Ok((implicit_vr(tag), self.input.u32(syntax.big_endian, began)?));
        }
        let mut code = [0u8; 2];
        self.input.exact(&mut code, began)?;
        let vr = Vr::read(code).ok_or_else(|| {
            malformed(began, format!("element {tag} has no value representation"))
        })?;
        let length = if vr.has_long_length() {
            self.input.skip(2, began)?;
            self.input.u32(syntax.big_endian, began)?
        } else {
            u32::from(self.input.u16(syntax.big_endian, began)?)
        };
        Ok((vr, length))
    }

    /// Elements until `end`. At the top level (`top`), reading stops at
    /// Pixel Data and the second value says so.
    fn dataset(
        &mut self,
        syntax: Syntax,
        end: End,
        depth: usize,
        top: bool,
        offset: u64,
    ) -> Result<(Dataset, bool), DicomError> {
        let mut dataset = Dataset {
            offset,
            ..Dataset::default()
        };
        loop {
            match end {
                End::At(at) if self.input.pos == at => break,
                End::At(at) if self.input.pos > at => {
                    return Err(malformed(
                        self.input.pos,
                        "an element runs past the end of its item",
                    ))
                }
                End::Input if self.input.at_end()? => break,
                _ => {}
            }
            let began = self.input.pos;
            let tag = self.tag(syntax, began)?;
            if tag == tags::ITEM_DELIMITATION {
                self.input.u32(syntax.big_endian, began)?;
                if matches!(end, End::Delimiter) {
                    break;
                }
                return Err(malformed(began, "an item delimiter outside an item"));
            }
            if tag == tags::ITEM || tag == tags::SEQUENCE_DELIMITATION {
                return Err(malformed(began, "a sequence marker outside a sequence"));
            }
            if top && tag.0 == 0x7FE0 && matches!(tag.1, 0x0008 | 0x0009 | 0x0010) {
                return Ok((dataset, true));
            }
            let (vr, length) = self.vr_and_length(tag, syntax, began)?;
            self.charge(ELEMENT_COST, began)?;
            let value = self.value(tag, vr, length, syntax, depth, began)?;
            dataset.insert(Element {
                tag,
                vr,
                value,
                big_endian: syntax.big_endian,
            });
        }
        Ok((dataset, false))
    }

    fn value(
        &mut self,
        tag: Tag,
        vr: Vr,
        length: u32,
        syntax: Syntax,
        depth: usize,
        began: u64,
    ) -> Result<Value, DicomError> {
        if length == UNDEFINED {
            if vr == Vr::SQ {
                return Ok(Value::Sequence(self.items(syntax, None, depth + 1)?));
            }
            if vr == Vr::UN {
                // PS3.5 §6.2.2: an unknown element of undefined length is a
                // sequence in implicit VR little endian.
                return Ok(Value::Sequence(self.items(IMPLICIT_LE, None, depth + 1)?));
            }
            if vr.is_bulk() || tag == tags::PIXEL_DATA {
                self.fragments(syntax, began)?;
                return Ok(Value::Encapsulated);
            }
            return Err(malformed(
                began,
                format!("element {tag} has an undefined length"),
            ));
        }
        let length = u64::from(length);
        if vr == Vr::SQ {
            let end = self.input.pos.saturating_add(length);
            return Ok(Value::Sequence(self.items(syntax, Some(end), depth + 1)?));
        }
        if length > MAX_VALUE_BYTES {
            if vr.is_bulk() {
                self.input.skip(length, began)?;
                return Ok(Value::Skipped { length });
            }
            return Err(DicomError::Limit {
                offset: began,
                what: "a text or number element over 16 MiB",
            });
        }
        self.charge(length, began)?;
        Ok(Value::Bytes(self.input.value_bytes(length, began)?))
    }

    /// A sequence's items, up to `end` or to the Sequence Delimitation.
    fn items(
        &mut self,
        syntax: Syntax,
        end: Option<u64>,
        depth: usize,
    ) -> Result<Vec<Dataset>, DicomError> {
        if depth > MAX_DEPTH {
            return Err(DicomError::Limit {
                offset: self.input.pos,
                what: "sequences nested more than 16 deep",
            });
        }
        let mut items = Vec::new();
        loop {
            if let Some(end) = end {
                if self.input.pos == end {
                    break;
                }
                if self.input.pos > end {
                    return Err(malformed(
                        self.input.pos,
                        "an item runs past the end of its sequence",
                    ));
                }
            }
            let began = self.input.pos;
            let tag = self.tag(syntax, began)?;
            let length = self.input.u32(syntax.big_endian, began)?;
            if tag == tags::ITEM {
                self.charge(ELEMENT_COST, began)?;
            }
            match tag {
                tags::SEQUENCE_DELIMITATION if end.is_none() => break,
                tags::ITEM if length == UNDEFINED => {
                    let (item, _) = self.dataset(syntax, End::Delimiter, depth, false, began)?;
                    items.push(item);
                }
                tags::ITEM => {
                    let item_end = self.input.pos.saturating_add(u64::from(length));
                    if end.is_some_and(|end| item_end > end) {
                        return Err(malformed(
                            began,
                            "an item runs past the end of its sequence",
                        ));
                    }
                    let (item, _) = self.dataset(syntax, End::At(item_end), depth, false, began)?;
                    items.push(item);
                }
                _ => {
                    return Err(malformed(
                        began,
                        "a sequence holds something other than items",
                    ))
                }
            }
        }
        Ok(items)
    }

    /// Encapsulated fragments, skipped to their Sequence Delimitation.
    fn fragments(&mut self, syntax: Syntax, began: u64) -> Result<(), DicomError> {
        loop {
            let at = self.input.pos;
            let tag = self.tag(syntax, at)?;
            let length = self.input.u32(syntax.big_endian, at)?;
            match tag {
                tags::SEQUENCE_DELIMITATION => return Ok(()),
                tags::ITEM if length != UNDEFINED => self.input.skip(u64::from(length), at)?,
                _ => return Err(malformed(began, "encapsulated data that is not fragments")),
            }
        }
    }
}

/// Whether these bytes begin a DICOM file: `DICM` after a 128-byte preamble,
/// or a file meta group with no preamble. Give it the first 132 bytes.
pub fn is_dicom(prefix: &[u8]) -> bool {
    prefix.get(128..132) == Some(b"DICM") || starts_with_meta(prefix)
}

/// A file meta group at byte 0: tag (0002,xxxx) little endian, then a VR.
fn starts_with_meta(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && bytes[..2] == [0x02, 0x00] && Vr::read([bytes[4], bytes[5]]).is_some()
}

/// Read a Part 10 file's header from `reader`: the file meta information
/// and the data set, stopping at Pixel Data, which is never decoded. Reads
/// through a buffer of its own, so a file can be passed as it is opened.
pub fn read_header<R: Read>(reader: R) -> Result<Header, DicomError> {
    let mut input = Input::new(io::BufReader::new(reader));
    let mut lead = [0u8; 132];
    let got = input.fill(&mut lead)?;
    if got == 132 && &lead[128..132] == b"DICM" {
        // The preamble is passed; offsets keep counting from the file's start.
    } else if starts_with_meta(&lead[..got]) {
        input.unread(&lead[..got]);
    } else {
        return Err(DicomError::NotDicom);
    }
    let mut parser = Parser { input, retained: 0 };

    let mut meta = Dataset::default();
    let mut meta_end = None;
    loop {
        if meta_end.is_some_and(|end| parser.input.pos >= end) {
            break;
        }
        let mut group = [0u8; 2];
        let n = parser.input.fill(&mut group)?;
        parser.input.unread(&group[..n]);
        if n < 2 || u16::from_le_bytes(group) != 0x0002 {
            break;
        }
        let began = parser.input.pos;
        let tag = parser.tag(EXPLICIT_LE, began)?;
        let (vr, length) = parser.vr_and_length(tag, EXPLICIT_LE, began)?;
        parser.charge(ELEMENT_COST, began)?;
        let value = parser.value(tag, vr, length, EXPLICIT_LE, 0, began)?;
        let element = Element {
            tag,
            vr,
            value,
            big_endian: false,
        };
        if tag == tags::FILE_META_GROUP_LENGTH {
            // A length that is negative or overflows is ignored, and the
            // meta group is read while its tags are group 0002.
            meta_end = element
                .int()
                .and_then(|n| u64::try_from(n).ok())
                .and_then(|n| parser.input.pos.checked_add(n));
        }
        meta.insert(element);
    }
    let transfer_syntax = meta.string(tags::TRANSFER_SYNTAX_UID).ok_or_else(|| {
        malformed(
            parser.input.pos,
            "the file meta information names no transfer syntax",
        )
    })?;

    let syntax = match transfer_syntax.as_str() {
        transfer_syntax::IMPLICIT_VR_LITTLE_ENDIAN => IMPLICIT_LE,
        transfer_syntax::EXPLICIT_VR_BIG_ENDIAN => Syntax {
            explicit: true,
            big_endian: true,
        },
        // Every other syntax, the compressed ones included, encodes the data
        // set in explicit VR little endian; only Pixel Data differs.
        _ => EXPLICIT_LE,
    };
    if transfer_syntax == transfer_syntax::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN {
        let rest = parser.input.rest();
        parser.input = Input::new(Inflating::new(rest));
    }
    let start = parser.input.pos;
    let (dataset, has_pixel_data) = parser.dataset(syntax, End::Input, 0, true, start)?;
    Ok(Header::new(meta, dataset, transfer_syntax, has_pixel_data))
}
