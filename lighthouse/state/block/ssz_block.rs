use super::ssz::decode::{
    decode_length,
    Decodable,
};
use super::utils::hash::canonical_hash;
use super::block::{
    MIN_SSZ_BLOCK_LENGTH,
    MAX_SSZ_BLOCK_LENGTH,
};
use super::attestation_record::MIN_SSZ_ATTESTION_RECORD_LENGTH;

#[derive(Debug, PartialEq)]
pub enum BlockValidatorError {
    TooShort,
    TooLong,
    BadPowHash,
    SlotTooLow,
    SlotTooHigh,
}

const LENGTH_BYTES: usize = 4;

/// Allows for reading of block values directly from serialized
/// ssz bytes.
///
/// Use this to perform intial checks before we fully de-serialize
/// a block. It should only really be used to verify blocks that come
/// in from the network, for internal operations we should use a full
/// `Block`.
#[derive(Debug, PartialEq)]
pub struct SszBlock<'a> {
    ssz: &'a [u8],
    attestation_len: usize,
    pub len: usize,
}

impl<'a> SszBlock<'a> {
    /// Create a new instance from a slice reference.
    ///
    /// This function will validate the length of the ssz
    /// string, however it will not validate the contents.
    ///
    /// The slice will not be copied, instead referenced.
    pub fn from_slice(vec: &'a [u8])
        -> Result<Self, BlockValidatorError>
    {
        let untrimmed_ssz = &vec[..];
        /*
         * Ensure the SSZ is long enough to be a block with
         * one attestation record (not necessarily a valid
         * attestation record).
         */
        if vec.len() < MIN_SSZ_BLOCK_LENGTH + MIN_SSZ_ATTESTION_RECORD_LENGTH {
            return Err(BlockValidatorError::TooShort);
        }
        /*
         * Ensure the SSZ slice isn't longer than is possible for a block.
         */
        if vec.len() > MAX_SSZ_BLOCK_LENGTH {
            return Err(BlockValidatorError::TooLong);
        }
        /*
         * Determine how many bytes are used to store attestation records.
         */
        let attestation_len = decode_length(untrimmed_ssz, 80, LENGTH_BYTES)
            .map_err(|_| BlockValidatorError::TooShort)?;
        /*
         * The block only has one variable field, `attestations`, therefore
         * the size of the block must be the minimum size, plus the length
         * of the attestations.
         */
        let block_ssz_len = {
            MIN_SSZ_BLOCK_LENGTH + attestation_len
        };
        if vec.len() < block_ssz_len {
            return Err(BlockValidatorError::TooShort);
        }
        Ok(Self{
            ssz: &untrimmed_ssz[0..block_ssz_len],
            attestation_len,
            len: block_ssz_len,
        })
    }

    /// Return the canonical hash for this block.
    pub fn block_hash(&self) -> Vec<u8> {
        canonical_hash(self.ssz)
    }

    /// Return the `parent_hash` field.
    pub fn parent_hash(&self) -> &[u8] {
        &self.ssz[4..36]
    }

    /// Return the `slot_number` field.
    pub fn slot_number(&self) -> u64 {
        /*
         * An error should be unreachable from this decode
         * because we checked the length of the array at
         * the initalization of this struct.
         *
         * If you can make this function panic, please report
         * it to paul@sigmaprime.io
         */
        if let Ok((n, _)) = u64::ssz_decode(&self.ssz, 36) {
            n
        } else {
            unreachable!();
        }
    }

    /// Return the `randao_reveal` field.
    pub fn randao_reveal(&self) -> &[u8] {
        &self.ssz[48..80]
    }

    /// Return the `attestations` field.
    pub fn attestations(&self) -> &[u8] {
        let start = 80 + LENGTH_BYTES;
        &self.ssz[start..(start + self.attestation_len)]
    }

    /// Return the `pow_chain_ref` field.
    pub fn pow_chain_ref(&self) -> &[u8] {
        let start = self.len - (32 + LENGTH_BYTES + 32 + LENGTH_BYTES + 32);
        &self.ssz[start..(start + 32)]
    }

    /// Return the `active_state_root` field.
    pub fn act_state_root(&self) -> &[u8] {
        let start = self.len - (32 + LENGTH_BYTES + 32);
        &self.ssz[start..(start + 32)]
    }

    /// Return the `active_state_root` field.
    pub fn cry_state_root(&self) -> &[u8] {
        let start = self.len - 32;
        &self.ssz[start..(start + 32)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::block::Block;
    use super::super::attestation_record::AttestationRecord;
    use super::super::ssz::SszStream;
    use super::super::utils::types::Hash256;

    fn get_block_ssz(b: &Block) -> Vec<u8> {
        let mut ssz_stream = SszStream::new();
        ssz_stream.append(b);
        ssz_stream.drain()
    }

    fn get_attestation_record_ssz(ar: &AttestationRecord) -> Vec<u8> {
        let mut ssz_stream = SszStream::new();
        ssz_stream.append(ar);
        ssz_stream.drain()
    }

    #[test]
    fn test_ssz_block_zero_attestation_records() {
        let mut b = Block::zero();
        b.attestations = vec![];
        let ssz = get_block_ssz(&b);

        assert_eq!(
            SszBlock::from_slice(&ssz[..]),
            Err(BlockValidatorError::TooShort)
        );
    }

    #[test]
    fn test_ssz_block_single_attestation_record_one_byte_short() {
        let mut b = Block::zero();
        b.attestations = vec![AttestationRecord::zero()];
        let ssz = get_block_ssz(&b);

        assert_eq!(
            SszBlock::from_slice(&ssz[0..(ssz.len() - 1)]),
            Err(BlockValidatorError::TooShort)
        );
    }

    #[test]
    fn test_ssz_block_single_attestation_record_one_byte_long() {
        let mut b = Block::zero();
        b.attestations = vec![AttestationRecord::zero()];
        let mut ssz = get_block_ssz(&b);
        let original_len = ssz.len();
        ssz.push(42);

        let ssz_block = SszBlock::from_slice(&ssz[..]).unwrap();

        assert_eq!(ssz_block.len, original_len);
    }

    #[test]
    fn test_ssz_block_single_attestation_record() {
        let mut b = Block::zero();
        b.attestations = vec![AttestationRecord::zero()];
        let ssz = get_block_ssz(&b);

        assert!(SszBlock::from_slice(&ssz[..]).is_ok());
    }

    #[test]
    fn test_ssz_block_block_hash() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();
        let hash = ssz_block.block_hash();
        // Note: this hash was not generated by some external program,
        // it was simply printed then copied into the code. This test
        // will tell us if the hash changes, not that it matches some
        // canonical reference.
        let expected_hash = [
            28, 184, 51, 12, 226, 15, 73, 50, 66, 19, 168, 149,
            229, 122, 141, 111, 42, 236, 137, 157, 230, 90, 149,
            58, 145, 52, 47, 62, 158, 131, 46, 147
        ];
        assert_eq!(hash, expected_hash);
    }

    #[test]
    fn test_ssz_block_parent_hash() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        let reference_hash = Hash256::from([42_u8; 32]);
        block.parent_hash = reference_hash.clone();

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();

        assert_eq!(ssz_block.parent_hash(), &reference_hash.to_vec()[..]);
    }

    #[test]
    fn test_ssz_block_slot_number() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        block.slot_number = 42;

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();

        assert_eq!(ssz_block.slot_number(), 42);
    }

