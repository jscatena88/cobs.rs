#![cfg_attr(not(feature = "use_std"), no_std)]

#[derive(Debug)]
pub struct CobsEncoder<'a> {
    dest: &'a mut [u8],
    dest_idx: usize,
    code_idx: usize,
    num_bt_sent: u8,
}

impl<'a> CobsEncoder<'a> {
    pub fn new(out_buf: &'a mut [u8]) -> CobsEncoder<'a> {
        CobsEncoder {
            dest: out_buf,
            dest_idx: 1,
            code_idx: 0,
            num_bt_sent: 1,
        }
    }

    pub fn push(&mut self, data: &[u8]) -> Result<(), ()> {
        // TODO: could probably check if this would fit without
        // iterating through all data
        for x in data {
            if *x == 0 {
                *self.dest.get_mut(self.code_idx)
                    .ok_or_else(|| ())? = self.num_bt_sent;

                self.num_bt_sent = 1;
                self.code_idx = self.dest_idx;
                self.dest_idx += 1;
            } else {
                *self.dest.get_mut(self.dest_idx)
                    .ok_or_else(|| ())? = *x;

                self.num_bt_sent += 1;
                self.dest_idx += 1;
                if 0xFF == self.num_bt_sent {
                    *self.dest.get_mut(self.code_idx)
                        .ok_or_else(|| ())? = self.num_bt_sent;
                    self.num_bt_sent = 1;
                    self.code_idx = self.dest_idx;
                    self.dest_idx += 1;
                }
            }
        }

        Ok(())
    }

    pub fn finalize(self) -> Result<usize, ()> {
        if self.dest_idx == 1 {
            return Ok(0);
        }

        *self.dest.get_mut(self.code_idx)
            .ok_or_else(|| ())? = self.num_bt_sent;

        return Ok(self.dest_idx);
    }
}

/// Encodes the `source` buffer into the `dest` buffer.
///
/// This function uses the typical sentinel value of 0. It returns the number of bytes
/// written to in the `dest` buffer.
///
/// # Panics
///
/// This function will panic if the `dest` buffer is not large enough for the
/// encoded message. You can calculate the size the `dest` buffer needs to be with
/// the `max_encoding_length` function.
pub fn encode(source: &[u8], dest: &mut[u8]) -> usize {
    let mut enc = CobsEncoder::new(dest);
    enc.push(source).unwrap();
    enc.finalize().unwrap()
}

/// Encodes the `source` buffer into the `dest` buffer using an
/// arbitrary sentinel value.
///
/// This is done by first encoding the message with the typical sentinel value
/// of 0, then XOR-ing each byte of the encoded message with the chosen sentinel
/// value. This will ensure that the sentinel value doesn't show up in the encoded
/// message. See the paper "Consistent Overhead Byte Stuffing" for details.
pub fn encode_with_sentinel(source: &[u8], dest: &mut[u8], sentinel: u8) -> usize {
    let encoded_size = encode(source, dest);
    for x in &mut dest[..encoded_size] {
        *x ^= sentinel;
    }
    return encoded_size;
}

#[cfg(feature = "use_std")]
/// Encodes the `source` buffer into a vector.
pub fn encode_vec(source: &[u8]) -> Vec<u8> {
    let mut encoded = vec![0; max_encoding_length(source.len())];
    let encoded_len = encode(source, &mut encoded[..]);
    encoded.truncate(encoded_len);
    return encoded;
}

#[cfg(feature = "use_std")]
/// Encodes the `source` buffer into a vector with an arbitrary sentinel value.
pub fn encode_vec_with_sentinel(source: &[u8], sentinel: u8) -> Vec<u8> {
    let mut encoded = vec![0; max_encoding_length(source.len())];
    let encoded_len = encode_with_sentinel(source, &mut encoded[..], sentinel);
    encoded.truncate(encoded_len);
    return encoded;
}

#[derive(Debug)]
pub struct CobsDecoder<'a> {
    dest: &'a mut [u8],
    dest_idx: usize,
    state: DecoderState,
}

#[derive(Debug)]
enum DecoderState {
    Idle,
    Wait,
    GrabChain(u8),
    ErrOrComplete,
}

fn add(to: &mut [u8], idx: usize, data: u8) -> Result<(), ()> {
    *to.get_mut(idx)
        .ok_or_else(|| ())? = data;
    Ok(())
}

impl<'a> CobsDecoder<'a> {
    pub fn new(dest: &'a mut [u8]) -> CobsDecoder<'a> {
        CobsDecoder {
            dest,
            dest_idx: 0,
            state: DecoderState::Idle,
        }
    }

    // fn test_encode_2() {
    //     test_pair(vec![0, 0, 1, 0], vec![1, 1, 2, 1, 1])
    // }

    pub fn feed(&mut self, data: u8) -> Result<Option<usize>, usize> {
        use DecoderState::*;
        let (ret, state) = match (&self.state, data) {
            (ErrOrComplete, _) => return Err(self.dest_idx),
            (Idle, 0) => {
                // Is an empty frame a Some?
                (Ok(None), Idle)
            },
            (Wait, 0) => {
                (Ok(Some(self.dest_idx)), ErrOrComplete)
            }
            (_, 0) => {
                (Err(self.dest_idx), ErrOrComplete)
            }
            (Idle, i) => (Ok(None), GrabChain(i)),
            (Wait, i) => (Ok(None), GrabChain(i)),
            (GrabChain(rem), d) => {
                if *rem == 1 {
                    (Ok(None), Wait)
                } else if add(self.dest, self.dest_idx, d).is_ok() {
                    self.dest_idx += 1;
                    (Ok(None), GrabChain(rem - 1))
                } else {
                    (Err(self.dest_idx), ErrOrComplete)
                }
            }

        };

        self.state = state;
        ret
    }

    pub fn push(&mut self, data: &[u8]) -> Result<Option<(usize, usize)>, ()> {
        for (i, d) in data.iter().enumerate() {
            if let Some(n) = self.feed(*d).map_err(|_| ())? {
                return Ok(Some((n, i)));
            }
        }

        Ok(None)
    }
}
    //     if self.done {
    //         return Err(());
    //     }

