#![cfg(test)]

use frame_support::assert_ok;
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::bounded_vec;
use std::marker::PhantomData;

use crate::chain::ethereum::{
    EthereumProof, EthereumProofItem, EthereumProofItems, EthereumProofValue,
};
use crate::instances::EthereumInstance;
use crate::stub::*;
use crate::types::*;
use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
};

#[test]
fn test_send_register_job_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 7;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        let ethereum_contract = StateOwner::try_from(hex!("6a34E1f07B57eD968e72895690f3df41b11487eb").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "7053568cde994fc0604e606e825b99713a941238bd76ad3070129802a63f7c2c"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a05c9791546a11d4c6442a1df2ec1bdc7a0a7858fe41da81ce8a5e3a29f1f733a9a0073de7228240c9955cadae4c9ef490009424061e244b91125771894f34d33cb0a03791cc1f3471bd6dccc163650293685f63a343ed6501b5de7833a728da0dbe8fa02915cb1f96d0b9380fc09411e99c64924d055c50134f8eed7b87e7657793b3d2a0a93d392737daab17a0015ea1a40fffe19a629d571ed98c04cd7b11e5adabe9d8a04dc05638c6579f768872a8709e07865dd491176d5bd49e03abaa7784a0137783a0e210b9ad9ce6ee090fe98ebd038c16c01c79b165e20efd18eb6af3d8d95ab5eda02fec982969b4eeddaa4ff5552e2e7a6adfccba57c62866baad9011f4ce2f6d1ea03dc5a40ed32069e7b905f5c55e84346bb3e8b93a1542d23eb6794c33143c4763a0d87cafd88922fde2a1af33cd42dfb664c0b876cc5ea605cc95949f8653c27cffa0d940e63b516e1cfb626a7cae433611fbb639457ccda745a792da439a54ab974da036dd2a3d52e05b6e0c04cc3e145edefabdf7710def4edf09e75b29b7d252ee50a0eedb24fd3f1a137f7dcdee511bec01208890a3c2cf9b17692570fde829ac756ea008fe8dfef989ee4d06171dc07da1ccc404f5f70cbcd48d26bb54c13953b84e08a05e40d858e92e71f152066ce95c215725368c4a234c2feaa47c29fb6d81f86eaba0600cc45ba17286872440948e60e76bc4a17cf01ccbdc99c8b63bef4a3547981d80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a00e772d0dd8ed5a441089d111aed89daa51cac46785fe1a773eebb8a6738e5abaa02ba989b876e8b74f00ee2a94fff249dcfee582e06a1f745fd36066826ca11a33a0f8be97231b608853d0043bf6f2f683bb123dc6589206921154865be55e4c89c7a030d07d5687b8ede5d0ddee3aa9317a42d70fdefa7e67cd27dc5e3427e797aa3aa09b6a1e03447df2d54ff9bf9bd4e645ff671b44a79e0ae0588eb2bcfaacbd720ca0279891ea1e0f262c0f42dc412a19d1b59302f739162730a4efaf629818b84f03a08c79a8225ea99886611b5c4633958fb2bafd4177dd2a38730ba34d04cd3b0803a027e12aaff59fb57a2aa3191842dfe663f41df7d7e17a1ec7e0ebd57066ac1b3ea07d59951d98ddd4bb4a6ab51b565c67f5b4b4f9b7c2ffad92878d7655a9b73b80a035b223c54ada410a617dabb903a8a050e123ced8d932a156f6157cba9579520da0ab3892b024331d9a60d83a886456fce322d9474d1b50c0055fd4d2c98c33fee8a0a3eef6ad02b1a64b0319cc5830e8d8778b621ae08e27057d83dee4c710c60d7da0eb119eba12c1d4c7eb49f9b81561c6b9a940de787688b45ba58952bc89ebdd79a0e30fcea200e1883b565d2a97cd5e2d105177a8bceb2126c7fe54e245b6876bf2a00b2f89fd7e944350197c6bfd5fa91a1ec275fca0277a1e2ea42d48e75451235ca017af88320c1ed7bdc5924748ef68eacb0610f85a3487134f357d73524db4c4ef80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0cb9cb721d828fadee3930a838bae4e723d0d2ae975c6ee66bd8bd0bf962c1b5ba0d9ce4c73c3c1697891743d27c23d52956b7c117976c7efb4123dcd0794f35c00a04293e337781768f2226fdfb9f70fd808fb83d6be9dae83e54341ddcb1e7c1e04a0c0ce8ef799645dfc16d843023f68b31b45f9cce0bb8c3e89fdbad83e57c02093a021fbdefbd454c7d266a93d63267008f1131353ea093c89e32937381085b92bbda06b7089b755e5aa973f7afc109bdb632e6c3614b4aaed47bf9312a48c74a2aca5a01dc1e12aab3c73f0433941743c6d06a4056529bdeab41812023d4d15d9a2285fa026a4dbcd7fafb680042c1ff484d3afa24fff4443147513ba64126f9459838544a001dd8c50d6512202f28e0d1e0c2b0cd32d0970a52c84c846b7b28c15b369e768a0385506035fc6f494f127a7547fd3795dd763afaadc7fb4768c923339c13a4780a02cafdff52916b9e46a719c1ad97e4ea04553f6beb0bf1247276cabef706f911da0fa63960ccc3d66b00373a1a5f4fbf4442a6a28559c4d7b46dce73969f0d38d8ea06b2b83f49db52aaca370009ee0d464aeed390771c0a1da70826c9562d404f747a087ac06d6b94e97913f40043588093eb046f94a6ec7db6e5e606c4b15025d107ca0cb453218a8890b0ebd47d2d4337d5b8ef5b8128faaedd8988e3b3be3d13feb0ba097b3c37ffc9d55d8043b887773f92deb1c9a08dcda0d0ea76828eab27064955380").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0b088e9227a8c3987a638adca0105d7d595c7bf1a7750699398e518d95558ad58a0e7e27903c822d7ccac3cf35491a3a4ac8a2bb8e2b8e4e9997ce55c61623c5d0ca099665e3138b6386c42655f4a4cdd03fa4c726010238bb33e47edb748a52fd778a001c7ee9ac038c3d9cb416487614fa9432e6526f3289ee64cebaf1b69b5246c1da08d730b76897e409a388f7d9c9598342adde6f164dfe1aa45f6fa54536a404438a055afdb4b988b8efa2fc9b30edd4272239455b332b23677966904ce98d894449ba09c28aeddd1c92cdb97d28cc065c4062ad130bf99e4658b9cc801d04620e8c157a0ecbc855a2fbc6f3b980daebdb5e8ce26da418672d6b0626ad2dfe5f210d3dc7aa0003348c9993d250d7e67c1474c81a0fa45b1a984a9cce71399dfcb196552c0e0a021f176706d2ed2ad80bf38d76e70e7d8eac721befb330c9454cfa086d6c667d3a0ee095925b3b2c0f2cec672b663ac40c220cd5e9e4319505c1abfa82100010987a07e6f7365a19d74135a01fcd2c23fd6f250d5c0ab186e75e3a560e72c06ffd653a082f9d7ffe1e43b25bf1fb4469294893a9613d695e4f8745bd75a76fd3f92df6ca03e103adba5bc62e97c779563454a1d40cea0ee2f9da4b770e025013c82c878c9a0717a9c119d54e0089300d7e85b129397451e16445bc4dc864e45917339fc8312a0c75293b894fab23a1b0c95127d0dd79e55bccdb8f03b21e81b822cf6b8fa562180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a00c3cf3dab87e6deefe49eb8fe0ea67288a72bfc0765069f23a330e66a72ddd0aa0b973adbb846243f3754860a5c3d970fa273867dac9a2ee1b6b4bbda70521b512a03bc66e06c39dbc084c89aac1b7a31a2e1454d1a5b5fac313fdef6c59e1719178a058dd9a85f310978d61c69ae10f4ea258c301733f386b038ff960b278f6f15c8da0c608bf61bea8927a39a1dc551482cd15f54952c925a4ee9ecf8299a7db345523a07d2423a6f3f44b0e48ee46a03df4e52adc5335422340b764239394e9ea9fc7a2a012edba4d9ed0cf9e2efac7ec9337e817905a8ddc119044935df6ae34fc20ec38a02e7e258bb05d023cac0486b05d2d0353b5f385a1a8abec3a2bfc0e8e0a3436d4a0a8ce06af1101a64d28fe2f186901b45f4d748816009a58d21f0596fdfaa2486ea0a89d593d80a4db3bc0d47a91578c99840cd8a79a11043c7ddb05f3299bfc69f1a0fc1746a4ee20de1f16a89fa366c391c9931eef5b3d09b5cb39685805e27fb4d3a07b3ba10e6816de80a550ede8ac36cf46f62dd5656bdab4ce5c820cc15d65f6f4a065ce34c09f0d58614c1b2201d97fea9936a55bfc281d0289834403b64372d8efa0b54bcd8b067d6087b0620d79322751dd0b297c269b875d871af5eac3a4997594a026dab4fc126b924790fcf74e7065a12bfd67d6b11b600327f3bdf5e33861039aa0dfaf0a9c9fb7735fb78c99f3e846f2889e8e395cd0f5d3b074e0633dc89431d780").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90171a053137fd9f832ec9a4f8f084c4a94789a6108586dc4b7a81d7e45464520406c2480a011ab943b4f79a6fa5c14fd19ed95c33ed1b5b6ab1d79b7b97723bf1d91f2a84ba05cb67fad37abf959c3a74dfe692ca3e4a15281cec939f1cf275a2bb78e15744d80a05034784476360d10b7715d880a84211337c92867a5943e5c4dd2c96b492ff2dca02b0fb32dc101a5dd42c7dde7b61f2d28c1e7334f296eb8b6394b48ecd53ca42ea0baff4073331f05a6913d91b50662fa0e6e97d30a8b312ae3424236ff9c8a66f2a095eb06d7ec5d02e63123c34044808369ed3231884687a4d2ce45ed56cda1d83580a08f8c5c7f2f749d90b0902e2c3da3fcfb8711504ba1e84b0eb2702c24f7b87b00a053e9dc5b5ae151e773f0305bbac7374fb050856457da1e29335230c81f601d28a0c362f44f620e38d244f3984590b2eb8bcc64f2058ec4d7c5217c97a9bd2cccb98080a0db957775f706697b313e0891476a0884455bb5f70b6588d049b1a0adbbc5fc8c80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f87180808080808080808080a003f13c8f241c0a3b01416e16bd3a0c5eae8e80a4c069dbf9912186add2f57da4a04cd68ab5366c84a6b03eae23d2fd0613b4d9ad923fd534b705fadd3f721a1a78a0b505b13863a48a79e0fd1f09a18a2515bde23ebf9186d5b96116461108671e4480808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d339622fd0302d2b4edbb8e3cbdf2963042f5c5934c78f17b88a2468235b846f8440164a0c3cf01200aa7e6c788669af5cad60590e314578e431d2d077596ae652650a261a0acbdb9b52a321f2759dc9c3a37fbd1445837b0bab1a74ce640f34b121f4f9325").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0f171bf2b9dcbb69b4e58b644acc664cafede5633e856c12576f9187fdedf6b14a01d1a7e909cd2fd46e61bf107078316eaeaa65ed7e51c9a0a2010d9f19f7acbcda055e941f8e9a461867d0bd8690bc3663aa4a2485a2525bda8768c4c1ece1b296fa08daaf2cf11c4345bd29e8de08642ddefb9999a6cb08fb6e0cef853b4750a2d20a0a36f509cb02d7011bd1bc4ab3006dffacebb667fd61ef4d267a82816c7517a33a0250ba258a24a394af71a97286f21b8facead6594057dd3b5c9df16e331e0adbfa0575dc068430b69da7ea9d7a3e89a8a3fbfef8a8a630c8f311bc12f1334b3469ea01f2b3dc14132355d446f0013b65228e55fe71b2884aa0bf993672734fb65a04fa09b038664c824eedd47b44e5c82acaba101323da3f8823fc6d8cbaa0c1daa72cea0ecd908afbcf4425c352f8489c58cee754e95c17935b09a6785e010101fd4d084a080346321986c39daf6568ba1b5a9ec8c61e111747a7b631696d81dc85146c282a0cb66748d597fcd182d2399c7f2097ec9009e0520fd8b57100984f5f9a131d801a09853c1fc01106b2bca044f0471ee3c7f32f759b79a1099f99be601ff8d851f45a071c14c32c385b403fe0330b4fa4405581163b8a6e0a32d51111db1c3df6cfcdba0f1fe7833657f747e262096b16f7dda413b98b9cc443055d5a3a54b1bc9c19b9da013abaf6801a3c5b31b1e39eb7d7d48affa0654bd996b7b5010932e8141653a9280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a037f95bf9b860c01d4d702abd2b530a36520fe6fef8b5598d7c9580b61fbd5b75a1a03a33e59a35fe781a0e985ff609dee13df0d6ed90c83644c3b261d97f2a7a1b7b").to_vec()).unwrap(),
        ];
        let message_id = 8u128;
        let value = EthereumProofValue::try_from(hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000001c0000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000001e0000000000000000000000000000000000000000000000000000000000000028000000000000000000000000000000000000000000000000000000000000002a0000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000").to_vec()).unwrap();

        let proof = EthereumProof::<Test, AcurastAccountId> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}

