use frame_support::traits::OnRuntimeUpgrade;

use acurast_runtime_common::{types::AccountId, weight::RocksDbWeight};
use hex_literal::hex;
use pallet_acurast_token_conversion::DeniedSource;
use sp_runtime::Weight;
use sp_std::vec;

use crate::Runtime;

pub struct RuntimeUpgradeHandler;
impl OnRuntimeUpgrade for RuntimeUpgradeHandler {
	fn on_runtime_upgrade() -> sp_runtime::Weight {
		add_denied_sources()
	}
}

fn add_denied_sources() -> Weight {
	let to_deny = vec![
		// 9
		AccountId::from(hex!("aa0ee6c2705e3dee1f1cb4f78977be38bf75cc80d43f3cfddaf743964b157a23")),
		// 10
		AccountId::from(hex!("5a19d172ea95ba355969cdf3f253023a95133a2e482c7c71554c1b88a394633d")),
		// 12
		AccountId::from(hex!("5ec049ad27ec043d7cc39125c8f59e9682e05a77bf3315ca53148bea5dbae754")),
		AccountId::from(hex!("4e078a5769802f991b55197cfc4bfb44e574835d51d2ca8bea28165dc2c8bf71")),
		AccountId::from(hex!("a05bb27c44f826cd2d7ef2213e9646014876bbf25ad333a884c3016af55e8613")),
		// 13
		AccountId::from(hex!("4279d5ed16cc90dacd92d24396923db11b84dd832961b1ffb8df675e2a42207f")),
		AccountId::from(hex!("caebf78511a9799a7cd11bbe83dc91ed9c099ee4ed77712d691b09ea65760d4e")),
		AccountId::from(hex!("60649db1a588b570192baaf5bd1b8f1982351fb6c88ce0d68320da37fd678a5f")),
		AccountId::from(hex!("20b51fbed16184e5ea85ff60f33aa4dc177b277ba25c12cd347d36674e227309")),
		AccountId::from(hex!("06d5f25e4ea05ce513fd1cc8d0bf0a8647095fd9f88aeeddfc04168ad66ac73a")),
		AccountId::from(hex!("28a952b49de7519e3841398f46667b675a46258689038c071e2fa25b310c815a")),
		AccountId::from(hex!("96d2b71b5dd4a2d491bbc1c2204c0d8fd8a37df946c6442655bf73480e133300")),
		AccountId::from(hex!("08a5c013ec613cf2d4784452ae3035e59ad8d129bc651d1b910e2517914e2124")),
		AccountId::from(hex!("a6f463f5ff46cbf9875b056ef3b267a2d311940822b12b843b4f8f0e6202042d")),
		// 15
		AccountId::from(hex!("a471c55caca4be7b4e60c6e94b20f9028883f8c64287d4454130c657383c3442")),
		// 16
		AccountId::from(hex!("fc24d6625df5e163ef1c01f1d5298126771aa95ea42f2626dc071b1eddd1a51a")),
		// 17
		AccountId::from(hex!("fe20a56fa743b92dd81f09e739edc268f67195988355b7bef0bf97752687585d")),
		// 18
		AccountId::from(hex!("a65cc5639b904c8221fdc797bab8e8b50deaba5253611e2abd301060000e0610")),
		// 19
		AccountId::from(hex!("4831ba3d0a6cf5b083b7854cbbd86edc8acf98b82272c5eea3c38684a1834559")),
		// 20
		AccountId::from(hex!("906d22eed12e4f8bb772c133ef253e07073691641ccb3b464bb97ea958ddbe70")),
		// 21
		AccountId::from(hex!("303e1cc9712a935918a89cbce0e38f4bbe9fc9166e4dde31c24a50f3b7e86500")),
		// 22
		AccountId::from(hex!("1cd345ce8592fee5adc4718a75780ad417f4d23f2214361d7354396bbc9a7d3c")),
		// 23
		AccountId::from(hex!("b210fd61096afd0455835d2a4da7b1d0d0f3fe8b450e8722eeb9c5aae25d6e2b")),
		// 24
		AccountId::from(hex!("ee2cc7f37829b104aa5950125f097215babd124b946c4b249c953c7ce24faa2a")),
		// 25
		AccountId::from(hex!("e0a18cc269e9165fe0ad35d6c1aa4e64c42c4f1eb33829d9003f9a2c7e720edd")),
		// 26
		AccountId::from(hex!("c8eabb6a8ea439f3fe5715bf063a41d6763b70350da406ed4eb1cbd7f77d8a10")),
		// 27
		AccountId::from(hex!("e07e5e85fdc02db53069c3ac167bd4cbcf732638cf1b1ad7ae2f930924d41a22")),
		// 28
		AccountId::from(hex!("dc83908d9c4aa5f678cc69dad093753fda42f4c0d4c4311b88e1992f5786d554")),
		// 29
		AccountId::from(hex!("1cfdb882a8b0053a468b6e88ec67ee364816dffaf79be86beb4e9d4ad37be935")),
		// 30
		AccountId::from(hex!("e857aa4184f665ac825b432c3e3dc7762221db698de95fb663fb1ab24ecb0d3c")),
		// 31
		AccountId::from(hex!("1c01afd55b84f0501059fd6944a52099acd77e79e2092c974ef35d1515b9252e")),
		// 32
		AccountId::from(hex!("d857c7691953383c71e23a936ac4dcd0c24c45bfe9a44025bb2579b5ecc0a52e")),
		// 41
		AccountId::from(hex!("c8b2304c752c52ca3ca2b10c488073c219b117754b067e6d4ffb178a70d0a938")),
		// 42
		AccountId::from(hex!("1c7dcd343bcd46bae6886313df16497aaea9d198b08283e17df30d6e57177e11")),
		// 43
		AccountId::from(hex!("5461b518602f4e85acee70a0634cf9cd6ce2774d3a157add9d4b16e53ba09a5c")),
		// 44
		AccountId::from(hex!("062d6528162fdee89707b54a12f99766d8b5461fbb7642f32639096bd2f67c16")),
		// 45
		AccountId::from(hex!("124fd442af9a995ce3c1bbbcd798edcd9615b7a1c227966a255c6222ce20e640")),
		// 46
		AccountId::from(hex!("ec055d2fdde37dcc54a6393ee77ae5f40f160e5933dd9fbbb7eccff8e88e800d")),
		// 47
		AccountId::from(hex!("e2c153ad32bc8f666568db575d5d123c61f7540314b925b2ab8ea53b61fe5d52")),
		// 48
		AccountId::from(hex!("ba5bb4cf137a77af01ddb7615832c5f485a5962e36abee6941116dabcce4f42e")),
		// 49
		AccountId::from(hex!("f18cd3305cf22bec2a65b18f37c83752c581a35d2af14525f22ad0bc6e87969d")),
		// 50
		AccountId::from(hex!("16e089b31a8c59dc7bdc597439db14717705fab1980f61621435e89588c4c86f")),
		// 54
		AccountId::from(hex!("70930f8d79927bba923fcc78f2ea8b5b2e22909556579941d16a27a737e75752")),
		AccountId::from(hex!("4a7587442bf8016de1580709867736465c227fcf3ceb9c07b54066d731a94846")),
		// 55
		AccountId::from(hex!("801d1b5699cba6fbd07cfd8af9edddf83452ed5cd0e376f268057371d7b93811")),
		// 56
		AccountId::from(hex!("e045937f9540dd73134cab93b8f0827409fa3d7db157e97ff4f46022bbf33d7e")),
		// 57
		AccountId::from(hex!("58dac7c3f4cbcf8d629c59d8de628d97d63097429c3e35aba42630f7ff93af66")),
		// 58
		AccountId::from(hex!("c04c82374c95c35a714fdedc495682d054d4e5e9c1eb0827fc7cdfa130176165")),
		// 59
		AccountId::from(hex!("2e8fd7505354415111da69e27bb38852bf2b25fcf7f4c2709b9720df292ad75b")),
		// 60
		AccountId::from(hex!("7a75f914f45636381ac0b8a1d436ebe7e61771cad6a227661359cb63e52ebd39")),
		// 61
		AccountId::from(hex!("0cdb9c62919b5ba33e9146f618a5e1c425c2f13ad323d0cbbad0bbee5e375c3a")),
		// 62
		AccountId::from(hex!("ba2351b343dbcb23edf30413e7f2755a8e154b7847f390749c7fca74d5b16a74")),
		// 63
		AccountId::from(hex!("a216290f665a45de16cf144aec5adf82f5e97166d09c7f2fd04feddef413b22a")),
		// 64
		AccountId::from(hex!("c6c28b08a37660662072fe51192e107d3b8d8479486d5d36d41ee8573e276368")),
		// 65
		AccountId::from(hex!("9e78a430b5c7d63ce49e797f38ba634b75c8e79e0dc4fc0a249e15f82802f30d")),
		// 75
		AccountId::from(hex!("bec1e4c6cf71c6e267564cccbe2b9d825c0b68811e40f52bdb4837d49b829a41")),
		AccountId::from(hex!("1c67534e115323b07342363bed56125fa6dddf6bcb54c247c88f325751bd9b66")),
		AccountId::from(hex!("e2f3d397b87256fad6ac65d44b882af098a9273efb5b412c2ae705e736120f08")),
		AccountId::from(hex!("449cad9c2b898886f5b0a57ee9f8e17e7f8d704e9719913ca61c6ceaeee8fb75")),
		AccountId::from(hex!("c613379abae34a5de00b4c9704659507db7917c253aac580f235ee59ed197176")),
		AccountId::from(hex!("aea64a72af8f6abdb8045f072c1bea24da138c4d068c00dac5ebb6d46072404c")),
		AccountId::from(hex!("64aee2a884b006f9f0aa67a535ffec86cea6710da21b81d32bb8a3e7cfc9e80b")),
		AccountId::from(hex!("c281f7431834328d115601e8cbfa0da166c5971790b668f7990ea9cf71ebd651")),
		AccountId::from(hex!("f88185bfe86688efc6d35f5525a86a39eaccd5e621e9408bd18e2369cfc22832")),
		AccountId::from(hex!("ac4962e2c67ceadc47276389bd1b2e9dd41b995bc71eef8b477f9f242c210375")),
		AccountId::from(hex!("987beb5b1ba45f59a4318fa2adcad32b189d3e9761eff81bc81d0b13b3c74c29")),
		AccountId::from(hex!("ea4f4eb25458a3ce80200b818388bf77202a1efa8bce38a199506a0bf0bf4a09")),
		AccountId::from(hex!("185976dcc3e14d92f56d0a6f082dca70fd4bb880e7a97dd8f2d2576f1758f55b")),
		AccountId::from(hex!("44baef098c36626f1ae6b3ca332ffba1f14fbdef7f91fb9946eb9e68e280974e")),
	];
	for account in &to_deny {
		DeniedSource::<Runtime>::insert(account, true);
	}
	RocksDbWeight::get().writes(to_deny.len() as u64)
}
