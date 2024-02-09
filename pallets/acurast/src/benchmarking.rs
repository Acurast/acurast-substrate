use frame_benchmarking::{account, benchmarks, whitelist_account};
use frame_support::{
    assert_ok,
    sp_runtime::traits::{AccountIdConversion, Get},
    traits::OriginTrait,
    BoundedVec,
};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Config;
use acurast_common::{AttestationChain, JobRegistration, Script};

use crate::utils::validate_and_extract_attestation;
use crate::Pallet as Acurast;

use super::*;

pub const SEED: u32 = 1337;
pub const ROOT_CERT: [u8; 1380] = hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4");
pub const INT_CERT_1: [u8; 987] = hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42");
pub const INT_CERT_2: [u8; 564] = hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb");
pub const LEAF_CERT: [u8; 672] = hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9");
const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

pub trait BenchmarkHelper<T: Config> {
    fn registration_extra(instant_match: bool) -> T::RegistrationExtra;
    fn funded_account(index: u32) -> T::AccountId;
}

pub fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn job_registration<T: Config>(extra: T::RegistrationExtra) -> JobRegistrationFor<T> {
    return JobRegistration {
        script: script(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration: 500,
            start_time: 1_671_800_400_000, // 23.12.2022 13:00
            end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
            interval: 1_800_000,           // 30min
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        required_modules: JobModules::default(),
        extra,
    };
}

pub fn script() -> Script {
    SCRIPT_BYTES.to_vec().try_into().unwrap()
}

pub fn processor_account_id<T: Config>() -> T::AccountId
where
    T::AccountId: From<[u8; 32]>,
{
    hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into()
}

pub fn attestation_chain() -> AttestationChain {
    AttestationChain {
        certificate_chain: vec![
            ROOT_CERT.to_vec().try_into().unwrap(),
            INT_CERT_1.to_vec().try_into().unwrap(),
            INT_CERT_2.to_vec().try_into().unwrap(),
            LEAF_CERT.to_vec().try_into().unwrap(),
        ]
        .try_into()
        .unwrap(),
    }
}

fn register_job<T: Config>(
    submit: bool,
    instant_match: bool,
) -> (T::AccountId, JobRegistrationFor<T>) {
    let caller: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0);
    whitelist_account!(caller);

    let job = job_registration::<T>(<T as Config>::BenchmarkHelper::registration_extra(
        instant_match,
    ));

    if submit {
        let register_call =
            Acurast::<T>::register(RawOrigin::Signed(caller.clone()).into(), job.clone());
        assert_ok!(register_call);
    }

    (caller, job)
}

benchmarks! {
    where_clause {  where
        T: pallet_timestamp::Config,
        <T as frame_system::Config>::AccountId: From<[u8; 32]>,
        <T as pallet_timestamp::Config>::Moment: From<u64>,
    }

    register {
        let (caller, job) = register_job::<T>(false, true);
    }: _(RawOrigin::Signed(caller.clone()), job.clone())
    verify {
        assert_last_event::<T>(Event::<T>::JobRegistrationStored(
            job, (MultiOrigin::Acurast(caller), 1)
        ).into());
    }

    deregister {
        let (caller, job) = register_job::<T>(true, false);
        let local_job_id = 1;
    }: _(RawOrigin::Signed(caller.clone()), local_job_id.clone())
    verify {
        assert_last_event::<T>(Event::<T>::JobRegistrationRemoved(
            (MultiOrigin::Acurast(caller), local_job_id)
        ).into());
    }

    update_allowed_sources {
        let x in 1 .. T::MaxAllowedSources::get();
        let (caller, job) = register_job::<T>(true, false);
        let mut updates: Vec<AllowedSourcesUpdate<T::AccountId>> = vec![];
        for i in 0..x {
            (&mut updates).push(AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                item: account("processor", i, SEED),
            })
        }
        let local_job_id = 1;
        let updates: BoundedVec<AllowedSourcesUpdate<T::AccountId>, <T as Config>::MaxAllowedSources> = updates.try_into().unwrap();
    }: _(RawOrigin::Signed(caller.clone()), local_job_id, updates.clone())
    verify {
        assert_last_event::<T>(Event::AllowedSourcesUpdated(
            (MultiOrigin::Acurast(caller), 1), job, updates
        ).into());
    }

    submit_attestation {
        let processor_account: T::AccountId = processor_account_id::<T>();
        let attestation_chain = attestation_chain();
        let timestamp_call = pallet_timestamp::Pallet::<T>::set(T::RuntimeOrigin::none(), 1657363915001u64.into());
        assert_ok!(timestamp_call);

    }: _(RawOrigin::Signed(processor_account.clone()), attestation_chain.clone())
    verify {
        let attestation = validate_and_extract_attestation::<T>(&processor_account, &attestation_chain).unwrap();
        assert_last_event::<T>(Event::AttestationStored(
            attestation,
            processor_account,
        ).into());
    }

    update_certificate_revocation_list {
        let updates =  vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            item: hex!("15905857467176635834").to_vec().try_into().unwrap()
        }];

        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

    }: _(RawOrigin::Signed(pallet_account.clone()), updates.clone().try_into().unwrap())
    verify {
        assert_last_event::<T>(Event::CertificateRecovationListUpdated(
            pallet_account,
            updates.try_into().unwrap()
        ).into());
    }

    set_environment {
        let x in 1 .. T::MaxEnvVars::get();
        let (caller, job) = register_job::<T>(true, false);
        let mut vars: Vec<(BoundedVec<u8, T::EnvKeyMaxSize>, BoundedVec<u8, T::EnvValueMaxSize>)> = vec![];
        for i in 0..x {
            (&mut vars).push((BoundedVec::truncate_from(vec![
                105, 112, 102, 115, 58, 47, 47, 8]), BoundedVec::truncate_from(vec![
                105, 112, 102, 115, 58, 47, 47, 8])))
        }
        let env: EnvironmentFor<T> = Environment{
            public_key: BoundedVec::truncate_from(vec![105, 112, 102, 115, 58, 47, 47, 8]),
            variables: BoundedVec::try_from(vars).unwrap(),
        };
        let local_job_id = 1;
    }: _(RawOrigin::Signed(caller.clone()), local_job_id, account("processor", 0, SEED), env.clone())
    verify {
        assert_last_event::<T>(Event::ExecutionEnvironmentUpdated(
            (MultiOrigin::Acurast(caller), 1), account("processor", 0, SEED)
        ).into());
    }

    impl_benchmark_test_suite!(Acurast, mock::ExtBuilder::default().build(), mock::Test);
}
