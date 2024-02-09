#![cfg(test)]

use frame_support::{assert_err, assert_ok, BoundedVec};
use hex_literal::hex;
use sp_runtime::{bounded_vec, AccountId32};

use acurast_common::{Environment, MultiOrigin};

use crate::{
    mock::*, utils::validate_and_extract_attestation, AllowedSourcesUpdate, AttestationChain,
    CertificateRevocationListUpdate, Error, ListUpdateOperation, SerialNumber,
};

#[test]
fn test_job_registration() {
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

        let registration = job_registration(None, false);
        let register_call = Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        );
        assert_ok!(register_call);

        assert_eq!(
            Some(registration.clone()),
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_ok!(Acurast::deregister(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Acurast::job_id_sequence()
        ));

        assert_eq!(
            None,
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1)
                )),
                RuntimeEvent::Acurast(crate::Event::JobRegistrationRemoved((
                    MultiOrigin::Acurast(alice_account_id()),
                    initial_job_id + 1
                )))
            ]
        );
    });
}

#[test]
fn test_job_registration_failure_1() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = invalid_job_registration_1();

        let initial_job_id = Acurast::job_id_sequence();

        assert_err!(
            Acurast::register(
                RuntimeOrigin::signed(alice_account_id()).into(),
                registration.clone()
            ),
            Error::<Test>::InvalidScriptValue
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_job_registration_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

        let registration = invalid_job_registration_2();
        assert_err!(
            Acurast::register(
                RuntimeOrigin::signed(alice_account_id()).into(),
                registration.clone()
            ),
            Error::<Test>::InvalidScriptValue
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_job_registration_failure_3() {
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

        let registration = job_registration(Some(bounded_vec![]), false);

        assert_eq!(
            None,
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_err!(
            Acurast::register(
                RuntimeOrigin::signed(alice_account_id()).into(),
                registration.clone()
            ),
            Error::<Test>::TooFewAllowedSources
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_update_allowed_sources() {
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

        let registration_1 = job_registration(None, false);
        let registration_2 = job_registration(
            Some(bounded_vec![alice_account_id(), bob_account_id()]),
            false,
        );
        let updates_1 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                item: alice_account_id(),
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                item: bob_account_id(),
            },
        ];
        let updates_2 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                item: alice_account_id(),
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                item: bob_account_id(),
            },
        ];
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration_1.clone(),
        ));

        assert_ok!(Acurast::update_allowed_sources(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Acurast::job_id_sequence(),
            updates_1.clone().try_into().unwrap()
        ));

        assert_eq!(
            Some(registration_2.clone()),
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_ok!(Acurast::update_allowed_sources(
            RuntimeOrigin::signed(alice_account_id()).into(),
            Acurast::job_id_sequence(),
            updates_2.clone().try_into().unwrap()
        ));

        assert_eq!(
            Some(registration_1.clone()),
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::Acurast(crate::Event::JobRegistrationStored(
                    registration_1.clone(),
                    (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1)
                )),
                RuntimeEvent::Acurast(crate::Event::AllowedSourcesUpdated(
                    (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1),
                    registration_1,
                    updates_1.try_into().unwrap()
                )),
                RuntimeEvent::Acurast(crate::Event::AllowedSourcesUpdated(
                    (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1),
                    registration_2,
                    updates_2.try_into().unwrap()
                ))
            ]
        );
    });
}

#[test]
fn test_update_allowed_sources_failure() {
    let registration = job_registration(
        Some(bounded_vec![
            alice_account_id(),
            bob_account_id(),
            charlie_account_id(),
            dave_account_id(),
        ]),
        false,
    );
    let updates = vec![AllowedSourcesUpdate {
        operation: ListUpdateOperation::Add,
        item: eve_account_id(),
    }];
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        ));

        assert_err!(
            Acurast::update_allowed_sources(
                RuntimeOrigin::signed(alice_account_id()).into(),
                initial_job_id + 1,
                updates.clone().try_into().unwrap()
            ),
            Error::<Test>::TooManyAllowedSources
        );

        assert_eq!(
            Some(registration.clone()),
            Acurast::stored_job_registration(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );

        assert_eq!(
            events(),
            [RuntimeEvent::Acurast(crate::Event::JobRegistrationStored(
                registration.clone(),
                (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1)
            )),]
        );
    });
}