#[test]
fn test_send_noop_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 0;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        let ethereum_contract = StateOwner::try_from(hex!("9b526A28eB683c431411435F2A06632642bCcBE9").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "660161203bd2b16c79b1e003d39fb65201c7b961355bb130b6ffdaa80ece9737"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a01e6a427425517df2b64d83df8c8cc2577227a3593d1ff8c1f43200ef40857fcaa0dd70148b3f76a278380eea8a0ed86ae725b8acd7d3d78a5092c59fb2d011990ca03809e04399911d5abf36e120b614a9da13ac4bcfeff658e391442e16d257d4b1a0feeebbaad3d132b85373ce0b713cde63d9fbda6e4a8920b232abc0c47ad63fafa022a28681bca7ede4e347b3a2cb6a0579122201f28bae7cc64eb2c04ff398da92a04d20ace3c48bc32801ea8981b962521087cb7252c3d2720548c5fddf8ba35d8fa0f2a796e896270c04ee6f37375219cdb84997c53123a83ea1ecfc4423e75ce7b0a0f50a317a8e7480ff78e886b6cece9ed7a931300a6d88d0fb63ba89c4d3afbc96a0be46e711f140c5e6b546f7a009f9a985531a9a4e363fcb536eaa01a2d2ed9070a0aacbc1190a7dbc30ad85284a0c7568535575836b26376e18c264ff02553d957ba0514bcf049e1802ce3bf91ae4a9252e460e84bff4fcfa92f5813b8738b556e380a02af47e0f88d5641608d03d3751f5905d32b56f6685bfd79c7153ae7977af13d7a035668a4e9099f0071dd53356da6d6d08b24f48f03cab9d7b2e70f8b7a9563200a0a43e25bb65f98f9ba2a9999d6cf5da1da2019919a3c95cb953a7d1eff650649ea0ede0a02bb3d82444eff81d1251c7e8f3c1ab8d84824f87f2c364ddb02e1ff7a6a0bd5b7de2e00ab0396fb9a42e12a7b3801e4a8409bf439ea073a28b6a3ca8616f80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a056191244eb9f024361fa4704b6a181ff111d0cb37a13cf15af81b795c2a93823a0a00ecaf344c97f51f111c8cbbf7897a64c93c31a1663cb09cc58b05a33810c8ca0398a72a9a9bbc6091292e6871291292fdfef2f85f8fe5ce520630bd477e55cd8a01a62908e6a386ff412d5cba050f9485530b36fe14aa29e668fe87335173b67afa00c88de9fd09b791e4b2c02ebabd3ebbd80af2e44572f1ca46447425969ec2dfda0637a86f1bb24273b42378a707cb385785116e4e3d86c3bd91761b5e501233ba4a082d9101880c166dd752bca20672be6a17dc0af25f675ca8148fec20986e854c8a058fba32e4201f8591c8fc60ce1f3207eaee055e4bd36f1cf08bb40138f001f38a0c8e3502b34b734bded6bc0595de7ec85e83e8ac9cba6619e1703c07c91e58d05a03ab8bb8772b4af2373e8a5eb70855015fbf1ddc0db13e7f19b8b61ab3c940bb5a0e0af59b0a454d5324c031dec76e0c5f68338d1fbfb5f132407d762ebccef108aa0e1a356c43f883107b6098ed1c181fe1dee91aa2d30803ae430bb24fcd7fb1dbea019e2759edcc71b15e5e4e8dacf895ea5e488b0a452720eff81088cc8c55835a4a0fdea5706ec6d2ca3af188022dbce15c4d042e6c103a1f5949ae155d121b56238a0725da3cc14339dfd78cc4f932f74e5559c8c1b06bedb071a19a7d7495ad1d542a06ff2c1218fd9a0596e5603a4b2da9ffc6504406277f8112b4427578b548737be80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ae4c2efc81cef10fb81e95229054a07f026c22eedda2582007beede43e478769a066eb86f82b30900eaea429345d4b344bda3d2684267a7443fa063683de170ed5a0da5492911f28cc7e13220bfb1ba84024a4436b319e4470c926954b30dc35b467a0d9adc4937283c6de302f5fbb6cbd128871ef27930ca3b42842e27d806ec65fafa00005ea9ad2a44b9779136735a01a9951726c984c8159c6108ca320e84306ba78a0eedbce0abf3d12bcab3b79cb0500f814913ca66dd522628e6081f2d7a2f316bba0375ef1473700c00b2f1129b4201ace1b91978a7e93a4623aac3dc9d8e03d84fba0bce9a43f1a91483bf3e8b0bca2db7ed9d865489d18351bf4bdfbeeef98a2c1eea0896659018b5be4f81975052bc33012d75990a3c4a3733a05707fd443ea6e2efca0084ce973bfb50078a182dc9de3d46338a9608b103e9bd95f3c14d59f890c8e60a0edd2176279800065a96d4af687238f4925c6f79e1af5fd0a7370097ee3c7a2d2a04321c84e8442f2c04f028c47be1746bbe61be1bfb933b5812e25d38a63655d79a0d68f9aceb1e90eb19e00a300f4109334ae9328b88a8a363d13e23c98eba76e36a0278b4b6ff61e30c699ada71bb82ada9967b8358d061a8ff5e93161459c3540dba06c43fa474d56eb93966b4f935c7b3b2604adc8c71331bdc9d3b12fd1777c5533a0b3b8c2b0b7e60a1084b821ea5ba35ceba40e77f5f877bca6297ff1f458639a1880").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0e23ff66a0f8ad62bb784a7ca8fbcc6d854c217f31631e0711af956b46ae82f7ea0e0ae1750b122312c3969a49dc2f4722fee0a8bfc8a03f99a2d6068fdc9c0476ca0f6abdfdec088980dd6490363db91aaac1c59bfe90432bc1ed7536aca9033b30aa064db2877e3ae60e18123d188d278b7cd13a956b4382844ad7e6c73a4b8c4ce79a0923951b2265bdcfcdcee6725345faa6d4319e87b6c5987cd4ccb765519d6e318a08eb358b0b35c495b0cffe6197f2ec3b365a0452cfa2871a496cb7fef4a3bc287a09e7dbbbbce9e4b699b8c80b7df4c9fe7438da63ebf3b225a1148c29fa9f615e9a0ddac088e32487c58240edca128c7ea0e06687eba3d3282ab1f60f9d37c517f5ba0eb804eebf02f22b8094da8df48ef32b0767a65b1d1de46afb4db5a35b14d936fa022474b23a46ce8f93efcf5b4f581237fa21b0c6b79b48ef444a58073835fa205a07dfe0b469ea9d3402284d638229aeaf013144027e37e6693feb6e88e9dceb988a056195a91c1924e0069d8fbd18f8e07e71fbb79359206c3b969a76ff235a51ab6a028573f64876850d45de94058292474604aabb75306d2b9631af69e2794f7f1a7a0703700caded78244d5f7576ef0acae92d11073f1a1f11e4f30c35325f5732a36a089744d45240d7eb0c66a1e7065b902e1a223521c241683cad5c0b4229cb04cc5a09b123534f8741c7385e194addb2e5803fc23e8c88ef0a8556b7e2265ac47485680").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0710b8047e020c963f2d596cf850131fdf34d32a1e000a78003726f6d362e446da05a5a90589869b57cbf6c632e7263edcafec5a1869d3073222065da8f7267e842a01a219600e7247c0386922c8f7644509a6adeabfd9909fd314786b5651969e375a07f973d4204d82be0983a22b50122d65c5c3079e2a3fd95023889f62793ed0fafa02fabe4ef80a9f2f1b8acd34e24f710417054ae490b10bee8b8f8a16b2051ddd1a01fefee09456f00e2215465df80dfc00efc289e9ca63407cbc98e2faeb4cecb81a054db2bbb190bb88f080e5ac1c462fdc2f618d64ab847e1ede4a220f210a12021a0b0ecc0a9bb68c04e655fd79feaa2e9835e52301166fe1f96d542cc8c3bbe2379a0bee5d780b353338ae2ad70fb61505f6dd194b4562391c2ce70c58a244355b147a0fac5446d3d4d0fab590373adeee5d9b702d55a70eb328160705dead5f85f979ba042733f5ef0e62d49350f0362e4ba410c7f69f168c190d32b7b5fbfa2c5d32eeba02c7d2b55e55f8aebeb2a05d2a83608709bf7247aff4ca12b4f4c13468e5384b8a0800ca1fa1006d797a2202e2591585a1fa9bca1dc6a2654f0b5a13a6aa4d25832a032d202cbed4555fb371af18d1c128f04a4b0fd9c2e192da5ecd1f4f62338ba96a02075b53e615282142fff6d8696ba07b4df46d37e19137eb8a0a04f8e904139bea06f078298e09b67636c38570908cb070b6091ea522f81de14e2666606e00d13fd80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90151a09df9dab6099b9db936388e80ae73619a63cf601caa452eeb62b9236aeefef2fca0b7ff6cc910ee043957e45514798bb8fc35949a9b212566b2962851ab6b3667bda0b8a3288fc50969d8bc42ca59c8f8ec5e08ce04f0b863b2e7dc1a89cd3794e5dba0925c3c60db03311e6176c67cd906cd10d18091ffbf70368b540c5adb31571a2080808080a0c03cf180255aed0a8cd58a3ca8e3e03a3cc295535ca2d2fa23a3988e45bf0a8780a06f52262da5abdfff6c87993606334702f43835cb8024ea3d0ce805a1704d87b1a09e3beb75b5b4f57d32eeabb580a9e86f6a14cbaf6710005103318fad78c4edb780a0d78ddc950cf676ae214cdc652a6fbd28edbae3fe6909fdcc52656d6e6ed58e0fa0c58b052c7531f7026f48bc785701ea6315a504babda50768afe48964e68ef924a0bc36dc0d7250bb77f5b033f465bb4083caa502f43d58b61824a968edf7bac67a80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f891a022fc1c9807f482cf7b8985a08a2122e83db36528afc8359f6ca618fda115f07280808080a02c790808fd07adf1b29e9d6e2f6c4dee1e1cb1c5869ba20baddcc7f1e0ea8c1ca0922f03fbb9164b5642c6bcff215fd68996d110d21de84f4218e4b771c166297780808080a0e2ced53fe1965299c88cdb923984280ad671f11d13929154f4ea7e308325c7088080808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d3f77bc3bbc1cfa5699cadd3850753e93731f02f6bf025f1e4ffc3fb788b846f8440180a0865742af102ffd57df06bdb6d58b31c8a76e368332f4b5db7386e5ba450eca0ea0dbe350999ed56f5a428aa0d998f2fc2d98a8599929bb156ca57dfc0fd5e75022").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f901318080a050408f10cff51549e48941edf16618f3deadebe590841243a50a40c69c5737d4a0011435f9acad83d797c3a1e98645e70e542cf8d7ce9ce4505b1e6881796bf241a0edcbb77044d384de05d37b2a615340234d578c7daca3796f2519b0cf72a4c086a072a63143fe3d941b735283c159ff066edb91b18426a20ca5a19e0df099638ed28080a03fc5025c19eec41ee8be0ae9ec9137f39343cc6f4b7bf33a328f7d10cec56807a0aec936740144389f8f764564a3b612f6720233bd77b4e3bde348bfcd330b950a80a0749b7339a025def255aaac6af7e40276f1c4cfeb915d92ccdf381635f7f6d071a0f318db4a726bbc999962a9038eb9ce393bd640d84f1c036104e576eba84dec2680a0f1fe7833657f747e262096b16f7dda413b98b9cc443055d5a3a54b1bc9c19b9d8080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a032401e68d452ad3af4aed95d6e19d1a690ed00a9a5bee1b2b0a83b6028446a4fa1a037a6ab991fec5e5d7cc7528cd5357fd0808bb7186ed960a0174e859a1c8851ec").to_vec()).unwrap(),
        ];
        let message_id = 1u128;
        let value = EthereumProofValue::try_from(hex!("000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b00000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000").to_vec()).unwrap();

        let proof = EthereumProof::<Test, AcurastAccountId> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}