    #[test]
    fn test_ssz_block_randao_reveal() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        let reference_hash = Hash256::from([42_u8; 32]);
        block.randao_reveal = reference_hash.clone();

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();

        assert_eq!(ssz_block.randao_reveal(), &reference_hash.to_vec()[..]);
    }

    #[test]
    fn test_ssz_block_attestations() {
        /*
         * Single AttestationRecord
         */
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();
        let ssz_ar = get_attestation_record_ssz(&AttestationRecord::zero());

        assert_eq!(ssz_block.attestations(), &ssz_ar[..]);

        /*
         * Multiple AttestationRecords
         */
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        block.attestations.push(AttestationRecord::zero());

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();
        let mut ssz_ar = get_attestation_record_ssz(&AttestationRecord::zero());
        ssz_ar.append(&mut get_attestation_record_ssz(&AttestationRecord::zero()));

        assert_eq!(ssz_block.attestations(), &ssz_ar[..]);
    }

    #[test]
    fn test_ssz_block_pow_chain_ref() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        let reference_hash = Hash256::from([42_u8; 32]);
        block.pow_chain_ref = reference_hash.clone();

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();

        assert_eq!(ssz_block.pow_chain_ref(), &reference_hash.to_vec()[..]);
    }

    #[test]
    fn test_ssz_block_act_state_root() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        let reference_hash = Hash256::from([42_u8; 32]);
        block.active_state_root = reference_hash.clone();

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();

        assert_eq!(ssz_block.act_state_root(), &reference_hash.to_vec()[..]);
    }

    #[test]
    fn test_ssz_block_cry_state_root() {
        let mut block = Block::zero();
        block.attestations.push(AttestationRecord::zero());
        let reference_hash = Hash256::from([42_u8; 32]);
        block.crystallized_state_root = reference_hash.clone();

        let serialized = get_block_ssz(&block);
        let ssz_block = SszBlock::from_slice(&serialized).unwrap();

        assert_eq!(ssz_block.cry_state_root(), &reference_hash.to_vec()[..]);
    }
}