#[test]
fn test_submit_attestation() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();
        let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            RuntimeOrigin::signed(processor_account_id()).into(),
            chain.clone()
        ));

        let attestation =
            validate_and_extract_attestation::<Test>(&processor_account_id(), &chain).unwrap();

        assert_eq!(
            Some(attestation.clone()),
            Acurast::stored_attestation(processor_account_id())
        );

        assert_eq!(
            events(),
            [RuntimeEvent::Acurast(crate::Event::AttestationStored(
                attestation,
                processor_account_id()
            ))]
        );
    });
}

#[test]
fn test_submit_attestation_parse_issuer_name() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = AttestationChain {
            certificate_chain: vec![
                hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                hex!("3082039930820181a0030201020210060d896bdc60a576a5947be0895f5989300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3230303931313138303232315a170d3330303930393138303232315a303f31123010060355040c0c095374726f6e67426f78312930270603550405132066336466313937623134316339333437633764616630333735656330663934393076301006072a8648ce3d020106052b81040022036200047246606805047a2007191896564ddc2931e0de34aa60fbd8b84ec6b544ef722a843b8fee768f2a611d7dc1785389736ff17314f67f7ec1e6484fc34b01e8493dc0c50c2af60d31c7f9b5a7f6963d5abc45ca36ba14a0b272cc6c6cf6f15ea363a3633061301d0603551d0e041604146ee611df7046d5bb346d8d2d8e06371f5271ab4d301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201003828d80663e5d9b0df41803e774fffe44e5c9ab0de9b067c9a8d569cdcef69bd880b6abbac853cb3f317dbb62a37584cb9e3ce108962a15db5531858edf57787d1dac77b675804aae8631586b08d0d758dedac0d6e331f34a1686f12e171bc040757ffd41ddd1840256e5e30d04cff0576bf27f1e60f05de5fc50a25fa9598d148db3e0a5fdcfcf92a4241129e5a8a3a0bbab94034bd75775a57f1fa2d0822e1874fd002b9856c0c53390ac221123edb25f40f50f06b3796ad92d20ff84f90cd7fb849a5784e395c4517cb630b762c6d12223acb2f80ff040c8b332856156e5d55196620cb1cfb4d4e3c50b396570f20f2c6925ad17e567246edb6a46e12077a88c59586131ab5f57b7bc8d79adef6049cd26563807f6854d977acea82068c56a1e8020570ca2a29f0730a5ac46e48844bea8b497b818c33642378d7714efbe4e23f43e80ad7dc83b7347ec25d6e3fc8af1a7462c5373deabfe2e00c13ca28a3bd65ba41070a0304c94a7883e4b4b957f8c1e6697aa17dba7c5850b5e44f02c2d741f0f5d5a76487bb2eaae58bdd957518d487fbcf6b6c6b4ed704a5f7c7079d4bb7d1d5d2c470403caf8ddd2eeccfc61a335081a5be1e5028fa01247a1ebfd4c56f461276a32ead8a3d87fc32a1b410283141ff4f0c10644e765f14c46ce3f14db5a52ed7474e8e3ee5b447eb4b4ac78e0604ca4accf5698e46591493d36ca1").to_vec().try_into().unwrap(),
                hex!("3082020030820185a0030201020210569a2401ba9238309bdac006c2ac251d300a06082a8648ce3d040302303f31123010060355040c0c095374726f6e67426f7831293027060355040513206633646631393762313431633933343763376461663033373565633066393439301e170d3230303931313138303234345a170d3330303930393138303234345a303f31123010060355040c0c095374726f6e67426f78312930270603550405132030363834326638346263626164626431393634303562666436613633343965623059301306072a8648ce3d020106082a8648ce3d0301070342000439989af279e4f44064007117458bc6a32b4d560564f05ba83a26440004861393653371f2bc5a30f5d53e968b62f9cd504e695ff352e4d06c4a523c88e470fe9ba3633061301d0603551d0e0416041451246eb031c712b66ec62ccc5890b4de1bbe2349301f0603551d230418301680146ee611df7046d5bb346d8d2d8e06371f5271ab4d300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300a06082a8648ce3d0403020369003066023100eedf5aa529ce13a5a00d35919a46ebcecc19ae638d8907d5e8a2a639436407b4b3f5991de8dc254f9009baad859e8d87023100ba5dce3152f65e35caa80a111d7a42dd7aa0d6c04601f781abed624f45ba8413631a91e3f6b97d990ab03bab392a6f63").to_vec().try_into().unwrap(),
                hex!("308202bb30820260a003020102020101300a06082a8648ce3d040302303f31123010060355040c0c095374726f6e67426f7831293027060355040513203036383432663834626362616462643139363430356266643661363334396562301e170d3730303130313030303030305a170d3438303130313030303030305a301f311d301b06035504031314416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004f47e5aae05375a331414fc79b8e783a576e9b4da257e0e78f89e922fd7a1a5d817c0eae09190b481f3f631a29a9025a7f19768615ce9b46b4a9fe2614aa981e5a382016b30820167300e0603551d0f0101ff04040302030830820153060a2b06010401d679020111048201433082013f0201640a01020201640a010204208aac831130e74725119d0a21a725e72c6e6978b81c3da96242cbf1e97657f41004003066bf853d080206018722de6983bf85455604543052312c302a0425636f6d2e616375726173742e61747465737465642e6578656375746f722e746573746e657402010f31220420a1dbdc7fde8ccc97b4906d8fce99b4d950ecf62389bbe4b393988ce9061b4e933081a4a1083106020102020106a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a04209ac4174153d45e4545b0f49e22fe63273999b6ac1cb6949c3a9f03ec8807eee90101ff0a0100042060d7ff2744c1fb4a4726c4050b2022a9f69ca87f5fe7d271096d7deb5507994fbf854105020301fbd0bf854205020303163ebf854e0602040134b03dbf854f0602040134b03d300a06082a8648ce3d0403020349003046022100ec7dd8f4eee1d2bed198ee19c7b7f1a4c8b991ee02f1643df96bf7c4a732d3fb022100a2ca91e2791d5fca189b200221cb9da0e07c3cb3aac987b020b65a027a10ec43").to_vec().try_into().unwrap(),
            ]
            .try_into()
            .unwrap(),
        };

        let account_id: AccountId32 = hex!("6c0711dba61c0c7f4a124d85384471b98ce9e28e2a6f7e567c4408f3ab77de09").into();
        assert_ok!(Acurast::submit_attestation(
            RuntimeOrigin::signed(account_id.clone()).into(),
            chain.clone()
        ));

        assert_ok!(validate_and_extract_attestation::<Test>(&account_id, &chain));
    });
}