#[test]
fn test_send_noop_message2() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 0;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        let ethereum_contract = StateOwner::try_from(hex!("047673ed04186d8Fc465B296e52084C1B001915c").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "2d1a359aa8aa2528f1a954aa79342d616fe833363454c89ffef447d2d42a3593"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a05be23b54da75bb41d93e65960e5252bc1b60a4965a3edc6c61f0acafe7248ba1a01ae35c827171ea0161ad7573812b6e20f031cd335e7640aa9294101b3f92bf68a021a5fb7bb180edcd2a0af30f71cec90d3fe5318ba15a061e1f58bf95f47da4fca0c9fcea896342ae0aa7564450342922ae8ae2bb0ab9c49f66e6d3d6d7a54ed23fa04c86b7da20d86dd096e96451a176e4b522827575c3249a92fd59555ae6cb656ba01766ee0505ed01c66836a70f79ce0910040bf240aa1e6fb1f5dd7fe0e22358eca0ffd4edd88859702455c589f990b0dddf805e3106cae46b05392b3fc4cb06a889a002b9892b11ede9ef8c6dbd8ed2d6fb9357ef0a89fa8f1983b6847dd1c42678e9a0344c570610bec03031e2dd3fa6a0dedf333e9f345f47c7160d32e1ac2c32147ea0141939620554a3a2bcdc171b65d03e75d8bf6854dd60c3d0d885c72a78c92e27a08c2ed04e9cad97bb70a9bdb81eb7d8e3876ac40e683b3334f0bfd45a4be83cdfa05c43567c058d7dc93e1e4e5c251e9ac66e24435ec320272b44b662de90750faea05ae55c0b60c5226b1bea12861148740f1ccbe37b5d1fa9cbbae58716c5221e8ca079bee02c0bca0ecd198a984f04c3afa2fda69d728b6ba881230e21314e0c93f4a0780f26d9403ebff2149f3883a5dbb8a4a3ae9c7e144f73baffd3da037175df6ba0b11e8af32ad6ec5bb26f28fb55caa799bb28f56b7d63aeaa5dd676e72817b79980").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a035cf2e3c5b5f258e59b566b2aac98bd41cdcda74eb8c7f3b367dbfa1fc758a8ca092b18695f38728de171ccfc5aecd5ee2a63ccfc2aeda71e2a80c636d85913af8a052a81f210543243f8e07ec2d4fbb03fdb1883a946b482628846dbca9706bf0cea0b130dc4f13445cf87a1806c3a86fb11121c1161d0d064559b0313ab2a78a4b2fa03833ffdf448b979893778a2694ffd5f89701031aef7d715963c3e92f7af56a81a0c4f1686f3aa223a13f0ceaaa7b65e21a1271aa814b7b9c705659f7559fb76398a08a0ebae3303623bf2b00144047346f12a58cb0aafec4c099906302dccec6277ca07ea7c8ddf40ddb4002551417af824b1ed5432e6768d16b13ff0c497954f0b401a058dcef292ac0d6871e0a4ae9874e024167ddeb585201c237ebc1dc280b9b3feea0df8652fbe65ab73823df4e0801e9bf2a5a099aa4ab8c0b9708a41aea6673f923a0d806ac7e44cd77cf76870d8518b40b88af925cd133579c5687006731be649afba0e1ec22bdf5621dee2eef67d11dabcce8d1b0c8a9704a4cfa1024bfd5556170faa086c5b6a6d5e2ed51a1ade08b46ab1413ddddff35bfb59fd7e7a3455ab50c2e6ca088829eb5db9e6fdb934d1589a4a598c09152bd6b3e5229d66bf74e6a95fd4e4aa06ca78e94a357ea878dd5ab38407126c1f2900bbc7deb11200eae601a9b2c5dfba0cf7f67feb6e2504048f9567ad204e4b4e0863aca2d82467a4684e71ced78ef4a80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a027ad0be59f57ae61451ba4353bd4cb2d7f05484f807e670649691371ac73819fa01db532747d8a609b89a5b07b4edbddf6ef2cf2ab52c545679b3892794823dbc8a0443f73afcd913983c60c602841b440830f7a7b62a347b3e933eb8d7d53ee5684a0696921d939897e625b74106a7cc1ec372e14b742e64cbfaef81310ae46a681c1a01a4ed797ac719cef55b1763de1baf6b8dc846af760b9cc9a6f0da43afacbef4ba0ab9500765e8fa2b8d4b5007abb2cd32589e902e87c3a58e9cc988490494f340fa034c99453f09afe5b3f0aef4c076a52d6a109e95266d367dc88022391fae73ad4a0ad488b347bdd8ea98aa687ae0dac3bd8e7fcc0b92f5e71017417e55c85bd1773a098f5baf6e1c006276d764ede6a71010bf7d1a088df880dedfb36579a773c8290a095f30325c49c8fc11f604799aa83ad23297d2090a311968f48a7b3f3392b4d0fa02073b82b6f05c53a5175035fe97fbd1a7ade2f7aa7231071d21424af988f716ca059ec33b936a1b6c4f4652a1f8f596a43a6c4a907be7200314dc19eff441abc00a0eabde7d87f46e23daa218ef4da773e2081ca5097f61f71b97a515d5cb07ac084a05afd17c49dbb1779036ede37d83656660d0a914bf23b13757a5a9aad9ae7ef36a0e44e8cbaaaf599a07643bdde92c2144394e3b3a076de0f365c148af263e1825ba094645b5a91266ef5a95908409cacddb24bf1d17efcd0aba4de38527ff072eae280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a044c37f5a261b9a6197b9eea226d169e671cd95941b357a32372c209b7d68d0dba058783686a4d6400aa5bc6e60c7280d7a1ed8b46649be9aa411115f999e95f865a0918fe0e760871e56a22d78ececa85ca370a4c01d1617cf4ef2a767ef07e2a1efa0fb96f199d7da48e54d32fcc538fb8b5de612ee0af3287c36d1079c5547c1799fa086eb93662d890cf6852cb2a463fc5b642e1c5dfdf639aca1a935c75b509afceaa0519cda69ae2832a57385160767f048b9216e808e528965f18fea26bbe7994bfea0f7a5c5f615de41611cc5167e5cf200992a6d25e918f676baf642a9e940faadf2a03c4cb43046d1b14dd8ab94efc48dd95e5f4406e08c64ff1f26be75a9fb98f3a9a0ca9b5209490cef42b6548c06ba1b92602712167b96ff9e9cc1d5ac20e0171f0ea0d39b1338d11801f0011b26a27bbae5bdc22f0eb06d8f913164a71e5701d9b4efa0f829ce484984492630aef2faf7df7974157d917c3b24f33b932ec741015745a0a0c588b691eebcdf803472ac3706ba45af3f4ebec45a0beb31cf5a5dee5644e1baa0b095f23633fc6e7bd0071f35c0ccf15692a1dbbbb74a184e5f57e2ac38588258a094d0685614893e181600cec4ede3ad680eabeb67063044031f61be297b3089eda07e046815c58cd2c19f4f828234ecc51e43bb410ae77b2d7722806b987e31d159a0b947c1cbd7efe0596e7552836e8315e33eac46bbae36159048ba9ba59807aa2180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a02bf2e3a0ff434b4789861381ba817df67799280a58dc88422547c5e7f60c44d0a0929516bb33f603b109f4123f701701408e2fd97dbc213c73a722c2cbd488d904a0e027b0df5499246716419678ae2f855730e5b1bee3088f448fc01ec3eda89b01a0963105ab73b3147825009f68287acde58a6504e3f830148cb7d75e2003b37ba3a0131cdf2612fbd9619858193dc240b4698c4aa2d72094e2b8641c6421fb1672b8a053212b9b50033e27f95bdf01d5c60778fe8bcd3ad56b4facdf8908d098faff3ea04d218549078243839f2a4dbb9f59a0f8dfdf4b8df014c8e99844814086bdf0aea01833182cd3e8383c5ee2541540ad8407c65141c004637e85f804a614daecae61a0ad60bd0086ea925aa3228ad06c9a515b1051d7386b62906c6e616586cdcf553ea0bab411bcea85af0d3827c9f68fc7175fdd4414bb663a2b17e967d4e85798c4b3a06a580efe5666a33a0a11003d1d94c8cabff76372309677e19670c92d0b33231aa0d7a2daf671d6392af6619b30c08483c7f656ee6a2a4ff1beac945a2ad0355a04a09b393622328b82b25f7a41227ab3ad76e8446274ee1e0c16023ddc5c899c2831a0283c50f84747cfeba2ccfc35a6fa317ea1420f050aaf14ac3fd0a6192d5ad961a0eef9808fcf7c7b67bc80def16fc611df7e769b9cd22789276b03130895a11ea9a0b3af4610386826603993aa02aeae85cb3c844465ba1d8cab93cf9695af75d23280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90131a0e62aa6f518ebc26dbf99e069ba0739c44cd56a9c3ed915f514331896139994d680a0882dc308b8265f0846be2b869a234c02f1caabb78675267ce737305ed3ae11caa00f05992b7ce29cd3906c3bcb74ce112108e2e5c82aca73ee430bda25377c5363a03aff1e1bf03f731b23a772c2c18dfecab4d5cc4f8c26bb73054fb9b9d5c7a0a5a0a91a1a41472983c4772c7df6206833ecc97bf2a0e38b4c6d86f7568be3d1d54580a08af0f6e0adca8982394cc56cb001622b791e6e5c549f7124a0f56f09611f08e8a06bf3218568736a6fd8d2fba1251eb4620f014266411caa7c6d5103e7656bcd8a808080a0e1afcb9a997e7a8f6449291678a389ee30eba694dc44ddc7b73819286b01411480a02c7d4c34248239abeed02e40baeaa240523f5b5ca71f6def9860e8bb2c278acd8080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8679e20bcac18887f8ea670ba2a830b36b0ddb825349a730cffdf504db758662eb846f8440180a04f8aa7e4475b6b6471c6eca4c137513e15c38ffd6712deeb5310fa36212e6b5da0aeeff0fdfbb5f2d17ea4f5b53afd7046e05eddfa9d2cbab1e06fa17c3e22c8e0").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f901118080a050408f10cff51549e48941edf16618f3deadebe590841243a50a40c69c5737d4a0011435f9acad83d797c3a1e98645e70e542cf8d7ce9ce4505b1e6881796bf241a0edcbb77044d384de05d37b2a615340234d578c7daca3796f2519b0cf72a4c086a072a63143fe3d941b735283c159ff066edb91b18426a20ca5a19e0df099638ed2808080a0aec936740144389f8f764564a3b612f6720233bd77b4e3bde348bfcd330b950a80a0935d6a53f3651dad86e30f412f93f740dce0daffe39c603e32897a8af2a6e880a0b1103e91b79b4163ae406817b44f7e0b4d1e33004591a8878e04148c033fc4bd80a0f1fe7833657f747e262096b16f7dda413b98b9cc443055d5a3a54b1bc9c19b9d8080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f85180a0fadb50fbc02b1f23382a8f33ff8b0445b1e83487895239791844c96a0c7f4811a0886036e1e948ebcfb6e6333640dbe8da304c4ad943b0946f7887b30da58582c08080808080808080808080808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a020401e68d452ad3af4aed95d6e19d1a690ed00a9a5bee1b2b0a83b6028446a4fa1a037a6ab991fec5e5d7cc7528cd5357fd0808bb7186ed960a0174e859a1c8851ec").to_vec()).unwrap(),
        ];
        let message_id = 1u128;
        let value = EthereumProofValue::try_from(hex!("000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b00000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000").to_vec()).unwrap();

        let proof = EthereumProof::<Test, AcurastAccountId> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}