    //     for (i, d) in data.iter().enumerate() {
    //         if self.code.is_none() {
    //             if *d == 0 {
    //                 self.done = true;

    //                 if i < (data.len() - 1) {
    //                     return Ok(Some(if self.dest_idx == 0 { 0 } else { self.dest_idx - 1 }))
    //                 } else {
    //                     continue
    //                 }
    //             }

    //             self.code = Some(*d);
    //             self.skip = (self.dest_idx == 0) || (*d == 0xFF);
    //             continue;
    //         } else if *d == 0 {
    //             return Err(());
    //         }

    //         let end = {
    //             let c = self.code.as_mut().unwrap();
    //             *c -= 1;

    //             if *c == 0 {
    //                 true
    //             } else {
    //                 *self.dest.get_mut(self.dest_idx)
    //                     .ok_or_else(|| ())? = *d;
    //                 self.dest_idx += 1;
    //                 false
    //             }
    //         };


    //         if end {
    //             self.code = None;
    //             if !self.skip {
    //                 *self.dest.get_mut(self.dest_idx)
    //                     .ok_or_else(|| ())? = 0;
    //                 self.dest_idx += 1;
    //             }
    //         }

    //     }
    //     Ok(None)
    // }

// This needs to be a macro because `src` and `dst` could be the same or different.
macro_rules! decode_raw (
    ($src:ident, $dst:ident) => ({
        let mut source_index = 0;
        let mut dest_index = 0;

        while source_index < $src.len() {
            let code = $src[source_index];

            if source_index + code as usize > $src.len() && code != 1 {
                return Err(());
            }

            source_index += 1;

            for _ in 1..code {
                $dst[dest_index] = $src[source_index];
                source_index += 1;
                dest_index += 1;
            }

            if 0xFF != code && source_index < $src.len() {
                $dst[dest_index] = 0;
                dest_index += 1;
            }
        }

        Ok(dest_index)
    })
);

/// Decodes the `source` buffer into the `dest` buffer.
///
/// This function uses the typical sentinel value of 0.
///
/// # Failures
///
/// This will return `Err(())` if there was a decoding error. Otherwise,
/// it will return `Ok(n)` where `n` is the length of the decoded message.
///
/// # Panics
///
/// This function will panic if the `dest` buffer is not large enough for the
/// decoded message. Since an encoded message as always larger than a decoded
/// message, it may be a good idea to make the `dest` buffer as big as the
/// `source` buffer.
pub fn decode(source: &[u8], dest: &mut[u8]) -> Result<usize, ()> {
    // decode_raw!(source, dest)
    let mut dec = CobsDecoder::new(dest);
    assert!(dec.push(source).unwrap().is_none());
    println!("{:?}", dec);
    if let Some((d_used, s_used)) = dec.push(&[0]).unwrap() {
        if s_used != source.len() {
            Err(())
        } else {
            Ok(d_used)
        }
    } else {
        Err(())
    }
}

/// Decodes a message in-place.
///
/// This is the same function as `decode`, but replaces the encoded message
/// with the decoded message instead of writing to another buffer.
pub fn decode_in_place(buff: &mut[u8]) -> Result<usize, ()> {
    decode_raw!(buff, buff)
}

/// Decodes the `source` buffer into the `dest` buffer using an arbitrary sentinel value.
///
/// This is done by XOR-ing each byte of the source message with the chosen sentinel value,
/// which transforms the message into the same message encoded with a sentinel value of 0.
/// Then the regular decoding transformation is performed.
pub fn decode_with_sentinel(source: &[u8], dest: &mut[u8], sentinel: u8) -> Result<usize, ()> {
    for (x, y) in source.iter().zip(dest.iter_mut()) {
        *y = *x ^ sentinel;
    }
    decode_in_place(dest)
}

/// Decodes a message in-place using an arbitrary sentinel value.
pub fn decode_in_place_with_sentinel(buff: &mut[u8], sentinel: u8) -> Result<usize, ()> {
    for x in buff.iter_mut() {
        *x ^= sentinel;
    }
    decode_in_place(buff)
}

#[cfg(feature = "use_std")]
/// Decodes the `source` buffer into a vector.
pub fn decode_vec(source: &[u8]) -> Result<Vec<u8>, ()> {
    let mut decoded = vec![0; source.len()];
    match decode(source, &mut decoded[..]) {
        Ok(n) => {
            decoded.truncate(n);
            Ok(decoded)
        },
        Err(()) => Err(()),
    }
}

#[cfg(feature = "use_std")]
/// Decodes the `source` buffer into a vector with an arbitrary sentinel value.
pub fn decode_vec_with_sentinel(source: &[u8], sentinel: u8) -> Result<Vec<u8>, ()> {
    let mut decoded = vec![0; source.len()];
    match decode_with_sentinel(source, &mut decoded[..], sentinel) {
        Ok(n) => {
            decoded.truncate(n);
            Ok(decoded)
        },
        Err(()) => Err(()),
    }
}

/// Calculates the maximum possible size of an encoded message given the length
/// of the source message. This may be useful for calculating how large the
/// `dest` buffer needs to be in the encoding functions.
pub fn max_encoding_length(source_len: usize) -> usize {
    source_len + (source_len / 254) + if source_len % 254 > 0 { 1 } else { 0 }
}
