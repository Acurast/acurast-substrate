use crate::Schedule;

macro_rules! tests {
    ($property_test_func:ident {
        $( $(#[$attr:meta])* $test_name:ident( $( $param:expr ),* ); )+
    }) => {
        $(
            $(#[$attr])*
            #[test]
            fn $test_name() {
                $property_test_func($( $param ),* )
            }
        )+
    }
}

fn test_schedule_execution_count(schedule: Schedule, exp_count: u64) {
	assert_eq!(schedule.execution_count(), exp_count);
}

tests! {
	test_schedule_execution_count {
		// ╭start ╭end
		// ■■■■■■■■
		test_schedule_execution_count_tight(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 8,
				interval: 2,
				max_start_delay: 0,
			},
			4
		);
		// ╭start         ╭end
		// ■■___■■___■■___
		test_schedule_execution_count_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 15,
				interval: 5,
				max_start_delay: 0,
			},
			3
		);
		// ╭start   ╭end
		// ■■___■■__
		test_schedule_execution_count_last_not_fitting(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 9,
				interval: 5,
				max_start_delay: 0,
			},
			2
		);
		// ╭start         ╭end
		// ■■□□_■■□□_■■□□_
		// □■■□_□■■□_□■■□_
		// □□■■_□□■■_□□■■_
		test_schedule_execution_count_delay_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 15,
				interval: 5,
				max_start_delay: 2,
			},
			3
		);
		// ╭start   ╭end
		// ■■□□_■■□□
		// □■■□_□■■□
		// □□■■_□□■■
		test_schedule_execution_count_delay_last_not_fitting(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 9,
				interval: 5,
				max_start_delay: 2,
			},
			2
		);
		// ╭start     ╭end
		// ■■□□_■■□□_■■□□
		// □■■□_□■■□_□■■□
		// □□■■_□□■■_□□■■
		test_schedule_execution_count_delay_reaching_over_end(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 11,
				interval: 5,
				max_start_delay: 2,
			},
			3
		);
		// ╭start=end
		//
		test_schedule_execution_count_zero_fits(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 0,
				interval: 5,
				max_start_delay: 2,
			},
			0
		);
		// ╭start=end-1
		// ■■□□
		// □■■□
		// □□■■
		test_schedule_execution_count_one_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 1,
				interval: 5,
				max_start_delay: 2,
			},
			1
		);
		//    ╭start     ╭end
		// ___■■□□_■■□□_■■□□
		// ___□■■□_□■■□_□■■□
		// ___□□■■_□□■■_□□■■
		test_schedule_execution_count_start_time_indifferent(
			Schedule{
				duration: 2,
				start_time: 3,
				end_time: 14,
				interval: 5,
				max_start_delay: 2,
			},
			3
		);
	}
}

fn test_schedule_iter(schedule: Schedule, start_delay: u64, exp_starts: Vec<u64>) {
	assert_eq!(schedule.iter(start_delay).unwrap().collect::<Vec<u64>>(), exp_starts);
}

tests! {
	test_schedule_iter {
		// ╭start  ╭end
		// ■■■■■■■■
		test_schedule_iter_tight(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 8,
				interval: 2,
				max_start_delay: 0,
			},
			0,
			vec![0,2,4,6]
		);
		// ╭start         ╭end
		// ■■___■■___■■___
		test_schedule_iter_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 15,
				interval: 5,
				max_start_delay: 0,
			},
			0,
			vec![0,5,10]
		);
		// ╭start   ╭end
		// ■■___■■__
		test_schedule_iter_last_not_fitting(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 9,
				interval: 5,
				max_start_delay: 0,
			},
			0,
			vec![0,5]
		);
		// ╭start         ╭end
		// ■■□□_■■□□_■■□□_
		test_schedule_iter_delay_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 15,
				interval: 5,
				max_start_delay: 2,
			},
			0,
			vec![0,5,10]
		);
		// ╭start         ╭end
		// □□■■_□□■■_□□■■_
		test_schedule_iter_delay_fit_2(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 15,
				interval: 5,
				max_start_delay: 2,
			},
			2,
			vec![2,7,12]
		);
		// ╭start   ╭end
		// ■■□□_■■□□
		// □■■□_□■■□
		// □□■■_□□■■
		test_schedule_iter_delay_last_not_fitting(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 9,
				interval: 5,
				max_start_delay: 2,
			},
			0,
			vec![0,5]
		);
		// ╭start     ╭end
		// ■■□□_■■□□_■■□□
		// □■■□_□■■□_□■■□
		// □□■■_□□■■_□□■■
		test_schedule_iter_delay_reaching_over_end(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 11,
				interval: 5,
				max_start_delay: 2,
			},
			0,
			vec![0,5,10]
		);
		// ╭start=end
		//
		test_schedule_iter_zero_fits(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 0,
				interval: 5,
				max_start_delay: 2,
			},
			0,
			vec![]
		);
		// ╭start=end-1
		// ■■□□
		// □■■□
		// □□■■
		test_schedule_iter_one_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 1,
				interval: 5,
				max_start_delay: 2,
			},
			0,
			vec![0]
		);
	}
}