#[test]
fn test_send_deregister_job_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 9;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        let ethereum_contract = StateOwner::try_from(hex!("6a34E1f07B57eD968e72895690f3df41b11487eb").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "f111c090a16fd8dfb1dc9f6be75e7c01c11ee0fd01c00a1379cc31c796b0f609"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0154f1e9bb831cd13d04c0cfdc2d4e56cafcb164a1f258ff9c034b407aa81c702a061a403766bbd00883d76be46d7e9e4847f90d7b4beb405a3d0977817c7f934d8a033212a1a78ea156975c05fed52dfb2c5e686baa7963890a7bb2753e52e6630afa05b9374f6fa0a3f86b22e447bb1f8051a458f2652a517975c22a9d59a8ff96076a091f5ccc2025e66263b1ae75ce1dbc6c14bc0f769c50911d4bb2e18d9bcf5a153a0c7f5766cf2f85b382d558514657fea4ef042087e9667d23787f05bc547f1635da0348bb06f67ff7ae65ec4d1632efda68fae55841a86909a67cba88ed389c36c02a0c84b59f11a1f5c29c08d118166fd1345fc6f72f652447dad107756e8610723f2a0455667ca83f8283dd14f8bff5f52b1418e8d1e594168f8456b7116f08369e377a02ca7f57c140275bd0777d1593fe869f53e9050ccafd8e4052804f6f64344870aa05ef6fed064e20f48065e464d124bba78be5d159cf39862fce0f149fb67185405a0e9cc615cd8312767e9b6c47ef9b623d32a324071f88d6aec7b5e6435dcbd22e0a0564385c88a66c4f9344609b64e2a87dd12617f3c36fa1f52c120e6620f277fe0a0cfc67cd838875741fbb9bcd921513e21d4bc13625735e12dab77a2e7910028b6a023a0797ffbbb289480936ca45e29c0db9543edab22baf18da6f9e888b406378ca04fe06425fec14e36f7ae8009836ae9a46fe27171b486db965bded769776c8d2180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0998bac43c14c2c1a253cbe3f757b42a3900cfd25863b8b9c267de2ff76a3adcfa0af0b5b0cd79c1417b428cad0b46823f3d13179998fc5672e05ada339bb95c7e1a04a9c6472e6d3c8248cfe93ed752392a3fc44870440cd06cf45d95aa18cdd9f90a02ff5e12976f175a070d9af587bfebc5975c78c20fffca72c64ec7766137acb50a0ff97d0d436f13ad22face00f6e6275cc26a9adcb5160099ba0a6a4e9f1f2fab2a05cf1002cb87f2f9e050188a4514ec933f765d12afa2799dfa61d4b1711d762bfa0481fe1a1565f5db61fc3062aac7217fdb151ea706221d63a1db7543d5fd43f05a05b049e3200f421ecdefdbcd8cc1635324bf2bacf07d03331fee8b73c547f8321a02d0a34839ba5622c5d92e24b311295709a641913c0f98fa7d013147fb2b4009ea0399aad162a009fc17a35e9abed0a6c0246043d9c971380fac746613736dab1dda0a41593d6bda5801304a4fae153ede4beddd37e510051096c8e0b82424954166ba0bf56674e22bf502e995690adca67da40b4933a8693cd0aa78a2db0d8274071c6a06bdd8c0f7d50f2a1a85c3b79e4685065a3952387e22b822ec83a8ae8ec031bbda0275825b4da0292ee1ea7e87cfcc80e11bfd32465a2ecfd66d3855c5946b2cd5fa0cb7403f57d6d52d6a5e810bb1a937cd8aa9a25f838f0b502f84b7d1be6f98daca0c31b5e27d895869af079e4d527ea36e786b5ab7034f096adbdb60c9d17f26ba080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0306436237194a308ff506037d458cf2afd8a5118b8488aa33b9cdaf2669ae201a0598145fce0c60d36b0fa0f2e851cd4d67e6aae24b39900072671dad8051f9309a08d417f91b3f7c7e2de96c0e1b1662a12d1ea339adb59ded651e05664305530b4a08dd72b6ce2fd407f168b37b2c57735e6c9a8b75c44f07a19b35dbcaa8b46d2e5a048c2af74351d84719bfb39d0b4c824eb93b3197b513da88eb9cd38ef68418fb9a004e915b2a2ff199ea1eb1d252810ae0690af44f25554fa3ba2148499b2137034a014df60fe61f4bfad529b16de61e40d1cd3cea13b28f1ee6ebb5c0ce2ae4b7cada0e7d275a738771131cdb9935f64e93e06d959b52ce25b326afbb66122fcb95e20a093243753d545ba83ad2129763695cea1e861dd316828ca44009990d4c3841ef1a048a6e16fa84be701724b573751489dceac2a4103bc77766bd7f80e89faf7d9b5a01932809b29b5e646640f0b103e7b56ece0ba2bd3c2537c3607d08723e8f89c92a0e19117082122864af46a9f6b109efd0f220956e7925087c33db8c0b2ca443749a02c006c1809f7bafabed11a792f124c9e2bbf83e4ed42cf7c3a2378311fa150c4a043be036f0ae3546cf0ff89110d7c6ca7c749dc3ffe7acd52fcfd25a5c4edf1e0a0aeb7a02b8ee26f68a74dd3ca60c523d6fab232121d733285935017c72c60b195a030aa054fbd82c64134f097eca6dfc3f7def3e4e4b9ff14dce7d8c21cd6a9eb3880").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0756469afd770437fbac3e2da9b82c8841b819437ba2f34f368739ea25b7869c7a0670ea5f45563a615c824645b7d7e9c9e166c45d19265ec3fe3a3760ac2577905a006accaff23de46b6487cd6fbe02134117d8a86e3313a00767ae08a6b7f71adc9a001c7ee9ac038c3d9cb416487614fa9432e6526f3289ee64cebaf1b69b5246c1da08d730b76897e409a388f7d9c9598342adde6f164dfe1aa45f6fa54536a404438a055afdb4b988b8efa2fc9b30edd4272239455b332b23677966904ce98d894449ba09c28aeddd1c92cdb97d28cc065c4062ad130bf99e4658b9cc801d04620e8c157a0ecbc855a2fbc6f3b980daebdb5e8ce26da418672d6b0626ad2dfe5f210d3dc7aa049a7a2d7e70457787c466f9373e9a45cc9936f9c7178179281e71f79ffd60feca021f176706d2ed2ad80bf38d76e70e7d8eac721befb330c9454cfa086d6c667d3a0160821c6a95146e4ff217459bb7d94263ca5b7d0e0cf32c0a7ba5b53c07e7b25a0e27b7190c2d540122c7ac6f53f5af5d4c3b67ad56901619e1888bc163bd9c58da080f38bedbab0ad8b40dbb02745e2d06bb399d450fab3ad19e19838b769276b3ea03e103adba5bc62e97c779563454a1d40cea0ee2f9da4b770e025013c82c878c9a0717a9c119d54e0089300d7e85b129397451e16445bc4dc864e45917339fc8312a0c75293b894fab23a1b0c95127d0dd79e55bccdb8f03b21e81b822cf6b8fa562180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a00c3cf3dab87e6deefe49eb8fe0ea67288a72bfc0765069f23a330e66a72ddd0aa0b973adbb846243f3754860a5c3d970fa273867dac9a2ee1b6b4bbda70521b512a03bc66e06c39dbc084c89aac1b7a31a2e1454d1a5b5fac313fdef6c59e1719178a0f2ad580cc1faf073f412dcb742ec1b008424acdc40211b948c573360bd09e43aa0c608bf61bea8927a39a1dc551482cd15f54952c925a4ee9ecf8299a7db345523a07d2423a6f3f44b0e48ee46a03df4e52adc5335422340b764239394e9ea9fc7a2a012edba4d9ed0cf9e2efac7ec9337e817905a8ddc119044935df6ae34fc20ec38a02e7e258bb05d023cac0486b05d2d0353b5f385a1a8abec3a2bfc0e8e0a3436d4a0a8ce06af1101a64d28fe2f186901b45f4d748816009a58d21f0596fdfaa2486ea0a89d593d80a4db3bc0d47a91578c99840cd8a79a11043c7ddb05f3299bfc69f1a0fc1746a4ee20de1f16a89fa366c391c9931eef5b3d09b5cb39685805e27fb4d3a07b3ba10e6816de80a550ede8ac36cf46f62dd5656bdab4ce5c820cc15d65f6f4a065ce34c09f0d58614c1b2201d97fea9936a55bfc281d0289834403b64372d8efa0b54bcd8b067d6087b0620d79322751dd0b297c269b875d871af5eac3a4997594a026dab4fc126b924790fcf74e7065a12bfd67d6b11b600327f3bdf5e33861039aa0dfaf0a9c9fb7735fb78c99f3e846f2889e8e395cd0f5d3b074e0633dc89431d780").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90171a053137fd9f832ec9a4f8f084c4a94789a6108586dc4b7a81d7e45464520406c2480a011ab943b4f79a6fa5c14fd19ed95c33ed1b5b6ab1d79b7b97723bf1d91f2a84ba05cb67fad37abf959c3a74dfe692ca3e4a15281cec939f1cf275a2bb78e15744d80a05034784476360d10b7715d880a84211337c92867a5943e5c4dd2c96b492ff2dca02b0fb32dc101a5dd42c7dde7b61f2d28c1e7334f296eb8b6394b48ecd53ca42ea0baff4073331f05a6913d91b50662fa0e6e97d30a8b312ae3424236ff9c8a66f2a095eb06d7ec5d02e63123c34044808369ed3231884687a4d2ce45ed56cda1d83580a08f8c5c7f2f749d90b0902e2c3da3fcfb8711504ba1e84b0eb2702c24f7b87b00a006df4234f3046b11a21232e729192458899441db8cf2ea14bf211be894b29cb5a0c362f44f620e38d244f3984590b2eb8bcc64f2058ec4d7c5217c97a9bd2cccb98080a0db957775f706697b313e0891476a0884455bb5f70b6588d049b1a0adbbc5fc8c80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f87180808080808080808080a04dd86c4a4cd1b4eb14c871fc9d9955551b537567aa6f7a3c09da770d337b4133a04cd68ab5366c84a6b03eae23d2fd0613b4d9ad923fd534b705fadd3f721a1a78a0b505b13863a48a79e0fd1f09a18a2515bde23ebf9186d5b96116461108671e4480808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d339622fd0302d2b4edbb8e3cbdf2963042f5c5934c78f17b88a2468235b846f8440164a0255a828af5d7f876d57a5b418502f28361eb71fa51db4f6076244e6870657e10a0acbdb9b52a321f2759dc9c3a37fbd1445837b0bab1a74ce640f34b121f4f9325").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0f171bf2b9dcbb69b4e58b644acc664cafede5633e856c12576f9187fdedf6b14a0adad05bf1d009f2ff38112ccd218563a9fda3aecde30d79876b8c692d6609e67a09c9784f2d923c86a996dbb19374569587312111e657d0ace78bc0932fe862623a058a9c646426f4369ceba24b46998e7692e1f47bf1e95c444ba76ea03b7688888a0a36f509cb02d7011bd1bc4ab3006dffacebb667fd61ef4d267a82816c7517a33a0320c5bdc1e54dff5dd54f42bde8cf29337aa59a86d61fea5850ec16a9fefdcc4a0575dc068430b69da7ea9d7a3e89a8a3fbfef8a8a630c8f311bc12f1334b3469ea06675ba7053a8aad5c4ba429e36a9c5fb133fd3728b7c1c628840b4f1426062bba086137fcded667bdd7190c904c421a75aec3053bbf392a08c515f1a5cad0143cca028b5a1e02e5a43d341c735db20faa93c6b3607e6dc2348f209e356f26a2dd145a0e7b69fe574c524afc97101c8b09551cc05b921a84c820006a7fe43e1e8d8a6c0a0cb66748d597fcd182d2399c7f2097ec9009e0520fd8b57100984f5f9a131d801a012a69a51034205a5b5090c193e571f89d5e719c0a8cb3b64e97159c90e161dfda071c14c32c385b403fe0330b4fa4405581163b8a6e0a32d51111db1c3df6cfcdba0f1fe7833657f747e262096b16f7dda413b98b9cc443055d5a3a54b1bc9c19b9da09c056be83f26c5384827b944d077973d153265e838bfcc7af41acdc8a3d7b82580").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8b1808080a0f22f596f4c2331fc7ac5a9f18f2bfe278f2b91aabdb473ee24331b21b6fcea528080808080a0abad66e09d18a8955523fb7756ec85bf58af02fa5a227bd88d3b1b7b416561d2a009c547f15a438564b15b69f0ca49e3cf450dd03499a35ece69e053b858e42504a03a13ece6d461529c5904c862315fb2dca199ed37387299c672c2101f8d63842b8080a0972d41c8b791d485843f0433637dce9052762455a49e9368a313e97a30f153218080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a0206ce3e87b799e248b0ca18752116604ca9aa882d9372514ca58448e6eb584bfa1a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d").to_vec()).unwrap(),
        ];
        let message_id = 10u128;
        let value = EthereumProofValue::try_from(hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001").to_vec()).unwrap();

        let proof = EthereumProof::<Test, AcurastAccountId> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}