#[test]
fn test_submit_attestation_failure_1() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = invalid_attestation_chain_1();

        assert_err!(
            Acurast::submit_attestation(
                RuntimeOrigin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::CertificateChainTooShort
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        let chain = invalid_attestation_chain_2();

        assert_err!(
            Acurast::submit_attestation(
                RuntimeOrigin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::CertificateChainValidationFailed
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        let chain = invalid_attestation_chain_3();

        assert_err!(
            Acurast::submit_attestation(
                RuntimeOrigin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::CertificateChainValidationFailed
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_submit_attestation_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();

        let _ = Timestamp::set(RuntimeOrigin::none(), 1657363914000);
        assert_err!(
            Acurast::submit_attestation(
                RuntimeOrigin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::AttestationCertificateNotValid
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_submit_attestation_failure_3() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();

        let _ = Timestamp::set(RuntimeOrigin::none(), 1842739199001);
        assert_err!(
            Acurast::submit_attestation(
                RuntimeOrigin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::AttestationCertificateNotValid
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_update_revocation_list() {
    ExtBuilder::default().build().execute_with(|| {
        let updates_1 = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            item: cert_serial_number(),
        }];
        assert_ok!(Acurast::update_certificate_revocation_list(
            RuntimeOrigin::signed(alice_account_id()).into(),
            updates_1.clone().try_into().unwrap(),
        ));
        assert_eq!(
            Some(()),
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        let updates_2 = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Remove,
            item: cert_serial_number(),
        }];
        assert_ok!(Acurast::update_certificate_revocation_list(
            RuntimeOrigin::signed(alice_account_id()).into(),
            updates_2.clone().try_into().unwrap(),
        ));
        assert_eq!(
            None,
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        assert_err!(
            Acurast::update_certificate_revocation_list(
                RuntimeOrigin::signed(bob_account_id()).into(),
                updates_1.clone().try_into().unwrap(),
            ),
            Error::<Test>::CertificateRevocationListUpdateNotAllowed
        );
        assert_eq!(
            None,
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::Acurast(crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates_1.try_into().unwrap()
                )),
                RuntimeEvent::Acurast(crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates_2.try_into().unwrap()
                ))
            ]
        );
    });
}

#[test]
fn test_update_revocation_list_submit_attestation() {
    ExtBuilder::default().build().execute_with(|| {
        let updates = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            item: cert_serial_number(),
        }];
        assert_ok!(Acurast::update_certificate_revocation_list(
            RuntimeOrigin::signed(alice_account_id()).into(),
            updates.clone().try_into().unwrap(),
        ));

        let chain = attestation_chain();
        let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915001);
        assert_err!(
            Acurast::submit_attestation(
                RuntimeOrigin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::RevokedCertificate
        );

        assert_eq!(
            events(),
            [RuntimeEvent::Acurast(
                crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates.try_into().unwrap()
                )
            ),]
        );
    });
}

#[test]
fn test_update_revocation_list_assign_job() {
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();
        let updates = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            item: cert_serial_number(),
        }];
        let chain = attestation_chain();
        let registration = job_registration(None, true);
        let _ = Timestamp::set(RuntimeOrigin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            RuntimeOrigin::signed(processor_account_id()).into(),
            chain.clone()
        ));
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(bob_account_id()).into(),
            registration.clone()
        ));
        assert_ok!(Acurast::update_certificate_revocation_list(
            RuntimeOrigin::signed(alice_account_id()).into(),
            updates.clone().try_into().unwrap(),
        ));

        let attestation =
            validate_and_extract_attestation::<Test>(&processor_account_id(), &chain).unwrap();

        assert_eq!(
            events(),
            [
                RuntimeEvent::Acurast(crate::Event::AttestationStored(
                    attestation,
                    processor_account_id()
                )),
                RuntimeEvent::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    (MultiOrigin::Acurast(bob_account_id()), initial_job_id + 1)
                )),
                RuntimeEvent::Acurast(crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates.try_into().unwrap()
                )),
            ]
        );
    });
}

#[test]
fn test_set_environment() {
    let registration = job_registration(
        Some(bounded_vec![
            alice_account_id(),
            bob_account_id(),
            charlie_account_id(),
            dave_account_id(),
        ]),
        false,
    );
    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        let job_id = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

        let env = Environment {
            public_key: BoundedVec::truncate_from(
                hex!("000000000000000000000000000000000000000000000000000000000000000000").into(),
            ),
            variables: bounded_vec![(
                BoundedVec::truncate_from(hex!("AAAA").into()),
                BoundedVec::truncate_from(hex!("BBBB").into())
            )],
        };
        assert_ok!(Acurast::set_environment(
            RuntimeOrigin::signed(alice_account_id()).into(),
            initial_job_id + 1,
            bob_account_id(),
            env
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    job_id.clone()
                )),
                RuntimeEvent::Acurast(crate::Event::ExecutionEnvironmentUpdated(
                    job_id.clone(),
                    bob_account_id()
                )),
            ]
        );
    });
}