fn test_schedule_overlaps(schedule: Schedule, start_delay: u64, ranges: Vec<((u64, u64), bool)>) {
	for (range, exp) in ranges.iter() {
		assert_eq!(
			&schedule.overlaps(start_delay, (range.0, range.1)),
			exp,
			"{:?}.overlaps(start_delay: {}, range, ({}, {})) != {}",
			schedule,
			start_delay,
			range.0,
			range.1,
			exp
		);
	}
}

tests! {
	test_schedule_overlaps {
		// ╭start ╭end
		// ■■■■■■■■
		// ranges:
		// ■
		//         ■■
		test_schedule_overlaps_tight(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 8,
				interval: 2,
				max_start_delay: 0,
			},
			0,
			vec![((0,1), true), ((8,10), false)]
		);
		//  ╭start ╭end
		// _■■■■■■■■
		// ranges:
		// ■
		// ■■
		// ■■■■■■■■■■
		//          ■■
		test_schedule_overlaps_start(
			Schedule{
				duration: 2,
				start_time: 1,
				end_time: 9,
				interval: 2,
				max_start_delay: 0,
			},
			0,
			vec![((0,1), false), ((0,2), true), ((0, 10), true), ((8, 10), true), ((9, 10), false)]
		);
		// ╭start         ╭end
		// ■■___■■___■■___
		//      ■■
		//       ■■
		//        ■■
		//             ■■■■
		test_schedule_overlaps_fit(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 15,
				interval: 5,
				max_start_delay: 0,
			},
			0,
			vec![((5,6), true), ((6,7), true), ((7,8), false), ((12, 16), false), ((3, 9), true), ((8, 14), true)]
		);
		//    ╭start     ╭end
		// ___□□■■_□□■■_□□■■
		// ranges:
		// ■■■
		//   ■■
		//           ■■
		//             ■■■
		test_schedule_overlaps_start_time(
			Schedule{
				duration: 2,
				start_time: 3,
				end_time: 14,
				interval: 5,
				max_start_delay: 2,
			},
			2,
			vec![((0,3), false), ((2,4), false), ((10,12), true), ((12,15), false)]
		);
		test_schedule_overlaps_end_before_start(
			Schedule{
				duration: 2,
				start_time: 1,
				end_time: 0,
				interval: 2,
				max_start_delay: 0,
			},
			0,
			vec![((0,1), false), ((0,2), false)]
		);
		test_schedule_overlaps_equal_start_end(
			Schedule{
				duration: 2,
				start_time: 0,
				end_time: 0,
				interval: 2,
				max_start_delay: 0,
			},
			0,
			vec![((0,1), false), ((0,2), false)]
		);
		// range starting before a schedule whose start_time % interval >= duration
		test_schedule_overlaps_range_starts_before(
			Schedule{
				duration: 300_000,
				start_time: 1_790_748_953_449,
				end_time: 1_793_340_953_450,
				interval: 86_400_000,
				max_start_delay: 10_000,
			},
			0,
			vec![((1_790_747_930_173, 1_790_834_330_173), true), ((1_790_747_930_173, 1_790_748_953_449), false)]
		);
	}
}

/// Execution spans of `schedule`, straight from the definition.
fn executions(schedule: &Schedule, delay: u64) -> impl Iterator<Item = (u128, u128)> + '_ {
	let first = schedule.start_time as u128 + delay as u128;
	(0..schedule.execution_count() as u128).map(move |i| {
		let start = first + i * schedule.interval as u128;
		(start, start + schedule.duration as u128)
	})
}

fn intersects(x: (u128, u128), y: (u128, u128)) -> bool {
	x.0.max(y.0) < x.1.min(y.1)
}

fn overlaps_by_definition(schedule: &Schedule, delay: u64, bounds: (u64, u64)) -> bool {
	executions(schedule, delay).any(|x| intersects(x, (bounds.0 as u128, bounds.1 as u128)))
}

fn overlaps_schedule_by_definition(a: &Schedule, a_delay: u64, b: &Schedule, b_delay: u64) -> bool {
	executions(a, a_delay).any(|x| executions(b, b_delay).any(|y| intersects(x, y)))
}

