use crate::{chain::ethereum::EthereumValidationError, MessageIdentifier};
use derive_more::{Display, From};
use rlp::{decode_list, encode, Rlp};
use sp_core::Hasher;
use sp_runtime::traits::Keccak256;
use sp_runtime::RuntimeDebug;
use sp_std::vec;
use sp_std::vec::Vec;

const EMPTY_TRIE_ROOT_HASH: [u8; 32] =
    hex_literal::hex!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");

/// Errors specific to the evm proof validation
#[derive(RuntimeDebug, Display, From, PartialEq)]
enum EvmProofValidationError {
    NodeDoesNotExist,
    InvalidNode,
    InvalidRlpValue,
    InvalidProof,
    BadFirstProofPart,
    BadHash,
    EmptyBytes,
    InvalidLength,
    UnexpectedFirstNibble,
    ContinuingBranchHasDepletedPath,
    InvalidExclusionProof,
    UnexpectedEndOfProof,
}

pub fn validate_storage_proof(
    account_path: &Vec<u8>,
    storage_path: &Vec<u8>,
    account_proof: &Vec<Vec<u8>>,
    storage_proof: &Vec<Vec<u8>>,
) -> Result<Vec<u8>, EthereumValidationError> {
    let root_hash: Vec<u8> = Keccak256::hash(&account_proof[0]).as_bytes().to_vec();
    let account_state_rlp = validate_proof(account_proof, &root_hash, &account_path)
        .map(|rlp| decode_list(&rlp))
        .map_err(|err| {
            log::debug!(
                "Account proof validation failed with the following error: {:?}",
                err
            );
            #[cfg(test)]
            dbg!(err);

            EthereumValidationError::InvalidAccountProof
        })?;
    // The account state root hash is the 3rd element
    let account_state_root = account_state_rlp
        .get(2)
        .ok_or(EthereumValidationError::InvalidAccountProof)?;

    validate_proof(storage_proof, &account_state_root, storage_path).map_err(|err| {
        log::debug!(
            "Storage proof validation failed with the following error: {:?}",
            err
        );
        #[cfg(test)]
        dbg!(err);

        EthereumValidationError::InvalidStorageProof
    })
}

fn validate_proof(
    proof: &Vec<Vec<u8>>,
    root: &Vec<u8>,
    path: &Vec<u8>,
) -> Result<Vec<u8>, EvmProofValidationError> {
    fn bytes_without_prefix(rlp: &Rlp) -> Result<Vec<u8>, EvmProofValidationError> {
        rlp.as_val()
            .map_err(|_| EvmProofValidationError::InvalidRlpValue)
    }

    let nibbles = to_nibbles(&path, 0)?;

    if proof.len() == 0 {
        // Root hash of an empty tx trie
        if root.as_slice() == EMPTY_TRIE_ROOT_HASH {
            return Ok(vec![]);
        }
        return Err(EvmProofValidationError::InvalidProof);
    }

    let mut path_offset = 0;
    let mut next_hash: Vec<u8> = vec![];
    for (i, node) in proof.iter().enumerate() {
        if i == 0 {
            if root.as_slice() != Keccak256::hash(node).0 {
                return Err(EvmProofValidationError::BadFirstProofPart);
            }
        } else {
            if next_hash.as_slice() != Keccak256::hash(node).0 {
                return Err(EvmProofValidationError::BadHash);
            }
        }

        let node_list: Vec<Rlp> = Rlp::new(&node).iter().collect();

        // Extension or Leaf node
        if node_list.len() == 2 {
            let node_wihout_prefix = bytes_without_prefix(
                node_list
                    .first()
                    .ok_or(EvmProofValidationError::NodeDoesNotExist)?,
            )?;
            let node_path = merkle_patricia_compact_decode(&node_wihout_prefix)?;
            path_offset += shared_prefix_length(&nibbles, path_offset, &node_path);
            let children = node_list
                .get(1)
                .ok_or(EvmProofValidationError::NodeDoesNotExist)?;
            let children_wihout_prefix = bytes_without_prefix(children)?;

            if i == proof.len() - 1 {
                // exclusion proof
                if path_offset == nibbles.len() {
                    return Ok(children_wihout_prefix); // Data is the second item in a leaf node
                } else {
                    return Err(EvmProofValidationError::UnexpectedEndOfProof);
                }
            } else {
                // not last proof item
                if Rlp::new(children.as_raw()).is_list() {
                    next_hash = Keccak256::hash(children.as_raw()).0.to_vec();
                } else {
                    next_hash = get_next_hash(&children_wihout_prefix)?;
                }
            }
        } else if node_list.len() == 17 {
            if i == proof.len() - 1 {
                // Proof ends in a branch node, exclusion proof in most cases
                if path_offset + 1 == nibbles.len() {
                    let node_wihout_prefix = bytes_without_prefix(
                        node_list
                            .get(16)
                            .ok_or(EvmProofValidationError::NodeDoesNotExist)?,
                    )?;
                    return Ok(node_wihout_prefix);
                } else {
                    let children_index = get_nibble(path, path_offset) as usize;
                    let children_wihout_prefix = bytes_without_prefix(
                        node_list
                            .get(children_index)
                            .ok_or(EvmProofValidationError::NodeDoesNotExist)?,
                    )?;

                    // Ensure that the next path item is empty, end of exclusion proof
                    if children_wihout_prefix.len() == 0 {
                        return Ok(vec![]);
                    }
                    return Err(EvmProofValidationError::InvalidExclusionProof);
                }
            } else {
                if path_offset < nibbles.len() {
                    let children_index = get_nibble(path, path_offset) as usize;
                    let children = node_list
                        .get(children_index)
                        .ok_or(EvmProofValidationError::NodeDoesNotExist)?;
                    let children_wihout_prefix = bytes_without_prefix(children)?;

                    path_offset += 1; // advance by one

                    // not last level
                    if Rlp::new(children.as_raw()).is_list() {
                        next_hash = Keccak256::hash(children.as_raw()).0.to_vec();
                    } else {
                        next_hash = get_next_hash(&children_wihout_prefix)?;
                    }
                } else {
                    return Err(EvmProofValidationError::ContinuingBranchHasDepletedPath);
                }
            }
        } else {
            return Err(EvmProofValidationError::InvalidLength);
        }
    }

    Err(EvmProofValidationError::InvalidProof)
}