#[test]
fn test_send_finalize_job_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 8;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        let ethereum_contract = StateOwner::try_from(hex!("6a34E1f07B57eD968e72895690f3df41b11487eb").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "f111c090a16fd8dfb1dc9f6be75e7c01c11ee0fd01c00a1379cc31c796b0f609"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0154f1e9bb831cd13d04c0cfdc2d4e56cafcb164a1f258ff9c034b407aa81c702a061a403766bbd00883d76be46d7e9e4847f90d7b4beb405a3d0977817c7f934d8a033212a1a78ea156975c05fed52dfb2c5e686baa7963890a7bb2753e52e6630afa05b9374f6fa0a3f86b22e447bb1f8051a458f2652a517975c22a9d59a8ff96076a091f5ccc2025e66263b1ae75ce1dbc6c14bc0f769c50911d4bb2e18d9bcf5a153a0c7f5766cf2f85b382d558514657fea4ef042087e9667d23787f05bc547f1635da0348bb06f67ff7ae65ec4d1632efda68fae55841a86909a67cba88ed389c36c02a0c84b59f11a1f5c29c08d118166fd1345fc6f72f652447dad107756e8610723f2a0455667ca83f8283dd14f8bff5f52b1418e8d1e594168f8456b7116f08369e377a02ca7f57c140275bd0777d1593fe869f53e9050ccafd8e4052804f6f64344870aa05ef6fed064e20f48065e464d124bba78be5d159cf39862fce0f149fb67185405a0e9cc615cd8312767e9b6c47ef9b623d32a324071f88d6aec7b5e6435dcbd22e0a0564385c88a66c4f9344609b64e2a87dd12617f3c36fa1f52c120e6620f277fe0a0cfc67cd838875741fbb9bcd921513e21d4bc13625735e12dab77a2e7910028b6a023a0797ffbbb289480936ca45e29c0db9543edab22baf18da6f9e888b406378ca04fe06425fec14e36f7ae8009836ae9a46fe27171b486db965bded769776c8d2180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0998bac43c14c2c1a253cbe3f757b42a3900cfd25863b8b9c267de2ff76a3adcfa0af0b5b0cd79c1417b428cad0b46823f3d13179998fc5672e05ada339bb95c7e1a04a9c6472e6d3c8248cfe93ed752392a3fc44870440cd06cf45d95aa18cdd9f90a02ff5e12976f175a070d9af587bfebc5975c78c20fffca72c64ec7766137acb50a0ff97d0d436f13ad22face00f6e6275cc26a9adcb5160099ba0a6a4e9f1f2fab2a05cf1002cb87f2f9e050188a4514ec933f765d12afa2799dfa61d4b1711d762bfa0481fe1a1565f5db61fc3062aac7217fdb151ea706221d63a1db7543d5fd43f05a05b049e3200f421ecdefdbcd8cc1635324bf2bacf07d03331fee8b73c547f8321a02d0a34839ba5622c5d92e24b311295709a641913c0f98fa7d013147fb2b4009ea0399aad162a009fc17a35e9abed0a6c0246043d9c971380fac746613736dab1dda0a41593d6bda5801304a4fae153ede4beddd37e510051096c8e0b82424954166ba0bf56674e22bf502e995690adca67da40b4933a8693cd0aa78a2db0d8274071c6a06bdd8c0f7d50f2a1a85c3b79e4685065a3952387e22b822ec83a8ae8ec031bbda0275825b4da0292ee1ea7e87cfcc80e11bfd32465a2ecfd66d3855c5946b2cd5fa0cb7403f57d6d52d6a5e810bb1a937cd8aa9a25f838f0b502f84b7d1be6f98daca0c31b5e27d895869af079e4d527ea36e786b5ab7034f096adbdb60c9d17f26ba080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0306436237194a308ff506037d458cf2afd8a5118b8488aa33b9cdaf2669ae201a0598145fce0c60d36b0fa0f2e851cd4d67e6aae24b39900072671dad8051f9309a08d417f91b3f7c7e2de96c0e1b1662a12d1ea339adb59ded651e05664305530b4a08dd72b6ce2fd407f168b37b2c57735e6c9a8b75c44f07a19b35dbcaa8b46d2e5a048c2af74351d84719bfb39d0b4c824eb93b3197b513da88eb9cd38ef68418fb9a004e915b2a2ff199ea1eb1d252810ae0690af44f25554fa3ba2148499b2137034a014df60fe61f4bfad529b16de61e40d1cd3cea13b28f1ee6ebb5c0ce2ae4b7cada0e7d275a738771131cdb9935f64e93e06d959b52ce25b326afbb66122fcb95e20a093243753d545ba83ad2129763695cea1e861dd316828ca44009990d4c3841ef1a048a6e16fa84be701724b573751489dceac2a4103bc77766bd7f80e89faf7d9b5a01932809b29b5e646640f0b103e7b56ece0ba2bd3c2537c3607d08723e8f89c92a0e19117082122864af46a9f6b109efd0f220956e7925087c33db8c0b2ca443749a02c006c1809f7bafabed11a792f124c9e2bbf83e4ed42cf7c3a2378311fa150c4a043be036f0ae3546cf0ff89110d7c6ca7c749dc3ffe7acd52fcfd25a5c4edf1e0a0aeb7a02b8ee26f68a74dd3ca60c523d6fab232121d733285935017c72c60b195a030aa054fbd82c64134f097eca6dfc3f7def3e4e4b9ff14dce7d8c21cd6a9eb3880").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0756469afd770437fbac3e2da9b82c8841b819437ba2f34f368739ea25b7869c7a0670ea5f45563a615c824645b7d7e9c9e166c45d19265ec3fe3a3760ac2577905a006accaff23de46b6487cd6fbe02134117d8a86e3313a00767ae08a6b7f71adc9a001c7ee9ac038c3d9cb416487614fa9432e6526f3289ee64cebaf1b69b5246c1da08d730b76897e409a388f7d9c9598342adde6f164dfe1aa45f6fa54536a404438a055afdb4b988b8efa2fc9b30edd4272239455b332b23677966904ce98d894449ba09c28aeddd1c92cdb97d28cc065c4062ad130bf99e4658b9cc801d04620e8c157a0ecbc855a2fbc6f3b980daebdb5e8ce26da418672d6b0626ad2dfe5f210d3dc7aa049a7a2d7e70457787c466f9373e9a45cc9936f9c7178179281e71f79ffd60feca021f176706d2ed2ad80bf38d76e70e7d8eac721befb330c9454cfa086d6c667d3a0160821c6a95146e4ff217459bb7d94263ca5b7d0e0cf32c0a7ba5b53c07e7b25a0e27b7190c2d540122c7ac6f53f5af5d4c3b67ad56901619e1888bc163bd9c58da080f38bedbab0ad8b40dbb02745e2d06bb399d450fab3ad19e19838b769276b3ea03e103adba5bc62e97c779563454a1d40cea0ee2f9da4b770e025013c82c878c9a0717a9c119d54e0089300d7e85b129397451e16445bc4dc864e45917339fc8312a0c75293b894fab23a1b0c95127d0dd79e55bccdb8f03b21e81b822cf6b8fa562180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a00c3cf3dab87e6deefe49eb8fe0ea67288a72bfc0765069f23a330e66a72ddd0aa0b973adbb846243f3754860a5c3d970fa273867dac9a2ee1b6b4bbda70521b512a03bc66e06c39dbc084c89aac1b7a31a2e1454d1a5b5fac313fdef6c59e1719178a0f2ad580cc1faf073f412dcb742ec1b008424acdc40211b948c573360bd09e43aa0c608bf61bea8927a39a1dc551482cd15f54952c925a4ee9ecf8299a7db345523a07d2423a6f3f44b0e48ee46a03df4e52adc5335422340b764239394e9ea9fc7a2a012edba4d9ed0cf9e2efac7ec9337e817905a8ddc119044935df6ae34fc20ec38a02e7e258bb05d023cac0486b05d2d0353b5f385a1a8abec3a2bfc0e8e0a3436d4a0a8ce06af1101a64d28fe2f186901b45f4d748816009a58d21f0596fdfaa2486ea0a89d593d80a4db3bc0d47a91578c99840cd8a79a11043c7ddb05f3299bfc69f1a0fc1746a4ee20de1f16a89fa366c391c9931eef5b3d09b5cb39685805e27fb4d3a07b3ba10e6816de80a550ede8ac36cf46f62dd5656bdab4ce5c820cc15d65f6f4a065ce34c09f0d58614c1b2201d97fea9936a55bfc281d0289834403b64372d8efa0b54bcd8b067d6087b0620d79322751dd0b297c269b875d871af5eac3a4997594a026dab4fc126b924790fcf74e7065a12bfd67d6b11b600327f3bdf5e33861039aa0dfaf0a9c9fb7735fb78c99f3e846f2889e8e395cd0f5d3b074e0633dc89431d780").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90171a053137fd9f832ec9a4f8f084c4a94789a6108586dc4b7a81d7e45464520406c2480a011ab943b4f79a6fa5c14fd19ed95c33ed1b5b6ab1d79b7b97723bf1d91f2a84ba05cb67fad37abf959c3a74dfe692ca3e4a15281cec939f1cf275a2bb78e15744d80a05034784476360d10b7715d880a84211337c92867a5943e5c4dd2c96b492ff2dca02b0fb32dc101a5dd42c7dde7b61f2d28c1e7334f296eb8b6394b48ecd53ca42ea0baff4073331f05a6913d91b50662fa0e6e97d30a8b312ae3424236ff9c8a66f2a095eb06d7ec5d02e63123c34044808369ed3231884687a4d2ce45ed56cda1d83580a08f8c5c7f2f749d90b0902e2c3da3fcfb8711504ba1e84b0eb2702c24f7b87b00a006df4234f3046b11a21232e729192458899441db8cf2ea14bf211be894b29cb5a0c362f44f620e38d244f3984590b2eb8bcc64f2058ec4d7c5217c97a9bd2cccb98080a0db957775f706697b313e0891476a0884455bb5f70b6588d049b1a0adbbc5fc8c80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f87180808080808080808080a04dd86c4a4cd1b4eb14c871fc9d9955551b537567aa6f7a3c09da770d337b4133a04cd68ab5366c84a6b03eae23d2fd0613b4d9ad923fd534b705fadd3f721a1a78a0b505b13863a48a79e0fd1f09a18a2515bde23ebf9186d5b96116461108671e4480808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d339622fd0302d2b4edbb8e3cbdf2963042f5c5934c78f17b88a2468235b846f8440164a0255a828af5d7f876d57a5b418502f28361eb71fa51db4f6076244e6870657e10a0acbdb9b52a321f2759dc9c3a37fbd1445837b0bab1a74ce640f34b121f4f9325").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0f171bf2b9dcbb69b4e58b644acc664cafede5633e856c12576f9187fdedf6b14a0adad05bf1d009f2ff38112ccd218563a9fda3aecde30d79876b8c692d6609e67a09c9784f2d923c86a996dbb19374569587312111e657d0ace78bc0932fe862623a058a9c646426f4369ceba24b46998e7692e1f47bf1e95c444ba76ea03b7688888a0a36f509cb02d7011bd1bc4ab3006dffacebb667fd61ef4d267a82816c7517a33a0320c5bdc1e54dff5dd54f42bde8cf29337aa59a86d61fea5850ec16a9fefdcc4a0575dc068430b69da7ea9d7a3e89a8a3fbfef8a8a630c8f311bc12f1334b3469ea06675ba7053a8aad5c4ba429e36a9c5fb133fd3728b7c1c628840b4f1426062bba086137fcded667bdd7190c904c421a75aec3053bbf392a08c515f1a5cad0143cca028b5a1e02e5a43d341c735db20faa93c6b3607e6dc2348f209e356f26a2dd145a0e7b69fe574c524afc97101c8b09551cc05b921a84c820006a7fe43e1e8d8a6c0a0cb66748d597fcd182d2399c7f2097ec9009e0520fd8b57100984f5f9a131d801a012a69a51034205a5b5090c193e571f89d5e719c0a8cb3b64e97159c90e161dfda071c14c32c385b403fe0330b4fa4405581163b8a6e0a32d51111db1c3df6cfcdba0f1fe7833657f747e262096b16f7dda413b98b9cc443055d5a3a54b1bc9c19b9da09c056be83f26c5384827b944d077973d153265e838bfcc7af41acdc8a3d7b82580").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8b1a0859d1dffbaac58a0a135570eb05206775ab79d6cd5fccd91627902ddc8435f4880a06586e4117be78aa749b3538cb4e541bc10e4d76dc197e0e3dbc1a232d054b38b80a09177c8c17bc0b5da7f64529e63daad72d95dc71734f717721c4c591f7c2179e680a0ef1beb730599df2f1f8211f8b83301cbc55a6bfe0ba36ee610ddb9c6978238118080808080808080a0f3e3866a3598d8e07618e866b2f25e94f1eded800e4903b67dee6d338a2a336280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a020357a22f45b14107b8bf062a61ef0c433b450fde05bf4e844e23dc0cf979dc9a1a081df23bbe0f19788b0659d468487cba22df7eda034ba50bd8fce225d5936e8fa").to_vec()).unwrap(),
        ];
        let message_id = 9u128;
        let value = EthereumProofValue::try_from(hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b00000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001").to_vec()).unwrap();

        let proof = EthereumProof::<Test, AcurastAccountId> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}