/// Same as [`overlaps_schedule_by_definition`] in O(n + m); requires `duration <= interval` for both.
fn overlaps_schedule_by_sweep(a: &Schedule, a_delay: u64, b: &Schedule, b_delay: u64) -> bool {
	let (mut xs, mut ys) = (executions(a, a_delay).peekable(), executions(b, b_delay).peekable());
	while let (Some(&x), Some(&y)) = (xs.peek(), ys.peek()) {
		if intersects(x, y) {
			return true;
		}
		if x.1 <= y.1 {
			xs.next();
		} else {
			ys.next();
		}
	}
	false
}

fn xorshift(mut seed: u64) -> impl FnMut(u64) -> u64 {
	move |max: u64| {
		seed ^= seed << 13;
		seed ^= seed >> 7;
		seed ^= seed << 17;
		if max == 0 {
			seed
		} else {
			seed % max
		}
	}
}

fn small_schedules(starts: core::ops::RangeInclusive<u64>) -> Vec<Schedule> {
	let mut schedules = vec![];
	for interval in 1..=5 {
		for duration in 0..=6 {
			for start_time in starts.clone() {
				for len in 1..=7 {
					schedules.push(Schedule {
						duration,
						start_time,
						end_time: start_time + len,
						interval,
						max_start_delay: 0,
					});
				}
			}
		}
	}
	schedules
}

/// Every pair in a small domain, including `duration == 0` and `duration > interval`.
#[test]
fn test_overlaps_schedule_exhaustive_small_domain() {
	let (xs, ys) = (small_schedules(0..=3), small_schedules(0..=10));
	for a in &xs {
		for b in &ys {
			let expected = overlaps_schedule_by_definition(a, 0, b, 0);
			assert_eq!(a.overlaps_schedule(0, b, 0), Some(expected), "a={a:?} b={b:?}");
			assert_eq!(b.overlaps_schedule(0, a, 0), Some(expected), "a={a:?} b={b:?}");
			// the `fits_schedule` prefilter must never hide an overlap
			if expected {
				assert!(a.overlaps(0, b.range(0)) && b.overlaps(0, a.range(0)), "a={a:?} b={b:?}");
			}
		}
	}
}

#[test]
fn test_overlaps_exhaustive_small_domain() {
	for schedule in small_schedules(0..=4) {
		for delay in 0..=2 {
			for a in 0..=20 {
				for b in a..=21 {
					assert_eq!(
						schedule.overlaps(delay, (a, b)),
						overlaps_by_definition(&schedule, delay, (a, b)),
						"schedule={schedule:?} delay={delay} bounds={:?}",
						(a, b)
					);
				}
			}
		}
	}
}

/// Realistic magnitudes with interval ratios and offsets chosen to sit on the overlap boundary.
#[test]
fn test_overlaps_schedule_adversarial_realistic() {
	let mut rand = xorshift(0x9E37_79B9_7F4A_7C15);
	let base = 1_790_748_953_449u64;
	let (mut hits, mut misses) = (0u32, 0u32);
	for case in 0..4_000u64 {
		let q = 60_001 + rand(86_400_000);
		let p = match case % 8 {
			0 => q,
			1 => q + 1,
			2 => 2 * q,
			3 => 2 * q + 1,
			4 => q * (2 + rand(5)) - 1,
			5 => q * 1_618_034 / 1_000_000,
			6 => q + rand(q),
			_ => q + 1 + rand(1_000),
		};
		let d = 1 + rand(q - 1);
		let e = match case % 3 {
			0 => q - d,
			1 => q - d + 1,
			_ => 1 + rand(q - d),
		};
		let offset = d + rand(3) - 1;
		let (s_delay, o_delay) = (rand(3_600_001), rand(3_600_001));
		let (n, m) = (1 + rand(2_000), 1 + rand(2_000));
		let s = Schedule {
			duration: d,
			start_time: base,
			end_time: base + (n - 1) * p + 1,
			interval: p,
			max_start_delay: 3_600_000,
		};
		let o_start = base + s_delay + offset - o_delay;
		let o = Schedule {
			duration: e,
			start_time: o_start,
			end_time: o_start + (m - 1) * q + 1 + rand(q),
			interval: q,
			max_start_delay: 3_600_000,
		};
		let expected = overlaps_schedule_by_sweep(&s, s_delay, &o, o_delay);
		assert_eq!(
			s.overlaps_schedule(s_delay, &o, o_delay),
			Some(expected),
			"s={s:?} s_delay={s_delay} o={o:?} o_delay={o_delay}"
		);
		assert_eq!(o.overlaps_schedule(o_delay, &s, s_delay), Some(expected));
		if expected {
			assert!(s.overlaps(s_delay, o.range(o_delay)) && o.overlaps(o_delay, s.range(s_delay)));
			hits += 1;
		} else {
			misses += 1;
		}
	}
	assert!(hits > 400 && misses > 400, "hits={hits} misses={misses}");
}