fn get_next_hash(bytes: &Vec<u8>) -> Result<Vec<u8>, EvmProofValidationError> {
    if bytes.len() == 32 {
        return Ok(bytes.clone());
    }
    Err(EvmProofValidationError::InvalidNode)
}

/// Convert a byte sequence to a sequence nibbels (e.g. [0xff] => [0x0f, 0x0f])
fn to_nibbles(bytes: &Vec<u8>, nibbles_to_skip: usize) -> Result<Vec<u8>, EvmProofValidationError> {
    // empty byte sequences are not allowed
    if bytes.is_empty() {
        return Err(EvmProofValidationError::EmptyBytes);
    }

    let nibbles_count: usize = bytes.len() * 2; // 1 byte is represented by 2 nibbles
    if nibbles_count < nibbles_count {
        return Err(EvmProofValidationError::InvalidLength);
    }

    let mut nibbles = vec![];
    for pos in nibbles_to_skip..nibbles_count {
        let index: usize = pos / 2;
        if pos % 2 == 0 {
            nibbles.push((bytes[index] >> 4) & 0xF);
        } else {
            nibbles.push((bytes[index] >> 0) & 0xF);
        }
    }

    return Ok(nibbles);
}

fn shared_prefix_length(path: &Vec<u8>, path_offset: usize, node_path: &Vec<u8>) -> usize {
    let path_without_offset = path.clone()[path_offset..].to_vec();

    let len = core::cmp::min(node_path.len(), path_without_offset.len());

    let mut prefix_len = 0;
    for i in 0..len {
        let path_nibble = get_nibble(&path_without_offset, i);
        let node_path_nibble = get_nibble(node_path, i);

        if path_nibble == node_path_nibble {
            prefix_len += 1;
        } else {
            break;
        }
    }

    prefix_len
}

fn merkle_patricia_compact_decode(compact: &Vec<u8>) -> Result<Vec<u8>, EvmProofValidationError> {
    if compact.is_empty() {
        return Err(EvmProofValidationError::EmptyBytes);
    }

    let first_nibble = (compact[0] >> 4) & 0xF;
    let nibbles_to_skip = match first_nibble {
        0 => 2,
        1 => 1,
        2 => 2,
        3 => 1,
        _ => {
            // Should never happen
            return Err(EvmProofValidationError::UnexpectedFirstNibble);
        }
    };

    return to_nibbles(compact, nibbles_to_skip);
}

fn get_nibble(path: &[u8], offset: usize) -> u8 {
    let byte = path[offset / 2];
    if offset % 2 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    }
}

/// Obtain the storage path for the proof
pub fn storage_path(storage_index: &u8, message_id: &MessageIdentifier) -> [u8; 32] {
    let mut key_bytes: [u8; 32] = [0u8; 32];
    let message_id_encoded = encode(message_id);
    key_bytes[32 - message_id_encoded.as_ref().len()..]
        .copy_from_slice(message_id_encoded.as_ref());

    let mut storage_index_bytes: [u8; 32] = [0u8; 32];
    storage_index_bytes[31] = encode(storage_index)[0];

    let combined = [key_bytes, storage_index_bytes].concat();

    Keccak256::hash(&Keccak256::hash(combined.as_slice()).0).0
}

#[cfg(test)]
mod tests {
    use crate::chain::util::evm::validate_proof;
    use rlp::Rlp;
    use sp_runtime::traits::Keccak256;

    use super::{shared_prefix_length, storage_path, to_nibbles};
    use hex_literal::hex;

    #[test]
    fn test_calculate_proof_path() {
        let path = storage_path(&6, &1);
        assert_eq!(
            path.as_slice(),
            hex!("80497882cf9008f7f796a89e5514a7b55bd96eab88ecb66aee4fb0a6fd34811c").as_slice()
        );

        let path = storage_path(&4, &1);
        assert_eq!(
            path.as_slice(),
            hex!("210afe6ebef982fa193bb4e17f9f236cdf09af7788627b5d54d9e3e4b100021b").as_slice()
        );
    }