fn arbitrary_u64(rand: &mut impl FnMut(u64) -> u64) -> u64 {
	match rand(4) {
		0 => rand(50),
		1 => u64::MAX - rand(50),
		2 => u64::MAX / 2 + rand(50),
		_ => rand(0),
	}
}

fn arbitrary_schedule(rand: &mut impl FnMut(u64) -> u64) -> Schedule {
	let start_time = arbitrary_u64(rand);
	Schedule {
		duration: arbitrary_u64(rand),
		start_time,
		end_time: start_time.saturating_add(arbitrary_u64(rand)),
		interval: arbitrary_u64(rand),
		max_start_delay: 0,
	}
}

/// Arbitrary `u64` inputs, including values near `u64::MAX`: no panic, and exact whenever it answers.
#[test]
fn test_overlaps_schedule_arbitrary_inputs() {
	let mut rand = xorshift(0xD1B5_4A32_D192_ED03);
	for _ in 0..200_000 {
		let (a, b) = (arbitrary_schedule(&mut rand), arbitrary_schedule(&mut rand));
		let (a_delay, b_delay) = (arbitrary_u64(&mut rand), arbitrary_u64(&mut rand));
		let result = a.overlaps_schedule(a_delay, &b, b_delay);
		assert_eq!(result, b.overlaps_schedule(b_delay, &a, a_delay));
		let Some(result) = result else {
			let ends_beyond_u64 = |s: &Schedule, delay: u64| {
				let count = s.execution_count() as u128;
				count > 0
					&& s.start_time as u128
						+ delay as u128 + (count - 1) * s.interval as u128
						+ s.duration as u128
						> u64::MAX as u128
			};
			assert!(ends_beyond_u64(&a, a_delay) || ends_beyond_u64(&b, b_delay));
			continue;
		};
		if a.execution_count() <= 20 && b.execution_count() <= 20 {
			assert_eq!(
				result,
				overlaps_schedule_by_definition(&a, a_delay, &b, b_delay),
				"a={a:?} a_delay={a_delay} b={b:?} b_delay={b_delay}"
			);
		}
	}
}

#[test]
fn test_schedule_execution() {
	let schedule =
		Schedule { duration: 2, start_time: 10, end_time: 20, interval: 4, max_start_delay: 5 };
	for index in 0..schedule.execution_count() {
		let execution = schedule.execution(index).unwrap();
		assert_eq!(execution.execution_count(), 1);
		assert_eq!(execution.nth_start_time(3, 0), schedule.nth_start_time(3, index));
		assert_eq!(execution.duration, schedule.duration);
	}
	assert_eq!(schedule.execution(schedule.execution_count()), None);
}

#[test]
fn test_overlaps_schedule_last_end_at_u64_max() {
	let schedule = |end_of_last: u64| Schedule {
		duration: 10,
		start_time: end_of_last - 20,
		end_time: end_of_last - 9,
		interval: 10,
		max_start_delay: 0,
	};
	let other =
		Schedule { duration: 5, start_time: 0, end_time: 1, interval: 10, max_start_delay: 0 };
	assert_eq!(schedule(u64::MAX).overlaps_schedule(0, &other, 0), Some(false));
	assert_eq!(schedule(u64::MAX).overlaps_schedule(1, &other, 0), None);
	assert_eq!(other.overlaps_schedule(0, &schedule(u64::MAX), 1), None);
}

#[test]
fn test_floor_sum_matches_brute_force() {
	let mut rand = xorshift(0x2545_F491_4F6C_DD1D);
	for case in 0..20_000 {
		let max = if case % 2 == 0 { 1_000 } else { u64::MAX };
		let (n, m) = (rand(50) as u128, 1 + rand(max) as u128);
		let (a, b) = (rand(max) as u128, rand(max) as u128);
		let expected = (0..n).fold(0u128, |acc, i| acc.wrapping_add((a * i + b) / m));
		assert_eq!(crate::types::floor_sum(n, m, a, b), expected, "n={n} m={m} a={a} b={b}");
	}
	let n = u64::MAX as u128;
	assert_eq!(crate::types::floor_sum(n, 1, 0, 7), 7 * n);
	assert_eq!(crate::types::floor_sum(n, 1, 1, 0), n * (n - 1) / 2);
	assert_eq!(crate::types::floor_sum(n, u64::MAX as u128, 0, 0), 0);
}