    #[test]
    fn test_shared_prefix_length() {
        // We compare the path starting from the 3th nibble
        let path: Vec<u8> = vec![0x01, 0x0f, 0x04, 0x03, 0x0c, 0x02, 0x08];
        let path_offset = 3;
        // Our node path matches only the last 4 nibbles of the path
        let node_path: Vec<u8> = vec![0x03, 0x0c, 0x02, 0x08];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 4);
    }

    #[test]
    fn test_verify_account_state_proof() {
        let proof: Vec<Vec<u8>> = vec![
            hex!("f90211a0f95b30e8057169e0fc1daa9f78787333a372e485d8e1d2c2d6e6490c3bd6016fa0681665dc7c7d2a1b6209c6f317c718eee11ab8eedd61fccf86c067a6e3806d27a07207538d7bfeebf3470e06fcdaee54257f5916b58116b26db1cf7b76f1159cb2a01686a528f93001316f899c817524945b9c9be4315dc338bfafa618c813a4e207a06c36fd3689e73ce2827b5f92d67a770eff3b50aa1347583e1322dbb368f23632a0f304a9873278d4a7883cbb1279f22ab463aff78049baa716c6afc1539f6597b8a01d5ebc8150378a4038a4abaaa3462fbbe73b9cd84e640d0c3e882b49a1ddafc2a0cb252bf4d64b84ed05e71d33394a21d8eb79c17efccdf4fc22a7616d7938b936a06e66ee831d3d94c099ae66dbe115b4ddbb6d25f74e80d5c518794b6780ce9384a01474af95a02eff151cccab14ddb3a742696cb4111c3ee6c9022e0320f67b3377a0fabd16e8b32fa448ef500b790994f20814926337176a15844b89477173fb807ba02e07808444c4f433715574baa5cee086b63e702921b14a201a2180db17d9cb4ca0ce71a37dc14cf4c103d685246b354c95662564142a5a150c070b36730c3d2634a07417c43675ea5cd1b7826d11d2dd9ddad001c5977d2928a2a00caa69a44509f0a0bce5abe6ef48bf0cd83c1b1dad37e6616b5acee93e055ef055698946a99edb58a060cd523662c2656342ca06a1914489c2d66b823a1996cabfa11888f8c5126f3580").to_vec(),
            hex!("f90211a04dad2e8c56b4d41a8bf62784c999d62946787aa89608e74e63e70db454a941dea06182484ec7e0ff2a22680b567bd979a0ed0883729192425be22346f66dbff7eaa09555cd7bd1a1f2f046e84af6293a1d90d427d1aa1e8532aa4d123d5b8a33872ba027d4b7804eeb1516fba785caebfe9cb13697b95d5b23b74119e0635b4f7aa3a9a02b18f67a754a345e573ae03aae01d3e371465d757999f0c41ef13eecd30a11cda096560ee6b086fb8e10d65d0ae75a6d8b093e226b06e181afd5f7869ec0327117a07954049a9a8256f41d22164429692c1cc75f1c6b604a088c79c5dc5778f05efaa007ffb113f7370423f31b3c0bb9e2c2e3513e0f48a8550fb42694b8a632b05c40a02291acda3ef7748c9aa6139832f0cc8c10e4227643d194ef11d4163b4cd36e01a03c71b4760b879e666c704c744036eb3ff585d085fe7fd82a08634140d6c98207a0daaed6465d195816bd1919fdd19688a6e2a9156ef351d106e2f1a07781cc9d57a058f9047b134ad2ecc5af428d272f5acc8d386249e1bef5bd6f96c18f457063cea032b4f66d9fc622bbab0c862a4b51aed5956b48ca9f05beb7da6d37b35a3263dda0dfefe8a210051438b50dd2f092d03300311d93e235aaeadcaf3e5681988c1feca0eeb5cbe3746d80b37387802ae393c36511e66b5080b3c767f0731588037e508aa03dcb5f831c18d2c6c6ca69e25a13266075960d81a632385ee3cb87d7a1e9843280").to_vec(),
            hex!("f90211a0d0fa35677e37b205b596cf4c212b479326befc3a4a9e18c6bfaa7c59643b9fd0a072fe41c97253aa0ec8dcac18ba5fd453f0eefdf850d10a39cd30524d20439452a0723752b0350d1ecefe1eba876307099c28fdac16f6e70a667a69d7d93fbf75cfa076e5549004b7168bc37c15e83b0c48966408c294c84c352c5fb650647b799292a00110a0b7592311b22cd1bc621e896861e1414546672baf6ecac1fdc618acc017a07c44a8353e9e0aad2cb10d6c6ea99b1546370138078b42ef37532314c705791ba0c5f785f12278ef0012b8aee57beefcd83ffd262c084c5eca786c06b916b01e85a001f1ef52f4a5c6a94a50694b595cc008a61f5571bfdf7d2b937bf1353c9acba7a039fee3000c0ac6daa58d716d8d412efcdc9bc0bd7939f7b7ca3b5a6cf0c061e0a0162e73de59897db33bd41bc8e09acdadb830df30e6036d5e7329f75285d3d914a02a83b09982f2d3f0a8b3730a768fbf2db12bfdc25dd383d5e8f4e5b7336b39ffa0bd052c8dcfe57c7ee8fe2ad4167ea361972cf68ce9dda5ffb14374ea72d1ac79a007b92b3f0c3cd17275ee20fae41512d59d04b5d3eef5cad3f48c0b21c0e17703a039abe83169b68fdf16d227d94a082cf112cfa9085208fe6b5896cd159d0588aba08261e532414b5bcd0929a7a950ea165e7e3ff54af23b3b460b9e862e22770260a01eb72340083708752d3aa06f2c0a3a1047e611aa1a07b7ed676d95401e5afe7480").to_vec(),
            hex!("f90211a0f3ae0e4692920a9093eba3b2810597ee80831ff6864fb526e0def51b538dc6eca09a33c672b1119c28d097b1f96afc69ad75f2bb175122761497d79a2c92f107f1a0c9936539b9bbad200f27dd0589c937e98d7cb627fb67add1f096eb50d75a3bada022e2f4fb91f2961dfc16e3e30da98ffe800ea24d094f383ca6bc1b66cf70d2d2a03a25c8cbbc6e2a236aff16e2415051f64642e04bdc4079ae38fc72def1c8be06a067639ea05f1846205a827ffc135ce949fede0072dcfa721b8d167ce61ba6898ba060c8df7225b4e06efb17e6e09568bcbc43bc0156fbd498abb99ed77adc4ae8f7a03999adfb9a4c463c56e3648b6853cf2a24cdeef40ddb278c77b868471e8c51a8a09be11017032234ac90a19821f492541824f024b4fdc00f031d7267193837f998a06568372ff5d4d33b6012026b85ac82317d7d941708ce3730ac3c148e5ca92c1aa055cd80c4a73ac9a4f87aaf626b891c1aa8e72699a1f8ab1f6f1389df679d6ebba01bf003aa86731cc1a0fff8896794ddf52efbfee182f894ecf6fadc17a79286f4a0f07f8e6d8a6c8926af30e22ef20fdf4d50a3cbf1699dff7494d8b5cccc5bee91a0b346e0b13d93f37be6498e91a7b09944f52a372d21020310775063e1398cabbea0d8fb399de1d5e3d9eb7c37e171771e4018ee0f4d69e7c879ebc3ad9850802d3fa07a18971c78172d4ffa80117d5318340f69dbb651c96baa205b6aed810740ca7c80").to_vec(),
            hex!("f90211a0855b55c2ab89eb13f31dbf1b713b3fbcbd44319a6e82f5b88a5a81fff37be2dca00242150c331fd426bab884e917f543987d8691f2a80bfd0506c5f226dc06bc27a009aed285d92badd8eeef5ea976ea37b37aa377acebc37dd8b72e9056d98638dda0497aad06d0e1536194a50eef2b5204a8ce2115dac71d1162d0638d183505b6fda0bcf9e7972ba2006afc8f6d75754d3144605d2568e627a8f8dcc657f87d6c4179a061fc222341591278641e4f7f23950699b5d6e556b55578628995db53ff9074f1a0f2dd11b267eda5dc67430e1ac88e3e868d02d2507d5146a40471122030441e6aa02488c6d04fab1938154c329fbada80fcdafa301ba28787e3929603ad49122e2ea0db97dfaeb2f81f4e62f24377e48b99a9c0c40add9e13b2f6acc6e6c7b9a05e61a0fe3344535fbf172577e0aa65b95bd831340e798153bbd372408c482e9104d5bea02f4a722816303116ab4562c5334bcbff7b3d59a1d6a64292ec59796291078df5a02c6313c5d25d45e203e5b836f3e4646d25f929c2b0899e44521452b35d80ef20a0d6a2361bfc61aba1ce27408d898c60c25212015547c4cda1ec354ba204e369f9a095de06eff0d25c8f783ac5dd6cdfd19b5076612d43181f27be0ea3b725668ec5a0f2873eb65424aaecb09658cbf2aa355dea12fda2f77d39ad5a50b1f77a47cf08a08399367c73f9de5cf54e56f9049aaab18b3463f69d769f62ecf838d1ae967e4f80").to_vec(),
            hex!("f87180a09561a997c264962c9a8b4aee2582b8ef36a189e3e726d35c2cb826fe8d0fd87d80808080a00010dfaea0e22d6ff3ef10c217e1c415911b21a17c987a26f23c3a01e89b3d0a808080808080a0be6964353ef31b78edaaad0e8bd87b64d62632aef85fdd3ae963955a98628ec2808080").to_vec(),
            hex!("f8679e20389db67e3b84adf9d34deca5638f3aaf86a8eaaa6147889bca489e7a7cb846f8440180a0c043666e3ecc8c280ba165497aa3ed83dddba54c00e6e73486c68427925e0778a0e751f0a9365eab5149f29145082d5b033520eb9cb2432527d65e19d6efcbdd0b").to_vec(),
        ];
        let path =
            hex!("14c9bd389db67e3b84adf9d34deca5638f3aaf86a8eaaa6147889bca489e7a7c").to_vec();
        let value = hex!("f8440180a0c043666e3ecc8c280ba165497aa3ed83dddba54c00e6e73486c68427925e0778a0e751f0a9365eab5149f29145082d5b033520eb9cb2432527d65e19d6efcbdd0b").to_vec();
        let root =
            hex!("297677d612641f8a53454bc8126f4b225b95ddb6ab395d12a2ed740b8ca81cd4").to_vec();
        assert_eq!(validate_proof(&proof, &root, &path), Ok(value));
    }

    #[test]
    fn test_verify_storage_proof() {
        let proof: Vec<Vec<u8>> = vec![
            hex!("f90111a04c77a8959da29908fa97ea8718d3dd2fc298c353a9da9e09c6131a6a1cc3de8880a0308611a8afda5c8a10b09de3fed011ae43c480313fd2c85d65a92d35359de7fb808080a0f088bca3be2219e02d2ce722d00fdf516680747991013835a1c30d5296b47fec80a0017e20495a1d135325ad9f1f72d720a0b20b85eca8319f10f8c6f461a62e27bf8080a0482e10e64fe37936565267fcb8e0dd9cf74303ab0ce750dd5437bfdd99249528a0b794f22030a6452bd30975ba1b9dee4b798f3b560807323473a401da7f87124980a01f0b30aa51df7ff59d462dcefc151653f1af532a650eb9a5c59672dcf751a5f7a02f948e17d693c90a394a6dff75aa79461702f6361a43daee7f3eaa143825489d80").to_vec(),
            hex!("f87180a0367682f42ce7bfb86a31cc6924f7038a750e822f31c0e905b51f5ddf9b8dfb2380808080808080a04d0c15612e60ae90c040ff5eef0f99778a6f3dfdbdfacf954295252cef782a108080808080a02e5c6b3fb31df33a8f3f8e62ddfb6ef3078682b4aa8e1748b4fde838aaac742e80").to_vec(),
            hex!("f843a0200afe6ebef982fa193bb4e17f9f236cdf09af7788627b5d54d9e3e4b100021ba1a05f786a9fcb8250a3f27ed9192c66594dec76f3d53a4bf9d27ffc086b5196280d").to_vec()
        ];

        let path =
            hex!("210afe6ebef982fa193bb4e17f9f236cdf09af7788627b5d54d9e3e4b100021b").to_vec();
        let value =
            hex!("a05f786a9fcb8250a3f27ed9192c66594dec76f3d53a4bf9d27ffc086b5196280d").to_vec();
        let root =
            hex!("c043666e3ecc8c280ba165497aa3ed83dddba54c00e6e73486c68427925e0778").to_vec();
        assert_eq!(validate_proof(&proof, &root, &path), Ok(value));
    }

    #[test]
    fn test_verify_storage_proof2() {
        let proof: Vec<Vec<u8>> = vec![
            hex!("f90131a0fe1cec69138a035b27919cba7d03d2f3b5867e183fc5928af3bc0b0f85b562a880a0e759fad30e475a8a7de20efb084aeaad48864ef0c5eb678f0133226a4489d5f8a0da9cbdd2154724e704491b792e162e096df39e9f51363b9b950933a61186820280a0de572a50aef9d550512795e67eaf06acda25ada12d45e5944fba2cb429641f5480a05abb50d3ee32dffe73e3a7f9f354bffe92e4971bf45b527d046208a6818120f980a0ca5985306e251400a05df43a16a3391bca6cf1e5a39acfde6f619c8ea03e3fbc8080a03783bc2fd4d98095264ccacf2098c92a04e317f93a82d87a713d645e0743ef8a80a0bb7fbc81f9cb125fa6229c00a3b6442d31316510f0a9054827bd2317fc95ac9ba0b60d522b76ccaef75c1d5d2faf67a3904ea0aadfe459950661a60e2111e94ca680").to_vec(),
            hex!("f8518080808080808080a0ff1d82682091977c3bd249fd5840706e2c8f487add0b1ae09d430e80d9aeb8f9808080808080a066ba505307e91ddbb884cf21cfffd24941ca533e0b9384a68144039ab7fc57a280").to_vec(),
            hex!("f843a0202ead72d53401d823f4de3290714b95c588de2c574133f57728a2d3d3763d3aa1a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d").to_vec()
        ];

        let path =
            hex!("ff2ead72d53401d823f4de3290714b95c588de2c574133f57728a2d3d3763d3a").to_vec();
        let value =
            hex!("a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d").to_vec();
        let root =
            hex!("c53cbaddd072fc5094f0e0986a1baff9ed3d6dbe4133eb4e7764dd9e93f9ec9d").to_vec();
        assert_eq!(validate_proof(&proof, &root, &path), Ok(value));
    }

    #[test]
    fn test_to_nibbles() {
        let input = vec![0xff];
        let expected = vec![0x0f, 0x0f];

        let output = to_nibbles(&input, 0);

        assert_eq!(output, Ok(expected));
    }

    #[test]
    fn t() {
        let input: Vec<u8> = vec![0x82, 0xff, 0xff];
        let expected: Vec<u8> = vec![0xff, 0xff];

        let rlp: Rlp<'_> = Rlp::new(input.as_slice());

        dbg!(rlp.data().unwrap());
        assert_eq!(rlp.data().unwrap(), expected.as_slice())
    }

    #[test]
    fn test_validate_proof1() {
        let proof: Vec<Vec<u8>> = vec![
            hex!("f90211a01e6a427425517df2b64d83df8c8cc2577227a3593d1ff8c1f43200ef40857fcaa0dd70148b3f76a278380eea8a0ed86ae725b8acd7d3d78a5092c59fb2d011990ca03809e04399911d5abf36e120b614a9da13ac4bcfeff658e391442e16d257d4b1a0feeebbaad3d132b85373ce0b713cde63d9fbda6e4a8920b232abc0c47ad63fafa022a28681bca7ede4e347b3a2cb6a0579122201f28bae7cc64eb2c04ff398da92a04d20ace3c48bc32801ea8981b962521087cb7252c3d2720548c5fddf8ba35d8fa0f2a796e896270c04ee6f37375219cdb84997c53123a83ea1ecfc4423e75ce7b0a0f50a317a8e7480ff78e886b6cece9ed7a931300a6d88d0fb63ba89c4d3afbc96a0be46e711f140c5e6b546f7a009f9a985531a9a4e363fcb536eaa01a2d2ed9070a0aacbc1190a7dbc30ad85284a0c7568535575836b26376e18c264ff02553d957ba0514bcf049e1802ce3bf91ae4a9252e460e84bff4fcfa92f5813b8738b556e380a02af47e0f88d5641608d03d3751f5905d32b56f6685bfd79c7153ae7977af13d7a035668a4e9099f0071dd53356da6d6d08b24f48f03cab9d7b2e70f8b7a9563200a0a43e25bb65f98f9ba2a9999d6cf5da1da2019919a3c95cb953a7d1eff650649ea0ede0a02bb3d82444eff81d1251c7e8f3c1ab8d84824f87f2c364ddb02e1ff7a6a0bd5b7de2e00ab0396fb9a42e12a7b3801e4a8409bf439ea073a28b6a3ca8616f80").to_vec(),
            hex!("f90211a056191244eb9f024361fa4704b6a181ff111d0cb37a13cf15af81b795c2a93823a0a00ecaf344c97f51f111c8cbbf7897a64c93c31a1663cb09cc58b05a33810c8ca0398a72a9a9bbc6091292e6871291292fdfef2f85f8fe5ce520630bd477e55cd8a01a62908e6a386ff412d5cba050f9485530b36fe14aa29e668fe87335173b67afa00c88de9fd09b791e4b2c02ebabd3ebbd80af2e44572f1ca46447425969ec2dfda0637a86f1bb24273b42378a707cb385785116e4e3d86c3bd91761b5e501233ba4a082d9101880c166dd752bca20672be6a17dc0af25f675ca8148fec20986e854c8a058fba32e4201f8591c8fc60ce1f3207eaee055e4bd36f1cf08bb40138f001f38a0c8e3502b34b734bded6bc0595de7ec85e83e8ac9cba6619e1703c07c91e58d05a03ab8bb8772b4af2373e8a5eb70855015fbf1ddc0db13e7f19b8b61ab3c940bb5a0e0af59b0a454d5324c031dec76e0c5f68338d1fbfb5f132407d762ebccef108aa0e1a356c43f883107b6098ed1c181fe1dee91aa2d30803ae430bb24fcd7fb1dbea019e2759edcc71b15e5e4e8dacf895ea5e488b0a452720eff81088cc8c55835a4a0fdea5706ec6d2ca3af188022dbce15c4d042e6c103a1f5949ae155d121b56238a0725da3cc14339dfd78cc4f932f74e5559c8c1b06bedb071a19a7d7495ad1d542a06ff2c1218fd9a0596e5603a4b2da9ffc6504406277f8112b4427578b548737be80").to_vec(),
            hex!("f90211a0ae4c2efc81cef10fb81e95229054a07f026c22eedda2582007beede43e478769a066eb86f82b30900eaea429345d4b344bda3d2684267a7443fa063683de170ed5a0da5492911f28cc7e13220bfb1ba84024a4436b319e4470c926954b30dc35b467a0d9adc4937283c6de302f5fbb6cbd128871ef27930ca3b42842e27d806ec65fafa00005ea9ad2a44b9779136735a01a9951726c984c8159c6108ca320e84306ba78a0eedbce0abf3d12bcab3b79cb0500f814913ca66dd522628e6081f2d7a2f316bba0375ef1473700c00b2f1129b4201ace1b91978a7e93a4623aac3dc9d8e03d84fba0bce9a43f1a91483bf3e8b0bca2db7ed9d865489d18351bf4bdfbeeef98a2c1eea0896659018b5be4f81975052bc33012d75990a3c4a3733a05707fd443ea6e2efca0084ce973bfb50078a182dc9de3d46338a9608b103e9bd95f3c14d59f890c8e60a0edd2176279800065a96d4af687238f4925c6f79e1af5fd0a7370097ee3c7a2d2a04321c84e8442f2c04f028c47be1746bbe61be1bfb933b5812e25d38a63655d79a0d68f9aceb1e90eb19e00a300f4109334ae9328b88a8a363d13e23c98eba76e36a0278b4b6ff61e30c699ada71bb82ada9967b8358d061a8ff5e93161459c3540dba06c43fa474d56eb93966b4f935c7b3b2604adc8c71331bdc9d3b12fd1777c5533a0b3b8c2b0b7e60a1084b821ea5ba35ceba40e77f5f877bca6297ff1f458639a1880").to_vec(),
            hex!("f90211a0e23ff66a0f8ad62bb784a7ca8fbcc6d854c217f31631e0711af956b46ae82f7ea0e0ae1750b122312c3969a49dc2f4722fee0a8bfc8a03f99a2d6068fdc9c0476ca0f6abdfdec088980dd6490363db91aaac1c59bfe90432bc1ed7536aca9033b30aa064db2877e3ae60e18123d188d278b7cd13a956b4382844ad7e6c73a4b8c4ce79a0923951b2265bdcfcdcee6725345faa6d4319e87b6c5987cd4ccb765519d6e318a08eb358b0b35c495b0cffe6197f2ec3b365a0452cfa2871a496cb7fef4a3bc287a09e7dbbbbce9e4b699b8c80b7df4c9fe7438da63ebf3b225a1148c29fa9f615e9a0ddac088e32487c58240edca128c7ea0e06687eba3d3282ab1f60f9d37c517f5ba0eb804eebf02f22b8094da8df48ef32b0767a65b1d1de46afb4db5a35b14d936fa022474b23a46ce8f93efcf5b4f581237fa21b0c6b79b48ef444a58073835fa205a07dfe0b469ea9d3402284d638229aeaf013144027e37e6693feb6e88e9dceb988a056195a91c1924e0069d8fbd18f8e07e71fbb79359206c3b969a76ff235a51ab6a028573f64876850d45de94058292474604aabb75306d2b9631af69e2794f7f1a7a0703700caded78244d5f7576ef0acae92d11073f1a1f11e4f30c35325f5732a36a089744d45240d7eb0c66a1e7065b902e1a223521c241683cad5c0b4229cb04cc5a09b123534f8741c7385e194addb2e5803fc23e8c88ef0a8556b7e2265ac47485680").to_vec(),
            hex!("f90211a0710b8047e020c963f2d596cf850131fdf34d32a1e000a78003726f6d362e446da05a5a90589869b57cbf6c632e7263edcafec5a1869d3073222065da8f7267e842a01a219600e7247c0386922c8f7644509a6adeabfd9909fd314786b5651969e375a07f973d4204d82be0983a22b50122d65c5c3079e2a3fd95023889f62793ed0fafa02fabe4ef80a9f2f1b8acd34e24f710417054ae490b10bee8b8f8a16b2051ddd1a01fefee09456f00e2215465df80dfc00efc289e9ca63407cbc98e2faeb4cecb81a054db2bbb190bb88f080e5ac1c462fdc2f618d64ab847e1ede4a220f210a12021a0b0ecc0a9bb68c04e655fd79feaa2e9835e52301166fe1f96d542cc8c3bbe2379a0bee5d780b353338ae2ad70fb61505f6dd194b4562391c2ce70c58a244355b147a0fac5446d3d4d0fab590373adeee5d9b702d55a70eb328160705dead5f85f979ba042733f5ef0e62d49350f0362e4ba410c7f69f168c190d32b7b5fbfa2c5d32eeba02c7d2b55e55f8aebeb2a05d2a83608709bf7247aff4ca12b4f4c13468e5384b8a0800ca1fa1006d797a2202e2591585a1fa9bca1dc6a2654f0b5a13a6aa4d25832a032d202cbed4555fb371af18d1c128f04a4b0fd9c2e192da5ecd1f4f62338ba96a02075b53e615282142fff6d8696ba07b4df46d37e19137eb8a0a04f8e904139bea06f078298e09b67636c38570908cb070b6091ea522f81de14e2666606e00d13fd80").to_vec(),
            hex!("f90151a09df9dab6099b9db936388e80ae73619a63cf601caa452eeb62b9236aeefef2fca0b7ff6cc910ee043957e45514798bb8fc35949a9b212566b2962851ab6b3667bda0b8a3288fc50969d8bc42ca59c8f8ec5e08ce04f0b863b2e7dc1a89cd3794e5dba0925c3c60db03311e6176c67cd906cd10d18091ffbf70368b540c5adb31571a2080808080a0c03cf180255aed0a8cd58a3ca8e3e03a3cc295535ca2d2fa23a3988e45bf0a8780a06f52262da5abdfff6c87993606334702f43835cb8024ea3d0ce805a1704d87b1a09e3beb75b5b4f57d32eeabb580a9e86f6a14cbaf6710005103318fad78c4edb780a0d78ddc950cf676ae214cdc652a6fbd28edbae3fe6909fdcc52656d6e6ed58e0fa0c58b052c7531f7026f48bc785701ea6315a504babda50768afe48964e68ef924a0bc36dc0d7250bb77f5b033f465bb4083caa502f43d58b61824a968edf7bac67a80").to_vec(),
            hex!("f891a022fc1c9807f482cf7b8985a08a2122e83db36528afc8359f6ca618fda115f07280808080a02c790808fd07adf1b29e9d6e2f6c4dee1e1cb1c5869ba20baddcc7f1e0ea8c1ca0922f03fbb9164b5642c6bcff215fd68996d110d21de84f4218e4b771c166297780808080a0e2ced53fe1965299c88cdb923984280ad671f11d13929154f4ea7e308325c7088080808080").to_vec(),
            hex!("f8669d3f77bc3bbc1cfa5699cadd3850753e93731f02f6bf025f1e4ffc3fb788b846f8440180a0865742af102ffd57df06bdb6d58b31c8a76e368332f4b5db7386e5ba450eca0ea0dbe350999ed56f5a428aa0d998f2fc2d98a8599929bb156ca57dfc0fd5e75022").to_vec(),
        ];

        let path =
            <Keccak256 as sp_core::Hasher>::hash(&hex!("9b526A28eB683c431411435F2A06632642bCcBE9"))
                .0
                .to_vec();
        let root = hex!("660161203bd2b16c79b1e003d39fb65201c7b961355bb130b6ffdaa80ece9737");

        assert!(validate_proof(&proof, &root.to_vec(), &path).is_ok());
    }

    #[test]
    fn test_validate_proof2() {
        let proof: Vec<Vec<u8>> = vec![
            hex!("f90211a0dded1ea1ded6da9ae53a491896a34344c885a87123a0fc3406403c40dada8fe1a062fad4c414d0968b0d4b61b06abe4a12785c05dc94f2052087a24603377a1127a0762dbe56774d5c2f74f3bc62eebbff2c1ef8d622e044d763ef46a9c6e05b0895a024e16ad0a70f8e2a373cd20fbcb1beefd236930c9e80972676ee75e71376f748a05c0b8e998808ff95805f6e6d114aca2bdbc3c8a05cac100caa83453a80fc551da084cc118be70d76f1d7d45e731adb32a5ea98194f1e55e12e8a81c1265bb35804a0cfe0d0d5a659bf87435d9175311ce312a99fde37a714844fe8a86b6671485b29a014d0b6e6bd843f8f5652a691b07e8bd57d8daefebf68518dfa1ca2c27371c955a0e59a89693faa726341f6eac51e893e81bcdf3983788bfcf88cd35e85f2692072a074ad7d620707530ae12cef2d83f28cfa6b381176f2f17dda157f3d94194c3edaa060e8042b25a70cbdb5b7bf7acd57e96f389f8c5ca61568a3d104bb6e89410397a00be1c2a477f8eac3a4ba620b959b4e9702f919ef860337420a3dce5be814158fa091dff9cd84917e1dad5c65c824aa549be1e2b537eaa84adebbf0c7a34c0516b8a09db42f5adcc060c2fb368c2b5b5f2df2bd432018e76c5497305cf9ab040ece2fa0827453674aa8a0721788852fc341b097c451b87949198314a6df7bf7640c34bea0926595591284d182861c26ea9e71e86c7b9f8ee3b6859ee29113ebe08026916b80").to_vec(),
            hex!("f90211a0ee8f768253ed6c37765832d64884a5e092e41cbfdaf0fb8f0932211ce0d568c4a0ce495a7fa0836aaeab91fbbdc7dea5ce1aa96b857fd1a8e39f86f254010f2973a0491069c53daf75d1997636cf24365e3df2fb7b0c1f473512f4015bccd7c0c495a0bc5e1a2ff02cee76141ebbde7e11ef1cde35ea2870dfb9c86c3686a1149359d3a0981b41642f7de62e3b86aacbaf87550fd5e6cd25fa74184b83547adf23a22fc5a0fddadd87475a00c34bb9d6f0247c6f2e1aa35a9ae3f704e053a8a5c2ce5c3913a073f979076c82f9b273fdb1490eed2a42118a22657e9e4aff5531b9434b601bf1a034f4c189b0d5b87be223f8d9ded6ea924b183ee5cf6559406de3c8c0a748df0fa0ad24cc8adf2c208cd3b4a618b16bbf9ad14caacd2b07fbc90bb62df6be85496ea0ea35325f678defc5ea53d77a23a293fc68c2d0b39fb1231466cd6c234da540b8a00ba70a69e36fa88db1d0eaa49c5b55e898560a8f7eefe11b6784020be2dbd8f4a0b3caf50d4308581d338e3ea7c22525d82188aebbee020b632587460edeae4af8a02272da0b5d0a15df06e7afbcb3c72d46f014ff8ffc767fed67bd913f7726681da0b7e7835994dcd6ff3b2acb150d385946911e12cff2deebce7b2c1d330443812fa0eaf2a4492c71a0c4f472d0def98e87407968800bef137205774089c5f516fcd4a093473cfe514cb3d370ab4be46ac4e5f4d7789246018164ce27b0aa67f389665e80").to_vec(),
            hex!("f90211a03467744e52388cea47422c20dd914ba06b7774b831b6495bb4698c15d389148ba088525a844b5a5d4d8ef78f2a3778ed158134c67bf398c7d3bdf75eaeecaea572a05eb80f9c3226203a10218dc0fae5ae105405d88965dae3f54e8182a78cce661ca0fce6fb480556411a6154d06549bb13faead17c58ad5a2f007dd15c27c8b71ab9a039ce7123fca637f72e2453bf5fb6d41d339c9ed89d952528b2834864b3b95380a049082b9365e9191eae486a26eb45c46b86c684f1e681262ab2901e8966f00e9da0d4033860fd491626ee7b19e5a139740adb20b20681a4caa3476d192bbe0fb394a0c93505a507ac2a8c02679f893abd01c472a1d4f55da00fdc761d8b05ec3af045a0974c124e54de2d0d35da4e8f96e0e79594f1fb8c89eb9988dcce906ba03000a4a0d2b3c203d5ec947f1a8fb4d1bb9d0c988543f26aabbc41348e539a1611b2f1dda08829379b6999f03962ed414b65a0a23eb969525a0ad183182c32b116cc889be1a0721b85e9bb81dba6362be176d5a335ed706ba83d0e977ef7621d72060d84e28fa0174f7dc9393883bcb4439c7af8b090e45edd7e812908bd8ff47903f371471fc3a0713ab60afd75ae70440f07eac886187729e27b06f760f056e81cf2ea72b766f0a0197b0c5479eb956a72b7b52e5b4a6c2617afb71d7437559f208850d3b2fac728a0d8a6b21e99114d2bcf3ccc468bd6bfd3c6cef707c930b709294e90b537d20ead80").to_vec(),
            hex!("f90211a0ff0a1740e1d2306fe57d45a2a282ac854ba247fef3d5b2bc22e83ee38a381578a0d2e14547273fc537625a75c302f56ee66cebc4768a150cfe58001566d7aa2d7ca010ee81a990481ea9d551664be71e8d9151dc05c2e11f9580f744b4c2a797ebfca037d54c4a3a414ce5b1dd5b05f54c3f9eb188a83f211d10e9856f3aee9fbe6c13a0afd0cb95be49860f9374c39afdb6bfff7bb920c1fa2b43713a62496198756b97a0316aa0c4bab2b1863dbfa2ec6ebadb68177d2af31c9801320c7ddfd26837a754a09a22b6f4e8b3bc901cdf1904c52a74f60e467521b71571c8919291e38ce646d4a09a67f363de88b930c48910acfe09799e88ad844ddbfe13cd644e755e99007694a0d0ebf268b32959711320592556273d90a191832efb32dad8313e454ac89b1203a00fbd113c8f88fa023ef525f1e963ba11cb31926bfd8e78e4a3d3a494cf011da6a086e5af77f6983dff46261e4ae6b04b26af984341fdb8820371b209c07baa8b31a007905fa7cfd6e1e359772711455b981134fc3a61748229d8a8fd0e38e7b4345aa077948f4f9040d14cfbfb4e6a4d998665c2284e6e613f5d6cd66e5b34e58fd7f9a03b86f28bc11cee028ed3b69fc2561ccb55b8563225537db8fce33cd7346b52cca015ca6665e74023a3d81068daf162f1dcd99880d0d0b9c4c08bc7a9011bcbe31ea051df22083c50d8216e7ae63ff8386c8e013ebfe146ac088cc068e8a98fbf5e2080").to_vec(),
            hex!("f90211a0f8e2caf4bfbd2387b011ae9b4e0434c4ef6c17fb7104bd3851ca9aa14c4c7be3a0b206684c96b45284dc843db0ef51474ac844697367d1e5f7a4b09b1d5b88b158a075a3fa28669ab58184a2e5bcd13eb55bf4751cfb4860c8074ff70442494d2229a019045885703bff231764c236c2dee89caee45ada51cebc30b8c2efe81a9f3782a0f453685df20ca90e4d1d642f1473139fff91f4436902baae4e704ad26976fc7ea05001ac402c71ec49abe1c679a5c6fd4fa0e55f49c597bec9d76de9cbf3ff5c08a013a1e86fe06edd27811e5129ea1730110de04bbd34e9a7bee5b717ec0023b78fa0a5e1c975d1f3ccb82efb8fbb767d9ce88851e034d9744837d59ca93b3f6c685ba07a0663e7103382619141edebc678ad5dedadb7aaf85c58289601fd583a35812aa0a15e7842d62674143b0e47ec04e5112e67d45ae9f89abdf73a421f0693a782e7a027b21a1eb9d192bfc7c24029690a3ccbe16771b82830dbe86ebf8e1be58aa8a0a0331021dc66a180e92ac2e1612db10bef58e6b79e86c1823c2dd5a906240a0930a0cb5bfa5dd659f5b0fb20ef8cc9471fa0abf204109e5486320f58d0d930c3c77ca0a0f05d5b9e5364abc8803976ba186997edafbac57fd64fa92a653010f62415b5a0001b4772a49facffbfd536e0e2dd12140dd71adcdf5a7e6d489e329b061b2585a0a029809ef5d8d359bd8f7fecf169578d49c6010a64b32c8649bd46e1bbfa65ea80").to_vec(),
            hex!("f90151a06801d40737cad61186b939eb4104c5502273d26c535f7a37c34a63abf6e4dfe980a0b31f02a6b752f8eba131c4ec775ce0444ad2857792d19ccf587726dce76a273280a0027f124239b915e70ef1a986bfe2dfac456fd44237382307eb754ecd44e3821280a036ae5c8e0cc6004f5d1d4c66c236f7930ba2f64cbae6d9f0363876e47a01445aa0afd1c866b64cf9611d0c3989dad1a383fc035cb72c0bcd0882444d77ecbe0cf18080a0fa1915f63b7faf7d5b2a76aaeeaa9a61a7ed451c1f4718e8bf3e5af77581eac8a0e91198c91dcacf9d20eaadff1f6532676f2e377287e68c88b043d2aefe8e9119a096f9d03774a23f2570ffb8cc4298ffc267f4364bc746069975f21c45726fc71a80a00442927e748e876312dea52b28dfccfcf0b18b3daa10e62a317c40e858dbb201a0e88950e7888948366eb14c601bbc63a1ca546fba0aedfa5a38df928e5ee7982f80").to_vec(),
            hex!("f85180808080808080a084e54b0b5c476fbaae3840e18ea5f1b54d2cbfd4b5d11c3945113253b2966836808080808080a0e71dfd63f0c4ffd6568e8230d4ab61ff8a344ee88295aa8da2c6a5eea96960288080").to_vec(),
            hex!("f8669d3eba0fa7e2848bcd30e1bf958707d2a02f8f03ae438d6622a21b562c7fb846f8440180a0ab4f5e5ac89f9bed9eab40a5b02763168e73ca32c5dd9f5ced76ae92815e42e1a03c789e7c0b32cfb991ed499ea05ca68b7ae8e89f5ed2bbf04deb5878c4018f68").to_vec(),
        ];

        let path =
            hex!("406f40eeba0fa7e2848bcd30e1bf958707d2a02f8f03ae438d6622a21b562c7f").to_vec();
        let root = hex!("165f651aca44dc76ac642127d4a904b2270b22459c13b1bd5a360ea25f314f1d");

        assert!(validate_proof(&proof, &root.to_vec(), &path).is_ok());
    }
}
